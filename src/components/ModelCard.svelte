<script lang="ts">
  import { open as pick } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type ModelView } from "../lib/api";
  import { eta, mb, speed } from "../lib/format";

  let { model }: { model: ModelView } = $props();

  let browserHint = $state(false);
  let pauseBtn = $state<HTMLButtonElement | null>(null);
  // "Baixar" some ao começar; sem isso o foco do teclado se perde.
  let focusPause = false;
  $effect(() => {
    if (pauseBtn && focusPause) {
      focusPause = false;
      pauseBtn.focus();
    }
  });
  let actionError = $state<string | null>(null);

  const retrying = $derived(model.step?.step === "downloading" && model.step.retry != null);
  const busy = $derived(["downloading", "verifying", "extracting"].includes(model.state));
  const pct = $derived(model.step && model.step.total > 0 ? Math.min(100, (model.step.done / model.step.total) * 100) : 0);
  const stateLabel: Record<ModelView["state"], string> = {
    absent: "Não instalado",
    downloading: "Baixando",
    paused: "Pausado",
    verifying: "Conferindo",
    extracting: "Extraindo",
    ready: "Pronto",
    corrupt: "Com problema",
  };

  async function run(p: Promise<unknown>) {
    actionError = null;
    try {
      await p;
    } catch (e) {
      actionError = String(e);
    }
  }

  async function importFrom(directory: boolean) {
    const path = await pick({
      directory,
      multiple: false,
      title: directory ? "Pasta com os arquivos do modelo" : "Pacote do modelo (.tar.bz2)",
      filters: directory ? undefined : [{ name: "Pacote do modelo", extensions: ["bz2"] }],
    });
    if (typeof path === "string") await run(api.modelImport(model.id, path));
  }

  async function viaBrowser() {
    if (!model.browser_url) return;
    await openUrl(model.browser_url);
    browserHint = true;
  }
</script>

<article class="card" class:ready={model.state === "ready" && !model.unused}>
  <header>
    <div>
      <h3>{model.name}</h3>
      <p class="meta">{mb(model.size)} · licença {model.license}</p>
      {#if model.note}<p class="meta note">{model.note}</p>{/if}
    </div>
    {#if model.unused}
      <span class="badge unused">Não está em uso</span>
    {:else}
      <span class="badge {model.state}">{model.embedded && model.state === "ready" ? "Vem com o app" : stateLabel[model.state]}</span>
    {/if}
  </header>

  {#if busy}
    <div class="progress" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin="0" aria-valuemax="100">
      <span style="width: {pct}%"></span>
    </div>
    <p class="status">
      {#if model.step?.step === "downloading" && model.step.retry}
        {@const r = model.step.retry}
        <span class="warn">{r.status === 0 ? "Sem conexão com o servidor" : `O servidor respondeu ${r.status}`}</span>
        · tentativa {r.attempt} de {r.max} · tentando de novo em {r.wait_secs} s
        {#if r.status === 0}<br /><span class="sub">Confira a internet. Na rede da empresa, pause e tente o espelho ou use as opções abaixo.</span>{/if}
      {:else if model.step?.step === "downloading"}
        {mb(model.step.done)} de {mb(model.step.total)} · {speed(model.step.bytes_per_sec)}
        {#if eta(model.step.total - model.step.done, model.step.bytes_per_sec)}· faltam {eta(model.step.total - model.step.done, model.step.bytes_per_sec)}{/if}
      {:else if model.step?.step === "verifying"}
        Conferindo a integridade (SHA-256)… {Math.round(pct)}%
      {:else if model.step?.step === "extracting"}
        Extraindo… {Math.round(pct)}%
      {:else}
        Conectando…
      {/if}
    </p>
  {:else if model.state === "paused"}
    <p class="status">{mb(model.partial)} já baixados. O download continua de onde parou.</p>
  {:else if model.state === "corrupt"}
    <p class="status warn">A pasta do modelo está incompleta ({model.detail}). Baixe ou importe de novo.</p>
  {/if}

  {#if model.error}
    <p class="error">{model.error}</p>
  {/if}
  {#if actionError}
    <p class="error">{actionError}</p>
  {/if}

  {#if !model.embedded}
    <div class="actions">
      {#if busy}
        <button class="btn" bind:this={pauseBtn} onclick={() => run(api.modelPause(model.id))}>
          {model.state === "downloading" ? "Pausar" : "Cancelar"}
        </button>
      {:else if model.state === "ready"}
        <button class="btn subtle" onclick={() => run(api.modelRemove(model.id))}>Apagar modelo</button>
      {:else}
        <button class="btn primary" onclick={() => { focusPause = true; run(api.modelDownload(model.id, "primary")); }}>
          {model.state === "paused" ? "Continuar download" : "Baixar"}
        </button>
        {#if model.has_mirror}
          <button class="btn" onclick={() => { focusPause = true; run(api.modelDownload(model.id, "mirror")); }}>Baixar do espelho</button>
        {/if}
      {/if}
    </div>

    {#if model.state !== "ready" && (!busy || retrying)}
      <!-- Plano B: sempre visível quando o modelo não está instalado. -->
      <div class="planb">
        <p class="planb-title">Sem acesso direto pela rede da empresa?</p>
        <div class="actions">
          {#if model.browser_url}
            <button class="btn" onclick={viaBrowser}>Baixar pelo navegador</button>
          {/if}
          <button class="btn" onclick={() => importFrom(false)}>Importar arquivo…</button>
          <button class="btn" onclick={() => importFrom(true)}>Importar pasta…</button>
        </div>
        {#if browserHint}
          <p class="hint">
            O download abriu no navegador. Quando terminar, clique em <strong>Importar arquivo…</strong> e escolha o arquivo
            <code>.tar.bz2</code> (normalmente na pasta Downloads). A integridade é conferida do mesmo jeito.
          </p>
        {/if}
      </div>
    {/if}
  {/if}
</article>

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
  .note {
    margin-top: 6px;
    max-width: 460px;
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
  .badge.corrupt {
    background: var(--warn-bg);
    color: var(--warn-ink);
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
  .sub {
    font-size: 12.5px;
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
  .btn.subtle {
    border-color: transparent;
    background: transparent;
    color: var(--text-2);
  }
  .planb {
    margin-top: 16px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .planb-title {
    margin: 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .planb .actions {
    margin-top: 8px;
  }
  .hint {
    margin: 10px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }
  code {
    font: 12px var(--mono);
  }
</style>
