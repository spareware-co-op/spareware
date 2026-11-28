// Tool: the section tool, a section line drawn in the plan.

import { register } from '../../core/registry.js';
import { C, say } from '../../core/state.js';
import { canEdit, manage, newId } from '../../core/problems.js';
import { queueDraw, snap, toModel } from '../../core/plan.js';
import { around } from '../../core/shapes.js';
import { lined, viewing } from './sectionview.js';

// --- The section tool ---
// A person picks the two ends of a section's line in the plan; the page sends `view.create` with the
// two ends, the side it looks to and how far it sees, and the server names the view's record and
// checks it. Its mark then appears in every plan (sectionview.js), where a second press on its head
// or tail opens it. The depth is typed in the tool's palette before drawing, in the text a person
// reads (`6 m`); left empty it reaches the far side of what the model holds, and a metre more.

DTOOLS.architecture.push(['section', 'Section', 'section', 'Pick the two ends of the section line. It looks to the left of the line; turn it round in its properties.']);
TOOLNAME.section = 'section';

/** What the tool is set to, and the pointer while its first end is picked. */
const T = { look: 'left', depth: '', preview: null };

/** The box round everything the open level holds, in the plan's x and y, or null when it holds nothing. */
function modelBox() {
  let box = null;
  for (const piece of C.elements.values()) {
    if (!piece.shape) continue;
    const { x, y, w, d } = around(piece);
    box = box
      ? [Math.min(box[0], x), Math.min(box[1], y), Math.max(box[2], x + w), Math.max(box[3], y + d)]
      : [x, y, x + w, y + d];
  }
  return box;
}

/** How far a line from `a` to `b` looking to `look` must see to reach the far side of the model, in whole millimetres. */
function reach(a, b, look) {
  const box = modelBox();
  if (!box) return 10000;
  const run = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
  const left = [-(b[1] - a[1]) / run, (b[0] - a[0]) / run];
  const side = look === 'right' ? [-left[0], -left[1]] : left;
  let far = 0;
  for (const x of [box[0], box[2]]) {
    for (const y of [box[1], box[3]]) far = Math.max(far, (x - a[0]) * side[0] + (y - a[1]) * side[1]);
  }
  return Math.max(3000, Math.ceil(far) + 1000);
}

/** The next free name: `Section 1`, `Section 2`... */
function nextName(word) {
  let top = 0;
  for (const view of C.views || []) {
    const found = new RegExp(`^${word} (\\d+)$`).exec(view.name);
    if (found) top = Math.max(top, Number(found[1]));
  }
  return `${word} ${top + 1}`;
}

function sectionHint() {
  return S.pend
    ? '<b>Section:</b> pick the other end of the line. Esc to stop.'
    : '<b>Section:</b> pick where its line starts. The tail is here; the head, with its arrow, is at the other end.';
}

function place(at) {
  if (!canEdit()) {
    say('Open a model in your team’s WIP to add to it.');
    return;
  }
  const point = [snap(at.x), snap(at.y)];
  if (!S.pend) {
    S.pend = point;
    T.preview = point;
    $('#hint').innerHTML = sectionHint();
    return;
  }
  const [a, b] = [S.pend, point];
  S.pend = null;
  T.preview = null;
  $('#hint').innerHTML = sectionHint();
  queueDraw();
  if (a[0] === b[0] && a[1] === b[1]) {
    say('A section needs a line: pick its second end somewhere else.');
    return;
  }
  const depth = T.depth.trim() || `${reach(a, b, T.look)} mm`;
  manage(
    'view.create',
    {
      view: newId('VS'),
      kind: 'section',
      name: nextName('Section'),
      look: T.look,
      depth,
      from: { x: `${a[0]} mm`, y: `${a[1]} mm` },
      to: { x: `${b[0]} mm`, y: `${b[1]} mm` },
    },
    'new-section',
  );
}

window.addEventListener('pointermove', (event) => {
  if (S.tool !== 'section' || !S.pend || !$('#pv-canvas svg')) return;
  const at = toModel(event);
  T.preview = [snap(at.x), snap(at.y)];
  queueDraw();
});

/** The tool's own options in the properties palette while it is in use. */
function options() {
  if (S.tool !== 'section' || viewing()) return undefined;
  const off = canEdit() ? '' : ' disabled';
  const side = (key, label) => `<option value="${key}"${T.look === key ? ' selected' : ''}>${label}</option>`;
  let html = typeSel('section', 'Section', '<b>New section</b>');
  html += `<table class="props">${prow('Looks to', `<select class="inp" data-chg="stacksecnewlook" data-key="section-new-look" aria-label="The side a new section looks to"${off}>${side('left', 'Left of its line')}${side('right', 'Right of its line')}</select>`)}`;
  html += prow('Far clip', `<input class="inp" data-chg="stacksecnewdepth" data-key="section-new-depth" value="${esc(T.depth)}" placeholder="the far side of the model" aria-label="How far a new section sees"${off}>`, 'How far it sees beyond its line.');
  html += '</table>';
  return `${html}<p class="pad small faint">${lined().length} section${lined().length === 1 ? '' : 's'} and elevation${lined().length === 1 ? '' : 's'} in this model.</p>`;
}

CHG.stacksecnewlook = (select) => {
  T.look = select.value === 'right' ? 'right' : 'left';
};
CHG.stacksecnewdepth = (input) => {
  T.depth = input.value;
};

/** The line being drawn, from its first end to the pointer. */
function sketch() {
  if (S.tool !== 'section' || !S.pend || !T.preview) return '';
  return L(S.pend, T.preview, 'ln ap dd');
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({
  id: 'section',
  discipline: 'architecture',
  order: 150,
  place,
  commands: ['view.create', 'view.set', 'view.rename', 'view.delete', 'view.line.move', 'view.line.flip'],
  test: 'Sections and elevations, in the plan and opened',
});
register.hints({ id: 'section', order: 150, hint: () => (S.tool === 'section' ? sectionHint() : undefined) });
register.palettes({ id: 'section-tool', order: 150, palette: () => options() });
register.layers({ id: 'section-sketch', order: 150, over: () => sketch() });
