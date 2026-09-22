type Size = { rows: number; cols: number };
type Frame = { size: Size; data: Uint8Array };

/** xterm parses writes asynchronously. Apply the next frame's size only after
 * earlier bytes have been parsed, preserving the backend's resize/output order.
 */
export function terminalOutputQueue(resize: (size: Size) => void, write: (data: Uint8Array, done: () => void) => void) {
  const pending: Frame[] = [];
  let writing = false;
  let disposed = false;
  function drain() {
    while (!disposed && !writing && pending.length) {
      const frame = pending.shift()!;
      resize(frame.size);
      if (!frame.data.length) continue;
      writing = true;
      write(frame.data, () => { writing = false; drain(); });
    }
  }
  return {
    push(frame: Frame) { if (!disposed) { pending.push(frame); drain(); } },
    dispose() { disposed = true; pending.length = 0; },
  };
}
