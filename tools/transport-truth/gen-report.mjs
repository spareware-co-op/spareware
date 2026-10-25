// Records the prototype's ITA report (js/features/report.js `Report`) for the Rust port's report module
// (projects/spare-construct/crates/construct-transport/src/report, tests/report_truth.rs). Writes
//   fixtures/data/report-<variant>.json   { name, project, results, host, foreign }: the saved project of the variant (settings, network,
//                                         the author's report inputs and every dataset the report reads), the engine's results for every
//                                         scenario and period, what the stand-ins for the other features answer, and the report sections
//                                         other features add (id, chapter, order, title, html)
//   expected/report-<variant>.json        everything the prototype produced for it: each section's HTML as built, the numbered document body,
//                                         the table of contents, the complete document, the data pack files and the zip
//   expected/report-functions.json        the pure functions on grids of inputs (risk and prediction formulas, pedestrian delay, the
//                                         staging trigger, table and chapter numbering helpers, the disclaimers)
// The clock is the harness's (2026-10-21), the time zone is UTC and Math.random is seeded, so every output is repeatable. The compliance,
// calibration, land-use and data-feed feature scripts are not loaded: the report treats them as not installed, except in the variants
// that define stand-ins for Compliance and ComplianceExtra (their answers are recorded with the variant).
process.env.TZ = 'UTC';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadReportCore, plain, NOW } from './js-report-core.mjs';
import { reportBlock, dataBlock, decorate, TEXT } from './report-fixtures.mjs';

const here = dirname(fileURLToPath(import.meta.url));
mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });
const read = rel => JSON.parse(readFileSync(join(here, rel), 'utf8'));
const write = (rel, data) => { const t = JSON.stringify(data); writeFileSync(join(here, rel), t); console.log(rel, t.length, 'bytes'); };
const only = process.argv[2];

// What the stand-ins for the compliance feature answer (recorded with the variant, replayed by the Rust test's host)
const COMPLIANCE = {
  supplementary: 'Supplementary: outside E27.8.1(4A) matters of discretion',
  regime: { pathway: 'consent', pathwayLabel: 'Resource consent', date: '2026-11-03', text: 'RMA as amended by the Planning Act 2026, as at lodgement (Sch 1 cl 19, 21)', short: 'RMA post-amendment' },
  stamp: { footer: 'Version stamp: RMA (post-amendment regime); Auckland Unitary Plan (operative text, July 2026); AT ITA Guidelines 2025 Version 2' },
  exportBlock: 'The scope linter has a blocking finding: no scope has been chosen for this proposal.',
};

async function variant(name, base, edit = () => {}, opts = {}) {
  if (only && only !== name) return;
  const host = opts.compliance ? { ...COMPLIANCE, ...(opts.compliance === true ? {} : opts.compliance) } : null;
  const conflated = {};
  const globals = {};
  if (host) {
    globals.Compliance = { supplementary: () => host.supplementary, regime: () => host.regime, stamp: () => host.stamp,
      conflate: (sc) => { const ls = Object.values(sc.links).filter(l => l.name).slice(0, 2); const m = {}; ls.forEach((l, i) => { m[l.id] = i ? 'Limited access road' : 'Vehicle access restriction'; }); conflated[sc.id] = m; return m; } };
    if (host.exportBlock) globals.ComplianceExtra = { exportBlock: () => host.exportBlock };
  }
  const core = loadReportCore({ seed: 1, globals });
  const { Store, Engine, Report, Registry, Scenarios, DataHub, U } = core;
  const P = core.migrate(read(`inputs/${base}.json`));
  Store.P = P;
  edit(P, core);
  const periods = opts.periods || ['AM', 'PM'];
  for (const sc of P.scenarios) {
    P.active = sc.id; sc.res = {};
    for (const period of (sc.periods || periods)) sc.res[period] = await Engine.assign(sc, { period });
  }
  P.active = opts.active ? P.scenarios[opts.active].id : P.scenarios[0].id;
  if (opts.after) opts.after(P, core);
  // sections other features add (registered here like a feature script would)
  const foreign = opts.foreign || [];
  for (const f of foreign) core.Registry.report({ id: f.id, chapter: f.chapter, order: f.order, title: f.title, render: () => f.html });
  if (opts.run) await opts.run(P, core);

  const resKeep = P.scenarios.map(s => s.res);
  P.scenarios.forEach(s => { s.res = {}; });
  const projectText = Store.serialise(P);
  P.scenarios.forEach((s, i) => { s.res = resKeep[i]; });
  const fixtureProject = JSON.parse(projectText);

  // what DataHub's crash analysis answers (the Rust port takes it through a trait), by scenario
  const crash = P.scenarios.map(sc => {
    const nr = DataHub.nodeRisk(sc), lr = DataHub.linkRisk(sc);
    return {
      nodeRisk: nr ? Object.fromEntries(Object.entries(nr).map(([k, v]) => [k, { name: v.name, nodes: v.nodes }])) : null,
      linkRisk: lr ? Object.fromEntries(Object.entries(lr).map(([k, v]) => [k, { aadt: v.aadt, aadtSrc: v.aadtSrc }])) : null,
      junctionOf: Object.fromEntries(Object.keys(sc.nodes).map(id => [id, DataHub.junctionOf(sc, id)])),
      onLinks: (() => { try { const s = DataHub.safetySummary(sc); return s ? s.onLinks : null; } catch { return null; } })(),
    };
  });
  // the project's data block as the prototype's DataHub.D() would make it (the pack runs it)
  write(`fixtures/data/report-${name}.json`, { name, project: fixtureProject, results: Object.fromEntries(P.scenarios.map((s, i) => [s.id, plain(resKeep[i])])), host, crash: plain(crash), conflated, foreign, opts: { period: opts.period || null, mapImages: opts.mapImages || null } });

  const out = { name };
  // each section's HTML as built, and the numbered document
  const sections = {};
  const orig = Registry.html;
  Registry.html = (def, fn, ...args) => { const h = orig(def, fn, ...args); sections[def.id] = h; return h; };
  const bopts = { ...(opts.period ? { period: opts.period } : {}), ...(opts.mapImages ? { mapImages: opts.mapImages } : {}) };
  const built = Report.build(P, bopts);
  Registry.html = orig;
  out.sections = sections;
  out.body = built.body; out.toc = built.toc; out.chapters = built.chapters; out.requests = built.requests;
  out.html = Report.html(P, bopts);
  if (!opts.noPack) {
    const files = Report.packFiles(P);
    out.pack = files.map(f => ({ name: f.name, data: typeof f.data === 'string' ? f.data : Buffer.from(f.data).toString('latin1'), of: f.of || null, handover: !!f.handover }));
    // the files again, after the pack has run (the pack changes the project: DataHub.D() adds the data block)
    out.projectAfterPack = Store.serialise(P);
    const when = new Date(Date.UTC(2026, 9, 21, 13, 5, 10));
    out.zipB64 = Buffer.from(Report.zip(files, when)).toString('base64');
    out.zipWhen = when.toISOString();
  }
  out.names = { report: Report.reportName(P), pack: Report.packName(P) };
  out.pick = (() => { const s = Report.pick(P); return Object.fromEntries(Object.entries(s).map(([k, v]) => [k, v ? P.scenarios.indexOf(v) : null])); })();
  out.assessedSites = Report.assessedSites(P);
  out.peakStatement = Report.peakStatement(P);
  out.conclusionText = Report.conclusionText(P);
  out.settingsRows = Report.settingsRows(P);
  out.crashYears = Report.crashYears(P);
  out.crashSites = Report.crashSites(P).map(s => ({ ...s, list: s.list.map(x => ({ k: x.k, d: x.d })) }));
  out.networkDiff = P.scenarios.flatMap((a, i) => P.scenarios.map((b, j) => (i === j ? null : [i, j, Report.networkDiff(a, b)])).filter(Boolean));
  write(`expected/report-${name}.json`, plain(out));
}

const addWithout = (P, core, from = 1) => {
  // a same-year scenario without the development: the proposal with its development zones taken out
  const src = P.scenarios[from], v = core.Scenarios.copy(src);
  for (const f of ['year', 'role', 'periods']) if (src[f] != null) v[f] = JSON.parse(JSON.stringify(src[f]));
  v.name = `Without development ${src.year || ''}`.trim();
  for (const z of Object.values(v.zones)) if (z.kind !== 'gateway' && z.lu) delete v.zones[z.id];
  P.scenarios.push(v);
  return v;
};
const roles = (P, r) => P.scenarios.forEach((s, i) => { if (r[i]) s.role = r[i]; });

const stagingRun = (P) => {
  const w = P.scenarios[P.scenarios.length - 1];
  const rows = [0, 10, 20, 30].map((pct, i) => ({ pct, dwellings: 100 * i, gfa: 2500 * i, fails: i > 1 ? ['n1'] : [], why: {}, crit: {}, dos: {}, worst: { id: 'n1', name: 'Main Road / Side 2', dos: 0.7 + i * 0.1, los: ['B', 'C', 'D', 'F'][i] }, queue: i === 3 }));
  P.report.staging = { at: Date.UTC(2026, 9, 15), sc: w.id, scName: w.name, mitigation: null, periods: ['AM', 'PM'], steps: [0, 10, 20, 30], rows,
    trigger: { pct: 20, dwellings: 200, gfa: 5000, sites: ['n1', 'n2'], names: ['Main Road / Side 2', ''], why: ['AM: DoS 0.90 over 0.85', 'LoS E'], how: ['new failure', ''] },
    lastPass: 10, existing: ['n2'], existingWhy: [{ id: 'n2', name: 'Side 4 <give way>', why: 'AM: LoS E' }, { id: 'n3', name: 'Other', why: '' }], resolved: false, measure: 'provide signals at Side 2',
    text: 'The Side 2 signals are needed before the 200th dwelling.', quantities: {}, worsen: 'x' };
  P.report.sens = { at: Date.UTC(2026, 9, 16), sc: w.id, scName: w.name, rows: [
    { label: 'Trip rates +10%', input: '+10%', trips: 1500.4, vht: 220.5, worst: { name: 'Main <Rd>', dos: 0.93 }, losEF: 2, over5: 3, over10: 1, elasticity: { vht: 1.2 } },
    { label: 'Discount 8%', input: '', trips: null, vht: null, worst: null, losEF: null, over5: null, bcrText: '1.4', bcr8Text: '0.9' }], sentences: ['No conclusion changes.', 'Growth <b>matters</b>.'], flags: ['Check the Side 2 result.'], note: 'Tests re-run the network.' };
};
const full = (P, core) => { reportBlock(P); dataBlock(P); decorate(P); };
const roleSet = (P) => roles(P, P.scenarios.length === 3 ? ['existing', 'baseline', 'proposal'] : ['existing', 'proposal']);

await variant('corridor', 'corridor');
await variant('grid', 'grid');
await variant('corridor-full', 'corridor', (P, core) => { full(P, core); roleSet(P); addWithout(P, core, 1); P.scenarios[2].role = 'baseline'; }, { after: stagingRun });
await variant('grid-full', 'grid', (P, core) => { full(P, core); P.scenarios[2].role = 'mitigation'; P.scenarios[1].role = 'proposal'; P.scenarios[0].role = 'existing'; addWithout(P, core, 1); P.scenarios[3].role = 'baseline'; P.scenarios[3].year = 2036; }, { after: stagingRun });
await variant('corridor-comp', 'corridor', (P, core) => { full(P, core); P.report.signoff = {}; P.report.scope = ''; P.report.details.pathway = 'ftaa'; P.compliance = { ctx: { pathway: 'consent', lodgement: '2026-11-03' } }; },
  { compliance: true, foreign: [
    { id: 'compliance-scope', chapter: 'front', order: 7, title: 'Scope', html: '<h2>Scope</h2><table><tr><th>Driver</th><th>Value</th></tr><tr><td>Trips</td><td>340</td></tr></table>' },
    { id: 'compliance-model-status', chapter: 'front', order: 6, title: 'Model status', html: '<h3>Status</h3><p>Fine.</p>' },
    { id: 'compliance-extra', chapter: 'Network performance', order: 45, title: 'Extra analysis', html: '<h4>Extra</h4><table><tr><th>DoS</th><th>LoS</th></tr><tr><td>0.8</td><td>C</td></tr></table><figure><figcaption>A picture</figcaption></figure>' },
    { id: 'pathway-annex', chapter: 'P', order: 10, title: 'Annex: Pathway', html: '<p>Pathway material.</p>' },
    { id: 'misc-late', chapter: '5', order: 150, title: 'Late material', html: '<p>After the register.</p>' } ] });
await variant('grid-maps', 'grid', (P, core) => { full(P, core); }, { mapImages: { 'vol-sidc01pnz-AM': { src: 'data:image/png;base64,AAAA', attribution: 'Test attribution' }, 'diff-AM': 'data:image/jpeg;base64,BBBB', 'vc-sidc01pnz-AM': { error: 'no tiles' } }, period: 'PM' });
console.log('report recorded');
