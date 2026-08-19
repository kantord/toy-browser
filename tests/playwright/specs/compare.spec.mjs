// Capturing the same page from this browser and from a real one.
//
// Both sides are driven through the same Playwright API and read by the same
// export function, so the two accounts are in one format by construction rather
// than by agreement — there is no translation step to get wrong.
//
// This writes artefacts; `toy-browser compare` reads them. `just compare` does
// both. Skipped under TOY_BROWSER_OFFLINE, because the page is a real one.

import { mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { test } from "@playwright/test";
import { chromium } from "playwright-core";

import { EXPORT } from "../export.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const INTO = resolve(REPO, "out/compare");

/** The page to compare. Fixed size, so neither render has to be scaled. */
const TARGET = process.env.COMPARE_URL ?? "https://news.ycombinator.com/";
const VIEWPORT = { width: 1000, height: 800 };

async function capture(page, engine) {
  await page.setViewportSize(VIEWPORT);
  await page.goto(TARGET);
  await writeFile(resolve(INTO, `${engine}.png`), await page.screenshot());
  const exported = await page.evaluate(EXPORT);
  await writeFile(
    resolve(INTO, `${engine}.json`),
    JSON.stringify(exported, null, 1),
  );
  return exported.nodes.length;
}

test.skip(!!process.env.TOY_BROWSER_OFFLINE, "needs the network");

test("captures the same page from both browsers", async () => {
  await mkdir(INTO, { recursive: true });

  const toy = await chromium.connectOverCDP("ws://127.0.0.1:9222/");
  const toyPage = await toy.contexts()[0].newPage();
  const ours = await capture(toyPage, "toy");
  await toy.close();

  // A real one, launched rather than connected to: this is the reference.
  const real = await chromium.launch();
  const realPage = await real.newPage();
  const theirs = await capture(realPage, "chromium");
  await real.close();

  console.log(`CAPTURED toy=${ours} chromium=${theirs} elements into ${INTO}`);
});
