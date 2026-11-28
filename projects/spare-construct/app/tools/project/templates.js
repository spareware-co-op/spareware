// Tool: view templates, overrides and filters.

import { provide } from '../../core/late.js';
import { C } from '../../core/state.js';
import { canEdit, capital, manage, newId, plural, problem, topLevel } from '../../core/problems.js';
import { manageField, openManage, renderManage } from '../../core/dialogs.js';
import { hexOf, rgbOf } from './graphics.js';

// --- View templates, overrides and filters ---
// A view template is a saved set of view settings (construct-views' FORMAT.md, "View templates"):
// scale, detail level, graphics rules, which categories show and filters, each with an "included"
// flag. A view tied to a template (`view.template.tie`) follows its included settings, and the
// server turns back a change the view makes to one of them, in words, beside the control that
// asked; a template applied once (`view.template.apply`) copies them. An override
// (`view.override.set`) is one element's look in the open view; a filter (`view.filter.add`) is
// a rule on a category and one of its parameters, with its own look, the first in the list
// winning. The page lists what the server reads (`/view-templates`, `/views/<view>/overrides`,
// `/views/<view>/filters`, `/view-templates/<template>/filters`) and sends only what a control
// holds; nothing is decided here, and the drawing is the server's, so every browser redraws
// when the log says a template, an override or a filter changed.

/** The path of what a dialog is about: the open view, or a template. */
const ownerPath = (m) => (m.template ? `view-templates/${encodeURIComponent(m.template)}` : `views/${encodeURIComponent(m.view)}`);
/** The argument that says what a settings command is about. */
const ownerArgs = (m) => (m.template ? { template: m.template } : { view: m.view });
const templateName = (id) => C.templates?.find((t) => t.id === id)?.name ?? id;
const SETTING_WORDS = { scale: 'Scale', detail: 'Detail level', graphics: 'Graphics rules', categories: 'Categories shown', filters: 'Filters' };
const OPERATOR_WORDS = { equals: 'equals', contains: 'contains', greater_than: 'is greater than' };

ACT.stackviewtemplates = () => {
  if (C.openView) openManage('templates');
};
ACT.stackviewfilters = () => {
  if (C.openView) openManage('filters');
};
ACT.stackviewoverrides = () => {
  if (C.openView) openManage('overrides');
};

const nameField = (key) => manageField(key)?.value ?? '';

function templatesHtml(m) {
  const off = canEdit() ? '' : ' disabled';
  const view = C.views?.find((v) => v.id === C.openView);
  C.templates = m.data.templates;
  let html = '<p class="small muted">A view template is a saved set of view settings. A view tied to one follows what it includes and cannot change that by itself; applying one once copies its settings and lets the view go.</p>';
  if (view) {
    html += `<div class="small" data-vt-open="${esc(view.id)}">Open view: ${esc(view.name)}. ${view.template ? `It follows ${esc(templateName(view.template))}.` : 'It follows no template.'}</div>`;
    if (view.template) html += `<div class="stack-line"><button class="btn sm" data-act="stacktpluntie" data-key="tpl-untie"${off}>Untie this view</button></div>`;
  }
  html += problem('template-view');
  html += `<div class="stack-line"><input class="inp" data-draft data-enter="stackmaketemplate" data-key="new-template-name" placeholder="Name of a new template" aria-label="Name for a new view template"${off}>`;
  html += `<button class="btn pri sm" data-act="stackmaketemplate" data-key="new-template-make"${view ? off : ' disabled'}>Make from this view</button></div>`;
  html += problem('new-template');
  if (!m.data.templates.length) html += '<p class="small faint" data-tpl-none>No view templates yet.</p>';
  for (const t of m.data.templates) {
    const id = esc(t.id);
    const tied = view?.template === t.id;
    html += `<section class="stack-tpl" data-template="${id}"><h3>${esc(t.name)}</h3>`;
    html += `<div class="stack-line"><input class="inp" data-chg="stacktplrename" data-a="${id}" data-key="tpl-name:${id}" value="${esc(t.name)}" aria-label="Name of ${esc(t.name)}"${off}>`;
    html += `<button class="btn sm" data-act="stacktpldelete" data-a="${id}" data-key="tpl-delete:${id}" aria-label="Delete ${esc(t.name)}"${off}>Delete</button></div>`;
    html += `<div class="stack-line"><label class="small">Scale 1:<input class="inp" type="number" data-chg="stacktplscale" data-a="${id}" data-key="tpl-scale:${id}" value="${t.scale}" aria-label="Scale of ${esc(t.name)}"${off}></label>`;
    html += `<select class="inp" data-chg="stacktpldetail" data-a="${id}" data-key="tpl-detail:${id}" aria-label="Detail level of ${esc(t.name)}"${off}>${['coarse', 'medium', 'fine'].map((d) => `<option value="${d}"${d === t.detail ? ' selected' : ''}>${capital(d)}</option>`).join('')}</select></div>`;
    html += '<div class="stack-line">';
    for (const [key, words] of Object.entries(SETTING_WORDS)) {
      html += `<label class="small"><input type="checkbox" data-chg="stacktplincluded" data-a="${id}" data-b="${key}" data-key="tpl-inc:${id}:${key}"${t.included.includes(key) ? ' checked' : ''}${off} aria-label="${esc(words)} included in ${esc(t.name)}"> ${esc(words)}</label>`;
    }
    html += '</div>';
    html += `<div class="small faint" data-tpl-summary="${id}">${plural(t.rules, 'graphics rule')}, ${t.hidden.length} hidden ${t.hidden.length === 1 ? 'category' : 'categories'}, ${plural(t.filters, 'filter')}; ${plural(t.views.length, 'view')} follow${t.views.length === 1 ? 's' : ''} it.</div>`;
    html += '<div class="stack-line">';
    const here = view ? off : ' disabled';
    html += tied
      ? `<span class="small" data-tpl-tied="${id}">This view follows it.</span>`
      : `<button class="btn sm" data-act="stacktpltie" data-a="${id}" data-key="tpl-tie:${id}"${here}>Tie this view</button><button class="btn sm" data-act="stacktplapply" data-a="${id}" data-key="tpl-apply:${id}"${here}>Apply once to this view</button>`;
    html += `<button class="btn sm" data-act="stacktplupdate" data-a="${id}" data-key="tpl-update:${id}"${here}>Take this view’s settings</button>`;
    html += `<button class="btn sm" data-act="stacktplgraphics" data-a="${id}" data-key="tpl-graphics:${id}">Graphics</button>`;
    html += `<button class="btn sm" data-act="stacktplfilters" data-a="${id}" data-key="tpl-filters:${id}">Filters</button></div>`;
    html += `${problem(`template:${t.id}`)}</section>`;
  }
  return html + problem('templates');
}

ACT.stackmaketemplate = () => {
  const name = manageField('new-template-name');
  if (!name || !C.openView) return;
  manage('view.template.create', { template: newId('VT'), name: name.value, source: C.openView }, 'new-template', () => {
    const again = manageField('new-template-name');
    if (again) again.value = '';
  });
};
ACT.stacktpluntie = () => manage('view.template.untie', { view: C.openView }, 'template-view');
ACT.stacktpltie = (button) => manage('view.template.tie', { view: C.openView, template: button.dataset.a }, `template:${button.dataset.a}`);
ACT.stacktplapply = (button) => manage('view.template.apply', { view: C.openView, template: button.dataset.a }, `template:${button.dataset.a}`);
ACT.stacktplupdate = (button) => manage('view.template.update', { template: button.dataset.a, source: C.openView }, `template:${button.dataset.a}`);
ACT.stacktpldelete = (button) => manage('view.template.delete', { template: button.dataset.a }, `template:${button.dataset.a}`);
ACT.stacktplgraphics = (button) => openManage('graphics', { template: button.dataset.a, view: null });
ACT.stacktplfilters = (button) => openManage('filters', { template: button.dataset.a, view: null });
CHG.stacktplrename = (input) => manage('view.template.rename', { template: input.dataset.a, name: input.value }, `template:${input.dataset.a}`);
CHG.stacktplscale = (input) => manage('view.template.set', { template: input.dataset.a, scale: Number(input.value) }, `template:${input.dataset.a}`);
CHG.stacktpldetail = (select) => manage('view.template.set', { template: select.dataset.a, detail: select.value }, `template:${select.dataset.a}`);
CHG.stacktplincluded = (box) => manage('view.template.set', { template: box.dataset.a, included: { [box.dataset.b]: box.checked } }, `template:${box.dataset.a}`);

/** What a filter or an override does to a look, in a few words. */
function lookWords(look) {
  const parts = [];
  if (look.pen !== undefined) parts.push(`pen ${look.pen}`);
  if (look.pattern !== undefined) parts.push(`pattern ${look.pattern}`);
  if (look.colour) parts.push(hexOf(look.colour));
  if (look.halftone !== undefined) parts.push(look.halftone ? 'halftone' : 'not halftone');
  if (look.shown !== undefined) parts.push(look.shown ? 'shown' : 'hidden');
  return parts.join(', ') || 'no change';
}

function filtersHtml(m) {
  const d = m.data;
  const locked = Boolean(d.follows_template);
  const off = canEdit() && !locked ? '' : ' disabled';
  let html = `<p class="small muted">${d.of === 'view' ? 'The filters of this view.' : 'The filters of this template.'} A filter gives the elements it matches a look of their own; the first filter in the list wins any part of the look it names.</p>`;
  if (locked) html += `<div class="note small" data-filters-follow>${esc(d.name)} follows the view template ${esc(d.follows_template)}, which includes filters: change them there.</div>`;
  if (!d.filters.length) html += '<p class="small faint" data-filters-none>No filters.</p>';
  d.filters.forEach((f, at) => {
    html += `<div class="stack-line" data-filter="${f.id}"><span class="grow"><b>${esc(f.name)}</b>: ${esc(f.category_name)}, ${esc(f.parameter_name)} ${esc(OPERATOR_WORDS[f.op] ?? f.op)} ${esc(f.value)}</span><span class="small faint">${esc(lookWords(f.look))}</span>`;
    html += `<button class="btn sm" data-act="stackfilterup" data-a="${f.id}" data-b="${at}" data-key="filter-up:${f.id}" aria-label="Move ${esc(f.name)} up"${at === 0 ? ' disabled' : off}>Up</button>`;
    html += `<button class="btn sm" data-act="stackfilterdown" data-a="${f.id}" data-b="${at}" data-key="filter-down:${f.id}" aria-label="Move ${esc(f.name)} down"${at === d.filters.length - 1 ? ' disabled' : off}>Down</button>`;
    html += `<button class="btn sm" data-act="stackfilteroff" data-a="${f.id}" data-key="filter-off:${f.id}" aria-label="Remove ${esc(f.name)}"${off}>Remove</button></div>`;
  });
  const tops = topLevel(C.catalog).filter((c) => d.parameters.some((p) => p.categories.includes(c.id)));
  if (!tops.length) return `${html}<p class="small faint" data-filters-no-parameters>No parameter is bound to a category yet, so there is nothing to filter by.</p>${problem('filters')}`;
  const category = tops.find((c) => c.id === m.form.category)?.id ?? tops[0].id;
  const parameters = d.parameters.filter((p) => p.categories.includes(category));
  const parameter = parameters.find((p) => p.id === m.form.parameter) ?? parameters[0];
  const op = parameter.operators.includes(m.form.op) ? m.form.op : parameter.operators[0];
  html += '<h3 class="mt2">New filter</h3>';
  html += `<div class="stack-line"><input class="inp" data-draft data-enter="stackfilteradd" data-key="filter-name" placeholder="Name" aria-label="Name of the new filter"${off}></div>`;
  html += `<div class="stack-line"><select class="inp" data-chg="stackfiltercat" data-key="filter-category" aria-label="Category of the new filter"${off}>${tops.map((c) => `<option value="${c.id}"${c.id === category ? ' selected' : ''}>${esc(c.name)}</option>`).join('')}</select>`;
  html += `<select class="inp" data-chg="stackfilterparam" data-key="filter-parameter" aria-label="Parameter to read"${off}>${parameters.map((p) => `<option value="${esc(p.id)}"${p.id === parameter.id ? ' selected' : ''}>${esc(p.name)}</option>`).join('')}</select>`;
  html += `<select class="inp" data-chg="stackfilterop" data-key="filter-op" aria-label="How to compare"${off}>${parameter.operators.map((key) => `<option value="${key}"${key === op ? ' selected' : ''}>${esc(OPERATOR_WORDS[key] ?? key)}</option>`).join('')}</select>`;
  html += `<input class="inp" data-draft data-enter="stackfilteradd" data-key="filter-value" placeholder="Value" aria-label="Value to compare with"${off}></div>`;
  html += `<div class="stack-line"><label class="small"><input type="checkbox" data-draft data-key="filter-use-colour" aria-label="Give matches a colour"${off}> Colour</label><input type="color" class="inp" data-draft data-key="filter-colour" value="#cc0000" aria-label="Colour of matches"${off}>`;
  html += `<label class="small"><input type="checkbox" data-draft data-key="filter-halftone" aria-label="Halftone for matches"${off}> Halftone</label>`;
  html += `<label class="small"><input type="checkbox" data-draft data-key="filter-hide" aria-label="Hide matches"${off}> Hide</label>`;
  html += `<button class="btn pri sm" data-act="stackfilteradd" data-key="filter-make"${off}>Add filter</button></div>`;
  return html + problem('filters');
}

CHG.stackfiltercat = (select) => {
  C.manage.form = { category: Number(select.value) };
  renderManage();
};
CHG.stackfilterparam = (select) => {
  C.manage.form = { ...C.manage.form, category: Number(manageField('filter-category').value), parameter: select.value, op: undefined };
  renderManage();
};
CHG.stackfilterop = (select) => {
  C.manage.form = { ...C.manage.form, category: Number(manageField('filter-category').value), parameter: manageField('filter-parameter').value, op: select.value };
};
ACT.stackfilteradd = () => {
  const m = C.manage;
  const look = {};
  if (manageField('filter-use-colour').checked) look.colour = rgbOf(manageField('filter-colour').value);
  if (manageField('filter-halftone').checked) look.halftone = true;
  if (manageField('filter-hide').checked) look.shown = false;
  const args = { ...ownerArgs(m), name: nameField('filter-name'), category: Number(nameField('filter-category')), parameter: nameField('filter-parameter'), op: nameField('filter-op'), value: nameField('filter-value'), appearance: look };
  manage('view.filter.add', args, 'filters', () => {
    for (const key of ['filter-name', 'filter-value']) {
      const again = manageField(key);
      if (again) again.value = '';
    }
  });
};
ACT.stackfilteroff = (button) => manage('view.filter.remove', { ...ownerArgs(C.manage), filter: Number(button.dataset.a) }, 'filters');
ACT.stackfilterup = (button) => manage('view.filter.move', { ...ownerArgs(C.manage), filter: Number(button.dataset.a), place: Number(button.dataset.b) - 1 }, 'filters');
ACT.stackfilterdown = (button) => manage('view.filter.move', { ...ownerArgs(C.manage), filter: Number(button.dataset.a), place: Number(button.dataset.b) + 1 }, 'filters');

function overridesHtml(m) {
  const d = m.data;
  const g = m.graphics;
  const off = canEdit() ? '' : ' disabled';
  const element = S.sel;
  let html = '<p class="small muted">An override changes how one element looks in this view only, over its category’s rule and its filters. It stays when the view follows a template.</p>';
  if (element) {
    html += `<div class="small" data-ov-element="${esc(element)}">Selected element: ${esc(element)}</div>`;
    const pens = ['<option value="">No change</option>', ...g.pens.map((pen) => `<option value="${pen.number}">${esc(`${pen.number} ${pen.name}, ${pen.width / 1000} mm`)}</option>`)];
    const patterns = ['<option value="">No change</option>', ...g.line_patterns.map((p) => `<option value="${p.id}">${esc(p.name)}</option>`)];
    html += `<div class="stack-line"><select class="inp" data-draft data-key="ov-pen" aria-label="Pen for the element"${off}>${pens.join('')}</select>`;
    html += `<select class="inp" data-draft data-key="ov-pattern" aria-label="Line pattern for the element"${off}>${patterns.join('')}</select></div>`;
    html += `<div class="stack-line"><label class="small"><input type="checkbox" data-draft data-key="ov-use-colour" aria-label="Give the element a line colour"${off}> Line colour</label><input type="color" class="inp" data-draft data-key="ov-colour" value="#cc0000" aria-label="Line colour of the element"${off}>`;
    html += `<label class="small"><input type="checkbox" data-draft data-key="ov-halftone" aria-label="Halftone for the element"${off}> Halftone</label>`;
    html += `<label class="small"><input type="checkbox" data-draft data-key="ov-hide" aria-label="Hide the element in this view"${off}> Hide in this view</label>`;
    html += `<button class="btn pri sm" data-act="stackoverrideset" data-key="ov-make"${off}>Set override</button></div>`;
  } else {
    html += '<p class="small faint" data-ov-none-selected>Select an element in the plan, then open this again.</p>';
  }
  html += problem('overrides');
  html += '<h3 class="mt2">Overrides in this view</h3>';
  if (!d.overrides.length) html += '<p class="small faint" data-ov-none>None.</p>';
  for (const o of d.overrides) {
    html += `<div class="stack-line" data-override="${esc(o.element)}"><span class="grow">${esc(o.name ?? o.element)}</span><span class="small faint">${esc(lookWords(o.look))}</span>`;
    html += `<button class="btn sm" data-act="stackoverrideclear" data-a="${esc(o.element)}" data-key="ov-clear:${esc(o.element)}"${off}>Clear</button></div>`;
  }
  if (d.overrides.length) html += `<div class="stack-line"><button class="btn sm" data-act="stackoverrideclearall" data-key="ov-clear-all"${off}>Clear all</button></div>`;
  return html;
}

ACT.stackoverrideset = () => {
  const m = C.manage;
  if (!S.sel) return;
  const look = {};
  const pen = nameField('ov-pen');
  const pattern = nameField('ov-pattern');
  if (pen !== '') look.pen = Number(pen);
  if (pattern !== '') look.pattern = Number(pattern);
  if (manageField('ov-use-colour').checked) look.colour = rgbOf(manageField('ov-colour').value);
  if (manageField('ov-halftone').checked) look.halftone = true;
  if (manageField('ov-hide').checked) look.shown = false;
  manage('view.override.set', { view: m.view, element: S.sel, appearance: look }, 'overrides');
};
ACT.stackoverrideclear = (button) => manage('view.override.clear', { view: C.manage.view, element: button.dataset.a }, 'overrides');
ACT.stackoverrideclearall = () => manage('view.override.clear', { view: C.manage.view }, 'overrides');

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get ownerArgs() { return ownerArgs; }, get ownerPath() { return ownerPath; },
  get templateName() { return templateName; },
});
