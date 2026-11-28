// Tool: the stair tool, a stair in the plan and the properties palette.

import { register } from '../../core/registry.js';
import { C, api, levelIdOf, levelOf, modelUrl } from '../../core/state.js';
import { boot } from '../../core/hub.js';
import { STEPS_PER_MM, around, outlineOf, steps, storedMm } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { parametersTable, paramsFor, refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, plural, problem } from '../../core/problems.js';
import { heldClaim, hint, queueDraw, snap } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { READ_ONLY, readOnlyTable, ringPath } from './floor.js';

// --- The stair tool, and a stair in the plan and the properties palette ---
// A stair is made between two levels by two clicks in the plan, as a wall is: the first is the foot
// of its first riser on its right-hand side going up, the second says which way it runs (the plan's
// axis the second point is further along). Its base is the plan's level, its top the level chosen
// in the properties palette; the category (the rule set's, from the server), its width and whether
// it is straight, with a landing, an L or a U are chosen there too. One `stair.create` edit
// (crates/construct-stairs' FORMAT.md). The page checks nothing and lays nothing out: how many
// risers, how high each is and how deep each tread is come from the server (the stair's read-only
// parameters, `/parameters/<element>`, and `/stairs` for its layout), and where the rules cannot
// lay a stair out, that is the server's refusal in its words, with the nearest heights that work,
// under the tool's options or the stair's. A selected stair's width, direction and runs are changed
// there (`stair.top`, `stair.category`, `stair.width`, `stair.direction`, `stair.runs`) and Delete removes it (`stair.delete`).
// In the plan a stair is its runs and landings from its recipe as the server filled it
// (`/filled/<element>`): each tread's line, solid where the stair is below the view's cut plane
// and dashed above it, the cut line where the plane crosses the stair, and the walking line from
// the bottom ending in an arrow. The cut plane is the open plan's, as the server read it
// (`/stairs`, `plans`); this page only draws. In 3D a stair is the shape the browser built from
// the same recipe: each run and landing, as the generic drawing shows any shape.

const STAIR_OP = 'stair';
const isStair = (record) => record?.kind === 'stair' && record?.recipe?.op === STAIR_OP;
/** Ways a stair's runs are laid: its ID, its label, how many runs, and the turn of its landing. */
const STAIR_LAYOUTS = [
  ['straight', 'Straight', 1, null],
  ['straight-landing', 'Straight, with a landing', 2, 'straight'],
  ['left', 'L, turning left', 2, 'left'],
  ['right', 'L, turning right', 2, 'right'],
  ['back-left', 'U, back on the left', 2, 'back-left'],
  ['back-right', 'U, back on the right', 2, 'back-right'],
];
const STAIR_DIRECTIONS = [['+x', 'East (+x)'], ['-x', 'West (-x)'], ['+y', 'North (+y)'], ['-y', 'South (-y)']];

DTOOLS.architecture.push(['stair', 'Stair', 'family', 'Pick the foot of its first riser, then a point in the direction it runs up.']);
TOOLNAME.stair = 'stair';
Object.assign(WORDS, {
  'stair.create': 'made a stair',
  'stair.width': 'changed the width of',
  'stair.direction': 'turned or moved',
  'stair.runs': 'changed the runs of',
  'stair.delete': 'deleted',
});
Object.assign(STEP_WORDS, {
  'stair.create': 'making a stair',
  'stair.width': 'a stair’s width',
  'stair.direction': 'a stair’s direction',
  'stair.runs': 'a stair’s runs',
  'stair.delete': 'deleting a stair',
});

C.stairTool = { top: undefined, category: null, width: '1000 mm', layout: 'straight', landing: '1000 mm' }; // top: a level's ID, undefined for the level above
C.stairsView = null; // { key, view } of the open model
C.stairsEpoch = 0; // goes up when an entry may change what `/stairs` says

/** The open model's stairs as the server reads them (its categories, plan cuts and each stair's layout); null until they have come. */
function stairsView() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}/${C.stairsEpoch}`;
  if (C.stairsView?.key !== key && C.stairsLoading !== key) {
    C.stairsLoading = key;
    api(`${modelUrl(model)}/stairs`)
      .then((view) => {
        C.stairsView = { key, view };
        refreshSide();
        queueDraw();
      })
      .catch(() => {});
  }
  return C.stairsView?.key.startsWith(`${model}/`) ? C.stairsView.view : null;
}

/** An entry arrived for the open model: what a stair's layout, or where a plan cuts it, may now be. */
function stairsChanged(entry) {
  if (/^(stair|level|view)\.|^edit\.(undo|redo)$/.test(entry.cmd)) C.stairsEpoch += 1;
}

/** The level a stair's top is tied to: the tool's choice, else the level above the plan's. */
function stairTop(levels) {
  const here = levelOf();
  const chosen = C.stairTool.top;
  if ((levels || []).some((level) => level.id === chosen && level.id !== here)) return chosen;
  const at = (levels || []).findIndex((level) => level.id === here);
  return (at >= 0 && levels[at + 1]?.id) || (levels || []).find((level) => level.id !== here)?.id || '';
}

/** The runs and landings a layout is sent as: `runs` (each left to share the risers) and `landings`. */
function stairRuns(layout, landing) {
  const found = STAIR_LAYOUTS.find(([id]) => id === layout) ?? STAIR_LAYOUTS[0];
  const [, , runs, turn] = found;
  return {
    runs: Array.from({ length: runs }, () => ({})),
    landings: turn ? [{ length: landing, turn }] : [],
  };
}

const layoutOptions = (chosen) =>
  STAIR_LAYOUTS.map(([id, name]) => `<option value="${id}"${id === chosen ? ' selected' : ''}>${name}</option>`).join('');

function stairToolHtml() {
  let html = typeSel('family', 'Stair', '<b>New stair</b>');
  const view = stairsView();
  if (!view) return `${html}<p class="pad small faint">Loading the stair rules.</p>`;
  const levels = modelLevels() || [];
  const here = levelOf();
  const top = stairTop(levels);
  const tool = C.stairTool;
  const category = view.categories.some((c) => c.id === tool.category)
    ? tool.category
    : view.categories.find((c) => c.id === 'common')?.id ?? view.categories[0]?.id;
  const base = levels.find((level) => level.id === here)?.name ?? here;
  const tops = levels
    .filter((level) => level.id !== here)
    .map((level) => `<option value="${esc(level.id)}"${level.id === top ? ' selected' : ''}>${esc(level.name)}</option>`);
  const categories = view.categories.map((c) => `<option value="${esc(c.id)}"${c.id === category ? ' selected' : ''}>${esc(c.name)}</option>`);
  const field = (key, label, value) => `<input class="inp" data-chg="stairtool${key}" data-key="stair-${key}" value="${esc(value)}" aria-label="${esc(label)}">`;
  html += '<table class="props">';
  html += prow('Base', esc(base));
  html += prow('Top', `<select class="inp" data-chg="stairtooltop" data-key="stair-top" aria-label="Top of the stair">${tops.join('')}</select>`);
  html += prow('Category', `<select class="inp" data-chg="stairtoolcategory" data-key="stair-category" aria-label="Category of stairway">${categories.join('')}</select>`);
  html += prow('Width', field('width', 'Width of the stair', tool.width));
  html += prow('Runs', `<select class="inp" data-chg="stairtoollayout" data-key="stair-layout" aria-label="How the runs are laid">${layoutOptions(tool.layout)}</select>`);
  if (tool.layout !== 'straight') html += prow('Landing', field('landing', 'Length of the landing', tool.landing));
  html += '</table>';
  html += `<p class="pad small muted" data-stair-rules>${esc(view.rules_name)}. The server works out the risers.</p>`;
  html += '<p class="pad small muted">Pick the foot of the first riser, on the stair’s right going up, then a point in the direction it runs.</p>';
  return html + problem('stair-draw');
}

CHG.stairtooltop = (select) => {
  C.stairTool.top = select.value;
  refreshSide();
};
CHG.stairtoolcategory = (select) => {
  C.stairTool.category = select.value;
};
CHG.stairtoollayout = (select) => {
  C.stairTool.layout = select.value;
  refreshSide();
};
CHG.stairtoolwidth = (input) => {
  C.stairTool.width = input.value;
};
CHG.stairtoollanding = (input) => {
  C.stairTool.landing = input.value;
};

function stairHint() {
  return S.pend
    ? '<b>Stair:</b> pick a point in the direction it runs up. Esc to stop.'
    : '<b>Stair:</b> pick the foot of its first riser. Choose its levels, category and runs in the properties palette.';
}

/** The ribbon: the stair tool beside the roof tool, in the Build panel. */
function stairRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:stair')) return;
  const roof = build.items.findIndex((item) => item.a === 'architecture:roof');
  build.items.splice(roof + 1, 0, DT_('architecture', 'stair'));
}

/**
 * The stair tool's second point makes the stair: it runs the way along x or y the second point
 * is further from the first, which is the foot of its first riser. What the server makes of it,
 * or its reason for refusing, comes back through the log, and the reason is shown under the options.
 */
function placeStair(at) {
  const point = [snap(at.x), snap(at.y)];
  if (!S.pend) {
    S.pend = point;
    $('#hint').innerHTML = hint();
    return;
  }
  const [ax, ay] = S.pend;
  S.pend = null;
  $('#hint').innerHTML = hint();
  const dx = point[0] - ax;
  const dy = point[1] - ay;
  const direction = Math.abs(dx) >= Math.abs(dy) ? (dx >= 0 ? '+x' : '-x') : dy >= 0 ? '+y' : '-y';
  const view = stairsView();
  const tool = C.stairTool;
  const args = {
    element: newId('S'),
    level: levelOf(),
    top: stairTop(modelLevels()),
    start: [`${ax} mm`, `${ay} mm`],
    direction,
    width: tool.width,
    ...stairRuns(tool.layout, tool.landing),
  };
  if (view?.categories.some((c) => c.id === tool.category)) args.category = tool.category;
  manage('stair.create', args, 'stair-draw');
}

/** The layout of a stair's runs as its record says it: one of STAIR_LAYOUTS' IDs, or null for any other. */
function stairLayoutOf(e) {
  const { runs, landings } = e.recipe.args;
  if (!Array.isArray(runs) || !Array.isArray(landings)) return null;
  if (runs.length === 1 && landings.length === 0) return 'straight';
  if (runs.length !== 2 || landings.length !== 1) return null;
  return STAIR_LAYOUTS.find(([, , , turn]) => turn === landings[0].turn)?.[0] ?? null;
}

/** A stair's width as stored, in millimetres. */
const stairWidthMm = (e) => `${storedMm(e.recipe.args.width)} mm`;

/** What the properties palette shows of a selected stair. */
function stairProperties(piece) {
  const { el: e, shape, failed } = piece;
  const view = paramsFor(piece);
  const stairs = stairsView();
  const info = stairs?.stairs.find((s) => s.id === e.id);
  const claim = heldClaim(e.id);
  const editable = canEdit();
  let html = typeSel('family', 'Stair', `<b>${esc(e.name)}</b>`);
  html += '<table class="props" data-stair-element>';
  if (view) html += prow('Category', esc(view.category?.name || 'None'));
  html += prow('Base', esc(levelName(e)));
  const topId = e.props?.['@top.level'];
  const levels = modelLevels() || [];
  const rule = stairs?.categories.find((c) => c.id === info?.category);
  if (editable && stairs && info) {
    const tops = levels
      .filter((level) => level.id !== levelIdOf(e))
      .map((level) => `<option value="${esc(level.id)}"${level.id === topId ? ' selected' : ''}>${esc(level.name)}</option>`);
    const categories = stairs.categories.map((c) => `<option value="${esc(c.id)}"${c.id === info.category ? ' selected' : ''}>${esc(c.name)}</option>`);
    html += prow('Top', `<select class="inp" data-chg="stairtop" data-key="stair-own-top" aria-label="Top of the stair">${tops.join('')}</select>`);
    html += prow('Stairway', `<select class="inp" data-chg="stairrules" data-key="stair-own-category" aria-label="Category of stairway">${categories.join('')}</select>`);
  } else {
    html += prow('Top', `<span data-stair-top>${esc(levels.find((level) => level.id === topId)?.name ?? topId ?? 'None')}</span>`);
    if (rule) html += prow('Stairway', esc(rule.name));
  }
  if (editable) {
    const here = stairLayoutOf(e);
    const directions = STAIR_DIRECTIONS.map(([id, name]) => `<option value="${id}"${id === e.recipe.args.direction ? ' selected' : ''}>${name}</option>`);
    html += prow('Width', `<input class="inp" data-chg="stairwidth" data-key="stair-own-width" value="${esc(stairWidthMm(e))}" aria-label="Width of the stair">`);
    html += prow('Direction', `<select class="inp" data-chg="stairdirection" data-key="stair-own-direction" aria-label="Which way the stair runs up">${directions.join('')}</select>`);
    const layouts = here ? layoutOptions(here) : `<option value="" selected>${plural(e.recipe.args.runs.length, 'run')}</option>${layoutOptions('')}`;
    html += prow('Runs', `<select class="inp" data-chg="stairlayout" data-key="stair-own-layout" aria-label="How the runs are laid">${layouts}</select>`);
    const landing = e.recipe.args.landings?.[0]?.length;
    if (landing !== undefined) {
      html += prow('Landing', `<input class="inp" data-chg="stairlanding" data-key="stair-own-landing" value="${esc(`${storedMm(landing)} mm`)}" aria-label="Length of the landing">`);
    }
  } else {
    html += prow('Width', esc(stairWidthMm(e))) + prow('Direction', esc(e.recipe.args.direction));
  }
  if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  // The rules' refusal, in the server's words: the limit with its clause and the nearest heights that work.
  if (info?.refusal) html += `<div class="note bad small" role="alert" data-stair-refusal>${esc(info.refusal)}</div>`;
  html += problem('stair-edit');
  if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
  if (view) {
    html += readOnlyTable(view);
    html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  }
  if (editable) html += '<div class="pad"><button class="btn sm" data-act="stackdelete" data-key="stair-delete">Delete stair</button></div>';
  return html;
}

CHG.stairtop = (select) => {
  if (S.sel) manage('stair.top', { element: S.sel, top: select.value }, 'stair-edit');
};
CHG.stairrules = (select) => {
  if (S.sel) manage('stair.category', { element: S.sel, category: select.value }, 'stair-edit');
};
CHG.stairwidth = (input) => {
  if (S.sel) manage('stair.width', { element: S.sel, width: input.value }, 'stair-edit');
  input.blur();
};
CHG.stairdirection = (select) => {
  if (S.sel) manage('stair.direction', { element: S.sel, direction: select.value }, 'stair-edit');
};
CHG.stairlayout = (select) => {
  const piece = C.elements.get(S.sel);
  if (!piece || !select.value) return;
  const length = piece.el.recipe.args.landings?.[0]?.length ?? piece.el.recipe.args.width;
  manage('stair.runs', { element: S.sel, ...stairRuns(select.value, `${storedMm(length)} mm`) }, 'stair-edit');
};
CHG.stairlanding = (input) => {
  const piece = C.elements.get(S.sel);
  if (!piece) return;
  const here = stairLayoutOf(piece.el);
  if (here && here !== 'straight') manage('stair.runs', { element: S.sel, ...stairRuns(here, input.value) }, 'stair-edit');
  input.blur();
};

/** The properties palette's stair part: the stair tool's options, or a selected stair; '' for anything else. */
function stairPalette(piece) {
  if (S.tool === 'stair' && canEdit()) return stairToolHtml();
  if (piece && isStair(piece.el) && !(S.tool === 'wall' && canEdit())) return stairProperties(piece);
  return '';
}

/** A point of a stair's piece, in grid steps, as millimetres in the plan: for drawing only. */
const stairMm = (p) => [p[0] / STEPS_PER_MM, p[1] / STEPS_PER_MM];

/**
 * Where the open plan cuts a stair, in grid steps above the stair's base: the open view's cut
 * plane as the server read it, else the cut plane of a plan of the plan's level; null where the
 * server has not said (the stair is then drawn without a cut).
 */
function stairCut(info) {
  const view = stairsView();
  if (!view || !info?.base) return null;
  const plan = view.plans.find((p) => p.view === C.openView) ?? view.plans.find((p) => p.level === levelOf());
  return plan ? steps(plan.cut.value) - steps(info.base.value) : null;
}

/** The line from `a` to `b` in the plan, in millimetres, with the classes and data given. */
const stairLine = (a, b, classes, data = '') =>
  `<line class="${classes}" ${data} x1="${a[0]}" y1="${a[1]}" x2="${b[0]}" y2="${b[1]}"/>`;

/** The point halfway between two. */
const midway = (a, b) => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];

/** A closed path through `points`, each `[x, y]`. */
const stairRing = (points) => ringPath(points);

/** The walking line's arrow: a head at the last point of `line`, pointing the way the last piece of it goes. */
function stairArrow(line) {
  const [from, to] = line.slice(-2);
  const length = Math.hypot(to[0] - from[0], to[1] - from[1]) || 1;
  const unit = [(to[0] - from[0]) / length, (to[1] - from[1]) / length];
  const side = [-unit[1], unit[0]];
  const back = [to[0] - unit[0] * 250, to[1] - unit[1] * 250];
  const wing = (sign) => `${back[0] + side[0] * 110 * sign},${back[1] + side[1] * 110 * sign}`;
  return `<polygon class="stair-arrow" data-stair-arrow points="${to[0]},${to[1]} ${wing(1)} ${wing(-1)}"/>`;
}

/**
 * A stair in the plan, from its recipe as the server filled it: each run's treads, each run and
 * landing a polygon, the lines above the view's cut dashed, the cut line where the plane
 * crosses a run, and the walking line from the bottom ending in an arrow. The runs' and
 * landings' corners are the server's (`pieces`, in grid steps); nothing is laid out here.
 */
function stairInPlan(piece, data, scale) {
  const { el: e } = piece;
  const pieces = piece.fill?.recipe?.args?.pieces;
  const box = around(piece);
  const chosen = S.sel === e.id;
  let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
  if (!Array.isArray(pieces) || !pieces.length) {
    group += outlineOf(piece, `body stair${chosen ? ' sel' : ''}`);
    return `<g ${data}>${group}</g>`;
  }
  const cut = stairCut(stairsView()?.stairs.find((s) => s.id === e.id));
  // One body to pick it by: every run and landing in one path.
  group += `<path class="body stair${chosen ? ' sel' : ''}" d="${pieces.map((p) => stairRing(p.plan.map(stairMm))).join('')}"/>`;
  const walk = [];
  for (const p of pieces) {
    if (p.loft) {
      // A run's profile at each side: its first corner on the base, then each tread's nosing and
      // inner corner. A tread is above the cut plane when its top is higher than it.
      const [right, left] = p.loft;
      const lastAt = 2 * p.treads;
      let cutAt = null;
      for (let i = 0; i <= p.treads; i += 1) {
        const at = i === p.treads ? lastAt : 1 + 2 * i;
        const above = cut !== null && right[at][2] > cut;
        group += stairLine(stairMm(right[at]), stairMm(left[at]), `stair-tread${above ? ' stair-above' : ''}`, `data-stair-tread="${esc(p.id)}.${i}" data-above="${above}"`);
        if (above && cutAt === null && cut >= 0 && i < p.treads) cutAt = at;
      }
      if (cutAt !== null) group += stairLine(stairMm(right[cutAt]), stairMm(left[cutAt]), 'stair-cut', `data-stair-cut="${esc(p.id)}"`);
      walk.push(midway(stairMm(right[1]), stairMm(left[1])), midway(stairMm(right[lastAt]), stairMm(left[lastAt])));
    } else {
      const corners = p.plan.map(stairMm);
      const under = p.extrude[0][2];
      const state = cut === null || under + p.by[2] <= cut ? 'below' : under > cut ? 'above' : 'cut';
      group += `<path class="stair-landing stair-${state}" data-stair-landing="${esc(p.id)}" data-above="${state === 'above'}" d="${stairRing(corners)}"/>`;
      walk.push(midway(corners[0], corners[2])); // the walking line goes through its middle
    }
  }
  // The walking line, from the bottom: a dot where it starts, an arrow where it ends.
  const line = walk.filter((point, i) => i === 0 || point[0] !== walk[i - 1][0] || point[1] !== walk[i - 1][1]);
  if (line.length >= 2) {
    group += `<polyline class="stair-walk" data-stair-walk points="${line.map(([x, y]) => `${x},${y}`).join(' ')}"/>`;
    group += `<circle class="stair-walk-start" cx="${line[0][0]}" cy="${line[0][1]}" r="80"/>`;
    group += stairArrow(line);
  }
  const claim = heldClaim(e.id);
  if (claim) {
    const theirs = claim.person !== C.me.person.id;
    group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
    if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
  }
  return `<g ${data}>${group}</g>`;
}

const deleteBeforeStairs = ACT.stackdelete;
ACT.stackdelete = () => {
  const piece = C.elements.get(S.sel);
  if (!piece || !isStair(piece.el)) {
    deleteBeforeStairs();
    return;
  }
  if (!canEdit()) return;
  manage('stair.delete', { element: S.sel }, 'stair-edit');
  S.sel = null;
  refresh();
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'stair', discipline: 'architecture', order: 130, place: (at) => placeStair(at) });
register.ribbons({ id: 'stair', order: 30, ribbon: (config) => stairRibbon(config) });
register.hints({ id: 'stair', order: 40, hint: () => (S.tool === 'stair' ? stairHint() : undefined) });
register.palettes({ id: 'stair', order: 30, palette: (piece) => stairPalette(piece) || undefined });
register.elements({ id: 'stair', order: 50, owns: isStair, inPlan: (piece, data, scale) => stairInPlan(piece, data, scale), pickOnly: true });
register.logs({ id: 'stairs', order: 40, entry: (entry) => stairsChanged(entry) });
