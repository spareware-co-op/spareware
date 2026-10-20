'use strict';
/* Spare Construct · existing land use from OpenStreetMap (contract C4). Downloads building outlines,
   land-use areas, schools, early childhood centres, clinics and shops via Overpass, classifies
   each building, and groups the result into base-year zones on a grid.

   NZ OSM buildings come from the LINZ import and are almost all untagged (building=yes), so the
   land-use area a building sits in and its footprint size do most of the work. Every number here
   is indicative and listed in stats.assumptions so it can be shown in the UI and report.

   Additions beyond C4 (no deviations): build() also takes opts.links (sc.links or an array of
   {name, pts}; default the active scenario's links) to name zones after the nearest named road when
   buildings carry no addr:street, and stats adds ignoredBy, schools[], children, medical, supermarket,
   zones and merged. Hotels, tertiary campuses, warehouses and fast food use the C2 land-use keys. */

const ExistingLU = (() => {
  const TILE = 3000;                        // m, building query tile edge
  const LU_TILE = 9000;                     // m, land-use/amenity/shop query tile edge
  const MAX_AREA = 400e6;                   // m², refuse anything bigger than 20 × 20 km
  const MIN_FOOT = 30, HOUSE_MIN = 60, HOUSE_MAX = 400, M2_PER_DWELLING = 90, M2_PER_UNIT = 200, M2_PER_ROOM = 35;
  const OUTBUILDING_R = 25, SHOP_NEAR = 60, RURAL_SHOP_NEAR = 40, POI_SNAP = 25, CAMPUS_R = 250, RURAL_GROUP = 40;
  const DENSE_R = 100, DENSE_N = 6;         // 6 or more buildings of house size or larger within 100 m: a settlement, not a farm
  const MEDICAL_M2 = 300, CHILDREN = 40, SHOP_M2 = 100, SUPERMARKET_M2 = 2500, FASTFOOD_M2 = 200, TERTIARY = 1000;
  const SITE_COVER = 0.35;                  // building cover of a shop site mapped without buildings
  const MERGE_BELOW = 15;                   // dwelling-equivalents
  // Dwelling-equivalents per unit (dwelling, m² GFA, student, child): AM+PM vehicle trips per unit
  // relative to a house, from the indicative default rates. Used only to weight zone centres and merges.
  const WEIGHT = { residential: 1, apartment: 0.53, retail: 0.033, office: 0.021, industrial: 0.0067,
    school: 0.28, childcare: 0.83, medical: 0.03, supermarket: 0.06, tertiary: 0.25, hotel: 0.44, warehouse: 0.0056, fastfood: 0.089 };
  const KEYS = Object.keys(WEIGHT);

  const IGNORE = new Set(['garage', 'garages', 'shed', 'carport', 'roof', 'greenhouse', 'hut', 'cabin', 'service', 'construction',
    'abandoned', 'ruins', 'hangar', 'farm_auxiliary', 'barn', 'cowshed', 'stable', 'sty', 'glasshouse', 'tank', 'silo', 'storage_tank',
    'water_tower', 'bunker', 'bridge', 'container', 'shelter', 'toilets', 'kiosk_atm', 'no']);
  // Buildings that generate trips but have no Spare Construct land use (churches, halls, clubrooms…)
  const OTHER_B = new Set(['church', 'chapel', 'cathedral', 'mosque', 'temple', 'synagogue', 'shrine', 'religious', 'pavilion', 'public',
    'civic', 'government', 'sports_hall', 'sports_centre', 'grandstand', 'stadium', 'transportation', 'train_station', 'marae',
    'fire_station', 'police', 'museum']);
  const OTHER_A = new Set(['place_of_worship', 'community_centre', 'library', 'toilets', 'shelter', 'townhall', 'social_facility',
    'fire_station', 'police', 'arts_centre', 'theatre', 'social_centre', 'events_venue', 'bus_station', 'parking', 'fuel']);
  const RES_B = new Set(['house', 'detached', 'semidetached_house', 'bungalow', 'static_caravan']);
  const RETAIL_B = new Set(['retail', 'kiosk', 'shop']);
  const IND_B = new Set(['industrial', 'factory', 'manufacture', 'workshop']);
  const EDU_B = new Set(['school']), TERT_B = new Set(['university', 'college']);
  const CHILD_B = new Set(['kindergarten', 'childcare']);
  const EDU_A = new Set(['school']), TERT_A = new Set(['college', 'university']);
  const CHILD_A = new Set(['kindergarten', 'childcare']);
  const MED_A = new Set(['doctors', 'clinic', 'dentist']);
  // Shops that are workshops, or trade and bulky goods: far fewer trips per m² than general retail
  const SHOP_IND = new Set(['car_repair', 'tyres', 'motorcycle_repair']);
  const SHOP_BULKY = new Set(['car', 'car_parts', 'hardware', 'trade', 'garden_centre', 'furniture', 'bathroom_furnishing', 'electrical', 'tools',
    'building_materials', 'doityourself', 'flooring', 'kitchen', 'appliance', 'boat', 'motorcycle', 'caravan', 'agrarian', 'tiles', 'bed', 'lighting']);
  const SHOP_YARD = new Set(['car', 'garden_centre', 'trade', 'building_materials', 'boat', 'caravan', 'motorcycle', 'agrarian']);
  const shopUse = s => SHOP_IND.has(s) ? 'industrial' : SHOP_BULKY.has(s) ? 'warehouse' : 'retail';
  const LU_CLASS = { residential: 'residential', commercial: 'commercial', retail: 'retail', industrial: 'industrial', education: 'education' };

  /* ---------- bounds, tiles ---------- */
  function box(b) {
    if (!b) return null;
    if (typeof b.getSouth === 'function') return { s: b.getSouth(), w: b.getWest(), n: b.getNorth(), e: b.getEast() };
    if (Array.isArray(b)) return { s: Math.min(b[0][0], b[1][0]), w: Math.min(b[0][1], b[1][1]), n: Math.max(b[0][0], b[1][0]), e: Math.max(b[0][1], b[1][1]) };
    if ('south' in b) return { s: b.south, w: b.west, n: b.north, e: b.east };
    return 's' in b ? b : null;
  }
  const size = B => { const f = U.flat((B.s + B.n) / 2); return { W: (B.e - B.w) * f.kx, H: (B.n - B.s) * f.ky }; };
  // Split a box into near-square tiles no larger than edge metres
  function tiles(b, edge = TILE) {
    const B = box(b), { W, H } = size(B), nx = Math.max(1, Math.ceil(W / edge - 1e-9)), ny = Math.max(1, Math.ceil(H / edge - 1e-9)), out = [];
    for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++)
      out.push({ s: B.s + (B.n - B.s) * j / ny, n: B.s + (B.n - B.s) * (j + 1) / ny, w: B.w + (B.e - B.w) * i / nx, e: B.w + (B.e - B.w) * (i + 1) / nx });
    return out;
  }

  /* ---------- download ---------- */
  const bboxStr = t => [t.s, t.w, t.n, t.e].map(v => v.toFixed(6)).join(',');
  const qBuildings = t => `[out:json][timeout:90];(way["building"](${bboxStr(t)});relation["building"]["type"="multipolygon"](${bboxStr(t)}););out tags geom;`;
  const qLanduse = t => {
    const bb = bboxStr(t), lu = '"landuse"~"^(residential|commercial|retail|industrial|education)$"';
    // A land-use area that encloses the whole tile has no part inside the box, so also ask which areas
    // contain the tile's centre and corners
    const pts = [[(t.s + t.n) / 2, (t.w + t.e) / 2], [t.s, t.w], [t.s, t.e], [t.n, t.w], [t.n, t.e]];
    const isIn = pts.map(([a, b], i) => `is_in(${a.toFixed(6)},${b.toFixed(6)})->.a${i};`).join('');
    const enc = pts.map((p, i) => `way(pivot.a${i})[${lu}];rel(pivot.a${i})[${lu}];`).join('');
    return `[out:json][timeout:90];${isIn}(way[${lu}](${bb});relation[${lu}](${bb});${enc}` +
      `nwr["amenity"~"^(school|college|university|kindergarten|childcare|doctors|clinic|dentist|hospital|fast_food)$"](${bb});nwr["shop"](${bb});nwr["tourism"~"^(hotel|motel)$"](${bb}););out tags geom;`;
  };
  let good = 0;   // index of the last mirror that answered
  async function post(q, label, onStatus) {
    let lastErr;
    for (let k = 0; k < OVERPASS.length; k++) {
      const i = (good + k) % OVERPASS.length, url = OVERPASS[i], host = new URL(url).host;
      try {
        onStatus(`${label} from ${host}…`);
        const r = await fetch(url, { method: 'POST', body: 'data=' + encodeURIComponent(q), headers: { 'Content-Type': 'application/x-www-form-urlencoded' } });
        if (!r.ok) throw new Error(`${host} answered HTTP ${r.status}`);
        const j = await r.json();
        if (j.remark && /error|timed out|out of memory/i.test(j.remark)) throw new Error(`${host}: ${j.remark}`);
        good = i;
        return j.elements || [];
      } catch (e) { lastErr = e; if (/HTTP 429/.test(e.message)) await U.sleep(1000); }
    }
    throw new Error(`Couldn't download existing land use: ${lastErr ? lastErr.message : 'no OpenStreetMap server answered'}. Try again in a minute or use a smaller area.`);
  }
  async function download(bounds, onStatus) {
    if (typeof onStatus !== 'function') onStatus = () => {};
    const B = box(bounds);
    if (!B || ![B.s, B.w, B.n, B.e].every(isFinite) || B.n <= B.s || B.e <= B.w) throw new Error('Existing land use needs a study area: draw or search for one first.');
    const { W, H } = size(B);
    if (W * H > MAX_AREA) throw new Error(`The study area is ${U.fmt.num(W / 1000, 1)} × ${U.fmt.num(H / 1000, 1)} km. Existing land use works up to about 20 × 20 km: zoom in or draw a smaller area.`);
    const jobs = [...tiles(B, TILE).map((t, i, a) => [qBuildings(t), `Downloading buildings${a.length > 1 ? ` ${i + 1}/${a.length}` : ''}`]),
      ...tiles(B, LU_TILE).map((t, i, a) => [qLanduse(t), `Downloading land-use areas, schools and shops${a.length > 1 ? ` ${i + 1}/${a.length}` : ''}`])];
    const seen = new Map();
    for (const [q, label] of jobs) for (const e of await post(q, label, onStatus)) seen.set(e.type + '/' + e.id, e);
    onStatus(`Downloaded ${U.fmt.int(seen.size)} OpenStreetMap features`);
    return [...seen.values()];
  }

  /* ---------- geometry (local metres) ---------- */
  function ringArea(r) { let a = 0; for (let i = 0, n = r.length; i < n; i++) { const p = r[i], q = r[(i + 1) % n]; a += p[0] * q[1] - q[0] * p[1]; } return a / 2; }
  function ringCentroid(r) {
    let a = 0, cx = 0, cy = 0;
    for (let i = 0, n = r.length; i < n; i++) { const p = r[i], q = r[(i + 1) % n], c = p[0] * q[1] - q[0] * p[1]; a += c; cx += (p[0] + q[0]) * c; cy += (p[1] + q[1]) * c; }
    if (Math.abs(a) < 1e-6) { cx = cy = 0; for (const p of r) { cx += p[0]; cy += p[1]; } return [cx / r.length, cy / r.length]; }
    return [cx / (3 * a), cy / (3 * a)];
  }
  function inRing(x, y, r) {
    let c = false;
    for (let i = 0, j = r.length - 1; i < r.length; j = i++) {
      const a = r[i], b = r[j];
      if ((a[1] > y) !== (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]) c = !c;
    }
    return c;
  }
  // Join open member ways end to end into closed rings (multipolygon relations)
  function assemble(parts) {
    const rings = [], open = [];
    const same = (p, q) => p[0] === q[0] && p[1] === q[1];
    for (const p of parts) { if (p.length < 2) continue; if (same(p[0], p[p.length - 1])) rings.push(p); else open.push(p); }
    while (open.length) {
      let cur = open.shift().slice();
      for (let grew = true; grew && !same(cur[0], cur[cur.length - 1]);) {
        grew = false;
        for (let k = 0; k < open.length; k++) {
          const o = open[k], end = cur[cur.length - 1];
          if (same(end, o[0])) cur = cur.concat(o.slice(1));
          else if (same(end, o[o.length - 1])) cur = cur.concat(o.slice(0, -1).reverse());
          else continue;
          open.splice(k, 1); grew = true; break;
        }
      }
      rings.push(cur);
    }
    return rings.filter(r => r.length >= 3);
  }

  // Small uniform grid index over local coordinates
  function grid(cell, lim) {
    const m = new Map(), key = (i, j) => (i + 50000) * 100000 + j + 50000;
    const rng = (a, b, lo, hi) => [Math.max(Math.floor(a / cell), Math.floor(lo / cell)), Math.min(Math.floor(b / cell), Math.floor(hi / cell))];
    return {
      add(bb, v) {
        const [i0, i1] = rng(bb[0], bb[2], lim[0], lim[2]), [j0, j1] = rng(bb[1], bb[3], lim[1], lim[3]);
        for (let i = i0; i <= i1; i++) for (let j = j0; j <= j1; j++) { const k = key(i, j); let a = m.get(k); if (!a) m.set(k, a = []); a.push(v); }
      },
      at(x, y) { return m.get(key(Math.floor(x / cell), Math.floor(y / cell))) || []; },
      near(x, y, r) {
        const out = new Set();
        for (let i = Math.floor((x - r) / cell); i <= Math.floor((x + r) / cell); i++)
          for (let j = Math.floor((y - r) / cell); j <= Math.floor((y + r) / cell); j++) for (const v of m.get(key(i, j)) || []) out.add(v);
        return out;
      },
    };
  }

  /* ---------- classification helpers ---------- */
  const num = v => { const x = parseFloat(String(v ?? '').replace(',', '.')); return isFinite(x) ? x : null; };
  // Counts (rolls, capacities, rooms) may carry thousands separators: '1,200' or '1 450'
  const count = v => { const s = String(v ?? '').trim(); return num(/^\d{1,3}([, ]\d{3})+$/.test(s) ? s.replace(/[, ]/g, '') : s); };
  function levelsOf(t, apartments) {
    const l = num(t['building:levels']);
    if (l != null && l >= 1) return Math.min(60, Math.round(l));
    const h = num(t.height);
    if (h != null && h >= 4.5) return Math.min(60, Math.max(1, Math.round(h / 3)));
    return apartments ? 3 : 1;
  }
  // School roll: capacity tag, else primary 350 / secondary 800 from year levels or name, else 400
  function students(t) {
    const c = count(t.capacity) ?? count(t.students) ?? count(t['capacity:pupils']);
    if (c != null && c > 0) return Math.round(c);
    const yrs = String(t['MOE:years'] || t.grades || t['school:years'] || '').match(/\d+/g);
    const lo = yrs ? Math.min(...yrs.map(Number)) : null, hi = yrs ? Math.max(...yrs.map(Number)) : null;
    const name = String(t.name || '');
    if (/\b(college|high)\b/i.test(name) || (lo != null && lo >= 7 && hi >= 9) || t['isced:level'] === '2;3' || t['isced:level'] === '3') return 800;
    if ((hi != null && hi <= 8) || /\b(primary|intermediate|kura)\b/i.test(name) || t['isced:level'] === '1') return 350;
    return 400;
  }

  /* ---------- build zones ---------- */
  function build(elements, bounds, opts = {}) {
    const cell = Math.max(50, +opts.cell || 400);
    const raw = Array.isArray(elements) ? elements : (elements && elements.elements) || [];
    // De-duplicate and order by id so the result doesn't depend on download order
    const byId = new Map();
    for (const e of raw) if (e && e.type && e.id != null) byId.set(e.type + '/' + e.id, e);
    const els = [...byId.values()].sort((a, b) => a.type < b.type ? -1 : a.type > b.type ? 1 : a.id - b.id);

    let B = box(bounds);
    if (!B || ![B.s, B.w, B.n, B.e].every(isFinite) || B.n <= B.s || B.e <= B.w) {
      B = { s: Infinity, w: Infinity, n: -Infinity, e: -Infinity };
      for (const e of els) for (const p of e.geometry || (e.lat != null ? [e] : [])) if (p) { B.s = Math.min(B.s, p.lat); B.n = Math.max(B.n, p.lat); B.w = Math.min(B.w, p.lon); B.e = Math.max(B.e, p.lon); }
    }
    const stats = { buildings: 0, used: 0, ignored: 0, outside: 0, byClass: {}, ignoredBy: {}, dwellings: 0, gfa: { retail: 0, office: 0, industrial: 0 },
      students: 0, children: 0, medical: 0, supermarket: 0, tertiaryStudents: 0, hotelRooms: 0, schools: [], tertiary: [], zones: 0, merged: 0, assumptions: [] };
    if (!isFinite(B.s) || !els.length) return { zones: [], stats };

    const f = U.flat((B.s + B.n) / 2), kx = f.kx, ky = f.ky;
    const W = (B.e - B.w) * kx, H = (B.n - B.s) * ky, lim = [-2000, -2000, W + 2000, H + 2000];
    const X = (lat, lng) => [(lng - B.w) * kx, (lat - B.s) * ky];
    const inside = (x, y) => x >= 0 && x <= W && y >= 0 && y <= H;
    const ptsOf = g => (g || []).filter(p => p && isFinite(p.lat) && isFinite(p.lon)).map(p => X(p.lat, p.lon));
    function shape(e) {
      let outer, inner = [];
      if (e.type === 'way') { const r = ptsOf(e.geometry); if (r.length < 3) return null; outer = [r]; }
      else if (e.type === 'relation') {
        const ms = (e.members || []).filter(m => m.type === 'way' && m.geometry);
        outer = assemble(ms.filter(m => m.role !== 'inner').map(m => ptsOf(m.geometry)));
        inner = assemble(ms.filter(m => m.role === 'inner').map(m => ptsOf(m.geometry)));
        if (!outer.length) return null;
      } else return null;
      const bb = [Infinity, Infinity, -Infinity, -Infinity];
      for (const r of outer) for (const p of r) { bb[0] = Math.min(bb[0], p[0]); bb[1] = Math.min(bb[1], p[1]); bb[2] = Math.max(bb[2], p[0]); bb[3] = Math.max(bb[3], p[1]); }
      const areas = outer.map(r => Math.abs(ringArea(r)));
      const area = Math.max(0, areas.reduce((a, b) => a + b, 0) - inner.reduce((a, r) => a + Math.abs(ringArea(r)), 0));
      const big = outer[areas.indexOf(Math.max(...areas))];
      let c = ringCentroid(big);
      if (!inRing(c[0], c[1], big)) c = big.reduce((a, p) => [a[0] + p[0] / big.length, a[1] + p[1] / big.length], [0, 0]);
      return { outer, inner, bb, area, cx: c[0], cy: c[1] };
    }
    const contains = (s, x, y) => x >= s.bb[0] && x <= s.bb[2] && y >= s.bb[1] && y <= s.bb[3] && s.outer.some(r => inRing(x, y, r)) && !s.inner.some(r => inRing(x, y, r));

    // Sort features into buildings, areas (land use + amenity/shop sites) and point POIs
    const blds = [], areas = [], pois = [];
    for (const e of els) {
      const t = e.tags || {};
      const kind = EDU_A.has(t.amenity) ? 'school' : TERT_A.has(t.amenity) ? 'tertiary' : CHILD_A.has(t.amenity) ? 'childcare' : MED_A.has(t.amenity) ? 'medical'
        : t.amenity === 'hospital' ? 'hospital' : t.amenity === 'fast_food' ? 'fastfood' : t.tourism === 'hotel' || t.tourism === 'motel' ? 'hotel'
        : t.shop === 'supermarket' ? 'supermarket' : t.shop ? 'shop' : null;
      if (e.type === 'node') { if (kind && isFinite(e.lat)) { const [x, y] = X(e.lat, e.lon); pois.push({ kind, x, y, t }); } continue; }
      const s = shape(e);
      if (t.building && t.building !== 'no') { if (s) blds.push({ e, t, s, b: t.building }); continue; }
      if (!s) {   // open way or broken relation carrying a POI: use the mean of its points
        const p = ptsOf(e.geometry); if (kind && p.length) pois.push({ kind, x: p.reduce((a, q) => a + q[0], 0) / p.length, y: p.reduce((a, q) => a + q[1], 0) / p.length, t });
        continue;
      }
      if (kind) areas.push({ site: kind, t, s, blds: [] });
      else if (LU_CLASS[t.landuse]) areas.push({ lu: LU_CLASS[t.landuse], t, s });
    }
    const aIdx = grid(250, lim);
    areas.forEach(a => aIdx.add(a.s.bb, a));
    const areasAt = (x, y) => aIdx.at(x, y).filter(a => contains(a.s, x, y)).sort((p, q) => p.s.area - q.s.area);

    // Building candidates (not ignored by tag or size) and their indexes
    const cand = [], bIdx = grid(100, lim), cIdx = grid(100, lim);
    const ign = (b, why) => { b.cls = null; b.k = null; b.why = why; };
    for (const b of blds) {
      b.x = b.s.cx; b.y = b.s.cy; b.area = b.s.area; b.in = inside(b.x, b.y);
      b.street = b.t['addr:street'] || null; b.pois = [];
      if (IGNORE.has(b.b) || /^(storage_tank|silo|water_tower|street_cabinet|reservoir_covered|wastewater_plant)$/.test(b.t.man_made || '') || b.t.aeroway) ign(b, 'outbuilding or structure');
      else if (b.area < MIN_FOOT) ign(b, 'under 30 m²');
      else { cand.push(b); bIdx.add(b.s.bb, b); cIdx.add([b.x, b.y, b.x, b.y], b); }
    }
    // Point POIs: snap to the building they sit in, or the one whose outline is nearest within 25 m
    // (points are often mapped at the entrance, outside the wall of a large building)
    const edge = (s, x, y) => { let d = Infinity; for (const r of s.outer) for (let i = 0, n = r.length; i < n; i++) { const p = r[i], q = r[(i + 1) % n]; d = Math.min(d, U.segDist(x, y, p[0], p[1], q[0], q[1]).d); } return d; };
    const loose = [];
    for (const p of pois) {
      let hit = bIdx.at(p.x, p.y).find(b => contains(b.s, p.x, p.y));
      if (!hit) { let best = POI_SNAP; for (const b of bIdx.near(p.x, p.y, POI_SNAP)) { const d = edge(b.s, p.x, p.y); if (d < best) { best = d; hit = b; } } }
      if (hit) hit.pois.push(p); else loose.push(p);
    }
    // General retail and supermarkets (not workshops or trade suppliers) make a commercial area retail
    const shops = grid(100, lim);
    for (const p of pois) if (p.kind === 'supermarket' || (p.kind === 'shop' && shopUse(p.t.shop) === 'retail')) shops.add([p.x, p.y, p.x, p.y], p);
    const shopNear = (x, y, r) => [...shops.near(x, y, r)].some(p => Math.hypot(p.x - x, p.y - y) <= r);
    const bigNear = b => [...cIdx.near(b.x, b.y, OUTBUILDING_R)].some(o => o !== b && o.area >= HOUSE_MIN && Math.hypot(o.x - b.x, o.y - b.y) <= OUTBUILDING_R);

    // Campuses (schools, tertiary, early childhood) and medical/hospital/hotel/shop sites
    const CAMPUS = new Set(['school', 'tertiary', 'childcare']), campuses = [];
    const campus = (kind, x, y, t, from) => { const c = { kind, x, y, t, from }; campuses.push(c); return c; };
    for (const a of areas) if (CAMPUS.has(a.site)) { a.campus = campus(a.site, a.s.cx, a.s.cy, a.t, 'area'); a.campus.area = a; }
    for (const p of pois) if (CAMPUS.has(p.kind)) campus(p.kind, p.x, p.y, p.t, 'point');
    const extra = [];   // activity not tied to one building: {x, y, k, q, street}
    const addX = (x, y, k, q, t) => extra.push({ x, y, k, q, street: (t && t['addr:street']) || null });

    const setUse = (b, cls, k, q) => { b.cls = cls; b.k = k; b.q = q; };
    const floor = (b, apt) => b.area * levelsOf(b.t, apt);
    const tagged = t => num(t['building:levels']) >= 1 || num(t.height) >= 4.5;
    const rooms = (b, t) => count(t.rooms) || Math.max(1, Math.round(floor(b) / M2_PER_ROOM));
    // A building with shops mapped in it: general retail if any shop is, else trade/bulky goods, else a workshop
    const shopsIn = b => { const u = b.pois.filter(p => p.kind === 'shop').map(p => shopUse(p.t.shop)); return u.includes('retail') ? 'retail' : u.includes('warehouse') ? 'warehouse' : 'industrial'; };
    const shopAs = (b, k) => setUse(b, k, k, floor(b));
    function residential(b) {
      if (b.area < HOUSE_MIN && bigNear(b)) return ign(b, 'outbuilding next to a larger building');
      // Large untagged footprints are not assumed to be apartment blocks: one dwelling per 200 m², one floor
      if (b.area > HOUSE_MAX) return tagged(b.t) ? setUse(b, 'apartment', 'apartment', floor(b) / M2_PER_DWELLING) : setUse(b, 'large residential', 'residential', Math.max(1, Math.round(b.area / M2_PER_UNIT)));
      setUse(b, 'residential', 'residential', 1);
    }
    const commercial = b => shopNear(b.x, b.y, SHOP_NEAR) ? setUse(b, 'retail', 'retail', floor(b)) : setUse(b, 'office', 'office', floor(b));
    const orphans = [], rural = [];
    let noArea = 0;

    for (const b of cand) {
      const t = b.t, bt = b.b, am = t.amenity, has = k => b.pois.some(p => p.kind === k), poi = k => b.pois.find(p => p.kind === k);
      const sites = areasAt(b.x, b.y);
      const site = sites.find(a => a.site), lu = sites.find(a => a.lu);
      if (b.in && !site && !lu) noArea++;
      // A shop site's own buildings carry its floor area, so the site adds nothing on top
      for (const a of sites) if (a.site === 'shop' || a.site === 'supermarket') a.blds.push(b);
      // (1) the building's own tags
      if (OTHER_B.has(bt) || OTHER_A.has(am)) { ign(b, 'community or other use'); b.other = true; }
      else if (bt === 'hotel' || t.tourism === 'hotel' || t.tourism === 'motel') setUse(b, 'hotel', 'hotel', rooms(b, t));
      else if (EDU_A.has(am)) { setUse(b, 'education'); campus('school', b.x, b.y, t, 'building'); }
      else if (TERT_A.has(am)) { setUse(b, 'tertiary'); campus('tertiary', b.x, b.y, t, 'building'); }
      else if (CHILD_A.has(am)) { setUse(b, 'childcare'); campus('childcare', b.x, b.y, t, 'building'); }
      else if (MED_A.has(am)) {
        setUse(b, 'medical'); b.ownMed = true; addX(b.x, b.y, 'medical', MEDICAL_M2, t);
        for (const a of sites) if (a.site === 'medical') a.hasMed = true;   // the site is this clinic
      }
      else if (am === 'hospital' || bt === 'hospital') setUse(b, 'medical', 'medical', floor(b));
      else if (am === 'fast_food') shopAs(b, 'fastfood');
      else if (bt === 'supermarket' || t.shop === 'supermarket' || has('supermarket')) shopAs(b, 'supermarket');
      else if (RETAIL_B.has(bt) || t.shop) shopAs(b, t.shop ? shopUse(t.shop) : 'retail');
      else if (bt === 'office' || t.office) setUse(b, 'office', 'office', floor(b));
      else if (bt === 'warehouse') shopAs(b, 'warehouse');
      else if (IND_B.has(bt) || t.industrial || t.craft || t.landuse === 'industrial') setUse(b, 'industrial', 'industrial', floor(b));
      else if (EDU_B.has(bt) || TERT_B.has(bt) || CHILD_B.has(bt)) {
        setUse(b, CHILD_B.has(bt) ? 'childcare' : TERT_B.has(bt) ? 'tertiary' : 'education');
        if (!site || !site.campus) orphans.push(b);
      }
      else if (RES_B.has(bt)) setUse(b, 'residential', 'residential', 1);
      else if (bt === 'apartments') setUse(b, 'apartment', 'apartment', Math.max(1, floor(b, true) / M2_PER_DWELLING));
      else if (bt === 'terrace') setUse(b, 'residential', 'residential', Math.max(1, Math.round(floor(b) / M2_PER_DWELLING)));
      else if (bt === 'commercial') commercial(b);
      // (2) shops, clinics, schools mapped as points in or at it
      else if (has('hotel')) setUse(b, 'hotel', 'hotel', rooms(b, poi('hotel').t));
      else if (has('fastfood')) shopAs(b, 'fastfood');
      else if (has('shop')) shopAs(b, shopsIn(b));
      else if (has('medical')) setUse(b, 'medical');
      else if (has('hospital')) setUse(b, 'medical', 'medical', floor(b));
      else if (has('childcare')) setUse(b, 'childcare');
      else if (has('school')) setUse(b, 'education');
      else if (has('tertiary')) setUse(b, 'tertiary');
      // (3) the site or land-use area it sits in
      else if (site && site.site === 'school') setUse(b, 'education');
      else if (site && site.site === 'tertiary') setUse(b, 'tertiary');
      else if (site && site.site === 'childcare') setUse(b, 'childcare');
      else if (site && site.site === 'medical') setUse(b, 'medical');
      else if (site && site.site === 'hospital') setUse(b, 'medical', 'medical', floor(b));
      else if (site && site.site === 'hotel') { setUse(b, 'hotel'); (site.hb ||= []).push(b); }
      else if (site && site.site === 'fastfood') shopAs(b, 'fastfood');
      else if (site) shopAs(b, site.site === 'supermarket' ? 'supermarket' : shopUse(site.t.shop));
      else if (lu && lu.lu === 'residential') residential(b);
      else if (lu && lu.lu === 'commercial') commercial(b);
      else if (lu && lu.lu === 'retail') setUse(b, 'retail', 'retail', floor(b));
      else if (lu && lu.lu === 'industrial') setUse(b, 'industrial', 'industrial', floor(b));
      else if (lu && lu.lu === 'education') { setUse(b, 'education'); orphans.push(b); }
      // (4) outside any mapped area: rural houses by size, shops by proximity
      else if (bt === 'residential' || (b.area >= HOUSE_MIN && b.area <= HOUSE_MAX)) { residential(b); if (b.cls === 'residential') rural.push(b); }
      else if (shopNear(b.x, b.y, RURAL_SHOP_NEAR)) setUse(b, 'retail', 'retail', floor(b));
      else ign(b, b.area < HOUSE_MIN ? 'small rural building' : 'large rural building (farm or yard)');
      // Clinics mapped as points add 300 m² each, unless the building or its site is already the clinic
      if (!b.ownMed && !(b.cls === 'medical' && site && site.site === 'medical')) for (const p of b.pois) if (p.kind === 'medical') addX(b.x, b.y, 'medical', MEDICAL_M2, p.t);
    }
    /* Rural houses: a farmhouse and its sheds sit together, so a house-sized building within 40 m of a
       house (measured from that house, never chained from shed to shed) is its outbuilding. Where 6 or
       more buildings of house size or larger stand within 100 m it is a settlement (a street with no
       mapped residential area), and each building is a dwelling. */
    const dense = b => { let n = 0; for (const o of cIdx.near(b.x, b.y, DENSE_R)) if (o.area >= HOUSE_MIN && Math.hypot(o.x - b.x, o.y - b.y) <= DENSE_R) n++; return n >= DENSE_N; };
    const farm = rural.filter(b => !dense(b)), taken = new Set(), rIdx = grid(100, lim);
    farm.forEach(b => rIdx.add([b.x, b.y, b.x, b.y], b));
    // The building closest to 150 m² is the house and claims the house-sized sheds around it
    for (const b of farm.slice().sort((p, q) => Math.abs(p.area - 150) - Math.abs(q.area - 150) || p.e.id - q.e.id)) {
      if (taken.has(b)) continue;
      taken.add(b);
      for (const o of rIdx.near(b.x, b.y, RURAL_GROUP)) if (!taken.has(o) && Math.hypot(o.x - b.x, o.y - b.y) <= RURAL_GROUP) { taken.add(o); ign(o, 'rural outbuilding'); }
    }
    // Clinic, hotel and shop sites. A clinic site is 300 m² unless a clinic building in it already counts;
    // shop sites mapped without buildings use a building cover of their land area (yards excluded)
    for (const a of areas) {
      if (!inside(a.s.cx, a.s.cy)) continue;
      if (a.site === 'medical' && !a.hasMed) addX(a.s.cx, a.s.cy, 'medical', MEDICAL_M2, a.t);
      if (a.site === 'hotel') {
        const q = count(a.t.rooms) || (a.hb ? Math.round(a.hb.reduce((s, b) => s + floor(b), 0) / M2_PER_ROOM) : 0);
        if (q > 0) addX(a.s.cx, a.s.cy, 'hotel', q, a.t);
      }
      if ((a.site === 'shop' || a.site === 'supermarket') && !a.blds.length && !SHOP_YARD.has(a.t.shop))
        addX(a.s.cx, a.s.cy, a.site === 'supermarket' ? 'supermarket' : shopUse(a.t.shop), Math.min(10000, a.s.area * SITE_COVER), a.t);
    }
    // Education buildings with no mapped campus: attach to one within 250 m, else start one
    for (const b of orphans) {
      const kind = b.cls === 'childcare' ? 'childcare' : b.cls === 'tertiary' ? 'tertiary' : 'school';
      if (!campuses.some(c => c.kind === kind && Math.hypot(c.x - b.x, c.y - b.y) <= CAMPUS_R)) campus(kind, b.x, b.y, b.t, 'building');
    }
    let looseN = 0;
    const inMedSite = p => areasAt(p.x, p.y).some(a => a.site === 'medical');
    for (const p of loose) {
      if (p.kind === 'shop') addX(p.x, p.y, shopUse(p.t.shop), SHOP_M2, p.t);
      else if (p.kind === 'supermarket') addX(p.x, p.y, 'supermarket', SUPERMARKET_M2, p.t);
      else if (p.kind === 'fastfood') addX(p.x, p.y, 'fastfood', FASTFOOD_M2, p.t);
      else if (p.kind === 'hotel' && count(p.t.rooms)) addX(p.x, p.y, 'hotel', count(p.t.rooms), p.t);
      else if (p.kind === 'medical' && !inMedSite(p)) addX(p.x, p.y, 'medical', MEDICAL_M2, p.t);   // in a clinic site, the site counts it
      else continue;
      if (inside(p.x, p.y)) looseN++;
    }
    // One campus each: a point or tagged building inside a mapped campus area, very close to another,
    // with the same name nearby, or unnamed within 250 m of another of its kind is the same campus
    const rank = { area: 0, building: 1, point: 2 }, kept = [];
    for (const c of campuses.sort((a, b) => rank[a.from] - rank[b.from])) {
      const dup = kept.some(k => {
        if (k.kind !== c.kind) return false;
        const d = Math.hypot(k.x - c.x, k.y - c.y);
        return (k.area && contains(k.area.s, c.x, c.y)) || d < 50 || (c.t.name && c.t.name === k.t.name && d <= CAMPUS_R) || (!c.t.name && c.from !== 'area' && d <= CAMPUS_R);
      });
      if (!dup) kept.push(c);
    }
    for (const c of kept) {
      if (!inside(c.x, c.y)) continue;
      if (c.kind === 'school') { const n = students(c.t); addX(c.x, c.y, 'school', n, c.t); stats.schools.push({ name: c.t.name || 'Unnamed school', students: n, src: c.from }); }
      else if (c.kind === 'tertiary') { const n = Math.round(count(c.t.capacity) || count(c.t.students) || TERTIARY); addX(c.x, c.y, 'tertiary', n, c.t); stats.tertiary.push({ name: c.t.name || 'Unnamed campus', students: n, src: c.from }); }
      else addX(c.x, c.y, 'childcare', Math.round(count(c.t.capacity) || CHILDREN), c.t);
    }

    // Stats over buildings inside the study area
    const items = [];
    for (const b of blds) {
      if (!b.in) { stats.outside++; continue; }
      stats.buildings++;
      if (b.cls) { stats.used++; stats.byClass[b.cls] = (stats.byClass[b.cls] || 0) + 1; if (b.k && b.q > 0) items.push({ x: b.x, y: b.y, k: b.k, q: b.q, street: b.street }); }
      else { stats.ignored++; stats.ignoredBy[b.why] = (stats.ignoredBy[b.why] || 0) + 1; }
    }
    for (const it of extra) if (inside(it.x, it.y) && it.q > 0) items.push(it);
    const tot = Object.fromEntries(KEYS.map(k => [k, 0]));
    for (const it of items) tot[it.k] += it.q;
    stats.dwellings = Math.round(tot.residential + tot.apartment);
    stats.gfa = { retail: Math.round(tot.retail), office: Math.round(tot.office), industrial: Math.round(tot.industrial), warehouse: Math.round(tot.warehouse), fastfood: Math.round(tot.fastfood) };
    stats.students = Math.round(tot.school); stats.children = Math.round(tot.childcare);
    stats.medical = Math.round(tot.medical); stats.supermarket = Math.round(tot.supermarket);
    stats.tertiaryStudents = Math.round(tot.tertiary); stats.hotelRooms = Math.round(tot.hotel);

    // Aggregate to grid cells
    const nx = Math.max(1, Math.ceil(W / cell)), ny = Math.max(1, Math.ceil(H / cell)), cells = new Map();
    for (const it of items) {
      const i = Math.min(nx - 1, Math.floor(it.x / cell)), j = Math.min(ny - 1, Math.floor(it.y / cell)), key = j * nx + i;
      let c = cells.get(key);
      if (!c) cells.set(key, c = { i, j, w: 0, sx: 0, sy: 0, lu: Object.fromEntries(KEYS.map(k => [k, 0])), streets: {} });
      const w = it.q * WEIGHT[it.k];
      c.w += w; c.sx += it.x * w; c.sy += it.y * w; c.lu[it.k] += it.q;
      if (it.street) c.streets[it.street] = (c.streets[it.street] || 0) + w;
    }
    // Merge cells with little activity into the nearest cell (by activity-weighted centre)
    const live = [...cells.values()].filter(c => c.w > 0).sort((a, b) => b.j - a.j || a.i - b.i);
    for (;;) {
      let small = null;
      for (const c of live) if (c.w < MERGE_BELOW && (!small || c.w < small.w)) small = c;
      if (!small || live.length < 2) break;
      const sx = small.sx / small.w, sy = small.sy / small.w;
      let best = null, bd = Infinity;
      for (const c of live) if (c !== small) { const d = Math.hypot(c.sx / c.w - sx, c.sy / c.w - sy); if (d < bd) { bd = d; best = c; } }
      best.w += small.w; best.sx += small.sx; best.sy += small.sy;
      for (const k of KEYS) best.lu[k] += small.lu[k];
      for (const [s, w] of Object.entries(small.streets)) best.streets[s] = (best.streets[s] || 0) + w;
      live.splice(live.indexOf(small), 1); stats.merged++;
    }

    // Named roads for zones with no addresses
    const roads = [];
    const linkSrc = opts.links || (typeof Store !== 'undefined' && Store.P && Store.sc ? Store.sc.links : null);
    for (const L of Array.isArray(linkSrc) ? linkSrc : Object.values(linkSrc || {}))
      if (L && L.name && L.pts && L.pts.length > 1) roads.push({ name: L.name, xy: L.pts.map(p => X(p[0], p[1])) });
    const nearestRoad = (x, y) => {
      let best = null, bd = 1000;
      for (const r of roads) for (let i = 1; i < r.xy.length; i++) { const d = U.segDist(x, y, r.xy[i - 1][0], r.xy[i - 1][1], r.xy[i][0], r.xy[i][1]).d; if (d < bd) { bd = d; best = r.name; } }
      return best;
    };

    const zones = [], used = {};
    let area = 0;
    for (const c of live) {
      const lu = {};
      for (const k of KEYS) {
        const q = c.lu[k], v = LANDUSES[k] && LANDUSES[k].per === 1 ? Math.round(q) : Math.round(q / 10) * 10;
        if (v > 0) lu[k] = v;
      }
      if (!Object.keys(lu).length) continue;
      const x = c.sx / c.w, y = c.sy / c.w;
      const st = Object.entries(c.streets).sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))[0];
      let base = st ? st[0] : nearestRoad(x, y);
      if (!base) base = 'Area ' + (++area);
      used[base] = (used[base] || 0) + 1;
      const name = 'Existing · ' + base + (used[base] > 1 ? ' ' + used[base] : '');
      zones.push({ id: U.uid('z'), kind: 'zone', existing: true, name, lat: U.r7(B.s + y / ky), lng: U.r7(B.w + x / kx), lu, connectors: null });
    }
    stats.zones = zones.length;

    const A = stats.assumptions;
    A.push('Indicative only: building outlines from OpenStreetMap (LINZ import). Check dwelling and floor-area totals against council or Stats NZ data before relying on them.');
    A.push('Untagged buildings are classed by the land-use area they sit in (residential, commercial, retail, industrial, education); clinics, shops and schools mapped inside a building take precedence.');
    A.push(`Garages, sheds, glasshouses, roofs, tanks and similar are ignored, as are footprints under ${MIN_FOOT} m² and buildings under ${HOUSE_MIN} m² within ${OUTBUILDING_R} m of a larger one (outbuildings).`);
    A.push(`Houses: 1 dwelling each. Residential buildings over ${HOUSE_MAX} m² footprint with mapped levels or height count as apartments at ${M2_PER_DWELLING} m² gross floor area per dwelling; without them, one dwelling per ${M2_PER_UNIT} m² of footprint.`);
    A.push('Floors from building:levels, else height ÷ 3 m, else 1 (buildings tagged apartments 3). Retail, office, industrial and warehouse GFA = footprint × floors.');
    A.push('Shops that are workshops (car repair, tyres) count as industrial; trade and bulky-goods shops (hardware, car yards, furniture, garden centres and similar) count as warehouse; other shops as retail.');
    A.push(`Hotels and motels from the rooms tag, else one room per ${M2_PER_ROOM} m² of floor area; universities and colleges are tertiary campuses (${U.fmt.int(TERTIARY)} students unless tagged); fast food ${FASTFOOD_M2} m² for a point with no building.`);
    A.push(`Commercial areas count as retail where shops are mapped within ${SHOP_NEAR} m, otherwise office.`);
    A.push(`Outside mapped land-use areas, buildings of ${HOUSE_MIN}–${HOUSE_MAX} m² count as rural houses; house-sized buildings within ${RURAL_GROUP} m of a house are its sheds, unless ${DENSE_N} or more buildings stand within ${DENSE_R} m (a street), when each is a dwelling. Others are ignored as farm or yard buildings unless shops are mapped nearby.`);
    A.push(`Shop sites mapped without buildings: ${Math.round(SITE_COVER * 100)}% of the site area as floor area (up to 10,000 m²); car yards, garden centres and trade yards add none.`);
    if (stats.buildings && noArea > stats.buildings / 2) A.push(`Warning: ${U.fmt.int(noArea)} of ${U.fmt.int(stats.buildings)} buildings are outside any mapped land-use area, so they were classed by size alone. Check the totals, or map the residential and industrial areas in OpenStreetMap.`);
    A.push('School rolls from the capacity tag, else 350 (years 1–8 or primary), 800 (College/High or secondary years), else 400 students.');
    A.push(`Early childhood centres ${CHILDREN} children; doctors, clinics and dentists ${MEDICAL_M2} m² GFA each.`);
    if (looseN) A.push(`${looseN} shop or clinic point${looseN > 1 ? 's' : ''} with no building given a default floor area (shop ${SHOP_M2} m², supermarket ${U.fmt.int(SUPERMARKET_M2)} m², clinic ${MEDICAL_M2} m²).`);
    if (stats.ignoredBy['community or other use']) A.push(`${stats.ignoredBy['community or other use']} churches, halls, clubrooms and similar have no Spare Construct land use and are left out.`);
    A.push(`Grouped on a ${cell} m grid at activity-weighted centres; cells under ${MERGE_BELOW} dwelling-equivalents merged into the nearest cell.`);
    return { zones, stats };
  }

  return { fetch: download, build, tiles, students, count, WEIGHT };
})();
