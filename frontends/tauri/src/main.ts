import { mount } from 'svelte';
import NativeRoot from './NativeRoot.svelte';
import '@conn/ui/styles.css';

mount(NativeRoot, { target: document.getElementById('app')! });
