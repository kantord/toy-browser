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
  proto.insertAdjacentHTML = function insertAdjacentHTML(where, markup) {
    const holder = globalThis.document.createElement("div");
    holder.innerHTML = String(markup);
    for (const node of Array.from(holder.childNodes)) place(this, where, node);
  };
})();
