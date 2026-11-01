// The reference runner's timings in a browser (piece G107; chapter 23.13 of the kernel
// architecture). Loaded by browsers.mjs, which reads what it reports.
//
// Times every benchmark named in the cases file (?cases=) with geom-bench-guest's
// `time <name> <round ms> 1`, built for wasm32-wasip1 (guest.wasm), at 1 thread and then at 8: one
// Web Worker per thread (worker.mjs), each with its own instance of the guest, all given the job at
// once, as the native build runs one thread per copy. The kernel runs serially in one Web Worker
// in a browser until WebAssembly threads reach stable Rust (chapter 23.4), so 8 threads here are 8
// workers each running its own copy. Each worker's guest builds its batch, checks it against
// golden.txt and times its rounds; the page combines them as the native guest combines its threads:
// the upper median of the workers' medians, the fewest passes, and the slowest worker's median.
//
// Each finished job is reported through window.referenceJob as the runner's job line without its
// `job <engine>` prefix: `<threads> <start ms> <end ms> <exit status> <text>`. A job is started
// only while it can finish before the deadline (?deadline=, milliseconds since 1970), allowing
// eight rounds of ?round= milliseconds.

const THREADS = [1, 8];

const params = new URLSearchParams(location.search);
const round = Number(params.get("round") ?? "150");
const deadline = Number(params.get("deadline") ?? "0");
const casesFile = params.get("cases") ?? "cases.txt";

// The upper median: the element at half the count once sorted, as the guest takes it.
function upperMedian(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

// Runs `name` on the first `count` workers at once; resolves to each worker's result.
function runOn(workers, count, name) {
  return Promise.all(
    workers.slice(0, count).map(
      (worker) =>
        new Promise((resolve) => {
          worker.onmessage = (event) => resolve(event.data);
          worker.postMessage({ args: ["geom-bench-guest", "time", name, String(round), "1"] });
        }),
    ),
  );
}

// One job's text, from its workers' results: the combined line, or the first failure.
function combine(name, count, results) {
  const failed = results.find((r) => r.status !== 0);
  if (failed) {
    const why = (failed.stderr.find((l) => l.length > 0) ?? "no message").replaceAll("\n", " ");
    return { status: failed.status, text: `${name} ${why}` };
  }
  const fields = results.map((r) => (r.stdout[0] ?? "").split(" "));
  if (fields.some((f) => f.length !== 5 || f[0] !== name)) {
    return { status: 3, text: `${name} the guest printed an unexpected line` };
  }
  const medians = fields.map((f) => Number(f[1]));
  const passes = Math.min(...fields.map((f) => Number(f[2])));
  const median = upperMedian(medians);
  const slowest = Math.max(...medians);
  return { status: 0, text: `${name} ${median.toFixed(3)} ${passes} ${count} ${slowest.toFixed(3)}` };
}

let summary = "";
try {
  const module = await WebAssembly.compileStreaming(fetch("guest.wasm"));
  const cases = (await (await fetch(casesFile)).text()).split("\n").filter((l) => l.length > 0);
  const workers = Array.from({ length: Math.max(...THREADS) }, () => new Worker("worker.mjs", { type: "module" }));
  await Promise.all(
    workers.map(
      (worker) =>
        new Promise((resolve) => {
          worker.onmessage = resolve;
          worker.postMessage({ module });
        }),
    ),
  );
  await window.referenceStarted(self.crossOriginIsolated === true);
  let jobs = 0;
  let stopped = false;
  for (const count of THREADS) {
    for (const name of cases) {
      if (Date.now() + 8 * round > deadline) {
        stopped = true;
        break;
      }
      const start = Date.now();
      const results = await runOn(workers, count, name);
      const end = Date.now();
      const { status, text } = combine(name, count, results);
      await window.referenceJob(`${count} ${start} ${end} ${status} ${text}`);
      jobs += 1;
    }
    if (stopped) break;
  }
  summary = `${jobs} jobs${stopped ? ", stopped at the deadline" : ""}`;
} catch (error) {
  summary = `failed: ${error}`;
}
await window.referenceDone(summary);
