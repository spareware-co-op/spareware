// Tool: the roof tool, a roof in the plan and the properties palette.

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, levelOf, modelUrl, say } from '../../core/state.js';
import { around, nearestMm, placementOf, storedMm } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { parametersTable, paramsFor, refreshSide } from '../../core/parameters.js';
import { canEdit, capital, manage, newId, problem } from '../../core/problems.js';
import { heldClaim, queueDraw } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { colourOf, materialName } from './wall.js';
import { FLOOR, READ_ONLY, readOnlyTable, ringPath, sketchChanged, sketchHtml } from './floor.js';

// --- The roof tool, and a roof in the plan and the properties palette ---
// A roof is made over a footprint sketched by clicks in the plan, as a floor's edge is: one loop
// of corners closed on its first corner or with Enter; Enter on no open loop, or Make roof,
// sends one `roof.create` of the roof type, form, pitch and overhang chosen in the properties
// palette, its base on the plan's level the offset typed there above it (construct-model's
// FORMAT.md, "Roofs"). The page checks nothing and works out no geometry: the roof types, their
// layers and the materials come from the server (`/roof-types`); where a model has none the page
// has the server make its default ones through the log (`roof.defaults`); a refusal (two
// pitches meeting at a hip, a pitch on a footprint that is not right-angled, a hole) is the server's reason in the
// server's words, under the tool's options, with the footprint it turned back drawn in the plan.
// In the plan a roof is its eaves outline from its recipe as the server filled it
// (`/filled/<element>`) and, where it slopes, the lines of its top face (ridge, hips, valleys,
// verges) as the server listed them there; a selected roof shows its type, level, form (flat,
// mono-pitch, gable or hip), pitch, a pitch for each edge it rises from where there are two or
// more (an uneven gable), and overhang, which are changed there (`roof.retype`, `roof.form`,
// `roof.pitch` with or without `edge`, `roof.overhang`), what the server works out of it, and
// Delete removes it (`roof.delete`).

const ROOF_OP = 'roof';
const isRoof = (record) => record?.kind === 'roof' && record?.recipe?.op === ROOF_OP;

DTOOLS.architecture.push(['roof', 'Roof', 'family', 'Click the corners of its footprint. Close the loop on its first corner or with Enter.']);
TOOLNAME.roof = 'roof';
Object.assign(WORDS, {
  'roof.create': 'made a roof',
  'roof.footprint': 'changed the footprint of',
  'roof.form': 'changed the form of',
  'roof.pitch': 'changed the pitch of',
  'roof.overhang': 'changed the overhang of',
  'roof.level': 'tied to a level',
  'roof.retype': 'changed the type of',
  'roof.delete': 'deleted',
  'roof.defaults': 'made the default roof types',
  'roof.type.create': 'made a roof type',
  'roof.type.layers': 'changed the layers of a roof type',
  'roof.type.rename': 'renamed a roof type',
  'roof.type.duplicate': 'copied a roof type',
  'roof.type.delete': 'deleted a roof type',
});
Object.assign(STEP_WORDS, {
  'roof.create': 'making a roof',
  'roof.footprint': 'a roof’s footprint',
  'roof.form': 'a roof’s form',
  'roof.pitch': 'a roof’s pitch',
  'roof.overhang': 'a roof’s overhang',
  'roof.level': 'a roof’s level',
  'roof.retype': 'a change of a roof’s type',
  'roof.delete': 'deleting a roof',
  'roof.defaults': 'making the default roof types',
  'roof.type.create': 'making a roof type',
  'roof.type.layers': 'the layers of a roof type',
  'roof.type.rename': 'renaming a roof type',
  'roof.type.duplicate': 'copying a roof type',
  'roof.type.delete': 'deleting a roof type',
});

C.roofTypes = null; // { key, view } of the open model
C.roofTool = { type: null, form: 'gable', pitch: '1:4', overhang: '300 mm', offset: '0 mm', eaves: '' };
C.madeRoofDefaults = null; // the model the default roof types were asked for

/** The open model's roof types and materials; null until they have come. */
function roofTypes() {
  if (!C.model) return null;
  const model = C.model.id;
  const key = `${model}/${C.paramsEpoch}`;
  if (C.roofTypes?.key !== key && C.roofTypesLoading !== key) {
    C.roofTypesLoading = key;
    api(`${modelUrl(model)}/roof-types`)
      .then((view) => {
        C.roofTypes = { key, view };
        refreshSide();
      })
      .catch(() => {});
  }
  return C.roofTypes?.key.startsWith(`${model}/`) ? C.roofTypes.view : null;
}

/** The open model's roof types, read now if they have not come yet. */
async function roofTypesNow() {
  const view = roofTypes();
  if (view) return view;
  const model = C.model.id;
  const fresh = await api(`${modelUrl(model)}/roof-types`);
  C.roofTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
  return fresh;
}

const chosenRoofType = (view) => view.types.find((t) => t.id === C.roofTool.type) ?? view.types[0] ?? null;

/** Has the server make its default roof types, once for a model, where it has none. */
function makeDefaultRoofs() {
  if (!canEdit() || C.madeRoofDefaults === C.model.id) return;
  C.madeRoofDefaults = C.model.id;
  manage('roof.defaults', {}, 'roof-draw');
}

const ROOF_FORMS = [['flat', 'Flat'], ['mono', 'Mono-pitch'], ['gable', 'Gable'], ['hip', 'Hip']];

function roofToolHtml() {
  let html = typeSel('family', 'Roof', '<b>New roof</b>');
  const view = roofTypes();
  if (!view) return `${html}<p class="pad small faint">Loading the roof types.</p>`;
  if (!view.types.length) {
    queueMicrotask(makeDefaultRoofs);
    return `${html}<p class="pad small faint" data-roof-tool="making">This model has no roof types yet, so the default ones are being made.</p>${problem('roof-draw')}`;
  }
  const chosen = chosenRoofType(view);
  const here = levelOf();
  const level = (modelLevels() || []).find((l) => l.id === here)?.name ?? here;
  const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
  const forms = ROOF_FORMS.map(([id, name]) => `<option value="${id}"${id === C.roofTool.form ? ' selected' : ''}>${name}</option>`);
  const field = (key, label, value, extra = '') => `<input class="inp" data-chg="stackroof${key}" data-key="roof-${key}" value="${esc(value)}" aria-label="${esc(label)}"${extra}>`;
  html += '<table class="props">';
  html += prow('Type', `<select class="inp" data-chg="stackrooftool" data-key="roof-type" aria-label="Roof type">${types.join('')}</select>`);
  html += prow('Form', `<select class="inp" data-chg="stackroofformtool" data-key="roof-form" aria-label="Roof form">${forms.join('')}</select>`);
  if (C.roofTool.form !== 'flat') html += prow('Pitch', field('pitch', 'Pitch, as 1:4, 25% or 30°', C.roofTool.pitch));
  if (C.roofTool.form !== 'flat') html += prow('Rises from', field('eaves', 'Edge numbers it rises from, blank for automatic', C.roofTool.eaves, ' placeholder="automatic"'));
  html += prow('Overhang', field('overhang', 'Overhang past its edges', C.roofTool.overhang));
  html += prow('Thickness', esc(chosen.thickness));
  html += prow('Level', esc(level));
  html += prow('Offset', field('offset', 'Offset of the roof’s base above its level', C.roofTool.offset));
  html += '</table>';
  html += `<span class="lbl">Layers, outside first</span><table class="props" data-roof-layers="${esc(chosen.id)}">`;
  for (const layer of chosen.layers) {
    html += prow(esc(capital(layer.function)), `${esc(layer.thickness)} · ${esc(materialName(view, layer.material))}`);
  }
  html += '</table>';
  return html + sketchHtml() + problem('roof-draw');
}

CHG.stackrooftool = (select) => {
  C.roofTool.type = select.value;
  refreshSide();
};
CHG.stackroofformtool = (select) => {
  C.roofTool.form = select.value;
  refreshSide();
};
for (const key of ['pitch', 'eaves', 'overhang', 'offset']) {
  CHG[`stackroof${key}`] = (input) => {
    C.roofTool[key] = input.value;
  };
}
ACT.stackroofmake = () => makeRoof();

function roofHint() {
  if (FLOOR.current.length) return '<b>Roof:</b> click the next corner, or its first corner or Enter to close the loop. Esc to stop.';
  if (FLOOR.loops.length) return '<b>Roof:</b> press Enter to make the roof. Esc to stop.';
  return '<b>Roof:</b> click the corners of its footprint. Choose its type, form and pitch in the properties palette.';
}

/** Sends the sketch as it is: what the server makes of it, or why it refuses, comes back through the log. */
async function makeRoof() {
  if (!FLOOR.loops.length || FLOOR.current.length || !canEdit()) return;
  let view;
  try {
    view = await roofTypesNow();
  } catch (error) {
    say(error.message);
    return;
  }
  const corners = (loop) => loop.map(([x, y]) => ({ at: [`${x} mm`, `${y} mm`] }));
  const [outer, ...holes] = FLOOR.loops;
  const tool = C.roofTool;
  const args = { element: newId('R'), level: levelOf(), form: tool.form, footprint: { outer: corners(outer), holes: holes.map(corners) } };
  const type = chosenRoofType(view);
  if (type) args.type = type.id;
  if (tool.form !== 'flat' && tool.pitch.trim() !== '') args.pitch = tool.pitch;
  if (tool.overhang.trim() !== '') args.overhang = tool.overhang;
  if (tool.offset.trim() !== '') args.offset = tool.offset;
  // Edges are numbered from 1 in the order the corners were clicked; the server names them o0, o1, ...
  const numbers = tool.eaves.split(/[\s,]+/).filter((n) => n !== '');
  if (tool.form !== 'flat' && numbers.length) args.eaves = numbers.map((n) => `o${Number(n) - 1}`);
  const sent = { loops: FLOOR.loops };
  Object.assign(FLOOR, { loops: [], current: [], at: null, sent });
  manage('roof.create', args, 'roof-draw', () => {
    if (FLOOR.sent === sent) FLOOR.sent = null;
    queueDraw();
  });
  sketchChanged();
}

/** The ribbon: the roof tool beside the floor tool, in the Build panel. */
function roofRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:roof')) return;
  const floor = build.items.findIndex((item) => item.a === 'architecture:floor');
  build.items.splice(floor + 1, 0, DT_('architecture', 'roof'));
}

/** The point `[x, y, z]` of a solid, in whole millimetres in plan. */
const planPoint = (p) => [nearestMm(p[0]), nearestMm(p[1])];

/**
 * A roof in the plan: its eaves outline from its recipe as the server filled it (the outline of
 * its display until that has come), in its outer layer's colour seen from above, and, where it
 * slopes, the lines of its top face (ridge, hips, valleys, verges) as the server worked them out.
 */
function roofInPlan(piece, data, scale) {
  const { el: e } = piece;
  const { x, y } = placementOf(e);
  const args = piece.fill?.recipe?.args;
  const outline = Array.isArray(args?.outline) && args.outline.length
    ? args.outline.map((corner) => planPoint(corner.at))
    : piece.shape.displays[0].outline;
  const ring = outline.map(([px, py]) => [x + px, y + py]);
  const outer = args?.layers?.[0];
  const colour = outer && colourOf(outer.material, 'surface');
  const fill = colour ? ` style="fill:${esc(colour)}"` : '';
  const box = around(piece);
  const chosen = S.sel === e.id;
  let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
  group += `<path class="body roof${chosen ? ' sel' : ''}" data-form="${esc(args?.form ?? '')}" data-material="${esc(outer?.material ?? '')}"${fill} d="${ringPath(ring)}"/>`;
  // The ridges, hips, valleys and verges as the server gave them in the filled recipe (`lines`,
  // in grid steps); an eave is the outline itself. Nothing is worked out here.
  for (const line of Array.isArray(args?.lines) ? args.lines : []) {
    if (line.kind === 'eave') continue;
    const [a, b] = [line.from, line.to].map(planPoint);
    const ridge = line.kind === 'ridge' ? ' data-roof-ridge' : '';
    group += `<line class="roof-line roof-${esc(line.kind)}" data-roof-line="${esc(line.kind)}"${ridge} x1="${x + a[0]}" y1="${y + a[1]}" x2="${x + b[0]}" y2="${y + b[1]}"/>`;
  }
  const claim = heldClaim(e.id);
  if (claim) {
    const theirs = claim.person !== C.me.person.id;
    group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
    if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
  }
  return `<g ${data}>${group}</g>`;
}

/** A stored pitch typed as a person would: `slope:1/4` as 1:4, `angle:270000000` as 30°. */
function typedPitch(stored) {
  if (stored.startsWith('slope:')) return stored.slice(6).replace('/', ':');
  if (stored.startsWith('angle:')) return `${Number(stored.slice(6)) / 9000000}°`;
  return stored;
}

/**
 * What the properties palette reads of a selected roof's record: its form, pitch and overhangs,
 * and the pitch of each edge it rises from (numbered from 1, as the roof tool numbers them). Its
 * form is the one the server's filled recipe names (`solved` is a hip roof) once that has come.
 */
function roofSpecs(piece) {
  const e = piece.el;
  const edges = Array.isArray(e.recipe.args.edges) ? e.recipe.args.edges : [];
  const sloping = edges.filter((edge) => edge.pitch);
  const form = ['flat', 'mono', 'gable'][Math.min(sloping.length, 2)] ?? 'flat';
  const filled = piece.fill?.recipe?.args?.form;
  const overhangs = [...new Set(edges.map((edge) => `${storedMm(edge.overhang)} mm`))];
  return {
    form: filled === 'solved' ? 'hip' : filled ?? (sloping.length > 2 ? 'hip' : form),
    pitch: sloping[0] ? typedPitch(sloping[0].pitch) : '',
    overhang: overhangs.length === 1 ? overhangs[0] : '',
    edges: edges.flatMap((edge, at) => (edge.pitch ? [{ id: edge.id, number: at + 1, pitch: typedPitch(edge.pitch) }] : [])),
  };
}

/** The base's offset above its level, as stored. */
function roofOffset(e) {
  const stored = e.props?.['@level.offset'];
  return `${stored === undefined ? 0 : storedMm(stored)} mm`;
}

/** What the properties palette shows of a selected roof. */
function roofProperties(piece) {
  const { el: e, shape, failed } = piece;
  const view = paramsFor(piece);
  const types = roofTypes();
  const claim = heldClaim(e.id);
  const specs = roofSpecs(piece);
  const editable = canEdit();
  let html = typeSel('family', 'Roof', `<b>${esc(e.name)}</b>`);
  html += '<table class="props" data-roof-element>';
  if (view) html += prow('Category', esc(view.category?.name || 'None'));
  if (types && editable) {
    const options = types.types.map((t) => `<option value="${esc(t.id)}"${t.id === view?.type?.id ? ' selected' : ''}>${esc(t.name)}</option>`);
    html += prow('Type', `<select class="inp" data-chg="stackrooftype" data-key="roof-own-type" aria-label="Roof type">${options.join('')}</select>`);
  } else if (view) {
    html += prow('Type', esc(view.type?.name || 'None'));
  }
  html += prow('Level', esc(levelName(e))) + prow('Offset', `<span class="mono" data-roof-offset>${esc(roofOffset(e))}</span>`);
  if (editable) {
    const forms = ROOF_FORMS.map(([id, name]) => `<option value="${id}"${id === specs.form ? ' selected' : ''}>${name}</option>`);
    html += prow('Form', `<select class="inp" data-chg="stackroofshape" data-key="roof-own-form" aria-label="Roof form">${forms.join('')}</select>`);
    if (specs.form !== 'flat') html += prow('Pitch', `<input class="inp" data-chg="stackroofown" data-a="pitch" data-key="roof-own-pitch" value="${esc(specs.pitch)}" aria-label="Pitch, as 1:4, 25% or 30°">`);
    html += prow('Overhang', `<input class="inp" data-chg="stackroofown" data-a="overhang" data-key="roof-own-overhang" value="${esc(specs.overhang)}" placeholder="varies" aria-label="Overhang past its edges">`);
    // One pitch for every edge it rises from, or a pitch of its own for each (an uneven gable).
    if (specs.edges.length > 1) {
      for (const edge of specs.edges) {
        html += prow(`Edge ${edge.number} pitch`, `<input class="inp" data-chg="stackroofedge" data-edge="${esc(edge.id)}" data-key="roof-edge-pitch:${edge.number}" value="${esc(edge.pitch)}" aria-label="Pitch of edge ${edge.number}, as 1:4, 25% or 30°">`);
      }
    }
  } else {
    html += prow('Form', esc(specs.form));
    if (specs.pitch) html += prow('Pitch', esc(specs.pitch));
  }
  if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
  html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
  html += problem('roof-edit');
  if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
  if (view) {
    html += readOnlyTable(view);
    html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  }
  if (editable) html += '<div class="pad"><button class="btn sm" data-act="stackdelete" data-key="roof-delete">Delete roof</button></div>';
  return html;
}

CHG.stackrooftype = (select) => {
  if (S.sel) manage('roof.retype', { element: S.sel, type: select.value }, 'roof-edit');
};
CHG.stackroofshape = (select) => {
  const piece = C.elements.get(S.sel);
  if (!piece) return;
  const args = { element: S.sel, form: select.value };
  // Going from flat to a form that slopes needs a pitch: the tool's.
  if (select.value !== 'flat' && !roofSpecs(piece).pitch) args.pitch = C.roofTool.pitch;
  manage('roof.form', args, 'roof-edit');
};
CHG.stackroofown = (input) => {
  if (!S.sel) return;
  const cmd = input.dataset.a === 'pitch' ? 'roof.pitch' : 'roof.overhang';
  manage(cmd, { element: S.sel, [input.dataset.a]: input.value }, 'roof-edit');
  input.blur();
};

CHG.stackroofedge = (input) => {
  if (!S.sel) return;
  manage('roof.pitch', { element: S.sel, pitch: input.value, edge: input.dataset.edge }, 'roof-edit');
  input.blur();
};

/** The properties palette's roof part: the roof tool's options, or a selected roof; '' for anything else. */
function roofPalette(piece) {
  if (S.tool === 'roof' && canEdit()) return roofToolHtml();
  if (piece && isRoof(piece.el) && !(S.tool === 'wall' && canEdit())) return roofProperties(piece);
  return '';
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'roof', discipline: 'architecture', order: 30 });
register.ribbons({ id: 'roof', order: 20, ribbon: (config) => roofRibbon(config) });
register.palettes({ id: 'roof', order: 20, palette: (piece) => roofPalette(piece) || undefined });
register.elements({ id: 'roof', order: 40, owns: isRoof, inPlan: (piece, data, scale) => roofInPlan(piece, data, scale) });
register.drawOrders({ id: 'roof', order: 10, drawOrder: (el) => (el.kind === 'roof' ? -2 : undefined) });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get makeRoof() { return makeRoof; }, get roofHint() { return roofHint; },
  get typedPitch() { return typedPitch; },
});
