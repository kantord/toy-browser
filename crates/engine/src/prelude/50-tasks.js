// Queued work: timers, animation frames, and the custom element upgrades that
// are scheduled the same way.
//
// Nothing runs on its own. Work piles up until the lifecycle drains it, which
// is why a load is reproducible and why an interval can only fire once.

(() => {
  const tb = globalThis.__tb;

  /// How much time an idle callback is told it has, in milliseconds. Nothing
  /// here is competing for the main thread, so the answer is the largest one
  /// the specification allows a browser to give.
  const IDLE = 50;

  // The queue lives in Rust — see `realm/node/tasks.rs` — because it retains a
  // page's callbacks and has to release them with the Realm. What stays here is
  // the aliasing: a single load produces one frame, so an interval is a
  // timeout, and there is no idle to wait for.
  globalThis.setInterval = globalThis.setTimeout;
  globalThis.clearInterval = globalThis.clearTimeout;
  globalThis.cancelAnimationFrame = globalThis.clearTimeout;
  // An idle callback is handed a deadline, and the first thing anything written
  // against this API does is ask it how long it has. Aliasing this to
  // `setTimeout` calls the callback with no argument at all, so that first
  // question throws — inside a callback, where nothing is watching — and
  // whatever was going to be done during the idle moment is simply never done.
  // A page that renders its rows this way renders none of them and reports
  // nothing.
  globalThis.requestIdleCallback = (callback, options = {}) =>
    globalThis.setTimeout(
      () => callback({ didTimeout: false, timeRemaining: () => IDLE }),
      options.timeout ? 0 : 0,
    );
  globalThis.cancelIdleCallback = globalThis.clearTimeout;

  globalThis.queueMicrotask = (callback) => {
    Promise.resolve().then(callback);
  };

  tb.runTask = (callback, args = []) => {
    try {
      callback(...args);
    } catch (error) {
      __console.error(`task threw: ${error}`);
    }
  };

  tb.drainTasks = __dom.drainTasks;

  globalThis.customElements = {
    __definitions: new Map(),
    define(name, constructor) {
      this.__definitions.set(name, constructor);
      // Upgrade what is already in the tree. The element keeps its own
      // wrapper rather than becoming an instance of `constructor`, so the
      // constructor never runs — only the lifecycle callbacks do.
      for (const id of __dom.elementsByTag(name)) {
        const element = tb.wrap(id);
        Object.setPrototypeOf(element, constructor.prototype);
        tb.runTask(() => element.connectedCallback?.());
      }
    },
    get(name) {
      return this.__definitions.get(name);
    },
  };
})();
