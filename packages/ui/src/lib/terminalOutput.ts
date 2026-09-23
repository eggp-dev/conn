type Size = { rows: number; cols: number };
type Frame = { size: Size; data: Uint8Array; reset?: boolean; applied?: () => void };

/** xterm parses writes asynchronously. Apply the next frame's size only after
 * earlier bytes have been parsed, preserving the backend's resize/output order.
 */
export function terminalOutputQueue(resize: (size: Size) => void, write: (data: Uint8Array, done: () => void) => void, reset: () => void = () => {}) {
  const pending: Frame[] = [];
  let writing = false;
  let disposed = false;
  let generation = 0;
  function drain() {
    while (!disposed && !writing && pending.length) {
      const frame = pending.shift()!;
      if (frame.reset) reset();
      resize(frame.size);
      if (!frame.data.length) { frame.applied?.(); continue; }
      writing = true;
      const current = generation;
      write(frame.data, () => { writing = false; if (!disposed && current === generation) frame.applied?.(); drain(); });
    }
  }
  return {
    push(frame: Frame) { if (!disposed) { if (frame.reset) { generation++; pending.length = 0; } pending.push(frame); drain(); } },
    clear() { generation++; pending.length = 0; },
    dispose() { disposed = true; pending.length = 0; },
  };
}
