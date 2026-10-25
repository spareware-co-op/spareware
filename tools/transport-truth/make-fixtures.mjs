// Builds the truth-test projects with the prototype's own functions (newProject, Net, Sites), so they are exactly what
// the app would save. Deterministic: js-core seeds Math.random and fixes the clock.
import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadCore } from './js-core.mjs';

const out = join(dirname(fileURLToPath(import.meta.url)), 'fixtures');
mkdirSync(out, { recursive: true });
const clone = o => JSON.parse(JSON.stringify(o));

function gridProject(c) {
  const { newProject, Net, U } = c;
  const P = newProject('Truth test: grid');
  Object.assign(P.settings, { baseYear: 2026, year: 2026 });
  const sc = P.scenarios[0];
  const lat0 = -36.9, lng0 = 174.76, dLat = 0.0025, dLng = 0.003;
  const N = {};
  for (let r = 0; r < 4; r++) for (let k = 0; k < 5; k++) N[r + ',' + k] = Net.addNode(sc, [lat0 - r * dLat, lng0 + k * dLng]);
  const cls = (r, k, horiz) => horiz ? (r === 1 ? 'arterial' : r === 2 ? 'collector' : 'local') : (k === 2 ? 'arterial' : 'local');
  for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) if (!(r === 2 && k === 3)) Net.addLink(sc, N[r + ',' + k], N[r + ',' + (k + 1)], [], { cls: cls(r, k, true), name: 'Row ' + r + ' ' + k });
  for (let r = 0; r < 3; r++) for (let k = 0; k < 5; k++) Net.addLink(sc, N[r + ',' + k], N[(r + 1) + ',' + k], [], { cls: cls(r, k, false), name: 'Col ' + k + ' ' + r });
  sc.nodes[N['1,2']].control = 'signals';
  sc.nodes[N['2,2']].control = 'roundabout';
  sc.nodes[N['1,4']].control = 'priority';
  sc.nodes[N['3,2']].control = 'stop';
  const zone = (id, name, ll, lu, extra = {}) => { sc.zones[id] = { id, kind: 'zone', name, lat: U.r7(ll[0]), lng: U.r7(ll[1]), lu, connectors: null, ...extra }; };
  zone('zA', 'Houses', [lat0 - 2.5 * dLat, lng0 + 0.5 * dLng], { residential: 400 });
  zone('zB', 'Shops', [lat0 - 0.5 * dLat, lng0 + 3.5 * dLng], { retail: 6000, supermarket: 2500 });
  zone('zC', 'Offices and flats', [lat0 - 1.5 * dLat, lng0 + 1.5 * dLng], { office: 8000, apartment: 120 });
  const gw = (id, name, nid, ll, f) => { sc.zones[id] = { id, kind: 'gateway', name, lat: U.r7(ll[0]), lng: U.r7(ll[1]), node: nid, gw: f }; };
  gw('gW', 'West gateway', N['1,0'], [lat0 - dLat, lng0 - 0.6 * dLng], { AMin: 600, AMout: 450, PMin: 500, PMout: 650 });
  gw('gE', 'East gateway', N['1,4'], [lat0 - dLat, lng0 + 4.6 * dLng], { AMin: 380, AMout: 520, PMin: 540, PMout: 360 });
  gw('gN', 'North gateway', N['0,2'], [lat0 + 0.6 * dLat, lng0 + 2 * dLng], { AMin: 300, AMout: 200, PMin: 220, PMout: 310 });
  sc.routes.r1 = { id: 'r1', name: 'Bus route 1', color: '#7048e8', freq: 8, wp: [N['1,0'], N['1,2'], N['1,4']] };
  // Option: the missing link opens as proposed works, in the design year, against a do-minimum in the same year
  const dm = clone(sc); dm.id = 's-dm'; dm.name = 'Do-minimum 2036'; dm.year = 2036;
  const opt = clone(sc); opt.id = 's-opt'; opt.name = 'Option 2036'; opt.year = 2036;
  Net.addLink(opt, N['2,3'], N['2,4'], [], { cls: 'collector', status: 'proposed', name: 'New link' });
  P.scenarios.push(dm, opt);
  return P;
}

function corridorProject(c) {
  const { newProject, Net, U, Sites } = c;
  const P = newProject('Truth test: corridor');
  Object.assign(P.settings, { baseYear: 2026, year: 2031, growthMode: 'compound', growth: 2 });
  P.settings.junction = { ...P.settings.junction, capMethod: 'movement' };
  const sc = P.scenarios[0];
  const lat0 = -37.1, lng0 = 174.95, dLng = 0.0028, dLat = 0.0022;
  const A = [];
  for (let k = 0; k < 8; k++) A.push(Net.addNode(sc, [lat0, lng0 + k * dLng]));
  // a staggered junction: A[4] and A4x 25 m apart, both signals, so the site rules treat them as one junction
  const A4x = Net.addNode(sc, [lat0, lng0 + 4 * dLng + 0.00028]);
  for (let k = 0; k < 7; k++) {
    if (k === 4) { Net.addLink(sc, A[4], A4x, [], { cls: 'arterial', name: 'Main Road' }); Net.addLink(sc, A4x, A[5], [], { cls: 'arterial', name: 'Main Road' }); }
    else Net.addLink(sc, A[k], A[k + 1], [], { cls: 'arterial', name: 'Main Road' });
  }
  const side = (at, k, dir, cls) => { const n = Net.addNode(sc, [lat0 + dir * dLat, lng0 + k * dLng]); Net.addLink(sc, at, n, [], { cls, name: 'Side ' + k + (dir > 0 ? 'N' : 'S') }); return n; };
  side(A[2], 2, 1, 'collector');
  const s2s = side(A[2], 2, -1, 'local');
  side(A[4], 4, 1, 'collector');
  const s4s = side(A4x, 4.1, -1, 'collector');
  const s6n = side(A[6], 6, 1, 'local');
  sc.nodes[A[2]].control = 'priority';
  sc.nodes[A[4]].control = 'signals';
  sc.nodes[A4x].control = 'signals';
  sc.nodes[A[6]].control = 'roundabout';
  // a toll and a variable speed on the main road
  const tolled = Object.values(sc.links).find(L => L.a === A[0] && L.b === A[1]);
  tolled.toll = { AM: 30, PM: 20 };
  const slow = Object.values(sc.links).find(L => L.a === A[5] && L.b === A[6]);
  slow.varSpeed = { v: 40, AM: true, PM: false };
  const zone = (id, name, ll, lu, extra = {}) => { sc.zones[id] = { id, kind: 'zone', name, lat: U.r7(ll[0]), lng: U.r7(ll[1]), lu, connectors: null, ...extra }; };
  zone('zDev', 'Development', [lat0 + 1.6 * dLat, lng0 + 3 * dLng], { residential: 600, childcare: 60 }, { role: 'development', year: 2028 });
  zone('zCom', 'Committed', [lat0 - 1.4 * dLat, lng0 + 5 * dLng], { apartment: 220, retail: 1500 }, { role: 'committed', year: 2027, buildout: 70 });
  zone('zEx', 'Existing', [lat0 + 0.8 * dLat, lng0 + 6.5 * dLng], { residential: 300, school: 400 }, { existing: true, src: 'census' });
  const gw = (id, name, nid, ll, f) => { sc.zones[id] = { id, kind: 'gateway', name, lat: U.r7(ll[0]), lng: U.r7(ll[1]), node: nid, gw: f }; };
  gw('gW', 'Main Road west', A[0], [lat0, lng0 - 0.6 * dLng], { AMin: 900, AMout: 700, PMin: 750, PMout: 950 });
  gw('gE', 'Main Road east', A[7], [lat0, lng0 + 7.6 * dLng], { AMin: 650, AMout: 820, PMin: 860, PMout: 640 });
  gw('g2s', 'Side 2 south', s2s, [lat0 - 1.6 * dLat, lng0 + 2 * dLng], { AMin: 150, AMout: 120, PMin: 130, PMout: 160 });
  gw('g4s', 'Side 4 south', s4s, [lat0 - 1.6 * dLat, lng0 + 4 * dLng], { AMin: 260, AMout: 210, PMin: 230, PMout: 270 });
  gw('g6n', 'Side 6 north', s6n, [lat0 + 1.6 * dLat, lng0 + 6 * dLng], { AMin: 90, AMout: 110, PMin: 100, PMout: 95 });
  sc.routes.r1 = { id: 'r1', name: 'Bus route 1', color: '#0c8599', freqs: { AM: 10, PM: 8 }, wp: [A[0], A[4], A[7]] };
  Sites.detect(sc, { P });
  const opt = clone(sc); opt.id = 's-opt'; opt.name = 'Option: signals at Side 2'; opt.nodes[A[2]].control = 'signals';
  P.scenarios.push(opt);
  return P;
}

const builders = { grid: gridProject, corridor: corridorProject };
for (const [name, build] of Object.entries(builders)) {
  const c = loadCore({ seed: name.length * 7919 });
  const P = build(c);
  c.Store.P = P;
  writeFileSync(join(out, name + '.json'), JSON.stringify(P, null, 1));
  const s0 = P.scenarios[0];
  console.log(name, 'scenarios', P.scenarios.length, 'nodes', Object.keys(s0.nodes).length, 'links', Object.keys(s0.links).length, 'zones', Object.keys(s0.zones).length, 'sites', Object.keys(s0.sites || {}).length);
}
