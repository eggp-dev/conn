/** Resources owned by one mounted app; disposal also fences late callbacks. */
export function createLifetime() {
  let disposed = false;
  const cleanups = new Set<() => void>();
  const timeouts = new Set<number>();
  return {
    get disposed() { return disposed; },
    own(cleanup: () => void) {
      let done = false;
      const stop = () => { if (done) return; done = true; cleanups.delete(stop); cleanup(); };
      if (disposed) stop(); else cleanups.add(stop);
      return stop;
    },
    timeout(callback: () => void, ms: number) {
      const id = window.setTimeout(() => { timeouts.delete(id); if (!disposed) callback(); }, ms);
      if (disposed) window.clearTimeout(id); else timeouts.add(id);
      return id;
    },
    clearTimeout(id: number) { timeouts.delete(id); window.clearTimeout(id); },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const id of timeouts) window.clearTimeout(id);
      timeouts.clear();
      for (const cleanup of [...cleanups]) cleanup();
      cleanups.clear();
    },
  };
}
export type Lifetime = ReturnType<typeof createLifetime>;
