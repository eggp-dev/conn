import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  // Recreate native ports together with the common app during development.
  plugins: [svelte({ compilerOptions: { hmr: false } })],
  clearScreen: false,
  server: { watch: { ignored: ['**/target/**'] }, port: 1420, strictPort: true, host: false },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'safari13', minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false, sourcemap: !!process.env.TAURI_ENV_DEBUG },
});
