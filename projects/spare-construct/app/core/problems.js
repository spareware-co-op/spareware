// Problems and the edits a dialog or a palette sends: what was refused, and what is waiting.

import { late } from './late.js';
import { C, say } from './state.js';
import { drawBrowser } from './browser.js';
import { edit } from './channel.js';
import { refreshSide } from './parameters.js';
import { renderManage } from './dialogs.js';

// --- Types and parameters ---
// A person manages the project's parameters and the types of a category in two dialogs, and changes
// an element's type in the properties palette. Everything is one of the ten commands through the
// same log (construct-model's FORMAT.md, "Categories, types and parameter values"): the page sends
// what was typed and shows what the server answers. The lists (kinds, categories, parameters, types)
// are read from the server, values are shown as the server formats them, and a refusal is shown in
// the server's own words beside what caused it. Nothing is checked here.

C.catalog = null; // { model, kinds, categories }
C.typeList = null; // { key, view }: the types of the selected element's category
C.manage = null; // the open dialog: { which, category, open, data, error }
C.problems = new Map(); // where -> the server's reason, until that is tried again
C.asked = new Map(); // edit counter -> { where, after, hold }
// Edits of the crop and range panel sent and not yet answered. While there are any, the panel is not
// drawn again from the page's older record of the view: a person's click on one of its controls
// (the crop's checkboxes, a field) stays as it was set until the server's answer for that command
// arrives, and the panel is then drawn from the view as it is after it.
C.cropHold = 0;

const canEdit = () => Boolean(C.model) && !C.readOnly && !C.viewing;
const capital = (text) => text.charAt(0).toUpperCase() + text.slice(1);
const topLevel = (catalog) => catalog.categories.filter((category) => category.parent === null);
const newId = (tag) => `${tag}-${crypto.randomUUID().slice(0, 8)}`;
const plural = (count, word) => `${count} ${word}${count === 1 ? '' : 's'}`;

/** The server's reason an edit was refused, in a slot beside what caused it. */
function problem(where) {
  const text = C.problems.get(where);
  return text ? `<div class="note bad small" role="alert" data-problem="${esc(where)}">${esc(text)}</div>` : '';
}

function clearProblems(which) {
  for (const where of [...C.problems.keys()]) {
    if (which.test(where)) C.problems.delete(where);
  }
}

/** Sends one of the ten commands, and remembers where to show the reason if it is turned back. */
function manage(cmd, args, where, after) {
  if (!C.model || C.readOnly) {
    say('Open a model in your team’s WIP to change it.');
    return;
  }
  C.problems.delete(where);
  const hold = where === 'crop' || where === 'range';
  if (hold) C.cropHold += 1;
  const counter = edit(cmd, args);
  C.asked.set(counter, { where, after, hold });
  showProblems();
}

/** An edit of the page's own was turned back: its reason goes where it was asked. */
function refused(counter, reason) {
  const ask = C.asked.get(counter);
  C.asked.delete(counter);
  if (!ask) return;
  if (ask.hold) C.cropHold = Math.max(0, C.cropHold - 1);
  C.problems.set(ask.where, reason);
  late.dimsRefused(ask.where);
  showProblems();
}

/** An edit of the page's own was applied. */
function done(counter) {
  const ask = C.asked.get(counter);
  C.asked.delete(counter);
  if (ask?.hold) {
    C.cropHold = Math.max(0, C.cropHold - 1);
    // The views read while it was held were not drawn: read them again, and draw them.
    if (C.cropHold === 0) late.loadViews();
  }
  if (ask?.after) ask.after();
}

function showProblems() {
  renderManage();
  late.renderSchedule();
  refreshSide();
  drawBrowser();
}

export { canEdit, capital, clearProblems, done, manage, newId, plural, problem, refused, topLevel };
