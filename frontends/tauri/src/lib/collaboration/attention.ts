/** App inactivity changes notification delivery only, never session access. */
import {getCurrentWindow,UserAttentionType} from '@tauri-apps/api/window';
let previous=new Set<string>();
let lastSignal=0;
let revision=0;
export async function signalPending(keys:string[]) {
  const current=++revision;
  const next=new Set(keys),fresh=keys.some(key=>!previous.has(key));previous=next;
  document.title=keys.length ? `(${keys.length}) Conn` : 'Conn';
  if(import.meta.env.MODE==='browser-test')return;
  const window=getCurrentWindow();
  if(!keys.length){await window.requestUserAttention(null);return;}
  if(!fresh || Date.now()-lastSignal<10000 || await window.isFocused() || current!==revision)return;
  lastSignal=Date.now();
  // Taskbar/Dock attention has no command, output or authentication payload.
  await window.requestUserAttention(UserAttentionType.Informational);
}
