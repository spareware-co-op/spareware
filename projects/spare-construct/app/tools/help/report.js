// Tool: Help's Report a problem and Send feedback (piece A100; note A100-report-a-problem.md).
//
// A report is shown to the person whole before anything leaves the page: each part has its own box,
// ticked, and a look at exactly what it holds, and a part left unticked is not sent. The page adds
// what it knows (the failure as shown, the browser, the page's own error lines, a screenshot the
// person chooses); the server adds, as a draft shown on the same screen, what only it knows: the
// elements the failure is about and those they read, every edit of the log that touched them with
// each author as a stand-in, the program's version and engine, and its own lines for this person's
// requests. A failure the core tells this tool of (register.failures: a refusal, "Something went
// wrong", a body that differs) offers "Send a report" beside its toast, on the spot. Sent, a report
// is numbered and its state (received, reproduced, fixed in a version) is in Your reports.

import { register } from '../../core/registry.js';
import { C, api, modelUrl, post, say } from '../../core/state.js';

const MOST_ERRORS = 50;
const LINE_CHARS = 500;
const SUMMARY_CHARS = 160;
const SHOT_BYTES = 1024 * 1024;
// construct-report's topics, in its order, by the names the page shows.
const TOPICS = [
  ['modelling', 'Drawing and changing the model'],
  ['views', 'Views and sheets'],
  ['data', 'Schedules, parameters and types'],
  ['sharing', 'Working with others'],
  ['speed', 'Speed'],
  ['words', 'The look and the words'],
  ['other', 'Something else'],
];

C.reportDraft = null; // the report on screen, every part in, as it would be sent whole
C.reportFailure = null; // the last failure the person was shown
const pageErrors = []; // this page's own error lines, newest last
let sending = false;

// --- The page's own error lines ---

function noteError(line) {
  pageErrors.push(String(line).replace(/\s+/g, ' ').slice(0, LINE_CHARS));
  if (pageErrors.length > MOST_ERRORS) pageErrors.shift();
}

window.addEventListener('error', (event) => noteError(`${event.message} (${String(event.filename || '').split('/').pop()}:${event.lineno})`));
window.addEventListener('unhandledrejection', (event) => noteError(`Not handled: ${event.reason?.message ?? event.reason}`));
const consoleError = console.error.bind(console);
console.error = (...args) => {
  noteError(args.map(String).join(' '));
  consoleError(...args);
};

// --- Send a report, on the spot ---

/** Offers a report beside the toast that shows the failure, for as long as the toast shows it. */
function offer(failure) {
  if (sending) return;
  C.reportFailure = { ...failure, at: Date.now() };
  const words = $('#toast-m');
  if (!words) return;
  if (words.textContent !== failure.words || $('#toast').hidden) say(failure.words);
  let button = $('#toast [data-act="stackreport"]');
  if (!button) {
    button = document.createElement('button');
    button.className = 'btn sm';
    button.dataset.act = 'stackreport';
    button.textContent = 'Send a report';
    words.after(button);
  }
  button.dataset.words = failure.words;
}

// The toast moves on to other words: its offer goes with the failure it was for.
new MutationObserver(() => {
  const button = $('#toast [data-act="stackreport"]');
  if (button && $('#toast-m')?.textContent !== button.dataset.words) button.remove();
}).observe($('#toast-m') || document.body, { childList: true, characterData: true, subtree: true });

// --- The report, shown whole before it is sent ---

const PARTS = [
  ['failure', 'What went wrong'],
  ['elements', 'The elements it is about, with their recipes and inputs'],
  ['edits', 'The edits that touched them, to replay'],
  ['program', 'The program’s version and engine'],
  ['browser', 'Your browser'],
  ['errors', 'This page’s error lines'],
  ['server', 'The server’s lines for your requests'],
];

/** What one part holds, as it would be sent, or null when there is none of it. */
function partOf(report, part) {
  switch (part) {
    case 'failure': return report.failure ?? null;
    case 'elements': return report.elements?.length ? report.elements : null;
    case 'edits': return report.edits?.length ? report.edits : null;
    case 'program': return report.program ?? null;
    case 'browser': return report.browser ?? null;
    case 'errors': return report.page_errors?.length ? report.page_errors : null;
    case 'server': return report.server_log?.length ? report.server_log : null;
    default: return null;
  }
}

function partWords(report, part) {
  const n = (list) => list.length;
  switch (part) {
    case 'failure': return `${report.failure.code}: ${report.failure.words}`;
    case 'elements': return `${n(report.elements)}: ${report.elements.map((p) => p.id).join(', ')}`;
    case 'edits': return `${n(report.edits)}, the last ${report.edits.at(-1).cmd}`;
    case 'program': return `version ${report.program.version}, ${report.program.engine}`;
    case 'browser': return report.browser;
    case 'errors': return `${n(report.page_errors)} lines`;
    case 'server': return `${n(report.server_log)} lines`;
    default: return '';
  }
}

/** Every request whose lines the server may add: the failure's, the live channel's, and those that failed. */
function requestsOf(failure) {
  const asked = C.asking.filter((a) => a.request && a.status >= 400).map((a) => a.request);
  return [...new Set([failure?.request, C.liveRequest, ...asked.slice(-20)].filter(Boolean))];
}

async function draftOf(failure) {
  if (!C.model) return null;
  const element = failure?.element ?? (failure ? null : S.sel);
  const ask = {
    elements: element ? [element] : [],
    client: failure?.edit ? C.client : undefined,
    counter: failure?.edit?.counter,
    requests: requestsOf(failure),
  };
  return post(`${modelUrl(C.model.id)}/report-draft`, ask);
}

function partRow(report, [part, label]) {
  const held = partOf(report, part);
  if (held === null) return '';
  const look = esc(JSON.stringify(held, null, 2));
  return `<div class="report-part"><label class="chk"><input type="checkbox" data-report-part="${part}" checked>` +
    `<span><b>${esc(label)}</b>: ${esc(partWords(report, part))}</span></label>` +
    `<details><summary class="small">What is sent</summary><pre class="mono small report-look">${look}</pre></details></div>`;
}

function shotRow() {
  return '<div class="report-part"><label class="small">A screenshot, if you want to add one (PNG or JPEG, up to 1 MB). It shows whatever is on it.' +
    '<input type="file" accept="image/png,image/jpeg" data-report-shot></label></div>';
}

/** Opens the report of a problem: the failure if there is one, else the selection, with the server's draft. */
async function openReport(failure) {
  const shown = failure ?? (C.reportFailure && Date.now() - C.reportFailure.at < 10 * 60 * 1000 ? C.reportFailure : null);
  openDlg('Report a problem', '<p class="small faint" data-report-loading>Gathering what the report would hold.</p>', '');
  let draft = null;
  try {
    draft = await draftOf(shown);
  } catch (error) {
    noteError(`The draft was not made: ${error.message}`);
  }
  const report = {
    format: 1,
    kind: 'problem',
    summary: '',
    words: '',
    ...(draft?.report ?? {}),
  };
  report.failure = draft?.report?.failure ?? (shown ? { code: shown.code, words: shown.words, element: shown.element ?? undefined, request: shown.request ?? undefined } : undefined);
  report.browser = navigator.userAgent.slice(0, 300);
  report.page_errors = [...pageErrors];
  C.reportDraft = report;
  const summary = report.failure ? `Turned back: ${report.failure.words}` : '';
  const left = draft?.left_out ? `<p class="small faint">${draft.left_out} older edits are over the cap and are left out.</p>` : '';
  const body =
    `<label class="small">One line on what went wrong<input class="inp" data-report-summary maxlength="${SUMMARY_CHARS}" value="${esc(summary.slice(0, SUMMARY_CHARS))}"></label>` +
    '<label class="small mt">What were you doing? (Please leave out other people’s names.)<textarea class="inp" rows="3" data-report-words></textarea></label>' +
    '<p class="small mt">This is everything the report sends, to the co-op’s private reports store, never a public page. Untick what you would rather keep back.</p>' +
    PARTS.map((part) => partRow(report, part)).join('') + shotRow() + left +
    '<div data-report-answer></div>';
  const foot = '<button class="btn" data-act="dgclose">Cancel</button><button class="btn pri" data-act="stackreportsend">Send</button>';
  openDlg('Report a problem', body, foot);
}

/** Opens Send feedback: a topic, a line and the person's words, and the browser if they leave it in. */
function openFeedback() {
  const report = { format: 1, kind: 'feedback', summary: '', words: '', browser: navigator.userAgent.slice(0, 300) };
  C.reportDraft = report;
  const topics = TOPICS.map(([id, label]) => `<option value="${id}">${esc(label)}</option>`).join('');
  const body =
    `<label class="small">What it is about<select class="inp" data-report-topic>${topics}</select></label>` +
    `<label class="small mt">Your idea in one line<input class="inp" data-report-summary maxlength="${SUMMARY_CHARS}"></label>` +
    '<label class="small mt">Say more, if you like (please leave out other people’s names)<textarea class="inp" rows="4" data-report-words></textarea></label>' +
    '<p class="small mt">This is everything it sends, to the co-op’s private reports store. Untick what you would rather keep back.</p>' +
    partRow(report, ['browser', 'Your browser']) + shotRow() + '<div data-report-answer></div>';
  const foot = '<button class="btn" data-act="dgclose">Cancel</button><button class="btn pri" data-act="stackreportsend">Send</button>';
  openDlg('Send feedback', body, foot);
}

function readShot(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(',')[1] || '');
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

/** The report as the person left it: only the parts still ticked. */
async function asSent() {
  const draft = C.reportDraft;
  const kept = (part) => $(`#dlg input[data-report-part="${part}"]`)?.checked === true;
  const report = {
    format: 1,
    kind: draft.kind,
    summary: ($('#dlg [data-report-summary]')?.value || '').trim(),
    words: $('#dlg [data-report-words]')?.value || '',
  };
  if (draft.kind === 'feedback') report.topic = $('#dlg [data-report-topic]')?.value;
  if (kept('failure')) report.failure = draft.failure;
  if (kept('elements')) report.elements = draft.elements;
  if (kept('edits')) report.edits = draft.edits;
  if (report.elements || report.edits) report.where = draft.where;
  if (kept('program')) report.program = draft.program;
  if (kept('browser')) report.browser = draft.browser;
  if (kept('errors')) report.page_errors = draft.page_errors;
  if (kept('server')) report.server_log = draft.server_log;
  const file = $('#dlg input[data-report-shot]')?.files?.[0];
  if (file) {
    if (file.size > SHOT_BYTES) throw new Error('The screenshot is larger than 1 MB. Choose a smaller one.');
    report.screenshot = { media: file.type, data: await readShot(file) };
  }
  return report;
}

async function sendReport() {
  const answer = $('#dlg [data-report-answer]');
  if (!C.reportDraft || !answer || sending) return;
  sending = true;
  try {
    const listed = await post('/api/reports', await asSent());
    answer.innerHTML =
      `<p class="note small mt" data-report-number="${listed.number}">Sent as report ${listed.number}. ` +
      `<span data-report-state="${esc(String(listed.state?.fixed ? 'fixed' : listed.state))}">${esc(listed.shown)}</span>. Your reports in Help shows where it stands.</p>`;
    $('#dlg-f').innerHTML = '<button class="btn pri" data-act="dgclose">Close</button>';
    C.reportFailure = null;
  } catch (error) {
    answer.innerHTML = `<p class="note bad small mt" role="alert" data-report-refused>${esc(error.message)}</p>`;
  } finally {
    sending = false;
  }
}

async function openMine() {
  let mine = [];
  try {
    mine = await api('/api/reports');
  } catch (error) {
    say(error.message);
    return;
  }
  const rows = mine.length
    ? mine.map((r) => `<li data-report="${r.number}"><span class="grow"><b>Report ${r.number}</b>: ${esc(r.summary)}<br><span class="small muted">${esc(r.code || r.kind)}</span></span><span class="st info">${esc(r.shown)}</span></li>`).join('')
    : '<li class="small faint">You have sent no reports.</li>';
  openDlg('Your reports', `<ul class="list">${rows}</ul>`, '<button class="btn pri" data-act="dgclose">Close</button>');
}

ACT.stackreport = () => openReport(C.reportFailure);
ACT.stackreportnew = () => openReport(null);
ACT.stackfeedback = () => openFeedback();
ACT.stackreports = () => openMine();
ACT.stackreportsend = () => sendReport();

// A look at a part may be long: it scrolls in its own box. The page's own tokens.
const style = document.createElement('style');
style.textContent =
  '.report-part{margin-top:calc(var(--space) * 1.5)}' +
  '.report-look{max-height:12rem;overflow:auto;margin:4px 0 0;padding:6px;background:var(--panel-alt);border:1px solid var(--hairline);white-space:pre-wrap;word-break:break-all}';
document.head.append(style);

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.ribbons({
  id: 'help',
  order: 990,
  ribbon: (config) => {
    config.tabs.push({
      id: 'help',
      label: 'Help',
      panels: [
        {
          label: 'Support',
          items: [
            { label: 'Report a problem', icon: 'warn', act: 'stackreportnew', tip: 'Send the co-op a report of what went wrong, with what it needs to find it. You see all of it first.' },
            { label: 'Send feedback', icon: 'help', act: 'stackfeedback', tip: 'Send an idea or a wish, in your words.' },
            { label: 'Your reports', icon: 'table', act: 'stackreports', tip: 'The reports you have sent, and where each stands.' },
          ],
        },
      ],
    });
  },
});
register.failures({ id: 'report', order: 10, failure: (failure) => offer(failure) });
