# Rasterizing safely

A rasterizer shared between clients that do not trust each other. What it may
tell them, who may reach it, and what has actually gone wrong.

The other half — why another process at all, and what it costs — is
`docs/rasterizing-elsewhere.md`.

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

### Nothing crosses until it is asked for

A client was sending its typefaces, and they are the *machine's* typefaces —
fontconfig read them out of `/usr/share/fonts`, and a rasterizer on the same
machine opens the same files. Three and a half megabytes a connection, shipped
to a process that already had it.

So a Scene now goes over with its tables empty. The rasterizer answers `Missing`
with what it cannot reach, those bytes cross, and nothing else does. On a long
Wikipedia article:

```
wire  asked for 46 of 53 named: 46 pictures (690kB), 0 faces (0kB)
scene: 7465 marks, 46 pictures (690kB), 7 faces (3507kB)
```

All seven typefaces resolved from the machine's own font directories. The
pictures are the page's, with no public source, so they still cross — once.

A Scene it cannot draw is **kept** rather than asked for again: the marks are
the large half of a first frame — 1.7MB on a whole page — and re-sending them
would have cost more than the fonts were saving.

### An oracle is safe exactly when its answer is already public

Saying *yes I have that* is normally the leak this whole design is built around.
For a system typeface it is not, because everything in that index came from a
world-readable file: a client that asks whether this machine has a given
typeface learns only what it could learn by reading the font directories itself.

Nothing a client sends ever enters that index. The two sources stay apart, and
which one answered decides whether answering was allowed.

**And it resolves nothing.** This crate's header forbids naming a font *family*
and letting the rasterizer resolve it a second time — that is how Hacker News
came out in Greek letters, because two resolutions disagreed. A Digest is not a
name a resolver interprets; it is the bytes, stated. Two resolutions cannot
disagree when there is one possible answer. What was forbidden was ambiguity,
and a content address has none.

### What is actually shared

Five caches. Which of them are shared is not obvious from the outside, and twice
this was claimed before it was true, so it is written down:

| held | scope | what it is |
| --- | --- | --- |
| bytes a client sent | process, refcounted | `wire/store.rs` |
| filled glyphs | process | `draw/atlas.rs` |
| decoded pictures | process, LRU under 128MB | `draw/images.rs` |
| composed patches | process, LRU under 128MB | `draw/images.rs` |
| public typefaces | process, by path and then by bytes | `wire/public.rs` |

Everything is keyed by content, so none of it needs a permission of its own.
The pictures and the patches were `thread_local!` — one copy per connection, and
a decoded photograph is megabytes — for the same reason the atlas was, which is
that nothing had been shared before and nobody had looked.

The public typefaces are two caches on purpose. The **index** is paths only, so
an unusually fonted machine costs nothing for the 1250 faces nobody asks for. A
face that *is* asked for is then held, because a page names the same seven on
every frame and re-reading and re-hashing three and a half megabytes per frame
is not a saving.

**In bytes, and least-wanted-first.** Both picture tables held sixty-four
*items* and emptied themselves wholesale when full, which was defensible while
each connection had its own and indefensible once they were shared: a limit
sized for one page became the limit for every window on the machine. Measured,
alternating two articles of 46 pictures each through one rasterizer:

| | decoded | composed |
| --- | --- | --- |
| first article | 46 | 46 |
| second | 111 | 115 |
| first again | **135** | **141** |

Twenty-four pictures decoded a second time, because the second article's
pictures had emptied the table. Under a byte budget and least-recently-wanted
eviction the same run reaches 66 and 68, and nothing is decoded twice. A
sixteen-pixel icon and a two-thousand-pixel photograph are not one thing each,
which is why the budget is not a count — `draw/kept.rs`.

The index is built between the `bind` and the first `accept`: the socket exists,
so a client connects and the kernel holds it, and the 175ms is nobody's wait.
On the first request's path it was measured at 180ms of one.

### Access to a derived thing is access to what it was derived from

There is no trusted client and no untrusted one here, and no reason to
distinguish them: the rule is not about who is asking but about what follows
from what they gave. A client may reach exactly the closure of what it supplied.

That rule settles the question this daemon exists to answer — how to share the
work without sharing the content — because it settles it *once*. The glyph atlas
is keyed by a face Digest and four numbers, so the same letter at the same size
in the same face is one entry for everybody, filled once between them. Reaching
it means naming a Cast, naming a Cast means naming a face, and naming a face
means having sent it. **There is no second permission to grant**, and nothing in
the atlas has any notion of a client.

The same holds for every derivation the rasterizer might keep: a decoded
picture, a parsed face, a shaped run. Each is a pure function of bytes somebody
proved they had, so each inherits that proof and none of them needs a rule.

The one place the rule bites is where a name is *cheaper than the content it
names*. A Digest is sixteen bytes and a typeface is three megabytes, so
accepting a digest as proof of possession is accepting sixteen bytes as proof of
three megabytes — which is what opens the existence oracle above. Possession has
to be demonstrated rather than asserted, and sending the bytes is the crude
demonstration. A challenge over the content would be the cheap one, and is not
written.

*(The atlas was `thread_local!` until this was thought through, so every
connection had one of its own and two clients drawing the same word filled it
twice. The isolation was accidental and so was the waste.)*

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

## What goes wrong, concretely

Three scenarios worth having in mind, found by going and looking rather than by
reasoning about it. Two were real and one was not, which is the useful part.

### One client stops every other one — *was real, fixed*

The glyph atlas is shared, behind a `Mutex`, and every lock was `.expect()`. A
connection thread that panicked while holding it — a malformed typeface reaching
skrifa, anything at all in the draw path — **poisons the lock, and every other
client's next drawing panics too.** One bad font, every window on the desktop
stops.

Sharing the atlas is what introduced it. Before, it was `thread_local!` and a
panic cost exactly one connection. *Sharing the work has to mean sharing the
work and not the crash*, so a poisoned lock is now taken anyway: the atlas is a
cache of filled outlines with no half-written state, and the store's worst case
is a hold nobody gives up.

### A hundred and twenty bytes for thirty-five seconds — *was real, fixed*

```xml
<svg width="100000" height="100000"><rect width="100%" height="100%"/></svg>
```

Placed in a **ten-pixel box**, that took 35 seconds. Where a Mark puts a picture
says nothing about what the file claims to be, so the Scene's own size limit —
which is checked — does not cover it at all. On a shared rasterizer that is
thirty-five seconds of everybody's windows.

### The same trick with a PNG — *was not real*

The obvious companion, and the guard written for it was deleted. A raster
picture claiming to be enormous is refused by the decoder's own limits, which
are shaped better than the one written here: a cap on what is *allocated*, not
on an edge, so a legitimate panorama still draws. The test is kept as the
evidence for not having the limit, and it passes with every guard removed.

A guard nobody needs is worse than none: it costs a real case and buys nothing,
and it makes the next reader believe in a threat that was never there.

## Still open

- **It does not ask who connected.** Anyone who can reach the socket can drive
  the rasterizer, and the directory's mode is the whole of the answer. The
  kernel will say — `SO_PEERCRED` — and nothing asks it. Worth doing if the
  socket ever moves somewhere less private.
- **A client can make it allocate.** A frame declares its length and the server
  allocates it, up to 256MB, before reading a byte. Reachable only by its owner,
  so today this is a way to inconvenience yourself.
- **A hang still takes everybody.** A panic now costs one connection. A loop in a
  decoder costs the daemon, and nothing bounds how long a drawing may take.
- **"Public" is decided here, not there.** A system typeface is treated as
  something any client could have read for itself. Under a sandbox that may be
  false — the client may not see `/usr/share/fonts` at all — and the daemon
  cannot know the client's view of the filesystem. What leaks is which fonts are
  installed, which is small, but the principle is worth stating: this process
  cannot determine what is public *to whoever is asking*.
