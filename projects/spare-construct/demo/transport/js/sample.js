'use strict';
/* Spare Construct · the Drury sample (docs/spec/sample-regression).
   A complete, honestly labelled screening project for Drury, south Auckland: real streets, counts, crashes and existing
   land use, with an illustrative private plan change and two illustrative network options, set up as an assessment set
   (S0 2026 base year, S1 2036 do-minimum, S2 2036 with the proposal, S3 2036 with mitigation) so every drawer tab,
   report section and checklist item has something real to show.

   Real (downloaded for the study area each time): OpenStreetMap roads, buildings and land-use areas; Stats NZ Census
   2023 (SA1) and employees (SA2); Ministry of Education schools and early learning; Auckland Unitary Plan zones; NZTA
   and Auckland Transport counts; NZTA CAS crashes; NZTA speed limits (NSLR) and TMS daily counts; Auckland Council flood
   plains and coastal inundation; AT bus timetables (GTFS); the Auckland Council site lookup, the NZTA state highway
   layer and the AUP access controls. A source that does not answer is left out, named in failed[] and in the scenario
   notes, and everything else carries on. Journey times are not checked (observed times need a survey or a paid key).
   To keep the saved project under 3 MB and the runs quick, the network is the through-road network (residential and
   living streets left out), cut at the study area plus a 25% margin, with shapes simplified to 2 m and dead-end roads
   that carry no traffic removed; existing land-use zones sit on a 700 m grid. Each step says so in the build notes.
   Illustrative (made up for the sample, and labelled so wherever it appears): the committed and proposed development
   yields, opening years and build-out shares; the trip-generation schedule's rates and mode shares; Options A and B;
   the junction lane coding at the two coded sites; the precinct register entries; and the turning count, which is
   labelled ILLUSTRATIVE — not a surveyed count and has the role 'Not used', so it never decides a validation result.
   Nothing is signed off: the report author is 'Sample — not signed' and the expert and proportionality steps stay open.

   API
   - Sample.build(opts = { onStatus, signal, run, extras, gtfs }) → { P, notes[], failed[{ id, label, why }], ms }.
     Headless-safe: no DOM and no UI.*; it uses Store, Engine, DataHub, Importers, ExistingLU, Scenarios, Sites and the
     feature APIs (LandusePlus, Calibration, Calibrate, Junctions, Compliance, ComplianceExtra, Feeds, Report) when they
     are loaded. Store.P is the new project while it runs and is put back afterwards: call Store.setProject(r.P) to show
     it. opts.run === true also runs the model (Engine.assign) and matrix estimation, and with extras (default on) the
     staging and sensitivity tests the report reads. Throws when the roads cannot be downloaded or on cancel (AbortError).
   - Sample.analyse(P, { runs, onStatus, signal, extras, notes, failed }): the runs, hold-out, matrix estimation and
     report tests on a built project; runs(scenarios) assigns AM and PM (Engine.assign by default, UI.runModel in drury).
   - Sample.drury(opts): the ☰ Menu entry. Builds with a cancellable busy line, opens the project, runs it with
     UI.runModel and says in a toast what loaded and what is missing. */

const Sample = (() => {
  const BOUNDS = [[-37.1175, 174.9355], [-37.0885, 174.9850]];
  const ILLUSTRATIVE = 'ILLUSTRATIVE — not a surveyed count';
  const LODGEMENT = '2026-11-16';
  const BASE_YEAR = 2026, DESIGN_YEAR = 2036, GROWTH = 1.5;
  const JUSTIFY = 'Screening-level corridor assessment for a plan change ITA (sample)';
  const SAMPLE_SRC = 'Spare Construct sample (illustrative): made up for the Drury sample, not from a survey, consent or precinct provision';
  // Outer time limits for downloads that have none of their own (their results have no side effects, so a late answer is ignored)
  const LIMIT = { roads: 150000, buildings: 180000 };
  const PERIODS = ['AM', 'PM'];
  const MAX_SIZE = 3e6;   // characters of Store.serialise(P)
  // The report's staging and sensitivity tests, kept small (each step is one model run per peak)
  const STAGING_STEPS = [0, 50, 100], SENS_TESTS = ['T1+', 'T4+'];
  const DROP_HIGHWAY = /^(residential|living_street)$/;
  // The modelled area: the study box and a 25% margin (about 800 m north–south and 1.1 km east–west). Roads are cut at
  // its edge and counts and crashes are downloaded for it, so every link's counts and crashes cover all of its length.
  const AREA = (() => { const [a, b] = BOUNDS, h = (b[0] - a[0]) * 0.25, w = (b[1] - a[1]) * 0.25; return { s: a[0] - h, w: a[1] - w, n: b[0] + h, e: b[1] + w }; })();
  // Existing land-use zones on a 700 m grid (ExistingLU's default is 400 m): fewer, larger zones keep the saved sample small
  const ZONE_CELL = 700;
  // Optional modules (feature files may be absent): top-level consts are not window properties, so each is named here
  const MODS = () => ({
    LP: typeof LandusePlus !== 'undefined' ? LandusePlus : null, CAL: typeof Calibration !== 'undefined' ? Calibration : null,
    CB: typeof Calibrate !== 'undefined' ? Calibrate : null, JN: typeof Junctions !== 'undefined' ? Junctions : null,
    CO: typeof Compliance !== 'undefined' ? Compliance : null, CX: typeof ComplianceExtra !== 'undefined' ? ComplianceExtra : null,
    FE: typeof Feeds !== 'undefined' ? Feeds : null, ELU: typeof ExistingLU !== 'undefined' ? ExistingLU : null, RP: typeof Report !== 'undefined' ? Report : null,
  });
  const today = () => new Date().toISOString().slice(0, 10);

  /* ---------- small helpers ---------- */
  const abortError = () => Object.assign(new Error('Sample cancelled'), { name: 'AbortError' });
  const isAbort = e => !!e && e.name === 'AbortError';
  const stop = signal => { if (signal && signal.aborted) throw abortError(); };
  // A promise with a time limit, which a cancel also ends (only for downloads without side effects)
  function within(p, ms, what, signal) {
    return new Promise((res, rej) => {
      let done = false, t = null;
      const onAbort = () => fin(() => rej(abortError()));
      const fin = f => { if (done) return; done = true; if (t) clearTimeout(t); if (signal && signal.removeEventListener) signal.removeEventListener('abort', onAbort); f(); };
      if (isFinite(ms)) t = setTimeout(() => fin(() => rej(new Error(`${what} did not answer within ${Math.round(ms / 1000)} s`))), ms);
      if (signal && signal.addEventListener) signal.addEventListener('abort', onAbort, { once: true });
      Promise.resolve(p).then(v => fin(() => res(v)), e => fin(() => rej(e)));
    });
  }
  const names = (sc, id) => [...new Set(Net.linksAt(sc, id).map(L => L.name || ''))].join(' | ');
  // Nearest node on a road whose name includes `road`
  function nodeOn(sc, ll, road, maxM = 600) {
    const ids = new Set(), re = new RegExp(road, 'i');
    for (const L of Object.values(sc.links)) if (re.test(L.name || '')) { ids.add(L.a); ids.add(L.b); }
    return Net.nearestNode(sc, ll, maxM, n => ids.has(n.id));
  }
  // The junction site (Sites.detect) whose roads include every pattern, nearest to ll
  function siteWith(sc, pats, ll) {
    const list = Object.values(sc.sites || {}).filter(s => { const t = s.nodes.map(id => names(sc, id)).join(' | '); return pats.every(p => new RegExp(p, 'i').test(t)); });
    const centre = s => { const n = s.nodes.map(id => sc.nodes[id]).filter(Boolean); return [n.reduce((a, x) => a + x.lat, 0) / n.length, n.reduce((a, x) => a + x.lng, 0) / n.length]; };
    return list.map(s => ({ s, c: centre(s) })).sort((a, b) => U.dist(a.c, ll) - U.dist(b.c, ll))[0] || null;
  }
  const COMPASS = ['N', 'E', 'S', 'W'];
  const compass = b => COMPASS[Math.round((((b % 360) + 360) % 360) / 90) % 4];

  /* ---------- illustrative inputs ---------- */
  // Committed growth (in S1 to S3) and the proposal (S2, S3). Yields, opening years and build-out are illustrative.
  const COMMITTED = [
    { key: 'i451', reg: 'sample:i451', precinct: 'I451', name: 'Drury East (I451) · committed growth (illustrative yield)', ll: [-37.0962, 174.9675], lu: { residential: 420, apartment: 80 }, year: 2027, buildout: 70 },
    { key: 'i450', reg: 'sample:i450', precinct: 'I450', name: 'Drury Centre (I450) · committed growth (illustrative yield)', ll: [-37.1008, 174.9612], lu: { apartment: 200, retail: 6000, office: 5000 }, year: 2029, buildout: 60 },
  ];
  const PROPOSAL = [
    { key: 'pc', name: 'Appleby Road plan change area (illustrative)', ll: [-37.1012, 174.9775], lu: { residential: 320, apartment: 120, retail: 1500, childcare: 60 }, year: 2030, tg: true },
    { key: 'biz', name: 'Fitzgerald Road business area (illustrative)', ll: [-37.1088, 174.9640], lu: { industrial: 20000, office: 1500 }, year: 2031 },
  ];
  /* The full trip-generation schedule on the plan change area (landuse-plus z.tg): person-trip rates with a mode split for
     the homes and the childcare centre, vehicle rates for the shops, opening hours, directional splits, internal and
     pass-by shares. Every rate is a sample value with no survey behind it, so each row is flagged for a sourced rate. */
  const TG = {
    on: true,
    rows: {
      residential: { basis: 'person', person: { AM: 1.6, PM: 1.8 }, inPct: { AM: 22, PM: 63 }, hours: '24 h', source: 'Illustrative sample rate (not surveyed): replace with RTA, TDB or RR 453 rates' },
      apartment: { basis: 'person', person: { AM: 0.9, PM: 1.0 }, inPct: { AM: 20, PM: 60 }, hours: '24 h', source: 'Illustrative sample rate (not surveyed): replace with RTA, TDB or RR 453 rates' },
      retail: { basis: 'vehicle', veh: { AM: 1.5, PM: 4.5 }, inPct: { AM: 60, PM: 50 }, freight: { AM: 0.05, PM: 0.05 }, hvPct: 8, hours: '7am–9pm', source: 'Illustrative sample rate (not surveyed): Spare Construct default retail rate' },
      childcare: { basis: 'person', person: { AM: 1.2, PM: 1.1 }, inPct: { AM: 55, PM: 45 }, hours: '7am–6pm', source: 'Illustrative sample rate (not surveyed): replace with a surveyed childcare rate' },
    },
    split: { driver: 72, passenger: 9, pt: 9, walk: 7, cycle: 3 },
    internal: 5, passby: 10,
  };
  // Illustrative turning-count volumes (veh/h in the peak hour) by approach (where traffic comes FROM) and movement
  const TURN_VOL = {
    AM: { N: { L: 180, T: 420, R: 60 }, S: { L: 50, T: 520, R: 260 }, E: { L: 300, T: 40, R: 380 }, W: { L: 70, T: 30, R: 60 } },
    PM: { N: { L: 290, T: 560, R: 80 }, S: { L: 70, T: 430, R: 210 }, E: { L: 200, T: 50, R: 260 }, W: { L: 60, T: 40, R: 90 } },
  };
  const BIN = [0.17, 0.2, 0.23, 0.26, 0.27, 0.24, 0.2, 0.18];   // share of the peak-hour volume in each 15 minutes; bins 3–6 make the peak hour
  const WINDOW = { AM: 7 * 60, PM: 16 * 60 };
  /* An illustrative long-format turning-count CSV (calibration feature layout) for a junction site: legs from the site's
     roads by compass side, 15-minute light and heavy counts for 07:00–09:00 and 16:00–18:00. */
  function turnCountCSV(sc, site, opts = {}) {
    const ms = new Set(site.nodes), pts = site.nodes.map(id => sc.nodes[id]).filter(Boolean);
    const c = [pts.reduce((a, n) => a + n.lat, 0) / pts.length, pts.reduce((a, n) => a + n.lng, 0) / pts.length];
    // each road leaving the site, by the compass side it leaves on (the bearing of its first segment)
    const legs = {};
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed' || ms.has(L.a) === ms.has(L.b) || !L.name || L.ring) continue;
      const g = Net.geom(sc, L), k = compass(ms.has(L.a) ? U.bearing(g[0], g[1]) : U.bearing(g[g.length - 1], g[g.length - 2]));
      if (!legs[k]) legs[k] = L.name;
    }
    const hv = 0.06, name = `${ILLUSTRATIVE}: ${opts.name || site.name || 'junction'}`, date = opts.date || '2026-03-10';
    const rows = [['site_id', 'site_name', 'lat', 'lng', 'date', 'time_start', 'interval_min', 'from_leg', 'from_road', 'movement', 'class', 'count']];
    for (const p of PERIODS) for (let i = 0; i < BIN.length; i++) {
      const t = WINDOW[p] + 15 * i, hh = `${String(Math.floor(t / 60)).padStart(2, '0')}:${String(t % 60).padStart(2, '0')}`;
      for (const [leg, mv] of Object.entries(TURN_VOL[p])) {
        if (!legs[leg]) continue;
        for (const [m, v] of Object.entries(mv)) {
          const n = v * BIN[i];
          const h = Math.round(n * hv), l = Math.max(0, Math.round(n) - h);
          rows.push(['SAMPLE-1', name, U.r7(c[0]), U.r7(c[1]), date, hh, 15, leg, legs[leg], m, 'LV', l]);
          rows.push(['SAMPLE-1', name, U.r7(c[0]), U.r7(c[1]), date, hh, 15, leg, legs[leg], m, 'HV', h]);
        }
      }
    }
    return { csv: U.csv(rows), legs, centre: c, name };
  }

  /* ---------- the build ---------- */
  async function build(opts = {}) {
    const t0 = Date.now(), signal = opts.signal || null, onStatus = typeof opts.onStatus === 'function' ? opts.onStatus : () => {};
    const notes = [], failed = [], prevP = Store.P;
    const status = t => { try { onStatus(t); } catch { } };
    const fail = (id, label, why) => { if (!failed.some(f => f.id === id)) failed.push({ id, label, why: String(why || 'no answer') }); };
    // One optional step: a failure is recorded under its label and the build carries on (a cancel always stops it)
    const soft = async (id, label, fn, doing = `Downloading ${label}`) => {
      stop(signal); status(doing + '…');
      try { return await fn(); }
      catch (e) { if (isAbort(e) && signal && signal.aborted) throw e; fail(id, label, e && e.message); return null; }
    };
    const { LP, CAL, JN, CO, CX, FE, ELU } = MODS();
    const P = newProject('Drury East plan change (sample)');
    const S = P.settings;
    Object.assign(S, { baseYear: BASE_YEAR, year: BASE_YEAR, growth: GROWTH, growthMode: 'linear', period: 'AM' });
    S.tmdg = { category: 'E', justification: JUSTIFY };
    P.view = { c: [-37.1012, 174.9605], z: 15 };
    const bounds = L.latLngBounds(BOUNDS[0], BOUNDS[1]);
    const src = P.scenarios[0];
    src.name = 'Sample source network';
    Store.P = P;
    try {
      /* 1 · roads (the one step the sample cannot do without) */
      stop(signal); status('Downloading Drury streets from OpenStreetMap…');
      let data;
      try { data = await within(Importers.overpass(bounds, { local: true }, status), LIMIT.roads, 'OpenStreetMap', signal); }
      catch (e) {
        if (isAbort(e)) throw e;
        throw Object.assign(new Error(`Could not download the sample streets from OpenStreetMap (${e.message}). Check your connection and try again.`), { sampleStage: 'roads' });
      }
      status('Building the network…');
      // Residential streets and living streets are left out: zones load onto the nearest through road, which keeps the
      // sample small (the saved project under 3 MB) and quick to run. Motorways down to rural (unclassified) roads stay.
      const all = (data && data.elements) || [], local = new Set();
      for (const e of all) if (e.type === 'way' && DROP_HIGHWAY.test((e.tags || {}).highway || '')) local.add(e.id);
      // Roads are cut where they leave the study area and its margin, so the network, counts and crashes cover one area
      const clipped = clipWays(all.filter(e => !(e.type === 'way' && local.has(e.id))), AREA);
      Importers.osmToScenario(src, { ...data, elements: clipped.elements }, { sites: true });
      const thin = simplify(src, 2);
      notes.push(`OpenStreetMap roads from motorways down to rural roads, cut at the study area plus a margin (${clipped.cut} roads cut at the edge); ${local.size} residential and living streets were left out to keep the sample small, so zones load onto the nearest through road. Road shapes are simplified to within 2 m (${thin} shape points dropped).`);
      if (Object.keys(src.links).length < 50) throw Object.assign(new Error('Could not download the sample streets: OpenStreetMap sent back no roads for Drury. Check your connection and try again.'), { sampleStage: 'roads' });
      if (!Object.keys(src.sites || {}).length && typeof Sites !== 'undefined') Sites.detect(src);
      Importers.autoGateways(src, bounds);
      Store.edited('net', src);

      /* 2 · counts and crashes (one source at a time, so each can fail on its own), for the area the roads were cut to */
      const near = L.latLngBounds([AREA.s, AREA.w], [AREA.n, AREA.e]), area = () => near;
      const SRC_LABEL = { nzta: 'NZTA state highway counts', at: 'Auckland Transport counts', cas: 'NZTA crash records (CAS)' };
      for (const w of ['nzta', 'at', 'cas']) {
        stop(signal);
        const msgs = await DataHub.loadAll(area(w), [w], status, signal ? { signal } : {});
        const bad = msgs.find(m => / failed /.test(m));
        if (bad) fail(w, SRC_LABEL[w], bad.replace(/^\w+ failed \((.*)\)$/, '$1'));
      }
      Store.emit('data');

      /* 3 · existing land use: OpenStreetMap buildings, then Census, employees, schools and zoning */
      let els = null;
      if (ELU) els = await soft('osm-buildings', 'OpenStreetMap buildings and land-use areas', () => within(ELU.fetch(bounds, status), LIMIT.buildings, 'OpenStreetMap buildings', signal));
      else fail('osm-buildings', 'OpenStreetMap buildings', 'the existing land use module is not loaded');
      let built = null;
      if (els && els.length) {
        status('Grouping buildings into zones…');
        built = ELU.build(els, bounds, { links: src.links, cell: ZONE_CELL });
        for (const z of built.zones || []) src.zones[z.id] = { connectors: null, lu: {}, ...z, kind: 'zone', existing: true, src: 'existing-lu' };
      }
      let fill = null;
      if (LP) {
        stop(signal); status('Downloading Census 2023, employees, schools and zoning…');
        try { fill = await LP.fillAll(src, { bounds, elements: els || [] }, status); }
        catch (e) { fail('census', 'Census, employees, schools and zoning', e.message); }
        if (fill) {
          const w = fill.warnings || [], why = re => (w.find(x => re.test(x)) || 'no answer');
          if (!fill.census) fail('census', 'Stats NZ Census 2023 (SA1)', why(/census|SA1|2023/i));
          if (!fill.employees) fail('employees', 'Stats NZ employees by SA2', why(/employ|SA2/i));
          if (!fill.schools || !fill.schools.loaded || !fill.schools.loaded.schools) fail('moe-schools', 'Ministry of Education schools', why(/school|education|catalogue/i));
          if (!fill.schools || !fill.schools.loaded || !fill.schools.loaded.ece) fail('moe-ece', 'Ministry of Education early learning services', why(/early|ece|education|catalogue/i));
          if (!fill.aup) fail('aup', 'Auckland Unitary Plan zones', why(/unitary|AUP|zone/i));
        }
      } else fail('census', 'Census, employees, schools and zoning', 'the land use module is not loaded');
      const existing = Object.values(src.zones).filter(z => z.kind !== 'gateway' && z.existing === true);
      if (!existing.length) notes.push('No existing land use could be built (OpenStreetMap buildings and the Census did not load), so the base year has gateway traffic only.');
      Store.edited('demand', src);

      /* 4 · gateway flows from the counts */
      const nGw = DataHub.gatewaysFromCounts(src);
      Store.edited('demand', src);

      /* 5 · official feeds: legal speed limits, TMS daily counts, flood hazards, bus timetables */
      let hazards = null;
      if (FE) {
        await soft('nslr', 'NZTA legal speed limits (National Speed Limit Register)', () => FE.nslr.all(near, [src], status));
        if (DataHub.D().counts.some(c => c.src === 'nzta')) await soft('tms', 'NZTA TMS daily counts', () => FE.tms.load(FE.tms.sites(), { status }));
        hazards = await soft('flood', 'Auckland Council flood plains and coastal inundation areas', async () => {
          const r = await FE.hazards.load(near, { status });
          if (!r.layers.length) throw new Error(r.errors[0] || 'no hazard layer answered');
          return r;
        });
        if (opts.gtfs !== false) await soft('gtfs', 'Auckland Transport bus timetable (GTFS)', async () => {
          const cat = await FE.gtfs.catalogue(near);
          const feed = cat && cat.feeds && cat.feeds[0]; if (!feed) throw new Error('no bus timetable covers Drury');
          const g = await FE.gtfs.load(feed, near, { status });
          const routes = FE.gtfs.routes(src, g);
          FE.gtfs.apply(src, routes); FE.gtfs.keep(g, routes);
          return routes.length;
        });
        Store.emit('data');
      }

      /* 6 · junction sites with lane coding (illustrative), before the scenarios are copied */
      const sites = [];
      const coded = [
        { pats: ['Great South', 'Waihoehoe'], ll: [-37.1035, 174.9500], label: 'Great South Road / Waihoehoe Road roundabout' },
        { pats: ['Waihoehoe', 'Fitzgerald'], ll: [-37.1042, 174.9588], label: 'Waihoehoe Road / Fitzgerald Road' },
      ];
      for (const c of coded) {
        const hit = siteWith(src, c.pats, c.ll);
        if (!hit || U.dist(hit.c, c.ll) > 800) { notes.push(`The junction site ${c.label} was not found in the downloaded roads, so it has no lane coding.`); continue; }
        sites.push({ ...c, site: hit.s, centre: hit.c });
        if (JN && JN.approaches && JN.editLanes) codeLanes(src, hit.s, JN);
      }

      /* 7 · committed growth (register rows), the proposal and Option A */
      P.register = (typeof Register !== 'undefined' ? Register.presets() : []).filter(r => r.id !== 'preset:drury-metro');
      for (const c of COMMITTED) {
        const id = U.uid('z');
        src.zones[id] = { id, kind: 'zone', role: 'committed', reg: c.reg, name: c.name, lat: c.ll[0], lng: c.ll[1], lu: { ...c.lu }, connectors: null, year: c.year, buildout: c.buildout,
          luSrc: Object.fromEntries(Object.entries(c.lu).map(([k, q]) => [k, { source: SAMPLE_SRC, basis: 'assumed', date: today(), q }])) };
        P.register.push({ id: c.reg, ref: `AUP ${c.precinct}`, name: `${c.precinct === 'I450' ? 'Drury Centre' : 'Drury East'} precinct (${c.precinct}): committed growth to ${DESIGN_YEAR} (illustrative yield and staging)`,
          type: 'precinct', yield: Object.entries(c.lu).map(([k, q]) => `${q} ${(LANDUSES[k] || {}).unit || k}`).join(', '), rates: null, status: `Operative precinct (AUP ${c.precinct})`, likely: 'likely', funding: null,
          source: SAMPLE_SRC, date: today(), asAt: today(), verify: true, preset: false, zones: [id], links: [],
          note: `Sample yield; ${c.buildout}% built by ${DESIGN_YEAR}, first homes ${c.year}. Check the precinct provisions and the developer's programme.` });
      }
      let tgZone = null;
      for (const d of PROPOSAL) {
        const id = U.uid('z');
        const z = src.zones[id] = { id, kind: 'zone', role: 'development', name: d.name, lat: d.ll[0], lng: d.ll[1], lu: { ...d.lu }, connectors: null, year: d.year,
          luSrc: Object.fromEntries(Object.entries(d.lu).map(([k, q]) => [k, { source: SAMPLE_SRC, basis: 'assumed', date: today(), q }])) };
        if (d.tg) tgZone = z;
      }
      if (tgZone && LP) { LP.setTable(tgZone, true, S, src); Object.assign(tgZone.tg, JSON.parse(JSON.stringify(TG))); LP.sync(tgZone, S, src); }
      const optA = optionA(src);
      if (optA.length < 2) notes.push('Option A could not find all of Takotowhenua Avenue, Appleby Road and Sutton Road in the downloaded roads, so it has fewer new links.');
      Store.edited('net', src);

      /* 8 · calibration data: one screenline and one illustrative turning count */
      if (CAL) {
        const st = CAL.store(P);
        st.screenlines.push({ id: U.uid('sl'), name: 'North screenline: Great South Road, SH1 and Parkhaven Drive', type: 'screen', role: 'calib', motorways: true,
          pts: [[-37.0870, 174.930], [-37.0870, 174.955]], src: { source: 'Spare Construct sample: drawn across Great South Road and the Southern Motorway north of Drury', basis: 'user', date: today() } });
        const gsr = sites.find(s => /Great South/.test(s.label));
        if (gsr) {
          const t = turnCountCSV(src, gsr.site, { name: 'Great South Road / Waihoehoe Road roundabout' });
          try {
            const r = CAL.importTurnCSV(t.csv, `drury-turning-count-ILLUSTRATIVE.csv`);
            for (const s of r.sites) { s.role = 'off'; s.illustrative = true; s.flags = [ILLUSTRATIVE, 'role Not used: made up for the sample, so it never decides a validation result', ...s.flags.filter(f => f !== ILLUSTRATIVE)]; s.file = `${ILLUSTRATIVE} (drury-turning-count-ILLUSTRATIVE.csv)`; }
          } catch (e) { notes.push(`The illustrative turning count could not be read: ${e.message}`); }
        }
      }

      // Dead-end roads that no zone, gateway, count, bus route or junction site uses carry no traffic: leave them out
      const stubs = pruneStubs(src);
      if (stubs) notes.push(`${stubs} dead-end road links with no zone, gateway, count or bus stop on them were left out (they carry no modelled traffic).`);
      Store.edited('net', src);

      /* 9 · the assessment set: S0 existing, S1 do-minimum, S2 proposal, S3 mitigation */
      status('Setting up the assessment scenarios…');
      const set = Scenarios.createSet(P, { type: 'planchange', countYear: BASE_YEAR, opening: DESIGN_YEAR, design: DESIGN_YEAR, from: src.id, mitigation: true, periods: PERIODS });
      P.scenarios = P.scenarios.filter(s => s !== src);
      const by = r => set.scenarios.find(s => s.role === r);
      const S0 = by('existing'), S1 = by('baseline'), S2 = by('proposal'), S3 = by('mitigation');
      S0.name = `${BASE_YEAR} base year (existing)`; S1.name = `${DESIGN_YEAR} do-minimum`; S2.name = `${DESIGN_YEAR} Option A · East links`; S3.name = `${DESIGN_YEAR} Option B · Widen Waihoehoe`;
      S2.buildout = 100; S3.buildout = 100;
      const widened = optionB(S3);
      if (!widened) notes.push('Option B found no Waihoehoe Road links to widen in the downloaded roads.');
      P.base = S0.id; P.active = S2.id;
      if (LP) for (const sc of [S2, S3]) for (const z of Object.values(sc.zones)) if (z.tg && z.tg.on) LP.sync(z, S, sc);
      // hazard lengths for the proposal scenario, which the climate adaptation table assesses (kept for one scenario only, for
      // size: Traffic data › Flood and coastal hazards fills the others)
      if (hazards && FE) { const Hz = (P.feeds ||= {}).hazards ||= {}; Hz[S2.id] = { at: new Date().toISOString(), layers: hazards.layers, rows: FE.hazards.links(S2) }; }
      Store.edited('net', S0);

      /* 10 · count roles: about 25% of eligible counts held out for validation; gateway counts are tagged by their use */
      let roles = null;
      if (CAL) {
        roles = CAL.holdOut(S0);
        CAL.matchSites(S0);
        Store.emit('data');
      }

      /* 11 · regulatory context, precinct staging and report details */
      const devZones = Object.values(S2.zones).filter(z => z.role === 'development');
      const site = devZones.find(z => z.tg) || devZones[0];
      if (CO) {
        const C = CO.state(P);
        Object.assign(C.ctx, { pathway: 'pc-private', lodgement: LODGEMENT, lodgementActual: false, region: 'auckland', context: 'urban' });
        if (site) Object.assign(C.site, { lat: U.r7(site.lat), lng: U.r7(site.lng), src: 'Sample: centre of the illustrative plan change area' });
        await soft('ac-site', 'Auckland Council site lookup (zone, precinct, FDS area)', () => CO.lookupSite(P), 'Looking up the site zone and precinct (Auckland Council)');
        await soft('nzta-sh', 'NZTA state highway network', () => CO.loadSH(P, { sc: S0 }));
        await soft('aup-controls', 'Auckland Unitary Plan arterial and access controls', () => CO.loadOverlays(P, { sc: S0 }));
      }
      if (CX) {
        const cx = CX.data(P);
        cx.precinct.id = 'I451';
        cx.precinct.register = COMMITTED.filter(c => c.precinct === 'I451').map(c => ({ id: U.uid('reg'), ref: `${c.name} (${SAMPLE_SRC})`, dwellings: Math.round(((c.lu.residential || 0) + (c.lu.apartment || 0)) * c.buildout / 100),
          retail: Math.round((c.lu.retail || 0) * c.buildout / 100), commercial: Math.round((c.lu.office || 0) * c.buildout / 100), community: 0, date: '', enabled: '' }));
        const pz = PROPOSAL.find(p => p.tg);
        cx.precinct.proposal = { ref: `${pz.name}: sample private plan change`, dwellings: (pz.lu.residential || 0) + (pz.lu.apartment || 0), retail: pz.lu.retail || 0, commercial: pz.lu.office || 0, community: 0 };
      }
      P.report = { details: { site: 'Appleby Road and Waihoehoe Road, Drury East, Auckland (sample site, illustrative)', client: 'Sample project (illustrative)', author: 'Sample — not signed', pathway: 'planchange', lodgement: LODGEMENT, date: '' },
        scope: 'broad', signoff: {}, revisions: [{ rev: 'A', date: today(), desc: 'Sample draft made by Spare Construct (illustrative; not for lodgement)', prepared: 'Spare Construct sample', reviewed: '', approved: '' }], engagement: [], text: {}, sel: {} };

      /* 12 · what is real and what is illustrative, on each scenario */
      const d = DataHub.D(), got = id => !failed.some(f => f.id === id);
      const real = [`OpenStreetMap roads (${Object.keys(S0.links).length} links, ODbL)`];
      if (existing.length) real.push(`existing land use in ${existing.length} zones from ${[els && els.length ? 'OpenStreetMap buildings' : '', fill && fill.census ? 'Stats NZ Census 2023 dwellings and people' : '', fill && fill.employees ? 'Stats NZ employees (2025)' : '', fill && fill.schools && fill.schools.loaded && fill.schools.loaded.schools ? 'Ministry of Education school rolls' : ''].filter(Boolean).join(', ')}`);
      if (got('nzta') || got('at')) real.push(`${d.counts.filter(c => c.src === 'nzta').length} NZTA and ${d.counts.filter(c => c.src === 'at').length} Auckland Transport count sites`);
      if (got('cas')) real.push(`${d.crashes.length} NZTA CAS crashes`);
      if (nGw) real.push(`${nGw} gateway flows set from counts`);
      if (got('nslr') && FE) real.push('NZTA legal speed limits where the register has a current version');
      if (got('gtfs') && FE && opts.gtfs !== false) real.push('Auckland Transport bus routes and frequencies (GTFS)');
      if (hazards) real.push('Auckland Council flood plains and coastal inundation areas');
      const missing = failed.map(f => f.label);
      const tail = missing.length ? ` Not loaded in this build (so missing, not estimated): ${missing.join('; ')}.` : '';
      const indic = 'Indicative: trip rates (Spare Construct defaults), junction delay (HCM 6 / Akcelik-based, not SIDRA output), the lane coding at the two coded junction sites (illustrative).';
      S0.notes = `${BASE_YEAR} base year. Real: ${real.join('; ')}. ${indic} Matrix estimation is fitted to the calibration counts only; about a quarter of the counts are held out for validation, and counts used to set gateway flows are tagged as gateway counts.${tail}`;
      S1.notes = `${DESIGN_YEAR} do-minimum: the ${BASE_YEAR} network and existing land use, plus committed growth in the Drury Centre (I450) and Drury East (I451) precincts. Illustrative: their yields, first-home years and ${DESIGN_YEAR} build-out shares are sample assumptions, not the precinct provisions or a developer programme. Background through traffic grows linearly at ${GROWTH}% of ${BASE_YEAR} volume a year (Spare Construct default; check against MSM).${tail}`;
      S2.notes = `${DESIGN_YEAR} with the proposal: S1 plus an illustrative private plan change (${devZones.map(z => z.name).join(', ')}; the plan change area carries a full trip-generation schedule with illustrative person-trip rates and mode shares) and Option A · East links (illustrative): a collector from Takotowhenua Avenue through Drury East to Appleby Road and an arterial extension of Takotowhenua Avenue north to Sutton Road. Not Auckland Council, AT or developer proposals.${tail}`;
      S3.notes = `${DESIGN_YEAR} with mitigation: everything in S2, plus Option B · Waihoehoe Road four-laned from the Great South Road roundabout to Takotowhenua Avenue (illustrative mitigation, ${widened} links). S3 against S2 shows the effect of the widening.${tail}`;
      notes.push(`Real: ${real.join('; ')}.`);
      notes.push('Illustrative: development yields and staging, Options A and B, the trip-generation rates, the junction lane coding and the turning count (labelled ' + ILLUSTRATIVE + ').');
      if (roles) notes.push(`Count roles: ${roles.valid} validation counts held out, ${roles.calib} calibration counts; gateway counts tagged.`);
      notes.push('Journey times are not checked: observed times need a survey, GPS data or a Google or TomTom key, and none is used by the sample.');
      P.sample = { version: 1, built: new Date().toISOString(), bounds: BOUNDS, area: AREA, lodgement: LODGEMENT, failed: failed.map(f => ({ ...f })),
        illustrative: ['development yields, opening years and build-out', 'Options A and B', 'trip-generation rates and mode shares', 'junction lane coding at two sites', 'precinct register entries', `the turning count (${ILLUSTRATIVE})`] };

      /* 13 · runs, matrix estimation and the report's staging and sensitivity tests */
      if (opts.run === true) await analyse(P, { onStatus: status, signal, extras: opts.extras, notes, failed });
      return { P, notes, failed, ms: Date.now() - t0, scenarios: { S0, S1, S2, S3 } };
    } finally {
      Store.P = prevP;
    }
  }

  /* Lane coding (illustrative) on every approach into a junction site: a kerb lane for left and through and a right lane
     for through and right; a one-lane road gets a 40 m flare for the right lane. Stored on the site (junctions feature). */
  function codeLanes(sc, site, JN) {
    const ms = new Set(site.nodes), SRC = 'illustrative (sample)';
    site.lanes ||= {};
    for (const nid of site.nodes) for (const ap of JN.approaches(sc, nid)) {
      if (ms.has(ap.link.a) && ms.has(ap.link.b)) continue;
      // an empty record on the site first, so editLanes stores the coding on the junction site rather than the node
      if (!site.lanes[ap.link.id]) site.lanes[ap.link.id] = { lanes: [], src: SRC };
      JN.editLanes(sc, nid, ap.link.id, cur => { cur.lanes = [{ d: 'LT', len: 0 }, { d: 'TR', len: ap.gl >= 2 ? 0 : 40 }]; return ['lanes', 'len']; });
      const rec = site.lanes[ap.link.id];
      if (rec) { rec.src = SRC; rec.srcBy = { lanes: SRC, len: SRC }; }
    }
  }
  /* Cut OpenStreetMap ways (Overpass 'out body; >; out body qt') where they leave box B: each run of nodes inside B
     becomes its own way, ending at a new node on the edge. Returns { elements, cut }. */
  function clipWays(elements, B) {
    const N = new Map(); for (const e of elements) if (e.type === 'node') N.set(e.id, e);
    const inB = o => !!o && o.lat >= B.s && o.lat <= B.n && o.lon >= B.w && o.lon <= B.e;
    const out = elements.filter(e => e.type !== 'way'), extra = [];
    let cut = 0, k = 0;
    const edge = (p, q) => {   // the point where p (inside) → q (outside) crosses the edge
      let lo = 0, hi = 1;
      for (let i = 0; i < 30; i++) { const m = (lo + hi) / 2; if (inB({ lat: p.lat + (q.lat - p.lat) * m, lon: p.lon + (q.lon - p.lon) * m })) lo = m; else hi = m; }
      const n = { type: 'node', id: `edge${++k}`, lat: U.r7(p.lat + (q.lat - p.lat) * lo), lon: U.r7(p.lon + (q.lon - p.lon) * lo) };
      extra.push(n); return n.id;
    };
    for (const w of elements) {
      if (w.type !== 'way') continue;
      const ids = (w.nodes || []).filter(id => N.has(id)), ins = ids.map(id => inB(N.get(id)));
      if (ins.every(Boolean)) { out.push(w); continue; }
      cut++;
      for (let i = 0, r = 0; i < ids.length; i++) {
        if (!ins[i]) continue;
        let j = i; while (j + 1 < ids.length && ins[j + 1]) j++;
        const run = ids.slice(i, j + 1);
        if (i > 0) run.unshift(edge(N.get(ids[i]), N.get(ids[i - 1])));
        if (j < ids.length - 1) run.push(edge(N.get(ids[j]), N.get(ids[j + 1])));
        if (run.length > 1) out.push({ ...w, id: r ? `${w.id}-${r}` : w.id, nodes: run });
        r++; i = j;
      }
    }
    return { elements: [...out, ...extra], cut };
  }
  /* Remove dead-end chains that cannot carry traffic: a road end (one link) that is not a zone connector or gateway node,
     not in a junction site, not on a bus route, and whose link has no count matched to it. Repeats inward along the chain.
     Exact for the assignment (no path can use a dead end it does not start or end in). */
  function pruneStubs(sc) {
    const keepN = new Set(), keepL = new Set();
    for (const ids of Object.values(Engine.connectorMap(sc))) for (const id of ids) keepN.add(id);
    for (const z of Object.values(sc.zones)) if (z.node) keepN.add(z.node);
    for (const s of Object.values(sc.sites || {})) for (const id of s.nodes) keepN.add(id);
    for (const r of Object.values(sc.routes || {})) for (const id of r.wp || []) keepN.add(id);
    for (const mt of DataHub.matches(sc).values()) if (mt) keepL.add(mt.link);
    let n = 0, again = true;
    while (again) {
      again = false;
      for (const [id, d] of Net.degree(sc)) {
        if (d !== 1 || keepN.has(id)) continue;
        const Lk = Net.linksAt(sc, id)[0];
        if (!Lk || keepL.has(Lk.id) || Lk.status !== 'existing') { keepN.add(id); continue; }
        Net.deleteLink(sc, Lk.id); n++; again = true;
      }
    }
    return n;
  }
  // Douglas-Peucker simplification of each link's shape points to within tol metres (lengths change by well under 0.1%)
  function simplify(sc, tol) {
    let dropped = 0;
    for (const Lk of Object.values(sc.links)) {
      const g = Net.geom(sc, Lk); if (g.length < 3) continue;
      const F = U.flat(g[0][0]), xy = g.map(p => [F.x(p), F.y(p)]), keep = new Uint8Array(g.length); keep[0] = keep[g.length - 1] = 1;
      const st = [[0, g.length - 1]];
      while (st.length) {
        const [i, j] = st.pop(); let k = -1, dm = tol;
        for (let m = i + 1; m < j; m++) { const d = U.segDist(xy[m][0], xy[m][1], xy[i][0], xy[i][1], xy[j][0], xy[j][1]).d; if (d > dm) { dm = d; k = m; } }
        if (k > 0) { keep[k] = 1; st.push([i, k], [k, j]); }
      }
      const pts = g.slice(1, -1).filter((p, m) => keep[m + 1]);
      dropped += Lk.pts.length - pts.length; Lk.pts = pts;
    }
    return dropped;
  }
  // Option A · East links: a collector loop to Appleby Road and an arterial north to Sutton Road (proposal links)
  function optionA(sc) {
    const s = nodeOn(sc, [-37.0988, 174.9600], 'Takotowhenua'), e = nodeOn(sc, [-37.1006, 174.9722], 'Appleby'), n2 = nodeOn(sc, [-37.0936, 174.9552], 'Sutton');
    const made = [];
    if (s && e) made.push(Net.addLink(sc, s, e, [[-37.0983, 174.9628], [-37.0981, 174.9660], [-37.0987, 174.9692], [-37.0997, 174.9710]],
      { cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'proposed', stage: 'proposal', name: 'Drury East Link (Option A, illustrative)', cycle: 'separated' }));
    if (s && n2) made.push(Net.addLink(sc, s, n2, [[-37.0968, 174.9590], [-37.0950, 174.9570]],
      { cls: 'arterial', lanesAB: 1, lanesBA: 1, speed: 50, status: 'proposed', stage: 'proposal', name: 'Takotowhenua Avenue extension (Option A, illustrative)', cycle: 'separated' }));
    Net.autoControl(sc, [s, e, n2].filter(Boolean));
    if (s && sc.nodes[s] && made.length === 2) { sc.nodes[s].control = 'roundabout'; sc.nodes[s].upgrade = true; }
    return made;
  }
  // Option B · Waihoehoe Road four-laned between Great South Road and Takotowhenua Avenue (mitigation stage)
  function optionB(sc) {
    let n = 0;
    for (const Lk of Object.values(sc.links)) {
      if (!/waihoehoe/i.test(Lk.name || '') || Lk.ring) continue;
      const A = sc.nodes[Lk.a], B = sc.nodes[Lk.b]; if (!A || !B) continue;
      if (Math.max(A.lng, B.lng) > 174.9600 || Math.min(A.lng, B.lng) < 174.9519) continue;
      Lk.lanesAB = Math.max(2, Lk.lanesAB); Lk.lanesBA = Math.max(2, Lk.lanesBA); Lk.cls = 'arterial'; Lk.speed = 50; Lk.status = 'proposed'; Lk.stage = 'mitigation'; n++;
    }
    return n;
  }

  /* ---------- runs, matrix estimation and report tests ---------- */
  async function engineRuns(scs, status, signal) {
    for (const sc of scs) for (const p of PERIODS) {
      stop(signal);
      status(`${sc.name} · ${p} peak…`);
      sc.res[p] = await Engine.assign(sc, { period: p, onProgress: (it, gap, msg) => { if (signal && signal.aborted) throw abortError(); if (it % 20 === 0) status(`${sc.name} · ${p} peak · ${msg || 'iteration ' + it}`); } });
      Registry.fire('results', sc, p, sc.res[p]);
    }
  }
  async function analyse(P, o = {}) {
    const signal = o.signal || null, status = t => { try { (o.onStatus || (() => {}))(t); } catch { } };
    const notes = o.notes || [], failed = o.failed || [];
    const runs = o.runs || (scs => engineRuns(scs, status, signal));
    const { CAL, CB, RP, CO, CX } = MODS();
    const role = r => P.scenarios.find(s => s.role === r);
    const S0 = role('existing'), rest = ['baseline', 'proposal', 'mitigation'].map(role).filter(Boolean);
    const keep = Store.P; Store.P = P;
    const t0 = Date.now();
    try {
      // base year first: matrix estimation needs it, and its factors then apply to every scenario
      await runs([S0]);
      // Matrix estimation on the calibration counts only (Calibration.runME → Calibrate.fit): a first fit that makes no
      // change is tried once more with smaller steps (XAMAX 1.5). The outcome of every attempt is kept in P.sample.me.
      const rec = (P.sample ||= {}).me = {};
      if (CAL && CB) for (const p of PERIODS) {
        const tries = [];
        for (const xamax of [2, 1.5]) {
          stop(signal); status(`Fitting the ${p} peak to the calibration counts${xamax < 2 ? ' (smaller steps)' : ''}…`);
          let r;
          try { r = await CAL.runME(S0, { period: p, xamax, onProgress: (k, n, msg) => { if (signal && signal.aborted) throw abortError(); status(`Matrix estimation, ${p} peak: ${msg || ''}`); } }); }
          catch (e) { if (isAbort(e)) throw e; r = { ok: false, note: `failed: ${e.message}` }; }
          tries.push({ xamax, ok: !!r.ok, note: String(r.note || (r.ok ? '' : 'no calibration counts')).replace(/\.+$/, '') });
          if (r.ok || !/no pass improved/i.test(r.note || '')) break;
        }
        const last = tries[tries.length - 1];
        rec[p] = { at: new Date().toISOString(), ok: last.ok, tries, info: !!(P.me && P.me.info && P.me.info[p]) };
        notes.push(`${p} peak: ${last.note}.${last.ok ? '' : ` The ${p} model uses the prior matrix${tries.length > 1 ? ' (smaller steps were tried too)' : ''}.`}`);
      }
      if (CAL) CAL.refreshValidation(P);
      await runs(rest);
      if (CAL) {
        CAL.matchSites(S0); CAL.refreshValidation(P);
        // the validation line the report prints: independent (held-out) counts only, TMDG Category E
        for (const p of PERIODS) { try { const a = CAL.assess(S0, p, { set: 'valid', area: 'auto', standard: 'tmdg' }); notes.push(`Validation (${p}, held-out counts): ${a.line}`); } catch { } }
      }
      // the mitigation register and condition parameters, seeded from the modelled effects and the mitigation scenario
      if (CX) {
        try {
          CX.seedMitigation(P);
          // keep the measures from the mitigation scenario and the ten largest unaddressed effects (by DoS or v/c)
          const cx = CX.data(P), eff = new Map(CX.effects(P).map(e => [e.id, e])), size = m => { const e = eff.get(m.effect); return e ? +(e.dos || e.vc || 0) : 0; };
          const measures = cx.mitigation.filter(m => /^diff:/.test(m.seed || '')), open = cx.mitigation.filter(m => /^eff:/.test(m.seed || '')).sort((a, b) => size(b) - size(a));
          cx.mitigation = [...measures, ...open.slice(0, 10), ...cx.mitigation.filter(m => !/^(diff|eff):/.test(m.seed || ''))];
          if (open.length > 10) notes.push(`The mitigation register lists the ten largest of ${open.length} modelled effects not yet addressed; the Compliance+ tab lists them all.`);
          CX.seedConditions(P);
        } catch (e) { notes.push(`The mitigation register could not be seeded: ${e.message}`); }
      }
      if (o.extras !== false && RP) {
        stop(signal);
        try { status('Staging test: the proposal at 0, 50 and 100% build-out…'); await RP.staging(P, { steps: STAGING_STEPS, onProgress: (k, n, t) => { stop(signal); status(`Staging test: ${t} (${k} of ${n})`); } }); }
        catch (e) { if (isAbort(e)) throw e; notes.push(`The staging test did not run: ${e.message}`); }
        stop(signal);
        try { status('Sensitivity tests: trip rates +10% and background growth +50%…'); await RP.sensitivity(P, { tests: SENS_TESTS, onProgress: (k, n, t) => { stop(signal); status(`Sensitivity tests: ${t} (${k} of ${n})`); } }); }
        catch (e) { if (isAbort(e)) throw e; notes.push(`The sensitivity tests did not run: ${e.message}`); }
      }
      if (CO && CO.recordOutputs) CO.recordOutputs(P);
      // the saved size (Store.serialise): the sample is built to stay under 3 MB; say so if a download made it larger
      const size = Store.serialise(P).length;
      (P.sample ||= {}).size = size;
      if (size > MAX_SIZE) notes.push(`The saved project is ${U.fmt.num(size / 1e6, 2)} MB, more than the sample's ${U.fmt.num(MAX_SIZE / 1e6, 0)} MB target. Save it to a file if browser storage refuses it.`);
      return { ms: Date.now() - t0, notes, failed, size };
    } finally { if (keep !== P) Store.P = keep; }
  }

  /* ---------- the ☰ Menu entry ---------- */
  let loading = false;
  // opts (for tests): { extras, gtfs } as for build and analyse; the menu passes none
  async function drury(opts = {}) {
    if (loading) return;
    loading = true;
    try { await druryRun(opts || {}); }
    finally { loading = false; }
  }
  async function druryRun(opts) {
    try { UI.stopSim(); } catch { }
    const old = Store.P, ac = typeof AbortController !== 'undefined' ? new AbortController() : null, t0 = Date.now();
    const busy = t => UI.busy(t, { cancel: ac ? () => ac.abort() : null });
    // While the sample is built the open project stays as it is: no redraws and no autosave of a half-built project
    const keep = { subs: Store.subs, save: Store.saveSoon };
    let r = null;
    try {
      try { if (keep.save && keep.save.flush) await keep.save.flush(); } catch { }
      Store.subs = [];
      Store.saveSoon = Object.assign(() => {}, { pending: () => false, flush: () => false, cancel: () => {} });
      busy('Downloading the Drury sample…');
      r = await build({ onStatus: busy, signal: ac && ac.signal, run: false, gtfs: opts.gtfs });
    } catch (e) {
      Store.subs = keep.subs; Store.saveSoon = keep.save;
      UI.busy(null);
      if (isAbort(e) || (Kit.isAbort && Kit.isAbort(e))) return UI.toast('Sample cancelled. Your project is unchanged.');
      return UI.toast(e.message || String(e), 'err');
    } finally { Store.subs = keep.subs; Store.saveSoon = keep.save; }
    const P = r.P;
    P.theme = old && old.theme;
    P.settings.basemap = old ? old.settings.basemap : '';
    P.settings.linzKey = old ? old.settings.linzKey : '';
    Store.setProject(P);
    try { UI.setColour('vc'); } catch { }
    let cancelled = false, size = null;
    try {
      const a = await analyse(P, {
        signal: ac && ac.signal, notes: r.notes, failed: r.failed, onStatus: busy, extras: opts.extras,
        runs: async scs => {
          const ok = await UI.runModel(scs, PERIODS);
          if (!ok || scs.some(sc => PERIODS.some(p => !sc.res[p]))) throw abortError();
          busy('Drury sample: next step…');
        },
      });
      size = a.size;
    } catch (e) {
      if (isAbort(e)) cancelled = true; else UI.toast(`The sample loaded, but a model step failed: ${e.message}. Run the model again from the top bar.`, 'warn');
    } finally { UI.busy(null); }
    if (Store.P === P) {
      P.active = (P.scenarios.find(s => s.role === 'proposal') || P.scenarios[0]).id;
      Store.emit('scenario');
    }
    try { UI.setColour('vc'); } catch { }
    try { UI.log(`<b>Drury sample</b><br>${r.notes.map(n => U.esc(n)).join('<br>')}`); } catch { }
    const miss = r.failed.map(f => f.label), secs = Math.round((Date.now() - t0) / 1000);
    UI.toast(`Drury sample loaded in ${secs} s: S0 ${BASE_YEAR} base year, S1 ${DESIGN_YEAR} do-minimum, S2 Option A and S3 Option B. Streets, counts, crashes and existing land use are real; the development, options and turning count are illustrative.`
      + (miss.length ? ` Missing (the services did not answer): ${miss.join(', ')}. Each scenario's notes say so; the Log tab lists what loaded.` : '')
      + (size > MAX_SIZE ? ` The project is ${U.fmt.num(size / 1e6, 1)} MB: save it to a file if browser storage refuses it.` : '')
      + (cancelled ? ' The model runs were cancelled: run the model again from the top bar.' : ''), miss.length || cancelled || size > MAX_SIZE ? 'warn' : '');
  }

  return { build, analyse, drury, turnCountCSV, BOUNDS, ILLUSTRATIVE, LODGEMENT, JUSTIFY, COMMITTED, PROPOSAL, TG };
})();
