// Runs the model on the Rust engine: the WebAssembly build of construct-wasm in ../pkg (tools/transport-truth/build-wasm.ps1).
// Each run opens the scenario in a fresh Session, runs the one period and hands back the same results object the
// JavaScript prototype makes (tools/transport-truth/wasm-check.mjs proves they match). If the module cannot load, the
// prototype keeps running the model and the badge says so.
import init, { Session } from '../pkg/construct_wasm.js';

const badge = document.getElementById('engineBadge');
const show = (rust, why) => {
  if (!badge) return;
  badge.textContent = rust ? 'Rust engine' : 'Prototype engine';
  badge.title = rust ? 'Model runs use the Rust engine (WebAssembly).' : 'Model runs use the JavaScript prototype' + (why ? ': ' + why : '.');
  badge.dataset.engine = rust ? 'rust' : 'js';
};

// Store.serialise leaves these out too; results are made again by the run
const strip = (k, v) => (k === 'res' || k === 'paths' || k === 'arcKeys' || k === 'settingsNotice') ? undefined : v;
// The command layer writes non-finite numbers as text, as the prototype's own results JSON does
const finite = (k, v) => v === 'NaN' ? NaN : v === 'Infinity' ? Infinity : v === '-Infinity' ? -Infinity : v;

async function rustAssign(sc, opts = {}) {
  const S = opts.settings || Store.S, period = opts.period || S.period;
  const project = { ...Store.P, settings: S, scenarios: [sc], active: sc.id };
  const session = new Session();
  try {
    const call = (name, params) => JSON.parse(session.command(name, JSON.stringify(params)), finite);
    call('transport.project.open', { text: JSON.stringify(project, strip) });
    if (opts.onProgress) session.onProgress(e => { const x = JSON.parse(e); if (x.event === 'progress') opts.onProgress(x.iteration, x.gap, x.message); });
    const assign = {};
    for (const k of ['demandFactor', 'select', 'arcPenalty']) if (opts[k] != null) assign[k] = opts[k];
    call('transport.run', { scenario: sc.id, periods: [period], assign });
    return call('transport.results.get', { scenario: sc.id, period, paths: true });
  } finally {
    session.free();
  }
}

try {
  await init();
  Engine.useRust(rustAssign);
  show(true);
} catch (e) {
  console.error('Rust engine did not load', e);
  show(false, e && e.message);
}
