// Tool: openings and shafts (a hole in a wall, a hole in a floor, a shaft through the floors).

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, levelOf, say } from '../../core/state.js';
import {
  LAYERED, STEPS_PER_MM, around, hostOf, lineOf, nearestMm, placementOf, steps, storedMm,
} from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, plural, problem } from '../../core/problems.js';
import { heldClaim, queueDraw, snap, toModel } from '../../core/plan.js';
import { modelLevels } from '../project/levels.js';
import { onWall } from './wall.js';
import { isFloor, ringPath, sketchChanged } from './floor.js';

// --- Openings and shafts: a hole in a wall, a hole in a floor, a shaft through the floors ---
// Three tools make them, one edit each (construct-openings' FORMAT.md; the commands are
// construct-api's `cloud/openings.rs`). The Opening tool takes a click on a wall, as the door
// tool does: the side clicked, the click the middle of the opening, its width, height and sill
// typed in the properties palette; `opening.place`. The Floor opening tool and the Shaft tool take
// two clicks, the opposite corners of a rectangle drawn in the plan: the floor under the first
// corner is the one `opening.draw` cuts, and a shaft (`shaft.create`) runs between the two levels
// chosen in the properties palette, each with an offset. A selected opening or shaft shows its
// size, place or reach in the palette, and a change is `opening.size`, `opening.move`,
// `opening.outline`, `shaft.outline` or `shaft.reach`; Delete is `opening.delete` or
// `shaft.delete`. The page checks nothing: a refusal is the server's words, beside the tool or
// the field, and the page draws nothing wrong because it draws only what the log holds. In the
// plan an opening in a wall is the rectangle it opens through its wall, a floor opening and a
// shaft their outlines, and a floor is drawn with the holes the server says cut it
// (`/filled/<floor>`, `cut_by`); in 3D the wall and the floor are what the browser's geometry
// builds with them, cut where the package cuts (the kernel's).

const WALL_OPENING_OP = 'opening.wall';
const FLOOR_OPENING_OP = 'opening.floor';
const SHAFT_OP = 'shaft';
const ownOp = (record, op) => record?.recipe?.owner === 'construct' && record.recipe.op === op;
const isWallOpening = (record) => ownOp(record, WALL_OPENING_OP);
const isFloorOpening = (record) => ownOp(record, FLOOR_OPENING_OP);
const isShaft = (record) => ownOp(record, SHAFT_OP);
const isVoid = (record) => isWallOpening(record) || isFloorOpening(record) || isShaft(record);

DTOOLS.architecture.push(
  ['opening', 'Opening', 'window', 'Click on a wall to cut an opening through it. Set its size in the properties palette.'],
  ['floor-opening', 'Floor opening', 'room', 'Click two opposite corners inside a floor to cut an opening through it.'],
  ['shaft', 'Shaft', 'cube', 'Click two opposite corners of its outline. Choose the levels it runs between in the properties palette.'],
);
Object.assign(TOOLNAME, { opening: 'opening', 'floor-opening': 'floor opening', shaft: 'shaft' });
const RECT_TOOLS = ['floor-opening', 'shaft'];
Object.assign(WORDS, {
  'opening.place': 'cut an opening in a wall:',
  'opening.draw': 'cut an opening in a floor:',
  'opening.size': 'resized',
  'opening.move': 'moved',
  'opening.outline': 'redrew',
  'opening.delete': 'deleted',
  'shaft.create': 'made a shaft:',
  'shaft.outline': 'redrew',
  'shaft.reach': 'changed where it starts and ends:',
  'shaft.delete': 'deleted',
});
Object.assign(STEP_WORDS, {
  'opening.place': 'cutting an opening in a wall',
  'opening.draw': 'cutting an opening in a floor',
  'opening.size': 'the size of an opening',
  'opening.move': 'moving an opening',
  'opening.outline': 'the outline of an opening',
  'opening.delete': 'deleting an opening',
  'shaft.create': 'making a shaft',
  'shaft.outline': 'the outline of a shaft',
  'shaft.reach': 'the reach of a shaft',
  'shaft.delete': 'deleting a shaft',
});

C.openingTool = { width: '1000 mm', height: '1200 mm', sill: '900 mm' }; // typed, as the server reads them
C.shaftTool = { level: null, offset: '-300 mm', top: null, topOffset: '0 mm' };
// The rectangle being drawn for a floor opening or a shaft: its first corner and the point under the pointer.
const RECT = { from: null, at: null };

/** The ribbon: the opening tools beside the family tool, in the Build panel. */
function openingRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:opening')) return;
  const family = build.items.findIndex((item) => item.a === 'architecture:family');
  build.items.splice(family + 1, 0, ...['opening', 'floor-opening', 'shaft'].map((tool) => DT_('architecture', tool)));
}

/** A typed width in millimetres, for centring an opening on a click; 1000 when it does not read. */
function typedMm(text) {
  const number = Number.parseFloat(text);
  if (!Number.isFinite(number)) return 1000;
  return /\d\s*m\s*$/.test(text) && !/mm\s*$/.test(text) ? number * 1000 : number;
}

/** The levels, the one the shaft starts on and the one it ends on, as the tool offers them. */
function shaftChoice() {
  const levels = modelLevels() || [];
  const known = (id) => id && levels.some((level) => level.id === id);
  const level = known(C.shaftTool.level) ? C.shaftTool.level : levelOf();
  const above = levels[levels.findIndex((l) => l.id === level) + 1]?.id ?? level;
  const top = known(C.shaftTool.top) ? C.shaftTool.top : above;
  return { levels, level, top };
}

const levelOptions = (levels, chosen) =>
  levels.map((l) => `<option value="${esc(l.id)}"${l.id === chosen ? ' selected' : ''}>${esc(l.name)}</option>`).join('');

/** The properties palette of the three tools. */
function openingToolHtml() {
  if (S.tool === 'opening') {
    let html = typeSel('window', 'Opening', '<b>New opening in a wall</b>');
    html += '<table class="props">';
    for (const [key, label] of [['width', 'Width'], ['height', 'Height'], ['sill', 'Sill height']]) {
      html += prow(label, `<input class="inp" data-chg="stackopeningtool" data-a="${key}" data-key="opening-tool-${key}" value="${esc(C.openingTool[key])}" aria-label="${label} of the opening">`);
    }
    html += '</table><p class="pad small muted">Click on a wall. The side you click is the side it goes on, and the click is the middle of the opening.</p>';
    return html + problem('opening-place');
  }
  if (S.tool === 'floor-opening') {
    const html = typeSel('room', 'Opening', '<b>New opening in a floor</b>');
    const text = RECT.from ? 'Click the opposite corner.' : 'Click one corner of the opening, inside a floor, then the opposite corner.';
    return `${html}<p class="pad small muted" data-opening-sketch="${RECT.from ? 'open' : 'empty'}">${esc(text)}</p>${problem('opening-draw')}`;
  }
  const { levels, level, top } = shaftChoice();
  let html = typeSel('cube', 'Shaft', '<b>New shaft</b>');
  html += '<table class="props">';
  html += prow('Starts on', `<select class="inp" data-chg="stackshafttool" data-a="level" data-key="shaft-tool-level" aria-label="Level the shaft starts on">${levelOptions(levels, level)}</select>`);
  html += prow('Start offset', `<input class="inp" data-chg="stackshafttool" data-a="offset" data-key="shaft-tool-offset" value="${esc(C.shaftTool.offset)}" aria-label="Offset of the shaft’s start from its level">`);
  html += prow('Ends on', `<select class="inp" data-chg="stackshafttool" data-a="top" data-key="shaft-tool-top" aria-label="Level the shaft ends on">${levelOptions(levels, top)}</select>`);
  html += prow('End offset', `<input class="inp" data-chg="stackshafttool" data-a="topOffset" data-key="shaft-tool-top-offset" value="${esc(C.shaftTool.topOffset)}" aria-label="Offset of the shaft’s end from its level">`);
  html += '</table>';
  const text = RECT.from ? 'Click the opposite corner.' : 'Click one corner of the shaft, then the opposite corner. It cuts every floor it passes through.';
  return `${html}<p class="pad small muted" data-shaft-sketch="${RECT.from ? 'open' : 'empty'}">${esc(text)}</p>${problem('shaft-create')}`;
}

CHG.stackopeningtool = (input) => {
  C.openingTool[input.dataset.a] = input.value;
};
CHG.stackshafttool = (control) => {
  C.shaftTool[control.dataset.a] = control.value;
  refreshSide();
};

function openingHint() {
  if (S.tool === 'opening') return '<b>Opening:</b> click on a wall to cut an opening through it. Set its size in the properties palette.';
  if (S.tool === 'floor-opening') return RECT.from ? '<b>Floor opening:</b> click the opposite corner. Esc to stop.' : '<b>Floor opening:</b> click one corner, inside a floor.';
  return RECT.from ? '<b>Shaft:</b> click the opposite corner. Esc to stop.' : '<b>Shaft:</b> click one corner of its outline. Choose its levels in the properties palette.';
}

/** The wall under a click, with the margin the door tool gives. */
function wallAt(at) {
  return [...C.elements.values()].find((piece) => {
    if (piece.el.kind !== 'wall' || !piece.shape) return false;
    const line = piece.el.recipe.op === LAYERED ? lineOf(piece) : null;
    if (line && line.axis === null) {
      // A wall at an angle is its line and its thickness, not the box round it.
      const { x, y } = piece.el.placement;
      const p = [at.x - x - line.start[0] / STEPS_PER_MM, at.y - y - line.start[1] / STEPS_PER_MM];
      const length = Math.hypot(line.d[0], line.d[1]);
      const alongIt = (p[0] * line.d[0] + p[1] * line.d[1]) / length;
      const right = (p[0] * line.d[1] - p[1] * line.d[0]) / length;
      const [low, high] = [line.exterior, line.interior].map((offset) => offset / STEPS_PER_MM).sort((a, b) => a - b);
      return alongIt >= -300 && alongIt <= length / STEPS_PER_MM + 300 && right >= low - 300 && right <= high + 300;
    }
    const box = around(piece);
    return at.x >= box.x - 300 && at.x <= box.x + box.w + 300 && at.y >= box.y - 300 && at.y <= box.y + box.d + 300;
  });
}

/** A click on a wall with the Opening tool: the side clicked, the click the middle of the opening. */
function placeOpening(at) {
  const wall = wallAt(at);
  if (!wall) {
    say('Click on a wall to cut an opening in it.');
    return;
  }
  const spot = onWall(wall, at, typedMm(C.openingTool.width));
  if (!spot) {
    say('That wall has no side to cut an opening through.');
    return;
  }
  const args = {
    element: newId('O'),
    host: wall.el.id,
    side: spot.exterior ? 'exterior' : 'interior',
    along: `${spot.offset} mm`,
    sill: C.openingTool.sill,
    width: C.openingTool.width,
    height: C.openingTool.height,
  };
  manage('opening.place', args, 'opening-place');
}

/** The floor whose box holds a point, if one does. */
function floorAt(at) {
  return [...C.elements.values()].find((piece) => {
    if (!isFloor(piece.el) || !piece.shape) return false;
    const box = around(piece);
    return at.x >= box.x && at.x <= box.x + box.w && at.y >= box.y && at.y <= box.y + box.d;
  });
}

/** The corners of a rectangle as the commands take an outline, in typed millimetres. */
const rectOutline = ([ax, ay], [bx, by]) =>
  [[ax, ay], [bx, ay], [bx, by], [ax, by]].map(([x, y]) => [`${x} mm`, `${y} mm`]);

function rectClick(event) {
  const at = toModel(event);
  const point = [snap(at.x), snap(at.y)];
  if (!RECT.from) {
    Object.assign(RECT, { from: point, at: point });
    sketchChanged();
    return;
  }
  const [from, to] = [RECT.from, point];
  if (from[0] === to[0] || from[1] === to[1]) {
    say('Click the opposite corner, away from the first, so the outline has an area.');
    return;
  }
  Object.assign(RECT, { from: null, at: null });
  if (S.tool === 'shaft') {
    const { level, top } = shaftChoice();
    const args = { element: newId('S'), outline: rectOutline(from, to), level, top };
    if (C.shaftTool.offset.trim() !== '') args.offset = C.shaftTool.offset;
    if (C.shaftTool.topOffset.trim() !== '') args.top_offset = C.shaftTool.topOffset;
    manage('shaft.create', args, 'shaft-create');
  } else {
    const floor = floorAt({ x: from[0], y: from[1] });
    if (floor) manage('opening.draw', { element: newId('O'), host: floor.el.id, outline: rectOutline(from, to) }, 'opening-draw');
    else say('Start the opening’s outline inside a floor.');
  }
  sketchChanged();
}

// In the plan, before the elements' own handlers: the two corners of a rectangle.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (!RECT_TOOLS.includes(S.tool) || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
    event.stopPropagation();
    if (!canEdit()) say('Open a model in your team’s WIP to add to it.');
    else rectClick(event);
  },
  true,
);
window.addEventListener('pointermove', (event) => {
  if (!RECT_TOOLS.includes(S.tool) || !RECT.from || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  RECT.at = [snap(at.x), snap(at.y)];
  queueDraw();
});
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && RECT.from) {
    Object.assign(RECT, { from: null, at: null });
    queueDraw();
  }
});
const toolBeforeOpenings = ACT.tool;
ACT.tool = (button) => {
  Object.assign(RECT, { from: null, at: null });
  toolBeforeOpenings(button);
};

/** The rectangle being drawn, from its first corner to the pointer. */
function rectSketch() {
  if (!RECT_TOOLS.includes(S.tool) || !RECT.from) return '';
  const [ax, ay] = RECT.from;
  const [bx, by] = RECT.at ?? RECT.from;
  return `<rect class="void sketch" x="${Math.min(ax, bx)}" y="${Math.min(ay, by)}" width="${Math.abs(bx - ax)}" height="${Math.abs(by - ay)}"/>`;
}

/** An opening or a shaft in the plan: the rectangle a wall opening opens through its wall, or an outline. */
function voidInPlan(piece, data, scale) {
  const { el: e } = piece;
  const chosen = S.sel === e.id;
  const paint = `body void ${isShaft(e) ? 'shaft' : 'opening'}${chosen ? ' sel' : ''}`;
  const { x, y } = placementOf(e);
  let box;
  let drawn;
  if (isWallOpening(e)) {
    const host = hostOf(e)?.host;
    const line = host && lineOf(host);
    const thickness = line ? Math.abs(line.exterior - line.interior) / STEPS_PER_MM : (host?.el.recipe.args.thickness ?? 200);
    const wide = storedMm(e.recipe.args.width);
    const seatLine = piece.shape.displays[0]?.outline;
    if (line && line.axis === null && seatLine?.length === 2) {
      // In a wall at an angle the opening is seated: its shape's outline is its line on its side
      // of the wall, and it opens through the wall's thickness to the left of that line.
      const length = Math.hypot(line.d[0], line.d[1]);
      const across = [(-line.d[1] / length) * thickness, (line.d[0] / length) * thickness];
      const corners = [seatLine[0], seatLine[1], [seatLine[1][0] + across[0], seatLine[1][1] + across[1]], [seatLine[0][0] + across[0], seatLine[0][1] + across[1]]]
        .map(([px, py]) => [x + Math.round(px), y + Math.round(py)]);
      const xs = corners.map((p) => p[0]);
      const ys = corners.map((p) => p[1]);
      box = { x: Math.min(...xs), y: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), d: Math.max(...ys) - Math.min(...ys) };
      drawn = `<path class="${paint}" d="${ringPath(corners)}"/>`;
    } else {
      const [w, d] = e.recipe.args.axis === 'y' ? [thickness, wide] : [wide, thickness];
      box = { x, y, w, d };
      drawn = RC(x, y, w, d, paint);
    }
  } else {
    const ring = (e.recipe.args.outline || []).map(([px, py]) => [x + nearestMm(px), y + nearestMm(py)]);
    const xs = ring.map((p) => p[0]);
    const ys = ring.map((p) => p[1]);
    box = { x: Math.min(...xs), y: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), d: Math.max(...ys) - Math.min(...ys) };
    drawn = `<path class="${paint}" d="${ringPath(ring)}"/>`;
  }
  let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
  group += drawn;
  const claim = heldClaim(e.id);
  if (claim) {
    const theirs = claim.person !== C.me.person.id;
    group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
    if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
  }
  return `<g ${data}>${group}</g>`;
}

/** A length stored in grid steps as millimetres, as typed. */
const stepsMm = (steps) => `${Number((steps / STEPS_PER_MM).toFixed(3))} mm`;

/** The corners of an outline as an axis-aligned rectangle, `[x0, y0, x1, y1]` in grid steps, or null for any other outline. */
function rectangleOf(outline) {
  if (!Array.isArray(outline) || outline.length !== 4) return null;
  const xs = [...new Set(outline.map((p) => p[0]))];
  const ys = [...new Set(outline.map((p) => p[1]))];
  return xs.length === 2 && ys.length === 2 ? [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] : null;
}

/** What the properties palette shows of a selected opening or shaft: its size, its place, its reach. */
function voidProperties(piece) {
  const { el } = piece;
  const mine = !C.readOnly && canEdit();
  const where = esc(el.id);
  const field = (key, shown, chg, a, label) =>
    mine
      ? `<input class="inp" data-chg="${chg}" data-a="${a}" data-key="${key}" value="${esc(shown)}" aria-label="${esc(label)}">`
      : esc(shown);
  let html = `<table class="props" data-opening-properties="${where}">`;
  if (isWallOpening(el)) {
    const hosted = hostOf(el);
    html += prow('Width', field('opening-width', stepsMm(steps(el.recipe.args.width)), 'stackopeningsize', 'width', 'Width of the opening'));
    html += prow('Height', field('opening-height', stepsMm(steps(el.recipe.args.height)), 'stackopeningsize', 'height', 'Height of the opening'));
    const along = el.props?.['@host.along'];
    const sill = el.props?.['@host.up'];
    if (along !== undefined) html += prow('Along its wall', field('opening-along', `${storedMm(along)} mm`, 'stackopeningmove', 'along', 'Along its wall'));
    if (sill !== undefined) html += prow('Sill height', field('opening-sill', `${storedMm(sill)} mm`, 'stackopeningmove', 'sill', 'Sill height'));
    const face = hosted?.face ?? (el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
    if (face) html += prow('Side', `${esc(face.signature ?? '')}${mine ? ' <button class="btn sm" data-act="stackopeningflip">Flip</button>' : ''}`);
  } else {
    const rectangle = rectangleOf(el.recipe.args.outline);
    if (rectangle) {
      const [x0, y0, x1, y1] = rectangle;
      const cmd = isShaft(el) ? 'shaft' : 'opening';
      html += prow('From x', field(`${cmd}-x`, stepsMm(x0), 'stackopeningrect', 'x', 'Left edge'));
      html += prow('From y', field(`${cmd}-y`, stepsMm(y0), 'stackopeningrect', 'y', 'Top edge'));
      html += prow('Width', field(`${cmd}-w`, stepsMm(x1 - x0), 'stackopeningrect', 'w', 'Width'));
      html += prow('Depth', field(`${cmd}-d`, stepsMm(y1 - y0), 'stackopeningrect', 'd', 'Depth'));
    } else {
      html += prow('Outline', esc(plural((el.recipe.args.outline || []).length, 'corner')));
    }
  }
  if (isShaft(el)) {
    const levels = modelLevels() || [];
    const offset = (key) => `${storedMm(el.props?.[key] ?? 'length:q0:0')} mm`;
    const select = (key, chosen, label) =>
      mine
        ? `<select class="inp" data-chg="stackshaftreach" data-a="${key}" data-key="shaft-${key}" aria-label="${label}">${levelOptions(levels, chosen)}</select>`
        : esc(levels.find((l) => l.id === chosen)?.name ?? chosen ?? '');
    html += prow('Starts on', select('level', el.props?.['@level'], 'Level the shaft starts on'));
    html += prow('Start offset', field('shaft-offset', offset('@level.offset'), 'stackshaftreach', 'offset', 'Offset of the shaft’s start'));
    html += prow('Ends on', select('top', el.props?.['@top.level'], 'Level the shaft ends on'));
    html += prow('End offset', field('shaft-top-offset', offset('@top.offset'), 'stackshaftreach', 'top_offset', 'Offset of the shaft’s end'));
  }
  html += '</table>';
  return html + problem(`opening:${el.id}`);
}

const openingWhere = () => `opening:${S.sel}`;
CHG.stackopeningsize = (input) => {
  if (S.sel && canEdit()) manage('opening.size', { element: S.sel, [input.dataset.a]: input.value }, openingWhere());
  input.blur();
};
CHG.stackopeningmove = (input) => {
  if (S.sel && canEdit()) manage('opening.move', { element: S.sel, [input.dataset.a]: input.value }, openingWhere());
  input.blur();
};
ACT.stackopeningflip = () => {
  const face = (C.elements.get(S.sel)?.el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
  if (S.sel && canEdit() && face) manage('opening.move', { element: S.sel, side: face.signature === 'interior' ? 'exterior' : 'interior' }, openingWhere());
};
/** A rectangle's field changed: the outline is sent again with that edge moved. */
CHG.stackopeningrect = (input) => {
  const piece = C.elements.get(S.sel);
  const rectangle = piece && rectangleOf(piece.el.recipe.args.outline);
  if (!rectangle || !canEdit()) return;
  const mm = (steps) => steps / STEPS_PER_MM;
  const [x0, y0, x1, y1] = rectangle.map(mm);
  const typed = Number.parseFloat(input.value);
  const next = { x: x0, y: y0, w: x1 - x0, d: y1 - y0 };
  next[input.dataset.a] = typed;
  const corners = rectOutline([next.x, next.y], [next.x + next.w, next.y + next.d]);
  const cmd = isShaft(piece.el) ? 'shaft.outline' : 'opening.outline';
  manage(cmd, { element: S.sel, outline: corners }, openingWhere());
  input.blur();
};
CHG.stackshaftreach = (control) => {
  const piece = C.elements.get(S.sel);
  if (!piece || !canEdit()) return;
  const props = piece.el.props || {};
  const length = (key) => `${storedMm(props[key] ?? 'length:q0:0')} mm`;
  const args = {
    element: S.sel,
    level: props['@level'],
    offset: length('@level.offset'),
    top: props['@top.level'],
    top_offset: length('@top.offset'),
  };
  args[control.dataset.a] = control.value;
  manage('shaft.reach', args, openingWhere());
  control.blur();
};

const deleteBeforeOpenings = ACT.stackdelete;
ACT.stackdelete = () => {
  const piece = C.elements.get(S.sel);
  if (!piece || !isVoid(piece.el)) {
    deleteBeforeOpenings();
    return;
  }
  if (!canEdit()) return;
  manage(isShaft(piece.el) ? 'shaft.delete' : 'opening.delete', { element: S.sel }, openingWhere());
  S.sel = null;
  refresh();
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'opening', discipline: 'architecture', order: 100, place: (at) => placeOpening(at) });
register.tools({ id: 'floor-opening', discipline: 'architecture', order: 110 });
register.tools({ id: 'shaft', discipline: 'architecture', order: 120 });
register.ribbons({ id: 'opening', order: 60, ribbon: (config) => openingRibbon(config) });
register.hints({ id: 'opening', order: 100, hint: () => (S.tool === 'opening' || RECT_TOOLS.includes(S.tool) ? openingHint() : undefined) });
register.palettes({
  id: 'opening',
  order: 80,
  palette: () => ((S.tool === 'opening' || RECT_TOOLS.includes(S.tool)) && canEdit() ? openingToolHtml() : undefined),
});
register.properties({ id: 'opening', order: 20, properties: (piece) => (isVoid(piece.el) ? voidProperties(piece) : undefined) });
register.elements({ id: 'opening', order: 30, owns: isVoid, inPlan: (piece, data, scale) => voidInPlan(piece, data, scale), drags: (el, hosted) => hosted });
register.elements({ id: 'wall-opening', order: 80, owns: isWallOpening, move: 'opening.move' });
register.layers({ id: 'rect-sketch', order: 60, over: () => rectSketch() });

// What files loaded before this one use from it, bound once it has loaded.
provide({ get isWallOpening() { return isWallOpening; }, get wallAt() { return wallAt; } });
