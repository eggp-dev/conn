// Exercise the actual native bootstrap and shared product UI with low-level IPC fixtures.
import { mount, unmount } from 'svelte';
import NativeRoot from '../../../../frontends/tauri/src/NativeRoot.svelte';
import { inspect, fenceNext } from './native-host';
import '../../src/styles/app.css';
const component = mount(NativeRoot, {target:document.getElementById('app')!});
(window as any).nativeLifetime = { inspect, fenceNext, unmount:()=>unmount(component) };
