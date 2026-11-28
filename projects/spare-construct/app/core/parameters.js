// The side panel host's own rows: an element's parameters and its type, in the properties palette.

import { late } from './late.js';
import { C, api, modelUrl } from './state.js';
import { edit } from './channel.js';
import { canEdit, manage, problem } from './problems.js';
import { redraw } from './dialogs.js';
import { propertiesPalette } from './plan.js';

// --- Parameters ---
// The selected element's category, type and parameters come from the server, which reads them
// the way it checks a change and builds a shape (construct-model's FORMAT.md, "Categories, types
// and parameter values"). A change is the `element.param` edit, through the same log as a move:
// the server parses the typed text with its units, and a refusal comes back as its reason.

const FROM = { type: 'From its type', default: 'Default', unset: 'Not set', formula: 'From its formula' };

/** A value its formula gives: shown, never typed, with the formula and, when it gives none, the server's reason. */
function worked(p) {
  const reason = p.reason ? `<div class="note bad small" data-reason="${esc(p.id)}">${esc(p.reason)}</div>` : '';
  return `${esc(p.shown ?? '')}<div class="f" data-formula="${esc(p.id)}">= ${esc(p.formula)}</div>${reason}`;
}
function hasShape(record) {
  // A schedule, a sheet, a viewport, a dimension and a dimension style are records with no shape
  // too: the plan draws none of them as an element (a dimension is drawn by its own block).
  return !['type', 'parameters', 'level', 'view', 'family.recipe', 'schedule', 'sheet', 'viewport', 'dimension', 'dimension.style'].includes(record.kind);
}
C.params = null; // { key, view } of the selected element
C.paramsEpoch = 0; // goes up when a type or a parameter changes
C.edit = edit; // the browser test defines parameters through the page's own edit path

function paramsFor(piece) {
  if (!C.model) return null;
  const key = `${piece.el.id}/${piece.hash}/${C.paramsEpoch}/${late.isRoom(piece.el) ? (piece.fill?.head ?? 0) : ''}`;
  if (C.params?.key !== key && C.paramsLoading !== key) {
    C.paramsLoading = key;
    api(`${modelUrl(C.model.id)}/parameters/${piece.el.id}`)
      .then((view) => {
        C.params = { key, view };
        refreshSide();
      })
      .catch(() => {});
  }
  return C.params?.view.element === piece.el.id ? C.params.view : null;
}

function parametersTable(view) {
  const editable = !C.readOnly && !C.viewing;
  let html = '<span class="lbl">Parameters</span><table class="props">';
  for (const p of view.parameters.filter((row) => row.from !== late.READ_ONLY)) {
    const value = p.shown ?? '';
    const field = p.formula
      ? worked(p)
      : p.holder === 'instance' && editable
        ? `<input class="inp" data-chg="stackparam" data-a="${esc(p.id)}" data-key="param:${esc(p.id)}" value="${esc(value)}" aria-label="${esc(p.name)}">`
        : esc(value);
    html += prow(esc(p.name), field + problem(`param:${p.id}`), FROM[p.from] || '');
  }
  return `${html}</table>`;
}

/** Draws the properties palette again, unless someone is typing in it. */
function refreshSide() {
  const typing = document.activeElement?.tagName === 'INPUT';
  for (const side of ['#pv-side', '#iso-side']) {
    const el = $(side);
    if (el && !(typing && el.contains(document.activeElement))) redraw(el, propertiesPalette());
  }
}

CHG.stackparam = (input) => {
  if (S.sel && C.model && !C.readOnly) {
    manage('element.param', { element: S.sel, parameter: input.dataset.a, value: input.value }, `param:${input.dataset.a}`);
    input.blur();
  }
};

ACT.stackdelete = () => {
  if (S.sel && !C.readOnly && C.elements.has(S.sel)) {
    const kind = C.elements.get(S.sel)?.el.kind;
    edit(kind === 'roof' ? 'roof.delete' : kind === 'curtain' ? 'curtain.delete' : 'element.delete', { element: S.sel });
    S.sel = null;
    refresh();
  }
};

// --- The element's type, in the properties palette ---

function typesOfCategory(category) {
  if (!C.model || !category) return null;
  const key = `${category.id}/${C.paramsEpoch}`;
  if (C.typeList?.key !== key && C.typeListLoading !== key) {
    C.typeListLoading = key;
    api(`${modelUrl(C.model.id)}/types?category=${category.id}`)
      .then((view) => {
        C.typeList = { key, view };
        refreshSide();
      })
      .catch(() => {});
  }
  return C.typeList?.view.category?.id === category.id ? C.typeList.view : null;
}

function typeCell(view) {
  const list = typesOfCategory(view.category);
  const none = esc(view.type?.name || 'None');
  if (!view.category || !canEdit() || !list || !list.types.length) {
    const hint = view.category && list && !list.types.length ? '<div class="f">No types of this category yet.</div>' : '';
    return `${none}${hint}${problem('element-type')}`;
  }
  const options = list.types.map((type) => `<option value="${esc(type.id)}"${view.type?.id === type.id ? ' selected' : ''}>${esc(type.name)}</option>`);
  const first = view.type ? '' : '<option value="" selected disabled>None</option>';
  return `<select class="inp" data-chg="stackelementtype" data-key="element-type" aria-label="Type">${first}${options.join('')}</select>${problem('element-type')}`;
}

CHG.stackelementtype = (select) => {
  if (S.sel && select.value) manage('element.type', { element: S.sel, type: select.value }, 'element-type');
};

export { FROM, hasShape, parametersTable, paramsFor, refreshSide, typeCell, worked };
