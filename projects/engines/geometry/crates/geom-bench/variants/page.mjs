// geom-bench's variant timings in a browser (piece G42; chapter 23.3 of the kernel architecture).
//
// Runs geom-bench-guest's `time` command for every benchmark named in cases.txt, in this page, on
// the wasm32-wasip1 build the query string names (?build=plain or ?build=wide, the build with
// wide arithmetic), with rounds of ?ms milliseconds, and leaves what it printed in
// window.variantResults for the driver (browsers.mjs or safari.mjs) to read. WASI is provided by
// @bjorn3/browser_wasi_shim (MIT OR Apache-2.0), installed beside this file at the version the
// variant workflow pins. A build the engine cannot validate (wide arithmetic where the engine has
// none) is reported as unsupported, not run.

import {
  ConsoleStdout,
  File,
  OpenFile,
  WASI,
} from "./node_modules/@bjorn3/browser_wasi_shim/dist/index.js";

const params = new URLSearchParams(location.search);
const build = params.get("build") ?? "plain";
const ms = params.get("ms") ?? "150";
const results = { build, lines: [], errors: [], supported: null, done: false };
window.variantResults = results;

try {
  const bytes = await (await fetch(`guest-${build}.wasm`)).arrayBuffer();
  results.supported = WebAssembly.validate(bytes);
  if (results.supported) {
    const module = await WebAssembly.compile(bytes);
    const cases = (await (await fetch("cases.txt")).text()).split("\n").filter((l) => l.length > 0);
    for (const name of cases) {
      const out = [];
      const fds = [
        new OpenFile(new File([])),
        ConsoleStdout.lineBuffered((line) => out.push(line)),
        ConsoleStdout.lineBuffered((line) => results.errors.push(`${name}: ${line}`)),
      ];
      const wasi = new WASI(["geom-bench-guest", "time", name, ms], [], fds);
      const instance = await WebAssembly.instantiate(module, {
        wasi_snapshot_preview1: wasi.wasiImport,
      });
      try {
        wasi.start(instance);
      } catch (e) {
        results.errors.push(`${name}: ${e}`);
      }
      results.lines.push(...out);
      // Yield between benchmarks, so the driver's polls are answered.
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }
} catch (e) {
  results.errors.push(String(e));
}
results.done = true;
