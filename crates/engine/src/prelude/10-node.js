// What `Node` still owes JavaScript.
//
// `Node` is a Rust class — see `realm/node/` — and owns the tree, the moves,
// the attribute surface and the wrapper cache behind them. What is left here is
// the part that is about JavaScript rather than about the document: the
// listener table, and the constants a page compares against.

(() => {
  const proto = globalThis.Node.prototype;

  proto.getRootNode = function getRootNode() {
    return globalThis.document;
  };

  Object.defineProperty(proto, "ownerDocument", {
    get() {
      return globalThis.document;
    },
    configurable: true,
  });

  // `nodeName` is `tagName` for an element, and the DOM's own name for the
  // node types that have no tag.
  Object.defineProperty(proto, "nodeName", {
    get() {
      if (this.nodeType === 3) return "#text";
      if (this.nodeType === 8) return "#comment";
      return this.tagName;
    },
    configurable: true,
  });

  // What DOM calls the ParentNode and ChildNode mixins: the short ways of
  // moving nodes around, each written in terms of the four long ones Rust
  // owns. A page reaches for these far more often than for `appendChild` —
  // `wrapper.append(a, b, c)` against three calls and a temporary — and a
  // testharness page that meets one missing throws while building its own
  // fixtures, so the suite reports a timeout rather than a failure.
  //
  // A string argument becomes a text node, which is the whole reason these
  // read better than what they call.
  const nodal = (it) =>
    typeof it === "string" ? globalThis.document.createTextNode(it) : it;

  proto.append = function append(...nodes) {
    for (const it of nodes) this.appendChild(nodal(it));
  };

  proto.prepend = function prepend(...nodes) {
    // Every one against the child that was first to begin with, held once:
    // inserting each before whatever is first *now* would put a run of them
    // in backwards. A parent with no children has no anchor, and
    // `insertBefore` with none appends, which is the same order again.
    const anchor = this.firstChild;
    for (const it of nodes) this.insertBefore(nodal(it), anchor);
  };

  proto.replaceChildren = function replaceChildren(...nodes) {
    while (this.firstChild) this.removeChild(this.firstChild);
    this.append(...nodes);
  };

  proto.before = function before(...nodes) {
    const parent = this.parentNode;
    if (!parent) return;
    for (const it of nodes) parent.insertBefore(nodal(it), this);
  };

  proto.after = function after(...nodes) {
    const parent = this.parentNode;
    if (!parent) return;
    // Held once, for the reason `prepend` gives.
    const anchor = this.nextSibling;
    for (const it of nodes) parent.insertBefore(nodal(it), anchor);
  };

  proto.replaceWith = function replaceWith(...nodes) {
    this.before(...nodes);
    this.remove();
  };

  // The node-type constants live on the constructor, and code compares
  // `child.nodeType === Node.TEXT_NODE` far more often than it calls anything.
  const Node = globalThis.Node;
  Node.ELEMENT_NODE = 1;
  Node.TEXT_NODE = 3;
  Node.CDATA_SECTION_NODE = 4;
  Node.PROCESSING_INSTRUCTION_NODE = 7;
  Node.COMMENT_NODE = 8;
  Node.DOCUMENT_NODE = 9;
  Node.DOCUMENT_TYPE_NODE = 10;
  Node.DOCUMENT_FRAGMENT_NODE = 11;
})();
