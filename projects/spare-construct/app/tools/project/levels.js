// Tool: levels (the Levels dialog, and the plan's level).

import { late, provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, levelIdOf, levelOf, modelUrl } from '../../core/state.js';
import { drawBrowser } from '../../core/browser.js';
import { followTheLevel } from '../../core/shapes.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, plural, problem } from '../../core/problems.js';
import { manageField, renderManage } from '../../core/dialogs.js';

// --- Levels ---
// A level is a record of the model with a name and an elevation, and an element is tied to one by
// its ID (construct-levels' FORMAT.md). The dialog lists them as the server reads them, lowest
// first, with each elevation as the server formats it; a change is one of the level commands
// through the same log, and a refusal is the server's reason beside the level. Show picks the level
// the plan shows and a newly drawn element is tied to.

C.planLevel = null; // { id, name } of a level chosen in the dialog; null is the floor plan's own
C.levelList = null; // { key, levels }: the open model's levels, for the properties palette

/** Keeps the levels the server listed, and the shown level's name with them. */
function knowLevels(key, levels) {
  C.levelList = { key, levels };
  drawBrowser(); // the new-view form offers these
  const shown = C.planLevel && levels.find((level) => level.id === C.planLevel.id);
  if (shown && shown.name !== C.planLevel.name) {
    C.planLevel.name = shown.name;
    headerView();
  }
}

/** The open model's levels, lowest first, as the server lists them; null until they have come. */
function modelLevels() {
  if (!C.model) return null;
  const key = `${C.model.id}/${C.paramsEpoch}`;
  if (C.levelList?.key !== key && C.levelListLoading !== key) {
    C.levelListLoading = key;
    api(`${modelUrl(C.model.id)}/levels`)
      .then((levels) => {
        knowLevels(key, levels);
        refreshSide();
      })
      .catch(() => {});
  }
  return C.levelList?.key.startsWith(`${C.model.id}/`) ? C.levelList.levels : null;
}

/** The name of the level a record stands on, from the server's list once it has come. */
function levelName(record) {
  const id = levelIdOf(record);
  if (!id) return 'None';
  return modelLevels()?.find((level) => level.id === id)?.name ?? id;
}

function levelsHtml(m) {
  const editable = canEdit();
  const off = editable ? '' : ' disabled';
  let html = '<p class="small muted">A level is a height in the building that elements stand on. Move a level and everything tied to it moves with it. Show a level to see its plan and draw on it.</p>';
  if (!m.data.levels.length) html += '<p class="small faint">No levels yet.</p>';
  for (const level of m.data.levels) {
    const id = esc(level.id);
    const label = esc(level.name);
    const shown = S.screen === 'plan' && levelOf() === level.id;
    html += `<section data-level="${id}"><div class="stack-line">`;
    html += `<input class="inp" data-chg="stacklevelrename" data-level="${id}" data-key="level-name:${id}" value="${label}" aria-label="Name of ${label}"${off}>`;
    html += `<input class="inp" data-chg="stacklevelelevation" data-level="${id}" data-key="level-elevation:${id}" value="${esc(level.shown)}" aria-label="Elevation of ${label}"${off}>`;
    html += `<label class="chk"><input type="checkbox" data-chg="stacklevelstorey" data-level="${id}" data-key="level-storey:${id}"${level.storey ? ' checked' : ''}${off}>Storey</label>`;
    html += `<span class="small faint">${plural(level.tied, 'element')}</span>`;
    html += `<button class="btn sm" data-act="stacklevelshow" data-a="${id}" data-key="level-show:${id}" aria-pressed="${shown}">${shown ? 'Shown' : 'Show'}</button>`;
    html += `<button class="btn sm" data-act="stackleveldelete" data-a="${id}" data-key="level-delete:${id}" aria-label="Delete ${label}"${off}>Delete</button></div>`;
    html += `${problem(`level:${level.id}`)}</section>`;
  }
  html += '<h3 class="mt2">New level</h3>';
  html += `<div class="stack-line"><input class="inp" data-draft data-enter="stacknewlevel" data-key="new-level-name" placeholder="Name" aria-label="Name for a new level"${off}>`;
  html += `<input class="inp" data-draft data-enter="stacknewlevel" data-key="new-level-elevation" placeholder="Elevation" aria-label="Elevation for a new level"${off}>`;
  html += `<button class="btn pri sm" data-act="stacknewlevel" data-key="new-level-make"${off}>Make level</button></div>`;
  html += problem('new-level');
  return html;
}

const levelWhere = (control) => `level:${control.dataset.level ?? control.dataset.a}`;
CHG.stacklevelrename = (input) => manage('level.rename', { level: input.dataset.level, name: input.value }, levelWhere(input));
CHG.stacklevelelevation = (input) => manage('level.elevation', { level: input.dataset.level, elevation: input.value }, levelWhere(input));
CHG.stacklevelstorey = (box) => manage('level.storey', { level: box.dataset.level, storey: box.checked }, levelWhere(box));
ACT.stackleveldelete = (button) => manage('level.delete', { level: button.dataset.a }, levelWhere(button));
ACT.stacknewlevel = () => {
  const args = { level: newId('L'), name: manageField('new-level-name').value, elevation: manageField('new-level-elevation').value };
  manage('level.create', args, 'new-level', () => {
    for (const key of ['new-level-name', 'new-level-elevation']) {
      const again = manageField(key);
      if (again) again.value = '';
    }
  });
};
ACT.stacklevelshow = (button) => {
  const level = C.manage?.data?.levels.find((l) => l.id === button.dataset.a);
  if (!level) return;
  C.planLevel = { id: level.id, name: level.name };
  late.leaveView();
  if (S.screen === 'plan' && FLOORV.includes(S.view)) {
    headerView();
    followTheLevel();
  } else {
    go('plan:l1');
  }
  renderManage();
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.dialogs({ id: 'levels', order: 20, html: (m) => levelsHtml(m) });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get knowLevels() { return knowLevels; }, get levelName() { return levelName; },
  get modelLevels() { return modelLevels; },
});

export { levelName, modelLevels };
