// Lays out the OpenStreetMap land-use test data for the Rust port's landuse tests (gen-landuse.mjs): a made-up suburb in Overpass
// API's JSON answer format for the prototype's two queries (`out tags geom`: building outlines, land-use areas, schools,
// clinics, hotels and shops, with the geometry of every way and relation member). Nothing is downloaded; the layout is invented
// but built the way NZ OSM data looks: a LINZ-style import of mostly untagged `building=yes` outlines, a residential land-use
// area with streets of houses, garages and sheds, a commercial strip, a shopping centre with a supermarket, an industrial area,
// a school, a college, a kindergarten, a hospital, clinics, a motel and a hotel, shop sites with no buildings, farms and a
// roadside settlement outside any mapped area, churches, multipolygon buildings with courtyards, and elements outside the study
// area or too small to use. Deterministic.
//
// Writes fixtures/landuse-papatoetoe.json (everything as one answer), fixtures/landuse-papatoetoe-buildings-<k>.json and
// landuse-papatoetoe-areas-<k>.json (the same data split the way the download tiles it, with features that straddle tiles repeated),
// and fixtures/landuse-odd.json (elements that are missing keys, have no geometry or odd tag values).
import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const out = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
mkdirSync(out, { recursive: true });

const O = [-36.97, 174.86];
const KM = 111320 * Math.cos(O[0] * Math.PI / 180), KY = 110574;
const at = (dx, dy) => ({ lat: Math.round((O[0] + dy / KY) * 1e7) / 1e7, lon: Math.round((O[1] + dx / KM) * 1e7) / 1e7 });

let seed = 20261021;
const rnd = () => { seed = (seed + 0x6D2B79F5) >>> 0; let t = seed; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
const uni = (a, b) => a + (b - a) * rnd();
const pick = a => a[Math.floor(rnd() * a.length)];

const elements = [];
let wayId = 130000000, nodeId = 5400000000, relId = 13500000;
const bounds = pts => ({ minlat: Math.min(...pts.map(p => p.lat)), minlon: Math.min(...pts.map(p => p.lon)), maxlat: Math.max(...pts.map(p => p.lat)), maxlon: Math.max(...pts.map(p => p.lon)) });
const way = (tags, ptsXY, closed = true) => {
  const pts = ptsXY.map(([x, y]) => at(x, y));
  if (closed) pts.push(pts[0]);
  wayId += 1 + Math.floor(rnd() * 900);
  const e = { type: 'way', id: wayId, bounds: bounds(pts), geometry: pts, tags };
  elements.push(e);
  return e;
};
const point = (tags, x, y) => { nodeId += 1 + Math.floor(rnd() * 40); const e = { type: 'node', id: nodeId, ...at(x, y), tags }; elements.push(e); return e; };
// A rectangle centred (cx, cy), w by h metres, turned `rot` degrees
const rect = (cx, cy, w, h, rot = 0) => {
  const a = rot * Math.PI / 180, c = Math.cos(a), s = Math.sin(a);
  return [[-w / 2, -h / 2], [w / 2, -h / 2], [w / 2, h / 2], [-w / 2, h / 2]].map(([x, y]) => [cx + x * c - y * s, cy + x * s + y * c]);
};
const box = (x0, y0, x1, y1) => [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
const building = (tags, cx, cy, w, h, rot = 0) => way({ building: 'yes', ...tags }, rect(cx, cy, w, h, rot));

/* ---- Land-use and site areas ---- */
way({ landuse: 'residential' }, box(-650, -380, 200, 420));
way({ landuse: 'commercial' }, box(-200, -20, 200, 70));
way({ landuse: 'retail', name: 'Papatoetoe Shopping Centre' }, box(260, 40, 460, 170));
way({ landuse: 'industrial' }, box(300, -460, 650, -160));
way({ landuse: 'education' }, box(-640, 300, -540, 400));
way({ amenity: 'school', name: 'Papatoetoe Intermediate', 'MOE:years': '7-8', 'addr:street': 'Cambria Road' }, box(-420, 200, -320, 300));
way({ amenity: 'college', name: 'Manukau Technical Institute', capacity: '3,200' }, box(-250, 330, -100, 440));
way({ amenity: 'clinic', name: 'Station Road Clinic' }, box(-120, -10, -90, 15));
way({ amenity: 'hospital', name: 'Middlemore Annex' }, box(-500, -300, -380, -200));
way({ tourism: 'motel', name: 'Great South Motel' }, box(100, -330, 170, -280));
way({ tourism: 'hotel', name: 'Airport Lodge', rooms: '60' }, box(210, -100, 250, -60));
way({ shop: 'supermarket', name: 'Fresh Choice' }, box(480, 60, 560, 130));
way({ shop: 'car', name: 'Station Motors' }, box(480, 150, 560, 200));
way({ shop: 'garden_centre', name: 'Green Thumb' }, box(570, 60, 640, 110));
way({ shop: 'bakery', name: 'Corner Bakery Site' }, box(-30, 20, -10, 40));
way({ landuse: 'residential', name: 'Duplicate residential' }, box(-300, 100, -100, 220));
way({ landuse: 'cemetery' }, box(100, 300, 180, 380));

/* ---- Houses, garages and sheds on eight streets inside the residential area ---- */
const STREETS = [['Cambria Road', -330], ['Rata Street', -250], ['Totara Avenue', -170], ['Hunters Park Drive', -90], ['Kowhai Place', 110], ['Matai Road', 190], ['Rimu Street', 270], ['Kauri Crescent', 350]];
let houses = 0;
for (const [street, y] of STREETS) for (const side of [-1, 1]) for (let x = -620; x < 180; x += 52) {
  if (rnd() < 0.12) continue;     // an empty lot
  const cy = y + side * 24 + uni(-1.2, 1.2), cx = x + uni(-2, 2);
  const big = rnd() < 0.12;
  const tags = rnd() < 0.55 ? { 'addr:street': street, 'addr:housenumber': String(2 + Math.floor((x + 620) / 26)) } : {};
  building({ ...(rnd() < 0.15 ? { building: 'house' } : {}), ...tags }, cx, cy, big ? 14 : 11, big ? 16 : 13, uni(-3, 3));
  houses++;
  if (rnd() < 0.3) way({ building: pick(['garage', 'shed', 'carport', 'garages']) }, rect(cx + 9, cy + side * 4, 6, 5));
  if (rnd() < 0.15) building({}, cx - 9, cy - side * 5, 5, 4.6);                 // under 30 m2
  if (rnd() < 0.18) building({}, cx + 10, cy - side * 6, 7.5, 6);                // 45 m2 beside a house: an outbuilding
}
// Larger buildings in the residential area
building({ building: 'apartments', 'building:levels': '4', 'addr:street': 'Matai Road' }, -150, 205, 40, 18);
building({ building: 'apartments', name: 'Rimu Court' }, -90, 280, 30, 14);
building({ building: 'terrace', 'addr:street': 'Rimu Street' }, -20, 300, 36, 9);
building({ building: 'residential', 'building:levels': '3', 'addr:street': 'Kauri Crescent' }, 40, 380, 32, 14);
building({ building: 'residential' }, 110, 380, 32, 25);                          // 800 m2, no levels
building({ 'building:levels': '5', height: '17' }, -400, 130, 36, 20);                // a tagged block of flats
building({}, -350, -350, 30, 20);                                                 // 600 m2 untagged: large residential
building({ building: 'dormitory' }, -560, 180, 20, 12);
building({ building: 'bungalow' }, 30, -200, 12, 10);
building({ building: 'detached', 'addr:street': 'Totara Avenue' }, 70, -170, 12, 10);
// Commercial strip: offices and shops (a bakery point is mapped beside the second building)
building({ building: 'commercial' }, -150, 20, 30, 18);
building({ building: 'commercial' }, -110, 45, 24, 14);
building({ building: 'office', 'building:levels': '3', 'addr:street': 'Great South Road' }, 150, 15, 40, 20);
building({ office: 'lawyer' }, 100, 50, 18, 12);
building({ building: 'retail', 'addr:street': 'Great South Road' }, -30, 30, 20, 15);
building({ building: 'kiosk' }, 5, 60, 6, 6);
building({ building: 'commercial', 'addr:street': 'Great South Road' }, 180, 60, 14, 10);
point({ shop: 'bakery', name: 'Corner Bakery' }, -28, 31);
point({ shop: 'hairdresser' }, -112, 46);
// Retail area: the supermarket and a few shops
building({ building: 'retail', shop: 'supermarket', name: 'Fresh Choice Papatoetoe', 'addr:street': 'Station Road' }, 360, 100, 90, 50);
building({ building: 'retail' }, 290, 150, 24, 14);
building({ building: 'retail', 'building:levels': '2' }, 330, 60, 30, 18);
building({ shop: 'chemist' }, 420, 70, 18, 12);
point({ shop: 'clothes', name: 'Style' }, 291, 150);
point({ shop: 'supermarket', name: 'Fresh Choice' }, 365, 102);
point({ amenity: 'fast_food', name: 'Burger Burger' }, 330, 60);
point({ amenity: 'fast_food', name: 'Burger Burger Drive-thru' }, 440, 40);
building({ amenity: 'fast_food', name: 'Pizza Place' }, 300, 110, 12, 10);
// Industrial area: warehouses, a workshop, trade suppliers
building({ building: 'warehouse', 'addr:street': 'Station Road' }, 420, -250, 80, 40);
building({ building: 'industrial' }, 500, -300, 60, 40);
building({}, 360, -340, 50, 30);
building({ building: 'industrial', 'building:levels': '2' }, 560, -220, 40, 40);
building({ building: 'factory', 'addr:street': 'Station Road' }, 600, -400, 50, 40);
building({ shop: 'car_repair', name: 'Quick Tyres' }, 330, -200, 20, 15);
building({ shop: 'hardware', name: 'Trade Hardware' }, 400, -190, 60, 30);
building({ craft: 'carpenter' }, 450, -430, 25, 18);
point({ shop: 'tyres' }, 331, -201);
point({ shop: 'car_repair' }, 560, -419);
building({ building: 'roof' }, 520, -180, 20, 20);
building({ building: 'yes', man_made: 'storage_tank' }, 640, -250, 10, 10);
building({ building: 'yes', aeroway: 'hangar' }, 600, -160, 30, 20);
// Schools: the intermediate (a point at its gate too), a college, a kindergarten, an orphan education building
building({ building: 'school' }, -380, 250, 40, 20);
building({ building: 'yes' }, -350, 220, 20, 14);
building({ building: 'school', name: 'Block C' }, -400, 280, 24, 12);
point({ amenity: 'school', name: 'Papatoetoe Intermediate' }, -370, 198);
building({ building: 'university' }, -200, 380, 60, 30);
building({ building: 'college', name: 'Manukau Technical Institute' }, -150, 420, 40, 20);
building({ building: 'yes' }, -170, 350, 30, 20);
building({ building: 'kindergarten', amenity: 'kindergarten', name: 'Little Kauri Kindergarten', capacity: '60' }, -300, 120, 22, 14);
point({ amenity: 'kindergarten', name: 'Rata Early Learning' }, -480, -120);
point({ amenity: 'childcare', name: 'Totara Childcare', capacity: '45' }, -200, -140);
building({ building: 'school', name: 'Te Kura Kaupapa Maori o Papatoetoe' }, -600, 350, 40, 18);
building({ building: 'school', name: 'Papatoetoe High School' }, -590, 320, 60, 30);
building({ building: 'school' }, 600, 300, 40, 20);                         // an orphan with no campus
// Medical and hospital
building({ amenity: 'doctors', name: 'Papatoetoe Medical', 'addr:street': 'Station Road' }, -240, 60, 24, 16);
building({ building: 'yes' }, -105, 2, 18, 12);                              // inside the clinic site
point({ amenity: 'dentist' }, -500, 10);                                      // a clinic point with no building
point({ amenity: 'doctors' }, -250, 62);                                      // beside the clinic building
building({ building: 'hospital' }, -440, -250, 70, 40);
building({ building: 'yes' }, -400, -220, 30, 20);                           // in the hospital site
point({ amenity: 'hospital', name: 'Middlemore Annex' }, -420, -280);
building({ amenity: 'hospital', 'building:levels': '3' }, 20, -350, 40, 30);
// Hotels and motels
building({ tourism: 'hotel', rooms: '45', name: 'Station Hotel' }, 20, 20, 30, 20);
building({ building: 'hotel', 'building:levels': '4' }, 400, 330, 40, 20);
for (const [x, y] of [[120, -310], [140, -300], [155, -318]]) building({}, x, y, 16, 10);
building({}, 230, -80, 22, 14);
point({ tourism: 'hotel', rooms: '12', name: 'Backpackers' }, 100, 450);
point({ tourism: 'motel' }, 90, -440);
// Shop sites without buildings, loose shops, a rural shop
building({}, 600, 85, 30, 20);                                                // in the garden centre
point({ shop: 'bakery' }, -600, -450);
point({ shop: 'supermarket' }, 690, 500);
point({ amenity: 'fast_food' }, 200, 500);
point({ shop: 'farm', name: 'Roadside Stall' }, 560, 330);
point({ shop: 'bicycle' }, 230, -300);
// Community and other uses, and things that are not buildings anyone uses
building({ building: 'church', name: 'St Johns' }, -250, -300, 24, 16);
building({ building: 'yes', amenity: 'place_of_worship' }, -80, -300, 20, 14);
building({ building: 'yes', amenity: 'community_centre' }, 100, 150, 24, 16);
building({ building: 'yes', amenity: 'library' }, 160, 120, 24, 16);
building({ building: 'yes', amenity: 'fuel' }, 190, 40, 12, 8);
building({ building: 'construction' }, -60, 150, 20, 14);
building({ building: 'greenhouse' }, -620, -300, 20, 12);
way({ building: 'no', name: 'Not a building' }, box(-20, -150, 0, -130));
way({ landuse: 'residential', building: 'no' }, box(-20, -190, 0, -170));
/* ---- Rural: farms with sheds, a roadside settlement, isolated houses, big sheds ---- */
building({ 'addr:street': 'Farm Road' }, 540, 300, 15, 10);              // a farmhouse
for (const [dx, dy, w, h] of [[22, 6, 12, 8], [-24, -10, 11, 9], [30, -14, 10, 12], [60, 20, 14, 10]]) building({}, 540 + dx, 300 + dy, w, h);
building({}, 640, 330, 30, 18);                                                // a 540 m2 farm shed
building({}, 560, 380, 6, 7);                                                  // a 42 m2 shed
building({ building: 'barn' }, 600, 360, 25, 14);
building({ building: 'farm_auxiliary' }, 500, 400, 12, 10);
building({}, 420, 380, 14, 11);
for (const [dx, dy] of [[12, 8], [-20, 14], [-30, -8]]) building({}, 420 + dx, 380 + dy, 12 + (dx % 5), 9);
// A roadside settlement: eight houses in a line within 100 m, no residential area
for (let i = 0; i < 8; i++) building({ 'addr:street': 'Mill Road' }, 310 + i * 13, -90 + (i % 2) * 4, 11, 9);
building({}, 330, -60, 9, 8);
// Isolated houses
building({}, 660, 150, 13, 11); building({}, 480, 470, 12, 10); building({ building: 'residential' }, 250, 520, 13, 10);
// A big building outside any area, and an untagged yard building far from shops
building({}, 200, -500, 40, 25);
building({}, 100, -500, 10, 9);
/* ---- Multipolygon buildings ---- */
const relation = (tags, members) => {
  relId += 1 + Math.floor(rnd() * 90);
  const ms = members.map(([role, pts, closed = true]) => { const g = pts.map(([x, y]) => at(x, y)); if (closed) g.push(g[0]); wayId += 5; return { type: 'way', ref: wayId, role, geometry: g }; });
  const all = ms.flatMap(m => m.geometry);
  elements.push({ type: 'relation', id: relId, bounds: bounds(all), members: ms, tags: { type: 'multipolygon', ...tags } });
};
relation({ building: 'apartments', 'building:levels': '5', 'addr:street': 'Matai Road' }, [['outer', box(-540, 200, -490, 250)], ['inner', box(-525, 215, -505, 235)]]);
// An outer ring in two open pieces, one of them listed backwards
{
  const left = [[-450, 120], [-430, 120], [-430, 150], [-450, 150]], right = [[-450, 150], [-470, 150], [-470, 120], [-450, 120]];
  const a = left.map(([x, y]) => at(x, y)), b = right.map(([x, y]) => at(x, y)).reverse();
  relId += 7; wayId += 3;
  elements.push({ type: 'relation', id: relId, bounds: bounds([...a, ...b]), members: [{ type: 'way', ref: wayId, role: 'outer', geometry: a }, { type: 'way', ref: wayId + 1, role: 'outer', geometry: b }], tags: { type: 'multipolygon', building: 'yes' } });
}
// A relation with only an inner way, and one with an unfinished ring
relId += 3; elements.push({ type: 'relation', id: relId, members: [{ type: 'way', ref: 1, role: 'inner', geometry: [at(0, 0), at(5, 0), at(5, 5), at(0, 0)] }], tags: { type: 'multipolygon', building: 'yes' } });
relId += 3; elements.push({ type: 'relation', id: relId, members: [{ type: 'way', ref: 2, role: 'outer', geometry: [at(-100, -100), at(-80, -100), at(-80, -80)] }], tags: { type: 'multipolygon', building: 'yes' } });
relation({ landuse: 'industrial' }, [['outer', box(-200, -470, -100, -420)]]);
relation({ amenity: 'school', name: 'Relation School', 'MOE:years': '9-13' }, [['outer', box(-690, -150, -640, -100)]]);
/* ---- Outside the study area, too small, or broken ---- */
building({}, 760, 100, 12, 10); building({}, -780, -200, 12, 10); building({}, 100, 640, 12, 10);
point({ shop: 'bakery' }, 900, 0);
elements.push({ type: 'way', id: 129999999, geometry: [at(0, 0), at(10, 0)], tags: { building: 'yes' } });
elements.push({ type: 'way', id: 129999998, geometry: [at(0, 0), at(10, 0), at(10, 10)], tags: { building: 'yes' } });
elements.push({ type: 'way', id: 129999997, tags: { building: 'yes' } });
elements.push({ type: 'way', id: 129999996, geometry: [at(-50, -450), at(-40, -450)], tags: { amenity: 'clinic' } });
elements.push({ type: 'way', id: 129999995, geometry: [at(-60, -460), at(-50, -465), at(-40, -460)], tags: { shop: 'florist' } });

/* ---- Write ---- */
const meta = { version: 0.6, generator: 'Overpass API 0.7.62.1 e8d1b4a5', osm3s: { timestamp_osm_base: '2026-10-20T21:42:03Z', copyright: 'The data included in this document is from www.openstreetmap.org. The data is made available under ODbL.' } };
writeFileSync(join(out, 'landuse-papatoetoe.json'), JSON.stringify({ ...meta, elements }));
// Split the way the download tiles it: buildings in 3 km tiles, the rest in 9 km tiles; a feature that straddles a tile is in each
const isBuilding = e => !!(e.tags && e.tags.building && e.type !== 'node');
const cx = e => { const pts = e.geometry || (e.members ? e.members.flatMap(m => m.geometry) : [e]); return pts.reduce((a, p) => a + p.lon, 0) / pts.length; };
const cuts = [O[1] - 0.001, O[1] + 0.003];
const tileOf = (e, side) => { const pts = e.geometry || (e.members ? e.members.flatMap(m => m.geometry) : [e]); const lo = Math.min(...pts.map(p => p.lon)), hi = Math.max(...pts.map(p => p.lon)); return side === 0 ? lo < cuts[1] : hi >= cuts[0]; };
const b0 = elements.filter(e => isBuilding(e) && tileOf(e, 0)), b1 = elements.filter(e => isBuilding(e) && tileOf(e, 1));
const a0 = elements.filter(e => !isBuilding(e) && tileOf(e, 0)), a1 = elements.filter(e => !isBuilding(e) && tileOf(e, 1));
for (const [name, list] of [['buildings-0', b0], ['buildings-1', b1], ['areas-0', a0], ['areas-1', a1]]) writeFileSync(join(out, `landuse-papatoetoe-${name}.json`), JSON.stringify({ ...meta, elements: list }));
console.log('elements', elements.length, 'houses', houses, 'tiles', b0.length, b1.length, a0.length, a1.length);

const odd = { elements: [
  { type: 'way', id: 1, geometry: [{ lat: -36.97, lon: 174.86 }, { lat: -36.97, lon: 174.8603 }, { lat: -36.9703, lon: 174.8603 }, { lat: -36.9703, lon: 174.86 }], tags: { building: 'house', 'building:levels': 'two' } },
  { type: 'way', id: 2, geometry: [{ lat: -36.9710, lon: 174.8610 }, { lat: -36.9710, lon: 174.8614 }, { lat: -36.9713, lon: 174.8614 }, { lat: -36.9713, lon: 174.8610 }], tags: { building: 'apartments', height: '9,5', 'building:levels': '1,5' } },
  { type: 'way', id: 3, geometry: [{ lat: -36.9720, lon: 174.8620 }, { lat: -36.9720, lon: 174.8626 }, { lat: -36.9724, lon: 174.8626 }, { lat: -36.9724, lon: 174.8620 }], tags: { building: 'school', capacity: '1 450', name: 'Big School High' } },
  { type: 'way', id: 4, geometry: [{ lat: -36.9730, lon: 174.8630 }, { lat: -36.9730, lon: 174.8636 }, { lat: -36.9734, lon: 174.8636 }, { lat: -36.9734, lon: 174.8630 }], tags: { building: 'school', capacity: 'many', 'MOE:years': 'Years 1 to 6', 'isced:level': '1' } },
  { type: 'way', id: 5, geometry: [{ lat: -36.9740, lon: 174.8640 }, { lat: -36.9740, lon: 174.8646 }, { lat: -36.9744, lon: 174.8646 }, { lat: -36.9744, lon: 174.8640 }], tags: { building: 'school', grades: '9-13', name: 'Some Kura' } },
  { type: 'way', id: 6, geometry: [{ lat: -36.9750, lon: 174.8650 }, { lat: -36.9750, lon: 174.8656 }, { lat: -36.9754, lon: 174.8656 }, { lat: -36.9754, lon: 174.8650 }], tags: { building: 'school', 'isced:level': '2;3' } },
  { type: 'way', id: 7, geometry: [{ lat: -36.9760, lon: 174.8660 }, { lat: -36.9760, lon: 174.8666 }, { lat: -36.9764, lon: 174.8666 }, { lat: -36.9764, lon: 174.8660 }], tags: { building: 'university', capacity: '1,2' } },
  { type: 'way', id: 8, geometry: [{ lat: -36.9770, lon: 174.8670 }, { lat: -36.9770, lon: 174.8676 }, { lat: -36.9774, lon: 174.8676 }, { lat: -36.9774, lon: 174.8670 }], tags: { amenity: 'kindergarten', building: 'yes', capacity: '12.5' } },
  { type: 'way', id: 9, geometry: [{ lat: -36.9780, lon: 174.8680 }, { lat: -36.9780, lon: 174.8686 }, { lat: -36.9784, lon: 174.8686 }, { lat: -36.9784, lon: 174.8680 }], tags: { tourism: 'hotel', rooms: '1,200' } },
  { type: 'node', id: 10, lat: -36.9790, lon: 174.8690, tags: { shop: 'supermarket' } },
  { type: 'node', id: 11, lat: '-36.9791', lon: '174.8691', tags: { amenity: 'fast_food' } },
  { type: 'node', id: 12, lat: -36.9792, tags: { amenity: 'doctors' } },
  { type: 'node', id: 13, lat: -36.9793, lon: 174.8693 },
  { type: 'way', id: 14, geometry: [{ lat: -36.9800, lon: 174.8700 }, { lat: null, lon: 174.8704 }, { lat: -36.9804, lon: 174.8704 }, { lat: -36.9804, lon: 174.8700 }, { lat: 'x', lon: 174.8700 }], tags: { building: 'yes' } },
  { type: 'way', id: 15, geometry: [{ lat: -36.9810, lon: 174.8710 }, { lat: -36.9810, lon: 174.8714 }, { lat: -36.9813, lon: 174.8714 }], tags: { building: 'yes' } },
  { type: 'way', id: 1, geometry: [{ lat: -36.9710, lon: 174.8610 }, { lat: -36.9710, lon: 174.8612 }, { lat: -36.9712, lon: 174.8612 }], tags: { building: 'yes' } },
  { type: 'way', geometry: [], tags: { building: 'yes' } },
  { id: 17, tags: { building: 'yes' } },
  null,
  { type: 'node', id: 18, lat: -36.9820, lon: 174.8720, tags: { shop: 'bakery', 'addr:street': 'Odd Street' } },
] };
writeFileSync(join(out, 'landuse-odd.json'), JSON.stringify(odd));
