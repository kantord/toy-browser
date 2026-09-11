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
  class Headers {
    // Enough of one to be read without throwing. Nothing here knows what a
    // response's headers were: the cache keeps bytes, not an exchange.
    get() {
      return null;
    }
    has() {
      return false;
    }
    forEach() {}
  }

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
      this.headers = new Headers();
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

  globalThis.Headers = Headers;
  globalThis.Response = Response;

  // `input` may be a string, a URL, or a Request-shaped object with a `url`.
  globalThis.fetch = (input, _init) => {
    const url = String(input && typeof input === "object" && "url" in input ? input.url : input);
    const [resolved, body, failed] = __dom.read(url);
    // A fetch that could not read rejects with a TypeError, which is what the
    // specification says a network error is and what every page catches.
    if (failed) return Promise.reject(new TypeError(failed));
    return Promise.resolve(new Response(body, { url: resolved, status: 200 }));
  };

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
