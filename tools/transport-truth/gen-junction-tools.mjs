// Records the prototype's junction tools (js/features/junctions.js `Junctions`) for the Rust port's junction_tools module
// (projects/spare-construct/crates/construct-transport/src/junction_tools, tests/junction_tools_truth.rs).
// Writes
//   fixtures/data/junction-tools-<variant>.json   the saved project of the variant (settings, network and edits) and the engine's
//                                                 results for every scenario and period ({ name, project, results, phf })
//   expected/junction-tools-<variant>.json        { name, calls: [{ fn, args..., out }] }: every call the Rust port replays, with the
//                                                 prototype's answer (text exactly, data as plain JSON; a throw is { error })
// Variants: the two saved test projects as they are (movement capacity), the same with approach capacity, a grid with a made-up
// two-node junction site, an extent of effects, an unconverged run, a stale run, the inter-peak and Saturday periods, counted
// peak hour factors, roles for the report's scenario choice, and the junction roles of a project without results.
// The clock is the harness's (2026-10-21) and the time zone is UTC, so the loader script and the volume grids carry fixed dates.
process.env.TZ = 'UTC';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadReportCore, plain, clone } from './js-report-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });
const read = rel => JSON.parse(readFileSync(join(here, rel), 'utf8'));
const write = (rel, data) => { const t = JSON.stringify(data); writeFileSync(join(here, rel), t); console.log(rel, t.length, 'bytes'); };

// Counted peak hour factors the stand-in Calibration.phf answers with (node id and period): recorded with the variant
const PHF_BY_VARIANT = {};

/* One variant: a project (edited), its results for the periods, and every junction call recorded on it. */
async function variant(name, base, edit = () => {}, periods = ['AM', 'PM'], opts = {}) {
  const phf = {};
  const globals = opts.phf ? { Calibration: { phf: (id, period) => { const v = (phf[`${id}|${period}`]); return v === undefined ? null : v; } } } : {};
  const core = loadReportCore({ seed: 1, globals });
  const { Store, Engine, Junctions: J, Sites, U, Turns } = core;
  const P = core.migrate(read(`inputs/${base}.json`));
  Store.P = P;
  edit(P, core);
  for (const sc of P.scenarios) {
    P.active = sc.id; sc.res = {};
    for (const period of (opts.periods || periods)) {
      const mi = opts.maxIter && opts.maxIter[sc.id];
      if (mi) { const keep = P.settings.maxIter; P.settings.maxIter = mi; sc.res[period] = await Engine.assign(sc, { period }); P.settings.maxIter = keep; }
      else sc.res[period] = await Engine.assign(sc, { period });
    }
  }
  P.active = P.scenarios[0].id;
  if (opts.after) opts.after(P, core);
  // every node and period gets an answer: usable factors, one under the 0.25 floor and one over 1 (both ignored)
  if (opts.phf) for (const sc of P.scenarios) for (const p of Object.keys(sc.res)) Object.keys(sc.nodes).forEach((id, i) => { phf[`${id}|${p}`] = [0.92, 0.88, 0.97, 0.2, 1.01, 0.5][(i * 7 + p.length) % 6]; });
  // fixture: the project with an empty res (as the app saves it), and the results apart, non-finite numbers as text
  const resKeep = P.scenarios.map(s => s.res);
  P.scenarios.forEach(s => { s.res = {}; });
  const projectText = Store.serialise(P);
  P.scenarios.forEach((s, i) => { s.res = resKeep[i]; });
  write(`fixtures/data/junction-tools-${name}.json`, { name, project: JSON.parse(projectText), results: Object.fromEntries(P.scenarios.map(s => [s.id, plain(s.res)])), phf });

  const calls = [];
  const out = (fn, args, thunk) => {
    let o;
    try { o = thunk(); } catch (e) { o = { error: e.message }; }
    calls.push({ fn, ...args, out: o === undefined ? null : plain(o) });
  };
  const scs = P.scenarios;
  let bump = 0;
  const fresh = () => { Store.ver = (Store.ver || 0) + (++bump); };   // the caches key on Store.ver: a new key for each change
  const periodsOf = sc => J.periodsOf(sc);
  const stripD = D => (D ? { ...D, e: { id: D.e.id, kind: D.e.kind, name: D.e.name }, nr: undefined, res: undefined, groups: D.groups.map(g => ({ ...g, appr: g.appr.map(x => ({ rows: x.rows, sub: x.sub })) })) } : D);
  const entJ = e => ({ id: e.id, kind: e.kind, name: e.name, node: e.node, nodes: e.nodes, type: e.type, atSiteNo: e.atSiteNo === undefined ? null : e.atSiteNo, ll: e.ll, hasNr: !!e.nr });

  const run = async () => {
    for (let si = 0; si < scs.length; si++) {
      const sc = scs[si];
      out('periodsOf', { sc: si }, () => periodsOf(sc));
      for (const p of Object.keys(sc.res)) {
        const res = sc.res[p];
        for (const all of [false, true]) {
          out('entries', { sc: si, period: p, all }, () => J.entries(sc, res, all ? { all: true } : {}).map(entJ));
        }
        out('footer', { sc: si, period: p }, () => J.footer(res));
        const ents = J.entries(sc, res, { all: true });
        for (const e of ents) {
          out('entryFor', { sc: si, period: p, id: e.id }, () => { const x = J.entryFor(sc, res, e.id); return x ? entJ(x) : null; });
          if (e.nodes.length > 1) for (const n of e.nodes.slice(0, 2)) out('entryFor', { sc: si, period: p, id: n }, () => { const x = J.entryFor(sc, res, n); return x ? entJ(x) : null; });
          out('nodeLegs', { sc: si, period: p, node: e.node }, () => J.nodeLegs(sc, res, e.node).map(a => ({ id: a.id, o: a.o, leg: a.leg, legLabel: a.legLabel, road: a.road, b: a.b, isIn: a.isIn, isOut: a.isOut, inKey: a.inKey, outKey: a.outKey, conn: a.conn, link: a.link ? a.link.id : null })));
          out('j1', { sc: si, period: p, id: e.id }, () => stripD(J.j1(sc, res, e.id)));
          for (const head of [true, false]) out('j1Html', { sc: si, period: p, id: e.id, head }, () => J.j1Html(J.j1(sc, res, e.id), head ? {} : { head: false, P }));
          out('summary', { sc: si, period: p, id: e.id }, () => J.summary(sc, res, e));
          out('checks', { sc: si, period: p, id: e.id }, () => J.checks(sc, res, e));
          out('xid', { sc: si, period: p, id: e.id }, () => J.xid(e));
          out('moves', { sc: si, period: p, id: e.id }, () => J.moves(sc, res, e.node).map(m => ({ key: m.key, from: m.from.id, to: m.to.id, mv: m.mv, turn: m.turn, id: m.id, banned: m.banned, explicit: m.explicit, rule: m.rule, v: m.v, hv: m.hv, inKey: m.inKey, outKey: m.outKey })));
          if (!e.nodes || e.kind === 'node') {
            out('banSvg', { sc: si, period: p, id: e.id }, () => J.banSvg(sc, e.node, J.moves(sc, res, e.node)));
            if (ents.indexOf(e) < 2) out('banSvg', { sc: si, period: p, id: e.id, size: 300 }, () => J.banSvg(sc, e.node, J.moves(sc, res, e.node), 300));
            out('approaches', { sc: si, period: p, id: e.id }, () => J.approaches(sc, e.node).map(a => ({ link: a.link.id, leg: a.leg.legLabel, gl: a.gl, raw: a.raw, model: a.model, onSite: a.onSite, editLanes: a.editLanes, src: a.src })));
          }
          for (const layer of (ents.indexOf(e) < 2 ? ['total', 'dev', 'base'] : ['total'])) out('figure', { sc: si, period: p, id: e.id, layer }, () => J.figure(sc, res, null, e.id, { layer }));
          if (ents.indexOf(e) < 2) out('figure', { sc: si, period: p, id: e.id, size: 240 }, () => J.figure(sc, null, res, e.id, { size: 240 }));
          out('turnCountFile', { sc: si, period: p, id: e.node }, () => J.turnCountFile(sc, res, e.node));
          out('mergeCandidates', { sc: si, id: e.node }, () => J.mergeCandidates(sc, e.node));
          out('upstreamLen', { sc: si, period: p, id: e.id }, () => {
            const D = J.j1(sc, res, e.id); if (!D) return null;
            return D.nr.appr.map(a => [a.key, J.upstreamLen(sc, res, a.key, new Set(e.nodes))]);
          });
        }
        out('figure', { sc: si, period: p, id: 'nothere' }, () => J.figure(sc, res, null, 'nothere', {}));
        out('j1', { sc: si, period: p, id: 'nothere' }, () => stripD(J.j1(sc, res, 'nothere')));
        out('j1Html', { sc: si, period: p, id: 'nothere', head: true }, () => J.j1Html(J.j1(sc, res, 'nothere'), {}));
        for (const f of [undefined, 1.25, 0.9]) {
          fresh();
          out('flowScale', { sc: si, period: p, f: f ?? null }, () => { const r = f === undefined ? J.flowScale(sc, res) : J.flowScale(sc, res, f); return r && { ...r, rows: r.rows.map(x => ({ ...x, first: x.first, already: x.already })) }; });
        }
        fresh();
        out('scaleHtml', { sc: si, period: p }, () => J.scaleHtml(sc, res, {}));
        out('scaleHtml', { sc: si, period: p, P: true }, () => J.scaleHtml(sc, res, { P }));
        out('scaleHtml', { sc: si, period: null }, () => J.scaleHtml(sc, null, {}));
        out('losTable', { method: (res.junction && res.junction.method) || 'hcm6' }, () => J.losTable((res.junction && res.junction.method) || 'hcm6'));
      }
      out('j2', { sc: si }, () => J.j2(sc).map(r => ({ ...r, p: r.p })));
      out('j2Html', { sc: si }, () => J.j2Html(sc));
      out('j2Html', { sc: si, fly: false, P: true }, () => J.j2Html(sc, periodsOf(sc), { fly: false, P }));
      out('junctionMovementsCSV', { sc: si }, () => J.junctionMovementsCSV(P, sc));
      out('keyJunctions', { sc: si }, () => J.keyJunctions(sc));
      out('keyJunctions', { sc: si, n: 2 }, () => J.keyJunctions(sc, 2));
      for (const force of [false, true]) {
        out('turningMovementsCSV', { sc: si, force }, () => J.turningMovementsCSV(P, sc, undefined, { force }));
        out('volumesCSV', { sc: si, force }, () => J.volumesCSV(sc, undefined, null, { force }));
        out('loaderPy', { sc: si, force }, () => J.loaderPy(sc, undefined, null, { force }));
        out('gridsHtml', { sc: si, force }, () => J.gridsHtml(sc, undefined, null, { force }));
        for (const scope of ['auto', 'controlled', 'all']) {
          out('volumeRows', { sc: si, force, scope }, () => J.volumeRows(sc, J.periodsOf(sc).map(p => sc.res[p]), null, { scope, force }));
          if (scope !== 'auto') out('volumesCSV', { sc: si, force, scope }, () => J.volumesCSV(sc, undefined, null, { force, scope }));
        }
      }
      const rows = J.volumeRows(sc, J.periodsOf(sc).map(p => sc.res[p]), null, { scope: 'all' });
      out('asCountCSV', { sc: si, date: '', src: 'rows' }, () => J.asCountCSV(rows));
      out('asCountCSV', { sc: si, date: '2026-10-21', src: 'rows' }, () => J.asCountCSV(rows, '2026-10-21'));
      const csvText = J.volumesCSV(sc, undefined, null, { force: true, scope: 'all' });
      out('asCountCSV', { sc: si, date: '2026-10-20', src: 'csv' }, () => J.asCountCSV(csvText, '2026-10-20'));
      out('parseVolumesCSV', { sc: si, scope: 'all' }, () => J.parseVolumesCSV(csvText));
      out('volumesCSV', { sc: si, ids: ids0(sc) }, () => J.volumesCSV(sc, undefined, ids0(sc), { force: true }));
      out('phfOf', { sc: si }, () => Object.values(sc.res).length ? J.entries(sc, Object.values(sc.res)[0], { all: true }).flatMap(e => Object.keys(sc.res).map(p => [e.id, p, J.phfOf(sc, e, p)])) : []);
    }
    for (const s of ['0', '1']) void s;
    // comparisons across scenarios
    for (let i = 0; i < scs.length; i++) for (let j = 0; j < scs.length; j++) {
      if (i === j) continue;
      out('j3', { from: i, to: j }, () => J.j3(scs[i], scs[j]));
      out('j3Html', { from: i, to: j }, () => J.j3Html(scs[i], scs[j]));
      out('j3Html', { from: i, to: j, P: true }, () => J.j3Html(scs[i], scs[j], undefined, { P }));
      out('compareOrder', { active: i, other: j, swap: false }, () => J.compareOrder(P, scs[i], scs[j], false).map(s => scs.indexOf(s)));
      out('compareOrder', { active: i, other: j, swap: true }, () => J.compareOrder(P, scs[i], scs[j], true).map(s => scs.indexOf(s)));
    }
    out('j3Html', { from: null, to: 0 }, () => J.j3Html(null, scs[0]));
    out('reportSel', { ctx: 'none' }, () => { const r = J.reportSel(P, {}); return { list: r.list.map(s => scs.indexOf(s)), pairs: r.pairs.map(([a, b, h, k]) => [scs.indexOf(a), scs.indexOf(b), h, k]) }; });
    out('reportSel', { ctx: 'scenarios' }, () => { const r = J.reportSel(P, { scenarios: scs.slice().reverse() }); return { list: r.list.map(s => scs.indexOf(s)), pairs: r.pairs.map(([a, b, h, k]) => [scs.indexOf(a), scs.indexOf(b), h, k]) }; });
    const Rep = core.Report;
    for (const id of ['junctions-performance', 'junctions-movement-summary', 'junctions-comparison', 'junctions-turning']) {
      fresh();
      out('section', { id }, () => core.Registry.get('report', id).render(P, Rep.context(P, {})));
      out('section', { id, ctx: 'scenarios' }, () => core.Registry.get('report', id).render(P, { ...Rep.context(P, {}), scenarios: scs.slice(0, 1) }));
    }
  };
  const ids0 = sc => { const r = Object.values(sc.res)[0]; const e = r ? J.entries(sc, r, { all: true }) : []; return e.slice(0, 2).map(x => x.id); };
  await run();
  write(`expected/junction-tools-${name}.json`, { name, calls });
  PHF_BY_VARIANT[name] = phf;
}

const setRoles = (P, roles) => P.scenarios.forEach((s, i) => { if (roles[i]) s.role = roles[i]; });
const setNotConverged = P => { P.settings.gapTol = 1e-12; };

await variant('corridor', 'corridor');
await variant('grid', 'grid');
await variant('corridor-arc', 'corridor', P => { P.settings.junction.capMethod = 'arc'; });
await variant('grid-arc', 'grid', P => { P.settings.junction.capMethod = 'arc'; setRoles(P, ['existing', 'baseline', 'mitigation']); });
await variant('grid-site', 'grid', (P, { Sites, U }) => {
  // a two-node junction site: the signals node and the next node along, by hand
  const sc = P.scenarios[1];
  const sig = Object.values(sc.nodes).find(n => n.control === 'signals');
  const near = Object.values(sc.nodes).filter(n => n.id !== sig.id).sort((a, b) => U.dist([sig.lat, sig.lng], [a.lat, a.lng]) - U.dist([sig.lat, sig.lng], [b.lat, b.lng]))[0];
  for (const s of P.scenarios) { const a = s.nodes[sig.id], b = s.nodes[near.id]; if (a && b) { const id = Sites.merge(s, [a.id, b.id]); if (id) { s.sites[id].name = 'Gate junction'; s.sites[id].atSiteNo = ['4', '5']; } } }
  setRoles(P, ['existing', 'proposal', 'mitigation']);
}, ['AM', 'PM', 'IP']);
await variant('corridor-extent', 'corridor', (P, { Sites }) => {
  const sc = P.scenarios[1];
  const ids = Object.values(sc.nodes).filter(n => n.control && n.control !== 'none').map(n => n.id);
  sc.extent = { sites: ids.slice(0, 2) };
  P.scenarios[0].extent = { sites: [ids[ids.length - 1]] };
  setRoles(P, ['baseline', 'proposal']);
}, ['AM', 'PM', 'SAT']);
// runs short of the SIDRA export gap (1e-4): the gap is set on the stored results
await variant('corridor-unconverged', 'corridor', P => { P.settings.gapTol = 1e-14; P.settings.maxIter = 3; }, ['AM', 'PM'], { after: P => { P.scenarios[0].res.AM.gap = 0.0031; P.scenarios[1].res.PM.gap = 0.00051; } });
await variant('grid-stale', 'grid', P => { setRoles(P, ['existing', 'baseline', 'proposal']); }, ['AM', 'PM', 'IP'], { after: P => { for (const sc of P.scenarios.slice(1)) for (const r of Object.values(sc.res)) r.stale = true; } });
await variant('grid-phf', 'grid', P => { P.settings.calib = { windows: { AM: ['07:30', '08:30'], PM: ['16:30', '17:30'] } }; }, ['AM', 'PM', 'IP'], { phf: true });
await variant('corridor-ban', 'corridor', (P) => {
  // banned turns and U-turns allowed on a junction, a node name, an AT site number and a counted site list for the exports
  const sc = P.scenarios[0];
  const n = Object.values(sc.nodes).find(x => x.control === 'signals');
  if (n) { const ls = Object.values(sc.links).filter(l => l.a === n.id || l.b === n.id); if (ls.length >= 2) n.bans = [`${ls[0].id}>${ls[1].id}`]; n.allowU = true; n.name = 'Main & Side (west)'; n.atSiteNo = '12'; }
});
// a junction grouped into a site in one scenario and not in the other, one that exists in one scenario only, a flow that does not
// add up, and U-turns: the comparison pills and the turn checks
await variant('grid-regroup', 'grid', (P, { Sites, U }) => {
  const [a, b] = P.scenarios;
  const sig = Object.values(a.nodes).find(n => n.control === 'signals');
  const near = Object.values(a.nodes).filter(n => n.id !== sig.id).sort((p, q) => U.dist([sig.lat, sig.lng], [p.lat, p.lng]) - U.dist([sig.lat, sig.lng], [q.lat, q.lng]));
  const id = Sites.merge(a, [sig.id, near[0].id]); if (id) a.sites[id].name = 'Gate junction';
  const id2 = Sites.merge(b, [sig.id, near[1].id]); if (id2) b.sites[id2].atSiteNo = 'AT-77';
  // the roundabout exists only in the first scenario, the stop junction only in the second
  const rb = Object.values(a.nodes).find(n => n.control === 'roundabout'); if (rb) P.scenarios[2].nodes[rb.id].control = 'none';
  const sp = Object.values(a.nodes).find(n => n.control === 'stop'); if (sp) a.nodes[sp.id].control = 'none';
  P.settings.turns = { ...(P.settings.turns || {}), uturn: 30 };
  setRoles(P, ['existing', 'baseline', 'proposal']);
}, ['AM', 'PM'], {
  after: P => {
    // turns that do not add up to the approach flow
    for (const sc of P.scenarios) for (const r of Object.values(sc.res)) { const ids = Object.keys(r.turns || {}); if (ids.length) { const l = r.turns[ids[0]]; if (l.length) { l[0].v = l[0].v * 1.6 + 7; l[0].pcu = l[0].pcu * 1.6 + 7; } } }
  },
});
// a very short approach link with a long queue
await variant('corridor-short', 'corridor', () => {}, ['AM', 'PM'], {
  after: P => {
    for (const sc of P.scenarios) for (const r of Object.values(sc.res)) {
      for (const nr of Object.values(r.nodes)) for (const a of nr.appr) { if (a.ctrl !== 'free') { a.q95 = 240; r.arcs[a.key].len = 22; } }
      // a U-turn over the flag (and so a turn count that no longer adds up)
      const ids = Object.keys(r.turns || {}); if (ids.length && r.turns[ids[0]].length) { r.turns[ids[0]][0].mv = 'U'; r.turns[ids[0]][0].v = 45; r.turns[ids[0]][0].pcu = 48; }
    }
  },
});
console.log('junction tools recorded');
