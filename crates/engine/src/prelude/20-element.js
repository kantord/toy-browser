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
      // A declaration is not only a bag of properties: it has methods, and the
      // trap has to hand those back as functions rather than as the empty
      // string a property nobody set reads as. A page calling
      // `style.setProperty(...)` and getting `""` back throws — inside the
      // promise an application boots in, where nobody was listening.
      const methods = {
        setProperty: (property, value) =>
          __dom.styleSet(id, String(property), String(value)),
        getPropertyValue: (property) => __dom.styleGet(id, String(property)),
        removeProperty: (property) => {
          const had = __dom.styleGet(id, String(property));
          __dom.styleSet(id, String(property), "");
          return had;
        },
        // `!important` is not kept, so nothing can answer anything but empty.
        getPropertyPriority: () => "",
        item: () => "",
      };
      return new Proxy(
        {},
        {
          get: (_target, property) =>
            methods[property] ?? __dom.styleGet(id, String(property)),
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

// `dataset`, live rather than a copy.
//
// The engine can hand over the `data-` attributes as an object, and for reading
// that is the whole of it — but a page writes these at least as often as it
// reads them, and `row.dataset.storyId = id` against a copy is a write that
// goes nowhere. The attribute stays absent, and the page's own
// `querySelectorAll("[data-story-id]")` finds none of the rows it has just
// built.
//
// A Proxy rather than a getter per name, because the names are not known: a
// page invents them.
(() => {
  // On `Element`, which shadows the engine's own read-only `dataset` further
  // up the chain rather than replacing it — that one cannot be redefined.
  const proto = globalThis.Element.prototype;

  /// `storyId` is the attribute `data-story-id`, which is the whole mapping.
  const attribute = (key) =>
    "data-" + String(key).replace(/[A-Z]/g, (upper) => "-" + upper.toLowerCase());
  const property = (name) =>
    name.slice(5).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());

  Object.defineProperty(proto, "dataset", {
    get() {
      const node = this;
      const named = () =>
        node.getAttributeNames().filter((name) => name.startsWith("data-"));
      return new Proxy(
        {},
        {
          get: (_, key) =>
            typeof key === "symbol" ? undefined : node.getAttribute(attribute(key)) ?? undefined,
          set(_, key, value) {
            node.setAttribute(attribute(key), String(value));
            return true;
          },
          has: (_, key) => typeof key !== "symbol" && node.hasAttribute(attribute(key)),
          deleteProperty(_, key) {
            node.removeAttribute(attribute(key));
            return true;
          },
          ownKeys: () => named().map(property),
          getOwnPropertyDescriptor: (_, key) =>
            typeof key !== "symbol" && node.hasAttribute(attribute(key))
              ? {
                  value: node.getAttribute(attribute(key)),
                  writable: true,
                  enumerable: true,
                  configurable: true,
                }
              : undefined,
        },
      );
    },
    configurable: true,
  });
})();
