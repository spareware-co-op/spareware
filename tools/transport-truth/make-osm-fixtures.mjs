// Lays out the OpenStreetMap test data for the import truth tests (gen-import.mjs): a made-up suburb of Papakura, Auckland, in
// Overpass API's JSON answer format (`[out:json]; way[highway]...; out body; >; out body qt;`: ways first, then their nodes in
// quadtile order), the way the prototype's importer asks for it. Nothing is downloaded; the roads, ids and tags are invented
// but follow how NZ roads are mapped: LINZ-style names, lanes and maxspeed tags, bus lanes, cycleways, a roundabout, a motorway
// with ramps, signals tagged at junctions and on stop lines, stop signs, one-way streets, driveways and private access, and
// ways that run off the edge of the clipped area (nodes missing from the answer).
//
// Writes fixtures/overpass-papakura.json (the whole area) and fixtures/overpass-papakura-east.json (the eastern half, overlapping
// the first, as a second import of an extended view would be) and fixtures/overpass-bare.json (a handful of elements with
// missing or odd tags and values). Deterministic.
import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const out = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
mkdirSync(out, { recursive: true });

const O = [-37.065, 174.94];
const KM = 111320 * Math.cos(O[0] * Math.PI / 180), KY = 110574;
const at = (dx, dy) => [Math.round((O[0] + dy / KY) * 1e7) / 1e7, Math.round((O[1] + dx / KM) * 1e7) / 1e7];

// A seeded generator for ids
let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };

const nodes = new Map();   // name -> { id, lat, lon, tags }
let nextNode = 1234567811;
const node = (name, dx, dy, tags) => {
  if (nodes.has(name)) throw new Error('duplicate node ' + name);
  nextNode += 1 + Math.floor(rnd() * 97);
  const [lat, lon] = at(dx, dy);
  nodes.set(name, { type: 'node', id: nextNode, lat, lon, ...(tags ? { tags } : {}) });
  return name;
};
const ways = [];
let nextWay = 41234500;
const way = (tags, names, extra = {}) => {
  nextWay += 1 + Math.floor(rnd() * 5000);
  const w = { type: 'way', id: nextWay, nodes: names.map(n => { const e = nodes.get(n); if (!e && !extra.missing?.includes(n)) throw new Error('no node ' + n); return e ? e.id : 9000000000 + names.indexOf(n); }), tags };
  ways.push({ w, names });
  return w;
};
// A straight run of nodes between two points, `n` segments, named base0..basen (only the interior ones are made)
const run = (base, x0, y0, x1, y1, n, tagsAt = {}) => {
  const names = [];
  for (let i = 0; i <= n; i++) {
    const name = `${base}${i}`;
    if (!nodes.has(name)) node(name, x0 + (x1 - x0) * i / n, y0 + (y1 - y0) * i / n + (i % 2 ? 0.6 : -0.4), tagsAt[i]);
    names.push(name);
  }
  return names;
};

/* ---- Great South Road: a primary road, four lanes with a bus lane, through two signalised junctions and a roundabout ---- */
// West to east along y ~ 20, with the junctions at x = -100 (Wood Street), 150 (Opaheke Road), the roundabout at 380
node('gsr0', -400, 22); node('gsr1', -330, 21); node('gsr2', -260, 20); node('gsr3', -190, 21); node('gsr4', -140, 19.5);
node('J1', -100, 19, { highway: 'traffic_signals' });
node('gsr5', -50, 20); node('gsr6', 0, 21); node('gsr7', 60, 23); node('gsr8', 110, 25);
node('J2', 150, 26);
node('gsr9', 200, 28); node('gsr10', 250, 30); node('xing', 300, 31, { highway: 'traffic_signals', crossing: 'traffic_signals' }); node('gsr11', 335, 33);
way({ highway: 'primary', name: 'Great South Road', ref: 'AT-GSR', lanes: '4', maxspeed: '50', 'cycleway:both': 'lane', 'bus:lanes': 'designated|yes|yes|yes' }, ['gsr0', 'gsr1', 'gsr2', 'gsr3', 'gsr4', 'J1']);
way({ highway: 'primary', name: 'Great South Road', lanes: '3', maxspeed: '50', 'bus:lanes': 'designated|yes|yes', cycleway: 'shared_lane' }, ['J1', 'gsr5', 'gsr6', 'gsr7', 'gsr8', 'J2']);
way({ highway: 'primary', name: 'Great South Road', lanes: '2', maxspeed: '50' }, ['J2', 'gsr9', 'gsr10', 'xing', 'gsr11']);
// 3 lanes with a bus lane tagged by psv, into the roundabout
// a roundabout at the east end: 12 nodes on a circle of radius 14 m centred (385, 36); rb0 is east, rb3 north, rb6 west, rb9 south
const rb = []; for (let i = 0; i < 12; i++) { const a = i * Math.PI / 6; rb.push(node(`rb${i}`, 385 + 14 * Math.cos(a), 36 + 14 * Math.sin(a))); }
nodes.get('rb6').tags = { highway: 'give_way' };
way({ highway: 'primary', name: 'Great South Road', lanes: '3', 'lanes:psv': 'bus_lane', maxspeed: '60', 'cycleway:right': 'track' }, ['gsr11', 'rb6']);
way({ highway: 'primary', junction: 'roundabout', name: 'Great South Road', lanes: '2', maxspeed: '30' }, [...rb, rb[0]]);
node('gsr13', 470, 36); node('gsr14', 500, 36);
way({ highway: 'primary', name: 'Great South Road', lanes: '2', maxspeed: '50' }, ['rb0', 'gsr13', 'gsr14']);
node('kirk1', 385, 110); node('kirk2', 385, 200);
way({ highway: 'residential', name: 'Kirk Street', lanes: '2', maxspeed: '50' }, ['rb3', 'kirk1', 'kirk2']);
node('bell1', 385, -60); node('bell2', 385, -160); node('bell3', 385, -250);
way({ highway: 'tertiary', name: 'Bell Road', lanes: '2', maxspeed: '50', 'cycleway:left': 'lane' }, ['rb9', 'bell1', 'bell2', 'bell3']);

/* ---- Wood Street: a secondary road, north-south through the signals at J1 ---- */
node('w0', -100, -340); node('w1', -100, -250); node('w2', -100, -160); node('w3', -100, -80); node('w4', -100, -30);
node('w5', -100, 70); node('w6', -100, 140); node('w7', -100, 230); node('w8', -100, 320); node('w9', -100, 395);
way({ highway: 'secondary', name: 'Wood Street', lanes: '2', maxspeed: '50', 'cycleway:both': 'lane' }, ['w0', 'w1', 'w2', 'w3', 'w4', 'J1']);
way({ highway: 'secondary', name: 'Wood Street', lanes: '2', maxspeed: '50' }, ['J1', 'w5', 'w6']);
way({ highway: 'secondary', name: 'Wood Street', oneway: 'no', lanes: '1', maxspeed: '40' }, ['w6', 'w7', 'w8', 'w9']);

/* ---- Opaheke Road: a tertiary road; its signals are tagged on the stop line 38 m south of the junction ---- */
node('o0', 150, -330); node('o1', 150, -240); node('o2', 150, -150); node('o3', 150, -70); node('o4', 150, -12, { highway: 'traffic_signals' });
node('o5', 150, 90); node('o6', 150, 170); node('o7', 150, 260); node('o8', 150, 340);
way({ highway: 'tertiary', name: 'Opaheke Road', lanes: '2', maxspeed: '50' }, ['o0', 'o1', 'o2', 'o3', 'o4', 'J2']);
way({ highway: 'tertiary', name: 'Opaheke Road', lanes: '2', maxspeed: '50' }, ['J2', 'o5', 'o6', 'o7', 'o8']);

/* ---- A residential grid north-west of Wood Street: three streets east-west, three north-south ---- */
// The east-west streets end on Wood Street's own nodes (w6, w7, w8): a stop sign is tagged on one of the junction nodes
nodes.get('w7').tags = { highway: 'stop' };
nodes.get('w8').tags = { highway: 'give_way' };
const ew = [['Elliot Street', 140, 'w6'], ['Averill Street', 230, 'w7'], ['Hunter Street', 320, 'w8']];
const nsX = [-340, -260, -180];
ew.forEach(([name, y, end], k) => {
  const names = [];
  for (let i = 0; i < 4; i++) {
    const x = [-380, ...nsX][i];
    // the cul-de-sac's connecting node sits on Hunter Street between its third and fourth nodes
    if (k === 2 && i === 3) names.push(node('lpjoin', -230, y));
    names.push(node(`e${k}x${i}`, x, y + (i % 2) * 0.8));
  }
  names.push(end);
  const tags = { highway: 'residential', name, maxspeed: k === 1 ? '30' : '50' };
  if (k === 1) tags.oneway = 'yes';
  if (k === 2) { tags.lanes = '2'; tags.oneway = 'no'; }
  way(tags, names);
});
nsX.forEach((x, j) => {
  const names = [`e0x${j + 1}`, node(`n${j}a`, x, 190 + j), `e1x${j + 1}`, node(`n${j}b`, x, 275 - j), `e2x${j + 1}`];
  way({ highway: 'residential', name: ['Selwyn Avenue', 'Kauri Place', 'Rimu Court'][j], ...(j === 1 ? { oneway: '-1' } : j === 2 ? { oneway: 'reverse', lanes: '1' } : {}) }, names);
});
// a cul-de-sac loop, and a closed way too small to use
const loop = []; for (let i = 0; i < 7; i++) { const a = i * 2 * Math.PI / 7; loop.push(node(`lp${i}`, -230 + 25 * Math.cos(a), 345 + 25 * Math.sin(a))); }
way({ highway: 'residential', name: 'Loop Close', lanes: '1' }, [...loop, loop[0]]);
way({ highway: 'residential', name: 'Loop Close' }, ['lpjoin', 'lp5']);
node('tri0', -300, 355); node('tri1', -290, 355); node('tri2', -295, 363);
way({ highway: 'residential', name: 'Tiny Close' }, ['tri0', 'tri1', 'tri2', 'tri0']);

/* ---- Service roads, driveways and private ways ---- */
node('sv0', 30, 60); node('sv1', 60, 80); node('sv2', 90, 85);
node('sv3', 20, -20); node('sv4', 50, -40);
node('sv5', 220, 60); node('sv6', 260, 90);
node('sv7', 20, 120); node('sv8', 70, 150);
way({ highway: 'service', service: 'alley', name: 'Back Lane' }, ['sv0', 'sv1', 'sv2']);
way({ highway: 'service', service: 'driveway' }, ['sv3', 'sv4']);
way({ highway: 'service' }, ['sv5', 'sv6']);
way({ highway: 'residential', access: 'private', name: 'Private Way' }, ['sv7', 'sv8']);
way({ highway: 'service', access: 'no' }, ['sv5', 'sv0']);
way({ highway: 'service', service: 'parking_aisle' }, ['sv2', 'sv6']);
way({ highway: 'residential', access: 'permissive', name: 'Permissive Lane' }, ['sv8', 'sv2']);

/* ---- A motorway with a diamond interchange to Opaheke Road, trunk and ramps ---- */
node('mw0', -400, -300); node('mw1', -250, -296); node('mw2', -100, -292); node('mw3', 20, -290); node('mw4', 150, -288); node('mw5', 300, -285); node('mw6', 500, -282);
way({ highway: 'motorway', name: 'Southern Motorway', ref: 'SH 1', lanes: '3', maxspeed: '100', oneway: 'yes' }, ['mw0', 'mw1', 'mw2', 'mw3']);
way({ highway: 'motorway', name: 'Southern Motorway', ref: 'SH 1', maxspeed: '100' }, ['mw3', 'mw4', 'mw5', 'mw6']);
node('rp0', 80, -280); node('rp1', 110, -262); node('rp2', 140, -250);
node('rq0', 190, -246); node('rq1', 215, -264); node('rq2', 240, -283);
way({ highway: 'motorway_link', name: 'Opaheke Road off-ramp', lanes: '1', oneway: 'yes', maxspeed: '70' }, ['mw3', 'rp0', 'rp1', 'rp2', 'o1']);
way({ highway: 'motorway_link', oneway: 'yes' }, ['o1', 'rq0', 'rq1', 'rq2', 'mw5']);
way({ highway: 'trunk_link', name: 'Wood Street slip', oneway: 'yes', lanes: '2' }, ['mw1', 'w1']);
way({ highway: 'primary_link', name: 'Great South Road slip', oneway: 'yes' }, ['gsr8', 'sv1']);
way({ highway: 'secondary_link', oneway: 'yes', lanes: '1' }, ['sv1', 'gsr7']);
way({ highway: 'tertiary_link', oneway: 'yes', maxspeed: '30' }, ['w3', 'w4']);

/* ---- Trunk road to the east, with NZ maxspeed values, a living street, unclassified, and a way off the edge of the area ---- */
node('sh0', 500, 40); node('sh1', 560, 80); node('sh2', 620, 130); node('sh3', 700, 190);
way({ highway: 'trunk', name: 'State Highway 22', ref: 'SH 22', maxspeed: 'NZ:rural', lanes: '2' }, ['gsr14', 'sh0', 'sh1']);
way({ highway: 'trunk', name: 'State Highway 22', maxspeed: 'NZ:urban' }, ['sh1', 'sh2']);
way({ highway: 'trunk', name: 'State Highway 22' }, ['sh2', 'sh3', 'sh4'], { missing: ['sh4'] });
node('lv0', 440, 180); node('lv1', 470, 205);
way({ highway: 'living_street', name: 'Shared Lane', maxspeed: '10' }, ['kirk2', 'lv0', 'lv1']);
node('un0', -380, -150); node('un1', -330, -130); node('un2', -280, -120);
way({ highway: 'unclassified', name: 'Rural Road', maxspeed: '80 km/h', lanes: '7' }, ['un0', 'un1', 'un2']);
way({ highway: 'unclassified', name: 'Rural Road', lanes: '3', 'lanes:forward': '2', 'lanes:backward': '1' }, ['un2', 'w2']);
way({ highway: 'residential', name: 'Counted Street', lanes: '4', 'lanes:backward': '3' }, ['un1', 'w1']);

/* ---- Ways that are not roads, or are not used ---- */
node('fp0', -90, -80); node('fp1', -90, 60);
way({ highway: 'footway', name: 'Wood Street path' }, ['fp0', 'fp1']);
way({ highway: 'cycleway', name: 'Papakura Cycleway' }, ['fp0', 'w3']);
way({ highway: 'path' }, ['fp1', 'w5']);
way({ highway: 'track', surface: 'gravel' }, ['sv4', 'sv3']);
way({ highway: 'pedestrian', area: 'yes', name: 'Town Square' }, ['sv0', 'sv1', 'sv2', 'sv0']);
way({ highway: 'residential', area: 'yes', name: 'Parking Area' }, ['sv5', 'sv6', 'sv8', 'sv5']);
way({ highway: 'construction', construction: 'primary' }, ['gsr13', 'sh0']);
way({ name: 'Untagged highway' }, ['sv5', 'sv6']);
way({ highway: 'residential' }, ['sv6']);

/* ---- Build the Overpass answers ---- */
// Overpass lists nodes in quadtile order; a Morton code of latitude and longitude does the same for a small area
const morton = n => {
  const x = Math.floor((n.lon + 180) / 360 * 65536), y = Math.floor((n.lat + 90) / 180 * 65536);
  let m = 0; for (let b = 0; b < 16; b++) m |= ((x >> b & 1) << (2 * b)) | ((y >> b & 1) << (2 * b + 1));
  return m >>> 0;
};
const answer = (wantWays, wantNode = () => true) => {
  const used = new Set(wantWays.flatMap(({ w }) => w.nodes));
  const ns = [...nodes.values()].filter(n => used.has(n.id) && wantNode(n)).sort((a, b) => morton(a) - morton(b) || a.id - b.id);
  return { version: 0.6, generator: 'Overpass API 0.7.62.1 e8d1b4a5', osm3s: { timestamp_osm_base: '2026-10-20T21:42:03Z', copyright: 'The data included in this document is from www.openstreetmap.org. The data is made available under ODbL.' },
    elements: [...wantWays.map(({ w }) => w), ...ns] };
};
writeFileSync(join(out, 'overpass-papakura.json'), JSON.stringify(answer(ways), null, 1));
// The eastern half: ways with any node east of x = 100 m, as a second view that overlaps the first
const xOf = n => (n.lon - O[1]) * KM;
const idToNode = new Map([...nodes.values()].map(n => [n.id, n]));
const east = ways.filter(({ w }) => w.nodes.some(id => idToNode.get(id) && xOf(idToNode.get(id)) > 100));
writeFileSync(join(out, 'overpass-papakura-east.json'), JSON.stringify(answer(east), null, 1));
console.log('ways', ways.length, 'nodes', nodes.size, 'east ways', east.length);

/* ---- A few odd elements: missing and unusual tags and values ---- */
const bare = { version: 0.6, generator: 'Overpass API 0.7.62.1', elements: [
  { type: 'way', id: 1, nodes: [101, 102, 103], tags: { highway: 'residential', lanes: '2;3', maxspeed: '50 mph', oneway: 'true', 'cycleway:both': 'separate', 'bus:lanes': '|designated|' } },
  { type: 'way', id: 2, nodes: [103, 104], tags: { highway: 'tertiary', lanes: 'two', maxspeed: 'signals', oneway: '1', 'lanes:psv': 'bus' } },
  { type: 'way', id: 3, nodes: [104, 105], tags: { highway: 'motorway_link', oneway: 'no', lanes: '1' } },
  { type: 'way', id: 4, nodes: [105, 106], tags: { highway: 'secondary', junction: 'circular', maxspeed: '40' } },
  { type: 'way', id: 5, nodes: [106, 101], tags: { highway: 'primary', oneway: '-1', lanes: '3', 'lanes:forward': '2', 'cycleway:right': 'track' } },
  { type: 'way', id: 6, nodes: [101], tags: { highway: 'residential' } },
  { type: 'way', id: 7, tags: { highway: 'residential' } },
  { type: 'way', id: 8, nodes: [101, 103], tags: { highway: 'residential', area: 'no', access: 'private' } },
  { type: 'way', id: 9, nodes: [102, 105], tags: { highway: 'service', service: 'alley', access: 'private' } },
  { type: 'way', id: 10, nodes: [102, 106] },
  { type: 'node', id: 101, lat: -37.0701, lon: 174.9401 },
  { type: 'node', id: 102, lat: -37.0702, lon: 174.9411, tags: { highway: 'stop' } },
  { type: 'node', id: 103, lat: -37.0702, lon: 174.9421, tags: { highway: 'traffic_signals' } },
  { type: 'node', id: 104, lat: -37.0712, lon: 174.9421 },
  { type: 'node', id: 105, lat: -37.0712, lon: 174.9411, tags: { highway: 'stop' } },
  { type: 'node', id: 106, lat: -37.0712, lon: 174.9401, tags: { highway: 'traffic_signals' } },
  { type: 'relation', id: 5, members: [] },
  { type: 'node', id: 107 },
] };
writeFileSync(join(out, 'overpass-bare.json'), JSON.stringify(bare, null, 1));
