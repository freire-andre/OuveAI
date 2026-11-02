<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, watchStatus, type Status, type UpdatePrefs } from "../lib/api";
  import Sessions from "./Sessions.svelte";
  import Settings from "./Settings.svelte";
  import Models from "./Models.svelte";
  import Files from "./Files.svelte";
  import Editor from "./Editor.svelte";
  import Subtitles from "./Subtitles.svelte";

  type View = "sessions" | "files" | "subtitles" | "models" | "settings";
  const views: View[] = ["sessions", "files", "subtitles", "models", "settings"];
  const asView = (v: string): View => (views.includes(v as View) ? (v as View) : "sessions");
  const fromHash = (): View => asView(location.hash.slice(1));

  let view = $state<View>(fromHash());
  let status = $state<Status | null>(null);
  /** Transcrição aberta no editor (dentro de "Arquivos"). */
  let editing = $state<number | null>(null);
  /** Versão nova (opção "Avisar quando sair versão nova"); fechar esconde até reabrir a janela. */
  let notice = $state<UpdatePrefs["notice"]>(null);

  onMount(() => {
    api.updatePrefs().then((p) => (notice = p.notice)).catch(() => {});
    const offs = [
      watchStatus((s) => (status = s)),
      listen<string>("navigate", (e) => (view = asView(e.payload))),
      listen<UpdatePrefs["notice"]>("update-notice", (e) => (notice = e.payload)),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  });

  const nav: { id: View; label: string }[] = [
    { id: "sessions", label: "Sessões" },
    { id: "files", label: "Arquivos" },
    { id: "subtitles", label: "Legendas" },
    { id: "models", label: "Modelos" },
    { id: "settings", label: "Configurações" },
  ];
</script>

<div class="shell">
  <aside>
    <div class="brand">OuveAI</div>
    <nav>
      {#each nav as item}
        <button
          class:active={view === item.id}
          onclick={() => {
            view = item.id;
            editing = null;
          }}>{item.label}</button
        >
      {/each}
    </nav>
    {#if status?.recording}
      <div class="rec"><span class="dot"></span>Gravando</div>
    {/if}
  </aside>
  <main>
    {#if notice}
      <div class="update" role="status">
        <span>A versão <strong>{notice.version}</strong> do OuveAI está disponível{status ? ` (você tem a ${status.version})` : ""}.</span>
        <span class="update-actions">
          <button class="btn primary" onclick={() => notice && api.openRelease(notice.url)}>Abrir página de download</button>
          <button class="btn" onclick={() => notice && api.updateSkip(notice.version)}>Ignorar esta versão</button>
          <button class="close" aria-label="Fechar aviso" title="Fechar (volta a aparecer na próxima vez)" onclick={() => (notice = null)}>×</button>
        </span>
      </div>
    {/if}
    {#if status}
      {#if view === "settings"}
        <Settings {status} />
      {:else if view === "files"}
        {#if editing !== null}
          <Editor id={editing} onBack={() => (editing = null)} />
        {:else}
          <Files onOpen={(id) => (editing = id)} />
        {/if}
      {:else if view === "subtitles"}
        <Subtitles />
      {:else if view === "models"}
        <Models />
      {:else}
        <Sessions
          {status}
          onOpenFile={(id) => {
            view = "files";
            editing = id;
          }}
        />
      {/if}
    {/if}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 200px 1fr;
    height: 100%;
  }
  aside {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px 12px;
    border-right: 1px solid var(--line);
    background: var(--surface);
  }
  .brand {
    padding: 0 10px;
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  nav button {
    text-align: left;
    padding: 7px 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-2);
  }
  nav button:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  nav button.active {
    background: var(--surface-2);
    color: var(--text);
    font-weight: 600;
  }
  .rec {
    margin-top: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    color: var(--rec);
    font-weight: 600;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--rec);
  }
  main {
    overflow: auto;
    padding: 28px 32px;
  }
  .update {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
    margin: -12px 0 20px;
    padding: 10px 12px 10px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--warn-bg);
    color: var(--warn-ink);
  }
  .update-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .close {
    border: 0;
    background: none;
    color: inherit;
    font-size: 18px;
    line-height: 1;
    padding: 2px 6px;
    cursor: pointer;
  }
</style>
