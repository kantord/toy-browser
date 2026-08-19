// Shrinking a page until only the difference is left.
//
// A score says how far apart two browsers are and the report says which element
// is answerable. Neither says *what about it* is wrong. This does the only
// thing that reliably answers that: take the page away a piece at a time, and
// keep every cut that leaves the difference standing.
//
// The reference browser is used as the DOM — it parses the candidate, the cut
// is made in it, and it serializes the result back out. That is one less HTML
// parser to disagree with the two already here.

import { execFileSync, spawn } from "node:child_process";
import { connect } from "node:net";
import { mkdir, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { chromium } from "playwright-core";

import { EXPORT, FREEZE } from "./export.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const BINARY = resolve(REPO, "target/debug/toy-browser");
const INTO = resolve(REPO, "out/reduce");
const COMPARE = resolve(REPO, "out/compare");
const CANDIDATE = resolve(INTO, "candidate.html");
const VIEWPORT = { width: 1000, height: 800 };

/// A cut is kept only if the difference that big remains. Below this the bug
/// has been cut away rather than isolated.
const KEEPS = 0.6;

const target = process.argv[2] ?? "https://news.ycombinator.com/";

/// Its own browser on its own port, so a reduction never fights whatever else
/// is listening and always dies with the run.
const PORT = 9223;

function serve() {
  const server = spawn(BINARY, ["serve", "--port", String(PORT)], {
    stdio: "ignore",
  });
  process.on("exit", () => server.kill());
  return server;
}

const listening = () =>
  new Promise((ok) => {
    const attempt = () => {
      const socket = connect(PORT, "127.0.0.1")
        .on("connect", () => socket.end(ok))
        .on("error", () => setTimeout(attempt, 100));
    };
    attempt();
  });

/** Renders one candidate in both browsers and asks how far apart they are. */
async function score(pages, html) {
  await writeFile(CANDIDATE, html);
  for (const [engine, page] of Object.entries(pages)) {
    await page.goto(`file://${CANDIDATE}`);
    await writeFile(resolve(COMPARE, `${engine}.png`), await page.screenshot());
    await writeFile(
      resolve(COMPARE, `${engine}.json`),
      JSON.stringify(await page.evaluate(EXPORT)),
    );
  }
  const out = execFileSync(BINARY, ["compare", "--json"], { cwd: REPO });
  return JSON.parse(out.toString());
}

/** The element at `path`, removed, and the document serialized back out. */
const CUT = (path) => {
  const at = path.split("/").slice(1).map(Number);
  let node = document.documentElement;
  for (const step of at) {
    node = node.children[step];
    if (!node) return null;
  }
  if (node === document.documentElement || node === document.body) return null;
  node.remove();
  return document.documentElement.outerHTML;
};

/** One CSS rule dropped, and the document serialized back out. */
const CUT_RULE = (index) => {
  const style = document.querySelector("style");
  const sheet = style?.sheet;
  if (!sheet || index >= sheet.cssRules.length) return null;
  sheet.deleteRule(index);
  style.textContent = [...sheet.cssRules].map((rule) => rule.cssText).join("\n");
  return document.documentElement.outerHTML;
};

const RULE_COUNT = () => document.querySelector("style")?.sheet?.cssRules.length ?? 0;

/**
 * Every element: shallowest first so the biggest cuts are tried first, and
 * last sibling first within a depth.
 *
 * The sibling order is not a preference. Cutting an element renumbers the ones
 * after it, so a list built once and walked forwards would start naming the
 * wrong elements the moment a cut lands. Going backwards, a cut only ever
 * renumbers paths already tried — and a path whose ancestor has gone finds
 * nothing and is skipped.
 */
const PATHS = () => {
  const paths = [];
  const visit = (element, path) => {
    paths.push(path);
    const children = element.children;
    for (let i = 0; i < children.length; i += 1) visit(children[i], `${path}/${i}`);
  };
  visit(document.documentElement, "0");
  const depth = (path) => path.split("/").length;
  return paths.sort((a, b) => depth(a) - depth(b) || b.localeCompare(a));
};

/** Whether a candidate still shows the difference we set out to isolate. */
const holds = (verdict, keep) =>
  verdict.score >= keep.score * KEEPS && verdict.cause === keep.cause;

/** Tries every cut a strategy offers, keeping the ones the difference survives. */
async function sweep(pages, current, keep, candidates, cut) {
  let kept = 0;
  for (const candidate of candidates) {
    await pages.chromium.setContent(current);
    const shorter = await pages.chromium.evaluate(cut, candidate);
    if (!shorter || shorter.length >= current.length) continue;
    if (holds(await score(pages, shorter), keep)) {
      current = shorter;
      kept += 1;
    }
  }
  return { current, kept };
}

async function reduce(pages, html, keep) {
  let current = html;
  let cuts = 0;
  for (let pass = 1; pass <= 4; pass += 1) {
    await pages.chromium.setContent(current);
    const paths = await pages.chromium.evaluate(PATHS);
    const elements = await sweep(pages, current, keep, paths, CUT);

    // Rules from the back, so dropping one never renumbers a rule not yet tried.
    await pages.chromium.setContent(elements.current);
    const count = await pages.chromium.evaluate(RULE_COUNT);
    const indexes = [...Array(count).keys()].reverse();
    const rules = await sweep(pages, elements.current, keep, indexes, CUT_RULE);

    current = rules.current;
    const kept = elements.kept + rules.kept;
    cuts += kept;
    console.log(`pass ${pass}: ${kept} cuts, ${current.length} bytes left`);
    if (kept === 0) break;
  }
  return { html: current, cuts };
}

const server = serve();
await listening();
const toy = await chromium.connectOverCDP(`ws://127.0.0.1:${PORT}/`);
const real = await chromium.launch();
const pages = {
  toy: await toy.contexts()[0].newPage(),
  chromium: await real.newPage(),
};
await Promise.all(Object.values(pages).map((p) => p.setViewportSize(VIEWPORT)));
await mkdir(INTO, { recursive: true });
await mkdir(COMPARE, { recursive: true });

await pages.chromium.goto(target);
// Through the parser once before anything is measured: the reference's own
// serialization is what every candidate will be compared against, and a
// hand-built string is never byte-identical to it.
await pages.chromium.setContent(await pages.chromium.evaluate(FREEZE));
const frozen = await pages.chromium.evaluate(
  () => document.documentElement.outerHTML,
);
const whole = await score(pages, frozen);
console.log(
  `frozen: score ${whole.score.toFixed(4)}, ${frozen.length} bytes, blamed on "${whole.cause}"`,
);

const { html, cuts } = await reduce(pages, frozen, whole);
const smallest = await score(pages, html);
await writeFile(resolve(INTO, "minimal.html"), html);

console.log(
  `\nreduced ${frozen.length} -> ${html.length} bytes in ${cuts} cuts`,
);
console.log(`score ${whole.score.toFixed(4)} -> ${smallest.score.toFixed(4)}`);
console.log(`still blamed on "${smallest.cause}"`);
console.log(`worst: ${JSON.stringify(smallest.worst)}`);
console.log(`\nminimal repro: ${resolve(INTO, "minimal.html")}`);

await toy.close();
await real.close();
server.kill();
