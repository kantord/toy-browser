# happy-dom host

`toy-browser-host`'s protocol (see `crates/host/src/main.rs`), with pages fetched
and parsed by [happy-dom](https://github.com/capricorn86/happy-dom) instead of
by the engine.

```
client (Neovim plugin, ...)  <->  host.mjs  <->  toy-browser-host (cells)
                                   happy-dom
```

`navigate` is answered by loading the page in happy-dom, taking its document as
it stands (scripts removed: happy-dom has already run them) and sending that on
as `markup`. Everything else passes through.

```
cd hosts/happy-dom && npm install
just nvim-happy https://en.wikipedia.org/wiki/Neovim
```

or point any client at it: `TOY_BROWSER_HOST=$PWD/hosts/happy-dom/host.mjs nvim ...`.

- `TOY_HAPPY_DOM_SCRIPTS=1` lets happy-dom run the page's JavaScript. It is off by
  default because that is untrusted code running inside this Node process, in a
  VM context that is not a sandbox (happy-dom says so itself).
- `TOY_BROWSER_HOST_BIN` is the cell host to run (default `target/debug/toy-browser-host`).

Clicks: every element in the snapshot is stamped `data-key="hd:N"`, and the engine's
`clicked` event names the one under the pointer. A click that is not on a link is
raised on that element in happy-dom (so the page's handlers run, with
`TOY_HAPPY_DOM_SCRIPTS=1`), and the document as it then stands is sent on again.
Links are the client's: it navigates.

Forms: the field the engine says has focus (a click on it, or `focus` stepping along
the tab order) is remembered by its key. `type` and `key` go to that element in
happy-dom instead of the engine's copy: the text goes in the field, its `input`
handlers run, and the document is sent again with focus restored. Enter on a button
presses it. Tested against a page whose `oninput` echoes what is typed.

Not done: `select`, caret movement and selection (only appending and Backspace),
`change` on blur, and form submission (the engine does not fake one either, see
`docs/adr/0010`).
