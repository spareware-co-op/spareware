// Tool: components (furniture, casework, plumbing fixtures, lighting, planting and generic models).

import { late } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, levelOf, say } from '../../core/state.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, problem } from '../../core/problems.js';
import { snap } from '../../core/plan.js';
import { onWall } from './wall.js';
import { library, valueInput } from './family.js';

// --- Components: furniture, casework, plumbing fixtures, lighting, planting and generic models ---
// (job 14.9, piece A90). The Component tool lists the library's families that are not doors or
// windows, by category, then family, then type (`/families`: each family says what it is placed
// on). A family that stands on a level is placed by one click in the plan, its point the middle of
// it, turned by the quarter turn chosen and raised by the offset typed, on the level the plan shows;
// a family that hangs against a wall (a wall cabinet, a basin) is placed by a click on a wall's
// face, as a door is. Each is one `family.place` edit (construct-model's FORMAT.md, "Placed
// families"). A selected one shows its type, its values, its point, its turn and its offset (or its
// place along its wall and its side) in the properties palette: `family.retype`, `family.set`,
// `family.move`, `family.turn`, `family.flip`; Delete is `family.delete`; dragging one that stands
// moves it. The page checks nothing: a refusal is the server's words, under the options.

C.componentTool = { category: null, family: null, type: null, values: {}, turn: 0, offset: '0 mm' }; // the tool's choices

DTOOLS.architecture.push([
  'component',
  'Component',
  'cube',
  'Choose a category, a family and a type in the properties palette, then click in the plan to place it (on a wall for one that hangs on it).',
]);
TOOLNAME.component = 'component';

/** The ribbon: the component tool after the opening and shaft tools, in the Build panel. */
function componentRibbon(config) {
  const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
  if (!build || build.items.some((item) => item.a === 'architecture:component')) return;
  const shaft = build.items.findIndex((item) => item.a === 'architecture:shaft');
  build.items.splice(shaft >= 0 ? shaft + 1 : build.items.length, 0, DT_('architecture', 'component'));
}

/** The library's families that do not cut a wall: what the component tool offers; null until they have come. */
const components = () => library()?.filter((family) => family.hosting === 'unhosted' || family.hosting === 'face') ?? null;

/** The tool's choices made whole: a category, the families in it, one of them and one of its types. */
function chosenComponent(list) {
  const categories = [...new Set(list.map((family) => family.category))];
  const category = categories.includes(C.componentTool.category) ? C.componentTool.category : categories[0];
  const inCategory = list.filter((family) => family.category === category);
  const family = inCategory.find((f) => f.id === C.componentTool.family) ?? inCategory[0] ?? null;
  const type = family && (family.types.find((t) => t.key === C.componentTool.type) ?? family.types[0] ?? null);
  return { categories, category, inCategory, family, type };
}

function componentToolHtml() {
  let html = typeSel('cube', 'Component', '<b>New component</b>');
  const list = components();
  if (!list) return `${html}<p class="pad small faint">Loading the library.</p>`;
  const { categories, category, inCategory, family, type } = chosenComponent(list);
  if (!family || !type) return `${html}<p class="pad small faint">The library has no components.</p>`;
  const option = (value, label, chosen) => `<option value="${esc(value)}"${chosen ? ' selected' : ''}>${esc(label)}</option>`;
  const names = new Map(list.map((f) => [f.category, f.category_name]));
  const select = (chg, key, label, options) =>
    `<select class="inp" data-chg="${chg}" data-key="${key}" aria-label="${label}">${options.join('')}</select>`;
  html += '<table class="props" data-component-tool="yes">';
  html += prow('Category', select('stackcomponentcategory', 'component-category', 'Category', categories.map((c) => option(c, names.get(c), c === category))));
  html += prow('Family', select('stackcomponentfamily', 'component-family', 'Family', inCategory.map((f) => option(f.id, f.name, f.id === family.id))));
  html += prow('Family type', select('stackcomponenttype', 'component-type', 'Type of the family', family.types.map((t) => option(t.key, t.name, t.key === type.key))));
  for (const value of family.values) {
    const typed = C.componentTool.values[value.key] ?? value.shown;
    html += prow(esc(value.name), valueInput(value, typed, `component-tool-value:${value.key}`, 'stackcomponenttoolvalue'));
  }
  if (family.hosting === 'unhosted') {
    const turns = [0, 1, 2, 3].map((q) => option(q, `${q * 90}°`, q === C.componentTool.turn));
    html += prow('Turn', select('stackcomponentturn', 'component-turn', 'Turn of the component', turns));
    html += prow('Offset from the level', `<input class="inp" data-chg="stackcomponentoffset" data-key="component-offset" value="${esc(C.componentTool.offset)}" aria-label="Offset from the level">`);
  }
  html += '</table>';
  html += family.hosting === 'face'
    ? '<p class="pad small muted">Click on a wall. The side you click is the side it hangs on, and the click is the middle of it.</p>'
    : '<p class="pad small muted">Click in the plan. The click is the middle of it.</p>';
  return html + problem('component-place');
}

CHG.stackcomponentcategory = (select) => {
  C.componentTool = { ...C.componentTool, category: Number(select.value), family: null, type: null, values: {} };
  refreshSide();
};
CHG.stackcomponentfamily = (select) => {
  C.componentTool = { ...C.componentTool, family: select.value, type: null, values: {} };
  refreshSide();
};
CHG.stackcomponenttype = (select) => {
  C.componentTool.type = select.value;
};
CHG.stackcomponentturn = (select) => {
  C.componentTool.turn = Number(select.value);
};
CHG.stackcomponentoffset = (input) => {
  C.componentTool.offset = input.value;
};
// The tool's own values, kept as typed until a component is placed.
CHG.stackcomponenttoolvalue = (input) => {
  C.componentTool.values[input.dataset.a] = input.value;
};

/**
 * A click with the component tool: a family that stands is put at the point clicked, on the level
 * the plan shows, turned and raised as the tool says; one that hangs on a wall goes on the side of
 * the wall clicked, the click the middle of it. The server checks it.
 */
function placeComponent(at) {
  const list = components();
  const { family, type } = list ? chosenComponent(list) : {};
  if (!family || !type) {
    say('The library has not come yet.');
    return;
  }
  const args = { element: newId('F'), family: family.id, type: type.key };
  const values = {};
  for (const [key, typed] of Object.entries(C.componentTool.values)) {
    if (family.values.some((value) => value.key === key) && typed !== '') values[key] = typed;
  }
  if (Object.keys(values).length) args.values = values;
  if (family.hosting === 'face') {
    const wall = late.wallAt(at);
    if (!wall) {
      say('Click on a wall to hang it.');
      return;
    }
    const spot = onWall(wall, at, 0);
    if (!spot) {
      say('That wall has no side to hang it on.');
      return;
    }
    Object.assign(args, { host: wall.el.id, side: spot.exterior ? 'exterior' : 'interior', along: `${spot.offset} mm` });
  } else {
    Object.assign(args, { x: `${snap(at.x)} mm`, y: `${snap(at.y)} mm`, level: levelOf(), turn: String(C.componentTool.turn * 90) });
    const offset = C.componentTool.offset.trim();
    if (offset !== '' && !/^0(\.0*)?\s*(mm)?$/.test(offset)) args.up = offset;
  }
  manage('family.place', args, 'component-place');
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'component', discipline: 'architecture', order: 95, place: (at) => placeComponent(at) });
register.ribbons({ id: 'component', order: 65, ribbon: (config) => componentRibbon(config) });
register.hints({
  id: 'component',
  order: 95,
  hint: () => (S.tool === 'component' ? '<b>Component:</b> choose a category, a family and a type in the properties palette, then click in the plan to place it (on a wall for one that hangs on it).' : undefined),
});
register.palettes({ id: 'component', order: 75, palette: () => (S.tool === 'component' && canEdit() ? componentToolHtml() : undefined) });
