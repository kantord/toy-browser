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
// reflects what they did.
//
// Images are saved beside the page under the names it already uses, so its own
// relative references find them. Not inlined as data URIs: a real site styles
// its images by source — `img[src="s.gif"]` is an actual rule on this one — and
// rewriting the source silently stops those rules matching, which changes the
// layout. Freezing a page must not edit it.

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

  // Everything the page points at, so it can be fetched and saved beside it.
  const referenced = new Set();
  for (const image of document.querySelectorAll("img[src]")) {
    referenced.add(image.getAttribute("src"));
  }
  for (const match of css.matchAll(/url\(["']?([^"')]+)["']?\)/g)) {
    referenced.add(match[1]);
  }

  return {
    html: `<!DOCTYPE html><html><head><meta charset="utf-8">
<style>${css}</style></head>
<body>${document.body.innerHTML}</body></html>`,
    referenced: [...referenced].filter((src) => src && !src.startsWith("data:")),
    from: location.href,
  };
};

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(target);
const { html, referenced, from } = await page.evaluate(SELF_CONTAINED);
await writeFile(into, html);

// Beside the page, under the name the page uses, so its own references resolve.
let saved = 0;
for (const src of referenced) {
  if (src.includes("..") || src.startsWith("/") || /^[a-z]+:/i.test(src)) continue;
  const response = await page.request.get(new URL(src, from).href).catch(() => null);
  if (!response?.ok()) continue;
  await writeFile(resolve(dirname(into), src.split("?")[0]), await response.body());
  saved += 1;
}
console.log(`froze ${target} into ${into} (${html.length} bytes, ${saved} images)`);
await browser.close();
