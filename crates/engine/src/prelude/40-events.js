// Events: the listener table every target shares, and the event objects a
// page can construct.
//
// A dispatch travels: down from `window` to the target capturing, then back out
// bubbling if the event is the kind that does. The walk itself is Rust — see
// `realm/node/events.rs` — so an ancestor nobody registered on costs nothing.
// What is left here is the shape of an event, and the flags the walk reads.

(() => {
  const tb = globalThis.__tb;

  // The listener table lives in Rust — see `realm/node/support.rs` — because it
  // outlives every call and has to be released with the Realm. `window` is not
  // a node, so it needs a key of its own.
  tb.WINDOW = "window";
  tb.addListener = __dom.addListener;
  tb.removeListener = __dom.removeListener;
  tb.dispatch = __dom.dispatch;

  // Constructible events. Only the fields anything here reads are real.
  class Event {
    constructor(type, init = {}) {
      this.type = String(type);
      this.bubbles = !!init.bubbles;
      this.cancelable = !!init.cancelable;
      this.defaultPrevented = false;
      this.target = null;
      this.currentTarget = null;
      this.eventPhase = 0;
      // Read by the walk between targets and between listeners. Own properties
      // rather than closure state, because Rust is what reads them.
      this.__stopped = false;
      this.__stoppedImmediate = false;
    }
    preventDefault() {
      this.defaultPrevented = true;
    }
    stopPropagation() {
      this.__stopped = true;
    }
    // Stops the rest of this target's listeners as well as the rest of the
    // walk, so it sets both.
    stopImmediatePropagation() {
      this.__stopped = true;
      this.__stoppedImmediate = true;
    }
  }

  class CustomEvent extends Event {
    constructor(type, init = {}) {
      super(type, init);
      this.detail = init.detail ?? null;
    }
  }

  // A key event a page constructs. The fields are real because pages read
  // them: a shortcut handler is `if (e.key === "k" && e.metaKey)`, and an
  // `Event` that answers `undefined` to both makes every one of them dead.
  class KeyboardEvent extends Event {
    constructor(type, init = {}) {
      super(type, init);
      this.key = init.key ?? "";
      this.code = init.code ?? "";
      this.location = init.location ?? 0;
      this.repeat = !!init.repeat;
      this.isComposing = !!init.isComposing;
      this.ctrlKey = !!init.ctrlKey;
      this.shiftKey = !!init.shiftKey;
      this.altKey = !!init.altKey;
      this.metaKey = !!init.metaKey;
      // Long dead, and still read: jQuery and everything built on it branch on
      // `which`. Answered from `key` so it is at least right for a letter.
      this.charCode = 0;
      this.keyCode = tb.keyCodeOf(this.key);
      this.which = this.keyCode;
    }
    getModifierState(name) {
      return tb.modifierState(this, name);
    }
  }

  // What the page is told the field did, after it did it.
  class InputEvent extends Event {
    constructor(type, init = {}) {
      super(type, init);
      this.data = init.data ?? null;
      this.inputType = init.inputType ?? "";
      this.isComposing = !!init.isComposing;
    }
  }

  globalThis.Event = Event;
  globalThis.CustomEvent = CustomEvent;
  globalThis.UIEvent = Event;
  globalThis.MouseEvent = Event;
  globalThis.KeyboardEvent = KeyboardEvent;
  globalThis.FocusEvent = Event;
  globalThis.InputEvent = InputEvent;
  globalThis.PointerEvent = Event;

  // Which modifier a name asks about. The DOM names more of these than any
  // keyboard has; the four that exist are answered and the rest are false,
  // which is what a machine without a Hyper key would say anyway.
  tb.modifierState = (event, name) =>
    ({
      Control: event.ctrlKey,
      Shift: event.shiftKey,
      Alt: event.altKey,
      Meta: event.metaKey,
      AltGraph: event.altKey,
      OS: event.metaKey,
    })[name] ?? false;

  // The legacy `keyCode`, for the pages that still branch on it. Enough of the
  // table to cover what anybody actually tests for, and 0 for the rest — which
  // is honest, where a wrong number would be believed.
  const KEY_CODES = {
    Backspace: 8, Tab: 9, Enter: 13, Shift: 16, Control: 17, Alt: 18,
    Escape: 27, " ": 32, PageUp: 33, PageDown: 34, End: 35, Home: 36,
    ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40, Delete: 46,
  };
  tb.keyCodeOf = (key) => {
    if (KEY_CODES[key] !== undefined) return KEY_CODES[key];
    if (key.length === 1) return key.toUpperCase().charCodeAt(0);
    return 0;
  };

  // The events the browser itself raises. These carry a target from the start,
  // and say whether they travel: most lifecycle events do not, which is why
  // `bubbles` is off unless a caller asks for it.
  tb.makeEvent = (type, target, bubbles = false) => ({
    type,
    target,
    currentTarget: target,
    bubbles,
    eventPhase: 0,
    defaultPrevented: false,
    __stopped: false,
    __stoppedImmediate: false,
    preventDefault() {
      this.defaultPrevented = true;
    },
    stopPropagation() {
      this.__stopped = true;
    },
    stopImmediatePropagation() {
      this.__stopped = true;
      this.__stoppedImmediate = true;
    },
  });

  // A mouse event, built here rather than in Rust so every field an event
  // carries is written in one place. Rust supplies only what it knows: where
  // the pointer was, what the buttons were doing, and which press this is.
  tb.makeMouseEvent = (type, x, y, buttons, detail) => {
    const event = tb.makeEvent(type, null, true);
    event.cancelable = true;
    event.detail = detail;
    event.button = 0;
    event.buttons = buttons;
    // Nothing scrolls, so the viewport and the document are the same surface
    // and every pair of coordinates is the same pair.
    event.clientX = x;
    event.clientY = y;
    event.pageX = x;
    event.pageY = y;
    event.screenX = x;
    event.screenY = y;
    event.altKey = false;
    event.ctrlKey = false;
    event.metaKey = false;
    event.shiftKey = false;
    event.relatedTarget = null;
    return event;
  };

  // The bits `Key::held` packs the four modifiers into. Named here because
  // this is the only place that unpacks them.
  const CTRL = 1, SHIFT = 2, ALT = 4, META = 8;

  // A key event the browser itself raised. Built here rather than in Rust for
  // the reason the mouse one is: every field an event carries is written in one
  // place, and Rust supplies only what it knows.
  tb.makeKeyEvent = (type, key, code, held, repeat) => {
    const event = tb.makeEvent(type, null, true);
    event.cancelable = true;
    event.key = key;
    event.code = code;
    event.location = 0;
    event.repeat = repeat;
    event.isComposing = false;
    event.ctrlKey = !!(held & CTRL);
    event.shiftKey = !!(held & SHIFT);
    event.altKey = !!(held & ALT);
    event.metaKey = !!(held & META);
    event.charCode = 0;
    event.keyCode = tb.keyCodeOf(key);
    event.which = event.keyCode;
    event.getModifierState = function (name) {
      return tb.modifierState(this, name);
    };
    return event;
  };

  // `beforeinput` can be refused; `input` reports what already happened, so it
  // cannot. That is the only difference between them and it is why one call
  // builds both.
  tb.makeInputEvent = (type, inputType, data) => {
    const event = tb.makeEvent(type, null, true);
    event.cancelable = type === "beforeinput";
    event.inputType = inputType;
    event.data = data ?? null;
    event.isComposing = false;
    return event;
  };

  // An `on*` attribute is a function body, compiled on first use.
  tb.runInlineHandler = (id, attribute, event) => {
    const source = __dom.getAttribute(id, attribute);
    if (!source) return;
    try {
      new Function("event", source).call(tb.wrap(id), event);
    } catch (error) {
      __console.error(`${attribute} handler threw: ${error}`);
    }
  };
})();
