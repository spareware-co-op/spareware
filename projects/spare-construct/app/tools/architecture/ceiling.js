// Tool: the ceiling tool, a ceiling in the plan and the properties palette, its grid pattern and its outline by a click in a room or a sketch.

import { register } from '../../core/registry.js';
import { C, api, levelIdOf, levelOf, modelUrl, say } from '../../core/state.js';
import { around, nearestMm, storedMm } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { parametersTable, paramsFor, refreshSide, typeCell } from '../../core/parameters.js';
import { canEdit, capital, manage, newId, plural, problem } from '../../core/problems.js';
import { heldClaim, hint, queueDraw, snap, toModel } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { colourOf, materialName } from './wall.js';
import { READ_ONLY, readOnlyTable, ringPath } from './floor.js';

// --- The ceiling tool, and a ceiling in the plan and the properties palette ---
// A ceiling is made by a click inside a room's walls (one `ceiling.create` edit with the point:
// the server finds the loop of walls round it by the very search a room uses and writes it as the
// ceiling's outline) or by a sketch of corners in the plan, closed as a floor's are (Enter makes
// it, and a second loop is an opening). Its type, its height above the plan's level (its exposed
// face, the underside a person sees) and an optional grid of tiles (a width, a depth and an
// angle) are chosen in the properties palette; construct-model's FORMAT.md, "Ceilings". The page
// checks nothing and works out no geometry that is stored or counted: the ceiling types and their
// layers come from the server (`/ceiling-types`; where a model has none the page has the server
// make its default ones, `ceiling.defaults`), a refusal (a point no walls close round) is the
// server's reason under the tool's options, the tile counts and the area are the server's
// read-only rows, and the grid lines in the plan are drawn here from the grid's numbers, a drawing
// aid clipped to the outline (the server counts the tiles, exactly). A selected ceiling's palette
// changes its height (`level.tie`) and its grid (`ceiling.grid`, `ceiling.grid.clear`).

const CEILING_OP = 'ceiling';
const CLOSE_PX = 10; // a click this near a loop's first corner on screen closes the loop
const MOST_LINES = 400; // grid lines drawn at most, so a tiny tile never fills the plan
const isCeiling = (record) => record?.kind === 'ceiling' && record?.recipe?.op === CEILING_OP;

DTOOLS.architecture.push(['ceiling', 'Ceiling', 'layers', 'Click inside a room’s walls to place a ceiling, or choose to sketch its outline.']);
TOOLNAME.ceiling = 'ceiling';
Object.assign(WORDS, {
  'ceiling.create': 'made a ceiling',
  'ceiling.boundary': 'changed the outline of',
  'ceiling.grid': 'set the grid of',
  'ceiling.grid.clear': 'took the grid off',
  'ceiling.defaults': 'made the default ceiling types',
  'ceiling.type.create': 'made a ceiling type',
  'ceiling.type.layers': 'changed the layers of a ceiling type',
  'ceiling.type.rename': 'renamed a ceiling type',
  'ceiling.type.duplicate': 'copied a ceiling type',
  'ceiling.type.delete': 'deleted a ceiling type',
});
Object.assign(STEP_WORDS, {
  'ceiling.create': 'making a ceiling',
  'ceiling.boundary': 'a ceiling’s outline',
  'ceiling.grid': 'a ceiling’s grid',
  'ceiling.grid.clear': 'taking a ceiling’s grid off',
  'ceiling.defaults': 'making the default ceiling types',
  'ceiling.type.create': 'making a ceiling type',
  'ceiling.type.layers': 'the layers of a ceiling type',
  'ceiling.type.rename': 'renaming a ceiling type',
  'ceiling.type.duplicate': 'copying a ceiling type',
  'ceiling.type.delete': 'deleting a ceiling type',
});

C.ceilingTypes = null; // { key, view } of the open model
C.ceilingTool = { type: null, offset: '2700 mm', mode: 'room', grid: false, width: '600 mm', depth: '1200 mm', angle: '0' };
C.madeCeilingDefaults = null; // the model the default ceiling types were asked for
// The sketch, in the model's millimetres: closed loops (the outline first, then openings), the
// loop being drawn, the point under the pointer, and the outline last sent while it may be turned back.
const CEIL = { loops: [], current: [], at: null, sent: null };

/** The open model's ceiling types and materials; null until they have come. */
function ceilingTypes() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}`;
  if (C.ceilingTypes?.key !== key && C.ceilingTypesLoading !== key) {
    C.ceilingTypesLoading = key;
    api(`${modelUrl(model)}/ceiling-types`)
      .then((view) => {
        C.ceilingTypes = { key, view };
        refreshSide();
      })
      .catch(() => {});
  }
  return C.ceilingTypes?.key.startsWith(`${model}/`) ? C.ceilingTypes.view : null;
}

/** The open model's ceiling types, read now if they have not come yet. */
async function ceilingTypesNow() {
  const view = ceilingTypes();
  if (view) return view;
  const model = C.model.id;
  const fresh = await api(`${modelUrl(model)}/ceiling-types`);
  C.ceilingTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
  return fresh;
}

const chosenCeilingType = (view) => view.types.find((t) => t.id === C.ceilingTool.type) ?? view.types[0] ?? null;

/** Has the server make its default ceiling types, once for a model, where it has none. */
function makeDefaultCeilings() {
  if (!canEdit() || C.madeCeilingDefaults === C.model.id) return;
  C.madeCeilingDefaults = C.model.id;
  manage('ceiling.defaults', {}, 'ceiling-draw');
}

const sketching = () => S.tool === 'ceiling' && C.ceilingTool.mode === 'sketch';

/** What the sketch holds, and the button that sends it. */
function sketchHtml() {
  const [outer, ...holes] = CEIL.loops;
  let state = 'empty';
  let text = 'Click the corners of the ceiling’s outline in the plan. Close it on its first corner or with Enter.';
  if (CEIL.current.length) {
    state = 'open';
    const which = outer ? `Opening ${holes.length + 1}` : 'Outline';
    text = `${which}: ${plural(CEIL.current.length, 'corner')}. Close it on its first corner or with Enter.`;
  } else if (outer) {
    state = 'closed';
    const holeText = holes.length ? `, ${plural(holes.length, 'opening')}` : '';
    text = `Outline: ${plural(outer.length, 'corner')}${holeText}. Click to start an opening, or press Enter to make the ceiling.`;
  }
  const ready = state === 'closed' ? '' : ' disabled';
  return `<p class="pad small muted" data-ceiling-sketch="${state}">${esc(text)}</p><div class="pad"><button class="btn pri sm" data-act="stackceilingmake" data-key="ceiling-make"${ready}>Make ceiling</button></div>`;
}

function ceilingToolHtml() {
  let html = typeSel('layers', 'Ceiling', '<b>New ceiling</b>');
  const view = ceilingTypes();
  if (!view) return `${html}<p class="pad small faint">Loading the ceiling types.</p>`;
  if (!view.types.length) {
    queueMicrotask(makeDefaultCeilings);
    return `${html}<p class="pad small faint" data-ceiling-tool="making">This model has no ceiling types yet, so the default ones are being made.</p>${problem('ceiling-draw')}`;
  }
  const chosen = chosenCeilingType(view);
  const here = levelOf();
  const level = (modelLevels() || []).find((l) => l.id === here)?.name ?? here;
  const tool = C.ceilingTool;
  const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
  const modes = [['room', 'Click in a room'], ['sketch', 'Sketch the outline']]
    .map(([id, name]) => `<option value="${id}"${id === tool.mode ? ' selected' : ''}>${name}</option>`);
  html += '<table class="props" data-ceiling-tool>';
  html += prow('Place', `<select class="inp" data-chg="stackceilingmode" data-key="ceiling-mode" aria-label="How the ceiling is placed">${modes.join('')}</select>`);
  html += prow('Type', `<select class="inp" data-chg="stackceilingtype" data-key="ceiling-type" aria-label="Ceiling type">${types.join('')}</select>`);
  html += prow('Thickness', esc(chosen.thickness));
  html += prow('Level', esc(level));
  html += prow('Height', `<input class="inp" data-chg="stackceilingoffset" data-key="ceiling-offset" value="${esc(tool.offset)}" aria-label="Height of the ceiling’s underside above its level">`);
  html += prow('Grid', `<select class="inp" data-chg="stackceilinggridon" data-key="ceiling-grid-on" aria-label="Whether the ceiling has a grid of tiles"><option value="no"${tool.grid ? '' : ' selected'}>None</option><option value="yes"${tool.grid ? ' selected' : ''}>Tiles</option></select>`);
  if (tool.grid) {
    for (const [key, label] of [['width', 'Tile width'], ['depth', 'Tile depth'], ['angle', 'Angle (degrees)']]) {
      html += prow(label, `<input class="inp" data-chg="stackceilingtool" data-a="${key}" data-key="ceiling-tool-${key}" value="${esc(tool[key])}" aria-label="${esc(label)}">`);
    }
  }
  html += '</table>';
  html += `<span class="lbl">Layers, top first</span><table class="props" data-ceiling-layers="${esc(chosen.id)}">`;
  for (const layer of chosen.layers) {
    html += prow(esc(capital(layer.function)), `${esc(layer.thickness)} · ${esc(materialName(view, layer.material))}`);
  }
  html += '</table>';
  html += tool.mode === 'sketch' ? sketchHtml() : '<p class="pad small muted" data-ceiling-room>Click inside a loop of walls. The ceiling takes the room’s outline.</p>';
  return html + problem('ceiling-draw');
}

CHG.stackceilingmode = (select) => {
  C.ceilingTool.mode = select.value;
  clearSketch();
  refreshSide();
  queueDraw();
};
CHG.stackceilingtype = (select) => {
  C.ceilingTool.type = select.value;
  refreshSide();
};
CHG.stackceilingoffset = (input) => {
  C.ceilingTool.offset = input.value;
};
CHG.stackceilinggridon = (select) => {
  C.ceilingTool.grid = select.value === 'yes';
  refreshSide();
};
CHG.stackceilingtool = (input) => {
  C.ceilingTool[input.dataset.a] = input.value;
};
ACT.stackceilingmake = () => makeCeiling();

/** The arguments every new ceiling shares: its type, its tie and its grid, as chosen in the palette. */
async function newCeiling() {
  let view;
  try {
    view = await ceilingTypesNow();
  } catch (error) {
    say(error.message);
    return null;
  }
  const tool = C.ceilingTool;
  const args = { element: newId('C'), level: levelOf() };
  const type = chosenCeilingType(view);
  if (type) args.type = type.id;
  if (tool.offset.trim() !== '') args.offset = tool.offset;
  if (tool.grid) args.grid = { width: tool.width, depth: tool.depth, angle: Number.parseInt(tool.angle, 10) || 0 };
  return args;
}

/** A click in a room: the point is the ceiling's whole outline, found by the server. */
async function placeInRoom(at) {
  if (C.ceilingTool.mode !== 'room') return;
  if (!canEdit()) {
    say('Open a model in your team’s WIP to add to it.');
    return;
  }
  const args = await newCeiling();
  if (!args) return;
  const point = [snap(at.x), snap(at.y)];
  args.at = [`${point[0]} mm`, `${point[1]} mm`];
  manage('ceiling.create', args, 'ceiling-draw');
}

/** Sends the sketch as it is: what the server makes of it, or why it refuses, comes back through the log. */
async function makeCeiling() {
  if (!CEIL.loops.length || CEIL.current.length || !canEdit()) return;
  const args = await newCeiling();
  if (!args) return;
  const corners = (loop) => loop.map(([x, y]) => ({ at: [`${x} mm`, `${y} mm`] }));
  const [outer, ...holes] = CEIL.loops;
  args.boundary = { outer: corners(outer), holes: holes.map(corners) };
  const sent = { loops: CEIL.loops };
  Object.assign(CEIL, { loops: [], current: [], at: null, sent });
  manage('ceiling.create', args, 'ceiling-draw', () => {
    if (CEIL.sent === sent) CEIL.sent = null;
    queueDraw();
  });
  sketchChanged();
}

function ceilingHint() {
  if (C.ceilingTool.mode === 'room') return '<b>Ceiling:</b> click inside a loop of walls. Choose its type, height and grid in the properties palette. Esc to stop.';
  if (CEIL.current.length) return '<b>Ceiling:</b> click the next corner, or its first corner or Enter to close the loop. Esc to stop.';
  if (CEIL.loops.length) return '<b>Ceiling:</b> click to start an opening, or press Enter to make the ceiling. Esc to stop.';
  return '<b>Ceiling:</b> click the corners of its outline. Choose its type in the properties palette.';
}

function sketchChanged() {
  $('#hint').innerHTML = hint();
  refreshSide();
  queueDraw();
}

function clearSketch() {
  Object.assign(CEIL, { loops: [], current: [], at: null, sent: null });
}

/** Whether a click at `at` (the model's millimetres) is within reach of `point` on screen. */
function nearOnScreen(point, at) {
  const scale = $('#pv-canvas svg')?.getScreenCTM()?.a || 1;
  return Math.hypot(point[0] - at.x, point[1] - at.y) * scale <= CLOSE_PX;
}

function closeLoop() {
  if (!CEIL.current.length) return;
  CEIL.loops.push(CEIL.current);
  CEIL.current = [];
  CEIL.at = null;
  sketchChanged();
}

function sketchClick(event) {
  const at = toModel(event);
  const point = [snap(at.x), snap(at.y)];
  CEIL.sent = null;
  if (CEIL.current.length > 1 && nearOnScreen(CEIL.current[0], at)) {
    closeLoop();
    return;
  }
  const last = CEIL.current.at(-1);
  if (!last || last[0] !== point[0] || last[1] !== point[1]) CEIL.current.push(point);
  sketchChanged();
}

// In the plan, before the elements' own handlers: the sketch's corners.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (!sketching() || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
    event.stopPropagation();
    if (!canEdit()) say('Open a model in your team’s WIP to add to it.');
    else sketchClick(event);
  },
  true,
);
window.addEventListener('pointermove', (event) => {
  if (!sketching() || !CEIL.current.length || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  CEIL.at = [snap(at.x), snap(at.y)];
  queueDraw();
});
// Escape leaves the tool (the demo's own key) and drops the sketch; Enter closes the loop being
// drawn, or makes the ceiling when none is open.
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    clearSketch();
    return;
  }
  if (!sketching() || event.key !== 'Enter' || event.target.closest?.('input,textarea,button')) return;
  event.preventDefault();
  if (CEIL.current.length) closeLoop();
  else makeCeiling();
});
const toolBeforeCeilings = ACT.tool;
ACT.tool = (button) => {
  clearSketch();
  toolBeforeCeilings(button);
};

/** The ribbon: the ceiling tool after the floor tool, in the Build panel. */
function ceilingRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:ceiling')) return;
  let after = build.items.findIndex((item) => item.a === 'architecture:floor');
  if (after < 0) after = build.items.findIndex((item) => item.a === 'architecture:wall');
  build.items.splice(after + 1, 0, DT_('architecture', 'ceiling'));
}

/** The sketch in the plan: closed loops, the loop being drawn up to the pointer, and an outline the server turned back. */
function ceilingSketch() {
  if (!sketching()) return '';
  let svg = '';
  if (CEIL.sent && C.problems.has('ceiling-draw')) {
    svg += `<path class="floor-refused" data-ceiling-refused="${CEIL.sent.loops.length}" d="${CEIL.sent.loops.map(ringPath).join('')}"/>`;
  }
  for (const loop of CEIL.loops) svg += `<path class="ln ap floor-sketch" d="${ringPath(loop)}"/>`;
  const open = CEIL.at && CEIL.current.length ? [...CEIL.current, CEIL.at] : CEIL.current;
  if (open.length > 1) svg += `<polyline class="ln ap dd floor-sketch" points="${open.map((p) => p.join(',')).join(' ')}"/>`;
  for (const [x, y] of CEIL.current) svg += `<circle class="apf floor-sketch" cx="${x}" cy="${y}" r="60"/>`;
  return svg;
}

/** A ceiling's grid as its record holds it, in millimetres, or null. */
function gridOf(e) {
  const grid = e.recipe?.args?.grid;
  if (!grid) return null;
  return {
    width: nearestMm(grid.width),
    depth: nearestMm(grid.depth),
    origin: [nearestMm(grid.origin[0]), nearestMm(grid.origin[1])],
    angle: grid.angle,
  };
}

/**
 * The lines of a grid over the bounding box of `rings`, as `M..L..` path data: lines along the
 * pattern's x axis a depth apart and lines along its y axis a width apart, through its origin, turned by its angle. A drawing aid
 * in floating point, clipped by the caller to the outline; the tiles are counted exactly by the server.
 */
function gridPath(grid, rings) {
  const turn = (grid.angle * Math.PI) / 180;
  const u = [Math.cos(turn), Math.sin(turn)];
  const v = [-Math.sin(turn), Math.cos(turn)];
  const points = rings.flat();
  const along = (axis, [px, py]) => (px - grid.origin[0]) * axis[0] + (py - grid.origin[1]) * axis[1];
  const range = (axis) => {
    const values = points.map((p) => along(axis, p));
    return [Math.min(...values), Math.max(...values)];
  };
  const [u0, u1] = range(u);
  const [v0, v1] = range(v);
  const reach = Math.hypot(u1 - u0, v1 - v0) + 1;
  const first = (low, step) => Math.floor(low / step);
  const last = (high, step) => Math.ceil(high / step);
  const lines = (last(v1, grid.depth) - first(v0, grid.depth) + 1) + (last(u1, grid.width) - first(u0, grid.width) + 1);
  if (!(grid.width > 0 && grid.depth > 0) || lines > MOST_LINES) return '';
  const at = (a, b) => [grid.origin[0] + u[0] * a + v[0] * b, grid.origin[1] + u[1] * a + v[1] * b];
  let d = '';
  for (let k = first(v0, grid.depth); k <= last(v1, grid.depth); k += 1) {
    const [x0, y0] = at(u0 - reach, k * grid.depth);
    const [x1, y1] = at(u1 + reach, k * grid.depth);
    d += `M${x0.toFixed(1)},${y0.toFixed(1)}L${x1.toFixed(1)},${y1.toFixed(1)}`;
  }
  for (let k = first(u0, grid.width); k <= last(u1, grid.width); k += 1) {
    const [x0, y0] = at(k * grid.width, v0 - reach);
    const [x1, y1] = at(k * grid.width, v1 + reach);
    d += `M${x0.toFixed(1)},${y0.toFixed(1)}L${x1.toFixed(1)},${y1.toFixed(1)}`;
  }
  return d;
}

/**
 * A ceiling in the plan: its outline and openings from its recipe as the server filled it (the
 * outline of its display until that has come), a light dashed fill over what is under it, its
 * grid lines clipped to it, and a tag at its middle: its name.
 */
function ceilingInPlan(piece, data, scale) {
  const { el: e } = piece;
  const loops = piece.fill?.recipe?.args?.loops;
  const rings = Array.isArray(loops) && loops.length
    ? loops.map((loop) => loop.map((corner) => [nearestMm(corner.at[0]), nearestMm(corner.at[1])]))
    : [piece.shape.displays[0].outline];
  const top = piece.shape.displays[0]?.parts?.at(-1);
  const colour = top && colourOf(top.material, 'surface');
  const box = around(piece);
  const chosen = S.sel === e.id;
  const outline = rings.map(ringPath).join('');
  let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
  // Its own style, so no shared stylesheet is touched: a light dashed fill that takes no click (a floor or a
  // room under it stays pickable), and a wide invisible stroke along its edges to pick it by.
  const ink = chosen ? 'var(--app-strong)' : 'var(--ink-3)';
  const look = `fill-opacity:.25;fill-rule:evenodd;stroke:${ink};stroke-width:1px;stroke-dasharray:6 3;vector-effect:non-scaling-stroke;pointer-events:none`;
  const filled = colour ? `fill:${esc(colour)};${look}` : `fill:var(--sink);${look}`;
  group += `<path class="body ceiling${chosen ? ' sel' : ''}" data-material="${esc(top?.material ?? '')}" data-holes="${rings.length - 1}" style="${filled}" d="${outline}"/>`;
  group += `<path class="ceiling-hit" style="fill:none;stroke:transparent;stroke-width:10px;vector-effect:non-scaling-stroke;pointer-events:stroke" d="${outline}"/>`;
  const grid = gridOf(e);
  const lines = grid ? gridPath(grid, rings.slice(0, 1)) : '';
  if (lines) {
    const clip = `ceiling-clip-${e.id.replace(/[^A-Za-z0-9_-]/g, '_')}`;
    group += `<clipPath id="${clip}"><path clip-rule="evenodd" d="${outline}"/></clipPath>`;
    group += `<g clip-path="url(#${clip})"><path class="ceiling-grid" data-ceiling-grid-lines="${esc(e.id)}" style="fill:none;stroke:var(--ink-3);stroke-width:1px;vector-effect:non-scaling-stroke;pointer-events:none" d="${lines}"/></g>`;
  }
  const cx = box.x + box.w / 2;
  const cy = box.y + box.d / 2;
  group += `<g class="ceiling-tag" data-ceiling-tag="${esc(e.id)}">${TX(cx, cy, scale, esc(e.name), 'room-name', 260)}</g>`;
  const claim = heldClaim(e.id);
  if (claim) {
    const theirs = claim.person !== C.me.person.id;
    group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
    if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
  }
  return `<g ${data}>${group}</g>`;
}

/** A ceiling's height above its level, as stored. */
function heightOf(e) {
  const stored = e.props?.['@level.offset'];
  return `${stored === undefined ? 0 : storedMm(stored)} mm`;
}

/** A selected ceiling's grid part: its tiles' size, angle and origin, to set, and a button to take it off. */
function gridHtml(e) {
  const editable = canEdit();
  const grid = gridOf(e);
  const off = editable ? '' : ' disabled';
  const field = (key, label, value) => `<input class="inp" data-chg="stackceilinggrid" data-ceiling-field="${key}" data-key="ceiling-grid-${key}" value="${esc(value)}" aria-label="${esc(label)}"${off}>`;
  let html = '<span class="lbl">Grid</span><table class="props" data-ceiling-grid-part>';
  if (!grid && !editable) return `${html}${prow('Now', '<span class="faint">None</span>')}</table>`;
  html += prow('Tile width', field('width', 'Tile width', `${grid?.width ?? 600} mm`));
  html += prow('Tile depth', field('depth', 'Tile depth', `${grid?.depth ?? 1200} mm`));
  html += prow('Angle (degrees)', field('angle', 'Angle of the grid in degrees', grid?.angle ?? 0));
  html += prow('Origin x', field('x', 'Where a grid line crosses, x', grid ? `${grid.origin[0]} mm` : ''));
  html += prow('Origin y', field('y', 'Where a grid line crosses, y', grid ? `${grid.origin[1]} mm` : ''));
  html += '</table>';
  if (editable) {
    html += `<div class="pad"><button class="btn sm" data-act="stackceilinggridset" data-key="ceiling-grid-set">${grid ? 'Set grid' : 'Add grid'}</button>`;
    if (grid) html += ' <button class="btn sm" data-act="stackceilinggridclear" data-key="ceiling-grid-clear">Remove grid</button>';
    html += '</div>';
  }
  return html + problem(`ceiling:${e.id}`);
}

/** What the properties palette shows of a selected ceiling. */
function ceilingProperties(piece) {
  const { el: e, shape, failed } = piece;
  const view = paramsFor(piece);
  const claim = heldClaim(e.id);
  const off = canEdit() ? '' : ' disabled';
  let html = typeSel('layers', 'Ceiling', `<b>${esc(e.name)}</b>`);
  html += '<table class="props" data-ceiling-element>';
  if (view) html += prow('Category', esc(view.category?.name || 'None')) + prow('Type', typeCell(view));
  html += prow('Level', esc(levelName(e)));
  html += prow('Height', `<input class="inp" data-chg="stackceilingheight" data-key="ceiling-height" value="${esc(heightOf(e))}" aria-label="Height of the ceiling’s underside above its level"${off}>`);
  if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  html += gridHtml(e);
  if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
  if (view) {
    html += readOnlyTable(view);
    html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  }
  return html;
}

/** The grid typed in the selected ceiling's palette, as `ceiling.grid` takes it. */
function typedGrid() {
  const get = (key) => document.querySelector(`[data-ceiling-field="${key}"]`)?.value ?? '';
  const grid = { width: get('width'), depth: get('depth'), angle: Number.parseInt(get('angle'), 10) || 0 };
  const [x, y] = [get('x').trim(), get('y').trim()];
  if (x !== '' && y !== '') grid.origin = [x, y];
  return grid;
}
const sendGrid = () => {
  if (S.sel && canEdit()) manage('ceiling.grid', { element: S.sel, grid: typedGrid() }, `ceiling:${S.sel}`);
};
ACT.stackceilinggridset = sendGrid;
CHG.stackceilinggrid = () => {
  if (C.elements.get(S.sel)?.el.recipe?.args?.grid) sendGrid();
};
ACT.stackceilinggridclear = () => {
  if (S.sel && canEdit()) manage('ceiling.grid.clear', { element: S.sel }, `ceiling:${S.sel}`);
};
CHG.stackceilingheight = (input) => {
  const e = C.elements.get(S.sel)?.el;
  if (e && canEdit()) manage('level.tie', { element: e.id, level: levelIdOf(e), offset: input.value }, `ceiling:${e.id}`);
};

/** The properties palette's ceiling part: the ceiling tool's options, or a selected ceiling; '' for anything else. */
function ceilingPalette(piece) {
  if (S.tool === 'ceiling' && canEdit()) return ceilingToolHtml();
  if (piece && isCeiling(piece.el) && !(S.tool === 'wall' && canEdit())) return ceilingProperties(piece);
  return '';
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'ceiling', discipline: 'architecture', order: 25, place: (at) => placeInRoom(at) });
register.ribbons({ id: 'ceiling', order: 12, ribbon: (config) => ceilingRibbon(config) });
register.hints({ id: 'ceiling', order: 35, hint: () => (S.tool === 'ceiling' ? ceilingHint() : undefined) });
register.palettes({ id: 'ceiling', order: 12, palette: (piece) => ceilingPalette(piece) || undefined });
register.elements({ id: 'ceiling', order: 22, owns: isCeiling, inPlan: (piece, data, scale) => ceilingInPlan(piece, data, scale) });
register.drawOrders({ id: 'ceiling', order: 32, drawOrder: (el) => (el.kind === 'ceiling' ? 0.5 : undefined) });
register.layers({ id: 'ceiling-sketch', order: 52, over: () => ceilingSketch() });

export { isCeiling };
