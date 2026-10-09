# TODO: a releasable modular TUI rendering engine

Goal: release a headless "HTML → cell grid" core, then three hosts on top of it.

```
host gives:    html/css (or DOM patches) + viewport in cols×rows
engine gives:  cell grid (char, fg, bg, attrs) + hit map (cell → node)
host gives:    input events (key, mouse, focus)
engine gives:  dirty cell regions + events for the host to handle
```

## Step 0: shared core (unblocks all three)

- [x] Write the TUI CSS profile: which CSS is guaranteed in cells (`docs/tui-css-profile.md`)
- [x] POC: headless host `toy-browser-host` (`crates/host`), ndjson over stdio: markup/resize/click/move/key/scroll in, frame/log out
- [x] The host is its own crate (`crates/host`, binary `toy-browser-host`). `inspect` now answers with the node id and tag under a cell, as well as the link.
- [x] The host says `hello` with a protocol number (`docs/host-protocol.md`), and `target` / `clicked` events name the node, tag, link and nearest `data-key` under a cell, so a client routes clicks to what it drew without console.log
- [x] Dirty regions out: a frame after the first carries only the rows that changed (`at`); `full` asks for the whole
- [x] DOM patches in: `patch {key, html}` replaces what is inside the element with that `data-key`; `nvim_html.py` sends only the rows that changed (the engine still lays the page out again, so the saving is the parse and the wire, not the layout)
- [x] QuickJS is an optional engine feature (`quickjs`, on by default; `cargo build -p toy-browser-host --no-default-features` has no `rquickjs` in its tree). Without it pages are parsed, laid out and acted on (clicks, links, focus, tab order, typing, patches) but their scripts are not run and `evaluate` says so; `behaviour/` holds what the document does by itself. Tests run with the feature on only.
- [x] Wire protocol: ndjson, versioned (`protocol: 1`), documented in `docs/host-protocol.md` and at the top of `crates/host/src/main.rs`

## Target 1: Neovim plugin renders HTML (first)

- [x] POC: `nvim/lua/toy_html` spawns the host, draws frames as lines + extmarks, `<CR>`/mouse click, `:ToyHtmlDemo`
- [x] `just nvim-test`: headless checks of the plugin against the real host (page, link, history, styling, forms, fragments)
- [x] Draw the grid into a buffer with extmarks (a split, a tab or here), or into a float (`open{float=true}`)
- [x] Mouse reaches the engine as a click at a cell; `inspect` maps a cell to node, tag and link. Keys are Neovim's own on purpose (the page is buffer text); the engine's `key` op exists for clients that want to send them.
- [x] Demo: `:ToyHtmlHelp` is a help page in a float; `:ToyHtmlDemo` is the picker

- [x] POC: host op `inspect` (cell → link href) drives a native right-click menu entry "Open in new tab"

- [x] POC: link click opens a new page buffer in the same window; back/forward are the jumplist (`<C-o>`/`<C-i>`) plus `<A-Left>`/`<A-Right>` for page-level jumps; trailing padding trimmed from lines
- [x] POC: loading/failed text in the buffer, address in the statusline, `r` reloads, `:ToyHtmlGo {url}`
- [x] POC: underlined text (links, `text-decoration`): thin horizontal fills are noted by the grid and put on the characters above them; with the `transparent` op the page's default paper/ink go out as "" so the colorscheme shows; used for plugin-supplied `html` only, because fetched pages (Wikipedia) set dark text without a background and become unreadable on a dark theme
- [x] POC: bold and italic, read from the font face of each text run (skrifa); frame runs carry flags `biu`
- [x] Italic: `Mark::Glyphs.slanted` says when layout leant the face over (synthesised oblique), so italic shows without an italic font file. A variable font's bold is still unseen.
- [x] `prefers-color-scheme`: host op `scheme`, plugin `opts.scheme` / `vim.g.toy_html_scheme` (`"dark"` asks pages for their dark side)
- [x] Wide characters: the blank the layout leaves after a CJK or emoji glyph is dropped, so rows stay aligned (wide glyphs with no blank after them are still a cell off)
- [x] `#fragment` links move the cursor to the element (host op `anchor`)
- [x] Image modes `Real` / `AltText` (default in the grid) / `None`: a small built-in rewrite engine (`engine::Rewrite {selector, action}`, applied while serialising the DOM for layout, so the page's own DOM is untouched). AltText writes `[alt]` in grey, an empty or missing alt is dropped. Host op `images`, plugin `opts.images` / `vim.g.toy_html_images`. Next rules to add: `svg`, `picture`, `canvas`; the same engine is the first cut of the host-owned projection below.
- [x] Real images: `images = "real"` draws pictures in terminals that speak kitty's graphics protocol (kitty, ghostty): the grid notes which cells a picture covers and which slice of it each is, the host sends each picture once as a PNG scaled to its cells (`image` event) and fills the cells with kitty's unicode placeholders coloured with the picture's number, and the plugin writes the image to the terminal. Because the picture is text in the buffer, Neovim scrolls, clips and hides it. Checked in kitty under Xvfb with a local page and Wikipedia's Neovim article (logo, screenshot, icons). Checked in kitty, in ghostty, and in tmux (`allow-passthrough on`) inside kitty, each under Xvfb with screenshots; SVG pictures are drawn with resvg at the size of their cells. Animated GIFs show their first frame (an animation is outside what a buffer shows).
- [x] History step 2: one host process for all pages (`page` id on every op, one shared fetch cache); a page no window shows is closed in the host and loaded again when shown (its buffer text stays as the snapshot, cursor restored). Markup pages have no URL to reload from, so they stay open.
- [x] History step 3: when a click or key makes the page change its own address (`history.pushState`), the host says `pushed`; the old buffer stays as the snapshot and the running page carries on in a new one. Going back reloads the old address; forward reloads the pushed one from the network, so a route that only the page's own JavaScript knows will fail to load (the snapshot stays).
- [x] Inline `onclick="…; return false"` cancels a link
- [x] Links with click handlers: the click goes to the page first; the host answers a followed link as a `navigate` event (`Browser::set_leave_navigation`) and the plugin opens it as a new page. `event.preventDefault()` in a handler stops it.

## Target 2: Neovim client rendered via HTML (hardest)

- [x] POC: `nvim/client/nvim_html.py` attaches with `ext_linegrid`, keeps the grid
- [x] Stable structure across frames: `#screen` with one `div#r<N>` per row (the document is still rebuilt each flush)
- [x] POC: same HTML drawn by the engine in the terminal and written to a self-refreshing file for a web browser
- [x] Cursor (reverse video), highlights, keys and mouse (SGR) go to Neovim and the picture comes back; a pty test types text and clicks
- [x] Update speed: a worst-case change of a whole 200x50 screen costs about 75 ms per flush in a release build (325 ms in debug), because the document is sent whole and laid out again. Fine for typing; DOM patches in (above) are the way down. Use `TOY_BROWSER_HOST=target/release/toy-browser-host`.

## Target 3: minimal TUI browser via happy-dom (last)

- [x] Node host runs happy-dom and sends HTML snapshots to the core (`hosts/happy-dom`, speaks the host protocol; scripts opt-in)
- [x] Fetch and navigate: links, back/forward and `:ToyHtmlGo` through the Neovim plugin work against it (a URL bar is `:ToyHtmlGo`)
- [x] Click handlers run on the happy-dom node (via `data-key`); the document is sent again afterwards
- [x] Typing, focus and tab order on the happy-dom host: `type`, `key` and `focus` ops; Enter presses a focused button (also in the engine). Appending and Backspace only; no caret movement, `select` or submit.
- [x] Smoke-test: Wikipedia's Neovim article loads through happy-dom into a Neovim buffer (302 lines); two more pages untested

## Cross-cutting (later)

- [x] Wide characters: the host drops the blank after a wide glyph (own width table, `grid/width.rs`); a run in a non-monospace face (field text) no longer loses letters that land in one cell. Ligatures do not arise: the grid forces a monospace face. Wide glyphs the layout gives no blank after stay a cell off.
- [x] Vendored Blitz and licences audited (`docs/release.md`): same licence, headers complete, no GPL dependency, MPL-2.0 Stylo crates unmodified. Left for the owner: publish the forks under new names or upstream the change (crates.io rejects path dependencies).
- [x] Names checked on crates.io (all free) and a proposal written (`docs/release.md`); the choice of stem and the first publish are the owner's

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
- [x] Scrolling was slow: it was the engine rebuilding the Scene on every scroll, now kept (20 ms to 3 ms), and frames after the first carry only the changed rows

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
