# A text field that works

What it would take to make `<input type="text">` a thing a person can type
into — with a caret, a selection, a clipboard, an input method and a screen
reader — and where each piece has to go.

Everything below the first section is a plan. The first section is measurement.

## What happens today

A page with `<input value="hello"><textarea>some text</textarea>` was loaded,
asked about itself, and rendered. The answers:

| asked | answered | should be |
| --- | --- | --- |
| `input.value` | `"hello"` | ✓ |
| `input.value = "typed"` | sets the **attribute** | sets the property only |
| the render | an empty box | `typed` |
| `<textarea>` | not drawn, `.value` is `""` | the text, `"some text"` |
| `new KeyboardEvent("keydown", {key:"a"}).key` | `null` | `"a"` |
| `new InputEvent("input", {data:"a"}).data` | `null` | `"a"` |
| `input.selectionStart` | `null` | `0` |
| `select`, `setSelectionRange` | `undefined` | functions |
| `form.elements` | throws | a collection |

And the window handles no `WindowEvent::KeyboardInput` and no `WindowEvent::Ime`
at all. Nothing anywhere in `crates/cli` mentions a key.

So: **a text field is drawn, is clickable, and is otherwise scenery.**

## What is already here

Rather more than the table suggests, and in the right places.

**The engine already has focus, and more of it than first measured.**
`Dom::focus`/`blur`/`focused` exist, `element.focus()` works,
`document.activeElement` already falls back to `<body>`, and a press already
moves focus to the nearest thing under it that can hold it
(`events/activation.rs`) — so clicking a field focuses it today. `Activated` is
the seam for "the element did something only the browser can carry out", which
today is a navigation.

What focus does *not* do is reach layout. The composed document is re-parsed
from serialised HTML, which says nothing about what is focused, so `:focus`
never matches and blitz's own `input:focus { outline: … }` never fires. Focus is
therefore real and invisible.

**blitz already has a whole text editor.** `TextInputData` holds a parley
`PlainEditor`; `BaseDocument::with_text_input(node, |driver| …)` hands out a
`PlainEditorDriver`; there are `events/{focus,keyboard,ime}.rs` and a `form.rs`.
The editor is **seeded from the `value` attribute at parse time**
(`mutator.rs`), which matters more than anything else here.

**parley answers every geometric question.** `cursor_geometry(size)` is the
caret, `selection_geometry()` is the highlight, `try_layout()` is the glyph run
in the same shape `paint/words.rs` already consumes, `select_byte_range` sets
the selection.

**The pointer pipeline is built.** Press, release and the click derived from
them; a `<webview>` routes; `cursor-icon` already travels from the cascade to
the window.

**And a screen reader can already read the page** — `docs/accessibility.md`, and
`just a11y` is a harness that can type into a field and read it back.

## The one hard question: who owns the caret

`Browser::compose` serialises the live DOM to HTML and **re-parses it into a
fresh blitz document** on every change of revision or viewport. Layout is
derived, never authoritative. So every piece of state blitz's editor
accumulates — the caret, the selection, the scroll offset within the field — is
destroyed several times a second.

Three ways out.

**1. The engine owns everything.** The Realm holds the string and the two
offsets; a key event mutates them. But moving a caret across proportional text,
or placing one where a click landed, is a text-measurement question, and the
engine has no fonts by design. It would have to grow them, which is the one
thing the layering exists to prevent.

**2. blitz's document becomes authoritative.** The editor is already complete,
so this is the least new code — and it inverts the pipeline. The serialise-and
re-parse is what makes measuring, painting and the whole `Keyed::Yes` scheme
work; unpicking it to keep a caret is the tail wagging the dog.

**3. The engine owns the string and the selection; layout measures them.**
*Recommended.* The value and `selectionStart`/`selectionEnd` live in the Realm,
where the DOM is. They reach layout the way everything else does — the value as
the serialised `value` attribute, which blitz already turns into editor text for
free, and the selection as one more **environment fact**, set on the composed
document with `with_text_input(node, |d| d.select_byte_range(..))`. The editor
is then disposable again: rebuilt each compose, authoritative about nothing,
and asked only where things are.

This is the same trade `getBoundingClientRect` already made, in the same
direction, for the same reason — `docs/layers.md`: *the engine resolves this by
not knowing*. A caret is a box, and boxes are measured one layer up.

## The plan

Five stages. Each is worth having on its own, and each is testable before the
next one starts.

### 1. Draw what is in the field — **done**

`paint/fields.rs` hands the editor's parley layout to the same `written` a
paragraph goes through, clipped to the content box and offset by
`scroll_offset`. Corpus cases 080 to 083; `crates/browser/tests/fields.rs`.

Three things this turned up that were not in the plan, and one it did not do.

**A password was being drawn as itself.** Not merely shown to whoever is behind
you: a Scene *names the words it draws*, so the password reached every SVG,
every snapshot and every comparison report. `blitz/fields.rs` replaces the value
with bullets in the laid-out document, before anything downstream holds it — the
mask has to be what was laid out, so that the field is as wide as what it shows.

**A `<textarea>` was empty.** blitz seeds every field's editor from the `value`
attribute, and a textarea has no `value` attribute — its value is the text
between its tags. Same file, same walk.

**And a `<textarea>` had no box at all.** blitz's own user-agent sheet gives it
`border: 1px solid #999` and gives only `input` an `inline-block`, so the
textarea stayed an inline box with nothing to draw the border on. One rule in
`blitz/agent.rs`.

**The placeholder is not drawn.** blitz has no notion of one, so it needs a
parley layout built here rather than read off the editor — which is the same
machinery stage 3 needs anyway, and is folded into it.

### 2. Make the DOM tell the truth about fields — **done**

`dom/fields.rs` holds each field's value and selection; `prelude/35-fields.js`
presents them as `value`, `defaultValue`, `selectionStart`/`End`,
`setSelectionRange`, `select` and `setRangeText`, shadowing the plain attribute
reflection that every other element keeps. `KeyboardEvent` and `InputEvent`
became real classes with real fields.

Two things went differently from the plan.

**The serialisation did not have to lie.** The plan had `Keyed::Yes` writing the
value property out as the attribute. It turned out the browser can simply *ask*
— `Engine::fields` — and that carries the selection as well, which an attribute
could not. So the caret travels the same way, and `innerHTML` still reports what
the markup says.

**Two characterisation tests had to be rewritten**, and both said why they would
have to be: one recorded that `value` and its attribute "move together" because
there was one place to keep them, the other that `KeyboardEvent === Event`
because "nothing here dispatches them". Both reasons expired in this change.

Still outstanding: the `focus`/`blur` events, `:focus` matching (focus does not
reach layout, so blitz's own `input:focus { outline }` never fires — and
outlines are `GAPS.md` gap 1), `CompositionEvent`, and `HTMLFormElement`.

### 3. Editing, as a Transition — **done, and cheaper than planned**

`Engine::raise_key` raises `keydown` → `beforeinput` → the edit → `input` →
`keyup`, and `events/editing.rs` decides what each key means.

**The measurement turned out not to be needed.** The plan assumed Home, End and
the arrow keys were parley's answer rather than a byte count. They are not:
moving a caret one character, to the start of a line, or to the line above is a
question about a *string*, and only two things genuinely need a text layout —
where a click landed inside the text, and Up or Down through lines that wrapped
rather than lines that were typed. Both are still outstanding, and everything
else is `dom/fields.rs`, which has no fonts in it at all.

So there is no `moved_caret` and no new environment fact. What crosses instead
is `Engine::fields`: the value and the selection, applied to the laid-out
document's own editor, which then answers where the caret goes.

Also outstanding: `change` on blur, and Enter submitting a form — the second
deliberately, since this browser does not fake a submit (`docs/adr/0010`).

### 4. The window, and the OS — *partly done*

This is the part called "integrates properly". **Keys are done** —
`window/typing.rs` maps winit's `logical_key`/`physical_key` to DOM `key`/`code`
— so a person can click a field in `just browse` and type into it. The rest is
not:
- **Input methods.** `set_ime_allowed(true)`, `WindowEvent::Ime`
  (`Preedit`/`Commit`) into `CompositionEvent`, and `set_ime_cursor_area` from
  the caret rect stage 3 already computes. Without that last call the candidate
  window for Japanese or Chinese opens in the wrong place, which is the
  difference between having an IME and appearing to.
- **Clipboard.** winit has none. `arboard` for Ctrl+C/X/V, and on X11 and
  Wayland the primary selection on middle click, which is what people actually
  use.
- **The caret blinks**, so `ControlFlow::Wait` becomes `WaitUntil` while a field
  has focus, and goes back when it does not. Focus lost at the window level
  hides the caret and fires `blur`.
- **Tab** moves focus. The I-beam is already done — `agent.rs` has had
  `input, textarea { cursor: text }` since the cursors went in.

**Done for the keys:** `just window <url> 60,44 t:typed k:BackSpace` clicks into
a field, types, deletes a character, and photographs each step.

### 5. A screen reader can use it — *half a session*

Cheaper than it looks: **parley already speaks AccessKit**. `PlainEditor` has
`accessibility(..)` and `select_from_accesskit(..)`, so the character lengths,
positions and widths below are a call rather than a derivation.

The `Reading` already gives a field `Role::TextInput` and a box. A reader needs
three things more, and parley has the numbers for all of them:

- `value` on the node, and the tree's `focus` pointing at the focused node
  rather than always at the window.
- `TextSelection`, which requires `Role::TextRun` children carrying
  `character_lengths`, `character_positions` and `character_widths` — cluster
  byte lengths and glyph advances, which `try_layout()` reports.
- `Action::SetTextSelection`, `ReplaceSelectedText` and `SetValue`, handled the
  way `Click` already is.

**Done when:** the AT-SPI probe in `crates/e2e` types into a field, reads the
value back, and moves the caret — the same test that already follows a link.

## What is deliberately not in here

`contenteditable` and rich text. A `<textarea>` is a plain string with a
selection; an editable document is a DOM with a range, and nothing above
generalises to it. Also: spellcheck, autofill, `<datalist>`, `<select>` popups,
and the date and colour pickers — each of those is a piece of OS integration of
its own and none of them is on the way to this one.

## The shape of the whole thing

Four to six sessions. The first is done, and was worth doing whatever happens to
the rest: **a form that renders as empty boxes is wrong on every page that has a
form**, and that was a paint bug rather than an input feature.

The order is also the order of risk. Stages 1 and 2 touch nothing structural.
Stage 3 is where the design either holds or does not, and the question it
answers is narrow: can a caret be an environment fact? If it can, the rest is
plumbing; if it cannot, the choice between options 1 and 2 above has to be made
for real, and it is better to find that out with the painting and the DOM
already correct.
