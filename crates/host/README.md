# toy-browser-host

The grid behind a pipe: one JSON object per line in (markup, navigate, click, key, resize) and out (frames of coloured runs, events). For editors and other programs that draw cells themselves, such as the Neovim plugin in `nvim/`. Protocol: `docs/host-protocol.md`. Pages are laid out in whole cells (`docs/adr/0014-the-terminal-lays-out-in-cells.md`); there is no pixel layout to switch to.

Part of [toy-browser](https://github.com/kantord/toy-browser), dual-licensed MIT or Apache-2.0.
