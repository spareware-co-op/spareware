// Problems and the edits a dialog or a palette sends: what was refused, and what is waiting.

import { late } from './late.js';
import { entries } from './registry.js';
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
C.asked = new Map(); // edit counter -> { where, after }
// The controls holding a person's pending value, by the counter of the edit they sent (see
// holdControl below).
C.holds = new Map(); // edit counter -> { key, checks, value, where }

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
  const counter = edit(cmd, args);
  C.asked.set(counter, { where, after });
  holdControl(counter, where);
  showProblems();
}

/** An edit of the page's own was turned back: its reason goes where it was asked. */
function refused(counter, reason) {
  letGoOfHold(counter, false);
  const ask = C.asked.get(counter);
  C.asked.delete(counter);
  if (!ask) return;
  C.problems.set(ask.where, reason);
  late.dimsRefused(ask.where);
  showProblems();
}

/** An edit of the page's own was applied. */
function done(counter) {
  letGoOfHold(counter, true);
  const ask = C.asked.get(counter);
  C.asked.delete(counter);
  if (ask?.after) ask.after();
}

function showProblems() {
  renderManage();
  late.renderSchedule();
  refreshSide();
  drawBrowser();
}

/* Held values. A panel is drawn again from the page's record of the model whenever anything changes,
   and until the server has answered a person's change that record is older than what they set: the
   redraw would put the old value back under their click. So the control a change came from keeps the
   person's value until the server answers the command it sent. manage() holds it (by its data-key);
   redraw() gives it back to the control it draws again; the answer lets go: a refusal at once, so the
   redraw that shows the refusal shows the server's value, and the panel's data is read again; an
   applied command once the tool has read its data again (register.holds), or at once when it has
   no way to. One place for every tool's panels; a tool keeps no hold of its own. */
let changing = null; // the control a change event is being handled for, while it is

/** Notes the control each change comes from, before the page's own handler sends its command. */
function watchHeldControls() {
  document.addEventListener(
    'change',
    (event) => {
      changing = event.target.closest?.('[data-key]') || null;
    },
    true,
  );
  window.addEventListener('change', () => {
    changing = null;
  });
}

/** Holds the control the person just changed, for the edit `counter` it sent. */
function holdControl(counter, where) {
  const control = changing;
  if (!control || !/^(INPUT|SELECT|TEXTAREA)$/.test(control.tagName)) return;
  const checks = control.type === 'checkbox' || control.type === 'radio';
  C.holds.set(counter, { key: control.dataset.key, checks, value: checks ? control.checked : control.value, where });
}

/** Gives each held control in `root`, just drawn again, the person's pending value. */
function keepHeld(root) {
  for (const hold of C.holds.values()) {
    const control = $$('[data-key]', root).find((el) => el.dataset.key === hold.key);
    if (!control) continue;
    if (hold.checks) control.checked = hold.value;
    else control.value = hold.value;
  }
}

/** The server answered the edit `counter`: lets go of its hold, as the note above says. */
function letGoOfHold(counter, applied) {
  const hold = C.holds.get(counter);
  if (!hold) return;
  const reread = entries('holds').find((entry) => entry.where.includes(hold.where))?.reread;
  if (!applied || !reread) {
    C.holds.delete(counter);
    if (!applied && reread) setTimeout(reread);
    return;
  }
  // Once the entry has been taken whole (the reads it starts are on their way), read again, and let go
  // once that read has been drawn.
  setTimeout(() => Promise.resolve(reread()).finally(() => C.holds.delete(counter)));
}

export {
  canEdit, capital, clearProblems, done, keepHeld, manage, newId, plural, problem, refused,
  topLevel, watchHeldControls,
};
