// Where the page is, and how it says where anything else is.
//
// The parsing is not here. `__dom.parseUrl` is the same crate that resolves
// every reference the document makes, so a router and an `<a href>` on the same
// page agree about what a path is — which a regular expression written here
// would eventually stop doing, on exactly the URL nobody tested.

(() => {
  // href, protocol, host, port, pathname, search, hash, origin — in the order
  // `Dom::parse_url` lists them. Empty when the URL does not parse.
  const parse = (href, base) => __dom.parseUrl(String(href), base ? String(base) : undefined);

  const parts = (from) => ({
    href: from[0],
    protocol: from[1],
    hostname: from[2],
    port: from[3],
    pathname: from[4],
    search: from[5],
    hash: from[6],
    origin: from[7],
    host: from[3] ? `${from[2]}:${from[3]}` : from[2],
  });

  class URLSearchParams {
    constructor(init = "") {
      this._pairs = [];
      if (typeof init === "string") {
        for (const pair of init.replace(/^\?/, "").split("&")) {
          if (!pair) continue;
          const at = pair.indexOf("=");
          const [name, value] = at < 0 ? [pair, ""] : [pair.slice(0, at), pair.slice(at + 1)];
          this._pairs.push([decode(name), decode(value)]);
        }
      } else if (init && typeof init === "object") {
        const entries = typeof init.forEach === "function" && !Array.isArray(init)
          ? [] : Object.entries(init);
        for (const [name, value] of Array.isArray(init) ? init : entries) {
          this._pairs.push([String(name), String(value)]);
        }
      }
    }
    get(name) {
      const found = this._pairs.find(([key]) => key === name);
      return found ? found[1] : null;
    }
    getAll(name) {
      return this._pairs.filter(([key]) => key === name).map(([, value]) => value);
    }
    has(name) {
      return this._pairs.some(([key]) => key === name);
    }
    set(name, value) {
      this.delete(name);
      this.append(name, value);
    }
    append(name, value) {
      this._pairs.push([String(name), String(value)]);
    }
    delete(name) {
      this._pairs = this._pairs.filter(([key]) => key !== name);
    }
    forEach(visit, thisArg) {
      for (const [name, value] of this._pairs) visit.call(thisArg, value, name, this);
    }
    keys() {
      return this._pairs.map(([name]) => name)[Symbol.iterator]();
    }
    values() {
      return this._pairs.map(([, value]) => value)[Symbol.iterator]();
    }
    entries() {
      return this._pairs.map((pair) => pair.slice())[Symbol.iterator]();
    }
    [Symbol.iterator]() {
      return this.entries();
    }
    toString() {
      return this._pairs
        .map(([name, value]) => `${encodeURIComponent(name)}=${encodeURIComponent(value)}`)
        .join("&");
    }
  }

  const decode = (text) => {
    try {
      return decodeURIComponent(text.replace(/\+/g, " "));
    } catch {
      return text;
    }
  };

  class URL {
    constructor(href, base) {
      const from = parse(href, base);
      if (!from.length) throw new TypeError(`Invalid URL: ${href}`);
      Object.assign(this, parts(from));
      this.searchParams = new URLSearchParams(this.search);
      this.username = "";
      this.password = "";
    }
    toString() {
      return this.href;
    }
    toJSON() {
      return this.href;
    }
  }

  globalThis.URL = URL;
  globalThis.URLSearchParams = URLSearchParams;

  // The page's own address, taken apart the same way.
  //
  // Resolved from the document's own base rather than left at `about:blank`
  // until someone says otherwise: a page's scripts run as it loads, and a
  // router reading `location.pathname` at that moment used to be told the page
  // was somewhere it has never been. The browser sets `href` again later — see
  // `Realm::set_environment` — which is why it is a property, and why
  // everything else here is derived from it rather than stored beside it.
  const address = { ...parts(parse("")) };
  globalThis.location = {
    get href() {
      return address.href;
    },
    set href(value) {
      const from = parse(value);
      Object.assign(address, from.length ? parts(from) : { href: String(value) });
    },
    get protocol() { return address.protocol; },
    get host() { return address.host; },
    get hostname() { return address.hostname; },
    get port() { return address.port; },
    get origin() { return address.origin; },
    // Writable, the way a browser's are. A router setting `location.hash` or
    // `location.pathname` is navigating within the page, and a property with
    // only a getter throws at it — which is how a page that was merely changing
    // a tab took its own hovercards down with it.
    get pathname() { return address.pathname; },
    set pathname(value) { reach({ pathname: String(value) }); },
    get search() { return address.search; },
    set search(value) { reach({ search: with_mark(value, "?") }); },
    get hash() { return address.hash; },
    set hash(value) { reach({ hash: with_mark(value, "#") }); },
    assign(value) { globalThis.location.href = value; },
    replace(value) { globalThis.location.href = value; },
    reload() {},
    toString() { return address.href; },
  };

  // One part of the address replaced, and the rest kept. Spelled out rather
  // than assigned into `address` directly so that every part stays derived from
  // one `href` — two fields that can disagree about where the page is are two
  // answers to a question that has one.
  const reach = (change) => {
    const next = { ...address, ...change };
    globalThis.location.href = `${next.origin === "null" ? "" : next.origin}${next.pathname}${next.search}${next.hash}`;
  };

  const with_mark = (value, mark) => {
    const text = String(value);
    return !text || text.startsWith(mark) ? text : mark + text;
  };

  // Enough of the history API for a router to run. Nothing navigates: a page
  // pushing a state is telling its own code where it thinks it is, and that is
  // the part this has to get right.
  let entries = [{ state: null, url: null }];
  let at = 0;
  globalThis.history = {
    get length() { return entries.length; },
    get state() { return entries[at].state; },
    scrollRestoration: "auto",
    pushState(state, _title, url) {
      entries = entries.slice(0, at + 1);
      entries.push({ state, url: url ?? null });
      at = entries.length - 1;
      if (url != null) globalThis.location.href = new URL(url, address.href).href;
    },
    replaceState(state, _title, url) {
      entries[at] = { state, url: url ?? null };
      if (url != null) globalThis.location.href = new URL(url, address.href).href;
    },
    go(delta = 0) {
      at = Math.min(Math.max(at + delta, 0), entries.length - 1);
    },
    back() { this.go(-1); },
    forward() { this.go(1); },
  };
})();
