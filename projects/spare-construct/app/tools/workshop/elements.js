// Tool: Workshop's elements.

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, modelUrl } from '../../core/state.js';
import { edit, watchSince } from '../../core/channel.js';

// --- Workshop's elements ---
// Workshop, a page of its own in a frame, keeps its parts and its assembly as elements of the open
// model, through the same edit log as walls and doors (construct-model's FORMAT.md, "Workshop's
// elements"). They have no place in the plan, so this finds them by kind, keeps them live, and lets
// the page read and change them through `spare.workshop`.

const WORKSHOP_KINDS = ['workshop.part', 'workshop.assembly'];
const isWorkshop = (record) => WORKSHOP_KINDS.includes(record?.kind);
const WS = { loaded: false, elements: new Map(), listeners: new Set() }; // id -> { el, hash, seq }

/** Tells the page. `why.turnedBack` says why an edit of its own was refused. */
function workshopChanged(why) {
  for (const listener of WS.listeners) {
    try {
      listener(why);
    } catch (error) {
      console.error(error);
    }
  }
}

async function loadWorkshop() {
  const model = C.model?.id;
  WS.elements.clear();
  if (model) {
    for (const kind of WORKSHOP_KINDS) {
      const seen = await api(`${modelUrl(model)}/kind/${kind}`);
      if (seen.through < C.head) watchSince(model, seen.through);
      await Promise.all(
        Object.entries(seen.pieces).map(async ([id, hash]) => {
          const el = await api(`${modelUrl(model)}/record/${hash}`);
          if (C.model?.id === model) keepWorkshop(id, el, hash, seen.through, true);
        }),
      );
    }
    if (C.model?.id !== model) return;
  }
  WS.loaded = true;
  workshopChanged({});
}

/** A record arrived. The page hears of it unless it came from its own edit, which it already shows. */
function keepWorkshop(id, el, hash, seq, quiet) {
  const known = WS.elements.get(id);
  if (known && known.seq > seq) return;
  const stillMine = C.pending.some((edit) => edit.args.element === id);
  WS.elements.set(id, { el: quiet && stillMine && known ? known.el : el, hash, seq });
  if (!quiet) workshopChanged({});
}

function workshopEntry(entry, change, mine) {
  if (!change.after) {
    WS.elements.delete(change.piece);
    if (!mine) workshopChanged({});
    return;
  }
  api(`${modelUrl(entry.target)}/record/${change.after}`).then((el) => {
    if (entry.target === C.model?.id) keepWorkshop(change.piece, el, change.after, entry.seq, mine);
  });
}

/** The server said no to one of the page's edits: back to the last confirmed version. */
async function workshopTurnBack(edit, reason) {
  const id = edit.args.element;
  const known = WS.elements.get(id);
  if (known?.hash && C.model) known.el = await api(`${modelUrl(C.model.id)}/record/${known.hash}`);
  else if (known && !known.hash) WS.elements.delete(id);
  workshopChanged({ turnedBack: reason });
}

/** Workshop's way in: what is open, its elements, and the three commands it uses. */
C.workshop = {
  state: () => ({
    loaded: WS.loaded,
    model: C.model ? { id: C.model.id, name: C.model.name } : null,
    fixed: !C.model && C.readOnly && WS.loaded, // a package: a fixed version
    editable: Boolean(C.model) && !C.readOnly,
  }),
  elements: () => [...WS.elements.values()].map(({ el }) => el),
  create(kind, element, name, recipe) {
    const args = { element, kind, name, level: 0, placement: { x: 0, y: 0, z: 0 }, recipe, props: { category: 'Workshop' } };
    WS.elements.set(element, { el: { id: element, ...args, references: [] }, hash: null, seq: C.head });
    edit('element.create', args);
  },
  set(element, name, recipe) {
    const known = WS.elements.get(element);
    if (known) known.el = { ...known.el, name, recipe };
    edit('element.set', { element, name, recipe });
  },
  remove(element) {
    WS.elements.delete(element);
    edit('element.delete', { element });
  },
  subscribe(listener) {
    WS.listeners.add(listener);
    return () => WS.listeners.delete(listener);
  },
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.disciplines({ id: 'workshop', label: 'Workshop', order: 20 });
register.models({ id: 'workshop', order: 20, open: () => loadWorkshop() });

// What files loaded before this one use from it, bound once it has loaded.
provide({
  get WS() { return WS; }, get isWorkshop() { return isWorkshop; },
  get keepWorkshop() { return keepWorkshop; }, get workshopChanged() { return workshopChanged; },
  get workshopEntry() { return workshopEntry; },
  get workshopTurnBack() { return workshopTurnBack; },
});
