// Opening a model or a package.

import { late } from './late.js';
import { entries } from './registry.js';
import { C, api, fileUrl, levelIdOf, levelOf, say } from './state.js';
import { builderFor, fromStored, loadClaims, loadLinks, loadPieces } from './shapes.js';
import { send } from './channel.js';

// --- Opening a model or a package ---

ACT.stackmodel = async (button) => {
  const team = C.data.teams.find((t) => t.id === button.dataset.team);
  const folder = team.folders.find((f) => f.kind === 'wip');
  await openModel(team.id, folder.models.find((m) => m.id === button.dataset.a));
  showPlan();
};

ACT.stackpackage = async (button) => {
  const { a: id, team, folder } = button.dataset;
  await openPackage(team, folder, id);
  showPlan();
};

function showPlan() {
  if (S.screen !== 'plan' && S.screen !== '3d') go(`plan:${S.view}`);
  else refresh();
}

function forgetTheOpenModel() {
  if (C.model) {
    send({ t: 'stop', document: C.model.id });
    C.watching.delete(C.model.id);
  }
  for (const id of C.linked.keys()) {
    send({ t: 'stop', document: id });
    C.watching.delete(id);
  }
  C.linked.clear();
  C.fixed.clear();
  C.elements.clear();
  C.claims.clear();
  C.links = [];
  C.steps = { undo: null, redo: null };
  C.params = C.typeList = C.manage = C.wallTypes = null;
  C.views = C.openView = C.drawn = null;
  C.warnings = null;
  C.warnSeq += 1;
  clearTimeout(C.warnTimer);
  C.schedules = C.sched = C.scheduleCategories = null;
  clearTimeout(C.schedulesTimer);
  C.problems.clear();
  C.asked.clear();
  C.holds.clear();
  C.cuts = false;
  C.builder = null;
  for (const entry of entries('models')) if (entry.forget) entry.forget(); // what each tool keeps of the model (register.models)
  S.sel = null;
  late.WS.elements.clear();
  late.WS.loaded = false;
  late.workshopChanged({});
}

async function openModel(team, model) {
  const builder = await builderFor(model);
  forgetTheOpenModel();
  C.model = model;
  C.team = team;
  C.viewing = !builder;
  C.builder = builder;
  C.cuts = Boolean(builder) && typeof builder.shapes.build_hosting === 'function';
  C.readOnly = C.viewing;
  C.watching.add(model.id);
  send({ t: 'watch', document: model.id, since: C.head });
  await loadPieces();
  await loadClaims();
  await loadLinks();
  for (const entry of entries('models')) if (entry.open) await entry.open(model); // in order, each awaited (register.models)
}

async function openPackage(team, folder, id) {
  forgetTheOpenModel();
  C.model = null;
  C.readOnly = true;
  C.viewing = false;
  const file = await api(fileUrl(team, folder, id));
  showPackage(file);
  C.file = file;
  for (const piece of file.elements) {
    if (late.isWorkshop(piece.element)) late.WS.elements.set(piece.element.id, { el: piece.element, hash: null, seq: 0 });
  }
  late.WS.loaded = true;
  late.workshopChanged({});
  say(`A fixed version: ${file.elements.length} elements.`);
}

/** A package carries finished shapes, so it is drawn as it came and never rebuilt. */
const packaged = (piece) => ({ el: piece.element, ...fromStored(piece), seq: 0 });

function showPackage(file) {
  for (const piece of file.elements) {
    if (levelIdOf(piece.element) === levelOf()) C.elements.set(piece.element.id, packaged(piece));
  }
}

export { openModel, packaged, showPackage };
