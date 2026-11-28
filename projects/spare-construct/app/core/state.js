// The core's state: the connection's shared state (C), constants and small helpers, and the calls to the server's API.

import { entries } from './registry.js';

const SNAP_MM = 50;
const SEND_EVERY_MS = 40;
const NOT_YET = 'That isn’t connected to the server yet.';
// The demo's commands that still mean something here. Anything else would change sample data.
const WORKS = new Set([
  'go', 'view', 'vtab', 'vclose', 'vcloseothers', 'browser', 'uipal', 'dockreset', 'pref', 'zoom',
  'tool', 'dtool', 'seltool', 'deselect', 'closeAll', 'dgclose', 'discmenu', 'account', 'vbmenu',
  'detailset', 'disc', 'warnlist',
]);

const C = {
  me: null,
  hub: null,
  people: null,
  projects: [],
  project: null,
  data: null, // the project as the server listed it
  model: null, // the open model's row
  team: null,
  readOnly: false,
  level: 0, // the level the pieces in hand are from
  elements: new Map(), // id -> { el, shape, failed, hash, seq }
  claims: new Map(),
  links: [],
  linked: new Map(), // live-linked model id -> Map of elements, as above
  fixed: new Map(), // link id -> elements of a package
  models: new Map(), // id -> a model's row, for the models this person can see
  client: crypto.randomUUID(),
  counter: 0,
  head: 0,
  pending: [],
  socket: null,
  connected: false,
  watching: new Set(),
  arriving: [], // other people's edits, until they are drawn
  latencies: [], // how long those took, from leaving their browser to drawn in this one
  journeys: [], // the same, split: to the server, on to this page, then onto the screen
  log: [],
  three: null,
  builders: new Map(), // package -> its shape builder once loaded, or null if this browser can't
  builder: null, // the open model's shape builder, for the seats the page asks of it
  viewing: false, // this browser can't build the open model, so it draws what's stored
  cuts: false, // the open model's package cuts what is hosted in a wall, as the server builds it
  checked: 0, // bodies compared with the server's
  mismatches: [], // and any that differed
  applying: new Map(), // document -> the entries (by seq) this page is still applying: loads, hosts built again
  waitingCheckpoints: [], // checkpoints that wait for those entries, in the order they came
  steps: { undo: null, redo: null }, // what this person can undo and redo in the open model
  views: null, // the open model's views as the server lists them, by name; null until they come
  openView: null, // the ID of the model's view whose plan the server draws in the plan pane
  drawn: null, // the server's drawing of that view: { view, svg, skipped } or { view, error }
};
window.spare = C; // the browser test reads this

// What this page was doing, kept so a slow edit says where its time went. Diagnostics only: they
// read the clock and change nothing the page does.
C.longTasks = []; // stretches the main thread was held for 50 ms or more: { at, ms }
C.doing = []; // named stretches of this page's own work, 2 ms or more: { what, at, ms }
C.asking = []; // what it asked the server for: { path, at, ms }
try {
  new PerformanceObserver((list) => {
    for (const task of list.getEntries()) {
      C.longTasks.push({ at: Math.round(performance.timeOrigin + task.startTime), ms: Math.round(task.duration) });
    }
  }).observe({ type: 'longtask', buffered: true });
} catch {
  // A browser without long-task reports just has none.
}
/** Runs `work` and notes how long it held the page, if it did for 2 ms or more. */
function spent(what, work) {
  const at = Date.now();
  try {
    return work();
  } finally {
    const ms = Date.now() - at;
    if (ms >= 2) C.doing.push({ what, at, ms });
  }
}
/** What the page was doing, and waiting on the server for, between two clock readings. */
C.busyIn = (from, to) => ({
  longTasks: C.longTasks.filter((t) => t.at + t.ms >= from && t.at <= to),
  doing: C.doing.filter((d) => d.at + d.ms >= from && d.at <= to),
  asking: C.asking.filter((a) => a.at + a.ms >= from && a.at <= to),
});

const named = (person) => person[0].toUpperCase() + person.slice(1);
// The level the plan shows and a new element is tied to: one chosen in Levels, or the floor plan's own.
const levelOf = () => C.planLevel?.id ?? (S.view === 'l2' ? '2' : '1');
const planName = () =>
  C.views?.find((view) => view.id === C.openView)?.name ?? (C.planLevel ? `${C.planLevel.name} plan` : VIEWS[S.view]?.name);
/** The ID of the level a record stands on: its tie, or the level its old integer names (construct-levels' FORMAT.md). */
const levelIdOf = (record) =>
  record.props?.['@level'] ?? (record.level > 0 ? String(record.level) : record.level < 0 ? `m${-record.level}` : null);
const say = (text) => toast(esc(text));
const initials = (name) => name.slice(0, 2).toUpperCase();

async function api(path, options) {
  const asked = Date.now();
  const response = await fetch(path, options);
  const body = await response.json().catch(() => ({}));
  // The server's ID of the request, which its own lines for it carry (a report asks for them).
  const request = response.headers.get('x-request-id') || undefined;
  C.asking.push({ path: String(path).split('/').slice(-2).join('/'), at: asked, ms: Date.now() - asked, request, status: response.status });
  if (!response.ok) {
    const error = new Error(body.error || response.statusText);
    error.status = response.status;
    error.request = request;
    // "Something went wrong": every failures entry is told (register.failures).
    if (response.status >= 500) for (const hook of entries('failures')) hook.failure({ code: 'server.failed', words: error.message, request });
    throw error;
  }
  return body;
}

const post = (path, body) =>
  api(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });

const modelUrl = (model) => `/api/projects/${C.project}/models/${model}`;
const fileUrl = (team, folder, id) =>
  `/api/projects/${C.project}/teams/${team}/folders/${folder}/packages/${id}/file`;

export {
  C, NOT_YET, SEND_EVERY_MS, SNAP_MM, WORKS, api, fileUrl, initials, levelIdOf, levelOf, modelUrl,
  named, planName, post, say, spent,
};
