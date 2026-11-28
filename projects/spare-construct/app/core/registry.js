// The registry: what the tools, the disciplines and the packs plug into the core with (note
// A96-tool-architecture.md). It has the Transport page's shape (demo/transport/js/registry.js):
// one list per kind, a definition checked when it is registered, entries in `order` and then in the
// order they were registered, and country or content packs that register between beginPack and
// endPack. It is an ES module, and it never touches the page: a tool registers when its file loads,
// the core reads the lists when it draws, picks, shows a palette or builds the ribbon.
//
// The core asks a kind's entries in order and, for a question one entry answers, takes the first
// answer that is not `undefined`; an entry that has nothing to say returns `undefined`. Each kind
// below says where the core asks it. A tool's own clicks and changes keep the demo's tables (ACT and
// CHG, by name), which the ribbon and the palettes already call through.

// Field types, as Transport's: s text, n number, f function, b true or false, a list.
const KINDS = {
  // A discipline: its ribbon and its tools (the shared ribbon's rule, one ribbon per discipline).
  disciplines: { req: { id: 's', label: 's' }, opt: { order: 'n' } },
  // A tool: what it is for the plan's `S.tool`, its discipline, the commands it sends up the log,
  // the label of its section of the two-browser test, and its click in the plan while it is the tool.
  tools: { req: { id: 's', discipline: 's' }, opt: { order: 'n', place: 'f', commands: 'a', test: 's' } },
  // The ribbon host: each adds its buttons to the demo's ribbon config. Under 1000, before the core's
  // shared buttons (Link, Share, the quick access bar, Manage, a selection's Modify tab); 1000 and over,
  // after them (a contextual tab of its own selection).
  ribbons: { req: { id: 's', ribbon: 'f' }, opt: { order: 'n' } },
  // The line under the ribbon while a tool is in use: the first answer.
  hints: { req: { id: 's', hint: 'f' }, opt: { order: 'n' } },
  // The side panel: a tool's part, for the selected element or for itself while in use. The first
  // answer is shown; one marked `always` is asked even after an earlier answer, as the page always has.
  palettes: { req: { id: 's', palette: 'f' }, opt: { order: 'n', always: 'b' } },
  // Rows a tool adds under the core's own rows for a selected element of its kind (every answer).
  properties: { req: { id: 's', properties: 'f' }, opt: { order: 'n' } },
  // The side panel for a selection that is not an element (a dimension, a grid): the first answer.
  selections: { req: { id: 's', selection: 'f' }, opt: { order: 'n' } },
  // A kind of element: which elements are its own (`owns`), how the plan draws one (`inPlan`, and
  // `unbuilt` to draw it before it has a shape), and how it is picked and dragged (`pickOnly`: picked,
  // never claimed by a drag; `drags(el, hosted)`: false keeps it still; `move`: the command a hosted one
  // is moved along its host with).
  elements: {
    req: { id: 's', owns: 'f' },
    opt: { order: 'n', inPlan: 'f', unbuilt: 'b', pickOnly: 'b', drags: 'f', move: 's' },
  },
  // How the plan stacks the elements: the first answer for an element, else walls over the rest.
  drawOrders: { req: { id: 's', drawOrder: 'f' }, opt: { order: 'n' } },
  // The plan's layers: `before` runs before anything is drawn; `under(scale)` is drawn under the
  // elements, `over(scale)` over them, and `note()` above the drawing.
  layers: { req: { id: 's' }, opt: { order: 'n', before: 'f', under: 'f', over: 'f', note: 'f' } },
  // The open model: `forget()` when it closes, `open(model)` (awaited, in order) when it opens.
  models: { req: { id: 's' }, opt: { order: 'n', open: 'f', forget: 'f' } },
  // An entry of the log for the open model, after the elements have taken it.
  logs: { req: { id: 's', entry: 'f' }, opt: { order: 'n' } },
  // A dialog of the Manage tab, by the name it is opened with.
  dialogs: { req: { id: 's', html: 'f' }, opt: { order: 'n' } },
};

const TYPE_OK = {
  s: (v) => typeof v === 'string' && v.length > 0,
  n: (v) => typeof v === 'number' && Number.isFinite(v),
  f: (v) => typeof v === 'function',
  b: (v) => typeof v === 'boolean',
  a: (v) => Array.isArray(v),
};
const TYPE_NAME = { s: 'a non-empty string', n: 'a number', f: 'a function', b: 'true or false', a: 'a list' };

const lists = {};
for (const kind of Object.keys(KINDS)) lists[kind] = [];
const sorted = new Map(); // kind and pack -> the kind's entries in order, until one is added
let seq = 0;

function check(kind, def) {
  const K = KINDS[kind];
  if (!def || typeof def !== 'object') throw new TypeError(`register.${kind}: pass an object, got ${def}`);
  const who = `register.${kind}(${JSON.stringify(def.id ?? '?')})`;
  for (const [field, type] of Object.entries(K.req)) {
    if (!TYPE_OK[type](def[field])) throw new TypeError(`${who}: '${field}' must be ${TYPE_NAME[type]}`);
  }
  for (const [field, type] of Object.entries(K.opt)) {
    if (def[field] !== undefined && !TYPE_OK[type](def[field])) throw new TypeError(`${who}: '${field}' must be ${TYPE_NAME[type]}`);
  }
  for (const field of Object.keys(def)) {
    if (!(field in K.req) && !(field in K.opt) && field !== 'pack') throw new TypeError(`${who}: no field '${field}' in this kind`);
  }
  if (!/^[a-z][\w-]*$/i.test(def.id)) throw new TypeError(`${who}: 'id' may only hold letters, digits, '-' and '_'`);
}

/* Country and content packs (app/packs/README.md), as Transport's: a pack's files load between
   beginPack and endPack, and everything they register carries the pack's id and takes part only on a
   project of that pack. A pack's entry of the same id as a core one replaces it on that pack's
   projects; the core one stays everywhere else. */
const packs = [];
let loading = null;
let packOf = () => null;

function beginPack(def) {
  if (!def || !/^[a-z][a-z0-9-]*$/.test(def.id || '') || typeof def.name !== 'string' || !def.name) {
    throw new TypeError('beginPack: give an id (a lower-case code such as nz) and a name');
  }
  if (loading) throw new TypeError(`beginPack(${def.id}): pack '${loading}' is still loading; end it with endPack() first`);
  const i = packs.findIndex((p) => p.id === def.id);
  if (i >= 0) packs[i] = { ...def };
  else packs.push({ ...def });
  loading = def.id;
  sorted.clear();
  return def;
}

function endPack() {
  loading = null;
}

/** How the core says which pack the open project is of: a function giving a pack's id, or null. */
function usePack(fn) {
  packOf = fn;
  sorted.clear();
}

/** The open project's pack: the one it names when installed, else the first installed, else none. */
function country() {
  const id = packOf();
  return packs.find((p) => p.id === id) || packs[0] || null;
}

function add(kind, def) {
  check(kind, def);
  const pack = def.pack || loading || '';
  const L = lists[kind];
  const i = L.findIndex((d) => d.id === def.id && (d.pack || '') === pack);
  const entry = { order: 1000, ...def, seq: i >= 0 ? L[i].seq : seq++ };
  if (pack) entry.pack = pack;
  if (i >= 0) L[i] = entry;
  else L.push(entry);
  sorted.clear();
  return entry;
}

/** A kind's entries for the open project, in order: the core's, with its pack's in place of those it replaces. */
function entries(kind) {
  if (!lists[kind]) throw new TypeError(`entries: no kind '${kind}'. Kinds: ${Object.keys(KINDS).join(', ')}`);
  const on = country()?.id || '';
  const key = `${kind}/${on}`;
  let out = sorted.get(key);
  if (!out) {
    const mine = new Set(lists[kind].filter((d) => d.pack && d.pack === on).map((d) => d.id));
    out = lists[kind].filter((d) => (d.pack ? d.pack === on : !mine.has(d.id))).sort((a, b) => a.order - b.order || a.seq - b.seq);
    sorted.set(key, out);
  }
  return out;
}

/** A kind's entry of this id for the open project, or null. */
const registered = (kind, id) => entries(kind).find((d) => d.id === id) || null;

/** The first answer of a kind's entries that is not undefined, asking each in order (`ask(entry)`). */
function firstAnswer(kind, ask) {
  for (const entry of entries(kind)) {
    const answer = ask(entry);
    if (answer !== undefined) return answer;
  }
  return undefined;
}

const register = {};
for (const kind of Object.keys(KINDS)) register[kind] = (def) => add(kind, def);

export { KINDS, beginPack, country, endPack, entries, firstAnswer, register, registered, usePack };
