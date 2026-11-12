/* Spare Construct · Workshop: the prototype's own builder.

   It builds a part's recipe the way the geometry kernel's builder does (crate construct-workshop, through
   engine-geometry) and gives the same answer, so the page can use either: a convex outline becomes a prism
   (the kernel's make.prism), a circle a cylinder (make.cylinder), and mirror, pattern and move are the kernel's
   transforms. Every coordinate is rounded to the kernel's grid for parts, 6.25 nm, as the kernel rounds it.
   What the kernel cannot build yet is not faked: its feature is marked as waiting, with the pieces it waits for.

   The answer, for a part:
     { engine, features: [{ id, state, message, bodies }], bodies: [body], volume, bbox }
   state is built, waits, failed, suppressed or rolled_back. A body is
     { id, by, kind: 'prism' | 'cylinder', outline | center and radius, z, height, m, exactness, volume, faces, bbox }
   where m is the affine map (3 rows of 4, row-major) from the profile's own place to where the body is. */

import { WAITS, MATERIALS } from './model.js';

export const STEPS_PER_MM = 160000;   // the kernel's grid step for parts and boards, Q1: 6.25 nm
export const REACH_MM = (2 ** 40 - 1) / STEPS_PER_MM;   // geom_num::grid::COORD_MAX in steps: about 6.87 km
const q = (v) => Math.round(v * STEPS_PER_MM) / STEPS_PER_MM;
const fmt = (v) => (Math.round(v * 1000) / 1000).toString();

export const IDENTITY = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0];
/* a then b: the map that applies a first */
export function then(a, b) {
  const r = [];
  for (let i = 0; i < 3; i++) {
    for (let j = 0; j < 4; j++) {
      let s = j === 3 ? b[i * 4 + 3] : 0;
      for (let k = 0; k < 3; k++) s += b[i * 4 + k] * a[k * 4 + j];
      r.push(s);
    }
  }
  return r;
}
export const apply = (m, p) => [0, 1, 2].map((i) => m[i * 4] * p[0] + m[i * 4 + 1] * p[1] + m[i * 4 + 2] * p[2] + m[i * 4 + 3]);
export const translation = (d) => [1, 0, 0, d[0], 0, 1, 0, d[1], 0, 0, 1, d[2]];
export function mirrorMap(axis, at) {
  const m = IDENTITY.slice();
  const i = 'xyz'.indexOf(axis);
  m[i * 4 + i] = -1;
  m[i * 4 + 3] = 2 * at;
  return m;
}
export function rotation(axis, deg, about) {
  const a = (deg * Math.PI) / 180, c = Math.cos(a), s = Math.sin(a);
  const r = { x: [1, 0, 0, 0, c, -s, 0, s, c], y: [c, 0, s, 0, 1, 0, -s, 0, c], z: [c, -s, 0, s, c, 0, 0, 0, 1] }[axis];
  const m = [r[0], r[1], r[2], 0, r[3], r[4], r[5], 0, r[6], r[7], r[8], 0];
  return then(then(translation(about.map((v) => -v)), m), translation(about));
}

class Refusal extends Error {
  constructor(state, message) { super(message); this.state = state; }
}
const fail = (m) => { throw new Refusal('failed', m); };
const waits = (m) => { throw new Refusal('waits', m); };

/* A field's value: a number, or the name of one of the part's parameters. Formulas are worked out by the kernel's
   builder (crate construct-formulas, exact and with units); this prototype doesn't read them. */
function value(env, v, what) {
  if (typeof v === 'number' && Number.isFinite(v)) return v;
  if (typeof v === 'string' && v.trim() !== '') {
    const t = v.trim();
    if (/^[-+]?\d*\.?\d+(e[-+]?\d+)?$/i.test(t)) return Number(t);
    if (Object.prototype.hasOwnProperty.call(env, t)) return env[t];
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(t)) fail(`${what}: formulas are worked out by the geometry kernel's builder, which hasn't loaded here.`);
    fail(`${what}: there is no parameter called ${t}.`);
  }
  fail(`${what} is not set.`);
}
const length = (env, v, what) => q(value(env, v, what));
function positive(env, v, what) {
  const x = length(env, v, what);
  if (!(x >= 1 / STEPS_PER_MM)) fail(`${what} must be more than zero.`);
  return x;
}
function whole(env, v, what, lo, hi) {
  const x = value(env, v, what);
  if (!Number.isInteger(x) || x < lo || x > hi) fail(`${what} must be a whole number from ${lo} to ${hi}.`);
  return x;
}

const cross = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
export function area2(pts) {
  let s = 0;
  for (let i = 0; i < pts.length; i++) {
    const a = pts[i], b = pts[(i + 1) % pts.length];
    s += a[0] * b[1] - b[0] * a[1];
  }
  return s;
}
/* Strictly convex and counter-clockwise, as make.prism asks: every turn to the left, none straight on. */
export function convex(pts) {
  const n = pts.length;
  for (let i = 0; i < n; i++) if (!(cross(pts[i], pts[(i + 1) % n], pts[(i + 2) % n]) > 0)) return false;
  // a star can turn left at every corner; once round means the turns add to one turn
  let turn = 0;
  for (let i = 0; i < n; i++) {
    const a = pts[i], b = pts[(i + 1) % n], c = pts[(i + 2) % n];
    const t = Math.atan2(cross(a, b, c), (b[0] - a[0]) * (c[0] - b[0]) + (b[1] - a[1]) * (c[1] - b[1]));
    turn += t;
  }
  return Math.abs(turn - 2 * Math.PI) < 1e-6;
}

/* A sketch's profile: a circle, or an outline taken counter-clockwise. */
function profile(env, f) {
  const z = length(env, f.z ?? 0, 'The sketch height');
  const cx = length(env, f.cx ?? 0, 'The centre x'), cy = length(env, f.cy ?? 0, 'The centre y');
  if (f.shape === 'circle') return { circle: true, center: [cx, cy], radius: positive(env, f.r, 'The radius'), z };
  let pts;
  if (f.shape === 'rect') {
    const w = positive(env, f.w, 'The width'), h = positive(env, f.h, 'The height');
    pts = [[cx - w / 2, cy - h / 2], [cx + w / 2, cy - h / 2], [cx + w / 2, cy + h / 2], [cx - w / 2, cy + h / 2]];
  } else if (f.shape === 'polygon') {
    const n = whole(env, f.n, 'The number of sides', 3, 64), r = positive(env, f.r, 'The radius');
    pts = [];
    for (let k = 0; k < n; k++) {
      const a = -Math.PI / 2 + Math.PI / n + (2 * Math.PI * k) / n;
      pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
    }
  } else if (f.shape === 'outline') {
    if (!Array.isArray(f.pts) || f.pts.length < 3) fail('An outline needs at least three corners.');
    pts = f.pts.map((p, i) => [cx + length(env, p[0], `Corner ${i + 1} x`), cy + length(env, p[1], `Corner ${i + 1} y`)]);
  } else fail(`There is no sketch shape called ${f.shape}.`);
  pts = pts.map((p) => [q(p[0]), q(p[1])]);
  for (let i = 0; i < pts.length; i++) {
    const a = pts[i], b = pts[(i + 1) % pts.length];
    if (a[0] === b[0] && a[1] === b[1]) fail(`Corners ${i + 1} and ${((i + 1) % pts.length) + 1} are the same point.`);
  }
  const s = area2(pts);
  if (s === 0) fail('The outline has no area.');
  const turned = s < 0;
  if (turned) pts.reverse();
  return { outline: pts, z, convex: convex(pts), turned };
}

function vertices(b) {
  const out = [];
  if (b.kind === 'prism') {
    for (const p of b.outline) {
      out.push(apply(b.m, [p[0], p[1], b.z]), apply(b.m, [p[0], p[1], b.z + b.height]));
    }
  } else {
    const [x, y] = b.center, r = b.radius;
    for (const dx of [-r, r]) for (const dy of [-r, r]) for (const dz of [0, b.height]) out.push(apply(b.m, [x + dx, y + dy, b.z + dz]));
  }
  return out;
}
function boxOf(points) {
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const p of points) for (let i = 0; i < 3; i++) { lo[i] = Math.min(lo[i], p[i]); hi[i] = Math.max(hi[i], p[i]); }
  return [lo, hi];
}
function finish(b) {
  b.bbox = boxOf(vertices(b));
  for (const p of b.bbox) for (const v of p) if (Math.abs(v) > REACH_MM) fail("A corner lands past the kernel's reach for parts, 6.87 km from the origin.");
  return b;
}

/* Builds one part. */
export function buildPart(part) {
  const env = {};
  const out = { engine: 'prototype', features: [], bodies: [] };
  const params = [];
  for (const p of part.params || []) {
    let state = 'built', message = '';
    try {
      if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(p.name)) fail('A name is letters, digits and _, starting with a letter.');
      if (Object.prototype.hasOwnProperty.call(env, p.name)) fail('Another parameter has this name.');
      env[p.name] = value(env, p.value, p.name);
    } catch (e) { state = 'failed'; message = e.message; }
    params.push({ name: p.name, state, message, value: env[p.name] });
  }
  out.params = params;
  const made = new Map();   // feature id -> what it made (a profile, or body ids)
  const live = new Map();   // body id -> body, in the order made
  const end = Math.max(0, Math.min(part.end ?? part.features.length, part.features.length));
  part.features.forEach((f, i) => {
    const row = { id: f.id, state: 'built', message: '', bodies: [] };
    out.features.push(row);
    if (i >= end) { row.state = 'rolled_back'; row.message = 'After the end of the history.'; return; }
    if (f.suppressed) { row.state = 'suppressed'; row.message = 'Suppressed: kept in the history, not built.'; return; }
    const ref = (id, what) => {
      const k = part.features.findIndex((g) => g.id === id);
      if (!id || k < 0) fail(`Pick the ${what}.`);
      if (k >= i) fail(`The ${what} comes later in the history.`);
      const r = made.get(id);
      if (!r) fail(`The ${what}, ${part.features[k].name}, is not built (${out.features[k].state.replace('_', ' ')}).`);
      return r;
    };
    try {
      if (WAITS[f.type]) waits(`Waits for ${WAITS[f.type].what}.`);
      if (f.type === 'sketch') {
        const p = profile(env, f);
        made.set(f.id, { profile: p });
        row.message = p.circle ? 'A circle: extrudes to a cylinder.'
          : p.convex ? `Convex, ${p.outline.length} corners: extrudes to a prism.${p.turned ? ' Drawn clockwise, taken anticlockwise.' : ''}`
            : `Not convex. Extruding it waits for ${WAITS.outline.what}.`;
        row.convex = !!(p.circle || p.convex);
      } else if (f.type === 'extrude') {
        const r = ref(f.sketch, 'sketch');
        if (!r.profile) fail('Extrude a sketch, not a body.');
        if ((f.op || 'new') !== 'new') waits(`${WAITS[f.op] ? WAITS[f.op].label : f.op} waits for ${WAITS.join.what}. A new body extrudes today.`);
        const p = r.profile, height = positive(env, f.depth, 'The distance');
        let b;
        if (p.circle) b = { kind: 'cylinder', center: p.center, radius: p.radius, exactness: 'Certified', faces: 3, volume: Math.PI * p.radius * p.radius * height };
        else if (!p.convex) waits(`Extruding a shape that is not convex waits for ${WAITS.outline.what}.`);
        else b = { kind: 'prism', outline: p.outline, exactness: 'Exact', faces: p.outline.length + 2, volume: (area2(p.outline) / 2) * height };
        Object.assign(b, { id: f.id, by: f.id, z: p.z, height, m: IDENTITY.slice() });
        live.set(b.id, finish(b));
        made.set(f.id, { bodies: [b.id] });
        row.bodies = [b.id];
        row.message = b.kind === 'prism' ? `A prism of ${b.faces} faces, Exact.` : 'A cylinder of 3 faces, Certified within one grid step.';
      } else if (f.type === 'mirror' || f.type === 'pattern' || f.type === 'move') {
        const r = ref(f.of, 'feature to copy');
        if (!r.bodies) fail('Pick a feature that made a body.');
        const src = r.bodies.map((id) => live.get(id)).filter(Boolean);
        if (!src.length) fail('That feature has no bodies left.');
        const ids = [];
        if (f.type === 'mirror') {
          const axis = f.plane || 'x';
          if (!'xyz'.includes(axis) || axis.length !== 1) fail('Mirror across x, y or z.');
          const mm = mirrorMap(axis, length(env, f.at ?? 0, 'The mirror plane'));
          for (const s of src) {
            const b = finish(Object.assign({}, s, { id: `${f.id}:${s.id}`, by: f.id, m: then(s.m, mm) }));
            live.set(b.id, b);
            ids.push(b.id);
          }
          row.message = `Mirrored across the plane ${axis} = ${fmt(length(env, f.at ?? 0, ''))} mm, Exact.`;
        } else if (f.type === 'pattern') {
          const n = whole(env, f.count, 'The number in the row', 2, 100);
          const d = ['dx', 'dy', 'dz'].map((k) => length(env, f[k] ?? 0, 'The spacing'));
          const n2 = f.count2 == null ? 1 : whole(env, f.count2, 'The number of rows', 1, 100);
          const d2 = ['dx2', 'dy2', 'dz2'].map((k) => length(env, f[k] ?? 0, 'The row spacing'));
          if (d.every((v) => v === 0)) fail('The spacing is zero.');
          if (n2 > 1 && d2.every((v) => v === 0)) fail('The row spacing is zero.');
          for (let j = 0; j < n2; j++) {
            for (let k = 0; k < n; k++) {
              if (!j && !k) continue;
              const t = translation([0, 1, 2].map((a) => k * d[a] + j * d2[a]));
              for (const s of src) {
                const b = finish(Object.assign({}, s, { id: `${f.id}:${s.id}:${j}.${k}`, by: f.id, m: then(s.m, t) }));
                live.set(b.id, b);
                ids.push(b.id);
              }
            }
          }
          row.message = `${n * n2 - 1} cop${n * n2 - 1 === 1 ? 'y' : 'ies'}, each moved by whole grid steps, Exact.`;
        } else {
          const d = ['dx', 'dy', 'dz'].map((k) => length(env, f[k] ?? 0, 'The distance'));
          for (const s of src) {
            const b = finish(Object.assign({}, s, { m: then(s.m, translation(d)) }));
            live.set(b.id, b);
            ids.push(b.id);
          }
          row.message = `Moved ${d.map(fmt).join(', ')} mm, Exact.`;
        }
        made.set(f.id, { bodies: ids });
        row.bodies = ids;
      } else fail(`There is no feature called ${f.type}.`);
    } catch (e) {
      if (!(e instanceof Refusal)) throw e;
      row.state = e.state;
      row.message = e.message;
    }
  });
  out.bodies = [...live.values()];
  out.volume = out.bodies.reduce((s, b) => s + b.volume, 0);
  out.bbox = out.bodies.length ? boxOf(out.bodies.flatMap((b) => b.bbox)) : null;
  if (part.material) out.mass = massOf(part.material, out.bodies);
  return out;
}

/* A part's mass in its material, in the kernel builder's form (crate construct-workshop, src/mass.rs): a prism's
   volume is exact, a cylinder's worked out from the recipe; the centre of mass is each body's centroid through its
   map, weighted by its volume. */
export function massOf(id, bodies) {
  const exactly = (b) => (b.volume_from ? b.volume_from === 'kernel' : b.kind === 'prism');
  const exact = bodies.filter(exactly).reduce((s, b) => s + b.volume, 0);
  const fromRecipe = bodies.filter((b) => !exactly(b)).reduce((s, b) => s + b.volume, 0);
  const volume = exact + fromRecipe;
  let centre = null;
  if (volume > 0) {
    centre = [0, 0, 0];
    for (const b of bodies) {
      let c;
      if (b.outline) {
        let a = 0, x = 0, y = 0;
        b.outline.forEach((p, i) => {
          const r = b.outline[(i + 1) % b.outline.length], k = p[0] * r[1] - r[0] * p[1];
          a += k; x += (p[0] + r[0]) * k; y += (p[1] + r[1]) * k;
        });
        c = [x / (3 * a), y / (3 * a), b.z + b.height / 2];
      } else c = [b.center[0], b.center[1], b.z + b.height / 2];
      apply(b.m, c).forEach((v, k) => { centre[k] += v * b.volume; });
    }
    centre = centre.map((v) => v / volume);
  }
  const m = MATERIALS[id];
  if (!m) return { material: id, name: null, density: null, source: null, grams: null, exact, from_recipe: fromRecipe, centre, message: `There is no material called ${id} with a sourced density, so the part has no mass. Pick one from the list.` };
  const grams = (volume * m.density) / 1e6;
  const volumes = fromRecipe > 0
    ? `${fmt(exact)} mm³ of the volume is the kernel's, exact, and ${fmt(fromRecipe)} mm³ is worked out from the recipe for its cylinders, whose certified volume waits for the kernel's mass properties (G247)`
    : "the volume is the kernel's, exact";
  return {
    material: id, name: m.label, density: m.density, source: m.source, grams, exact, from_recipe: fromRecipe, centre,
    message: `${fmt(grams)} g of ${m.label}: ${volumes}; the density, ${fmt(m.density)} kg/m³, is from ${m.source}. The centre of mass is worked out from the recipe; the kernel's certified mass properties wait for G247.`,
  };
}

/* Where each instance of an assembly sits, with its joints at their values, clamped to their limits. */
export function pose(asm) {
  const at = new Map();
  for (const inst of asm.instances) if (inst.grounded) at.set(inst.id, translation(inst.at || [0, 0, 0]));
  const notes = [];
  // a joint places its second part from its first; go round until nothing more can be placed
  let moved = true;
  while (moved) {
    moved = false;
    for (const j of asm.joints) {
      if (!at.has(j.a) || at.has(j.b)) continue;
      const inst = asm.instances.find((x) => x.id === j.b);
      if (!inst) continue;
      const v = Math.min(j.max, Math.max(j.min, Number(j.value) || 0));
      let jm = translation(j.origin);
      if (j.type === 'hinge') jm = then(rotation(j.axis, v, [0, 0, 0]), jm);
      else if (j.type === 'slider') jm = then(translation([0, 1, 2].map((i) => ('xyz'[i] === j.axis ? v : 0))), jm);
      at.set(j.b, then(then(translation(inst.at || [0, 0, 0]), jm), at.get(j.a)));
      moved = true;
    }
  }
  for (const inst of asm.instances) if (!at.has(inst.id)) { at.set(inst.id, translation(inst.at || [0, 0, 0])); notes.push(`${inst.name} is not joined to a grounded part, so it stays where it was put.`); }
  return { at, notes };
}
