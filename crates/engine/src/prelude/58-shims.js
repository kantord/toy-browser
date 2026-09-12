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

  const MONTHS = ["January","February","March","April","May","June","July",
    "August","September","October","November","December"];
  const DAYS = ["Sunday","Monday","Tuesday","Wednesday","Thursday","Friday","Saturday"];

  class DateTimeFormat {
    constructor(locales, options = {}) {
      this._options = options;
      this._locale = String((Array.isArray(locales) ? locales[0] : locales) || "en-US");
      this._utc = options.timeZone === "UTC" || options.timeZone === "Etc/UTC";
    }

    // The pieces, typed, because a page that builds a key out of a date reads
    // them rather than the string: `parts.find((p) => p.type === "year")`. One
    // literal covering the whole date answers every such search with nothing,
    // and the key comes out empty — which is how a feed filtered by day threw
    // away every story it had.
    formatToParts(when) {
      const date = when instanceof Date ? when : new Date(when ?? Date.now());
      const get = (name) => (this._utc ? date[`getUTC${name}`]() : date[`get${name}`]());
      const { year, month, day, weekday, hour, minute, second } = this._options;
      // Nothing asked for is the same as asking for a date, which is what the
      // specification says the default is.
      const plain = !year && !month && !day && !weekday && !hour && !minute && !second;
      const parts = [];
      const pad2 = (value) => String(value).padStart(2, "0");
      const number = (kind, value) =>
        kind === "2-digit" ? pad2(value) : String(value);

      if (weekday) {
        const name = DAYS[get("Day")];
        parts.push({ type: "weekday", value: weekday === "short" ? name.slice(0, 3) : name });
        if (year || month || day) parts.push({ type: "literal", value: ", " });
      }
      const wantsDate = plain || year || month || day;
      if (wantsDate) {
        const y = { type: "year", value: number(year, get("FullYear")) };
        const monthKind = month || (plain ? "2-digit" : undefined);
        const m =
          monthKind === "short" || monthKind === "long"
            ? { type: "month", value: monthKind === "short" ? MONTHS[get("Month")].slice(0, 3) : MONTHS[get("Month")] }
            : { type: "month", value: number(monthKind, get("Month") + 1) };
        const d = { type: "day", value: number(day || (plain ? "2-digit" : undefined), get("Date")) };
        // Order and separator by locale — en-US puts the month first, the
        // ISO-ordered locales the year — and only the pieces that were asked
        // for. The separators are put *between* what survives rather than
        // written out with the full date: asking for a month and a day and
        // getting `-Sep-12` back is a key that matches nothing.
        const american = this._locale.startsWith("en-US");
        const wanted = (american ? [m, d, y] : [y, m, d]).filter((part) => {
          if (part.type === "year") return plain || !!year;
          if (part.type === "month") return plain || !!month;
          return plain || !!day;
        });
        // A named month reads with spaces, a numeric one with the locale's
        // separator: "September 12", not "September/12".
        const named = month === "short" || month === "long";
        const between = named ? " " : american ? "/" : "-";
        for (let at = 0; at < wanted.length; at += 1) {
          if (at) parts.push({ type: "literal", value: between });
          parts.push(wanted[at]);
        }
      }
      if (hour || minute || second) {
        if (wantsDate || weekday) parts.push({ type: "literal", value: ", " });
        const clock = [];
        if (hour) clock.push({ type: "hour", value: number(hour, get("Hours")) });
        if (minute) clock.push({ type: "minute", value: pad2(get("Minutes")) });
        if (second) clock.push({ type: "second", value: pad2(get("Seconds")) });
        for (let at = 0; at < clock.length; at += 1) {
          if (at) parts.push({ type: "literal", value: ":" });
          parts.push(clock[at]);
        }
      }
      return parts;
    }

    format(when) {
      return this.formatToParts(when)
        .map((part) => part.value)
        .join("");
    }

    resolvedOptions() {
      return {
        locale: this._locale,
        timeZone: this._options.timeZone || "UTC",
        calendar: "gregory",
        numberingSystem: "latn",
        ...this._options,
      };
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

  // Cancelling work a page has started. Nothing here can actually stop a fetch
  // — the cache answers before the signal could be read — so what this does is
  // let the page say it, and let anything waiting on `aborted` or the `abort`
  // event hear it. Chromium without this renders a fifth of hcker.news.
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
      signal.reason = reason ?? new Error("aborted");
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
      this.signal.reason = reason ?? new Error("aborted");
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
