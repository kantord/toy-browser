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

What the window's own thread spends on a frame, which is what decides whether
it answers the mouse:

| | on the window thread | elsewhere |
| --- | --- | --- |
| in process, waiting | **34–40 ms** | — |
| over the socket, waiting | **29–43 ms** | — |
| over the socket, not waiting | **6.5 ms paint + 2.1 ms blit** | 36 ms |

Another process does not on its own give the thread back: a call that waits
waits wherever the work happens, which is why the middle row is no better than
the first. What gives it back is **not waiting** — hand the Scene over, return
to the event loop, and collect the pixels when they arrive, waking the loop
through the `EventLoopProxy` that was already there for the screen reader.

The 6.5ms that remains is building the Scene, which has to happen here: it reads
the laid-out document. The drawing is 97% of the work and all of it has gone.

### Two slots, and newest wins

A window being scrolled asks for a band faster than anything can draw one, and
every band but the last is already wrong by the time it could be shown. So the
request is a *slot*, not a queue: a new job replaces the pending one. A queue
would draw every position the scroll passed through, each staler than the last,
and reach the right picture last of all.

A band that comes back is put up only if the window is still over that part of
the page. `asked` and `over` are therefore different fields — what has been
sent for, and what is actually held — and the gap between them is exactly the
time the rasterizer is taking.

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

## Shared by clients that do not trust each other

The point of a daemon is that many clients use one. If those clients are
untrusted apps, two things have to be true at once, and they pull against each
other: **hold one copy** of each typeface, because they are megabytes and
everybody sends the same handful; and **tell nobody anything**, because one
client must not learn what another has drawn.

### The obvious reading of content addressing, and why it is wrong

A Digest is bytes named by their own content, so it is tempting to treat the
name as the permission: to say the digest you must have had the content, so you
may have it back. That is nearly right, and it leaks the one thing worth
leaking — **existence**.

A client that can ask *do you have this digest* and be told **yes** has an
oracle. It names the digest of a document, a photograph, a company's logo, and
learns whether anyone else on this machine has drawn it. It never receives a
byte it did not already have, and it learns something it had no business
knowing.

### What is done instead

**Deduplicated globally, authorised per connection.** `store.rs` holds the
bytes once for everybody; the permission to *name* them is per connection and
is earned only by sending them. A client that has the content sends it and is
told nothing about whether it was already here. A client that does not have the
content cannot get it, and cannot discover that it exists — a digest ten other
clients are using answers exactly as a digest nobody has ever sent.

So the saving is **memory, not transfer**: one copy in this process rather than
one per client. A client still sends each typeface once per connection, which
costs it four milliseconds at the start and nothing afterwards.

*(Before this, `held` was per connection and nothing was shared at all — ten
windows held ten copies. The isolation was perfect and the saving was zero.)*

### The one check it all rests on

**A digest is verified against its bytes before anything is kept.** Without it
the model inverts: a hostile client sends the digest of a popular typeface with
contents of its own, and every other client that later names that digest is
handed the attacker's bytes. Content addressing is only addressing *by content*
if somebody checks.

### What content addressing does not do

It gives the protocol no ambient authority — there is no way to say "open this
file" or "use the font called Verdana", so a rasterizer cannot be talked into
reaching for anything. That is a real and unusual property and it is the right
foundation. It is not, on its own, isolation:

- **It says nothing about resource exhaustion.** A client can send blobs until
  memory is gone, ask for a picture the size of a wall, or simply connect a
  thousand times. There is a cap on what is held and on how many pixels will be
  drawn at once; there is nothing per client, and nothing counts connections.
- **It says nothing about who is asking.** Nothing calls `SO_PEERCRED`, and on
  Linux two apps under one uid are not separated by the kernel anyway. Real
  per-app identity means a socket per app, handed in by whatever sandboxes them
  — the shape a desktop portal has.
- **Timing still says a little.** Keeping bytes that are already held is a hash
  compare; keeping new ones is an allocation. The difference is small beside the
  transfer that preceded it, and it is not nothing.
- **The rasterizer parses what it is given.** Fonts through skrifa, images
  through `image`, SVG through usvg — all safe Rust, so the worst case is a
  panic or a hang rather than anything worse, and a panic takes one connection's
  thread. A hang takes the daemon, and with it everybody.

## Who can reach it

What crosses this socket is everything on somebody's screen: every word of
every page they have open, as text. So it sits somewhere only its owner can
reach — `$XDG_RUNTIME_DIR`, which is already 0700 — and the socket itself is
narrowed to 0600, because `bind` takes the umask and a socket made under the
usual 0022 comes out connectable by every user on the machine. Which is what
this one was, until somebody asked.

The fallback when there is no run-time directory is a directory of *ours* under
the temporary one, made 0700. **Never the temporary directory itself.** It is
world-writable, and the attack that matters is not eavesdropping but squatting:
another user creates the socket path first, this browser connects to it
believing it is a rasterizer, and hands over every Scene it draws. So both ends
check the directory's mode before they speak, and refuse anything but 0700 —
the client especially, since the client is the end that would have sent the
page.

That check needs no syscall to ask who we are. Creating a 0700 directory makes
it ours; finding one that already exists and is 0700 means it is *somebody's*,
and if that somebody is not us then everything we try inside it fails with
permission denied. It fails closed either way.

One thing that helps by accident: a password is replaced with bullets in the
laid-out document, before a Scene is ever built. A stolen Scene has never held
one.

## What it does not do yet

- **Pixels come back as bytes on the socket.** A band is 6.5MB a frame, copied
  twice — once into the reply and once out of it — for about 6ms. Measured:

  ```
  wire  offer 0.0ms  ask 0.1ms  wait+read 25.1ms  marks 65kB  back 6646kB
  ```

  The marks are 65kB and cost 0.1ms; the wire format is not the problem and the
  typefaces cost nothing after the first frame. It is all the pixels. The answer
  on Linux is shared memory — a `memfd` passed over the socket with `SCM_RIGHTS`
  and mapped at both ends, so the rasterizer draws straight into the buffer the
  window blits from and nothing is copied at all.
- **Nothing stops the server.** It lives until it is killed, holding whatever
  typefaces its clients have sent. It should go away when the last one leaves.
- **It does not ask who connected.** Anyone who can reach the socket can drive
  the rasterizer, and the directory's mode is the whole of the answer. The
  kernel will say — `SO_PEERCRED` — and nothing asks it. Worth doing if the
  socket ever moves somewhere less private.
- **A client can make the server allocate.** A frame declares its length and
  the server allocates it, up to 256MB, before reading a byte. A rasterizer is
  reachable only by its owner, so this is a way to inconvenience yourself; it
  would be a denial of service if that ever stopped being true.
- **The server keeps every picture and typeface it has been sent**, per
  connection, for as long as the connection lasts — and the process outlives
  every browser that used it. Nothing forgets and nothing stops it.
- **A server nobody started is not checked for version.** A browser that starts
  its own runs its own executable, so those two agree by construction. One that
  finds a server already listening — started by an older build — does not ask.
  The socket is per user, so the two are usually the same build anyway.
