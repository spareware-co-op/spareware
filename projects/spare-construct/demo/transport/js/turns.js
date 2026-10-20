'use strict';
/* Spare Construct · junction outputs (contract C5): turning movement tables, a junction diagram (SVG) and
   SIDRA-friendly CSV, all from the assignment's res.turns (contract C1).
   Left-hand traffic: arriving traffic keeps to the left of each arm and a right turn crosses the
   opposing flow. An approach is named after the side traffic comes FROM (heading north = South approach).
   Additions beyond C5: rows also carry toName, lv and side; Turns.arms, Turns.classify, Turns.sideName
   (outward arm bearing) and Turns.approachName (arrival heading, as Engine.approachName) are exposed for
   the UI. Light vehicles = total − heavy, rounded so they add up. Zone connector movements (development
   accesses) come from the engine and are drawn as dashed arms named after the zone.
   v2: rows carry the NZ / SIDRA leg (leg, legName, legLabel 'South: Great South Road', toLeg), turn designation
   (turn, 'L2') and movement ID ('S-L2'), and the development share (dev, base) when the engine tracked it;
   junction sites (res.sites) list movements through the whole site; sidraExport refuses unconverged runs. */

const Turns = (() => {
  const MV = ['L', 'T', 'R', 'U'];
  const N4 = ['North', 'East', 'South', 'West'];
  const N8 = ['North', 'North-east', 'East', 'South-east', 'South', 'South-west', 'West', 'North-west'];
  const settings = () => (typeof Store !== 'undefined' && Store.P ? Store.S : defaultSettings());
  const norm = b => ((b % 360) + 360) % 360;
  // The compass side of an arm from its outward bearing; approachName takes the arrival heading instead
  const sideName = (b, eight) => eight ? N8[Math.round(norm(b) / 45) % 8] : N4[Math.round(norm(b) / 90) % 4];
  const approachName = (inB, eight) => sideName(+inB + 180, eight);
  // Same thresholds as Engine.turnType (C1); used only when a turn record has no valid mv
  function classify(inB, outB) {
    if (typeof Engine !== 'undefined' && Engine.turnType) return Engine.turnType(inB, outB);
    const t = U.turn(inB, outB), a = Math.abs(t);
    return a <= 30 ? 'T' : a > 150 ? 'U' : t > 0 ? 'R' : 'L';
  }
  const roadName = L => L.name || (L.cls === 'ramp' ? 'Unnamed ramp' : 'Unnamed ' + (CLASSES[L.cls] || CLASSES.local).label.toLowerCase() + ' road');

  /* ---------- NZ / SIDRA conventions (docs/spec/nz-conventions) ----------
     A leg is named by where traffic comes FROM: a northbound approach is the South leg. Leg bearing = the arrival
     heading + 180°, snapped to 45° bins coded S 0, SE 1, E 2, NE 3, N 4, NW 5, W 6, SW 7; two legs never share a code
     (greedy minimum angular error), and past eight legs the least important (driveways, zone accesses) merge into
     the nearest leg. Turn designation t = (o_in − o_out) mod 8: 0 U, 1 L3, 2 L2, 3 L1, 4 T1, 5 R1, 6 R2, 7 R3, as
     SIDRA's drive-on-left ConvertMovODToMovTurn; L/T/R/U come from the first letter. Movement ID 'S-R2'.
     Priority ranks (HCM mirrored for left-hand traffic, NZ 2012 rules): rank 1 major T and L; rank 2 major R
     (opposed by the opposing T + L) and minor L; rank 3 minor T; rank 4 minor R. Deviation: the spec names a new
     js/conventions.js; these live here in Turns, which every export and table already uses. */
  const LEG_CODES = ['S', 'SE', 'E', 'NE', 'N', 'NW', 'W', 'SW'];
  const LEG_NAMES = ['South', 'South-east', 'East', 'North-east', 'North', 'North-west', 'West', 'South-west'];
  const TURN_CODES = ['U', 'L3', 'L2', 'L1', 'T1', 'R1', 'R2', 'R3'];
  const binBearing = o => norm(180 - 45 * o);
  const legCodeOf = legB => Math.round(norm(180 - legB) / 45) % 8;
  // Leg codes for outward leg bearings; prio (higher = more important) decides who keeps a code past eight legs
  function assignLegs(bearings, prio = []) {
    const n = bearings.length, res = bearings.map(() => null), order = bearings.map((b, i) => i).sort((p, q) => (prio[q] || 0) - (prio[p] || 0));
    const main = order.slice(0, 8), pairs = [];
    for (const i of main) for (let o = 0; o < 8; o++) pairs.push({ i, o, e: U.angDiff(bearings[i], binBearing(o)) });
    pairs.sort((p, q) => p.e - q.e || p.i - q.i);
    const used = new Set();
    for (const p of pairs) if (!res[p.i] && !used.has(p.o)) { res[p.i] = { o: p.o, err: Math.round(p.e), merged: false }; used.add(p.o); }
    for (const i of order.slice(8)) {   // more than eight legs: join the nearest coded leg
      let best = main[0];
      for (const j of main) if (U.angDiff(bearings[i], bearings[j]) < U.angDiff(bearings[i], bearings[best])) best = j;
      res[i] = { o: res[best].o, err: Math.round(U.angDiff(bearings[i], binBearing(res[best].o))), merged: true, into: best };
    }
    return n ? res : [];
  }
  const turnCode = (oIn, oOut) => TURN_CODES[(((oIn - oOut) % 8) + 8) % 8];
  const movementId = (oIn, oOut) => LEG_CODES[oIn] + '-' + turnCode(oIn, oOut);
  const legLabel = (o, road) => `${LEG_NAMES[o]}${road ? ': ' + road : ''}`;
  // Priority rank of a movement (L/T/R/U) from a major or minor approach
  const rank = (mv, major) => (major ? (mv === 'T' || mv === 'L' ? 1 : 2) : mv === 'L' ? 2 : mv === 'T' ? 3 : 4);
  // Codes onto arms (outward bearing a.b): a.o, a.leg, a.legName, a.legLabel
  function codeArms(A) {
    const pr = A.map(a => (a.conn ? 0 : a.link ? (CLASSES[a.link.cls] || CLASSES.local).rank : 1));
    assignLegs(A.map(a => a.b), pr).forEach((c, i) => { const a = A[i]; a.o = c.o; a.leg = LEG_CODES[c.o]; a.legName = LEG_NAMES[c.o]; a.legLabel = legLabel(c.o, a.road); a.legMerged = c.merged; });
    return A;
  }
  // Bearings over the first / last 20 m of a polyline (as the engine measures arc bearings)
  function endB(pts, last) {
    const c = U.cumLen(pts), len = c[c.length - 1], e = Math.min(20, len / 2);
    return last ? U.bearing(U.along(pts, c, len - e).p, pts[pts.length - 1]) : U.bearing(pts[0], U.along(pts, c, e).p);
  }
  // Heavy vehicles from PCU; where PCU carries no heavy-vehicle information (PCU per HV of 1 or less, or
  // no pcu value) the heavy-vehicle share setting is used instead
  function hvOf(v, pcu, S) {
    if (!(v > 0)) return 0;
    if (!(S.pcuHV > 1) || pcu == null) return U.clamp(v * (+S.hvPct || 0) / 100, 0, v);
    return U.clamp(v * (pcu / v - 1) / (S.pcuHV - 1), 0, v);
  }

  /* ---------- arms ---------- */
  // Each arm is a link (or a zone connector) touching the node, with its outward bearing from the node.
  // inKey arrives at the node along the arm, outKey leaves along it. Names follow the side of the arm;
  // 8-point names are used when two approaches share a quadrant, and the road name is added if that still clashes.
  function arms(sc, nodeId, list, links) {
    const n = sc && sc.nodes[nodeId];
    if (!n) return [];
    list = list || [];
    const used = new Set(); for (const t of list) { used.add(t.from); used.add(t.to); }
    const out = [];
    for (const L of links || Net.linksAt(sc, nodeId)) {
      if (L.a === L.b || !sc.nodes[L.a] || !sc.nodes[L.b]) continue;
      const atA = L.a === nodeId, inKey = L.id + (atA ? ':ba' : ':ab'), outKey = L.id + (atA ? ':ab' : ':ba');
      if (L.status === 'closed' && !used.has(inKey) && !used.has(outKey)) continue;
      const g = Net.geom(sc, L), pts = atA ? g : g.slice().reverse();
      const lin = (atA ? L.lanesBA : L.lanesAB) | 0, lout = (atA ? L.lanesAB : L.lanesBA) | 0;
      out.push({ id: L.id, link: L, conn: false, road: roadName(L), b: endB(pts, false), inKey, outKey,
        isIn: lin > 0 || used.has(inKey), isOut: lout > 0 || used.has(outKey) });
    }
    const seen = new Set();
    for (const k of used) {
      if (!k || !k.startsWith('c:')) continue;
      const [, zid, nid] = k.split(':'), id = `c:${zid}:${nid}`, z = sc.zones[zid];
      if (nid !== nodeId || !z || seen.has(id)) continue;
      seen.add(id);
      out.push({ id, link: null, conn: true, road: z.name || 'Zone', b: U.bearing([n.lat, n.lng], [z.lat, z.lng]),
        inKey: id + ':o', outKey: id + ':d', isIn: used.has(id + ':o'), isOut: used.has(id + ':d') });
    }
    const appr = out.filter(a => a.isIn);
    const clash = eight => { const s = new Set(); return appr.some(a => { const k = sideName(a.b, eight); return s.has(k) || !s.add(k); }); };
    const eight = clash(false);
    for (const a of out) { a.side = a.b; a.name = sideName(a.b, eight); }
    const dupes = () => { const cnt = {}; for (const a of appr) cnt[a.name] = (cnt[a.name] || 0) + 1; return cnt; };
    if (eight && clash(true)) { const cnt = dupes(); for (const a of out) if (cnt[a.name] > 1) a.name += ` (${a.road})`; }
    // Still the same (two same-named ramps from one side): number them clockwise so every approach is distinct
    const cnt = dupes(), seq = {};
    for (const a of out.slice().sort((p, q) => norm(p.b) - norm(q.b))) if (a.isIn && cnt[a.name] > 1) a.name += ' ' + (seq[a.name] = (seq[a.name] || 0) + 1);
    // Clockwise from North, starting half a sector west of North so the North arm comes first
    const off = eight ? 22.5 : 45;
    return out.sort((p, q) => norm(p.b + off) - norm(q.b + off));
  }

  /* ---------- movements ---------- */
  function rowsFor(sc, res, nodeId, links) {
    const list = res && res.turns && res.turns[nodeId];
    if (!sc || !sc.nodes[nodeId] || !Array.isArray(list)) return { rows: [], A: arms(sc, nodeId, [], links) };
    const S = settings(), pf = 1 + S.hvPct / 100 * (S.pcuHV - 1);
    const A = codeArms(arms(sc, nodeId, list, links)), byIn = new Map(), byOut = new Map();
    A.forEach((a, i) => { a.order = i; byIn.set(a.inKey, a); byOut.set(a.outKey, a); });
    const inB = new Map();
    const arriving = key => {
      if (!inB.has(key)) {
        let b = null;
        if (key.startsWith('c:')) { const a = byIn.get(key); b = a ? norm(a.b + 180) : null; }
        else {
          const i = key.lastIndexOf(':'), L = sc.links[key.slice(0, i)];
          if (L && sc.nodes[L.a] && sc.nodes[L.b]) { const g = Net.geom(sc, L); b = endB(key.endsWith(':ab') ? g : g.slice().reverse(), true); }
        }
        inB.set(key, b);
      }
      return inB.get(key);
    };
    const rows = [];
    for (const t of list) {
      const a = byIn.get(t.from), x = byOut.get(t.to);
      if (!a || !x) continue;                 // arc no longer at this node (stale result) or malformed
      const b = arriving(t.from);
      if (b == null) continue;
      const v = Math.max(0, +t.v || 0), pcu = isFinite(+t.pcu) && t.pcu != null ? Math.max(0, +t.pcu) : v * pf, hv = hvOf(v, pcu, S);
      const turn = U.turn(b, x.b), dev = Math.min(v, Math.max(0, +t.dev || 0));
      rows.push({ approach: t.from, approachName: a.name, road: a.road, mv: MV.includes(t.mv) ? t.mv : classify(b, x.b),
        to: t.to, toRoad: x.road, toName: x.name, v, pcu, hv, lv: v - hv, share: 0, side: a.side,
        leg: a.leg, legName: a.legName, legLabel: a.legLabel, toLeg: x.leg, turn: turnCode(a.o, x.o), id: movementId(a.o, x.o), dev, base: v - dev, _o: a.order, _t: turn });
    }
    const tot = {}; for (const r of rows) tot[r.approach] = (tot[r.approach] || 0) + r.v;
    for (const r of rows) r.share = tot[r.approach] > 0 ? r.v / tot[r.approach] : 0;
    rows.sort((p, q) => p._o - q._o || MV.indexOf(p.mv) - MV.indexOf(q.mv) || p._t - q._t);
    for (const r of rows) { delete r._o; delete r._t; }
    return { rows, A };
  }
  /* Movements of a junction site (res.sites / res.siteTurns from the engine): legs are the site's legs (both
     carriageways of a divided road on one side are one leg), movements run from entry to exit through the site. The
     engine traces them along the assignment's paths (res.siteTurnsBasis 'paths') and gives their development share (dev). */
  function siteRows(sc, res, siteId) {
    const site = res && res.sites && res.sites[siteId], list = (res && res.siteTurns && res.siteTurns[siteId]) || [];
    if (!site) return { rows: [], A: [] };
    const S = settings(), pf = 1 + S.hvPct / 100 * (S.pcuHV - 1);
    const A = site.legs.map(l => ({ id: 'leg' + l.k, k: l.k, b: l.b, road: l.name || 'Unnamed road', link: null, conn: false, ins: l.ins, outs: l.outs, isIn: l.ins.length > 0, isOut: l.outs.length > 0 }));
    const pr = A.map(a => Math.max(0, ...[...a.ins, ...a.outs].map(k => { const L = sc && sc.links[k.slice(0, k.lastIndexOf(':'))]; return L ? (CLASSES[L.cls] || CLASSES.local).rank : 1; })));
    assignLegs(A.map(a => a.b), pr).forEach((c, i) => Object.assign(A[i], { o: c.o, leg: LEG_CODES[c.o], legName: LEG_NAMES[c.o], legLabel: legLabel(c.o, A[i].road), name: LEG_NAMES[c.o], side: A[i].b }));
    const inLeg = new Map(), outLeg = new Map();
    A.forEach(a => { a.ins.forEach(k => inLeg.set(k, a)); a.outs.forEach(k => outLeg.set(k, a)); });
    const rows = [];
    for (const t of list) {
      const a = inLeg.get(t.from), x = outLeg.get(t.to); if (!a || !x) continue;
      const v = Math.max(0, +t.v || 0), pcu = t.pcu != null && isFinite(+t.pcu) ? +t.pcu : v * pf, hv = hvOf(v, pcu, S), dev = Math.min(v, Math.max(0, +t.dev || 0));
      rows.push({ approach: t.from, approachName: a.legName, road: a.road, mv: MV.includes(t.mv) ? t.mv : 'T', to: t.to, toRoad: x.road, toName: x.legName, v, pcu, hv, lv: v - hv, share: 0, side: a.b,
        leg: a.leg, legName: a.legName, legLabel: a.legLabel, toLeg: x.leg, turn: turnCode(a.o, x.o), id: movementId(a.o, x.o), dev, base: v - dev, site: siteId });
    }
    const tot = {}; for (const r of rows) tot[r.leg] = (tot[r.leg] || 0) + r.v;
    for (const r of rows) r.share = tot[r.leg] > 0 ? r.v / tot[r.leg] : 0;
    rows.sort((p, q) => LEG_CODES.indexOf(p.leg) - LEG_CODES.indexOf(q.leg) || MV.indexOf(p.mv) - MV.indexOf(q.mv));
    return { rows, A, site };
  }
  // Rows for a node, or for a junction site when id is a site id in res.sites
  const movements = (sc, res, id) => (res && res.sites && res.sites[id] ? siteRows(sc, res, id).rows : rowsFor(sc, res, id).rows);

  function junctionName(sc, nodeId, links) {
    const n = sc && sc.nodes[nodeId];
    if (!n) return 'Unknown junction';
    if (n.name) return n.name;
    const ls = (links || Net.linksAt(sc, nodeId)).filter(L => L.status !== 'closed' && L.name);
    ls.sort((p, q) => ((CLASSES[q.cls] || CLASSES.local).rank - (CLASSES[p.cls] || CLASSES.local).rank) || p.name.localeCompare(q.name));
    const names = [...new Set(ls.map(L => L.name))];
    return names.length ? names.slice(0, 3).join(' / ') : 'Unnamed junction';
  }

  // Why a junction has nothing to show, and what to do about it
  function emptyText(sc, res, nodeId) {
    if (!res || !res.turns) return 'Run the model to see turning volumes.';
    if (res.stale) return 'Results are out of date. Run the model again.';
    if (!res.turns[nodeId]) return (sc && Net.linksAt(sc, nodeId).length < 3) ? 'Not a junction (fewer than three roads), so no turns.' : 'No traffic turns here in this period.';
    return 'No traffic turns here in this period.';
  }

  /* ---------- diagram ---------- */
  const f1 = x => Math.round(x * 10) / 10;
  const byKey = (k, a) => a.inKey === k;
  function diagram(sc, res, nodeId, opts = {}) {
    const size = Math.max(120, +opts.size || 240), h = size / 2, aw = size * 0.17;
    // opts.layer: 'total' (default), 'base' (without development traffic) or 'dev' (development traffic only), so
    // Base + Development = Total diagrams can be drawn side by side (dev-tracking)
    const layer = opts.layer === 'dev' || opts.layer === 'base' ? opts.layer : 'total', R0 = rowsFor(sc, res, nodeId), A = R0.A;
    const rows = layer === 'total' ? R0.rows : R0.rows.map(r => ({ ...r, v: r[layer] || 0 }));
    const name = sc && sc.nodes[nodeId] ? junctionName(sc, nodeId) : 'Unknown junction';
    const period = (res && res.period ? res.period + ' peak' : '') + (layer === 'dev' ? ', development traffic' : layer === 'base' ? ', without development traffic' : '');
    // Ids unique per scenario, junction and period; everything outside [A-Za-z0-9_-] is dropped (res may come from a file)
    const uid = ('tj' + (sc && sc.id || '') + '-' + nodeId + '-' + (res && res.period || '') + (layer === 'total' ? '' : '-' + layer) + (opts.idSuffix || '')).replace(/[^\w-]/g, '');
    const title = `${name}${period ? ' · ' + period : ''} turning volumes (veh/h)`;
    const desc = A.filter(a => a.isIn).map(a => {
      const rs = rows.filter(r => byKey(r.approach, a));
      return `${a.name} approach (${a.road}): ` + (rs.length ? rs.map(r => `${r.mv} ${Math.round(r.v)} to ${r.toRoad}`).join(', ') : 'no traffic');
    }).join('. ');
    const fs = Math.max(8, size / 26), font = `font:600 ${f1(fs)}px var(--f-cond, sans-serif)`;
    const P = (r, u, nrm, d) => [h + u[0] * r + nrm[0] * d, h + u[1] * r + nrm[1] * d];
    const vec = b => [Math.sin(b * Math.PI / 180), -Math.cos(b * Math.PI / 180)];   // screen x right, y down
    const s = [`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}" width="${size}" height="${size}" role="img" aria-labelledby="${uid}-t ${uid}-d">`,
      `<title id="${uid}-t">${U.esc(title)}</title><desc id="${uid}-d">${U.esc(desc || 'No turning volumes.')}</desc>`];
    // Road bands: one translucent group so overlaps read as a single surface
    s.push(`<g style="stroke:var(--muted);opacity:.2" stroke-linecap="butt">`);
    for (const a of A) {
      const u = vec(a.b), e = P(h - 2, u, [0, 0], 0);
      s.push(`<line x1="${f1(h)}" y1="${f1(h)}" x2="${f1(e[0])}" y2="${f1(e[1])}" stroke-width="${f1(a.conn ? aw * 0.45 : a.isIn && a.isOut ? aw : aw * 0.6)}"${a.conn ? ' stroke-dasharray="6 4"' : ''}/>`);
    }
    if (A.length) s.push(`<circle cx="${f1(h)}" cy="${f1(h)}" r="${f1(aw * 0.5)}" style="fill:var(--muted);stroke:none"/>`);
    s.push('</g>');
    // Centre lines on two-way arms
    for (const a of A) if (a.isIn && a.isOut && !a.conn) {
      const u = vec(a.b), p = P(aw * 0.9, u, [0, 0], 0), q = P(h - 2, u, [0, 0], 0);
      s.push(`<line x1="${f1(p[0])}" y1="${f1(p[1])}" x2="${f1(q[0])}" y2="${f1(q[1])}" style="stroke:var(--muted)" stroke-width="1" stroke-dasharray="4 4" opacity=".6"/>`);
    }
    // Road names along each arm, on the departure side, kept upright
    for (const a of A) {
      const u = vec(a.b), dep = [u[1], -u[0]];            // left of outward travel = departure side
      const p = P(h * 0.71, u, dep, aw / 2 + fs * 0.45);
      let ang = a.b - 90; if (norm(ang) > 90 && norm(ang) < 270) ang += 180;
      const max = Math.max(4, Math.floor(h * 0.58 / (fs * 0.5))), lbl = a.road.length > max ? a.road.slice(0, max - 1) + '…' : a.road;
      s.push(`<text x="${f1(p[0])}" y="${f1(p[1])}" transform="rotate(${f1(norm(ang))} ${f1(p[0])} ${f1(p[1])})" text-anchor="middle" dominant-baseline="middle" style="fill:var(--muted);${font}">${U.esc(lbl)}</text>`);
    }
    // Movement arrows: start on the arrival lane (left of inbound travel), end on the exit arm's departure lane
    const vmax = Math.max(1, ...rows.map(r => r.v)), R1 = h * 0.42, LANE = { L: 0.38, T: 0.25, R: 0.13, U: 0.05 };
    const armOf = new Map(A.map(a => [a.outKey, a]));
    for (const r of rows) {
      if (r.v < 0.5) continue;
      const a = A.find(x => byKey(r.approach, x)), x = armOf.get(r.to);
      if (!a || !x) continue;
      const ui = vec(a.b), uo = vec(x.b), nIn = [-ui[1], ui[0]], nOut = [uo[1], -uo[0]];
      // A one-way arm is a narrower band with no opposing half, so its lanes spread across the whole band
      const off = a.isOut ? aw * LANE[r.mv] : aw * 0.6 * (LANE[r.mv] - 0.215);
      const S0 = P(R1, ui, nIn, off), E = P(R1, uo, nOut, x.isIn ? aw * 0.25 : aw * 0.6 * 0.035), k = R1 * (r.mv === 'U' ? 0.9 : 0.75);
      const C1 = [S0[0] - ui[0] * k, S0[1] - ui[1] * k], C2 = [E[0] - uo[0] * k, E[1] - uo[1] * k];
      const w = 1.2 + 5.3 * r.v / vmax, hl = 3 + w * 1.1, hw = 2 + w * 0.9, tip = [E[0] + uo[0] * hl, E[1] + uo[1] * hl];
      s.push(`<g style="stroke:var(--accent);fill:var(--accent)"><path d="M${f1(S0[0])} ${f1(S0[1])}C${f1(C1[0])} ${f1(C1[1])} ${f1(C2[0])} ${f1(C2[1])} ${f1(E[0])} ${f1(E[1])}" fill="none" stroke-width="${f1(w)}" stroke-linecap="round" opacity=".85"/>`
        + `<polygon points="${f1(tip[0])},${f1(tip[1])} ${f1(E[0] + nOut[0] * hw)},${f1(E[1] + nOut[1] * hw)} ${f1(E[0] - nOut[0] * hw)},${f1(E[1] - nOut[1] * hw)}" stroke="none"/></g>`);
    }
    // Volumes beside each approach's arrival lane
    for (const a of A) {
      const rs = rows.filter(r => byKey(r.approach, a) && r.v >= 0.5);
      if (!rs.length) continue;
      const by = {}; for (const r of rs) by[r.mv] = (by[r.mv] || 0) + r.v;
      const lines = MV.filter(m => by[m] != null).map(m => `${m} ${Math.round(by[m])}`);
      const u = vec(a.b), nIn = [-u[1], u[0]], p = P(h * 0.6, u, nIn, (a.isOut ? aw / 2 : aw * 0.3) + fs * 0.4);
      const anchor = nIn[0] > 0.35 ? 'start' : nIn[0] < -0.35 ? 'end' : 'middle', lh = fs * 1.15, bh = lines.length * lh;
      const top = nIn[1] > 0.35 ? p[1] : nIn[1] < -0.35 ? p[1] - bh : p[1] - bh / 2;
      s.push(`<text text-anchor="${anchor}" style="fill:var(--ink);${font}">` + lines.map((t, i) => `<tspan x="${f1(p[0])}" y="${f1(top + (i + 0.8) * lh)}">${t}</tspan>`).join('') + '</text>');
    }
    // North point
    const nx = size - fs * 1.2, ny = fs * 2.6;
    s.push(`<g style="fill:var(--muted)"><polygon points="${f1(nx)},${f1(ny - fs * 1.4)} ${f1(nx - fs * 0.45)},${f1(ny - fs * 0.3)} ${f1(nx + fs * 0.45)},${f1(ny - fs * 0.3)}"/><text x="${f1(nx)}" y="${f1(ny + fs * 0.7)}" text-anchor="middle" style="${font}">N</text></g>`);
    if (!A.length) s.push(`<text x="${f1(h)}" y="${f1(h)}" text-anchor="middle" style="fill:var(--muted);${font}">No junction here</text>`);
    else if (!rows.some(r => r.v >= 0.5)) s.push(`<text x="${f1(h)}" y="${f1(size - fs)}" text-anchor="middle" style="fill:var(--muted);${font}">${U.esc(emptyText(sc, res, nodeId))}</text>`);
    s.push('</svg>');
    return s.join('');
  }

  /* ---------- CSV ---------- */
  const SIDRA_HEAD = ['Site', 'Junction', 'Approach', 'Approach road', 'Movement', 'To road', 'Light vehicles (veh/h)', 'Heavy vehicles (veh/h)', 'Total (veh/h)', 'Period', 'Scenario'];
  // Rounds total and heavy vehicles to whole vehicles; light = total − heavy so each row adds up
  const split = r => { const t = Math.round(r.v), hv = Math.min(t, Math.round(r.hv)); return [t - hv, hv, t]; };
  function linkIndex(sc) {
    const m = new Map();
    for (const L of Object.values(sc.links)) for (const k of L.a === L.b ? [L.a] : [L.a, L.b]) { let a = m.get(k); if (!a) m.set(k, a = []); a.push(L); }
    return m;
  }
  /* res may be one period's result or an array of them (e.g. [AM, PM]). nodeIds: node ids and / or junction-site
     ids (site rows use the site's legs and its movements through the site). Without nodeIds the export covers the
     sites included by the extent test (sc.extent.sites, from Engine.extentTest) when there are any, otherwise every
     junction in res.turns; opts.all forces every junction. opts.codes adds SIDRA leg, turn designation and
     movement ID columns (South leg, R2, S-R2). */
  const CODE_HEAD = ['Leg', 'Turn designation', 'Movement ID'];
  function sidraCSV(sc, res, nodeIds, opts = {}) {
    const out = [opts.codes ? SIDRA_HEAD.concat(CODE_HEAD) : SIDRA_HEAD];
    if (!sc) return U.csv(out);
    const idx = linkIndex(sc), ext = !nodeIds && !opts.all && sc.extent && Array.isArray(sc.extent.sites) && sc.extent.sites.length ? sc.extent.sites : null;
    const want = nodeIds || ext;
    for (const r of [].concat(res || []).filter(Boolean)) {
      if (!r.turns) continue;
      const isSite = id => !!(r.sites && r.sites[id]);
      const names = new Map(), nm = id => names.has(id) ? names.get(id) : (names.set(id, isSite(id) ? r.sites[id].name : junctionName(sc, id, idx.get(id) || [])), names.get(id));
      const ids = (want || Object.keys(r.turns)).filter(id => isSite(id) || (sc.nodes[id] && r.turns[id]));
      if (!want) ids.sort((p, q) => nm(p).localeCompare(nm(q)) || (p < q ? -1 : p > q ? 1 : 0));
      for (const id of ids) {
        const rows = isSite(id) ? siteRows(sc, r, id).rows : rowsFor(sc, r, id, idx.get(id) || []).rows;
        for (const m of rows) out.push([id, nm(id), m.approachName, m.road, m.mv, m.toRoad, ...split(m), r.period || '', sc.name || '', ...(opts.codes ? [m.legName, m.turn, m.id] : [])]);
      }
    }
    return U.csv(out);
  }
  // Export gate (docs/spec/convergence): turn volumes for SIDRA need a well-converged run. Returns { ok, why, csv };
  // with opts.force the CSV is made anyway and why still explains the problem.
  function sidraExport(sc, res, nodeIds, opts = {}) {
    const list = [].concat(res || []).filter(Boolean);
    if (!list.length) return { ok: false, why: 'Run the model first.', csv: null };
    const bad = typeof Engine !== 'undefined' && Engine.exportReady ? list.map(r => ({ r, c: Engine.exportReady(r) })).find(x => !x.c.ok) : null;
    const why = bad ? `${bad.r.period || 'A'} result: ${bad.c.why}` : list.some(r => r.stale) ? 'Results are out of date. Run the model again.' : '';
    if (why && !opts.force) return { ok: false, why, csv: null };
    return { ok: !why, why, csv: sidraCSV(sc, list, nodeIds, opts) };
  }
  // A classic turning-count sheet for one junction: rows = approaches, columns = L / T / R / U and totals
  function turnCountCSV(sc, res, nodeId) {
    const { rows, A } = rowsFor(sc, res, nodeId);
    const out = [['Junction', sc && sc.nodes[nodeId] ? junctionName(sc, nodeId) : 'Unknown junction'], ['Scenario', sc ? sc.name || '' : ''],
      ['Period', res && res.period ? res.period + ' peak hour' : ''], ['Units', 'veh/h (heavy vehicles included in totals)'], [],
      ['Approach', 'Road', ...MV, 'Total', 'Heavy vehicles']];
    const sum = Object.fromEntries([...MV, 'tot', 'hv'].map(k => [k, 0]));
    for (const a of A.filter(x => x.isIn)) {
      const c = Object.fromEntries([...MV, 'tot', 'hv'].map(k => [k, 0]));
      for (const r of rows) if (r.approach === a.inKey) { const [, hv, t] = split(r); c[r.mv] += t; c.tot += t; c.hv += hv; }
      for (const k in c) sum[k] += c[k];
      out.push([a.name, a.road, ...MV.map(m => c[m]), c.tot, c.hv]);
    }
    out.push(['Total', '', ...MV.map(m => sum[m]), sum.tot, sum.hv]);
    return U.csv(out);
  }

  return { movements, diagram, sidraCSV, sidraExport, turnCountCSV, junctionName, arms, classify, approachName, sideName, emptyText, SIDRA_HEAD, CODE_HEAD,
    LEG_CODES, LEG_NAMES, TURN_CODES, assignLegs, legCodeOf, turnCode, movementId, legLabel, rank, siteRows };
})();
