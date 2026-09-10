# A window, on a screen nobody is sitting at

`toy-browser browse` opens a real window and answers a real mouse, and the only
way to try one used to be to open it on your own display and click at it — which
moves your pointer, takes your focus, and answers as much about your window
manager as about this browser.

```sh
just window                                     # the Lion article, opened and photographed
just window https://example.com/ 280,295        # and clicked at, in window pixels
```

Xvfb inside podman, `xdotool` for the mouse, `scrot` for the picture. Each step
waits for the previous one to answer — `xdpyinfo` for the display, a window
title for the window — rather than sleeping at it. What comes out is what a
person would have seen: `out/window/*.png`, and the title after every click,
which is how the window says which page it is showing.

This is what found the hit-testing bugs in `docs/limits.md`. Neither showed up
in the corpus, because both are about *where a click lands* rather than about
what is drawn, and a screenshot cannot tell you that.

## Why a container and not your desktop

The first version of this investigation drove the developer's own X session. It
moved their pointer, stole focus from what they were doing, and a stray `pkill`
took down a window they had open — and none of that bought anything a private
display would not have given. A harness that needs someone's attention while it
runs is a harness that gets run once.
