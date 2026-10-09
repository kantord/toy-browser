# Neovim and the engine (proof of concept)

Needs `cargo build -p toy-browser-host` first: both use `target/debug/toy-browser-host`
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
`q` closes. Clicking a link opens it as a new page in the same window and records the jump:
`<C-o>`/`<C-i>` go back and forward like any jump, and `<A-Left>`/`<A-Right>` do the same
but skip jumps within a page (scrolling, search) and go to the previous or next *page or file*.
A click reaches the page first, so its own click handlers run; a link they leave alone opens as a new page.

One engine process serves every page; a page that is not on screen is closed there and loaded again when you return to it. Every page stays in the buffer list (`:ls`) as a snapshot. Right-clicking a link adds "Open in new tab" to Neovim's own right-click menu; it opens the link in a new Neovim tab page. Bold, italic (when the monospace font has one) and underlined text, links included, are styled; pictures show as `[alt text]` in grey (pictures without alt text are left out; `vim.g.toy_html_images = "none"` drops them all); pages you pass as `html` take Neovim's colours for their default background and text; fetched pages stay on white, since they set text colours but often no background (`transparent = true` in `open` overrides). `:ToyHtmlGo {url}` loads a page in the current window (`<C-o>` returns, as after a link). The statusline shows the address and whether it is loading or failed; a failed load says so in the buffer, and `r` reloads. `:ToyHtmlDemo` shows a page that reports clicks
through `console.log`, which reaches `on_event`.

## 2. Neovim drawn as HTML — `client/nvim_html.py`

```
python3 nvim/client/nvim_html.py [--html /tmp/nvim.html] [nvim args]
```

Neovim's screen becomes an HTML document on every flush. The engine draws it in the
terminal; the same document is written to `/tmp/nvim.html`, which a web browser can
open (it reloads itself every second).

## Options

Set globally (`vim.g.toy_html_*`) or per page in `require("toy_html").open{...}`:

| option | values | meaning |
| --- | --- | --- |
| `images` | `"alt"` (default), `"none"`, `"real"` | pictures as `[alt text]`, left out, or drawn (kitty graphics protocol: kitty, ghostty; turns on `termguicolors`; inside tmux needs `allow-passthrough on`) |
| `scheme` | `"light"` (default), `"dark"` | what `prefers-color-scheme` says |
| `transparent` | boolean | the page's default white and black take Neovim's colours (default: only for `html`, not for fetched pages) |

Forms: `<CR>` on a field focuses it, `]f` / `[f` move focus along the tab order (the cursor follows), `i` asks for text and types it into the focused field, `gs` presses Enter there.

Keys in a page buffer: `<CR>` / click follows a link, `r` reloads, `q` closes the window, `<A-Left>` / `<A-Right>` go back and forward by page (`<C-o>` / `<C-i>` work too). `:ToyHtmlGo {url}` loads a page in the current window; `:ToyHtmlHelp` shows these keys as a page in a float (`open{float=true}`).
