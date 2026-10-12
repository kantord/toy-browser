# toy-browser-tui

A page as a grid of character cells (text, colours, bold, italic, underline, and where pictures fall), plus the terminal application that shows one. See `docs/tui-css-profile.md` for what CSS survives the trip.

Part of [toy-browser](https://github.com/kantord/toy-browser), dual-licensed MIT or Apache-2.0.

## Cell-native layout

`--cells` (on `toy-browser-tui` and `toy-browser-host`) or `TOY_BROWSER_LAYOUT=cells` lays pages out in whole terminal cells with `Browser::cells` instead of in pixels with Blitz, so rows, wrapping and clicks are exact integers. The result is converted into the same `Grid`, so the host protocol and the Neovim plugin are unchanged. 

What cells mode does, beyond the layout:

- **Clusters.** A `Cell` holds a whole grapheme cluster (`ch` plus `more`), so accents, skin tones, ZWJ sequences and VS16 emoji reach the host frame and the terminal whole. The terminal writer skips the blank after a wide cluster and re-sends the cursor position after a VS16 one, since terminals disagree on its width. The pixel path fills the same field (it only ever has the one `char`).
- **Forms.** Typed values, checkbox and radio state show in the text forms `[value___]`, `[x]`, `(o)`; a long value shows its end; a `<select>` shows its selected option. There is no caret mark.
- **Focus and hover.** The focused element is shown in reverse video (its text and background colours swapped), so nothing new goes over the wire. `:focus` rules apply, and `:hover` rules apply to the element under the pointer; the page is styled again when that element changes.
- **Colour scheme.** `prefers-color-scheme` follows the host `scheme` op and `g:toy_html_scheme`. `transparent` needs nothing: default colours are left to the client as in the pixel path.
- **Pictures.** `images` `alt` (the default) and `none` work as before. With `real`, an `<img>` whose bytes can be fetched is a block of cells sized from its `width`/`height`, CSS or its own size over a nominal 8 by 16 px cell (capped to the page width, shape kept), shown through the same kitty placeholder cells and `image` events as the pixel path. One that cannot be fetched keeps its `[alt]`.

Degraded: a picture is always its own block (never inline in a line of text), `<webview>` routing is not done, a hover over a page with `:hover` rules re-styles the whole page (the cascade only, no layout), and everything `crates/cells/README.md` lists as out of scope (floats, positioning, table-specific layout such as collapsed borders, grid named areas).
