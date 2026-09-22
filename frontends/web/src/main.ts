import { mount } from 'svelte';
import WebRoot from './WebRoot.svelte';
import '@conn/ui/styles.css';
mount(WebRoot, { target: document.getElementById('app')! });
