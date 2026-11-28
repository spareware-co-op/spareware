// The ribbon host: the demo's frame, with the sample taken out of what is connected.

import { late } from './late.js';
import { entries } from './registry.js';
import { C, NOT_YET, WORKS, planName, say } from './state.js';
import { drawBrowser } from './browser.js';
import { letGo } from './channel.js';
import { stepWords } from './undo.js';
import { clearProblems } from './problems.js';
import { drawPlan, plan } from './plan.js';
import { threeD } from './viewport3d.js';

// --- The demo's frame, with the sample taken out of what's connected ---

function takeOverTheFrame() {
  VIEWS.l1 = { name: 'Level 1 plan', kind: 'Floor plan', sc: '1:100' };
  VIEWS.l2 = { name: 'Level 2 plan', kind: 'Floor plan', sc: '1:100' };
  VB.l1 = [-2500, -2500, 17000, 13000];
  VB.l2 = VB.l1;
  FLOORV.splice(0, FLOORV.length, 'l1', 'l2');
  HOMEV.architecture = 'l1';
  // The disciplines that registered are connected (register.disciplines): Architecture to the
  // project, Workshop building its parts with the geometry kernel in the browser.
  for (const discipline of DISCS) discipline[4] = entries('disciplines').some((entry) => entry.id === discipline[0]) ? 1 : 0;
  renderDisc();
  for (const side of $$('#props-host .side')) side.remove();
  S.view = 'l1';
  S.sel = null;
  OPEN.splice(0, OPEN.length, curView());
  history.replaceState(null, '', `#${curView()}`);
  S.units = 'mm';
  BOPEN.clear();
  for (const open of ['Projects', 'Views', 'Warnings', 'Schedules', 'Project files', 'Linked models']) BOPEN.add(open);

  document.addEventListener(
    'click',
    (event) => {
      const button = event.target.closest('[data-act]');
      const act = button?.dataset.act;
      if (act && !WORKS.has(act) && !act.startsWith('stack')) {
        event.stopPropagation();
        event.preventDefault();
        say(NOT_YET);
      }
    },
    true,
  );
  openPalette = () => say(NOT_YET);
  headerView = () => {
    const name = S.screen === 'plan' ? planName() : viewName(S.screen);
    $('#h-view').textContent = name || '';
    document.title = `Spare Construct: ${name || ''}`;
  };
  statusBar = () => {
    const last = C.latencies.at(-1);
    const waiting = C.pending.length ? `${C.pending.length} waiting · ` : '';
    const checks = C.checked ? `${C.checked} shapes checked · ` : '';
    const speed = last === undefined ? '' : `Last edit from someone else: ${last} ms · `;
    $('#status-r').innerHTML = `${late.warningsCount()}<span id="st-log" class="mono">${esc(waiting + checks + speed)}Edit ${C.head}</span>`;
  };
  renderBrowser = drawBrowser;

  const sampleRibbon = ribbonCfg;
  ribbonCfg = () => connectRibbon(sampleRibbon());

  const demoSelect = select;
  select = (id) => {
    if (S.sel && S.sel !== id) letGo(S.sel);
    clearProblems(/^(param:|element-type)/);
    demoSelect(id);
  };

  const demoView = ACT.view;
  ACT.view = (button) => {
    if (FLOORV.includes(button.dataset.a)) {
      C.planLevel = null;
      late.leaveView();
      demoView(button);
    } else say(NOT_YET);
  };
  ACT.account = account;
  // The demo's Ctrl+Z and Ctrl+Y, and its command palette, call these.
  ACT.undo = () => ACT.stackundo();
  ACT.redo = () => ACT.stackredo();

  // a discipline whose view is its own page (Workshop's) keeps the demo's way of opening it
  const framePlan = R.plan;
  R.plan = () => (FRAMEV[S.view] ? framePlan() : plan());
  R['3d'] = threeD;
  for (const [screen] of SCREENS) {
    if (screen !== 'plan' && screen !== '3d') {
      R[screen] = () => {
        $(`#sc-${screen}`).innerHTML = `<p class="pad faint stack-later">${esc(NOT_YET)}</p>`;
      };
    }
  }

  $('.crumb').innerHTML =
    `<label class="sr" for="stack-hub">Hub</label>` +
    `<select id="stack-hub" class="inp stack-hub" data-chg="stackhub"></select>` +
    `<span aria-hidden="true">/</span><b id="stack-project"></b>` +
    `<span aria-hidden="true">/</span><span class="t" id="h-view"></span>`;
  const team = $('#b-team');
  team.dataset.act = 'account';
  team.setAttribute('aria-label', 'Your account');
  $('#saved').innerHTML = '';

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Delete' && !event.target.closest('input,textarea,select')) ACT.stackdelete();
  });
  setInterval(drawPlan, 1000); // claims run out by themselves
}

/** The demo's ribbon, with only the buttons that have a meaning here changed. */
function connectRibbon(config) {
  const panel = (tab, label) =>
    config.tabs.find((t) => t.id === tab)?.panels.find((p) => p.label === label);
  const set = (tab, label, items) => {
    const found = panel(tab, label);
    if (found) found.items = items;
  };
  const planButton = (view) => ({
    act: 'view',
    a: view,
    label: VIEWS[view].name,
    icon: 'plan',
    pressed: S.screen === 'plan' && S.view === view,
  });
  set('arch', 'Views', [
    { stack: [planButton('l1'), planButton('l2'), { label: 'Default 3D', icon: 'cube', act: 'go', a: '3d', pressed: S.screen === '3d' }] },
  ]);
  // Each tool's buttons beside the demo's own (register.ribbons, under 1000).
  for (const entry of entries('ribbons')) if (entry.order < 1000) entry.ribbon(config);
  set('ins', 'Link', [
    { label: 'Link model', icon: 'link', act: 'stacklink', tip: 'Link a model: live from your team’s WIP, or from a Shared or Consumed folder.', disabled: !C.model },
  ]);
  set('col', 'Share with other firms', [
    { label: 'Share this model', icon: 'up', act: 'stackshare', a: C.model?.id, tip: 'Publish the open model as the next numbered package in your team’s Shared folder.', disabled: !C.model },
  ]);

  // The quick access bar's Undo and Redo: this person's own steps in the open model, from the server.
  const editable = Boolean(C.model) && !C.readOnly;
  for (const item of config.quick || []) {
    if (item.act === 'undo') {
      const step = editable && C.steps.undo;
      Object.assign(item, { act: 'stackundo', disabled: !step, tip: step ? `Undo: ${stepWords(step)}` : 'Nothing of yours to undo' });
    } else if (item.act === 'redo') {
      const step = editable && C.steps.redo;
      Object.assign(item, { act: 'stackredo', disabled: !step, tip: step ? 'Redo what you undid' : 'Nothing of yours to redo' });
    }
  }

  // Manage: the project's parameters and the types, in dialogs (see "Types and parameters" below).
  config.tabs.find((t) => t.id === 'man')?.panels.push({
    label: 'Types and parameters',
    items: [
      { label: 'Types', icon: 'family', act: 'stacktypes', tip: 'The types of a category: make, rename, copy and delete them, and edit their values.', disabled: !C.model },
      { label: 'Project parameters', icon: 'props', act: 'stackparams', tip: 'The parameters of this project: define one and choose the categories that have it.', disabled: !C.model },
      { label: 'Families', icon: 'family', act: 'stackfamilies', tip: 'The families of this model: load one from the library, make a door or window type from a spec, reload a family, or remove one that is not placed.', disabled: !C.model },
      { label: 'Levels', icon: 'elev', act: 'stacklevels', tip: 'The levels of this model: make, rename, move and delete them, and choose the one the plan shows.', disabled: !C.model },
    ],
  });

  const piece = C.elements.get(S.sel);
  if (piece && !C.readOnly && S.screen === 'plan') {
    const kind = piece.el.kind === 'wall' ? 'Walls' : piece.el.kind === 'door' ? 'Doors' : piece.el.kind === 'window' ? 'Windows' : 'Element';
    config.contextual.push({
      id: 'sel',
      key: `sel:${S.sel}`,
      group: 'Modify',
      label: kind,
      panels: [
        { label: kind, items: [{ label: 'Delete', icon: 'del', act: 'stackdelete', kbd: 'Delete' }] },
        { label: 'Selection', items: [{ label: 'Deselect', icon: 'x', act: 'deselect' }] },
      ],
    });
  }
  // A selection's own Modify tab, after the core's (register.ribbons, 1000 and over).
  for (const entry of entries('ribbons')) if (entry.order >= 1000) entry.ribbon(config);
  return config;
}

function account() {
  const body = `<p>Signed in as <b>${esc(C.me.person.name)}</b>.</p>`;
  const foot = `<button class="btn pri" data-act="stacksignout">Sign out</button>`;
  openDlg('Your account', body, foot);
}

ACT.stacksignout = async () => {
  await fetch('/auth/logout', { method: 'POST' });
  location.reload();
};

function setStatus(text, live) {
  const box = $('#saved');
  box.textContent = text;
  box.dataset.state = live ? 'live' : 'down';
}

/** Workshop's page finds its way into the open model (`spare.workshop`) once, as it loads. This page's
 *  code is a module, which runs once the document has been read: a Workshop frame the demo's start
 *  opened (a reload on Workshop's view) can have loaded before it and found none. Such a frame is
 *  loaded again, so it finds it, as it always did when this page was one script run as the
 *  document was read. A frame still loading finds it by itself. */
function reloadEarlyFrame() {
  const frame = $('#sc-workshop iframe');
  let early = false;
  try {
    early = Boolean(frame?.contentWindow?.WorkshopFrame);
  } catch {
    early = false;
  }
  if (early) frame.contentWindow.location.reload();
}

export { reloadEarlyFrame, setStatus, takeOverTheFrame };
