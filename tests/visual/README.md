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
