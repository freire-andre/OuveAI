<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open as pick } from "@tauri-apps/plugin-dialog";
  import { api, watchFfmpeg, watchQueue, NEED_FFMPEG, type Phase, type QueueView, type SessionRow } from "../lib/api";
  import { clock, duration, when } from "../lib/format";
  import FfmpegCard from "../components/FfmpegCard.svelte";

  let { onOpen }: { onOpen: (id: number) => void } = $props();

  const EXT = ["mp4", "mkv", "mov", "avi", "webm", "wmv", "m4v", "mp3", "m4a", "wav", "ogg", "opus", "flac", "aac", "wma"];

  let queue = $state<QueueView>({ current: null, pending: [], finished: [] });
  let sessions = $state<SessionRow[]>([]);
  let dragging = $state(false);
  let error = $state<string | null>(null);
  /** Arquivos esperando o FFmpeg ficar pronto (pedido só no 1º "Importar arquivo"). */
  let waitingFfmpeg = $state<string[]>([]);

  const refresh = () => api.fileSessions().then((s) => (sessions = s));

  async function enqueue(paths: string[]) {
    error = null;
    const media = paths.filter((p) => EXT.includes(p.split(".").pop()?.toLowerCase() ?? ""));
    if (media.length < paths.length) error = "Alguns itens não são áudio nem vídeo e ficaram de fora.";
    if (!media.length) return;
    try {
      await api.fileEnqueue(media);
    } catch (e) {
      if (String(e) === NEED_FFMPEG) waitingFfmpeg = [...waitingFfmpeg, ...media];
      else error = String(e);
    }
  }

  async function choose() {
    const r = await pick({ multiple: true, title: "Arquivos de áudio ou vídeo", filters: [{ name: "Áudio e vídeo", extensions: EXT }] });
    if (Array.isArray(r)) await enqueue(r);
    else if (typeof r === "string") await enqueue([r]);
  }

  onMount(() => {
    refresh();
    const offs = [
      watchQueue((q) => (queue = q)),
      listen("file-done", refresh),
      getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") dragging = true;
        else if (p.type === "leave") dragging = false;
        else if (p.type === "drop") {
          dragging = false;
          enqueue(p.paths);
        }
      }),
      // Quando o FFmpeg fica pronto, segue com os arquivos que estavam esperando.
      watchFfmpeg((f) => {
        if (f.state === "ready" && waitingFfmpeg.length) {
          const list = waitingFfmpeg;
          waitingFfmpeg = [];
          enqueue(list);
        }
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  });

  function phaseLabel(p: Phase): string {
    if (p.kind === "waiting") return "Aguardando a reunião terminar (a gravação tem prioridade)";
    if (p.kind === "waitingcompare") return "Aguardando a comparação com o Whisper ou a verificação de legenda terminar";
    if (p.kind === "loading") return "Carregando o modelo de reconhecimento…";
    const s = p.stage;
    switch (s.stage) {
      case "converting":
        return "Convertendo o áudio…";
      case "analyzing":
        return "Medindo os silêncios…";
      case "recognizing":
        return s.chunks > 1 ? `Reconhecendo a fala · parte ${s.chunk} de ${s.chunks} (a partir de ${clock(s.done)})` : "Reconhecendo a fala…";
      case "saving":
        return "Salvando…";
    }
  }

  function pct(p: Phase): number | null {
    if (p.kind !== "working" || p.stage.stage !== "recognizing" || p.stage.chunks < 2) return null;
    return Math.round(((p.stage.chunk - 1) / p.stage.chunks) * 100);
  }
</script>

<h1>Arquivos</h1>
<p class="lead">
  Transcreva vídeos e áudios gravados. O texto, a legenda <code>.srt</code> e o tempo de cada palavra (<code>.json</code>) ficam
  ao lado do arquivo, com o mesmo nome. Tudo roda neste computador.
</p>

<button class="drop" class:dragging onclick={choose}>
  <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
    <path d="M12 16V4m0 0-4 4m4-4 4 4M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" stroke-linecap="round" stroke-linejoin="round" />
  </svg>
  <span class="drop-title">{dragging ? "Solte para transcrever" : "Arraste arquivos aqui ou clique para escolher"}</span>
  <span class="drop-sub">MP4, MKV, MOV, MP3, M4A, WAV e outros formatos de áudio e vídeo</span>
</button>

{#if error}<p class="error">{error}</p>{/if}

{#if waitingFfmpeg.length}
  <div class="need">
    <p>
      Para ler {waitingFfmpeg.length === 1 ? "este arquivo" : `estes ${waitingFfmpeg.length} arquivos`} o OuveAI precisa do FFmpeg. A
      transcrição começa sozinha quando ele estiver pronto.
    </p>
    <FfmpegCard />
    <button class="btn subtle" onclick={() => (waitingFfmpeg = [])}>Desistir</button>
  </div>
{/if}

{#if queue.current || queue.pending.length || queue.finished.length}
  <ul class="queue">
    {#if queue.current}
      {@const [item, phase] = queue.current}
      {@const p = pct(phase)}
      <li class="busy">
        <div class="row">
          <span class="name">{item.name}</span>
          <button class="btn subtle" onclick={() => api.fileCancel(item.id)}>Cancelar</button>
        </div>
        <div class="progress" class:indeterminate={p === null}><span style={p === null ? "" : `width: ${p}%`}></span></div>
        <p class="status">{phaseLabel(phase)}</p>
      </li>
    {/if}
    {#each queue.pending as item (item.id)}
      <li>
        <div class="row">
          <span class="name">{item.name}</span>
          <span class="muted">na fila</span>
          <button class="btn subtle" onclick={() => api.fileCancel(item.id)}>Tirar da fila</button>
        </div>
      </li>
    {/each}
    {#each queue.finished as f (f.item.id)}
      <li class:failed={!!f.error}>
        <div class="row">
          <span class="name">{f.item.name}</span>
          {#if f.session_id !== null}
            <button class="btn primary" onclick={() => onOpen(f.session_id!)}>Abrir</button>
          {/if}
          <button class="btn subtle" aria-label="Dispensar" onclick={() => api.fileDismiss(f.item.id)}>×</button>
        </div>
        {#if f.error}<p class="status err">{f.error === "cancelado" ? "Cancelado." : f.error}</p>{/if}
        {#if f.warning}<p class="status warn">{f.warning}</p>{/if}
        {#if f.session_id !== null && !f.warning}<p class="status ok">Pronto. Os arquivos estão ao lado do original.</p>{/if}
      </li>
    {/each}
  </ul>
{/if}

<h2>Transcritos</h2>
{#if sessions.length === 0}
  <p class="muted">Nenhum arquivo transcrito ainda.</p>
{:else}
  <ul class="list">
    {#each sessions as s (s.id)}
      <li>
        <button class="item" onclick={() => onOpen(s.id)}>
          <span class="name">{s.title}</span>
          <span class="muted">{s.duration_s ? duration(s.duration_s) : ""} · {s.words} palavras · {when(s.started_at)}</span>
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  h1 {
    margin: 0 0 8px;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  h2 {
    margin: 28px 0 10px;
    font-size: 14px;
    font-weight: 650;
  }
  .lead {
    max-width: 640px;
    margin: 0 0 18px;
    color: var(--text-2);
  }
  code {
    font: 12.5px var(--mono);
  }
  .drop {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 100%;
    max-width: 640px;
    padding: 28px 20px;
    border: 1.5px dashed var(--line);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-2);
    transition:
      border-color 0.15s,
      background 0.15s;
  }
  .drop:hover,
  .drop.dragging {
    border-color: var(--accent);
    color: var(--accent);
  }
  .drop.dragging {
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
  }
  .drop-title {
    color: var(--text);
    font-weight: 600;
  }
  .drop-sub {
    font-size: 12.5px;
  }
  .error {
    color: var(--err);
    font-size: 13px;
  }
  .need {
    max-width: 640px;
    margin-top: 16px;
    padding: 14px 16px;
    border-radius: var(--radius);
    background: var(--warn-bg);
  }
  .need > p {
    margin: 0 0 12px;
    color: var(--warn-ink);
  }
  .need .btn {
    margin-top: 8px;
  }
  .queue,
  .list {
    max-width: 640px;
    margin: 16px 0 0;
    padding: 0;
    list-style: none;
  }
  .queue li {
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .queue li + li {
    margin-top: 8px;
  }
  .queue li.failed {
    border-color: color-mix(in srgb, var(--err) 40%, var(--line));
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .muted {
    color: var(--text-2);
    font-size: 13px;
  }
  .status {
    margin: 6px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .status.err {
    color: var(--err);
  }
  .status.warn {
    color: var(--warn-ink);
  }
  .status.ok {
    color: var(--accent);
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
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  .progress.indeterminate span {
    position: absolute;
    width: 30%;
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
    .progress.indeterminate span {
      animation: none;
      width: 100%;
      opacity: 0.5;
    }
  }
  .btn.subtle {
    border-color: transparent;
    background: transparent;
    color: var(--text-2);
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
  .item:hover .name {
    color: var(--accent);
  }
  .item .name {
    max-width: 100%;
  }
</style>
