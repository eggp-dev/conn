/** A read started before an event cannot overwrite that event's state. */
export class Reconciler<T> {
  private disposed = false;
  dispose() { this.disposed = true; }
  private versions = new Map<string, number>();
  private reads = new Map<string, Promise<void>>();
  private read: (id: string) => Promise<T>;
  private apply: (id: string, snapshot: T) => void;
  constructor(read: (id: string) => Promise<T>, apply: (id: string, snapshot: T) => void) { this.read = read; this.apply = apply; }
  changed(id: string) { this.versions.set(id, (this.versions.get(id) ?? 0) + 1); }
  sync(id: string): Promise<void> {
    if (this.reads.has(id)) return this.reads.get(id)!;
    const work = (async () => {
      // Bounded retries under a busy output stream; the next poll reconciles again.
      for (let attempt = 0; attempt < 3; attempt++) {
        const version = this.versions.get(id) ?? 0;
        const snapshot = await this.read(id);
        if (this.disposed) return;
        if ((this.versions.get(id) ?? 0) === version) { this.apply(id, snapshot); return; }
      }
    })().finally(() => this.reads.delete(id));
    this.reads.set(id, work); return work;
  }
}
