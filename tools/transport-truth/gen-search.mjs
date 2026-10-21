// Records the prototype's place search (search.js: parseCoords, expand, shorten, geocode and the way Nominatim's answer is turned into
// places and areas) for the Rust port's search module (projects/spare-construct/crates/construct-transport/src/search, tests/search_truth.rs).
//
// Nothing is downloaded. The answers are made up in the shape Nominatim's jsonv2 format gives (fixtures/data/nominatim-answer.json), and
// the prototype runs against a stand-in for fetch that returns them; every request is recorded with the address it asked for.
// The rate limit and the cache are the host's affair in the Rust port and are not recorded. Deterministic.
// Writes fixtures/data/nominatim-answer.json and expected/search-cases.json. Run: node gen-search.mjs
process.env.TZ = 'Pacific/Auckland';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';
import { JS_DIR } from './js-core.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const NOW = Date.UTC(2026, 9, 21);
mkdirSync(join(here, 'fixtures', 'data'), { recursive: true });

function loadSearch(fetchImpl) {
  const ctx = {
    console, JSON, Promise, Map, Set, Array, Object, Number, String, Boolean, Symbol, Error, TypeError, RangeError, Float64Array, Uint32Array, Int32Array, ArrayBuffer, DataView, Intl, RegExp,
    Math, Date: class extends Date { constructor(...a) { super(...(a.length ? a : [NOW])); } static now() { return NOW; } },
    setTimeout, clearTimeout, URL, URLSearchParams, AbortController, performance: { now: () => 0 }, fetch: fetchImpl, isFinite, isNaN, parseFloat, parseInt, Infinity, NaN, encodeURIComponent, decodeURIComponent,
    localStorage: { getItem: () => null, setItem() {} },
  };
  ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
  vm.createContext(ctx);
  const src = ['util', 'search'].map(n => readFileSync(join(JS_DIR, n + '.js'), 'utf8')).join('\n;\n');
  // the rate limit waits on a timer: the clock is fixed, so the waits are skipped
  vm.runInContext(src + `\n;U.sleep = () => Promise.resolve(); globalThis.__s = { Search, U };`, ctx);
  return ctx.__s;
}
const plain = v => (typeof v === 'number' ? (Number.isFinite(v) ? v : String(v)) : Array.isArray(v) ? v.map(plain) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).filter(([, x]) => x !== undefined).map(([k, x]) => [k, plain(x)])) : v);

const COORDS = ['-37.10, 174.95', '-37.10 174.95', '174.95, -37.10', '37.1 S, 174.95 E', '37.1S 174.95E', '174.95 E, 37.1 S', '174.95E,37.1S', '-37.1;174.9', '-37.1°, 174.9°', '-37.1° S 174.9° E', '45 N, 170 W', '+37.1 +174.9', '  -37.12345678 , 174.987654321  ', '91, 50', '95, 175', '-37.1, 181', '1,2', '12.5 N 100 W', '37S 174E', '0 0', '0, 0', '-0, 0', '-37.1', '-37.1,', 'hello', '', '37.1 174.9 12', '-37.1, 174.9 extra', '1234, 5', '-37, 174, 5', '160 -37', '165, -45', '160, -91', '175.5, -37.5', '90, 180', '-90.0, -180.0', '90.1 180.1', '37.1 N, 174.9 E', '37.1 N, 174.9 W', 'S 37.1, E 174.9', '37.1 e, 174.9 s', '-37,5 174,9', '.5 .5', '37. 174.', '1e2, 3', '\t-37.1\t174.9\t', '-37.1\n174.9', '-37.1 ,174.9', '+0.5, -0.5', '100, 50', '-170, -37', '174.95, 37.1', '161, -37', '160, -37', '159.99, -37', '175, 5', '-37.1 S, 174.9 E', '37.1 NE', '37.1N,174.9N', '37.1E, 174.9N', '12345', '1.5, 2.5, 3.5'];
const EXPAND = ['Waihoehoe Rd', 'Waihoehoe Rd.', 'waihoehoe rd', 'WAIHOEHOE RD', 'Great South Rd, Papakura', 'Rd 2', 'St Heliers', 'St Heliers Bay Rd', 'Smith Dr', 'Smith Dr.', 'Smith Dr, Auckland', 'Dr Smith Dr', 'Main St', 'Main St.', 'Main St, Ellerslie', 'St Marks Rd', '12 St Marks Rd, Remuera', 'Great North Rd Pl', 'Queen Ave', 'Queen Av', 'Queen Ave.', 'Cres', 'Hobson Cres', 'Hobson Cres.', 'Southern Hwy', 'Karangahape Tce', 'Marine Pde', 'Mt Eden Ln', 'Southern Mwy', 'Sunset Blvd', 'Ct Court', 'Smith Ct', 'Smith Cl', 'Smith Cr', 'Smith Sq', 'Smith Sq., Auckland', 'Rd', 'rd', 'x rd', 'x  rd', 'x\trd', 'xrd', 'x rdx', 'x rd2', 'x rd,y', 'x rd y', 'x rd. y', 'a rd b rd', 'a rd rd', 'a st b st', 'a st st', 'a st, b st, c st', 'a st , b', 'a st  ,b', 'a st  ', 'a st.  ', '  a  rd  ', 'Pl Pl', 'Ave Ave', 'ave ave', 'x Ave, y Rd, z St', 'Great South Rd Papakura', 'Takanini Rd & Papakura Rd', 'Mill Rd/Alfriston Rd', "O'Brien Rd", 'Māori Rd', 'Mānuka St', 'É Rd', 'x rd\nnext', 'x rd\n', 'x St. Marys', 'x Cl 5', 'x cl', 'ln', 'x ln.', 'x, rd', 'x ,rd', ', rd', 'x mwy', 'x Mwy.', 'x  Hwy 1', 'x Hwy.', 'x blvd.', 'x Blvd, y', 'x tce, y tce', 'x pde pde', '123 Smith St', '123 Smith St Ellerslie', 'Smith St Ellerslie Auckland'];
const SHORTEN = ['12, St Marks Road, Remuera, Auckland, 1050, New Zealand / Aotearoa', 'St Marks Road, Remuera, Auckland, New Zealand', 'Papakura, Auckland, 2110, New Zealand / Aotearoa', 'New Zealand / Aotearoa', 'New Zealand', 'Aotearoa / New Zealand', 'Aotearoa', 'new zealand/aotearoa', 'NEW ZEALAND  /  AOTEAROA', '1050', '1050, Auckland', 'Auckland, 1050', '12, 14, Smith Street', '12a, Smith Street, Ellerslie', '12-14, Smith Street, Ellerslie', '3/45, Smith Street, Ellerslie', '12, Smith Street', '12', '12, ', ' , 12, Smith', 'A, B, C, D, E', 'A, B, C', 'A, B', 'A', '', ' ', ',,,', ', ,A, ,', 'A,,B', '  A  ,  B  ', '1, 2, 3, 4', '12b-3, Smith Street', '12/3b, Smith Street', '12 3, Smith Street', '12.5, Smith Street', 'The Warehouse, 12, Smith Street, Ellerslie, Auckland, 1051, New Zealand / Aotearoa', 'Mānuka Road, Bayview, Auckland, 0629, New Zealand / Aotearoa', "St Mary's Bay, Auckland, New Zealand", '12, Smith, 1234, X', '0629, A', 'A, 0629', 'Place, 12345, X'];
const ROW = (o) => ({ place_id: 1, licence: 'Data © OpenStreetMap contributors, ODbL 1.0. http://osm.org/copyright', osm_type: 'way', osm_id: 12345, category: 'highway', type: 'residential', place_rank: 26, importance: 0.5, addresstype: 'road', name: '', display_name: '', ...o });
const ANSWER = [
  ROW({ lat: '-37.0611234', lon: '174.9412345', display_name: 'Great South Road, Papakura, Auckland, 2110, New Zealand / Aotearoa', name: 'Great South Road', boundingbox: ['-37.0650000', '-37.0570000', '174.9380000', '174.9450000'] }),
  ROW({ lat: '-36.9140000', lon: '174.8850000', display_name: 'Manukau, Auckland, New Zealand / Aotearoa', name: 'Manukau', addresstype: 'suburb', category: 'boundary', type: 'administrative', boundingbox: ['-37.0200000', '-36.8700000', '174.8100000', '174.9900000'] }),
  ROW({ lat: '-36.8512345', lon: '174.7654321', display_name: '12, Smith Street, Ellerslie, Auckland, 1051, New Zealand / Aotearoa', addresstype: 'house', category: 'place', type: 'house', boundingbox: ['-36.8512800', '-36.8511900', '174.7653900', '174.7654700'] }),
  ROW({ lat: '-41.2865', lon: '174.7762', display_name: 'Wellington, New Zealand / Aotearoa', name: 'Wellington', addresstype: 'city', type: 'city', category: 'place' }),                         // no bounding box
  ROW({ lat: -43.5321, lon: 172.6362, display_name: 'Christchurch', name: 'Christchurch', addresstype: undefined, type: 'city', category: 'place', boundingbox: [-43.65, -43.4, 172.5, 172.8] }),   // numbers, not text
  ROW({ lat: '-45.0312', lon: '168.6626', display_name: '', name: 'Queenstown', addresstype: '', type: '', category: 'place', boundingbox: ['-45.04', '-45.02', '168.65', '168.67'] }),                  // a name only, and a small box
  ROW({ lat: 'x', lon: '174.0', display_name: 'Bad latitude' }), ROW({ lat: '-37.1', lon: null, display_name: 'No longitude' }), ROW({ lon: '174.1', display_name: 'Missing latitude' }),
  ROW({ lat: '-37.2', lon: '174.2', display_name: 'Three box values', boundingbox: ['-37.3', '-37.1', '174.1'] }), ROW({ lat: '-37.2', lon: '174.2', display_name: 'Bad box value', boundingbox: ['-37.3', 'x', '174.1', '174.3'] }),
  ROW({ lat: '-37.25', lon: '174.25', display_name: 'Tall thin box', boundingbox: ['-37.30', '-37.20', '174.2500', '174.2501'] }), ROW({ lat: '-37.25', lon: '174.25', display_name: 'Wide short box', boundingbox: ['-37.2500', '-37.2501', '174.20', '174.30'] }),
  ROW({ lat: '-37.3', lon: '174.3', display_name: '1, 2, Name, 3333', name: undefined, category: 'amenity', type: 'cafe', addresstype: undefined }), ROW({ lat: '  -37.4 ', lon: '174.4abc', display_name: undefined, name: 'Padded numbers', type: undefined, category: undefined, addresstype: undefined }),
  ROW({ lat: '-37.5', lon: '174.5', display_name: 'Exactly 500 m', boundingbox: ['-37.5022635', '-37.4977365', '174.4972', '174.5028'] }), ROW({ lat: '0', lon: '0', display_name: 'Null Island', boundingbox: ['-0.1', '0.1', '-0.1', '0.1'] }),
  ROW({ lat: '-90', lon: '180', display_name: 'The edge of the map', boundingbox: ['-90', '-89.99', '179.99', '180'] }),
];
writeFileSync(join(here, 'fixtures', 'data', 'nominatim-answer.json'), JSON.stringify(ANSWER));

async function main() {
  const out = { coords: [], expand: [], shorten: [], boxes: [], geocode: [] };
  const { Search } = loadSearch(() => { throw new Error('no network'); });
  for (const q of COORDS) out.coords.push({ q, result: plain(Search.parseCoords(q)) });
  for (const q of [null, undefined, 12, 0]) out.coords.push({ q: q === undefined ? { undefined: true } : q, result: plain(Search.parseCoords(q)) });
  for (const q of EXPAND) out.expand.push({ q, result: Search.expand(q) });
  for (const q of SHORTEN) out.shorten.push({ q, result: Search.shorten(q) });
  for (const q of [null, undefined]) out.shorten.push({ q: q === undefined ? { undefined: true } : q, result: Search.shorten(q) });
  out.gap = Search.GAP;
  // the boxes around points (through parseCoords with a spread of latitudes)
  for (let lat = -90; lat <= 90; lat += 7.5) for (const lng of [-180, -37.5, 0, 174.25, 180]) { const r = Search.parseCoords(`${lat}, ${lng}`); out.boxes.push({ lat, lng, result: plain(r) }); }
  // geocode: every kind of query and answer, with the requests it makes
  const queries = ['Papakura', ' Waihoehoe Rd, Papakura ', 'Great South Rd', '12 St Marks Rd, Remuera', '-37.1, 174.9', '174.9 E, 37.1 S', '', '   ', 'St Heliers', 'Smith Dr', 'Main St, Ellerslie', 'a   b\tc', 'Mānuka Road & Co. "x"', "O'Brien Rd #5", 'q=1&limit=100', 'x+y', 'ünïcode café', '100%', 'a/b', 'Hobson Cres.', 'ROTORUA'];
  const answers = [['ok', { status: 200, body: ANSWER }], ['empty', { status: 200, body: [] }], ['not-a-list', { status: 200, body: { error: 'x' } }], ['null', { status: 200, body: null }], ['busy', { status: 429, body: {} }], ['server-error', { status: 500, body: {} }], ['not-found', { status: 404, body: {} }], ['network', 'net'], ['bad-json', { status: 200, body: 'throw' }], ['box-not-a-list', { status: 200, body: [ROW({ lat: '-37.2', lon: '174.2', display_name: 'Box not a list', boundingbox: 'wrong' })] }], ['box-number', { status: 200, body: [ROW({ lat: '-37.2', lon: '174.2', display_name: 'Box a number', boundingbox: 5 })] }], ['box-empty-text', { status: 200, body: [ROW({ lat: '-37.2', lon: '174.2', display_name: 'Box empty text', boundingbox: '' })] }], ['null-element', { status: 200, body: [ROW({ lat: '-37.2', lon: '174.2' }), null] }], ['odd-elements', { status: 200, body: [5, 'x', true, [], ROW({ lat: '-37.2', lon: '174.2', display_name: 'Fine' })] }]];
  for (const [label, a] of answers) for (const q of queries) {
    const log = [];
    const fetchImpl = async (url, opts) => { log.push({ url, opts: opts ?? null }); if (a === 'net') throw new TypeError('Failed to fetch'); return { ok: a.status >= 200 && a.status < 300, status: a.status, json: async () => { if (a.body === 'throw') throw new SyntaxError('Unexpected end of JSON input'); return JSON.parse(JSON.stringify(a.body)); } }; };
    const { Search: S } = loadSearch(fetchImpl);
    const rec = { label, q };
    try { rec.result = plain(await S.geocode(q)); } catch (e) { rec.thrown = e.message; }
    rec.requests = log.map(r => r.url);
    out.geocode.push(rec);
  }
  // geocode of null and undefined
  for (const q of [null, undefined]) { const { Search: S } = loadSearch(() => { throw new Error('no network'); }); out.geocode.push({ label: 'nothing', q: q === undefined ? { undefined: true } : q, result: plain(await S.geocode(q)), requests: [] }); }
  writeFileSync(join(here, 'expected', 'search-cases.json'), JSON.stringify(out));
  console.log('expected/search-cases.json', JSON.stringify(out).length, 'bytes');
}
await main();
