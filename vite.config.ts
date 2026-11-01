import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Porta fixa: o tauri.conf.json aponta o devUrl para ela.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
