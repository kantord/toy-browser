// A `<canvas>` that answers without drawing.
//
// Nothing here paints: there is no raster surface behind a canvas in this
// browser, and a page that wanted a picture out of one does not get it. What a
// page does get is every call it makes answered, which is a different and much
// more useful thing — because the commonest use of a canvas on a text-heavy
// page is not drawing at all. It is measuring:
//
// ```js
// const width = canvas.getContext("2d").measureText(title).width;
// ```
//
// With no `getContext` at all, that line throws a TypeError made by the engine
// — which no error-tracing can see, because the engine does not build it by
// calling `TypeError` — and a page that wraps its render in a `try` reports a
// failure about data it is already holding. hcker.news measures a story title
// that way, from inside the function that lays out the list, and drew no rows
// for exactly that reason while holding all eighty stories.
//
// Chromium with its canvas context removed renders that page in full, which is
// how the measurement below was settled on: the numbers only have to be
// plausible, not right.

(() => {
  const proto = globalThis.Element.prototype;
  const isCanvas = (node) => String(node.tagName).toUpperCase() === "CANVAS";

  /// The default canvas, in CSS pixels, as HTML defines it.
  const WIDE = 300;
  const TALL = 150;

  /// How wide a character is as a fraction of the font size. Proportional text
  /// averages about half an em across a sentence of ordinary English; a
  /// monospaced face is its own fixed advance and that one is known exactly.
  const AVERAGE = 0.5;
  const FIXED = 0.6;

  const sizeOf = (font) => {
    const found = /(\d+(?:\.\d+)?)px/.exec(String(font));
    return found ? Number(found[1]) : 10;
  };

  const measured = (context, text) => {
    const size = sizeOf(context.font);
    const each = /mono/i.test(String(context.font)) ? FIXED : AVERAGE;
    const width = String(text).length * size * each;
    return {
      width,
      actualBoundingBoxLeft: 0,
      actualBoundingBoxRight: width,
      // The usual proportions of a Latin face, which is what a page reading
      // these is placing text against.
      actualBoundingBoxAscent: size * 0.7,
      actualBoundingBoxDescent: size * 0.2,
      fontBoundingBoxAscent: size * 0.8,
      fontBoundingBoxDescent: size * 0.2,
      emHeightAscent: size * 0.8,
      emHeightDescent: size * 0.2,
    };
  };

  // Everything a page draws with. Each one is accepted and has no effect, which
  // is the honest answer: the call was understood, the picture was not kept.
  const DRAWS = [
    "arc", "arcTo", "beginPath", "bezierCurveTo", "clearRect", "clip",
    "closePath", "drawImage", "ellipse", "fill", "fillRect", "fillText",
    "lineTo", "moveTo", "putImageData", "quadraticCurveTo", "rect", "resetTransform",
    "restore", "rotate", "save", "scale", "setLineDash", "setTransform",
    "stroke", "strokeRect", "strokeText", "transform", "translate",
  ];

  const context2d = (canvas) => {
    const context = {
      canvas,
      font: "10px sans-serif",
      fillStyle: "#000000",
      strokeStyle: "#000000",
      lineWidth: 1,
      lineCap: "butt",
      lineJoin: "miter",
      globalAlpha: 1,
      globalCompositeOperation: "source-over",
      textAlign: "start",
      textBaseline: "alphabetic",
      direction: "ltr",
      imageSmoothingEnabled: true,
      measureText: (text) => measured(context, text),
      getLineDash: () => [],
      isPointInPath: () => false,
      isPointInStroke: () => false,
      // An image of nothing, the size that was asked for.
      getImageData: (x, y, width, height) => ({
        width: Math.max(0, Math.trunc(width)),
        height: Math.max(0, Math.trunc(height)),
        data: new Uint8ClampedArray(Math.max(0, Math.trunc(width) * Math.trunc(height) * 4)),
      }),
      createImageData: (width, height) => context.getImageData(0, 0, width, height),
      // A gradient a page can add stops to and hand back as a fill.
      createLinearGradient: () => ({ addColorStop() {} }),
      createRadialGradient: () => ({ addColorStop() {} }),
      createPattern: () => null,
    };
    for (const name of DRAWS) context[name] = () => {};
    return context;
  };

  const held = new Map();

  Object.defineProperty(proto, "getContext", {
    value(kind) {
      if (!isCanvas(this)) return undefined;
      // Only the 2D context. A page asking for WebGL is asking for something
      // this browser genuinely cannot do, and `null` is how that is said — it
      // is also what a real browser says when the context is unavailable, so
      // the page already has a path for it.
      if (String(kind) !== "2d") return null;
      let context = held.get(this.__nodeId);
      if (!context) {
        context = context2d(this);
        held.set(this.__nodeId, context);
      }
      return context;
    },
    writable: true,
    configurable: true,
  });

  // The drawing surface's own size, which is an attribute and not a style, and
  // which a page both reads and writes.
  for (const [name, fallback] of Object.entries({ width: WIDE, height: TALL })) {
    Object.defineProperty(proto, name, {
      get() {
        if (!isCanvas(this)) return undefined;
        const raw = this.getAttribute(name);
        const found = raw == null ? NaN : Number(raw);
        return Number.isFinite(found) ? found : fallback;
      },
      set(value) {
        if (isCanvas(this)) this.setAttribute(name, String(Math.trunc(Number(value) || 0)));
      },
      configurable: true,
    });
  }

  // A picture of nothing, so a page that exports one gets a URL rather than a
  // thrown call. One transparent pixel.
  const BLANK =
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk" +
    "YPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
  Object.defineProperty(proto, "toDataURL", {
    value() {
      return isCanvas(this) ? BLANK : undefined;
    },
    writable: true,
    configurable: true,
  });
})();
