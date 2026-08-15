// Queued work: timers, animation frames, and the custom element upgrades that
// are scheduled the same way.
//
// Nothing runs on its own. Work piles up until the lifecycle drains it, which
// is why a load is reproducible and why an interval can only fire once.

(() => {
  const tb = globalThis.__tb;

  const timers = [];
  const frames = [];
  let nextTimerId = 1;

  globalThis.setTimeout = (callback, delay = 0, ...args) => {
    const handle = nextTimerId++;
    timers.push({ handle, callback, delay: Number(delay) || 0, args });
    return handle;
  };
  globalThis.clearTimeout = (handle) => {
    const index = timers.findIndex((timer) => timer.handle === handle);
    if (index >= 0) timers.splice(index, 1);
  };
  // A single load produces one frame, so an interval is a timeout.
  globalThis.setInterval = globalThis.setTimeout;
  globalThis.clearInterval = globalThis.clearTimeout;

  globalThis.requestAnimationFrame = (callback) => {
    const handle = nextTimerId++;
    frames.push({ handle, callback });
    return handle;
  };
  globalThis.cancelAnimationFrame = globalThis.clearTimeout;
  globalThis.requestIdleCallback = globalThis.setTimeout;

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

  // Drains one round of queued work. Returns true while there is more.
  tb.drainTasks = () => {
    if (timers.length === 0 && frames.length === 0) return false;

    // Timers fire by delay, then by the order they were scheduled.
    const dueTimers = timers.splice(0).sort((a, b) => a.delay - b.delay || a.handle - b.handle);
    for (const timer of dueTimers) tb.runTask(timer.callback, timer.args);

    const dueFrames = frames.splice(0);
    for (const frame of dueFrames) tb.runTask(frame.callback, [0]);

    return true;
  };

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
