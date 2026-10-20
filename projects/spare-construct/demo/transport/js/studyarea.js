'use strict';
/* Spare Construct · study area setup. One call builds a working model of an area in the CURRENT scenario:
   roads from OpenStreetMap → gateways → counts and crashes → existing land use zones (if the
   ExistingLU module is present) → gateway flows from counts → AM and PM runs → optional fit to
   counts (if the Calibrate module is present). Each step is independent: a failure (say, a map
   server that does not answer) is reported in warnings and the remaining steps still run.
   Counts and crashes are downloaded for the study box and the roads imported for it, and gateways go
   only on those roads, never on roads elsewhere in the scenario.
   Running setup again replaces the existing land use inside the new box instead of adding to it.
   With more than one scenario, new roads, gateways and zones go into every scenario (opts.all,
   on by default then), as the road import does, so scenarios stay comparable.
   Pure logic, no UI. Options default to on, except service lanes and fitting to counts. */

const StudyArea = (() => {
  const WARN_KM2 = 36, MAX_KM2 = 120, SAME_ZONE_M = 150, SRC = 'existing-lu';
  let running = false;

  const toBounds = b => b && typeof b.getSouth === 'function' ? b : L.latLngBounds(b);

  // Area of a bounding box in km²; warns above 36 km² (with a rough time) and refuses above 120 km²
  function estimate(bounds) {
    const b = toBounds(bounds), c = b.getCenter();
    const w = U.dist([c.lat, b.getWest()], [c.lat, b.getEast()]), h = U.dist([b.getSouth(), c.lng], [b.getNorth(), c.lng]);
    const a = w * h / 1e6, km2 = a < 1 ? Math.round(a * 100) / 100 : Math.round(a * 10) / 10;   // rounded for display only
    const mins = Math.max(1, Math.round(a / 12));
    let warn = null;
    if (!(a > 0)) warn = 'The study area has no size. Draw a box around the area you want to model.';
    else if (a > MAX_KM2) warn = `That area is ${U.fmt.num(a, 0)} km², too large to set up in one go (the limit is ${MAX_KM2} km²). Zoom in or draw a smaller study area.`;
    else if (a > WARN_KM2) warn = `That is a large area (${U.fmt.num(a, 0)} km²). Setting it up may take about ${mins} minute${mins === 1 ? '' : 's'}, and the free map servers may refuse it. Areas under ${WARN_KM2} km² work best.`;
    return { km2, warn, ok: a > 0 && a <= MAX_KM2, minutes: mins };
  }

  const inBox = (b, z) => isFinite(z.lat) && isFinite(z.lng) && b.contains([z.lat, z.lng]);
  // Zones a previous setup made from existing land use (older ones are recognised by their name)
  const fromLU = z => z.kind === 'zone' && z.existing === true && (z.src === SRC || /^Existing · /.test(String(z.name || '')));

  async function setup(bounds, opts = {}, onStatus = () => {}) {
    if (running) throw new Error('A study area setup is already running. Wait for it to finish.');
    if (!Store.P) throw new Error('Open or start a project first.');
    const b = toBounds(bounds), est = estimate(b);
    if (!est.ok) throw new Error(est.warn);
    const multi = Store.P.scenarios.length > 1;
    const o = { roads: true, local: true, service: false, gateways: true, counts: true, crashes: true, existing: true, run: true, fitCounts: false, all: multi, ...opts };
    const t0 = performance.now(), sc = Store.sc;
    sc.res ||= {}; sc.zones ||= {};
    const out = { roads: 0, gateways: 0, gwFromCounts: 0, counts: 0, crashes: 0, zones: 0, replaced: 0, dwellings: 0, ran: false, kpi: {}, fit: {}, calibration: null,
      scenarios: 1, warnings: est.warn ? [est.warn] : [], ms: 0 };
    const status = (t, f) => { try { onStatus(t, Math.max(0, Math.min(1, f))); } catch { } };
    // Runs one step; a failure becomes a warning and setup carries on
    const step = async (what, f, fn) => {
      status(what + '…', f);
      try { await fn(); }
      catch (e) { out.warnings.push(`${what} failed: ${e.message}`); }
    };
    const before = { n: new Set(Object.keys(sc.nodes)), l: new Set(Object.keys(sc.links)), z: new Set(Object.keys(sc.zones)) };
    const removed = [];
    let changed = false;
    running = true;
    Store.snapshot();
    try {
      /* 1 · roads */
      if (o.roads) await step('Downloading roads from OpenStreetMap', 0.02, async () => {
        const data = await Importers.overpass(b, { local: o.local, service: o.service }, t => status(t, 0.05));
        status('Building the network…', 0.2); await U.sleep(0);
        out.roads = Importers.osmToScenario(sc, data);
        if (out.roads) changed = true;
        if (!Object.keys(sc.links).length) out.warnings.push('No roads were found in this area. Check the box covers the roads you want to model.');
      });

      // This area's roads: the ones this setup imported, else those around the box (never roads elsewhere)
      const fresh = Object.keys(sc.nodes).filter(id => !before.n.has(id)), far = b.pad(2);
      const local = new Set(fresh.length ? fresh : Object.values(sc.nodes).filter(n => far.contains([n.lat, n.lng])).map(n => n.id));

      /* 2 · gateways where main roads leave the area */
      if (o.gateways && Object.keys(sc.links).length) await step('Adding gateways', 0.25, async () => {
        out.gateways = Importers.autoGateways(sc, b, local);
        if (out.gateways) changed = true;
      });

      /* 3 · counts and crashes for the study box */
      const which = [...(o.counts ? ['nzta', 'at'] : []), ...(o.crashes ? ['cas'] : [])];
      if (which.length) await step('Downloading counts and crashes', 0.3, async () => {
        const area = L.latLngBounds([[b.getSouth(), b.getWest()], [b.getNorth(), b.getEast()]]);
        for (const id of local) { const n = sc.nodes[id]; if (n) area.extend([n.lat, n.lng]); }
        const msgs = await DataHub.loadAll(area.pad(0.05), which, t => status(t, 0.35));
        const failed = which.filter(w => msgs.some(m => m.startsWith(w.toUpperCase() + ' failed')));
        for (const m of msgs) if (/ failed /.test(m)) out.warnings.push('Data download: ' + m);
        const d = DataHub.D();
        out.counts = d.counts.filter(c => which.includes(c.src) && !failed.includes(c.src)).length;
        out.crashes = which.includes('cas') && !failed.includes('cas') ? d.crashes.length : 0;
        Store.emit('data');
      });

      /* 4 · existing land use as zones: replaces what an earlier setup made inside this box */
      if (o.existing) {
        if (typeof ExistingLU === 'undefined') out.warnings.push('Existing land use is not available in this version, so no existing zones were added.');
        else await step('Downloading buildings and land use', 0.5, async () => {
          const els = await ExistingLU.fetch(b, t => status(t, 0.55));
          status('Grouping buildings into zones…', 0.65); await U.sleep(0);
          const built = ExistingLU.build(els, b, { links: sc.links }) || {};
          for (const [id, z] of Object.entries(sc.zones)) if (fromLU(z) && inBox(b, z)) { removed.push(id); delete sc.zones[id]; }
          out.replaced = removed.length;
          for (const z of built.zones || []) {
            if (!isFinite(z.lat) || !isFinite(z.lng)) continue;
            const nm = String(z.name || '').trim().toLowerCase();
            const dup = Object.values(sc.zones).some(y => y.kind !== 'gateway' && String(y.name || '').trim().toLowerCase() === nm && U.dist([y.lat, y.lng], [z.lat, z.lng]) < SAME_ZONE_M);
            if (dup) continue;
            const id = z.id && !sc.zones[z.id] ? z.id : U.uid('z');
            sc.zones[id] = { connectors: null, lu: {}, ...z, id, kind: 'zone', existing: true, src: SRC };
            out.zones++;
            out.dwellings += (+(z.lu || {}).residential || 0) + (+(z.lu || {}).apartment || 0);
          }
          out.dwellings = Math.round(out.dwellings);
          if (out.zones || removed.length) changed = true;
          const me = Store.P.me;
          if (removed.length && me && ['AM', 'PM'].some(p => me[p] && Object.keys(me[p]).some(k => removed.some(id => k.split('>').includes(id)))))
            out.warnings.push('Existing land use was replaced, so matrix estimation factors for the old zones no longer apply. Fit to counts again.');
        });
      }

      // Other scenarios get the same new roads, gateways and zones, and lose the replaced zones
      const others = o.all ? Store.P.scenarios.filter(s => s !== sc) : [];
      if (others.length && changed) {
        const clone = x => JSON.parse(JSON.stringify(x));
        for (const s of others) {
          s.zones ||= {};
          for (const id of removed) if (s.zones[id] && fromLU(s.zones[id])) delete s.zones[id];
          for (const id of Object.keys(sc.nodes)) if (!before.n.has(id) && !s.nodes[id]) s.nodes[id] = clone(sc.nodes[id]);
          for (const id of Object.keys(sc.links)) if (!before.l.has(id) && !s.links[id]) s.links[id] = clone(sc.links[id]);
          for (const id of Object.keys(sc.zones)) if (!before.z.has(id) && !s.zones[id]) s.zones[id] = clone(sc.zones[id]);
          for (const r of Object.values(s.res || {})) if (r) r.stale = true;
        }
        out.scenarios += others.length;
      } else if (multi && changed) out.warnings.push('Only the current scenario was set up; the other scenarios are unchanged.');
      // Count-to-road matches are cached by Store.ver, so tell the store about the new network first
      if (changed) Store.edited('net', sc);

      /* 5 · gateway flows from counts (every scenario that got the new network) */
      const hasGw = Object.values(sc.zones).some(z => z.kind === 'gateway');
      if (hasGw && DataHub.D().counts.length) await step('Setting gateway flows from counts', 0.7, async () => {
        out.gwFromCounts = DataHub.gatewaysFromCounts(sc);
        for (const s of others) DataHub.gatewaysFromCounts(s);
        if (out.gwFromCounts) { changed = true; Store.edited('demand', sc); }
      });

      /* 6 · run the model */
      const canRun = Object.keys(sc.links).length && Object.keys(sc.zones).length;
      if (o.run && !canRun) out.warnings.push(Object.keys(sc.links).length ? 'There are no zones or gateways, so the model was not run.' : 'There are no roads, so the model was not run.');
      // Fitting to counts runs the model itself, so the plain runs are only needed without it
      const fit = o.fitCounts && typeof Calibrate !== 'undefined' && DataHub.D().counts.length && canRun;
      if (o.run && canRun && !fit) {
        for (const [k, period] of ['AM', 'PM'].entries()) await step(`Running the ${period} peak`, 0.75 + k * 0.08, async () => {
          sc.res[period] = await Engine.assign(sc, { period, onProgress: (it, gap, msg) => status(`${period} peak · ${msg || 'iteration ' + it}`, 0.75 + k * 0.08 + 0.06) });
          out.ran = true;
        });
      }

      /* 7 · fit the model to counts */
      if (o.fitCounts) {
        if (typeof Calibrate === 'undefined') out.warnings.push('Fitting to counts is not available in this version, so the model was not calibrated.');
        else if (!DataHub.D().counts.length) out.warnings.push('No counts were loaded, so the model was not fitted to counts.');
        else if (canRun) {
          out.calibration = {};
          for (const [k, period] of ['AM', 'PM'].entries()) await step(`Fitting the ${period} peak to counts`, 0.85 + k * 0.07, async () => {
            const prev = sc.res[period];
            out.calibration[period] = await Calibrate.fit(sc, { period, onProgress: () => status(`Fitting the ${period} peak to counts…`, 0.87 + k * 0.07) });
            // keep the stored results in step with the fitted matrix
            if (o.run && (sc.res[period] === prev || !sc.res[period] || sc.res[period].stale)) sc.res[period] = await Engine.assign(sc, { period });
            out.ran = out.ran || !!sc.res[period];
          });
          if (!Object.keys(out.calibration).length) out.calibration = null;
        }
      }

      // Only current results go in the summary; out-of-date ones are flagged instead
      for (const period of ['AM', 'PM']) {
        const r = sc.res[period];
        if (!r || !r.kpi) continue;
        if (r.stale) { out.warnings.push(`The ${period} peak results are out of date (not run in this setup). Run the model to update them.`); continue; }
        out.kpi[period] = { trips: r.kpi.trips, spd: r.kpi.spd, over: r.kpi.over, vht: r.kpi.vht };
        const c = DataHub.calibration(sc, period);
        if (c && c.n) out.fit[period] = { n: c.n, pass: c.pass, r2: c.r2, ratio: c.totObs > 0 ? c.totMod / c.totObs : null };
      }
      if (out.ran) Store.emit('results');
    } finally {
      running = false;
    }
    out.ms = Math.round(performance.now() - t0);
    status('Study area ready', 1);
    return out;
  }

  return { setup, estimate, WARN_KM2, MAX_KM2 };
})();
