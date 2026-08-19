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
test("web-first assertions poll via the injected script", async () => {
  await page.goto(fixture("hello.html"));
  await expect(page).toHaveTitle("Hello");
  await expect(page.locator("p")).toHaveCount(2);
});

test("rejects a scheme it cannot load", async () => {
  // `ftp`, not `https`: this browser speaks http now. Deliberately a scheme
  // nothing will ever look up, so the test cannot reach a network.
  await expect(page.goto("ftp://example.invalid/")).rejects.toThrow(
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

test("web-first assertions", async () => {
  await page.goto(fixture("hello.html"));
  await expect(page).toHaveTitle("Hello");
  await expect(page.locator("p")).toHaveCount(2);
  await expect(page.locator("h1")).toBeVisible();
  await expect(page.locator("h1")).toHaveText("Hello, toy browser");
  await expect(page.locator("h1")).toContainText("toy");
});

test("locators find and read elements", async () => {
  await page.goto(fixture("hello.html"));
  expect(await page.textContent("h1")).toBe("Hello, toy browser");
  expect(await page.getByText("toy browser").count()).toBeGreaterThan(0);
  await expect(page.locator("p.muted")).toHaveAttribute("class", "muted");
});

/** The middle of an element, which is where a click aimed at it would land. */
const centre = (selector) =>
  page.evaluate((css) => {
    const box = document.querySelector(css).getBoundingClientRect();
    return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  }, selector);

test("clicking runs the page's handlers", async () => {
  await page.goto(fixture("click.html"));
  const { x, y } = await centre("#tap");
  await page.mouse.click(x, y);

  // The inline `onclick` wrote this, so the whole path ran: hit test, the
  // event travelling to a listener, then the element's own behaviour.
  expect(await page.textContent("#log")).toBe("inline ran");
});

test("clicking a link navigates", async () => {
  await page.goto(fixture("activate.html"));
  const { x, y } = await centre("#label");
  await page.mouse.click(x, y);

  // The client learns about a document it never asked for from the events the
  // press emitted afterwards.
  expect(page.url()).toContain("hello.html");
  expect(await page.textContent("h1")).toBe("Hello, toy browser");
});

// `page.mouse` works; `locator.click()` does not. Playwright checks a target is
// actionable first, and that check awaits a promise from its injected script —
// which comes back as `{}` because `Runtime.evaluate` ignores `awaitPromise`.
// `innerText` needs layout-aware text this browser cannot compute at all.
// See docs/cdp-surface.md.
test.fixme("locator actions and innerText", async () => {
  await page.locator("h1").innerText();
  await page.locator("h1").click();
});
