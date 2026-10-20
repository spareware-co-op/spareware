'use strict';
/* Spare Construct · real-world data hub. Loads NZTA state highway counts, Auckland Transport counts and
   NZTA crash records (all free, no key), plus live traffic from TomTom and traffic-aware travel
   times from Google with the user's own keys. Counts are matched to model links for calibration
   and can set gateway flows. Calibration statistics report the slope through the origin with both R²
   definitions beside OLS; crash records cover five full years plus the year to date. In Auckland it also loads AT
   Future Connect classes (conflated onto links as L.fc for the extent test) and AT controlled intersections (for
   junction sites). Every ArcGIS request has a 30 s time limit and fails soft with a message; a user cancel (an aborted
   opts.signal, or Kit.task's cancel) is passed on as an AbortError, not reported as a slow server.
   Integration pass (Oct 2026):
   - periodVol for NZTA counts: the TMS Tue–Thu school-term mean (c.tms.target.mean, from the feeds lane) when it is
     there and not flagged indicative (target.ok false), else the AADT; × the site's own TMS peak share (c.share[period],
     a fraction of daily) when present, else the Settings share.
   - Count kind 'ramm_estimate' (NZTA RCA centreline AADT, docs/spec/local-road-counts): weight 0.25 in matrix
     estimation, never in validation statistics, and never used within 200 m of an NZTA or AT count (in use).
     countWeight(c, period) gives the observation-method weights (count-roles): measured hour 1.0, weekday factored 0.7,
     ADT factored 0.4, RAMM estimate 0.25, virtual 0.
   - Crash tuples keep the extra CAS fields at fixed positions: [7] fatalCount, [8] seriousInjuryCount,
     [9] minorInjuryCount, [10] light, [11] weatherA, [12] OBJECTID, [13] trafficControl, [14] urban (trailing empty
     fields are left off; readers treat a missing position as unknown). loadAll(b, ['cas10']) also downloads a 10-year
     window (D().crashesLong, loaded.casLong) for state highway accesses, without touching the 5-year tables. */

const DataHub = (() => {
  const NZTA_SITES = 'https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/Assets_SHTrafficMonitoringSites/FeatureServer/0';
  const NZTA_CAS = 'https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services/CAS_Data_Public/FeatureServer/0';
  const AT_COUNTS = 'https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/TrafficService/FeatureServer/0';
  const SOURCES = {
    nzta: { label: 'NZTA state highway counts', short: 'NZTA', colour: '#7048e8' },
    at: { label: 'Auckland Transport counts', short: 'AT', colour: '#0c8599' },
    user: { label: 'Your counts', short: 'Yours', colour: '#e8590c' },
    ramm: { label: 'NZTA road centreline estimates (RAMM)', short: 'RAMM', colour: '#868e96' },
  };
  /* Count observation methods and their matrix-estimation weights (docs/spec/count-roles: Spare Construct suggestion;
     ramm_estimate from docs/spec/local-road-counts). validation: false = never in validation statistics. */
  const OBS_METHODS = {
    measured_hour: { label: 'Measured peak hour', weight: 1.0, validation: true },
    weekday_factored: { label: 'TMS weekday mean × peak share', weight: 0.7, validation: true },
    adt_factored: { label: 'ADT or AADT × peak share', weight: 0.4, validation: true },
    ramm_estimate: { label: 'RAMM centreline estimate (NZTA RCA data)', weight: 0.25, validation: false, overrideM: 200 },
    virtual: { label: 'NZTA estimate (virtual site)', weight: 0, validation: false },
    source: 'docs/spec/count-roles (weights: Spare Construct suggestion); docs/spec/local-road-counts (RAMM estimates)', verified: false,
  };
  const isEstimate = c => !!c && (c.obs_method === 'ramm_estimate' || c.kind === 'ramm_estimate' || c.src === 'ramm');
  const DIR_BEARING = { NB: 0, EB: 90, SB: 180, WB: 270 };

  /* ---------- per-browser API keys (never saved in project files) ---------- */
  const keys = (() => { try { return JSON.parse(localStorage.getItem('corridor.keys') || '{}'); } catch { return {}; } })();
  function setKey(k, v) { keys[k] = (v || '').trim(); try { localStorage.setItem('corridor.keys', JSON.stringify(keys)); } catch { } }

  /* ---------- project data store ---------- */
  function D() {
    const P = Store.P;
    P.data ||= {};
    const d = P.data;
    d.counts ||= []; d.crashes ||= []; d.loaded ||= {}; d.jt ||= [];
    d.use = { nzta: true, at: true, user: true, virtual: false, ramm: true, ...(d.use || {}) };
    return d;
  }

  /* ---------- ArcGIS feature service query, paged, WGS84 ---------- */
  // Each page has its own time limit (AbortController); a slow server fails with a message that says so. signal
  // (optional) is the caller's cancel: when it aborts, the AbortError itself is thrown so callers can tell a user
  // cancel from a failure (Kit.task's fetch hook cancels the same way).
  const ARCGIS_TIMEOUT = 30000;
  const abortError = () => Object.assign(new Error('Cancelled'), { name: 'AbortError', cancelled: true });
  // Loader request options: an AbortSignal, or { signal, timeout (ms) }
  const reqOf = o => (o && typeof o === 'object' && !('aborted' in o) ? { signal: o.signal || null, timeout: +o.timeout > 0 ? +o.timeout : ARCGIS_TIMEOUT } : { signal: o || null, timeout: ARCGIS_TIMEOUT });
  async function arcgis(url, b, where = '1=1', fields = '*', timeout = ARCGIS_TIMEOUT, signal = null) {
    const out = [], host = new URL(url).host;
    for (let page = 0, offset = 0; page < 25; page++) {
      if (signal && signal.aborted) throw abortError();
      const q = new URLSearchParams({
        where, geometry: [b.getWest(), b.getSouth(), b.getEast(), b.getNorth()].map(v => v.toFixed(6)).join(','),
        geometryType: 'esriGeometryEnvelope', inSR: '4326', spatialRel: 'esriSpatialRelIntersects',
        outFields: fields, outSR: '4326', f: 'json', resultOffset: String(offset), orderByFields: 'OBJECTID',
      });
      const ac = typeof AbortController !== 'undefined' ? new AbortController() : null;
      let timedOut = false;
      const tm = ac ? setTimeout(() => { timedOut = true; ac.abort(); }, timeout) : null;
      const onAbort = () => { if (ac) ac.abort(); };
      if (signal && signal.addEventListener) signal.addEventListener('abort', onAbort, { once: true });
      let r, j;
      try {
        try { r = await fetch(url + '/query?' + q, ac ? { signal: ac.signal } : undefined); }
        catch (e) {
          if (e && e.name === 'AbortError' && !timedOut) throw e;   // the user cancelled (our signal or Kit.task's)
          throw new Error(e && e.name === 'AbortError' ? `${host} did not answer within ${Math.round(timeout / 1000)} s. Try again later` : `${host} could not be reached (${(e && e.message) || e})`);
        }
        if (!r.ok) throw new Error(`${host} answered HTTP ${r.status}`);
        try { j = await r.json(); }
        catch (e) { if (e && e.name === 'AbortError' && !timedOut) throw e; throw new Error(e && e.name === 'AbortError' ? `${host} did not answer within ${Math.round(timeout / 1000)} s. Try again later` : `${host} sent an answer Spare Construct could not read`); }
      } finally {
        if (tm) clearTimeout(tm);
        if (signal && signal.removeEventListener) signal.removeEventListener('abort', onAbort);
      }
      if (!j || typeof j !== 'object') throw new Error(`${host} sent an answer Spare Construct could not read`);
      if (j.error) throw new Error(j.error.message || 'query failed');
      const f = j.features || [];
      out.push(...f);
      if (!j.exceededTransferLimit || !f.length) break;
      offset += f.length;
    }
    return out;
  }

  const title = s => String(s || '').toLowerCase().replace(/\b[a-z]/g, c => c.toUpperCase());
  const norm = s => String(s || '').toLowerCase().replace(/\(.*?\)/g, ' ')
    .replace(/\brd\b/g, 'road').replace(/\bst\b/g, 'street').replace(/\baves?\b/g, 'avenue').replace(/\bdr\b/g, 'drive')
    .replace(/\bpl\b/g, 'place').replace(/\bcres\b/g, 'crescent').replace(/\bhwy\b/g, 'highway').replace(/\bpde\b/g, 'parade')
    .replace(/\btce\b/g, 'terrace').replace(/\bln\b/g, 'lane').replace(/\bmwy\b/g, 'motorway').replace(/\bsh\s*(\d+)/g, 'sh$1')
    .replace(/[^a-z0-9 ]/g, ' ').replace(/\s+/g, ' ').trim();
  const dirOf = s => { const m = /\b(NB|SB|EB|WB)\b/.exec(String(s || '').toUpperCase()); return m ? m[1] : null; };

  /* ---------- loaders ---------- */
  async function loadNZTA(b, req = null) {
    const q = reqOf(req), f = await arcgis(NZTA_SITES, b, '1=1', 'OBJECTID,siteref,description,lane,sh,sitetype,type,percentheavy,aadt1yearago,aadt2yearsago,aadt3yearsago,aadt4yearsago,aadt5yearsago,accepteddays', q.timeout, q.signal);
    return f.filter(x => x.geometry).map(x => {
      const a = x.attributes, hist = [a.aadt5yearsago, a.aadt4yearsago, a.aadt3yearsago, a.aadt2yearsago, a.aadt1yearago];
      return {
        id: 'nzta:' + a.siteref, src: 'nzta', ref: a.siteref, name: a.description || ('SH' + a.sh + ' site ' + a.siteref), road: 'SH' + (a.sh || '').replace(/[NS]$/, ''),
        lat: U.r7(x.geometry.y), lng: U.r7(x.geometry.x), adt: a.aadt1yearago || a.aadt2yearsago || null, hv: a.percentheavy,
        dir: dirOf(a.description), directional: true, virtual: a.sitetype === 'Virtual' || a.type === 'Virtual', ramp: /\bramp\b/i.test(a.description || ''),
        hist, note: a.sitetype === 'Virtual' ? 'NZTA estimate (virtual site), not a physical count' : `${a.sitetype || 'Count'} site${a.accepteddays ? `, ${a.accepteddays} counted days last year` : ''}`,
      };
    }).filter(c => c.adt > 0);
  }
  async function loadAT(b, req = null) {
    const q = reqOf(req), f = await arcgis(AT_COUNTS, b, "latest='Yes'", 'OBJECTID,road_name,start_name,end_name,count_date,peak_hour,count_duration,adt,peaktraffic,pcheavy,carr_way_no', q.timeout, q.signal);
    // AT also lists state highway sections; NZTA's own directional counts cover those
    return f.filter(x => x.geometry && !/^SH\s*\d/i.test(x.attributes.road_name || '')).map(x => {
      const a = x.attributes, ph = String(a.peak_hour || '').padStart(4, '0');
      return {
        id: 'at:' + a.OBJECTID, src: 'at', ref: String(a.carr_way_no || a.OBJECTID), name: `${title(a.road_name)} · ${title(a.start_name)} to ${title(a.end_name)}`,
        road: a.road_name, lat: U.r7(x.geometry.y), lng: U.r7(x.geometry.x), adt: a.adt, peak: a.peaktraffic,
        peakTime: ph.slice(0, 2) + ':' + ph.slice(2), hv: a.pcheavy, date: a.count_date ? new Date(a.count_date).toISOString().slice(0, 10) : null,
        dur: a.count_duration, directional: false, note: `${a.count_duration || ''} count${a.count_date ? ', ' + new Date(a.count_date).toLocaleDateString('en-NZ', { month: 'short', year: 'numeric' }) : ''}`,
      };
    }).filter(c => c.adt > 0);
  }
  /* CAS query window (docs/spec/calc-corrections (2), crash-history): the last `years` FULL calendar years
     (Y−5 to Y−1, Y = the current year in New Zealand) for the main tables, plus year Y to date, labelled
     'YTD, incomplete'. Before 1 April year Y−1 is flagged as possibly incomplete (CAS entry lag of up to
     three months, MBCM). 10 years is an option for low-volume roads (AADT < 1,500 veh/day) and for
     pedestrian and cycle analysis. Deviation from the spec: one query (crashYear >= Y−5) fetches the full
     years and the year to date, told apart by crashYear, instead of a second year-to-date query. */
  const CAS_WINDOW = { years: 5, longYears: 10, lagMonths: 3, ytdLabel: 'YTD, incomplete', source: 'NZTA CAS; MBCM v1.7.5 crash analysis period', verified: false };
  function nzDate(now = new Date()) {
    try {
      const p = Object.fromEntries(new Intl.DateTimeFormat('en-NZ', { timeZone: 'Pacific/Auckland', year: 'numeric', month: 'numeric', day: 'numeric' }).formatToParts(now).map(x => [x.type, x.value]));
      if (+p.year > 0) return { y: +p.year, m: +p.month, d: +p.day };
    } catch { }
    return { y: now.getFullYear(), m: now.getMonth() + 1, d: now.getDate() };
  }
  function casWindow(now = new Date(), years = CAS_WINDOW.years) {
    const t = nzDate(now), Y = t.y, n = Math.max(1, Math.round(+years || CAS_WINDOW.years)), from = Y - n, to = Y - 1;
    const lagYear = t.m < 4 ? to : null;
    return { from, to, ytd: Y, years: n, lagYear, ytdLabel: CAS_WINDOW.ytdLabel,
      lagNote: lagYear ? `${lagYear} may be incomplete (CAS entry lag up to ${CAS_WINDOW.lagMonths} months, MBCM)` : '',
      label: `${from}–${to} (${n} full years), plus ${Y} to date (${CAS_WINDOW.ytdLabel})`,
      where: `crashYear>=${from}` };
  }
  /* A crash tuple: [0] lat, [1] lng, [2] severity F|S|M|N, [3] year, [4] location, [5] flags p|c|m, [6] speed limit, then
     the fields crash-history asks for at fixed positions (CAS_FIELDS): [7] fatalCount, [8] seriousInjuryCount,
     [9] minorInjuryCount, [10] light, [11] weatherA, [12] OBJECTID, [13] trafficControl, [14] urban. Counts are numbers,
     the others text; an unknown value is null, and trailing nulls are left off to keep project files small. */
  const CAS_FIELDS = ['fatalCount', 'seriousInjuryCount', 'minorInjuryCount', 'light', 'weatherA', 'OBJECTID', 'trafficControl', 'urban'];
  const CAS_POS = Object.fromEntries(CAS_FIELDS.map((k, i) => [k, 7 + i]));
  const CAS_OUT = 'OBJECTID,crashSeverity,crashYear,crashLocation1,crashLocation2,speedLimit,pedestrian,bicycle,motorcycle,fatalCount,seriousInjuryCount,minorInjuryCount,light,weatherA,trafficControl,urban';
  function casTuple(x) {
    const a = x.attributes || {}, s = String(a.crashSeverity || 'N')[0].toUpperCase();
    const t = [U.r7(x.geometry.y), U.r7(x.geometry.x), 'FSMN'.includes(s) ? s : 'N', a.crashYear, [a.crashLocation1, a.crashLocation2].filter(Boolean).map(title).join(' / '), (a.pedestrian ? 'p' : '') + (a.bicycle ? 'c' : '') + (a.motorcycle ? 'm' : ''), a.speedLimit];
    const num = v => (v == null || v === '' || !isFinite(+v) ? null : +v), txt = v => (v == null || v === '' ? null : String(v));
    const extra = CAS_FIELDS.map(k => (/Count$|^OBJECTID$/.test(k) ? num(a[k]) : txt(a[k])));
    while (extra.length && extra[extra.length - 1] == null) extra.pop();
    return extra.length ? t.concat(extra) : t;
  }
  async function loadCAS(b, years = CAS_WINDOW.years, now = new Date(), req = null) {
    const w = casWindow(now, years), q = reqOf(req);
    const f = await arcgis(NZTA_CAS, b, w.where, CAS_OUT, q.timeout, q.signal);
    return f.filter(x => x.geometry).map(casTuple);
  }

  /* ---------- AT Future Connect classes and controlled intersections (Auckland only) ----------
     Future Connect (docs/spec/extent-and-difference): general traffic, public transport, freight and cycle
     classes, now and for the first decade. Deviation from the spec: queried as f=json through the same paged
     helper as the other ArcGIS layers (identical attributes and geometry; f=geojson is not needed). */
  const AT_FC = 'https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/FutureConnect_IntegratedNetwork/FeatureServer/0';
  const AT_CI = 'https://services2.arcgis.com/JkPEgZJGxhSjYOo0/arcgis/rest/services/RoadingService/FeatureServer/2';
  const AT_LICENCE = 'Auckland Transport open data, CC BY 4.0 as stated on the AT open data portal (confirm before publishing)';
  const FC_FIELDS = 'OBJECTID,street_name,generaltraffic_current,publictransport_current,freight_current,cycle_current,generaltraffic_first_decade,publictransport_first_decade,freight_first_decade,cycle_first_decade';
  async function loadFutureConnect(b, req = null) {
    const q = reqOf(req), f = await arcgis(AT_FC, b, '1=1', FC_FIELDS, q.timeout, q.signal);
    return f.filter(x => x.geometry && Array.isArray(x.geometry.paths)).map(x => {
      const a = x.attributes || {};
      return { id: a.OBJECTID, name: a.street_name || '', gt: a.generaltraffic_current || null, pt: a.publictransport_current || null, freight: a.freight_current || null, cycle: a.cycle_current || null,
        gt10: a.generaltraffic_first_decade || null, pt10: a.publictransport_first_decade || null, freight10: a.freight_first_decade || null, cycle10: a.cycle_first_decade || null,
        paths: x.geometry.paths.map(p => p.map(q => [U.r7(q[1]), U.r7(q[0])])) };
    });
  }
  async function loadControlled(b, req = null) {
    const q = reqOf(req), f = await arcgis(AT_CI, b, '1=1', 'OBJECTID,intersection_id,intersection_no,intersection_desc,intersection_type,controlled', q.timeout, q.signal);
    return f.filter(x => x.geometry).map(x => {
      const a = x.attributes || {};
      return { id: a.intersection_id ?? a.OBJECTID, no: a.intersection_no ?? null, name: String(a.intersection_desc || '').replace(/^\(\d+\)\s*/, ''), type: a.intersection_type || '', controlled: a.controlled === 'Yes',
        lat: U.r7(x.geometry.y), lng: U.r7(x.geometry.x) };
    });
  }
  /* Conflation of Future Connect to model links: a link takes the classes of the section whose polyline lies within
     radius (15 m) of at least two of three points along the link (¼, ½, ¾), with the section's bearing there within
     30° of the link's (either direction), and whose street name matches the link's where both have one.
     Rail-only sections are skipped. A class the user set (L.fc.user) is never replaced. */
  function conflateFC(sc, feats, opts = {}) {
    const R = opts.radius || 15, B = opts.bearing || 30, CELL_M = 60, now = new Date().toISOString().slice(0, 10);
    const use = (feats || []).filter(f => f.gt || f.pt || f.gt10 || f.pt10 || (f.freight && f.freight !== 'Rail'));
    const lat0 = Object.values(sc.nodes)[0] ? Object.values(sc.nodes)[0].lat : -36.85, F = U.flat(lat0), grid = new Map(), segs = [];
    use.forEach((f, fi) => {
      for (const p of f.paths) for (let i = 0; i < p.length - 1; i++) {
        const ax = F.x(p[i]), ay = F.y(p[i]), bx = F.x(p[i + 1]), by = F.y(p[i + 1]), k = segs.length;
        segs.push([ax, ay, bx, by, fi, U.bearing(p[i], p[i + 1])]);
        for (let cx = Math.floor((Math.min(ax, bx) - R) / CELL_M); cx <= Math.floor((Math.max(ax, bx) + R) / CELL_M); cx++)
          for (let cy = Math.floor((Math.min(ay, by) - R) / CELL_M); cy <= Math.floor((Math.max(ay, by) + R) / CELL_M); cy++) {
            const key = cx * 1e6 + cy, a = grid.get(key); if (a) a.push(k); else grid.set(key, [k]);
          }
      }
    });
    const nameOK = (a, b) => { const x = norm(a), y = norm(b); return !x || !y || x === y || x.includes(y) || y.includes(x); };
    let matched = 0, links = 0, art = 0, artM = 0;
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed' || !sc.nodes[L.a] || !sc.nodes[L.b]) continue;
      if (L.fc && L.fc.user) continue;
      links++; if (L.cls === 'arterial') art++;
      const g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1], votes = new Map();
      for (const fr of [0.25, 0.5, 0.75]) {
        const at = U.along(g, cum, len * fr), b0 = U.bearing(U.along(g, cum, Math.max(0, len * fr - 5)).p, U.along(g, cum, Math.min(len, len * fr + 5)).p);
        const px = F.x(at.p), py = F.y(at.p), best = new Map();
        for (const k of grid.get(Math.floor(px / CELL_M) * 1e6 + Math.floor(py / CELL_M)) || []) {
          const q = segs[k], d = U.segDist(px, py, q[0], q[1], q[2], q[3]).d, ang = U.angDiff(b0, q[5]);
          if (d > R || Math.min(ang, 180 - ang) > B) continue;
          if (!(best.get(q[4]) <= d)) best.set(q[4], d);
        }
        for (const [fi, d] of best) { const v = votes.get(fi) || { n: 0, d: 0 }; v.n++; v.d += d; votes.set(fi, v); }
      }
      let pick = null;
      for (const [fi, v] of votes) if (v.n >= 2 && nameOK(L.name, use[fi].name) && (!pick || v.n > pick.v.n || (v.n === pick.v.n && v.d < pick.v.d))) pick = { fi, v };
      if (!pick) { if (L.fc && L.fc.src === 'AT Future Connect') delete L.fc; continue; }
      const f = use[pick.fi];
      L.fc = { gt: f.gt, pt: f.pt, freight: f.freight, cycle: f.cycle, gt10: f.gt10, pt10: f.pt10, freight10: f.freight10, cycle10: f.cycle10,
        src: 'AT Future Connect', fid: f.id, d: U.r1(pick.v.d / pick.v.n), at: now };
      matched++; if (L.cls === 'arterial') artM++;
    }
    /* Short pieces (under 60 m) at junctions often miss: divided carriageways sit off the centreline and bend into
       the junction, and roundabout ring links are a few metres long. They take the classes of an adjacent matched
       link of the same road; a ring link takes the highest general-traffic class of the roads meeting the ring. */
    const GT_ORDER = ['motorway', 'strategic', 'primary', 'secondary', 'collector', 'local'], gtRank = g => { const k = GT_ORDER.findIndex(x => norm(g).startsWith(x)); return k < 0 ? 9 : k; };
    const at = new Map();
    for (const L of Object.values(sc.links)) if (L.status !== 'closed') for (const n of [L.a, L.b]) { const a = at.get(n); if (a) a.push(L); else at.set(n, [L]); }
    for (let pass = 0; pass < 3; pass++) for (const L of Object.values(sc.links)) {
      if (L.fc || L.status === 'closed' || !sc.nodes[L.a] || !sc.nodes[L.b] || Net.len(sc, L) >= 60) continue;
      const adj = [...(at.get(L.a) || []), ...(at.get(L.b) || [])].filter(M => M !== L && M.fc && /^AT Future Connect/.test(M.fc.src || ''));
      const pool = L.ring ? adj.filter(M => !M.ring || M.fc.src === 'AT Future Connect') : adj.filter(M => norm(M.name) && norm(M.name) === norm(L.name));
      if (!pool.length) continue;
      const M = pool.slice().sort((p, q) => gtRank(p.fc.gt) - gtRank(q.fc.gt))[0];
      L.fc = { ...M.fc, src: 'AT Future Connect (from the adjacent link)', from: M.id };
      matched++; if (L.cls === 'arterial') artM++;
    }
    return { matched, links, arterials: art, arterialsMatched: artM, share: links ? matched / links : 0, arterialShare: art ? artM / art : null };
  }

  // The area to fetch for: the network's extent if there is one, otherwise the map view
  function area(map) {
    const sc = Store.sc, pts = Object.values(sc.nodes).map(n => [n.lat, n.lng]);
    const b = pts.length ? L.latLngBounds(pts) : map.getBounds();
    return b.pad(0.05);
  }

  /* opts.casYears: full years of crashes, 5 (default) or 10; opts.signal: an AbortSignal that cancels the whole load
     (the AbortError is thrown, not listed as a failed source); opts.timeout: the time limit per request (ms, 30 s). 'cas10': a separate 10-year window (CAS_WINDOW.longYears)
     for state highway accesses (NZTA asks for longer histories there; docs/spec/calc-corrections (2) option), kept in
     D().crashesLong and loaded.casLong so the five-year tables and risk bands are unchanged. Pass bounds around the
     accesses to keep the download small. */
  async function loadAll(b, which = ['nzta', 'at', 'cas'], status = () => {}, opts = {}) {
    const d = D(), msgs = [], signal = opts.signal || null, req = { signal, timeout: opts.timeout };
    for (const w of which) {
      if (signal && signal.aborted) throw abortError();
      try {
        if (w === 'cas') {
          status('Downloading crash records (NZTA CAS)…');
          const cw = casWindow(new Date(), opts.casYears);
          d.crashes = await loadCAS(b, cw.years, new Date(), req);
          // The window loadCAS asks for: full years from–to, then the year to date (ytd, incomplete)
          d.loaded.cas = { at: Date.now(), n: d.crashes.length, from: cw.from, to: cw.to, ytd: cw.ytd, years: cw.years,
            nYTD: d.crashes.filter(c => c[3] === cw.ytd).length, lagYear: cw.lagYear, label: cw.label, fields: CAS_FIELDS.slice() };
          msgs.push(`${U.fmt.int(d.crashes.length)} crashes`);
        } else if (w === 'cas10') {
          status('Downloading 10 years of crash records for state highway accesses (NZTA CAS)…');
          const cw = casWindow(new Date(), CAS_WINDOW.longYears);
          d.crashesLong = await loadCAS(b, cw.years, new Date(), req);
          d.loaded.casLong = { at: Date.now(), n: d.crashesLong.length, from: cw.from, to: cw.to, ytd: cw.ytd, years: cw.years, lagYear: cw.lagYear, label: cw.label,
            purpose: 'state highway accesses', bounds: [[U.r7(b.getSouth()), U.r7(b.getWest())], [U.r7(b.getNorth()), U.r7(b.getEast())]], fields: CAS_FIELDS.slice() };
          msgs.push(`${U.fmt.int(d.crashesLong.length)} crashes over ${cw.years} years (state highway accesses)`);
        } else if (w === 'fc') {
          status('Downloading AT Future Connect road classes…');
          d.fc = await loadFutureConnect(b, req);
          const m = conflateFC(Store.sc, d.fc);
          d.loaded.fc = { at: Date.now(), n: d.fc.length, source: 'AT Future Connect (integrated network)', url: AT_FC, licence: AT_LICENCE, matched: m.matched, links: m.links, arterialShare: m.arterialShare };
          msgs.push(`${d.fc.length} Future Connect sections (${m.matched} of ${m.links} links classified)`);
        } else if (w === 'controlled') {
          status('Downloading AT controlled intersections…');
          d.controlled = await loadControlled(b, req);
          d.loaded.controlled = { at: Date.now(), n: d.controlled.length, source: 'AT controlled intersections (RoadingService layer 2)', url: AT_CI, licence: AT_LICENCE };
          msgs.push(`${d.controlled.length} AT controlled intersections`);
        } else if (w !== 'nzta' && w !== 'at') {
          msgs.push(`${String(w).toUpperCase()} is not a source Spare Construct downloads here`);
        } else {
          status(`Downloading ${SOURCES[w].label}…`);
          const list = w === 'nzta' ? await loadNZTA(b, req) : await loadAT(b, req);
          const keep = new Map(d.counts.filter(c => c.src === w).map(c => [c.id, c]));
          d.counts = d.counts.filter(c => c.src !== w).concat(list.map(c => ({ ...c, off: keep.get(c.id) ? keep.get(c.id).off : false })));
          d.loaded[w] = { at: Date.now(), n: list.length };
          msgs.push(`${list.length} ${SOURCES[w].short} count sites`);
        }
      } catch (e) {
        // A cancel stops the whole load; anything else fails soft for that source only
        if ((e && e.name === 'AbortError') || (signal && signal.aborted)) throw e && e.name === 'AbortError' ? e : abortError();
        msgs.push(`${w.toUpperCase()} failed (${e.message})`);
      }
    }
    return msgs;
  }

  /* ---------- your own counts (CSV) ---------- */
  function importCSV(text) {
    const rows = U.parseCSV(text); if (rows.length < 2) throw new Error('The file has no data rows.');
    const h = rows[0].map(s => s.trim().toLowerCase());
    const col = (...names) => h.findIndex(x => names.includes(x));
    const iLat = col('lat', 'latitude', 'y'), iLng = col('lng', 'lon', 'long', 'longitude', 'x'), iName = col('name', 'site', 'location', 'road');
    const iAM = col('am', 'am_peak', 'am peak', 'am_vph'), iPM = col('pm', 'pm_peak', 'pm peak', 'pm_vph'), iADT = col('adt', 'aadt', 'daily'), iDir = col('dir', 'direction');
    if (iLat < 0 || iLng < 0) throw new Error('Columns lat and lng are required.');
    if (iAM < 0 && iPM < 0 && iADT < 0) throw new Error('Add at least one of: am, pm, adt.');
    const d = D(), list = [];
    rows.slice(1).forEach((r, k) => {
      const lat = +r[iLat], lng = +r[iLng]; if (!isFinite(lat) || !isFinite(lng)) return;
      const dir = iDir >= 0 ? dirOf(r[iDir]) : null;
      list.push({ id: 'user:' + k + ':' + U.uid(), src: 'user', name: iName >= 0 ? r[iName] : `Count ${k + 1}`, lat, lng, adt: iADT >= 0 ? +r[iADT] || null : null,
        am: iAM >= 0 ? +r[iAM] || null : null, pm: iPM >= 0 ? +r[iPM] || null : null, dir, directional: !!dir, note: 'Imported from your file' });
    });
    d.counts = d.counts.filter(c => c.src !== 'user').concat(list);
    d.loaded.user = { at: Date.now(), n: list.length };
    return list.length;
  }

  /* ---------- turning a count into a modelled-period volume (veh/h) ---------- */
  /* NZTA counts: the daily volume is the TMS Tue–Thu school-term mean (c.tms.target, docs/spec/tms-daily) when the feeds
     lane has attached one that is not flagged indicative (ok false: fewer term weeks than wanted), else the AADT; the
     peak share is the site's own from the TMS peak profiles (c.share[period], a fraction of daily) when present, else
     the Settings share. basis: the observation method the volume rests on (count-roles). */
  function periodVol(c, period) {
    const S = Store.S, sk = period === 'AM' ? 'amShare' : 'pmShare', share = S.data[sk] / 100, shareTxt = `${S.data[sk]}% of daily`;
    if (c.src === 'user') {
      const v = period === 'AM' ? c.am : c.pm;
      if (v) return { v, how: 'your count', basis: 'measured_hour' };
      return c.adt ? { v: c.adt * share, how: shareTxt, basis: 'adt_factored' } : null;
    }
    if (c.src === 'at' && c.peak && c.peakTime) {
      const h = +c.peakTime.slice(0, 2) + (+c.peakTime.slice(3)) / 60;
      if ((period === 'AM' && h >= 6 && h < 10) || (period === 'PM' && h >= 14.5 && h < 19)) return { v: c.peak, how: `counted peak hour (${c.peakTime})`, basis: 'measured_hour' };
    }
    if (isEstimate(c)) return c.adt > 0 ? { v: c.adt * share, how: `RAMM estimate × ${shareTxt}`, basis: 'ramm_estimate' } : null;
    if (c.src === 'nzta') {
      const t = c.tms && typeof c.tms === 'object' ? c.tms.target : null, tm = t && +t.mean > 0 && isFinite(+t.mean) && t.ok !== false ? +t.mean : null;
      const ps = c.share && typeof c.share === 'object' ? +c.share[period] : NaN, own = ps > 0 && ps < 1 ? ps : null;
      const daily = tm != null ? tm : c.adt;
      if (!(daily > 0)) return null;
      if (tm == null && own == null) return { v: daily * share, how: shareTxt, basis: c.virtual ? 'virtual' : 'adt_factored' };
      return { v: daily * (own != null ? own : share), how: `${tm != null ? `TMS Tue–Thu term-time mean (${U.fmt.int(tm)}/day)` : 'AADT'} × ${own != null ? `${U.fmt.num(own * 100, 1)}% (TMS peak profile)` : shareTxt}`,
        basis: c.virtual ? 'virtual' : tm != null ? 'weekday_factored' : 'adt_factored' };
    }
    return c.adt ? { v: c.adt * share, how: shareTxt, basis: c.virtual ? 'virtual' : 'adt_factored' } : null;
  }
  // The observation method of a count in a period (count-roles): an importer's own c.obs_method wins
  function obsMethod(c, period) {
    if (!c) return null;
    if (typeof c.obs_method === 'string' && OBS_METHODS[c.obs_method]) return c.obs_method;
    if (isEstimate(c)) return 'ramm_estimate';
    if (c.virtual) return 'virtual';
    const pv = periodVol(c, period);
    return (pv && pv.basis) || 'adt_factored';
  }
  // Matrix-estimation weight of a count by its observation method (measured 1, weekday 0.7, ADT 0.4, RAMM 0.25, virtual 0)
  function countWeight(c, period) { const m = OBS_METHODS[obsMethod(c, period)]; return m ? m.weight : 0.4; }
  /* RAMM estimates never override an NZTA or AT count: an estimate within 200 m of a count from those sources (not
     switched off) is not used. Cached on the counts list. */
  let shadowCache = { key: '', set: new Set() };
  function shadowed(c) {
    if (!isEstimate(c)) return false;
    const d = D(), key = `${d.counts.length}|${(d.loaded.nzta || {}).at}|${(d.loaded.at || {}).at}|${d.counts.filter(x => x.off).length}`;
    if (shadowCache.key !== key) {
      const real = d.counts.filter(x => (x.src === 'nzta' || x.src === 'at') && !x.off && !x.virtual && !isEstimate(x) && isFinite(x.lat) && isFinite(x.lng)), R = OBS_METHODS.ramm_estimate.overrideM, set = new Set();
      for (const e of d.counts) if (isEstimate(e) && real.some(x => Math.abs(x.lat - e.lat) < 0.003 && Math.abs(x.lng - e.lng) < 0.004 && U.dist([x.lat, x.lng], [e.lat, e.lng]) <= R)) set.add(e.id);
      shadowCache = { key, set };
    }
    return shadowCache.set.has(c.id);
  }

  /* ---------- matching counts to model links ---------- */
  function match(sc, counts = D().counts) {
    const S = Store.S, R = S.data.matchRadius || 40;
    const items = [];
    for (const Lk of Object.values(sc.links)) {
      if (Lk.status === 'closed' || !sc.nodes[Lk.a] || !sc.nodes[Lk.b]) continue;
      const g = Net.geom(sc, Lk);
      let la0 = Infinity, la1 = -Infinity, ln0 = Infinity, ln1 = -Infinity;
      for (const p of g) { la0 = Math.min(la0, p[0]); la1 = Math.max(la1, p[0]); ln0 = Math.min(ln0, p[1]); ln1 = Math.max(ln1, p[1]); }
      items.push({ Lk, g, la0, la1, ln0, ln1, nm: norm(Lk.name), rank: (CLASSES[Lk.cls] || CLASSES.local).rank, bAB: U.bearing(g[0], g[g.length - 1]) });
    }
    const pad = Math.max(R, 70) / 90000 + 0.0002;
    const out = new Map();
    for (const c of counts) {
      const cn = norm(c.road || c.name), ll = [c.lat, c.lng];
      let best = null, bestNamed = null;
      for (const it of items) {
        if (c.lat < it.la0 - pad || c.lat > it.la1 + pad || c.lng < it.ln0 - pad * 1.3 || c.lng > it.ln1 + pad * 1.3) continue;
        const pr = Net.project(sc, it.Lk, ll);
        if (pr.d > (c.src === 'nzta' ? Math.max(R, 70) : R)) continue;
        const named = it.nm && cn && (cn.includes(it.nm) || it.nm.includes(cn.split(' ').slice(0, 2).join(' ')));
        let score = pr.d;
        if (c.src === 'nzta') {
          // NZTA ramp sites belong on ramps, mainline sites on the motorway or highway itself
          if (!!c.ramp !== (it.Lk.cls === 'ramp')) continue;
          score += it.rank >= 4 ? 0 : 30;
        } else if (cn && it.nm && !named) {
          if (pr.d > 12) continue;          // a named count on a differently named road only counts if it sits right on it
          score += 25;
        }
        let dir = 'both';
        if (c.directional && c.dir) {
          const want = DIR_BEARING[c.dir];
          const ab = it.Lk.lanesAB > 0 ? U.angDiff(it.bAB, want) : 999, ba = it.Lk.lanesBA > 0 ? U.angDiff((it.bAB + 180) % 360, want) : 999;
          if (Math.min(ab, ba) > 80) continue;   // this road doesn't carry traffic in the counted direction
          dir = ab <= ba ? 'ab' : 'ba';
        } else if (!(it.Lk.lanesAB > 0 && it.Lk.lanesBA > 0)) dir = it.Lk.lanesAB > 0 ? 'ab' : 'ba';
        const cand = { link: it.Lk.id, dir, d: pr.d, score, named };
        if (named && (!bestNamed || score < bestNamed.score)) bestNamed = cand;
        if (!best || score < best.score) best = cand;
      }
      // a road of the count's own name wins whenever one is in range
      if (bestNamed || best) out.set(c.id, bestNamed || best);
    }
    return out;
  }
  let mcache = { key: '' , m: new Map() };
  function matches(sc = Store.sc) {
    const d = D(), key = `${sc.id}|${Store.ver}|${d.counts.length}|${(d.loaded.nzta || {}).at}|${(d.loaded.at || {}).at}|${(d.loaded.user || {}).at}`;
    if (mcache.key !== key) mcache = { key, m: match(sc, d.counts) };
    return mcache.m;
  }
  const inUse = c => { const u = D().use; return !c.off && !!u[c.src] && (!c.virtual || u.virtual) && !(isEstimate(c) && (u.ramm === false || shadowed(c))); };
  // Counts that may stand in validation statistics: never virtual sites or RAMM estimates
  const validationOK = c => !!c && !c.virtual && !isEstimate(c) && (!c.obs_method || !OBS_METHODS[c.obs_method] || OBS_METHODS[c.obs_method].validation !== false);

  /* ---------- calibration: model against counts ---------- */
  function calibration(sc = Store.sc, period = Store.S.period) {
    const res = Store.res(sc, period); if (!res) return null;
    const m = matches(sc), rows = [];
    for (const c of D().counts) {
      // RAMM estimates are never validation evidence (docs/spec/local-road-counts)
      if (!inUse(c) || isEstimate(c)) continue;
      const mt = m.get(c.id); if (!mt) continue;
      const pv = periodVol(c, period); if (!pv) continue;
      const a = res.arcs[mt.link + ':ab'], b = res.arcs[mt.link + ':ba'];
      const model = mt.dir === 'ab' ? (a ? a.v : null) : mt.dir === 'ba' ? (b ? b.v : null) : (a ? a.v : 0) + (b ? b.v : 0);
      if (model == null) continue;
      const obs = pv.v, geh = Math.sqrt(2 * (model - obs) ** 2 / Math.max(1, model + obs));
      rows.push({ c, mt, obs, model, geh, how: pv.how });
    }
    if (!rows.length) return { rows, n: 0 };
    const n = rows.length, reg = regression(rows.map(r => r.obs), rows.map(r => r.model));
    const totObs = rows.reduce((s, r) => s + r.obs, 0), totMod = rows.reduce((s, r) => s + r.model, 0);
    rows.sort((x, y) => y.geh - x.geh);
    // r2 stays the OLS value for older callers; pass/fail uses r2OriginCentred and b0 (criteria-and-stats)
    return { rows, n, pass: rows.filter(r => r.geh < 5).length / n, r2: reg.r2Ols, r2Ols: reg.r2Ols, olsSlope: reg.olsB, olsIntercept: reg.olsA,
      b0: reg.b0, slope: reg.b0, r2OriginCentred: reg.r2OriginCentred, r2OriginExcel: reg.r2OriginExcel, labels: REG_LABELS, totObs, totMod };
  }
  /* Model (y) against counts (x), docs/spec/calc-corrections (3):
     slope through the origin b0 = Σxy / Σx²; R²_origin_centred = 1 − Σ(y − b0·x)² / Σ(y − ȳ)² (the TMDG test);
     R²_origin_excel = 1 − Σ(y − b0·x)² / Σy² (what Excel shows for a trendline set through zero);
     OLS y = a + b·x and its R² (the square of Pearson's r). Values are null where undefined. */
  const REG_LABELS = { b0: 'Slope through the origin', r2OriginCentred: 'R² through the origin (centred; TMDG test)', r2OriginExcel: 'R² through the origin (Excel)', r2Ols: 'R² (OLS)', olsSlope: 'OLS slope', olsIntercept: 'OLS intercept (veh/h)' };
  function regression(x, y) {
    const n = Math.min(x.length, y.length);
    const out = { n, b0: null, r2OriginCentred: null, r2OriginExcel: null, olsA: null, olsB: null, r2Ols: null };
    if (n < 2) return out;
    let sx = 0, sy = 0, sxy = 0, sxx = 0, syy = 0;
    for (let i = 0; i < n; i++) { sx += x[i]; sy += y[i]; sxy += x[i] * y[i]; sxx += x[i] * x[i]; syy += y[i] * y[i]; }
    const mx = sx / n, my = sy / n;
    if (sxx > 0) {
      const b0 = sxy / sxx;
      let se = 0, st = 0;
      for (let i = 0; i < n; i++) { se += (y[i] - b0 * x[i]) ** 2; st += (y[i] - my) ** 2; }
      Object.assign(out, { b0, r2OriginCentred: st > 0 ? 1 - se / st : null, r2OriginExcel: syy > 0 ? 1 - se / syy : null });
    }
    let cxy = 0, cxx = 0, cyy = 0;
    for (let i = 0; i < n; i++) { cxy += (x[i] - mx) * (y[i] - my); cxx += (x[i] - mx) ** 2; cyy += (y[i] - my) ** 2; }
    if (cxx > 0) { out.olsB = cxy / cxx; out.olsA = my - out.olsB * mx; }
    if (cxx > 0 && cyy > 0) out.r2Ols = cxy * cxy / (cxx * cyy);
    return out;
  }

  /* ---------- journey-time validation (calc-corrections (4)) ----------
     Tier 1 passes when |Tm − To| ≤ max(15% of To, 60 s); tier 2 when ≤ max(25% of To, 90 s). */
  const JT_CRITERIA = { id: 'tmdg-jt', name: 'Journey time validation', edition: 'NZTA TMDG 2019; UK TAG M3.1 Table 3', verified: false,
    tiers: [{ id: 'tier1', rel: 0.15, abs: 60 }, { id: 'tier2', rel: 0.25, abs: 90 }],
    text: 'Within 15% or 1 minute, whichever is larger (TMDG 2019; TAG M3.1 Table 3).', googleLabel: 'Google typical (predicted, not observed)' };
  function jtCheck(modelS, obsS) {
    if (!(modelS >= 0) || !(obsS > 0)) return null;
    const diff = modelS - obsS, out = { model: modelS, obs: obsS, diff, pct: diff / obsS, text: JT_CRITERIA.text };
    for (const t of JT_CRITERIA.tiers) { const tol = Math.max(t.rel * obsS, t.abs); out[t.id] = Math.abs(diff) <= tol; out[t.id + 'Tol'] = tol; }
    return out;
  }

  /* ---------- gateway flows from counts ---------- */
  function gatewaysFromCounts(sc) {
    const d = D(), m = matches(sc), deg = Net.degree(sc);
    // Physical counts first; RAMM centreline estimates only where no count is near a gateway
    const usable = d.counts.filter(c => inUse(c) && !isEstimate(c) && m.get(c.id)), estimates = d.counts.filter(c => inUse(c) && isEstimate(c) && m.get(c.id));
    // NZTA's own estimates (virtual sites) beat a guess where no physical count is near a gateway
    const fallback = d.counts.filter(c => c.virtual && !c.off && d.use.nzta && m.get(c.id));
    let set = 0;
    const log = [];
    for (const z of Object.values(sc.zones)) {
      if (z.kind !== 'gateway' || !z.node || !sc.nodes[z.node]) continue;
      const Ls = Net.linksAt(sc, z.node); if (!Ls.length) continue;
      const Lg = Ls[0], gNode = sc.nodes[z.node], gName = norm(Lg.name), gRank = (CLASSES[Lg.cls] || CLASSES.local).rank;
      // bearing of traffic entering the area from this gateway
      const g = Net.geom(sc, Lg), inB = Lg.a === z.node ? U.bearing(g[0], g[Math.min(1, g.length - 1)]) : U.bearing(g[g.length - 1], g[g.length - 2]);
      const near = pool => pool.map(c => ({ c, mt: m.get(c.id), dist: U.dist([gNode.lat, gNode.lng], [c.lat, c.lng]) }))
        .filter(x => {
          if (x.dist > 3000) return false;
          const Lc = sc.links[x.mt.link]; if (!Lc || Lc.cls !== Lg.cls) return false;
          if (x.c.src === 'nzta' && !!x.c.ramp !== (Lg.cls === 'ramp')) return false;
          const same = gName && norm(Lc.name) === gName;
          return same || (gRank >= 5 && x.dist < 3000);
        }).sort((a, b) => a.dist - b.dist);
      let cand = near(usable);
      if (!cand.length && estimates.length) cand = near(estimates);
      if (!cand.length) cand = near(fallback);
      if (!cand.length) continue;
      const gw = z.gw ||= {};
      const used = [];
      for (const per of ['AM', 'PM']) {
        let vin = null, vout = null;
        const dirs = cand.filter(x => x.c.directional && x.c.dir);
        if (dirs.length) {
          const inC = dirs.find(x => U.angDiff(DIR_BEARING[x.c.dir], inB) < 70), outC = dirs.find(x => U.angDiff(DIR_BEARING[x.c.dir], (inB + 180) % 360) < 70);
          if (inC) { vin = periodVol(inC.c, per).v; used.push(inC.c); }
          if (outC) { vout = periodVol(outC.c, per).v; used.push(outC.c); }
          const inLanes = Lg.a === z.node ? Lg.lanesAB : Lg.lanesBA, outLanes = Lg.a === z.node ? Lg.lanesBA : Lg.lanesAB;
          if (!(inLanes > 0)) vin = 0;
          if (!(outLanes > 0)) vout = 0;
        } else {
          const tw = cand.find(x => !x.c.directional);
          if (tw) {
            const v = periodVol(tw.c, per).v, inLanes = Lg.a === z.node ? Lg.lanesAB : Lg.lanesBA, outLanes = Lg.a === z.node ? Lg.lanesBA : Lg.lanesAB;
            const split = inLanes > 0 && outLanes > 0 ? 0.5 : inLanes > 0 ? 1 : 0;
            vin = v * split; vout = v * (1 - split); used.push(tw.c);
          }
        }
        if (vin != null) gw[per + 'in'] = Math.round(vin / 10) * 10;
        if (vout != null) gw[per + 'out'] = Math.round(vout / 10) * 10;
      }
      if (used.length) {
        const u = [...new Set(used)];
        z.gwSrc = u.map(c => `${(SOURCES[c.src] || { short: c.src }).short}${c.virtual ? ' estimate' : ''} ${c.ref || ''} ${c.name}${c.adt ? ` (${U.fmt.int(c.adt)}/day)` : ''}`).join('; ');
        set++; log.push(z.name);
      }
    }
    return set;
  }

  /* ---------- crashes near a link or junction ---------- */
  // opts.long: the 10-year set for state highway accesses (loadAll 'cas10'), when it has been downloaded
  function crashesNear(sc, sel, R = 25, opts = {}) {
    const cr = opts.long ? D().crashesLong || [] : D().crashes; if (!cr.length) return null;
    const out = { F: 0, S: 0, M: 0, N: 0, list: [] };
    if (sel.type === 'node') {
      const n = sc.nodes[sel.id]; if (!n) return null;
      for (const c of cr) if (U.dist([n.lat, n.lng], [c[0], c[1]]) <= R + 15) { out[c[2]]++; out.list.push(c); }
    } else if (sel.type === 'link') {
      const Lk = sc.links[sel.id]; if (!Lk) return null;
      const g = Net.geom(sc, Lk);
      let la0 = Infinity, la1 = -Infinity, ln0 = Infinity, ln1 = -Infinity;
      for (const p of g) { la0 = Math.min(la0, p[0]); la1 = Math.max(la1, p[0]); ln0 = Math.min(ln0, p[1]); ln1 = Math.max(ln1, p[1]); }
      const pad = R / 90000;
      for (const c of cr) {
        if (c[0] < la0 - pad || c[0] > la1 + pad || c[1] < ln0 - pad * 1.3 || c[1] > ln1 + pad * 1.3) continue;
        if (Net.project(sc, Lk, [c[0], c[1]]).d <= R) { out[c[2]]++; out.list.push(c); }
      }
    }
    return out;
  }

  /* ---------- road-safety metrics: crashes assigned to links and junctions ----------
     Crash tuples are [lat, lng, severity F|S|M|N, year, location, flags p|c|m, speed limit, then the CAS fields of
     CAS_FIELDS at positions 7–14 where known].
     Each crash goes to its nearest existing (not proposed) link within 25 m; where several links are
     about equally near (a junction), the link named in the crash's first location wins if it is within
     5 m of the nearest. Crashes within 30 m of a junction are also tallied to that junction. A junction
     is a cluster of nodes: a roundabout's ring nodes are one junction, and nodes of three or more links
     within 30 m of each other that share a road name (a divided-road intersection) are one junction.
     Clusters of motorway and ramp links only are interchange merges and diverges, kept apart from junctions.
     Rates use the window the crashes were downloaded for (loadCAS asks for the last five years and the
     current year to date), not just the years that happen to have crashes; old projects without a load
     time fall back to the span of crash years. Divided roads mapped as two one-way carriageways count
     road km once (KiwiRAP collective risk is per km of road, both directions). Personal risk uses
     current exposure only: counts, or modelled volumes from results no later than the crash window.
     Links shorter than 500 m, or with under 200 veh/day, get no risk band: KiwiRAP measures risk over
     sections several kilometres long. */
  const LINK_R = 25, NODE_R = 30, NAME_SLACK = 5, CELL = 50, CAS_YEARS = 5, PAIR_M = 60, SHORT_KM = 0.5, LOW_AADT = 200;
  const isDSI = s => s === 'F' || s === 'S', isInj = s => s === 'F' || s === 'S' || s === 'M';
  const tally = () => ({ crashes: 0, F: 0, S: 0, M: 0, N: 0, dsi: 0, injury: 0, p: 0, c: 0, m: 0 });
  function addTo(t, c) {
    t.crashes++; t[c[2]]++;
    if (isDSI(c[2])) t.dsi++;
    if (isInj(c[2])) t.injury++;
    const f = c[5] || '';
    if (f.includes('p')) t.p++;
    if (f.includes('c')) t.c++;
    if (f.includes('m')) t.m++;
  }
  // Years of crash data covered: the downloaded window, with the current year counted to the load date
  function crashYears(cr, cas = {}) {
    let y0 = Infinity, y1 = -Infinity;
    for (const c of cr) if (c[3] > 0) { y0 = Math.min(y0, c[3]); y1 = Math.max(y1, c[3]); }
    if (cas.at) {
      const t = new Date(cas.at), Y = t.getFullYear(), from = cas.from > 0 ? cas.from : Y - CAS_YEARS;
      const frac = Math.max(1 / 12, (t - new Date(Y, 0, 1)) / (new Date(Y + 1, 0, 1) - new Date(Y, 0, 1)));
      // fullTo / ytd: the full years and the year to date (the CR1 table shows them apart)
      return { from, to: Y, span: Y - from + frac, partial: true, window: true, fullTo: cas.to > 0 ? cas.to : Y - 1, ytd: cas.ytd > 0 ? cas.ytd : Y, lagYear: cas.lagYear || null };
    }
    if (!isFinite(y0)) return { from: null, to: null, span: null, partial: false, window: false };
    return { from: y0, to: y1, span: y1 - y0 + 1, partial: false, window: false };
  }

  let acache = { key: '' };
  function assignCrashes(sc) {
    const d = D(), cr = d.crashes, cas = d.loaded.cas || {};
    const key = `${sc.id}|${Store.ver}|${Object.keys(sc.links).length}|${Object.keys(sc.nodes).length}|${cas.at}|${cr.length}`;
    if (acache.key === key) return acache;
    const links = Object.values(sc.links).filter(Lk => Lk.status !== 'proposed' && sc.nodes[Lk.a] && sc.nodes[Lk.b]);
    const nodeIds = Object.keys(sc.nodes);
    let lat0 = 0;
    for (const id of nodeIds) lat0 += sc.nodes[id].lat;
    lat0 = nodeIds.length ? lat0 / nodeIds.length : (cr.length ? cr[0][0] : 0);
    const F = U.flat(lat0), cellKey = (cx, cy) => cx * 1e6 + cy;
    const put = (grid, x0, y0, x1, y1, pad, v) => {
      const cx0 = Math.floor((x0 - pad) / CELL), cx1 = Math.floor((x1 + pad) / CELL), cy0 = Math.floor((y0 - pad) / CELL), cy1 = Math.floor((y1 + pad) / CELL);
      for (let cx = cx0; cx <= cx1; cx++) for (let cy = cy0; cy <= cy1; cy++) {
        const k = cellKey(cx, cy), a = grid.get(k);
        if (a) a.push(v); else grid.set(k, [v]);
      }
    };
    // link segments in flat metres, gridded with the search radius as padding
    const seg = [], lgrid = new Map(), lk = links.map(Lk => ({ Lk, nm: norm(Lk.name), km: U.polyLen(Net.geom(sc, Lk)) / 1000, t: tally() }));
    lk.forEach((it, li) => {
      const g = Net.geom(sc, it.Lk);
      for (let i = 0; i < g.length - 1; i++) {
        const ax = F.x(g[i]), ay = F.y(g[i]), bx = F.x(g[i + 1]), by = F.y(g[i + 1]), s = seg.length;
        seg.push([ax, ay, bx, by, li]);
        put(lgrid, Math.min(ax, bx), Math.min(ay, by), Math.max(ax, bx), Math.max(ay, by), LINK_R, s);
      }
    });
    // Junction nodes: three or more existing links, and every node of a roundabout ring
    const deg = new Map(), inc = new Map(), onRing = new Set();
    for (const Lk of links) for (const n of [Lk.a, Lk.b]) {
      deg.set(n, (deg.get(n) || 0) + 1);
      const a = inc.get(n); if (a) a.push(Lk); else inc.set(n, [Lk]);
      if (Lk.ring) onRing.add(n);
    }
    const jids = [...deg.keys()].filter(id => deg.get(id) >= 3 || onRing.has(id)).sort();
    const par = new Map(jids.map(id => [id, id]));
    const find = id => { while (par.get(id) !== id) { par.set(id, par.get(par.get(id))); id = par.get(id); } return id; };
    const join = (a, b) => { const x = find(a), y = find(b); if (x !== y) par.set(x < y ? y : x, x < y ? x : y); };
    for (const Lk of links) if (Lk.ring && par.has(Lk.a) && par.has(Lk.b)) join(Lk.a, Lk.b);
    const xy = id => { const n = sc.nodes[id]; return [F.x([n.lat, n.lng]), F.y([n.lat, n.lng])]; };
    const pgrid = new Map(), nameSet = id => new Set((inc.get(id) || []).map(Lk => norm(Lk.name)).filter(Boolean));
    for (const id of jids) if (deg.get(id) >= 3) { const [x, y] = xy(id); put(pgrid, x, y, x, y, 0, id); }
    for (const id of jids) {
      if (deg.get(id) < 3) continue;
      const [x, y] = xy(id), mine = nameSet(id), seen = new Set();
      for (let cx = Math.floor((x - NODE_R) / CELL); cx <= Math.floor((x + NODE_R) / CELL); cx++)
        for (let cy = Math.floor((y - NODE_R) / CELL); cy <= Math.floor((y + NODE_R) / CELL); cy++)
          for (const o of pgrid.get(cellKey(cx, cy)) || []) {
            if (o <= id || seen.has(o)) continue;
            seen.add(o);
            const [ox, oy] = xy(o);
            if (Math.hypot(ox - x, oy - y) <= NODE_R && [...nameSet(o)].some(n => mine.has(n))) join(id, o);
          }
    }
    const groups = new Map();
    for (const id of jids) { const r = find(id), g = groups.get(r); if (g) g.push(id); else groups.set(r, [id]); }
    const jn = [], jOfNode = new Map(), ngrid = new Map(), ptNode = [];
    for (const members of groups.values()) {
      const inside = new Set(members);
      // Legs: links leaving the cluster; the two carriageways of one road on one side are one leg
      const cx = members.reduce((s, id) => s + xy(id)[0], 0) / members.length, cy = members.reduce((s, id) => s + xy(id)[1], 0) / members.length;
      const legs = [], ext = [];
      for (const id of members) for (const Lk of inc.get(id) || []) {
        if (inside.has(Lk.a) && inside.has(Lk.b)) continue;
        if (ext.includes(Lk)) continue;
        ext.push(Lk);
        const g = Net.geom(sc, Lk), far = inside.has(Lk.a) ? g[Math.min(g.length - 1, 1)] : g[Math.max(0, g.length - 2)];
        const b = Math.atan2(F.x(far) - cx, F.y(far) - cy) * 180 / Math.PI, nm = norm(Lk.name);
        if (!legs.some(l => nm && l.nm === nm && U.angDiff(l.b, b) < 45)) legs.push({ nm, b, Lk });
      }
      if (legs.length < 3) continue;   // a turning loop or a bend, not a junction
      const rep = members.slice().sort((a, b) => deg.get(b) - deg.get(a) || (a < b ? -1 : 1))[0];
      const interchange = ext.every(Lk => Lk.cls === 'motorway' || Lk.cls === 'ramp');
      const named = legs.map(l => l.Lk).filter(Lk => Lk.name).sort((p, q) => (CLASSES[q.cls] || CLASSES.local).rank - (CLASSES[p.cls] || CLASSES.local).rank || p.name.localeCompare(q.name));
      const j = { id: rep, nodes: members, x: cx, y: cy, legs: legs.length, interchange, name: [...new Set(named.map(Lk => Lk.name))].slice(0, 3).join(' / ') || (interchange ? 'Motorway merge or diverge' : 'Unnamed junction'), t: tally() };
      for (const id of members) { jOfNode.set(id, jn.length); const [x, y] = xy(id); put(ngrid, x, y, x, y, NODE_R, ptNode.length); ptNode.push([x, y, jn.length]); }
      jn.push(j);
    }
    const linkOf = new Int32Array(cr.length).fill(-1), nodeOf = new Int32Array(cr.length).fill(-1);
    cr.forEach((c, ci) => {
      const ll = [c[0], c[1]], px = F.x(ll), py = F.y(ll), k = cellKey(Math.floor(px / CELL), Math.floor(py / CELL));
      const cand = lgrid.get(k);
      if (cand) {
        const dl = new Map();
        for (const s of cand) {
          const q = seg[s], r = U.segDist(px, py, q[0], q[1], q[2], q[3]);
          if (r.d <= LINK_R && !(dl.get(q[4]) <= r.d)) dl.set(q[4], r.d);
        }
        let best = -1, bd = Infinity;
        for (const [li, dd] of dl) if (dd < bd) { bd = dd; best = li; }
        if (best >= 0) {
          const loc = norm(String(c[4] || '').split(' / ')[0]);
          if (loc && lk[best].nm !== loc) {
            let nb = -1, nd = Infinity;
            for (const [li, dd] of dl) {
              const nm = lk[li].nm;
              if (dd <= bd + NAME_SLACK && dd < nd && nm && (nm === loc || nm.includes(loc) || loc.includes(nm))) { nd = dd; nb = li; }
            }
            if (nb >= 0) best = nb;
          }
          linkOf[ci] = best; addTo(lk[best].t, c);
        }
      }
      // to the junction of the nearest junction node within 30 m
      const nc = ngrid.get(k);
      if (nc) {
        let best = -1, bd = NODE_R;
        for (const p of nc) { const dd = Math.hypot(px - ptNode[p][0], py - ptNode[p][1]); if (dd <= bd) { bd = dd; best = ptNode[p][2]; } }
        if (best >= 0) { nodeOf[ci] = best; addTo(jn[best].t, c); }
      }
    });
    // One-way carriageways paired with the opposite carriageway of the same road (a divided road)
    const pair = new Map(), byName = new Map();
    lk.forEach((it, li) => {
      const Lk = it.Lk; if (!it.nm || ((Lk.lanesAB | 0) > 0 && (Lk.lanesBA | 0) > 0)) return;
      const a = byName.get(it.nm); if (a) a.push(li); else byName.set(it.nm, [li]);
    });
    const dirB = Lk => { const g = Net.geom(sc, Lk), b = U.bearing(g[0], g[g.length - 1]); return (Lk.lanesAB | 0) > 0 ? b : (b + 180) % 360; };
    for (const ids of byName.values()) for (const li of ids) {
      const Lk = lk[li].Lk, g = Net.geom(sc, Lk), mid = U.along(g, U.cumLen(g), U.polyLen(g) / 2).p, b = dirB(Lk);
      for (const lj of ids) {
        if (lj === li) continue;
        const O = lk[lj].Lk;
        if (U.angDiff(b, dirB(O)) > 135 && Net.project(sc, O, mid).d <= PAIR_M) { pair.set(Lk.id, O.id); break; }
      }
    }
    acache = { key, lk, jn, jOfNode, linkOf, nodeOf, pair, years: crashYears(cr, cas), out: null };
    return acache;
  }

  /* Two-way AADT on a link from matched counts in use (NZTA estimates only while the NZTA source is on).
     Two-way link: a two-way count wins, then both directions summed, then one direction doubled.
     One-way carriageway: counts in its own direction win; a count with no direction is a two-way total,
     so it is halved where the road's other carriageway runs alongside. */
  function linkAADT(sc, pair = new Map()) {
    const m = matches(sc), by = new Map(), use = D().use || {};
    for (const c of D().counts) {
      if (c.off || !(c.adt > 0) || !(inUse(c) || (c.virtual && use.nzta))) continue;
      const mt = m.get(c.id); if (!mt) continue;
      const e = by.get(mt.link) || { both: [], ab: [], ba: [] };
      e[c.directional && c.dir && mt.dir !== 'both' ? mt.dir : 'both'].push(c.adt);
      by.set(mt.link, e);
    }
    const mean = a => a.reduce((s, v) => s + v, 0) / a.length, out = new Map();
    for (const [id, e] of by) {
      const Lk = sc.links[id]; if (!Lk) continue;
      const two = Lk.lanesAB > 0 && Lk.lanesBA > 0, own = Lk.lanesAB > 0 ? e.ab : e.ba;
      if (two) {
        if (e.both.length) out.set(id, { v: mean(e.both), src: 'count' });
        else if (e.ab.length && e.ba.length) out.set(id, { v: mean(e.ab) + mean(e.ba), src: 'count' });
        else out.set(id, { v: mean(e.ab.length ? e.ab : e.ba) * 2, src: 'count (one direction, doubled)' });
      } else if (own.length) out.set(id, { v: mean(own), src: 'count' });
      else if (e.both.length) out.set(id, pair.has(id) ? { v: mean(e.both) / 2, src: 'count (two-way total, halved for this carriageway)' } : { v: mean(e.both), src: 'count' });
    }
    return out;
  }

  /* linkRisk(sc, res?) → { linkId: { crashes, dsi, injury, F, S, M, N, p, c, m, km, roadKm, divided, years, perKmYr
     (injury crashes per road km per year), dsiPerKmYr (KiwiRAP collective risk), aadt, aadtSrc, personal (DSI crashes
     per 100 million vehicle-km), short, lowVolume } } for every existing link, or null if no crashes are loaded.
     Volume comes from a matched count where there is one, otherwise (AB + BA modelled veh/h) × peakToDaily from res,
     or the mean of the scenario's AM and PM results when res is not given; results for a year after the crash
     window are not used (the base scenario's are tried instead). The returned object is cached: do not change it. */
  function linkRisk(sc = Store.sc, res) {
    if (!D().crashes.length) return null;
    const A = assignCrashes(sc), yr = A.years;
    const current = r => r && r.arcs && !(r.year > 0 && yr.to > 0 && r.year > yr.to + 1);
    let rs = res ? [res] : ['AM', 'PM'].map(p => sc.res && sc.res[p]).filter(Boolean);
    const forecast = rs.length && !rs.some(current) ? rs[0].year : null;
    rs = rs.filter(current);
    if (!rs.length && Store.P && Store.baseSc && Store.baseSc !== sc) rs = ['AM', 'PM'].map(p => Store.baseSc.res && Store.baseSc.res[p]).filter(current);
    const vkey = `${(D().loaded.nzta || {}).at}|${(D().loaded.at || {}).at}|${(D().loaded.user || {}).at}|${D().counts.length}|${JSON.stringify(D().use || {})}`;
    if (A.out && A.vkey === vkey && A.rs.length === rs.length && A.rs.every((r, i) => r === rs[i])) return A.out;
    const yrs = yr.span, ptd = Store.S.peakToDaily || 10, aadt = linkAADT(sc, A.pair), out = {};
    for (const it of A.lk) {
      const id = it.Lk.id, t = it.t;
      let v = aadt.get(id), vol = v ? v.v : null, src = v ? v.src : null;
      if (vol == null && rs.length) {
        let s = 0;
        for (const r of rs) { const a = r.arcs[id + ':ab'], b = r.arcs[id + ':ba']; s += (a ? a.v : 0) + (b ? b.v : 0); }
        vol = s / rs.length * ptd; src = 'model';
      } else if (vol == null && forecast) src = `model (forecast year ${forecast}, not used)`;
      const divided = A.pair.has(id), roadKm = divided ? it.km / 2 : it.km;
      const ky = roadKm * yrs, vkm = vol > 0 && it.km > 0 && yrs > 0 ? vol * 365 * it.km * yrs : 0;
      out[id] = { ...t, km: it.km, roadKm, divided, years: yrs, aadt: vol, aadtSrc: src,
        perKmYr: ky > 0 ? t.injury / ky : null, dsiPerKmYr: ky > 0 ? t.dsi / ky : null, personal: vkm > 0 ? t.dsi / vkm * 1e8 : null,
        short: it.km < SHORT_KM, lowVolume: !(vol >= LOW_AADT) };
    }
    Object.assign(A, { out, rs, vkey });
    return out;
  }

  // nodeRisk(sc) → { junctionId: { crashes, dsi, injury, F, S, M, N, p, c, m, legs, nodes[], interchange, years, dsiPerYr,
  // injPerYr, name } } for every junction (keyed by its main node; see junctionOf), or null if no crashes are loaded
  function nodeRisk(sc = Store.sc) {
    if (!D().crashes.length) return null;
    const A = assignCrashes(sc);
    if (A.nodes) return A.nodes;
    const yrs = A.years.span, out = {};
    for (const j of A.jn) {
      out[j.id] = { ...j.t, legs: j.legs, nodes: j.nodes, interchange: j.interchange, years: yrs, dsiPerYr: yrs ? j.t.dsi / yrs : null, injPerYr: yrs ? j.t.injury / yrs : null, name: j.name };
    }
    return A.nodes = out;
  }
  // The junction (nodeRisk key) a node belongs to, or null
  function junctionOf(sc, nodeId) {
    if (!D().crashes.length) return null;
    const A = assignCrashes(sc), j = A.jOfNode.get(nodeId);
    return j == null ? null : A.jn[j].id;
  }

  /* riskBand(value, kind) → { band, label, colour, basis }. Kinds:
     'collective' (DSI crashes per km per year) and 'personal' (DSI crashes per 100 million vehicle-km): KiwiRAP
       risk-map bands, verified against the KiwiRAP regional brochure "How safe are our roads?" (Northland/
       Auckland, 2012, 2007–2011 crashes; kiwirap.org.nz/assets/pdf): collective Low <= 0.039, Low-medium 0.04–0.069,
       Medium 0.07–0.10, Medium-high 0.11–0.189, High 0.19+; personal Low < 4, Low-medium 4–4.9, Medium 5–6.9,
       Medium-high 7–8.9, High 9+. KiwiRAP set them for rural state highways (urban sections excluded) by spreading
       links equally over five bands; they are not a design standard, so they are indicative on urban and local roads.
     'injury' (all injury crashes per km per year) and 'node' (DSI crashes per year at a junction): Spare Construct's own
       indicative bands, not NZTA thresholds. */
  const BANDS = {
    collective: { cuts: [0.04, 0.07, 0.11, 0.19], basis: 'KiwiRAP collective risk bands (rural state highways), indicative here' },
    personal: { cuts: [4, 5, 7, 9], basis: 'KiwiRAP personal risk bands (rural state highways), indicative here' },
    injury: { cuts: [0.5, 1, 2, 4], basis: 'Indicative bands set by Spare Construct, not an NZTA threshold' },
    node: { cuts: [0.1, 0.2, 0.4, 0.6], basis: 'Indicative bands set by Spare Construct, not an NZTA threshold' },
  };
  const BAND_NAMES = ['low', 'low-medium', 'medium', 'medium-high', 'high'];
  const BAND_LABELS = ['Low', 'Low-medium', 'Medium', 'Medium-high', 'High'];
  const BAND_COLOURS = ['#2f9e44', '#94d82d', '#fcc419', '#f76707', '#e03131'];
  function riskBand(value, kind = 'collective') {
    const B = BANDS[kind] || BANDS.collective;
    if (value == null || !isFinite(value)) return { band: null, label: 'No data', colour: '#adb5bd', basis: B.basis };
    let i = 0;
    while (i < B.cuts.length && value >= B.cuts[i]) i++;
    return { band: BAND_NAMES[i], label: BAND_LABELS[i], colour: BAND_COLOURS[i], basis: B.basis };
  }
  // Too short (or too quiet) to band: a rate over a few hundred metres says little
  const noBand = why => ({ band: null, label: why, colour: '#adb5bd', basis: `No band: ${why.toLowerCase()}. KiwiRAP bands are for sections several kilometres long.` });

  // safetySummary(sc, res?) → study-area totals by severity and year, vulnerable road users, the top ten links
  // (500 m or longer) and junctions by DSI then injury crashes, and interchange merges apart; null if no crashes
  function safetySummary(sc = Store.sc, res) {
    const cr = D().crashes; if (!cr.length) return null;
    const A = assignCrashes(sc), lr = linkRisk(sc, res), nr = nodeRisk(sc);
    const total = tally(), years = new Map(), vul = { p: tally(), c: tally(), m: tally(), any: tally() };
    for (const c of cr) {
      addTo(total, c);
      const y = c[3] > 0 ? c[3] : null;
      if (!years.has(y)) years.set(y, tally());
      addTo(years.get(y), c);
      const f = c[5] || '';
      for (const k of ['p', 'c', 'm']) if (f.includes(k)) addTo(vul[k], c);
      if (f) addTo(vul.any, c);
    }
    const rank = (x, y) => y.dsi - x.dsi || y.injury - x.injury || y.crashes - x.crashes;
    const topLinks = Object.entries(lr).filter(([, r]) => r.crashes > 0 && !r.short).map(([id, r]) => {
      const Lk = sc.links[id];
      return { id, name: Lk.name || 'Unnamed road', cls: Lk.cls, ...r, collective: riskBand(r.dsiPerKmYr, 'collective'),
        personalBand: r.lowVolume ? noBand(r.aadt > 0 ? 'Under 200 veh/day' : 'No traffic volume') : riskBand(r.personal, 'personal') };
    }).sort(rank).slice(0, 10);
    const nodes = Object.entries(nr).filter(([, r]) => r.crashes > 0).map(([id, r]) => ({ id, ...r, band: riskBand(r.dsiPerYr, 'node') })).sort(rank);
    const topNodes = nodes.filter(n => !n.interchange).slice(0, 10), interchanges = nodes.filter(n => n.interchange).slice(0, 5);
    let onLinks = 0, atNodes = 0;
    for (let i = 0; i < cr.length; i++) { if (A.linkOf[i] >= 0) onLinks++; if (A.nodeOf[i] >= 0) atNodes++; }
    const shortN = Object.values(lr).filter(r => r.crashes > 0 && r.short).length;
    return {
      years: A.years, total, dsi: total.dsi, injury: total.injury,
      // ytd: the year to date ('YTD, incomplete'); mayBeIncomplete: a full year still inside the CAS entry lag
      byYear: [...years].sort((a, b) => (a[0] ?? 9999) - (b[0] ?? 9999)).map(([year, t]) => ({ year, ...t,
        ytd: !!(A.years.window && year === A.years.ytd), mayBeIncomplete: !!(A.years.lagYear && year === A.years.lagYear) })),
      vulnerable: vul, onLinks, atNodes, unassigned: cr.length - onLinks, topLinks, topNodes, interchanges, shortLinks: shortN,
      basis: `Reported crashes (NZTA CAS) within ${LINK_R} m of a road and ${NODE_R} m of a junction, ${A.years.from || '?'}–${A.years.to || '?'}${A.years.partial ? ' (latest year to date)' : ''}. ` +
        `Links under ${SHORT_KM * 1000} m are left out of the top links and get no band. Risk bands are indicative.`,
    };
  }

  // wrap one link's rate for display (the UI can use this to grey out short links)
  function linkBand(r, kind = 'collective') {
    if (!r) return riskBand(null, kind);
    if (r.short) return noBand('Under 500 m');
    if (kind === 'personal' && r.lowVolume) return noBand(r.aadt > 0 ? 'Under 200 veh/day' : 'No traffic volume');
    return riskBand(kind === 'personal' ? r.personal : kind === 'injury' ? r.perKmYr : r.dsiPerKmYr, kind);
  }

  /* ---------- live traffic (TomTom) ---------- */
  const tomtomTiles = () => keys.tomtom ? `https://api.tomtom.com/traffic/map/4/tile/flow/relative0/{z}/{x}/{y}.png?key=${encodeURIComponent(keys.tomtom)}&tileSize=256` : null;
  async function tomtomSpeed(ll) {
    if (!keys.tomtom) throw new Error('Add a TomTom key in the Data tab first.');
    const r = await fetch(`https://api.tomtom.com/traffic/services/4/flowSegmentData/absolute/10/json?point=${ll[0].toFixed(6)},${ll[1].toFixed(6)}&unit=KMPH&key=${encodeURIComponent(keys.tomtom)}`);
    if (!r.ok) throw new Error(r.status === 403 || r.status === 401 ? 'TomTom refused the key' : 'TomTom answered HTTP ' + r.status);
    const f = (await r.json()).flowSegmentData;
    return { now: f.currentSpeed, free: f.freeFlowSpeed, conf: f.confidence, closed: f.roadClosure, at: Date.now() };
  }

  /* ---------- traffic-aware travel time (Google Routes API) ---------- */
  // Next Tuesday–Thursday at hh:00 New Zealand time, as an ISO instant
  function nextWeekdayNZ(hour) {
    const now = new Date();
    for (let add = 1; add < 10; add++) {
      const t = new Date(now.getTime() + add * 86400000);
      const parts = Object.fromEntries(new Intl.DateTimeFormat('en-NZ', { timeZone: 'Pacific/Auckland', year: 'numeric', month: '2-digit', day: '2-digit', weekday: 'short' }).formatToParts(t).map(p => [p.type, p.value]));
      if (!['Tue', 'Wed', 'Thu'].includes(parts.weekday)) continue;
      const y = +parts.year, mo = +parts.month - 1, dd = +parts.day;
      let guess = Date.UTC(y, mo, dd, hour) - 12 * 3600000;
      for (let k = 0; k < 3; k++) {
        const p = Object.fromEntries(new Intl.DateTimeFormat('en-NZ', { timeZone: 'Pacific/Auckland', hour: '2-digit', hourCycle: 'h23', day: '2-digit' }).formatToParts(new Date(guess)).map(x => [x.type, x.value]));
        const diffH = hour - +p.hour + (+p.day !== dd ? (+p.day > dd ? -24 : 24) : 0);
        if (!diffH) break;
        guess += diffH * 3600000;
      }
      return { iso: new Date(guess).toISOString(), label: new Intl.DateTimeFormat('en-NZ', { timeZone: 'Pacific/Auckland', weekday: 'short', day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' }).format(new Date(guess)) };
    }
    return null;
  }
  async function googleTime(from, to, period) {
    if (!keys.google) throw new Error('Add a Google Maps Platform key in the Data tab first.');
    const dep = nextWeekdayNZ(period === 'AM' ? 8 : 17);
    const pt = ll => ({ location: { latLng: { latitude: ll[0], longitude: ll[1] } } });
    const r = await fetch('https://routes.googleapis.com/directions/v2:computeRoutes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Goog-Api-Key': keys.google, 'X-Goog-FieldMask': 'routes.duration,routes.staticDuration,routes.distanceMeters' },
      body: JSON.stringify({ origin: pt(from), destination: pt(to), travelMode: 'DRIVE', routingPreference: 'TRAFFIC_AWARE_OPTIMAL', departureTime: dep.iso, units: 'METRIC' }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok || j.error) throw new Error((j.error && j.error.message) || 'Google answered HTTP ' + r.status);
    const rt = j.routes && j.routes[0];
    if (!rt) throw new Error('Google found no route');
    return { t: parseFloat(rt.duration), tFree: parseFloat(rt.staticDuration), len: rt.distanceMeters, when: dep.label, at: Date.now() };
  }

  return { SOURCES, keys, setKey, D, area, arcgis, loadAll, loadCAS, loadFutureConnect, loadControlled, conflateFC, AT_LICENCE, ARCGIS_TIMEOUT, casWindow, CAS_WINDOW, CAS_FIELDS, CAS_POS, casTuple,
    importCSV, periodVol, obsMethod, countWeight, OBS_METHODS, isEstimate, shadowed, validationOK, matches, inUse, calibration, regression, REG_LABELS, jtCheck, JT_CRITERIA, gatewaysFromCounts, crashesNear, linkRisk, nodeRisk, junctionOf, riskBand, linkBand, safetySummary, tomtomTiles, tomtomSpeed, googleTime, nextWeekdayNZ, norm };
})();
