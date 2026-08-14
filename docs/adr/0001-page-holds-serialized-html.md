---
status: accepted
---

# A Page holds serialized HTML, not a live DOM

A Page keeps its Document as the HTML string produced after parsing and running
scripts, and re-renders from that string on every screenshot, rather than
holding the live blitz DOM. Cutting there reuses the serialization step the
pipeline already performs, keeps a Page trivially inspectable, and avoids
carrying a `!Send` DOM and a QuickJS runtime across protocol calls.

## Consequences

The DOM ceases to exist the moment a Load finishes, so nothing can query or
mutate it afterwards. That closes the door on `Runtime.evaluate`, and with it on
`page.title()`, `page.content()`, locators, and every Playwright API that
evaluates in the page — all of which would need the tree back. Reopening it
means storing the `HtmlDocument` and keeping its JavaScript context alive for
the lifetime of the Page, which is a different ownership model, not an addition
to this one.

We accepted that because the target was `goto` plus `screenshot`, and because
the screenshot path avoids evaluation entirely as long as the client sets a
viewport explicitly — see `docs/cdp-surface.md`.
