'use strict';
/* Spare Construct · calibration and validation to NZTA TMDG 2019 (owner: calibration lane).
   Specs: docs/spec/criteria-and-stats, count-roles, calibration-panel, turning-counts, screenlines, calibrate-workflow,
   matrix-estimation (UI and reporting) and _validation-rules.

   - Criteria library: TMDG 2019 purpose categories A–G (the category is settings.tmdg.category, shared with the model-status
     badge), UK TAG M3.1 (May 2026) alongside, RMS 2013 and ATAP presets. Every value carries its source, edition, table
     and a verified flag; unverified values show an amber 'verify' marker.
   - Count roles: 'gateway' (used to set gateway flows, found from z.gwSrc), 'me' (calibration: matrix estimation and
     trip-end factors), 'valid' (held out, independent) and 'off'. Validation statistics only use 'valid' counts that
     matrix estimation did not see; matrix estimation from this panel only sees 'me' counts.
   - Observation methods: measured_hour | weekday_factored | adt_factored | virtual. Virtual counts are never scored.
   - The calibration panel replaces the 'Model against counts' and journey-time sections of the Traffic data tab (the
     built-in tab is wrapped, its source cards fold into a chip row once counts are loaded).
   - Turning counts (long, wide and SCATS CSV), peak hour and PHF, turn validation, Furness turn estimation from leg
     totals and balancing between adjacent sites. Screenlines and cordons (drawing tool), and a cordon OD export.
   - Calibrate in order: network checks, stress test (demand × 1.1 / × 1.2), trip-end factors, bounded matrix estimation
     (Calibrate.fit) with the TMDG section 8 / TAG M3.1 Table 5 report.
   Project data lives in Store.P.data.calib = { roles, screenlines, turnSites }; trip-end factors in sc.calib.

   Deviations from the specs (also in the lane report):
   - Roles live in P.data.calib.roles (by count id) so they survive a reload of the counts; c.role is mirrored onto
     the count objects. c.obs_method is derived per period (an AT count is a measured hour in one peak and factored
     in the other); an importer that sets c.obs_method as a string is respected.
   - Matrix estimation is calibrate.js's ME2-style method. This panel runs it on 'me' counts only (other counts are
     switched off for the duration of the run), maps XAMAX to its maxStep and maxFactor, and adds the Table M1 report.
     Count weights by method and the parallel-group check need calibrate.js support.
   - Trip-end factors use a linearised coordinate descent: one assignment per factor to measure its effect, a bounded
     descent on that linear model, then one assignment to confirm (Engine has no warm start). Gateways set from counts
     keep their counts; only estimated gateways get the gateway factor.
   - Future scenarios carry estimation forward the way the fit recorded it: an additive delta per OD pair when
     me.info[period].future === 'delta' (Engine.applyME: T = max(0, T + Δ)), otherwise multiplicative factors; the
     panel and the report say which (futureText).
   - Matrix estimation passes the calibration counts as opts.countIds (other counts are no longer switched off),
     their observation-method weights (METHODS), calibration screenlines as fitted groups, and freezeGG (settings
     calib.freezeGG, default on: gateway-to-gateway through traffic keeps the prior).
   - Trip-end factors are applied as Store.P.luFactors[period] (Engine.tripEnds scales existing-land-use zones and
     estimated gateways only), so development and committed zones keep their own trip rates; a factor at its bound
     is still refused. The fit itself runs without luFactors, so it always starts from the trip rates as they are.
   - Turning counts at a node of a junction site are compared with the whole site; at a multi-node junction with no
     site defined (a roundabout ring) they are not scored. Site movements are traced along the assigned paths, as
     docs/spec/junction-sites defines them (entry arc to exit arc of each path). res.siteTurns chains turn shares
     node by node, which loses where vehicles came from: at a roundabout it sends half of each entry to the first
     exit and invents U-turns. Paths are not saved with the project, so the traced totals are kept in
     sc.calib.siteTurns[period] for the result they came from.
   - Share targets are strict, as TMDG and TAG word them ('E > 85'): needed = floor(target · n) + 1, so 17 of 20
     (85.0%) does not pass. criteria-and-stats writes ceil(target · n − 1e-9), which would pass it.
   - TAG M3.1 sits beside every row in its own column (the TAG test for the same evidence); the TAG rows the
     calibration-panel spec lists stay in the table too. */

const Calibration = (() => {
  const PERIODS = ['AM', 'PM'];
  const esc = s => U.esc(s);
  const pct = (x, d = 1) => x == null || !isFinite(x) ? '–' : U.fmt.pct(x, d);
  const num = (x, d = 2) => x == null || !isFinite(x) ? '–' : U.fmt.num(x, d);
  const BOM = '﻿';
  // Values read from a project file go into HTML: whole numbers through n0, peak-hour times through hm, land-use names through luName
  const n0 = x => x != null && x !== '' && isFinite(+x) ? U.fmt.int(+x) : '–';
  const hm = pk => pk ? esc(pk.start) + '–' + esc(pk.end) : '–';
  const luName = g => g === 'gateways' ? 'Estimated gateways' : esc((LANDUSES[g] || { label: String(g) }).label);

  /* ================= standards as data ================= */
  const SOURCES = {
    tmdg: { id: 'tmdg', name: 'NZTA Transport Model Development Guidelines', short: 'TMDG 2019', edition: '2019', url: 'https://www.nzta.govt.nz/assets/resources/transport-model-development-guidelines/docs/tmd.pdf' },
    tag: { id: 'tag', name: 'UK TAG Unit M3.1 Highway Assignment Modelling', short: 'TAG M3.1', edition: 'May 2026', url: 'https://assets.publishing.service.gov.uk/media/6a033d074fb0713aa63ea802/tag-m3-1-highway-assignment-modelling.pdf' },
    rms: { id: 'rms', name: 'RMS Traffic Modelling Guidelines (superseded)', short: 'RMS 2013', edition: '2013', url: 'https://standards.transport.nsw.gov.au/_entity/annotation/23428da1-b835-ed11-9db1-000d3ae011f9' },
    atap: { id: 'atap', name: 'ATAP T1 Travel demand modelling, Appendix C', short: 'ATAP', edition: 'secondary extracts', url: 'https://www.atap.gov.au/tools-techniques/travel-demand-modelling/appendix-c' },
    ttoc: { id: 'ttoc', name: 'QLDC TTOC-06 Traffic signals modelling guidelines', short: 'TTOC-06', edition: 'QLDC', url: 'https://www.qldc.govt.nz/media/lekcx4iu/ttoc-06-traffic-signals-modelling-guidelines.pdf' },
    corridor: { id: 'corridor', name: 'Spare Construct assumption', short: 'Spare Construct assumption', edition: 'Spare Construct', url: '' },
  };
  const CATS = ['A', 'B', 'C', 'D', 'E', 'F', 'G'];
  const CAT_NAMES = { A: 'regional', B: 'strategic network', C: 'urban area', D: 'NZTA project', E: 'small area or corridor', F: 'intersection or short corridor', G: 'high-flow multi-lane' };
  const STANDARDS = { tmdg: 'NZTA TMDG 2019', rms: 'RMS 2013 (superseded)', atap: 'ATAP (unverified extracts)' };
  /* TMDG 2019 values by category (docs/spec/_validation-rules, read from tmd.pdf). vf = categories still to check
     against the PDF: A and B link GEH (two sources disagree; the primary-read values are shipped), the screenline
     GEH 7.5 / 10 columns. */
  const TMDG = {
    geh5: { v: { A: .65, B: .75, C: .80, D: .825, E: .85, F: .95, G: .85 }, vf: 'AB', table: 'Individual turning movements and directional link counts (GEH < 5)' },
    geh75: { v: { A: .75, B: .80, C: .85, D: .875, E: .90, F: 1, G: .90 }, vf: 'AB', table: 'Individual turning movements and directional link counts (GEH < 7.5)' },
    geh10: { v: { A: .85, B: .85, C: .90, D: .925, E: .95, F: 1, G: .95 }, vf: 'AB', table: 'Individual turning movements and directional link counts (GEH < 10)' },
    geh12: { v: { A: .95 }, vf: 'A', table: 'Individual counts (GEH < 12, Category A only)' },
    band: { v: { B: .70, C: .775, D: .85, E: .90, F: .95, G: .80 }, vf: '', table: 'Count band criteria (< 400 within 50 veh/h; 400–2,000 within 12.5%; > 2,000 within 250 veh/h)' },
    sgeh5: { v: { A: .60, B: .75, C: .85, D: .90 }, vf: '', table: 'Screenline totals (GEH < 5)' },
    sgeh75: { v: { A: .75, B: .85, C: .90, D: .95 }, vf: 'ABCD', table: 'Screenline totals (GEH < 7.5)' },
    sgeh10: { v: { A: .90, B: .95, C: .95, D: 1 }, vf: 'ABCD', table: 'Screenline totals (GEH < 10)' },
    sw10: { v: { A: .70 }, vf: '', table: 'Screenline totals within 10% (Category A)' },
    sw15: { v: { A: .80 }, vf: '', table: 'Screenline totals within 15% (Category A)' },
    r2: { v: { A: .85, B: .90, C: .95, D: .95, E: .95, F: .95, G: .95 }, vf: '', table: 'Scatter plot, trendline through the origin (R²)' },
    slope: { v: { A: [.90, 1.10], B: [.90, 1.10], C: [.90, 1.10], D: [.925, 1.075], E: [.95, 1.05], F: [.97, 1.03], G: [.97, 1.03] }, vf: '', table: 'Scatter plot, trendline through the origin (slope)' },
    rmse: { v: { A: [30, 40], B: [25, 35], C: [20, 30], D: [17.5, 27.5], E: [15, 25] }, vf: '', table: '%RMSE (acceptable; requires clarification)' },
    jt1: { v: { A: .80, B: .85, C: .85, D: .875, E: .90, F: .90, G: .90 }, vf: '', table: 'Journey times, tier 1 (within 15% or 1 minute)' },
    jt2: { v: { A: .85, B: .90, C: .90, D: .925, E: .95, F: 1, G: 1 }, vf: '', table: 'Journey times, tier 2 (within 25% or 1.5 minutes)' },
    meTotal: { v: { E: [5, 10], F: [7, 15] }, vf: '', table: 'Section 8, total matrix change (high-quality prior; low-quality prior)' },
    meCR: { v: { C: .70, D: .75, E: .80 }, vf: '', table: 'Section 8, trip-length coincidence ratio' },
  };
  /* TMDG Figure 5 applicability by category A–G: e expected, u potentially useful, n not expected. Only the Category E
     row and the F and G %RMSE cells come from the rules read so far (verified); the rest are Spare Construct readings to be
     transcribed from Figure 5 before release (verified: false). */
  const FIG5 = { link: 'eeeeeee', turn: 'eeeeeee', band: 'uuuuuuu', scatter: 'eeeeeee', rmse: 'uuuuunn', screen: 'eeeennn', jt: 'uuuuuuu' };
  const FIG5_OK = { E: ['link', 'turn', 'band', 'scatter', 'rmse', 'screen'], F: ['rmse'], G: ['rmse'] };
  const USE = { e: 'expected', u: 'useful', n: 'not-expected' };
  /* Count band rules (veh/h): an observed flow C under lo is met within absLo, from lo to hi within rel × C, above hi
     within absHi. TMDG count bands and the TAG M3.1 Table 2 flow band. */
  const BANDS = {
    tmdg: { lo: 400, hi: 2000, absLo: 50, rel: 0.125, absHi: 250, src: 'tmdg', table: 'Count band criteria', verified: true },
    tag: { lo: 700, hi: 2700, absLo: 100, rel: 0.15, absHi: 400, src: 'tag', table: 'Table 2', verified: true },
  };
  /* UK TAG M3.1 (May 2026), read in full (docs/spec/_validation-rules), shown alongside TMDG. Shares are of counts,
     screenline totals or routes. The journey-time test is the same check as TMDG tier 1 (DataHub.jtCheck). */
  const TAG = {
    src: 'tag', verified: true,
    either: { v: 0.85, table: 'Table 2', crit: 'GEH < 5 or within the flow band, more than 85%' },
    screen: { v: 0.95, within: 0.05, minLinks: 5, table: 'Table 1', crit: 'directional totals within 5%, all or nearly all (95%)' },
    jt: { v: 0.85, rel: 0.15, abs: 60, table: 'Table 3', crit: 'within 15% or 60 s, more than 85% of routes' },
    gap: { v: 0.1, table: 'Table 4', crit: 'relative gap, %' },
    p: { chg: 0.01, share: 0.98, consec: 4, table: 'Table 4', crit: 'more than 98% of links change by less than 1%, 4 iterations running' },
    cells: { slope: [0.98, 1.02], r2: 0.95, table: 'Table 5', crit: 'slope 0.98–1.02, R² > 0.95, intercept near 0' },
    ends: { slope: [0.99, 1.01], r2: 0.98, table: 'Table 5', crit: 'slope 0.99–1.01, R² > 0.98' },
    tld: { v: 5, table: 'Table 5', crit: 'mean and SD both within 5%' },
    sectors: { v: 0.05, table: 'Table 5', crit: 'sector-to-sector within 5%' },
  };
  // RMS 2013 preset (the document is marked superseded) and ATAP (values from secondary extracts, to verify)
  const RMS = { src: 'rms', table: 'RMS 2013 calibration criteria', verified: true,
    geh5: 0.95, geh10: 1, tgeh5: 0.85, tgeh10: 1, sgeh4: 1, r2: 0.9, rmse: 30, jt: 0.95 };
  const ATAP = { src: 'atap', table: 'Appendix C (secondary extract)', verified: false,
    within20: 0.85, sw10: 1, r2: 0.85, slope: [0.9, 1.1], rmse: 30 };
  /* Spare Construct conventions that decide a status or a flag (Spare Construct assumption unless a source is named). */
  const CORRIDOR = {
    minN: 10,                              // fewer observations than this: 'Indicative', never pass or fail
    holdEvery: 4,                          // hold-out: every 4th eligible count is a validation count (about 25%)
    tefBounds: [0.5, 2],                   // trip-end factor bounds
    meIntercept: 0.01,                     // ME cell intercept flagged above 1% of the mean prior cell (TAG Table 5 'near 0')
    meSectorPrior: 50,                     // ME sector cells tested only with a prior of 50 trips or more
    dos: { v: 0.95, src: 'ttoc' },         // model DoS above this: a stop-line count understates demand (TTOC-06)
    balance: { abs: 50, rel: 0.1 },        // turning-count balancing: a gap above max(50 veh/h, 10%) is a midblock source or sink
    stress: { factors: [1.1, 1.2], rise: 2 },   // stress test: flag a DoS rising more than twice the demand increase
  };
  const MIN_N = CORRIDOR.minN;
  // Observation methods and the matrix-estimation weight each would carry (Spare Construct suggestion, count-roles)
  const METHODS = {
    measured_hour: { label: 'Measured hour', short: 'measured', weight: 1 },
    weekday_factored: { label: 'Weekday mean × peak share', short: 'weekday', weight: 0.7 },
    adt_factored: { label: 'Daily × peak share', short: 'factored', weight: 0.4 },
    virtual: { label: 'NZTA estimate (virtual site)', short: 'virtual', weight: 0 },
  };
  const ROLES = { me: 'Calibration', valid: 'Validation', gateway: 'Gateway', off: 'Not used' };
  const GEH_BANDS = [[0, 5], [5, 7.5], [7.5, 10], [10, 12], [12, Infinity]];
  /* NZ school terms (Ministry of Education); a count inside the holidays is flagged. Term 1 uses the latest permitted
     start. To be checked against education.govt.nz each year (verified: false). */
  const SCHOOL_TERMS = { source: 'Ministry of Education, school term dates', url: 'https://www.education.govt.nz/school/term-dates/', verified: false,
    years: {
      2023: [['2023-02-07', '2023-04-06'], ['2023-04-24', '2023-06-30'], ['2023-07-17', '2023-09-22'], ['2023-10-09', '2023-12-20']],
      2024: [['2024-02-07', '2024-04-12'], ['2024-04-29', '2024-07-05'], ['2024-07-22', '2024-09-27'], ['2024-10-14', '2024-12-20']],
      2025: [['2025-02-07', '2025-04-11'], ['2025-04-28', '2025-07-04'], ['2025-07-21', '2025-09-26'], ['2025-10-13', '2025-12-19']],
      2026: [['2026-02-06', '2026-04-02'], ['2026-04-20', '2026-07-03'], ['2026-07-20', '2026-09-25'], ['2026-10-12', '2026-12-18']],
    } };
  /* area: 'auto' = inside the area of influence when there is one (it decides the status), else every count;
     'all' and 'outside' are for information only while an area of influence exists. */
  const DEFAULTS = { calib: { standard: 'tmdg', set: 'valid', area: 'auto', minComplete: 1, priorQuality: 'high', xamax: 2, freezeGG: true, noSinks: false, useBalanced: true, coreBuffer: 500, scatterLog: false, scatterNoMotorway: false,
    windows: { AM: ['07:00', '09:00'], IP: ['09:00', '15:00'], PM: ['16:00', '18:00'] } } };
  // The project settings, or null before a project exists (the criteria library works without one)
  const curS = () => { try { return Store.P ? Store.S : null; } catch { return null; } };
  const cfg = (s = curS()) => ({ ...DEFAULTS.calib, ...((s && s.calib) || {}), windows: { ...DEFAULTS.calib.windows, ...(((s && s.calib) || {}).windows || {}) } });
  const category = (s = curS()) => { const c = String((s && s.tmdg && s.tmdg.category) || 'E').toUpperCase(); return CATS.includes(c) ? c : 'E'; };

  /* ================= small geometry and statistics helpers ================= */
  const geh = (m, c) => Math.sqrt(2 * (m - c) ** 2 / Math.max(1, m + c));
  // TMDG count band and TAG M3.1 Table 2 flow band (veh/h), from BANDS
  const inBand = (B, m, c) => Math.abs(m - c) <= (c < B.lo ? B.absLo : c <= B.hi ? B.rel * c : B.absHi);
  const tmdgBand = (m, c) => inBand(BANDS.tmdg, m, c);
  const tagBand = (m, c) => inBand(BANDS.tag, m, c);
  // Counts needed for a share target: TMDG and TAG targets are strict ('E > 85'), so 17 of 20 (85.0%) is not enough; 100% means all
  const need = (target, n) => target >= 1 ? n : Math.min(n, Math.floor(target * n + 1e-9) + 1);
  const rmsePct = (rows, so) => rows.length > 1 && so > 0 ? 100 * Math.sqrt(rows.reduce((s, r) => s + (r.model - r.obs) ** 2, 0) / (rows.length - 1)) / (so / rows.length) : null;
  function hash(s) { let h = 0x811c9dc5; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; } return h; }
  /* Minutes after midnight from 'HH:MM', 'H.MM', 'HHMM', '7:15 am' or a date-time such as '2026-03-10 07:15:00' or
     '10/03/2026 7:15' (the last time in the cell); null when no valid time can be read. */
  const toMin = t => {
    const s = String(t ?? '').trim(), all = [...s.matchAll(/(\d{1,2})[:.](\d{2})(?::\d{2}(?:\.\d+)?)?\s*([ap]\.?m\.?)?/gi)];
    let h, m, ap;
    if (all.length) [, h, m, ap] = all[all.length - 1];
    else { const x = /^(\d{1,2})(\d{2})$/.exec(s); if (!x) return null; [, h, m] = x; }
    h = +h; m = +m;
    if (ap) { const pm = /^p/i.test(ap); if (h < 1 || h > 12) return null; h = h % 12 + (pm ? 12 : 0); }
    return h < 24 && m < 60 ? h * 60 + m : null;
  };
  /* A count date as ISO yyyy-mm-dd from ISO, yyyy/mm/dd, or NZ dd/mm/yyyy (also d-m-yy and dd.mm.yyyy); a date-time
     cell gives its date. null when the cell holds something that is not a real date. */
  function isoDate(v) {
    const s = String(v ?? '').trim(); if (!s) return '';
    let y, mo, d, m = /^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})(?!\d)/.exec(s);
    if (m) [, y, mo, d] = m;
    else if ((m = /^(\d{1,2})[-/.](\d{1,2})[-/.](\d{2}|\d{4})(?!\d)/.exec(s))) { [, d, mo, y] = m; if (y.length === 2) y = '20' + y; }
    else return null;
    const D = new Date(Date.UTC(+y, +mo - 1, +d));
    if (D.getUTCFullYear() !== +y || D.getUTCMonth() !== +mo - 1 || D.getUTCDate() !== +d) return null;
    return `${y}-${String(mo).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
  }
  const hhmm = m => `${String(Math.floor(m / 60) % 24).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
  const isMw = L => !!L && (L.cls === 'motorway' || L.cls === 'ramp');
  const DIR4 = ['NB', 'EB', 'SB', 'WB'];
  const dirOf = b => DIR4[Math.round((((b % 360) + 360) % 360) / 90) % 4];
  function pointInPoly(ll, poly) {
    let inside = false;
    for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
      const [yi, xi] = poly[i], [yj, xj] = poly[j];
      if ((yi > ll[0]) !== (yj > ll[0]) && ll[1] < (xj - xi) * (ll[0] - yi) / (yj - yi) + xi) inside = !inside;
    }
    return inside;
  }
  // Distance (m) from a point to a polyline of [lat, lng]
  function distToLine(ll, pts) {
    const F = U.flat(ll[0]), px = F.x(ll), py = F.y(ll);
    let d = Infinity;
    for (let i = 0; i < pts.length - 1; i++) d = Math.min(d, U.segDist(px, py, F.x(pts[i]), F.y(pts[i]), F.x(pts[i + 1]), F.y(pts[i + 1])).d);
    return d;
  }
  function statsOf(rows) {
    const n = rows.length, cnt = { geh5: 0, geh75: 0, geh10: 0, geh12: 0, geh5le: 0, geh10le: 0, band: 0, tag: 0, either: 0, within20: 0 };
    let so = 0, sm = 0;
    for (const r of rows) {
      const g = r.geh;
      so += r.obs; sm += r.model;
      if (g < 5) cnt.geh5++; if (g < 7.5) cnt.geh75++; if (g < 10) cnt.geh10++; if (g < 12) cnt.geh12++;
      if (g <= 5) cnt.geh5le++; if (g <= 10) cnt.geh10le++;
      const b = tmdgBand(r.model, r.obs), t = tagBand(r.model, r.obs);
      if (b) cnt.band++; if (t) cnt.tag++; if (g < 5 || t) cnt.either++;
      if (Math.abs(r.model - r.obs) <= 0.2 * r.obs) cnt.within20++;
    }
    const share = k => n ? cnt[k] / n : null;
    const reg = n >= 2 ? DataHub.regression(rows.map(r => r.obs), rows.map(r => r.model)) : {};
    let cum = 0;
    const gehBands = GEH_BANDS.map(([lo, hi]) => { const k = rows.filter(r => r.geh >= lo && r.geh < hi).length; cum += k; return { lo, hi, label: hi === Infinity ? `≥ ${lo}` : lo === 0 ? `< ${hi}` : `${lo}–${hi}`, n: k, share: n ? k / n : null, cum: n ? cum / n : null }; });
    const out = { n, cnt, totObs: so, totMod: sm, rmse: rmsePct(rows, so), b0: reg.b0 ?? null, r2: reg.r2OriginCentred ?? null, r2Excel: reg.r2OriginExcel ?? null, r2Ols: reg.r2Ols ?? null, olsA: reg.olsA ?? null, olsB: reg.olsB ?? null, gehBands };
    for (const k of Object.keys(cnt)) out[k] = share(k);
    return out;
  }

  /* ================= project data ================= */
  function store(P = Store.P) {
    const d = P.data ||= {};
    const c = d.calib ||= {};
    c.roles ||= {}; c.screenlines ||= []; c.turnSites ||= [];
    return c;
  }
  const D = () => DataHub.D();
  const memo = new Map();
  // Cache a computation on the store version (cleared on every emit)
  function cached(key, fn) { const k = Store.ver + '|' + key; if (memo.has(k)) return memo.get(k); if (memo.size > 60) memo.clear(); const v = fn(); memo.set(k, v); return v; }

  /* ================= count roles and observation methods ================= */
  // Counts that set gateway flows: DataHub.gatewaysFromCounts writes each one's label into z.gwSrc
  const countLabel = c => `${(DataHub.SOURCES[c.src] || { short: c.src }).short}${c.virtual ? ' estimate' : ''} ${c.ref || ''} ${c.name}`;
  function gatewayIds() {
    return cached('gw', () => {
      const srcs = [];
      for (const s of Store.P.scenarios) for (const z of Object.values(s.zones || {})) if (z.kind === 'gateway' && z.gwSrc) srcs.push(z.gwSrc);
      const out = new Set();
      if (srcs.length) for (const c of D().counts) { const l = countLabel(c); if (srcs.some(g => g.includes(l))) out.add(c.id); }
      return out;
    });
  }
  // The role the user (or the hold-out action) gave; 'me' when none was given
  const baseRole = c => { const r = store().roles[c.id]; return ROLES[r] && r !== 'gateway' ? r : 'me'; };
  function meUsed(period) {
    const info = Store.P.me && Store.P.me.info && Store.P.me.info[period];
    if (!info) return null;
    return Array.isArray(info.countIds) ? new Set(info.countIds) : 'all';
  }
  // Counts that fitted trip-end factors now applied to the trip rates (any scenario) in this period
  function tefUsed(period) {
    return cached('tef|' + period, () => {
      const out = new Set();
      for (const s of Store.P.scenarios) { const r = s.calib && s.calib.tripEndFactors && s.calib.tripEndFactors[period]; if (r && r.applied && Array.isArray(r.countIds)) for (const id of r.countIds) out.add(id); }
      return out;
    });
  }
  /* The role that counts for statistics in a period: gateway use always wins (it sets demand), and a validation count
     that matrix estimation saw (estimation run outside this panel saw every count) or that fitted applied trip-end
     factors is calibration. */
  function roleOf(c, period, raw = false) {
    const r = baseRole(c);
    if (r === 'off') return 'off';
    if (gatewayIds().has(c.id)) return 'gateway';
    if (raw || r !== 'valid') return r;
    const used = meUsed(period);
    return (used && (used === 'all' || used.has(c.id))) || tefUsed(period).has(c.id) ? 'me' : 'valid';
  }
  // Why a count given the validation role is a calibration count in this period ('' when it is not)
  function seenBy(c, period) {
    if (baseRole(c) !== 'valid' || roleOf(c, period) !== 'me') return '';
    const used = meUsed(period);
    return used && (used === 'all' || used.has(c.id)) ? 'seen by ME' : 'used by trip-end factors';
  }
  function methodOf(c, period) {
    if (typeof c.obs_method === 'string' && METHODS[c.obs_method]) return c.obs_method;
    if (c.virtual) return 'virtual';
    if (c.src === 'user') return (period === 'AM' ? c.am : c.pm) ? 'measured_hour' : 'adt_factored';
    if (c.src === 'at' && c.peak && c.peakTime) {
      const h = +c.peakTime.slice(0, 2) + (+c.peakTime.slice(3)) / 60;
      if ((period === 'AM' && h >= 6 && h < 10) || (period === 'PM' && h >= 14.5 && h < 19)) return 'measured_hour';
    }
    return 'adt_factored';
  }
  function setRole(id, role) {
    const st = store();
    if (role === 'me' || !ROLES[role] || role === 'gateway') delete st.roles[id]; else st.roles[id] = role;
    syncCounts();
  }
  // Mirror roles onto the counts so other features (report, exports) can read c.role
  function syncCounts() { memo.clear(); for (const c of D().counts) c.role = roleOf(c, (Store.S && Store.S.period) || 'AM', true); }
  /* Hold out about 25% for validation (count-roles): eligible counts sorted by road-class rank, then by a hash of the
     id, and every 4th becomes 'valid' (deterministic; Spare Construct convention). The rest become calibration. */
  function holdOut(sc = Store.sc, every = CORRIDOR.holdEvery) {
    const m = DataHub.matches(sc), gw = gatewayIds(), st = store();
    const el = D().counts.filter(c => DataHub.inUse(c) && !c.virtual && m.get(c.id) && !gw.has(c.id) && baseRole(c) !== 'off').map(c => {
      const L = sc.links[m.get(c.id).link];
      return { c, rank: L ? (CLASSES[L.cls] || CLASSES.local).rank : 0, h: hash(c.id) };
    }).sort((a, b) => b.rank - a.rank || a.h - b.h);
    // a fresh hold-out: every role except 'not used' is cleared first, so the result depends only on the counts
    for (const id of Object.keys(st.roles)) if (st.roles[id] !== 'off') delete st.roles[id];
    let k = 0;
    el.forEach((x, i) => { if (i % every === every - 1) { st.roles[x.c.id] = 'valid'; k++; } else delete st.roles[x.c.id]; });
    syncCounts();
    return { valid: k, calib: el.length - k };
  }
  function resetRoles() { const st = store(); for (const id of Object.keys(st.roles)) if (st.roles[id] !== 'off') delete st.roles[id]; syncCounts(); }

  /* ================= area of influence ================= */
  // A drawn 'core' polygon, else a 500 m buffer of the links that differ from the base scenario, else everything
  function coreArea(sc = Store.sc) {
    return cached('core|' + sc.id, () => {
      const st = store(), poly = st.screenlines.find(s => s.type === 'core' && s.pts.length >= 3);
      if (poly) return { kind: 'user', poly: poly.pts, label: `inside “${poly.name}”` };
      const base = Store.baseSc;
      if (!base || base === sc) return { kind: 'none', label: 'no area of influence: this is the base scenario, so every count is inside' };
      const K = ['a', 'b', 'cls', 'lanesAB', 'lanesBA', 'speed', 'status', 'capPerLane'], lines = [];
      for (const L of Object.values(sc.links)) { const B = base.links[L.id]; if (!B || K.some(k => L[k] !== B[k])) if (sc.nodes[L.a] && sc.nodes[L.b]) lines.push(Net.geom(sc, L)); }
      for (const B of Object.values(base.links)) if (!sc.links[B.id] && base.nodes[B.a] && base.nodes[B.b]) lines.push(Net.geom(base, B));
      if (!lines.length) return { kind: 'none', label: 'no area of influence: no road differs from the base scenario, so every count is inside' };
      const r = cfg().coreBuffer;
      return { kind: 'buffer', lines, r, label: `within ${U.fmt.int(r)} m of the ${lines.length} road${lines.length === 1 ? '' : 's'} that differ from the base scenario (Spare Construct assumption)` };
    });
  }
  function inCore(core, ll) {
    if (!core || core.kind === 'none') return true;
    if (core.kind === 'user') return pointInPoly(ll, core.poly);
    return core.lines.some(g => distToLine(ll, g) <= core.r);
  }

  /* ================= counts against a result ================= */
  function countRows(sc, period, res = Store.res(sc, period), raw = false) {
    if (!res) return [];
    return cached(`rows|${sc.id}|${period}|${res.ts}|${raw}`, () => {
      const m = DataHub.matches(sc), core = coreArea(sc), out = [];
      for (const c of D().counts) {
        if (!DataHub.inUse(c)) continue;
        const mt = m.get(c.id); if (!mt) continue;
        const pv = DataHub.periodVol(c, period); if (!pv || !(pv.v >= 0)) continue;
        const a = res.arcs[mt.link + ':ab'], b = res.arcs[mt.link + ':ba'];
        const model = mt.dir === 'ab' ? (a ? a.v : null) : mt.dir === 'ba' ? (b ? b.v : null) : (a ? a.v : 0) + (b ? b.v : 0);
        if (model == null) continue;
        const L = sc.links[mt.link], method = methodOf(c, period), role = roleOf(c, period, raw);
        out.push({ c, mt, L, obs: pv.v, model, how: pv.how, geh: geh(model, pv.v), method, role, scored: method !== 'virtual' && role !== 'off', inCore: inCore(core, [c.lat, c.lng]), mw: isMw(L) });
      }
      return out.sort((x, y) => y.geh - x.geh);
    });
  }
  const inSet = (r, set) => r.scored && (set === 'valid' ? r.role === 'valid' : set === 'calib' ? r.role === 'me' || r.role === 'gateway' : true);
  const inArea = (r, area) => area === 'inside' ? r.inCore : area === 'outside' ? !r.inCore : true;
  const SET_LABEL = { valid: 'validation counts', calib: 'calibration counts', all: 'all counts' };

  /* ================= journey times ================= */
  function journeys(sc, period, res = Store.res(sc, period)) {
    const out = [];
    for (const j of D().jt) {
      let mt = null, len = null;
      if (sc.nodes[j.from] && sc.nodes[j.to]) { const r = Engine.travel(sc, res, j.from, j.to); if (r && !r.none) { mt = r.time; len = r.len; } }
      const ob = j.obs && +j.obs[period] > 0 ? { t: +j.obs[period], kind: 'observed', label: j.obs.src || 'observed' } : null;
      const g = j.g && j.g[period] && j.g[period].t > 0 ? { t: j.g[period].t, kind: 'predicted', label: DataHub.JT_CRITERIA.googleLabel, when: j.g[period].when } : null;
      const ref = ob || g, chk = mt != null && ref ? DataHub.jtCheck(mt, ref.t) : null;
      out.push({ j, model: mt, len, obs: ob, google: g, ref, chk, short: len != null && len < 3000, long: len != null && len > 15000 });
    }
    return out;
  }
  function jtStats(rows) {
    const obs = rows.filter(r => r.chk && r.obs), use = obs.length ? obs : rows.filter(r => r.chk);
    const n = use.length, t1 = use.filter(r => r.chk.tier1).length, t2 = use.filter(r => r.chk.tier2).length;
    return { n, cnt: { jt1: t1, jt2: t2 }, jt1: n ? t1 / n : null, jt2: n ? t2 / n : null, predicted: !obs.length && n > 0 };
  }

  /* ================= screenlines and cordons ================= */
  function slCrossings(sc, sl) {
    return cached(`slx|${sc.id}|${sl.id}|${sl.pts.length}|${sl.type}`, () => {
      const closed = sl.type !== 'screen', P = sl.pts.slice();
      if (P.length < 2) return [];
      if (closed && P.length >= 3) {
        // counter-clockwise, so crossing an edge to its left means going inside
        let a = 0; for (let i = 0; i < P.length; i++) { const p = P[i], q = P[(i + 1) % P.length]; a += (q[1] - p[1]) * (q[0] + p[0]); }
        if (a > 0) P.reverse();
        P.push(P[0]);
      }
      const F = U.flat(P[0][0]), xy = ll => [F.x(ll), F.y(ll)], S = P.map(xy);
      const bx = [Math.min(...S.map(p => p[0])), Math.max(...S.map(p => p[0])), Math.min(...S.map(p => p[1])), Math.max(...S.map(p => p[1]))];
      const out = [];
      for (const L of Object.values(sc.links)) {
        if (L.status === 'closed' || !sc.nodes[L.a] || !sc.nodes[L.b]) continue;
        const g = Net.geom(sc, L).map(xy);
        if (Math.max(...g.map(p => p[0])) < bx[0] || Math.min(...g.map(p => p[0])) > bx[1] || Math.max(...g.map(p => p[1])) < bx[2] || Math.min(...g.map(p => p[1])) > bx[3]) continue;
        for (let i = 0; i < g.length - 1; i++) for (let k = 0; k < S.length - 1; k++) {
          const p = g[i], r = [g[i + 1][0] - p[0], g[i + 1][1] - p[1]], q = S[k], s = [S[k + 1][0] - q[0], S[k + 1][1] - q[1]];
          const den = r[0] * s[1] - r[1] * s[0]; if (Math.abs(den) < 1e-9) continue;
          const t = ((q[0] - p[0]) * s[1] - (q[1] - p[1]) * s[0]) / den, u = ((q[0] - p[0]) * r[1] - (q[1] - p[1]) * r[0]) / den;
          if (t < 0 || t > 1 || u < 0 || u > 1) continue;
          // + when A→B crosses from the right of the screenline segment to its left
          const side = s[0] * r[1] - s[1] * r[0] > 0 ? 1 : -1;
          const ll = [(p[1] + t * r[1]) / F.ky, (p[0] + t * r[0]) / F.kx];
          out.push({ link: L.id, L, side, ll, bAB: (Math.atan2(r[0], r[1]) * 180 / Math.PI + 360) % 360, mw: isMw(L) });
        }
      }
      return out;
    });
  }
  // Heading of the screenline's left side (traffic crossing in the + sense heads this way)
  function slLeft(sl) {
    const a = sl.pts[0], b = sl.pts[sl.pts.length - 1];
    return (U.bearing(a, b) + 270) % 360;
  }
  /* Per direction: ΣC on counted arcs, ΣM on counted arcs and on every crossing arc, completeness = ΣM counted / ΣM
     all. A two-way count covers both directions only, so it feeds the 'Both directions' row. */
  function slRows(sc, sl, period, res = Store.res(sc, period), opts = {}) {
    if (!res) return [];
    const xs = slCrossings(sc, sl).filter(x => opts.motorways !== false || !x.mw);
    const m = DataHub.matches(sc), byArc = new Map(), byLink = new Map();
    for (const c of D().counts) {
      if (!DataHub.inUse(c) || c.virtual) continue;
      const mt = m.get(c.id); if (!mt) continue;
      const pv = DataHub.periodVol(c, period); if (!pv) continue;
      const e = { c, v: pv.v, role: roleOf(c, period), method: methodOf(c, period) };
      if (e.role === 'off') continue;
      if (mt.dir === 'both') { if (!byLink.has(mt.link)) byLink.set(mt.link, e); } else { const k = mt.link + ':' + mt.dir; if (!byArc.has(k)) byArc.set(k, e); }
    }
    const closed = sl.type !== 'screen', left = slLeft(sl), rows = [];
    const mk = (label, items) => {
      let C = 0, Mc = 0, M = 0, nc = 0;
      const used = [];
      for (const it of items) { M += it.m; if (it.cnt) { C += it.cnt.v; Mc += it.m; nc++; used.push(it.cnt); } }
      const complete = M > 0 ? Mc / M : items.length ? nc / items.length : 0;
      const ok = items.length > 0 && complete >= Math.min(1, cfg().minComplete) - 1e-9;
      const g = C + Mc > 0 ? geh(Mc, C) : null, d = Mc - C, p = C > 0 ? d / C : null;
      return { label, n: items.length, counted: nc, C, Mc, M, complete, ok, geh: g, d, pct: p, w5: p != null && Math.abs(p) <= TAG.screen.within, w10: p != null && Math.abs(p) <= 0.10, w15: p != null && Math.abs(p) <= 0.15,
        notIndependent: sl.role === 'valid' && used.some(u => u.role !== 'valid'), used: used.map(u => u.c.id), arcs: items.filter(it => it.cnt).flatMap(it => it.key.includes(':') ? [it.key] : [it.key + ':ab', it.key + ':ba']),
        roles: used.map(u => u.role),
        text: ok ? 'complete' : `incomplete (${nc} of ${items.length} counted)` };
    };
    const dirItems = sgn => {
      const seen = new Set(), items = [];
      for (const x of xs) {
        const key = x.link + (x.side === sgn ? ':ab' : ':ba'), r = res.arcs[key];
        if (!r || seen.has(key)) continue;
        seen.add(key); items.push({ key, link: x.link, m: r.v, cnt: byArc.get(key) || null });
      }
      return items;
    };
    for (const sgn of [1, -1]) {
      const items = dirItems(sgn);
      if (items.length) rows.push({ ...mk(closed ? (sgn === 1 ? 'Inbound' : 'Outbound') : dirOf(sgn === 1 ? left : left + 180), items), dir: sgn });
    }
    // Both directions: two-way counts against the two-way model, plus directional counts on both arcs of a link
    const links = [...new Set(xs.map(x => x.link))], both = [];
    for (const id of links) {
      const ab = res.arcs[id + ':ab'], ba = res.arcs[id + ':ba'], mm = (ab ? ab.v : 0) + (ba ? ba.v : 0);
      let cnt = byLink.get(id) || null;
      const ca = byArc.get(id + ':ab'), cb = byArc.get(id + ':ba');
      if (!cnt && (ca || !ab) && (cb || !ba) && (ca || cb)) cnt = { v: (ca ? ca.v : 0) + (cb ? cb.v : 0), role: [ca, cb].some(x => x && x.role !== 'valid') ? 'me' : 'valid', c: (ca || cb).c };
      both.push({ key: id, link: id, m: mm, cnt });
    }
    if (both.length) rows.push({ ...mk('Both directions', both), dir: 0 });
    return rows;
  }
  // Observations for the summary: each complete directional total; a screenline with neither direction complete
  // but a complete two-way total gives that one observation
  function slObservations(sc, period, set) {
    const out = [], dropped = [];
    for (const sl of store().screenlines) {
      if (sl.type === 'core') continue;
      if (set === 'valid' && sl.role !== 'valid') continue;
      if (set === 'calib' && sl.role === 'valid') continue;
      const rows = slRows(sc, sl, period, undefined, { motorways: sl.motorways !== false });
      // validation statistics use validation counts only: a total that adds in calibration or gateway counts is left out
      const indep = r => set !== 'valid' || !r.notIndependent;
      for (const r of rows) if (r.ok && !indep(r)) dropped.push(`${sl.name} ${r.label}`);
      const dirs = rows.filter(r => r.dir && r.ok && indep(r)), both = rows.find(r => !r.dir && r.ok && indep(r));
      if (dirs.length) dirs.forEach(r => out.push({ sl, ...r })); else if (both) out.push({ sl, ...both });
    }
    const n = out.length, k = f => out.filter(f).length;
    const cnt = { sgeh5: k(r => r.geh < 5), sgeh75: k(r => r.geh < 7.5), sgeh10: k(r => r.geh < 10), sgeh4: k(r => r.geh < 4), sw5: k(r => r.w5), sw10: k(r => r.w10), sw15: k(r => r.w15) };
    const s = { n, cnt, rows: out, dropped };
    for (const key of Object.keys(cnt)) s[key] = n ? cnt[key] / n : null;
    return s;
  }

  /* ================= turning counts ================= */
  const LEGS = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
  const legBearing = l => LEGS.indexOf(l) * 45;
  const MOVES = /^(L|T|R|U|L1|L2|L3|T1|R1|R2|R3)$/;
  const VEH = ['LV', 'HV', 'BUS'];
  /* Vehicle classes as they appear in NZ count files, onto LV / HV / BUS / CYC (cycles are kept but are not vehicles in
     the totals). NZTA's MCV, HCV-I and HCV-II are heavy; cars, light commercials and motorcycles are light. A class
     not listed here is skipped and named in the import message. */
  const CLASS_OF = { LV: 'LV', L: 'LV', LIGHT: 'LV', CAR: 'LV', CARS: 'LV', PC: 'LV', LCV: 'LV', VAN: 'LV', UTE: 'LV', MC: 'LV', MCY: 'LV', MOTORCYCLE: 'LV', TAXI: 'LV',
    HV: 'HV', H: 'HV', HEAVY: 'HV', MCV: 'HV', HCV: 'HV', HCVI: 'HV', HCVII: 'HV', HCV1: 'HV', HCV2: 'HV', TRUCK: 'HV', TRUCKS: 'HV', BUS: 'BUS', BUSES: 'BUS', COACH: 'BUS',
    CYC: 'CYC', CYCLE: 'CYC', CYCLES: 'CYC', BIKE: 'CYC', BICYCLE: 'CYC' };
  const classOf = v => CLASS_OF[String(v || '').trim().toUpperCase().replace(/[\s_.-]/g, '')] || null;
  /* Import a turning-count CSV: long (site_id, site_name, lat, lng, date, time_start, interval_min, from_leg, movement,
     class, count), wide (time_start plus <leg>_<mov>_<class> columns) or SCATS (a detector column; a shared-lane
     movement such as 'LT' is split equally between its movements, Spare Construct assumption). from_leg is where vehicles
     come FROM. movement IN / OUT gives leg totals for turn estimation. A Spare Construct SIDRA volumes export
     (corridor-sidra-volumes.csv, Junctions.VOL_HEAD) is converted first (Junctions.asCountCSV): it is modelled, not
     counted, so its sites are labelled and get the role 'Not used' (validating the model against itself proves
     nothing). Returns the imported sites. */
  const MODEL_FLAG = 'Spare Construct model volumes (SIDRA volumes export), not a count: not used for validation until you give the site a role';
  function fromSidraVolumes(text) {
    if (typeof Junctions === 'undefined' || !Array.isArray(Junctions.VOL_HEAD) || typeof Junctions.asCountCSV !== 'function') return null;
    const head = ((U.parseCSV(text.split(/\r?\n/, 1)[0] || '')[0]) || []).map(x => String(x).trim().toLowerCase());
    return Junctions.VOL_HEAD.every(k => head.includes(k)) ? String(Junctions.asCountCSV(text)) : null;
  }
  function parseTurnCSV(text, name = 'turning counts') {
    const raw0 = String(text).replace(/^﻿/, ''), sv = fromSidraVolumes(raw0), model = sv != null;
    const rows = U.parseCSV(model ? sv : raw0);
    if (rows.length < 2) throw new Error('The file has no data rows.');
    const h = rows[0].map(s => s.trim().toLowerCase().replace(/\s+/g, '_'));
    const col = (...n) => h.findIndex(x => n.includes(x));
    const iT = col('time_start', 'time', 'start', 'interval_start'); if (iT < 0) throw new Error('Add a time_start column (HH:MM).');
    const iSite = col('site_id', 'site'), iName = col('site_name', 'name'), iLat = col('lat', 'latitude'), iLng = col('lng', 'lon', 'long', 'longitude'), iDate = col('date', 'count_date');
    const iInt = col('interval_min', 'interval'), iLeg = col('from_leg', 'leg', 'approach'), iMv = col('movement', 'mov', 'turn'), iCls = col('class', 'vehicle_class'), iN = col('count', 'volume', 'vehicles');
    const iDet = col('detector', 'det'), iRoad = col('from_road', 'road');
    const wide = h.map((x, i) => [i, /^(n|ne|e|se|s|sw|w|nw)_(l|t|r|u|l1|l2|l3|t1|r1|r2|r3|in|out)(?:_(lv|hv|bus|cyc))?$/i.exec(x)]).filter(([, m]) => m);
    if (iLat < 0 || iLng < 0) throw new Error('Add lat and lng columns (WGS84) so each site can be matched to a junction.');
    if (!wide.length && (iLeg < 0 || iMv < 0 || iN < 0)) throw new Error('Use the long layout (from_leg, movement, count columns) or the wide layout (columns such as N_L_LV).');
    const sites = new Map(), errs = [], why = new Map(), skip = (li, w) => { errs.push(li + 2); why.set(w, (why.get(w) || 0) + 1); };
    const cell = (r, i) => i >= 0 && r[i] != null ? String(r[i]).trim() : '';
    const siteOf = r => {
      // the date column, else the date part of a date-time in the time column; an unreadable date is kept as written
      const raw = cell(r, iDate) || (/\d{1,4}[-/.]\d{1,2}[-/.]\d{2,4}/.test(cell(r, iT)) ? cell(r, iT) : ''), iso = isoDate(raw);
      const sid = cell(r, iSite) || 'site 1', date = iso == null ? raw : iso;
      const key = sid + '|' + date;
      let s = sites.get(key);
      if (!s) {
        const lat = +r[iLat], lng = +r[iLng];
        if (!isFinite(lat) || !isFinite(lng)) return null;
        s = { id: U.uid('tc'), ref: sid, name: cell(r, iName) || `Turning count ${sid}`, lat: U.r7(lat), lng: U.r7(lng), date, src: model ? 'corridor-sidra' : iDet >= 0 ? 'scats' : 'mcc', file: name,
          role: model ? 'off' : 'valid', obs_method: model ? 'modelled' : 'measured_hour', interval: null, bins: {}, legRoads: {}, flags: [], at: Date.now() };
        if (model) s.flags.push(MODEL_FLAG);
        if (iDet >= 0) s.flags.push('stop-line throughput (SCATS detectors)');
        sites.set(key, s);
      }
      return s;
    };
    const add = (s, t, iv, leg, mv, cls, n) => {
      if (!(n >= 0)) return;
      const b = Math.floor(t / 15) * 15;   // 5- or 10-minute intervals fold into 15-minute bins
      const bin = s.bins[b] ||= {}, k = leg + '-' + mv, e = bin[k] ||= {};
      e[cls] = (e[cls] || 0) + n;
      s.interval = s.interval ? Math.max(s.interval, iv) : iv;
    };
    rows.slice(1).forEach((r, li) => {
      if (r.every(x => !String(x ?? '').trim())) return;   // blank line
      const s = siteOf(r); if (!s) { skip(li, 'no lat / lng'); return; }
      const t = toMin(r[iT]); if (t == null) { skip(li, 'time not readable (use HH:MM)'); return; }
      const iv = iInt >= 0 && +r[iInt] > 0 ? +r[iInt] : 15;
      if (wide.length) {
        for (const [i, m] of wide) add(s, t, iv, m[1].toUpperCase(), m[2].toUpperCase(), (m[3] || 'LV').toUpperCase(), +r[i]);
        return;
      }
      const leg = cell(r, iLeg).toUpperCase(), mv = cell(r, iMv).toUpperCase(), n = +cell(r, iN), rc = cell(r, iCls), cls = rc ? classOf(rc) : 'LV';
      if (!cls) { skip(li, `vehicle class “${rc.slice(0, 20)}” not recognised`); return; }
      if (!LEGS.includes(leg)) { skip(li, 'from_leg not N, NE … NW'); return; }
      if (!(n >= 0)) { skip(li, 'count not a number'); return; }
      if (iRoad >= 0 && cell(r, iRoad)) s.legRoads[leg] = cell(r, iRoad);
      if (mv === 'IN' || mv === 'OUT' || MOVES.test(mv)) { add(s, t, iv, leg, mv, cls, n); return; }
      // SCATS shared lane: split equally between its movements
      if (/^[LTRU]{2,4}$/.test(mv)) { for (const x of mv) add(s, t, iv, leg, x, cls, n / mv.length); if (!s.flags.includes('shared-lane detector counts split equally (Spare Construct assumption)')) s.flags.push('shared-lane detector counts split equally (Spare Construct assumption)'); return; }
      skip(li, 'movement not L, T, R, U, IN or OUT');
    });
    const list = [...sites.values()].filter(s => Object.keys(s.bins).length);
    const reasons = [...why].map(([w, k]) => `${k} × ${w}`);
    if (!list.length) throw new Error(`No rows could be read${reasons.length ? ` (${reasons.join('; ')})` : ''}. Check the time_start, from_leg (N, NE … NW), movement (L, T, R, U), class and count columns.`);
    for (const s of list) { s.interval = s.interval || 15; if (s.interval > 15) s.flags.push(`${s.interval}-minute intervals: no peak hour factor`); peaks(s); s.peakWin = JSON.stringify(cfg().windows); flagDate(s); }
    return { sites: list, skipped: errs.length, skippedRows: errs.slice(0, 10), reasons, fromModel: model };
  }
  // Peak hours again after the peak-hour windows change (balanced counts for a moved peak are dropped)
  function repeak(P = Store.P) {
    if (!P) return 0;
    const w = JSON.stringify(cfg().windows); let n = 0;
    for (const s of store(P).turnSites) {
      if (s.peakWin === w || !s.bins) continue;
      const old = JSON.stringify(s.peak || {}); peaks(s); s.peakWin = w;
      if (JSON.stringify(s.peak) !== old) { delete s.bal; delete s.audit; n++; }
    }
    return n;
  }
  function importTurnCSV(text, name) {
    const r = parseTurnCSV(text, name), st = store();
    // A re-import of the same site and date replaces it
    for (const s of r.sites) { const i = st.turnSites.findIndex(x => x.ref === s.ref && x.date === s.date); if (i >= 0) st.turnSites.splice(i, 1, s); else st.turnSites.push(s); }
    return r;
  }
  /* One school-term calendar for the app: the feeds feature's (Feeds.TERMS.dates, which TMS and GTFS also use) when it
     is installed, otherwise this file's own copy. Both are Spare Construct's entry of the MoE dates (verify each year). */
  const termsOf = y => { try { const F = typeof Feeds !== 'undefined' && Feeds.TERMS && Feeds.TERMS.dates; if (F && F[y]) return F[y]; } catch { } return SCHOOL_TERMS.years[y]; };
  /* A count's 95% confidence interval: its own ci95, else the NZTA TMS Tue–Thu term-time mean's (c.tms.target.ci95,
     veh/day, from the feeds feature). → { text, csv } or null */
  function ciOf(c) {
    if (c && c.ci95 != null && c.ci95 !== '') return { text: String(c.ci95), csv: c.ci95 };
    const t = c && c.tms && c.tms.target;
    if (t && t.ci95 != null && isFinite(t.ci95)) return { text: `± ${U.fmt.int(t.ci95)} veh/day on the TMS mean of ${U.fmt.int(t.mean)} veh/day (${t.n} weekdays)`, csv: `±${Math.round(t.ci95)} veh/day (TMS)` };
    return null;
  }
  function flagDate(s) {
    if (!s.date) return;
    if (!/^\d{4}-\d{2}-\d{2}$/.test(s.date)) { s.flags.push(`count date “${String(s.date).slice(0, 20)}” could not be read, so the weekend and school-holiday checks were not made`); return; }
    const y = +s.date.slice(0, 4), T = termsOf(y), dow = new Date(s.date + 'T12:00:00Z').getUTCDay();
    if (dow === 0 || dow === 6) s.flags.push('weekend count');
    if (T && !T.some(([a, b]) => s.date >= a && s.date <= b)) s.flags.push('counted in the school holidays (term dates to verify)');
    if (!T) s.flags.push(`school term dates for ${y} are not in Spare Construct's table, so the school-holiday check was not made`);
  }
  /* Rolling peak hour within each window, stepping by the count interval (4 × 15 minutes, 2 × 30 minutes or one
     hour); PHF = V60 / (4 · V15max), so only 15-minute (or finer) counts have one. */
  function peaks(s, windows = cfg().windows) {
    const tot = new Map();
    for (const [t, bin] of Object.entries(s.bins)) { let v = 0; for (const [k, e] of Object.entries(bin)) if (!/-(IN|OUT)$/.test(k)) for (const c of VEH) v += e[c] || 0; tot.set(+t, v); }
    const legOnly = ![...Object.values(s.bins)].some(b => Object.keys(b).some(k => !/-(IN|OUT)$/.test(k)));
    if (legOnly) for (const [t, bin] of Object.entries(s.bins)) { let v = 0; for (const [k, e] of Object.entries(bin)) if (/-IN$/.test(k)) for (const c of VEH) v += e[c] || 0; tot.set(+t, v); }
    s.peak = {};
    for (const [p, w] of Object.entries(windows)) {
      const a = toMin(w[0]), b = toMin(w[1]);
      if (a == null || b == null) continue;
      let best = null;
      const step = s.interval >= 60 ? 60 : s.interval >= 30 ? 30 : 15, fine = step === 15;
      for (const t of [...tot.keys()].sort((x, y) => x - y)) {
        if (t < a || t + 60 > b) continue;
        const ts = []; for (let x = t; x < t + 60; x += step) ts.push(x);
        if (!ts.every(x => tot.has(x))) continue;
        const v = ts.reduce((q, x) => q + tot.get(x), 0);
        if (!best || v > best.v) best = { start: t, v, ts, v15: fine ? Math.max(...ts.map(x => tot.get(x))) : null };
      }
      if (!best) continue;
      const mv = {}, hv = {};
      for (const t of best.ts) for (const [k, e] of Object.entries(s.bins[t] || {})) { mv[k] = (mv[k] || 0) + VEH.reduce((q, c) => q + (e[c] || 0), 0); hv[k] = (hv[k] || 0) + (e.HV || 0) + (e.BUS || 0); }
      s.peak[p] = { start: hhmm(best.start), end: hhmm(best.start + 60), total: best.v, phf: best.v15 ? U.r3(best.v / (4 * best.v15)) : null, mv, hv, legOnly };
    }
    return s.peak;
  }
  // The junction a site sits on: the nearest node with 3 or more roads within 40 m
  function siteNode(sc, s) {
    const deg = Net.degree(sc);
    return Net.nearestNode(sc, [s.lat, s.lng], 40, n => (deg.get(n.id) || 0) >= 3);
  }
  const siteOfNode = (sc, id) => typeof Sites !== 'undefined' && sc && id ? Sites.of(sc, id) : null;
  // One node of a junction made of several: on a roundabout ring, or joined to another junction node by a short link
  function multiNode(sc, id) {
    const deg = Net.degree(sc), near = (typeof Sites !== 'undefined' && Sites.RULES && Sites.RULES.near) || 35;
    return Net.linksAt(sc, id).some(L => L.status !== 'closed' && (L.ring || ((deg.get(L.a === id ? L.b : L.a) || 0) >= 3 && Net.len(sc, L) <= near)));
  }
  /* Movements through junction sites traced along the assigned paths (docs/spec/junction-sites: a movement is the
     entry arc and the exit arc of a path that enters the site on one boundary arc and leaves on another). Done for
     every site in one pass over the paths and kept in sc.calib.siteTurns[period] = { result, basis, sites: { siteId:
     [[entry arc, exit arc, veh/h]] } }, because paths are not saved with the project. null when this result has no
     paths and nothing was kept for it. */
  const traceKey = res => `${res.period}|${res.ts}|${res.it}|${res.gap}`;
  function siteTraces(sc, res) {
    if (!sc || !res || !res.sites) return null;
    const key = traceKey(res), kept = sc.calib && sc.calib.siteTurns && sc.calib.siteTurns[res.period];
    if (kept && kept.result === key && kept.sites && typeof kept.sites === 'object') return kept.sites;
    if (!Array.isArray(res.paths) || !Array.isArray(res.arcKeys)) return null;
    const idx = new Map(res.arcKeys.map((k, i) => [k, i])), at = new Map();
    const mark = (k, h) => { const i = idx.get(k); if (i == null) return; const l = at.get(i); if (l) l.push(h); else at.set(i, [h]); };
    for (const st of Object.values(res.sites)) for (const l of st.legs || []) { for (const k of l.ins || []) mark(k, { s: st.id, entry: true, k }); for (const k of l.outs || []) mark(k, { s: st.id, entry: false, k }); }
    const pf = Engine.pcuF(Store.S), flow = {};
    for (const od of res.paths) for (const p of od.list || []) {
      const open = new Map();
      for (const a of p.arcs) {
        const hs = at.get(a); if (!hs) continue;
        for (const h of hs) {
          if (h.entry) { open.set(h.s, h.k); continue; }
          const e = open.get(h.s); if (e == null) continue;
          const m = flow[h.s] ||= new Map(), k = e + '\u0000' + h.k;
          m.set(k, (m.get(k) || 0) + p.w / pf); open.delete(h.s);
        }
      }
    }
    const sites = {};
    for (const [id, m] of Object.entries(flow)) sites[id] = [...m].map(([k, v]) => { const [a, b] = k.split('\u0000'); return [a, b, Math.round(v * 100) / 100]; });
    // kept only for the scenario's own current result (a trial run, such as the stress test, is not kept)
    if (sc.res && sc.res[res.period] === res) { (sc.calib ||= {}).siteTurns ||= {}; sc.calib.siteTurns[res.period] = { result: key, basis: 'paths', sites }; }
    return sites;
  }
  // Rows for one site from its traced movements, in the shape of Turns.siteRows rows (the leg codes come from r.A)
  function tracedRows(r, list) {
    const inLeg = new Map(), outLeg = new Map(), mvOf = new Map();
    for (const a of r.A) { for (const k of a.ins || []) inLeg.set(k, a); for (const k of a.outs || []) outLeg.set(k, a); }
    for (const x of r.rows) mvOf.set(x.approach + '\u0000' + x.to, x.mv);
    const out = [];
    for (const e of Array.isArray(list) ? list : []) {
      const [f, t, v] = e, a = inLeg.get(f), x = outLeg.get(t);
      if (!a || !x || !a.leg || !x.leg || !(+v > 0)) continue;
      const turn = Turns.turnCode(a.o, x.o), mv = a === x ? 'U' : mvOf.get(f + '\u0000' + t) || turn[0];
      out.push({ approach: a.leg, to: x.leg, leg: a.leg, toLeg: x.leg, from: f, toArc: t, mv, turn, v: +v, traced: true });
    }
    return out;
  }
  /* What a turning count is compared with. A node in a junction site (sc.sites: a roundabout ring, a divided-road
     crossing) is compared as the whole site: its legs (Turns.siteRows) and the movements through it traced along
     the paths (siteTraces). A node that is one of several making up a junction with no site defined is not scored:
     one ring node carries only part of each movement. Otherwise the node's own arms and movements.
     → { kind: 'node' | 'site' | 'cluster', id, node, members, arms: [{ inKey, outKey, b, road, links }], rows, note } */
  function junctionView(sc, s, res, nodeId = s.node) {
    const node = nodeId && sc.nodes[nodeId] ? nodeId : null;
    if (!node) return null;
    const site = siteOfNode(sc, node);
    if (site) {
      const rs = res && res.sites && res.sites[site.id], base = { kind: 'site', id: site.id, node, members: new Set(site.nodes), arms: [], rows: [] };
      if (!rs) return { ...base, note: res ? `Run the model again: the junction site ${site.name || ''} is newer than these results.` : 'Run the model first.' };
      const r = Turns.siteRows(sc, res, site.id), lid = k => { const L = sc.links[String(k).slice(0, String(k).lastIndexOf(':'))]; return L ? L.id : null; };
      // an engine that traces site movements along paths itself says so; otherwise trace them here
      const tr = res.siteTurnsBasis === 'paths' ? null : siteTraces(sc, res);
      if (res.siteTurnsBasis !== 'paths' && !tr) return { ...base, name: rs.name, note: (res.zoneIds || []).length > 300 ? 'Not scored: movements through a junction site are traced along the model\'s paths, and models with more than 300 zones do not keep them.' : 'Run the model again in this session to compare turns: movements through a junction site are traced along the model\'s paths, which are not saved with the project.' };
      const arms = r.A.filter(a => a.leg).map(a => ({ inKey: a.leg, outKey: a.leg, b: a.b, road: a.road, links: new Set([...a.outs, ...a.ins].map(lid).filter(Boolean)) }));
      const rows = tr ? tracedRows(r, tr[site.id]) : r.rows.map(m => ({ ...m, approach: m.leg, to: m.toLeg }));
      return { ...base, name: rs.name, arms, rows, traced: !!tr || res.siteTurnsBasis === 'paths' };   // traced here, or by the engine along its paths
    }
    if (multiNode(sc, node)) return { kind: 'cluster', id: node, node, members: new Set([node]), arms: [], rows: [], note: 'Multi-node junction (a roundabout ring or closely spaced nodes): define a junction site in the Junctions tab and run the model again to compare turns. Not scored.' };
    const list = (res && res.turns && res.turns[node]) || [];
    const arms = Turns.arms(sc, node, list).filter(a => !a.conn).map(a => ({ ...a, links: new Set([a.link.id]) }));
    return { kind: 'node', id: node, node, members: new Set([node]), arms, rows: res ? Turns.movements(sc, res, node) : [] };
  }
  // Count legs onto model arms: outward bearing within 30°, else the leg's road name
  function legArms(sc, nodeId, s, res, view = junctionView(sc, s, res, nodeId)) {
    const arms = view ? view.arms : [];
    const legs = [...new Set(Object.values(s.bins).flatMap(b => Object.keys(b).map(k => k.split('-')[0])))];
    const map = {}, used = new Set();
    const pairs = [];
    for (const l of legs) for (const a of arms) pairs.push({ l, a, e: U.angDiff(a.b, legBearing(l)) });
    pairs.sort((p, q) => p.e - q.e);
    for (const p of pairs) if (p.e <= 30 && !map[p.l] && !used.has(p.a.inKey)) { map[p.l] = p.a; used.add(p.a.inKey); }
    for (const l of legs) if (!map[l] && s.legRoads[l]) {
      const nm = DataHub.norm(s.legRoads[l]), a = arms.find(x => !used.has(x.inKey) && DataHub.norm(x.road) === nm);
      if (a) { map[l] = a; used.add(a.inKey); }
    }
    return { map, arms, view };
  }
  const CANON = { L: -90, T: 0, R: 90, U: 180 };
  // The arm a movement leaves by (left-hand traffic: U.turn > 0 is a right turn)
  function exitArm(arms, from, mv) {
    const inB = (from.b + 180) % 360, want = CANON[mv[0]];
    let best = null, be = Infinity;
    for (const x of arms) {
      if (x === from && mv[0] !== 'U') continue;
      const t = x === from ? 180 : U.turn(inB, x.b), e = Math.abs(((t - want + 540) % 360) - 180);
      if (Turns.classify(inB, x.b) === mv[0] || x === from) if (e < be) { be = e; best = x; }
    }
    return best;
  }
  /* Furness turn estimation from leg in / out totals (turning-counts): seed[i][j] = model turn shares where the model
     has traffic on that approach, else T 70 / L 15 / R 15 (0 for U). Rows and columns are scaled in turn until the
     largest relative error is under tol (0.1%) or 50 iterations. Out totals are scaled to the in total first. */
  function furness(ins, outs, seed, { tol = 0.001, maxIt = 50 } = {}) {
    const n = ins.length, M = seed.map(r => r.map(v => Math.max(0, +v || 0)));
    const ti = ins.reduce((a, b) => a + b, 0), to = outs.reduce((a, b) => a + b, 0);
    const O = to > 0 ? outs.map(v => v * ti / to) : outs.slice();
    let it = 0, err = Infinity;
    for (it = 1; it <= maxIt; it++) {
      for (let i = 0; i < n; i++) { const r = M[i].reduce((a, b) => a + b, 0); if (r > 0) for (let j = 0; j < n; j++) M[i][j] *= ins[i] / r; }
      for (let j = 0; j < n; j++) { let c = 0; for (let i = 0; i < n; i++) c += M[i][j]; if (c > 0) for (let i = 0; i < n; i++) M[i][j] *= O[j] / c; }
      err = 0;
      for (let i = 0; i < n; i++) { const r = M[i].reduce((a, b) => a + b, 0); if (ins[i] > 0) err = Math.max(err, Math.abs(r - ins[i]) / ins[i]); }
      for (let j = 0; j < n; j++) { let c = 0; for (let i = 0; i < n; i++) c += M[i][j]; if (O[j] > 0) err = Math.max(err, Math.abs(c - O[j]) / O[j]); }
      if (err < tol) break;
    }
    return { m: M, it: Math.min(it, maxIt), err, converged: err < tol, outsScaled: Math.abs(ti - to) > 1e-6, tag: 'estimated' };
  }
  // Turn volumes for a site and period: counted movements, or Furness estimates from leg totals
  function siteVolumes(sc, s, period, res) {
    const pk = s.peak && s.peak[period]; if (!pk) return null;
    if (!pk.legOnly) {
      const bal = cfg().useBalanced && s.bal && s.bal[period];
      return { mv: bal || pk.mv, estimated: false, balanced: !!bal };
    }
    const legs = [...new Set(Object.keys(pk.mv).map(k => k.split('-')[0]))];
    const ins = legs.map(l => pk.mv[l + '-IN'] || 0), outs = legs.map(l => pk.mv[l + '-OUT'] || 0);
    const view = s.node && sc.nodes[s.node] ? junctionView(sc, s, res) : null, la = view && view.arms.length ? legArms(sc, s.node, s, res, view) : null;
    const rows = view && res ? view.rows : [];
    const seed = legs.map((li, i) => legs.map((lj, j) => {
      const inH = (legBearing(li) + 180) % 360, mv = i === j ? 'U' : Turns.classify(inH, legBearing(lj));
      if (mv === 'U') return 0;
      const a = la && la.map[li], x = la && la.map[lj];
      if (a && x) { const tot = rows.filter(r => r.approach === a.inKey).reduce((q, r) => q + r.v, 0); if (tot > 0) return rows.filter(r => r.approach === a.inKey && r.to === x.outKey).reduce((q, r) => q + r.v, 0) / tot; }
      return mv === 'T' ? 0.7 : 0.15;
    }));
    const f = furness(ins, outs, seed), mv = {};
    legs.forEach((li, i) => legs.forEach((lj, j) => {
      if (!(f.m[i][j] > 0)) return;
      const m = i === j ? 'U' : Turns.classify((legBearing(li) + 180) % 360, legBearing(lj));
      mv[li + '-' + m] = (mv[li + '-' + m] || 0) + f.m[i][j];
    }));
    return { mv, estimated: true, furness: f };
  }
  // Turn validation rows for one site: counted against modelled, per movement
  function siteRows(sc, s, period, res = Store.res(sc, period)) {
    const node = s.node && sc.nodes[s.node] ? s.node : null;
    if (!node || !res) return { rows: [], node, note: !node ? 'Not matched to a junction within 40 m.' : 'Run the model first.' };
    const vol = siteVolumes(sc, s, period, res); if (!vol) return { rows: [], node, note: `No ${period} peak hour in the count (window ${cfg().windows[period].join('–')}).` };
    const view = junctionView(sc, s, res), scored = view && view.arms.length > 0;
    const { map } = scored ? legArms(sc, node, s, res, view) : { map: {} }, mrows = scored ? view.rows : [], out = [];
    for (const [k, v] of Object.entries(vol.mv)) {
      const [leg, mv] = k.split('-'); if (mv === 'IN' || mv === 'OUT') continue;
      const a = map[leg];
      const model = a ? mrows.filter(r => r.approach === a.inKey && (mv.length === 1 ? r.mv === mv : r.turn === mv)).reduce((q, r) => q + r.v, 0) : null;
      out.push({ s, leg, mv, key: k, obs: +v || 0, model, geh: model == null ? null : geh(model, +v || 0), arm: a ? a.road : null, estimated: vol.estimated, balanced: vol.balanced });
    }
    const flags = [], dos = Math.max(0, ...[...(view ? view.members : [node])].map(n => (res.nodes && res.nodes[n] && +res.nodes[n].dos) || 0));
    if (dos > CORRIDOR.dos.v) flags.push(`Model DoS ${num(dos)} > ${num(CORRIDOR.dos.v)}: a stop-line count understates demand here (${SOURCES[CORRIDOR.dos.src].short})`);
    if (scored) { const un = out.filter(r => r.model == null).map(r => r.leg); if (un.length) flags.push(`leg${new Set(un).size === 1 ? '' : 's'} ${[...new Set(un)].join(', ')} not matched to a road at the junction, so not scored`); }
    return { rows: out.sort((a, b) => LEGS.indexOf(a.leg) - LEGS.indexOf(b.leg) || 'LTRU'.indexOf(a.mv[0]) - 'LTRU'.indexOf(b.mv[0])), node, view, flags, vol, note: scored ? '' : (view && view.note) || 'No roads at this junction.' };
  }
  function matchSites(sc = Store.sc) { for (const s of store().turnSites) { s.node = siteNode(sc, s); const st = siteOfNode(sc, s.node); s.site = st ? st.id : null; } }
  function turnRows(sc, period, set, area = 'all') {
    const out = [], core = area === 'all' ? null : coreArea(sc);
    for (const s of store().turnSites) {
      if (s.role === 'off') continue;
      if (core && (area === 'inside') !== inCore(core, [s.lat, s.lng])) continue;
      if (set === 'valid' && s.role !== 'valid') continue;
      if (set === 'calib' && s.role === 'valid') continue;
      for (const r of siteRows(sc, s, period).rows) if (r.model != null) out.push(r);
    }
    return out;
  }
  /* Balance adjacent sites (turning-counts): the flow leaving site A towards site B (E) and arriving at B from A (A)
     move to F = (E + A) / 2, or max(E, A) with 'no sinks'. Movements are scaled proportionally. A gap above
     max(50 veh/h, 10%) is flagged as a midblock source or sink. */
  function balance(sc = Store.sc, period = Store.S.period, opts = {}) {
    const noSinks = opts.noSinks ?? cfg().noSinks, res = Store.res(sc, period), deg = Net.degree(sc);
    matchSites(sc);
    const all = store().turnSites.filter(s => s.node && s.peak && s.peak[period] && !s.peak[period].legOnly);
    const info = new Map(all.map(s => [s, legArms(sc, s.node, s, res)]));
    // only sites compared with a node or a junction site with results (a multi-node junction without a site is left out)
    const sites = all.filter(s => info.get(s).arms.length);
    const byNode = new Map(), audit = [];
    for (const s of sites) for (const n of info.get(s).view.members) byNode.set(n, s);
    const work = new Map(sites.map(s => [s, { ...s.peak[period].mv }]));
    const exitsBy = s => { const { map, arms } = info.get(s), o = new Map(); for (const k of Object.keys(work.get(s))) { const [leg, mv] = k.split('-'), a = map[leg]; if (!a) continue; const x = exitArm(arms, a, mv); if (x) o.set(k, x); } return o; };
    for (const s of sites) {
      const { arms, view } = info.get(s);
      for (const x of arms) {
        // leave the junction along this arm's road (either carriageway) and walk through plain bends to the next junction
        let hit = null;
        for (const id of x.links) {
          let L = sc.links[id], node = L && (view.members.has(L.a) ? L.a : view.members.has(L.b) ? L.b : null), far = 0, guard = 0, end = null;
          if (!node) continue;
          while (L && guard++ < 50) {
            const nxt = L.a === node ? L.b : L.a; far += Net.len(sc, L);
            if (far > 2000) break;
            if (byNode.has(nxt)) { end = nxt; break; }
            if (view.members.has(nxt) || (deg.get(nxt) || 0) !== 2) break;
            const prev = L; L = Net.linksAt(sc, nxt).find(l => l !== prev); node = nxt;
          }
          if (end && byNode.get(end) !== s) { hit = { t: byNode.get(end), L }; break; }
        }
        if (!hit) continue;
        const t = hit.t, L = hit.L, ti = info.get(t), y = ti.arms.find(a => a.links.has(L.id)); if (!y) continue;
        const legT = Object.keys(ti.map).find(l => ti.map[l] === y); if (!legT) continue;
        const ex = exitsBy(s), keysE = [...ex].filter(([, a]) => a === x).map(([k]) => k), keysA = Object.keys(work.get(t)).filter(k => k.startsWith(legT + '-'));
        const E = keysE.reduce((q, k) => q + work.get(s)[k], 0), A = keysA.reduce((q, k) => q + work.get(t)[k], 0);
        if (!(E > 0) || !(A > 0)) continue;
        const F = noSinks ? Math.max(E, A) : (E + A) / 2, gap = Math.abs(E - A) > Math.max(CORRIDOR.balance.abs, CORRIDOR.balance.rel * Math.max(E, A));
        const why = `${s.name} → ${t.name}: leaving ${U.fmt.int(E)}, arriving ${U.fmt.int(A)}${gap ? '; midblock source or sink, consider a driveway zone' : ''}`;
        for (const [site, keys, tot] of [[s, keysE, E], [t, keysA, A]]) for (const k of keys) {
          const w = work.get(site), o = w[k], b = o * F / tot;
          w[k] = b;
          audit.push({ site: site.name, siteId: site.id, movement: k, observed: site.peak[period].mv[k], balanced: b, d: b - site.peak[period].mv[k], pct: site.peak[period].mv[k] > 0 ? b / site.peak[period].mv[k] - 1 : null, reason: why, sink: gap });
        }
      }
    }
    for (const s of sites) { (s.bal ||= {})[period] = work.get(s); (s.audit ||= {})[period] = audit.filter(a => a.siteId === s.id); }
    return { audit, sites: sites.length };
  }
  // Whether a turning-count site is at this node, or at the junction site (id or any of its nodes) the node belongs to
  function atJunction(sc, s, id) {
    const n = s.node || siteNode(sc, s); if (!n) return false;
    if (n === id) return true;
    const st = siteOfNode(sc, n);
    return !!st && (st.id === id || st.nodes.includes(id));
  }
  // PHF of the turning count at a junction (node or junction-site id), for the SIDRA export (junctions.js reads it)
  function phf(id, period, sc = Store.sc) { const s = store().turnSites.find(x => x.role !== 'off' && x.peak && x.peak[period] && atJunction(sc, x, id)); return s ? s.peak[period].phf : null; }

  /* ================= criteria ================= */
  const t0 = o => ({ src: 'tmdg', use: 'expected', verified: true, ...o });
  const bandCrit = B => `within ${B.absLo} veh/h (< ${U.fmt.int(B.lo)}), ${U.fmt.num(B.rel * 100, B.rel * 100 % 1 ? 1 : 0)}% (${U.fmt.int(B.lo)}–${U.fmt.int(B.hi)}), ${B.absHi} veh/h (> ${U.fmt.int(B.hi)})`;
  // TAG M3.1 tests, built from TAG and BANDS.tag; Table 2 on links ('link') or turning movements ('turn')
  const tagT = o => t0({ src: TAG.src, verified: TAG.verified, use: 'check', ...o });
  const TAGT = {
    band: on => tagT({ id: on === 'turn' ? 'tTagBand' : 'tagBand', group: 'tag', on, stat: 'tag', kind: 'none', label: on === 'turn' ? 'Turn TAG flow band' : 'TAG flow band', crit: bandCrit(BANDS.tag), table: BANDS.tag.table, use: 'useful' }),
    either: on => tagT({ id: on === 'turn' ? 'tTagEither' : 'tagEither', group: 'tag', on, stat: 'either', kind: 'share', min: TAG.either.v, label: on === 'turn' ? 'Turn TAG either (GEH < 5 or flow band)' : 'TAG either (GEH < 5 or flow band)', crit: `${TAG.either.crit} of ${on === 'turn' ? 'turning movements' : 'counts'}`, table: TAG.either.table }),
    screen: () => tagT({ id: 'tagSl', group: 'tag', on: 'screen', stat: 'sw5', kind: 'share', min: TAG.screen.v, label: 'Screenline within 5% (TAG)', crit: TAG.screen.crit, table: TAG.screen.table }),
    jt: () => tagT({ id: 'tagJt', group: 'tag', on: 'jt', stat: 'jt1', kind: 'share', min: TAG.jt.v, label: 'Journey times (TAG)', crit: TAG.jt.crit, table: TAG.jt.table }),
  };
  function tmdgTests(cat) {
    const i = CATS.indexOf(cat), useOf = g => USE[FIG5[g][i]], fig5ok = g => (FIG5_OK[cat] || []).includes(g);
    const v = (key) => TMDG[key].v[cat], vf = key => !TMDG[key].vf.includes(cat), tb = key => TMDG[key].table;
    const share = (id, group, on, key, label, crit, extra = {}) => t0({ id, group, on, stat: extra.stat || key, kind: v(key) == null ? 'none' : 'share', min: v(key), label, crit, table: tb(key), verified: vf(key), use: useOf(group), useVerified: fig5ok(group), ...extra });
    const sl = useOf('screen');
    const out = [
      share('geh5', 'link', 'link', 'geh5', 'Link GEH < 5', 'GEH < 5, % of directional counts'),
      share('geh75', 'link', 'link', 'geh75', 'Link GEH < 7.5', 'GEH < 7.5'),
      share('geh10', 'link', 'link', 'geh10', 'Link GEH < 10', 'GEH < 10'),
      ...(cat === 'A' ? [share('geh12', 'link', 'link', 'geh12', 'Link GEH < 12', 'GEH < 12')] : []),
      share('band', 'band', 'link', 'band', 'TMDG count band', bandCrit(BANDS.tmdg)),
      TAGT.band('link'),
      TAGT.either('link'),
      share('tgeh5', 'turn', 'turn', 'geh5', 'Turn GEH < 5', 'GEH < 5, % of turning movements'),
      share('tgeh75', 'turn', 'turn', 'geh75', 'Turn GEH < 7.5', 'GEH < 7.5'),
      share('tgeh10', 'turn', 'turn', 'geh10', 'Turn GEH < 10', 'GEH < 10'),
      share('tband', 'band', 'turn', 'band', 'Turn TMDG count band', 'turning movements ' + bandCrit(BANDS.tmdg)),
      TAGT.band('turn'),
      TAGT.either('turn'),
      { ...share('sgeh5', 'screen', 'screen', 'sgeh5', 'Screenline GEH < 5', 'GEH < 5 on directional totals'), use: sl },
      { ...share('sgeh75', 'screen', 'screen', 'sgeh75', 'Screenline GEH < 7.5', 'GEH < 7.5 on directional totals'), use: sl },
      { ...share('sgeh10', 'screen', 'screen', 'sgeh10', 'Screenline GEH < 10', 'GEH < 10 on directional totals'), use: sl },
    ];
    if (cat === 'A') out.push(share('sw10', 'screen', 'screen', 'sw10', 'Screenline within 10%', 'directional totals within 10%'), share('sw15', 'screen', 'screen', 'sw15', 'Screenline within 15%', 'directional totals within 15%'));
    out.push(
      TAGT.screen(),
      t0({ id: 'rmsSl', group: 'tag', on: 'screen', stat: 'sgeh4', kind: 'share', min: RMS.sgeh4, label: 'Screenline GEH < 4 (RMS)', crit: 'every directional total', src: RMS.src, table: 'screenline criteria', verified: RMS.verified, use: 'useful' }),
      t0({ id: 'r2Ols', group: 'scatter', on: 'link', stat: 'r2Ols', kind: 'none', label: 'R² (OLS)', crit: 'ordinary least squares, for information', table: 'reported beside the TMDG test', use: 'useful' }),
      t0({ id: 'r2', group: 'scatter', on: 'link', stat: 'r2', kind: 'min', min: v('r2'), label: 'R² through the origin', crit: 'centred on the mean (the TMDG test)', table: tb('r2'), verified: vf('r2'), use: useOf('scatter'), useVerified: fig5ok('scatter') }),
      t0({ id: 'r2Excel', group: 'scatter', on: 'link', stat: 'r2Excel', kind: 'none', label: 'R² through the origin (Excel)', crit: 'what Excel shows for a trendline through zero', table: 'for information', use: 'useful' }),
      t0({ id: 'slope', group: 'scatter', on: 'link', stat: 'b0', kind: 'range', lo: v('slope')[0], hi: v('slope')[1], label: 'Slope b0', crit: 'trendline through the origin', table: tb('slope'), verified: vf('slope'), use: useOf('scatter'), useVerified: fig5ok('scatter') }),
      t0({ id: 'rmse', group: 'rmse', on: 'link', stat: 'rmse', kind: v('rmse') ? 'rmse' : 'none', max: v('rmse') && v('rmse')[0], clarify: v('rmse') && v('rmse')[1], label: '%RMSE', crit: '100·√(Σ(M−C)²/(N−1)) / (ΣC/N)', table: tb('rmse'), verified: vf('rmse'), use: useOf('rmse'), useVerified: fig5ok('rmse') }),
      t0({ id: 'jt1', group: 'jt', on: 'jt', stat: 'jt1', kind: 'share', min: v('jt1'), label: 'Journey times, tier 1', crit: 'within 15% or 1 minute, % of routes', table: tb('jt1'), use: useOf('jt') }),
      t0({ id: 'jt2', group: 'jt', on: 'jt', stat: 'jt2', kind: 'share', min: v('jt2'), label: 'Journey times, tier 2', crit: 'within 25% or 1.5 minutes, % of routes', table: tb('jt2'), use: useOf('jt') }),
    );
    return out.concat(commonTests(cat));
  }
  // Convergence and matrix-estimation rows (shown with every standard)
  function commonTests(cat = 'E') {
    const me = TMDG.meTotal.v[cat], cr = TMDG.meCR.v[cat], hi = cfg().priorQuality !== 'low';
    return [
      tagT({ id: 'gap', group: 'conv', on: 'conv', stat: 'gapPct', kind: 'max', max: TAG.gap.v, label: '%GAP', crit: TAG.gap.crit, table: TAG.gap.table }),
      tagT({ id: 'p4', group: 'conv', on: 'conv', stat: 'p4', kind: 'bool', label: `P × ${TAG.p.consec}`, crit: TAG.p.crit, table: TAG.p.table }),
      t0({ id: 'meTotal', group: 'me', on: 'me', stat: 'totalAbs', kind: me ? 'max' : 'none', max: me && me[hi ? 0 : 1], label: 'Matrix total change', crit: `${hi ? 'high' : 'low'}-quality prior, %`, table: TMDG.meTotal.table, use: 'check' }),
      tagT({ id: 'meCells', group: 'me', on: 'me', stat: 'cells', kind: 'pair', label: 'Matrix cells, slope and R²', crit: TAG.cells.crit, table: TAG.cells.table }),
      tagT({ id: 'meEnds', group: 'me', on: 'me', stat: 'ends', kind: 'pair', label: 'Trip ends, slope and R²', crit: TAG.ends.crit, table: TAG.ends.table }),
      tagT({ id: 'meTld', group: 'me', on: 'me', stat: 'tld', kind: 'pair', label: 'Trip-length mean and SD', crit: TAG.tld.crit, table: TAG.tld.table }),
      t0({ id: 'meCR', group: 'me', on: 'me', stat: 'cr', kind: cr ? 'min' : 'none', min: cr, label: 'Trip-length coincidence ratio', crit: 'Σmin / Σmax over distance bands', table: TMDG.meCR.table, use: 'check' }),
    ];
  }
  function presetTests(std) {
    if (std === 'rms') {
      const r = o => t0({ src: RMS.src, table: RMS.table, verified: RMS.verified, ...o });
      return [
        r({ id: 'geh5', group: 'link', on: 'link', stat: 'geh5le', kind: 'share', min: RMS.geh5, label: 'Link GEH ≤ 5', crit: '95% of links' }),
        r({ id: 'geh10', group: 'link', on: 'link', stat: 'geh10le', kind: 'share', min: RMS.geh10, label: 'Link GEH ≤ 10', crit: 'all links' }),
        r({ id: 'tgeh5', group: 'turn', on: 'turn', stat: 'geh5le', kind: 'share', min: RMS.tgeh5, label: 'Turn GEH ≤ 5', crit: '85% of turns' }),
        r({ id: 'tgeh10', group: 'turn', on: 'turn', stat: 'geh10le', kind: 'share', min: RMS.tgeh10, label: 'Turn GEH ≤ 10', crit: 'all turns' }),
        r({ id: 'rmsSl', group: 'screen', on: 'screen', stat: 'sgeh4', kind: 'share', min: RMS.sgeh4, label: 'Screenline GEH < 4', crit: 'every directional total' }),
        r({ id: 'r2', group: 'scatter', on: 'link', stat: 'r2', kind: 'min', min: RMS.r2, label: 'R² through the origin', crit: `more than ${RMS.r2}` }),
        r({ id: 'rmse', group: 'rmse', on: 'link', stat: 'rmse', kind: 'max', max: RMS.rmse, label: '%RMSE', crit: `${RMS.rmse}% or less` }),
        r({ id: 'jt1', group: 'jt', on: 'jt', stat: 'jt1', kind: 'share', min: RMS.jt, label: 'Journey times', crit: '95% of routes within 15% or 1 minute' }),
        TAGT.either('link'),
      ].concat(commonTests());
    }
    const a = o => t0({ src: ATAP.src, table: ATAP.table, verified: ATAP.verified, ...o });
    return [
      a({ id: 'within20', group: 'link', on: 'link', stat: 'within20', kind: 'share', min: ATAP.within20, label: 'Hourly links within ±20%', crit: '85% of links (share is a Spare Construct assumption)' }),
      a({ id: 'sw10', group: 'screen', on: 'screen', stat: 'sw10', kind: 'share', min: ATAP.sw10, label: 'Screenlines within ±10%', crit: 'every directional total' }),
      a({ id: 'r2', group: 'scatter', on: 'link', stat: 'r2', kind: 'min', min: ATAP.r2, label: 'R² through the origin', crit: `more than ${ATAP.r2}` }),
      a({ id: 'slope', group: 'scatter', on: 'link', stat: 'b0', kind: 'range', lo: ATAP.slope[0], hi: ATAP.slope[1], label: 'Slope b0', crit: `${ATAP.slope[0]} to ${ATAP.slope[1]}` }),
      a({ id: 'rmse', group: 'rmse', on: 'link', stat: 'rmse', kind: 'max', max: ATAP.rmse, label: '%RMSE', crit: `under ${ATAP.rmse}%` }),
      TAGT.either('link'),
    ].concat(commonTests());
  }
  /* The TAG M3.1 test beside each row (criteria-and-stats: TAG M3.1 side by side): Table 2 for link and turn rows,
     Table 1 for screenline rows, Table 3 for journey times. A TAG row is its own; null where TAG sets no criterion
     (scatter, %RMSE, the TMDG matrix-change rows). */
  function tagPeer(t) {
    if (t.src === 'tag') return t;
    if ((t.on === 'link' || t.on === 'turn') && ['link', 'turn', 'band'].includes(t.group)) return TAGT.either(t.on);
    if (t.on === 'screen') return TAGT.screen();
    if (t.on === 'jt') return TAGT.jt();
    return null;
  }
  function tests(std = cfg().standard, cat = category()) { return std === 'tmdg' || !std ? tmdgTests(cat) : presetTests(std); }
  const targetText = t => t.kind === 'share' ? (t.min >= 1 ? '100%' : `> ${U.fmt.num(t.min * 100, t.min * 100 % 1 ? 1 : 0)}%`) : t.kind === 'min' ? `> ${t.min}` : t.kind === 'range' ? `${t.lo} to ${t.hi}` : t.kind === 'rmse' ? `< ${t.max}% (${t.max}–${t.clarify}% explain)`
    : t.kind === 'max' ? `< ${t.max}${t.id === 'gap' ? '%' : t.group === 'me' ? '%' : ''}` : t.kind === 'bool' ? 'yes' : t.kind === 'pair' ? 'see criterion' : '–';
  function tooltip(t) {
    const s = SOURCES[t.src] || SOURCES.tmdg;
    return `${s.name} (${s.edition}), ${t.table}. ${t.verified ? 'Value checked against the source.' : 'Value not yet checked against the source document: verify before relying on it.'}${t.useVerified === false && t.src === 'tmdg' && t.use !== 'not-expected' ? ' Applicability for this category is a Spare Construct reading of TMDG Figure 5, to verify.' : ''}${t.src === 'corridor' ? ' Spare Construct assumption.' : ''}${s.url ? ' ' + s.url : ''}`;
  }

  /* ================= evidence and assessment ================= */
  function convStats(res) {
    if (!res) return { n: 0 };
    const c = res.conv || [], last = c.slice(-TAG.p.consec);
    return { n: 1, gapPct: res.gap * 100, gap: res.gap, it: res.it, converged: res.converged, gapTol: res.gapTol, note: res.convNote || '', p4: last.length === TAG.p.consec && last.every(e => e.P != null && e.P > TAG.p.share), lastP: last.length ? last[last.length - 1].P : null };
  }
  function meStats(period) {
    const info = Store.P.me && Store.P.me.info && Store.P.me.info[period];
    if (!info) return { n: 0 };
    const m1 = info.m1 || null, mc = info.matrixChange || {};
    const cells = m1 ? m1.cells : { slope: mc.cellSlope, r2: mc.cellR2, intercept: null };
    const ends = m1 ? m1.ends.all : { slope: mc.rowSlope, r2: mc.tripEndR2 };
    return { n: 1, info, m1, totalAbs: Math.abs(m1 ? m1.totalPct : mc.totalPct), totalPct: m1 ? m1.totalPct : mc.totalPct,
      cells: { ok: cells.slope != null && cells.slope >= TAG.cells.slope[0] && cells.slope <= TAG.cells.slope[1] && cells.r2 > TAG.cells.r2 && !(m1 && m1.cells.interceptFlag), text: `slope ${num(cells.slope, 3)}, R² ${num(cells.r2, 3)}${m1 ? `, intercept ${num(m1.cells.intercept, 2)}` : ''}` },
      ends: { ok: ends.slope != null && ends.slope >= TAG.ends.slope[0] && ends.slope <= TAG.ends.slope[1] && ends.r2 > TAG.ends.r2, text: `slope ${num(ends.slope, 3)}, R² ${num(ends.r2, 3)}` },
      tld: m1 && m1.tld ? { ok: Math.abs(m1.tld.meanPct) <= TAG.tld.v && Math.abs(m1.tld.sdPct) <= TAG.tld.v, text: `mean ${U.fmt.signedPct(m1.tld.meanPct / 100, 1)}, SD ${U.fmt.signedPct(m1.tld.sdPct / 100, 1)}` } : null,
      cr: m1 && m1.tld ? m1.tld.cr : null };
  }
  function evidence(sc = Store.sc, period = Store.S.period, o = {}) {
    const c = cfg(), set = o.set || c.set, res = Store.res(sc, period), S = Store.S;
    // the area whose counts decide the status: inside the area of influence when there is one, else every count
    const core = coreArea(sc), gate = core.kind === 'none' ? 'all' : 'inside', req = o.area || c.area;
    const area = req === 'auto' || req === 'inside' || !req ? gate : req, infoOnly = area !== gate;
    const all = countRows(sc, period, res, !!o.raw);
    const pick = s => all.filter(r => inSet(r, s) && inArea(r, area));
    const validN = pick('valid').length;
    let used = set, fallback = false, rows = pick(set);
    if (set === 'valid' && validN < MIN_N && !o.strict) { fallback = true; used = 'calib'; rows = pick('calib'); }
    const tr = turnRows(sc, period, used, area);
    const jr = journeys(sc, period, res);
    return { sc, period, res, set: used, requested: set, area, core, infoOnly, fallback, validN, rows, all,
      link: statsOf(rows), measured: statsOf(rows.filter(r => r.method === 'measured_hour')), turn: statsOf(tr), turnRows: tr,
      screen: slObservations(sc, period, used), jt: jtStats(jr), jtRows: jr, conv: convStats(res), me: meStats(period),
      baseYear: !!res && (!(res.year > 0) || res.year === S.baseYear), year: res ? res.year : null };
  }
  const COUNT_GROUPS = new Set(['link', 'turn', 'band', 'tag', 'scatter', 'rmse', 'screen', 'jt']);
  function evaluate(t, ev) {
    const d = ev[t.on] || {}, n = d.n || 0, val = d[t.stat];
    const out = { t, n, value: val, got: null, need: null, ok: null, status: '', shown: '–' };
    if (t.use === 'not-expected') { out.status = 'N/A'; out.shown = val == null ? '–' : show(t, d, val); return out; }
    if (!n || (val == null && t.kind !== 'pair')) { out.status = 'No data'; return out; }
    out.shown = show(t, d, val);
    // Google typical times are predicted, not observed: reported, never deciding the status
    if (t.on === 'jt' && d.predicted) { out.shown += ' (Google, predicted)'; out.predicted = true; }
    if (t.kind === 'share') { out.got = d.cnt[t.stat]; out.need = need(t.min, n); out.ok = out.got >= out.need; }
    else if (t.kind === 'min') out.ok = val > t.min;
    else if (t.kind === 'range') out.ok = val >= t.lo && val <= t.hi;
    else if (t.kind === 'max') out.ok = val < t.max;
    else if (t.kind === 'rmse') { out.ok = val < t.max; out.clarify = !out.ok && val <= t.clarify; }
    else if (t.kind === 'bool') out.ok = !!val;
    else if (t.kind === 'pair') { if (!val) { out.status = 'No data'; return out; } out.ok = val.ok; }
    if (t.kind === 'none') { out.status = 'Info'; return out; }
    const verdict = out.ok ? 'Pass' : out.clarify ? 'Requires clarification' : t.kind === 'share' ? `Fail (${out.got} of ${n}; ${out.need} needed)` : 'Fail';
    out.verdict = verdict;
    if (out.predicted) { out.status = 'Info'; return out; }
    if (COUNT_GROUPS.has(t.group) && n < MIN_N) { out.status = 'Indicative'; out.indicative = true; return out; }
    // potentially useful tests, and every count test outside the area that decides the status, are reported only
    out.status = t.use === 'useful' ? (out.clarify ? 'Info (requires clarification)' : 'Info') : ev.infoOnly && COUNT_GROUPS.has(t.group) ? 'Info' : verdict;
    return out;
  }
  function show(t, d, v) {
    if (t.kind === 'share') return `${d.cnt[t.stat]}/${d.n} · ${pct(v)}`;
    if (t.kind === 'pair') return v ? v.text : '–';
    if (t.kind === 'bool') return v ? 'yes' : 'no';
    if (t.stat === 'rmse' || t.stat === 'gapPct' || t.stat === 'totalAbs') return t.stat === 'gapPct' ? `${num(v, 3)}%` : `${num(v, 1)}%`;
    if (t.kind === 'none' && d.cnt && d.cnt[t.stat] != null) return `${d.cnt[t.stat]}/${d.n} · ${pct(v)}`;
    return num(v, 3);
  }
  /* The tests that decide the overall status: expected ones on counts with data (turn and screenline rows only count
     when present). Indicative rows (n < 10) and predicted journey times never decide it. */
  const expected = r => r.t.use === 'expected' && r.t.kind !== 'none' && r.status !== 'No data' && r.status !== 'N/A' && COUNT_GROUPS.has(r.t.group) && !r.predicted;
  const gating = r => expected(r) && !r.indicative;
  function assess(sc = Store.sc, period = Store.S.period, o = {}) {
    const ev = o.ev || evidence(sc, period, o), std = o.standard || cfg().standard, cat = o.category || category();
    const rows = tests(std, cat).map(t => evaluate(t, ev));
    const g = rows.filter(gating);
    let status;
    if (!ev.res) status = 'none';
    else if (!ev.link.n) status = 'none';
    else if (ev.infoOnly) status = 'info';
    else if (rows.some(r => expected(r) && r.indicative && r.t.group === 'link')) status = 'indicative';
    else if (!ev.baseYear) status = 'not base year';
    else status = g.every(r => r.ok) ? 'pass' : 'fail';
    const out = { period, standard: std, category: cat, ev, rows, status, pass: status === 'pass' ? true : status === 'fail' ? false : null, also: [], fails: [] };
    if (std === 'tmdg' && (status === 'pass' || status === 'fail')) for (const c2 of CATS) {
      if (c2 === cat) continue;
      const r2 = tmdgTests(c2).map(t => evaluate(t, ev)).filter(gating);
      const bad = r2.find(r => !r.ok);
      if (!bad) out.also.push(c2); else out.fails.push({ cat: c2, test: bad.t.label, got: bad.shown.replace(/^.*· /, ''), target: targetText(bad.t).replace(/^> /, '') });
    }
    out.line = statusLine(out);
    return out;
  }
  function statusLine(a) {
    const ev = a.ev, lab = a.standard === 'tmdg' ? `TMDG Category ${a.category}` : STANDARDS[a.standard];
    const where = ev.area === 'inside' ? ' inside the area of influence' : ev.area === 'outside' ? ' outside the area of influence' : '';
    const who = `${SET_LABEL[ev.set]}${where}, n=${ev.link.n}`;
    if (a.status === 'none') return !ev.res ? `Run the model for the ${a.period} peak to compare it with the counts.` : `No ${SET_LABEL[ev.set]}${where} match the network for the ${a.period} peak.`;
    if (a.status === 'info') return `For information only (${who}): pass or fail is decided on the counts inside the area of influence, ${ev.core.label}. Choose Area: Area of influence to see the status.`;
    if (a.status === 'indicative') return `Indicative only: ${who} (fewer than ${MIN_N}), so no pass or fail is given.`;
    if (a.status === 'not base year') return `Not assessed: the ${ev.year} results are compared with base-year (${Store.S.baseYear}) counts. Validate a base-year scenario; these figures are for information.`;
    const failed = a.rows.filter(r => gating(r) && !r.ok).map(r => r.t.label);
    let s = `Assessed against ${lab}: ${a.pass ? 'all expected tests pass' : 'fails ' + failed.join(', ')} (${who}).`;
    if (a.also.length) s += ` Also meets Category ${a.also.join(', ')}.`;
    if (a.fails.length && a.standard === 'tmdg') s += ` Fails: ${a.fails.slice(0, 3).map(f => `${f.cat} on ${f.test} (${f.got} v ${f.target})`).join('; ')}.`;
    const ind = [...new Set(a.rows.filter(r => expected(r) && r.indicative && r.t.group !== 'link').map(r => ({ turn: 'turn', screen: 'screenline', jt: 'journey-time' }[r.t.group] || r.t.group)))];
    if (ind.length) s += ` The ${ind.join(' and ')} tests have fewer than ${MIN_N} observations, so they are indicative and do not decide the status.`;
    const dr = (ev.screen && ev.screen.dropped) || [];
    if (dr.length) s += ` ${dr.length} validation screenline total${dr.length === 1 ? ' is' : 's are'} left out because ${dr.length === 1 ? 'it adds' : 'they add'} in calibration or gateway counts.`;
    return s + ' Meeting criteria does not by itself show the model is fit for purpose (TAG M3.1 3.2.2).';
  }
  const STATUS_COL = s => /^Pass/.test(s) ? 'var(--ok)' : /^Fail/.test(s) ? 'var(--danger)' : /clarification|Indicative/.test(s) ? 'var(--warn)' : 'var(--muted)';
  /* Record the base-year validation result for the model-status badge (Engine.modelStatus) and the checklist: TMDG,
     the current category, independent validation counts inside the area of influence. The record is removed when
     the base-year result is out of date or no longer gives a pass or fail (too few counts, roles changed), so the
     badge never keeps an old 'Validated'. A run of another (future-year) scenario leaves it alone. */
  function recordValidation(sc, period) {
    const P = Store.P; if (!P || !sc) return null;
    const old = P.validation && P.validation[period], res = Store.res(sc, period);
    const drop = () => { if (P.validation) { delete P.validation[period]; if (!Object.keys(P.validation).length) delete P.validation; } return null; };
    const base = !!res && (!(res.year > 0) || res.year === Store.S.baseYear);
    if (!res || res.stale || !base) return old && old.scenario === sc.id ? drop() : null;
    const a = assess(sc, period, { set: 'valid', strict: true, area: 'auto', standard: cfg().standard || 'tmdg' });   // the project's criteria, recorded as assessed
    if (a.status !== 'pass' && a.status !== 'fail') return drop();
    const v = P.validation ||= {};
    v[period] = { pass: a.pass, independent: a.ev.link.n >= MIN_N && !a.ev.fallback, category: a.category, standard: a.standard || cfg().standard || 'tmdg', area: a.ev.area, n: a.ev.link.n, scenario: sc.id, date: new Date().toISOString() };
    return v[period];
  }
  // Re-record both periods for the scenario the validation chapter reports (after a change of category, roles or estimation)
  function refreshValidation(P = Store.P) {
    if (!P || !P.scenarios) return false;
    const before = JSON.stringify(P.validation || null), sc = reportScenario(P);
    for (const p of PERIODS) {
      const v = P.validation && P.validation[p];
      // only the reported scenario's record stands: one for an option run in the base year, or for a scenario that
      // has gone, is not the base model's validation
      if (v && (!sc || v.scenario !== sc.id)) { delete P.validation[p]; if (!Object.keys(P.validation).length) delete P.validation; }
      if (sc) recordValidation(sc, p);
    }
    const strip = s => s.replace(/"date":"[^"]*"/g, '');
    return strip(before) !== strip(JSON.stringify(P.validation || null));
  }

  /* ================= matrix estimation (UI side of Calibrate.fit) ================= */
  // How future-year scenarios use the estimation (me.info[period].future, set by Calibrate.fit)
  function futureText(info) {
    return info && info.future === 'delta'
      ? 'Future-year scenarios carry the estimation forward as an additive change: the veh/h the fit added to or took from each existing-zone and gateway OD pair (T = max(0, T + Δ)), so background growth is not scaled by the base-year factors and new development keeps its own trip rates.'
      : 'Future-year scenarios carry the estimation forward as factors on the same existing-zone and gateway OD pairs, so new development keeps its own trip rates.';
  }
  const withoutME = (period, fn) => { const me = Store.P.me, f = me && me[period]; if (f) delete me[period]; try { return fn(); } finally { if (f) me[period] = f; } };
  async function runME(sc = Store.sc, opts = {}) {
    const period = opts.period || Store.S.period, xamax = +opts.xamax || cfg().xamax, prog = opts.onProgress || (() => {});
    const ids = D().counts.filter(c => DataHub.inUse(c) && !c.virtual && roleOf(c, period, true) === 'me').map(c => c.id);
    if (!ids.length) return { ok: false, note: 'No calibration counts: every count is a gateway, validation or unused count. Give some counts the calibration role first.' };
    const { view } = Calibrate.baseView(sc);
    prog(0, 1, 'Running the base year without matrix estimation');
    const P = Store.P, me = P.me ||= {}, keep = me[period];
    if (keep) delete me[period];
    let res0;
    try { res0 = await Engine.assign(view, { period }); } finally { if (keep) me[period] = keep; }
    const vr = r => countRows(view, period, r, true).filter(x => x.scored && x.role === 'valid');
    const validBefore = statsOf(vr(res0)), calibBefore = statsOf(countRows(view, period, res0, true).filter(x => x.scored && x.role === 'me'));
    // only the calibration counts take part (countIds), weighted by observation method; calibration screenlines are
    // fitted as group totals, and through traffic (gateway to gateway) keeps the prior when freezeGG is on
    const idSet = new Set(ids), groups = [];
    for (const sl of store().screenlines.filter(x => x.type !== 'core' && x.role !== 'valid')) {
      let rows = []; try { rows = slRows(view, sl, period, res0); } catch { rows = []; }
      for (const r of rows) { const g = [...new Set((r.used || r.ids || []).filter(id => idSet.has(id)))]; if (r.dir && g.length >= 2) groups.push({ id: `${sl.id}:${r.dir}`, label: `${sl.name} ${r.label}`, countIds: g }); }
    }
    const weight = (c, p) => { const m = METHODS[methodOf(c, p)]; return m ? m.weight : 0.4; };
    const out = await Calibrate.fit(sc, { period, maxStep: xamax, maxFactor: xamax, countIds: ids, weights: true, weight, groups, freezeGG: cfg().freezeGG !== false, onProgress: (k, n, msg) => prog(k, n, msg) });
    const info = P.me && P.me.info && P.me.info[period];
    if (!out.res || !info) return { ok: false, out, note: out.note };
    info.countIds = ids; info.xamax = xamax; info.roleAware = true; info.weighted = true; info.groups = groups.length; info.freezeGG = cfg().freezeGG !== false;
    info.validBefore = validBefore; info.validAfter = statsOf(vr(out.res));
    info.calibBefore = calibBefore; info.calibAfter = statsOf(countRows(view, period, out.res, true).filter(x => x.scored && x.role === 'me'));
    info.m1 = meReport(sc, period);
    memo.clear();
    return { ok: true, out, info, note: out.note };
  }
  // 'Reduce ME': XAMAX 2 → 1.5 → 1.25 → 1.1 until the TAG Table 5 rows pass
  async function reduceME(sc = Store.sc, opts = {}) {
    const period = opts.period || Store.S.period, steps = [2, 1.5, 1.25, 1.1].filter(x => x <= (+opts.from || 2)), tried = [];
    let last = null;
    for (const x of steps) {
      last = await runME(sc, { ...opts, period, xamax: x });
      const m = last.ok ? meStats(period) : null, ok = !!(m && m.cells.ok && m.ends.ok && (!m.tld || m.tld.ok));
      tried.push({ xamax: x, ok, note: last.note });
      if (ok || !last.ok) break;
    }
    return { ...last, tried };
  }
  // k-means sectors (deterministic farthest-point start) for the sector matrix
  function kmeans(pts, k) {
    if (!pts.length) return [];
    const C = [pts[0]];
    while (C.length < Math.min(k, pts.length)) { let bi = 0, bd = -1; pts.forEach((p, i) => { const d = Math.min(...C.map(c => (p[0] - c[0]) ** 2 + (p[1] - c[1]) ** 2)); if (d > bd) { bd = d; bi = i; } }); C.push(pts[bi]); }
    let lab = pts.map(() => 0);
    for (let it = 0; it < 20; it++) {
      lab = pts.map(p => { let b = 0, bd = Infinity; C.forEach((c, j) => { const d = (p[0] - c[0]) ** 2 + (p[1] - c[1]) ** 2; if (d < bd) { bd = d; b = j; } }); return b; });
      C.forEach((c, j) => { const m = pts.filter((p, i) => lab[i] === j); if (m.length) C[j] = [m.reduce((s, p) => s + p[0], 0) / m.length, m.reduce((s, p) => s + p[1], 0) / m.length]; });
    }
    return lab;
  }
  /* Table M1 (TMDG section 8, TAG M3.1 Table 5): prior against estimated on the base-year view. Trip lengths use the
     free-flow shortest path; bins 0–2, 2–5, 5–10, 10–20, 20+ km. */
  function meReport(sc = Store.sc, period = Store.S.period) {
    const me = Store.P.me && Store.P.me[period]; if (!me) return null;
    const S = Store.S, { view } = Calibrate.baseView(sc), G = Engine.buildGraph(view), Z = G.zones.length;
    const prior = withoutME(period, () => Engine.demand(view, G, period, S).T), est = Float64Array.from(prior);
    Engine.applyME(G, period, est);
    const isG = G.zones.map(z => z.kind === 'gateway'), name = i => G.zones[i].name || G.zones[i].id;
    let tp = 0, te = 0;
    const cp = [], ce = [], top = [], chg = { over50: 0, b40: 0, b30: 0, n: 0 };
    const O = [new Float64Array(Z), new Float64Array(Z)], Dd = [new Float64Array(Z), new Float64Array(Z)];
    for (let i = 0; i < Z; i++) for (let j = 0; j < Z; j++) {
      const p = prior[i * Z + j], e = est[i * Z + j];
      tp += p; te += e; O[0][i] += p; O[1][i] += e; Dd[0][j] += p; Dd[1][j] += e;
      if (p > 0) {
        cp.push(p); ce.push(e); chg.n++;
        const r = Math.abs(e / p - 1); if (r > 0.5) chg.over50++; else if (r > 0.4) chg.b40++; else if (r > 0.3) chg.b30++;
        if (Math.abs(e - p) > 0.05) top.push({ from: name(i), to: name(j), prior: p, est: e, d: e - p, pct: e / p - 1 });
      }
    }
    const reg = DataHub.regression(cp, ce), meanCell = cp.length ? tp / cp.length : 0;
    const endsOf = keep => { const x = [], y = []; for (let i = 0; i < Z; i++) if (keep(i)) { x.push(O[0][i], Dd[0][i]); y.push(O[1][i], Dd[1][i]); } const r = DataHub.regression(x, y); return { slope: r.olsB, intercept: r.olsA, r2: r.r2Ols, n: x.length / 2 }; };
    const gb = [2.5, 5, 7.5, 10, Infinity], tg = gb.map(() => 0);
    for (let i = 0; i < Z; i++) for (const [a, b] of [[O[0][i], O[1][i]], [Dd[0][i], Dd[1][i]]]) if (a + b > 0) tg[gb.findIndex(x => geh(b, a) < x)]++;
    // trip lengths on the free-flow shortest paths
    const W = Engine.workspace(G), bins = [2000, 5000, 10000, 20000, Infinity], fp = bins.map(() => 0), fe = bins.map(() => 0);
    let sp = 0, se = 0, sp2 = 0, se2 = 0;
    for (let i = 0; i < Z; i++) {
      let any = false; for (let j = 0; j < Z; j++) if (prior[i * Z + j] > 0 || est[i * Z + j] > 0) { any = true; break; }
      if (!any) continue;
      Engine.spt(G, G.ffc, G.zoneNode[i], W);
      for (let j = 0; j < Z; j++) {
        const p = prior[i * Z + j], e = est[i * Z + j]; if (!(p > 0 || e > 0) || i === j || !isFinite(W.nd[G.zoneNode[j]])) continue;
        let len = 0; for (const a of Engine.pathTo(G, W, G.zoneNode[j])) len += G.arcs[a].len;
        const b = bins.findIndex(x => len < x); fp[b] += p; fe[b] += e;
        sp += p * len; se += e * len; sp2 += p * len * len; se2 += e * len * len;
      }
    }
    const tP = fp.reduce((a, b) => a + b, 0), tE = fe.reduce((a, b) => a + b, 0);
    const mP = tP ? sp / tP : 0, mE = tE ? se / tE : 0, sdP = tP ? Math.sqrt(Math.max(0, sp2 / tP - mP * mP)) : 0, sdE = tE ? Math.sqrt(Math.max(0, se2 / tE - mE * mE)) : 0;
    let mn = 0, mx = 0; bins.forEach((b, k) => { const a = tP ? fp[k] / tP : 0, c = tE ? fe[k] / tE : 0; mn += Math.min(a, c); mx += Math.max(a, c); });
    // sectors: the gateways together, plus up to 8 k-means sectors of the zones
    const inner = G.zones.map((z, i) => i).filter(i => !isG[i]), lab = kmeans(inner.map(i => [G.zones[i].lat, G.zones[i].lng * Math.cos(G.zones[i].lat * Math.PI / 180)]), 8);
    const sec = new Array(Z).fill(0); inner.forEach((i, k) => { sec[i] = lab[k] + 1; });
    const nS = Math.max(0, ...lab) + 2, smP = new Float64Array(nS * nS), smE = new Float64Array(nS * nS);
    for (let i = 0; i < Z; i++) for (let j = 0; j < Z; j++) { smP[sec[i] * nS + sec[j]] += prior[i * Z + j]; smE[sec[i] * nS + sec[j]] += est[i * Z + j]; }
    let sTest = 0, sOk = 0; for (let k = 0; k < nS * nS; k++) if (smP[k] >= CORRIDOR.meSectorPrior) { sTest++; if (Math.abs(smE[k] / smP[k] - 1) <= TAG.sectors.v) sOk++; }
    top.sort((a, b) => Math.abs(b.d) - Math.abs(a.d));
    return {
      date: new Date().toISOString(), period, priorTotal: tp, total: te, totalPct: tp > 0 ? (te - tp) / tp * 100 : 0,
      cells: { n: cp.length, slope: reg.olsB, intercept: reg.olsA, r2: reg.r2Ols, meanPrior: meanCell, interceptFlag: reg.olsA != null && Math.abs(reg.olsA) > CORRIDOR.meIntercept * meanCell },
      ends: { all: endsOf(() => true), noGateways: endsOf(i => !isG[i]) },
      tld: { bins: ['0–2 km', '2–5 km', '5–10 km', '10–20 km', '20+ km'], prior: fp.map(v => tP ? v / tP : 0), est: fe.map(v => tE ? v / tE : 0), meanP: mP, meanE: mE, sdP, sdE,
        meanPct: mP ? (mE / mP - 1) * 100 : 0, sdPct: sdP ? (sdE / sdP - 1) * 100 : 0, cr: mx ? mn / mx : null, basis: 'free-flow shortest path, base-year network' },
      tripEndGeh: { bands: ['< 2.5', '2.5–5', '5–7.5', '7.5–10', '> 10'], n: tg },
      cellChange: chg, sectors: { n: nS, tested: sTest, within5: sOk, note: 'gateways plus up to 8 k-means sectors; cells with a prior of 50 trips or more; report only under TMDG' },
      top: top.slice(0, 10),
    };
  }

  /* ================= calibrate in order: network checks, stress test, trip-end factors ================= */
  function networkChecks(sc = Store.sc, period = Store.S.period) {
    const rows = countRows(sc, period), res = Store.res(sc, period), out = { far: [], worst: [], mwZones: [], noDelay: [] };
    out.far = rows.filter(r => r.mt.d > 30).slice(0, 8);
    out.worst = rows.filter(r => r.scored).slice(0, 5);
    const deg = Net.degree(sc);
    for (const z of Object.values(sc.zones)) {
      if (z.kind === 'gateway') continue;
      const near = Net.nearestNode(sc, [z.lat, z.lng], 2000, n => (deg.get(n.id) || 0) > 0);
      const L = near && Net.linksAt(sc, near);
      if (L && L.length && L.every(isMw)) out.mwZones.push(z);
    }
    if (res) for (const [k, a] of Object.entries(res.arcs)) if (a.vc > 1 && !(a.nd > 0)) { const L = sc.links[k.split(':')[0]]; if (L) out.noDelay.push({ key: k, L, vc: a.vc }); }
    out.noDelay.sort((a, b) => b.vc - a.vc);
    return out;
  }
  let stressState = null;
  /* Stress test: assign at demand × 1.1 and × 1.2. A junction is flagged when its DoS rises by more than twice the
     demand increase; a road when V/C > 1 with no junction delay. */
  async function stressTest(sc = Store.sc, opts = {}) {
    const period = opts.period || Store.S.period, prog = opts.onProgress || (() => {}), factors = opts.factors || CORRIDOR.stress.factors;
    const base = Store.res(sc, period) && !Store.res(sc, period).stale ? Store.res(sc, period) : await Engine.assign(sc, { period });
    const runs = [];
    for (let k = 0; k < factors.length; k++) { prog(k, factors.length, `Assigning demand × ${factors[k]}`); runs.push({ f: factors[k], res: await Engine.assign(sc, { period, demandFactor: factors[k] }) }); }
    const nodes = [];
    for (const [id, n0] of Object.entries(base.nodes || {})) {
      if (!(n0.dos > 0.05)) continue;
      const r = runs.map(x => { const n = x.res.nodes[id]; return { f: x.f, dos: n ? n.dos : null, rise: n ? n.dos / n0.dos - 1 : null }; });
      const sens = Math.max(...r.map(x => x.rise != null ? x.rise / (x.f - 1) : 0));
      nodes.push({ id, name: Kit.junctionName(id, sc), dos0: n0.dos, runs: r, sens, flag: r.some(x => x.rise != null && x.rise > CORRIDOR.stress.rise * (x.f - 1)) });
    }
    nodes.sort((a, b) => b.sens - a.sens);
    const links = [];
    for (const x of [{ f: 1, res: base }, ...runs]) for (const [k, a] of Object.entries(x.res.arcs)) if (a.vc > 1 && !(a.nd > 0)) { const L = sc.links[k.split(':')[0]]; if (L && !links.some(l => l.key === k)) links.push({ key: k, L, vc: a.vc, f: x.f }); }
    stressState = { scId: sc.id, period, at: Date.now(), nodes, links, top: nodes[0] || null, factors };
    return stressState;
  }
  /* Trip-end factors (calibrate-workflow): at most 7, one per land use with the most trips plus estimated gateways
     (gateways set from counts keep their counts). Minimise Σ w·((ΣM − ΣC)/ΣC)² over calibration screenlines, or over
     single calibration and gateway counts when there are none (weights by observation method); never validation
     counts. f ∈ [0.5, 2.0] (Spare Construct bounds). Linearised: one run per factor at 1.5, a bounded coordinate descent on
     that model, then a confirming run; the factors are only kept if the confirming run improves the objective. */
  // The fit always starts from the trip rates as they are: applied factors (Store.P.luFactors[period]) are set aside while it runs
  async function tripEndFactors(sc = Store.sc, opts = {}) {
    const P = Store.P, period = opts.period || Store.S.period, keep = P && P.luFactors && P.luFactors[period];
    if (keep) delete P.luFactors[period];
    try { return await fitTripEnds(sc, opts); } finally { if (keep) (P.luFactors ||= {})[period] = keep; }
  }
  async function fitTripEnds(sc = Store.sc, opts = {}) {
    const S0 = Store.S, period = opts.period || S0.period, prog = opts.onProgress || (() => {}), [LO, HI] = CORRIDOR.tefBounds;
    const { view } = Calibrate.baseView(sc, S0), G = Engine.buildGraph(view, S0);
    const tot = {};
    for (const z of G.zones) {
      if (z.kind === 'gateway') { if (!z.gwSrc) { const t = Engine.tripEnds(z, S0, period, view); tot.gateways = (tot.gateways || 0) + t.O + t.D; } continue; }
      for (const k of Object.keys(LANDUSES)) { const q = +((z.lu || {})[k]) || 0, r = ((S0.rates || {})[k] || {})[period]; if (q && r) tot[k] = (tot[k] || 0) + q / LANDUSES[k].per * r.rate; }
    }
    const lu = Object.keys(tot).filter(k => k !== 'gateways' && tot[k] > 1).sort((a, b) => tot[b] - tot[a]).slice(0, 6);
    const groups = tot.gateways > 1 ? [...lu, 'gateways'] : lu;
    if (!groups.length) return { ok: false, note: 'No base-year land use or estimated gateways to factor. Mark existing zones as existing land use first.' };
    // what to fit: calibration screenlines, else calibration and gateway counts one by one
    let obs = [], basis = 'screenlines';
    const base0 = await withoutMEAsync(period, () => Engine.assign(view, { period, settings: S0 }));
    // only counts with a calibration or gateway role: a validation count on a calibration screenline is left out
    for (const sl of store().screenlines.filter(s => s.type !== 'core' && s.role !== 'valid')) for (const r of slRows(view, sl, period, base0)) {
      if (!r.dir || !r.counted || r.roles.includes('valid')) continue;
      if (r.arcs.length && r.C > 0) obs.push({ arcs: r.arcs, C: r.C, w: 1, label: `${sl.name} ${r.label}`, ids: r.used });
    }
    if (!obs.length) {
      /* No calibration screenlines: count groups (calibrate-workflow). Single counts would let a 20 veh/h count with
         a large relative error decide every factor, so counts are grouped by road class and compass sector round
         the counts' centre (Spare Construct convention), and each group's totals are fitted. Each count adds its
         observation-method weight to the group weight; groups under 100 veh/h are left out. */
      basis = 'count groups';
      const cand = countRows(view, period, base0, true).filter(r => r.scored && r.role !== 'valid' && r.obs > 0 && METHODS[r.method].weight > 0);
      const cLat = cand.reduce((s, r) => s + r.c.lat, 0) / Math.max(1, cand.length), cLng = cand.reduce((s, r) => s + r.c.lng, 0) / Math.max(1, cand.length);
      const cg = L => !L ? 'local' : isMw(L) ? 'motorway' : L.cls === 'arterial' || L.cls === 'collector' ? L.cls : 'local';
      const grp = new Map();
      for (const r of cand) {
        const sec = dirOf(U.bearing([cLat, cLng], [r.c.lat, r.c.lng])).replace('B', ''), key = `${cg(r.L)}|${sec}`;
        const g = grp.get(key) || { arcs: [], C: 0, wsum: 0, n: 0, ids: [], label: `${(CLASSES[cg(r.L)] || CLASSES.local).label} roads, ${{ N: 'north', E: 'east', S: 'south', W: 'west' }[sec]}` };
        g.arcs.push(...(r.mt.dir === 'both' ? [r.mt.link + ':ab', r.mt.link + ':ba'] : [r.mt.link + ':' + r.mt.dir]));
        g.C += r.obs; g.wsum += METHODS[r.method].weight; g.n++; g.ids.push(r.c.id);
        grp.set(key, g);
      }
      for (const g of grp.values()) if (g.C >= 100) obs.push({ arcs: g.arcs, C: g.C, w: g.wsum / g.n, label: `${g.label} (${g.n} count${g.n === 1 ? '' : 's'})`, ids: g.ids });
    }
    obs = obs.filter(o => o.w > 0);
    if (!obs.length) return { ok: false, note: 'No calibration counts or calibration screenlines to fit. Give some counts the calibration role first.' };
    const vol = res => obs.map(o => o.arcs.reduce((s, k) => s + (res.arcs[k] ? res.arcs[k].v : 0), 0));
    const objective = m => obs.reduce((s, o, i) => s + o.w * ((m[i] - o.C) / o.C) ** 2, 0);
    const run = async f => {
      const S1 = JSON.parse(JSON.stringify(S0)), zones = {};
      for (const k of lu) if (S1.rates[k] && S1.rates[k][period]) S1.rates[k][period].rate *= f[k];
      for (const [id, z] of Object.entries(view.zones)) zones[id] = z.kind === 'gateway' && !z.gwSrc && f.gateways != null && z.gw ? { ...z, gw: { ...z.gw, [period + 'in']: (+z.gw[period + 'in'] || 0) * f.gateways, [period + 'out']: (+z.gw[period + 'out'] || 0) * f.gateways } } : z;
      return vol(await withoutMEAsync(period, () => Engine.assign({ ...view, zones }, { period, settings: S1 })));
    };
    const ones = Object.fromEntries(groups.map(k => [k, 1]));
    const m0 = vol(base0), obj0 = objective(m0), grad = {};
    let k = 0;
    for (const gk of groups) { prog(++k, groups.length + 1, `Measuring the effect of ${gk === 'gateways' ? 'estimated gateways' : LANDUSES[gk].label.toLowerCase()} trips`); const m = await run({ ...ones, [gk]: 1.5 }); grad[gk] = m.map((v, i) => (v - m0[i]) / 0.5); }
    // bounded coordinate descent on the linear model m(f) = m0 + Σ g_k (f_k − 1)
    const f = { ...ones }, lin = () => m0.map((v, i) => v + groups.reduce((s, g) => s + grad[g][i] * (f[g] - 1), 0));
    for (let sweep = 0; sweep < 60; sweep++) {
      let moved = 0;
      for (const g of groups) {
        const m = lin(); let num = 0, den = 0;
        obs.forEach((o, i) => { const a = m[i] - grad[g][i] * f[g], c2 = o.C * o.C; num += o.w * grad[g][i] * (o.C - a) / c2; den += o.w * grad[g][i] ** 2 / c2; });
        if (!(den > 0)) continue;
        const nf = U.clamp(num / den, LO, HI); moved = Math.max(moved, Math.abs(nf - f[g])); f[g] = nf;
      }
      if (moved < 1e-4) break;
    }
    for (const g of groups) f[g] = Math.round(f[g] * 1000) / 1000;
    prog(groups.length + 1, groups.length + 1, 'Confirming the factors');
    const m1 = await run(f), obj1 = objective(m1), objLin = objective(lin());
    const keep = obj1 < obj0 - 1e-9;
    const rec = { period, date: new Date().toISOString(), basis, groups, factors: keep ? f : ones, tried: f, objBefore: obj0, objAfter: keep ? obj1 : obj0, objLinear: objLin, kept: keep, nObs: obs.length,
      rmsBefore: Math.sqrt(obj0 / obs.reduce((s, o) => s + o.w, 0)), rmsAfter: Math.sqrt((keep ? obj1 : obj0) / obs.reduce((s, o) => s + o.w, 0)), applied: null,
      countIds: [...new Set(obs.flatMap(o => o.ids || (o.id ? [o.id] : [])))],
      bounds: [LO, HI], note: keep ? '' : 'The confirming run did not improve on the trip rates as they are, so every factor is 1.' };
    // A factor stuck at a bound usually points at missing or wrong land use, gateway flows or network, not at trip rates
    const lab = g => g === 'gateways' ? 'estimated gateways' : (LANDUSES[g] || { label: String(g) }).label.toLowerCase();
    rec.atBound = keep ? groups.filter(g => f[g] <= LO + 1e-9 || f[g] >= HI - 1e-9) : [];
    if (rec.atBound.length) rec.note = `${rec.atBound.map(lab).join(', ')} reached the ${LO}–${HI} bound. A factor at its bound usually means missing or wrong land use, gateway flows or roads: check those before applying.`;
    sc.calib ||= {}; sc.calib.tripEndFactors ||= {};
    const prev = sc.calib.tripEndFactors[period];
    if (prev && prev.applied) rec.applied = prev.applied;
    sc.calib.tripEndFactors[period] = rec;
    return { ok: true, ...rec };
  }
  async function withoutMEAsync(period, fn) { const me = Store.P.me, f = me && me[period]; if (f) delete me[period]; try { return await fn(); } finally { if (f) me[period] = f; } }
  /* Whether fitted factors may be applied. They go to Store.P.luFactors[period], which scales existing-land-use zones
     and estimated gateways only (development keeps its own trip rates). A factor at its bound points at land use,
     gateway flows or roads, not trip rates, so it is refused. */
  function canApply(sc = Store.sc, period = Store.S.period) {
    const rec = sc && sc.calib && sc.calib.tripEndFactors && sc.calib.tripEndFactors[period];
    if (!rec) return { ok: false, why: 'Fit the trip-end factors first.' };
    if (rec.applied) return { ok: false, why: 'The factors are already applied.' };
    if (!rec.kept) return { ok: false, why: rec.note || 'The factors did not improve the fit, so there is nothing to apply.' };
    if (rec.atBound && rec.atBound.length) return { ok: false, why: 'Not applied: a factor at its bound points at missing or wrong land use, gateway flows or roads. Fix those, then fit again.' };
    // applied as Store.P.luFactors, which Engine.tripEnds uses for existing-land-use zones and estimated gateways only,
    // so development and committed zones keep their own trip rates
    return { ok: true, why: '' };
  }
  /* Apply trip-end factors as Store.P.luFactors[period] = { <land use>: f, gateways: f }: Engine.tripEnds scales the
     land-use trips of existing-land-use zones and the flows of estimated gateways (no gwSrc) in every scenario. The
     trip rates and gateway flows themselves are not changed; undo puts back the factors that were there before. */
  function applyFactors(sc = Store.sc, period = Store.S.period) {
    const rec = sc.calib && sc.calib.tripEndFactors && sc.calib.tripEndFactors[period];
    if (!rec || !canApply(sc, period).ok) return false;
    const P = Store.P, lf = P.luFactors ||= {}, before = lf[period] ? JSON.parse(JSON.stringify(lf[period])) : null, f = {};
    for (const k of rec.groups) { const v = +rec.factors[k]; if (isFinite(v) && v > 0 && Math.abs(v - 1) > 1e-9) f[k] = Math.round(v * 1000) / 1000; }
    lf[period] = f;
    rec.applied = { at: new Date().toISOString(), via: 'luFactors', before, factors: f };
    for (const s of P.scenarios) for (const r of Object.values(s.res || {})) if (r) r.stale = true;
    memo.clear();
    return true;
  }
  function revertFactors(sc = Store.sc, period = Store.S.period) {
    const rec = sc.calib && sc.calib.tripEndFactors && sc.calib.tripEndFactors[period];
    if (!rec || !rec.applied) return false;
    if (rec.applied.via === 'luFactors') {
      const P = Store.P, lf = P.luFactors ||= {};
      if (rec.applied.before) lf[period] = rec.applied.before; else delete lf[period];
      if (!Object.keys(lf).length) delete P.luFactors;
      rec.applied = null;
      for (const s of P.scenarios) for (const r of Object.values(s.res || {})) if (r) r.stale = true;
      memo.clear();
      return true;
    }
    const S = Store.S, p = rec.applied.prior;
    for (const [k, v] of Object.entries(p.rates)) if (S.rates[k] && S.rates[k][period]) S.rates[k][period].rate = v;
    for (const [sid, zs] of Object.entries(p.gw)) { const s = Store.P.scenarios.find(x => x.id === sid); if (s) for (const [zid, [a, b]] of Object.entries(zs)) { const z = s.zones[zid]; if (z && z.gw) { z.gw[period + 'in'] = a; z.gw[period + 'out'] = b; } } }
    rec.applied = null;
    for (const s of Store.P.scenarios) for (const r of Object.values(s.res || {})) if (r) r.stale = true;
    memo.clear();
    return true;
  }

  /* ================= cordon OD export (PTV $O, long CSV, GeoJSON boundary zones) ================= */
  function cordonOD(sc, res, sl) {
    if (!sl || sl.type !== 'cordon' || sl.pts.length < 3) return { error: 'Draw a cordon (a closed area) first.' };
    if (!res || !res.paths || !res.arcKeys) return { error: 'Run the model again in this session: the cordon export traces the stored paths.' };
    const xs = slCrossings(sc, sl), dirOfArc = new Map();
    for (const x of xs) { dirOfArc.set(x.link + ':ab', x.side === 1 ? 'in' : 'out'); dirOfArc.set(x.link + ':ba', x.side === 1 ? 'out' : 'in'); }
    const inside = id => { const z = sc.zones[id]; return !!z && pointInPoly([z.lat, z.lng], sl.pts); };
    const pf = Engine.pcuF(Store.S), T = new Map(), bz = new Map();
    const bzOf = (key, kind) => { const id = 'b:' + key; if (!bz.has(id)) { const L = sc.links[key.split(':')[0]], x = xs.find(q => q.link === L.id); bz.set(id, { id, name: `${L.name || (CLASSES[L.cls] || CLASSES.local).label + ' road'} ${kind === 'in' ? 'in' : 'out'} (${dirOf(key.endsWith(':ab') ? x.bAB : x.bAB + 180)})`, ll: x ? x.ll : null, kind }); } return id; };
    for (const od of res.paths) {
      const zi = res.zoneIds[od.i], zj = res.zoneIds[od.j], ii = inside(zi), jj = inside(zj);
      for (const p of od.list) {
        let o = ii ? zi : null, d = jj ? zj : null, seen = ii;
        for (const a of p.arcs) {
          const k = res.arcKeys[a], dir = dirOfArc.get(k); if (!dir) continue;
          if (dir === 'in' && !o) { o = bzOf(k, 'in'); seen = true; }
          if (dir === 'out' && seen && !jj) d = bzOf(k, 'out');
        }
        if (!o || !d || o === d) continue;
        const key = o + '\u0000' + d; T.set(key, (T.get(key) || 0) + p.w / pf);
      }
    }
    const zones = [...new Set([...T.keys()].flatMap(k => k.split('\u0000')))].sort((a, b) => (a.startsWith('b:') - b.startsWith('b:')) || a.localeCompare(b));
    const num0 = new Map(); let ni = 1, nb = 1001;
    for (const z of zones) num0.set(z, z.startsWith('b:') ? nb++ : ni++);
    const nameOf = z => z.startsWith('b:') ? bz.get(z).name : (sc.zones[z] || {}).name || z;
    const per = res.period, hours = per === 'PM' ? '16.00 17.00' : '7.00 8.00';
    const lines = ['$O;D2', '* From  To', hours, '* Factor', '1.00', `* Spare Construct ${per} peak hour, veh/h, cordon “${sl.name}” (peak-hour times indicative)`];
    for (const [k, v] of [...T].sort()) { const [o, d] = k.split('\u0000'); if (v >= 0.005) lines.push(`${num0.get(o)} ${num0.get(d)} ${v.toFixed(2)}`); }
    lines.push('$NAMES'); for (const z of zones) lines.push(`${num0.get(z)} "${String(nameOf(z)).replace(/"/g, "'")}"`);
    const csv = BOM + U.csv([['from_id', 'from_name', 'to_id', 'to_name', 'veh_h'], ...[...T].sort().map(([k, v]) => { const [o, d] = k.split('\u0000'); return [num0.get(o), nameOf(o), num0.get(d), nameOf(d), U.r1(v)]; })]);
    const geojson = JSON.stringify({ type: 'FeatureCollection', features: [...bz.values()].filter(b => b.ll && num0.has(b.id)).map(b => ({ type: 'Feature', geometry: { type: 'Point', coordinates: [U.r7(b.ll[1]), U.r7(b.ll[0])] }, properties: { zone: num0.get(b.id), name: b.name, kind: b.kind } })) });
    return { o: lines.join('\r\n') + '\r\n', csv, geojson, zones: zones.length, boundary: bz.size, trips: [...T.values()].reduce((a, b) => a + b, 0) };
  }

  /* ================= screenline suggestions from OpenStreetMap (screenlines: optional) =================
     Rivers, canals and main railways funnel traffic onto their crossings, so each makes a natural screenline. The
     features come from Overpass (OpenStreetMap contributors, ODbL) for the network's bounding box, with a time limit
     on each server, cached per study area for the session; a failure says what to do instead. Railway sidings, yards
     and spurs are left out. A suggestion is added only when the user presses Add. */
  const OSM_SL = { timeout: 30000, prov: { source: 'OpenStreetMap contributors (Overpass API)', licence: 'ODbL 1.0', basis: 'derived' },
    query: bb => `[out:json][timeout:50];(way["waterway"~"^(river|canal)$"](${bb});way["railway"="rail"][!"service"](${bb}););out geom;` };
  const slOsmCache = new Map();
  let slSuggest = null;
  async function osmScreenFeatures(bb, opts) {
    const key = bb.map(v => v.toFixed(3)).join(',');
    if (slOsmCache.has(key)) return { data: slOsmCache.get(key), cached: true };
    let err = null;
    for (const url of typeof OVERPASS !== 'undefined' ? OVERPASS : []) {
      const ac = typeof AbortController !== 'undefined' ? new AbortController() : null, tm = ac ? setTimeout(() => ac.abort(), opts.timeout || OSM_SL.timeout) : null;
      try {
        const r = await fetch(url, { method: 'POST', body: 'data=' + encodeURIComponent(OSM_SL.query(bb.map(v => v.toFixed(6)).join(','))), headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, ...(ac ? { signal: ac.signal } : {}) });
        if (!r.ok) throw new Error(`${new URL(url).host} answered HTTP ${r.status}`);
        const data = await r.json();
        slOsmCache.set(key, data);
        return { data, cached: false };
      } catch (e) { err = e && e.name === 'AbortError' ? new Error(`${new URL(url).host} did not answer within ${Math.round((opts.timeout || OSM_SL.timeout) / 1000)} s`) : e; }
      finally { if (tm) clearTimeout(tm); }
    }
    throw err || new Error('no OpenStreetMap server is set up');
  }
  // Join ways that share an end point into polylines ([[lat, lng]]), longest first
  function joinWays(ways) {
    const left = ways.map(w => ({ id: w.id, pts: w.geometry.map(g => [+g.lat, +g.lon]) })).filter(w => w.pts.every(p => isFinite(p[0]) && isFinite(p[1]))), out = [];
    const same = (p, q) => Math.abs(p[0] - q[0]) < 1e-7 && Math.abs(p[1] - q[1]) < 1e-7;
    while (left.length) {
      const c = left.shift(), ids = [c.id];
      let pts = c.pts, grew = true;
      while (grew) {
        grew = false;
        for (let i = 0; i < left.length; i++) {
          const w = left[i].pts, a = pts[0], b = pts[pts.length - 1];
          let j = null;
          if (same(b, w[0])) j = pts.concat(w.slice(1)); else if (same(b, w[w.length - 1])) j = pts.concat(w.slice(0, -1).reverse());
          else if (same(a, w[w.length - 1])) j = w.concat(pts.slice(1)); else if (same(a, w[0])) j = w.slice().reverse().concat(pts.slice(1));
          if (j) { pts = j; ids.push(left[i].id); left.splice(i, 1); grew = true; break; }
        }
      }
      out.push({ pts, ids });
    }
    return out.sort((p, q) => q.pts.length - p.pts.length);
  }
  async function suggestScreenlines(sc = Store.sc, opts = {}) {
    const ns = Object.values(sc.nodes); if (ns.length < 2) return { ok: false, list: [], note: 'Import or draw roads first.' };
    const bb = [Math.min(...ns.map(n => n.lat)), Math.min(...ns.map(n => n.lng)), Math.max(...ns.map(n => n.lat)), Math.max(...ns.map(n => n.lng))];
    let got;
    try { got = await osmScreenFeatures(bb, opts); }
    catch (e) { return { ok: false, list: [], note: `Could not get rivers and railways from OpenStreetMap: ${e.message}. Draw screenlines with the Screenline tool (S) instead, or try again later.` }; }
    const groups = new Map();
    for (const e of (got.data && got.data.elements) || []) {
      if (e.type !== 'way' || !Array.isArray(e.geometry) || e.geometry.length < 2 || !e.tags) continue;
      const kind = /^(river|canal)$/.test(e.tags.waterway || '') ? e.tags.waterway : e.tags.railway === 'rail' && !e.tags.service ? 'railway' : null;
      if (!kind) continue;
      const k = kind + '\u0000' + String(e.tags.name || '').trim(); (groups.get(k) || groups.set(k, []).get(k)).push(e);
    }
    const have = new Set(store().screenlines.map(s => (s.src && s.src.osm || []).join(',')).filter(Boolean)), list = [];
    for (const [k, ws] of groups) {
      const [kind, name] = k.split('\u0000');
      for (const ch of joinWays(ws)) {
        const sl = { id: 'osm:' + hash(ch.ids.join(',')), type: 'screen', pts: ch.pts }, links = new Set(slCrossings(sc, sl).map(x => x.link));
        if (links.size < 2) continue;
        const nm = name || { river: 'Unnamed river', canal: 'Unnamed canal', railway: 'Railway' }[kind];
        list.push({ name: nm, kind, pts: ch.pts.map(p => [U.r7(p[0]), U.r7(p[1])]), osm: ch.ids.slice(0, 50), crossings: links.size, few: links.size < TAG.screen.minLinks, added: have.has(ch.ids.slice(0, 50).join(',')) });
      }
    }
    list.sort((a, b) => b.crossings - a.crossings);
    return { ok: true, list, cached: got.cached, note: list.length ? '' : 'No river, canal or railway crosses two or more roads in this network.' };
  }
  function addSuggested(item, role = 'valid') {
    if (!item || !Array.isArray(item.pts) || item.pts.length < 2) return null;
    const sl = { id: U.uid('sl'), name: String(item.name).slice(0, 80), type: 'screen', role, motorways: true, pts: item.pts,
      src: { ...OSM_SL.prov, date: new Date().toISOString().slice(0, 10), osm: item.osm, feature: item.kind } };
    store().screenlines.push(sl); item.added = true;
    return sl;
  }

  /* ================= CSV exports ================= */
  function countsCSV(sc = Store.sc) {
    const head = ['site_id', 'source', 'source_ref', 'road', 'dir', 'period', 'observed_veh_h', 'obs_method', 'count_date', 'n_days', 'ci95', 'role', 'in_core', 'modelled_veh_h', 'diff', 'diff_pct', 'geh', 'geh_band', 'tmdg_band_pass', 'tag_band_pass', 'either_pass', 'screenline_id', 'link_id', 'arc_dir', 'match_dist_m', 'road_class'];
    const out = [head], sls = store().screenlines.filter(s => s.type !== 'core');
    for (const p of PERIODS) {
      const res = Store.res(sc, p); if (!res) continue;
      for (const r of countRows(sc, p, res)) {
        const c = r.c, band = GEH_BANDS.find(([lo, hi]) => r.geh >= lo && r.geh < hi), slIds = sls.filter(s => slCrossings(sc, s).some(x => x.link === r.mt.link)).map(s => s.id).join(';');
        out.push([c.id, c.src, c.ref || '', c.road || c.name, c.dir || 'both', p, U.r1(r.obs), r.method, c.date || '', nDays(c) ?? '', ciOf(c) ? ciOf(c).csv : '', r.role, r.inCore ? 1 : 0, U.r1(r.model), U.r1(r.model - r.obs), r.obs > 0 ? U.r1(100 * (r.model - r.obs) / r.obs) : '',
          U.r3(r.geh), band ? (band[1] === Infinity ? '12+' : `${band[0]}-${band[1]}`) : '', tmdgBand(r.model, r.obs) ? 1 : 0, tagBand(r.model, r.obs) ? 1 : 0, r.geh < 5 || tagBand(r.model, r.obs) ? 1 : 0, slIds, r.mt.link, r.mt.dir, Math.round(r.mt.d), r.L ? r.L.cls : '']);
      }
    }
    return BOM + U.csv(out);
  }
  function summaryCSV(sc = Store.sc) {
    const out = [['criterion', 'standard', 'category', 'period', 'target', 'achieved', 'n', 'status']];
    for (const p of PERIODS) {
      if (!Store.res(sc, p)) continue;
      const a = assess(sc, p);
      for (const r of a.rows) out.push([r.t.label, (SOURCES[r.t.src] || SOURCES.tmdg).short + (r.t.table ? ', ' + r.t.table : ''), a.standard === 'tmdg' ? a.category : '', p, targetText(r.t), r.shown, r.n, r.status]);
      out.push(['Overall', a.standard === 'tmdg' ? 'TMDG 2019' : STANDARDS[a.standard], a.category, p, '', a.status, a.ev.link.n, a.line]);
    }
    return BOM + U.csv(out);
  }
  function turnsCSV(sc = Store.sc) {
    const out = [['site', 'site_ref', 'date', 'period', 'peak_hour', 'phf', 'leg', 'movement', 'road', 'counted_veh_h', 'modelled_veh_h', 'geh', 'tmdg_band_pass', 'tag_band_pass', 'estimated', 'balanced', 'role', 'flags']];
    for (const s of store().turnSites) for (const p of PERIODS) {
      const r = siteRows(sc, s, p), pk = s.peak && s.peak[p];
      for (const x of r.rows) out.push([s.name, s.ref, s.date, p, pk ? `${pk.start}-${pk.end}` : '', pk && pk.phf != null ? pk.phf : '', x.leg, x.mv, x.arm || '', U.r1(x.obs), x.model == null ? '' : U.r1(x.model), x.geh == null ? '' : U.r3(x.geh),
        x.model == null ? '' : tmdgBand(x.model, x.obs) ? 1 : 0, x.model == null ? '' : tagBand(x.model, x.obs) ? 1 : 0, x.estimated ? 1 : 0, x.balanced ? 1 : 0, s.role, [...(s.flags || []), ...(r.flags || [])].join('; ')]);
    }
    return BOM + U.csv(out);
  }
  function screenlinesCSV(sc = Store.sc) {
    const out = [['screenline', 'type', 'role', 'period', 'direction', 'motorways', 'crossing_arcs', 'counted_arcs', 'counted_veh_h', 'modelled_counted_veh_h', 'modelled_all_veh_h', 'diff', 'diff_pct', 'geh', 'completeness', 'status']];
    for (const sl of store().screenlines.filter(s => s.type !== 'core')) for (const p of PERIODS) for (const mw of [true, false]) {
      if (!mw && !slCrossings(sc, sl).some(x => x.mw)) continue;
      for (const r of slRows(sc, sl, p, undefined, { motorways: mw })) out.push([sl.name, sl.type, sl.role, p, r.label, mw ? 'with' : 'without', r.n, r.counted, U.r1(r.C), U.r1(r.Mc), U.r1(r.M), U.r1(r.d), r.pct == null ? '' : U.r1(100 * r.pct), r.geh == null ? '' : U.r3(r.geh), U.r3(r.complete), r.text]);
    }
    return BOM + U.csv(out);
  }
  const nDays = c => { if (c.n_days != null) return c.n_days; const m = /(\d+) counted days/.exec(c.note || ''); return m ? +m[1] : null; };

  /* ================= HTML: summary table, charts, sections ================= */
  const verifyPill = t => t.verified === false || (t.useVerified === false && t.src === 'tmdg' && t.use !== 'not-expected') ? ' <span class="pill" style="color:var(--warn)" title="Not yet checked against the source document">verify</span>' : '';
  /* The TAG M3.1 verdict beside row k, per period: the TAG test for the same evidence (tagPeer), never deciding the
     status. null where TAG sets no criterion. → { t, cells: [evaluate rows] } */
  function tagSide(as, k) {
    const t = as[0].rows[k].t, peer = tagPeer(t);
    return peer ? { t: peer, cells: as.map(a => peer === t ? a.rows[k] : evaluate(peer, a.ev)) } : null;
  }
  const tagTip = p => `UK TAG M3.1 (May 2026) ${p.t.table}: ${p.t.label}, ${p.t.crit}; target ${targetText(p.t)}. Shown alongside; it does not decide the TMDG status.`;
  function summaryTable(as) {
    const ids = as[0].rows.map(r => r.t.id);
    const rows = ids.map((id, k) => {
      const t = as[0].rows[k].t, cells = as.map(a => a.rows[k]);
      if (t.group === 'me' && cells.every(c => c.status === 'No data')) return null;
      const st = cells.map((c, i) => `<span style="color:${STATUS_COL(c.status)}" title="${esc(c.verdict && c.status === 'Info' ? `${c.verdict} (reported, does not decide the status)` : c.status)}">${as.length > 1 ? as[i].period + ' ' : ''}${esc(c.status)}</span>`).join('<br>');
      const ts = tagSide(as, k), tag = ts ? `<span title="${esc(tagTip(ts))}">${ts.cells.map((c, i) => `<span style="color:${STATUS_COL(c.status)}">${as.length > 1 ? as[i].period + ' ' : ''}${esc(c.status)}</span>`).join('<br>')}</span>` : '<span class="note" title="TAG M3.1 sets no criterion for this test">–</span>';
      return [`<span title="${esc(tooltip(t))}">${esc(t.label)}</span>${verifyPill(t)}`, `<span class="note">${esc(t.crit)}</span>`, `<span title="${esc(tooltip(t))}">${esc(targetText(t))}</span>`, ...cells.map(c => esc(c.shown)), st, tag];
    }).filter(Boolean);
    return Kit.table({ head: ['Test', 'Criterion', 'Target', ...as.map(a => ({ label: a.period, n: true })), 'Status', 'TAG M3.1'], rows });
  }
  // GEH bands as one stacked bar per period
  function gehBar(as) {
    const W = 360, cols = ['var(--ok)', 'var(--los-c)', 'var(--warn)', 'var(--los-e)', 'var(--danger)'];
    const bars = as.map((a, i) => {
      const s = a.ev.link; let x = 60;
      const segs = s.n ? s.gehBands.map((b, k) => { const w = (W - 70) * (b.share || 0); const r = `<rect x="${x.toFixed(1)}" y="${8 + i * 26}" width="${Math.max(0, w).toFixed(1)}" height="18" fill="${cols[k]}"><title>GEH ${b.label}: ${b.n} (${pct(b.share)}; cumulative ${pct(b.cum)})</title></rect>`; x += w; return r; }).join('') : '';
      return `<text x="0" y="${21 + i * 26}">${a.period}${s.n ? '' : ' –'}</text>${segs}`;
    }).join('');
    const leg = GEH_BANDS.map((b, k) => `<span class="lg-row" style="margin:0;gap:4px"><span class="lg-sw" style="background:${cols[k]}"></span>${b[1] === Infinity ? '≥ 12' : b[0] === 0 ? '< 5' : `${b[0]}–${b[1]}`}</span>`).join('');
    return `<svg class="mini-chart" viewBox="0 0 ${W} ${as.length * 26 + 8}" style="height:${as.length * 26 + 8}px">${bars}</svg><div class="hud-row" style="font-size:11.5px;gap:10px;flex-wrap:wrap">${leg}</div>`;
  }
  /* Scatter: y = x, dashed 0.9x and 1.1x, GEH = 5 envelopes M = C + 6.25 ± √(39.0625 + 25C), the through-origin fit;
     calibration points filled, validation hollow, coloured by GEH; log-log and no-motorway options. */
  function scatterSvg(rows, st, { log = false, noMw = false, width = 300, height = 230 } = {}) {
    const pts = rows.filter(r => !noMw || !r.mw);
    const mx = Math.max(100, ...pts.map(r => Math.max(r.obs, r.model))) * 1.05, P = 36, W = width, H = height, lo = 10;
    const f = v => log ? Math.log10(Math.max(lo, v)) : v, fmx = f(mx), fmn = log ? Math.log10(lo) : 0;
    const X = v => P + (W - P - 8) * (f(v) - fmn) / (fmx - fmn), Y = v => H - 22 - (H - 30) * (f(v) - fmn) / (fmx - fmn);
    const curve = g => { const s = []; for (let k = 0; k <= 40; k++) { const c = log ? lo * Math.pow(mx / lo, k / 40) : mx * k / 40, m = g(c); if (m >= (log ? lo : 0) && m <= mx) s.push(`${s.length ? 'L' : 'M'}${X(c).toFixed(1)},${Y(m).toFixed(1)}`); } return s.join(''); };
    const env = sgn => c => c + 6.25 + sgn * Math.sqrt(39.0625 + 25 * c);
    const col = g => g < 5 ? 'var(--ok)' : g < 10 ? 'var(--warn)' : 'var(--danger)';
    const dots = pts.map(r => { const hollow = r.role === 'valid', grey = !r.scored, c = grey ? 'var(--faint)' : col(r.geh);
      return `<circle cx="${X(r.obs).toFixed(1)}" cy="${Y(r.model).toFixed(1)}" r="4" fill="${hollow || grey ? 'var(--panel)' : c}" stroke="${c}" stroke-width="${hollow ? 1.8 : 1}" data-fly="count:${esc(r.c.id)}" style="cursor:pointer"><title>${esc(r.c.name)}: counted ${U.fmt.int(r.obs)}, model ${U.fmt.int(r.model)}, GEH ${num(r.geh, 1)} · ${ROLES[r.role] || r.role}${grey ? ' (not scored)' : ''}</title></circle>`; }).join('');
    const fit = st && st.b0 ? `<path d="${curve(c => st.b0 * c)}" stroke="var(--accent)" stroke-width="1.6" fill="none"/>` : '';
    return `<svg class="mini-chart" viewBox="0 0 ${W} ${H}" style="height:${H}px;max-width:${W + 40}px" role="img" aria-label="Modelled against counted flows">
      <path d="${curve(env(-1))}" stroke="var(--line)" fill="none"/><path d="${curve(env(1))}" stroke="var(--line)" fill="none"/>
      <path d="${curve(c => 0.9 * c)}" stroke="var(--faint)" stroke-dasharray="3 3" fill="none"/><path d="${curve(c => 1.1 * c)}" stroke="var(--faint)" stroke-dasharray="3 3" fill="none"/>
      <path d="${curve(c => c)}" stroke="var(--muted)" fill="none"/>${fit}
      <line x1="${P}" y1="${H - 22}" x2="${W - 8}" y2="${H - 22}" stroke="var(--line)"/><line x1="${P}" y1="8" x2="${P}" y2="${H - 22}" stroke="var(--line)"/>
      ${dots}<text x="${W - 8}" y="${H - 8}" text-anchor="end">counted veh/h${log ? ' (log)' : ''} → ${U.fmt.int(mx)}</text><text x="${P - 4}" y="14" text-anchor="end">model</text></svg>
      <div class="note" style="font-size:11.5px">Filled: calibration · hollow: validation · grey: not scored. Solid line y = x, dashed ±10%, thin curves GEH = 5, blue: fit through the origin${st && st.b0 ? ` (b0 ${num(st.b0, 3)})` : ''}.</div>`;
  }
  const roleSel = (c, period) => {
    const r = roleOf(c, period), gw = r === 'gateway';
    if (gw) return '<span class="pill" title="Used to set gateway flows, so it cannot validate the model">Gateway</span>';
    const b = baseRole(c);
    const why = seenBy(c, period);
    return `<select data-cal-role="${esc(c.id)}" aria-label="Role of ${esc(c.name)}" style="font-size:12px">${Kit.opt({ me: 'Calibration', valid: 'Validation', off: 'Not used' }, b)}</select>${why ? ` <span class="pill" style="color:var(--warn)" title="${why === 'seen by ME' ? 'Matrix estimation used this count' : 'This count fitted the trip-end factors applied to the trip rates'}, so it cannot validate the model">${why}</span>` : ''}`;
  };
  function worstTable(ev) {
    const rows = ev.all.filter(r => inArea(r, ev.area)).slice(0, 12).map(r => {
      const c = r.c, src = DataHub.SOURCES[c.src] || { short: c.src, colour: 'var(--muted)' }, d = r.model - r.obs;
      return { cls: r.scored ? '' : 'muted', cells: [`<a href="#" data-fly="count:${esc(c.id)}">${esc(c.name.slice(0, 44))}</a>${c.dir ? ' ' + esc(c.dir) : ''}`, `<span class="pill" style="color:${src.colour}">${esc(src.short)}</span>`,
        `<span class="prov ${r.method === 'measured_hour' ? 'measured' : 'estimated'}" title="${esc(METHODS[r.method].label + ' · ' + (r.how || ''))}">${METHODS[r.method].short}</span>`, roleSel(c, ev.period), U.fmt.int(r.obs), U.fmt.int(r.model), U.fmt.signed(d), r.obs > 0 ? U.fmt.signedPct(d / r.obs, 0) : '–', Kit.gehBadge(r.geh),
        tmdgBand(r.model, r.obs) ? '✓' : '✗', esc(r.L ? (CLASSES[r.L.cls] || CLASSES.local).label : ''), U.fmt.int(r.mt.d), `<button class="btn small" data-cal="sla:${esc(r.mt.link)}:${esc(r.mt.dir)}" ${ev.res && ev.res.paths ? '' : 'disabled title="Run the model again in this session to trace who uses the road"'}>Select link</button>`] };
    });
    return Kit.table({ head: ['Count', 'Source', 'Method', 'Role', { label: 'Counted', n: true }, { label: 'Model', n: true }, { label: 'M−C', n: true }, { label: '%', n: true }, { label: 'GEH', n: true }, 'Band', 'Road class', { label: 'Match m', n: true }, ''], rows, empty: 'No counts match the network.' });
  }
  // Rivers and railways from OpenStreetMap as screenline suggestions, each added with one press
  function suggestBlock(sc) {
    const s = slSuggest && slSuggest.scId === sc.id ? slSuggest : null;
    const btn = `<div class="actions" style="margin:4px 0"><button class="btn small" data-cal="slsuggest">Suggest screenlines from rivers and railways</button><span class="note">From OpenStreetMap (ODbL), for this network's extent.</span></div>`;
    if (!s) return btn;
    if (!s.ok || !s.list.length) return btn + `<p class="note warn">${esc(s.note)}</p>`;
    return btn + Kit.table({ head: ['Suggested screenline', 'Feature', { label: 'Roads crossed', n: true }, ''], rows: s.list.map((x, i) => [esc(x.name), esc(x.kind), `${U.fmt.int(x.crossings)}${x.few ? ` <span class="note warn" title="TAG M3.1 Table 1 asks for at least ${TAG.screen.minLinks} links per screenline">under ${TAG.screen.minLinks}</span>` : ''}`,
      x.added ? '<span class="note">added</span>' : `<button class="btn small" data-cal="sladd:${i}">Add</button>`]) });
  }
  function screenlineSection(sc, period) {
    const sls = store().screenlines.filter(s => s.type !== 'core');
    if (!sls.length) return '<p class="note">No screenlines yet. Use the Screenline tool (S) to draw a line across the roads a river, railway or ridge funnels traffic onto, or a closed cordon round the study area.</p>' + suggestBlock(sc);
    const res = Store.res(sc, period), rows = [];
    for (const sl of sls) for (const mw of [true, false]) {
      if (!mw && !slCrossings(sc, sl).some(x => x.mw)) continue;
      for (const r of slRows(sc, sl, period, res, { motorways: mw })) rows.push({ fly: 'screenline:' + sl.id, cells: [esc(sl.name) + (mw ? '' : ' <span class="note">without motorways</span>'), sl.role === 'valid' ? 'Validation' : 'Calibration', esc(r.label), `${r.counted}/${r.n}`, U.fmt.int(r.C), U.fmt.int(r.Mc), U.fmt.int(r.M), r.ok ? U.fmt.signedPct(r.pct, 1) : '–', r.ok ? Kit.gehBadge(r.geh) : '–', r.ok ? (r.w5 ? '✓' : '✗') : '–', r.ok ? (r.geh < 4 ? '✓' : '✗') : '–', `${esc(r.text)}${r.notIndependent ? ' <span class="pill" style="color:var(--warn)" title="Uses counts that also calibrate the model">not independent</span>' : ''}`] });
    }
    return Kit.table({ head: ['Screenline (Table S1)', 'Role', 'Direction', 'Counted', { label: 'ΣC', n: true }, { label: 'ΣM counted', n: true }, { label: 'ΣM all', n: true }, { label: 'Diff', n: true }, { label: 'GEH', n: true }, 'Within 5%', 'GEH < 4', 'Status'], rows, empty: res ? 'No road crosses these screenlines.' : 'Run the model to compare.' }) +
      `<p class="note">Totals of hourly directional vehicles. A screenline with uncounted crossings is incomplete and has no pass or fail${cfg().minComplete < 1 ? ` (${pct(cfg().minComplete, 0)} counted is accepted, a project setting)` : ''}. TAG M3.1 asks for at least ${TAG.screen.minLinks} links per screenline and keeps calibration and validation screenlines separate.</p>` + suggestBlock(sc);
  }
  function turnSection(sc, period) {
    const st = store();
    const head = `<div class="actions" style="margin:4px 0"><button class="btn small" data-cal="importturns">Import turning counts (CSV)</button>${st.turnSites.length ? `<button class="btn small" data-cal="balance">Balance adjacent sites (${period})</button><label class="check" style="font-size:12.5px"><input type="checkbox" data-cal-set="noSinks" ${cfg().noSinks ? 'checked' : ''}> No sinks (use the larger flow)</label><label class="check" style="font-size:12.5px"><input type="checkbox" data-cal-set="useBalanced" ${cfg().useBalanced ? 'checked' : ''}> Use balanced counts</label>` : ''}</div>`;
    if (!st.turnSites.length) return head + '<p class="note">Import 15-minute classified turning counts (long or wide CSV, or SCATS detector counts). Spare Construct finds the peak hour and PHF, matches each site to the junction within 40 m and compares every movement with the model.</p>';
    const rows = st.turnSites.map(s => {
      const r = siteRows(sc, s, period), pk = s.peak && s.peak[period], ts = statsOf(r.rows.filter(x => x.model != null));
      return { fly: 'turn-site:' + s.id, cells: [esc(s.name), esc(s.date || '–'), hm(pk), pk && pk.phf != null ? num(pk.phf, 2) : '–', pk ? U.fmt.int(pk.total) : '–', r.node ? (r.view && r.view.kind === 'cluster' ? '<span style="color:var(--warn)" title="Define a junction site in the Junctions tab">multi-node, not scored</span>' : r.view && r.view.kind === 'site' ? 'site ✓' : '✓') : '<span style="color:var(--warn)">no junction</span>', ts.n ? `${ts.cnt.geh5}/${ts.n} · ${pct(ts.geh5, 0)}` : '–', esc(s.role === 'valid' ? 'Validation' : s.role === 'off' ? 'Not used' : 'Calibration'), esc([...(s.flags || []), ...(r.flags || [])].join('; ') || (r.note || ''))] };
    });
    return head + Kit.table({ head: ['Site', 'Date', `${period} peak hour`, { label: 'PHF', n: true }, { label: 'Veh/h', n: true }, 'Junction', 'GEH < 5', 'Role', 'Flags'], rows }) +
      '<p class="note">Peak hour: the busiest four consecutive 15-minute intervals inside the window. PHF = peak-hour volume / (4 × busiest 15 minutes); it is what SIDRA calls the peak flow factor. Select a site for the movement table.</p>';
  }
  function jtSection(sc, period, ev) {
    if (!ev.jtRows.length) return '<p class="note">Use the Travel time tool (P) and press <b>Add to journey checks</b>, then enter observed times (survey or GPS) or fetch Google typical times.</p>';
    const rows = ev.jtRows.map(r => [esc(r.j.name), U.fmt.len(r.len), U.fmt.dur(r.model),
      `<input type="number" min="0" step="1" data-cal-jt="${esc(r.j.id)}" value="${r.obs ? r.obs.t : ''}" placeholder="s" style="width:70px" aria-label="Observed ${period} time in seconds">`,
      r.google ? `${U.fmt.dur(r.google.t)} <span class="note" title="${esc(r.google.label)}">predicted</span>` : '–',
      r.chk ? U.fmt.signedPct(r.chk.pct, 0) : '–', r.chk ? (r.chk.tier1 ? '✓' : '✗') : '–', r.chk ? (r.chk.tier2 ? '✓' : '✗') : '–',
      r.short ? '<span class="note warn">under 3 km</span>' : r.long ? '<span class="note warn">over 15 km</span>' : '',
      `<button class="btn small icon" data-do="jtdel:${esc(r.j.id)}" aria-label="Remove">✕</button>`]);
    return Kit.table({ head: ['Journey', { label: 'Length', n: true }, { label: 'Model', n: true }, `Observed ${period} (s)`, `Google ${period}`, { label: 'Diff', n: true }, 'Tier 1', 'Tier 2', '', ''], rows }) +
      `<div class="actions" style="margin-top:6px"><button class="btn small" data-do="jtgoogle">Get Google times (${period} peak)</button><span class="note">${esc(DataHub.JT_CRITERIA.text)} Tier 2: within 25% or 1.5 minutes. Observed times are used where entered; otherwise Google's ${esc(DataHub.JT_CRITERIA.googleLabel.toLowerCase())}. Routes under 3 km or over 15 km are flagged.</span></div>`;
  }
  function convSection(res) {
    if (!res) return '<p class="note">Run the model first.</p>';
    const c = res.conv || [], g = c.filter(e => e.gap > 0);
    let spark = '';
    if (g.length > 1) {
      const lmin = Math.log10(Math.min(...g.map(e => e.gap), res.gapTol || 1e-4)), lmax = Math.log10(Math.max(...g.map(e => e.gap)));
      spark = Kit.line([{ pts: g.map((e, i) => [i, (Math.log10(e.gap) - lmin) / Math.max(1e-9, lmax - lmin)]), colour: 'var(--accent)', label: 'relative gap (log scale)' },
        { pts: c.filter(e => e.P != null).map((e, i) => [i + (g.length - c.filter(x => x.P != null).length), e.P]), colour: 'var(--ok)', dash: '4 3', label: 'P: share of links changing by less than 1%' }],
      { xMax: Math.max(1, g.length - 1), yMax: 1, yLabel: '', xLabels: ['iteration 1', `iteration ${n0(res.it)}`], cssHeight: 110 });
    }
    const gl = (Engine.STANDARDS && Engine.STANDARDS.conv && Engine.STANDARDS.conv.gapLabel) || 'relative gap';
    return `<p class="note ${res.converged === false ? 'warn' : ''}" style="${res.converged === false ? 'color:var(--danger)' : ''}">${esc(res.convNote || `${res.it} iterations, ${gl} ${num(res.gap * 100, 4)}%.`)}</p>${spark}
      <p class="note">TAG M3.1 Table 4: %GAP under 0.1% and P over 98% for 4 iterations running. Spare Construct's own gate is ${num((res.gapTol || 1e-4) * 100, 3)}% (${esc(gl)}).</p>`;
  }
  function meSection(sc, period) {
    const info = Store.P.me && Store.P.me.info && Store.P.me.info[period], c = cfg();
    const ctl = `<div class="actions" style="margin:4px 0;flex-wrap:wrap"><label class="check">XAMAX <select data-cal-set="xamax">${Kit.opt({ 2: '2 (default)', 1.5: '1.5', 1.25: '1.25', 1.1: '1.1' }, c.xamax)}</select></label>
      <label class="check">Prior matrix <select data-cal-set="priorQuality">${Kit.opt({ high: 'high quality (TMDG 5%)', low: 'low quality (TMDG 10%)' }, c.priorQuality)}</select></label>
      <button class="btn small primary" data-cal="me">Run matrix estimation (${period})</button><button class="btn small" data-cal="reduceme">Reduce ME until Table 5 passes</button>${info ? `<button class="btn small" data-cal="clearme">Clear estimation</button>` : ''}</div>
      <p class="note">Matrix estimation can hide network errors (TAG M3.1 8.3.9). Fix network issues first. It uses calibration counts only and runs on the base year (${Store.S.baseYear}). On NZTA projects matrix estimation needs the peer reviewer's agreement (TMDG).</p>`;
    if (!info) return ctl;
    const m1 = info.m1, warn = !info.roleAware ? '<p class="note warn">This estimation was run outside the calibration panel and saw every count, so no count is independent of it. Run it again from here after holding out validation counts.</p>' : '';
    const vb = info.validBefore, va = info.validAfter, cb = info.calibBefore || info.before, ca = info.calibAfter || info.after;
    const vs = (a, b) => a && b ? `${pct(a.geh5 ?? a.pass, 0)} → ${pct(b.geh5 ?? b.pass, 0)} (n=${n0(b.n)})` : '–';
    let h = ctl + warn + `<div class="kpis" style="grid-template-columns:repeat(auto-fill,minmax(150px,1fr))">${Kit.kpi('Matrix total', m1 ? U.fmt.signedPct(m1.totalPct / 100, 1) : '–', m1 ? `${U.fmt.int(m1.priorTotal)} → ${U.fmt.int(m1.total)} veh/h` : '')}${Kit.kpi('Calibration GEH < 5', vs(cb, ca))}${Kit.kpi('Validation GEH < 5', vs(vb, va), 'independent counts')}${Kit.kpi('XAMAX', esc(String(info.xamax || '–')), `${n0(info.iters)} pass${+info.iters === 1 ? '' : 'es'} · ${esc(info.stopped || '')}`)}</div>`;
    if (m1) {
      h += Kit.table({ head: ['Table M1', 'Prior → estimated', 'Target', 'Source'], rows: [
        ['Cells (OLS)', `slope ${num(m1.cells.slope, 3)}, R² ${num(m1.cells.r2, 3)}, intercept ${num(m1.cells.intercept, 2)}${m1.cells.interceptFlag ? ' <span class="pill" style="color:var(--warn)">intercept over 1% of the mean cell</span>' : ''}`, 'slope 0.98–1.02, R² > 0.95, intercept near 0', 'TAG M3.1 Table 5'],
        ['Trip ends, all zones', `slope ${num(m1.ends.all.slope, 3)}, R² ${num(m1.ends.all.r2, 3)}`, 'slope 0.99–1.01, R² > 0.98', 'TAG M3.1 Table 5'],
        ['Trip ends, without gateways', `slope ${num(m1.ends.noGateways.slope, 3)}, R² ${num(m1.ends.noGateways.r2, 3)}`, 'slope 0.99–1.01, R² > 0.98', 'TAG M3.1 Table 5'],
        ['Trip length mean / SD', `${U.fmt.len(m1.tld.meanP)} → ${U.fmt.len(m1.tld.meanE)} (${U.fmt.signedPct(m1.tld.meanPct / 100, 1)}) / ${U.fmt.signedPct(m1.tld.sdPct / 100, 1)}`, 'both within 5%', 'TAG M3.1 Table 5'],
        ['Trip-length coincidence ratio', num(m1.tld.cr, 3), TMDG.meCR.v[category()] ? `> ${TMDG.meCR.v[category()]}` : 'no TMDG value for this category', 'TMDG section 8'],
        ['Trip-end GEH', m1.tripEndGeh.bands.map((b, i) => `${esc(b)}: ${n0(m1.tripEndGeh.n[i])}`).join(' · '), 'reported', 'TMDG section 8'],
        ['Cells changed', `> 50%: ${n0(m1.cellChange.over50)} · 40–50%: ${n0(m1.cellChange.b40)} · 30–40%: ${n0(m1.cellChange.b30)} (of ${n0(m1.cellChange.n)})`, 'reported', 'TMDG section 8'],
        ['Sector cells within 5%', `${n0(m1.sectors.within5)} of ${n0(m1.sectors.tested)}`, 'report only', 'Spare Construct (TAG Table 5 sector test)'],
      ] }) + `<details><summary class="note">Top 10 cell changes and trip-length bins</summary>${Kit.table({ head: ['From', 'To', { label: 'Prior', n: true }, { label: 'Estimated', n: true }, { label: 'Change', n: true }], rows: m1.top.map(t => [esc(t.from), esc(t.to), num(+t.prior, 1), num(+t.est, 1), isFinite(+t.pct) ? U.fmt.signedPct(+t.pct, 0) : '–']) })}
        ${Kit.table({ head: ['Distance', { label: 'Prior', n: true }, { label: 'Estimated', n: true }], rows: m1.tld.bins.map((b, i) => [esc(b), pct(m1.tld.prior[i]), pct(m1.tld.est[i])]) })}<p class="note">Trip lengths on the ${esc(m1.tld.basis)}.</p></details>`;
    }
    const fut = Store.P.scenarios.some(s => Engine.yearOf(s) !== Store.S.baseYear);
    if (fut) h += `<p class="note">${esc(futureText(Store.P.me && Store.P.me.info && Store.P.me.info[period]))}</p>`;
    return h;
  }
  function workflowSection(sc, period) {
    const nc = networkChecks(sc, period), st = stressState && stressState.scId === sc.id && stressState.period === period ? stressState : null;
    const tf = sc.calib && sc.calib.tripEndFactors && sc.calib.tripEndFactors[period];
    const step = (n, title, body) => `<div style="border-top:1px solid var(--line);padding:8px 0"><div class="lg-title">${n}. ${title}</div>${body}</div>`;
    const s1 = `<ul class="note" style="margin:4px 0 4px 18px;padding:0">
        <li>${nc.far.length ? `${nc.far.length} count${nc.far.length === 1 ? ' is' : 's are'} matched more than 30 m from the road: ${nc.far.slice(0, 4).map(r => `<a href="#" data-fly="count:${esc(r.c.id)}">${esc(r.c.name.slice(0, 30))}</a> (${U.fmt.int(r.mt.d)} m)`).join(', ')}` : 'Every count sits within 30 m of its road.'}</li>
        <li>Worst fits: ${nc.worst.map(r => `<a href="#" data-fly="count:${esc(r.c.id)}">${esc(r.c.name.slice(0, 30))}</a> (GEH ${num(r.geh, 1)})`).join(', ') || 'none'}. Use <b>Select link</b> in the table above to see who uses each road.</li>
        <li>${nc.mwZones.length ? `${nc.mwZones.length} zone${nc.mwZones.length === 1 ? '' : 's'} would connect to a motorway or ramp: ${nc.mwZones.slice(0, 4).map(z => `<a href="#" data-fly="zone:${esc(z.id)}">${esc(z.name)}</a>`).join(', ')}. Set their connectors by hand.` : 'No zone connects only to a motorway or ramp.'}</li>
        <li>${nc.noDelay.length ? `${nc.noDelay.length} road direction${nc.noDelay.length === 1 ? ' is' : 's are'} over capacity with no junction delay, e.g. <a href="#" data-fly="link:${esc(nc.noDelay[0].L.id)}">${esc(nc.noDelay[0].L.name || 'unnamed road')}</a> (V/C ${num(nc.noDelay[0].vc, 2)}). Check the lanes and capacity.` : 'No road is over capacity without junction delay.'}</li></ul>`;
    const s2 = `<div class="actions"><button class="btn small" data-cal="stress">Run the stress test (${period}, demand × 1.1 and × 1.2)</button></div>` + (st ? Kit.table({ head: ['Junction', { label: 'DoS', n: true }, ...st.factors.map(f => ({ label: `× ${f}`, n: true })), 'Flag'], rows: st.nodes.slice(0, 8).map(n => ({ fly: 'node:' + n.id, cells: [esc(n.name), num(n.dos0, 2), ...n.runs.map(r => r.dos == null ? '–' : `${num(r.dos, 2)} (${U.fmt.signedPct(r.rise, 0)})`), n.flag ? '<span style="color:var(--danger)">rises more than twice the demand</span>' : ''] })) }) +
      `<p class="note">${st.links.length} road direction${st.links.length === 1 ? '' : 's'} go over capacity with no junction delay. The Stress layer marks the flagged junctions; the most sensitive is <b>${esc(st.top ? st.top.name : '–')}</b>.</p>` : '<p class="note">Flags junctions whose degree of saturation rises more than twice as fast as demand, and roads over capacity with no junction delay.</p>');
    const fRows = tf ? tf.groups.map(g => [luName(g), num(+tf.factors[g], 3), !tf.applied ? '' : tf.applied.via === 'luFactors' ? (tf.applied.factors && tf.applied.factors[g] != null ? `× ${num(+tf.applied.factors[g], 3)} (existing zones${g === 'gateways' ? ', estimated gateways' : ''})` : 'unchanged') : tf.applied.prior && tf.applied.prior.rates && tf.applied.prior.rates[g] != null && Store.S.rates[g] && Store.S.rates[g][period] ? `${num(+tf.applied.prior.rates[g], 4)} → ${num(+Store.S.rates[g][period].rate, 4)}` : '']) : [];
    const s3 = `<div class="actions"><button class="btn small" data-cal="tef">Fit trip-end factors (${period})</button>${tf && canApply(sc, period).ok ? '<button class="btn small primary" data-cal="tefapply">Apply to existing land use</button>' : ''}${tf && tf.applied ? '<button class="btn small" data-cal="tefrevert">Undo applied factors</button>' : ''}</div>` +
      (tf ? Kit.table({ head: ['Trip ends', { label: 'Factor', n: true }, 'Rate applied'], rows: fRows }) + `<p class="note">Fitted to ${n0(tf.nObs)} calibration ${tf.basis === 'screenlines' ? 'screenline totals' : 'count-group totals (road class by compass sector)'} (never validation counts), bounds ${(tf.bounds || []).map(x => num(+x, 1)).join('–')} (Spare Construct bounds): weighted RMS error ${pct(tf.rmsBefore, 1)} → ${pct(tf.rmsAfter, 1)}. ${esc(tf.note)}${tf.applied ? ` Applied: existing-land-use zones and estimated gateways in every scenario use them in the ${period} peak; development and committed zones keep their own trip rates.` : canApply(sc, period).ok ? ` Applying scales the ${period} trips of existing-land-use zones and estimated gateways in every scenario (Store.P.luFactors); development and committed zones keep their own trip rates, and the trip rates in Settings are not changed.` : ` ${esc(canApply(sc, period).why)}`}</p>` : '<p class="note">Up to seven factors: one per main land use and one for estimated gateways (gateways set from counts keep their counts).</p>');
    return step(1, 'Network checks', s1) + step(2, 'Stress test', s2) + step(3, 'Trip-end factors', s3) + step(4, 'Bounded matrix estimation', meSection(sc, period));
  }
  function provenance(sc) {
    const d = D(), by = new Map(), period = Store.S.period, dates = [];
    for (const r of countRows(sc, period)) { const k = `${r.c.src}|${r.method}|${r.role}`; by.set(k, (by.get(k) || 0) + 1); if (r.c.date) dates.push(r.c.date); }
    const rows = [...by].sort().map(([k, n]) => { const [s, m, r] = k.split('|'); return [esc((DataHub.SOURCES[s] || { label: s }).label), esc(METHODS[m].label), esc(ROLES[r] || r), U.fmt.int(n)]; });
    dates.sort();
    const ld = Object.entries(d.loaded || {}).filter(([k, v]) => v && v.at && ['nzta', 'at', 'user'].includes(k)).map(([k, v]) => `${(DataHub.SOURCES[k] || { short: k }).short} loaded ${new Date(v.at).toLocaleString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric', hour: 'numeric', minute: '2-digit' })}`).join(' · ');
    return Kit.table({ head: ['Source', 'Method', 'Role', { label: 'Counts', n: true }], rows, empty: 'No matched counts.' }) +
      `<p class="note">${dates.length ? `Count dates ${esc(dates[0])} to ${esc(dates[dates.length - 1])}. ` : ''}${esc(ld)}. Virtual NZTA sites are estimates and are never scored. TMDG 5.1 warns that factored counts flatter the fit, so the measured-only figures are shown too.</p>`;
  }
  function footnote(a) {
    const s = a.standard === 'tmdg' ? `TMDG 2019 (NZTA), Category ${a.category} (${CAT_NAMES[a.category]})` : STANDARDS[a.standard];
    return `<p class="note" style="margin-top:8px">Criteria from ${esc(s)}, with UK TAG M3.1 (May 2026) alongside. Meeting them does not by itself show the model is fit for purpose (TAG M3.1 3.2.2). Spare Construct is a sketch model. ${esc(DISCLAIMER)}</p>`;
  }
  const DISCLAIMER = 'Calibration and validation statistics are reported against the NZTA Transport Model Development Guidelines (2019) for the stated model purpose category. Those criteria are not a pass/fail checklist, and meeting them does not by itself make a model fit for purpose. Fitness for purpose is a judgment for the analyst and peer reviewer and, where NZTA is involved, NZTA\'s modelling specialists.';

  /* ================= the panel (top of the Traffic data tab) ================= */
  const openSec = { worst: true };
  const det = (id, title, body) => `<details data-cal-sec="${id}" ${openSec[id] ? 'open' : ''} style="margin-top:8px"><summary style="cursor:pointer;font-weight:600">${title}</summary><div style="margin-top:6px">${body}</div></details>`;
  function panelHtml(sc = Store.sc) {
    const d = D(), c = cfg(), per = Store.S.period, cat = category();
    if (!d.counts.length && !store().turnSites.length) return `<div class="cal-panel"><div class="lg-title">Model against counts</div><p class="note">Load counts to check the model against what is really on the roads, or import turning counts.</p><div class="actions"><button class="btn small" data-cal="importturns">Import turning counts (CSV)</button></div></div>`;
    const as = PERIODS.filter(p => Store.res(sc, p)).map(p => assess(sc, p));
    const evP = (as.find(a => a.period === per) || {}).ev || evidence(sc, per);
    const head = `<div class="bar-h" style="flex-wrap:wrap;gap:6px"><h3>Model against counts · ${esc(sc.name)}</h3>
      <label class="check" title="NZTA TMDG 2019 model purpose category">Purpose <select data-cal-set="category">${Kit.opt(Object.fromEntries(CATS.map(k => [k, `${k} · ${CAT_NAMES[k]}`])), cat)}</select></label>
      <label class="check">Standard <select data-cal-set="standard">${Kit.opt(STANDARDS, c.standard)}</select></label>
      <label class="check">Counts <select data-cal-set="set">${Kit.opt({ valid: 'Validation', calib: 'Calibration', all: 'All' }, c.set)}</select></label>
      <label class="check" title="${esc(coreArea(sc).label)}">Area <select data-cal-set="area">${Kit.opt(coreArea(sc).kind === 'none' ? { auto: 'All (no area of influence)' } : { auto: 'Area of influence (decides the status)', all: 'All (information)', outside: 'Outside (information)' }, c.area === 'inside' || coreArea(sc).kind === 'none' ? 'auto' : c.area)}</select></label>
      <span class="chips" role="group" aria-label="Period">${PERIODS.map(p => `<button class="btn small ${p === per ? 'primary' : ''}" data-cal="period:${p}" aria-pressed="${p === per}">${p}</button>`).join('')}</span>
      <button class="btn small" data-cal="holdout" title="Every 4th count by road class and a fixed hash becomes a validation count">Hold out 25% for validation</button>
      <button class="btn small" data-cal="exp:calibration-counts">counts_calibration.csv</button><button class="btn small" data-cal="exp:calibration-summary">calibration_summary.csv</button></div>`;
    if (!as.length) return `<div class="cal-panel">${head}<p class="note">Run the model to compare it with the counts.</p>${det('turns', 'Turning counts', turnSection(sc, per))}</div>`;
    const warn = [];
    if (evP.fallback) warn.push(`Too few independent counts (${evP.validN} validation count${evP.validN === 1 ? '' : 's'}); figures shown are calibration fit. Press <b>Hold out 25% for validation</b> before matrix estimation.`);
    for (const a of as) { if (a.ev.res.stale) warn.push(`The ${a.period} results are out of date. Run the model again before reporting.`); if (a.ev.res.converged === false) warn.push(esc(a.ev.res.convNote || `The ${a.period} assignment did not converge.`)); }
    const me = Store.P.me && Store.P.me.info; if (me && Object.values(me).some(i => i && !i.roleAware)) warn.push('Matrix estimation was run on every count (outside this panel), so no count is independent of it.');
    const m = evP.measured;
    const body = `${as.map(a => `<p class="note" style="color:${a.status === 'pass' ? 'var(--ok)' : a.status === 'fail' ? 'var(--danger)' : 'var(--warn)'};font-weight:600">${a.period}: ${esc(a.line)}</p>`).join('')}
      ${warn.map(w => `<p class="note warn">${w}</p>`).join('')}
      ${summaryTable(as)}
      <p class="note">Measured counts only (${per}, n=${m.n}): GEH &lt; 5 ${pct(m.geh5, 0)}, R² through the origin ${num(m.r2, 3)}, slope ${num(m.b0, 3)}. Area: ${esc(coreArea(sc).label)}.</p>
      <div class="row2" style="margin-top:8px"><div><div class="lg-title">GEH bands · ${esc(SET_LABEL[evP.set])}</div>${gehBar(as)}</div>
        <div><div class="lg-title">Modelled against counted · ${per}</div>${scatterSvg(evP.all.filter(r => inArea(r, evP.area)), evP.link, { log: c.scatterLog, noMw: c.scatterNoMotorway })}
        <label class="check" style="font-size:12px"><input type="checkbox" data-cal-set="scatterLog" ${c.scatterLog ? 'checked' : ''}> Log scale</label> <label class="check" style="font-size:12px"><input type="checkbox" data-cal-set="scatterNoMotorway" ${c.scatterNoMotorway ? 'checked' : ''}> Leave out motorways and ramps</label></div></div>
      ${det('worst', `Worst fits · ${per}`, worstTable(evP))}
      ${det('screen', 'Screenlines and cordons', screenlineSection(sc, per))}
      ${det('turns', 'Turning counts', turnSection(sc, per))}
      ${det('jt', 'Journey times', jtSection(sc, per, evP))}
      ${det('conv', 'Convergence', convSection(evP.res))}
      ${det('workflow', 'Calibrate in order', workflowSection(sc, per))}
      ${det('prov', 'Count roles and provenance', provenance(sc))}
      ${footnote(as[0])}`;
    return `<div class="cal-panel">${head}${body}</div>`;
  }
  // One chip row in place of the six source cards once counts are loaded
  function chips() {
    const d = D(), L = d.loaded || {}, k = DataHub.keys || {}, ch = (t, x) => `${t} ${x}`;
    return [ch('NZTA', L.nzta ? `${L.nzta.n} ✓` : '–'), ch('AT', L.at ? `${L.at.n} ✓` : '–'), ch('CAS', L.cas ? `${L.cas.n} ✓` : '–'), ch('Your counts', L.user ? `${L.user.n} ✓` : '–'), ch('TomTom', k.tomtom ? '✓' : '–'), ch('Google', k.google ? '✓' : '–')].join(' · ');
  }
  /* The Traffic data tab wrapper. Each wrapper keeps the entry it wraps in its own closure (never a shared variable),
     so a chain such as feeds(calibration(built-in)) renders each layer once. lastOrig is the entry the newest wrapper
     wraps (for dataTabHtml() called without one). */
  let lastOrig = null, lastWrap = null, depth = 0, probing = false;
  const PROBE = '<!--calibration-panel-probe-->';
  const wrappedTabs = new WeakSet();
  // The end of the element that opens at i (balanced <div> … </div>), or -1
  function divEnd(h, i) {
    const re = /<div\b|<\/div>/g; re.lastIndex = i;
    let d = 0, m;
    while ((m = re.exec(h))) { d += m[0] === '</div>' ? -1 : 1; if (d === 0) return re.lastIndex; }
    return -1;
  }
  /* The built-in tab's 'Model against counts' and journey-time block (one row2 element) is replaced by the panel;
     what comes after it (a section another feature appends, such as feeds' Data sources) is kept. */
  function dataTabHtml(origEntry = lastOrig) {
    const orig = origEntry ? Registry.html(origEntry, 'render') : '', panel = Registry.html({ id: 'calibration-panel', title: 'The calibration panel', render: panelHtml }, 'render');
    const i = orig.indexOf('<div class="lg-title">Model against counts'), j = i >= 0 ? orig.lastIndexOf('<div class="row2"', i) : -1, g = orig.indexOf('<div class="src-grid">');
    if (i < 0 || j < 0 || g < 0 || g > j) return orig + panel;
    const head = orig.slice(0, g), cards = orig.slice(g, j), e = divEnd(orig, j), tail = e > 0 ? orig.slice(e) : '';
    if (!D().counts.length) return head + cards + `<div style="margin-top:14px">${panel}</div>` + tail;
    return head + panel + `<details data-cal-sec="sources" ${openSec.sources ? 'open' : ''} style="margin-top:12px"><summary style="cursor:pointer" class="note"><b>Data sources:</b> ${chips()}</summary>${cards}</details>` + tail;
  }
  // Whether the tab already shows this panel through another feature's wrapper of ours: a probe render in which our
  // wrapper only returns a marker (so nothing heavy runs and nothing recurses)
  function holdsPanel(t) {
    if (!lastWrap) return false;
    probing = true;
    try { return String(t.render() ?? '').includes(PROBE); } catch { return false; } finally { probing = false; }
  }
  /* Upgrade the built-in Traffic data tab, keeping its own buttons and handlers. Idempotent: our own wrapper is never
     wrapped, an entry is wrapped at most once, and a tab that already shows the panel through another feature's
     wrapper (feeds re-registers the tab around ours) is left alone. A nested call (a wrapper of ours inside another)
     passes its original through, so the panel is added once. Other features' _markers are copied onto the wrapper,
     so their own idempotence checks keep working. */
  function wrapDataTab() {
    const t = Registry.get('tabs', 'data');
    if (!t || t._calibration || wrappedTabs.has(t)) return false;
    if (holdsPanel(t)) { wrappedTabs.add(t); return false; }
    const orig = t, marks = Object.fromEntries(Object.entries(t).filter(([k, v]) => /^_[a-z]/i.test(k) && k !== '_seq' && v === true));
    const render = () => {
      if (probing) return PROBE;
      if (depth) return Registry.html(orig, 'render');
      depth++;
      try { return dataTabHtml(orig); } finally { depth--; }
    };
    wrappedTabs.add(t);
    lastOrig = orig;
    lastWrap = Registry.tabs({ id: 'data', label: t.label, order: t.order, ...(t.title ? { title: t.title } : {}), ...(t.hidden ? { hidden: t.hidden } : {}), render,
      onClick: (e, body) => onClick(e, body) === true || Registry.call(orig, 'onClick', e, body) === true,
      onChange: (e, body) => onChange(e, body) === true || Registry.call(orig, 'onChange', e, body) === true,
      onInput: (e, body) => Registry.call(orig, 'onInput', e, body) === true,
      onKeydown: (e, body) => Registry.call(orig, 'onKeydown', e, body) === true, ...marks, _calibration: true });
    return true;
  }
  const redraw = () => { if (typeof Drawer !== 'undefined' && Drawer.render) Drawer.render(); };
  const toast = (t, k) => { if (typeof Kit !== 'undefined' && typeof document !== 'undefined' && document.getElementById) Kit.toast(t, k); };
  async function busyRun(text, fn) {
    Kit.busy(text);
    try { return await fn(t => Kit.busy(t)); }
    catch (e) { console.error(e); toast(`${text.replace(/…$/, '')} failed: ${e.message}`, 'err'); return null; }
    finally { Kit.busy(null); }
  }
  function onClick(e) {
    const sum = e.target.closest('summary'), dd = sum && sum.parentElement && sum.parentElement.dataset.calSec;
    if (dd) { openSec[dd] = !sum.parentElement.open; return false; }
    const b = e.target.closest('[data-cal]'); if (!b) return false;
    e.preventDefault && e.preventDefault();
    const a = b.dataset.cal, sc = Store.sc, period = Store.S.period;
    if (a.startsWith('period:')) { Store.S.period = a.slice(7); Store.emit('period'); return true; }
    if (a === 'holdout') { const r = holdOut(sc); refreshValidation(); Store.emit('data'); toast(`${r.valid} counts held out for validation, ${r.calib} kept for calibration. Matrix estimation from this panel will not use the validation counts.`); return true; }
    if (a.startsWith('exp:')) { UI.exportFile(a.slice(4)); return true; }
    if (a.startsWith('sla:')) {
      const [, id, dir] = a.split(':'), res = Store.res(), L = sc.links[id];
      if (!L) return true;
      const keys = dir === 'both' ? [id + ':ab', id + ':ba'].filter(k => res && res.arcs[k]) : [id + ':' + dir];
      const parts = keys.map(k => Engine.selectLink(res, k)).filter(Boolean);
      if (!parts.length) { toast('Run the model again to enable select-link analysis.', 'warn'); return true; }
      const s = parts[0]; for (const p of parts.slice(1)) { for (const [k, f] of Object.entries(p.flows)) s.flows[k] = (s.flows[k] || 0) + f; s.ods.push(...p.ods); s.tot += p.tot; }
      View.sla = s; UI.setColour('select');
      toast(`${U.fmt.int(s.tot / Engine.pcuF(Store.S))} veh/h use ${L.name || 'this road'}. Their full routes are shown.`);
      return true;
    }
    if (a === 'importturns') { pickTurnFile(); return true; }
    if (a === 'slsuggest') { busyRun('Finding rivers and railways…', async () => { const r = await suggestScreenlines(sc); slSuggest = { scId: sc.id, ...r }; openSec.screen = true; redraw(); toast(r.ok ? (r.list.length ? `${r.list.length} screenline suggestion${r.list.length === 1 ? '' : 's'} from rivers and railways. Add the ones you want.` : r.note) : r.note, r.ok && r.list.length ? '' : 'warn'); }); return true; }
    if (a.startsWith('sladd:')) {
      const item = slSuggest && slSuggest.scId === sc.id && slSuggest.list[+a.slice(6)], sl = addSuggested(item);
      if (sl) { memo.clear(); refreshValidation(); Store.emit('data'); toast(`${sl.name} added as a validation screenline. Change its role in the panel when you select it.`); }
      return true;
    }
    if (a === 'balance') { const r = balance(sc, period); Store.emit('data'); toast(r.sites < 2 ? 'Balancing needs two or more matched turning-count sites on the same road.' : `Balanced ${r.audit.length} movements between adjacent sites (${period}).`, r.sites < 2 ? 'warn' : ''); return true; }
    if (a === 'stress') { busyRun('Running the stress test…', async bz => { await stressTest(sc, { period, onProgress: (k, n, m) => bz(m) }); View.show['calibration-stress'] = true; Render.redraw(); UI.renderLegend && UI.renderLegend(); redraw(); toast(`Stress test done. Most sensitive junction: ${stressState.top ? stressState.top.name : 'none'}.`); }); return true; }
    if (a === 'tef') { busyRun('Fitting trip-end factors…', async bz => { const r = await tripEndFactors(sc, { period, onProgress: (k, n, m) => bz(`${m} (${k} of ${n})`) }); Store.emit('meta'); toast(r.ok ? (r.kept ? `Trip-end factors fitted: weighted error ${pct(r.rmsBefore, 1)} → ${pct(r.rmsAfter, 1)}. Review them, then apply.${r.atBound.length ? ' ' + r.note : ''}` : r.note) : r.note, r.ok && r.kept && !r.atBound.length ? '' : 'warn'); }); return true; }
    if (a === 'tefapply') { if (applyFactors(sc, period)) { refreshValidation(); Store.emit('settings'); toast(`Factors applied to existing land use and estimated gateways (${period}). Run the model again.`); } return true; }
    if (a === 'tefrevert') { if (revertFactors(sc, period)) { refreshValidation(); Store.emit('settings'); toast('Trip-end factors taken off. Run the model again.'); } return true; }
    if (a === 'me' || a === 'reduceme') {
      busyRun('Running matrix estimation…', async bz => {
        const r = a === 'me' ? await runME(sc, { period, onProgress: (k, n, m) => bz(m) }) : await reduceME(sc, { period, from: cfg().xamax, onProgress: (k, n, m) => bz(m) });
        refreshValidation();
        Store.emit('results');
        toast(r.note || (r.ok ? 'Matrix estimation done.' : 'Matrix estimation made no change.'), r.ok ? '' : 'warn');
        if (r.tried) Kit.log(`Reduce ME: ${r.tried.map(t => `XAMAX ${t.xamax} ${t.ok ? 'passes' : 'fails'} Table 5`).join(', ')}`);
      });
      return true;
    }
    if (a === 'clearme') { const n = Calibrate.clear(period); memo.clear(); if (refreshValidation()) Store.emit('meta'); toast(n ? `Cleared ${n} estimation factors for the ${period} peak. Run the model again.` : 'Nothing to clear.'); return true; }
    return false;
  }
  function setCfg(k, v) {
    const S = Store.S;
    if (k === 'category') { S.tmdg ||= {}; S.tmdg.category = v; }
    else { S.calib ||= {}; S.calib[k] = v; }
    memo.clear();
    refreshValidation();
    Store.emit('settings');
  }
  function onChange(e) {
    const t = e.target;
    if (t.dataset.calSet) { const k = t.dataset.calSet; setCfg(k, t.type === 'checkbox' ? t.checked : k === 'xamax' ? +t.value : t.value); return true; }
    if (t.dataset.calRole) { setRole(t.dataset.calRole, t.value); memo.clear(); refreshValidation(); Store.emit('data'); return true; }
    if (t.dataset.calJt) { const j = D().jt.find(x => x.id === t.dataset.calJt); if (j) { const v = +t.value; j.obs ||= {}; if (v > 0) { j.obs[Store.S.period] = v; j.obs.src = 'observed (entered)'; } else delete j.obs[Store.S.period]; Store.emit('data'); } return true; }
    return false;
  }
  function pickTurnFile() {
    const inp = document.createElement('input');
    inp.type = 'file'; inp.accept = '.csv,text/csv';
    inp.onchange = async () => {
      const f = inp.files && inp.files[0]; if (!f) return;
      try {
        const r = importTurnCSV(await f.text(), f.name);
        matchSites(Store.sc);
        Store.emit('data');
        const un = r.sites.filter(s => !s.node).length;
        toast(`Imported ${r.sites.length} turning-count site${r.sites.length === 1 ? '' : 's'}${r.skipped ? `; ${r.skipped} row${r.skipped === 1 ? '' : 's'} skipped (e.g. line ${r.skippedRows[0]})` : ''}${un ? `; ${un} not within 40 m of a junction` : ''}.${r.fromModel ? ' Read as Spare Construct model volumes (a SIDRA volumes export), not counts: the sites are not used for validation until you give them a role.' : ''}`, r.skipped || un || r.fromModel ? 'warn' : '');
      } catch (err) { toast('Could not import the turning counts: ' + err.message, 'err'); }
    };
    inp.click();
  }

  /* ================= screenline tool, layers and panels ================= */
  let draft = null;
  async function finishDraft() {
    const d = draft; draft = null; Render.redrawFx();
    if (!d || d.length < 2) return;
    const n = store().screenlines.length + 1;
    const body = `<label class="fld"><span>Name</span><input id="slName" value="Screenline ${n}"></label>
      <label class="fld"><span>Type</span><select id="slType">${Kit.opt({ screen: 'Screenline (a line across roads)', cordon: 'Cordon (a closed area)', core: 'Area of influence (closed area)' }, 'screen')}</select></label>
      <label class="fld"><span>Role</span><select id="slRole">${Kit.opt({ calib: 'Calibration', valid: 'Validation' }, 'valid')}</select></label>
      <label class="check"><input type="checkbox" id="slMw" checked> Include motorways</label>
      <p class="note">Screenlines and cordons are shared by every scenario. A closed type joins the last point to the first.</p>`;
    const v = await Kit.modal('New screenline', body, [['Cancel', null, ''], ['Save', () => ({ name: U.$('#slName').value.trim() || `Screenline ${n}`, type: U.$('#slType').value, role: U.$('#slRole').value, motorways: U.$('#slMw').checked })]]);
    if (!v) return;
    if (v.type !== 'screen' && d.length < 3) return toast('A cordon or area needs at least three points.', 'warn');
    const sl = { id: U.uid('sl'), ...v, pts: d.map(p => [U.r7(p[0]), U.r7(p[1])]) };
    store().screenlines.push(sl);
    Store.emit('data');
    UI.select({ type: 'screenline', id: sl.id });
    toast(`${sl.name} saved. ${slCrossings(Store.sc, sl).length} road crossings.`);
  }
  function drawPoly(ctx, off, geo, pts, closed) {
    ctx.beginPath();
    pts.forEach((ll, i) => { const q = geo.project(ll); i ? ctx.lineTo(q.x - off.x, q.y - off.y) : ctx.moveTo(q.x - off.x, q.y - off.y); });
    if (closed) ctx.closePath();
  }
  const TH = () => (typeof Render !== 'undefined' && Render.theme) || {};
  function drawLabel(ctx, text, x, y, col) {
    ctx.font = '600 11.5px system-ui, sans-serif'; ctx.lineWidth = 3; ctx.strokeStyle = TH().halo || '#fff'; ctx.strokeText(text, x, y); ctx.fillStyle = col; ctx.fillText(text, x, y);
  }
  function registerMap() {
    Registry.tools({ id: 'calibration-screenline', key: 'S', label: 'Screenline', group: 'analyse', order: 60, cursor: 'crosshair',
      iconSvgPath: '<path d="M4 18 L20 6"/><path d="M6 8 l3 3 M14 14 l3 3" stroke-dasharray="2 2"/>',
      hint: '<b>Screenline</b> · Click to place points across the roads; double-click or Enter to finish, Backspace removes the last point, Esc cancels.',
      enter() { draft = null; try { UI.map.doubleClickZoom.disable(); } catch { } },
      leave() { draft = null; try { UI.map.doubleClickZoom.enable(); } catch { } Render.redrawFx(); },
      click(e) { (draft ||= []).push([e.latlng.lat, e.latlng.lng]); Render.redrawFx(); },
      dbl() { finishDraft(); },
      keydown(e) {
        if (e.key === 'Escape' && draft) { draft = null; Render.redrawFx(); return true; }
        if (e.key === 'Enter' && draft) { finishDraft(); return true; }
        if (e.key === 'Backspace' && draft && draft.length) { draft.pop(); Render.redrawFx(); return true; }
        return false;
      },
      draw(ctx, off, size, geo) {
        if (!draft || !draft.length) return;
        const pts = View.cursor ? [...draft, [View.cursor.lat, View.cursor.lng]] : draft;
        ctx.strokeStyle = TH().sel || '#c2255c'; ctx.lineWidth = 3; ctx.setLineDash([6, 4]); drawPoly(ctx, off, geo, pts, false); ctx.stroke(); ctx.setLineDash([]);
      } });
    Registry.layers({ id: 'calibration-screenlines', label: 'Screenlines', order: 40, defaultOn: true,
      draw(ctx, off, size, geo) {
        const T = TH();
        for (const sl of store().screenlines) {
          if (sl.pts.length < 2) continue;
          const col = sl.type === 'core' ? (T.muted || '#888') : sl.role === 'valid' ? (T.accent || '#1971c2') : (T.amber || '#f59f00');
          ctx.strokeStyle = col; ctx.lineWidth = View.sel && View.sel.id === sl.id ? 5 : 3; ctx.setLineDash(sl.type === 'core' ? [2, 4] : sl.type === 'cordon' ? [8, 4] : []);
          drawPoly(ctx, off, geo, sl.pts, sl.type !== 'screen'); ctx.stroke(); ctx.setLineDash([]);
          if (geo.z >= 13) { const q = geo.project(sl.pts[0]); drawLabel(ctx, sl.name, q.x - off.x + 6, q.y - off.y - 6, col); }
        }
      },
      hit(X, Y, geo) {
        for (const sl of store().screenlines) {
          const P = sl.pts.map(ll => geo.project(ll)); if (sl.type !== 'screen') P.push(P[0]);
          for (let i = 0; i < P.length - 1; i++) if (U.segDist(X, Y, P[i].x, P[i].y, P[i + 1].x, P[i + 1].y).d < 6) return { type: 'screenline', id: sl.id, d: 0 };
        }
        return null;
      },
      legend: () => store().screenlines.length ? `<div class="lg-title">Screenlines</div><div class="lg-row"><span class="lg-sw" style="background:var(--accent)"></span><span>Validation</span></div><div class="lg-row"><span class="lg-sw" style="background:var(--amber)"></span><span>Calibration</span></div>` : '' });
    Registry.layers({ id: 'calibration-turn-sites', label: 'Turning-count sites', order: 41, defaultOn: true, minZoom: 12,
      draw(ctx, off, size, geo) {
        const T = TH();
        for (const s of store().turnSites) { const q = geo.project([s.lat, s.lng]), x = q.x - off.x, y = q.y - off.y; ctx.beginPath(); ctx.moveTo(x, y - 8); ctx.lineTo(x + 8, y); ctx.lineTo(x, y + 8); ctx.lineTo(x - 8, y); ctx.closePath(); ctx.fillStyle = s.node ? (T.accent || '#1971c2') : (T.amber || '#f59f00'); ctx.fill(); ctx.lineWidth = 2; ctx.strokeStyle = T.halo || '#fff'; ctx.stroke(); }
      },
      hit(X, Y, geo) { for (const s of store().turnSites) { const q = geo.project([s.lat, s.lng]); if (Math.hypot(q.x - X, q.y - Y) < 9) return { type: 'turn-site', id: s.id, d: 0 }; } return null; },
      legend: () => store().turnSites.length ? '<div class="lg-title">Turning counts</div><div class="lg-row"><span class="lg-dot" style="background:var(--accent)"></span><span>Matched to a junction</span></div><div class="lg-row"><span class="lg-dot" style="background:var(--amber)"></span><span>No junction within 40 m</span></div>' : '' });
    Registry.layers({ id: 'calibration-roles', label: 'Count roles', order: 42, defaultOn: false, minZoom: 12,
      draw(ctx, off, size, geo) {
        const T = TH(), gw = gatewayIds(), per = Store.S.period;
        for (const c of D().counts) {
          if (!DataHub.inUse(c)) continue;
          const r = c.virtual ? 'virtual' : gw.has(c.id) ? 'gateway' : roleOf(c, per), q = geo.project([c.lat, c.lng]), x = q.x - off.x, y = q.y - off.y;
          ctx.beginPath(); ctx.arc(x, y, 11, 0, Math.PI * 2); ctx.lineWidth = 2.5;
          ctx.strokeStyle = r === 'valid' ? (T.accent || '#1971c2') : r === 'gateway' ? (T.ink || '#333') : r === 'off' || r === 'virtual' ? (T.faint || '#aaa') : (T.amber || '#f59f00');
          if (r === 'gateway') ctx.setLineDash([3, 2]); ctx.stroke(); ctx.setLineDash([]);
        }
      },
      legend: () => '<div class="lg-title">Count roles</div><div class="lg-row"><span class="lg-dot" style="background:var(--accent)"></span><span>Validation</span></div><div class="lg-row"><span class="lg-dot" style="background:var(--amber)"></span><span>Calibration</span></div><div class="lg-row"><span class="lg-dot" style="background:var(--ink)"></span><span>Gateway</span></div><div class="lg-row"><span class="lg-dot" style="background:var(--faint)"></span><span>Not scored</span></div>' });
    Registry.layers({ id: 'calibration-stress', label: 'Stress test', order: 43, defaultOn: false,
      draw(ctx, off, size, geo) {
        const st = stressState; if (!st || st.scId !== Store.sc.id) return;
        const T = TH(), sc = Store.sc;
        ctx.strokeStyle = T.danger || '#e03131'; ctx.lineWidth = 6; ctx.globalAlpha = 0.6;
        for (const l of st.links) { if (!sc.links[l.L.id]) continue; drawPoly(ctx, off, geo, Net.geom(sc, sc.links[l.L.id]), false); ctx.stroke(); }
        ctx.globalAlpha = 1;
        st.nodes.filter(n => n.flag || n === st.top).forEach(n => {
          const nd = sc.nodes[n.id]; if (!nd) return;
          const q = geo.project([nd.lat, nd.lng]), x = q.x - off.x, y = q.y - off.y, top = n === st.top;
          ctx.beginPath(); ctx.arc(x, y, top ? 15 : 10, 0, Math.PI * 2); ctx.lineWidth = top ? 4 : 2.5; ctx.strokeStyle = T.danger || '#e03131'; ctx.stroke();
          if (top) drawLabel(ctx, `Most sensitive: ${n.name}`, x + 18, y + 4, T.danger || '#e03131');
        });
      },
      legend: () => stressState ? `<div class="lg-title">Stress test (${stressState.period}, × ${stressState.factors.join(' / × ')})</div><div class="lg-row"><span class="lg-dot" style="background:var(--danger)"></span><span>DoS rises more than twice the demand</span></div><div class="lg-row"><span class="lg-sw" style="background:var(--danger)"></span><span>Over capacity, no junction delay</span></div>` : '' });

    Registry.panels({ type: 'screenline', label: 'Screenline',
      render(sc, sel) {
        const sl = store().screenlines.find(s => s.id === sel.id); if (!sl) return '';
        const per = Store.S.period, rows = slRows(sc, sl, per), xs = slCrossings(sc, sl);
        return `<div class="ins-head"><div class="ins-kind">${sl.type === 'cordon' ? 'Cordon' : sl.type === 'core' ? 'Area of influence' : 'Screenline'}<span style="margin-left:auto"><button class="btn small icon" data-act="close" aria-label="Close">✕</button></span></div>
          <input class="ins-title" data-sl="name" value="${esc(sl.name)}" aria-label="Name"></div>
          <div class="ins-sec">${Kit.fld('Type', `<select data-sl="type">${Kit.opt({ screen: 'Screenline', cordon: 'Cordon', core: 'Area of influence' }, sl.type)}</select>`)}
            ${sl.type !== 'core' ? Kit.fld('Role', `<select data-sl="role">${Kit.opt({ calib: 'Calibration', valid: 'Validation' }, sl.role)}</select>`) : ''}
            ${sl.type !== 'core' ? `<label class="check"><input type="checkbox" data-sl="motorways" ${sl.motorways !== false ? 'checked' : ''}> Include motorways</label>` : '<p class="note">Statistics inside this area decide the status; outside is for information.</p>'}
            <p class="note">${xs.length} road crossings · shared by every scenario.${sl.src ? ` Line from ${esc(sl.src.source)} (${esc(sl.src.licence || '')}, ${esc(sl.src.date || '')}) ${Kit.prov(sl.src)}` : ''}</p></div>
          ${sl.type !== 'core' ? `<div class="ins-sec"><h4>${per} peak</h4>${Kit.table({ head: ['Direction', 'Counted', { label: 'ΣC', n: true }, { label: 'ΣM', n: true }, { label: 'GEH', n: true }], rows: rows.map(r => [esc(r.label), `${r.counted}/${r.n}`, U.fmt.int(r.C), U.fmt.int(r.Mc), r.ok ? Kit.gehBadge(r.geh) : esc(r.text)]), empty: Store.res(sc, per) ? 'No road crosses it.' : 'Run the model.' })}</div>` : ''}
          ${Inspector.SECTIONS}
          <div class="ins-sec actions">${sl.type === 'cordon' ? '<button class="btn small" data-sl-act="cordon">Export cordon OD ($O)</button>' : ''}<button class="btn small danger" data-sl-act="delete">Delete</button></div>`;
      },
      onChange(e) {
        const t = e.target, k = t.dataset.sl; if (!k) return false;
        const sl = store().screenlines.find(s => s.id === View.sel.id); if (!sl) return true;
        sl[k] = t.type === 'checkbox' ? t.checked : t.value.trim() || sl[k];
        memo.clear(); refreshValidation(); Store.emit('data'); return true;
      },
      onClick(e) {
        const b = e.target.closest('[data-sl-act]'); if (!b) return false;
        if (b.dataset.slAct === 'delete') this.onDelete(View.sel);
        if (b.dataset.slAct === 'cordon') UI.exportFile('calibration-cordon-o');
        return true;
      },
      onDelete(sel) { const st = store(), i = st.screenlines.findIndex(s => s.id === sel.id); if (i >= 0) { const [sl] = st.screenlines.splice(i, 1); UI.select(null); Store.emit('data'); toast(`${sl.name} deleted.`); } } });

    Registry.panels({ type: 'turn-site', label: 'Turning count',
      render(sc, sel) {
        const s = store().turnSites.find(x => x.id === sel.id); if (!s) return '';
        const per = Store.S.period, r = siteRows(sc, s, per), pk = s.peak && s.peak[per];
        return `<div class="ins-head"><div class="ins-kind">Turning count · ${esc(s.src === 'scats' ? 'SCATS detectors' : s.src === 'corridor-sidra' ? 'Spare Construct model volumes' : 'classified count')}<span style="margin-left:auto"><button class="btn small icon" data-act="close" aria-label="Close">✕</button></span></div><div style="font:600 17px/1.25 var(--f-ui)">${esc(s.name)}</div></div>
          <div class="ins-sec"><div class="facts">${Kit.fact(esc(s.date || '–'), 'count date')}${Kit.fact(hm(pk), `${per} peak hour`)}${Kit.fact(pk && pk.phf != null ? num(pk.phf, 2) : '–', 'PHF')}${Kit.fact(pk ? U.fmt.int(pk.total) : '–', 'veh/h')}</div>
            ${Kit.fld('Role', `<select data-ts="role">${Kit.opt({ valid: 'Validation', calib: 'Calibration', off: 'Not used' }, s.role)}</select>`)}
            ${(s.flags || []).concat(r.flags || []).map(f => `<p class="note warn">${esc(f)}</p>`).join('')}
            <p class="note">${r.node ? `Matched to <a href="#" data-fly="node:${esc(r.node)}">${esc(Kit.junctionName(r.node, sc))}</a>.` : esc(r.note || '')} ${r.vol && r.vol.estimated ? `Movements estimated from leg totals (Furness, ${r.vol.furness.it} iterations).` : ''}${r.vol && r.vol.balanced ? ' Balanced with adjacent sites.' : ''}</p></div>
          <div class="ins-sec"><h4>Movements · ${per}</h4>${Kit.table({ head: ['Leg', 'Turn', { label: 'Counted', n: true }, { label: 'Model', n: true }, { label: 'GEH', n: true }], rows: r.rows.map(x => [esc(x.leg + (x.arm ? ' · ' + x.arm.slice(0, 18) : '')), esc(x.mv), U.fmt.int(x.obs), x.model == null ? '–' : U.fmt.int(x.model), Kit.gehBadge(x.geh)]), empty: esc(r.note || 'No movements.') })}
            <p class="note">Legs are named by where traffic comes from. ${esc(Engine.JUNCTION_LABEL || '')}</p></div>
          ${Inspector.SECTIONS}
          <div class="ins-sec actions"><button class="btn small danger" data-ts-act="delete">Delete site</button></div>`;
      },
      onChange(e) { const t = e.target; if (t.dataset.ts !== 'role') return false; const s = store().turnSites.find(x => x.id === View.sel.id); if (s) { s.role = t.value; memo.clear(); refreshValidation(); Store.emit('data'); } return true; },
      onClick(e) { const b = e.target.closest('[data-ts-act]'); if (!b) return false; this.onDelete(View.sel); return true; },
      onDelete(sel) { const st = store(), i = st.turnSites.findIndex(s => s.id === sel.id); if (i >= 0) { st.turnSites.splice(i, 1); UI.select(null); Store.emit('data'); } } });

    Registry.inspectorSections({ id: 'calibration-count-role', types: ['count'], order: 10,
      render(sc, sel) {
        const c = D().counts.find(x => x.id === sel.id); if (!c) return '';
        const r = roleOf(c, Store.S.period), dur = c.dur ? String(c.dur) : '', single = /^\s*(24\s*h|1\s*d)/i.test(dur);
        return Kit.sec('Calibration role', `${r === 'gateway' ? '<p class="note">Used to set gateway flows, so it is a calibration count and cannot validate the model.</p>' : Kit.fld('Role', roleSel(c, Store.S.period))}
          ${Kit.table({ head: ['', 'Method', 'ME weight'], rows: PERIODS.map(p => { const m = methodOf(c, p); return [p, esc(METHODS[m].label), num(METHODS[m].weight, 1)]; }) })}
          <p class="note">${c.date ? `Counted ${esc(c.date)}. ` : ''}${dur ? `Duration ${esc(dur)}${single ? ' (single day)' : ''}. ` : ''}${nDays(c) != null ? `${esc(nDays(c))} counted days. ` : ''}95% confidence interval ${ciOf(c) ? esc(ciOf(c).text) : 'not known'}.${c.virtual ? ' A virtual site is an NZTA estimate: never scored.' : ''}</p>`);
      },
      onChange(e) { const t = e.target; if (!t.dataset.calRole) return false; setRole(t.dataset.calRole, t.value); memo.clear(); refreshValidation(); Store.emit('data'); return true; } });
    Registry.inspectorSections({ id: 'calibration-node-turns', types: ['node'], order: 30,
      render(sc, sel) {
        const s = store().turnSites.find(x => atJunction(sc, x, sel.id)); if (!s) return '';
        const r = siteRows(sc, s, Store.S.period), st = statsOf(r.rows.filter(x => x.model != null));
        return Kit.sec('Turning count', `<p class="note"><a href="#" data-cal-site="${esc(s.id)}">${esc(s.name)}</a>${s.date ? ' · ' + esc(s.date) : ''}: ${st.n ? `${st.cnt.geh5} of ${st.n} movements with GEH &lt; 5` : esc(r.note || 'no movements')}.</p>`);
      },
      onClick(e) { const a = e.target.closest('[data-cal-site]'); if (!a) return false; e.preventDefault(); UI.select({ type: 'turn-site', id: a.dataset.calSite }); return true; } });
  }

  /* ================= report sections ================= */
  const rTable = (head, rows) => `<table class="t"><tr>${head.map(h => `<th>${h}</th>`).join('')}</tr>${rows.map(r => `<tr>${r.map(c => `<td>${c ?? ''}</td>`).join('')}</tr>`).join('')}</table>`;
  const runs = (P, sc) => PERIODS.filter(p => sc && sc.res && sc.res[p]);
  // The scenario the validation chapter reports: the base scenario when it has base-year results, else another
  // scenario with base-year results, else the base scenario
  function reportScenario(P) {
    const baseYr = s => runs(P, s).some(p => s.res[p].year === P.settings.baseYear), b = P.scenarios.find(s => s.id === P.base);
    return (b && baseYr(b) ? b : null) || P.scenarios.find(baseYr) || b || P.scenarios[0];
  }
  function inProject(P, fn) { const keep = Store.P; if (P && P !== keep) Store.P = P; try { return fn(); } finally { Store.P = keep; } }
  function registerReport() {
    const R = (id, order, title, body) => Registry.report({ id, chapter: 'Model calibration and validation', order, title, render: (P, ctx = {}) => inProject(P || Store.P, () => body(P || Store.P, ctx || {})) });
    R('calibration-standard', 10, 'Validation standard', P => {
      const c = category(P.settings), S = P.settings, st = Engine.modelStatus ? Engine.modelStatus(P, S.period) : null;
      return `<h3>Validation standard</h3><p>The model is assessed against the ${esc(SOURCES.tmdg.name)} (${SOURCES.tmdg.edition}) for model purpose Category ${c} (${CAT_NAMES[c]}), with UK TAG M3.1 (May 2026) shown alongside. ${S.tmdg && S.tmdg.justification ? 'Category justification: ' + esc(S.tmdg.justification) : '<b>The category justification has not been given.</b>'}</p>
        ${st ? `<p>Model status: <b>${esc(st.label)}</b>.</p>` : ''}<p class="note">${esc(DISCLAIMER)}</p>`;
    });
    R('calibration-links', 20, 'Link and turn validation', P => {
      const sc = reportScenario(P), ps = runs(P, sc);
      if (!ps.length) return '<h3>Link validation</h3><p>No model results to report. Run the base-year model first.</p>';
      const as = ps.map(p => assess(sc, p));
      const tagOf = k => { const ts = tagSide(as, k); return ts ? `${esc(ts.t.table)}, ${esc(targetText(ts.t))}: ${ts.cells.map((c, i) => `${as[i].period} ${esc(c.status)}`).join(', ')}` : '–'; };
      const rows = as[0].rows.map((r, k) => [esc(r.t.label) + (r.t.verified === false ? ' (verify)' : ''), esc(targetText(r.t)), ...as.map(a => esc(a.rows[k].shown)), as.map(a => `${a.period}: ${esc(a.rows[k].status)}`).join('<br>'), tagOf(k), esc((SOURCES[r.t.src] || SOURCES.tmdg).short + ', ' + r.t.table)]).filter((x, k) => !(as[0].rows[k].t.group === 'me' && as.every(a => a.rows[k].status === 'No data')));
      return `<h3>Table V1 · Validation summary (${esc(sc.name)})</h3>${as.map(a => `<p><b>${a.period}:</b> ${esc(a.line)}</p>`).join('')}${rTable(['Test', 'Target', ...as.map(a => a.period), 'Status', 'TAG M3.1 (May 2026)', 'Source'], rows)}<p class="note">The TAG M3.1 column applies the UK TAG test for the same evidence (Table 2 for links and turns, Table 1 for screenlines, Table 3 for journey times). It is shown alongside and does not decide the status.</p>
        ${as.map(a => `<h4>${a.period} peak · ${esc(SET_LABEL[a.ev.set])}</h4>${a.ev.fallback ? '<p><b>Too few independent counts; figures shown are calibration fit.</b></p>' : ''}${scatterSvg(a.ev.all, a.ev.link, {})}<p>GEH bands: ${a.ev.link.gehBands.map(b => `${b.label}: ${b.n} (${pct(b.cum, 0)} cumulative)`).join(' · ')}. Measured counts only: n=${a.ev.measured.n}, GEH &lt; 5 ${pct(a.ev.measured.geh5, 0)}.</p>`).join('')}`;
    });
    R('calibration-screenlines', 30, 'Screenlines', P => {
      const sc = reportScenario(P), sls = store(P).screenlines.filter(s => s.type !== 'core');
      if (!sls.length) return '<h3>Table S1 · Screenlines</h3><p>No screenlines were defined.</p>';
      const rows = [];
      for (const sl of sls) for (const p of runs(P, sc)) for (const r of slRows(sc, sl, p)) rows.push([esc(sl.name), sl.role === 'valid' ? 'Validation' : 'Calibration', p, esc(r.label), `${r.counted}/${r.n}`, U.fmt.int(r.C), U.fmt.int(r.Mc), r.ok ? U.fmt.signedPct(r.pct, 1) : '–', r.ok ? num(r.geh, 2) : '–', esc(r.text)]);
      return `<h3>Table S1 · Screenlines</h3>${rTable(['Screenline', 'Role', 'Period', 'Direction', 'Counted arcs', 'ΣC veh/h', 'ΣM veh/h', 'Diff', 'GEH', 'Status'], rows)}<p class="note">Incomplete screenlines have no pass or fail.</p>`;
    });
    R('calibration-turns', 40, 'Turning counts', P => {
      const sc = reportScenario(P), sites = store(P).turnSites;
      if (!sites.length) return '<h3>Table T1 · Turning counts</h3><p>No turning counts were imported.</p>';
      const rows = [];
      for (const s of sites) for (const p of runs(P, sc)) { const r = siteRows(sc, s, p), st = statsOf(r.rows.filter(x => x.model != null)), pk = s.peak && s.peak[p]; rows.push([esc(s.name), esc(s.date || ''), p, hm(pk), pk && pk.phf != null ? num(pk.phf, 2) : '–', st.n ? `${st.cnt.geh5}/${st.n}` : '–', esc([...(s.flags || []), ...(r.flags || [])].join('; '))]); }
      return `<h3>Table T1 · Turning counts</h3>${rTable(['Site', 'Date', 'Period', 'Peak hour', 'PHF', 'GEH < 5', 'Flags'], rows)}<p class="note">${esc(Engine.JUNCTION_LABEL || '')}</p>`;
    });
    // Journey times (Table J1) and assignment convergence (Table C1) are printed once, in the report's Appendix B
    // (report-app-counts, report-app-convergence); they are not repeated here.
    R('calibration-me', 70, 'Matrix estimation', P => {
      const sc = reportScenario(P), info = P.me && P.me.info, tf = sc && sc.calib && sc.calib.tripEndFactors;
      let h = '<h3>Table M1 · Matrix estimation</h3>';
      if (!info || !Object.keys(info).length) h += '<p>Matrix estimation was not used.</p>';
      for (const p of PERIODS) {
        const i = info && info[p]; if (!i) continue;
        const m1 = i.m1;
        h += `<h4>${p} peak</h4>${i.roleAware ? '' : '<p><b>Estimation used every count, so no count is independent of it.</b></p>'}` + (m1 ? rTable(['Measure', 'Result', 'Target'], [
          ['Total trips', `${U.fmt.int(m1.priorTotal)} → ${U.fmt.int(m1.total)} (${U.fmt.signedPct(m1.totalPct / 100, 1)})`, 'TMDG section 8'],
          ['Cells (OLS)', `slope ${num(m1.cells.slope, 3)}, R² ${num(m1.cells.r2, 3)}, intercept ${num(m1.cells.intercept, 2)}`, 'slope 0.98–1.02, R² > 0.95 (TAG Table 5)'],
          ['Trip ends', `slope ${num(m1.ends.all.slope, 3)}, R² ${num(m1.ends.all.r2, 3)}; without gateways slope ${num(m1.ends.noGateways.slope, 3)}, R² ${num(m1.ends.noGateways.r2, 3)}`, 'slope 0.99–1.01, R² > 0.98 (TAG Table 5)'],
          ['Trip length', `mean ${U.fmt.signedPct(m1.tld.meanPct / 100, 1)}, SD ${U.fmt.signedPct(m1.tld.sdPct / 100, 1)}, CR ${num(m1.tld.cr, 3)}`, 'within 5% (TAG); CR per TMDG section 8'],
          ['Validation GEH < 5', i.validBefore && i.validAfter ? `${pct(i.validBefore.geh5, 0)} → ${pct(i.validAfter.geh5, 0)} (n=${n0(i.validAfter.n)})` : '–', 'independent counts, before and after'],
        ]) : '') + `<p class="note">On NZTA projects matrix estimation needs the peer reviewer's agreement (TMDG). ${esc(futureText(i))}</p>`;
      }
      if (tf) for (const [p, r] of Object.entries(tf)) h += `<h4>Trip-end factors (${esc(p)})</h4>${rTable(['Trip ends', 'Factor'], (r.groups || []).map(g => [luName(g), num(+(r.factors || {})[g], 3)]))}<p class="note">Fitted to ${n0(r.nObs)} calibration ${r.basis === 'screenlines' ? 'screenline totals' : 'count-group totals (road class by compass sector)'}, bounds ${(r.bounds || []).map(x => num(+x, 1)).join('–')} (Spare Construct bounds). ${r.applied ? 'Applied to the trip rates.' : 'Not applied.'}</p>`;
      return h;
    });
    R('calibration-provenance', 80, 'Count provenance', P => `<h3>Count provenance</h3>${provenance(reportScenario(P)).replace(/class="tbl-wrap"/g, '')}`);
  }

  /* ================= settings, about, exports, checklist, menu ================= */
  function registerRest() {
    Registry.settings({ id: 'calibration', title: 'Calibration and validation', order: 50, defaults: () => JSON.parse(JSON.stringify(DEFAULTS)),
      render: s => { const c = cfg(s), w = c.windows; return `<div class="set-grid">
        <label class="fld"><span>Model purpose (NZTA TMDG 2019)</span><select data-set="tmdg.category">${Kit.opt(Object.fromEntries(CATS.map(k => [k, `${k} · ${CAT_NAMES[k]}`])), category(s))}</select></label>
        <label class="fld"><span>Why this category</span><input type="text" data-set="tmdg.justification" value="${esc((s.tmdg && s.tmdg.justification) || '')}" placeholder="e.g. corridor assessment of a plan change"></label>
        <label class="fld"><span>Criteria</span><select data-set="calib.standard">${Kit.opt(STANDARDS, c.standard)}</select></label>
        <label class="fld"><span>Screenline counted share needed</span><select data-set="calib.minComplete">${Kit.opt({ 1: 'every crossing (100%)', 0.95: '95% of modelled flow' }, c.minComplete)}</select></label>
        <label class="fld"><span>Area-of-influence buffer</span><span class="fld-in"><input type="number" step="50" data-set="calib.coreBuffer" value="${esc(c.coreBuffer)}"><span class="unit">m</span></span></label>
        ${['AM', 'IP', 'PM'].map(p => `<label class="fld"><span>${p} peak-hour window</span><span class="fld-in"><input type="text" data-set="calib.windows.${p}.0" value="${esc(w[p][0])}" style="width:60px"> to <input type="text" data-set="calib.windows.${p}.1" value="${esc(w[p][1])}" style="width:60px"></span></label>`).join('')}
        </div><label class="check"><input type="checkbox" data-set="calib.freezeGG" ${cfg(s).freezeGG !== false ? 'checked' : ''}> Matrix estimation keeps through traffic (gateway to gateway) as it is</label><p class="note">Category, criteria and windows are project settings. The purpose category is not a quality ladder: choose the one that matches what the model is for.</p>`; } });
    Registry.about({ id: 'calibration', title: 'Calibration and validation criteria', order: 40, intro: 'Thresholds are stored as data with their source and a verified flag; amber “verify” marks values still to check against the source document.',
      rows: () => [
        ['Link, turn, scatter, %RMSE, screenline, journey-time and matrix criteria', `<a href="${SOURCES.tmdg.url}" target="_blank" rel="noopener">NZTA TMDG 2019</a>`, 'Categories A–G. Category A and B link GEH, the screenline 7.5 / 10 columns and most Figure 5 applicability cells are marked verify.'],
        ['Flow band, screenline, journey-time, convergence and matrix-estimation tests', `<a href="${SOURCES.tag.url}" target="_blank" rel="noopener">UK TAG M3.1 (May 2026)</a>`, 'Shown alongside for information, in its own column of the summary table and Table V1.'],
        ['RMS and ATAP presets', `<a href="${SOURCES.rms.url}" target="_blank" rel="noopener">RMS 2013</a> · <a href="${SOURCES.atap.url}" target="_blank" rel="noopener">ATAP</a>`, 'RMS is superseded; ATAP values come from secondary extracts (verify).'],
        ['Stop-line count flag (model DoS above 0.95)', `<a href="${SOURCES.ttoc.url}" target="_blank" rel="noopener">QLDC TTOC-06</a>`, 'A stop-line count understates demand at a saturated junction.'],
        ['School holidays (turning counts)', `<a href="${SCHOOL_TERMS.url}" target="_blank" rel="noopener">${SCHOOL_TERMS.source}</a>`, 'Term dates 2023–2026, to verify each year.'],
        ['Count roles, hold-out, area of influence, completeness, fewer than 10 observations (indicative), trip-end factor bounds, ME intercept and sector tests, balancing and stress-test flags', 'Spare Construct assumption', 'Agree them with the reviewing authority at scoping.'],
      ] });
    const ex = (id, label, ext, order, build, opts = {}) => Registry.exports({ id, label, ext, mime: opts.mime || 'text/csv', order, ...(opts.stamp ? { stamp: true } : {}), enabled: opts.enabled || (c => !!c.res), filename: opts.filename || (c => `${c.fileBase}-${id.replace('calibration-', '')}.${ext}`), build });
    ex('calibration-counts', 'Calibration counts (counts_calibration.csv)', 'csv', 70, (P, c) => countsCSV(c.sc), { stamp: true, filename: c => `${c.fileBase}-counts_calibration.csv` });
    ex('calibration-summary', 'Calibration summary (calibration_summary.csv)', 'csv', 71, (P, c) => summaryCSV(c.sc), { stamp: true, filename: c => `${c.fileBase}-calibration_summary.csv` });
    ex('calibration-turns', 'Turn validation (CSV)', 'csv', 72, (P, c) => turnsCSV(c.sc), { stamp: true, enabled: c => !!c.res && store().turnSites.length > 0 });
    ex('calibration-screenlines', 'Screenline validation (CSV)', 'csv', 73, (P, c) => screenlinesCSV(c.sc), { stamp: true, enabled: c => !!c.res && store().screenlines.some(s => s.type !== 'core') });
    const cordon = () => store().screenlines.find(s => s.type === 'cordon');
    const cOD = c => { const r = cordonOD(c.sc, c.res, cordon()); if (r.error) throw new Error(r.error); return r; };
    ex('calibration-cordon-o', 'Cordon OD matrix (PTV $O)', 'mtx', 74, (P, c) => cOD(c).o, { mime: 'text/plain', enabled: c => !!(c.res && c.res.paths && cordon()) });
    ex('calibration-cordon-csv', 'Cordon OD matrix (CSV)', 'csv', 75, (P, c) => cOD(c).csv, { enabled: c => !!(c.res && c.res.paths && cordon()) });
    ex('calibration-cordon-zones', 'Cordon boundary zones (GeoJSON)', 'geojson', 76, (P, c) => cOD(c).geojson, { mime: 'application/geo+json', enabled: c => !!(c.res && c.res.paths && cordon()) });
    Registry.menu({ id: 'calibration-import-turns', label: 'Import turning counts (CSV)…', order: 130, run: () => pickTurnFile() });
  }
  function checklist() {
    const has = P => !!(P.data && P.data.counts && P.data.counts.length);
    Registry.checklist({ id: 'calibration-holdout', group: 'Modelling', order: 40, label: 'Independent validation counts', source: 'TMDG 2019; TAG M3.1',
      status: P => { if (!has(P)) return { state: 'todo', text: 'Load traffic counts', action: { label: 'Open', run: () => UI.openTab('data') } }; const sc = Store.sc, n = countRows(sc, Store.S.period).filter(r => r.scored && r.role === 'valid').length;
        return n >= MIN_N ? { state: 'done', text: `${n} independent validation counts held out` } : { state: 'todo', text: `Hold out validation counts (${n} now, ${MIN_N} or more needed)`, action: { label: 'Hold out 25%', run: () => { holdOut(sc); Store.emit('data'); } } }; } });
    Registry.checklist({ id: 'calibration-validation', group: 'Modelling', order: 41, label: 'Base-year validation to TMDG', source: 'TMDG 2019',
      status: P => { const v = P.validation && P.validation[Store.S.period];
        if (v && v.pass && v.independent) return { state: 'done', text: `Validated: independent counts meet TMDG Category ${v.category} (${Store.S.period}, n=${v.n})` };
        if (v) return { state: 'warn', text: `Validation ${v.pass ? 'passes on too few independent counts' : 'fails'} TMDG Category ${v.category} (${Store.S.period})`, action: { label: 'Review', run: () => UI.openTab('data') } };
        return { state: 'todo', text: 'Validate a base-year run against independent counts', action: { label: 'Open', run: () => UI.openTab('data') } }; } });
    Registry.checklist({ id: 'calibration-category', group: 'Modelling', order: 42, label: 'Model purpose category justified', source: 'TMDG 2019',
      status: P => (P.settings.tmdg && String(P.settings.tmdg.justification || '').trim()) ? { state: 'done', text: `TMDG Category ${category(P.settings)} justified` } : { state: 'todo', text: `Justify TMDG Category ${category(P.settings)} in Settings`, action: { label: 'Settings', run: () => Dialogs.settings() } } });
  }

  /* ================= start-up ================= */
  registerMap();
  registerReport();
  registerRest();
  wrapDataTab();
  // Late parts: after UI.init (the checklist tab only appears once an item is registered)
  function registerLate() { wrapDataTab(); checklist(); }
  Registry.onInit(() => { registerLate(); try { syncCounts(); matchSites(Store.sc); } catch { } if (typeof Drawer !== 'undefined') Drawer.render(); });
  Registry.onStore(kind => {
    if (kind === 'data' || kind === 'project' || kind === 'demand') { try { syncCounts(); } catch { } }
    if (kind === 'net' || kind === 'scenario' || kind === 'project' || kind === 'undo') { try { matchSites(Store.sc); } catch { } }
    if (kind === 'settings' || kind === 'project') { try { repeak(); } catch (e) { console.error(e); } }
    // the model-status badge follows category, roles and estimation changes made anywhere (Settings, the inspector,
    // an estimation run outside this panel, which emits 'results')
    if (/^(data|settings|project|demand|net|undo|scenario|results)$/.test(kind)) { try { if (refreshValidation()) Store.emit('meta'); } catch (e) { console.error(e); } }
  });
  // After each run: keep the traced site movements (paths are not saved), then re-record validation for the scenario
  // the validation chapter reports (a base-year option run must not replace the base model's record)
  Registry.onResults((sc, period, res) => { try { siteTraces(sc, res); refreshValidation(); } catch (e) { console.error(e); } });

  return {
    SOURCES, TMDG, FIG5, CATS, CAT_NAMES, METHODS, ROLES, SCHOOL_TERMS, MIN_N, DEFAULTS, BANDS, TAG, RMS, ATAP, CORRIDOR,
    geh, tmdgBand, tagBand, need, statsOf, tests, tagPeer, evaluate, evidence, assess, targetText, tooltip, recordValidation, refreshValidation, reportScenario, siteTraces,
    roleOf, baseRole, methodOf, setRole, holdOut, resetRoles, gatewayIds, countRows, coreArea, inCore, store,
    slCrossings, slRows, slObservations, cordonOD, suggestScreenlines, addSuggested, joinWays,
    parseTurnCSV, importTurnCSV, peaks, repeak, furness, junctionView, classOf, isoDate, toMin, siteRows, siteVolumes, matchSites, balance, phf, exitArm, legArms,
    journeys, jtStats, networkChecks, stressTest, get stress() { return stressState; }, tripEndFactors, canApply, applyFactors, revertFactors,
    runME, reduceME, meReport, meStats,
    countsCSV, summaryCSV, turnsCSV, screenlinesCSV,
    panelHtml, dataTabHtml, wrapDataTab, registerLate, scatterSvg, onClick, onChange,
  };
})();
