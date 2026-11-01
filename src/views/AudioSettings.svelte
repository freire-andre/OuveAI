<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, meter, type AudioSetup, type MicMode, type OutrosMode, type Retention, type TestTick } from "../lib/api";

  /** `compact`: só dispositivos + Testar áudio (assistente). */
  let { compact = false }: { compact?: boolean } = $props();

  let setup = $state<AudioSetup | null>(null);
  let err = $state<string | null>(null);
  let testing = $state(false);
  let level = $state<TestTick | null>(null);
  let heard = $state({ eu: false, outros: false });
  let tone: { ctx: AudioContext; stop: () => void } | null = null;

  async function load() {
    try {
      setup = await api.audioSetup();
    } catch (e) {
      err = String(e);
    }
  }

  onMount(() => {
    load();
    const off = listen<TestTick>("audio-test", (e) => {
      level = e.payload;
      if (e.payload.eu > -45) heard.eu = true;
      if (e.payload.outros > -45) heard.outros = true;
    });
    return () => {
      off.then((f) => f());
      if (testing) stopTest();
    };
  });

  const mics = $derived(setup?.devices.filter((d) => d.input) ?? []);
  const outs = $derived(setup?.devices.filter((d) => !d.input) ?? []);

  const micValue = $derived(setup ? (setup.mic.mode === "device" ? `device:${setup.mic.id}` : setup.mic.mode) : "auto");
  const outrosValue = $derived(setup ? (setup.outros.mode === "auto" ? "auto" : `${setup.outros.mode}:${setup.outros.id}`) : "auto");

  async function set(key: "mic" | "outros" | "headphones" | "retention", value: string) {
    err = null;
    try {
      await api.audioSet(key, value);
      await load();
      if (testing && (key === "mic" || key === "outros")) {
        await api.audioTestStart();
      }
    } catch (e) {
      err = String(e);
    }
  }

  function setMic(v: string) {
    const m: MicMode = v.startsWith("device:") ? { mode: "device", id: v.slice(7) } : ({ mode: v } as MicMode);
    set("mic", JSON.stringify(m));
  }

  function setOutros(v: string) {
    const i = v.indexOf(":");
    const m: OutrosMode = i < 0 ? { mode: "auto" } : ({ mode: v.slice(0, i), id: v.slice(i + 1) } as OutrosMode);
    set("outros", JSON.stringify(m));
  }

  async function startTest() {
    err = null;
    heard = { eu: false, outros: false };
    try {
      await api.audioTestStart();
      testing = true;
    } catch (e) {
      err = String(e);
    }
  }

  async function stopTest() {
    stopTone();
    testing = false;
    level = null;
    await api.audioTestStop();
  }

  /** Som de teste curto na saída padrão: deve mexer o medidor OUTROS. */
  function playTone() {
    stopTone();
    const ctx = new AudioContext();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.frequency.value = 440;
    gain.gain.setValueAtTime(0, ctx.currentTime);
    for (let k = 0; k < 4; k++) {
      const t0 = ctx.currentTime + k * 0.5;
      gain.gain.linearRampToValueAtTime(0.25, t0 + 0.02);
      gain.gain.linearRampToValueAtTime(0.25, t0 + 0.3);
      gain.gain.linearRampToValueAtTime(0, t0 + 0.34);
    }
    osc.connect(gain).connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + 2.1);
    osc.onended = () => stopTone();
    tone = { ctx, stop: () => ctx.close() };
  }

  function stopTone() {
    tone?.stop();
    tone = null;
  }

  function hpLabel(s: AudioSetup): string {
    if (s.headphones_auto === null) return "Automático";
    return `Automático (agora: ${s.headphones_auto ? "fone" : "alto-falante"})`;
  }
</script>

{#if setup}
  {#if setup.mic_denied}
    <p class="warn">
      O Windows está bloqueando o acesso ao microfone para apps da área de trabalho. Libere em
      Configurações do Windows › Privacidade e segurança › Microfone › "Permitir que aplicativos da área de trabalho acessem o microfone".
    </p>
  {/if}

  <div class="grid">
    <label for="mic">Microfone (EU)</label>
    <select id="mic" value={micValue} onchange={(e) => setMic(e.currentTarget.value)}>
      <option value="auto">Automático (o mic do app de reunião)</option>
      {#each mics as d (d.id)}
        <option value={`device:${d.id}`}>{d.name}{d.default ? " · padrão" : ""}{d.bluetooth ? " · Bluetooth" : ""}</option>
      {/each}
      <option value="off">Não gravar o microfone</option>
    </select>

    <label for="outros">Som da reunião (OUTROS)</label>
    <select id="outros" value={outrosValue} onchange={(e) => setOutros(e.currentTarget.value)}>
      <option value="auto">Automático (saídas padrão)</option>
      {#each outs as d (d.id)}
        <option value={`device:${d.id}`}>{d.name}{d.default ? " · padrão" : ""}</option>
      {/each}
      {#if setup.app_loopback}
        <optgroup label="Por aplicativo (avançado)">
          {#each setup.apps as a (a)}
            <option value={`app:${a}`}>Só o som do {a}</option>
          {/each}
          {#if setup.outros.mode === "app" && !setup.apps.includes(setup.outros.id)}
            <option value={`app:${setup.outros.id}`}>Só o som do {setup.outros.id}</option>
          {/if}
        </optgroup>
      {/if}
    </select>
  </div>
  {#if setup.app_loopback && setup.apps.length === 0 && setup.outros.mode !== "app"}
    <p class="hint">Para gravar só o som de um aplicativo, abra a reunião e volte aqui: os apps tocando som aparecem na lista.</p>
  {/if}

  <div class="test">
    {#if testing}
      <button class="btn" onclick={stopTest}>Parar teste</button>
      <button class="btn" onclick={playTone}>Tocar som de teste</button>
    {:else}
      <button class="btn" onclick={startTest}>Testar áudio</button>
    {/if}
    {#if testing}
      <div class="meters">
        <span class="who eu">EU</span>
        <span class="meter eu"><span style="width: {level ? meter(level.eu) : 0}%"></span></span>
        <span class="ok">{level && !level.eu_open ? "não abriu" : heard.eu ? "ouvindo ✓" : "fale algo"}</span>
        <span class="who outros">OUTROS</span>
        <span class="meter outros"><span style="width: {level ? meter(level.outros) : 0}%"></span></span>
        <span class="ok">{level && !level.outros_open ? "não abriu" : heard.outros ? "ouvindo ✓" : "toque o som de teste"}</span>
      </div>
    {/if}
  </div>

  {#if !compact}
    <div class="grid">
      <label for="hp">Fone de ouvido</label>
      <select id="hp" value={setup.headphones} onchange={(e) => set("headphones", e.currentTarget.value)}>
        <option value="auto">{hpLabel(setup)}</option>
        <option value="yes">Estou usando fone</option>
        <option value="no">Estou no alto-falante</option>
      </select>

      <label for="ret">Salvar o áudio da reunião</label>
      <select id="ret" value={setup.retention} onchange={(e) => set("retention", e.currentTarget.value as Retention)}>
        <option value="none">Não salvar (só o texto)</option>
        <option value="7">Por 7 dias</option>
        <option value="30">Por 30 dias</option>
        <option value="always">Sempre</option>
      </select>
    </div>
    <p class="hint">
      No alto-falante, o microfone também capta a voz dos outros; o OuveAI reconhece esse eco e o esconde da transcrição.
      O áudio salvo fica só neste computador e é apagado sozinho ao vencer.
    </p>
  {/if}
{:else if !err}
  <p class="hint">Procurando dispositivos…</p>
{/if}
{#if err}
  <p class="err">{err}</p>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    align-items: center;
    gap: 10px 12px;
    margin: 12px 0;
  }
  .grid label {
    color: var(--text-2);
  }
  select {
    min-width: 0;
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font: inherit;
  }
  .test {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin: 12px 0;
  }
  .meters {
    display: grid;
    flex-basis: 100%;
    grid-template-columns: 60px minmax(0, 1fr) 150px;
    align-items: center;
    gap: 6px 10px;
  }
  .who {
    font: 600 11px var(--mono);
  }
  .eu {
    color: var(--eu);
  }
  .outros {
    color: var(--outros);
  }
  .meter {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .meter span {
    display: block;
    height: 100%;
    background: currentColor;
    transition: width 0.1s linear;
  }
  .ok {
    color: var(--text-2);
    font-size: 12.5px;
  }
  .hint {
    display: block;
    margin: 6px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .warn {
    margin: 0 0 12px;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-size: 13px;
  }
  .err {
    color: var(--err);
    font-size: 12.5px;
  }
</style>
