'use strict';
/* Spare Construct · calibration (contract C3): matrix estimation against traffic counts, and a validation
   summary against NZTA TMDG 2019 (default Category E, small area / corridor), with UK TAG M3.1 alongside.
   Method (SATURN ME2-style, multiplicative, damped): assign; compare each counted site with the model
   (DataHub.matches and DataHub.periodVol, as DataHub.calibration does; two-way counts against both
   directions); site ratio r = obs / model clamped to [1/maxStep, maxStep]; each OD pair's factor is
   multiplied by exp(damp × path-flow-weighted mean, over its paths, of the mean log r of the counted
   sites the path crosses). A path that crosses no counted site adds log r = 0, so an OD pair that is
   only partly counted moves only partly (ME2's p_ija weighting). Cumulative factors are clamped to
   [1/maxFactor, maxFactor]. Only OD pairs with both zones existing land use (z.existing === true) or
   gateways change; new development keeps its own trip rates. Factors go to Store.P.me[period] as
   '<zoneId>><zoneId>': factor, which Engine.demand applies (contract C2) in every scenario.
   Counts are today's traffic, so the fit always runs on a base-year view of the scenario: the settings
   base year (no background growth), existing land use and gateways open by then, and no proposed links.
   Each pass must keep the matrix change inside the TMDG section 8 total and TAG M3.1 Table 5 trip-end
   limits; when the first pass breaks them the step is reduced (maxStep 2 → 1.5 → 1.25 → 1.1).
   Additions beyond C3: options maxStep (2), minVol (10) and constrain (true); a fit always starts from
   the prior matrix and keeps the best pass (most sites with GEH < 5, then lowest mean GEH), stopping
   when a pass does not improve or breaks a limit; the result also carries history, stopped, res, note.
   Thresholds carry their source, edition, table and a verified flag. They are the TMDG defaults; the
   model specification agreed with the peer reviewer takes precedence.
   Integration pass (docs/spec/matrix-estimation, count-roles; docs/reviews/calibration-fix.md):
   - Count weights by observation method (opts.weights true: DataHub.countWeight, measured hour 1.0, weekday factored
     0.7, ADT factored 0.4, RAMM estimate 0.25, virtual 0; or opts.weight(c, period) → w). A path's adjustment is the
     weighted mean of the log ratios of the counts it crosses, times the largest of their weights: where counts
     disagree the better-measured one leads, and a path seen only by factored counts moves only part of the way (as
     SATURN's X_a ← X_a·(C_a/V_a)^w_a). A zero weight leaves the count out. Passes are judged on the weighted
     GEH < 5 share and mean GEH. Off by default (every count 1, the earlier behaviour, except the estimates: RAMM
     0.25, virtual 0): on Drury, weights make the first pass lose one site under GEH 5, so the fit would change
     nothing; the calibration panel opts in.
   - opts.countIds (array or Set): only these counts take part (so callers need not switch other counts off).
   - opts.groups [{ id, label, countIds }]: counts fitted as a group total (one ratio for the group). A group must be
     parallel: when a path crosses two of its counts (in sequence) carrying more than max(1 pcu/h, 1% of the group's
     modelled flow) (Spare Construct convention), the group is rejected, left out of the fit and listed in result.rejected.
     Calibrate.parallel(res, rows, groups) runs the check on its own.
   - opts.freezeGG: gateway-to-gateway cells (through traffic) keep the prior.
   - The fit also records additive deltas (veh/h per estimated pair = prior × (factor − 1)) in me.info[period].delta,
     with the base year: Engine.applyME carries them forward to future-year scenarios as T = max(0, T + delta). */

const Calibrate = (() => {
  const PERIODS = ['AM', 'PM'];
  const TMDG = 'NZTA Transport Model Development Guidelines (TMDG) 2019', TAG = 'UK TAG M3.1 (May 2026)';
  /* Validation criteria; report.js and the UI cite these. kind: 'share' (of sites), 'r2', 'range', 'max'.
     use: 'required' decides the overall status, 'info' is reported only (TMDG 'potentially useful'),
     'warn' applies to the matrix change after estimation. */
  const RULES = {
    geh5: { kind: 'share', min: 0.85, use: 'required', label: 'Sites with GEH < 5', target: 'more than 85% of sites', basis: `${TMDG}, Category E, individual counts`, source: TMDG, table: 'individual counts, GEH', verified: true },
    geh75: { kind: 'share', min: 0.90, use: 'required', label: 'Sites with GEH < 7.5', target: 'more than 90% of sites', basis: `${TMDG}, Category E, individual counts`, source: TMDG, table: 'individual counts, GEH', verified: true },
    geh10: { kind: 'share', min: 0.95, use: 'required', label: 'Sites with GEH < 10', target: 'more than 95% of sites', basis: `${TMDG}, Category E, individual counts`, source: TMDG, table: 'individual counts, GEH', verified: true },
    r2: { kind: 'r2', min: 0.95, use: 'required', label: 'R² of model against counts (through the origin, centred)', target: 'more than 0.95', basis: `${TMDG}, Category E, scatter forced through the origin`, source: TMDG, table: 'scatter plot criteria', verified: true },
    slope: { kind: 'range', min: 0.95, max: 1.05, use: 'required', label: 'Slope of model against counts (through the origin)', target: '0.95 to 1.05', basis: `${TMDG}, Category E, scatter forced through the origin`, source: TMDG, table: 'scatter plot criteria', verified: true },
    band: { kind: 'share', min: 0.90, use: 'info', label: 'Sites within the TMDG count band', target: 'more than 90% of sites (within 50 veh/h below 400, 12.5% to 2,000, 250 veh/h above)', basis: `${TMDG}, Category E, potentially useful`, source: TMDG, table: 'count band criteria', verified: true },
    flow: { kind: 'share', min: 0.85, use: 'info', label: 'Sites with GEH < 5 or within the TAG flow band', target: 'more than 85% of sites (within 100 veh/h below 700, 15% to 2,700, 400 veh/h above)', basis: `${TAG}, link flow criteria, for information`, source: TAG, table: 'Table 2', verified: true },
    rmse: { kind: 'max', max: 15, clarify: 25, use: 'info', label: '%RMSE', target: 'under 15% (15–25% needs explaining)', basis: `${TMDG}, Category E, potentially useful`, source: TMDG, table: '%RMSE criteria', verified: true },
    matrixTotal: { kind: 'max', max: 5, use: 'warn', label: 'Change in matrix total', target: 'within 5% (a good prior; 10% for a poor one)', basis: `${TMDG}, section 8, Category E`, source: TMDG, table: 'section 8 (matrix estimation)', verified: true },
    cells: { kind: 'r2', min: 0.95, slope: [0.98, 1.02], use: 'warn', label: 'R² of matrix cells, prior against estimated', target: 'more than 0.95, slope 0.98 to 1.02', basis: `${TAG}, matrix estimation significance`, source: TAG, table: 'Table 5', verified: true },
    tripEnds: { kind: 'r2', min: 0.98, slope: [0.99, 1.01], use: 'warn', label: 'R² of trip ends, prior against estimated', target: 'more than 0.98, slope 0.99 to 1.01', basis: `${TAG}, matrix estimation significance`, source: TAG, table: 'Table 5', verified: true },
  };
  const MIN_SITES = 10;   // fewer sites than this: 'Indicative', never pass or fail (Spare Construct convention)
  const eligible = z => !!z && (z.kind === 'gateway' || z.existing === true);
  const keyOf = (a, b) => a + '>' + b;
  const meOf = () => { const P = Store.P; if (!P.me || typeof P.me !== 'object') P.me = {}; return P.me; };
  const flowOK = (m, o) => o < 700 ? Math.abs(m - o) <= 100 : o <= 2700 ? Math.abs(m - o) <= 0.15 * o : Math.abs(m - o) <= 400;
  const bandOK = (m, o) => o < 400 ? Math.abs(m - o) <= 50 : o <= 2000 ? Math.abs(m - o) <= 0.125 * o : Math.abs(m - o) <= 250;
  function pearson2(x, y) {
    const n = x.length; if (n < 2) return null;
    let mx = 0, my = 0; for (let i = 0; i < n; i++) { mx += x[i]; my += y[i]; }
    mx /= n; my /= n;
    let sxy = 0, sxx = 0, syy = 0;
    for (let i = 0; i < n; i++) { const a = x[i] - mx, b = y[i] - my; sxy += a * b; sxx += a * a; syy += b * b; }
    return sxx > 0 && syy > 0 ? sxy * sxy / (sxx * syy) : null;
  }
  // Fit y = b0·x through the origin: slope, R² centred on the mean of y (the TMDG test) and R² against Σy² (Excel's)
  function origin(x, y) {
    const n = x.length; if (n < 2) return { b0: null, r2c: null, r2e: null };
    let sxy = 0, sxx = 0, sy = 0, syy = 0;
    for (let i = 0; i < n; i++) { sxy += x[i] * y[i]; sxx += x[i] * x[i]; sy += y[i]; syy += y[i] * y[i]; }
    if (!(sxx > 0)) return { b0: null, r2c: null, r2e: null };
    const b0 = sxy / sxx, my = sy / n;
    let se = 0, st = 0;
    for (let i = 0; i < n; i++) { se += (y[i] - b0 * x[i]) ** 2; st += (y[i] - my) ** 2; }
    return { b0, r2c: st > 0 ? 1 - se / st : null, r2e: syy > 0 ? 1 - se / syy : null };
  }

  /* ---------- counted sites against a result ---------- */
  /* Same selection and model volume as DataHub.calibration, plus the arcs each site counts and its weight w.
     opts: countIds (only these counts), estimates (true: RAMM centreline estimates take part, as in the fit; validation
     statistics never use them), weights (true: w by observation method; otherwise every w = 1), weight(c, period) (a
     caller's own weights). */
  const idSet = ids => (ids == null ? null : ids instanceof Set ? ids : new Set([].concat(ids).map(String)));
  function weightOf(c, period, o) {
    if (typeof o.weight === 'function') { const w = +o.weight(c, period); return w >= 0 && isFinite(w) ? w : 1; }
    // Without weights every count weighs 1, except the estimates: RAMM centreline estimates 0.25, NZTA virtual sites 0
    if (o.weights !== true) return DataHub.isEstimate && DataHub.isEstimate(c) ? DataHub.OBS_METHODS.ramm_estimate.weight : c.virtual ? 0 : 1;
    return DataHub.countWeight ? DataHub.countWeight(c, period) : 1;
  }
  function sites(sc, res, period, opts = {}) {
    const m = DataHub.matches(sc), rows = [], idx = new Map((res.arcKeys || []).map((k, i) => [k, i])), only = idSet(opts.countIds);
    const est = c => (DataHub.isEstimate ? DataHub.isEstimate(c) : false);
    for (const c of DataHub.D().counts) {
      if (!DataHub.inUse(c)) continue;
      if (only && !only.has(c.id)) continue;
      if (est(c) && !opts.estimates) continue;
      const mt = m.get(c.id); if (!mt) continue;
      const pv = DataHub.periodVol(c, period); if (!pv) continue;
      const a = res.arcs[mt.link + ':ab'], b = res.arcs[mt.link + ':ba'];
      const model = mt.dir === 'ab' ? (a ? a.v : null) : mt.dir === 'ba' ? (b ? b.v : null) : (a ? a.v : 0) + (b ? b.v : 0);
      if (model == null) continue;
      const keys = mt.dir === 'both' ? [mt.link + ':ab', mt.link + ':ba'] : [mt.link + ':' + mt.dir];
      const obs = pv.v, geh = Math.sqrt(2 * (model - obs) ** 2 / Math.max(1, model + obs));
      rows.push({ c, mt, obs, model, geh, arcs: keys.map(k => idx.get(k)).filter(i => i != null), w: weightOf(c, period, opts) });
    }
    return rows;
  }
  /* Count groups (docs/spec/matrix-estimation: groups must be parallel). groups: [{ id, label, countIds }]. Returns
     { units, rejected }: units are the fitting units (a group total, or a single count not in any group) as
     { obs, model, arcs, w, ids, group }; a group whose counts lie in sequence on one path is rejected. */
  const SEQ = { minPcu: 1, share: 0.01, basis: 'Spare Construct convention' };
  function parallel(res, rows, groups) {
    const out = { ok: [], rejected: [] };
    if (!Array.isArray(groups) || !groups.length) return out;
    const byId = new Map(rows.map((r, k) => [r.c.id, k]));
    const G = groups.map((g, gi) => ({ g, gi, members: [...new Set([].concat((g && g.countIds) || []).map(String))].map(id => byId.get(id)).filter(k => k != null) }))
      .filter(x => x.members.length);
    const at = new Map();   // arc index → [[group, member row]]
    for (const x of G) for (const k of x.members) for (const a of rows[k].arcs) { const l = at.get(a); if (l) l.push([x.gi, k]); else at.set(a, [[x.gi, k]]); }
    const shared = new Map(), flow = new Map();
    for (const x of G) flow.set(x.gi, x.members.reduce((s, k) => s + rows[k].model, 0));
    for (const od of res.paths || []) for (const p of od.list) {
      const hit = new Map();
      for (const a of p.arcs) for (const [gi, k] of at.get(a) || []) { const s = hit.get(gi); if (s) s.add(k); else hit.set(gi, new Set([k])); }
      for (const [gi, s] of hit) if (s.size > 1) shared.set(gi, (shared.get(gi) || 0) + p.w);
    }
    const pf = Engine.pcuF(Store.S);
    for (const x of G) {
      const sh = (shared.get(x.gi) || 0) / pf, lim = Math.max(SEQ.minPcu, SEQ.share * (flow.get(x.gi) || 0));
      if (sh > lim) out.rejected.push({ id: x.g.id ?? x.gi, label: x.g.label || String(x.g.id ?? x.gi), shared: U.r1(sh), countIds: x.members.map(k => rows[k].c.id),
        reason: `Counts in sequence: ${U.fmt.int(sh)} veh/h pass more than one of its counts on the same path, so the group total counts them twice (groups must be parallel; ${SEQ.basis}).` });
      else out.ok.push({ ...x, ids: x.members.map(k => rows[k].c.id) });
    }
    return out;
  }
  /* Fitting units from rows (one result's sites) and a parallel check made once on the prior run: each accepted group
     is one unit (its total), rejected groups are left out, other counts are units of their own. Rows are matched to the
     check by count id, so the same check serves every pass. */
  function units(rows, chk) {
    const single = r => ({ obs: r.obs, model: r.model, arcs: r.arcs, w: r.w ?? 1, ids: [r.c.id] });
    if (!chk || (!chk.ok.length && !chk.rejected.length)) return rows.map(single);
    const byId = new Map(rows.map(r => [r.c.id, r])), used = new Set(), list = [], gone = new Set(chk.rejected.flatMap(r => r.countIds));
    for (const x of chk.ok) {
      const rs = x.ids.filter(id => !used.has(id) && byId.has(id)).map(id => byId.get(id)); if (!rs.length) continue;
      rs.forEach(r => used.add(r.c.id));
      list.push({ obs: rs.reduce((s, r) => s + r.obs, 0), model: rs.reduce((s, r) => s + r.model, 0), arcs: [...new Set(rs.flatMap(r => r.arcs))],
        w: rs.reduce((s, r) => s + (r.w ?? 1), 0) / rs.length, ids: rs.map(r => r.c.id), group: x.g.id ?? x.gi });
    }
    for (const r of rows) if (!used.has(r.c.id) && !gone.has(r.c.id)) list.push(single(r));
    return list;
  }
  // GEH bands of the criteria library (criteria-and-stats): count and cumulative share per band
  const GEH_BANDS = [[0, 5], [5, 7.5], [7.5, 10], [10, 12], [12, Infinity]];
  function gehBands(rows) {
    const n = rows.length; let cum = 0;
    return GEH_BANDS.map(([lo, hi]) => {
      const k = rows.filter(r => r.geh >= lo && r.geh < hi).length; cum += k;
      return { lo, hi, label: hi === Infinity ? `≥ ${lo}` : lo === 0 ? `< ${hi}` : `${lo}–${hi}`, n: k, share: n ? k / n : null, cum: n ? cum / n : null };
    });
  }
  // OLS fit y = a + b·x (reported beside the through-origin fit; never the pass/fail test)
  function ols(x, y) {
    const n = x.length; if (n < 2) return { a: null, b: null };
    let mx = 0, my = 0; for (let i = 0; i < n; i++) { mx += x[i]; my += y[i]; }
    mx /= n; my /= n;
    let sxy = 0, sxx = 0;
    for (let i = 0; i < n; i++) { sxy += (x[i] - mx) * (y[i] - my); sxx += (x[i] - mx) ** 2; }
    return sxx > 0 ? { a: my - sxy / sxx * mx, b: sxy / sxx } : { a: null, b: null };
  }
  /* r2 is the TMDG test statistic (through the origin, centred: 1 − Σ(m − b0·c)² / Σ(m − m̄)²); r2Excel is
     1 − Σ(m − b0·c)² / Σm²; slope is b0 = Σ(c·m) / Σc²; r2Ols / olsSlope / olsIntercept are the ordinary
     least-squares fit (r2Ols equals DataHub.calibration's r2). */
  function stats(rows) {
    const n = rows.length;
    if (!n) return { n: 0, pass: null, pass75: null, pass10: null, band: null, flow: null, geh: null, r2: null, r2Excel: null, r2Ols: null, slope: null, olsSlope: null, olsIntercept: null, rmse: null, gehBands: gehBands(rows), totObs: 0, totMod: 0 };
    let so = 0, sm = 0, g = 0, p5 = 0, p75 = 0, p10 = 0, fl = 0, bd = 0, se = 0;
    for (const r of rows) {
      so += r.obs; sm += r.model; g += r.geh; se += (r.model - r.obs) ** 2;
      if (r.geh < 5) p5++; if (r.geh < 7.5) p75++; if (r.geh < 10) p10++;
      if (r.geh < 5 || flowOK(r.model, r.obs)) fl++;
      if (bandOK(r.model, r.obs)) bd++;
    }
    const x = rows.map(r => r.obs), y = rows.map(r => r.model), o = origin(x, y), L = ols(x, y);
    return { n, pass: p5 / n, pass75: p75 / n, pass10: p10 / n, band: bd / n, flow: fl / n, geh: g / n,
      r2: o.r2c, r2Excel: o.r2e, r2Ols: pearson2(x, y), slope: o.b0, olsSlope: L.b, olsIntercept: L.a, gehBands: gehBands(rows),
      rmse: n > 1 && so > 0 ? 100 * Math.sqrt(se / (n - 1)) / (so / n) : null, totObs: so, totMod: sm };
  }
  const better = (a, b) => a.pass > b.pass + 1e-9 || (a.pass > b.pass - 1e-9 && a.geh < b.geh * 0.995);
  /* What a pass is judged on: the share of sites with GEH < 5, then the mean GEH, each weighted by the count weights
     (with every weight 1 these are the plain statistics), so a pass is not refused for moving an ADT-factored count
     the fit was told to trust less. The reported before / after statistics stay unweighted (TMDG). */
  function wfit(rows) {
    let sw = 0, p5 = 0, g = 0;
    for (const r of rows) { const w = r.w ?? 1; if (!(w > 0)) continue; sw += w; g += w * r.geh; if (r.geh < 5) p5 += w; }
    return sw > 0 ? { pass: p5 / sw, geh: g / sw } : { pass: 0, geh: Infinity };
  }

  /* ---------- one ME2 step: new cumulative factors from a result ----------
     rows: fitting units { obs, model, arcs, w }. A path takes the w-weighted mean of the log ratios of the units it
     crosses (so counts in sequence do not compound), times the largest w among them (w = 0: left out). With every
     w = 1 this is the plain mean, as before weights. */
  function step(sc, res, rows, f, o) {
    const lr = rows.map(r => r.model < o.minVol || r.obs < o.minVol || !((r.w ?? 1) > 0) ? null : Math.log(U.clamp(r.obs / r.model, 1 / o.maxStep, o.maxStep)));
    const wt = rows.map(r => r.w ?? 1);
    const at = new Map();
    rows.forEach((r, s) => { if (lr[s] != null) for (const a of r.arcs) { let l = at.get(a); if (!l) at.set(a, l = []); l.push(s); } });
    const seen = new Int32Array(rows.length).fill(-1), lo = 1 / o.maxFactor;
    let stamp = 0;
    for (const od of res.paths) {
      const zi = sc.zones[res.zoneIds[od.i]], zj = sc.zones[res.zoneIds[od.j]];
      if (od.i === od.j || !eligible(zi) || !eligible(zj)) continue;
      if (o.freezeGG && zi.kind === 'gateway' && zj.kind === 'gateway') continue;   // through traffic keeps the prior
      let acc = 0, w = 0;
      for (const p of od.list) {
        stamp++;
        let s = 0, sw = 0, wm = 0;
        for (const a of p.arcs) {
          const l = at.get(a); if (!l) continue;
          for (const t of l) if (seen[t] !== stamp) { seen[t] = stamp; s += wt[t] * lr[t]; sw += wt[t]; if (wt[t] > wm) wm = wt[t]; }
        }
        acc += p.w * (sw > 0 ? s / sw * wm : 0); w += p.w;
      }
      if (!(w > 0) || acc === 0) continue;
      const key = keyOf(zi.id, zj.id);
      f.set(key, U.clamp((f.get(key) || 1) * Math.exp(o.damp * acc / w), lo, o.maxFactor));
    }
  }
  // Rounded towards 1 so a stored factor never passes the clamp
  const toObj = f => { const o = {}; for (const [k, v] of f) if (Math.abs(v - 1) > 1e-5) o[k] = (v < 1 ? Math.ceil(v * 1e5) : Math.floor(v * 1e5)) / 1e5; return o; };

  /* ---------- matrix change: prior against estimated ---------- */
  function matrixChange(G, prior, est) {
    const Z = G.zones.length, ro = new Float64Array(Z), re = new Float64Array(Z), co = new Float64Array(Z), ce = new Float64Array(Z), cp = [], cq = [];
    let tp = 0, te = 0, cells = 0, dev = 0, pairs = 0;
    for (let i = 0; i < Z; i++) for (let j = 0; j < Z; j++) {
      const p = prior[i * Z + j], e = est[i * Z + j];
      ro[i] += p; re[i] += e; co[j] += p; ce[j] += e; tp += p; te += e;
      if (p > 0) { cells++; dev += Math.abs(e / p - 1); cp.push(p); cq.push(e); if (Math.abs(e / p - 1) > 1e-6) pairs++; }
    }
    // Zones with no trips either way are left out of the trip-end tests
    const live = [...ro.keys()].filter(i => ro[i] + co[i] + re[i] + ce[i] > 0);
    const pick = (a, ix) => ix.map(i => a[i]);
    const rowR2 = pearson2(pick(ro, live), pick(re, live)), colR2 = pearson2(pick(co, live), pick(ce, live));
    const rs = origin(pick(ro, live), pick(re, live)).b0, cs = origin(pick(co, live), pick(ce, live)).b0, cl = origin(cp, cq).b0;
    return {
      priorTotal: tp, total: te, totalPct: tp > 0 ? (te - tp) / tp * 100 : 0, meanAbsCellPct: cells ? dev / cells * 100 : 0,
      pairs, cells, cellR2: pearson2(cp, cq), cellSlope: cl, rowR2, colR2, rowSlope: rs, colSlope: cs,
      tripEndR2: rowR2 == null ? colR2 : colR2 == null ? rowR2 : Math.min(rowR2, colR2),
    };
  }
  // The limits a pass must keep to (TMDG section 8 total; TAG Table 5 trip-end R²); '' when within them
  function breach(mc) {
    if (Math.abs(mc.totalPct) > RULES.matrixTotal.max) return 'matrix total';
    if (mc.tripEndR2 != null && mc.tripEndR2 < RULES.tripEnds.min) return 'trip ends';
    return '';
  }

  /* ---------- base-year view ---------- */
  // What the counts measure: the scenario in the base year with existing land use and existing roads
  function baseView(sc, S = Store.S) {
    const by = +S.baseYear, zones = {}, links = {};
    for (const [id, z] of Object.entries(sc.zones || {})) if (eligible(z) && !(+z.year > by)) zones[id] = z;
    for (const [id, L] of Object.entries(sc.links || {})) if (L.status !== 'proposed') links[id] = L;
    const same = Engine.yearOf(sc, S) === by && Object.keys(zones).length === Object.keys(sc.zones || {}).length && Object.keys(links).length === Object.keys(sc.links || {}).length;
    return { view: same ? sc : { ...sc, id: sc.id + ':me', name: (sc.name || '') + ' (base year)', year: by, zones, links, res: {} }, same };
  }

  /* ---------- fit ---------- */
  async function fit(sc = Store.sc, opts = {}) {
    const S = Store.S, period = opts.period || S.period, prog = opts.onProgress || (() => {});
    const o = {
      iters: Math.max(0, Math.round(opts.iters ?? 6)), damp: U.clamp(+(opts.damp ?? 0.7) || 0.7, 0.01, 1),
      maxFactor: Math.max(1, +opts.maxFactor || 3), maxStep: Math.max(1, +opts.maxStep || 2), minVol: opts.minVol ?? 10,
      constrain: opts.constrain !== false, freezeGG: !!opts.freezeGG, weights: opts.weights === true ? 'method' : typeof opts.weight === 'function' ? 'caller' : 'none',
    };
    // Which counts take part and how much each weighs (sites() options); groups fit as totals once found parallel
    const so = { countIds: opts.countIds, estimates: true, weights: opts.weights, weight: opts.weight }, groups = Array.isArray(opts.groups) ? opts.groups : null;
    if (opts.countIds != null) o.nCounts = idSet(opts.countIds).size;
    if (groups) o.groups = groups.length;
    const me = meOf(), old = me[period];
    // Everything a failed fit must put back
    const keep = { info: me.info && me.info[period], res: sc.res && sc.res[period], stale: Store.P.scenarios.map(s => [s, s.res && s.res[period], s.res && s.res[period] && s.res[period].stale]) };
    const restore = () => {
      if (old) me[period] = old; else delete me[period];
      if (keep.info) (me.info ||= {})[period] = keep.info; else if (me.info) delete me.info[period];
      if (sc.res) { if (keep.res) sc.res[period] = keep.res; else delete sc.res[period]; }
      for (const [s, r, st] of keep.stale) if (r) r.stale = st;
    };
    delete me[period];
    let out;
    try {
      const { view, same } = baseView(sc, S);
      const G = Engine.buildGraph(view), prior = Engine.demand(view, G, period, S).T;
      const mcOf = () => { const est = Float64Array.from(prior); Engine.applyME(G, period, est); return matrixChange(G, prior, est); };
      prog(0, o.iters, `Running the base year (${S.baseYear}) with the prior matrix`);
      const res0 = await Engine.assign(view, { period });
      let rows = sites(view, res0, period, so);
      const before = stats(rows), history = [before];
      const chk = groups && res0.paths ? parallel(res0, rows, groups) : null, rejected = chk ? chk.rejected : [];
      const none = note => { restore(); return { before, after: before, iters: 0, changedPairs: old ? Object.keys(old).length : 0, matrixChange: null, history, stopped: 'none', res: null, note, kept: !!old, rejected }; };
      if (!before.n) return none(`No counts match the network for the ${period} peak. Load counts for this area first.`);
      if (!res0.paths) return none('This model has too many zones to keep the paths matrix estimation needs.');
      if (!units(rows, chk).some(u => (u.w ?? 1) > 0)) return none('Every count that matches the network has zero weight (NZTA estimates only), so nothing can be fitted. Load measured counts first.');
      if (res0.zoneIds.filter(id => eligible(view.zones[id])).length < 2) return none('No OD pairs can be estimated. Mark the base-year zones as existing land use, or add gateways.');
      let best = { f: new Map(), st: before, wf: wfit(rows), res: res0, k: 0, mc: null }, stopped = 'iterations', step0 = o.maxStep;
      const STEPS = [2, 1.5, 1.25, 1.1].filter(x => x < step0);
      for (let tries = 0; ; tries++) {
        const f = new Map();
        let res = res0, broke = '';
        rows = sites(view, res0, period, so);
        for (let k = 1; k <= o.iters; k++) {
          step(view, res, units(rows, chk), f, o);
          me[period] = toObj(f);
          prog(k, o.iters, `Matrix estimation pass ${k} of ${o.iters}${tries ? ` (smaller steps, up to ×${o.maxStep})` : ''}`);
          const mc = mcOf();
          if (o.constrain && (broke = breach(mc))) break;
          res = await Engine.assign(view, { period });
          rows = sites(view, res, period, so);
          const st = stats(rows), wf = wfit(rows);
          history.push(st);
          if (better(wf, best.wf)) best = { f: new Map(f), st, wf, res, k, mc };
          else { stopped = 'no improvement'; break; }
        }
        if (broke) stopped = `${broke} limit`;
        // The very first pass broke a limit: retry from the prior with smaller steps
        if (broke && best.k === 0 && STEPS.length) { o.maxStep = STEPS.shift(); history.length = 1; continue; }
        break;
      }
      if (best.k === 0) {
        const why = stopped.endsWith('limit') ? `every pass broke the ${stopped} (TMDG section 8 / TAG M3.1 Table 5)` : 'no pass improved the fit';
        out = none(`Matrix estimation made no change: ${why}. ${old ? 'The earlier factors were kept.' : 'Check the trip rates and gateway counts.'}`);
        out.stopped = stopped;
        return out;
      }
      const factors = toObj(best.f);
      me[period] = factors;
      const mc = best.mc || mcOf();
      /* Additive deltas for future years (docs/spec/matrix-estimation: futures inherit meDelta = T_post − T_prior):
         veh/h per estimated pair, from the base-year prior; Engine.applyME uses them for other scenario years */
      const delta = {}, zi = new Map(G.zones.map((z, i) => [z.id, i])), Z = G.zones.length;
      for (const [key, fv] of Object.entries(factors)) {
        const s = key.indexOf('>'), i = zi.get(key.slice(0, s)), j = zi.get(key.slice(s + 1));
        if (i == null || j == null) continue;
        const d = prior[i * Z + j] * (fv - 1);
        if (isFinite(d) && Math.abs(d) >= 0.0005) delta[key] = Math.round(d * 1000) / 1000;
      }
      me.info ||= {};
      me.info[period] = { date: new Date().toISOString(), iters: best.k, before, after: best.st, matrixChange: mc, tripEndR2: mc.tripEndR2, options: o, scenario: sc.id, baseYear: +S.baseYear, stopped,
        delta, future: 'delta', ...(opts.countIds != null ? { countIds: [...idSet(opts.countIds)] } : {}), ...(rejected.length ? { rejected } : {}) };
      // The factors change this period's demand in every scenario with existing land use
      for (const s of Store.P.scenarios) if (s !== sc && s.res && s.res[period]) s.res[period].stale = true;
      // The scenario's own result: the fitted one when the scenario is the base year, else a fresh run with the factors
      prog(o.iters, o.iters, 'Running the scenario with the fitted matrix');
      sc.res ||= {};
      sc.res[period] = same ? best.res : await Engine.assign(sc, { period });
      const n = Object.keys(factors).length;
      out = {
        before, after: best.st, iters: best.k, changedPairs: n, matrixChange: mc, history, stopped, res: best.res, baseYear: S.baseYear, rejected,
        note: `Matrix estimation (base year ${S.baseYear}): ${U.fmt.int(n)} OD pair${n === 1 ? '' : 's'} adjusted in ${best.k} pass${best.k === 1 ? '' : 'es'}; sites with GEH < 5 went from ${U.fmt.pct(before.pass)} to ${U.fmt.pct(best.st.pass)}; matrix total ${U.fmt.signedPct(mc.totalPct / 100, 1)}.` +
          (rejected.length ? ` ${rejected.length} count group${rejected.length === 1 ? ' was' : 's were'} left out: counts in sequence (${rejected.map(r => r.label).join(', ')}).` : '') +
          (same ? '' : ' Other years carry the estimation forward as an additive delta on the same OD pairs.'),
      };
    } catch (e) {
      restore();
      throw e;
    }
    // Emit once everything is committed, so a failing listener cannot leave the project half-updated
    Store.emit('results');
    return out;
  }

  /* ---------- clear ---------- */
  function clear(period) {
    const P = Store.P; if (!P || !P.me || typeof P.me !== 'object') return 0;
    let n = 0;
    const hit = [];
    for (const p of period ? [period] : PERIODS) {
      const k = P.me[p] ? Object.keys(P.me[p]).length : 0;
      if (P.me[p]) { n += k; delete P.me[p]; }
      if (P.me.info) delete P.me.info[p];
      if (k) hit.push(p);
    }
    // Only results that used factors are out of date
    for (const p of hit) for (const s of P.scenarios) if (s.res && s.res[p]) s.res[p].stale = true;
    if (P.me.info && !Object.keys(P.me.info).length) delete P.me.info;
    if (!Object.keys(P.me).length) delete P.me;
    if (hit.length) Store.emit('demand');
    return n;
  }

  /* ---------- validation summary ---------- */
  const pc = x => U.fmt.pct(x, 1);
  function report(sc = Store.sc, period = Store.S.period) {
    const S = Store.S, res = Store.res(sc, period), info = (Store.P.me && Store.P.me.info && Store.P.me.info[period]) || null;
    const out = { period, standard: `${TMDG}, Category E (small area / corridor)`, n: 0, geh5: null, geh75: null, geh10: null, band: null, flow: null, r2: null, r2Excel: null, r2Ols: null, slope: null, rmse: null, ratio: null,
      totObs: 0, totMod: 0, year: res ? res.year : null, stale: !!(res && res.stale), checks: [], status: 'none', pass: null, summary: '', matrix: info, warnings: [], notes: [] };
    if (!res) { out.notes.push(`Run the model for the ${period} peak first.`); return out; }
    const st = stats(sites(sc, res, period));
    Object.assign(out, { n: st.n, geh5: st.pass, geh75: st.pass75, geh10: st.pass10, band: st.band, flow: st.flow, r2: st.r2, r2Excel: st.r2Excel, r2Ols: st.r2Ols, slope: st.slope, olsSlope: st.olsSlope, olsIntercept: st.olsIntercept, gehBands: st.gehBands, rmse: st.rmse,
      ratio: st.totObs > 0 ? st.totMod / st.totObs : null, totObs: st.totObs, totMod: st.totMod });
    if (!st.n) { out.notes.push(`No counts match the network for the ${period} peak. Load counts for this area first.`); return out; }
    const few = st.n < MIN_SITES;
    const chk = (id, value, ok, extra = {}) => {
      const R = RULES[id], judged = R.use !== 'info' && !(few && R.use === 'required');
      const status = value == null ? 'N/A' : R.use === 'info' ? 'Info' : few && R.use === 'required' ? 'Indicative' : ok ? 'Pass' : 'Fail';
      out.checks.push({ id, ...R, value, ok: value == null ? null : !!ok, required: R.use === 'required', judged, status, ...extra });
    };
    // Shares also say how many sites were needed, e.g. 'Fail (41 of 52; 45 needed)'
    const share = (id, v) => {
      const need = Math.ceil(RULES[id].min * st.n - 1e-9), got = Math.round(v * st.n);
      chk(id, v, got >= need, { got, need, of: st.n });
      const c = out.checks[out.checks.length - 1];
      if (c.status === 'Fail') c.status = `Fail (${got} of ${st.n}; ${need} needed)`;
    };
    share('geh5', st.pass); share('geh75', st.pass75); share('geh10', st.pass10);
    chk('r2', st.r2, st.r2 != null && st.r2 > RULES.r2.min);
    chk('slope', st.slope, st.slope != null && st.slope >= RULES.slope.min && st.slope <= RULES.slope.max);
    share('band', st.band); share('flow', st.flow);
    chk('rmse', st.rmse, st.rmse != null && st.rmse < RULES.rmse.max, { clarify: st.rmse != null && st.rmse >= RULES.rmse.max && st.rmse <= RULES.rmse.clarify });
    if (info && info.matrixChange) {
      const mc = info.matrixChange;
      chk('matrixTotal', mc.totalPct, Math.abs(mc.totalPct) <= RULES.matrixTotal.max);
      chk('cells', mc.cellR2, mc.cellR2 == null || (mc.cellR2 > RULES.cells.min && (mc.cellSlope == null || (mc.cellSlope >= RULES.cells.slope[0] && mc.cellSlope <= RULES.cells.slope[1]))), { slopeValue: mc.cellSlope ?? null });
      chk('tripEnds', mc.tripEndR2, mc.tripEndR2 == null || mc.tripEndR2 > RULES.tripEnds.min, { slopeValue: mc.rowSlope ?? null });
      if (Math.abs(mc.totalPct) > RULES.matrixTotal.max) out.warnings.push(`Matrix estimation changed the matrix total by ${U.fmt.signedPct(mc.totalPct / 100, 1)}, more than ${RULES.matrixTotal.max}%. Check the trip rates and gateway counts before relying on the factors.`);
      if (mc.tripEndR2 != null && mc.tripEndR2 <= RULES.tripEnds.min) out.warnings.push(`Matrix estimation changed the trip ends a lot (R² ${mc.tripEndR2.toFixed(3)}, TAG M3.1 Table 5 asks for more than ${RULES.tripEnds.min}). The prior matrix is probably wrong in places; fix it rather than let the counts force it.`);
      out.notes.push('The same counts were used to estimate the matrix and to validate it. TMDG and TAG M3.1 expect validation against independent counts, so hold some counts back for an independent check.');
      out.notes.push('On NZTA projects matrix estimation needs the peer reviewer\'s agreement (TMDG).');
    }
    // Counts are base-year traffic: comparing them with a forecast is not validation
    const baseYear = !(res.year > 0) || res.year === S.baseYear;
    if (!baseYear) out.warnings.push(`These results are for ${res.year}, but the counts are base-year (${S.baseYear}) traffic. Validate a base-year scenario; this comparison is for information only.`);
    // Statistics from a run that did not converge mix assignment noise into the fit
    if (res.converged === false) out.warnings.push(`${res.convNote || "The assignment did not converge."} Re-run the model to convergence before reporting these statistics.`);
    if (out.stale) out.warnings.push('The results are out of date. Run the model again before reporting.');
    if (few) out.warnings.push(`Only ${st.n} count site${st.n === 1 ? '' : 's'} match the network. With fewer than ${MIN_SITES} the statistics are indicative only, so no pass or fail is given.`);
    out.notes.push('Meeting the criteria does not by itself show the model is fit for purpose (TAG M3.1 3.2.2). The model specification agreed with the peer reviewer takes precedence over these defaults.');
    const req = out.checks.filter(c => c.required);
    if (few) out.status = 'indicative';
    else if (!baseYear) out.status = 'not base year';
    else out.status = req.every(c => c.ok) ? 'pass' : 'fail';
    out.pass = out.status === 'pass' ? true : out.status === 'fail' ? false : null;
    const failed = req.filter(c => c.ok === false).map(c => c.label);
    out.summary = out.status === 'indicative' ? `Indicative only: ${st.n} count sites (${period} peak, ${pc(st.pass)} with GEH < 5).`
      : out.status === 'not base year' ? `Not assessed: the ${res.year} results are compared with base-year counts (${period} peak, ${pc(st.pass)} with GEH < 5).`
      : `Assessed against TMDG 2019 Category E, ${period} peak, ${st.n} count sites: ${out.pass ? 'all expected tests pass' : 'fails ' + failed.join(', ')}.`;
    return out;
  }

  return { fit, clear, report, RULES, sites, stats, eligible, baseView, matrixChange, MIN_SITES, origin, ols, gehBands, GEH_BANDS, parallel, units, SEQ };
})();
