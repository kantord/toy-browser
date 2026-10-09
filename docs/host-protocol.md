# The host protocol

`toy-browser-host` (`crates/host`) is the engine as a process: HTML goes in, a
grid of cells comes out, one JSON object per line each way on stdio. The
reference is the comment at the top of `crates/host/src/main.rs`; this file is
the contract around it.

- **Version.** The first line out is `{"ev":"hello","protocol":N}`. N is bumped
  when something that was there changes meaning or goes. A new op, event or
  field is not a bump: clients ignore events and fields they do not know, and
  the host ignores fields it does not know.
- **Pages.** Any command may carry `"page": n`; the host keeps one engine page
  per number, made on first use, all reading through one fetch cache. Replies
  carry the number back. `{"op":"close"}` forgets a page.
- **Replies.** Every command is answered with a frame, except `inspect`,
  `anchor`, `close`, and a command with `"silent":true`. `"full":true` makes the
  frame whole; otherwise frames after the first carry only changed rows (`at`).
- **Naming elements.** An element in the markup a client sends may carry
  `data-key="…"`. `target` and `clicked` events say the nearest `data-key` above
  the cell, so a client that builds the markup can route a click to what it
  stands for without parsing the page again.
- **Clients.** `nvim/lua/toy_html` (Neovim buffers), `nvim/client/nvim_html.py`
  (Neovim drawn as HTML), `hosts/happy-dom` (a host that fetches with happy-dom
  and wraps this one).
- **Scripts.** The host builds with or without a JavaScript interpreter
  (`--no-default-features` is without). The protocol is the same; without one the
  page's scripts are simply not run.
- **Pictures.** With `{"op":"images","mode":"real"}` each picture is announced
  once by an `image` event (`id`, `cols`, `rows`, a PNG as base64, scaled to the
  cells it covers), and every cell of it in a frame is kitty's unicode
  placeholder for that picture — the placeholder character, a combining mark for
  the row and one for the column — with the picture's `id` as its foreground
  colour. A client shows the image by sending it to a terminal under that id
  (`a=T,U=1,i=<id>,c=<cols>,r=<rows>`); the text does the placing, which is why
  an editor can scroll and clip it. Numbers start at 256 and are shared by every
  page of the host.
