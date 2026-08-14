// Starts toy-browser for the duration of a test run.
//
// Playwright's own `webServer` option probes with an HTTP GET, and our port only
// speaks WebSocket, so it can never see the server come up. A plain TCP connect
// is the right readiness check here.

import { spawn } from "node:child_process";
import { createConnection } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const PORT = Number(process.env.TOY_BROWSER_PORT ?? 9222);

const reachable = (port) =>
  new Promise((done) => {
    const socket = createConnection({ port, host: "127.0.0.1" })
      .on("connect", () => (socket.end(), done(true)))
      .on("error", () => done(false));
  });

export default async function globalSetup() {
  if (await reachable(PORT)) {
    return () => {};
  }

  const server = spawn(
    "cargo",
    ["run", "--quiet", "--", "serve", "--port", String(PORT)],
    { cwd: REPO, stdio: ["ignore", "inherit", "inherit"] },
  );

  const deadline = Date.now() + 180_000;
  while (!(await reachable(PORT))) {
    if (Date.now() > deadline) {
      server.kill("SIGTERM");
      throw new Error(`toy-browser did not start on ${PORT}`);
    }
    await new Promise((done) => setTimeout(done, 250));
  }

  return () => server.kill("SIGTERM");
}
