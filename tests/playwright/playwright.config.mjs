import { defineConfig } from "@playwright/test";

// The browser under test is toy-browser, started by global setup and reached
// over CDP. Playwright still needs its own Chromium for the GUI shells (UI
// mode, the trace viewer, the HTML report) — those are separate browsers and
// have nothing to do with the one under test.
export default defineConfig({
  testDir: "./specs",
  reporter: [["list"], ["html", { open: "never" }]],
  // One page at a time: the browser is single-threaded and serves one
  // connection.
  workers: 1,
  globalSetup: "./global-setup.mjs",
  use: {
    // Always record, so there is something to open in the trace viewer even
    // when everything passed. The action timeline is complete; the film strip
    // and DOM snapshot panes stay empty — see docs/cdp-surface.md.
    trace: "on",
    // Screenshots are `page.screenshot()`, which this browser does implement —
    // unlike the trace's film strip, which needs a screencast it does not.
    screenshot: "on",
  },
});
