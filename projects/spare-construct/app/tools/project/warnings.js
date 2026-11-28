// Tool: the model's warnings, in the browser and the status bar.

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, modelUrl, spent } from '../../core/state.js';
import { drawBrowser } from '../../core/browser.js';
import { plural } from '../../core/problems.js';

// --- Views ---
// --- Warnings ---
// The problems the model holds (construct-warnings, A2-F7): the server works the list out of the
// document each time it is asked, so this page keeps no rules and no copy of one. It reads the
// list when a model opens and again after every entry of the log that changes the model, an undo
// and a redo included, whoever made it, so the count in the status bar and the list in the project
// browser are never stale. Choosing a warning selects the element it names, the way any list in
// the page does; a warning of two walls offers each.

C.warnings = null; // the open model's warnings as the server lists them; null until they come
C.warnSeq = 0; // goes up with each read, so an old answer is not kept
C.warnTimer = null;

/** Reads the model's warnings and draws the count and the list. */
async function loadWarnings() {
  if (!C.model) return;
  const model = C.model.id;
  C.warnSeq += 1;
  const seq = C.warnSeq;
  let answer;
  try {
    answer = await api(`${modelUrl(model)}/warnings`);
  } catch {
    return;
  }
  if (C.model?.id !== model || seq !== C.warnSeq) return;
  const same = JSON.stringify(C.warnings) === JSON.stringify(answer.warnings);
  C.warnings = answer.warnings;
  if (same) return;
  spent('warnings-draw', () => {
    statusBar();
    // Only the Warnings part is drawn again, never the whole browser: a person may be typing in it.
    const part = $('#browser [data-warnings]');
    if (part) part.innerHTML = warningsList();
    else drawBrowser();
  });
}

/** An entry of the log arrived for the open model: read the list again once the entries arriving together have come. */
function warningsChanged(entry) {
  if (/^element\.(claim|release)$/.test(entry.cmd)) return;
  clearTimeout(C.warnTimer);
  C.warnTimer = setTimeout(loadWarnings, 100);
}

/** The count, always in the status bar: a button that shows the list. */
function warningsCount() {
  if (!C.model) return '';
  const n = C.warnings?.length;
  const words = n === undefined ? 'Warnings' : n === 0 ? 'No warnings' : plural(n, 'warning');
  return `<button id="st-warnings" data-act="warnlist" data-count="${n ?? ''}">${ic('warn', 'sm')}${esc(words)}</button><span class="sep"></span>`;
}

const SEVERITY = { notice: 'Notice', warning: 'Warning', error: 'Error' };

/** The project browser's Warnings part: each warning with how serious it is and its sentence. */
function warningsList() {
  if (!C.warnings) return '<p class="pad small faint">Loading.</p>';
  if (!C.warnings.length) return '<p class="pad small faint" data-warnings-empty>No warnings: nothing in this model needs a look.</p>';
  let html = '';
  for (const w of C.warnings) {
    html += `<div class="stack-warning" data-warning="${esc(w.identity)}" data-severity="${esc(w.severity)}" data-kind="${esc(w.kind)}">`;
    html += `<button class="leaf" data-act="stackwarning" data-a="${esc(w.elements[0])}">${ic('warn', 'sm')}<span><b>${esc(SEVERITY[w.severity] || w.severity)}</b>: ${esc(w.words)}</span></button>`;
    if (w.elements.length > 1) {
      html += '<div class="stack-warning-at">';
      for (const id of w.elements) html += `<button class="btn sm" data-act="stackwarning" data-a="${esc(id)}">${esc(id)}</button>`;
      html += '</div>';
    }
    html += '</div>';
  }
  return html;
}

ACT.stackwarning = (button) => select(button.dataset.a);

/** The count in the status bar, and the ribbon's Warnings, show the list. */
ACT.warnlist = () => {
  BOPEN.add('Warnings');
  drawBrowser();
  $('#browser [data-k="Warnings"]')?.scrollIntoView({ block: 'start' });
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.models({ id: 'warnings', order: 50, open: () => loadWarnings() });
register.logs({ id: 'warnings', order: 20, entry: (entry) => warningsChanged(entry) });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get warningsCount() { return warningsCount; }, get warningsList() { return warningsList; },
});
