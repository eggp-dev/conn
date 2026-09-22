import { getContext, setContext } from 'svelte';
import type { ConnAppState } from './app.svelte';
const context = Symbol('ConnApp');
export function provideConnApp(app: ConnAppState) { setContext(context, app); return app; }
export function useConnApp(): ConnAppState {
  const app = getContext<ConnAppState>(context);
  if (!app) throw new Error('Conn UI components must be mounted inside ConnApp');
  return app;
}
