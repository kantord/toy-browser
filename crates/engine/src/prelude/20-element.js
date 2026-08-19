// `HTMLElement`: everything an element has that a bare node does not —
// selectors, attributes, the boxes layout measured, and inline style.
//
// Every wrapper is constructed as one of these, whatever the node turns out
// to be, so this is where a page's scripts spend nearly all of their time.

(() => {
  // `class extends HTMLElement` has to resolve to something.
  class HTMLElement extends globalThis.Node {
    // `Node` keeps the id on the Rust side, where reading it costs a native
    // call. This layer reads it on nearly every line, so it is mirrored here as
    // a plain property — the same number, an order of magnitude cheaper.
    constructor(id) {
      super(id);
      this.__id = id;
    }

    scrollIntoView() {}

    // Nothing here has a shadow tree, and saying so is better than pretending.
    get shadowRoot() {
      return null;
    }

    get assignedSlot() {
      return null;
    }

    // Focus is document state rather than anything visual: nothing here draws
    // a focus ring, but `document.activeElement` answers honestly.
    focus() {
      __dom.focus(this.__id);
    }
    blur() {
      __dom.blur(this.__id);
    }

    // Reads and writes the `style` attribute itself, because that attribute is
    // what survives serialization into the renderer. Only inline style is
    // visible here; nothing computes cascaded style.
    get style() {
      const id = this.__id;
      return new Proxy(
        {},
        {
          get: (_target, property) => __dom.styleGet(id, String(property)),
          set(_target, property, value) {
            __dom.styleSet(id, String(property), String(value));
            return true;
          },
        },
      );
    }
  }

  globalThis.HTMLElement = HTMLElement;
  globalThis.Element = HTMLElement;
  // Every wrapper Rust mints gets this prototype unless a tag registers its
  // own, so an unlisted element is a plain HTMLElement.
  __dom.registerInterface("", HTMLElement.prototype);
})();
