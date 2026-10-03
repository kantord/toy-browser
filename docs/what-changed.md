# What changed, and why the engine cannot say

Layout gets a serialised copy of the engine's DOM and re-parses it on every
composition. `docs/measuring-again.md` measures what that costs and what
keeping one document alive would be worth: **85–90 ms against a fifth of a
millisecond** for the change a real page makes most often.

Collecting that means applying the engine's mutations to the layout document
instead of re-serialising. Which means the engine has to say *what* changed, and
today it says only *that* something did — `revision` is a counter.

This is the audit of what it would take, done before designing anything on top.

## The mutation surface is one method and thirteen callers

Every change to the DOM goes through `Dom::touched()` in
`crates/engine/src/dom/mod.rs`, and **nothing bypasses it**:

- no method hands out `&mut BaseDocument` or a `DocumentMutator`
- every `borrow_mut()` inside `dom/` is inside one of the thirteen
- every `borrow_mut()` outside `dom/` is unrelated state — the console report,
  the timer tables, `ready_state`
- `with_document` takes `&BaseDocument`, so a caller cannot alter anything

That is the whole reason this is tractable. **Each of the thirteen already has
the node id in its signature**, so none of them needs information it does not
have to say what it touched.

| what it would report | methods |
| --- | --- |
| one node's text | `set_text` |
| one node's attributes | `set_attribute`, `remove_attribute` |
| one node's children | `append_child`, `insert_before`, `remove_node` |
| one node's whole subtree | `set_inner_html`, `append_html` |
| **nothing** | `create_element`, `create_text_node`, `clone_node` |
| **nothing** | `with_field`, `blur` |

### The two rows that report nothing

**A node that has just been created has no place yet.** `create_element` and
friends answer an id and leave it unparented, so there is nothing for layout to
do until something adopts it — and whatever does is `append_child` or
`insert_before`, which reports its parent. So three of the thirteen need no
variant at all, and the node arrives in the layout document as part of its
parent's new children.

**Focus and typed values are not markup.** They never were: `engine.fields()`
carries them to `laid_out.typed()` separately, because what has been typed into
a field is not in the HTML that field was parsed from — `docs/pipeline.md` step
5. So typing is already outside this problem.

## The shape to build, and the one to avoid

The dangerous version is a change list the layout path trusts. Miss a drain on
one path and the browser concludes nothing changed and draws **stale pixels**.
A revision counter cannot fail that way: a counter that is wrong is wrong in the
direction of doing too much work.

So the list must never become the authority:

```rust
struct Dom {
    /// Still the authority. Moves on every mutation, as it does now.
    revision: Cell<u64>,
    /// What moved since the last drain, *when that is known*. `None` means it
    /// is not — and the answer to that is the rebuild we already do.
    changed: RefCell<Option<Vec<Change>>>,
}

/// What one mutation did, as the node it did it to.
///
/// Advisory. Every variant is something layout may apply instead of rebuilding,
/// and never something it must.
enum Change {
    /// This node's text is new.
    Text(usize),
    /// This node's attributes are new.
    Attributes(usize),
    /// This node's children are new. New nodes arrive this way.
    Children(usize),
    /// This node and everything under it is new.
    Subtree(usize),
}

fn touched(&self, what: Change) {
    self.revision.set(self.revision.get() + 1);
    let mut changed = self.changed.borrow_mut();
    match changed.as_mut() {
        // Past the point where replaying is cheaper than rebuilding, stop
        // collecting rather than collect what nobody will use.
        Some(list) if list.len() >= MOST => *changed = None,
        Some(list) => list.push(what),
        None => {}
    }
}
```

**`None` is the load-bearing part.** Giving up has to be representable, cheap
and the default for anything unusual, so that every path which does not yet
know how to report lands on a slow correct answer rather than a fast wrong one.
A missed drain costs 85 ms. A trusted incomplete list costs a wrong page.

### Where `MOST` comes from

Not from taste. An invalidating change costs 21 ms to resolve and a rebuild
costs 85–90 ms, so **replaying stops paying at about four of them** — the
numbers are in `docs/measuring-again.md`. A page whose script makes a hundred
mutations between compositions should rebuild, and this is the line where that
becomes true.

Note that the 21 ms is the *invalidating* case — a class on the root, an append
that moves every sibling. A hundred `set_text` calls deep in the tree cost a
fifth of a millisecond each and never approach the rebuild. So the bound wants
to count what invalidates, not what changes, and that distinction is the first
thing to get wrong.

## What is still unsolved

**New nodes have no key in the layout document.** Geometry is attributed back
through the `__tb-key-<id>` classes written at serialisation time, so a node
created after the layout document exists has no counterpart there. Applying
`Children(parent)` means creating those nodes *and* recording the
correspondence — which makes the key map something the browser keeps, rather
than something regenerated on every compose. That is a change to the join
between the two DOMs, and it is the real work in this.

**Nothing has been built.** `compose()` still calls `lay_out` with a fresh
document every time, which is correct and 85 ms. Everything above is what it
would take to stop.
