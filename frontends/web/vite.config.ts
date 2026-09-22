import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  // Recreate the host attachment with the app. Component-only HMR would reuse
  // ports disposed by the old ConnApp; a page reload retains the server's PTYs.
  plugins: [svelte({ compilerOptions: { hmr: false } })],
  resolve: { dedupe: ['svelte'] },
  server: {
    host: '127.0.0.1', port: 1429, strictPort: true,
    watch: { ignored: ['**/target/**'] },
    proxy: { '/api': { target: process.env.CONN_WEB_BACKEND ?? 'http://127.0.0.1:1430', ws: true, changeOrigin: true } },
  },
  build: { target: 'es2021' },
});
