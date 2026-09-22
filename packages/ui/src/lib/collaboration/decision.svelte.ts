/** A decision stays retryable after failure. Never hide a request in finally. */
export function decision() {
  let busy = $state(false);
  let error = $state<unknown>(null);
  return {
    get busy() { return busy; }, get error() { return error; },
    async run(action: () => Promise<unknown>) {
      if (busy) return false;
      busy = true; error = null;
      try { await action(); return true; }
      catch (cause) { error = cause; return false; }
      finally { busy = false; }
    },
  };
}

export function createRequestActions() {
  // Keyboard and card buttons share progress/error state for the same request.
  const actions = new Map<string, ReturnType<typeof decision>>();
  function requestAction(key: string) {
    let action = actions.get(key);
    if (!action) {
      for (const [id, old] of actions) if (actions.size >= 128 && !old.busy) actions.delete(id);
      action = decision(); actions.set(key, action);
    }
    return action;
  }

  return { requestAction, forgetSession(session: string) { for (const key of actions.keys()) if (key.startsWith(`control:${session}:`) || key.startsWith(`approval:${session}:`)) actions.delete(key); }, dispose() { actions.clear(); } };
}
