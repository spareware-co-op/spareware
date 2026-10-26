// Records the prototype's existing land use from OpenStreetMap (landuse.js `ExistingLU`) for the Rust port's landuse module
// (projects/spare-construct/crates/construct-transport/src/landuse, tests/landuse_truth.rs). Writes expected/landuse-*.json:
//   landuse-constants.json  the constants and tag sets read from landuse.js's own source (the Rust test compares its copy)
//   landuse-functions.json  tiles, students and count over grids and many tag combinations
//   landuse-build.json      ExistingLU.build on the made-up suburb in fixtures/ (make-landuse-fixtures.mjs): the whole answer, other
//                           study areas, cell sizes, road names, a reversed element order, the odd elements, empty and broken input
//   landuse-download.json   what ExistingLU.fetch asks for (queries and progress messages) and the elements it returns, with the server
//                           answers stubbed from the same files (no network is used), and the errors for areas it refuses
// Zone ids come from the harness's seeded generator (js-core.mjs), as in gen-edit.mjs.
import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore, JS_DIR } from './js-core.mjs';
import { plain, idStart } from './edit-ops.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const read = rel => readFileSync(join(here, rel), 'utf8');
const readJson = rel => JSON.parse(read(rel));
const write = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'expected', name), t); console.log(name, t.length, 'bytes'); };
const NOW = Date.UTC(2026, 9, 21);
const lu = core => core.U.dist.constructor('return ExistingLU')();

let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const uni = (a, b) => a + (b - a) * rnd();
const pick = a => a[Math.floor(rnd() * a.length)];

/* ---------- constants, read from the prototype's source ---------- */
{
  const src = readFileSync(join(JS_DIR, 'landuse.js'), 'utf8');
  const sets = {}, nums = {};
  for (const m of src.matchAll(/const ([A-Z_]+) = new Set\(\[([^\]]*)\]\)/g)) sets[m[1]] = [...m[2].matchAll(/'([^']*)'/g)].map(x => x[1]).sort();
  // `const EDU_B = new Set(['school']), TERT_B = new Set([...])` declares several sets in one statement
  for (const m of src.matchAll(/\b([A-Z_]+) = new Set\(\[([^\]]*)\]\)/g)) sets[m[1]] = [...m[2].matchAll(/'([^']*)'/g)].map(x => x[1]).sort();
  for (const m of src.matchAll(/\b(TILE|LU_TILE|MAX_AREA|MIN_FOOT|HOUSE_MIN|HOUSE_MAX|M2_PER_DWELLING|M2_PER_UNIT|M2_PER_ROOM|OUTBUILDING_R|SHOP_NEAR|RURAL_SHOP_NEAR|POI_SNAP|CAMPUS_R|RURAL_GROUP|DENSE_R|DENSE_N|MEDICAL_M2|CHILDREN|SHOP_M2|SUPERMARKET_M2|FASTFOOD_M2|TERTIARY|SITE_COVER|MERGE_BELOW) = ([0-9.e]+)/g)) nums[m[1]] = Number(m[2]);
  const w = /const WEIGHT = \{([^}]*)\}/.exec(src)[1];
  const weight = Object.fromEntries([...w.matchAll(/(\w+): ([0-9.]+)/g)].map(m => [m[1], Number(m[2])]));
  const lc = /const LU_CLASS = \{([^}]*)\}/.exec(src)[1];
  const luClass = [...lc.matchAll(/(\w+): '/g)].map(m => m[1]).sort();
  // the ignore set is two lines of one statement; the shop sets can span lines
  const wanted = ['IGNORE', 'OTHER_B', 'OTHER_A', 'RES_B', 'RETAIL_B', 'IND_B', 'EDU_B', 'TERT_B', 'CHILD_B', 'EDU_A', 'TERT_A', 'CHILD_A', 'MED_A', 'SHOP_IND', 'SHOP_BULKY', 'SHOP_YARD'];
  const multi = {};
  for (const name of wanted) {
    const m = new RegExp(`\\b${name} = new Set\\(\\[([^\\]]*)\\]\\)`).exec(src);
    if (!m) throw new Error('no set ' + name);
    multi[name] = [...m[1].matchAll(/'([^']*)'/g)].map(x => x[1]).sort();
  }
  write('landuse-constants.json', { ...nums, WEIGHT: weight, KEYS: Object.keys(weight), ...multi, LU_CLASS: luClass });
}

/* ---------- the pure functions ---------- */
{
  const core = loadCore({ seed: 1 });
  const L = lu(core);
  const f = {};
  const boxes = [{ s: -36.975, w: 174.852, n: -36.965, e: 174.868 }, { s: -37.0, w: 174.8, n: -36.96, e: 174.88 }, { s: -37.1, w: 174.7, n: -36.9, e: 174.95 }, { s: 0, w: 0, n: 0.0001, e: 0.0002 },
    { s: -36.9, w: 174.9, n: -36.9001, e: 174.9001 }, { s: -45.0, w: 168.0, n: -44.9, e: 168.4 }, { s: 60, w: 10, n: 60.5, e: 11 }, { s: -36.8, w: 174.7, n: -36.8 + 3000 / 110574, e: 174.7 + 3000 / 89000 }];
  f.tiles = [];
  for (const b of boxes) for (const edge of [undefined, 3000, 9000, 1000, 250.5]) { const out = L.tiles(b, edge); if (out.length <= 120) f.tiles.push({ b, edge: edge ?? null, out }); }
  f.tiles.push({ b: [[-36.9, 174.7], [-36.8, 174.8]], edge: null, out: L.tiles([[-36.9, 174.7], [-36.8, 174.8]]), corners: true });
  f.count = ['1,200', '1 450', '12,345,678', '1,2', '1,234', '1 234 567', '12', '', ' 7 ', '3.5', '3,5', '1,2345', '12 3456', '0', '-5', '1e3', 'many', '1,200 pupils', '１２', '1 450', '  800  ', '+9', '.5', '5.', 'Infinity', '12,3,4', '1,234,56', '0,5', '0,000'].map(s => [s, L.count(s)]);
  f.count.push([null, L.count(null)], [undefined, L.count(undefined)], [450, L.count(450)], [true, L.count(true)]);
  const names = [undefined, '', 'Papatoetoe High School', 'Manukau College', 'highland primary', 'Highfield', 'Kura Kaupapa', 'KURA', 'Intermediate School', 'primary', 'Primary School', 'Eastern High', 'underhigh', 'rehigh', 'COLLEGE', 'colleges', 'Te Kura o Ōtara', 'High_School', 'High-School'];
  const years = [undefined, '1-8', '9-13', '1-13', '7-8', 'Years 9 to 13', '0', '7;8;9', '7-10', '1', '13', '', 'x', '9-9', '1 - 6', 'Y1-Y8'];
  const iscs = [undefined, '1', '2', '3', '2;3', '0'];
  const caps = [undefined, '', '350', '1,200', 'x', '0', '-3', '60.4', '12.5'];
  f.students = [];
  for (const name of names) for (const yr of years.slice(0, 8)) for (const isc of iscs.slice(0, 3)) f.students.push([{ ...(name === undefined ? {} : { name }), ...(yr === undefined ? {} : { 'MOE:years': yr }), ...(isc === undefined ? {} : { 'isced:level': isc }) }]);
  for (const yr of years) for (const g of years.slice(0, 4)) f.students.push([{ ...(yr === undefined ? {} : { grades: yr }), ...(g === undefined ? {} : { 'school:years': g }) }]);
  for (const c of caps) for (const s of caps) for (const p of caps.slice(0, 5)) f.students.push([{ ...(c === undefined ? {} : { capacity: c }), ...(s === undefined ? {} : { students: s }), ...(p === undefined ? {} : { 'capacity:pupils': p }), name: 'Plain School' }]);
  for (const isc of iscs) for (const yr of years) f.students.push([{ ...(isc === undefined ? {} : { 'isced:level': isc }), ...(yr === undefined ? {} : { 'MOE:years': yr }) }]);
  f.students = f.students.map(([t]) => [t, L.students(t)]);
  f.weight = L.WEIGHT;
  write('landuse-functions.json', plain(f));
}

/* ---------- build ---------- */
const full = readJson('fixtures/landuse-papatoetoe.json');
const odd = readJson('fixtures/landuse-odd.json');
const O = [-36.97, 174.86];
const KM = 111320 * Math.cos(O[0] * Math.PI / 180), KY = 110574;
const boxXY = (x0, y0, x1, y1) => ({ s: O[0] + y0 / KY, w: O[1] + x0 / KM, n: O[0] + y1 / KY, e: O[1] + x1 / KM });
const STUDY = boxXY(-700, -550, 700, 550);
const ROADS = [
  { name: 'Great South Road', pts: [[O[0] + 40 / KY, O[1] - 400 / KM], [O[0] + 40 / KY, O[1]], [O[0] + 40 / KY, O[1] + 400 / KM]] },
  { name: 'Station Road', pts: [[O[0] - 300 / KY, O[1] + 300 / KM], [O[0] + 100 / KY, O[1] + 300 / KM]] },
  { name: 'Only One Point', pts: [[O[0], O[1]]] },
  { name: '', pts: [[O[0], O[1]], [O[0] + 1 / KY, O[1]]] },
  { name: 'North South Street', pts: [[O[0] - 500 / KY, O[1] - 500 / KM], [O[0] + 500 / KY, O[1] - 500 / KM]] },
  { name: 'Far Away Road', pts: [[O[0] + 5000 / KY, O[1]], [O[0] + 5100 / KY, O[1]]] },
];
const cases = [];
const add = (name, elements, bounds, opts = {}, seedNo = 700 + cases.length) => cases.push({ name, elements, bounds, opts, seed: seedNo });
add('whole answer', full.elements, STUDY, { links: [] });
add('answer object', full, STUDY, { links: [] });
add('no bounds', full.elements, null, { links: [] });
add('invalid bounds', full.elements, { s: 1, w: 1, n: 0, e: 2 }, { links: [] });
add('small area', full.elements, boxXY(-300, -200, 300, 200), { links: [] });
add('west half', full.elements, boxXY(-700, -550, 0, 550), { links: [] });
add('cell 150', full.elements, STUDY, { cell: 150, links: [] });
add('cell 20', full.elements, STUDY, { cell: 20, links: [] });
add('cell 1000', full.elements, STUDY, { cell: 1000, links: [] });
add('cell text', full.elements, STUDY, { cell: 'abc', links: [] });
add('cell numeric text', full.elements, STUDY, { cell: '250', links: [] });
add('road names', full.elements, STUDY, { links: ROADS });
add('road names small cells', full.elements, STUDY, { cell: 100, links: ROADS });
add('reversed order', [...full.elements].reverse(), STUDY, { links: [] });
add('doubled', [...full.elements, ...full.elements.slice(0, 100)], STUDY, { links: [] });
add('buildings only', full.elements.filter(e => e.tags && e.tags.building), STUDY, { links: [] });
add('areas and points only', full.elements.filter(e => !(e.tags && e.tags.building) || e.type === 'node'), STUDY, { links: [] });
add('houses only', full.elements.filter(e => e.tags && (e.tags.building === 'yes' || e.tags.building === 'house')), STUDY, { links: ROADS });
add('odd', odd.elements, boxXY(-100, -900, 700, 100), { links: [] });
add('odd no bounds', odd.elements, null, { links: [] });
add('odd with roads', odd.elements, { s: -36.983, w: 174.859, n: -36.9695, e: 174.873 }, { links: [{ name: 'Odd Road', pts: [[-36.9775, 174.8665], [-36.9765, 174.8675]] }] });
add('empty list', [], STUDY, { links: [] });
add('empty object', {}, STUDY, { links: [] });
add('null elements', null, STUDY, { links: [] });
add('no usable elements', [{ type: 'relation', id: 1 }, { type: 'node', id: 2, lat: 1, lon: 1 }, { foo: 1 }], null, { links: [] });
add('one house no bounds', [{ type: 'way', id: 9, geometry: [{ lat: -36.97, lon: 174.86 }, { lat: -36.97, lon: 174.8601 }, { lat: -36.9701, lon: 174.8601 }, { lat: -36.9701, lon: 174.86 }], tags: { building: 'house' } }], null, { links: [] });
add('houses in a line', Array.from({ length: 12 }, (_, i) => ({ type: 'way', id: 100 + i, geometry: [[0, 0], [1, 0], [1, 1], [0, 1]].map(([a, b]) => ({ lat: -36.97 - a * 0.0001, lon: 174.86 + i * 0.00012 + b * 0.0001 })), tags: { building: 'house', 'addr:street': i % 3 ? 'Line Road' : 'Row Street' } })), null, { links: [] });
// Random suburbs: many elements with random shapes and tags, to reach the branches the made-up suburb does not
function randomElements(seedNo, n) {
  let st = seedNo >>> 0;
  const r = () => { st = (st + 0x6D2B79F5) >>> 0; let t = st; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const u = (a, b) => a + (b - a) * r(), p = a => a[Math.floor(r() * a.length)];
  const at = (x, y) => ({ lat: Math.round((O[0] + y / KY) * 1e7) / 1e7, lon: Math.round((O[1] + x / KM) * 1e7) / 1e7 });
  const rect = (cx, cy, w, h, rot = 0) => { const a = rot * Math.PI / 180, c = Math.cos(a), s2 = Math.sin(a); return [[-w / 2, -h / 2], [w / 2, -h / 2], [w / 2, h / 2], [-w / 2, h / 2]].map(([x, y]) => [cx + x * c - y * s2, cy + x * s2 + y * c]); };
  const els = []; let id = 1000;
  const way = (tags, pts) => { const g = pts.map(([x, y]) => at(x, y)); g.push(g[0]); els.push({ type: 'way', id: id += 1 + Math.floor(r() * 50), geometry: g, tags }); };
  const point = (tags, x, y) => els.push({ type: 'node', id: id += 1 + Math.floor(r() * 50), ...at(x, y), tags });
  const W = u(300, 900), H = u(300, 900);
  // areas
  for (let i = 0; i < 4 + Math.floor(r() * 5); i++) {
    const cx = u(-W, W), cy = u(-H, H), w = u(80, 400), h = u(80, 400);
    const kind = p(['landuse', 'landuse', 'landuse', 'site']);
    const tags = kind === 'landuse' ? { landuse: p(['residential', 'residential', 'commercial', 'retail', 'industrial', 'education', 'farmland']) }
      : p([{ amenity: 'school', name: p(['North Primary', 'East High School', 'Kura', '']), ...(r() < 0.4 ? { 'MOE:years': p(['1-8', '9-13', '7-10']) } : {}) }, { amenity: 'college', capacity: p(['1,500', '300', 'x']) }, { amenity: 'kindergarten' }, { amenity: 'clinic' }, { amenity: 'hospital' },
        { tourism: 'hotel', ...(r() < 0.5 ? { rooms: String(10 + Math.floor(r() * 90)) } : {}) }, { tourism: 'motel' }, { shop: p(['supermarket', 'car', 'garden_centre', 'bakery', 'hardware', 'car_repair']) }, { amenity: 'fast_food' }]);
    way(tags, rect(cx, cy, w, h, u(-30, 30)));
  }
  // buildings
  const BT = ['yes', 'yes', 'yes', 'yes', 'yes', 'yes', 'house', 'house', 'detached', 'apartments', 'terrace', 'residential', 'commercial', 'retail', 'office', 'industrial', 'warehouse', 'factory', 'school', 'university', 'kindergarten', 'church', 'garage', 'shed', 'hotel', 'hospital', 'supermarket', 'farm_auxiliary', 'roof', 'no', 'kiosk', 'bungalow', 'public'];
  for (let i = 0; i < n; i++) {
    const tags = { building: p(BT) };
    if (r() < 0.15) tags['building:levels'] = p(['1', '2', '3', '5', 'x', '12', '70', '2.6']);
    if (r() < 0.08) tags.height = p(['4', '9', '15.5', 'tall', '40']);
    if (r() < 0.1) tags.amenity = p(['doctors', 'dentist', 'school', 'fast_food', 'library', 'place_of_worship', 'fuel', 'hospital', 'kindergarten', 'community_centre']);
    if (r() < 0.12) tags.shop = p(['supermarket', 'bakery', 'car_repair', 'hardware', 'car', 'clothes', 'convenience', 'tyres']);
    if (r() < 0.04) tags.office = 'company';
    if (r() < 0.03) tags.craft = 'carpenter';
    if (r() < 0.03) tags.industrial = 'warehouse';
    if (r() < 0.03) tags.man_made = p(['storage_tank', 'silo', 'chimney']);
    if (r() < 0.02) tags.aeroway = 'hangar';
    if (r() < 0.03) tags.tourism = p(['hotel', 'motel', 'attraction']);
    if (r() < 0.05) tags.rooms = p(['10', '3', '1,200', 'x']);
    if (r() < 0.04) { tags.name = p(['Greenwood High', 'Hillcrest Primary', 'Some College', 'The Centre']); tags.capacity = p(['500', '1 200', '40', 'many']); }
    if (r() < 0.5) tags['addr:street'] = p(['Alpha Road', 'Beta Street', 'Gamma Avenue', '12th Street']);
    const big = r() < 0.12;
    way(tags, rect(u(-W, W), u(-H, H), big ? u(20, 80) : u(4, 16), big ? u(15, 60) : u(4, 14), u(-45, 45)));
  }
  // points
  for (let i = 0; i < Math.floor(n / 8); i++) {
    point(p([{ shop: 'bakery' }, { shop: 'supermarket' }, { shop: 'car_repair' }, { shop: 'hardware' }, { amenity: 'fast_food' }, { amenity: 'doctors' }, { amenity: 'dentist' }, { amenity: 'school', name: 'Point School' }, { amenity: 'kindergarten', capacity: '30' }, { amenity: 'college' }, { amenity: 'hospital' },
      { tourism: 'hotel', rooms: '20' }, { tourism: 'motel' }, { amenity: 'cafe' }]), u(-W * 1.05, W * 1.05), u(-H * 1.05, H * 1.05));
  }
  // a few relations
  for (let i = 0; i < 3; i++) {
    const cx = u(-W, W), cy = u(-H, H), w = u(20, 60), h = u(15, 40);
    const outer = rect(cx, cy, w, h).map(([x, y]) => at(x, y)), inner = rect(cx, cy, w / 3, h / 3).map(([x, y]) => at(x, y));
    outer.push(outer[0]); inner.push(inner[0]);
    els.push({ type: 'relation', id: id += 7, members: [{ type: 'way', ref: 1, role: 'outer', geometry: outer }, ...(r() < 0.6 ? [{ type: 'way', ref: 2, role: 'inner', geometry: inner }] : [])], tags: { type: 'multipolygon', building: p(['yes', 'apartments', 'commercial', 'industrial']) } });
  }
  return { els, W, H };
}
for (let k = 0; k < 10; k++) {
  const { els, W, H } = randomElements(31000 + k, 90 + 20 * (k % 4));
  const b = k % 3 === 0 ? null : boxXY(-W * 1.1, -H * 1.1, W * 1.1, H * 1.1);
  const links = k % 2 ? [{ name: 'Alpha Road', pts: [[O[0] + 100 / KY, O[1] - 300 / KM], [O[0] + 100 / KY, O[1] + 300 / KM]] }, { name: 'Beta Street', pts: [[O[0] - 200 / KY, O[1]], [O[0] + 200 / KY, O[1] + 50 / KM]] }] : [];
  add('random ' + k, els, b, { links, ...(k % 4 === 1 ? { cell: 120 } : k % 4 === 2 ? { cell: 800 } : {}) }, 720 + k);
}
const results = cases.map(c => {
  const core = loadCore({ seed: c.seed });
  const L = lu(core);
  const el = c.elements === undefined ? undefined : JSON.parse(JSON.stringify(c.elements));
  const out = L.build(el, c.bounds, { ...c.opts });
  return { name: c.name, seed: c.seed, now: NOW, ...idStart(c.seed), bounds: c.bounds, opts: c.opts, file: c.name, zones: plain(out.zones), stats: plain(out.stats) };
});
const summary = results.map(r => `${r.name}: ${r.zones.length} zones, ${r.stats.buildings} buildings, ${r.stats.used} used, ${r.stats.dwellings} dwellings`);
console.log(summary.join('\n'));
// The inputs are the fixture files; the case list says which
write('landuse-build.json', { cases: results.map((r, i) => ({ ...r, input: inputOf(cases[i]) })) });

// How each case's elements are made from the fixtures, so the Rust test builds the same input
function inputOf(c) {
  const e = c.elements;
  if (e === null || e === undefined) return { kind: 'null' };
  if (e === full) return { kind: 'object', file: 'landuse-papatoetoe.json' };
  if (e === odd.elements) return { kind: 'list', file: 'landuse-odd.json' };
  if (Array.isArray(e) && e.length === 0) return { kind: 'list', empty: true };
  if (!Array.isArray(e)) return { kind: 'inline', value: e };
  if (e === full.elements) return { kind: 'list', file: 'landuse-papatoetoe.json' };
  const name = c.name;
  if (name === 'reversed order') return { kind: 'list', file: 'landuse-papatoetoe.json', reversed: true };
  if (name === 'doubled') return { kind: 'list', file: 'landuse-papatoetoe.json', doubled: 100 };
  if (name === 'buildings only') return { kind: 'list', file: 'landuse-papatoetoe.json', filter: 'buildings' };
  if (name === 'areas and points only') return { kind: 'list', file: 'landuse-papatoetoe.json', filter: 'areas' };
  if (name === 'houses only') return { kind: 'list', file: 'landuse-papatoetoe.json', filter: 'houses' };
  return { kind: 'inline', value: e };
}

/* ---------- download ---------- */
{
  const files = ['buildings-0', 'buildings-1', 'areas-0', 'areas-1'].map(n => readJson(`fixtures/landuse-papatoetoe-${n}.json`));
  const real = globalThis.fetch;
  const runs = [];
  const run = async (name, box, answerFor) => {
    const core = loadCore({ seed: 3 });
    const L = lu(core);
    const queries = [], statuses = [];
    let nb = 0, na = 0;
    globalThis.fetch = async (url, init) => {
      const q = decodeURIComponent(init.body.slice(5));
      queries.push({ url, q });
      const isB = q.includes('way["building"]');
      const k = isB ? nb++ : na++;
      const answer = answerFor(isB, k, url);
      return { ok: true, status: 200, json: async () => answer };
    };
    let elements = null, error = null;
    try { elements = await L.fetch(box, t => statuses.push(t)); } catch (e) { error = e.message; }
    globalThis.fetch = real;
    runs.push({ name, box, queries, statuses, elements, error });
  };
  const slice = (isB, k) => files[(isB ? 0 : 2) + (k % 2)];
  await run('one tile each', STUDY, slice);
  await run('four building tiles', boxXY(-2500, -2000, 2500, 2000), slice);
  await run('many tiles', boxXY(-7000, -7000, 7000, 7000), slice);
  await run('too big', boxXY(-12000, -12000, 12000, 12000), slice);
  await run('invalid', { s: 1, w: 1, n: 0, e: 2 }, slice);
  await run('not a number', { s: NaN, w: 1, n: 2, e: 3 }, slice);
  await run('remarks', STUDY, () => ({ remark: 'runtime error: Query timed out in "query" at line 4 after 181 seconds.', elements: [] }));
  await run('remark out of memory', STUDY, () => ({ remark: 'runtime error: Query run out of memory using about 2048 MB of RAM.', elements: [{ type: 'node', id: 1, lat: 0, lon: 0 }] }));
  await run('harmless remark', STUDY, (isB, k) => ({ remark: 'note: slow server', elements: files[(isB ? 0 : 2) + (k % 2)].elements.slice(0, 5) }));
  await run('no elements key', STUDY, () => ({}));
  write('landuse-download.json', { runs });
}
