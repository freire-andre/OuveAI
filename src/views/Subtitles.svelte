<script module lang="ts">
  /** O edl.json aberto continua aberto ao trocar de tela. */
  let lastPath: string | null = null;
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { open as pick } from "@tauri-apps/plugin-dialog";
  import { api, watchEdl, type EdlCheckView, type EdlGenerated, type EdlView, type Stage } from "../lib/api";
  import { clock } from "../lib/format";

  const VIDEO = ["mp4", "mkv", "mov", "avi", "webm", "wmv", "m4v", "mp3", "m4a", "wav"];
  const REFERENCIA = 0.12;

  let edl = $state<EdlView | null>(null);
  let check = $state<EdlCheckView | null>(null);
  let generated = $state<EdlGenerated | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);

  const num = (n: number, d = 3) => n.toFixed(d).replace(".", ",");
  const signed = (n: number) => (n > 0 ? "+" : n < 0 ? "−" : "") + num(Math.abs(n), 2);
  const base = (p: string) => p.split(/[\\/]/).pop() ?? p;

  async function load(path: string) {
    error = null;
    notice = null;
    generated = null;
    try {
      edl = await api.edlOpen(path);
      lastPath = path;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    const off = watchEdl((v) => {
      check = v;
      if (v && !edl) load(v.edl);
    });
    if (lastPath) load(lastPath);
    return () => off.then((f) => f());
  });

  async function chooseEdl() {
    const r = await pick({ title: "Escolha o edl.json", filters: [{ name: "Lista de cortes (edl.json)", extensions: ["json"] }] });
    if (typeof r === "string") {
      if (check && check.kind !== "loading" && check.kind !== "working") await api.edlDismiss();
      await load(r);
    }
  }

  async function generate() {
    if (!edl) return;
    busy = true;
    error = null;
    try {
      generated = await api.edlGenerate(edl.path);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function verify() {
    if (!edl) return;
    const r = await pick({ title: "Escolha o vídeo final exportado", filters: [{ name: "Vídeo ou áudio", extensions: VIDEO }] });
    if (typeof r !== "string") return;
    error = null;
    notice = null;
    try {
      await api.edlCheckStart(edl.path, r);
    } catch (e) {
      error = String(e);
    }
  }

  async function applyFixes() {
    if (check?.kind !== "done") return;
    busy = true;
    error = null;
    try {
      const bak = await api.edlApply(check.check.fixes);
      notice = `edl.json corrigido. O original ficou em ${bak}. A legenda e a verificação foram refeitas.`;
      edl = await api.edlOpen(check.edl);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function stageLabel(s: Stage): string {
    switch (s.stage) {
      case "converting":
        return "Convertendo o áudio do vídeo final…";
      case "analyzing":
        return "Medindo os silêncios…";
      case "recognizing":
        return s.chunks > 1 ? `Transcrevendo o vídeo final · parte ${s.chunk} de ${s.chunks}` : "Transcrevendo o vídeo final…";
      case "saving":
        return "Salvando…";
    }
  }

  const running = $derived(check?.kind === "loading" || check?.kind === "working");
  const mine = $derived(check !== null && edl !== null && check.edl === edl.path);
</script>

<h1>Legendas</h1>
<p class="lead">
  Legenda de um vídeo editado a partir dos brutos já transcritos. Transcreva cada bruto em Arquivos (e corrija o texto, se
  precisar), monte a lista de cortes no <code>edl.json</code> e gere a legenda do vídeo final. Os tempos vêm das palavras medidas nos
  brutos.
</p>

<details class="format">
  <summary>Formato do <code>edl.json</code></summary>
  <pre>[
  {"{"}"file": "bruto1.mp4", "in": 12.40, "out": 25.10, "position": 0.00{"}"},
  {"{"}"file": "bruto2.mp4", "in": 3.00, "out": 18.50, "position": 12.70{"}"}
]</pre>
  <p>
    Em segundos e sem mudança de velocidade. <code>in</code> e <code>out</code> são o trecho do bruto; <code>position</code> é onde ele
    começa no vídeo final. O <code>.json</code> de cada bruto (mesmo nome, ao lado dele) precisa existir.
  </p>
</details>

<div class="row top">
  <button class="btn primary" onclick={chooseEdl}>{edl ? "Abrir outro edl.json…" : "Escolher o edl.json…"}</button>
  {#if edl}<span class="path" title={edl.path}>{base(edl.path)}</span>{/if}
</div>

{#if error}<p class="error" role="alert">{error}</p>{/if}

{#if edl}
  {#if edl.errors.length}
    <div class="box err" role="alert">
      <strong>Corrija antes de gerar:</strong>
      <ul>
        {#each edl.errors as e}<li>{e}</li>{/each}
      </ul>
    </div>
  {/if}

  {#if edl.cuts.length}
    <table>
      <thead>
        <tr><th>#</th><th>Arquivo</th><th class="n">in</th><th class="n">out</th><th class="n">No vídeo final</th><th class="n">Palavras</th></tr>
      </thead>
      <tbody>
        {#each edl.cuts as c, i}
          <tr>
            <td class="muted">{i + 1}</td>
            <td class="file" title={c.file}>{c.file}</td>
            <td class="n">{num(c.in)}</td>
            <td class="n">{num(c.out)}</td>
            <td class="n">{clock(c.position)}–{clock(c.position + c.out - c.in)}</td>
            <td class="n">{#if c.words === null}<span class="warn-ink">falta o .json</span>{:else}{c.words}{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if edl.gaps.length}
    <p class="muted small">
      Lacunas entre cortes (títulos, transições), para conferir:
      {edl.gaps.map((g) => `${clock(g.start)}–${clock(g.end)}`).join(", ")}.
    </p>
  {/if}
  {#each edl.warnings as w}<p class="warn small">{w}</p>{/each}

  {#if !edl.errors.length}
    <div class="row actions">
      <button class="btn" disabled={busy || running} onclick={generate}>Gerar legenda</button>
      <button class="btn" disabled={busy || running} onclick={verify}>Verificar com o vídeo final…</button>
    </div>
    <p class="muted small">
      A verificação transcreve o vídeo final exportado (ou reaproveita a transcrição dele), compara com a legenda e, com o vídeo
      escolhido, grava a legenda com o nome dele para o player carregar sozinho.
    </p>
    {#if generated}
      <p class="ok">Legenda gravada: <span class="path" title={generated.srt}>{base(generated.srt)}</span> · {generated.cues} blocos, {generated.words} palavras.</p>
    {/if}
  {/if}
{/if}

{#if check && (mine || running)}
  <section class="panel" aria-live="polite">
    {#if check.kind === "loading" || check.kind === "working"}
      <div class="row">
        <p class="lead-s">{check.kind === "loading" ? "Carregando o modelo de reconhecimento…" : stageLabel(check.stage)}</p>
        <button class="btn subtle" onclick={() => api.edlCancel()}>Cancelar</button>
      </div>
      <div class="progress indeterminate"><span></span></div>
    {:else if check.kind === "failed"}
      <div class="row">
        <p class="error">{check.error}</p>
        <button class="btn subtle" onclick={() => api.edlDismiss()}>Fechar</button>
      </div>
    {:else if check.kind === "done"}
      {@const r = check.check}
      <header class="row">
        <h2>Verificação com {base(check.video)}</h2>
        <button class="btn subtle" onclick={() => api.edlDismiss()}>Fechar</button>
      </header>
      <dl class="stats">
        <div class:good={r.median <= REFERENCIA} class:bad={r.median > REFERENCIA}>
          <dt>Desvio mediano</dt>
          <dd>{num(r.median)} s</dd>
          <dd class="sub">referência: {num(REFERENCIA, 2)} s</dd>
        </div>
        <div>
          <dt>Desvio máximo</dt>
          <dd>{num(r.max)} s</dd>
        </div>
        <div>
          <dt>Palavras casadas</dt>
          <dd>{r.matched}</dd>
        </div>
        <div>
          <dt>Sem par</dt>
          <dd>{r.unmatched_subs} · {r.unmatched_check}</dd>
          <dd class="sub">legenda · vídeo final</dd>
        </div>
      </dl>
      <p class="muted small">
        Legenda gravada em <span class="path" title={check.generated.srt}>{base(check.generated.srt)}</span> ({check.generated.cues} blocos).
      </p>

      {#if r.issues.length === 0}
        <p class="ok">Nenhum trecho com desvio acima de 0,3 s.</p>
      {:else}
        <h3>Trechos com desvio acima de 0,3 s</h3>
        <ul class="issues">
          {#each r.issues as is}
            <li>
              <span class="t">{clock(is.start)}–{clock(is.end)}</span>
              <span>
                <strong>Corte {is.cut + 1}</strong> · {is.words} {is.words === 1 ? "palavra" : "palavras"} · até {signed(is.worst)} s
                <span class="why">{is.why}</span>
              </span>
            </li>
          {/each}
        </ul>
      {/if}

      {#if r.fixes.length}
        <div class="box fix">
          <strong>Correção proposta no edl.json</strong>
          <ul>
            {#each r.fixes as f}
              <li>Corte {f.cut}: <code>{f.field}</code> {num(f.old)} → <strong>{num(f.new)}</strong></li>
            {/each}
          </ul>
          <p class="small">Nada muda sem o seu clique. O arquivo original fica guardado como <code>edl.json.bak</code>.</p>
          <button class="btn primary" disabled={busy} onclick={applyFixes}>Aplicar no edl.json</button>
        </div>
      {/if}
    {/if}
  </section>
{/if}

{#if notice}<p class="ok" role="status">{notice}</p>{/if}

<style>
  .lead {
    max-width: 68ch;
    color: var(--text-2);
  }
  .format {
    margin: 4px 0 16px;
    color: var(--text-2);
    font-size: 13px;
  }
  .format summary {
    cursor: pointer;
  }
  .format pre {
    margin: 8px 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface-2);
    font: 12px var(--mono);
    overflow-x: auto;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }
  .top {
    margin-bottom: 12px;
  }
  .actions {
    margin-top: 16px;
  }
  .path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  table {
    width: 100%;
    margin: 8px 0;
    border-collapse: collapse;
    font-size: 13px;
  }
  th {
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
    color: var(--text-3);
    font-weight: 600;
    text-align: left;
  }
  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
  }
  .n {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .file {
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: var(--text-2);
  }
  .small {
    margin: 6px 0 0;
    font-size: 12.5px;
  }
  .warn {
    color: var(--warn-ink);
  }
  .warn-ink {
    color: var(--warn-ink);
  }
  .error {
    margin: 8px 0;
    flex: 1;
    color: var(--err);
    white-space: pre-line;
  }
  .ok {
    margin: 10px 0 0;
    color: var(--accent);
  }
  .box {
    margin: 12px 0;
    padding: 10px 14px;
    border-radius: var(--radius);
  }
  .box ul {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  .box.err {
    border: 1px solid color-mix(in srgb, var(--err) 40%, var(--line));
    color: var(--err);
  }
  .box.fix {
    border: 1px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .box.fix .btn {
    margin-top: 10px;
  }
  .panel {
    margin: 20px 0;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
  }
  header.row {
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
  }
  h3 {
    margin: 16px 0 6px;
    font-size: 13.5px;
    font-weight: 650;
  }
  .lead-s {
    flex: 1;
    margin: 0;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 8px;
    margin: 12px 0 0;
  }
  .stats div {
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
  }
  .stats dt {
    color: var(--text-3);
    font-size: 12px;
  }
  .stats dd {
    margin: 2px 0 0;
    font-size: 18px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
  }
  .stats dd.sub {
    color: var(--text-3);
    font-size: 11.5px;
    font-weight: 400;
  }
  .stats .good {
    border-color: var(--accent);
  }
  .stats .bad {
    border-color: var(--warn-ink);
  }
  .issues {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .issues li {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 10px;
    padding: 8px 0;
    border-top: 1px solid var(--line);
    font-size: 13px;
  }
  .t {
    color: var(--text-3);
    font: 12px var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .why {
    display: block;
    margin-top: 2px;
    color: var(--text-2);
  }
  .btn.subtle {
    border-color: transparent;
    background: transparent;
    color: var(--text-2);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .progress {
    position: relative;
    height: 6px;
    margin-top: 10px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .progress span {
    position: absolute;
    width: 30%;
    height: 100%;
    background: var(--accent);
    animation: slide 1.4s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      left: -30%;
    }
    to {
      left: 100%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .progress span {
      animation: none;
      width: 100%;
      opacity: 0.5;
    }
  }
  @media (max-width: 560px) {
    .issues li {
      grid-template-columns: 1fr;
      gap: 2px;
    }
  }
</style>
