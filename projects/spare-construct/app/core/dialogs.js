// The dialogs: the frame, and the Types and Project parameters dialogs.

import { late } from './late.js';
import { registered } from './registry.js';
import { C, api, modelUrl, say } from './state.js';
import { FROM, worked } from './parameters.js';
import { canEdit, capital, manage, newId, plural, problem, topLevel } from './problems.js';

// --- The dialogs ---

function selectedCategory() {
  return C.params?.view.element === S.sel ? (C.params.view.category?.id ?? null) : null;
}

function openManage(which) {
  if (!C.model) {
    say('Open a model in your team’s WIP first.');
    return;
  }
  const category = which === 'types' ? (selectedCategory() ?? C.manage?.category ?? null) : null;
  C.manage = { which, category, open: new Set(), data: null, error: null, shown: false, seq: 0 };
  if (which === 'graphics') Object.assign(C.manage, { view: C.openView, edge: 'cut' });
  if (which === 'crop') Object.assign(C.manage, { view: C.openView });
  const title = { types: 'Types', parameters: 'Project parameters', levels: 'Levels', families: 'Families', graphics: 'View graphics', crop: 'Crop and range' }[which];
  openDlg(title, `<div id="stack-manage" class="stack-manage" data-which="${which}"><p class="small faint">Loading.</p></div>`, '<button class="btn" data-act="dgclose">Close</button>');
  loadManage();
}
ACT.stacktypes = () => openManage('types');
ACT.stackparams = () => openManage('parameters');
ACT.stacklevels = () => openManage('levels');
ACT.stackfamilies = () => openManage('families');
ACT.stackviewgraphics = () => {
  if (C.openView) openManage('graphics');
};
ACT.stackviewcrop = () => {
  if (C.openView) openManage('crop');
};

/** Reads what the open dialog shows from the server, then draws it. Does nothing when no dialog is open. */
async function loadManage() {
  const m = C.manage;
  if (!m || !C.model || !$('#stack-manage') || $('#dlg').hidden) return;
  const model = C.model.id;
  m.seq += 1;
  const seq = m.seq;
  try {
    if (m.which !== 'families' && C.catalog?.model !== model) C.catalog = { model, ...(await api(`${modelUrl(model)}/catalog`)) };
    if (m.which === 'families') {
      m.data = { families: await api(`${modelUrl(model)}/families`) };
    } else if (m.which === 'types') {
      if (m.category === null) m.category = topLevel(C.catalog)[0]?.id ?? null;
      m.data = await api(`${modelUrl(model)}/types${m.category === null ? '' : `?category=${m.category}`}`);
    } else if (m.which === 'graphics') {
      m.data = await api(`${modelUrl(model)}/views/${encodeURIComponent(m.view)}/graphics`);
      if (m.category === null) m.category = m.data.looks[0]?.category ?? null;
    } else if (m.which === 'crop') {
      m.data = {}; // read from the list of views, which this page keeps live
      late.modelLevels();
    } else if (m.which === 'levels') {
      m.data = { levels: await api(`${modelUrl(model)}/levels`) };
      late.knowLevels(`${model}/${C.paramsEpoch}`, m.data.levels);
    } else {
      m.data = await api(`${modelUrl(model)}/project-parameters`);
    }
    m.error = null;
  } catch (error) {
    m.error = error.message;
  }
  if (C.manage === m && m.seq === seq) renderManage();
}

function renderManage() {
  const m = C.manage;
  const root = $('#stack-manage');
  if (!m || !root || $('#dlg').hidden || !(m.data || m.error)) return;
  if (m.which === 'crop' && C.cropHold > 0) return;
  let html;
  if (m.error) html = `<div class="note bad small" role="alert">${esc(m.error)}</div>`;
  else if (registered('dialogs', m.which)) html = registered('dialogs', m.which).html(m); // a tool's dialog (register.dialogs)
  else html = m.which === 'types' ? typesHtml(m) : parametersHtml(m);
  redraw(root, html);
  if (!m.shown) {
    m.shown = true;
    $('select, input', root)?.focus();
  }
}

/** Replaces what a container shows, keeping what is typed in it, where the cursor is and how far it is scrolled. */
function redraw(root, html) {
  const active = document.activeElement;
  const key = root.contains(active) ? active.dataset.key : null;
  const cursor = key && typeof active.selectionStart === 'number' ? [active.selectionStart, active.selectionEnd] : null;
  const drafts = new Map($$('[data-draft]', root).map((input) => [input.dataset.key, input.type === 'checkbox' ? input.checked : input.value]));
  const scroller = root.closest('.dlg-b');
  const scrolled = scroller?.scrollTop;
  root.innerHTML = html;
  for (const input of $$('[data-draft]', root)) {
    if (!drafts.has(input.dataset.key)) continue;
    if (input.type === 'checkbox') input.checked = drafts.get(input.dataset.key);
    else input.value = drafts.get(input.dataset.key);
  }
  if (scroller) scroller.scrollTop = scrolled;
  if (key) {
    const again = $$('[data-key]', root).find((el) => el.dataset.key === key);
    if (again) {
      again.focus();
      if (cursor && typeof again.setSelectionRange === 'function') again.setSelectionRange(...cursor);
    }
  }
}

function typesHtml(m) {
  const editable = canEdit();
  const off = editable ? '' : ' disabled';
  const options = topLevel(C.catalog).map((c) => `<option value="${c.id}"${c.id === m.category ? ' selected' : ''}>${esc(c.name)}</option>`);
  let html = `<label class="fld"><span>Category</span><select class="inp" data-chg="stacktypecat" data-key="type-category">${options.join('')}</select></label>`;
  html += '<p class="small muted">A type holds the values that every element of that type shares. Change a value on a type and every element of it changes.</p>';
  const name = esc(m.data.category?.name || 'This category');
  if (!m.data.types.length) html += `<p class="small faint">${name} has no types yet.</p>`;
  for (const type of m.data.types) {
    const label = esc(type.name);
    html += `<section data-type="${esc(type.id)}"><div class="stack-line">`;
    html += `<input class="inp" data-chg="stacktyperename" data-type="${esc(type.id)}" data-key="type-name:${esc(type.id)}" value="${label}" aria-label="Name of the type ${label}"${off}>`;
    html += `<span class="small faint">${plural(type.elements, 'element')}</span>`;
    html += `<button class="btn sm" data-act="stacktypecopy" data-a="${esc(type.id)}" data-key="type-copy:${esc(type.id)}" aria-label="Copy the type ${label}"${off}>Copy</button>`;
    html += `<button class="btn sm" data-act="stacktypedelete" data-a="${esc(type.id)}" data-key="type-delete:${esc(type.id)}" aria-label="Delete the type ${label}"${off}>Delete</button></div>`;
    if (type.parameters.length) {
      html += '<table class="props">';
      for (const p of type.parameters) {
        const settable = p.holder === 'type' && editable;
        const control = p.holder !== 'type'
          ? 'Each element has its own'
          : p.formula
            ? worked(p)
            : settable
            ? `<input class="inp" data-chg="stacktypeset" data-type="${esc(type.id)}" data-a="${esc(p.id)}" data-key="type-value:${esc(type.id)}:${esc(p.id)}" value="${esc(p.shown ?? '')}" aria-label="${label}: ${esc(p.name)}">`
            : esc(p.shown ?? '');
        html += prow(esc(p.name), control, FROM[p.from] || '');
      }
      html += '</table>';
    } else {
      html += '<p class="small faint">No parameters are bound to this category yet. Project parameters adds them.</p>';
    }
    html += `${problem(`type:${type.id}`)}</section>`;
  }
  html += '<h3 class="mt2">New type</h3>';
  html += `<div class="stack-line"><input class="inp" data-draft data-enter="stacknewtype" data-key="new-type-name" placeholder="Name" aria-label="Name for a new type of ${name}"${off}><button class="btn pri sm" data-act="stacknewtype" data-key="new-type-make"${off}>Make type</button></div>`;
  html += problem('new-type');
  return html;
}

CHG.stacktypecat = (select) => {
  C.manage.category = Number(select.value);
  C.manage.data = null;
  loadManage();
};
CHG.stacktyperename = (input) => manage('type.rename', { type: input.dataset.type, name: input.value }, `type:${input.dataset.type}`);
CHG.stacktypeset = (input) => manage('type.set', { type: input.dataset.type, parameter: input.dataset.a, value: input.value }, `type:${input.dataset.type}`);
ACT.stacktypecopy = (button) => {
  const type = C.manage?.data?.types.find((t) => t.id === button.dataset.a);
  if (type) manage('type.duplicate', { type: type.id, new: newId('T'), name: `${type.name} copy` }, `type:${type.id}`);
};
ACT.stacktypedelete = (button) => manage('type.delete', { type: button.dataset.a }, `type:${button.dataset.a}`);
ACT.stacknewtype = () => {
  const input = $('#stack-manage [data-key="new-type-name"]');
  manage('type.create', { type: newId('T'), name: input.value, category: C.manage.category }, 'new-type', () => {
    const again = $('#stack-manage [data-key="new-type-name"]');
    if (again) again.value = '';
  });
};

function parametersHtml(m) {
  const editable = canEdit();
  const off = editable ? '' : ' disabled';
  const categories = topLevel(C.catalog);
  const holderWords = { instance: 'Each element holds its own value', type: 'The type holds one value for all its elements' };
  let html = '<p class="small muted">A parameter is a named value. It belongs to the categories you bind it to, and every element of those categories has it.</p>';
  if (!m.data.parameters.length) html += '<p class="small faint">No parameters are defined yet.</p>';
  for (const p of m.data.parameters) {
    const bound = new Set(p.categories.map((c) => c.id));
    html += `<section data-param="${esc(p.id)}"><div class="stack-line"><b>${esc(p.name)}</b><span class="badge n">${esc(capital(p.kind_label))}</span><span class="small faint">${esc(holderWords[p.holder])}</span></div>`;
    const where = p.categories.length ? `Bound to ${esc(p.categories.map((c) => c.name).join(', '))}.` : 'Not bound to any category yet.';
    const standard = p.default === null ? '' : ` Default: ${esc(p.default)}.`;
    const choices = p.choices.length ? ` Choices: ${esc(p.choices.map((c) => c.label).join(', '))}.` : '';
    html += `<p class="small">${where}${standard}${choices}</p>`;
    if (p.formula) html += `<p class="small" data-formula="${esc(p.id)}">Formula: ${esc(p.formula)}</p>`;
    if (C.catalog.kinds.find((k) => k.tag === p.kind)?.takes_formula) {
      html += `<label class="fld"><span>Formula of other parameters, such as Width - 2 * Frame (empty: a person sets the value)</span><input class="inp" data-draft data-enter="stackformula" data-a="${esc(p.id)}" data-key="formula:${esc(p.id)}" value="${esc(p.formula ?? '')}" aria-label="Formula for ${esc(p.name)}"${off}></label>`;
      html += problem(`formula:${p.id}`);
    }
    html += `<details data-open="${esc(p.id)}"${m.open.has(p.id) ? ' open' : ''}><summary>Bind to categories</summary><div class="stack-checks">`;
    for (const c of categories) {
      const on = bound.has(c.id);
      html += `<label class="chk"><input type="checkbox" data-draft data-key="bind:${esc(p.id)}:${c.id}" value="${c.id}"${on ? ' checked disabled' : off}>${esc(c.name)}${on ? ' <span class="small faint">(bound)</span>' : ''}</label>`;
    }
    html += '</div>';
    html += `<label class="fld mt"><span>Default for the categories you choose (optional)</span><input class="inp" data-draft data-enter="stackbind" data-a="${esc(p.id)}" data-key="bind-default:${esc(p.id)}" aria-label="Default for ${esc(p.name)}"${off}></label>`;
    html += `<button class="btn sm" data-act="stackbind" data-a="${esc(p.id)}" data-key="bind-go:${esc(p.id)}"${off}>Bind</button>${problem(`bind:${p.id}`)}</details></section>`;
  }
  html += '<h3 class="mt2">Define a parameter</h3>';
  const kinds = C.catalog.kinds.map((k) => `<option value="${esc(k.tag)}">${esc(capital(k.label))}</option>`);
  const holders = Object.entries(holderWords).map(([value, label]) => `<option value="${value}">${esc(label)}</option>`);
  html += '<div class="grid2">';
  html += `<label class="fld"><span>Name</span><input class="inp" data-draft data-enter="stackdefine" data-key="def-name"${off}></label>`;
  html += `<label class="fld"><span>Kind of value</span><select class="inp" data-draft data-key="def-kind"${off}>${kinds.join('')}</select></label>`;
  html += `<label class="fld"><span>Who holds the value</span><select class="inp" data-draft data-key="def-holder"${off}>${holders.join('')}</select></label>`;
  html += `<label class="fld"><span>Choices, for a choice only, separated by commas</span><input class="inp" data-draft data-key="def-choices"${off}></label>`;
  html += `<label class="fld"><span>Default (optional)</span><input class="inp" data-draft data-enter="stackdefine" data-key="def-default"${off}></label>`;
  html += `<label class="fld"><span>Formula of other parameters (optional), such as Width - 2 * Frame</span><input class="inp" data-draft data-enter="stackdefine" data-key="def-formula"${off}></label>`;
  html += '</div>';
  html += '<div class="lbl">Bind it to (optional)</div><div class="stack-checks">';
  for (const c of categories) {
    html += `<label class="chk"><input type="checkbox" data-draft data-key="def-cat:${c.id}" value="${c.id}"${off}>${esc(c.name)}</label>`;
  }
  html += `</div><div class="stack-line mt"><button class="btn pri sm" data-act="stackdefine" data-key="def-go"${off}>Define parameter</button></div>${problem('define')}`;
  return html;
}

document.addEventListener(
  'toggle',
  (event) => {
    const open = event.target.dataset?.open;
    if (open && C.manage) {
      if (event.target.open) C.manage.open.add(open);
      else C.manage.open.delete(open);
    }
  },
  true,
);

// Enter in a field does what its button does.
document.addEventListener('keydown', (event) => {
  const act = event.key === 'Enter' && event.target.dataset?.enter;
  if (act && ACT[act]) {
    event.preventDefault();
    ACT[act](event.target);
  }
});

const manageField = (key) => $(`#stack-manage [data-key="${key}"]`);
const chosen = (root) => $$('input[type="checkbox"]:checked:not(:disabled)', root).map((box) => Number(box.value));

ACT.stackbind = (source) => {
  const id = source.dataset.a;
  const section = $(`#stack-manage section[data-param="${id}"]`);
  const text = manageField(`bind-default:${id}`).value;
  manage('parameter.bind', { parameter: id, categories: chosen(section), default: text === '' ? null : text }, `bind:${id}`);
};

// A formula goes to the server as typed: the server checks it and stores it in its one spelling.
ACT.stackformula = (input) => {
  const text = input.value.trim();
  manage('parameter.change', { parameter: input.dataset.a, formula: text === '' ? null : text }, `formula:${input.dataset.a}`);
};

ACT.stackdefine = () => {
  const id = newId('P');
  const labels = manageField('def-choices').value.split(',').map((text) => text.trim()).filter(Boolean);
  const definition = {
    id,
    name: manageField('def-name').value,
    kind: manageField('def-kind').value,
    holder: manageField('def-holder').value,
    choices: labels.map((label) => ({ key: label, label })),
  };
  const formula = manageField('def-formula').value.trim();
  if (formula !== '') definition.formula = formula;
  const categories = $$('#stack-manage [data-key^="def-cat:"]:checked').map((box) => Number(box.value));
  const text = manageField('def-default').value;
  const clear = () => {
    for (const el of $$('#stack-manage [data-key^="def-"]')) {
      if (el.type === 'checkbox') el.checked = false;
      else if (el.tagName === 'INPUT') el.value = '';
    }
  };
  manage('parameter.define', { definition }, 'define', () => {
    if (!categories.length) {
      clear();
      return;
    }
    manage('parameter.bind', { parameter: id, categories, default: text === '' ? null : text }, 'define', clear);
  });
};

export { loadManage, manageField, redraw, renderManage };
