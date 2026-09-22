type Size = { rows: number; cols: number };

/** One resize in flight per terminal, retaining the newest pending dimensions.
 * Native IPC workers can finish in a different order from invocation. Sending
 * the next size only after acknowledgement keeps an older size from winning.
 * Intermediate window/dock sizes do not need their own PTY redraw.
 */
export function terminalResizeQueue(send: (size: Size) => Promise<unknown>, onError: (error: unknown) => void) {
  let pending: Size | undefined;
  let applied: Size | undefined;
  let running = false;
  let disposed = false;

  async function drain() {
    running = true;
    try {
      while (!disposed && pending) {
        const size = pending;
        pending = undefined;
        if (applied?.rows === size.rows && applied.cols === size.cols) continue;
        try { await send(size); applied = size; }
        catch (error) { if (!disposed) onError(error); }
      }
    } finally { running = false; }
  }

  return {
    resize(size: Size) {
      if (disposed) return;
      pending = { ...size };
      if (!running) void drain();
    },
    dispose() { disposed = true; pending = undefined; },
  };
}
