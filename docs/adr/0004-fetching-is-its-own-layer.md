---
status: accepted
supersedes: part of ADR-0003
---

# Fetching is its own layer, and the engine does none of it

`crates/fetch` owns every read: documents, scripts, modules, images. It is keyed
by URL, thread-safe, and shared by every session in the process. The engine was
handed one instead of reading files itself, which meant it also had to start
speaking URLs — `LoadPage` takes a `base_url`, not a base directory.

This reverses the decision in ADR-0003 that the engine reads the page's own
subresources. The goal that changed it is a massively parallel test browser: a
page's scripts vastly outnumber its documents, so a cache the engine bypassed
would have missed most of the bytes a parallel run reads.

## Consequences

**Reads block.** The engine asks for modules from inside QuickJS during module
resolution, where it cannot yield. `Resources::get` is synchronous and locks
internally. Any future network backend has to live with that or the engine's
module loading has to be rebuilt.

**The engine knows what a URL is, but still opens nothing.** It joins
specifiers against a base and asks the cache. Scheme dispatch — what `file://`
means, what `http://` will mean — belongs to fetch.

**`about:` is answered above.** It names markup rather than bytes, so the
browser layer produces it directly rather than asking the cache for something
that was never there.

**Fonts are the exception.** They are read directly when a `Browser` is built,
once, rather than per page. If font loading ever becomes per-page it should move
behind the cache like everything else.

**The cache is observable, and that is deliberate.** `Resources::len` exists so
a test can assert that a second page loading the same document reads nothing new
— which is the only way to know the sharing is actually working, and it caught
the engine being handed its own empty cache the first time.
