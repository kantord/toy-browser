// What a page asks about a language rather than about a value.
//
// Split from `58-intl.js` because the two move for different reasons: that one
// when a page wants another way of writing a date or a number, this one when it
// wants to know something about the language it is writing in — how a count
// changes a word, how a list is joined, what a region code is called.
//
// Every one of these is English-only and approximate. They are here for the
// reason everything in the shims is here: `new Intl.PluralRules(...)` on a
// browser without one is not a missing feature, it is a TypeError thrown by the
// engine — invisible to any error tracing, because the engine does not build it
// by calling `TypeError` — and a page that catches it reports its own polite
// failure about work it had already done. A story row saying "142 comments"
// asks a `PluralRules` how to spell the word.

(() => {
  const first = (locales) => String((Array.isArray(locales) ? locales[0] : locales) || "en-US");

  // English: one of a thing, or any number of them. Nothing here knows about
  // the languages with a dual, a paucal, or a separate form for eleven.
  class PluralRules {
    constructor(locales, options = {}) {
      this._locale = first(locales);
      this._type = options.type === "ordinal" ? "ordinal" : "cardinal";
    }
    select(count) {
      const number = Number(count);
      if (this._type === "cardinal") return Math.abs(number) === 1 ? "one" : "other";
      // 1st, 2nd, 3rd, 4th — and the teens, which are all "th".
      const last = Math.abs(number) % 10;
      const teen = Math.abs(number) % 100;
      if (teen >= 11 && teen <= 13) return "other";
      return { 1: "one", 2: "two", 3: "few" }[last] ?? "other";
    }
    selectRange(from, to) {
      return this.select(to);
    }
    resolvedOptions() {
      return {
        locale: this._locale,
        type: this._type,
        pluralCategories: ["one", "other"],
        minimumIntegerDigits: 1,
      };
    }
    static supportedLocalesOf() {
      return [];
    }
  }

  /// How a list of things is joined: "a, b and c", or "a, b or c".
  const JOINS = { conjunction: "and", disjunction: "or", unit: "" };

  class ListFormat {
    constructor(locales, options = {}) {
      this._locale = first(locales);
      this._type = options.type || "conjunction";
      this._style = options.style || "long";
    }
    formatToParts(list) {
      const items = Array.from(list ?? []).map(String);
      const word = JOINS[this._type] ?? JOINS.conjunction;
      // The last pair is joined by the word; everything before it by a comma.
      // Two items take no comma at all — "a and b", never "a, and b".
      const parts = [];
      items.forEach((item, at) => {
        if (at > 0) {
          const last = at === items.length - 1;
          const separator = !word ? ", " : last ? (items.length === 2 ? ` ${word} ` : `, ${word} `) : ", ";
          parts.push({ type: "literal", value: separator });
        }
        parts.push({ type: "element", value: item });
      });
      return parts;
    }
    format(list) {
      return this.formatToParts(list)
        .map((part) => part.value)
        .join("");
    }
    resolvedOptions() {
      return { locale: this._locale, type: this._type, style: this._style };
    }
    static supportedLocalesOf() {
      return [];
    }
  }

  // The name of a region, a language, a currency. Nothing here has the tables
  // that turn "US" into "United States", so the honest answer is the code
  // itself — which is what a page displays, rather than nothing at all or a
  // thrown call.
  class DisplayNames {
    constructor(locales, options = {}) {
      this._locale = first(locales);
      this._type = options.type || "language";
      this._fallback = options.fallback || "code";
    }
    of(code) {
      const said = String(code);
      return this._fallback === "none" ? undefined : said;
    }
    resolvedOptions() {
      return { locale: this._locale, type: this._type, fallback: this._fallback, style: "long" };
    }
    static supportedLocalesOf() {
      return [];
    }
  }

  /// A language tag taken apart: `en-Latn-US` is a language, a script and a
  /// region, in that order, and any of the last two may be absent.
  class Locale {
    constructor(tag, options = {}) {
      const parts = String(tag).replace(/_/g, "-").split("-");
      this.language = (parts[0] || "und").toLowerCase();
      this.script = undefined;
      this.region = undefined;
      for (const part of parts.slice(1)) {
        if (part.length === 4) this.script = part[0].toUpperCase() + part.slice(1).toLowerCase();
        else if (part.length === 2 || part.length === 3) this.region = part.toUpperCase();
      }
      Object.assign(this, options);
      this.baseName = [this.language, this.script, this.region].filter(Boolean).join("-");
      this.calendar = options.calendar;
      this.numberingSystem = options.numberingSystem;
    }
    toString() {
      return this.baseName;
    }
    // Nothing here knows which script a language is usually written in, so
    // both answers are the tag as given.
    maximize() {
      return this;
    }
    minimize() {
      return this;
    }
  }

  // Two dates as one span. A real one collapses what the two have in common —
  // "12–14 September" — and this one does not.
  const range = globalThis.Intl.DateTimeFormat.prototype;
  range.formatRange = function (from, to) {
    return `${this.format(from)} – ${this.format(to)}`;
  };
  range.formatRangeToParts = function (from, to) {
    return [
      ...this.formatToParts(from).map((part) => ({ ...part, source: "startRange" })),
      { type: "literal", value: " – ", source: "shared" },
      ...this.formatToParts(to).map((part) => ({ ...part, source: "endRange" })),
    ];
  };

  Object.assign(globalThis.Intl, { PluralRules, ListFormat, DisplayNames, Locale });
})();
