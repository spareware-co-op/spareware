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

// --- Linear dimensions ---
// A dimension (construct-dimensions' FORMAT.md) belongs to one view and is bound to references:
// a wall's face or reference line, a grid line, a door's, window's or opening's centre or edge. It
// is made and changed only by its own commands through the same log: `dimension.create`,
// `dimension.reference.add`, `dimension.reference.remove`, `dimension.move`, `dimension.equal`,
// `dimension.delete`. Its value is never stored and never worked out here: the server works every
// segment out, exactly, from where the references are now, and this page asks for the open view's
// dimensions again whenever the log changes (`GET .../views/<view>/dimensions`) and draws what
// comes back: the offset line, a witness line to each reference, a tick at each, and each segment's
// text, in the size the server gave it (the plain default behind construct-dimensions' `TextSize`),
// scaled to the plan's scale as every other text is. A reference that was deleted or cannot be
// used is the server's warning, in its words, in the palette. The dimension tool picks references
// as a person does, by clicking them in the plan, then the offset line's place; a picked dimension
// has its line dragged, references added and removed, and its segments held equal (stored, not yet
// enforced) in the properties palette. Nothing is checked here: a refusal is the server's reason.

const FEATURE_WORDS = {
  'wall.face.exterior': 'the exterior face of',
  'wall.face.interior': 'the interior face of',
  'wall.line': 'the reference line of',
  'grid.line': 'the grid line',
  centre: 'the centre of',
  'edge.start': 'the start edge of',
  'edge.end': 'the end edge of',
};
// wanted: the view this holds the dimensions of; picks: what the tool has picked so far; adding: the
// dimension whose next reference is being picked; drag: its line being dragged; held: a moved line,
// drawn where it was put until a fetch that started after the server's answer comes back
const DM = { wanted: null, list: [], applied: 0, fetches: 0, timer: null, picks: [], layout: 'horizontal', adding: null, drag: null, held: null };
C.dimensions = DM; // the browser test reads this
Object.assign(WORDS, {
  'dimension.create': 'made a dimension',
  'dimension.reference.add': 'added a reference to a dimension',
  'dimension.reference.remove': 'took a reference from a dimension',
  'dimension.move': 'moved a dimension',
  'dimension.equal': 'set segments of a dimension equal',
  'dimension.delete': 'deleted a dimension',
});
Object.assign(STEP_WORDS, {
  'dimension.create': 'making a dimension',
  'dimension.reference.add': 'adding a reference to a dimension',
  'dimension.reference.remove': 'taking a reference from a dimension',
  'dimension.move': 'moving a dimension',
  'dimension.equal': 'setting segments equal',
  'dimension.delete': 'deleting a dimension',
});

const dimsKey = () => (C.model && C.openView ? `${C.model.id}/${C.openView}` : null);
/** A dimension of the open view, by its ID. */
const ownDimension = (id) => (id && DM.wanted === dimsKey() ? DM.list.find((d) => d.id === id) : undefined);
const mmOfStored = (text) => steps(text) / STEPS_PER_MM;

function forgetDimensions() {
  Object.assign(DM, { wanted: null, list: [], picks: [], adding: null, drag: null, held: null });
  clearTimeout(DM.timer);
}

/** An entry of the log arrived: what a dimension reads may have changed, so the view's are asked for again. */
function dimsChanged() {
  if (!dimsKey()) return;
  clearTimeout(DM.timer);
  DM.timer = setTimeout(loadDims, 120);
}

/** Asks the server for the open view's dimensions, worked out at the log's head. */
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
    answer = error.status === 404 ? { dimensions: [] } : null;
  }
  if (!answer || dimsKey() !== key || started < DM.applied) return;
  DM.applied = started;
  if (DM.held?.settled && started > DM.held.after) DM.held = null;
  const known = new Set(DM.list.map((d) => d.id));
  DM.list = answer.dimensions;
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

/** One dimension in the plan: its offset line, witness lines, ticks and the text of each segment. */
function dimSvg(d, s, slot) {
  const picked = S.sel === d.id;
  const pin = DM.drag?.id === d.id ? DM.drag : DM.held?.id === d.id ? DM.held : null;
  const horizontal = d.layout.kind === 'horizontal';
  const vertical = d.layout.kind === 'vertical';
  const raw = (p) => [mmOfStored(p.x), mmOfStored(p.y)];
  // A line being dragged, or put and not yet answered, is drawn where it is, not where it was.
  const onLine = (p) => {
    const [x, y] = raw(p);
    return pin && horizontal ? [x, pin.value] : pin && vertical ? [pin.value, y] : [x, y];
  };
  const claim = claimedByOther(d.id);
  const marks =
    `data-dim="${esc(d.id)}" data-kind="dimension" data-layout="${esc(d.layout.kind)}" ` +
    `data-line="${horizontal ? mmOfStored(d.layout.y) : vertical ? mmOfStored(d.layout.x) : ''}" ` +
    `data-values="${esc(d.segments.map((x) => x.text ?? '').join('|'))}" data-warnings="${d.warnings.length}"` +
    (picked ? ' data-picked="yes"' : '');
  const witnesses = d.witnesses.map((w) => ({ from: raw(w.from), to: onLine(w.to) }));
  if (!witnesses.length) {
    // Nothing of it can be drawn in place: it is listed in the corner, in the server's words, so it can still be picked.
    const [vx, vy] = VB[S.view];
    const at = [vx + 300 * s, vy + (500 + slot.lost * 380) * s];
    slot.lost += 1;
    const why = d.warnings[0]?.words ?? 'It has nothing to measure.';
    return `<g class="dim own lost${d.warnings.length ? ' bad' : ''}" ${marks}>${TX(at[0], at[1], s, esc(`Dimension: ${why}`), 'dimt bad', 220, 'start')}</g>`;
  }
  // The direction the line runs, as a unit vector that reads left to right, or bottom to top.
  let [ux, uy] = horizontal ? [1, 0] : vertical ? [0, 1] : [mmOfStored(d.layout.direction.x), mmOfStored(d.layout.direction.y)];
  const length = Math.hypot(ux, uy) || 1;
  [ux, uy] = [ux / length, uy / length];
  if (ux < 0 || (ux === 0 && uy > 0)) [ux, uy] = [-ux, -uy];
  const along = (p) => p[0] * ux + p[1] * uy;
  const ends = witnesses.map((w) => w.to).sort((a, b) => along(a) - along(b));
  const [first, last] = [ends[0], ends.at(-1)];
  let g = L(first, last, 'ln hv dim-line');
  const tick = 110 * s;
  for (const w of witnesses) {
    g += L(w.from, w.to, 'ln fn dim-w');
    g += L([w.to[0] - tick, w.to[1] + tick], [w.to[0] + tick, w.to[1] - tick], 'ln hv dim-tick');
  }
  // The text sits on the line's upper side, turned to run along it.
  const [nx, ny] = [uy, -ux];
  const angle = (Math.atan2(uy, ux) * 180) / Math.PI;
  for (const segment of d.segments) {
    if (!segment.text || !segment.middle) continue;
    const [mx, my] = onLine(segment.middle);
    const size = segment.text_height ? mmOfStored(segment.text_height) : 250;
    const [tx, ty] = [mx + nx * 60 * s, my + ny * 60 * s];
    const turn = ` transform="rotate(${n0(angle)} ${n0(tx)} ${n0(ty)})"`;
    g += TX(tx, ty, s, esc(segment.text), 'dimt', size, 'middle', ` data-segment="${segment.from}" data-exact="${segment.exact}"${turn}`);
  }
  if (d.warnings.length) {
    g += TX(last[0] + 120 * s, last[1], s, '!', 'dimt bad', 250, 'start', ' data-dim-warning="yes"');
  }
  if (claim) g += TX(last[0], last[1] + 330 * s, s, `${esc(claim.name)} editing`, 'claimt', 260);
  g += PL([first, last], 'dim-hit');
  return `<g class="dim own${d.warnings.length ? ' bad' : ''}" ${marks}>${g}</g>`;
}

/**
 * The reference a click means, or null with the reason said: the grid line, the side of a wall
 * the click is nearest (its two finish faces and its reference line), a hosted element's centre or
 * the edge the click is nearer. `hit` is the group clicked.
 */
function referenceAt(hit, at) {
  if (hit.classList.contains('datum')) {
    if (hit.dataset.kind !== 'grid') {
      say('A reference plane can’t be dimensioned: pick a grid, a wall, a door or a window.');
      return null;
    }
    return { element: hit.dataset.datum, feature: 'grid.line', name: hit.dataset.name };
  }
  const piece = C.elements.get(hit.dataset.id);
  if (!piece) return null;
  const name = piece.el.name || piece.el.id;
  if (piece.el.kind === 'wall') {
    const feature = wallFeature(piece, at);
    if (!feature) say('That wall isn’t ready to be dimensioned yet.');
    return feature && { element: piece.el.id, feature, name };
  }
  const hosted = hostOf(piece.el);
  if (hosted && piece.shape) return { element: piece.el.id, feature: hostedFeature(piece, hosted.host, at), name };
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
  const count = DM.picks.length;
  if (count < 2) return `<b>Dimension:</b> pick walls, grids, doors or windows, by the side or edge you mean${count ? ` (${count} picked)` : ''}. Esc to stop.`;
  return `<b>Dimension:</b> ${count} picked. Pick another, or click an empty place to put the line there. Esc to stop.`;
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
  if (DM.adding) {
    if (!found) return;
    const dimension = DM.adding;
    DM.adding = null;
    manage('dimension.reference.add', { dimension, reference: { element: found.element, feature: found.feature } }, `dim:${dimension}`);
  } else if (hit) {
    if (found && !DM.picks.some((p) => p.element === found.element && p.feature === found.feature)) {
      DM.picks.push({ ...found, at: [at.x, at.y] });
    }
  } else if (DM.picks.length < 2) {
    say('Pick at least two references first.');
    return;
  } else {
    const horizontal = DM.layout === 'horizontal';
    const axis = horizontal ? 0 : 1;
    // In order along the dimension, so each segment is the stretch between neighbours.
    const picks = DM.picks.slice().sort((a, b) => a.at[axis] - b.at[axis]);
    const layout = horizontal ? { kind: 'horizontal', y: `${snap(at.y)} mm` } : { kind: 'vertical', x: `${snap(at.x)} mm` };
    const args = { dimension: newId('DIM'), view: C.openView, layout, references: picks.map(({ element, feature }) => ({ element, feature })) };
    DM.picks = [];
    manage('dimension.create', args, 'dim-create');
  }
  $('#hint').innerHTML = hint();
  refreshSide();
  queueDraw();
}

/** A dimension was picked in the plan: selected, and its line taken up to be dragged. */
function pickDimension(id) {
  if (S.sel !== id) select(id);
  const d = ownDimension(id);
  if (!canEdit() || !d || d.layout.kind === 'aligned' || !d.witnesses.length) return;
  const value = mmOfStored(d.layout.kind === 'horizontal' ? d.layout.y : d.layout.x);
  DM.held = null;
  DM.drag = { id, kind: d.layout.kind, start: value, value };
}

/** The line let go: where it is goes to the server once, and it is drawn there until the server answers. */
function letGoOfALine() {
  const drag = DM.drag;
  if (!drag) return;
  DM.drag = null;
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
    pickDimension(hit.dataset.dim);
  },
  true,
);
window.addEventListener('pointermove', (event) => {
  if (!DM.drag || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  DM.drag.value = snap(DM.drag.kind === 'horizontal' ? at.y : at.x);
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

/** What the properties palette shows while the dimension tool is in hand. */
function dimToolHtml() {
  let html = typeSel('dim', 'Dimension', '<b>New dimension</b>');
  if (!C.openView) return `${html}<p class="pad small faint" data-dim-tool="no-view">Open a plan view in the project browser first: a dimension belongs to one view.</p>`;
  const option = (value, text) => `<option value="${value}"${DM.layout === value ? ' selected' : ''}>${text}</option>`;
  html += '<table class="props">';
  html += prow('Direction', `<select class="inp" data-chg="stackdimlayout" data-key="dim-layout" aria-label="Direction of the dimension">${option('horizontal', 'Horizontal')}${option('vertical', 'Vertical')}</select>`);
  const picked = DM.picks.map((pick) => `<li>${esc(pickWords(pick))}</li>`).join('');
  html += prow('Picked', picked ? `<ol class="pad" data-dim-picks>${picked}</ol>` : 'Nothing yet');
  html += '</table>';
  if (DM.picks.length) html += '<div class="pad"><button class="btn sm" data-act="stackdimclear">Clear the picks</button></div>';
  html += '<p class="pad small muted">Pick two or more walls (the face or the reference line nearer your click), grids, doors or windows (the centre, or the edge nearer your click), then click where the line goes.</p>';
  return html + problem('dim-create');
}

CHG.stackdimlayout = (select) => {
  DM.layout = select.value;
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
  const kinds = { horizontal: 'Horizontal', vertical: 'Vertical', aligned: 'Aligned' };
  const where = d.layout.kind === 'horizontal' ? `y ${typedLength(d.layout.y)}` : d.layout.kind === 'vertical' ? `x ${typedLength(d.layout.x)}` : 'Along its direction';
  let html = typeSel('dim', 'Dimension', `<b>${esc(d.overall_text ?? 'No value')}</b>`);
  html += `<table class="props">${prow('Direction', esc(kinds[d.layout.kind] ?? d.layout.kind))}${prow('Line', `<span class="mono">${esc(where)}</span>`)}`;
  for (const [i, segment] of d.segments.entries()) {
    html += prow(`Segment ${i + 1}`, `<span data-dim-segment="${i}">${esc(segment.text ?? 'No value')}</span>${segment.value && !segment.exact ? ' <span class="small faint">(nearest step)</span>' : ''}`);
  }
  if (d.segments.length > 1) html += prow('Overall', `<span data-dim-overall>${esc(d.overall_text ?? 'No value')}</span>`);
  const items = d.references.map((reference, i) => {
    const off = canEdit() && d.references.length > 2 ? '' : ' disabled';
    const remove = canEdit() ? ` <button class="btn sm" data-act="stackdimremove" data-a="${i}" data-key="dim-remove:${i}" aria-label="Take out ${esc(reference.words)}"${off}>Remove</button>` : '';
    return `<li data-dim-reference="${i}">${esc(reference.words)}${reference.usable ? '' : ' <span class="small faint">(not usable)</span>'}${remove}</li>`;
  });
  html += prow('References', `<ol class="pad" data-dim-references>${items.join('')}</ol>`);
  for (const equal of d.equalities) {
    const state = equal.equal === null ? 'not known' : equal.equal ? 'equal' : 'not equal now';
    html += prow('Held equal', `segments ${esc(equal.segments.map((n) => n + 1).join(', '))}: ${state}`);
  }
  if (d.warnings.length) html += prow('Warnings', `<ul class="pad" data-dim-warnings>${d.warnings.map((w) => `<li class="note bad small" data-warning="${esc(w.kind)}">${esc(w.words)}</li>`).join('')}</ul>`);
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  if (canEdit()) {
    html += '<div class="pad"><button class="btn sm" data-act="stackdimadd" data-key="dim-add">Add a reference</button>';
    if (d.segments.length > 1) html += ' <button class="btn sm" data-act="stackdimequal" data-key="dim-equal">Hold the segments equal</button>';
    if (d.equalities.length) html += ' <button class="btn sm" data-act="stackdimunequal" data-key="dim-unequal">Let them differ</button>';
    html += '</div><p class="pad small faint">Drag its line to move it. Delete removes it.</p>';
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
  manage('dimension.equal', { dimension: d.id, groups: [d.segments.map((_, i) => i)] }, `dim:${d.id}`);
};
ACT.stackdimunequal = () => {
  const d = ownDimension(S.sel);
  if (!canEdit() || !d) return;
  manage('dimension.equal', { dimension: d.id, groups: [] }, `dim:${d.id}`);
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
