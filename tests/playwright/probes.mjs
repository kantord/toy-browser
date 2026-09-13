// Every probe, against real Chromium, ranked by how wrong it is.
//
// `tests/probes/` has one page per CSS feature and the README says how to run
// *one* of them: start two servers, set an environment variable, run a spec,
// then run the comparator. That is four commands for one answer, which is why
// the answer was only ever taken one feature at a time — and why the question
// this directory exists to answer, *which feature is worst today*, had to be
// remembered rather than measured.
//
// This asks all of them in one pass. Both browsers are launched once and every
// probe is loaded in them, because launching Chromium is most of the cost and
// twenty-seven launches is four minutes of it.
//
// Self-running, like `diverge.mjs` and `oracle.mjs`: `node probes.mjs`, or
// `just probes`. It is not a test — nothing here passes or fails. It is the
// instrument the paint work is aimed with.

import { execFile } from "node:child_process";
import { mkdir, readdir, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { chromium } from "playwright-core";

import { EXPORT } from "./export.mjs";
import { serve } from "./serve.mjs";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const PROBES = resolve(REPO, "tests/probes");
const INTO = resolve(REPO, "out/probes");
const BINARY = resolve(REPO, "target/debug/toy-browser");
const VIEWPORT = { width: 1000, height: 800 };
const run = promisify(execFile);

/** The probe pages are served rather than opened as files: several of them ask
 *  for an image beside themselves, and a `file://` origin answers differently
 *  from an `http://` one about what it may fetch. */
function files(port) {
  const types = { ".html": "text/html", ".png": "image/png", ".svg": "image/svg+xml" };
  const server = createServer(async (request, response) => {
    const name = decodeURIComponent(request.url.split("?")[0]).replace(/^\//, "");
    try {
      const body = await readFile(resolve(PROBES, name));
      response.writeHead(200, { "content-type": types[extname(name)] ?? "text/plain" });
      response.end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  return new Promise((ok) => server.listen(port, "127.0.0.1", () => ok(server)));
}

/** One page, from one browser, as a screenshot and an account of its elements. */
async function capture(page, url, into, engine) {
  await page.goto(url, { waitUntil: "load" });
  await writeFile(resolve(into, `${engine}.png`), await page.screenshot());
  const exported = await page.evaluate(EXPORT);
  await writeFile(resolve(into, `${engine}.json`), JSON.stringify(exported));
}

/** What `toy-browser compare` makes of one probe. */
async function scored(into) {
  const { stdout } = await run(BINARY, ["compare", "--dir", into, "--json"]);
  return JSON.parse(stdout);
}

async function main() {
  const port = 8911;
  const site = await files(port);
  const ours = await serve(9225);
  const toy = await chromium.connectOverCDP(ours.url);
  const real = await chromium.launch();
  const pages = {
    toy: await toy.contexts()[0].newPage(),
    chromium: await real.newPage(),
  };
  await Promise.all(Object.values(pages).map((p) => p.setViewportSize(VIEWPORT)));

  const names = (await readdir(PROBES)).filter((n) => n.endsWith(".html")).sort();
  const found = [];
  for (const name of names) {
    const probe = name.replace(".html", "");
    const into = resolve(INTO, probe);
    await mkdir(into, { recursive: true });
    const url = `http://127.0.0.1:${port}/${name}`;
    for (const [engine, page] of Object.entries(pages)) {
      await capture(page, url, into, engine);
    }
    found.push({ probe, ...(await scored(into)) });
  }

  await toy.close();
  await real.close();
  ours.stop();
  site.close();

  report(found);
}

/** Worst first, because the question is which one to fix. */
function report(found) {
  found.sort((a, b) => b.score - a.score);
  console.log("probe            score   badly  ink  why");
  for (const one of found) {
    console.log(
      `${one.probe.padEnd(16)} ${one.score.toFixed(4)}  ${(one.badly * 100).toFixed(2)}%  ` +
        `${String(one.painted_differently).padStart(3)}  ${one.cause}` +
        (one.worst ? `  (${one.worst.what}: ${one.worst.why})` : ""),
    );
  }
  console.log(`\nartefacts in out/probes/<probe>/, ${found.length} probes`);
}

await main();
