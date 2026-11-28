// Tool: a view's crop and range.

import { register } from '../../core/registry.js';
import { C } from '../../core/state.js';
import { canEdit, manage, problem } from '../../core/problems.js';
import { modelLevels } from './levels.js';

// --- A view's crop and range ---
// The part of the model a view draws, and how high a plan cuts (construct-views' FORMAT.md,
// "Crop regions" and "View range"). The dialog is reached from the open view's line in the
// browser and reads the view as the server lists it (`/views`), which this page keeps live through
// the log, so a change by anyone shows here and in the plan at once. A control sends only what it
// holds: a height as `view.set` with that one part of `range`, the region as `view.crop.set`, the
// switches as `view.crop.set` (on) and `view.crop.show` (shown), and Remove as `view.crop.clear`.
// What is typed is the person's own text (`6 m`, `-500`): the server reads it, refuses what it
// cannot in words beside the controls, and the drawing, clipped to the crop and with its outline,
// is the server's.

const RANGE_ROWS = [['top', 'Top'], ['cut', 'Cut plane'], ['bottom', 'Bottom'], ['depth', 'View depth']];

function cropHtml(m) {
  const view = C.views?.find((v) => v.id === m.view);
  if (!view) return '<p class="small faint">This view has gone.</p>';
  const off = canEdit() ? '' : ' disabled';
  let html = '<p class="small muted">The part of the model this view draws, and how high a plan cuts. Heights are an offset from a level; the crop is in the plan’s own x and y.</p>';
  if (view.range) {
    const levels = modelLevels() || [];
    html += '<h3>View range</h3><table class="props" data-view-range>';
    for (const [key, label] of RANGE_ROWS) {
      const tie = view.range[key];
      const known = levels.some((level) => level.id === tie.level) ? levels : [...levels, { id: tie.level, name: tie.level }];
      const options = known.map((level) => `<option value="${esc(level.id)}"${level.id === tie.level ? ' selected' : ''}>${esc(level.name)}</option>`).join('');
      const control = `<select class="inp" data-chg="stackrangelevel" data-range="${key}" data-key="range-level:${key}" aria-label="${label}, level"${off}>${options}</select>`
        + `<input class="inp" data-chg="stackrangeoffset" data-range="${key}" data-key="range-offset:${key}" value="${esc(tie.offset)}" aria-label="${label}, offset from the level"${off}>`;
      const height = view.range.heights?.[key];
      html += prow(label, control, height ? `at ${esc(height)}` : 'does not resolve');
    }
    html += `</table>${problem('range')}`;
  }
  html += '<h3>Crop region</h3>';
  const crop = view.crop;
  const field = (key, label, value) => `<input class="inp" data-chg="stackcropedit" data-crop="${key}" data-key="crop-${key}" value="${esc(value)}" aria-label="${label}"${off}>`;
  if (!crop) {
    const draft = (key, label) => `<input class="inp" data-draft data-enter="stackcropmake" data-key="crop-new-${key}" placeholder="0 mm" aria-label="${label}"${off}>`;
    html += '<p class="small faint" data-crop-none>This view has no crop: it draws all of the model.</p><table class="props">';
    html += prow('Lower left', `${draft('minx', 'Lower left, x')}${draft('miny', 'Lower left, y')}`, '');
    html += prow('Upper right', `${draft('maxx', 'Upper right, x')}${draft('maxy', 'Upper right, y')}`, '');
    html += `</table><div class="stack-line"><button class="btn pri sm" data-act="stackcropmake" data-key="crop-make"${off}>Make crop</button></div>`;
  } else {
    html += `<div class="stack-line" data-crop-state="${crop.on ? 'on' : 'off'}-${crop.shown ? 'shown' : 'hidden'}">`;
    html += `<label class="chk"><input type="checkbox" data-chg="stackcropon" data-key="crop-on"${crop.on ? ' checked' : ''}${off}>Crop the view to the region</label>`;
    html += `<label class="chk"><input type="checkbox" data-chg="stackcropshown" data-key="crop-shown"${crop.shown ? ' checked' : ''}${off}>Show the crop outline</label></div>`;
    html += '<table class="props" data-crop-region>';
    const corners = crop.region.corners;
    if (crop.region.shape === 'rectangle') {
      html += prow('Lower left', `${field('minx', 'Lower left, x', corners[0].x)}${field('miny', 'Lower left, y', corners[0].y)}`, '');
      html += prow('Upper right', `${field('maxx', 'Upper right, x', corners[2].x)}${field('maxy', 'Upper right, y', corners[2].y)}`, '');
    } else {
      corners.forEach((corner, at) => {
        html += prow(`Corner ${at + 1}`, `${field(`c${at}x`, `Corner ${at + 1}, x`, corner.x)}${field(`c${at}y`, `Corner ${at + 1}, y`, corner.y)}`, '');
      });
    }
    html += '</table>';
    html += `<div class="stack-line"><button class="btn sm" data-act="stackcropclear" data-key="crop-clear"${off}>Remove crop</button></div>`;
  }
  return `${html}${problem('crop')}`;
}

const cropView = () => C.views?.find((v) => v.id === C.manage?.view);
CHG.stackrangelevel = (select) => manage('view.set', { view: C.manage.view, range: { [select.dataset.range]: { level: select.value } } }, 'range');
CHG.stackrangeoffset = (input) => manage('view.set', { view: C.manage.view, range: { [input.dataset.range]: { offset: input.value } } }, 'range');
CHG.stackcropon = (box) => manage('view.crop.set', { view: C.manage.view, on: box.checked }, 'crop');
CHG.stackcropshown = (box) => manage('view.crop.show', { view: C.manage.view, shown: box.checked }, 'crop');
CHG.stackcropedit = () => {
  const view = cropView();
  if (!view?.crop) return;
  const read = (key) => $(`#stack-manage [data-crop="${key}"]`).value;
  const region = view.crop.region.shape === 'rectangle'
    ? { rectangle: { min: { x: read('minx'), y: read('miny') }, max: { x: read('maxx'), y: read('maxy') } } }
    : { outline: view.crop.region.corners.map((_, at) => ({ x: read(`c${at}x`), y: read(`c${at}y`) })) };
  manage('view.crop.set', { view: view.id, region }, 'crop');
};
ACT.stackcropmake = () => {
  const read = (key) => $(`#stack-manage [data-key="crop-new-${key}"]`).value;
  const region = { rectangle: { min: { x: read('minx'), y: read('miny') }, max: { x: read('maxx'), y: read('maxy') } } };
  manage('view.crop.set', { view: C.manage.view, region }, 'crop');
};
ACT.stackcropclear = () => manage('view.crop.clear', { view: C.manage.view }, 'crop');

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.dialogs({ id: 'crop', order: 40, html: (m) => cropHtml(m) });
