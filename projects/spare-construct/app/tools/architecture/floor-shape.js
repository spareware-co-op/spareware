// Tool: a floor's shape edits (job 14.15): points, split lines, supports and its type's variable layer.

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C } from '../../core/state.js';
import { nearestMm, placementOf, storedMm } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, clearProblems, manage, plural, problem } from '../../core/problems.js';
import { queueDraw, snap, toModel } from '../../core/plan.js';
import { floorTypes, isFloor } from './floor.js';

// --- A floor's shape edits ---
// A selected floor's palette edits its top face: a point clicked in the plan at the height typed
// (`floor.shape.point`), a split line by two clicks (`floor.shape.split`), a wall clicked as a
// support (`floor.shape.support`), a point's height typed in its row (`floor.shape.height`), a row
// removed (`floor.shape.remove`), and Reset shape (`floor.shape.reset`); and its type's variable
// layer (`floor.type.layers` with one layer marked). The page checks nothing and works out no
// geometry: the plan draws the points and lines from the floor's record and the creases of its top
// from its recipe as the server filled it (`faces`), 3D draws what was built, the heights and
// volumes are the server's read-only rows, and a refusal (a point off the floor, lines that cross,
// a slope and shape edits on one floor) is the server's reason under this part. The floor tool's
// palette asks for this part (`late.floorTopHtml`); the plan draws it as a layer over the elements.

Object.assign(WORDS, {
  'floor.shape.point': 'put a point on',
  'floor.shape.height': 'set a height on',
  'floor.shape.split': 'drew a split line on',
  'floor.shape.support': 'picked a support for',
  'floor.shape.remove': 'took a point or line off',
  'floor.shape.reset': 'reset the shape of',
});
Object.assign(STEP_WORDS, {
  'floor.shape.point': 'a point on a floor',
  'floor.shape.height': 'a height on a floor',
  'floor.shape.split': 'a split line on a floor',
  'floor.shape.support': 'a support under a floor',
  'floor.shape.remove': 'taking a point or line off a floor',
  'floor.shape.reset': 'resetting a floor’s shape',
});

C.topTool = { height: '-20 mm' };
// What the plan is waiting for: { floor, what: 'point' | 'split' | 'support', tail } or null.
const TOP_PICK = { now: null };

const mmPoint = (stored) => [nearestMm(stored[0]), nearestMm(stored[1])];

/** A floor's shape edits as its record holds them: `{ points: [{ id, at, height }], splits: [{ id, from, to }] }`. */
function topOf(e) {
  const top = e.recipe?.args?.top;
  if (!top) return { points: [], splits: [] };
  return {
    points: (top.points ?? []).map((p) => ({ id: p.id, at: mmPoint(p.at), height: `${storedMm(p.height)} mm` })),
    splits: top.splits ?? [],
  };
}

/** The shape part of a selected floor's palette. */
function floorTopHtml(piece) {
  const { el: e } = piece;
  const editable = canEdit();
  const { points, splits } = topOf(e);
  let html = '<span class="lbl">Shape</span><table class="props" data-floor-top-part>';
  const now = points.length ? `${plural(points.length, 'point')}, ${plural(splits.length, 'split line')}` : 'Plain';
  html += prow('Now', `<span class="${points.length ? 'mono' : 'faint'}" data-floor-top-now>${esc(now)}</span>`);
  for (const p of points) {
    const height = editable
      ? `<input class="inp" data-chg="stackfloortopheight" data-point="${esc(p.id)}" data-key="floor-top-height-${esc(p.id)}" value="${esc(p.height)}" aria-label="Height of point ${esc(p.id)}">`
      : `<span class="mono">${esc(p.height)}</span>`;
    const remove = editable ? ` <button class="btn sm" data-act="stackfloortopremove" data-point="${esc(p.id)}" data-key="floor-top-remove-${esc(p.id)}">Remove</button>` : '';
    html += prow(`Point ${esc(p.id)}`, `<span data-floor-top-row="${esc(p.id)}">${height}${remove}</span>`);
  }
  for (const s of splits) {
    const remove = editable ? ` <button class="btn sm" data-act="stackfloortopremove" data-split="${esc(s.id)}" data-key="floor-top-remove-${esc(s.id)}">Remove</button>` : '';
    html += prow(`Line ${esc(s.id)}`, `<span data-floor-top-row="${esc(s.id)}">${esc(s.from)} to ${esc(s.to)}${remove}</span>`);
  }
  if (editable) {
    html += prow('New height', `<input class="inp" data-chg="stackfloortopnew" data-key="floor-top-height" value="${esc(C.topTool.height)}" aria-label="Height of a new point, from the floor’s top">`);
    html += floorVariableRow(e);
  }
  html += '</table>';
  if (editable) {
    const waiting = TOP_PICK.now?.floor === e.id ? TOP_PICK.now : null;
    const words = { point: 'Click where the point goes.', split: waiting?.tail ? 'Click where the line ends.' : 'Click where the line starts.', support: 'Click the wall that holds the floor up.' };
    if (waiting) html += `<p class="pad small muted" data-floor-pick="${waiting.what}${waiting.tail ? '-end' : ''}">${words[waiting.what]}</p>`;
    const button = (what, label) => `<button class="btn sm" data-act="stackfloortoppick" data-what="${what}" data-key="floor-top-${what}">${waiting?.what === what ? 'Stop' : label}</button>`;
    html += `<div class="pad">${button('point', 'Add point')} ${button('split', 'Add split line')} ${button('support', 'Pick support')}`;
    if (points.length) html += ' <button class="btn sm" data-act="stackfloortopreset" data-key="floor-top-reset">Reset shape</button>';
    html += '</div>';
  }
  return html + problem('floor-top');
}

/** The row that chooses the floor type's variable layer, once the type's layers have come. */
function floorVariableRow(e) {
  const ty = floorTypes()?.types.find((t) => t.id === e.props?.['@type']);
  if (!ty) return '';
  const chosen = ty.layers.findIndex((l) => l.variable);
  const options = [`<option value=""${chosen < 0 ? ' selected' : ''}>None: every layer follows the top</option>`];
  ty.layers.forEach((l, k) => {
    if (l.function !== 'membrane') options.push(`<option value="${k}"${k === chosen ? ' selected' : ''}>Layer ${k + 1}: ${esc(l.function)}, ${esc(l.thickness)}</option>`);
  });
  return prow('Variable layer', `<select class="inp" data-chg="stackfloorvariable" data-type="${esc(ty.id)}" data-key="floor-top-variable" aria-label="The floor type’s variable layer">${options.join('')}</select>`);
}

/** Starts or stops waiting for clicks in the plan for the selected floor. */
function startTopPick(what) {
  const stopping = TOP_PICK.now?.floor === S.sel && TOP_PICK.now.what === what;
  TOP_PICK.now = stopping || !S.sel ? null : { floor: S.sel, what, tail: null };
  clearProblems(/^floor-top$/);
  refreshSide();
  queueDraw();
}

ACT.stackfloortoppick = (button) => startTopPick(button.dataset.what);
ACT.stackfloortopreset = () => {
  if (S.sel) manage('floor.shape.reset', { element: S.sel }, 'floor-top');
};
ACT.stackfloortopremove = (button) => {
  if (!S.sel) return;
  const args = button.dataset.point ? { point: button.dataset.point } : { split: button.dataset.split };
  manage('floor.shape.remove', { element: S.sel, ...args }, 'floor-top');
};
CHG.stackfloortopnew = (input) => {
  C.topTool.height = input.value;
};
CHG.stackfloortopheight = (input) => {
  if (S.sel) manage('floor.shape.height', { element: S.sel, point: input.dataset.point, height: input.value }, 'floor-top');
};
CHG.stackfloorvariable = (select) => {
  const ty = floorTypes()?.types.find((t) => t.id === select.dataset.type);
  if (!ty) return;
  const k = select.value === '' ? -1 : Number(select.value);
  const layers = ty.layers.map((l, at) => ({ function: l.function, thickness: l.thickness, material: l.material, ...(at === k ? { variable: true } : {}) }));
  manage('floor.type.layers', { type: ty.id, layers }, 'floor-top');
};

/** A click in the plan while a point, a split line or a support is awaited. */
function topClick(event) {
  const pick = TOP_PICK.now;
  const at = toModel(event);
  const point = [snap(at.x), snap(at.y)].map((v) => `${v} mm`);
  if (pick.what === 'point') {
    TOP_PICK.now = null;
    manage('floor.shape.point', { element: pick.floor, at: point, height: C.topTool.height }, 'floor-top');
  } else if (pick.what === 'split' && !pick.tail) {
    pick.tail = point;
  } else if (pick.what === 'split') {
    TOP_PICK.now = null;
    manage('floor.shape.split', { element: pick.floor, from: pick.tail, to: point }, 'floor-top');
  } else {
    const wall = event.target.closest?.('g.element[data-id]')?.dataset.id;
    if (!wall || wall === pick.floor) return;
    TOP_PICK.now = null;
    manage('floor.shape.support', { element: pick.floor, wall }, 'floor-top');
  }
  refreshSide();
  queueDraw();
}

// Before the plan's own handlers, as the floor tool's slope arrow clicks are.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (!TOP_PICK.now || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
    if (S.sel !== TOP_PICK.now.floor || !canEdit()) return;
    event.stopPropagation();
    topClick(event);
  },
  true,
);
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && TOP_PICK.now) {
    TOP_PICK.now = null;
    refreshSide();
    queueDraw();
  }
});

/**
 * A floor's shape edits in the plan: the creases of its top face (each triangle's edges, from its
 * recipe as the server filled it), each split line, and each point with its height.
 */
function floorTopInPlan(piece, scale) {
  const { points, splits } = topOf(piece.el);
  if (!points.length) return '';
  const { x, y } = placementOf(piece.el);
  const faces = piece.fill?.recipe?.args?.faces;
  const mm = (p) => [x + nearestMm(p[0]), y + nearestMm(p[1])];
  let svg = `<g class="floor-top" data-floor-top="${esc(piece.el.id)}">`;
  const seen = new Set();
  for (const face of Array.isArray(faces) ? faces : []) {
    face.forEach((a, k) => {
      const b = face[(k + 1) % 3];
      const key = [a, b].map((p) => `${p[0]},${p[1]}`).sort().join('|');
      if (seen.has(key)) return;
      seen.add(key);
      const [p, q] = [mm(a), mm(b)];
      svg += `<line class="floor-top-crease" x1="${p[0]}" y1="${p[1]}" x2="${q[0]}" y2="${q[1]}"/>`;
    });
  }
  const where = new Map(points.map((p) => [p.id, [x + p.at[0], y + p.at[1]]]));
  for (const s of splits) {
    const [p, q] = [where.get(s.from), where.get(s.to)];
    if (p && q) svg += `<line class="floor-top-split" data-floor-top-split="${esc(s.id)}" x1="${p[0]}" y1="${p[1]}" x2="${q[0]}" y2="${q[1]}"/>`;
  }
  for (const p of points) {
    const [px, py] = where.get(p.id);
    svg += `<g data-floor-top-point="${esc(p.id)}"><circle cx="${px}" cy="${py}" r="60"/>${TX(px, py - 120, scale, p.height, 'tf', 220)}</g>`;
  }
  return `${svg}</g>`;
}

/** Every shape-edited floor of the open model in the plan, over the elements. */
function floorTopsInPlan(scale) {
  let svg = '';
  for (const piece of C.elements.values()) {
    if (isFloor(piece.el) && piece.el.recipe?.args?.top) svg += floorTopInPlan(piece, scale);
  }
  return svg;
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.layers({ id: 'floor-shape', order: 55, over: (scale) => floorTopsInPlan(scale) });

// What files loaded before this one use from it, bound once it has loaded.
provide({ get floorTopHtml() { return floorTopHtml; } });
