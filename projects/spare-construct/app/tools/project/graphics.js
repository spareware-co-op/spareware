// Tool: a view's graphics rules.

import { late } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C } from '../../core/state.js';
import { canEdit, capital, manage, problem } from '../../core/problems.js';
import { renderManage } from '../../core/dialogs.js';

// --- A view's graphics rules ---
// How a category looks in the open view, for what the view cuts and for what it sees beyond the
// cut plane (construct-views' FORMAT.md, "Graphics rules"). The dialog is reached from the open
// view's line in the browser. The server lists every category with how it looks in the view now,
// where that came from (this view or the project), the pens and patterns a rule may name, and
// the rules the view holds (`/views/<view>/graphics`); a control sends only the part it holds,
// as `view.graphics.set`, and the server keeps the rest; Clear is `view.graphics.clear`. A
// refusal is the server's words beside the controls. Nothing is checked here, and the drawing
// is the server's: it follows the log in every browser.

const hexOf = (rgb) => `#${rgb.map((part) => Number(part).toString(16).padStart(2, '0')).join('')}`;
const rgbOf = (hex) => [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16));
const ORIGIN_WORDS = {
  subcategory: 'This view has a rule for this subcategory.',
  category: 'This view has a rule for its category.',
  project: 'No rule in this view: as the project’s styles say.',
};

function graphicsHtml(m) {
  const d = m.data;
  const off = canEdit() ? '' : ' disabled';
  const here = d.looks.find((l) => l.category === m.category) ?? d.looks[0];
  if (!here) return '<p class="small faint">Nothing to set.</p>';
  if (m.edge === 'cut' && !here.cut) m.edge = 'projection';
  const look = m.edge === 'cut' ? here.cut : here.projection;
  const style = look.style;
  const mm = (um) => `${um / 1000} mm`;
  const pens = (here.annotation ? d.annotation_pens : d.pens).map((pen) => `<option value="${pen.number}"${pen.number === style.pen ? ' selected' : ''}>${esc(`${pen.number} ${pen.name}, ${mm(pen.width)}`)}</option>`);
  const lines = d.line_patterns.map((p) => `<option value="${p.id}"${p.id === style.pattern ? ' selected' : ''}>${esc(p.name)}</option>`);
  const fills = ['<option value="">The material’s own</option>', ...d.fill_patterns.map((name) => `<option value="${esc(name)}"${style.fill?.pattern === name ? ' selected' : ''}>${esc(capital(name))}</option>`)];
  const edge = (which, label) => `<button class="btn sm" data-act="stackgfxedge" data-a="${which}" data-key="gfx-edge:${which}" aria-pressed="${m.edge === which}"${which === 'cut' && !here.cut ? ' disabled' : ''}>${label}</button>`;
  const options = d.looks.map((l) => `<option value="${l.category}"${l.category === here.category ? ' selected' : ''}>${l.parent === null ? '' : '– '}${esc(l.name)}</option>`);
  const ruled = d.rules.find((r) => r.category === here.category)?.[m.edge];
  let html = '<p class="small muted">How a category looks in this view. A rule here changes the drawing of this view only; what it does not name stays as it looks now.</p>';
  html += `<label class="fld"><span>Category</span><select class="inp" data-chg="stackgfxcat" data-key="gfx-category">${options.join('')}</select></label>`;
  html += `<div class="stack-line">${edge('cut', 'Cut')}${edge('projection', 'Seen beyond the cut')}</div>`;
  html += `<div class="small faint" data-gfx-origin="${esc(look.origin)}">${esc(ORIGIN_WORDS[look.origin])}</div>`;
  html += '<table class="props">';
  html += prow('Category drawn', `<input type="checkbox" data-chg="stackgfxdrawn" data-key="gfx-drawn"${d.hidden.includes(here.category) ? '' : ' checked'}${off} aria-label="Category drawn">`, '');
  html += prow('Shown', `<input type="checkbox" data-chg="stackgfxshown" data-key="gfx-shown"${style.shown ? ' checked' : ''}${off} aria-label="Shown">`, '');
  html += prow('Pen', `<select class="inp" data-chg="stackgfxpen" data-key="gfx-pen"${off} aria-label="Pen">${pens.join('')}</select>`, '');
  html += prow('Line pattern', `<select class="inp" data-chg="stackgfxpattern" data-key="gfx-pattern"${off} aria-label="Line pattern">${lines.join('')}</select>`, '');
  html += prow('Line colour', `<input type="color" class="inp" data-chg="stackgfxcolour" data-key="gfx-colour" value="${hexOf(style.colour)}"${off} aria-label="Line colour">`, '');
  if (m.edge === 'cut') {
    html += prow('Fill', `<select class="inp" data-chg="stackgfxfill" data-key="gfx-fill"${off} aria-label="Fill pattern">${fills.join('')}</select>`, '');
    html += prow('Fill colour', `<input type="color" class="inp" data-chg="stackgfxfillcolour" data-key="gfx-fill-colour" value="${hexOf(style.fill?.colour ?? [0, 0, 0])}"${off || (style.fill ? '' : ' disabled')} aria-label="Fill colour">`, '');
  }
  html += prow('Halftone', `<input type="checkbox" data-chg="stackgfxhalftone" data-key="gfx-halftone"${style.halftone ? ' checked' : ''}${off} aria-label="Halftone">`, '');
  html += prow('Transparency', `<input class="inp" type="number" data-chg="stackgfxtransparency" data-key="gfx-transparency" value="${style.transparency}"${off} aria-label="Transparency, per cent">`, 'per cent');
  html += '</table>';
  html += `<div class="stack-line"><button class="btn sm" data-act="stackgfxclear" data-a="${m.edge}" data-key="gfx-clear"${ruled ? '' : ' disabled'}${off}>Clear this rule</button></div>`;
  html += problem(`graphics:${here.category}:${m.edge}`);
  html += '<h3 class="mt2">Rules in this view</h3>';
  if (!d.rules.length) html += '<p class="small faint" data-gfx-none>None: every category looks as the project says.</p>';
  for (const rule of d.rules) {
    const part = (label, st) => (st ? `${label}: pen ${st.pen}, ${hexOf(st.colour)}${st.shown ? '' : ', hidden'}${st.halftone ? ', halftone' : ''}${st.transparency ? `, ${st.transparency}% transparent` : ''}${st.fill ? `, ${st.fill.pattern} fill ${hexOf(st.fill.colour)}` : ''}` : '');
    const text = [part('cut', rule.cut), part('seen', rule.projection)].filter(Boolean).join('; ');
    html += `<div class="stack-line" data-gfx-rule="${rule.category}"><span>${esc(rule.name)}</span><span class="small faint">${esc(text)}</span>`;
    html += `<button class="btn sm" data-act="stackgfxpick" data-a="${rule.category}" data-key="gfx-pick:${rule.category}">Edit</button>`;
    html += `<button class="btn sm" data-act="stackgfxclearall" data-a="${rule.category}" data-key="gfx-clear-all:${rule.category}"${off}>Clear</button></div>`;
  }
  html += problem('graphics');
  return html;
}

/** Sends one part of the rule the dialog shows; the server keeps the parts not named. */
function graphicsSet(part) {
  const m = C.manage;
  manage('view.graphics.set', { ...late.ownerArgs(m), category: m.category, edge: m.edge, style: part }, `graphics:${m.category}:${m.edge}`);
}
CHG.stackgfxdrawn = (box) => manage('view.categories.set', { ...late.ownerArgs(C.manage), category: C.manage.category, shown: box.checked }, `graphics:${C.manage.category}:${C.manage.edge}`);
const graphicsStyle = () => {
  const m = C.manage;
  const look = m.data.looks.find((l) => l.category === m.category);
  return (m.edge === 'cut' ? look.cut : look.projection).style;
};
CHG.stackgfxcat = (select) => {
  C.manage.category = Number(select.value);
  renderManage();
};
ACT.stackgfxedge = (button) => {
  C.manage.edge = button.dataset.a;
  renderManage();
};
ACT.stackgfxpick = (button) => {
  C.manage.category = Number(button.dataset.a);
  renderManage();
};
CHG.stackgfxshown = (box) => graphicsSet({ shown: box.checked });
CHG.stackgfxpen = (select) => graphicsSet({ pen: Number(select.value) });
CHG.stackgfxpattern = (select) => graphicsSet({ pattern: Number(select.value) });
CHG.stackgfxcolour = (input) => graphicsSet({ colour: rgbOf(input.value) });
CHG.stackgfxfill = (select) => graphicsSet({ fill: select.value ? { pattern: select.value, colour: graphicsStyle().fill?.colour ?? [0, 0, 0] } : null });
CHG.stackgfxfillcolour = (input) => graphicsSet({ fill: { pattern: graphicsStyle().fill?.pattern, colour: rgbOf(input.value) } });
CHG.stackgfxhalftone = (box) => graphicsSet({ halftone: box.checked });
CHG.stackgfxtransparency = (input) => graphicsSet({ transparency: input.value === '' ? null : Number(input.value) });
ACT.stackgfxclear = (button) => {
  const m = C.manage;
  manage('view.graphics.clear', { ...late.ownerArgs(m), category: m.category, edge: button.dataset.a }, `graphics:${m.category}:${button.dataset.a}`);
};
ACT.stackgfxclearall = (button) => manage('view.graphics.clear', { ...late.ownerArgs(C.manage), category: Number(button.dataset.a) }, 'graphics');

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.dialogs({ id: 'graphics', order: 30, html: (m) => graphicsHtml(m) });

export { hexOf, rgbOf };
