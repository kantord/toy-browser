// Taking a real page off the network and putting it in the corpus.
//
// A live page cannot be a baseline: its content changes hourly, so a score
// measured against it moves for reasons nobody controls, and a change of a
// thousandth is indistinguishable from a different set of stories. Frozen, it
// becomes what every other corpus case already is — a page that says the same
// thing every time it is asked.
//
// Stylesheets come with it, because the CSS lives in a file of its own and a
// page without it is not the page. Scripts are dropped, because the DOM already
// reflects what they did. Images are left exactly as they were written and
// simply do not load: they carry `width` and `height`, so they take the same
// room either way, and a page that styles them — `img[src="s.gif"]` is a real
// rule on a real site — goes on matching. Carrying them inline as data URIs
// breaks that rule and changes the layout, which is the opposite of freezing.

import { writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { chromium } from "playwright-core";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const target = process.argv[2] ?? "https://news.ycombinator.com/";
const into = resolve(REPO, "tests/corpus", process.argv[3] ?? "900-hackernews.frozen.html");

/** The page as one self-contained file, reaching for nothing. */
const SELF_CONTAINED = async () => {
  const css = [...document.styleSheets]
    .map((sheet) => {
      try {
        return [...sheet.cssRules].map((rule) => rule.cssText).join("\n");
      } catch {
        return "";
      }
    })
    .join("\n");

  for (const script of document.querySelectorAll("script")) script.remove();

  return `<!DOCTYPE html><html><head><meta charset="utf-8">
<style>${css}</style></head>
<body>${document.body.innerHTML}</body></html>`;
};

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(target);
const frozen = await page.evaluate(SELF_CONTAINED);
await writeFile(into, frozen);
console.log(`froze ${target} into ${into} (${frozen.length} bytes)`);
await browser.close();
