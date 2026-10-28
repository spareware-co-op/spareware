'use strict';
/* Spare Construct · official data feeds (feature file, lane 'feeds'). A "Data sources" section in the Traffic data tab
   fetches free, CORS-enabled official layers for the study area and records where every number came from:
   - NZTA National Speed Limit Register (current versions only; gazetted future limits for a future scenario year)
     → L.speedLimit / L.nslr on links, a "Legal vs modelled speed" map mode, and a review before modelled speeds change;
   - AT controlled intersections and AT regulatory signs (RG 5 stop, RG 6 give way, RG 6R roundabout) → proposed
     junction controls and AT site numbers, applied only after the user ticks them in a review list (a sign never
     downgrades signals or a roundabout);
   - OpenStreetMap turn restrictions (relation type=restriction via a node) → banned movements (node.bans), after review;
   - NZTA TMS daily counts (deduplicated with groupBy + max) → weekday means with a 95% confidence interval, %HCV,
     day-of-week and monthly factors, stored on the NZTA count as c.tms (and c.share when peak profiles are installed).
     The count gets obs_method 'weekday_factored' only once DataHub.periodVol turns the TMS mean into its peak volume;
     until then the model uses the NZTA AADT and the panel says so;
   - Auckland Council AGS23 v1.1 → household, job and population growth factors per zone for forecast years, and the
     growth beyond each gateway (fetched live for the study area, never bundled or saved raw);
   - GTFS bus timetables through the Mobility Database mirror (zip read with DecompressionStream, in a Web Worker when
     the browser allows) → bus routes with AM and PM frequencies, which the model preloads on links and junctions;
   - council ArcGIS count layers (Hamilton and QLDC presets, or any URL) with field mapping the user confirms;
   - Auckland Council flood plains and coastal inundation → length of each link in each hazard, and 'only access';
   - the NZTA Motor Vehicle Register fleet mix for a district.
   Every request has an AbortController time limit and fails soft with a message that says what to do. Query date,
   source, licence and record count go into Store.P.feeds.vintage (the data-vintage table and About the data).

   Deviations from the specs, on purpose:
   - ArcGIS layers are queried as f=json, not f=geojson (same attributes and geometry, one paged helper).
   - TMS peak profiles: data/tms_profiles.json is not in the repo (it needs the 1.7 GB quarter-hourly archive and a
     maintainer script). Spare Construct reads it when present; otherwise peak hours use the shares in Settings.
   - GTFS routes keep both peaks in r.freqs (the engine reads r.freqs[period], else r.freq, the busier peak) and the
     engine counts settings.busPcu per bus (2.0 for new projects; projects saved before it keep 3).
   - Count estimates (RAMM road centreline AADT) carry src 'ramm' (DataHub: weight 0.25, never validation); this file
     imports none.
   - Fleet mix: "Use for CO₂" sets settings.co2Factor (the engine's fleet-mix hook, a multiplier on the light-vehicle
     CO₂ curve) to the share of registered cars and vans that are not battery electric (tailpipe CO₂ only; hybrids
     and plug-in hybrids count as fuel-burning: Spare Construct assumption), with settings.co2FactorSrc saying where it came from.
   - The AGS23 fallback outside Auckland (NZTA Projected_Population_Growth) is not built: zones outside Auckland get
     no factor and say so.
   - GTFS caching uses IndexedDB when available, keyed by feed, study area and Last-Modified; a cached reading is used
     while its service day is still ahead. The service day is a school-term Tuesday where Spare Construct has the term dates.
   - NSLR coverage: the live register (checked 9 Oct 2026 by the first build) has no current version for most Drury
     roads, because its Urban Traffic Area and Default Area versions all ended in 2023. Ended versions are never used,
     so those roads keep their OSM or class-default speed and say so; the spec's '> 95% of Drury links' cannot be met
     from current data.
   - Stops snap to the nearest road within 60 m (not to a node), so stops along long rural links still count.
   - AGS23 runs 2022–2052. A year outside that uses the nearest AGS23 year, and the review row, the zone and the
     gateway derivation say so. */

const Feeds = (() => {
  const NZTA = 'https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services';
  const AT = 'https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services';
  const AC = 'https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services';
  const MDB = 'https://files.mobilitydatabase.org';
  const AC_LICENCE = 'Auckland Council open data, CC BY 4.0 (confirm before publishing)';

  /* ---------- sources (data, not code) ---------- */
  const SRC = {
    nslr: { label: 'Legal speed limits', name: 'NZTA National Speed Limit Register', url: `${NZTA}/SpeedLimitZoneFull__View/FeatureServer/0`,
      page: 'https://www.arcgis.com/home/item.html?id=aa376f1f2f3643bdac4d18855229239c', licence: 'CC BY 4.0', attribution: 'Speed limits: NZTA National Speed Limit Register (CC BY 4.0)' },
    controlled: { label: 'Signalised sites', name: 'AT controlled intersections (RoadingService layer 2)', url: `${AT}/RoadingService/FeatureServer/2`,
      page: 'https://at.govt.nz/about-us/at-data-sources', licence: DataHub.AT_LICENCE, note: "AT marks this layer 'not refreshed', so check recently built signals on site." },
    signs: { label: 'Regulatory signs', name: 'AT signs (RoadingService layer 0): RG 5 stop, RG 6 give way, RG 6R roundabout', url: `${AT}/RoadingService/FeatureServer/0`,
      page: 'https://at.govt.nz/about-us/at-data-sources', licence: DataHub.AT_LICENCE },
    osm: { label: 'Turn bans', name: 'OpenStreetMap turn restrictions (Overpass)', url: 'https://overpass-api.de/api/interpreter',
      page: 'https://www.openstreetmap.org/copyright', licence: 'ODbL 1.0, © OpenStreetMap contributors' },
    tms: { label: 'Daily counts', name: 'NZTA TMS daily traffic counts', url: `${NZTA}/TMS_Telemetry_Sites/FeatureServer/0`,
      page: 'https://catalogue.data.govt.nz/dataset/tms-daily-traffic-counts-api4', licence: 'CC BY 4.0' },
    profiles: { label: 'Peak profiles', name: 'TMS peak profiles built from the NZTA quarter-hourly archive', url: 'data/tms_profiles.json',
      page: 'https://www.arcgis.com/home/item.html?id=39cdaa422dbd45c4b41e0e59b49e79e2', licence: 'CC BY 4.0' },
    ags: { label: 'Growth (Auckland)', name: 'Auckland Council AGS23 v1.1 (Auckland Growth Scenario)', url: `${AC}/Auckland_Growth_Scenario_v1_1/FeatureServer/0`,
      page: 'https://www.knowledgeauckland.org.nz/publications/auckland-growth-scenario-2023-version-11-ags23v11-data/',
      licence: 'Auckland Council terms: fetched live for this study area; never bundled or republished', attribution: 'Auckland Council AGS23 v1.1' },
    gtfsCat: { label: 'Bus feed list', name: 'Mobility Database catalogue', url: `${MDB}/feeds_v2.csv`, page: 'https://mobilitydatabase.org/terms-and-conditions', licence: 'CC0 (catalogue metadata)' },
    gtfs: { label: 'Bus timetables', name: 'GTFS timetable through the Mobility Database mirror', url: `${MDB}/{id}/latest.zip`, page: 'https://mobilitydatabase.org/terms-and-conditions',
      licence: "The publisher's own licence (see the feed); Mobility Database terms" },
    flood: { label: 'Flood plains', name: 'Auckland Council flood plains', url: `${AC}/Flood_Plains/FeatureServer/0`,
      page: 'https://data-aucklandcouncil.opendata.arcgis.com/', licence: AC_LICENCE },
    coastal: { label: 'Coastal inundation', name: 'Auckland Council coastal inundation (1% AEP and sea-level-rise layers)', url: AC,
      page: 'https://data-aucklandcouncil.opendata.arcgis.com/', licence: AC_LICENCE },
    council: { label: 'Council counts', name: 'Council count layer (ArcGIS)', url: '', licence: "The council's own licence (check the layer)" },
    mvr: { label: 'Fleet mix', name: 'NZTA Motor Vehicle Register', url: NZTA, page: 'https://opendata-nzta.opendata.arcgis.com/', licence: 'CC BY 4.0' },
  };
  const COUNCILS = {
    hamilton: { label: 'Hamilton City Council', url: 'https://services1.arcgis.com/R6s0QqCMQdwKY6yp/arcgis/rest/services/Hamilton%20City%20Traffic%20Counts/FeatureServer/0' },
    qldc: { label: 'Queenstown Lakes District Council', url: 'https://services1.arcgis.com/9YyqaQtDdDR8tupG/arcgis/rest/services/Traffic_by_Site_Latest_Counts/FeatureServer/0', latest: true, maxAgeYears: 10 },
  };
  // Mobility Database ids for NZ feeds, with rough coverage boxes [s, w, n, e] used only when the catalogue cannot be read
  const NZ_FEEDS = [
    { id: 'mdb-1029', provider: 'Auckland Transport', box: [-37.45, 174.15, -36.05, 175.6] },
    { id: 'mdb-1132', provider: 'Metlink (Greater Wellington)', box: [-41.65, 174.55, -40.6, 176.1] },
    { id: 'mdb-1313', provider: 'Metro (Christchurch)', box: [-43.95, 171.9, -43.2, 173.1] },
    { id: 'mdb-983', provider: 'Otago Regional Council', box: [-46.3, 168.3, -44.6, 170.9] },
    { id: 'tld-6738', provider: 'Waikato Regional Council', box: [-38.6, 174.6, -37.25, 176.3] },
    { id: 'mdb-1071', provider: 'BayBus (Bay of Plenty)', box: [-38.4, 175.8, -37.4, 177.3] },
  ];

  /* ---------- thresholds and conventions (standards as data) ---------- */
  const RULES = {
    nslr: { samples: [0.25, 0.5, 0.75], futureDay: '07-01', source: 'docs/spec/nslr-speeds (modal value of three samples along the link)', verified: false },
    // Modelled peak periods for "does a variable (school) limit apply": Spare Construct assumption
    periods: { AM: [7, 9], PM: [16, 18], source: 'Spare Construct assumption: two-hour peak windows', verified: false },
    control: { siteM: 40, signM: 25, tailM: 30, pedCycle: 60, pedCycleRange: '60–90 s', source: 'docs/spec/auto-junction-control; distances are Spare Construct assumptions', verified: false },
    tms: { minShare: 0.5, medianDays: 28, aadtDays: 300, termWeeks: [8, 12], z: 1.96, days: 400, chunk: 8, maxPages: 120,
      hcvRule: 'NZTA length classes: under 5.5 m light, over 11 m heavy, 5.5–11 m split 50:50 between light and heavy',
      source: 'docs/spec/tms-daily; NZTA TMS', verified: false },
    ags: { gatewayKm: 10, gatewayDeg: 60, years: [2022, 2052], groups: { Residential: 'hh', Commercial: 'emp', Industrial: 'emp', Community: 'pop' },
      source: 'docs/spec/ags23-growth (Spare Construct convention). The ±60° sector beyond a gateway and population growth for community uses are Spare Construct assumptions', verified: false },
    gtfs: { bufferM: 500, linkM: 60, source: 'docs/spec/gtfs-bus; snapping a stop to a road within 60 m is a Spare Construct assumption', verified: false },
    counts: { adt: [/^adt$/i, /aadt/i, /average.*daily/i, /^vpd$/i], peakVol: [/peak.*(traffic|vol)/i, /peak_?hour_?vol/i], peakTime: [/peak_?hour$/i, /peak_?time/i],
      hcv: [/heavy|hcv|pc_?hv/i], date: [/count_?date/i, /^date$/i, /survey_?date/i], name: [/road_?name/i, /^name$/i, /street/i, /location/i, /site_?desc/i, /description/i],
      dir: [/^dir/i, /direction/i], latest: [/^latest$/i], year: /^year_?(\d{4})$/i, source: 'docs/spec/local-road-counts', verified: false },
    // The register's statistics query takes about 21 s (docs/spec/fleet-mix), so it gets at least 90 s
    fleetDays: 90, fleetTimeout: 90,
  };
  // NZ state school terms (Tue–Thu calibration days must fall in term). Term 1 starts at the latest permitted date.
  const TERMS = {
    source: 'Ministry of Education school term dates, https://www.education.govt.nz/school/school-terms-and-holidays/ (as entered by Spare Construct; check before relying on them)', verified: false,
    dates: {
      2022: [['2022-02-07', '2022-04-14'], ['2022-05-02', '2022-07-08'], ['2022-07-25', '2022-09-30'], ['2022-10-17', '2022-12-20']],
      2023: [['2023-02-07', '2023-04-06'], ['2023-04-24', '2023-06-30'], ['2023-07-17', '2023-09-22'], ['2023-10-09', '2023-12-20']],
      2024: [['2024-02-07', '2024-04-12'], ['2024-04-29', '2024-07-05'], ['2024-07-22', '2024-09-27'], ['2024-10-14', '2024-12-20']],
      2025: [['2025-02-07', '2025-04-11'], ['2025-04-28', '2025-07-04'], ['2025-07-21', '2025-09-26'], ['2025-10-13', '2025-12-19']],
      2026: [['2026-02-06', '2026-04-02'], ['2026-04-20', '2026-07-03'], ['2026-07-20', '2026-09-25'], ['2026-10-12', '2026-12-18']],
    },
  };
  // Matariki public holiday dates (Te Kāhui o Matariki Public Holiday Act 2022, Schedule 1); later years are not entered
  const MATARIKI = { source: 'Te Kāhui o Matariki Public Holiday Act 2022, Schedule 1', verified: false,
    dates: { 2022: '06-24', 2023: '07-14', 2024: '06-28', 2025: '06-20', 2026: '07-10', 2027: '06-25', 2028: '07-14', 2029: '07-06', 2030: '06-21' } };

  // Settings (Settings › Official data feeds); saved projects keep their values
  const DEFAULTS = { timeout: 30, nslrFuture: true, tmsDays: RULES.tms.days, otherScenarios: false };
  const cfg = () => ({ ...DEFAULTS, ...((Store.P && Store.S && Store.S.feeds) || {}) });

  /* ---------- project data, memory and provenance ---------- */
  // Store.P.feeds = { vintage: { id: { source, url, at, licence, n, note, error } }, nslr, tms, ags, gtfs, hazards, fleet }
  function F() { const P = Store.P; P.feeds ||= {}; P.feeds.vintage ||= {}; return P.feeds; }
  // Raw layers stay in memory only (AGS23 must not be saved or republished; flood polygons are large)
  const mem = { meta: new Map(), nslr: null, control: null, restrictions: null, ags: null, hazards: null, catalogue: null };
  function stamp(id, extra = {}) {
    const s = SRC[id] || {};
    const v = { source: s.name, url: extra.url || s.url, page: s.page, licence: extra.licence || s.licence, at: new Date().toISOString(), ...extra };
    F().vintage[extra.key || id] = v;
    return v;
  }
  const failStamp = (id, e, extra = {}) => stamp(id, { ...extra, error: String(e && e.message || e), n: 0 });
  const NZF = (() => { try { return new Intl.DateTimeFormat('en-CA', { timeZone: 'Pacific/Auckland', year: 'numeric', month: '2-digit', day: '2-digit' }); } catch { return null; } })();
  const today = (now = new Date()) => { try { return NZF.format(now); } catch { return now.toISOString().slice(0, 10); } };
  const nzYear = now => +today(now).slice(0, 4);
  // '9 Oct 2026'; a missing or unreadable date gives '' (or the text as stored), never 'Invalid Date'
  const dateText = iso => { if (iso == null || iso === '') return ''; const t = new Date(iso); if (isNaN(t)) return String(iso).slice(0, 10); try { return t.toLocaleDateString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric' }); } catch { return t.toISOString().slice(0, 10); } };
  // A relative address (data/tms_profiles.json) is on this site
  const hostOf = url => { if (!url) return ''; try { return new URL(url).host || url; } catch { return 'this site'; } };
  const box = b => b && typeof b.getSouth === 'function' ? { s: b.getSouth(), w: b.getWest(), n: b.getNorth(), e: b.getEast() } : b;
  const padBox = (b, m) => { const B = box(b), dy = m / 110574, dx = m / (111320 * Math.cos((B.s + B.n) / 2 * Math.PI / 180)); return { s: B.s - dy, w: B.w - dx, n: B.n + dy, e: B.e + dx }; };
  const esc = s => U.esc(s);
  // Links only to web addresses (or a plain path on this site): a project file could carry 'javascript:' in a URL field
  const safeHref = h => { const s = String(h || '').trim(); return /^https?:\/\/[^\s"'<>]+$/i.test(s) || /^[\w][\w./-]*$/.test(s) ? s : ''; };
  const a = (href, t) => safeHref(href) ? `<a href="${esc(safeHref(href))}" target="_blank" rel="noopener">${esc(t)}</a>` : esc(t);
  // Time limit in ms from Settings › Official data feeds; a cleared or zero box (which aborted every request at once) uses the default
  const limitMs = () => (+cfg().timeout > 0 ? +cfg().timeout : DEFAULTS.timeout) * 1000;

  /* ---------- network calls ---------- */
  // GET (or POST with body) with a time limit; as 'json' | 'text' | 'buffer'. Errors say what to do.
  async function get(url, { as = 'json', timeout, method, body, headers } = {}) {
    const ms = timeout || limitMs(), host = hostOf(url);
    const ac = typeof AbortController !== 'undefined' ? new AbortController() : null, tm = ac ? setTimeout(() => ac.abort(), ms) : null;
    try {
      let r;
      try { r = await fetch(url, { ...(ac ? { signal: ac.signal } : {}), ...(method ? { method } : {}), ...(body != null ? { body } : {}), ...(headers ? { headers } : {}) }); }
      catch (e) { throw new Error(e && e.name === 'AbortError' ? `${host} did not answer within ${Math.round(ms / 1000)} s. Try again later, or raise the time limit in Settings › Official data feeds` : `${host} could not be reached (${(e && e.message) || e}). Check your connection and try again`); }
      if (!r.ok) throw new Error(`${host} answered HTTP ${r.status}${r.status === 404 ? ' (not found: the service may have moved)' : ''}`);
      const lm = r.headers && typeof r.headers.get === 'function' ? r.headers.get('last-modified') : null;
      if (method === 'HEAD') return { lastModified: lm };
      try {
        const body = as === 'buffer' ? await r.arrayBuffer() : as === 'text' ? await r.text() : await r.json();
        return as === 'buffer' ? { buf: body, lastModified: lm } : body;
      } catch (e) { throw new Error(e && e.name === 'AbortError' ? `${host} stopped sending data after ${Math.round(ms / 1000)} s. Try again later` : `${host} sent something Spare Construct could not read (${(e && e.message) || e})`); }
    } finally { if (tm) clearTimeout(tm); }
  }
  // Layer description (?f=json), cached for the session: field names are resolved from it at run time
  async function layerMeta(url) {
    if (mem.meta.has(url)) return mem.meta.get(url);
    const j = await get(url + '?f=json');
    if (j.error) throw new Error(`${hostOf(url)}: ${j.error.message || 'layer description failed'}`);
    mem.meta.set(url, j);
    return j;
  }
  const fieldNames = meta => ((meta && meta.fields) || []).map(f => f.name);
  // The layer's own spelling of each wanted field (case-insensitive); missing fields are left out
  const resolve = (meta, want) => { const have = fieldNames(meta); if (!have.length) return want.slice(); return want.map(w => have.find(h => h.toLowerCase() === w.toLowerCase())).filter(Boolean); };
  const env = b => { const B = box(b); return { geometry: [B.w, B.s, B.e, B.n].map(v => (+v).toFixed(6)).join(','), geometryType: 'esriGeometryEnvelope', inSR: '4326', spatialRel: 'esriSpatialRelIntersects', outSR: '4326' }; };
  // Paged ArcGIS query (resultOffset up to the layer's maxRecordCount); complete: false when the page cap was reached
  async function arcQuery(url, params, { maxPages = 40, status, timeout } = {}) {
    const out = [];
    for (let page = 0, offset = 0; page < maxPages; page++) {
      const q = new URLSearchParams({ f: 'json', ...params, resultOffset: String(offset) });
      const j = await get(url + '/query?' + q, { timeout });
      if (j.error) throw new Error(`${hostOf(url)}: ${j.error.message || 'query failed'}${(j.error.details || []).length ? ' (' + j.error.details.join('; ') + ')' : ''}`);
      const f = j.features || [];
      out.push(...f);
      if (status) status(out.length);
      if (!j.exceededTransferLimit || !f.length) return { features: out, complete: true };
      offset += f.length;
    }
    return { features: out, complete: false };
  }

  /* ---------- geometry helpers ---------- */
  const ringsBox = rings => { let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity; for (const r of rings) for (const p of r) { if (p[0] < x0) x0 = p[0]; if (p[0] > x1) x1 = p[0]; if (p[1] < y0) y0 = p[1]; if (p[1] > y1) y1 = p[1]; } return [x0, y0, x1, y1]; };
  // Even-odd point in polygon (holes included); x = lng, y = lat
  function inRings(x, y, rings) {
    let inside = false;
    for (const r of rings) for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
      const xi = r[i][0], yi = r[i][1], xj = r[j][0], yj = r[j][1];
      if ((yi > y) !== (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) inside = !inside;
    }
    return inside;
  }
  // Grid of polygon bounding boxes for fast point lookups (lng/lat degrees)
  function polyIndex(polys, cell = 0.005) {
    const g = new Map(), k = (i, j) => i * 1e6 + j;
    polys.forEach((p, n) => { const [x0, y0, x1, y1] = p.bb; for (let i = Math.floor(x0 / cell); i <= Math.floor(x1 / cell); i++) for (let j = Math.floor(y0 / cell); j <= Math.floor(y1 / cell); j++) { const a = g.get(k(i, j)); if (a) a.push(n); else g.set(k(i, j), [n]); } });
    return (x, y) => (g.get(k(Math.floor(x / cell), Math.floor(y / cell))) || []).map(n => polys[n]).filter(p => x >= p.bb[0] && x <= p.bb[2] && y >= p.bb[1] && y <= p.bb[3] && inRings(x, y, p.rings));
  }
  function centroid(rings) {
    const r = rings.reduce((m, q) => q.length > m.length ? q : m, rings[0] || []);
    let A = 0, cx = 0, cy = 0;
    for (let i = 0, j = r.length - 1; i < r.length; j = i++) { const f = r[j][0] * r[i][1] - r[i][0] * r[j][1]; A += f; cx += (r[j][0] + r[i][0]) * f; cy += (r[j][1] + r[i][1]) * f; }
    if (!A) { const bb = ringsBox(rings); return [(bb[1] + bb[3]) / 2, (bb[0] + bb[2]) / 2]; }
    return [cy / (3 * A), cx / (3 * A)];   // [lat, lng]
  }
  const liveLinks = sc => Object.values(sc.links).filter(L => L.status !== 'closed' && sc.nodes[L.a] && sc.nodes[L.b]);
  const netBox = sc => { const n = Object.values(sc.nodes); if (!n.length) return null; return { s: Math.min(...n.map(x => x.lat)), n: Math.max(...n.map(x => x.lat)), w: Math.min(...n.map(x => x.lng)), e: Math.max(...n.map(x => x.lng)) }; };
  const inBox = (B, ll) => ll[0] >= B.s && ll[0] <= B.n && ll[1] >= B.w && ll[1] <= B.e;

  /* =====================================================================================
     1. NZTA National Speed Limit Register
     ===================================================================================== */
  const NSLR_FIELDS = ['OBJECTID', 'speedLimitZoneValue', 'speedCategoryName', 'speedLimitZoneVariableSpeed', 'speedLimitZoneVarPrdDesc', 'speedLimitZoneDesc',
    'speedLimitZoneStartDate', 'speedLimitZoneEndDate', 'speedLimitZoneLanePurposeName', 'rcaZoneReferenceName', 'legalReference', 'legalInstrumentURL'];
  // Current versions only. A future scenario year uses 1 July of that year, so gazetted future changes apply.
  function nslrWhere(year, now = new Date()) {
    const t = year && year > nzYear(now) ? `TIMESTAMP '${year}-${RULES.nslr.futureDay} 00:00:00'` : 'CURRENT_TIMESTAMP';
    return `(speedLimitZoneEndDate IS NULL OR speedLimitZoneEndDate > ${t}) AND speedLimitZoneStartDate <= ${t}`;
  }
  const asOfMs = (year, now = new Date()) => year && year > nzYear(now) ? Date.UTC(year, 6, 1) : now.getTime();
  // Peak windows a variable limit's period text covers: '08:30 – 9:15am and 2:45 – 3:30pm' → { AM: true, PM: false }
  function varPeriods(desc) {
    // Minutes are optional when am/pm is given ('8am – 9.15am'); bare numbers ('40 - 50') are not times
    const out = [], re = /(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm)?\s*(?:–|-|to)\s*(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm)?/gi;
    let m;
    while ((m = re.exec(String(desc || '')))) {
      if (!((m[2] || m[3] || m[6]) && (m[5] || m[6])) || +m[1] > 24 || +m[4] > 24) { re.lastIndex = m.index + 1; continue; }
      const h = (hh, ap) => (+hh % 12) + (ap && ap.toLowerCase() === 'pm' ? 12 : 0) + (!ap && +hh === 12 ? 12 : 0);
      const eAp = m[6] || null; let sAp = m[3] || eAp;
      const m1 = (+m[2] || 0) / 60, m2 = (+m[5] || 0) / 60;
      let s = h(m[1], sAp) + m1; const e = h(m[4], eAp) + m2;
      if (s > e && !m[3]) s = h(m[1], 'am') + m1;
      out.push([s, e]);
    }
    const ov = w => out.some(([s, e]) => s < w[1] && e > w[0]);
    return out.length ? { AM: ov(RULES.periods.AM), PM: ov(RULES.periods.PM), windows: out } : null;
  }
  async function loadNSLR(b, year, now = new Date()) {
    let meta = null; try { meta = await layerMeta(SRC.nslr.url); } catch { }
    const fields = resolve(meta, NSLR_FIELDS), where = nslrWhere(year, now);
    const r = await arcQuery(SRC.nslr.url, { where, outFields: fields.join(','), ...env(b), geometryPrecision: '6', maxAllowableOffset: '0.00002', orderByFields: 'OBJECTID' }, { maxPages: 30 });
    const at = asOfMs(year, now), drop = { ended: 0, lane: 0, value: 0 };
    const polys = [];
    for (const x of r.features) {
      const A = x.attributes || {}, g = x.geometry;
      if (!g || !g.rings || !g.rings.length) continue;
      const start = A.speedLimitZoneStartDate, end = A.speedLimitZoneEndDate;
      // Never trust the server filter alone: a version that has ended (or not started) by the as-at date is dropped
      if ((end != null && end <= at) || (start != null && start > at)) { drop.ended++; continue; }
      const lane = A.speedLimitZoneLanePurposeName;
      if (lane && !/general|all/i.test(lane)) { drop.lane++; continue; }
      const v = parseInt(A.speedLimitZoneValue, 10);
      if (!(v > 0)) { drop.value++; continue; }
      const variable = /variable/i.test(A.speedCategoryName || ''), vv = parseInt(A.speedLimitZoneVariableSpeed, 10);
      polys.push({ rings: g.rings, bb: ringsBox(g.rings), v, cat: A.speedCategoryName || '', variable,
        varV: variable && vv > 0 ? vv : null, varDesc: A.speedLimitZoneVarPrdDesc || '', rca: A.rcaZoneReferenceName || '', ref: A.legalReference || '', inst: A.legalInstrumentURL || '', start });
    }
    return { polys, where, year: year && year > nzYear(now) ? year : null, asOf: new Date(at).toISOString().slice(0, 10), drop, complete: r.complete, n: r.features.length };
  }
  // Attach the legal limit to every link of a scenario: modal value of the polygons under 25/50/75% of the link
  function assignNSLR(sc, set) {
    const find = polyIndex(set.polys), st = { links: 0, nslr: 0, variable: 0, missing: 0 }, date = set.asOf;
    for (const L of liveLinks(sc)) {
      st.links++;
      const g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1], picks = [], vars = [];
      // Overlapping versions: the one that started last is in force (then the lower value, to be safe)
      const latest = hs => hs.length ? hs.reduce((m, h) => (h.start || 0) > (m.start || 0) || ((h.start || 0) === (m.start || 0) && h.v < m.v) ? h : m) : null;
      for (const fr of RULES.nslr.samples) {
        const p = U.along(g, cum, len * fr).p, hit = find(p[1], p[0]);
        const perm = hit.filter(h => !h.variable);
        picks.push(latest(perm.length ? perm : hit));
        // A variable (school) zone counts even where a permanent version overlaps it
        const vz = latest(hit.filter(h => h.variable && h.varV)); if (vz) vars.push(vz);
      }
      const votes = new Map();
      for (const h of picks) if (h) votes.set(h.v, (votes.get(h.v) || 0) + 1);
      if (!votes.size) {
        // Nothing from the register here now: drop what an earlier load put on the link, school limits included
        if (L.speedSource === 'NSLR') { delete L.speedLimit; delete L.nslr; delete L.varSpeed; }
        L.speedSource = 'OSM or class default'; st.missing++; continue;
      }
      const top = Math.max(...votes.values()), cands = [...votes].filter(([, n]) => n === top).map(([v]) => v);
      const v = cands.length === 1 ? cands[0] : picks[1] && cands.includes(picks[1].v) ? picks[1].v : Math.min(...cands);
      const src = picks.find(h => h && h.v === v), vz = vars[0] || null;
      L.speedLimit = v; L.speedSource = 'NSLR';
      L.nslr = { v, cat: src.cat, rca: src.rca, ref: src.ref, inst: src.inst, at: date, year: set.year,
        ...(vz ? { var: { v: vz.varV, desc: vz.varDesc, ...(varPeriods(vz.varDesc) || {}) } } : {}) };
      if (L.nslr.var) { delete L.nslr.var.windows; L.varSpeed = { v: vz.varV, desc: vz.varDesc, AM: !!L.nslr.var.AM, PM: !!L.nslr.var.PM }; st.variable++; } else delete L.varSpeed;
      st.nslr++;
    }
    st.share = st.links ? st.nslr / st.links : 0;
    return st;
  }
  // A modelled speed the user changed after Spare Construct applied the legal limit is the user's (never overwritten)
  const userSpeed = L => L.speedApplied != null && L.speed !== L.speedApplied;
  function speedProposals(sc) {
    return liveLinks(sc).filter(L => L.status === 'existing' && L.speedLimit > 0 && L.speed !== L.speedLimit)
      .map(L => ({ id: L.id, name: L.name || 'Unnamed road', cls: L.cls, from: L.speed, to: L.speedLimit, user: userSpeed(L), variable: !!L.varSpeed }));
  }
  // Copy legal limits into modelled (free-flow) speeds for the chosen links; returns the number changed
  function applySpeeds(sc, ids) {
    let n = 0;
    for (const id of ids) {
      const L = sc.links[id]; if (!L || !(L.speedLimit > 0)) continue;
      if (L.speedOSM == null) L.speedOSM = L.speed;
      L.speed = L.speedLimit; L.speedApplied = L.speedLimit; n++;
    }
    return n;
  }
  async function nslrAll(b, scs, status = () => {}) {
    const years = new Set(), now = new Date(), Y = nzYear(now);
    for (const sc of scs) { const y = Engine.yearOf(sc); years.add(cfg().nslrFuture && y > Y ? y : 0); }
    const sets = new Map(), out = { scenarios: [], drop: { ended: 0, lane: 0, value: 0 }, complete: true };
    for (const y of years) {
      status(y ? `Downloading speed limits in force on 1 July ${y}…` : 'Downloading current speed limits (NZTA register)…');
      const s = await loadNSLR(b, y || null, now);
      sets.set(y, s);
      for (const k in s.drop) out.drop[k] += s.drop[k];
      out.complete = out.complete && s.complete;
    }
    for (const sc of scs) { const y = Engine.yearOf(sc), set = sets.get(cfg().nslrFuture && y > Y ? y : 0); out.scenarios.push({ id: sc.id, name: sc.name, year: set.year, ...assignNSLR(sc, set) }); }
    mem.nslr = { sets: [...sets.values()], at: Date.now() };
    const cur = out.scenarios.find(s => s.id === Store.sc.id) || out.scenarios[0];
    F().nslr = { at: new Date().toISOString(), scenarios: out.scenarios.map(s => ({ id: s.id, year: s.year, links: s.links, nslr: s.nslr, variable: s.variable })), drop: out.drop };
    stamp('nslr', { n: [...sets.values()].reduce((s, x) => s + x.polys.length, 0), note: `Current versions only${[...sets.keys()].some(Boolean) ? '; future scenario years use the limits gazetted for 1 July of that year' : ''}. ${cur ? `${U.fmt.pct(cur.share)} of roads in ${cur.name} matched.` : ''}${out.complete ? '' : ' Not every page downloaded: zoom to a smaller area.'}` });
    return out;
  }

  /* =====================================================================================
     2. AT controlled intersections and signs → junction controls
     ===================================================================================== */
  async function loadControl(b) {
    let mc = null, ms = null;
    try { mc = await layerMeta(SRC.controlled.url); } catch { }
    try { ms = await layerMeta(SRC.signs.url); } catch { }
    const cf = resolve(mc, ['OBJECTID', 'intersection_id', 'intersection_no', 'intersection_desc', 'intersection_type', 'controlled', 'ped_crossing_flg']);
    const ctl = await arcQuery(SRC.controlled.url, { where: "controlled='Yes'", outFields: cf.join(','), ...env(b), orderByFields: 'OBJECTID' });
    const sf = resolve(ms, ['OBJECTID', 'sign_id', 'sign_type', 'side', 'road_name', 'legend_note']);
    const sg = await arcQuery(SRC.signs.url, { where: "sign_type IN ('RG 5','RG 6','RG 6R')", outFields: sf.join(','), ...env(b), orderByFields: 'OBJECTID' });
    const sites = ctl.features.filter(x => x.geometry).map(x => {
      const A = x.attributes || {};
      return { id: A.intersection_id ?? A.OBJECTID, no: A.intersection_no ?? null, name: String(A.intersection_desc || '').replace(/^\(\d+\)\s*/, ''), type: A.intersection_type || '',
        controlled: A.controlled === 'Yes', ped: A.ped_crossing_flg === 'Yes' || /pedestrian/i.test(A.intersection_type || ''), lat: U.r7(x.geometry.y), lng: U.r7(x.geometry.x) };
    });
    const signs = sg.features.filter(x => x.geometry && /^RG ?(5|6|6R)$/i.test((x.attributes || {}).sign_type || '')).map(x => {
      const A = x.attributes; return { id: A.sign_id ?? A.OBJECTID, type: A.sign_type.replace(/\s+/, ' ').toUpperCase(), side: A.side || '', road: A.road_name || '', lat: U.r7(x.geometry.y), lng: U.r7(x.geometry.x) };
    });
    // Shared with junction sites (Sites.points reads P.data.controlled)
    const d = DataHub.D();
    d.controlled = sites;
    d.loaded.controlled = { at: Date.now(), n: sites.length, source: SRC.controlled.name, url: SRC.controlled.url, licence: SRC.controlled.licence };
    mem.control = { sites, signs, at: Date.now() };
    F().control = { at: new Date().toISOString(), sites: sites.length, signs: signs.length };
    stamp('controlled', { n: sites.length, note: SRC.controlled.note });
    stamp('signs', { n: signs.length, note: 'Signs within 25 m of a junction set its control: RG 6R roundabout, RG 5 stop, RG 6 give way.' });
    return mem.control;
  }
  const CTL_RANK = { none: 0, priority: 1, stop: 2, roundabout: 3, signals: 4 };
  const SIGN_CTL = { 'RG 5': 'stop', 'RG 6': 'priority', 'RG 6R': 'roundabout' };
  // A control the user changed after Spare Construct set it is the user's
  const userControl = n => !!n.controlSrc && n.controlAuto != null && n.control !== n.controlAuto;
  /* Proposed controls for one scenario: AT signal sites within 40 m of a junction (the nearest junction node and the
     rest of its junction site), signs within 25 m on the approach whose last 30 m is nearest with the sign on the
     driver's left. Returns rows [{ id, node, name, from, to, src, why, atSiteNo, d, signs, user, ped, split }]. */
  function controlProposals(sc, data = mem.control) {
    if (!data) return { rows: [], unmatched: [] };
    const R = RULES.control, at = new Map(), ring = new Set();
    for (const L of liveLinks(sc)) { for (const n of [L.a, L.b]) { const x = at.get(n); if (x) x.push(L); else at.set(n, [L]); } if (L.ring) { ring.add(L.a); ring.add(L.b); } }
    const nodes = Object.values(sc.nodes).filter(n => at.has(n.id));
    const junction = n => (at.get(n.id) || []).length >= 3 || ring.has(n.id);
    const ll = n => [n.lat, n.lng];
    const near = (p, filt, maxM) => { let best = null, bd = maxM; for (const n of nodes) { if (!filt(n)) continue; const d = U.dist(p, ll(n)); if (d <= bd) { bd = d; best = n; } } return best ? { n: best, d: bd } : null; };
    const plan = new Map(), unmatched = [];
    const add = (n, to, src, why, extra = {}) => {
      const p = plan.get(n.id);
      if (p && CTL_RANK[p.to] >= CTL_RANK[to]) { if (extra.signs) Object.assign(p.signs, extra.signs); return; }
      plan.set(n.id, { node: n.id, to, src, why, signs: { ...((p && p.signs) || {}), ...(extra.signs || {}) }, ...extra, ...(p && p.atSiteNo && !extra.atSiteNo ? { atSiteNo: p.atSiteNo } : {}) });
    };
    for (const s of data.sites) {
      const p = [s.lat, s.lng];
      if (s.ped) {
        const m = near(p, n => (at.get(n.id) || []).length === 2 && !ring.has(n.id), R.siteM);
        if (m) add(m.n, 'signals', SRC.controlled.name, `Signalised pedestrian crossing, AT site ${s.no}`, { atSiteNo: s.no, d: m.d, ped: true });
        else {
          const lk = Net.nearestLink(sc, p, R.siteM, L => L.status !== 'closed');
          if (lk) unmatched.push({ site: s, link: lk.id, d: lk.d, split: true }); else unmatched.push({ site: s });
        }
        continue;
      }
      const m = near(p, junction, R.siteM);
      if (!m) { unmatched.push({ site: s }); continue; }
      const site = typeof Sites !== 'undefined' ? Sites.of(sc, m.n.id) : null;
      const group = site ? site.nodes.map(id => sc.nodes[id]).filter(Boolean) : [m.n];
      for (const n of group) add(n, 'signals', SRC.controlled.name, `AT signal site ${s.no}${s.name ? ': ' + s.name : ''}`, { atSiteNo: s.no, d: U.dist(p, ll(n)), site: site ? site.id : null });
    }
    // Signs: which approach a sign belongs to. The approach's last 30 m runs into the node; the sign must be on its left.
    for (const sg of data.signs) {
      const p = [sg.lat, sg.lng], m = near(p, junction, R.signM);
      if (!m) continue;
      const F = U.flat(p[0]), px = F.x(p), py = F.y(p);
      let best = null;
      for (const L of at.get(m.n.id)) {
        if (L.ring) continue;
        let g = Net.geom(sc, L); if (L.a === m.n.id) g = g.slice().reverse();   // travel towards the node
        const cum = U.cumLen(g), len = cum[cum.length - 1], from = Math.max(0, len - R.tailM);
        const s0 = U.along(g, cum, from).p, pts = [s0, ...g.filter((_, i) => cum[i] > from)];
        let bd = Infinity, left = false;
        for (let i = 0; i < pts.length - 1; i++) {
          const ax = F.x(pts[i]), ay = F.y(pts[i]), bx = F.x(pts[i + 1]), by = F.y(pts[i + 1]), r = U.segDist(px, py, ax, ay, bx, by);
          if (r.d < bd) { bd = r.d; left = (bx - ax) * (py - ay) - (by - ay) * (px - ax) > 0; }
        }
        const score = bd + (left ? 0 : 1000);
        if (!best || score < best.score) best = { L, score, d: bd, left };
      }
      if (!best) continue;
      const to = SIGN_CTL[sg.type];
      if (to === 'roundabout' && (at.get(m.n.id) || []).length < 3 && !ring.has(m.n.id)) continue;
      /* A sign never downgrades signals or a roundabout: slip lanes at signals and roundabout entries carry RG 6 signs
         (on Drury, five roundabout nodes were proposed as give way). Nor does a stop or give-way sign control a node on
         a roundabout ring. Signal sites from AT still upgrade (they are matched above, not here). */
      const cur = sc.nodes[m.n.id].control;
      if ((ring.has(m.n.id) && to !== 'roundabout') || cur === 'signals' || (cur === 'roundabout' && to !== 'roundabout')) {
        const p = plan.get(m.n.id); if (p) Object.assign(p.signs, { [best.L.id]: sg.type });
        continue;
      }
      add(m.n, to, SRC.signs.name.split(' (')[0], `${sg.type} sign${sg.road ? ' on ' + DataHub.norm(sg.road).replace(/\b[a-z]/g, c => c.toUpperCase()) : ''}${best.left ? '' : ' (sign not on the driver’s left: check)'}`, { signs: { [best.L.id]: sg.type }, d: m.d });
    }
    const rows = [];
    for (const p of plan.values()) {
      const n = sc.nodes[p.node], user = userControl(n);
      const change = n.control !== p.to, meta = !change && (p.atSiteNo != null && n.atSiteNo !== p.atSiteNo || !n.controlSrc);
      if (!change && !meta) continue;
      rows.push({ id: p.node, ...p, name: Kit.junctionName(p.node, sc), from: n.control || 'none', change, user });
    }
    for (const u of unmatched.filter(u => u.split)) rows.push({ id: 'split:' + u.site.id, node: null, split: { link: u.link, ll: [u.site.lat, u.site.lng] }, to: 'signals', ped: true, change: true,
      src: SRC.controlled.name, why: `Signalised pedestrian crossing, AT site ${u.site.no}: adds a node on ${(sc.links[u.link] || {}).name || 'the road'}`, atSiteNo: u.site.no, from: 'none', name: u.site.name || 'Pedestrian crossing', d: u.d, signs: {} });
    rows.sort((x, y) => (y.change - x.change) || String(x.name).localeCompare(String(y.name)));
    return { rows, unmatched: unmatched.filter(u => !u.split) };
  }
  // Apply chosen rows to a scenario (call Store.snapshot() first for undo); returns the number of junctions changed
  function applyControls(sc, rows, ids, when = today()) {
    const pick = new Set(ids); let n = 0;
    for (const r of rows) {
      if (!pick.has(r.id)) continue;
      let id = r.node;
      if (r.split) { id = Net.splitLink(sc, r.split.link, r.split.ll); if (!id) continue; }
      const N = sc.nodes[id]; if (!N) continue;
      if (N.control !== r.to) n++;
      N.control = r.to; N.controlAuto = r.to; N.controlSrc = r.src; N.controlAt = when; N.upgrade = false;
      if (r.atSiteNo != null) N.atSiteNo = r.atSiteNo;
      if (r.ped) { N.ped = true; N.cycle = N.cycle || RULES.control.pedCycle; }
      if (r.signs && Object.keys(r.signs).length) N.signs = { ...(N.signs || {}), ...r.signs };
      const site = typeof Sites !== 'undefined' ? Sites.of(sc, id) : null;
      if (site && r.atSiteNo != null && site.atSiteNo == null) site.atSiteNo = r.atSiteNo;
    }
    return n;
  }

  /* =====================================================================================
     2b. OpenStreetMap turn restrictions → banned movements (docs/spec/auto-junction-control)
     relation[type=restriction] with from (way), via (node) and to (way). Links keep the OSM way id (L.osm = '<way>:<i>')
     and nodes the OSM node id (N.osm), so a restriction maps onto node.bans ('<inLinkId>><outLinkId>', engine C1).
     no_* bans the one movement; only_* bans every other movement from that road. Restrictions for one vehicle type
     (restriction:hgv …), with a time condition, that exempt cars, or with a via way are left out and counted.
     ===================================================================================== */
  async function loadRestrictions(b) {
    const B = box(b), bb = [B.s, B.w, B.n, B.e].map(v => (+v).toFixed(6)).join(',');
    const q = `[out:json][timeout:60];relation["type"="restriction"](${bb});out body;`;
    let last = null;
    for (const url of (typeof OVERPASS !== 'undefined' ? OVERPASS : [])) {
      try {
        const j = await get(url, { method: 'POST', body: 'data=' + encodeURIComponent(q), headers: { 'Content-Type': 'application/x-www-form-urlencoded' } });
        if (!j || !Array.isArray(j.elements)) throw new Error(`${hostOf(url)} sent no OpenStreetMap data`);
        const rels = j.elements.filter(e => e.type === 'relation' && Array.isArray(e.members));
        mem.restrictions = { rels, at: Date.now(), url };
        stamp('osm', { url, n: rels.length, note: 'Turn restrictions (relation type=restriction) through a junction node become banned movements after review.' });
        return mem.restrictions;
      } catch (e) { last = e; }
    }
    throw last || new Error('No OpenStreetMap server answered. Try again later.');
  }
  const RESTRICT_TEXT = { no_left_turn: 'no left turn', no_right_turn: 'no right turn', no_straight_on: 'no straight on', no_u_turn: 'no U-turn',
    only_left_turn: 'left turn only', only_right_turn: 'right turn only', only_straight_on: 'straight on only', only_u_turn: 'U-turn only' };
  /* Proposed bans for a scenario: rows [{ id, node, name, kind, bans: ['in>out'], why, src, user }] and skipped
     [{ rel, why }]. A ban the user removed after Spare Construct added it is the user's (row ticked off). */
  function banProposals(sc, data = mem.restrictions) {
    if (!data) return { rows: [], skipped: [] };
    const byOsm = new Map(), at = new Map();
    for (const n of Object.values(sc.nodes)) if (n.osm != null) byOsm.set(String(n.osm), n);
    for (const L of liveLinks(sc)) for (const n of [L.a, L.b]) { const x = at.get(n); if (x) x.push(L); else at.set(n, [L]); }
    const wayOf = L => String(L.osm || '').split(':')[0];
    const rows = [], skipped = [], name = L => L.name || 'the unnamed road';
    for (const r of data.rels) {
      const t = r.tags || {}, kind = t.restriction, skip = why => skipped.push({ rel: r.id, why });
      if (!RESTRICT_TEXT[kind]) { skip(kind ? `${kind} is not a turn restriction Spare Construct reads` : 'only for some vehicles or times'); continue; }
      if (t['restriction:conditional'] || /\bmotorcar\b|\bmotor_vehicle\b/.test(t.except || '')) { skip('conditional, or cars are exempt'); continue; }
      const m = role => r.members.filter(x => x.role === role);
      const from = m('from'), via = m('via'), to = m('to');
      if (from.length !== 1 || to.length !== 1 || via.length !== 1 || from[0].type !== 'way' || to[0].type !== 'way') { skip('not one from way, one via and one to way'); continue; }
      if (via[0].type !== 'node') { skip('via a way (not read yet)'); continue; }
      const N = byOsm.get(String(via[0].ref)); if (!N) continue;   // outside the modelled network
      const links = at.get(N.id) || [], fl = links.filter(L => wayOf(L) === String(from[0].ref)), tl = links.filter(L => wayOf(L) === String(to[0].ref));
      // The from and to ways must end at the via node (OSM rule); a way running through it is ambiguous
      if (fl.length !== 1 || tl.length !== 1) { skip(fl.length && tl.length ? 'the from or to way runs through the junction (split it in OSM)' : 'its roads are not in the model'); continue; }
      const F0 = fl[0], T0 = tl[0];
      const bans = kind.startsWith('only_') ? links.filter(L => L !== T0).map(L => `${F0.id}>${L.id}`) : [`${F0.id}>${T0.id}`];
      const have = new Set(N.bans || []), auto = new Set(N.bansAuto || []);
      const add = bans.filter(x => !have.has(x));
      if (!add.length) continue;
      rows.push({ id: `ban:${r.id}:${N.id}`, node: N.id, name: Kit.junctionName(N.id, sc), kind, bans: add, rel: r.id, src: `OpenStreetMap relation ${r.id}`,
        why: `${RESTRICT_TEXT[kind]} from ${name(F0)}${kind.startsWith('only_') ? ` (only into ${name(T0)})` : ` into ${name(T0)}`}`, user: add.some(x => auto.has(x)) });
    }
    rows.sort((x, y) => String(x.name).localeCompare(String(y.name)));
    return { rows, skipped };
  }
  // Apply chosen ban rows (call snapshot first); returns the number of movements banned
  function applyBans(sc, rows, ids, when = today()) {
    const pick = new Set(ids); let n = 0;
    for (const r of rows) {
      if (!pick.has(r.id)) continue;
      const N = sc.nodes[r.node]; if (!N) continue;
      const have = new Set(N.bans || []), auto = new Set(N.bansAuto || []), src = { ...(N.banSrc || {}) };
      for (const x of r.bans) { if (!have.has(x)) n++; have.add(x); auto.add(x); src[x] = `${r.src} (${r.kind}), ${when}`; }
      N.bans = [...have]; N.bansAuto = [...auto]; N.banSrc = src;
    }
    return n;
  }

  /* =====================================================================================
     3. NZTA TMS daily counts
     ===================================================================================== */
  const D2 = 86400000;
  const dnum = iso => Math.round(Date.parse(iso + 'T00:00:00Z') / D2);
  const diso = n => new Date(n * D2).toISOString().slice(0, 10);
  const dow = n => (n + 4) % 7;   // 0 = Sunday
  // NZ date of a TMS start date: midnight UTC and NZ midnight (11:00 or 12:00 UTC the day before) both give the NZ day
  const nzDay = new Map(), nzDateOf = ms => { let d = nzDay.get(ms); if (!d) nzDay.set(ms, d = today(new Date(ms))); return d; };
  function easter(y) {
    const a = y % 19, b = Math.floor(y / 100), c = y % 100, d = Math.floor(b / 4), e = b % 4, f = Math.floor((b + 8) / 25), g = Math.floor((b - f + 1) / 3),
      h = (19 * a + b - d - g + 15) % 30, i = Math.floor(c / 4), k = c % 4, l = (32 + 2 * e + 2 * i - h - k) % 7, m = Math.floor((a + 11 * h + 22 * l) / 451);
    const mo = Math.floor((h + l - 7 * m + 114) / 31), da = ((h + l - 7 * m + 114) % 31) + 1;
    return Math.round(Date.UTC(y, mo - 1, da) / D2);
  }
  const nthMonday = (y, m, nth) => { let d = Math.round(Date.UTC(y, m, 1) / D2); while (dow(d) !== 1) d++; return d + 7 * (nth - 1); };
  /* NZ public holidays for a year (day numbers), Mondayised (Holidays Act 2003). Auckland Anniversary (Monday nearest
     29 January) is added for the Auckland region. Matariki only for the years in MATARIKI. */
  function holidays(y, region = 'Auckland') {
    const out = new Set();
    const md = (m, d) => Math.round(Date.UTC(y, m - 1, d) / D2);
    const weekday = d => dow(d) !== 0 && dow(d) !== 6;
    // New Year and Christmas pairs move to the next free weekdays
    for (const pair of [[md(1, 1), md(1, 2)], [md(12, 25), md(12, 26)]]) {
      for (const d of pair) if (weekday(d)) out.add(d);
      for (const d of pair) if (!weekday(d)) { let x = d; while (!weekday(x) || out.has(x)) x++; out.add(x); }
    }
    for (const d of [md(2, 6), md(4, 25)]) { if (weekday(d) || y < 2014) out.add(d); else { let x = d; while (dow(x) !== 1) x++; out.add(x); } }
    const E = easter(y); out.add(E - 2); out.add(E + 1);
    out.add(nthMonday(y, 5, 1));    // King's Birthday: first Monday in June
    out.add(nthMonday(y, 9, 4));    // Labour Day: fourth Monday in October
    if (MATARIKI.dates[y]) out.add(md(+MATARIKI.dates[y].slice(0, 2), +MATARIKI.dates[y].slice(3)));
    // Auckland Anniversary: the Monday nearest 29 January
    if (/auckland/i.test(region || '')) { const j29 = md(1, 29), back = (dow(j29) + 6) % 7; out.add(back <= 3 ? j29 - back : j29 - back + 7); }
    return out;
  }
  const inTerm = n => { const y = diso(n).slice(0, 4), t = TERMS.dates[y]; if (!t) return null; return t.some(([s, e]) => n >= dnum(s) && n <= dnum(e)); };
  const median = xs => { if (!xs.length) return null; const s = xs.slice().sort((p, q) => p - q), m = s.length >> 1; return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2; };
  const mean = xs => xs.length ? xs.reduce((s, x) => s + x, 0) / xs.length : null;
  const sdev = xs => { if (xs.length < 2) return null; const m = mean(xs); return Math.sqrt(xs.reduce((s, x) => s + (x - m) ** 2, 0) / (xs.length - 1)); };
  function ols(pts) {
    if (pts.length < 2) return null;
    const mx = mean(pts.map(p => p[0])), my = mean(pts.map(p => p[1]));
    const sxx = pts.reduce((s, p) => s + (p[0] - mx) ** 2, 0); if (!sxx) return null;
    const b = pts.reduce((s, p) => s + (p[0] - mx) * (p[1] - my), 0) / sxx;
    return { slope: b, intercept: my - b * mx, mean: my };
  }
  /* Statistics for one site from deduplicated rows [{ SiteRef, startDate, laneNumber, flowDirection, classWeight, v }].
     Days under 50% of the 28-day median are dropped (a partial or faulty day). Returns null without data. */
  function tmsStats(rows, { ref, region = 'Auckland', hist = null, now = new Date() } = {}) {
    const day = new Map(), dirs = new Set();
    for (const r of rows) {
      const n = dnum(nzDateOf(r.startDate)), v = +r.v || 0, cw = String(r.classWeight || '');
      let t = day.get(n); if (!t) day.set(n, t = { L: 0, H: 0, O: 0 });
      // Light and Heavy add to the day; any other class (a total, say) is used only for a day without the split
      if (/heavy/i.test(cw)) t.H += v; else if (/light/i.test(cw)) t.L += v; else t.O += v;
      if (r.flowDirection != null) dirs.add(r.flowDirection);
    }
    if (!day.size) return null;
    const ds = [...day.keys()].sort((p, q) => p - q), tot = n => { const t = day.get(n); return t.L + t.H || t.O; }, R = RULES.tms;
    const valid = [], dropped = [];
    for (const n of ds) {
      const win = ds.filter(m => Math.abs(m - n) <= R.medianDays / 2).map(tot), med = median(win);
      (tot(n) >= R.minShare * med && tot(n) > 0 ? valid : dropped).push(n);
    }
    const years = [...new Set(ds.map(n => +diso(n).slice(0, 4)))], hol = new Set(years.flatMap(y => [...holidays(y, region)]));
    const vals = ns => ns.map(tot), last = ds[ds.length - 1];
    const lastYear = valid.filter(n => n > last - 365);
    const aadt = lastYear.length >= R.aadtDays ? mean(vals(lastYear)) : null;
    // AAWDT is annual too: weekdays of the same last 12 months, with the same 300-day rule; otherwise only a weekday mean
    const isWk = n => dow(n) >= 1 && dow(n) <= 5 && !hol.has(n), wk = valid.filter(isWk);
    const aawdt = aadt != null ? mean(vals(lastYear.filter(isWk))) : null;
    // Calibration target: Tue–Thu, not a holiday, in school term, latest 12 term weeks
    const tt = valid.filter(n => dow(n) >= 2 && dow(n) <= 4 && !hol.has(n) && inTerm(n) !== false).sort((p, q) => q - p);
    const weeks = new Set(), use = [];
    for (const n of tt) { const w = Math.floor((n + 3) / 7); if (!weeks.has(w) && weeks.size >= R.termWeeks[1]) break; weeks.add(w); use.push(n); }
    const tv = vals(use), tm = mean(tv), sd = sdev(tv), unknownTerm = use.some(n => inTerm(n) === null);
    const target = use.length ? { mean: tm, sd, n: use.length, ci95: sd != null ? R.z * sd / Math.sqrt(use.length) : null, weeks: weeks.size, from: diso(Math.min(...use)), to: diso(Math.max(...use)),
      ok: weeks.size >= R.termWeeks[0] && !unknownTerm, note: weeks.size < R.termWeeks[0] ? `Only ${weeks.size} school-term week${weeks.size === 1 ? '' : 's'} of Tuesday–Thursday data (${R.termWeeks[0]}–${R.termWeeks[1]} wanted): indicative.` : unknownTerm ? 'School term dates are not entered for every year used: indicative.' : '' } : null;
    const L = valid.reduce((s, n) => s + day.get(n).L, 0), H = valid.reduce((s, n) => s + day.get(n).H, 0);
    const all = mean(vals(valid));
    const dowF = [0, 1, 2, 3, 4, 5, 6].map(k => { const x = vals(valid.filter(n => dow(n) === k)); return x.length && all ? mean(x) / all : null; });
    let madt = null;
    if (aadt) { madt = {}; for (const n of lastYear) { const m = +diso(n).slice(5, 7); (madt[m] ||= []).push(tot(n)); } for (const m in madt) madt[m] = mean(madt[m]) / aadt; }
    // Growth: OLS slope of annual ADT (the NZTA site's five years of AADT, oldest first)
    let growth = null;
    if (hist && hist.some(Boolean)) {
      const Y = nzYear(now), pts = hist.map((v, i) => v ? [Y - 5 + i, v] : null).filter(Boolean), f = ols(pts);
      if (f) growth = { slope: f.slope, pct: f.mean ? f.slope / f.mean * 100 : null, years: pts.length, from: pts[0][0], to: pts[pts.length - 1][0], basis: 'OLS slope of annual ADT (NZTA site history)' };
    }
    const d0 = ds[0];
    const ok = new Set(valid);
    return { ref, from: diso(d0), to: diso(last), days: ds.length, valid: valid.length, dropped: dropped.length, adt: all, aadt, aawdt, wdt: wk.length ? { mean: mean(vals(wk)), n: wk.length } : null, target,
      hcv: L + H ? H / (L + H) : null, dow: dowF, madt, growth, dirs: [...dirs].sort(), series: { d0: diso(d0), v: ds.map(n => (ok.has(n) ? 1 : -1) * Math.round(tot(n))) },
      units: 'veh/day', method: 'weekday_factored', at: new Date().toISOString(), source: SRC.tms.name };
  }
  const tmsSites = (counts = DataHub.D().counts) => counts.filter(c => c.src === 'nzta' && !c.virtual && /^[0-9A-Z]{8}$/i.test(c.ref || ''));
  async function loadTMS(counts = tmsSites(), { days = cfg().tmsDays, now = new Date(), status = () => {} } = {}) {
    // A cleared or text 'days to download' setting uses the default (it gave a since-date of today: no data at all)
    days = Math.round(+days) > 0 ? Math.round(+days) : RULES.tms.days;
    const refs = [...new Set(counts.map(c => c.ref))], since = diso(dnum(today(now)) - days), R = RULES.tms;
    const rows = new Map(refs.map(r => [r, []]));
    let complete = true, n = 0;
    for (let i = 0; i < refs.length; i += R.chunk) {
      const part = refs.slice(i, i + R.chunk);
      status(`Downloading daily counts for sites ${i + 1}–${i + part.length} of ${refs.length} (NZTA TMS)…`);
      const r = await arcQuery(SRC.tms.url, {
        where: `SiteRef IN (${part.map(s => `'${s.replace(/'/g, '')}'`).join(',')}) AND startDate >= DATE '${since}'`,
        groupByFieldsForStatistics: 'SiteRef,startDate,laneNumber,flowDirection,classWeight',
        outStatistics: JSON.stringify([{ statisticType: 'max', onStatisticField: 'trafficCount', outStatisticFieldName: 'v' }]),
        orderByFields: 'SiteRef,startDate,laneNumber,flowDirection,classWeight', resultRecordCount: '2000',
      }, { maxPages: R.maxPages });
      complete = complete && r.complete;
      for (const f of r.features) { const A = f.attributes || {}, ref = A.SiteRef ?? A.siteref ?? A.SITEREF; if (rows.has(ref)) { rows.get(ref).push(A); n++; } }
    }
    const out = {};
    for (const c of counts) { const s = tmsStats(rows.get(c.ref) || [], { ref: c.ref, hist: c.hist, now }); if (s) out[c.ref] = s; }
    const T = F().tms ||= {};
    Object.assign(T, out);
    attachTMS();
    stamp('tms', { n: Object.keys(out).length, note: `${U.fmt.int(n)} deduplicated rows (groupBy and max) since ${since}. ${RULES.tms.hcvRule}.${complete ? '' : ' Some sites hit the download cap: shorten the period in Settings.'}` });
    return { sites: Object.keys(out).length, of: refs.length, rows: n, since, complete, stats: out };
  }
  /* Does the core turn a count's TMS weekday mean into its modelled-period volume? DataHub.periodVol uses the NZTA
     AADT today, so the count must not claim obs_method 'weekday_factored' (calibration weights it 0.7, against 0.4 for
     a factored AADT). Once periodVol reads c.tms.target.mean, the count is labelled as the spec says. */
  function periodUsesTMS() {
    try {
      const pv = c => DataHub.periodVol(c, 'AM'), a = pv({ src: 'nzta', adt: 1000, tms: { target: { mean: 2000, n: 30, ok: true } } }), b = pv({ src: 'nzta', adt: 2000 });
      return !!(a && b && b.v > 0 && Math.abs(a.v / b.v - 1) < 1e-9);
    } catch { return false; }
  }
  // The same check for the peak shares from data/tms_profiles.json (c.share)
  function periodUsesShare() {
    try { const a = DataHub.periodVol({ src: 'nzta', adt: 1000, share: { AM: 0.0123, PM: 0.0123 } }, 'AM'); return !!a && Math.abs(a.v - 12.3) < 1e-9; } catch { return false; }
  }
  // Put TMS results (and peak profiles) on the NZTA counts; count reloads drop them, so this runs on every data change
  function attachTMS() {
    if (!Store.P) return 0;
    const T = (Store.P.feeds || {}).tms, prof = (Store.P.feeds || {}).profiles;
    if (!T && !prof) return 0;
    let n = 0;
    const weekday = T ? periodUsesTMS() : false;
    for (const c of DataHub.D().counts) {
      if (c.src !== 'nzta') continue;
      const s = T && T[c.ref];
      // Older Spare Construct versions wrote c.obsMethod 'weekday_factored' although the volume came from the AADT
      if ('obsMethod' in c) delete c.obsMethod;
      if (s && c.tms !== s) { c.tms = s; n++; }
      // labelled from what the core actually does with this count: periodVol uses the TMS mean only when it is usable
      // (target.ok is not false), so an indicative mean (under 8 weeks) keeps the AADT basis and no 'weekday_factored' label
      if (s && weekday && s.target) {
        let uses = false;
        try {
          const r1 = DataHub.periodVol(c, 'AM'), r0 = DataHub.periodVol({ ...c, tms: undefined }, 'AM');
          uses = !!r1 && (r1.basis ? r1.basis === 'weekday_factored' : !!r0 && Math.abs(r1.v - r0.v) > 1e-9);
        } catch { uses = false; }
        if (uses) { c.obs_method = 'weekday_factored'; c.n_days = s.target.n; }
        else if (c.obs_method === 'weekday_factored') { delete c.obs_method; delete c.n_days; }
      }
      const p = prof && prof.sites && prof.sites[c.ref];
      if (p && !(c.share && c.share.src === prof.source)) c.share = { AM: p.amShare, PM: p.pmShare, amStart: p.amStart, pmStart: p.pmStart, src: prof.source };
    }
    return n;
  }
  /* Peak profiles: data/tms_profiles.json, built by a maintainer from the quarter-hourly archive. Expected shape:
     { source, built, sites: { <SiteRef>: { amShare, pmShare (fractions of daily), amStart, pmStart ('07:45'), phf, hcvPeak, k30 } } } */
  async function loadProfiles() {
    const j = await get(SRC.profiles.url, { timeout: 8000 });
    if (!j || !j.sites) throw new Error('data/tms_profiles.json has no sites');
    F().profiles = { source: j.source || SRC.profiles.name, built: j.built || null, sites: j.sites };
    const n = attachTMS();
    stamp('profiles', { n: Object.keys(j.sites).length, note: j.built ? `Built ${j.built}.` : '' });
    return n;
  }

  /* =====================================================================================
     4. AGS23 v1.1 growth factors (Auckland)
     ===================================================================================== */
  const agsYear = y => U.clamp(Math.round(+y || 2023), RULES.ags.years[0], RULES.ags.years[1]);
  async function loadAGS(b, years, base) {
    let meta = null; try { meta = await layerMeta(SRC.ags.url); } catch { }
    const ys = [...new Set([agsYear(base), ...years.map(agsYear)])];
    const want = ['OBJECTID', 'MSM', 'LocalBoard', ...ys.flatMap(y => [`HH_${y}`, `EMP_${y}`, `POP_${y}`])];
    const fields = resolve(meta, want);
    const r = await arcQuery(SRC.ags.url, { where: '1=1', outFields: fields.join(','), ...env(padBox(b, RULES.ags.gatewayKm * 1000)), orderByFields: 'OBJECTID' });
    const get3 = (A, k, y) => { const f = Object.keys(A).find(x => x.toLowerCase() === `${k}_${y}`.toLowerCase()); return f ? +A[f] : null; };
    const zones = r.features.filter(x => x.geometry && x.geometry.rings).map(x => {
      const A = x.attributes || {}, v = {};
      for (const y of ys) v[y] = { hh: get3(A, 'HH', y), emp: get3(A, 'EMP', y), pop: get3(A, 'POP', y) };
      return { msm: String(A.MSM ?? A.msm ?? A.OBJECTID), board: A.LocalBoard || '', rings: x.geometry.rings, bb: ringsBox(x.geometry.rings), c: centroid(x.geometry.rings), v };
    });
    mem.ags = { zones, years: ys, at: Date.now() };
    stamp('ags', { n: zones.length, note: `${SRC.ags.attribution}; factor method by MSM zone. Not saved with the project.` });
    return mem.ags;
  }
  const ratio = (a, b) => a > 0 && b != null ? b / a : null;
  const agsRangeText = (base, year, f) => `AGS23 covers ${RULES.ags.years[0]}–${RULES.ags.years[1]}, so this uses growth from ${f.base} to ${f.year} (asked: ${base} to ${year})`;
  // Factor for one zone (or point) from the MSM zone containing it
  function agsZoneFactor(ll, base, year, data = mem.ags) {
    if (!data) return null;
    const B = agsYear(base), Y = agsYear(year), z = data.zones.find(m => ll[1] >= m.bb[0] && ll[1] <= m.bb[2] && ll[0] >= m.bb[1] && ll[0] <= m.bb[3] && inRings(ll[1], ll[0], m.rings));
    if (!z || !z.v[B] || !z.v[Y]) return null;
    return { msm: z.msm, board: z.board, base: B, year: Y, hh: ratio(z.v[B].hh, z.v[Y].hh), emp: ratio(z.v[B].emp, z.v[Y].emp), pop: ratio(z.v[B].pop, z.v[Y].pop) };
  }
  const existingZone = z => z.kind === 'zone' && (z.existing === true || z.src === 'existing-lu');
  // Proposed land-use changes for every existing zone in every forecast scenario: rows [{ id, sc, zone, f, lu: {k: [from, to]} }]
  function agsProposals(P = Store.P, data = mem.ags) {
    const S = P.settings, rows = [];
    for (const sc of P.scenarios) {
      const Y = Engine.yearOf(sc, S); if (Y === S.baseYear) continue;
      for (const z of Object.values(sc.zones)) {
        if (!existingZone(z)) continue;
        const f = agsZoneFactor([z.lat, z.lng], S.baseYear, Y, data);
        const base = z.luBase || z.lu || {}, lu = {};
        if (f) for (const [k, q] of Object.entries(base)) { const g = RULES.ags.groups[(LANDUSES[k] || {}).group] || 'emp', m = f[g]; if (m != null && +q) lu[k] = [(z.lu || {})[k], Math.round(q * m * 10) / 10]; }
        // A land use the user edited after Spare Construct applied a factor is the user's
        const user = !!(z.ags && z.ags.lu && Object.entries(z.ags.lu).some(([k, q]) => (z.lu || {})[k] !== q));
        // AGS23 runs 2022–2052: a year outside that uses the nearest AGS23 year, and the row says so
        const clamped = f && (f.year !== Y || f.base !== S.baseYear) ? agsRangeText(S.baseYear, Y, f) : '';
        rows.push({ id: sc.id + '|' + z.id, sc: sc.id, scName: sc.name, zone: z.id, name: z.name || 'Zone', year: Y, f, lu, user, outside: !f, clamped });
      }
    }
    return rows;
  }
  function applyAGS(P, rows, ids, when = today()) {
    const pick = new Set(ids); let n = 0;
    for (const r of rows) {
      if (!pick.has(r.id) || !r.f) continue;
      const sc = P.scenarios.find(s => s.id === r.sc), z = sc && sc.zones[r.zone]; if (!z) continue;
      if (!z.luBase) z.luBase = { ...(z.lu || {}) };
      // A new object, never an edit in place: zones built from shared templates (samples, tests) must not change them
      z.lu = { ...(z.lu || {}), ...Object.fromEntries(Object.entries(r.lu).map(([k, [, to]]) => [k, to])) };
      z.ags = { msm: r.f.msm, base: r.f.base, year: r.f.year, f: { hh: r.f.hh, emp: r.f.emp, pop: r.f.pop }, lu: Object.fromEntries(Object.entries(r.lu).map(([k, v]) => [k, v[1]])), at: when, src: agsBasisText, ...(r.clamped ? { note: r.clamped } : {}) };
      n++;
    }
    return n;
  }
  /* Growth beyond a gateway: households + jobs of MSM zones whose centre lies outside the study area, within 10 km of
     the gateway and within ±60° of the direction from the study-area centre to the gateway; shown as an equivalent
     linear % a year so it compares with the growth setting. */
  function agsGateway(sc, z, base, year, data = mem.ags) {
    if (!data) return null;
    const B = agsYear(base), Y = agsYear(year), area = netBox(sc); if (!area || Y === B) return null;
    const c = [(area.s + area.n) / 2, (area.w + area.e) / 2], g = [z.lat, z.lng], dir = U.bearing(c, g), R = RULES.ags;
    let b0 = 0, b1 = 0; const used = [];
    for (const m of data.zones) {
      if (inBox(area, m.c) || U.dist(g, m.c) > R.gatewayKm * 1000 || U.angDiff(U.bearing(c, m.c), dir) > R.gatewayDeg) continue;
      const v0 = m.v[B], v1 = m.v[Y]; if (!v0 || !v1) continue;
      const x0 = (v0.hh || 0) + (v0.emp || 0), x1 = (v1.hh || 0) + (v1.emp || 0); if (!x0) continue;
      b0 += x0; b1 += x1; used.push(m.msm);
    }
    if (!b0) return { base: B, year: Y, factor: null, pct: null, n: 0, msm: [], basis: `No AGS23 zones lie beyond this gateway within ${R.gatewayKm} km.` };
    const factor = b1 / b0, pct = (factor - 1) / (Y - B) * 100, cl = Y !== +year || B !== +base ? ` ${agsRangeText(base, year, { base: B, year: Y })}.` : '';
    return { base: B, year: Y, factor, pct, n: used.length, msm: used.slice(0, 20),
      basis: `Households + jobs in ${used.length} MSM zone${used.length === 1 ? '' : 's'} beyond the gateway (within ${R.gatewayKm} km, ±${R.gatewayDeg}°): ${U.fmt.int(b0)} in ${B} → ${U.fmt.int(b1)} in ${Y}, ×${factor.toFixed(3)} = ${pct.toFixed(2)}% a year, linear.${cl}` };
  }
  function agsGateways(P = Store.P, data = mem.ags) {
    const S = P.settings, out = [];
    for (const sc of P.scenarios) {
      const Y = Engine.yearOf(sc, S);
      for (const z of Object.values(sc.zones)) if (z.kind === 'gateway') { const g = agsGateway(sc, z, S.baseYear, Y, data); if (g) { z.agsGrowth = { ...g, at: today(), src: SRC.ags.attribution }; out.push({ sc: sc.id, zone: z.id, name: z.name, ...g }); } }
    }
    return out;
  }
  const agsBasisText = 'Land-use growth: AGS23 v1.1 by MSM zone, factor method';

  /* =====================================================================================
     5. GTFS bus frequencies (Mobility Database mirror)
     ===================================================================================== */
  async function catalogue(b) {
    const B = box(b);
    try {
      if (!mem.catalogue) {
        const text = await get(SRC.gtfsCat.url, { as: 'text', timeout: Math.max(cfg().timeout, 45) * 1000 });
        const rows = U.parseCSV(text), h = rows[0].map(s => s.trim()), col = n => h.indexOf(n);
        const c = { id: col('id'), type: col('data_type'), status: col('status'), cc: col('location.country_code'), prov: col('provider'), name: col('name'), latest: col('urls.latest'), lic: col('urls.license'),
          s: col('location.bounding_box.minimum_latitude'), n: col('location.bounding_box.maximum_latitude'), w: col('location.bounding_box.minimum_longitude'), e: col('location.bounding_box.maximum_longitude') };
        mem.catalogue = rows.slice(1).filter(r => r[c.type] === 'gtfs' && (!r[c.status] || r[c.status] === 'active')).map(r => ({
          id: r[c.id], provider: r[c.prov] || '', name: c.name >= 0 ? r[c.name] || '' : '', cc: r[c.cc] || '', latest: c.latest >= 0 ? r[c.latest] : '', licence: c.lic >= 0 ? r[c.lic] : '',
          box: [+r[c.s], +r[c.w], +r[c.n], +r[c.e]] })).filter(f => f.box.every(isFinite));
        stamp('gtfsCat', { n: mem.catalogue.length });
      }
      const hit = mem.catalogue.filter(f => f.box[0] <= B.n && f.box[2] >= B.s && f.box[1] <= B.e && f.box[3] >= B.w);
      if (hit.length) return { feeds: hit.sort((p, q) => rank(p, B) - rank(q, B) || area4(p.box) - area4(q.box)), from: 'catalogue' };
      throw new Error('no feed in the list covers this area');
    } catch (e) {
      const hit = NZ_FEEDS.filter(f => f.box[0] <= B.n && f.box[2] >= B.s && f.box[1] <= B.e && f.box[3] >= B.w);
      return { feeds: hit.sort((p, q) => rank(p, B) - rank(q, B) || area4(p.box) - area4(q.box)), from: 'built-in list', error: e.message };
    }
  }
  const area4 = b => Math.abs((b[2] - b[0]) * (b[3] - b[1]));
  /* Catalogue boxes can be far too big (the AT feed's reaches the Chathams because of stray stops), so a feed whose
     Spare Construct coverage box holds the study-area centre comes first; then the smallest box. */
  const rank = (f, B) => { const k = NZ_FEEDS.find(x => x.id === f.id), cy = (B.s + B.n) / 2, cx = (B.w + B.e) / 2; return k && cy >= k.box[0] && cy <= k.box[2] && cx >= k.box[1] && cx <= k.box[3] ? 0 : 1; };
  const feedUrl = f => f.latest || SRC.gtfs.url.replace('{id}', encodeURIComponent(f.id));

  /* The GTFS reader. Self-contained (it is sent to a Web Worker as source text): it uses only DecompressionStream and
     TextDecoder. Reads the zip's central directory, then streams each member line by line, so the 91 MB AT
     stop_times.txt never sits in memory as one string. o = { box: [s, w, n, e], today: 'YYYYMMDD', holidays: [...],
     am: [7, 9], pm: [16, 18] } → { date, stale, stops, patterns, trips, feedInfo, agencies, ms }. */
  async function gtfsWork(buf, o) {
    const t0 = Date.now(), u8 = buf instanceof Uint8Array ? buf : new Uint8Array(buf), dv = new DataView(u8.buffer, u8.byteOffset, u8.byteLength), td = new TextDecoder('utf-8');
    let eocd = -1;
    for (let i = u8.length - 22; i >= Math.max(0, u8.length - 65557); i--) if (dv.getUint32(i, true) === 0x06054b50) { eocd = i; break; }
    if (eocd < 0) throw new Error('The download is not a zip file. The feed may have moved; try again later.');
    const nEnt = dv.getUint16(eocd + 10, true); let p = dv.getUint32(eocd + 16, true);
    if (nEnt === 0xFFFF || p === 0xFFFFFFFF) throw new Error('This feed uses the ZIP64 format, which Spare Construct cannot read yet.');
    const files = {};
    for (let k = 0; k < nEnt && p + 46 <= u8.length && dv.getUint32(p, true) === 0x02014b50; k++) {
      const nl = dv.getUint16(p + 28, true), xl = dv.getUint16(p + 30, true), cl = dv.getUint16(p + 32, true);
      files[td.decode(u8.subarray(p + 46, p + 46 + nl)).replace(/^.*\//, '')] = { method: dv.getUint16(p + 10, true), csize: dv.getUint32(p + 20, true), lho: dv.getUint32(p + 42, true) };
      p += 46 + nl + xl + cl;
    }
    const split = s => {
      if (s.indexOf('"') < 0) return s.split(',');
      const out = []; let cur = '', q = false;
      for (let i = 0; i < s.length; i++) { const ch = s[i]; if (q) { if (ch === '"') { if (s[i + 1] === '"') { cur += '"'; i++; } else q = false; } else cur += ch; } else if (ch === '"') q = true; else if (ch === ',') { out.push(cur); cur = ''; } else cur += ch; }
      out.push(cur); return out;
    };
    async function rows(name, fn, need = true) {
      const f = files[name];
      if (!f) { if (need) throw new Error(`The feed has no ${name}, so it cannot be used.`); return 0; }
      if (f.csize === 0xFFFFFFFF) throw new Error('This feed uses the ZIP64 format, which Spare Construct cannot read yet.');
      const h = f.lho, at = h + 30 + dv.getUint16(h + 26, true) + dv.getUint16(h + 28, true), raw = u8.subarray(at, at + f.csize);
      let head = null, rest = '', count = 0;
      const take = (text, last) => {
        rest += text; const parts = rest.split('\n'); rest = last ? '' : parts.pop();
        for (let line of parts) {
          if (line.endsWith('\r')) line = line.slice(0, -1); if (!line) continue;
          const c = split(line);
          if (!head) { head = {}; c.forEach((k, i) => { head[k.replace(/^﻿/, '').trim()] = i; }); continue; }
          count++; fn(c, head);
        }
      };
      if (f.method === 0) take(new TextDecoder('utf-8').decode(raw), true);
      else if (f.method === 8) {
        if (typeof DecompressionStream === 'undefined') throw new Error('This browser cannot unzip files (no DecompressionStream). Use a current Chrome, Edge, Firefox or Safari.');
        const ds = new DecompressionStream('deflate-raw'), w = ds.writable.getWriter(), r = ds.readable.getReader(), dec = new TextDecoder('utf-8');
        w.write(raw).catch(() => {}); w.close().catch(() => {});
        for (;;) { const { done, value } = await r.read(); if (done) break; take(dec.decode(value, { stream: true }), false); }
        take(dec.decode(), true);
      } else throw new Error(`${name} uses zip method ${f.method}, which Spare Construct cannot read.`);
      return count;
    }
    const ymd = n => new Date(n * 86400000).toISOString().slice(0, 10).replace(/-/g, '');
    const num = s => Math.round(Date.UTC(+s.slice(0, 4), +s.slice(4, 6) - 1, +s.slice(6, 8)) / 86400000);
    const sec = s => { const m = /^\s*(\d+):(\d{2})(?::(\d{2}))?/.exec(s || ''); return m ? +m[1] * 3600 + +m[2] * 60 + +(m[3] || 0) : NaN; };
    // 1. calendar → a representative Tuesday in service that is not a public holiday
    const cal = [], ex = new Map(), hol = new Set(o.holidays || []);
    await rows('calendar.txt', (c, h) => cal.push({ id: c[h.service_id], tue: c[h.tuesday] === '1', s: c[h.start_date], e: c[h.end_date] }), false);
    await rows('calendar_dates.txt', (c, h) => { const d = c[h.date]; let m = ex.get(d); if (!m) ex.set(d, m = new Map()); m.set(c[h.service_id], c[h.exception_type]); }, false);
    const active = d => { const s = new Set(); for (const r of cal) if (r.tue && r.s <= d && d <= r.e) s.add(r.id); for (const [id, t] of ex.get(d) || []) { if (t === '1') s.add(id); else s.delete(id); } return s; };
    let date = null, services = null, stale = false;
    const t = num(o.today);
    // School terms (o.terms [[YYYYMMDD, YYYYMMDD]…]): a term-time Tuesday first; a year without term dates counts as unknown
    const terms = o.terms || [], tYears = new Set(terms.map(x => x[0].slice(0, 4)));
    const termOf = d => tYears.has(d.slice(0, 4)) ? terms.some(([s, e]) => d >= s && d <= e) : null;
    for (const needTerm of [true, false]) for (let k = 0; k < 200 && !date; k++) { const n = t + k; if ((n + 4) % 7 !== 2 || hol.has(ymd(n)) || (needTerm && termOf(ymd(n)) === false)) continue; const s = active(ymd(n)); if (s.size) { date = ymd(n); services = s; } }
    for (let k = 1; k < 400 && !date; k++) { const n = t - k; if ((n + 4) % 7 !== 2 || hol.has(ymd(n))) continue; const s = active(ymd(n)); if (s.size) { date = ymd(n); services = s; stale = true; } }
    if (!date) throw new Error('The feed has no Tuesday with buses in service. It may have expired.');
    // 2. stops in the box, bus routes, trips that day
    const [bs, bw, bn, be] = o.box, stops = new Map(), routes = new Map(), trips = new Map();
    await rows('stops.txt', (c, h) => { const lat = +c[h.stop_lat], lng = +c[h.stop_lon]; if (lat >= bs && lat <= bn && lng >= bw && lng <= be) stops.set(c[h.stop_id], { id: c[h.stop_id], code: h.stop_code != null ? c[h.stop_code] || '' : '', name: c[h.stop_name] || '', lat, lng, routes: new Set(), am: 0, pm: 0 }); });
    await rows('routes.txt', (c, h) => { const ty = +c[h.route_type]; if (ty === 3 || (ty >= 700 && ty < 800)) routes.set(c[h.route_id], { short: c[h.route_short_name] || '', long: c[h.route_long_name] || '' }); });
    await rows('trips.txt', (c, h) => { const r = c[h.route_id]; if (routes.has(r) && services.has(c[h.service_id])) trips.set(c[h.trip_id], { r, dir: h.direction_id != null ? c[h.direction_id] || '0' : '0', head: h.trip_headsign != null ? c[h.trip_headsign] || '' : '', st: [] }); });
    let nst = 0;
    nst = await rows('stop_times.txt', (c, h) => { const tr = trips.get(c[h.trip_id]); if (!tr || !stops.has(c[h.stop_id])) return; tr.st.push([+c[h.stop_sequence], c[h.stop_id], sec(c[h.departure_time] || c[h.arrival_time])]); });
    // 3. patterns (route, direction and the stops in the box) and departures in the AM and PM windows
    const win = (x, w) => x >= w[0] * 3600 && x < w[1] * 3600, pats = new Map();
    for (const tr of trips.values()) {
      if (!tr.st.length) continue;
      tr.st.sort((x, y) => x[0] - y[0]);
      const R = routes.get(tr.r), label = R.short || R.long, first = tr.st.find(x => isFinite(x[2])), t1 = first ? first[2] : NaN;
      for (const x of tr.st) { const S = stops.get(x[1]); S.routes.add(label); if (isFinite(x[2])) { if (win(x[2], o.am)) S.am++; if (win(x[2], o.pm)) S.pm++; } }
      const am = win(t1, o.am), pm = win(t1, o.pm), seq = tr.st.map(x => x[1]);
      if (seq.length < 2 || !(am || pm)) continue;
      const key = tr.r + '|' + tr.dir + '|' + seq.join('>');
      let P = pats.get(key); if (!P) pats.set(key, P = { route: tr.r, short: R.short, long: R.long, dir: tr.dir, head: tr.head, stops: seq, am: 0, pm: 0 });
      if (am) P.am++; if (pm) P.pm++;
    }
    let feedInfo = null; const agencies = [];
    await rows('feed_info.txt', (c, h) => { feedInfo = { publisher: c[h.feed_publisher_name] || '', url: h.feed_publisher_url != null ? c[h.feed_publisher_url] : '', start: c[h.feed_start_date] || '', end: c[h.feed_end_date] || '', version: h.feed_version != null ? c[h.feed_version] : '' }; }, false);
    await rows('agency.txt', (c, h) => agencies.push(c[h.agency_name] || ''), false);
    const hA = o.am[1] - o.am[0], hP = o.pm[1] - o.pm[0], byNum = (x, y) => (parseFloat(x) || 1e9) - (parseFloat(y) || 1e9) || String(x).localeCompare(String(y));
    return { date, stale, term: termOf(date), services: services.size, trips: trips.size, stopTimes: nst, feedInfo, agencies,
      stops: [...stops.values()].filter(s => s.routes.size).map(s => ({ id: s.id, code: s.code, name: s.name, lat: s.lat, lng: s.lng, routes: [...s.routes].sort(byNum), am: s.am / hA, pm: s.pm / hP })),
      patterns: [...pats.values()].map(q => ({ ...q, am: q.am / hA, pm: q.pm / hP })), ms: Date.now() - t0 };
  }
  // Run the reader in a Web Worker when the browser allows (the UI stays responsive); otherwise inline
  function runGtfs(buf, o) {
    if (typeof Worker === 'undefined' || typeof Blob === 'undefined' || typeof URL.createObjectURL !== 'function') return gtfsWork(buf, o);
    return new Promise((resolve, reject) => {
      const src = `'use strict';const gtfsWork=${gtfsWork.toString()};onmessage=async e=>{try{postMessage({ok:true,r:await gtfsWork(e.data.buf,e.data.o)})}catch(err){postMessage({ok:false,e:String(err&&err.message||err)})}};`;
      let w, url;
      try { url = URL.createObjectURL(new Blob([src], { type: 'text/javascript' })); w = new Worker(url); } catch { resolve(gtfsWork(buf, o)); return; }
      const done = () => { try { w.terminate(); URL.revokeObjectURL(url); } catch { } };
      w.onmessage = e => { done(); if (e.data.ok) resolve(e.data.r); else reject(new Error(e.data.e)); };
      w.onerror = e => { done(); if (e && e.preventDefault) e.preventDefault(); resolve(gtfsWork(buf, o)); };
      w.postMessage({ buf, o });
    });
  }
  // IndexedDB cache for parsed feeds (per feed, study area and Last-Modified); fails soft to no cache
  function idb() {
    return new Promise(res => {
      try { if (typeof indexedDB === 'undefined') return res(null); const q = indexedDB.open('corridor-feeds', 1); q.onupgradeneeded = () => q.result.createObjectStore('cache'); q.onsuccess = () => res(q.result); q.onerror = () => res(null); }
      catch { res(null); }
    });
  }
  async function cacheGet(key) { const db = await idb(); if (!db) return null; return new Promise(res => { try { const q = db.transaction('cache').objectStore('cache').get(key); q.onsuccess = () => res(q.result || null); q.onerror = () => res(null); } catch { res(null); } }); }
  async function cachePut(key, val) { const db = await idb(); if (!db) return; try { db.transaction('cache', 'readwrite').objectStore('cache').put(val, key); } catch { } }
  async function loadGTFS(feed, b, { now = new Date(), status = () => {} } = {}) {
    const B = padBox(b, RULES.gtfs.bufferM), url = feedUrl(feed), d = today(now);
    // Holidays from last year too: an out-of-date feed is searched backwards for its last Tuesday
    const yr = +d.slice(0, 4), hol = [yr - 1, yr, yr + 1].flatMap(y => [...holidays(y, feed.provider)]).map(n => diso(n).replace(/-/g, ''));
    const terms = Object.values(TERMS.dates).flat().map(([s, e]) => [s.replace(/-/g, ''), e.replace(/-/g, '')]);
    const o = { box: [B.s, B.w, B.n, B.e], today: d.replace(/-/g, ''), holidays: hol, terms, am: RULES.periods.AM, pm: RULES.periods.PM };
    // Cached per feed, study area and Last-Modified; a cached reading is reused while its service day is still ahead
    const key = `gtfs:${feed.id}:${o.box.map(v => v.toFixed(3)).join(',')}`;
    let lm = null;
    try { lm = (await get(url, { method: 'HEAD', timeout: 10000 })).lastModified; } catch { }
    const hit = lm ? await cacheGet(key) : null;
    let r;
    if (hit && hit.lastModified === lm && hit.result && (hit.result.stale || hit.result.date >= o.today)) { r = hit.result; r.cached = true; }
    else {
      status(`Downloading the ${feed.provider || feed.id} timetable…`);
      const dl = await get(url, { as: 'buffer', timeout: Math.max(cfg().timeout, 120) * 1000 });
      status(`Reading ${U.fmt.num(dl.buf.byteLength / 1048576, 1)} MB of timetable for ${o.today.slice(0, 4)}…`);
      r = await runGtfs(dl.buf, o);
      r.lastModified = dl.lastModified || lm;
      cachePut(key, { lastModified: r.lastModified, result: r });
    }
    r.feed = { id: feed.id, provider: feed.provider, name: feed.name || '', url, licence: feed.licence || '' };
    return r;
  }
  /* Bus routes for a scenario from the parsed feed. Each stop snaps to the nearest road (not a motorway or ramp)
     within 60 m, and the route passes the end of that road towards the next stop, so stops in the middle of long rural
     links still count. Stops outside the network are skipped; the engine finds the shortest path between waypoints. */
  function gtfsRoutes(sc, g) {
    const byStop = new Map(g.stops.map(s => [s.id, s])), snap = new Map(), R = RULES.gtfs;
    const busRoad = L => L.status !== 'closed' && (CLASSES[L.cls] || CLASSES.local).rank < 5;
    const linkOf = id => { if (!snap.has(id)) { const s = byStop.get(id); snap.set(id, s ? Net.nearestLink(sc, [s.lat, s.lng], R.linkM, busRoad) : null); } return snap.get(id); };
    const ll = id => { const s = byStop.get(id); return s ? [s.lat, s.lng] : null; };
    const node = id => { const n = sc.nodes[id]; return [n.lat, n.lng]; };
    const merged = new Map();
    for (const p of g.patterns) {
      const wp = [];
      p.stops.forEach((s, i) => {
        const hit = linkOf(s); if (!hit) return;
        const L = sc.links[hit.id], nx = ll(p.stops[i + 1]), pv = ll(p.stops[i - 1]);
        // the exit end: nearer the next stop, or (for the last stop) further from the previous one
        const n = nx ? (U.dist(node(L.a), nx) <= U.dist(node(L.b), nx) ? L.a : L.b) : pv ? (U.dist(node(L.a), pv) >= U.dist(node(L.b), pv) ? L.a : L.b) : L.b;
        if (wp[wp.length - 1] !== n) wp.push(n);
      });
      if (new Set(wp).size < 2) continue;
      const key = p.route + '|' + p.dir + '|' + wp.join('>'), m = merged.get(key);
      if (m) { m.am += p.am; m.pm += p.pm; } else merged.set(key, { ...p, wp });
    }
    let i = 0;
    return [...merged.values()].sort((x, y) => (y.am + y.pm) - (x.am + x.pm)).map(p => {
      const AM = U.r1(p.am), PM = U.r1(p.pm), name = `${p.short || p.long || 'Bus'}${p.head ? ' to ' + p.head : ''}`;
      return { id: 'gtfs-' + hashStr(p.route + '|' + p.dir + '|' + p.wp.join('>')), name, color: ROUTE_COLOURS[i++ % ROUTE_COLOURS.length], freq: Math.max(AM, PM), freqs: { AM, PM }, wp: p.wp,
        src: 'GTFS', gtfs: { feed: g.feed.id, provider: g.feed.provider, date: g.date, route: p.short, dir: p.dir, stops: p.stops.length } };
    });
  }
  const hashStr = s => { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return (h >>> 0).toString(36); };
  // Replace earlier GTFS routes in a scenario with these (call Store.snapshot() first); returns the count added
  function applyGTFS(sc, routes) {
    for (const [id, r] of Object.entries(sc.routes || {})) if (r.src === 'GTFS') delete sc.routes[id];
    sc.routes ||= {};
    for (const r of routes) sc.routes[r.id] = JSON.parse(JSON.stringify(r));
    return routes.length;
  }
  // The feed's own date: Last-Modified of the download, then feed_info version and validity
  const feedDate = g => { const fi = g.feedInfo || {}, lm = g.lastModified ? dateText(g.lastModified) : '', v = [fi.version && `version ${fi.version}`, fi.start && fi.end && `valid ${fi.start}–${fi.end}`].filter(Boolean).join(', ');
    return [lm && `feed dated ${lm}`, v].filter(Boolean).join('; '); };
  function keepGTFS(g, routes) {
    F().gtfs = { at: new Date().toISOString(), feed: g.feed, date: g.date, stale: g.stale, term: g.term ?? null, feedDate: feedDate(g), publisher: (g.feedInfo && g.feedInfo.publisher) || g.agencies.join(', '), routes: routes.length,
      stops: g.stops.map(s => ({ id: s.id, code: s.code, name: s.name, mode: 'Bus', lat: U.r7(s.lat), lng: U.r7(s.lng), routes: s.routes, am: U.r1(s.am), pm: U.r1(s.pm) })) };   // only bus routes are read (route_type 3, 700–799), so every stop kept is a bus stop
    const fd = F().gtfs.feedDate;
    stamp('gtfs', { key: 'gtfs', url: g.feed.url, licence: g.feed.licence || SRC.gtfs.licence, n: g.stops.length,
      note: `${F().gtfs.publisher || g.feed.provider}${fd ? ` (${fd})` : ''}; service day ${g.date.slice(6, 8)}/${g.date.slice(4, 6)}/${g.date.slice(0, 4)}${g.stale ? ' (the feed has no later Tuesday: it may be out of date)' : g.term === false ? ' (not in a school term: the feed has no term-time Tuesday)' : ''}; ${routes.length} route patterns in the study area.` });
  }

  /* =====================================================================================
     6. Council count layers (generic ArcGIS importer)
     ===================================================================================== */
  function guessFields(meta) {
    const fs = (meta.fields || []).filter(f => !/oid|geometry|globalid/i.test(f.type || '')), R = RULES.counts;
    const best = res => { for (const re of res) { const f = fs.find(x => re.test(x.name) || re.test(x.alias || '')); if (f) return f.name; } return ''; };
    const years = fs.map(f => { const m = R.year.exec(f.name); return m ? { name: f.name, year: +m[1] } : null; }).filter(Boolean).sort((p, q) => q.year - p.year);
    return { adt: best(R.adt), peak: best(R.peakVol), peakTime: best(R.peakTime), hcv: best(R.hcv), date: best(R.date), name: best(R.name), dir: best(R.dir), latest: best(R.latest), years: years.map(y => y.name) };
  }
  async function councilRead(url) {
    const u = String(url || '').trim().replace(/\/query.*$/i, '').replace(/\?.*$/, '').replace(/\/+$/, '');
    if (!/^https:\/\/[^/]+\/.+\/(FeatureServer|MapServer)\/\d+$/i.test(u)) throw new Error('Paste the address of one ArcGIS layer, ending in /FeatureServer/0 (or another layer number).');
    const meta = await layerMeta(u);
    if (!meta.fields) throw new Error('That address is not a feature layer with fields.');
    return { url: u, name: meta.name || 'Count layer', meta, map: guessFields(meta), fields: meta.fields.map(f => f.name) };
  }
  const yearOfVal = v => { if (v == null || v === '') return null; if (typeof v === 'number' && v > 1e10) return new Date(v).getUTCFullYear(); const m = /(\d{4})/.exec(String(v)); return m ? +m[1] : null; };
  async function councilImport(read, map, b, { preset = null, now = new Date(), status = () => {} } = {}) {
    const P = preset ? COUNCILS[preset] : null, fields = [...new Set(['OBJECTID', ...Object.values(map).flat()].filter(Boolean))];
    const where = map.latest ? `${map.latest} IN ('Y','Yes','YES','y')` : '1=1';
    const r = await arcQuery(read.url, { where, outFields: resolve(read.meta, fields).join(',') || '*', ...env(b) }, { status: n => status(`Downloading council counts… ${n}`) });
    const Y = nzYear(now), maxAge = (P && P.maxAgeYears) || null, list = [], key = hashStr(read.url);
    for (const x of r.features) {
      const A = x.attributes || {}, g = x.geometry; if (!g) continue;
      let ll = null;
      if (g.x != null) ll = [g.y, g.x];
      else if (g.paths && g.paths[0]) { const pa = g.paths[0].map(q => [q[1], q[0]]), cum = U.cumLen(pa); ll = U.along(pa, cum, cum[cum.length - 1] / 2).p; }
      else if (g.points && g.points[0]) ll = [g.points[0][1], g.points[0][0]];
      if (!ll) continue;
      let adt = map.adt ? +A[map.adt] : null, year = map.date ? yearOfVal(A[map.date]) : null;
      if (!(adt > 0) && map.years) for (const f of map.years) if (+A[f] > 0) { adt = +A[f]; year = yearOfVal(f); break; }
      if (!(adt > 0)) continue;
      if (maxAge && year && Y - year > maxAge) continue;
      const dir = map.dir ? (/\b(NB|SB|EB|WB)\b/i.exec(String(A[map.dir] || '').toUpperCase()) || [])[1] || null : null;
      const peak = map.peak ? +A[map.peak] || null : null, pt = map.peakTime ? String(A[map.peakTime] ?? '').replace(/\D/g, '').padStart(4, '0') : '';
      const h = pt ? +pt.slice(0, 2) + +pt.slice(2, 4) / 60 : null;
      const c = { id: `feeds:${key}:${A.OBJECTID ?? list.length}`, src: 'user', name: String((map.name && A[map.name]) || `${read.name} site ${A.OBJECTID ?? list.length + 1}`), lat: U.r7(ll[0]), lng: U.r7(ll[1]),
        adt: Math.round(adt), hv: map.hcv && A[map.hcv] != null && isFinite(+A[map.hcv]) ? +A[map.hcv] : null, dir, directional: !!dir, date: year ? String(year) : null,
        am: peak && h != null && h >= 6 && h < 10 ? peak : null, pm: peak && h != null && h >= 14.5 && h < 19 ? peak : null,
        note: `${(P && P.label) || read.name}${year ? ', ' + year : ''}`, obsMethod: 'council count',
        feed: { source: (P && P.label) || read.name, url: read.url, date: today(now), licence: SRC.council.licence } };
      list.push(c);
    }
    const d = DataHub.D();
    d.counts = d.counts.filter(c => !(c.feed && c.feed.url === read.url)).concat(list);
    stamp('council', { key: 'council:' + key, url: read.url, source: (P && P.label) || read.name, n: list.length,
      note: `Fields used: ${Object.entries(map).filter(([, v]) => v && v.length).map(([k, v]) => `${k} = ${[].concat(v).slice(0, 3).join(', ')}`).join('; ')}.${maxAge ? ` Counts older than ${maxAge} years left out.` : ''}${r.complete ? '' : ' Not every page downloaded.'}` });
    return { n: list.length, of: r.features.length, complete: r.complete };
  }

  /* =====================================================================================
     7. Flood plains and coastal inundation (climate screening)
     ===================================================================================== */
  const floodLabel = A => {
    const cc = String(A.CLIMATE_CHANGE_ADJUSTED || ''), adj = cc && !/^none$/i.test(cc.trim()) && !/^no\b/i.test(cc.trim());
    const ev = +A.RAINFALL_EVENT, aep = ev > 0 ? `${U.fmt.num(100 / ev, ev > 100 ? 1 : 0)}% AEP` : 'event not stated';
    return `Flood plain, ${aep}, ${adj ? 'climate-change adjusted' : 'current climate'}`;
  };
  /* The coastal layers used (docs/spec/climate-overlay): the 1% AEP storm tide and mean high water springs plus 0.5,
     1.0 and 1.5 m of sea-level rise. Auckland Council publishes about 40 other coastal services (other return periods);
     downloading them all took over 3 minutes for Drury, so only these are asked for. */
  const COASTAL = [
    ['Coastal_Inundation_1_AEP', 'Coastal inundation, 1% AEP storm tide'],
    ['Mean_High_Water_Springs_05m_sea_level_rise', 'Coastal inundation, MHWS + 0.5 m sea-level rise'],
    ['Mean_High_Water_Springs_10m_sea_level_rise', 'Coastal inundation, MHWS + 1.0 m sea-level rise'],
    ['Mean_High_Water_Springs_15m_sea_level_rise', 'Coastal inundation, MHWS + 1.5 m sea-level rise'],
  ];
  // Polygon detail asked of the server: about 1 m (0.00001°) keeps lengths well inside the 5 m check and cuts the download
  const HAZ_GEOM = { geometryPrecision: '6', maxAllowableOffset: '0.00001' };
  async function loadHazards(b, { status = () => {} } = {}) {
    const layers = new Map(), errors = [];
    const addPoly = (label, src, rings, A) => { let L = layers.get(label); if (!L) layers.set(label, L = { label, src, polys: [] }); L.polys.push({ rings, bb: ringsBox(rings), attrs: A }); };
    status('Downloading Auckland Council flood plains…');
    try {
      let meta = null; try { meta = await layerMeta(SRC.flood.url); } catch { }
      const r = await arcQuery(SRC.flood.url, { where: '1=1', outFields: resolve(meta, ['OBJECTID', 'Hazard', 'RAINFALL_EVENT', 'CLIMATE_CHANGE_ADJUSTED', 'Superseded_Date', 'YEAR_PRODUCED', 'DOCUMENT_NAME']).join(','), ...env(b), ...HAZ_GEOM, orderByFields: 'OBJECTID' });
      let sup = 0;
      for (const x of r.features) { const A = x.attributes || {}; if (!x.geometry || !x.geometry.rings) continue; if (A.Superseded_Date) { sup++; continue; } addPoly(floodLabel(A), 'flood', x.geometry.rings, A); }
      stamp('flood', { n: r.features.length - sup, note: `${sup ? sup + ' superseded polygons left out. ' : ''}Each label merges overlapping models, so a length is never counted twice.` });
    } catch (e) { errors.push(`Flood plains: ${e.message}`); failStamp('flood', e); }
    status('Looking for coastal inundation layers…');
    try {
      const list = await get(AC + '?f=json'), have = new Set((list.services || []).map(s => s.name.replace(/^.*\//, '')));
      const use = COASTAL.filter(([nm]) => have.has(nm)), gone = COASTAL.filter(([nm]) => !have.has(nm));
      if (!use.length) throw new Error('none of the coastal inundation services Spare Construct uses is listed any more. Tell the Spare Construct maintainers');
      if (gone.length) errors.push(`Not listed by Auckland Council any more: ${gone.map(([, l]) => l).join('; ')}.`);
      let n = 0;
      // The layers are asked for together; each one fails on its own without stopping the others
      const got = await Promise.all(use.map(([nm, label]) => arcQuery(`${AC}/${encodeURIComponent(nm)}/FeatureServer/0`, { where: '1=1', outFields: 'OBJECTID', ...env(b), ...HAZ_GEOM })
        .then(r => ({ label, r }), e => { errors.push(`${label}: ${e.message}`); return null; })));
      for (const x of got) if (x) for (const f of x.r.features) if (f.geometry && f.geometry.rings) { addPoly(x.label, 'coastal', f.geometry.rings, f.attributes || {}); n++; }
      if (got.every(x => !x)) throw new Error('no coastal layer could be downloaded');
      stamp('coastal', { n, note: `Layers: ${use.filter((_, i) => got[i]).map(([nm]) => nm).join(', ')}.` });
    } catch (e) { errors.push(`Coastal inundation: ${e.message}`); failStamp('coastal', e); }
    mem.hazards = { layers: [...layers.values()], at: Date.now() };
    return { layers: mem.hazards.layers.map(l => ({ label: l.label, n: l.polys.length })), errors };
  }
  /* Edge grid for exact segment-in-polygon tests in flat metres (cell 50 m). Kept on the polygon for the same flat
     projection (compared by value, so every scenario reuses it: building it was 60% of the time on Drury). The grid
     is a flat array over the polygon's box, not a Map with float keys. */
  function prepPoly(p, Fl) {
    if (p._g && p._F && p._F.kx === Fl.kx && p._F.ky === Fl.ky) return p._g;
    const fl = Math.floor, mn = Math.min, mx = Math.max;   // local: global lookups in the hot loop are slow in the test harness
    const cs = 50, bx = [p.bb[0] * Fl.kx, p.bb[1] * Fl.ky, p.bb[2] * Fl.kx, p.bb[3] * Fl.ky];
    const i0 = fl(bx[0] / cs), j0 = fl(bx[1] / cs), ni = fl(bx[2] / cs) - i0 + 1, nj = fl(bx[3] / cs) - j0 + 1;
    const cells = new Array(ni * nj), ex = [];
    for (const r of p.rings) for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
      const x1 = r[j][0] * Fl.kx, y1 = r[j][1] * Fl.ky, x2 = r[i][0] * Fl.kx, y2 = r[i][1] * Fl.ky, e = ex.length / 4;
      ex.push(x1, y1, x2, y2);
      const a0 = mx(0, fl(mn(x1, x2) / cs) - i0), a1 = mn(ni - 1, fl(mx(x1, x2) / cs) - i0);
      const c0 = mx(0, fl(mn(y1, y2) / cs) - j0), c1 = mn(nj - 1, fl(mx(y1, y2) / cs) - j0);
      for (let a = a0; a <= a1; a++) for (let c = c0; c <= c1; c++) { const k = a * nj + c, q = cells[k]; if (q) q.push(e); else cells[k] = [e]; }
    }
    const E = Float64Array.from(ex), seen = new Uint32Array(ex.length / 4); let tick = 0;
    const cell = (i, j) => (i < i0 || j < j0 || i >= i0 + ni || j >= j0 + nj ? null : cells[(i - i0) * nj + (j - j0)]) || [];
    const g = { cs, E, cell, i0, ni, bx, next: () => (++tick === 0xFFFFFFFF ? (seen.fill(0), tick = 1) : tick), seen };
    p._g = g; p._F = { kx: Fl.kx, ky: Fl.ky };
    return g;
  }
  // Even-odd test along the shorter horizontal ray (to the left or right edge of the polygon's box)
  function pipFlat(g, x, y) {
    if (x < g.bx[0] || x > g.bx[2] || y < g.bx[1] || y > g.bx[3]) return false;
    const t = g.next(), cy = Math.floor(y / g.cs), c0 = Math.floor(x / g.cs), right = g.bx[2] - x <= x - g.bx[0];
    let inside = false;
    for (let cx = c0; right ? cx < g.i0 + g.ni : cx >= g.i0; cx += right ? 1 : -1) for (const e of g.cell(cx, cy)) {
      if (g.seen[e] === t) continue; g.seen[e] = t;
      const x1 = g.E[e * 4], y1 = g.E[e * 4 + 1], x2 = g.E[e * 4 + 2], y2 = g.E[e * 4 + 3];
      if ((y1 > y) !== (y2 > y)) { const xc = (x2 - x1) * (y - y1) / (y2 - y1) + x1; if (right ? x < xc : x > xc) inside = !inside; }
    }
    return inside;
  }
  // Parameter intervals [t0, t1] of segment A→B (flat metres) inside the polygon
  function segInside(g, ax, ay, bx, by) {
    if (Math.max(ax, bx) < g.bx[0] || Math.min(ax, bx) > g.bx[2] || Math.max(ay, by) < g.bx[1] || Math.min(ay, by) > g.bx[3]) return [];
    const ts = [0, 1], t = g.next(), dx = bx - ax, dy = by - ay;
    for (let i = Math.floor(Math.min(ax, bx) / g.cs); i <= Math.floor(Math.max(ax, bx) / g.cs); i++) for (let j = Math.floor(Math.min(ay, by) / g.cs); j <= Math.floor(Math.max(ay, by) / g.cs); j++) for (const e of g.cell(i, j)) {
      if (g.seen[e] === t) continue; g.seen[e] = t;
      const x1 = g.E[e * 4], y1 = g.E[e * 4 + 1], ex = g.E[e * 4 + 2] - x1, ey = g.E[e * 4 + 3] - y1, den = dx * ey - dy * ex;
      if (!den) continue;
      const s = ((x1 - ax) * ey - (y1 - ay) * ex) / den, u = ((x1 - ax) * dy - (y1 - ay) * dx) / den;
      if (s > 0 && s < 1 && u >= 0 && u <= 1) ts.push(s);
    }
    ts.sort((p, q) => p - q);
    const out = [];
    for (let k = 0; k < ts.length - 1; k++) { if (ts[k + 1] - ts[k] < 1e-9) continue; const m = (ts[k] + ts[k + 1]) / 2; if (pipFlat(g, ax + dx * m, ay + dy * m)) out.push([ts[k], ts[k + 1]]); }
    return out;
  }
  const mergeLen = iv => { iv.sort((p, q) => p[0] - q[0]); let s = 0, c = null; for (const [a0, a1] of iv) { if (!c || a0 > c[1]) { if (c) s += c[1] - c[0]; c = [a0, a1]; } else c[1] = Math.max(c[1], a1); } if (c) s += c[1] - c[0]; return s; };
  /* Length (m) of each link inside each hazard layer, plus 'only access': the link is a connector road whose removal
     cuts a proposal (development) zone off from every gateway. Rows [{ link, name, fc, layer, len, only }]. */
  function hazardLinks(sc, data = mem.hazards) {
    if (!data) return [];
    // One flat projection per hazard download, so the polygon grids are built once for every scenario
    const lat0 = data.lat0 ?? (data.lat0 = (Object.values(sc.nodes)[0] || { lat: -36.85 }).lat), Fl = U.flat(lat0), rows = [];
    for (const L of liveLinks(sc)) {
      const g = Net.geom(sc, L), xs = g.map(p => p[1] * Fl.kx), ys = g.map(p => p[0] * Fl.ky);
      const bb = [Math.min(...g.map(p => p[1])), Math.min(...g.map(p => p[0])), Math.max(...g.map(p => p[1])), Math.max(...g.map(p => p[0]))];
      for (const lay of data.layers) {
        let len = 0;
        for (let i = 0; i < g.length - 1; i++) {
          const iv = [];
          for (const p of lay.polys) { if (bb[2] < p.bb[0] || bb[0] > p.bb[2] || bb[3] < p.bb[1] || bb[1] > p.bb[3]) continue; iv.push(...segInside(prepPoly(p, Fl), xs[i], ys[i], xs[i + 1], ys[i + 1])); }
          if (iv.length) len += mergeLen(iv) * Math.hypot(xs[i + 1] - xs[i], ys[i + 1] - ys[i]);
        }
        if (len >= 0.5) rows.push({ link: L.id, name: L.name || 'Unnamed road', cls: L.cls, fc: L.fc && L.fc.gt ? L.fc.gt : `${(CLASSES[L.cls] || CLASSES.local).label} (road class)`, layer: lay.label, len: U.r1(len), only: false });
      }
    }
    // only access: undirected reachability from each development zone's connector nodes to any gateway node
    const dev = Object.values(sc.zones).filter(z => z.kind === 'zone' && !existingZone(z)), gws = Object.values(sc.zones).filter(z => z.kind === 'gateway');
    if (rows.length && dev.length && gws.length) {
      const cm = Engine.connectorMap(sc), adj = new Map();
      for (const L of liveLinks(sc)) for (const [x, y] of [[L.a, L.b], [L.b, L.a]]) { const q = adj.get(x); if (q) q.push([y, L.id]); else adj.set(x, [[y, L.id]]); }
      const gwN = new Set(gws.flatMap(z => cm[z.id] || []));
      const reach = (from, skip) => { const seen = new Set(from), st = [...from]; while (st.length) { const x = st.pop(); if (gwN.has(x)) return true; for (const [y, id] of adj.get(x) || []) if (id !== skip && !seen.has(y)) { seen.add(y); st.push(y); } } return false; };
      const only = new Map();
      for (const id of new Set(rows.map(r => r.link))) only.set(id, dev.some(z => { const c = cm[z.id] || []; return c.length && reach(c, null) && !reach(c, id); }));
      for (const r of rows) r.only = !!only.get(r.link);
    }
    return rows.sort((p, q) => q.len - p.len);
  }
  // Table for report section 4.5 (climate adaptation): Link | FC role | Length in hazard (m) | Hazard layer | Only access
  function hazardTable(sc = Store.sc, rows = (F().hazards || {})[sc.id] && F().hazards[sc.id].rows) {
    rows = rows || [];
    return Kit.table({ head: ['Link', 'Future Connect role', { label: 'Length in hazard (m)', n: true }, 'Hazard layer', 'Only access'],
      rows: rows.map(r => ({ cells: [esc(r.name), esc(r.fc), U.fmt.int(r.len), esc(r.layer), r.only ? '<b>Yes</b>' : 'No'], fly: 'link:' + r.link })),
      empty: 'No modelled road lies in a mapped flood plain or coastal inundation area.' });
  }

  /* =====================================================================================
     8. Fleet mix (Motor Vehicle Register)
     ===================================================================================== */
  const MON = { Jan: 1, Feb: 2, Mar: 3, Apr: 4, May: 5, Jun: 6, Jul: 7, Aug: 8, Sep: 9, Oct: 10, Nov: 11, Dec: 12 };
  async function loadFleet(district = 'AUCKLAND', { now = Date.now() } = {}) {
    const D = String(district || '').trim().toUpperCase().replace(/'/g, '');
    if (!D) throw new Error('Type a district name as the register spells it, such as AUCKLAND or HAMILTON CITY.');
    const ck = 'corridor.feeds.fleet.' + D;
    try {
      const c = JSON.parse(localStorage.getItem(ck) || 'null');
      if (c && now - Date.parse(c.at) < RULES.fleetDays * D2) {
        // Kept from an earlier query (perhaps another project): this project's data-vintage table still names it
        F().fleet = c;
        stamp('mvr', { url: c.url, n: c.total, at: c.at, note: `${c.service}: passenger cars and vans registered in ${c.district}, by motive power. Kept from the query of ${dateText(c.at)} (results are kept for ${RULES.fleetDays} days).` });
        return { ...c, cached: true };
      }
    } catch { }
    const list = await get(NZTA + '?f=json');
    const mvr = (list.services || []).map(s => s.name.replace(/^.*\//, '')).map(n => { const m = /^MVR_([A-Z][a-z]{2})(\d{2})$/.exec(n); return m && MON[m[1]] ? { n, k: (2000 + +m[2]) * 12 + MON[m[1]] } : null; }).filter(Boolean).sort((p, q) => q.k - p.k)[0];
    if (!mvr) throw new Error('The NZTA services list has no Motor Vehicle Register layer (MVR_…) at the moment. Try again later.');
    const url = `${NZTA}/${mvr.n}/FeatureServer/0`, meta = await layerMeta(url);
    const [tla, vt, mp] = ['TLA', 'VEHICLE_TYPE', 'MOTIVE_POWER'].map(w => resolve(meta, [w])[0] || w);
    const r = await arcQuery(url, { where: `${tla}='${D}' AND ${vt}='PASSENGER CAR/VAN'`, groupByFieldsForStatistics: mp, outStatistics: JSON.stringify([{ statisticType: 'count', onStatisticField: resolve(meta, ['OBJECTID'])[0] || 'OBJECTID', outStatisticFieldName: 'n' }]) }, { maxPages: 2, timeout: Math.max(limitMs(), RULES.fleetTimeout * 1000) });
    const counts = {}; let total = 0;
    for (const f of r.features) { const A = f.attributes || {}, k = A[mp] ?? A[mp.toLowerCase()] ?? 'UNKNOWN', n = +A.n || +A.N || 0; counts[k] = n; total += n; }
    if (!total) throw new Error(`The register has no passenger cars for '${D}'. Check the spelling (for example AUCKLAND, HAMILTON CITY, CHRISTCHURCH CITY).`);
    const out = { district: D, service: mvr.n, url, at: new Date(now).toISOString(), total, shares: Object.fromEntries(Object.entries(counts).sort((p, q) => q[1] - p[1]).map(([k, n]) => [k, n / total])) };
    F().fleet = out;
    try { localStorage.setItem(ck, JSON.stringify(out)); } catch { }
    stamp('mvr', { url, n: total, note: `${mvr.n}: passenger cars and vans registered in ${D}, by motive power.` });
    return out;
  }

  /* =====================================================================================
     UI: the Data sources section, review lists, inspector sections, map modes and layers
     ===================================================================================== */
  const tag = t => `<span class="pill">${t}</span>`;
  const card = (title, tags, body, actions) => `<div class="src"><h5>${title}</h5><div class="chips">${tags}</div><div class="note">${body}</div><div class="actions">${actions}</div></div>`;
  const btn = (act, label, extra = '') => `<button class="btn small" data-feeds="${act}" ${extra}>${label}</button>`;
  const when = id => { const v = F().vintage[id]; return !v ? 'Not loaded yet.' : v.error ? `<span style="color:var(--danger)">Last try failed: ${esc(v.error)}.</span>` : `<b>${U.fmt.int(v.n || 0)}</b> loaded ${esc(dateText(v.at))}.`; };
  function vintageRows() {
    const V = F().vintage;
    return Object.entries(V).sort((p, q) => String(p[1].source).localeCompare(String(q[1].source))).map(([, v]) => [
      esc(v.source || ''), a(v.page || v.url, hostOf(v.url || v.page || '')), esc(dateText(v.at)), esc(v.licence || ''), v.error ? '–' : U.fmt.int(v.n || 0), v.error ? `<span style="color:var(--danger)">${esc(v.error)}</span>` : esc(v.note || '')]);
  }
  function vintageHtml() {
    return Kit.table({ head: ['Data', 'Source', 'Queried', 'Licence', { label: 'Records', n: true }, 'Notes'], rows: vintageRows(), empty: 'Nothing fetched yet. Each source above records its query date, licence and record count here.' });
  }
  // The section appended to the Traffic data tab (HTML only; handlers in onClick / onChange)
  function sectionHtml() {
    const sc = Store.sc, P = Store.P, f = F(), counts = tmsSites(), tms = f.tms || {}, gt = f.gtfs, hz = (f.hazards || {})[sc.id];
    const ns = f.nslr && f.nslr.scenarios && (f.nslr.scenarios.find(s => s.id === sc.id) || f.nslr.scenarios[0]);
    const cards = [
      card('Legal speed limits · NZTA register', tag('real') + tag('free, no key') + tag('CC BY 4.0'),
        `Current speed limits from the National Speed Limit Register (past versions are left out). A future scenario year uses the limits gazetted for 1 July of that year. ${ns ? `${U.fmt.pct(ns.nslr / Math.max(1, ns.links))} of roads in ${esc(sc.name)} have a legal limit${ns.variable ? `, ${esc(ns.variable)} with a variable (school) limit` : ''}.` : ''} ${when('nslr')}`,
        btn('nslr', 'Load speed limits') + btn('speeds', 'Review modelled speeds…', ns ? '' : 'disabled')),
      card('Junction controls · AT', tag('real') + tag('free, no key') + tag('Auckland only'),
        `AT signal sites and stop, give-way and roundabout signs suggest each junction's control and AT site number. You review the list before anything changes. ${when('controlled')}`,
        btn('control', 'Check junction controls…')),
      card('Turn bans · OpenStreetMap', tag('real') + tag('free, no key') + tag('ODbL'),
        `OSM turn restrictions (no right turn, left turn only …) at modelled junctions become banned movements. You review the list before anything changes. ${when('osm')}`,
        btn('bans', 'Check turn bans…')),
      card('Daily counts · NZTA TMS', tag('real counts') + tag('free, no key') + tag('CC BY 4.0'),
        `Daily traffic at state highway telemetry sites, cleaned of duplicate rows: Tuesday–Thursday school-term means with a 95% confidence interval, %HCV, day-of-week and monthly factors. ${counts.length ? `${counts.length} NZTA count site${counts.length === 1 ? '' : 's'} can use it; ${Object.keys(tms).length} done.` : 'Load NZTA counts first.'} ${when('tms')}`,
        btn('tms', 'Load daily counts', counts.length ? '' : 'disabled') + btn('profiles', 'Load peak profiles')),
      card('Growth · AGS23 v1.1', tag('real forecast') + tag('Auckland only') + tag('fetched live'),
        `Auckland Council's growth scenario by MSM zone: household, job and population growth from ${esc(P.settings.baseYear)} to each scenario year, applied as factors to existing-land-use zones, and growth beyond each gateway. Never stored in the project. ${when('ags')}`,
        btn('ags', 'Get growth factors…')),
      card('Bus timetables · GTFS', tag('real timetables') + tag('free, no key') + tag("publisher's licence"),
        `Bus routes and AM and PM frequencies for a normal Tuesday, from the Mobility Database copy of the operator's feed. Buses load the roads and junctions they use. ${gt ? `${esc(gt.routes)} route patterns, ${esc((gt.stops || []).length)} stops from ${esc(gt.publisher || gt.feed.provider)}.` : ''} ${when('gtfs')}`,
        btn('gtfs', 'Find bus timetable…') + (gt ? btn('stops', 'Stops and routes serving') : '')),
      card('Council counts · ArcGIS', tag('real counts') + tag('other councils'),
        `Counts from a council's ArcGIS layer. Pick Hamilton or Queenstown Lakes, or paste any layer address; you confirm which fields hold the volumes.`,
        `<select data-feeds-in="preset" aria-label="Council"><option value="hamilton">Hamilton City</option><option value="qldc">Queenstown Lakes</option><option value="url">Other layer (address)…</option></select><input data-feeds-in="url" type="url" placeholder="https://…/FeatureServer/0" style="flex:1;min-width:150px" aria-label="Layer address">${btn('council', 'Read layer…')}`),
      card('Flood and coastal hazards · Auckland Council', tag('real') + tag('Auckland only'),
        `Flood plains and coastal inundation areas, and the length of each road inside them, for the climate adaptation table (ITA section 4.5). ${hz ? `${new Set(hz.rows.map(r => r.link)).size} roads affected${hz.rows.some(r => r.only) ? `, ${new Set(hz.rows.filter(r => r.only).map(r => r.link)).size} the only access to a proposal zone` : ''}.` : ''} ${when('flood')}`,
        btn('hazards', 'Check hazards') + (hz ? btn('hazardtable', 'Show table') : '')),
      card('Fleet mix · Motor Vehicle Register', tag('real') + tag('free, no key'),
        `Passenger cars by motive power for a district. ${f.fleet ? `${esc(f.fleet.district)}: ${Object.entries(f.fleet.shares).slice(0, 4).map(([k, v]) => `${esc(k.toLowerCase())} ${U.fmt.pct(v)}`).join(', ')}.` : ''} The register query takes about 20 s; results are kept for ${RULES.fleetDays} days. ${when('mvr')}`,
        `<input data-feeds-in="district" value="${esc((f.fleet && f.fleet.district) || 'AUCKLAND')}" style="width:150px" aria-label="District">${btn('fleet', 'Load fleet mix')}${f.fleet ? (Store.S.co2FactorSrc ? btn('fleetco2off', 'Stop using it for CO₂') : btn('fleetco2', 'Use for CO₂')) : ''}${Store.S.co2FactorSrc ? `<p class="note">CO₂: light vehicles × ${esc(U.fmt.num(+Store.S.co2Factor, 3))} (${esc(Store.S.co2FactorSrc)}).</p>` : ''}`),
    ];
    return `<div class="feeds-sec" style="margin-top:16px"><div class="bar-h"><h3>Data sources</h3><span class="note">Official feeds for the study area. Each one records its source, query date and licence below.</span></div>
      <div class="src-grid">${cards.join('')}</div>
      <div class="lg-title" style="margin-top:12px">Data vintage</div>${vintageHtml()}</div>`;
  }
  function reviewHtml(intro, head, rows) {
    return `<p>${intro}</p><div class="tbl-wrap" style="max-height:52vh"><table class="t compact" id="feedsReview"><tr><th><input type="checkbox" data-feeds-all aria-label="Select all"></th>${head.map(h => `<th>${h}</th>`).join('')}</tr>${rows.map(r => `<tr><td><input type="checkbox" value="${esc(r.id)}" ${r.checked ? 'checked' : ''} aria-label="Apply"></td>${r.cells.map(c => `<td>${c}</td>`).join('')}</tr>`).join('')}</table></div>`;
  }
  function review(title, intro, head, rows, applyLabel, extra = '') {
    const p = Kit.modal(esc(title), reviewHtml(intro, head, rows) + extra, [['Cancel', null, ''], [applyLabel, () => ({ ids: U.$$('#feedsReview input[value]').filter(i => i.checked).map(i => i.value), all: !!(U.$('#feedsAllSc') || {}).checked })]], true);
    setTimeout(() => { const al = U.$('#feedsReview [data-feeds-all]'); if (al) al.onchange = () => U.$$('#feedsReview input[value]').forEach(i => { i.checked = al.checked; }); }, 0);
    return p;
  }
  const otherScBox = label => Store.P.scenarios.length > 1 ? `<label class="check" style="margin-top:8px"><input type="checkbox" id="feedsAllSc" ${cfg().otherScenarios ? 'checked' : ''}> ${label}</label>` : '';
  const CTL_TEXT = k => (CONTROLS[k] || k || 'Uncontrolled');

  /* Undo snapshots for every scenario a change touches (Store.snapshot covers the active one only): each Undo then
     restores one scenario. The active scenario goes last, so the first Undo restores what is on screen. */
  function snapEach(scs, label) {
    const P = Store.P, act0 = P.active, list = [...new Set(scs)].filter(Boolean).sort((x, y) => (x.id === act0) - (y.id === act0));
    try { for (const s of list) { P.active = s.id; Kit.snap(label); } } finally { P.active = act0; }
    return list.length;
  }
  async function run(label, fn) {
    Kit.busy(label);
    try { return await fn(t => Kit.busy(t)); }
    catch (e) { console.error(e); Kit.toast(e.message || String(e), 'err'); Kit.log('Data sources: ' + esc(e.message || e)); return null; }
    finally { Kit.busy(null); }
  }
  const area = () => DataHub.area(UI.map);
  async function act(name, el) {
    const sc = Store.sc, P = Store.P;
    if (name === 'nslr') {
      const r = await run('Downloading speed limits…', st => nslrAll(area(), P.scenarios, st).catch(e => { failStamp('nslr', e); throw e; }));
      Store.emit('data');
      if (!r) return;
      const me = r.scenarios.find(s => s.id === sc.id) || r.scenarios[0];
      Kit.toast(`Legal speed limits on ${U.fmt.int(me.nslr)} of ${U.fmt.int(me.links)} roads (${U.fmt.pct(me.share)}) in ${me.name}.${r.drop.ended ? ` ${r.drop.ended} ended versions left out.` : ''}${me.share < 0.8 ? ' The register has no current version for the other roads, so they keep their OSM or class-default speed.' : ''}`, me.share < 0.8 ? 'warn' : '');
      Kit.log(`Speed limits: ${r.scenarios.map(s => `${esc(s.name)} ${s.nslr}/${s.links}${s.year ? ' (as at 1 July ' + s.year + ')' : ''}`).join(', ')}. ${esc(SRC.nslr.attribution)}.`);
      if (speedProposals(sc).length) return act('speeds');
      return;
    }
    if (name === 'speeds') {
      const rows = speedProposals(sc);
      if (!rows.length) return Kit.toast('Every road with a legal limit already uses it as its modelled speed.');
      const v = await review('Use legal speed limits as modelled speeds', `${rows.length} existing road${rows.length === 1 ? '' : 's'} in <b>${esc(sc.name)}</b> have a modelled speed that differs from the legal limit. Ticked roads take the legal limit as their free-flow speed. Roads whose speed you changed yourself are unticked. Undo puts them back.`,
        ['Road', 'Class', 'Modelled now', 'Legal limit', 'Note'],
        rows.map(r => ({ id: r.id, checked: !r.user, cells: [esc(r.name), esc((CLASSES[r.cls] || {}).label || r.cls), `${esc(r.from)} km/h`, `<b>${esc(r.to)} km/h</b>`, r.user ? 'You changed this speed' : r.variable ? 'Variable (school) limit too' : ''] })),
        'Apply ticked roads', otherScBox('Also apply to the same roads in the other scenarios (Undo restores one scenario at a time)'));
      if (!v || !v.ids.length) return;
      // Undo steps only for scenarios that change (another scenario may already use the limits, or have your own speeds)
      const changes = s => s === sc || v.ids.some(id => { const L = s.links[id]; return L && L.speedLimit > 0 && !userSpeed(L) && L.speed !== L.speedLimit; });
      snapEach(v.all ? P.scenarios.filter(changes) : [sc], 'legal speed limits');
      const n = applySpeeds(sc, v.ids);
      if (v.all) for (const s of P.scenarios) if (s !== sc) { applySpeeds(s, v.ids.filter(id => s.links[id] && s.links[id].speedLimit > 0 && !userSpeed(s.links[id]))); for (const r of Object.values(s.res || {})) if (r) r.stale = true; }
      Store.edited('net');
      return Kit.toast(`${n} road${n === 1 ? '' : 's'} now use the legal speed limit. Run the model again.`);
    }
    if (name === 'control') {
      const data = await run('Downloading AT signal sites and signs…', () => loadControl(area()).catch(e => { failStamp('controlled', e); throw e; }));
      Store.emit('data');
      if (!data) return;
      const { rows, unmatched } = controlProposals(sc, data);
      if (!rows.length) return Kit.toast(`AT data agrees with every junction control here (${data.sites.length} signal sites, ${data.signs.length} signs).${unmatched.length ? ` ${unmatched.length} AT sites have no junction within ${RULES.control.siteM} m.` : ''}`);
      const v = await review('Junction controls from AT data', `AT lists ${data.sites.length} signal sites and ${data.signs.length} stop, give-way and roundabout signs here. Ticked junctions take the control shown. Junctions you changed yourself are unticked. ${esc(SRC.controlled.note)}${unmatched.length ? ` ${unmatched.length} AT site${unmatched.length === 1 ? ' has' : 's have'} no modelled junction within ${RULES.control.siteM} m.` : ''}`,
        ['Junction', 'Now', 'From AT', 'Why', 'AT site'],
        rows.map(r => ({ id: r.id, checked: !r.user, cells: [esc(r.name), esc(CTL_TEXT(r.from)), r.change ? `<b>${esc(CTL_TEXT(r.to))}</b>` : 'same (adds the source)', esc(r.why) + (r.user ? ' · <i>you changed this control</i>' : ''), r.atSiteNo != null ? esc(r.atSiteNo) : ''] })),
        'Apply ticked junctions');
      if (!v || !v.ids.length) return;
      Store.snapshot();
      const n = applyControls(sc, rows, v.ids);
      Store.edited('net');
      return Kit.toast(`${n} junction control${n === 1 ? '' : 's'} set from AT data${v.ids.length > n ? `, ${v.ids.length - n} given their AT source` : ''}. Run the model again.`);
    }
    if (name === 'bans') {
      const data = await run('Downloading OpenStreetMap turn restrictions…', () => loadRestrictions(area()).catch(e => { failStamp('osm', e); throw e; }));
      Store.emit('data');
      if (!data) return;
      const { rows, skipped } = banProposals(sc, data);
      const sk = skipped.length ? ` ${skipped.length} restriction${skipped.length === 1 ? ' was' : 's were'} left out (only for some vehicles or times, via a road, or not at a modelled junction).` : '';
      if (!rows.length) return Kit.toast(`No new turn bans: OpenStreetMap has ${data.rels.length} turn restriction${data.rels.length === 1 ? '' : 's'} here and the model already has every one it can use.${sk}`);
      const v = await review('Turn bans from OpenStreetMap', `Ticked movements are banned in <b>${esc(sc.name)}</b>. Bans you removed yourself are unticked. Undo puts them back.${esc(sk)}`,
        ['Junction', 'Restriction', 'Movements', 'Source'],
        rows.map(r => ({ id: r.id, checked: !r.user, cells: [esc(r.name), esc(r.why) + (r.user ? ' · <i>you removed this ban</i>' : ''), esc(r.bans.length), a(`https://www.openstreetmap.org/relation/${encodeURIComponent(r.rel)}`, r.src)] })),
        'Ban ticked movements');
      if (!v || !v.ids.length) return;
      snapEach([sc], 'turn bans from OpenStreetMap');
      const n = applyBans(sc, rows, v.ids);
      Store.edited('net');
      return Kit.toast(`${n} movement${n === 1 ? '' : 's'} banned from OpenStreetMap turn restrictions. Run the model again.`);
    }
    if (name === 'tms') {
      const r = await run('Downloading daily counts…', st => loadTMS(tmsSites(), { status: st }).catch(e => { failStamp('tms', e); throw e; }));
      Store.emit('data');
      if (r) Kit.toast(`Daily counts for ${r.sites} of ${r.of} NZTA sites since ${dateText(r.since)}.${r.complete ? '' : ' Some sites hit the download cap.'}`, r.sites ? '' : 'warn');
      return;
    }
    if (name === 'profiles') {
      const n = await run('Loading peak profiles…', () => loadProfiles().catch(e => { throw new Error(`Peak profiles are not installed on this site (${e.message}). Peak hours use the shares in Settings › Traffic data.`); }));
      Store.emit('data');
      if (n != null) Kit.toast(`Peak profiles hold the AM and PM shares of ${n} count site${n === 1 ? '' : 's'}.${periodUsesShare() ? '' : ' The model still uses the shares in Settings › Traffic data until it reads them.'}`);
      return;
    }
    if (name === 'ags') {
      const S = P.settings, years = P.scenarios.map(s => Engine.yearOf(s, S));
      const data = await run('Downloading AGS23 growth for this area…', () => loadAGS(area(), years, S.baseYear).catch(e => { failStamp('ags', e); throw e; }));
      if (!data) return Store.emit('data');
      const gw = agsGateways(P, data);
      F().ags = { at: new Date().toISOString(), zones: data.zones.length, base: S.baseYear, gateways: gw.length };
      const rows = agsProposals(P, data);
      Store.emit('data');
      if (!rows.length) return Kit.toast(`AGS23 loaded (${data.zones.length} MSM zones). ${gw.length ? `Growth beyond ${gw.length} gateways is in each gateway's panel.` : ''} No existing-land-use zones in a forecast-year scenario to grow.`);
      const v = await review('Grow existing land use with AGS23', `Existing-land-use zones in forecast-year scenarios take the AGS23 growth of the MSM zone they sit in, from ${S.baseYear}: houses by households, jobs by employment, community uses by population (Spare Construct assumption). ${agsBasisText}. Zones outside Auckland get no factor.`,
        ['Scenario', 'Zone', 'MSM zone', 'Households ×', 'Jobs ×', 'Population ×', 'Change'],
        rows.map(r => ({ id: r.id, checked: !!r.f && !r.user, cells: [esc(r.scName) + ' ' + esc(r.year), esc(r.name) + (r.clamped ? ` <span class="note">${esc(r.clamped)}</span>` : ''), r.f ? esc(r.f.msm) : '<i>outside AGS23</i>', r.f && r.f.hh != null ? r.f.hh.toFixed(3) : '–', r.f && r.f.emp != null ? r.f.emp.toFixed(3) : '–', r.f && r.f.pop != null ? r.f.pop.toFixed(3) : '–',
          Object.entries(r.lu).map(([k, [x, y]]) => `${esc((LANDUSES[k] || {}).label || k)} ${U.fmt.num(x, 0)} → ${U.fmt.num(y, 0)}`).join('; ') + (r.user ? ' · <i>you changed this zone</i>' : '')] })),
        'Apply ticked zones');
      if (!v || !v.ids.length) return;
      // Forecast scenarios other than this one change too: each gets its own undo step
      const touched = P.scenarios.filter(s => rows.some(r => r.sc === s.id && r.f && v.ids.includes(r.id)));
      const steps = snapEach(touched, 'AGS23 growth');
      const n = applyAGS(P, rows, v.ids);
      for (const s of P.scenarios) for (const r of Object.values(s.res || {})) if (r) r.stale = true;
      Store.edited('demand');
      return Kit.toast(`${n} zone${n === 1 ? '' : 's'} grown with AGS23 v1.1${steps > 1 ? ` in ${steps} scenarios (Undo restores one scenario at a time)` : ''}. Run the model again.`);
    }
    if (name === 'agsrate') {
      const gws = Object.values(sc.zones).filter(z => z.kind === 'gateway' && z.agsGrowth && z.agsGrowth.pct != null);
      if (!gws.length) return Kit.toast('Get AGS23 growth factors first, in a forecast-year scenario.', 'warn');
      const pct = Math.round(mean(gws.map(z => z.agsGrowth.pct)) * 100) / 100;
      if (!(await Kit.modal('Use AGS23 gateway growth', `<p>Set the background growth rate to <b>${pct.toFixed(2)}% a year</b> (linear), the average AGS23 growth beyond ${gws.length} gateway${gws.length === 1 ? '' : 's'} in ${esc(sc.name)}? It applies to every scenario. It is ${(+P.settings.growth || 0).toFixed(2)}% now.</p>`, [['Cancel', false, ''], ['Set growth rate', true]]))) return;
      // A project setting: undo-able, and every scenario's results go out of date (Store.edited marks this one only)
      Kit.snapSettings('background growth from AGS23');
      P.settings.growth = pct;
      for (const s of P.scenarios) for (const r of Object.values(s.res || {})) if (r) r.stale = true;
      Store.edited('settings');
      return Kit.toast(`Background growth is now ${pct.toFixed(2)}% a year. Run the model again.`);
    }
    if (name === 'gtfs') {
      const b = area();
      const cat = await run('Finding bus timetables for this area…', () => catalogue(b));
      if (!cat) return;
      if (!cat.feeds.length) return Kit.toast(`No bus timetable covers this area${cat.error ? ` (the feed list could not be read: ${cat.error})` : ''}.`, 'warn');
      let feed = cat.feeds[0];
      if (cat.feeds.length > 1) {
        const pickId = await Kit.modal('Choose a bus timetable', `<p>These feeds cover the study area${cat.from !== 'catalogue' ? ' (from Spare Construct’s built-in list: the Mobility Database list could not be read)' : ''}.</p>${cat.feeds.slice(0, 8).map((f, i) => `<label class="check"><input type="radio" name="feedsFeed" value="${esc(f.id)}" ${i ? '' : 'checked'}> ${esc(f.provider)}${f.name ? ' · ' + esc(f.name) : ''} <span class="note">${esc(f.id)}</span></label>`).join('')}`,
          [['Cancel', null, ''], ['Download', () => (U.$$('input[name=feedsFeed]').find(i => i.checked) || {}).value]]);
        if (!pickId) return;
        feed = cat.feeds.find(f => f.id === pickId) || feed;
      }
      const g = await run('Downloading the bus timetable…', st => loadGTFS(feed, b, { status: st }).catch(e => { failStamp('gtfs', e, { key: 'gtfs' }); throw e; }));
      if (!g) return Store.emit('data');
      const routes = gtfsRoutes(sc, g);
      keepGTFS(g, routes);
      Store.emit('data');
      if (!routes.length) return Kit.toast(`The timetable has ${g.stops.length} stops here but no route runs between two of them in the peaks.`, 'warn');
      const v = await review('Bus routes from the timetable', `${esc(F().gtfs.publisher || feed.provider)}, ${g.date.slice(6, 8)}/${g.date.slice(4, 6)}/${g.date.slice(0, 4)} (a Tuesday in service, not a public holiday${g.term ? ', in school term' : ''})${g.stale ? ', <b>the feed may be out of date</b>' : g.term === false ? ', <b>not in a school term</b> (the feed has no term-time Tuesday)' : ''}. Buses per hour from departures between ${RULES.periods.AM.join(':00 and ')}:00 and ${RULES.periods.PM.join(':00 and ')}:00. The model uses one frequency for both peaks (the busier one) until it reads each peak separately. Earlier timetable routes are replaced.`,
        ['Route', 'AM bus/h', 'PM bus/h', 'Stops in area'],
        routes.map(r => ({ id: r.id, checked: true, cells: [esc(r.name), U.fmt.num(r.freqs.AM, 1), U.fmt.num(r.freqs.PM, 1), String(r.gtfs.stops)] })),
        'Add ticked routes', otherScBox('Also add them to the other scenarios (Undo restores one scenario at a time)'));
      if (!v || !v.ids.length) return;
      const pick = routes.filter(r => v.ids.includes(r.id));
      snapEach(v.all ? P.scenarios : [sc], 'bus routes from the timetable');
      applyGTFS(sc, pick);
      if (v.all) for (const s of P.scenarios) if (s !== sc) { applyGTFS(s, gtfsRoutes(s, g).filter(r => v.ids.includes(r.id))); for (const r of Object.values(s.res || {})) if (r) r.stale = true; }
      F().gtfs.routes = pick.length;
      Store.edited('net');
      return Kit.toast(`${pick.length} bus route${pick.length === 1 ? '' : 's'} added. Run the model to load the buses onto the roads.`);
    }
    if (name === 'stops') return Kit.modal('Bus stops and routes serving', stopsHtml(), [['Close', null]], true);
    if (name === 'council') {
      const body = el.closest('.src'), preset = (body.querySelector('[data-feeds-in=preset]') || {}).value || 'hamilton', typed = (body.querySelector('[data-feeds-in=url]') || {}).value || '';
      const url = preset === 'url' ? typed : COUNCILS[preset].url;
      const read = await run('Reading the layer…', () => councilRead(url));
      if (!read) return;
      const m = read.map, opts = ['', ...read.fields], sel = (k, label) => `<label class="fld"><span>${label}</span><span class="fld-in"><select data-feeds-map="${k}">${opts.map(f => `<option value="${esc(f)}" ${f === m[k] ? 'selected' : ''}>${f ? esc(f) : '(none)'}</option>`).join('')}</select></span></label>`;
      const map = await Kit.modal(`Fields in ${esc(read.name)}`, `<p>Spare Construct guessed which fields hold each value. Check them, then import.${m.years.length ? ` Year columns (${esc(m.years.slice(0, 3).join(', '))}…) are read newest first when there is no daily volume field.` : ''}</p>
        <div class="set-grid">${sel('adt', 'Daily volume (veh/day)')}${sel('peak', 'Peak-hour volume (veh/h)')}${sel('peakTime', 'Peak-hour time')}${sel('hcv', 'Heavy vehicles (%)')}${sel('date', 'Count date')}${sel('name', 'Site name')}${sel('dir', 'Direction')}${sel('latest', 'Latest-count flag')}</div>`,
        [['Cancel', null, ''], ['Import counts', () => { const o = { years: m.years }; U.$$('[data-feeds-map]').forEach(s => { o[s.dataset.feedsMap] = s.value; }); return o; }]], true);
      if (!map) return;
      if (!map.adt && !map.years.length) return Kit.toast('Pick the field that holds the daily volume.', 'warn');
      const r = await run('Importing council counts…', st => councilImport(read, map, area(), { preset: preset === 'url' ? null : preset, status: st }).catch(e => { failStamp('council', e, { key: 'council:' + hashStr(read.url), url: read.url, source: read.name }); throw e; }));
      View.show.counts = true; Store.emit('data');
      if (r) Kit.toast(`${r.n} council count${r.n === 1 ? '' : 's'} imported${r.of > r.n ? ` (${r.of - r.n} without a usable volume or too old)` : ''}. They are checked with your own counts.`, r.n ? '' : 'warn');
      return;
    }
    if (name === 'hazards') {
      const r = await run('Checking flood and coastal hazards…', st => loadHazards(area(), { status: st }));
      if (!r) return;
      const H = F().hazards ||= {};
      for (const s of P.scenarios) H[s.id] = { at: new Date().toISOString(), layers: r.layers, rows: hazardLinks(s) };
      View.show['feeds-hazards'] = true;
      Store.emit('data');
      const rows = H[sc.id].rows;
      Kit.toast(`${new Set(rows.map(x => x.link)).size} roads in ${sc.name} cross a mapped hazard.${r.errors.length ? ' ' + r.errors.join(' ') : ''}`, r.errors.length ? 'warn' : '');
      return;
    }
    if (name === 'hazardtable') return Kit.modal('Roads in flood plains and coastal inundation areas', `<p>For ITA section 4.5 (climate adaptation). Length is the part of each road inside the hazard area, from the road's centreline. 'Only access' means removing the road cuts a proposal zone off from every gateway.</p>${hazardTable(sc)}`, [['Close', null]], true);
    if (name === 'fleetco2' || name === 'fleetco2off') {
      const S = Store.S, fl = F().fleet;
      if (name === 'fleetco2off') { delete S.co2Factor; delete S.co2FactorSrc; Store.edited('settings'); Kit.toast('CO₂ uses Spare Construct\'s generic fleet again. Run the model again.'); return; }
      if (!fl || !fl.shares) return Kit.toast('Load the fleet mix first.', 'warn');
      const bev = Object.entries(fl.shares).filter(([k]) => /^ELECTRIC$/i.test(String(k).trim())).reduce((a, [, v]) => a + (+v || 0), 0), k = Math.round(U.clamp(1 - bev, 0, 1) * 1000) / 1000;
      S.co2Factor = k;
      S.co2FactorSrc = `${fl.district} fleet, ${fl.service}: ${U.fmt.pct(bev, 1)} battery electric (tailpipe CO₂ only; hybrids counted as fuel-burning, Spare Construct assumption)`;
      Store.edited('settings');
      Kit.toast(`CO₂ now uses the ${fl.district} fleet: light vehicles × ${U.fmt.num(k, 3)}. Run the model again.`);
      return;
    }
    if (name === 'fleet') {
      const district = (el.closest('.src').querySelector('[data-feeds-in=district]') || {}).value;
      const r = await run('Asking the Motor Vehicle Register (about 20 s)…', () => loadFleet(district).catch(e => { failStamp('mvr', e); throw e; }));
      Store.emit('data');
      if (r) Kit.toast(`${r.district}: ${U.fmt.int(r.total)} cars and vans${r.cached ? ' (kept from an earlier query)' : ''}.`);
      return;
    }
  }
  function stopsHtml() {
    const g = F().gtfs; if (!g) return '<p class="note">Find a bus timetable first.</p>';
    return `<p class="note">${esc(g.publisher || g.feed.provider)}, service day ${esc(g.date.slice(6, 8))}/${esc(g.date.slice(4, 6))}/${esc(g.date.slice(0, 4))}. Departures per hour in the AM and PM windows.</p>` +
      Kit.table({ head: ['Stop', 'Routes serving', { label: 'AM bus/h', n: true }, { label: 'PM bus/h', n: true }],
        rows: g.stops.slice().sort((p, q) => (q.am + q.pm) - (p.am + p.pm)).map(s => [esc(s.name) + (s.code ? ` <span class="note">${esc(s.code)}</span>` : ''), esc(s.routes.join(', ')), U.fmt.num(s.am, 1), U.fmt.num(s.pm, 1)]) });
  }
  function onClick(e) {
    const b = e.target.closest('[data-feeds]'); if (!b) return;
    act(b.dataset.feeds, b);
    return true;
  }
  function onChange(e) {
    const t = e.target;
    if (t.dataset && t.dataset.feedsIn === 'preset') { const u = t.closest('.src').querySelector('[data-feeds-in=url]'); if (u) { u.disabled = t.value !== 'url'; if (t.value !== 'url') u.value = ''; } return true; }
    if (t.dataset && t.dataset.feedsIn) return true;
  }
  /* Add the section to the Traffic data tab, keeping whatever that tab already does (call again if it is replaced).
     Returns true only when it wrapped the tab now. Other features' markers on the tab (_calibration and the like:
     true-valued '_name' properties, as calibration.js's wrapDataTab copies them) are kept on the new definition, so
     their own "already wrapped" checks keep working and nobody wraps the tab twice. */
  function wrapDataTab() {
    const t = Registry.get('tabs', 'data');
    if (!t || t._feeds) return false;
    const marks = Object.fromEntries(Object.entries(t).filter(([k, v]) => /^_[a-z]/i.test(k) && k !== '_seq' && v === true));
    const base = { render: t.render, onClick: t.onClick, onChange: t.onChange, onInput: t.onInput, onKeydown: t.onKeydown };
    Registry.tabs({ id: 'data', label: t.label, ...(t.order != null ? { order: t.order } : {}), ...(t.title ? { title: t.title } : {}), ...(t.hidden ? { hidden: t.hidden } : {}),
      render: () => base.render() + Registry.html({ id: 'feeds', title: 'Data sources', render: sectionHtml }, 'render'),
      onClick: (e, body) => onClick(e, body) === true ? true : base.onClick ? base.onClick(e, body) : undefined,
      onChange: (e, body) => onChange(e, body) === true ? true : base.onChange ? base.onChange(e, body) : undefined,
      ...(base.onInput ? { onInput: base.onInput } : {}), ...(base.onKeydown ? { onKeydown: base.onKeydown } : {}), ...marks, _feeds: true });
    return true;
  }

  /* ---------- inspector sections ---------- */
  function linkSection(sc, sel) {
    const L = sc.links[sel.id]; if (!L) return '';
    const hz = ((F().hazards || {})[sc.id] || { rows: [] }).rows.filter(r => r.link === L.id);
    const parts = [];
    if (L.nslr) {
      // Older projects may hold L.nslr as a bare number or as { limit }
      const n0 = L.nslr, n = typeof n0 === 'object' ? { ...n0, v: n0.v ?? n0.limit ?? L.speedLimit } : { v: +n0 || L.speedLimit }, src = { source: SRC.nslr.name, date: n.at, licence: SRC.nslr.licence, basis: 'measured' };
      parts.push(`<div class="facts">${Kit.fact(`${esc(n.v)} km/h`, 'legal limit')}${Kit.fact(`${esc(L.speed)} km/h`, 'modelled free-flow')}${n.var ? Kit.fact(`${esc(n.var.v)} km/h`, 'variable (school) limit') : ''}</div>
        <p class="note">${esc(n.cat || 'Permanent')} limit, ${esc(n.rca || 'road controlling authority not stated')}${n.year ? `, in force on 1 July ${esc(n.year)}` : `, current at ${esc(dateText(n.at))}`}. ${n.inst ? a(n.inst, 'Legal instrument') : esc(n.ref || '')} ${Kit.prov(src)}</p>
        ${n.var ? `<p class="note">${esc(n.var.desc)}${n.var.AM != null ? ` · applies in the modelled ${[n.var.AM && 'AM', n.var.PM && 'PM'].filter(Boolean).join(' and ') || 'peaks: no'}` : ''}.</p>` : ''}
        ${L.speed !== n.v && L.status === 'existing' ? Kit.note(`The modelled speed differs from the legal limit${userSpeed(L) ? ' (you changed it)' : ''}. Use Traffic data › Review modelled speeds to align them.`, true) : ''}
        <p class="note">${esc(SRC.nslr.attribution)}</p>`);
    } else if (L.speedSource) parts.push(Kit.note(`No legal limit found in the NZTA register; the modelled speed comes from ${esc(L.speedSource)}.`));
    if (hz.length) parts.push(`<div class="lg-title">Hazards</div>${hz.map(r => `<p class="note">${esc(r.layer)}: <b>${U.fmt.int(r.len)} m</b> of this road${r.only ? ' · the only access to a proposal zone' : ''}.</p>`).join('')}`);
    return parts.length ? Kit.sec('Official data', parts.join('')) : '';
  }
  function nodeSection(sc, sel) {
    const n = sc.nodes[sel.id]; if (!n || !(n.controlSrc || n.atSiteNo != null || n.signs || n.banSrc)) return '';
    const signs = n.signs ? Object.entries(n.signs).map(([lid, t]) => `${esc(t)} on ${esc((sc.links[lid] || {}).name || 'an approach')}`).join('; ') : '';
    const lk = id => esc((sc.links[id] || {}).name || 'an unnamed road');
    const bans = n.banSrc ? Object.entries(n.banSrc).filter(([b]) => (n.bans || []).includes(b)).map(([b, src]) => { const [i, o] = b.split('>'); return `${lk(i)} → ${lk(o)}: ${esc(src)}`; }) : [];
    const gone = n.banSrc ? Object.keys(n.banSrc).filter(b => !(n.bans || []).includes(b)).length : 0;
    return Kit.sec('Control source', `<p class="note">${n.controlSrc ? `${esc(CTL_TEXT(n.controlAuto || n.control))} from ${esc(n.controlSrc)}${n.controlAt ? `, ${esc(dateText(n.controlAt))}` : ''}.` : ''}${userControl(n) ? ' You have since changed it; your choice stays.' : ''}</p>
      ${n.atSiteNo != null ? `<p class="note">AT site number <b>${esc(n.atSiteNo)}</b>${n.ped ? ' (signalised pedestrian crossing, ' + RULES.control.pedCycleRange + ' cycle)' : ''}.</p>` : ''}${signs ? `<p class="note">Signs: ${signs}.</p>` : ''}${bans.length ? `<p class="note">Banned movements from OpenStreetMap: ${bans.join('; ')}.</p>` : ''}${gone ? `<p class="note">You removed ${gone} ban${gone === 1 ? '' : 's'} that came from OpenStreetMap; your choice stays.</p>` : ''}`);
  }
  function countSection(sc, sel) {
    const c = DataHub.D().counts.find(x => x.id === sel.id); if (!c) return '';
    if (c.feed) return Kit.sec('Source', `<p class="note">${esc(c.feed.source)}, imported ${esc(dateText(c.feed.date))}${c.date ? `, count year ${esc(c.date)}` : ''}. ${a(c.feed.url, 'Layer')} ${Kit.prov({ source: c.feed.source, date: c.feed.date, licence: c.feed.licence, basis: 'measured' })}</p>`);
    const s = c.tms; if (!s) return '';
    const t = s.target, v = s.series.v, mx = Math.max(1, ...v.map(Math.abs)), pts = v.map((x, i) => [i, Math.abs(x)]);
    const spark = Kit.line([{ pts, colour: 'var(--accent)', width: 1.2, label: 'veh/day' }], { width: 560, height: 110, cssHeight: 90, xMax: Math.max(1, v.length - 1), yMax: mx * 1.05, yLabel: U.fmt.int(mx), xLabels: [esc(s.from), esc(s.to)], legend: false });
    const dn = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    // Units: vehicles (not PCU) per day; a SiteRef is usually one direction (NZTA lists NB and SB refs separately)
    const dirs = s.dirs || [], way = dirs.length > 1 ? 'both directions' : c.dir ? esc(c.dir) : dirs.length ? 'one direction' : 'direction not stated';
    const wd = s.aawdt ? [U.fmt.int(s.aawdt), 'AAWDT (Mon–Fri, not holidays, last 12 months)'] : s.wdt ? [U.fmt.int(s.wdt.mean), `Weekday mean (Mon–Fri, not holidays; ${esc(s.wdt.n)} days, too few for AAWDT)`] : ['–', 'AAWDT'];
    return Kit.sec('Daily counts · NZTA TMS', `<div class="facts">${Kit.fact(t ? `${U.fmt.int(t.mean)}${t.ci95 != null ? ` ± ${U.fmt.int(t.ci95)}` : ''}` : '–', `Tue–Thu term-time mean (veh/day, ${way}, 95% CI)`)}${Kit.fact(s.hcv != null ? U.fmt.pct(s.hcv, 1) : '–', '%HCV')}${Kit.fact(s.aadt ? U.fmt.int(s.aadt) : '–', s.aadt ? 'AADT (last 12 months)' : `AADT needs ${RULES.tms.aadtDays} valid days`)}${Kit.fact(wd[0], wd[1])}</div>
      <p class="note">${c.obs_method === 'weekday_factored' ? 'The model compares this site with the Tue–Thu term-time mean × the peak share.' : `The model still compares this site with the NZTA AADT (${esc(U.fmt.int(c.adt))} veh/day) × the peak share; the figures here are for checking it.`}</p>
      ${t ? `<p class="note">${esc(t.n)} days in ${esc(t.weeks)} school-term week${t.weeks === 1 ? '' : 's'}, ${esc(dateText(t.from))} to ${esc(dateText(t.to))}. ${esc(t.note)}</p>` : ''}
      <div class="lg-title">Daily history</div>${spark}
      <p class="note">${esc(s.valid)} of ${esc(s.days)} days valid (days under ${RULES.tms.minShare * 100}% of the 28-day median left out). Day-of-week factors: ${s.dow.map((f, i) => f ? `${dn[i]} ${f.toFixed(2)}` : '').filter(Boolean).join(', ')}.${s.growth ? ` Growth ${U.fmt.signed(s.growth.slope)} veh/day a year (${s.growth.pct != null ? U.fmt.signedPct(s.growth.pct / 100) : '–'}), ${esc(s.growth.basis)}, ${esc(s.growth.from)}–${esc(s.growth.to)}.` : ''}</p>
      <p class="note">%HCV = heavy ÷ (light + heavy). ${esc(RULES.tms.hcvRule)}. ${Kit.prov({ source: SRC.tms.name, date: s.at, licence: SRC.tms.licence, basis: 'measured' })}${c.share ? ` Peak shares from ${esc(c.share.src)}: AM ${U.fmt.pct(c.share.AM, 1)}, PM ${U.fmt.pct(c.share.PM, 1)}${periodUsesShare() ? '' : ' (the model still uses the shares in Settings)'}.` : ''}</p>`);
  }
  function zoneSection(sc, sel) {
    const z = sc.zones[sel.id]; if (!z || !z.ags) return '';
    const f = z.ags.f, base = z.luBase || {};
    return Kit.sec('Growth · AGS23 v1.1', `<p class="note">MSM zone <b>${esc(z.ags.msm)}</b>, ${esc(z.ags.base)} to ${esc(z.ags.year)}: households ×${f.hh != null ? f.hh.toFixed(3) : '–'}, jobs ×${f.emp != null ? f.emp.toFixed(3) : '–'}, population ×${f.pop != null ? f.pop.toFixed(3) : '–'}.${z.ags.note ? ' ' + esc(z.ags.note) + '.' : ''}</p>
      <p class="note">Base-year land use: ${Object.entries(base).filter(([, q]) => +q).map(([k, q]) => `${esc((LANDUSES[k] || {}).label || k)} ${U.fmt.num(+q, 0)}`).join(', ') || '–'}. ${esc(z.ags.src)}. ${Kit.prov({ source: SRC.ags.attribution, date: z.ags.at, licence: SRC.ags.licence, basis: 'derived' })}</p>`);
  }
  function gatewaySection(sc, sel) {
    const z = sc.zones[sel.id]; if (!z || !z.agsGrowth) return '';
    const g = z.agsGrowth, S = Store.S;
    return Kit.sec('Growth beyond this gateway · AGS23', `${g.pct != null ? `<div class="facts">${Kit.fact(`${g.pct.toFixed(2)}%`, `a year, linear (${esc(g.base)}–${esc(g.year)})`)}${Kit.fact(`${(+S.growth || 0).toFixed(2)}%`, 'background growth setting')}</div>` : ''}
      <p class="note">${esc(g.basis)} ${Kit.prov({ source: SRC.ags.attribution, date: g.at, licence: SRC.ags.licence, basis: 'derived' })}</p>
      <p class="note">The model grows every gateway at the background growth setting. ${g.pct != null ? `<button class="btn small" data-feeds-ins="agsrate">Use the gateway average</button>` : ''}</p>`);
  }
  const insClick = e => { const b = e.target.closest('[data-feeds-ins]'); if (!b) return; act(b.dataset.feedsIns, b); return true; };

  /* ---------- map modes and layers ---------- */
  const LIMIT_COL = v => v <= 30 ? '#2f9e44' : v <= 40 ? '#74b816' : v <= 50 ? '#1c7ed6' : v <= 60 ? '#7048e8' : v <= 80 ? '#e8590c' : '#c92a2a';
  const LIMIT_BANDS = [[30, '30 km/h or less'], [40, '40 km/h'], [50, '50 km/h'], [60, '60 km/h'], [80, '70–80 km/h'], [110, '90 km/h or more']];
  const swatch = (c, t) => `<div class="lg-row"><span class="lg-sw" style="background:${c}"></span><span>${t}</span></div>`;

  /* ---------- registration ---------- */
  Registry.settings({ id: 'feeds', title: 'Official data feeds', order: 60,
    defaults: () => ({ feeds: { ...DEFAULTS } }),
    render: s => { const f = { ...DEFAULTS, ...(s.feeds || {}) }; return `<div class="set-grid">
      <label class="fld"><span>Time limit for each request</span><span class="fld-in"><input type="number" min="5" step="5" data-set="feeds.timeout" value="${esc(f.timeout)}"><span class="unit">s</span></span></label>
      <label class="fld"><span>TMS daily counts: days to download</span><span class="fld-in"><input type="number" min="28" step="7" data-set="feeds.tmsDays" value="${esc(f.tmsDays)}"><span class="unit">days</span></span></label>
      <label class="check"><input type="checkbox" data-set="feeds.nslrFuture" ${f.nslrFuture ? 'checked' : ''}> Future scenario years use the speed limits gazetted for 1 July of that year</label>
      <label class="check"><input type="checkbox" data-set="feeds.otherScenarios" ${f.otherScenarios ? 'checked' : ''}> Offer to apply speed limits and bus routes to the other scenarios too</label></div>
      <p class="note">AADT needs ${RULES.tms.aadtDays} valid days, so keep at least 400 days for it.</p>`; } });
  Registry.about({ id: 'feeds', title: 'Official data feeds (Traffic data › Data sources)', order: 30,
    intro: 'Fetched live for the study area when you ask; nothing is bundled with Spare Construct. The data-vintage table in the Traffic data tab records each query date.',
    rows: () => [
      [SRC.nslr.label, a(SRC.nslr.page, SRC.nslr.name), `<b>Real.</b> Current versions only (past versions are dropped even if the server returns them); future years use limits gazetted for 1 July. Modal value of three points along each road. ${esc(SRC.nslr.licence)}.`],
      ['Junction controls', a(SRC.controlled.page, 'Auckland Transport RoadingService: controlled intersections and signs'), `<b>Real.</b> Signals within ${RULES.control.siteM} m, signs within ${RULES.control.signM} m (Spare Construct assumptions), applied only after review. ${esc(SRC.controlled.note)} ${esc(DataHub.AT_LICENCE)}.`],
      [SRC.osm.label, a(SRC.osm.page, SRC.osm.name), `<b>Real.</b> OSM turn restrictions through a junction node become banned movements after review; restrictions for some vehicles or times, or via a road, are left out. ${esc(SRC.osm.licence)}.`],
      [SRC.tms.label, a(SRC.tms.page, SRC.tms.name), `<b>Real.</b> Duplicate rows removed (groupBy and max). Tue–Thu school-term means with 95% CI; %HCV; day-of-week and monthly factors. School terms and Matariki dates are entered by Spare Construct (check them). ${esc(RULES.tms.hcvRule)}. CC BY 4.0.`],
      [SRC.ags.label, a(SRC.ags.page, SRC.ags.name), `<b>Real forecast.</b> Factors by MSM zone, not absolute values. ${esc(SRC.ags.licence)}.`],
      [SRC.gtfs.label, a(SRC.gtfs.page, 'Mobility Database (feed list CC0; each feed under its publisher’s licence)'), '<b>Real timetables.</b> A Tuesday in service that is not a public holiday, in school term where Spare Construct has the term dates; buses per hour in two-hour AM and PM windows (Spare Construct assumption).'],
      [SRC.council.label, 'Hamilton City Council, Queenstown Lakes District Council, or a layer you give', '<b>Real counts.</b> You confirm the fields. Checked with your own counts. Each council’s licence applies.'],
      ['Climate hazards', a(SRC.flood.page, 'Auckland Council flood plains and coastal inundation'), `<b>Real.</b> Superseded flood models are left out. ${esc(AC_LICENCE)}.`],
      [SRC.mvr.label, a(SRC.mvr.page, SRC.mvr.name), '<b>Real.</b> Passenger cars and vans by motive power in a district. CC BY 4.0.'],
    ] });
  Registry.inspectorSections({ id: 'feeds-link', types: ['link'], order: 40, render: linkSection });
  Registry.inspectorSections({ id: 'feeds-node', types: ['node'], order: 40, render: nodeSection });
  Registry.inspectorSections({ id: 'feeds-count', types: ['count'], order: 20, render: countSection });
  Registry.inspectorSections({ id: 'feeds-zone', types: ['zone'], order: 40, render: zoneSection });
  Registry.inspectorSections({ id: 'feeds-gateway', types: ['gateway'], order: 40, render: gatewaySection, onClick: insClick });
  Registry.modes({ id: 'feeds-limit', label: 'Legal speed limit', order: 56, proposedOutline: false,
    colour: (key, L, res, c) => L.speedLimit > 0 ? { c: LIMIT_COL(L.speedLimit), w: null, v: L.speedLimit } : { c: c.T.line, w: 1.2 },
    labelText: v => v + '',
    legend: () => `<div class="lg-title">Legal speed limit (NZTA register)</div>${LIMIT_BANDS.map(([v, t]) => swatch(LIMIT_COL(v), t)).join('')}<div class="lg-note">Grey: no limit found. Load it from Traffic data › Data sources. ${esc(SRC.nslr.attribution)}.</div>` });
  Registry.modes({ id: 'feeds-speed', label: 'Legal vs modelled speed', order: 57, proposedOutline: false,
    colour: (key, L, res, c) => !(L.speedLimit > 0) ? { c: c.T.line, w: 1.2 } : L.speed === L.speedLimit ? { c: c.T.losA, w: null, v: `${L.speed}` } : { c: L.speed > L.speedLimit ? c.T.losF : c.T.sel, w: null, v: `${L.speed}/${L.speedLimit}` },
    labelText: v => v + '',
    legend: (res, c) => `<div class="lg-title">Modelled vs legal speed</div>${swatch(c.T.losA, 'Same as the legal limit')}${swatch(c.T.losF, 'Modelled faster than the limit')}${swatch(c.T.sel, 'Modelled slower than the limit')}${swatch(c.T.line, 'No legal limit found')}<div class="lg-note">Labels: modelled/legal km/h. ${esc(SRC.nslr.attribution)}.</div>` });
  Registry.layers({ id: 'feeds-hazards', label: 'Flood and coastal hazards', order: 15, defaultOn: false, below: true,
    draw(ctx, off, size, geo) {
      const H = mem.hazards; if (!H) return;
      for (const lay of H.layers) {
        ctx.fillStyle = lay.src === 'coastal' ? 'rgba(12,133,153,0.22)' : /adjusted/.test(lay.label) ? 'rgba(25,113,194,0.16)' : 'rgba(25,113,194,0.26)';
        for (const p of lay.polys) { ctx.beginPath(); for (const r of p.rings) r.forEach((q, i) => { const s = geo.project([q[1], q[0]]); i ? ctx.lineTo(s.x - off.x, s.y - off.y) : ctx.moveTo(s.x - off.x, s.y - off.y); }); ctx.fill('evenodd'); }
      }
    },
    legend: () => `<div class="lg-title">Hazards (Auckland Council)</div>${swatch('rgba(25,113,194,0.45)', 'Flood plain, current climate')}${swatch('rgba(25,113,194,0.3)', 'Flood plain, climate-change adjusted')}${swatch('rgba(12,133,153,0.4)', 'Coastal inundation')}${mem.hazards ? '' : '<div class="lg-note">Check hazards in Traffic data › Data sources.</div>'}` });
  Registry.layers({ id: 'feeds-at', label: 'AT signal sites and signs', order: 16, defaultOn: false, minZoom: 14,
    draw(ctx, off, size, geo, z) {
      const M = mem.control; if (!M) return;
      const T = Render.theme || {}, r = z >= 16 ? 5 : 3.5;
      ctx.lineWidth = 1.2; ctx.strokeStyle = T.panel || '#fff';
      for (const s of M.sites) { const p = geo.project([s.lat, s.lng]); ctx.fillStyle = s.ped ? '#e8590c' : '#c2255c'; ctx.fillRect(p.x - off.x - r, p.y - off.y - r, 2 * r, 2 * r); ctx.strokeRect(p.x - off.x - r, p.y - off.y - r, 2 * r, 2 * r); }
      for (const g of M.signs) { const p = geo.project([g.lat, g.lng]); ctx.fillStyle = g.type === 'RG 5' ? '#c92a2a' : g.type === 'RG 6R' ? '#1c7ed6' : '#f08c00'; ctx.beginPath(); ctx.arc(p.x - off.x, p.y - off.y, r * 0.75, 0, Math.PI * 2); ctx.fill(); ctx.stroke(); }
    },
    legend: () => `<div class="lg-title">AT data</div><div class="lg-row"><span class="lg-dot" style="background:#c2255c;border-radius:1px"></span><span>Signal site</span></div><div class="lg-row"><span class="lg-dot" style="background:#e8590c;border-radius:1px"></span><span>Signalised crossing</span></div><div class="lg-row"><span class="lg-dot" style="background:#c92a2a"></span><span>Stop sign (RG 5)</span></div><div class="lg-row"><span class="lg-dot" style="background:#f08c00"></span><span>Give way (RG 6)</span></div><div class="lg-row"><span class="lg-dot" style="background:#1c7ed6"></span><span>Roundabout (RG 6R)</span></div>` });
  Registry.layers({ id: 'feeds-bus-stops', label: 'Bus stops (timetable)', order: 17, defaultOn: false, minZoom: 14,
    draw(ctx, off, size, geo, z) {
      const g = (Store.P.feeds || {}).gtfs; if (!g) return;
      const T = Render.theme || {};
      ctx.lineWidth = 1.5; ctx.strokeStyle = T.panel || '#fff';
      for (const s of g.stops) { const p = geo.project([s.lat, s.lng]), r = Math.min(7, 2.5 + Math.sqrt(s.am + s.pm)); ctx.fillStyle = '#0c8599'; ctx.beginPath(); ctx.arc(p.x - off.x, p.y - off.y, r, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
        if (z >= 17 && s.routes.length) { ctx.fillStyle = T.ink || '#222'; ctx.font = '11px sans-serif'; ctx.fillText(s.routes.slice(0, 4).join(' '), p.x - off.x + r + 3, p.y - off.y + 4); } }
    },
    legend: () => `<div class="lg-title">Bus stops</div><div class="lg-row"><span class="lg-dot" style="background:#0c8599"></span><span>Stop (size: buses per hour)</span></div><div class="lg-note">Route numbers show at zoom 17.</div>` });
  // Subscribed to Store directly (not Registry.onStore) so counts get their TMS results before the UI redraws, and
  // headless callers get them too. Never throws into Store.emit.
  Store.on(kind => { if (kind === 'data' || kind === 'project' || kind === 'undo') { try { attachTMS(); } catch (e) { console.error('feeds: ' + e.message); } } });
  Registry.onInit(() => { wrapDataTab(); attachTMS(); });
  wrapDataTab();

  return {
    SRC, RULES, TERMS, MATARIKI, COUNCILS, NZ_FEEDS, DEFAULTS, cfg, F, mem, stamp,
    get, layerMeta, arcQuery, resolve, inRings, centroid,
    nslr: { where: nslrWhere, load: loadNSLR, assign: assignNSLR, all: nslrAll, proposals: speedProposals, apply: applySpeeds, varPeriods },
    control: { load: loadControl, proposals: controlProposals, apply: applyControls, loadBans: loadRestrictions, bans: banProposals, applyBans },
    tms: { load: loadTMS, stats: tmsStats, holidays, inTerm, attach: attachTMS, loadProfiles, sites: tmsSites, dnum, diso },
    ags: { load: loadAGS, zoneFactor: agsZoneFactor, proposals: agsProposals, apply: applyAGS, gateway: agsGateway, gateways: agsGateways, basisText: agsBasisText },
    gtfs: { catalogue, load: loadGTFS, parse: gtfsWork, run: runGtfs, routes: gtfsRoutes, apply: applyGTFS, keep: keepGTFS },
    council: { read: councilRead, guess: guessFields, import: councilImport },
    hazards: { load: loadHazards, links: hazardLinks, table: hazardTable },
    fleet: { load: loadFleet },
    sectionHtml, vintageHtml, vintageRows, reviewHtml, stopsHtml, wrapDataTab, act, snapEach, safeHref,
    sections: { link: linkSection, node: nodeSection, count: countSection, zone: zoneSection, gateway: gatewaySection },
  };
})();
