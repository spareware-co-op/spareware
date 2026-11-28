// Tool: sections and elevations as views, their marks in the plan and the view opened in the page.

import { late } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, modelUrl } from '../../core/state.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { canEdit, manage, problem } from '../../core/problems.js';
import { queueDraw } from '../../core/plan.js';
import { refreshSide } from '../../core/parameters.js';
import { loadViews } from '../project/views.js';

// --- Sections and elevations ---
// A section or an elevation is a view of the model (construct-views' FORMAT.md): a line in the plan,
// the side it looks to and how far it sees (its depth, which is also its far clip). This module
// shows one in two places, and changes nothing the server decides.
//
// In a plan, each section and elevation of the model is a mark: the line with a head (a bubble
// holding an arrow to the side it looks to) at its second end and a tail at its first. A press on
// a mark selects it, a second press on it within a moment opens the view, and the properties palette
// of a selected one changes its name, depth, ends and side (`view.rename`, `view.set`,
// `view.line.flip`, `view.line.move`, `view.delete`); the server refuses what is wrong in words, in
// a slot beside it.
//
// Opened, the view takes the plan's place in the plan pane: the layer `section-view` says it
// replaces the live elements (register.layers, `replaces`), and draws what the server draws of it at
// `/views/<view>/section`: the cut regions filled by material, the outlines beyond, the levels and
// grids that cross it, in the view's own x along its line and y up. The server's picture is asked
// for again, after a short pause, when anything in the model or the view changes. A line the
// drawing cannot cut yet (not along a grid axis), or a server with no geometry, is the server's
// refusal, shown above the pane in its own words.

/** What is open: the view, its key in the demo's view tables, the plan it came from, the picture. */
const V = { open: null, seq: 0, timer: null, last: null };

/** The open model's views that have a line: sections and elevations, as the server lists them. */
const lined = () => (C.views || []).filter((view) => view.line);
const lineOf = (id) => lined().find((view) => view.id === id);
/** A length the server wrote (`1200 mm`) as millimetres. */
const mm = (text) => Number.parseFloat(text);

/** True while the plan pane shows a section or an elevation instead of a plan. */
const viewing = () => Boolean(V.open) && S.screen === 'plan' && S.view === V.open.key;

Object.assign(WORDS, {
  'view.line.move': 'moved a section',
  'view.line.flip': 'turned a section round',
  'view.elevation.create': 'made an elevation',
});
Object.assign(STEP_WORDS, {
  'view.line.move': 'moving a section',
  'view.line.flip': 'turning a section round',
  'view.elevation.create': 'making an elevation',
});

// --- The marks in the plan ---

/** One section's or elevation's mark: its line, its head and tail, its name. */
function markSvg(view, s) {
  const { from, to } = view.line;
  const a = [mm(from.x), mm(from.y)];
  const b = [mm(to.x), mm(to.y)];
  const run = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
  const d = [(b[0] - a[0]) / run, (b[1] - a[1]) / run];
  // The side it looks to, in the plan's own x and y, as the server reads the line.
  const left = [-d[1], d[0]];
  const look = view.line.look === 'right' ? [-left[0], -left[1]] : left;
  const picked = S.sel === view.id;
  const section = view.kind === 'section';
  const r = 380 * s;
  let g = L(a, b, `ln ${section ? 'ap dd' : 'dsh'}${picked ? ' hv' : ''}`) + L(a, b, 'datum-hit');
  const at = (p, along, across) => [p[0] + d[0] * along + look[0] * across, p[1] + d[1] * along + look[1] * across];
  // The head, beyond the second end: a bubble with an arrow to the side it looks to.
  const head = at(b, r, 0);
  g += `<circle class="bub" data-section-head="${esc(view.id)}" cx="${n0(head[0])}" cy="${n0(head[1])}" r="${n0(r)}"/>`;
  g += PL([at(head, -r * 0.5, -r * 0.3), at(head, 0, r * 0.75), at(head, r * 0.5, -r * 0.3)], 'ln ap', true).replace('class="ln ap"', `class="ln ap secarrow" data-section-arrow="${esc(view.id)}"`);
  // The tail, at the first end: a bar across the line, to both sides.
  g += L(at(a, 0, -r * 0.8), at(a, 0, r * 0.8), 'ln ap');
  g += `<g data-section-tail="${esc(view.id)}">${L(at(a, 0, -r * 0.8), at(a, 0, r * 0.8), 'datum-hit')}</g>`;
  const name = at(head, 0, -r - 160 * s);
  g += TX(name[0], name[1] + (look[1] > 0 ? 0 : 300 * s), s, esc(view.name), 'tn', 300);
  const marks = `data-section="${esc(view.id)}" data-kind="${esc(view.kind)}" data-name="${esc(view.name)}" data-x0="${a[0]}" data-y0="${a[1]}" data-x1="${b[0]}" data-y1="${b[1]}" data-look="${esc(view.line.look)}"${picked ? ' data-picked="yes"' : ''}`;
  return `<g class="secmark" ${marks}>${g}</g>`;
}

/** Every section and elevation in the plan, drawn over the elements; none while one is open. */
function marksSvg(s) {
  if (viewing() || S.screen !== 'plan') return '';
  return lined().map((view) => markSvg(view, s)).join('');
}

// --- Opening a view ---

/** The server's picture of a section or an elevation, as the lines and hatches and words it holds. */
const DRAWN_TAGS = new Set(['g', 'path', 'line', 'text']);
const DRAWN_ATTRS = new Set([
  'd', 'x1', 'y1', 'x2', 'y2', 'x', 'y', 'fill', 'stroke', 'stroke-width', 'stroke-linecap', 'opacity', 'data-element', 'data-role',
  'data-material', 'font-family', 'font-size', 'text-anchor', 'dominant-baseline',
]);

function plainSvg(root) {
  let out = '';
  for (const child of root.children) {
    const tag = child.localName;
    if (!DRAWN_TAGS.has(tag)) continue;
    const attrs = [...child.attributes]
      .filter((attr) => DRAWN_ATTRS.has(attr.name))
      .map((attr) => ` ${attr.name}="${esc(attr.value)}"`)
      .join('');
    out += `<${tag}${attrs}>${tag === 'text' ? esc(child.textContent) : plainSvg(child)}</${tag}>`;
  }
  return out;
}

/** Reads the server's document: what it draws and the box round it, in the view's own units. */
function readPicture(text) {
  const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
  const root = doc.documentElement;
  if (root.localName !== 'svg') return { body: '', box: null };
  const box = (root.getAttribute('viewBox') || '').split(/\s+/).map(Number);
  return { body: plainSvg(root), box: box.length === 4 && box.every(Number.isFinite) ? box : null };
}

/** Asks the server for the open view's picture and keeps what comes back. */
async function loadSection() {
  const open = V.open;
  const model = C.model?.id;
  if (!open || !model) return;
  V.seq += 1;
  const seq = V.seq;
  let drawn;
  try {
    const answer = await api(`${modelUrl(model)}/views/${encodeURIComponent(open.view)}/section`);
    const picture = readPicture(answer.svg);
    drawn = { svg: answer.svg, skipped: answer.skipped, ...picture };
  } catch (error) {
    drawn = { error: error.message };
  }
  if (V.open !== open || C.model?.id !== model || seq !== V.seq) return;
  open.drawn = drawn;
  if (drawn.box) {
    // A little room round the drawing; zoom to fit shows all of it.
    const [x, y, w, h] = drawn.box;
    VB[open.key] = [x - w * 0.04, y - h * 0.06, w * 1.08, h * 1.12];
  }
  queueDraw();
}

/** Asks for the open view's picture again, once the edits arriving together have all come. */
function sectionChanged() {
  if (!viewing()) return;
  clearTimeout(V.timer);
  V.timer = setTimeout(loadSection, 150);
}

/** Opens a section or an elevation in the plan pane, drawn by the server. */
function openSection(id) {
  const view = lineOf(id);
  if (!view) return;
  late.leaveView?.();
  const key = `sec-${id}`;
  const home = viewing() ? V.open.home : FLOORV.includes(S.view) ? S.view : 'l1';
  V.open = { view: id, key, home, drawn: null };
  VIEWS[key] = { name: view.name, kind: view.kind === 'section' ? 'Section' : 'Elevation', sc: `1:${view.scale}` };
  if (!VB[key]) VB[key] = [-2000, -6000, 12000, 8000];
  S.tool = 'select';
  S.pend = null;
  S.sel = id;
  ACT.view({ dataset: { a: key } });
  loadSection();
}

/** Back to the plan the view was opened from. */
function leaveSection() {
  if (!viewing()) return;
  const { home } = V.open;
  S.view = home;
  HOMEV.architecture = home;
  go(`plan:${home}`);
}

/** The picture, in place of the plan: no flip, because the view's y already runs up the sheet. */
function drawnSection() {
  if (!viewing() || !V.open.drawn?.body) return '';
  return `<g class="drawn" data-section-view="${esc(V.open.view)}">${V.open.drawn.body}</g>`;
}

/** A line above the pane saying what is missing of the picture, and what the server refused. */
function sectionNote() {
  const asked = problem('new-section') + problem('new-elevation');
  if (!viewing()) return asked;
  const drawn = V.open.drawn;
  let text;
  let cls = 'stack-plan-note';
  if (!drawn) text = 'Drawing the view.';
  else if (drawn.error) {
    text = drawn.error;
    cls += ' bad';
  } else if (drawn.skipped.length) {
    text = `Not drawn: ${drawn.skipped.map((skipped) => skipped.element).join(', ')}.`;
  } else return asked;
  return `${asked}<div class="${cls}" role="status" data-section-note>${esc(text)}</div>`;
}

// --- The properties palette ---

function palette() {
  // While another tool is in use its own options are the palette.
  if (!viewing() && S.tool !== 'select') return undefined;
  const id = viewing() ? V.open.view : S.sel;
  const view = lineOf(id);
  if (!view) return undefined;
  const key = esc(view.id);
  const off = canEdit() ? '' : ' disabled';
  const what = view.kind === 'section' ? 'Section' : 'Elevation';
  const field = (name, value, label) => `<input class="inp" data-chg="stacksecline" data-key="sec-${name}:${key}" data-sec="${name}" value="${esc(value)}" aria-label="${label}"${off}>`;
  const name = `<input class="inp" data-chg="stacksecname" data-key="sec-name:${key}" data-view="${key}" value="${esc(view.name)}" aria-label="Name of ${esc(view.name)}"${off}>`;
  const { from, to } = view.line;
  let html = typeSel('section', what, `<b>${esc(view.name)}</b>`);
  html += `<table class="props" data-section-palette="${key}">${prow('Name', name)}`;
  html += prow('Looks to', `<span data-section-look>${esc(view.line.look)}</span> <button class="btn sm" data-act="stacksecflip" data-a="${key}" data-key="sec-flip:${key}"${off}>Flip</button>`);
  html += prow('Far clip', field('depth', view.line.depth, 'How far it sees, square to its line'), 'How far it sees beyond its line.');
  html += prow('First end', `${field('x0', from.x, 'First end, x')}${field('y0', from.y, 'First end, y')}`);
  html += prow('Second end', `${field('x1', to.x, 'Second end, x')}${field('y1', to.y, 'Second end, y')}`);
  html += prow('Scale', `1:${view.scale}`);
  html += `</table>${problem(`section:${view.id}`)}`;
  html += '<div class="pad stack-line">';
  html += `<button class="btn pri sm" data-act="stacksecopen" data-a="${key}" data-key="sec-open:${key}">Open</button> `;
  html += `<button class="btn sm" data-act="stackviewcrop" data-a="${key}" data-key="sec-crop:${key}" aria-label="Crop and range of ${esc(view.name)}">Crop and range</button> `;
  html += `<button class="btn sm" data-act="stacksecdelete" data-a="${key}" data-key="sec-delete:${key}"${off}>Delete</button></div>`;
  html += `<p class="pad small faint">${viewing() ? 'This is the view the server draws.' : 'Press a second time on its mark to open it.'}</p>`;
  return html;
}

CHG.stacksecname = (input) => manage('view.rename', { view: input.dataset.view, name: input.value }, `section:${input.dataset.view}`);
CHG.stacksecline = (input) => {
  const id = viewing() ? V.open.view : S.sel;
  if (!lineOf(id)) return;
  const read = (name) => $(`#pv-side [data-sec="${name}"]`).value;
  if (input.dataset.sec === 'depth') {
    manage('view.set', { view: id, depth: read('depth') }, `section:${id}`);
    return;
  }
  const end = input.dataset.sec.endsWith('0') ? 'from' : 'to';
  const n = input.dataset.sec.slice(1);
  manage('view.set', { view: id, [end]: { x: read(`x${n}`), y: read(`y${n}`) } }, `section:${id}`);
};
ACT.stacksecflip = (button) => manage('view.line.flip', { view: button.dataset.a }, `section:${button.dataset.a}`);
ACT.stacksecopen = (button) => openSection(button.dataset.a);
ACT.stacksecdelete = (button) => {
  const id = button.dataset.a;
  manage('view.delete', { view: id }, `section:${id}`);
  if (S.sel === id) S.sel = null;
  leaveSection();
};

// Keep the browser's "Model views" list working for these views: opening one is this module's.
const openAView = ACT.stackview;
ACT.stackview = (button) => {
  if (lineOf(button.dataset.a)) openSection(button.dataset.a);
  else openAView(button);
};

// Choosing a tool while a view is open goes back to the plan the view came from first.
const chooseTool = ACT.tool;
ACT.tool = (button) => {
  leaveSection();
  chooseTool(button);
};

// Delete removes a selected section or elevation as it does any selection.
const deleteSelected = ACT.stackdelete;
ACT.stackdelete = () => {
  const view = lineOf(S.sel);
  if (!view) {
    deleteSelected();
    return;
  }
  if (canEdit()) ACT.stacksecdelete({ dataset: { a: view.id } });
};

// A press on a mark selects it; a second press on it within a moment opens the view.
document.addEventListener(
  'pointerdown',
  (event) => {
    if (event.button !== 0 || S.screen !== 'plan' || S.tool !== 'select' || !event.target.closest?.('#pv-canvas svg')) return;
    const hit = event.target.closest('g.secmark');
    if (!hit) return;
    event.stopPropagation();
    const id = hit.dataset.section;
    const now = performance.now();
    if (V.last?.id === id && now - V.last.at < 700) {
      V.last = null;
      openSection(id);
      return;
    }
    V.last = { id, at: now };
    if (S.sel !== id) select(id);
  },
  true,
);
document.addEventListener(
  'dblclick',
  (event) => {
    const hit = event.target.closest?.('#pv-canvas g.secmark');
    if (hit && S.tool === 'select' && !viewing()) openSection(hit.dataset.section);
  },
  true,
);

/** An entry of the log arrived: the views are read again when a view changed, and an open one is drawn again. */
function logged(entry) {
  if (/^view\.|^edit\.(undo|redo)$/.test(entry.cmd)) {
    loadViews().then(() => {
      queueDraw();
      if (S.sel && lineOf(S.sel)) refreshSide();
      renderRibbon();
      if (V.open && !lineOf(V.open.view)) leaveSection();
    });
  }
  if (!/^element[.](claim|release)$/.test(entry.cmd)) sectionChanged();
}

/** The ribbon: Section and Elevation on the View tab, and what a selected one can do. */
function ribbon(config) {
  const create = config.tabs?.find((tab) => tab.id === 'view')?.panels.find((panel) => panel.label === 'Create');
  if (create) {
    create.items = create.items.map((item) =>
      item.label === 'Section' ? DT_('architecture', 'section') : item.label === 'Elevation' ? DT_('architecture', 'elevation') : item,
    );
  }
  const id = viewing() ? V.open.view : S.sel;
  const view = lineOf(id);
  if (view && S.screen === 'plan' && canEdit()) {
    const label = view.kind === 'section' ? 'Sections' : 'Elevations';
    config.contextual?.push({
      id: 'sel',
      key: `sel:${id}`,
      group: 'Modify',
      label,
      panels: [
        {
          label,
          items: [
            { label: 'Open', icon: 'section', act: 'stacksecopen', a: id, disabled: viewing() },
            { label: 'Flip', icon: 'mirror', act: 'stacksecflip', a: id },
            { label: 'Delete', icon: 'del', act: 'stackdelete', kbd: 'Delete' },
          ],
        },
        { label: 'Selection', items: [{ label: 'Deselect', icon: 'x', act: 'deselect' }] },
      ],
    });
  }
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.ribbons({ id: 'section-views', order: 1020, ribbon: (config) => ribbon(config) });
register.hints({ id: 'section-view', order: 5, hint: () => (viewing() ? '<b>View:</b> the server draws this from the model: cut regions filled by material, outlines beyond. Pick a plan in the browser to go back.' : undefined) });
register.palettes({ id: 'section-view', order: 5, palette: () => palette() });
register.selections({ id: 'section-view', order: 30, selection: (id) => (lineOf(id) ? palette() : undefined) });
register.layers({
  id: 'section-view',
  order: 40,
  under: () => drawnSection(),
  over: (scale) => marksSvg(scale),
  note: () => sectionNote(),
  replaces: () => viewing(),
});
register.models({ id: 'section-view', order: 50, open: () => queueDraw(), forget: () => { V.open = null; V.last = null; } });
register.logs({ id: 'section-view', order: 20, entry: (entry) => logged(entry) });

export { lineOf, lined, mm, viewing };
