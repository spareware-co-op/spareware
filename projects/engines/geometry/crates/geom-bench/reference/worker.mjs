// One thread of the reference runner's timings in a browser (piece G107): a Web Worker that runs
// geom-bench-guest (wasm32-wasip1) once per message, in a fresh instance with its own WASI, and
// posts back its exit status and what it printed. page.mjs starts one per thread.
//
// The first message carries the compiled module ({ module }) and is answered with {}; each later
// one carries the guest's arguments ({ args }) and is answered with { status, stdout, stderr }.
// WASI comes from @bjorn3/browser_wasi_shim (MIT OR Apache-2.0), whose monotonic clock is
// performance.now(), at the version nightly.sh pins.

import { ConsoleStdout, File, OpenFile, WASI } from "./node_modules/@bjorn3/browser_wasi_shim/dist/index.js";

let module = null;

self.onmessage = async (event) => {
  if (event.data.module) {
    module = event.data.module;
    self.postMessage({});
    return;
  }
  const stdout = [];
  const stderr = [];
  const fds = [
    new OpenFile(new File([])),
    ConsoleStdout.lineBuffered((line) => stdout.push(line)),
    ConsoleStdout.lineBuffered((line) => stderr.push(line)),
  ];
  const wasi = new WASI(event.data.args, [], fds);
  let status;
  try {
    const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi.wasiImport });
    status = wasi.start(instance);
  } catch (error) {
    stderr.push(String(error));
    status = 4;
  }
  self.postMessage({ status, stdout, stderr });
};
