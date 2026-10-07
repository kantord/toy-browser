# Neovim and the engine (proof of concept)

Needs `cargo build -p toy-browser-tui` first: both use `target/debug/toy-browser-host`
(override with `TOY_BROWSER_HOST`).

## 1. HTML inside Neovim — `lua/toy_html`

From the repo root:

```
nvim -u NONE --cmd 'set rtp+=nvim' -c 'runtime plugin/toy_html.lua' -c ToyHtml README.md
```

`:ToyHtml [url]` opens the page in a vertical split to the right (default: Wikipedia's
Neovim article), so the file on the left stays an ordinary buffer. In your own config,
add `nvim/` to the runtimepath instead (e.g. a local plugin spec).

The whole page is real buffer text, so Neovim's own motions, `/` search, visual mode,
yank and mouse selection all work. A click (release in normal mode) or `<CR>` clicks the page;
`q` closes. Right-clicking a link adds "Open in new tab" to Neovim's own right-click menu; it opens the link in a new Neovim tab page. `:ToyHtmlDemo` shows a page that reports clicks
through `console.log`, which reaches `on_event`.

## 2. Neovim drawn as HTML — `client/nvim_html.py`

```
python3 nvim/client/nvim_html.py [--html /tmp/nvim.html] [nvim args]
```

Neovim's screen becomes an HTML document on every flush. The engine draws it in the
terminal; the same document is written to `/tmp/nvim.html`, which a web browser can
open (it reloads itself every second).
