<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, meter, watchLive, watchStatus, type ChannelView, type LiveView, type Status, type Tick } from "../lib/api";
  import { clock } from "../lib/format";

  let status = $state<Status | null>(null);
  let live = $state<LiveView | null>(null);
  let t = $state<Tick>({ elapsed: 0, eu: -120, outros: -120 });
  let error = $state<string | null>(null);
  let feed = $state<HTMLElement | null>(null);

  onMount(() => {
    const offs = [
      watchStatus((s) => (status = s)),
      watchLive(async (v) => {
        live = v;
        if (v.state === "recording") error = null;
        await tick();
        feed?.scrollTo({ top: feed.scrollHeight });
      }),
      listen<Tick>("live-tick", (e) => (t = e.payload)),
      listen<string>("live-error", (e) => (error = e.payload)),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  });

  const active = $derived(live?.state === "recording" || live?.state === "paused");
  const paused = $derived(live?.state === "paused");
  const shown = $derived((live?.segments ?? []).filter((s) => !s.echo).slice(-6));

  function chanNote(c: ChannelView, who: "EU" | "OUTROS"): string {
    switch (c.state) {
      case "opening":
        return "abrindo…";
      case "waiting":
        return "aguardando o Windows liberar o áudio (antivírus?)";
      case "stuck":
        return "não abriu há 1 min";
      case "bt_idle":
        return "mic Bluetooth sem uso: não abri para não derrubar o som do fone";
      case "off":
        return "desligado";
      case "app_missing":
        return `${c.device ?? "o app"} não está aberto`;
      case "error":
        return c.error ?? "erro";
      default:
        return c.device ?? (who === "EU" ? "microfone" : "saída de som");
    }
  }
</script>

<div class="mini">
  <header data-tauri-drag-region>
    {#if live?.state === "finishing"}
      <span class="idle" data-tauri-drag-region>Salvando…</span>
    {:else if active}
      <span class="rec" class:paused data-tauri-drag-region>
        <span class="dot"></span>{paused ? "PAUSA" : "REC"} {clock(t.elapsed)}
      </span>
    {:else}
      <span class="idle" data-tauri-drag-region>Pronto</span>
    {/if}
    <div class="tools">
      {#if active}
        <button title="Marcar momento" aria-label="Marcar momento" onclick={() => api.mark()}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z" /></svg>
          {#if live?.marks}<small>{live.marks}</small>{/if}
        </button>
        <button
          title={paused ? "Retomar" : "Pausar (solta o microfone)"}
          aria-label={paused ? "Retomar" : "Pausar"}
          onclick={() => (paused ? api.liveResume() : api.livePause())}
        >
          {#if paused}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5 3.5l7 4.5-7 4.5z" class="fill" /></svg>
          {:else}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5.5 3.5v9M10.5 3.5v9" /></svg>
          {/if}
        </button>
      {/if}
      <button
        title={active ? "Parar e salvar" : "Gravar"}
        aria-label={active ? "Parar e salvar" : "Gravar"}
        class:recbtn={!active}
        disabled={live?.state === "finishing"}
        onclick={() => api.toggleRecord()}
      >
        {#if active}
          <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="4" y="4" width="8" height="8" rx="1.5" class="fill" /></svg>
        {:else}
          <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="4.5" class="fill" /></svg>
        {/if}
      </button>
      {#if status?.stealth_supported}
        <button
          title={status.stealth ? "Invisível no compartilhamento de tela (clique para mostrar)" : "Visível no compartilhamento de tela (clique para ocultar)"}
          aria-label="Ocultar do compartilhamento de tela"
          aria-pressed={status.stealth}
          class:on={status.stealth}
          onclick={() => status && api.setStealth(!status.stealth)}
        >
          {#if status.stealth}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4z" /><path d="M3 13L13 3" /></svg>
          {:else}
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 8s2.2-4 6-4 6 4 6 4-2.2 4-6 4-6-4-6-4z" /><circle cx="8" cy="8" r="1.8" /></svg>
          {/if}
        </button>
      {/if}
      <button title="Abrir o OuveAI" aria-label="Abrir o OuveAI" onclick={() => api.openMain()}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 3h7v7M13 3L4 12" /></svg>
      </button>
      <button title="Ocultar (continua gravando)" aria-label="Ocultar" onclick={() => api.hideMini()}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
    </div>
  </header>

  {#if active && live}
    <div class="lanes">
      {#each [["EU", live.eu, t.eu], ["OUTROS", live.outros, t.outros]] as const as [who, c, db] (who)}
        <div class="lane" class:dim={paused || c.state !== "open"}>
          <span class="who" class:eu={who === "EU"} class:outros={who === "OUTROS"}>{who}</span>
          <span class="meter" class:eu={who === "EU"} class:outros={who === "OUTROS"}>
            <span style="width: {paused ? 0 : meter(db)}%"></span>
          </span>
          <span class="dev" title={chanNote(c, who)}>{chanNote(c, who)}</span>
        </div>
      {/each}
    </div>

    {#if live.mic_denied}
      <p class="alert">O Windows está bloqueando o microfone. Libere em Configurações › Privacidade › Microfone.</p>
    {:else if live.mic_elsewhere}
      <p class="alert">
        O {live.mic_elsewhere.app} usa outro mic: <b>{live.mic_elsewhere.device}</b>
        <button class="link" onclick={() => live?.mic_elsewhere && api.liveUseMic(live.mic_elsewhere.device_id)}>Usar este mic</button>
      </p>
    {:else if live.eu.state === "stuck" || live.outros.state === "stuck"}
      <p class="alert">O áudio não abriu. <button class="link" onclick={() => api.liveRetry()}>Tentar de novo</button></p>
    {:else if live.outros_silent && !paused}
      <p class="hint">
        OUTROS sem som há 1 min. A reunião está tocando em outro dispositivo?
        <button class="link" onclick={() => api.openMain("settings")}>Escolher saída</button>
      </p>
    {:else if live.model === "loading"}
      <p class="hint">Carregando o modelo… já estou gravando, o texto aparece em instantes.</p>
    {:else if live.model === "error"}
      <p class="alert">O modelo não carregou: {live.model_error}. O áudio continua sendo capturado.</p>
    {:else if live.notice}
      <p class="hint">{live.notice}</p>
    {/if}

    <div class="feed" bind:this={feed} aria-live="polite">
      {#each shown as s (s.id)}
        <p><span class="tag" class:eu={s.ch === "EU"} class:outros={s.ch === "OUTROS"}>{s.ch === "EU" ? "EU" : "OUT"}</span>{s.text}</p>
      {:else}
        <p class="placeholder">{paused ? "Pausado. O microfone está livre." : "As falas aparecem aqui."}</p>
      {/each}
    </div>
  {:else}
    <div class="feed">
      {#if error}
        <p class="alert">{error}</p>
      {:else}
        <p class="placeholder">Clique em gravar (ou use o atalho) quando a reunião começar.</p>
      {/if}
    </div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  .mini {
    display: flex;
    flex-direction: column;
    height: 100vh;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
    overflow: hidden;
    user-select: none;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 6px 6px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--surface-2);
    cursor: grab;
  }
  .rec {
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--rec);
    font: 600 12.5px var(--mono);
  }
  .rec.paused {
    color: var(--text-2);
  }
  .idle {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: currentColor;
  }
  .tools {
    display: flex;
    gap: 2px;
  }
  .tools button {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    min-width: 28px;
    height: 26px;
    padding: 0 6px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-2);
    font-size: 13px;
    justify-content: center;
  }
  .tools button:hover {
    background: var(--surface);
    color: var(--text);
  }
  .tools button.on {
    color: var(--text);
  }
  .tools svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .tools svg .fill {
    fill: currentColor;
    stroke: none;
  }
  .tools .recbtn {
    color: var(--rec);
  }
  .tools small {
    font-size: 10px;
  }
  .lanes {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 9px 12px 6px;
  }
  .lane {
    display: grid;
    grid-template-columns: 50px 70px 1fr;
    align-items: center;
    gap: 8px;
  }
  .lane.dim {
    opacity: 0.6;
  }
  .who {
    font: 600 11px var(--mono);
    letter-spacing: 0.04em;
  }
  .eu {
    color: var(--eu);
  }
  .outros {
    color: var(--outros);
  }
  .meter {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .meter span {
    display: block;
    height: 100%;
    background: currentColor;
    transition: width 0.2s linear;
  }
  .dev {
    overflow: hidden;
    color: var(--text-3);
    font-size: 11.5px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .alert,
  .hint {
    margin: 2px 12px 4px;
    font-size: 12px;
    line-height: 1.35;
  }
  .alert {
    color: var(--rec);
  }
  .hint {
    color: var(--text-2);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent, var(--text));
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .feed {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 12px 10px;
    border-top: 1px solid var(--line);
  }
  .feed p {
    margin: 6px 0 0;
    font-size: 12.5px;
    line-height: 1.4;
    user-select: text;
  }
  .tag {
    display: inline-block;
    min-width: 30px;
    margin-right: 6px;
    font: 600 10.5px var(--mono);
  }
  .placeholder {
    color: var(--text-3);
  }
</style>
