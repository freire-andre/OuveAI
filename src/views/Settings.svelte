<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { save } from "@tauri-apps/plugin-dialog";
  import { accelFromEvent, api, type Action, type DataUsage, type Status, type UpdateInfo } from "../lib/api";
  import { mb } from "../lib/format";
  const size = (b: number) => (b === 0 ? "nada" : b < 1024 * 1024 ? "menos de 1 MB" : mb(b));
  import AudioSettings from "./AudioSettings.svelte";

  let { status }: { status: Status } = $props();

  const labels: Record<Action, string> = {
    mini: "Mostrar/ocultar a mini",
    record: "Gravar / parar",
    mark: "Marcar momento",
  };

  let proxy = $state("");
  let proxyMsg = $state<{ ok: boolean; text: string } | null>(null);
  api.getProxy().then((p) => (proxy = p));

  async function saveProxy() {
    try {
      await api.setProxy(proxy);
      proxyMsg = { ok: true, text: proxy.trim() ? "Proxy salvo." : "Usando o proxy do sistema." };
    } catch (e) {
      proxyMsg = { ok: false, text: String(e) };
    }
  }

  // Dados locais: uso de disco e "Excluir tudo".
  let usage = $state<DataUsage | null>(null);
  let wipeStep = $state<0 | 1>(0);
  let wipeMsg = $state<{ ok: boolean; text: string } | null>(null);
  const loadUsage = () => api.dataUsage().then((u) => (usage = u)).catch(() => {});
  onMount(() => {
    loadUsage();
    const off = listen("sessions-changed", loadUsage);
    return () => off.then((f) => f());
  });

  async function wipe() {
    wipeMsg = null;
    try {
      await api.deleteAll();
      wipeStep = 0;
      wipeMsg = { ok: true, text: "Todas as sessões e todo o áudio salvo foram apagados." };
      loadUsage();
    } catch (e) {
      wipeStep = 0;
      wipeMsg = { ok: false, text: String(e) };
    }
  }

  // Sobre: verificar atualização (manual, D5) e exportar diagnóstico.
  let upd = $state<{ busy: boolean; info: UpdateInfo | null; err: string }>({ busy: false, info: null, err: "" });
  async function checkUpdate() {
    upd = { busy: true, info: null, err: "" };
    try {
      upd = { busy: false, info: await api.checkUpdate(), err: "" };
    } catch (e) {
      upd = { busy: false, info: null, err: String(e) };
    }
  }

  let diagMsg = $state<{ ok: boolean; text: string } | null>(null);
  async function exportDiag() {
    diagMsg = null;
    const d = new Date();
    const day = `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}`;
    const dest = await save({ defaultPath: `OuveAI-diagnostico-${day}.zip`, filters: [{ name: "Arquivo zip", extensions: ["zip"] }] });
    if (!dest) return;
    try {
      await api.exportDiagnostics(dest);
      diagMsg = { ok: true, text: "Diagnóstico salvo. Ele não tem nenhuma transcrição, áudio nem título de reunião." };
    } catch (e) {
      diagMsg = { ok: false, text: String(e) };
    }
  }

  const credits: { name: string; license: string; note?: string }[] = [
    { name: "Parakeet TDT 0.6B v3 (NVIDIA)", license: "CC-BY-4.0", note: "modelo de reconhecimento de fala" },
    { name: "Ajuste pt-BR do Parakeet (alexandreacff; conversão de joaorura)", license: "CC-BY-4.0", note: "treinado com o conjunto TAGARELA" },
    { name: "sherpa-onnx (k2-fsa)", license: "Apache-2.0" },
    { name: "ONNX Runtime (Microsoft)", license: "MIT" },
    { name: "Silero VAD", license: "MIT", note: "detecção de fala" },
    { name: "FFmpeg (build de gyan.dev)", license: "GPL-3.0", note: "baixado à parte, só para importar arquivos" },
    { name: "Tauri", license: "MIT / Apache-2.0" },
    { name: "Svelte", license: "MIT" },
    { name: "SQLite", license: "domínio público" },
    { name: "rusqlite, ureq, rustls, wasapi, zip e demais crates Rust", license: "MIT / Apache-2.0" },
  ];

  let capturing = $state<Action | null>(null);
  let localError = $state<{ action: Action; msg: string } | null>(null);

  async function startCapture(a: Action) {
    capturing = a;
    localError = null;
    // Sem isso, apertar o atalho atual durante a captura dispararia a ação.
    await api.suspendShortcuts(true);
  }

  async function stopCapture() {
    if (capturing === null) return;
    capturing = null;
    await api.suspendShortcuts(false);
  }

  async function onKey(e: KeyboardEvent, a: Action) {
    if (capturing !== a) return;
    e.preventDefault();
    if (e.key === "Escape") return stopCapture();
    const accel = accelFromEvent(e);
    if (!accel) return; // ainda só modificadores
    capturing = null;
    try {
      await api.setShortcut(a, accel);
    } catch (err) {
      localError = { action: a, msg: String(err) };
    }
    await api.suspendShortcuts(false);
  }
</script>

<h1>Configurações</h1>

<section>
  <h2>Áudio</h2>
  <AudioSettings />
</section>

<section>
  <h2>Privacidade</h2>
  <label class="row" class:disabled={!status.stealth_supported}>
    <input
      type="checkbox"
      checked={status.stealth}
      disabled={!status.stealth_supported}
      onchange={(e) => api.setStealth(e.currentTarget.checked)}
    />
    <span>
      <strong>Stealth</strong>
      <span class="hint">As janelas do OuveAI não aparecem em prints, gravações de tela nem no compartilhamento de tela. Você continua vendo normalmente.</span>
    </span>
  </label>
  {#if !status.stealth_supported}
    <p class="warn">
      Este Windows (build {status.build}) não permite ocultar janelas de capturas. Isso exige o Windows 10 versão 2004 (build 19041) ou mais novo.
    </p>
  {/if}
</section>

<section>
  <div class="head">
    <h2>Atalhos globais</h2>
    <button class="btn" onclick={() => api.resetShortcuts()}>Restaurar padrões</button>
  </div>
  <p class="hint">Funcionam com o OuveAI em segundo plano. Clique no atalho e pressione a nova combinação (Esc cancela).</p>
  <ul class="shortcuts">
    {#each status.shortcuts as s (s.action)}
      {@const err = localError?.action === s.action ? localError.msg : s.error}
      <li>
        <span>{labels[s.action]}</span>
        <button
          class="accel"
          class:capturing={capturing === s.action}
          onclick={() => startCapture(s.action)}
          onkeydown={(e) => onKey(e, s.action)}
          onblur={stopCapture}
        >
          {capturing === s.action ? "Pressione o atalho…" : s.accel}
        </button>
        {#if err}
          <span class="err">{err}. Tente outra combinação.</span>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<section>
  <h2>Rede</h2>
  <p class="hint">O OuveAI só usa a internet para baixar o modelo e o FFmpeg (sempre com a sua confirmação) e, quando você clica em "Verificar atualização", para consultar a última versão. Nenhuma transcrição ou áudio sai deste computador. Normalmente o proxy do Windows basta; preencha só se o download falhar na rede da empresa.</p>
  <form class="proxy" onsubmit={(e) => { e.preventDefault(); saveProxy(); }}>
    <label for="proxy">Proxy manual</label>
    <input id="proxy" type="text" placeholder="http://servidor:porta" bind:value={proxy} spellcheck="false" />
    <button class="btn" type="submit">Salvar</button>
  </form>
  {#if proxyMsg}
    <p class={proxyMsg.ok ? "hint" : "err"}>{proxyMsg.text}</p>
  {/if}
</section>

<section>
  <h2>Dados neste computador</h2>
  {#if usage}
    <dl>
      <dt>Sessões</dt>
      <dd>{usage.meetings} {usage.meetings === 1 ? "reunião" : "reuniões"} · {usage.files} {usage.files === 1 ? "arquivo" : "arquivos"}</dd>
      <dt>Áudio salvo</dt>
      <dd>{size(usage.audio_bytes)}</dd>
      <dt>Transcrições</dt>
      <dd>{size(usage.db_bytes)}</dd>
      <dt>Modelos</dt>
      <dd>{size(usage.models_bytes)}</dd>
    </dl>
  {/if}
  <p class="hint spaced">O áudio vencido é apagado sozinho (ao abrir o OuveAI e de hora em hora), conforme “Salvar o áudio da reunião” acima.</p>
  <div class="actions">
    <button class="btn" onclick={() => api.openDataDir()}>Abrir pasta dos dados</button>
    {#if wipeStep === 0}
      <button class="btn danger" disabled={status.recording || (usage !== null && usage.meetings + usage.files === 0 && usage.audio_bytes === 0)} onclick={() => ((wipeStep = 1), (wipeMsg = null))}>
        Excluir tudo…
      </button>
    {:else}
      <button class="btn danger solid" onclick={wipe}>Apagar todas as sessões e áudios</button>
      <button class="btn" onclick={() => (wipeStep = 0)}>Cancelar</button>
    {/if}
  </div>
  {#if wipeStep === 1}
    <p class="warn">
      Apaga de vez todas as reuniões e arquivos transcritos, com os áudios salvos. Não tem como desfazer.
      Ficam as configurações, os modelos e as legendas já salvas ao lado dos seus vídeos.
    </p>
  {/if}
  {#if wipeMsg}
    <p class={wipeMsg.ok ? "hint spaced" : "err"}>{wipeMsg.text}</p>
  {/if}
</section>

<section>
  <h2>Sobre</h2>
  <dl>
    <dt>Versão</dt>
    <dd>{status.version}</dd>
    <dt>Windows</dt>
    <dd>build {status.build}</dd>
    <dt>Dados locais</dt>
    <dd class="mono">{status.data_dir}</dd>
  </dl>

  <div class="actions spaced">
    <button class="btn" onclick={checkUpdate} disabled={upd.busy}>{upd.busy ? "Verificando…" : "Verificar atualização"}</button>
    <button class="btn" onclick={exportDiag}>Exportar diagnóstico…</button>
  </div>
  {#if upd.info}
    {#if upd.info.newer && upd.info.latest}
      <p class="hint spaced">
        A versão {upd.info.latest.version} está disponível (você tem a {upd.info.current}).
        <button class="link" onclick={() => upd.info?.latest && api.openRelease(upd.info.latest.url)}>Abrir a página de download</button>
      </p>
    {:else if upd.info.latest}
      <p class="hint spaced">Você já está na versão mais recente ({upd.info.current}).</p>
    {:else}
      <p class="hint spaced">Nenhuma versão publicada ainda.</p>
    {/if}
  {/if}
  {#if upd.err}
    <p class="err">{upd.err}</p>
  {/if}
  {#if diagMsg}
    <p class={diagMsg.ok ? "hint spaced" : "err"}>{diagMsg.text}</p>
  {/if}
  <p class="hint spaced">O diagnóstico junta versão, Windows, processador, memória, dispositivos de áudio, configurações e logs, para você mandar a quem estiver ajudando.</p>

  <details class="credits">
    <summary>Licenças e atribuições</summary>
    <ul>
      {#each credits as c}
        <li><b>{c.name}</b>: {c.license}{c.note ? ` · ${c.note}` : ""}</li>
      {/each}
    </ul>
    <p class="hint">
      O modelo Parakeet é distribuído pela NVIDIA sob CC-BY-4.0. A versão pt-BR usada aqui foi ajustada por alexandreacff (Hugging Face) e convertida para o sherpa-onnx por joaorura, sob a mesma licença. O FFmpeg não vem no instalador:
      ele é baixado à parte, e a licença dele fica em <span class="mono">bin\FFMPEG-LICENSE.txt</span>.
    </p>
  </details>
</section>

<style>
  h1 {
    margin: 0 0 20px;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  section {
    max-width: 640px;
    padding: 18px 0 22px;
    border-top: 1px solid var(--line);
  }
  h2 {
    margin: 0 0 12px;
    font-size: 14px;
    font-weight: 650;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .head h2 {
    margin: 0;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    cursor: pointer;
  }
  .row input {
    margin-top: 3px;
    accent-color: var(--accent);
  }
  .row.disabled {
    opacity: 0.6;
    cursor: default;
  }
  .hint {
    display: block;
    color: var(--text-2);
    font-size: 13px;
  }
  .warn {
    margin: 12px 0 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-size: 13px;
  }
  .shortcuts {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
  }
  .shortcuts li {
    display: grid;
    grid-template-columns: 220px auto;
    align-items: center;
    gap: 4px 12px;
    padding: 8px 0;
  }
  .accel {
    justify-self: start;
    min-width: 180px;
    padding: 5px 10px;
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    font: 13px var(--mono);
    text-align: left;
  }
  .accel.capturing {
    border-color: var(--accent);
    color: var(--accent);
  }
  .err {
    grid-column: 2;
    color: var(--err);
    font-size: 12.5px;
  }
  .proxy {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }
  .proxy label {
    color: var(--text-2);
  }
  .proxy input {
    flex: 1;
    max-width: 320px;
    padding: 6px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font: 13px var(--mono);
  }
  section > .err {
    display: block;
    margin: 8px 0 0;
  }
  dl {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
  }
  .spaced {
    margin: 12px 0 0;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 12px;
  }
  .btn.danger {
    border-color: var(--err);
    color: var(--err);
  }
  .btn.danger.solid {
    background: var(--err);
    color: #fff;
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .mono {
    font: 12.5px var(--mono);
    word-break: break-all;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .credits {
    margin-top: 16px;
  }
  .credits summary {
    cursor: pointer;
    color: var(--text-2);
  }
  .credits ul {
    margin: 10px 0 0;
    padding-left: 18px;
    line-height: 1.6;
  }
</style>
