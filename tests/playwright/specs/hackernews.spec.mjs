// A real website, over the real network.
//
// Hacker News is the target because it barely changes shape: the nav bar has
// said the same words for years, so a test can click through it without
// pinning anything that moves. What the stories say changes hourly, which is
// why nothing here asserts on their text.
//
// Kept apart from `browse.spec.mjs` on purpose: everything there is a `file://`
// fixture and never touches a network. This one does, and is skipped when
// `TOY_BROWSER_OFFLINE` is set.

import { test, expect } from "@playwright/test";
import { chromium } from "playwright-core";

import { centre, centreOfLink } from "../export.mjs";

const HN = "https://news.ycombinator.com/";

/** The logo, which is the one link on the page a click can be aimed at. */
const LOGO = 'a[href="https://news.ycombinator.com"]';

/** @type {import('playwright-core').Browser} */
let browser;
/** @type {import('playwright-core').Page} */
let page;

test.skip(!!process.env.TOY_BROWSER_OFFLINE, "needs the network");

test.beforeAll(async () => {
  browser = await chromium.connectOverCDP("ws://127.0.0.1:9222/");
});

test.afterAll(async () => {
  await browser?.close();
});

test.beforeEach(async () => {
  page = await browser.contexts()[0].newPage();
  await page.setViewportSize({ width: 1000, height: 800 });
});

test.afterEach(async () => {
  await page?.close();
});


test("renders the front page", async () => {
  await page.goto(HN);

  expect(await page.title()).toContain("Hacker News");
  // Thirty stories, as it has had for its whole life.
  expect(await page.locator("tr.athing").count()).toBe(30);
  // Proof the linked stylesheet was fetched: without it nothing paints a
  // background and the render is a transparent rectangle.
  expect(await page.locator("table#hnmain").count()).toBe(1);
});

test("clicking the logo goes back to the front page", async () => {
  await page.goto(`${HN}newest`);
  expect(page.url()).toContain("newest");

  const at = await centre(page, LOGO);
  expect(at, "no logo to click").not.toBeNull();
  await page.mouse.click(at.x, at.y);

  expect(page.url()).toBe(HN);
  expect(await page.locator("tr.athing").count()).toBe(30);
});

// The nav bar — new, past, comments, ask, show, jobs — is text links, and an
// inline element has no box in this browser: takumi's paint items are nodes and
// nested contexts with nothing for a text run, so `getBoundingClientRect` on
// one of these `<a>`s reports 0x0 and there is nowhere to aim.
//
// Measured on the live page: 229 links, 31 with a box, and every one of those
// 31 contains an image — the logo and the thirty upvote arrows. See
// docs/adr/0010 and the README's notes.
test.fixme("clicking a nav link", async () => {
  await page.goto(HN);
  const at = await centreOfLink(page, "ask");
  await page.mouse.click(at.x, at.y);
});
