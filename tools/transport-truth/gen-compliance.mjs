// Records the prototype's compliance checks (packs/nz/compliance.js, packs/nz/compliance-extra.js) for the Rust port's
// compliance module (projects/spare-construct/crates/construct-transport/src/compliance, tests/compliance_*.rs).
//
//   node gen-compliance.mjs data       the rule tables (thresholds, source texts, disclaimers, checklist) written to
//                                      src/compliance/data/*.json, which the Rust module embeds
//   node gen-compliance.mjs            every part: expected/compliance-*.json and fixtures/data/compliance-*.json
//   node gen-compliance.mjs <part>...  the named parts only
//
// The parts are the sibling files gen-compliance-<part>.mjs; each registers itself with `part(name, async c => { ... })`
// (see gen-compliance-lib.mjs for the harness: loadCompliance, withResults, saveFixture, write, plain).
import { readdirSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { here, RUST_DATA, loadCompliance, plain, PARTS } from './gen-compliance-lib.mjs';

/* ---------- the rule tables ---------- */
function dataTables(c) {
  const C = c.Compliance, X = c.ComplianceExtra;
  // interface state, caches and function tables are not rules
  const noise = new Set(['ui', 'F', 'SECS', 'SECTIONS', 'RENDER', 'guarded', 'metaCache', 'filled', 'shapeBox', 'holCache', 'AUTO', 'MATRIX_OPTS', 'SIG_OPTS', 'PILL_CLS', 'TIMEOUT', 'GUARD']);
  const prune = o => Object.fromEntries(Object.entries(o).filter(([k]) => !noise.has(k)));
  const core = prune(plain(C.__internal));
  const extra = prune(plain(X.__internal));
  const model = plain({ CLASSES: c.CLASSES, LANDUSES: c.LANDUSES, CONTROLS: c.CONTROLS, STATUS: c.STATUS, CYCLE: c.CYCLE, SCENARIO_ROLES: c.SCENARIO_ROLES, FUNDING: c.FUNDING, REGISTER_TYPES: c.REGISTER_TYPES });
  mkdirSync(RUST_DATA, { recursive: true });
  const out = (name, data) => { const t = JSON.stringify(data, null, 1); writeFileSync(join(RUST_DATA, name), t); console.log('src/compliance/data/' + name, t.length, 'bytes'); };
  out('compliance.json', core);
  out('extra.json', extra);
  out('model.json', model);
}

const args = process.argv.slice(2);
const c = loadCompliance();
if (args[0] === 'data') dataTables(c);
else {
  for (const f of readdirSync(here).filter(n => /^gen-compliance-.+\.mjs$/.test(n) && n !== 'gen-compliance-lib.mjs')) await import(pathToFileURL(join(here, f)).href);
  const names = args.length ? args : Object.keys(PARTS);
  for (const n of names) {
    if (!PARTS[n]) throw new Error(`no part '${n}' (parts: ${Object.keys(PARTS).join(', ') || 'none'})`);
    console.log('part', n);
    await PARTS[n]();
  }
}
