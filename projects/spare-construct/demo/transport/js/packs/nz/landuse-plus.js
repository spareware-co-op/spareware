'use strict';
/* Spare Construct · land use plus (feature file, lane 'landuse').
   Existing land use from official data, person-trip generation, zoned yields and development traffic.

   - Existing land use (docs/spec/existing-landuse): Stats NZ Census 2023 SA1 (dwellings, households, vehicles,
     travel to work and education), Stats NZ employees by SA2 (2025, a hard control), the Ministry of Education
     school and early learning directory, and Auckland Unitary Plan base zones. Each is fetched for the study area
     (AbortController, cached per area, fails soft), allocated to Spare Construct zones with provenance (z.luSrc, z.census,
     z.jobs, z.moe, z.aup) and combined with the OpenStreetMap zones from ExistingLU: CHOICES says which source wins
     for each item and why. VAR codes, the SA1/SA2 code fields, the latest employee-count year and the AUP zone
     names are resolved from each layer's ?f=json at run time.
   - Trip generation (docs/spec/trip-generation, AT ITA Guidelines 2025 v2 Appendix 1): per zone and activity, a
     vehicle rate (Settings) or a person-trip rate with mode split and car occupancy, then the adjustments in
     sequence (less existing site trips, internal, pass-by, TDM = z.reduction), freight kept separate, tables
     T1–T5, the RR 422 sanity row and trip_generation.csv.
   - Zoned yield (docs/regulatory/reg-plan-change, yield part): dwellings/ha and GFA ratio per AUP zone, labelled
     'Spare Construct default yield; check plan rules'; full build-out uses all zoned capacity.
   - Development traffic (docs/spec/dev-tracking (b), (d)): gateway distribution table and figure, and the
     'without vs with development' junction table, from Engine.devDistribution and Engine.withWithout.
   - Zone and gateway inspector sections (docs/spec/zone-gateway-ux (b)–(d)).

   How a trip-generation table reaches the assignment: sync() writes z.tgLoad = { AMin, AMout, PMin, PMout }, the
   table's trips before the zone's TDM reduction and staging (T4 after pass-by, plus freight ÷ (1 − reduction)).
   An engine that reads z.tgLoad (in place of Σ land use × Settings rate + z.extra, when z.tg.on) loads it
   directly and z.extra stays the user's own 'other trips'. The current engine.js does not read it, so until it
   does (detected at run time, tgNative()), sync() also rewrites z.extra so that Engine.tripEnds loads exactly the
   T4 net trips plus freight; the trips that were in z.extra are kept as tg.other and put back when the table is
   switched off. Store events re-sync every such zone, so rate or land-use edits keep them consistent.

   Deviations (also in the report):
   - Census car share uses the five named modes (driver, passenger, public transport, walk, cycle) rescaled to
     100%; 'Other' and 'Work at home' are left out. The spec's (775+776+777)/(786−773) is kept as carSpec.
   - SA1s and SA2s that cross the study-area edge are split by area overlap (sampled), because building outlines
     are only downloaded inside the study area; inside, dwellings follow residential building footprints.
   - Employees are allocated by commercial and industrial floor area; floor area itself stays from OSM (there is
     no free official floor-area source), so the employee counts are a control and a cross-check, not a trip input.
   - The tab, checklist items and report sections are registered in Registry.onInit (registerLate), because
     test/registry.test.js asserts the exact built-in tab and report lists at load time.
   - A zone centre on a Road, Water, Strategic Transport Spare Construct or coastal marine polygon takes the nearest land-use
     zone within 80 m, and says so (z.aup.near, z.aup.note).
   - The zone tool's card is placeZone(sc, latlng, { undo }); ui.js's built-in zone tool cannot be replaced from a
     feature file, so the onboarding lane (or ui.js) calls it.
   - 'Pick a count site on the map' arms a pick from the gateway panel; the count's own panel (a registered count
     section) then offers 'Use for <gateway>'. Only that count is used (DataHub.gatewaysFromCounts with the other
     counts set aside for the call).
   - The Future Urban Zone has no zoned capacity (AUP H18 holds it until it is rezoned): its default yield is a
     structure-plan assumption, shown and totalled separately, never in 'Full build-out (zoned capacity)'.
   - The PC120 proposed-zone overlay is not built (no verified open service). A zoned area can instead be given a
     proposed zone by the user; its capacity is labelled 'proposed (user-selected)' and totalled separately.
   - Census layers fail soft one by one. Without the dwellings layer the dwellings are left as they were and only
     people, households, vehicles and travel are filled. */

const LandusePlus = (() => {
  /* ---------- sources and standards (cross-cutting rule 4: data with source, edition and verified flag) ---------- */
  const STATS = 'https://services2.arcgis.com/vKb0s8tBIA3bdocZ/arcgis/rest/services';
  const CKAN = 'https://catalogue.data.govt.nz/api/3/action/datastore_search_sql';
  const MOE_URL = 'https://catalogue.data.govt.nz/dataset/directory-of-educational-institutions';
  const SRC = {
    census: { id: 'census', name: 'Stats NZ 2023 Census totals by topic, SA1', vintage: 'Census 2023 (7 March 2023)', licence: 'CC BY 4.0, Stats NZ', url: 'https://www.stats.govt.nz/',
      rounding: 'Stats NZ random-rounds counts to base 3, so sums of SA1 counts can differ slightly from published totals',
      layers: { ind0: `${STATS}/2023_Census_totals_by_topic_for_individuals_by_SA1/FeatureServer/0`, ind2: `${STATS}/2023_Census_totals_by_topic_for_individuals_by_SA1/FeatureServer/2`,
        dw: `${STATS}/2023_Census_totals_by_topic_for_dwellings_by_SA1/FeatureServer/0`, hh: `${STATS}/2023_Census_totals_by_topic_for_households_by_SA1/FeatureServer/0` } },
    employees: { id: 'employees', name: 'Stats NZ business demography, employees by SA2', licence: 'CC BY 4.0, Stats NZ', url: `${STATS}/2025_employees_by_statistical_area_2_2/FeatureServer/0` },
    schools: { id: 'schools', name: 'Ministry of Education, Directory of Educational Institutions (schools)', licence: 'CC BY 4.0, Ministry of Education (confirm)', resource: '4b292323-9fcc-41f8-814b-3c7b19cf14b3', url: MOE_URL },
    ece: { id: 'ece', name: 'Ministry of Education, Directory of Educational Institutions (early learning services)', licence: 'CC BY 4.0, Ministry of Education (confirm)', resource: 'a9d65b07-8483-4b05-bdfd-d2abe4f38827', url: MOE_URL },
    aup: { id: 'aup', name: 'Auckland Unitary Plan (Operative in part), base zones', licence: 'Auckland Council open data, CC BY 4.0 (confirm before publishing)', url: 'https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services/Unitary_Plan_Base_Zone/FeatureServer/0' },
    osm: { id: 'osm', name: 'OpenStreetMap building outlines (LINZ import), classified by Spare Construct', licence: 'ODbL, © OpenStreetMap contributors' },
  };
  const ITA_URL = 'https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf';
  const STD = {
    ita: { id: 'at-ita-2025-v2', name: 'AT Integrated Transport Assessment Guidelines 2025 v2, Appendix 1', url: ITA_URL, verified: false,
      review: 'review per AT ITA Guidelines App 1(iv)', reviewTo: 2022,
      sequence: 'gross vehicle trips, less existing site trips, less internal trips, less pass-by and undiverted linked trips, less travel demand management (TDM)' },
    rr422: { id: 'nzta-rr422', name: 'NZTA Research Report 422, Trips and parking related to land use (2010)', url: 'https://www.nzta.govt.nz/assets/resources/research/reports/422/docs/422.pdf', verified: false,
      perHousehold: 1, band: [0.4, 1.2], bandLabel: 'Spare Construct band (assumption) around RR 422’s about 1 peak-hour vehicle trip per household' },
    census: { minN: 30, minNLabel: 'Spare Construct assumption: a mode share from fewer than 30 people falls back to the default',
      newer: 0.10, newerAbs: 3, newerLabel: 'Spare Construct assumption: OSM more than 10% (and 3 dwellings) above the Census suggests homes built since March 2023' },
    split: { driver: 75, passenger: 7, pt: 10, walk: 6, cycle: 2, label: 'Spare Construct assumption (indicative Auckland suburban mode share); replace with surveyed or Census data' },
    eduOcc: { v: 1.3, label: 'Spare Construct assumption: 1.3 students per car on education trips' },
    createM: 1000,       // Spare Construct assumption: Census dwellings more than 1 km from any existing zone get a zone of their own
    jobsCheck: [0.7, 1.3],
    yields: { label: 'Spare Construct default yield; check plan rules', netPct: 70, verified: false,
      netLabel: 'Spare Construct assumption: 70% of the zoned land is developable (the rest is roads, reserves and stormwater)',
      none: 'no Spare Construct default; enter a yield', notLand: 'not for development (no yield)', user: 'user-entered yield',
      assumed: 'assumed; not zoned capacity', proposed: 'proposed (user-selected); not operative zoning' },
  };
  // Default yields per AUP base zone code: dph = dwellings per developable ha, far = GFA ÷ developable land, mix = GFA shares
  const YIELDS = {
    19: { dph: 15, homes: 'residential' }, 18: { dph: 30, homes: 'residential' }, 60: { dph: 60, homes: 'mixed' }, 8: { dph: 120, homes: 'apartment' },
    35: { dph: 250, homes: 'apartment', far: 4, mix: { office: 0.7, retail: 0.3 } }, 10: { dph: 120, homes: 'apartment', far: 1.5, mix: { retail: 0.5, office: 0.5 } },
    22: { dph: 80, homes: 'apartment', far: 1.2, mix: { retail: 0.6, office: 0.4 } }, 7: { dph: 40, homes: 'apartment', far: 0.6, mix: { retail: 0.8, office: 0.2 } },
    44: { dph: 30, homes: 'apartment', far: 0.4, mix: { retail: 1 } }, 12: { dph: 60, homes: 'apartment', far: 0.6, mix: { office: 0.5, retail: 0.5 } },
    49: { far: 0.5, mix: { retail: 0.5, warehouse: 0.5 } }, 1: { far: 0.6, mix: { office: 1 } }, 17: { far: 0.5, mix: { industrial: 0.5, warehouse: 0.5 } },
    5: { far: 0.4, mix: { industrial: 1 } }, 63: { per: { school: 120 } }, 64: { per: { tertiary: 200 } }, 55: { far: 0.6, mix: { medical: 1 } },
    4: { dph: 25, homes: 'residential', assumed: true, note: 'Future Urban Zone (AUP H18): no urban development is enabled until the land is rezoned, so this is a structure-plan density assumption, not zoned capacity' },
  };
  // Zoned land that takes no yield: roads, water, transport corridors, open space, cemeteries and coastal zones
  const NO_YIELD = new Set([25, 26, 27, 30, 31, 32, 33, 34, 37, 39, 40, 41, 45, 53, 59, 61, 62]);
  const NO_YIELD_GROUPS = /^(public open space|coastal)$/i;
  const GFA_USES = ['retail', 'office', 'industrial', 'warehouse', 'medical'];
  const AUP_NAMES = { 19: 'Residential - Single House', 18: 'Residential - Mixed Housing Suburban', 60: 'Residential - Mixed Housing Urban', 8: 'Residential - Terrace Housing and Apartment Building',
    35: 'Business - City Centre', 10: 'Business - Metropolitan Centre', 22: 'Business - Town Centre', 7: 'Business - Local Centre', 44: 'Business - Neighbourhood Centre', 12: 'Business - Mixed Use',
    49: 'Business - General Business', 1: 'Business - Business Park', 17: 'Business - Light Industry', 5: 'Business - Heavy Industry', 63: 'Special Purpose - School', 64: 'Special Purpose - Tertiary Education',
    55: 'Special Purpose - Healthcare Facility and Hospital', 4: 'Future Urban' };
  const TIMEOUT = 30000;
  const DEFAULT_RATE_SOURCE = 'Spare Construct indicative default (Settings › Trip rates)';
  const DEFAULTS = () => ({ landuse: { split: { driver: STD.split.driver, passenger: STD.split.passenger, pt: STD.split.pt, walk: STD.split.walk, cycle: STD.split.cycle }, eduOcc: STD.eduOcc.v, rateMeta: {} } });
  const conf = S => U.deepMerge(DEFAULTS().landuse, (S && S.landuse) || {});
  const PERIODS = ['AM', 'PM'];
  const MODES = ['driver', 'passenger', 'pt', 'walk', 'cycle'];
  const MODE_LABEL = { driver: 'Car driver', passenger: 'Car passenger', pt: 'Public transport', walk: 'Walk', cycle: 'Cycle' };
  // Census travel topic per land use: r = to work by usual residence, w = to work by workplace, e = to education
  const GROUP_OF = { residential: 'r', apartment: 'r', retirement: 'r', school: 'e', childcare: 'e', tertiary: 'e' };
  const MODE_SRC = { r: 'Census 2023 main means of travel to work, by usual residence', w: 'Census 2023 main means of travel to work, by workplace', e: 'Census 2023 main means of travel to education, by education address' };
  const JOB_KEYS = ['office', 'retail', 'industrial', 'warehouse', 'supermarket', 'fastfood', 'medical'];

  // Which source wins for each item, and why (shown in the tab and the report)
  const CHOICES = [
    { id: 'dwellings', item: 'Dwellings (houses, apartments)', use: 'Census 2023', instead: 'OpenStreetMap',
      why: 'The Census counts occupied private dwellings (random-rounded to base 3). Building outlines miss units in one building and include outbuildings, so they are used only to place the Census dwellings inside each SA1. Separate houses become houses and joined dwellings apartments.' },
    { id: 'people', item: 'Population, households, vehicles, travel to work and education', use: 'Census 2023', instead: '–', why: 'Only the Census has these. Mode shares feed the person-trip tables.' },
    { id: 'jobs', item: 'Employees', use: 'Stats NZ employees by SA2 (2025)', instead: 'Census 2023 employed by workplace',
      why: 'Newer than the Census. Each SA2 total is kept as a hard control and placed in zones by commercial and industrial floor area; the Census workplace count is a cross-check.' },
    { id: 'schools', item: 'School rolls', use: 'Ministry of Education directory', instead: 'OpenStreetMap', why: 'The directory lists every registered school with its roll and roll date; OpenStreetMap rolls are tags or defaults (350 to 800).' },
    { id: 'ece', item: 'Early learning places', use: 'Ministry of Education directory', instead: 'OpenStreetMap', why: 'Licensed places for each service, where OpenStreetMap only gives a default of 40 children. Home-based networks are left out (no site of their own).' },
    { id: 'gfa', item: 'Floor area (retail, office, industrial, warehouse, supermarket, medical, hotel, fast food)', use: 'OpenStreetMap', instead: '–', why: 'No free official floor-area source: building footprint × levels, indicative.' },
    { id: 'zoning', item: 'Zoning', use: 'Auckland Unitary Plan base zones', instead: '–', why: 'Operative base zones at each zone centre, for context and default yields.' },
  ];

  /* ---------- small helpers ---------- */
  const esc = s => U.esc(s);
  const today = () => new Date().toISOString().slice(0, 10);
  const r2 = x => Math.round(x * 100) / 100;
  const n1 = x => (x == null || !isFinite(x) ? '–' : U.fmt.num(x, 1));
  const n0 = x => (x == null || !isFinite(x) ? '–' : U.fmt.int(x));
  const pc = x => (x == null || !isFinite(x) ? '–' : U.fmt.num(x, 0) + '%');
  // '12,000', '5000 m2', '5 000 m²', '250 sqm' → number; '' → null; anything else → NaN (zone-gateway-ux (d))
  function parseNum(v) {
    if (typeof v === 'number') return isFinite(v) ? v : NaN;
    const s = String(v ?? '').replace(/m²|m2|sqm/gi, '').replace(/[\s, ]/g, '');
    if (s === '') return null;
    return /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(s) ? +s : NaN;
  }
  // Box {s, w, n, e} from Leaflet bounds, [[s, w], [n, e]], {south, …} or {s, …}
  function box(b) {
    if (!b) return null;
    if (typeof b.getSouth === 'function') return { s: b.getSouth(), w: b.getWest(), n: b.getNorth(), e: b.getEast() };
    if (Array.isArray(b)) return { s: Math.min(b[0][0], b[1][0]), w: Math.min(b[0][1], b[1][1]), n: Math.max(b[0][0], b[1][0]), e: Math.max(b[0][1], b[1][1]) };
    if ('south' in b) return { s: b.south, w: b.west, n: b.north, e: b.east };
    return 's' in b && [b.s, b.w, b.n, b.e].every(isFinite) ? { s: b.s, w: b.w, n: b.n, e: b.e } : null;
  }
  /* The study area, current first: the box drawn in onboarding (P.onboarding.area), else the extent of the scenario's
     network and zones, else the box of the last fill. .basis says which one (the tab and the summary show it). */
  function areaBox(sc = Store.sc) {
    const P = Store.P, drawn = P && P.onboarding && box(P.onboarding.area);
    if (drawn) return { ...drawn, basis: 'the study area' };
    const pts = [...Object.values((sc && sc.nodes) || {}), ...Object.values((sc && sc.zones) || {})].filter(p => isFinite(p.lat) && isFinite(p.lng));
    if (pts.length) return { s: Math.min(...pts.map(p => p.lat)), n: Math.max(...pts.map(p => p.lat)), w: Math.min(...pts.map(p => p.lng)), e: Math.max(...pts.map(p => p.lng)), basis: 'the extent of the network and zones' };
    const saved = P && P.landuse && box(P.landuse.bounds);
    return saved ? { ...saved, basis: 'the area of the last fill' } : null;
  }
  const boxText = B => { const f = U.flat((B.s + B.n) / 2); return `${U.fmt.num((B.e - B.w) * f.kx / 1000, 1)} × ${U.fmt.num((B.n - B.s) * f.ky / 1000, 1)} km`; };
  const inBox = (B, lat, lng) => lat >= B.s && lat <= B.n && lng >= B.w && lng <= B.e;
  const boxKey = B => [B.s, B.w, B.n, B.e].map(v => v.toFixed(4)).join(',');
  // Largest-remainder split of an integer total by weights (sums exactly)
  function lr(total, weights) {
    const s = weights.reduce((a, b) => a + b, 0);
    if (!(s > 0) || !(total > 0)) return weights.map(() => 0);
    const raw = weights.map(w => total * w / s), out = raw.map(Math.floor);
    let left = total - out.reduce((a, b) => a + b, 0);
    raw.map((r, i) => [r - out[i], i]).sort((a, b) => b[0] - a[0] || a[1] - b[1]).forEach(([, i]) => { if (left > 0) { out[i]++; left--; } });
    return out;
  }

  /* ---------- geometry: ArcGIS rings are [lng, lat] ---------- */
  function bbOf(rings) {
    const bb = [Infinity, Infinity, -Infinity, -Infinity];
    for (const r of rings) for (const p of r) { bb[0] = Math.min(bb[0], p[0]); bb[1] = Math.min(bb[1], p[1]); bb[2] = Math.max(bb[2], p[0]); bb[3] = Math.max(bb[3], p[1]); }
    return bb;
  }
  function inRings(x, y, rings) {   // even-odd over every ring, so holes work
    let c = false;
    for (const r of rings) for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
      const xi = r[i][0], yi = r[i][1], xj = r[j][0], yj = r[j][1];
      if ((yi > y) !== (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) c = !c;
    }
    return c;
  }
  const inPoly = (f, lng, lat) => lng >= f.bb[0] && lng <= f.bb[2] && lat >= f.bb[1] && lat <= f.bb[3] && inRings(lng, lat, f.rings);
  const bbInBox = (bb, B) => bb[0] >= B.w && bb[2] <= B.e && bb[1] >= B.s && bb[3] <= B.n;
  const bbMeets = (bb, B) => bb[0] <= B.e && bb[2] >= B.w && bb[1] <= B.n && bb[3] >= B.s;
  function ringsArea(rings) {   // m², holes subtracted (ArcGIS winds holes the other way)
    if (!rings.length) return 0;
    const f = U.flat(rings[0][0][1]);
    let a = 0;
    for (const r of rings) for (let i = 0, n = r.length; i < n; i++) { const p = r[i], q = r[(i + 1) % n]; a += (p[0] * f.kx) * (q[1] * f.ky) - (q[0] * f.kx) * (p[1] * f.ky); }
    return Math.abs(a / 2);
  }
  // Sampled share of a polygon's area that lies inside box B, and the sample points inside both
  function sampleShare(f, B, n = 24) {
    const [w, s, e, nn] = f.bb, pts = [];
    let inside = 0;
    for (let j = 0; j < n; j++) for (let i = 0; i < n; i++) {
      const x = w + (e - w) * (i + 0.5) / n, y = s + (nn - s) * (j + 0.5) / n;
      if (!inRings(x, y, f.rings)) continue;
      inside++;
      if (inBox(B, y, x)) pts.push([x, y]);
    }
    return { share: inside ? pts.length / inside : (bbMeets(f.bb, B) ? 1 : 0), pts, inside };
  }

  /* ---------- fetching (rule 5: AbortController, cached per study area, fail soft) ---------- */
  // The timer runs until the body has been read, so a stalled answer also times out
  const REJECTED = 'the service rejected the query; its fields may have changed. Enter these values by hand for now';
  async function getJSON(url, label, timeout = TIMEOUT) {
    const host = new URL(url).host, ac = typeof AbortController !== 'undefined' ? new AbortController() : null, tm = ac ? setTimeout(() => ac.abort(), timeout) : null;
    const secs = `${Math.round(timeout / 1000)} s`, aborted = e => !!(e && e.name === 'AbortError');
    try {
      let r, j;
      try { r = await fetch(url, ac ? { signal: ac.signal } : undefined); }
      catch (e) { throw new Error(aborted(e) ? `${label}: ${host} did not answer within ${secs}. Try again later.` : `${label}: ${host} could not be reached (${(e && e.message) || e}). Check your connection and try again.`); }
      if (!r.ok) throw new Error(r.status >= 400 && r.status < 500 ? `${label}: ${host} answered HTTP ${r.status}: ${REJECTED}.` : `${label}: ${host} answered HTTP ${r.status}. Try again later.`);
      try { j = await r.json(); }
      catch (e) { throw new Error(aborted(e) ? `${label}: ${host} did not finish answering within ${secs}. Try again later.` : `${label}: ${host} sent an answer Spare Construct could not read. Try again later.`); }
      if (j && j.error) {
        const code = +j.error.code, msg = j.error.message || JSON.stringify(j.error);
        throw new Error(code >= 400 && code < 500 ? `${label}: ${msg}: ${REJECTED}.` : `${label}: ${msg}`);
      }
      return j;
    } finally { if (tm) clearTimeout(tm); }
  }
  async function arcQuery(url, B, outFields, label, geometry = true) {
    const out = [];
    for (let page = 0, offset = 0; page < 30; page++) {
      const q = new URLSearchParams({ where: '1=1', geometry: [B.w, B.s, B.e, B.n].map(v => v.toFixed(6)).join(','), geometryType: 'esriGeometryEnvelope', inSR: '4326',
        spatialRel: 'esriSpatialRelIntersects', outFields: outFields.join(','), returnGeometry: String(geometry), outSR: '4326', geometryPrecision: '6', f: 'json', resultOffset: String(offset) });
      const j = await getJSON(`${url}/query?${q}`, label);
      const f = j.features || [];
      out.push(...f);
      if (!j.exceededTransferLimit || !f.length) break;
      offset += f.length;
    }
    return out;
  }
  // A layer's ?f=json (fields, aliases, domains), then the features with only the fields pick() asks for
  async function arcLayer(url, B, pick, label, geometry = true) {
    const meta = await getJSON(url + '?f=json', label);
    const fields = pick(meta.fields || []);
    return { meta, features: await arcQuery(url, B, fields, label, geometry) };
  }
  // Results are cached per study area; failures and partial results (v.partial) are not kept, so a retry fetches again
  const cache = new Map();
  function cached(what, B, fn) {
    const k = what + '|' + boxKey(B);
    if (!cache.has(k)) cache.set(k, Promise.resolve().then(fn).then(v => { if (v && v.partial) cache.delete(k); return v; }, e => { cache.delete(k); throw e; }));
    return cache.get(k);
  }

  /* ---------- Census 2023 SA1 ---------- */
  const WORK_CATS = [['home', 0, 'Work at home'], ['notToday', 1, 'Did not go to work today'], ['drivePriv', 2, 'Drive a private car, truck or van'], ['driveCo', 3, 'Drive a company car, truck or van'],
    ['pass', 4, 'Passenger in a car, truck, van or company bus'], ['bus', 5, 'Public bus'], ['train', 6, 'Train'], ['bike', 7, 'Bicycle'], ['walk', 8, 'Walk or jog'], ['ferry', 9, 'Ferry'], ['other', 10, 'Other'], ['stated', 13, 'Total stated']];
  const EDU_CATS = [['home', 0, 'Study at home'], ['drive', 1, 'Drive a car, truck or van'], ['pass', 2, 'Passenger in a car, truck or van'], ['bike', 3, 'Bicycle'], ['walk', 4, 'Walk or jog'],
    ['schoolBus', 5, 'School bus'], ['bus', 6, 'Public bus'], ['train', 7, 'Train'], ['ferry', 8, 'Ferry'], ['other', 9, 'Other'], ['stated', 12, 'Total Stated']];
  const cats = (pfx, topic, base, list) => Object.fromEntries(list.map(([k, o, c]) => [pfx + '.' + k, { code: `VAR_2_${base + o}`, alias: `${topic} (${c})` }]));
  // Each item: the VAR code in the 2023 release and the alias text it is resolved by (Year: 2023 is required)
  const CENSUS_VARS = {
    ind0: { pop: { code: 'VAR_1_3', alias: 'Var1: Census usually resident population count (Total)' } },
    ind2: { empWork: { code: 'VAR_2_677', alias: 'Industry, by workplace address (Total)' },
      ...cats('r', 'Main means of travel to work, by usual residence address', 773, WORK_CATS),
      ...cats('w', 'Main means of travel to work, by workplace address', 815, WORK_CATS),
      ...cats('e', 'Main means of travel to education, by education address', 195, EDU_CATS) },
    dw: { occ: { code: 'VAR_3_54', alias: 'Dwelling occupancy status (Occupied Dwelling)' }, dwTot: { code: 'VAR_3_59', alias: 'Dwelling occupancy status (Total)' },
      sep: { code: 'VAR_3_82', alias: 'Dwelling type (Separate house)' }, joined: { code: 'VAR_3_83', alias: 'Dwelling type (Joined dwelling)' }, otherPriv: { code: 'VAR_3_84', alias: 'Dwelling type (Other private dwelling)' } },
    hh: { hh: { code: 'VAR_4_3', alias: 'Count of households in occupied private dwellings (Total)' },
      ...Object.fromEntries(['No motor vehicle', 'One motor vehicle', 'Two motor vehicles', 'Three motor vehicles', 'Four motor vehicles', 'Five or more motor vehicles'].map((c, i) => ['veh.' + i, { code: `VAR_4_${136 + i}`, alias: `Number of motor vehicles (${c})` }])),
      'veh.stated': { code: 'VAR_4_144', alias: 'Number of motor vehicles (Total stated)' } },
  };
  // Resolve each item to a field name by its alias; fall back to the known code, else null (reported)
  function resolve(fields, spec, year = 2023) {
    const low = s => String(s || '').toLowerCase(), out = {}, names = new Set((fields || []).map(f => f.name));
    for (const [k, v] of Object.entries(spec)) {
      const want = low(v.alias), f = (fields || []).find(f => low(f.alias).includes(`year: ${year}`) && low(f.alias).endsWith(want));
      out[k] = f ? { name: f.name, by: 'alias' } : names.has(v.code) || !(fields || []).length ? { name: v.code, by: 'code' } : { name: null, by: 'missing' };
    }
    return out;
  }
  const codeField = (fields, re) => ((fields || []).find(f => re.test(f.name)) || {}).name || null;
  const SA1_RE = /^SA1\d{4}_V\d+_00$/, SA2_RE = /^SA2\d{4}_V\d+_00$/;
  // Random rounding to base 3 keeps counts ≥ 0; negative values mark suppressed or unavailable cells
  const val = (a, name) => { if (!name || !a) return null; const v = a[name]; return v == null || !isFinite(+v) || +v < 0 ? null : +v; };
  function setP(o, k, v) { const p = k.split('.'); if (p.length === 1) o[k] = v; else { o[p[0]] ||= {}; o[p[0]][p[1]] = v; } }
  const ringsOf = g => (g && g.rings ? g.rings.filter(r => r && r.length > 2) : []);

  // layers: { ind0, ind2, dw, hh } each { meta: {fields}, features }. Returns { sa1: [{ code, rings, bb, v }], fields, missing }
  function parseCensus(layers) {
    const byCode = new Map(), fields = {}, missing = [];
    for (const [lk, spec] of Object.entries(CENSUS_VARS)) {
      const L = layers[lk]; if (!L) { missing.push(lk + ' layer'); continue; }
      const fl = (L.meta && L.meta.fields) || [], cf = codeField(fl, SA1_RE) || 'SA12023_V1_00', res = resolve(fl, spec);
      Object.assign(fields, res);
      for (const [k, r] of Object.entries(res)) if (!r.name) missing.push(k);
      for (const f of L.features || []) {
        const a = f.attributes || {}, code = String(a[cf] ?? ''); if (!code) continue;
        let o = byCode.get(code);
        if (!o) byCode.set(code, o = { code, rings: [], bb: null, v: {} });
        const rg = ringsOf(f.geometry);
        if (rg.length && !o.rings.length) { o.rings = rg; o.bb = bbOf(rg); }
        for (const [k, r] of Object.entries(res)) setP(o.v, k, val(a, r.name));
      }
    }
    return { sa1: [...byCode.values()].filter(o => o.rings.length), fields, missing };
  }
  function censusFields(lk) { return fl => [codeField(fl, SA1_RE) || 'SA12023_V1_00', ...Object.values(resolve(fl, CENSUS_VARS[lk])).map(r => r.name).filter(Boolean)]; }
  /* Each layer fails soft on its own: the layers that loaded are used and the others are reported (errors, missing).
     The SA1 outlines come with the first layer that loads (dwellings, else households). With neither, there are no
     SA1s to place, so the whole download fails. */
  async function fetchCensus(bounds, onStatus = () => {}) {
    const B = box(bounds);
    return cached('census', B, async () => {
      const L = SRC.census.layers, layers = {}, errors = [];
      let geo = false;
      for (const [k, label] of [['dw', 'dwellings'], ['hh', 'households'], ['ind0', 'population'], ['ind2', 'travel to work and education']]) {
        onStatus(`Downloading Census 2023 ${label} for each SA1…`);
        const wantGeo = !geo && (k === 'dw' || k === 'hh');
        try { layers[k] = await arcLayer(L[k], B, censusFields(k), 'Census 2023 ' + label, wantGeo); if (wantGeo) geo = true; }
        catch (e) { errors.push(e.message); }
      }
      if (!geo) throw new Error(errors.join(' '));
      return { ...parseCensus(layers), errors, partial: errors.length > 0 };
    });
  }

  /* ---------- employees by SA2 ---------- */
  function parseEmployees(L) {
    const fl = (L.meta && L.meta.fields) || [], cf = codeField(fl, SA2_RE) || 'SA22023_V1_00', nf = fl.some(f => f.name === cf + '_NAME') ? cf + '_NAME' : null;
    const years = fl.map(f => /^ec(\d{4})$/.exec(f.name)).filter(Boolean).map(m => +m[1]);
    const year = years.length ? Math.max(...years) : 2025, ef = 'ec' + year, gf = fl.some(f => f.name === 'ec_avperinc') || !fl.length ? 'ec_avperinc' : null;
    const sa2 = (L.features || []).map(f => { const a = f.attributes || {}, rg = ringsOf(f.geometry);
      return { code: String(a[cf] ?? ''), name: (nf && a[nf]) || String(a[cf] ?? ''), n: val(a, ef), year, growth: gf && a[gf] != null && isFinite(+a[gf]) ? +a[gf] : null, rings: rg, bb: rg.length ? bbOf(rg) : null };
    }).filter(a => a.code && a.rings.length);
    return { sa2, year, field: ef };
  }
  async function fetchEmployees(bounds, onStatus = () => {}) {
    const B = box(bounds);
    return cached('employees', B, async () => {
      onStatus('Downloading Stats NZ employee counts for each SA2…');
      return parseEmployees(await arcLayer(SRC.employees.url, B, fl => {
        const cf = codeField(fl, SA2_RE) || 'SA22023_V1_00', ec = fl.filter(f => /^ec\d{4}$/.test(f.name) || f.name === 'ec_avperinc').map(f => f.name);
        return [cf, cf + '_NAME', ...(ec.length ? ec : ['ec2025', 'ec_avperinc'])].filter((x, i, a) => a.indexOf(x) === i && (fl.some(f => f.name === x) || !fl.length));
      }, 'Stats NZ employees'));
    });
  }

  /* ---------- Ministry of Education directory (CKAN datastore, by resource id) ---------- */
  const MOE_FIELDS = { schools: ['School_Id', 'Org_Name', 'Org_Type', 'Total', 'Roll_Date', 'Latitude', 'Longitude', 'Status'], ece: ['ECE_Id', 'Org_Name', 'Org_Type', 'All_Children', 'Total', 'Roll_Date', 'Latitude', 'Longitude'] };
  function parseMoE(records, kind) {
    const num = v => (v == null || v === '' || !isFinite(+v) ? null : +v);
    return (records || []).map(r => {
      const name = String(r.Org_Name || '').trim(), m = /\(Proposed opening date:\s*(\d{4})-(\d\d)-(\d\d)\)/i.exec(name);
      return { id: String(r.School_Id || r.ECE_Id || name), kind, name: name.replace(/\s*\(Proposed opening date:[^)]*\)/i, '').trim(), type: r.Org_Type || '',
        roll: num(r.Total), places: kind === 'ece' ? num(r.All_Children) : null, rollDate: String(r.Roll_Date || '').slice(0, 10) || null,
        lat: num(r.Latitude), lng: num(r.Longitude), status: r.Status || (m ? 'Proposed' : 'Open'), opens: m ? +m[1] : null };
    }).filter(r => r.lat != null && r.lng != null && !/^closed$/i.test(r.status));
  }
  async function fetchMoE(kind, bounds, onStatus = () => {}) {
    const B = box(bounds), s = SRC[kind];
    return cached('moe-' + kind, B, async () => {
      onStatus(`Downloading the Ministry of Education ${kind === 'ece' ? 'early learning' : 'school'} directory…`);
      const sql = `SELECT ${MOE_FIELDS[kind].map(f => `"${f}"`).join(',')} FROM "${s.resource}" WHERE "Latitude" BETWEEN ${B.s.toFixed(6)} AND ${B.n.toFixed(6)} AND "Longitude" BETWEEN ${B.w.toFixed(6)} AND ${B.e.toFixed(6)}`;
      const j = await getJSON(`${CKAN}?sql=${encodeURIComponent(sql)}`, 'Ministry of Education directory');
      if (!j.success || !j.result) throw new Error('Ministry of Education directory: the search failed. Its fields may have changed; try again later.');
      return parseMoE(j.result.records, kind);
    });
  }

  /* ---------- Auckland Unitary Plan base zones ---------- */
  function parseAUP(L) {
    const fl = (L.meta && L.meta.fields) || [], dom = n => { const f = fl.find(f => f.name === n); return Object.fromEntries(((f && f.domain && f.domain.codedValues) || []).map(c => [c.code, c.name])); };
    const zn = dom('ZONE'), gn = dom('GROUPZONE'), vs = dom('VERSIONSTATUS');
    const zones = (L.features || []).map(f => { const a = f.attributes || {}, rg = ringsOf(f.geometry), code = +a.ZONE;
      return { code, name: zn[code] || AUP_NAMES[code] || `AUP zone ${code}`, group: gn[a.GROUPZONE] || '', status: vs[a.VERSIONSTATUS] || '', rings: rg, bb: rg.length ? bbOf(rg) : null };
    }).filter(z => z.rings.length && isFinite(z.code));
    return { zones, names: zn };
  }
  async function fetchAUP(bounds, onStatus = () => {}) {
    const B = box(bounds);
    return cached('aup', B, async () => {
      onStatus('Downloading Auckland Unitary Plan zones…');
      return parseAUP(await arcLayer(SRC.aup.url, B, fl => ['ZONE', 'GROUPZONE', 'VERSIONSTATUS'].filter(n => fl.some(f => f.name === n) || !fl.length), 'Auckland Unitary Plan zones'));
    });
  }

  /* ---------- allocation to zones ---------- */
  const RES_BLD = new Set(['yes', 'house', 'detached', 'semidetached_house', 'terrace', 'residential', 'apartments', 'bungalow', 'static_caravan', 'dormitory']);
  const NONRES_LU = new Set(['industrial', 'commercial', 'retail', 'education', 'farmyard', 'cemetery', 'military']);
  // Residential-capable building outlines (40–1500 m², not in a non-residential land-use area), weighted by footprint × levels
  function prepBuildings(els, B) {
    const out = [], areas = [];
    if (!Array.isArray(els) || !els.length) return out;
    const lls = g => (g || []).filter(p => p && isFinite(p.lat) && isFinite(p.lon)).map(p => [p.lon, p.lat]);
    const ringsOfEl = e => e.type === 'way' ? [lls(e.geometry)] : e.type === 'relation' ? (e.members || []).filter(m => m.type === 'way' && m.role !== 'inner').map(m => lls(m.geometry)) : [];
    for (const e of els) { const t = e.tags || {}; if (!t.building && NONRES_LU.has(t.landuse)) { const rg = ringsOfEl(e).filter(r => r.length > 3); if (rg.length) areas.push({ rings: rg, bb: bbOf(rg) }); } }
    for (const e of els) {
      const t = e.tags || {}; if (!t.building || !RES_BLD.has(t.building) || e.type !== 'way') continue;
      const r = lls(e.geometry); if (r.length < 4) continue;
      const a = ringsArea([r]); if (a < 40 || a > 1500) continue;
      const lng = r.reduce((s, p) => s + p[0], 0) / r.length, lat = r.reduce((s, p) => s + p[1], 0) / r.length;
      if (!inBox(B, lat, lng) || areas.some(A => inPoly(A, lng, lat))) continue;
      const lv = Math.max(1, Math.min(30, Math.round(+t['building:levels'] || (t.building === 'apartments' ? 3 : 1))));
      out.push({ lat, lng, w: a * lv });
    }
    return out;
  }
  const landZones = (sc, B) => Object.values(sc.zones).filter(z => z.kind !== 'gateway' && isFinite(z.lat) && isFinite(z.lng) && (!B || inBox(B, z.lat, z.lng)));
  const prov = (src, extra = {}) => ({ source: src.name, licence: src.licence, date: today(), basis: 'derived', ...extra });
  const isCensus = s => !!(s && String(s.source || '').startsWith(SRC.census.name));
  function nearestRoad(sc, lat, lng) {
    const f = U.flat(lat);
    let best = null, bd = 800;
    for (const Lk of Object.values(sc.links || {})) {
      if (!Lk.name || !Lk.pts || Lk.pts.length < 2) continue;
      for (let i = 1; i < Lk.pts.length; i++) {
        const a = Lk.pts[i - 1], b = Lk.pts[i], d = U.segDist(lng * f.kx, lat * f.ky, a[1] * f.kx, a[0] * f.ky, b[1] * f.kx, b[0] * f.ky).d;
        if (d < bd) { bd = d; best = Lk.name; }
      }
    }
    return best;
  }
  function addCounts(dst, src, k) {   // dst += src × k for nested count objects
    for (const [key, v] of Object.entries(src || {})) {
      if (v && typeof v === 'object') addCounts(dst[key] ||= {}, v, k);
      else if (v != null && isFinite(v)) dst[key] = (dst[key] || 0) + v * k;
    }
  }
  const roundCounts = o => { for (const [k, v] of Object.entries(o)) o[k] = v && typeof v === 'object' ? roundCounts(v) : U.r1(v); return o; };

  /* Census dwellings and attributes to existing zones. Inside an SA1 the dwellings follow residential building
     footprints (each building goes to its nearest existing zone); an SA1 that crosses the study-area edge is
     first split by area overlap; an SA1 with no buildings mapped is split by area. opts.newer: 'flag' (default)
     lists zones where OSM shows clearly more dwellings than the Census; 'osm' keeps the OSM count there. */
  function censusToZones(sc, C, blds, B, opts = {}) {
    const zones = landZones(sc, B).filter(isExisting), f = U.flat((B.s + B.n) / 2);
    // Without the dwellings layer the dwellings stay as they are; people, households, vehicles and travel still fill
    const dwOk = !(C.missing || []).includes('dw layer');
    const nearest = (lat, lng) => { let best = null, bd = Infinity; for (const z of zones) { const d = Math.hypot((z.lng - lng) * f.kx, (z.lat - lat) * f.ky); if (d < bd) { bd = d; best = z; } } return [best, bd]; };
    const buckets = new Map(), rows = [], warnings = [];
    const bucket = (k, z) => { let b = buckets.get(k); if (!b) buckets.set(k, b = { z, res: 0, apt: 0, sa1: [], attrs: {}, sx: 0, sy: 0, sw: 0 }); return b; };
    let cum = 0, placedSum = 0;
    for (const a of C.sa1) {
      if (!bbMeets(a.bb, B)) continue;
      const within = bbInBox(a.bb, B), smp = within ? null : sampleShare(a, B), inShare = within ? 1 : smp.share;
      if (!(inShare > 0)) continue;
      const bl = blds.filter(b => inPoly(a, b.lng, b.lat));
      let pts, basis;
      if (bl.length) { pts = bl.map(b => [b.lat, b.lng, b.w]); basis = within ? 'building footprints' : 'area overlap at the study-area edge, then building footprints'; }
      else { const sp = smp || sampleShare(a, B); pts = sp.pts.map(p => [p[1], p[0], 1]); basis = 'area overlap (no residential buildings mapped)'; }
      if (!pts.length) continue;
      const w = new Map();
      for (const [lat, lng, wt] of pts) {
        const [z, d] = nearest(lat, lng), key = z && !(opts.create !== false && d > STD.createM) ? z.id : 'new:' + a.code;
        const b = bucket(key, key.startsWith('new:') ? null : z); b.sx += lng * wt; b.sy += lat * wt; b.sw += wt;
        w.set(key, (w.get(key) || 0) + wt);
      }
      let occ = a.v.occ, occBasis = 'occupied private dwellings';
      if (!dwOk) { occ = 0; occBasis = 'dwellings layer not downloaded'; }
      if (occ == null && a.v.hh != null) { occ = a.v.hh; occBasis = 'households (occupied dwellings suppressed)'; }
      if (occ == null) { occ = 0; warnings.push(`SA1 ${a.code}: dwelling and household counts are suppressed, so it adds no dwellings.`); occBasis = 'suppressed'; }
      const target = occ * inShare; cum += target;
      const T = Math.max(0, Math.round(cum) - placedSum); placedSum += T;   // cumulative rounding keeps the total within 0.5
      const typed = (a.v.sep || 0) + (a.v.joined || 0) + (a.v.otherPriv || 0), Apt = Math.round(T * (typed > 0 ? (a.v.joined || 0) / typed : 0)), Hs = T - Apt;
      const keys = [...w.keys()], ws = keys.map(k => w.get(k)), tot = ws.reduce((s, x) => s + x, 0);
      const hs = lr(Hs, ws), ap = lr(Apt, ws);
      keys.forEach((k, i) => { const b = buckets.get(k), sh = ws[i] / tot * inShare; b.res += hs[i]; b.apt += ap[i]; b.sa1.push({ code: a.code, share: U.r3(sh) }); addCounts(b.attrs, a.v, sh); });
      rows.push({ code: a.code, occ: a.v.occ, used: occ, occBasis, inShare: U.r3(inShare), target: U.r1(target), placed: T, houses: Hs, apartments: Apt, basis, zones: keys.length });
    }
    // Zones for Census dwellings far from any existing zone (or when there are none)
    for (const [k, b] of buckets) {
      if (b.z || !(b.res + b.apt > 0)) continue;
      const lat = b.sy / b.sw, lng = b.sx / b.sw, id = U.uid('z'), road = nearestRoad(sc, lat, lng);
      b.z = sc.zones[id] = { id, kind: 'zone', existing: true, src: 'census', name: `Existing · ${road || 'SA1 ' + k.slice(4)}`, lat: U.r7(lat), lng: U.r7(lng), lu: {}, connectors: null };
      zones.push(b.z);
    }
    const flagged = [], when = today();
    let osmTotal = 0, censusTotal = 0;
    for (const z of zones) {
      const b = [...buckets.values()].find(x => x.z === z) || { res: 0, apt: 0, sa1: [], attrs: {} };
      z.lu ||= {}; z.luSrc ||= {};
      const osm = isCensus(z.luSrc.residential) && z.osm ? z.osm : { residential: +z.lu.residential || 0, apartment: +z.lu.apartment || 0 };
      const oT = (osm.residential || 0) + (osm.apartment || 0), cT = b.res + b.apt;
      osmTotal += oT; censusTotal += cT;
      if (dwOk) {
        const newer = oT > cT * (1 + STD.census.newer) + STD.census.newerAbs, keep = newer && opts.newer === 'osm';
        if (newer) flagged.push({ id: z.id, name: z.name, osm: oT, census: cT, kept: keep });
        if (oT && !z.osm) z.osm = { residential: osm.residential || 0, apartment: osm.apartment || 0 };
        if (!keep) for (const [k, q] of [['residential', b.res], ['apartment', b.apt]]) {
          if (q > 0) z.lu[k] = q; else delete z.lu[k];
          z.luSrc[k] = prov(SRC.census, { q, vintage: SRC.census.vintage, note: `Census occupied dwellings in SA1 ${b.sa1.map(s => s.code).join(', ') || '–'}, placed by ${b.sa1.length ? 'building footprint or area' : 'none'}; random-rounded to base 3` });
        } else z.luSrc.residential = { source: SRC.osm.name, licence: SRC.osm.licence, basis: 'estimated', date: when, q: z.lu.residential, note: 'Kept: OSM shows more dwellings than the 2023 Census (likely built since March 2023)' };
      }
      const v = roundCounts(b.attrs), veh = v.veh || {};
      const vehicles = [0, 1, 2, 3, 4, 5].reduce((s, i) => s + i * (veh[i] || 0), 0);
      z.census = { at: when, source: SRC.census.name, licence: SRC.census.licence, basis: 'derived', vintage: SRC.census.vintage, rounding: SRC.census.rounding,
        sa1: b.sa1, dwellings: dwOk ? cT : null, pop: v.pop ?? null, hh: v.hh ?? null, empWork: v.empWork ?? null,
        vehicles: U.r1(vehicles), vehStated: veh.stated ?? null, noCar: veh[0] ?? null, modes: { r: v.r || null, w: v.w || null, e: v.e || null } };
    }
    // Location-only mode shares for the other land-use zones (development, committed): the SA1 they stand in
    for (const z of landZones(sc, B).filter(z => !isExisting(z))) {
      const a = C.sa1.find(a => inPoly(a, z.lng, z.lat));
      if (a) z.censusLoc = { at: when, source: SRC.census.name, licence: SRC.census.licence, basis: 'derived', vintage: SRC.census.vintage, rounding: SRC.census.rounding,
        sa1: a.code, modes: { r: a.v.r || null, w: a.v.w || null, e: a.v.e || null } };
    }
    const expected = rows.reduce((s, r) => s + r.used * r.inShare, 0);
    if (!dwOk) warnings.push('The Census dwellings layer did not download, so dwellings are left as they were; people, households, vehicles and travel were filled from the layers that loaded.');
    return { rows, zones: zones.length, created: [...buckets.keys()].filter(k => k.startsWith('new:') && buckets.get(k).z).length, expected: U.r1(expected), placed: placedSum, dwellings: dwOk,
      censusTotal, osmTotal, flagged, warnings, sa1Total: rows.reduce((s, r) => s + (r.occ || 0), 0) };
  }

  /* Employees: each SA2 total is a hard control. The part inside the study area (area overlap when the SA2 crosses
     the edge) goes to existing zones in the SA2 by commercial and industrial floor area; the rest is reported as
     outside, or unplaced when no floor area is mapped. allocated + outside + unplaced = the SA2 count. */
  function employeesToZones(sc, E, B) {
    const zones = landZones(sc, B).filter(isExisting), rows = [], when = today();
    for (const z of zones) delete z.jobs;
    const gfa = z => JOB_KEYS.reduce((s, k) => s + (+((z.lu || {})[k]) || 0), 0);
    let allocated = 0, growthW = 0, growthS = 0;
    for (const a of E.sa2) {
      if (!(a.n > 0) || !bbMeets(a.bb, B)) continue;
      const within = bbInBox(a.bb, B), inShare = within ? 1 : sampleShare(a, B).share;
      if (!(inShare > 0)) continue;
      const inN = Math.round(a.n * inShare), zs = zones.filter(z => inPoly(a, z.lng, z.lat) && gfa(z) > 0);
      const parts = lr(inN, zs.map(gfa));
      zs.forEach((z, i) => { if (!parts[i]) return; z.jobs ||= { n: 0, by: [], year: E.year, source: SRC.employees.name, licence: SRC.employees.licence, basis: 'derived', date: when, note: 'SA2 employees placed by commercial and industrial floor area' };
        z.jobs.n += parts[i]; z.jobs.by.push({ sa2: a.code, n: parts[i] }); });
      const placed = zs.length ? inN : 0;
      allocated += placed;
      if (a.growth != null) { growthW += a.n * inShare; growthS += a.n * inShare * a.growth; }
      rows.push({ code: a.code, name: a.name, n: a.n, year: E.year, inShare: U.r3(inShare), allocated: placed, outside: a.n - inN, unplaced: inN - placed, growth: a.growth,
        basis: within ? 'inside the study area' : 'area overlap at the study-area edge' });
    }
    const census = zones.reduce((s, z) => s + ((z.census && z.census.empWork) || 0), 0), ratio = census > 0 ? allocated / census : null;
    return { rows, year: E.year, allocated, census: U.r1(census), ratio: ratio == null ? null : U.r3(ratio), growth: growthW > 0 ? U.r1(growthS / growthW) : null,
      check: ratio == null ? 'No Census 2023 workplace counts to compare with.' : ratio < STD.jobsCheck[0] || ratio > STD.jobsCheck[1]
        ? `Check: ${E.year} employees are ${U.fmt.num(ratio, 2)} × the Census 2023 workplace count (Spare Construct band ${STD.jobsCheck.join('–')}). The floor area from OSM may be incomplete.`
        : `${E.year} employees are ${U.fmt.num(ratio, 2)} × the Census 2023 workplace count, within the Spare Construct band ${STD.jobsCheck.join('–')}.` };
  }

  /* Schools become zones of their own with the MoE roll (prompting for rolls the directory lacks: opts.rolls);
     early learning places go to the nearest existing zone within 300 m, else a zone of their own. The school and
     early-learning numbers in EXISTING (base-year) zones are replaced; development and committed zones are the
     proposal and are never touched. opts.replace: false skips the replacement (entering rolls later). Proposed
     schools open in their stated year. */
  const isExisting = z => z.kind !== 'gateway' && z.existing === true && Engine.zoneClass(z) === 'existing';
  const fromMoE = s => String((s || {}).source || '').startsWith('Ministry');
  // Take land use k out of zone z, keeping what it was: z.osm for OpenStreetMap estimates, z.replaced otherwise
  function replaceLu(z, k, by, when) {
    const q = +z.lu[k], pr = luProv(z, k);
    if (pr.basis === 'estimated' || pr.source === SRC.osm.name) (z.osm ||= {})[k] = q;
    else (z.replaced ||= {})[k] = { q, source: pr.source || 'entered in Spare Construct', basis: pr.basis || 'user', by };
    delete z.lu[k];
    (z.luSrc ||= {})[k] = { source: `Replaced by the ${by}`, basis: 'derived', date: when, q: 0 };
    return q;
  }
  function schoolsToZones(sc, schools, ece, B, opts = {}) {
    const rolls = opts.rolls || {}, when = today(), zones = landZones(sc, B).filter(isExisting), replace = opts.replace !== false, by = 'Ministry of Education directory';
    const out = { created: 0, updated: 0, needsRoll: [], replaced: { school: 0, childcare: 0 }, schools: [], ece: 0, skipped: [], loaded: { schools: !!opts.schoolsLoaded || !!(schools && schools.length), ece: !!opts.eceLoaded || !!(ece && ece.length) } };
    const moeZone = id => Object.values(sc.zones).find(z => z.moe && z.moe.id === id);
    if (replace && schools && schools.length) for (const z of zones) if (!z.moe && +((z.lu || {}).school) > 0 && !fromMoE((z.luSrc || {}).school)) out.replaced.school += replaceLu(z, 'school', by, when);
    for (const s of schools || []) {
      if (!inBox(B, s.lat, s.lng)) continue;
      const roll = s.roll != null ? s.roll : rolls[s.id] != null && isFinite(+rolls[s.id]) ? +rolls[s.id] : null;
      // the full record is saved, so the roll can be entered after the project is reopened
      if (roll == null) { out.needsRoll.push({ id: s.id, name: s.name, type: s.type, status: s.status, opens: s.opens || null, rollDate: s.rollDate || null, lat: s.lat, lng: s.lng }); continue; }
      let z = moeZone(s.id);
      const p = prov(SRC.schools, { basis: s.roll != null ? 'measured' : 'user', q: roll, vintage: s.rollDate ? `roll at ${s.rollDate}` : 'no roll date', note: s.roll != null ? `MoE roll, ${s.type}` : 'Roll entered by the user (the MoE directory has none)' });
      if (!z) { const id = U.uid('z'); z = sc.zones[id] = { id, kind: 'zone', name: s.name, lat: U.r7(s.lat), lng: U.r7(s.lng), lu: {}, connectors: null, src: 'moe' }; out.created++; } else out.updated++;
      z.lu = { ...(z.lu || {}), school: roll }; (z.luSrc ||= {}).school = p;
      z.moe = { id: s.id, type: s.type, rollDate: s.rollDate, status: s.status, source: SRC.schools.name, licence: SRC.schools.licence, date: when };
      if (s.opens) { z.year = s.opens; z.role = 'committed'; z.existing = false; } else z.existing = true;
      out.schools.push({ id: s.id, name: s.name, type: s.type, roll, rollDate: s.rollDate, opens: s.opens || null, entered: s.roll == null });
    }
    if (ece && ece.length) {
      // places from an earlier fill come out first (unless changed by hand since)
      for (const z of Object.values(sc.zones)) if (z.moeEce) { delete z.moeEce; if (z.lu && fromMoE(luProv(z, 'childcare'))) { delete z.lu.childcare; delete z.luSrc.childcare; } }
      if (replace) for (const z of zones) if (+((z.lu || {}).childcare) > 0 && !fromMoE(luProv(z, 'childcare'))) out.replaced.childcare += replaceLu(z, 'childcare', by, when);
      const f = U.flat((B.s + B.n) / 2), cands = Object.values(sc.zones).filter(z => isExisting(z) && !z.moe);
      for (const c of ece) {
        if (!inBox(B, c.lat, c.lng)) continue;
        if (/home.?based/i.test(c.type)) { out.skipped.push(`${c.name} (home-based network)`); continue; }
        const q = c.places != null ? c.places : c.roll;
        if (!(q > 0)) { out.skipped.push(`${c.name} (no places or roll in the directory)`); continue; }
        let z = null, bd = 300;
        for (const x of cands) { const d = Math.hypot((x.lng - c.lng) * f.kx, (x.lat - c.lat) * f.ky); if (d < bd) { bd = d; z = x; } }
        if (!z) { const id = U.uid('z'); z = sc.zones[id] = { id, kind: 'zone', existing: true, src: 'moe', name: c.name, lat: U.r7(c.lat), lng: U.r7(c.lng), lu: {}, connectors: null }; cands.push(z); out.created++; }
        z.lu ||= {}; z.lu.childcare = (+z.lu.childcare || 0) + q;
        (z.moeEce ||= []).push(c.id);
        (z.luSrc ||= {}).childcare = prov(SRC.ece, { basis: 'measured', q: z.lu.childcare, vintage: c.rollDate ? `roll at ${c.rollDate}` : '', note: `MoE licensed places (${z.moeEce.length} service${z.moeEce.length > 1 ? 's' : ''})` });
        out.ece++;
      }
    }
    return out;
  }
  // Zone centres often sit on road reserve: look past Road, Water, Strategic Transport Spare Construct and coastal marine
  // polygons to the nearest land-use zone within 80 m (8 directions at 20, 40 and 80 m)
  const NOT_LAND = new Set([25, 26, 27, 30]);
  function aupAt(A, lat, lng) {
    const at = (la, ln) => A.zones.find(f => inPoly(f, ln, la)), here = at(lat, lng);
    if (here && !NOT_LAND.has(here.code)) return { f: here, near: 0 };
    const k = U.flat(lat);
    for (const m of [20, 40, 80]) for (let i = 0; i < 8; i++) {
      const b = i * Math.PI / 4, f = at(lat + m * Math.cos(b) / k.ky, lng + m * Math.sin(b) / k.kx);
      if (f && !NOT_LAND.has(f.code)) return { f, near: m };
    }
    return here ? { f: here, near: 0 } : null;
  }
  function aupToZones(sc, A, B) {
    let n = 0;
    for (const z of landZones(sc, B)) {
      const hit = aupAt(A, z.lat, z.lng), f = hit && hit.f;
      if (f) { z.aup = { code: f.code, name: f.name, group: f.group, status: f.status, date: today(), source: SRC.aup.name, licence: SRC.aup.licence, basis: 'derived', ...(hit.near ? { near: hit.near, note: `Nearest zone within ${hit.near} m: the zone centre is on road reserve or water` } : {}) }; n++; }
    }
    // zoned land inside the study area by zone, for the zoned-capacity table (the polygons are not saved)
    const by = new Map();
    for (const f of A.zones) {
      if (!bbMeets(f.bb, B)) continue;
      const sh = bbInBox(f.bb, B) ? 1 : sampleShare(f, B, 16).share; if (!(sh > 0)) continue;
      const r = by.get(f.code) || { code: f.code, name: f.name, group: f.group, ha: 0, n: 0 };
      r.ha += ringsArea(f.rings) * sh / 1e4; r.n++; by.set(f.code, r);
    }
    return { zones: n, summary: [...by.values()].map(r => ({ ...r, ha: U.r1(r.ha) })).sort((a, b) => b.ha - a.ha) };
  }

  /* Apply downloaded data to a scenario (pure: no store events). data: { census, employees, schools, ece, aup, elements }.
     opts: { bounds, newer: 'flag' | 'osm', create, rolls }. Returns the report the tab and the report chapter show. */
  function apply(sc, data, opts = {}) {
    const given = box(opts.bounds), B = given ? { ...given, basis: (opts.bounds && opts.bounds.basis) || 'the area given' } : areaBox(sc);
    if (!B) throw new Error('Add roads or zones first, so Spare Construct knows the study area.');
    const out = { at: today(), bounds: B, warnings: [], census: null, employees: null, schools: null, aup: null };
    const blds = prepBuildings(data.elements, B);
    if (data.census) { out.census = censusToZones(sc, data.census, blds, B, opts); out.census.buildings = blds.length; out.warnings.push(...out.census.warnings); if (data.census.missing && data.census.missing.length) out.warnings.push(`Census fields not found: ${data.census.missing.join(', ')}. Those values are left blank.`); }
    if (data.employees) out.employees = employeesToZones(sc, data.employees, B);
    if (data.schools || data.ece) out.schools = schoolsToZones(sc, data.schools || [], data.ece || [], B, { ...opts, schoolsLoaded: !!data.schools, eceLoaded: !!data.ece });
    if (data.aup) out.aup = aupToZones(sc, data.aup, B);
    out.choices = choices(out);
    return out;
  }
  function choices(o) {
    const c = o.census, e = o.employees, s = o.schools;
    const val = { dwellings: c && c.dwellings !== false ? `Census ${n0(c.placed)} (OSM ${n0(c.osmTotal)})` : null, people: c ? `${n0(c.rows.length)} SA1s` : null,
      jobs: e ? `${n0(e.allocated)} employees (Census workplace ${n0(e.census)})` : null,
      schools: s && s.loaded.schools ? `${s.schools.length} schools (rolls replaced in existing zones: ${n0(s.replaced.school)} students)` : null,
      ece: s && s.loaded.ece ? `${s.ece} services (places replaced in existing zones: ${n0(s.replaced.childcare)})` : null, gfa: 'OSM', zoning: o.aup ? `${o.aup.zones} zones matched` : null };
    return CHOICES.map(ch => ({ ...ch, used: val[ch.id] != null, value: val[ch.id] }));
  }

  /* Download and apply everything for the active scenario (or every scenario with opts.all). The UI wraps this.
     Each source fails soft on its own (schools and early learning separately). opts.beforeApply(targets) runs after
     the downloads and before any zone changes (the UI takes its undo snapshots there). Every changed scenario's
     results are marked out of date. */
  async function fillAll(sc = Store.sc, opts = {}, onStatus = () => {}) {
    const given = box(opts.bounds), B = given ? { ...given, basis: 'the area given' } : areaBox(sc);
    if (!B) throw new Error('Add roads or zones first, so Spare Construct knows the study area.');
    const want = { osm: true, census: true, employees: true, schools: true, aup: true, ...(opts.which || {}) }, data = {}, warnings = [];
    const step = async (k, fn, as = k) => { if (!want[k]) return; try { data[as] = await fn(); } catch (e) { warnings.push(e.message); } };
    // opts.elements: OpenStreetMap elements the caller already downloaded (the study-area set-up), so they are not fetched twice
    if (Array.isArray(opts.elements)) data.elements = opts.elements;
    else if (typeof ExistingLU !== 'undefined') await step('osm', () => cached('osm', B, () => ExistingLU.fetch(B, onStatus)));
    if (data.osm) { data.elements = data.osm; delete data.osm; }
    await step('census', () => fetchCensus(B, onStatus));
    if (data.census && data.census.errors) warnings.push(...data.census.errors);
    await step('employees', () => fetchEmployees(B, onStatus));
    await step('schools', () => fetchMoE('schools', B, onStatus));
    await step('schools', () => fetchMoE('ece', B, onStatus), 'ece');
    await step('aup', () => fetchAUP(B, onStatus));
    const targets = opts.all ? Store.P.scenarios.slice() : [sc];
    if (typeof opts.beforeApply === 'function') opts.beforeApply(targets);
    let out = null, built = 0;
    for (const s of targets) {
      // No existing zones yet: build them from OpenStreetMap first, as the study-area set-up does
      if (data.elements && typeof ExistingLU !== 'undefined' && !landZones(s, B).some(isExisting)) {
        const r = ExistingLU.build(data.elements, B, { links: s.links });
        for (const z of r.zones) { z.src = 'existing-lu'; s.zones[z.id] = z; }
        if (s === sc) built = r.zones.length;
      }
      const o = apply(s, data, { ...opts, bounds: B });
      for (const r of Object.values(s.res || {})) if (r) r.stale = true;
      if (s === sc) out = o;
    }
    out.warnings.unshift(...warnings);
    out.builtFromOSM = built;
    out.scenarios = targets.length;
    lastData = { data, B };
    record(out);
    return out;
  }
  let lastData = null;
  /* Rolls for schools the directory lacks: add those schools with the user's numbers. The schools are the records
     saved with this project (P.landuse.last.schools.needsRoll, with their location), so this also works after the
     project is reopened; this session's download fills in older records without a location. No other zone's roll
     is replaced here. Returns null when there is nothing it can apply (rollsReady(...).why says why). */
  function rollsReady(rolls = null, P = Store.P) {
    const last = P && P.landuse && P.landuse.last, saved = (last && last.schools && last.schools.needsRoll) || [], B = box(P && P.landuse && P.landuse.bounds);
    const run = lastData && lastData.data.schools && B && boxKey(lastData.B) === boxKey(B) ? new Map(lastData.data.schools.map(s => [s.id, s])) : new Map();
    const list = saved.filter(s => s && (!rolls || rolls[s.id] != null))
      .map(s => (isFinite(+s.lat) && isFinite(+s.lng) && s.lat !== null && s.lng !== null ? { ...s, lat: +s.lat, lng: +s.lng } : run.get(s.id))).filter(Boolean).map(s => ({ ...s, roll: null }));
    if (list.length && B) return { list, B };
    return { list: [], B, why: !saved.length ? 'No schools are waiting for a roll. Fill existing land use again if the directory has changed.'
      : 'This project was filled before school locations were saved with it. Fill existing land use again, then enter the rolls.' };
  }
  // sc: a scenario, or the list of scenarios the last fill covered (each gets the schools; the first one's report is returned)
  function applyRolls(sc, rolls) {
    const R = rollsReady(rolls), targets = Array.isArray(sc) ? sc : [sc];
    if (!R.list.length || !targets.length) return null;
    let r = null;
    for (const s of targets) {
      const o = schoolsToZones(s, R.list, [], R.B, { rolls, replace: false });
      if (!r) r = o;
      for (const x of Object.values(s.res || {})) if (x) x.stale = true;
    }
    if (Store.P.landuse && Store.P.landuse.last && Store.P.landuse.last.schools) {
      const s = Store.P.landuse.last.schools;
      s.needsRoll = (s.needsRoll || []).filter(x => !r.schools.some(y => y.id === x.id)); (s.schools ||= []).push(...r.schools); s.created = (+s.created || 0) + r.created;
    }
    r.scenarios = targets.length;
    return r;
  }
  // Save a compact summary (no polygons) with provenance in the project
  function record(out) {
    const P = Store.P, L = P.landuse ||= {};
    L.bounds = out.bounds;
    L.last = JSON.parse(JSON.stringify({ at: out.at, bounds: out.bounds, census: out.census, employees: out.employees, schools: out.schools, aup: out.aup && { zones: out.aup.zones }, choices: out.choices, warnings: out.warnings, builtFromOSM: out.builtFromOSM || 0, scenarios: out.scenarios || 1 }));
    if (out.aup) L.aupSummary = out.aup.summary;
    const src = L.sources ||= {};
    if (out.census) src.census = { name: SRC.census.name, vintage: SRC.census.vintage, licence: SRC.census.licence, at: out.at, n: out.census.rows.length };
    if (out.employees) src.employees = { name: SRC.employees.name, vintage: `February ${out.employees.year}`, licence: SRC.employees.licence, at: out.at, n: out.employees.rows.length };
    if (out.schools) src.moe = { name: 'Ministry of Education, Directory of Educational Institutions', licence: SRC.schools.licence, at: out.at, n: out.schools.schools.length + out.schools.ece };
    if (out.aup) src.aup = { name: SRC.aup.name, licence: SRC.aup.licence, at: out.at, n: out.aup.summary.length };
  }

  /* ---------- trip generation (AT ITA Guidelines 2025 v2 Appendix 1) ---------- */
  function sharesOf(c, g) {
    if (!c) return null;
    const m = g === 'e' ? { driver: c.drive, passenger: c.pass, pt: (c.schoolBus || 0) + (c.bus || 0) + (c.train || 0) + (c.ferry || 0), walk: c.walk, cycle: c.bike }
      : { driver: (c.drivePriv || 0) + (c.driveCo || 0), passenger: c.pass, pt: (c.bus || 0) + (c.train || 0) + (c.ferry || 0), walk: c.walk, cycle: c.bike };
    const n = MODES.reduce((s, k) => s + (+m[k] || 0), 0);
    if (!(n > 0)) return null;
    const carSpec = g !== 'e' && c.stated > (c.home || 0) ? 100 * ((c.drivePriv || 0) + (c.driveCo || 0) + (c.pass || 0)) / (c.stated - (c.home || 0)) : null;
    return { ...Object.fromEntries(MODES.map(k => [k, 100 * (+m[k] || 0) / n])), n: Math.round(n), carSpec };
  }
  // Mode split for an activity in a zone: the zone's own entry, else the Census for its SA1s, else the default
  function splitFor(z, k, S = Store.S) {
    const tg = z && z.tg && z.tg.on ? z.tg : null, cfg = conf(S);
    if (tg && tg.split && MODES.some(m => tg.split[m] != null && tg.split[m] !== '')) {
      const raw = Object.fromEntries(MODES.map(m => [m, Math.max(0, +tg.split[m] || 0)])), s = MODES.reduce((a, m) => a + raw[m], 0) || 1;
      return { ...Object.fromEntries(MODES.map(m => [m, 100 * raw[m] / s])), basis: 'user', source: 'entered for this zone', sum: s };
    }
    const g = GROUP_OF[k] || 'w', src = (z && z.census && z.census.modes && z.census.modes[g]) ? z.census : z && z.censusLoc, sh = src && src.modes && sharesOf(src.modes[g], g);
    if (sh && sh.n >= STD.census.minN) {
      const codes = Array.isArray(src.sa1) ? src.sa1.map(s => s.code) : [src.sa1];
      return { ...sh, basis: 'derived', source: `${MODE_SRC[g]}, SA1 ${codes.slice(0, 4).join(', ')}${codes.length > 4 ? ' …' : ''} (${sh.n} people)` };
    }
    // the Settings split, rescaled to 100% like a zone's own entry (sum says what was entered)
    const raw = Object.fromEntries(MODES.map(m => [m, Math.max(0, +cfg.split[m] || 0)])), s = MODES.reduce((a, m) => a + raw[m], 0);
    const split = s > 0 ? Object.fromEntries(MODES.map(m => [m, 100 * raw[m] / s])) : { ...STD.split };
    return { ...Object.fromEntries(MODES.map(m => [m, split[m]])), basis: 'assumed', sum: s, source: STD.split.label + (sh ? ` (the Census sample is only ${sh.n} people)` : '') + (Math.abs(s - 100) > 0.5 ? ` (entered shares add up to ${U.fmt.num(s, 0)}%, rescaled to 100%)` : '') };
  }
  function occFor(z, k, sp, cfg) {
    const t = z && z.tg && z.tg.on ? z.tg : {};
    if (+t.occupancy > 0) return { v: +t.occupancy, basis: 'user' };
    if (GROUP_OF[k] === 'e') return { v: cfg.eduOcc, basis: 'assumed' };
    return { v: sp.driver > 0 ? (sp.driver + sp.passenger) / sp.driver : 1, basis: 'derived' };
  }
  function rateFlag(year) {
    const y = +year;
    if (!(y > 1900)) return { level: 'warn', text: 'No survey year: an indicative default. Replace it with a surveyed or published rate (AT ITA Guidelines App 1).' };
    if (y < 2020) return { level: 'review', text: `Rate from ${y}, before 2020: ${STD.ita.review}` };
    if (y <= STD.ita.reviewTo) return { level: 'review', text: `Rate from ${y}, surveyed 2020–2022: ${STD.ita.review}` };
    return null;
  }
  const unitOne = L => (L.per === 100 ? '100 m² GFA' : String(L.unit).replace(/s$/, '').replace(/places$/, 'place'));
  const pv = (o, p) => (o && o[p] !== '' && o[p] != null && isFinite(+o[p]) ? +o[p] : null);
  const sub = (a, b) => ({ in: a.in - b.in, out: a.out - b.out });
  const mul = (a, f) => ({ in: a.in * f, out: a.out * f });

  /* Trip generation for one zone and period: rows per activity and the zone totals, gross → net in the ITA order.
     loaded = (net + freight) × build-out (0 before the opening year): what the assignment loads (see sync). */
  function tripGen(z, S = Store.S, period = S.period, sc = Store.sc) {
    const on = !!(z && z.tg && z.tg.on), tg = on ? z.tg : {}, cfg = conf(S), rows = [];
    for (const k of Object.keys(LANDUSES)) {
      const q = +((z.lu || {})[k]) || 0; if (!q) continue;
      const R = ((S.rates || {})[k] || {})[period]; if (!R) continue;
      const L = LANDUSES[k], spec = on ? ((tg.rows || {})[k] || {}) : {}, units = q / L.per, meta = cfg.rateMeta[k] || {};
      // hours and the drive-through flag describe the activity (T1); they never switch the table on
      const desc = ((z.tg && z.tg.rows) || {})[k] || {}, hours = desc.hours != null ? String(desc.hours).trim() : '', drive = desc.driveThrough === true;
      const sp = splitFor(z, k, S), car = (sp.driver + sp.passenger) / 100, oc = occFor(z, k, sp, cfg), occ = oc.v;
      let basis = 'vehicle', rate, person, veh;
      if (spec.basis === 'person' && pv(spec.person, period) != null) { basis = 'person'; rate = pv(spec.person, period); person = units * rate; veh = occ > 0 ? person * car / occ : 0; }
      else { rate = pv(spec.veh, period) ?? R.rate; veh = units * rate; person = car > 0 ? veh * occ / car : null; }
      const custom = basis === 'person' || pv(spec.veh, period) != null;
      const source = spec.source || (custom ? 'entered for this zone (no source given)' : meta.source || DEFAULT_RATE_SOURCE);
      const year = spec.year != null && spec.year !== '' ? +spec.year : custom ? null : meta.year != null && meta.year !== '' ? +meta.year : null;
      const inPct = pv(spec.inPct, period) ?? R.inPct, fr = pv(spec.freight, period) || 0, hvPct = spec.hvPct != null && spec.hvPct !== '' && isFinite(+spec.hvPct) ? +spec.hvPct : S.hvPct;
      rows.push({ k, label: L.label, group: L.group, q, unit: L.unit, basis, rate, rateUnit: `${basis === 'person' ? 'person trips' : 'veh'} per ${unitOne(L)} per hour`,
        source, year, flag: rateFlag(year), person, split: sp, occ, occBasis: oc.basis, veh, inPct, vin: veh * inPct / 100, vout: veh * (1 - inPct / 100), freight: units * fr, freightRate: fr, hvPct,
        hours, driveThrough: drive, ownRate: custom });
    }
    // Trips entered directly (the user's z.extra; while the engine loads the table through z.extra, they wait in tg.other)
    const ex = otherOf(z), xin = +((ex || {})[period + 'in']) || 0, xout = +((ex || {})[period + 'out']) || 0;
    if (xin || xout) rows.push({ k: 'other', label: 'Other trips (entered directly)', group: 'Other', q: null, unit: '', basis: 'vehicle', rate: null, rateUnit: 'veh/h', source: 'entered directly', year: null, flag: null,
      person: null, split: null, occ: null, veh: xin + xout, inPct: xin + xout > 0 ? 100 * xin / (xin + xout) : 50, vin: xin, vout: xout, freight: 0, freightRate: 0, hvPct: S.hvPct, hours: '', driveThrough: false, ownRate: false });
    const tot = f => rows.reduce((s, r) => s + r[f], 0);
    const gross = { in: tot('vin'), out: tot('vout') }, eg = on ? (tg.existing || {}) : {};
    // Existing site trips larger than the new trips: the net change is negative, but the model cannot load fewer than 0
    const existingRaw = { in: Math.max(0, +eg[period + 'in'] || 0), out: Math.max(0, +eg[period + 'out'] || 0) };
    const existing = { in: Math.min(gross.in, existingRaw.in), out: Math.min(gross.out, existingRaw.out) };
    const exceeds = existingRaw.in > gross.in + 1e-9 || existingRaw.out > gross.out + 1e-9;
    const pct = v => U.clamp(+v || 0, 0, 100) / 100, iP = on ? pct(tg.internal) : 0, pP = on ? pct(tg.passby) : 0, tP = pct(z.reduction);
    const afterE = sub(gross, existing), internal = mul(afterE, iP), afterI = sub(afterE, internal), passby = mul(afterI, pP), afterP = sub(afterI, passby), tdm = mul(afterP, tP), net = sub(afterP, tdm);
    const fr = tot('freight'), freight = { in: fr / 2, out: fr / 2 }, lost = tP >= 1 && fr > 0;
    const open = !(+z.year > 0 && Engine.yearOf(sc, S) < +z.year), stage = Engine.stageOf(z, sc), kf = open ? stage : 0;
    const loaded = { in: (net.in + (lost ? 0 : freight.in)) * kf, out: (net.out + (lost ? 0 : freight.out)) * kf };
    for (const r of rows) {
      const si = gross.in > 0 ? r.vin / gross.in : 0, so = gross.out > 0 ? r.vout / gross.out : 0, both = (a) => a.in * si + a.out * so;
      r.adj = { existing: both(existing), internal: both(internal), passby: both(passby), tdm: both(tdm) };
      r.netIn = net.in * si; r.netOut = net.out * so; r.net = r.netIn + r.netOut; r.hv = r.net * r.hvPct / 100 + r.freight;
    }
    return { zone: z.id, name: z.name, cls: Engine.zoneClass(z), period, on, rows, gross, existing, existingRaw, exceeds, internal, passby, afterP, tdm, net, freight, lost, open, stage, loaded,
      pct: { internal: iP * 100, passby: pP * 100, tdm: tP * 100 } };
  }

  /* How a switched-on table reaches the engine. 'tgLoad': the engine reads z.tgLoad (in place of Σ lu × rate + z.extra)
     and z.extra stays the user's. 'extra': the engine reads only lu × rates + z.extra, so z.extra is rewritten and
     the user's trips wait in tg.other. Projects saved before tg.synced existed are in the 'extra' state. */
  const syncState = z => (z.tg && (z.tg.synced || ('other' in z.tg ? 'extra' : null))) || null;
  const otherOf = z => (z.tg && z.tg.on && syncState(z) === 'extra' ? z.tg.other : z.extra);
  // Does Engine.tripEnds read z.tgLoad in place of land use × rates + z.extra? (probed once per engine function)
  let probedFn = null, probedV = false;
  function tgNative() {
    if (typeof Engine === 'undefined' || typeof Engine.tripEnds !== 'function') return false;
    if (probedFn !== Engine.tripEnds) {
      probedFn = Engine.tripEnds;
      try {
        const S = Store.P ? Store.S : defaultSettings();
        const t = Engine.tripEnds({ id: 'lp-probe', kind: 'zone', lu: {}, extra: { AMin: 5, AMout: 5 }, tg: { on: true }, tgLoad: { AMin: 1, AMout: 1, PMin: 1, PMout: 1 } }, S, 'AM', { zones: {} });
        probedV = Math.abs(t.O + t.D - 2) < 1e-9;
      } catch { probedV = false; }
    }
    return probedV;
  }
  /* Load the zone's table into the model. load[p] = T4 after pass-by + freight ÷ (1 − r), so that the engine's
     load × (1 − r) × build-out = (net + freight) × build-out. With the 'extra' route:
     (lu × Settings rates + extra) × (1 − r) = afterPassby × (1 − r) + freight  ⇒  extra = load − lu × Settings rates. */
  function sync(z, S = Store.S, sc = Store.sc) {
    if (!z || z.kind === 'gateway' || !z.tg || !z.tg.on) return false;
    const native = tgNative(), state = syncState(z), before = JSON.stringify([z.extra || null, z.tgLoad || null]);
    // move the user's own trips to where this route keeps them before the table is worked out
    if (native && state !== 'tgLoad') { if (state === 'extra') { if (z.tg.other) z.extra = { ...z.tg.other }; else delete z.extra; } delete z.tg.other; }
    if (!native && state !== 'extra') z.tg.other = z.extra ? { ...z.extra } : null;
    z.tg.synced = native ? 'tgLoad' : 'extra';
    const load = {}, ex = {}, red = U.clamp(+z.reduction || 0, 0, 100) / 100, fk = red < 1 ? 1 / (1 - red) : 0;
    for (const p of PERIODS) {
      const t = tripGen(z, S, p, sc);
      let luIn = 0, luOut = 0;
      for (const k of Object.keys(LANDUSES)) {
        const q = +((z.lu || {})[k]) || 0; if (!q) continue;
        const r = ((S.rates || {})[k] || {})[p]; if (!r) continue;
        const T = q / LANDUSES[k].per * r.rate; luIn += T * r.inPct / 100; luOut += T * (1 - r.inPct / 100);
      }
      load[p + 'in'] = t.afterP.in + t.freight.in * fk; load[p + 'out'] = t.afterP.out + t.freight.out * fk;
      ex[p + 'in'] = load[p + 'in'] - luIn; ex[p + 'out'] = load[p + 'out'] - luOut;
    }
    if (native) z.tgLoad = load;
    else { delete z.tgLoad; z.extra = ex; }
    return JSON.stringify([z.extra || null, z.tgLoad || null]) !== before;
  }
  function syncAll() {
    if (!Store.P) return 0;
    let n = 0;
    for (const sc of Store.P.scenarios || []) for (const z of Object.values(sc.zones || {})) if (z.tg && z.tg.on && sync(z, Store.S, sc)) n++;
    return n;
  }
  // Switch a zone's trip-generation table on (the user's z.extra stays as 'other trips') or off (z.extra is the user's again)
  function setTable(z, on, S = Store.S, sc = Store.sc) {
    z.tg ||= {};
    if (on && !z.tg.on) { delete z.tg.other; delete z.tg.synced; z.tg.on = true; sync(z, S, sc); }
    else if (!on && z.tg.on) {
      if (syncState(z) === 'extra') { if (z.tg.other) z.extra = { ...z.tg.other }; else delete z.extra; }
      z.tg.on = false; delete z.tg.other; delete z.tg.synced; delete z.tgLoad;
    }
  }

  // Zones the tables cover: development zones (the proposal), or every land-use zone
  function tableZones(sc, scope = (Store.P && Store.P.landuse && Store.P.landuse.scope) || 'dev') {
    const all = Object.values(sc.zones).filter(z => z.kind !== 'gateway');
    return scope === 'all' ? all : all.filter(z => Engine.zoneClass(z) === 'development');
  }
  function tables(sc = Store.sc, S = Store.S, opts = {}) {
    const zones = opts.zones || tableZones(sc, opts.scope), tg = zones.map(z => ({ z, p: Object.fromEntries(PERIODS.map(p => [p, tripGen(z, S, p, sc)])) }));
    const flags = [], seen = new Set();
    for (const { z, p } of tg) for (const r of p.AM.rows) if (r.flag && !seen.has(z.id + r.k)) { seen.add(z.id + r.k); flags.push({ zone: z.name, activity: r.label, ...r.flag }); }
    // RR 422 sanity: residential peak-hour vehicle trips per dwelling (houses and apartments)
    const sanity = PERIODS.map(per => {
      let veh = 0, dw = 0;
      for (const { p } of tg) for (const r of p[per].rows) if (r.k === 'residential' || r.k === 'apartment') { veh += r.net; dw += r.q; }
      const rate = dw > 0 ? veh / dw : null;
      return { period: per, dwellings: dw, veh: U.r1(veh), rate: rate == null ? null : U.r3(rate), ok: rate == null ? null : rate >= STD.rr422.band[0] && rate <= STD.rr422.band[1] };
    });
    return { sc, S, zones, tg, flags, sanity };
  }
  const CSV_HEAD = ['zone_id', 'activity', 'quantity', 'unit', 'period', 'rate', 'rate_unit', 'rate_source', 'rate_year', 'person_trips', 'pct_car_driver', 'pct_car_pass', 'pct_pt', 'pct_walk', 'pct_cycle',
    'occupancy', 'veh_trips_gross', 'less_existing', 'less_internal', 'less_passby', 'less_tdm', 'veh_trips_net', 'pct_in', 'veh_in', 'veh_out', 'hv_trips'];
  function csv(sc = Store.sc, S = Store.S, opts = {}) {
    const T = tables(sc, S, opts), out = [CSV_HEAD];
    const f = x => (x == null || !isFinite(x) ? '' : r2(x));
    for (const { z, p } of T.tg) for (const per of PERIODS) for (const r of p[per].rows)
      out.push([z.id, r.k, r.q, r.unit, per, f(r.rate), r.rateUnit, r.source, r.year ?? '', f(r.person), ...MODES.map(m => (r.split ? f(r.split[m]) : '')), f(r.occ),
        f(r.veh), f(r.adj.existing), f(r.adj.internal), f(r.adj.passby), f(r.adj.tdm), f(r.net), f(r.inPct), f(r.netIn), f(r.netOut), f(r.hv)]);
    return U.csv(out);
  }

  /* ---------- zoned yield (reg-plan-change) ---------- */
  // User yields live in P.landuse.yields[code] = { dph, far, netPct, use }: use is the activity for a GFA ratio
  // on a zone with no Spare Construct default mix. P.landuse.proposed[code] = a proposed zone code (user-selected).
  function yieldDef(code, P = Store.P) {
    const base = YIELDS[code] || {}, over = (P && P.landuse && P.landuse.yields && P.landuse.yields[code]) || {};
    const num = k => (over[k] !== '' && over[k] != null && isFinite(+over[k]) ? +over[k] : null);
    const d = { ...base };
    for (const k of ['dph', 'far']) if (num(k) != null) d[k] = num(k);
    d.netPct = num('netPct') ?? STD.yields.netPct;
    d.user = ['dph', 'far', 'netPct'].some(k => num(k) != null) || GFA_USES.includes(over.use);
    if (d.far > 0 && !base.mix) { if (GFA_USES.includes(over.use)) d.mix = { [over.use]: 1 }; else d.needsUse = true; }
    return d;
  }
  // Land use from a zoned area: dwellings = ha × developable % × dph; GFA = ha × 10,000 m² × developable % × GFA ratio, split by mix
  function yieldOf(code, ha, P = Store.P, name = null) {
    const d = yieldDef(code, P), net = (+ha || 0) * d.netPct / 100, lu = {};
    if (d.dph > 0) {
      const n = Math.round(net * d.dph);
      if (d.homes === 'mixed') { lu.residential = Math.round(n / 2); lu.apartment = n - lu.residential; } else lu[d.homes || 'residential'] = n;
    }
    if (d.far > 0) for (const [k, s] of Object.entries(d.mix || {})) lu[k] = Math.round(net * 1e4 * d.far * s / 10) * 10;
    for (const [k, perHa] of Object.entries(d.per || {})) lu[k] = Math.round(net * perHa);
    for (const k of Object.keys(lu)) if (!(lu[k] > 0)) delete lu[k];
    const known = !!YIELDS[code];
    return { code: +code, name: name || AUP_NAMES[code] || `AUP zone ${code}`, ha: +ha || 0, netPct: d.netPct, dph: d.dph || null, far: d.far || null, lu, known, user: d.user,
      assumed: !!d.assumed, needsUse: !!d.needsUse, label: d.user ? STD.yields.user : known ? STD.yields.label : STD.yields.none,
      note: [d.note, d.needsUse ? 'Choose what the floor area is used for, so the GFA ratio gives a land use' : ''].filter(Boolean).join('. '),
      basis: `${U.fmt.num(+ha || 0, 2)} ha × ${d.netPct}% developable${d.dph ? ` × ${d.dph} dwellings/ha` : ''}${d.far ? ` × GFA ratio ${d.far}` : ''}${d.per ? ' × ' + Object.entries(d.per).map(([k, v]) => `${v} ${LANDUSES[k].unit}/ha`).join(', ') : ''}` };
  }
  const noYield = r => NO_YIELD.has(+r.code) || NO_YIELD_GROUPS.test(String(r.group || ''));
  const proposedOf = (code, P = Store.P) => { const t = P && P.landuse && P.landuse.proposed && P.landuse.proposed[code]; return t && +t !== +code && (YIELDS[+t] || (P.landuse.yields || {})[+t]) ? +t : null; };
  const addLu = (tot, lu) => { for (const [k, v] of Object.entries(lu || {})) tot[k] = (tot[k] || 0) + v; return tot; };
  /* Every zoned area in the study area (P.landuse.aupSummary), each with a kind:
     'zoned' (a yield, counted in full build-out), 'missing' (no Spare Construct default and none entered: listed so one can be
     entered), 'assumed' (Future Urban: a structure-plan assumption, totalled separately, never zoned capacity) or
     'none' (roads, water, open space and the like: no yield). total = full build-out of the zoned capacity.
     A row with a user-selected proposed zone also carries .proposed; proposedTotal is the build-out with those
     proposals in place of the operative zoning. opts.codes limits the zones. */
  function zonedCapacity(summary = (Store.P && Store.P.landuse && Store.P.landuse.aupSummary) || [], opts = {}) {
    const P = opts.P || Store.P;
    const rows = summary.filter(r => !opts.codes || opts.codes.includes(+r.code)).map(r => {
      const y = yieldOf(r.code, r.ha, P, r.name), none = noYield(r), to = none ? null : proposedOf(r.code, P);
      const kind = none ? 'none' : y.assumed ? 'assumed' : Object.keys(y.lu).length ? 'zoned' : 'missing';
      return { ...y, group: r.group || '', kind, label: none ? STD.yields.notLand : y.assumed ? STD.yields.assumed : y.label, proposed: to ? { ...yieldOf(to, r.ha, P), label: STD.yields.proposed } : null };
    });
    const total = {}, assumedTotal = {}, proposedTotal = {};
    for (const r of rows) {
      if (r.kind === 'zoned') addLu(total, r.lu);
      if (r.kind === 'assumed') addLu(assumedTotal, r.lu);
      addLu(proposedTotal, r.proposed ? r.proposed.lu : r.kind === 'zoned' ? r.lu : {});
    }
    const zoned = rows.filter(r => r.kind === 'zoned'), proposed = rows.filter(r => r.proposed);
    return { rows, zoned, assumed: rows.filter(r => r.kind === 'assumed'), missing: rows.filter(r => r.kind === 'missing'), total, assumedTotal,
      proposedTotal: proposed.length ? proposedTotal : null, proposedRows: proposed, label: STD.yields.label, netLabel: STD.yields.netLabel };
  }
  function applyYield(z, code, ha, P = Store.P) {
    const y = yieldOf(code, ha, P);
    z.lu = { ...y.lu }; z.site = { code: +code, ha: +ha }; z.luSrc = {};
    const source = y.user ? `User-entered yield for ${y.name}` : `${STD.yields.label} (${y.name}${y.assumed ? '; ' + STD.yields.assumed : ''})`;
    for (const [k, q] of Object.entries(y.lu)) z.luSrc[k] = { source, basis: y.user ? 'user' : 'assumed', date: today(), q, note: y.basis + (y.note ? `. ${y.note}` : '') };
    return y;
  }

  /* ---------- development traffic (dev-tracking (b) and (d)) ---------- */
  // Gateways whose flows come from counts (z.gwSrc), of all gateways
  function gwCounted(sc) { const g = Object.values(sc.zones).filter(z => z.kind === 'gateway'); return { n: g.filter(z => z.gwSrc).length, of: g.length }; }
  function distBasis(sc, res, P = Store.P, S = Store.S) {
    if (res && res.manual) return 'manual OD matrix entered by the user';
    const g = gwCounted(sc), gw = !g.of ? 'no gateways' : g.n === g.of ? `gateway flows from counts at all ${g.of} gateways`
      : g.n ? `gateway flows from counts at ${g.n} of ${g.of} gateways (the others estimated from road class and lanes)` : `gateway flows estimated from road class and lanes at all ${g.of} gateways (none from counts)`;
    const me = P && P.me && P.me[res && res.period] && Object.keys(P.me[res.period]).length > 1 ? ', with matrix estimation factors fitted to counts' : '';
    return `gravity model (β = ${S.gravityBeta} per minute of free-flow time); ${gw}${me}`;
  }
  function distribution(sc = Store.sc) {
    const res = PERIODS.map(p => Store.res(sc, p)).filter(r => r && r.dev && r.dev.trips > 0);
    if (!res.length) return null;
    const d = Engine.devDistribution(sc, res), sums = {};
    for (const r of res) sums[r.period] = { in: d.rows.reduce((s, x) => s + (x[r.period + 'in'] || 0), 0), out: d.rows.reduce((s, x) => s + (x[r.period + 'out'] || 0), 0) };
    return { ...d, sums, res, basis: res.map(r => `${r.period}: ${distBasis(sc, r)}`).join('; '), stale: res.some(r => r.stale) };
  }
  const short = s => { s = String(s || ''); return s.length > 26 ? s.slice(0, 25) + '…' : s; };
  // Figure: % arrows from the development to each gateway, placed by bearing (CSS tokens with print fallbacks)
  function distFigure(sc, d, { width = 600, height = 380 } = {}) {
    if (!d || !d.rows.length) return '';
    const dev = Object.values(sc.zones).filter(z => z.kind !== 'gateway' && Engine.zoneClass(z) === 'development');
    const gws = d.rows.map(r => sc.zones[r.id]).filter(Boolean), pts = dev.length ? dev : gws;
    const c = [pts.reduce((s, z) => s + z.lat, 0) / pts.length, pts.reduce((s, z) => s + z.lng, 0) / pts.length];
    const cx = width / 2, cy = height / 2, R = Math.min(width, height) / 2 - 52, ps = Object.keys(d.periods);
    const ink = 'var(--ink, #18201e)', mut = 'var(--muted, #56635f)', acc = 'var(--accent, #0d6a4d)';
    const arms = d.rows.map(r => {
      const g = sc.zones[r.id]; if (!g) return '';
      const share = ps.reduce((s, p) => s + (r[p + 'in'] || 0) + (r[p + 'out'] || 0), 0) / (2 * ps.length);
      if (!(share > 0.05)) return '';
      const b = U.bearing(c, [g.lat, g.lng]) * Math.PI / 180, dx = Math.sin(b), dy = -Math.cos(b), x2 = cx + dx * R, y2 = cy + dy * R;
      const lab = ps.map(p => `${p} ${U.fmt.num(r[p + 'in'] || 0, 0)}% in · ${U.fmt.num(r[p + 'out'] || 0, 0)}% out`), ax = dx >= 0.2 ? 'start' : dx <= -0.2 ? 'end' : 'middle', tx = x2 + dx * 8, ty = y2 + dy * 12;
      return `<line x1="${(cx + dx * 26).toFixed(1)}" y1="${(cy + dy * 26).toFixed(1)}" x2="${x2.toFixed(1)}" y2="${y2.toFixed(1)}" stroke="${acc}" stroke-width="${(1.5 + share / 6).toFixed(1)}" stroke-linecap="round" marker-end="url(#lpArrow)" marker-start="url(#lpArrowIn)"/>
        <text x="${tx.toFixed(1)}" y="${ty.toFixed(1)}" text-anchor="${ax}" font-size="11" fill="${ink}" font-weight="600">${esc(short(r.name))}</text>${lab.map((t, i) => `<text x="${tx.toFixed(1)}" y="${(ty + 13 * (i + 1)).toFixed(1)}" text-anchor="${ax}" font-size="10.5" fill="${mut}">${t}</text>`).join('')}`;
    }).join('');
    return `<svg class="mini-chart lp-dist" viewBox="0 0 ${width} ${height}" style="height:${height}px;max-width:100%" role="img" aria-label="Development traffic distribution by gateway">
      <defs><marker id="lpArrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="5" markerHeight="5" orient="auto"><path d="M0,0L10,5L0,10z" fill="${acc}"/></marker>
      <marker id="lpArrowIn" viewBox="0 0 10 10" refX="2" refY="5" markerWidth="5" markerHeight="5" orient="auto"><path d="M10,0L0,5L10,10z" fill="${acc}"/></marker></defs>
      <circle cx="${cx}" cy="${cy}" r="24" fill="var(--accent-soft, #dcefe7)" stroke="${acc}"/><text x="${cx}" y="${cy + 4}" text-anchor="middle" font-size="10.5" fill="${ink}" font-weight="600">Development</text>
      ${arms}<text x="8" y="${height - 8}" font-size="10" fill="${mut}">Arrow width: average share. Directions are bearings from the development to each gateway, not road routes.</text></svg>`;
  }
  // Without vs with development per junction (one period), from Engine.withWithout
  function withWithout(sc, woSc, period) {
    const a = woSc && Store.res(woSc, period), b = Store.res(sc, period);
    if (!a || !b) return null;
    const name = id => { const nr = b.nodes[id] || a.nodes[id] || {}, s = nr.site && b.sites && b.sites[nr.site]; return s && s.name ? s.name : Kit.junctionName(id, sc); };
    return { ...Engine.withWithout(a, b, { name }), stale: !!(a.stale || b.stale), period };
  }
  /* The 'without development' scenario: the user's choice (P.landuse.without), else a same-year baseline, else the
     base scenario if it is the same year, else another same-year scenario that is not a proposal or mitigation.
     A different-year scenario is never picked on its own (AT ITA s4.2.1.1: with and without in one year), as in
     Report.pick; wwHtml warns when the user's choice is a different year. */
  function withoutScenario(sc = Store.sc, P = Store.P) {
    const others = P.scenarios.filter(s => s !== sc), pick = P.landuse && P.landuse.without && others.find(s => s.id === P.landuse.without);
    if (pick) return pick;
    const yr = s => Engine.yearOf(s, P.settings), same = others.filter(s => yr(s) === yr(sc));
    return same.find(s => s.role === 'baseline') || same.find(s => s.id === P.base) || same.find(s => s.role !== 'proposal' && s.role !== 'mitigation') || null;
  }

  /* ---------- HTML: shared table builders (used by the tab and the report) ---------- */
  const T = Kit.table;
  const pill = (t, c) => `<span class="pill${c ? ' ' + c : ''}">${t}</span>`;
  const okPill = t => `<span class="pill" style="background:color-mix(in srgb, var(--ok) 18%, transparent);color:var(--ok)">${t}</span>`;
  const flagPill = f => (!f ? '' : f.level === 'review' ? pill('review', 'proposed') + ` <span class="note">${esc(f.text)}</span>` : pill('undated', 'stale'));
  const luProv = (z, k) => {
    const s = z.luSrc && z.luSrc[k];
    if (s && (s.q == null || +s.q === (+((z.lu || {})[k]) || 0))) return s;
    if (s && s.q != null) return { source: 'changed in Spare Construct after it was filled', basis: 'user' };
    if (z.src === 'existing-lu' || /^Existing · /.test(z.name || '')) return { source: SRC.osm.name, licence: SRC.osm.licence, basis: 'estimated' };
    return { source: 'entered in Spare Construct', basis: 'user' };
  };
  const provText = p => (p ? [p.source, p.vintage, p.licence, isCensus(p) ? 'random-rounded to base 3' : ''].filter(Boolean).join(' · ') : '');
  const EMPTY = 'No development zones with land use.';
  function t1Html(R) {
    const rows = [];
    for (const { z, p } of R.tg) for (const r of p.AM.rows) if (r.k !== 'other') {
      const pr = luProv(z, r.k);
      rows.push([esc(z.name), `${esc(r.label)}${r.driveThrough ? ' ' + pill('drive-through') : ''}`, n0(r.q), esc(r.unit), r.hours ? esc(r.hours) : '–', z.year ? esc(z.year) : 'always open', esc(Engine.zoneClass(z)), `${Kit.prov(pr)} <span class="note">${esc(provText(pr))}</span>`]);
    }
    return T({ head: ['Zone', 'Activity', { label: 'Quantity', n: true }, 'Unit', 'Hours', 'Opening year', 'Class', 'Source'], rows, empty: EMPTY });
  }
  function t2Html(R) {
    const rows = [];
    for (const { z, p } of R.tg) for (const per of PERIODS) for (const r of p[per].rows) if (r.k !== 'other')
      rows.push([esc(z.name), esc(r.label), per, U.fmt.num(r.rate, 3), esc(r.rateUnit), r.basis === 'person' ? 'person-trip rate' : 'vehicle rate', `${esc(r.source)}${r.year ? ` (${esc(r.year)})` : ''} ${flagPill(r.flag)}`, n1(r.person)]);
    return T({ head: ['Zone', 'Activity', 'Period', { label: 'Rate', n: true }, 'Rate unit', 'Basis', 'Rate source', { label: 'Person trips', n: true }], rows, empty: EMPTY });
  }
  function t3Html(R) {
    const rows = [];
    for (const { z, p } of R.tg) for (const r of p.AM.rows) if (r.split) rows.push([esc(z.name), esc(r.label), ...MODES.map(m => pc(r.split[m])), U.fmt.num(r.occ, 2), `${Kit.prov({ basis: r.split.basis, source: r.split.source })} <span class="note">${esc(r.split.source)}</span>`]);
    return T({ head: ['Zone', 'Activity', ...MODES.map(m => ({ label: MODE_LABEL[m], n: true })), { label: 'Car occupancy', n: true }, 'Source'], rows, empty: EMPTY });
  }
  // Build-out text for a zone in a period: 'opens 2031' before its opening year, else the staged share
  const stageText = (z, t) => (!t.open ? `opens ${esc(z.year)}` : t.stage < 1 ? pc(t.stage * 100) : '100%');
  // Existing site trips larger than the new trips: '−12.0 veh/h' per direction where that happens
  const exceedText = t => ['in', 'out'].filter(d => t.existingRaw[d] > t.gross[d] + 1e-9).map(d => `${d} ${U.fmt.num(t.gross[d] - t.existingRaw[d], 1)} veh/h`).join(', ');
  /* T4: gross to net, then what the model loads: (net + freight) × build-out, 0 before the opening year. The
     Loaded column is what Engine.tripEnds gives the assignment, so it reconciles T4 with the model under staging. */
  function t4Html(R) {
    const rows = [], sum = {}, over = [];
    for (const { z, p } of R.tg) for (const per of PERIODS) {
      const t = p[per], g = t.gross.in + t.gross.out, n = t.net.in + t.net.out, fr = t.lost ? 0 : t.freight.in + t.freight.out, ld = t.loaded.in + t.loaded.out;
      const c = [g, t.existing.in + t.existing.out, t.internal.in + t.internal.out, t.passby.in + t.passby.out, t.tdm.in + t.tdm.out, n, t.net.in, t.net.out, fr];
      const s = sum[per] ||= { c: c.map(() => 0), ld: 0 }; c.forEach((v, i) => { s.c[i] += v; }); s.ld += ld;
      const cells = c.map(n1);
      if (t.exceeds) { cells[1] += ' *'; over.push(`${esc(z.name)} ${per} (${exceedText(t)})`); }
      rows.push({ cells: [esc(z.name), per, ...cells, stageText(z, t), `<b>${n1(ld)}</b>`], fly: 'zone:' + z.id });
    }
    for (const per of PERIODS) if (sum[per] && R.tg.length > 1) rows.push({ cells: ['<b>Total</b>', per, ...sum[per].c.map(v => `<b>${n1(v)}</b>`), '', `<b>${n1(sum[per].ld)}</b>`] });
    return T({ head: ['Zone', 'Period', { label: 'Gross', n: true }, { label: 'Less existing', n: true }, { label: 'Less internal', n: true }, { label: 'Less pass-by', n: true }, { label: 'Less TDM', n: true },
      { label: 'Net', n: true }, { label: 'Net in', n: true }, { label: 'Net out', n: true }, { label: 'Freight and service', n: true }, { label: 'Build-out', n: true }, { label: 'Loaded into the model', n: true }], rows, empty: EMPTY }) +
      (over.length ? Kit.note(`* Existing site trips are larger than the new trips, so the net change is negative: ${over.join('; ')}. The model cannot load fewer than 0 trips for a zone, so it loads 0 there (plus freight). Show the reduction in the assessment text.`, true) : '');
  }
  function t4bHtml(R) {
    const rows = [];
    for (const { z, p } of R.tg) for (const per of PERIODS) for (const r of p[per].rows) if (r.k !== 'other') rows.push([esc(z.name), esc(r.label), per, U.fmt.num(r.freightRate, 3), n1(r.freight), pc(r.hvPct), n1(r.hv)]);
    return T({ head: ['Zone', 'Activity', 'Period', { label: 'Freight rate (veh per unit per hour)', n: true }, { label: 'Freight and service veh/h', n: true }, { label: 'HV share of vehicle trips', n: true }, { label: 'HV trips', n: true }], rows, empty: EMPTY });
  }
  // Checks the table against the engine for the scenario and settings the table was worked out for
  function loadedNote(R) {
    const parts = R.tg.map(({ z, p }) => PERIODS.map(per => { const t = p[per], e = Engine.tripEnds(z, R.S, per, R.sc), l = t.loaded.in + t.loaded.out, m = e.O + e.D;
      return Math.abs(l - m) > 0.05 ? `${esc(z.name)} ${per}: table ${n1(l)}, model ${n1(m)} veh/h` : null; }).filter(Boolean)).flat();
    const lost = R.tg.filter(({ p }) => PERIODS.some(per => p[per].lost)).map(({ z }) => esc(z.name));
    const drive = R.tg.flatMap(({ z, p }) => p.AM.rows.filter(r => r.driveThrough && !r.ownRate).map(r => `${esc(z.name)} ${esc(r.label.toLowerCase())}`));
    return (parts.length ? Kit.note(`The model loads different trips from this table: ${parts.join('; ')}. Open the zone and change any trip-generation input to reload it.`, true) : '') +
      (lost.length ? Kit.note(`Freight cannot be loaded for ${lost.join(', ')}: the TDM reduction is 100%.`, true) : '') +
      (drive.length ? Kit.note(`Drive-through: ${drive.join(', ')} use${drive.length === 1 ? 's' : ''} the Settings rate, which is not a drive-through rate. Enter a surveyed drive-through rate for the zone.`, true) : '');
  }
  function sanityHtml(R) {
    return T({ head: ['Period', { label: 'Dwellings', n: true }, { label: 'Net vehicle trips', n: true }, { label: 'veh per dwelling', n: true }, 'Check'],
      rows: R.sanity.map(s => [s.period, n0(s.dwellings), n1(s.veh), s.rate == null ? '–' : U.fmt.num(s.rate, 2), s.ok == null ? 'no dwellings' : s.ok ? okPill('within band') : pill('outside band', 'proposed')]) }) +
      Kit.note(`Residential peak-hour vehicle trips per dwelling against ${esc(STD.rr422.name)}: about ${STD.rr422.perHousehold} per household. ${esc(STD.rr422.bandLabel)}: ${STD.rr422.band.join('–')}.`);
  }
  const tableNote = () => Kit.note(`Order of adjustments (${esc(STD.ita.name)}): ${esc(STD.ita.sequence)}. TDM is the zone's trip reduction for public transport, walking and cycling. Freight and service trips are added after the adjustments and are not reduced. Loaded into the model = (net + freight) × the scenario's build-out, and 0 before the zone's opening year. Values are veh/h in the peak hour.`);
  /* The trip generation tables T1–T4b, the RR 422 check and the rate notes. opts.parts picks some of them, in this
     order: any of TRIP_PARTS ('flags' = the review and undated-rate notes); without it everything is printed, as the
     Land use tab and the report section show it. The report lane prints T1 apart from the rest this way. */
  const TRIP_PARTS = ['T1', 'T2', 'T3', 'T4', 'T4b', 'check', 'flags'];
  function tripTablesHtml(sc = Store.sc, opts = {}) {
    const R = tables(sc, Store.S, opts);
    const fl = R.flags.filter(f => f.level === 'review');
    const part = {
      T1: () => `<h4>T1 Land use schedule</h4>${t1Html(R)}`,
      T2: () => `<h4>T2 Person trips</h4>${t2Html(R)}`,
      T3: () => `<h4>T3 Mode share</h4>${t3Html(R)}`,
      T4: () => `<h4>T4 Vehicle trips, gross to net (veh/h)</h4>${t4Html(R)}${tableNote()}${loadedNote(R)}`,
      T4b: () => `<h4>T4b Freight and service vehicles</h4>${t4bHtml(R)}`,
      check: () => `<h4>Check against RR 422</h4>${sanityHtml(R)}`,
      flags: () => `${fl.length ? Kit.note(`${fl.length} rate${fl.length > 1 ? 's need' : ' needs'} review: ${fl.map(f => `${esc(f.zone)} ${esc(f.activity)} (${esc(f.text)})`).join('; ')}.`, true) : ''}
      ${R.flags.some(f => f.level === 'warn') ? Kit.note('Rates without a survey year are Spare Construct’s indicative defaults. Add the source and year in Settings › Land use and trip generation, or per zone.', true) : ''}`,
    };
    if (!Array.isArray(opts.parts)) return `${part.T1()}${part.T2()}${part.T3()}${part.T4()}
      ${part.T4b()}${part.check()}
      ${part.flags()}`;
    return TRIP_PARTS.filter(k => opts.parts.includes(k)).map(k => part[k]()).join('');
  }
  /* The count source of a gateway as 'NZTA 01N00478 · SH1 Drury · 61,200/day · NB' (zone-gateway-ux (b)): each count named in
     z.gwSrc (DataHub's label) with its direction, or 'both directions' for a two-way count. Counts that are no longer
     loaded keep DataHub's text. Plain text: callers escape it. */
  function gwSrcText(z) {
    if (!z || !z.gwSrc) return '';
    const D = typeof DataHub !== 'undefined' ? DataHub : null, counts = D ? D.D().counts : [], short = c => ((D && D.SOURCES[c.src]) || { short: c.src }).short;
    const label = c => `${short(c)}${c.virtual ? ' estimate' : ''} ${c.ref || ''} ${c.name}${c.adt ? ` (${U.fmt.int(c.adt)}/day)` : ''}`;
    return String(z.gwSrc).split('; ').map(seg => {
      const c = counts.find(x => label(x) === seg);
      return c ? [`${short(c)}${c.virtual ? ' estimate' : ''}${c.ref ? ' ' + c.ref : ''}`, c.name, c.adt ? `${U.fmt.int(c.adt)}/day` : '', c.directional && c.dir ? c.dir : 'both directions'].filter(Boolean).join(' · ') : seg;
    }).join('; ');
  }
  const gwBadge = z => (z && z.gwSrc ? `<span title="${esc(gwSrcText(z))}">${okPill('From counts')}</span>` : pill('Estimated', 'proposed'));
  function t5Html(sc = Store.sc) {
    const d = distribution(sc);
    if (!d) return Kit.note('Run the model (AM and PM) with development zones to see where their traffic goes.');
    const ps = Object.keys(d.periods);
    const rows = d.rows.map(r => ({ cells: [esc(r.name), gwBadge(sc.zones[r.id]), ...ps.flatMap(p => [pc(r[p + 'in']), pc(r[p + 'out'])]), ps.map(p => n0(r.vph[p])).join(' / ')], fly: 'zone:' + r.id }));
    rows.push({ cells: ['<b>Total crossing the boundary</b>', '', ...ps.flatMap(p => [`<b>${pc(d.sums[p].in)}</b>`, `<b>${pc(d.sums[p].out)}</b>`]), ps.map(p => n0(d.periods[p].in + d.periods[p].out)).join(' / ')] });
    return T({ head: ['Gateway', 'Gateway flows', ...ps.flatMap(p => [{ label: `${p} in`, n: true }, { label: `${p} out`, n: true }]), { label: `veh/h (${ps.join(' / ')})`, n: true }], rows }) +
      Kit.note(`${esc(d.label)} Internal: ${ps.map(p => `${p} ${d.periods[p].internalPct == null ? '–' : U.fmt.num(d.periods[p].internalPct, 0) + '%'}`).join(', ')}. Distribution basis: ${esc(d.basis)}.${d.stale ? ' Results are out of date: run the model again.' : ''}`);
  }
  /* Without vs with development (dev-tracking (d)). Junctions in only one of the two scenarios (new site accesses,
     or junctions removed) are listed as new or removed; the rest are the 15 with the largest change in average
     delay either way (|Δ|), per period. A different-year 'without' gets a warning: the change then includes growth. */
  function wwHtml(sc = Store.sc, woSc = withoutScenario(sc)) {
    const P = Store.P, yr = s => Engine.yearOf(s, P.settings);
    if (!woSc) return Kit.note(`No scenario without the development for ${esc(yr(sc))}. Add the do-minimum for the same year (or choose one) to compare junctions without and with the development.`);
    const out = PERIODS.map(p => withWithout(sc, woSc, p)).filter(Boolean);
    if (!out.length) return Kit.note(`Run the model for both ${esc(woSc.name)} and ${esc(sc.name)} to compare junctions without and with the development.`);
    const rows = [], dash = '–', NEW = pill('new', 'proposed'), GONE = pill('removed', 'stale');
    let nNew = 0, nGone = 0;
    for (const w of out) {
      const one = w.rows.filter(r => !(r.without && r.with)), both = w.rows.filter(r => r.without && r.with).sort((p, q) => Math.abs(q.delta.delay) - Math.abs(p.delta.delay)).slice(0, 15);
      for (const r of one) {
        const a = r.without, b = r.with, isNew = !a; if (isNew) nNew++; else nGone++;
        const v = (x, f) => (x ? f(x) : dash), tag = isNew ? NEW : GONE;
        rows.push([`${esc(r.name)} ${tag}`, w.period, v(a, x => U.fmt.num(x.dos, 2)), v(b, x => U.fmt.num(x.dos, 2)), tag, v(a, x => U.fmt.num(x.delay, 1)), v(b, x => U.fmt.num(x.delay, 1)), tag,
          a ? Kit.losBadge(a.los) : dash, b ? Kit.losBadge(b.los) : dash, tag, v(a, x => n0(x.q95)), v(b, x => n0(x.q95)), tag]);
      }
      for (const r of both) {
        const a = r.without, b = r.with, d = r.delta;
        rows.push([esc(r.name), w.period, U.fmt.num(a.dos, 2), U.fmt.num(b.dos, 2), esc(d.dosText), U.fmt.num(a.delay, 1), U.fmt.num(b.delay, 1), esc(d.delayText), Kit.losBadge(a.los), Kit.losBadge(b.los), esc(d.los), n0(a.q95), n0(b.q95), esc(d.q95Text)]);
      }
    }
    const sub = ['Max DoS', 'Avg delay (s)', 'LoS', 'Max Q95 (m)'], ya = yr(woSc), yb = yr(sc);
    return T({ head: ['Junction', 'Period', ...sub.flatMap(s => [{ label: `${s} without`, n: true }, { label: 'with', n: true }, { label: 'Δ', n: true }])], rows, empty: 'No junction results.' }) +
      (ya !== yb ? Kit.note(`Without (${esc(woSc.name)}, ${esc(ya)}) and with (${esc(sc.name)}, ${esc(yb)}) are different years, so the change includes background growth, not just the development. Choose a ${esc(yb)} scenario without the development.`, true) : '') +
      Kit.note(`Without: ${esc(woSc.name)}. With: ${esc(sc.name)}. Per period, the 15 junctions with the largest change in average delay (increase or decrease)${nNew || nGone ? `, and every junction found in only one of the two (${[nNew ? `${nNew} new, such as site accesses` : '', nGone ? `${nGone} removed` : ''].filter(Boolean).join('; ')})` : ''}. ${esc(Engine.JUNCTION_LABEL)}. ${esc(out[0].label)}`) +
      out.filter(w => w.note || w.stale).map(w => Kit.note(`${w.period}: ${esc(w.note || 'results are out of date; run both scenarios again.')}`, true)).join('');
  }
  const luText = lu => Object.entries(lu || {}).map(([k, v]) => `${n0(v)} ${esc(LANDUSES[k].unit)} ${esc(LANDUSES[k].label.toLowerCase())}`).join(', ');
  /* The zoned-capacity table: every zoned area in the study area. edit = inputs for the yield, the activity of a GFA
     ratio where Spare Construct has no default mix, and a user-selected proposed zone. */
  function yieldTableHtml(edit = false) {
    const cap = zonedCapacity(undefined, {}), P = Store.P, L = (P && P.landuse) || {};
    if (!cap.rows.length) return Kit.note('Fill existing land use (with Unitary Plan zones) first, so Spare Construct knows the zoned land in the study area.');
    const inp = (code, k, v, label) => (edit ? `<input type="text" inputmode="decimal" style="width:62px" data-lp-yield="${esc(code)}.${esc(k)}" value="${esc(v ?? '')}" aria-label="${esc(label)}">` : v == null || v === '' ? '–' : esc(v));
    const useSel = (r, o) => (!edit || (YIELDS[r.code] && YIELDS[r.code].mix) ? '' : ` <select data-lp-yield="${esc(r.code)}.use" aria-label="${esc(`Floor area use for ${r.name}`)}"><option value="">use…</option>${GFA_USES.map(u => `<option value="${u}" ${o.use === u ? 'selected' : ''}>${esc(LANDUSES[u].label)}</option>`).join('')}</select>`);
    const propSel = r => {
      if (r.kind === 'none') return '–';
      const cur = (L.proposed || {})[r.code];
      if (!edit) return r.proposed ? `${esc(r.proposed.name)} <span class="note">(${esc(STD.yields.proposed)})</span>` : '–';
      return `<select data-lp-proposed="${esc(r.code)}" aria-label="${esc(`Proposed zone for ${r.name}`)}"><option value="">none</option>${Object.keys(YIELDS).filter(c => +c !== +r.code && !YIELDS[c].assumed).map(c => `<option value="${c}" ${String(cur) === String(c) ? 'selected' : ''}>${esc(AUP_NAMES[c])}</option>`).join('')}</select>`;
    };
    const order = { zoned: 0, missing: 1, assumed: 2, none: 3 };
    const rows = cap.rows.slice().sort((a, b) => order[a.kind] - order[b.kind] || b.ha - a.ha).map(r => {
      const o = (L.yields || {})[r.code] || {}, grey = t => `<span class="note">${t}</span>`;
      if (r.kind === 'none') return [grey(esc(r.name)), U.fmt.num(r.ha, 1), '–', '–', '–', '–', grey(esc(r.label)), '–'];
      const capTxt = (luText(r.lu) || '–') + (r.proposed ? `<br><span class="note">Proposed: ${luText(r.proposed.lu) || '–'}</span>` : '');
      return [esc(r.name), U.fmt.num(r.ha, 1), inp(r.code, 'netPct', o.netPct ?? r.netPct, `Developable % for ${r.name}`), inp(r.code, 'dph', o.dph ?? r.dph, `Dwellings per ha for ${r.name}`),
        inp(r.code, 'far', o.far ?? r.far, `GFA ratio for ${r.name}`) + useSel(r, o), capTxt, `${esc(r.label)}${r.note ? ` <span class="note">${esc(r.note)}.</span>` : ''}`, propSel(r)];
    });
    const miss = cap.missing.map(r => `${esc(r.name)} (${U.fmt.num(r.ha, 1)} ha)`), ass = cap.assumed;
    return T({ head: ['AUP zone', { label: 'Zoned land (ha)', n: true }, { label: 'Developable %', n: true }, { label: 'Dwellings/ha', n: true }, { label: 'GFA ratio', n: true }, 'Capacity (full build-out)', 'Yield', 'Proposed zone'], rows }) +
      Kit.note(`<b>Full build-out (zoned capacity, every zone with a yield):</b> ${luText(cap.total) || '–'}. ${esc(STD.yields.label)}. ${esc(STD.yields.netLabel)}. Interim stages: set a scenario's build-out % (staging).`) +
      (miss.length ? Kit.note(`Not included, as Spare Construct has no default yield for them: ${miss.join('; ')}. ${edit ? 'Enter a yield in the table to include them.' : 'Enter a yield in the Land use tab to include them.'}`, true) : '') +
      (ass.length ? Kit.note(`<b>Structure-plan assumption (not zoned capacity):</b> ${ass.map(r => `${esc(r.name)} ${U.fmt.num(r.ha, 1)} ha: ${luText(r.lu) || '–'}`).join('; ')}. ${esc(ass[0].note)}.`, true) : '') +
      (cap.proposedTotal ? Kit.note(`<b>Full build-out with the proposed zoning (${esc(STD.yields.proposed)}):</b> ${luText(cap.proposedTotal) || '–'}, from ${cap.proposedRows.map(r => `${esc(r.name)} → ${esc(r.proposed.name)}`).join('; ')}. The PC120 proposed-zone overlay is not loaded; these proposals are entered by the user.`) : '');
  }

  /* ---------- zone inspector section ---------- */
  const GROUPS = [...new Set(Object.values(LANDUSES).map(l => l.group))];
  function zoneTripsHtml(z, sc, S = Store.S) {
    const t = Object.fromEntries(PERIODS.map(p => [p, tripGen(z, S, p, sc)]));
    const line = (label, f, bold) => `<tr><td>${bold ? `<b>${label}</b>` : label}</td>${PERIODS.map(p => `<td class="n">${bold ? '<b>' : ''}${n1(f(t[p]))}${bold ? '</b>' : ''}</td>`).join('')}</tr>`;
    const io = o => o.in + o.out;
    const acts = t.AM.rows.map(r => { const rp = t.PM.rows.find(x => x.k === r.k);
      return `<tr><td>${esc(r.label)}${r.flag && r.flag.level === 'review' ? ' ' + pill('review', 'proposed') : ''}${r.driveThrough ? ' ' + pill('drive-through') : ''}</td><td class="n">${n1(r.person)}</td><td class="n">${n1(r.veh)}</td><td class="n">${rp ? n1(rp.person) : '–'}</td><td class="n">${rp ? n1(rp.veh) : '–'}</td></tr>`; }).join('');
    const sp = t.AM.rows.find(r => r.split), over = PERIODS.filter(p => t[p].exceeds).map(p => `${p} ${exceedText(t[p])}`);
    return `<div class="tbl-wrap"><table class="t compact"><tr><th>Activity</th><th class="n">AM person</th><th class="n">AM veh</th><th class="n">PM person</th><th class="n">PM veh</th></tr>${acts || '<tr><td class="note" colspan="5">No land use yet.</td></tr>'}</table></div>
      <div class="tbl-wrap"><table class="t compact"><tr><th>Vehicle trips (veh/h)</th>${PERIODS.map(p => `<th class="n">${p}</th>`).join('')}</tr>
        ${line('Gross', x => io(x.gross))}${line('Less existing site trips', x => -io(x.existing))}${line(`Less internal (${U.fmt.num(t.AM.pct.internal, 0)}%)`, x => -io(x.internal))}
        ${line(`Less pass-by (${U.fmt.num(t.AM.pct.passby, 0)}%)`, x => -io(x.passby))}${line(`Less TDM (${U.fmt.num(t.AM.pct.tdm, 0)}%)`, x => -io(x.tdm))}${line('Net', x => io(x.net), true)}
        ${line('Freight and service', x => io(x.freight))}${line('Loaded into the model', x => io(x.loaded), true)}</table></div>
      ${over.length ? Kit.note(`Existing site trips exceed the new trips: net change ${over.join('; ')}. The model loads 0 for that direction (plus freight), as it cannot load fewer than 0.`, true) : ''}
      ${!t.AM.open ? Kit.note(`Opens in ${esc(z.year)}: no trips in ${esc(Engine.yearOf(sc, S))}.`, true) : ''}${t.AM.stage < 1 ? Kit.note(`Build-out ${U.fmt.num(t.AM.stage * 100, 0)}% in this scenario.`) : ''}
      ${sp ? Kit.note(`Mode split: ${MODES.map(m => `${MODE_LABEL[m].toLowerCase()} ${U.fmt.num(sp.split[m], 0)}%`).join(', ')}; ${U.fmt.num(sp.occ, 2)} people per car. ${Kit.prov({ basis: sp.split.basis, source: sp.split.source })} ${esc(sp.split.source)}.`) : ''}
      ${t.AM.on ? '' : Kit.note('Vehicle trips use the rates in Settings. Person trips are derived from them with the mode split and occupancy.')}`;
  }
  // Paths that describe an activity without changing its trips: they never switch the zone's table on
  const DESCRIBES = /^tg\.rows\.[a-z]+\.(hours|driveThrough)$/;
  function zoneSection(sc, sel) {
    const z = sc.zones[sel.id]; if (!z || z.kind === 'gateway') return '';
    const S = Store.S, cls = Engine.zoneClass(z), on = !!(z.tg && z.tg.on), tg = z.tg || {}, cfg = conf(S);
    const per = k => { const t = PERIODS.map(p => { const r = tripGen(z, S, p, sc).rows.find(x => x.k === k); return r ? n1(r.veh) : '–'; }); return t.join(' / '); };
    const luRows = GROUPS.map(g => `<tr><th colspan="4">${esc(g)}</th></tr>` + Object.entries(LANDUSES).filter(([, L]) => L.group === g).map(([k, L]) => {
      const q = (z.lu || {})[k], pr = q ? luProv(z, k) : null;
      return `<tr><td>${esc(L.label)}</td><td><span class="fld-in"><input type="text" inputmode="decimal" style="width:84px" data-lp="lu.${k}" data-lpt="num" value="${q ? esc(q) : ''}" placeholder="0" aria-label="${esc(L.label)} (${esc(L.unit)})"><span class="unit">${esc(L.unit)}</span></span><span class="lp-err note" hidden></span></td>
        <td>${pr ? `<span title="${esc(provText(pr))}">${Kit.prov(pr)}</span>` : ''}</td><td class="n">${q ? per(k) : ''}</td></tr>`; }).join('')).join('');
    const num = (path, v, unit, w = 64, label = path) => `<span class="fld-in"><input type="text" inputmode="decimal" style="width:${w}px" data-lp="${esc(path)}" data-lpt="num" value="${esc(v ?? '')}" aria-label="${esc(label)}">${unit ? `<span class="unit">${esc(unit)}</span>` : ''}</span><span class="lp-err note" hidden></span>`;
    const rowsTg = Object.keys(z.lu || {}).filter(k => LANDUSES[k] && +z.lu[k]).map(k => {
      const r = (tg.rows || {})[k] || {}, rr = (S.rates[k] || {}), L = LANDUSES[k];
      return `<tr><td>${esc(L.label)}</td><td><select data-lp="tg.rows.${k}.basis" data-lpt="str" aria-label="${esc(L.label)} rate basis"><option value="vehicle" ${r.basis !== 'person' ? 'selected' : ''}>veh</option><option value="person" ${r.basis === 'person' ? 'selected' : ''}>person</option></select></td>
        ${PERIODS.map(p => `<td>${num(`tg.rows.${k}.${r.basis === 'person' ? 'person' : 'veh'}.${p}`, pv(r.basis === 'person' ? r.person : r.veh, p), '', 54, `${L.label} ${p} rate`)}</td><td>${num(`tg.rows.${k}.inPct.${p}`, pv(r.inPct, p) ?? (rr[p] || {}).inPct, '', 40, `${L.label} ${p} % in`)}</td>`).join('')}
        <td><input type="text" style="width:120px" data-lp="tg.rows.${k}.source" data-lpt="str" value="${esc(r.source || '')}" placeholder="${esc((cfg.rateMeta[k] || {}).source || 'source')}" aria-label="${esc(L.label)} rate source"></td>
        <td>${num(`tg.rows.${k}.year`, r.year, '', 50, `${L.label} rate year`)}</td><td>${num(`tg.rows.${k}.freight.AM`, pv(r.freight, 'AM'), '', 44, `${L.label} AM freight rate`)}</td><td>${num(`tg.rows.${k}.freight.PM`, pv(r.freight, 'PM'), '', 44, `${L.label} PM freight rate`)}</td>
        <td><input type="text" style="width:96px" data-lp="tg.rows.${k}.hours" data-lpt="str" value="${esc(r.hours || '')}" placeholder="e.g. 7am–10pm" aria-label="${esc(L.label)} hours"></td>
        <td>${L.group === 'Commercial' ? `<input type="checkbox" data-lp="tg.rows.${k}.driveThrough" data-lpt="bool" ${r.driveThrough === true ? 'checked' : ''} aria-label="${esc(L.label)} has a drive-through">` : ''}</td></tr>`;
    }).join('');
    const ex = tg.existing || {}, yc = (z.site && z.site.code) || (z.aup && z.aup.code) || '';
    return `<div class="ins-sec"><h4>Land use and trips ${pill(esc(cls))}</h4>
        <label class="check"><input type="checkbox" ${Kit.bind('zone', z.id, 'existing', 'bool')} ${z.existing ? 'checked' : ''}> Existing land use (part of the base year; used for matrix estimation)</label>
        ${Kit.fld('Opening year', `<input type="number" min="1900" max="2200" step="1" ${Kit.bind('zone', z.id, 'year', 'numnull')} value="${esc(z.year ?? '')}" placeholder="always open">`)}
        ${Kit.fld('Trip reduction (TDM)', `<input type="number" min="0" max="100" step="1" ${Kit.bind('zone', z.id, 'reduction', 'num')} value="${esc(z.reduction || '')}" placeholder="0">`, '% of vehicle trips by PT, walking, cycling')}
        <div class="tbl-wrap"><table class="t compact"><tr><th>Land use</th><th>Quantity</th><th>Source</th><th class="n">veh/h AM / PM</th></tr>${luRows}</table></div>
        ${Kit.note('Type 12,000 or 5000 m². Changes update the trip table as you type and save when you leave the box.')}
      </div>
      <div class="ins-sec"><h4>Person and vehicle trips</h4><div data-lp-trips>${zoneTripsHtml(z, sc, S)}</div>
        <details${on ? ' open' : ''}><summary>Trip-generation inputs (AT ITA Guidelines App 1)</summary>
          <label class="check"><input type="checkbox" data-lp="tg.on" data-lpt="bool" ${on ? 'checked' : ''}> Use this zone's own trip-generation table in the model</label>
          ${Kit.note('Any change below, except hours and drive-through, switches the table on. The model then loads its net trips plus freight.')}
          <div class="tbl-wrap"><table class="t compact"><tr><th>Activity</th><th>Basis</th>${PERIODS.map(p => `<th>${p} rate</th><th>${p} % in</th>`).join('')}<th>Rate source</th><th>Year</th><th>Freight AM</th><th>Freight PM</th><th>Hours</th><th>Drive-through</th></tr>${rowsTg || '<tr><td class="note" colspan="12">Add land use first.</td></tr>'}</table></div>
          ${Kit.note('Rates are per unit per peak hour: person trips for a person basis, vehicles for a vehicle basis. Freight is heavy and service vehicles per unit per hour, kept separate. A drive-through needs its own surveyed rate.')}
          <div class="pair">${MODES.map(m => `<label>${MODE_LABEL[m]} %${num('tg.split.' + m, (tg.split || {})[m], '', 52, MODE_LABEL[m] + ' %')}</label>`).join('')}<label>People per car${num('tg.occupancy', tg.occupancy, '', 52, 'People per car')}</label></div>
          <div class="pair"><label>Internal %${num('tg.internal', tg.internal, '', 52, 'Internal %')}</label><label>Pass-by %${num('tg.passby', tg.passby, '', 52, 'Pass-by %')}</label>
            ${PERIODS.map(p => `<label>Existing ${p} in${num(`tg.existing.${p}in`, ex[p + 'in'], '', 52, `Existing site trips ${p} in`)}</label><label>Existing ${p} out${num(`tg.existing.${p}out`, ex[p + 'out'], '', 52, `Existing site trips ${p} out`)}</label>`).join('')}</div>
          ${Kit.note('Existing site trips are surveyed veh/h at the site today. Leave the mode split blank to use the Census for this location, or the default.')}
        </details></div>
      <div class="ins-sec"><h4>Zoned yield</h4>
        ${Kit.fld('Unitary Plan zone', `<select data-lp-yc aria-label="Unitary Plan zone"><option value="">Choose…</option>${Object.keys(YIELDS).map(c => `<option value="${c}" ${String(c) === String(yc) ? 'selected' : ''}>${esc(AUP_NAMES[c])}${YIELDS[c].assumed ? ` (${esc(STD.yields.assumed)})` : ''}</option>`).join('')}</select>`)}
        ${Kit.fld('Site area', `<input type="text" inputmode="decimal" style="width:80px" data-lp-yha value="${esc((z.site && z.site.ha) || '')}" aria-label="Site area (ha)">`, 'ha')}
        <div class="actions"><button class="btn small" data-lp-do="yield">Fill land use from zoned yield</button></div>
        ${Kit.note(`${esc(STD.yields.label)} (or your own yield from the Land use tab). Replaces this zone's land use.${z.aup ? ` This zone stands in ${esc(z.aup.name)}.` : ''}`)}</div>
      ${sourcesHtml(z)}`;
  }
  function sourcesHtml(z) {
    const items = [], luList = o => Object.entries(o).map(([k, v]) => `${n0(v)} ${esc((LANDUSES[k] || {}).unit || k)} ${esc(((LANDUSES[k] || {}).label || k).toLowerCase())}`).join(', ');
    if (z.census && z.census.sa1 && z.census.sa1.length) items.push(`Census 2023: ${n0(z.census.sa1.length)} SA1${z.census.sa1.length > 1 ? 's' : ''} (${esc(z.census.sa1.slice(0, 5).map(s => s.code).join(', '))}${z.census.sa1.length > 5 ? ' …' : ''}): ${n0(z.census.pop)} people, ${n0(z.census.hh)} households, ${z.census.hh ? U.fmt.num(z.census.vehicles / z.census.hh, 2) : '–'} vehicles per household. Counts are random-rounded to base 3 and split by building footprint or area.`);
    if (z.jobs) items.push(`${n0(z.jobs.n)} employees (${esc(z.jobs.source)}, ${esc(z.jobs.year)}), placed by floor area.`);
    if (z.moe) items.push(`Ministry of Education: ${esc(z.moe.type)}${z.moe.rollDate ? `, roll at ${esc(z.moe.rollDate)}` : ''}.`);
    if (z.aup) items.push(`Unitary Plan: ${esc(z.aup.name)}${z.aup.status ? ` (${esc(z.aup.status)})` : ''}.${z.aup.note ? ` ${esc(z.aup.note)}.` : ''}`);
    if (z.osm && Object.keys(z.osm).length) items.push(`OpenStreetMap estimate before replacement: ${luList(z.osm)}.`);
    if (z.replaced && Object.keys(z.replaced).length) items.push(`Replaced by official data: ${Object.entries(z.replaced).map(([k, r]) => `${luList({ [k]: (r || {}).q })} (${esc((r || {}).source || 'entered in Spare Construct')}; replaced by the ${esc((r || {}).by || 'official data')})`).join('; ')}.`);
    return items.length ? `<div class="ins-sec"><h4>Sources</h4>${items.map(t => Kit.note(t)).join('')}</div>` : '';
  }

  /* ---------- gateway inspector section (zone-gateway-ux (b)) ---------- */
  // 'Pick a count site on the map…': the pick waits for a count to be selected; the count's panel then offers it
  let gwPick = null;
  function gatewaySection(sc, sel) {
    const z = sc.zones[sel.id]; if (!z || z.kind !== 'gateway') return '';
    const counts = typeof DataHub !== 'undefined' ? DataHub.D().counts.length : 0, picking = !!(gwPick && gwPick.gw === z.id && gwPick.sc === sc.id);
    return `<div class="ins-sec"><h4>Count source ${gwBadge(z)}</h4>
      ${z.gwSrc ? Kit.note(`Site used: ${esc(gwSrcText(z))}.`) : Kit.note('These flows are estimated from the road class and lanes. Use nearest counts, or Pick a count site on the map… to set them from a count.')}
      <div class="actions"><button class="btn small" data-lp-do="gwcounts" ${counts ? '' : 'disabled'}>Use nearest counts</button>
        <button class="btn small" data-lp-do="gwpick" ${counts ? '' : 'disabled'} aria-pressed="${picking}">${picking ? 'Picking a count site…' : 'Pick a count site on the map…'}</button></div>
      ${picking ? Kit.note('Click a count site on the map, then press Use for this gateway in its panel.') : ''}
      ${counts ? '' : Kit.note('Load counts in the Traffic data tab first.')}</div>`;
  }
  /* Set a gateway's flows from one chosen count: DataHub.gatewaysFromCounts with every other count set aside for the
     call (restored after). The count must be on the gateway's road and class within 3 km, as for the nearest counts. */
  function useCount(sc, gwId, countId) {
    const z = sc.zones[gwId], d = DataHub.D(), c = d.counts.find(x => x.id === countId);
    if (!z || z.kind !== 'gateway') return { n: 0, error: 'That gateway is no longer in this scenario.' };
    if (!c) return { n: 0, error: 'That count is no longer loaded.' };
    if (c.off) return { n: 0, error: `${c.name || 'That count'} is switched off in Traffic data. Switch it on to use it.` };
    const was = d.counts.map(x => [x, 'off' in x, x.off]);
    let n = 0;
    try { for (const x of d.counts) if (x !== c) x.off = true; n = DataHub.gatewaysFromCounts({ ...sc, zones: { [z.id]: z } }); }
    finally { for (const [x, had, v] of was) { if (had) x.off = v; else delete x.off; } }
    return n ? { n, src: gwSrcText(z) } : { n: 0, error: `${c.name || 'That count'} cannot set ${z.name || 'this gateway'}'s flows: it must be on the gateway's road, of the same class, within 3 km (and its source switched on). Pick another count, or enter the flows by hand.` };
  }
  function countSection(sc, sel) {
    if (!gwPick || gwPick.sc !== sc.id) return '';
    const z = sc.zones[gwPick.gw]; if (!z) return '';
    return `<div class="ins-sec"><h4>Gateway flows from this count</h4>${Kit.note(`Set ${esc(z.name || 'the gateway')}'s AM and PM flows from this count only.`)}
      <div class="actions"><button class="btn small primary" data-lp-do="usecount">Use for ${esc(z.name || 'the gateway')}</button><button class="btn small" data-lp-do="cancelpick">Cancel</button></div></div>`;
  }

  /* ---------- zone placement card (zone-gateway-ux (a)); ui.js calls promptZone() from the zone tool ---------- */
  const CARD_USES = ['residential', 'apartment', 'retail', 'office', 'industrial', 'school'];
  function zoneCardHtml(k = '', q = '') {
    return `<p class="note">Choose what is planned here, then enter how much.</p><div class="chips" data-lp-card>${CARD_USES.map(u => `<button type="button" class="chip${u === k ? ' on' : ''}" data-lp-use="${u}">${esc(LANDUSES[u].label)}</button>`).join('')}
      <select data-lp-more aria-label="More land uses"><option value="">+ more</option>${Object.entries(LANDUSES).filter(([u]) => !CARD_USES.includes(u)).map(([u, L]) => `<option value="${u}" ${u === k ? 'selected' : ''}>${esc(L.label)}</option>`).join('')}</select></div>
      <label class="fld"><span>Quantity</span><span class="fld-in"><input id="lpCardQ" type="text" inputmode="decimal" value="${esc(q)}" aria-label="Quantity"><span class="unit" id="lpCardU">${k ? esc(LANDUSES[k].unit) : ''}</span></span></label>
      <p class="note" id="lpCardErr" hidden>Enter a number.</p><p class="note">Enter places the zone; Esc cancels.</p>`;
  }
  // The card's answer, checked the same way for Enter and for the Place zone button: { k, q } or { error }
  function cardCheck(k, raw) {
    if (!k || !LANDUSES[k]) return { error: 'Choose a land use.' };
    const q = parseNum(raw);
    return q > 0 ? { k, q } : { error: 'Enter a number.' };
  }
  /* → Promise of { k, q } or null (cancelled). A type and a number are required: Enter and the Place zone button
     both keep the card open with the message until they are given. The quantity box gets the focus. Listeners go on
     the card's own body and footer, which the next dialog replaces, so they never pile up on #modal. */
  function promptZone() {
    let k = '';
    const p = Kit.modal('What’s planned here?', zoneCardHtml(), [['Cancel', null, ''], ['Place zone', () => { const r = cardCheck(k, (U.$('#lpCardQ') || {}).value); return r.error ? null : r; }]]);
    setTimeout(() => {
      const m = typeof document !== 'undefined' && typeof document.querySelector === 'function' ? U.$('#modal') : null; if (!m) return;
      const body = m.querySelector('.m-body'), foot = m.querySelector('.m-foot'), qi = U.$('#lpCardQ'), er = U.$('#lpCardErr');
      if (!body || !qi) return;
      const setK = u => {
        k = u;
        body.querySelectorAll('[data-lp-use]').forEach(b => b.classList.toggle('on', b.dataset.lpUse === u));
        const more = body.querySelector('[data-lp-more]'); if (more && CARD_USES.includes(u)) more.value = '';
        const un = U.$('#lpCardU'); if (un) un.textContent = u && LANDUSES[u] ? LANDUSES[u].unit : '';
        if (er && !er.hidden && er.textContent === 'Choose a land use.') er.hidden = true;
      };
      const submit = () => {
        const r = cardCheck(k, qi.value);
        if (!r.error) { Kit.closeModal(r); return; }
        const typeMissing = r.error === 'Choose a land use.';
        qi.setAttribute('aria-invalid', typeMissing ? 'false' : 'true');
        if (er) { er.textContent = r.error; er.hidden = false; }
        const f = typeMissing ? body.querySelector('[data-lp-use]') : qi; if (f && f.focus) f.focus();
      };
      body.addEventListener('click', e => { const b = e.target.closest('[data-lp-use]'); if (b) { setK(b.dataset.lpUse); qi.focus(); } });
      body.addEventListener('change', e => { if (e.target.matches('[data-lp-more]')) { setK(e.target.value); qi.focus(); } });
      qi.addEventListener('keydown', e => { if (e.key === 'Enter') { if (e.preventDefault) e.preventDefault(); submit(); } });
      if (foot) { const close = foot.onclick; foot.onclick = e => { const b = e.target.closest('[data-mv]'); if (b && b.dataset.mv === '1') { submit(); return; } if (close) close(e); }; }
      qi.focus();
    }, 40);
    return p;
  }
  /* The zone tool's click: ask what is planned, then place the zone (nothing is placed on Cancel or Esc).
     ui.js: `else if (t === 'zone') LandusePlus.placeZone(sc, e.latlng, { undo });` → Promise of the zone or null. */
  async function placeZone(sc, ll, opts = {}) {
    const a = await promptZone();
    if (!a || !(a.q > 0) || !LANDUSES[a.k]) return null;
    const id = U.uid('z'), n = Object.values(sc.zones).filter(z => z.kind !== 'gateway').length + 1, L = LANDUSES[a.k];
    if (typeof Kit.snap === 'function') Kit.snap(`new zone Zone ${n}`); else Store.snapshot();
    const z = sc.zones[id] = { id, kind: 'zone', name: `Zone ${n}`, lat: U.r7(ll.lat), lng: U.r7(ll.lng), lu: { [a.k]: a.q }, connectors: null,
      luSrc: { [a.k]: { source: 'entered in Spare Construct', basis: 'user', date: today(), q: a.q } } };
    View.sel = { type: 'zone', id };
    Store.edited('demand');
    Kit.toast(`Zone ${n} added: ${n0(a.q)} ${L.unit} ${L.label.toLowerCase()}. Add more land use in the panel.`, '', opts.undo ? { action: { label: 'Undo', run: () => opts.undo() } } : undefined);
    return z;
  }

  /* ---------- the drawer tab ---------- */
  let view = 'sources';
  const fillOpts = { census: true, employees: true, schools: true, aup: true, newer: false, all: false };
  function tabHtml() {
    const P = Store.P, sc = Store.sc, nav = [['sources', 'Existing land use'], ['trips', 'Trip generation'], ['dev', 'Development traffic'], ['yield', 'Zoned yield']];
    const head = `<div class="bar-h"><h3>Land use and trips</h3><span class="seg" role="tablist">${nav.map(([k, l]) => `<button class="${view === k ? 'on' : ''}" data-lp-do="view:${k}" role="tab" aria-selected="${view === k}">${l}</button>`).join('')}</span></div>`;
    if (view === 'trips') {
      const scope = (P.landuse && P.landuse.scope) || 'dev';
      return `${head}<div class="actions"><label class="check"><input type="checkbox" data-lp-scope ${scope === 'all' ? 'checked' : ''}> Include existing and committed zones</label>
        <button class="btn small" data-lp-do="csv">Download trip_generation.csv</button></div>${tripTablesHtml(sc, { scope })}`;
    }
    if (view === 'dev') {
      const wo = withoutScenario(sc), others = P.scenarios.filter(s => s !== sc), d = distribution(sc), chosen = P.landuse && P.landuse.without && others.some(s => s.id === P.landuse.without);
      const opt = s => `<option value="${esc(s.id)}" ${chosen && wo === s ? 'selected' : ''}>${esc(s.name)} (${esc(Engine.yearOf(s, P.settings))})</option>`;
      return `${head}<h4>T5 Development traffic by gateway</h4>${t5Html(sc)}${d ? distFigure(sc, d) : ''}
        <h4>Without vs with development</h4>${others.length ? Kit.fld('Without development', `<select data-lp-without><option value="" ${chosen ? '' : 'selected'}>Automatic: same-year baseline${!chosen && wo ? ` (${esc(wo.name)})` : ' (none)'}</option>${others.map(opt).join('')}</select>`) : ''}${wwHtml(sc, wo)}`;
    }
    if (view === 'yield') return `${head}<h4>Zoned capacity in the study area</h4>${yieldTableHtml(true)}`;
    const last = P.landuse && P.landuse.last, B = areaBox(sc);
    const ck = (k, label) => `<label class="check"><input type="checkbox" data-lp-opt="${k}" ${fillOpts[k] ? 'checked' : ''}> ${label}</label>`;
    return `${head}<p class="note">Fill the existing (base-year) zones from official data. Spare Construct downloads building outlines from OpenStreetMap, the Census 2023 for each SA1, Stats NZ employee counts for each SA2, the Ministry of Education school and early learning directory and the Unitary Plan zones, then uses the better source for each item (below). Every value keeps its source. Development and committed zones are never changed.</p>
      ${B ? Kit.note(`Area to fill: ${esc(boxText(B))}, from ${esc(B.basis)}.`) : Kit.note('Add roads or zones, or draw the study area, so Spare Construct knows the area to fill.', true)}
      <div class="actions">${ck('census', 'Census dwellings and travel')}${ck('employees', 'Employees')}${ck('schools', 'Schools and early learning')}${ck('aup', 'Unitary Plan zones')}${ck('newer', 'Keep the OSM dwelling count where it is clearly higher')}${P.scenarios.length > 1 ? ck('all', 'All scenarios') : ''}
        <button class="btn small primary" data-lp-do="fill">Fill existing land use</button></div>
      ${last ? lastHtml(last) : Kit.note('Nothing filled yet in this project.')}`;
  }
  function lastHtml(o) {
    const parts = [], arr = a => (Array.isArray(a) ? a : []);
    parts.push(T({ head: ['Item', 'Source used', 'Instead of', 'Why', 'This area'], rows: arr(o.choices || CHOICES).map(c => [esc(c.item), esc(c.use), esc(c.instead), esc(c.why), c.value ? esc(c.value) : '–']) }));
    if (o.bounds && box(o.bounds)) parts.push(Kit.note(`Area filled: ${esc(boxText(box(o.bounds)))}${o.bounds.basis ? `, from ${esc(o.bounds.basis)}` : ''}${+o.scenarios > 1 ? `, in ${n0(+o.scenarios)} scenarios` : ''}.`));
    const c = o.census;
    if (c) parts.push(`<details><summary>Census 2023: ${n0(arr(c.rows).length)} SA1s${c.dwellings === false ? ', dwellings not downloaded (left as they were)' : `, ${n0(c.placed)} dwellings placed (expected ${n1(c.expected)})`}${+c.created ? `, ${n0(+c.created)} new zones` : ''}. Counts are random-rounded to base 3.</summary>
      ${T({ head: ['SA1', { label: 'Occupied dwellings', n: true }, { label: 'In study area', n: true }, { label: 'Placed', n: true }, { label: 'Houses', n: true }, { label: 'Apartments', n: true }, 'Basis'],
        rows: arr(c.rows).map(r => [esc(r.code), r.occ == null ? 'suppressed' : n0(r.occ), U.fmt.num(r.inShare * 100, 0) + '%', n0(r.placed), n0(r.houses), n0(r.apartments), esc(r.basis + (r.occBasis !== 'occupied private dwellings' ? '; ' + r.occBasis : ''))]) })}</details>`);
    const fl = arr(c && c.flagged);
    if (fl.length) parts.push(Kit.note(`${n0(fl.length)} zone${fl.length > 1 ? 's show' : ' shows'} more dwellings in OpenStreetMap than in the 2023 Census, probably homes built since: ${fl.slice(0, 6).map(f => `${esc(f.name)} (OSM ${n0(f.osm)}, Census ${n0(f.census)}${f.kept ? ', OSM kept' : ''})`).join('; ')}. ${esc(STD.census.newerLabel)}.`, true));
    const e = o.employees;
    if (e) parts.push(`<details><summary>Employees ${esc(e.year)}: ${n0(e.allocated)} placed in zones${e.growth != null ? `; Stats NZ average annual change ${U.fmt.num(e.growth, 1)}% a year (ec_avperinc; information only, not applied: Spare Construct's trips come from floor area)` : ''}</summary>
      ${T({ head: ['SA2', { label: 'Employees', n: true }, { label: 'In study area', n: true }, { label: 'Placed', n: true }, { label: 'Outside', n: true }, { label: 'No floor area', n: true }],
        rows: arr(e.rows).map(r => [esc(r.name), n0(r.n), U.fmt.num(r.inShare * 100, 0) + '%', n0(r.allocated), n0(r.outside), n0(r.unplaced)]) })}${Kit.note(esc(e.check))}</details>`);
    const s = o.schools;
    if (s) {
      const need = arr(s.needsRoll), loaded = s.loaded || { schools: true, ece: true };
      parts.push(Kit.note(`Ministry of Education: ${loaded.schools === false ? 'the school directory did not download' : `${n0(arr(s.schools).length)} schools (${n0(+s.created || 0)} new zones)`}, ${loaded.ece === false ? 'the early learning directory did not download' : `${n0(+s.ece || 0)} early learning services`}.${need.length ? ` No roll in the directory for ${need.map(x => esc(x.name)).join(', ')}: <button class="btn small" data-lp-do="rolls">Enter rolls…</button>` : ''}${arr(s.skipped).length ? ` Left out: ${esc(arr(s.skipped).join('; '))}.` : ''}`, !!need.length));
    }
    if (+o.builtFromOSM) parts.push(Kit.note(`${n0(+o.builtFromOSM)} existing zones were first built from OpenStreetMap buildings (indicative).`));
    if (arr(o.warnings).length) parts.push(o.warnings.map(w => Kit.note(esc(w), true)).join(''));
    return `<h4>Last filled ${esc(o.at)}</h4>${parts.join('')}`;
  }
  // Undo snapshots for every scenario a fill changes; the active one last, so the first Undo restores it
  function snapAll(targets, active = Store.sc, what = 'fill existing land use') {
    const P = Store.P, keep = P.active, list = [...targets.filter(s => s !== active), ...targets.filter(s => s === active)];
    for (const s of list) { P.active = s.id; if (typeof Kit.snap === 'function') Kit.snap(`${what} (${s.name})`); else Store.snapshot(); }
    P.active = keep;
    return list.length;
  }
  async function runFill() {
    const sc = Store.sc;
    Kit.busy('Filling existing land use…');
    let out;
    try {
      out = await fillAll(sc, { which: { census: fillOpts.census, employees: fillOpts.employees, schools: fillOpts.schools, aup: fillOpts.aup }, newer: fillOpts.newer ? 'osm' : 'flag', all: fillOpts.all,
        beforeApply: targets => snapAll(targets, sc) }, t => Kit.busy(t));
    } catch (e) { Kit.busy(null); Kit.toast(e.message, 'err'); return; }
    Kit.busy(null);
    syncAll();
    Store.edited('demand');
    const c = out.census, many = out.scenarios > 1 ? ` in ${out.scenarios} scenarios (Undo restores one scenario at a time)` : '';
    Kit.toast(c && c.dwellings !== false ? `Filled ${n0(c.placed)} dwellings from the Census${out.schools ? `, ${out.schools.schools.length} schools` : ''}${out.employees ? `, ${n0(out.employees.allocated)} employees` : ''}${many}.` : `Existing land use filled${many}.`, out.warnings.length ? 'warn' : '');
    if (out.schools && out.schools.needsRoll.length) askRolls();
  }
  // Ask for the missing rolls and add those schools (works after the project is reopened); says why when it cannot
  async function askRolls() {
    const ready = rollsReady();
    if (!ready.list.length) { Kit.toast(ready.why, 'warn'); return; }
    const rolls = {};
    for (const s of ready.list) {
      const v = await Kit.ask(`Roll for ${s.name} (${s.type}${s.status && s.status !== 'Open' ? ', ' + String(s.status).toLowerCase() : ''}): the directory has none`, '');
      if (v == null) continue;
      const n = parseNum(v);
      if (n > 0) rolls[s.id] = n; else Kit.toast(`Enter a number of students for ${s.name}.`, 'warn');
    }
    if (!Object.keys(rolls).length) return;
    // a fill across every scenario put the other schools in every scenario, so these go there too
    const last = Store.P.landuse && Store.P.landuse.last, targets = last && +last.scenarios > 1 ? Store.P.scenarios.slice() : [Store.sc];
    if (!rollsReady(rolls).list.length) { Kit.toast(rollsReady(rolls).why, 'warn'); return; }
    const n = snapAll(targets, Store.sc, 'school rolls');
    const r = applyRolls(targets, rolls);
    if (!r) { Store.undoStack.splice(-n, n); Kit.toast(rollsReady(rolls).why, 'warn'); return; }
    Store.edited('demand');
    Kit.toast(`Added ${r.schools.length} school${r.schools.length === 1 ? '' : 's'} with the rolls you entered${targets.length > 1 ? ` in ${targets.length} scenarios` : ''}.`);
  }
  function tabClick(e) {
    const b = e.target.closest('[data-lp-do]'); if (!b) return;
    const d = b.dataset.lpDo;
    if (d.startsWith('view:')) { view = d.slice(5); Drawer.render(); return true; }
    if (d === 'fill') { runFill(); return true; }
    if (d === 'rolls') { askRolls(); return true; }
    if (d === 'csv') { U.download(`${UI.fileBase()}-trip_generation.csv`, csv(Store.sc, Store.S, { scope: (Store.P.landuse || {}).scope }), 'text/csv'); return true; }
  }
  function tabChange(e) {
    const t = e.target, P = Store.P;
    if (t.dataset.lpOpt) { fillOpts[t.dataset.lpOpt] = t.checked; return true; }
    if (t.matches('[data-lp-scope]')) { (P.landuse ||= {}).scope = t.checked ? 'all' : 'dev'; Store.emit('meta'); Drawer.render(); return true; }
    if (t.matches('[data-lp-without]')) { const L = P.landuse ||= {}; if (t.value) L.without = t.value; else delete L.without; Store.emit('meta'); Drawer.render(); return true; }
    if (t.dataset.lpProposed) {
      const L = P.landuse ||= {}, pr = L.proposed ||= {}, code = t.dataset.lpProposed;
      if (t.value && YIELDS[+t.value]) pr[code] = +t.value; else delete pr[code];
      Store.emit('meta'); Drawer.render(); return true;
    }
    if (t.dataset.lpYield) {
      const [code, k] = t.dataset.lpYield.split('.');
      const Y = ((P.landuse ||= {}).yields ||= {}), o = Y[code] ||= {};
      if (k === 'use') { if (GFA_USES.includes(t.value)) o.use = t.value; else delete o.use; }
      else {
        const v = parseNum(t.value);
        if (Number.isNaN(v) || (v != null && v < 0) || (k === 'netPct' && v > 100)) { t.setAttribute('aria-invalid', 'true'); Kit.toast(k === 'netPct' ? 'Enter a developable share from 0 to 100%.' : 'Enter a number.', 'warn'); return true; }
        t.removeAttribute('aria-invalid');
        if (v == null) delete o[k]; else o[k] = v;
      }
      if (!Object.keys(o).length) delete Y[code];
      Store.emit('meta'); Drawer.render(); return true;
    }
  }

  /* ---------- inspector events ---------- */
  function setLp(z, path, raw, type, sc = Store.sc) {
    let v = raw;
    if (type === 'num') { v = parseNum(raw); if (Number.isNaN(v) || (v != null && v < 0)) return { error: 'Enter a number' }; }
    if (type === 'bool') v = !!raw;
    if (path === 'tg.on') { setTable(z, v, Store.S, sc); return { ok: true }; }
    if (path.startsWith('tg.') && !(z.tg && z.tg.on) && !DESCRIBES.test(path)) setTable(z, true, Store.S, sc);
    if (path.startsWith('lu.')) {
      const k = path.slice(3); z.lu ||= {};
      if ((+z.lu[k] || 0) === (v || 0)) return { ok: true };   // the same quantity again keeps its source
      if (v == null || v === 0) delete z.lu[k]; else z.lu[k] = v;
      (z.luSrc ||= {})[k] = { source: 'entered in Spare Construct', basis: 'user', date: today(), q: v || 0 };
      return { ok: true };
    }
    if (path.startsWith('tg.rows.') && path.endsWith('.basis') && v === 'person') {
      // switching to person trips keeps the vehicle trips: person rate = vehicle rate × occupancy ÷ car share
      const k = path.split('.')[2], r = ((z.tg.rows ||= {})[k] ||= {});
      if (!r.person) { r.person = {}; for (const p of PERIODS) { const t = tripGen(z, Store.S, p, sc).rows.find(x => x.k === k); if (t && t.person != null) r.person[p] = U.r3(t.person / (z.lu[k] / LANDUSES[k].per)); } }
    }
    const parts = path.split('.'), last = parts.pop();
    let o = z; for (const p of parts) { if (o[p] == null || typeof o[p] !== 'object') o[p] = {}; o = o[p]; }
    if (v == null || v === '' || v === false) delete o[last]; else o[last] = v;
    return { ok: true };
  }
  const zoneState = z => JSON.stringify([z.lu, z.tg, z.extra, z.luSrc, z.tgLoad]);
  // Saved on change: checked first (a bad number marks the box and changes nothing), tried on a copy, and only a real
  // change takes an undo snapshot and marks the results out of date
  function zoneChange(e, el) {
    const t = e.target, sc = Store.sc, sel = View.sel, z = sel && sc.zones[sel.id];
    if (!z) return;
    if (t.dataset.lp) {
      const err = t.parentElement && t.parentElement.parentElement && t.parentElement.parentElement.querySelector('.lp-err');
      const raw = t.type === 'checkbox' ? t.checked : t.value, path = t.dataset.lp, type = t.dataset.lpt;
      const copy = JSON.parse(JSON.stringify(z)), r = setLp(copy, path, raw, type, sc);
      if (r.error) { t.setAttribute('aria-invalid', 'true'); if (err) { err.textContent = r.error; err.hidden = false; } return true; }
      t.removeAttribute('aria-invalid'); if (err) err.hidden = true;
      sync(copy, Store.S, sc);
      if (zoneState(copy) === zoneState(z)) return true;
      if (typeof Kit.snap === 'function') Kit.snap(`land use of ${z.name || 'zone'}`); else Store.snapshot();
      setLp(z, path, raw, type, sc);
      sync(z, Store.S, sc);
      Store.edited('demand');
      return true;
    }
    if (t.matches('[data-lp-yc],[data-lp-yha]')) return true;
  }
  // Live trip table while typing (zone-gateway-ux (c)); nothing is saved until the box changes
  function zoneInput(e, el) {
    const t = e.target, sc = Store.sc, sel = View.sel, z = sel && sc.zones[sel.id];
    if (!z || !t.dataset.lp || t.dataset.lpt !== 'num') return;
    const v = parseNum(t.value), err = t.parentElement && t.parentElement.parentElement && t.parentElement.parentElement.querySelector('.lp-err');
    if (Number.isNaN(v)) { t.setAttribute('aria-invalid', 'true'); if (err) { err.textContent = 'Enter a number'; err.hidden = false; } return true; }
    t.removeAttribute('aria-invalid'); if (err) err.hidden = true;
    const copy = JSON.parse(JSON.stringify(z));
    setLp(copy, t.dataset.lp, t.value, 'num', sc);
    const box = el.querySelector('[data-lp-trips]');
    if (box) box.innerHTML = zoneTripsHtml(copy, sc, Store.S);
    return true;
  }
  function zoneClick(e, el) {
    const b = e.target.closest('[data-lp-do]'); if (!b) return;
    const sc = Store.sc, z = View.sel && sc.zones[View.sel.id]; if (!z) return true;
    if (b.dataset.lpDo === 'yield') {
      const code = +el.querySelector('[data-lp-yc]').value, ha = parseNum(el.querySelector('[data-lp-yha]').value);
      if (!code) { Kit.toast('Choose the Unitary Plan zone first.', 'warn'); return true; }
      if (!(ha > 0)) { el.querySelector('[data-lp-yha]').setAttribute('aria-invalid', 'true'); Kit.toast('Enter the site area in hectares.', 'warn'); return true; }
      if (typeof Kit.snap === 'function') Kit.snap(`zoned yield for ${z.name || 'zone'}`); else Store.snapshot();
      const y = applyYield(z, code, ha);
      sync(z, Store.S, sc);
      Store.edited('demand');
      Kit.toast(`Filled from the zoned yield: ${Object.entries(y.lu).map(([k, v]) => `${n0(v)} ${LANDUSES[k].unit} ${LANDUSES[k].label.toLowerCase()}`).join(', ') || 'no land use'}. ${y.assumed ? STD.yields.assumed : y.label}.`);
      return true;
    }
    return true;
  }
  function gatewayClick(e) {
    const b = e.target.closest('[data-lp-do]'); if (!b) return;
    const sc = Store.sc, z = View.sel && sc.zones[View.sel.id]; if (!z) return true;
    if (b.dataset.lpDo === 'gwcounts') {
      Store.snapshot();
      const n = DataHub.gatewaysFromCounts({ ...sc, zones: { [z.id]: z } });
      if (n) { Store.edited('demand'); Kit.toast(`${z.name}: flows set from ${gwSrcText(z)}.`); }
      else { Store.undoStack.pop(); Kit.toast('No count on this road within 3 km. Pick a count site on the map, load more counts, or enter flows by hand.', 'warn'); }
      return true;
    }
    if (b.dataset.lpDo === 'gwpick') {
      gwPick = gwPick && gwPick.gw === z.id && gwPick.sc === sc.id ? null : { sc: sc.id, gw: z.id };
      if (gwPick) { View.show.counts = true; Kit.toast(`Click a count site on the map, then press Use for ${z.name || 'the gateway'} in its panel.`); }
      Store.emit('data');
      return true;
    }
  }
  function countClick(e) {
    const b = e.target.closest('[data-lp-do]'); if (!b || !gwPick) return;
    const sc = Store.sc, sel = View.sel;
    if (b.dataset.lpDo === 'cancelpick') { gwPick = null; Store.emit('meta'); return true; }
    if (b.dataset.lpDo !== 'usecount' || !sel || sel.type !== 'count') return;
    const gw = gwPick.gw;
    Store.snapshot();
    const r = useCount(sc, gw, sel.id);
    if (!r.n) { Store.undoStack.pop(); Kit.toast(r.error, 'warn'); return true; }
    gwPick = null;
    Store.edited('demand');
    Kit.toast(`${sc.zones[gw].name || 'Gateway'}: flows set from ${r.src}.`);
    if (typeof UI !== 'undefined' && typeof UI.select === 'function') UI.select({ type: 'gateway', id: gw });
    return true;
  }

  /* ---------- report sections (Report builds the document from Registry.list('report')) ---------- */
  // Inside the report (ctx.table present) table headings become numbered captions and the figure gets a figure number
  const numbered = (html, ctx) => (ctx && typeof ctx.table === 'function'
    ? html.replace(/<h4>([^<]*)<\/h4>/g, '<p class="tcap"><b><span class="tno">%%TNO%%</span>$1</b></p>').replace(/(<svg class="mini-chart lp-dist"[\s\S]*?<\/svg>)/, '<figure class="fig">$1<figcaption>%%FNO%%Development traffic distribution by gateway</figcaption></figure>')
    : html);
  // The assessed scenario: the report's 'with' selection, else ctx.scenarioId, else the active one
  const scOf = (P, ctx = {}) => (ctx.sel && ctx.sel.with) || (ctx.scenarioId && P.scenarios.find(s => s.id === ctx.scenarioId)) || Store.sc;
  function reportExisting(P, ctx) {
    const o = P.landuse && P.landuse.last, arr = a => (Array.isArray(a) ? a : []);
    if (!o) return '<h3>Existing land use</h3><p>Existing land use was not filled from official data for this report.</p>';
    const src = Object.values(P.landuse.sources || {}).map(s => [esc(s.name), esc(s.vintage || ''), esc(s.licence || ''), esc(s.at || ''), n0(s.n)]);
    const c = o.census, e = o.employees, fl = arr(c && c.flagged);
    const html = `<h3>Existing land use</h3><p>Base-year zones were filled from official data on ${esc(o.at)}${o.bounds && box(o.bounds) ? `, for an area of ${esc(boxText(box(o.bounds)))}${o.bounds.basis ? ` (${esc(o.bounds.basis)})` : ''}` : ''}. Development and committed zones were not changed. The table gives the source used for each item and why.</p>
      <h4>Source used for each item</h4>${T({ head: ['Item', 'Source used', 'Instead of', 'Why'], rows: arr(o.choices || CHOICES).map(x => [esc(x.item), esc(x.use), esc(x.instead), esc(x.why)]) })}
      <h4>Land-use data sources</h4>${T({ head: ['Data', 'Vintage', 'Licence', 'Downloaded', { label: 'Records', n: true }], rows: src })}
      ${c ? (c.dwellings === false ? `<p>Census 2023: the dwellings layer did not download, so dwellings are the OpenStreetMap estimate; people, households, vehicles and travel come from ${n0(arr(c.rows).length)} SA1s. Counts are random-rounded to base 3.</p>`
        : `<p>Census 2023: ${n0(c.placed)} occupied dwellings placed in zones from ${n0(arr(c.rows).length)} SA1s (${n1(c.expected)} expected inside the study area). Counts are random-rounded to base 3.</p>`) : ''}
      ${e ? `<p>Employees (${esc(e.year)}): ${n0(e.allocated)} placed by floor area. ${esc(e.check)}</p>` : ''}
      ${fl.length ? `<p>${n0(fl.length)} zones show more dwellings in OpenStreetMap than in the Census (likely homes built since March 2023); ${esc(STD.census.newerLabel)}.</p>` : ''}`;
    return numbered(html, ctx);
  }
  const reportTrips = (P, ctx = {}) => {
    const sc = scOf(P, ctx);
    return `<h3>Trip generation</h3><p>Trips for ${esc(sc.name)} follow ${esc(STD.ita.name)}: person trips by activity, mode share, vehicle trips and the adjustments in sequence.</p>${numbered(tripTablesHtml(sc, { scope: (P.landuse || {}).scope }), ctx)}`;
  };
  // Inside the report the 'without' scenario is the report's own choice (ctx.sel.without, same-year by Report.pick)
  const reportDist = (P, ctx = {}) => {
    const sc = scOf(P, ctx), d = distribution(sc), inRep = !!(ctx.sel && 'without' in ctx.sel), wo = inRep ? ctx.sel.without : withoutScenario(sc, P);
    const ww = wo ? wwHtml(sc, wo) : inRep ? Kit.note(`The report has no scenario without the development for ${esc(Engine.yearOf(sc, P.settings))}. Choose one in the Report tab (Scenarios › Without), and run it, to compare junctions without and with the development.`, true) : wwHtml(sc, null);
    return numbered(`<h3>Development traffic distribution</h3><h4>T5 Development traffic by gateway</h4>${t5Html(sc)}${d ? distFigure(sc, d) : ''}<h3>Junctions without and with the development</h3><h4>Without vs with development, by junction</h4>${ww}`, ctx);
  };
  const reportYield = (P, ctx) => numbered(`<h3>Zoned capacity</h3><p>Full build-out uses the zoned capacity of every zone in the study area that has a yield; zones without one are listed. Future Urban land is a structure-plan assumption, shown separately and not counted as zoned capacity. ${esc(STD.yields.label)}.</p><h4>Zoned capacity at full build-out</h4>${yieldTableHtml(false)}`, ctx);

  /* ---------- checklist ---------- */
  function ckRates(P) {
    const sc = Store.sc, R = tables(sc, Store.S, { scope: 'dev' });
    if (!R.zones.length) return { state: 'todo', text: 'Add development zones to generate trips', action: { label: 'Open', run: () => { view = 'trips'; UI.openTab('landuse-trips'); } } };
    const rev = R.flags.filter(f => f.level === 'review').length, und = R.flags.filter(f => f.level === 'warn').length;
    return rev || und ? { state: 'warn', text: `${rev ? `${rev} rate${rev > 1 ? 's' : ''} need review (AT ITA App 1(iv))` : ''}${rev && und ? '; ' : ''}${und ? `${und} undated default rate${und > 1 ? 's' : ''}` : ''}`, action: { label: 'Open', run: () => { view = 'trips'; UI.openTab('landuse-trips'); } } }
      : { state: 'done', text: 'Every development rate has a source and a current year' };
  }
  function ckSanity() {
    const s = tables(Store.sc, Store.S, { scope: 'dev' }).sanity.filter(x => x.ok != null);
    if (!s.length) return { state: 'na', text: 'No residential development' };
    const bad = s.filter(x => !x.ok);
    return bad.length ? { state: 'warn', text: `Residential trips per dwelling outside ${STD.rr422.band.join('–')}: ${bad.map(x => `${x.period} ${U.fmt.num(x.rate, 2)}`).join(', ')}` } : { state: 'done', text: `Residential trips per dwelling within ${STD.rr422.band.join('–')} (RR 422)` };
  }
  // The checklist tab prints the text as HTML, so the saved date is escaped
  const ckExisting = P => (P.landuse && P.landuse.last ? { state: 'done', text: `Filled from official data on ${esc(P.landuse.last.at)}` }
    : { state: 'todo', text: 'Fill existing land use from the Census, employee counts and the MoE directory', action: { label: 'Open', run: () => { view = 'sources'; UI.openTab('landuse-trips'); } } });

  /* ---------- registration ---------- */
  Registry.inspectorSections({ id: 'landuse-zone', types: ['zone'], order: 20, render: zoneSection, onChange: zoneChange, onInput: zoneInput, onClick: zoneClick });
  Registry.inspectorSections({ id: 'landuse-gateway', types: ['gateway'], order: 20, render: gatewaySection, onClick: gatewayClick });
  Registry.inspectorSections({ id: 'landuse-count-pick', types: ['count'], order: 5, render: countSection, onClick: countClick });
  // stamp: a human-readable table (the compliance feature adds its D1 / version footer); authorText: it carries the
  // rate sources the author typed, so the scope linter can hold it back
  Registry.exports({ id: 'landuse-tripgen', label: 'Trip generation (CSV)', ext: 'csv', mime: 'text/csv', order: 70, stamp: true, authorText: true, enabled: c => !!c.sc, filename: c => `${c.fileBase}-trip_generation.csv`, build: (P, c) => csv(c.sc, P.settings, { scope: (P.landuse || {}).scope }) });
  Registry.menu({ id: 'landuse-open', label: 'Existing land use and trip generation…', order: 150, run: () => { view = 'sources'; UI.openTab('landuse-trips'); } });
  Registry.settings({ id: 'landuse', title: 'Land use and trip generation', order: 30, defaults: DEFAULTS,
    render: s => { const c = conf(s), sum = MODES.reduce((a, m) => a + Math.max(0, +c.split[m] || 0), 0);
      return `<p class="note">Default mode split where a zone has no Census data or entry of its own (${esc(STD.split.label)}), and the source and survey year of each trip rate. ${esc(STD.ita.name)} asks for rates newer than 2022.</p>
        <div class="set-grid">${MODES.map(m => `<label class="fld"><span>${MODE_LABEL[m]}</span><span class="fld-in"><input type="number" min="0" max="100" step="1" data-set="landuse.split.${m}" value="${esc(U.r1(+c.split[m] || 0))}"><span class="unit">%</span></span></label>`).join('')}
        <label class="fld"><span>Students per car (education)</span><span class="fld-in"><input type="number" min="1" max="5" step="0.1" data-set="landuse.eduOcc" value="${esc(c.eduOcc)}"></span></label></div>
        ${Math.abs(sum - 100) > 0.5 ? Kit.note(`The five mode shares add up to ${esc(U.fmt.num(sum, 0))}%. Spare Construct rescales them to 100% when it uses them; change them so they add up to 100%.`, true) : ''}
        <div class="tbl-wrap"><table class="t compact"><tr><th>Land use</th><th>Rate source</th><th>Survey year</th></tr>${Object.entries(LANDUSES).map(([k, L]) => `<tr><td>${esc(L.label)}</td>
          <td><input type="text" style="width:220px" data-set="landuse.rateMeta.${k}.source" value="${esc((c.rateMeta[k] || {}).source || '')}" placeholder="${esc(DEFAULT_RATE_SOURCE)}" aria-label="Trip rate source for ${esc(L.label)}"></td><td><input type="number" min="1950" max="2100" style="width:80px" data-set="landuse.rateMeta.${k}.year" value="${esc((c.rateMeta[k] || {}).year ?? '')}" aria-label="Survey year for ${esc(L.label)} trip rates"></td></tr>`).join('')}</table></div>`; } });
  Registry.about({ id: 'landuse', title: 'Existing land use and trip generation', order: 30, intro: 'Downloaded for the study area when you fill existing land use; nothing is bundled.',
    rows: () => [
      ['Dwellings, people, vehicles, travel to work', `<a href="${SRC.census.url}" target="_blank" rel="noopener">${esc(SRC.census.name)}</a>`, `<b>Real.</b> ${esc(SRC.census.vintage)}; random-rounded to base 3. ${esc(SRC.census.licence)}.`],
      ['Employees', `<a href="${SRC.employees.url}" target="_blank" rel="noopener">${esc(SRC.employees.name)}</a>`, `<b>Real.</b> Latest year in the layer; placed in zones by floor area. ${esc(SRC.employees.licence)}.`],
      ['School rolls, early learning places', `<a href="${MOE_URL}" target="_blank" rel="noopener">Ministry of Education directory</a>`, `<b>Real.</b> Roll dates per school. ${esc(SRC.schools.licence)}.`],
      ['Zoning', `<a href="${SRC.aup.url}" target="_blank" rel="noopener">${esc(SRC.aup.name)}</a>`, `<b>Real.</b> ${esc(SRC.aup.licence)}.`],
      ['Default yields', 'Spare Construct', `<b>Assumed.</b> ${esc(STD.yields.label)}. Future Urban land: ${esc(STD.yields.assumed)}.`],
      ['Default mode split', 'Spare Construct', `<b>Assumed.</b> ${esc(STD.split.label)}.`],
    ] });
  Registry.onStore(kind => {
    if (kind === 'project') gwPick = null;
    if (['demand', 'settings', 'undo', 'project', 'scenario'].includes(kind)) syncAll();
  });

  // Registered at start-up rather than at load: test/registry.test.js checks the built-in tab, report and checklist lists
  let late = false;
  function registerLate() {
    if (late) return; late = true;
    Registry.tabs({ id: 'landuse-trips', label: 'Land use', order: 62, title: 'Existing land use, trip generation, development traffic and zoned yield', render: tabHtml, onClick: tabClick, onChange: tabChange });
    Registry.checklist({ id: 'landuse-existing', group: 'Existing environment', order: 20, label: 'Existing land use from official data', source: 'Stats NZ Census 2023; MoE directory', status: ckExisting });
    Registry.checklist({ id: 'landuse-rates', group: 'Trip generation', order: 10, label: 'Trip rates sourced and current', source: 'AT ITA Guidelines 2025 v2 App 1(iv)', status: ckRates });
    Registry.checklist({ id: 'landuse-sanity', group: 'Trip generation', order: 20, label: 'Residential trips per dwelling checked', source: 'NZTA RR 422', status: ckSanity });
    Registry.report({ id: 'landuse-existing', chapter: 'Existing environment', order: 30, title: 'Existing land use', render: reportExisting });
    Registry.report({ id: 'landuse-tripgen', chapter: 'Trip generation', order: 10, title: 'Trip generation', render: reportTrips });
    Registry.report({ id: 'landuse-distribution', chapter: 'Trip distribution and assignment', order: 10, title: 'Development traffic distribution', render: reportDist });
    Registry.report({ id: 'landuse-yield', chapter: 'Plan change', order: 20, title: 'Zoned capacity', render: reportYield });
  }
  Registry.onInit(() => { registerLate(); syncAll(); if (typeof Drawer !== 'undefined') Drawer.render(); });

  return {
    SRC, STD, YIELDS, CHOICES, CENSUS_VARS, MODES, CSV_HEAD,
    parseNum, box, areaBox, lr, sampleShare, resolve, getJSON,
    fetchCensus, fetchEmployees, fetchMoE, fetchAUP, parseCensus, parseEmployees, parseMoE, parseAUP,
    prepBuildings, censusToZones, employeesToZones, schoolsToZones, aupToZones, apply, fillAll, applyRolls, rollsReady, record,
    sharesOf, splitFor, rateFlag, tripGen, sync, syncAll, setTable, tgNative, tables, csv,
    yieldOf, zonedCapacity, applyYield, distribution, distFigure, withWithout, withoutScenario, useCount, gwSrcText,
    zoneSection, gatewaySection, countSection, zoneTripsHtml, zoneCardHtml, cardCheck, promptZone, placeZone, tabHtml, tripTablesHtml, t5Html, wwHtml, yieldTableHtml,
    reportExisting, reportTrips, reportDist, reportYield, registerLate, setView: v => { view = v; }, clearCache: () => cache.clear(),
    pickGateway: (scId, gwId) => { gwPick = scId && gwId ? { sc: scId, gw: gwId } : null; },
  };
})();
