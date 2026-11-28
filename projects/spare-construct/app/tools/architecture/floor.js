// Tool: the floor tool, a floor in the plan and the properties palette, its slope, slab edges and sketched outline.

import { late, provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, levelOf, modelUrl, say } from '../../core/state.js';
import { around, nearestMm, placementOf, storedMm } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { parametersTable, paramsFor, refreshSide, typeCell } from '../../core/parameters.js';
import {
  canEdit, capital, clearProblems, manage, newId, plural, problem,
} from '../../core/problems.js';
import { heldClaim, hint, queueDraw, snap, toModel } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { colourOf, materialName } from './wall.js';

// --- The floor tool, and a floor in the plan and the properties palette ---
// A floor is drawn from a boundary sketched by clicks in the plan: its edge, a closed loop of
// corners, then any holes, each closed on its first corner or with Enter; Enter on no open loop,
// or Make floor, sends one `floor.create` edit of the floor type chosen in the properties palette,
// its top on the plan's level the offset typed there above it (construct-model's FORMAT.md,
// "Floors"). The page checks nothing and works out no geometry: the floor types, their layers and
// the materials' colours come from the server (`/floor-types`); where a model has none the page
// has the server make its default ones through the log (`floor.defaults`); a refusal is the
// server's reason, under the tool's options, with the boundary it turned back drawn in the plan.
// In the plan a floor is its edge and holes, from its recipe as the server filled it
// (`/filled/<element>`), in its top layer's colour seen from above; a selected floor shows its
// type, its level and offset, its parameters, and apart from them what the server works out of
// its boundary and its type (`from: "read-only"` in `/parameters/<element>`), which nobody sets.

const FLOOR_OP = 'floor';
const READ_ONLY = 'read-only';
const CLOSE_PX = 10; // a click this near a loop's first corner on screen closes the loop
const isFloor = (record) => record?.kind === 'floor' && record?.recipe?.op === FLOOR_OP;

DTOOLS.architecture.push(['floor', 'Floor', 'layers', 'Click the corners of its edge, then any holes. Close each on its first corner or with Enter.']);
TOOLNAME.floor = 'floor';
Object.assign(WORDS, {
  'floor.create': 'drew a floor',
  'floor.boundary': 'changed the boundary of',
  'floor.defaults': 'made the default floor types',
  'floor.type.create': 'made a floor type',
  'floor.type.layers': 'changed the layers of a floor type',
  'floor.type.rename': 'renamed a floor type',
  'floor.type.duplicate': 'copied a floor type',
  'floor.type.delete': 'deleted a floor type',
});
Object.assign(STEP_WORDS, {
  'floor.create': 'drawing a floor',
  'floor.boundary': 'a floor’s boundary',
  'floor.defaults': 'making the default floor types',
  'floor.type.create': 'making a floor type',
  'floor.type.layers': 'the layers of a floor type',
  'floor.type.rename': 'renaming a floor type',
  'floor.type.duplicate': 'copying a floor type',
  'floor.type.delete': 'deleting a floor type',
});

C.floorTypes = null; // { key, view } of the open model
C.floorTool = { type: null, offset: '0 mm' };
C.madeFloorDefaults = null; // the model the default floor types were asked for
// The sketch, in the model's millimetres: closed loops (the edge first, then holes), the loop
// being drawn, the point under the pointer, and the boundary last sent while it may be turned back.
const FLOOR = { loops: [], current: [], at: null, sent: null };

/** The open model's floor types and materials; null until they have come. */
function floorTypes() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}`;
  if (C.floorTypes?.key !== key && C.floorTypesLoading !== key) {
    C.floorTypesLoading = key;
    api(`${modelUrl(model)}/floor-types`)
      .then((view) => {
        C.floorTypes = { key, view };
        refreshSide();
      })
      .catch(() => {});
  }
  return C.floorTypes?.key.startsWith(`${model}/`) ? C.floorTypes.view : null;
}

/** The open model's floor types, read now if they have not come yet. */
async function floorTypesNow() {
  const view = floorTypes();
  if (view) return view;
  const model = C.model.id;
  const fresh = await api(`${modelUrl(model)}/floor-types`);
  C.floorTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
  return fresh;
}

const chosenFloorType = (view) => view.types.find((t) => t.id === C.floorTool.type) ?? view.types[0] ?? null;

/** Has the server make its default floor types, once for a model, where it has none. */
function makeDefaultFloors() {
  if (!canEdit() || C.madeFloorDefaults === C.model.id) return;
  C.madeFloorDefaults = C.model.id;
  manage('floor.defaults', {}, 'floor-draw');
}

/** What the sketch holds, and the button that sends it. */
const sketching = () => S.tool === 'floor' || S.tool === 'roof';
function sketchHtml() {
  const noun = S.tool === 'roof' ? 'roof' : 'floor';
  const [outer, ...holes] = FLOOR.loops;
  let state = 'empty';
  let text = `Click the corners of the ${noun}’s ${noun === 'roof' ? 'footprint' : 'edge'} in the plan. Close it on its first corner or with Enter.`;
  if (FLOOR.current.length) {
    state = 'open';
    const which = outer ? `Hole ${holes.length + 1}` : 'Edge';
    text = `${which}: ${plural(FLOOR.current.length, 'corner')}. Close it on its first corner or with Enter.`;
  } else if (outer) {
    state = 'closed';
    const holeText = holes.length ? `, ${plural(holes.length, 'hole')}` : '';
    text = `Edge: ${plural(outer.length, 'corner')}${holeText}. Click to start a hole, or press Enter to make the ${noun}.`;
  }
  const ready = state === 'closed' ? '' : ' disabled';
  return `<p class="pad small muted" data-${noun}-sketch="${state}">${esc(text)}</p><div class="pad"><button class="btn pri sm" data-act="stack${noun}make" data-key="${noun}-make"${ready}>Make ${noun}</button></div>`;
}

function floorToolHtml() {
  let html = typeSel('layers', 'Floor', '<b>New floor</b>');
  const view = floorTypes();
  if (!view) return `${html}<p class="pad small faint">Loading the floor types.</p>`;
  if (!view.types.length) {
    queueMicrotask(makeDefaultFloors);
    return `${html}<p class="pad small faint" data-floor-tool="making">This model has no floor types yet, so the default ones are being made.</p>${problem('floor-draw')}`;
  }
  const chosen = chosenFloorType(view);
  const here = levelOf();
  const level = (modelLevels() || []).find((l) => l.id === here)?.name ?? here;
  const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
  html += '<table class="props">';
  html += prow('Type', `<select class="inp" data-chg="stackfloortype" data-key="floor-type" aria-label="Floor type">${types.join('')}</select>`);
  html += prow('Thickness', esc(chosen.thickness));
  html += prow('Level', esc(level));
  html += prow('Offset', `<input class="inp" data-chg="stackflooroffset" data-key="floor-offset" value="${esc(C.floorTool.offset)}" aria-label="Offset of the floor’s top above its level">`);
  html += '</table>';
  html += `<span class="lbl">Layers, top first</span><table class="props" data-floor-layers="${esc(chosen.id)}">`;
  for (const layer of chosen.layers) {
    html += prow(esc(capital(layer.function)), `${esc(layer.thickness)} · ${esc(materialName(view, layer.material))}`);
  }
  html += '</table>';
  return html + sketchHtml() + problem('floor-draw');
}

CHG.stackfloortype = (select) => {
  C.floorTool.type = select.value;
  refreshSide();
};
CHG.stackflooroffset = (input) => {
  C.floorTool.offset = input.value;
};
ACT.stackfloormake = () => makeFloor();

function floorHint() {
  if (S.tool === 'roof') return late.roofHint();
  if (FLOOR.current.length) return '<b>Floor:</b> click the next corner, or its first corner or Enter to close the loop. Esc to stop.';
  if (FLOOR.loops.length) return '<b>Floor:</b> click to start a hole, or press Enter to make the floor. Esc to stop.';
  return '<b>Floor:</b> click the corners of its edge. Choose its type in the properties palette.';
}

function sketchChanged() {
  $('#hint').innerHTML = hint();
  refreshSide();
  queueDraw();
}

function clearSketch() {
  Object.assign(FLOOR, { loops: [], current: [], at: null, sent: null });
}

/** Whether a click at `at` (the model's millimetres) is within reach of `point` on screen. */
function nearOnScreen(point, at) {
  const scale = $('#pv-canvas svg')?.getScreenCTM()?.a || 1;
  return Math.hypot(point[0] - at.x, point[1] - at.y) * scale <= CLOSE_PX;
}

function closeLoop() {
  if (!FLOOR.current.length) return;
  FLOOR.loops.push(FLOOR.current);
  FLOOR.current = [];
  FLOOR.at = null;
  sketchChanged();
}

function floorClick(event) {
  const at = toModel(event);
  const point = [snap(at.x), snap(at.y)];
  FLOOR.sent = null;
  if (FLOOR.current.length > 1 && nearOnScreen(FLOOR.current[0], at)) {
    closeLoop();
    return;
  }
  const last = FLOOR.current.at(-1);
  if (!last || last[0] !== point[0] || last[1] !== point[1]) FLOOR.current.push(point);
  sketchChanged();
}

/** Sends the sketch as it is: what the server makes of it, or why it refuses, comes back through the log. */
async function makeFloor() {
  if (!FLOOR.loops.length || FLOOR.current.length || !canEdit()) return;
  let view;
  try {
    view = await floorTypesNow();
  } catch (error) {
    say(error.message);
    return;
  }
  const corners = (loop) => loop.map(([x, y]) => ({ at: [`${x} mm`, `${y} mm`] }));
  const [outer, ...holes] = FLOOR.loops;
  const args = { element: newId('F'), level: levelOf(), boundary: { outer: corners(outer), holes: holes.map(corners) } };
  const type = chosenFloorType(view);
  if (type) args.type = type.id;
  if (C.floorTool.offset.trim() !== '') args.offset = C.floorTool.offset;
  const sent = { loops: FLOOR.loops };
  Object.assign(FLOOR, { loops: [], current: [], at: null, sent });
  manage('floor.create', args, 'floor-draw', () => {
    if (FLOOR.sent === sent) FLOOR.sent = null;
    queueDraw();
  });
  sketchChanged();
}

// In the plan, before the elements' own handlers: the floor tool's corners.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (!sketching() || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
    event.stopPropagation();
    if (!canEdit()) say('Open a model in your team’s WIP to add to it.');
    else floorClick(event);
  },
  true,
);
window.addEventListener('pointermove', (event) => {
  if (!sketching() || !FLOOR.current.length || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  FLOOR.at = [snap(at.x), snap(at.y)];
  queueDraw();
});
// Escape leaves the tool (the demo's own key) and drops the sketch; Enter closes the loop being
// drawn, or makes the floor when none is open.
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    clearSketch();
    return;
  }
  if (!sketching() || event.key !== 'Enter' || event.target.closest?.('input,textarea,button')) return;
  event.preventDefault();
  if (FLOOR.current.length) closeLoop();
  else if (S.tool === 'roof') late.makeRoof();
  else makeFloor();
});
const toolBeforeFloors = ACT.tool;
ACT.tool = (button) => {
  clearSketch();
  toolBeforeFloors(button);
};

/** The ribbon: the floor tool beside the wall tool, in the Build panel. */
function floorRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:floor')) return;
  const wall = build.items.findIndex((item) => item.a === 'architecture:wall');
  build.items.splice(wall + 1, 0, DT_('architecture', 'floor'));
}

const ringPath = (points) => `M${points.map(([x, y]) => `${x},${y}`).join('L')}Z`;

/** The sketch in the plan: closed loops, the loop being drawn up to the pointer, and a boundary the server turned back. */
function floorSketch() {
  let svg = pickInPlan();
  if (!sketching()) return svg;
  const where = S.tool === 'roof' ? 'roof-draw' : 'floor-draw';
  if (FLOOR.sent && C.problems.has(where)) {
    svg += `<path class="floor-refused" data-${S.tool}-refused="${FLOOR.sent.loops.length}" d="${FLOOR.sent.loops.map(ringPath).join('')}"/>`;
  }
  for (const loop of FLOOR.loops) svg += `<path class="ln ap floor-sketch" d="${ringPath(loop)}"/>`;
  const open = FLOOR.at && FLOOR.current.length ? [...FLOOR.current, FLOOR.at] : FLOOR.current;
  if (open.length > 1) svg += `<polyline class="ln ap dd floor-sketch" points="${open.map((p) => p.join(',')).join(' ')}"/>`;
  for (const [x, y] of FLOOR.current) svg += `<circle class="apf floor-sketch" cx="${x}" cy="${y}" r="60"/>`;
  return svg;
}

/**
 * A floor in the plan: its edge and holes, from its recipe as the server filled it (the outline of
 * its display, which has no holes, until that has come), in its top layer's colour seen from above.
 */
function floorInPlan(piece, data, scale) {
  const { el: e } = piece;
  const { x, y } = placementOf(e);
  const loops = piece.fill?.recipe?.args?.loops;
  const rings = Array.isArray(loops) && loops.length
    ? loops.map((loop) => loop.map((corner) => [nearestMm(corner.at[0]), nearestMm(corner.at[1])]))
    : [piece.shape.displays[0].outline];
  // The openings and shafts that cut it, as the server says (`/filled`), are more holes.
  for (const cut of piece.fill?.cut_by?.cuts ?? []) rings.push(cut.corners.map(([cx, cy]) => [nearestMm(cx), nearestMm(cy)]));
  const placed = rings.map((ring) => ring.map(([px, py]) => [x + px, y + py]));
  const top = piece.shape.displays[0]?.parts?.[0];
  const colour = top && colourOf(top.material, 'surface');
  const fill = colour ? ` style="fill:${esc(colour)}"` : '';
  const box = around(piece);
  const chosen = S.sel === e.id;
  let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
  group += `<path class="body floor${chosen ? ' sel' : ''}" data-material="${esc(top?.material ?? '')}" data-holes="${placed.length - 1}"${fill} d="${placed.map(ringPath).join('')}"/>`;
  group += floorShapeInPlan(piece, x, y, scale);
  const claim = heldClaim(e.id);
  if (claim) {
    const theirs = claim.person !== C.me.person.id;
    group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
    if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
  }
  return `<g ${data}>${group}</g>`;
}

/** What the server works out of a floor, apart from what a person sets, and set by nobody. */
function readOnlyTable(view) {
  const rows = view.parameters.filter((p) => p.from === READ_ONLY);
  if (!rows.length) return '';
  let html = '<span class="lbl">Worked out, read-only</span><table class="props" data-read-only-rows>';
  for (const p of rows) html += prow(esc(p.name), `<span class="mono" data-read-only="${esc(p.id)}">${esc(p.shown ?? '')}</span>`);
  return `${html}</table>`;
}

/** A floor's offset above its level, as stored. */
function floorOffset(e) {
  const stored = e.props?.['@level.offset'];
  return `${stored === undefined ? 0 : storedMm(stored)} mm`;
}

/** What the properties palette shows of a selected floor. */
function floorProperties(piece) {
  const { el: e, shape, failed } = piece;
  const view = paramsFor(piece);
  const claim = heldClaim(e.id);
  let html = typeSel('layers', 'Floor', `<b>${esc(e.name)}</b>`);
  html += '<table class="props" data-floor-element>';
  if (view) html += prow('Category', esc(view.category?.name || 'None')) + prow('Type', typeCell(view));
  html += prow('Level', esc(levelName(e))) + prow('Offset', `<span class="mono" data-floor-offset>${esc(floorOffset(e))}</span>`);
  if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  html += floorShapeHtml(piece);
  // Its top face's shape edits: the floor-shape module's part (job 14.15).
  html += late.floorTopHtml?.(piece) ?? '';
  if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
  if (view) {
    html += readOnlyTable(view);
    html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  }
  return html;
}

// --- A floor's slope and slab edges, and its sketched outline ---
// A selected floor's palette sets its slope (an arrow drawn in the plan by two clicks, with the top
// face's two heights typed beside it: `floor.slope`; `floor.slope.clear`) and its slab edges (a
// profile typed, then the floor's edges picked in the plan: `floor.edge`; `floor.edge.remove`).
// The page checks nothing and works out no geometry: the plan draws the arrow from the floor's
// record and its slab edges from its recipe as the server filled it, 3D draws what the server
// built, the quantities are the server's read-only rows, and a refusal (a slope and slab edges on
// one floor, a corner off the grid) is the server's reason under the part it came from.
// A sketched outline's dimensions are changed by `floor.dimension`, which the page does not offer
// yet: it has no way to show a sketch's dimensions (construct-model's FORMAT.md, "Floors").

Object.assign(WORDS, {
  'floor.slope': 'sloped',
  'floor.slope.clear': 'took the slope off',
  'floor.edge': 'put a slab edge on',
  'floor.edge.remove': 'took a slab edge off',
  'floor.dimension': 'changed a dimension of',
});
Object.assign(STEP_WORDS, {
  'floor.slope': 'a floor’s slope',
  'floor.slope.clear': 'taking a floor’s slope off',
  'floor.edge': 'a floor’s slab edge',
  'floor.edge.remove': 'taking a slab edge off',
  'floor.dimension': 'a floor’s dimension',
});

C.slopeTool = { atTail: '0 mm', atHead: '-40 mm' };
C.edgeTool = { kind: 'downturn', width: '300 mm', foot: '150 mm', depth: '400 mm' };
// What the plan is waiting for: { floor, what: 'slope', tail: [x, y] | null } for the arrow's two
// clicks, or { floor, what: 'edge' } for slab edges picked on the floor's sides.
const PICK = { now: null };

const mmPoint = (stored) => [nearestMm(stored[0]), nearestMm(stored[1])];

/** A floor's slope as its record holds it, or null. */
function slopeOf(e) {
  const slope = e.recipe?.args?.slope;
  if (!slope) return null;
  const at = (key) => (slope[key] === undefined ? null : `${storedMm(slope[key])} mm`);
  return {
    tail: mmPoint(slope.tail),
    head: mmPoint(slope.head),
    atTail: at('at_tail'),
    atHead: at('at_head'),
    gradient: slope.slope ? late.typedPitch(slope.slope) : null,
  };
}

/** A floor's slab edges as its record holds them: `[{ edge, kind, width, foot, depth }]`. */
function slabEdgesOf(e) {
  const list = e.recipe?.args?.edges;
  if (!Array.isArray(list)) return [];
  const mm = (text) => (text === undefined ? '' : `${storedMm(text)} mm`);
  return list.map((s) => ({ edge: s.edge, kind: s.kind, width: mm(s.width), foot: mm(s.foot), depth: mm(s.depth) }));
}

/** The slope and slab edges part of a selected floor's palette. */
function floorShapeHtml(piece) {
  const { el: e } = piece;
  const editable = canEdit();
  const slope = slopeOf(e);
  const edges = slabEdgesOf(e);
  const field = (tool, key, label) => `<input class="inp" data-chg="stackfloorfield" data-tool="${tool}" data-a="${key}" data-key="floor-${tool}-${key}" value="${esc(C[`${tool}Tool`][key])}" aria-label="${esc(label)}">`;
  let html = '<span class="lbl">Slope</span><table class="props" data-floor-slope-part>';
  if (slope) {
    const run = slope.gradient ? `rises ${esc(slope.gradient)}` : `${esc(slope.atTail)} to ${esc(slope.atHead)}`;
    html += prow('Now', `<span class="mono" data-floor-slope-now>${run}</span>`);
  } else {
    html += prow('Now', '<span class="faint" data-floor-slope-now>Flat</span>');
  }
  if (editable) {
    html += prow('Height at tail', field('slope', 'atTail', 'The top face’s height at the arrow’s tail, from the floor’s own height'));
    html += prow('Height at head', field('slope', 'atHead', 'The top face’s height at the arrow’s head'));
  }
  html += '</table>';
  if (editable) {
    const waiting = PICK.now?.floor === e.id && PICK.now.what === 'slope';
    const hint = waiting ? `<p class="pad small muted" data-floor-pick="${PICK.now.tail ? 'head' : 'tail'}">${PICK.now.tail ? 'Click where the arrow ends.' : 'Click where the arrow starts.'}</p>` : '';
    html += `${hint}<div class="pad"><button class="btn sm" data-act="stackslopepick" data-key="floor-slope-pick">${waiting ? 'Stop drawing' : slope ? 'Draw it again' : 'Draw slope arrow'}</button>`;
    if (slope) html += ' <button class="btn sm" data-act="stackslopeclear" data-key="floor-slope-clear">Clear slope</button>';
    html += '</div>';
  }
  html += problem('floor-slope');
  html += '<span class="lbl">Slab edges</span>';
  if (edges.length) {
    html += '<table class="props" data-floor-slab-edges>';
    for (const s of edges) {
      const sizes = s.kind === 'thickened' ? `${s.width} wide, ${s.foot} foot, ${s.depth} deep` : `${s.width} wide, ${s.depth} deep`;
      const remove = editable ? ` <button class="btn sm" data-act="stackedgeremove" data-edge="${esc(s.edge)}" data-key="floor-edge-remove-${esc(s.edge)}">Remove</button>` : '';
      html += prow(esc(s.edge), `<span data-slab-edge-row="${esc(s.edge)}">${esc(capital(s.kind))}: ${esc(sizes)}</span>${remove}`);
    }
    html += '</table>';
  } else {
    html += '<p class="pad small faint" data-floor-slab-edges="none">None.</p>';
  }
  if (editable) {
    const kinds = [['downturn', 'Downturn'], ['thickened', 'Thickened']].map(([id, name]) => `<option value="${id}"${id === C.edgeTool.kind ? ' selected' : ''}>${name}</option>`);
    html += '<table class="props">';
    html += prow('Edge', `<select class="inp" data-chg="stackedgekind" data-key="floor-edge-kind" aria-label="Slab edge profile">${kinds.join('')}</select>`);
    html += prow('Width', field('edge', 'width', 'Width in from the edge'));
    if (C.edgeTool.kind === 'thickened') html += prow('Foot', field('edge', 'foot', 'Width at the bottom'));
    html += prow('Depth', field('edge', 'depth', 'Depth below the floor'));
    html += '</table>';
    const waiting = PICK.now?.floor === e.id && PICK.now.what === 'edge';
    const hint = waiting ? '<p class="pad small muted" data-floor-pick="edge">Click the floor’s edges in the plan.</p>' : '';
    html += `${hint}<div class="pad"><button class="btn sm" data-act="stackedgepick" data-key="floor-edge-pick">${waiting ? 'Stop picking' : 'Pick edges'}</button></div>`;
  }
  return html + problem('floor-edge');
}

CHG.stackfloorfield = (input) => {
  C[`${input.dataset.tool}Tool`][input.dataset.a] = input.value;
};
CHG.stackedgekind = (select) => {
  C.edgeTool.kind = select.value;
  refreshSide();
};

/** Starts or stops waiting for clicks in the plan for the selected floor. */
function startPick(what) {
  const stopping = PICK.now?.floor === S.sel && PICK.now.what === what;
  PICK.now = stopping || !S.sel ? null : { floor: S.sel, what, tail: null };
  clearProblems(/^floor-(slope|edge)$/);
  refreshSide();
  queueDraw();
}
ACT.stackslopepick = () => startPick('slope');
ACT.stackedgepick = () => startPick('edge');
ACT.stackslopeclear = () => {
  if (S.sel) manage('floor.slope.clear', { element: S.sel }, 'floor-slope');
};
ACT.stackedgeremove = (button) => {
  if (S.sel) manage('floor.edge.remove', { element: S.sel, edge: button.dataset.edge }, 'floor-edge');
};

/** The arrow's two clicks: its tail, then its head, which sends the slope with the heights typed. */
function slopeClick(at) {
  const point = [snap(at.x), snap(at.y)];
  if (!PICK.now.tail) {
    PICK.now.tail = point;
    refreshSide();
    queueDraw();
    return;
  }
  const [tail, floor] = [PICK.now.tail, PICK.now.floor];
  PICK.now = null;
  if (tail[0] === point[0] && tail[1] === point[1]) {
    refreshSide();
    queueDraw();
    return;
  }
  const slope = {
    tail: tail.map((v) => `${v} mm`),
    head: point.map((v) => `${v} mm`),
    at_tail: C.slopeTool.atTail,
    at_head: C.slopeTool.atHead,
  };
  manage('floor.slope', { element: floor, slope }, 'floor-slope');
  queueDraw();
}

/** A click on one of the selected floor's sides while edges are being picked. */
function edgeClick(side) {
  const tool = C.edgeTool;
  const profile = { kind: tool.kind, width: tool.width, depth: tool.depth };
  if (tool.kind === 'thickened') profile.foot = tool.foot;
  manage('floor.edge', { element: PICK.now.floor, edge: side.dataset.floorEdge, profile }, 'floor-edge');
}

// Before the plan's own handlers, as the floor tool's clicks are.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (!PICK.now || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
    if (S.sel !== PICK.now.floor || !canEdit()) return;
    if (PICK.now.what === 'slope') {
      event.stopPropagation();
      slopeClick(toModel(event));
      return;
    }
    // A click that is not on one of the floor's sides is the plan's own (it picks something else).
    const side = event.target.closest('[data-floor-edge]');
    if (!side) return;
    event.stopPropagation();
    edgeClick(side);
  },
  true,
);
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && PICK.now) {
    PICK.now = null;
    refreshSide();
    queueDraw();
  }
});

/**
 * A floor's slope arrow and slab edges in the plan, from its record and its recipe as the server
 * filled it: the arrow with its two heights, each slab edge's plan at the floor's underside, and,
 * while edges are picked, a stripe along each side of the chosen floor to click.
 */
function floorShapeInPlan(piece, x, y, scale) {
  const { el: e } = piece;
  let svg = '';
  for (const s of piece.fill?.recipe?.args?.edges ?? []) {
    const ring = s.top.map((p) => [x + nearestMm(p[0]), y + nearestMm(p[1])]);
    svg += `<path class="floor-slab-edge" data-slab-edge="${esc(s.id)}" data-kind="${esc(s.kind)}" d="${ringPath(ring)}"/>`;
  }
  const slope = slopeOf(e);
  if (slope) {
    const [[ax, ay], [bx, by]] = [slope.tail, slope.head].map(([px, py]) => [x + px, y + py]);
    const length = Math.hypot(bx - ax, by - ay) || 1;
    const [ux, uy] = [(bx - ax) / length, (by - ay) / length];
    const back = (n) => [bx - ux * n - uy * n * 0.4, by - uy * n + ux * n * 0.4];
    const other = (n) => [bx - ux * n + uy * n * 0.4, by - uy * n - ux * n * 0.4];
    const head = [[bx, by], back(250), other(250)];
    svg += `<g class="floor-slope" data-floor-slope="${esc(e.id)}"><line x1="${ax}" y1="${ay}" x2="${bx}" y2="${by}"/><path d="${ringPath(head)}"/>`;
    svg += `${TX(ax, ay - 120, scale, slope.atTail ?? '', 'tf', 220)}${TX(bx, by - 120, scale, slope.atHead ?? slope.gradient ?? '', 'tf', 220)}</g>`;
  }
  if (PICK.now?.what === 'edge' && PICK.now.floor === e.id) {
    for (const ring of piece.fill?.recipe?.args?.loops ?? []) {
      ring.forEach((corner, k) => {
        const next = ring[(k + 1) % ring.length];
        const [p, q] = [corner, next].map((c) => [x + nearestMm(c.at[0]), y + nearestMm(c.at[1])]);
        svg += `<line class="floor-edge-hit" data-floor-edge="${esc(corner.id)}" x1="${p[0]}" y1="${p[1]}" x2="${q[0]}" y2="${q[1]}"/>`;
      });
    }
  }
  return svg;
}

/** The arrow's tail once it is clicked, drawn while the head is awaited. */
function pickInPlan() {
  const tail = PICK.now?.what === 'slope' ? PICK.now.tail : null;
  return tail ? `<circle class="apf floor-sketch" data-floor-slope-tail cx="${tail[0]}" cy="${tail[1]}" r="60"/>` : '';
}

/** The properties palette's floor part: the floor tool's options, or a selected floor; '' for anything else. */
function floorPalette(piece) {
  if (S.tool === 'floor' && canEdit()) return floorToolHtml();
  if (piece && isFloor(piece.el) && !(S.tool === 'wall' && canEdit())) return floorProperties(piece);
  return '';
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'floor', discipline: 'architecture', order: 20 });
register.ribbons({ id: 'floor', order: 10, ribbon: (config) => floorRibbon(config) });
register.hints({ id: 'floor', order: 30, hint: () => (S.tool === 'floor' || S.tool === 'roof' ? floorHint() : undefined) });
register.palettes({ id: 'floor', order: 10, palette: (piece) => floorPalette(piece) || undefined });
register.elements({ id: 'floor', order: 20, owns: isFloor, inPlan: (piece, data, scale) => floorInPlan(piece, data, scale) });
register.drawOrders({ id: 'floor', order: 30, drawOrder: (el) => (isFloor(el) ? 0 : undefined) });
register.layers({ id: 'floor-sketch', order: 50, over: () => floorSketch() });

// What files loaded before this one use from it, bound once it has loaded.
provide({ get READ_ONLY() { return READ_ONLY; } });

export { FLOOR, READ_ONLY, floorTypes, isFloor, readOnlyTable, ringPath, sketchChanged, sketchHtml };
