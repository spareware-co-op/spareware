// Times every benchmark in one browser for the reference runner's night (piece G107; chapter
// 23.13 of the kernel architecture, "Wall clock and scaling"). Run by nightly.sh with Node on the
// reference machine's Windows, pinned to the performance cores (decision 32.16): the browser it
// launches inherits the pin.
//
// Usage: node browsers.mjs <engine> <round ms> <deadline ms since 1970> <cases file> <jobs file>
//   engine: chrome (the machine's installed Chrome, Playwright's "chrome" channel), firefox or
//   webkit (Playwright's builds; WebKit is Safari's engine, and Safari itself runs only on macOS).
//
// The folder this file is in holds guest.wasm (geom-bench-guest built for wasm32-wasip1),
// page.html, page.mjs, worker.mjs, the cases file, node_modules with Playwright and
// @bjorn3/browser_wasi_shim at the versions nightly.sh pins, and the browsers Playwright installed
// (browsers/). It serves that folder on 127.0.0.1 with the headers that isolate the page
// (Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy), so performance.now() is as fine
// as each browser allows: 5 microseconds in Chrome, 20 in Firefox and WebKit, against 100 or 1,000
// without isolation (W3C High Resolution Time, https://w3c.github.io/hr-time/, "coarsen time").
//
// The page times each benchmark at 1 thread and then at 8 (page.mjs), and every job it finishes
// is appended at once to the jobs file as the runner's job line, so a night that stops at its
// deadline keeps every job it finished. The jobs file's first line is
// `# version <the browser's version>, Playwright <version>, isolated|not isolated`, or
// `# unavailable <why>` when the browser could not run. At the deadline the browser is closed and
// the program exits, whatever is running.
//
// Playwright (Apache-2.0) and @bjorn3/browser_wasi_shim (MIT OR Apache-2.0); the variant
// workflow's browser driver (variants/browsers.mjs, piece G42) is the pattern followed.

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [engine, round, deadlineText, casesFile, jobsFile] = process.argv.slice(2);
const deadline = Number(deadlineText);
const root = path.dirname(fileURLToPath(import.meta.url));
process.env.PLAYWRIGHT_BROWSERS_PATH = path.join(root, "browsers");

if (!["chrome", "firefox", "webkit"].includes(engine) || !(deadline > 0) || !jobsFile) {
  console.error("usage: node browsers.mjs <chrome|firefox|webkit> <round ms> <deadline ms> <cases file> <jobs file>");
  process.exit(2);
}

const types = {
  ".html": "text/html; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".txt": "text/plain; charset=utf-8",
};

// Serves the folder, and nothing outside it, with the isolating headers.
const server = http.createServer((request, response) => {
  const url = new URL(request.url, "http://127.0.0.1");
  const file = path.normalize(path.join(root, decodeURIComponent(url.pathname)));
  if (!file.startsWith(root + path.sep) || request.method !== "GET") {
    response.writeHead(404).end();
    return;
  }
  let body;
  try {
    body = readFileSync(file);
  } catch {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, {
    "Content-Type": types[path.extname(file)] ?? "application/octet-stream",
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "require-corp",
    "Cross-Origin-Resource-Policy": "same-origin",
    "Cache-Control": "no-store",
  });
  response.end(body);
});

let browser = null;
// Stop at the deadline, whatever is running: every job finished is already in the jobs file.
const stop = setTimeout(async () => {
  console.error(`${engine}: the deadline came; stopping`);
  try {
    await browser?.close();
  } finally {
    process.exit(0);
  }
}, Math.max(0, deadline - Date.now()));

try {
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  const playwright = await import("playwright");
  const { version: playwrightVersion } = JSON.parse(
    readFileSync(path.join(root, "node_modules", "playwright", "package.json"), "utf8"),
  );
  const launchers = {
    chrome: () => playwright.chromium.launch({ channel: "chrome" }),
    firefox: () => playwright.firefox.launch(),
    webkit: () => playwright.webkit.launch(),
  };
  try {
    browser = await launchers[engine]();
  } catch (error) {
    writeFileSync(jobsFile, `# unavailable ${String(error).split("\n")[0]}\n`);
    throw error;
  }
  const page = await browser.newPage();
  page.on("pageerror", (error) => console.error(`${engine}: ${error}`));
  let header = null;
  await page.exposeFunction("referenceStarted", (isolated) => {
    header = `# version ${browser.version()}, Playwright ${playwrightVersion}, ${isolated ? "isolated" : "not isolated"}`;
    writeFileSync(jobsFile, header + "\n");
  });
  await page.exposeFunction("referenceJob", (line) => appendFileSync(jobsFile, `job ${engine} ${line}\n`));
  const finished = new Promise((resolve) => page.exposeFunction("referenceDone", resolve));
  const query = new URLSearchParams({ round, deadline: String(deadline), cases: path.basename(casesFile) });
  await page.goto(`http://127.0.0.1:${port}/page.html?${query}`);
  const summary = await finished;
  console.log(`${engine} ${browser.version()}: ${summary}`);
  await browser.close();
} catch (error) {
  console.error(`${engine}: ${error}`);
  process.exitCode = 1;
} finally {
  clearTimeout(stop);
  server.close();
}
