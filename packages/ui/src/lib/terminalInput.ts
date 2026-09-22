/**
 * xterm 5.5's public onData event contains both user input and terminal replies.
 * Its internal onUserInput signal fires synchronously immediately before genuine
 * input. Keep this compatibility boundary small and covered against real xterm;
 * if the signal is unavailable, treating all input as human fails conservatively.
 */
export type InputOrigin = "human" | "terminal";
type Disposable = { dispose(): void };
type DataSource = { onData(listener: (data: string) => void): Disposable };
type InputSignal = { onUserInput(listener: () => void): Disposable };

export function observeTerminalInput(source: DataSource, receive: (data: string, origin: InputOrigin) => void): Disposable {
  const core = (source as DataSource & { _core?: { coreService?: Partial<InputSignal> } })._core?.coreService;
  let human = false;
  let signal: Disposable | undefined;
  if (typeof core?.onUserInput === "function") {
    try { signal = core.onUserInput(() => { human = true; }); }
    catch { /* Unknown xterm internals use the conservative path below. */ }
  }
  const hasSignal = typeof signal?.dispose === "function";
  let active = true;
  const dataListener = source.onData(data => {
    if (!active) return;
    const origin: InputOrigin = !hasSignal || human ? "human" : "terminal";
    human = false;
    receive(data, origin);
  });
  return { dispose() {
    if (!active) return;
    active = false;
    dataListener.dispose();
    if (hasSignal) signal!.dispose();
  } };
}
