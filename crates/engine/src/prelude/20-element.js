// `HTMLElement`: everything an element has that a bare node does not —
// selectors, attributes, the boxes layout measured, and inline style.
//
// Every wrapper is constructed as one of these, whatever the node turns out
// to be, so this is where a page's scripts spend nearly all of their time.

(() => {
  const tb = globalThis.__tb;

  const kebab = (name) => name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);

  // `class extends HTMLElement` has to resolve to something.
  class HTMLElement extends globalThis.Node {
    // Selectors are matched against the whole document, then narrowed to this
    // element's descendants, because the DOM only offers a document-wide query.
    querySelectorAll(selector) {
      return __dom
        .queryAll(String(selector))
        .filter((id) => tb.isDescendant(id, this.__id))
        .map(tb.wrap);
    }

    querySelector(selector) {
      return this.querySelectorAll(selector)[0] ?? null;
    }

    matches(selector) {
      return __dom.queryAll(String(selector)).includes(this.__id);
    }

    closest(selector) {
      for (let at = this; at !== null; at = at.parentNode) {
        if (at.matches?.(selector)) return at;
      }
      return null;
    }

    setAttribute(name, value) {
      __dom.setAttribute(this.__id, name, String(value));
    }

    // `null`, not `undefined`, when the attribute is absent — callers compare
    // against null and Rust's `None` arrives as undefined.
    getAttribute(name) {
      return __dom.getAttribute(this.__id, name) ?? null;
    }

    hasAttribute(name) {
      return this.getAttribute(name) !== null;
    }

    removeAttribute(name) {
      __dom.removeAttribute(this.__id, name);
    }

    getAttributeNames() {
      return __dom.attributes(this.__id).map(([name]) => name);
    }

    hasAttributes() {
      return __dom.attributes(this.__id).length > 0;
    }

    // A live-ish NamedNodeMap: an array of {name, value} that also answers
    // getNamedItem, which is the part anything actually calls.
    get attributes() {
      const pairs = __dom.attributes(this.__id).map(([name, value]) => ({ name, value }));
      pairs.getNamedItem = (name) => pairs.find((pair) => pair.name === name) ?? null;
      return pairs;
    }

    // `data-foo-bar` reads as `fooBar`, as the DOM says.
    get dataset() {
      const data = {};
      for (const [name, value] of __dom.attributes(this.__id)) {
        if (!name.startsWith("data-")) continue;
        const key = name.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
        data[key] = value;
      }
      return data;
    }

    // Geometry is measured outside and published before anything reads it. An
    // element the layout never produced a box for reports an empty rect, the
    // same as a display:none element in a browser.
    getBoundingClientRect() {
      const [x = 0, y = 0, width = 0, height = 0] = globalThis.__boxes?.[this.__id] ?? [];
      return {
        x,
        y,
        width,
        height,
        top: y,
        left: x,
        right: x + width,
        bottom: y + height,
        toJSON() {
          return this;
        },
      };
    }

    getClientRects() {
      const rect = this.getBoundingClientRect();
      return rect.width === 0 && rect.height === 0 ? [] : [rect];
    }

    scrollIntoView() {}

    get offsetWidth() {
      return this.getBoundingClientRect().width;
    }

    get offsetHeight() {
      return this.getBoundingClientRect().height;
    }

    // The border box, which is all we measure. Padding and border are not
    // subtracted because nothing here knows them.
    get clientWidth() {
      return this.getBoundingClientRect().width;
    }

    get clientHeight() {
      return this.getBoundingClientRect().height;
    }

    // Nothing here has a shadow tree, and saying so is better than pretending.
    get shadowRoot() {
      return null;
    }

    get assignedSlot() {
      return null;
    }

    get localName() {
      return __dom.tagName(this.__id);
    }

    get namespaceURI() {
      return "http://www.w3.org/1999/xhtml";
    }

    // Form state, read off the attributes that carry it. Nothing here has a
    // value the user typed, because nothing here has a user.
    get value() {
      return __dom.getAttribute(this.__id, "value") ?? "";
    }

    set value(next) {
      __dom.setAttribute(this.__id, "value", String(next));
    }

    get checked() {
      return this.hasAttribute("checked");
    }

    get disabled() {
      return this.hasAttribute("disabled");
    }

    get type() {
      return __dom.getAttribute(this.__id, "type") ?? "";
    }

    get hidden() {
      return this.hasAttribute("hidden");
    }

    get title() {
      return __dom.getAttribute(this.__id, "title") ?? "";
    }

    get role() {
      return __dom.getAttribute(this.__id, "role") ?? null;
    }

    // No element is ever focused: there is no input to give focus to.
    focus() {}
    blur() {}

    get tagName() {
      const tag = __dom.tagName(this.__id);
      return tag === null ? null : tag.toUpperCase();
    }

    get innerHTML() {
      return __dom.innerHtml(this.__id);
    }

    set innerHTML(value) {
      __dom.setInnerHtml(this.__id, String(value));
    }

    get outerHTML() {
      return __dom.outerHtml(this.__id);
    }

    get className() {
      return __dom.getAttribute(this.__id, "class") ?? "";
    }

    set className(value) {
      __dom.setAttribute(this.__id, "class", String(value));
    }

    get id() {
      return __dom.getAttribute(this.__id, "id") ?? "";
    }

    set id(value) {
      __dom.setAttribute(this.__id, "id", String(value));
    }

    get classList() {
      const node = this;
      const tokens = () => node.className.split(/\s+/).filter(Boolean);
      return {
        contains: (token) => tokens().includes(token),
        add(...added) {
          node.className = [...new Set([...tokens(), ...added])].join(" ");
        },
        remove(...removed) {
          node.className = tokens()
            .filter((token) => !removed.includes(token))
            .join(" ");
        },
      };
    }

    // Reads and writes the `style` attribute itself, because that attribute is
    // what survives serialization into the renderer. Only inline style is
    // visible here; nothing computes cascaded style.
    get style() {
      const node = this;
      const declarations = () => {
        const map = new Map();
        for (const declaration of (node.getAttribute("style") ?? "").split(";")) {
          const colon = declaration.indexOf(":");
          if (colon > 0) {
            map.set(declaration.slice(0, colon).trim(), declaration.slice(colon + 1).trim());
          }
        }
        return map;
      };
      return new Proxy(
        {},
        {
          get: (_target, property) => declarations().get(kebab(String(property))) ?? "",
          set(_target, property, value) {
            const map = declarations();
            map.set(kebab(String(property)), String(value));
            node.setAttribute(
              "style",
              [...map].map(([name, declared]) => `${name}: ${declared}`).join("; "),
            );
            return true;
          },
        },
      );
    }
  }

  globalThis.HTMLElement = HTMLElement;
  globalThis.Element = HTMLElement;
})();
