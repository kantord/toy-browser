// Chromium, asked what a page cannot do without.
//
//   node tests/playwright/oracle.mjs <url> <count expression> [shim…]
//   node tests/playwright/oracle.mjs https://hcker.news/ ".story"
//
// Renders the page as Chromium, then again with one thing changed, and counts
// what came out each time. Two numbers settle an argument that guessing cannot:
// if taking a capability away costs nothing, it is not the bug, however missing
// it looks — and if putting this browser's version of something *in* costs
// sixty rows, that shim is the bug, whatever else is also wrong.
//
// This is the measurement the whole hunt rests on. `IntersectionObserver` was
// blamed here for a day; an observer that never fires still renders every row,
// and only deleting the constructor breaks anything. A silent stub is fine. A
// missing constructor is not, because `new` on one throws, and a page that
// wraps its render in a `try` reports a failure about data it already has.

import { readFileSync } from "node:fs";
import { chromium } from "playwright-core";

const [url, counted = ".story", ...shims] = process.argv.slice(2);
if (!url) {
  console.error("usage: node tests/playwright/oracle.mjs <url> <selector> [prelude-file…]");
  process.exit(2);
}

// Taking something away. Each is written so that the page sees the same absence
// a browser without it would show — a deleted constructor, not a broken one.
const without = {
  indexedDB: `delete window.indexedDB;`,
  "canvas 2d": `HTMLCanvasElement.prototype.getContext = () => null;`,
  DOMParser: `delete window.DOMParser;`,
  XMLHttpRequest: `delete window.XMLHttpRequest;`,
  IntersectionObserver: `delete window.IntersectionObserver;`,
  ResizeObserver: `delete window.ResizeObserver;`,
  "idle callbacks": `delete window.requestIdleCallback; delete window.cancelIdleCallback;`,
  Segmenter: `delete Intl.Segmenter;`,
};

// Putting one of this browser's own shims in. The prelude files are plain
// scripts that assign globals, so a real browser can run one — which turns
// "does our version of this behave?" into a number.
const swapped = (path) => `
  globalThis.__tb = { scheme: "light" };
  ${readFileSync(path, "utf8")}
`;

const count = async (change) => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  if (change) await page.addInitScript(change);
  try {
    await page.goto(url, { waitUntil: "networkidle", timeout: 45000 });
  } catch {
    // A page that never settles has still done everything it is going to do.
  }
  const rows = await page.evaluate(
    `document.querySelectorAll(${JSON.stringify(counted)}).length`,
  );
  await browser.close();
  return rows;
};

const say = (rows, label) => console.log(String(rows).padStart(5), label);

const asItself = await count(null);
say(asItself, `as Chromium — ${counted}`);
for (const [name, script] of Object.entries(without)) {
  say(await count(script), `without ${name}`);
}
for (const path of shims) {
  say(await count(swapped(path)), `with our ${path.split("/").pop()}`);
}
