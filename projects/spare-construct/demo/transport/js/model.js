'use strict';
/* Spare Construct · project model: road classes and defaults (settings version 2, with the migration rule that
   changed defaults reach new projects only), the project store with undo, browser storage (IndexedDB with
   a localStorage fallback, recent projects, second-tab guard), network edits, OpenStreetMap / GeoJSON
   import and export, assessment scenarios (roles, years, staging; the committed-development register) and
   junction sites (Sites.detect groups the nodes of one intersection). */

// Mid-block capacity per lane (pcu/h), default speed limit (km/h), lanes each way, lane width (m)
const CLASSES = {
  motorway:  { label: 'Motorway',  rank: 6, speed: 100, lanes: 2, cap: 2000, lw: 3.5, minPx: 4.5, color: '#3d5ccc' },
  ramp:      { label: 'Ramp',      rank: 5, speed: 70,  lanes: 1, cap: 1700, lw: 3.5, minPx: 3,   color: '#7b93e8' },
  arterial:  { label: 'Arterial',  rank: 4, speed: 50,  lanes: 2, cap: 1200, lw: 3.5, minPx: 3.6, color: '#d4501a' },
  collector: { label: 'Collector', rank: 3, speed: 50,  lanes: 1, cap: 1000, lw: 3.3, minPx: 2.6, color: '#12858a' },
  local:     { label: 'Local',     rank: 2, speed: 50,  lanes: 1, cap: 800,  lw: 3.0, minPx: 1.7, color: '#8a9490' },
};
const CONTROLS = { none: 'Uncontrolled', priority: 'Give way (minor road)', stop: 'Stop (minor road)', roundabout: 'Roundabout', signals: 'Traffic signals' };
const STATUS = { existing: 'Existing', proposed: 'Proposed', closed: 'Closed' };
const CYCLE = { none: 'None', painted: 'Painted cycle lane', separated: 'Separated cycleway' };
const SPEEDS = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110];
// Trip-rate units: per dwelling, per 100 m² GFA, or per student, child place, unit or room.
// group sorts the land uses for display (Residential, Commercial, Community, Industrial).
const LANDUSES = {
  residential: { label: 'Houses', unit: 'dwellings', per: 1, group: 'Residential' },
  apartment:   { label: 'Apartments', unit: 'dwellings', per: 1, group: 'Residential' },
  retirement:  { label: 'Retirement village', unit: 'units', per: 1, group: 'Residential' },
  retail:      { label: 'Retail', unit: 'm² GFA', per: 100, group: 'Commercial' },
  supermarket: { label: 'Supermarket', unit: 'm² GFA', per: 100, group: 'Commercial' },
  fastfood:    { label: 'Fast food', unit: 'm² GFA', per: 100, group: 'Commercial' },
  office:      { label: 'Office', unit: 'm² GFA', per: 100, group: 'Commercial' },
  hotel:       { label: 'Hotel', unit: 'rooms', per: 1, group: 'Commercial' },
  school:      { label: 'School', unit: 'students', per: 1, group: 'Community' },
  childcare:   { label: 'Childcare', unit: 'child places', per: 1, group: 'Community' },
  medical:     { label: 'Medical centre', unit: 'm² GFA', per: 100, group: 'Community' },
  tertiary:    { label: 'Tertiary education', unit: 'students', per: 1, group: 'Community' },
  industrial:  { label: 'Industrial', unit: 'm² GFA', per: 100, group: 'Industrial' },
  warehouse:   { label: 'Warehouse and logistics', unit: 'm² GFA', per: 100, group: 'Industrial' },
};
const ROUTE_COLOURS =['#7048e8', '#0c8599', '#e8590c', '#c2255c', '#2f9e44', '#1971c2', '#9c36b5'];

/* Settings for NEW projects. sv is the settings version: 2 = the NZ-guidance defaults (linear growth,
   MBCM discounting, gapTol 1e-4 / maxIter 200 with the TAG stability rule, PFF 0.95 and T 0.5 h); 3 = 2.0 PCU
   per bus on route preloads (docs/spec/gtfs-bus; the engine used 3 before).
   Projects saved before a version keep their own values (see migrate and SETTINGS_V2). */
const SV = 3;
function defaultSettings() {
  return {
    sv: SV,
    period: 'AM', baseYear: 2026, year: 2026, growth: 1.5,
    // 'linear' = % of base-year traffic per year (MBCM arithmetic growth); 'compound' grows on the grown value
    growthMode: 'linear',
    hvPct: 6, pcuHV: 2.3, peakToDaily: 10,
    // Passenger car units per bus on bus-route preloads (GTFS frequencies × this; docs/spec/gtfs-bus)
    busPcu: 2.0,
    bprAlpha: 0.15, bprBeta: 4, satFlow: 1800, cycle: 100, lostPerPhase: 5, signalOpt: true,
    // Convergence target (relative gap) and the TAG M3.1 stability rule: P > 98% of links changing by < 1% for 4 iterations
    maxIter: 200, gapTol: 0.0001,
    conv: { stable: true, chg: 0.01, share: 0.98, consec: 4, capDamp: 'msa' },
    // Junction analysis: LoS method (hcm6 | sidra | rta), peak flow factor, analysis period T (h) for the
    // overflow terms, queue space (m/veh), 95th-percentile queue factor (to be verified: Engine.STANDARDS.queue).
    // capMethod 'arc' = one gap-acceptance capacity per approach; 'movement' = per-movement capacities with lanes,
    // short lanes and flush medians (docs/spec/movement-capacity; about 3–4× the run time on Drury)
    junction: { losMethod: 'hcm6', pff: 0.95, T: 0.5, queueSpace: 7.0, f95: 1.6, capMethod: 'arc' },
    // Model status (reg-model-status): the TMDG model category chosen for the study and the author's justification
    tmdg: { category: 'E', justification: '' },
    // Assessment scenarios (scenario-matrix): the share of an MSM 2-hour period in the peak hour (Spare Construct assumption)
    assess: { msmPeakShare: 0.55 },
    gravityBeta: 0.10, connectorSpeed: 25, connectors: 2,
    // Indicative peak-hour vehicle trip rates. Replace with RTA/TDB/RR453 surveyed rates for real assessments.
    rates: {
      residential: { AM: { rate: 0.85, inPct: 20 }, PM: { rate: 0.95, inPct: 65 } },
      apartment:   { AM: { rate: 0.45, inPct: 20 }, PM: { rate: 0.50, inPct: 60 } },
      retail:      { AM: { rate: 1.5,  inPct: 60 }, PM: { rate: 4.5,  inPct: 50 } },
      office:      { AM: { rate: 2.0,  inPct: 85 }, PM: { rate: 1.8,  inPct: 15 } },
      industrial:  { AM: { rate: 0.6,  inPct: 70 }, PM: { rate: 0.6,  inPct: 30 } },
      school:      { AM: { rate: 0.45, inPct: 50 }, PM: { rate: 0.05, inPct: 50 } },
      supermarket: { AM: { rate: 3.0,  inPct: 60 }, PM: { rate: 9.0,  inPct: 50 } },
      childcare:   { AM: { rate: 0.8,  inPct: 55 }, PM: { rate: 0.7,  inPct: 45 } },
      medical:     { AM: { rate: 2.5,  inPct: 75 }, PM: { rate: 3.0,  inPct: 35 } },
      retirement:  { AM: { rate: 0.2,  inPct: 40 }, PM: { rate: 0.3,  inPct: 55 } },
      hotel:       { AM: { rate: 0.4,  inPct: 30 }, PM: { rate: 0.4,  inPct: 60 } },
      warehouse:   { AM: { rate: 0.5,  inPct: 65 }, PM: { rate: 0.5,  inPct: 35 } },
      fastfood:    { AM: { rate: 4.0,  inPct: 50 }, PM: { rate: 12.0, inPct: 50 } },
      tertiary:    { AM: { rate: 0.25, inPct: 85 }, PM: { rate: 0.2,  inPct: 20 } },
    },
    // Indicative turn penalties (s); uturn 'ban' = U-turns only at dead ends or where node.allowU
    turns: { left: 2, right: 6, rightSignals: 4, uturn: 'ban' },
    /* Indicative economics. method 'mbcm': MBCM v1.7.5 schedule (rates, % per year to year `to`; to: null = onwards)
       as NZTA General Circular 25/01 sets it (2% years 1–30, 1.5% years 31–100, 1% from year 101), 8% sensitivity;
       activity 'commercial' uses 8% for all years with 2% sensitivity (GC 25/01).
       periodHours = equivalent hours a year per modelled period (AM + PM = the earlier 1,200 h), arithmetic
       benefit growth (benefitGrowth % a year; null = the background growth rate). votUpdate: true when vot is
       at the MBCM base date and needs the July 2025 update factor. years > 40 needs longLifeNote, > 60 approval.
       method 'uniform' (older projects): flat benefits at `discount` % (annualHours × the average peak). */
    econ: {
      method: 'mbcm', activity: 'non-commercial', vot: 32, votUpdate: false, annualHours: 1200, periodHours: { AM: 600, PM: 600, IP: 0 },
      rates: [{ to: 30, r: 2 }, { to: 100, r: 1.5 }, { to: null, r: 1 }], sensRate: 8, discount: 2, years: 40,
      longLifeNote: '', approval: '', benefitGrowth: null, p95: false,
      laneKm: { motorway: 12, ramp: 8, arterial: 5, collector: 3.5, local: 2.5 },   // NZ$m per lane-km
      node: { signals: 1.5, roundabout: 3, priority: 0.2, stop: 0.2, none: 0 },        // NZ$m per new/upgraded junction
    },
    sim: { scale: 1, speed: 6, maxVeh: 6000 },
    // Peak hour as a share of daily traffic, used where a count has no measured peak hour (%)
    data: { amShare: 8.5, pmShare: 9.5, matchRadius: 40 },
    linzKey: '', basemap: '',
  };
}
// sc.year is left unset: the scenario then follows the project year (Engine.yearOf)
function newScenario(name) {
  return { id: U.uid('s'), name, notes: '', nodes: {}, links: {}, zones: {}, routes: {}, odManual: null, res: {} };
}
function newProject(name = 'Untitled project') {
  const s = newScenario('Base case');
  return { v: 1, id: U.uid('p'), name, settings: defaultSettings(), scenarios: [s], active: s.id, base: s.id, view: { c: [-36.8485, 174.7633], z: 14 } };
}
// What undo restores: the network, demand and the scenario's assessment fields (role, staging, periods, junction sites)
const editable = sc => ({ nodes: sc.nodes, links: sc.links, zones: sc.zones, routes: sc.routes, odManual: sc.odManual, year: sc.year ?? null,
  sites: sc.sites ?? null, role: sc.role ?? null, buildout: sc.buildout ?? null, periods: sc.periods ?? null });
const getPath = (o, k) => k.split('.').reduce((a, p) => (a == null ? undefined : a[p]), o);
const setPath = (o, k, v) => { const p = k.split('.'), last = p.pop(); let a = o; for (const q of p) { if (a[q] == null || typeof a[q] !== 'object') a[q] = {}; a = a[q]; } a[last] = v; };

/* Defaults that changed after settings version 1 (docs/spec/_overview rule 3), each with the version that brought it
   (since). A project saved before that version keeps its own values; a key it never saved gets its earlier value (v1),
   so its results do not change. migrate() lists the differences in P.settingsNotice for the UI ('Review and adopt' →
   Store.adoptDefaults, 'Keep mine' → Store.keepSettings). Until one is chosen the project keeps its saved settings
   version and the notice comes back on load. */
const SETTINGS_V2 = [
  { key: 'growthMode', v1: 'compound', since: 2, label: 'Background growth', text: v => v === 'linear' ? 'linear (% of base-year traffic a year, as MBCM)' : 'compound' },
  { key: 'gapTol', v1: 0.0005, since: 2, label: 'Convergence target (relative gap)', text: v => `${+(v * 100).toPrecision(2)}%` },
  { key: 'maxIter', v1: 60, since: 2, label: 'Maximum iterations', text: v => String(v) },
  { key: 'conv.stable', v1: false, since: 2, label: 'Stop only when link flows are stable (TAG M3.1: P > 98% for 4 iterations)', text: v => v ? 'on' : 'off' },
  { key: 'conv.capDamp', v1: 'none', since: 2, label: 'Damped give-way and roundabout capacities during assignment', text: v => v === 'msa' ? 'on (MSA)' : 'off' },
  { key: 'econ.method', v1: 'uniform', since: 2, label: 'Discounting', text: (v, S) => v === 'mbcm' ? 'MBCM v1.7.5 and NZTA GC 25/01: 2% to year 30, then 1.5% to year 100, then 1%; 8% sensitivity; arithmetic benefit growth' : `${(S.econ || {}).discount}% a year, flat benefits` },
  { key: 'junction.pff', v1: 1, since: 2, label: 'Peak flow factor', text: v => `${Math.round(v * 100)}%` },
  { key: 'junction.T', v1: 1, since: 2, label: 'Analysis period for overflow delay and queues', text: v => `${Math.round(v * 60)} min` },
  { key: 'busPcu', v1: 3, since: 3, label: 'Passenger car units per bus (bus route preload)', text: v => `${v} PCU` },
];
const SETTINGS_NOTICE = "Spare Construct's defaults changed to match NZ guidance (growth, discounting, convergence, buses). This project keeps its own settings until you choose.";
function legacySettings(P, saved, ver = 1) {
  const list = SETTINGS_V2.filter(c => c.since > ver);
  for (const c of list) if (getPath(saved, c.key) === undefined) setPath(P.settings, c.key, c.v1);
  const D = defaultSettings(), S = P.settings;
  const changed = list.filter(c => getPath(S, c.key) !== getPath(D, c.key)).map(c => ({
    key: c.key, label: c.label, yours: getPath(S, c.key), now: getPath(D, c.key), yoursText: c.text(getPath(S, c.key), S), nowText: c.text(getPath(D, c.key), D) }));
  if (changed.length) { S.sv = ver; P.settingsNotice = { from: ver, to: SV, text: SETTINGS_NOTICE, changed }; }
  else { S.sv = SV; delete P.settingsNotice; }
}
/* The discount schedule Spare Construct saved before NZTA GC 25/01 was encoded in full ([2% to 30, 1.5% after]) is the GC
   25/01 schedule up to year 100, so a saved copy of it is upgraded to the full schedule when the analysis period is
   100 years or less (the results are identical; past 100 years GC 25/01 uses 1%). */
const OLD_RATES = JSON.stringify([{ to: 30, r: 2 }, { to: 60, r: 1.5 }]);
function upgradeRates(S) {
  const E = S && S.econ;
  if (E && Array.isArray(E.rates) && JSON.stringify(E.rates) === OLD_RATES && !(+E.years > 100)) E.rates = JSON.parse(JSON.stringify(defaultSettings().econ.rates));
}
// A year typed or read from a file as text ('2036') is stored as a number; anything else falls back
const migYear = (v, d) => ((typeof v === 'number' || (typeof v === 'string' && v.trim() !== '')) && isFinite(+v) && +v > 0 ? Math.round(+v) : d);

/* ---------------- browser storage ----------------
   Projects live in IndexedDB ('corridor' v1: stores projects {id, name, updated, json, size, scenarios},
   meta (the same without json, for the recent list) and handles (file handles for save-in-place)), or in
   localStorage ('corridor.p.<id>' plus 'corridor.index') where IndexedDB is missing. The pre-v2 single
   key 'corridor.project' is copied in on first use and removed only after that write succeeds.
   Store.saveNow / saveSoon / load keep working as before until Store.boot() switches autosave over. */
const LS = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); return true; } catch { return false; } },
  del(k) { try { localStorage.removeItem(k); } catch { } },
};
const KEY = { legacy: 'corridor.project', last: 'corridor.last', index: 'corridor.index', p: id => 'corridor.p.' + id };
const IDB = {
  db: null, p: null,
  open(ms = 4000) {
    if (this.p) return this.p;
    return this.p = new Promise(res => {
      let done = false;
      const fin = v => { if (done) return; done = true; this.db = v; res(v); };
      try {
        if (typeof indexedDB === 'undefined' || !indexedDB) return fin(null);
        const rq = indexedDB.open('corridor', 1);
        rq.onupgradeneeded = () => {
          const db = rq.result;
          for (const n of ['projects', 'meta', 'handles']) if (!db.objectStoreNames.contains(n)) db.createObjectStore(n, { keyPath: 'id' });
        };
        rq.onsuccess = () => fin(rq.result);
        rq.onerror = () => fin(null);
        rq.onblocked = () => fin(null);
        setTimeout(() => fin(null), ms);
      } catch { fin(null); }
    });
  },
  // One transaction over the named stores; fn(stores) returns the request whose result is wanted
  run(names, mode, fn) {
    return new Promise((res, rej) => {
      try {
        const tx = this.db.transaction(names, mode), st = {};
        for (const n of [].concat(names)) st[n] = tx.objectStore(n);
        const r = fn(st);
        tx.oncomplete = () => res(r ? r.result : undefined);
        tx.onerror = () => rej(tx.error || new Error('IndexedDB refused the write'));
        tx.onabort = () => rej(tx.error || new Error('IndexedDB write aborted'));
      } catch (e) { rej(e); }
    });
  },
};
const metaOf = r => ({ id: r.id, name: r.name, updated: r.updated, size: r.size, scenarios: r.scenarios });
// The two back ends behind one interface: put(rec), get(id), list(), del(id)
const Backends = {
  idb: {
    put: rec => IDB.run(['projects', 'meta'], 'readwrite', st => { st.projects.put(rec); st.meta.put(metaOf(rec)); }),
    get: id => IDB.run('projects', 'readonly', st => st.projects.get(id)),
    list: () => IDB.run('meta', 'readonly', st => st.meta.getAll()),
    del: id => IDB.run(['projects', 'meta', 'handles'], 'readwrite', st => { st.projects.delete(id); st.meta.delete(id); st.handles.delete(id); }),
  },
  local: {
    index() { try { const a = JSON.parse(LS.get(KEY.index) || '[]'); return Array.isArray(a) ? a : []; } catch { return []; } },
    async put(rec) {
      if (!LS.set(KEY.p(rec.id), rec.json)) throw new Error('browser storage is full');
      const ix = this.index().filter(m => m.id !== rec.id); ix.push(metaOf(rec));
      if (!LS.set(KEY.index, JSON.stringify(ix))) throw new Error('browser storage is full');
    },
    async get(id) { const json = LS.get(KEY.p(id)); if (json == null) return null; const m = this.index().find(x => x.id === id) || { id }; return { ...m, json }; },
    async list() { return this.index(); },
    async del(id) { LS.del(KEY.p(id)); LS.set(KEY.index, JSON.stringify(this.index().filter(m => m.id !== id))); },
  },
};

const Store = {
  P: null, ver: 0, undoStack: [], redoStack: [], subs: [], saveWarned: false,
  // dbActive: autosave goes to Store.projects (set by boot); saveState: the last save, for the status line
  dbActive: false, saveState: { ok: null, where: 'none', at: null, size: 0, error: '' }, saveSubs: [], persisted: false,
  get sc() { return this.P.scenarios.find(s => s.id === this.P.active) || this.P.scenarios[0]; },
  get baseSc() { return this.P.scenarios.find(s => s.id === this.P.base) || this.P.scenarios[0]; },
  get S() { return this.P.settings; },
  res(sc = this.sc, period = this.S.period) { return (sc && sc.res && sc.res[period]) || null; },
  on(fn) { this.subs.push(fn); },
  emit(kind) { this.ver++; for (const f of this.subs) f(kind); this.saveSoon(); },
  // Listeners for save results only (they do not trigger a redraw): fn(saveState)
  onSave(fn) { this.saveSubs.push(fn); },
  _saved(st) { this.saveState = st; for (const f of this.saveSubs) { try { f(st); } catch { } } },
  snapshot() {
    const sc = this.sc;
    this.undoStack.push({ sid: sc.id, data: JSON.stringify(editable(sc)) });
    if (this.undoStack.length > 60) this.undoStack.shift();
    this.redoStack.length = 0;
  },
  // Any edit to a scenario's network or demand makes its results out of date
  edited(kind = 'net', sc = this.sc) {
    for (const r of Object.values(sc.res || {})) if (r) r.stale = true;
    this.emit(kind);
  },
  _swap(from, to) {
    const u = from.pop(); if (!u) return false;
    const sc = this.P.scenarios.find(s => s.id === u.sid); if (!sc) return false;
    to.push({ sid: sc.id, data: JSON.stringify(editable(sc)) });
    Object.assign(sc, JSON.parse(u.data));
    for (const k of ['year', 'sites', 'role', 'buildout', 'periods']) if (sc[k] == null) delete sc[k];
    this.P.active = sc.id;
    this.edited('undo', sc);
    return true;
  },
  undo() { return this._swap(this.undoStack, this.redoStack); },
  redo() { return this._swap(this.redoStack, this.undoStack); },
  serialise(P = this.P) { return JSON.stringify(P, (k, v) => (k === 'paths' || k === 'arcKeys' || k === 'settingsNotice') ? undefined : v); },
  // Autosave. Before boot(): the single localStorage key, as before v2. After boot(): the project store
  // (returns a Promise of true/false). Nothing is written while another tab has taken this project over.
  saveNow() {
    if (!this.P) return false;
    if (this.tabs.yielded) return false;
    if (this.dbActive && this.projects.backend) return this.projects.write();
    try { localStorage.setItem(KEY.legacy, this.serialise()); this._saved({ ok: true, where: 'local', at: Date.now(), size: 0, error: '' }); return true; }
    catch (e) {
      this._saved({ ok: false, where: 'none', at: Date.now(), size: 0, error: String((e && e.message) || e) });
      if (!this.saveWarned) { this.saveWarned = true; window.UI && UI.toast('This project is too big for autosave. Use File › Save project to keep your work.', 'warn'); }
      return false;
    }
  },
  load() {
    try {
      const last = LS.get(KEY.last), t = LS.get(KEY.legacy) || (last && LS.get(KEY.p(last)));
      if (!t) return false;
      this.P = migrate(JSON.parse(t));
      return true;
    } catch { return false; }
  },
  setProject(P) { this.P = migrate(P); this.undoStack = []; this.redoStack = []; this.emit('project'); this.tabs.post('here'); },
  // Open a project without writing it straight back (opening is not an edit)
  _openQuiet(P) {
    this.P = migrate(P); this.undoStack = []; this.redoStack = [];
    this.ver++; for (const f of this.subs) f('project');
    this.tabs.post('here');
  },
  /* Async start-up for the UI: opens the project store (copying the old localStorage project in), then
     opens the last project used in this browser, or the most recent. Returns true if one was opened; with
     false the caller starts a new project. Also starts the second-tab guard. */
  async boot() {
    const be = await this.projects.ready();
    this.tabs.start();
    if (!be) return this.load();
    this.dbActive = true;
    const last = LS.get(KEY.last), list = await this.projects.list();
    const pick = list.find(m => m.id === last) || list[0];
    return pick ? this.projects.open(pick.id) : false;
  },
  // 'Saved in this browser · 14:32', or what went wrong
  saveText() {
    const s = this.saveState;
    if (s.ok == null) return '';
    if (!s.ok) return 'Not saved: browser storage refused. Save the project to a file to keep your work.';
    let t = '';
    try { t = new Intl.DateTimeFormat('en-NZ', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(s.at)); } catch { }
    return `Saved in this browser${t ? ' · ' + t : ''}`;
  },
  // Settings notice (rule 3): adopt the new defaults (all, or the listed keys) or keep the project's own
  adoptDefaults(keys = null) {
    const N = this.P && this.P.settingsNotice; if (!N) return 0;
    const D = defaultSettings(), pick = N.changed.filter(c => !keys || keys.includes(c.key));
    for (const c of pick) setPath(this.P.settings, c.key, getPath(D, c.key));
    this.P.settings.sv = SV; delete this.P.settingsNotice;
    if (pick.length) for (const s of this.P.scenarios) for (const r of Object.values(s.res || {})) if (r) r.stale = true;
    this.emit('settings');
    return pick.length;
  },
  keepSettings() {
    if (!this.P || !this.P.settingsNotice) return false;
    this.P.settings.sv = SV; delete this.P.settingsNotice;
    this.emit('settings');
    return true;
  },
};
// Debounced autosave; flush() writes a pending save now (before switching projects), cancel() drops it
Store.saveSoon = (() => {
  let t = null;
  const f = () => { clearTimeout(t); t = setTimeout(() => { t = null; Store.saveNow(); }, 900); };
  f.pending = () => t != null;
  f.flush = () => { if (t == null) return false; clearTimeout(t); t = null; return Store.saveNow(); };
  f.cancel = () => { clearTimeout(t); t = null; };
  return f;
})();

/* Store.projects: the projects kept in this browser. Every call is async and never throws for a storage
   failure; failures come back as false / null / { ok: false, why }. */
Store.projects = {
  backend: null,   // 'idb' | 'local' | null (no storage at all)
  _ready: null, _chain: Promise.resolve(),
  get be() { return Backends[this.backend]; },
  ready() {
    if (this._ready) return this._ready;
    return this._ready = (async () => {
      const db = await IDB.open();
      if (db) this.backend = 'idb';
      else { const ok = LS.set('corridor.probe', '1'); LS.del('corridor.probe'); this.backend = ok ? 'local' : null; }
      if (this.backend) await this._migrateLegacy();
      return this.backend;
    })();
  },
  async _migrateLegacy() {
    const t = LS.get(KEY.legacy); if (!t) return false;
    let P;
    try { P = migrate(JSON.parse(t)); } catch { return false; }
    try {
      await this.be.put(this._rec(P, Store.serialise(P)));
      const back = await this.be.get(P.id);
      if (!back || !back.json) return false;
      LS.del(KEY.legacy);   // only once the copy is safely written
      if (!LS.get(KEY.last)) LS.set(KEY.last, P.id);
      return true;
    } catch { return false; }
  },
  _rec(P, json) {
    return { id: P.id, name: P.name || 'Untitled project', updated: Date.now(), json, size: json.length, scenarios: (P.scenarios || []).length };
  },
  // Recent projects, newest first: [{ id, name, updated, size (characters), scenarios, current }]
  async list() {
    if (!(await this.ready())) return [];
    try {
      const a = (await this.be.list()) || [];
      return a.map(m => ({ ...m, current: !!(Store.P && m.id === Store.P.id) })).sort((x, y) => (y.updated || 0) - (x.updated || 0));
    } catch { return []; }
  },
  async open(id) {
    if (!(await this.ready())) return false;
    try {
      const r = await this.be.get(id);
      if (!r || !r.json) return false;
      const P = JSON.parse(r.json);
      P.id = r.id;
      await Store.saveSoon.flush();   // edits still waiting in the open project are saved first
      Store._openQuiet(P);
      Store.dbActive = true;
      LS.set(KEY.last, r.id);
      return true;
    } catch { return false; }
  },
  // Write the open project now. Writes are queued, so a slow write is never overtaken by an older one.
  write() {
    const P = Store.P; if (!P || !this.backend) return Promise.resolve(false);
    if (!P.id) P.id = U.uid('p');
    let rec;
    try { rec = this._rec(P, Store.serialise()); }
    catch (e) { Store._saved({ ok: false, where: 'none', at: Date.now(), size: 0, error: String((e && e.message) || e) }); return Promise.resolve(false); }
    const where = this.backend === 'idb' ? 'browser' : 'local';
    const job = this._chain.then(async () => {
      try {
        await this.be.put(rec);
        LS.set(KEY.last, rec.id);
        Store._saved({ ok: true, where, at: rec.updated, size: rec.size, error: '' });
        if (!Store.persisted) {
          Store.persisted = true;
          try { if (typeof navigator !== 'undefined' && navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(() => { }); } catch { }
        }
        return true;
      } catch (e) {
        Store._saved({ ok: false, where: 'none', at: Date.now(), size: rec.size, error: String((e && e.message) || e) });
        return false;
      }
    });
    this._chain = job.catch(() => false);
    return job;
  },
  async save() { if (!(await this.ready())) return false; Store.dbActive = true; return this.write(); },
  // Keep a copy under a new name and carry on in the copy (the original stays as it was last saved)
  async saveAs(name) {
    if (!(await this.ready()) || !Store.P) return null;
    await this.write();
    Store.P.id = U.uid('p');
    Store.P.name = String(name || '').trim() || (Store.P.name || 'Untitled project') + ' (copy)';
    Store.dbActive = true;
    const ok = await this.write();
    Store.emit('meta');
    Store.tabs.post('here');
    return ok ? Store.P.id : null;
  },
  // A saved file handle per project (Chrome/Edge showSaveFilePicker), so Ctrl+S can write the same file
  // again; the caller checks queryPermission/requestPermission before reuse. Null where unavailable.
  async getHandle(id = Store.P && Store.P.id) {
    if ((await this.ready()) !== 'idb' || !id) return null;
    try { const r = await IDB.run('handles', 'readonly', st => st.handles.get(id)); return (r && r.handle) || null; } catch { return null; }
  },
  async setHandle(handle, id = Store.P && Store.P.id) {
    if ((await this.ready()) !== 'idb' || !id) return false;
    try { await IDB.run('handles', 'readwrite', st => handle ? st.handles.put({ id, handle, name: handle.name || '' }) : st.handles.delete(id)); return true; } catch { return false; }
  },
  // Remove a project from this browser (the UI asks first). The open project cannot be removed.
  async remove(id) {
    if (!(await this.ready())) return { ok: false, why: 'This browser has no project storage.' };
    if (Store.P && id === Store.P.id) return { ok: false, why: 'This project is open. Open or start another project first, then remove it.' };
    try {
      await this.be.del(id);
      if (LS.get(KEY.last) === id) LS.del(KEY.last);
      return { ok: true, why: '' };
    } catch (e) { return { ok: false, why: `The browser refused to remove it (${(e && e.message) || e}).` }; }
  },
};

/* Store.tabs: second-tab guard over BroadcastChannel('corridor'). Tabs announce themselves ('hello' →
   'here'); another tab with the same project open is a conflict. 'Use this tab' (claim) makes the other
   tabs stop autosaving that project. Store.tabs.on(fn) gets { others, conflict, yielded, text }. */
Store.tabs = {
  id: U.uid('t'), ch: null, peers: new Map(), yielded: false, subs: [],
  start() {
    if (this.ch || typeof BroadcastChannel === 'undefined') return !!this.ch;
    try {
      this.ch = new BroadcastChannel('corridor');
      this.ch.onmessage = e => this._msg(e && e.data);
      if (typeof addEventListener === 'function') addEventListener('pagehide', () => this.post('bye'));
      this.post('hello');
    } catch { this.ch = null; }
    return !!this.ch;
  },
  stop() { this.post('bye'); try { if (this.ch) this.ch.close(); } catch { } this.ch = null; this.peers.clear(); },
  post(t) { if (!this.ch) return; try { this.ch.postMessage({ t, tab: this.id, pid: Store.P ? Store.P.id || null : null, at: Date.now() }); } catch { } },
  _msg(m) {
    if (!m || typeof m !== 'object' || m.tab === this.id) return;
    if (m.t === 'bye') this.peers.delete(m.tab);
    else {
      this.peers.set(m.tab, { pid: m.pid, at: m.at });
      if (m.t === 'hello') this.post('here');
      if (m.t === 'claim' && Store.P && m.pid === Store.P.id) this.yielded = true;
    }
    this._changed();
  },
  // 'Use this tab': this tab saves again and the others stop saving this project
  claim() { this.yielded = false; this.post('claim'); this._changed(); if (Store.P) Store.saveSoon(); },
  state() {
    const pid = Store.P && Store.P.id, conflict = [...this.peers.values()].some(p => p.pid && p.pid === pid);
    const text = this.yielded ? 'This project was taken over in another tab. Changes made here are not saved. Choose Use this tab to save from here instead.'
      : conflict ? 'Spare Construct is open in another tab. Edits in both tabs will overwrite each other.' : '';
    return { others: this.peers.size, conflict, yielded: this.yielded, text };
  },
  on(fn) { this.subs.push(fn); },
  _changed() { const s = this.state(); for (const f of this.subs) { try { f(s); } catch { } } },
};

function migrate(P) {
  const saved = P.settings && typeof P.settings === 'object' && !Array.isArray(P.settings) ? P.settings : null;
  // The saved settings version (1 when a project predates versions); a project without settings is new
  const ver = saved ? (+saved.sv >= 1 ? Math.min(SV, Math.floor(+saved.sv)) : 1) : SV;
  P.settings = U.deepMerge(defaultSettings(), saved || {});
  if (ver < SV) legacySettings(P, saved, ver); else delete P.settingsNotice;
  // Years from a file can be text ('2036'): numbers from here on (Engine.yearOf, growth and economics compare them)
  const D0 = defaultSettings();
  P.settings.baseYear = migYear(P.settings.baseYear, D0.baseYear);
  P.settings.year = migYear(P.settings.year, P.settings.baseYear);
  upgradeRates(P.settings);
  if (!P.id || typeof P.id !== 'string') P.id = U.uid('p');
  P.scenarios = P.scenarios && P.scenarios.length ? P.scenarios : [newScenario('Base case')];
  for (const s of P.scenarios) {
    s.nodes ||= {}; s.links ||= {}; s.zones ||= {}; s.routes ||= {}; s.res ||= {};
    for (const r of Object.values(s.res)) if (r && !r.paths) r.noPaths = true;
    if (s.year != null) { const y = migYear(s.year, null); if (y == null) delete s.year; else s.year = y; }
    // Junction sites from a file: keep well-formed ones (id = key, nodes an array)
    if (s.sites != null && (typeof s.sites !== 'object' || Array.isArray(s.sites))) delete s.sites;
    else if (s.sites) for (const [k, v] of Object.entries(s.sites)) { if (!v || typeof v !== 'object' || !Array.isArray(v.nodes)) delete s.sites[k]; else v.id = k; }
    if (s.buildout != null && !isFinite(+s.buildout)) delete s.buildout;
  }
  if (P.register != null && !Array.isArray(P.register)) delete P.register;
  // Saved Mill Road preset rows gain the designation 6786 record (data only; inclusion and results are unchanged)
  if (Array.isArray(P.register)) {
    const fresh = new Map(Register.presets().filter(r => r.designation).map(r => [r.id, r.designation]));
    for (const r of P.register) if (r && typeof r === 'object' && r.preset && fresh.has(r.id) && !r.designation) r.designation = { ...fresh.get(r.id) };
  }
  if (P.me != null && typeof P.me !== 'object') delete P.me;
  if (!P.scenarios.some(s => s.id === P.active)) P.active = P.scenarios[0].id;
  if (!P.scenarios.some(s => s.id === P.base)) P.base = P.scenarios[0].id;
  P.view ||= { c: [-36.8485, 174.7633], z: 14 };
  return P;
}

/* ---------------- network edits ---------------- */
const Net = {
  geom(sc, L) {
    const A = sc.nodes[L.a], B = sc.nodes[L.b];
    return [[A.lat, A.lng], ...(L.pts || []), [B.lat, B.lng]];
  },
  len(sc, L) { return U.polyLen(this.geom(sc, L)); },
  addNode(sc, ll, props = {}) {
    const id = U.uid('n');
    sc.nodes[id] = { id, lat: U.r7(ll[0]), lng: U.r7(ll[1]), control: 'none', ...props };
    return id;
  },
  addLink(sc, a, b, pts = [], props = {}) {
    const t = CLASSES[props.cls || 'local'] || CLASSES.local;
    const id = U.uid('l');
    sc.links[id] = {
      id, a, b, name: '', cls: 'local', lanesAB: t.lanes, lanesBA: t.lanes, speed: t.speed, capPerLane: null,
      status: 'existing', busLane: false, cycle: 'none', ...props,
      pts: pts.map(p => [U.r7(p[0]), U.r7(p[1])]),
    };
    return id;
  },
  linksAt(sc, nid) { return Object.values(sc.links).filter(L => L.a === nid || L.b === nid); },
  // Nearest point on a link to ll; returns segment index and the snapped position
  project(sc, L, ll) {
    const g = this.geom(sc, L), F = U.flat(ll[0]);
    const px = F.x(ll), py = F.y(ll);
    let best = { d: Infinity };
    for (let i = 0; i < g.length - 1; i++) {
      const r = U.segDist(px, py, F.x(g[i]), F.y(g[i]), F.x(g[i + 1]), F.y(g[i + 1]));
      if (r.d < best.d) best = { d: r.d, i, x: r.x, y: r.y };
    }
    return { d: best.d, i: best.i, p: [best.y / F.ky, best.x / F.kx], g };
  },
  nearestLink(sc, ll, maxM = Infinity, filter) {
    let best = null;
    for (const L of Object.values(sc.links)) {
      if (filter && !filter(L)) continue;
      const A = sc.nodes[L.a], B = sc.nodes[L.b];
      if (!A || !B) continue;
      // quick reject on bounding box
      const pad = maxM / 90000 + 0.0005;
      let minLa = Math.min(A.lat, B.lat), maxLa = Math.max(A.lat, B.lat), minLn = Math.min(A.lng, B.lng), maxLn = Math.max(A.lng, B.lng);
      for (const p of L.pts || []) { minLa = Math.min(minLa, p[0]); maxLa = Math.max(maxLa, p[0]); minLn = Math.min(minLn, p[1]); maxLn = Math.max(maxLn, p[1]); }
      if (isFinite(maxM) && (ll[0] < minLa - pad || ll[0] > maxLa + pad || ll[1] < minLn - pad * 1.3 || ll[1] > maxLn + pad * 1.3)) continue;
      const r = this.project(sc, L, ll);
      if (r.d <= maxM && (!best || r.d < best.d)) best = { id: L.id, d: r.d, p: r.p };
    }
    return best;
  },
  nearestNode(sc, ll, maxM = Infinity, filter) {
    let best = null, bd = maxM;
    for (const n of Object.values(sc.nodes)) {
      if (filter && !filter(n)) continue;
      const d = U.dist(ll, [n.lat, n.lng]);
      if (d < bd) { bd = d; best = n.id; }
    }
    return best;
  },
  // Split a link at the point nearest ll. Returns the node at the split (an end node if ll is at an end).
  splitLink(sc, id, ll) {
    const L = sc.links[id];
    const r = this.project(sc, L, ll), g = r.g;
    if (U.dist(r.p, g[0]) < 2.5) return L.a;
    if (U.dist(r.p, g[g.length - 1]) < 2.5) return L.b;
    const nid = this.addNode(sc, r.p);
    const pts1 = g.slice(1, r.i + 1), pts2 = g.slice(r.i + 1, g.length - 1);
    const { id: _i, a: _a, b, pts: _p, ...props } = L;
    L.b = nid; L.pts = pts1;
    this.addLink(sc, nid, b, pts2, props);
    return nid;
  },
  // Junctions with three or more roads default to give-way control
  autoControl(sc, ids) {
    const deg = new Map(), fwy = new Map();
    for (const L of Object.values(sc.links)) for (const id of [L.a, L.b]) {
      deg.set(id, (deg.get(id) || 0) + 1);
      fwy.set(id, (fwy.get(id) ?? true) && (CLASSES[L.cls] || CLASSES.local).rank >= 5);
    }
    for (const id of ids) {
      const n = sc.nodes[id]; if (!n) continue;
      if ((deg.get(id) || 0) >= 3 && n.control === 'none' && !fwy.get(id)) n.control = 'priority';
      if ((deg.get(id) || 0) < 3 && (n.control === 'priority' || n.control === 'stop')) n.control = 'none';
    }
  },
  pruneNode(sc, nid) {
    if (!sc.nodes[nid]) return;
    if (this.linksAt(sc, nid).length) return;
    if (Object.values(sc.zones).some(z => z.node === nid)) return;
    if (Object.values(sc.routes).some(r => (r.wp || []).includes(nid))) return;
    delete sc.nodes[nid];
  },
  deleteLink(sc, id) {
    const L = sc.links[id]; if (!L) return;
    delete sc.links[id];
    this.pruneNode(sc, L.a); this.pruneNode(sc, L.b);
    this.autoControl(sc, [L.a, L.b]);
  },
  deleteNode(sc, nid) {
    const nbrs = new Set();
    for (const L of this.linksAt(sc, nid)) { delete sc.links[L.id]; nbrs.add(L.a); nbrs.add(L.b); }
    nbrs.delete(nid);
    delete sc.nodes[nid];
    for (const z of Object.values(sc.zones)) if (z.node === nid) z.node = null;
    for (const r of Object.values(sc.routes)) r.wp = (r.wp || []).filter(w => w !== nid);
    for (const n of nbrs) this.pruneNode(sc, n);
    this.autoControl(sc, [...nbrs]);
  },
  reverse(sc, id) { const L = sc.links[id]; [L.lanesAB, L.lanesBA] = [L.lanesBA, L.lanesAB]; },
  degree(sc) {
    const d = new Map();
    for (const L of Object.values(sc.links)) { d.set(L.a, (d.get(L.a) || 0) + 1); d.set(L.b, (d.get(L.b) || 0) + 1); }
    return d;
  },
};

/* ---------------- OpenStreetMap import ---------------- */
const OSM_CLASS = {
  motorway: 'motorway', trunk: 'arterial', primary: 'arterial', secondary: 'arterial', tertiary: 'collector',
  unclassified: 'local', residential: 'local', living_street: 'local', service: 'local',
  motorway_link: 'ramp', trunk_link: 'ramp', primary_link: 'arterial', secondary_link: 'arterial', tertiary_link: 'collector',
};
const OVERPASS = [
  'https://overpass-api.de/api/interpreter',
  'https://maps.mail.ru/osm/tools/overpass/api/interpreter',
  'https://overpass.private.coffee/api/interpreter',
  'https://overpass.kumi.systems/api/interpreter',
];

const Importers = {
  async overpass(b, opts = {}, status = () => {}) {
    const types = Object.keys(OSM_CLASS)
      .filter(t => opts.service || t !== 'service')
      .filter(t => opts.local !== false || OSM_CLASS[t] !== 'local');
    const bbox = [b.getSouth(), b.getWest(), b.getNorth(), b.getEast()].map(v => v.toFixed(6)).join(',');
    const q = `[out:json][timeout:120];way["highway"~"^(${types.join('|')})$"](${bbox});out body;>;out body qt;`;
    let lastErr;
    for (const url of OVERPASS) {
      try {
        status(`Downloading roads from ${new URL(url).host}…`);
        const r = await fetch(url, { method: 'POST', body: 'data=' + encodeURIComponent(q), headers: { 'Content-Type': 'application/x-www-form-urlencoded' } });
        if (!r.ok) throw new Error(`${new URL(url).host} answered HTTP ${r.status}`);
        return await r.json();
      } catch (e) { lastErr = e; }
    }
    throw lastErr || new Error('No OpenStreetMap server answered');
  },

  // opts.sites true also groups the junction nodes into sites (docs/spec/junction-sites). Off by default for now so
  // existing imports and tests keep their results; the UI turns it on for new imports.
  osmToScenario(sc, data, opts = {}) {
    const N = new Map(), ways = [];
    for (const e of data.elements || []) {
      if (e.type === 'node') N.set(e.id, e);
      else if (e.type === 'way' && e.nodes && e.nodes.length > 1) ways.push(e);
    }
    const keep = ways.filter(w => {
      const t = w.tags || {}, cls = OSM_CLASS[t.highway];
      if (!cls || t.area === 'yes') return false;
      if (cls === 'local' && /^(no|private)$/.test(t.access || '')) return false;
      if (t.highway === 'service' && t.service && t.service !== 'alley') return false;
      return true;
    });
    const use = new Map();
    for (const w of keep) w.nodes.forEach((id, i) => use.set(id, (use.get(id) || 0) + (i === 0 || i === w.nodes.length - 1 ? 2 : 1)));
    const byOsm = new Map();
    for (const n of Object.values(sc.nodes)) if (n.osm) byOsm.set(n.osm, n.id);
    const have = new Set();
    for (const L of Object.values(sc.links)) if (L.osm) have.add(L.osm);
    const getNode = oid => {
      let id = byOsm.get(oid); if (id) return id;
      const o = N.get(oid);
      id = Net.addNode(sc, [o.lat, o.lon], { osm: oid });
      byOsm.set(oid, id);
      return id;
    };
    let added = 0;
    const ringNodes = new Set(), touched = new Set();
    for (const w of keep) {
      const t = w.tags || {}, cls = OSM_CLASS[t.highway], C = CLASSES[cls];
      const ring = t.junction === 'roundabout' || t.junction === 'circular';
      let ow = (ring || t.highway === 'motorway' || t.highway === 'motorway_link') ? 1 : 0;
      if (/^(yes|true|1)$/.test(t.oneway || '')) ow = 1;
      else if (t.oneway === '-1' || t.oneway === 'reverse') ow = -1;
      else if (t.oneway === 'no' && !ring) ow = 0;
      const lanesT = parseInt(t.lanes, 10);
      let ab, ba;
      if (ow) {
        const n = lanesT > 0 ? lanesT : (cls === 'motorway' ? 2 : 1);
        ab = ow > 0 ? n : 0; ba = ow < 0 ? n : 0;
      } else {
        const f = parseInt(t['lanes:forward'], 10), bk = parseInt(t['lanes:backward'], 10);
        ab = f > 0 ? f : lanesT > 1 ? Math.ceil(lanesT / 2) : 1;
        ba = bk > 0 ? bk : lanesT > 1 ? Math.floor(lanesT / 2) : 1;
      }
      ab = Math.min(ab, 6); ba = Math.min(ba, 6);
      let speed = parseInt(t.maxspeed, 10);
      if (!(speed > 0)) speed = t.maxspeed === 'NZ:rural' ? 100 : t.maxspeed === 'NZ:urban' ? 50 : t.highway === 'trunk' ? 80 : C.speed;
      const cyc = [t.cycleway, t['cycleway:left'], t['cycleway:right'], t['cycleway:both']].join(' ');
      const props = {
        cls, lanesAB: ab, lanesBA: ba, speed, name: t.name || t.ref || '', status: 'existing', ring,
        busLane: /(^|\|)designated/.test(t['bus:lanes'] || '') || /bus/.test(t['lanes:psv'] || ''),
        cycle: /track|separate/.test(cyc) ? 'separated' : /lane|shared_lane/.test(cyc) ? 'painted' : 'none',
      };
      if (props.busLane && Math.max(ab, ba) < 2) props.busLane = false;
      const addSeg = (seg, key) => {
        const a = getNode(seg[0]), b = getNode(seg[seg.length - 1]);
        if (a === b) return;
        const pts = seg.slice(1, -1).map(id => { const o = N.get(id); return [o.lat, o.lon]; });
        Net.addLink(sc, a, b, pts, { ...props, osm: key });
        added++; touched.add(a); touched.add(b);
        if (ring) { ringNodes.add(a); ringNodes.add(b); }
      };
      const ids = w.nodes.filter(id => N.has(id));
      let s = 0;
      for (let i = 1; i < ids.length; i++) {
        if (i < ids.length - 1 && (use.get(ids[i]) || 0) < 2) continue;
        const seg = ids.slice(s, i + 1), key = w.id + ':' + s;
        s = i;
        if (have.has(key) || have.has(key + 'a')) continue;
        if (seg[0] === seg[seg.length - 1]) {
          if (seg.length < 4) continue;
          const m = seg.length >> 1;
          addSeg(seg.slice(0, m + 1), key + 'a'); addSeg(seg.slice(m), key + 'b');
        } else addSeg(seg, key);
      }
    }
    // Junction control from tags and geometry; motorway-only nodes (merges, diverges) stay uncontrolled
    const deg = Net.degree(sc), fwy = new Map();
    for (const Lk of Object.values(sc.links)) for (const id of [Lk.a, Lk.b]) fwy.set(id, (fwy.get(id) ?? true) && CLASSES[Lk.cls].rank >= 5);
    for (const nid of touched) {
      const n = sc.nodes[nid], d = deg.get(nid) || 0, tg = (N.get(n.osm) || {}).tags || {};
      if (ringNodes.has(nid) && d >= 3) n.control = 'roundabout';
      else if (d >= 3 && n.control === 'none' && !fwy.get(nid)) n.control = 'priority';
      if (tg.highway === 'stop' && d >= 3) n.control = 'stop';
      if (tg.highway === 'traffic_signals') n.control = 'signals';
    }
    // Signals are often tagged on the stop line just before a junction: move them onto the junction
    const junctions = [...touched].filter(id => (deg.get(id) || 0) >= 3);
    for (const [oid, o] of N) {
      if (!o.tags || o.tags.highway !== 'traffic_signals') continue;
      const own = byOsm.get(oid);
      if (own && (deg.get(own) || 0) >= 3) continue;
      let best = null, bd = 45;
      for (const j of junctions) {
        const n = sc.nodes[j], d = U.dist([o.lat, o.lon], [n.lat, n.lng]);
        if (d < bd) { bd = d; best = j; }
      }
      if (best) sc.nodes[best].control = 'signals';
    }
    // A signal tagged on a plain road bend is a pedestrian crossing; keep it only if it splits a link
    for (const nid of touched) { const n = sc.nodes[nid]; if (n.control === 'signals' && (deg.get(nid) || 0) < 2) n.control = 'none'; }
    if (opts.sites === true && added) Sites.detect(sc);
    return added;
  },

  // External zones where roads leave the imported area, loaded with indicative through traffic
  // only (optional Set of node ids): consider just these dead ends, e.g. the nodes one import added, so
  // roads imported earlier somewhere else never get gateways for this area
  autoGateways(sc, b, only = null) {
    const adj = new Map();
    for (const L of Object.values(sc.links)) for (const n of [L.a, L.b]) { if (!adj.has(n)) adj.set(n, []); adj.get(n).push(L); }
    const W = b.getEast() - b.getWest(), H = b.getNorth() - b.getSouth(), mx = W * 0.05, my = H * 0.05;
    const pinned = new Set(Object.values(sc.zones).map(z => z.node).filter(Boolean));
    let made = 0;
    for (const [nid, Ls] of adj) {
      if (Ls.length !== 1 || pinned.has(nid)) continue;
      const n = sc.nodes[nid], L = Ls[0], C = CLASSES[L.cls];
      if (C.rank < 3 || L.status === 'closed') continue;
      if (only && !only.has(nid)) continue;
      const near = n.lng - b.getWest() < mx || b.getEast() - n.lng < mx || n.lat - b.getSouth() < my || b.getNorth() - n.lat < my;
      if (!near) continue;
      const inLanes = L.a === nid ? L.lanesAB : L.lanesBA, outLanes = L.a === nid ? L.lanesBA : L.lanesAB;
      const per = { motorway: 1500, ramp: 650, arterial: 550, collector: 320 }[L.cls] || 250;
      const inb = Math.round(inLanes * per * 0.85 / 10) * 10, outb = Math.round(outLanes * per * 0.85 / 10) * 10;
      // place the gateway marker 60 m beyond the road end
      const g = Net.geom(sc, L), far = L.a === nid ? g[1] : g[g.length - 2];
      const dLat = n.lat - far[0], dLng = n.lng - far[1], m = Math.hypot(dLat, dLng * Math.cos(n.lat * Math.PI / 180)) || 1;
      const k = 60 / 111000 / m;
      const id = U.uid('z');
      sc.zones[id] = { id, kind: 'gateway', name: (L.name || C.label) + ' gateway', lat: U.r7(n.lat + dLat * k), lng: U.r7(n.lng + dLng * k), node: nid, gw: { AMin: inb, AMout: outb, PMin: inb, PMout: outb } };
      pinned.add(nid); made++;
    }
    return made;
  },

  importGeoJSON(sc, gj, template) {
    const feats = gj.type === 'FeatureCollection' ? gj.features : gj.type === 'Feature' ? [gj] : [{ type: 'Feature', geometry: gj, properties: {} }];
    const snap = ll => Net.nearestNode(sc, ll, 2) || Net.addNode(sc, ll);
    let added = 0;
    for (const f of feats) {
      const g = f.geometry; if (!g) continue;
      const lines = g.type === 'LineString' ? [g.coordinates] : g.type === 'MultiLineString' ? g.coordinates : [];
      const p = f.properties || {};
      for (const line of lines) {
        if (line.length < 2) continue;
        const ll = line.map(c => [c[1], c[0]]);
        const a = snap(ll[0]), b = snap(ll[ll.length - 1]);
        if (a === b) continue;
        const cls = CLASSES[p.cls] ? p.cls : template.cls;
        Net.addLink(sc, a, b, ll.slice(1, -1), {
          ...template, cls,
          name: p.name || '', speed: +p.speed || template.speed,
          lanesAB: p.lanesAB != null ? +p.lanesAB : template.lanesAB, lanesBA: p.lanesBA != null ? +p.lanesBA : template.lanesBA,
          status: STATUS[p.status] ? p.status : template.status,
        });
        added++;
      }
    }
    Net.autoControl(sc, Object.keys(sc.nodes));
    return added;
  },

  geojson(sc, res) {
    const features = [];
    for (const L of Object.values(sc.links)) {
      const g = Net.geom(sc, L), a = res && res.arcs[L.id + ':ab'], b = res && res.arcs[L.id + ':ba'];
      features.push({
        type: 'Feature', geometry: { type: 'LineString', coordinates: g.map(p => [p[1], p[0]]) },
        properties: {
          id: L.id, name: L.name, cls: L.cls, status: L.status, speed: L.speed, lanesAB: L.lanesAB, lanesBA: L.lanesBA,
          busLane: L.busLane, cycle: L.cycle, length_m: Math.round(U.polyLen(g)),
          vol_ab: a ? a.v : null, vc_ab: a ? a.vc : null, spd_ab: a ? a.spd : null,
          vol_ba: b ? b.v : null, vc_ba: b ? b.vc : null, spd_ba: b ? b.spd : null,
        },
      });
    }
    for (const n of Object.values(sc.nodes)) if (n.control !== 'none') {
      const r = res && res.nodes[n.id];
      features.push({ type: 'Feature', geometry: { type: 'Point', coordinates: [n.lng, n.lat] }, properties: { id: n.id, control: n.control, delay_s: r ? r.delay : null, los: r ? r.los : null } });
    }
    for (const z of Object.values(sc.zones)) features.push({ type: 'Feature', geometry: { type: 'Point', coordinates: [z.lng, z.lat] }, properties: { id: z.id, zone: z.name, kind: z.kind, ...(z.lu || {}), ...(z.gw || {}) } });
    return JSON.stringify({ type: 'FeatureCollection', features }, null, 1);
  },

  linksCSV(sc, res) {
    const rows = [['link_id', 'name', 'class', 'status', 'direction', 'lanes', 'speed_limit_kmh', 'length_m', 'volume_veh_h', 'volume_pcu_h', 'capacity_pcu_h', 'v_c', 'los', 'speed_kmh', 'travel_time_s', 'junction_delay_s']];
    for (const L of Object.values(sc.links)) for (const d of ['ab', 'ba']) {
      const lanes = d === 'ab' ? L.lanesAB : L.lanesBA; if (!lanes) continue;
      const r = res && res.arcs[L.id + ':' + d];
      rows.push([L.id, L.name, L.cls, L.status, d.toUpperCase(), lanes, L.speed, Math.round(Net.len(sc, L)), r && r.v, r && r.pcu, r && r.cap, r && r.vc, r && r.los, r && r.spd, r && r.t, r && r.nd]);
    }
    return U.csv(rows);
  },
};

/* ---------------- assessment scenarios (docs/spec/scenario-matrix, docs/regulatory/reg-receiving-env) ----------------
   Scenario fields: role (SCENARIO_ROLES), year (Engine.yearOf), buildout (% of the development zones built, staging),
   periods (['AM', 'PM', …]), code ('S0', 'S1' …), setId (the assessment set it belongs to), network ('existing' |
   'committed' | 'mitigation': what the wizard put in). Links carry stage (LINK_STAGES; default from status) and, for
   committed projects, funding (FUNDING). Zones carry role (ZONE_ROLES; default: existing if z.existing, otherwise
   development) and optionally reg (a register row id) and buildout (% of that zone built by the scenario year).
   Committed developments and network projects are listed in P.register (Register). */
const SCENARIO_ROLES = {
  existing:    { label: 'Existing (base year)', short: 'Existing', code: 'S0', order: 0, side: 'without' },
  baseline:    { label: 'Future baseline (do-minimum)', short: 'Baseline', code: 'S1', order: 1, side: 'without' },
  proposal:    { label: 'With proposal (option)', short: 'With proposal', code: 'S2', order: 2, side: 'with' },
  mitigation:  { label: 'With proposal and mitigation (option)', short: 'With mitigation', code: 'S3', order: 3, side: 'with' },
  sensitivity: { label: 'Sensitivity test', short: 'Sensitivity', code: 'ST', order: 4, side: 'other' },
  permitted:   { label: 'Permitted baseline (RMA s104(2): may disregard)', short: 'Permitted baseline', code: 'PB', order: 5, side: 'without' },
};
const ZONE_ROLES = { existing: 'Existing land use', committed: 'Committed development (consented, not yet built)', development: 'Proposed development' };
const LINK_STAGES = { existing: 'Existing road', committed: 'Committed network project', unfunded: 'Unfunded network project', proposal: 'Part of the proposal', mitigation: 'Mitigation' };
const FUNDING = { funded: 'Funded (NLTP, RLTP or LTP)', programmed: 'Programmed, not yet funded', unfunded: 'Unfunded' };
const PATHWAYS = { consent: 'Resource consent', planchange: 'Plan change', nor: 'Notice of requirement or designation', ftaa: 'Fast-track application' };

/* The committed-development register (reg-receiving-env). Rows: { id, ref, name, type (REGISTER_TYPES), yield,
   rates, status, likely ('likely' | 'possible' | 'unlikely'), funding (network projects), source, date, asAt,
   verify, preset, zones [zone ids], links [link ids], include (true / false: the author's override), note }. */
const REGISTER_TYPES = { consent: 'Granted resource consent', ftaa: 'Fast-track (FTAA) approval', planchange: 'Operative plan change', precinct: 'Precinct yield', network: 'Committed network project' };
const Register = {
  /* South Auckland preset rows, as stated in Spare Construct's regulatory spec (docs/regulatory/reg-receiving-env.md,
     8 Oct 2026). They have not been checked against primary sources: each is tagged verify, with its as-at date. */
  presets(asAt = '2026-10-08') {
    const src = 'Spare Construct regulatory spec (reg-receiving-env), not checked against primary sources';
    const row = (id, name, type, extra) => ({ id: 'preset:' + id, ref: '', name, type, yield: null, rates: null, status: '', likely: 'possible', funding: null,
      source: src, date: asAt, asAt, verify: true, preset: true, zones: [], links: [], ...extra });
    /* Mill Road: NZTA designation 6786 (Mill Road Redoubt Road Spare Construct Project). Its lapse-date extension under RMA
       s184 is listed as confirmed, plan updated 21/09/2026, in Auckland Council's AUP plan modification schedule
       (docs/references/aup-modification-schedule-2026.pdf, checked). The designation is on record; funding is not. */
    const mill = { id: '6786', name: 'Mill Road Redoubt Road Spare Construct Project', authority: 'New Zealand Transport Agency', lapseExtension: 'confirmed', confirmed: '2026-09-21',
      basis: 'RMA s184 extension of the lapse date, confirmed and plan updated 21/09/2026', source: 'Auckland Council, Auckland Unitary Plan plan modification schedule (2026)', verified: true };
    const millNote = 'Designation 6786 (NZTA): lapse-date extension confirmed 21 Sep 2026 (AUP plan modification schedule).';
    return [
      row('mill-road-1', 'Mill Road Stage 1', 'network', { status: 'Fast-track', funding: 'unfunded', designation: { ...mill }, note: `Fast-track; construction funding not committed. ${millNote}` }),
      row('mill-road-2-3', 'Mill Road Stages 2–3', 'network', { funding: 'unfunded', designation: { ...mill }, note: millNote }),
      row('westfield-pukekohe', 'Westfield–Pukekohe four-tracking', 'network', { note: 'Funding status to be confirmed.' }),
      row('sunfield', 'Sunfield', 'ftaa', { status: 'Secondary-source status; High Court appeal' }),
      row('drury-metro', 'Drury Metropolitan Centre', 'precinct'),
    ];
  },
  // Whether a row belongs in the future baseline (do-minimum), and why (AT ITA Guidelines s3.6.1)
  inBaseline(row) {
    if (!row) return { included: false, reason: 'Not in the register.' };
    if (row.include === true || row.include === false) return { included: row.include, reason: `Author's decision${row.note ? ': ' + row.note : ''}.` };
    if (row.type === 'network') {
      if (row.funding === 'funded') return { included: true, reason: 'Funded network project.' };
      return { included: false, reason: row.funding === 'programmed' ? 'Programmed but not funded: sensitivity scenarios only (AT ITA s3.6.1).'
        : row.funding === 'unfunded' ? 'Unfunded: sensitivity scenarios only (AT ITA s3.6.1).' : 'Funding status not recorded: confirm it before including it.' };
    }
    if (!/grant|approv|operative/i.test(row.status || '')) return { included: false, reason: `Not granted (${row.status || 'status not recorded'}).` };
    if (row.likely !== 'likely') return { included: false, reason: `Granted, but implementation is rated ${row.likely || 'not assessed'}.` };
    return { included: true, reason: 'Granted and likely to be implemented.' };
  },
  // Report section 2.8: every row, with 'Included in S1' and the reason
  table(P = Store.P) {
    return (P.register || []).map(r => {
      const b = this.inBaseline(r);
      return { id: r.id, name: r.name, type: REGISTER_TYPES[r.type] || r.type, status: r.status || '', funding: r.funding ? FUNDING[r.funding] || r.funding : '',
        likely: r.likely || '', source: r.source || '', asAt: r.asAt || r.date || '', verify: !!r.verify, included: b.included, reason: b.reason };
    });
  },
};

const Scenarios = {
  roleOf: sc => (sc && SCENARIO_ROLES[sc.role] ? sc.role : null),
  linkStage(L) {
    if (L && LINK_STAGES[L.stage]) return L.stage;
    return L && L.status === 'proposed' ? 'proposal' : 'existing';
  },
  // Unfunded network projects (stage 'unfunded', or committed with funding 'unfunded'): sensitivity scenarios only.
  // Programmed-but-not-funded projects also stay out of the do-minimum by default, with a warning rather than a block.
  unfunded(L) { const s = this.linkStage(L); return s === 'unfunded' || (s === 'committed' && L.funding === 'unfunded'); },
  programmed(L) { return this.linkStage(L) === 'committed' && L.funding === 'programmed'; },
  zoneRole(z) {
    if (!z || z.kind === 'gateway') return 'external';
    if (ZONE_ROLES[z.role]) return z.role;
    return z.existing === true ? 'existing' : 'development';
  },
  // A committed zone belongs in the baseline unless its register row says otherwise
  zoneInBaseline(z, P = Store.P) {
    if (!z.reg) return true;
    return Register.inBaseline((P.register || []).find(r => r.id === z.reg)).included;
  },
  // Default design year by pathway (NZTA RR 422 s5.5; AT ITA 3.6)
  designYear(type, opening) {
    const D = (typeof Engine !== 'undefined' && Engine.STANDARDS.design) || { consent: 5, planchange: 10, nor: 10, ftaa: 5 };
    return (+opening || 0) + (D[type] != null ? D[type] : D.consent);
  },
  /* The 'Create assessment set' wizard: S0 existing (count year), then for the opening and design years
     S1 future baseline (existing + committed developments and funded projects), S2 = S1 + proposal,
     S3 = S2 + mitigation. Unfunded and not-granted items stay out of S1 and are listed in excluded.
     opts: { type (PATHWAYS), countYear, opening, design, from (scenario id), mitigation (true), periods, buildout: { year: % } }.
     Adds the scenarios to P and returns { scenarios, excluded, warnings, set }. */
  createSet(P = Store.P, opts = {}) {
    const S = P.settings, type = PATHWAYS[opts.type] ? opts.type : 'consent';
    const src = P.scenarios.find(s => s.id === (opts.from || P.active)) || P.scenarios[0];
    const countYear = +opts.countYear || +S.baseYear, opening = +opts.opening || countYear;
    const design = +opts.design || this.designYear(type, opening);
    const periods = Array.isArray(opts.periods) && opts.periods.length ? opts.periods.slice() : ['AM', 'PM'];
    const setId = U.uid('set'), years = [...new Set([opening, design])].sort((a, b) => a - b), out = [], excluded = [], warnings = [];
    for (const L of Object.values(src.links)) if ((this.unfunded(L) || this.programmed(L)) && L.status !== 'closed')
      excluded.push({ kind: 'link', id: L.id, name: L.name || 'Unnamed road', stage: this.linkStage(L), funding: L.funding || null,
        reason: L.funding === 'programmed' ? 'Programmed but not funded: sensitivity scenarios only (AT ITA s3.6.1).' : 'Unfunded: sensitivity scenarios only (AT ITA s3.6.1).' });
    for (const z of Object.values(src.zones)) if (this.zoneRole(z) === 'committed' && !this.zoneInBaseline(z, P))
      excluded.push({ kind: 'zone', id: z.id, name: z.name || 'Unnamed zone', reason: Register.inBaseline((P.register || []).find(x => x.id === z.reg)).reason });
    const make = (role, year, net, lu, buildout) => {
      const R = SCENARIO_ROLES[role];
      const sc = this.copy(src, {
        links: L => net.has(this.linkStage(L)) && !this.unfunded(L) && !this.programmed(L),
        zones: z => { const r = this.zoneRole(z); return r === 'external' || (lu.has(r) && (r !== 'committed' || this.zoneInBaseline(z, P))); },
      });
      Object.assign(sc, { name: `${R.code} · ${year} ${R.short.toLowerCase()}`, role, year, periods: periods.slice(), code: R.code, setId,
        network: net.has('mitigation') ? 'mitigation' : net.has('committed') ? 'committed' : 'existing', notes: `Created by the assessment-set wizard (${PATHWAYS[type]}).` });
      if (buildout != null) sc.buildout = buildout;
      out.push(sc);
    };
    const ex = new Set(['existing']), base = new Set(['existing', 'committed']), prop = new Set([...base, 'proposal']), mit = new Set([...prop, 'mitigation']);
    const luBase = new Set(['existing', 'committed']), luDev = new Set([...luBase, 'development']);
    make('existing', countYear, ex, new Set(['existing']));
    for (const y of years) {
      const b = opts.buildout && opts.buildout[y] != null ? U.clamp(+opts.buildout[y], 0, 100) : 100;
      make('baseline', y, base, luBase);
      make('proposal', y, prop, luDev, b);
      if (opts.mitigation !== false) make('mitigation', y, mit, luDev, b);
    }
    const ahead = design - (+S.baseYear || countYear), warnAhead = (typeof Engine !== 'undefined' && Engine.STANDARDS.design.warnAhead) || 10;
    if (ahead > warnAhead && !(P.data && P.data.msm && P.data.msm.validated))
      warnings.push(`The design year ${design} is ${ahead} years after the base year and no validated regional forecast is loaded. Check forecasts more than ${warnAhead} years ahead against MSM or Auckland Forecasting Centre outputs.`);
    if (opening < countYear) warnings.push(`The opening year ${opening} is before the count year ${countYear}. Check both years.`);
    P.scenarios.push(...out);
    P.assessment = { id: setId, type, pathway: PATHWAYS[type], countYear, opening, design, periods, created: new Date().toISOString(), excluded, warnings, scenarios: out.map(s => s.id) };
    return { scenarios: out, excluded, warnings, set: P.assessment };
  },
  // A sensitivity copy of a scenario; opts.unfunded puts the unfunded projects back in
  sensitivityOf(P, scId, opts = {}) {
    const src = P.scenarios.find(s => s.id === scId); if (!src) return null;
    const sc = this.copy(src, { links: L => opts.unfunded || !this.unfunded(L) });
    Object.assign(sc, { name: `${src.name} · sensitivity${opts.unfunded ? ' (unfunded projects in)' : ''}`, role: 'sensitivity', sensOf: src.id });
    for (const k of ['year', 'periods', 'buildout']) if (src[k] != null) sc[k] = JSON.parse(JSON.stringify(src[k]));
    P.scenarios.push(sc);
    return sc;
  },
  // A copy of a scenario with only the links and zones that pass the filters (no results; ids kept so results compare link by link)
  copy(src, f = {}) {
    const sc = newScenario(src.name);
    const c = JSON.parse(JSON.stringify({ nodes: src.nodes, links: src.links, zones: src.zones, routes: src.routes, sites: src.sites || null, odManual: src.odManual || null }));
    Object.assign(sc, { nodes: c.nodes, links: c.links, zones: c.zones, routes: c.routes, odManual: c.odManual });
    if (c.sites) sc.sites = c.sites;
    const gone = [];
    if (f.links) for (const L of Object.values(sc.links)) if (!f.links(L)) { delete sc.links[L.id]; gone.push(L.a, L.b); }
    if (f.zones) for (const z of Object.values(sc.zones)) if (!f.zones(z)) delete sc.zones[z.id];
    for (const n of gone) Net.pruneNode(sc, n);
    if (sc.sites) for (const s of Object.values(sc.sites)) { s.nodes = s.nodes.filter(id => sc.nodes[id]); if (s.nodes.length < 2) delete sc.sites[s.id]; }
    return sc;
  },
  // What a scenario contains, in words, with its year and growth basis
  describe(sc, P = Store.P) {
    const S = P.settings, role = this.roleOf(sc), R = role ? SCENARIO_ROLES[role] : null, E = typeof Engine !== 'undefined' ? Engine : null;
    const lu = new Set(Object.values(sc.zones).map(z => this.zoneRole(z)).filter(r => r !== 'external'));
    const st = new Set(Object.values(sc.links).filter(L => L.status !== 'closed').map(L => this.linkStage(L)));
    const year = E ? E.yearOf(sc, S) : +sc.year || S.year;
    const landUse = ['existing', 'committed', 'development'].filter(r => lu.has(r))
      .map((r, i) => (i ? r : r[0].toUpperCase() + r.slice(1)) + (r === 'development' && sc.buildout != null && +sc.buildout !== 100 ? ` (${+sc.buildout}% built)` : '')).join(' + ') || 'None';
    const NW = { existing: 'Existing', committed: 'committed (funded)', unfunded: 'unfunded', proposal: 'proposal', mitigation: 'mitigation' };
    const network = ['existing', 'committed', 'unfunded', 'proposal', 'mitigation'].filter(r => st.has(r)).map(r => NW[r]).join(' + ') || 'None';
    const gb = E ? E.growthBasis(S) : '', gf = E ? E.growth(S, sc) : null;
    return { id: sc.code || '', scId: sc.id, name: sc.name, role, roleLabel: R ? R.label : 'Role not set', year, landUse, network,
      periods: (sc.periods || ['AM', 'PM']).join(', '), buildout: lu.has('development') ? (sc.buildout != null ? +sc.buildout : 100) : null,
      growthBasis: gb, growthFactor: gf, text: `${year}: background growth ${gb}${gf != null ? ` (× ${gf.toFixed(3)} from ${S.baseYear})` : ''}` };
  },
  // Table 'Assessment scenarios' (ID | Role | Year | Land use | Network | Periods | Build-out %), by year, then without / with
  table(P = Store.P) {
    const ord = r => (r.role ? SCENARIO_ROLES[r.role].order : 9);
    return P.scenarios.map(sc => this.describe(sc, P)).sort((a, b) => a.year - b.year || ord(a) - ord(b));
  },
  groups(P = Store.P) {
    const g = new Map();
    for (const r of this.table(P)) {
      if (!g.has(r.year)) g.set(r.year, { year: r.year, without: [], with: [], other: [] });
      g.get(r.year)[r.role ? SCENARIO_ROLES[r.role].side : 'other'].push(r.scId);
    }
    return [...g.values()];
  },
  /* Checks on a scenario (reg-receiving-env): [{ level: 'block' | 'warn' | 'info', code, id?, text }]. An unfunded
     project outside a sensitivity scenario blocks; proposed or not-granted development in a baseline warns. */
  check(sc, P = Store.P) {
    const out = [], role = this.roleOf(sc), S = P.settings;
    if (role !== 'sensitivity') {
      for (const L of Object.values(sc.links)) {
        if (L.status === 'closed') continue;
        if (this.unfunded(L)) out.push({ level: 'block', code: 'unfunded', id: L.id, text: `${L.name || 'An unnamed road'} is an unfunded project, and it is in ${sc.name}. AT ITA Guidelines s3.6.1 allow unfunded projects only in sensitivity scenarios. Remove it from this scenario, or make the scenario a sensitivity test.` });
        else if (this.programmed(L) && role !== 'existing') out.push({ level: 'warn', code: 'programmed', id: L.id, text: `${L.name || 'An unnamed road'} is programmed but not yet funded. Keep it out of the do-minimum unless its funding is confirmed (record the source), or test it as a sensitivity.` });
      }
      for (const r of P.register || []) {
        if (r.type !== 'network' || Register.inBaseline(r).included) continue;
        const ids = (r.links || []).filter(id => sc.links[id] && sc.links[id].status !== 'closed');
        if (ids.length && !out.some(o => ids.includes(o.id))) out.push({ level: 'block', code: 'unfunded', id: r.id, text: `${r.name} (register) is in ${sc.name} but is not funded. Use it in a sensitivity scenario only (AT ITA s3.6.1).` });
      }
    }
    if (role === 'baseline' || role === 'existing') for (const z of Object.values(sc.zones)) {
      const zr = this.zoneRole(z);
      if (zr === 'development') out.push({ level: 'warn', code: 'dev-in-baseline', id: z.id, text: `${z.name || 'A zone'} is proposed development, but this is the ${SCENARIO_ROLES[role].short.toLowerCase()} scenario. Mark it committed (with its consent in the register) or remove it.` });
      else if (zr === 'committed' && !this.zoneInBaseline(z, P)) out.push({ level: 'warn', code: 'not-granted', id: z.id, text: `${z.name || 'A zone'} is committed development that is not granted and likely (see the register). The do-minimum should include only granted and likely developments.` });
    }
    if (role === 'permitted') out.push({ level: 'info', code: 'permitted', text: 'Permitted baseline: under RMA s104(2) the consent authority may disregard these effects.' });
    const yr = typeof Engine !== 'undefined' ? Engine.yearOf(sc, S) : +sc.year || S.year;
    if (yr - S.baseYear > 10 && !(P.data && P.data.msm && P.data.msm.validated)) out.push({ level: 'warn', code: 'far-year', text: `${yr} is more than 10 years after the base year and no validated regional forecast is loaded.` });
    return out;
  },
};

/* ---------------- junction sites (docs/spec/junction-sites) ----------------
   sc.sites[id] = { id, nodes: [node ids], control, cycle, phases, name, atSiteNo, fixedTiming, lanes: {}, src: {} }.
   Detection (union-find): candidate nodes have a control, or 3+ links with one of collector rank or higher, and no
   motorway or ramp link (interchange terminals are never joined). Two candidates join along a link of 35 m or less;
   up to 60 m when both are signals, when the link is one carriageway of a dual pair (one-way, with an opposite
   one-way link within 15° nearby), or when both are within 30 m of the same AT controlled-intersection point.
   A roundabout's ring nodes join along the ring and are never merged with a junction beside it. No site is wider
   than 80 m. Sites made by hand (src.nodes 'user') are kept; detect() replaces only the ones it made before. */
const Sites = {
  RULES: { near: 35, wide: 60, at: 30, diameter: 80, dualBearing: 15, dualGap: 60, source: 'Spare Construct junction-site rules (docs/spec/junction-sites)', basis: 'Spare Construct assumption' },
  of(sc, nodeId) { for (const s of Object.values(sc.sites || {})) if (s.nodes.includes(nodeId)) return s; return null; },
  // AT controlled-intersection points (DataHub.loadControlled), if loaded
  points(P = Store.P) { return P && P.data && Array.isArray(P.data.controlled) ? P.data.controlled : []; },
  detect(sc, opts = {}) {
    const R = { ...this.RULES, ...(opts.rules || {}) }, nodes = sc.nodes, inc = new Map();
    sc.sites ||= {};
    const keep = Object.values(sc.sites).filter(s => !(s.src && s.src.nodes === 'auto')), kept = new Set(keep.flatMap(s => s.nodes));
    for (const s of Object.values(sc.sites)) if (s.src && s.src.nodes === 'auto') delete sc.sites[s.id];
    const links = Object.values(sc.links).filter(L => L.status !== 'closed' && nodes[L.a] && nodes[L.b] && L.a !== L.b);
    for (const L of links) for (const n of [L.a, L.b]) { const a = inc.get(n); if (a) a.push(L); else inc.set(n, [L]); }
    const rank = L => (CLASSES[L.cls] || CLASSES.local).rank, ring = new Set();
    for (const L of links) if (L.ring) { ring.add(L.a); ring.add(L.b); }
    const cand = new Set();
    for (const [id, Ls] of inc) {
      if (kept.has(id) || Ls.some(L => rank(L) >= 5)) continue;
      if ((nodes[id].control || 'none') !== 'none' || (Ls.length >= 3 && Math.max(...Ls.map(rank)) >= 3)) cand.add(id);
    }
    const ll = id => [nodes[id].lat, nodes[id].lng], lenC = new Map(), len = L => { if (!lenC.has(L.id)) lenC.set(L.id, Net.len(sc, L)); return lenC.get(L.id); };
    const oneway = L => !((L.lanesAB | 0) > 0 && (L.lanesBA | 0) > 0);
    const brg = L => { const g = Net.geom(sc, L); return (L.lanesAB | 0) > 0 ? U.bearing(g[0], g[g.length - 1]) : U.bearing(g[g.length - 1], g[0]); };
    const mid = L => { const g = Net.geom(sc, L); return U.along(g, U.cumLen(g), len(L) / 2).p; };
    const ow = links.filter(L => oneway(L) && cand.has(L.a) && cand.has(L.b) && len(L) <= R.wide);
    const dual = L => oneway(L) && ow.some(M => M !== L && U.angDiff(brg(L), brg(M)) >= 180 - R.dualBearing && U.dist(mid(L), mid(M)) <= R.dualGap);
    const pts = this.points(opts.P || Store.P), nearAT = id => pts.filter(p => U.dist(ll(id), [p.lat, p.lng]) <= R.at).map(p => p.no ?? p.id);
    const sameAT = (a, b) => { const x = nearAT(a); return x.length > 0 && nearAT(b).some(k => x.includes(k)); };
    // union-find, refusing a join that would make the site wider than R.diameter
    const par = new Map([...cand].map(id => [id, id])), mem = new Map([...cand].map(id => [id, [id]]));
    const find = id => { while (par.get(id) !== id) { par.set(id, par.get(par.get(id))); id = par.get(id); } return id; };
    const join = (a, b) => {
      const x = find(a), y = find(b); if (x === y) return false;
      const all = [...mem.get(x), ...mem.get(y)];
      for (let i = 0; i < all.length; i++) for (let j = i + 1; j < all.length; j++) if (U.dist(ll(all[i]), ll(all[j])) > R.diameter) return false;
      par.set(y, x); mem.set(x, all); mem.delete(y); return true;
    };
    const order = links.filter(L => cand.has(L.a) && cand.has(L.b)).map(L => ({ L, d: len(L) })).sort((p, q) => p.d - q.d);
    for (const { L, d } of order) {
      const ra = ring.has(L.a), rb = ring.has(L.b);
      if (L.ring && ra && rb) { join(L.a, L.b); continue; }
      if (ra || rb) continue;   // a roundabout is not merged with a junction beside it
      const sig = nodes[L.a].control === 'signals' && nodes[L.b].control === 'signals';
      if (d <= R.near || (d <= R.wide && (sig || dual(L) || sameAT(L.a, L.b)))) join(L.a, L.b);
    }
    let made = 0, inSites = 0;
    for (const m of mem.values()) {
      if (m.length < 2) continue;
      const id = U.uid('j'), at = [...new Set(m.flatMap(nearAT))];
      const site = { id, nodes: m.slice().sort(), control: null, cycle: null, phases: null, name: '', atSiteNo: at.length === 1 ? at[0] : at.length ? at : null, fixedTiming: false, lanes: {},
        src: { nodes: 'auto', at: new Date().toISOString().slice(0, 10), rule: R.source, ...(at.length ? { atSiteNo: 'AT controlled intersections' } : {}) } };
      site.name = this.name(sc, site);
      sc.sites[id] = site; made++; inSites += m.length;
    }
    return { sites: made, nodes: inSites, kept: keep.length };
  },
  // 'Great South Road / Waihoehoe Road': the roads leaving the site, highest class first
  name(sc, site) {
    if (site.name) return site.name;
    const ms = new Set(site.nodes), ext = Object.values(sc.links).filter(L => L.status !== 'closed' && ms.has(L.a) !== ms.has(L.b) && L.name);
    ext.sort((p, q) => (CLASSES[q.cls] || CLASSES.local).rank - (CLASSES[p.cls] || CLASSES.local).rank || p.name.localeCompare(q.name));
    return [...new Set(ext.map(L => L.name))].slice(0, 3).join(' / ') || 'Unnamed junction';
  },
  // Merge sites and / or nodes into one site made by hand; returns its id (null if fewer than two nodes)
  merge(sc, ids) {
    sc.sites ||= {};
    const nodes = new Set(), old = [];
    for (const id of ids) {
      const s = sc.sites[id] || (sc.nodes[id] ? this.of(sc, id) : null);
      if (s) { if (!old.includes(s)) { old.push(s); s.nodes.forEach(n => nodes.add(n)); } }
      else if (sc.nodes[id]) nodes.add(id);
    }
    if (nodes.size < 2) return null;
    const first = old[0] || {}, id = U.uid('j');
    for (const s of old) delete sc.sites[s.id];
    sc.sites[id] = { id, nodes: [...nodes].sort(), control: first.control || null, cycle: first.cycle || null, phases: first.phases || null, name: '', atSiteNo: first.atSiteNo ?? null,
      fixedTiming: !!first.fixedTiming, lanes: { ...(first.lanes || {}) }, src: { nodes: 'user', at: new Date().toISOString().slice(0, 10) } };
    return id;
  },
  // Split a site back into separate nodes
  split(sc, siteId) { if (!sc.sites || !sc.sites[siteId]) return false; delete sc.sites[siteId]; return true; },
};
