# TODO: a releasable modular TUI rendering engine

Goal: release a headless "HTML → cell grid" core, then three hosts on top of it.

```
host gives:    html/css (or DOM patches) + viewport in cols×rows
engine gives:  cell grid (char, fg, bg, attrs) + hit map (cell → node)
host gives:    input events (key, mouse, focus)
engine gives:  dirty cell regions + events for the host to handle
```

## Step 0: shared core (unblocks all three)

- [ ] Write the TUI CSS profile: which CSS is guaranteed in cells (`docs/`)
- [x] POC: headless host `toy-browser-host` (`crates/tui/src/bin/host.rs`), ndjson over stdio: markup/resize/click/move/key/scroll in, frame/log out
- [ ] Promote the host from `crates/tui` into its own crate with a stable API; add a hit map (cell → node) instead of console.log as the only event channel
- [ ] Incremental update: DOM patches in, dirty regions out
- [ ] Make QuickJS optional, one host among others
- [ ] Define the wire protocol properly (POC is ndjson; frames are whole, not dirty regions)

## Target 1: Neovim plugin renders HTML (first)

- [x] POC: `nvim/lua/toy_html` spawns the host, draws frames as lines + extmarks, `<CR>`/mouse click, `:ToyHtmlDemo`
- [ ] Draw the grid into a buffer with extmarks, or into a float
- [ ] Route keys and mouse back to the engine, map hit map → node
- [ ] Demo: a small HTML picker or help page in a float

- [x] POC: host op `inspect` (cell → link href) drives a native right-click menu entry "Open in new tab"

- [x] POC: link click opens a new page buffer in the same window; back/forward are the jumplist (`<C-o>`/`<C-i>`) plus `<A-Left>`/`<A-Right>` for page-level jumps; trailing padding trimmed from lines
- [x] POC: loading/failed text in the buffer, address in the statusline, `r` reloads, `:ToyHtmlGo {url}`
- [x] POC: underlined text (links, `text-decoration`): thin horizontal fills are noted by the grid and put on the characters above them; with the `transparent` op the page's default paper/ink go out as "" so the colorscheme shows; used for plugin-supplied `html` only, because fetched pages (Wikipedia) set dark text without a background and become unreadable on a dark theme
- [x] POC: bold and italic, read from the font face of each text run (skrifa); frame runs carry flags `biu`
- [ ] Italic shows only if the monospace font has an italic file: a synthesised oblique leaves the face unchanged, so the engine would have to say `font-style` per run. Same for a variable font's bold.
- [x] POC: a picture is a pale patch with `[alt]` on its first row (`grid/images.rs`; the alt text is read by the app after painting)
- [ ] Real images: `Mark::Image` is left undrawn. Idea: slice every image into strips one character row tall, one per buffer line (Kitty/Sixel placement per strip, or a placeholder with the `alt` text first). Neovim then crops, hides and scrolls them with the line like any text, and the host never has to track window clipping.
- [ ] History step 2: one host process for all pages (`page` id on every op), close and lazily reload pages that are not shown
- [ ] History step 3: `history.pushState` makes a new buffer; check how the new URL reaches the host
- [ ] Links with click handlers: a click on a link is currently a navigation and skips the page's own handlers

## Target 2: Neovim client rendered via HTML (hardest)

- [x] POC: `nvim/client/nvim_html.py` attaches with `ext_linegrid`, keeps the grid
- [ ] Map grid cells to an HTML/DOM tree with a stable structure across frames (POC: one `<pre>` of spans, rebuilt every flush)
- [x] POC: same HTML drawn by the engine in the terminal and written to a self-refreshing file for a web browser
- [ ] Cursor, highlights and input round trip
- [ ] Check update speed on scroll and large redraws

## Target 3: minimal TUI browser via happy-dom (last)

- [ ] Node host runs happy-dom and streams HTML snapshots or mutation patches to the core
- [ ] Fetch and navigate: URL bar, links, back/forward
- [ ] Forms and focus: input, click, tab order
- [ ] Smoke-test on 3 real pages, e.g. Wikipedia

## Cross-cutting (later)

- [ ] Wide chars, emoji and ligatures in the grid
- [ ] Decide how to handle the vendored Blitz fork and its licences for release
- [ ] Crate names, versioning and README for the release

## Architecture idea: host-owned projection (after the basics work)

Split the library into separate parts the host wires together: DOM, rendering,
and (later) JS. The host then owns the seam between them, and can place a
filter between the **logical DOM** (what the page says) and the **displayed
DOM** (what is laid out and drawn): `displayed = view(state, logical)`.

- Overrides and plugins act at DOM level, so they take part in layout, but never
  edit the real DOM and never stitch into ASCII output. Neovim's extmarks over
  buffer text are the same idea one level down.
- Host state, and deep integration with the rest of the app, drive the view
  (diagnostics, marks, badges, folds as node swaps).
- Needs stable node identities, and a hit map that resolves a displayed cell to
  a displayed node and back through the projection to the logical one.
- Needs incremental invalidation, or a filter change re-lays-out everything.
- First cut: one filter slot, `fn(dom) -> dom`, before layout. Split the rest later.

## Neovim buffer feel (known issues in the POC)

- [x] Whole page in the buffer so Neovim scrolls, searches and folds natively (host op `whole`)
- [x] Leave `<LeftMouse>` alone; click the page on `<LeftRelease>` in normal mode
- [ ] Scrolling is slow, and a release build did not help: profile host vs Lua `draw`

## Parked: buffer as structured source (decided too complex for now)

Explored and set aside. Kept here so it is not re-derived.

- Layout can't be interleaved with source in one buffer: source order (DOM) and
  view order (rows) disagree on any multi-column page. Neovim can hide text
  (conceal, `conceal_lines`; 80k marks set in ~46 ms) but cannot reorder it.
- The core structure, if ever built: a **fragment table**
  `{node, src_start, src_end, row, col, len}` indexed by source offset and by
  `(row, col)`; the host keeps it, buffers only receive finished extmarks.
- Two buffers (source: lossless Pug-like or Markdown, editable; view: the grid,
  read-only) swapped by one key, cursor carried across by the table.
- Candidate source for the DOM-ordered side: the engine's `reading/` tree.
- Only worth it for editing static documents; scripted pages mutate the DOM
  under the user's edits.
