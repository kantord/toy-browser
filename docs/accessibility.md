# Accessibility

A page is read as well as looked at. This browser answers both questions from
the same document and the same layout, and the reading half is built in three
pieces that know as little about each other as possible.

```
Reading          crates/browser/src/reading/   a value: roles, names, boxes
the adapter      crates/cli/src/window/        one window's worth, on a bus
the desktop      crates/e2e/                   a real AT, in a container
```

## The Reading

`Browser::reading(&page)` walks the laid-out document and answers with an
`accesskit::TreeUpdate`: every element that is on the page, what it is, what it
is called, and what can be done to it.

It is a **value**. Nothing in it talks to a platform, so it can be built with no
window open, printed, diffed and asserted on — which is why most of what this
browser knows about accessibility is checked by `cargo test` rather than by a
desktop. `toy-browser reading <url>` prints one.

AccessKit's vocabulary rather than one of our own, because it is the vocabulary
every platform's accessibility API is already reachable from: AT-SPI on Linux,
UI Automation on Windows, NSAccessibility on macOS. Inventing a role enum here
would mean writing that translation three times.

Four decisions are worth knowing:

**A thing that can be pressed is one thing.** A link whose words sit in a
`<span>` is a link, not a link containing a span. Activatable roles keep their
name and drop their children, because a reader offered both would offer the same
thing twice. Everything else keeps its children — a heading full of links is a
heading full of links.

**What is not on the page is not in the tree.** `<head>` and what it holds were
never on it, `display: none` took an element off it, and `aria-hidden` is an
author saying this is scaffolding. All three take the subtree with them.

**Not everything gets a name.** A name is said out loud before anything is done
with a thing, so `<body>` is not labelled with the whole page. Text becomes a
name for an element whose role is named by its content — a link, a button, a
heading — and for a leaf, where its own words are all there is to go on.

**Every node carries a box.** An activation names a node and nothing else; the
box is what turns that into somewhere to press.

## The window

`toy-browser browse` offers the page to the desktop through `accesskit_winit`.
`--no-a11y` declines; `--no-default-features` removes the dependency from the
build entirely, which is the other half of the same choice — one is a window
that says nothing, the other is a binary that cannot.

Nothing is built until somebody attaches. An assistive technology announces
itself by turning on `org.a11y.Status.IsEnabled`, and only then does the window
start reading the page — which matters, because reading one is a walk of every
element on it and a window redraws for reasons that change nothing.

**An activation rides the same pointer a click does.** The AT names a node, the
window looks its box up in the tree it last sent, and presses the middle of it
in the page's own coordinates. There is no second way to activate something, so
there is nothing for a second way to diverge from — and a reader can activate
something that is not on screen, which is exactly what it should be able to do.

The page read is the one the chrome is showing, not the chrome. A `<webview>`
holds its own document with its own numbering, and a tree that spliced the two
would have two nodes called 0. What that costs today is the Back button.

## The desktop

`just a11y` runs the one test that is not about what this browser believes:
a container with an X server, a session bus, the accessibility bus, and a
thirty-line Python program that is a screen reader. It finds the links on a page
and follows one, knowing nothing about what drew the window.

It is the sibling of `tests/window`, which opens the same window on the same
kind of screen and photographs it. One asks what the page looks like and the
other what it says, and neither can answer the other's question.

The image is built separately (`just a11y-image`) because the cache mounts that
make an in-container cargo build bearable belong to the builder, and
testcontainers drives it through an API that does not offer them. The browser is
compiled inside rather than copied in: a binary built on a rolling distro links
a glibc newer than any stable base image ships, and glibc only promises to work
forwards.

The test fails rather than skipping when the image is missing. A test that skips
itself when its dependency is absent is a test that passes forever on the
machine where it matters least.
