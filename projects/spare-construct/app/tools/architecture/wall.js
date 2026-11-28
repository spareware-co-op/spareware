// Tool: the wall tool.

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, levelOf, modelUrl, say } from '../../core/state.js';
import { LAYERED, STEPS_PER_MM, around, lineOf } from '../../core/shapes.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, capital, manage, newId, problem } from '../../core/problems.js';
import { hint, queueDraw, snap } from '../../core/plan.js';
import { modelLevels } from '../project/levels.js';

// --- The wall tool ---
// A wall is drawn as a line between two clicked points, of the wall type chosen in the properties
// palette, its base on the plan's level and its top on the level chosen there or a height typed
// there: one `wall.draw` edit (construct-model's FORMAT.md, "Walls"). The wall types, their layers
// and the colours their materials are drawn in come from the server; where a model has no wall
// types, the page sends the server's default ones through the log (`wall.type.create`). A refusal
// is the server's reason, under the tool's options.

C.wallTypes = null; // { key, view } of the open model
C.wallTool = { type: null, top: undefined, height: '3000 mm' }; // top: a level's ID, '' for a height, undefined for the level above
C.madeDefaults = null; // the model the default wall types were sent for

/** The open model's wall types, materials and defaults; null until they have come. */
function wallTypes() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}`;
  if (C.wallTypes?.key !== key && C.wallTypesLoading !== key) {
    C.wallTypesLoading = key;
    api(`${modelUrl(model)}/wall-types`)
      .then((view) => {
        C.wallTypes = { key, view };
        refreshSide();
        queueDraw();
      })
      .catch(() => {});
  }
  return C.wallTypes?.key.startsWith(`${model}/`) ? C.wallTypes.view : null;
}

/** A material's colour where a view cuts it (`cut`) or seen beyond (`surface`); null for a material not listed. */
function colourOf(material, which) {
  const found = wallTypes()?.materials.find((m) => m.id === material);
  return found ? found[which] : null;
}

const materialName = (view, id) => view.materials.find((m) => m.id === id)?.name ?? id;

function chosenWallType(view) {
  return view.types.find((t) => t.id === C.wallTool.type) ?? view.types[0] ?? null;
}

/** The top the tool draws to: a level's ID, or '' for a height. By default the level above the plan's. */
function wallTop(levels) {
  const here = levelOf();
  const others = (levels || []).filter((level) => level.id !== here);
  const chosen = C.wallTool.top;
  if (chosen === '' || others.some((level) => level.id === chosen)) return chosen;
  const at = (levels || []).findIndex((level) => level.id === here);
  return (at >= 0 && levels[at + 1]?.id) || '';
}

/** Sends the server's default wall types, once for a model, where it has none. */
function makeDefaultWalls(view) {
  if (!canEdit() || C.madeDefaults === C.model.id) return;
  C.madeDefaults = C.model.id;
  for (const args of view.defaults) manage('wall.type.create', args, 'wall-draw');
}

function wallToolHtml() {
  let html = typeSel('wall', 'Wall', '<b>New wall</b>');
  const view = wallTypes();
  if (!view) return `${html}<p class="pad small faint">Loading the wall types.</p>`;
  if (!view.types.length) {
    queueMicrotask(() => makeDefaultWalls(view));
    return `${html}<p class="pad small faint" data-wall-tool="making">This model has no wall types yet, so the default ones are being made.</p>${problem('wall-draw')}`;
  }
  const chosen = chosenWallType(view);
  const levels = modelLevels() || [];
  const here = levelOf();
  const top = wallTop(levels);
  const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
  const tops = levels
    .filter((level) => level.id !== here)
    .map((level) => `<option value="${esc(level.id)}"${level.id === top ? ' selected' : ''}>${esc(level.name)}</option>`);
  tops.push(`<option value=""${top === '' ? ' selected' : ''}>A height</option>`);
  const base = levels.find((level) => level.id === here)?.name ?? here;
  html += '<table class="props">';
  html += prow('Type', `<select class="inp" data-chg="stackwalltype" data-key="wall-type" aria-label="Wall type">${types.join('')}</select>`);
  html += prow('Thickness', esc(chosen.thickness));
  html += prow('Base', esc(base));
  html += prow('Top', `<select class="inp" data-chg="stackwalltop" data-key="wall-top" aria-label="Top of the wall">${tops.join('')}</select>`);
  if (top === '') {
    html += prow('Height', `<input class="inp" data-chg="stackwallheight" data-key="wall-height" value="${esc(C.wallTool.height)}" aria-label="Height of the wall">`);
  }
  html += '</table>';
  html += `<span class="lbl">Layers, exterior first</span><table class="props" data-layers="${esc(chosen.id)}">`;
  for (const layer of chosen.assembly.layers) {
    const wraps = layer.wraps ? ' · wraps' : '';
    html += prow(esc(capital(layer.function)), `${esc(layer.thickness)} · ${esc(materialName(view, layer.material))}${wraps}`);
  }
  html += '</table>';
  const wrap = chosen.assembly.wrap ?? { ends: 'none', inserts: 'none' };
  html += `<span class="lbl">Wrapping of the type</span><table class="props" data-type-wrap="${esc(chosen.id)}">`;
  html += prow('At ends', wrapSelect('stacktypewrap', 'ends', WRAP_ENDS, wrap.ends, ''));
  html += prow('At inserts', wrapSelect('stacktypewrap', 'inserts', WRAP_INSERTS, wrap.inserts, ''));
  html += '</table>';
  html += '<p class="pad small muted">Pick two points in the plan. Walking from the first to the second, the exterior is on the right. Hold Shift on the second for any angle.</p>';
  return html + problem('wall-draw');
}

// --- A layered wall's joins and wrapping (job 6.1) ---
// The server works every joint out (`filled`'s `joins`: the walls joined and why the others are
// not); this palette shows what it says and sends a person's choices: how each joint is made and
// which wall runs through (`wall.join`; a joint's choice is kept by the wall of the lower ID),
// whether joins are allowed at each end (`wall.join.end`), and the wall's own wrapping over its
// type's (`wall.wrap`). The type's wrapping is in the wall tool (`wall.type.wrap`). A refusal is
// the server's reason, under the rows. Nothing is worked out here.

const JOIN_KINDS = [
  ['mitre', 'Mitre'],
  ['butt', 'Butt'],
  ['square-off', 'Square off'],
];
const WRAP_ENDS = [
  ['none', 'None'],
  ['exterior', 'Exterior'],
  ['interior', 'Interior'],
];
const WRAP_INSERTS = [...WRAP_ENDS, ['both', 'Both']];
/** The wall that keeps the choice of the joint of walls `a` and `b`: the one of the lower ID. */
const keeperOf = (a, b) => (a <= b ? a : b);

/** The choice kept for the joint of walls `a` and `b` (`{kind, first}`), or null for none. */
function jointChoice(a, b) {
  const keeper = keeperOf(a, b);
  const other = keeper === a ? b : a;
  return C.elements.get(keeper)?.el.recipe?.args?.joining?.with?.[other] ?? null;
}

/** A select of wrapping choices for `which` (`ends` or `inserts`), `current` chosen. */
function wrapSelect(chg, which, choices, current, extra) {
  const off = canEdit() ? '' : ' disabled';
  const options = choices
    .map(([key, label]) => `<option value="${key}"${key === current ? ' selected' : ''}>${label}</option>`)
    .join('');
  return `<select class="inp" data-chg="${chg}" data-wrap="${which}"${off} aria-label="Wrapping at ${which}">${extra}${options}</select>`;
}

function wallJoinsHtml(piece) {
  const e = piece.el;
  const joins = piece.fill?.recipe?.args?.joins ?? {};
  const own = e.recipe.args.joining ?? {};
  const off = canEdit() ? '' : ' disabled';
  let html = `<span class="lbl">Joins</span><table class="props" data-joins="${esc(e.id)}">`;
  for (const other of joins.with ?? []) {
    const choice = jointChoice(e.id, other);
    const kind = choice?.kind ?? 'mitre';
    const first = choice?.first ?? '';
    const name = C.elements.get(other)?.el.name ?? other;
    const options = JOIN_KINDS.map(([key, label]) => `<option value="${key}"${key === kind ? ' selected' : ''}>${label}</option>`).join('');
    const through = first ? ` <span class="small faint" data-through="${esc(first)}">${esc(C.elements.get(first)?.el.name ?? first)} runs through</span>` : '';
    html += prow(
      `With ${esc(name)}`,
      `<select class="inp" data-chg="stackjoint" data-joint="${esc(other)}" data-kind="${esc(kind)}" data-first="${esc(first)}"${off} aria-label="The joint with ${esc(name)}">${options}</select>${through} <button class="btn sm" data-act="stackjointswitch" data-joint="${esc(other)}"${off}>Switch order</button>`,
    );
  }
  if (!(joins.with ?? []).length) html += prow('Joined with', 'No wall');
  for (const [end, label] of [['start', 'At its start'], ['end', 'At its end']]) {
    const allowed = !(own.disallow ?? []).includes(end);
    html += prow(
      label,
      `<select class="inp" data-chg="stackjoinend" data-end="${end}" data-allowed="${allowed}"${off} aria-label="Joins ${label.toLowerCase()}"><option value="yes"${allowed ? ' selected' : ''}>Joins allowed</option><option value="no"${allowed ? '' : ' selected'}>Joins not allowed</option></select>`,
    );
  }
  html += '</table>';
  for (const why of joins.unjoined ?? []) html += `<p class="pad small faint" data-unjoined>${esc(why)}</p>`;
  html += problem(`joins:${e.id}`);
  const wrapping = e.recipe.args.wrapping ?? {};
  const byType = '<option value="type">By its type</option>';
  html += `<span class="lbl">Wrapping of this wall</span><table class="props" data-wall-wrap="${esc(e.id)}">`;
  html += prow('At ends', wrapSelect('stackwallwrap', 'ends', WRAP_ENDS, wrapping.ends ?? 'type', byType));
  html += prow('At inserts', wrapSelect('stackwallwrap', 'inserts', WRAP_INSERTS, wrapping.inserts ?? 'type', byType));
  html += '</table>';
  return html + problem(`wrap:${e.id}`);
}

CHG.stackjoint = (select) => {
  if (!S.sel) return;
  const kind = select.value;
  const args = { element: S.sel, with: select.dataset.joint, kind };
  // A butt or a square-off keeps the wall that runs through, this one if none was chosen.
  if (kind !== 'mitre') args.first = select.dataset.first || S.sel;
  manage('wall.join', args, `joins:${S.sel}`);
};
ACT.stackjointswitch = (button) => {
  if (!S.sel) return;
  const other = button.dataset.joint;
  const choice = jointChoice(S.sel, other);
  const kind = choice && choice.kind !== 'mitre' ? choice.kind : 'butt';
  // With no choice kept, the wall of the lower ID runs through first.
  const now = choice?.first ?? keeperOf(S.sel, other);
  manage('wall.join', { element: S.sel, with: other, kind, first: now === S.sel ? other : S.sel }, `joins:${S.sel}`);
};
CHG.stackjoinend = (select) => {
  if (S.sel) manage('wall.join.end', { element: S.sel, at: select.dataset.end, allow: select.value === 'yes' }, `joins:${S.sel}`);
};
CHG.stackwallwrap = (select) => {
  if (S.sel) manage('wall.wrap', { element: S.sel, [select.dataset.wrap]: select.value }, `wrap:${S.sel}`);
};
CHG.stacktypewrap = (select) => {
  const view = wallTypes();
  const chosen = view && chosenWallType(view);
  if (chosen) manage('wall.type.wrap', { type: chosen.id, [select.dataset.wrap]: select.value }, 'wall-draw');
};

CHG.stackwalltype = (select) => {
  C.wallTool.type = select.value;
  refreshSide();
};
CHG.stackwalltop = (select) => {
  C.wallTool.top = select.value;
  refreshSide();
};
CHG.stackwallheight = (input) => {
  C.wallTool.height = input.value;
};

/** The open model's wall types, read now if they have not come yet. */
async function wallTypesNow() {
  const view = wallTypes();
  if (view) return view;
  const model = C.model.id;
  const fresh = await api(`${modelUrl(model)}/wall-types`);
  C.wallTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
  return fresh;
}

/**
 * The wall tool's second point draws the wall: straight along x or y, whichever the second point
 * is further along, as the plan draws walls, of the chosen type, from the plan's level to the
 * chosen top. What the server makes of it, or its reason for refusing, comes back through the log.
 */
async function placeWall(at, anyAngle = false) {
  const point = [snap(at.x), snap(at.y)];
  if (!S.pend) {
    S.pend = point;
    $('#hint').innerHTML = hint();
    return;
  }
  const [ax, ay] = S.pend;
  S.pend = null;
  $('#hint').innerHTML = hint();
  // A wall runs along x or y, whichever is nearer the pointer; with Shift held, to the point itself, at any angle.
  const along = Math.abs(point[0] - ax) >= Math.abs(point[1] - ay);
  const end = anyAngle ? point : along ? [point[0], ay] : [ax, point[1]];
  const level = levelOf();
  let view;
  try {
    view = await wallTypesNow();
  } catch (error) {
    say(error.message);
    return;
  }
  const args = { element: newId('W'), start: [`${ax} mm`, `${ay} mm`], end: [`${end[0]} mm`, `${end[1]} mm`], level };
  const type = chosenWallType(view);
  if (type) args.type = type.id;
  const top = wallTop(modelLevels());
  if (top === '') args.height = C.wallTool.height;
  else args.top = top;
  manage('wall.draw', args, 'wall-draw');
}

/**
 * Where a click lands on a wall, for a door: the wall's side clicked (`exterior` or `interior`),
 * how far along the wall from its start the door's near end is when the click is its middle, how
 * thick the wall is, and the axis the wall runs along. A layered wall reads its line and faces from
 * its filled recipe; the old box runs along x, its exterior at its low y side, or along y, its
 * exterior at its high x side.
 */
function onWall(wall, at, width) {
  if (wall.el.recipe.op === LAYERED) {
    const line = lineOf(wall);
    if (!line) return null;
    const { x, y } = wall.el.placement;
    const p = [snap(at.x) - x - line.start[0] / STEPS_PER_MM, snap(at.y) - y - line.start[1] / STEPS_PER_MM];
    const length = Math.hypot(line.d[0], line.d[1]);
    const alongIt = (p[0] * line.d[0] + p[1] * line.d[1]) / length;
    const right = (p[0] * line.d[1] - p[1] * line.d[0]) / length;
    const middle = (line.exterior + line.interior) / 2 / STEPS_PER_MM;
    const exterior = right > middle === line.exterior > line.interior;
    return {
      exterior,
      offset: Math.round(alongIt - width / 2),
      thickness: Math.ceil(Math.abs(line.exterior - line.interior) / STEPS_PER_MM),
      axis: line.axis === 1 ? 'y' : 'x',
    };
  }
  const box = around(wall);
  const along = box.w >= box.d;
  const middle = along ? box.y + box.d / 2 : box.x + box.w / 2;
  return {
    exterior: along ? at.y < middle : at.x > middle,
    offset: (along ? snap(at.x) - box.x : snap(at.y) - box.y) - width / 2,
    thickness: along ? box.d : box.w,
    axis: along ? 'x' : 'y',
  };
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.disciplines({ id: 'architecture', label: 'Architecture', order: 10 });
register.tools({ id: 'wall', discipline: 'architecture', order: 10, place: (at, event) => placeWall(at, event.shiftKey) });
register.hints({
  id: 'wall',
  order: 70,
  hint: () => (S.tool === 'wall' ? (S.pend ? '<b>Wall:</b> pick the end point. Esc to stop.' : '<b>Wall:</b> pick a start point. Choose its type in the properties palette.') : undefined),
});
register.palettes({ id: 'wall', order: 60, palette: () => (S.tool === 'wall' && canEdit() ? wallToolHtml() : undefined) });

// What files loaded before this one use from it, bound once it has loaded.
provide({ get colourOf() { return colourOf; }, get wallJoinsHtml() { return wallJoinsHtml; } });

export { colourOf, materialName, onWall, wallTop };
