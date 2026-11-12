/* Spare Construct · Workshop: the page. The project browser on the left (workspaces, parts, parameters and the
   feature history), the view in the middle (3D, or a drawing sheet), properties on the right. Spare Construct's
   ribbon carries the tools: the frame builds Workshop's tabs from WorkshopFrame.tabs(), and every button runs a
   function here, so the page works the same from the ribbon as from its own panels. */

import { WAITS, KINDS, MATERIALS, kindOf, sample, newFeature } from './model.js';
import { buildPart, pose } from './build.js';
import { createView } from './view3d.js';
import { sheet } from './drawing.js';

const $ = (s, el = document) => el.querySelector(s);
const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const ic = (n, cls = '') => `<svg class="ic ${cls}" aria-hidden="true"><use href="#i-${n}"/></svg>`;
const n2 = (v) => (Math.round(v * 100) / 100).toLocaleString('en-NZ', { maximumFractionDigits: 2 });
const STORE = 'spare-construct-workshop-v1';
/* In the Spare Construct app (the server's page adds connect.js), the open model keeps the parts, through its edit
   log (see "the project's model" below); the demo keeps them in this browser. */
const IN_APP = (() => { try { return !!parent.document.querySelector('script[src$="connect.js"]'); } catch (e) { return false; } })();
const LINK = (() => { try { return (IN_APP && parent.spare && parent.spare.workshop) || null; } catch (e) { return null; } })();

/* Workshop's own icons, drawn to the family's rules (design/ribbon/README.md): a 24-unit grid, lines in currentColor,
   round ends, at most one accent area. Ours, drawn from the thing itself. */
export const ICONS = {
  'ws-part': '<path class="f" d="M4 9l8-4 8 4v8l-8 4-8-4z"/><path d="M4 9l8 4 8-4M12 13v8"/>',
  'ws-sketch': '<rect x="4" y="7" width="14" height="10" rx="1"/><path d="M16 4l4 4-8 8-5 1 1-5z" class="f"/>',
  'ws-rect': '<rect class="f" x="4" y="6" width="16" height="12" rx="1"/><circle cx="4" cy="6" r="1"/><circle cx="20" cy="18" r="1"/>',
  'ws-circle': '<circle class="f" cx="12" cy="12" r="8"/><path d="M12 12h8"/><circle cx="12" cy="12" r="1"/>',
  'ws-polygon': '<path class="f" d="M8 4h8l4 8-4 8H8l-4-8z"/>',
  'ws-outline': '<path class="f" d="M4 18l3-12 9 2 4 10z"/><circle cx="4" cy="18" r="1"/><circle cx="7" cy="6" r="1"/><circle cx="16" cy="8" r="1"/><circle cx="20" cy="18" r="1"/>',
  'ws-extrude': '<path d="M4 15l8 4 8-4-8-4z"/><path class="f" d="M4 15V9l8-4 8 4v6l-8-4z"/><path d="M12 11V5"/>',
  'ws-revolve': '<path d="M12 3v18"/><path class="f" d="M12 6c4 0 6 2 6 6s-2 6-6 6"/><path d="M6 9a6 3 0 0 0 0 6"/>',
  'ws-sweep': '<path d="M4 18c4 0 4-12 16-12"/><circle class="f" cx="4" cy="18" r="3"/>',
  'ws-fillet': '<path d="M4 20V10a6 6 0 0 1 6-6h10"/><path class="fo" d="M4 10a6 6 0 0 1 6-6v6z"/>',
  'ws-chamfer': '<path d="M4 20V10l6-6h10"/><path class="fo" d="M4 10l6-6v6z"/>',
  'ws-shell': '<path d="M3 7h18v12H3z"/><path class="f" d="M6 7v9h12V7"/>',
  'ws-hole': '<rect x="3" y="6" width="18" height="12" rx="1"/><circle class="f" cx="12" cy="12" r="3"/>',
  'ws-join': '<rect x="3" y="6" width="11" height="10" rx="1"/><rect class="f" x="10" y="9" width="11" height="10" rx="1"/>',
  'ws-cut': '<rect x="3" y="6" width="11" height="10" rx="1"/><rect x="10" y="9" width="11" height="10" rx="1" stroke-dasharray="2 2"/>',
  'ws-intersect': '<rect x="3" y="6" width="11" height="10" rx="1"/><rect x="10" y="9" width="11" height="10" rx="1"/><rect class="fo" x="10" y="9" width="4" height="7"/>',
  'ws-pattern': '<rect class="f" x="3" y="9" width="5" height="6" rx="1"/><rect x="10" y="9" width="5" height="6" rx="1"/><rect x="17" y="9" width="4" height="6" rx="1"/>',
  'ws-param': '<path d="M4 7h10M4 17h6"/><circle class="f" cx="17" cy="7" r="3"/><circle cx="13" cy="17" r="3"/>',
  'ws-constraint': '<path d="M5 19L19 5"/><path d="M5 5v14h14"/><path class="fo" d="M5 15h4v4H5z"/>',
  'ws-onface': '<path d="M3 15l9-5 9 5-9 5z" class="f"/><path d="M9 14l3-2 3 2-3 2z"/>',
  'ws-history': '<path d="M5 5h14M5 10h14M5 15h9"/><path class="f" d="M3 19h18v2H3z"/>',
  'ws-rollback': '<path d="M5 6h14M5 11h9"/><path d="M3 16h18"/><path d="M12 13v7M9 17l3 3 3-3"/>',
  'ws-joint': '<circle class="f" cx="12" cy="12" r="4"/><path d="M3 12h5M16 12h5M12 3v5"/>',
  'ws-hinge': '<path d="M12 4v16"/><path class="f" d="M12 7l8 3v4l-8 3z"/><path d="M12 7L4 10v4l8 3"/>',
  'ws-slider': '<path d="M3 15h18"/><rect class="f" x="8" y="8" width="8" height="7" rx="1"/><path d="M5 11l-2 2 2 2M19 11l2 2-2 2"/>',
  'ws-fixed': '<rect x="4" y="4" width="8" height="8" rx="1"/><rect class="f" x="12" y="12" width="8" height="8" rx="1"/><path d="M10 14l4-4"/>',
  'ws-drive': '<circle cx="12" cy="12" r="8"/><path class="fo" d="M10 8l6 4-6 4z"/>',
  'ws-insert': '<path class="f" d="M4 10l6-3 6 3v6l-6 3-6-3z"/><path d="M18 3v6M15 6h6"/>',
  'ws-drawing': '<rect x="3" y="4" width="18" height="16" rx="1"/><path class="f" d="M6 8h5v5H6z"/><path d="M14 8h4M14 11h4M13 17h6"/>',
  'ws-mass': '<path class="f" d="M7 8h10l3 12H4z"/><circle cx="12" cy="5" r="2"/>',
};

const S = {
  project: null, part: 'bracket', mode: 'part', drawOf: 'part', sel: null, undo: [], redo: [], dims: true, list: true,
  driving: null, toast: '',
  linked: false, draft: false, kept: new Map(), // in the app: the model's parts, and what it holds of each
};
let builder = buildPart, engine = { id: 'prototype', label: 'Prototype', tip: "Built by the prototype's own geometry, which does only what the geometry kernel does today." };
const subs = new Set();
const notify = () => { for (const f of subs) { try { f(); } catch (e) { console.error(e); } } };

function load() {
  try { const s = localStorage.getItem(STORE); if (s) { const p = JSON.parse(s); if (p && Array.isArray(p.parts)) return p; } } catch (e) { /* storage is a convenience */ }
  return sample();
}
function save() {
  if (S.linked) { queueSync(); return; }
  try { localStorage.setItem(STORE, JSON.stringify(S.project)); } catch (e) { /* the page works without it */ }
}
const part = () => S.project.parts.find((p) => p.id === S.part) || S.project.parts[0];
const cache = new Map();
function built(p) {
  const key = JSON.stringify(p);
  if (!cache.has(p.id) || cache.get(p.id).key !== key) {
    let out;
    try { out = builder(p); } catch (e) { console.warn('Workshop: the builder refused this recipe, so the prototype built it.', e); out = buildPart(p); }
    cache.set(p.id, { key, out });
  }
  return cache.get(p.id).out;
}

function edit(label, fn) {
  if (S.linked && !LINK.state().editable) { say(LOOK_ONLY); return; }
  S.undo.push({ label, snap: JSON.stringify(S.project) });
  if (S.undo.length > 100) S.undo.shift();
  S.redo.length = 0;
  fn();
  save();
  render();
}
function undo() { const u = S.undo.pop(); if (!u) return; S.redo.push({ label: u.label, snap: JSON.stringify(S.project) }); S.project = JSON.parse(u.snap); save(); say('Undone: ' + u.label); render(); }
function redo() { const u = S.redo.pop(); if (!u) return; S.undo.push({ label: u.label, snap: JSON.stringify(S.project) }); S.project = JSON.parse(u.snap); save(); say('Redone: ' + u.label); render(); }
function say(t) { S.toast = t; const el = $('#toast'); el.textContent = t; el.hidden = !t; clearTimeout(say.h); say.h = setTimeout(() => { el.hidden = true; }, 4200); }

/* ---------- the project's model ----------
   In the Spare Construct app the open model keeps the parts and the assembly, as elements written through its edit log
   like walls and doors (construct-model's FORMAT.md, "Workshop's elements"; connect.js's spare.workshop). Each change
   here becomes element.create, element.set or element.delete for what it changed, so a part survives a reload and
   everyone on the model sees it; other people's changes redraw the page. A number with a fraction is kept as its
   decimal text, so no hash depends on how a number prints. Outside the app, in the demo, parts stay in this browser. */
const LOOK_ONLY = 'This model is open to look at, so changes here are not kept.';
const TEXT = new Set(['id', 'type', 'name', 'note', 'shape', 'sketch', 'of', 'plane', 'op', 'material', 'part', 'a', 'b', 'axis', 'unit', 'kind']);
const DECIMAL = /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i;
/* A parameter's kind and the unit its values are shown in. */
const UNIT = { length: 'mm', angle: '°', count: '' };
/* A parameter's name where it is used in a value: the whole word, and not a unit after a number (2 m). */
const nameRe = (name) => new RegExp(`(?<![0-9.]\\s*)\\b${name.replace(/[^\w]/g, '')}\\b`, 'g');
/* Whether a feature or a parameter uses a parameter, by name or in a formula. */
function uses(o, name) {
  const re = nameRe(name);
  const any = (v, k) => typeof v === 'string' ? !TEXT.has(k) && (re.lastIndex = 0, re.test(v)) : Array.isArray(v) ? v.some((x) => any(x, k)) : false;
  return Object.entries(o).some(([k, v]) => k !== 'name' && any(v, k));
}
/* Renames a parameter wherever a value uses it. */
function renameIn(o, old, nu) {
  const re = nameRe(old);
  const swap = (v, k) => typeof v === 'string' ? (TEXT.has(k) ? v : v.replace(re, nu)) : Array.isArray(v) ? v.map((x) => swap(x, k)) : v;
  for (const k of Object.keys(o)) if (k !== 'name') o[k] = swap(o[k], k);
}
function encode(v) {
  if (typeof v === 'number') return Number.isSafeInteger(v) ? v : String(v);
  if (Array.isArray(v)) return v.map(encode);
  if (v && typeof v === 'object') { const o = {}; for (const k of Object.keys(v).sort()) if (v[k] !== undefined) o[k] = encode(v[k]); return o; }
  return v;
}
function decode(v, key) {
  if (typeof v === 'string') return !TEXT.has(key) && DECIMAL.test(v) ? Number(v) : v;
  if (Array.isArray(v)) return v.map((x) => decode(x, key));
  if (v && typeof v === 'object') { const o = {}; for (const k of Object.keys(v)) o[k] = decode(v[k], k); return o; }
  return v;
}
const elementId = (tag) => `${tag}-${crypto.randomUUID().slice(0, 8)}`;
const recipeOf = (op, args) => ({ owner: 'workshop', schema: 1, op, op_version: 1, args });
/* What the model should hold: each part, and the assembly once it has an id. */
function wanted() {
  const out = new Map();
  for (const p of S.project.parts) out.set(p.id, { kind: 'workshop.part', op: 'part', name: p.name, args: encode({ params: p.params, features: p.features, end: p.end, material: p.material }) });
  const a = S.project.assembly;
  if (a && a.id) out.set(a.id, { kind: 'workshop.assembly', op: 'assembly', name: a.name, args: encode({ instances: a.instances, joints: a.joints }) });
  return out;
}
const keyOf = (w) => JSON.stringify([w.kind, w.name, w.args]);
function fromModel() {
  const st = LINK.state(), els = LINK.elements();
  const name = st.model ? st.model.name : 'A fixed version';
  const parts = els.filter((e) => e.kind === 'workshop.part').map((e) => ({ ...decode(e.recipe.args), id: e.id, name: e.name }))
    .sort((x, y) => x.name.localeCompare(y.name) || (x.id < y.id ? -1 : 1));
  const asm = els.find((e) => e.kind === 'workshop.assembly');
  S.kept = new Map();
  if (!parts.length) { S.project = sample(); S.project.name = name; S.draft = true; return; }
  S.draft = false;
  S.project = { name, units: 'mm', parts, assembly: asm ? { ...decode(asm.recipe.args), id: asm.id, name: asm.name } : { name, instances: [], joints: [] } };
  for (const [id, w] of wanted()) S.kept.set(id, keyOf(w));
}
/* The sample becomes the model's at the first change: its parts get element ids. */
function adopt() {
  const ids = new Map();
  for (const p of S.project.parts) { const id = elementId('WP'); ids.set(p.id, id); if (S.part === p.id) S.part = id; p.id = id; }
  const a = S.project.assembly;
  a.id = elementId('WA');
  for (const i of a.instances) i.part = ids.get(i.part) || i.part;
  S.draft = false;
  S.undo.length = 0;
  S.redo.length = 0;
}
let syncTimer = 0;
function queueSync() { clearTimeout(syncTimer); syncTimer = setTimeout(sync, 150); }
function sync() {
  clearTimeout(syncTimer);
  syncTimer = 0;
  if (!S.linked || !LINK.state().editable) return;
  if (S.draft) adopt();
  const a = S.project.assembly;
  if (a && !a.id && a.instances.length) a.id = elementId('WA');
  const want = wanted();
  for (const [id, w] of want) {
    const key = keyOf(w);
    if (S.kept.get(id) === key) continue;
    if (S.kept.has(id)) LINK.set(id, w.name, recipeOf(w.op, w.args)); else LINK.create(w.kind, id, w.name, recipeOf(w.op, w.args));
    S.kept.set(id, key);
  }
  for (const id of [...S.kept.keys()]) if (!want.has(id)) { LINK.remove(id); S.kept.delete(id); }
}
/* The model changed: another person's edit, a refused edit of this page's, or another model opened. */
function linkChanged(why) {
  const st = LINK.state();
  if (!st.loaded) return;
  if (syncTimer) sync();
  if (!st.model && !st.fixed) { S.linked = false; S.project = load(); render(); return; }
  S.linked = true;
  fromModel();
  // an undo from before someone else's change would put their change back, so the history starts here
  S.undo.length = 0;
  S.redo.length = 0;
  if (!S.project.parts.some((p) => p.id === S.part)) { S.part = S.project.parts[0].id; S.sel = null; }
  render();
  if (why && why.turnedBack) say('Not kept: ' + why.turnedBack);
}
function linkNote() {
  if (!IN_APP) return '';
  if (!S.linked) return '<div class="note wait">Open a model in your team’s WIP to keep parts in it, for everyone on it. Until then they stay in this browser.</div>';
  const st = LINK.state();
  if (!st.editable) return `<div class="note">${st.fixed ? 'A fixed version' : esc(S.project.name) + ' is open to look at'}: you can look, not change.</div>`;
  if (S.draft) return `<div class="note wait">${esc(S.project.name)} has no parts yet, so this is a sample. Your first change keeps it in the model.</div>`;
  return `<div class="note ok">Kept in ${esc(S.project.name)}, for everyone on it: each change goes through the model’s edit log.</div>`;
}
function newPart() {
  edit('Add a part', () => {
    let k = S.project.parts.length + 1;
    while (S.project.parts.some((p) => p.name === 'Part ' + k)) k++;
    const id = S.linked && !S.draft ? elementId('WP') : 'part' + k;
    const p = { id, name: 'Part ' + k, material: 'steel', params: [{ name: 'size', value: 40, note: '' }], features: [], end: 0 };
    p.features.push(newFeature(p, 'sketch', { shape: 'rect' }));
    p.end = 1;
    S.project.parts.push(p);
    S.part = id;
    S.sel = null;
    if (S.mode !== 'drawing') S.mode = 'part';
  });
  say('A new part, with a rectangle to start from. Extrude it, or change it on the right.');
}

/* ---------- tools ---------- */
function insertFeature(f) {
  const p = part();
  const at = Math.min(p.end ?? p.features.length, p.features.length);
  p.features.splice(at, 0, f);
  p.end = at + 1;
  S.sel = { kind: 'feature', id: f.id };
}
function lastWith(test) {
  const p = part(), out = built(p);
  if (S.sel && S.sel.kind === 'feature') {
    const i = p.features.findIndex((f) => f.id === S.sel.id);
    if (i >= 0 && test(p.features[i], out.features[i])) return p.features[i];
  }
  for (let i = Math.min(p.end ?? p.features.length, p.features.length) - 1; i >= 0; i--) if (test(p.features[i], out.features[i])) return p.features[i];
  return null;
}
function tool(type, opts = {}) {
  setMode('part');
  const p = part();
  if (type === 'sketch') {
    const out = built(p), z = out.bbox ? Math.round(out.bbox[1][2] * 1000) / 1000 : 0;
    edit('Add a sketch', () => insertFeature(newFeature(p, 'sketch', { shape: opts.shape, z })));
    say('Sketch added at the top of the part. Set its sizes on the right; a parameter name works in any field.');
    return;
  }
  if (type === 'extrude') {
    const used = new Set(p.features.filter((f) => f.type === 'extrude').map((f) => f.sketch));
    const sk = lastWith((f, r) => f.type === 'sketch' && r && r.state === 'built' && (!used.has(f.id) || (S.sel && S.sel.id === f.id)));
    if (!sk) { say('Draw a sketch first, then extrude it.'); return; }
    edit('Extrude ' + sk.name, () => insertFeature(newFeature(p, 'extrude', { of: sk.id })));
    return;
  }
  const src = lastWith((f, r) => r && r.state === 'built' && r.bodies && r.bodies.length);
  if (!src) { say('Build a body first, then ' + type + ' it.'); return; }
  edit(KINDS[type].label + ' ' + src.name, () => insertFeature(newFeature(p, type, { of: src.id })));
}
function addParam() {
  const p = part();
  let k = p.params.length + 1;
  while (p.params.some((x) => x.name === 'size_' + k)) k++;
  edit('Add a parameter', () => { p.params.push({ name: 'size_' + k, value: 10, note: '' }); S.sel = { kind: 'param', id: 'size_' + k }; });
}
function rollTo(n) {
  const p = part();
  n = Math.max(0, Math.min(p.features.length, n));
  if (n === (p.end ?? p.features.length)) return;
  edit(n < (p.end ?? p.features.length) ? 'Roll back' : 'Roll forward', () => { p.end = n; });
}
function setMode(m) {
  if (S.mode === m) return;
  S.mode = m;
  if (m !== 'assembly') stopDrive();
  if (m === 'drawing' && S.sel && S.sel.kind === 'instance') S.drawOf = 'assembly';
  render();
}
function addInstance(pid) {
  setMode('assembly');
  const a = S.project.assembly, pt = S.project.parts.find((p) => p.id === pid);
  let k = a.instances.length + 1;
  while (a.instances.some((i) => i.id === 'i' + k)) k++;
  edit('Insert ' + pt.name, () => { a.instances.push({ id: 'i' + k, part: pid, name: pt.name, at: [0, 0, 80] }); S.sel = { kind: 'instance', id: 'i' + k }; });
  say(pt.name + ' inserted above the assembly. Join it to a part with a hinge, a slider or a fixed joint.');
}
function addJoint(type) {
  setMode('assembly');
  const a = S.project.assembly;
  const ground = a.instances.find((i) => i.grounded);
  const b = S.sel && S.sel.kind === 'instance' ? a.instances.find((i) => i.id === S.sel.id && !i.grounded) : a.instances.find((i) => !i.grounded && !a.joints.some((j) => j.b === i.id));
  if (!ground || !b) { say('Select a part that is not grounded, then add a joint to it.'); return; }
  if (a.joints.some((j) => j.b === b.id)) { say(b.name + ' already has a joint. Select it to change it.'); return; }
  let k = a.joints.length + 1;
  while (a.joints.some((j) => j.id === 'j' + k)) k++;
  const label = { hinge: 'Hinge', slider: 'Slider', fixed: 'Fixed' }[type];
  edit('Add a ' + label.toLowerCase() + ' joint', () => {
    a.joints.push({ id: 'j' + k, name: label + ' ' + k, type, a: ground.id, b: b.id, origin: (b.at || [0, 0, 0]).slice(), axis: 'z',
      min: type === 'hinge' ? -90 : 0, max: type === 'hinge' ? 90 : 50, value: 0, unit: type === 'hinge' ? 'deg' : 'mm' });
    b.at = [0, 0, 0];
    S.sel = { kind: 'joint', id: 'j' + k };
  });
}
function drive(id) {
  const j = S.project.assembly.joints.find((x) => x.id === id);
  if (!j || j.type === 'fixed') return;
  if (S.driving) { stopDrive(); render(); return; }
  const t0 = performance.now(), start = Number(j.value) || 0, span = j.max - j.min || 1;
  S.driving = { id, frame: 0 };
  const stepFn = (t) => {
    if (!S.driving) return;
    const u = ((t - t0) / 4000) % 1;   // four seconds through the range and back
    const v = j.min + span * (u < 0.5 ? u * 2 : 2 - u * 2);
    j.value = Math.round(v * 10) / 10;
    renderView();
    const r = $('#right [data-chg="jv"]');
    if (r && S.sel && S.sel.id === id) { r.value = j.value; r.closest('.fld').querySelector('span').textContent = `Position: ${n2(j.value)}${j.unit === 'deg' ? '°' : ' mm'}`; }
    S.driving.frame = requestAnimationFrame(stepFn);
  };
  void start;
  S.driving.frame = requestAnimationFrame(stepFn);
  render();
}
function stopDrive() { if (S.driving) { cancelAnimationFrame(S.driving.frame); S.driving = null; save(); } }

/* ---------- the panels ---------- */
function stateBadge(r) {
  if (!r) return '';
  if (r.state === 'waits') return '<span class="badge wait">Waits for the kernel</span>';
  if (r.state === 'failed') return '<span class="badge bad">Fails</span>';
  if (r.state === 'suppressed') return '<span class="badge n">Suppressed</span>';
  return '';
}
function renderLeft() {
  const p = part(), out = built(p), end = Math.min(p.end ?? p.features.length, p.features.length);
  let h = `<div class="seg" role="group" aria-label="Workspace">${[['part', 'Part'], ['assembly', 'Assembly'], ['drawing', 'Drawing']].map(([k, l]) => `<button data-act="mode" data-a="${k}" aria-pressed="${S.mode === k}">${l}</button>`).join('')}</div>`;
  h += `<div class="sec"><div class="lbl">Parts <button class="mini" data-act="newpart" title="Add a part">${ic('plus', 'sm')}<span class="sr">Add a part</span></button></div><ul class="list">${S.project.parts.map((x) => {
    const o = built(x), mat = MATERIALS[x.material] || MATERIALS.steel;
    return `<li><button class="leaf${x.id === p.id ? ' cur' : ''}" data-act="part" data-a="${esc(x.id)}">${ic('ws-part', 'sm')}<span>${esc(x.name)}</span><span class="n">${n2((o.volume * mat.density) / 1e6)} g</span></button></li>`;
  }).join('')}</ul></div>`;
  if (S.mode === 'assembly' || (S.mode === 'drawing' && S.drawOf === 'assembly')) {
    const a = S.project.assembly;
    h += `<div class="sec"><div class="lbl">Assembly: ${esc(a.name)}</div><ul class="list">${a.instances.map((i) => `<li><button class="leaf${S.sel && S.sel.id === i.id ? ' cur' : ''}" data-act="sel" data-k="instance" data-a="${esc(i.id)}">${ic(i.grounded ? 'pin' : 'ws-part', 'sm')}<span>${esc(i.name)}</span><span class="n">${i.grounded ? 'Grounded' : ''}</span></button></li>`).join('')}</ul>`;
    h += `<div class="lbl sub">Joints</div><ul class="list">${a.joints.map((j) => `<li><button class="leaf${S.sel && S.sel.id === j.id ? ' cur' : ''}" data-act="sel" data-k="joint" data-a="${esc(j.id)}">${ic('ws-' + j.type, 'sm')}<span>${esc(j.name)}</span><span class="n">${j.type === 'fixed' ? '' : n2(j.value) + (j.unit === 'deg' ? '°' : ' mm')}</span></button></li>`).join('') || '<li class="faint pad">No joints yet.</li>'}</ul></div>`;
  }
  h += `<div class="sec"><div class="lbl">Parameters of ${esc(p.name)} <button class="mini" data-act="addparam" title="Add a parameter">${ic('plus', 'sm')}<span class="sr">Add a parameter</span></button></div><table class="params"><tbody>${p.params.map((x, i) => {
    const r = out.params[i] || {};
    return `<tr class="${S.sel && S.sel.kind === 'param' && S.sel.id === x.name ? 'cur' : ''}${r.state === 'failed' ? ' bad' : ''}"><td><button class="pname" data-act="sel" data-k="param" data-a="${esc(x.name)}" title="${esc(x.note || r.message || '')}">${esc(x.name)}</button></td><td><input aria-label="${esc(x.name)}" data-chg="param" data-a="${i}" value="${esc(x.value)}" spellcheck="false"></td><td class="u">${r.formula && r.value != null ? '= ' + n2(r.value) + ' ' : ''}${UNIT[x.kind] ?? 'mm'}</td></tr>`;
  }).join('')}</tbody></table></div>`;
  h += `<div class="sec"><div class="lbl">History of ${esc(p.name)}</div><ol class="hist">`;
  p.features.forEach((f, i) => {
    const r = out.features[i], k = kindOf(f), cur = S.sel && S.sel.kind === 'feature' && S.sel.id === f.id;
    h += `<li class="${r ? r.state : ''}${cur ? ' cur' : ''}"><button data-act="sel" data-k="feature" data-a="${esc(f.id)}" title="${esc(r ? r.message : '')}">${ic(k.icon, 'sm')}<span class="nm">${esc(f.name)}</span>${stateBadge(r)}</button></li>`;
    if (i === end - 1) h += endMarker(end, p.features.length);
  });
  if (end === 0) h = h.replace('<ol class="hist">', '<ol class="hist">' + endMarker(0, p.features.length));
  h += '</ol></div>';
  $('#left').innerHTML = h;
}
const endMarker = (end, n) => `<li class="endm" aria-label="End of the history"><span>End of history</span><button class="mini" data-act="roll" data-a="${end - 1}" ${end ? '' : 'disabled'} title="Roll back one feature">▲</button><button class="mini" data-act="roll" data-a="${end + 1}" ${end < n ? '' : 'disabled'} title="Roll forward one feature">▼</button></li>`;

const field = (label, key, v, f, note = '') => `<label class="fld"><span>${label}</span><input data-chg="f" data-a="${key}" value="${esc(v == null ? '' : v)}" list="pnames" spellcheck="false"${note ? ` title="${esc(note)}"` : ''}></label>`;
const pick = (label, key, v, opts) => `<label class="fld"><span>${label}</span><select data-chg="f" data-a="${key}">${opts.map(([k, l, dis]) => `<option value="${esc(k)}"${k === v ? ' selected' : ''}${dis ? ' disabled' : ''}>${esc(l)}</option>`).join('')}</select></label>`;

function profileSvg(f, r) {
  // the sketch drawn from the built profile, or nothing if it did not build
  if (!r || r.state !== 'built') return '';
  let one;
  try { one = builder({ params: part().params, features: [f, { id: 'x', type: 'extrude', sketch: f.id, depth: 1, op: 'new' }], end: 2 }); } catch (e) { return ''; }
  const body = one.bodies[0];
  let pts;
  if (body && body.kind === 'prism') pts = body.outline;
  else if (body) { pts = []; for (let i = 0; i < 48; i++) { const a = (i / 48) * 2 * Math.PI; pts.push([body.center[0] + body.radius * Math.cos(a), body.center[1] + body.radius * Math.sin(a)]); } }
  else if (Array.isArray(f.pts)) pts = f.pts.map((p) => [Number(p[0]) || 0, Number(p[1]) || 0]);
  if (!pts) return '';
  const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
  const x0 = Math.min(...xs), x1 = Math.max(...xs), y0 = Math.min(...ys), y1 = Math.max(...ys), pad = Math.max(x1 - x0, y1 - y0) * 0.12 + 1;
  const ok = r && r.convex;
  return `<svg class="skv${ok ? '' : ' bad'}" viewBox="${x0 - pad} ${-y1 - pad} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}" role="img" aria-label="The profile, seen from above"${f.shape === 'outline' ? ' data-act="addpt"' : ''}><path d="M${pts.map((p) => `${p[0]} ${-p[1]}`).join('L')}Z"/>${f.shape === 'outline' || f.shape === 'polygon' || f.shape === 'rect' ? pts.map((p, i) => `<circle cx="${p[0]}" cy="${-p[1]}" r="${pad * 0.12}"><title>Corner ${i + 1}: ${n2(p[0])}, ${n2(p[1])}</title></circle>`).join('') : ''}</svg>`;
}

function renderSide() {
  const p = part(), out = built(p);
  let h = '';
  const sel = S.sel;
  if (sel && sel.kind === 'feature' && p.features.some((f) => f.id === sel.id)) {
    const i = p.features.findIndex((f) => f.id === sel.id), f = p.features[i], r = out.features[i], k = kindOf(f);
    const before = p.features.slice(0, i);
    h += `<div class="sec"><div class="lbl">${esc(k.label)}</div>${field('Name', 'name', f.name, f)}`;
    if (f.type === 'sketch') {
      h += pick('Shape', 'shape', f.shape, [['rect', 'Rectangle'], ['circle', 'Circle'], ['polygon', 'Regular polygon'], ['outline', 'Outline, corner by corner']]);
      h += field('Height of the sketch plane', 'z', f.z, f) + `<div class="two">${field('Centre x', 'cx', f.cx, f)}${field('Centre y', 'cy', f.cy, f)}</div>`;
      if (f.shape === 'rect') h += `<div class="two">${field('Width', 'w', f.w, f)}${field('Height', 'h', f.h, f)}</div>`;
      if (f.shape === 'circle') h += field('Radius', 'r', f.r, f);
      if (f.shape === 'polygon') h += `<div class="two">${field('Sides', 'n', f.n, f)}${field('Radius to the corners', 'r', f.r, f)}</div>`;
      if (f.shape === 'outline') {
        h += `<div class="lbl sub">Corners, from the centre</div><table class="pts"><tbody>${(f.pts || []).map((q, j) => `<tr><td class="u">${j + 1}</td><td><input aria-label="Corner ${j + 1} x" data-chg="pt" data-a="${j}" data-c="0" value="${esc(q[0])}" list="pnames"></td><td><input aria-label="Corner ${j + 1} y" data-chg="pt" data-a="${j}" data-c="1" value="${esc(q[1])}" list="pnames"></td><td><button class="mini" data-act="delpt" data-a="${j}" title="Remove corner ${j + 1}">${ic('x', 'sm')}</button></td></tr>`).join('')}</tbody></table><p class="small faint">Click in the sketch below to add a corner after the last.</p>`;
      }
      h += profileSvg(f, r);
      h += `<button class="btn" data-act="tool" data-a="extrude"${r && r.state === 'built' ? '' : ' disabled'}>${ic('ws-extrude', 'sm')}Extrude this sketch</button>`;
      h += `<p class="small faint mt">${ic('info', 'sm')} Constraints between corners and edges wait for ${esc(WAITS.constraints.what)}; until then each size is typed in or named.</p>`;
    } else if (f.type === 'extrude') {
      h += pick('Sketch', 'sketch', f.sketch, [['', 'Pick a sketch'], ...before.filter((g) => g.type === 'sketch').map((g) => [g.id, g.name])]);
      h += field('Distance', 'depth', f.depth, f);
      h += pick('Result', 'op', f.op || 'new', [['new', 'A new body'], ['join', 'Join to the part (waits for the kernel)'], ['cut', 'Cut from the part (waits for the kernel)'], ['intersect', 'Keep the overlap (waits for the kernel)']]);
    } else if (f.type === 'mirror' || f.type === 'pattern' || f.type === 'move') {
      const srcs = before.filter((g, j) => out.features[j] && out.features[j].bodies && out.features[j].bodies.length || ['extrude', 'mirror', 'pattern', 'move'].includes(g.type));
      h += pick(f.type === 'move' ? 'Move the bodies of' : 'Copy the bodies of', 'of', f.of, [['', 'Pick a feature'], ...srcs.map((g) => [g.id, g.name])]);
      if (f.type === 'mirror') h += `<div class="two">${pick('Across the plane', 'plane', f.plane, [['x', 'x = a value'], ['y', 'y = a value'], ['z', 'z = a value']])}${field('Value', 'at', f.at, f)}</div>`;
      if (f.type === 'pattern') {
        h += `${field('How many in the row', 'count', f.count, f)}<div class="three">${field('Step x', 'dx', f.dx, f)}${field('Step y', 'dy', f.dy, f)}${field('Step z', 'dz', f.dz, f)}</div>`;
        h += `${field('How many rows', 'count2', f.count2 == null ? 1 : f.count2, f)}<div class="three">${field('Row step x', 'dx2', f.dx2 ?? 0, f)}${field('Row step y', 'dy2', f.dy2 ?? 0, f)}${field('Row step z', 'dz2', f.dz2 ?? 0, f)}</div>`;
        h += `<p class="small faint">${ic('info', 'sm')} A pattern round an axis waits for ${esc(WAITS.turn.what)}.</p>`;
      }
      if (f.type === 'move') h += `<div class="three">${field('By x', 'dx', f.dx, f)}${field('By y', 'dy', f.dy, f)}${field('By z', 'dz', f.dz, f)}</div>`;
    } else {
      h += `<label class="fld"><span>What it will do</span><textarea data-chg="f" data-a="note" rows="2">${esc(f.note || '')}</textarea></label>`;
    }
    h += '</div>';
    const cls = r.state === 'waits' ? 'wait' : r.state === 'failed' ? 'bad' : r.state === 'built' ? 'ok' : 'n';
    h += `<div class="sec"><div class="lbl">Result</div><div class="note ${cls}">${esc(r.message || { built: 'Built.', rolled_back: 'After the end of the history.' }[r.state] || '')}</div>`;
    const bodies = out.bodies.filter((b) => r.bodies.includes(b.id));
    if (bodies.length) h += `<table class="props"><tbody>${bodies.slice(0, 8).map((b) => `<tr><th>${b.kind === 'prism' ? 'Prism' : 'Cylinder'}</th><td>${b.faces} faces · ${esc(b.exactness)} · ${n2(b.volume)} mm³</td></tr>`).join('')}${bodies.length > 8 ? `<tr><th></th><td>and ${bodies.length - 8} more</td></tr>` : ''}</tbody></table>`;
    if (KINDS[f.type] && KINDS[f.type].kernel) h += `<p class="small faint">In the kernel: ${esc(KINDS[f.type].kernel)}.</p>`;
    h += `</div><div class="sec acts"><button class="btn" data-act="roll" data-a="${i + 1}">${ic('ws-rollback', 'sm')}Roll back to here</button><button class="btn" data-act="suppress">${f.suppressed ? 'Unsuppress' : 'Suppress'}</button>
      <button class="btn" data-act="mv" data-a="-1"${i ? '' : ' disabled'}>Earlier</button><button class="btn" data-act="mv" data-a="1"${i < p.features.length - 1 ? '' : ' disabled'}>Later</button><button class="btn" data-act="delf">${ic('del', 'sm')}Delete</button></div>`;
  } else if (sel && sel.kind === 'param' && p.params.some((x) => x.name === sel.id)) {
    const i = p.params.findIndex((x) => x.name === sel.id), x = p.params[i], r = out.params[i];
    const users = p.features.filter((f) => uses(f, x.name)), formulas = p.params.filter((y) => y !== x && uses(y, x.name));
    const kind = x.kind || 'length';
    h += `<div class="sec"><div class="lbl">Parameter</div><label class="fld"><span>Name</span><input data-chg="pname" data-a="${i}" value="${esc(x.name)}" spellcheck="false"></label>
      <label class="fld"><span>What it is</span><select data-chg="pkind" data-a="${i}">${[['length', 'A length, in mm'], ['angle', 'An angle, in degrees'], ['count', 'A count']].map(([k, l]) => `<option value="${k}"${k === kind ? ' selected' : ''}>${l}</option>`).join('')}</select></label>
      <label class="fld"><span>Value: a number${UNIT[kind] ? ' in ' + UNIT[kind] : ''}, another parameter's name, or a formula</span><input data-chg="param" data-a="${i}" value="${esc(x.value)}" list="pnames" spellcheck="false"></label>
      ${r && r.state === 'built' && r.formula ? `<div class="note ok">Works out to ${n2(r.value)}${UNIT[kind] ? ' ' + UNIT[kind] : ''}.</div>` : ''}
      <label class="fld"><span>Note</span><input data-chg="pnote" data-a="${i}" value="${esc(x.note || '')}"></label>${r && r.state === 'failed' ? `<div class="note bad">${esc(r.message)}</div>` : ''}
      <p class="small faint">${ic('info', 'sm')} A formula uses other parameters in any order, with units: <span class="mono">width / 2 - 4 mm</span>, <span class="mono">2 ft 8 in</span>, <span class="mono">n &gt; 3 ? 12 mm : 8 mm</span>, and min, max, abs, round, floor and ceil. It is worked out exactly; a length and a plain number don't add, so write 5 mm, not 5.</p></div>
      <div class="sec"><div class="lbl">Used by</div>${users.length || formulas.length ? `<ul class="list">${users.map((f) => `<li><button class="leaf" data-act="sel" data-k="feature" data-a="${esc(f.id)}">${ic(kindOf(f).icon, 'sm')}<span>${esc(f.name)}</span></button></li>`).join('')}${formulas.map((y) => `<li><button class="leaf" data-act="sel" data-k="param" data-a="${esc(y.name)}">${ic('ws-param', 'sm')}<span>${esc(y.name)}</span></button></li>`).join('')}</ul>` : '<p class="pad faint">Nothing uses it yet.</p>'}</div>
      <div class="sec acts"><button class="btn" data-act="delp" data-a="${i}"${users.length || formulas.length ? ' disabled title="Something uses it"' : ''}>${ic('del', 'sm')}Delete</button></div>`;
  } else if (sel && sel.kind === 'joint' && S.project.assembly.joints.some((j) => j.id === sel.id)) {
    const a = S.project.assembly, j = a.joints.find((x) => x.id === sel.id), inst = a.instances;
    const u = j.unit === 'deg' ? '°' : ' mm';
    h += `<div class="sec"><div class="lbl">${esc({ hinge: 'Hinge joint', slider: 'Slider joint', fixed: 'Fixed joint' }[j.type])}</div>${jf('Name', 'name', j.name)}
      ${pick2('From', 'a', j.a, inst.map((x) => [x.id, x.name]))}${pick2('Moves', 'b', j.b, inst.filter((x) => !x.grounded).map((x) => [x.id, x.name]))}
      <div class="three">${jf('Origin x', 'o0', j.origin[0])}${jf('Origin y', 'o1', j.origin[1])}${jf('Origin z', 'o2', j.origin[2])}</div>
      ${j.type === 'fixed' ? '' : `${pick2(j.type === 'hinge' ? 'Turns about' : 'Slides along', 'axis', j.axis, [['x', 'x'], ['y', 'y'], ['z', 'z']])}
      <div class="two">${jf('Least' + (j.unit === 'deg' ? ', degrees' : ', mm'), 'min', j.min)}${jf('Greatest', 'max', j.max)}</div>
      <label class="fld"><span>Position: ${n2(j.value)}${u}</span><input type="range" data-chg="jv" min="${j.min}" max="${j.max}" step="${j.unit === 'deg' ? 1 : 0.5}" value="${Math.min(j.max, Math.max(j.min, j.value))}"></label>
      <button class="btn${S.driving ? ' on' : ''}" data-act="drive" data-a="${esc(j.id)}">${ic('ws-drive', 'sm')}${S.driving ? 'Stop' : 'Drive through its range'}</button>`}
      <p class="small faint mt">${ic('info', 'sm')} Here a joint's axis is typed in. Joining faces and axes picked on the parts waits for ${esc(WAITS.mates.what.split(' and ')[0])} and persistent face names.</p></div>
      <div class="sec acts"><button class="btn" data-act="delj">${ic('del', 'sm')}Delete</button></div>`;
  } else if (sel && sel.kind === 'instance' && S.project.assembly.instances.some((i) => i.id === sel.id)) {
    const a = S.project.assembly, i = a.instances.find((x) => x.id === sel.id), pt = S.project.parts.find((x) => x.id === i.part), joined = a.joints.find((j) => j.b === i.id);
    h += `<div class="sec"><div class="lbl">Part in the assembly</div><label class="fld"><span>Name</span><input data-chg="iname" value="${esc(i.name)}"></label><p>${esc(pt ? pt.name : '')}${i.grounded ? ', grounded: it does not move.' : joined ? `, placed by the joint ${esc(joined.name)}.` : ', not joined to anything yet.'}</p>
      ${joined || i.grounded ? '' : `<div class="three">${['x', 'y', 'z'].map((ax, k) => `<label class="fld"><span>At ${ax}</span><input data-chg="iat" data-a="${k}" value="${esc((i.at || [0, 0, 0])[k])}"></label>`).join('')}</div>`}
      <button class="btn" data-act="part" data-a="${esc(i.part)}">${ic('ws-part', 'sm')}Edit ${esc(pt ? pt.name : 'the part')}</button></div>
      ${i.grounded ? '' : `<div class="sec acts"><button class="btn" data-act="deli">${ic('del', 'sm')}Remove from the assembly</button></div>`}`;
  } else {
    const mat = MATERIALS[p.material] || MATERIALS.steel, b = out.bbox;
    h += `<div class="sec"><div class="lbl">Part</div><label class="fld"><span>Name</span><input data-chg="partname" value="${esc(p.name)}"></label>
      ${`<label class="fld"><span>Material</span><select data-chg="material">${Object.entries(MATERIALS).map(([k, m]) => `<option value="${k}"${k === p.material ? ' selected' : ''}>${esc(m.label)}, ${m.density} kg/m³</option>`).join('')}</select></label>`}</div>
      <div class="sec"><div class="lbl">Mass properties</div><table class="props"><tbody><tr><th>Bodies</th><td>${out.bodies.length}</td></tr><tr><th>Volume</th><td>${n2(out.volume)} mm³</td></tr><tr><th>Mass</th><td>${n2((out.volume * mat.density) / 1e6)} g</td></tr>
      ${b ? `<tr><th>Size</th><td>${n2(b[1][0] - b[0][0])} × ${n2(b[1][1] - b[0][1])} × ${n2(b[1][2] - b[0][2])} mm</td></tr>` : ''}<tr><th>Built by</th><td>${esc(engine.label)}</td></tr></tbody></table>
      ${linkNote()}
      <p class="small faint">Volumes add body by body: joining bodies into one solid waits for ${esc(WAITS.join.what)}. Mass properties of curved bodies, certified, wait for G247.</p></div>
      <div class="sec"><div class="lbl">What waits for the kernel</div><ul class="waits">${Object.values(WAITS).map((w) => `<li>${ic(w.icon, 'sm')}<span><b>${esc(w.label)}</b>: ${esc(w.what)}.</span></li>`).join('')}</ul></div>`;
  }
  $('#right').innerHTML = h;
}
const jf = (label, key, v) => `<label class="fld"><span>${label}</span><input data-chg="j" data-a="${key}" value="${esc(v)}"></label>`;
const pick2 = (label, key, v, opts) => `<label class="fld"><span>${label}</span><select data-chg="j" data-a="${key}">${opts.map(([k, l]) => `<option value="${esc(k)}"${k === v ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`;

/* ---------- the view ---------- */
let view = null, shown = '';
function assemblyItems() {
  const a = S.project.assembly, { at, notes } = pose(a), items = [];
  a.instances.forEach((inst, n) => {
    const pt = S.project.parts.find((x) => x.id === inst.part);
    if (!pt) return;
    const out = built(pt);
    const order = [...new Set(a.instances.map((x) => x.part))].indexOf(inst.part) + 1;
    for (const b of out.bodies) items.push({ body: b, at: at.get(inst.id), tone: S.sel && S.sel.id === inst.id ? 'sel' : 'abc'[n % 3], key: inst.id, item: order });
  });
  return { items, notes };
}
function renderView() {
  const p = part(), draw = S.mode === 'drawing';
  $('#v3d').hidden = draw;
  $('#vdraw').hidden = !draw;
  let info = '';
  if (draw) {
    let items, title, list = null, material = '', mass = '';
    if (S.drawOf === 'assembly') {
      ({ items } = assemblyItems());
      title = S.project.assembly.name;
      const counts = new Map();
      for (const i of S.project.assembly.instances) counts.set(i.part, (counts.get(i.part) || 0) + 1);
      list = S.list ? [...counts].map(([pid, qty], k) => { const x = S.project.parts.find((y) => y.id === pid); return { item: k + 1, name: x.name, material: (MATERIALS[x.material] || MATERIALS.steel).label, qty }; }) : null;
    } else {
      const out = built(p), mat = MATERIALS[p.material] || MATERIALS.steel;
      items = out.bodies.map((b) => ({ body: b }));
      title = p.name;
      material = mat.label;
      mass = n2((out.volume * mat.density) / 1e6) + ' g';
    }
    $('#vdraw').innerHTML = items.length ? sheet(items, { title, project: S.project.name, material, mass, list, dims: S.dims }) : '<p class="pad faint">Nothing is built yet, so there is nothing to draw.</p>';
    info = `${S.drawOf === 'assembly' ? 'Assembly' : 'Part'} drawing, A3, third-angle projection.`;
  } else if (S.mode === 'assembly') {
    const { items, notes } = assemblyItems();
    if (view) view.show(items, { fit: shown !== 'assembly' });
    shown = 'assembly';
    info = `${S.project.assembly.instances.length} parts and ${S.project.assembly.joints.length} joints.${notes.length ? ' ' + notes.join(' ') : ''}`;
  } else {
    const out = built(p), f = S.sel && S.sel.kind === 'feature' ? S.sel.id : null;
    const mine = f ? new Set((out.features.find((r) => r.id === f) || { bodies: [] }).bodies) : new Set();
    if (view) view.show(out.bodies.map((b) => ({ body: b, tone: mine.has(b.id) ? 'sel' : 'body', key: b.by })), { fit: shown !== 'part:' + p.id });
    shown = 'part:' + p.id;
    const waiting = out.features.filter((r) => r.state === 'waits').length, failed = out.features.filter((r) => r.state === 'failed').length;
    info = `${out.bodies.length} bod${out.bodies.length === 1 ? 'y' : 'ies'}, ${n2(out.volume)} mm³.${waiting ? ` ${waiting} feature${waiting === 1 ? '' : 's'} wait${waiting === 1 ? 's' : ''} for the kernel.` : ''}${failed ? ` ${failed} fail${failed === 1 ? 's' : ''}.` : ''}`;
  }
  $('#vinfo').textContent = info;
  $('#engine').textContent = engine.label;
  $('#engine').title = engine.tip;
  $('#engine').dataset.engine = engine.id;
  $('#vtitle').textContent = draw ? 'Drawing' : S.mode === 'assembly' ? S.project.assembly.name : p.name;
}
function render() {
  renderLeft();
  renderSide();
  renderView();
  $('#pnames').innerHTML = part().params.map((x) => `<option value="${esc(x.name)}">`).join('');
  notify();
}

/* ---------- events ---------- */
const ACT = {
  mode: (b) => setMode(b.dataset.a),
  part: (b) => { S.part = b.dataset.a; S.sel = null; if (S.mode !== 'part') S.mode = S.mode === 'drawing' ? 'drawing' : 'part'; S.drawOf = 'part'; render(); },
  sel: (b) => { S.sel = { kind: b.dataset.k, id: b.dataset.a }; render(); },
  addparam: addParam,
  newpart: newPart,
  roll: (b) => rollTo(Number(b.dataset.a)),
  tool: (b) => tool(b.dataset.a),
  suppress: () => { const f = selFeature(); if (f) edit((f.suppressed ? 'Unsuppress ' : 'Suppress ') + f.name, () => { f.suppressed = !f.suppressed; }); },
  mv: (b) => {
    const p = part(), i = p.features.findIndex((f) => f.id === S.sel.id), j = i + Number(b.dataset.a);
    if (i < 0 || j < 0 || j >= p.features.length) return;
    edit('Reorder the history', () => { const [f] = p.features.splice(i, 1); p.features.splice(j, 0, f); });
  },
  delf: () => { const p = part(), f = selFeature(); if (!f) return; edit('Delete ' + f.name, () => { const i = p.features.indexOf(f); p.features.splice(i, 1); if ((p.end ?? 0) > i) p.end--; S.sel = null; }); },
  delp: (b) => { const p = part(), i = Number(b.dataset.a); edit('Delete a parameter', () => { p.params.splice(i, 1); S.sel = null; }); },
  delpt: (b) => { const f = selFeature(); if (f && f.pts.length > 3) edit('Remove a corner', () => f.pts.splice(Number(b.dataset.a), 1)); else say('An outline keeps at least three corners.'); },
  delj: () => { const a = S.project.assembly; stopDrive(); edit('Delete a joint', () => { a.joints = a.joints.filter((j) => j.id !== S.sel.id); S.sel = null; }); },
  deli: () => { const a = S.project.assembly; edit('Remove a part from the assembly', () => { a.instances = a.instances.filter((i) => i.id !== S.sel.id); a.joints = a.joints.filter((j) => j.a !== S.sel.id && j.b !== S.sel.id); S.sel = null; }); },
  drive: (b) => drive(b.dataset.a),
};
const selFeature = () => S.sel && S.sel.kind === 'feature' ? part().features.find((f) => f.id === S.sel.id) : null;
const num = (v) => { const t = String(v).trim(); return /^[-+]?\d*\.?\d+(e[-+]?\d+)?$/i.test(t) ? Number(t) : t; };
const CHG = {
  f: (el) => {
    const f = selFeature();
    if (!f) return;
    const k = el.dataset.a, v = ['name', 'note', 'shape', 'sketch', 'of', 'plane', 'op'].includes(k) ? el.value : num(el.value);
    edit('Change ' + f.name, () => {
      f[k] = v;
      if (k === 'shape') Object.assign(f, newFeature(part(), 'sketch', { shape: v }), { id: f.id, name: f.name, z: f.z, cx: f.cx, cy: f.cy });
    });
  },
  pt: (el) => { const f = selFeature(); if (f) edit('Move a corner', () => { f.pts[Number(el.dataset.a)][Number(el.dataset.c)] = num(el.value); }); },
  param: (el) => { const p = part(), x = p.params[Number(el.dataset.a)]; edit('Set ' + x.name, () => { x.value = num(el.value); }); },
  pname: (el) => {
    const p = part(), x = p.params[Number(el.dataset.a)], old = x.name, nu = el.value.trim();
    if (!nu || nu === old) return;
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(nu)) { say('A name is letters, digits and _, starting with a letter.'); render(); return; }
    edit('Rename ' + old, () => {
      for (const f of p.features) renameIn(f, old, nu);
      for (const y of p.params) if (y !== x) renameIn(y, old, nu);
      x.name = nu;
      S.sel = { kind: 'param', id: nu };
    });
  },
  pkind: (el) => { const p = part(), x = p.params[Number(el.dataset.a)]; edit('Set what ' + x.name + ' is', () => { if (el.value === 'length') delete x.kind; else x.kind = el.value; }); },
  pnote: (el) => { const p = part(), x = p.params[Number(el.dataset.a)]; edit('Note on ' + x.name, () => { x.note = el.value; }); },
  partname: (el) => { const p = part(); edit('Rename the part', () => { p.name = el.value; }); },
  material: (el) => { const p = part(); edit('Set the material', () => { p.material = el.value; }); },
  j: (el) => {
    const j = S.project.assembly.joints.find((x) => x.id === S.sel.id), k = el.dataset.a;
    edit('Change ' + j.name, () => {
      if (k[0] === 'o' && k.length === 2) j.origin[Number(k[1])] = Number(el.value) || 0;
      else if (k === 'min' || k === 'max') { j[k] = Number(el.value) || 0; if (j.min > j.max) [j.min, j.max] = [j.max, j.min]; }
      else j[k] = el.value;
    });
  },
  jv: (el) => { const j = S.project.assembly.joints.find((x) => x.id === S.sel.id); j.value = Number(el.value); save(); renderView(); renderLeft(); const l = el.closest('.fld').querySelector('span'); l.textContent = `Position: ${n2(j.value)}${j.unit === 'deg' ? '°' : ' mm'}`; },
  iname: (el) => { const i = S.project.assembly.instances.find((x) => x.id === S.sel.id); edit('Rename a part in the assembly', () => { i.name = el.value; }); },
  iat: (el) => { const i = S.project.assembly.instances.find((x) => x.id === S.sel.id); edit('Place ' + i.name, () => { i.at = (i.at || [0, 0, 0]).slice(); i.at[Number(el.dataset.a)] = Number(el.value) || 0; }); },
};
document.addEventListener('click', (e) => {
  const b = e.target.closest('[data-act]');
  if (!b || b.disabled) return;
  if (b.dataset.act === 'addpt') {
    const f = selFeature(), svg = b, r = svg.getBoundingClientRect(), vb = svg.viewBox.baseVal;
    const x = vb.x + ((e.clientX - r.left) / r.width) * vb.width, y = -(vb.y + ((e.clientY - r.top) / r.height) * vb.height);
    const cx = typeof f.cx === 'number' ? f.cx : 0, cy = typeof f.cy === 'number' ? f.cy : 0;
    edit('Add a corner', () => f.pts.push([Math.round((x - cx) * 10) / 10, Math.round((y - cy) * 10) / 10]));
    return;
  }
  if (ACT[b.dataset.act]) ACT[b.dataset.act](b);
});
document.addEventListener('change', (e) => { const el = e.target.closest('[data-chg]'); if (el && CHG[el.dataset.chg]) CHG[el.dataset.chg](el); });
document.addEventListener('input', (e) => { const el = e.target.closest('[data-chg="jv"]'); if (el) CHG.jv(el); });
document.addEventListener('keydown', (e) => {
  const typing = /INPUT|TEXTAREA|SELECT/.test((e.target.tagName || ''));
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z' && !typing) { e.preventDefault(); undo(); }
  else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y' && !typing) { e.preventDefault(); redo(); }
  else if (e.key === 'Delete' && !typing && selFeature()) ACT.delf();
  else if (e.key === 'Escape' && !typing) { S.sel = null; render(); }
});

/* ---------- Spare Construct's ribbon ---------- */
const W = (k) => ({ label: WAITS[k].label, icon: WAITS[k].icon, disabled: true, tip: `Waits for ${WAITS[k].what}.` });
function tabs() {
  const m = S.mode, st = (k) => ({ stack: k });
  const looks = [{ label: 'Fit', icon: 'fit', run: () => view3d('iso') }, ...['front', 'top', 'right'].map((s) => ({ label: s[0].toUpperCase() + s.slice(1), icon: 'cube', run: () => view3d(s) }))];
  const home = { id: 'ws-home', label: 'Part', panels: [
    { label: 'Sketch', items: [{ label: 'Rectangle', icon: 'ws-rect', run: () => tool('sketch', { shape: 'rect' }), tip: 'A rectangle sketch, sized by width and height.' },
      st([{ label: 'Circle', icon: 'ws-circle', run: () => tool('sketch', { shape: 'circle' }) }, { label: 'Polygon', icon: 'ws-polygon', run: () => tool('sketch', { shape: 'polygon' }) }, { label: 'Outline', icon: 'ws-outline', run: () => tool('sketch', { shape: 'outline' }), tip: 'An outline corner by corner. The kernel extrudes it today when it is convex.' }]),
      st([W('constraints'), W('onface')])] },
    { label: 'Create', items: [{ label: 'Extrude', icon: 'ws-extrude', run: () => tool('extrude'), tip: "Lift a sketch into a solid: the kernel's make.prism, or make.cylinder for a circle." }, st([W('revolve'), W('sweep')])] },
    { label: 'Modify', items: [st([W('fillet'), W('chamfer'), W('shell')]), st([W('hole'), W('join'), W('cut')])] },
    { label: 'Copy', items: [{ label: 'Pattern', icon: 'ws-pattern', run: () => tool('pattern'), tip: "Copies of a feature's bodies in a row or a grid: the kernel's transform.pattern." },
      st([{ label: 'Mirror', icon: 'mirror', run: () => tool('mirror'), tip: "Mirror images of a feature's bodies: the kernel's transform.mirror." }, { label: 'Move', icon: 'move', run: () => tool('move'), tip: "Move a feature's bodies: the kernel's transform.move." }, W('turn')])] },
    { label: 'Drive', items: [{ label: 'Parameter', icon: 'ws-param', run: addParam, tip: 'Add a named size. Any field takes a parameter name.' }, st([{ label: 'Roll back', icon: 'ws-rollback', run: () => rollTo((part().end ?? part().features.length) - 1), disabled: !(part().end ?? part().features.length) }, { label: 'Roll forward', icon: 'down', run: () => rollTo((part().end ?? part().features.length) + 1), disabled: (part().end ?? part().features.length) >= part().features.length }, { label: 'Part properties', icon: 'ws-mass', run: () => { S.sel = null; setMode('part'); render(); } }])] },
    { label: 'View', items: [st(looks.slice(0, 3)), st(looks.slice(3))] },
  ] };
  const asm = { id: 'ws-asm', label: 'Assembly', panels: [
    { label: 'Insert', items: [{ label: 'Insert part', icon: 'ws-insert', menu: S.project.parts.map((x) => ({ label: x.name, run: () => addInstance(x.id) })) }] },
    { label: 'Joints', items: [{ label: 'Hinge', icon: 'ws-hinge', run: () => addJoint('hinge'), tip: 'Turns about one axis, between limits.' }, { label: 'Slider', icon: 'ws-slider', run: () => addJoint('slider'), tip: 'Slides along one axis, between limits.' },
      st([{ label: 'Fixed', icon: 'ws-fixed', run: () => addJoint('fixed') }, W('pinslot'), W('ball')]), st([W('mates')])] },
    { label: 'Motion', items: [{ label: S.driving ? 'Stop' : 'Drive', icon: S.driving ? 'stop' : 'ws-drive', pressed: !!S.driving, run: () => { const j = S.sel && S.sel.kind === 'joint' ? S.sel.id : (S.project.assembly.joints.find((x) => x.type !== 'fixed') || {}).id; if (j) { setMode('assembly'); S.sel = { kind: 'joint', id: j }; drive(j); } }, tip: 'Move the selected joint through its range and back.' }] },
    { label: 'Check', items: [W('interference')] },
    { label: 'View', items: [st(looks.slice(0, 3)), st(looks.slice(3))] },
  ] };
  const drw = { id: 'ws-draw', label: 'Drawing', panels: [
    { label: 'Sheet', items: [{ label: 'Part drawing', icon: 'ws-drawing', pressed: m === 'drawing' && S.drawOf === 'part', run: () => { S.drawOf = 'part'; S.mode = 'drawing'; render(); } }, { label: 'Assembly drawing', icon: 'sheet', pressed: m === 'drawing' && S.drawOf === 'assembly', run: () => { S.drawOf = 'assembly'; S.mode = 'drawing'; render(); } }] },
    { label: 'Annotate', items: [st([{ label: 'Overall sizes', icon: 'dim', pressed: S.dims, run: () => { S.dims = !S.dims; render(); } }, { label: 'Parts list', icon: 'table', pressed: S.list, run: () => { S.list = !S.list; render(); } }, W('hidden')])] },
    { label: 'Output', items: [{ label: 'Save as SVG', icon: 'down', run: saveSvg, disabled: m !== 'drawing', tip: 'The sheet as an SVG file, to print or open in a drawing program.' }, st([W('step'), W('mesh')])] },
  ] };
  return [home, asm, drw];
}
function view3d(side) {
  if (S.mode === 'drawing') { S.mode = S.drawOf === 'assembly' ? 'assembly' : 'part'; render(); }
  if (view) view.look(side);
}
function saveSvg() {
  const svg = $('#vdraw svg');
  if (!svg) return;
  const style = `<style>${[...document.styleSheets].flatMap((s) => { try { return [...s.cssRules].map((r) => r.cssText).filter((t) => t.startsWith('.sheet') || t.startsWith(':root')); } catch (e) { return []; } }).join('\n')}</style>`;
  const text = svg.outerHTML.replace('>', '>' + style);
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([text], { type: 'image/svg+xml' }));
  a.download = (S.drawOf === 'assembly' ? S.project.assembly.name : part().name).replace(/[^\w-]+/g, '-') + '.svg';
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

window.WorkshopFrame = {
  subscribe(fn) { subs.add(fn); return () => subs.delete(fn); },
  icons: () => ICONS,
  tabs,
  home: () => ({ part: 'ws-home', assembly: 'ws-asm', drawing: 'ws-draw' }[S.mode]),
  state: () => ({ mode: S.mode, part: part().name, undo: S.undo.length ? S.undo[S.undo.length - 1].label : '', redo: S.redo.length > 0, engine: engine.label, driving: !!S.driving,
    kept: !S.linked ? 'browser' : S.draft ? 'sample' : 'model' }),
  /* a copy of the parts and the assembly, as the page holds them */
  project: () => JSON.parse(JSON.stringify(S.project)),
  undo, redo,
  file: () => [
    { label: 'New part', run: newPart },
    { label: 'Start again from the sample', run: () => edit('Start again from the sample', () => {
      const name = S.project.name;
      S.project = sample(); S.part = 'bracket'; S.sel = null;
      // in the model, the sample's parts take the place of what was there, under new element ids
      if (S.linked) { S.project.name = name; S.draft = true; }
    }) },
  ],
  /* the app's way to build with the geometry kernel instead (piece A17) */
  useBuilder(fn, about) { builder = fn; engine = about; cache.clear(); render(); },
};

/* ---------- start ---------- */
S.project = load();
SpareIcons.install(ICONS);
try { view = createView($('#v3d'), { onPick: (key) => {
  if (!key) { S.sel = null; render(); return; }
  if (S.mode === 'assembly') S.sel = { kind: 'instance', id: key };
  else S.sel = { kind: 'feature', id: key };
  render();
} }); } catch (e) {
  $('#v3d').innerHTML = '<p class="pad faint">This browser cannot draw 3D (WebGL is off). The history, parameters and drawings still work.</p>';
  view = null;
}
render();
// in the app: the open model's parts, once it has them, and every change to them after that
if (LINK) { LINK.subscribe(linkChanged); linkChanged({}); }
// the geometry kernel, compiled for the browser, builds instead once it loads (piece A17)
import('./kernel.js').then((k) => k.useKernel(window.WorkshopFrame)).catch((e) => console.warn("Workshop: the geometry kernel did not load, so the prototype's builder carries on.", e));
