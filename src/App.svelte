<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Main from "./views/Main.svelte";
  import Mini from "./views/Mini.svelte";
  import Wizard from "./views/Wizard.svelte";

  // Cada janela carrega o mesmo index.html; o rótulo decide o que mostrar.
  const label = getCurrentWindow().label;
</script>

{#if label === "mini"}
  <Mini />
{:else if label === "wizard"}
  <Wizard />
{:else}
  <Main />
{/if}
