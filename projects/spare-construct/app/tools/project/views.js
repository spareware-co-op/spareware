// Tool: views, the server's drawing of the open view, and the Views part of the browser.

import { late, provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, levelOf, modelUrl } from '../../core/state.js';
import { drawBrowser } from '../../core/browser.js';
import { followTheLevel } from '../../core/shapes.js';
import { canEdit, manage, newId, problem } from '../../core/problems.js';
import { renderManage } from '../../core/dialogs.js';
import { queueDraw } from '../../core/plan.js';
import { modelLevels } from './levels.js';

// A view is a record of the model with a name, a kind, a scale and, for a plan, a level
// (construct-views' FORMAT.md). The project browser lists them as the server reads them and keeps
// the list live through the log; a person makes, renames and deletes one there with the view
// commands (`view.create`, `view.rename`, `view.delete`), never the generic element commands,
// which the server turns back for a view. Opening a floor plan shows the plan the server draws
// from the stored model at the log's head (`/views/<view>/plan`): this page draws that picture
// and cuts nothing itself. The picture goes under the live elements, which stay in the plan with
// their picking, claims and tools but draw no fill of their own while it is there (see
// `.dw.drawn` in connect.css), so a click still picks what it lands on and a wall drawn here or
// by someone else shows in the server's picture when the model changes. The plan is asked for
// again, after a short pause, whenever an edit changes the model or the view.

C.viewsSeq = 0; // goes up with each read of the list, so an old answer is not kept
C.planSeq = 0; // the same for the plan
C.planTimer = null;

/** Reads the model's views, and drops the open one if it has gone. */
async function loadViews() {
  if (!C.model) return;
  const model = C.model.id;
  C.viewsSeq += 1;
  const seq = C.viewsSeq;
  let list;
  try {
    list = await api(`${modelUrl(model)}/views`);
  } catch {
    return;
  }
  if (C.model?.id !== model || seq !== C.viewsSeq) return;
  C.views = list;
  if (C.manage?.which === 'crop') renderManage();
  const open = list.find((view) => view.id === C.openView);
  if (C.openView && !open) leaveView();
  else if (open) {
    const shown = open.level && { id: open.level, name: open.level_name || open.level };
    if (shown && C.planLevel && (C.planLevel.id !== shown.id || C.planLevel.name !== shown.name)) {
      C.planLevel = shown;
      followTheLevel();
    }
    planChanged();
  }
  headerView();
  drawBrowser();
}

/** Stops showing a model view's plan; the plan pane draws only its live elements again. */
function leaveView() {
  C.openView = null;
  C.drawn = null;
  C.planSeq += 1;
  clearTimeout(C.planTimer);
  drawBrowser();
}

/** An entry of the log arrived for the open model: what it changes of views and of the plan. */
function viewsChanged(entry) {
  if (/^view\.|^level\.|^edit\.(undo|redo)$/.test(entry.cmd)) loadViews();
  else if (!/^element\.(claim|release)$/.test(entry.cmd)) planChanged();
  if (!/^element[.](claim|release)$/.test(entry.cmd)) late.dimsChanged();
}

/** Asks for the open view's plan again, once the edits arriving together have all come. */
function planChanged() {
  if (!C.openView) return;
  clearTimeout(C.planTimer);
  C.planTimer = setTimeout(loadPlan, 150);
}

/** Asks the server for the open view's plan and draws what comes back. */
async function loadPlan() {
  const view = C.openView;
  const model = C.model?.id;
  if (!view || !model) return;
  C.planSeq += 1;
  const seq = C.planSeq;
  let drawn;
  try {
    const answer = await api(`${modelUrl(model)}/views/${encodeURIComponent(view)}/plan`);
    drawn = { view, svg: answer.svg, skipped: answer.skipped, through: answer.through };
  } catch (error) {
    drawn = { view, error: error.message };
  }
  if (C.openView !== view || C.model?.id !== model || seq !== C.planSeq) return;
  C.drawn = drawn;
  queueDraw();
}

/** Opens a model view in the plan pane: a plan shows its level, drawn by the server. */
function openView(id) {
  const view = C.views?.find((v) => v.id === id);
  if (!view) return;
  C.openView = id;
  C.drawn = null;
  if (view.level) C.planLevel = { id: view.level, name: view.level_name || view.level };
  if (S.screen === 'plan' && FLOORV.includes(S.view)) {
    headerView();
    followTheLevel();
  } else {
    go(`plan:${FLOORV.includes(S.view) ? S.view : 'l1'}`);
  }
  drawBrowser();
  loadPlan();
}

/** What the server's drawing adds to the plan pane's SVG, under everything live. */
function drawnPlan() {
  const drawn = C.drawn;
  if (!drawn?.svg || drawn.view !== C.openView) return '';
  if (drawn.body === undefined) drawn.body = plainSvg(drawn.svg);
  // The server's y runs up the sheet and is written negated; this plan's y runs down the model.
  return `<g class="drawn" data-view="${esc(drawn.view)}" transform="scale(1 -1)">${drawn.body}</g>`;
}

const DRAWN_TAGS = new Set(['g', 'path', 'line']);
const DRAWN_ATTRS = new Set([
  'd', 'x1', 'y1', 'x2', 'y2', 'fill', 'stroke', 'stroke-width', 'stroke-linecap', 'opacity', 'data-element', 'data-role', 'data-material',
]);

/** The lines and hatches of the server's SVG document, and nothing else a document could carry. */
function plainSvg(text) {
  const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
  const walk = (node) => {
    let out = '';
    for (const child of node.children) {
      const tag = child.localName;
      if (!DRAWN_TAGS.has(tag)) continue;
      const attrs = [...child.attributes]
        .filter((a) => DRAWN_ATTRS.has(a.name))
        .map((a) => ` ${a.name}="${esc(a.value)}"`)
        .join('');
      out += `<${tag}${attrs}>${walk(child)}</${tag}>`;
    }
    return out;
  };
  return doc.documentElement.localName === 'svg' ? walk(doc.documentElement) : '';
}

/** A line over the plan saying what the server's picture is missing, when it is. */
function planNote() {
  const drawn = C.drawn;
  if (!C.openView) return '';
  let text;
  let cls = 'stack-plan-note';
  if (!drawn) text = 'Drawing the plan.';
  else if (drawn.error) {
    text = drawn.error;
    cls += ' bad';
  } else if (drawn.skipped.length) {
    text = `Not drawn: ${drawn.skipped.map((s) => s.element).join(', ')}.`;
  } else return '';
  return `<div class="${cls}" role="status">${esc(text)}</div>`;
}

/** The browser's part for the model's views: each one, the open one's name and delete, and a new one. */
function modelViews(leaf) {
  if (!C.model) return '';
  let html = '<div class="sub">Model views</div>';
  if (!C.views) return `${html}<p class="pad small faint">Loading.</p>`;
  if (!C.views.length) html += '<p class="pad small faint">No views yet.</p>';
  const off = canEdit() ? '' : ' disabled';
  for (const view of C.views) {
    const open = view.id === C.openView;
    html += `<div data-view="${esc(view.id)}">${leaf('stackview', view.id, view.name, open && S.screen === 'plan', view.kind === 'floor_plan' ? 'plan' : 'cube', `title="${esc(`${view.kind}, 1:${view.scale}`)}"`)}`;
    if (open) {
      const id = esc(view.id);
      html += `<div class="stack-line pad"><input class="inp" data-chg="stackviewrename" data-view="${id}" data-key="view-name:${id}" value="${esc(view.name)}" aria-label="Name of ${esc(view.name)}"${off}>`;
      html += `<button class="btn sm" data-act="stackviewgraphics" data-a="${id}" data-key="view-graphics:${id}" aria-label="Graphics of ${esc(view.name)}">Graphics</button>`;
      html += `<button class="btn sm" data-act="stackviewcrop" data-a="${id}" data-key="view-crop:${id}" aria-label="Crop and range of ${esc(view.name)}">Crop and range</button>`;
      html += `<button class="btn sm" data-act="stackviewdelete" data-a="${id}" data-key="view-delete:${id}" aria-label="Delete ${esc(view.name)}"${off}>Delete</button></div>`;
      html += problem(`view:${view.id}`);
    }
    html += '</div>';
  }
  const levels = modelLevels() || [];
  const here = C.newViewLevel ?? levelOf();
  const options = levels.map((l) => `<option value="${esc(l.id)}"${l.id === here ? ' selected' : ''}>${esc(l.name)}</option>`).join('');
  html += `<div class="stack-line pad"><input class="inp" data-draft data-enter="stacknewview" data-key="new-view-name" placeholder="New plan name" aria-label="Name for a new plan view"${off}>`;
  html += `<select class="inp" data-chg="stacknewviewlevel" data-key="new-view-level" aria-label="Level of the new plan view"${off}>${options}</select>`;
  html += `<button class="btn pri sm" data-act="stacknewview" data-key="new-view-make"${off}>Make plan</button></div>`;
  return html + problem('new-view');
}

CHG.stacknewviewlevel = (select) => {
  C.newViewLevel = select.value; // kept across redraws of the browser
};
ACT.stackview = (button) => openView(button.dataset.a);
CHG.stackviewrename = (input) => manage('view.rename', { view: input.dataset.view, name: input.value }, `view:${input.dataset.view}`);
ACT.stackviewdelete = (button) => manage('view.delete', { view: button.dataset.a }, `view:${button.dataset.a}`);
ACT.stacknewview = () => {
  const name = $('#browser [data-key="new-view-name"]');
  const level = $('#browser [data-key="new-view-level"]');
  if (!name || !level) return;
  const args = { view: newId('VP'), kind: 'floor_plan', name: name.value, level: level.value };
  manage('view.create', args, 'new-view', () => {
    const again = $('#browser [data-key="new-view-name"]');
    if (again) again.value = '';
  });
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.layers({ id: 'server-drawing', order: 20, under: () => drawnPlan(), note: () => planNote() });
register.models({ id: 'views', order: 40, open: () => loadViews() });
register.logs({ id: 'views', order: 10, entry: (entry) => viewsChanged(entry) });

// What files loaded before this one use from it, bound once it has loaded.
provide({ get leaveView() { return leaveView; }, get modelViews() { return modelViews; } });

export { loadViews };
