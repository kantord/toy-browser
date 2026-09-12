// What a page can ask for after it has loaded, and who it says it is.
//
// `fetch` reads through the same cache the document and its scripts came
// through — see `Dom::read`. A page that fetches a file it already has gets the
// bytes it already has, and gets the same answer the `<img>` beside it would.
//
// The wait is not real. The cache is blocking, there is no thread for a request
// to happen on, and the promise handed back is already settled by the time the
// page holds it. What that costs is ordering: a page racing two fetches sees
// them finish in the order it started them, always.

(() => {
  // A real one, because a page builds these and reads them back. The previous
  // version took no constructor argument and had no `set` — and a page doing
  // `new Headers({ Accept: "application/json" })` and then `headers.set(...)`
  // got a TypeError in the middle of building a request, which is why
  // hcker.news reached the line before every one of its API calls and made
  // none of them.
  //
  // Names are matched without case, as HTTP does, and kept in the order they
  // arrived.
  class Headers {
    constructor(init) {
      this._pairs = [];
      if (init instanceof Headers) {
        for (const [name, value] of init._pairs) this.append(name, value);
      } else if (Array.isArray(init)) {
        for (const [name, value] of init) this.append(name, value);
      } else if (init && typeof init === "object") {
        for (const [name, value] of Object.entries(init)) this.append(name, value);
      }
    }
    append(name, value) {
      this._pairs.push([String(name).toLowerCase(), String(value)]);
    }
    set(name, value) {
      this.delete(name);
      this.append(name, value);
    }
    delete(name) {
      const wanted = String(name).toLowerCase();
      this._pairs = this._pairs.filter(([held]) => held !== wanted);
    }
    // Several of the same name read as one comma-separated value, which is what
    // HTTP says they mean.
    get(name) {
      const wanted = String(name).toLowerCase();
      const found = this._pairs.filter(([held]) => held === wanted).map(([, value]) => value);
      return found.length ? found.join(", ") : null;
    }
    has(name) {
      return this._pairs.some(([held]) => held === String(name).toLowerCase());
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
  }

  // What the bytes look like, since the cache keeps no exchange to ask.
  //
  // A page that checks `response.headers.get("content-type")` before parsing —
  // which a careful one does — reads `null` from a header set that is empty,
  // and treats a perfectly good answer as the wrong kind of thing.
  const kindOf = (body) => {
    const start = String(body ?? "").trimStart().slice(0, 1);
    if (start === "{" || start === "[") return "application/json";
    if (start === "<") return "text/html; charset=utf-8";
    return "text/plain; charset=utf-8";
  };

  class Response {
    constructor(body, { url = "", status = 200 } = {}) {
      this._body = body;
      this.url = url;
      this.status = status;
      this.ok = status >= 200 && status < 300;
      this.statusText = this.ok ? "OK" : "Not Found";
      this.redirected = false;
      this.type = "basic";
      this.bodyUsed = false;
      this.headers = new Headers({ "content-type": kindOf(body) });
    }

    text() {
      this.bodyUsed = true;
      return Promise.resolve(this._body);
    }

    json() {
      this.bodyUsed = true;
      // Parsed in the promise rather than before it, so a file that is not
      // JSON rejects the way a page expects instead of throwing at the call.
      return Promise.resolve().then(() => JSON.parse(this._body));
    }

    clone() {
      return new Response(this._body, { url: this.url, status: this.status });
    }
  }

  // Not every environment has one, and a page catching by name expects it.
  if (typeof globalThis.DOMException === "undefined") {
    globalThis.DOMException = class DOMException extends Error {
      constructor(message, name = "Error") {
        super(message);
        this.name = name;
      }
    };
  }

  // What a page hands to `fetch` when it wants to say more than a URL. Built
  // here rather than left out because a page that constructs one and finds no
  // such name stops there, and the request it was describing is never made.
  class Request {
    constructor(input, init = {}) {
      const from = input instanceof Request ? input : null;
      this.url = String(from ? from.url : input);
      this.method = String(init.method ?? from?.method ?? "GET").toUpperCase();
      this.headers = new Headers(init.headers ?? from?.headers);
      this.body = init.body ?? from?.body ?? null;
      this.mode = init.mode ?? "cors";
      this.credentials = init.credentials ?? "same-origin";
      this.cache = init.cache ?? "default";
      this.redirect = init.redirect ?? "follow";
      this.referrer = init.referrer ?? "about:client";
      this.integrity = init.integrity ?? "";
      this.keepalive = init.keepalive ?? false;
      this.signal = init.signal ?? from?.signal ?? null;
      this.bodyUsed = false;
    }
    clone() {
      return new Request(this);
    }
    text() {
      return Promise.resolve(this.body == null ? "" : String(this.body));
    }
    json() {
      return this.text().then((it) => JSON.parse(it));
    }
  }

  // What each says it is when asked, which is what `Object.prototype.toString`
  // reads and so what a page's own type check reads too. Without it every one
  // of them is an anonymous `[object Object]`.
  for (const [made, name] of [[Headers, "Headers"], [Request, "Request"], [Response, "Response"]]) {
    Object.defineProperty(made.prototype, Symbol.toStringTag, {
      value: name,
      configurable: true,
    });
  }

  globalThis.Headers = Headers;
  globalThis.Request = Request;
  globalThis.Response = Response;

  // `input` may be a string, a URL, or a Request.
  //
  // Only the URL is honoured. The cache reads what a GET would read, so a
  // method, a body and a header set are taken and ignored — a page may say
  // them, and asking for something this cannot do answers as though it had
  // been asked plainly rather than refusing.
  // Answered in a later task, never in this one.
  //
  // The cache can answer at once, and for a long time this did: the promise
  // handed back was already settled. That is not a shortcut, it is a different
  // page. An application asks for its data while it is still building the page
  // that will hold it, and the answer arriving *first* puts the response
  // handler in front of the render it was waiting for — so it fills in a
  // document that does not exist yet, finds nothing, and reports that it could
  // not load. hcker.news fetched eighty stories, drew none of them, and said
  // "couldn't load this feed" about data it was already holding.
  //
  // A task rather than a microtask because that is the difference that matters:
  // microtasks run before the page gets to do anything else, and everything a
  // page does between asking and being answered — a timer, an animation frame,
  // its own DOMContentLoaded work — is a task.
  globalThis.fetch = (input, init = {}) =>
    new Promise((settle, fail) => {
      const request = input instanceof Request ? input : new Request(input, init);
      setTimeout(() => {
        if (request.signal && request.signal.aborted) {
          fail(new DOMException("aborted", "AbortError"));
          return;
        }
        const [resolved, body, status, failed] = __dom.read(request.url);
        // Only a failure to ask is a network error. A status the page did not
        // want — a 404 — comes back as a Response it can read, which is what
        // the specification says and what a page's own error handling is
        // written for.
        if (failed) {
          fail(new TypeError(failed));
          return;
        }
        settle(new Response(body, { url: resolved, status: Number(status) }));
      }, 0);
    });

  // What a service worker registration looks like from the page, holding no
  // worker: nothing is installing, nothing is waiting, nothing is active.
  const registration = {
    scope: "/",
    active: null,
    installing: null,
    waiting: null,
    updateViaCache: "none",
    update: () => Promise.resolve(),
    unregister: () => Promise.resolve(true),
    addEventListener() {},
    removeEventListener() {},
  };

  // Who the page is talking to. Read far more often than it is acted on — a
  // script asking for `navigator.userAgent` and finding nothing there stops at
  // that line, which is how three of hcker.news's four scripts died.
  globalThis.navigator = {
    userAgent: "Mozilla/5.0 (X11; Linux x86_64) toy-browser",
    appName: "Netscape",
    appVersion: "5.0 (X11)",
    platform: "Linux x86_64",
    vendor: "",
    product: "Gecko",
    language: "en-US",
    languages: ["en-US", "en"],
    onLine: true,
    cookieEnabled: true,
    hardwareConcurrency: 1,
    maxTouchPoints: 0,
    // iOS says whether a page was opened from the home screen. It is not a
    // standard, and a page reading it on anything else expects `undefined`
    // rather than a throw — which is the whole of what it needs from us.
    standalone: undefined,
    webdriver: false,
    doNotTrack: null,
    // No service workers: one is a background thread, and there is none.
    //
    // Answered with a registration that holds nothing, rather than with a
    // rejection or with a promise that never settles. Both of those were tried
    // and both are worse: almost nobody catches `register`, so a rejection
    // stops the boot it was written in the middle of — and a `ready` that never
    // settles hangs whatever waited for it, silently and for ever, which is the
    // hardest kind of nothing to debug.
    serviceWorker: {
      controller: null,
      register: () => Promise.resolve(registration),
      getRegistration: () => Promise.resolve(registration),
      getRegistrations: () => Promise.resolve([registration]),
      ready: Promise.resolve(registration),
      addEventListener() {},
      removeEventListener() {},
    },
    sendBeacon: () => false,
    clipboard: {
      writeText: () => Promise.resolve(),
      readText: () => Promise.resolve(""),
    },
  };
})();
