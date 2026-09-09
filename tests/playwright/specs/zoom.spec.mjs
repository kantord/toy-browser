// The same pages at six zoom levels, read from both browsers.
//
// Zoom is not a magnifying glass: the page is laid out in a viewport narrower
// by the zoom and drawn that much bigger, so every number in the layout is a
// number that could be wrong in a way it is not at 100%. A rule read in `em`,
// a border rounded to a whole pixel, a line that fits at one size and wraps at
// another — none of them is exercised by a suite that only ever looks at a page
// one way.
//
// Both sides are asked the same way. `deviceScaleFactor` is how a client says
// "this many real pixels per CSS pixel" over CDP, Chromium lays out in the CSS
// viewport it is given, and so do we; the pictures come out the same size and
// the boxes are in the same units.
//
// This is the same ratchet the corpus is: an expected file per zoom, recording
// exactly what disagrees today. Rewrite it deliberately with UPDATE_CORPUS=1
// and read the diff.

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { test, expect } from "@playwright/test";
import { chromium } from "playwright-core";

import { disagreements } from "../disagree.mjs";
import { EXPORT } from "../export.mjs";
import { serve } from "../serve.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const CORPUS = resolve(REPO, "tests/corpus");
const EXPECTED = resolve(CORPUS, "expected/zoom");

/// The window, in the pixels a screen has. The CSS viewport is this divided by
/// the zoom, which is what makes one zoom comparable with another.
const WINDOW = { width: 800, height: 600 };

/// Where it goes wrong is rarely where it is obvious, so: a real page with
/// tables and links, and the two cases most likely to round differently.
const PAGES = ["900-hackernews.frozen.html", "021-block-border.html", "040-inline-span.html"];

const ZOOMS = [50, 75, 100, 150, 200, 400];

/** Every element both browsers report, keyed the same way. */
async function read(page, url, zoom) {
  await page.setViewportSize({
    width: Math.round((WINDOW.width * 100) / zoom),
    height: Math.round((WINDOW.height * 100) / zoom),
  });
  await page.goto(url);
  return page.evaluate(EXPORT);
}

test.setTimeout(5 * 60 * 1000);

test("every zoom level disagrees exactly as much as it did", async () => {
  const ours = await serve(9226);
  const toy = await chromium.connectOverCDP(ours.url);
  const real = await chromium.launch();
  const pages = {
    ours: await toy.contexts()[0].newPage(),
    theirs: await real.newPage(),
  };

  await mkdir(EXPECTED, { recursive: true });
  const failures = [];
  for (const zoom of ZOOMS) {
    const found = [];
    for (const name of PAGES) {
      const url = `file://${resolve(CORPUS, name)}`;
      const mine = await read(pages.ours, url, zoom);
      const theirs = await read(pages.theirs, url, zoom);
      found.push(`## ${name}`, ...disagreements(mine, theirs), "");
    }
    const now = `${found.join("\n")}\n`;
    const at = resolve(EXPECTED, `${zoom}.txt`);
    if (process.env.UPDATE_CORPUS) {
      await writeFile(at, now);
      continue;
    }
    const before = await readFile(at, "utf8").catch(() => null);
    if (before === null) failures.push(`${zoom}%: no expected file — run with UPDATE_CORPUS=1`);
    else if (before !== now) failures.push(`${zoom}%:\n  was:\n${before}  now:\n${now}`);
  }

  await toy.close();
  await real.close();
  ours.stop();
  expect(failures.join("\n"), "a zoom level moved").toBe("");
});
