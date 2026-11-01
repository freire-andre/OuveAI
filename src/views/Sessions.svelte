<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { save } from "@tauri-apps/plugin-dialog";
  import ComparePanel from "../components/ComparePanel.svelte";
  import { api, FLAG_ECHO, snippetParts, tzOffset, type Hit, type MeetingDetail, type SegmentRow, type SessionRow, type Status } from "../lib/api";
  import { clock, duration, when } from "../lib/format";

  let { status, onOpenFile }: { status: Status; onOpenFile: (id: number) => void } = $props();
  const recordAccel = $derived(status.shortcuts.find((s) => s.action === "record")?.accel ?? "");

  let sessions = $state<SessionRow[] | null>(null);
  let open = $state<number | null>(null);
  let detail = $state<MeetingDetail | null>(null);
  let panel = $state<ComparePanel>();
  let showEcho = $state(false);
  let confirmDelete = $state(false);
  let renaming = $state<string | null>(null);
  let err = $state<string | null>(null);
  let msg = $state<string | null>(null);
  /** Fala vinda da busca: destacada e rolada até ficar visível. */
  let focusSeg = $state<number | null>(null);

  // Busca
  let query = $state("");
  let hits = $state<Hit[] | null>(null);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSeq = 0;

  // Hora do relógio ao copiar/exportar (preferência deste computador).
  const CLOCK_KEY = "ouveai.copy.clock";
  let useClock = $state(readClock());
  function readClock(): boolean {
    try {
      return localStorage.getItem(CLOCK_KEY) === "1";
    } catch {
      return false;
    }
  }
  $effect(() => {
    try {
      localStorage.setItem(CLOCK_KEY, useClock ? "1" : "0");
    } catch {
      /* sem armazenamento: vale só nesta janela */
    }
  });

  const refresh = () => api.meetingSessions().then((s) => (sessions = s)).catch((e) => (err = String(e)));

  async function show(id: number, seg: number | null = null) {
    err = null;
    msg = null;
    confirmDelete = false;
    renaming = null;
    stopAudio();
    try {
      detail = await api.meetingDetail(id);
      open = id;
      focusSeg = seg;
      if (seg !== null) {
        await tick();
        document.getElementById(`seg-${seg}`)?.scrollIntoView({ block: "center" });
      }
    } catch (e) {
      err = String(e);
    }
  }

  function back() {
    stopAudio();
    open = null;
    detail = null;
    focusSeg = null;
  }

  onMount(() => {
    refresh();
    const offs = [
      listen<number>("meeting-done", (e) => {
        refresh();
        if (open === e.payload) show(e.payload);
      }),
      listen("live-segment", () => {
        if (open !== null && status.recording) show(open, focusSeg);
      }),
      listen("sessions-changed", () => {
        refresh();
        if (query.trim()) runSearch();
        if (open !== null) api.meetingDetail(open).then((d) => (detail = d)).catch(() => back());
      }),
    ];
    return () => {
      offs.forEach((p) => p.then((off) => off()));
      stopAudio();
    };
  });

  $effect(() => {
    if (status.recording) refresh();
  });

  // ---------- busca ----------

  function onQuery() {
    clearTimeout(searchTimer);
    if (!query.trim()) {
      hits = null;
      return;
    }
    searchTimer = setTimeout(runSearch, 180);
  }

  async function runSearch() {
    const mine = ++searchSeq;
    try {
      const r = await api.search(query);
      if (mine === searchSeq) hits = r;
    } catch (e) {
      err = String(e);
    }
  }

  type Group = { id: number; kind: Hit["kind"]; title: string; started_at: string; hits: Hit[] };
  const groups = $derived.by((): Group[] => {
    const out: Group[] = [];
    for (const h of hits ?? []) {
      const g = out.at(-1);
      if (g && g.id === h.session_id) g.hits.push(h);
      else out.push({ id: h.session_id, kind: h.kind, title: h.title, started_at: h.started_at, hits: [h] });
    }
    return out;
  });

  function openHit(h: Hit) {
    if (h.kind === "file") onOpenFile(h.session_id);
    else show(h.session_id, h.segment_id);
  }

  // ---------- detalhe ----------

  type Item =
    | { kind: "seg"; t: number; seg: SegmentRow; echo: boolean }
    | { kind: "mark"; t: number; id: number; mark: string; note: string | null };

  const items = $derived.by((): Item[] => {
    if (!detail) return [];
    const segs: Item[] = detail.segments
      .map((s) => ({ kind: "seg" as const, t: s.start_s, seg: s, echo: (s.flags & FLAG_ECHO) !== 0 }))
      .filter((s) => showEcho || !s.echo || s.seg.id === focusSeg);
    const marks: Item[] = detail.marks.map((m) => ({ kind: "mark" as const, t: m.t_s, id: m.id, mark: m.kind, note: m.note }));
    return [...segs, ...marks].sort((a, b) => a.t - b.t);
  });
  const echoes = $derived(detail?.segments.filter((s) => (s.flags & FLAG_ECHO) !== 0).length ?? 0);
  const live = $derived(detail?.session.status === "recording" || detail?.session.status === "paused");

  async function rename() {
    if (!detail || renaming === null) return;
    try {
      await api.meetingRename(detail.session.id, renaming);
      renaming = null;
      await show(detail.session.id);
      refresh();
    } catch (e) {
      err = String(e);
    }
  }

  async function remove() {
    if (!detail) return;
    try {
      await api.meetingDelete(detail.session.id);
      back();
      refresh();
    } catch (e) {
      err = String(e);
    }
  }

  async function copy() {
    if (!detail) return;
    err = null;
    try {
      const c = await api.meetingCopy(detail.session.id, useClock ? tzOffset() : null);
      if (!c.markdown.trim()) {
        msg = "Nada para copiar ainda.";
        return;
      }
      try {
        // HTML para o Teams/Outlook colarem com negrito; Markdown como texto puro.
        await navigator.clipboard.write([
          new ClipboardItem({
            "text/plain": new Blob([c.markdown], { type: "text/plain" }),
            "text/html": new Blob([c.html], { type: "text/html" }),
          }),
        ]);
      } catch {
        await navigator.clipboard.writeText(c.markdown);
      }
      msg = "Transcrição copiada. Cole no chat, no e-mail ou num documento.";
    } catch (e) {
      err = `Não foi possível copiar: ${e}`;
    }
  }

  const exportNames: Record<string, string> = { md: "Markdown", txt: "Texto", srt: "Legenda SubRip", json: "JSON" };

  async function exportAs(ext: string) {
    if (!detail || !ext) return;
    err = null;
    msg = null;
    const safe = detail.session.title.replace(/[\\/:*?"<>|]+/g, " ").trim() || "Reunião";
    const dest = await save({ defaultPath: `${safe}.${ext}`, filters: [{ name: exportNames[ext], extensions: [ext] }] });
    if (!dest) return;
    try {
      await api.meetingExport(detail.session.id, dest, useClock ? tzOffset() : null);
      msg = `Exportado: ${dest}`;
    } catch (e) {
      err = String(e);
    }
  }

  // ---------- ouvir o trecho ----------

  let player: HTMLAudioElement | null = null;
  let playerUrl: string | null = null;
  let playing = $state<number | null>(null);
  let loadingClip = $state<number | null>(null);

  function stopAudio() {
    player?.pause();
    player = null;
    if (playerUrl) URL.revokeObjectURL(playerUrl);
    playerUrl = null;
    playing = null;
  }

  async function toggleAudio(s: SegmentRow) {
    if (!detail) return;
    if (playing === s.id) return stopAudio();
    stopAudio();
    err = null;
    loadingClip = s.id;
    try {
      const buf = await api.meetingClip(detail.session.id, s.start_s, s.end_s);
      playerUrl = URL.createObjectURL(new Blob([buf], { type: "audio/wav" }));
      player = new Audio(playerUrl);
      player.onended = stopAudio;
      playing = s.id;
      await player.play();
    } catch (e) {
      stopAudio();
      err = String(e);
    } finally {
      loadingClip = null;
    }
  }

  const statusLabel: Record<string, string> = {
    recording: "gravando",
    paused: "pausada",
    interrupted: "interrompida",
  };
  const markLabel: Record<string, string> = { estrela: "★ Marcado", dispositivo: "Troca de dispositivo", pausa: "Pausa" };
</script>

{#if open !== null && detail}
  <button class="back" onclick={back}>← Sessões</button>
  <div class="title">
    {#if renaming !== null}
      <form onsubmit={(e) => { e.preventDefault(); rename(); }}>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={renaming} autofocus maxlength="120" aria-label="Nome da reunião" />
        <button class="btn primary" type="submit">Salvar</button>
        <button class="btn" type="button" onclick={() => (renaming = null)}>Cancelar</button>
      </form>
    {:else}
      <h1>{detail.session.title}</h1>
      <button class="btn" onclick={() => (renaming = detail?.session.title ?? "")}>Renomear</button>
    {/if}
  </div>
  <p class="muted">
    {when(detail.session.started_at)}
    {#if detail.session.duration_s} · {duration(detail.session.duration_s)}{/if}
    · {detail.session.words} palavras
    {#if statusLabel[detail.session.status]} · <b>{statusLabel[detail.session.status]}</b>{/if}
  </p>

  <div class="bar">
    <button class="btn primary" onclick={copy} title="Copia em Markdown, com negrito ao colar no Teams e no Outlook. Os ecos ficam de fora.">
      Copiar transcrição
    </button>
    <select
      class="export"
      aria-label="Exportar"
      value=""
      onchange={(e) => {
        const v = e.currentTarget.value;
        e.currentTarget.value = "";
        exportAs(v);
      }}
    >
      <option value="" disabled>Exportar…</option>
      <option value="md">Markdown (.md)</option>
      <option value="txt">Texto (.txt)</option>
      <option value="srt">Legenda (.srt)</option>
      <option value="json">JSON (.json)</option>
    </select>
    <label class="toggle" title="Ao copiar e exportar em .md/.txt: [14:32:05] em vez de [00:12:03]">
      <input type="checkbox" bind:checked={useClock} />
      Hora do relógio
    </label>
    <span class="spacer"></span>
    {#if detail.audio_dir && !live}
      <button class="btn" title="Transcreve de novo com o Whisper, a partir do áudio salvo, e mostra só os trechos em que os dois discordam" onclick={() => panel?.start()}>Comparar com o Whisper</button>
    {/if}
    {#if detail.audio_dir}
      <button class="btn" onclick={() => detail && api.meetingRevealAudio(detail.session.id)}>
        Abrir áudio{detail.audio_expires_at ? ` (até ${when(detail.audio_expires_at)})` : ""}
      </button>
    {/if}
    {#if confirmDelete}
      <button class="btn danger" onclick={remove}>Apagar de vez</button>
      <button class="btn" onclick={() => (confirmDelete = false)}>Cancelar</button>
    {:else}
      <button class="btn" disabled={live} onclick={() => (confirmDelete = true)}>Apagar</button>
    {/if}
  </div>
  {#if echoes > 0}
    <label class="toggle echo-toggle">
      <input type="checkbox" bind:checked={showEcho} />
      Mostrar ecos ({echoes}): falas dos outros que o microfone captou pela caixa de som
    </label>
  {/if}
  {#if msg}<p class="ok">{msg}</p>{/if}
  {#if err}<p class="err">{err}</p>{/if}
  {#key detail.session.id}
    <ComparePanel bind:this={panel} sessionId={detail.session.id} onApplied={() => detail && show(detail.session.id)} />
  {/key}

  {#if items.length === 0}
    <p class="muted">Nenhuma fala transcrita.</p>
  {:else}
    {#if detail.audio_dir}
      <p class="muted small">Clique no horário de uma fala para ouvir o trecho.</p>
    {/if}
    <ol class="transcript">
      {#each items as it (it.kind + (it.kind === "seg" ? it.seg.id : it.id))}
        {#if it.kind === "seg"}
          <li id="seg-{it.seg.id}" class:echo={it.echo} class:focus={it.seg.id === focusSeg} class:playing={playing === it.seg.id}>
            {#if detail.audio_dir}
              <button
                class="t play"
                aria-label={playing === it.seg.id ? "Parar" : `Ouvir a fala de ${clock(it.t)}`}
                onclick={() => toggleAudio(it.seg)}
              >
                <span class="ico" aria-hidden="true">{playing === it.seg.id ? "■" : loadingClip === it.seg.id ? "…" : "▶"}</span>{clock(it.t)}
              </button>
            {:else}
              <span class="t">{clock(it.t)}</span>
            {/if}
            <span class="who" class:eu={it.seg.channel === "EU"} class:outros={it.seg.channel === "OUTROS"}>{it.seg.channel}</span>
            <span class="text">{it.echo ? "(eco) " : ""}{it.seg.text}</span>
          </li>
        {:else}
          <li class="mark" class:star={it.mark === "estrela"}>
            <span class="t">{clock(it.t)}</span>
            <span class="text">{markLabel[it.mark] ?? it.mark}{it.note ? `: ${it.note}` : ""}</span>
          </li>
        {/if}
      {/each}
    </ol>
  {/if}
{:else}
  <div class="head">
    <h1>Sessões</h1>
    <button class="btn primary" onclick={() => api.toggleRecord()}>{status.recording ? "Parar gravação" : "Gravar reunião"}</button>
  </div>
  <div class="search">
    <input
      type="search"
      placeholder="Buscar nas reuniões e arquivos (sem precisar de acento)"
      aria-label="Buscar nas transcrições"
      bind:value={query}
      oninput={onQuery}
      onkeydown={(e) => e.key === "Escape" && ((query = ""), onQuery())}
      spellcheck="false"
    />
  </div>
  {#if err}<p class="err">{err}</p>{/if}

  {#if query.trim()}
    {#if hits === null}
      <p class="muted">Buscando…</p>
    {:else if hits.length === 0}
      <p class="muted">Nada encontrado para “{query.trim()}”.</p>
    {:else}
      <p class="muted small">{hits.length === 200 ? "Os 200 primeiros resultados" : `${hits.length} ${hits.length === 1 ? "resultado" : "resultados"}`}</p>
      <div class="results">
        {#each groups as g (g.id)}
          <section class="group">
            <header>
              <span class="kind">{g.kind === "file" ? "Arquivo" : "Reunião"}</span>
              <span class="name">{g.title}</span>
              <span class="muted">{when(g.started_at)}</span>
            </header>
            {#each g.hits as h (h.segment_id)}
              <button class="hit" onclick={() => openHit(h)}>
                <span class="t">{clock(h.start_s)}</span>
                <span class="who" class:eu={h.channel === "EU"} class:outros={h.channel === "OUTROS"}>{h.channel === "ARQUIVO" ? "" : h.channel}</span>
                <span class="text">
                  {#each snippetParts(h.snippet) as p}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}
                </span>
              </button>
            {/each}
          </section>
        {/each}
      </div>
    {/if}
  {:else if sessions && sessions.length === 0}
    <section class="empty">
      <p class="lead">Nenhuma reunião ainda.</p>
      <p>
        Quando a reunião começar, use <kbd>{recordAccel}</kbd> de qualquer lugar ou o botão acima. O OuveAI grava o seu
        microfone (EU) e o som do computador (OUTROS) e transcreve enquanto vocês falam, sem nada sair desta máquina.
      </p>
    </section>
  {:else if sessions}
    <ul class="list">
      {#each sessions as s (s.id)}
        <li>
          <button class="item" onclick={() => show(s.id)}>
            <span class="name">{s.title}</span>
            <span class="muted">
              {when(s.started_at)}{s.duration_s ? ` · ${duration(s.duration_s)}` : ""} · {s.words} palavras
              {#if statusLabel[s.status]} · <b class:live={s.status === "recording"}>{statusLabel[s.status]}</b>{/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

<style>
  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .head,
  .title {
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 760px;
    margin-bottom: 16px;
  }
  .head {
    justify-content: space-between;
  }
  .title {
    margin-bottom: 4px;
  }
  .title form {
    display: flex;
    flex: 1;
    gap: 8px;
  }
  .title input,
  .search input {
    flex: 1;
    padding: 6px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
  }
  .title input {
    font: 600 16px inherit;
  }
  .search {
    display: flex;
    max-width: 760px;
    margin-bottom: 14px;
  }
  .search input {
    padding: 8px 12px;
    font: inherit;
  }
  .back {
    margin: 0 0 12px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
  }
  .back:hover {
    color: var(--text);
  }
  .muted {
    margin: 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .small {
    margin-bottom: 6px;
    font-size: 12px;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    max-width: 760px;
    margin: 14px 0 8px;
  }
  .spacer {
    flex: 1;
  }
  .export {
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font: inherit;
  }
  .toggle {
    display: flex;
    gap: 6px;
    align-items: center;
    color: var(--text-2);
    font-size: 13px;
  }
  .echo-toggle {
    margin: 0 0 8px;
  }
  .toggle input {
    accent-color: var(--accent);
  }
  .btn.danger {
    border-color: var(--err);
    color: var(--err);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .ok {
    margin: 4px 0 8px;
    color: var(--accent);
    font-size: 13px;
  }
  .err {
    color: var(--err);
    font-size: 13px;
  }
  .transcript {
    max-width: 760px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .transcript li {
    display: grid;
    grid-template-columns: 64px 64px 1fr;
    gap: 10px;
    padding: 6px 4px;
    border-radius: var(--radius-sm);
    line-height: 1.5;
  }
  .transcript li.echo {
    opacity: 0.5;
  }
  .transcript li.focus {
    background: var(--warn-bg);
  }
  .transcript li.playing {
    background: var(--surface-2);
  }
  .transcript li.mark {
    grid-template-columns: 64px 1fr;
    color: var(--text-2);
    font-size: 12.5px;
  }
  .transcript li.mark.star {
    color: var(--accent);
    font-weight: 600;
  }
  .t {
    color: var(--text-3);
    font: 12px var(--mono);
    padding-top: 2px;
  }
  .play {
    display: inline-flex;
    gap: 5px;
    align-items: flex-start;
    height: fit-content;
    padding: 2px 0 0;
    border: 0;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .play .ico {
    width: 9px;
    font-size: 9px;
    line-height: 16px;
    opacity: 0.35;
  }
  .play:hover,
  li.playing .play {
    color: var(--accent);
  }
  .play:hover .ico,
  li.playing .play .ico {
    opacity: 1;
  }
  .who {
    font: 600 11px var(--mono);
    padding-top: 3px;
  }
  .eu {
    color: var(--eu);
  }
  .outros {
    color: var(--outros);
  }
  .text {
    user-select: text;
  }
  .results {
    max-width: 760px;
  }
  .group {
    margin-bottom: 14px;
  }
  .group header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 4px 4px 6px;
    border-bottom: 1px solid var(--line);
  }
  .group .kind {
    color: var(--text-3);
    font: 600 10.5px var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .group .name {
    font-weight: 600;
  }
  .hit {
    display: grid;
    grid-template-columns: 56px auto 1fr;
    gap: 10px;
    width: 100%;
    padding: 6px 4px;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    text-align: left;
    line-height: 1.45;
  }
  .hit:hover {
    background: var(--surface-2);
  }
  .hit .text {
    grid-column: span 1;
  }
  mark {
    padding: 0 1px;
    border-radius: 2px;
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-weight: 600;
  }
  .empty {
    max-width: 520px;
    padding: 28px;
    border: 1px dashed var(--line);
    border-radius: var(--radius);
    color: var(--text-2);
  }
  .empty p {
    margin: 0 0 14px;
  }
  .lead {
    color: var(--text);
    font-size: 16px;
    font-weight: 600;
  }
  .list {
    max-width: 640px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .list li + li {
    border-top: 1px solid var(--line);
  }
  .item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    width: 100%;
    padding: 10px 4px;
    border: 0;
    background: none;
    text-align: left;
  }
  .item .name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .item:hover .name {
    color: var(--accent);
  }
  b.live {
    color: var(--rec);
  }
</style>
