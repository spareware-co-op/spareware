// Checks the WebAssembly build of the Transport command layer against the prototype's recorded answers.
//
// Loads projects/spare-construct/demo/transport/pkg (built by tools/transport-truth/build-wasm.ps1 or the commands in that file's
// header), opens inputs/grid.json and inputs/corridor.json through the `command` function, runs the model for every scenario and
// period, and compares each result with expected/<name>.json, using the Rust comparator's rules: numbers within a relative 1e-6 (or an
// absolute 1e-9), text and booleans exactly, "NaN", "Infinity" and "-Infinity" equal to the matching number, and the time stamp and
// run time left out. Run: node tools/transport-truth/wasm-check.mjs
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = join(here, '..', '..', 'projects', 'spare-construct', 'demo', 'transport', 'pkg');
const { initSync, Session } = await import(new URL('file:///' + join(pkg, 'construct_wasm.js').replace(/\\/g, '/')));
initSync({ module: readFileSync(join(pkg, 'construct_wasm_bg.wasm')) });

const REL_TOL = 1e-6, ABS_TOL = 1e-9, IGNORED = new Set(['ts', 'ms']);
const num = v => (typeof v === 'number' ? v : v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : v === '-Infinity' ? -Infinity : undefined);
const close = (a, e) => {
  if (Number.isNaN(a) || Number.isNaN(e)) return Number.isNaN(a) && Number.isNaN(e);
  if (!Number.isFinite(a) || !Number.isFinite(e)) return a === e;
  const d = Math.abs(a - e);
  return d <= ABS_TOL || d <= REL_TOL * Math.max(Math.abs(a), Math.abs(e));
};
const diffs = [];
let compared = 0;
function walk(a, e, path) {
  const x = num(a), y = num(e);
  if (x !== undefined && y !== undefined && (typeof a === 'number' || typeof e === 'number')) {
    compared++;
    if (!close(x, y)) diffs.push(`${path}: expected ${JSON.stringify(e)}, got ${JSON.stringify(a)}`);
    return;
  }
  if (Array.isArray(e) && Array.isArray(a)) {
    if (a.length !== e.length) diffs.push(`${path}.length: expected ${e.length}, got ${a.length}`);
    for (let i = 0; i < Math.min(a.length, e.length); i++) walk(a[i], e[i], `${path}[${i}]`);
    return;
  }
  if (e && a && typeof e === 'object' && typeof a === 'object' && !Array.isArray(e) && !Array.isArray(a)) {
    for (const k of Object.keys(e)) {
      if (IGNORED.has(k)) continue;
      if (!(k in a)) { compared++; diffs.push(`${path}.${k}: missing`); continue; }
      walk(a[k], e[k], `${path}.${k}`);
    }
    for (const k of Object.keys(a)) if (!IGNORED.has(k) && !(k in e)) { compared++; diffs.push(`${path}.${k}: not expected`); }
    return;
  }
  compared++;
  if (a !== e) diffs.push(`${path}: expected ${JSON.stringify(e)}, got ${JSON.stringify(a)}`);
}

const commands = JSON.parse(Session.commands());
console.log(`${commands.length} commands in the module`);
let runs = 0;
for (const name of ['grid', 'corridor']) {
  const input = readFileSync(join(here, 'inputs', `${name}.json`), 'utf8');
  const expected = JSON.parse(readFileSync(join(here, 'expected', `${name}.json`), 'utf8'));
  const session = new Session();
  const call = (cmd, params = {}) => JSON.parse(session.command(cmd, JSON.stringify(params)));
  call('transport.project.open', { text: input });
  const events = [];
  session.onProgress(e => events.push(JSON.parse(e)));
  const out = call('transport.run', { all: true });
  if (out.skipped.length) throw new Error(`${name}: runs skipped: ${JSON.stringify(out.skipped)}`);
  if (!events.some(e => e.event === 'progress')) throw new Error(`${name}: the run reported no progress`);
  for (const sid of Object.keys(expected.scenarios)) {
    for (const period of ['AM', 'PM']) {
      const got = call('transport.results.get', { scenario: sid, period, paths: true });
      walk(got, expected.scenarios[sid].periods[period].res, `${name}.${sid}.${period}.res`);
      runs++;
    }
    walk(call('transport.analysis.capital', { scenario: sid }), expected.scenarios[sid].capital, `${name}.${sid}.capital`);
  }
  // an error is thrown with its code
  let thrown = null;
  try { session.command('transport.network.get', JSON.stringify({ kind: 'node', id: 'nope' })); } catch (e) { thrown = e; }
  if (!thrown || thrown.code !== 'not_found') throw new Error('a missing junction should throw not_found');
  // the ids and clock come from the browser: a new junction gets a fresh id
  const made = call('transport.network.add_node', { point: [-36.9, 174.7] });
  if (!/^n[0-9a-z]+$/.test(made.id)) throw new Error('unexpected id ' + made.id);
}
if (diffs.length) {
  console.error(`${diffs.length} difference(s) of ${compared} values compared:`);
  for (const d of diffs.slice(0, 30)) console.error('  ' + d);
  process.exit(1);
}
console.log(`wasm-check: ${runs} runs match the prototype (${compared} values compared)`);
