// What a real browser has on `globalThis` that this one does not — and, for
// the names the page actually touches, which of them it cannot do without.
import { readFileSync } from "node:fs";
import { chromium } from "playwright-core";

const ours = new Set(readFileSync(process.argv[3], "utf8").split(",").map((it) => it.trim()));
const url = process.argv[2] ?? "https://hcker.news/";
// Every global this browser lacks is replaced with an accessor that answers
// with the real thing and writes down that it was asked. A list of a thousand
// names says nothing; the handful the page actually reaches is the work.
const watch = `
  globalThis.__touched = [];
  const absent = ${JSON.stringify([...ours])};
  const known = new Set(absent);
  for (const name of Object.getOwnPropertyNames(globalThis)) {
    if (known.has(name) || name.startsWith("webkit") || name.startsWith("__")) continue;
    const held = Object.getOwnPropertyDescriptor(globalThis, name);
    if (!held || !held.configurable || held.get) continue;
    const value = held.value;
    try {
      Object.defineProperty(globalThis, name, {
        configurable: true,
        get() {
          if (!globalThis.__touched.includes(name)) globalThis.__touched.push(name);
          return value;
        },
        set(to) {
          Object.defineProperty(globalThis, name, { value: to, configurable: true, writable: true });
        },
      });
    } catch {}
  }
`;

const browser = await chromium.launch();
const page = await browser.newPage();
await page.addInitScript(watch);
try { await page.goto(url, { waitUntil: "networkidle", timeout: 45000 }); } catch {}
const touched = JSON.parse(await page.evaluate(`JSON.stringify(globalThis.__touched)`));
await browser.close();
console.log(touched.length, "globals the page reached for that this browser does not have:\n");
console.log(touched.join(" "));
