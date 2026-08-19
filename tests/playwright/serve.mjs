// A browser of one's own, for the length of one run.
//
// The Playwright suite reuses whatever is already listening, which is right for
// a test run and wrong for anything that edits the pages it then loads: reads
// are cached for the life of a server, so a reused one answers with the file as
// it was when some earlier run first asked. Chromium re-reads, this does not,
// and the two quietly stop describing the same document.

import { spawn } from "node:child_process";
import { connect } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const BINARY = resolve(REPO, "target/debug/toy-browser");

const listening = (port) =>
  new Promise((ok) => {
    const attempt = () => {
      const socket = connect(port, "127.0.0.1")
        .on("connect", () => socket.end(ok))
        .on("error", () => setTimeout(attempt, 100));
    };
    attempt();
  });

/** Starts a browser on `port` and hands back the URL and a way to stop it. */
export async function serve(port) {
  const server = spawn(BINARY, ["serve", "--port", String(port)], { stdio: "ignore" });
  process.on("exit", () => server.kill());
  await listening(port);
  return { url: `ws://127.0.0.1:${port}/`, stop: () => server.kill() };
}
