// Tool: the column tool, architectural columns in the plan and the properties palette.

import { register } from '../../core/registry.js';
import { C, api, levelOf, modelUrl, say } from '../../core/state.js';
import { around, outlineOf, steps } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { parametersTable, paramsFor, refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, plural, problem } from '../../core/problems.js';
import { heldClaim, hint, queueDraw, snap, toModel } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { READ_ONLY, readOnlyTable } from './floor.js';

// --- The column tool, and a column in the plan and the properties palette ---
// A column is made by its type (chosen in the properties palette from the server's list: section and
// material are the type's), at a point, or at grid intersections: by picking grid lines (every place
// two picked lines cross), or by dragging a box over them (every crossing in the box). Its base is
// the plan's level plus an offset; its top is a level plus an offset, or an unconnected height. Its
// turn is an angle, or, at grid intersections, the angle of the grid line to the nearest quarter turn
// ("rotate with grid": the angle is read once, as the column is placed, and kept; it does not follow
// the grid afterwards). One `column.place` edit for a click, one `column.place.grids` for all the
// crossings picked, so a whole grid of columns is one step to undo (crates/construct-columns'
// FORMAT.md). The page works out no geometry beyond the box test: the crossings, the plan cut planes
// and every column's height above zero come from the server (`/columns`), the quantities from the
// column's read-only parameters, and a refusal is the server's words, under the tool's options or
// the column's. A column grid-bound stays on its grids when they move (the server regenerates it);
// one whose grid is deleted keeps its place as a point and is noticed in the properties palette and
// the warnings. In the plan a column is the outline of its section from the shape the server built:
// cut where the view's cut plane crosses it (filled), a thin outline where it ends below the
// plane, dashed where it starts above it. In 3D it is the shape the browser built, as any shape.

const isColumn = (record) => record?.kind === 'column';
const MODES = [
  ['point', 'At a point'],
  ['pick', 'At grid intersections: pick grid lines'],
  ['box', 'At grid intersections: drag a box over them'],
];
const FREE = 'free'; // a top that is not tied to a level

DTOOLS.architecture.push(['column', 'Column', 'family', 'Click a point, or choose grid intersections in the properties palette.']);
TOOLNAME.column = 'column';
Object.assign(WORDS, {
  'column.place': 'placed a column',
  'column.place.grids': 'placed columns at grid intersections',
  'column.type': 'changed the type of',
  'column.levels': 'changed the base or top of',
  'column.move': 'moved',
  'column.turn': 'turned',
  'column.delete': 'deleted',
  'column.defaults': 'made the default column types',
});
Object.assign(STEP_WORDS, {
  'column.place': 'placing a column',
  'column.place.grids': 'placing columns at grid intersections',
  'column.type': 'a column’s type',
  'column.levels': 'a column’s base or top',
  'column.move': 'moving a column',
  'column.turn': 'turning a column',
  'column.delete': 'deleting a column',
  'column.defaults': 'making the default column types',
});

// type: a column type's ID (null: the first); top: a level's ID, FREE, or undefined for the level above
C.columnTool = { type: null, mode: 'point', top: undefined, baseOffset: '0', topOffset: '0', height: '3000 mm', turn: '0', withGrid: false, picked: [], box: null };
C.columnsView = null; // { key, view } of the open model, as the server reads it
C.columnsEpoch = 0; // goes up when an entry may change what `/columns` says

/** The open model's column types, columns, grid crossings and plan cuts as the server reads them; null until they have come. */
function columnsView() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}/${C.columnsEpoch}`;
  if (C.columnsView?.key !== key && C.columnsLoading !== key) {
    C.columnsLoading = key;
    api(`${modelUrl(model)}/columns`)
      .then((view) => {
        C.columnsView = { key, view };
        refreshSide();
        queueDraw();
      })
      .catch(() => {});
  }
  return C.columnsView?.key.startsWith(`${model}/`) ? C.columnsView.view : null;
}

/** An entry arrived for the open model: what a column's place, type or height, or a grid's crossings, may now be. */
function columnsChanged(entry) {
  if (/^(column|level|grid|view|type|parameter)\.|^edit\.(undo|redo)$/.test(entry.cmd)) C.columnsEpoch += 1;
}

/** The server's row for a column, once the view has come. */
const columnInfo = (id) => columnsView()?.columns.find((c) => c.id === id);

/** The level a new column's top is tied to: the tool's choice, FREE, or the level above the plan's. */
function columnTop(levels) {
  const here = levelOf();
  const chosen = C.columnTool.top;
  if (chosen === FREE) return FREE;
  if ((levels || []).some((level) => level.id === chosen && level.id !== here)) return chosen;
  const at = (levels || []).findIndex((level) => level.id === here);
  return (at >= 0 && levels[at + 1]?.id) || FREE;
}

/** The type a new column has: the tool's choice if the server lists it, else the first listed. */
function columnType(view) {
  return view?.types.some((t) => t.id === C.columnTool.type) ? C.columnTool.type : view?.types[0]?.id ?? null;
}

const typeLabel = (t) => `${t.name}`;

function columnToolHtml() {
  let html = typeSel('family', 'Column', '<b>New column</b>');
  const view = columnsView();
  if (!view) return `${html}<p class="pad small faint">Loading the column types.</p>`;
  if (!view.types.length) {
    return `${html}<p class="pad small muted">This model has no column types yet.</p><div class="pad"><button class="btn sm" data-act="columndefaults" data-key="column-defaults">Add the default column types</button></div>${problem('column-draw')}`;
  }
  const levels = modelLevels() || [];
  const here = levelOf();
  const top = columnTop(levels);
  const tool = C.columnTool;
  const type = columnType(view);
  const base = levels.find((level) => level.id === here)?.name ?? here;
  const tops = [
    ...levels.filter((level) => level.id !== here).map((level) => `<option value="${esc(level.id)}"${level.id === top ? ' selected' : ''}>${esc(level.name)}</option>`),
    `<option value="${FREE}"${top === FREE ? ' selected' : ''}>Unconnected height</option>`,
  ];
  const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === type ? ' selected' : ''}>${esc(typeLabel(t))}</option>`);
  const modes = MODES.map(([id, label]) => `<option value="${id}"${id === tool.mode ? ' selected' : ''}>${label}</option>`);
  const field = (key, label, value) => `<input class="inp" data-chg="columntool${key}" data-key="column-${key}" value="${esc(value)}" aria-label="${esc(label)}">`;
  html += '<table class="props">';
  html += prow('Type', `<select class="inp" data-chg="columntooltype" data-key="column-type" aria-label="Type of the column">${types.join('')}</select>`);
  html += prow('Place', `<select class="inp" data-chg="columntoolmode" data-key="column-mode" aria-label="Where the column is placed">${modes.join('')}</select>`);
  html += prow('Base', esc(base));
  html += prow('Base offset', field('baseOffset', 'Offset of the base from its level', tool.baseOffset));
  html += prow('Top', `<select class="inp" data-chg="columntooltop" data-key="column-top" aria-label="Top of the column">${tops.join('')}</select>`);
  html += top === FREE
    ? prow('Height', field('height', 'Height of the column', tool.height))
    : prow('Top offset', field('topOffset', 'Offset of the top from its level', tool.topOffset));
  html += prow('Turn', field('turn', 'Turn of the column, in degrees', tool.turn));
  if (tool.mode !== 'point') {
    html += prow('Rotate with grid', `<input type="checkbox" data-chg="columntoolwithgrid" data-key="column-withgrid" aria-label="Rotate with the grid line"${tool.withGrid ? ' checked' : ''}>`);
  }
  html += '</table>';
  if (tool.mode === 'point') {
    html += '<p class="pad small muted">Click a point in the plan.</p>';
  } else if (tool.mode === 'pick') {
    const count = pickedCrossings(view).length;
    html += `<p class="pad small muted" data-column-picked>${plural(tool.picked.length, 'grid line')} picked, ${plural(count, 'crossing')}. Click grid lines to pick or let go of them.</p>`;
    html += `<div class="pad"><button class="btn sm" data-act="columnplace" data-key="column-place"${count ? '' : ' disabled'}>Place ${plural(count, 'column')}</button></div>`;
  } else {
    html += '<p class="pad small muted">Drag a box over the grid intersections to put a column on each.</p>';
  }
  return html + problem('column-draw');
}

CHG.columntooltype = (select) => {
  C.columnTool.type = select.value;
};
CHG.columntoolmode = (select) => {
  Object.assign(C.columnTool, { mode: select.value, picked: [], box: null });
  refreshSide();
  queueDraw();
  $('#hint').innerHTML = hint();
};
CHG.columntooltop = (select) => {
  C.columnTool.top = select.value;
  refreshSide();
};
CHG.columntoolbaseOffset = (input) => {
  C.columnTool.baseOffset = input.value;
};
CHG.columntooltopOffset = (input) => {
  C.columnTool.topOffset = input.value;
};
CHG.columntoolheight = (input) => {
  C.columnTool.height = input.value;
};
CHG.columntoolturn = (input) => {
  C.columnTool.turn = input.value;
};
CHG.columntoolwithgrid = (input) => {
  C.columnTool.withGrid = input.checked;
};
ACT.columndefaults = () => manage('column.defaults', {}, 'column-draw');
ACT.columnplace = () => placePicked();

function columnHint() {
  if (C.columnTool.mode === 'pick') return '<b>Column:</b> click grid lines, then Enter or the button in the properties palette. Esc lets them go.';
  if (C.columnTool.mode === 'box') return '<b>Column:</b> drag a box over the grid intersections. Esc to stop.';
  return '<b>Column:</b> click where the column stands. Choose its type, levels and turn in the properties palette.';
}

/** The ribbon: the column tool beside the stair tool, in the Build panel. */
function columnRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:column')) return;
  const after = build.items.findIndex((item) => item.a === 'architecture:stair');
  const roof = build.items.findIndex((item) => item.a === 'architecture:roof');
  build.items.splice((after >= 0 ? after : roof) + 1, 0, DT_('architecture', 'column'));
}

/** The names the next columns are given: C1, C2, ... after the highest the model has. */
function nameFrom(view) {
  const used = (view?.columns ?? []).map((c) => /^C(\d+)$/.exec(c.name)).filter(Boolean).map((m) => Number(m[1]));
  return Math.max(0, ...used) + 1;
}

/** What every new column shares, as `column.place` and `column.place.grids` take it; null (with the server's absence said) without a type. */
function sharedArgs(view, turn) {
  const type = columnType(view);
  if (!type) {
    say('This model has no column types yet.');
    return null;
  }
  const tool = C.columnTool;
  const top = columnTop(modelLevels());
  const args = { type, level: levelOf(), offset: tool.baseOffset, turn };
  if (top === FREE) args.height = tool.height;
  else args.top = { level: top, offset: tool.topOffset };
  return args;
}

/** A click in the plan, in point mode, makes a column there. */
function placeColumn(at) {
  if (C.columnTool.mode !== 'point') return;
  const view = columnsView();
  const shared = sharedArgs(view, C.columnTool.turn);
  if (!shared) return;
  const point = [snap(at.x), snap(at.y)];
  manage('column.place', { element: newId('C'), name: `C${nameFrom(view)}`, ...shared, at: point.map((v) => `${v} mm`) }, 'column-draw');
}

/** A crossing's point in millimetres (the server's typed text). */
const crossingMm = (c) => [parseFloat(c.x), parseFloat(c.y)];

/** The crossings of the grid lines picked so far (both lines of a pair picked). */
function pickedCrossings(view) {
  const picked = new Set(C.columnTool.picked);
  return (view?.crossings ?? []).filter((c) => picked.has(c.first) && picked.has(c.second));
}

/** The crossings inside the box being dragged or just dragged. */
function boxCrossings(view, box) {
  const [x0, x1] = [Math.min(box.x0, box.x1), Math.max(box.x0, box.x1)];
  const [y0, y1] = [Math.min(box.y0, box.y1), Math.max(box.y0, box.y1)];
  return (view?.crossings ?? []).filter((c) => {
    const [x, y] = crossingMm(c);
    return x >= x0 && x <= x1 && y >= y0 && y <= y1;
  });
}

/** The outline of a grid line as the page drew it: two points for a straight one. */
const gridOutline = (id) => C.datums.get(C.model?.id)?.get(id)?.shape?.displays?.[0]?.outline ?? null;
const gridName = (id) => C.datums.get(C.model?.id)?.get(id)?.el.name ?? id;

/** The turn of a column at a crossing when it rotates with the grid: the direction of the first straight line of the pair, to the quarter turn, to 0.01 degree. */
function gridTurn(crossing) {
  for (const id of [crossing.first, crossing.second]) {
    const outline = gridOutline(id);
    if (outline?.length !== 2) continue;
    const degrees = (Math.atan2(outline[1][1] - outline[0][1], outline[1][0] - outline[0][0]) * 180) / Math.PI;
    const quarter = ((degrees % 90) + 90) % 90;
    return String(Math.round(quarter * 100) / 100);
  }
  return C.columnTool.turn;
}

/** One `column.place.grids` for every crossing given. */
function placeAtCrossings(crossings) {
  if (!crossings.length) {
    say('There are no grid intersections there.');
    return;
  }
  const view = columnsView();
  const shared = sharedArgs(view, C.columnTool.turn);
  if (!shared) return;
  const first = nameFrom(view);
  const withGrid = C.columnTool.withGrid;
  // Columns turn one way each when they rotate with the grid, so the turn is sent for each.
  const turns = crossings.map((c) => (withGrid ? gridTurn(c) : null));
  const alike = turns.every((t) => t === turns[0]);
  const columns = crossings.map((c, i) => ({
    element: newId('C'),
    name: `C${first + i}`,
    first: c.first,
    second: c.second,
    cross: c.cross,
  }));
  if (withGrid && !alike) {
    // Different angles: one step each would be many steps, so group the columns by angle in a batch of edits.
    const byTurn = new Map();
    columns.forEach((column, i) => byTurn.set(turns[i], [...(byTurn.get(turns[i]) ?? []), column]));
    for (const [turn, group] of byTurn) manage('column.place.grids', { ...shared, turn, columns: group }, 'column-draw');
  } else {
    manage('column.place.grids', { ...shared, turn: withGrid ? turns[0] : shared.turn, columns }, 'column-draw');
  }
  C.columnTool.picked = [];
  C.columnTool.box = null;
  refreshSide();
  queueDraw();
}

function placePicked() {
  placeAtCrossings(pickedCrossings(columnsView()));
}

// In the plan, before the elements' own handlers: the column tool's picks of grid lines and its box.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (event.button !== 0 || S.screen !== 'plan' || S.tool !== 'column' || C.columnTool.mode === 'point') return;
    if (!event.target.closest?.('#pv-canvas svg')) return;
    event.stopPropagation();
    if (!canEdit()) {
      say('Open a model in your team’s WIP to add to it.');
      return;
    }
    if (C.columnTool.mode === 'box') {
      const at = toModel(event);
      C.columnTool.box = { x0: at.x, y0: at.y, x1: at.x, y1: at.y, held: true };
      return;
    }
    const line = event.target.closest('g.datum.own');
    if (!line) return;
    const id = line.dataset.datum;
    const picked = C.columnTool.picked;
    C.columnTool.picked = picked.includes(id) ? picked.filter((p) => p !== id) : [...picked, id];
    refreshSide();
    queueDraw();
  },
  true,
);
window.addEventListener('pointermove', (event) => {
  const box = C.columnTool.box;
  if (!box?.held || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  box.x1 = at.x;
  box.y1 = at.y;
  queueDraw();
});
const boxLetGo = () => {
  const box = C.columnTool.box;
  if (!box?.held) return;
  C.columnTool.box = null;
  if (Math.abs(box.x1 - box.x0) < 1 && Math.abs(box.y1 - box.y0) < 1) {
    queueDraw();
    return;
  }
  placeAtCrossings(boxCrossings(columnsView(), box));
};
window.addEventListener('pointerup', boxLetGo);
window.addEventListener('pointercancel', boxLetGo);
document.addEventListener('keydown', (event) => {
  if (S.tool !== 'column') return;
  if (event.key === 'Escape') {
    Object.assign(C.columnTool, { picked: [], box: null });
    refreshSide();
    queueDraw();
    return;
  }
  if (event.key === 'Enter' && C.columnTool.mode === 'pick' && !event.target.closest?.('input,textarea,button,select')) {
    event.preventDefault();
    placePicked();
  }
});

/** What the tool sketches over the plan: the crossings the picked lines give, the picked lines, and the box. */
function columnSketch(scale) {
  if (S.tool !== 'column' || !C.model) return '';
  const view = columnsView();
  let svg = '';
  const r = 90 * scale;
  const dot = (c) => {
    const [x, y] = crossingMm(c);
    return `<circle class="column-cross" cx="${x}" cy="${y}" r="${r}"/>`;
  };
  if (C.columnTool.mode === 'pick') {
    for (const id of C.columnTool.picked) {
      const outline = gridOutline(id);
      if (outline?.length >= 2) svg += PL(outline, 'column-picked');
    }
    for (const c of pickedCrossings(view)) svg += dot(c);
  } else if (C.columnTool.mode === 'box' && C.columnTool.box) {
    const box = C.columnTool.box;
    svg += RC(Math.min(box.x0, box.x1), Math.min(box.y0, box.y1), Math.abs(box.x1 - box.x0), Math.abs(box.y1 - box.y0), 'column-box');
    for (const c of boxCrossings(view, box)) svg += dot(c);
  }
  return svg ? `<g class="column-sketch">${svg}</g>` : '';
}

/** How the open plan sees a column: `cut` where its cut plane crosses it, `below` where it ends under it, `above` where it starts over it. */
function cutState(info) {
  const view = columnsView();
  if (!view || !info?.span) return 'cut';
  const plan = view.plans.find((p) => p.view === C.openView) ?? view.plans.find((p) => p.level === levelOf());
  if (!plan) return 'cut';
  const cut = steps(plan.cut.value);
  if (steps(info.span.top.value) <= cut) return 'below';
  if (steps(info.span.base.value) > cut) return 'above';
  return 'cut';
}

/** A column in the plan: the outline of its section, filled where the plan cuts it. */
function columnInPlan(piece, data, scale) {
  const { el: e } = piece;
  const box = around(piece);
  const chosen = S.sel === e.id;
  const state = cutState(columnInfo(e.id));
  let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
  group += outlineOf(piece, `body column column-${state}${chosen ? ' sel' : ''}`);
  const claim = heldClaim(e.id);
  if (claim) {
    const theirs = claim.person !== C.me.person.id;
    group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
    if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
  }
  return `<g ${data}>${group}</g>`;
}

/** Where a column stands, in words: the grid lines it stands on, or its point. */
function standsOn(info) {
  if (info.at.on === 'grids') return `${esc(gridName(info.at.first))} and ${esc(gridName(info.at.second))}`;
  return `<span class="mono">${esc(info.at.x)}, ${esc(info.at.y)}</span>`;
}

/** What the properties palette shows of a selected column. */
function columnProperties(piece) {
  const { el: e, shape, failed } = piece;
  const view = paramsFor(piece);
  const columns = columnsView();
  const info = columns?.columns.find((c) => c.id === e.id);
  const claim = heldClaim(e.id);
  const editable = canEdit();
  let html = typeSel('family', 'Column', `<b>${esc(e.name)}</b>`);
  html += '<table class="props" data-column-element>';
  if (view) html += prow('Category', esc(view.category?.name || 'None'));
  if (info && columns) {
    const levels = modelLevels() || [];
    const levelOptions = (chosen, extra = '') =>
      `${levels.map((level) => `<option value="${esc(level.id)}"${level.id === chosen ? ' selected' : ''}>${esc(level.name)}</option>`).join('')}${extra}`;
    if (editable) {
      const types = columns.types.map((t) => `<option value="${esc(t.id)}"${t.id === info.type ? ' selected' : ''}>${esc(typeLabel(t))}</option>`);
      html += prow('Type', `<select class="inp" data-chg="columntype" data-key="column-own-type" aria-label="Type of the column">${types.join('')}</select>`);
      html += prow('Base', `<select class="inp" data-chg="columnbase" data-key="column-own-base" aria-label="Level of the base">${levelOptions(info.base?.level)}</select>`);
      html += prow('Base offset', `<input class="inp" data-chg="columnbaseoffset" data-key="column-own-baseoffset" value="${esc(info.base?.offset ?? '0 mm')}" aria-label="Offset of the base from its level">`);
      html += prow('Top', `<select class="inp" data-chg="columntop" data-key="column-own-top" aria-label="Top of the column">${levelOptions(info.top?.level, `<option value="${FREE}"${info.top ? '' : ' selected'}>Unconnected height</option>`)}</select>`);
      html += info.top
        ? prow('Top offset', `<input class="inp" data-chg="columntopoffset" data-key="column-own-topoffset" value="${esc(info.top.offset)}" aria-label="Offset of the top from its level">`)
        : prow('Height', `<input class="inp" data-chg="columnheight" data-key="column-own-height" value="${esc(info.height ?? '')}" aria-label="Height of the column">`);
      html += prow('Turn', `<input class="inp" data-chg="columnturn" data-key="column-own-turn" value="${esc(info.turn)}" aria-label="Turn of the column, in degrees">`);
    } else {
      html += prow('Type', esc(columns.types.find((t) => t.id === info.type)?.name ?? 'None'));
      html += prow('Base', esc(levelName(e)));
      html += prow('Turn', esc(`${info.turn}°`));
    }
    html += prow('Stands on', standsOn(info));
  }
  if (!shape) html += prow('Shape', esc(info?.words || failed?.message || 'Not built yet'));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  if (info?.released) {
    html += `<div class="note small" data-column-released>It stood on ${esc(info.released)}, which was deleted. It keeps its place as a point.</div>`;
  }
  html += problem('column-edit');
  if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
  if (view) {
    html += readOnlyTable(view);
    html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  }
  if (editable) html += '<div class="pad"><button class="btn sm" data-act="stackdelete" data-key="column-delete">Delete column</button></div>';
  return html;
}

const selectedColumn = () => (isColumn(C.elements.get(S.sel)?.el) ? S.sel : null);
CHG.columntype = (select) => {
  if (selectedColumn()) manage('column.type', { elements: [S.sel], type: select.value }, 'column-edit');
};
CHG.columnbase = (select) => {
  if (selectedColumn()) manage('column.levels', { elements: [S.sel], level: select.value }, 'column-edit');
};
CHG.columnbaseoffset = (input) => {
  if (selectedColumn()) manage('column.levels', { elements: [S.sel], offset: input.value }, 'column-edit');
  input.blur();
};
CHG.columntop = (select) => {
  if (!selectedColumn()) return;
  const info = columnInfo(S.sel);
  if (select.value === FREE) {
    // Keep the height it has now.
    const height = info?.span ? `${Math.round((steps(info.span.top.value) - steps(info.span.base.value)) / 6400)} mm` : C.columnTool.height;
    manage('column.levels', { elements: [S.sel], height }, 'column-edit');
  } else {
    manage('column.levels', { elements: [S.sel], top: { level: select.value, offset: info?.top?.offset ?? '0' } }, 'column-edit');
  }
};
CHG.columntopoffset = (input) => {
  const info = columnInfo(S.sel);
  if (selectedColumn() && info?.top) manage('column.levels', { elements: [S.sel], top: { level: info.top.level, offset: input.value } }, 'column-edit');
  input.blur();
};
CHG.columnheight = (input) => {
  if (selectedColumn()) manage('column.levels', { elements: [S.sel], height: input.value }, 'column-edit');
  input.blur();
};
CHG.columnturn = (input) => {
  if (selectedColumn()) manage('column.turn', { elements: [S.sel], turn: input.value }, 'column-edit');
  input.blur();
};

/** The properties palette's column part: the tool's options, or a selected column; '' for anything else. */
function columnPalette(piece) {
  if (S.tool === 'column' && canEdit()) return columnToolHtml();
  if (piece && isColumn(piece.el) && !(S.tool === 'wall' && canEdit())) return columnProperties(piece);
  return '';
}

const deleteBeforeColumns = ACT.stackdelete;
ACT.stackdelete = () => {
  const piece = C.elements.get(S.sel);
  if (!piece || !isColumn(piece.el)) {
    deleteBeforeColumns();
    return;
  }
  if (!canEdit()) return;
  manage('column.delete', { elements: [S.sel] }, 'column-edit');
  S.sel = null;
  refresh();
};

const toolBeforeColumns = ACT.tool;
ACT.tool = (button) => {
  Object.assign(C.columnTool, { picked: [], box: null });
  toolBeforeColumns(button);
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'column', discipline: 'architecture', order: 140, place: (at) => placeColumn(at) });
register.ribbons({ id: 'column', order: 70, ribbon: (config) => columnRibbon(config) });
register.hints({ id: 'column', order: 110, hint: () => (S.tool === 'column' ? columnHint() : undefined) });
register.palettes({ id: 'column', order: 80, palette: (piece) => columnPalette(piece) || undefined });
register.elements({ id: 'column', order: 90, owns: isColumn, inPlan: (piece, data, scale) => columnInPlan(piece, data, scale), pickOnly: true });
register.layers({ id: 'column-sketch', order: 70, over: (scale) => columnSketch(scale) });
register.logs({ id: 'columns', order: 50, entry: (entry) => columnsChanged(entry) });
