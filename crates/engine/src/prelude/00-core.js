// The scope the rest of the prelude shares.
//
// The prelude is several standalone scripts, evaluated in filename order into
// one global scope. What would be closure state in a single script is named
// state on `__tb`; only what a page is meant to find reaches `globalThis`.
//
// This file owns the bridge from a node id to the object model, and the tree
// arithmetic underneath it. It names globals — `HTMLElement`, `Text` — that
// later files define; nothing here runs until the whole prelude has been
// evaluated.

globalThis.__tb = {
  // One wrapper per node id, so `a === b` holds for the same element.
  wrappers: new Map(),

  // Which interface a tag presents. Filled in by the interfaces file.
  interfaces: {},
};

(() => {
  const tb = globalThis.__tb;

  // Which interface a node presents, by what it is.
  const interfaceFor = (id) => {
    if (__dom.nodeType(id) === 3) return globalThis.Text;
    if (__dom.nodeType(id) === 8) return globalThis.Comment;
    return tb.interfaces[__dom.tagName(id)] ?? globalThis.HTMLElement;
  };

  tb.wrap = (id) => {
    if (id === null || id === undefined) return null;
    let wrapper = tb.wrappers.get(id);
    if (!wrapper) {
      wrapper = new globalThis.HTMLElement(id);
      const interface_ = interfaceFor(id);
      if (interface_ !== globalThis.HTMLElement) {
        Object.setPrototypeOf(wrapper, interface_.prototype);
      }
      tb.wrappers.set(id, wrapper);
    }
    return wrapper;
  };

  // Whether `id` sits anywhere under `ancestorId`.
  tb.isDescendant = (id, ancestorId) => {
    if (typeof id !== "number" || typeof ancestorId !== "number") return false;
    for (let at = __dom.parent(id); at !== null && at !== undefined; at = __dom.parent(at)) {
      if (at === ancestorId) return true;
    }
    return false;
  };

  // The node `offset` places after `node` among its parent's child nodes,
  // text included.
  tb.nodeSibling = (node, offset) => {
    const parent = node.parentNode;
    if (!parent) return null;
    const siblings = __dom.childNodes(parent.__id);
    const index = siblings.indexOf(node.__id);
    return index < 0 ? null : tb.wrap(siblings[index + offset] ?? null);
  };

  // The element `offset` places after `node` among its parent's elements.
  tb.sibling = (node, offset) => {
    const parent = node.parentNode;
    if (!parent) return null;
    const siblings = parent.children;
    const index = siblings.findIndex((candidate) => candidate.__id === node.__id);
    return index < 0 ? null : (siblings[index + offset] ?? null);
  };
})();
