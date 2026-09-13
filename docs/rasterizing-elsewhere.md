# Rasterizing somewhere else

`toy-browser rasterize [socket]` is the rasterizer listening on a Unix socket,
and a browser draws through it with `browse --raster` — or `render --raster` for
a one-off. Nobody has to start it: the first browser that wants one does, and
every browser after that finds it already there.

A subcommand of the browser rather than a binary of its own, and not only for
tidiness. A browser starts one by running **its own executable**, so the two
ends of the socket are necessarily the same build and cannot disagree about what
a Mark is. A second binary would also make `cargo run` ambiguous, which is how
this was noticed.

One server serves every window, which is the point of it being a process.

## Why it is possible

A Scene carries everything it needs. A picture and a typeface travel in full and
a Mark refers to one by [`Digest`] — bytes named by their own content — so there
is nothing in a Scene to resolve and nothing the far end has to know about where
it came from. That was decided long before there was anywhere to send one, for
an unrelated reason: a rasterizer handed `<image href="http://…">` declines the
errand silently. The seam it produced is the reason this took a protocol and not
a redesign.

## Why it is affordable

The two halves of a Scene behave completely differently.

| page | marks | pictures | faces |
| --- | --- | --- | --- |
| a form with five fields | 36 | — | 401 kB |
| Hacker News | 1,240 | 1 kB | 1,390 kB |
| a long Wikipedia article | 7,465 | 690 kB | 3,507 kB |

The marks are many, small and different every frame. The bytes are few, large
and *the same bytes as last frame*. So the marks cross on every request and the
bytes cross once, named by Digest — a name both ends can agree on without either
describing what it holds. Sending Wikipedia's typefaces on every frame would
cost more than drawing the page.

The tally is per connection, not per server: a server that restarted has
forgotten, and neither end can tell that from a server that never knew. When a
Scene names something the server has not got, it says which, the client throws
its tally away and offers everything again. Once — a server missing one of them
is missing all of them.

## What it costs, measured

| | build the Scene | draw it |
| --- | --- | --- |
| Hacker News, in process | 3.8 ms | 120.7 ms |
| Wikipedia, in process | 18.6 ms | 1,481.7 ms |

Rasterizing is 97% of the work between a laid-out page and its pixels. That is
the case for moving it.

A window's own frames, which draw a bandful rather than the whole page:

| | first frame | settled |
| --- | --- | --- |
| in process | 25.4 ms | 20.3 ms |
| over the socket | 42.8 ms | 29.3 ms |

**So it is currently slower, and honestly so.** About 9ms a frame goes on
writing the marks down and copying a windowful of pixels back. What has been
bought so far is that many windows share one copy of every typeface and one
rasterizer, and that a crash there is not a crash here.

**The latency win is not in yet, because the call still blocks.** `Client::draw`
asks and waits. Getting the thread back means not waiting — sending the request,
carrying on with the event loop, and blitting when the answer arrives. The
window already has the machinery: `EventLoopProxy`, added so an assistive
technology could wake it. Until that is done, this trades 9ms of latency for
shared memory and isolation, which is a real trade and a smaller one than it
will be.

## Correctness

The same Scene must draw the same picture either way, or this is two browsers.
Checked two ways: `crates/rasterizer/tests/wire.rs` on the protocol, and
`crates/browser/tests/elsewhere.rs` on whole pages through the browser — text,
so that a typeface really crossed, and a picture, so that one was decoded at the
far end. Both compare every pixel.

Spot-checked on real pages too: the frozen Hacker News corpus page and the
gradient, radius, shadow and transform probes are byte-identical as PNGs. Live
`hcker.news` is not, and that is the page rather than the rasterizer — it prints
how long ago each story was posted.

## What it does not do yet

- **It blocks.** See above. This is the one that matters.
- **Pixels come back as bytes on the socket.** A windowful is 3.2MB a frame. The
  proper answer on Linux is shared memory — a `memfd` passed over the socket —
  and it was not worth doing before measuring whether the copy showed up. It
  does: it is most of the 9ms.
- **Nothing stops the server.** It lives until it is killed, holding whatever
  typefaces its clients have sent. It should go away when the last one leaves.
- **A server nobody started is not checked for version.** A browser that starts
  its own runs its own executable, so those two agree by construction. One that
  finds a server already listening — started by an older build — does not ask.
  The socket is per user, so the two are usually the same build anyway.
