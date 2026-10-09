#!/usr/bin/env node
// toy-browser-host's protocol, with the pages fetched and parsed by happy-dom.
//
//   host.mjs  <- the client (the Neovim plugin, or anything that speaks the
//                ndjson protocol in crates/host/src/main.rs)
//     |
//   happy-dom   fetches a `navigate`, builds the DOM, and (opt-in) runs the
//     |         page's scripts
//   toy-browser-host   draws the resulting document as cells
//
// A `navigate` op is answered by loading the page in happy-dom, taking its
// document as it stands, and sending that on as `markup`. Every other op, and
// every reply, passes through untouched. The document sent on has no scripts:
// whatever they were going to do, happy-dom has already done.
//
// Scripts are off unless TOY_HAPPY_DOM_SCRIPTS=1: it runs the web's JavaScript
// inside this process. Without it happy-dom is the fetcher and parser, and
// that is all.
//
// Clicks: every element in the snapshot is stamped `data-key="hd:N"`, so the
// engine's `clicked` event names the happy-dom element under the pointer. A
// click that is not on a link is raised on that element here, the page's
// handlers run in happy-dom, and the document as it then stands is sent on again.
// (A link is the client's: it navigates.)
//
// Forms: the element the engine says has focus (a click on it, or a `focus` op
// stepping along the tab order) is remembered by its key, and `type` and `key`
// ops go to that element here instead of to the engine's copy: the text is put
// in the happy-dom field, its input events run, and the document is sent again.

import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { Browser } from "happy-dom";

const here = dirname(fileURLToPath(import.meta.url));
const engine = process.env.TOY_BROWSER_HOST_BIN ?? resolve(here, "../../target/debug/toy-browser-host");
const scripts = process.env.TOY_HAPPY_DOM_SCRIPTS === "1";

const child = spawn(engine, [], { stdio: ["pipe", "pipe", "inherit"] });
child.on("exit", (code) => process.exit(code ?? 0));
const send = (command) => child.stdin.write(JSON.stringify(command) + "\n");
const say = (event) => process.stdout.write(JSON.stringify(event) + "\n");

// The engine's output is ours too, line for line; a `clicked` is also acted on.

const browser = new Browser({
  settings: { enableJavaScriptEvaluation: scripts, disableErrorCapturing: false },
});
const pages = new Map();
let busy = Promise.resolve();

/** The page for a client's page number, made the first time it is asked for. */
function pageFor(id) {
  if (!pages.has(id)) pages.set(id, browser.newPage());
  return pages.get(id);
}

/** The element a client's typing goes to, by page: the key the engine last said had focus. */
const focus = new Map();

const FIELDS = "input, textarea, select, button";

/** The page's document as it now stands, sent to the engine to draw. */
function snapshot(id) {
  const page = pageFor(id);
  const document = page.mainFrame.document;
  for (const script of document.querySelectorAll("script")) script.remove();
  // What a field holds is a property, not markup: write it where the engine reads.
  for (const field of document.querySelectorAll("input, textarea")) {
    if (field.tagName === "TEXTAREA") field.textContent = field.value;
    else if (field.type === "checkbox" || field.type === "radio") field.toggleAttribute("checked", field.checked);
    else field.setAttribute("value", field.value);
  }
  let n = 0;
  for (const element of document.querySelectorAll("*")) element.setAttribute("data-key", `hd:${n++}`);
  send({
    op: "markup",
    page: id,
    html: document.documentElement.outerHTML,
    base: page.mainFrame.url,
  });
  // The new document has nothing focused: put it back where it was.
  if (focus.has(id)) send({ op: "focus", page: id, key: focus.get(id), silent: true });
}

/** The element the page's focus is on, in happy-dom. */
function focused(id) {
  const key = focus.get(id);
  return key ? pages.get(id)?.mainFrame.document.querySelector(`[data-key="${key}"]`) : null;
}

/** Typing, or a key, into the focused field; then the document is sent again. */
async function typed(id, command) {
  const element = focused(id);
  const page = pages.get(id);
  if (!element || !page) return;
  const { InputEvent, KeyboardEvent } = page.mainFrame.window;
  const field = element.matches("input, textarea");
  if (command.op === "type" && field) {
    element.value += command.text;
    element.dispatchEvent(new InputEvent("input", { bubbles: true, data: command.text, inputType: "insertText" }));
  } else if (command.key === "Backspace" && field) {
    element.value = element.value.slice(0, -1);
    element.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "deleteContentBackward" }));
  } else if (command.key === "Enter" || command.key === " ") {
    element.dispatchEvent(new KeyboardEvent("keydown", { key: command.key, bubbles: true, cancelable: true }));
    if (element.matches("button")) element.click();
  }
  await page.waitUntilComplete();
  snapshot(id);
}

async function navigate(id, url) {
  const page = pageFor(id);
  focus.delete(id);
  await page.goto(url);
  await page.waitUntilComplete();
  snapshot(id);
}

/** A click the engine saw on an element of the snapshot, raised on the real one. */
async function clicked(id, key) {
  const page = pages.get(id);
  const element = page?.mainFrame.document.querySelector(`[data-key="${key}"]`);
  if (!element) return;
  const { MouseEvent } = page.mainFrame.window;
  element.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  if (element.matches(FIELDS)) {
    element.focus();
    focus.set(id, key);
  } else {
    focus.delete(id);
  }
  await page.waitUntilComplete();
  snapshot(id);
}

createInterface({ input: child.stdout }).on("line", (line) => {
  process.stdout.write(line + "\n");
  let event;
  try {
    event = JSON.parse(line);
  } catch {
    return;
  }
  if (event.ev === "clicked" && event.key?.startsWith("hd:") && !event.href) {
    busy = busy.then(() => clicked(event.page ?? 0, event.key));
  } else if (event.ev === "focused" && event.key?.startsWith("hd:")) {
    // The engine stepped focus along the tab order: happy-dom follows.
    const id = event.page ?? 0;
    focus.set(id, event.key);
    focused(id)?.focus();
  }
});

const lines = createInterface({ input: process.stdin });
lines.on("line", (line) => {
  // One at a time and in order: a navigate takes a while and what follows it
  // (a resize, a click) means nothing until it has landed.
  busy = busy.then(async () => {
    let command;
    try {
      command = JSON.parse(line);
    } catch {
      return send({ op: "invalid", raw: line });
    }
    const id = command.page ?? 0;
    if (command.op === "navigate") {
      try {
        await navigate(id, command.url);
      } catch (error) {
        say({ ev: "error", op: "navigate", page: id, message: String(error?.message ?? error) });
      }
    } else if (command.op === "close") {
      await pages.get(id)?.close();
      pages.delete(id);
      focus.delete(id);
      send(command);
    } else if ((command.op === "type" || command.op === "key") && focused(id)) {
      await typed(id, command);
    } else {
      send(command);
    }
  });
});
lines.on("close", async () => {
  await busy;
  await browser.close();
  child.stdin.end();
});
