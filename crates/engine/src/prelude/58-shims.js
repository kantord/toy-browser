// What a page asks the platform for that this platform does not have.
//
// Everything here is an approximation, and each one is here for the same
// reason: a page that reads a missing global does not degrade, it stops at that
// line. An approximate answer lets the rest of the page run; no answer at all
// loses everything after it.

(() => {
  // QuickJS is built without ECMA-402, so `Intl` is absent rather than partial.
  // These format in one locale — the machine's idea of English — and ignore
  // most options. A page showing a date gets a date; a page relying on Japanese
  // era names does not, and should read this comment rather than the output.
  const pad = (number) => String(number).padStart(2, "0");

  class DateTimeFormat {
    constructor(locales, options = {}) {
      this._options = options;
      this._locale = Array.isArray(locales) ? locales[0] : locales || "en-US";
    }
    format(when) {
      const date = when instanceof Date ? when : new Date(when ?? Date.now());
      const { hour, minute, year, month, day } = this._options;
      const clock = `${pad(date.getHours())}:${pad(date.getMinutes())}`;
      const calendar = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
      if ((hour || minute) && !(year || month || day)) return clock;
      if (hour || minute) return `${calendar} ${clock}`;
      return calendar;
    }
    formatToParts(when) {
      return [{ type: "literal", value: this.format(when) }];
    }
    resolvedOptions() {
      return { locale: this._locale, timeZone: "UTC", calendar: "gregory", numberingSystem: "latn" };
    }
  }

  class NumberFormat {
    constructor(locales, options = {}) {
      this._options = options;
      this._locale = Array.isArray(locales) ? locales[0] : locales || "en-US";
    }
    format(value) {
      const digits = this._options.maximumFractionDigits;
      const number = typeof digits === "number" ? Number(value).toFixed(digits) : String(value);
      // Thousands separators, which is the one piece of formatting a page can
      // see the absence of at a glance.
      const [whole, fraction] = String(number).split(".");
      const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
      return fraction ? `${grouped}.${fraction}` : grouped;
    }
    formatToParts(value) {
      return [{ type: "literal", value: this.format(value) }];
    }
    resolvedOptions() {
      return { locale: this._locale, numberingSystem: "latn", style: this._options.style || "decimal" };
    }
  }

  class RelativeTimeFormat {
    constructor(locales, options = {}) {
      this._options = options;
      this._locale = Array.isArray(locales) ? locales[0] : locales || "en-US";
    }
    format(value, unit) {
      const count = Math.abs(value);
      const name = count === 1 ? unit : `${unit}s`;
      return value < 0 ? `${count} ${name} ago` : `in ${count} ${name}`;
    }
    formatToParts(value, unit) {
      return [{ type: "literal", value: this.format(value, unit) }];
    }
    resolvedOptions() {
      return { locale: this._locale, numeric: this._options.numeric || "always" };
    }
  }

  class Collator {
    constructor(locales) {
      this._locale = Array.isArray(locales) ? locales[0] : locales || "en-US";
    }
    compare(left, right) {
      return String(left) < String(right) ? -1 : String(left) > String(right) ? 1 : 0;
    }
    resolvedOptions() {
      return { locale: this._locale };
    }
  }

  globalThis.Intl = {
    DateTimeFormat,
    NumberFormat,
    RelativeTimeFormat,
    Collator,
    getCanonicalLocales: (locales) =>
      (Array.isArray(locales) ? locales : [locales]).filter(Boolean).map(String),
  };

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
