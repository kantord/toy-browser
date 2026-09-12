// What a page asks about dates, numbers and text in a person's language.
//
// QuickJS is built without ECMA-402, so `Intl` is absent rather than partial.
// Everything here is an approximation, and each one is here for the same
// reason: a page that reads a missing global does not degrade, it stops at that
// line. An approximate answer lets the rest of the page run; no answer at all
// loses everything after it.
//
// Split from `58-shims.js` because the two move for different reasons: this one
// when a page wants another way of writing a date, that one when the platform
// gains another thing to answer about itself.

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

  // Text cut into pieces a person would recognise: characters, words,
  // sentences. Approximate — real segmentation is a Unicode algorithm with
  // tables, and this is a regular expression — but the shape is exact, because
  // what pages do with it is iterate and count. hcker.news asks for one while
  // rendering a story, and a missing constructor there is not a missing
  // feature: it throws, the render is abandoned, and the feed shows "couldn't
  // load this feed" about data it already has.
  const PIECES = {
    // Letters, numbers and marks stay together as one word-like run; runs of
    // anything else are the separators between them.
    word: /[\p{L}\p{N}\p{M}_']+|[^\p{L}\p{N}\p{M}_']+/gu,
    // A sentence runs to its closing punctuation and takes the spaces after it.
    sentence: /[^.!?]*[.!?]+[\s]*|[^.!?]+/gu,
  };

  class Segmenter {
    constructor(locales, options = {}) {
      this._granularity = options.granularity || "grapheme";
    }
    segment(text) {
      const input = String(text);
      const kind = this._granularity;
      const found = [];
      if (kind === "grapheme") {
        let at = 0;
        // By code point rather than by UTF-16 unit, so an emoji is one piece.
        for (const piece of Array.from(input)) {
          found.push({ segment: piece, index: at, input });
          at += piece.length;
        }
      } else {
        for (const match of input.matchAll(PIECES[kind] ?? PIECES.word)) {
          const piece = { segment: match[0], index: match.index, input };
          if (kind === "word") piece.isWordLike = /[\p{L}\p{N}]/u.test(match[0]);
          found.push(piece);
        }
      }
      // Iterable, and with `containing`, which is the other half of the
      // interface a page can lean on.
      return {
        [Symbol.iterator]: () => found[Symbol.iterator](),
        containing: (index = 0) =>
          found.find((piece) => index >= piece.index && index < piece.index + piece.segment.length),
      };
    }
    resolvedOptions() {
      return { locale: "en-US", granularity: this._granularity };
    }
  }

  // Callable without `new`, which is not a nicety: ECMA-402 keeps these three
  // callable for compatibility, and `Intl.DateTimeFormat().resolvedOptions()
  // .timeZone` is how every page in the world asks what time zone it is in. A
  // class refuses that call, and the refusal is a TypeError thrown by the
  // engine — so a page that wraps its request building in a `try` reports a
  // failed *request* and never makes one. hcker.news put that line in the
  // function that builds the headers for every one of its API calls.
  const callable = (Made) => {
    const either = function (...args) {
      return new Made(...args);
    };
    either.prototype = Made.prototype;
    Object.defineProperty(either, "name", { value: Made.name });
    // Nothing here is locale-aware, so the honest answer is that none of what
    // was asked for is supported beyond the one locale everything is in.
    either.supportedLocalesOf = () => [];
    return either;
  };

  globalThis.Intl = {
    DateTimeFormat: callable(DateTimeFormat),
    NumberFormat: callable(NumberFormat),
    // Not callable without `new`, and that is the specified behaviour: only the
    // three older constructors kept it.
    RelativeTimeFormat,
    Collator: callable(Collator),
    // Not callable without `new` either: it is one of the newer ones.
    Segmenter,
    getCanonicalLocales: (locales) =>
      (Array.isArray(locales) ? locales : [locales]).filter(Boolean).map(String),
  };

  // A date formatted through the same machinery, because QuickJS has its own
  // `toLocaleDateString` and it ignores every option it is handed: asking for
  // `{ weekday: "short", month: "short", day: "numeric" }` gives back
  // "09/10/2026" rather than "Thu, Sep 10". A page grouping stories by day
  // builds its headings out of exactly that call, and every day formats
  // identically to every other — so one heading swallows the lot, and the rest
  // of the feed is never drawn. Giving Chromium this browser's `Intl` takes it
  // from 81 rows to 21, which is how that was measured.
  const formatted = (when, locales, options) =>
    new DateTimeFormat(locales, options).format(when);
  const DATE = { year: "numeric", month: "numeric", day: "numeric" };
  const CLOCK = { hour: "2-digit", minute: "2-digit", second: "2-digit" };
  for (const [name, defaults] of [
    ["toLocaleDateString", DATE],
    ["toLocaleTimeString", CLOCK],
    ["toLocaleString", { ...DATE, ...CLOCK }],
  ]) {
    Date.prototype[name] = function (locales, options) {
      // The defaults apply only when nothing was asked for. Merging them into
      // what *was* asked for adds pieces the caller left out on purpose:
      // `{ weekday: "short", month: "short", day: "numeric" }` is "Thu, Sep 10"
      // and not "Thu, Sep 10 2026".
      const wanted = options && Object.keys(options).some((key) => key !== "timeZone");
      return formatted(this, locales, wanted ? options : { ...defaults, ...options });
    };
  }
})();
