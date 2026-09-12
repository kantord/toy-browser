// Which members of an element a page reaches for that this browser has not got.
//
//   node tests/playwright/untouched.mjs <url> <this-browser's-member-list>
//
// The same measurement `missing.mjs` makes for globals, one level down. Every
// member Chromium has and this browser does not is replaced with an accessor
// that answers with the real thing and writes down that it was asked — because
// a list of four hundred names says nothing, and the handful a page actually
// touches is the work.
//
// This is the tool for the failure that leaves no trace at all: reaching a
// member that is not there is a TypeError built by the engine, never by calling
// `TypeError`, so no error tracing can see it and a page that catches it
// reports its own polite message instead.

import { readFileSync } from "node:fs";
import { chromium } from "playwright-core";

const [url, listing] = process.argv.slice(2);
if (!url || !listing) {
  console.error("usage: node tests/playwright/untouched.mjs <url> <members.txt>");
  process.exit(2);
}

const ours = readFileSync(listing, "utf8").split(",").map((it) => it.trim());

const browser = await chromium.launch();
const page = await browser.newPage();
await page.addInitScript(`
  globalThis.__touched = [];
  const known = new Set(${JSON.stringify(ours)});
  for (const proto of [Node.prototype, Element.prototype, HTMLElement.prototype,
                       CharacterData.prototype, DOMTokenList.prototype]) {
    for (const name of Object.getOwnPropertyNames(proto)) {
      if (known.has(name) || name === "constructor") continue;
      const held = Object.getOwnPropertyDescriptor(proto, name);
      if (!held || !held.configurable) continue;
      const seen = () => {
        const said = proto.constructor.name + "." + name;
        if (!globalThis.__touched.includes(said)) globalThis.__touched.push(said);
      };
      try {
        if (held.get || held.set) {
          Object.defineProperty(proto, name, {
            configurable: true,
            get() { seen(); return held.get ? held.get.call(this) : undefined; },
            set(to) { seen(); if (held.set) held.set.call(this, to); },
          });
        } else if (typeof held.value === "function") {
          const under = held.value;
          Object.defineProperty(proto, name, {
            configurable: true, writable: true,
            value(...args) { seen(); return under.apply(this, args); },
          });
        }
      } catch {}
    }
  }
`);
try {
  await page.goto(url, { waitUntil: "networkidle", timeout: 45000 });
} catch {
  // A page that never settles has still done everything it is going to do.
}
const touched = JSON.parse(await page.evaluate(`JSON.stringify(globalThis.__touched)`));
await browser.close();
console.log(touched.length, "element members the page reached for that this browser has not got:\n");
console.log(touched.join(" "));
