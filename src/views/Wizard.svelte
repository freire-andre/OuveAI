<script lang="ts">
  import { onMount } from "svelte";
  import { api, watchModels, watchStatus, type MachineCheck, type ModelView, type Status } from "../lib/api";
  import { mb } from "../lib/format";
  import ModelCard from "../components/ModelCard.svelte";
  import AudioSettings from "./AudioSettings.svelte";

  const steps = ["Boas-vindas", "Privacidade", "Seu computador", "Modelo", "Áudio", "Atalhos"] as const;
  let i = $state(0);
  let machine = $state<MachineCheck | null>(null);
  let models = $state<ModelView[]>([]);
  let status = $state<Status | null>(null);

  onMount(() => {
    api.machine().then((m) => (machine = m));
    const offs = [watchModels((m) => (models = m)), watchStatus((s) => (status = s))];
    return () => offs.forEach((p) => p.then((f) => f()));
  });

  const parakeet = $derived(models.find((m) => m.id === "parakeet-tdt-0.6b-v3-int8"));
  const labels = { mini: "Mostrar/ocultar a mini", record: "Gravar / parar", mark: "Marcar momento" } as const;

  interface Check {
    label: string;
    value: string;
    ok: boolean;
    note?: string;
  }
  const checks = $derived.by((): Check[] => {
    const m = machine;
    if (!m) return [];
    return [
      {
        label: "Windows",
        value: `build ${m.windows_build}`,
        ok: m.windows_build >= m.min_build,
        note: "Precisa do Windows 10 versão 2004 ou mais novo para ocultar as janelas em capturas.",
      },
      {
        label: "Memória",
        value: `${(m.ram_total_mb / 1024).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} GB`,
        ok: m.ram_total_mb >= m.min_ram_mb,
        note: "Com menos de 8 GB a transcrição funciona, mas pode deixar o computador lento durante a reunião.",
      },
      {
        label: "Processador",
        value: `${m.cores} núcleos${m.avx2 ? " · AVX2" : ""}`,
        ok: m.cores >= m.min_cores && m.avx2,
        note: "Com menos de 4 núcleos ou sem AVX2, a transcrição fica mais lenta.",
      },
      {
        label: "Espaço livre",
        value: mb(m.disk_free_mb * 1024 * 1024),
        ok: m.disk_free_mb >= m.model_mb + 200,
        note: `O modelo precisa de cerca de ${mb(m.model_mb * 1024 * 1024)} livres para baixar e instalar.`,
      },
    ];
  });
</script>

<div class="wizard">
  <ol class="steps" aria-label="Etapas">
    {#each steps as s, n}
      <li class:current={n === i} class:done={n < i}>{s}</li>
    {/each}
  </ol>

  <section class="body">
    {#if i === 0}
      <h1>Bem-vindo ao OuveAI</h1>
      <p>O OuveAI transcreve suas reuniões (Teams, Meet, Zoom) e arquivos de áudio <strong>neste computador</strong>.</p>
      <ul class="points">
        <li>Separa o que <strong>você</strong> fala do que <strong>os outros</strong> falam.</li>
        <li>Funciona sem internet depois de baixar o modelo de reconhecimento de fala.</li>
        <li>Fica discreto na bandeja, perto do relógio, e não aparece no compartilhamento de tela.</li>
      </ul>
    {:else if i === 1}
      <h1>Privacidade</h1>
      <ul class="points">
        <li><strong>Nenhum áudio sai da máquina.</strong> A única conexão é o download do modelo, sempre com a sua confirmação.</li>
        <li>O áudio da reunião <strong>não é gravado</strong> por padrão; só o texto transcrito fica salvo.</li>
        <li>As transcrições ficam só neste computador, na sua pasta de dados do Windows.</li>
      </ul>
      <div class="notice">
        <strong>Lembrete (LGPD):</strong> transcrever uma reunião é tratar dados pessoais de quem participa. Avise os participantes
        e siga as regras da sua empresa sobre gravação e registro de reuniões.
      </div>
    {:else if i === 2}
      <h1>Seu computador</h1>
      {#if !machine}
        <p>Verificando…</p>
      {:else}
        <ul class="checks">
          {#each checks as c}
            <li>
              <span class="mark" class:ok={c.ok}>{c.ok ? "✓" : "!"}</span>
              <span class="label">{c.label}</span>
              <span class="value">{c.value}</span>
              {#if !c.ok && c.note}<span class="note">{c.note}</span>{/if}
            </li>
          {/each}
        </ul>
        {#if checks.every((c) => c.ok)}
          <p class="muted">Tudo certo para usar o OuveAI.</p>
        {:else}
          <p class="muted">Você pode continuar mesmo assim.</p>
        {/if}
      {/if}
    {:else if i === 3}
      <h1>Modelo de reconhecimento de fala</h1>
      <p>Baixe o modelo agora (cerca de {parakeet ? mb(parakeet.size) : "650 MB"}). Você pode continuar o assistente enquanto ele baixa, ou fazer isso depois em <em>Modelos</em>.</p>
      {#if parakeet}
        <ModelCard model={parakeet} />
      {/if}
    {:else if i === 4}
      <h1>Microfone e som</h1>
      <p>Clique em <em>Testar áudio</em>, fale algo e toque o som de teste: os dois medidores devem se mexer. Na dúvida, deixe no automático.</p>
      <AudioSettings compact />
    {:else if i === 5}
      <h1>Atalhos e bandeja</h1>
      <p>O OuveAI fica na <strong>bandeja</strong> (perto do relógio). Clique no ícone para abrir; o botão direito mostra o menu.</p>
      {#if status}
        <ul class="shortcuts">
          {#each status.shortcuts as s (s.action)}
            <li><span>{labels[s.action]}</span><kbd>{s.accel}</kbd></li>
          {/each}
        </ul>
        <p class="muted">Dá para trocar os atalhos em <em>Configurações</em>.</p>
      {/if}
    {/if}
  </section>

  <footer>
    {#if i > 0}
      <button class="btn" onclick={() => i--}>Voltar</button>
    {/if}
    <span class="spacer"></span>
    {#if i < steps.length - 1}
      <button class="btn primary" onclick={() => i++}>
        {i === 3 && parakeet?.state !== "ready" ? "Continuar sem esperar" : "Continuar"}
      </button>
    {:else}
      <button class="btn primary" onclick={() => api.wizardFinish()}>Concluir</button>
    {/if}
  </footer>
</div>

<style>
  .wizard {
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: 100%;
  }
  .steps {
    display: flex;
    gap: 6px;
    margin: 0;
    padding: 16px 28px;
    list-style: none;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
    font-size: 12.5px;
    color: var(--text-3);
    counter-reset: step;
  }
  .steps li {
    counter-increment: step;
  }
  .steps li::before {
    content: counter(step) ". ";
  }
  .steps li:not(:last-child)::after {
    content: "  ›";
    color: var(--line);
  }
  .steps li.current {
    color: var(--text);
    font-weight: 600;
  }
  .steps li.done {
    color: var(--accent);
  }
  .body {
    overflow: auto;
    padding: 28px;
  }
  h1 {
    margin: 0 0 14px;
    font-size: 21px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  p {
    margin: 0 0 14px;
    max-width: 600px;
  }
  .points {
    margin: 0 0 16px;
    padding-left: 20px;
    max-width: 600px;
  }
  .points li {
    margin-bottom: 8px;
  }
  .notice {
    max-width: 600px;
    padding: 12px 14px;
    border-radius: var(--radius-sm);
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-size: 13px;
  }
  .checks {
    margin: 0 0 14px;
    padding: 0;
    list-style: none;
    max-width: 600px;
  }
  .checks li {
    display: grid;
    grid-template-columns: 24px 120px 1fr;
    align-items: baseline;
    gap: 2px 8px;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .mark {
    font-weight: 700;
    color: var(--warn-ink);
  }
  .mark.ok {
    color: var(--accent);
  }
  .label {
    color: var(--text-2);
  }
  .note {
    grid-column: 3;
    color: var(--warn-ink);
    font-size: 12.5px;
  }
  .muted {
    color: var(--text-2);
    font-size: 13px;
  }
  .shortcuts {
    margin: 0 0 14px;
    padding: 0;
    list-style: none;
    max-width: 420px;
  }
  .shortcuts li {
    display: flex;
    justify-content: space-between;
    padding: 7px 0;
    border-bottom: 1px solid var(--line);
  }
  footer {
    display: flex;
    gap: 8px;
    padding: 14px 28px;
    border-top: 1px solid var(--line);
    background: var(--surface);
  }
  .spacer {
    flex: 1;
  }
</style>
