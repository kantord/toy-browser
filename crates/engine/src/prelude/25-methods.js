// The short ways a page does things to an element.
//
// All of them are built on what is already there — the four long-form moves,
// the attribute pair, the selector engine — and none of them is interesting on
// its own. They are here because a page calls them constantly and a missing one
// does not degrade: it throws, usually inside a promise nobody is catching, and
// takes the rest of whatever was being set up with it.

(() => {
  const proto = globalThis.Node.prototype;

  // Present or absent rather than written: `toggleAttribute(name)` flips it,
  // `toggleAttribute(name, on)` says which. The forced form is what a page
  // uses to mirror a boolean it already has.
  proto.toggleAttribute = function toggleAttribute(name, force) {
    const held = this.hasAttribute(name);
    const wanted = force === undefined ? !held : !!force;
    if (wanted && !held) this.setAttribute(name, "");
    if (!wanted && held) this.removeAttribute(name);
    return wanted;
  };

  // A click a page gives itself. The same path a real one takes, so an inline
  // `onclick` and a listener both hear it.
  proto.click = function click() {
    const event = globalThis.__tb.makeEvent("click", this, true);
    this.dispatchEvent(event);
  };

  proto.hasChildNodes = function hasChildNodes() {
    return this.childNodes.length > 0;
  };

  // Nothing here splits or empties text nodes, so there is never anything to
  // join up. Answering rather than throwing is the whole of what it owes.
  proto.normalize = function normalize() {};

  proto.getElementsByTagName = function getElementsByTagName(name) {
    return this.querySelectorAll(name === "*" ? "*" : String(name));
  };

  proto.getElementsByClassName = function getElementsByClassName(names) {
    const selector = String(names)
      .split(/\s+/)
      .filter(Boolean)
      .map((name) => `.${name}`)
      .join("");
    return selector ? this.querySelectorAll(selector) : [];
  };

  // `beforebegin`, `afterbegin`, `beforeend`, `afterend` — where relative to
  // this element the new thing goes.
  const place = (element, where, node) => {
    const parent = element.parentNode;
    switch (String(where).toLowerCase()) {
      case "beforebegin":
        if (parent) parent.insertBefore(node, element);
        return;
      case "afterbegin":
        element.insertBefore(node, element.firstChild);
        return;
      case "beforeend":
        element.appendChild(node);
        return;
      case "afterend":
        if (parent) parent.insertBefore(node, element.nextSibling);
        return;
      default:
        throw new SyntaxError(`not a position: ${where}`);
    }
  };

  proto.insertAdjacentElement = function insertAdjacentElement(where, node) {
    place(this, where, node);
    return node;
  };

  proto.insertAdjacentText = function insertAdjacentText(where, text) {
    place(this, where, globalThis.document.createTextNode(String(text)));
  };

  // Parsed by the document rather than here: the markup goes into a holder,
  // and what comes out of it is moved into place. `innerHTML` is what knows
  // how to parse, so this borrows it rather than inventing a second answer.
  // A document fragment: a holder whose children are what gets inserted, not
  // the holder itself. Building a list of rows into one and inserting it once
  // is how a page avoids laying the page out per row, and it is what every
  // template-cloning renderer reaches for.
  //
  // Backed by a detached element, because the layout tree has no fragment of
  // its own. What makes it a fragment is the flag below and the two moves that
  // honour it: insert one and its children go in, it does not.
  // An attribute rather than a property, because `cloneNode` copies attributes
  // and not properties — and a cloned fragment that has forgotten it is one
  // gets inserted whole, holder and all.
  const FRAGMENT = "data-tb-fragment";
  const isFragment = (node) =>
    node && typeof node.hasAttribute === "function" && node.hasAttribute(FRAGMENT);

  globalThis.document.createDocumentFragment = function createDocumentFragment() {
    const holder = globalThis.document.createElement("div");
    holder.setAttribute(FRAGMENT, "");
    return holder;
  };

  // A `<template>`'s children belong to its content fragment and not to the
  // document — which is why they are never drawn, and why a page clones them
  // instead of moving them. Every row on a template-rendered page comes through
  // here: `template.content.cloneNode(true)`.
  //
  // Moved on first ask rather than at parse time, because the parser hands them
  // over as ordinary children and nothing else needs them moved until a page
  // reaches for `content`.
  //
  // `content` means two different things depending on the element, and only
  // one of them is a fragment: on `<meta>` it is the attribute, read *and*
  // written — `meta.content = "#fff"` is how every page on the web sets its
  // theme colour. Answering the fragment for both broke that.
  Object.defineProperty(proto, "content", {
    get() {
      if (String(this.tagName).toUpperCase() !== "TEMPLATE") {
        return this.getAttribute("content");
      }
      const held = globalThis.__tb.templates;
      let holder = held.get(this.__nodeId);
      if (!holder) {
        holder = globalThis.document.createDocumentFragment();
        for (const child of Array.from(this.childNodes)) holder.appendChild(child);
        held.set(this.__nodeId, holder);
      }
      return holder;
    },
    set(value) {
      // A template's content is not assignable; anything else's is the
      // attribute.
      if (String(this.tagName).toUpperCase() !== "TEMPLATE") {
        this.setAttribute("content", String(value));
      }
    },
    configurable: true,
  });

  for (const name of ["appendChild", "insertBefore"]) {
    const under = proto[name];
    proto[name] = function (node, anchor) {
      if (!isFragment(node)) return under.call(this, node, anchor);
      // Taken as a list first: moving a child out of the holder changes the
      // holder's own child list underneath the walk.
      for (const child of Array.from(node.childNodes)) {
        under.call(this, child, anchor);
      }
      return node;
    };
  }

  proto.insertAdjacentHTML = function insertAdjacentHTML(where, markup) {
    const holder = globalThis.document.createElement("div");
    holder.innerHTML = String(markup);
    for (const node of Array.from(holder.childNodes)) place(this, where, node);
  };

  // Properties a page reads constantly and that cost nothing to answer. Each
  // one absent is a silent wrong answer rather than an error: `undefined === 0`
  // is simply false, so `if (list.childElementCount === 0) load()` never loads
  // and never says why.
  // Read and written both. A page assigns `innerText` to put text in, and
  // `scrollTop` to move a list back to the top — and a property with a getter
  // and no setter does not ignore that in a module, it throws.
  for (const name of ["innerText", "outerText"]) {
    Object.defineProperty(proto, name, {
      // Layout-aware text is what a browser answers here; this is the text. The
      // difference shows on hidden elements and at block boundaries, and a page
      // reading one in order to show it is right either way.
      get() {
        return this.textContent;
      },
      set(value) {
        this.textContent = String(value);
      },
      configurable: true,
    });
  }

  // Nothing here scrolls, so the position is always the top — but a page that
  // puts it back there must be allowed to say so.
  for (const name of ["scrollTop", "scrollLeft"]) {
    Object.defineProperty(proto, name, {
      get: () => 0,
      set() {},
      configurable: true,
    });
  }

  const reading = {
    childElementCount() {
      return this.children.length;
    },
    // No border is measured per side: a page reads these to offset a position,
    // and zero is the honest offset.
    clientTop: () => 0,
    clientLeft: () => 0,
    offsetTop() {
      return this.getBoundingClientRect().top;
    },
    offsetLeft() {
      return this.getBoundingClientRect().left;
    },
    offsetParent() {
      return globalThis.document.body;
    },
    baseURI() {
      return globalThis.location.href;
    },
    isContentEditable: () => false,
  };
  for (const [name, read] of Object.entries(reading)) {
    Object.defineProperty(proto, name, { get: read, configurable: true });
  }

  // The same, but written as well: a page sets these, and a getter with no
  // setter throws in a module rather than failing quietly.
  for (const [name, fallback] of Object.entries({
    // Reflected on nearly every element that has one — `<meta>`, `<a>`,
    // `<form>`, every form control — and written as a property far more often
    // than as an attribute. Without it `meta.name = "theme-color"` sets a plain
    // JS property, the attribute stays absent, and the page's own
    // `querySelector("meta[name=theme-color]")` does not find the element it
    // just added. It then adds another, on every render, forever.
    name: "",
    tabIndex: -1,
    dir: "",
    lang: "",
    accessKey: "",
    contentEditable: "inherit",
    draggable: false,
    spellcheck: true,
    translate: true,
  })) {
    Object.defineProperty(proto, name, {
      get() {
        const raw = this.getAttribute(name.toLowerCase());
        if (raw == null) return fallback;
        if (typeof fallback === "number") return Number(raw);
        return typeof fallback === "boolean" ? raw !== "false" : raw;
      },
      set(value) {
        this.setAttribute(name.toLowerCase(), String(value));
      },
      configurable: true,
    });
  }
})();
