<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, type FileDetail, type Suspect } from "../lib/api";
  import { clock, duration, num, when } from "../lib/format";

  let { id, onBack }: { id: number; onBack: () => void } = $props();

  let d = $state<FileDetail | null>(null);
  let loadError = $state<string | null>(null);
  let editing = $state<number | null>(null);
  let draft = $state("");
  let msg = $state<{ ok: boolean; text: string } | null>(null);
  let showSuspects = $state(true);
  let confirmDelete = $state(false);

  async function load() {
    try {
      d = await api.fileDetail(id);
    } catch (e) {
      loadError = String(e);
    }
  }
  $effect(() => {
    id;
    load();
  });

  const suspectAt = $derived(new Map<number, Suspect>((d?.suspects ?? []).map((s) => [s.idx, s])));

  /** Parágrafos para leitura: frase terminada ou silêncio medido de 1 s ou mais. */
  const paragraphs = $derived.by(() => {
    if (!d) return [];
    const { words, pauses } = d.transcript;
    const silence = (a: number, b: number) => pauses.reduce((acc, p) => acc + Math.max(0, Math.min(p.end, b) - Math.max(p.start, a)), 0);
    const out: number[][] = [];
    let cur: number[] = [];
    words.forEach((w, i) => {
      if (cur.length) {
        const prev = words[i - 1];
        if (/[.?!…]["')»”]*$/.test(prev.word) || silence(prev.end, w.start) >= 1) {
          out.push(cur);
          cur = [];
        }
      }
      cur.push(i);
    });
    if (cur.length) out.push(cur);
    return out;
  });

  function startEdit(i: number) {
    editing = i;
    draft = d!.transcript.words[i].word;
    msg = null;
  }

  async function commit() {
    if (editing === null || !d) return;
    const i = editing;
    const text = draft.trim();
    editing = null;
    if (text === d.transcript.words[i].word) return;
    try {
      const warning = await api.fileUpdateWord(id, i, text);
      d.transcript.words[i].word = text;
      await load(); // recalcula os candidatos
      msg = warning ? { ok: false, text: warning } : { ok: true, text: d.outputs_exist ? "Corrigido. .json, .txt e .srt atualizados." : "Corrigido." };
    } catch (e) {
      msg = { ok: false, text: String(e) };
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      editing = null;
    }
  }

  async function exportAs(ext: "srt" | "txt" | "json") {
    if (!d) return;
    const names = { srt: "Legenda", txt: "Texto", json: "Palavras com tempo" };
    const dest = await save({ defaultPath: `${d.session.title}.${ext}`, filters: [{ name: names[ext], extensions: [ext] }] });
    if (!dest) return;
    try {
      await api.fileExport(id, dest);
      msg = { ok: true, text: `Exportado: ${dest}` };
    } catch (e) {
      msg = { ok: false, text: String(e) };
    }
  }

  async function remove() {
    await api.fileDelete(id);
    onBack();
  }

  const kindLabel = { pt_pt: "grafia de Portugal", name: "nome próprio?", language: "inglês/espanhol?" } as const;
  function suspectTitle(s: Suspect): string {
    return `${kindLabel[s.kind]}${s.hint ? ` → ${s.hint}` : ""} · ${clock(s.start)}`;
  }

  const counts = $derived.by(() => {
    const c = { pt_pt: 0, name: 0, language: 0 };
    d?.suspects.forEach((s) => c[s.kind]++);
    return c;
  });
</script>

<button class="back" onclick={onBack}>← Arquivos</button>

{#if loadError}
  <p class="error">{loadError}</p>
{:else if d}
  <header>
    <div>
      <h1>{d.session.title}</h1>
      <p class="meta">
        {duration(d.transcript.duration)} · {d.transcript.words.length} palavras · {when(d.session.started_at)}
      </p>
    </div>
    <div class="actions">
      {#if d.outputs_exist}
        <button class="btn" onclick={() => api.fileReveal(id)}>Abrir pasta</button>
      {/if}
      <button class="btn" onclick={() => exportAs("srt")}>Exportar .srt</button>
      <button class="btn" onclick={() => exportAs("txt")}>.txt</button>
      <button class="btn" onclick={() => exportAs("json")}>.json</button>
    </div>
  </header>

  {#if d.report}
    {@const r = d.report}
    <dl class="report">
      <div><dt>Carga do modelo</dt><dd>{num(r.load_s, 2)} s</dd></div>
      <div><dt>Transcrição</dt><dd>{num(r.transcribe_s, 2)} s</dd></div>
      <div>
        <dt>Fator de tempo real</dt>
        <dd>{num(r.rtf)}× <span class="hint">duração ÷ tempo</span></dd>
      </div>
      {#if r.chunks > 1}<div><dt>Partes</dt><dd>{r.chunks}</dd></div>{/if}
    </dl>
    {#if r.forced_cuts.length > 3}
      <p class="warnbox">
        {r.forced_cuts.length} de {r.chunks - 1} cortes sem silêncio medido (o áudio tem ruído de fundo): foram feitos no ponto de menor
        volume. Alguma palavra perto deles pode ter saído cortada.
      </p>
    {:else if r.forced_cuts.length}
      <p class="warnbox">
        Sem silêncio para cortar em {r.forced_cuts.map(clock).join(", ")}: o corte foi no ponto de menor volume. Confira as palavras
        perto desses tempos.
      </p>
    {/if}
  {/if}

  <section class="suspects">
    <div class="srow">
      <strong>{d.suspects.length} candidatos a erro</strong>
      <span class="hint">
        {num(d.suspects_per_min)} por minuto · {counts.name} nomes próprios · {counts.language} em inglês/espanhol · {counts.pt_pt} grafias de Portugal
      </span>
      <label class="toggle"><input type="checkbox" bind:checked={showSuspects} /> destacar</label>
    </div>
    {#if d.suspects_per_min > 3}
      <p class="hint">Mais de 3 por minuto. Se o texto precisar ficar exato, a comparação com o Whisper (Parte 4) vai ajudar quando estiver disponível.</p>
    {/if}
    <p class="hint">Clique numa palavra para corrigir o texto. Os tempos nunca mudam. Enter salva, Esc cancela.</p>
  </section>

  {#if msg}<p class={msg.ok ? "okmsg" : "error"} role="status">{msg.text}</p>{/if}

  <article class="text">
    {#each paragraphs as para}
      <p>
        <span class="t">{clock(d.transcript.words[para[0]].start)}</span>
        {#each para as i (i)}
          {@const w = d.transcript.words[i]}
          {@const s = showSuspects ? suspectAt.get(i) : undefined}
          {#if editing === i}
            <input
              class="edit"
              bind:value={draft}
              onkeydown={onKey}
              onblur={commit}
              size={Math.max(3, draft.length)}
              spellcheck="false"
              aria-label="Corrigir palavra"
              {@attach (el) => {
                el.focus();
                el.select();
              }}
            />
          {:else}
            <button class="w {s ? `s-${s.kind}` : ''}" title={s ? suspectTitle(s) : clock(w.start)} onclick={() => startEdit(i)}>{w.word}</button>
          {/if}
          {" "}
        {/each}
      </p>
    {/each}
  </article>

  <footer>
    {#if confirmDelete}
      <span>Apagar esta transcrição do OuveAI? Os arquivos ao lado do original continuam lá.</span>
      <button class="btn danger" onclick={remove}>Apagar</button>
      <button class="btn" onclick={() => (confirmDelete = false)}>Cancelar</button>
    {:else}
      <button class="btn subtle" onclick={() => (confirmDelete = true)}>Apagar transcrição</button>
    {/if}
  </footer>
{:else}
  <p class="hint">Carregando…</p>
{/if}

<style>
  .back {
    margin: -8px 0 14px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
  }
  .back:hover {
    color: var(--accent);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    max-width: 760px;
  }
  h1 {
    margin: 0;
    font-size: 21px;
    font-weight: 650;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }
  .meta {
    margin: 2px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .report {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 28px;
    max-width: 760px;
    margin: 18px 0 0;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .report dt {
    color: var(--text-2);
    font-size: 12px;
  }
  .report dd {
    margin: 0;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .warnbox {
    max-width: 760px;
    margin: 10px 0 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-size: 13px;
  }
  .suspects {
    max-width: 760px;
    margin: 18px 0 0;
  }
  .srow {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 12px;
  }
  .toggle {
    margin-left: auto;
    color: var(--text-2);
    font-size: 13px;
  }
  .toggle input {
    accent-color: var(--accent);
  }
  .hint {
    margin: 4px 0 0;
    color: var(--text-2);
    font-size: 12.5px;
    font-weight: 400;
  }
  .error {
    color: var(--err);
    font-size: 13px;
  }
  .okmsg {
    color: var(--accent);
    font-size: 13px;
  }
  .text {
    max-width: 760px;
    margin-top: 14px;
    padding: 18px 22px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
    font-size: 15px;
    line-height: 1.75;
  }
  .text p {
    position: relative;
    margin: 0 0 12px;
    padding-left: 56px;
  }
  .t {
    position: absolute;
    left: 0;
    top: 3px;
    color: var(--text-3);
    font: 12px var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .w {
    display: inline;
    padding: 0 1px;
    border: 0;
    border-radius: 3px;
    background: none;
    font: inherit;
    line-height: inherit;
  }
  .w:hover {
    background: var(--surface-2);
  }
  .w:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .s-name {
    text-decoration: underline dotted var(--outros);
    text-underline-offset: 4px;
  }
  .s-language,
  .s-pt_pt {
    background: var(--warn-bg);
    color: var(--warn-ink);
  }
  .edit {
    padding: 0 3px;
    border: 1px solid var(--accent);
    border-radius: 3px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    line-height: 1.3;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 760px;
    margin-top: 18px;
    color: var(--text-2);
    font-size: 13px;
  }
  .btn.subtle {
    border-color: transparent;
    background: transparent;
    color: var(--text-2);
  }
  .btn.danger {
    border-color: var(--err);
    color: var(--err);
  }
</style>
