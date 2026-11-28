// Tool: grids and reference planes.

import { late, provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, modelUrl, say } from '../../core/state.js';
import { packaged } from '../../core/open.js';
import { shapeFor } from '../../core/shapes.js';
import { WORDS, edit, watchSince } from '../../core/channel.js';
import { STEP_WORDS, UNDOING } from '../../core/undo.js';
import { paramsFor, refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, problem } from '../../core/problems.js';
import { claimedByOther, heldClaim, hint, queueDraw, snap, toModel } from '../../core/plan.js';

// --- Grids and reference planes ---
// A grid line or a reference plane is a record of the model (construct-grids' FORMAT.md), made and
// changed only by its own commands through the same log: `grid.create`, `grid.rename`,
// `grid.reshape`, `grid.delete`, `plane.rename`, `plane.delete`. They stand on no level, so the plan's
// area never lists them: this finds the open model's by kind, keeps them apart from the elements
// (so nothing that counts or picks elements sees them), and draws them under everything, a linked
// model's as they came, seen and never picked. A grid is a thin chain line with its name in a bubble
// at its start, a plane a dashed line, in the demo's own datum classes (the lightest pen in a muted
// colour with a chain or a dashed pattern, as construct-styles' defaults give grids and planes); an
// arc grid is the chain of chords the geometry builds of it. The grid tool sends two picked points
// and no name: the server gives it the next one. Nothing is checked here: a refusal is the
// server's reason, beside what caused it.

const DATUM_KINDS = ['grid', 'refplane'];
const isDatum = (record) => DATUM_KINDS.includes(record?.kind);
C.datums = new Map(); // model id -> Map of id -> { el, shape, failed, hash, seq }: grids and planes
// drag: an end being dragged; held: a reshape sent, drawn until the server answers; preview: the
// pointer while a grid's first point is picked; after: the last grid this person made or renamed
const G = { drag: null, held: null, preview: null, after: null };

DTOOLS.architecture.push(['grid', 'Grid', 'grid', 'Pick both ends of a grid line. The server gives it the next name.']);
TOOLNAME.grid = 'grid';
Object.assign(WORDS, {
  'grid.create': 'made a grid',
  'grid.rename': 'renamed a grid',
  'grid.move': 'moved a grid',
  'grid.reshape': 'moved an end of a grid',
  'grid.delete': 'deleted a grid',
  'plane.create': 'made a reference plane',
  'plane.rename': 'renamed a reference plane',
  'plane.move': 'moved a reference plane',
  'plane.delete': 'deleted a reference plane',
});
Object.assign(STEP_WORDS, {
  'grid.create': 'making a grid',
  'grid.rename': 'renaming a grid',
  'grid.move': 'moving a grid',
  'grid.reshape': 'moving an end of a grid',
  'grid.delete': 'deleting a grid',
  'plane.create': 'making a reference plane',
  'plane.rename': 'renaming a reference plane',
  'plane.move': 'moving a reference plane',
  'plane.delete': 'deleting a reference plane',
});

/** A grid or a plane of the open model, by its ID. */
const ownDatum = (id) => (id && C.model ? C.datums.get(C.model.id)?.get(id) : undefined);

function forgetDatums() {
  C.datums.clear();
  Object.assign(G, { drag: null, held: null, preview: null, after: null });
}

/** Reads the grids and planes of the open model and of each live-linked one. */
async function loadDatums() {
  const models = [C.model?.id, ...C.links.filter((link) => link.kind === 'live').map((link) => link.model)];
  try {
    for (const model of models.filter(Boolean)) {
      for (const kind of DATUM_KINDS) {
        const seen = await api(`${modelUrl(model)}/kind/${kind}`);
        if (seen.through < C.head) watchSince(model, seen.through);
        await Promise.all(
          Object.entries(seen.pieces).map(async ([id, hash]) =>
            keepDatum(model, id, await api(`${modelUrl(model)}/record/${hash}`), hash, seen.through),
          ),
        );
      }
    }
  } catch (error) {
    console.warn(`The grids and planes couldn’t be read: ${error.message}`);
  }
}

/** A grid's or a plane's record arrived: it is built as any record is here, and kept. */
async function keepDatum(model, id, record, hash, seq, mine = false) {
  const answer = await shapeFor(model, record, null);
  if (model !== C.model?.id && !C.watching.has(model)) return;
  const pieces = C.datums.get(model) ?? new Map();
  C.datums.set(model, pieces);
  const known = pieces.get(id);
  if (known && known.seq > seq) return;
  pieces.set(id, { el: record, ...answer, hash, seq });
  if (mine && record.kind === 'grid') G.after = { model, name: record.name };
  if (G.held?.id === id && G.held.applied) G.held = null;
  queueDraw();
  if (S.sel === id) {
    refreshSide();
    renderRibbon();
  }
}

/** An entry changed a grid or a plane this page holds: true when it was one, which this takes care of. */
function datumChange(entry, change) {
  const pieces = C.datums.get(entry.target);
  const known = pieces?.get(change.piece);
  if (!known) return false;
  if (known.seq >= entry.seq) return true;
  if (!change.after) {
    pieces.delete(change.piece);
    if (S.sel === change.piece) {
      S.sel = null;
      refreshSide();
      renderRibbon();
    }
    return true;
  }
  const mine = entry.client === C.client && !UNDOING.has(entry.cmd);
  api(`${modelUrl(entry.target)}/record/${change.after}`)
    .then((record) => keepDatum(entry.target, change.piece, record, change.after, entry.seq, mine))
    .catch((error) => console.warn(`A grid or a plane couldn’t be read: ${error.message}`));
  return true;
}

/** The points a datum is drawn through: an end being dragged, a reshape awaiting its answer, or what was built. */
function datumOutline(piece) {
  const id = piece.el.id;
  if (G.drag?.id === id && G.drag.to) {
    const outline = G.drag.outline.slice();
    outline[G.drag.end] = G.drag.to;
    return outline;
  }
  if (G.held?.id === id && (G.held.applied || C.asked.has(G.held.counter))) return G.held.outline;
  return piece.shape?.displays?.[0]?.outline ?? null;
}

/** One grid or plane in the plan. `how` is `own`, the open model's, or `linked`. */
function datumSvg(piece, s, how) {
  const { el } = piece;
  const outline = datumOutline(piece);
  // A horizontal plane has no place in plan; a record that didn't build has nothing to draw.
  if (!outline || outline.length < 2) return '';
  const grid = el.kind === 'grid';
  const own = how === 'own';
  const picked = own && S.sel === el.id;
  let g = PL(outline, `ln fn ${grid ? 'dd' : 'dsh'}${picked ? ' ap' : ''}`);
  if (own) g += PL(outline, 'datum-hit');
  const [[ax, ay], [bx, by]] = outline;
  const run = Math.hypot(ax - bx, ay - by) || 1;
  const r = 420 * s;
  // The bubble sits beyond the start, on the line carried on.
  const [cx, cy] = [ax + ((ax - bx) / run) * r, ay + ((ay - by) / run) * r];
  if (grid) g += `<circle class="bub" cx="${n0(cx)}" cy="${n0(cy)}" r="${n0(r)}"/>${TX(cx, cy + 130 * s, s, esc(el.name), 'tn', 360)}`;
  else if (el.name) g += TX(ax, ay - 200 * s, s, esc(el.name), 'ta', 260, 'start');
  const claim = own && claimedByOther(el.id);
  if (claim) g += TX(cx, cy - r - 160 * s, s, `${esc(claim.name)} editing`, 'claimt', 260);
  if (picked && canEdit() && el.recipe.op === 'grid.line') {
    const h = 150 * s;
    for (const [end, [x, y]] of [outline[0], outline.at(-1)].entries()) {
      g += `<rect class="ln ap pf datum-end" data-grid-end="${end}" x="${n0(x - h)}" y="${n0(y - h)}" width="${n0(2 * h)}" height="${n0(2 * h)}"/>`;
    }
  }
  const [x0, y0] = outline[0];
  const [x1, y1] = outline.at(-1);
  const marks = `data-datum="${esc(el.id)}" data-kind="${esc(el.kind)}" data-name="${esc(el.name)}" data-x0="${x0}" data-y0="${y0}" data-x1="${x1}" data-y1="${y1}"${picked ? ' data-picked="yes"' : ''}`;
  return `<g class="datum ${how}" ${marks}>${g}</g>`;
}

/** Every grid and plane in view, drawn first so that the elements stay on top and are picked. */
function datumsSvg(s) {
  if (S.cats?.grids === false) return '';
  let svg = '';
  const linked = (elements) => {
    for (const piece of elements) if (isDatum(piece.element)) svg += datumSvg(packaged(piece), s, 'linked');
  };
  for (const elements of C.fixed.values()) linked(elements);
  if (!C.model && C.file) linked(C.file.elements);
  for (const [model, pieces] of C.datums) {
    for (const piece of pieces.values()) svg += datumSvg(piece, s, model === C.model?.id ? 'own' : 'linked');
  }
  if (S.tool === 'grid' && S.pend && G.preview) svg += L(S.pend, G.preview, 'ln ap dd');
  return svg;
}

function gridHint() {
  return S.pend ? '<b>Grid:</b> pick its other end. Esc to stop.' : '<b>Grid:</b> pick where it starts: its name goes in a bubble there.';
}

/** The name of the grid this person last made or renamed in the open model, for the server's next name. */
const lastGridName = () => (G.after?.model === C.model?.id ? G.after.name : null);

function placeGrid(at) {
  const point = [snap(at.x), snap(at.y)];
  if (!S.pend) {
    S.pend = point;
    G.preview = point;
    $('#hint').innerHTML = hint();
    return;
  }
  const [[x0, y0], [x1, y1]] = [S.pend, point];
  S.pend = null;
  G.preview = null;
  const args = { grid: newId('G'), name: null, x0: `${x0}`, y0: `${y0}`, x1: `${x1}`, y1: `${y1}` };
  const after = lastGridName();
  if (after !== null) args.after = after;
  manage('grid.create', args, 'new-grid');
  $('#hint').innerHTML = hint();
  queueDraw();
}

/** A stored length (`length:q0:<steps>`, or an older whole `length:<mm>`) as typed text, exactly. */
function typedLength(text) {
  const [, unit, steps] = String(text).split(':');
  if (unit !== 'q0') return `${unit} mm`;
  const n = Number(steps);
  const [whole, rest] = [Math.floor(Math.abs(n) / 6400), Math.abs(n) % 6400];
  // A step is 1/6400 mm, which is 15625 hundred-millionths: so the fraction is exact in 8 places.
  const fraction = rest ? `.${String(rest * 15625).padStart(8, '0').replace(/0+$/, '')}` : '';
  return `${n < 0 ? '-' : ''}${whole}${fraction} mm`;
}

/** A grid or a plane was picked in the plan: selected, claimed as an element is, and maybe an end taken. */
function pickDatum(id, end) {
  if (S.sel !== id) select(id);
  if (!canEdit()) return;
  edit('element.claim', { element: id });
  const piece = ownDatum(id);
  const outline = piece && datumOutline(piece);
  if (end && piece?.el.recipe.op === 'grid.line' && outline?.length === 2) {
    G.held = null;
    G.drag = { id, end: Number(end.dataset.gridEnd), outline: outline.map((point) => [...point]), to: null };
  }
}

/** An end let go: the grid's two ends go to the server, the one not dragged as it is stored. */
function letGoOfAnEnd() {
  const drag = G.drag;
  if (!drag) return;
  G.drag = null;
  const piece = ownDatum(drag.id);
  if (!piece || !drag.to || drag.to.join() === drag.outline[drag.end].join()) {
    queueDraw();
    return;
  }
  const { args } = piece.el.recipe;
  const ends = [
    [typedLength(args.x0), typedLength(args.y0)],
    [typedLength(args.x1), typedLength(args.y1)],
  ];
  ends[drag.end] = drag.to.map((value) => `${value} mm`);
  const outline = drag.outline.slice();
  outline[drag.end] = drag.to;
  const reshape = { grid: drag.id, x0: ends[0][0], y0: ends[0][1], x1: ends[1][0], y1: ends[1][1] };
  // The counter the edit is about to take: the dragged end is drawn until the server answers it.
  const counter = C.counter + 1;
  G.held = { id: drag.id, counter, outline };
  manage('grid.reshape', reshape, `datum:${drag.id}`, () => {
    if (G.held?.counter === counter) G.held.applied = true;
  });
  queueDraw();
}

// In the plan, before the elements' own handlers: the grid tool's points, and a pick of a grid or a plane.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
    if (S.tool === 'grid') {
      event.stopPropagation();
      if (!canEdit()) say('Open a model in your team’s WIP to add to it.');
      else placeGrid(toModel(event));
      return;
    }
    const hit = S.tool === 'select' && !late.DM.adding && event.target.closest('g.datum.own');
    if (!hit) return;
    event.stopPropagation();
    pickDatum(hit.dataset.datum, event.target.closest('[data-grid-end]'));
  },
  true,
);
window.addEventListener('pointermove', (event) => {
  if (!(G.drag || (S.tool === 'grid' && S.pend)) || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  const point = [snap(at.x), snap(at.y)];
  if (G.drag) G.drag.to = point;
  else G.preview = point;
  queueDraw();
});
window.addEventListener('pointerup', letGoOfAnEnd);
window.addEventListener('pointercancel', letGoOfAnEnd);

/** What the properties palette shows of a selected grid or plane. */
function datumProperties(piece) {
  const { el } = piece;
  const grid = el.kind === 'grid';
  const what = grid ? 'grid' : 'reference plane';
  const outline = datumOutline(piece) || [];
  const view = paramsFor(piece);
  const claim = heldClaim(el.id);
  const name = canEdit()
    ? `<input class="inp" data-chg="stackdatumname" data-key="datum-name" value="${esc(el.name)}" aria-label="Name of the ${what}">`
    : esc(el.name || 'None');
  const at = (point) => `<span class="mono">${point[0]}, ${point[1]}</span>`;
  let html = typeSel('grid', grid ? 'Grid' : 'Reference plane', `<b>${esc(el.name || `Unnamed ${what}`)}</b>`);
  html += `<table class="props">${prow('Name', name + problem(`datum:${el.id}`))}`;
  if (view?.category) html += prow('Category', esc(view.category.name));
  html += prow('Shape', el.recipe.op === 'grid.arc' ? 'Arc' : outline.length ? 'Straight' : 'Level');
  if (outline.length) html += prow('Start', at(outline[0])) + prow('End', at(outline.at(-1)));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  if (canEdit()) {
    const drag = el.recipe.op === 'grid.line' ? 'Drag an end to move it. ' : '';
    html += `<p class="pad small faint">${drag}Delete removes it.</p>`;
  }
  return html;
}

CHG.stackdatumname = (input) => {
  const piece = ownDatum(S.sel);
  if (!piece) return;
  const id = piece.el.id;
  const name = input.value;
  if (piece.el.kind === 'refplane') manage('plane.rename', { plane: id, name: name === '' ? null : name }, `datum:${id}`);
  else manage('grid.rename', { grid: id, name }, `datum:${id}`);
  input.blur();
};

const deleteElement = ACT.stackdelete;
ACT.stackdelete = () => {
  const piece = ownDatum(S.sel);
  if (!piece) {
    deleteElement();
    return;
  }
  if (!canEdit()) return;
  const id = piece.el.id;
  if (piece.el.kind === 'refplane') manage('plane.delete', { plane: id }, `datum:${id}`);
  else manage('grid.delete', { grid: id }, `datum:${id}`);
  S.sel = null;
  refresh();
};

/** The ribbon: the grid tool beside the Build panel, and Delete for a selected grid or plane. */
function datumRibbon(config) {
  const arch = config.tabs?.find((tab) => tab.id === 'arch');
  if (arch) {
    const build = arch.panels.findIndex((panel) => panel.label === 'Build');
    arch.panels.splice(build + 1, 0, { label: 'Datum', items: [DT_('architecture', 'grid')] });
  }
  const piece = ownDatum(S.sel);
  if (piece && canEdit() && S.screen === 'plan') {
    const label = piece.el.kind === 'grid' ? 'Grids' : 'Reference planes';
    config.contextual?.push({
      id: 'sel',
      key: `sel:${S.sel}`,
      group: 'Modify',
      label,
      panels: [
        { label, items: [{ label: 'Delete', icon: 'del', act: 'stackdelete', kbd: 'Delete' }] },
        { label: 'Selection', items: [{ label: 'Deselect', icon: 'x', act: 'deselect' }] },
      ],
    });
  }
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'grid', discipline: 'architecture', order: 50 });
register.ribbons({ id: 'datum', order: 1010, ribbon: (config) => datumRibbon(config) });
register.hints({ id: 'grid', order: 10, hint: () => (S.tool === 'grid' ? gridHint() : undefined) });
register.selections({ id: 'datum', order: 20, selection: (id) => (ownDatum(id) ? datumProperties(ownDatum(id)) : undefined) });
register.layers({ id: 'datums', order: 30, under: (scale) => datumsSvg(scale) });
register.models({ id: 'datums', order: 10, open: () => loadDatums(), forget: () => forgetDatums() });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get datumChange() { return datumChange; }, get isDatum() { return isDatum; },
  get keepDatum() { return keepDatum; },
});

export { typedLength };
