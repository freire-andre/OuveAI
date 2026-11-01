<script lang="ts">
  import { onMount } from "svelte";
  import { api, watchCompare, watchModels, NEED_WHISPER, WHISPER_ID, type CompareView, type ModelView } from "../lib/api";
  import { clock, duration } from "../lib/format";
  import ModelCard from "./ModelCard.svelte";

  let { sessionId, onApplied }: { sessionId: number; onApplied: () => void } = $props();

  let view = $state<CompareView | null>(null);
  let whisper = $state<ModelView | null>(null);
  /** Pediu para comparar sem o Whisper instalado: mostra o cartão para baixar. */
  let needModel = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<{ ok: boolean; text: string } | null>(null);
  /** Trechos marcados para usar o texto do Whisper. */
  let chosen = $state<Set<number>>(new Set());
  let applying = $state(false);

  onMount(() => {
    const offs = [
      watchCompare((v) => {
        if (v?.kind === "done" && view?.kind !== "done") chosen = new Set();
        view = v;
      }),
      watchModels((m) => (whisper = m.find((x) => x.id === WHISPER_ID) ?? null)),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  });

  const mine = $derived(view !== null && view.session_id === sessionId);
  const elsewhere = $derived(
    view !== null && !mine && (view.kind === "loading" || view.kind === "converting" || view.kind === "comparing" || view.kind === "cancelling"),
  );

  /** Chamado pelo botão "Comparar com o Whisper" da tela. */
  export async function start() {
    error = null;
    notice = null;
    try {
      await api.compareStart(sessionId);
      needModel = false;
    } catch (e) {
      if (String(e) === NEED_WHISPER) needModel = true;
      else error = String(e);
    }
  }

  function toggle(i: number, useWhisper: boolean) {
    const next = new Set(chosen);
    if (useWhisper) next.add(i);
    else next.delete(i);
    chosen = next;
  }

  async function applyChoices() {
    applying = true;
    error = null;
    try {
      const n = chosen.size;
      const warning = await api.compareApply(sessionId, [...chosen].sort((a, b) => a - b));
      notice = warning ? { ok: false, text: warning } : { ok: true, text: `${n} ${n === 1 ? "trecho trocado" : "trechos trocados"} pelo texto do Whisper.` };
      onApplied();
    } catch (e) {
      error = String(e);
    } finally {
      applying = false;
    }
  }

  const pct = $derived(mine && view?.kind === "comparing" && view.total > 0 ? (100 * view.done) / view.total : 0);
</script>

{#if needModel && whisper && whisper.state !== "ready"}
  <section class="panel">
    <p class="lead">
      Para comparar, baixe o Whisper ({whisper.size > 0 ? `${Math.round(whisper.size / 1048576)} MB` : "cerca de 1 GB"}). Ele é
      opcional e só é usado aqui.
    </p>
    <ModelCard model={whisper} />
  </section>
{:else if needModel && whisper?.state === "ready"}
  <section class="panel row">
    <p class="lead">O Whisper está pronto.</p>
    <button class="btn primary" onclick={start}>Comparar agora</button>
    <button class="btn subtle" onclick={() => (needModel = false)}>Fechar</button>
  </section>
{/if}

{#if elsewhere}
  <p class="muted">Há uma comparação em andamento em outra sessão.</p>
{/if}

{#if mine && view}
  <section class="panel" aria-live="polite">
    {#if view.kind === "cancelling"}
      <p class="lead">Cancelando… o trecho em andamento termina em alguns segundos.</p>
    {:else if view.kind === "loading" || view.kind === "converting" || view.kind === "comparing"}
      <div class="row">
        <p class="lead">
          {#if view.kind === "loading"}Carregando o Whisper…
          {:else if view.kind === "converting"}Lendo o áudio original…
          {:else}Comparando com o Whisper… trecho {Math.min(view.done + 1, view.total)} de {view.total}{/if}
        </p>
        <button class="btn subtle" onclick={() => api.compareCancel()}>Cancelar</button>
      </div>
      <div class="progress" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin="0" aria-valuemax="100">
        <span style="width: {pct}%"></span>
      </div>
      <p class="muted small">O Whisper é mais lento que o Parakeet: leva cerca de metade da duração do áudio.</p>
    {:else if view.kind === "failed"}
      <div class="row">
        <p class="error">{view.error}</p>
        <button class="btn subtle" onclick={() => api.compareDismiss()}>Fechar</button>
      </div>
    {:else if view.kind === "done"}
      {@const r = view.result}
      <header class="row">
        <div>
          <h2>Comparação com o Whisper</h2>
          <p class="muted small">
            {duration(r.audio_s)} de áudio em {duration(r.whisper_s)} ·
            {r.divergences.length === 0
              ? "os dois modelos concordam em tudo"
              : `${r.divergences.length} ${r.divergences.length === 1 ? "trecho diferente" : "trechos diferentes"}`}
          </p>
        </div>
        {#if r.divergences.length > 0}
          <div class="actions">
            <button class="btn subtle" onclick={() => (chosen = new Set(r.divergences.map((_, i) => i)))}>Whisper em todos</button>
            <button class="btn subtle" onclick={() => (chosen = new Set())}>Manter todos</button>
          </div>
        {/if}
      </header>
      {#if r.unreliable.length > 0}
        <p class="warn small">
          O Whisper se perdeu em {r.unreliable.length === 1 ? "1 trecho" : `${r.unreliable.length} trechos`}
          ({r.unreliable.map(([a, b]) => `${clock(a)}–${clock(b)}`).join(", ")}), {r.unreliable.length === 1 ? "que ficou" : "que ficaram"} de fora.
        </p>
      {/if}
      {#if r.divergences.length > 0}
        <p class="muted small">Escolha o texto de cada trecho. Os tempos continuam os do Parakeet.</p>
        <ol class="list">
          <li class="head" aria-hidden="true"><span></span><span>Parakeet (atual)</span><span>Whisper</span></li>
          {#each r.divergences as d, i (i)}
            {@const w = chosen.has(i)}
            <li>
              <span class="t">{clock(d.start)}</span>
              <button class="opt" class:on={!w} aria-pressed={!w} onclick={() => toggle(i, false)}>{d.parakeet}</button>
              <button class="opt" class:on={w} aria-pressed={w} onclick={() => toggle(i, true)}>
                {#if d.whisper}{d.whisper}{:else}<em>(apagar)</em>{/if}
              </button>
            </li>
          {/each}
        </ol>
      {/if}
      <div class="row foot">
        {#if r.divergences.length > 0}
          <button class="btn primary" disabled={chosen.size === 0 || applying} onclick={applyChoices}>
            {chosen.size === 0 ? "Aplicar" : `Aplicar ${chosen.size} ${chosen.size === 1 ? "troca" : "trocas"}`}
          </button>
        {/if}
        <button class="btn subtle" onclick={() => api.compareDismiss()}>{r.divergences.length > 0 ? "Descartar" : "Fechar"}</button>
      </div>
    {/if}
  </section>
{/if}

{#if error}<p class="error">{error}</p>{/if}
{#if notice}<p class={notice.ok ? "ok" : "warn"}>{notice.text}</p>{/if}

<style>
  .panel {
    margin: 16px 0;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }
  header.row {
    justify-content: space-between;
    align-items: flex-start;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
  }
  .lead {
    margin: 0;
    flex: 1;
  }
  .muted {
    color: var(--text-2);
  }
  .small {
    margin: 4px 0 0;
    font-size: 12.5px;
  }
  .warn {
    margin: 10px 0 0;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--warn-bg);
    color: var(--warn-ink);
  }
  .error {
    margin: 0;
    flex: 1;
    color: var(--err);
  }
  .ok {
    color: var(--accent);
  }
  .actions {
    display: flex;
    gap: 6px;
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
    height: 6px;
    margin: 12px 0 8px;
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
  .list {
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
    max-height: 420px;
    overflow-y: auto;
  }
  .list li {
    display: grid;
    grid-template-columns: 56px 1fr 1fr;
    gap: 8px;
    align-items: stretch;
    padding: 4px 0;
  }
  .list li.head {
    position: sticky;
    top: 0;
    background: var(--surface);
    color: var(--text-3);
    font-size: 12px;
    font-weight: 600;
  }
  .t {
    padding-top: 7px;
    color: var(--text-3);
    font: 12px var(--mono);
    font-variant-numeric: tabular-nums;
  }
  .opt {
    padding: 6px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-2);
    text-align: left;
    line-height: 1.4;
  }
  .opt:hover {
    background: var(--surface-2);
  }
  .opt.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text);
  }
  .opt:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .foot {
    margin-top: 14px;
  }
  @media (max-width: 560px) {
    .list li {
      grid-template-columns: 1fr 1fr;
    }
    .t {
      grid-column: 1 / -1;
      padding-top: 0;
    }
  }
</style>
