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
  globalThis.MutationObserver = MutationObserver;
  globalThis.ResizeObserver = MutationObserver;
  globalThis.IntersectionObserver = MutationObserver;

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
