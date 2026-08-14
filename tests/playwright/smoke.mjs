// Drives toy-browser through Playwright's Chromium CDP client.
//
// Starts `toy-browser serve`, connects to it, opens a page, navigates to a
// fixture and screenshots it. Nothing here knows that the browser on the other
// end is a toy.

import { spawn } from "node:child_process";
import { once } from "node:events";
import { createConnection } from "node:net";
import { mkdirSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright-core";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "../..");
const PORT = Number(process.env.PORT ?? 9222);
const VIEWPORT = { width: 800, height: 600 };

// Navigated in order on one page, so this also covers re-navigation.
const PAGES = [
  { fixture: "tests/fixtures/hello.html", output: "out/pw-hello.png" },
  { fixture: "tests/fixtures/js/js-module.html", output: "out/pw-js-module.png" },
];

async function waitForPort(port, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const reachable = await new Promise((done) => {
      const socket = createConnection({ port, host: "127.0.0.1" })
        .on("connect", () => (socket.end(), done(true)))
        .on("error", () => done(false));
    });
    if (reachable) return;
    await new Promise((done) => setTimeout(done, 200));
  }
  throw new Error(`nothing listening on ${port} after ${timeoutMs}ms`);
}

const server = spawn(
  "cargo",
  ["run", "--quiet", "--", "serve", "--port", String(PORT)],
  { cwd: REPO, stdio: ["ignore", "inherit", "inherit"] },
);
server.on("exit", (code) => {
  if (code !== null && code !== 0) {
    console.error(`server exited with ${code}`);
    process.exit(1);
  }
});

try {
  await waitForPort(PORT);

  const browser = await chromium.connectOverCDP(`ws://127.0.0.1:${PORT}/`);
  const context = browser.contexts()[0];
  const page = await context.newPage();

  // Required: connectOverCDP contexts have no default viewport, so without this
  // Playwright tries to read window.innerWidth from a page it cannot evaluate in.
  await page.setViewportSize(VIEWPORT);

  for (const { fixture, output } of PAGES) {
    const path = resolve(REPO, output);
    mkdirSync(dirname(path), { recursive: true });

    await page.goto(`file://${resolve(REPO, fixture)}`);
    await page.screenshot({ path });

    const title = await page.title();
    const html = await page.content();
    console.log(
      `${fixture} -> ${output} (${statSync(path).size} bytes)` +
        ` title=${JSON.stringify(title)} html=${html.length}b`,
    );
  }

  // Back to a known page for the evaluation checks.
  await page.goto(`file://${resolve(REPO, PAGES[0].fixture)}`);

  const checks = [
    ["evaluate expression", () => page.evaluate("1 + 1"), 2],
    ["evaluate function", () => page.evaluate(() => document.title), "Hello"],
    ["evaluate with argument", () => page.evaluate((n) => n * 3, 7), 21],
    ["locator count", () => page.locator("p").count(), 2],
    ["locator visibility", () => page.locator("h1").isVisible(), true],
  ];

  // Geometry comes from a real layout pass, so the heading must sit inside the
  // body's 32px padding and span the rest of the 800px viewport.
  const rect = await page.evaluate(() => {
    const { x, y, width } = document.querySelectorAll("h1")[0].getBoundingClientRect();
    return { x, y, width };
  });
  if (rect.width <= 0 || rect.y <= 0 || rect.x !== 40) {
    throw new Error(`heading geometry looks wrong: ${JSON.stringify(rect)}`);
  }
  checks.push(["heading measured", () => Promise.resolve(rect.width), 720]);

  for (const [name, run, expected] of checks) {
    const actual = await run();
    if (actual !== expected) {
      throw new Error(`${name}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
    }
    console.log(`${name} -> ${JSON.stringify(actual)}`);
  }

  // A scheme we do not implement must fail loudly rather than hang.
  await page
    .goto("https://example.invalid/")
    .then(() => {
      throw new Error("expected https:// to be rejected");
    })
    .catch((error) => {
      if (!error.message.includes("ERR_UNKNOWN_URL_SCHEME")) throw error;
      console.log("https:// rejected as expected");
    });

  await page.close();
  await browser.close();
  console.log("OK");
} finally {
  server.kill("SIGTERM");
  await once(server, "exit").catch(() => {});
}
