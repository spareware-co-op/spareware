// Tool: linear dimensions.

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, modelUrl, say } from '../../core/state.js';
import { LAYERED, STEPS_PER_MM, around, hostOf, lineOf, steps } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, problem } from '../../core/problems.js';
import { claimedByOther, heldClaim, hint, queueDraw, snap, toModel } from '../../core/plan.js';
import { typedLength } from './grid.js';

// --- Dimensions ---
// A dimension (construct-dimensions' FORMAT.md) belongs to one view and is bound to references:
// a wall's face or reference line, a grid line (straight, or curved for a radius, a diameter or an
// arc length), a door's, window's or opening's centre or edge. It is made and changed only by its
// own commands through the same log: `dimension.create`, `dimension.reference.add`,
// `dimension.reference.remove`, `dimension.move`, `dimension.equal`, `dimension.equalize`,
// `dimension.style`, `dimension.override`, `dimension.delete`, and its styles by
// `dimension.style.create`. Its value is never stored and never worked out here: the server works
// every value out from where the references are now (an angle exactly at a multiple of 45 degrees,
// otherwise to the nearest unit within a stated bound), and this page asks for the open view's
// dimensions again whenever the log changes (`GET .../views/<view>/dimensions`) and draws what
// comes back: the offset line or arc or leader the server worked out, a witness line to each
// reference (stopping short and running past as the dimension's style says), the style's mark at
// each (a tick, an arrow or a dot), and each value's text as the server words it (with the
// dimension's overrides; the true value when asked), in the size its style gives, scaled to the
// plan's scale as every other text is. A reference that was deleted or cannot be used is the
// server's warning, in its words, in the palette. The dimension tool offers each kind and picks
// references as a person does, by clicking them in the plan, then the place it goes: for an angle,
// a radius or an arc length that place is sent as it is and the server works out what it means.
// A picked dimension has its line or arc dragged, references added and removed, its segments
// made equal (the walls, grids and doors move, in one step), its style and its words changed, in
// the properties palette. Nothing is checked here: a refusal is the server's reason.

const FEATURE_WORDS = {
  'wall.face.exterior': 'the exterior face of',
  'wall.face.interior': 'the interior face of',
  'wall.line': 'the reference line of',
  'grid.line': 'the grid line',
  centre: 'the centre of',
  'edge.start': 'the start edge of',
  'edge.end': 'the end edge of',
};
// What the tool makes: the layout it sends, whether its values are read from the first reference,
// how many references it measures (none: two or more), and whether the server works its layout out
// from the place the person clicks.
const DIM_KINDS = {
  horizontal: { label: 'Horizontal', layout: 'horizontal' },
  vertical: { label: 'Vertical', layout: 'vertical' },
  aligned: { label: 'Aligned', layout: 'aligned' },
  'ordinate-x': { label: 'Ordinate, along x', layout: 'horizontal', ordinate: true },
  'ordinate-y': { label: 'Ordinate, along y', layout: 'vertical', ordinate: true },
  angular: { label: 'Angle', layout: 'angular', count: 2, placed: true },
  radial: { label: 'Radius', layout: 'radial', count: 1, placed: true },
  diameter: { label: 'Diameter', layout: 'diameter', count: 1, placed: true },
  arc_length: { label: 'Arc length', layout: 'arc_length', count: 1, placed: true },
};
const KIND_WORDS = { linear: 'Linear', ordinate: 'Ordinate', angular: 'Angle', radial: 'Radius', diameter: 'Diameter', arc_length: 'Arc length' };
// wanted: the view this holds the dimensions of; styles: the model's dimension styles; picks: what
// the tool has picked so far; layout: the kind the tool makes; style: the style it makes them in
// ('' for the first built-in one); adding: the dimension whose next reference is being picked;
// drag: its line or arc being dragged; held: a moved line, drawn where it was put until a fetch
// that started after the server's answer comes back; showTrue: true values drawn in place of
// overridden words
const DM = { wanted: null, list: [], styles: [], applied: 0, fetches: 0, timer: null, picks: [], layout: 'horizontal', style: '', adding: null, drag: null, held: null, showTrue: false };
C.dimensions = DM; // the browser test reads this
Object.assign(WORDS, {
  'dimension.create': 'made a dimension',
  'dimension.reference.add': 'added a reference to a dimension',
  'dimension.reference.remove': 'took a reference from a dimension',
  'dimension.move': 'moved a dimension',
  'dimension.equal': 'set segments of a dimension equal',
  'dimension.equalize': 'made segments of a dimension equal',
  'dimension.style': 'gave a dimension a style',
  'dimension.override': 'changed the words of a dimension',
  'dimension.delete': 'deleted a dimension',
  'dimension.style.create': 'made a dimension style',
  'dimension.style.change': 'changed a dimension style',
  'dimension.style.delete': 'deleted a dimension style',
});
Object.assign(STEP_WORDS, {
  'dimension.create': 'making a dimension',
  'dimension.reference.add': 'adding a reference to a dimension',
  'dimension.reference.remove': 'taking a reference from a dimension',
  'dimension.move': 'moving a dimension',
  'dimension.equal': 'setting segments equal',
  'dimension.equalize': 'making segments equal',
  'dimension.style': 'giving a dimension a style',
  'dimension.override': 'changing the words of a dimension',
  'dimension.delete': 'deleting a dimension',
  'dimension.style.create': 'making a dimension style',
  'dimension.style.change': 'changing a dimension style',
  'dimension.style.delete': 'deleting a dimension style',
});

const dimsKey = () => (C.model && C.openView ? `${C.model.id}/${C.openView}` : null);
/** A dimension of the open view, by its ID. */
const ownDimension = (id) => (id && DM.wanted === dimsKey() ? DM.list.find((d) => d.id === id) : undefined);
const mmOfStored = (text) => steps(text) / STEPS_PER_MM;
const pointOf = (p) => [mmOfStored(p.x), mmOfStored(p.y)];

/** A whole number of steps as typed millimetres, exactly: 6400 is `1 mm`, 3200 is `0.5 mm`. */
function stepsText(n) {
  const sign = n < 0 ? '-' : '';
  const whole = Math.trunc(Math.abs(n) / STEPS_PER_MM);
  const rest = Math.abs(n) % STEPS_PER_MM;
  // A step is 0.00015625 mm: eight places hold any remainder exactly.
  const places = rest ? `.${String(rest * 15625).padStart(8, '0').replace(/0+$/, '')}` : '';
  return `${sign}${whole}${places} mm`;
}

const gcd = (a, b) => (b ? gcd(b, a % b) : Math.abs(a));

function forgetDimensions() {
  Object.assign(DM, { wanted: null, list: [], styles: [], picks: [], adding: null, drag: null, held: null });
  clearTimeout(DM.timer);
}

/** An entry of the log arrived: what a dimension reads may have changed, so the view's are asked for again. */
function dimsChanged() {
  if (!dimsKey()) return;
  clearTimeout(DM.timer);
  DM.timer = setTimeout(loadDims, 120);
}

/** Asks the server for the open view's dimensions, worked out at the log's head, and the model's styles. */
async function loadDims() {
  const key = dimsKey();
  if (!key) return;
  DM.fetches += 1;
  const started = DM.fetches;
  let answer;
  try {
    answer = await api(`${modelUrl(C.model.id)}/views/${encodeURIComponent(C.openView)}/dimensions`);
  } catch (error) {
    if (error.status !== 404) console.warn(`The dimensions couldn’t be read: ${error.message}`);
    answer = error.status === 404 ? { dimensions: [], styles: [] } : null;
  }
  if (!answer || dimsKey() !== key || started < DM.applied) return;
  DM.applied = started;
  if (DM.held?.settled && started > DM.held.after) DM.held = null;
  const known = new Set(DM.list.map((d) => d.id));
  DM.list = answer.dimensions;
  DM.styles = answer.styles ?? [];
  if (S.sel && known.has(S.sel) && !DM.list.some((d) => d.id === S.sel)) {
    S.sel = null;
    renderRibbon();
  }
  refreshSide();
  queueDraw();
}

/** A refusal of one of the page's own dimension edits: what was drawn ahead of it goes back. */
function dimsRefused(where) {
  if (!where.startsWith('dim:')) return;
  DM.held = null;
  DM.drag = null;
  queueDraw();
}

/** Every dimension of the open view, and what the tool has picked; the plan's drawing calls this. */
function dimensionsSvg(s) {
  const key = dimsKey();
  if (DM.wanted !== key) {
    Object.assign(DM, { wanted: key, list: [], held: null, drag: null, picks: [], adding: null });
    if (key) loadDims();
  }
  if (S.tool !== 'dim' && !DM.adding && DM.picks.length) DM.picks = [];
  if (!key) return '';
  const slot = { lost: 0 };
  let svg = '';
  for (const d of DM.list) svg += dimSvg(d, s, slot);
  for (const [i, pick] of DM.picks.entries()) {
    const [x, y] = pick.at;
    svg += `<g class="dim-pick" data-pick="${i}" data-element="${esc(pick.element)}" data-feature="${esc(pick.feature)}"><circle class="bub" cx="${n0(x)}" cy="${n0(y)}" r="${n0(110 * s)}"/>${TX(x, y + 45 * s, s, String(i + 1), 'tn', 150)}</g>`;
  }
  return svg;
}

const unit = ([x, y]) => {
  const length = Math.hypot(x, y) || 1;
  return [x / length, y / length];
};

/** The mark where a witness line meets the dimension's line: `back` is a unit vector into the dimension. */
function dimMark(at, back, s, marker) {
  const [x, y] = at;
  if (marker === 'arrow') {
    const [bx, by] = back;
    const [long, half] = [220 * s, 70 * s];
    const base = [x + bx * long, y + by * long];
    const points = [at, [base[0] - by * half, base[1] + bx * half], [base[0] + by * half, base[1] - bx * half]];
    return PL(points, 'dim-arrow', true);
  }
  if (marker === 'dot') return `<circle class="dim-dot" cx="${n0(x)}" cy="${n0(y)}" r="${n0(45 * s)}"/>`;
  const tick = 110 * s;
  return L([x - tick, y + tick], [x + tick, y - tick], 'ln hv dim-tick');
}

/** A witness line, stopping `gap` short of its reference and running `extension` past its end. */
function dimWitness(from, to, look, s) {
  const [dx, dy] = [to[0] - from[0], to[1] - from[1]];
  const run = Math.hypot(dx, dy);
  if (!run) return '';
  const [ux, uy] = [dx / run, dy / run];
  const gap = Math.min(mmOfStored(look.gap) * s, run);
  const extension = mmOfStored(look.extension) * s;
  return L([from[0] + ux * gap, from[1] + uy * gap], [to[0] + ux * extension, to[1] + uy * extension], 'ln fn dim-w');
}

/** A value's text at `at`, turned to read along `angle` degrees, above its line or in it. */
function dimText(d, segment, at, normal, angle, s, look) {
  const text = DM.showTrue ? segment.value_text : segment.text;
  if (!text) return '';
  const size = segment.text_height ? mmOfStored(segment.text_height) : 250;
  const inline = look.text_place === 'inline';
  const lift = inline ? -size * 0.35 * s : 60 * s;
  const [tx, ty] = [at[0] + normal[0] * lift, at[1] + normal[1] * lift];
  // Read from left to right, or from the bottom up: never upside down.
  let turn = angle;
  if (turn > 90) turn -= 180;
  if (turn <= -90) turn += 180;
  const numbered = d.kind === 'ordinate' ? segment.to : segment.from;
  const extra = ` data-segment="${numbered}" data-exact="${segment.exact}" data-overridden="${segment.overridden}" transform="rotate(${n0(turn)} ${n0(tx)} ${n0(ty)})"`;
  return TX(tx, ty, s, esc(text), `dimt${inline ? ' inline' : ''}`, size, 'middle', extra);
}

/** A linear or ordinate dimension: its offset line through its witness lines' ends. */
function dimLineFigure(d, s, look, witnesses, onLine) {
  const horizontal = d.layout.kind === 'horizontal';
  const vertical = d.layout.kind === 'vertical';
  // The direction the line runs, as a unit vector that reads left to right, or bottom to top.
  let [ux, uy] = unit(horizontal ? [1, 0] : vertical ? [0, 1] : pointOf(d.layout.direction));
  if (ux < 0 || (ux === 0 && uy > 0)) [ux, uy] = [-ux, -uy];
  const along = (p) => p[0] * ux + p[1] * uy;
  const ends = witnesses.map((w) => w.to).sort((a, b) => along(a) - along(b));
  const [first, last] = [ends[0], ends.at(-1)];
  let g = L(first, last, 'ln hv dim-line');
  for (const w of witnesses) g += dimWitness(w.from, w.to, look, s);
  for (const [i, end] of ends.entries()) {
    if (look.marker === 'arrow') {
      if (i > 0) g += dimMark(end, [-ux, -uy], s, 'arrow');
      if (i < ends.length - 1) g += dimMark(end, [ux, uy], s, 'arrow');
    } else {
      g += dimMark(end, [ux, uy], s, look.marker);
    }
  }
  // The text sits on the line's upper side, turned to run along it.
  const normal = [uy, -ux];
  const angle = (Math.atan2(uy, ux) * 180) / Math.PI;
  for (const segment of d.segments) {
    if (segment.middle) g += dimText(d, segment, onLine(segment.middle), normal, angle, s, look);
  }
  return { g, last };
}

/** An angular or arc length dimension: its arc, witness lines and marks, and its text at the arc's middle. */
function dimArcFigure(d, s, look, witnesses) {
  const f = d.figure;
  const [centre, from, to, middle] = [f.centre, f.from, f.to, f.middle].map(pointOf);
  const r = mmOfStored(f.radius);
  let g = `<path class="ln hv dim-line dim-arc" d="M ${n0(from[0])} ${n0(from[1])} A ${n0(r)} ${n0(r)} 0 ${f.large ? 1 : 0} ${f.anticlockwise ? 1 : 0} ${n0(to[0])} ${n0(to[1])}"/>`;
  for (const w of witnesses) g += dimWitness(w.from, w.to, look, s);
  for (const end of [from, to]) g += dimMark(end, unit([middle[0] - end[0], middle[1] - end[1]]), s, look.marker);
  const out = unit([middle[0] - centre[0], middle[1] - centre[1]]);
  const angle = (Math.atan2(out[0], -out[1]) * 180) / Math.PI;
  for (const segment of d.segments) g += dimText(d, segment, middle, out, angle, s, look);
  return { g, last: to };
}

/** A radial or diameter dimension: its line to the curve, its mark there, its centre mark and its text. */
function dimLeaderFigure(d, s, look) {
  const f = d.figure;
  const [centre, from, to] = [f.centre, f.from, f.to].map(pointOf);
  let g = L(from, to, 'ln hv dim-line dim-leader');
  // On a curve a tick across one line reads as nothing: a radius or a diameter has arrows there.
  g += dimMark(to, unit([from[0] - to[0], from[1] - to[1]]), s, look.marker === 'tick' ? 'arrow' : look.marker);
  if (d.kind === 'diameter') g += dimMark(from, unit([to[0] - from[0], to[1] - from[1]]), s, look.marker === 'tick' ? 'arrow' : look.marker);
  if (look.centre_mark) {
    const c = 150 * s;
    g += L([centre[0] - c, centre[1]], [centre[0] + c, centre[1]], 'ln fn dim-centre') + L([centre[0], centre[1] - c], [centre[0], centre[1] + c], 'ln fn dim-centre');
  }
  const run = unit([to[0] - from[0], to[1] - from[1]]);
  const normal = [run[1], -run[0]];
  const angle = (Math.atan2(run[1], run[0]) * 180) / Math.PI;
  for (const segment of d.segments) if (segment.middle) g += dimText(d, segment, pointOf(segment.middle), normal, angle, s, look);
  return { g, last: to };
}

/** One dimension in the plan, as its figure and style say. */
function dimSvg(d, s, slot) {
  const picked = S.sel === d.id;
  const look = d.look ?? { marker: 'tick', text_place: 'above', gap: 'length:q0:0', extension: 'length:q0:0', centre_mark: true };
  const pin = DM.drag?.id === d.id && !DM.drag.point ? DM.drag : DM.held?.id === d.id ? DM.held : null;
  const horizontal = d.layout.kind === 'horizontal';
  const vertical = d.layout.kind === 'vertical';
  // A line being dragged, or put and not yet answered, is drawn where it is, not where it was.
  const onLine = (p) => {
    const [x, y] = pointOf(p);
    return pin && horizontal ? [x, pin.value] : pin && vertical ? [pin.value, y] : [x, y];
  };
  const claim = claimedByOther(d.id);
  const shownText = (segment) => (DM.showTrue ? segment.value_text : segment.text) ?? '';
  const figure = d.figure?.kind ?? 'line';
  const marks =
    `data-dim="${esc(d.id)}" data-kind="dimension" data-dim-kind="${esc(d.kind ?? 'linear')}" data-layout="${esc(d.layout.kind)}" data-figure="${esc(figure)}" ` +
    `data-line="${horizontal ? mmOfStored(d.layout.y) : vertical ? mmOfStored(d.layout.x) : ''}" data-style="${esc(d.style ?? '')}" data-marker="${esc(look.marker)}" ` +
    `data-values="${esc(d.segments.map(shownText).join('|'))}" data-warnings="${d.warnings.length}"` +
    (picked ? ' data-picked="yes"' : '');
  const witnesses = d.witnesses.map((w) => ({ from: pointOf(w.from), to: onLine(w.to) }));
  if (figure === 'none' || (figure === 'line' && !witnesses.length)) {
    // Nothing of it can be drawn in place: it is listed in the corner, in the server's words, so it can still be picked.
    const [vx, vy] = VB[S.view];
    const at = [vx + 300 * s, vy + (500 + slot.lost * 380) * s];
    slot.lost += 1;
    const why = d.warnings[0]?.words ?? 'It has nothing to measure.';
    return `<g class="dim own lost${d.warnings.length ? ' bad' : ''}" ${marks}>${TX(at[0], at[1], s, esc(`Dimension: ${why}`), 'dimt bad', 220, 'start')}</g>`;
  }
  const { g: drawn, last } =
    figure === 'arc' ? dimArcFigure(d, s, look, witnesses) : figure === 'leader' ? dimLeaderFigure(d, s, look) : dimLineFigure(d, s, look, witnesses, onLine);
  let g = drawn;
  if (d.warnings.length) {
    g += TX(last[0] + 120 * s, last[1], s, '!', 'dimt bad', 250, 'start', ' data-dim-warning="yes"');
  }
  if (claim) g += TX(last[0], last[1] + 330 * s, s, `${esc(claim.name)} editing`, 'claimt', 260);
  // A band to pick it by, along its line, arc or leader.
  if (figure === 'arc') {
    const f = d.figure;
    const r = mmOfStored(f.radius);
    const [from, to] = [pointOf(f.from), pointOf(f.to)];
    g += `<path class="dim-hit" d="M ${n0(from[0])} ${n0(from[1])} A ${n0(r)} ${n0(r)} 0 ${f.large ? 1 : 0} ${f.anticlockwise ? 1 : 0} ${n0(to[0])} ${n0(to[1])}"/>`;
  } else if (figure === 'leader') {
    g += PL([pointOf(d.figure.from), pointOf(d.figure.to)], 'dim-hit');
  } else {
    const ends = witnesses.map((w) => w.to);
    g += PL([ends[0], ends.at(-1)], 'dim-hit');
  }
  return `<g class="dim own${d.warnings.length ? ' bad' : ''}" ${marks}>${g}</g>`;
}

/**
 * The reference a click means, or null with the reason said: the grid line, the side of a wall
 * the click is nearest (its two finish faces and its reference line), a hosted element's centre or
 * the edge the click is nearer. `hit` is the group clicked. `dir` is the way the reference's line
 * runs, in steps, where it has one: an aligned dimension measures across the first one picked.
 */
function referenceAt(hit, at) {
  if (hit.classList.contains('datum')) {
    if (hit.dataset.kind !== 'grid') {
      say('A reference plane can’t be dimensioned: pick a grid, a wall, a door or a window.');
      return null;
    }
    const { x0, y0, x1, y1 } = hit.dataset;
    const dir = [(Number(x1) - Number(x0)) * STEPS_PER_MM, (Number(y1) - Number(y0)) * STEPS_PER_MM];
    return { element: hit.dataset.datum, feature: 'grid.line', name: hit.dataset.name, dir };
  }
  const piece = C.elements.get(hit.dataset.id);
  if (!piece) return null;
  const name = piece.el.name || piece.el.id;
  if (piece.el.kind === 'wall') {
    const feature = wallFeature(piece, at);
    if (!feature) say('That wall isn’t ready to be dimensioned yet.');
    const line = piece.el.recipe.op === LAYERED ? lineOf(piece) : null;
    const box = !line && piece.shape ? around(piece) : null;
    const dir = line ? line.d : box ? (box.w >= box.d ? [1, 0] : [0, 1]) : null;
    return feature && { element: piece.el.id, feature, name, dir };
  }
  const hosted = hostOf(piece.el);
  if (hosted && piece.shape) {
    const line = hosted.host.el.recipe.op === LAYERED ? lineOf(hosted.host) : null;
    const dir = line ? [line.d[1], -line.d[0]] : null;
    return { element: piece.el.id, feature: hostedFeature(piece, hosted.host, at), name, dir };
  }
  say('Pick a wall, a grid, a door or a window to dimension.');
  return null;
}

/** Which of a wall's two finish faces and its reference line a click at `at` is nearest, across the wall. */
function wallFeature(piece, at) {
  const line = piece.el.recipe.op === LAYERED ? lineOf(piece) : null;
  let across;
  let near;
  if (line) {
    const { x, y } = piece.el.placement;
    const p = [at.x - x - line.start[0] / STEPS_PER_MM, at.y - y - line.start[1] / STEPS_PER_MM];
    across = (p[0] * line.d[1] - p[1] * line.d[0]) / (Math.hypot(line.d[0], line.d[1]) || 1);
    near = { 'wall.face.exterior': line.exterior / STEPS_PER_MM, 'wall.face.interior': line.interior / STEPS_PER_MM, 'wall.line': 0 };
  } else if (piece.el.recipe.op !== LAYERED && piece.shape) {
    // The old box wall: along x its exterior is at its low y, along y at its high x.
    const box = around(piece);
    const alongX = box.w >= box.d;
    across = alongX ? at.y : at.x;
    const [low, high] = alongX ? [box.y, box.y + box.d] : [box.x, box.x + box.w];
    near = { 'wall.face.exterior': alongX ? low : high, 'wall.face.interior': alongX ? high : low, 'wall.line': (low + high) / 2 };
  } else {
    return null;
  }
  return Object.entries(near).sort((a, b) => Math.abs(a[1] - across) - Math.abs(b[1] - across))[0][0];
}

/** The centre of a hosted element, or the edge nearer the wall's start or end, by where along it a click is. */
function hostedFeature(piece, host, at) {
  const box = around(piece);
  const line = host.el.recipe.op === LAYERED ? lineOf(host) : null;
  const axis = line ? line.axis : host.el.recipe.args.axis === 'y' ? 1 : host.el.recipe.args.axis === 'x' ? 0 : null;
  const a = axis ?? (box.w >= box.d ? 0 : 1);
  const [low, size, here] = a === 0 ? [box.x, box.w, at.x] : [box.y, box.d, at.y];
  const t = size > 0 ? (here - low) / size : 0.5;
  if (t > 1 / 3 && t < 2 / 3) return 'centre';
  const startIsLow = line ? line.unit[a] > 0 : true;
  return t <= 1 / 3 === startIsLow ? 'edge.start' : 'edge.end';
}

/** The words for a reference the page picked, as the server words a reference. */
const pickWords = (pick) => `${FEATURE_WORDS[pick.feature]} ${pick.name}`;

function dimHint() {
  if (!C.openView) return '<b>Dimension:</b> open a plan view in the project browser first: a dimension belongs to one view.';
  if (DM.adding) return '<b>Dimension:</b> pick a wall, grid, door or window to add to it. Esc to stop.';
  const kind = DIM_KINDS[DM.layout] ?? DIM_KINDS.horizontal;
  const count = DM.picks.length;
  if (kind.count === 2) {
    return count < 2
      ? `<b>Angle:</b> pick two walls or grids that cross${count ? ' (1 picked)' : ''}. Esc to stop.`
      : '<b>Angle:</b> click inside the angle you mean, where its arc goes. Esc to stop.';
  }
  if (kind.count === 1) {
    return count < 1 ? `<b>${kind.label}:</b> pick a curved grid. Esc to stop.` : `<b>${kind.label}:</b> click where it goes. Esc to stop.`;
  }
  if (count < 2) return `<b>Dimension:</b> pick walls, grids, doors or windows, by the side or edge you mean${count ? ` (${count} picked)` : ''}. Esc to stop.`;
  return `<b>Dimension:</b> ${count} picked. Pick another, or click an empty place to put the line there. Esc to stop.`;
}

/** The `dimension.create` arguments the picks and a click at `at` make, or null with the reason said. */
function dimCreateArgs(at) {
  const kind = DIM_KINDS[DM.layout] ?? DIM_KINDS.horizontal;
  const count = DM.picks.length;
  if (kind.count ? count !== kind.count : count < 2) {
    say(kind.count === 1 ? 'Pick a curved grid first.' : kind.count === 2 ? 'Pick two walls or grids that cross first.' : 'Pick at least two references first.');
    return null;
  }
  const place = [snap(at.x), snap(at.y)];
  let picks = DM.picks.slice();
  let layout;
  if (kind.placed) {
    layout = { kind: kind.layout, place: { x: `${place[0]} mm`, y: `${place[1]} mm` } };
  } else if (kind.layout === 'aligned') {
    const dir = picks[0].dir;
    if (!dir || (!dir[0] && !dir[1])) {
      say('Pick a wall or a grid first: an aligned dimension measures across it.');
      return null;
    }
    // Across the first reference's line: the smallest whole vector that way.
    const g = gcd(dir[0], dir[1]) || 1;
    const across = [-dir[1] / g, dir[0] / g];
    const reach = (p) => p.at[0] * across[0] + p.at[1] * across[1];
    picks.sort((a, b) => reach(a) - reach(b));
    layout = {
      kind: 'aligned',
      through: { x: `${place[0]} mm`, y: `${place[1]} mm` },
      direction: { x: stepsText(across[0]), y: stepsText(across[1]) },
    };
  } else {
    const horizontal = kind.layout === 'horizontal';
    const axis = horizontal ? 0 : 1;
    // In order along the dimension, so each segment is the stretch between neighbours; an
    // ordinate dimension keeps the first picked first, its origin.
    if (!kind.ordinate) picks.sort((a, b) => a.at[axis] - b.at[axis]);
    else picks = [picks[0], ...picks.slice(1).sort((a, b) => a.at[axis] - b.at[axis])];
    layout = horizontal ? { kind: 'horizontal', y: `${place[1]} mm` } : { kind: 'vertical', x: `${place[0]} mm` };
  }
  const args = { dimension: newId('DIM'), view: C.openView, layout, references: picks.map(({ element, feature }) => ({ element, feature })) };
  if (kind.ordinate) args.ordinate = true;
  if (DM.style) args.style = DM.style;
  return args;
}

function dimClick(event) {
  if (!canEdit()) {
    say('Open a model in your team’s WIP to add to it.');
    return;
  }
  if (!C.openView) {
    say('Open a plan view in the project browser first: a dimension belongs to one view.');
    return;
  }
  const at = toModel(event);
  const hit = event.target.closest('g.element[data-id], g.datum.own');
  const found = hit ? referenceAt(hit, at) : null;
  const kind = DIM_KINDS[DM.layout] ?? DIM_KINDS.horizontal;
  if (DM.adding) {
    if (!found) return;
    const dimension = DM.adding;
    DM.adding = null;
    manage('dimension.reference.add', { dimension, reference: { element: found.element, feature: found.feature } }, `dim:${dimension}`);
  } else if (hit) {
    if (found && !DM.picks.some((p) => p.element === found.element && p.feature === found.feature)) {
      if (kind.count && DM.picks.length >= kind.count) say(`${kind.label === 'Angle' ? 'An angle measures two' : `A ${kind.label.toLowerCase()} measures one`}: click an empty place to put it.`);
      else DM.picks.push({ ...found, at: [at.x, at.y] });
    }
  } else {
    const args = dimCreateArgs(at);
    if (!args) return;
    DM.picks = [];
    manage('dimension.create', args, 'dim-create');
  }
  $('#hint').innerHTML = hint();
  refreshSide();
  queueDraw();
}

/** A dimension was picked in the plan at `at`: selected, and its line or arc taken up to be dragged. */
function pickDimension(id, at) {
  if (S.sel !== id) select(id);
  const d = ownDimension(id);
  if (!canEdit() || !d || d.figure?.kind === 'none' || (d.figure?.kind === 'line' && !d.witnesses.length)) return;
  DM.held = null;
  if (d.layout.kind === 'horizontal' || d.layout.kind === 'vertical') {
    const value = mmOfStored(d.layout.kind === 'horizontal' ? d.layout.y : d.layout.x);
    DM.drag = { id, kind: d.layout.kind, start: value, value };
    return;
  }
  // An aligned line goes through the point it is let go at; an angle, a radius or an arc length
  // is put where it is let go, and the server works out what that means.
  const start = [snap(at.x), snap(at.y)];
  DM.drag = { id, kind: d.layout.kind === 'aligned' ? 'through' : 'place', point: true, start, value: start };
}

/** The line or arc let go: where it is goes to the server once, and a line is drawn there until the server answers. */
function letGoOfALine() {
  const drag = DM.drag;
  if (!drag) return;
  DM.drag = null;
  if (drag.point) {
    if (drag.value[0] !== drag.start[0] || drag.value[1] !== drag.start[1]) {
      const point = { x: `${drag.value[0]} mm`, y: `${drag.value[1]} mm` };
      manage('dimension.move', { dimension: drag.id, [drag.kind]: point }, `dim:${drag.id}`);
    }
    queueDraw();
    return;
  }
  if (drag.value === drag.start) {
    queueDraw();
    return;
  }
  DM.held = { id: drag.id, value: drag.value, settled: false, after: 0 };
  manage('dimension.move', { dimension: drag.id, line: `${drag.value} mm` }, `dim:${drag.id}`, () => {
    if (DM.held?.id !== drag.id) return;
    DM.held.settled = true;
    DM.held.after = DM.fetches;
    dimsChanged();
  });
  queueDraw();
}

// In the plan, before the elements' own handlers: the dimension tool's picks, a reference being added,
// and a pick of a dimension.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
    if (S.tool === 'dim' || (DM.adding && S.tool === 'select')) {
      event.stopPropagation();
      dimClick(event);
      return;
    }
    const hit = S.tool === 'select' && event.target.closest('g.dim.own');
    if (!hit) return;
    event.stopPropagation();
    pickDimension(hit.dataset.dim, toModel(event));
  },
  true,
);
window.addEventListener('pointermove', (event) => {
  if (!DM.drag || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  if (DM.drag.point) DM.drag.value = [snap(at.x), snap(at.y)];
  else DM.drag.value = snap(DM.drag.kind === 'horizontal' ? at.y : at.x);
  queueDraw();
});
window.addEventListener('pointerup', letGoOfALine);
window.addEventListener('pointercancel', letGoOfALine);
document.addEventListener('keydown', (event) => {
  if (event.key !== 'Escape' || (!DM.adding && !DM.picks.length)) return;
  DM.adding = null;
  DM.picks = [];
  queueDraw();
  refreshSide();
});

/** A select of the model's dimension styles, the current one chosen. */
function styleSelect(key, change, current, label) {
  const options = DM.styles
    .map((style) => `<option value="${esc(style.id)}"${style.id === current ? ' selected' : ''}>${esc(style.name)}</option>`)
    .join('');
  return `<select class="inp" data-chg="${change}" data-key="${key}" aria-label="${esc(label)}">${options}</select>`;
}

const TICKS = 'dimension-style.ticks';

/** What the properties palette shows while the dimension tool is in hand. */
function dimToolHtml() {
  let html = typeSel('dim', 'Dimension', '<b>New dimension</b>');
  if (!C.openView) return `${html}<p class="pad small faint" data-dim-tool="no-view">Open a plan view in the project browser first: a dimension belongs to one view.</p>`;
  const option = (value, text) => `<option value="${value}"${DM.layout === value ? ' selected' : ''}>${text}</option>`;
  html += '<table class="props">';
  html += prow('Kind', `<select class="inp" data-chg="stackdimlayout" data-key="dim-layout" aria-label="Kind of dimension">${Object.entries(DIM_KINDS).map(([value, kind]) => option(value, kind.label)).join('')}</select>`);
  if (DM.styles.length) html += prow('Style', styleSelect('dim-tool-style', 'stackdimtoolstyle', DM.style || TICKS, 'Style of new dimensions'));
  const picked = DM.picks.map((pick) => `<li>${esc(pickWords(pick))}</li>`).join('');
  html += prow('Picked', picked ? `<ol class="pad" data-dim-picks>${picked}</ol>` : 'Nothing yet');
  html += '</table>';
  if (DM.picks.length) html += '<div class="pad"><button class="btn sm" data-act="stackdimclear">Clear the picks</button></div>';
  const kind = DIM_KINDS[DM.layout] ?? DIM_KINDS.horizontal;
  const how =
    kind.count === 2
      ? 'Pick two walls or grids that cross, then click inside the angle you mean, where its arc goes.'
      : kind.count === 1
        ? 'Pick a curved grid, then click where the dimension goes.'
        : kind.ordinate
          ? 'Pick the reference the values are read from first, then the others, then click where the line goes.'
          : 'Pick two or more walls (the face or the reference line nearer your click), grids, doors or windows (the centre, or the edge nearer your click), then click where the line goes.';
  html += `<p class="pad small muted">${how}</p>`;
  return html + problem('dim-create');
}

CHG.stackdimlayout = (select) => {
  DM.layout = select.value;
  $('#hint').innerHTML = hint();
  refreshSide();
};
CHG.stackdimtoolstyle = (select) => {
  DM.style = select.value === TICKS ? '' : select.value;
  refreshSide();
};
ACT.stackdimclear = () => {
  DM.picks = [];
  $('#hint').innerHTML = hint();
  refreshSide();
  queueDraw();
};

/** What the properties palette shows of a selected dimension. */
function dimProperties(d) {
  const claim = heldClaim(d.id);
  const line = d.layout.kind === 'horizontal' || d.layout.kind === 'vertical' || d.layout.kind === 'aligned';
  const where =
    d.layout.kind === 'horizontal' ? `y ${typedLength(d.layout.y)}` : d.layout.kind === 'vertical' ? `x ${typedLength(d.layout.x)}` : d.layout.kind === 'aligned' ? 'Along its direction' : null;
  const title = d.overall_text ?? d.segments[0]?.text ?? 'No value';
  let html = typeSel('dim', 'Dimension', `<b>${esc(title)}</b>`);
  html += `<table class="props">${prow('Kind', `<span data-dim-kind>${esc(KIND_WORDS[d.kind] ?? d.kind)}</span>`)}`;
  if (line) html += prow('Direction', esc({ horizontal: 'Horizontal', vertical: 'Vertical', aligned: 'Aligned' }[d.layout.kind]));
  if (where) html += prow('Line', `<span class="mono">${esc(where)}</span>`);
  if (d.layout.radius) html += prow('Arc radius', `<span class="mono">${esc(typedLength(d.layout.radius))}</span>`);
  for (const [i, segment] of d.segments.entries()) {
    const name = d.segments.length > 1 ? (d.kind === 'ordinate' ? `Value ${i + 1}` : `Segment ${i + 1}`) : 'Value';
    const shown = DM.showTrue ? segment.value_text : segment.text;
    const note = segment.overridden && segment.value_text ? ` <span class="small faint" data-dim-true="${i}">(true value ${esc(segment.value_text)})</span>` : segment.value && !segment.exact ? ' <span class="small faint">(nearest step)</span>' : segment.angle && !segment.exact ? ' <span class="small faint">(nearest unit)</span>' : '';
    html += prow(name, `<span data-dim-segment="${i}">${esc(shown ?? 'No value')}</span>${note}`);
  }
  if (d.segments.length > 1 && d.kind === 'linear') html += prow('Overall', `<span data-dim-overall>${esc(d.overall_text ?? 'No value')}</span>`);
  const items = d.references.map((reference, i) => {
    const off = canEdit() && d.references.length > 2 && line ? '' : ' disabled';
    const remove = canEdit() && line ? ` <button class="btn sm" data-act="stackdimremove" data-a="${i}" data-key="dim-remove:${i}" aria-label="Take out ${esc(reference.words)}"${off}>Remove</button>` : '';
    return `<li data-dim-reference="${i}">${esc(reference.words)}${reference.usable ? '' : ' <span class="small faint">(not usable)</span>'}${remove}</li>`;
  });
  html += prow('References', `<ol class="pad" data-dim-references>${items.join('')}</ol>`);
  for (const equal of d.equalities) {
    const state = equal.equal === null ? 'not known' : equal.equal ? 'equal' : equal.holds ? 'equal to the step' : 'not equal now';
    html += prow('Held equal', `<span data-dim-equal>segments ${esc(equal.segments.map((n) => n + 1).join(', '))}: ${state}</span>`);
  }
  if (d.warnings.length) html += prow('Warnings', `<ul class="pad" data-dim-warnings>${d.warnings.map((w) => `<li class="note bad small" data-warning="${esc(w.kind)}">${esc(w.words)}</li>`).join('')}</ul>`);
  if (canEdit() && DM.styles.length) html += prow('Style', styleSelect('dim-style', 'stackdimstyle', d.style, 'Style of the dimension'));
  if (canEdit()) {
    const input = (key, value, label) => `<input class="inp" data-chg="stackdimwords" data-key="${key}" value="${esc(value ?? '')}" aria-label="${esc(label)}">`;
    html += prow('Text in place of the value', input('dim-text', d.overrides?.text, 'Text in place of the value'));
    html += prow('Before the value', input('dim-prefix', d.overrides?.prefix, 'Text before the value'));
    html += prow('After the value', input('dim-suffix', d.overrides?.suffix, 'Text after the value'));
  }
  html += prow('True values', `<label class="small"><input type="checkbox" data-chg="stackdimtrue" data-key="dim-true"${DM.showTrue ? ' checked' : ''}> Show the true values</label>`);
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  if (canEdit()) {
    html += '<div class="pad">';
    if (line) html += '<button class="btn sm" data-act="stackdimadd" data-key="dim-add">Add a reference</button>';
    if (d.segments.length > 1 && d.kind === 'linear') html += ' <button class="btn sm" data-act="stackdimequal" data-key="dim-equal">Make the segments equal</button>';
    if (d.equalities.length) html += ' <button class="btn sm" data-act="stackdimunequal" data-key="dim-unequal">Let them differ</button>';
    html += '</div>';
    html += '<div class="pad"><input class="inp" data-chg="stackdimnewstyle" data-key="dim-new-style" placeholder="A new style from this one" aria-label="Name of a new style made from this one">';
    html += ` <select class="inp" data-key="dim-new-marker" aria-label="Its mark">${['tick', 'arrow', 'dot'].map((m) => `<option value="${m}"${m === d.look?.marker ? ' selected' : ''}>${m === 'tick' ? 'Ticks' : m === 'arrow' ? 'Arrows' : 'Dots'}</option>`).join('')}</select>`;
    html += ` <select class="inp" data-key="dim-new-text" aria-label="Where its text sits">${['above', 'inline'].map((t) => `<option value="${t}"${t === d.look?.text_place ? ' selected' : ''}>${t === 'above' ? 'Text above' : 'Text in the line'}</option>`).join('')}</select></div>`;
    html += '<p class="pad small faint">Drag its line or arc to move it. Delete removes it.</p>';
  }
  return html + problem(`dim:${d.id}`);
}

ACT.stackdimadd = () => {
  if (!canEdit() || !ownDimension(S.sel)) return;
  DM.adding = S.sel;
  $('#hint').innerHTML = hint();
  refreshSide();
};
ACT.stackdimremove = (button) => {
  if (!canEdit() || !ownDimension(S.sel)) return;
  manage('dimension.reference.remove', { dimension: S.sel, at: Number(button.dataset.a) }, `dim:${S.sel}`);
};
ACT.stackdimequal = () => {
  const d = ownDimension(S.sel);
  if (!canEdit() || !d) return;
  manage('dimension.equalize', { dimension: d.id, groups: [d.segments.map((_, i) => i)] }, `dim:${d.id}`);
};
ACT.stackdimunequal = () => {
  const d = ownDimension(S.sel);
  if (!canEdit() || !d) return;
  manage('dimension.equal', { dimension: d.id, groups: [] }, `dim:${d.id}`);
};
CHG.stackdimstyle = (select) => {
  const d = ownDimension(S.sel);
  if (!canEdit() || !d) return;
  manage('dimension.style', { dimension: d.id, style: select.value === TICKS ? null : select.value }, `dim:${d.id}`);
};
CHG.stackdimwords = () => {
  const d = ownDimension(S.sel);
  if (!canEdit() || !d) return;
  // All three as they stand: one left empty is none.
  const args = { dimension: d.id };
  for (const [key, field] of [['dim-text', 'text'], ['dim-prefix', 'prefix'], ['dim-suffix', 'suffix']]) {
    const value = $(`#pv-side input[data-key="${key}"]`)?.value ?? '';
    if (value) args[field] = value;
  }
  manage('dimension.override', args, `dim:${d.id}`);
};
CHG.stackdimtrue = (box) => {
  DM.showTrue = box.checked;
  refreshSide();
  queueDraw();
};
CHG.stackdimnewstyle = (input) => {
  const d = ownDimension(S.sel);
  const name = input.value;
  if (!canEdit() || !d || !name) return;
  const style = newId('DS');
  const marker = $('#pv-side select[data-key="dim-new-marker"]')?.value;
  const textPlace = $('#pv-side select[data-key="dim-new-text"]')?.value;
  manage('dimension.style.create', { style, name, from: d.style, marker, text_place: textPlace }, `dim:${d.id}`, () => {
    manage('dimension.style', { dimension: d.id, style }, `dim:${d.id}`);
  });
};

const deleteBelow = ACT.stackdelete;
ACT.stackdelete = () => {
  const d = ownDimension(S.sel);
  if (!d) {
    deleteBelow();
    return;
  }
  if (!canEdit()) return;
  manage('dimension.delete', { dimension: d.id }, `dim:${d.id}`);
  S.sel = null;
  refresh();
};

/** The ribbon: Delete for a selected dimension. */
function dimRibbon(config) {
  const d = ownDimension(S.sel);
  if (!d || !canEdit() || S.screen !== 'plan') return;
  config.contextual?.push({
    id: 'sel',
    key: `sel:${S.sel}`,
    group: 'Modify',
    label: 'Dimensions',
    panels: [
      { label: 'Dimensions', items: [{ label: 'Delete', icon: 'del', act: 'stackdelete', kbd: 'Delete' }] },
      { label: 'Selection', items: [{ label: 'Deselect', icon: 'x', act: 'deselect' }] },
    ],
  });
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'dim', discipline: 'architecture', order: 60 });
register.ribbons({ id: 'dimension', order: 1020, ribbon: (config) => dimRibbon(config) });
register.hints({ id: 'dimension', order: 20, hint: () => (S.tool === 'dim' || DM.adding ? dimHint() : undefined) });
register.palettes({ id: 'dimension', order: 90, palette: () => (S.tool === 'dim' && canEdit() ? dimToolHtml() : undefined) });
register.selections({ id: 'dimension', order: 10, selection: (id) => (ownDimension(id) ? dimProperties(ownDimension(id)) : undefined) });
register.layers({ id: 'dimensions', order: 40, over: (scale) => dimensionsSvg(scale) });
register.models({ id: 'dimensions', order: 70, forget: () => forgetDimensions() });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get DM() { return DM; }, get dimsChanged() { return dimsChanged; },
  get dimsRefused() { return dimsRefused; },
});
