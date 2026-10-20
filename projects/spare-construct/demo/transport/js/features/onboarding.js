'use strict';
/* Spare Construct · the first ten minutes and expert speed (docs/spec/onboarding, multi-edit, scenario-consistency,
   check-network, command-palette, zone-gateway-ux). Everything goes in through Registry:
   - Place search in the top bar and the welcome dialog: Photon typeahead (3+ characters, 300 ms debounce,
     aborted when a newer search replaces it, cached). OpenStreetMap Nominatim (Search.geocode) is asked only on
     Enter and only when Photon has no hit, never while typing (its usage policy).
   - The Study area tool (A): drag a box, or click to drop a 1.5 × 1.5 km square, then drag its corners. 'Set up
     this area' runs StudyArea.setup (roads, gateways, counts and crashes, existing land use, gateway flows), then
     the NZTA legal speed limits (feeds lane) and the 2023 Census, employee counts and schools (land-use lane) when
     those lanes are loaded, then Check network, then the base AM and PM runs (and, if asked, the fit to counts),
     ticking each step. The card's readout counts the roads OpenStreetMap holds in the box (an Overpass count).
   - Multi-select: Shift+click toggles roads and junctions, Shift+drag box-selects roads, and the road panel's
     'Select whole street'. The 'multi' panel edits class, status, speed, lanes, cycling and junction control for
     all of them under one undo step, in this scenario and any others ticked under 'Edits apply to'.
   - Scenario consistency: 'Edits apply to' in every road, junction, zone and gateway panel (an edit, or a new
     road, zone or gateway, goes to the ticked scenarios too), differences from the base with 'Reset to base', a
     one-off copy of an object to other scenarios, the 'Option changes' tab, CSV and report section (chapter 3),
     zone Duplicate (Ctrl+D) and 'Copy land use from'. Roads split where a new road joins them are handled in all.
   - Check network: issues in View.issues = [{ code, level, msg, ids: ['link:id' …], at }], drawn on the map and
     listed in the Network check tab; a click selects and flies to them. It also runs after a model run that
     leaves trips unserved.
   - The command palette (Ctrl+K or /) and the extra shortcuts; the zone placement card; gateway count source.
   Deviations from the spec, and why:
   - The selection is { type: 'multi', id: 'multi', links, nodes, ids } (ids = links), not { type: 'links' },
     because the multi panel also sets junction control.
   - Set-up order follows StudyArea: zones are filled before gateway flows are set from counts, and Check
     network runs before the base model so its issues show before the results. The steps say what they actually
     use: without the feeds or land-use lane there are no legal speed limits or Census figures.
   - 'Edits apply to' copies deletions only from the multi panel, after a confirmation; bulk changes (more than
     MIRROR_MAX objects: imports, set-ups) stay in the scenario they were made in.
   - The zone placement card replaces the built-in zone tool's click by listening first (capture phase) on the
     map, because ui.js belongs to another lane. When the land-use lane (LandusePlus.placeZone) is loaded, its
     card is used instead of ours, and its gateway count-source section replaces ours, so neither shows twice.
     'Check network' is a Menu item and command, not in Run ▾.
   - Number keys for colour modes, Ctrl+Shift+Enter and Ctrl+Shift+S are the shell's own (bound) shortcuts.
   - Site leg collisions are judged from the roads leaving each junction site, grouped by name and bearing. */

const Onboarding = (() => {
  const esc = U.esc;
  const S = () => Store.S;
  const hasUI = () => typeof UI !== 'undefined' && !!UI.map;
  const plural = (n, one, many = one + 's') => `${U.fmt.int(n)} ${n === 1 ? one : many}`;
  const andList = a => a.length <= 1 ? (a[0] || '') : a.slice(0, -1).join(', ') + ' and ' + a[a.length - 1];
  const someNames = (names, k = 3) => names.length <= k ? andList(names) : `${names.slice(0, k).join(', ')} and ${names.length - k} more`;
  const sections = () => (typeof Inspector !== 'undefined' ? Inspector.SECTIONS : '');
  // A named undo step (Kit.snap labels it for the undo toast) on the active scenario
  const snap = label => (typeof Kit !== 'undefined' && Kit.snap ? Kit.snap(label) : Store.snapshot());
  const FIELD_LABEL = { cls: 'road class', status: 'status', speed: 'speed limit', lanes: 'lanes', cycle: 'cycling facility', control: 'junction control' };
  // An AbortController, or a stand-in where the runtime has none (the headless harness)
  const controller = () => {
    if (typeof AbortController === 'function') return new AbortController();
    const signal = { aborted: false, fns: [], addEventListener(t, f) { this.fns.push(f); }, removeEventListener(t, f) { this.fns = this.fns.filter(x => x !== f); } };
    return { signal, abort() { if (signal.aborted) return; signal.aborted = true; for (const f of signal.fns.slice()) { try { f(); } catch { } } } };
  };
  const abortErr = () => Object.assign(new Error('The search was replaced by a newer one.'), { name: 'AbortError' });

  /* ---------- numbers people type (zone-gateway-ux d) ----------
     '12,000' → 12000, '5000 m2', '5 000 m²' and '5000sqm' → 5000. Empty → null; anything else → NaN, so the
     caller can say 'Enter a number' instead of saving a silent 0. */
  function parseNum(v) {
    if (typeof v === 'number') return isFinite(v) ? v : NaN;
    const s = String(v ?? '').trim().toLowerCase().replace(/\s*(m2|m²|sq\.?\s*m|sqm)\.?$/, '').replace(/[\s,]/g, '');
    if (!s) return null;
    return /^[-+]?(\d+\.?\d*|\.\d+)$/.test(s) ? +s : NaN;
  }

  /* ---------- standards as data (overview rule 4) ---------- */
  const SEARCH = {
    photon: 'https://photon.komoot.io/api/', bbox: '166.3,-47.4,178.6,-34.3', minChars: 3, debounce: 300, timeout: 8000, limit: 6, cacheMax: 100,
    attribution: 'Search: Photon by komoot, © OpenStreetMap contributors',
    fallback: 'Search © OpenStreetMap contributors (Nominatim)',
    licence: 'OpenStreetMap data, ODbL 1.0',
  };
  // Thresholds the network check uses. None is a published rule: all are Spare Construct assumptions.
  const CHECK_RULES = {
    id: 'corridor-network-check', name: 'Spare Construct network check', edition: 'Spare Construct v2 (2026)', verified: false, basis: 'Spare Construct assumption',
    connectorMaxM: 400,                      // a zone further than this from its access junction loads in the wrong place
    boundaryM: 150, boundaryShare: 0.08,     // a dead end this close to the edge of the network is a missing gateway
    minSpeed: 10,
    // Highest speed limit and lanes in one direction expected for each class; 110 km/h is normally only set on
    // motorways and expressways (Land Transport Rule: Setting of Speed Limits 2024)
    classMax: { motorway: { speed: 110, lanes: 5 }, ramp: { speed: 100, lanes: 3 }, arterial: { speed: 100, lanes: 4 }, collector: { speed: 100, lanes: 3 }, local: { speed: 100, lanes: 2 } },
    legTol: 22.5,                            // a site leg more than half a 45° sector off its code has collided with another
    source: 'Spare Construct assumptions (docs/spec/check-network); speed limits: Land Transport Rule: Setting of Speed Limits 2024',
  };
  const LEVEL = { error: ['F', '!', 'Problem'], warn: ['D', '?', 'Check'], info: ['none', 'i', 'Note'] };
  const ISSUE_COLOUR = '#d6336c';   // magenta, as the spec asks; reads on both themes

  /* ---------- place search: Photon typeahead, Nominatim on Enter ---------- */
  const pcache = new Map();
  const copyRes = r => ({ ...r, bounds: r.bounds.map(p => p.slice()) });
  // A box of side m metres (or w × h) around a point, as [[s, w], [n, e]]
  function box(lat, lng, m = AREA_M, h = m) {
    const dLat = h / 2 / 110574, dLng = m / 2 / (111320 * Math.max(0.05, Math.cos(lat * Math.PI / 180)));
    return [[U.r7(lat - dLat), U.r7(lng - dLng)], [U.r7(lat + dLat), U.r7(lng + dLng)]];
  }
  // 'Waihoehoe Road, Drury, Auckland' from Photon's properties
  function photonLabel(p) {
    const first = p.name || [p.housenumber, p.street].filter(Boolean).join(' ') || p.street || p.city || 'Unnamed place';
    const rest = [];
    for (const k of ['district', 'locality', 'city', 'county']) {
      const v = p[k];
      if (v && v !== first && !rest.includes(v) && rest.length < 2) rest.push(v);
    }
    if (p.name && p.street && p.housenumber) rest.unshift(`${p.housenumber} ${p.street}`);
    return [first, ...rest].slice(0, 3).join(', ');
  }
  function fromPhoton(f) {
    const c = f && f.geometry && f.geometry.coordinates, p = (f && f.properties) || {};
    if (!Array.isArray(c) || !isFinite(c[0]) || !isFinite(c[1])) return null;
    if (p.countrycode && String(p.countrycode).toUpperCase() !== 'NZ') return null;
    const lat = +c[1], lng = +c[0], e = p.extent;     // extent: [minLon, maxLat, maxLon, minLat]
    const bounds = Array.isArray(e) && e.length === 4 && e.every(v => isFinite(v))
      ? [[Math.min(e[1], e[3]), Math.min(e[0], e[2])], [Math.max(e[1], e[3]), Math.max(e[0], e[2])]] : box(lat, lng, 500);
    return { name: photonLabel(p), lat: U.r7(lat), lng: U.r7(lng), bounds, type: p.type || p.osm_value || 'place', src: 'photon' };
  }
  // → [{ name, lat, lng, bounds, type, src: 'photon' }]; [] under 3 characters. Throws AbortError when aborted.
  async function photon(q, { signal, near, timeout = SEARCH.timeout } = {}) {
    const s = String(q ?? '').trim();
    if (s.length < SEARCH.minChars) return [];
    const qx = typeof Search !== 'undefined' ? Search.expand(s) : s, key = qx.toLowerCase().replace(/\s+/g, ' ');
    if (pcache.has(key)) return pcache.get(key).map(copyRes);
    if (signal && signal.aborted) throw abortErr();
    const params = { q: qx, limit: String(SEARCH.limit), bbox: SEARCH.bbox, lang: 'en' };
    if (near && isFinite(near.lat) && isFinite(near.lng)) { params.lat = (+near.lat).toFixed(4); params.lon = (+near.lng).toFixed(4); }
    const ctl = controller(), timer = setTimeout(() => ctl.abort(), timeout), relay = () => ctl.abort();
    if (signal) signal.addEventListener('abort', relay);
    let r, j;
    try {
      r = await fetch(SEARCH.photon + '?' + new URLSearchParams(params), { signal: ctl.signal });
      if (!r.ok) throw Object.assign(new Error(`The place search answered HTTP ${r.status}. Press Enter to search OpenStreetMap instead.`), { http: true });
      j = await r.json();
    } catch (e) {
      if (signal && signal.aborted) throw abortErr();
      if (e.http) throw e;
      throw new Error(ctl.signal.aborted ? `The place search did not answer within ${timeout / 1000} s. Press Enter to try OpenStreetMap instead.`
        : `The place search could not be reached (${e.message}). Check your internet connection and try again.`);
    } finally { clearTimeout(timer); if (signal) signal.removeEventListener('abort', relay); }
    const seen = new Set(), list = [];
    for (const f of (j && j.features) || []) {
      const x = fromPhoton(f); if (!x || seen.has(x.name)) continue;
      seen.add(x.name); list.push(x);
    }
    pcache.set(key, list);
    if (pcache.size > SEARCH.cacheMax) pcache.delete(pcache.keys().next().value);
    return list.map(copyRes);
  }
  /* A search box's brain: input(q) on every keystroke (debounced Photon, never Nominatim); enter(q) on Enter →
     the Photon answer for that text if it has one, otherwise one Nominatim search (Search.geocode). */
  function typeahead({ onResults = () => {}, onError = () => {}, near = () => null, delay = SEARCH.debounce } = {}) {
    let timer = null, ctl = null, seq = 0, last = { q: '', items: [] };
    const stop = () => { clearTimeout(timer); timer = null; if (ctl) { ctl.abort(); ctl = null; } };
    const coords = s => typeof Search !== 'undefined' && !!Search.parseCoords(s);
    return {
      input(q) {
        stop();
        const s = String(q || '').trim(), my = ++seq;
        if (s.length < SEARCH.minChars || coords(s)) { last = { q: s, items: [] }; onResults([], s); return; }
        timer = setTimeout(async () => {
          timer = null;
          const c = ctl = controller();
          try {
            const items = await photon(s, { signal: c.signal, near: near() });
            if (my === seq) { last = { q: s, items }; onResults(items, s); }
          } catch (e) { if (e.name !== 'AbortError' && my === seq) onError(e, s); }
          finally { if (ctl === c) ctl = null; }
        }, delay);
      },
      async enter(q) {
        stop(); seq++;
        const s = String(q || '').trim();
        if (!s) return [];
        if (coords(s)) return Search.geocode(s);
        let items = last.q === s ? last.items : [];
        if (!items.length) { try { items = await photon(s, { near: near() }); } catch { items = []; } }
        if (items.length || typeof Search === 'undefined') return items;
        return (await Search.geocode(s)).map(r => ({ ...r, src: 'nominatim' }));
      },
      cancel: stop,
      get last() { return last; },
    };
  }
  const mapCentre = () => (hasUI() ? UI.map.getCenter() : null);

  /* ---------- study area ---------- */
  const AREA_M = 1500;
  const toArr = b => Array.isArray(b) ? [[+b[0][0], +b[0][1]], [+b[1][0], +b[1][1]]] : [[b.getSouth(), b.getWest()], [b.getNorth(), b.getEast()]];
  const normBox = b => { const [[a, c], [d, e]] = toArr(b); return [[Math.min(a, d), Math.min(c, e)], [Math.max(a, d), Math.max(c, e)]]; };
  function areaSize(b) {
    const [[s, w], [n, e]] = normBox(b), lat = (s + n) / 2;
    return { w: U.dist([lat, w], [lat, e]), h: U.dist([s, (w + e) / 2], [n, (w + e) / 2]) };
  }
  /* How many OpenStreetMap roads a box holds, before anything is downloaded: an Overpass count query (a few hundred
     bytes back), with the same road types the set-up downloads. Cached per box; null when no server answers. */
  const rcache = new Map();
  const roadKey = (b, local = true, service = false) => normBox(b).flat().map(v => (+v).toFixed(4)).join(',') + (local ? 'L' : '') + (service ? 'S' : '');
  async function roadCount(b, { signal, local = true, service = false, timeout = 10000 } = {}) {
    if (typeof OSM_CLASS === 'undefined' || typeof OVERPASS === 'undefined') return null;
    const key = roadKey(b, local, service);
    if (rcache.has(key)) return rcache.get(key);
    const types = Object.keys(OSM_CLASS).filter(t => service || t !== 'service').filter(t => local || OSM_CLASS[t] !== 'local');
    const [[s, w], [n, e]] = normBox(b), q = `[out:json][timeout:15];way["highway"~"^(${types.join('|')})$"](${[s, w, n, e].map(v => v.toFixed(6)).join(',')});out count;`;
    for (const url of OVERPASS) {
      if (signal && signal.aborted) throw abortErr();
      const ctl = controller(), timer = setTimeout(() => ctl.abort(), timeout), relay = () => ctl.abort();
      if (signal) signal.addEventListener('abort', relay);
      try {
        const r = await fetch(url, { method: 'POST', body: 'data=' + encodeURIComponent(q), headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, signal: ctl.signal });
        if (!r.ok) continue;
        const j = await r.json(), c = j && Array.isArray(j.elements) && j.elements.find(x => x && x.type === 'count'), v = c && c.tags ? Math.round(+c.tags.ways) : NaN;
        if (isFinite(v) && v >= 0) { rcache.set(key, v); if (rcache.size > 50) rcache.delete(rcache.keys().next().value); return v; }
      } catch (err) { if (signal && signal.aborted) throw abortErr(); }
      finally { clearTimeout(timer); if (signal) signal.removeEventListener('abort', relay); }
    }
    return null;
  }
  /* '2.1 × 1.6 km · 3.4 km² · about 450 roads in OpenStreetMap · about 1 minute to set up': the StudyArea size warning,
     the OpenStreetMap road count once roadCount has it, and road segments already in this project inside the box */
  function readout(b, sc = Store.sc) {
    const bb = normBox(b), { w, h } = areaSize(bb);
    const est = typeof StudyArea !== 'undefined' ? StudyArea.estimate(L.latLngBounds(bb)) : { km2: Math.round(w * h / 1e5) / 10, warn: null, ok: w * h > 0, minutes: 1 };
    const inB = (lat, lng) => lat >= bb[0][0] && lat <= bb[1][0] && lng >= bb[0][1] && lng <= bb[1][1];
    let segments = 0;
    for (const Lk of Object.values(sc ? sc.links : {})) { const A = sc.nodes[Lk.a], B = sc.nodes[Lk.b]; if (A && B && (inB(A.lat, A.lng) || inB(B.lat, B.lng))) segments++; }
    const osm = rcache.has(roadKey(bb)) ? rcache.get(roadKey(bb)) : null;
    const size = `${U.fmt.num(w / 1000, 1)} × ${U.fmt.num(h / 1000, 1)} km · ${U.fmt.num(est.km2, est.km2 < 1 ? 2 : 1)} km²`;
    const text = `${size}${osm != null ? ` · about ${plural(osm, 'road')} in OpenStreetMap` : ''} · about ${plural(est.minutes, 'minute')} to set up${segments ? ` · ${plural(segments, 'road segment')} already in this project` : ''}`;
    return { w, h, km2: est.km2, minutes: est.minutes, warn: est.warn, ok: est.ok, segments, osm, size, text };
  }
  // The card asks for the road count once the box stops changing (never while a corner is being dragged)
  let countTimer = null, countCtl = null;
  function countRoadsSoon(b) {
    clearTimeout(countTimer); if (countCtl) { countCtl.abort(); countCtl = null; }
    if (!b || !hasUI() || rcache.has(roadKey(b))) return;
    countTimer = setTimeout(async () => {
      const c = countCtl = controller();
      try {
        const v = await roadCount(b, { signal: c.signal });
        const ro = card && card.querySelector('.onb-readout'), now = areaOf();
        if (c === countCtl && v != null && ro && now && roadKey(now) === roadKey(b)) ro.textContent = readout(now).text;
      } catch { /* replaced by a newer box */ }
      finally { if (countCtl === c) countCtl = null; }
    }, 700);
  }
  const areaOf = (P = Store.P) => (P && P.onboarding && Array.isArray(P.onboarding.area) ? P.onboarding.area : null);
  function setArea(b, save = true) {
    const P = Store.P; if (!P) return;
    if (!b) { if (P.onboarding) delete P.onboarding.area; }
    else { P.onboarding ||= {}; P.onboarding.area = normBox(b).map(p => p.map(U.r7)); }
    if (save) { Store.saveSoon(); countRoadsSoon(b); }
    drawArea();
  }

  /* ---------- 'Set up this area' ---------- */
  const STEPS = [
    { id: 'roads', label: 'Download roads (OpenStreetMap)' },
    { id: 'gateways', label: 'Add gateways where main roads cross the boundary' },
    { id: 'data', label: 'Load NZTA and Auckland Transport counts and NZTA CAS crashes' },
    { id: 'landuse', label: 'Fill zones from existing buildings and land use (OpenStreetMap)' },
    { id: 'gwflows', label: 'Set gateway flows from counts' },
    { id: 'check', label: 'Check the network' },
    { id: 'run', label: 'Run the base model, AM and PM peaks' },
  ];
  const STUDY_STEPS = ['roads', 'gateways', 'data', 'landuse', 'gwflows'];
  // Years as whole numbers (a project file can hold anything), or null
  const yearNum = v => { const n = Math.round(+v); return v !== '' && v != null && n >= 1900 && n <= 2200 ? n : null; };
  const yearsOf = (sc = Store.sc) => ({ yr: yearNum(Engine.yearOf(sc)), by: yearNum(S().baseYear) });
  function setupDefaults(P = Store.P) {
    const multi = P.scenarios.length > 1, { yr, by } = yearsOf();
    return { local: true, service: false, gateways: true, data: true, existing: typeof ExistingLU !== 'undefined', census: typeof ExistingLU !== 'undefined' && hasCensus(),
      run: true, fit: false, all: multi, baseYear: !multi && by != null && yr !== by };
  }
  function setupHtml(b, o = setupDefaults()) {
    const r = readout(b), multi = Store.P.scenarios.length > 1, { yr, by } = yearsOf();
    const ck = (k, text, note = '') => `<label class="check"><input type="checkbox" data-onb-opt="${k}" ${o[k] ? 'checked' : ''}> ${text}</label>${note ? `<p class="note" style="margin:0 0 0 26px">${note}</p>` : ''}`;
    return `<p>Spare Construct builds a base model for this ${esc(r.size)} box${r.osm != null ? ` (about ${esc(plural(r.osm, 'road'))} in OpenStreetMap)` : ''} in <b>${esc(Store.sc.name)}</b>, in the steps below. A step that fails is reported and the rest still run.</p>
      ${r.warn ? Kit.note(esc(r.warn), true) : ''}
      <div style="display:grid;gap:6px;margin:10px 0">
        ${ck('local', 'Include local and residential streets')}
        ${ck('service', 'Include service lanes and alleys')}
        ${ck('gateways', 'Add gateways where main roads leave the area')}
        ${ck('data', 'Load traffic counts (NZTA, Auckland Transport) and crashes (NZTA CAS)')}
        ${typeof ExistingLU !== 'undefined' ? ck('existing', 'Add zones for the existing buildings and land use (OpenStreetMap)', 'Indicative: dwellings and floor areas are estimated from building footprints.') : Kit.note('Existing land use is not available in this version, so only gateways will carry traffic.', true)}
        ${typeof ExistingLU !== 'undefined' && hasCensus() ? ck('census', 'Then use the 2023 Census for their dwellings, and add employee counts and schools', 'Stats NZ Census 2023 (SA1) and business demography employee counts, and the Ministry of Education directory, placed in the zones by building footprint.') : ''}
        ${ck('run', 'Run the base model for the AM and PM peaks')}
        ${typeof Calibrate !== 'undefined' ? ck('fit', 'Then fit the matrix to the counts (matrix estimation, about 1–2 s a peak)', 'Changes the trip matrix to match the counts. Agree its use with the peer reviewer; the counts are then not independent for validation.') : ''}
        ${by != null && yr !== by ? ck('baseYear', `Model the base year (${by}) in this scenario, so it can be compared with counts`, `It is set to ${yr ?? 'no valid year'} now. Make forecast options from it afterwards with + Option.`) : ''}
        ${multi ? ck('all', 'Add the new roads, gateways and zones to every scenario (keeps them comparable)') : ''}
      </div>
      <p class="note">Roads and buildings © OpenStreetMap contributors (ODbL). Counts: NZTA (CC BY 4.0) and Auckland Transport open data. Crashes: NZTA CAS (CC BY 4.0).${hasLimits() ? ' Legal speed limits: NZTA National Speed Limit Register (CC BY 4.0).' : ''}${typeof ExistingLU !== 'undefined' && hasCensus() ? ' Census and employee counts: Stats NZ (CC BY 4.0); schools: Ministry of Education.' : ''} Results are indicative until you have checked them.</p>`;
  }
  const STEP_ICON = { todo: ['none', '·', 'Not started'], run: ['C', '…', 'Running'], done: ['A', '✓', 'Done'], warn: ['D', '!', 'Done with a warning'], skip: ['none', '–', 'Skipped'], fail: ['F', '✕', 'Failed'] };
  function stepsHtml(steps) {
    return `<ol class="onb-steps" id="onbSteps" aria-label="Set-up steps">${steps.map((s, i) => {
      const [cls, sym, lab] = STEP_ICON[s.state] || STEP_ICON.todo;
      return `<li class="onb-step ${s.state}"><span class="los ${cls}" role="img" aria-label="${lab}">${sym}</span><span><b>${i + 1}. ${esc(s.label)}</b>${s.text ? `<br><span class="note">${esc(s.text)}</span>` : ''}${(s.warn || []).map(w => `<br><span class="note warn">${esc(w)}</span>`).join('')}</span></li>`;
    }).join('')}</ol>`;
  }
  /* Which set-up step a StudyArea status message belongs to: by its progress fraction (StudyArea.setup reports
     roads 0.02–0.2, gateways 0.25, counts and crashes 0.3–0.35, buildings 0.5–0.65, gateway flows 0.7), else by its text */
  function stepOf(t, f) {
    if (typeof f === 'number' && isFinite(f)) return f < 0.25 ? 'roads' : f < 0.3 ? 'gateways' : f < 0.5 ? 'data' : f < 0.7 ? 'landuse' : 'gwflows';
    t = String(t || '');
    if (/gateway flows/i.test(t)) return 'gwflows';
    if (/gateway/i.test(t)) return 'gateways';
    if (/buildings|land use|land-use|grouping/i.test(t)) return 'landuse';
    if (/building the network|roads|overpass/i.test(t)) return 'roads';
    if (/count|crash|\bcas\b|nzta|auckland transport/i.test(t)) return 'data';
    return 'roads';
  }
  // Which step a StudyArea warning belongs to
  const warnStep = w => /^Adding gateways/i.test(w) ? 'gateways' : /^Data download|counts and crashes/i.test(w) ? 'data'
    : /land use|buildings|existing zones/i.test(w) ? 'landuse' : /gateway flows/i.test(w) ? 'gwflows' : 'roads';

  /* Legal speed limits from the NZTA National Speed Limit Register (feeds lane) for the new roads. They are stored
     beside each road for review (Traffic data › Data sources › Review modelled speeds); modelled speeds do not change.
     The register is matched to every road in a scenario, so it is only loaded for a project that had no roads
     before, or whose roads all lie around the study area: roads elsewhere would otherwise lose their earlier limits.
     → { text, share, differ } or null */
  const hasLimits = () => typeof Feeds !== 'undefined' && !!(Feeds.nslr && Feeds.nslr.all);
  async function loadLimits(sc, bounds, all, fresh, say, warn) {
    if (!hasLimits() || !Object.keys(sc.links).length) return null;
    const [[s, w], [n, e]] = normBox(bounds), ps = (n - s) * 0.5, pw = (e - w) * 0.5;
    const near = x => x.lat >= s - ps && x.lat <= n + ps && x.lng >= w - pw && x.lng <= e + pw;
    const scs = all ? Store.P.scenarios : [sc];
    if (!fresh && !scs.every(x => Object.values(x.nodes).every(near))) return { text: 'legal speed limits not loaded, as there are roads outside the study area: load them for the whole network in Traffic data › Data sources', share: null, differ: 0 };
    say('Downloading legal speed limits (NZTA register)…');
    try {
      const area = fresh ? L.latLngBounds(Object.values(sc.nodes).map(x => [x.lat, x.lng])).pad(0.02) : L.latLngBounds([[s - ps, w - pw], [n + ps, e + pw]]);
      const r = await Feeds.nslr.all(area, scs, say);
      const me = r.scenarios.find(x => x.id === sc.id) || r.scenarios[0], differ = Feeds.nslr.proposals ? Feeds.nslr.proposals(sc).length : 0;
      Store.emit('data');
      return { text: `legal speed limits for ${U.fmt.pct(me.share)} of roads${differ ? ` (${plural(differ, 'road')} modelled at another speed: review them in Traffic data › Data sources)` : ''}`, share: me.share, differ };
    } catch (err) { warn(`Legal speed limits (NZTA register) could not be downloaded: ${err.message} Load them later in Traffic data › Data sources.`); return null; }
  }

  /* Census 2023 dwellings, 2025 employee counts and the schools directory (the land-use lane's LandusePlus.fillAll),
     to refine the zones built from OpenStreetMap buildings (spec step 5) */
  const hasCensus = () => typeof LandusePlus !== 'undefined' && typeof LandusePlus.fillAll === 'function';
  // elements: the OpenStreetMap buildings the set-up already downloaded (null: fillAll downloads them)
  async function fillCensus(sc, bounds, all, say, warn, elements = null) {
    const others = all ? Store.P.scenarios.filter(s => s !== sc) : [];
    const was = new Map(Object.entries(sc.zones).map(([id, z]) => [id, JSON.stringify(z)]));
    const r = await LandusePlus.fillAll(sc, { bounds: normBox(bounds), which: { osm: true, census: true, employees: true, schools: true, aup: false }, newer: 'flag', all: false,
      ...(Array.isArray(elements) && elements.length ? { elements } : {}) }, say);
    // the other scenarios get the same zones (same ids), as the set-up gave them the same OpenStreetMap zones
    const changed = Object.keys(sc.zones).filter(id => was.get(id) !== JSON.stringify(sc.zones[id]));
    for (const s of others) { s.zones ||= {}; for (const id of changed) s.zones[id] = structuredClone(sc.zones[id]); stale(s); }
    for (const w of (r && r.warnings) || []) warn(w);
    quiet(() => Store.edited('demand', sc));
    const c = r && r.census, e = r && r.employees, k = r && r.schools;
    // census.dwellings === false: the dwellings layer failed, so the dwellings stay the OpenStreetMap estimate
    const nSA1 = c ? plural((c.rows || []).length, 'SA1') : '';
    const bits = [!c ? '' : c.dwellings === false ? `people, households and travel from the 2023 Census (${nSA1}; its dwellings layer did not download, so dwellings stay the OpenStreetMap estimate)`
      : `${plural(Math.round(c.placed), 'dwelling')} from the 2023 Census (${nSA1})`, e ? `${U.fmt.int(e.allocated)} employees (${e.year})` : '', k ? plural(k.schools.length, 'school') : ''].filter(Boolean);
    return { text: bits.length ? bits.join(', ') : '', zones: changed.length, census: !!c, dwellings: !!c && c.dwellings !== false, employees: !!e };
  }

  /* runSetup(bounds, opts, onStep) → summary. opts: local, service, gateways, data, existing, census, run, fit, all,
     baseYear (see setupDefaults), signal (an AbortSignal: stops before the check and the runs). onStep(steps) is
     called whenever a step changes. Pure logic: no DOM, so it runs headlessly. Its edits are its own: Edits apply
     to does not copy them (the 'all' option does). */
  async function runSetup(bounds, opts = {}, onStep = () => {}) {
    setupDepth++;
    try { return await setupSteps(bounds, opts, onStep); } finally { setupDepth--; }
  }
  async function setupSteps(bounds, opts, onStep) {
    if (typeof StudyArea === 'undefined') throw new Error('Study area set-up is not available in this version of Spare Construct.');
    const o = { ...setupDefaults(), ...opts }, sc = Store.sc, signal = o.signal;
    o.existing = !!o.existing && typeof ExistingLU !== 'undefined';
    o.census = !!o.census && o.existing && hasCensus();
    const limits = o.data && hasLimits();
    // the step labels say what this set-up actually uses
    const label = { data: `Load NZTA and Auckland Transport counts${limits ? ', NZTA legal speed limits' : ''} and NZTA CAS crashes`,
      landuse: `Fill zones from existing buildings (OpenStreetMap)${o.census ? ', the 2023 Census and employee counts (Stats NZ)' : ''}` };
    const steps = STEPS.map(s => ({ ...s, label: label[s.id] || s.label, state: 'todo', text: '', warn: [] })), st = id => steps.find(s => s.id === id);
    const tell = () => { try { onStep(steps); } catch (e) { console.error(e); } };
    const set = (id, state, text) => { const s = st(id); if (state) s.state = state; if (text != null) s.text = text; tell(); };
    const finish = (id, text) => { const s = st(id); if (s.state === 'skip' || s.state === 'fail') return; s.state = s.warn.length ? 'warn' : 'done'; if (text != null) s.text = text; };
    if (!o.gateways) set('gateways', 'skip', 'Not asked for');
    if (!o.data) set('data', 'skip', 'Not asked for');
    if (!o.existing) set('landuse', 'skip', typeof ExistingLU === 'undefined' ? 'Not available in this version' : 'Not asked for');
    if (!o.run) set('run', 'skip', 'Not asked for');
    tell();
    const yy = yearsOf(sc);
    if (o.baseYear && yy.by != null && yy.yr !== yy.by) { snap(`base year ${yy.by}`); sc.year = yy.by; Store.edited('settings'); }
    let cur = null;
    const onStatus = (t, f) => {
      if (f >= 0.999 || /study area ready/i.test(String(t))) return;
      let id = stepOf(t, f);
      if (cur && STUDY_STEPS.indexOf(id) < STUDY_STEPS.indexOf(cur)) id = cur;   // never step backwards
      if (id !== cur) {
        const from = cur ? STUDY_STEPS.indexOf(cur) : -1;
        for (const k of STUDY_STEPS.slice(from, STUDY_STEPS.indexOf(id))) if (st(k).state === 'run') st(k).state = 'done';
        cur = id;
      }
      if (st(id).state === 'skip') return;   // a step not asked for stays skipped
      set(id, 'run', String(t || ''));
    };
    const fresh = (o.all ? Store.P.scenarios : [sc]).every(x => !Object.keys(x.links).length);   // no roads before this set-up
    /* The OpenStreetMap buildings StudyArea downloads for the existing zones go on to the Census step
       (LandusePlus.fillAll opts.elements), so they are not downloaded twice. StudyArea.setup does not return them yet
       (out.elements is used when it does), so its ExistingLU.fetch call is watched for the length of the set-up only. */
    let osmEls = null;
    const ELU = o.census && typeof ExistingLU !== 'undefined' && ExistingLU && typeof ExistingLU.fetch === 'function' ? ExistingLU : null;
    const fetch0 = ELU ? ELU.fetch : null, watch = ELU ? async function (...a) { const els = await fetch0.apply(this, a); if (Array.isArray(els)) osmEls = els; return els; } : null;
    if (ELU) ELU.fetch = watch;
    let out;
    try {
      out = await StudyArea.setup(L.latLngBounds(normBox(bounds)), { roads: true, local: o.local, service: o.service, gateways: o.gateways, counts: o.data, crashes: o.data,
        existing: o.existing, all: o.all, run: false, fitCounts: false }, onStatus);
    } catch (e) {
      set(cur || 'roads', 'fail', e.message);
      throw e;
    } finally {
      if (ELU && ELU.fetch === watch) ELU.fetch = fetch0;
    }
    const elements = Array.isArray(out.elements) ? out.elements : osmEls;
    for (const w of out.warnings || []) if (!/results are out of date/i.test(w)) st(warnStep(w)).warn.push(w);
    const stopped = () => !!(signal && signal.aborted);
    const gws = Object.values(sc.zones).filter(z => z.kind === 'gateway'), fromCounts = gws.filter(z => z.gwSrc).length;
    // the steps StudyArea finished tick now; the two that load more (speed limits, the Census) tick when that is done
    finish('roads', `${plural(out.roads, 'road segment')} added${out.scenarios > 1 ? ` to ${out.scenarios} scenarios` : ''}`);
    finish('gateways', `${plural(out.gateways, 'gateway')} added (${gws.length} in all)`);
    if (!gws.length) finish('gwflows', 'There are no gateways');
    else {
      if (!o.data || !out.counts) st('gwflows').warn.push(`No counts were loaded, so all ${plural(gws.length, 'gateway')} use estimated flows.`);
      finish('gwflows', `${fromCounts} of ${plural(gws.length, 'gateway')} from counts, ${gws.length - fromCounts} estimated`);
    }
    const dataText = `${plural(out.counts, 'count site')} and ${plural(out.crashes, 'crash', 'crashes')}`;
    const luText = `${plural(out.zones, 'zone')} with ${plural(out.dwellings, 'dwelling')} from OpenStreetMap buildings${out.replaced ? `, replacing ${out.replaced} from an earlier set-up` : ''}`;
    if (!limits) finish('data', dataText);
    if (!o.census) finish('landuse', luText);
    tell();
    // 3 · legal speed limits (NZTA register), stored beside the roads for review
    if (limits) {
      const nslr = !stopped() ? await loadLimits(sc, bounds, o.all, fresh, t => set('data', 'run', t), w => st('data').warn.push(w)) : null;
      finish('data', `${dataText}${nslr ? `; ${nslr.text}` : ''}`);
      tell();
    }
    // 4 · Census 2023 dwellings, employee counts and schools for the zones
    if (o.census) {
      let cen = null;
      if (stopped()) st('landuse').warn.push('Cancelled before the Census and employee counts were added. Add them in the Land use tab.');
      else {
        set('landuse', 'run', 'Adding the 2023 Census, employee counts and schools…');
        try { cen = await fillCensus(sc, bounds, o.all, t => set('landuse', 'run', String(t || '')), w => st('landuse').warn.push(w), elements); }
        catch (e) { st('landuse').warn.push(`The Census and employee counts could not be added: ${e.message} Add them later in the Land use tab.`); }
      }
      finish('landuse', `${luText}${cen && cen.text ? `; then ${cen.text}` : ''}`);
      tell();
    }
    // 6 · check the network
    let chk = null;
    if (stopped()) set('check', 'skip', 'Cancelled');
    else {
      set('check', 'run', 'Checking…');
      chk = check(sc);
      setIssues(chk);
      set('check', chk.counts.error ? 'warn' : 'done', chk.issues.length ? `${checkSummary(chk)}. The Network check tab lists them.` : 'No problems found');
    }
    // 7 · base runs, then the fit to counts if asked for
    let ran = false;
    const calibration = {};
    if (o.run && stopped()) set('run', 'skip', 'Cancelled');
    else if (o.run) {
      const canRun = Object.keys(sc.links).length && Object.keys(sc.zones).length;
      if (!canRun) { st('run').warn.push(Object.keys(sc.links).length ? 'There are no zones or gateways, so there is no traffic to load.' : 'There are no roads, so the model cannot run.'); set('run', 'warn', 'Not run'); }
      else {
        sc.res ||= {};
        for (const period of ['AM', 'PM']) {
          set('run', 'run', `${period} peak…`);
          try {
            sc.res[period] = await Engine.assign(sc, { period, onProgress: (it, gap, msg) => set('run', null, `${period} peak · ${msg || 'iteration ' + it}`) });
            ran = true;
            Registry.fire('results', sc, period, sc.res[period]);
          } catch (e) { st('run').warn.push(`The ${period} peak could not run: ${e.message}`); }
        }
        if (o.fit && ran) {
          if (typeof Calibrate === 'undefined') st('run').warn.push('Fitting to counts is not available in this version.');
          else if (!DataHub.D().counts.length) st('run').warn.push('No counts were loaded, so the model was not fitted to counts.');
          else for (const period of ['AM', 'PM']) {
            set('run', 'run', `Fitting the ${period} peak to counts…`);
            try {
              calibration[period] = await Calibrate.fit(sc, { period });
              if (!sc.res[period] || sc.res[period].stale) sc.res[period] = await Engine.assign(sc, { period });
            } catch (e) { st('run').warn.push(`Fitting the ${period} peak failed: ${e.message}`); }
          }
        }
        if (ran) Store.emit('results');
        // TMDG: convergence is shown before any comparison with counts is used
        for (const p of ['AM', 'PM']) { const r = sc.res[p]; if (r && !r.stale && r.converged === false) st('run').warn.push(`${p} peak: ${r.convNote || 'Not converged.'}`); }
        const k = p => sc.res[p] && sc.res[p].kpi;
        finish('run', ['AM', 'PM'].filter(k).map(p => `${p} ${U.fmt.int(k(p).trips)} veh/h at ${U.fmt.num(k(p).spd, 0)} km/h`).join(' · ') || 'Not run');
      }
    }
    tell();
    const fit = {}, notConverged = [];
    for (const p of ['AM', 'PM']) {
      const r = Store.res(sc, p); if (!r || r.stale) continue;
      if (r.converged === false) notConverged.push({ period: p, note: r.convNote || 'Not converged.' });
      const c = DataHub.calibration(sc, p);
      if (c && c.n) fit[p] = { n: c.n, pass: c.pass, totObs: c.totObs, totMod: c.totMod, r2: c.r2OriginCentred ?? c.r2, slope: c.b0 ?? c.slope };
    }
    const yrs = yearsOf(sc);
    return { out, steps, check: chk, ran, fit, notConverged, calibration: Object.keys(calibration).length ? calibration : null, year: yrs.yr, baseYear: yrs.by,
      scenario: sc.name, gateways: gws.length, gwFromCounts: fromCounts, cancelled: stopped() };
  }

  /* The GEH < 5 target for the project's calibration standard and TMDG purpose category (settings.calib.standard,
     settings.tmdg.category), from the calibration lane's criteria library when it is loaded → { min, label } */
  function gehTarget() {
    const def = { min: (typeof Calibrate !== 'undefined' && Calibrate.RULES && Calibrate.RULES.geh5 && Calibrate.RULES.geh5.min) || 0.85, label: 'TMDG 2019 Category E' };
    if (typeof Calibration === 'undefined' || typeof Calibration.tests !== 'function') return def;
    try {
      const std = String((S().calib || {}).standard || 'tmdg'), t = Calibration.tests().find(x => x.id === 'geh5');
      if (std === 'tmdg') {
        const c = String((S().tmdg || {}).category || 'E').toUpperCase(), cat = (Calibration.CATS || []).includes(c) ? c : 'E';
        return { min: t && t.kind === 'share' ? t.min : null, label: `TMDG 2019 Category ${cat}` };
      }
      const names = { rms: 'RMS 2013', atap: 'ATAP' };
      return { min: t && t.kind === 'share' ? t.min : null, label: names[std] || std };
    } catch { return def; }
  }

  const D6 = 'Calibration and validation statistics are reported against the NZTA Transport Model Development Guidelines (2019) for the stated model purpose category. Those criteria are not a pass/fail checklist, and meeting them does not by itself make a model fit for purpose. Fitness for purpose is a judgment for the analyst and peer reviewer and, where NZTA is involved, NZTA\'s modelling specialists.';
  // The closing summary: how the base model compares with counts, and what to do next
  function summaryHtml(sum) {
    const pct = x => Math.round(x * 100), minSites = typeof Calibrate !== 'undefined' && Calibrate.MIN_SITES || 10;
    const lines = [], tg = gehTarget(), nc = sum.notConverged || [];
    const periods = Object.keys(sum.fit);
    for (const p of periods) { const f = sum.fit[p]; lines.push(`${pct(f.pass)}% of the ${plural(f.n, 'matched count')} have GEH < 5 in the ${p} peak (${tg.min != null ? `target ${pct(tg.min)}%, ` : 'no GEH < 5 target for '}${tg.label}${nc.length ? '; indicative only, as the run did not converge' : ''}).`); }
    let next;
    if (sum.cancelled && !sum.ran) next = 'Set-up was cancelled before the model ran. Press Set up this area again, or run the model (Ctrl+Enter).';
    else if (!sum.ran) next = 'Next: fix the steps marked ! above, then run the model (Ctrl+Enter).';
    else if (nc.length) next = 'Next: raise Maximum iterations in Settings › Assignment and junctions, then run the model again (Ctrl+Enter). Agree any change to the convergence target with your peer reviewer.';
    else if (!periods.length) next = 'No counts matched a modelled road, so the model cannot be compared with real traffic yet. Next: load counts in the Traffic data tab, or import your own.';
    else {
      const worst = periods.map(p => sum.fit[p]).sort((a, b) => a.pass - b.pass)[0];
      if (worst.n < minSites) next = `Fewer than ${minSites} counts matched, so this is indicative only: there are too few sites to judge calibration. Next: add counts for the main roads.`;
      else if (tg.min != null && worst.pass >= tg.min) next = 'Next: check the sites with the highest GEH in the Traffic data tab, and confirm the base inputs with your peer reviewer.';
      else if (worst.totMod < worst.totObs * 0.95) next = 'Next: check zones where the model is below the counts (missing land use, or gateway flows that are too low).';
      else if (worst.totMod > worst.totObs * 1.05) next = 'Next: check gateway flows and road coding where the model is above the counts.';
      else next = 'Next: check the sites with the highest GEH in the Traffic data tab; totals match, so look at how traffic is split between roads.';
    }
    const yr = yearNum(sum.year), by = yearNum(sum.baseYear);
    const yearNote = yr !== by ? Kit.note(esc(`This scenario models ${yr ?? 'no valid year'}, not the base year ${by ?? '(not set)'}, so the comparison with counts is indicative only.`), true) : '';
    const convNote = nc.length ? Kit.note(esc(nc.map(x => `${x.period} peak: ${x.note}`).join(' ')), true) : '';
    const head = !sum.ran ? 'The study area is set up, but the model has not run.' : nc.length ? 'The base model ran, but the assignment did not converge.' : 'Your base model is ready.';
    const facts = ['AM', 'PM'].map(p => { const r = Store.res(Store.sc, p); return r && r.kpi && !r.stale ? Kit.fact(U.fmt.int(r.kpi.trips) + ' veh/h', `${p} trips · ${U.fmt.num(r.kpi.spd, 0)} km/h average`) : ''; }).join('');
    const chk = sum.check && sum.check.issues.length ? `<p class="note">${esc(checkSummary(sum.check))} in the network. <a href="#" data-onb="show-check">Show them</a>.</p>` : '';
    return `<p><b>${head}</b> ${lines.map(esc).join(' ')}</p>${convNote}
      <p>${esc(next)}</p>${yearNote}
      ${facts ? `<div class="facts">${facts}</div>` : ''}
      ${chk}
      ${stepsHtml(sum.steps)}
      <p class="note">${esc(D6)}</p>`;
  }

  /* ---------- network check ---------- */
  const rankOf = L => (CLASSES[L.cls] || CLASSES.local).rank;
  const clsLabel = L => (CLASSES[L.cls] || CLASSES.local).label.toLowerCase();
  function check(sc = Store.sc, R = CHECK_RULES) {
    const t0 = performance.now(), issues = [], nodes = sc.nodes;
    const add = (code, level, msg, ids, at) => issues.push({ code, level, msg, ids: [...new Set(ids)], at: at || null });
    const links = Object.values(sc.links).filter(L => nodes[L.a] && nodes[L.b] && L.a !== L.b), open = links.filter(L => L.status !== 'closed');
    const inc = new Map();
    for (const L of open) for (const n of [L.a, L.b]) { const a = inc.get(n); if (a) a.push(L); else inc.set(n, [L]); }
    const deg = id => (inc.get(id) || []).length;
    const zones = Object.values(sc.zones || {}).filter(z => isFinite(z.lat) && isFinite(z.lng)), gws = zones.filter(z => z.kind === 'gateway');
    const pinned = new Set(zones.map(z => z.node).filter(id => id && nodes[id]));
    const llOf = id => [nodes[id].lat, nodes[id].lng];
    const lens = new Map(open.map(L => [L.id, Net.len(sc, L)]));
    const midOf = L => { const g = Net.geom(sc, L); return g[Math.floor((g.length - 1) / 2)]; };
    const named = open.filter(L => L.name).map(L => ({ L, p: midOf(L) }));
    function nearName(ll, skip) {
      let best = null, bd = Infinity;
      for (const x of named) { if (skip && skip.has(x.L.id)) continue; const d = U.dist(ll, x.p); if (d < bd) { bd = d; best = x.L.name; } }
      return best;
    }
    const roadName = L => L.name || `an unnamed ${clsLabel(L)} road`;
    const where = id => {
      const names = [...new Set((inc.get(id) || []).map(L => L.name).filter(Boolean))];
      if (deg(id) >= 3) return names.length > 1 ? `the junction of ${andList(names.slice(0, 3))}` : names.length ? `a junction on ${names[0]}` : 'an unnamed junction';
      if (deg(id) === 2) return names.length > 1 ? `the corner of ${andList(names)}` : names.length ? `a point on ${names[0]}` : 'a point on an unnamed road';
      return names.length ? `the end of ${names[0]}` : 'a road end';
    };
    const s = (n, one, many) => n === 1 ? one : many;

    // 1 · pieces not connected to the main network (weakly connected, closed roads left out)
    const ids = Object.keys(nodes), ix = new Map(ids.map((id, i) => [id, i])), par = ids.map((_, i) => i);
    const find = i => { while (par[i] !== i) { par[i] = par[par[i]]; i = par[i]; } return i; };
    for (const L of open) { const a = find(ix.get(L.a)), b = find(ix.get(L.b)); if (a !== b) par[a] = b; }
    const comps = new Map();
    for (const L of open) { const r = find(ix.get(L.a)); let c = comps.get(r); if (!c) comps.set(r, c = { links: [], len: 0, nodes: new Set() }); c.links.push(L); c.len += lens.get(L.id); c.nodes.add(L.a); c.nodes.add(L.b); }
    let main = null;
    for (const c of comps.values()) if (!main || c.len > main.len) main = c;
    for (const c of comps.values()) {
      if (c === main) continue;
      const own = new Set(c.links.map(L => L.id)), n = c.links.length, at = midOf(c.links[0]);
      const names = [...new Set(c.links.map(L => L.name).filter(Boolean))], nr = nearName(at, own);
      const place = nr ? `near ${nr}` : names.length ? `on ${andList(names.slice(0, 2))}` : 'away from the main network';
      const gw = gws.filter(z => c.nodes.has(z.node));
      add('disconnected', gw.length ? 'error' : 'warn', `${plural(n, 'road')} (${U.fmt.len(c.len)}) ${place}${names.length && nr && !names.includes(nr) ? ` (${andList(names.slice(0, 2))})` : ''} ${s(n, 'is', 'are')} not connected to the main network, so no traffic can use ${s(n, 'it', 'them')}${gw.length ? `, including ${someNames(gw.map(z => z.name))}` : ''}. Join ${s(n, 'it', 'them')} to a road, or delete ${s(n, 'it', 'them')}.`,
        [...c.links.map(L => 'link:' + L.id), ...gw.map(z => 'zone:' + z.id)], at);
    }
    const inMain = id => !!main && main.nodes.has(id);

    // 2 · main-road dead ends near the edge of the network with no gateway
    if (ids.length > 1) {
      let sLat = Infinity, nLat = -Infinity, wLng = Infinity, eLng = -Infinity;
      for (const n of Object.values(nodes)) { sLat = Math.min(sLat, n.lat); nLat = Math.max(nLat, n.lat); wLng = Math.min(wLng, n.lng); eLng = Math.max(eLng, n.lng); }
      const midLat = (sLat + nLat) / 2, W = U.dist([midLat, wLng], [midLat, eLng]), H = U.dist([sLat, wLng], [nLat, wLng]);
      const margin = Math.max(R.boundaryM, R.boundaryShare * Math.min(W, H));
      const edge = ll => Math.min(U.dist(ll, [ll[0], wLng]), U.dist(ll, [ll[0], eLng]), U.dist(ll, [sLat, ll[1]]), U.dist(ll, [nLat, ll[1]]));
      for (const [id, Ls] of inc) {
        if (Ls.length !== 1 || pinned.has(id) || !inMain(id)) continue;
        const L = Ls[0]; if (rankOf(L) < 3) continue;
        if (edge(llOf(id)) > margin) continue;
        add('edge-no-gateway', 'warn', `${roadName(L)} (${clsLabel(L)}) ends near the edge of the network without a gateway, so no traffic enters or leaves there. Add a gateway (G) at the road end.`, ['node:' + id, 'link:' + L.id], llOf(id));
      }
    }

    // 3 · zones with no connector, or connectors longer than 400 m
    let cm = {};
    try { cm = Engine.connectorMap(sc) || {}; } catch (e) { cm = {}; }
    const connT = new Set();
    for (const z of zones) for (const t of cm[z.id] || []) connT.add(t);
    for (const z of zones) {
      const t = (cm[z.id] || []).filter(id => nodes[id]), zl = [z.lat, z.lng];
      if (!t.length) { add('no-connector', 'error', `${z.name} has no connector to the network, so its trips cannot load. Move it next to a connected road.`, ['zone:' + z.id], zl); continue; }
      if (z.kind === 'gateway') continue;
      const far = t.map(id => ({ id, d: U.dist(zl, llOf(id)) })).filter(x => x.d > R.connectorMaxM);
      if (far.length) add('connector-long', 'warn', `${z.name} connects to a junction ${U.fmt.len(Math.max(...far.map(x => x.d)))} away (more than ${R.connectorMaxM} m), so its trips start in the wrong place. Move the zone next to its access road, or draw the access road.`,
        ['zone:' + z.id, ...far.map(x => 'node:' + x.id)], zl);
    }

    // 4 · one-way traps: every road at a node points in (or out)
    for (const [id, Ls] of inc) {
      if (pinned.has(id) || connT.has(id)) continue;
      let i = 0, o = 0;
      for (const L of Ls) { const ab = (L.lanesAB | 0) > 0, ba = (L.lanesBA | 0) > 0; if (L.a === id) { if (ab) o++; if (ba) i++; } else { if (ab) i++; if (ba) o++; } }
      if (i && !o) add('trap-in', 'error', `Traffic can drive into ${where(id)} but not out: every road there is one-way towards it. Make one of them two-way, or swap its direction.`, ['node:' + id, ...Ls.map(L => 'link:' + L.id)], llOf(id));
      else if (o && !i) add('trap-out', 'error', `Nothing can drive into ${where(id)}: every road there is one-way away from it. Make one of them two-way, or swap its direction.`, ['node:' + id, ...Ls.map(L => 'link:' + L.id)], llOf(id));
    }

    // 5 · gateways and zones that one-way roads cut off from the rest of the network
    if (main) {
      const out = new Map(), rev = new Map(), push = (m, a, b) => { const x = m.get(a); if (x) x.push(b); else m.set(a, [b]); };
      for (const L of open) {
        if ((L.lanesAB | 0) > 0) { push(out, L.a, L.b); push(rev, L.b, L.a); }
        if ((L.lanesBA | 0) > 0) { push(out, L.b, L.a); push(rev, L.a, L.b); }
      }
      let root = null, rd = -1;
      for (const id of main.nodes) { const d = deg(id); if (d > rd) { rd = d; root = id; } }
      const reach = m => { const seen = new Set([root]), q = [root]; while (q.length) { const x = q.pop(); for (const y of m.get(x) || []) if (!seen.has(y)) { seen.add(y); q.push(y); } } return seen; };
      const F = reach(out), B = reach(rev);
      for (const z of zones) {
        const t = (cm[z.id] || []).filter(id => nodes[id] && inMain(id)); if (!t.length) continue;
        const g = z.gw || {}, isGw = z.kind === 'gateway';
        const enters = isGw ? (+g.AMin || 0) + (+g.PMin || 0) > 0 : true, leaves = isGw ? (+g.AMout || 0) + (+g.PMout || 0) > 0 : true;
        const refs = ['zone:' + z.id, ...t.map(id => 'node:' + id)];
        if (enters && !t.some(id => B.has(id))) add('cut-off-in', 'error', isGw ? `Traffic entering at ${z.name} cannot reach the rest of the network (one-way roads or a gap), so it is left unserved.` : `Trips from ${z.name} cannot reach the rest of the network (one-way roads or a gap), so they are left unserved.`, refs, [z.lat, z.lng]);
        if (leaves && !t.some(id => F.has(id))) add('cut-off-out', 'error', isGw ? `Traffic cannot reach ${z.name} to leave the area (one-way roads or a gap), so it is left unserved.` : `Trips to ${z.name} cannot reach it (one-way roads or a gap), so they are left unserved.`, refs, [z.lat, z.lng]);
      }
    }

    // 6 · proposed roads joined at one end only
    const free = n => deg(n) === 1 && !pinned.has(n) && !connT.has(n);
    for (const L of open) if (L.status === 'proposed' && free(L.a) !== free(L.b))
      add('proposed-one-end', 'warn', `${roadName(L)} (proposed) is joined to the network at one end only, so it only serves the land beside it. Join its other end to a road if it should carry through traffic.`, ['link:' + L.id], midOf(L));

    // 7 · signals or roundabouts with fewer than three legs
    const siteOf = new Map();
    for (const st of Object.values(sc.sites || {})) for (const id of st.nodes || []) siteOf.set(id, st);
    for (const n of Object.values(nodes)) {
      if (n.control !== 'signals' && n.control !== 'roundabout') continue;
      const d = deg(n.id);
      if (d >= 3 || (inc.get(n.id) || []).some(L => L.ring) || siteOf.has(n.id)) continue;
      const ped = n.control === 'signals' && d === 2 ? ' If it is a mid-block pedestrian crossing, it can stay as it is.' : '';
      add('control-legs', 'warn', `${CONTROLS[n.control]} at ${where(n.id)}, but only ${plural(d, 'road')} meet${d === 1 ? 's' : ''} there. A junction with ${n.control === 'signals' ? 'signals' : 'a roundabout'} needs at least three legs: set the control to Uncontrolled, or join the missing road.${ped}`, ['node:' + n.id], llOf(n.id));
    }

    // 8 · odd speed limits and lanes for the road class; 9 · main roads with no name
    const groups = new Map(), group = (key, L) => { const g = groups.get(key); if (g) g.push(L); else groups.set(key, [L]); };
    for (const L of open) {
      const C = R.classMax[L.cls] || R.classMax.local, sp = +L.speed, ln = Math.max(L.lanesAB | 0, L.lanesBA | 0);
      if (!(sp >= R.minSpeed)) group('speed-low', L);
      else if (sp > C.speed) group(`speed|${L.cls}|${sp}`, L);
      if (ln > C.lanes) group(`lanes|${L.cls}|${ln}`, L);
    }
    for (const [key, Ls] of groups) {
      const [kind, cls, v] = key.split('|'), n = Ls.length, names = [...new Set(Ls.map(L => L.name).filter(Boolean))], C = R.classMax[cls] || R.classMax.local;
      const on = names.length ? ` (${someNames(names, 2)})` : '', ref = Ls.map(L => 'link:' + L.id), at = midOf(Ls[0]), lab = cls ? (CLASSES[cls] || CLASSES.local).label.toLowerCase() : '';
      if (kind === 'speed-low') add('speed-missing', 'warn', `${plural(n, 'road segment')}${on} ${s(n, 'has', 'have')} no speed limit, or one below ${R.minSpeed} km/h. Set the speed limit in the road panel.`, ref, at);
      else if (kind === 'speed') add('speed-high', 'warn', `${plural(n, `${lab} segment`)}${on} ${s(n, 'has', 'have')} a ${v} km/h limit, above the ${C.speed} km/h Spare Construct expects for that class (Spare Construct assumption). Check the speed limit against the National Speed Limit Register, or the road class.`, ref, at);
      else add('lanes-high', 'warn', `${plural(n, `${lab} segment`)}${on} ${s(n, 'has', 'have')} ${v} lanes in one direction, more than the ${C.lanes} Spare Construct expects for that class (Spare Construct assumption). Check the lanes or the road class.`, ref, at);
    }
    const unnamed = open.filter(L => rankOf(L) >= 3 && !String(L.name || '').trim());
    if (unnamed.length) add('no-name', 'info', `${plural(unnamed.length, 'motorway, arterial or collector segment')} ${s(unnamed.length, 'has', 'have')} no name, so tables and reports show ${s(unnamed.length, 'it', 'them')} as unnamed. Name ${s(unnamed.length, 'it', 'them')} in the road panel.`, unnamed.map(L => 'link:' + L.id), midOf(unnamed[0]));

    // 10 · gateways on estimated flows
    const est = gws.filter(z => !z.gwSrc);
    if (est.length) add('gw-estimated', 'info', `${est.length} of ${plural(gws.length, 'gateway')} use estimated flows, not counts: ${someNames(est.map(z => z.name))}. Load counts in the Traffic data tab and set gateway flows from counts, or type in surveyed flows.`, est.map(z => 'zone:' + z.id), [est[0].lat, est[0].lng]);

    // 11 · connectors attached inside junction sites; 12 · site legs that collide in the 8-point scheme
    const siteName = st => { try { return (typeof Sites !== 'undefined' && Sites.name(sc, st)) || st.name || 'a junction site'; } catch { return st.name || 'a junction site'; } };
    for (const z of zones) {
      const st = z.node && siteOf.get(z.node);
      if (st && (st.nodes || []).length > 1) add('connector-in-site', 'warn', `${z.name} is attached inside the junction site ${siteName(st)}. Move it to a road end outside the junction, so its traffic uses one of the legs.`, ['zone:' + z.id, 'node:' + z.node], [z.lat, z.lng]);
    }
    if (typeof Turns !== 'undefined' && typeof Turns.assignLegs === 'function') for (const st of Object.values(sc.sites || {})) {
      const set = new Set(st.nodes || []), legs = [];
      for (const id of set) for (const L of inc.get(id) || []) {
        const other = L.a === id ? L.b : L.a; if (set.has(other)) continue;
        const g = Net.geom(sc, L), pts = L.a === id ? g : g.slice().reverse(), cum = U.cumLen(pts);
        legs.push({ b: U.bearing(pts[0], U.along(pts, cum, Math.min(30, cum[cum.length - 1])).p), nm: DataHub.norm(L.name), rank: rankOf(L), L });
      }
      if (legs.length < 3) continue;
      const p = legs.map((_, k) => k), f = k => { while (p[k] !== k) k = p[k] = p[p[k]]; return k; };
      for (let a = 0; a < legs.length; a++) for (let b = a + 1; b < legs.length; b++) {
        const d = U.angDiff(legs[a].b, legs[b].b);
        if ((legs[a].nm && legs[a].nm === legs[b].nm && d <= 90) || d <= 35) { const x = f(a), y = f(b); if (x !== y) p[y] = x; }
      }
      const gr = new Map();
      legs.forEach((x, k) => { const r = f(k); if (!gr.has(r)) gr.set(r, x); });
      const reps = [...gr.values()], coded = Turns.assignLegs(reps.map(x => x.b), reps.map(x => x.rank));
      const bad = coded.map((c, k) => (c && (c.merged || c.err > R.legTol) ? reps[k] : null)).filter(Boolean);
      if (bad.length) add('site-legs', 'warn', `Two or more legs of ${siteName(st)} point almost the same way, so their 8-point leg codes (S, SE, E … in 45° steps) clash for ${someNames(bad.map(x => x.L.name || 'an unnamed road'))}. Check the site's nodes, or split it.`,
        [...set].map(id => 'node:' + id).concat(bad.map(x => 'link:' + x.L.id)), llOf([...set][0]));
    }

    const order = { error: 0, warn: 1, info: 2 };
    issues.sort((a, b) => order[a.level] - order[b.level]);
    const counts = { error: 0, warn: 0, info: 0 };
    for (const it of issues) counts[it.level]++;
    return { issues, counts, ms: Math.round((performance.now() - t0) * 10) / 10, at: Date.now(), sc: sc.id, rules: R };
  }
  const checkSummary = c => [c.counts.error ? plural(c.counts.error, 'problem') : '', c.counts.warn ? `${c.counts.warn} to check` : '', c.counts.info ? plural(c.counts.info, 'note') : ''].filter(Boolean).join(', ') || 'No problems';
  let lastCheck = null;
  function setIssues(c) { lastCheck = c; View.issues = c ? c.issues : null; View.issueFocus = null; }
  const refsOf = it => (it.ids || []).map(s => { const k = s.indexOf(':'); return [s.slice(0, k), s.slice(k + 1)]; });
  // The selection a click on an issue makes
  function selForIssue(it, sc = Store.sc) {
    const r = refsOf(it), links = r.filter(([t, id]) => t === 'link' && sc.links[id]).map(x => x[1]), nodes = r.filter(([t, id]) => t === 'node' && sc.nodes[id]).map(x => x[1]);
    const zones = r.filter(([t, id]) => t === 'zone' && sc.zones[id]).map(x => x[1]);
    if (zones.length && (r[0] || [])[0] === 'zone') return { type: 'zone', id: zones[0] };
    return makeSel(links, links.length ? [] : nodes) || (zones.length ? { type: 'zone', id: zones[0] } : null);
  }
  // Points to fit the map to for a list of 'type:id' references
  function pointsOf(ids, sc = Store.sc) {
    const pts = [];
    for (const [t, id] of refsOf({ ids })) {
      if (t === 'link' && sc.links[id] && sc.nodes[sc.links[id].a] && sc.nodes[sc.links[id].b]) pts.push(...Net.geom(sc, sc.links[id]));
      else if (t === 'node' && sc.nodes[id]) pts.push([sc.nodes[id].lat, sc.nodes[id].lng]);
      else if (t === 'zone' && sc.zones[id]) pts.push([sc.zones[id].lat, sc.zones[id].lng]);
    }
    return pts;
  }
  function flyToPoints(pts) {
    if (!hasUI() || !pts.length) return;
    const b = L.latLngBounds(pts);
    if (pts.length === 1 || U.dist([b.getSouth(), b.getWest()], [b.getNorth(), b.getEast()]) < 60) UI.map.setView(b.getCenter(), Math.max(UI.map.getZoom(), 16));
    else UI.map.fitBounds(b.pad(0.4), { maxZoom: 17 });
  }
  function focusIssue(i) {
    const it = (View.issues || [])[i]; if (!it) return;
    View.issueFocus = i;
    select(selForIssue(it));
    flyToPoints(pointsOf(it.ids));
  }
  function runCheck(open = false) {
    const c = check(Store.sc);
    setIssues(c);
    if (hasUI()) {
      Render.redraw(); UI.renderLegend();
      if (open) UI.openTab('onboarding-check'); else if (View.drawer === 'onboarding-check') Drawer.render();
      Kit.toast(c.issues.length ? `Network check: ${checkSummary(c).toLowerCase()}.` : 'Network check: no problems found.');
    }
    return c;
  }
  function checkTab() {
    const c = lastCheck, list = View.issues || [];
    if (!c) return `<div class="bar-h"><h3>Network check</h3><button class="btn small" data-onb="recheck">Check the network</button></div><p class="note">Finds roads that are not connected, zones without connectors, one-way traps, unreachable gateways, missing names and odd speed limits or lanes.</p>`;
    const sc = Store.P.scenarios.find(s => s.id === c.sc);
    const rows = list.map((it, i) => {
      const [cls, sym, lab] = LEVEL[it.level] || LEVEL.info;
      return `<tr class="click${View.issueFocus === i ? ' on' : ''}" data-onb-issue="${i}" tabindex="0" aria-label="${lab}: ${esc(it.msg)}"><td><span class="los ${cls}" role="img" aria-label="${lab}">${sym}</span></td><td style="white-space:normal">${esc(it.msg)}</td><td class="n">${plural(it.ids.length, 'object')}</td></tr>`;
    }).join('');
    return `<div class="bar-h"><h3>Network check</h3><span class="note">${esc(sc ? sc.name : 'Scenario')} · ${esc(checkSummary(c))} · checked in ${U.fmt.num(c.ms, 0)} ms</span><button class="btn small" data-onb="recheck">Check again</button></div>
      ${list.length ? `<div class="tbl-wrap"><table class="t compact"><tr><th></th><th>What Spare Construct found (click to show it on the map)</th><th class="n">Objects</th></tr>${rows}</table></div>` : '<p class="note">No problems found. Every road is connected, and every zone and gateway can send and receive traffic.</p>'}
      <p class="note">The thresholds (connectors over ${CHECK_RULES.connectorMaxM} m, speed limits and lanes for each class, dead ends near the edge) are Spare Construct assumptions, not published rules. Issues are shown in magenta on the map.</p>`;
  }

  /* ---------- multi-select (multi-edit) ---------- */
  function makeSel(links = [], nodes = []) {
    links = [...new Set(links)]; nodes = [...new Set(nodes)];
    const n = links.length + nodes.length;
    if (!n) return null;
    if (n === 1) return links.length ? { type: 'link', id: links[0] } : { type: 'node', id: nodes[0] };
    return { type: 'multi', id: 'multi', links, nodes, ids: links };
  }
  // Any selection → { links, nodes }. Idempotent: a { links, nodes } list (multiOf's own result) passes through.
  function multiOf(sel) {
    if (!sel) return { links: [], nodes: [] };
    if (sel.type === 'multi' || (!sel.type && (Array.isArray(sel.links) || Array.isArray(sel.nodes)))) return { links: [...(sel.links || [])], nodes: [...(sel.nodes || [])] };
    if (sel.type === 'link') return { links: [sel.id], nodes: [] };
    if (sel.type === 'node') return { links: [], nodes: [sel.id] };
    return { links: [], nodes: [] };
  }
  function toggle(sel, type, id) {
    const m = multiOf(sel), arr = type === 'link' ? m.links : m.nodes, i = arr.indexOf(id);
    if (i >= 0) arr.splice(i, 1); else arr.push(id);
    return makeSel(m.links, m.nodes);
  }
  function select(sel) {
    if (hasUI()) { UI.select(sel); Render.redraw(); } else View.sel = sel;
  }
  function incidence(sc) {
    const inc = new Map();
    for (const L of Object.values(sc.links)) for (const n of [L.a, L.b]) { const a = inc.get(n); if (a) a.push(L); else inc.set(n, [L]); }
    return inc;
  }
  // Every segment of the same named street joined to this one (roundabout rings left out)
  function wholeStreet(sc, id) {
    const L0 = sc.links[id]; if (!L0) return { ids: [], len: 0 };
    const key = DataHub.norm(L0.name);
    let ids = [id];
    if (key && !L0.ring) {
      const inc = incidence(sc), seen = new Set([id]), q = [L0];
      while (q.length) {
        const L = q.shift();
        for (const n of [L.a, L.b]) for (const M of inc.get(n) || []) if (!seen.has(M.id) && !M.ring && DataHub.norm(M.name) === key) { seen.add(M.id); q.push(M); }
      }
      ids = [...seen];
    }
    return { ids, len: ids.reduce((s, k) => s + (sc.nodes[sc.links[k].a] && sc.nodes[sc.links[k].b] ? Net.len(sc, sc.links[k]) : 0), 0) };
  }
  // Roads with any part inside a box [[s, w], [n, e]]
  const orient = (a, b, c) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  const crosses = (p1, p2, p3, p4) => ((orient(p3, p4, p1) > 0) !== (orient(p3, p4, p2) > 0)) && ((orient(p1, p2, p3) > 0) !== (orient(p1, p2, p4) > 0));
  function linksInBox(sc, b) {
    const [[s, w], [n, e]] = normBox(b), inside = p => p[0] >= s && p[0] <= n && p[1] >= w && p[1] <= e;
    const edges = [[[s, w], [s, e]], [[s, e], [n, e]], [[n, e], [n, w]], [[n, w], [s, w]]];
    const out = [];
    for (const L of Object.values(sc.links)) {
      if (!sc.nodes[L.a] || !sc.nodes[L.b]) continue;
      const g = Net.geom(sc, L);
      let la0 = Infinity, la1 = -Infinity, ln0 = Infinity, ln1 = -Infinity;
      for (const p of g) { la0 = Math.min(la0, p[0]); la1 = Math.max(la1, p[0]); ln0 = Math.min(ln0, p[1]); ln1 = Math.max(ln1, p[1]); }
      if (la1 < s || la0 > n || ln1 < w || ln0 > e) continue;
      for (let i = 0; i < g.length; i++) {
        if (inside(g[i]) || (i && edges.some(([p, q]) => crosses(g[i - 1], g[i], p, q)))) { out.push(L.id); break; }
      }
    }
    return out;
  }

  // Fields shared by every selected object, or MIXED
  const MIXED = Object.freeze({ mixed: true });
  const same = vals => { if (!vals.length) return MIXED; const f = JSON.stringify(vals[0]); return vals.every(v => JSON.stringify(v) === f) ? vals[0] : MIXED; };
  const lanesEach = L => ((L.lanesAB | 0) > 0 && (L.lanesBA | 0) > 0 ? (L.lanesAB === L.lanesBA ? L.lanesAB | 0 : 'uneven') : Math.max(L.lanesAB | 0, L.lanesBA | 0));
  function multiFields(sc, sel) {
    const links = (sel.links || []).map(id => sc.links[id]).filter(Boolean), nodes = (sel.nodes || []).map(id => sc.nodes[id]).filter(Boolean);
    const lanes = same(links.map(lanesEach));
    return {
      links, nodes,
      cls: same(links.map(L => L.cls)), status: same(links.map(L => L.status)), speed: same(links.map(L => +L.speed)),
      lanes: lanes === 'uneven' ? MIXED : lanes, cycle: same(links.map(L => L.cycle || 'none')), control: same(nodes.map(n => n.control || 'none')),
    };
  }
  // One field on one road; dry = only say whether it would change. Mirrors Inspector.applyField's rules.
  function setLinkField(L, f, v, dry = false) {
    if (f === 'lanes') {
      const n = Math.max(1, Math.round(v)), two = (L.lanesAB | 0) > 0 && (L.lanesBA | 0) > 0, none = !((L.lanesAB | 0) > 0) && !((L.lanesBA | 0) > 0);
      const ab = two || none || (L.lanesAB | 0) > 0 ? n : 0, ba = two || none || (L.lanesBA | 0) > 0 ? n : 0;
      if ((L.lanesAB | 0) === ab && (L.lanesBA | 0) === ba) return false;
      if (!dry) { L.lanesAB = ab; L.lanesBA = ba; if (Math.max(ab, ba) < 2) L.busLane = false; }
      return true;
    }
    const cur = f === 'cycle' ? L.cycle || 'none' : f === 'speed' ? +L.speed : L[f];
    if (cur === v) return false;
    if (!dry) {
      if (f === 'cls') { const prev = CLASSES[L.cls]; if (prev && +L.speed === prev.speed) L.speed = CLASSES[v].speed; }
      L[f] = v;
    }
    return true;
  }
  function checkValue(f, raw) {
    if (f === 'cls') { if (!CLASSES[raw]) throw new Error('Choose a road class.'); return raw; }
    if (f === 'status') { if (!STATUS[raw]) throw new Error('Choose a status.'); return raw; }
    if (f === 'cycle') { if (!CYCLE[raw]) throw new Error('Choose a cycling facility.'); return raw; }
    if (f === 'control') { if (!CONTROLS[raw]) throw new Error('Choose a junction control.'); return raw; }
    const n = parseNum(raw);
    if (f === 'speed') { if (!(n >= 5 && n <= 130)) throw new Error('Enter a speed limit between 5 and 130 km/h.'); return Math.round(n); }
    if (f === 'lanes') { if (!(n >= 1 && n <= 8)) throw new Error('Enter between 1 and 8 lanes each way.'); return Math.round(n); }
    throw new Error(`Spare Construct cannot set '${f}' on several roads at once.`);
  }
  // Snapshot a scenario that may not be the active one (Store.snapshot works on the active scenario)
  function snapshotOf(s, label) {
    const a = Store.P.active;
    Store.P.active = s.id;
    try { snap(label); } finally { Store.P.active = a; }
  }
  const stale = s => { for (const r of Object.values(s.res || {})) if (r) r.stale = true; };
  const scenariosFor = ids => {
    const P = Store.P, list = [Store.sc];
    for (const id of ids || []) { const s = P.scenarios.find(x => x.id === id); if (s && !list.includes(s)) list.push(s); }
    return list;
  };
  /* The same roads in another scenario s: each id, plus the pieces s has where a new road split it there
     (links only in s, same name and class, lying on the road in `from`) → { ids, missing } */
  function linksIn(s, from, ids) {
    if (s === from) return { ids: ids.filter(id => s.links[id]), missing: ids.filter(id => !s.links[id]).length };
    const fresh = Object.values(s.links).filter(M => !from.links[M.id] && s.nodes[M.a] && s.nodes[M.b]), out = new Set();
    let missing = 0;
    for (const id of ids) {
      const L = from.links[id], before = out.size;
      if (s.links[id]) out.add(id);
      if (L && fresh.length && from.nodes[L.a] && from.nodes[L.b]) {
        const nm = DataHub.norm(L.name);
        for (const M of fresh) if (M.cls === L.cls && DataHub.norm(M.name) === nm && Net.project(from, L, midPoint(s, M)).d < SPLIT_M) out.add(M.id);
      }
      if (out.size === before && !s.links[id]) missing++;
    }
    return { ids: [...out], missing };
  }
  /* Apply one field to every selected road (and junction, for control) in this scenario and the ticked ones,
     under one undo step per scenario → { changed, scenarios: [names], missing } */
  function applyMulti(sel, f, raw, scIds = []) {
    sel = multiOf(sel);   // a single road or junction works too
    const v = checkValue(f, raw), list = scenariosFor(scIds), links = sel.links || [], nodes = sel.nodes || [], from = Store.sc;
    const plan = list.map(s => {
      let n = 0, missing = 0;
      const ls = f === 'control' ? null : linksIn(s, from, links);
      if (f === 'control') { for (const id of nodes) { const N = s.nodes[id]; if (!N) missing++; else if ((N.control || 'none') !== v) n++; } }
      else { missing = ls.missing; for (const id of ls.ids) if (setLinkField(s.links[id], f, v, true)) n++; }
      return { s, n, missing, ids: ls ? ls.ids : null };
    });
    const todo = plan.filter(x => x.n);
    if (!todo.length) return { changed: 0, scenarios: [], missing: plan.reduce((t, x) => t + x.missing, 0) };
    // the other scenarios first, so the first Ctrl+Z undoes the one on screen
    for (const x of todo.slice().reverse()) snapshotOf(x.s, `${FIELD_LABEL[f] || f} on ${plural(links.length || nodes.length, links.length ? 'road' : 'junction')}`);
    let changed = 0;
    for (const { s, ids } of todo) {
      if (f === 'control') for (const id of nodes) { const N = s.nodes[id]; if (N && (N.control || 'none') !== v) { N.control = v; N.upgrade = v !== 'none' && v !== 'priority'; changed++; } }
      else for (const id of ids) { const L = s.links[id]; if (L && setLinkField(L, f, v)) changed++; }
      if (s !== Store.sc) stale(s);
    }
    quiet(() => Store.edited('net'));
    return { changed, scenarios: todo.map(x => x.s.name), missing: plan.reduce((t, x) => t + x.missing, 0) };
  }
  // The scenarios a delete reaches: this one and the ticked ones that hold any of the selection
  function deleteTargets(sel, scIds = []) {
    sel = multiOf(sel);
    const from = Store.sc;
    return scenariosFor(scIds).filter(s => linksIn(s, from, sel.links || []).ids.length || (sel.nodes || []).some(id => s.nodes[id]));
  }
  function deleteMulti(sel, scIds = []) {
    sel = multiOf(sel);
    const from = Store.sc, list = deleteTargets(sel, scIds);
    if (!list.length) return { scenarios: [], links: 0 };
    for (const s of list.slice().reverse()) snapshotOf(s, `deleting ${plural((sel.links || []).length || (sel.nodes || []).length, (sel.links || []).length ? 'road' : 'junction')}`);
    let n = 0;
    for (const s of list) {
      for (const id of linksIn(s, from, sel.links || []).ids) if (s.links[id]) { Net.deleteLink(s, id); if (s === from) n++; }
      for (const id of sel.nodes || []) if (s.nodes[id]) Net.deleteNode(s, id);
      if (s !== from) stale(s);
    }
    View.sel = null;
    quiet(() => Store.edited('net'));
    return { scenarios: list.map(s => s.name), links: n };
  }
  const GEOM = ['a', 'b', 'pts'];
  // Put the selected roads back as they are in the base scenario. A road split where a new road joins it keeps its alignment.
  function resetMulti(sel) {
    sel = multiOf(sel);
    const sc = Store.sc, base = Store.baseSc; if (sc === base) return 0;
    const ids = (sel.links || []).filter(id => sc.links[id] && base.links[id] && linkRows(base.links[id], sc.links[id], base, sc).rows.length);
    const nids = (sel.nodes || []).filter(id => sc.nodes[id] && base.nodes[id] && (sc.nodes[id].control || 'none') !== (base.nodes[id].control || 'none'));
    if (!ids.length && !nids.length) return 0;
    snap('reset to base');
    for (const id of ids) {
      const B = base.links[id], L = sc.links[id];
      if (isSplit(L, sc, B, base)) { const keep = Object.fromEntries(GEOM.filter(k => L[k] !== undefined).map(k => [k, L[k]])), c = structuredClone(B); for (const k of GEOM) delete c[k]; sc.links[id] = { ...c, ...keep, id }; continue; }
      for (const n of [B.a, B.b]) if (!sc.nodes[n] && base.nodes[n]) sc.nodes[n] = structuredClone(base.nodes[n]);
      sc.links[id] = structuredClone(B);
    }
    for (const id of nids) { const B = base.nodes[id], N = sc.nodes[id]; for (const k of ['control', 'cycle', 'fixedTiming', 'upgrade']) { if (B[k] === undefined) delete N[k]; else N[k] = B[k]; } }
    quiet(() => Store.edited('net'));
    return ids.length + nids.length;
  }

  /* Other scenarios ticked (this session only): 'Edits apply to' (the multi panel and the single-object panels share
     it), and 'Copy to' for a one-off copy of one object. Kept apart, so a scenario ticked for a copy never receives
     later edits or deletes. */
  let scope = new Set(), copyScope = new Set();
  // Our own multi-scenario edits apply the scope themselves: quiet() keeps mirrorEdit (below) from copying them again
  let mirroring = false, setupDepth = 0, mirrorSeen = new WeakSet();
  const quiet = fn => { const was = mirroring; mirroring = true; try { return fn(); } finally { mirroring = was; } };
  function scopeHtml(sc, what, status, set = scope, attr = 'data-onb-scope') {
    const others = Store.P.scenarios.filter(s => s !== sc);
    if (!others.length) return '';
    return `<div role="group" aria-label="${esc(what)}" style="display:grid;gap:4px">
      <label class="check"><input type="checkbox" checked disabled> ${esc(sc.name)} <span class="note">(this scenario)</span></label>
      ${others.map(s => `<label class="check"><input type="checkbox" ${attr}="${esc(s.id)}" ${set.has(s.id) ? 'checked' : ''}> ${esc(s.name)}${status ? ` <span class="note">(${esc(status(s))})</span>` : ''}</label>`).join('')}</div>`;
  }
  function multiPanel(sc, sel) {
    const F = multiFields(sc, sel), links = F.links, nodes = F.nodes;
    if (!links.length && !nodes.length) return '';
    const len = links.reduce((t, L) => t + Net.len(sc, L), 0);
    const title = [links.length ? `${plural(links.length, 'road')} selected · ${U.fmt.len(len)}` : '', nodes.length ? `${plural(nodes.length, 'junction')}${links.length ? '' : ' selected'}` : ''].filter(Boolean).join(' and ');
    const pick = (f, label, pairs, v, unit = '') => Kit.fld(label, `<select data-onb-f="${f}" aria-label="${esc(label)}">${v === MIXED ? '<option value="" selected disabled>Mixed</option>' : ''}${Kit.opt(pairs, v === MIXED ? null : v)}</select>`, unit);
    const speeds = Object.fromEntries(SPEEDS.map(x => [x, x + ' km/h']));
    if (F.speed !== MIXED && !(F.speed in speeds)) speeds[F.speed] = F.speed + ' km/h';
    const roads = links.length ? Kit.sec('Roads', `
      ${pick('cls', 'Class', Object.fromEntries(Object.entries(CLASSES).map(([k, c]) => [k, c.label])), F.cls)}
      ${pick('status', 'Status', STATUS, F.status)}
      ${pick('speed', 'Speed limit', speeds, F.speed)}
      ${pick('lanes', 'Lanes', Object.fromEntries([1, 2, 3, 4, 5, 6].map(n => [n, n + ' each way'])), F.lanes)}
      ${pick('cycle', 'Cycling', CYCLE, F.cycle)}
      <p class="note">“Mixed” means the selected roads differ. A change applies to all of them in one step (Ctrl+Z undoes it); one-way roads keep their one direction.</p>`) : '';
    const juncs = nodes.length ? Kit.sec('Junctions', pick('control', 'Control', CONTROLS, F.control)) : '';
    const multi = Store.P.scenarios.length > 1;
    const miss = s => { const k = linksIn(s, sc, links.map(L => L.id)).missing + nodes.filter(n => !s.nodes[n.id]).length; return k ? `${k} not in it` : 'all in it'; };
    const scopeSec = multi ? Kit.sec('Edits apply to', scopeHtml(sc, 'Scenarios the edits apply to', miss) + '<p class="note">Changes go to the same roads and junctions in each ticked scenario; their results go out of date.</p>') : '';
    let baseNote = '';
    if (multi && sc !== Store.baseSc) {
      const base = Store.baseSc, differ = links.filter(L => base.links[L.id] && linkRows(base.links[L.id], L, base, sc).rows.length).length, fresh = links.filter(L => !base.links[L.id]).length;
      baseNote = Kit.note(`${differ ? `<span class="onb-dot" aria-hidden="true"></span>${plural(differ, 'road')} of these differ${differ === 1 ? 's' : ''} from ${esc(base.name)}` : `None of these roads differ from ${esc(base.name)}`}${fresh ? `; ${fresh} ${fresh === 1 ? 'is' : 'are'} new in this scenario` : ''}.${differ ? ' <a href="#" data-onb="reset-multi">Reset them to base</a>' : ''}`);
    }
    return `<div class="ins-head"><div class="ins-kind">Selection<span style="margin-left:auto"><button class="btn small icon" data-act="close" title="Clear the selection (Esc)" aria-label="Clear the selection">✕</button></span></div>
        <div style="font:600 17px/1.25 var(--f-ui)">${esc(title)}</div></div>
      ${roads}${juncs}${scopeSec}${baseNote ? `<div class="ins-sec">${baseNote}</div>` : ''}
      ${sections()}
      <div class="ins-sec"><p class="note">Shift+click adds or removes a road or junction; Shift+drag adds the roads in a box.</p><div class="actions">
        <button class="btn small" data-onb="zoom-multi">Zoom to</button>
        <button class="btn small danger" data-onb="delete-multi">Delete ${links.length ? plural(links.length, 'road') : plural(nodes.length, 'junction')}</button></div></div>`;
  }
  function onMultiChange(e) {
    const t = e.target, sel = View.sel;
    if (t.dataset.onbScope) { if (t.checked) scope.add(t.dataset.onbScope); else scope.delete(t.dataset.onbScope); return true; }
    if (!t.dataset.onbF || !sel || sel.type !== 'multi' || t.value === '') return false;
    try {
      const r = applyMulti(sel, t.dataset.onbF, t.value, [...scope]);
      if (r.changed) Kit.toast(`Changed in ${andList(r.scenarios)}.${r.scenarios.length > 1 ? ' Ctrl+Z undoes one scenario at a time.' : ''}${r.missing ? ` ${r.missing} selected object${r.missing === 1 ? ' was' : 's were'} not in every scenario.` : ''}`);
    } catch (err) { Kit.toast(err.message, 'warn'); Inspector.render(); }
    return true;
  }
  function onMultiClick(e) {
    const b = e.target.closest('[data-onb]'), sel = View.sel; if (!b || !sel) return false;
    e.preventDefault();
    const act = b.dataset.onb;
    if (act === 'zoom-multi') flyToPoints(pointsOf([...(sel.links || []).map(id => 'link:' + id), ...(sel.nodes || []).map(id => 'node:' + id)]));
    else if (act === 'delete-multi') onMultiDelete(sel).catch(err => Kit.toast(err.message, 'err'));
    else if (act === 'reset-multi') { const n = resetMulti(sel); Kit.toast(n ? `Reset ${plural(n, 'object')} to ${Store.baseSc.name}. Ctrl+Z to undo.` : 'Nothing to reset.'); }
    else return false;
    return true;
  }
  async function onMultiDelete(sel) {
    // a delete that reaches other scenarios is confirmed first: Ctrl+Z undoes them one at a time
    const list = deleteTargets(sel, [...scope]), others = list.filter(s => s !== Store.sc), m = multiOf(sel);
    if (!list.length) return;
    if (others.length) {
      const what = m.links.length ? plural(m.links.length, 'road') : plural(m.nodes.length, 'junction');
      if (!(await Kit.confirmBox(`Delete in ${plural(list.length, 'scenario')}?`, `This deletes ${what} in ${andList(list.map(s => s.name))}, because ${andList(others.map(s => s.name))} ${others.length === 1 ? 'is' : 'are'} ticked under Edits apply to. Ctrl+Z undoes one scenario at a time.`, 'Delete'))) return;
    }
    const r = deleteMulti(sel, [...scope]);
    if (hasUI()) { UI.select(null); Render.redraw(); }
    Kit.toast(`Deleted ${plural(r.links || (sel.nodes || []).length, r.links ? 'road' : 'junction')} in ${andList(r.scenarios)}. Ctrl+Z to undo${r.scenarios.length > 1 ? ' (one scenario at a time)' : ''}.`);
  }

  /* ---------- scenario consistency ---------- */
  const yes = v => (v ? 'yes' : 'no');
  const lanesText = L => { const a = L.lanesAB | 0, b = L.lanesBA | 0; return a && b ? (a === b ? `${a} each way` : `${a} A→B, ${b} B→A`) : `one-way, ${Math.max(a, b)} lane${Math.max(a, b) === 1 ? '' : 's'}`; };
  const posText = o => `${(+o.lat).toFixed(5)}, ${(+o.lng).toFixed(5)}`;
  const luVal = v => (+v > 0 ? +v : null), bool = v => !!v;
  // Field descriptors: key, label, paths (default [key]), text(o, s)
  function fieldsFor(type, a, b) {
    if (type === 'link') return [
      { key: 'name', label: 'Name', text: o => o.name || 'unnamed' },
      { key: 'cls', label: 'Class', text: o => (CLASSES[o.cls] || {}).label || o.cls },
      { key: 'status', label: 'Status', text: o => STATUS[o.status] || o.status },
      { key: 'speed', label: 'Speed limit', text: o => o.speed + ' km/h' },
      { key: 'lanes', label: 'Lanes', paths: ['lanesAB', 'lanesBA'], text: lanesText },
      { key: 'capPerLane', label: 'Capacity', norm: v => (v == null || v === '' ? null : +v), text: o => (o.capPerLane == null ? 'class default' : o.capPerLane + ' pcu/h/lane') },
      { key: 'busLane', label: 'Bus lane', norm: bool, text: o => yes(o.busLane) },
      { key: 'cycle', label: 'Cycling', norm: v => v || 'none', text: o => CYCLE[o.cycle || 'none'] },
      { key: 'geom', label: 'Alignment', paths: ['a', 'b', 'pts'], text: (o, s) => { try { return U.fmt.len(Net.len(s, o)) + ' long'; } catch { return 'moved'; } } },
    ];
    if (type === 'node') return [
      { key: 'control', label: 'Control', norm: v => v || 'none', text: o => CONTROLS[o.control || 'none'] },
      { key: 'cycle', label: 'Cycle time', norm: v => (+v > 0 ? +v : null), text: o => (o.cycle ? o.cycle + ' s' : 'optimised') },
      { key: 'fixedTiming', label: 'Equal green splits', norm: bool, text: o => yes(o.fixedTiming) },
      { key: 'upgrade', label: 'New or upgraded', norm: bool, text: o => yes(o.upgrade) },
      { key: 'bans', label: 'Banned turns', norm: v => (Array.isArray(v) && v.length ? v : null), text: o => plural((o.bans || []).length, 'banned turn') },
      { key: 'position', label: 'Position', paths: ['lat', 'lng'], text: posText },
    ];
    if (type === 'gateway') return [
      { key: 'name', label: 'Name', text: o => o.name || 'unnamed' },
      ...[['AMin', 'AM entering'], ['AMout', 'AM leaving'], ['PMin', 'PM entering'], ['PMout', 'PM leaving']].map(([k, l]) => ({ key: 'gw.' + k, label: l, norm: v => +v || 0, text: o => U.fmt.int(+((o.gw || {})[k]) || 0) + ' veh/h' })),
      { key: 'position', label: 'Position', paths: ['lat', 'lng', 'node'], text: posText },
    ];
    const keys = [...new Set([...Object.keys((a && a.lu) || {}), ...Object.keys((b && b.lu) || {})])];
    return [
      { key: 'name', label: 'Name', text: o => o.name || 'unnamed' },
      ...keys.map(k => ({ key: 'lu.' + k, label: (LANDUSES[k] || { label: k }).label, norm: luVal, text: o => { const v = luVal((o.lu || {})[k]); return v ? `${U.fmt.int(v)} ${(LANDUSES[k] || { unit: '' }).unit}` : 'none'; } })),
      { key: 'connectors', label: 'Connectors', text: o => (o.connectors == null ? 'default' : String(o.connectors)) },
      { key: 'year', label: 'Opening year', norm: v => (+v > 0 ? +v : null), text: o => (o.year ? String(o.year) : 'always open') },
      { key: 'reduction', label: 'Trip reduction', norm: v => +v || 0, text: o => (+o.reduction || 0) + '%' },
      { key: 'existing', label: 'Existing land use', norm: bool, text: o => yes(o.existing) },
      { key: 'position', label: 'Position', paths: ['lat', 'lng'], text: posText },
    ];
  }
  // Fields where object b (in scenario sb) differs from a (in sa) → [{ key, label, paths, base, here }]
  function compareObj(type, a, b, sa, sb) {
    const out = [];
    for (const f of fieldsFor(type, a, b)) {
      const paths = f.paths || [f.key], nz = f.norm || (v => (v === undefined || v === '' ? null : v));
      const va = JSON.stringify(paths.map(p => nz(Kit.getPath(a, p)))), vb = JSON.stringify(paths.map(p => nz(Kit.getPath(b, p))));
      if (va !== vb) out.push({ key: f.key, label: f.label, paths, base: f.text(a, sa), here: f.text(b, sb) });
    }
    return out;
  }
  const typeOf = (s, sel) => (sel.type === 'zone' ? (s.zones[sel.id] && s.zones[sel.id].kind === 'gateway' ? 'gateway' : 'zone') : sel.type);
  const objOf = (s, t, id) => (t === 'link' ? s.links[id] : t === 'node' ? s.nodes[id] : t === 'zone' ? s.zones[id] : null);
  const NOUN = { link: 'road', node: 'junction', zone: 'zone', gateway: 'gateway' };

  /* Roads split where a new road joins them. Drawing a road that meets an existing one mid-block splits it
     (Net.splitLink): the piece that keeps the id ends at the new junction and the rest gets a new id. Such a
     piece differs from its base road only in its ends, and resetting or copying its alignment would put the
     whole road back on top of the other piece. */
  const SPLIT_M = 5;   // metres: a midpoint this close to the other road lies on it
  const midPoint = (s, L) => { const g = Net.geom(s, L); return U.along(g, U.cumLen(g), U.polyLen(g) / 2).p; };
  function isSplit(L, s, B, base) {
    if (!L || !B || (L.a === B.a && L.b === B.b) || (L.a === B.b && L.b === B.a)) return false;
    if (!s.nodes[L.a] || !s.nodes[L.b] || !base.nodes[B.a] || !base.nodes[B.b]) return false;
    return Net.project(base, B, midPoint(s, L)).d < SPLIT_M || Net.project(s, L, midPoint(base, B)).d < SPLIT_M;
  }
  // Link differences without the alignment of a split piece → { rows, split }
  function linkRows(B, L, base, s) {
    const rows = compareObj('link', B, L, base, s);
    if (!rows.some(r => r.key === 'geom') || !isSplit(L, s, B, base)) return { rows, split: false };
    return { rows: rows.filter(r => r.key !== 'geom'), split: true };
  }
  const rowsFor = (type, a, b, sa, sb) => (type === 'link' ? linkRows(a, b, sa, sb) : { rows: compareObj(type, a, b, sa, sb), split: false });
  function scenarioSection(sc, sel) {
    const P = Store.P; if (P.scenarios.length < 2) return '';
    const type = typeOf(sc, sel), obj = objOf(sc, sel.type, sel.id); if (!obj) return '';
    const base = Store.baseSc, parts = [];
    if (base !== sc) {
      const bo = objOf(base, sel.type, sel.id);
      if (!bo) parts.push(Kit.note(`New in this scenario: not in ${esc(base.name)}.`));
      else {
        const { rows, split } = rowsFor(type, bo, obj, base, sc);
        parts.push(rows.length ? rows.map(r => `<div class="onb-diff"><span class="onb-dot" aria-hidden="true"></span><span title="${esc(base.name)}: ${esc(r.base)}"><b>${esc(r.label)}</b> · base ${esc(r.base)} · here ${esc(r.here)}</span> <a href="#" data-onb-reset="${esc(r.key)}">Reset to base</a></div>`).join('')
          : Kit.note(`Same as ${esc(base.name)}${split ? ' apart from the split' : ''}.`));
        if (split) parts.push(Kit.note(`This road is split where a new road joins it (this piece ${esc(U.fmt.len(Net.len(sc, obj)))}, the whole road ${esc(U.fmt.len(Net.len(base, bo)))} in ${esc(base.name)}). Its alignment stays as it is; the other settings compare with the whole road.`));
      }
    }
    const status = s => { const o = objOf(s, sel.type, sel.id); return !o ? 'not in it' : rowsFor(type, o, obj, s, sc).rows.length ? 'different' : 'same'; };
    parts.push(`<div class="lg-title" style="margin-top:8px">Edits apply to</div>${scopeHtml(sc, 'Edits apply to', status)}
      <p class="note">Edits you make in ${esc(sc.name)} also go to the ticked scenarios: changed settings, and new roads, zones and gateways. Deleting stays in this scenario. Their results go out of date.</p>`);
    parts.push(`<details class="onb-copy"${[...copyScope].some(id => id !== sc.id) ? ' open' : ''}><summary>Copy this ${NOUN[type]} as it is now…</summary>
      <div style="margin-top:6px">${scopeHtml(sc, 'Scenarios to copy to', status, copyScope, 'data-onb-copyto')}</div>
      <div class="actions"><button class="btn small" data-onb="copy-to">Copy this ${NOUN[type]} to the ticked scenarios</button></div><p class="note">Copying replaces all its settings there, once, and their results go out of date.</p></details>`);
    return Kit.sec('Scenarios', parts.join(''));
  }
  // How copying a road into s goes: 'copy' (whole), 'settings' (s has it split differently: keep its alignment there)
  // or 'skip' (a piece of a road split in `from` that would lie on top of the whole road in s)
  function copyHow(type, src, from, s) {
    if (type !== 'link') return 'copy';
    const T = s.links[src.id];
    if (T) return (T.a === src.a && T.b === src.b) || (T.a === src.b && T.b === src.a) ? 'copy' : 'settings';
    return onBase(src, from, s) ? 'skip' : 'copy';
  }
  // Metres along road L (in scenario s) to the point nearest p
  const alongOf = (s, L, p) => { const r = Net.project(s, L, p), cum = U.cumLen(r.g); return cum[r.i] + U.dist(r.g[r.i], r.p); };
  /* Node n of `from` is missing in s. Where it lies part-way along a road T of s (a new road joined T there and split
     it), split T in s the same way: the pieces T has in `from`, with the same ids and T's own settings in s. So a copied
     road joins the network in s as it does in `from`, instead of ending at a loose junction. → true when split */
  function copySplitAt(s, from, n, skipId) {
    const N = from.nodes[n]; if (!N) return false;
    const ll = [N.lat, N.lng], ok = (sc, L) => !!(L && sc.nodes[L.a] && sc.nodes[L.b]), on = (L, p) => Net.project(s, L, p).d < SPLIT_M;
    let T = null;
    for (const F of Object.values(from.links)) {
      if (F.id === skipId || (F.a !== n && F.b !== n) || !ok(from, F)) continue;
      const cands = ok(s, s.links[F.id]) ? [s.links[F.id]] : Object.values(s.links).filter(M => ok(s, M) && M.cls === F.cls && DataHub.norm(M.name) === DataHub.norm(F.name));
      T = cands.find(M => on(M, ll) && on(M, midPoint(from, F))) || null;
      if (T) break;
    }
    if (!T) return false;
    const nm = DataHub.norm(T.name), at = k => [from.nodes[k].lat, from.nodes[k].lng];
    const pieces = Object.values(from.links).filter(F => F.id !== skipId && ok(from, F) && F.cls === T.cls && DataHub.norm(F.name) === nm
      && (F.id === T.id || !s.links[F.id]) && on(T, at(F.a)) && on(T, at(F.b)) && on(T, midPoint(from, F)));
    const total = pieces.reduce((t, F) => t + Net.len(from, F), 0), whole = Net.len(s, T);
    // the pieces must cover the road once, end to end; otherwise leave it alone
    if (pieces.length < 2 || Math.abs(total - whole) > Math.max(5, 0.02 * whole)) return false;
    const { id: _i, a: _a, b: _b, pts: _p, ...props } = T;
    for (const F of pieces) {
      for (const k of [F.a, F.b]) if (!s.nodes[k]) s.nodes[k] = structuredClone(from.nodes[k]);
      const c = { ...structuredClone(props), id: F.id, a: F.a, b: F.b, pts: structuredClone(F.pts || []) };
      if (alongOf(s, T, at(F.a)) > alongOf(s, T, at(F.b))) [c.lanesAB, c.lanesBA] = [c.lanesBA, c.lanesAB];   // runs the other way along T
      s.links[F.id] = c;
    }
    if (!pieces.some(F => F.id === T.id)) delete s.links[T.id];
    return true;
  }
  /* Copy one object of `from` into scenario s (no undo step; the caller takes it) → 'copied' | 'kept' | 'skipped',
     plus loose: true when a road end could not be joined to the road it meets in `from` */
  function copyInto(s, from, type, id, how = copyHow(type, objOf(from, type, id), from, s)) {
    const src = objOf(from, type, id); if (!src || how === 'skip') return { did: 'skipped', loose: false };
    let loose = false;
    if (type === 'link' && how === 'settings') {
      const T = s.links[id], c = structuredClone(src);
      for (const k of GEOM) { delete c[k]; if (T[k] !== undefined) c[k] = T[k]; }
      s.links[id] = { ...c, id };
      stale(s);
      return { did: 'kept', loose };
    }
    if (type === 'link') {
      for (const n of [src.a, src.b]) if (!s.nodes[n] && from.nodes[n] && !copySplitAt(s, from, n, id)) {
        s.nodes[n] = structuredClone(from.nodes[n]);
        const N = s.nodes[n];
        if (Object.values(s.links).some(M => M.a !== n && M.b !== n && s.nodes[M.a] && s.nodes[M.b] && Net.project(s, M, [N.lat, N.lng]).d < SPLIT_M)) loose = true;
      }
      s.links[id] = structuredClone(src);
    } else if (type === 'node') s.nodes[id] = structuredClone(src);
    else { s.zones ||= {}; if (src.node && !s.nodes[src.node] && from.nodes[src.node]) s.nodes[src.node] = structuredClone(from.nodes[src.node]); s.zones[id] = structuredClone(src); }
    stale(s);
    return { did: 'copied', loose };
  }
  /* Copy one object (and the junctions a road needs) into other scenarios → { scenarios, kept, skipped, loose } (names).
     Where a road is split in one scenario and whole in the other, only its settings are copied (kept: its alignment
     there stays), and a split piece is not added on top of the whole road (skipped). A new road that joins a road
     part-way along it splits that road in the target too; loose lists scenarios where it could not be joined. */
  function copyTo(type, id, scIds, from = Store.sc) {
    const src = objOf(from, type, id); if (!src) return { scenarios: [], kept: [], skipped: [], loose: [] };
    const targets = (scIds || []).map(x => Store.P.scenarios.find(s => s.id === x)).filter(s => s && s !== from);
    const names = [], kept = [], skipped = [], loose = [];
    const plan = targets.map(s => ({ s, how: copyHow(type, src, from, s) }));
    for (const x of plan.filter(x => x.how !== 'skip').reverse()) snapshotOf(x.s, `copying ${NOUN[type] || type} from ${from.name}`);
    for (const { s, how } of plan) {
      const r = copyInto(s, from, type, id, how);
      if (r.did === 'skipped') { skipped.push(s.name); continue; }
      if (r.did === 'kept') kept.push(s.name);
      if (r.loose) loose.push(s.name);
      names.push(s.name);
    }
    if (names.length) quiet(() => Store.emit(type === 'zone' ? 'demand' : 'net'));
    return { scenarios: names, kept, skipped, loose };
  }

  /* 'Edits apply to' for one object at a time (scenario-consistency a). After an edit to the active scenario (an
     inspector field, a moved junction, a new road, zone or gateway) the same change goes to the scenarios ticked under
     Edits apply to: changed fields are set on the same objects there (and on the pieces of a road split there), and new
     objects are copied in (a road joined part-way along another splits it there too). Deleting stays in this scenario.
     It works from the undo step the edit took, so it sees every edit with an undo step, from any panel. Each scenario
     gets its own undo step, and the first Ctrl+Z still undoes the one on screen. Bulk changes are not copied. */
  const MIRROR_MAX = 40;   // objects; more is an import or a set-up, not an edit
  const plainObj = v => !!v && typeof v === 'object' && !Array.isArray(v);
  // Paths that differ between a and b: top-level keys, one level into plain objects such as lu and gw
  function changedPaths(a, b) {
    const out = [], same = (x, y) => JSON.stringify(x) === JSON.stringify(y);
    for (const k of new Set([...Object.keys(a || {}), ...Object.keys(b || {})])) {
      if (k === 'id') continue;
      const x = a ? a[k] : undefined, y = b ? b[k] : undefined;
      if (same(x, y)) continue;
      if (plainObj(x) && plainObj(y)) { for (const j of new Set([...Object.keys(x), ...Object.keys(y)])) if (!same(x[j], y[j])) out.push(k + '.' + j); }
      else out.push(k);
    }
    return out;
  }
  const setOrDrop = (o, p, v) => {
    if (v !== undefined) return Kit.setPath(o, p, structuredClone(v));
    const i = p.lastIndexOf('.'), parent = i < 0 ? o : Kit.getPath(o, p.slice(0, i));
    if (parent && typeof parent === 'object') delete parent[i < 0 ? p : p.slice(i + 1)];
  };
  // What an edit did to scenario sc, against the copy its undo step holds
  // → { add: [[type, id]], change: [[type, id, paths]], removed: number of roads, zones and gateways it deleted }
  function editsSince(before, sc) {
    const add = [], change = [];
    let removed = 0;
    for (const [type, key] of [['node', 'nodes'], ['link', 'links'], ['zone', 'zones']]) {
      const A = before[key] || {}, B = sc[key] || {};
      for (const [id, o] of Object.entries(B)) {
        if (!A[id]) add.push([type, id]);
        else { const p = changedPaths(A[id], o); if (p.length) change.push([type, id, p]); }
      }
      if (type !== 'node') for (const id of Object.keys(A)) if (!B[id]) removed++;
    }
    return { add, change, removed };
  }
  function mirrorEdit() {
    const P = Store.P, top = Store.undoStack[Store.undoStack.length - 1];
    if (!P || !top || mirrorSeen.has(top)) return null;
    mirrorSeen.add(top);
    const sc = Store.sc;
    if (mirroring || setupDepth || !scope.size || top.sid !== sc.id || typeof top.data !== 'string') return null;
    const targets = [...scope].map(id => P.scenarios.find(s => s.id === id)).filter(s => s && s !== sc);
    if (!targets.length) return null;
    let before; try { before = JSON.parse(top.data); } catch { return null; }
    const { add, change, removed } = editsSince(before, sc), n = add.length + change.length;
    // a delete (or a merge) also changes the junctions around it; none of that goes to the other scenarios
    if (removed) return { scenarios: [], missing: 0, removed };
    if (!n) return null;
    if (n > MIRROR_MAX) return { scenarios: [], missing: 0, tooMany: n };
    // per scenario: the field changes it can take, and the new objects it does not have yet
    const plans = targets.map(s => {
      const sets = [], adds = [];
      let missing = 0;
      for (const [type, id, paths] of change) {
        if (type === 'link') {
          // a road whose ends moved was split, or re-joined, here: the new road carries the split; never copy ends
          const geo = paths.some(p => p === 'a' || p === 'b'), ps = paths.filter(p => !GEOM.includes(p) || (p === 'pts' && !geo));
          if (!ps.length) continue;
          const ids = linksIn(s, before, [id]).ids;
          if (!ids.length) { missing++; continue; }
          for (const lid of ids) {
            const T = s.links[lid], whole = T.a === sc.links[id].a && T.b === sc.links[id].b;
            const use = ps.filter(p => p !== 'pts' || (lid === id && whole));
            if (use.some(p => JSON.stringify(Kit.getPath(T, p)) !== JSON.stringify(Kit.getPath(sc.links[id], p)))) sets.push([T, use, sc.links[id]]);
          }
        } else {
          const T = type === 'node' ? s.nodes[id] : (s.zones || {})[id];
          if (!T) { missing++; continue; }
          if (paths.some(p => JSON.stringify(Kit.getPath(T, p)) !== JSON.stringify(Kit.getPath(objOf(sc, type, id), p)))) sets.push([T, paths, objOf(sc, type, id)]);
        }
      }
      for (const [type, id] of add) {
        if (type === 'node') continue;   // junctions come with the roads and gateways that use them
        if (objOf(s, type, id)) continue;
        const how = copyHow(type, objOf(sc, type, id), sc, s);
        if (how !== 'skip') adds.push([type, id, how]);
      }
      return { s, sets, adds, missing };
    });
    const touch = plans.filter(x => x.sets.length || x.adds.length);
    const label = `${top.label || 'edit'} (from ${sc.name})`;
    for (const x of touch.slice().reverse()) snapshotOf(x.s, label);
    const loose = [];
    for (const { s, sets, adds } of touch) {
      for (const [T, paths, src] of sets) for (const p of paths) setOrDrop(T, p, Kit.getPath(src, p));
      // roads first, so a gateway or zone finds its junction
      for (const [type, id, how] of adds.sort((a, b) => (a[0] === 'link' ? 0 : 1) - (b[0] === 'link' ? 0 : 1))) if (!objOf(s, type, id) && copyInto(s, sc, type, id, how).loose) loose.push(s.name);
      stale(s);
    }
    // the edit on screen stays the newest undo step, so Ctrl+Z undoes it first
    if (touch.length) { const i = Store.undoStack.indexOf(top); if (i >= 0) { Store.undoStack.splice(i, 1); Store.undoStack.push(top); } }
    const missing = plans.reduce((t, x) => t + x.missing, 0), out = { scenarios: touch.map(x => x.s.name), missing, loose: [...new Set(loose)] };
    // other scenarios changed: move the cache key on now, and redraw what shows them once this event is over
    if (touch.length) { Store.ver++; if (hasUI()) setTimeout(() => quiet(() => Store.emit('net')), 0); }
    return out;
  }
  function mirrorToast(r) {
    if (!r || !hasUI()) return;
    if (r.removed) return Kit.toast(`Deleted in ${Store.sc.name} only: Edits apply to does not delete in other scenarios. Switch to each of them ([ and ]) and delete it there too if it should go.`);
    if (r.tooMany) return Kit.toast(`This change touches ${U.fmt.int(r.tooMany)} objects, so it stays in ${Store.sc.name}: Edits apply to copies single edits only. Use Copy to, or the set-up's 'every scenario' option.`, 'warn');
    if (!r.scenarios.length && !r.missing) return;
    const sc = Store.sc.name;
    Kit.toast(`${r.scenarios.length ? `Changed in ${andList([sc, ...r.scenarios])}.` : `Changed in ${sc} only.`}${r.missing ? ` ${plural(r.missing, 'edited object')} ${r.missing === 1 ? 'is' : 'are'} not in every ticked scenario.` : ''}${(r.loose || []).length ? ` In ${andList(r.loose)} the new road could not be joined to the road it meets here: join it with the road tool.` : ''}${r.scenarios.length ? ' Ctrl+Z undoes one scenario at a time.' : ''}`, r.missing || (r.loose || []).length ? 'warn' : '');
  }
  function resetField(type, id, key, sc = Store.sc, base = Store.baseSc) {
    const t = type === 'zone' ? (sc.zones[id] && sc.zones[id].kind === 'gateway' ? 'gateway' : 'zone') : type;
    const o = objOf(sc, type, id), b = objOf(base, type, id); if (!o || !b) return false;
    const f = fieldsFor(t, b, o).find(x => x.key === key); if (!f) return false;
    if (t === 'link' && key === 'geom' && isSplit(o, sc, b, base))
      throw new Error(`This road is split where a new road joins it in ${sc.name}, so its alignment is not reset: that would lay the whole road over its other piece. Its other settings can be reset.`);
    snap(`reset ${f.label.toLowerCase()} to base`);
    if (t === 'link' && key === 'geom') for (const n of [b.a, b.b]) if (!sc.nodes[n] && base.nodes[n]) sc.nodes[n] = structuredClone(base.nodes[n]);
    for (const p of f.paths || [f.key]) { const v = Kit.getPath(b, p); if (v === undefined && !p.includes('.')) delete o[p]; else Kit.setPath(o, p, v === undefined ? null : structuredClone(v)); }
    if (t === 'gateway' && key.startsWith('gw.')) o.gwSrc = b.gwSrc ?? null;
    quiet(() => Store.edited(type === 'zone' ? 'demand' : 'net'));
    return true;
  }

  /* What each option changes compared with the base: roads, junction controls, zones and gateways.
     Roads split where a new road joins them are recognised and left out. Cached on Store.ver. */
  const diffCache = { ver: null, map: new Map() };   // one entry per base|option pair, cleared when Store.ver moves on
  const CTRL_SHORT = { none: 'uncontrolled', priority: 'give way', stop: 'stop', roundabout: 'roundabout', signals: 'traffic signals' };
  const cap1 = t => t.charAt(0).toUpperCase() + t.slice(1);
  function onBase(L, s, base) {
    // a piece of an existing road (same name and class) that only exists because a new road joined it
    const nm = DataHub.norm(L.name), g = Net.geom(s, L), mid = U.along(g, U.cumLen(g), U.polyLen(g) / 2).p;
    for (const B of Object.values(base.links)) {
      if (B.cls !== L.cls || DataHub.norm(B.name) !== nm || !base.nodes[B.a] || !base.nodes[B.b]) continue;
      if (Net.project(base, B, mid).d < 5) return true;
    }
    return false;
  }
  function diff(base, opt) {
    if (diffCache.ver !== Store.ver) { diffCache.ver = Store.ver; diffCache.map.clear(); }
    const key = `${base.id}|${opt.id}`;
    const hit = diffCache.map.get(key);
    if (hit && hit.of[0] === base && hit.of[1] === opt && hit.P === Store.P) return hit.v;   // the same scenario objects (a report can build another project)
    const rows = [], E = ((Store.P && S()) || {}).econ || {}, laneKm = E.laneKm || {}, nodeCost = E.node || {};
    let split = 0;
    const linkLen = (s, L) => (s.nodes[L.a] && s.nodes[L.b] ? Net.len(s, L) : 0);
    const nm = L => L.name || `Unnamed ${clsLabel(L)} road`;
    for (const [id, L] of Object.entries(opt.links)) {
      if (!opt.nodes[L.a] || !opt.nodes[L.b]) continue;
      const B = base.links[id], len = linkLen(opt, L);
      if (!B) {
        if (L.status !== 'proposed' && onBase(L, opt, base)) { split++; continue; }
        const cost = L.status === 'proposed' ? len / 1000 * ((L.lanesAB | 0) + (L.lanesBA | 0)) * (laneKm[L.cls] ?? 3) : null;
        rows.push({ kind: 'Road', change: 'added', type: 'link', id, name: nm(L), text: `+ ${nm(L)} · ${clsLabel(L)} · ${U.fmt.len(len)} · ${(STATUS[L.status] || L.status).toLowerCase()}`, len, cost });
        continue;
      }
      const { rows: f, split: cut } = linkRows(B, L, base, opt);
      if (cut && !f.length) { split++; continue; }
      if (f.length) rows.push({ kind: 'Road', change: 'changed', type: 'link', id, name: nm(L), text: `${nm(L)}: ${f.map(r => `${r.label.toLowerCase()} ${r.base} → ${r.here}`).join('; ')}`, len, cost: null });
    }
    for (const [id, B] of Object.entries(base.links)) if (!opt.links[id] && base.nodes[B.a] && base.nodes[B.b])
      rows.push({ kind: 'Road', change: 'removed', type: 'link', id, name: nm(B), text: `− ${nm(B)} · ${clsLabel(B)} · ${U.fmt.len(linkLen(base, B))} (removed)`, len: linkLen(base, B), cost: null, inBase: true });
    for (const [id, N] of Object.entries(opt.nodes)) {
      const B = base.nodes[id], c = N.control || 'none';
      if (!B && (c === 'signals' || c === 'roundabout')) rows.push({ kind: 'Junction', change: 'added', type: 'node', id, name: Kit.junctionName(id, opt), text: `${cap1(CTRL_SHORT[c])} at ${Kit.junctionName(id, opt)} (new junction)`, cost: N.upgrade ? nodeCost[c] ?? null : null });
      else if (B && (B.control || 'none') !== c) rows.push({ kind: 'Junction', change: 'changed', type: 'node', id, name: Kit.junctionName(id, opt), text: `${cap1(CTRL_SHORT[c])} at ${Kit.junctionName(id, opt)} (was ${CTRL_SHORT[B.control || 'none']})`, cost: N.upgrade ? nodeCost[c] ?? null : null });
      else if (B) { const f = compareObj('node', B, N, base, opt).filter(r => r.key !== 'control' && r.key !== 'position'); if (f.length) rows.push({ kind: 'Junction', change: 'changed', type: 'node', id, name: Kit.junctionName(id, opt), text: `${Kit.junctionName(id, opt)}: ${f.map(r => `${r.label.toLowerCase()} ${r.base} → ${r.here}`).join('; ')}`, cost: null }); }
    }
    for (const [id, Z] of Object.entries(opt.zones || {})) {
      const B = (base.zones || {})[id], t = Z.kind === 'gateway' ? 'gateway' : 'zone', kind = t === 'gateway' ? 'Gateway' : 'Zone';
      if (!B) { const lu = Object.entries(Z.lu || {}).filter(([, v]) => +v > 0).map(([k, v]) => `${U.fmt.int(+v)} ${(LANDUSES[k] || { unit: k }).unit} ${(LANDUSES[k] || { label: k }).label.toLowerCase()}`); rows.push({ kind, change: 'added', type: 'zone', id, name: Z.name, text: `+ ${Z.name}${lu.length ? ' · ' + lu.join(', ') : ''}`, cost: null }); continue; }
      const f = compareObj(t, B, Z, base, opt);
      if (f.length) rows.push({ kind, change: 'changed', type: 'zone', id, name: Z.name, text: `${Z.name}: ${f.map(r => `${r.label.toLowerCase()} ${r.base} → ${r.here}`).join('; ')}`, cost: null });
    }
    for (const [id, B] of Object.entries(base.zones || {})) if (!(opt.zones || {})[id]) rows.push({ kind: B.kind === 'gateway' ? 'Gateway' : 'Zone', change: 'removed', type: 'zone', id, name: B.name, text: `− ${B.name} (removed)`, cost: null, inBase: true });
    const v = { rows, split, cost: rows.reduce((t, r) => t + (r.cost || 0), 0), base: base.id, opt: opt.id };
    diffCache.map.set(key, { v, of: [base, opt], P: Store.P });
    return v;
  }
  function changesTab() {
    if (!Store.P || !Array.isArray(Store.P.scenarios)) return '<p class="note">Open or start a project to compare its options.</p>';
    const P = Store.P, base = Store.baseSc, others = P.scenarios.filter(s => s !== base);
    if (!others.length) return '<p class="note">Add an option (+ Option) to see what it changes.</p>';
    const blocks = others.map(s => {
      const d = diff(base, s);
      const rows = d.rows.map(r => `<tr class="click" data-onb-go="${esc(r.inBase ? base.id : s.id)}|${r.type}|${esc(r.id)}" tabindex="0"><td>${esc(r.kind)}</td><td style="white-space:normal">${esc(r.text)}</td><td class="n">${r.cost != null ? U.fmt.money(r.cost) : '–'}</td></tr>`).join('');
      return `<div class="lg-title" style="margin-top:12px">${esc(s.name)} compared with ${esc(base.name)} · ${plural(d.rows.length, 'change')}${d.cost ? ` · indicative cost ${U.fmt.money(d.cost)}` : ''}</div>
        ${d.rows.length ? `<div class="tbl-wrap"><table class="t compact"><tr><th>What</th><th>Change</th><th class="n">Indicative cost</th></tr>${rows}</table></div>` : `<p class="note">${esc(s.name)} is the same as ${esc(base.name)}.</p>`}
        ${d.split ? `<p class="note">${plural(d.split, 'existing road segment')} ${d.split === 1 ? 'was' : 'were'} split where new roads join; ${d.split === 1 ? 'it is' : 'they are'} not listed.</p>` : ''}`;
    }).join('');
    return `<div class="bar-h"><h3>What each option changes</h3><span class="note">Compared with the base scenario, ${esc(base.name)}. Click a row to show it.</span><button class="btn small" data-onb="changes-csv">Export CSV</button></div>${blocks}
      <p class="note">Costs are indicative: proposed roads at the lane-km rates in Settings and upgraded junctions at the junction rates. Not a cost estimate.</p>`;
  }
  function changesCSV(P = Store.P) {
    const base = P.scenarios.find(s => s.id === P.base) || P.scenarios[0], rows = [['scenario', 'compared_with', 'what', 'change', 'name', 'details', 'length_m', 'indicative_cost_NZD_m']];
    for (const s of P.scenarios) if (s !== base) for (const r of diff(base, s).rows) rows.push([s.name, base.name, r.kind, r.change, r.name, r.text, r.len != null ? Math.round(r.len) : '', r.cost != null ? U.r3(r.cost) : '']);
    return U.csv(rows);
  }
  /* The same list in the ITA report, chapter 3 (description of the proposal): the proposal and the mitigation
     scenario the report uses, each against its without-development (or base) scenario. ctx from the Report framework. */
  function reportChanges(P, c = {}) {
    if (!P || !Array.isArray(P.scenarios) || !P.scenarios.length) return '';
    c = c || {};
    const sel = c.sel || null, base = (sel && (sel.without || sel.base)) || P.scenarios.find(s => s.id === P.base) || P.scenarios[0];
    const list = (sel ? [sel.with, sel.mitigation] : P.scenarios).filter((s, i, a) => s && s !== base && a.indexOf(s) === i);
    if (!base || !list.length) return '';
    const rows = [];
    for (const s of list) {
      const d = diff(base, s);
      for (const r of d.rows) rows.push([esc(s.name), esc(r.kind), esc(r.text), r.cost != null ? U.fmt.money(r.cost) : '–']);
      if (!d.rows.length) rows.push([esc(s.name), '–', `No changes to roads, junctions, zones or gateways against ${esc(base.name)}`, '–']);
    }
    const body = { head: ['Scenario', 'What', 'Change', { label: 'Indicative cost', n: true }], rows };
    const note = 'Indicative costs: proposed roads at the lane-km rates in Settings and new or upgraded junctions at the junction rates; not a cost estimate. Existing roads split where a new road joins them are not listed.';
    return typeof c.table === 'function'
      ? c.table(`Changes in each option against ${esc(base.name)}`, body, { note, source: `Spare Construct's comparison of this project's scenarios, object by object (${esc(base.name)} and ${esc(andList(list.map(s => s.name)))}).` })
      : Kit.table(body) + `<p class="note">${note}</p>`;
  }

  /* ---------- zones: duplicate, copy land use, placement card (zone-gateway-ux a) ---------- */
  function duplicateZone(sc, id) {
    const z = sc.zones[id];
    if (!z) throw new Error('Select a zone to duplicate it.');
    if (z.kind === 'gateway') throw new Error('Gateways cannot be duplicated. Add another with the gateway tool (G) at a road end.');
    snap(`duplicating ${z.name}`);
    const nid = U.uid('z'), c = structuredClone(z);
    Object.assign(c, { id: nid, name: `${z.name} (copy)`, lng: U.r7(z.lng + 60 / (111320 * Math.cos(z.lat * Math.PI / 180))) });
    delete c.src;
    sc.zones[nid] = c;
    View.sel = { type: 'zone', id: nid };
    Store.edited('demand');
    return nid;
  }
  function copyLanduse(sc, toId, fromId) {
    const a = sc.zones[toId], b = sc.zones[fromId];
    if (!a || !b || a === b) return false;
    snap(`copying land use to ${a.name}`);
    a.lu = structuredClone(b.lu || {});
    Store.edited('demand');
    return true;
  }
  function zoneSection(sc, sel) {
    const z = sc.zones[sel.id]; if (!z) return '';
    const others = Object.values(sc.zones).filter(o => o.kind !== 'gateway' && o.id !== z.id && Object.values(o.lu || {}).some(v => +v > 0)).sort((a, b) => String(a.name).localeCompare(String(b.name)));
    return Kit.sec('', `<div class="actions"><button class="btn small" data-onb="dup-zone" title="Duplicate this zone (Ctrl+D)">Duplicate <kbd class="k">Ctrl+D</kbd></button></div>
      ${others.length ? Kit.fld('Copy land use from', `<select data-onb-copylu aria-label="Copy land use from another zone"><option value="">Choose a zone…</option>${others.map(o => `<option value="${esc(o.id)}">${esc(o.name)}</option>`).join('')}</select>`) : ''}`);
  }
  const QUICK_LU = ['residential', 'apartment', 'retail', 'office', 'industrial', 'school'];
  // The land-use lane's zone card (LandusePlus.placeZone) is used when it is loaded, so there is only one
  const landuseCard = () => typeof LandusePlus !== 'undefined' && typeof LandusePlus.placeZone === 'function';
  // A new zone with the land use the user chose; refuses without a type and a positive amount
  function placeZone(sc, ll, key, qty) {
    const n = parseNum(qty);
    if (!LANDUSES[key]) throw new Error('Choose what is planned here first.');
    if (!(n > 0)) throw new Error(`Enter how many ${LANDUSES[key].unit} are planned.`);
    snap('new zone');
    const id = U.uid('z'), k = Object.values(sc.zones).filter(z => z.kind !== 'gateway').length + 1;
    sc.zones[id] = { id, kind: 'zone', name: `Zone ${k}`, lat: U.r7(ll[0]), lng: U.r7(ll[1]), lu: { [key]: n }, connectors: null,
      luSrc: { [key]: { source: 'entered in Spare Construct', basis: 'user', date: new Date().toISOString().slice(0, 10), q: n } } };
    View.sel = { type: 'zone', id };
    Store.edited('demand');
    return id;
  }
  function placementHtml(p) {
    const lu = p.key && LANDUSES[p.key], n = parseNum(p.raw), bad = p.raw != null && p.raw !== '' && !(n > 0);
    return `<div class="lg-title" id="onbPlaceT">What's planned here?</div>
      <div class="chips" role="group" aria-labelledby="onbPlaceT">${QUICK_LU.map(k => `<button type="button" class="chip ${p.key === k ? 'on' : ''}" data-onb-lu="${k}" aria-pressed="${p.key === k}">${esc(LANDUSES[k].label)}</button>`).join('')}
        <select data-onb-lu-more aria-label="More land uses" class="chip"><option value="">+ more</option>${Object.entries(LANDUSES).filter(([k]) => !QUICK_LU.includes(k)).map(([k, l]) => `<option value="${k}" ${p.key === k ? 'selected' : ''}>${esc(l.label)}</option>`).join('')}</select></div>
      <div style="margin-top:8px">${lu ? Kit.fld(esc(lu.label), `<input id="onbPlaceQty" data-onb-qty inputmode="decimal" autocomplete="off" value="${esc(p.raw ?? '')}" placeholder="${lu.per === 100 ? 'e.g. 5,000' : 'e.g. 200'}" aria-invalid="${bad}" aria-describedby="onbPlaceErr">`, esc(lu.unit)) : '<p class="note" style="margin:0">Choose what is planned to set its size.</p>'}</div>
      <p class="note warn" id="onbPlaceErr" ${bad ? '' : 'hidden'} style="margin:4px 0 0">Enter a number, such as 200 or 5,000.</p>
      <div class="actions" style="margin-top:8px"><button type="button" class="btn small primary" data-onb="place-ok" ${lu && n > 0 ? '' : 'disabled'}>Place zone</button><button type="button" class="btn small" data-onb="place-cancel">Cancel (Esc)</button></div>`;
  }

  /* ---------- gateways: where the flows came from (zone-gateway-ux b) ---------- */
  function gatewayFromCounts(sc, zid) {
    const z = sc.zones[zid];
    if (!z || z.kind !== 'gateway') return { ok: false, why: 'Select a gateway first.' };
    if (!DataHub.D().counts.length) return { ok: false, why: 'No counts are loaded. Load them in the Traffic data tab first.' };
    snap(`gateway flows for ${z.name}`);
    const n = DataHub.gatewaysFromCounts({ ...sc, zones: { [zid]: z } });
    if (!n) { Store.undoStack.pop(); return { ok: false, why: 'No usable count within 3 km on the same road. Load more counts, or type in surveyed flows.' }; }
    Store.edited('demand');
    return { ok: true, src: z.gwSrc };
  }
  function gatewaySection(sc, sel) {
    // the land-use lane shows the same count source and button; don't show it twice
    if (Registry.get('inspectorSections', 'landuse-gateway')) return '';
    const z = sc.zones[sel.id]; if (!z) return '';
    const has = DataHub.D().counts.length > 0;
    const badge = z.gwSrc ? '<span class="pill" style="color:var(--ok)">From counts</span>' : '<span class="pill proposed">Estimated</span>';
    return Kit.sec('Flow source', `<p class="note" style="margin:0">${badge} ${z.gwSrc ? esc(z.gwSrc) : 'From the road class and lanes; no count used.'}</p>
      <div class="actions" style="margin-top:6px"><button class="btn small" data-onb="gw-counts" ${has ? '' : 'disabled title="Load counts in the Traffic data tab first"'}>Use nearest counts</button></div>`);
  }

  /* ---------- command palette ---------- */
  const GROUPS = ['Commands', 'Roads', 'Junctions and sites', 'Zones and gateways', 'Count sites', 'Places'];
  const escRe = s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  // 3 = starts with the query, 2 = a word starts with it, 1 = contains it (or every word of it), 0 = no match
  function score(text, q) {
    const t = String(text || '').toLowerCase();
    if (!q) return 1;
    if (t.startsWith(q)) return 3;
    if (new RegExp('(^|[\\s/,·(\\-–:])' + escRe(q)).test(t)) return 2;
    if (t.includes(q)) return 1;
    const w = q.split(/\s+/).filter(Boolean);
    return w.length > 1 && w.every(x => t.includes(x)) ? 1 : 0;
  }
  function paletteItems(q, { sc = Store.sc, places = [], max = 30 } = {}) {
    const query = String(q || '').trim().toLowerCase().replace(/\s+/g, ' '), items = [];
    const push = (group, label, sub, run, o = {}) => { const s = Math.max(score(label, query), o.also ? score(o.also, query) : 0); if (s) items.push({ group, label, sub, run, keys: o.keys || '', score: s }); };
    // commands, then Menu items and exports not already listed (keeping the one that shows a shortcut)
    const cmds = new Map();
    const addCmd = (label, sub, run, keys) => { const k = String(label).toLowerCase(), had = cmds.get(k); if (!had || (!had.keys && keys)) cmds.set(k, { label, sub, run, keys }); };
    for (const c of Registry.list('commands')) { if (c.enabled && !Registry.call(c, 'enabled')) continue; addCmd(c.label, c.group || '', () => Registry.call(c, 'run'), c.keys); }
    for (const m of Registry.list('menu')) { if (m.enabled && !Registry.call(m, 'enabled')) continue; const label = typeof m.label === 'function' ? Registry.call(m, 'label') : m.label; if (label) addCmd(label, 'Menu', () => Registry.call(m, 'run'), m.keys); }
    for (const d of Registry.list('exports')) addCmd('Export: ' + d.label, 'Export', () => (hasUI() ? UI.exportFile(d.id) : null), '');
    for (const c of cmds.values()) push('Commands', c.label, c.sub, c.run, { keys: c.keys });
    if (query) {
      // roads, by street name
      const streets = new Map();
      for (const L of Object.values(sc.links)) { if (!L.name || !sc.nodes[L.a] || !sc.nodes[L.b]) continue; const k = DataHub.norm(L.name); const e = streets.get(k); if (e) { e.ids.push(L.id); e.len += Net.len(sc, L); } else streets.set(k, { name: L.name, ids: [L.id], len: Net.len(sc, L), cls: L.cls }); }
      for (const st of streets.values()) push('Roads', st.name, `${plural(st.ids.length, 'segment')} · ${U.fmt.len(st.len)} · ${(CLASSES[st.cls] || CLASSES.local).label.toLowerCase()}`, () => { select(makeSel(st.ids, [])); flyToPoints(pointsOf(st.ids.map(id => 'link:' + id), sc)); });
      // junction sites, and signals and roundabouts
      const inSite = new Set();
      for (const st of Object.values(sc.sites || {})) {
        (st.nodes || []).forEach(id => inSite.add(id));
        const id = (st.nodes || []).find(n => sc.nodes[n]); if (!id) continue;
        let nm = st.name; try { nm = (typeof Sites !== 'undefined' && Sites.name(sc, st)) || nm; } catch { }
        push('Junctions and sites', nm || Kit.junctionName(id, sc), `Junction site · ${(st.nodes || []).length} nodes`, () => focusRefs([['node', id]]), { also: st.atSiteNo ? 'AT ' + st.atSiteNo : '' });
      }
      for (const n of Object.values(sc.nodes)) if ((n.control === 'signals' || n.control === 'roundabout') && !inSite.has(n.id)) push('Junctions and sites', Kit.junctionName(n.id, sc), CONTROLS[n.control], () => focusRefs([['node', n.id]]));
      for (const z of Object.values(sc.zones || {})) push('Zones and gateways', z.name || 'Unnamed zone', z.kind === 'gateway' ? 'Gateway' : 'Zone', () => focusRefs([['zone', z.id]]));
      for (const c of DataHub.D().counts) { const src = DataHub.SOURCES[c.src] || { short: c.src }; push('Count sites', c.name, `${src.short}${c.ref ? ' ' + c.ref : ''}${c.adt ? ` · ${U.fmt.int(c.adt)} veh/day` : ''}`, () => { if (hasUI()) { View.show.counts = true; UI.flyTo('count', c.id); } }, { also: c.ref }); }
      for (const r of places) items.push({ group: 'Places', label: r.name, sub: r.src === 'nominatim' ? 'Place (Nominatim)' : 'Place (Photon)', run: () => pickPlace(r), keys: '', score: 0.5 });
    }
    const gi = g => GROUPS.indexOf(g);
    items.sort((a, b) => b.score - a.score || gi(a.group) - gi(b.group) || a.label.localeCompare(b.label));
    const top = items.slice(0, max), first = new Map();
    top.forEach((it, i) => { if (!first.has(it.group)) first.set(it.group, i); });
    // keep each group together, groups in the order of their best match
    return top.slice().sort((a, b) => first.get(a.group) - first.get(b.group) || top.indexOf(a) - top.indexOf(b));
  }
  function paletteListHtml(items, active, q, prefix = 'onbPalO') {
    if (!items.length) return `<p class="note" style="padding:8px 10px;margin:0">${q ? `Nothing matches “${esc(q)}”. Try a road name, a count site number such as 01N00478, or a command.` : 'Type to search.'}</p>`;
    let html = '', g = null;
    items.forEach((it, i) => {
      if (it.group !== g) { if (g !== null) html += '</div>'; g = it.group; html += `<div role="group" aria-label="${esc(g)}"><div class="onb-grp" aria-hidden="true">${esc(g)}</div>`; }
      html += `<div role="option" id="${prefix}${i}" data-i="${i}" class="onb-opt${i === active ? ' on' : ''}" aria-selected="${i === active}"><span class="onb-opt-l">${esc(it.label)}</span>${it.sub ? `<span class="note">${esc(it.sub)}</span>` : ''}${it.keys ? `<kbd class="k">${esc(it.keys)}</kbd>` : ''}</div>`;
    });
    return html + '</div>';
  }
  function focusRefs(refs) {
    const sc = Store.sc, [t, id] = refs[0];
    select(t === 'zone' ? { type: 'zone', id } : t === 'node' ? { type: 'node', id } : makeSel(refs.filter(r => r[0] === 'link').map(r => r[1]), []));
    flyToPoints(pointsOf(refs.map(([a, b]) => a + ':' + b), sc));
  }
  /* A combobox in a modal: input + listbox, arrows move, Enter chooses, Esc closes (the shell closes modals).
     items(q, places) → rows; enter(q) → Promise of places for Enter with nothing highlighted. */
  function comboModal({ title, placeholder, label, prefix, initial = '', items, enter, note, buttons = [['Close', null, '']], wantPlaces = true }) {
    const p = Kit.modal(esc(title), `<input id="${prefix}In" type="search" role="combobox" aria-expanded="true" aria-controls="${prefix}List" aria-autocomplete="list" aria-label="${esc(label)}" autocomplete="off" spellcheck="false" placeholder="${esc(placeholder)}" value="${esc(initial)}" style="width:100%">
      <div id="${prefix}List" class="onb-list" role="listbox" aria-label="${esc(label)}: results"></div><p class="note" id="${prefix}Msg" aria-live="polite">${note}</p>`, buttons, true);
    const input = U.$('#' + prefix + 'In'), list = U.$('#' + prefix + 'List'), msg = U.$('#' + prefix + 'Msg');
    if (!input || !list) return p;
    const st = { active: 0, rows: [], places: [] };
    const ta = typeahead({ near: mapCentre, onResults: r => { st.places = r; draw(); }, onError: e => { msg.textContent = e.message; } });
    const draw = (keep = false) => {
      st.rows = items(input.value, st.places);
      if (!keep) st.active = 0;
      st.active = Math.min(st.active, Math.max(0, st.rows.length - 1));
      list.innerHTML = paletteListHtml(st.rows, st.active, input.value.trim(), prefix + 'O');
      input.setAttribute('aria-activedescendant', st.rows.length ? prefix + 'O' + st.active : '');
      const on = list.querySelector('.onb-opt.on'); if (on && on.scrollIntoView) on.scrollIntoView({ block: 'nearest' });
    };
    const choose = it => { if (!it) return; ta.cancel(); Kit.closeModal(null); setTimeout(() => { try { it.run(); } catch (err) { console.error(err); Kit.toast('That did not work: ' + err.message, 'err'); } }, 0); };
    input.addEventListener('input', () => { st.places = []; draw(); if (wantPlaces) ta.input(input.value); });
    input.addEventListener('keydown', async e => {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); if (st.rows.length) { st.active = (st.active + (e.key === 'ArrowDown' ? 1 : st.rows.length - 1)) % st.rows.length; draw(true); } }
      else if (e.key === 'Home' && st.rows.length && !input.value) { e.preventDefault(); st.active = 0; draw(true); }
      else if (e.key === 'Enter') {
        e.preventDefault();
        if (st.rows.length && (st.rows[st.active].group !== 'Commands' || !enter || !input.value.trim())) return choose(st.rows[st.active]);
        if (!enter) return;
        msg.textContent = 'Searching…';
        try {
          const r = await enter(input.value);
          if (r.length) { msg.textContent = ''; return choose({ run: () => pickPlace(r[0], { area: true }) }); }
          msg.textContent = `No places found for “${input.value.trim()}”. Check the spelling, or try a nearby road or suburb.`;
        } catch (err) { msg.textContent = err.message; }
      }
    });
    list.addEventListener('mousedown', e => e.preventDefault());
    list.addEventListener('click', e => { const o = e.target.closest('[data-i]'); if (o) choose(st.rows[+o.dataset.i]); });
    draw();
    if (initial && wantPlaces) ta.input(initial);
    return p;
  }
  function openPalette(initial = '') {
    if (!hasUI() || Kit.modalOpen()) return;
    comboModal({ title: 'Go to or run…', prefix: 'onbPal', label: 'Search roads, junctions, zones, count sites, places and commands', placeholder: 'Road, junction, zone, count site (01N00478), place or command', initial,
      items: (q, places) => paletteItems(q, { places }), note: `↑ ↓ move · Enter chooses · Esc closes. Places: ${esc(SEARCH.attribution)}.` });
  }
  // 'Where is your study area?': search, then a pick drops the study-area box
  function openFinder() {
    if (!hasUI()) return;
    if (Kit.modalOpen()) Kit.closeModal(null);
    const ta = typeahead({ near: mapCentre });
    comboModal({ title: 'Where is your study area?', prefix: 'onbFind', label: 'Address, road or suburb', placeholder: 'Address, road or suburb, e.g. Waihoehoe Road, Drury',
      items: (q, places) => places.map(r => ({ group: 'Places', label: r.name, sub: r.type, run: () => pickPlace(r, { area: true }), keys: '' })),
      enter: q => ta.enter(q), note: `Type at least 3 letters, or press Enter to search. ${esc(SEARCH.attribution)}; Enter falls back to ${esc(SEARCH.fallback)}.`,
      buttons: [['Draw a box on the map instead', 'draw', ''], ['Close', null, '']] })
      .then(v => { if (v === 'draw') { UI.setTool('onboarding-area'); Kit.toast('Drag a box around your site, or click to drop a 1.5 km square.'); } });
  }

  /* ---------- the study-area box on the map, its card and the set-up run (browser only) ---------- */
  let rect = null, corners = [], card = null, cardOpen = false, setupRunning = false;
  const accent = () => (Render.theme && Render.theme.accent) || '#0d6a4d';
  function drawArea() {
    if (!hasUI()) return;
    const b = areaOf(), map = UI.map;
    if (!b) { if (rect) { map.removeLayer(rect); rect = null; } corners.forEach(m => map.removeLayer(m)); corners = []; renderCard(); return; }
    const col = accent();
    if (!rect) rect = L.rectangle(b, { color: col, weight: 2, dashArray: '6 4', fillColor: col, fillOpacity: 0.05, interactive: false }).addTo(map);
    else { rect.setBounds(b); rect.setStyle({ color: col, fillColor: col }); }
    const pos = bb => [[bb[0][0], bb[0][1]], [bb[0][0], bb[1][1]], [bb[1][0], bb[1][1]], [bb[1][0], bb[0][1]]];
    if (!corners.length) {
      const icon = L.divIcon({ className: 'onb-corner', iconSize: [14, 14], html: '<span style="display:block;width:14px;height:14px;border-radius:3px;background:var(--panel);border:2px solid var(--accent);box-shadow:var(--shadow);cursor:move"></span>' });
      corners = pos(b).map((ll, i) => {
        const m = L.marker(ll, { draggable: true, keyboard: true, icon, title: 'Drag to resize the study area', zIndexOffset: 1000 }).addTo(map);
        let opp = null;
        m.on('dragstart', () => { const bb = areaOf(); opp = pos(bb)[(i + 2) % 4]; });
        m.on('drag', ev => {
          const ll2 = ev.target.getLatLng(), nb = normBox([[ll2.lat, ll2.lng], opp]);
          Store.P.onboarding.area = nb;
          rect.setBounds(nb);
          pos(nb).forEach((p, k) => { if (k !== i) corners[k].setLatLng(p); });
          const ro = card && card.querySelector('.onb-readout'); if (ro) ro.textContent = readout(nb).text;
        });
        m.on('dragend', () => { setArea(Store.P.onboarding.area); cardOpen = true; renderCard(); });
        return m;
      });
    } else pos(b).forEach((p, k) => corners[k].setLatLng(p));
    renderCard();
  }
  function cardHtml(b) {
    const r = readout(b);
    return `<div class="lg-title" style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px">Study area<button class="btn small icon" data-onb="area-hide" aria-label="Hide this card (the box stays)" title="Hide this card">✕</button></div>
      <p class="note onb-readout" style="margin:0 0 6px">${esc(r.text)}</p>
      ${r.warn ? `<p class="note warn" style="margin:0 0 6px">${esc(r.warn)}</p>` : ''}
      <div style="display:grid;gap:4px">${Kit.fld('Width', `<input type="number" min="0.2" max="20" step="0.1" data-onb-size="w" value="${(r.w / 1000).toFixed(1)}" aria-label="Study area width">`, 'km')}${Kit.fld('Height', `<input type="number" min="0.2" max="20" step="0.1" data-onb-size="h" value="${(r.h / 1000).toFixed(1)}" aria-label="Study area height">`, 'km')}</div>
      <div class="actions" style="margin-top:8px"><button class="btn small primary" data-onb="setup" ${r.ok && !setupRunning ? '' : 'disabled'}>Set up this area…</button><button class="btn small" data-onb="area-fit">Zoom to</button><button class="btn small danger" data-onb="area-clear">Remove</button></div>
      <p class="note" style="margin:6px 0 0">Drag the corner squares, or set the size, to fit your site.</p>`;
  }
  function renderCard(stepsList) {
    if (!card) return;
    const b = areaOf();
    if (stepsList) { card.hidden = false; card.innerHTML = `<div class="lg-title">Setting up the study area</div>${stepsHtml(stepsList)}<p class="note">Use Cancel on the progress bar to stop the downloads.</p>`; return; }
    if (setupRunning) return;
    card.hidden = !b || !cardOpen || View.tool === 'road';
    if (!card.hidden) card.innerHTML = cardHtml(b);
  }
  // Whose search found the place (the services ask to be credited where their results are shown)
  const srcNote = r => (r && r.src === 'nominatim' ? ` ${SEARCH.fallback}.` : r && r.src === 'photon' ? ` ${SEARCH.attribution}.` : '');
  function pickPlace(r, { area = false } = {}) {
    if (!r || !hasUI()) return;
    if (area || !Object.keys(Store.sc.links).length) {
      UI.map.setView([r.lat, r.lng], 15);
      setArea(box(r.lat, r.lng));
      cardOpen = true; renderCard();
      Kit.toast(`${r.name}: drag the corners to fit your study area, then press Set up this area.${srcNote(r)}`);
    } else {
      UI.map.fitBounds(L.latLngBounds(r.bounds), { maxZoom: 16 });
      Kit.toast(`${r.name}. Press A to draw a study area here.${srcNote(r)}`);
    }
  }
  function startSetup() { if (areaOf()) openSetup(); else openFinder(); }
  async function openSetup() {
    if (!hasUI()) return;
    const b = areaOf();
    if (!b) { UI.setTool('onboarding-area'); return Kit.toast('Draw the study area first: drag a box around your site.', 'warn'); }
    if (setupRunning) return Kit.toast('The study area is already being set up. Wait for it to finish.', 'warn');
    const r = readout(b);
    const o = await Kit.modal('Set up this area', setupHtml(b), [['Cancel', null, ''], [r.ok ? 'Set up this area' : 'Too large to set up', () => {
      if (!r.ok) return null;
      const v = setupDefaults();
      for (const el of U.$$('#modal [data-onb-opt]')) v[el.dataset.onbOpt] = el.checked;
      return v;
    }]], true);
    if (!o) return;
    setupRunning = true;
    let sum = null;
    const go = async (signal, status) => {
      try {
        sum = await runSetup(b, { ...o, signal }, steps => { renderCard(steps); const cur = steps.find(s => s.state === 'run'); if (cur && status) status(`${cur.label}: ${cur.text || '…'}`); });
      } catch (err) { console.error(err); Kit.toast('Set-up stopped: ' + err.message, 'err'); }
    };
    try { if (Kit.task) await Kit.task('Setting up the study area…', go); else { Kit.busy('Setting up the study area…'); await go(null, t => Kit.busy(t)); } }
    finally { setupRunning = false; if (!Kit.task) Kit.busy(null); cardOpen = false; renderCard(); }
    if (!sum) return;
    View.show.counts = true; View.show.crashes = true;
    if (sum.ran && hasUI()) UI.setColour('vc');
    UI.refresh();
    const calTab = Registry.list('tabs').find(t => /calib/.test(t.id)) || Registry.get('tabs', 'data');
    const v = await Kit.modal(!sum.ran ? 'Study area set up' : (sum.notConverged || []).length ? 'Base model not converged' : 'Your base model is ready', summaryHtml(sum), [['Close', null, ''], ['Show the calibration', 'cal']], true);
    if (v === 'cal' && calTab) UI.openTab(calTab.id);
  }

  /* ---------- map extras: highlight layer, Shift-select, box-select, zone placement, drop files ---------- */
  function drawHighlights(ctx, off, size, g) {
    // not in report figures (Render.snapshot builds its own geometry), and only for the scenario on screen
    if (!g || g.connVer === 'snap' || g.sc !== Store.sc) return;
    const T = (typeof Render !== 'undefined' && Render.theme) || {};
    const strokeLink = (id, col, w, alpha, dash) => {
      const o = g.byId.get(id); if (!o) return;
      ctx.strokeStyle = col; ctx.globalAlpha = alpha; ctx.lineWidth = w; ctx.setLineDash(dash || []);
      ctx.beginPath();
      for (let i = 0; i < o.xy.length; i += 2) { const x = o.xy[i] - off.x, y = o.xy[i + 1] - off.y; if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y); }
      ctx.stroke();
    };
    const ring = (p, col, r, alpha, w = 3) => { if (!p) return; ctx.setLineDash([]); ctx.globalAlpha = alpha; ctx.strokeStyle = col; ctx.lineWidth = w; ctx.beginPath(); ctx.arc(p.x - off.x, p.y - off.y, r, 0, 7); ctx.stroke(); };
    ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    const sel = View.sel;
    if (sel && sel.type === 'multi') {
      for (const id of sel.links || []) strokeLink(id, T.sel || '#2459d6', 10, 0.45);
      for (const id of sel.nodes || []) ring(g.nodes.get(id), T.sel || '#2459d6', 12, 0.85);
    }
    (View.issues || []).forEach((it, i) => {
      const focus = i === View.issueFocus;
      for (const [t, id] of refsOf(it)) {
        if (t === 'link') strokeLink(id, ISSUE_COLOUR, focus ? 11 : 7, focus ? 0.85 : 0.55, focus ? null : [7, 5]);
        else if (t === 'node') ring(g.nodes.get(id), ISSUE_COLOUR, focus ? 15 : 11, focus ? 0.95 : 0.7);
        else if (t === 'zone') ring(g.zones.get(id), ISSUE_COLOUR, focus ? 17 : 14, focus ? 0.95 : 0.7);
      }
    });
    ctx.setLineDash([]); ctx.globalAlpha = 1;
  }
  function bindMapExtras() {
    const map = UI.map, C = map.getContainer();
    let down = null, boxEl = null;
    const cp = e => map.mouseEventToContainerPoint(e);
    const onUi = e => !!e.target.closest('.leaflet-control, .leaflet-marker-icon, .leaflet-popup');
    // Shift+press is our selection gesture: keep the shell from starting a junction or zone drag on pointerdown
    C.addEventListener('pointerdown', e => { if (e.button === 0 && e.shiftKey && View.tool === 'select' && !onUi(e)) e.stopPropagation(); }, true);
    C.addEventListener('mousedown', e => {
      down = { x: e.clientX, y: e.clientY };
      if (e.button !== 0 || !e.shiftKey || View.tool !== 'select' || onUi(e)) return;
      // Shift: our selection gesture, not a map pan
      e.preventDefault(); e.stopPropagation();
      const start = cp(e);
      let moved = false;
      const mm = ev => {
        const p = cp(ev);
        if (!moved && Math.hypot(p.x - start.x, p.y - start.y) < 4) return;
        moved = true;
        if (!boxEl) { boxEl = document.createElement('div'); boxEl.className = 'onb-box'; C.appendChild(boxEl); }
        Object.assign(boxEl.style, { left: Math.min(p.x, start.x) + 'px', top: Math.min(p.y, start.y) + 'px', width: Math.abs(p.x - start.x) + 'px', height: Math.abs(p.y - start.y) + 'px' });
      };
      const mu = ev => {
        document.removeEventListener('mousemove', mm); document.removeEventListener('mouseup', mu, true);
        if (boxEl) { boxEl.remove(); boxEl = null; }
        const sc = Store.sc;
        if (moved) {
          const a = map.containerPointToLatLng(start), b = map.containerPointToLatLng(cp(ev));
          const ids = linksInBox(sc, [[a.lat, a.lng], [b.lat, b.lng]]), m = multiOf(View.sel);
          select(makeSel([...m.links, ...ids], m.nodes));
          if (!ids.length) Kit.toast('No roads in that box.');
        } else {
          const h = Render.hit(start, { zones: false, routes: false, counts: false, layers: false });
          if (h && (h.type === 'link' || h.type === 'node')) select(toggle(View.sel, h.type, h.id));
        }
      };
      document.addEventListener('mousemove', mm); document.addEventListener('mouseup', mu, true);
    }, true);
    C.addEventListener('click', e => {
      if (onUi(e)) return;
      if (e.shiftKey && View.tool === 'select') { e.stopPropagation(); return; }     // handled on mouseup
      if (View.tool === 'zone' && !e.shiftKey) {
        if (down && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 5) return;  // the end of a map pan
        e.stopPropagation();
        const ll = map.mouseEventToLatLng(e);
        // one card only: the land-use lane's (with provenance) when it is loaded, otherwise ours
        if (landuseCard()) LandusePlus.placeZone(Store.sc, ll, { undo: () => UI.undo() }).catch(err => Kit.toast(err.message, 'warn'));
        else openPlacement([ll.lat, ll.lng], cp(e));
      }
    }, true);
  }
  // The zone placement card: no zone without a land-use type and an amount
  let place = null, placeEl = null;
  function openPlacement(ll, pt) {
    if (!placeEl) {
      placeEl = document.createElement('div'); placeEl.id = 'onbPlace'; placeEl.className = 'float card'; placeEl.setAttribute('role', 'dialog'); placeEl.setAttribute('aria-labelledby', 'onbPlaceT');
      placeEl.style.cssText = 'width:290px;padding:12px 14px;font-size:13px';
      U.$('#mapwrap').appendChild(placeEl);
      placeEl.addEventListener('click', e => {
        const c = e.target.closest('[data-onb-lu]');
        if (c) { place.key = c.dataset.onbLu; drawPlacement(true); return; }
        const b = e.target.closest('[data-onb]'); if (!b) return;
        if (b.dataset.onb === 'place-ok') finishPlacement(); else if (b.dataset.onb === 'place-cancel') closePlacement();
      });
      placeEl.addEventListener('change', e => { if (e.target.matches('[data-onb-lu-more]') && e.target.value) { place.key = e.target.value; drawPlacement(true); } });
      placeEl.addEventListener('input', e => {
        if (!e.target.matches('[data-onb-qty]')) return;
        place.raw = e.target.value;
        const n = parseNum(place.raw), bad = place.raw !== '' && !(n > 0);
        e.target.setAttribute('aria-invalid', String(bad));
        U.$('#onbPlaceErr').hidden = !bad;
        U.$('#onbPlace [data-onb="place-ok"]').disabled = !(n > 0);
      });
      placeEl.addEventListener('keydown', e => {
        if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); closePlacement(); }
        else if (e.key === 'Enter' && e.target.matches('[data-onb-qty]')) { e.preventDefault(); e.stopPropagation(); finishPlacement(); }
      });
    }
    place = { ll, key: null, raw: '' };
    const W = U.$('#mapwrap').clientWidth, H = U.$('#mapwrap').clientHeight;
    placeEl.style.left = Math.max(8, Math.min(pt.x + 12, W - 300)) + 'px';
    placeEl.style.top = Math.max(8, Math.min(pt.y - 20, H - 240)) + 'px';
    placeEl.hidden = false;
    drawPlacement(false);
    const f = placeEl.querySelector('[data-onb-lu]'); if (f) f.focus();
  }
  function drawPlacement(focusQty) {
    placeEl.innerHTML = placementHtml(place);
    if (focusQty) { const q = U.$('#onbPlaceQty'); if (q) q.focus(); }
  }
  function closePlacement() { if (placeEl) placeEl.hidden = true; place = null; }
  function finishPlacement() {
    if (!place) return;
    try {
      placeZone(Store.sc, place.ll, place.key, place.raw);
      const lu = LANDUSES[place.key], n = parseNum(place.raw);
      closePlacement();
      Kit.toast(`Zone added with ${U.fmt.int(n)} ${lu.unit} (${lu.label.toLowerCase()}). Add more land use in the panel.`);
    } catch (err) { Kit.toast(err.message, 'warn'); }
  }
  // Drag and drop a project, a GeoJSON road file or a CSV of counts anywhere on the page
  function dropKind(name, text) {
    const n = String(name || '').toLowerCase();
    if (n.endsWith('.csv')) return 'counts';
    if (!/\.(geo)?json$/.test(n)) throw new Error(`Spare Construct opens .corridor.json projects, .geojson road files and .csv counts, not “${name}”.`);
    let j;
    try { j = JSON.parse(text); } catch { throw new Error(`“${name}” is not valid JSON.`); }
    if (j && Array.isArray(j.scenarios)) return 'project';
    if (j && (j.type === 'FeatureCollection' || Array.isArray(j.features))) return 'geojson';
    throw new Error(`“${name}” is neither a Spare Construct project nor GeoJSON.`);
  }
  async function openDropped(file) {
    try {
      const text = await file.text(), kind = dropKind(file.name, text);
      if (kind === 'project') {
        const P = JSON.parse(text);
        if (!(await Kit.confirmBox(`Open “${P.name || file.name}”?`, 'The current project is replaced. Save it first (Menu › Save project) if you want to keep it.', 'Open project'))) return;
        Store.setProject(P); Kit.toast(`Opened ${P.name || file.name}.`);
      } else if (kind === 'geojson') {
        snap(`importing ${file.name}`);
        const n = Importers.importGeoJSON(Store.sc, JSON.parse(text), UI.roadProps());
        Store.edited();
        Kit.toast(`Imported ${plural(n, 'road')} from ${file.name}. Ctrl+Z to undo.`);
        const pts = Object.values(Store.sc.nodes).map(x => [x.lat, x.lng]); if (pts.length) UI.map.fitBounds(L.latLngBounds(pts));
      } else {
        let n;
        try { n = DataHub.importCSV(text); } catch (err) { throw new Error(`${err.message} If it is an OD matrix, use Menu › Import OD matrix.`); }
        View.show.counts = true; Store.emit('data');
        Kit.toast(`Imported ${plural(n, 'count')} from ${file.name}. They are matched to the nearest road.`);
      }
    } catch (err) { Kit.toast('Could not open that file: ' + err.message, 'err'); }
  }
  function bindDrop() {
    const ov = document.createElement('div');
    ov.className = 'onb-drop'; ov.hidden = true;
    ov.innerHTML = '<div class="card">Drop a <b>.corridor.json</b> project to open it, a <b>.geojson</b> file to import roads, or a <b>.csv</b> of traffic counts.</div>';
    document.body.appendChild(ov);
    let depth = 0;
    const files = e => !!(e.dataTransfer && [...(e.dataTransfer.types || [])].includes('Files'));
    window.addEventListener('dragenter', e => { if (!files(e)) return; depth++; ov.hidden = false; });
    window.addEventListener('dragover', e => { if (!files(e)) return; e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; });
    window.addEventListener('dragleave', e => { if (!files(e)) return; depth = Math.max(0, depth - 1); if (!depth) ov.hidden = true; });
    window.addEventListener('drop', e => { if (!files(e)) return; e.preventDefault(); depth = 0; ov.hidden = true; const f = e.dataTransfer.files[0]; if (f) openDropped(f); });
  }
  const STYLE = `
    .onb-list { max-height: min(52vh, 420px); overflow: auto; margin: 8px 0 4px; border: 1px solid var(--line); border-radius: 8px; }
    .onb-list:empty { display: none; }
    .onb-grp { font: 600 11px/1 var(--f-cond); letter-spacing: .08em; text-transform: uppercase; color: var(--muted); padding: 9px 10px 4px; }
    .onb-opt { display: flex; gap: 8px; align-items: baseline; padding: 6px 10px; cursor: pointer; min-width: 0; }
    .onb-opt .note { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
    .onb-opt kbd { margin-left: auto; flex: none; }
    .onb-opt.on, .onb-opt:hover { background: var(--accent-soft); }
    .onb-opt-l { flex: 0 1 auto; min-width: 0; overflow-wrap: anywhere; }
    .onb-steps { list-style: none; padding: 0; margin: 8px 0; display: grid; gap: 7px; font-size: 13px; }
    .onb-steps li { display: grid; grid-template-columns: 22px minmax(0, 1fr); gap: 8px; align-items: start; }
    .onb-dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: var(--amber); margin-right: 6px; vertical-align: 1px; }
    .onb-diff { font-size: 12.5px; margin: 3px 0; }
    .onb-search { width: 190px; max-width: 28vw; }
    .onb-box { position: absolute; z-index: 650; border: 1.5px dashed var(--sel); background: color-mix(in srgb, var(--sel) 12%, transparent); pointer-events: none; }
    .onb-drop { position: fixed; inset: 0; z-index: 3000; background: var(--scrim); display: grid; place-items: center; pointer-events: none; }
    .onb-drop .card { padding: 22px 26px; font-size: 16px; max-width: min(520px, calc(100% - 32px)); }
    tr[data-onb-issue].on td, tr[data-onb-issue]:focus-visible td { background: var(--accent-soft); }`;
  function injectStyle() {
    if (document.getElementById('onb-style')) return;
    const s = document.createElement('style'); s.id = 'onb-style'; s.textContent = STYLE; document.head.appendChild(s);
  }
  function stepScenario(d) {
    const P = Store.P, i = P.scenarios.findIndex(s => s.id === P.active), s = P.scenarios[(i + d + P.scenarios.length) % P.scenarios.length];
    if (!s || s.id === P.active) return;
    P.active = s.id; View.sel = null; View.sla = null;
    Store.emit('scenario');
    Kit.toast(`Scenario: ${s.name}`);
  }
  const toolKey = k => hasUI() && UI.tools().some(t => t.key && t.key.toLowerCase() === k);

  /* ---------- registrations ---------- */
  Registry.topbar({
    id: 'onboarding-search', zone: 'left', order: 10, title: 'Find a place: address, road or suburb', label: 'Find a place…', run: () => openFinder(),
    render: () => `<input type="search" class="onb-search" placeholder="Find a place" aria-label="Find a place: address, road or suburb" autocomplete="off" spellcheck="false" aria-haspopup="menu" aria-expanded="false">`,
    onInput(e) { if (e.target.matches('.onb-search')) topSearch().input(e.target.value); },
    onKeydown(e) {
      const input = e.target.closest('.onb-search'); if (!input) return;
      if (e.key === 'ArrowDown' && Kit.menuOpen()) { e.preventDefault(); const b = U.$('#menu button:not([disabled])'); if (b) b.focus(); }
      else if (e.key === 'Enter') {
        e.preventDefault();
        const q = input.value.trim(); if (!q) return;
        Kit.hideMenu(false);
        topSearch().enter(q).then(r => {
          if (r.length) { input.value = r[0].name; pickPlace(r[0]); }
          else Kit.toast(`No places found for “${q}”. Check the spelling, or try a nearby road or suburb.`, 'warn');
        }).catch(err => Kit.toast(err.message, 'err'));
      }
    },
  });
  let topTA = null;
  function topSearch() {
    return topTA ||= typeahead({ near: mapCentre, onResults: (r, q) => showTop(r, q), onError: (err, q) => showTop([], q, err) });
  }
  function showTop(items, q, err) {
    const input = U.$('#top .onb-search');
    if (!input || document.activeElement !== input || input.value.trim() !== q) return;
    if (!items.length && !err) { if (Kit.menuOpen()) Kit.hideMenu(false); return; }
    const list = err ? [{ label: err.message, disabled: true }] : items.map(r => ({ label: r.name, title: r.type, run: () => { input.value = r.name; pickPlace(r); } }));
    list.push(null, { label: SEARCH.attribution, disabled: true });
    Kit.menu(input, list, { focus: false });
  }

  Registry.tools({
    id: 'onboarding-area', key: 'A', label: 'Study area', group: 'analyse', order: 5, cursor: 'crosshair',
    iconSvgPath: '<rect x="4" y="5" width="16" height="14" rx="1" stroke-dasharray="3 2.4"/><path d="M2.5 3.5h3v3M21.5 3.5h-3v3M2.5 20.5h3v-3M21.5 20.5h-3v-3"/>',
    hint: '<b>Study area</b> · Drag a box around your site, or click to drop a 1.5 km square. Drag the corners to adjust, then press Set up this area (Enter). Esc to finish.',
    enter() { cardOpen = true; renderCard(); },
    leave() { if (areaTool.drawing) { areaTool.drawing = null; UI.map.dragging.enable(); } },
    down(e) {
      if (e.originalEvent.button !== 0 || e.originalEvent.shiftKey) return;
      UI.map.dragging.disable();
      areaTool.drawing = { start: e.latlng, cp: e.containerPoint, moved: false };
      const finish = () => areaTool.finish();
      areaTool.off = () => document.removeEventListener('mouseup', finish, true);
      document.addEventListener('mouseup', finish, true);
    },
    move(e) {
      const d = areaTool.drawing; if (!d) return;
      if (!d.moved && e.containerPoint.distanceTo(d.cp) < 6) return;
      d.moved = true; d.end = e.latlng;
      setArea([[d.start.lat, d.start.lng], [e.latlng.lat, e.latlng.lng]], false);
    },
    up() { areaTool.finish(); },
    keydown(e) {
      // Enter on a focused button, link, tab or option keeps its own meaning
      if (e.key !== 'Enter' || !areaOf() || (e.target && e.target.closest && e.target.closest('button, a, [role="tab"], [role="option"], [role="button"], [contenteditable]'))) return false;
      openSetup(); return true;
    },
  });
  const areaTool = {
    drawing: null, off: null,
    finish() {
      const d = this.drawing; if (!d) return;
      this.drawing = null; if (this.off) { this.off(); this.off = null; }
      UI.map.dragging.enable();
      if (d.moved && d.end) setArea([[d.start.lat, d.start.lng], [d.end.lat, d.end.lng]]);
      else setArea(box(d.start.lat, d.start.lng));
      cardOpen = true; renderCard();
    },
  };

  Registry.panels({ type: 'multi', label: 'Selection', render: multiPanel, onChange: onMultiChange, onClick: onMultiClick, onDelete: sel => { onMultiDelete(sel).catch(err => Kit.toast(err.message, 'err')); } });

  Registry.inspectorSections({
    id: 'onboarding-street', types: ['link'], order: 5,
    render(sc, sel) {
      const w = wholeStreet(sc, sel.id);
      if (w.ids.length < 2) return '';
      return `<div class="ins-sec"><div class="actions"><button class="btn small" data-onb="street">Select whole street (${plural(w.ids.length, 'segment')} · ${U.fmt.len(w.len)})</button></div></div>`;
    },
    onClick(e) {
      if (!e.target.closest('[data-onb="street"]')) return false;
      const w = wholeStreet(Store.sc, View.sel.id);
      select(makeSel(w.ids, []));
      return true;
    },
  });
  Registry.inspectorSections({
    id: 'onboarding-gateway', types: ['gateway'], order: 20, render: gatewaySection,
    onClick(e) {
      if (!e.target.closest('[data-onb="gw-counts"]')) return false;
      const r = gatewayFromCounts(Store.sc, View.sel.id);
      Kit.toast(r.ok ? `Gateway flows set from counts: ${r.src}.` : r.why, r.ok ? '' : 'warn');
      return true;
    },
  });
  Registry.inspectorSections({
    id: 'onboarding-zone', types: ['zone'], order: 30, render: zoneSection,
    onClick(e) {
      if (!e.target.closest('[data-onb="dup-zone"]')) return false;
      try { duplicateZone(Store.sc, View.sel.id); Kit.toast('Zone duplicated beside the original. Move it and change its land use.'); } catch (err) { Kit.toast(err.message, 'warn'); }
      return true;
    },
    onChange(e) {
      const t = e.target; if (!t.matches('[data-onb-copylu]') || !t.value) return false;
      const from = Store.sc.zones[t.value];
      if (copyLanduse(Store.sc, View.sel.id, t.value)) Kit.toast(`Copied the land use from ${from.name}. Ctrl+Z to undo.`);
      return true;
    },
  });
  Registry.inspectorSections({
    id: 'onboarding-scenarios', types: ['link', 'node', 'zone', 'gateway'], order: 90, render: scenarioSection,
    onChange(e) {
      const t = e.target, [set, id] = t.dataset.onbCopyto ? [copyScope, t.dataset.onbCopyto] : t.dataset.onbScope ? [scope, t.dataset.onbScope] : [null, null];
      if (!set) return false;
      if (t.checked) set.add(id); else set.delete(id);
      return true;
    },
    onClick(e) {
      const sel = View.sel; if (!sel) return false;
      const rs = e.target.closest('[data-onb-reset]');
      if (rs) {
        e.preventDefault();
        try { if (resetField(sel.type, sel.id, rs.dataset.onbReset)) Kit.toast(`Reset to ${Store.baseSc.name}. Ctrl+Z to undo.`); } catch (err) { Kit.toast(err.message, 'warn'); }
        return true;
      }
      if (!e.target.closest('[data-onb="copy-to"]')) return false;
      const ids = [...copyScope].filter(id => id !== Store.sc.id);
      if (!ids.length) { Kit.toast('Tick the scenarios to copy to first.', 'warn'); return true; }
      const r = copyTo(sel.type, sel.id, ids);
      const notes = [r.kept.length ? ` The road is split differently in ${andList(r.kept)}, so only its settings were copied there.` : '',
        r.skipped.length ? ` Not copied to ${andList(r.skipped)}: this piece of a split road would lie on top of the whole road there.` : '',
        r.loose.length ? ` In ${andList(r.loose)} it could not be joined to the road it meets here: join it with the road tool.` : ''].join('');
      Kit.toast(r.scenarios.length ? `Copied to ${andList(r.scenarios)}. Their results are out of date.${notes}` : `Nothing was copied.${notes}`, r.skipped.length || r.loose.length ? 'warn' : '');
      return true;
    },
  });

  Registry.layers({
    id: 'onboarding-highlight', label: 'Selection and network check', order: 900, defaultOn: true, draw: drawHighlights,
    legend: () => ((View.issues || []).length ? `<div class="lg-title">Network check</div><div class="lg-row"><span class="lg-sw" style="background:${ISSUE_COLOUR}"></span><span>${plural(View.issues.length, 'issue')} · see the Network check tab</span></div>` : ''),
  });

  Registry.tabs({
    id: 'onboarding-check', label: 'Network check', order: 85, title: 'Problems in the network that stop traffic loading or routing', hidden: () => !lastCheck, render: checkTab,
    onClick(e) {
      if (e.target.closest('[data-onb="recheck"]')) { runCheck(false); return true; }
      const r = e.target.closest('[data-onb-issue]'); if (!r) return false;
      focusIssue(+r.dataset.onbIssue); Drawer.render(); return true;
    },
    onKeydown(e) { const r = e.target.closest('[data-onb-issue]'); if (!r || (e.key !== 'Enter' && e.key !== ' ')) return false; e.preventDefault(); focusIssue(+r.dataset.onbIssue); return true; },
  });
  const goRow = el => {
    const [sid, type, id] = el.dataset.onbGo.split('|');
    if (Store.P.active !== sid) { Store.P.active = sid; View.sel = null; Store.emit('scenario'); }
    if (type === 'link' || type === 'node' || type === 'zone') UI.flyTo(type, id);
  };
  Registry.tabs({
    id: 'onboarding-changes', label: 'Option changes', order: 27, title: 'What each option changes compared with the base scenario', hidden: () => !(Store.P && Array.isArray(Store.P.scenarios) && Store.P.scenarios.length > 1), render: changesTab,
    onClick(e) {
      if (e.target.closest('[data-onb="changes-csv"]')) { UI.exportFile('onboarding-changes'); return true; }
      const r = e.target.closest('[data-onb-go]'); if (!r) return false;
      goRow(r); return true;
    },
    onKeydown(e) { const r = e.target.closest('[data-onb-go]'); if (!r || e.key !== 'Enter') return false; e.preventDefault(); goRow(r); return true; },
  });
  // stamp: a human-readable table (the compliance feature adds its D1 / version footer); authorText: scenario, road and
  // zone names the author typed go out in it, so the scope linter can hold it back
  Registry.exports({ id: 'onboarding-changes', label: 'Option changes (CSV)', ext: 'csv', mime: 'text/csv', order: 45, stamp: true, authorText: true, enabled: () => !!(Store.P && Array.isArray(Store.P.scenarios) && Store.P.scenarios.length > 1), filename: c => `${c.fileBase}-option-changes.csv`, build: P => changesCSV(P || Store.P) });
  Registry.report({ id: 'onboarding-changes', chapter: '3', order: 15, title: 'Changes in each option', render: reportChanges });

  Registry.welcome({ id: 'onboarding-setup', title: 'Set up a study area', order: 5, text: 'Search for a place, draw a box, and Spare Construct downloads the roads, counts, crashes and existing land use, then runs a base model.', run: () => openFinder() });

  Registry.menu({ id: 'onboarding-setup', label: 'Set up a study area…', order: 100, run: () => startSetup() });
  Registry.menu({ id: 'onboarding-check', label: 'Check network', order: 160, run: () => runCheck(true) });
  Registry.menu({ id: 'onboarding-palette', label: 'Command palette…', order: 405, keys: 'Ctrl+K', run: () => openPalette() });

  Registry.checklist({
    id: 'onboarding-network', group: 'Model set-up', order: 10, label: 'Network check', source: 'Spare Construct network check',
    status() {
      const sc = Store.sc;
      if (!Object.keys(sc.links).length) return { state: 'na', text: 'Network check: no roads yet.' };
      const c = check(sc), show = { label: 'Show', run: () => { setIssues(c); if (hasUI()) { UI.openTab('onboarding-check'); Render.redraw(); } } };
      return c.counts.error ? { state: 'warn', text: `Network check: ${plural(c.counts.error, 'problem')} in ${esc(sc.name)}`, action: show }
        : { state: 'done', text: `Network check: no problems in ${esc(sc.name)}${c.counts.warn ? ` (${c.counts.warn} to check)` : ''}`, action: show };
    },
  });

  Registry.about({
    id: 'onboarding', title: 'Place search and study area', order: 900, intro: 'Used to find and size your study area. Only the search text, and the corners of the study-area box, are sent to these services.',
    rows: () => [
      ['Roads in the study area (count)', '<a href="https://wiki.openstreetmap.org/wiki/Overpass_API" target="_blank" rel="noopener">OpenStreetMap Overpass API</a>', `<b>Real.</b> ${esc(SEARCH.licence)}. A count of the roads in the box, asked once the box stops changing, so you can judge its size before setting it up.`],
      ['Typeahead place search', '<a href="https://photon.komoot.io/" target="_blank" rel="noopener">Photon by komoot</a>', `<b>Real.</b> ${esc(SEARCH.licence)}. Asked after you pause typing (3+ letters), limited to New Zealand.`],
      ['Search on Enter', '<a href="https://nominatim.openstreetmap.org/" target="_blank" rel="noopener">OpenStreetMap Nominatim</a>', `<b>Real.</b> ${esc(SEARCH.licence)}. Only when Photon has no match; never while typing, at most one request a second.`],
    ],
  });

  const cmd = d => Registry.commands(d);
  cmd({ id: 'onboarding-palette', label: 'Command palette: find roads, junctions, zones, counts and commands', keys: 'Ctrl+K, /', group: 'App', run: () => openPalette() });
  cmd({ id: 'onboarding-find', label: 'Find a place…', group: 'Project', run: () => openFinder() });
  cmd({ id: 'onboarding-setup', label: 'Set up a study area…', group: 'Project', run: () => startSetup() });
  cmd({ id: 'onboarding-check', label: 'Check network', group: 'Model', run: () => runCheck(true) });
  cmd({ id: 'onboarding-dup-zone', label: 'Duplicate the selected zone', keys: 'Ctrl+D', group: 'Edit', enabled: () => !!(View.sel && View.sel.type === 'zone' && Store.sc.zones[View.sel.id] && Store.sc.zones[View.sel.id].kind !== 'gateway'),
    run: () => { try { duplicateZone(Store.sc, View.sel.id); Kit.toast('Zone duplicated beside the original. Move it and change its land use.'); } catch (err) { Kit.toast(err.message, 'warn'); } } });
  cmd({ id: 'onboarding-street', label: 'Select the whole street', group: 'Edit', enabled: () => !!(View.sel && View.sel.type === 'link'), run: () => select(makeSel(wholeStreet(Store.sc, View.sel.id).ids, [])) });
  cmd({ id: 'onboarding-prev-sc', label: 'Previous scenario (keeps the view)', keys: '[', group: 'Scenario', enabled: () => !!Store.P && Store.P.scenarios.length > 1, run: () => stepScenario(-1) });
  cmd({ id: 'onboarding-next-sc', label: 'Next scenario (keeps the view)', keys: ']', group: 'Scenario', enabled: () => !!Store.P && Store.P.scenarios.length > 1, run: () => stepScenario(1) });
  cmd({ id: 'onboarding-period', label: 'Switch between the AM and PM peaks', keys: '\\', group: 'Model', run: () => { S().period = S().period === 'AM' ? 'PM' : 'AM'; Store.emit('period'); Kit.toast(`${S().period} peak`); } });
  cmd({ id: 'onboarding-fit', label: 'Fit the map to the network', keys: 'F', group: 'Map', enabled: () => !toolKey('f'),
    run: () => { const pts = Object.values(Store.sc.nodes).map(n => [n.lat, n.lng]); if (pts.length) UI.map.fitBounds(L.latLngBounds(pts).pad(0.05)); else Kit.toast('No roads yet. Import some or draw them first.'); } });
  cmd({ id: 'onboarding-labels', label: 'Show or hide labels', keys: 'L', group: 'Map', enabled: () => !toolKey('l'), run: () => { View.show.labels = !View.show.labels; Render.redraw(); Inspector.render(); } });
  cmd({ id: 'onboarding-drawer', label: 'Show or hide the results panel', keys: 'H', group: 'Map', enabled: () => !toolKey('h'), run: () => { View.drawerOpen = !View.drawerOpen; Drawer.render(); } });

  Registry.onInit(() => {
    // the browser shell only: headless tests fire 'init' without a page
    if (!Kit.hasDom() || typeof document.getElementById !== 'function' || !U.$('#mapwrap') || !hasUI()) return;
    injectStyle();
    bindMapExtras();
    bindDrop();
    card = document.createElement('div');
    card.id = 'onbArea'; card.className = 'float card'; card.hidden = true; card.setAttribute('role', 'region'); card.setAttribute('aria-label', 'Study area');
    card.style.cssText = 'top:54px;left:10px;width:300px;max-width:calc(100% - 20px);padding:12px 14px;font-size:13px';
    U.$('#mapwrap').appendChild(card);
    card.addEventListener('click', e => {
      const b = e.target.closest('[data-onb]'); if (!b) return;
      const a = b.dataset.onb, bb = areaOf();
      if (a === 'setup') openSetup();
      else if (a === 'area-fit' && bb) UI.map.fitBounds(bb, { padding: [30, 30] });
      else if (a === 'area-clear') { setArea(null); Kit.toast('Study area removed.'); }
      else if (a === 'area-hide') { cardOpen = false; renderCard(); }
    });
    card.addEventListener('change', e => {
      const t = e.target, bb = areaOf(); if (!t.dataset.onbSize || !bb) return;
      const km = parseNum(t.value); if (!(km >= 0.2 && km <= 20)) { Kit.toast('Enter a size between 0.2 and 20 km.', 'warn'); return renderCard(); }
      const r = readout(bb), c = [(bb[0][0] + bb[1][0]) / 2, (bb[0][1] + bb[1][1]) / 2];
      setArea(box(c[0], c[1], t.dataset.onbSize === 'w' ? km * 1000 : r.w, t.dataset.onbSize === 'h' ? km * 1000 : r.h));
    });
    U.$('#tools').addEventListener('click', () => { closePlacement(); setTimeout(renderCard, 0); });
    // Save as (Ctrl+Shift+S) when the project store offers it and nobody else has claimed the keys
    if (Store.projects && typeof Store.projects.saveAs === 'function' && !Registry.list('commands').some(c => /ctrl\+shift\+s/i.test(c.keys || '')))
      cmd({ id: 'onboarding-save-as', label: 'Save as a new project…', keys: 'Ctrl+Shift+S', group: 'Project', run: async () => {
        const name = await Kit.ask('Save as a new project', `${Store.P.name} (copy)`); if (!name) return;
        const id = await Store.projects.saveAs(name);
        Kit.toast(id ? `Saved as ${name}.` : 'Could not save a copy in this browser. Use Menu › Save project to keep a file.', id ? '' : 'warn');
      } });
    drawArea();
  });
  Registry.onStore(kind => {
    // Edits apply to: an edit's own event carries it to the ticked scenarios; undo and redo are never copied
    if (/^(net|demand|meta)$/.test(kind)) mirrorToast(mirrorEdit());
    else if (kind === 'undo') { const t = Store.undoStack[Store.undoStack.length - 1]; if (t) mirrorSeen.add(t); }
    if (kind === 'project') { scope = new Set(); copyScope = new Set(); setIssues(null); closePlacement(); cardOpen = false; drawArea(); }
    else if (kind === 'meta' && rect) rect.setStyle({ color: accent(), fillColor: accent() });
    if (lastCheck && /^(net|undo|demand|scenario|data)$/.test(kind)) {
      const focus = View.issueFocus;
      setIssues(check(Store.sc));
      if (focus != null && focus < View.issues.length) View.issueFocus = focus;
      if (hasUI()) { Render.redrawNet(); UI.renderLegend(); if (View.drawer === 'onboarding-check' && View.drawerOpen) Drawer.render(); }
    }
  });
  Registry.onSelect(() => { if (hasUI()) Render.redrawNet(); });
  // After a run with unserved trips: check the network and say where to look
  Registry.onResults((sc, period, res) => {
    if (!hasUI() || setupRunning || sc !== Store.sc || !(res && res.kpi && res.kpi.unserved > 1)) return;
    setIssues(check(sc));
    setTimeout(() => {
      Render.redrawNet(); UI.renderLegend();
      Kit.toast(`${U.fmt.int(res.kpi.unserved)} veh/h couldn't find a route in the ${period} peak.`, 'warn', { action: { label: 'Show why', run: () => UI.openTab('onboarding-check') } });
    }, 60);
  });

  return {
    parseNum, SEARCH, CHECK_RULES, photon, typeahead, box, readout, areaOf, setArea, STEPS, setupDefaults, setupHtml, stepsHtml, stepOf, runSetup, fillCensus, summaryHtml,
    check, checkSummary, setIssues, selForIssue, pointsOf, focusIssue, runCheck, checkTab, get lastCheck() { return lastCheck; },
    makeSel, multiOf, toggle, wholeStreet, linksInBox, multiFields, MIXED, applyMulti, deleteMulti, resetMulti, multiPanel,
    fieldsFor, compareObj, scenarioSection, copyTo, resetField, diff, changesTab, changesCSV,
    duplicateZone, copyLanduse, zoneSection, placeZone, placementHtml, gatewayFromCounts, gatewaySection,
    score, paletteItems, paletteListHtml, openPalette, openFinder, pickPlace, openSetup, startSetup, dropKind,
    get scope() { return scope; }, get copyScope() { return copyScope; }, deleteTargets, linksIn, isSplit, gehTarget, roadCount, mirrorEdit, copySplitAt, reportChanges,
  };
})();
