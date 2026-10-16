# Visual regression

Pages written for this repo (no site's markup or text), each with a comment
naming the bug it guards. `just visual` draws every page two ways:

1. **The host's frame** is compared with `expected/<page>.txt`: the characters,
   then the colour behind and in front of each cell as letters named in a
   legend. Read a golden like a picture: `a` behind a row of text is the page's
   paper, and a band should be a different letter.
2. **The Neovim plugin in a real terminal** (tmux) must paint the same
   backgrounds and characters the host sent. That is the check that caught the
   plugin losing a page's coloured bands behind its text.

Adding a case: reproduce the bug with the smallest generic HTML, put the bug in
the page's first comment, run `just visual-bless`, and read the new golden
before keeping it. A golden that shows the bug is not a test.

## The list of supported features

`docs/supported-features.md` is generated from the `Features:` lines in each
page's first comment (`css/order: supported`, `html/progress: unsupported`) and
from nothing else; `just visual` fails when it is stale. A feature is supported
only if a page that passes says so, so the list cannot claim more than the tests
show. An `unsupported` page is the known-limit twin: it shows what happens today
(the golden is the limit, not the goal). When a fix lands, bless, read the
golden, and flip the line to `supported` in the same commit.
