<script lang="ts">
  import { onMount } from "svelte";
  import { api, watchModels, type ModelView } from "../lib/api";
  import ModelCard from "../components/ModelCard.svelte";
  import FfmpegCard from "../components/FfmpegCard.svelte";

  let models = $state<ModelView[]>([]);

  onMount(() => {
    const off = watchModels((m) => (models = m));
    return () => off.then((f) => f());
  });
</script>

<h1>Modelos</h1>
<p class="lead">
  O reconhecimento de fala roda neste computador. O modelo é baixado uma vez e fica em
  <button class="link" onclick={() => api.openDataDir()}>sua pasta de dados</button>; nenhum áudio sai da máquina.
</p>

<div class="list">
  {#each models as m (m.id)}
    <ModelCard model={m} />
  {/each}
</div>

<h2>Ferramentas</h2>
<div class="list">
  <FfmpegCard />
</div>

<style>
  h1 {
    margin: 0 0 8px;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  h2 {
    margin: 28px 0 12px;
    font-size: 14px;
    font-weight: 650;
  }
  .lead {
    max-width: 620px;
    margin: 0 0 20px;
    color: var(--text-2);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 640px;
  }
</style>
