// What a page asked the document for, in the order it asked.
//
// The same file runs in both browsers — here through `render --init-script`, in
// Chromium through Playwright's `addInitScript` — which is the whole point. Two
// traces of the same page, diffed, name the first place the two stop agreeing,
// and that place is almost always the bug.
//
// It is a debugger for a browser that has no debugger. Every serious gap found
// in this engine so far was found by reading one of these: a request built with
// headers that threw, a template whose content was undefined, a deadline that
// cancelled its own request, a font set that could not be listened to. None of
// them reported an error; each showed up as two traces parting company.
//
// Load `record.js` before this one — it is the notebook this writes into, and
// `docs/diverging.md` is how the pair is used.

(() => {
  const Uncaught = globalThis.Error;
  const { note, where, frames } = globalThis.__tb_trace;

  // Wrapped so that a throw is recorded before it propagates: a page that stops
  // without a word is the case this exists for.
  const wrap = (owner, name, shape) => {
    const under = owner[name];
    if (typeof under !== "function") {
      note("MISSING " + name);
      return;
    }
    owner[name] = function (...args) {
      let out;
      try {
        out = under.apply(this, args);
      } catch (error) {
        note(name + "(" + shape(args) + ") THREW " + String(error).slice(0, 90));
        throw error;
      }
      note(name + "(" + shape(args) + ")" + (out == null ? " -> null" : "") + where());
      return out;
    };
  };

  const first = (args) => String(args[0]).slice(0, 60);

  // A node as something you can look for afterwards. "DIV" appears four
  // thousand times in a trace and says nothing; "DIV#content" is the line you
  // were looking for.
  const named = (node) => {
    const tag = node.tagName || node.nodeType;
    const id = node.id ? "#" + node.id : "";
    return String(tag) + id;
  };

  for (const name of ["getElementById", "querySelector", "querySelectorAll", "createElement"]) {
    wrap(document, name, first);
  }
  wrap(globalThis, "fetch", first);
  wrap(globalThis, "setTimeout", (args) => String(args[1] ?? 0));

  // Where these live differs between browsers — `replaceChildren` and `append`
  // are on `Element` in a real one and on `Node` here — so each is wrapped
  // wherever it actually is. A trace that says MISSING on one side only would
  // part the two at the first line and hide everything after it.
  const owners = [globalThis.Node.prototype, globalThis.Element?.prototype].filter(Boolean);
  // Only the outermost call is recorded. A native `replaceChildren` does its
  // work without calling anything else a page can see; ours is written in terms
  // of `append`, so tracing both would put lines in one trace that the other
  // browser has no way to produce — a difference in how this engine is built
  // rather than in what the page did.
  let within = 0;
  for (const name of ["cloneNode", "appendChild", "insertBefore", "replaceChildren", "append"]) {
    const node = owners.find((it) => Object.prototype.hasOwnProperty.call(it, name)) ?? owners[0];
    const under = node[name];
    if (typeof under !== "function") continue;
    node[name] = function (...args) {
      const outer = within === 0;
      within += 1;
      try {
        const out = under.apply(this, args);
        if (outer) note("Node." + name + " on " + named(this) + where());
        return out;
      } catch (error) {
        if (outer) note("Node." + name + " THREW " + String(error).slice(0, 90));
        throw error;
      } finally {
        within -= 1;
      }
    };
  }

  // What a request is built out of. An error thrown by the engine itself — a
  // missing method, a bad argument — is never made by calling one of the error
  // constructors below, so it leaves no line of its own; a page that catches
  // one and reports its own message shows only the message. What names the
  // cause is the last thing built before the trace stops: if one browser
  // reaches `new Request` and the other does not, the difference is in what
  // comes between.
  const builders = ["Headers", "Request", "Response", "AbortController", "URL", "URLSearchParams", "FormData"];
  for (const name of builders) {
    const Built = globalThis[name];
    if (typeof Built !== "function") continue;
    const Wrapper = function (...args) {
      note("new " + name + "(" + (args.length ? first(args) : "") + ")");
      return Reflect.construct(Built, args, new.target ?? Wrapper);
    };
    Wrapper.prototype = Built.prototype;
    for (const key of Object.getOwnPropertyNames(Built)) {
      if (["length", "name", "prototype"].includes(key)) continue;
      try {
        Wrapper[key] = Built[key];
      } catch {}
    }
    globalThis[name] = Wrapper;
  }

  // And every method those carry. When two traces agree right up to `new
  // Headers` and part immediately after, the next question is always which call
  // on it was the one that threw — and one line per method answers it without
  // another round of guessing.
  for (const name of builders) {
    const held = globalThis[name];
    const proto = held && held.prototype;
    if (!proto) continue;
    for (const key of Object.getOwnPropertyNames(proto)) {
      if (key === "constructor") continue;
      const held = Object.getOwnPropertyDescriptor(proto, key);
      if (typeof held?.value !== "function") continue;
      const under = held.value;
      proto[key] = function (...args) {
        try {
          return under.apply(this, args);
        } catch (error) {
          note(name + "." + key + " THREW " + String(error).slice(0, 90));
          throw error;
        }
      };
    }
  }

  // Every other global function a page can call, by the same argument: what is
  // wanted is the last call before the trace stops, and there is no telling in
  // advance which one that will be. Lower-case names only — the capitalised
  // ones are constructors, and wrapping those breaks `instanceof` for everyone.
  for (const key of Object.getOwnPropertyNames(globalThis)) {
    if (key[0] !== key[0].toLowerCase() || key.startsWith("__")) continue;
    const held = Object.getOwnPropertyDescriptor(globalThis, key);
    if (typeof held?.value !== "function" || !held.writable) continue;
    if (["note", "wrap", "first", "frames"].includes(key)) continue;
    const under = held.value;
    globalThis[key] = function (...args) {
      try {
        return under.apply(this, args);
      } catch (error) {
        note(key + "() THREW " + String(error).slice(0, 90));
        throw error;
      }
    };
  }

  // Where a list of things became a list of nothing. A page that renders no
  // rows has either fetched no rows or thrown all of them away, and those are
  // different bugs with the same appearance; this tells them apart in one line.
  // Only the emptying is recorded, so an app filtering a hundred lists stays
  // quiet until one of them comes out empty.
  // Guarded, because working out *where* a list emptied means taking a stack
  // apart with the very methods being watched — and an unguarded wrapper
  // recurses until the stack it was reporting on is gone.
  for (const name of ["filter", "slice", "splice"]) {
    const under = Array.prototype[name];
    Array.prototype[name] = function (...args) {
      const out = under.apply(this, args);
      if (this.length > 0 && out.length === 0) {
        note("EMPTIED " + name + " of " + this.length + " @" + where(true));
      }
      return out;
    };
  }

  // How big every list the page walks was, and where it walked it.
  //
  // For the case the rest of this file cannot reach: two browsers making the
  // same calls in the same order, and one of them drawing rows the other does
  // not. The difference is then in a value, and a value is invisible to a trace
  // of calls. What is visible is how many things went round each loop — a
  // render that draws eighty rows iterates eighty of something, and a render
  // that draws none iterates none of it, in a function that names itself.
  //
  // Off by default: this is the noisiest thing here by a wide margin.
  if (globalThis.__trace_values) {
    const walked = Array.prototype[Symbol.iterator];
    Array.prototype[Symbol.iterator] = function () {
      note("ITER " + this.length + where(true));
      return walked.call(this);
    };
    for (const name of ["keys", "values", "entries"]) {
      const under = Object[name];
      Object[name] = function (held) {
        const out = under.call(this, held);
        note("OBJ " + name + " " + out.length + where(true));
        return out;
      };
    }
    for (const held of [Map, Set]) {
      const walked = held.prototype[Symbol.iterator];
      held.prototype[Symbol.iterator] = function () {
        note("ITER " + held.name + " " + this.size + where(true));
        return walked.call(this);
      };
    }
  }


  // An error a page catches is invisible from outside, and a page that shows a
  // failure message has caught one. This is how to read it.
  //
  // Every error constructor, not only `Error`: a `TypeError` is not made by
  // calling `Error`, so wrapping that one alone leaves the commonest failure of
  // all — a call into something this engine implements differently, caught by
  // the page and reported as its own polite message — completely invisible.
  const trace_errors = (kind) => {
    const Real = globalThis[kind];
    if (typeof Real !== "function") return;
    const Traced = function (...args) {
      const made = new Real(...args);
      note(kind.toUpperCase() + " " + String(args[0]).slice(0, 60) + " @ " + frames(made));
      return made;
    };
    Traced.prototype = Real.prototype;
    for (const key of Object.getOwnPropertyNames(Real)) {
      if (["length", "name", "prototype"].includes(key)) continue;
      try {
        Traced[key] = Real[key];
      } catch {}
    }
    globalThis[kind] = Traced;
  };

  for (const kind of ["Error", "TypeError", "RangeError", "SyntaxError", "ReferenceError", "DOMException"]) {
    trace_errors(kind);
  }
})();
