'use strict';
/* Spare Construct · UI kit: the shared view state (View) and the HTML, table, chart, dialog, menu and
   toast helpers every panel uses (Kit). Render helpers return HTML strings and only read the
   store, so they can be tested headlessly; the dialog, menu, toast and busy helpers touch the DOM
   only when called. Also here because the report and other headless callers need them: the
   glossary, capacity source, convergence pill, settings changed from defaults (nonDefaults), the
   data-source list and walk / cycle catchments. */

// Shared view state (not saved with the project). Registered layers add View.show[layerId] at start-up.
const View = {
  tool: 'select', colorBy: 'class', sel: null, hover: null, cursor: null, snap: null, draft: null,
  show: { nodes: true, zones: true, connectors: true, routes: true, labels: true, counts: true, crashes: true, live: false },
  path: null, pathEnds: null, pathPick: [], iso: null, isoMode: 'drive', sla: null, measure: [], sim: null, simPlaying: true, simAcc: 0, simColour: 'speed',
  template: { cls: 'arterial', lanes: 1, oneway: false, speed: 50, status: 'proposed' },
  drawer: 'summary', drawerOpen: true, linkSort: { k: 'vc', dir: -1 }, linkFilter: '', linkOver: false, roadPick: null,
  // review: phone/tablet review mode (analysis tools only); riskKind: crash-risk colour mode basis
  review: false, riskKind: 'collective',
};
// Colour modes that need model results (kept for older code: RES_MODES.has(mode))
const RES_MODES = { has: k => !!(Registry.get('modes', k) || {}).needsResults };

const Kit = (() => {
  const esc = U.esc;

  /* ---------- small HTML helpers ---------- */
  const opt = (pairs, val) => Object.entries(pairs).map(([v, l]) => `<option value="${esc(v)}" ${String(v) === String(val) ? 'selected' : ''}>${esc(l)}</option>`).join('');
  const fld = (label, inner, unit = '') => `<label class="fld"><span>${label}</span><span class="fld-in">${inner}${unit ? `<span class="unit">${unit}</span>` : ''}</span></label>`;
  // Inspector inputs: data-obj/id/k/t are read by Inspector.applyField (snapshot + edited)
  const bind = (o, id, k, t = 'str') => `data-obj="${esc(o)}" data-id="${esc(id)}" data-k="${esc(k)}" data-t="${esc(t)}"`;
  const fact = (v, l) => `<div class="fact"><b>${v}</b><span>${l}</span></div>`;
  const kpi = (label, val, sub = '', cls = '') => `<div class="kpi"><span>${label}</span><b>${val}</b>${sub ? `<small class="${cls}">${sub}</small>` : ''}</div>`;
  function delta(cur, base, d = 0, lowerIsBetter = true, pct = true) {
    if (base == null || !isFinite(base)) return ['', ''];
    const dv = cur - base, p = base ? dv / base : 0;
    const txt = `${U.fmt.signed(dv, d)}${pct && base ? ` (${U.fmt.signedPct(p)})` : ''} vs base`;
    const cls = Math.abs(p) < 0.005 ? '' : (dv < 0) === lowerIsBetter ? 'down' : 'up';
    return [txt, cls];
  }
  const losBadge = l => l && l !== '–' ? `<span class="los ${esc(l)}">${esc(l)}</span>` : '<span class="los none">–</span>';
  const gehBadge = g => g == null ? '–' : `<b style="color:${g < 5 ? 'var(--ok)' : g < 10 ? 'var(--warn)' : 'var(--danger)'}">${g.toFixed(1)}</b>`;
  const pill = (text, cls = '') => `<span class="pill ${esc(cls)}">${text}</span>`;
  const compass = b => ['Northbound', 'Eastbound', 'Southbound', 'Westbound'][Math.round(((b % 360) + 360) % 360 / 90) % 4];
  // Provenance tag for a number: basis is measured | derived | estimated | assumed | user
  const prov = p => !p ? '' : `<span class="prov ${esc(p.basis || '')}" title="${esc([p.source, p.date, p.licence].filter(Boolean).join(' · '))}">${esc(p.basis || 'source')}</span>`;
  // An inspector section; body is HTML
  const sec = (title, body) => `<div class="ins-sec">${title ? `<h4>${title}</h4>` : ''}${body}</div>`;
  const note = (text, warn) => `<p class="note${warn ? ' warn' : ''}">${text}</p>`;
  function junctionName(id, sc = Store.sc) {
    const names = [...new Set(Net.linksAt(sc, id).map(l => l.name).filter(Boolean))];
    return names.length ? names.slice(0, 3).join(' / ') : 'Unnamed junction';
  }
  /* Dotted paths ('econ.rates.0.r'). Paths can come from files (a settings CSV, a crafted project), so '__proto__',
     'constructor' and 'prototype' are refused: setPath ignores the write and getPath gives undefined, and a path never
     reaches Object.prototype. A missing or empty step on the way becomes {}; a step that holds a plain value stops the write. */
  const BAD_KEY = new Set(['__proto__', 'constructor', 'prototype']);
  function setPath(o, k, v) {
    const p = String(k).split('.');
    if (!o || typeof o !== 'object' || p.some(x => BAD_KEY.has(x))) return false;
    let t = o;
    for (let i = 0; i < p.length - 1; i++) { if (!Object.prototype.hasOwnProperty.call(t, p[i]) || !t[p[i]]) t[p[i]] = {}; t = t[p[i]]; if (typeof t !== 'object') return false; }
    t[p[p.length - 1]] = v;
    return true;
  }
  function getPath(o, k) { const p = String(k).split('.'); return p.some(x => BAD_KEY.has(x)) ? undefined : p.reduce((t, x) => (t != null && Object.prototype.hasOwnProperty.call(Object(t), x) ? t[x] : undefined), o); }
  // Whole numbers above 100, d decimals below (wording: 'Round to whole numbers above 100')
  const num = (v, d = 1) => v == null || !isFinite(v) ? '–' : Math.abs(v) >= 100 ? U.fmt.int(v) : U.fmt.num(v, d);
  // A gap or other small fraction as a percentage with two significant figures: 8e-5 → '0.008%', 2e-3 → '0.20%'
  function gapPct(g) {
    if (g == null || !isFinite(g)) return '–';
    const p = g * 100;
    return (p >= 1 ? p.toFixed(1) : p >= 0.1 ? p.toFixed(2) : String(+p.toPrecision(2))) + '%';
  }

  /* ---------- glossary: every abbreviation gets a tooltip (wording, accessibility (9)) ---------- */
  const GLOSSARY = {
    'V/C': 'Volume to capacity ratio: traffic in the hour divided by what the road can carry',
    LoS: 'Level of service, from A (free flowing) to F (over capacity)',
    DoS: 'Degree of saturation: demand divided by capacity on a junction approach',
    GEH: 'GEH statistic: compares a modelled flow with a count; under 5 is a good match',
    LTS: 'Level of traffic stress for cycling, from 1 (suitable for all ages) to 4 (confident riders only)',
    pcu: 'Passenger car units: a heavy vehicle counts as more than one car',
    AADT: 'Annual average daily traffic: vehicles a day, averaged over a year',
    VKT: 'Vehicle-kilometres travelled',
    VHT: 'Vehicle-hours travelled',
    OD: 'Origin–destination: where trips start and end',
    BCR: 'Benefit–cost ratio: present value of benefits divided by costs',
    BPR: 'US Bureau of Public Roads speed-flow curve: travel time rises as traffic nears capacity',
    CAS: 'NZTA Crash Analysis System',
    DSI: 'Deaths and serious injuries',
    HCM: 'Highway Capacity Manual (US Transportation Research Board)',
    TMDG: 'NZTA Transport Model Development Guidelines (2019)',
    MBCM: 'NZTA Monetised Benefits and Costs Manual',
    IPM: 'NZTA Investment Prioritisation Method',
    ANOP: 'Auckland Network Operating Plan: target levels of service by road and mode',
    FYRR: 'First-year rate of return: the first year’s benefit divided by the cost',
    PV: 'Present value: future amounts discounted to today’s dollars',
    Q95: '95th-percentile back of queue: the queue length exceeded 5% of the time',
  };
  const gl = (term, label = term) => GLOSSARY[term] ? `<abbr title="${esc(GLOSSARY[term])}">${label}</abbr>` : label;

  /* ---------- results helpers ---------- */
  /* Why an arc's capacity is what it is (results-trust (c)): the mid-block capacity (lanes × capacity per lane)
     or the junction at its downstream end. Uses the engine's capSrc / capMid when a result carries them. */
  const SRC_NAME = { signals: 'signals', priority: 'give way', stop: 'stop', roundabout: 'roundabout' };
  function capSource(sc, res, key) {
    const r = res && res.arcs && res.arcs[key]; if (!r) return null;
    const [id, d] = key.split(':'), Lk = sc.links[id]; if (!Lk) return null;
    const lanes = (d === 'ab' ? Lk.lanesAB : Lk.lanesBA) | 0, gl1 = Lk.busLane && lanes >= 2 ? lanes - 1 : lanes;
    const mid = r.capMid != null ? Math.round(r.capMid) : Math.round(gl1 * (+Lk.capPerLane || (CLASSES[Lk.cls] || CLASSES.local).cap));
    const nodeId = r.capNode || (d === 'ab' ? Lk.b : Lk.a);
    const limited = r.capSrc ? r.capSrc !== 'mid-block' : r.cap < mid * 0.995 - 0.5;
    const capTxt = `Capacity ${U.fmt.int(r.cap)} pcu/h`;
    if (!limited) return { src: 'mid-block', mid, cap: r.cap, node: null, at: '', text: `${capTxt} · mid-block (${gl1} lane${gl1 === 1 ? '' : 's'} × ${U.fmt.int(mid / Math.max(1, gl1))} pcu/h)` };
    const site = Object.values(sc.sites || {}).find(s => s && Array.isArray(s.nodes) && s.nodes.includes(nodeId));
    const nr = res.nodes && (res.nodes[nodeId] || (site && Object.values(res.nodes).find(n => n.site === site.id)));
    const type = (nr && nr.type) || (site && site.control) || (sc.nodes[nodeId] || {}).control;
    const src = r.capSrc || SRC_NAME[type] || 'junction';
    const others = [...new Set(Net.linksAt(sc, nodeId).map(l => l.name).filter(n => n && n !== Lk.name))];
    const at = (site && site.name) || (others.length ? others.slice(0, 2).join(' / ') : junctionName(nodeId, sc));
    return { src, mid, cap: r.cap, node: nodeId, at, text: `${capTxt} · limited by ${src} at ${at} (mid-block ${U.fmt.int(mid)} pcu/h)` };
  }
  /* Convergence pill (results-trust (b)): '✓ Converged (gap 0.008%)' or the amber not-converged wording.
     Results saved before v2 carry no converged flag and show their gap only. */
  function convState(res) {
    if (!res) return null;
    const tol = res.gapTol != null ? res.gapTol : (Store.P ? Store.S.gapTol : null);
    const ok = res.converged != null ? !!res.converged : null;
    if (ok === true) return { ok, cls: 'ok', short: `✓ Converged (gap ${gapPct(res.gap)})`, text: res.convNote || `Converged: gap ${gapPct(res.gap)}, target ${gapPct(tol)}.` };
    if (ok === false) return { ok, cls: 'warn', short: `Not fully converged: gap ${gapPct(res.gap)}, target ${gapPct(tol)}`,
      text: `Not fully converged: gap ${gapPct(res.gap)}, target ${gapPct(tol)}. Volumes on busy roads may still shift by a few percent.${res.hitMax ? ` The run stopped at its limit of ${res.maxIter || res.it} iterations; raise Maximum iterations in Settings or simplify the network.` : ''}` };
    return { ok: null, cls: '', short: `Gap ${gapPct(res.gap)}`, text: `Relative gap ${gapPct(res.gap)} after ${res.it} iterations. This result was saved before Spare Construct checked convergence; run the model again for a convergence check.` };
  }
  const convPill = res => { const c = convState(res); return c ? `<span class="pill conv ${c.cls}" title="${esc(c.text)}">${esc(c.short)}</span>` : ''; };

  /* ---------- settings changed from the defaults (about-data (d), (e)) ---------- */
  // Labels for settings paths; trip rates, unit costs and registered settings are labelled from their own tables
  const SETTING_LABELS = {
    baseYear: ['Base year', ''], year: ['Default forecast year', ''], growth: ['Background growth', '% a year'], growthMode: ['Growth method', ''],
    hvPct: ['Heavy vehicles', '%'], pcuHV: ['PCU per heavy vehicle', ''], peakToDaily: ['Peak hour to daily factor', '×'],
    gravityBeta: ['Trip-length sensitivity (gravity β)', 'per min'], connectors: ['Zone access lines per zone', ''], connectorSpeed: ['Zone access line speed', 'km/h'],
    bprAlpha: ['Speed-flow curve α (BPR)', ''], bprBeta: ['Speed-flow curve β (BPR)', ''], satFlow: ['Saturation flow', 'pcu/h/lane'], cycle: ['Default signal cycle', 's'],
    lostPerPhase: ['Lost time per phase', 's'], signalOpt: ['Optimise signal timings', ''], maxIter: ['Maximum iterations', ''], gapTol: ['Convergence target (relative gap)', ''],
    'conv.stable': ['Stop only when link flows are stable (TAG M3.1)', ''], 'conv.capDamp': ['Damped junction capacities during assignment', ''],
    'conv.chg': ['Stable link: flow change under', ''], 'conv.share': ['Stable links needed', ''], 'conv.consec': ['Stable iterations in a row', ''],
    'junction.losMethod': ['Junction LoS method', ''], 'junction.pff': ['Peak flow factor', ''], 'junction.T': ['Analysis period for overflow delay', 'h'],
    'junction.queueSpace': ['Queue space per vehicle', 'm'], 'junction.f95': ['95th-percentile queue factor', ''], 'junction.capMethod': ['Junction capacity method', ''],
    'tmdg.category': ['TMDG model category', ''], 'tmdg.justification': ['TMDG category justification', ''], 'assess.msmPeakShare': ['Peak-hour share of an MSM 2-hour period', ''],
    'turns.left': ['Left-turn penalty', 's'], 'turns.right': ['Right-turn penalty', 's'], 'turns.rightSignals': ['Right-turn penalty at signals', 's'], 'turns.uturn': ['U-turns', ''],
    'econ.method': ['Discounting method', ''], 'econ.vot': ['Value of time', '$/veh·h'], 'econ.votUpdate': ['Apply the MBCM update factor to the value of time', ''],
    'econ.annualHours': ['Peak hour to year (uniform method)', 'h'], 'econ.periodHours.AM': ['AM peak hours a year', 'h'], 'econ.periodHours.PM': ['PM peak hours a year', 'h'],
    'econ.periodHours.IP': ['Inter-peak hours a year', 'h'], 'econ.rates': ['Discount rate schedule', ''], 'econ.sensRate': ['Sensitivity discount rate', '%'],
    'econ.discount': ['Discount rate (uniform method)', '%'], 'econ.years': ['Evaluation period', 'years'], 'econ.longLifeNote': ['Reason for a period over 40 years', ''],
    'econ.approval': ['Approval for a period over 60 years', ''], 'econ.benefitGrowth': ['Benefit growth', '% a year'], 'econ.p95': ['P95 cost estimate prepared', ''],
    'sim.scale': ['Simulation demand', '×'], 'sim.speed': ['Simulation speed', '×'], 'sim.maxVeh': ['Simulation vehicle limit', ''],
    'data.amShare': ['AM peak hour share of daily', '%'], 'data.pmShare': ['PM peak hour share of daily', '%'], 'data.matchRadius': ['Match counts to roads within', 'm'],
  };
  // Not assumptions: view state, keys and the settings version
  const NOT_ASSUMPTION = /^(sv|period|basemap|linzKey|display(\..*)?|sim\.speed)$/;
  function settingLabel(k) {
    if (SETTING_LABELS[k]) return SETTING_LABELS[k];
    let m = k.match(/^rates\.(\w+)\.(AM|PM)\.(rate|inPct)$/);
    if (m) { const lu = (typeof LANDUSES !== 'undefined' && LANDUSES[m[1]]) || { label: m[1], unit: '' };
      return m[3] === 'rate' ? [`Trip rate: ${lu.label}, ${m[2]} peak`, `trips per ${lu.per === 100 ? '100 m² GFA' : String(lu.unit || '').replace(/s$/, '')}`] : [`Trip rate: ${lu.label}, ${m[2]} peak, share arriving`, '%']; }
    m = k.match(/^econ\.laneKm\.(\w+)$/); if (m) return [`Cost of new ${((typeof CLASSES !== 'undefined' && CLASSES[m[1]]) || { label: m[1] }).label.toLowerCase()} road`, '$m per lane-km'];
    m = k.match(/^econ\.node\.(\w+)$/); if (m) return [`Cost of a new or upgraded ${((typeof CONTROLS !== 'undefined' && CONTROLS[m[1]]) || m[1]).toLowerCase()}`, '$m'];
    for (const d of (typeof Registry !== 'undefined' ? Registry.list('settings') : [])) {
      const def = Registry.call(d, 'defaults'); if (def && getPath(def, k) !== undefined) return [`${d.title}: ${k.split('.').pop()}`, ''];
    }
    return [k, ''];
  }
  // The defaults a project is compared with: defaultSettings() plus every registered feature's defaults
  function settingDefaults() {
    const D = typeof defaultSettings === 'function' ? defaultSettings() : {};
    return typeof Registry !== 'undefined' ? Registry.applyDefaults(D) : D;
  }
  const valText = v => v == null || v === '' ? '(blank)' : typeof v === 'boolean' ? (v ? 'on' : 'off') : Array.isArray(v) ? v.map(x => x && typeof x === 'object' ? Object.values(x).join(' ') : x).join('; ') : String(v);
  /* nonDefaults(S) → [{ key, label, unit, value, def, text }] for every setting that differs from the defaults.
     Feeds Summary › Assumptions, assumptions.csv, the GeoJSON properties and report Appendix A. */
  function nonDefaults(S = Store.S, D = settingDefaults()) {
    const out = [];
    const walk = (d, s, pre) => {
      for (const k of Object.keys(d)) {
        const key = pre ? pre + '.' + k : k;
        if (NOT_ASSUMPTION.test(key)) continue;
        const dv = d[k], sv = s ? s[k] : undefined;
        if (dv && typeof dv === 'object' && !Array.isArray(dv)) { walk(dv, sv && typeof sv === 'object' ? sv : {}, key); continue; }
        const same = Array.isArray(dv) ? JSON.stringify(dv) === JSON.stringify(sv) : (dv === sv || (dv == null && (sv == null || sv === '')) || (typeof dv === 'number' && +sv === dv && sv !== '' && sv != null));
        if (same || sv === undefined) continue;
        const [label, unit] = settingLabel(key);
        out.push({ key, label, unit, value: sv, def: dv, text: `${label}: ${valText(sv)}${unit ? ' ' + unit : ''} (default ${valText(dv)}${unit ? ' ' + unit : ''})` });
      }
    };
    if (S) walk(D, S, '');
    return out;
  }
  function assumptionsCSV(S = Store.S) {
    return U.csv([['setting', 'label', 'value', 'default', 'unit'], ...nonDefaults(S).map(a => [a.key, a.label, valText(a.value), valText(a.def), a.unit])]);
  }

  /* ---------- data sources (about-data (f)): each source with licence, query date and basis ----------
     dataSources(P) → [{ id, what, source, url, licence, date, basis, notes }]. The About the data page and the
     report's data appendix read this; feature files add their own rows through Registry.about. */
  function dataSources(P = Store.P) {
    const ld = (P && P.data && P.data.loaded) || {}, S0 = (P && P.settings) || {};
    const when = x => x && x.at ? new Date(x.at).toLocaleDateString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric' }) : null;
    const nn = x => x && x.n != null ? ` · ${U.fmt.int(x.n)} records` : '';
    return [
      { id: 'osm', what: 'Existing roads', source: 'OpenStreetMap contributors, via the Overpass API', url: 'https://www.openstreetmap.org/copyright', licence: 'ODbL 1.0', date: when(ld.osm), basis: 'measured',
        notes: 'Road class, lanes, speed limit, one-way, roundabouts, signals and cycleways from OSM tags; where a tag is missing Spare Construct uses the road-class default. Good, but not survey-grade.' + nn(ld.osm) },
      { id: 'nzta', what: 'State highway counts', source: 'NZTA open data: State highway traffic monitoring sites', url: 'https://opendata-nzta.opendata.arcgis.com/', licence: 'CC BY 4.0', date: when(ld.nzta), basis: 'measured',
        notes: 'AADT by direction for five years and heavy-vehicle share; peak hour = AADT × the peak share in Settings. Virtual sites are NZTA estimates and are left out unless included.' + nn(ld.nzta) },
      { id: 'at', what: 'Local road counts', source: 'Auckland Transport: Average Daily Traffic Counts', url: 'https://data-atgis.opendata.arcgis.com/', licence: 'AT open data terms (check before republishing)', date: when(ld.at), basis: 'measured',
        notes: 'Latest count per site with its measured peak hour, time and vehicle mix.' + nn(ld.at) },
      { id: 'cas', what: 'Crashes', source: 'NZTA Crash Analysis System (CAS)', url: 'https://opendata-nzta.opendata.arcgis.com/', licence: 'CC BY 4.0', date: when(ld.cas), basis: 'measured',
        notes: `${ld.cas && ld.cas.label ? ld.cas.label + '. ' : 'Five full years plus the year to date. '}The latest months may be incomplete.` + nn(ld.cas) },
      { id: 'user', what: 'Your counts', source: 'Your CSV file', url: '', licence: 'Yours', date: when(ld.user), basis: 'user', notes: 'SCATS, tube or survey counts you load.' + nn(ld.user) },
      { id: 'tiles', what: 'Background maps', source: 'Esri, OpenStreetMap or LINZ Basemaps tiles', url: 'https://basemaps.linz.govt.nz/', licence: 'Esri terms; OSM ODbL; LINZ CC BY 4.0', date: null, basis: 'measured', notes: 'Pictures only; the model never reads them.' },
      { id: 'rates', what: 'Trip rates', source: 'Spare Construct defaults in the range of the RTA Guide and NZ Trips Database', url: 'https://www.nzta.govt.nz/resources/research/reports/453/', licence: '', date: null, basis: 'assumed',
        notes: `Indicative, e.g. houses ${(((S0.rates || {}).residential || {}).AM || {}).rate ?? '–'} trips per dwelling in the AM peak. Replace with surveyed or NZTA Research Report 453 rates.` },
      { id: 'growth', what: 'Background growth', source: 'Settings', url: '', licence: '', date: null, basis: 'assumed', notes: `${S0.growth ?? '–'}% a year from ${S0.baseYear ?? '–'}, ${S0.growthMode === 'compound' ? 'compound' : 'linear (% of base-year traffic)'}.` },
    ];
  }

  /* ---------- hardening a project read from a file or browser storage ----------
     harden(P) makes every value the panels print the type the model gives it, so a crafted .corridor.json cannot
     put markup into the page through a number, a class name or a colour (text is escaped wherever it is shown).
     Only values of the wrong type change: numbers stored as text become numbers (or the default when they are not
     numbers at all), unknown road classes, statuses, cycling facilities and controls fall back to the default,
     route colours that are not #rgb hex become the default. Returns how many values it changed. */
  function harden(P, D = settingDefaults()) {
    if (!P || typeof P !== 'object') return 0;
    let n = 0;
    const obj = v => !!v && typeof v === 'object' && !Array.isArray(v);
    const vals = o => obj(o) ? Object.values(o).filter(obj) : [];
    const num = (o, k, d = 0, nullable = false) => {
      if (!obj(o) || !(k in o)) return;
      const v = o[k];
      if ((typeof v === 'number' && isFinite(v)) || (nullable && v == null)) return;
      const x = typeof v === 'string' && v.trim() !== '' ? +v : NaN;
      o[k] = isFinite(x) ? x : nullable ? null : d; n++;
    };
    const nums = (o, keys, d = 0, nullable = false) => { for (const k of keys) num(o, k, d, nullable); };
    const str = (o, ...keys) => { if (obj(o)) for (const k of keys) if (k in o && o[k] != null && typeof o[k] !== 'string') { o[k] = typeof o[k] === 'object' ? '' : String(o[k]); n++; } };
    const oneOf = (o, k, set, d) => { if (Object.keys(set).length && obj(o) && k in o && o[k] != null && !(typeof o[k] === 'string' && Object.prototype.hasOwnProperty.call(set, o[k]))) { o[k] = d; n++; } };
    const bool = (o, k) => { if (obj(o) && k in o && o[k] != null && typeof o[k] !== 'boolean') { o[k] = o[k] === true || o[k] === 1 || o[k] === 'true'; n++; } };
    const numList = (o, k) => { if (obj(o) && Array.isArray(o[k])) o[k] = o[k].map(x => { if (x == null || (typeof x === 'number' && isFinite(x))) return x; n++; return isFinite(+x) && x !== '' ? +x : null; }); };
    const CL = typeof CLASSES !== 'undefined' ? CLASSES : {}, ST = typeof STATUS !== 'undefined' ? STATUS : {};
    const CY = typeof CYCLE !== 'undefined' ? CYCLE : {}, CT = typeof CONTROLS !== 'undefined' ? CONTROLS : {};
    // settings: the type of each default decides (a text default accepts any plain value, e.g. U-turns 'ban' or 30)
    const walk = (d, s) => {
      for (const k of Object.keys(d)) {
        if (!(k in s) || s[k] == null) continue;
        const dv = d[k], sv = s[k];
        if (obj(dv)) { if (obj(sv)) walk(dv, sv); else { s[k] = JSON.parse(JSON.stringify(dv)); n++; } }
        else if (Array.isArray(dv)) {
          if (!Array.isArray(sv)) { s[k] = JSON.parse(JSON.stringify(dv)); n++; }
          else if (obj(dv[0])) sv.forEach((x, i) => { if (obj(x)) walk(dv[Math.min(i, dv.length - 1)], x); else { sv[i] = JSON.parse(JSON.stringify(dv[Math.min(i, dv.length - 1)])); n++; } });
        }
        else if (typeof dv === 'number') num(s, k, dv);
        else if (typeof dv === 'boolean') bool(s, k);
        else if (typeof dv === 'string' && typeof sv === 'object') { s[k] = dv; n++; }
      }
    };
    const PERIODS = { AM: 1, PM: 1, IP: 1 };
    if (obj(P.settings) && obj(D)) { walk(D, P.settings); oneOf(P.settings, 'period', PERIODS, 'AM'); }
    str(P, 'name', 'id');
    for (const sc of Array.isArray(P.scenarios) ? P.scenarios.filter(obj) : []) {
      str(sc, 'name', 'notes', 'id');
      for (const L of vals(sc.links)) {
        str(L, 'name', 'speedSource', 'a', 'b');
        oneOf(L, 'cls', CL, 'local'); oneOf(L, 'status', ST, 'existing'); oneOf(L, 'cycle', CY, 'none');
        nums(L, ['lanesAB', 'lanesBA'], 1); num(L, 'speed', 50); nums(L, ['capPerLane', 'speedLimit'], null, true); bool(L, 'busLane');
      }
      for (const N of vals(sc.nodes)) { str(N, 'name'); nums(N, ['lat', 'lng']); oneOf(N, 'control', CT, 'none'); num(N, 'cycle', null, true); bool(N, 'fixedTiming'); bool(N, 'upgrade'); }
      for (const z of vals(sc.zones)) {
        str(z, 'name', 'kind', 'role', 'gwSrc', 'node'); nums(z, ['lat', 'lng']); num(z, 'connectors', null, true);
        for (const k of obj(z.lu) ? Object.keys(z.lu) : []) num(z.lu, k);
        for (const k of obj(z.gw) ? Object.keys(z.gw) : []) num(z.gw, k);
      }
      for (const r of vals(sc.routes)) { str(r, 'name'); num(r, 'freq'); if (r.color != null && !/^#[0-9a-f]{3,8}$/i.test(String(r.color))) { r.color = '#1971c2'; n++; } }
      for (const r of vals(sc.res)) {
        nums(r, ['it', 'ms', 'gap', 'year']); nums(r, ['gapTol', 'maxIter'], null, true); str(r, 'note', 'convNote'); oneOf(r, 'period', PERIODS, 'AM'); bool(r, 'stale');
        if (obj(r.kpi)) { for (const k of Object.keys(r.kpi)) if (typeof r.kpi[k] === 'string') num(r.kpi, k); if (obj(r.kpi.worst)) { str(r.kpi.worst, 'id', 'los'); num(r.kpi.worst, 'delay'); } }
        for (const a of vals(r.arcs)) { nums(a, ['v', 'cap', 'vc', 'spd', 't', 'ff', 'len', 'nd', 'pcu']); num(a, 'capMid', null, true); str(a, 'los', 'capSrc', 'capNode'); }
        for (const j of vals(r.nodes)) {
          nums(j, ['delay', 'dos', 'vol']); num(j, 'C', null, true); oneOf(j, 'type', CT, 'priority'); str(j, 'los', 'losText', 'delayBasis', 'site', 'intersectionLos'); numList(j, 'split');
          if (j.appr != null && !Array.isArray(j.appr)) { j.appr = []; n++; }
          for (const a of (j.appr || []).filter(obj)) { nums(a, ['v', 'dos', 'delay', 'q95', 'cap', 'inB']); str(a, 'los', 'approach', 'name', 'dosBand'); }
        }
        for (const q of vals(r.routes)) nums(q, ['t', 'len', 'spd']);
      }
    }
    const d = P.data;
    if (obj(d)) {
      const SRC = (typeof DataHub !== 'undefined' && DataHub.SOURCES) || {};
      for (const c of Array.isArray(d.counts) ? d.counts.filter(obj) : []) {
        str(c, 'id', 'name', 'dir', 'date', 'peakTime', 'note', 'ref'); nums(c, ['lat', 'lng']); nums(c, ['adt', 'hv', 'peak'], null, true); numList(c, 'hist');
        if (Object.keys(SRC).length) oneOf(c, 'src', SRC, 'user');
      }
      if (obj(d.loaded)) for (const x of vals(d.loaded)) { nums(x, ['at', 'n', 'from', 'to', 'ytd', 'years'], null, true); str(x, 'label'); }
    }
    return n;
  }
  // Phone and tablet portrait layout: the same test as the CSS breakpoint (max-width: 820px, inclusive)
  function narrow() {
    try { if (typeof matchMedia === 'function') return matchMedia('(max-width: 820px)').matches; } catch { }
    return typeof window !== 'undefined' && window.innerWidth <= 820;
  }
  // The crash window loaded from CAS, e.g. '2021–2025 (5 full years), plus 2026 to date (YTD, incomplete)'
  function casText(P = typeof Store !== 'undefined' ? Store.P : null) {
    const c = P && P.data && P.data.loaded && P.data.loaded.cas;
    if (c && c.label) return String(c.label);
    if (c && c.from && c.to) return `${c.from}–${c.to}${c.ytd ? `, plus ${c.ytd} to date` : ''}`;
    return typeof DataHub !== 'undefined' && DataHub.casWindow ? DataHub.casWindow().label : 'the last five full years, plus the year to date';
  }
  // The same window, short, for keys and cards: '2021–2025, plus 2026 to date'
  function casShort(P = typeof Store !== 'undefined' ? Store.P : null) {
    const c = P && P.data && P.data.loaded && P.data.loaded.cas;
    return c && c.from && c.to ? `${c.from}–${c.to}${c.ytd ? `, plus ${c.ytd} to date` : ''}` : casText(P);
  }
  // Junction results' honest label (rule 2): the result's LoS method, then 'HCM 6 / Akcelik-based, indicative; not SIDRA output'
  function junctionLabel(res) {
    const own = typeof Engine !== 'undefined' && Engine.JUNCTION_LABEL || 'HCM 6 / Akcelik-based, indicative; not SIDRA output';
    return [res && res.junction && res.junction.label, own].filter(Boolean).map(s => String(s).trim().replace(/\.+$/, '')).filter((s, i, a) => a.indexOf(s) === i).join('. ') + '.';
  }
  /* Junction approaches named by the leg traffic comes FROM, as SIDRA does (nz-conventions): a northbound approach is
     'South: Great South Road'. legLabels(sc, res, nodeId) → Map(arriving arc key → label), from Turns' leg coding;
     empty when Turns is not loaded. */
  function legLabels(sc, res, nodeId) {
    const out = new Map();
    if (typeof Turns === 'undefined' || !Turns.arms || !Turns.assignLegs || !Turns.legLabel || !sc || !sc.nodes || !sc.nodes[nodeId]) return out;
    try {
      const A = Turns.arms(sc, nodeId, (res && res.turns && res.turns[nodeId]) || []);
      const pr = A.map(a => (a.conn ? 0 : a.link ? (CLASSES[a.link.cls] || CLASSES.local).rank : 1));
      Turns.assignLegs(A.map(a => a.b), pr).forEach((c, i) => { if (c && A[i].inKey) out.set(A[i].inKey, Turns.legLabel(c.o, A[i].road)); });
    } catch (e) { console.warn(e); }
    return out;
  }
  // The LINZ Basemaps key: kept in this browser only (DataHub.keys), never in the project file
  const linzKey = () => (typeof DataHub !== 'undefined' && DataHub.keys && DataHub.keys.linz) || '';
  /* Write the open project's pending autosave now, before another project replaces it (opening a file, New project).
     Uses Store.saveSoon.flush() when the store offers it, otherwise saves straight away. Never throws. */
  async function flushSave() {
    try {
      const f = Store.saveSoon && Store.saveSoon.flush;
      return await (typeof f === 'function' ? f() : Store.P ? Store.saveNow() : false);
    } catch (e) { console.warn(e); return false; }
  }

  /* ---------- walk and cycle catchments (core colour mode) ----------
     catchment(sc, fromId, 'walk' | 'cycle') → { from, times Map(nodeId → s), mode, basis }. Spare Construct assumption:
     walking at 4.8 km/h on any road except motorways and ramps, both directions; cycling at 15 km/h on the
     same roads, respecting one-way streets. No footpath, crossing or gradient data. */
  const CATCH = { walk: { kmh: 4.8, oneway: false, label: 'Walk' }, cycle: { kmh: 15, oneway: true, label: 'Cycle' } };
  function catchment(sc, fromId, mode = 'walk') {
    const m = CATCH[mode] || CATCH.walk, v = m.kmh / 3.6, ids = Object.keys(sc.nodes), ix = new Map(ids.map((id, i) => [id, i])), adj = ids.map(() => []);
    for (const Lk of Object.values(sc.links)) {
      const C = CLASSES[Lk.cls] || CLASSES.local, a = ix.get(Lk.a), b = ix.get(Lk.b);
      if (C.rank >= 5 || Lk.status === 'closed' || a == null || b == null) continue;
      const t = Net.len(sc, Lk) / v;
      if (!m.oneway || Lk.lanesAB > 0) adj[a].push([b, t]);
      if (!m.oneway || Lk.lanesBA > 0) adj[b].push([a, t]);
    }
    const best = new Float64Array(ids.length).fill(Infinity), s = ix.get(fromId), heap = new U.Heap();
    if (s != null) { best[s] = 0; heap.push(0, s); }
    while (heap.n) {
      const i = heap.pop(), t = heap.top;
      if (t > best[i]) continue;
      for (const [j, w] of adj[i]) if (t + w < best[j]) { best[j] = t + w; heap.push(t + w, j); }
    }
    const times = new Map();
    best.forEach((t, i) => { if (isFinite(t)) times.set(ids[i], t); });
    return { from: fromId, times, mode, basis: `Spare Construct assumption: ${m.label.toLowerCase()} at ${m.kmh} km/h on every road except motorways and ramps${m.oneway ? ', obeying one-way streets' : ''}; no footpath, crossing or gradient data.` };
  }

  /* ---------- tables ---------- */
  /* table({ head: ['Road', {label: 'veh/h', n: true, sort: 'v', sorted: 1 | -1}], rows: [[cells…] | {cells, fly: 'link:id', flyCol, cls}], compact: true, cls })
     Cells are HTML; a header with n: true right-aligns its column in a monospace font. A row with fly gets a button in its
     first cell (or column flyCol), so it can be reached by keyboard; a header with sort is a button with aria-sort. */
  function table({ head = [], rows = [], compact = true, cls = '', empty = '' } = {}) {
    const H = head.map(h => typeof h === 'string' ? { label: h } : h);
    const thc = h => [h.n ? 'n' : '', h.sort ? 'sortable' : ''].filter(Boolean).join(' ');
    const th = H.length ? `<tr>${H.map(h => `<th${thc(h) ? ` class="${thc(h)}"` : ''}${h.title ? ` title="${esc(h.title)}"` : ''}${h.sort ? ` aria-sort="${h.sorted > 0 ? 'ascending' : h.sorted < 0 ? 'descending' : 'none'}"` : ''}>${h.sort ? `<button type="button" class="sorth" data-sort="${esc(h.sort)}">${h.label}${h.sorted > 0 ? ' ▲' : h.sorted < 0 ? ' ▼' : ''}</button>` : h.label}</th>`).join('')}</tr>` : '';
    const tr = rows.map(r => {
      const o = Array.isArray(r) ? { cells: r } : r;
      const attrs = `${o.fly || o.cls ? ` class="${o.fly ? 'click ' : ''}${esc(o.cls || '')}"` : ''}${o.fly ? ` data-fly="${esc(o.fly)}"` : ''}`;
      return `<tr${attrs}>${o.cells.map((c, i) => `<td${H[i] && H[i].n ? ' class="n"' : ''}>${o.fly && i === (o.flyCol || 0) ? rowlink(c) : c ?? ''}</td>`).join('')}</tr>`;
    }).join('');
    const body = tr || (empty ? `<tr><td class="note" colspan="${Math.max(1, H.length)}">${empty}</td></tr>` : '');
    return `<div class="tbl-wrap"><table class="t${compact ? ' compact' : ''}${cls ? ' ' + esc(cls) : ''}">${th}${body}</table></div>`;
  }
  // The first cell of a clickable row: a button, so the row can be reached and opened by keyboard
  const rowlink = c => `<button type="button" class="rowlink">${c ?? ''}</button>`;

  /* ---------- SVG charts (CSS-token colours so both themes work) ---------- */
  // Horizontal bars: items [{ label, value (null = not run), text, colour }]
  function bar(items, { width = 520, barH = 22, labelW = 150, caption = '', max } = {}) {
    const mx = max || Math.max(1, ...items.map(i => i.value).filter(v => v != null)), H = items.length * (barH + 8) + 22;
    return `<svg class="mini-chart" viewBox="0 0 ${width} ${H}" style="height:${items.length * 30 + 22}px" role="img" aria-label="${esc(caption || 'Bar chart')}">
      ${items.map((it, i) => { const v = it.value, w = v == null ? 0 : (width - labelW - 120) * v / mx, y = i * (barH + 8) + 4;
        return `<text x="0" y="${y + 15}">${esc(String(it.label).slice(0, 22))}</text><rect x="${labelW}" y="${y}" width="${Math.max(1, w)}" height="${barH}" rx="3" fill="${it.colour || 'var(--accent)'}"/><text x="${labelW + 6 + w}" y="${y + 15}">${v == null ? 'not run' : it.text ?? U.fmt.num(v, 1)}</text>`; }).join('')}
      ${caption ? `<text x="${labelW}" y="${items.length * (barH + 8) + 18}">${caption}</text>` : ''}</svg>`;
  }
  /* Time series: series [{ pts: [[x, y]…], yMax, colour, width, dash, label }]; xMax; yLabel (top-left value);
     xLabels [left, right]. Each series may use its own yMax (e.g. speed on 0–80). */
  function line(series, { width = 560, height = 140, xMax = 1, yMax = 1, yLabel = '', xLabels = ['', ''], cssHeight = 150, legend = true } = {}) {
    const W = width, H = height, px = x => 40 + (W - 50) * x / xMax;
    const paths = series.map(s => { const ym = s.yMax || yMax, py = y => H - 22 - (H - 34) * y / ym;
      return `<path d="${s.pts.map((p, i) => `${i ? 'L' : 'M'}${px(p[0]).toFixed(1)},${py(p[1]).toFixed(1)}`).join('')}" fill="none" stroke="${s.colour}" stroke-width="${s.width || 1.6}"${s.dash ? ` stroke-dasharray="${s.dash}"` : ''}/>`; }).join('');
    return `<svg class="mini-chart" viewBox="0 0 ${W} ${H}" preserveAspectRatio="none" style="height:${cssHeight}px">
        <line x1="40" y1="${H - 22}" x2="${W - 10}" y2="${H - 22}" stroke="var(--line)"/><line x1="40" y1="12" x2="${W - 10}" y2="12" stroke="var(--line-2)"/>
        ${paths}
        <text x="36" y="16" text-anchor="end">${yLabel}</text><text x="36" y="${H - 20}" text-anchor="end">0</text>
        <text x="40" y="${H - 6}">${xLabels[0]}</text><text x="${W - 10}" y="${H - 6}" text-anchor="end">${xLabels[1]}</text></svg>` +
      (legend ? `<div class="hud-row" style="font-size:12px;gap:14px">${series.filter(s => s.label).map(s => `<span style="color:${s.colour}">${s.dash ? '┅' : '━'} ${s.label}</span>`).join('')}</div>` : '');
  }
  // Scatter with a 1:1 line: points [{ x, y, colour, title, fly }]
  function scatter(points, { width = 250, height = 190, max, xLabel = '', yLabel = '', diagonal = true } = {}) {
    const W = width, H = height, P = 34, mx = max || Math.max(100, ...points.map(p => Math.max(p.x, p.y))) * 1.05;
    const X = v => P + (W - P - 8) * v / mx, Y = v => H - 22 - (H - 30) * v / mx;
    const pts = points.map(p => `<circle cx="${X(p.x).toFixed(1)}" cy="${Y(p.y).toFixed(1)}" r="4" fill="${p.colour || 'var(--accent)'}" fill-opacity=".8" stroke="var(--panel)"${p.fly ? ` data-fly="${esc(p.fly)}" style="cursor:pointer"` : ''}>${p.title ? `<title>${esc(p.title)}</title>` : ''}</circle>`).join('');
    return `<svg class="mini-chart" viewBox="0 0 ${W} ${H}" style="height:${H}px;max-width:${W + 40}px">
        ${diagonal ? `<line x1="${X(0)}" y1="${Y(0)}" x2="${X(mx)}" y2="${Y(mx)}" stroke="var(--faint)" stroke-dasharray="4 3"/>` : ''}
        <line x1="${P}" y1="${H - 22}" x2="${W - 8}" y2="${H - 22}" stroke="var(--line)"/><line x1="${P}" y1="8" x2="${P}" y2="${H - 22}" stroke="var(--line)"/>
        ${pts}<text x="${W - 8}" y="${H - 8}" text-anchor="end">${xLabel} → ${U.fmt.int(mx)}</text><text x="${P - 4}" y="14" text-anchor="end">${yLabel}</text><text x="${P - 4}" y="${H - 22}" text-anchor="end">0</text></svg>`;
  }

  /* ---------- undo labels ----------
     snap(label) takes an undo snapshot that names the change ('lanes on Waihoehoe Road'); the shell shows
     'Undid: lanes on Waihoehoe Road (Option A)'. snapSettings(label) does the same for the project settings. */
  function snap(label) {
    Store.snapshot();
    const u = Store.undoStack[Store.undoStack.length - 1];
    if (u && label) u.label = label;
  }
  function snapSettings(label) {
    Store.undoStack.push({ sid: 'settings', label: label || 'settings', data: JSON.stringify(Store.P.settings) });
    if (Store.undoStack.length > 60) Store.undoStack.shift();
    Store.redoStack.length = 0;
  }

  /* ---------- dialogs ----------
     modal(titleHtml, bodyHtml, [[label, value | () => value, cls]…], wide | { wide, dismiss }) → Promise of the
     chosen value (null on Esc). Labelled by its heading, keeps keyboard focus inside while open and gives it back
     to whatever had it before. dismiss: false keeps a backdrop click from closing it (the welcome dialog). */
  // Without a DOM (the headless test harness) dialogs resolve null and menus, toasts and busy do nothing
  const hasDom = () => typeof document !== 'undefined' && typeof document.querySelector === 'function';
  let modalResolve = null, modalBound = false, opener = null;
  const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
  function modal(title, body, buttons = [['Close', null]], wide = false) {
    const o = wide && typeof wide === 'object' ? wide : { wide: !!wide };
    if (!hasDom()) return Promise.resolve(null);
    const m = U.$('#modal'), back = U.$('#modalBack');
    if (!modalBound) {
      modalBound = true;
      back.addEventListener('mousedown', e => { if (e.target.id === 'modalBack' && back.dataset.dismiss !== '0') closeModal(null); });
      m.addEventListener('keydown', e => {
        if (e.key !== 'Tab') return;
        const f = [...m.querySelectorAll(FOCUSABLE)].filter(x => x.offsetParent !== null || x === document.activeElement);
        if (!f.length) return;
        const first = f[0], last = f[f.length - 1];
        if (e.shiftKey && (document.activeElement === first || !m.contains(document.activeElement))) { e.preventDefault(); last.focus(); }
        else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      });
    }
    if (modalResolve) { const r = modalResolve; modalResolve = null; r(null); }
    else if (back.hidden) opener = document.activeElement;
    m.className = o.wide ? 'wide' : '';
    m.setAttribute('aria-labelledby', 'modalTitle');
    back.dataset.dismiss = o.dismiss === false ? '0' : '1';
    m.innerHTML = `<div class="m-head"><h2 id="modalTitle">${title}</h2></div><div class="m-body">${body}</div><div class="m-foot">${buttons.map(([l, v, cls], i) => `<button class="btn ${cls || (i === buttons.length - 1 ? 'primary' : '')}" data-mv="${i}">${l}</button>`).join('')}</div>`;
    back.hidden = false;
    m.querySelector('.m-foot').onclick = e => { const b = e.target.closest('[data-mv]'); if (b) closeModal(buttons[+b.dataset.mv][1]); };
    setTimeout(() => { const f = m.querySelector('.m-body input, .m-body select, .m-foot .primary'); f && f.focus(); }, 30);
    return new Promise(r => { modalResolve = r; });
  }
  function closeModal(v) {
    if (hasDom()) U.$('#modalBack').hidden = true;
    const r = modalResolve; modalResolve = null;
    const back = opener; opener = null;
    if (back && back.focus && document.contains(back)) { try { back.focus(); } catch { } }
    if (r) r(typeof v === 'function' ? v() : v);
  }
  const modalOpen = () => hasDom() && !U.$('#modalBack').hidden;
  async function ask(title, value) {
    const p = modal(esc(title), `<input id="askIn" value="${esc(value)}" style="width:100%" aria-label="${esc(title)}">`, [['Cancel', null, ''], ['Save', () => U.$('#askIn').value.trim()]]);
    // without a page (headless) the modal has already resolved null: nothing to focus
    if (hasDom()) setTimeout(() => { const i = U.$('#askIn'); if (i) { i.select(); i.onkeydown = e => { if (e.key === 'Enter') closeModal(i.value.trim()); }; } }, 40);
    return p;
  }
  const confirmBox = (title, text, yes) => modal(esc(title), `<p>${esc(text)}</p>`, [['Cancel', false, ''], [yes, true, 'primary danger']]);

  /* ---------- menus ----------
     Items: [label, fn, cls, kbd] | { label, run, danger, keys, sub: [items], disabled } | null (separator).
     Keyboard: arrows move, Home/End, Right opens a submenu, Left goes back, Esc closes. */
  let menuAnchor = null;
  function menu(anchor, items, { focus = true } = {}) {
    if (!hasDom()) return;
    const m = U.$('#menu'), r = anchor.getBoundingClientRect();
    menuAnchor = anchor;
    const norm = it => !it ? null : Array.isArray(it) ? { label: it[0], run: it[1], danger: it[2] === 'danger', cls: it[2] || '', keys: it[3] } : it;
    const stack = [];
    const show = list => {
      const L = list.map(norm);
      m.innerHTML = (stack.length ? `<button role="menuitem" data-back="1">‹ Back</button><hr>` : '') +
        L.map((it, i) => it ? `<button role="menuitem" data-mi="${i}" class="${esc(it.cls || (it.danger ? 'danger' : ''))}"${it.disabled ? ' disabled' : ''}${it.sub ? ' aria-haspopup="menu"' : ''}${it.title ? ` title="${esc(it.title)}"` : ''}>${esc(it.label)}${it.keys ? `<kbd>${esc(it.keys)}</kbd>` : it.sub ? '<kbd>▸</kbd>' : ''}</button>` : '<hr role="separator">').join('');
      m.onclick = e => {
        if (e.target.closest('[data-back]')) { show(stack.pop()); focusItem(0); return; }
        const b = e.target.closest('[data-mi]'); if (!b) return;
        const it = L[+b.dataset.mi];
        if (it.sub) { stack.push(list); show(it.sub); focusItem(stack.length ? 1 : 0); return; }
        hideMenu(); it.run && it.run();
      };
      m.onkeydown = e => {
        const btns = [...m.querySelectorAll('button:not([disabled])')], i = btns.indexOf(document.activeElement);
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); btns[(i + (e.key === 'ArrowDown' ? 1 : btns.length - 1)) % btns.length]?.focus(); }
        else if (e.key === 'Home' || e.key === 'End') { e.preventDefault(); btns[e.key === 'Home' ? 0 : btns.length - 1]?.focus(); }
        else if (e.key === 'ArrowRight' && document.activeElement?.getAttribute('aria-haspopup')) { e.preventDefault(); document.activeElement.click(); }
        else if (e.key === 'ArrowLeft' && stack.length) { e.preventDefault(); show(stack.pop()); focusItem(0); }
        else if (e.key === 'Tab') hideMenu();
      };
    };
    const focusItem = i => { if (!focus) return; const btns = m.querySelectorAll('button:not([disabled])'); (btns[i] || btns[0])?.focus(); };
    m.setAttribute('role', 'menu');
    show(items);
    m.hidden = false;
    const w = m.offsetWidth;
    m.style.left = Math.max(8, Math.min(r.left, window.innerWidth - w - 8)) + 'px'; m.style.top = (r.bottom + 4) + 'px';
    m.style.maxHeight = `calc(100vh - ${Math.round(r.bottom + 12)}px)`;
    anchor.setAttribute && anchor.setAttribute('aria-expanded', 'true');
    focusItem(0);
    setTimeout(() => document.addEventListener('mousedown', outside), 0);
    function outside(e) { if (!m.contains(e.target)) { hideMenu(false); } document.removeEventListener('mousedown', outside); }
  }
  function hideMenu(refocus = true) {
    if (!hasDom()) return;
    const m = U.$('#menu'); if (m.hidden) return;
    m.hidden = true;
    if (menuAnchor) { menuAnchor.setAttribute && menuAnchor.setAttribute('aria-expanded', 'false'); if (refocus && m.contains(document.activeElement)) menuAnchor.focus(); }
    if (refocus && menuAnchor && document.activeElement === document.body) menuAnchor.focus();
  }
  const menuOpen = () => hasDom() && !U.$('#menu').hidden;

  /* ---------- toasts, alerts, busy, log ----------
     toast(text, '' | 'warn' | 'err', { action: { label, run } | [..], persist, details }). Plain toasts go to the
     polite #toasts live region and fade; warnings and errors go to #alerts (role=alert) and stay until dismissed.
     details adds a 'Copy details' button (the error stack, for a bug report). */
  function toast(msg, kind = '', o = {}) {
    if (kind && typeof kind === 'object') { o = kind; kind = o.kind || ''; }
    const persist = o.persist ?? (kind === 'err' || kind === 'warn');
    if (!hasDom()) return null;
    const host = U.$(persist ? '#alerts' : '#toasts') || U.$('#toasts');
    if (!host) return null;
    for (const old of [...host.children]) if (old.dataset.msg === msg) old.remove();
    const el = document.createElement('div');
    el.className = 'toast ' + kind; el.dataset.msg = msg;
    const acts = [].concat(o.action || []).filter(Boolean);
    if (o.details) acts.push({ label: 'Copy details', run: () => copyText(o.details).then(ok => toast(ok ? 'Details copied. Paste them into your bug report.' : 'Could not copy. Select the text in the Run log instead.', ok ? '' : 'warn')), keep: true });
    const span = document.createElement('span'); span.className = 't-msg'; span.textContent = msg; el.appendChild(span);
    const close = () => el.remove();
    for (const a of acts) {
      const b = document.createElement('button'); b.className = 'btn small t-act'; b.textContent = a.label;
      b.onclick = () => { try { a.run(); } finally { if (!a.keep) close(); } };
      el.appendChild(b);
    }
    if (persist) { const x = document.createElement('button'); x.className = 'btn small icon t-x'; x.setAttribute('aria-label', 'Dismiss'); x.title = 'Dismiss'; x.textContent = '✕'; x.onclick = close; el.appendChild(x); }
    if (acts.length) el.classList.add('has-act');
    host.appendChild(el);
    if (persist) { while (host.children.length > 4) host.firstElementChild.remove(); }
    else setTimeout(close, acts.length ? 9000 : 4200);
    return el;
  }
  async function copyText(t) {
    try { await navigator.clipboard.writeText(t); return true; }
    catch { try { const a = document.createElement('textarea'); a.value = t; document.body.appendChild(a); a.select(); const ok = document.execCommand('copy'); a.remove(); return ok; } catch { return false; } }
  }
  /* busy(text | null, { cancel }) shows the working overlay with the elapsed time; with cancel (a function) it
     also shows a Cancel button. task(text, fn(signal, status, info)) runs a long action that can be cancelled:
     every fetch made while it runs carries its AbortSignal, so Cancel stops a download within a moment, and
     info.fail records the last server that refused (for 'overpass-api.de busy (HTTP 429); trying …'). */
  let busyT0 = 0, busyTimer = 0, cancelFn = null;
  function busyTick() {
    if (!hasDom()) return;
    const el = U.$('#busyTime'); if (!el) return;
    const s = Math.round((Date.now() - busyT0) / 1000);
    el.textContent = s >= 2 ? (s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`) : '';
  }
  function busy(text, o = {}) {
    if (!hasDom()) return;
    const b = U.$('#busy'); if (!b) return;
    if (!text) { b.hidden = true; clearInterval(busyTimer); busyTimer = 0; cancelFn = null; const c = U.$('#busyCancel'); if (c) c.hidden = true; return; }
    if (b.hidden) { busyT0 = Date.now(); clearInterval(busyTimer); busyTimer = setInterval(busyTick, 500); }
    b.hidden = false; U.$('#busyText').textContent = text; busyTick();
    if ('cancel' in o) cancelFn = o.cancel || null;
    const c = U.$('#busyCancel');
    if (c) { c.hidden = !cancelFn; c.onclick = () => { const f = cancelFn; if (f) { c.disabled = true; U.$('#busyText').textContent = 'Cancelling…'; f(); setTimeout(() => { c.disabled = false; }, 800); } }; }
  }
  const tasks = [];
  let fetchHooked = false;
  function hookFetch() {
    if (fetchHooked || typeof window === 'undefined' || typeof window.fetch !== 'function') return;
    fetchHooked = true;
    const orig = window.fetch.bind(window);
    window.fetch = function (input, init) {
      const t = tasks[tasks.length - 1];
      if (!t) return orig(input, init);
      if (t.ac.signal.aborted) return Promise.reject(Object.assign(new Error('Cancelled'), { name: 'AbortError' }));
      init = { ...(init || {}) };
      let sig = t.ac.signal;
      if (init.signal) {
        if (typeof AbortSignal !== 'undefined' && AbortSignal.any) sig = AbortSignal.any([init.signal, sig]);
        else { const both = new AbortController(), own = init.signal; [own, sig].forEach(s => s.addEventListener('abort', () => both.abort(), { once: true })); sig = both.signal; }
      }
      init.signal = sig;
      let host = '';
      try { host = new URL(String(input && input.url || input), location.href).host; } catch { }
      return orig(input, init).then(r => { if (!r.ok) t.info.fail = { host, status: r.status }; return r; },
        e => { if (!t.ac.signal.aborted) t.info.fail = { host, status: 0, error: String(e && e.message || e) }; throw e; });
    };
  }
  async function task(text, fn) {
    hookFetch();
    const ac = new AbortController(), info = { fail: null, cancelled: false };
    const t = { ac, info };
    tasks.push(t);
    busy(text, { cancel: () => { info.cancelled = true; ac.abort(); } });
    try { return await fn(ac.signal, s => busy(s), info); }
    finally { const i = tasks.indexOf(t); if (i >= 0) tasks.splice(i, 1); if (!tasks.length) busy(null); }
  }
  const isAbort = e => !!e && (e.name === 'AbortError' || /cancel/i.test(String(e.message || '')));
  const log = html => typeof UI !== 'undefined' && UI.log(html);

  return {
    esc, opt, fld, bind, fact, kpi, delta, losBadge, gehBadge, pill, compass, prov, sec, note, junctionName, setPath, getPath, num, gapPct,
    GLOSSARY, gl, capSource, convState, convPill, SETTING_LABELS, settingLabel, settingDefaults, nonDefaults, assumptionsCSV, valText, dataSources, CATCH, catchment,
    harden, narrow, casText, casShort, linzKey, junctionLabel, flushSave, legLabels,
    table, rowlink, bar, line, scatter, snap, snapSettings,
    modal, closeModal, modalOpen, hasDom, ask, confirmBox, menu, hideMenu, menuOpen, toast, copyText, busy, task, isAbort, log,
  };
})();
