// How a trace is written down, and how a stack is read.
//
// The notebook half of `tracer.js`, which is the recording half. They are split
// because they change for different reasons: this one when a trace has to
// travel out differently or read differently between two engines, that one when
// a page turns out to reach for something nobody had thought to record.
//
// This runs first and leaves `globalThis.__tb_trace` — `{ note, where }` — for
// the other half to write into. Two files rather than one because they are
// passed as two `--init-script`s, and so cannot share a closure.

(() => {
  const Uncaught = globalThis.Error;
  const trace = [];
  globalThis.__trace = trace;

  const MOST = 20000;

  // How much of the trace survives the trip out. It leaves through an attribute
  // on the root element, because a run that ends without anyone asking still has
  // to be readable afterwards and the document is the one thing `render` always
  // writes out. The cap has to be generous: a truncated trace does not look
  // truncated, it looks like a browser that stopped early — which is exactly the
  // thing being investigated.
  const ROOM = 8_000_000;

  // Where the page was when it did that. Off by default — a stack on every
  // line makes a trace unreadable and enormous — and turned on with
  // `__trace_stacks` when two traces part on *which* call was made rather than
  // on the call itself: the function names on either side name the two
  // branches of whatever `if` the browsers disagreed about.
  //
  // Guarded, because walking a stack means using the very array methods being
  // watched, and an unguarded walk recurses until the stack it was reporting on
  // is gone.
  let walking = false;
  const where = (always) => {
    if (walking || !(always || globalThis.__trace_stacks)) return "";
    walking = true;
    try {
      return " " + frames(new Uncaught());
    } finally {
      walking = false;
    }
  };

  // Written out in batches, because writing it out costs the whole trace each
  // time: a line-by-line flush is quadratic and turns a long trace into a
  // browser that appears to hang. Short runs flush every line, so a page that
  // does almost nothing is still recorded exactly.
  const EVERY = 50;
  const flush = () => {
    // Quotes and newlines would end the attribute early; a trace is read for
    // its shape rather than its punctuation.
    try {
      globalThis.document.documentElement.setAttribute(
        "data-trace",
        trace.join(" ~ ").replace(/["\n]/g, "'").slice(0, ROOM),
      );
    } catch {}
  };

  // Once at the start, so the attribute exists even for a page that never does
  // anything worth recording. A missing attribute reads as a broken run.
  flush();

  const note = (line) => {
    if (trace.length >= MOST) return;
    trace.push(line);
    if (trace.length < EVERY || trace.length % EVERY === 0) flush();
  };


  // frame past the length a trace line is cut to — cutting it somewhere else on
  // each side. What is left is the function and the file, which is what names a
  // frame anyway.
  function frames(error) {
    return (error.stack || "")
      .split("\n")
      .map((line) => line.trim().replace(/^at new /, "at "))
      // Frames only: one engine's stack opens with the message, the other's
      // does not. And not this file's own frames, which only one engine counts.
      .filter((line) => line.startsWith("at ") && !line.includes("Traced"))
      // Frames from a page's own files only. Everything this file wraps adds a
      // frame of its own, and the two engines name it differently — "<anonymous>"
      // there, "eval_script" here — so keeping them parts two traces that record
      // exactly the same thing. The engine's internals are not the page's story.
      .filter(
        (line) =>
          !line.includes("eval_script") &&
          !line.includes("(<anonymous>") &&
          // QuickJS counts a `.apply` as a frame of its own; V8 does not.
          !line.includes("(native)"),
      )
      .slice(0, Number(globalThis.__trace_frames ?? 3))
      .map((line) => {
        const frame = line.match(/^at (?:(.+?) )?\(?([^()\s]+?)(?::\d+:\d+)?\)?$/);
        if (!frame) return line;
        // By the function's own name, not the receiver's: V8 writes a method
        // as "Object.setValue" where QuickJS writes "setValue".
        const called = (frame[1] ?? "<anonymous>").split(".").pop();
        return "at " + called + " (" + frame[2].split("/").pop() + ")";
      })
      .join(" | ")
      .slice(0, Number(globalThis.__trace_frames ?? 3) > 3 ? 600 : 160);
  }


  globalThis.__tb_trace = { note, where, frames };
})();
