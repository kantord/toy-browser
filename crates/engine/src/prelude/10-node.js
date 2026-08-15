// `Node`: what every wrapper is, and everything that is true of a node
// whatever tag it carries — its place in the tree, how it moves, and the
// listeners attached to it.
//
// Attributes, geometry and style are an element's business and live next door.

(() => {
  const tb = globalThis.__tb;

  class Node {
    constructor(id) {
      this.__id = id;
    }

    appendChild(child) {
      __dom.appendChild(this.__id, child.__id);
      return child;
    }

    insertBefore(node, anchor) {
      if (anchor == null) return this.appendChild(node);
      __dom.insertBefore(node.__id, anchor.__id);
      return node;
    }

    remove() {
      __dom.removeNode(this.__id);
    }

    // Always deep: the DOM underneath clones subtrees, and a shallow copy
    // would quietly drop children rather than refuse.
    cloneNode() {
      return tb.wrap(__dom.cloneNode(this.__id));
    }

    // Anything that is not one of our nodes cannot be inside one of ours, so
    // it is not contained. Callers pass all sorts of things here.
    contains(other) {
      const id = other?.__id;
      if (typeof id !== "number") return false;
      return id === this.__id || tb.isDescendant(id, this.__id);
    }

    addEventListener(type, listener) {
      tb.addListener(this.__id, type, listener);
    }

    removeEventListener(type, listener) {
      tb.removeListener(this.__id, type, listener);
    }

    dispatchEvent(event) {
      tb.dispatch(this.__id, event);
      return true;
    }

    get nodeType() {
      return __dom.nodeType(this.__id);
    }

    get nodeName() {
      return this.tagName;
    }

    get nodeValue() {
      return __dom.nodeValue(this.__id) ?? null;
    }

    get isConnected() {
      return tb.isDescendant(this.__id, __dom.root()) || this.__id === __dom.root();
    }

    get ownerDocument() {
      return globalThis.document;
    }

    getRootNode() {
      return globalThis.document;
    }

    get parentNode() {
      return tb.wrap(__dom.parent(this.__id));
    }

    get parentElement() {
      return this.parentNode;
    }

    // Every child, text nodes included — unlike `children`, which is elements.
    get childNodes() {
      return __dom.childNodes(this.__id).map(tb.wrap);
    }

    get firstChild() {
      return this.childNodes[0] ?? null;
    }

    get lastChild() {
      const children = this.childNodes;
      return children[children.length - 1] ?? null;
    }

    get nextSibling() {
      return tb.nodeSibling(this, 1);
    }

    get previousSibling() {
      return tb.nodeSibling(this, -1);
    }

    get children() {
      return __dom.elementChildren(this.__id).map(tb.wrap);
    }

    get firstElementChild() {
      return this.children[0] ?? null;
    }

    get lastElementChild() {
      const children = this.children;
      return children[children.length - 1] ?? null;
    }

    get nextElementSibling() {
      return tb.sibling(this, 1);
    }

    get previousElementSibling() {
      return tb.sibling(this, -1);
    }

    get textContent() {
      return __dom.text(this.__id);
    }

    set textContent(value) {
      __dom.setText(this.__id, String(value));
    }
  }

  // The node-type constants live on the constructor, and code compares
  // `child.nodeType === Node.TEXT_NODE` far more often than it calls anything.
  Node.ELEMENT_NODE = 1;
  Node.TEXT_NODE = 3;
  Node.CDATA_SECTION_NODE = 4;
  Node.PROCESSING_INSTRUCTION_NODE = 7;
  Node.COMMENT_NODE = 8;
  Node.DOCUMENT_NODE = 9;
  Node.DOCUMENT_TYPE_NODE = 10;
  Node.DOCUMENT_FRAGMENT_NODE = 11;

  globalThis.Node = Node;
})();
