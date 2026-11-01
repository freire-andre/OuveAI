<script lang="ts">
  import { onMount } from "svelte";
  import { api, watchFfmpeg, type FfmpegView } from "../lib/api";
  import { eta, mb, speed } from "../lib/format";

  let f = $state<FfmpegView | null>(null);
  let confirming = $state(false);
  let actionError = $state<string | null>(null);

  onMount(() => {
    const off = watchFfmpeg((v) => (f = v));
    return () => off.then((fn) => fn());
  });

  const busy = $derived(f != null && ["downloading", "verifying", "extracting"].includes(f.state));
  const pct = $derived(f?.step && f.step.total > 0 ? Math.min(100, (f.step.done / f.step.total) * 100) : 0);
  const retry = $derived(f?.step?.step === "downloading" ? f.step.retry : null);

  async function run(p: Promise<unknown>) {
    actionError = null;
    try {
      await p;
    } catch (e) {
      actionError = String(e);
    }
  }

  function confirm() {
    confirming = false;
    run(api.ffmpegDownload());
  }
</script>

{#if f}
  <article class="card" class:ready={f.state === "ready"}>
    <header>
      <div>
        <h3>FFmpeg</h3>
        <p class="meta">Usado só para importar arquivos de áudio e vídeo. A transcrição de reuniões não precisa dele.</p>
      </div>
      <span class="badge {f.state}">
        {f.state === "ready" ? "Pronto" : busy ? "Baixando" : f.state === "paused" ? "Pausado" : "Não encontrado"}
      </span>
    </header>

    {#if f.found}
      <p class="status">
        {f.found.version.replace(/ Copyright.*$/, "")}
        · {f.found.origin === "path" ? "instalado no Windows (PATH)" : "baixado pelo OuveAI"}
      </p>
    {:else if busy}
      <div class="progress" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin="0" aria-valuemax="100">
        <span style="width: {pct}%"></span>
      </div>
      <p class="status">
        {#if retry}
          <span class="warn">{retry.status === 0 ? "Sem conexão com o servidor" : `O servidor respondeu ${retry.status}`}</span>
          · tentativa {retry.attempt} de {retry.max} · tentando de novo em {retry.wait_secs} s
        {:else if f.step?.step === "downloading"}
          {mb(f.step.done)} de {mb(f.step.total)} · {speed(f.step.bytes_per_sec)}
          {#if eta(f.step.total - f.step.done, f.step.bytes_per_sec)}· faltam {eta(f.step.total - f.step.done, f.step.bytes_per_sec)}{/if}
        {:else if f.step?.step === "verifying"}
          Conferindo a integridade (SHA-256)… {Math.round(pct)}%
        {:else if f.step?.step === "extracting"}
          Extraindo o ffmpeg.exe… {Math.round(pct)}%
        {:else}
          Conectando…
        {/if}
      </p>
    {:else if f.state === "paused"}
      <p class="status">{mb(f.partial)} já baixados. O download continua de onde parou.</p>
    {/if}

    {#if f.error}<p class="error">{f.error}</p>{/if}
    {#if actionError}<p class="error">{actionError}</p>{/if}

    {#if !f.found}
      {#if busy}
        <div class="actions">
          <button class="btn" onclick={() => run(api.ffmpegPause())}>{f.state === "downloading" ? "Pausar" : "Cancelar"}</button>
        </div>
      {:else if confirming}
        <!-- Regra do projeto: todo download é confirmado, dizendo o que é, de onde vem e o tamanho. -->
        <div class="confirm">
          <p>Baixar o FFmpeg {f.package.version} (build estática, licença {f.package.license})?</p>
          <dl>
            <dt>De onde</dt>
            <dd>{f.package.source}</dd>
            <dt>Tamanho</dt>
            <dd>{mb(f.package.size)} (conferido por SHA-256)</dd>
            <dt>Para onde</dt>
            <dd>a pasta de dados do OuveAI; nada é instalado no Windows</dd>
          </dl>
          <div class="actions">
            <button class="btn primary" onclick={confirm} {@attach (el) => el.focus()}>Baixar</button>
            <button class="btn" onclick={() => (confirming = false)}>Cancelar</button>
          </div>
        </div>
      {:else}
        <div class="actions">
          <button class="btn primary" onclick={() => (confirming = true)}>
            {f.state === "paused" ? "Continuar download" : "Baixar FFmpeg…"}
          </button>
        </div>
        <p class="hint">Se preferir, instale pelo Windows (<code>winget install Gyan.FFmpeg</code>) e reabra esta tela.</p>
      {/if}
    {/if}
  </article>
{/if}

<style>
  .card {
    padding: 18px 20px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
  }
  .meta {
    margin: 2px 0 0;
    color: var(--text-2);
    font-size: 12.5px;
  }
  .badge {
    flex: none;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 12px;
    font-weight: 600;
  }
  .badge.ready {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
  }
  .progress {
    height: 6px;
    margin: 16px 0 8px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.25s linear;
  }
  .status {
    margin: 12px 0 0;
    color: var(--text-2);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .progress + .status {
    margin-top: 0;
  }
  .warn {
    color: var(--warn-ink);
  }
  .error {
    margin: 12px 0 0;
    color: var(--err);
    font-size: 13px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 14px;
  }
  .confirm {
    margin-top: 14px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--bg);
    font-size: 13px;
  }
  .confirm p {
    margin: 0 0 8px;
    font-weight: 600;
  }
  dl {
    display: grid;
    grid-template-columns: 80px 1fr;
    gap: 4px 10px;
    margin: 0;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
  }
  .hint {
    margin: 10px 0 0;
    color: var(--text-2);
    font-size: 12.5px;
  }
  code {
    font: 12px var(--mono);
  }
</style>
