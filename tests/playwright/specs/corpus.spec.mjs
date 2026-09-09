// The corpus: small pages, each isolating one thing, read from both browsers.
//
// Two accounts of each case, because neither one alone is enough.
//
// **The boxes**, from the DOM, which depend on no font rasterizing and say the
// document laid out the same. They cannot see a colour: an element painted
// entirely the wrong shade lays out perfectly, and the `a:link` rule that makes
// every story title on Hacker News black was missing for a week without one
// number here moving.
//
// **What was painted**, from a screenshot of each, compared per element by
// `toy-browser compare`. Not the raw score — that is dominated by which
// rasterizer draws a heavier stem — but the colour of each element's ink and
// its ground, read as an extreme so it says what colour a thing is rather than
// how much of it landed.
//
// Cases are numbered by complexity: when several fail, the lowest-numbered one
// is usually why the rest do.
//
// Each case has an expected file recording exactly which boxes disagree today.
// That is a ratchet, not a target: a toy browser is not going to agree about
// everything, and what matters is that the list only ever gets shorter. Rewrite
// it deliberately with UPDATE_CORPUS=1, and the diff is the review.

import { execFile } from "node:child_process";
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { test, expect } from "@playwright/test";
import { chromium } from "playwright-core";

import { disagreements } from "../disagree.mjs";
import { EXPORT } from "../export.mjs";
import { serve } from "../serve.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const CORPUS = resolve(REPO, "tests/corpus");
const INTO = resolve(REPO, "out/corpus");
const VIEWPORT = { width: 800, height: 600 };
const run = promisify(execFile);

/**
 * What `toy-browser compare` makes of one case, from the four files it reads.
 *
 * Shelling out rather than reimplementing: the comparator already owns the
 * owner map, the percentiles and the thresholds, and a second copy of those
 * here would drift from the one `just compare` reports.
 */
async function painting(name, shots, exports) {
  const dir = resolve(INTO, name.replace(".html", ""));
  await mkdir(dir, { recursive: true });
  await Promise.all([
    writeFile(resolve(dir, "toy.png"), shots.ours),
    writeFile(resolve(dir, "chromium.png"), shots.theirs),
    writeFile(resolve(dir, "toy.json"), JSON.stringify(exports.ours)),
    writeFile(resolve(dir, "chromium.json"), JSON.stringify(exports.theirs)),
  ]);
  const { stdout } = await run(resolve(REPO, "target/debug/toy-browser"), [
    "compare",
    "--dir",
    dir,
    "--json",
  ]);
  const { painted_differently: count, colour_apart: apart } = JSON.parse(stdout);
  return count === 0
    ? []
    : [`painted differently: ${count} elements, ${apart.toFixed(3)} apart in colour in total`];
}

/** How far apart one element is: the worst of how much it moved and resized. */
/**
 * Where the two browsers computed a different style for the same element.
 *
 * Not a position and not a pixel: this is what layout was *told* to do, which
 * is where a wrong colour is a fact rather than an inference. It is also the
 * only account of an element laid out inline, which has no box here to compare.
 */
const UNRENDERED = new Set(["HEAD", "STYLE", "SCRIPT", "TITLE", "META", "LINK", "BASE"]);

function restyled(ours, theirs) {
  const mine = new Map(ours.nodes.map((node) => [node.path, node]));
  const lines = theirs.nodes.flatMap((node) => {
    const ours = mine.get(node.path);
    // A browser computes a style for an element it never draws; a renderer that
    // drops it does not. Comparing those reports four lines of nothing on every
    // page in the corpus.
    if (UNRENDERED.has(node.tag)) return [];
    if (!ours || !ours.style || !node.style) return [];
    return Object.keys(node.style)
      .filter((property) => ours.style[property] !== node.style[property])
      .map(
        (property) =>
          `${node.tag} ${node.path}  ${property}: ${ours.style[property]}, theirs ${node.style[property]}`,
      );
  });
  return lines.length === 0 ? [] : [`${lines.length} styles computed differently`, ...lines];
}

// Every case is read from two browsers, screenshotted twice and compared by a
// subprocess, and one of them is a real page. That is minutes of work, not the
// seconds a unit test gets.
test.setTimeout(5 * 60 * 1000);

test("every corpus case disagrees exactly as much as it did", async () => {
  // Its own browser, so its cache is as new as the corpus files are.
  const ours = await serve(9224);
  const toy = await chromium.connectOverCDP(ours.url);
  const real = await chromium.launch();
  const pages = {
    ours: await toy.contexts()[0].newPage(),
    theirs: await real.newPage(),
  };
  await Promise.all(Object.values(pages).map((p) => p.setViewportSize(VIEWPORT)));

  await rm(INTO, { recursive: true, force: true });
  const cases = (await readdir(CORPUS)).filter((n) => n.endsWith(".html")).sort();
  expect(cases.length, "the corpus is empty").toBeGreaterThan(0);
  const failures = [];

  for (const name of cases) {
    const url = `file://${resolve(CORPUS, name)}`;
    const read = {};
    const shots = {};
    for (const [side, page] of Object.entries(pages)) {
      await page.goto(url);
      read[side] = await page.evaluate(EXPORT);
      shots[side] = await page.screenshot();
    }

    const lines = [
      ...disagreements(read.ours, read.theirs),
      ...restyled(read.ours, read.theirs),
      ...(await painting(name, shots, read)),
    ];
    const found = `${lines.join("\n")}\n`;
    const expected = resolve(CORPUS, "expected", name.replace(".html", ".txt"));
    if (process.env.UPDATE_CORPUS) {
      await writeFile(expected, found);
      continue;
    }
    const before = await readFile(expected, "utf8").catch(() => null);
    if (before === null) failures.push(`${name}: no expected file — run with UPDATE_CORPUS=1`);
    else if (before !== found) failures.push(`${name}:\n  was:\n${before}  now:\n${found}`);
  }

  await toy.close();
  await real.close();
  ours.stop();
  expect(failures.join("\n"), "corpus cases moved").toBe("");
});
