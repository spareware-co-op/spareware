'use strict';
/* Spare Construct · junction workspace (owner: junctions lane). Registers, through the plug-in registry:
   - node inspector sections: the turning diagram and movement table (turn-volumes), the turn-ban editor (click an
     arrow; writes node.bans) with the U-turn switch (node.allowU), the lane editor for movement capacity
     (node.lanes / site.lanes, movement-capacity) and junction-site grouping (merge, split; junction-sites);
   - a 'junction-site' panel, a map layer outlining sites and a Merge sites tool (J);
   - the upgraded Junctions drawer tab: J2 summary, J1 SIDRA-style Movement Summary, J3 comparison across scenarios,
     the +10% flow-scale test and the SIDRA hand-off (sidra-tables, sidra-export);
   - exports (junction_movements.csv, turning_movements.csv, SIDRA volumes CSV, the SIDRA 11 loader script, printable
     volume grids), report sections, Settings › Junction analysis and About-the-data rows.
   Legs and turn codes follow docs/spec/nz-conventions through Turns (leg = where traffic comes FROM; 'South: <road>',
   S-R2). Every junction result is labelled LABEL. Pure builders return strings or plain objects and read the store
   only, so test/f-junctions.test.js runs them headlessly.
   Deviations: the flow-scale test re-evaluates junctions at fixed flows through Engine.buildGraph / updateNodeCaps /
   junctionResult with the run's signal timings (no re-assignment). Storage ratio thresholds and the U-turn flag are
   Spare Construct assumptions. The CSVs and the SIDRA loader identify a junction site by its member nodes ('J' + a hash of
   the sorted node ids, see xid) rather than the random site id, so a re-run of 'Find junction sites' keeps the same
   SIDRA Intersection ID. A count PHF (15-minute) is exported with a 15-minute peak flow period. */

const Junctions = (() => {
  const esc = s => U.esc(s);
  const LABEL = Engine.JUNCTION_LABEL || 'HCM 6 / Akcelik-based, indicative; not SIDRA output';
  const ITA_URL = 'https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf';
  const TTOC_URL = 'https://www.qldc.govt.nz/media/lekcx4iu/ttoc-06-traffic-signals-modelling-guidelines.pdf';
  const SIDRA_API = 'https://api.sidrasolutions.com/latest/llms-full.txt';
  // Thresholds and conventions as data (overview rule 4)
  const STANDARDS = {
    // storage = 95% back of queue ÷ (bay length, or the distance back to the upstream junction's stop line)
    storage: { id: 'corridor-storage-ratio', name: 'Queue storage ratio (95% back of queue ÷ storage)', edition: 'Spare Construct assumption; supports AT ITA Guidelines 2025 v2 s4.2.1.1 safety effects', doc: 'docs/spec/sidra-tables', url: ITA_URL, verified: false, amber: 0.8, red: 1.0, basis: 'Spare Construct assumption' },
    flowScale: { id: 'ttoc06-flow-scale', name: 'Flow-scale test', edition: 'TTOC-06 Traffic Signals Modelling Guidelines', url: TTOC_URL, verified: false, factor: 1.10 },
    uTurn: { id: 'corridor-uturn-flag', name: 'U-turn flag', edition: 'Spare Construct assumption', doc: 'docs/spec/turn-volumes', verified: false, vph: 20 },
    balance: { id: 'corridor-turn-balance', name: 'Turns balance approach flows', edition: 'Spare Construct assumption', doc: 'docs/spec/turn-volumes', verified: false, tol: 1 },
    figure: { id: 'corridor-figure-round', name: 'Figure rounding', edition: 'Spare Construct convention (nearest 5 veh/h)', doc: 'docs/spec/turn-volumes', verified: false, step: 5 },
    sidra: { id: 'sidra11-nz', name: 'SIDRA 11 hand-off', edition: 'SIDRA INTERSECTION 11 .NET API (untested against a live install)', url: SIDRA_API, verified: false,
      siteType: { signals: 0, roundabout: 1, priority: 2, stop: 3, none: 2 }, typeName: ['signals', 'roundabout', 'give-way', 'two-way stop'], setup: 2, setupName: 'New Zealand', unitTime: 60, peakFlowPeriod: 30, phfPeriod: 15, growthPct: 0 },
  };
  // Stable junction id for files that leave Spare Construct: a node keeps its id; a site is 'J' + a hash of its sorted member
  // nodes, so it survives a re-run of 'Find junction sites' (whose site ids are random) and matches across scenarios
  const hash = s => { let h = 0x811c9dc5; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 0x01000193); } return (h >>> 0).toString(36); };
  const xid = e => (e.kind === 'site' ? 'J' + hash(e.nodes.slice().sort().join('|')) : e.id);
  // Convergence marker for exports forced past the gate (sidra-export, turn-volumes: gap ≤ 1e-4)
  const ncText = res => (Engine.exportReady(res).ok ? '' : `NOT CONVERGED: gap ${(+res.gap * 100).toPrecision(2)}%, SIDRA needs ${(Engine.STANDARDS.sidraGap.max * 100).toPrecision(1)}% or better`);
  const D3 = 'Junction delay, degree of saturation, level of service and queue results are calculated by Spare Construct using HCM 6 / Akcelik-based methods. They are indicative and are not SIDRA output. Any junction result relied on for a consent decision, signal design (ATOC Traffic Signals Design Guidelines) or condition should be confirmed in SIDRA or another tool accepted by the road controlling authority.';
  // D1 and D12 (docs/regulatory/_disclaimers.md, exact wording; D12 with the tool filled in). The SIDRA volumes and
  // turning CSVs stay unstamped for the machine hand-over, so these two travel in the loader script and the grids.
  const D1 = 'Spare Construct is a browser-based sketch and screening tool. It is not an approved regional transport model and it is not the Auckland Forecasting Centre Macro Strategic Model (MSM). Its results are indicative and are intended to scope, test and support a transport assessment. They must be reviewed, and where necessary confirmed with approved tools, by a suitably qualified transport planner or traffic engineer before they are relied on in any resource consent, plan change, notice of requirement, Fast-track application or funding decision.';
  const D12 = 'This export is prepared for use in SIDRA Intersection. Results produced in that tool, not the Spare Construct estimates, should be reported where the tool is required or accepted by the reviewing authority.';
  /* The E27 tier 1 marker (Compliance.supplementary: '' or 'Supplementary: outside E27.8.1(4A) matters of discretion'),
     shown above each junction table as a note; the numbers are unchanged */
  function suppText(P) {
    if (typeof Compliance === 'undefined' || typeof Compliance.supplementary !== 'function') return '';
    try { const P0 = P || (typeof Store !== 'undefined' ? Store.P : null); return P0 ? String(Compliance.supplementary(P0) || '') : ''; } catch { return ''; }
  }
  const suppNote = P => { const t = suppText(P); return t ? `<p class="note supp" data-supp="1"><b>${esc(t)}</b></p>` : ''; };
  const PERIODS = ['AM', 'PM'];   // the turning figures' 'AM (PM)' pair
  // Periods with results in a scenario, in the time-periods order (AM | IP | PM | SAT), so an enabled IP or SAT period
  // appears in every table and in the SIDRA export
  const ORDER = ['AM', 'IP', 'PM', 'SAT'];
  const periodsOf = sc => { const ks = Object.keys((sc && sc.res) || {}).filter(p => sc.res[p] && sc.res[p].nodes); return [...ORDER.filter(p => ks.includes(p)), ...ks.filter(p => !ORDER.includes(p)).sort()]; };
  const periodsOfAll = scs => { const ks = new Set(scs.flatMap(periodsOf)); return [...ORDER.filter(p => ks.has(p)), ...[...ks].filter(p => !ORDER.includes(p)).sort()]; };
  const perName = p => ({ AM: 'AM peak', PM: 'PM peak', IP: 'Inter-peak', SAT: 'Saturday peak' }[p] || p + ' period');
  const MV = ['L', 'T', 'R', 'U'];
  const TURN_ORDER = ['L3', 'L2', 'L1', 'T1', 'R1', 'R2', 'R3', 'U'];
  const DISC = { L: 'L', T: 'T', R: 'R', LT: 'L+T', TR: 'T+R', LTR: 'L+T+R' };
  const S = () => (Store.P ? Store.S : defaultSettings());
  const norm = b => ((b % 360) + 360) % 360;
  const rankOf = L => (CLASSES[L && L.cls] || CLASSES.local).rank;
  // View state of the workspace (not saved)
  const st = { view: 'summary', pick: null, period: null, against: null, swap: false, scope: 'auto', layer: 'total', merge: [] };

  /* ---------- formatting ---------- */
  const n0 = x => (x == null || !isFinite(x) ? '–' : U.fmt.int(x));
  const d1 = d => (d == null || !isFinite(d) ? '–' : d > 300 ? '> 300' : (+d).toFixed(1));
  const x2 = x => (x == null || !isFinite(x) ? '–' : (+x).toFixed(2));
  const p1 = x => (x == null || !isFinite(x) ? '–' : (+x).toFixed(1));
  const sgn = (x, u = '', dp = 1) => (x == null || !isFinite(x) || Math.abs(x) < Math.pow(10, -dp) / 2 ? '–' : `${x > 0 ? '+' : '−'}${Math.abs(x).toFixed(dp)}${u}`);
  const dosCol = (dos, xp) => (dos == null || xp == null ? '' : dos > 1 ? 'var(--danger)' : dos > xp ? 'var(--warn)' : 'var(--ok)');
  const dosCell = (dos, xp) => { const c = dosCol(dos, xp); return c ? `<span style="color:${c};font-weight:600">${x2(dos)}</span>` : x2(dos); };
  const rqCell = (rq, bay) => {
    if (rq == null || !isFinite(rq)) return '–';
    const T = STANDARDS.storage, c = rq > T.red ? 'var(--danger)' : rq > T.amber ? 'var(--warn)' : '';
    const t = rq > T.red ? (bay ? 'Queue overflows the bay' : 'Queue blocks the upstream junction') : rq > T.amber ? 'Queue fills most of the available storage' : '';
    return `<span${c ? ` style="color:${c};font-weight:600"` : ''}${t ? ` title="${t}"` : ''}>${rq.toFixed(2)}</span>`;
  };
  const r5 = v => { const s = STANDARDS.figure.step; return v > 0 && v < s / 2 ? '<5' : String(Math.round(v / s) * s); };
  const losText = l => (l && l !== '–' ? l : '–');
  const ctlName = t => (CONTROLS[t] || t || 'Uncontrolled');
  const resFooter = res => (res && res.meta && res.meta.status && res.meta.status.footer) ? `<p class="note">${esc(res.meta.status.footer)}</p>` : '';

  // Level of service from delay (and DoS) with the method's thresholds (Engine.STANDARDS.los)
  function losFrom(d, dos, method, type) {
    const L = Engine.STANDARDS.los[method] || Engine.STANDARDS.los.hcm6, th = type === 'signals' ? L.signals : type === 'roundabout' ? L.roundabout : L.sign;
    if (L.overCapacityF && dos > 1) return 'F';
    for (let i = 0; i < 5; i++) if (method === 'rta' ? d < th[i] : d <= th[i]) return 'ABCDE'[i];
    return 'F';
  }

  /* ---------- junctions in a result: sites and single controlled nodes ---------- */
  const centre = (sc, ids) => { const ns = ids.map(id => sc.nodes[id]).filter(Boolean); return ns.length ? [ns.reduce((s, n) => s + n.lat, 0) / ns.length, ns.reduce((s, n) => s + n.lng, 0) / ns.length] : null; };
  /* Entries { id, kind 'site'|'node', name, nr (res.nodes or site result), node (controller or rep), nodes, type, atSiteNo, ll }.
     opts.all adds uncontrolled junctions with turning traffic (nr null). */
  const entCache = new WeakMap();   // per result object; the key changes with every store edit
  function entries(sc, res, opts = {}) {
    if (!sc || !res || !res.nodes) return [];
    const key = `${sc.id}|${Store.ver || 0}|${Object.keys(sc.sites || {}).length}`, slot = opts.all ? 'all' : 'ctl';
    let c = entCache.get(res); if (!c) entCache.set(res, c = {});
    if (!c[slot] || c[slot].key !== key) c[slot] = { key, list: entriesOf(sc, res, opts) };
    return c[slot].list.slice();
  }
  function entriesOf(sc, res, opts) {
    const out = [], inSite = new Set();
    for (const s of Object.values(res.sites || {})) {
      s.nodes.forEach(n => inSite.add(n));
      const nr = s.one ? res.nodes[s.rep] : s.result;
      if ((!nr || !nr.appr || !nr.appr.length) && !opts.all) continue;
      const own = (sc.sites || {})[s.id];
      out.push({ id: s.id, kind: 'site', name: (own && own.name) || s.name || 'Unnamed junction', nr: nr && nr.appr ? nr : null, node: s.rep, nodes: s.nodes.slice(), type: (nr && nr.type) || s.type || 'none',
        atSiteNo: s.atSiteNo ?? (own ? own.atSiteNo : null), ll: centre(sc, s.nodes) });
    }
    for (const [id, nr] of Object.entries(res.nodes)) {
      if ((nr.site && res.sites && res.sites[nr.site]) || inSite.has(id) || !sc.nodes[id]) continue;
      out.push({ id, kind: 'node', name: Turns.junctionName(sc, id), nr, node: id, nodes: [id], type: nr.type, atSiteNo: sc.nodes[id].atSiteNo ?? null, ll: [sc.nodes[id].lat, sc.nodes[id].lng] });
    }
    if (opts.all) for (const id of Object.keys(res.turns || {})) {
      if (res.nodes[id] || inSite.has(id) || !sc.nodes[id]) continue;
      out.push({ id, kind: 'node', name: Turns.junctionName(sc, id), nr: null, node: id, nodes: [id], type: sc.nodes[id].control || 'none', atSiteNo: null, ll: [sc.nodes[id].lat, sc.nodes[id].lng] });
    }
    return out.sort((p, q) => p.name.localeCompare(q.name) || (p.id < q.id ? -1 : 1));
  }
  // The entry that shows a node: its site when it belongs to one in this result, else the node itself
  function entryFor(sc, res, id) {
    const all = entries(sc, res, { all: true });
    return all.find(e => e.id === id) || all.find(e => e.nodes.includes(id)) || null;
  }

  // Arms of a node coded with NZ / SIDRA legs (as Turns does for its rows): { inKey, outKey, b, road, o, leg, legLabel, link, conn, isIn, isOut }
  function nodeLegs(sc, res, nodeId) {
    const list = (res && res.turns && res.turns[nodeId]) || [];
    const A = Turns.arms(sc, nodeId, list);
    const pr = A.map(a => (a.conn ? 0 : a.link ? rankOf(a.link) : 1));
    Turns.assignLegs(A.map(a => a.b), pr).forEach((c, i) => Object.assign(A[i], { o: c.o, leg: Turns.LEG_CODES[c.o], legName: Turns.LEG_NAMES[c.o], legLabel: Turns.legLabel(c.o, A[i].road), inKeys: [A[i].inKey], outKeys: [A[i].outKey] }));
    return A;
  }
  function legsOf(sc, res, e) {
    if (e.kind === 'site' && res.sites && res.sites[e.id]) return Turns.siteRows(sc, res, e.id).A.map(a => ({ ...a, inKeys: a.ins, outKeys: a.outs }));
    return nodeLegs(sc, res, e.node);
  }
  const movesOf = (sc, res, e) => (e.kind === 'site' && res.sites && res.sites[e.id] ? Turns.siteRows(sc, res, e.id).rows : Turns.movements(sc, res, e.node));

  /* ---------- J1 Movement Summary ---------- */
  /* Distance back to the upstream stop line (sidra-tables L_avail): from the approach arc, walk upstream through
     uncontrolled nodes with only two roads (OSM splits roads mid-block) and add up the arc lengths, stopping at a
     junction, a controlled node (a signalised crossing has a stop line), a dead end or a one-way road the wrong way.
     It also passes the split where a two-way road divides into a one-way entry and exit at a splitter island (the
     exit leads back from this junction or a roundabout), even when that split node was given a give-way control.
     side: the junction's own nodes. */
  function upstreamLen(sc, res, key, side) {
    if (!key || key.startsWith('c:') || !res.arcs[key]) return null;
    const inArc = (M, u) => (M.b === u ? M.id + ':ab' : M.id + ':ba'), outArc = (M, u) => (M.a === u ? M.id + ':ab' : M.id + ':ba');
    const far = (M, u) => (M.a === u ? M.b : M.a);
    const own = n => (side && side.has(n)) || ((sc.nodes[n] || {}).control === 'roundabout');
    let L = 0, k = key;
    const seen = new Set();
    while (k && !seen.has(k) && seen.size < 500) {
      seen.add(k);
      const i = k.lastIndexOf(':'), lk = sc.links[k.slice(0, i)], A = res.arcs[k];
      if (!lk || !A) break;
      L += A.len || Net.len(sc, lk);
      const u = k.endsWith(':ab') ? lk.a : lk.b, nu = sc.nodes[u], ctl = nu ? nu.control || 'none' : null;
      if (!nu || (ctl !== 'none' && ctl !== 'priority') || (typeof Sites !== 'undefined' && Sites.of(sc, u))) break;
      const ls = Net.linksAt(sc, u).filter(M => M.status !== 'closed' && M.a !== M.b), others = ls.filter(M => M.id !== lk.id);
      let next = null;
      if (ctl === 'none' && ls.length === 2 && others.length === 1) next = others[0];
      else if (ls.length === 3 && others.length === 2 && !res.arcs[inArc(lk, u)]) {   // a splitter island: lk is the one-way entry
        const X = others.find(M => res.arcs[inArc(M, u)] && !res.arcs[outArc(M, u)] && own(far(M, u))), M = X && others.find(m => m !== X);
        if (M && res.arcs[inArc(M, u)] && res.arcs[outArc(M, u)]) next = M;
      }
      if (!next) break;
      k = inArc(next, u);
    }
    return L > 0 ? L : null;
  }
  // Storage for a movement: the bay when every lane it uses is a short lane, otherwise the distance to the upstream junction
  function storageOf(ap, mv, L) {
    const ls = (ap.lanes || []).filter(l => l.d && l.d.includes(mv));
    const bay = ls.length && ls.every(l => l.len > 0) ? Math.max(...ls.map(l => l.len)) : 0;
    return { storage: bay || L || null, bay: bay > 0 };
  }
  function j1(sc, res, id, e0) {
    const es = e0 ? null : entries(sc, res), e = e0 || es.find(x => x.id === id) || es.find(x => x.nodes.includes(id));
    if (!e || !e.nr) return null;
    const nr = e.nr, J = res.junction || {}, method = J.method || nr.method || 'hcm6', set = S();
    const legs = legsOf(sc, res, e), byIn = new Map();
    for (const a of legs) for (const k of a.inKeys || []) byIn.set(k, a);
    const mvs = movesOf(sc, res, e), groups = new Map(), side = new Set(e.nodes);
    for (const ap of nr.appr) {
      const a = byIn.get(ap.key), code = a ? a.o : 8 + groups.size;
      let g = groups.get(code);
      if (!g) groups.set(code, g = { o: code, leg: a ? a.leg : '', label: a ? a.legLabel : `${ap.approach || 'Approach'}: ${ap.name || 'Unnamed road'}`, road: a ? a.road : (ap.name || ''), appr: [] });
      const rows = mvs.filter(r => r.approach === ap.key), vT = rows.reduce((s, r) => s + r.v, 0), hvT = rows.reduce((s, r) => s + r.hv, 0);
      const hvPct = vT > 0 ? hvT / vT * 100 : +set.hvPct || 0, L = upstreamLen(sc, res, ap.key, side), free = ap.ctrl === 'free';
      const out = [];
      if (ap.mov) for (const m of ap.mov) {
        const rs = rows.filter(r => r.mv === m.mv), sv = rs.reduce((s, r) => s + r.v, 0), sh = rs.reduce((s, r) => s + r.hv, 0), so = storageOf(ap, m.mv, L);
        out.push({ ids: rs.map(r => r.id), turns: rs.map(r => r.turn), mv: m.mv, v: m.v, hvPct: sv > 0 ? sh / sv * 100 : hvPct, cap: m.cap, dos: m.dos, xp: ap.xp, delay: m.delay, los: m.los,
          q95veh: m.q95veh, q95: m.q95, psc: m.psc, storage: so.storage, bay: so.bay, rq: so.storage ? m.q95 / so.storage : null, full: true, twoStage: !!m.twoStage });
      } else for (const r of rows) out.push({ ids: [r.id], turns: [r.turn], mv: r.mv, v: r.v, hvPct: r.v > 0 ? r.hv / r.v * 100 : hvPct, full: false, toRoad: r.toRoad });
      g.appr.push({ ap, rows: out, sub: { v: ap.v, hvPct, cap: ap.cap, dos: ap.dos, xp: ap.xp, delay: ap.delay, los: free ? '–' : ap.los, q95veh: ap.q95veh, q95: ap.q95, psc: ap.psc,
        storage: L, rq: L && !free ? ap.q95 / L : null, free, key: ap.key, name: ap.name, capT: ap.capT, bay: ap.bay, rightBlock: ap.rightBlock, laneSrc: ap.laneSrc } });
    }
    const list = [...groups.values()].sort((p, q) => p.o - q.o);
    for (const g of list) if (g.appr.length > 1) {   // a leg with more than one entry (e.g. a slip lane): leg subtotal
      const A = g.appr.map(x => x.sub), ctl = A.filter(a => !a.free), v = A.reduce((s, a) => s + a.v, 0), cv = ctl.reduce((s, a) => s + a.v, 0);
      const dly = cv > 0 ? ctl.reduce((s, a) => s + a.v * a.delay, 0) / cv : 0, dos = Math.max(0, ...ctl.map(a => a.dos));
      g.leg_sub = { v, hvPct: v > 0 ? A.reduce((s, a) => s + a.v * a.hvPct, 0) / v : 0, cap: A.reduce((s, a) => s + (a.cap || 0), 0), dos, xp: A[0].xp, delay: dly, los: ctl.length ? losFrom(dly, dos, method, nr.type) : '–',
        q95veh: Math.max(0, ...A.map(a => a.q95veh || 0)), q95: Math.max(0, ...A.map(a => a.q95 || 0)), psc: ctl.length ? Math.min(...ctl.map(a => a.psc ?? Infinity)) : null };
    }
    const twsc = nr.intersectionLos === null || (nr.intersectionLos === undefined && (nr.type === 'priority' || nr.type === 'stop'));
    const subs = list.flatMap(g => g.appr.map(x => x.sub)), vAll = subs.reduce((s, a) => s + a.v, 0);
    const all = twsc ? null : { v: vAll, hvPct: vAll > 0 ? subs.reduce((s, a) => s + a.v * a.hvPct, 0) / vAll : 0, dos: nr.dos, xp: nr.xp ?? (subs[0] || {}).xp, delay: nr.avgDelay ?? nr.delay, los: nr.intersectionLos || nr.los,
      q95veh: Math.max(0, ...subs.map(a => a.q95veh || 0)), q95: Math.max(0, ...subs.map(a => a.q95 || 0)) };
    return { e, nr, groups: list, all, twsc, method, type: nr.type, period: res.period, year: res.year, stale: !!res.stale, movement: !!subs.length && nr.appr.some(a => a.mov), footer: footer(res), res };
  }
  function footer(res) {
    const J = (res && res.junction) || {}, ST = Engine.STANDARDS, L = ST.los[J.method] || ST.los.hcm6, mvm = J.capMethod === 'movement';
    const pff = J.pff != null ? Math.round(J.pff * 100) : 100, T = J.T != null ? J.T : 1, qs = J.queueSpace != null ? J.queueSpace : ST.queue.space;
    return `LoS method: ${L.name}; roundabout capacity: ${mvm ? 'HCM 6 lane-based entry equations (SIDRA Standard is geometry-sensitive and will differ)' : 'HCM 6 entry equation'}; give way and stop: ${mvm ? 'HCM 6 movement capacities (gap acceptance, rank impedance)' : 'gap acceptance against all major-road flow'}; signals Webster / Akcelik; PFF ${pff}%; analysis period ${T} h; queue space ${(+qs).toFixed(1)} m. ${LABEL}.`;
  }
  const J1_HEAD = ['Mov ID', 'Turn', 'Demand flow (veh/h)', 'HV (%)', 'Capacity (veh/h)', 'DoS (v/c)', 'Average delay (s)', 'LoS', '95% back of queue (veh)', '(m)', 'Practical spare capacity (%)', 'Storage ratio'];
  function j1Html(D, opts = {}) {
    if (!D) return '<p class="note">No junction results here. Run the model, then pick a junction with a give-way, stop, roundabout or signal control.</p>';
    const th = J1_HEAD.map((h, i) => `<th${i >= 2 ? ' class="n"' : ''}>${h}</th>`).join('');
    const cells = (r, lab, turn, bold) => {
      const td = (x, n = true) => `<td${n ? ' class="n"' : ''}>${x}</td>`;
      return `<tr${bold ? ' style="font-weight:600;background:var(--panel-2)"' : ''}>${td(lab, false)}${td(turn, false)}${td(n0(r.v))}${td(p1(r.hvPct))}${td(r.cap != null ? n0(r.cap) : '–')}${td(r.dos != null ? dosCell(r.dos, r.free ? null : r.xp) : '–')}`
        + `${td(r.delay != null ? d1(r.delay) : '–')}${td(r.los !== undefined ? Kit.losBadge(r.los) : '–', false)}${td(r.q95veh != null && !r.free ? p1(r.q95veh) : '–')}${td(r.q95 != null && !r.free ? n0(r.q95) : '–')}${td(r.psc != null ? sgn(r.psc, '', 0).replace('–', '0') : '–')}${td(r.rq !== undefined ? rqCell(r.rq, r.bay) : '–')}</tr>`;
    };
    const body = [];
    for (const g of D.groups) {
      body.push(`<tr><td colspan="12" style="font-weight:700;padding-top:8px">${esc(g.label)}</td></tr>`);
      for (const x of g.appr) {
        for (const r of x.rows) body.push(cells(r, esc(r.ids.join(', ') || '–'), esc(r.turns.join(', ') || r.mv), false));
        const s = x.sub, extra = g.appr.length > 1 ? ` (${esc(s.name || 'entry')})` : '';
        body.push(cells(s, 'Approach' + extra, s.free ? 'major road' : '', true));
        if (s.capT != null) body.push(`<tr><td colspan="12" class="note">Major road: through capacity ${n0(s.capT)} veh/h${s.bay ? ` with a ${n0(s.bay)} m right-turn bay` : ', no right-turn bay'}; right turners block through traffic ${p1((s.rightBlock || 0) * 100)}% of the time.</td></tr>`);
      }
      if (g.leg_sub) body.push(cells(g.leg_sub, 'Leg total', '', true));
    }
    if (D.all) body.push(cells(D.all, 'Intersection (all vehicles)', '', true));
    else body.push(`<tr><td colspan="12" class="note">Two-way sign control: no intersection level of service. Worst movement: ${esc(D.nr.losText || D.nr.los || '–')}.</td></tr>`);
    const head = opts.head === false ? '' : `<h4 style="margin:6px 0">${esc(D.e.name)} · ${esc(ctlName(D.type))} · ${esc(perName(D.period))} ${esc(D.year || '')}${D.stale ? ' <span class="pill stale">out of date</span>' : ''}</h4>`;
    const cyc = D.nr.C ? `<p class="note">Cycle time ${n0(D.nr.C)} s; green split ${(Array.isArray(D.nr.split) ? D.nr.split : []).map(s => n0(s * 100) + '%').join(' / ')}.</p>` : '';
    // a very short storage to a red queue usually means the next node is part of this junction (an ungrouped divided road)
    const short = D.groups.some(g => g.appr.some(x => x.sub.storage != null && x.sub.storage < 30 && x.sub.rq > STANDARDS.storage.red));
    const near = short ? '<p class="note">An approach has less than 30 m to the next junction node. If those nodes are one junction (a divided road or an offset T), group them into a site with Merge junctions or Find junction sites, so the queue is measured to the real upstream junction.</p>' : '';
    const lvl = D.movement ? '' : '<p class="note">Approach-level results: capacity, DoS, delay and queue are per approach. Switch to movement capacity in Settings › Junction analysis for per-movement rows.</p>';
    return `${head}${suppNote(opts.P)}<div class="tbl-wrap"><table class="t compact"><tr>${th}</tr>${body.join('')}</table></div>${cyc}${lvl}${near}
      <p class="note">${esc(D.footer)} Storage ratio: amber above ${STANDARDS.storage.amber}, red above ${STANDARDS.storage.red} (${STANDARDS.storage.basis}).</p>${resFooter(D.res)}`;
  }

  /* ---------- J2 summary and J3 comparison ---------- */
  // One period's summary of an entry: worst DoS, delay, LoS, longest Q95 and its leg
  function summary(sc, res, e) {
    if (!e || !e.nr) return null;
    const nr = e.nr, legs = legsOf(sc, res, e), byIn = new Map();
    for (const a of legs) for (const k of a.inKeys || []) byIn.set(k, a);
    let qa = null;
    for (const a of nr.appr) if (a.ctrl !== 'free' && (!qa || (a.q95 || 0) > (qa.q95 || 0))) qa = a;
    const twsc = nr.intersectionLos === null;
    return { dos: nr.dos, xp: nr.xp ?? (nr.appr.find(a => a.xp != null) || {}).xp, delay: nr.delay, delayBasis: nr.delayBasis || (twsc ? 'worst movement' : 'average'), los: nr.los, losText: nr.losText || nr.los, twsc,
      q95: qa ? qa.q95 || 0 : 0, q95Leg: qa ? (byIn.get(qa.key) ? byIn.get(qa.key).legLabel : `${qa.approach}: ${qa.name}`) : '', stale: !!res.stale };
  }
  const resOf = (sc, p) => (sc && sc.res && sc.res[p]) || null;
  function j2(sc, periods = periodsOf(sc)) {
    const by = new Map();
    for (const p of periods) {
      const res = resOf(sc, p); if (!res) continue;
      for (const e of entries(sc, res)) { let r = by.get(e.id); if (!r) by.set(e.id, r = { id: e.id, xid: xid(e), kind: e.kind, name: e.name, type: e.type, node: e.node, nodes: e.nodes.slice(), p: {} }); r.p[p] = summary(sc, res, e); }
    }
    return [...by.values()].sort((a, b) => Math.max(...Object.values(b.p).map(s => s ? s.delay : 0)) - Math.max(...Object.values(a.p).map(s => s ? s.delay : 0)));
  }
  const flyOf = r => (r.kind === 'site' ? `junction-site:${r.id}` : `node:${r.id}`);
  function j2Html(sc, periods = periodsOf(sc), opts = {}) {
    const rows = j2(sc, periods), ps = periods.filter(p => resOf(sc, p));
    if (!rows.length) return '<p class="empty">No controlled junctions with results. Set junction controls (give way, stop, roundabout or signals), then run the model.</p>';
    const grp = ps.map(p => `<th colspan="4" style="text-align:center">${esc(perName(p))} ${esc((resOf(sc, p) || {}).year || '')}${(resOf(sc, p) || {}).stale ? ' <span class="pill stale">out of date</span>' : ''}</th>`).join('');
    const sub = ps.map(() => '<th class="n">Worst DoS</th><th class="n">Delay (s)</th><th>LoS</th><th>Longest Q95 (m) and leg</th>').join('');
    const body = rows.map(r => `<tr class="click" ${opts.fly === false ? '' : `data-jx-fly="${esc(flyOf(r))}"`}><td>${esc(r.name)}${r.kind === 'site' ? ' <span class="pill">site</span>' : ''}</td><td>${esc(ctlName(r.type))}</td>${ps.map(p => {
      const s = r.p[p]; if (!s) return '<td class="n">–</td><td class="n">–</td><td>–</td><td>–</td>';
      return `<td class="n">${dosCell(s.dos, s.xp)}</td><td class="n">${d1(s.delay)}${s.twsc ? '†' : ''}</td><td>${Kit.losBadge(s.los)}${s.twsc ? '†' : ''}</td><td>${n0(s.q95)} m${s.q95Leg ? ` <span class="note">${esc(s.q95Leg)}</span>` : ''}</td>`;
    }).join('')}</tr>`).join('');
    return `${suppNote(opts.P)}<div class="tbl-wrap"><table class="t compact"><tr><th rowspan="2">Junction</th><th rowspan="2">Control</th>${grp}</tr><tr>${sub}</tr>${body}</table></div>
      <p class="note">Delay is the volume-weighted average control delay (signals, roundabouts). † Give way and stop: the worst movement, as there is no intersection level of service. DoS colours: green at or below practical DoS, amber above it, red over capacity. ${LABEL}.</p>`;
  }
  /* J3: the same layout with changes from scenario `from` to `to`. Junctions are matched by id, then by member nodes
     (site ids are random, so sites found separately in each scenario still match), then by the most shared nodes
     (a junction grouped differently in the two scenarios; flagged 'regrouped'). Exact matches are made for every
     junction first, so a regrouped neighbour cannot take a junction that has an exact partner. A junction left
     without a partner that still shares nodes with one in the other scenario (a node taken out of a site) is
     flagged 'regrouped' rather than 'new' or 'removed'. */
  function j3(from, to, periods = periodsOfAll([from, to])) {
    const A = j2(from, periods), B = j2(to, periods), rows = [], used = new Set(), pairs = [], got = new Map();
    const share = (a, b) => a.nodes.filter(n => b.nodes.includes(n)).length;
    for (const b of B) {
      const a = A.find(x => !used.has(x) && x.id === b.id) || A.find(x => !used.has(x) && x.xid === b.xid);
      if (a) { used.add(a); got.set(b, [a, '']); }
    }
    for (const b of B) {
      if (!got.has(b)) { let a = null, best = 0; for (const x of A) { const k = used.has(x) ? 0 : share(x, b); if (k > best) { best = k; a = x; } } if (a) used.add(a); got.set(b, [a, a || A.some(x => share(x, b)) ? 'regrouped' : '']); }
      pairs.push([got.get(b)[0], b, got.get(b)[1]]);
    }
    for (const a of A) if (!used.has(a)) pairs.push([a, null, B.some(y => share(a, y)) ? 'regrouped' : '']);
    for (const [a, b, how] of pairs) {
      const base = b || a, p = {};
      for (const per of periods) {
        const x = a && a.p[per], y = b && b.p[per];
        p[per] = { a: x || null, b: y || null, d: x && y ? { dos: y.dos - x.dos, dosText: sgn(y.dos - x.dos, '', 2), delay: y.delay - x.delay, delayText: sgn(y.delay - x.delay, ' s'), los: x.los === y.los ? '–' : `${x.los} to ${y.los}`, q95: y.q95 - x.q95, q95Text: sgn(y.q95 - x.q95, ' m', 0) } : null };
      }
      rows.push({ id: base.id, fromId: a ? a.id : null, kind: base.kind, name: base.name, type: base.type, only: how || (!a ? 'new' : !b ? 'gone' : ''), p });
    }
    const yA = periods.map(p => (resOf(from, p) || {}).year).find(Boolean), yB = periods.map(p => (resOf(to, p) || {}).year).find(Boolean);
    return { rows: rows.sort((p, q) => Math.max(0, ...periods.map(k => (q.p[k].d ? q.p[k].d.delay : 0))) - Math.max(0, ...periods.map(k => (p.p[k].d ? p.p[k].d.delay : 0)))),
      note: yA && yB && yA !== yB ? `The two scenarios are modelled for different years (${yA} and ${yB}), so the changes mix background growth with the scheme.` : '' };
  }
  function j3Html(from, to, periods = periodsOfAll([from, to].filter(Boolean)), opts = {}) {
    if (!from || !to) return '<p class="note">Add a second scenario and run both to compare junctions.</p>';
    const C = j3(from, to, periods), ps = periods.filter(p => resOf(from, p) && resOf(to, p));
    if (!ps.length) return `<p class="note">Run both ${esc(from.name)} and ${esc(to.name)} for the same peak to compare junctions.</p>`;
    const grp = ps.map(p => `<th colspan="4" style="text-align:center">${esc(perName(p))}</th>`).join(''), sub = ps.map(() => '<th class="n">Worst DoS</th><th class="n">Delay (s)</th><th>LoS</th><th class="n">Longest Q95 (m)</th>').join('');
    const cell = (x, f) => (x ? f(x) : '–');
    const pill = { new: ' <span class="pill">new</span>', gone: ' <span class="pill">removed</span>', regrouped: ' <span class="pill" title="Grouped into a junction site differently in the two scenarios">regrouped</span>' };
    const body = C.rows.map(r => `<tr class="click" data-jx-fly="${esc(flyOf(r))}"><td>${esc(r.name)}${pill[r.only] || ''}</td>${ps.map(p => {
      const c = r.p[p];
      return `<td class="n">${cell(c.a, s => x2(s.dos))} → ${cell(c.b, s => dosCell(s.dos, s.xp))}${c.d ? ` <span class="note">${c.d.dosText}</span>` : ''}</td><td class="n">${cell(c.a, s => d1(s.delay))} → ${cell(c.b, s => d1(s.delay))}${c.d ? ` <span class="note">${c.d.delayText}</span>` : ''}</td>`
        + `<td>${c.d ? esc(c.d.los) : '–'}</td><td class="n">${c.d ? c.d.q95Text : '–'}</td>`;
    }).join('')}</tr>`).join('');
    return `${suppNote(opts.P)}<div class="tbl-wrap"><table class="t compact"><tr><th rowspan="2">Junction</th>${grp}</tr><tr>${sub}</tr>${body}</table></div>
      ${C.note ? `<p class="note warn">${esc(C.note)}</p>` : ''}<p class="note">From ${esc(from.name)} to ${esc(to.name)}: each change is ${esc(to.name)} minus ${esc(from.name)}. LoS changes read 'B to D'; '–' means unchanged. ${LABEL}.</p>`;
  }

  /* ---------- flow-scale test (TTOC-06): junctions only, flows × factor, timings as run ---------- */
  const scaleCache = new Map();
  function flowScale(sc, res, f = STANDARDS.flowScale.factor, set = S()) {
    if (!sc || !res || !res.arcs || !res.nodes) return null;
    const key = `${sc.id}|${res.period}|${res.ts}|${f}|${Store.ver || 0}`;
    if (scaleCache.has(key)) return scaleCache.get(key);
    const G = Engine.buildGraph(sc, set), pf = Engine.pcuF(set), J = Engine.junctionSet(set), xt = new Float64Array(G.M);
    G.arcs.forEach((A, i) => { const r = res.arcs[A.key]; if (r) xt[i] = (r.pcu != null ? r.pcu : r.v * pf) * f; });
    let xT = null;
    if (G.capMethod === 'movement') {
      xT = new Float64Array(G.nT);
      for (const list of Object.values(res.turns || {})) for (const t of list) {
        const a = G.keyIdx.get(t.from), b = G.keyIdx.get(t.to); if (a == null || b == null) continue;
        const n = G.arcs[a].to; for (let k = G.head[n]; k < G.head[n + 1]; k++) if (G.out[k] === b) xT[G.tOff[a] + k - G.head[n]] = (t.pcu != null ? t.pcu : t.v * pf) * f;
      }
    }
    for (const ctl of G.ctl.values()) {   // keep the run's signal timings
      const nr = res.nodes[G.ids[ctl.n]];
      if (ctl.type === 'signals' && nr && nr.C && Array.isArray(nr.split)) { ctl.C = nr.C; ctl.split = nr.split.slice(); Engine.applySignal(ctl, G); }
    }
    G.mvDetail = true;
    // junction sites read the movements traced along the run's paths (res.siteTurns), scaled with the flows; without
    // them the engine falls back to chaining turn shares node by node, which invents U-turns on multi-node sites
    if (typeof Engine.siteFlows === 'function') { const sm = Engine.siteFlows(G, res, f); if (sm) G.smX = sm; }
    Engine.updateNodeCaps(G, xt, 1, xT);
    const nodes = {};
    for (const ctl of G.ctl.values()) {
      const arcsRes = {};
      for (const ai of ctl.arcs) { const A = G.arcs[ai], x = xt[ai], run = A.ff * (1 + set.bprAlpha * Math.pow(x / A.capMid, set.bprBeta)); arcsRes[A.key] = { v: x / pf, nd: Engine.arcTime(A, x, set) - run }; }
      nodes[G.ids[ctl.n]] = Engine.junctionResult(G, ctl, xt, arcsRes, set, J, pf);
    }
    const rows = [];
    for (const e of entries(sc, res)) {
      const el = nr => { const m = new Map(); if (!nr) return m; for (const a of nr.appr) { if (a.ctrl === 'free' && !a.mov) continue; if (a.mov) for (const mv of a.mov) m.set(a.key + '|' + mv.mv, { dos: mv.dos, xp: a.xp, key: a.key, mv: mv.mv }); else m.set(a.key, { dos: a.dos, xp: a.xp, key: a.key, mv: '' }); } return m; };
      const after = new Map(), before = el(e.nr);
      for (const id of e.nodes.concat(e.node)) for (const [k, v] of el(nodes[id])) if (before.has(k)) after.set(k, v);
      if (!before.size) continue;
      const legs = legsOf(sc, res, e), byIn = new Map(); for (const a of legs) for (const k of a.inKeys || []) byIn.set(k, a);
      const name = x => { const a = byIn.get(x.key); return (a ? a.legLabel : x.key) + (x.mv ? ` (${x.mv})` : ''); };
      let w0 = 0, w1 = 0, first = null, already = null;
      for (const [k, b] of before) {
        const a = after.get(k) || b; w0 = Math.max(w0, b.dos); w1 = Math.max(w1, a.dos);
        if (b.xp != null && b.dos > b.xp) { if (!already || b.dos > already.dos) already = { ...b, where: name(b) }; }
        else if (b.xp != null && a.dos > a.xp && (!first || a.dos > first.dos)) first = { ...a, before: b.dos, where: name(a) };
      }
      const xp = (e.nr.appr.find(a => a.xp != null) || {}).xp ?? null;
      rows.push({ id: e.id, kind: e.kind, name: e.name, type: e.type, dos: U.r3(w0), dosF: U.r3(w1), xp, crosses: !!first, first, already,
        text: first ? `Crosses practical DoS at ${first.where}: ${x2(first.before)} → ${x2(first.dos)}${already ? `; already above it at ${already.where} (${x2(already.dos)})` : ''}` : already ? `Already above practical DoS at ${already.where} (${x2(already.dos)})` : 'Stays at or below practical DoS' });
    }
    rows.sort((p, q) => (q.crosses - p.crosses) || (!!q.already - !!p.already) || q.dosF - p.dosF);
    const out = { factor: f, period: res.period, rows, crossing: rows.filter(r => r.crosses), basis: `${STANDARDS.flowScale.name}: ${STANDARDS.flowScale.edition}. Junctions re-evaluated at flows × ${f.toFixed(2)} with the run's signal timings; routes are not re-assigned.` };
    scaleCache.set(key, out); if (scaleCache.size > 12) scaleCache.delete(scaleCache.keys().next().value);
    return out;
  }
  function scaleHtml(sc, res, opts = {}) {
    if (!res) return '<p class="note">Run the model to test junctions at higher flows.</p>';
    const R = flowScale(sc, res);
    if (!R || !R.rows.length) return '<p class="note">No controlled junctions to test.</p>';
    const rows = R.rows.map(r => ({ cells: [esc(r.name), esc(ctlName(r.type)), x2(r.dos), dosCell(r.dosF, r.xp), x2(r.xp), r.crosses ? `<b style="color:var(--warn)">${esc(r.text)}</b>` : r.already ? `<span style="color:var(--danger)">${esc(r.text)}</span>` : esc(r.text)], fly: flyOf(r) }));
    return `${suppNote(opts.P)}<p>${R.crossing.length ? `<b>${R.crossing.length} junction${R.crossing.length === 1 ? '' : 's'}</b> cross practical DoS when flows rise by ${Math.round((R.factor - 1) * 100)}%.` : `No junction crosses practical DoS when flows rise by ${Math.round((R.factor - 1) * 100)}%.`}</p>
      ${Kit.table({ head: ['Junction', 'Control', { label: `Worst DoS (${esc(res.period)})`, n: true }, { label: `At +${Math.round((R.factor - 1) * 100)}%`, n: true }, { label: 'Practical DoS', n: true }, 'Result'], rows }).replace(/data-fly=/g, 'data-jx-fly=')}
      <p class="note">${esc(R.basis)} ${LABEL}.</p>`;
  }

  /* ---------- turn checks (turn-volumes) ---------- */
  function checks(sc, res, e) {
    const mvs = movesOf(sc, res, e), legs = legsOf(sc, res, e), bad = [];
    // a mid-block signal (two roads) records no turns: nothing to balance
    if (mvs.length) for (const a of legs) for (const k of a.inKeys || []) {
      if (k.startsWith('c:') || !res.arcs[k]) continue;
      const t = mvs.filter(r => r.approach === k).reduce((s, r) => s + r.v, 0), v = res.arcs[k].v;
      if (Math.abs(t - v) > STANDARDS.balance.tol) bad.push({ key: k, leg: a.legLabel, turns: U.r1(t), arc: v });
    }
    return { balanced: !bad.length, bad, uTurns: mvs.filter(r => r.mv === 'U' && r.v > STANDARDS.uTurn.vph).map(r => ({ id: r.id, v: r.v, road: r.road })) };
  }

  /* ---------- CSVs ---------- */
  const scList = (P, which) => (which ? [].concat(which) : P.scenarios);
  // junction_movements.csv (docs/spec/sidra-tables)
  const JM_HEAD = ['scenario_id', 'year', 'period', 'site_id', 'junction', 'control', 'cycle_s', 'pff', 'approach_leg', 'approach_road', 'movement', 'demand_veh_h', 'hv_pct', 'capacity_veh_h', 'dos', 'practical_dos',
    'psc_pct', 'delay_s', 'los', 'los_method', 'q95_veh', 'q95_m', 'queue_space_m', 'storage_m', 'exceeds_storage'];
  function junctionMovementsCSV(P = Store.P, which, periods) {
    const out = [JM_HEAD];
    for (const sc of scList(P, which)) for (const p of periods || periodsOf(sc)) {
      const res = resOf(sc, p); if (!res) continue;
      const J = res.junction || {};
      for (const e of entries(sc, res)) {
        const D = j1(sc, res, e.id, e); if (!D) continue;
        const base = [sc.id, res.year, p, xid(e), e.name, e.type, D.nr.C || '', J.pff ?? ''];
        const row = (g, mv, r) => out.push([...base, g.leg, g.road, mv, r.v != null ? Math.round(r.v) : '', r.hvPct != null ? U.r1(r.hvPct) : '', r.cap != null ? Math.round(r.cap) : '', r.dos != null ? U.r3(r.dos) : '',
          r.xp ?? '', r.psc ?? '', r.delay != null ? U.r1(r.delay) : '', r.los && r.los !== '–' ? r.los : '', J.method || D.method, r.q95veh ?? '', r.q95 ?? '', J.queueSpace ?? '', r.storage != null ? Math.round(r.storage) : '', r.rq != null ? (r.rq > STANDARDS.storage.red ? 'yes' : 'no') : '']);
        for (const g of D.groups) for (const x of g.appr) { for (const r of x.rows) row(g, r.turns.join('+') || r.mv, r); row(g, 'APPROACH', x.sub); }
        if (D.all) out.push([...base, '', '', 'ALL', Math.round(D.all.v), U.r1(D.all.hvPct), '', U.r3(D.all.dos), D.all.xp ?? '', '', U.r1(D.all.delay), D.all.los, J.method || D.method, D.all.q95veh, D.all.q95, J.queueSpace ?? '', '', '']);
      }
    }
    return U.csv(out);
  }
  /* turning_movements.csv (docs/spec/turn-volumes): every junction with turning traffic; site movements for sites.
     Turn-volume exports need gap ≤ 1e-4: an unconverged run throws its reason unless opts.force, and a forced file
     gains a last column, export_note, that says NOT CONVERGED on every row from such a run. */
  const TM_HEAD = ['scenario_id', 'period', 'year', 'site_id', 'site_name', 'at_site_no', 'lat', 'lng', 'control', 'from_leg', 'from_road', 'to_leg', 'to_road', 'turn', 'lv_veh_h', 'hv_veh_h', 'total_veh_h', 'hv_pct', 'pcu_h'];
  const split = r => { const t = Math.round(r.v), hv = Math.min(t, Math.round(r.hv)); return [t - hv, hv, t]; };
  function turningMovementsCSV(P = Store.P, which, periods, opts = {}) {
    const runs = [];
    for (const sc of scList(P, which)) for (const p of periods || periodsOf(sc)) { const res = resOf(sc, p); if (res) runs.push([sc, p, res]); }
    const bad = runs.filter(([, , res]) => !Engine.exportReady(res).ok);
    if (bad.length && !opts.force) { const [sc, p, res] = bad[0]; throw new Error(`${sc.name || sc.id}, ${perName(p)}: ${Engine.exportReady(res).why}`); }
    const out = [bad.length ? [...TM_HEAD, 'export_note'] : TM_HEAD];
    for (const [sc, p, res] of runs) {
      const nc = bad.length ? [ncText(res)] : [];
      for (const e of entries(sc, res, { all: true })) for (const m of movesOf(sc, res, e)) {
        const [lv, hv, t] = split(m);
        out.push([sc.id, p, res.year, xid(e), e.name, e.atSiteNo ?? '', e.ll ? U.r7(e.ll[0]) : '', e.ll ? U.r7(e.ll[1]) : '', e.type, m.leg, m.road, m.toLeg, m.toRoad, m.turn, lv, hv, t, t > 0 ? U.r1(hv / t * 100) : 0, U.r1(m.pcu), ...nc]);
      }
    }
    return U.csv(out);
  }
  // One junction's turning count (Turns.turnCountCSV); a run short of the turn-volume export gap starts with a
  // 'Convergence, NOT CONVERGED …' row, so the file says so wherever it goes. { text, nc }
  function turnCountFile(sc, res, nodeId) {
    const nc = res ? ncText(res) : '', text = Turns.turnCountCSV(sc, res, nodeId);
    return { text: nc ? `${U.csv([['Convergence', nc]])}\r\n${text}` : text, nc };
  }

  /* ---------- SIDRA hand-off (docs/spec/sidra-export) ---------- */
  // Which junctions go to SIDRA: ids given, else scope 'auto' (sites from the extent test, else every controlled junction), 'controlled' or 'all'
  function targets(sc, res, ids, scope = 'auto') {
    const all = entries(sc, res, { all: true });
    if (ids && ids.length) return all.filter(e => ids.includes(e.id) || e.nodes.some(n => ids.includes(n)));
    const ext = sc.extent && Array.isArray(sc.extent.sites) && sc.extent.sites.length ? sc.extent.sites : null;
    if (scope === 'auto' && ext) return all.filter(e => ext.includes(e.id) || e.nodes.some(n => ext.includes(n)));
    return scope === 'all' ? all : all.filter(e => e.nr || e.type !== 'none');
  }
  const VOL_HEAD = ['site_id', 'site_name', 'at_site_no', 'lat', 'lng', 'site_type', 'period', 'origin_leg', 'dest_leg', 'turn', 'lv_veh_h', 'hv_veh_h', 'pff_pct', 'unit_time_min', 'peak_flow_period_min', 'growth_pct'];
  function volumeRows(sc, resList, ids, opts = {}) {
    const rows = [], X = STANDARDS.sidra;
    for (const res of [].concat(resList || []).filter(Boolean)) {
      const pff0 = Math.round(((res.junction && res.junction.pff) || (S().junction || {}).pff || 1) * 100);
      const nc = ncText(res);
      for (const e of targets(sc, res, ids, opts.scope)) {
        /* pff_pct: 100 × PHF of a turning count at this junction in this period (calibration lane), else the model's
           PFF. Calibration.phf is a 15-minute factor (V60 ÷ 4·V15) and SIDRA applies the PFF over its Peak Flow Period,
           so a count PHF goes with a 15-minute period; the model's PFF keeps SIDRA's default 30 minutes. */
        const phf = phfOf(sc, e, res.period), pff = phf ? Math.round(phf * 100) : pff0;
        for (const m of movesOf(sc, res, e)) {
          const [lv, hv, t] = split(m); if (!t) continue;
          rows.push({ site_id: xid(e), site_name: e.name, at_site_no: e.atSiteNo ?? '', lat: e.ll ? U.r7(e.ll[0]) : '', lng: e.ll ? U.r7(e.ll[1]) : '', site_type: X.siteType[e.type] ?? 2, period: res.period,
            origin_leg: Turns.LEG_CODES.indexOf(m.leg), dest_leg: Turns.LEG_CODES.indexOf(m.toLeg), turn: m.turn, lv_veh_h: lv, hv_veh_h: hv, pff_pct: pff, unit_time_min: X.unitTime, peak_flow_period_min: phf ? X.phfPeriod : X.peakFlowPeriod, growth_pct: X.growthPct,
            export_note: nc, _from: m.legLabel, _to: Turns.legLabel(Turns.LEG_CODES.indexOf(m.toLeg), m.toRoad), _year: res.year, _pffSrc: phf ? 'turning count PHF' : 'model setting', _id: e.id });
        }
      }
    }
    return rows;
  }
  // Peak hour factor from an imported turning count (Calibration.phf), when one exists for any node of the junction
  function phfOf(sc, e, period) {
    if (typeof Calibration === 'undefined' || typeof Calibration.phf !== 'function') return null;
    for (const id of e.nodes) { let f = null; try { f = Calibration.phf(id, period, sc); } catch { f = null; } if (f > 0.25 && f <= 1) return f; }
    return null;
  }
  // The export gate (Engine.exportReady via Turns.sidraExport): { ok, why }
  function gate(sc, resList) {
    const r = Turns.sidraExport(sc, [].concat(resList || []).filter(Boolean), [], {});
    return { ok: r.ok, why: r.why };
  }
  const bothRes = sc => periodsOf(sc).map(p => resOf(sc, p));
  // A forced export of an unconverged run gains a last column, export_note, saying NOT CONVERGED on its rows
  function volumesCSV(sc, resList = bothRes(sc), ids, opts = {}) {
    if (!opts.force) { const g = gate(sc, resList); if (!g.ok) throw new Error(g.why); }
    const rows = volumeRows(sc, resList, ids, opts), head = rows.some(r => r.export_note) ? [...VOL_HEAD, 'export_note'] : VOL_HEAD;
    return U.csv([head, ...rows.map(r => head.map(k => r[k]))]);
  }
  /* The volumes CSV in Spare Construct's turning-count layout (long: site_id, lat, lng, time_start, interval_min, from_leg,
     movement, class, count), so it re-imports through the calibration panel (Calibration.parseTurnCSV) as a one-hour
     count at the start of each period's peak-hour window. Accepts the CSV text or volumeRows. */
  const COUNT_HEAD = ['site_id', 'site_name', 'lat', 'lng', 'date', 'time_start', 'interval_min', 'from_leg', 'movement', 'class', 'count'];
  const WIN0 = { AM: '07:00', IP: '11:00', PM: '16:00', SAT: '11:00' };
  function asCountCSV(src, date = '') {
    const rows = typeof src === 'string' ? parseVolumesCSV(src) : src, W = (S().calib && S().calib.windows) || {}, out = [COUNT_HEAD];
    for (const r of rows) for (const [cls, k] of [['LV', 'lv_veh_h'], ['HV', 'hv_veh_h']]) {
      if (!(r[k] > 0)) continue;
      out.push([r.site_id, r.site_name, r.lat, r.lng, date, (W[r.period] && W[r.period][0]) || WIN0[r.period] || '07:00', 60, Turns.LEG_CODES[r.origin_leg], r.turn, cls, r[k]]);
    }
    return U.csv(out);
  }
  // Reads the volumes CSV back (e.g. as turning counts): rows with numbers where the column is numeric
  function parseVolumesCSV(text) {
    const rows = U.parseCSV(String(text || '')), head = rows.shift() || [], num = new Set(['lat', 'lng', 'site_type', 'origin_leg', 'dest_leg', 'lv_veh_h', 'hv_veh_h', 'pff_pct', 'unit_time_min', 'peak_flow_period_min', 'growth_pct']);
    return rows.map(r => Object.fromEntries(head.map((h, i) => [h, num.has(h) && r[i] !== '' ? +r[i] : r[i] ?? ''])));
  }
  // Lanes on each leg's approach (1–3), so the loader can add legs with roughly the right number of lanes
  function legLanes(sc, res, e) {
    const out = {};
    for (const a of legsOf(sc, res, e)) {
      let n = 1;
      for (const k of a.inKeys || []) { const i = k.lastIndexOf(':'), L = sc.links[k.slice(0, i)]; if (L) n = Math.max(n, (k.endsWith(':ab') ? L.lanesAB : L.lanesBA) | 0); }
      out[a.o] = Math.max(1, Math.min(3, n));
    }
    return out;
  }
  // load_corridor_volumes.py: a SIDRA INTERSECTION 11 .NET API loader (pythonnet), NZ setup, with --dry-run
  function loaderPy(sc, resList = bothRes(sc), ids, opts = {}) {
    if (!opts.force) { const g = gate(sc, resList); if (!g.ok) throw new Error(g.why); }
    const rows = volumeRows(sc, resList, ids, opts).map(r => Object.fromEntries(VOL_HEAD.map(k => [k, r[k]])));
    const list = [].concat(resList || []).filter(Boolean), legs = {};
    for (const res of list) for (const e of targets(sc, res, ids, opts.scope)) legs[xid(e)] = { ...(legs[xid(e)] || {}), ...legLanes(sc, res, e) };
    // JSON inside a Python raw string: no ''' can occur because ' is written as \u0027
    const js = x => JSON.stringify(x).replace(/'/g, '\\u0027').replace(/[\u007f-\uffff]/g, c => '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0'));
    const pyStr = s => JSON.stringify(String(s)).replace(/[\u007f-\uffff]/g, c => '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0'));
    // Project data in a comment: one line, no control characters, so nothing from a shared project file can become code
    const cm = s => String(s ?? '').replace(/[\u0000-\u001f\u007f-\u009f\u2028\u2029]/g, ' ');
    const yr = list.map(r => r.year).find(y => /^\d{4}$/.test(String(y))) || '', d0 = new Date(), when = `${d0.getFullYear()}-${String(d0.getMonth() + 1).padStart(2, '0')}-${String(d0.getDate()).padStart(2, '0')}`;
    const sites = new Set(rows.map(r => r.site_id)).size, periods = [...new Set(rows.map(r => r.period))].filter(p => /^\w{1,12}$/.test(String(p))).join(', ') || 'none';
    const nc = opts.force ? [...new Set(list.map(r => ncText(r) && `${perName(r.period)}: ${ncText(r)}`).filter(Boolean))].join('; ') : '';
    // D1 and D12 as a comment block (wrapped, '# ' lines) and as a string the script prints once when it runs
    const wrapC = (s, w = 112) => { const out = []; let line = ''; for (const word of String(s).split(/\s+/)) { if (line && line.length + word.length + 1 > w) { out.push(line); line = word; } else line = line ? line + ' ' + word : word; } if (line) out.push(line); return out.map(l => '#   ' + l).join('\n'); };
    const notice = `D1 Screening status: ${D1}\nD12 Hand-over: ${D12}`;
    return `#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# Spare Construct to SIDRA INTERSECTION 11 volume loader (New Zealand setup)
#
# D1 Screening status
${wrapC(D1)}
# D12 Hand-over
${wrapC(D12)}
#
# Requires licensed SIDRA 11, 64-bit Python, pythonnet, .NET 8; close the project in SIDRA first; untested against a live install.
#${nc ? `\n# ${cm(nc)}\n# These volumes come from a run that has not converged far enough for SIDRA. Do not rely on them.\n#` : ''}
# Made by Spare Construct on ${when} for scenario ${cm(sc.name || sc.id)} (${yr}); periods ${cm(periods)}; ${rows.length} movements at ${sites} sites.
# Volumes are modelled demand in vehicles per hour (light and heavy vehicles), not PCU and not throughput, from a
# Spare Construct sketch model. Spare Construct junction results are HCM 6 / Akcelik-based, indicative and not SIDRA output.
#
# Usage
#   py -3 load_corridor_volumes.py --dry-run                print the planned SIDRA API calls; SIDRA is not needed
#   py -3 load_corridor_volumes.py PROJECT.sipx             add or update sites in an existing SIDRA project
#   py -3 load_corridor_volumes.py PROJECT.sipx --create    create the project first
#   --csv corridor-sidra-volumes.csv                        read the volumes from the CSV instead of the copy below
#
# Update mode: a site in the same folder whose Intersection ID is the Spare Construct site id keeps its lane and phasing
# coding. Its volumes, peak flow factors, unit time for volumes and peak flow period are overwritten; every movement
# between its legs is set to 0 first, so a movement with no traffic now (a banned turn, a changed network) does not
# keep an old volume. New sites get one leg per Spare Construct leg with default NZ geometry, which you must check and code.
# A peak flow factor from a turning count's 15-minute peak hour factor comes with a 15-minute peak flow period.
# Leg and turn codes follow SIDRA drive-on-left conventions: legs S 0, SE 1, E 2, NE 3, N 4, NW 5, W 6, SW 7
# (named by where traffic comes from); turns U, L3, L2, L1, T1, R1, R2, R3.
# Volumes already include growth to the scenario year, so growth_pct is 0.

import argparse
import csv
import json
import os
import sys

SITE_TYPES = {0: 'signals', 1: 'roundabout', 2: 'give-way', 3: 'two-way stop'}
NZ_SETUP = ${STANDARDS.sidra.setup}  # SIDRA site standard: 2 = New Zealand
SCENARIO = ${pyStr(sc.name || sc.id)}
NOT_CONVERGED = ${pyStr(nc)}
NOTICE = ${pyStr(notice)}
DATA =json.loads(r'''${js(rows)}''')
LEG_LANES = json.loads(r'''${js(legs)}''')
NUMERIC = ('lat', 'lng', 'site_type', 'origin_leg', 'dest_leg', 'lv_veh_h', 'hv_veh_h', 'pff_pct', 'unit_time_min', 'peak_flow_period_min', 'growth_pct')
RETURNS = {'OpenProject': 'project', 'CreateAndOpenProject': 'project', 'AddSiteFolder': 'folder', 'AddSite': 'site'}


class DryRun(object):
    """Stands in for the SIDRA API: prints each call and assignment instead of making it."""

    def __init__(self, name):
        object.__setattr__(self, '_name', name)

    def __getattr__(self, attr):
        return DryRun(self._name + '.' + attr)

    def __setattr__(self, attr, value):
        print('%s.%s = %r' % (self._name, attr, value))

    def __call__(self, *args):
        print('%s(%s)' % (self._name, ', '.join(repr(a) for a in args)))
        return DryRun(RETURNS.get(self._name.split('.')[-1], 'result'))

    def __getitem__(self, key):
        k = ', '.join(repr(x) for x in key) if isinstance(key, tuple) else repr(key)
        return DryRun('%s[%s]' % (self._name, k))


def read_csv(path):
    with open(path, newline='', encoding='utf-8-sig') as f:
        rows = list(csv.DictReader(f))
    for r in rows:
        for k in NUMERIC:
            if r.get(k, '') != '':
                r[k] = float(r[k]) if k in ('lat', 'lng') else int(float(r[k]))
    return rows


def group(rows):
    """One SIDRA site per Spare Construct site and period, with its legs and movements."""
    sites = {}
    for r in rows:
        key = (r['period'], str(r['site_id']))
        s = sites.get(key)
        if s is None:
            s = sites[key] = {'row': r, 'legs': set(), 'moves': []}
        s['legs'].add(int(r['origin_leg']))
        s['legs'].add(int(r['dest_leg']))
        s['moves'].append(r)
    return sites


def load_api():
    if sys.maxsize <= 2 ** 32:
        sys.exit('Use 64-bit Python: SIDRA 11 is a 64-bit application.')
    try:
        import winreg
        key = winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r'SOFTWARE\\SIDRA SOLUTIONS\\SIDRA INTERSECTION 11')
        loc = winreg.QueryValueEx(key, 'InstallLocation')[0]
    except OSError:
        sys.exit('SIDRA INTERSECTION 11 was not found in the registry. Install SIDRA 11, or run with --dry-run.')
    try:
        from pythonnet import set_runtime
        from clr_loader import get_coreclr
        set_runtime(get_coreclr(runtime_config=os.path.join(loc, 'SIDRASolutions.SI.API.runtimeconfig.json')))
        import clr
    except ImportError:
        sys.exit('pythonnet is missing. Install it with: py -3 -m pip install pythonnet')
    sys.path.append(loc)
    clr.AddReference(os.path.join(loc, 'SIDRASolutions.SI.API.dll'))
    from SIDRASolutions.SI.API import SIAPI
    return SIAPI()


def existing_sites(project):
    """Sites already in the project, keyed by (folder name, Intersection ID)."""
    found, folders = {}, {}
    if isinstance(project, DryRun):
        return found, folders
    for folder in getattr(project, 'SiteFolders', None) or []:
        folders[str(folder.Name)] = folder
        for site in getattr(folder, 'Sites', None) or []:
            found[(str(folder.Name), str(site.Intersectionid))] = site
    return found, folders


def set_volumes(site, moves):
    for m in moves:
        od = site.MovementVehicleODs[int(m['origin_leg']), int(m['dest_leg'])]
        od.MovementVehicleODMCs[1].Volume = int(m['lv_veh_h'])
        od.MovementVehicleODMCs[2].Volume = int(m['hv_veh_h'])
        od.Peak_flow_factor = int(m['pff_pct'])


def site_legs(site_id, legs=()):
    return sorted(set(int(k) for k in LEG_LANES.get(site_id, {})) | set(int(x) for x in legs))


def clear_volumes(site, legs):
    """Update mode: set every movement between the site's legs to 0 before the new volumes are written."""
    for o in legs:
        for d in legs:
            try:
                od = site.MovementVehicleODs[o, d]
            except Exception:  # no such movement on this site
                continue
            if od is None:
                continue
            for mc in (1, 2):
                try:
                    od.MovementVehicleODMCs[mc].Volume = 0
                except Exception:
                    pass


def main(argv=None, api=None):
    ap = argparse.ArgumentParser(description='Load Spare Construct turning volumes into a SIDRA INTERSECTION 11 project (New Zealand setup).')
    ap.add_argument('project', nargs='?', help='the .sipx project file')
    ap.add_argument('--create', action='store_true', help='create the project first')
    ap.add_argument('--csv', help='read volumes from this CSV (corridor-sidra-volumes.csv) instead of the embedded copy')
    ap.add_argument('--dry-run', action='store_true', help='print the planned SIDRA API calls without SIDRA')
    a = ap.parse_args(argv)
    print(NOTICE)
    rows = read_csv(a.csv) if a.csv else DATA
    if not rows:
        print('No volumes to load. Export again from Spare Construct after running the model.')
        return 1
    if not a.project and not a.dry_run:
        ap.error('give the .sipx project file, or --dry-run')
    if NOT_CONVERGED:
        print('WARNING: ' + NOT_CONVERGED + '. These volumes are not fit to rely on.')
    path = a.project or 'corridor.sipx'
    if api is None:
        api = DryRun('api') if a.dry_run else load_api()
    project = api.CreateAndOpenProject(path, 'Spare Construct ' + SCENARIO) if a.create else api.OpenProject(path)
    if project is None:
        project = getattr(api, 'Project', None)
    if project is None:
        sys.exit('SIDRA could not open %s. Close it in SIDRA and check the path.' % path)
    found, folders = existing_sites(project)
    made = updated = cleared = 0
    groups = group(rows)
    for (period, site_id), s in sorted(groups.items()):
        r = s['row']
        fname = 'Spare Construct %s %s' % (SCENARIO, period)
        folder = folders.get(fname)
        if folder is None:
            folder = folders[fname] = project.AddSiteFolder(fname)
        site = found.get((fname, site_id))
        if site is None:
            site = folder.AddSite(int(r['site_type']), NZ_SETUP)
            site.Name = '%s (%s)' % (r['site_name'], period)
            site.Intersectionid = site_id
            try:
                site.MapCoordinate.Latitude = float(r['lat'])
                site.MapCoordinate.Longitude = float(r['lng'])
            except Exception as e:  # MapCoordinate may be a value type in the API
                print('Could not set the map position of %s: %s' % (site_id, e))
            lanes = LEG_LANES.get(site_id, {})
            for leg in sorted(s['legs']):
                site.AddLegWithLanes(leg, int(lanes.get(str(leg), 1)))
            site.UpdateGeometryData()
            made += 1
        else:
            clear_volumes(site, site_legs(site_id, s['legs']))
            updated += 1
        site.Unittimeforvolumes = int(r['unit_time_min'])
        site.Peakflowperiod = int(r['peak_flow_period_min'])
        set_volumes(site, s['moves'])
    # a site in this export with no traffic at all in a period: clear its old volumes too
    prefix, periods = 'Spare Construct %s ' % SCENARIO, set(str(r['period']) for r in rows)
    for (fname, site_id), site in sorted(found.items(), key=lambda kv: kv[0]):
        period = fname[len(prefix):] if fname.startswith(prefix) else None
        if period in periods and site_id in LEG_LANES and (period, site_id) not in groups:
            clear_volumes(site, site_legs(site_id))
            cleared += 1
    api.SaveProject()
    print('Done: %d sites created, %d updated and %d set to no traffic in %s. Lanes and phasing of existing sites were kept.' % (made, updated, cleared, path))
    return 0


if __name__ == '__main__':
    sys.exit(main())
`;
  }
  // Printable grids per site and period: rows = legs; columns = turn × class (LV | HV)
  function gridsHtml(sc, resList = bothRes(sc), ids, opts = {}) {
    const rows = volumeRows(sc, resList, ids, opts), by = new Map(), list = [].concat(resList || []).filter(Boolean);
    for (const r of rows) { const k = r.site_id + '|' + r.period; let g = by.get(k); if (!g) by.set(k, g = { r, rows: [] }); g.rows.push(r); }
    const g0 = gate(sc, list), when = new Date().toLocaleDateString('en-NZ', { day: 'numeric', month: 'long', year: 'numeric' });
    const parts = [...by.values()].map(({ r, rows: rs }) => {
      const turns = TURN_ORDER.filter(t => rs.some(x => x.turn === t)), legs = [...new Set(rs.map(x => x.origin_leg))].sort((a, b) => a - b);
      const cell = (o, t, k) => { const x = rs.filter(y => y.origin_leg === o && y.turn === t).reduce((s, y) => s + y[k], 0); return x ? x : ''; };
      const body = legs.map(o => { const lab = (rs.find(x => x.origin_leg === o) || {})._from || Turns.LEG_NAMES[o];
        const tot = rs.filter(x => x.origin_leg === o).reduce((s, x) => s + x.lv_veh_h + x.hv_veh_h, 0);
        return `<tr><th>${esc(lab)}</th>${turns.map(t => `<td>${cell(o, t, 'lv_veh_h')}</td><td>${cell(o, t, 'hv_veh_h')}</td>`).join('')}<td><b>${tot}</b></td></tr>`; }).join('');
      const tot = rs.reduce((s, x) => s + x.lv_veh_h + x.hv_veh_h, 0);
      return `<section><h2>${esc(r.site_name)} · ${esc(r.period === 'IP' ? 'inter-peak average hour' : perName(r.period) + ' hour')}${r._year ? ' ' + esc(r._year) : ''}</h2>
        <p class="meta">Site ${esc(r.site_id)}${r.at_site_no !== '' ? ` · AT site ${esc(r.at_site_no)}` : ''} · ${esc(STANDARDS.sidra.typeName[r.site_type] || 'give-way')} · PFF ${esc(r.pff_pct)}% over ${esc(r.peak_flow_period_min)} min (${esc(r._pffSrc || 'model setting')}) · vehicles per hour</p>
        <table><tr><th rowspan="2">Approach leg</th>${turns.map(t => `<th colspan="2">${t}</th>`).join('')}<th rowspan="2">Total</th></tr><tr>${turns.map(() => '<th>LV</th><th>HV</th>').join('')}</tr>${body}
        <tr><th>Total</th>${turns.map(t => `<td>${rs.filter(x => x.turn === t).reduce((s, x) => s + x.lv_veh_h, 0) || ''}</td><td>${rs.filter(x => x.turn === t).reduce((s, x) => s + x.hv_veh_h, 0) || ''}</td>`).join('')}<td><b>${tot}</b></td></tr></table></section>`;
    }).join('');
    return `<!doctype html><html lang="en-NZ"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Turning volumes for SIDRA · ${esc(sc.name || '')}</title>
<style>body{font:13px/1.45 system-ui,sans-serif;color:#18201e;background:#fff;margin:24px}h1{font-size:20px;margin:0 0 4px}h2{font-size:15px;margin:22px 0 2px}.meta,.note{color:#56635f;margin:2px 0 8px}
table{border-collapse:collapse;margin:4px 0 10px}th,td{border:1px solid #c9d1cd;padding:3px 7px;text-align:right;font-variant-numeric:tabular-nums}th{background:#f1f4f2;font-weight:600}tr th:first-child{text-align:left}
.warn{color:#a15c07;font-weight:600}section{break-inside:avoid;page-break-inside:avoid}@media print{body{margin:10mm}}
.disc{border:1px solid #c9d1cd;border-left:4px solid #56635f;padding:6px 10px;margin:8px 0 12px;color:#18201e}.disc p{margin:2px 0}
table.pg{border-collapse:collapse;width:100%;margin:0}table.pg>tbody>tr>td,table.pg>tfoot>tr>td{border:0;padding:0;text-align:left;font-variant-numeric:normal}
.pf{border-top:1px solid #c9d1cd;margin-top:14px;padding-top:6px;color:#56635f;font-size:11px}.pf p{margin:2px 0}@media print{.pf{font-size:8pt}}</style></head><body>
<table class="pg" role="presentation"><tfoot><tr><td><footer class="pf"><p>${esc(LABEL)}.</p><p><b>D1 Screening status.</b> ${esc(D1)}</p><p><b>D12 Hand-over.</b> ${esc(D12)}</p></footer></td></tr></tfoot><tbody><tr><td>
<h1>Turning volumes for SIDRA · ${esc(sc.name || '')}</h1>
<p class="meta">Spare Construct, ${esc(when)}. Modelled demand in vehicles per hour (LV light, HV heavy vehicles), whole vehicles. Legs are named by where traffic comes from; turns use SIDRA drive-on-left designations.</p>
<div class="disc" role="note"><p><b>D1 Screening status.</b> ${esc(D1)}</p><p><b>D12 Hand-over.</b> ${esc(D12)}</p></div>
${g0.ok ? '' : `<p class="warn">${esc(g0.why)}</p>`}
${parts || '<p>No turning volumes. Run the model first.</p>'}
</td></tr></tbody></table></body></html>`;
  }

  /* ---------- figure diagram for reports: 'AM (PM)' labels rounded to 5 veh/h ---------- */
  const vec = b => [Math.sin(b * Math.PI / 180), -Math.cos(b * Math.PI / 180)];
  const f1 = x => Math.round(x * 10) / 10;
  function arrow(h, R1, aw, bIn, bOut, mv, inTwo, outTwo) {
    const ui = vec(bIn), uo = vec(bOut), nIn = [-ui[1], ui[0]], nOut = [uo[1], -uo[0]], LANE = { L: 0.38, T: 0.25, R: 0.13, U: 0.05 };
    const P = (r, u, n, d) => [h + u[0] * r + n[0] * d, h + u[1] * r + n[1] * d];
    const S0 = P(R1, ui, nIn, inTwo ? aw * (LANE[mv] || 0.25) : aw * 0.6 * ((LANE[mv] || 0.25) - 0.215)), E = P(R1, uo, nOut, outTwo ? aw * 0.25 : aw * 0.02), k = R1 * (mv === 'U' ? 0.9 : 0.75);
    return { S0, E, C1: [S0[0] - ui[0] * k, S0[1] - ui[1] * k], C2: [E[0] - uo[0] * k, E[1] - uo[1] * k], uo, nOut };
  }
  const pathD = a => `M${f1(a.S0[0])} ${f1(a.S0[1])}C${f1(a.C1[0])} ${f1(a.C1[1])} ${f1(a.C2[0])} ${f1(a.C2[1])} ${f1(a.E[0])} ${f1(a.E[1])}`;
  const arrowHead = (a, w) => { const hl = 3 + w * 1.1, hw = 2 + w * 0.9, t = [a.E[0] + a.uo[0] * hl, a.E[1] + a.uo[1] * hl]; return `${f1(t[0])},${f1(t[1])} ${f1(a.E[0] + a.nOut[0] * hw)},${f1(a.E[1] + a.nOut[1] * hw)} ${f1(a.E[0] - a.nOut[0] * hw)},${f1(a.E[1] - a.nOut[1] * hw)}`; };
  function figure(sc, resA, resB, id, opts = {}) {
    const size = opts.size || 300, h = size / 2, aw = size * 0.15, R1 = h * 0.42, fs = Math.max(9, size / 27), font = `font:600 ${f1(fs)}px var(--f-cond, sans-serif)`;
    const layer = opts.layer === 'dev' || opts.layer === 'base' ? opts.layer : 'total';
    const pick = res => { if (!res) return null; const e = entryFor(sc, res, id); return e ? { e, rows: movesOf(sc, res, e).map(r => ({ ...r, v: layer === 'total' ? r.v : r[layer] || 0 })), legs: legsOf(sc, res, e) } : null; };
    const A = pick(resA), B = pick(resB), base = A || B;
    if (!base) return `<p class="note">No turning volumes at this junction. Run the model first.</p>`;
    const legs = new Map(); for (const a of base.legs) if (!legs.has(a.o)) legs.set(a.o, a);
    const mv = new Map(), add = (R, k) => { for (const r of (R ? R.rows : [])) { const m = mv.get(r.id) || { r, a: 0, b: 0 }; m[k] += r.v; mv.set(r.id, m); } };
    add(A, 'a'); add(B, 'b');
    const name = base.e.name, two = o => { const a = legs.get(o); return !a || (a.isIn && a.isOut); };
    const s = [`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" width="${size}" height="${size}" role="img" aria-label="${esc(`${name}: turning volumes ${resA ? resA.period : ''}${resB ? ` (${resB.period})` : ''}, veh/h`)}">`,
      `<g style="stroke:var(--muted, #56635f);opacity:.2">`];
    for (const a of legs.values()) { const u = vec(a.b); s.push(`<line x1="${h}" y1="${h}" x2="${f1(h + u[0] * (h - 2))}" y2="${f1(h + u[1] * (h - 2))}" stroke-width="${f1(two(a.o) ? aw : aw * 0.6)}"/>`); }
    s.push(`<circle cx="${h}" cy="${h}" r="${f1(aw * 0.5)}" style="fill:var(--muted, #56635f)"/></g>`);
    const vmax = Math.max(1, ...[...mv.values()].map(m => Math.max(m.a, m.b)));
    for (const m of mv.values()) {
      const a = legs.get(Turns.LEG_CODES.indexOf(m.r.leg)), x = legs.get(Turns.LEG_CODES.indexOf(m.r.toLeg)); if (!a || !x || Math.max(m.a, m.b) < 0.5) continue;
      const g = arrow(h, R1, aw, a.b, x.b, m.r.mv, two(a.o), two(x.o)), w = 1.2 + 4.5 * Math.max(m.a, m.b) / vmax;
      s.push(`<g style="stroke:var(--accent, #0d6a4d);fill:var(--accent, #0d6a4d)"><path d="${pathD(g)}" fill="none" stroke-width="${f1(w)}" stroke-linecap="round" opacity=".85"/><polygon points="${arrowHead(g, w)}" stroke="none"/></g>`);
    }
    for (const a of legs.values()) {
      const u = vec(a.b), dep = [u[1], -u[0]], p = [h + u[0] * h * 0.78 + dep[0] * (aw / 2 + fs * 0.5), h + u[1] * h * 0.78 + dep[1] * (aw / 2 + fs * 0.5)];
      let ang = a.b - 90; if (norm(ang) > 90 && norm(ang) < 270) ang += 180;
      const lbl = a.road.length > 22 ? a.road.slice(0, 21) + '…' : a.road;
      s.push(`<text x="${f1(p[0])}" y="${f1(p[1])}" transform="rotate(${f1(norm(ang))} ${f1(p[0])} ${f1(p[1])})" text-anchor="middle" dominant-baseline="middle" style="fill:var(--muted, #56635f);${font}">${esc(lbl)}</text>`);
      const lines = [...mv.values()].filter(m => m.r.leg === a.leg && Math.max(m.a, m.b) >= 0.5).sort((p2, q) => TURN_ORDER.indexOf(p2.r.turn) - TURN_ORDER.indexOf(q.r.turn))
        .map(m => `${m.r.turn} ${A ? r5(m.a) : ''}${B ? (A ? ` (${r5(m.b)})` : r5(m.b)) : ''}`);
      if (!lines.length) continue;
      const nIn = [-u[1], u[0]], q = [h + u[0] * h * 0.56 + nIn[0] * (aw / 2 + fs * 0.4), h + u[1] * h * 0.56 + nIn[1] * (aw / 2 + fs * 0.4)];
      const anchor = nIn[0] > 0.35 ? 'start' : nIn[0] < -0.35 ? 'end' : 'middle', lh = fs * 1.15, bh = lines.length * lh, top = nIn[1] > 0.35 ? q[1] : nIn[1] < -0.35 ? q[1] - bh : q[1] - bh / 2;
      s.push(`<text text-anchor="${anchor}" style="fill:var(--ink, #18201e);${font}">${lines.map((t, i) => `<tspan x="${f1(q[0])}" y="${f1(top + (i + 0.8) * lh)}">${esc(t)}</tspan>`).join('')}</text>`);
    }
    const nx = size - fs * 1.2, ny = fs * 2.6;
    s.push(`<g style="fill:var(--muted, #56635f)"><polygon points="${f1(nx)},${f1(ny - fs * 1.4)} ${f1(nx - fs * 0.45)},${f1(ny - fs * 0.3)} ${f1(nx + fs * 0.45)},${f1(ny - fs * 0.3)}"/><text x="${f1(nx)}" y="${f1(ny + fs * 0.7)}" text-anchor="middle" style="${font}">N</text></g></svg>`);
    const lab = `${A ? resA.period : ''}${B ? (A ? ` (${resB.period})` : resB.period) : ''}`;
    return `${s.join('')}<p class="note">${esc(name)}: ${esc(lab)} peak-hour turning volumes${layer === 'dev' ? ', development traffic only' : layer === 'base' ? ', without development traffic' : ''}, veh/h, rounded to the nearest ${STANDARDS.figure.step}.</p>`;
  }

  /* ---------- turn bans (contract C1: node.bans '<inLinkId>><outLinkId>', node.allowU) ---------- */
  function moves(sc, res, nodeId, set = S()) {
    const n = sc.nodes[nodeId]; if (!n) return [];
    const A = nodeLegs(sc, res, nodeId).filter(a => !a.conn), deg = new Set(A.map(a => a.id)).size;
    if (deg < 3) return [];
    const T = { ...Engine.TURN_DEF, ...(set.turns || {}) }, uOK = T.uturn !== 'ban' && T.uturn !== '' && T.uturn != null && isFinite(+T.uturn);
    const bans = Array.isArray(n.bans) ? n.bans : [], flows = Turns.movements(sc, res, nodeId), out = [];
    for (const a of A) if (a.isIn) for (const x of A) if (x.isOut) {
      let mv = a.id === x.id ? 'U' : Engine.turnType(norm(a.b + 180), x.b);
      if (mv === 'U' && a.id !== x.id && deg < 3) mv = U.turn(norm(a.b + 180), x.b) > 0 ? 'R' : 'L';
      const key = a.id + '>' + x.id, explicit = bans.includes(key), rule = !explicit && mv === 'U' && !uOK && n.allowU !== true;
      const f = flows.find(r => r.approach === a.inKey && r.to === x.outKey);
      out.push({ key, from: a, to: x, mv, turn: Turns.turnCode(a.o, x.o), id: Turns.movementId(a.o, x.o), banned: explicit || rule, explicit, rule, v: f ? f.v : 0, hv: f ? f.hv : 0, inKey: a.inKey, outKey: x.outKey });
    }
    return out.sort((p, q) => p.from.o - q.from.o || TURN_ORDER.indexOf(p.turn) - TURN_ORDER.indexOf(q.turn));
  }
  function banSvg(sc, nodeId, list, size = 240) {
    const h = size / 2, aw = size * 0.17, R1 = h * 0.42, fs = Math.max(8, size / 26), font = `font:600 ${f1(fs)}px var(--f-cond, sans-serif)`;
    const arms = new Map(); for (const m of list) { arms.set(m.from.id, m.from); arms.set(m.to.id, m.to); }
    const s = [`<svg viewBox="0 0 ${size} ${size}" width="${size}" height="${size}" role="group" aria-label="Turns at this junction. Click an arrow to ban or allow that turn."><g style="stroke:var(--muted);opacity:.2">`];
    for (const a of arms.values()) { const u = vec(a.b); s.push(`<line x1="${h}" y1="${h}" x2="${f1(h + u[0] * (h - 2))}" y2="${f1(h + u[1] * (h - 2))}" stroke-width="${f1(a.isIn && a.isOut ? aw : aw * 0.6)}"/>`); }
    s.push(`<circle cx="${h}" cy="${h}" r="${f1(aw * 0.5)}" style="fill:var(--muted)"/></g>`);
    for (const a of arms.values()) {
      const u = vec(a.b), dep = [u[1], -u[0]], p = [h + u[0] * h * 0.74 + dep[0] * (aw / 2 + fs * 0.45), h + u[1] * h * 0.74 + dep[1] * (aw / 2 + fs * 0.45)];
      let ang = a.b - 90; if (norm(ang) > 90 && norm(ang) < 270) ang += 180;
      s.push(`<text x="${f1(p[0])}" y="${f1(p[1])}" transform="rotate(${f1(norm(ang))} ${f1(p[0])} ${f1(p[1])})" text-anchor="middle" dominant-baseline="middle" style="fill:var(--muted);${font}">${esc(a.leg + ' ' + (a.road.length > 16 ? a.road.slice(0, 15) + '…' : a.road))}</text>`);
    }
    for (const m of list) {
      if (m.rule) continue;   // U-turns banned by the default rule are not drawn; the switch below allows them
      const g = arrow(h, R1, aw, m.from.b, m.to.b, m.mv, m.from.isIn && m.from.isOut, m.to.isIn && m.to.isOut), col = m.banned ? 'var(--danger)' : 'var(--accent)', w = 2.2;
      const tip = `${m.id}: ${m.from.legLabel} to ${m.to.legLabel} · ${m.banned ? 'banned. Click to allow' : 'allowed. Click to ban'}`;
      s.push(`<g data-jban="${esc(m.key)}" data-node="${esc(nodeId)}" style="cursor:pointer;stroke:${col};fill:${col}"><title>${esc(tip)}</title>`
        + `<path d="${pathD(g)}" fill="none" stroke="transparent" stroke-width="12" style="pointer-events:stroke"/>`
        + `<path d="${pathD(g)}" fill="none" stroke-width="${w}"${m.banned ? ' stroke-dasharray="4 3" opacity=".7"' : ' opacity=".9"'} stroke-linecap="round"/><polygon points="${arrowHead(g, w)}" stroke="none"${m.banned ? ' opacity=".7"' : ''}/>`
        + (m.banned ? `<text x="${f1((g.S0[0] + g.E[0] * 2) / 3)}" y="${f1((g.S0[1] + g.E[1] * 2) / 3)}" text-anchor="middle" dominant-baseline="middle" style="fill:var(--danger);stroke:none;${font}">✕</text>` : '') + '</g>');
    }
    const nx = size - fs * 1.2, ny = fs * 2.6;
    s.push(`<g style="fill:var(--muted)"><polygon points="${f1(nx)},${f1(ny - fs * 1.4)} ${f1(nx - fs * 0.45)},${f1(ny - fs * 0.3)} ${f1(nx + fs * 0.45)},${f1(ny - fs * 0.3)}"/><text x="${f1(nx)}" y="${f1(ny + fs * 0.7)}" text-anchor="middle" style="${font}">N</text></g></svg>`);
    return s.join('');
  }
  /* node.bans is the list the engine reads. The feeds lane also keeps node.banSrc (ban → where it came from, e.g. an
     OpenStreetMap turn restriction) and node.bansAuto (the bans it added). Those stay as they are when the user edits
     bans: a ban the user allows again keeps its banSrc and bansAuto entries, so the feeds panel can say "You removed 1
     ban that came from OpenStreetMap" and the next review leaves that ban ticked off. Only node.bans changes here. */
  function setBans(n, list) {
    const b = [...new Set(list)];
    if (b.length) n.bans = b; else delete n.bans;
    return n;
  }
  function toggleBan(sc, nodeId, key) {
    const n = sc.nodes[nodeId]; if (!n) return null;
    const m = moves(sc, Store.res(sc), nodeId).find(x => x.key === key);
    if (m && m.rule) return { ok: false, why: 'U-turns are banned here by default. Tick "Allow U-turns here" first.' };
    Store.snapshot();
    const b = new Set(Array.isArray(n.bans) ? n.bans : []);
    const banned = !b.has(key); if (banned) b.add(key); else b.delete(key);
    setBans(n, [...b]);
    Store.edited('net');
    const osm = !banned && n.banSrc && n.banSrc[key] ? ' It came from the imported turn restrictions; your choice stays when they are reviewed again.' : '';
    return { ok: true, banned, text: `${m ? m.id + ' ' : ''}${banned ? 'banned' : 'allowed'}. Run the model again to see the effect.${osm}` };
  }

  /* ---------- lanes (movement-capacity lane editor) ---------- */
  // The approach links into a node, with lanes on the road, the stored lane model and where it is stored
  function approaches(sc, nodeId) {
    const n = sc.nodes[nodeId]; if (!n) return [];
    const site = typeof Sites !== 'undefined' ? Sites.of(sc, nodeId) : null, A = nodeLegs(sc, null, nodeId).filter(a => !a.conn && a.isIn && a.link);
    return A.map(a => {
      const L = a.link, lanes = (L.a === nodeId ? L.lanesBA : L.lanesAB) | 0, gl = Math.max(1, L.busLane && lanes >= 2 ? lanes - 1 : lanes);
      const onSite = !!(site && site.lanes && site.lanes[L.id]), raw = onSite ? site.lanes[L.id] : (n.lanes && n.lanes[L.id]) || null;
      const model = Engine.mov.laneModel(raw, gl), src = raw ? (raw.src || 'user') : 'default', by = (raw && raw.srcBy) || {};
      // Per-attribute source: srcBy first; an OSM or AT record's own fields carry its source; a user record without
      // srcBy (older projects) credits the user only with the uses and with fields set to something other than the default
      const srcOf = k => by[k] || (!raw ? 'default' : k === 'lanes' ? src : src !== 'user' ? (raw[k] != null ? src : 'default') : (!raw.srcBy && raw[k] ? 'user' : 'default'));
      return { link: L, leg: a, gl, raw, model, onSite, site, editLanes: raw && Array.isArray(raw.lanes) && raw.lanes.length ? raw.lanes.map(l => ({ d: String(l.d || l.mv || '').toUpperCase().replace(/\+/g, ''), len: +l.len || 0 }))
        : (gl === 1 ? ['LTR'] : ['LT', ...Array(gl - 2).fill('T'), 'TR']).map(d => ({ d, len: 0 })), src: { lanes: srcOf('lanes'), len: srcOf('len'), slip: srcOf('slip'), median: srcOf('median') } };
    });
  }
  const SRC_LABEL = { default: 'default NZ markings', user: 'you', osm: 'OSM turn:lanes', at: 'AT' };
  const srcTag = s => `<span class="note" title="Source">${esc(SRC_LABEL[s] || s)}</span>`;
  function editLanes(sc, nodeId, linkId, fn) {
    const ap = approaches(sc, nodeId).find(a => a.link.id === linkId); if (!ap) return false;
    Store.snapshot();
    const tgt = ap.onSite ? ap.site : sc.nodes[nodeId];
    tgt.lanes ||= {};
    const cur = tgt.lanes[linkId] ? JSON.parse(JSON.stringify(tgt.lanes[linkId])) : { lanes: ap.editLanes.map(l => ({ ...l })), median: false, slip: false };
    cur.lanes = (cur.lanes || ap.editLanes).map(l => ({ d: String(l.d || l.mv || '').toUpperCase().replace(/\+/g, ''), len: +l.len || 0 }));
    const changed = fn(cur) || [];
    if (cur === null || cur.reset) delete tgt.lanes[linkId];
    else { cur.src = 'user'; cur.srcBy = { ...(cur.srcBy || {}) }; for (const k of changed) cur.srcBy[k] = 'user'; tgt.lanes[linkId] = cur; }
    Store.edited('net');
    return true;
  }

  /* ---------- inspector sections ---------- */
  const nodeEntry = (sc, res, nodeId) => (res ? entryFor(sc, res, nodeId) : null);
  function turnsSection(sc, sel) {
    const n = sc.nodes[sel.id]; if (!n) return '';
    const res = Store.res(sc), list = res && res.turns && res.turns[sel.id];
    const links = Net.linksAt(sc, sel.id).length;
    if (!list && links < 3) return '';
    const rows = list ? Turns.movements(sc, res, sel.id) : [];
    const hasDev = rows.some(r => r.dev > 0), layer = hasDev ? st.layer : 'total';
    const pick = hasDev ? `<div class="seg" style="margin:4px 0">${[['total', 'Total'], ['base', 'Without development'], ['dev', 'Development only']].map(([k, l]) => `<button class="${layer === k ? 'on' : ''}" data-jlayer="${k}">${l}</button>`).join('')}</div>` : '';
    const ck = res && list ? checks(sc, res, { kind: 'node', node: sel.id, nodes: [sel.id] }) : null;
    const warn = ck ? [...(ck.balanced ? [] : [`Turns do not add up to the approach flow on ${ck.bad.map(b => esc(b.leg)).join(', ')} (more than ${STANDARDS.balance.tol} veh/h out). Run the model again.`]),
      ...ck.uTurns.map(u => `${esc(u.id)}: ${n0(u.v)} veh/h make a U-turn (over ${STANDARDS.uTurn.vph} veh/h; ${STANDARDS.uTurn.edition}). Check the network coding.`)].map(t => Kit.note(t, true)).join('') : '';
    const tbl = rows.length ? Kit.table({ head: ['Mov ID', 'From', 'To', { label: 'veh/h', n: true }, { label: 'HV %', n: true }], rows: rows.map(r => [esc(r.id), esc(r.legLabel), esc(Turns.legLabel(Turns.LEG_CODES.indexOf(r.toLeg), r.toRoad)), n0(layer === 'total' ? r.v : r[layer]), r.v > 0 ? p1(r.hv / r.v * 100) : '–']) }) : '';
    return Kit.sec(`Turning movements${res ? ' · ' + esc(perName(res.period)) : ''}${res && res.stale ? ' <span class="pill stale">out of date</span>' : ''}`,
      `${pick}${list ? Turns.diagram(sc, res, sel.id, { size: 240, layer, idSuffix: 'ins' }) : `<p class="note">${esc(Turns.emptyText(sc, res, sel.id))}</p>`}${warn}${tbl}
      ${list ? `<div class="actions"><button class="btn small" data-jx-do="turncsv" data-node="${esc(sel.id)}">Turning count (CSV)</button><button class="btn small" data-jx-do="j1" data-node="${esc(sel.id)}">Movement summary</button></div>` : ''}
      <p class="note">${LABEL}.</p>`);
  }
  function bansSection(sc, sel) {
    const n = sc.nodes[sel.id]; if (!n) return '';
    const list = moves(sc, Store.res(sc), sel.id); if (!list.length) return '';
    const nb = list.filter(m => m.explicit).length;
    const rows = list.map(m => `<tr><td>${esc(m.id)}</td><td>${esc(m.from.legLabel)}</td><td>${esc(m.to.legLabel)}</td><td class="n">${m.v ? n0(m.v) : '–'}</td>`
      + `<td><label class="check" style="margin:0"><input type="checkbox" data-jban="${esc(m.key)}" data-node="${esc(sel.id)}" ${m.banned ? '' : 'checked'} ${m.rule ? 'disabled title="Tick Allow U-turns here first"' : ''} aria-label="Allow ${esc(m.id)}"> ${m.rule ? 'banned (U-turn rule)' : m.banned ? 'banned' : 'allowed'}</label></td></tr>`).join('');
    return Kit.sec(`Allowed turns${nb ? ` · ${nb} banned` : ''}`, `${banSvg(sc, sel.id, list)}
      <p class="note">Click an arrow to ban or allow that turn. Banned turns are dashed and carry no traffic after the next run.</p>
      <label class="check"><input type="checkbox" ${Kit.bind('node', sel.id, 'allowU', 'bool')} ${n.allowU ? 'checked' : ''}> Allow U-turns here</label>
      <details><summary>Turn list</summary><div class="tbl-wrap"><table class="t compact"><tr><th>Mov ID</th><th>From</th><th>To</th><th class="n">veh/h</th><th>Allowed</th></tr>${rows}</table></div></details>`);
  }
  function lanesSection(sc, sel) {
    const n = sc.nodes[sel.id]; if (!n || n.control === 'signals' || Net.linksAt(sc, sel.id).length < 3) return '';
    const aps = approaches(sc, sel.id); if (!aps.length) return '';
    const mvm = Engine.capMethodOf(S()), res = Store.res(sc), e = res ? nodeEntry(sc, res, sel.id) : null;
    const opts = d => Object.entries(DISC).map(([k, l]) => `<option value="${k}" ${k === d ? 'selected' : ''}>${l}</option>`).join('');
    const body = aps.map(ap => {
      const lid = esc(ap.link.id), apRes = e && e.nr ? e.nr.appr.find(a => a.link === ap.link.id || a.key === ap.leg.inKey) : null;
      const caps = apRes && apRes.mov ? `<p class="note">Capacity ${apRes.mov.map(m => `${esc(m.mv)} ${n0(m.cap)}`).join(', ')} veh/h${apRes.capT != null ? `; through ${n0(apRes.capT)} veh/h` : ''}.</p>` : '';
      return `<div style="margin:8px 0 10px" data-jlink="${lid}"><div><b>${esc(ap.leg.legLabel)}</b> · ${ap.gl} lane${ap.gl === 1 ? '' : 's'} on the road${ap.onSite ? ' · stored on the junction site' : ''}</div>
        <div class="tbl-wrap"><table class="t compact"><tr><th>Lane (kerb first)</th><th>Use</th><th class="n">Bay length (m)</th><th></th></tr>
        ${ap.editLanes.map((l, i) => `<tr><td>${i + 1}</td><td><select data-jl="d" data-i="${i}" data-node="${esc(sel.id)}" data-link="${lid}" aria-label="Lane ${i + 1} use">${opts(l.d)}</select></td>
          <td class="n"><input type="number" min="0" step="5" style="width:70px" data-jl="len" data-i="${i}" data-node="${esc(sel.id)}" data-link="${lid}" value="${l.len || ''}" placeholder="full" aria-label="Lane ${i + 1} bay length in metres"></td>
          <td>${ap.editLanes.length > 1 ? `<button class="btn small icon" data-jl-do="del" data-i="${i}" data-node="${esc(sel.id)}" data-link="${lid}" title="Remove lane ${i + 1}" aria-label="Remove lane ${i + 1}">✕</button>` : ''}</td></tr>`).join('')}</table></div>
        <div style="font-size:12px">Uses: ${srcTag(ap.src.lanes)} · bays: ${srcTag(ap.src.len)}</div>
        <label class="check"><input type="checkbox" data-jl="slip" data-node="${esc(sel.id)}" data-link="${lid}" ${ap.model.slip ? 'checked' : ''}> Left-turn slip lane (gives way) ${srcTag(ap.src.slip)}</label>
        <label class="check"><input type="checkbox" data-jl="median" data-node="${esc(sel.id)}" data-link="${lid}" ${ap.model.median ? 'checked' : ''}> Flush median (side-road turns cross in two stages) ${srcTag(ap.src.median)}</label>
        <div class="actions"><button class="btn small" data-jl-do="add" data-node="${esc(sel.id)}" data-link="${lid}">Add lane</button>${ap.raw ? `<button class="btn small" data-jl-do="reset" data-node="${esc(sel.id)}" data-link="${lid}">Use default markings</button>` : ''}</div>${caps}</div>`;
    }).join('');
    const site = typeof Sites !== 'undefined' ? Sites.of(sc, sel.id) : null, ctl = (site && site.control) || n.control || 'none';
    return `<div class="ins-sec"><details${mvm === 'movement' ? ' open' : ''}><summary><b>Lanes</b>${mvm === 'movement' ? '' : ' <span class="note">(used by movement capacity)</span>'}</summary>
      ${mvm === 'movement' ? '' : Kit.note('These lanes are used when Settings › Junction analysis › Capacity method is "per movement". The current method uses one capacity per approach.')}
      ${body}<p class="note">Blank bay length = a full-length lane. A bay of ${n0(Engine.STANDARDS.queue.space)} m holds one vehicle. Lane uses follow NZ markings (L+T kerb lane, T+R right lane).</p>
      ${ctl === 'roundabout' ? '' : gapsHtml(sc, sel.id)}</details></div>`;
  }
  // Critical gaps and follow-up headways for this junction (node.gaps, or site.gaps when the node is in a site); blank = the standard
  const GAP_ROWS = [['majorR', 'Major road right turn'], ['minorL', 'Side road left turn'], ['minorT', 'Side road straight through'], ['minorR', 'Side road right turn']];
  function gapsOf(sc, nodeId) { const s = typeof Sites !== 'undefined' ? Sites.of(sc, nodeId) : null; return { tgt: s || sc.nodes[nodeId], onSite: !!s }; }
  function gapsHtml(sc, nodeId) {
    const G = Engine.STANDARDS.gaps; if (!G) return '';
    const { tgt, onSite } = gapsOf(sc, nodeId), own = (tgt && tgt.gaps) || {}, id = esc(nodeId);
    const inp = (k, f) => `<input type="number" min="1" max="15" step="0.1" style="width:64px" data-jg="${k}" data-f="${f}" data-node="${id}" value="${own[k] && own[k][f] != null ? esc(own[k][f]) : ''}" placeholder="${G[k][f]}" aria-label="${esc(GAP_ROWS.find(r => r[0] === k)[1])} ${f === 'tc' ? 'critical gap' : 'follow-up headway'} in seconds">`;
    const n = Object.keys(own).length;
    return `<details><summary>Critical gaps${n ? ` · ${n} changed` : ''}</summary>
      <div class="tbl-wrap"><table class="t compact"><tr><th>Movement</th><th class="n">Critical gap (s)</th><th class="n">Follow-up (s)</th></tr>
      ${GAP_ROWS.map(([k, l]) => `<tr><td>${l}</td><td class="n">${inp(k, 'tc')}</td><td class="n">${inp(k, 'tf')}</td></tr>`).join('')}</table></div>
      <p class="note">Blank uses ${esc(G.edition)}${G.verified ? '' : ' (not yet checked against the source)'}. ${onSite ? 'Stored on the junction site.' : 'Stored on this node.'} Used by movement capacity only.</p></details>`;
  }
  function editGap(sc, nodeId, k, f, value) {
    const { tgt } = gapsOf(sc, nodeId); if (!tgt || !Engine.STANDARDS.gaps[k] || (f !== 'tc' && f !== 'tf')) return false;
    const v = value === '' || value == null ? null : +value;
    if (v != null && !(v >= 1 && v <= 15)) return false;
    Store.snapshot();
    const g = { ...(tgt.gaps || {}) }, row = { ...(g[k] || {}) };
    if (v == null) delete row[f]; else row[f] = v;
    if (Object.keys(row).length) g[k] = row; else delete g[k];
    if (Object.keys(g).length) tgt.gaps = g; else delete tgt.gaps;
    Store.edited('net');
    return true;
  }
  // Nearby junction nodes and sites this node could be merged with (within the site diameter)
  function mergeCandidates(sc, nodeId) {
    const n = sc.nodes[nodeId]; if (!n || typeof Sites === 'undefined') return [];
    const R = Sites.RULES.diameter, own = Sites.of(sc, nodeId), seen = new Set(), out = [];
    for (const m of Object.values(sc.nodes)) {
      if (m.id === nodeId || (own && own.nodes.includes(m.id))) continue;
      const d = U.dist([n.lat, n.lng], [m.lat, m.lng]); if (d > R) continue;
      const ls = Net.linksAt(sc, m.id); if (!((m.control || 'none') !== 'none' || ls.length >= 3)) continue;
      const s = Sites.of(sc, m.id), id = s ? s.id : m.id; if (seen.has(id)) continue;
      seen.add(id);
      out.push({ id, kind: s ? 'site' : 'node', name: s ? Sites.name(sc, s) : Turns.junctionName(sc, m.id), d: Math.round(d) });
    }
    return out.sort((a, b) => a.d - b.d).slice(0, 6);
  }
  function siteSection(sc, sel) {
    const n = sc.nodes[sel.id]; if (!n || typeof Sites === 'undefined') return '';
    const s = Sites.of(sc, sel.id), cand = mergeCandidates(sc, sel.id);
    if (!s && !cand.length) return '';
    if (s) return Kit.sec('Junction site', `<p>Part of <b>${esc(Sites.name(sc, s))}</b> (${s.nodes.length} nodes), modelled as one junction${s.src && s.src.nodes === 'auto' ? ', grouped automatically' : ''}.</p>
      <div class="actions"><button class="btn small" data-jx-do="opensite" data-site="${esc(s.id)}">Open site</button><button class="btn small" data-jx-do="split" data-site="${esc(s.id)}">Split site</button></div>`);
    return Kit.sec('Junction site', `<p class="note">Junctions mapped as several nearby nodes (divided roads, offset T-junctions) are best modelled as one site with one control.</p>
      ${cand.map(c => `<div class="lg-row"><span>${esc(c.name)} <span class="note">${c.d} m${c.kind === 'site' ? ', site' : ''}</span></span><button class="btn small" data-jx-do="merge" data-node="${esc(sel.id)}" data-with="${esc(c.id)}">Merge</button></div>`).join('')}`);
  }

  /* ---------- the junction-site panel ---------- */
  function sitePanel(sc, sel) {
    const s = (sc.sites || {})[sel.id]; if (!s) return '';
    const derived = (() => { let b = 'none'; const R = { none: 0, priority: 1, stop: 2, roundabout: 3, signals: 4 }; for (const id of s.nodes) { const c = (sc.nodes[id] || {}).control || 'none'; if ((R[c] || 0) > R[b]) b = c; } return b; })();
    // 'none' is not offered: the engine models a site set to 'none' with its strongest member control (Engine.siteControl)
    const own = s.control && s.control !== 'none' ? s.control : null, ctl = own || derived;
    const cOpts = `<option value="" ${own ? '' : 'selected'}>From its junctions (${esc(ctlName(derived))})</option>` + Object.entries(CONTROLS).filter(([k]) => k !== 'none').map(([k, l]) => `<option value="${esc(k)}" ${own === k ? 'selected' : ''}>${esc(l)}</option>`).join('');
    const per = periodsOf(sc).map(p => { const res = resOf(sc, p), e = res ? entries(sc, res).find(x => x.id === s.id) : null, m = e ? summary(sc, res, e) : null;
      return m ? `<tr><td>${esc(perName(p))}</td><td class="n">${dosCell(m.dos, m.xp)}</td><td class="n">${d1(m.delay)}</td><td>${Kit.losBadge(m.los)}</td><td class="n">${n0(m.q95)} m</td></tr>` : ''; }).join('');
    // junction-sites: zone connectors may not attach to site nodes (phantom movements); the engine skips them for
    // automatic connectors, but a zone pinned to a node (z.node) still would
    const pinned = Object.values(sc.zones || {}).filter(z => z.node && s.nodes.includes(z.node));
    const zoneWarn = pinned.length ? Kit.note(`${pinned.map(z => esc(z.name || z.id)).join(', ')} ${pinned.length === 1 ? 'connects' : 'connect'} straight to a node inside this site, which adds movements SIDRA would not have. Connect ${pinned.length === 1 ? 'it' : 'them'} part-way along a road, or add a driveway leg.`, true) : '';
    const nodes = s.nodes.filter(id => sc.nodes[id]).map(id => `<div class="lg-row"><a href="#" data-fly="node:${esc(id)}">${esc(Turns.junctionName(sc, id))}</a> <span class="note">${esc(ctlName(sc.nodes[id].control))}</span><button class="btn small" style="margin-left:auto" data-jx-do="unsite" data-site="${esc(s.id)}" data-node="${esc(id)}">Remove</button></div>`).join('');
    return `<div class="ins-head"><div class="ins-kind">Junction site<span style="margin-left:auto"><button class="btn small icon" data-act="close" title="Close (Esc)" aria-label="Close">✕</button></span></div>
      <input class="ins-title" data-jsite="name" value="${esc(s.name || '')}" placeholder="${esc(Sites.name(sc, { ...s, name: '' }))}" aria-label="Name"></div>
      <div class="ins-sec"><h4>Control</h4>${Kit.fld('Control', `<select data-jsite="control">${cOpts}</select>`)}
        ${ctl === 'signals' ? `${Kit.fld('Cycle time', `<input type="number" min="40" max="180" step="5" data-jsite="cycle" value="${esc(s.cycle ?? '')}" placeholder="auto">`, 's')}<label class="check"><input type="checkbox" data-jsite="fixedTiming" ${s.fixedTiming ? 'checked' : ''}> Keep equal green splits (don’t optimise)</label>` : ''}
        ${Kit.fld('AT site number', `<input data-jsite="atSiteNo" value="${esc(Array.isArray(s.atSiteNo) ? s.atSiteNo.join(', ') : s.atSiteNo ?? '')}" placeholder="optional">`)}
        <p class="note">${s.nodes.length} nodes modelled as one junction: one controller, legs grouped by road, and no junction delay on the links inside the site.${s.src && s.src.nodes === 'auto' ? ` Grouped automatically (${esc(Sites.RULES.basis)}).` : ' Grouped by hand.'}</p>${zoneWarn}</div>
      ${per ? `<div class="ins-sec"><h4>Results</h4>${suppNote()}<div class="tbl-wrap"><table class="t compact"><tr><th>Peak</th><th class="n">Worst DoS</th><th class="n">Delay (s)</th><th>LoS</th><th class="n">Longest Q95</th></tr>${per}</table></div>
        <div class="actions"><button class="btn small" data-jx-do="j1" data-site="${esc(s.id)}">Movement summary</button></div><p class="note">${LABEL}.</p></div>` : `<div class="ins-sec"><p class="note">Run the model to see this site's results.</p></div>`}
      ${typeof Inspector !== 'undefined' ? Inspector.SECTIONS : ''}
      <div class="ins-sec"><h4>Nodes in this site</h4>${nodes}</div>
      <div class="ins-sec"><div class="actions"><button class="btn small" data-jx-do="zoomsite" data-site="${esc(s.id)}">Zoom to</button><button class="btn small danger" data-jx-do="split" data-site="${esc(s.id)}">Split site</button></div></div>`;
  }
  function splitSite(sc, id) {
    if (!(sc.sites || {})[id]) return false;
    Store.snapshot(); Sites.split(sc, id); Store.edited('net');
    return true;
  }
  function mergeInto(sc, a, b) {
    const ids = [a, b].map(x => (Sites.of(sc, x) ? Sites.of(sc, x).id : x));
    Store.snapshot();
    const id = Sites.merge(sc, ids);
    if (!id) { Store.undoStack && Store.undoStack.pop(); return null; }
    Store.edited('net');
    return id;
  }
  function detectSites(sc) { Store.snapshot(); const r = Sites.detect(sc); Store.edited('net'); return r; }

  /* ---------- the drawer tab ---------- */
  const VIEWS = [['summary', 'Summary'], ['movements', 'Movement summary'], ['compare', 'Compare'], ['scale', `+${Math.round((STANDARDS.flowScale.factor - 1) * 100)}% flow test`], ['handoff', 'SIDRA hand-off'], ['los', 'LoS definitions']];
  function losTable(method) {
    const L = Engine.STANDARDS.los[method] || Engine.STANDARDS.los.hcm6, op = method === 'rta' ? '<' : '≤';
    const rows = 'ABCDE'.split('').map((l, i) => [Kit.losBadge(l), `${i ? `> ${L.signals[i - 1]} and ` : ''}${op} ${L.signals[i]}`, `${i ? `> ${L.roundabout[i - 1]} and ` : ''}${op} ${L.roundabout[i]}`, `${i ? `> ${L.sign[i - 1]} and ` : ''}${op} ${L.sign[i]}`]);
    rows.push([Kit.losBadge('F'), `> ${L.signals[4]}, or DoS > 1`, `> ${L.roundabout[4]}, or DoS > 1`, `> ${L.sign[4]}, or DoS > 1`]);
    const xp = Engine.STANDARDS.practicalDoS;
    return `${Kit.table({ head: ['LoS', 'Signals: control delay (s)', 'Roundabouts (s)', 'Give way and stop (s)'], rows })}
      <p class="note">${esc(L.name)}, ${esc(L.edition)}${L.verified ? '' : ' (not yet checked against the source)'}. Practical degree of saturation: signals ${xp.signals}, roundabouts ${xp.roundabout}, give way and stop ${xp.sign} (${esc(xp.edition)}). Give way and stop junctions report the worst movement, with no intersection LoS.</p>`;
  }
  function pickList(sc, res) {
    const es = res ? entries(sc, res) : [];
    if (!st.pick || !es.some(e => e.id === st.pick)) {
      const sel = View.sel && (View.sel.type === 'node' || View.sel.type === 'junction-site') ? es.find(e => e.id === View.sel.id || e.nodes.includes(View.sel.id)) : null;
      st.pick = sel ? sel.id : es.slice().sort((a, b) => (b.nr.delay || 0) - (a.nr.delay || 0))[0]?.id || null;
    }
    return es;
  }
  function handoffHtml(sc) {
    const list = bothRes(sc), g = list.length ? gate(sc, list) : { ok: false, why: 'Run the model first.' };
    const per = list.map(r => { const c = Engine.exportReady(r); return `<div class="lg-row"><span class="los ${c.ok ? 'A' : 'none'}">${c.ok ? '✓' : '!'}</span><span>${esc(perName(r.period))}: ${c.ok ? `converged enough for SIDRA (gap ${(r.gap * 100).toPrecision(2)}%)` : esc(c.why)}${r.stale ? ' Results are out of date.' : ''}</span></div>`; }).join('');
    const counts = list.length ? { auto: targets(sc, list[0], null, 'auto').length, controlled: targets(sc, list[0], null, 'controlled').length, all: targets(sc, list[0], null, 'all').length } : { auto: 0, controlled: 0, all: 0 };
    const ext = sc.extent && Array.isArray(sc.extent.sites) && sc.extent.sites.length;
    const scope = `<select data-jx="scope" aria-label="Junctions to export"><option value="auto" ${st.scope === 'auto' ? 'selected' : ''}>${ext ? `Sites in the extent of effects (${counts.auto})` : `Controlled junctions (${counts.auto})`}</option>
      ${ext ? `<option value="controlled" ${st.scope === 'controlled' ? 'selected' : ''}>Controlled junctions (${counts.controlled})</option>` : ''}<option value="all" ${st.scope === 'all' ? 'selected' : ''}>Every junction with turning traffic (${counts.all})</option></select>`;
    return `<p>Export the modelled turning volumes to SIDRA INTERSECTION 11. The browser cannot write a .sipx file, so Spare Construct gives you three files: the volumes (CSV), a Python loader that adds or updates sites through the SIDRA 11 API with the New Zealand setup, and printable volume grids.</p>
      ${per || '<p class="note">Run the model first.</p>'}
      ${Kit.fld('Junctions', scope)}
      <div class="actions"><button class="btn small primary" data-jx-do="sidra-all" ${g.ok ? '' : 'disabled'}>Export to SIDRA (3 files)</button><button class="btn small" data-jx-do="sidra-volumes" ${g.ok ? '' : 'disabled'}>Volumes (CSV)</button><button class="btn small" data-jx-do="sidra-loader" ${g.ok ? '' : 'disabled'}>Loader script (.py)</button><button class="btn small" data-jx-do="sidra-grids" ${list.length ? '' : 'disabled'}>Volume grids (HTML)</button></div>
      ${!g.ok && list.length ? `<p class="note warn">${esc(g.why)} <button class="btn small" data-jx-do="sidra-force">Export anyway, marked not converged</button></p>` : ''}
      <div class="actions"><button class="btn small" data-jx-do="csv-movements" ${list.length ? '' : 'disabled'}>Junction movements (CSV)</button><button class="btn small" data-jx-do="csv-turning" ${list.length ? '' : 'disabled'}>Turning movements (CSV)</button></div>
      <p class="note">The loader needs licensed SIDRA 11, 64-bit Python, pythonnet and .NET 8, and is untested against a live install: run it with --dry-run first. It matches existing sites by Intersection ID and overwrites only their volumes, peak flow factors, unit time and peak flow period, keeping your lanes and phasing. Volumes are vehicles per hour (demand), split into light and heavy vehicles. ${esc(D12)}</p>`;
  }
  /* Which way a comparison runs: from the existing / without-development side to the proposal or mitigation side, by
     scenario role (existing, baseline or the base scenario first; proposal, then mitigation, last). Equal ranks
     measure the active scenario against the other one. swap reverses it. */
  const roleRank = (P, s) => (s.role === 'existing' ? 0 : s.role === 'baseline' ? 1 : s.role === 'mitigation' ? 3 : s.role === 'proposal' ? 2 : s.id === P.base ? 1 : 2);
  function compareOrder(P, active, other, swap) {
    const o = roleRank(P, active) < roleRank(P, other) ? [active, other] : [other, active];
    return swap ? o.reverse() : o;
  }
  function tabHtml() {
    const sc = Store.sc, P = Store.P, pl = periodsOf(sc).length ? periodsOf(sc) : PERIODS, period = st.period && pl.includes(st.period) ? st.period : pl.includes(Store.S.period) ? Store.S.period : pl[0], res = resOf(sc, period);
    const nSites = Object.keys(sc.sites || {}).length;
    const seg = `<div class="seg" role="tablist" aria-label="Junction views">${VIEWS.map(([k, l]) => `<button class="${st.view === k ? 'on' : ''}" data-jx-view="${k}" role="tab" aria-selected="${st.view === k}">${l}</button>`).join('')}</div>`;
    const perSel = `<select data-jx="period" aria-label="Peak">${pl.map(p => `<option value="${esc(p)}" ${p === period ? 'selected' : ''}>${esc(perName(p))}</option>`).join('')}</select>`;
    const top = `<div class="bar-h"><h3>Junctions</h3>${seg}<span class="note" style="margin-left:auto">${nSites ? `${nSites} junction site${nSites === 1 ? '' : 's'} · ` : ''}<button class="btn small" data-jx-do="detect">Find junction sites</button></span></div>`;
    let body = '';
    if (st.view === 'summary') body = j2Html(sc);
    else if (st.view === 'movements') {
      const es = pickList(sc, res);
      body = !res ? '<p class="note">Run the model to see the Movement Summary.</p>' : `<div class="hud-row">${Kit.fld('Junction', `<select data-jx="pick" aria-label="Junction">${es.map(e => `<option value="${esc(e.id)}" ${e.id === st.pick ? 'selected' : ''}>${esc(e.name)}${e.kind === 'site' ? ' (site)' : ''}</option>`).join('')}</select>`)}${Kit.fld('Peak', perSel)}</div>${j1Html(st.pick ? j1(sc, res, st.pick) : null)}`;
    } else if (st.view === 'compare') {
      const others = P.scenarios.filter(s => s.id !== sc.id && periodsOf(s).length);
      if (!others.some(s => s.id === st.against)) { st.against = (others.find(s => s.id === P.base) || others.find(s => s.role === 'proposal') || others[0] || {}).id || null; st.swap = false; }
      const other = others.find(s => s.id === st.against), [from, to] = other ? compareOrder(P, sc, other, st.swap) : [null, sc];
      body = others.length ? `<div class="hud-row">${Kit.fld('Compare with', `<select data-jx="against" aria-label="Scenario to compare with">${others.map(s => `<option value="${esc(s.id)}" ${s.id === st.against ? 'selected' : ''}>${esc(s.name)}</option>`).join('')}</select>`)}
        <button class="btn small" data-jx-do="swap" title="Swap which scenario the changes are measured from">Swap direction</button></div>${j3Html(from, to)}`
        : '<p class="note">Run another scenario to compare its junctions with this one.</p>';
    } else if (st.view === 'scale') body = `<div class="hud-row">${Kit.fld('Peak', perSel)}</div>${scaleHtml(sc, res)}`;
    else if (st.view === 'handoff') body = handoffHtml(sc);
    else body = losTable((res && res.junction && res.junction.method) || (Store.S.junction || {}).losMethod || 'hcm6');
    return `${top}${body}`;
  }
  const dl = (name, text, mime) => U.download(name, text, mime);
  const base = () => (typeof UI !== 'undefined' && UI.fileBase ? UI.fileBase() : 'corridor');
  function sidraFiles(which, force) {
    const sc = Store.sc, list = bothRes(sc), opts = { scope: st.scope, force }, sfx = force ? '-NOT-CONVERGED' : '';
    try {
      if (which === 'volumes' || which === 'all') dl(`corridor-sidra-volumes${sfx}.csv`, volumesCSV(sc, list, null, opts), 'text/csv');
      if (which === 'loader' || which === 'all') dl(`load_corridor_volumes${force ? '_NOT_CONVERGED' : ''}.py`, loaderPy(sc, list, null, opts), 'text/x-python');
      if (which === 'grids' || which === 'all') dl(`${base()}-sidra-volume-grids${sfx}.html`, gridsHtml(sc, list, null, opts), 'text/html');
      Kit.toast(which === 'all' ? 'Saved the SIDRA volumes, the loader script and the volume grids.' : 'Saved.');
    } catch (e) { Kit.toast(`Could not export for SIDRA: ${e.message}`, 'err'); }
  }
  function flySite(id) {
    const sc = Store.sc, s = (sc.sites || {})[id];
    if (s && typeof UI !== 'undefined' && UI.map) { const c = centre(sc, s.nodes); if (c) UI.map.setView(c, Math.max(UI.map.getZoom(), 17)); }
    UI.select({ type: 'junction-site', id });
  }
  function fly(f) { const i = f.indexOf(':'), t = f.slice(0, i), id = f.slice(i + 1); if (t === 'junction-site') flySite(id); else UI.flyTo(t, id); }
  async function askDetect() {
    const ok = await Kit.confirmBox('Group junction nodes into sites?', 'Spare Construct groups nodes that form one junction (divided roads, offset T-junctions, roundabouts) into sites with one control each. Results change at those junctions when you run the model again, so check each site before relying on it. Sites you grouped by hand are kept. Ctrl+Z undoes this.', 'Group nodes');
    if (!ok) return;
    const r = detectSites(Store.sc);
    Kit.toast(`Grouped ${r.nodes} nodes into ${r.sites} junction site${r.sites === 1 ? '' : 's'}${r.kept ? `; kept ${r.kept} you made` : ''}. Run the model again.`);
  }
  // Shared handlers for the tab, the sections and the panel ([data-jx-do])
  function doAction(b) {
    const d = b.dataset.jxDo, sc = Store.sc;
    if (d === 'turncsv') {
      const res = Store.res(); if (!res) return;
      const f = turnCountFile(sc, res, b.dataset.node);
      dl(`${base()}-turns-${b.dataset.node}-${res.period}${f.nc ? '-NOT-CONVERGED' : ''}.csv`, f.text, 'text/csv');
      if (f.nc) Kit.toast(`Saved, marked NOT CONVERGED: ${Engine.exportReady(res).why}`, 'warn');
      return;
    }
    if (d === 'j1') { const res = Store.res(), e = res ? entryFor(sc, res, b.dataset.site || b.dataset.node) : null; st.view = 'movements'; st.pick = e ? e.id : null; UI.openTab('junctions'); return; }
    if (d === 'opensite' || d === 'zoomsite') return flySite(b.dataset.site);
    if (d === 'split') { if (splitSite(sc, b.dataset.site)) { Kit.toast('Site split back into separate junctions. Run the model again. Ctrl+Z to undo.'); if (View.sel && View.sel.type === 'junction-site') UI.select(null); } return; }
    if (d === 'unsite') {
      const s = (sc.sites || {})[b.dataset.site]; if (!s) return;
      Store.snapshot(); s.nodes = s.nodes.filter(n => n !== b.dataset.node); if (s.nodes.length < 2) Sites.split(sc, s.id); else s.src = { ...(s.src || {}), nodes: 'user' };
      Store.edited('net'); if (!sc.sites[b.dataset.site]) UI.select(null); return;
    }
    if (d === 'merge') { const id = mergeInto(sc, b.dataset.node, b.dataset.with); if (id) { Kit.toast(`Merged into one junction site: ${Sites.name(sc, sc.sites[id])}. Run the model again.`); UI.select({ type: 'junction-site', id }); } return; }
    if (d === 'detect') return askDetect();
    if (d === 'sidra-all') return sidraFiles('all');
    if (d === 'sidra-volumes') return sidraFiles('volumes');
    if (d === 'sidra-loader') return sidraFiles('loader');
    if (d === 'sidra-grids') return sidraFiles('grids', !gate(sc, bothRes(sc)).ok);
    if (d === 'sidra-force') return sidraFiles('all', true);
    if (d === 'csv-movements') return dl(`${base()}-junction_movements.csv`, junctionMovementsCSV(Store.P, sc), 'text/csv');
    if (d === 'csv-turning') { try { dl(`${base()}-turning_movements.csv`, turningMovementsCSV(Store.P, sc), 'text/csv'); } catch (e) { Kit.toast(`Could not export turning movements: ${e.message}`, 'warn'); } return; }
    if (d === 'swap') { st.swap = !st.swap; Drawer.render(); return; }
  }

  /* ---------- registrations ---------- */
  Registry.tabs({ id: 'junctions', label: 'Junctions', order: 50, title: 'Junction performance, movement summaries and SIDRA hand-off',
    render: tabHtml,
    onClick(e) {
      const v = e.target.closest('[data-jx-view]'); if (v) { st.view = v.dataset.jxView; Drawer.render(); return true; }
      const f = e.target.closest('[data-jx-fly]'); if (f) { fly(f.dataset.jxFly); return true; }
      const b = e.target.closest('[data-jx-do]'); if (b) { doAction(b); return true; }
    },
    onChange(e) {
      const t = e.target, k = t.dataset && t.dataset.jx; if (!k) return;
      if (k === 'period') st.period = t.value; else if (k === 'pick') st.pick = t.value; else if (k === 'against') { st.against = t.value; st.swap = false; } else if (k === 'scope') st.scope = t.value;
      Drawer.render(); return true;
    } });
  const secClick = (e) => {
    const b = e.target.closest('[data-jx-do]'); if (b) { doAction(b); return true; }
    const lay = e.target.closest('[data-jlayer]'); if (lay) { st.layer = lay.dataset.jlayer; Inspector.render(); return true; }
    const ban = e.target.closest('g[data-jban]'); if (ban) { const r = toggleBan(Store.sc, ban.dataset.node, ban.dataset.jban); if (r && !r.ok) Kit.toast(r.why, 'warn'); else if (r) Kit.toast(r.text); return true; }
    const ld = e.target.closest('[data-jl-do]');
    if (ld) {
      const i = +ld.dataset.i, act = ld.dataset.jlDo;
      editLanes(Store.sc, ld.dataset.node, ld.dataset.link, cur => {
        if (act === 'add') { cur.lanes.push({ d: 'T', len: 0 }); return ['lanes']; }
        if (act === 'del') { cur.lanes.splice(i, 1); return ['lanes']; }
        if (act === 'reset') { cur.reset = true; return []; }
      });
      return true;
    }
  };
  const secChange = (e) => {
    const t = e.target;
    if (t.matches && t.matches('input[type=checkbox][data-jban]')) { const r = toggleBan(Store.sc, t.dataset.node, t.dataset.jban); if (r && !r.ok) Kit.toast(r.why, 'warn'); return true; }
    if (t.dataset && t.dataset.jg) { if (!editGap(Store.sc, t.dataset.node, t.dataset.jg, t.dataset.f, t.value.trim())) Kit.toast('Enter a time between 1 and 15 seconds, or leave it blank for the standard value.', 'warn'); return true; }
    if (t.dataset && t.dataset.jl) {
      const k = t.dataset.jl, i = +t.dataset.i;
      editLanes(Store.sc, t.dataset.node, t.dataset.link, cur => {
        if (k === 'd') { cur.lanes[i].d = t.value; return ['lanes']; }
        if (k === 'len') { cur.lanes[i].len = Math.max(0, +t.value || 0); return ['len']; }
        if (k === 'slip' || k === 'median') { cur[k] = t.checked; return [k]; }
      });
      return true;
    }
  };
  Registry.inspectorSections({ id: 'junctions-site', types: ['node'], order: 5, render: siteSection, onClick: secClick });
  Registry.inspectorSections({ id: 'junctions-turns', types: ['node'], order: 10, render: turnsSection, onClick: secClick });
  Registry.inspectorSections({ id: 'junctions-bans', types: ['node'], order: 20, render: bansSection, onClick: secClick, onChange: secChange });
  Registry.inspectorSections({ id: 'junctions-lanes', types: ['node'], order: 30, render: lanesSection, onClick: secClick, onChange: secChange });

  Registry.panels({ type: 'junction-site', label: 'Junction site', render: sitePanel,
    onClick(e) {
      const b = e.target.closest('[data-jx-do]'); if (b) { doAction(b); return true; }
      const f = e.target.closest('a[data-fly]'); if (f) { e.preventDefault(); const v = f.dataset.fly, i = v.indexOf(':'); UI.flyTo(v.slice(0, i), v.slice(i + 1)); return true; }
    },
    onChange(e) {
      const t = e.target, k = t.dataset && t.dataset.jsite, sc = Store.sc, s = View.sel && (sc.sites || {})[View.sel.id]; if (!k || !s) return;
      Store.snapshot();
      if (k === 'name') { s.name = t.value.trim(); Store.emit('meta'); return true; }
      if (k === 'control') s.control = t.value || null;
      else if (k === 'cycle') s.cycle = t.value === '' ? null : Math.max(0, +t.value || 0);
      else if (k === 'fixedTiming') s.fixedTiming = t.checked;
      else if (k === 'atSiteNo') { const v = t.value.trim(); s.atSiteNo = v === '' ? null : /,/.test(v) ? v.split(',').map(x => x.trim()).filter(Boolean) : v; Store.emit('meta'); return true; }
      Store.edited('net'); return true;
    },
    onDelete(sel) { if (splitSite(Store.sc, sel.id)) { UI.select(null); Kit.toast('Site split back into separate junctions. Ctrl+Z to undo.'); } } });

  // Map layer: a dashed ring round each junction site; click the ring to select the site
  const ringOf = (sc, s, geo) => {
    const pts = s.nodes.map(id => geo.nodes.get(id)).filter(Boolean); if (!pts.length) return null;
    const cx = pts.reduce((a, p) => a + p.x, 0) / pts.length, cy = pts.reduce((a, p) => a + p.y, 0) / pts.length;
    return { cx, cy, r: Math.max(10, ...pts.map(p => Math.hypot(p.x - cx, p.y - cy))) + 9 };
  };
  Registry.layers({ id: 'junctions-sites', label: 'Junction sites', order: 20, defaultOn: true, minZoom: 15,
    draw(ctx, off, size, geo) {
      const sc = Store.sc, T = (typeof Render !== 'undefined' && Render.theme) || {};
      ctx.lineWidth = 1.6; ctx.setLineDash([5, 4]); ctx.strokeStyle = T.sel || '#2459d6';
      for (const s of Object.values(sc.sites || {})) {
        const r = ringOf(sc, s, geo); if (!r) continue;
        ctx.globalAlpha = View.sel && View.sel.type === 'junction-site' && View.sel.id === s.id ? 1 : 0.6;
        ctx.beginPath(); ctx.arc(r.cx - off.x, r.cy - off.y, r.r, 0, Math.PI * 2); ctx.stroke();
      }
      ctx.setLineDash([]); ctx.globalAlpha = 1;
      if (View.tool === 'junctions-merge') for (const id of st.merge) { const p = geo.nodes.get(id); if (!p) continue; ctx.strokeStyle = T.amber || '#e39a00'; ctx.lineWidth = 3; ctx.beginPath(); ctx.arc(p.x - off.x, p.y - off.y, 11, 0, Math.PI * 2); ctx.stroke(); }
    },
    hit(X, Y, geo) {
      const sc = Store.sc; let best = null;
      for (const s of Object.values(sc.sites || {})) { const r = ringOf(sc, s, geo); if (!r) continue; const d = Math.abs(Math.hypot(X - r.cx, Y - r.cy) - r.r); if (d < 5 && (!best || d < best.d)) best = { type: 'junction-site', id: s.id, d }; }
      return best;
    },
    legend: () => `<div class="lg-row"><span class="lg-sw" style="background:transparent;border:1.5px dashed var(--sel)"></span><span>Junction site (click the ring)</span></div>` });

  // Merge sites tool: click junctions, then Enter (or double-click) to group them into one site
  Registry.tools({ id: 'junctions-merge', key: 'J', label: 'Merge junctions', group: 'edit', order: 60, cursor: 'pointer',
    iconSvgPath: '<circle cx="8" cy="12" r="3"/><circle cx="16" cy="12" r="3"/><circle cx="12" cy="12" r="9" stroke-dasharray="3 2"/>',
    hint: '<b>Merge junctions</b> · Click the nodes of one junction, then press Enter to make them one site. Esc clears.',
    enter() { st.merge = []; }, leave() { st.merge = []; },
    click(e) {
      const h = Render.hit(e.containerPoint, { zones: false, links: false, nodeR: 16 }); if (!h || h.type !== 'node') return;
      const sc = Store.sc, s = Sites.of(sc, h.id), ids = s ? s.nodes : [h.id], on = st.merge.includes(h.id);
      st.merge = on ? st.merge.filter(x => !ids.includes(x)) : [...new Set([...st.merge, ...ids])];
      Render.redraw();
    },
    dbl() { this.keydown({ key: 'Enter' }); },
    keydown(e) {
      if (e.key === 'Escape') { st.merge = []; Render.redraw(); return true; }
      if (e.key !== 'Enter') return false;
      if (st.merge.length < 2) { Kit.toast('Click two or more junction nodes first.', 'warn'); return true; }
      const sc = Store.sc; Store.snapshot(); const id = Sites.merge(sc, st.merge); st.merge = [];
      if (id) { Store.edited('net'); Kit.toast(`Made one junction site: ${Sites.name(sc, sc.sites[id])}. Run the model again.`); UI.setTool('select'); UI.select({ type: 'junction-site', id }); }
      return true;
    } });

  // Exports (Export submenu). 'junctions' replaces the built-in junction CSV with the spec's junction_movements.csv.
  // stamp: true asks the compliance feature to add its D1 / version footer rows to that human-readable table; the
  // SIDRA volumes and turning movements are machine hand-over files and stay unstamped (D1 and D12 travel in the
  // loader script and the volume grids). None of these carries author-written text (authorText).
  const hasRes = c => !!(c.sc && periodsOf(c.sc).length);
  Registry.exports({ id: 'junctions', label: 'Junction results (CSV)', ext: 'csv', mime: 'text/csv', order: 30, stamp: true, enabled: hasRes, filename: c => `${c.fileBase}-junction_movements.csv`, build: (P, c) => junctionMovementsCSV(P, c.sc) });
  Registry.exports({ id: 'junctions-turning', label: 'Turning movements (CSV)', ext: 'csv', mime: 'text/csv', order: 31, enabled: hasRes, filename: c => `${c.fileBase}-turning_movements.csv`, build: (P, c) => turningMovementsCSV(P, c.sc) });
  Registry.exports({ id: 'junctions-sidra-volumes', label: 'SIDRA volumes (CSV)', ext: 'csv', mime: 'text/csv', order: 60, enabled: hasRes, filename: () => 'corridor-sidra-volumes.csv', build: (P, c) => volumesCSV(c.sc, bothRes(c.sc), null, { scope: st.scope }) });
  Registry.exports({ id: 'junctions-sidra-loader', label: 'SIDRA 11 loader script (.py)', ext: 'py', mime: 'text/x-python', order: 61, enabled: hasRes, filename: () => 'load_corridor_volumes.py', build: (P, c) => loaderPy(c.sc, bothRes(c.sc), null, { scope: st.scope }) });
  Registry.exports({ id: 'junctions-volume-grids', label: 'Printable volume grids (HTML)', ext: 'html', mime: 'text/html', order: 62, enabled: hasRes, filename: c => `${c.fileBase}-sidra-volume-grids.html`, build: (P, c) => gridsHtml(c.sc, bothRes(c.sc), null, { scope: st.scope }) });

  Registry.commands({ id: 'junctions-open', label: 'Open junction results', group: 'Junctions', run: () => { st.view = 'summary'; UI.openTab('junctions'); } });
  Registry.commands({ id: 'junctions-sidra', label: 'Export to SIDRA', group: 'Junctions', run: () => { st.view = 'handoff'; UI.openTab('junctions'); } });

  // Settings › Junction analysis (the engine's settings.junction block; defaults come from defaultSettings)
  Registry.settings({ id: 'junctions', title: 'Junction analysis', order: 10,
    render: s => {
      const J = s.junction || {}, L = Engine.STANDARDS.los, v = k => J[k] ?? '';
      const num = (k, label, unit, step, extra = '') => `<label class="fld"><span>${label}</span><span class="fld-in"><input type="number" step="${step}" data-set="junction.${k}" value="${esc(v(k))}" ${extra}>${unit ? `<span class="unit">${unit}</span>` : ''}</span></label>`;
      return `<div class="set-grid">
        <label class="fld"><span>Level of service method</span><select data-set="junction.losMethod">${Object.values(L).map(m => `<option value="${esc(m.id)}" ${J.losMethod === m.id ? 'selected' : ''}>${esc(m.name)}</option>`).join('')}</select></label>
        <label class="fld"><span>Capacity method</span><select data-set="junction.capMethod"><option value="arc" ${J.capMethod !== 'movement' ? 'selected' : ''}>One capacity per approach (faster)</option><option value="movement" ${J.capMethod === 'movement' ? 'selected' : ''}>Per movement, with lanes (about 3–4× slower)</option></select></label>
        ${num('pff', 'Peak flow factor', '0–1', 0.01, 'min="0.5" max="1"')}${num('T', 'Analysis period', 'h', 0.25, 'min="0.25" max="2"')}${num('queueSpace', 'Queue space per vehicle', 'm', 0.5, 'min="4" max="15"')}</div>
        <p class="note">Peak flow factor 0.95 means the busiest 15 minutes run at the hourly flow ÷ 0.95 (${esc(Engine.STANDARDS.pff.edition)}). Values outside 0–1 fall back to 1. ${LABEL}.</p>`;
    } });
  Registry.about({ id: 'junctions', title: 'Junction analysis', order: 30, intro: 'How Spare Construct estimates junction performance. These are indicative screening results, not SIDRA output.',
    rows: () => {
      const ST = Engine.STANDARDS, v = x => (x.verified ? '<b>Checked.</b>' : '<b>To be checked</b> against the source.');
      const a = (u, t) => u ? `<a href="${esc(u)}" target="_blank" rel="noopener">${esc(t)}</a>` : esc(t);
      return [['Level of service', a(ST.los.hcm6.url, ST.los.hcm6.edition), `${v(ST.los.hcm6)} DoS above 1 is F. Give way and stop report the worst movement.`],
        ['Practical degree of saturation', a(ST.practicalDoS.url, ST.practicalDoS.edition), `${v(ST.practicalDoS)} Signals ${ST.practicalDoS.signals}, roundabouts ${ST.practicalDoS.roundabout}, give way ${ST.practicalDoS.sign}.`],
        ['Movement capacity', a(ST.gaps.url, ST.gaps.edition), `${v(ST.gaps)} Used when Capacity method is per movement.`],
        ['Back of queue', a(ST.queue.url, ST.queue.edition), `${v(ST.queue)} ${esc(ST.queue.note)}`],
        ['Queue storage ratio', a(STANDARDS.storage.url, STANDARDS.storage.edition), `${v(STANDARDS.storage)} Amber above ${STANDARDS.storage.amber}, red above ${STANDARDS.storage.red}.`],
        ['Flow-scale test', a(STANDARDS.flowScale.url, STANDARDS.flowScale.edition), `${v(STANDARDS.flowScale)} Flows × ${STANDARDS.flowScale.factor.toFixed(2)}, junctions only.`],
        ['SIDRA hand-off', a(STANDARDS.sidra.url, STANDARDS.sidra.edition), `${v(STANDARDS.sidra)} Leg and turn codes follow SIDRA drive-on-left conventions; the loader is untested against a live install.`]];
    } });

  // Report sections (js/packs/nz/report.js builds the document from Registry.list('report'))
  /* The scenarios a report section covers: the report's assessed set (ctx.sel, else Report.pick: existing, without,
     with, mitigation), so a sensitivity or option run the author did not choose stays out. Comparisons run without →
     with and with → mitigation (existing → with when there is no same-year without). ctx.scenarios, when given, and a
     project with no report module list every run scenario, compared with the base. */
  const hasRun = s => !!(s && periodsOf(s).length);
  function reportSel(P, ctx) {
    let sel = ctx && !Array.isArray(ctx.scenarios) ? ctx.sel : null;
    if (!sel && !(ctx && Array.isArray(ctx.scenarios)) && typeof Report !== 'undefined' && Report.pick) { try { sel = Report.pick(P); } catch { sel = null; } }
    if (!sel) {
      const list = (ctx && Array.isArray(ctx.scenarios) ? ctx.scenarios : P.scenarios).filter(hasRun), b = list.find(s => s.id === P.base) || list[0];
      return { list, pairs: list.filter(s => s !== b).map(s => [b, s, 'Change from the base scenario', 'base']) };
    }
    // each pair: [from, to, heading, kind]; kind 'development' is the without → with pair the report's own Table J3 shows
    const list = [...new Set([sel.existing, sel.without, sel.with, sel.mitigation])].filter(hasRun), pairs = [];
    if (hasRun(sel.without) && hasRun(sel.with)) pairs.push([sel.without, sel.with, 'Effect of the development', 'development']);
    else if (hasRun(sel.existing) && hasRun(sel.with) && sel.existing !== sel.with) pairs.push([sel.existing, sel.with, 'Change from the existing network', 'existing']);
    if (hasRun(sel.with) && hasRun(sel.mitigation)) pairs.push([sel.with, sel.mitigation, 'Effect of the mitigation', 'mitigation']);
    return { list, pairs };
  }
  const reportScs = (P, ctx) => reportSel(P, ctx).list;
  // The junctions worth a full table: extent-of-effects sites, else the worst by delay (max n)
  function keyJunctions(sc, n = 8) {
    const res = resOf(sc, 'AM') || resOf(sc, 'PM') || resOf(sc, periodsOf(sc)[0]); if (!res) return [];
    const ext = sc.extent && Array.isArray(sc.extent.sites) && sc.extent.sites.length ? sc.extent.sites : null;
    const es = entries(sc, res);
    return (ext ? es.filter(e => ext.includes(e.id) || e.nodes.some(x => ext.includes(x))) : es.slice().sort((a, b) => (b.nr.delay || 0) - (a.nr.delay || 0))).slice(0, n).map(e => e.id);
  }
  /* The report's own section 4.2.3 (report-junctions, js/packs/nz/report.js) prints Table J2 per scenario, Table J3
     without → with, the queue storage check, the level of service definitions and D3. These sections add only what it
     does not: the TTOC-06 flow-scale test, the turn-level movement summaries (Appendix C, after its approach-level
     tables), the comparisons J3 leaves out (existing → with, with → mitigation) and the turning diagrams. */
  const PCT = Math.round((STANDARDS.flowScale.factor - 1) * 100), PERF_TITLE = `Junction flow-scale test (+${PCT}%)`;
  Registry.report({ id: 'junctions-performance', chapter: 'Network performance', order: 40, title: PERF_TITLE,
    render: (P, ctx) => {
      const scs = reportScs(P, ctx); if (!scs.length) return '';
      const scale = scs.map(sc => periodsOf(sc).map(p => { const r = resOf(sc, p) ? flowScale(sc, resOf(sc, p)) : null; return r && r.crossing.length ? `<li>${esc(sc.name)}, ${esc(perName(p))}: ${r.crossing.map(x => esc(`${x.name} (${x.first.where})`)).join('; ')}</li>` : ''; }).join('')).join('');
      return `<h3>${esc(PERF_TITLE)}</h3>${suppNote(P)}
        ${scale ? `<p>Junctions whose degree of saturation crosses the practical limit when flows rise by ${PCT}%:</p><ul>${scale}</ul>` : `<p>No junction crosses its practical degree of saturation when flows rise by ${PCT}%.</p>`}
        <p class="note">${esc(STANDARDS.flowScale.name)}: ${esc(STANDARDS.flowScale.edition)}. Junctions are re-evaluated at flows × ${STANDARDS.flowScale.factor.toFixed(2)} with each run's signal timings; routes are not re-assigned. ${LABEL}.</p>`;
    } });
  Registry.report({ id: 'junctions-movement-summary', chapter: 'C', order: 20, title: 'Movement summaries by turn',
    render: (P, ctx) => {
      const scs = reportScs(P, ctx); if (!scs.length) return '';
      return `<h3>Movement summaries by turn</h3><p>SIDRA-style Movement Summary tables, turn by turn, for the key junctions (the extent-of-effects sites, or the most delayed). ${LABEL}.</p>
        ${scs.map(sc => keyJunctions(sc).map(id => periodsOf(sc).map(p => { const res = resOf(sc, p); return res ? `<h4>${esc(sc.name)} · ${esc((entries(sc, res).find(e => e.id === id) || {}).name || id)} · ${esc(perName(p))}</h4>${j1Html(j1(sc, res, id), { head: false, P })}` : ''; }).join('')).join('')).join('')}`;
    } });
  Registry.report({ id: 'junctions-comparison', chapter: 'Network performance', order: 42, title: 'Junction comparison',
    render: (P, ctx) => {
      const pairs = reportSel(P, ctx).pairs.filter(x => x[3] !== 'development'); if (!pairs.length) return '';
      return `<h3>Junction comparison</h3>${pairs.map(([a, b, what]) => `<h4>${esc(what)}: ${esc(a.name)} to ${esc(b.name)}</h4>${j3Html(a, b, undefined, { P }).replace(/ data-jx-fly="[^"]*"/g, '')}`).join('')}`;
    } });
  Registry.report({ id: 'junctions-turning', chapter: 'Network performance', order: 43, title: 'Turning movement diagrams',
    render: (P, ctx) => {
      const scs = reportScs(P, ctx); if (!scs.length) return '';
      return `<h3>Turning movement diagrams</h3><p>Modelled peak-hour turning volumes, AM (PM), rounded to the nearest ${STANDARDS.figure.step} veh/h. ${LABEL}.</p>
        ${scs.map(sc => keyJunctions(sc).map(id => `<figure style="display:inline-block;margin:6px;break-inside:avoid">${figure(sc, resOf(sc, 'AM'), resOf(sc, 'PM'), id)}<figcaption>${esc(sc.name)}</figcaption></figure>`).join('')).join('')}`;
    } });

  return { LABEL, STANDARDS, D1, D3, D12, suppText, setBans, entries, entryFor, nodeLegs, j1, j1Html, j2, j2Html, j3, j3Html, summary, footer, flowScale, scaleHtml, checks, losFrom, losTable,
    junctionMovementsCSV, turningMovementsCSV, turnCountFile, volumeRows, volumesCSV, parseVolumesCSV, asCountCSV, loaderPy, upstreamLen, xid, compareOrder, reportSel, gridsHtml, figure, moves, banSvg, toggleBan, approaches, editLanes,
    mergeCandidates, mergeInto, splitSite, detectSites, sitePanel, tabHtml, keyJunctions, gapsHtml, editGap, periodsOf, perName, phfOf, JM_HEAD, TM_HEAD, VOL_HEAD, state: st };
})();
