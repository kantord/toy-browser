---
type: Playbook
title: Parts that share private state
description: Splitting a file whose pieces cannot become separate modules because they share one scope.
tags: [file-too-long, javascript, structure]
---

# Parts that share private state

Sometimes the pieces cannot simply become separate files: they close over the
same private bindings, and the language offers no module boundary that keeps
them.

Give them one explicit namespace and make each part a standalone file evaluated
in order. What was closure state becomes named state on that namespace.

## Worked example

**`crates/engine/src/prelude.js` (833)** — one IIFE, `include_str!`'d and
evaluated as a single classic script. Its parts share `wrappers`, `wrap`,
`dispatch` and the listener table, so splitting the file would break them.

It already publishes `__dom`, `__console`, `__lifecycle`, `__boxes` and `__node`
on `globalThis`, so the encapsulation being given up was mostly given up
already.

```
prelude/00-core.js        globalThis.__tb = { wrappers, wrap, tree helpers }
prelude/10-node.js        Node — identity, the tree, listeners
prelude/20-element.js     HTMLElement — selectors, attributes, boxes, style
prelude/30-interfaces.js  the per-tag interfaces, observers, CSSOM stubs
prelude/40-events.js      listeners, dispatch, Event and CustomEvent
prelude/50-tasks.js       timers, frames, customElements
prelude/60-document.js    document, window, the load lifecycle
```

Each file is valid JavaScript on its own, so a formatter and an editor
understand it. Rust evaluates them in filename order — an ordered array of
`include_str!`s, since the macro needs a literal path.

Elements needed three files, not one: `Node` and `HTMLElement` together are
over the budget on their own. The cut that fell out is the DOM's own — the
tree in one file, what an element adds in the next — which is why the numbers
leave gaps. Keeping each file to one job is what decides the count; the list
above is not a fixed shape to reproduce.

## The alternative that was rejected

Concatenating fragments inside one preserved IIFE keeps the encapsulation and
needs no code to move. But no fragment is valid JavaScript alone — unbalanced
braces, no formatter, no editor support — which is a strange result from a split
done for readability.
