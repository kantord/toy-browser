// The corpus: small pages, each isolating one thing, read from both browsers.
//
// The DOM only — no screenshots — so nothing here depends on font rasterizing
// and a disagreement is about the document rather than about pixels. Cases are
// numbered by complexity: when several fail, the lowest-numbered one is usually
// why the rest do.
//
// Each case has an expected file recording exactly which boxes disagree today.
// That is a ratchet, not a target: a toy browser is not going to agree about
// everything, and what matters is that the list only ever gets shorter. Rewrite
// it deliberately with UPDATE_CORPUS=1, and the diff is the review.

import { readdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { test, expect } from "@playwright/test";
import { chromium } from "playwright-core";

import { EXPORT } from "../export.mjs";
import { serve } from "../serve.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const CORPUS = resolve(REPO, "tests/corpus");
const VIEWPORT = { width: 800, height: 600 };

/** How the two accounts of one case differ, as lines anybody can read. */
function disagreements(ours, theirs) {
  const mine = new Map(ours.nodes.map((node) => [node.path, node]));
  return theirs.nodes
    .map((node) => {
      const ours = mine.get(node.path);
      if (!ours) return `${node.tag} ${node.path}  missing here`;
      if (ours.tag !== node.tag) return `${node.path}  we say ${ours.tag}, they say ${node.tag}`;
      if (JSON.stringify(ours.rect) === JSON.stringify(node.rect)) return null;
      return `${node.tag} ${node.path}  ours ${JSON.stringify(ours.rect)}  theirs ${JSON.stringify(node.rect)}`;
    })
    .filter(Boolean);
}

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

  const cases = (await readdir(CORPUS)).filter((n) => n.endsWith(".html")).sort();
  expect(cases.length, "the corpus is empty").toBeGreaterThan(0);
  const failures = [];

  for (const name of cases) {
    const url = `file://${resolve(CORPUS, name)}`;
    const read = {};
    for (const [side, page] of Object.entries(pages)) {
      await page.goto(url);
      read[side] = await page.evaluate(EXPORT);
    }

    const found = `${disagreements(read.ours, read.theirs).join("\n")}\n`;
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
