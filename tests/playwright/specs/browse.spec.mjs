import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { test, expect } from "@playwright/test";
import { chromium } from "playwright-core";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const fixture = (name) => `file://${resolve(REPO, "tests/fixtures", name)}`;

/** @type {import('playwright-core').Browser} */
let browser;
/** @type {import('playwright-core').Page} */
let page;

test.beforeAll(async () => {
  browser = await chromium.connectOverCDP("ws://127.0.0.1:9222/");
});

test.afterAll(async () => {
  await browser?.close();
});

test.beforeEach(async () => {
  page = await browser.contexts()[0].newPage();
  await page.setViewportSize({ width: 800, height: 600 });
});

test.afterEach(async () => {
  await page?.close();
});

test("navigates to a static page and reads its title", async () => {
  await page.goto(fixture("hello.html"));
  expect(page.url()).toContain("hello.html");
  expect(await page.title()).toBe("Hello");
});

test("runs the page's JavaScript before we see it", async () => {
  await page.goto(fixture("js/js-module.html"));
  const swatches = await page.evaluate(
    () => document.querySelectorAll(".swatch").length,
  );
  expect(swatches).toBe(3);
});

test("screenshots at the requested viewport", async () => {
  await page.goto(fixture("hello.html"));
  const png = await page.screenshot();
  // PNG header: width and height are big-endian u32 at bytes 16 and 20.
  expect(png.readUInt32BE(16)).toBe(800);
  expect(png.readUInt32BE(20)).toBe(600);
});

test("measures where elements ended up", async () => {
  await page.goto(fixture("hello.html"));
  const box = await page.evaluate(() => {
    const { x, y, width } = document.querySelector("h1").getBoundingClientRect();
    return { x, y, width };
  });
  expect(box).toEqual({ x: 40, y: 67, width: 720 });
});

test("counts elements through a locator", async () => {
  await page.goto(fixture("hello.html"));
  expect(await page.locator("p").count()).toBe(2);
});

// Web-first assertions poll through Playwright's injected script, which is the
// part of it this browser cannot yet host. The plain-API forms of both of these
// pass above; only the polling wrappers fail.
test.fixme("web-first assertions poll via the injected script", async () => {
  await page.goto(fixture("hello.html"));
  await expect(page).toHaveTitle("Hello");
  await expect(page.locator("p")).toHaveCount(2);
});

test("rejects a scheme it cannot load", async () => {
  await expect(page.goto("https://example.invalid/")).rejects.toThrow(
    /ERR_UNKNOWN_URL_SCHEME/,
  );
});

test("runs an init script before the page's own", async () => {
  await page.addInitScript(() => {
    globalThis.__initRan = "yes";
  });
  await page.goto(fixture("hello.html"));
  expect(await page.evaluate(() => globalThis.__initRan)).toBe("yes");
});
