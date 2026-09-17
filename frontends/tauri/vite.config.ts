import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig(({mode}) => ({
  plugins: [svelte(), ...(mode === "browser-test" ? [{
    name: "conn-browser-test-transport",
    configureServer(server: import("vite").ViteDevServer) {
      server.middlewares.use(async(req,res,next)=>{
        const path=new URL(req.url??"/","http://localhost").pathname;
        if(path==="/__conn/connection") {
          try { res.setHeader("Content-Type","application/json");res.setHeader("Cache-Control","no-store");res.end(await readFile(resolve(process.env.CONN_TEST_STATE!,"connection.json"))); }
          catch {res.statusCode=503;res.end("test harness unavailable");} return;
        }
        next();
      });
    }
  }] : [])],
  clearScreen: false,
  server: {
    watch: { ignored: ["**/target/**"] },
    ...(mode === "browser-test" ? {hmr:false} : {}), port: 1420, strictPort: true, host: false },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "safari13", minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false, sourcemap: !!process.env.TAURI_ENV_DEBUG },
}));
