// Tool: curtain walls (the Curtain Wall tool, a curtain wall in the plan and its properties).

import { late, provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, levelOf, modelUrl, say } from '../../core/state.js';
import { LAYERED, STEPS_PER_MM, lineOf } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { paramsFor, refreshSide } from '../../core/parameters.js';
import { canEdit, capital, manage, newId, plural, problem } from '../../core/problems.js';
import { heldClaim, hint, snap } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { materialName, onWall, wallTop } from './wall.js';
import { readOnlyTable } from './floor.js';

// --- Curtain walls: the Curtain Wall tool, a curtain wall in the plan and its properties ---
// A curtain wall is drawn between two clicked points, as a wall is, from the plan's level to the
// level above it or a height (`curtain.draw`), or embedded in a wall by picking that wall
// (`curtain.embed`: the side clicked, how far along, its length and height typed in the palette,
// construct-model's FORMAT.md, "Curtain walls"). Its type is chosen in the palette from the
// model's curtain wall types (`/curtain-types`; where a model has none the page has the server
// make its default ones through the log, `curtain.defaults`). A selected curtain wall shows its
// type (`curtain.retype`), what the server works out of it read-only (length, height, area,
// panels and their area, glazed area, mullions and their length), its grid as the server laid it
// out (`/filled/<element>`: every line and every panel) and edits it a command at a time: a grid
// line added or removed (`curtain.grid.add`, `curtain.grid.remove`), a segment of a line left out
// or kept again (`curtain.segment.omit`, `curtain.segment.restore`), the panel of a cell swapped
// (`curtain.panel`: glazed, solid, empty, a door of the library, or its type's own), and Delete
// (`curtain.delete`). The page works out no grid and checks nothing: it names lines and cells by
// the exact text the server gave, and a refusal is the server's words under the controls. In the
// plan and in 3D a curtain wall is the panels and mullions its display lists, each in its
// material's colour; one embedded in a wall stands where the server seats it (the filled recipe
// and `placement` it answers with), and the wall is built with the opening it cuts.

const CURTAIN_OP = 'wall.curtain';
const isCurtain = (record) => record?.kind === 'curtain' && record?.recipe?.op === CURTAIN_OP && record?.recipe?.owner === 'construct';
const SEATED = new Map(); // an embedded curtain wall's ID -> where the server seats its record
C.curtainTypes = null; // { key, view } of the open model
C.curtainTool = { mode: 'draw', type: null, top: undefined, height: '3000 mm', length: '2400', embedHeight: '2100 mm', up: '0 mm', direction: 'vertical', glazing: null, solid: null };
C.madeCurtainDefaults = null; // the model the default curtain wall types were asked for

DTOOLS.architecture.push(['curtain', 'Curtain Wall', 'wall', 'Pick the two ends of a curtain wall, or choose to embed it and pick a wall.']);
TOOLNAME.curtain = 'curtain wall';
Object.assign(WORDS, {
  'curtain.draw': 'drew a curtain wall',
  'curtain.embed': 'embedded a curtain wall in a wall:',
  'curtain.retype': 'changed the type of',
  'curtain.grid.add': 'added a grid line to',
  'curtain.grid.remove': 'removed a grid line from',
  'curtain.segment.omit': 'left out a segment of',
  'curtain.segment.restore': 'kept a segment of',
  'curtain.panel': 'swapped a panel of',
  'curtain.delete': 'deleted',
  'curtain.defaults': 'made the default curtain wall types',
  'curtain.type.create': 'made a curtain wall type',
  'curtain.type.spec': 'changed a curtain wall type',
  'curtain.type.rename': 'renamed a curtain wall type',
  'curtain.type.duplicate': 'copied a curtain wall type',
  'curtain.type.delete': 'deleted a curtain wall type',
});
Object.assign(STEP_WORDS, {
  'curtain.draw': 'drawing a curtain wall',
  'curtain.embed': 'embedding a curtain wall',
  'curtain.retype': 'a change of a curtain wall’s type',
  'curtain.grid.add': 'adding a grid line',
  'curtain.grid.remove': 'removing a grid line',
  'curtain.segment.omit': 'leaving out a segment',
  'curtain.segment.restore': 'keeping a segment',
  'curtain.panel': 'swapping a panel',
  'curtain.delete': 'deleting a curtain wall',
  'curtain.defaults': 'making the default curtain wall types',
  'curtain.type.create': 'making a curtain wall type',
  'curtain.type.spec': 'a curtain wall type’s grid',
  'curtain.type.rename': 'renaming a curtain wall type',
  'curtain.type.duplicate': 'copying a curtain wall type',
  'curtain.type.delete': 'deleting a curtain wall type',
});

/** The ribbon: the curtain wall tool beside the roof tool, in the Build panel. */
function curtainRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:curtain')) return;
  const after = build.items.findIndex((item) => item.a === 'architecture:roof');
  const wall = build.items.findIndex((item) => item.a === 'architecture:wall');
  build.items.splice((after >= 0 ? after : wall) + 1, 0, DT_('architecture', 'curtain'));
}

/** The open model's curtain wall types, their grids and parts, and the materials; null until they have come. */
function curtainTypes() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}`;
  if (C.curtainTypes?.key !== key && C.curtainTypesLoading !== key) {
    C.curtainTypesLoading = key;
    api(`${modelUrl(model)}/curtain-types`)
      .then((view) => {
        C.curtainTypes = { key, view };
        refreshSide();
      })
      .catch(() => {});
  }
  return C.curtainTypes?.key.startsWith(`${model}/`) ? C.curtainTypes.view : null;
}

/** The open model's curtain wall types, read now if they have not come yet. */
async function curtainTypesNow() {
  const view = curtainTypes();
  if (view) return view;
  const model = C.model.id;
  const fresh = await api(`${modelUrl(model)}/curtain-types`);
  C.curtainTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
  return fresh;
}

const chosenCurtainType = (view) => view.types.find((t) => t.id === C.curtainTool.type) ?? view.types[0] ?? null;

/** Has the server make its default curtain wall types, once for a model, where it has none. */
function makeDefaultCurtains() {
  if (!canEdit() || C.madeCurtainDefaults === C.model.id) return;
  C.madeCurtainDefaults = C.model.id;
  manage('curtain.defaults', {}, 'curtain-draw');
}

/** A layout as a sentence, from what the server listed. */
function layoutWords(layout) {
  if (layout.layout === 'spacing') return `Every ${layout.spacing}, from the ${layout.justify}`;
  if (layout.layout === 'count') return `${plural(layout.count, 'equal panel')}`;
  if (layout.layout === 'most') return `Panels at most ${layout.spacing} wide`;
  return 'None';
}

function curtainToolHtml() {
  let html = typeSel('wall', 'Curtain wall', '<b>New curtain wall</b>');
  const view = curtainTypes();
  if (!view) return `${html}<p class="pad small faint">Loading the curtain wall types.</p>`;
  if (!view.types.length) {
    queueMicrotask(makeDefaultCurtains);
    return `${html}<p class="pad small faint" data-curtain-tool="making">This model has no curtain wall types yet, so the default ones are being made.</p>${problem('curtain-draw')}`;
  }
  const chosen = chosenCurtainType(view);
  const levels = modelLevels() || [];
  const here = levelOf();
  const embed = C.curtainTool.mode === 'embed';
  const modes = [['draw', 'Draw on a line'], ['embed', 'Embed in a wall']]
    .map(([id, name]) => `<option value="${id}"${id === C.curtainTool.mode ? ' selected' : ''}>${name}</option>`);
  const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
  const field = (key, label, value) => `<input class="inp" data-chg="stackcurtaintool" data-a="${key}" data-key="curtain-${key}" value="${esc(value)}" aria-label="${esc(label)}">`;
  html += '<table class="props" data-curtain-tool="options">';
  html += prow('Place', `<select class="inp" data-chg="stackcurtainmode" data-key="curtain-mode" aria-label="How the curtain wall is placed">${modes.join('')}</select>`);
  html += prow('Type', `<select class="inp" data-chg="stackcurtaintype" data-key="curtain-type" aria-label="Curtain wall type">${types.join('')}</select>`);
  html += prow('Vertical lines', esc(layoutWords(chosen.spec.vertical)));
  html += prow('Horizontal lines', esc(layoutWords(chosen.spec.horizontal)));
  html += prow('Panel', esc(`${capital(chosen.spec.panel.kind)}${chosen.spec.panel.thickness ? `, ${chosen.spec.panel.thickness}` : ''}`));
  if (embed) {
    html += prow('Length (mm)', field('length', 'Length of the curtain wall in mm', C.curtainTool.length));
    html += prow('Height', field('embedHeight', 'Height of the curtain wall', C.curtainTool.embedHeight));
    html += prow('Up from its wall’s base', field('up', 'How far up its wall the curtain wall starts', C.curtainTool.up));
  } else {
    const top = wallTop(levels);
    const tops = levels
      .filter((level) => level.id !== here)
      .map((level) => `<option value="${esc(level.id)}"${level.id === top ? ' selected' : ''}>${esc(level.name)}</option>`);
    tops.push(`<option value=""${top === '' ? ' selected' : ''}>A height</option>`);
    html += prow('Base', esc(levels.find((level) => level.id === here)?.name ?? here));
    html += prow('Top', `<select class="inp" data-chg="stackwalltop" data-key="curtain-top" aria-label="Top of the curtain wall">${tops.join('')}</select>`);
    if (top === '') html += prow('Height', field('height', 'Height of the curtain wall', C.curtainTool.height));
  }
  html += '</table>';
  html += embed
    ? '<p class="pad small muted">Click on a wall. The side you click is the side it goes on, and the click is the middle of the curtain wall.</p>'
    : '<p class="pad small muted">Pick two points in the plan. Walking from the first to the second, the exterior is on the right.</p>';
  return html + problem('curtain-draw');
}

CHG.stackcurtainmode = (select) => {
  C.curtainTool.mode = select.value;
  S.pend = null;
  $('#hint').innerHTML = hint();
  refreshSide();
};
CHG.stackcurtaintype = (select) => {
  C.curtainTool.type = select.value;
  refreshSide();
};
CHG.stackcurtaintool = (input) => {
  C.curtainTool[input.dataset.a] = input.value;
};

function curtainHint() {
  if (C.curtainTool.mode === 'embed') return '<b>Curtain wall:</b> click on a wall to embed one in it. Esc to stop.';
  return S.pend ? '<b>Curtain wall:</b> pick the end point. Esc to stop.' : '<b>Curtain wall:</b> pick a start point. Choose its type in the properties palette.';
}

/** The tool's click: the second point of a curtain wall's line, or the wall it is embedded in. */
async function placeCurtain(at) {
  if (C.curtainTool.mode === 'embed') {
    await embedCurtain(at);
    return;
  }
  const point = [snap(at.x), snap(at.y)];
  if (!S.pend) {
    S.pend = point;
    $('#hint').innerHTML = hint();
    return;
  }
  const [ax, ay] = S.pend;
  S.pend = null;
  $('#hint').innerHTML = hint();
  const along = Math.abs(point[0] - ax) >= Math.abs(point[1] - ay);
  const end = along ? [point[0], ay] : [ax, point[1]];
  let view;
  try {
    view = await curtainTypesNow();
  } catch (error) {
    say(error.message);
    return;
  }
  const args = { element: newId('CW'), start: [`${ax} mm`, `${ay} mm`], end: [`${end[0]} mm`, `${end[1]} mm`], level: levelOf() };
  const type = chosenCurtainType(view);
  if (type) args.type = type.id;
  const top = wallTop(modelLevels());
  if (top === '') args.height = C.curtainTool.height;
  else args.top = top;
  manage('curtain.draw', args, 'curtain-draw');
}

/** Embeds a curtain wall in the layered wall under the click: the side clicked, its middle at the click. */
async function embedCurtain(at) {
  // The layered wall nearest the click among those it is on (within the margin the door tool
  // gives): a wall is its line and its thickness, at any angle (piece A83), not the box round it,
  // so a neighbour's box or margin never takes the click.
  let wall = null;
  let nearest = Infinity;
  for (const piece of C.elements.values()) {
    const line = piece.el.recipe.op === LAYERED && piece.shape ? lineOf(piece) : null;
    if (!line) continue;
    const { x, y } = piece.el.placement;
    const p = [at.x - x - line.start[0] / STEPS_PER_MM, at.y - y - line.start[1] / STEPS_PER_MM];
    const length = Math.hypot(line.d[0], line.d[1]);
    const alongIt = (p[0] * line.d[0] + p[1] * line.d[1]) / length;
    const right = (p[0] * line.d[1] - p[1] * line.d[0]) / length;
    const [low, high] = [line.exterior, line.interior].map((offset) => offset / STEPS_PER_MM).sort((m, n) => m - n);
    if (alongIt < -300 || alongIt > length / STEPS_PER_MM + 300 || right < low - 300 || right > high + 300) continue;
    const away = Math.abs(right - (low + high) / 2);
    if (away < nearest) [wall, nearest] = [piece, away];
  }
  if (!wall) {
    say('Click on a wall to embed a curtain wall in it.');
    return;
  }
  const width = Number.parseFloat(C.curtainTool.length);
  if (!Number.isFinite(width)) {
    say('Type its length in millimetres in the palette first.');
    return;
  }
  const spot = onWall(wall, at, width);
  if (!spot) {
    say('That wall has no side to embed a curtain wall in.');
    return;
  }
  let view;
  try {
    view = await curtainTypesNow();
  } catch (error) {
    say(error.message);
    return;
  }
  const args = {
    element: newId('CW'),
    host: wall.el.id,
    side: spot.exterior ? 'exterior' : 'interior',
    along: `${spot.offset} mm`,
    up: C.curtainTool.up,
    length: `${width} mm`,
    height: C.curtainTool.embedHeight,
  };
  const type = chosenCurtainType(view);
  if (type) args.type = type.id;
  manage('curtain.embed', args, 'curtain-draw');
}

/** The door families of the library, with their types: what a panel can be swapped to. */
const doorFamilies = () => (late.library() || []).filter((family) => family.kind === 'door');

/** The panel a swap to `kind` sends: the wall's type's thickness, in the material chosen for it. */
function infillFor(kind, view, type) {
  const spec = type?.spec.panel.thickness ? type.spec.panel : view.defaults[0]?.spec?.panel;
  if (!spec) return null;
  const glass = spec.kind === 'glazed' ? spec.material : view.materials.find((m) => m.id === 'glass-plate')?.id ?? view.materials[0]?.id;
  const solid = view.materials.find((m) => m.id !== glass)?.id ?? glass;
  const material = (kind === 'glazed' ? C.curtainTool.glazing ?? glass : C.curtainTool.solid ?? solid);
  return { kind, thickness: spec.thickness, offset: spec.offset, material };
}

/** The value of a panel's swap list for a panel the server listed. */
function panelChoice(panel) {
  if (!panel.swapped) return 'type';
  if (panel.kind !== 'door') return panel.kind;
  const family = doorFamilies().find((f) => f.content === panel.door[0]);
  return family ? `door:${family.id}:${panel.door[1]}` : `door:${panel.door[0]}:${panel.door[1]}`;
}

/** What a swap list's value sends as the panel; null is the type's own. */
function panelFor(choice, view, type) {
  if (choice === 'type') return null;
  if (choice === 'empty') return { kind: 'empty' };
  if (choice.startsWith('door:')) {
    const [, family, ...key] = choice.split(':');
    return { kind: 'door', family, type: key.join(':') };
  }
  return infillFor(choice, view, type);
}

/** The panels' swap lists and the grid's lines, for a selected curtain wall. */
function curtainGrid(piece, view, types) {
  const grid = piece.fill?.curtain;
  if (!grid) return '';
  const own = view?.type?.id;
  const type = types?.types.find((t) => t.id === own) ?? null;
  let html = '<span class="lbl">Grid lines</span><table class="props" data-curtain-lines>';
  for (const line of grid.lines.filter((l) => !l.border)) {
    const remove = `<button class="btn sm" data-act="stackcurtainremove" data-d="${line.direction}" data-at="${esc(line.at)}" aria-label="Remove the ${line.direction} line at ${esc(line.shown)}">Remove</button>`;
    html += prow(esc(capital(line.direction)), `<span class="mono" data-curtain-line="${line.direction}:${esc(line.at)}">${esc(line.shown)}</span>${line.added ? ' <span class="faint">added</span>' : ''} ${remove}`);
  }
  html += '</table>';
  const directions = ['vertical', 'horizontal']
    .map((d) => `<option value="${d}"${d === C.curtainTool.direction ? ' selected' : ''}>${capital(d)}</option>`);
  html += '<table class="props">';
  html += prow('Add a line', `<select class="inp" data-chg="stackcurtaindirection" data-key="curtain-line-direction" aria-label="Direction of the new line">${directions.join('')}</select>`);
  html += prow('At', `<input class="inp" data-chg="stackcurtainadd" data-key="curtain-line-at" placeholder="1200 mm" aria-label="Where the new line stands: along the wall, or up it">`);
  html += '</table>';
  html += '<span class="lbl">Segments</span><table class="props" data-curtain-segments>';
  for (const line of grid.lines.filter((l) => !l.border)) {
    const chips = line.segments
      .map((s) => `<button class="btn sm" data-act="stackcurtainsegment" data-d="${line.direction}" data-at="${esc(line.at)}" data-from="${esc(s.from)}" data-omitted="${s.omitted ? 1 : 0}" aria-pressed="${s.omitted}" aria-label="${s.omitted ? 'Keep' : 'Leave out'} the segment of the ${line.direction} line at ${esc(line.shown)} that ends at ${esc(s.to)}">${s.omitted ? 'Left out to ' : 'To '}${esc(s.to)}</button>`)
      .join(' ');
    html += prow(`${esc(capital(line.direction))} ${esc(line.shown)}`, chips);
  }
  html += '</table>';
  if (types) {
    const materials = types.materials.map((m) => m.id);
    const pick = (chosen) => materials
      .map((id) => `<option value="${esc(id)}"${id === chosen ? ' selected' : ''}>${esc(materialName(types, id))}</option>`)
      .join('');
    const glazing = infillFor('glazed', types, type)?.material;
    const solid = infillFor('solid', types, type)?.material;
    html += '<span class="lbl">Panels</span><table class="props">';
    html += prow('Glazed with', `<select class="inp" data-chg="stackcurtainmaterial" data-a="glazing" data-key="curtain-glazing" aria-label="Material of a glazed panel">${pick(glazing)}</select>`);
    html += prow('Solid with', `<select class="inp" data-chg="stackcurtainmaterial" data-a="solid" data-key="curtain-solid" aria-label="Material of a solid panel">${pick(solid)}</select>`);
    html += '</table><table class="props" data-curtain-panels>';
    const doors = doorFamilies().flatMap((family) => family.types.map((t) => [`door:${family.id}:${t.key}`, `Door: ${family.name}, ${t.name}`]));
    const options = [['type', 'The type’s panel'], ['glazed', 'Glazed'], ['solid', 'Solid'], ['empty', 'Empty'], ...doors];
    for (const panel of grid.panels) {
      const chosen = panelChoice(panel);
      const list = options.some(([id]) => id === chosen) ? options : [...options, [chosen, 'A door']];
      const choices = list.map(([id, name]) => `<option value="${esc(id)}"${id === chosen ? ' selected' : ''}>${esc(name)}</option>`).join('');
      const where = `${panel.shown[0]}, ${panel.shown[1]}`;
      html += prow(`<span class="mono">${esc(where)}</span>`, `<select class="inp" data-chg="stackcurtainpanel" data-along="${esc(panel.at[0])}" data-up="${esc(panel.at[1])}" data-key="curtain-panel:${esc(panel.at[0])}:${esc(panel.at[1])}" aria-label="Panel at ${esc(where)}, ${esc(panel.width)} by ${esc(panel.height)}">${choices}</select>`, `${esc(panel.width)} × ${esc(panel.height)}`);
    }
    html += '</table>';
  }
  if (grid.unused.length) html += `<p class="pad small muted" data-curtain-unused>${esc(grid.unused.join(' '))}</p>`;
  return html;
}

/** What the properties palette shows of a selected curtain wall. */
function curtainProperties(piece) {
  const { el: e, shape, failed } = piece;
  const view = paramsFor(piece);
  const types = curtainTypes();
  const claim = heldClaim(e.id);
  const editable = canEdit();
  let html = typeSel('wall', 'Curtain wall', `<b>${esc(e.name)}</b>`);
  html += '<table class="props" data-curtain-element>';
  if (view) html += prow('Category', esc(view.category?.name || 'None'));
  if (types && editable) {
    const options = types.types.map((t) => `<option value="${esc(t.id)}"${t.id === view?.type?.id ? ' selected' : ''}>${esc(t.name)}</option>`);
    html += prow('Type', `<select class="inp" data-chg="stackcurtainretype" data-key="curtain-own-type" aria-label="Curtain wall type">${options.join('')}</select>`);
  } else if (view) {
    html += prow('Type', esc(view.type?.name || 'None'));
  }
  html += prow('Level', esc(levelName(e)));
  for (const host of e.references) html += prow('Embedded in', esc(host.element));
  if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  html += problem('curtain-edit');
  if (view) html += readOnlyTable(view);
  if (editable) html += curtainGrid(piece, view, types);
  if (view) html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  if (editable) html += '<div class="pad"><button class="btn sm" data-act="stackdelete" data-key="curtain-delete">Delete curtain wall</button></div>';
  return html;
}

/** The properties palette's curtain wall part: the tool's options, or a selected curtain wall; '' for anything else. */
function curtainPalette(piece) {
  if (S.tool === 'curtain' && canEdit()) return curtainToolHtml();
  if (piece && isCurtain(piece.el) && !(S.tool === 'wall' && canEdit())) return curtainProperties(piece);
  return '';
}

CHG.stackcurtainretype = (select) => {
  if (S.sel) manage('curtain.retype', { element: S.sel, type: select.value }, 'curtain-edit');
};
CHG.stackcurtaindirection = (select) => {
  C.curtainTool.direction = select.value;
};
CHG.stackcurtainadd = (input) => {
  if (!S.sel || !input.value.trim()) return;
  manage('curtain.grid.add', { element: S.sel, direction: C.curtainTool.direction, at: input.value }, 'curtain-edit');
  input.value = '';
  input.blur();
};
CHG.stackcurtainmaterial = (select) => {
  C.curtainTool[select.dataset.a] = select.value;
  refreshSide();
};
CHG.stackcurtainpanel = (select) => {
  const types = curtainTypes();
  const piece = C.elements.get(S.sel);
  if (!types || !piece) return;
  const own = paramsFor(piece)?.type?.id;
  const type = types.types.find((t) => t.id === own) ?? null;
  manage('curtain.panel', { element: S.sel, at: [select.dataset.along, select.dataset.up], panel: panelFor(select.value, types, type) }, 'curtain-edit');
};
ACT.stackcurtainremove = (button) => {
  if (S.sel) manage('curtain.grid.remove', { element: S.sel, direction: button.dataset.d, at: button.dataset.at }, 'curtain-edit');
};
ACT.stackcurtainsegment = (button) => {
  if (!S.sel) return;
  const cmd = button.dataset.omitted === '1' ? 'curtain.segment.restore' : 'curtain.segment.omit';
  manage(cmd, { element: S.sel, direction: button.dataset.d, at: button.dataset.at, from: button.dataset.from }, 'curtain-edit');
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'curtain', discipline: 'architecture', order: 70, place: (at) => placeCurtain(at) });
register.ribbons({ id: 'curtain', order: 40, ribbon: (config) => curtainRibbon(config) });
register.hints({ id: 'curtain', order: 60, hint: () => (S.tool === 'curtain' ? curtainHint() : undefined) });
register.palettes({ id: 'curtain', order: 40, palette: (piece) => curtainPalette(piece) || undefined });
register.elements({ id: 'curtain', order: 60, owns: isCurtain, drags: (el, hosted) => !hosted });

// What files loaded before this one use from it, bound once it has loaded.
provide({ get SEATED() { return SEATED; }, get isCurtain() { return isCurtain; } });
