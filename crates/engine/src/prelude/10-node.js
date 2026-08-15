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
