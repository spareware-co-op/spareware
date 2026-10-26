// The operation lists the editing truth tests run (see edit-ops.mjs): a network built node by node and edited, junction sites
// detected and merged, assessment scenarios made and checked, and undo. Each list is plain data, so the Rust test replays the
// same one. Used by gen-edit.mjs.

const KM = 111320, KY = 110574;
// A point dx metres east and dy metres north of a start point
const at = (p, dx, dy) => [p[0] + dy / KY, p[1] + dx / (KM * Math.cos(p[0] * Math.PI / 180))];
const r7 = x => Math.round(x * 1e7) / 1e7;

function builder() {
  const ops = [];
  const op = (name, a = {}) => { ops.push({ op: name, ...a }); };
  return { ops, op };
}

/* ---------- a network built node by node, queried and edited ---------- */
export function buildScript() {
  const { ops, op } = builder();
  const lat0 = -36.9, lng0 = 174.76, dLat = 0.0025, dLng = 0.003;
  const ll = (r, k, dr = 0, dk = 0) => [lat0 - (r + dr) * dLat, lng0 + (k + dk) * dLng];
  op('newProject', { name: 'Edit truth: build', as: 'proj' });
  op('settings', { set: { baseYear: 2026, year: 2031 } });
  // A 4 x 5 grid of junctions; one is made with its control, one carries an OpenStreetMap id
  for (let r = 0; r < 4; r++) for (let k = 0; k < 5; k++) {
    const props = r === 1 && k === 2 ? { control: 'signals' } : r === 0 && k === 4 ? { osm: 1234567 } : undefined;
    op('addNode', { ll: ll(r, k), ...(props ? { props } : {}), as: `n${r}${k}` });
  }
  // Rows: arterial, collector, local, local; one missing link; a bend; extra lanes
  for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) {
    if (r === 2 && k === 3) continue;
    const props = { cls: r === 0 ? 'arterial' : r === 1 ? 'collector' : 'local', name: `Row ${r}` };
    if (r === 1 && k === 1) Object.assign(props, { lanesAB: 3, lanesBA: 2, speed: 60 });
    if (r === 3 && k === 0) Object.assign(props, { status: 'proposed', speed: 30 });
    const bend = r === 2 && k === 1 ? [ll(r, k, 0.35, 0.4), ll(r, k, -0.3, 0.7)] : [];
    op('addLink', { a: `$n${r}${k}`, b: `$n${r}${k + 1}`, pts: bend, props, as: `h${r}${k}` });
  }
  // Columns: the middle one an arterial, the last one-way, a closed link, a bus lane, cycleways
  for (let r = 0; r < 3; r++) for (let k = 0; k < 5; k++) {
    const props = { cls: k === 2 ? 'arterial' : 'local', name: `Col ${k}` };
    if (k === 4) Object.assign(props, { lanesAB: 1, lanesBA: 0 });
    if (r === 1 && k === 3) props.status = 'closed';
    if (r === 2 && k === 1) Object.assign(props, { busLane: true, lanesAB: 2, lanesBA: 2 });
    if (r === 0 && k === 1) props.cycle = 'painted';
    if (r === 1 && k === 4) props.cycle = 'separated';
    op('addLink', { a: `$n${r}${k}`, b: `$n${r + 1}${k}`, pts: [], props, as: `v${r}${k}` });
  }
  // A motorway with a ramp to the north, and a link with every class default
  op('addNode', { ll: ll(-1, 0), as: 'm0' });
  op('addNode', { ll: ll(-1, 2), as: 'm1' });
  op('addNode', { ll: ll(-1, 4), as: 'm2' });
  op('addLink', { a: '$m0', b: '$m1', props: { cls: 'motorway', name: 'State Highway 1' }, as: 'mw1' });
  op('addLink', { a: '$m1', b: '$m2', props: { cls: 'motorway', name: 'State Highway 1' }, as: 'mw2' });
  op('addLink', { a: '$m1', b: '$n02', props: { cls: 'ramp', name: 'Ramp A' }, as: 'rp' });
  op('addLink', { a: '$m0', b: '$n00', props: { cls: 'nonsense' }, as: 'odd' });
  op('addLink', { a: '$m2', b: '$n04', as: 'plain' });
  // Controls: set by hand, and a give-way on a junction with two roads
  op('setNode', { id: '$n22', set: { control: 'roundabout' } });
  op('setNode', { id: '$n14', set: { control: 'priority' } });
  op('setNode', { id: '$n32', set: { control: 'stop' } });
  op('setNode', { id: '$n30', set: { control: 'priority' } });
  op('autoControl', { ids: ['$n00', '$n01', '$n02', '$n03', '$n04', '$n10', '$n11', '$n12', '$n13', '$n14', '$n20', '$n21', '$n22', '$n23', '$n24', '$n30', '$n31', '$n32', '$n33', '$n34', '$m0', '$m1', '$m2', 'missing'] });
  // Zones and bus routes
  op('addZone', { zone: { id: 'zA', kind: 'zone', name: 'Houses', lat: r7(ll(2, 0, 0.5, 0.5)[0]), lng: r7(ll(2, 0, 0.5, 0.5)[1]), lu: { residential: 300 }, connectors: null } });
  op('addZone', { zone: { id: 'gW', kind: 'gateway', name: 'West gateway', lat: r7(ll(1, -0.4)[0]), lng: r7(ll(1, -0.4)[1]), node: '$n10', gw: { AMin: 600, AMout: 450, PMin: 500, PMout: 650 } } });
  op('addZone', { zone: { id: 'gS', kind: 'gateway', name: 'South gateway', lat: r7(ll(3.4, 2)[0]), lng: r7(ll(3.4, 2)[1]), node: '$n32', gw: { AMin: 200, AMout: 100, PMin: 100, PMout: 200 } } });
  op('addRoute', { route: { id: 'r1', name: 'Bus route 1', color: '#7048e8', freq: 6, wp: ['$n10', '$n12', '$n14'] } });
  op('addRoute', { route: { id: 'r2', name: 'Bus route 2', color: '#0c8599', freq: 4 } });
  // Looking things up
  const mid = ll(1, 1, 0, 0.5), above = [mid[0] + 0.00007, mid[1]];
  op('nearestLink', { ll: above, maxM: 20 });
  op('nearestLink', { ll: above, maxM: 5 });
  op('nearestLink', { ll: above });
  op('nearestLink', { ll: above, maxM: 400, filter: 'proposed' });
  op('nearestLink', { ll: above, maxM: 400, filter: 'arterial' });
  op('nearestLink', { ll: above, maxM: 400, filter: 'hasName' });
  op('nearestLink', { ll: above, filter: 'notProposed' });
  op('nearestLink', { ll: ll(5, 5), maxM: 300 });
  op('nearestLink', { ll: ll(5, 5) });
  op('nearestLink', { ll: ll(2, 1, 0.35, 0.4), maxM: 3 });
  op('nearestNode', { ll: ll(2, 2, 0.01, 0.01), maxM: 30 });
  op('nearestNode', { ll: ll(2, 2, 0.5, 0.5) });
  op('nearestNode', { ll: ll(2, 2, 0.5, 0.5), filter: 'signals' });
  op('nearestNode', { ll: ll(2, 2, 0.5, 0.5), maxM: 50 });
  op('nearestNode', { ll: ll(0, 0), filter: 'connected' });
  op('project', { id: '$h11', ll: above });
  op('project', { id: '$h11', ll: ll(1, 5) });
  op('project', { id: '$h21', ll: ll(2, 1, 0.5, 0.5) });
  op('linksAt', { id: '$n12' });
  op('linksAt', { id: '$n00' });
  op('degree');
  op('geomLen', { id: '$h21' });
  op('geomLen', { id: '$h00' });
  // Splitting: in the middle, at an end (no split), on a bent road, and a road that carries extra keys
  op('splitLink', { id: '$h11', ll: mid, as: 'sp1' });
  op('splitLink', { id: '$h11', ll: ll(1, 1, 0, 0.001) });
  op('splitLink', { id: '$h11', ll: ll(1, 1, 0, 0.3) });
  op('splitLink', { id: '$h00', ll: [ll(0, 0)[0] + 0.00001, ll(0, 0)[1] + 0.00002] });
  op('splitLink', { id: '$h21', ll: ll(2, 1, 0.3, 0.7), as: 'sp2' });
  op('splitLink', { id: '$h21', ll: ll(2, 1, 0.35, 0.4), as: 'sp3' });
  op('setLink', { id: '$h02', set: { osm: '99:3', toll: { AM: 20, PM: 10 }, varSpeed: { v: 40, AM: true, PM: false }, stage: 'committed', funding: 'funded', capPerLane: 900 }, unset: ['cycle'] });
  op('splitLink', { id: '$h02', ll: ll(0, 2, 0, 0.5), as: 'sp4' });
  // A spur from a split point gives it three roads; deleting the spur takes the control back and prunes the end
  op('addNode', { ll: [mid[0] + 0.0008, mid[1]], as: 'spurEnd' });
  op('addLink', { a: '$sp1', b: '$spurEnd', props: { cls: 'collector', name: 'Spur' }, as: 'spur' });
  op('autoControl', { ids: ['$sp1', '$spurEnd'] });
  op('deleteLink', { id: '$spur' });
  op('deleteLink', { id: 'nope' });
  // A spur whose end a gateway uses keeps its end node; one on a bus route keeps it too
  op('addNode', { ll: ll(3.6, 4), as: 'pin' });
  op('addLink', { a: '$n34', b: '$pin', as: 'pinLink' });
  op('addZone', { zone: { id: 'gP', kind: 'gateway', name: 'Pinned', lat: r7(ll(3.7, 4)[0]), lng: r7(ll(3.7, 4)[1]), node: '$pin', gw: { AMin: 50, AMout: 50, PMin: 50, PMout: 50 } } });
  op('deleteLink', { id: '$pinLink' });
  op('addNode', { ll: ll(3.6, 1), as: 'stop1' });
  op('addLink', { a: '$n31', b: '$stop1', as: 'stopLink' });
  op('setRoute', { id: 'r2', set: { wp: ['$n31', '$stop1'] } });
  op('deleteLink', { id: '$stopLink' });
  // Deleting a junction: its roads, zones that sat on it, routes through it, and the junctions left alone
  op('deleteNode', { id: '$n22' });
  op('deleteNode', { id: '$n10' });
  op('deleteNode', { id: 'missing' });
  op('addNode', { ll: ll(8, 8), as: 'lonely' });
  op('deleteNode', { id: '$lonely' });
  op('reverse', { id: '$v04' });
  op('reverse', { id: '$v04' });
  op('reverse', { id: '$v14' });
  op('autoControl', { ids: ['$n11', '$n12', '$n21', '$n23', '$sp1', '$sp2'] });
  op('geomLen', { id: '$h21' });
  op('degree');
  op('nearestLink', { ll: ll(2, 1, 0.3, 0.7), maxM: 25 });
  return { seed: 101, ops };
}

/* ---------- junction sites ---------- */
export function sitesScript() {
  const { ops, op } = builder();
  const O = [-36.95, 174.78];
  const node = (name, dx, dy, props) => op('addNode', { ll: at(O, dx, dy), ...(props ? { props } : {}), as: name });
  const link = (name, a, b, props, pts) => op('addLink', { a: `$${a}`, b: `$${b}`, ...(pts ? { pts } : {}), props: { name: '', ...props }, as: name });
  op('newProject', { name: 'Edit truth: sites', as: 'proj' });
  // 1. A main road with a staggered pair of signals 50 m apart (joined by the 60 m signals rule) and a pair of give-way junctions 50 m apart (not joined)
  node('a0', 0, 0); node('a1', 200, 0, { control: 'signals' }); node('a2', 250, 0, { control: 'signals' }); node('a3', 450, 0);
  node('a1n', 200, 120); node('a2s', 250, -120); node('a4', 700, 0, { control: 'priority' }); node('a5', 750, 0, { control: 'priority' }); node('a6', 1000, 0);
  node('a4n', 700, 100); node('a5s', 750, -100);
  link('m1', 'a0', 'a1', { cls: 'arterial', name: 'Great South Road' }); link('m2', 'a1', 'a2', { cls: 'arterial', name: 'Great South Road' });
  link('m3', 'a2', 'a3', { cls: 'arterial', name: 'Great South Road' }); link('m4', 'a3', 'a4', { cls: 'arterial', name: 'Great South Road' });
  link('m5', 'a4', 'a5', { cls: 'arterial', name: 'Great South Road' }); link('m6', 'a5', 'a6', { cls: 'arterial', name: 'Great South Road' });
  link('s1', 'a1', 'a1n', { cls: 'collector', name: 'Waihoehoe Road' }); link('s2', 'a2', 'a2s', { cls: 'collector', name: 'Alfriston Road' });
  link('s3', 'a4', 'a4n', { cls: 'local', name: 'Elliot Street' }); link('s4', 'a5', 'a5s', { cls: 'local', name: 'Elliot Street' });
  // 2. A divided road: two one-way carriageways 45 m long, 20 m apart, between candidate nodes (the dual-pair rule, up to 60 m)
  node('d0', 0, 400, { control: 'priority' }); node('d1', 45, 400, { control: 'priority' }); node('e0', 0, 420, { control: 'priority' }); node('e1', 45, 420, { control: 'priority' });
  node('d2', -150, 400); node('d3', 200, 400); node('e2', -150, 420); node('e3', 200, 420);
  link('dc1', 'd0', 'd1', { cls: 'arterial', name: 'Manukau Road', lanesAB: 2, lanesBA: 0 }); link('dc2', 'e1', 'e0', { cls: 'arterial', name: 'Manukau Road', lanesAB: 2, lanesBA: 0 });
  link('dx1', 'd2', 'd0', { cls: 'arterial', name: 'Manukau Road' }); link('dx2', 'd1', 'd3', { cls: 'arterial', name: 'Manukau Road' });
  link('ex1', 'e2', 'e0', { cls: 'arterial', name: 'Manukau Road' }); link('ex2', 'e1', 'e3', { cls: 'arterial', name: 'Manukau Road' });
  link('cross', 'd0', 'e0', { cls: 'collector', name: 'Link Road' });
  // 3. A roundabout (ring nodes 20 m apart round a circle) with four arms, and a give-way junction 30 m from a ring node (never merged)
  const R = at(O, 600, 400), ring = [];
  for (let i = 0; i < 4; i++) { const a = i * Math.PI / 2; ring.push(at(R, 12 * Math.cos(a), 12 * Math.sin(a))); op('addNode', { ll: ring[i], props: { control: 'roundabout' }, as: `rb${i}` }); }
  for (let i = 0; i < 4; i++) link(`ring${i}`, `rb${i}`, `rb${(i + 1) % 4}`, { cls: 'arterial', name: 'Roundabout', ring: true, lanesAB: 1, lanesBA: 0 });
  const arms = [[0, 150], [150, 0], [0, -150], [-150, 0]];
  arms.forEach(([dx, dy], i) => { op('addNode', { ll: at(R, dx, dy), as: `arm${i}` }); link(`armlink${i}`, `rb${i}`, `arm${i}`, { cls: i % 2 ? 'arterial' : 'collector', name: `Arm ${i}` }); });
  op('addNode', { ll: at(R, 40, 12), props: { control: 'priority' }, as: 'near' });
  link('nearlink', 'near', 'rb0', { cls: 'collector', name: 'Near Road' }); link('nearx', 'near', 'arm1', { cls: 'local', name: 'Near Road' });
  link('nearz', 'near', 'arm0', { cls: 'local', name: 'Near Road' });
  // 4. A motorway with a ramp junction (never joined) and an uncontrolled node where three collectors meet beside another (within 35 m)
  node('w0', 0, 800); node('w1', 300, 800); node('w2', 600, 800); node('wr', 300, 900);
  link('w01', 'w0', 'w1', { cls: 'motorway', name: 'Southern Motorway' }); link('w12', 'w1', 'w2', { cls: 'motorway', name: 'Southern Motorway' }); link('wr1', 'wr', 'w1', { cls: 'ramp', name: 'Ramp' });
  node('t0', 800, 800); node('t1', 825, 800); node('t2', 800, 900); node('t3', 825, 700); node('t4', 700, 800); node('t5', 925, 800);
  link('t01', 't0', 't1', { cls: 'collector', name: 'Station Road' }); link('t02', 't0', 't2', { cls: 'collector', name: 'Station Road' }); link('t03', 't0', 't4', { cls: 'collector', name: 'Park Road' });
  link('t13', 't1', 't3', { cls: 'collector', name: 'Park Road' }); link('t15', 't1', 't5', { cls: 'collector', name: 'Park Road' });
  // 5. Three signals in a row 45 m apart (the third would make the site 90 m wide: refused), a closed link and a link with a missing end
  node('g0', 0, 1100, { control: 'signals' }); node('g1', 45, 1100, { control: 'signals' }); node('g2', 90, 1100, { control: 'signals' }); node('g3', -150, 1100); node('g4', 240, 1100);
  link('g01', 'g0', 'g1', { cls: 'arterial', name: 'Wide Road' }); link('g12', 'g1', 'g2', { cls: 'arterial', name: 'Wide Road' }); link('g30', 'g3', 'g0', { cls: 'arterial', name: 'Wide Road' }); link('g24', 'g2', 'g4', { cls: 'arterial', name: 'Wide Road' });
  node('c0', 400, 1100, { control: 'signals' }); node('c1', 420, 1100, { control: 'signals' }); node('c2', 400, 1200); node('c3', 420, 1000);
  link('c01', 'c0', 'c1', { cls: 'collector', name: 'Closed Road', status: 'closed' }); link('c02', 'c0', 'c2', { cls: 'collector', name: 'Closed Road' }); link('c13', 'c1', 'c3', { cls: 'collector', name: 'Closed Road' });
  link('loop', 'c2', 'c2', { cls: 'local', name: 'Loop' });
  // Detection with the default rules
  op('detect');
  op('siteOf', { node: '$a1' });
  op('siteOf', { node: '$a4' });
  op('siteOf', { node: '$nonexistent' });
  // AT controlled-intersection points: one near the give-way pair (so they join), two near the wide pair, one keyed by id
  op('data', { set: { controlled: [
    { lat: r7(at(O, 725, 5)[0]), lng: r7(at(O, 725, 5)[1]), no: 'AT-0451' },
    { lat: r7(at(O, 225, 5)[0]), lng: r7(at(O, 225, 5)[1]), no: 'AT-0210' },
    { lat: r7(at(O, 215, 0)[0]), lng: r7(at(O, 215, 0)[1]), id: 'ATX-9' },
    { lat: r7(at(O, 20, 410)[0]), lng: r7(at(O, 20, 410)[1]), no: 77 },
    { lat: r7(at(O, 4000, 4000)[0]), lng: r7(at(O, 4000, 4000)[1]), no: 'far' },
  ] } });
  op('detect', { points: true });
  op('detect', { points: true, rules: { near: 60, wide: 90, at: 10, diameter: 120, dualBearing: 25, dualGap: 40, source: 'Test rules' } });
  op('detect', { rules: { near: 10, wide: 20 } });
  op('siteName', { nodes: ['$a1', '$a2'] });
  op('siteName', { nodes: ['$d0', '$e0'] });
  op('siteName', { nodes: ['$g1'] });
  op('siteName', { nodes: ['$w1'] });
  // Hand-made sites: merge nodes, merge a site with a node, and detection keeps them
  op('detect', { points: true });
  op('merge', { ids: ['$a4', '$a5'], as: 'hand1' });
  op('merge', { ids: ['$a4'] });
  op('merge', { ids: ['$nonexistent', '$a6'] });
  op('merge', { ids: ['$hand1', '$a3'], as: 'hand2' });
  op('detect', { points: true });
  op('setNode', { id: '$g1', set: { control: 'none' } });
  op('setNode', { id: '$g2', set: { control: 'none' } });
  op('detect');
  op('splitSite', { id: '$hand2' });
  op('splitSite', { id: 'nope' });
  op('detect', { points: true });
  op('merge', { ids: ['$a1', '$a2'], as: 'hand3' });
  op('siteOf', { node: '$a1' });
  return { seed: 202, ops };
}

/* ---------- scenarios, the register and checks ---------- */
export function scenariosScript() {
  const { ops, op } = builder();
  const lat0 = -37.0, lng0 = 174.9, dLat = 0.002, dLng = 0.0025;
  const ll = (r, k) => [lat0 - r * dLat, lng0 + k * dLng];
  op('newProject', { name: 'Edit truth: scenarios', as: 'proj' });
  op('settings', { set: { baseYear: 2026, year: 2026, growth: 1.5, growthMode: 'linear' } });
  for (let r = 0; r < 3; r++) for (let k = 0; k < 4; k++) op('addNode', { ll: ll(r, k), as: `n${r}${k}` });
  const L = (name, a, b, props) => op('addLink', { a: `$${a}`, b: `$${b}`, props, as: name });
  for (let r = 0; r < 3; r++) for (let k = 0; k < 3; k++) L(`h${r}${k}`, `n${r}${k}`, `n${r}${k + 1}`, { cls: r === 0 ? 'arterial' : 'collector', name: `Row ${r}` });
  for (let r = 0; r < 2; r++) for (let k = 0; k < 4; k++) L(`v${r}${k}`, `n${r}${k}`, `n${r + 1}${k}`, { cls: 'local', name: `Col ${k}` });
  // Stages and funding on some roads
  op('setLink', { id: '$h00', set: { stage: 'committed', funding: 'funded', status: 'proposed' } });
  op('setLink', { id: '$h01', set: { stage: 'committed', funding: 'programmed', status: 'proposed' } });
  op('setLink', { id: '$h02', set: { stage: 'unfunded', status: 'proposed' } });
  op('setLink', { id: '$h10', set: { stage: 'committed', funding: 'unfunded', status: 'proposed' } });
  op('setLink', { id: '$v00', set: { status: 'proposed' } });
  op('setLink', { id: '$v01', set: { stage: 'proposal', status: 'proposed' } });
  op('setLink', { id: '$v02', set: { stage: 'mitigation', status: 'proposed' } });
  op('setLink', { id: '$h21', set: { status: 'closed', stage: 'unfunded' } });
  op('setLink', { id: '$v13', set: { stage: 'bogus' } });
  for (const id of ['h00', 'h01', 'h02', 'h10', 'v00', 'v01', 'v02', 'h21', 'v13', 'h11']) op('linkStage', { id: `$${id}` });
  // Zones of every role, a gateway, and a zone tied to a register row
  const z = (id, name, p, lu, extra = {}) => op('addZone', { zone: { id, kind: 'zone', name, lat: r7(ll(...p)[0]), lng: r7(ll(...p)[1]), lu, connectors: null, ...extra } });
  z('zEx', 'Existing houses', [0.5, 0.5], { residential: 300, school: 400 }, { existing: true });
  z('zCom', 'Committed flats', [0.5, 1.5], { apartment: 200 }, { role: 'committed', year: 2028, reg: 'preset:sunfield' });
  z('zCom2', 'Committed shops', [0.5, 2.5], { retail: 2000 }, { role: 'committed', reg: 'consent-1' });
  z('zCom3', 'Committed offices', [1.5, 0.5], { office: 5000 }, { role: 'committed' });
  z('zCom4', 'Committed no row', [1.5, 1.5], { office: 1000 }, { role: 'committed', reg: 'missing-row' });
  z('zDev', 'Proposed estate', [1.5, 2.5], { residential: 500, childcare: 60 }, { role: 'development', year: 2030, buildout: 60 });
  z('zOld', 'Untagged', [1.8, 0.2], { industrial: 4000 });
  z('zBad', 'Odd role', [1.8, 1.2], { industrial: 1000 }, { role: 'nonsense', existing: 'yes' });
  op('addZone', { zone: { id: 'gW', kind: 'gateway', name: 'West gateway', lat: r7(ll(1, -0.5)[0]), lng: r7(ll(1, -0.5)[1]), node: '$n10', gw: { AMin: 600, AMout: 450, PMin: 500, PMout: 650 } } });
  for (const id of ['zEx', 'zCom', 'zDev', 'zOld', 'zBad', 'gW']) op('zoneRole', { id });
  op('addRoute', { route: { id: 'r1', name: 'Bus', color: '#7048e8', freq: 6, wp: ['$n10', '$n12'] } });
  // The register: the presets, and rows of each kind
  op('presets');
  op('registerTable');
  op('register', { rows: [
    { id: 'consent-1', ref: 'RC-2025-1', name: 'Shops', type: 'consent', yield: 2000, status: 'Granted 14 Mar 2025', likely: 'likely', source: 'Council', date: '2025-03-14', zones: ['zCom2'], links: [] },
    { id: 'preset:sunfield', ref: '', name: 'Sunfield', type: 'ftaa', status: 'Secondary-source status; High Court appeal', likely: 'possible', verify: true, preset: true, zones: ['zCom'], links: [] },
    { id: 'plan-1', name: 'Plan change 5', type: 'planchange', status: 'Operative', likely: 'possible', asAt: '2026-01-01', zones: [], links: [] },
    { id: 'plan-2', name: 'Plan change 6', type: 'planchange', status: 'operative in part', likely: 'likely', date: '2026-02-02', zones: [], links: [] },
    { id: 'net-1', name: 'Link road', type: 'network', funding: 'funded', status: 'Funded', zones: [], links: ['h00'] },
    { id: 'net-2', name: 'Bridge', type: 'network', funding: 'programmed', zones: [], links: ['h01'] },
    { id: 'net-3', name: 'Bypass', type: 'network', funding: 'unfunded', zones: [], links: ['h02', 'h21'] },
    { id: 'net-4', name: 'Interchange', type: 'network', zones: [], links: ['v01'] },
    { id: 'net-5', name: 'Overridden in', type: 'network', funding: 'unfunded', include: true, note: 'Council confirmed', zones: [], links: ['v02'] },
    { id: 'net-6', name: 'Overridden out', type: 'network', funding: 'funded', include: false, zones: [], links: [] },
    { id: 'pre-1', name: 'Precinct', type: 'precinct', status: 'Approved', likely: 'unlikely', zones: [], links: [] },
    { id: 'odd-1', name: 'Odd', type: 'mystery', zones: [], links: [] },
  ] });
  op('registerTable');
  for (const row of [null, { id: 'x', type: 'network', funding: 'funded' }, { id: 'x', type: 'consent', status: 'Granted', likely: 'likely' }, { id: 'x', type: 'consent', status: 'GRANTED', likely: 'Likely' },
    { id: 'x', type: 'consent', status: '', likely: 'likely' }, { id: 'x', include: false, note: 'No' }, { id: 'x', include: true }, { id: 'x', type: 'consent', status: 'approved' }]) op('inBaseline', { row });
  op('selectScenario', { id: '$proj.sc' });
  op('designYear', { type: 'consent', opening: 2028 });
  op('designYear', { type: 'planchange', opening: 2030 });
  op('designYear', { type: 'nor', opening: 2030 });
  op('designYear', { type: 'ftaa', opening: 2027 });
  op('designYear', { type: 'nonsense', opening: 2027 });
  op('designYear', { type: 'consent', opening: null });
  op('describe');
  op('check');
  // Sets
  op('createSet', { opts: { type: 'planchange', countYear: 2026, opening: 2031, design: 2036, mitigation: true, periods: ['AM', 'PM'], buildout: { 2036: 80 } } });
  op('table');
  op('groups');
  op('createSet', { opts: { type: 'ftaa', opening: 2028 } });
  op('createSet', { opts: { type: 'consent', countYear: 2026, opening: 2026, design: 2026, mitigation: false, periods: ['AM'], buildout: { 2026: 250 } } });
  op('createSet', { opts: { type: 'nonsense', countYear: 2040, opening: 2030, design: 2050, buildout: { 2030: -5, 2050: 10 } } });
  op('table');
  op('groups');
  // Sensitivity copies, with and without the unfunded projects, and a copy with filters
  op('sensitivityOf', { unfunded: false, as: 'sens1' });
  op('sensitivityOf', { unfunded: true, as: 'sens2' });
  op('sensitivityOf', { sc: 'nope' });
  op('copy', { links: 'notProposed', zones: 'noGateways', push: true, as: 'cp1' });
  op('copy', { links: 'arterial', push: true, as: 'cp2' });
  op('copy', { zones: 'residential', push: true, as: 'cp3' });
  op('copy', { links: 'keepAll', push: false, as: 'cp4' });
  op('table');
  op('groups');
  // Describe and check every scenario the project now has
  op('setScenario', { set: { role: 'permitted' } });
  op('check');
  op('describe');
  return { seed: 303, ops, describeAll: true };
}

/* ---------- undo and redo ---------- */
export function historyScript() {
  const { ops, op } = builder();
  const ll = (i) => [-36.9 - i * 0.0004, 174.7 + i * 0.0006];
  op('newProject', { name: 'Edit truth: history', as: 'proj' });
  op('historyLength');
  op('undo');
  op('redo');
  for (let i = 0; i < 4; i++) { op('snapshot', { rec: false }); op('addNode', { ll: ll(i), as: `n${i}`, rec: false }); }
  op('snapshot', { rec: false }); op('addLink', { a: '$n0', b: '$n1', props: { cls: 'collector', name: 'One' }, as: 'l0', rec: false });
  op('snapshot', { rec: false }); op('addLink', { a: '$n1', b: '$n2', props: { cls: 'arterial', name: 'Two' }, as: 'l1' });
  op('snapshot', { rec: false }); op('addZone', { zone: { id: 'z1', kind: 'zone', name: 'Zone 1', lat: ll(1)[0], lng: ll(1)[1], lu: { residential: 200 }, connectors: null } });
  op('snapshot', { rec: false }); op('setScenario', { set: { role: 'baseline', year: 2031, buildout: 70, periods: ['AM'] } });
  op('historyLength');
  op('undo'); op('undo'); op('historyLength');
  op('redo'); op('historyLength');
  op('snapshot', { rec: false }); op('deleteLink', { id: '$l0' });
  op('redo');
  op('undo'); op('undo'); op('undo');
  op('historyLength');
  // The cap: past 60 steps the oldest go
  for (let i = 4; i < 70; i++) { op('snapshot', { rec: false }); op('addNode', { ll: ll(i), as: `n${i}`, rec: false }); }
  op('historyLength');
  for (let i = 0; i < 62; i++) op('undo', { rec: i % 20 === 0 });
  op('historyLength');
  op('undo');
  for (let i = 0; i < 3; i++) op('redo', { rec: i === 2 });
  op('historyLength');
  return { seed: 404, ops };
}

/* ---------- random networks: many operations of every kind in a random order (positions are picked by index, so a deleted road never breaks the list) ---------- */
export function fuzzScript(seed, { clusters = 9, extra = 150 } = {}) {
  const { ops, op } = builder();
  let s = seed >>> 0;
  const rnd = () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
  const uni = (a, b) => a + (b - a) * rnd();
  const int = (a, b) => a + Math.floor(rnd() * (b - a + 1));
  const pick = a => a[Math.floor(rnd() * a.length)];
  const O = [-36.95, 174.78];
  const NAMES = ['Great South Road', 'Station Road', 'Elliot Street', 'Hunter Street', 'Wood Street', '', '', 'Main Road'];
  op('newProject', { name: `Edit truth: random ${seed}` });
  // Clusters of two to four junctions within 35 m of a centre: staggered junctions, divided roads, rings
  const nodes = [], ranges = [];
  for (let ci = 0; ci < clusters; ci++) {
    const c = [uni(-700, 700), uni(-500, 500)];
    const first = nodes.length;
    for (let k = int(2, 4); k > 0; k--) {
      const p = [c[0] + uni(-35, 35), c[1] + uni(-35, 35)];
      const control = pick([undefined, undefined, 'signals', 'signals', 'priority', 'roundabout', 'stop', 'none']);
      op('addNode', { ll: at(O, p[0], p[1]), ...(control ? { props: { control } } : {}), rec: false });
      nodes.push(p);
    }
    ranges.push([first, nodes.length - 1]);
  }
  const N = nodes.length;
  const props = () => {
    const cls = pick(['motorway', 'ramp', 'arterial', 'arterial', 'collector', 'collector', 'local', 'local', 'local']);
    const p = { cls, name: pick(NAMES), status: pick(['existing', 'existing', 'existing', 'existing', 'proposed', 'closed']) };
    if (rnd() < 0.3) Object.assign(p, { lanesAB: int(1, 3), lanesBA: 0 });
    else if (rnd() < 0.3) Object.assign(p, { lanesAB: int(1, 3), lanesBA: int(1, 3) });
    if (rnd() < 0.08) p.ring = true;
    if (rnd() < 0.1) p.busLane = true;
    return p;
  };
  const link = () => {
    const a = int(0, N - 1);
    // prefer a neighbour within 450 m
    const near = nodes.map((n, i) => [i, Math.hypot(n[0] - nodes[a][0], n[1] - nodes[a][1])]).filter(([i, d]) => i !== a && d < 450);
    const b = near.length && rnd() < 0.9 ? pick(near)[0] : int(0, N - 1);
    const mid = [(nodes[a][0] + nodes[b][0]) / 2 + uni(-20, 20), (nodes[a][1] + nodes[b][1]) / 2 + uni(-20, 20)];
    op('addLink', { a: `@n:${a}`, b: `@n:${b}`, ...(rnd() < 0.4 ? { pts: [at(O, mid[0], mid[1])] } : {}), props: props(), rec: false });
  };
  // Inside each cluster: roads between its junctions (two-way, or a divided pair of one-way carriageways), so sites can form
  for (const [first, last] of ranges) for (let a = first; a < last; a++) for (let b = a + 1; b <= last; b++) {
    if (rnd() < 0.25) continue;
    const cls = pick(['arterial', 'arterial', 'collector', 'local']), name = pick(NAMES);
    if (rnd() < 0.35) {
      op('addLink', { a: `@n:${a}`, b: `@n:${b}`, props: { cls, name, lanesAB: 2, lanesBA: 0 }, rec: false });
      op('addLink', { a: `@n:${b}`, b: `@n:${a}`, props: { cls, name, lanesAB: 2, lanesBA: 0 }, rec: false });
    } else op('addLink', { a: `@n:${a}`, b: `@n:${b}`, props: { cls, name, ...(rnd() < 0.2 ? { ring: true, lanesAB: 1, lanesBA: 0 } : {}) }, rec: false });
  }
  for (let t = 0; t < N * 1.7; t++) link();
  op('addZone', { zone: { id: 'zF', kind: 'zone', name: 'Estate', lat: at(O, 0, 0)[0], lng: at(O, 0, 0)[1], lu: { residential: 100 }, node: '@n:3', connectors: null } });
  op('addZone', { zone: { id: 'gF', kind: 'gateway', name: 'West gateway', lat: at(O, -800, 0)[0], lng: at(O, -800, 0)[1], node: '@n:0', gw: { AMin: 100, AMout: 100, PMin: 100, PMout: 100 } } });
  op('addRoute', { route: { id: 'rF', name: 'Bus', color: '#7048e8', freq: 6, wp: ['@n:1', '@n:5', '@n:6'] } });
  const box = () => at(O, uni(-800, 800), uni(-600, 600));
  const sel = (k, n = 999) => `@${k}:${int(0, n)}`;
  for (let t = 0; t < extra; t++) {
    const r = rnd(), rec = t % 12 === 11 ? { rec: 'sc' } : { rec: false };
    if (r < 0.2) op('splitLink', { id: sel('l'), ll: box(), ...rec });
    else if (r < 0.31) op('addNode', { ll: box(), ...(rnd() < 0.3 ? { props: { control: pick(['signals', 'priority']) } } : {}), ...rec });
    else if (r < 0.46) op('addLink', { a: sel('n'), b: sel('n'), props: props(), ...rec });
    else if (r < 0.5) op('deleteLink', { id: sel('l'), ...rec });
    else if (r < 0.52) op('deleteNode', { id: sel('n'), ...rec });
    else if (r < 0.56) op('reverse', { id: sel('l'), ...rec });
    else if (r < 0.62) op('autoControl', { ids: [sel('n'), sel('n'), sel('n'), sel('n')], ...rec });
    else if (r < 0.67) op('setNode', { id: sel('n'), set: { control: pick(['none', 'priority', 'stop', 'roundabout', 'signals']) }, ...rec });
    else if (r < 0.72) op('setLink', { id: sel('l'), set: pick([{ status: 'closed' }, { status: 'existing' }, { cls: 'collector' }, { lanesAB: 2, lanesBA: 0 }, { lanesAB: 1, lanesBA: 1 }, { ring: true }]), ...rec });
    else if (r < 0.8) op('detect', { ...(rnd() < 0.3 ? { rules: pick([{ near: 50 }, { wide: 90, diameter: 120 }, { dualGap: 90, dualBearing: 30 }, { near: 20, wide: 40 }]) } : {}), ...rec });
    else if (r < 0.83) op('merge', { ids: [sel('n'), sel('n'), ...(rnd() < 0.3 ? [sel('j', 5)] : [])], ...rec });
    else if (r < 0.85) op('splitSite', { id: sel('j', 5), ...rec });
    else if (r < 0.89) op('nearestLink', { ll: box(), ...(rnd() < 0.7 ? { maxM: pick([30, 100, 300]) } : {}), filter: pick(['none', 'arterial', 'proposed', 'hasName']) });
    else if (r < 0.92) op('nearestNode', { ll: box(), ...(rnd() < 0.7 ? { maxM: pick([30, 100, 300]) } : {}), filter: pick(['none', 'signals', 'connected']) });
    else if (r < 0.95) op('project', { id: sel('l'), ll: box() });
    else if (r < 0.97) op('linksAt', { id: sel('n') });
    else if (r < 0.98) op('degree');
    else op('geomLen', { id: sel('l') });
  }
  op('detect', { rec: 'sc' });
  op('degree');
  return { seed, ops };
}
