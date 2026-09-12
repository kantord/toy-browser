// The names a page expects to find on `window` and never really uses: the
// per-tag element interfaces, the observers, the CSSOM.
//
// Nothing here carries behaviour. They exist because `instanceof` has to
// answer and because tooling reads these prototypes on load, failing the whole
// script if a name is missing.

(() => {
  const tb = globalThis.__tb;

  // The per-tag interfaces. Anything not listed stays a plain HTMLElement, as
  // it would in a browser.
  const defineInterface = (name, tags = []) => {
    const interface_ = class extends globalThis.HTMLElement {};
    Object.defineProperty(interface_, "name", { value: name });
    globalThis[name] = interface_;
    for (const tag of tags) __dom.registerInterface(tag, interface_.prototype);
  };

  defineInterface("HTMLInputElement", ["input"]);
  defineInterface("HTMLTextAreaElement", ["textarea"]);
  defineInterface("HTMLSelectElement", ["select"]);
  defineInterface("HTMLOptionElement", ["option"]);
  defineInterface("HTMLButtonElement", ["button"]);
  defineInterface("HTMLAnchorElement", ["a"]);
  defineInterface("HTMLImageElement", ["img"]);
  defineInterface("HTMLFormElement", ["form"]);
  defineInterface("HTMLLabelElement", ["label"]);
  defineInterface("HTMLIFrameElement", ["iframe"]);
  defineInterface("HTMLSlotElement", ["slot"]);
  defineInterface("HTMLBodyElement", ["body"]);
  defineInterface("HTMLHtmlElement", ["html"]);
  defineInterface("SVGElement", ["svg"]);
  defineInterface("Text", ["#text"]);
  defineInterface("Comment", ["#comment"]);
  defineInterface("DocumentFragment");
  defineInterface("ShadowRoot");

  // The document's own interface, so that a feature can be tested for the way
  // one is: `Document.prototype.hasOwnProperty("fonts")` rather than by asking
  // an instance. Test runners open with exactly that, and a missing name is a
  // thrown reference rather than a false answer.
  class Document {}
  // Every font this browser will ever have is already there — they are loaded
  // from disk before the page is — so the set is finished, `ready` is settled,
  // and nothing will ever be dispatched. It still has to be an *event target*:
  // a page that measures text waits for the fonts before trusting a
  // measurement, and it says so with
  //
  // ```js
  // document.fonts?.ready.then(again);
  // document.fonts?.addEventListener("loadingdone", again);
  // ```
  //
  // The `?.` protects a browser with no `document.fonts` at all. It does not
  // protect one that has the object and not the method: that line throws a
  // TypeError built by the engine, invisible to any error tracing, on the first
  // row a page tries to measure — which is how hcker.news came to draw none of
  // its eighty stories while holding all of them.
  Document.prototype.fonts = {
    ready: Promise.resolve(),
    status: "loaded",
    size: 0,
    addEventListener() {},
    removeEventListener() {},
    dispatchEvent: () => true,
    // Whatever was asked about, this browser has it: the faces are fixed.
    check: () => true,
    load: (font, text) => Promise.resolve([]),
    add() {},
    delete: () => false,
    clear() {},
    forEach() {},
    entries: () => [][Symbol.iterator](),
    keys: () => [][Symbol.iterator](),
    values: () => [][Symbol.iterator](),
    [Symbol.iterator]: () => [][Symbol.iterator](),
  };
  globalThis.Document = Document;
  globalThis.HTMLDocument = Document;

  // Nothing observes anything here: the DOM only changes while script is
  // running, and no client is watching when it does. These exist because
  // tooling constructs them on load and fails outright if the name is missing.
  class MutationObserver {
    observe() {}
    disconnect() {}
    takeRecords() {
      return [];
    }
  }
  // The per-tag interface names. A page uses them for `instanceof` far more
  // often than for anything else — `node instanceof HTMLScriptElement` is how
  // half the DOM-walking code on the web narrows a node — and a name that is
  // not defined throws rather than answering `false`.
  //
  // All the same class underneath, because this browser has one: what an
  // element *is* lives in the document, not in the prototype chain. That makes
  // `instanceof HTMLElement` right, `instanceof HTMLScriptElement` too
  // generous, and both of them better than a ReferenceError.
  for (const tag of [
    "Anchor", "Area", "Audio", "BR", "Body", "Button", "Canvas", "Data",
    "DataList", "Details", "Dialog", "Div", "Embed", "FieldSet", "Form",
    "Head", "Heading", "Hr", "Html", "IFrame", "Image", "Input", "Label",
    "Legend", "LI", "Link", "Map", "Media", "Menu", "Meta", "Meter", "Mod",
    "OList", "Object", "OptGroup", "Option", "Output", "Paragraph", "Picture",
    "Pre", "Progress", "Quote", "Script", "Select", "Slot", "Source", "Span",
    "Style", "Table", "TableCell", "TableRow", "TableSection", "Template",
    "TextArea", "Time", "Title", "Track", "UList", "Unknown", "Video",
  ]) {
    const name = `HTML${tag}Element`;
    if (globalThis[name] === undefined) globalThis[name] = globalThis.HTMLElement;
  }

  globalThis.MutationObserver = MutationObserver;

  // The two that watch *elements* rather than the tree. Aliases of the one
  // above until now, which cost them `unobserve` — a method neither the
  // mutation observer has nor needed, and which a page calls as soon as it
  // stops caring about an element. Calling a method that is not there threw
  // inside the promise a page boots in, where nothing was listening: the whole
  // application stopped and said nothing at all.
  //
  // Still reporting nothing. Knowing when an element scrolls into view or
  // changes size means watching a layout that is only computed when someone
  // asks for it, and nobody is asking between frames. What these do is let a
  // page ask for that and carry on.
  class ElementObserver {
    constructor(callback) {
      this._callback = callback;
      this._watched = new Set();
    }
    observe(target) {
      this._watched.add(target);
    }
    unobserve(target) {
      this._watched.delete(target);
    }
    disconnect() {
      this._watched.clear();
    }
    takeRecords() {
      return [];
    }
  }

  globalThis.ResizeObserver = ElementObserver;

  // This one reports, because a page waits on it before doing anything.
  //
  // An infinite list asks to be told when its sentinel comes into view, and
  // loads its first page when it is. Told nothing, it loads nothing — not a
  // slower feed, no feed and not one request made. A silent observer is worse
  // than a missing one: at least a missing one throws.
  //
  // What it reports is that everything is in view, which is the truth here. A
  // page is rendered whole — there is no window cutting it off, so an element
  // with a box is on screen by definition. A browser looking through a window
  // would have to compare against the band it shows; this one has no band.
  //
  // Reported on a later turn rather than from inside `observe`, because that is
  // where a page expects it: one that starts loading during its own setup call
  // re-enters whatever was setting it up.
  globalThis.IntersectionObserver = class IntersectionObserver extends ElementObserver {
    constructor(callback, options = {}) {
      super(callback);
      this.root = options.root ?? null;
      this.rootMargin = options.rootMargin ?? "0px";
      this.thresholds = [options.threshold ?? 0].flat();
    }

    observe(target) {
      super.observe(target);
      globalThis.setTimeout(() => {
        if (!this._watched.has(target)) return;
        const box = target.getBoundingClientRect();
        this._callback(
          [
            {
              target,
              isIntersecting: true,
              intersectionRatio: 1,
              time: 0,
              boundingClientRect: box,
              intersectionRect: box,
              rootBounds: null,
            },
          ],
          this,
        );
      }, 0);
    }
  };

  // Stylesheets are parsed outside the engine, so these are names to reach for
  // rather than working objects.
  class StyleSheet {}
  class CSSStyleSheet extends StyleSheet {
    constructor() {
      super();
      this.cssRules = [];
    }
  }
  class CSSRule {}
  class CSSGroupingRule extends CSSRule {}

  globalThis.StyleSheet = StyleSheet;
  globalThis.CSSStyleSheet = CSSStyleSheet;
  globalThis.CSSRule = CSSRule;
  globalThis.CSSGroupingRule = CSSGroupingRule;

  // Constants only. Nothing here walks a tree with them yet, but code that
  // means to reads them at load time.
  globalThis.NodeFilter = {
    SHOW_ALL: 0xffffffff,
    SHOW_ELEMENT: 1,
    SHOW_TEXT: 4,
    SHOW_COMMENT: 128,
    FILTER_ACCEPT: 1,
    FILTER_REJECT: 2,
    FILTER_SKIP: 3,
  };
})();
