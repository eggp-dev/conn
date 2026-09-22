type Size = { rows: number; cols: number };

/** Submit size changes immediately to the common owner command queue. Keeping a
 * second pending-size buffer here would reorder a resize across intervening input.
 * Only consecutive identical measurements are redundant.
 */
export function terminalResizeQueue(send: (size: Size) => Promise<unknown>, onError: (error: unknown) => void) {
  let requested: Size | undefined;
  let revision = 0;
  let disposed = false;
  return {
    resize(size: Size) {
      if (disposed || (requested?.rows === size.rows && requested.cols === size.cols)) return;
      requested = { ...size };
      const current = ++revision;
      void send({ ...size }).catch(error => {
        if (disposed) return;
        if (revision === current) requested = undefined;
        onError(error);
      });
    },
    invalidate() { requested = undefined; revision++; },
    dispose() { disposed = true; requested = undefined; },
  };
}
