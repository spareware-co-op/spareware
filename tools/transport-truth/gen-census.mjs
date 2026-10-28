// Records the prototype's land use feature (packs/nz/landuse-plus.js: Census, employees, schools, zoning, trip generation, yield and development
// traffic) for the Rust port's census module (projects/spare-construct/crates/construct-transport/src/census, tests/census_*.rs).
//
// No network is used. The downloaded answers are made up here (hand-written layouts, deterministic): Stats NZ Census 2023 layers by SA1 with the
// real field codes and aliases, employees by SA2, Ministry of Education directory records, Auckland Unitary Plan zones and the OpenStreetMap
// buildings of the made-up suburb in fixtures/landuse-papatoetoe.json. The fixtures are written to fixtures/data/census-*.json; the prototype
// then runs on them with `fetch` stubbed (every request it makes is recorded with the answer it was given) and each result is written to:
//   expected/census-constants.json  SRC, STD, YIELDS, CHOICES, CENSUS_VARS, MODES, CSV_HEAD as the module exports them
//   expected/census-functions.json  parseNum, lr, sampleShare, resolve, parsers, sharesOf, splitFor, rateFlag, yieldOf, zonedCapacity, cardCheck and more
//   expected/census-fetch.json      fetchCensus, fetchEmployees, fetchMoE, fetchAUP: requests, status messages and results, and the failures
//   expected/census-apply.json      apply on a project of existing zones: reports and the zones afterwards, with variations
//   expected/census-fill.json       fillAll end to end (requests, report, project, school rolls) and ExistingLU.fetch's server fallback
//   expected/census-trips.json      tripGen, tables, csv, sync, setTable, syncAll on many zones and settings, and the report data
//   expected/census-traffic.json    distribution, withWithout, withoutScenario, gwSrcText, useCount
// Zone ids come from the harness's seeded generator (as gen-landuse.mjs). Run: node gen-census.mjs (the same input always gives the same files).
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const NOW = Date.UTC(2026, 9, 21);
const CORE = ['util', 'model', 'registry', 'engine', 'data', 'landuse', 'turns', 'calibrate', 'sim', 'sample', 'kit', 'packs/nz/landuse-plus'];
const inert = () => new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive ? () => '' : inert()), apply: () => undefined });
mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });
mkdirSync(join(here, 'expected'), { recursive: true });
const readJson = rel => JSON.parse(readFileSync(join(here, rel), 'utf8'));
const writeData = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'fixtures', 'data', name), t); console.log('fixtures/data/' + name, t.length, 'bytes'); };
const writeExpected = (name, data) => { const t = JSON.stringify(data); writeFileSync(join(here, 'expected', name), t); console.log('expected/' + name, t.length, 'bytes'); };

// JSON as the saved project and the report would hold it: undefined left out, non-finite numbers null
export const jsonSafe = v => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)));

// js-core.mjs's loader with kit.js and packs/nz/landuse-plus.js after the core; fetchImpl stands in for the network
export function load({ seed = 1, now = NOW, fetchImpl = null } = {}) {
  let s = seed >>> 0;
  const rand = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const M = Object.create(Math); M.random = rand;
  const store = new Map();
  const ctx = {
    console, JSON, Promise, Map, Set, WeakMap, WeakSet, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError,
    Float64Array, Float32Array, Int32Array, Uint32Array, Uint8Array, Int8Array, Uint16Array, ArrayBuffer, DataView, Intl, RegExp,
    Math: M, Date: class extends Date { constructor(...a) { super(...(a.length ? a : [now])); } static now() { return now; } },
    setTimeout, clearTimeout, queueMicrotask, structuredClone, TextEncoder, TextDecoder, URL, URLSearchParams, AbortController,
    performance: { now: () => 0 }, fetch: fetchImpl || ((...a) => { throw new Error('no network in the truth tests: ' + a[0]); }), isFinite, isNaN, parseFloat, parseInt, Infinity, NaN, encodeURIComponent, decodeURIComponent,
    localStorage: { getItem: k => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: k => store.delete(k) },
    UI: inert(), Render: inert(), Inspector: inert(), Dialogs: inert(), Drawer: inert(),
    CSS: { escape: x => String(x) },
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const src = CORE.map(n => `/* ${n}.js */\n` + readFileSync(join(JS_DIR, n + '.js'), 'utf8')).join('\n;\n');
  vm.runInContext(src + `\n;globalThis.__core = { U, Store, Net, Sites, Scenarios, Importers, Engine, DataHub, Register, LandusePlus, ExistingLU, Kit, newProject, newScenario, defaultSettings, migrate, CLASSES, LANDUSES };`, ctx, { filename: 'transport-census.js' });
  return ctx.__core;
}

// How many numbers loading the scripts draws, and where the id counter stands (one probe id from a fresh load)
export function idStart(seed) {
  const c = load({ seed, now: NOW });
  const id = c.U.uid('t');
  let s = seed >>> 0;
  const draws = Array.from({ length: 12 }, () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; });
  const m = /^t(.{4})([0-9a-z]+?)([0-9a-z]{3})$/.exec(id);
  const skip = draws.findIndex(d => d.toString(36).slice(2, 5) === m[3]);
  if (skip < 0) throw new Error('the id does not come from the seeded generator: ' + id);
  return { rngSkip: skip, seq0: parseInt(m[2], 36) };
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Made-up downloaded answers around the made-up suburb of fixtures/landuse-papatoetoe.json (origin O, metres east and north of it)
   ------------------------------------------------------------------------------------------------------------------------------------ */
let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const uni = (a, b) => a + (b - a) * rnd();
const O = [-36.97, 174.86];
const KM = 111320 * Math.cos(O[0] * Math.PI / 180), KY = 110574;
const r6 = x => Math.round(x * 1e6) / 1e6;
const ll = (x, y) => [r6(O[1] + x / KM), r6(O[0] + y / KY)];            // ArcGIS order: [lng, lat]
const ring = pts => { const r = pts.map(([x, y]) => ll(x, y)); r.push(r[0]); return r; };
const rectXY = (x0, y0, x1, y1, mid = false) => (mid ? [[x0, y0], [(x0 + x1) / 2 + uni(-30, 30), y0 + uni(-8, 8)], [x1, y0], [x1 + uni(-8, 8), (y0 + y1) / 2], [x1, y1], [(x0 + x1) / 2, y1 + uni(-8, 8)], [x0, y1], [x0 + uni(-8, 8), (y0 + y1) / 2]] : [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]);
const STUDY = (() => { const b = { s: O[0] - 550 / KY, w: O[1] - 700 / KM, n: O[0] + 550 / KY, e: O[1] + 700 / KM }; return b; })();
const base3 = x => Math.max(0, Math.round(x / 3) * 3);                  // Stats NZ random-rounds counts to base 3
const suburb = readJson('fixtures/landuse-papatoetoe.json');

// Census 2023 layers: the fields carry the alias text and the year the prototype resolves them by
const CAT_WORK = ['Work at home', 'Did not go to work today', 'Drive a private car, truck or van', 'Drive a company car, truck or van', 'Passenger in a car, truck, van or company bus', 'Public bus', 'Train', 'Bicycle', 'Walk or jog', 'Ferry', 'Other', null, null, 'Total stated'];
const CAT_EDU = ['Study at home', 'Drive a car, truck or van', 'Passenger in a car, truck or van', 'Bicycle', 'Walk or jog', 'School bus', 'Public bus', 'Train', 'Ferry', 'Other', null, null, 'Total Stated'];
const VEH = ['No motor vehicle', 'One motor vehicle', 'Two motor vehicles', 'Three motor vehicles', 'Four motor vehicles', 'Five or more motor vehicles'];
const fieldList = (items, extra = []) => [
  { name: 'OBJECTID', type: 'esriFieldTypeOID', alias: 'OBJECTID' }, { name: 'SA12023_V1_00', type: 'esriFieldTypeString', alias: 'SA1 2023 code' },
  { name: 'SA12023_V1_00_NAME', type: 'esriFieldTypeString', alias: 'SA1 2023 name' },
  ...items.map(([code, alias, year = 2023]) => ({ name: code, type: 'esriFieldTypeInteger', alias: `Census ${year} Year: ${year} ${alias}` })), ...extra,
  { name: 'Shape__Area', type: 'esriFieldTypeDouble', alias: 'Shape__Area' }, { name: 'Shape__Length', type: 'esriFieldTypeDouble', alias: 'Shape__Length' }];

// The SA1s: a 4 by 3 grid over the suburb; the study area (1.4 by 1.1 km) cuts several of them
const XS = [-1000, -560, -130, 330, 1000], YS = [-750, -300, 200, 750];
const resBuildings = suburb.elements.filter(e => e.type === 'way' && e.geometry && e.tags && ['yes', 'house', 'detached', 'terrace', 'apartments', 'residential', 'bungalow'].includes(e.tags.building));
const centreOf = e => { const g = e.geometry; return [(g.reduce((s, p) => s + p.lon, 0) / g.length - O[1]) * KM, (g.reduce((s, p) => s + p.lat, 0) / g.length - O[0]) * KY]; };
const sa1s = [];
for (let j = 0; j < 3; j++) for (let i = 0; i < 4; i++) {
  const x0 = XS[i], x1 = XS[i + 1], y0 = YS[j], y1 = YS[j + 1], k = j * 4 + i;
  const nb = resBuildings.filter(e => { const [x, y] = centreOf(e); return x >= x0 && x < x1 && y >= y0 && y < y1; }).length;
  const rural = k === 11 || k === 3;
  const factor = [0.92, 1.0, 1.08, 0.85, 1.12, 0.97, 0.9, 1.2, 1.0, 0.95, 1.03, 0.8][k];
  const occ = base3((nb > 0 ? nb : (rural ? 12 : 30)) * factor), tot = base3(occ * 1.05 + 3);
  const joined = base3(occ * [0.1, 0.45, 0.2, 0.0, 0.3, 0.08, 0.15, 0.6, 0.05, 0.1, 0.25, 0.02][k]), other = base3(occ * 0.02), sep = Math.max(0, base3(occ - joined - other + 3));
  const hh = base3(occ * 0.98), pop = base3(hh * uni(2.6, 3.4));
  const veh = [0.06, 0.34, 0.38, 0.14, 0.06, 0.02].map(f => base3(hh * f));
  const stated = veh.reduce((a, b) => a + b, 0);
  const employed = base3(pop * 0.46), mode = (tot, shares) => shares.map(f => base3(tot * f));
  const work = (n, k2) => { const m = mode(n, [0.05, 0.03, 0.62, 0.04, 0.05, 0.07, 0.03, 0.01, 0.04, 0.003, 0.01]); const st = m[0] + m.slice(2).reduce((a, b) => a + b, 0); return [...m, 0, 0, st]; };
  const wByRes = work(employed), wByWork = work(base3(employed * uni(0.6, 1.3)));
  const students = base3(pop * 0.2), edu = [0.06, 0.18, 0.12, 0.02, 0.2, 0.18, 0.1, 0.1, 0.01, 0.02].map(f => base3(students * f));
  const eduStated = edu[0] + edu.slice(1).reduce((a, b) => a + b, 0);
  sa1s.push({ code: `700${String(1000 + k * 7)}`, k, rings: [ring(rectXY(x0, y0, x1, y1, k % 3 === 1))], occ, tot, sep, joined, other, hh, pop, veh, stated, employed, wByRes, wByWork, edu: [...edu, 0, 0, eduStated] });
}
// one SA1 has a reserve cut out of it (a hole, wound the other way as ArcGIS does)
sa1s[5].rings.push(ring([[-60, -200], [-60, -120], [60, -120], [60, -200]]).reverse());
// suppressed cells are small negative numbers; one SA1 has no dwelling count at all
const SUPPRESSED = -999;

const attrsFor = (a, parts) => {
  const o = { OBJECTID: 1 + sa1s.indexOf(a), SA12023_V1_00: a.code, SA12023_V1_00_NAME: a.code };
  Object.assign(o, parts);
  return o;
};
const dwItems = [['VAR_3_54', 'Dwelling occupancy status (Occupied Dwelling)'], ['VAR_3_55', 'Dwelling occupancy status (Unoccupied Dwelling)'], ['VAR_3_59', 'Dwelling occupancy status (Total)'], ['VAR_3_82', 'Dwelling type (Separate house)'], ['VAR_3_83', 'Dwelling type (Joined dwelling)'], ['VAR_3_84', 'Dwelling type (Other private dwelling)']];
const dwOld = [['VAR_3_54', 'Dwelling occupancy status (Occupied Dwelling)', 2018]];
const dwAttrs = a => attrsFor(a, { VAR_3_54: a.k === 7 ? SUPPRESSED : a.occ, VAR_3_55: 3, VAR_3_59: a.tot, VAR_3_82: a.sep, VAR_3_83: a.joined, VAR_3_84: a.other });
const hhItems = [['VAR_4_3', 'Count of households in occupied private dwellings (Total)'], ...VEH.map((c, i) => [`VAR_4_${136 + i}`, `Number of motor vehicles (${c})`]), ['VAR_4_144', 'Number of motor vehicles (Total stated)']];
const hhAttrs = a => attrsFor(a, { VAR_4_3: a.hh, ...Object.fromEntries(a.veh.map((v, i) => [`VAR_4_${136 + i}`, v])), VAR_4_144: a.stated });
const ind0Items = [['VAR_1_1', 'Census usually resident population count (Male)'], ['VAR_1_3', 'Var1: Census usually resident population count (Total)']];
const ind0Attrs = a => attrsFor(a, { VAR_1_1: base3(a.pop / 2), VAR_1_3: a.k === 2 ? SUPPRESSED : a.pop });
const ind2Items = [['VAR_2_677', 'Industry, by workplace address (Total)'],
  ...CAT_WORK.map((c, i) => c && [`VAR_2_${773 + i}`, `Main means of travel to work, by usual residence address (${c})`]).filter(Boolean),
  ...CAT_WORK.map((c, i) => c && [`VAR_2_${815 + i}`, `Main means of travel to work, by workplace address (${c})`]).filter(Boolean),
  ...CAT_EDU.map((c, i) => c && [`VAR_2_${195 + i}`, `Main means of travel to education, by education address (${c})`]).filter(Boolean)];
const ind2Attrs = a => attrsFor(a, {
  VAR_2_677: a.employed,
  ...Object.fromEntries(CAT_WORK.map((c, i) => c && [`VAR_2_${773 + i}`, a.wByRes[i]]).filter(Boolean)),
  ...Object.fromEntries(CAT_WORK.map((c, i) => c && [`VAR_2_${815 + i}`, a.wByWork[i]]).filter(Boolean)),
  ...Object.fromEntries(CAT_EDU.map((c, i) => c && [`VAR_2_${195 + i}`, a.edu[i]]).filter(Boolean)),
});
const layerFeatures = (attrs, geometry) => sa1s.map(a => ({ attributes: attrs(a), ...(geometry ? { geometry: { rings: a.rings } } : {}) }));
const CENSUS_LAYERS = {
  dw: { fields: fieldList(dwItems, dwOld.map(([c, a, y]) => ({ name: c + '_2018', type: 'esriFieldTypeInteger', alias: `Census ${y} Year: ${y} ${a}` }))), attrs: dwAttrs },
  hh: { fields: fieldList(hhItems), attrs: hhAttrs },
  ind0: { fields: fieldList(ind0Items), attrs: ind0Attrs },
  ind2: { fields: fieldList(ind2Items), attrs: ind2Attrs },
};

// Employees by SA2: three polygons, three years of counts and Stats NZ's average annual change
const SA2 = [{ code: '600101', name: 'Papatoetoe West', box: [-1100, -800, -50, 800], ec: [2210, 2259, 2310], g: 2.1 }, { code: '600102', name: 'Papatoetoe East', box: [-50, -800, 1100, 300], ec: [4300, 4506, 4680], g: 1.4 },
  { code: '600103', name: 'Cambria Park', box: [-50, 300, 1100, 800], ec: [1290, 1311, 1230], g: -0.6 }];
const EMP_FIELDS = [{ name: 'OBJECTID', type: 'esriFieldTypeOID', alias: 'OBJECTID' }, { name: 'SA22023_V1_00', type: 'esriFieldTypeString', alias: 'SA2 2023 code' }, { name: 'SA22023_V1_00_NAME', type: 'esriFieldTypeString', alias: 'SA2 2023 name' },
  ...[2023, 2024, 2025].map(y => ({ name: `ec${y}`, type: 'esriFieldTypeInteger', alias: `Employee count ${y}` })), { name: 'ec_avperinc', type: 'esriFieldTypeDouble', alias: 'Average annual change' }, { name: 'ec2025_note', type: 'esriFieldTypeString', alias: 'note' }];
const empFeatures = SA2.map((a, i) => ({ attributes: { OBJECTID: i + 1, SA22023_V1_00: a.code, SA22023_V1_00_NAME: a.name, ec2023: a.ec[0], ec2024: a.ec[1], ec2025: a.ec[2], ec_avperinc: a.g }, geometry: { rings: [ring(rectXY(...a.box))] } }));

// Ministry of Education directory (CKAN datastore records): schools and early learning services
const SCHOOL_RECORDS = [
  { School_Id: 1421, Org_Name: 'Papatoetoe Intermediate', Org_Type: 'Intermediate (Year 7-8)', Total: 612, Roll_Date: '2025-07-01T00:00:00', Latitude: O[0] + 250 / KY, Longitude: O[1] - 370 / KM, Status: 'Open' },
  { School_Id: 1422, Org_Name: 'Cambria Primary School', Org_Type: 'Contributing', Total: 281, Roll_Date: '2025-07-01', Latitude: O[0] + 410 / KY, Longitude: O[1] - 595 / KM, Status: 'Open' },
  { School_Id: 1423, Org_Name: 'Station Road Kura', Org_Type: 'Composite', Total: null, Roll_Date: '', Latitude: O[0] - 60 / KY, Longitude: O[1] + 120 / KM, Status: 'Open' },
  { School_Id: 1424, Org_Name: 'Old Mill School', Org_Type: 'Full Primary', Total: 40, Roll_Date: '2019-07-01', Latitude: O[0] - 120 / KY, Longitude: O[1] - 200 / KM, Status: 'Closed' },
  { School_Id: 1425, Org_Name: 'Cambria South School (Proposed opening date: 2028-02-01)', Org_Type: 'Contributing', Total: null, Roll_Date: null, Latitude: O[0] - 420 / KY, Longitude: O[1] + 460 / KM },
  { School_Id: 1426, Org_Name: 'Far Away College', Org_Type: 'Secondary (Year 9-15)', Total: 1180, Roll_Date: '2025-07-01', Latitude: O[0] + 5000 / KY, Longitude: O[1] + 4000 / KM, Status: 'Open' },
  { School_Id: 1427, Org_Name: 'Manukau Technical Institute', Org_Type: 'Secondary (Year 7-15)', Total: 1530, Roll_Date: '2025-07-01', Latitude: O[0] + 385 / KY, Longitude: O[1] - 175 / KM, Status: 'Open' },
  { School_Id: 1428, Org_Name: 'No Position School', Org_Type: 'Full Primary', Total: 90, Roll_Date: '2025-07-01', Latitude: null, Longitude: null, Status: 'Open' },
];
const ECE_RECORDS = [
  { ECE_Id: 31001, Org_Name: 'Little Kauri Kindergarten', Org_Type: 'Free Kindergarten', All_Children: 60, Total: 54, Roll_Date: '2025-03-01', Latitude: O[0] + 150 / KY, Longitude: O[1] - 380 / KM },
  { ECE_Id: 31002, Org_Name: 'Sunbeam Early Learning', Org_Type: 'Education & Care Service', All_Children: null, Total: 38, Roll_Date: '2025-03-01', Latitude: O[0] + 30 / KY, Longitude: O[1] + 80 / KM },
  { ECE_Id: 31003, Org_Name: 'Home Grown Network', Org_Type: 'Homebased Network', All_Children: 120, Total: 100, Roll_Date: '2025-03-01', Latitude: O[0] + 100 / KY, Longitude: O[1] + 10 / KM },
  { ECE_Id: 31004, Org_Name: 'Tiny Toes Childcare', Org_Type: 'Education & Care Service', All_Children: 45, Total: 40, Roll_Date: '2025-03-01', Latitude: O[0] - 380 / KY, Longitude: O[1] - 500 / KM },
  { ECE_Id: 31005, Org_Name: 'Empty Place Playcentre', Org_Type: 'Playcentre', All_Children: 0, Total: null, Roll_Date: '2025-03-01', Latitude: O[0] - 200 / KY, Longitude: O[1] + 200 / KM },
  { ECE_Id: 31006, Org_Name: 'Distant Daycare', Org_Type: 'Education & Care Service', All_Children: 30, Total: 30, Roll_Date: '2025-03-01', Latitude: O[0] + 4000 / KY, Longitude: O[1] },
  { ECE_Id: 31007, Org_Name: 'Little Kauri Annex', Org_Type: 'Education & Care Service', All_Children: 30, Total: 28, Roll_Date: '2025-03-01', Latitude: O[0] + 160 / KY, Longitude: O[1] - 372 / KM },
];

// Auckland Unitary Plan base zones
const AUP_FIELDS = [{ name: 'OBJECTID', type: 'esriFieldTypeOID', alias: 'OBJECTID' },
  { name: 'ZONE', type: 'esriFieldTypeSmallInteger', alias: 'Zone', domain: { type: 'codedValue', name: 'Zone', codedValues: [
    [18, 'Residential - Mixed Housing Suburban'], [19, 'Residential - Single House'], [22, 'Business - Town Centre'], [44, 'Business - Neighbourhood Centre'], [17, 'Business - Light Industry'], [63, 'Special Purpose - School'],
    [25, 'Road'], [30, 'Coastal - General Coastal Marine'], [4, 'Future Urban'], [90, 'Open Space - Informal Recreation'], [21, 'Business - Metropolitan Centre Fringe'], [60, 'Residential - Mixed Housing Urban'], [8, 'Residential - Terrace Housing and Apartment Building']].map(([code, name]) => ({ name, code })) } },
  { name: 'GROUPZONE', type: 'esriFieldTypeSmallInteger', alias: 'Group zone', domain: { type: 'codedValue', name: 'GroupZone', codedValues: [[1, 'Residential'], [2, 'Business'], [3, 'Special Purpose'], [4, 'Public Open Space'], [5, 'Coastal'], [6, 'Future Urban'], [7, 'Strategic Transport Corridor']].map(([code, name]) => ({ name, code })) } },
  { name: 'VERSIONSTATUS', type: 'esriFieldTypeSmallInteger', alias: 'Version status', domain: { type: 'codedValue', name: 'VersionStatus', codedValues: [[1, 'Operative in part'], [2, 'Proposed']].map(([code, name]) => ({ name, code })) } }];
const AUP_ZONES = [
  [18, 1, [-650, -380, 200, 420]], [19, 1, [-1000, 420, -650, 750]], [22, 2, [-200, -20, 200, 70]], [44, 2, [260, 40, 460, 170]], [17, 2, [300, -460, 650, -160]], [63, 3, [-420, 200, -320, 300]], [63, 3, [-640, 300, -540, 400]],
  [4, 6, [-1000, -750, 1000, -480]], [25, 7, [-1000, 28, 1000, 52]], [90, 4, [-130, -300, -60, -120]], [30, 5, [650, 480, 1000, 750]], [21, 2, [330, 170, 700, 420]], [60, 1, [-560, 20, -250, 200]], [8, 1, [200, -150, 300, -20]],
  [18, 1, [460, -150, 560, 20]]];
const aupFeatures = AUP_ZONES.map(([code, grp, box], i) => ({ attributes: { OBJECTID: i + 1, ZONE: code, GROUPZONE: grp, VERSIONSTATUS: code === 21 ? 2 : 1 }, geometry: { rings: [ring(rectXY(...box, i % 4 === 3))] } }));
const AUP_LAYER = { fields: AUP_FIELDS, features: aupFeatures };

const ANSWERS = {
  study: STUDY,
  census: Object.fromEntries(Object.entries(CENSUS_LAYERS).map(([k, v]) => [k, { fields: v.fields, features: layerFeatures(v.attrs, k === 'dw' || k === 'hh') }])),
  employees: { fields: EMP_FIELDS, features: empFeatures },
  schools: SCHOOL_RECORDS, ece: ECE_RECORDS, aup: AUP_LAYER,
};
writeData('census-answers.json', ANSWERS);

/* ------------------------------------------------------------------------------------------------------------------------------------
   The harness: a stubbed network that records every request, and helpers that run the prototype's functions
   ------------------------------------------------------------------------------------------------------------------------------------ */
// The answers every request got, by request (shared by every case, so the same address is stored once): fixtures/data/census-exchanges.json
const EXCHANGE_FILE = join(here, 'fixtures', 'data', 'census-exchanges.json');
export const EXCHANGES = (() => { try { return JSON.parse(readFileSync(EXCHANGE_FILE, 'utf8')); } catch { return {}; } })();
const exchangeKey = (method, url, body) => `${method} ${url}${body ? ' | ' + body : ''}`;
export const saveExchanges = () => { const t = JSON.stringify(EXCHANGES); writeFileSync(EXCHANGE_FILE, t); console.log('fixtures/data/census-exchanges.json', t.length, 'bytes'); };

// Answers like the ArcGIS and CKAN services do: layers by address, features by page (the dwellings layer in pages of 5, so paging is exercised).
// Every request is recorded in log; fail makes requests whose address contains a key fail ('network', 'abort', 'unreadable', a status number or { json })
export function makeNet(answers, { pageSize = 5, fail = {}, post = null } = {}) {
  const log = [];
  const layerOf = url => {
    if (url.includes('dwellings_by_SA1')) return 'dw';
    if (url.includes('households_by_SA1')) return 'hh';
    if (url.includes('individuals_by_SA1/FeatureServer/0')) return 'ind0';
    if (url.includes('individuals_by_SA1/FeatureServer/2')) return 'ind2';
    if (url.includes('employees_by_statistical')) return 'employees';
    if (url.includes('Unitary_Plan_Base_Zone')) return 'aup';
    return null;
  };
  const fetchImpl = async (url, init) => {
    const method = init && init.method ? init.method : 'GET', body = init && init.body ? String(init.body) : undefined;
    const entry = { method, url: String(url) };
    if (body) entry.body = body;
    log.push(entry);
    const key = exchangeKey(method, entry.url, body);
    const respond = (status, answer, text) => {
      entry.status = status;
      if (answer !== undefined) { if (text === undefined && pageSize === 5) { EXCHANGES[key] = answer; entry.ans = key; } else entry.answer = answer; }
      return { ok: status >= 200 && status < 300, status, json: async () => { if (text) throw new SyntaxError(text); return JSON.parse(JSON.stringify(answer)); } };
    };
    const f = Object.entries(fail).find(([pat]) => String(url).includes(pat) || (body && body.includes(pat)));
    if (f) {
      const how = f[1];
      if (how === 'network') { entry.failure = 'network'; throw new TypeError('fetch failed'); }
      if (how === 'abort') { entry.failure = 'timeout'; throw Object.assign(new Error('The operation was aborted'), { name: 'AbortError' }); }
      if (typeof how === 'number') return respond(how);
      if (how === 'unreadable') { entry.failure = 'unreadable'; entry.status = 200; return { ok: true, status: 200, json: async () => { throw new SyntaxError('Unexpected token < in JSON at position 0'); } }; }
      if (how && how.json !== undefined) { entry.status = 200; entry.answer = how.json; return { ok: true, status: 200, json: async () => JSON.parse(JSON.stringify(how.json)) }; }
    }
    if (method === 'POST' && post) { const r = await post(entry, body); EXCHANGES[key] = r; entry.ans = key; entry.status = 200; return { ok: true, status: 200, json: async () => JSON.parse(JSON.stringify(r)) }; }
    const u = new URL(String(url));
    if (u.host === 'catalogue.data.govt.nz') {
      const sql = u.searchParams.get('sql') || '';
      return respond(200, { success: true, result: { records: sql.includes('a9d65b07') ? answers.ece : answers.schools } });
    }
    const layer = layerOf(String(url));
    if (!layer) return respond(404);
    const L = layer === 'employees' || layer === 'aup' ? answers[layer] : answers.census[layer];
    if (!u.pathname.endsWith('/query')) return respond(200, { currentVersion: 11.1, name: layer, fields: L.fields });
    const offset = +u.searchParams.get('resultOffset') || 0, geom = u.searchParams.get('returnGeometry') === 'true', want = (u.searchParams.get('outFields') || '').split(',');
    const size = layer === 'dw' ? pageSize : 1000, page = L.features.slice(offset, offset + size);
    const features = page.map(f => ({ attributes: Object.fromEntries(Object.entries(f.attributes).filter(([k]) => want.includes(k))), ...(geom && f.geometry ? { geometry: f.geometry } : {}) }));
    return respond(200, { objectIdFieldName: 'OBJECTID', features, ...(offset + size < L.features.length ? { exceededTransferLimit: true } : {}) });
  };
  return { fetchImpl, log };
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 1: constants and the pure functions
   ------------------------------------------------------------------------------------------------------------------------------------ */
const nanText = v => (typeof v === 'number' && !Number.isFinite(v) ? String(v) : v);
const bbOf = rings => { const bb = [Infinity, Infinity, -Infinity, -Infinity]; for (const r of rings) for (const p of r) { bb[0] = Math.min(bb[0], p[0]); bb[1] = Math.min(bb[1], p[1]); bb[2] = Math.max(bb[2], p[0]); bb[3] = Math.max(bb[3], p[1]); } return bb; };
const summarise = list => list.map(o => ({ ...Object.fromEntries(Object.entries(o).filter(([k]) => k !== 'rings' && k !== 'bb')), bb: o.bb, nRings: (o.rings || []).length, nPts: (o.rings || []).reduce((s, r) => s + r.length, 0) }));

async function partFunctions() {
  reseed(1100);
  const c = load({ seed: 1 }), L = c.LandusePlus, out = {};
  writeExpected('census-constants.json', jsonSafe({ SRC: L.SRC, STD: L.STD, YIELDS: L.YIELDS, CHOICES: L.CHOICES, CENSUS_VARS: L.CENSUS_VARS, MODES: L.MODES, CSV_HEAD: L.CSV_HEAD }));

  // parseNum: the quantity box
  const texts = ['12,000', '5000 m2', '5 000 m²', '250 sqm', '', '  ', null, undefined, 'abc', '1e3', '-5', '+7.5', '.5', '5.', '1,2,3', '12 000 SQM', '3 M2', '0', '1e', 'e5', '١٢', '1 2 3', '4,5.6', '--5', 12, 0, -3.5, NaN, Infinity, true, false, [], [5], {}, '12m²', 'm2', 'sqm sqm'];
  out.parseNum = texts.map(t => ({ in: t === undefined ? { undefined: true } : typeof t === 'number' && !Number.isFinite(t) ? { nonFinite: String(t) } : t, out: nanText(L.parseNum(t)) }));
  out.cardCheck = [['residential', '12'], ['', '12'], ['nope', '5'], ['retail', '1,500 m2'], ['office', ''], ['school', 'x'], ['apartment', '-4'], ['hotel', 0], ['childcare', 60]].map(([k, raw]) => ({ k, raw, out: jsonSafe(L.cardCheck(k, raw)) }));

  // lr: largest-remainder split, with many random cases
  out.lr = [];
  const cases = [[10, [1, 1, 1]], [7, [3, 2, 2]], [0, [1, 2]], [5, [0, 0]], [5, []], [100, [0.2, 0.3, 0.5]], [1, [1, 1, 1, 1]], [13, [5]], [-4, [1, 2]], [2.5, [1, 1]], [9, [1, 0, 2]], [6, [-1, 3, 4]], [4, [1e-9, 1]]];
  for (let i = 0; i < 300; i++) { const n = 1 + Math.floor(rnd() * 8), w = Array.from({ length: n }, () => (rnd() < 0.15 ? 0 : Math.round(uni(0.1, 900) * 100) / 100)); cases.push([Math.floor(rnd() * 400), w]); }
  for (const [t, w] of cases) out.lr.push({ total: t, weights: w, out: L.lr(t, w) });

  // box
  out.box = [[[-37, 174], [-36.9, 174.2]], [[-36.9, 174.2], [-37, 174]], { south: -37, west: 174, north: -36.9, east: 174.2 }, { s: -37, w: 174, n: -36.9, e: 174.2 }, { s: -37, w: 174, n: -36.9 }, { s: 'a', w: 1, n: 2, e: 3 }, null, undefined, 0, {}, [[1, 2], [3, 4]]]
    .map(b => ({ in: b === undefined ? { undefined: true } : b, out: jsonSafe(L.box(b)) }));

  // sampleShare on the SA1 polygons, the hole and a polygon off to one side
  out.sampleShare = [];
  const boxes = [{ s: STUDY.s, w: STUDY.w, n: STUDY.n, e: STUDY.e }, { s: O[0] - 100 / KY, w: O[1] - 100 / KM, n: O[0] + 100 / KY, e: O[1] + 100 / KM }, { s: 0, w: 0, n: 1, e: 1 }];
  for (const a of sa1s) for (const B of boxes) for (const n of [undefined, 16, 3]) {
    const f = { rings: a.rings, bb: bbOf(a.rings) }, r = L.sampleShare(f, B, n);
    out.sampleShare.push({ code: a.code, n: n ?? null, B, out: jsonSafe({ share: r.share, inside: r.inside, pts: r.pts }) });
  }

  // resolve: aliases, codes, empty field lists, other years
  out.resolve = [];
  for (const k of ['dw', 'hh', 'ind0', 'ind2']) {
    const odd = [{ name: 'SA12023_V1_00', alias: 'x' }, { name: Object.values(L.CENSUS_VARS[k])[0].code, alias: 'something else' }];
    for (const [year, fields] of [[undefined, CENSUS_LAYERS[k].fields], [undefined, []], [undefined, odd], [2018, CENSUS_LAYERS[k].fields]]) out.resolve.push({ spec: k, year: year ?? null, fields, out: L.resolve(fields, L.CENSUS_VARS[k], year) });
  }
  const upper = [{ name: 'F1', alias: 'YEAR: 2023 DWELLING OCCUPANCY STATUS (OCCUPIED DWELLING)' }, { name: 'F2' }, { alias: 'Year: 2023 Dwelling type (Separate house)' }];
  out.resolve.push({ spec: 'dw', year: null, fields: upper, out: L.resolve(upper, L.CENSUS_VARS.dw) });

  // the parsers
  const layers = Object.fromEntries(Object.entries(ANSWERS.census).map(([k, v]) => [k, { meta: { fields: v.fields }, features: v.features }]));
  const censusCases = { all: layers, noDw: { ...layers, dw: undefined }, noGeometry: { ind0: layers.ind0, ind2: layers.ind2, hh: { meta: layers.hh.meta, features: layers.hh.features.map(f => ({ attributes: f.attributes })) } },
    hhFirst: { hh: layers.hh, dw: { meta: layers.dw.meta, features: layers.dw.features.map(f => ({ attributes: f.attributes })) } }, empty: {}, codeOnly: { dw: { meta: { fields: [] }, features: layers.dw.features } } };
  out.parseCensus = {};
  for (const [name, ls] of Object.entries(censusCases)) {
    const r = L.parseCensus(ls);
    out.parseCensus[name] = { sa1: summarise(r.sa1), fields: r.fields, missing: r.missing };
  }
  const emp = { meta: { fields: ANSWERS.employees.fields }, features: ANSWERS.employees.features };
  const empNoName = { meta: { fields: ANSWERS.employees.fields.filter(f => f.name !== 'SA22023_V1_00_NAME' && f.name !== 'ec_avperinc') }, features: ANSWERS.employees.features };
  const empEmpty = { meta: { fields: [] }, features: ANSWERS.employees.features };
  const empOdd = { meta: { fields: ANSWERS.employees.fields }, features: [...ANSWERS.employees.features, { attributes: { SA22023_V1_00: '', ec2025: 5 }, geometry: ANSWERS.employees.features[0].geometry }, { attributes: { SA22023_V1_00: '600999', ec2025: -3, ec_avperinc: 'x' } },
    { attributes: { SA22023_V1_00: '600998', SA22023_V1_00_NAME: 'No count', ec2025: null, ec_avperinc: null }, geometry: ANSWERS.employees.features[1].geometry }] };
  out.parseEmployees = Object.fromEntries(Object.entries({ all: emp, noName: empNoName, empty: empEmpty, odd: empOdd, none: { features: [] } }).map(([k, v]) => { const r = L.parseEmployees(v); return [k, { year: r.year, field: r.field, sa2: summarise(r.sa2) }]; }));
  const oddMoe = [{ Org_Name: '  Spaced  Name  ', Latitude: '-36.9', Longitude: '174.8', Total: '45' }, { Org_Name: 'Prop (Proposed opening date:  2031-07-09)', School_Id: 'X1', Latitude: 1, Longitude: 2, Status: 'Open' }, { Org_Name: 'closed one', Latitude: 1, Longitude: 2, Status: 'CLOSED' },
    { Org_Name: 'Bad (Proposed opening date: soon)', Latitude: 3, Longitude: 4 }, { School_Id: 0, ECE_Id: 77, Latitude: 5, Longitude: 6, All_Children: '12' }, { Latitude: '', Longitude: 3 },
    { Org_Name: 'Two (proposed opening date: 2030-01-01) and (Proposed opening date: 2031-01-01)', Latitude: 7, Longitude: 8, Roll_Date: '2024-12-31T10:00:00Z' }, { Org_Name: 'N', School_Id: 5, Latitude: 1, Longitude: 2, Total: 0, Org_Type: '' }];
  out.parseMoE = { schools: L.parseMoE(SCHOOL_RECORDS, 'schools'), ece: L.parseMoE(ECE_RECORDS, 'ece'), oddEce: L.parseMoE(oddMoe, 'ece'), oddSchools: L.parseMoE(oddMoe, 'schools'), none: L.parseMoE(undefined, 'schools') };
  const aupLayer = { meta: { fields: AUP_FIELDS }, features: AUP_LAYER.features };
  const aupNoDomain = { meta: { fields: AUP_FIELDS.map(f => ({ name: f.name, alias: f.alias })) }, features: [...AUP_LAYER.features, { attributes: { ZONE: 'x' }, geometry: AUP_LAYER.features[0].geometry }, { attributes: { ZONE: 18 } }] };
  out.parseAUP = Object.fromEntries(Object.entries({ all: aupLayer, noDomain: aupNoDomain, none: { features: [] } }).map(([k, v]) => { const r = L.parseAUP(v); return [k, { zones: summarise(r.zones), names: r.names }]; }));
  // areaBox: the box drawn in onboarding, the extent of the network and zones, the box of the last fill
  out.areaBox = [];
  const areaCases = [
    { name: 'drawn as corners', onboarding: { area: [[-37.1, 174.9], [-37.0, 175.0]] }, nodes: [[-36.9, 174.8]] },
    { name: 'drawn as an object', onboarding: { area: { south: -37.1, west: 174.9, north: -37.0, east: 175.0 } } },
    { name: 'drawn as s w n e', onboarding: { area: { s: -37.1, w: 174.9, n: -37.0, e: 175.0 } }, zones: [[1, 1]] },
    { name: 'drawn but unusable', onboarding: { area: { s: 'a' } }, nodes: [[-37.0, 174.9], [-37.02, 174.95]], zones: [[-37.01, 174.99]] },
    { name: 'network and zones', nodes: [[-37.0, 174.9], [-37.02, 174.95]], zones: [[-37.01, 174.99], [-36.99, 174.91]] },
    { name: 'zones only', zones: [[-37.01, 174.99], [-36.99, 174.91]] },
    { name: 'nodes only', nodes: [[-37.0, 174.9]] },
    { name: 'a zone without a position', nodes: [[-37.0, 174.9]], zones: [[null, null], ['x', 'y']] },
    { name: 'saved as an object', landuse: { bounds: { s: -37.1, w: 174.9, n: -37.0, e: 175.0, basis: 'the study area' } } },
    { name: 'saved as corners', landuse: { bounds: [[-37.1, 174.9], [-37.0, 175.0]] } },
    { name: 'nothing', },
    { name: 'saved but unusable', landuse: { bounds: { s: 1 } } },
  ];
  for (const ac of areaCases) {
    const P = c.newProject('area'), sc = P.scenarios[0];
    (ac.nodes || []).forEach(([lat, lng], i) => { sc.nodes['n' + i] = { id: 'n' + i, lat, lng }; });
    (ac.zones || []).forEach(([lat, lng], i) => { sc.zones['z' + i] = { id: 'z' + i, kind: i % 2 ? 'gateway' : 'zone', lat, lng }; });
    if (ac.onboarding) P.onboarding = ac.onboarding;
    if (ac.landuse) P.landuse = ac.landuse;
    c.Store.P = P;
    out.areaBox.push({ ...ac, out: jsonSafe(L.areaBox(sc)) ?? null });
  }
  writeExpected('census-functions.json', jsonSafe(out));
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 2: the downloads (fetchCensus, fetchEmployees, fetchMoE, fetchAUP) with the network stubbed
   ------------------------------------------------------------------------------------------------------------------------------------ */
const censusSummary = r => ({ sa1: summarise(r.sa1), fields: r.fields, missing: r.missing, errors: r.errors, partial: r.partial });
const empSummary = r => ({ year: r.year, field: r.field, sa2: summarise(r.sa2) });
const aupSummary = r => ({ zones: summarise(r.zones), names: r.names });

// Runs one download in a fresh copy of the prototype; twice when asked (the second answer comes from the cache unless the first was partial or failed)
async function runDownload(name, what, fn, summary, netOpts = {}, twice = false) {
  const net = makeNet(ANSWERS, netOpts), c = load({ seed: 1, fetchImpl: net.fetchImpl }), L = c.LandusePlus, status = [];
  const one = async () => { try { return { result: summary(await fn(L, t => status.push(t))) }; } catch (e) { return { error: e.message }; } };
  const first = await one(), n1 = net.log.length, s1 = status.length;
  const second = twice ? await one() : null;
  return { name, what, bounds: STUDY, fail: netOpts.fail || {}, pageSize: netOpts.pageSize ?? 5, log: net.log, status, first, firstRequests: n1, firstStatus: s1, second };
}

async function partFetch() {
  const cases = [];
  const census = (name, netOpts, twice) => runDownload(name, 'census', (L, st) => L.fetchCensus(STUDY, st), censusSummary, netOpts, twice);
  cases.push(await census('census', {}, true));
  cases.push(await census('census pages of 2', { pageSize: 2 }, false));
  cases.push(await census('census travel layer 500', { fail: { 'individuals_by_SA1/FeatureServer/2/query': 500 } }, true));
  cases.push(await census('census dwellings layer unreachable', { fail: { 'dwellings_by_SA1/FeatureServer/0?f=json': 'network' } }, false));
  cases.push(await census('census every layer fails', { fail: { FeatureServer: 'abort' } }, false));
  cases.push(await census('census dwellings and households 404', { fail: { dwellings_by_SA1: 404, households_by_SA1: 403 } }, false));
  cases.push(await census('census households time out', { fail: { 'households_by_SA1/FeatureServer/0/query': 'abort' } }, false));
  cases.push(await census('census population unreadable', { fail: { 'individuals_by_SA1/FeatureServer/0/query': 'unreadable' } }, false));
  cases.push(await census('census service error 400', { fail: { 'dwellings_by_SA1/FeatureServer/0/query': { json: { error: { code: 400, message: 'Invalid query parameters' } } } } }, false));
  cases.push(await census('census service error 503', { fail: { 'households_by_SA1/FeatureServer/0/query': { json: { error: { code: 503, message: 'Service unavailable' } } } } }, false));
  cases.push(await census('census service error no message', { fail: { 'individuals_by_SA1/FeatureServer/2?f=json': { json: { error: { code: 500, details: ['x'] } } } } }, false));
  cases.push(await census('census 5xx on dwellings meta', { fail: { 'dwellings_by_SA1/FeatureServer/0?f=json': 502 } }, false));
  const emp = (name, netOpts, twice) => runDownload(name, 'employees', (L, st) => L.fetchEmployees(STUDY, st), empSummary, netOpts, twice);
  cases.push(await emp('employees', {}, true));
  cases.push(await emp('employees 403', { fail: { employees_by_statistical: 403 } }, true));
  cases.push(await emp('employees unreadable', { fail: { employees_by_statistical: 'unreadable' } }, false));
  cases.push(await emp('employees service error', { fail: { employees_by_statistical: { json: { error: { code: 400, message: 'Cannot perform query. Invalid query parameters.' } } } } }, false));
  const moe = (name, kind, netOpts, twice) => runDownload(name, 'moe-' + kind, (L, st) => L.fetchMoE(kind, STUDY, st), r => jsonSafe(r), netOpts, twice);
  cases.push(await moe('schools', 'schools', {}, true));
  cases.push(await moe('early learning', 'ece', {}, true));
  cases.push(await moe('schools 429', 'schools', { fail: { 'catalogue.data.govt.nz': 429 } }, false));
  cases.push(await moe('schools unreachable', 'schools', { fail: { 'catalogue.data.govt.nz': 'network' } }, false));
  cases.push(await moe('schools search failed', 'schools', { fail: { 'catalogue.data.govt.nz': { json: { success: false, error: { __type: 'Validation Error' } } } } }, false));
  cases.push(await moe('early learning no result', 'ece', { fail: { 'catalogue.data.govt.nz': { json: { success: true } } } }, false));
  const aup = (name, netOpts, twice) => runDownload(name, 'aup', (L, st) => L.fetchAUP(STUDY, st), aupSummary, netOpts, twice);
  cases.push(await aup('aup', {}, true));
  cases.push(await aup('aup 500', { fail: { Unitary_Plan_Base_Zone: 500 } }, false));
  cases.push(await aup('aup timeout', { fail: { 'Unitary_Plan_Base_Zone/FeatureServer/0/query': 'abort' } }, false));
  writeExpected('census-fetch.json', jsonSafe(cases));
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 3: the project the fills run on, and apply
   ------------------------------------------------------------------------------------------------------------------------------------ */
const ROADS = [
  { name: 'Great South Road', cls: 'arterial', pts: [[-900, 40], [-450, 40], [0, 40], [450, 40], [900, 40]] },
  { name: 'Station Road', cls: 'collector', pts: [[300, -600], [300, -200], [300, 100], [300, 600]] },
  { name: 'Cambria Road', cls: 'collector', pts: [[-400, -600], [-400, 100], [-400, 600]] },
  { name: 'Manukau Road', cls: 'local', pts: [[-900, -300], [-300, -300], [200, -300]] },
];
const latlng = ([x, y]) => [O[0] + y / KY, O[1] + x / KM];

// A project of roads and existing zones built from the OpenStreetMap buildings (ExistingLU.build), plus a development zone, a committed zone and two gateways
export function buildProject(c, { zones = true, extra = true, onlyEast = false } = {}) {
  const { Store, Net, newProject, ExistingLU } = c;
  const P = newProject('Census test project');
  const sc = P.scenarios[0];
  sc.name = 'Base';
  Store.P = P;
  const links = [];
  for (const r of ROADS) {
    const ids = r.pts.map(p => Net.addNode(sc, latlng(p)));
    for (let i = 1; i < ids.length; i++) { const L = Net.addLink(sc, ids[i - 1], ids[i], [], { cls: r.cls, name: r.name }); links.push(L); }
    // shape points on each link make the road lines the zone namer and the nearest-road search use
    r.pts.slice(1).forEach((p, i) => { const L = sc.links[links[links.length - r.pts.length + 1 + i]]; if (L) L.pts = [latlng([(r.pts[i][0] + p[0]) / 2, (r.pts[i][1] + p[1]) / 2])]; });
  }
  if (zones) {
    const keep = onlyEast ? suburb.elements.filter(e => { const g = e.geometry || (e.members && e.members[0] && e.members[0].geometry); if (!g) return true; return (g.reduce((a, p) => a + p.lon, 0) / g.length - O[1]) * KM > 300; }) : suburb.elements;
    const r = ExistingLU.build(keep, STUDY, { links: sc.links });
    for (const z of r.zones) sc.zones[z.id] = { connectors: null, lu: {}, ...z, kind: 'zone', existing: true, src: 'existing-lu' };
  }
  if (extra) {
    const mk = (id, o) => { sc.zones[id] = { id, kind: 'zone', connectors: null, ...o }; };
    mk('zDev', { name: 'Station Precinct (proposed)', lat: O[0] - 200 / KY, lng: O[1] + 50 / KM, lu: { residential: 300, apartment: 120, retail: 800 }, role: 'development', year: 2030 });
    mk('zCom', { name: 'Cambria Terraces (consented)', lat: O[0] + 300 / KY, lng: O[1] + 500 / KM, lu: { residential: 150 }, role: 'committed', year: 2028, buildout: 60 });
    mk('zFar', { name: 'Far Farm', lat: O[0] + 900 / KY, lng: O[1] - 900 / KM, lu: { residential: 3 }, existing: true });
    sc.zones.gW = { id: 'gW', kind: 'gateway', name: 'Great South Road west', lat: latlng([-950, 40])[0], lng: latlng([-950, 40])[1], node: Object.keys(sc.nodes)[0], gw: { AMin: 800, AMout: 500, PMin: 600, PMout: 900 } };
    sc.zones.gE = { id: 'gE', kind: 'gateway', name: 'Great South Road east', lat: latlng([950, 40])[0], lng: latlng([950, 40])[1], node: Object.keys(sc.nodes)[4], gw: { AMin: 500, AMout: 850, PMin: 900, PMout: 650 }, gwSrc: 'NZTA 01N00478 Great South Road (18,200/day)' };
  }
  return P;
}

const MOE_PARSED = L => ({ schools: L.parseMoE(SCHOOL_RECORDS, 'schools'), ece: L.parseMoE(ECE_RECORDS, 'ece') });
const layersOf = () => Object.fromEntries(Object.entries(ANSWERS.census).map(([k, v]) => [k, { meta: { fields: v.fields }, features: v.features }]));
export function dataFor(L, which = {}) {
  const w = { census: true, employees: true, schools: true, aup: true, elements: true, ...which };
  const d = {};
  if (w.elements) d.elements = suburb.elements;
  if (w.census) d.census = L.parseCensus(w.census === 'noDw' ? (() => { const l = layersOf(); delete l.dw; return l; })() : layersOf());
  if (w.employees) d.employees = L.parseEmployees({ meta: { fields: ANSWERS.employees.fields }, features: ANSWERS.employees.features });
  if (w.schools) { const m = MOE_PARSED(L); d.schools = m.schools; d.ece = m.ece; }
  if (w.aup) d.aup = L.parseAUP({ meta: { fields: AUP_FIELDS }, features: AUP_LAYER.features });
  return d;
}

async function partApply() {
  const cases = [];
  const seedOf = i => 100 + i;
  const PROJECTS = { 'census-project': buildProject(load({ seed: 5 })), 'census-project-bare': buildProject(load({ seed: 5 }), { zones: false }), 'census-project-east': buildProject(load({ seed: 5 }), { onlyEast: true }) };
  for (const [k, p] of Object.entries(PROJECTS)) writeData(k + '.json', jsonSafe(p));
  const run = (name, steps, projName = 'census-project') => {
    const proj = jsonSafe(PROJECTS[projName]);
    const sd = seedOf(cases.length), c = load({ seed: sd }), L = c.LandusePlus;
    c.Store.P = JSON.parse(JSON.stringify(proj));
    const P = c.Store.P, sc = P.scenarios[0], out = [];
    for (const st of steps) {
      let report, error = null;
      try { report = L.apply(sc, dataFor(L, st.data), { ...st.opts, ...(st.bounds ? { bounds: st.bounds } : {}) }); }
      catch (e) { error = e.message; }
      out.push({ ...st, report: error ? null : jsonSafe(report), error, zones: jsonSafe(sc.zones) });
    }
    cases.push({ name, project: projName, seed: sd, now: NOW, ...idStart(sd), steps: out });
  };
  const box = (x0, y0, x1, y1) => ({ s: O[0] + y0 / KY, w: O[1] + x0 / KM, n: O[0] + y1 / KY, e: O[1] + x1 / KM });
  run('everything', [{ name: 'first', opts: { bounds: STUDY } }, { name: 'again', opts: { bounds: STUDY } }]);
  run('keep OpenStreetMap where newer', [{ name: 'first', opts: { bounds: STUDY, newer: 'osm' } }, { name: 'again', opts: { bounds: STUDY, newer: 'osm' } }]);
  run('no new zones', [{ name: 'first', opts: { bounds: STUDY, create: false } }]);
  run('no dwellings layer', [{ name: 'first', opts: { bounds: STUDY }, data: { census: 'noDw' } }]);
  run('rolls entered', [{ name: 'first', opts: { bounds: STUDY, rolls: { 1423: 215, 1425: '300', 9999: 5 } } }, { name: 'rolls later', opts: { bounds: STUDY, rolls: { 1423: 240 }, replace: false }, data: { census: false, employees: false, aup: false } }]);
  run('keep the numbers already in zones', [{ name: 'first', opts: { bounds: STUDY, replace: false } }]);
  run('no building outlines', [{ name: 'first', opts: { bounds: STUDY }, data: { elements: false } }]);
  run('a smaller area', [{ name: 'first', opts: { bounds: box(-300, -250, 350, 300) } }]);
  run('an area that cuts SA1s', [{ name: 'first', opts: { bounds: box(-250, -400, 900, 150) } }]);
  run('census only', [{ name: 'first', opts: { bounds: STUDY }, data: { employees: false, schools: false, aup: false } }]);
  run('employees only', [{ name: 'first', opts: { bounds: STUDY }, data: { census: false, schools: false, aup: false } }]);
  run('schools only', [{ name: 'first', opts: { bounds: STUDY }, data: { census: false, employees: false, aup: false } }]);
  run('zones only', [{ name: 'first', opts: { bounds: STUDY }, data: { census: false, employees: false, schools: false } }]);
  run('no study area', [{ name: 'first', opts: {}, data: {} }]);
  run('no existing zones', [{ name: 'first', opts: { bounds: STUDY } }, { name: 'again', opts: { bounds: STUDY } }], 'census-project-bare');
  run('zones only in the east', [{ name: 'first', opts: { bounds: STUDY } }, { name: 'again', opts: { bounds: STUDY, newer: 'osm' } }], 'census-project-east');
  run('zones only in the east, none made', [{ name: 'first', opts: { bounds: STUDY, create: false } }], 'census-project-east');
  run('no existing zones, none made', [{ name: 'first', opts: { bounds: STUDY, create: false } }], 'census-project-bare');
  writeExpected('census-apply.json', jsonSafe(cases));
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 4: fillAll end to end (downloads, the apply on every scenario, the saved summary) and the school rolls
   ------------------------------------------------------------------------------------------------------------------------------------ */
// A small Overpass answer for ExistingLU.fetch: the buildings query gets some buildings, the land-use query the areas and shops
const smallOsm = (() => {
  const b = suburb.elements.filter(e => e.type === 'way' && e.tags && e.tags.building && e.geometry).slice(0, 70);
  const a = suburb.elements.filter(e => !(e.tags && e.tags.building)).slice(0, 30);
  return { buildings: b, areas: a };
})();
const overpassPost = async (entry, body) => {
  const q = decodeURIComponent(String(body).replace(/^data=/, ''));
  return { version: 0.6, generator: 'Overpass API (made up)', elements: q.includes('["building"]') ? smallOsm.buildings : smallOsm.areas };
};
const twoScenarios = () => {
  const c = load({ seed: 5 }), P = buildProject(c), sc = P.scenarios[0];
  const opt = JSON.parse(JSON.stringify(sc));
  opt.id = 'sOpt'; opt.name = 'Option'; opt.year = 2036;
  for (const [id, z] of Object.entries(opt.zones)) if (z.kind === 'zone' && z.existing && id !== 'zFar') delete opt.zones[id];
  opt.zones.zDev.lu.residential = 500;
  P.scenarios.push(opt);
  for (const s of P.scenarios) s.res = { AM: { period: 'AM', kpi: { trips: 100 } }, PM: { period: 'PM', kpi: { trips: 80 }, stale: false } };
  return P;
};

async function partFill() {
  const cases = [];
  const PROJECTS = { 'census-project': jsonSafe(buildProject(load({ seed: 5 }))), 'census-project-bare': jsonSafe(buildProject(load({ seed: 5 }), { zones: false })), 'census-project-two': jsonSafe(twoScenarios()), 'census-project-empty': jsonSafe(load({ seed: 5 }).newProject('Empty project')) };
  writeData('census-project-two.json', PROJECTS['census-project-two']);
  writeData('census-project-empty.json', PROJECTS['census-project-empty']);
  const box = (x0, y0, x1, y1) => ({ s: O[0] + y0 / KY, w: O[1] + x0 / KM, n: O[0] + y1 / KY, e: O[1] + x1 / KM });
  const run = async (name, { project = 'census-project', active = 0, opts = {}, netOpts = {}, calls = 1, reopen = null, onboarding = null, elements = false } = {}) => {
    const seedNo = 300 + cases.length, net = makeNet(ANSWERS, { post: overpassPost, ...netOpts }), c = load({ seed: seedNo, fetchImpl: net.fetchImpl }), L = c.LandusePlus;
    c.Store.P = JSON.parse(JSON.stringify(PROJECTS[project]));
    const P = c.Store.P;
    P.active = P.scenarios[active].id;
    if (onboarding) P.onboarding = onboarding;
    const sc = c.Store.sc, status = [], out = [];
    for (let i = 0; i < calls; i++) {
      let report = null, error = null;
      const mark = net.log.length, smark = status.length;
      try { report = await L.fillAll(sc, { ...opts, ...(elements ? { elements: suburb.elements } : {}) }, t => status.push(t)); } catch (e) { error = e.message; }
      out.push({ report: jsonSafe(report), error, requests: [mark, net.log.length], status: [smark, status.length] });
    }
    const after = { landuse: jsonSafe(P.landuse), scenarios: jsonSafe(P.scenarios.map(s => ({ id: s.id, zones: s.zones, res: s.res }))) };
    const rolls = [];
    if (reopen) {
      // reopen: a new copy of the prototype given the saved project; the schools without a roll are looked up from the saved summary
      const c2 = load({ seed: seedNo + 50, fetchImpl: net.fetchImpl }), L2 = c2.LandusePlus;
      c2.Store.P = JSON.parse(JSON.stringify(P));
      if (reopen.dropLocations) for (const s of c2.Store.P.landuse.last.schools.needsRoll) { s.lat = null; s.lng = null; }
      const sc2 = c2.Store.sc;
      const targets = reopen.all ? c2.Store.P.scenarios : [sc2];
      for (const step of reopen.steps) {
        if (step.op === 'ready') rolls.push({ op: 'ready', rolls: step.rolls ?? null, out: jsonSafe(L2.rollsReady(step.rolls)) });
        else if (step.op === 'apply') { const r = L2.applyRolls(targets, step.rolls); rolls.push({ op: 'apply', rolls: step.rolls, out: jsonSafe(r), landuse: jsonSafe(c2.Store.P.landuse), scenarios: jsonSafe(c2.Store.P.scenarios.map(s => ({ id: s.id, zones: s.zones, res: s.res }))) }); }
      }
    }
    cases.push({ name, project, active, seed: seedNo, now: NOW, ...idStart(seedNo), opts, elements, calls, onboarding, fail: netOpts.fail || {}, log: net.log, status, out, after,
      reopen: reopen ? { ...reopen, seed: seedNo + 50, now: NOW, ...idStart(seedNo + 50), rolls } : null });
  };
  await run('given buildings', { opts: { bounds: STUDY }, elements: true, calls: 2 });
  await run('given buildings, keep OpenStreetMap', { opts: { bounds: STUDY, newer: 'osm' }, elements: true });
  await run('only the Census', { opts: { bounds: STUDY, which: { employees: false, schools: false, aup: false, osm: false } } });
  await run('only schools and zoning', { opts: { bounds: STUDY, which: { census: false, employees: false, osm: false } }, calls: 2 });
  await run('area from the project', { opts: { which: { osm: false } } });
  const drawn = box(-300, -250, 350, 300);
  await run('area drawn in onboarding', { opts: { which: { osm: false } }, onboarding: { area: [[drawn.s, drawn.w], [drawn.n, drawn.e]] } });
  await run('every scenario', { project: 'census-project-two', active: 1, opts: { bounds: STUDY, all: true }, elements: true });
  await run('one scenario of two', { project: 'census-project-two', active: 1, opts: { bounds: STUDY }, elements: true });
  await run('zones built from OpenStreetMap', { project: 'census-project-bare', opts: { bounds: STUDY }, elements: true });
  await run('buildings downloaded', { project: 'census-project-bare', opts: { bounds: STUDY } });
  await run('buildings downloaded, first server fails', { project: 'census-project-bare', opts: { bounds: STUDY }, netOpts: { fail: { 'overpass-api.de': 504 } } });
  await run('buildings downloaded, remark error', { project: 'census-project-bare', opts: { bounds: STUDY }, netOpts: { fail: { 'overpass-api.de': 'network', 'maps.mail.ru': { json: { elements: [], remark: 'runtime error: Query timed out in "query" at line 3 after 91 seconds.' } } } } });
  await run('buildings downloaded, every server fails', { project: 'census-project-bare', opts: { bounds: STUDY }, netOpts: { fail: { 'overpass-api.de': 504, 'maps.mail.ru': 'network', 'overpass.private.coffee': 'unreadable', 'overpass.kumi.systems': { json: { elements: [], remark: 'runtime error: out of memory' } } } } });
  await run('buildings downloaded, null answer', { project: 'census-project-bare', opts: { bounds: STUDY }, netOpts: { fail: { 'overpass-api.de': { json: null } } } });
  await run('an area too large', { opts: { bounds: box(-14000, -14000, 14000, 14000), which: { census: false, employees: false, schools: false, aup: false } } });
  await run('an area with no size', { opts: { bounds: { s: 1, w: 1, n: 0, e: 2 }, which: { census: false, employees: false, schools: false, aup: false } } });
  await run('every source fails', { opts: { bounds: STUDY }, elements: true, netOpts: { fail: { FeatureServer: 500, 'catalogue.data.govt.nz': 'network' } } });
  await run('travel layer fails, then works', { opts: { bounds: STUDY, which: { osm: false } }, calls: 2, netOpts: { fail: { 'individuals_by_SA1/FeatureServer/2/query': 500 } } });
  await run('no places, no roads, no zones', { project: 'census-project-empty', opts: {} });
  // the schools without a roll: asked for later, in a reopened project
  await run('rolls after reopening', { opts: { bounds: STUDY }, elements: true, reopen: { steps: [{ op: 'ready' }, { op: 'ready', rolls: { 1423: 100 } }, { op: 'ready', rolls: { 5555: 1 } }, { op: 'apply', rolls: { 1423: 215 } }, { op: 'apply', rolls: { 1423: 215, 1425: 300 } }, { op: 'ready' }] } });
  await run('rolls on every scenario', { project: 'census-project-two', active: 1, opts: { bounds: STUDY, all: true }, elements: true, reopen: { all: true, steps: [{ op: 'apply', rolls: { 1423: 180, 1425: '320' } }] } });
  await run('rolls for a save without locations', { opts: { bounds: STUDY }, elements: true, reopen: { dropLocations: true, steps: [{ op: 'ready' }, { op: 'apply', rolls: { 1423: 215 } }] } });
  await run('rolls when nothing is waiting', { opts: { bounds: STUDY, which: { schools: false } }, elements: true, reopen: { steps: [{ op: 'ready' }, { op: 'apply', rolls: { 1423: 215 } }] } });
  writeExpected('census-fill.json', jsonSafe(cases));
}

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 5: trip generation, tables, the CSV, how a table reaches the model, zoned yield
   ------------------------------------------------------------------------------------------------------------------------------------ */
const reseed = n => { seed = n >>> 0; };
const pickOne = a => a[Math.floor(rnd() * a.length)];
const maybe = (p, v) => (rnd() < p ? v : undefined);
const cleanObj = o => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined));

// Mode counts of a Census travel topic as a zone's census record holds them
const modeCounts = (g, small) => {
  const k = g === 'e' ? ['home', 'drive', 'pass', 'bike', 'walk', 'schoolBus', 'bus', 'train', 'ferry', 'other'] : ['home', 'notToday', 'drivePriv', 'driveCo', 'pass', 'bus', 'train', 'bike', 'walk', 'ferry', 'other'];
  const o = {};
  for (const key of k) o[key] = rnd() < 0.08 ? null : Math.round(uni(0, small ? 4 : 160) / 3) * 3;
  o.stated = rnd() < 0.2 ? null : Object.values(o).reduce((s, v) => s + (v || 0), 0) - (rnd() < 0.15 ? 5 : 0);
  if (rnd() < 0.05) for (const key of Object.keys(o)) o[key] = 0;
  return o;
};
const censusRec = () => {
  const small = rnd() < 0.2, sa1 = Array.from({ length: 1 + Math.floor(rnd() * 6) }, (_, i) => ({ code: `700${1000 + Math.floor(rnd() * 90) * 7}`, share: Math.round(rnd() * 1000) / 1000 }));
  return { sa1, modes: { r: rnd() < 0.1 ? null : modeCounts('r', small), w: rnd() < 0.3 ? null : modeCounts('w', small), e: rnd() < 0.3 ? null : modeCounts('e', small) } };
};
const tgRows = lu => {
  const rows = {};
  for (const k of Object.keys(lu)) {
    if (rnd() < 0.3) continue;
    const r = {};
    const mode = pickOne(['veh', 'person', 'person', 'none']);
    if (mode === 'veh') r.veh = { AM: maybe(0.9, Math.round(uni(0.1, 9) * 100) / 100), PM: maybe(0.9, Math.round(uni(0.1, 12) * 100) / 100) };
    if (mode === 'person') { r.basis = 'person'; r.person = { AM: maybe(0.9, Math.round(uni(0.2, 4) * 100) / 100), PM: maybe(0.8, Math.round(uni(0.2, 5) * 100) / 100) }; }
    if (mode === 'none' && rnd() < 0.3) r.basis = 'person';
    if (rnd() < 0.5) r.inPct = { AM: maybe(0.8, Math.round(uni(5, 95))), PM: maybe(0.8, String(Math.round(uni(5, 95)))) };
    if (rnd() < 0.4) r.freight = { AM: maybe(0.8, Math.round(uni(0, 0.4) * 100) / 100), PM: maybe(0.7, Math.round(uni(0, 0.4) * 100) / 100) };
    if (rnd() < 0.3) r.hvPct = pickOne([4, 12, '9', '', 'x', 0]);
    if (rnd() < 0.6) r.source = pickOne(['Surveyed rate (made up)', '', 'RTA 2013', 'ITA App 1']);
    if (rnd() < 0.5) r.year = pickOne([2016, 2019, 2021, 2022, 2023, 2025, '2018', '', null, 'n/a']);
    if (rnd() < 0.4) r.hours = pickOne(['7am-9pm', ' 24 h ', '', 12]);
    if (rnd() < 0.2) r.driveThrough = pickOne([true, false, 'yes']);
    rows[k] = cleanObj({ ...r, veh: r.veh && cleanObj(r.veh), person: r.person && cleanObj(r.person), inPct: r.inPct && cleanObj(r.inPct), freight: r.freight && cleanObj(r.freight) });
  }
  return rows;
};
const GFA = ['retail', 'supermarket', 'fastfood', 'office', 'medical', 'industrial', 'warehouse'];
function fuzzZone(i) {
  const keys = Object.keys(LANDUSES_BY_KEY), lu = {};
  for (const k of [...keys].sort(() => rnd() - 0.5).slice(0, 1 + Math.floor(rnd() * 4))) lu[k] = GFA.includes(k) ? Math.round(uni(200, 15000) / 10) * 10 : Math.round(uni(2, 500));
  if (rnd() < 0.08) lu[pickOne(keys)] = pickOne(['300', '1,200', 0, -5, 'x', null]);
  const z = { id: `zf${i}`, kind: 'zone', name: `Fuzz zone ${i}`, lat: O[0], lng: O[1], lu, connectors: null };
  const cls = pickOne(['existing', 'existing', 'committed', 'development', 'development', 'none']);
  if (cls === 'existing') z.existing = true; else if (cls !== 'none') z.role = cls;
  if (rnd() < 0.25) z.year = pickOne([2020, 2030, 2036, '2032', 0, null]);
  if (rnd() < 0.2) z.buildout = pickOne([50, 80, 100, 0, '40', '', 150, -10]);
  if (rnd() < 0.4) z.reduction = pickOne([0, 10, 35, 100, '20', 150, -5, null]);
  if (rnd() < 0.25) z.extra = cleanObj({ AMin: maybe(0.8, Math.round(uni(0, 40))), AMout: maybe(0.8, Math.round(uni(0, 40))), PMin: maybe(0.8, Math.round(uni(0, 40))), PMout: maybe(0.8, Math.round(uni(0, 40))) });
  const where = rnd();
  if (where < 0.4) z.census = { ...censusRec(), at: '2026-10-21' }; else if (where < 0.6) z.censusLoc = censusRec();
  if (rnd() < 0.7) {
    const tg = { on: rnd() < 0.85 };
    if (rnd() < 0.8) tg.rows = tgRows(lu);
    if (rnd() < 0.35) tg.split = cleanObj({ driver: maybe(0.8, pickOne([60, 70, '55', 0, ''])), passenger: maybe(0.7, 8), pt: maybe(0.7, pickOne([12, 20])), walk: maybe(0.6, 5), cycle: maybe(0.5, 2) });
    if (rnd() < 0.3) tg.occupancy = pickOne([1.2, 1.5, '1.8', 0, -1, 'x']);
    if (rnd() < 0.5) tg.internal = pickOne([0, 5, 10, 25, '8', 120, -3, '']);
    if (rnd() < 0.5) tg.passby = pickOne([0, 10, 20, '15', 150, null]);
    if (rnd() < 0.4) tg.existing = cleanObj({ AMin: maybe(0.8, pickOne([0, 20, 80, 400])), AMout: maybe(0.8, pickOne([0, 25, 90, 500])), PMin: maybe(0.8, pickOne([10, 30, 700])), PMout: maybe(0.8, pickOne([5, 45])) });
    if (rnd() < 0.15) tg.other = { AMin: 3, AMout: 4, PMin: 5, PMout: 6 };
    if (rnd() < 0.15) tg.synced = pickOne(['tgLoad', 'extra']);
    z.tg = tg;
    if (tg.on && rnd() < 0.5) z.tgLoad = { AMin: 12, AMout: 9, PMin: 8, PMout: 14 };
  }
  if (rnd() < 0.04) { z.kind = 'gateway'; z.gw = { AMin: 500, AMout: 400, PMin: 450, PMout: 520 }; }
  return z;
}
const LANDUSES_BY_KEY = Object.fromEntries(['residential', 'apartment', 'retirement', 'retail', 'supermarket', 'fastfood', 'office', 'hotel', 'school', 'childcare', 'medical', 'tertiary', 'industrial', 'warehouse'].map(k => [k, 1]));

const settingsVariants = c => {
  const base = JSON.parse(JSON.stringify(c.defaultSettings()));
  const v1 = JSON.parse(JSON.stringify(base)); v1.hvPct = 8; v1.landuse = { split: { driver: 60, passenger: 10, pt: 15, walk: 10, cycle: 5 }, eduOcc: 1.5, rateMeta: { residential: { source: 'Surveyed rate (made up)', year: 2024 }, retail: { source: 'Old survey', year: 2016 }, office: { year: 2021 }, school: { source: 'Ministry 2022', year: '2022' } } };
  const v2 = JSON.parse(JSON.stringify(base)); v2.landuse = { split: { driver: 40, passenger: 10, pt: 10, walk: 10, cycle: 10 }, rateMeta: { residential: { year: '' }, apartment: { year: 2019, source: '' }, hotel: { year: 2023 } } };
  delete v2.rates.hotel.AM; delete v2.rates.fastfood; v2.rates.office.PM.rate = '1.9'; v2.rates.industrial.AM.inPct = '70'; v2.hvPct = '5';
  const v3 = JSON.parse(JSON.stringify(base)); v3.landuse = { split: { driver: 0, passenger: 0, pt: 0, walk: 0, cycle: 0 }, eduOcc: 'x' }; v3.rates.retail.PM.rate = 5.5; v3.year = 2030;
  return [base, v1, v2, v3];
};
const scenarioVariants = [{ id: 's0', name: 'Base' }, { id: 's1', name: 'Option 2036', year: 2036, buildout: 60 }, { id: 's2', name: 'Opening 2028', year: 2028, buildout: '' }, { id: 's3', name: 'Zero build-out', year: '2036', buildout: 0 }];
const scenarioOf = (c, v) => ({ ...c.newScenario(v.name), ...v });

async function partTrips() {
  reseed(5100);
  const c = load({ seed: 7 }), L = c.LandusePlus, out = {};
  const S = settingsVariants(c);
  const zones = Array.from({ length: 60 }, (_, i) => fuzzZone(i));
  // hand-made zones: a mixed-use development with a full table, a house zone with the Census, a school with education modes, trips entered directly
  zones.push({ id: 'zMix', kind: 'zone', name: 'Mixed use', lat: O[0], lng: O[1], role: 'development', year: 2030, reduction: 15, connectors: null, lu: { residential: 320, apartment: 120, retail: 1500, childcare: 60, office: 2400 },
    tg: { on: true, rows: { residential: { basis: 'person', person: { AM: 1.6, PM: 1.8 }, inPct: { AM: 22, PM: 63 }, hours: '24 h', source: 'Illustrative sample rate', year: 2024 }, retail: { veh: { AM: 1.5, PM: 4.5 }, freight: { AM: 0.05, PM: 0.05 }, hvPct: 8, hours: '7am-9pm', source: 'Spare Construct default retail rate', year: 2023 }, childcare: { basis: 'person', person: { AM: 1.2, PM: 1.1 }, inPct: { AM: 55, PM: 45 }, hours: '7am-6pm', driveThrough: false } },
      split: { driver: 72, passenger: 9, pt: 9, walk: 7, cycle: 3 }, internal: 5, passby: 10, existing: { AMin: 12, AMout: 8, PMin: 9, PMout: 15 } } });
  zones.push({ id: 'zHome', kind: 'zone', name: 'Existing homes', lat: O[0], lng: O[1], existing: true, connectors: null, lu: { residential: 240, apartment: 60, school: 400, childcare: 40 }, census: { ...censusRec(), at: '2026-10-21' } });
  zones.push({ id: 'zDirect', kind: 'zone', name: 'Direct trips', lat: O[0], lng: O[1], role: 'committed', connectors: null, lu: {}, extra: { AMin: 30, AMout: 20, PMin: 25, PMout: 35 }, tg: { on: true, other: { AMin: 3, AMout: 4, PMin: 5, PMout: 6 } } });
  zones.push({ id: 'zEmpty', kind: 'zone', name: 'No land use', lat: O[0], lng: O[1], connectors: null });
  out.settings = S;
  out.scenarios = scenarioVariants;
  out.zones = zones;
  out.tripGen = [];
  const caseList = [];
  zones.forEach((z, zi) => { for (const si of [0, 1, 2, 3]) for (const sc of [0, 1, 2, 3]) if (zi >= 60 ? sc === si : (zi * 7 + si * 3 + sc) % 7 === 0) caseList.push([zi, si, sc]); });
  for (const [zi, si, sc] of caseList) {
    const z = JSON.parse(JSON.stringify(zones[zi])), s = S[si], scn = scenarioOf(c, scenarioVariants[sc]);
    for (const period of ['AM', 'PM']) out.tripGen.push({ zone: zi, S: si, sc, period, out: jsonSafe(L.tripGen(z, s, period, scn)) });
  }
  // directly: the mode shares and rate flags
  out.sharesOf = [];
  for (let i = 0; i < 120; i++) { const g = pickOne(['r', 'w', 'e']), m = rnd() < 0.1 ? null : modeCounts(g, rnd() < 0.3); out.sharesOf.push({ g, c: m, out: jsonSafe(L.sharesOf(m, g)) }); }
  out.rateFlag = [undefined, null, '', 0, 1899, 1901, 2015, 2019, 2020, 2021, 2022, 2023, 2025, '2018', '2021', 'x', NaN, 2020.5, 2022.5].map(y => ({ in: y === undefined ? { undefined: true } : typeof y === 'number' && !Number.isFinite(y) ? { nonFinite: String(y) } : y, out: jsonSafe(L.rateFlag(y)) }));

  // how a table reaches the model: sync on a copy of each switched-on zone, and setTable on and off
  out.sync = [];
  for (const [zi, si, sc] of [[0, 0, 0], [1, 1, 1], [2, 2, 2], [3, 3, 3]].concat(zones.map((_, zi) => [zi, zi % 4, zi % 4]))) {
    const z = JSON.parse(JSON.stringify(zones[zi])), s = S[si], scn = scenarioOf(c, scenarioVariants[sc]);
    const changed = L.sync(z, s, scn), again = L.sync(z, s, scn);
    out.sync.push({ zone: zi, S: si, sc, changed, again, z: jsonSafe(z) });
  }
  out.setTable = [];
  for (const [zi, ops] of [[61, ['on', 'off', 'on', 'on', 'off', 'off']], [62, ['off', 'on', 'off']], [63, ['on', 'off']], [0, ['on', 'off']], [1, ['off', 'on']], [5, ['on', 'on', 'off']]]) {
    const z = JSON.parse(JSON.stringify(zones[zi])), s = S[0], scn = scenarioOf(c, scenarioVariants[1]), steps = [];
    for (const op of ops) { L.setTable(z, op === 'on', s, scn); steps.push({ op, z: jsonSafe(z) }); }
    out.setTable.push({ zone: zi, ops, steps });
  }
  // the tables, the CSV and the rate flags for a scenario of several zones, in each scope
  out.tables = [];
  const mk = (ids, si, sc, landuse) => {
    const P = c.newProject('tables'), scn = P.scenarios[0]; Object.assign(scn, scenarioOf(c, scenarioVariants[sc]), { id: scn.id });
    for (const zi of ids) scn.zones[zones[zi].id] = JSON.parse(JSON.stringify(zones[zi]));
    P.settings = JSON.parse(JSON.stringify(S[si])); if (landuse) P.landuse = landuse;
    c.Store.P = P;
    return { P, scn };
  };
  const sets = [[range(0, 9), 0, 0], [range(9, 18), 1, 1], [range(18, 27), 2, 2], [range(27, 33), 3, 3], [[58, 59, 60, 61, 62, 63], 1, 0], [[], 0, 0]];
  for (const [ids, si, sc] of sets) for (const opts of [{}, { scope: 'all' }, { scope: 'all', landuse: { scope: 'all' } }, { zones: ids.slice(0, 2) }]) {
    const { P, scn } = mk(ids, si, sc, opts.landuse), o = opts.zones ? { zones: opts.zones.map(zi => scn.zones[zones[zi].id]) } : opts.scope ? { scope: opts.scope } : {};
    const T = L.tables(scn, P.settings, o);
    out.tables.push({ ids, S: si, sc, opts: { scope: opts.scope ?? null, landuse: opts.landuse ?? null, zones: opts.zones ?? null }, tables: jsonSafe({ zones: T.zones.map(z => z.id), flags: T.flags, sanity: T.sanity }), csv: L.csv(scn, P.settings, o) });
  }
  // syncAll over every scenario of a project
  const P = c.newProject('sync all');
  P.settings = JSON.parse(JSON.stringify(S[1]));
  for (const [i, v] of scenarioVariants.entries()) { const scn = i === 0 ? P.scenarios[0] : (P.scenarios.push(c.newScenario(v.name)), P.scenarios[i]); Object.assign(scn, scenarioOf(c, v), { id: i === 0 ? scn.id : 'sx' + i }); for (const z of zones.slice(0, 40)) if (z.tg && z.tg.on) scn.zones[z.id] = JSON.parse(JSON.stringify(z)); }
  c.Store.P = P;
  const before = jsonSafe(P);
  const n = L.syncAll();
  out.syncAll = { before, n, after: jsonSafe(P.scenarios.map(s => ({ id: s.id, zones: s.zones }))) };
  writeExpected('census-trips.json', jsonSafe(out));
}
function range(a, b) { return Array.from({ length: b - a }, (_, i) => a + i); }

/* ------------------------------------------------------------------------------------------------------------------------------------
   Part 6: zoned yield, then development traffic (the gateways, junctions without and with, the 'without' scenario, one chosen count)
   ------------------------------------------------------------------------------------------------------------------------------------ */
async function partYield() {
  reseed(6100);
  const c = load({ seed: 9 }), L = c.LandusePlus, out = {};
  const user = { landuse: { yields: { 19: { dph: 20 }, 22: { far: 2, use: 'office' }, 49: { use: 'industrial' }, 90: { dph: 10, use: 'bogus' }, 5: { netPct: '55', far: '0.8' }, 4: { dph: '', far: null }, 60: { dph: 70, far: 0.3, netPct: 90, use: 'retail' }, 99: { far: 1.2, use: 'medical', dph: 5 }, 63: { dph: 3 } },
    proposed: { 18: 60, 19: 18, 22: 8, 4: 18, 99: 19, 25: 18, 21: 99, 17: 17, 5: 4 } } };
  const Ps = [null, {}, { landuse: {} }, user, { landuse: { yields: {}, proposed: {} } }];
  out.Ps = Ps;
  const codes = [1, 4, 5, 7, 8, 10, 12, 17, 18, 19, 22, 35, 44, 49, 55, 60, 63, 64, 21, 25, 90, 0, 99, '19', '60'];
  const has = [0, 0.5, 1, 12.345, 100, '3.2', '', 'x', -2, null, 7.77];
  out.yieldOf = [];
  for (const code of codes) for (const pi of [0, 3]) for (const ha of has) if (rnd() < 0.35 || (code === 18 && ha === 12.345)) { const name = rnd() < 0.2 ? 'Named zone' : undefined; out.yieldOf.push({ code, ha, P: pi, name: name ?? null, out: jsonSafe(L.yieldOf(code, ha, Ps[pi], name)) }); }
  for (const code of [18, 19, 22, 49, 5, 4, 99, 63, 60]) for (const pi of [1, 2, 4]) out.yieldOf.push({ code, ha: 4.5, P: pi, name: null, out: jsonSafe(L.yieldOf(code, 4.5, Ps[pi])) });
  const base = [{ code: 18, name: 'Residential - Mixed Housing Suburban', group: 'Residential', ha: 69.7, n: 2 }, { code: 4, name: 'Future Urban', group: 'Future Urban', ha: 23.3, n: 1 }, { code: 17, name: 'Business - Light Industry', group: 'Business', ha: 10.5, n: 1 },
    { code: 19, name: 'Residential - Single House', group: 'Residential', ha: 10.1, n: 1 }, { code: 21, name: 'Business - Metropolitan Centre Fringe', group: 'Business', ha: 9.2, n: 1 }, { code: 30, name: 'Coastal - General Coastal Marine', group: 'Coastal', ha: 8.3, n: 1 },
    { code: 60, name: 'Residential - Mixed Housing Urban', group: 'Residential', ha: 5.6, n: 1 }, { code: 25, name: 'Road', group: 'Strategic Transport Corridor', ha: 4.8, n: 1 }, { code: 22, name: 'Business - Town Centre', group: 'Business', ha: 3.6, n: 1 },
    { code: 44, name: 'Business - Neighbourhood Centre', group: 'Business', ha: 2.5, n: 1 }, { code: 63, name: 'Special Purpose - School', group: 'Special Purpose', ha: 2, n: 2 }, { code: 90, name: 'Open Space - Informal Recreation', group: 'Public Open Space', ha: 1.3, n: 1 },
    { code: 8, name: 'Residential - Terrace Housing and Apartment Building', group: 'Residential', ha: 1.3, n: 1 }, { code: 99, name: 'Unknown zone', ha: 6, n: 1 }, { code: 5, name: 'Business - Heavy Industry', group: 'Business', ha: 3.3, n: 1 }, { code: 49, name: 'Business - General Business', group: 'Business', ha: 2.2, n: 1 },
    { code: 55, group: 'Special Purpose', ha: 1.1, n: 1 }, { code: 31, name: 'Open Space - Sport', group: 'Public Open Space', ha: 12, n: 1 }, { code: 91, name: 'Marine', group: 'COASTAL', ha: 4, n: 1 }];
  out.summaries = { base, none: [], oddRows: [{ code: '18', ha: '5', name: '' }, { code: 7, ha: null }, { code: '19', ha: 3 }] };
  out.zonedCapacity = [];
  for (const [sname, summary] of Object.entries(out.summaries)) for (const pi of [0, 1, 3, 4]) for (const codesOpt of [null, [18, 19, 22], [4, 99]]) out.zonedCapacity.push({ summary: sname, P: pi, codes: codesOpt, out: jsonSafe(L.zonedCapacity(summary, { P: Ps[pi], ...(codesOpt ? { codes: codesOpt } : {}) })) });
  out.applyYield = [];
  for (const [code, ha, pi] of [[18, 3.2, 0], [22, 1.5, 3], [4, 10, 0], [99, 2, 3], [60, 1, 3], [63, 2.5, 0], [1, 0, 0], [17, 8.25, 3]]) {
    const z = { id: 'zy', kind: 'zone', name: 'Yield zone', lat: O[0], lng: O[1], connectors: null, lu: { residential: 9, retail: 100 }, luSrc: { residential: { source: 'x', basis: 'user', q: 9 } }, osm: { residential: 3 } };
    const y = L.applyYield(z, code, ha, Ps[pi]);
    out.applyYield.push({ code, ha, P: pi, y: jsonSafe(y), z: jsonSafe(z) });
  }
  writeExpected('census-yield.json', jsonSafe(out));
}

// The project of the traffic tests: the corridor with the proposal and four other scenarios, counts, and results for each
async function partTraffic() {
  reseed(6200);
  const inputP = readJson('inputs/corridor.json');
  const onMain = (lng, d = 0) => ({ lat: -37.1 + d / 110574, lng });
  const counts = [
    { id: 'nzta:01N00478', src: 'nzta', ref: '01N00478', name: 'SH1 Main Road west', road: 'Main Road', ...onMain(174.9514, 6), adt: 18200, hv: 6, dir: 'EB', directional: true, virtual: false, ramp: false },
    { id: 'nzta:01N00479', src: 'nzta', ref: '01N00479', name: 'SH1 Main Road west', road: 'Main Road', ...onMain(174.9514, -6), adt: 17650, hv: 6, dir: 'WB', directional: true, virtual: false, ramp: false },
    { id: 'at:991', src: 'at', ref: '991', name: 'Main Road · Station Road to Side 4', road: 'Main Road', ...onMain(174.9672, 3), adt: 14500, peak: 1320, peakTime: '07:45', directional: false },
    { id: 'nzta:V1', src: 'nzta', ref: 'V1', name: 'Virtual Main Road east', road: 'Main Road', ...onMain(174.9695), adt: 12000, directional: false, virtual: true },
    { id: 'user:1', src: 'user', name: 'Side 2 count', road: 'Side 2N', lat: -37.0990, lng: 174.9556, am: 300, pm: 280, directional: false },
    { id: 'at:992', src: 'at', ref: '992', name: 'Side 4 count', road: 'Side 4N', lat: -37.0990, lng: 174.9612, adt: 4000, directional: false, off: true },
    { id: 'nzta:01N00480', src: 'nzta', ref: '', name: 'Unreferenced site', road: 'Main Road', ...onMain(174.9608), adt: 9000, directional: true, dir: 'EB' },
    { id: 'at:993', src: 'at', ref: '993', name: 'Main Road east', road: 'Main Road', ...onMain(174.9690, 2), adt: 21000, directional: false },
  ];
  const mkProject = () => {
    const P = JSON.parse(JSON.stringify(inputP)), base = P.scenarios[0];
    base.id = 'sWith'; base.name = 'Proposal 2030'; base.role = 'proposal'; base.year = 2030; P.active = 'sWith';
    const noDev = id => { const s = JSON.parse(JSON.stringify(base)); s.id = id; delete s.zones.zDev; return s; };
    const a = noDev('sBase'); a.name = 'Do-minimum 2030'; a.role = 'baseline';
    const b = noDev('sOld'); b.name = 'Existing 2026'; b.role = 'existing'; b.year = 2026;
    const m = JSON.parse(JSON.stringify(base)); m.id = 'sMit'; m.name = 'Mitigation 2030'; m.role = 'mitigation';
    const o = noDev('sOther'); o.name = 'Other 2030'; delete o.role;
    P.scenarios = [base, a, b, m, o];
    P.base = 'sOld';
    P.data = { counts: JSON.parse(JSON.stringify(counts)) };
    return P;
  };
  const project = mkProject();
  writeData('census-traffic-project.json', jsonSafe(project));
  const c = load({ seed: 12 }), L = c.LandusePlus, { Engine, Store } = c, out = {};
  Store.P = JSON.parse(JSON.stringify(project));
  const P0 = Store.P;
  for (const s of P0.scenarios) for (const period of ['AM', 'PM']) { const r = await Engine.assign(s, { period }); s.res[period] = r; }
  const withRes = jsonSafe(P0.scenarios.map(s => s.res));
  const restore = () => { const p = JSON.parse(JSON.stringify(project)); p.scenarios.forEach((s, i) => { s.res = JSON.parse(JSON.stringify(withRes[i])); }); Store.P = p; return p; };
  const scOf = (p, id) => p.scenarios.find(s => s.id === id);
  const dist = scn => { const d = L.distribution(scn); return d && jsonSafe({ rows: d.rows, periods: d.periods, label: d.label, sums: d.sums, basis: d.basis, stale: d.stale }); };
  out.cases = [];
  const variants = [
    { name: 'plain', f: () => {} },
    { name: 'some gateways from counts', f: p => { scOf(p, 'sWith').zones.gE.gwSrc = 'x'; } },
    { name: 'every gateway from counts', f: p => { for (const z of Object.values(scOf(p, 'sWith').zones)) if (z.kind === 'gateway') z.gwSrc = 'NZTA 01N00478 SH1 Main Road west (18,200/day)'; } },
    { name: 'matrix estimation factors', f: p => { p.me = { AM: { 'gW>zDev': 1.1, 'zEx>gE': 0.9 }, PM: { 'gW>zDev': 1 }, info: {} }; } },
    { name: 'stale results', f: p => { scOf(p, 'sWith').res.AM.stale = true; } },
    { name: 'a manual matrix', f: p => { scOf(p, 'sWith').res.PM.manual = true; } },
    { name: 'no gateways', f: p => { for (const [id, z] of Object.entries(scOf(p, 'sWith').zones)) if (z.kind === 'gateway') delete scOf(p, 'sWith').zones[id]; } },
    { name: 'without stale', f: p => { scOf(p, 'sBase').res.PM.stale = true; } },
  ];
  for (const v of variants) {
    const p = restore(); v.f(p);
    const scn = scOf(p, 'sWith'), ww = {};
    for (const wo of ['sBase', 'sOld', 'sMit']) for (const period of ['AM', 'PM']) { const r = L.withWithout(scn, scOf(p, wo), period); ww[`${wo}|${period}`] = r && jsonSafe({ rows: r.rows, note: r.note, label: r.label, stale: r.stale, period: r.period }); }
    out.cases.push({ name: v.name, distribution: dist(scn), withWithout: ww, others: Object.fromEntries(p.scenarios.map(s => [s.id, dist(s)])) });
  }
  // withoutScenario: the user's choice, then each rule in turn
  out.without = [];
  const setups = [
    ['automatic', {}, () => {}],
    ['chosen', { without: 'sOther' }, () => {}],
    ['chosen scenario is gone', { without: 'nope' }, () => {}],
    ['chosen is itself', { without: 'sWith' }, () => {}],
    ['no baseline, base is same year', {}, p => { scOf(p, 'sBase').role = 'sensitivity'; p.base = 'sBase'; }],
    ['no baseline, another same-year scenario', {}, p => { scOf(p, 'sBase').role = 'proposal'; scOf(p, 'sMit').role = 'mitigation'; }],
    ['only a different year', {}, p => { for (const s of p.scenarios) if (s.id !== 'sWith' && s.id !== 'sOld') s.year = 2040; }],
    ['nothing else', {}, p => { p.scenarios = p.scenarios.filter(s => s.id === 'sWith'); }],
    ['project year', {}, p => { for (const s of p.scenarios) delete s.year; p.settings.year = 2026; }],
  ];
  for (const [name, landuse, mod] of setups) {
    const p = restore(); p.landuse = landuse; mod(p); Store.P = p;
    out.without.push({ name, landuse, picked: (L.withoutScenario(scOf(p, 'sWith'), p) || {}).id ?? null });
  }
  // the count behind a gateway's flows
  Store.P = restore();
  const labels = Store.P.data.counts.map(k => `${(c.DataHub.SOURCES[k.src] || { short: k.src }).short}${k.virtual ? ' estimate' : ''} ${k.ref || ''} ${k.name}${k.adt ? ` (${c.U.fmt.int(k.adt)}/day)` : ''}`);
  out.labels = labels;
  const srcs = ['', undefined, labels[0], `${labels[0]}; ${labels[1]}`, labels[2], labels[3], labels[6], `${labels[4]}; ${labels[2]}; unknown count`, 'nothing known', `${labels[0]}; `, labels[7], 'x; y; z', labels[5]];
  out.gwSrcText = srcs.map(s => { const g = { id: 'g', kind: 'gateway' }; if (s !== undefined) g.gwSrc = s; return { gwSrc: s ?? null, out: L.gwSrcText(g) }; });
  out.gwSrcText.push({ gwSrc: 'no zone', out: L.gwSrcText(null) });
  out.useCount = [];
  for (const [gw, cid] of [['gW', 'nzta:01N00478'], ['gW', 'nzta:01N00479'], ['gE', 'at:993'], ['gE', 'at:991'], ['gE', 'nzta:V1'], ['gW', 'user:1'], ['gW', 'at:992'], ['gW', 'nope'], ['zDev', 'at:991'], ['gone', 'at:991'], ['gE', 'nzta:01N00480'], ['gW', 'nzta:01N00480'], ['gW', 'nzta:V1']]) {
    const p = restore();
    if (cid === 'nzta:V1') p.data.use = { virtual: true };
    const scn = p.scenarios[0], before = jsonSafe(scn.zones[gw]), r = L.useCount(scn, gw, cid);
    out.useCount.push({ gw, count: cid, useVirtual: cid === 'nzta:V1', out: jsonSafe(r), before: before ?? null, after: jsonSafe(scn.zones[gw]) ?? null, off: jsonSafe(p.data.counts.map(k => k.off ?? null)) });
  }
  writeExpected('census-traffic.json', jsonSafe(out));
}

const parts = process.argv.slice(2);
const want = n => parts.length === 0 || parts.includes(n);
if (want('functions')) await partFunctions();
if (want('fetch')) await partFetch();
if (want('apply')) await partApply();
if (want('fill')) await partFill();
if (want('trips')) await partTrips();
if (want('yield')) await partYield();
if (want('traffic')) await partTraffic();
saveExchanges();
