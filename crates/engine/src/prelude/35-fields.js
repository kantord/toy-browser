// What a form field holds, and what is selected in it.
//
// `value` on an `<input>` is not the `value` attribute. The attribute says what
// the field *started* with and never moves again; the property says what is in
// it now. A page reads both, and reads them for different reasons — a form
// knows it is dirty by comparing them — so reflecting one onto the other makes
// "has this been edited" answer no forever.
//
// Every other element keeps the plain reflection it has on `Node.prototype`: an
// `<option value>`, a `<button value>` and a `<li value>` really are their
// attributes. These two shadow it, which is what a prototype chain is for.
//
// The state itself is Rust's — see `dom/fields.rs` — because there is nowhere
// in the markup to keep it.

(() => {
  const FIELDS = [globalThis.HTMLInputElement, globalThis.HTMLTextAreaElement];

  const define = (proto, name, get, set) =>
    Object.defineProperty(proto, name, { get, set, configurable: true });

  for (const interface_ of FIELDS) {
    const proto = interface_.prototype;

    define(
      proto,
      "value",
      function () {
        return __dom.fieldValue(this.__id);
      },
      function (next) {
        __dom.setFieldValue(this.__id, next == null ? "" : String(next));
      },
    );

    // What the markup said. For a `<textarea>` that is the text between its
    // tags, which is why writing it writes the text rather than an attribute.
    define(
      proto,
      "defaultValue",
      function () {
        return __dom.defaultValue(this.__id);
      },
      function (next) {
        const said = next == null ? "" : String(next);
        if (this.tagName === "TEXTAREA") this.textContent = said;
        else this.setAttribute("value", said);
      },
    );

    // A field nobody has been in has no caret anywhere, and a browser answers 0
    // rather than inventing one — so the zero here is the same claim, not a
    // fallback hiding a missing answer.
    define(
      proto,
      "selectionStart",
      function () {
        return (__dom.fieldRange(this.__id) ?? [0, 0])[0];
      },
      function (at) {
        this.setSelectionRange(at, this.selectionEnd);
      },
    );

    define(
      proto,
      "selectionEnd",
      function () {
        return (__dom.fieldRange(this.__id) ?? [0, 0])[1];
      },
      function (at) {
        this.setSelectionRange(this.selectionStart, at);
      },
    );

    // Which end of the selection the caret is at. Always reported forwards:
    // nothing here drags a selection backwards yet, and saying "forward" is
    // what a browser says for a selection made any other way.
    define(
      proto,
      "selectionDirection",
      function () {
        return "forward";
      },
      function () {},
    );

    proto.setSelectionRange = function setSelectionRange(start, end) {
      const from = Math.max(0, Number(start) || 0);
      const to = Math.max(from, Number(end) || 0);
      __dom.setFieldRange(this.__id, from, to);
    };

    proto.select = function select() {
      this.setSelectionRange(0, this.value.length);
    };

    // Replace a stretch of the value without disturbing the rest, which is how
    // an autocomplete puts its suggestion in. Defaults to the selection, which
    // is what makes `setRangeText(x)` mean "replace what is selected".
    proto.setRangeText = function setRangeText(replacement, start, end) {
      const from = start === undefined ? this.selectionStart : Number(start);
      const to = end === undefined ? this.selectionEnd : Number(end);
      const was = this.value;
      const before = [...was].slice(0, from).join("");
      const after = [...was].slice(to).join("");
      this.value = before + String(replacement) + after;
    };
  }
})();
