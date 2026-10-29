// Drives page.html in a browser and writes what it measured (piece G42; chapter 23.3).
//
// Usage: node browsers.mjs <engine> <build> <ms> <url-base> <out.json>
//   engine: chromium, firefox or webkit (Playwright's builds, Apache-2.0), or safari (the
//   machine's Safari through safaridriver, with selenium-webdriver, Apache-2.0; macOS only).
//   build: plain, or wide (the build with wide arithmetic).
// The page runs every benchmark of cases.txt; this script waits for it, then writes
// {engine, build, version, supported, lines, errors} to out.json. Versions are pinned by the
// variant workflow's npm install.

import { writeFileSync } from "node:fs";

const [engine, build, ms, base, out] = process.argv.slice(2);
const url = `${base}/page.html?build=${build}&ms=${ms}`;
// Three hours: every benchmark of cases.txt, six rounds each.
const patience = 3 * 60 * 60 * 1000;

async function withPlaywright() {
  const playwright = await import("playwright");
  const type = { chromium: playwright.chromium, firefox: playwright.firefox, webkit: playwright.webkit }[
    engine
  ];
  if (!type) throw new Error(`unknown engine ${engine}`);
  const browser = await type.launch();
  const version = browser.version();
  const page = await browser.newPage();
  await page.goto(url);
  await page.waitForFunction(() => window.variantResults?.done === true, null, {
    timeout: patience,
    polling: 5000,
  });
  const results = await page.evaluate(() => window.variantResults);
  await browser.close();
  return { version, results };
}

async function withSafari() {
  const { Builder } = await import("selenium-webdriver");
  const driver = await new Builder().forBrowser("safari").build();
  try {
    const capabilities = await driver.getCapabilities();
    const version = capabilities.get("browserVersion");
    await driver.get(url);
    await driver.wait(
      async () => driver.executeScript("return !!(window.variantResults && window.variantResults.done)"),
      patience,
      "the page did not finish",
      5000,
    );
    const results = await driver.executeScript("return window.variantResults");
    return { version, results };
  } finally {
    await driver.quit();
  }
}

const { version, results } = engine === "safari" ? await withSafari() : await withPlaywright();
writeFileSync(
  out,
  JSON.stringify(
    {
      engine,
      build,
      version,
      supported: results.supported,
      lines: results.lines,
      errors: results.errors,
    },
    null,
    1,
  ),
);
console.log(
  `${engine} ${version}, ${build}: ${results.supported ? `${results.lines.length} timings` : "unsupported"}, ${results.errors.length} error(s)`,
);
