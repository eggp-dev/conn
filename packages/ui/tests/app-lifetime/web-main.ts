// Actual web bootstrap and transport; the test intercepts HTTP/WS at the host boundary.
import { mount } from 'svelte';
import WebRoot from '../../../../frontends/web/src/WebRoot.svelte';
import '../../src/styles/app.css';
mount(WebRoot, {target:document.getElementById('app')!});
