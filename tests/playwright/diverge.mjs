// The same page in both browsers, traced identically, diffed at the first
// place they stop agreeing.
//
//   just diverge <url>            (or: node tests/playwright/diverge.mjs <url>)
//
// Reads this browser's trace out of `out/<page>.dom.html`, where
// `render --init-script tests/trace/tracer.js` left it, and takes Chromium's
// from a live run. Prints what each did at the point they parted.
//
// Written because reading one trace tells you what a page did and reading two
// tells you what this browser did *wrong*, which is a different and much
// shorter question.
//
// It lives here rather than beside the tracer it loads because this half needs
// Playwright, and node resolves that from the file asking for it.

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const url = process.argv[2];
if (!url) {
  console.error("usage: node tests/playwright/diverge.mjs <url> [--context N]");
  process.exit(2);
}
const flag = process.argv.indexOf("--context");
const context = flag > 0 ? Number(process.argv[flag + 1]) : 8;

// The tracer both browsers run, with stacks turned on when asked for. Stacks
// answer a different question — not *what* was done but which function was
// doing it — and that is what names the two branches of an `if` the browsers
// disagreed about. They are off by default because they treble the size of a
// trace.
const beside = (name) => new URL("../trace/" + name, import.meta.url);
const settings =
  (process.env.TRACE_STACKS ? "globalThis.__trace_stacks = true;\n" : "") +
  (process.env.TRACE_VALUES ? "globalThis.__trace_values = true;\n" : "");
const tracer = settings + ["record.js", "tracer.js"].map((it) => readFileSync(beside(it), "utf8")).join("\n");

/** This browser, rendering the page with the tracer in it. */
const render = () => {
  // One file, because the two halves and the settings above have to arrive in
  // that order and this is the order they are in.
  const script = join(mkdtempSync(join(tmpdir(), "diverge-")), "tracer.js");
  writeFileSync(script, tracer);
  execFileSync("cargo", ["run", "--quiet", "--", "render", "--init-script", script, url], {
    cwd: new URL("../../", import.meta.url).pathname,
    stdio: "inherit",
  });
};

/** The trace this browser left in the document it rendered. */
const ours = (page) => {
  const html = readFileSync(page, "utf8");
  const found = /data-trace="(.*?)"[ >]/s.exec(html);
  if (!found) throw new Error(`no trace in ${page} — was --init-script passed?`);
  const written = found[1].replaceAll("&amp;", "&").replaceAll("&lt;", "<");
  // A page that ran no script of its own leaves the attribute empty, which is
  // a trace of nothing rather than a missing trace.
  return written ? written.split(" ~ ") : [];
};

/** The same page in a real browser, with the same tracer. */
const theirs = async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  await page.addInitScript(tracer);
  try {
    await page.goto(url, { waitUntil: "networkidle", timeout: 45000 });
  } catch {
    // A page that never settles has still done everything it is going to do.
  }
  const trace = await page.evaluate(`JSON.stringify(globalThis.__trace || [])`);
  await browser.close();
  return JSON.parse(trace);
};

// Our side travels through an HTML attribute, which cannot carry a quote, so
// both sides are read with the same punctuation before being compared.
const alike = (line) => line.replaceAll('"', "'");

// Aligned rather than compared line for line. The two browsers do the same work
// in slightly different orders — a preload polyfill runs a moment earlier here,
// a module a moment later there — and a strict comparison parts them on the
// first such shuffle and hides everything after it.
//
// So: walk both, and when they differ look ahead a little way in each for the
// other's current line. Finding it means the other browser did something extra,
// which is skipped over and counted. Finding it in neither is the real answer:
// the place where this browser did something different, rather than something
// late.
const REACH = 200;

const align = (mine, theirs) => {
  const extra = { ours: 0, theirs: 0 };
  let a = 0;
  let b = 0;
  while (a < mine.length && b < theirs.length) {
    if (alike(mine[a]) === alike(theirs[b])) {
      a += 1;
      b += 1;
      continue;
    }
    const ahead = (seq, from, want) => {
      for (let at = from; at < Math.min(seq.length, from + REACH); at += 1) {
        if (alike(seq[at]) === want) return at;
      }
      return -1;
    };
    const inTheirs = ahead(theirs, b, alike(mine[a]));
    const inOurs = ahead(mine, a, alike(theirs[b]));
    // Whichever gap is shorter is the one that is merely extra work.
    if (inTheirs >= 0 && (inOurs < 0 || inTheirs - b <= inOurs - a)) {
      extra.theirs += inTheirs - b;
      b = inTheirs;
      continue;
    }
    if (inOurs >= 0) {
      extra.ours += inOurs - a;
      a = inOurs;
      continue;
    }
    return { at: a, theirsAt: b, extra, parted: true };
  }
  return { at: a, theirsAt: b, extra, parted: false };
};

const name = new URL(url).hostname.split(".").at(-2) ?? "page";
const dom =
  process.env.TOY_BROWSER_DOM ??
  new URL(`../../out/${name}.dom.html`, import.meta.url).pathname;
if (!process.env.TOY_BROWSER_DOM) render();
const mine = ours(dom);
const other = await theirs();

// Both sides are written out whole, because the printed window around the
// divergence is never quite the window you want next.
const written = (name) => new URL("../../out/" + name, import.meta.url);
writeFileSync(written("diverge-ours.txt"), mine.join("\n"));
writeFileSync(written("diverge-chromium.txt"), other.join("\n"));

const { at, theirsAt, extra, parted } = align(mine, other);
console.log(`ours ${mine.length} calls, chromium ${other.length}`);
console.log(
  parted
    ? `aligned to our #${at} / their #${theirsAt} (skipped ${extra.ours} ours, ${extra.theirs} theirs), then they part:\n`
    : `aligned to the end of ${at === mine.length ? "ours" : "chromium"} — no divergence in what was traced\n`,
);
const show = (label, seq, from) => {
  console.log(`--- ${label}`);
  for (const line of seq.slice(from, from + context)) console.log("   ", line.slice(0, Number(process.env.WIDTH ?? 110)));
};
show("ours", mine, at);
console.log();
show("chromium", other, theirsAt);
console.log("\nboth traces in full: out/diverge-ours.txt, out/diverge-chromium.txt");
