// Tool: the railing tool, a railing in the plan and the properties palette.

import { register } from '../../core/registry.js';
import { C, api, levelOf, modelUrl } from '../../core/state.js';
import { STEPS_PER_MM, around, nearestMm, placementOf, refill, storedMm } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { parametersTable, paramsFor, refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, plural, problem } from '../../core/problems.js';
import { heldClaim, hint, queueDraw, snap, toModel } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { READ_ONLY, readOnlyTable } from './floor.js';

// --- The railing tool, and a railing in the plan and the properties palette ---
// A railing is made three ways, each one edit (crates/construct-railings' FORMAT.md): by clicking
// the points of its path on the plan's level and pressing Enter (`railing.create`; a path of
// straight segments, an arc is refused in the server's words until curved paths come); by picking
// a stair, which it then follows on the side chosen in the properties palette (`railing.stair`);
// or by picking an edge of a floor, which it stands along on the floor's top (`railing.create`
// with that edge's two corners). Its category, the top rail's height and the balusters' spacing
// are chosen there too and sent only when typed: what the server's standard type holds is not
// repeated here. The page checks nothing and lays nothing out: every baluster, rail and post, the
// quantities, and the warnings where a limit of the code is broken (with its clause) are the
// server's (`/railings`, the railing's parameters, `/filled/<element>`), and a refusal is its
// reason, under the options or the railing's own. A selected railing's category, top rail,
// spacing, side and path points are changed there (`railing.type`, `railing.path`) and flipped
// (`railing.flip`), and Delete removes it (`railing.delete`); the balusters are laid out again by
// the server whenever the path, the type or the stair it follows changes.
// In the plan a railing is its pieces as the server filled it: the top rail and the handrail as
// lines along the path, each post and baluster as the profile it is swept from. In 3D it is the
// shape the browser built from the same recipe, as the generic drawing shows any shape.

const RAILING_OP = 'railing';
const CLOSE_PX = 10; // a click this near a floor's edge on screen picks the edge
const isRailing = (record) => record?.kind === 'railing' && record?.recipe?.op === RAILING_OP;
const isStairRecord = (record) => record?.kind === 'stair' && record?.recipe?.op === 'stair';
const isFloorRecord = (record) => record?.kind === 'floor' && record?.recipe?.op === 'floor';
const SIDES = [['both', 'Both sides'], ['left', 'Left going up'], ['right', 'Right going up']];

DTOOLS.architecture.push(['railing', 'Railing', 'family', 'Click the points of its path then press Enter, or pick a stair or the edge of a floor.']);
TOOLNAME.railing = 'railing';
Object.assign(WORDS, {
  'railing.create': 'drew a railing',
  'railing.stair': 'put a railing on a stair',
  'railing.type': 'changed the type of',
  'railing.path': 'changed the path of',
  'railing.flip': 'flipped',
  'railing.delete': 'deleted',
});
Object.assign(STEP_WORDS, {
  'railing.create': 'drawing a railing',
  'railing.stair': 'putting a railing on a stair',
  'railing.type': 'a railing’s type',
  'railing.path': 'a railing’s path',
  'railing.flip': 'flipping a railing',
  'railing.delete': 'deleting a railing',
});

C.railingTool = { category: '', side: 'both', height: '', spacing: '' };
C.railingsView = null; // { key, view } of the open model
C.railingsEpoch = 0; // goes up when an entry may change what `/railings` says
// The sketch, in the model's millimetres: the points clicked and the point under the pointer.
const SKETCH = { points: [], at: null };

/** The open model's railings as the server reads them (its categories and each railing's quantities and warnings); null until they have come. */
function railingsView() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}/${C.railingsEpoch}`;
  if (C.railingsView?.key !== key && C.railingsLoading !== key) {
    C.railingsLoading = key;
    api(`${modelUrl(model)}/railings`)
      .then((view) => {
        C.railingsView = { key, view };
        refreshSide();
        queueDraw();
      })
      .catch(() => {});
  }
  return C.railingsView?.key.startsWith(`${model}/`) ? C.railingsView.view : null;
}

/**
 * An entry arrived for the open model: what a railing's read-back may now be, and, where a stair
 * changed, the railings on this page are filled and built again (the server lays a railing on a
 * stair out from the stair as it stands, and the log names only the stair).
 */
function railingsChanged(entry) {
  if (/^(railing|stair|level|view)\.|^edit\.(undo|redo)$/.test(entry.cmd)) C.railingsEpoch += 1;
  if (/^stair\./.test(entry.cmd)) {
    for (const [id, piece] of C.elements) if (isRailing(piece.el)) refill(entry.target, C.elements, id, piece);
  }
}

/** The part of the type a person typed in the tool's options, as `railing.create`'s and `railing.stair`'s `type`; undefined when none. */
function typedType(tool) {
  const type = {};
  if (tool.height.trim() !== '') type.top_rail = { height: tool.height };
  if (tool.spacing.trim() !== '') type.balusters = { spacing: tool.spacing };
  return Object.keys(type).length ? type : undefined;
}

/** What the options say beside the place a railing is made: category and type. */
function optionArgs() {
  const tool = C.railingTool;
  const args = {};
  const view = railingsView();
  if (view?.categories.some((c) => c.id === tool.category)) args.category = tool.category;
  const type = typedType(tool);
  if (type) args.type = type;
  return args;
}

function railingToolHtml() {
  let html = typeSel('family', 'Railing', '<b>New railing</b>');
  const view = railingsView();
  if (!view) return `${html}<p class="pad small faint">Loading the railing rules.</p>`;
  const tool = C.railingTool;
  const categories = [`<option value=""${tool.category === '' ? ' selected' : ''}>As the place decides</option>`]
    .concat(view.categories.map((c) => `<option value="${esc(c.id)}"${c.id === tool.category ? ' selected' : ''}>${esc(c.name)}</option>`));
  const sides = SIDES.map(([id, name]) => `<option value="${id}"${id === tool.side ? ' selected' : ''}>${name}</option>`);
  const field = (key, label, value, hold) => `<input class="inp" data-chg="railingtool${key}" data-key="railing-${key}" value="${esc(value)}" placeholder="${esc(hold)}" aria-label="${esc(label)}">`;
  html += '<table class="props">';
  const here = levelOf();
  html += prow('Level', esc((modelLevels() || []).find((level) => level.id === here)?.name ?? here));
  html += prow('Category', `<select class="inp" data-chg="railingtoolcategory" data-key="railing-category" aria-label="Where the barrier stands">${categories.join('')}</select>`);
  html += prow('Top rail', field('height', 'Height of the top rail', tool.height, 'standard'));
  html += prow('Spacing', field('spacing', 'Spacing of the balusters', tool.spacing, 'standard'));
  html += prow('On a stair', `<select class="inp" data-chg="railingtoolside" data-key="railing-side" aria-label="Which side of a stair">${sides.join('')}</select>`);
  html += '</table>';
  html += `<p class="pad small muted" data-railing-rules>${esc(view.rules_name)}. The server lays out every baluster and checks the limits.</p>`;
  html += railingSketchHtml();
  return html + problem('railing-draw');
}

/** What the sketch holds, and the button that sends it. */
function railingSketchHtml() {
  const count = SKETCH.points.length;
  const state = count === 0 ? 'empty' : count === 1 ? 'open' : 'ready';
  const text = count === 0
    ? 'Click the points of its path, then press Enter. Or pick a stair, or the edge of a floor.'
    : `Path: ${plural(count, 'point')}. Click the next point, and press Enter to make the railing.`;
  const ready = state === 'ready' ? '' : ' disabled';
  return `<p class="pad small muted" data-railing-sketch="${state}">${esc(text)}</p><div class="pad"><button class="btn pri sm" data-act="stackrailingmake" data-key="railing-make"${ready}>Make railing</button></div>`;
}

CHG.railingtoolcategory = (select) => {
  C.railingTool.category = select.value;
};
CHG.railingtoolside = (select) => {
  C.railingTool.side = select.value;
};
CHG.railingtoolheight = (input) => {
  C.railingTool.height = input.value;
};
CHG.railingtoolspacing = (input) => {
  C.railingTool.spacing = input.value;
};
ACT.stackrailingmake = () => makeSketched();

function railingHint() {
  if (SKETCH.points.length) return '<b>Railing:</b> click the next point, then press Enter to make it. Esc to stop.';
  return '<b>Railing:</b> click the points of its path, or pick a stair or the edge of a floor. Choose its category and type in the properties palette.';
}

function sketchChanged() {
  $('#hint').innerHTML = hint();
  refreshSide();
  queueDraw();
}

function clearSketch() {
  SKETCH.points = [];
  SKETCH.at = null;
}

const mm = (point) => [`${point[0]} mm`, `${point[1]} mm`];

/** Sends the sketched path: what the server makes of it, or why it refuses, comes back through the log. */
function makeSketched() {
  if (SKETCH.points.length < 2 || !canEdit()) return;
  const args = { element: newId('R'), level: levelOf(), path: SKETCH.points.map(mm), ...optionArgs() };
  clearSketch();
  manage('railing.create', args, 'railing-draw');
  sketchChanged();
}

/** A railing on the stair `piece`, on the side the options say. */
function makeOnStair(piece) {
  const args = { element: newId('R'), stair: piece.el.id, side: C.railingTool.side, ...optionArgs() };
  manage('railing.stair', args, 'railing-draw');
}

/** The distance from `at` to the segment from `a` to `b`, in millimetres. */
function distanceTo(at, a, b) {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const along = dx * dx + dy * dy === 0 ? 0 : Math.max(0, Math.min(1, ((at.x - a[0]) * dx + (at.y - a[1]) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(at.x - (a[0] + along * dx), at.y - (a[1] + along * dy));
}

/** The edge of a floor on this plan within reach of a click at `at` (millimetres): `{floor, from, to}` or null. */
function floorEdgeAt(at) {
  const scale = $('#pv-canvas svg')?.getScreenCTM()?.a || 1;
  let best = null;
  for (const piece of C.elements.values()) {
    if (!isFloorRecord(piece.el)) continue;
    const { x, y } = placementOf(piece.el);
    for (const ring of piece.fill?.recipe?.args?.loops ?? []) {
      ring.forEach((corner, k) => {
        const next = ring[(k + 1) % ring.length];
        const [from, to] = [corner, next].map((c) => [x + nearestMm(c.at[0]), y + nearestMm(c.at[1])]);
        const reach = distanceTo(at, from, to);
        if (reach * scale <= CLOSE_PX && (!best || reach < best.reach)) best = { floor: piece, from, to, reach };
      });
    }
  }
  return best;
}

/** A railing along a floor's edge, standing on the floor's top. */
function makeAlongEdge(edge) {
  const stored = edge.floor.el.props?.['@level.offset'];
  const args = { element: newId('R'), level: levelOf(), path: [mm(edge.from), mm(edge.to)], ...optionArgs() };
  if (stored !== undefined) args.offset = `${storedMm(stored)} mm`;
  manage('railing.create', args, 'railing-draw');
}

/**
 * A click in the plan with the railing tool: the first click on a stair makes a railing on it, the
 * first on the edge of a floor makes one along the edge, and any other adds a point to the path.
 */
function placeRailing(at, event) {
  if (!canEdit()) return;
  const point = [snap(at.x), snap(at.y)];
  if (!SKETCH.points.length) {
    const hit = document.elementFromPoint(event.clientX, event.clientY)?.closest('[data-id]');
    const piece = hit && C.elements.get(hit.dataset.id);
    if (piece && isStairRecord(piece.el)) {
      makeOnStair(piece);
      return;
    }
    const edge = floorEdgeAt(at);
    if (edge) {
      makeAlongEdge(edge);
      return;
    }
  }
  const last = SKETCH.points.at(-1);
  if (!last || last[0] !== point[0] || last[1] !== point[1]) SKETCH.points.push(point);
  sketchChanged();
}

window.addEventListener('pointermove', (event) => {
  if (S.tool !== 'railing' || !SKETCH.points.length || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  SKETCH.at = [snap(at.x), snap(at.y)];
  queueDraw();
});
// Escape leaves the tool (the demo's own key) and drops the sketch; Enter makes the railing.
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    clearSketch();
    return;
  }
  if (S.tool !== 'railing' || event.key !== 'Enter' || event.target.closest?.('input,textarea,button')) return;
  event.preventDefault();
  makeSketched();
});
const toolBeforeRailings = ACT.tool;
ACT.tool = (button) => {
  clearSketch();
  toolBeforeRailings(button);
};

/** The ribbon: the railing tool beside the stair tool, in the Build panel. */
function railingRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:railing')) return;
  const stair = build.items.findIndex((item) => item.a === 'architecture:stair');
  build.items.splice(stair + 1, 0, DT_('architecture', 'railing'));
}

/** The sketch in the plan: the points clicked, and the next segment up to the pointer. */
function railingSketch() {
  if (S.tool !== 'railing') return '';
  const open = SKETCH.at && SKETCH.points.length ? [...SKETCH.points, SKETCH.at] : SKETCH.points;
  let svg = '';
  if (open.length > 1) svg += `<polyline class="ln ap dd floor-sketch" data-railing-path points="${open.map((p) => p.join(',')).join(' ')}"/>`;
  for (const [x, y] of SKETCH.points) svg += `<circle class="apf floor-sketch" cx="${x}" cy="${y}" r="60"/>`;
  return svg;
}

/** A point of a piece, in grid steps, as millimetres in the plan: for drawing only. */
const toMm = (p) => [p[0] / STEPS_PER_MM, p[1] / STEPS_PER_MM];

/** The middle of a profile's corners, in the plan. */
function middleOf(profile) {
  const corners = profile.map(toMm);
  return [corners.reduce((sum, c) => sum + c[0], 0) / corners.length, corners.reduce((sum, c) => sum + c[1], 0) / corners.length];
}

const ringPath = (points) => `M${points.map(([x, y]) => `${x},${y}`).join('L')}Z`;

/**
 * A railing in the plan, from its recipe as the server filled it: each piece of top rail and of
 * handrail a line along the path, each post and baluster the profile it is swept from. The pieces'
 * corners are the server's (`pieces`, in grid steps); nothing is laid out here.
 */
function railingInPlan(piece, data, scale) {
  const { el: e } = piece;
  const pieces = piece.fill?.recipe?.args?.pieces;
  const box = around(piece);
  const chosen = S.sel === e.id;
  let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
  if (!Array.isArray(pieces) || !pieces.length) {
    const outline = piece.shape.displays[0]?.outline;
    if (outline?.length) group += `<path class="body railing${chosen ? ' sel' : ''}" d="${ringPath(outline)}"/>`;
    return `<g ${data}>${group}</g>`;
  }
  let rails = '';
  let marks = '';
  for (const p of pieces) {
    if (p.part === 'top-rail' || p.part === 'handrail') {
      const from = middleOf(p.extrude);
      const to = [from[0] + p.by[0] / STEPS_PER_MM, from[1] + p.by[1] / STEPS_PER_MM];
      const hand = p.part === 'handrail';
      group += `<line class="railing-rail${hand ? ' railing-hand' : ''}${chosen ? ' sel' : ''}" data-railing-piece="${esc(p.id)}" x1="${from[0]}" y1="${from[1]}" x2="${to[0]}" y2="${to[1]}"/>`;
      if (!hand) rails += `M${from[0]},${from[1]}L${to[0]},${to[1]}`;
    } else {
      marks += `<path class="railing-${p.part}${chosen ? ' sel' : ''}" data-railing-piece="${esc(p.id)}" d="${ringPath(p.extrude.map(toMm))}"/>`;
    }
  }
  // One body to pick it by: the top rail's lines, drawn wide and clear.
  group += `<path class="body railing${chosen ? ' sel' : ''}" d="${rails}"/>${marks}`;
  const claim = heldClaim(e.id);
  if (claim) {
    const theirs = claim.person !== C.me.person.id;
    group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
    if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
  }
  return `<g ${data}>${group}</g>`;
}

/** A length as stored, `length:q0:<steps>`, in millimetres as typed text. */
const stored = (text) => `${storedMm(text)} mm`;

/** The points of a sketched railing's path, as typed text in millimetres. */
const pathPoints = (e) => (e.recipe.args.path?.points ?? []).map(([x, y]) => [stored(x), stored(y)]);

/** What the properties palette shows of a selected railing. */
function railingProperties(piece) {
  const { el: e, shape, failed } = piece;
  const view = paramsFor(piece);
  const railings = railingsView();
  const info = railings?.railings.find((r) => r.id === e.id);
  const claim = heldClaim(e.id);
  const editable = canEdit();
  const args = e.recipe.args;
  let html = typeSel('family', 'Railing', `<b>${esc(e.name)}</b>`);
  html += '<table class="props" data-railing-element>';
  if (view) html += prow('Category', esc(view.category?.name || 'None'));
  html += prow('Base', esc(levelName(e)));
  const onStair = Boolean(args.path?.stair);
  if (onStair) html += prow('Follows', `<span data-railing-stair>${esc(C.elements.get(args.path.stair)?.el.name ?? args.path.stair)}</span>`);
  if (editable && railings && info) {
    const barriers = railings.categories.map((c) => `<option value="${esc(c.id)}"${c.id === info.category ? ' selected' : ''}>${esc(c.name)}</option>`);
    html += prow('Barrier', `<select class="inp" data-chg="railingcategory" data-key="railing-own-category" aria-label="Where the barrier stands">${barriers.join('')}</select>`);
    html += prow('Top rail', `<input class="inp" data-chg="railingheight" data-key="railing-own-height" value="${esc(stored(args.type.top_rail.height))}" aria-label="Height of the top rail">`);
    html += prow('Spacing', `<input class="inp" data-chg="railingspacing" data-key="railing-own-spacing" value="${esc(stored(args.type.balusters.spacing))}" aria-label="Spacing of the balusters">`);
    if (onStair) {
      const sides = SIDES.map(([id, name]) => `<option value="${id}"${id === args.path.side ? ' selected' : ''}>${name}</option>`);
      html += prow('Side', `<select class="inp" data-chg="railingside" data-key="railing-own-side" aria-label="Which side of the stair">${sides.join('')}</select>`);
    }
  } else {
    html += prow('Top rail', esc(info?.height.shown ?? stored(args.type.top_rail.height)));
    html += prow('Spacing', esc(stored(args.type.balusters.spacing)));
  }
  if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  if (info?.refusal) html += `<div class="note bad small" role="alert" data-railing-refusal>${esc(info.refusal)}</div>`;
  // The limits the code sets that it breaks, in the server's words with the clause.
  for (const warning of info?.laid?.warnings ?? []) {
    html += `<div class="note bad small" data-railing-warning="${esc(warning.kind)}">${esc(warning.words)}</div>`;
  }
  if (editable && !onStair) html += railingPathHtml(e);
  html += problem('railing-edit');
  if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
  if (view) {
    html += readOnlyTable(view);
    html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  }
  if (editable) html += '<div class="pad"><button class="btn sm" data-act="railingflip" data-key="railing-flip">Flip</button> <button class="btn sm" data-act="stackdelete" data-key="railing-delete">Delete railing</button></div>';
  return html;
}

/** A sketched railing's path, a row of two fields for each point, to change it by. */
function railingPathHtml(e) {
  const points = pathPoints(e);
  const field = (i, axis, value) => `<input class="inp" data-chg="railingpoint" data-i="${i}" data-axis="${axis}" data-key="railing-point-${i}-${axis}" value="${esc(value)}" aria-label="Point ${i + 1}, ${axis}">`;
  let html = '<span class="lbl">Path</span><table class="props" data-railing-points>';
  points.forEach(([x, y], i) => {
    const remove = points.length > 2 ? ` <button class="btn sm" data-act="railingdroppoint" data-i="${i}" data-key="railing-drop-${i}" aria-label="Remove point ${i + 1}">×</button>` : '';
    html += prow(`${i + 1}`, `${field(i, 'x', x)} ${field(i, 'y', y)}${remove}`);
  });
  return `${html}</table><div class="pad"><button class="btn sm" data-act="railingaddpoint" data-key="railing-add-point">Add a point</button></div>`;
}

/** Sends the railing's path with `points` (typed text) in place of the one it has. */
function sendPath(points) {
  if (S.sel) manage('railing.path', { element: S.sel, path: points }, 'railing-edit');
}

CHG.railingcategory = (select) => {
  if (S.sel) manage('railing.type', { element: S.sel, category: select.value }, 'railing-edit');
};
CHG.railingheight = (input) => {
  if (S.sel) manage('railing.type', { element: S.sel, type: { top_rail: { height: input.value } } }, 'railing-edit');
  input.blur();
};
CHG.railingspacing = (input) => {
  if (S.sel) manage('railing.type', { element: S.sel, type: { balusters: { spacing: input.value } } }, 'railing-edit');
  input.blur();
};
CHG.railingside = (select) => {
  if (S.sel) manage('railing.path', { element: S.sel, side: select.value }, 'railing-edit');
};
CHG.railingpoint = (input) => {
  const piece = C.elements.get(S.sel);
  if (!piece) return;
  const points = pathPoints(piece.el);
  points[Number(input.dataset.i)][input.dataset.axis === 'x' ? 0 : 1] = input.value;
  sendPath(points);
  input.blur();
};
ACT.railingflip = () => {
  if (S.sel) manage('railing.flip', { element: S.sel }, 'railing-edit');
};
ACT.railingaddpoint = () => {
  const piece = C.elements.get(S.sel);
  if (!piece) return;
  // The new point goes on in the direction of the last segment, as far again.
  const points = pathPoints(piece.el);
  const [[px, py], [lx, ly]] = points.slice(-2).map(([x, y]) => [parseFloat(x), parseFloat(y)]);
  sendPath([...points, [`${lx + (lx - px)} mm`, `${ly + (ly - py)} mm`]]);
};
ACT.railingdroppoint = (button) => {
  const piece = C.elements.get(S.sel);
  if (!piece) return;
  sendPath(pathPoints(piece.el).filter((_, i) => i !== Number(button.dataset.i)));
};

const deleteBeforeRailings = ACT.stackdelete;
ACT.stackdelete = () => {
  const piece = C.elements.get(S.sel);
  if (!piece || !isRailing(piece.el)) {
    deleteBeforeRailings();
    return;
  }
  if (!canEdit()) return;
  manage('railing.delete', { element: S.sel }, 'railing-edit');
  S.sel = null;
  refresh();
};

/** The properties palette's railing part: the railing tool's options, or a selected railing; '' for anything else. */
function railingPalette(piece) {
  if (S.tool === 'railing' && canEdit()) return railingToolHtml();
  if (piece && isRailing(piece.el) && !(S.tool === 'wall' && canEdit())) return railingProperties(piece);
  return '';
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'railing', discipline: 'architecture', order: 140, place: (at, event) => placeRailing(at, event) });
register.ribbons({ id: 'railing', order: 35, ribbon: (config) => railingRibbon(config) });
register.hints({ id: 'railing', order: 45, hint: () => (S.tool === 'railing' ? railingHint() : undefined) });
register.palettes({ id: 'railing', order: 35, palette: (piece) => railingPalette(piece) || undefined });
register.elements({ id: 'railing', order: 55, owns: isRailing, inPlan: (piece, data, scale) => railingInPlan(piece, data, scale), pickOnly: true });
register.drawOrders({ id: 'railing', order: 40, drawOrder: (el) => (isRailing(el) ? 3 : undefined) });
register.layers({ id: 'railing-sketch', order: 55, over: () => railingSketch() });
register.logs({ id: 'railings', order: 45, entry: (entry) => railingsChanged(entry) });

export { isRailing };
