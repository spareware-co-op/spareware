// Tool: schedules.

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, modelUrl, say, spent } from '../../core/state.js';
import { drawBrowser } from '../../core/browser.js';
import { canEdit, manage, newId, plural, problem } from '../../core/problems.js';
import { redraw } from '../../core/dialogs.js';

// --- Schedules ---
// A schedule is a record of the model: a name, the category it lists, its fields in order, its
// filters and its sorting (construct-schedules' FORMAT.md). Its rows are worked out by the server
// from the model each time they are asked for and are stored nowhere, so this page keeps no rows
// of its own and works none out: it asks for the table (`/schedules/<schedule>`: the columns, the
// rows with each value as the server shows it with its unit, and the fields the category offers)
// and draws it. The table is asked for again after every entry of the log for the open model (an
// edit of this browser's or another's, an undo, a redo) except claims, which change no row, once
// the entries arriving together have all come, so it is never older than the log the page holds.
// The browser lists the schedules in a group of their own beside the views; a person makes one
// there (a name and a category), renames it (type the new name, Enter) and deletes it, all
// through the schedule commands, never the generic element commands, which the server turns back
// for a schedule. Opening one shows its table in a dialog with its fields, filters and sorting
// to change, each a whole-list schedule command; a refusal is the server's words in the part
// where the person made the change. Choosing a row selects that element in the model.

C.schedules = null; // the model's schedules as the server lists them; null until they come
C.scheduleCategories = null; // { model, list }: the categories a schedule may list
C.sched = null; // the schedule last opened: { id, table, error, seq, gone, shown, draft }
C.schedulesSeq = 0; // goes up with each read of the list, so an old answer is not kept
C.schedulesTimer = null;
C.schedulesBusy = false; // a read is out
C.schedulesAgain = false; // an entry came while it was
C.newScheduleCategory = null;

const COMPARISONS = {
  equals: 'is',
  not_equals: 'is not',
  less: 'is less than',
  less_or_equal: 'is at most',
  greater: 'is more than',
  greater_or_equal: 'is at least',
  contains: 'contains',
  begins_with: 'begins with',
};

/** Reads the model's schedules and the open table again; one read at a time, and one more if entries came meanwhile. */
async function loadSchedules() {
  if (C.schedulesBusy) {
    C.schedulesAgain = true;
    return;
  }
  C.schedulesBusy = true;
  try {
    await readSchedules();
  } finally {
    C.schedulesBusy = false;
  }
  if (C.schedulesAgain) {
    C.schedulesAgain = false;
    loadSchedules();
  }
}

/** Reads the model's schedules and the categories they may list, and the open table again. */
async function readSchedules() {
  if (!C.model) return;
  const model = C.model.id;
  C.schedulesSeq += 1;
  const seq = C.schedulesSeq;
  let list;
  let categories = C.scheduleCategories;
  try {
    list = await api(`${modelUrl(model)}/schedules`);
    if (categories?.model !== model) categories = { model, list: await api(`${modelUrl(model)}/schedule-categories`) };
  } catch {
    return;
  }
  if (C.model?.id !== model || seq !== C.schedulesSeq) return;
  C.schedules = list;
  C.scheduleCategories = categories;
  // Gone only once it has been seen: one just made is not in a read that began before its entry.
  if (C.sched) C.sched.gone = C.sched.table !== null && !list.some((schedule) => schedule.id === C.sched.id);
  spent('schedules-draw', drawBrowser);
  if (C.sched?.gone) spent('schedule-draw', renderSchedule);
  else loadScheduleTable();
}

/** Reads the open schedule's table from the server and draws it. */
async function loadScheduleTable() {
  const open = C.sched;
  if (!open || !C.model || open.gone) return;
  const model = C.model.id;
  open.seq += 1;
  const seq = open.seq;
  try {
    const table = await api(`${modelUrl(model)}/schedules/${encodeURIComponent(open.id)}`);
    if (C.sched !== open || C.model?.id !== model || seq !== open.seq) return;
    open.table = table;
    open.error = null;
  } catch (error) {
    if (C.sched !== open || seq !== open.seq) return;
    // A schedule just made is not in the list until the log's entry for it comes.
    open.error = C.schedules?.some((schedule) => schedule.id === open.id) ? error.message : null;
  }
  spent('schedule-draw', renderSchedule);
}

/** An entry of the log arrived for the open model: its schedules and the open table may differ. */
function schedulesChanged(entry) {
  if (/^element\.(claim|release)$/.test(entry.cmd)) return;
  // With no schedule there are no rows to follow; one is made or comes back only by these.
  if (C.schedules && !C.schedules.length && !/^schedule\.|^edit\.(undo|redo)$/.test(entry.cmd)) return;
  clearTimeout(C.schedulesTimer);
  C.schedulesTimer = setTimeout(loadSchedules, 100);
}

/** The browser's group for the model's schedules: each one, the open one's name and delete, and a new one. */
function modelSchedules(leaf) {
  if (!C.model) return '';
  if (!C.schedules) return '<p class="pad small faint">Loading.</p>';
  let html = '';
  if (!C.schedules.length) html += '<p class="pad small faint">No schedules yet.</p>';
  const off = canEdit() ? '' : ' disabled';
  for (const schedule of C.schedules) {
    const open = schedule.id === C.sched?.id;
    const id = esc(schedule.id);
    html += `<div data-schedule="${id}">${leaf('stackschedule', schedule.id, schedule.name, open, 'sheet', `title="${esc(`Lists ${schedule.category_name}`)}"`)}`;
    if (open) {
      html += `<div class="stack-line pad"><input class="inp" data-chg="stackschedrename" data-schedule="${id}" data-key="schedule-name:${id}" value="${esc(schedule.name)}" aria-label="Name of ${esc(schedule.name)}"${off}>`;
      html += `<button class="btn sm" data-act="stackscheddelete" data-a="${id}" data-key="schedule-delete:${id}" aria-label="Delete ${esc(schedule.name)}"${off}>Delete</button></div>`;
      html += problem(`schedule:${schedule.id}`);
    }
    html += '</div>';
  }
  const categories = C.scheduleCategories?.list ?? [];
  const here = C.newScheduleCategory ?? categories[0]?.id;
  const options = categories.map((c) => `<option value="${c.id}"${c.id === here ? ' selected' : ''}>${esc(c.name)}</option>`).join('');
  html += `<div class="stack-line pad"><input class="inp" data-draft data-enter="stacknewschedule" data-key="new-schedule-name" placeholder="New schedule name" aria-label="Name for a new schedule"${off}>`;
  html += `<select class="inp" data-chg="stacknewschedulecat" data-key="new-schedule-category" aria-label="Category the new schedule lists"${off}>${options}</select>`;
  html += `<button class="btn pri sm" data-act="stacknewschedule" data-key="new-schedule-make"${off}>Make schedule</button></div>`;
  return html + problem('new-schedule');
}

CHG.stacknewschedulecat = (select) => {
  C.newScheduleCategory = Number(select.value); // kept across redraws of the browser
};
CHG.stackschedrename = (input) => manage('schedule.rename', { schedule: input.dataset.schedule, name: input.value }, `schedule:${input.dataset.schedule}`);
ACT.stackscheddelete = (button) => manage('schedule.delete', { schedule: button.dataset.a }, `schedule:${button.dataset.a}`);
ACT.stacknewschedule = () => {
  const name = $('#browser [data-key="new-schedule-name"]');
  const category = $('#browser [data-key="new-schedule-category"]');
  if (!name || !category) return;
  const id = newId('SC');
  const args = { schedule: id, name: name.value, category: Number(category.value), fields: ['name'] };
  manage('schedule.create', args, 'new-schedule', () => {
    const again = $('#browser [data-key="new-schedule-name"]');
    if (again) again.value = '';
    openSchedule(id);
  });
};
ACT.stackschedule = (button) => openSchedule(button.dataset.a);

/** Opens a schedule's table in a dialog. */
function openSchedule(id) {
  const schedule = C.schedules?.find((s) => s.id === id);
  C.sched = { id, table: null, error: null, seq: 0, gone: false, shown: false, draft: {} };
  openDlg(schedule?.name ?? 'Schedule', `<div id="stack-sched" class="stack-manage stack-sched" data-schedule="${esc(id)}"><p class="small faint">Loading.</p></div>`, '<button class="btn" data-act="dgclose">Close</button>');
  drawBrowser();
  loadScheduleTable();
}

/** Draws the open schedule's dialog from the table as the server last gave it. */
function renderSchedule() {
  const open = C.sched;
  const root = $('#stack-sched');
  if (!open || !root || $('#dlg').hidden || root.dataset.schedule !== open.id) return;
  let html;
  if (open.gone) html = '<p class="note bad small" role="alert">This schedule has been deleted.</p>';
  else if (open.error) html = `<div class="note bad small" role="alert">${esc(open.error)}</div>`;
  else if (!open.table) html = '<p class="small faint">Loading.</p>';
  else html = scheduleHtml(open);
  redraw(root, html);
  if (open.table && !open.gone) $('#dlg-h').textContent = open.table.schedule.name;
}

/** The words for a field key, from what the category offers. */
const fieldName = (table, key) => table.offered.find((field) => field.field === key)?.name ?? key;

function scheduleHtml(open) {
  const table = open.table;
  const schedule = table.schedule;
  const id = esc(schedule.id);
  const off = canEdit() ? '' : ' disabled';
  const draft = open.draft;
  let html = `<p class="small muted">Lists ${esc(schedule.category_name)}: ${plural(table.rows.length, 'row')}, worked out from the model each time it changes. Choose a row to select that element.</p>`;
  if (schedule.problem) html += `<div class="note bad small" role="alert">${esc(schedule.problem)}</div>`;

  // The table: a heading for each field, a row for each element, each value as the server shows it.
  html += `<div class="sched-scroll"><table class="sched-table" data-schedule-table="${id}"><thead><tr>`;
  html += table.columns.map((column) => `<th data-field="${esc(column.field)}">${esc(column.name)}</th>`).join('');
  html += '</tr></thead><tbody>';
  for (const row of table.rows) {
    const chosen = S.sel === row.element;
    html += `<tr class="click${chosen ? ' sel' : ''}" data-act="stackschedrow" data-a="${esc(row.element)}" data-row="${esc(row.element)}"${chosen ? ' aria-selected="true"' : ''}>`;
    html += row.cells.map((cell) => `<td>${esc(cell.shown ?? '')}</td>`).join('');
    html += '</tr>';
  }
  html += '</tbody></table></div>';
  if (!table.rows.length) html += '<p class="small faint">No element passes the filters.</p>';

  // Fields: in order, to move or take off; the category's other fields to add.
  html += '<section data-part="fields"><h3>Fields</h3>';
  table.columns.forEach((column, at) => {
    html += `<div class="stack-line" data-field-row="${esc(column.field)}"><span class="grow">${esc(column.name)}</span>`;
    html += `<button class="btn sm" data-act="stackschedfieldup" data-a="${at}" aria-label="Move ${esc(column.name)} earlier"${at === 0 ? ' disabled' : off}>Earlier</button>`;
    html += `<button class="btn sm" data-act="stackschedfielddown" data-a="${at}" aria-label="Move ${esc(column.name)} later"${at === table.columns.length - 1 ? ' disabled' : off}>Later</button>`;
    html += `<button class="btn sm" data-act="stackschedfieldoff" data-a="${at}" aria-label="Take ${esc(column.name)} off"${off}>Remove</button></div>`;
  });
  const unused = table.offered.filter((field) => !schedule.fields.includes(field.field));
  if (unused.length) {
    const chosen = unused.find((field) => field.field === draft.add)?.field ?? unused[0].field;
    html += `<div class="stack-line"><select class="inp" data-chg="stackschedaddchoice" data-key="sched-add-field" aria-label="Field to add"${off}>`;
    html += unused.map((field) => `<option value="${esc(field.field)}"${field.field === chosen ? ' selected' : ''}>${esc(field.name)}</option>`).join('');
    html += `</select><button class="btn pri sm" data-act="stackschedfieldadd" data-key="sched-add-make"${off}>Add field</button></div>`;
  }
  html += `${problem(`sched-fields:${schedule.id}`)}</section>`;

  // Filters: every one must hold.
  html += '<section data-part="filters"><h3>Filters</h3>';
  if (!schedule.filters.length) html += '<p class="small faint">No filters: every element of the category is listed.</p>';
  schedule.filters.forEach((filter, at) => {
    const words = `${fieldName(table, filter.field)} ${COMPARISONS[filter.is] ?? filter.is} ${filter.to}`;
    html += `<div class="stack-line" data-filter-row="${at}"><span class="grow">${esc(words)}</span>`;
    html += `<button class="btn sm" data-act="stackschedfilteroff" data-a="${at}" aria-label="Remove the filter ${esc(words)}"${off}>Remove</button></div>`;
  });
  if (table.offered.length) {
    const field = table.offered.find((f) => f.field === draft.filterField) ?? table.offered[0];
    const is = field.comparisons.includes(draft.filterIs) ? draft.filterIs : field.comparisons[0];
    html += `<div class="stack-line"><select class="inp" data-chg="stackschedfilterfield" data-key="sched-filter-field" aria-label="Field to filter by"${off}>`;
    html += table.offered.map((f) => `<option value="${esc(f.field)}"${f.field === field.field ? ' selected' : ''}>${esc(f.name)}</option>`).join('');
    html += '</select>';
    html += `<select class="inp" data-chg="stackschedfilteris" data-key="sched-filter-is" aria-label="How to compare"${off}>`;
    html += field.comparisons.map((key) => `<option value="${esc(key)}"${key === is ? ' selected' : ''}>${esc(COMPARISONS[key] ?? key)}</option>`).join('');
    html += '</select>';
    html += `<input class="inp" data-draft data-enter="stackschedfilteradd" data-key="sched-filter-value" placeholder="Value" aria-label="Value to compare with"${off}>`;
    html += `<button class="btn pri sm" data-act="stackschedfilteradd" data-key="sched-filter-make"${off}>Add filter</button></div>`;
  }
  html += `${problem(`sched-filters:${schedule.id}`)}</section>`;

  // Sorting: the first key first, each up or down.
  html += '<section data-part="sorting"><h3>Sorting</h3>';
  if (!schedule.sorting.length) html += '<p class="small faint">Not sorted: the rows come in the order of the elements.</p>';
  schedule.sorting.forEach((sort, at) => {
    const words = `${fieldName(table, sort.field)}, ${sort.order === 'descending' ? 'down' : 'up'}`;
    html += `<div class="stack-line" data-sort-row="${at}"><span class="grow">${esc(words)}</span>`;
    html += `<button class="btn sm" data-act="stackschedsortflip" data-a="${at}" aria-label="Turn ${esc(fieldName(table, sort.field))} the other way round"${off}>${sort.order === 'descending' ? 'Up instead' : 'Down instead'}</button>`;
    html += `<button class="btn sm" data-act="stackschedsortoff" data-a="${at}" aria-label="Remove sorting by ${esc(fieldName(table, sort.field))}"${off}>Remove</button></div>`;
  });
  if (table.offered.length) {
    const field = table.offered.find((f) => f.field === draft.sortField)?.field ?? table.offered[0].field;
    html += `<div class="stack-line"><select class="inp" data-chg="stackschedsortfield" data-key="sched-sort-field" aria-label="Field to sort by"${off}>`;
    html += table.offered.map((f) => `<option value="${esc(f.field)}"${f.field === field ? ' selected' : ''}>${esc(f.name)}</option>`).join('');
    html += '</select>';
    html += `<select class="inp" data-chg="stackschedsortorder" data-key="sched-sort-order" aria-label="Which way"${off}>`;
    html += ['ascending', 'descending'].map((o) => `<option value="${o}"${o === (draft.sortOrder ?? 'ascending') ? ' selected' : ''}>${o === 'ascending' ? 'Up' : 'Down'}</option>`).join('');
    html += '</select>';
    html += `<button class="btn pri sm" data-act="stackschedsortadd" data-key="sched-sort-make"${off}>Add sorting</button></div>`;
  }
  html += `${problem(`sched-sorting:${schedule.id}`)}</section>`;
  return html;
}

/** Sends a whole new list for one part of the open schedule. */
const scheduleSet = (cmd, key, list, part) => {
  const open = C.sched;
  if (!open?.table) return;
  manage(cmd, { schedule: open.id, [key]: list }, `sched-${part}:${open.id}`);
};
const scheduleOf = () => C.sched?.table?.schedule;
const moved = (list, at, by) => {
  const next = [...list];
  const [item] = next.splice(at, 1);
  next.splice(at + by, 0, item);
  return next;
};

ACT.stackschedrow = (row) => {
  const id = row.dataset.a;
  if (!C.elements.has(id)) {
    say('That element is not in this model’s plan.');
    return;
  }
  select(id);
  renderSchedule();
};
CHG.stackschedaddchoice = (select) => {
  C.sched.draft.add = select.value;
};
ACT.stackschedfieldadd = () => {
  const field = $('#stack-sched [data-key="sched-add-field"]')?.value;
  if (field) scheduleSet('schedule.fields', 'fields', [...scheduleOf().fields, field], 'fields');
};
ACT.stackschedfieldoff = (button) => {
  const fields = scheduleOf().fields.filter((_, at) => at !== Number(button.dataset.a));
  scheduleSet('schedule.fields', 'fields', fields, 'fields');
};
ACT.stackschedfieldup = (button) => scheduleSet('schedule.fields', 'fields', moved(scheduleOf().fields, Number(button.dataset.a), -1), 'fields');
ACT.stackschedfielddown = (button) => scheduleSet('schedule.fields', 'fields', moved(scheduleOf().fields, Number(button.dataset.a), 1), 'fields');

CHG.stackschedfilterfield = (select) => {
  C.sched.draft.filterField = select.value;
  C.sched.draft.filterIs = undefined;
  renderSchedule();
};
CHG.stackschedfilteris = (select) => {
  C.sched.draft.filterIs = select.value;
};
ACT.stackschedfilteradd = () => {
  const root = $('#stack-sched');
  const field = $('[data-key="sched-filter-field"]', root)?.value;
  const is = $('[data-key="sched-filter-is"]', root)?.value;
  const to = $('[data-key="sched-filter-value"]', root)?.value ?? '';
  if (!field || !is) return;
  scheduleSet('schedule.filters', 'filters', [...scheduleOf().filters.map(({ field: f, is: i, to: t }) => ({ field: f, is: i, to: t })), { field, is, to }], 'filters');
};
ACT.stackschedfilteroff = (button) => {
  const filters = scheduleOf().filters.filter((_, at) => at !== Number(button.dataset.a));
  scheduleSet('schedule.filters', 'filters', filters.map(({ field, is, to }) => ({ field, is, to })), 'filters');
};

CHG.stackschedsortfield = (select) => {
  C.sched.draft.sortField = select.value;
};
CHG.stackschedsortorder = (select) => {
  C.sched.draft.sortOrder = select.value;
};
ACT.stackschedsortadd = () => {
  const root = $('#stack-sched');
  const field = $('[data-key="sched-sort-field"]', root)?.value;
  const order = $('[data-key="sched-sort-order"]', root)?.value;
  if (field && order) scheduleSet('schedule.sorting', 'sorting', [...scheduleOf().sorting.map(({ field: f, order: o }) => ({ field: f, order: o })), { field, order }], 'sorting');
};
ACT.stackschedsortoff = (button) => {
  const sorting = scheduleOf().sorting.filter((_, at) => at !== Number(button.dataset.a));
  scheduleSet('schedule.sorting', 'sorting', sorting.map(({ field, order }) => ({ field, order })), 'sorting');
};
ACT.stackschedsortflip = (button) => {
  const sorting = scheduleOf().sorting.map(({ field, order }, at) => ({
    field,
    order: at === Number(button.dataset.a) ? (order === 'descending' ? 'ascending' : 'descending') : order,
  }));
  scheduleSet('schedule.sorting', 'sorting', sorting, 'sorting');
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.models({ id: 'schedules', order: 60, open: () => loadSchedules() });
register.logs({ id: 'schedules', order: 30, entry: (entry) => schedulesChanged(entry) });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get modelSchedules() { return modelSchedules; }, get renderSchedule() { return renderSchedule; },
});
