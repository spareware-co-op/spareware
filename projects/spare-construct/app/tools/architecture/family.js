// Tool: placed families (a door or a window of the library, in a wall).

import { late, provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, modelUrl, say } from '../../core/state.js';
import {
  LAYERED, buildOn, builderFor, hostIdOf, hostOf, lineOf, nearestMm, seatAtAngle, steps, storedMm,
} from '../../core/shapes.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, problem } from '../../core/problems.js';
import { queueDraw } from '../../core/plan.js';
import { onWall } from './wall.js';


// --- Placed families: a door or a window of the library, in a wall ---
// The family tool lists the library's families and their types (`/families`), and a click on a wall
// sends one `family.place` edit: the family, its type, the wall, the side clicked and how far along
// the wall (construct-model's FORMAT.md, "Placed families"). A selected family shows its type and
// the values each placed one holds in the properties palette: another type is `family.retype`, a
// value `family.set`, a position `family.move` (a drag along its wall sends it too), the other side
// `family.flip`, and Delete is `family.delete`. The page checks nothing: a refusal is the server's
// words, under the options. In the plan and in 3D a family is what the browser's geometry builds
// from its record, standing where its wall puts it, turned to face out of its side; where the wall
// is cut (the kernel's package) the wall is built with it. Interim, as the library is (job 7.6): the
// families are the library's baked in, and the old box door tool stays beside this one until it is
// retired.

const FAMILY_OP = 'family.instance';
const isFamily = (record) => record?.recipe?.op === FAMILY_OP && record?.recipe?.owner === 'construct';
C.library = null; // { model, list } of the open model
C.familyTool = { family: null, type: null, values: {} }; // the tool's choices: a family's permanent ID, a type's key

DTOOLS.architecture.push(['family', 'Family', 'family', 'Choose a family and a type in the properties palette, then click on a wall to place it.']);
TOOLNAME.family = 'family';

/** The ribbon: the family tool beside the door tool, in the Build panel. */
function familyRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:family')) return;
  const door = build.items.findIndex((item) => item.a === 'architecture:door');
  build.items.splice(door + 1, 0, DT_('architecture', 'family'));
}

/** The library's families, with their types and the values each placed one holds; null until they have come. */
function library() {
  if (!C.model) return null;
  const model = C.model.id;
  if (C.library?.model !== model && C.libraryLoading !== model) {
    C.libraryLoading = model;
    api(`${modelUrl(model)}/families`)
      .then((list) => {
        C.library = { model, list };
        refreshSide();
      })
      .catch(() => {});
  }
  return C.library?.model === model ? C.library.list : null;
}

function chosenFamily(list) {
  return list.find((family) => family.id === C.familyTool.family) ?? list[0] ?? null;
}

function chosenFamilyType(family) {
  return family.types.find((type) => type.key === C.familyTool.type) ?? family.types[0] ?? null;
}

/** A stored value as a person reads it: a length in millimetres, a choice by its label. */
function shownValue(value, stored) {
  if (stored === undefined) return value.shown;
  if (value.kind === 'length') return `${storedMm(stored)} mm`;
  if (value.kind === 'choice') return value.choices.find((choice) => `choice:${choice.key}` === stored)?.label ?? stored;
  return stored;
}

/** A value's input: a choice is a list of its labels, anything else typed text. */
function valueInput(value, shown, key, chg = 'stackfamilyvalue') {
  if (value.kind === 'choice') {
    const options = value.choices.map((choice) => `<option${choice.label === shown ? ' selected' : ''}>${esc(choice.label)}</option>`);
    return `<select class="inp" data-chg="${chg}" data-key="${key}" data-a="${esc(value.key)}" aria-label="${esc(value.name)}">${options.join('')}</select>`;
  }
  return `<input class="inp" data-chg="${chg}" data-key="${key}" data-a="${esc(value.key)}" value="${esc(shown)}" aria-label="${esc(value.name)}">`;
}

function familyToolHtml() {
  let html = typeSel('family', 'Family', '<b>New door or window</b>');
  const list = library();
  if (!list) return `${html}<p class="pad small faint">Loading the library.</p>`;
  const family = chosenFamily(list);
  if (!family) return `${html}<p class="pad small faint">The library has no families.</p>`;
  const type = chosenFamilyType(family);
  const families = list.map((f) => `<option value="${esc(f.id)}"${f.id === family.id ? ' selected' : ''}>${esc(f.name)}</option>`);
  const types = family.types.map((t) => `<option value="${esc(t.key)}"${t.key === type.key ? ' selected' : ''}>${esc(t.name)}</option>`);
  html += '<table class="props">';
  html += prow('Family', `<select class="inp" data-chg="stackfamilychoice" data-key="family-choice" aria-label="Family">${families.join('')}</select>`);
  html += prow('Category', esc(family.category_name));
  html += prow('Family type', `<select class="inp" data-chg="stackfamilytype" data-key="family-type" aria-label="Type of the family">${types.join('')}</select>`);
  for (const value of family.values) {
    const typed = C.familyTool.values[value.key] ?? value.shown;
    html += prow(esc(value.name), valueInput(value, typed, `family-tool-value:${value.key}`, 'stackfamilytoolvalue'));
  }
  html += '</table>';
  html += '<p class="pad small muted">Click on a wall. The side you click is the side it goes on, and the click is the middle of its opening.</p>';
  return html + problem('family-place');
}

CHG.stackfamilychoice = (select) => {
  C.familyTool = { family: select.value, type: null, values: {} };
  refreshSide();
};
CHG.stackfamilytype = (select) => {
  C.familyTool.type = select.value;
};

/** Reads the families again: what the document holds, placed or made has changed, or may have. */
function reloadLibrary() {
  const model = C.model?.id;
  if (!model) return;
  C.libraryAsked = (C.libraryAsked ?? 0) + 1;
  const asked = C.libraryAsked;
  api(`${modelUrl(model)}/families`)
    .then((list) => {
      if (C.libraryAsked !== asked || C.model?.id !== model) return;
      C.library = { model, list };
      refreshSide();
    })
    .catch(() => {});
}

/** The family a placed one's record names: by the permanent id it carries, else by its recipe's content id. */
function familyOf(record) {
  const id = record.recipe.args.family_id;
  const list = library();
  return (id && list?.find((family) => family.id === id)) || list?.find((family) => family.content === record.recipe.args.family) || null;
}

/** Every other type a placed door or window of `family` may be swapped to: the same kind, any family of the list. */
function swapOptions(family, current) {
  const options = [];
  for (const other of library() ?? []) {
    if (other.kind !== family.kind) continue;
    for (const type of other.types) {
      if (other.id === family.id && type.key === current) continue;
      options.push(`<option value="${esc(other.id)}|${esc(type.key)}">${esc(other.name)}: ${esc(type.name)}</option>`);
    }
  }
  return options;
}

/** What the properties palette shows of a selected family: its type, its values, its place, its side. */
function familyProperties(piece) {
  const { el } = piece;
  const family = familyOf(el);
  if (!family) return '<p class="pad small faint">Reading its family from the library.</p>';
  const mine = !C.readOnly && canEdit();
  const types = family.types.map((t) => `<option value="${esc(t.key)}"${t.key === el.recipe.args.type ? ' selected' : ''}>${esc(t.name)}</option>`);
  let html = '<table class="props" data-family-properties="' + esc(el.id) + '">';
  html += prow('Family', esc(family.name));
  html += prow('Family type', mine ? `<select class="inp" data-chg="stackfamilyretype" data-key="family-retype" aria-label="Type of the family">${types.join('')}</select>` : esc(family.types.find((t) => t.key === el.recipe.args.type)?.name ?? el.recipe.args.type));
  if (mine) {
    const swaps = swapOptions(family, el.recipe.args.type);
    html += prow('Swap to', `<select class="inp" data-chg="stackfamilyswap" data-key="family-swap" aria-label="Swap to another type or family">${['<option value="" selected>Choose a type</option>', ...swaps].join('')}</select>`);
  }
  for (const value of family.values) {
    const shown = shownValue(value, el.recipe.args[`value.${value.key}`]);
    html += prow(esc(value.name), mine ? valueInput(value, shown, `family-value:${value.key}`) : esc(shown));
  }
  const along = el.props?.['@host.along'];
  const face = (el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
  if (along !== undefined) {
    html += prow('Along its wall', mine ? `<input class="inp" data-chg="stackfamilyalong" data-key="family-along" value="${storedMm(along)} mm" aria-label="Along its wall">` : `${storedMm(along)} mm`);
  }
  if (face) html += prow('Side', `${esc(face.signature ?? '')}${mine ? ' <button class="btn sm" data-act="stackfamilyflip">Flip</button>' : ''}`);
  html += '</table>';
  return html + problem(`family:${el.id}`);
}

CHG.stackfamilyretype = (select) => {
  if (S.sel && canEdit()) manage('family.retype', { element: S.sel, type: select.value }, `family:${S.sel}`);
};
CHG.stackfamilyswap = (select) => {
  const [to_family, to_type] = select.value.split('|');
  if (S.sel && canEdit() && to_family) manage('family.swap', { element: S.sel, to_family, to_type }, `family:${S.sel}`);
};
CHG.stackfamilyvalue = (input) => {
  if (!S.sel || !canEdit()) return;
  const typed = input.value.trim();
  manage('family.set', { element: S.sel, values: { [input.dataset.a]: typed === '' ? null : typed } }, `family:${S.sel}`);
  input.blur();
};
CHG.stackfamilyalong = (input) => {
  if (S.sel && canEdit()) manage('family.move', { element: S.sel, along: input.value }, `family:${S.sel}`);
  input.blur();
};
ACT.stackfamilyflip = () => {
  if (S.sel && canEdit()) manage('family.flip', { element: S.sel }, `family:${S.sel}`);
};

// The tool's own values, kept as typed until a family is placed.
CHG.stackfamilytoolvalue = (input) => {
  C.familyTool.values[input.dataset.a] = input.value;
};

/**
 * A click on a wall with the family tool: the family goes on the side clicked, its opening's
 * middle where the click is, as far along the wall from its start (the server checks that it fits).
 */
function placeFamily(at) {
  const list = library();
  const family = list && chosenFamily(list);
  if (!family) {
    say('The library has not come yet.');
    return;
  }
  const wall = late.wallAt(at);
  if (!wall) {
    say('Click on a wall to place it.');
    return;
  }
  const spot = onWall(wall, at, 0);
  if (!spot) {
    say('That wall has no side to place it on.');
    return;
  }
  const values = {};
  for (const [key, typed] of Object.entries(C.familyTool.values)) {
    if (family.values.some((value) => value.key === key) && typed !== '') values[key] = typed;
  }
  const args = {
    element: newId('F'),
    family: family.id,
    type: chosenFamilyType(family).key,
    host: wall.el.id,
    side: spot.exterior ? 'exterior' : 'interior',
    along: `${spot.offset} mm`,
  };
  if (Object.keys(values).length) args.values = values;
  manage('family.place', args, 'family-place');
}

/**
 * How a placed family sits on side `role` of a layered wall, as construct-regen's `hosting::seat`
 * works it out: its origin on the wall's centre plane, `along` the line from its start, `up` from
 * the wall's base, each to the nearest millimetre, and the turn (0 to 3 quarter turns about z) that
 * points its front out of that side. Null until the wall is filled, and for a wall at an angle,
 * which seats through `seatAtAngle`.
 */
function onFamily(role, host, along, up) {
  const line = lineOf(host);
  if (!line || line.axis === null || host.el.recipe.op !== LAYERED) return null;
  const centre = Math.floor((line.exterior + line.interior) / 2);
  const [ux, uy] = line.unit;
  const right = [uy, -ux];
  const outwardOfExterior = line.exterior > line.interior ? 1 : -1;
  const outward = role === 'interior' ? -outwardOfExterior : outwardOfExterior;
  const [nx, ny] = right.map((c) => c * outward);
  const turn = { '1,0': 0, '0,1': 1, '-1,0': 2, '0,-1': 3 }[`${ny},${-nx}`];
  if (turn === undefined) return null;
  const origin = [0, 1].map((i) => nearestMm(line.start[i] + steps(along) * line.unit[i] + centre * right[i]));
  const base = host.el.placement;
  return { at: { x: base.x + origin[0], y: base.y + origin[1], z: base.z + storedMm(up) }, turn };
}

const turnedRecipe = (recipe, turn) => ({ ...recipe, args: { ...recipe.args, turn } });

/** The recipe a piece is built from before its wall seats it: a family copy's filled recipe holds its family's text. */
const ownRecipe = (piece) => (isFamily(piece.el) ? piece.fill?.recipe ?? piece.el.recipe : piece.el.recipe);

/**
 * Where a hosted element sits and the recipe its builder is handed, on its wall (`host`, or the
 * one it names, if it is here), as construct-regen's `hosting::seat` works them out: `{at, recipe}`.
 * A placed family on a wall along an axis is handed its turn, anything in a wall at an angle its
 * seat (`seatAtAngle`); null for anything else, which is built as it is stored, and until the
 * wall is here and filled.
 */
function seatOfPiece(piece, host, builder = C.builder) {
  const hosted = hostOf(piece.el);
  const wall = host ?? hosted?.host;
  const face = hosted?.face ?? (piece.el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
  const along = piece.el.props?.['@host.along'];
  const up = piece.el.props?.['@host.up'];
  if (!wall || !face || along === undefined || up === undefined) return null;
  const role = wall.shape?.names?.find((name) => name.name === face.name)?.role ?? face.signature;
  const angled = seatAtAngle(builder, { ...piece.el, recipe: ownRecipe(piece) }, role, wall, along, up);
  if (angled) return angled;
  if (!isFamily(piece.el)) return null;
  const turned = onFamily(role, wall, along, up);
  return turned && { at: turned.at, recipe: turnedRecipe(ownRecipe(piece), turned.turn) };
}

/**
 * What a hosted piece is to be built from: `{seat}` with its seat (null: as stored), or null while
 * its wall is not here and filled enough to say, as a piece that is hosted but not yet seated.
 */
function seatWanted(piece) {
  const hosted = hostOf(piece.el);
  if (!hosted) return null;
  const { host } = hosted;
  if (host.el.recipe.op === LAYERED && !lineOf(host)) return null;
  return { seat: seatOfPiece(piece) };
}

/** The text a seat is told apart by: the recipe the builder is handed, or none. */
const seatKey = (seat) => (seat ? JSON.stringify(seat.recipe) : null);

/** Whether `piece` is built from other than the recipe its wall now seats it with. */
const seatChanged = (piece) => {
  const wanted = seatWanted(piece);
  return Boolean(wanted) && seatKey(wanted.seat) !== (piece.seatedKey ?? null);
};

/**
 * Builds each hosted piece again, as its wall seats it, where it is not built so yet (a placed
 * family turned to stand in its wall; anything in a wall at an angle with its seat), and lets a
 * hosted piece built as it is stored be compared once its wall has said so.
 */
function seatFamilies() {
  if (!C.model || C.viewing) return;
  for (const piece of C.elements.values()) {
    if (piece.turning || piece.failed || hostIdOf(piece.el) === null) continue;
    const wanted = seatWanted(piece);
    if (!wanted) continue;
    if (seatKey(wanted.seat) !== (piece.seatedKey ?? null)) reseat(piece, wanted.seat);
    else if (piece.stale && !isFamily(piece.el)) piece.stale = false;
  }
}

async function reseat(piece, seat) {
  piece.turning = true;
  try {
    const row = C.models.get(C.model.id);
    const builder = row && (await builderFor(row));
    if (!builder) return;
    const recipe = seat ? seat.recipe : ownRecipe(piece);
    const answer = buildOn(builder, { ...piece.el, recipe }, null, null);
    Object.assign(piece, { shape: undefined, failed: undefined }, answer, { seatedKey: seatKey(seat), stale: false });
    queueDraw();
  } finally {
    piece.turning = false;
  }
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'family', discipline: 'architecture', order: 90, place: (at) => placeFamily(at) });
register.ribbons({ id: 'family', order: 50, ribbon: (config) => familyRibbon(config) });
register.hints({
  id: 'family',
  order: 90,
  hint: () => (S.tool === 'family' ? '<b>Family:</b> choose a family and a type in the properties palette, then click on a wall to place it.' : undefined),
});
register.palettes({ id: 'family', order: 70, palette: () => (S.tool === 'family' && canEdit() ? familyToolHtml() : undefined) });
register.properties({ id: 'family', order: 10, properties: (piece) => (isFamily(piece.el) ? familyProperties(piece) : undefined) });
register.elements({ id: 'family', order: 70, owns: isFamily, move: 'family.move' });
register.layers({ id: 'family-seats', order: 10, before: () => seatFamilies() });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get FAMILY_OP() { return FAMILY_OP; }, get isFamily() { return isFamily; },
  get library() { return library; }, get onFamily() { return onFamily; },
  get reloadLibrary() { return reloadLibrary; }, get seatChanged() { return seatChanged; },
  get seatFamilies() { return seatFamilies; }, get seatOfPiece() { return seatOfPiece; },
});

export { isFamily };
