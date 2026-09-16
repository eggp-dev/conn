import { invoke as nativeInvoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listen as nativeListen } from "@tauri-apps/api/event";

// Only the transport boundary knows about the browser test adapter.
const browser = import.meta.env.MODE === "browser-test";
const adapter = () => import("../../tests/browser/transport");
export async function invoke<T = any>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  return browser ? (await adapter()).invoke<T>(name,args) : nativeInvoke<T>("dispatch",{name,args});
}
export async function listen<T>(name: string, handler: (event: {payload:T})=>void): Promise<()=>void> {
  return browser ? (await adapter()).listen(name,handler) : nativeListen<T>(name,handler, { target: { kind: "WebviewWindow", label: getCurrentWebviewWindow().label } });
}
