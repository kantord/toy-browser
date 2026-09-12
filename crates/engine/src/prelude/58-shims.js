// What a page asks the platform for that this platform does not have.
//
// Everything here is an approximation, and each one is here for the same
// reason: a page that reads a missing global does not degrade, it stops at that
// line. An approximate answer lets the rest of the page run; no answer at all
// loses everything after it.

(() => {
  // Media queries, answered from what this browser actually is: a light colour
  // scheme, no pointer of its own to speak of, at whatever width the window
  // said. A query it cannot read answers `false`, which is what a page treats
  // as "the ordinary case".
  const answers = (query) => {
    const text = String(query);
    const width = (kind) => {
      const found = text.match(new RegExp(`${kind}-width:\\\\s*(\\\\d+(?:\\\\.\\\\d+)?)px`));
      return found ? Number(found[1]) : null;
    };
    // Whichever scheme this page is being shown in, which is the same thing
    // the cascade was told — a page whose script disagrees with its own
    // stylesheet about the scheme renders half of each.
    const scheme = __tb.scheme || "light";
    if (/prefers-color-scheme:\s*light/.test(text)) return scheme === "light";
    if (/prefers-color-scheme:\s*dark/.test(text)) return scheme === "dark";
    if (/prefers-reduced-motion:\s*reduce/.test(text)) return true;
    const most = width("max");
    const least = width("min");
    if (most !== null && globalThis.innerWidth > most) return false;
    if (least !== null && globalThis.innerWidth < least) return false;
    return most !== null || least !== null;
  };

  // `randomUUID` and `getRandomValues`, which is all a page usually wants —
  // for a key in a list, a request id, a cache buster. Not cryptography: these
  // come from `Math.random`, and anything that needs unguessable numbers must
  // not ask this browser for them.
  globalThis.crypto = {
    randomUUID() {
      const hex = (count) =>
        Array.from({ length: count }, () =>
          Math.floor(Math.random() * 16).toString(16),
        ).join("");
      // Version 4, variant 1: the two fixed nibbles are what tells a reader
      // which kind of UUID this claims to be.
      return `${hex(8)}-${hex(4)}-4${hex(3)}-${"89ab"[Math.floor(Math.random() * 4)]}${hex(3)}-${hex(12)}`;
    },
    getRandomValues(into) {
      for (let at = 0; at < into.length; at += 1) {
        into[at] = Math.floor(Math.random() * 0x100000000);
      }
      return into;
    },
    subtle: undefined,
  };

  // Cancelling work a page has started. Nothing here can actually stop a fetch
  // — the cache answers before the signal could be read — so what this does is
  // let the page say it, and let anything waiting on `aborted` or the `abort`
  // event hear it. Chromium without this renders a fifth of hcker.news.
  // What an abort means when nobody said why. Not an `Error`: a page tells a
  // cancellation from a failure by `error.name === "AbortError"`, and an
  // ordinary Error is named "Error" — so a request machinery written to ignore
  // its own cancellations reports a network failure for every one of them.
  const cancelled = () =>
    new DOMException("signal is aborted without reason", "AbortError");

  class AbortSignal {
    constructor() {
      this.aborted = false;
      this.reason = undefined;
      this.onabort = null;
      this._listeners = [];
    }
    addEventListener(type, listener) {
      if (type === "abort") this._listeners.push(listener);
    }
    removeEventListener(type, listener) {
      if (type === "abort") this._listeners = this._listeners.filter((it) => it !== listener);
    }
    dispatchEvent() {
      return true;
    }
    throwIfAborted() {
      if (this.aborted) throw this.reason;
    }
    static abort(reason) {
      const signal = new AbortSignal();
      signal.aborted = true;
      signal.reason = reason ?? cancelled();
      return signal;
    }
    static timeout() {
      return new AbortSignal();
    }
  }

  class AbortController {
    constructor() {
      this.signal = new AbortSignal();
    }
    abort(reason) {
      if (this.signal.aborted) return;
      this.signal.aborted = true;
      this.signal.reason = reason ?? cancelled();
      const event = { type: "abort", target: this.signal };
      if (typeof this.signal.onabort === "function") this.signal.onabort(event);
      for (const listener of this.signal._listeners.slice()) listener(event);
    }
  }

  globalThis.AbortSignal = AbortSignal;
  globalThis.AbortController = AbortController;

  // A deep copy, which is what a page uses it for. Structured clone can carry
  // things JSON cannot — a Map, a Date, a cycle — and this cannot; what it can
  // do is stop the call throwing.
  globalThis.structuredClone = (value) => {
    const seen = new WeakMap();
    const copy = (it) => {
      if (it === null || typeof it !== "object") return it;
      if (seen.has(it)) return seen.get(it);
      if (it instanceof Date) return new Date(it.getTime());
      if (it instanceof Map) return new Map([...it].map(([k, v]) => [copy(k), copy(v)]));
      if (it instanceof Set) return new Set([...it].map(copy));
      const made = Array.isArray(it) ? [] : {};
      seen.set(it, made);
      for (const [name, held] of Object.entries(it)) made[name] = copy(held);
      return made;
    };
    return copy(value);
  };

  globalThis.matchMedia = (query) => ({
    media: String(query),
    matches: answers(query),
    onchange: null,
    // Nothing here ever changes, so a listener is remembered and never called
    // rather than refused: a page that cannot add one stops at that line.
    addListener() {},
    removeListener() {},
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => false,
  });
})();
