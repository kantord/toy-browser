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
| `document.activeElement` | `""` before anything is focused | `<body>` |
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

**The engine already has focus.** `Dom::focus`/`blur`/`focused` exist,
`element.focus()` works, and `Activated` is already the seam for "the element
did something only the browser can carry out" — today that is a navigation.

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

### 1. Draw what is in the field  — *half a session*

`paint/words.rs` reads `inline_layout_data` and nothing else, so a field's text
is never drawn. Add a paint phase that walks `text_input_data`, takes
`try_layout()`, and emits the same glyph Marks — plus the placeholder when the
value is empty, clipped to the content box, offset by `scroll_offset`.

No new state, no events. It closes the worst of the table above: a filled form
currently screenshots as a row of empty boxes, which makes every corpus and WPT
comparison of a form page wrong before anything else is considered.

**Done when:** a corpus case with a filled input and a `<textarea>` agrees with
Chromium.

### 2. Make the DOM tell the truth about fields — *one session*

In the engine, and all of it unit-testable with no window:

- `value` as a real property, distinct from `defaultValue` and the attribute,
  for `input` and `textarea` (whose value is its text content).
- `selectionStart` / `selectionEnd` / `selectionDirection`, `select()`,
  `setSelectionRange()`, `setRangeText()`.
- `activeElement` defaulting to `<body>`; `focus`/`blur`/`focusin`/`focusout`.
- `KeyboardEvent` with `key`, `code`, `location`, `repeat`, the modifiers and
  `getModifierState`; `InputEvent` with `data`, `inputType`, `isComposing`;
  `CompositionEvent`.
- `HTMLFormElement`: `elements`, `reset()`, `requestSubmit()`.

The serialisation must carry the current value out to layout — for `<input>` by
writing the *property* as the `value` attribute in `Keyed::Yes`, which is a
lie about the DOM told deliberately and in one place, the way the `__tb-key-`
classes already are.

**Done when:** the probe table above answers correctly, in `cargo test`.

### 3. Editing, as a Transition — *one to two sessions*

A new engine primitive beside `raise_mouse`: `raise_key(node, Key)`, raising
`keydown` → `beforeinput` → the edit → `input` → `keyup`, with `change` on blur
and `submit` on Enter inside a form. `Activated` grows `Submit(..)` next to
`Navigate(..)`, because a submission is a navigation the browser carries out
after the dispatch unwinds — exactly the existing shape.

Which *edit* a key performs is the part that needs measurement: Home, End, and
the arrow keys across a proportional run are parley's answer, not a byte count.
So `Browser` gains a `moved_caret(page, node, how) -> (usize, usize)` that
composes, asks the editor, and hands the offsets back. One more environment
fact in the same direction as the boxes.

Clicking into a field, and dragging to select, are the same question asked from
a Point.

**Done when:** a browser-level test types into a field, selects a word, deletes
it and reads the value back — no window involved.

### 4. The window, and the OS — *one to two sessions*

This is the part called "integrates properly".

- **Keys.** `WindowEvent::KeyboardInput` → winit's `logical_key`/`physical_key`
  mapped to DOM `key`/`code`. Repeat comes free.
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
- **Tab** moves focus; `input { cursor: text }` in the user-agent sheet gives an
  I-beam through machinery that already exists.

**Done when:** `just window` types a string into a field and photographs it.

### 5. A screen reader can use it — *half a session*

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

Four to six sessions, and the first one is worth doing whatever happens to the
rest: **a form that renders as empty boxes is wrong on every page that has a
form**, and that is a paint bug rather than an input feature.

The order is also the order of risk. Stages 1 and 2 touch nothing structural.
Stage 3 is where the design either holds or does not, and the question it
answers is narrow: can a caret be an environment fact? If it can, the rest is
plumbing; if it cannot, the choice between options 1 and 2 above has to be made
for real, and it is better to find that out with the painting and the DOM
already correct.
