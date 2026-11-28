// Connects the Spare Construct demo page to its server. The server sends the demo's own file
// unchanged, so its ribbon, docks, view tabs and status bar are the real ones; this script runs
// after it and puts live data where the sample was: the plan and 3D views, the project browser,
// the properties palette, the Types and Project parameters dialogs and the status bar. The page keeps only what is in view; whatever it
// fetched by hash stays in the browser's cache. Edits go up the socket and are drawn at once, then
// confirmed or turned back by the log.
//
// An element here is its recipe. Shapes come from the same geometry the server builds with,
// compiled for the browser, and this file draws what comes back: an outline, a mesh, a box around
// it. It never reads a shape's size from anywhere else, so it doesn't change when the geometry does.
// The server's checkpoints name the bodies it built; this page compares them with its own.
(() => {
  'use strict';

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
    viewing: false, // this browser can't build the open model, so it draws what's stored
    cuts: false, // the open model's package cuts what is hosted in a wall, as the server builds it
    checked: 0, // bodies compared with the server's
    mismatches: [], // and any that differed
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
    C.asking.push({ path: String(path).split('/').slice(-2).join('/'), at: asked, ms: Date.now() - asked });
    if (!response.ok) {
      const error = new Error(body.error || response.statusText);
      error.status = response.status;
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

  // --- The demo's frame, with the sample taken out of what's connected ---

  function takeOverTheFrame() {
    VIEWS.l1 = { name: 'Level 1 plan', kind: 'Floor plan', sc: '1:100' };
    VIEWS.l2 = { name: 'Level 2 plan', kind: 'Floor plan', sc: '1:100' };
    VB.l1 = [-2500, -2500, 17000, 13000];
    VB.l2 = VB.l1;
    FLOORV.splice(0, FLOORV.length, 'l1', 'l2');
    HOMEV.architecture = 'l1';
    // Architecture is connected to the project; Workshop builds its parts with the geometry kernel in the browser
    for (const discipline of DISCS) discipline[4] = ['architecture', 'workshop'].includes(discipline[0]) ? 1 : 0;
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
      $('#status-r').innerHTML = `${warningsCount()}<span id="st-log" class="mono">${esc(waiting + checks + speed)}Edit ${C.head}</span>`;
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
        leaveView();
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
    floorRibbon(config); // the floor tool beside the wall tool
    roofRibbon(config); // and the roof tool beside the floor tool
    familyRibbon(config); // the family tool beside the door tool
    openingRibbon(config); // the opening and shaft tools beside the family tool
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
    datumRibbon(config); // the grid tool, and Delete for a selected grid or plane
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

  // --- Signing in and choosing where to work ---

  async function boot() {
    try {
      C.me = await api('/api/me');
    } catch (e) {
      if (e.status !== 401) throw e;
      document.documentElement.dataset.stack = 'signin';
      return;
    }
    takeOverTheFrame();
    await loadGeometry();
    const { person, hubs } = C.me;
    $('#stack-hub').innerHTML = hubs
      .map((hub) => `<option value="${esc(hub.id)}">${esc(hub.name)} (${hub.role === 'external' ? 'external' : 'home'})</option>`)
      .join('');
    $('#b-team').innerHTML = `<span class="avs" aria-hidden="true"><span class="av me">${esc(initials(person.name))}</span></span><span class="hide-s">${esc(person.name)}</span>`;
    CHG.stackhub = (select) => openHub(select.value);
    await openHub(person.home_hub);
    document.documentElement.dataset.stack = 'ready';
  }

  async function openHub(id) {
    $('#stack-hub').value = id;
    const data = await api(`/api/hubs/${id}`);
    C.hub = data.hub;
    C.people = data;
    C.projects = data.projects;
    closeProject();
    if (data.projects.length) await openProject(data.projects[0].id);
    else refresh();
  }

  function closeProject() {
    if (C.socket) {
      C.socket.onclose = null;
      C.socket.close();
    }
    Object.assign(C, { socket: null, connected: false, project: null, data: null, model: null });
    Object.assign(C, { pending: [], links: [], log: [], readOnly: false });
    for (const map of [C.elements, C.claims, C.linked, C.fixed]) map.clear();
    C.watching.clear();
    S.sel = null;
    S.tool = 'select';
    $('#stack-project').textContent = '';
    $('#saved').textContent = '';
  }

  async function openProject(id) {
    closeProject();
    C.project = id;
    await refreshProject();
    C.head = C.data.head;
    connect();
    const models = C.data.teams.flatMap((team) =>
      team.folders.flatMap((f) => (f.models || []).map((model) => ({ team: team.id, model }))),
    );
    if (models.length) await openModel(models[0].team, models[0].model);
    if (S.screen !== 'plan' && S.screen !== '3d') go(`plan:${S.view}`);
    else refresh();
  }

  async function refreshProject() {
    C.data = await api(`/api/projects/${C.project}`);
    C.models.clear();
    for (const team of C.data.teams) {
      for (const folder of team.folders) {
        for (const model of folder.models || []) C.models.set(model.id, model);
      }
    }
    $('#stack-project').textContent = C.data.project.name;
    $('#pal-browser-h small').textContent = `· ${C.data.project.name}`;
    drawBrowser();
  }

  ACT.stackproject = (button) => openProject(button.dataset.a);

  // --- The project browser ---

  function drawBrowser() {
    const box = $('#browser');
    if (!box) return;
    for (const group of $$('details', box)) {
      if (group.open) BOPEN.add(group.dataset.k);
      else BOPEN.delete(group.dataset.k);
    }
    const leaf = (act, a, label, on, icon, attrs = '') =>
      `<button class="leaf" data-act="${act}" data-a="${esc(a)}"${on ? ' aria-current="true"' : ''} ${attrs}>${ic(icon, 'sm')}<span>${esc(label)}</span></button>`;
    const group = (key, body) =>
      `<details data-k="${key}"${BOPEN.has(key) ? ' open' : ''}><summary>${ic('chev', 'sm chev')}${key}</summary>${body}</details>`;

    let tree = '';
    const name = C.data?.project.name || 'No project';
    tree += `<div class="pad stack-head"><div class="lbl">Project</div><b>${esc(name)}</b><div class="small faint">${esc(C.hub?.name || '')}</div></div>`;
    tree += group(
      'Projects',
      C.projects
        .map((p) => leaf('stackproject', p.id, p.name, p.id === C.project, 'folder', `data-project="${esc(p.id)}"`))
        .join('') || '<p class="pad small faint">No projects here for you yet.</p>',
    );
    tree += group(
      'Views',
      `<div class="sub">Floor plans</div>${['l1', 'l2'].map((v) => leaf('view', v, VIEWS[v].name, S.screen === 'plan' && S.view === v && !C.openView, 'plan')).join('')}<div class="sub">3D views</div>${leaf('go', '3d', 'Default 3D', S.screen === '3d', 'cube')}${modelViews(leaf)}`,
    );
    if (C.model) tree += group('Schedules', modelSchedules(leaf));
    if (C.data) tree += group('Project files', projectFiles(leaf));
    if (C.model) tree += group('Warnings', `<div data-warnings>${warningsList()}</div>`);
    if (C.model) tree += group('Linked models', linkedModels());
    if (C.people) {
      const names = (list) => esc(list.join(', ')) || 'None';
      tree += group(
        'Hub people',
        `<p class="pad small"><b>Users:</b> ${names(C.people.users)}<br><b>External:</b> ${names(C.people.externals)}</p>`,
      );
    }
    redraw(box, `<div class="tree">${tree}</div>`);
  }

  const FOLDERS = { wip: 'WIP', shared: 'Shared', consumed: 'Consumed' };

  function projectFiles(leaf) {
    let html = '';
    for (const incoming of C.data.incoming) {
      html += `<div class="note small stack-note" data-incoming="${esc(incoming.id)}">${esc(incoming.item.name)} from ${esc(incoming.origin.hub_name)} is ready to take in.<br><button class="btn sm pri" data-act="stacktake" data-a="${esc(incoming.id)}" data-team="${esc(incoming.to_team)}">Take into Consumed</button></div>`;
    }
    for (const team of C.data.teams) {
      html += `<div class="sub stack-team">${esc(team.name)}</div>`;
      if (!team.folders.some((f) => f.kind === 'wip')) {
        html += `<div class="stack-folder faint" data-hidden-folder="${esc(team.id)}">WIP: only this team can see it</div>`;
      }
      for (const folder of team.folders) {
        html += `<div class="stack-folder" data-folder="${esc(team.id)}/${folder.kind}">${FOLDERS[folder.kind]}</div>`;
        for (const model of folder.models || []) {
          const open = leaf('stackmodel', model.id, model.name, C.model?.id === model.id, 'file', `data-team="${esc(team.id)}" data-model="${esc(model.id)}"`);
          html += `<div class="stack-row">${open}<button class="btn sm" data-act="stackshare" data-a="${esc(model.id)}" data-team="${esc(team.id)}">Share</button></div>`;
        }
        for (const item of folder.packages || []) {
          const from = item.from ? ` (from ${item.from.hub_name})` : '';
          const sent = item.bridged_to.length ? ` · bridged to ${item.bridged_to.join(', ')}` : '';
          const open = leaf('stackpackage', item.id, `${item.name}${from}${sent}`, false, 'sheet', `data-team="${esc(team.id)}" data-folder="${folder.kind}" data-package="${esc(item.id)}"`);
          const bridge = folder.kind === 'shared' && team.member
            ? `<button class="btn sm" data-act="stackbridge" data-a="${esc(item.id)}" data-team="${esc(team.id)}">Bridge</button>`
            : '';
          html += `<div class="stack-row">${open}${bridge}</div>`;
        }
        if (!(folder.models || folder.packages).length) html += '<div class="stack-folder faint">Empty</div>';
      }
    }
    return html;
  }

  function linkedModels() {
    const rows = C.links.map(
      (link) => `<div class="stack-folder" data-link="${esc(link.id)}">${esc(link.name)} · ${link.kind}</div>`,
    );
    return `${rows.join('') || '<p class="pad small faint">None yet.</p>'}<div class="pad"><button class="btn sm" data-act="stacklink">Add link</button></div>`;
  }

  ACT.stackshare = async (button) => {
    const id = button.dataset.a || C.model?.id;
    const row = C.data.teams.flatMap((t) => t.folders.flatMap((f) => (f.models || []).map((m) => ({ t, m })))).find(({ m }) => m.id === id);
    if (!row) return;
    try {
      const item = await post(`/api/projects/${C.project}/teams/${row.t.id}/publish`, { model: id });
      say(`Shared ${item.name}.`);
      await refreshProject();
    } catch (e) {
      say(e.message);
    }
  };

  /** A list of choices in the demo's own dialog. */
  function pick(title, choices) {
    C.choices = choices;
    const rows = choices.map((choice, index) => {
      const button = `<button class="btn sm" data-act="stackpick" data-a="${index}"${choice.disabled ? ' disabled' : ''}>Choose</button>`;
      return `<li><span class="grow">${esc(choice.label)}${choice.hint ? `<br><span class="small faint">${esc(choice.hint)}</span>` : ''}</span>${button}</li>`;
    });
    const body = `<ul class="list">${rows.join('') || '<li class="faint">Nothing to choose.</li>'}</ul>`;
    openDlg(title, body, '<button class="btn" data-act="dgclose">Cancel</button>');
  }

  ACT.stackpick = async (button) => {
    const choice = C.choices[Number(button.dataset.a)];
    closeDlg();
    try {
      await choice.run();
    } catch (e) {
      say(e.message);
    }
  };

  ACT.stackbridge = async (button) => {
    const { a: id, team } = button.dataset;
    const targets = await api(`/api/projects/${C.project}/bridge-targets`);
    const item = id;
    const choices = targets.flatMap((target) =>
      target.teams.map((to) => ({
        label: `${target.hub_name} / ${target.project_name} / ${to.name}`,
        disabled: !target.permitted,
        hint: target.permitted ? '' : `${target.hub_name} hasn’t permitted this bridge`,
        run: async () => {
          const body = { to_project: target.project, to_team: to.id };
          await post(`/api/projects/${C.project}/teams/${team}/packages/${item}/bridge`, body);
          say(`Bridged to ${target.hub_name}.`);
          await refreshProject();
        },
      })),
    );
    pick('Bridge to', choices);
  };

  ACT.stacktake = async (button) => {
    const { a: incoming, team } = button.dataset;
    try {
      const item = await post(`/api/projects/${C.project}/teams/${team}/consume`, { incoming });
      say(`Took in ${item.name}.`);
      await refreshProject();
    } catch (e) {
      say(e.message);
    }
  };

  ACT.stacklink = () => {
    if (!C.model) return;
    const choices = [];
    const add = (label, body) =>
      choices.push({
        label,
        run: async () => {
          await post(`${modelUrl(C.model.id)}/links`, body);
          await loadLinks();
          refresh();
        },
      });
    for (const team of C.data.teams) {
      for (const folder of team.folders) {
        for (const model of folder.models || []) {
          if (team.id === C.team && model.id !== C.model.id) {
            add(`${model.name}: live, from your team’s WIP`, { kind: 'live', source: model.id });
          }
        }
        for (const item of folder.packages || []) {
          if (folder.kind === 'shared') {
            add(`${item.name}: from ${team.name}’s Shared`, { kind: 'shared', team: team.id, source: item.id });
          } else if (team.id === C.team) {
            add(`${item.name}: from Consumed`, { kind: 'consumed', source: item.id });
          }
        }
      }
    }
    pick('Link into this model', choices);
  };

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
    C.cuts = false;
    forgetDatums();
    S.sel = null;
    WS.elements.clear();
    WS.loaded = false;
    workshopChanged({});
  }

  async function openModel(team, model) {
    const builder = await builderFor(model);
    forgetTheOpenModel();
    C.model = model;
    C.team = team;
    C.viewing = !builder;
    C.cuts = Boolean(builder) && typeof builder.shapes.build_hosting === 'function';
    C.readOnly = C.viewing;
    C.watching.add(model.id);
    send({ t: 'watch', document: model.id, since: C.head });
    await loadPieces();
    await loadClaims();
    await loadLinks();
    await loadDatums();
    await loadWorkshop();
    await loadSteps();
    await loadViews();
    await loadWarnings();
    await loadSchedules();
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
      if (isWorkshop(piece.element)) WS.elements.set(piece.element.id, { el: piece.element, hash: null, seq: 0 });
    }
    WS.loaded = true;
    workshopChanged({});
    say(`A fixed version: ${file.elements.length} elements.`);
  }

  /** A package carries finished shapes, so it is drawn as it came and never rebuilt. */
  const packaged = (piece) => ({ el: piece.element, ...fromStored(piece), seq: 0 });

  function showPackage(file) {
    for (const piece of file.elements) {
      if (levelIdOf(piece.element) === levelOf()) C.elements.set(piece.element.id, packaged(piece));
    }
  }

  // --- Shapes ---

  // Two packages build shapes, chosen by the engine a document's edition names: the geometry
  // kernel's for a document built on it, the stand-in's otherwise. The stand-in's loads with the
  // page; the kernel's, far larger, the first time a document names it. Each loads once.
  const KERNEL = 'construct.kernel';
  const STAND_IN_PACKAGE = '/shapes/construct_shapes.js';
  const KERNEL_PACKAGE = '/shapes/kernel/construct_shapes_kernel.js';

  function loadGeometry(edition) {
    const path = edition?.engine === KERNEL ? KERNEL_PACKAGE : STAND_IN_PACKAGE;
    if (!C.builders.has(path)) C.builders.set(path, loadPackage(path));
    return C.builders.get(path);
  }

  async function loadPackage(path) {
    try {
      const shapes = await import(path);
      await shapes.default();
      // built: recipe text -> what it builds to, which never changes; hosting: the same for a host
      // and what is hosted in it
      return { shapes, edition: JSON.parse(shapes.edition()), built: new Map(), hosting: new Map() };
    } catch (error) {
      console.warn(`This browser can’t build shapes: ${error.message}`);
      return null;
    }
  }

  /** A value as text with every object's keys in order, so two editions compare field by field. */
  const canonical = (value) =>
    JSON.stringify(value, (_, v) =>
      v && typeof v === 'object' && !Array.isArray(v)
        ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
        : v,
    );

  /**
   * The package that builds `model`'s edition, or null: then this browser shows what's stored. Two
   * editions are the same only when every field is, the engine's report included (FORMAT.md, "A
   * document").
   */
  async function builderFor(model) {
    const builder = await loadGeometry(model.edition);
    return builder && canonical(builder.edition) === canonical(model.edition) ? builder : null;
  }

  function buildShape(builder, recipe) {
    const text = JSON.stringify(recipe);
    if (!builder.built.has(text)) builder.built.set(text, JSON.parse(builder.shapes.build(text)));
    return builder.built.get(text);
  }

  /** What the server stored, as `buildShape` gives it. */
  function fromStored({ built: row, display }) {
    const { shape, failed } = row.result;
    return shape ? { shape: { ...shape, displays: display ? [display] : [] } } : { failed };
  }

  /** The server's copy. Nothing yet means it hasn't regenerated this element: a checkpoint will say. */
  async function storedShape(model, id) {
    try {
      return fromStored(await api(`${modelUrl(model)}/shape/${id}?lod=0`));
    } catch (error) {
      if (error.status === 404) return {};
      throw error;
    }
  }

  /**
   * What `record` builds to here. A wall with elements hosted in it on this page is built with them,
   * each opening cut, as the server's regenerator builds it, when the package can (the kernel's
   * `build_hosting`); the stand-in's builds a host as the wall alone, as its `build` does. A recipe
   * that reads records is built from `fill`, the recipe the server filled for it (`fillOf`).
   */
  function buildOn(builder, record, into, fill) {
    const recipe = fill?.recipe ?? record.recipe;
    const guests = typeof builder.shapes.build_hosting === 'function' && into ? guestsOf(record, into, fill) : [];
    if (!guests.length) return buildShape(builder, recipe);
    // Each guest by its place in the host's frame, so a host that moves builds to the same text.
    const host = JSON.stringify({ recipe, placement: { x: 0, y: 0, z: 0 } });
    const hosted = JSON.stringify(guests);
    const text = `${host}\n${hosted}`;
    if (!builder.hosting.has(text)) builder.hosting.set(text, JSON.parse(builder.shapes.build_hosting(host, hosted)));
    return builder.hosting.get(text);
  }

  async function shapeFor(model, record, into, fill) {
    const row = C.models.get(model);
    const builder = row && (await builderFor(row));
    if (!builder) return storedShape(model, record.id);
    if (fill?.failed) return { failed: fill.failed };
    return buildOn(builder, record, into, fill);
  }

  // A recipe that reads records (a layered wall's layers from its wall type, a top tied to a level, a
  // parameter's value) is filled by the server, the way its regenerator fills it, and this page builds
  // what comes back: it never works out a type's layers or a level's height itself. The answer says
  // how far the log had got, so a checkpoint from before then is not compared with it; an entry that
  // may change what such a recipe reads has it filled and built again (`fillAgain`).

  /** Whether a recipe reads records: an argument that is an object, such as `{"wall":"type"}`. */
  const readsRecords = (recipe) =>
    Object.values(recipe?.args || {}).some((arg) => arg !== null && typeof arg === 'object' && !Array.isArray(arg));

  /** The server's filling of `record`'s recipe in `model`: `{head, recipe}` or `{head, failed}`; null when it reads nothing. */
  async function fillOf(model, record) {
    if (!readsRecords(record.recipe)) return null;
    try {
      return await api(`${modelUrl(model)}/filled/${record.id}`);
    } catch (error) {
      return { head: 0, failed: { code: 'NotFilled', message: error.message } };
    }
  }

  /** Every element of `into` whose recipe reads records, filled and built again. */
  function fillAgain(model, into) {
    for (const [id, piece] of into) {
      if (readsRecords(piece.el.recipe)) refill(model, into, id, piece);
    }
  }

  async function refill(model, into, id, piece) {
    piece.stale = true;
    piece.asked = (piece.asked || 0) + 1;
    const asked = piece.asked;
    const fill = await fillOf(model, piece.el);
    if (into.get(id) !== piece || piece.asked !== asked) return;
    const answer = await shapeFor(model, piece.el, into, fill);
    if (into.get(id) !== piece || piece.asked !== asked) return;
    Object.assign(piece, { shape: undefined, failed: undefined }, answer, { fill, stale: false, settled: asked });
    queueDraw();
    if (id === S.sel) refreshSide();
  }

  /** A stored length, `length:q0:<steps>`, in millimetres. */
  const storedMm = (text) => Number(String(text).split(':').pop()) / 6400;

  /** The face reference a hosted door names its wall by, and that wall, if it is on this page. */
  function hostOf(el) {
    const face = (el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
    const host = face && C.elements.get(face.element);
    return host ? { face, host } : null;
  }

  /**
   * The ID of the element `el` is hosted on, as construct-regen's `hosting::read` reads its record:
   * a face reference with both offsets, or an older door with a face reference and neither.
   */
  function hostIdOf(el) {
    const faces = (el.references || []).filter((ref) => ref.kind === 'face' && !ref.doc);
    const along = el.props?.['@host.along'];
    const up = el.props?.['@host.up'];
    if (along === undefined && up === undefined) {
      const own = el.recipe?.owner === 'construct';
      const opening = own && (el.recipe?.op === 'door.single' || el.recipe?.op === 'opening.floor');
      return opening && faces.length ? faces[0].element : null;
    }
    return along !== undefined && up !== undefined && faces.length === 1 ? faces[0].element : null;
  }

  /**
   * What is hosted in `host` among `into`'s elements, as the kernel's package takes them: each by
   * its ID, its recipe and its place in the host's frame (in whole millimetres), as the server's
   * regenerator hands them to its builder.
   */
  function guestsOf(host, into, fill) {
    const drawn = { el: host, shape: into.get(host.id)?.shape, fill: fill ?? into.get(host.id)?.fill };
    const guests = [];
    for (const [id, piece] of into) {
      if (id === host.id || hostIdOf(piece.el) !== host.id) continue;
      const face = (piece.el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
      const at = placedOn(piece.el, face, drawn);
      const placement = { x: at.x - host.placement.x, y: at.y - host.placement.y, z: at.z - host.placement.z };
      // A placed family is handed to the builder turned to stand in its wall, as the server hands it.
      const seat = isFamily(piece.el) ? seatOfPiece(piece, drawn) : null;
      guests.push({ key: id, recipe: seat ? turnedRecipe(piece.el.recipe, seat.turn) : piece.el.recipe, placement });
    }
    // A shaft is hosted on nothing: the server says which pass through a floor, as it hands its builder.
    for (const shaft of drawn.fill?.cut_by?.shafts ?? []) guests.push({ key: shaft.key, recipe: shaft.recipe, placement: shaft.placement });
    return guests;
  }

  /**
   * Where an element sits. A door hosted on a face of its wall sits where its offsets put it on
   * the wall, as the server reads it (construct-regen's `hosting`): along the wall from its start,
   * up from its base, and flush with its side; so it moves with its wall, whose record alone
   * changes. Anything else, or a door whose wall is not here, sits at its own placement.
   */
  function placementOf(el) {
    const hosted = hostOf(el);
    return hosted ? placedOn(el, hosted.face, hosted.host) : el.placement;
  }

  /** Where `el` sits hosted on `face` of `host` (`{el, shape, fill}`): `placementOf`'s rule. */
  function placedOn(el, face, host) {
    const along = el.props?.['@host.along'];
    const up = el.props?.['@host.up'];
    if (along === undefined || up === undefined) return el.placement;
    const role = host.shape?.names?.find((name) => name.name === face.name)?.role ?? face.signature;
    if (el.recipe.op === FAMILY_OP) return onFamily(role, host, along, up)?.at ?? el.placement;
    if (host.el.recipe.op === LAYERED) return onLayered(el, role, host, along, up);
    const thickness = host.el.recipe.args.thickness;
    const own = isWallOpening(el) ? thickness : el.recipe.args.thickness;
    if (!Number.isFinite(thickness) || !Number.isFinite(own)) return el.placement;
    const alongY = host.el.recipe.args.axis === 'y';
    // Along x the exterior is the wall's side at y = 0; along y, its side at x = its thickness.
    const across = (role === 'exterior') !== alongY ? 0 : thickness - own;
    const [a, u] = [storedMm(along), storedMm(up)];
    const at = host.el.placement;
    return alongY
      ? { x: at.x + across, y: at.y + a, z: at.z + u }
      : { x: at.x + a, y: at.y + across, z: at.z + u };
  }

  // A layered wall (`wall.layered`): drawn as a line between two points, its layers read from its
  // wall type (construct-model's FORMAT.md, "Walls"). Lengths in its recipe are grid steps.
  const LAYERED = 'wall.layered';
  const STEPS_PER_MM = 6400;
  const steps = (text) => Number(String(text).split(':').pop());

  /** A number of steps to the nearest whole millimetre, a half away from zero, as construct-regen's `nearest_mm`. */
  function nearestMm(value) {
    const floor = Math.floor(value / STEPS_PER_MM);
    const twice = (value - floor * STEPS_PER_MM) * 2;
    return twice > STEPS_PER_MM || (twice === STEPS_PER_MM && floor >= 0) ? floor + 1 : floor;
  }

  /**
   * A layered wall's line and outer faces, from the recipe the server filled for it: its start in
   * steps, the axis it runs along (0 for x, 1 for y, null at an angle), which way, and its two
   * finish faces' offsets to the right of the line, in steps. Null until it is filled.
   */
  function lineOf(host) {
    const args = host.fill?.recipe?.args;
    if (!args || !Array.isArray(args.layers) || !args.layers.length) return null;
    const start = args.start.map(steps);
    const end = args.end.map(steps);
    const d = [end[0] - start[0], end[1] - start[1]];
    const axis = d[1] === 0 ? 0 : d[0] === 0 ? 1 : null;
    const unit = [Math.sign(d[0]), Math.sign(d[1])];
    const exterior = steps(args.layers[0].exterior);
    const interior = steps(args.layers.at(-1).interior);
    return { start, end, d, axis, unit, exterior, interior };
  }

  /**
   * Where `el` sits on side `role` of a layered wall, as construct-regen's `hosting` puts it: along
   * the wall's line from its start the way the wall runs, flush with that side and reaching toward
   * the other, each to the nearest millimetre, and up from its base. Only a wall along an axis hosts.
   */
  function onLayered(el, role, host, along, up) {
    const line = lineOf(host);
    // A plain opening goes through all of its wall, and its width is a stored length.
    const plain = isWallOpening(el);
    const { width, thickness } = el.recipe.args;
    if (!line || line.axis === null) return el.placement;
    const wide = plain ? steps(width) : width * STEPS_PER_MM;
    const deep = plain ? Math.abs(line.exterior - line.interior) : thickness * STEPS_PER_MM;
    if (!Number.isFinite(wide) || !Number.isFinite(deep)) return el.placement;
    const a = line.axis;
    const c = 1 - a;
    const right = [line.unit[1], -line.unit[0]][c];
    const [on, other] = role === 'interior' ? [line.interior, line.exterior] : [line.exterior, line.interior];
    const lowAlong = line.unit[a] > 0
      ? line.start[a] + steps(along)
      : line.start[a] - (steps(along) + wide);
    const face = line.start[c] + on * right;
    const back = line.start[c] + other * right;
    const lowAcross = back > face ? face : face - deep;
    const at = [0, 0];
    at[a] = nearestMm(lowAlong);
    at[c] = nearestMm(lowAcross);
    const base = host.el.placement;
    return { x: base.x + at[0], y: base.y + at[1], z: base.z + storedMm(up) };
  }

  /** How far along its wall a hosted door at `x`, `y` (its low corner, in the model) is from the wall's start. */
  function alongOn(host, el, x, y) {
    const wall = host.el;
    if (wall.recipe.op !== LAYERED) return wall.recipe.args.axis === 'y' ? y - wall.placement.y : x - wall.placement.x;
    const line = lineOf(host);
    if (!line || line.axis === null) return null;
    const a = line.axis;
    const at = [x - wall.placement.x, y - wall.placement.y][a];
    const start = Math.round(line.start[a] / STEPS_PER_MM);
    const wide = isWallOpening(el) ? storedMm(el.recipe.args.width) : (el.recipe.args.width ?? 0);
    return line.unit[a] > 0 ? at - start : start - at - wide;
  }

  /** The box around a piece in the model. */
  function around(piece) {
    const { x, y } = placementOf(piece.el);
    const { min, max } = piece.shape.bounds;
    return { x: x + min[0], y: y + min[1], w: max[0] - min[0], d: max[1] - min[1] };
  }

  function outlineOf(piece, paint) {
    const { x, y } = placementOf(piece.el);
    const points = piece.shape.displays[0].outline.map(([px, py]) => `${x + px},${y + py}`);
    return `<polygon class="${paint}" points="${points.join(' ')}"/>`;
  }

  /** Each part of a shape, filled with its material's colour where the plan cuts it. */
  function partsOf(piece, parts) {
    const { x, y } = placementOf(piece.el);
    return parts
      .map((part) => {
        const points = part.outline.map(([px, py]) => `${x + px},${y + py}`).join(' ');
        const colour = colourOf(part.material, 'cut');
        const fill = colour ? ` style="fill:${esc(colour)}"` : '';
        return `<polygon class="part" data-material="${esc(part.material)}" data-function="${esc(part.function)}"${fill} points="${points}"/>`;
      })
      .join('');
  }

  /** Compares what the server built with what this page holds (note A05). */
  function compareCheckpoint(entry) {
    const { through, bodies } = entry.args;
    const into = entry.target === C.model?.id ? C.elements : C.linked.get(entry.target);
    if (!into) return;
    for (const [id, body] of Object.entries(bodies)) {
      const piece = into.get(id);
      // Built from a recipe filled later in the log than this checkpoint: the next one covers it.
      if (!piece || piece.seq > through || piece.stale || (piece.fill && piece.fill.head > through)) continue;
      if (piece.shape?.body === body) {
        C.checked += 1;
      } else if (C.viewing || !piece.shape) {
        // Not built here: this is the news that the server's copy is ready.
        storedShape(entry.target, id).then((answer) => {
          Object.assign(piece, { shape: undefined, failed: undefined }, answer);
          queueDraw();
        });
      } else {
        C.mismatches.push({ id, here: piece.shape.body, server: body });
        console.error(`${id} built differently here and on the server.`);
        say(`${piece.el.name} built differently here, so this shows the server’s version.`);
        storedShape(entry.target, id).then((answer) => {
          Object.assign(piece, answer);
          queueDraw();
        });
      }
    }
    queueDraw();
  }

  /** Fetches what is on this level. Records are named by hash, so a repeat comes from the cache. */
  async function loadPieces(model = C.model?.id, into = C.elements) {
    if (!model) return;
    C.level = levelOf();
    const [x, y, w, h] = VB[S.view];
    const area = `level=${encodeURIComponent(C.level)}&x0=${x}&y0=${y}&x1=${x + w}&y1=${y + h}`;
    const seen = await api(`${modelUrl(model)}/area?${area}`);
    // Regeneration can be behind the log, so ask for what came after what this shows.
    if (seen.through < C.head) send({ t: 'watch', document: model, since: seen.through });
    const ids = new Set(Object.keys(seen.pieces));
    await Promise.all(
      [...ids].map(async (id) => {
        const known = into.get(id);
        if (known && (known.hash === seen.pieces[id] || known.seq > seen.through)) return;
        await loadPiece(model, id, seen.pieces[id], seen.through, into);
      }),
    );
    for (const [id, piece] of into) {
      if (!ids.has(id) && piece.seq <= seen.through) into.delete(id);
    }
  }

  async function loadPiece(model, id, hash, seq, into, mine = false) {
    const record = await api(`${modelUrl(model)}/record/${hash}`);
    // A type or the parameter table has no place in the plan: the properties palette reads them.
    if (!hasShape(record)) return;
    if (isWorkshop(record)) {
      if (model === C.model?.id) keepWorkshop(id, record, hash, seq, mine);
      return;
    }
    if (isDatum(record)) return keepDatum(model, id, record, hash, seq, mine); // grids and planes: their own block
    const fill = await fillOf(model, record);
    const answer = await shapeFor(model, record, into, fill);
    // A placed family is built turned to stand in its wall once the wall is here (`seatFamilies`).
    const unseated = isFamily(record) && into === C.elements;
    if (levelIdOf(record) === levelOf()) into.set(id, { el: record, ...answer, hash, seq, fill, ...(unseated ? { stale: true } : {}) });
    // What it is hosted in is built again with it, as the server builds a host with its guests.
    if (cutsIn(model, into)) await buildHostAgain(model, into, hostIdOf(record));
  }

  /** Whether this page builds `model`'s hosts with what is hosted in them, into `into`. */
  const cutsIn = (model, into) => C.cuts && model === C.model?.id && into === C.elements;

  /** Builds host `id` again with what is now hosted in it on this page, and lets it be compared. */
  async function buildHostAgain(model, into, id) {
    const host = id && into.get(id);
    if (!host) return;
    const asked = host.asked;
    const answer = await shapeFor(model, host.el, into, host.fill);
    // A refill (`fillAgain`) that began or ended meanwhile has the newer fill: its build stands.
    if (into.get(id) !== host || host.asked !== asked) return;
    Object.assign(host, { shape: undefined, failed: undefined }, answer);
    // A refill still in flight (`fillAgain`) keeps it out of the checkpoint compare: this build used the fill held now.
    host.stale = (host.asked || 0) !== (host.settled || 0);
  }

  async function loadClaims() {
    C.claims = new Map(Object.entries(await api(`${modelUrl(C.model.id)}/claims`)));
  }

  async function loadLinks() {
    C.links = await api(`${modelUrl(C.model.id)}/links`);
    for (const link of C.links) {
      if (link.kind === 'live') {
        const pieces = C.linked.get(link.model) || new Map();
        C.linked.set(link.model, pieces);
        C.watching.add(link.model);
        send({ t: 'watch', document: link.model, since: C.head });
        await loadPieces(link.model, pieces);
      } else {
        const file = await api(fileUrl(link.team, link.kind, link.package));
        C.fixed.set(link.id, file.elements);
      }
    }
  }

  /** A view changed to another level: the pieces in hand are the wrong ones. */
  async function followTheLevel() {
    if (C.level === levelOf()) return;
    letGo(S.sel);
    S.sel = null;
    C.elements.clear();
    for (const pieces of C.linked.values()) pieces.clear();
    await loadPieces();
    for (const [model, pieces] of C.linked) await loadPieces(model, pieces);
    if (C.readOnly && !C.viewing && C.file) showPackage(C.file);
    drawPlan();
  }

  // --- The live channel ---

  function send(message) {
    if (C.connected) C.socket.send(JSON.stringify(message));
  }

  function connect() {
    const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${scheme}://${location.host}/ws/${C.project}`);
    C.socket = socket;
    setStatus('Connecting', false);
    socket.onopen = () => {
      C.connected = true;
      send({ t: 'hello', client: C.client, documents: [...C.watching], since: C.head });
    };
    socket.onmessage = (event) => {
      const got = Date.now();
      receive(JSON.parse(event.data), got);
    };
    socket.onclose = () => {
      C.connected = false;
      setStatus('Reconnecting', false);
      setTimeout(() => {
        if (C.socket === socket) connect();
      }, 500);
    };
  }

  function receive(message, got) {
    switch (message.t) {
      case 'welcome':
        for (const edit of C.pending) C.socket.send(JSON.stringify(edit));
        break;
      case 'caught_up':
        C.head = Math.max(C.head, message.head);
        setStatus('Live', true);
        statusBar();
        break;
      case 'entry':
        spent('apply', () => apply(message.entry, message.sent_ms, got));
        break;
      case 'ack':
        C.pending = C.pending.filter((edit) => edit.counter !== message.counter);
        break;
      case 'rejected':
        turnBack(message.counter, message.reason);
        break;
      case 'retry': {
        const edit = C.pending.find((e) => e.counter === message.counter);
        if (edit) setTimeout(() => send(edit), 500);
        break;
      }
      case 'refused':
        say(`You don’t have access to ${message.document}.`);
        break;
      default:
        break;
    }
  }

  /** The server said no, so the drawn guess goes back to the last confirmed version. */
  async function turnBack(counter, reason) {
    const edit = C.pending.find((e) => e.counter === counter);
    C.pending = C.pending.filter((e) => e.counter !== counter);
    say(reason);
    refused(counter, reason);
    if (edit && (WS.elements.has(edit.args.element) || isWorkshop(edit.args))) {
      await workshopTurnBack(edit, reason);
      return;
    }
    const piece = edit && C.elements.get(edit.args.element);
    if (piece?.hash && C.model) {
      piece.el = await api(`${modelUrl(C.model.id)}/record/${piece.hash}`);
      drawPlan();
    }
  }

  function apply(entry, sent, got) {
    const fresh = entry.seq > C.head;
    C.head = Math.max(C.head, entry.seq);
    if (entry.cmd === 'document.checkpoint') {
      compareCheckpoint(entry);
      return;
    }
    const mine = entry.client === C.client;
    if (entry.author === C.me.person.id && entry.target === C.model?.id) loadSteps();
    logEntry(entry);
    if (entry.failed) {
      if (mine) turnBack(entry.counter, entry.failed);
      return;
    }
    if (mine) done(entry.counter);
    if (!mine && fresh) C.arriving.push({ ...entry, arrived_ms: Date.now(), sent_ms: sent, got_ms: got });
    const into = entry.target === C.model?.id ? C.elements : C.linked.get(entry.target);
    if (!into) return;
    // A move of elements that are not walls (a door along its wall, say) changes no wall's ends,
    // so no wall is asked of the server again for it. Read before the loop below, which may delete.
    const movedNoWall =
      entry.cmd === 'element.move' &&
      entry.changes.every((change) => {
        const known = into.get(change.piece);
        return known && known.el.kind !== 'wall';
      });
    // This page drew its own edits ahead of the log, but not an undo: the server works that out.
    const drawn = mine && !UNDOING.has(entry.cmd);
    for (const change of entry.changes) {
      if (entry.target === C.model?.id && WS.elements.has(change.piece)) {
        workshopEntry(entry, change, drawn);
        continue;
      }
      if (datumChange(entry, change)) continue; // a grid or a plane this page holds
      const piece = into.get(change.piece);
      if (piece && piece.seq >= entry.seq) continue;
      const stillMine = C.pending.some((edit) => edit.args.element === change.piece);
      // A host is built with what is hosted in it: while what it hosts changes here, it is not
      // compared with the server's, and once that is in hand it is built again.
      const cuts = cutsIn(entry.target, into);
      const placing = ['hosted.place', 'family.place', 'family.move', 'opening.place', 'opening.draw', 'opening.move'].includes(entry.cmd);
      const hosts = new Set([piece && hostIdOf(piece.el), placing && entry.args.host]);
      if (cuts) for (const id of hosts) if (id && into.get(id)) into.get(id).stale = true;
      const again = () => Promise.all([...hosts].map((id) => buildHostAgain(entry.target, into, id)));
      if (!change.after) {
        into.delete(change.piece);
        if (cuts) again().then(queueDraw);
      } else if (piece && entry.cmd === 'element.move') {
        piece.hash = change.after;
        piece.seq = entry.seq;
        if (!(mine && stillMine)) piece.el = { ...piece.el, placement: entry.args.placement };
        // A door hosted by its position sits apart from its host's frame, so a move of either
        // moves it in that frame.
        if (cuts) {
          hosts.add(change.piece);
          again().then(queueDraw);
        }
      } else {
        if (piece) piece.stale = true;
        loadPiece(entry.target, change.piece, change.after, entry.seq, into, drawn)
          .then(() => (cuts ? again() : undefined))
          .then(() => {
            queueDraw();
            if (change.piece === S.sel) refreshSide();
          });
      }
    }
    roomsAgain(entry.target, into, entry);
    // A type, a parameter or a level changed, or may have: what the selected element reads may have
    // too, and so may what a recipe that reads records is filled with.
    const reads = READ_BY_RECIPES.test(entry.cmd) || UNDOING.has(entry.cmd);
    // A wall drawn, moved, retyped or deleted changes the ends of the walls that meet it, though
    // their own records stay as they are: the log names only the wall that changed, so every wall
    // that reads records is asked of the server again (its `filled` holds the joints; this page
    // never works a join out), and the ones the change did not touch come back the same.
    if (reads || (MEETS_WALLS.test(entry.cmd) && !movedNoWall)) fillAgain(entry.target, into);
    if (reads && entry.target === C.model?.id) {
      C.paramsEpoch += 1;
      refreshSide();
    }
    if (reads || /^element\.(type|create|delete)$|^wall\.draw$|^roof\.(create|delete|retype)$|^view\.graphics\./.test(entry.cmd)) {
      if (entry.target === C.model?.id) loadManage();
    }
    if (entry.target === C.model?.id) {
      for (const change of entry.claims) {
        if (change.claim) C.claims.set(change.piece, change.claim);
        else C.claims.delete(change.piece);
      }
      viewsChanged(entry);
      warningsChanged(entry);
      schedulesChanged(entry);
    }
    queueDraw();
  }

  /** The commands that can change where walls meet: a wall made, moved, retyped, changed or deleted. */
  const MEETS_WALLS = /^(element\.(create|move|set|type|delete)|wall\.draw)$/;

  /** The commands that change a type, a parameter or a level: what a recipe that reads records reads. */
  const READ_BY_RECIPES = /^(type|parameter|level|wall\.type|floor\.type|roof\.type|opening|shaft)\.|^(floor|roof)\.defaults$/;

  const WORDS = {
    'element.create': 'added',
    'element.move': 'moved',
    'element.set': 'changed',
    'element.delete': 'deleted',
    'element.claim': 'took',
    'element.release': 'let go of',
    'element.param': 'set a value of',
    'element.type': 'changed the type of',
    'parameter.define': 'defined a parameter',
    'parameter.change': 'changed a parameter',
    'parameter.bind': 'bound a parameter to categories',
    'type.create': 'made a type',
    'type.rename': 'renamed a type',
    'type.duplicate': 'copied a type',
    'type.delete': 'deleted a type',
    'type.set': 'set a value of a type',
    'hosted.place': 'placed on a wall',
    'hosted.move': 'moved along its wall',
    'family.place': 'placed a family on a wall:',
    'family.retype': 'changed the type of',
    'family.move': 'moved along its wall:',
    'family.flip': 'flipped to the other side of its wall:',
    'family.set': 'set a value of',
    'family.delete': 'deleted',
    'level.create': 'made a level',
    'level.rename': 'renamed a level',
    'level.elevation': 'moved a level',
    'level.storey': 'changed a level',
    'level.delete': 'deleted a level',
    'level.tie': 'tied to a level',
    'level.top': 'tied the top of',
    'wall.draw': 'drew',
    'wall.type.create': 'made a wall type',
    'wall.type.layers': 'changed the layers of a wall type',
    'wall.type.rename': 'renamed a wall type',
    'wall.type.duplicate': 'copied a wall type',
    'wall.type.delete': 'deleted a wall type',
    'edit.undo': 'undid a change to',
    'edit.redo': 'redid a change to',
  };

  function logEntry(entry) {
    if (!WORDS[entry.cmd]) return;
    const which = entry.args.element ?? entry.changes.map((change) => change.piece).join(', ');
    const what = entry.failed ? `turned back: ${entry.failed}` : `${WORDS[entry.cmd]} ${which}`;
    C.log.unshift(`<li><span class="mono faint">#${entry.seq}</span> ${esc(named(entry.author))} ${esc(what)}</li>`);
    C.log.length = Math.min(C.log.length, 14);
    const list = $('#stack-log');
    if (list) list.innerHTML = C.log.join('');
  }

  function edit(cmd, args) {
    C.counter += 1;
    const message = {
      t: 'op',
      counter: C.counter,
      base: C.head,
      document: C.model.id,
      cmd,
      args,
      author_ms: Date.now(),
    };
    C.pending.push(message);
    send(message);
    return message.counter;
  }

  function letGo(id) {
    if (id && !C.readOnly && C.claims.get(id)?.person === C.me.person.id) {
      edit('element.release', { element: id });
    }
  }

  // --- Undo and redo ---
  // An undo is an edit like any other: `edit.undo` goes up the same socket, and the server finds this
  // person's last step from the log, puts back what it changed, and turns it back with its reason if
  // someone else has changed the same value since (construct-model's FORMAT.md, "Undo and redo").
  // What there is to undo comes from the server too, so every tab of one person agrees.

  const UNDOING = new Set(['edit.undo', 'edit.redo']);
  const STEP_WORDS = {
    'element.create': 'adding an element',
    'element.move': 'a move',
    'element.set': 'a change',
    'element.delete': 'a delete',
    'element.param': 'a value',
    'element.type': 'a change of type',
    'parameter.define': 'defining a parameter',
    'parameter.change': 'changing a parameter',
    'parameter.bind': 'binding a parameter',
    'type.create': 'making a type',
    'type.rename': 'renaming a type',
    'type.duplicate': 'copying a type',
    'type.delete': 'deleting a type',
    'type.set': 'a value of a type',
    'family.place': 'placing a family',
    'family.retype': 'a change of a family’s type',
    'family.move': 'moving a family',
    'family.flip': 'flipping a family',
    'family.set': 'a value of a family',
    'family.delete': 'deleting a family',
    'level.create': 'making a level',
    'level.rename': 'renaming a level',
    'level.elevation': 'moving a level',
    'level.storey': 'a change to a level',
    'level.delete': 'deleting a level',
    'level.tie': 'a tie to a level',
    'level.top': 'a tie of a top',
    'wall.draw': 'drawing a wall',
    'wall.type.create': 'making a wall type',
    'wall.type.layers': 'the layers of a wall type',
    'wall.type.rename': 'renaming a wall type',
    'wall.type.duplicate': 'copying a wall type',
    'wall.type.delete': 'deleting a wall type',
    'edit.redo': 'a redo',
  };
  const stepWords = (step) => STEP_WORDS[step.cmd] || step.cmd.replace('.', ' ');

  let stepsAsked = null; // the model being asked about, while a request is out
  let stepsAgain = false;
  async function loadSteps() {
    const model = C.model?.id;
    if (!model) return;
    if (stepsAsked) {
      stepsAgain = true;
      return;
    }
    stepsAsked = model;
    try {
      const steps = await api(`${modelUrl(model)}/undo`);
      if (C.model?.id === model) C.steps = steps;
    } catch (error) {
      console.warn(`What there is to undo couldn't be read: ${error.message}`);
    } finally {
      stepsAsked = null;
    }
    renderRibbon();
    if (stepsAgain) {
      stepsAgain = false;
      loadSteps();
    }
  }

  function undoOrRedo(cmd, which) {
    if (!C.model || C.readOnly) {
      say('Open a model in your team’s WIP to undo in it.');
      return;
    }
    // An edit still on its way may be the step: the server decides.
    if (!C.steps[which] && !C.pending.length) {
      say(`There is nothing of yours to ${which} in this model.`);
      return;
    }
    edit(cmd, {});
  }
  ACT.stackundo = () => undoOrRedo('edit.undo', 'undo');
  ACT.stackredo = () => undoOrRedo('edit.redo', 'redo');

  // --- Parameters ---
  // The selected element's category, type and parameters come from the server, which reads them
  // the way it checks a change and builds a shape (construct-model's FORMAT.md, "Categories, types
  // and parameter values"). A change is the `element.param` edit, through the same log as a move:
  // the server parses the typed text with its units, and a refusal comes back as its reason.

  const FROM = { type: 'From its type', default: 'Default', unset: 'Not set', formula: 'From its formula' };

  /** A value its formula gives: shown, never typed, with the formula and, when it gives none, the server's reason. */
  function worked(p) {
    const reason = p.reason ? `<div class="note bad small" data-reason="${esc(p.id)}">${esc(p.reason)}</div>` : '';
    return `${esc(p.shown ?? '')}<div class="f" data-formula="${esc(p.id)}">= ${esc(p.formula)}</div>${reason}`;
  }
  function hasShape(record) {
    // A schedule, a sheet and a viewport are records with no shape too: the plan draws none of them.
    return !['type', 'parameters', 'level', 'view', 'schedule', 'schedule-placement', 'sheet', 'viewport'].includes(record.kind);
  }
  C.params = null; // { key, view } of the selected element
  C.paramsEpoch = 0; // goes up when a type or a parameter changes
  C.edit = edit; // the browser test defines parameters through the page's own edit path

  function paramsFor(piece) {
    if (!C.model) return null;
    const key = `${piece.el.id}/${piece.hash}/${C.paramsEpoch}/${isRoom(piece.el) ? (piece.fill?.head ?? 0) : ''}`;
    if (C.params?.key !== key && C.paramsLoading !== key) {
      C.paramsLoading = key;
      api(`${modelUrl(C.model.id)}/parameters/${piece.el.id}`)
        .then((view) => {
          C.params = { key, view };
          refreshSide();
        })
        .catch(() => {});
    }
    return C.params?.view.element === piece.el.id ? C.params.view : null;
  }

  function parametersTable(view) {
    const editable = !C.readOnly && !C.viewing;
    let html = '<span class="lbl">Parameters</span><table class="props">';
    for (const p of view.parameters.filter((row) => row.from !== READ_ONLY)) {
      const value = p.shown ?? '';
      const field = p.formula
        ? worked(p)
        : p.holder === 'instance' && editable
          ? `<input class="inp" data-chg="stackparam" data-a="${esc(p.id)}" data-key="param:${esc(p.id)}" value="${esc(value)}" aria-label="${esc(p.name)}">`
          : esc(value);
      html += prow(esc(p.name), field + problem(`param:${p.id}`), FROM[p.from] || '');
    }
    return `${html}</table>`;
  }

  /** Draws the properties palette again, unless someone is typing in it. */
  function refreshSide() {
    const typing = document.activeElement?.tagName === 'INPUT';
    for (const side of ['#pv-side', '#iso-side']) {
      const el = $(side);
      if (el && !(typing && el.contains(document.activeElement))) redraw(el, propertiesPalette());
    }
  }

  CHG.stackparam = (input) => {
    if (S.sel && C.model && !C.readOnly) {
      manage('element.param', { element: S.sel, parameter: input.dataset.a, value: input.value }, `param:${input.dataset.a}`);
      input.blur();
    }
  };

  ACT.stackdelete = () => {
    if (S.sel && !C.readOnly && C.elements.has(S.sel)) {
      edit(C.elements.get(S.sel)?.el.kind === 'roof' ? 'roof.delete' : 'element.delete', { element: S.sel });
      S.sel = null;
      refresh();
    }
  };

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
    showProblems();
  }

  /** An edit of the page's own was turned back: its reason goes where it was asked. */
  function refused(counter, reason) {
    const ask = C.asked.get(counter);
    C.asked.delete(counter);
    if (!ask) return;
    C.problems.set(ask.where, reason);
    showProblems();
  }

  /** An edit of the page's own was applied. */
  function done(counter) {
    const ask = C.asked.get(counter);
    C.asked.delete(counter);
    if (ask?.after) ask.after();
  }

  function showProblems() {
    renderManage();
    renderSchedule();
    refreshSide();
    drawBrowser();
  }

  // --- The element's type, in the properties palette ---

  function typesOfCategory(category) {
    if (!C.model || !category) return null;
    const key = `${category.id}/${C.paramsEpoch}`;
    if (C.typeList?.key !== key && C.typeListLoading !== key) {
      C.typeListLoading = key;
      api(`${modelUrl(C.model.id)}/types?category=${category.id}`)
        .then((view) => {
          C.typeList = { key, view };
          refreshSide();
        })
        .catch(() => {});
    }
    return C.typeList?.view.category?.id === category.id ? C.typeList.view : null;
  }

  function typeCell(view) {
    const list = typesOfCategory(view.category);
    const none = esc(view.type?.name || 'None');
    if (!view.category || !canEdit() || !list || !list.types.length) {
      const hint = view.category && list && !list.types.length ? '<div class="f">No types of this category yet.</div>' : '';
      return `${none}${hint}${problem('element-type')}`;
    }
    const options = list.types.map((type) => `<option value="${esc(type.id)}"${view.type?.id === type.id ? ' selected' : ''}>${esc(type.name)}</option>`);
    const first = view.type ? '' : '<option value="" selected disabled>None</option>';
    return `<select class="inp" data-chg="stackelementtype" data-key="element-type" aria-label="Type">${first}${options.join('')}</select>${problem('element-type')}`;
  }

  CHG.stackelementtype = (select) => {
    if (S.sel && select.value) manage('element.type', { element: S.sel, type: select.value }, 'element-type');
  };

  // --- The dialogs ---

  function selectedCategory() {
    return C.params?.view.element === S.sel ? (C.params.view.category?.id ?? null) : null;
  }

  function openManage(which) {
    if (!C.model) {
      say('Open a model in your team’s WIP first.');
      return;
    }
    const category = which === 'types' ? (selectedCategory() ?? C.manage?.category ?? null) : null;
    C.manage = { which, category, open: new Set(), data: null, error: null, shown: false, seq: 0 };
    if (which === 'graphics') Object.assign(C.manage, { view: C.openView, edge: 'cut' });
    const title = { types: 'Types', parameters: 'Project parameters', levels: 'Levels', graphics: 'View graphics' }[which];
    openDlg(title, `<div id="stack-manage" class="stack-manage" data-which="${which}"><p class="small faint">Loading.</p></div>`, '<button class="btn" data-act="dgclose">Close</button>');
    loadManage();
  }
  ACT.stacktypes = () => openManage('types');
  ACT.stackparams = () => openManage('parameters');
  ACT.stacklevels = () => openManage('levels');
  ACT.stackviewgraphics = () => {
    if (C.openView) openManage('graphics');
  };

  /** Reads what the open dialog shows from the server, then draws it. Does nothing when no dialog is open. */
  async function loadManage() {
    const m = C.manage;
    if (!m || !C.model || !$('#stack-manage') || $('#dlg').hidden) return;
    const model = C.model.id;
    m.seq += 1;
    const seq = m.seq;
    try {
      if (C.catalog?.model !== model) C.catalog = { model, ...(await api(`${modelUrl(model)}/catalog`)) };
      if (m.which === 'types') {
        if (m.category === null) m.category = topLevel(C.catalog)[0]?.id ?? null;
        m.data = await api(`${modelUrl(model)}/types${m.category === null ? '' : `?category=${m.category}`}`);
      } else if (m.which === 'graphics') {
        m.data = await api(`${modelUrl(model)}/views/${encodeURIComponent(m.view)}/graphics`);
        if (m.category === null) m.category = m.data.looks[0]?.category ?? null;
      } else if (m.which === 'levels') {
        m.data = { levels: await api(`${modelUrl(model)}/levels`) };
        knowLevels(`${model}/${C.paramsEpoch}`, m.data.levels);
      } else {
        m.data = await api(`${modelUrl(model)}/project-parameters`);
      }
      m.error = null;
    } catch (error) {
      m.error = error.message;
    }
    if (C.manage === m && m.seq === seq) renderManage();
  }

  function renderManage() {
    const m = C.manage;
    const root = $('#stack-manage');
    if (!m || !root || $('#dlg').hidden || !(m.data || m.error)) return;
    let html;
    if (m.error) html = `<div class="note bad small" role="alert">${esc(m.error)}</div>`;
    else html = m.which === 'types' ? typesHtml(m) : m.which === 'levels' ? levelsHtml(m) : m.which === 'graphics' ? graphicsHtml(m) : parametersHtml(m);
    redraw(root, html);
    if (!m.shown) {
      m.shown = true;
      $('select, input', root)?.focus();
    }
  }

  /** Replaces what a container shows, keeping what is typed in it, where the cursor is and how far it is scrolled. */
  function redraw(root, html) {
    const active = document.activeElement;
    const key = root.contains(active) ? active.dataset.key : null;
    const cursor = key && typeof active.selectionStart === 'number' ? [active.selectionStart, active.selectionEnd] : null;
    const drafts = new Map($$('[data-draft]', root).map((input) => [input.dataset.key, input.type === 'checkbox' ? input.checked : input.value]));
    const scroller = root.closest('.dlg-b');
    const scrolled = scroller?.scrollTop;
    root.innerHTML = html;
    for (const input of $$('[data-draft]', root)) {
      if (!drafts.has(input.dataset.key)) continue;
      if (input.type === 'checkbox') input.checked = drafts.get(input.dataset.key);
      else input.value = drafts.get(input.dataset.key);
    }
    if (scroller) scroller.scrollTop = scrolled;
    if (key) {
      const again = $$('[data-key]', root).find((el) => el.dataset.key === key);
      if (again) {
        again.focus();
        if (cursor && typeof again.setSelectionRange === 'function') again.setSelectionRange(...cursor);
      }
    }
  }

  function typesHtml(m) {
    const editable = canEdit();
    const off = editable ? '' : ' disabled';
    const options = topLevel(C.catalog).map((c) => `<option value="${c.id}"${c.id === m.category ? ' selected' : ''}>${esc(c.name)}</option>`);
    let html = `<label class="fld"><span>Category</span><select class="inp" data-chg="stacktypecat" data-key="type-category">${options.join('')}</select></label>`;
    html += '<p class="small muted">A type holds the values that every element of that type shares. Change a value on a type and every element of it changes.</p>';
    const name = esc(m.data.category?.name || 'This category');
    if (!m.data.types.length) html += `<p class="small faint">${name} has no types yet.</p>`;
    for (const type of m.data.types) {
      const label = esc(type.name);
      html += `<section data-type="${esc(type.id)}"><div class="stack-line">`;
      html += `<input class="inp" data-chg="stacktyperename" data-type="${esc(type.id)}" data-key="type-name:${esc(type.id)}" value="${label}" aria-label="Name of the type ${label}"${off}>`;
      html += `<span class="small faint">${plural(type.elements, 'element')}</span>`;
      html += `<button class="btn sm" data-act="stacktypecopy" data-a="${esc(type.id)}" data-key="type-copy:${esc(type.id)}" aria-label="Copy the type ${label}"${off}>Copy</button>`;
      html += `<button class="btn sm" data-act="stacktypedelete" data-a="${esc(type.id)}" data-key="type-delete:${esc(type.id)}" aria-label="Delete the type ${label}"${off}>Delete</button></div>`;
      if (type.parameters.length) {
        html += '<table class="props">';
        for (const p of type.parameters) {
          const settable = p.holder === 'type' && editable;
          const control = p.holder !== 'type'
            ? 'Each element has its own'
            : p.formula
              ? worked(p)
              : settable
              ? `<input class="inp" data-chg="stacktypeset" data-type="${esc(type.id)}" data-a="${esc(p.id)}" data-key="type-value:${esc(type.id)}:${esc(p.id)}" value="${esc(p.shown ?? '')}" aria-label="${label}: ${esc(p.name)}">`
              : esc(p.shown ?? '');
          html += prow(esc(p.name), control, FROM[p.from] || '');
        }
        html += '</table>';
      } else {
        html += '<p class="small faint">No parameters are bound to this category yet. Project parameters adds them.</p>';
      }
      html += `${problem(`type:${type.id}`)}</section>`;
    }
    html += '<h3 class="mt2">New type</h3>';
    html += `<div class="stack-line"><input class="inp" data-draft data-enter="stacknewtype" data-key="new-type-name" placeholder="Name" aria-label="Name for a new type of ${name}"${off}><button class="btn pri sm" data-act="stacknewtype" data-key="new-type-make"${off}>Make type</button></div>`;
    html += problem('new-type');
    return html;
  }

  CHG.stacktypecat = (select) => {
    C.manage.category = Number(select.value);
    C.manage.data = null;
    loadManage();
  };
  CHG.stacktyperename = (input) => manage('type.rename', { type: input.dataset.type, name: input.value }, `type:${input.dataset.type}`);
  CHG.stacktypeset = (input) => manage('type.set', { type: input.dataset.type, parameter: input.dataset.a, value: input.value }, `type:${input.dataset.type}`);
  ACT.stacktypecopy = (button) => {
    const type = C.manage?.data?.types.find((t) => t.id === button.dataset.a);
    if (type) manage('type.duplicate', { type: type.id, new: newId('T'), name: `${type.name} copy` }, `type:${type.id}`);
  };
  ACT.stacktypedelete = (button) => manage('type.delete', { type: button.dataset.a }, `type:${button.dataset.a}`);
  ACT.stacknewtype = () => {
    const input = $('#stack-manage [data-key="new-type-name"]');
    manage('type.create', { type: newId('T'), name: input.value, category: C.manage.category }, 'new-type', () => {
      const again = $('#stack-manage [data-key="new-type-name"]');
      if (again) again.value = '';
    });
  };

  function parametersHtml(m) {
    const editable = canEdit();
    const off = editable ? '' : ' disabled';
    const categories = topLevel(C.catalog);
    const holderWords = { instance: 'Each element holds its own value', type: 'The type holds one value for all its elements' };
    let html = '<p class="small muted">A parameter is a named value. It belongs to the categories you bind it to, and every element of those categories has it.</p>';
    if (!m.data.parameters.length) html += '<p class="small faint">No parameters are defined yet.</p>';
    for (const p of m.data.parameters) {
      const bound = new Set(p.categories.map((c) => c.id));
      html += `<section data-param="${esc(p.id)}"><div class="stack-line"><b>${esc(p.name)}</b><span class="badge n">${esc(capital(p.kind_label))}</span><span class="small faint">${esc(holderWords[p.holder])}</span></div>`;
      const where = p.categories.length ? `Bound to ${esc(p.categories.map((c) => c.name).join(', '))}.` : 'Not bound to any category yet.';
      const standard = p.default === null ? '' : ` Default: ${esc(p.default)}.`;
      const choices = p.choices.length ? ` Choices: ${esc(p.choices.map((c) => c.label).join(', '))}.` : '';
      html += `<p class="small">${where}${standard}${choices}</p>`;
      if (p.formula) html += `<p class="small" data-formula="${esc(p.id)}">Formula: ${esc(p.formula)}</p>`;
      if (C.catalog.kinds.find((k) => k.tag === p.kind)?.takes_formula) {
        html += `<label class="fld"><span>Formula of other parameters, such as Width - 2 * Frame (empty: a person sets the value)</span><input class="inp" data-draft data-enter="stackformula" data-a="${esc(p.id)}" data-key="formula:${esc(p.id)}" value="${esc(p.formula ?? '')}" aria-label="Formula for ${esc(p.name)}"${off}></label>`;
        html += problem(`formula:${p.id}`);
      }
      html += `<details data-open="${esc(p.id)}"${m.open.has(p.id) ? ' open' : ''}><summary>Bind to categories</summary><div class="stack-checks">`;
      for (const c of categories) {
        const on = bound.has(c.id);
        html += `<label class="chk"><input type="checkbox" data-draft data-key="bind:${esc(p.id)}:${c.id}" value="${c.id}"${on ? ' checked disabled' : off}>${esc(c.name)}${on ? ' <span class="small faint">(bound)</span>' : ''}</label>`;
      }
      html += '</div>';
      html += `<label class="fld mt"><span>Default for the categories you choose (optional)</span><input class="inp" data-draft data-enter="stackbind" data-a="${esc(p.id)}" data-key="bind-default:${esc(p.id)}" aria-label="Default for ${esc(p.name)}"${off}></label>`;
      html += `<button class="btn sm" data-act="stackbind" data-a="${esc(p.id)}" data-key="bind-go:${esc(p.id)}"${off}>Bind</button>${problem(`bind:${p.id}`)}</details></section>`;
    }
    html += '<h3 class="mt2">Define a parameter</h3>';
    const kinds = C.catalog.kinds.map((k) => `<option value="${esc(k.tag)}">${esc(capital(k.label))}</option>`);
    const holders = Object.entries(holderWords).map(([value, label]) => `<option value="${value}">${esc(label)}</option>`);
    html += '<div class="grid2">';
    html += `<label class="fld"><span>Name</span><input class="inp" data-draft data-enter="stackdefine" data-key="def-name"${off}></label>`;
    html += `<label class="fld"><span>Kind of value</span><select class="inp" data-draft data-key="def-kind"${off}>${kinds.join('')}</select></label>`;
    html += `<label class="fld"><span>Who holds the value</span><select class="inp" data-draft data-key="def-holder"${off}>${holders.join('')}</select></label>`;
    html += `<label class="fld"><span>Choices, for a choice only, separated by commas</span><input class="inp" data-draft data-key="def-choices"${off}></label>`;
    html += `<label class="fld"><span>Default (optional)</span><input class="inp" data-draft data-enter="stackdefine" data-key="def-default"${off}></label>`;
    html += `<label class="fld"><span>Formula of other parameters (optional), such as Width - 2 * Frame</span><input class="inp" data-draft data-enter="stackdefine" data-key="def-formula"${off}></label>`;
    html += '</div>';
    html += '<div class="lbl">Bind it to (optional)</div><div class="stack-checks">';
    for (const c of categories) {
      html += `<label class="chk"><input type="checkbox" data-draft data-key="def-cat:${c.id}" value="${c.id}"${off}>${esc(c.name)}</label>`;
    }
    html += `</div><div class="stack-line mt"><button class="btn pri sm" data-act="stackdefine" data-key="def-go"${off}>Define parameter</button></div>${problem('define')}`;
    return html;
  }

  document.addEventListener(
    'toggle',
    (event) => {
      const open = event.target.dataset?.open;
      if (open && C.manage) {
        if (event.target.open) C.manage.open.add(open);
        else C.manage.open.delete(open);
      }
    },
    true,
  );

  // Enter in a field does what its button does.
  document.addEventListener('keydown', (event) => {
    const act = event.key === 'Enter' && event.target.dataset?.enter;
    if (act && ACT[act]) {
      event.preventDefault();
      ACT[act](event.target);
    }
  });

  const manageField = (key) => $(`#stack-manage [data-key="${key}"]`);
  const chosen = (root) => $$('input[type="checkbox"]:checked:not(:disabled)', root).map((box) => Number(box.value));

  ACT.stackbind = (source) => {
    const id = source.dataset.a;
    const section = $(`#stack-manage section[data-param="${id}"]`);
    const text = manageField(`bind-default:${id}`).value;
    manage('parameter.bind', { parameter: id, categories: chosen(section), default: text === '' ? null : text }, `bind:${id}`);
  };

  // A formula goes to the server as typed: the server checks it and stores it in its one spelling.
  ACT.stackformula = (input) => {
    const text = input.value.trim();
    manage('parameter.change', { parameter: input.dataset.a, formula: text === '' ? null : text }, `formula:${input.dataset.a}`);
  };

  ACT.stackdefine = () => {
    const id = newId('P');
    const labels = manageField('def-choices').value.split(',').map((text) => text.trim()).filter(Boolean);
    const definition = {
      id,
      name: manageField('def-name').value,
      kind: manageField('def-kind').value,
      holder: manageField('def-holder').value,
      choices: labels.map((label) => ({ key: label, label })),
    };
    const formula = manageField('def-formula').value.trim();
    if (formula !== '') definition.formula = formula;
    const categories = $$('#stack-manage [data-key^="def-cat:"]:checked').map((box) => Number(box.value));
    const text = manageField('def-default').value;
    const clear = () => {
      for (const el of $$('#stack-manage [data-key^="def-"]')) {
        if (el.type === 'checkbox') el.checked = false;
        else if (el.tagName === 'INPUT') el.value = '';
      }
    };
    manage('parameter.define', { definition }, 'define', () => {
      if (!categories.length) {
        clear();
        return;
      }
      manage('parameter.bind', { parameter: id, categories, default: text === '' ? null : text }, 'define', clear);
    });
  };

  // --- Levels ---
  // A level is a record of the model with a name and an elevation, and an element is tied to one by
  // its ID (construct-levels' FORMAT.md). The dialog lists them as the server reads them, lowest
  // first, with each elevation as the server formats it; a change is one of the level commands
  // through the same log, and a refusal is the server's reason beside the level. Show picks the level
  // the plan shows and a newly drawn element is tied to.

  C.planLevel = null; // { id, name } of a level chosen in the dialog; null is the floor plan's own
  C.levelList = null; // { key, levels }: the open model's levels, for the properties palette

  /** Keeps the levels the server listed, and the shown level's name with them. */
  function knowLevels(key, levels) {
    C.levelList = { key, levels };
    drawBrowser(); // the new-view form offers these
    const shown = C.planLevel && levels.find((level) => level.id === C.planLevel.id);
    if (shown && shown.name !== C.planLevel.name) {
      C.planLevel.name = shown.name;
      headerView();
    }
  }

  /** The open model's levels, lowest first, as the server lists them; null until they have come. */
  function modelLevels() {
    if (!C.model) return null;
    const key = `${C.model.id}/${C.paramsEpoch}`;
    if (C.levelList?.key !== key && C.levelListLoading !== key) {
      C.levelListLoading = key;
      api(`${modelUrl(C.model.id)}/levels`)
        .then((levels) => {
          knowLevels(key, levels);
          refreshSide();
        })
        .catch(() => {});
    }
    return C.levelList?.key.startsWith(`${C.model.id}/`) ? C.levelList.levels : null;
  }

  /** The name of the level a record stands on, from the server's list once it has come. */
  function levelName(record) {
    const id = levelIdOf(record);
    if (!id) return 'None';
    return modelLevels()?.find((level) => level.id === id)?.name ?? id;
  }

  function levelsHtml(m) {
    const editable = canEdit();
    const off = editable ? '' : ' disabled';
    let html = '<p class="small muted">A level is a height in the building that elements stand on. Move a level and everything tied to it moves with it. Show a level to see its plan and draw on it.</p>';
    if (!m.data.levels.length) html += '<p class="small faint">No levels yet.</p>';
    for (const level of m.data.levels) {
      const id = esc(level.id);
      const label = esc(level.name);
      const shown = S.screen === 'plan' && levelOf() === level.id;
      html += `<section data-level="${id}"><div class="stack-line">`;
      html += `<input class="inp" data-chg="stacklevelrename" data-level="${id}" data-key="level-name:${id}" value="${label}" aria-label="Name of ${label}"${off}>`;
      html += `<input class="inp" data-chg="stacklevelelevation" data-level="${id}" data-key="level-elevation:${id}" value="${esc(level.shown)}" aria-label="Elevation of ${label}"${off}>`;
      html += `<label class="chk"><input type="checkbox" data-chg="stacklevelstorey" data-level="${id}" data-key="level-storey:${id}"${level.storey ? ' checked' : ''}${off}>Storey</label>`;
      html += `<span class="small faint">${plural(level.tied, 'element')}</span>`;
      html += `<button class="btn sm" data-act="stacklevelshow" data-a="${id}" data-key="level-show:${id}" aria-pressed="${shown}">${shown ? 'Shown' : 'Show'}</button>`;
      html += `<button class="btn sm" data-act="stackleveldelete" data-a="${id}" data-key="level-delete:${id}" aria-label="Delete ${label}"${off}>Delete</button></div>`;
      html += `${problem(`level:${level.id}`)}</section>`;
    }
    html += '<h3 class="mt2">New level</h3>';
    html += `<div class="stack-line"><input class="inp" data-draft data-enter="stacknewlevel" data-key="new-level-name" placeholder="Name" aria-label="Name for a new level"${off}>`;
    html += `<input class="inp" data-draft data-enter="stacknewlevel" data-key="new-level-elevation" placeholder="Elevation" aria-label="Elevation for a new level"${off}>`;
    html += `<button class="btn pri sm" data-act="stacknewlevel" data-key="new-level-make"${off}>Make level</button></div>`;
    html += problem('new-level');
    return html;
  }

  const levelWhere = (control) => `level:${control.dataset.level ?? control.dataset.a}`;
  CHG.stacklevelrename = (input) => manage('level.rename', { level: input.dataset.level, name: input.value }, levelWhere(input));
  CHG.stacklevelelevation = (input) => manage('level.elevation', { level: input.dataset.level, elevation: input.value }, levelWhere(input));
  CHG.stacklevelstorey = (box) => manage('level.storey', { level: box.dataset.level, storey: box.checked }, levelWhere(box));
  ACT.stackleveldelete = (button) => manage('level.delete', { level: button.dataset.a }, levelWhere(button));
  ACT.stacknewlevel = () => {
    const args = { level: newId('L'), name: manageField('new-level-name').value, elevation: manageField('new-level-elevation').value };
    manage('level.create', args, 'new-level', () => {
      for (const key of ['new-level-name', 'new-level-elevation']) {
        const again = manageField(key);
        if (again) again.value = '';
      }
    });
  };
  ACT.stacklevelshow = (button) => {
    const level = C.manage?.data?.levels.find((l) => l.id === button.dataset.a);
    if (!level) return;
    C.planLevel = { id: level.id, name: level.name };
    leaveView();
    if (S.screen === 'plan' && FLOORV.includes(S.view)) {
      headerView();
      followTheLevel();
    } else {
      go('plan:l1');
    }
    renderManage();
  };

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

  // A view is a record of the model with a name, a kind, a scale and, for a plan, a level
  // (construct-views' FORMAT.md). The project browser lists them as the server reads them and keeps
  // the list live through the log; a person makes, renames and deletes one there with the view
  // commands (`view.create`, `view.rename`, `view.delete`), never the generic element commands,
  // which the server turns back for a view. Opening a floor plan shows the plan the server draws
  // from the stored model at the log's head (`/views/<view>/plan`): this page draws that picture
  // and cuts nothing itself. The picture goes under the live elements, which stay in the plan with
  // their picking, claims and tools but draw no fill of their own while it is there (see
  // `.dw.drawn` in connect.css), so a click still picks what it lands on and a wall drawn here or
  // by someone else shows in the server's picture when the model changes. The plan is asked for
  // again, after a short pause, whenever an edit changes the model or the view.

  C.viewsSeq = 0; // goes up with each read of the list, so an old answer is not kept
  C.planSeq = 0; // the same for the plan
  C.planTimer = null;

  /** Reads the model's views, and drops the open one if it has gone. */
  async function loadViews() {
    if (!C.model) return;
    const model = C.model.id;
    C.viewsSeq += 1;
    const seq = C.viewsSeq;
    let list;
    try {
      list = await api(`${modelUrl(model)}/views`);
    } catch {
      return;
    }
    if (C.model?.id !== model || seq !== C.viewsSeq) return;
    C.views = list;
    const open = list.find((view) => view.id === C.openView);
    if (C.openView && !open) leaveView();
    else if (open) {
      const shown = open.level && { id: open.level, name: open.level_name || open.level };
      if (shown && C.planLevel && (C.planLevel.id !== shown.id || C.planLevel.name !== shown.name)) {
        C.planLevel = shown;
        followTheLevel();
      }
      planChanged();
    }
    headerView();
    drawBrowser();
  }

  /** Stops showing a model view's plan; the plan pane draws only its live elements again. */
  function leaveView() {
    C.openView = null;
    C.drawn = null;
    C.planSeq += 1;
    clearTimeout(C.planTimer);
    drawBrowser();
  }

  /** An entry of the log arrived for the open model: what it changes of views and of the plan. */
  function viewsChanged(entry) {
    if (/^view\.|^level\.|^edit\.(undo|redo)$/.test(entry.cmd)) loadViews();
    else if (!/^element\.(claim|release)$/.test(entry.cmd)) planChanged();
  }

  /** Asks for the open view's plan again, once the edits arriving together have all come. */
  function planChanged() {
    if (!C.openView) return;
    clearTimeout(C.planTimer);
    C.planTimer = setTimeout(loadPlan, 150);
  }

  /** Asks the server for the open view's plan and draws what comes back. */
  async function loadPlan() {
    const view = C.openView;
    const model = C.model?.id;
    if (!view || !model) return;
    C.planSeq += 1;
    const seq = C.planSeq;
    let drawn;
    try {
      const answer = await api(`${modelUrl(model)}/views/${encodeURIComponent(view)}/plan`);
      drawn = { view, svg: answer.svg, skipped: answer.skipped, through: answer.through };
    } catch (error) {
      drawn = { view, error: error.message };
    }
    if (C.openView !== view || C.model?.id !== model || seq !== C.planSeq) return;
    C.drawn = drawn;
    queueDraw();
  }

  /** Opens a model view in the plan pane: a plan shows its level, drawn by the server. */
  function openView(id) {
    const view = C.views?.find((v) => v.id === id);
    if (!view) return;
    C.openView = id;
    C.drawn = null;
    if (view.level) C.planLevel = { id: view.level, name: view.level_name || view.level };
    if (S.screen === 'plan' && FLOORV.includes(S.view)) {
      headerView();
      followTheLevel();
    } else {
      go(`plan:${FLOORV.includes(S.view) ? S.view : 'l1'}`);
    }
    drawBrowser();
    loadPlan();
  }

  /** What the server's drawing adds to the plan pane's SVG, under everything live. */
  function drawnPlan() {
    const drawn = C.drawn;
    if (!drawn?.svg || drawn.view !== C.openView) return '';
    if (drawn.body === undefined) drawn.body = plainSvg(drawn.svg);
    // The server's y runs up the sheet and is written negated; this plan's y runs down the model.
    return `<g class="drawn" data-view="${esc(drawn.view)}" transform="scale(1 -1)">${drawn.body}</g>`;
  }

  const DRAWN_TAGS = new Set(['g', 'path', 'line']);
  const DRAWN_ATTRS = new Set([
    'd', 'x1', 'y1', 'x2', 'y2', 'fill', 'stroke', 'stroke-width', 'stroke-linecap', 'opacity', 'data-element', 'data-role', 'data-material',
  ]);

  /** The lines and hatches of the server's SVG document, and nothing else a document could carry. */
  function plainSvg(text) {
    const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
    const walk = (node) => {
      let out = '';
      for (const child of node.children) {
        const tag = child.localName;
        if (!DRAWN_TAGS.has(tag)) continue;
        const attrs = [...child.attributes]
          .filter((a) => DRAWN_ATTRS.has(a.name))
          .map((a) => ` ${a.name}="${esc(a.value)}"`)
          .join('');
        out += `<${tag}${attrs}>${walk(child)}</${tag}>`;
      }
      return out;
    };
    return doc.documentElement.localName === 'svg' ? walk(doc.documentElement) : '';
  }

  /** A line over the plan saying what the server's picture is missing, when it is. */
  function planNote() {
    const drawn = C.drawn;
    if (!C.openView) return '';
    let text;
    let cls = 'stack-plan-note';
    if (!drawn) text = 'Drawing the plan.';
    else if (drawn.error) {
      text = drawn.error;
      cls += ' bad';
    } else if (drawn.skipped.length) {
      text = `Not drawn: ${drawn.skipped.map((s) => s.element).join(', ')}.`;
    } else return '';
    return `<div class="${cls}" role="status">${esc(text)}</div>`;
  }

  /** The browser's part for the model's views: each one, the open one's name and delete, and a new one. */
  function modelViews(leaf) {
    if (!C.model) return '';
    let html = '<div class="sub">Model views</div>';
    if (!C.views) return `${html}<p class="pad small faint">Loading.</p>`;
    if (!C.views.length) html += '<p class="pad small faint">No views yet.</p>';
    const off = canEdit() ? '' : ' disabled';
    for (const view of C.views) {
      const open = view.id === C.openView;
      html += `<div data-view="${esc(view.id)}">${leaf('stackview', view.id, view.name, open && S.screen === 'plan', view.kind === 'floor_plan' ? 'plan' : 'cube', `title="${esc(`${view.kind}, 1:${view.scale}`)}"`)}`;
      if (open) {
        const id = esc(view.id);
        html += `<div class="stack-line pad"><input class="inp" data-chg="stackviewrename" data-view="${id}" data-key="view-name:${id}" value="${esc(view.name)}" aria-label="Name of ${esc(view.name)}"${off}>`;
        html += `<button class="btn sm" data-act="stackviewgraphics" data-a="${id}" data-key="view-graphics:${id}" aria-label="Graphics of ${esc(view.name)}">Graphics</button>`;
        html += `<button class="btn sm" data-act="stackviewdelete" data-a="${id}" data-key="view-delete:${id}" aria-label="Delete ${esc(view.name)}"${off}>Delete</button></div>`;
        html += problem(`view:${view.id}`);
      }
      html += '</div>';
    }
    const levels = modelLevels() || [];
    const here = C.newViewLevel ?? levelOf();
    const options = levels.map((l) => `<option value="${esc(l.id)}"${l.id === here ? ' selected' : ''}>${esc(l.name)}</option>`).join('');
    html += `<div class="stack-line pad"><input class="inp" data-draft data-enter="stacknewview" data-key="new-view-name" placeholder="New plan name" aria-label="Name for a new plan view"${off}>`;
    html += `<select class="inp" data-chg="stacknewviewlevel" data-key="new-view-level" aria-label="Level of the new plan view"${off}>${options}</select>`;
    html += `<button class="btn pri sm" data-act="stacknewview" data-key="new-view-make"${off}>Make plan</button></div>`;
    return html + problem('new-view');
  }

  CHG.stacknewviewlevel = (select) => {
    C.newViewLevel = select.value; // kept across redraws of the browser
  };
  ACT.stackview = (button) => openView(button.dataset.a);
  CHG.stackviewrename = (input) => manage('view.rename', { view: input.dataset.view, name: input.value }, `view:${input.dataset.view}`);
  ACT.stackviewdelete = (button) => manage('view.delete', { view: button.dataset.a }, `view:${button.dataset.a}`);
  ACT.stacknewview = () => {
    const name = $('#browser [data-key="new-view-name"]');
    const level = $('#browser [data-key="new-view-level"]');
    if (!name || !level) return;
    const args = { view: newId('VP'), kind: 'floor_plan', name: name.value, level: level.value };
    manage('view.create', args, 'new-view', () => {
      const again = $('#browser [data-key="new-view-name"]');
      if (again) again.value = '';
    });
  };

  // --- A view's graphics rules ---
  // How a category looks in the open view, for what the view cuts and for what it sees beyond the
  // cut plane (construct-views' FORMAT.md, "Graphics rules"). The dialog is reached from the open
  // view's line in the browser. The server lists every category with how it looks in the view now,
  // where that came from (this view or the project), the pens and patterns a rule may name, and
  // the rules the view holds (`/views/<view>/graphics`); a control sends only the part it holds,
  // as `view.graphics.set`, and the server keeps the rest; Clear is `view.graphics.clear`. A
  // refusal is the server's words beside the controls. Nothing is checked here, and the drawing
  // is the server's: it follows the log in every browser.

  const hexOf = (rgb) => `#${rgb.map((part) => Number(part).toString(16).padStart(2, '0')).join('')}`;
  const rgbOf = (hex) => [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16));
  const ORIGIN_WORDS = {
    subcategory: 'This view has a rule for this subcategory.',
    category: 'This view has a rule for its category.',
    project: 'No rule in this view: as the project’s styles say.',
  };

  function graphicsHtml(m) {
    const d = m.data;
    const off = canEdit() ? '' : ' disabled';
    const here = d.looks.find((l) => l.category === m.category) ?? d.looks[0];
    if (!here) return '<p class="small faint">Nothing to set.</p>';
    if (m.edge === 'cut' && !here.cut) m.edge = 'projection';
    const look = m.edge === 'cut' ? here.cut : here.projection;
    const style = look.style;
    const mm = (um) => `${um / 1000} mm`;
    const pens = (here.annotation ? d.annotation_pens : d.pens).map((pen) => `<option value="${pen.number}"${pen.number === style.pen ? ' selected' : ''}>${esc(`${pen.number} ${pen.name}, ${mm(pen.width)}`)}</option>`);
    const lines = d.line_patterns.map((p) => `<option value="${p.id}"${p.id === style.pattern ? ' selected' : ''}>${esc(p.name)}</option>`);
    const fills = ['<option value="">The material’s own</option>', ...d.fill_patterns.map((name) => `<option value="${esc(name)}"${style.fill?.pattern === name ? ' selected' : ''}>${esc(capital(name))}</option>`)];
    const edge = (which, label) => `<button class="btn sm" data-act="stackgfxedge" data-a="${which}" data-key="gfx-edge:${which}" aria-pressed="${m.edge === which}"${which === 'cut' && !here.cut ? ' disabled' : ''}>${label}</button>`;
    const options = d.looks.map((l) => `<option value="${l.category}"${l.category === here.category ? ' selected' : ''}>${l.parent === null ? '' : '– '}${esc(l.name)}</option>`);
    const ruled = d.rules.find((r) => r.category === here.category)?.[m.edge];
    let html = '<p class="small muted">How a category looks in this view. A rule here changes the drawing of this view only; what it does not name stays as it looks now.</p>';
    html += `<label class="fld"><span>Category</span><select class="inp" data-chg="stackgfxcat" data-key="gfx-category">${options.join('')}</select></label>`;
    html += `<div class="stack-line">${edge('cut', 'Cut')}${edge('projection', 'Seen beyond the cut')}</div>`;
    html += `<div class="small faint" data-gfx-origin="${esc(look.origin)}">${esc(ORIGIN_WORDS[look.origin])}</div>`;
    html += '<table class="props">';
    html += prow('Shown', `<input type="checkbox" data-chg="stackgfxshown" data-key="gfx-shown"${style.shown ? ' checked' : ''}${off} aria-label="Shown">`, '');
    html += prow('Pen', `<select class="inp" data-chg="stackgfxpen" data-key="gfx-pen"${off} aria-label="Pen">${pens.join('')}</select>`, '');
    html += prow('Line pattern', `<select class="inp" data-chg="stackgfxpattern" data-key="gfx-pattern"${off} aria-label="Line pattern">${lines.join('')}</select>`, '');
    html += prow('Line colour', `<input type="color" class="inp" data-chg="stackgfxcolour" data-key="gfx-colour" value="${hexOf(style.colour)}"${off} aria-label="Line colour">`, '');
    if (m.edge === 'cut') {
      html += prow('Fill', `<select class="inp" data-chg="stackgfxfill" data-key="gfx-fill"${off} aria-label="Fill pattern">${fills.join('')}</select>`, '');
      html += prow('Fill colour', `<input type="color" class="inp" data-chg="stackgfxfillcolour" data-key="gfx-fill-colour" value="${hexOf(style.fill?.colour ?? [0, 0, 0])}"${off || (style.fill ? '' : ' disabled')} aria-label="Fill colour">`, '');
    }
    html += prow('Halftone', `<input type="checkbox" data-chg="stackgfxhalftone" data-key="gfx-halftone"${style.halftone ? ' checked' : ''}${off} aria-label="Halftone">`, '');
    html += prow('Transparency', `<input class="inp" type="number" data-chg="stackgfxtransparency" data-key="gfx-transparency" value="${style.transparency}"${off} aria-label="Transparency, per cent">`, 'per cent');
    html += '</table>';
    html += `<div class="stack-line"><button class="btn sm" data-act="stackgfxclear" data-a="${m.edge}" data-key="gfx-clear"${ruled ? '' : ' disabled'}${off}>Clear this rule</button></div>`;
    html += problem(`graphics:${here.category}:${m.edge}`);
    html += '<h3 class="mt2">Rules in this view</h3>';
    if (!d.rules.length) html += '<p class="small faint" data-gfx-none>None: every category looks as the project says.</p>';
    for (const rule of d.rules) {
      const part = (label, st) => (st ? `${label}: pen ${st.pen}, ${hexOf(st.colour)}${st.shown ? '' : ', hidden'}${st.halftone ? ', halftone' : ''}${st.transparency ? `, ${st.transparency}% transparent` : ''}${st.fill ? `, ${st.fill.pattern} fill ${hexOf(st.fill.colour)}` : ''}` : '');
      const text = [part('cut', rule.cut), part('seen', rule.projection)].filter(Boolean).join('; ');
      html += `<div class="stack-line" data-gfx-rule="${rule.category}"><span>${esc(rule.name)}</span><span class="small faint">${esc(text)}</span>`;
      html += `<button class="btn sm" data-act="stackgfxpick" data-a="${rule.category}" data-key="gfx-pick:${rule.category}">Edit</button>`;
      html += `<button class="btn sm" data-act="stackgfxclearall" data-a="${rule.category}" data-key="gfx-clear-all:${rule.category}"${off}>Clear</button></div>`;
    }
    html += problem('graphics');
    return html;
  }

  /** Sends one part of the rule the dialog shows; the server keeps the parts not named. */
  function graphicsSet(part) {
    const m = C.manage;
    manage('view.graphics.set', { view: m.view, category: m.category, edge: m.edge, style: part }, `graphics:${m.category}:${m.edge}`);
  }
  const graphicsStyle = () => {
    const m = C.manage;
    const look = m.data.looks.find((l) => l.category === m.category);
    return (m.edge === 'cut' ? look.cut : look.projection).style;
  };
  CHG.stackgfxcat = (select) => {
    C.manage.category = Number(select.value);
    renderManage();
  };
  ACT.stackgfxedge = (button) => {
    C.manage.edge = button.dataset.a;
    renderManage();
  };
  ACT.stackgfxpick = (button) => {
    C.manage.category = Number(button.dataset.a);
    renderManage();
  };
  CHG.stackgfxshown = (box) => graphicsSet({ shown: box.checked });
  CHG.stackgfxpen = (select) => graphicsSet({ pen: Number(select.value) });
  CHG.stackgfxpattern = (select) => graphicsSet({ pattern: Number(select.value) });
  CHG.stackgfxcolour = (input) => graphicsSet({ colour: rgbOf(input.value) });
  CHG.stackgfxfill = (select) => graphicsSet({ fill: select.value ? { pattern: select.value, colour: graphicsStyle().fill?.colour ?? [0, 0, 0] } : null });
  CHG.stackgfxfillcolour = (input) => graphicsSet({ fill: { pattern: graphicsStyle().fill?.pattern, colour: rgbOf(input.value) } });
  CHG.stackgfxhalftone = (box) => graphicsSet({ halftone: box.checked });
  CHG.stackgfxtransparency = (input) => graphicsSet({ transparency: input.value === '' ? null : Number(input.value) });
  ACT.stackgfxclear = (button) => {
    const m = C.manage;
    manage('view.graphics.clear', { view: m.view, category: m.category, edge: button.dataset.a }, `graphics:${m.category}:${button.dataset.a}`);
  };
  ACT.stackgfxclearall = (button) => manage('view.graphics.clear', { view: C.manage.view, category: Number(button.dataset.a) }, 'graphics');
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
  // A94 adds to the same dialog: a sort key with a header and a footer groups the table (the server
  // gives the items in order: header, rows, footer), "Itemize every instance" off folds equal rows into
  // one with a count, the totals of each column and the grand total are the server's exact sums,
  // a calculated field is a formula the server checks, a double-click on a value changes it in the
  // model (`schedule.cell`: the element's own command, so one undo step and a claim; a value that is
  // worked out or held elsewhere is refused in the server's words), and a schedule placed on a sheet is
  // drawn from the sheet's listing, which the server draws from the live table.

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
      await readScheduleSheets(open, model);
      if (C.sched !== open || C.model?.id !== model || seq !== open.seq) return;
    } catch (error) {
      if (C.sched !== open || seq !== open.seq) return;
      // A schedule just made is not in the list until the log's entry for it comes.
      open.error = C.schedules?.some((schedule) => schedule.id === open.id) ? error.message : null;
    }
    spent('schedule-draw', renderSchedule);
  }

  /** The model's sheets and, for each sheet the open schedule stands on, that sheet with the schedule drawn from its live table. */
  async function readScheduleSheets(open, model) {
    try {
      open.sheets = await api(`${modelUrl(model)}/sheets`);
      const drawn = {};
      for (const placed of open.table.schedule.placements) drawn[placed.sheet] = await api(`${modelUrl(model)}/sheets/${encodeURIComponent(placed.sheet)}`);
      open.drawn = drawn;
    } catch {
      // The sheets are shown beside the table when they come; the table is not held back by them.
    }
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
    C.sched = { id, table: null, error: null, seq: 0, gone: false, shown: false, draft: {}, editing: null, sheets: [], drawn: {} };
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

  /** One cell of a data row: its text as the server shows it, or the input a person is typing a new value in. */
  function scheduleCell(open, column, row, cell) {
    const editing = open.editing?.element === row.element && open.editing?.field === column.field;
    const shown = cell.shown ?? '';
    if (editing) {
      return `<td data-cell="${esc(column.field)}" data-element="${esc(row.element)}" class="editing"><input class="inp" data-draft data-enter="stackschedcellset" data-key="sched-cell" value="${esc(shown)}" aria-label="New value of ${esc(column.name)}"></td>`;
    }
    // Only a row of one element is edited in place; a folded row stands for several.
    const where = row.count === 1 ? ` data-cell="${esc(column.field)}" data-element="${esc(row.element)}"${column.editable ? ' class="cell-edit"' : ''}` : '';
    return `<td${where}>${esc(shown)}</td>`;
  }

  /** The cells of a footer or the grand total: each column's totals as "sum: 12 m", the label where a column has none. */
  function scheduleFooter(table, footer, grand) {
    const count = table.columns.length + (table.schedule.itemize ? 0 : 1);
    const cells = Array.from({ length: count }, () => []);
    for (const total of footer.totals) {
      if (total.shown !== null && cells[total.column]) cells[total.column].push(`${total.total}: ${total.shown}`);
    }
    const label = grand ? `Grand total (${footer.elements})` : `${footer.name}: ${footer.shown ?? 'none'} (${footer.elements})`;
    cells[0]?.unshift(label);
    const cls = grand ? 'sched-foot sched-grand' : 'sched-foot';
    const at = grand ? ' data-grand="1"' : ` data-footer="${footer.level}"`;
    return `<tr class="${cls}"${at}>${cells.map((words) => `<td>${esc(words.join('  '))}</td>`).join('')}</tr>`;
  }

  /** The table: a heading for each field, then each header, row and footer as the server gives them. */
  function scheduleTable(open) {
    const table = open.table;
    const schedule = table.schedule;
    const counted = !schedule.itemize;
    const width = table.columns.length + (counted ? 1 : 0);
    let html = `<div class="sched-scroll"><table class="sched-table" data-schedule-table="${esc(schedule.id)}"><thead><tr>`;
    html += table.columns.map((column) => `<th data-field="${esc(column.field)}">${esc(column.name)}</th>`).join('');
    html += `${counted ? '<th data-field="count">Count</th>' : ''}</tr></thead><tbody>`;
    for (const item of table.items) {
      if (item.kind === 'header') {
        html += `<tr class="sched-head" data-header="${item.level}"><td colspan="${width}">${esc(item.name)}: ${esc(item.shown ?? 'none')}</td></tr>`;
      } else if (item.kind === 'footer') {
        html += scheduleFooter(table, item, false);
      } else {
        const chosen = S.sel === item.element;
        html += `<tr class="click${chosen ? ' sel' : ''}" data-act="stackschedrow" data-a="${esc(item.element)}" data-row="${esc(item.element)}"${chosen ? ' aria-selected="true"' : ''}>`;
        html += item.cells.map((cell, at) => scheduleCell(open, table.columns[at], item, cell)).join('');
        html += `${counted ? `<td class="sched-count">${item.count}</td>` : ''}</tr>`;
      }
    }
    if (table.grand) html += scheduleFooter(table, table.grand, true);
    html += '</tbody></table></div>';
    return html;
  }

  /** The sorting part: each key up or down, with a header and a footer for its groups. */
  function scheduleSortingPart(table, off) {
    const schedule = table.schedule;
    const draft = C.sched.draft;
    let html = '<section data-part="sorting"><h3>Sorting and grouping</h3>';
    if (!schedule.sorting.length) html += '<p class="small faint">Not sorted: the rows come in the order of the elements.</p>';
    schedule.sorting.forEach((sort, at) => {
      const name = fieldName(table, sort.field);
      const words = `${name}, ${sort.order === 'descending' ? 'down' : 'up'}`;
      html += `<div class="stack-line" data-sort-row="${at}"><span class="grow">${esc(words)}</span>`;
      html += `<label class="chk"><input type="checkbox" data-chg="stackschedsortheader" data-a="${at}" data-key="sched-sort-header:${at}"${sort.header ? ' checked' : ''}${off}>Header</label>`;
      html += `<label class="chk"><input type="checkbox" data-chg="stackschedsortfooter" data-a="${at}" data-key="sched-sort-footer:${at}"${sort.footer ? ' checked' : ''}${off}>Footer</label>`;
      html += `<button class="btn sm" data-act="stackschedsortflip" data-a="${at}" aria-label="Turn ${esc(name)} the other way round"${off}>${sort.order === 'descending' ? 'Up instead' : 'Down instead'}</button>`;
      html += `<button class="btn sm" data-act="stackschedsortoff" data-a="${at}" aria-label="Remove sorting by ${esc(name)}"${off}>Remove</button></div>`;
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
    return `${html}${problem(`sched-sorting:${schedule.id}`)}</section>`;
  }

  /** The totals part: whether every instance is listed, the grand total, and each column's totals. */
  function scheduleTotalsPart(table, off) {
    const schedule = table.schedule;
    let html = '<section data-part="totals"><h3>Totals</h3>';
    html += `<label class="chk"><input type="checkbox" data-chg="stackscheditemize" data-key="sched-itemize"${schedule.itemize ? ' checked' : ''}${off}>Itemize every instance</label>`;
    html += `<label class="chk"><input type="checkbox" data-chg="stackschedgrand" data-key="sched-grand"${schedule.grand_totals ? ' checked' : ''}${off}>Grand total</label>`;
    if (!schedule.itemize) html += '<p class="small faint">Rows that agree in every column are one row with a count; the totals still count every element.</p>';
    for (const totals of schedule.totals) {
      for (const kind of totals.do) {
        const words = `${fieldName(table, totals.field)}: ${kind}`;
        html += `<div class="stack-line" data-total-row="${esc(totals.field)}:${kind}"><span class="grow">${esc(words)}</span>`;
        html += `<button class="btn sm" data-act="stackschedtotaloff" data-field="${esc(totals.field)}" data-kind="${kind}" aria-label="Remove the total ${esc(words)}"${off}>Remove</button></div>`;
      }
    }
    if (schedule.fields.length) {
      const field = schedule.fields.includes(C.sched.draft.totalField) ? C.sched.draft.totalField : schedule.fields[0];
      html += `<div class="stack-line"><select class="inp" data-chg="stackschedtotalfield" data-key="sched-total-field" aria-label="Column to total"${off}>`;
      html += schedule.fields.map((key) => `<option value="${esc(key)}"${key === field ? ' selected' : ''}>${esc(fieldName(table, key))}</option>`).join('');
      html += '</select>';
      html += `<select class="inp" data-chg="stackschedtotalkind" data-key="sched-total-kind" aria-label="Which total"${off}>`;
      html += ['sum', 'count', 'min', 'max'].map((k) => `<option value="${k}"${k === (C.sched.draft.totalKind ?? 'sum') ? ' selected' : ''}>${k}</option>`).join('');
      html += '</select>';
      html += `<button class="btn pri sm" data-act="stackschedtotaladd" data-key="sched-total-make"${off}>Add total</button></div>`;
    }
    return `${html}${problem(`sched-totals:${schedule.id}`)}</section>`;
  }

  /** The calculated fields part: each with its formula, and a new one from a name, a formula and a kind. */
  function scheduleCalculatedPart(table, off) {
    const schedule = table.schedule;
    let html = '<section data-part="calculated"><h3>Calculated fields</h3>';
    if (!schedule.calculated.length) html += '<p class="small faint">None: a calculated field is a formula over other fields, such as Width * Height.</p>';
    for (const calc of schedule.calculated) {
      html += `<div class="stack-line" data-calc-row="${esc(calc.name)}"><span class="grow">${esc(`${calc.name} = ${calc.formula}`)}</span>`;
      html += `<button class="btn sm" data-act="stackschedcalcoff" data-a="${esc(calc.name)}" aria-label="Remove the calculated field ${esc(calc.name)}"${off}>Remove</button></div>`;
    }
    const usable = table.offered.filter((f) => f.formula_name);
    if (usable.length) html += `<p class="small faint">A formula names a field like this: ${esc(usable.map((f) => f.formula_name).join(', '))}.</p>`;
    html += `<div class="stack-line"><input class="inp" data-draft data-enter="stackschedcalcadd" data-key="sched-calc-name" placeholder="Name" aria-label="Name of the calculated field"${off}>`;
    html += `<input class="inp" data-draft data-enter="stackschedcalcadd" data-key="sched-calc-formula" placeholder="Formula" aria-label="Formula of the calculated field"${off}>`;
    html += `<select class="inp" data-draft data-key="sched-calc-kind" aria-label="What the formula gives"${off}>`;
    html += ['length', 'area', 'volume', 'angle', 'number', 'whole'].map((k) => `<option value="${k}">${k}</option>`).join('');
    html += '</select>';
    html += `<button class="btn pri sm" data-act="stackschedcalcadd" data-key="sched-calc-make"${off}>Add calculated field</button></div>`;
    return `${html}${problem(`sched-calculated:${schedule.id}`)}</section>`;
  }

  const lengthMm = (stored) => Number(String(stored).split(':').pop()) / 6400;

  /** The sheets part: where the schedule stands, a drawing of each sheet with it, and a new sheet to place it on. */
  function scheduleSheetsPart(open, off) {
    const schedule = open.table.schedule;
    const sheets = open.sheets ?? [];
    let html = '<section data-part="sheets"><h3>On sheets</h3>';
    if (!schedule.placements.length) html += '<p class="small faint">Not on any sheet.</p>';
    for (const placed of schedule.placements) {
      const sheet = sheets.find((s) => s.id === placed.sheet);
      const words = `${sheet ? `${sheet.number} ${sheet.name}` : placed.sheet}`;
      html += `<div class="stack-line" data-placement="${esc(placed.id)}"><span class="grow">${esc(words)}</span>`;
      html += `<button class="btn sm" data-act="stackschedunplace" data-a="${esc(placed.id)}" aria-label="Take the schedule off ${esc(words)}"${off}>Take off</button></div>`;
      html += scheduleSheetDrawing(open.drawn?.[placed.sheet], placed.id);
    }
    const free = sheets.filter((s) => !schedule.placements.some((p) => p.sheet === s.id));
    if (free.length) {
      html += `<div class="stack-line"><select class="inp" data-chg="stackschedsheetchoice" data-key="sched-sheet" aria-label="Sheet to place the schedule on"${off}>`;
      html += free.map((s) => `<option value="${esc(s.id)}"${s.id === open.draft.sheet ? ' selected' : ''}>${esc(`${s.number} ${s.name}`)}</option>`).join('');
      html += `</select><button class="btn pri sm" data-act="stackschedplace" data-key="sched-place"${off}>Place on sheet</button></div>`;
    }
    return `${html}${problem(`sched-sheets:${schedule.id}`)}</section>`;
  }

  /** The sheet's paper with the schedule as the server drew it from its table, in millimetres, y up the paper. */
  function scheduleSheetDrawing(listing, placement) {
    const drawn = listing?.schedules.find((s) => s.placement === placement);
    if (!listing || !drawn) return '<p class="small faint">Drawing…</p>';
    if (drawn.problem) return `<div class="note bad small" role="alert">${esc(drawn.problem)}</div>`;
    const wide = lengthMm(listing.sheet.width);
    const tall = lengthMm(listing.sheet.height);
    const x = lengthMm(drawn.x);
    const y = lengthMm(drawn.y);
    // The paper's y runs up; the drawing's runs down from the table's top left corner at (x, y).
    const top = tall - y;
    let svg = `<svg class="sched-sheet" data-sheet-preview="${esc(placement)}" viewBox="0 0 ${wide} ${tall}" role="img" aria-label="${esc(drawn.name)} on the sheet">`;
    svg += `<rect x="0" y="0" width="${wide}" height="${tall}" fill="none" stroke="currentColor" stroke-width="1"/>`;
    svg += `<g transform="translate(${x} ${top})" stroke="currentColor" stroke-width="0.25" fill="currentColor" font-size="2.5">`;
    for (const rule of drawn.rules) {
      svg += `<line x1="${lengthMm(rule.x1)}" y1="${lengthMm(rule.y1)}" x2="${lengthMm(rule.x2)}" y2="${lengthMm(rule.y2)}"/>`;
    }
    for (const text of drawn.texts) {
      svg += `<text x="${lengthMm(text.x)}" y="${lengthMm(text.y) + 3.5}" stroke="none"${text.bold ? ' font-weight="700"' : ''}>${esc(text.text)}</text>`;
    }
    return `${svg}</g></svg>`;
  }

  function scheduleHtml(open) {
    const table = open.table;
    const schedule = table.schedule;
    const off = canEdit() ? '' : ' disabled';
    const draft = open.draft;
    let html = `<p class="small muted">Lists ${esc(schedule.category_name)}: ${plural(table.rows.length, 'row')}, worked out from the model each time it changes. Choose a row to select that element; double-click a value to change it in the model.</p>`;
    if (schedule.problem) html += `<div class="note bad small" role="alert">${esc(schedule.problem)}</div>`;

    html += scheduleTable(open);
    html += problem(`sched-cell:${schedule.id}`);
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

    html += scheduleSortingPart(table, off);
    html += scheduleTotalsPart(table, off);
    html += scheduleCalculatedPart(table, off);
    html += scheduleSheetsPart(open, off);
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

  /** The sorting as the open schedule holds it, to send back whole. */
  const sortingList = () => scheduleOf().sorting.map(({ field, order, header, footer }) => ({ field, order, header, footer }));
  const sortingSet = (list) => scheduleSet('schedule.sorting', 'sorting', list, 'sorting');

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
    if (field && order) sortingSet([...sortingList(), { field, order, header: false, footer: false }]);
  };
  ACT.stackschedsortoff = (button) => sortingSet(sortingList().filter((_, at) => at !== Number(button.dataset.a)));
  ACT.stackschedsortflip = (button) =>
    sortingSet(sortingList().map((sort, at) => (at === Number(button.dataset.a) ? { ...sort, order: sort.order === 'descending' ? 'ascending' : 'descending' } : sort)));
  // A header or a footer on a key makes each group of its values one group of the table.
  CHG.stackschedsortheader = (box) => sortingSet(sortingList().map((sort, at) => (at === Number(box.dataset.a) ? { ...sort, header: box.checked } : sort)));
  CHG.stackschedsortfooter = (box) => sortingSet(sortingList().map((sort, at) => (at === Number(box.dataset.a) ? { ...sort, footer: box.checked } : sort)));

  // Totals: every instance on or off, the grand total, and each column's totals.
  CHG.stackscheditemize = (box) => manage('schedule.itemize', { schedule: C.sched.id, itemize: box.checked }, `sched-totals:${C.sched.id}`);
  CHG.stackschedgrand = (box) => manage('schedule.totals', { schedule: C.sched.id, grand: box.checked }, `sched-totals:${C.sched.id}`);
  CHG.stackschedtotalfield = (select) => {
    C.sched.draft.totalField = select.value;
  };
  CHG.stackschedtotalkind = (select) => {
    C.sched.draft.totalKind = select.value;
  };
  const totalsList = () => scheduleOf().totals.map(({ field, do: kinds }) => ({ field, do: [...kinds] }));
  const totalsSet = (list) => scheduleSet('schedule.totals', 'totals', list, 'totals');
  ACT.stackschedtotaladd = () => {
    const root = $('#stack-sched');
    const field = $('[data-key="sched-total-field"]', root)?.value;
    const kind = $('[data-key="sched-total-kind"]', root)?.value;
    if (!field || !kind) return;
    const list = totalsList();
    const held = list.find((totals) => totals.field === field);
    if (held) {
      if (!held.do.includes(kind)) held.do.push(kind);
    } else list.push({ field, do: [kind] });
    totalsSet(list);
  };
  ACT.stackschedtotaloff = (button) => {
    const list = totalsList()
      .map((totals) => (totals.field === button.dataset.field ? { ...totals, do: totals.do.filter((kind) => kind !== button.dataset.kind) } : totals))
      .filter((totals) => totals.do.length);
    totalsSet(list);
  };

  // Calculated fields: a name, a formula and what it gives; the server checks the units and the names.
  const calculatedList = () => scheduleOf().calculated.map(({ name, formula, kind }) => ({ name, formula, kind }));
  ACT.stackschedcalcadd = () => {
    const root = $('#stack-sched');
    const name = $('[data-key="sched-calc-name"]', root)?.value ?? '';
    const formula = $('[data-key="sched-calc-formula"]', root)?.value ?? '';
    const kind = $('[data-key="sched-calc-kind"]', root)?.value ?? 'length';
    manage('schedule.calculated', { schedule: C.sched.id, calculated: [...calculatedList(), { name, formula, kind }] }, `sched-calculated:${C.sched.id}`, () => {
      for (const key of ['sched-calc-name', 'sched-calc-formula']) {
        const field = $(`#stack-sched [data-key="${key}"]`);
        if (field) field.value = '';
      }
    });
  };
  ACT.stackschedcalcoff = (button) => {
    const key = `calculated:${button.dataset.a}`;
    const rest = calculatedList().filter((calc) => calc.name !== button.dataset.a);
    const set = () => manage('schedule.calculated', { schedule: C.sched.id, calculated: rest }, `sched-calculated:${C.sched.id}`);
    // A column that shows it goes first: the schedule must not list a field it no longer has.
    if (scheduleOf().fields.includes(key)) manage('schedule.fields', { schedule: C.sched.id, fields: scheduleOf().fields.filter((f) => f !== key) }, `sched-fields:${C.sched.id}`, set);
    else set();
  };

  // On sheets: the schedule placed on a sheet is its own element, drawn by the server from its table.
  CHG.stackschedsheetchoice = (select) => {
    C.sched.draft.sheet = select.value;
  };
  ACT.stackschedplace = () => {
    const sheet = $('#stack-sched [data-key="sched-sheet"]')?.value;
    if (sheet) manage('schedule.place', { placement: newId('PL'), schedule: C.sched.id, sheet }, `sched-sheets:${C.sched.id}`);
  };
  ACT.stackschedunplace = (button) => manage('schedule.unplace', { placement: button.dataset.a }, `sched-sheets:${C.sched.id}`);

  // Editing a value in place: double-click a cell, type, press Enter. (The second click of a
  // double-click is taken in the capture phase, before the row's own click redraws the table: a
  // `dblclick` event would be sent to the cell that redraw has just taken out of the page.) The value goes back to the
  // element by the server (`schedule.cell`), so it is one step to undo and the element is claimed
  // like any edit. A value that is worked out or held elsewhere is not offered as an input: the
  // server's words say why, where the person asked.
  document.addEventListener(
    'click',
    (event) => {
      if (event.detail !== 2) return;
      const cell = event.target.closest?.('#stack-sched td[data-cell]');
      const open = C.sched;
      if (!cell || !open?.table || !canEdit()) return;
      const column = open.table.columns.find((c) => c.field === cell.dataset.cell);
      if (!column) return;
      const where = `sched-cell:${open.id}`;
      if (!column.editable) {
        manage('schedule.cell', { schedule: open.id, element: cell.dataset.element, field: column.field, value: cell.textContent }, where);
        return;
      }
      C.problems.delete(where);
      open.editing = { element: cell.dataset.element, field: column.field };
      renderSchedule();
      $('#stack-sched [data-key="sched-cell"]')?.focus();
    },
    true,
  );
  window.addEventListener(
    'keydown',
    (event) => {
      if (event.key === 'Escape' && event.target.dataset?.key === 'sched-cell' && C.sched?.editing) {
        // Escape leaves the value alone, not the dialog.
        event.stopPropagation();
        event.preventDefault();
        C.sched.editing = null;
        renderSchedule();
      }
    },
    true,
  );
  ACT.stackschedcellset = (input) => {
    const open = C.sched;
    if (!open?.editing) return;
    const { element, field } = open.editing;
    manage('schedule.cell', { schedule: open.id, element, field, value: input.value }, `sched-cell:${open.id}`, () => {
      if (C.sched === open) open.editing = null;
      renderSchedule();
    });
  };

  // --- The wall tool ---
  // A wall is drawn as a line between two clicked points, of the wall type chosen in the properties
  // palette, its base on the plan's level and its top on the level chosen there or a height typed
  // there: one `wall.draw` edit (construct-model's FORMAT.md, "Walls"). The wall types, their layers
  // and the colours their materials are drawn in come from the server; where a model has no wall
  // types, the page sends the server's default ones through the log (`wall.type.create`). A refusal
  // is the server's reason, under the tool's options.

  C.wallTypes = null; // { key, view } of the open model
  C.wallTool = { type: null, top: undefined, height: '3000 mm' }; // top: a level's ID, '' for a height, undefined for the level above
  C.madeDefaults = null; // the model the default wall types were sent for

  /** The open model's wall types, materials and defaults; null until they have come. */
  function wallTypes() {
    if (!C.model) return null;
    const model = C.model.id;
    const key = `${model}/${C.paramsEpoch}`;
    if (C.wallTypes?.key !== key && C.wallTypesLoading !== key) {
      C.wallTypesLoading = key;
      api(`${modelUrl(model)}/wall-types`)
        .then((view) => {
          C.wallTypes = { key, view };
          refreshSide();
          queueDraw();
        })
        .catch(() => {});
    }
    return C.wallTypes?.key.startsWith(`${model}/`) ? C.wallTypes.view : null;
  }

  /** A material's colour where a view cuts it (`cut`) or seen beyond (`surface`); null for a material not listed. */
  function colourOf(material, which) {
    const found = wallTypes()?.materials.find((m) => m.id === material);
    return found ? found[which] : null;
  }

  const materialName = (view, id) => view.materials.find((m) => m.id === id)?.name ?? id;

  function chosenWallType(view) {
    return view.types.find((t) => t.id === C.wallTool.type) ?? view.types[0] ?? null;
  }

  /** The top the tool draws to: a level's ID, or '' for a height. By default the level above the plan's. */
  function wallTop(levels) {
    const here = levelOf();
    const others = (levels || []).filter((level) => level.id !== here);
    const chosen = C.wallTool.top;
    if (chosen === '' || others.some((level) => level.id === chosen)) return chosen;
    const at = (levels || []).findIndex((level) => level.id === here);
    return (at >= 0 && levels[at + 1]?.id) || '';
  }

  /** Sends the server's default wall types, once for a model, where it has none. */
  function makeDefaultWalls(view) {
    if (!canEdit() || C.madeDefaults === C.model.id) return;
    C.madeDefaults = C.model.id;
    for (const args of view.defaults) manage('wall.type.create', args, 'wall-draw');
  }

  function wallToolHtml() {
    let html = typeSel('wall', 'Wall', '<b>New wall</b>');
    const view = wallTypes();
    if (!view) return `${html}<p class="pad small faint">Loading the wall types.</p>`;
    if (!view.types.length) {
      queueMicrotask(() => makeDefaultWalls(view));
      return `${html}<p class="pad small faint" data-wall-tool="making">This model has no wall types yet, so the default ones are being made.</p>${problem('wall-draw')}`;
    }
    const chosen = chosenWallType(view);
    const levels = modelLevels() || [];
    const here = levelOf();
    const top = wallTop(levels);
    const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
    const tops = levels
      .filter((level) => level.id !== here)
      .map((level) => `<option value="${esc(level.id)}"${level.id === top ? ' selected' : ''}>${esc(level.name)}</option>`);
    tops.push(`<option value=""${top === '' ? ' selected' : ''}>A height</option>`);
    const base = levels.find((level) => level.id === here)?.name ?? here;
    html += '<table class="props">';
    html += prow('Type', `<select class="inp" data-chg="stackwalltype" data-key="wall-type" aria-label="Wall type">${types.join('')}</select>`);
    html += prow('Thickness', esc(chosen.thickness));
    html += prow('Base', esc(base));
    html += prow('Top', `<select class="inp" data-chg="stackwalltop" data-key="wall-top" aria-label="Top of the wall">${tops.join('')}</select>`);
    if (top === '') {
      html += prow('Height', `<input class="inp" data-chg="stackwallheight" data-key="wall-height" value="${esc(C.wallTool.height)}" aria-label="Height of the wall">`);
    }
    html += '</table>';
    html += `<span class="lbl">Layers, exterior first</span><table class="props" data-layers="${esc(chosen.id)}">`;
    for (const layer of chosen.assembly.layers) {
      html += prow(esc(capital(layer.function)), `${esc(layer.thickness)} · ${esc(materialName(view, layer.material))}`);
    }
    html += '</table>';
    html += '<p class="pad small muted">Pick two points in the plan. Walking from the first to the second, the exterior is on the right.</p>';
    return html + problem('wall-draw');
  }

  CHG.stackwalltype = (select) => {
    C.wallTool.type = select.value;
    refreshSide();
  };
  CHG.stackwalltop = (select) => {
    C.wallTool.top = select.value;
    refreshSide();
  };
  CHG.stackwallheight = (input) => {
    C.wallTool.height = input.value;
  };

  // --- The floor tool, and a floor in the plan and the properties palette ---
  // A floor is drawn from a boundary sketched by clicks in the plan: its edge, a closed loop of
  // corners, then any holes, each closed on its first corner or with Enter; Enter on no open loop,
  // or Make floor, sends one `floor.create` edit of the floor type chosen in the properties palette,
  // its top on the plan's level the offset typed there above it (construct-model's FORMAT.md,
  // "Floors"). The page checks nothing and works out no geometry: the floor types, their layers and
  // the materials' colours come from the server (`/floor-types`); where a model has none the page
  // has the server make its default ones through the log (`floor.defaults`); a refusal is the
  // server's reason, under the tool's options, with the boundary it turned back drawn in the plan.
  // In the plan a floor is its edge and holes, from its recipe as the server filled it
  // (`/filled/<element>`), in its top layer's colour seen from above; a selected floor shows its
  // type, its level and offset, its parameters, and apart from them what the server works out of
  // its boundary and its type (`from: "read-only"` in `/parameters/<element>`), which nobody sets.

  const FLOOR_OP = 'floor';
  const READ_ONLY = 'read-only';
  const CLOSE_PX = 10; // a click this near a loop's first corner on screen closes the loop
  const isFloor = (record) => record?.kind === 'floor' && record?.recipe?.op === FLOOR_OP;
  /** How the plan stacks what it draws: floors under walls, walls under the rest, so a door in a wall stays on top. */
  const drawOrder = (el) => (el.kind === 'roof' ? -2 : el.kind === 'room' ? -1 : isFloor(el) ? 0 : el.kind === 'wall' ? 1 : 2);

  DTOOLS.architecture.push(['floor', 'Floor', 'layers', 'Click the corners of its edge, then any holes. Close each on its first corner or with Enter.']);
  TOOLNAME.floor = 'floor';
  Object.assign(WORDS, {
    'floor.create': 'drew a floor',
    'floor.boundary': 'changed the boundary of',
    'floor.defaults': 'made the default floor types',
    'floor.type.create': 'made a floor type',
    'floor.type.layers': 'changed the layers of a floor type',
    'floor.type.rename': 'renamed a floor type',
    'floor.type.duplicate': 'copied a floor type',
    'floor.type.delete': 'deleted a floor type',
  });
  Object.assign(STEP_WORDS, {
    'floor.create': 'drawing a floor',
    'floor.boundary': 'a floor’s boundary',
    'floor.defaults': 'making the default floor types',
    'floor.type.create': 'making a floor type',
    'floor.type.layers': 'the layers of a floor type',
    'floor.type.rename': 'renaming a floor type',
    'floor.type.duplicate': 'copying a floor type',
    'floor.type.delete': 'deleting a floor type',
  });

  C.floorTypes = null; // { key, view } of the open model
  C.floorTool = { type: null, offset: '0 mm' };
  C.madeFloorDefaults = null; // the model the default floor types were asked for
  // The sketch, in the model's millimetres: closed loops (the edge first, then holes), the loop
  // being drawn, the point under the pointer, and the boundary last sent while it may be turned back.
  const FLOOR = { loops: [], current: [], at: null, sent: null };

  /** The open model's floor types and materials; null until they have come. */
  function floorTypes() {
    if (!C.model) return null;
    const model = C.model.id;
    const key = `${model}/${C.paramsEpoch}`;
    if (C.floorTypes?.key !== key && C.floorTypesLoading !== key) {
      C.floorTypesLoading = key;
      api(`${modelUrl(model)}/floor-types`)
        .then((view) => {
          C.floorTypes = { key, view };
          refreshSide();
        })
        .catch(() => {});
    }
    return C.floorTypes?.key.startsWith(`${model}/`) ? C.floorTypes.view : null;
  }

  /** The open model's floor types, read now if they have not come yet. */
  async function floorTypesNow() {
    const view = floorTypes();
    if (view) return view;
    const model = C.model.id;
    const fresh = await api(`${modelUrl(model)}/floor-types`);
    C.floorTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
    return fresh;
  }

  const chosenFloorType = (view) => view.types.find((t) => t.id === C.floorTool.type) ?? view.types[0] ?? null;

  /** Has the server make its default floor types, once for a model, where it has none. */
  function makeDefaultFloors() {
    if (!canEdit() || C.madeFloorDefaults === C.model.id) return;
    C.madeFloorDefaults = C.model.id;
    manage('floor.defaults', {}, 'floor-draw');
  }

  /** What the sketch holds, and the button that sends it. */
  const sketching = () => S.tool === 'floor' || S.tool === 'roof';
  function sketchHtml() {
    const noun = S.tool === 'roof' ? 'roof' : 'floor';
    const [outer, ...holes] = FLOOR.loops;
    let state = 'empty';
    let text = `Click the corners of the ${noun}’s ${noun === 'roof' ? 'footprint' : 'edge'} in the plan. Close it on its first corner or with Enter.`;
    if (FLOOR.current.length) {
      state = 'open';
      const which = outer ? `Hole ${holes.length + 1}` : 'Edge';
      text = `${which}: ${plural(FLOOR.current.length, 'corner')}. Close it on its first corner or with Enter.`;
    } else if (outer) {
      state = 'closed';
      const holeText = holes.length ? `, ${plural(holes.length, 'hole')}` : '';
      text = `Edge: ${plural(outer.length, 'corner')}${holeText}. Click to start a hole, or press Enter to make the ${noun}.`;
    }
    const ready = state === 'closed' ? '' : ' disabled';
    return `<p class="pad small muted" data-${noun}-sketch="${state}">${esc(text)}</p><div class="pad"><button class="btn pri sm" data-act="stack${noun}make" data-key="${noun}-make"${ready}>Make ${noun}</button></div>`;
  }

  function floorToolHtml() {
    let html = typeSel('layers', 'Floor', '<b>New floor</b>');
    const view = floorTypes();
    if (!view) return `${html}<p class="pad small faint">Loading the floor types.</p>`;
    if (!view.types.length) {
      queueMicrotask(makeDefaultFloors);
      return `${html}<p class="pad small faint" data-floor-tool="making">This model has no floor types yet, so the default ones are being made.</p>${problem('floor-draw')}`;
    }
    const chosen = chosenFloorType(view);
    const here = levelOf();
    const level = (modelLevels() || []).find((l) => l.id === here)?.name ?? here;
    const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
    html += '<table class="props">';
    html += prow('Type', `<select class="inp" data-chg="stackfloortype" data-key="floor-type" aria-label="Floor type">${types.join('')}</select>`);
    html += prow('Thickness', esc(chosen.thickness));
    html += prow('Level', esc(level));
    html += prow('Offset', `<input class="inp" data-chg="stackflooroffset" data-key="floor-offset" value="${esc(C.floorTool.offset)}" aria-label="Offset of the floor’s top above its level">`);
    html += '</table>';
    html += `<span class="lbl">Layers, top first</span><table class="props" data-floor-layers="${esc(chosen.id)}">`;
    for (const layer of chosen.layers) {
      html += prow(esc(capital(layer.function)), `${esc(layer.thickness)} · ${esc(materialName(view, layer.material))}`);
    }
    html += '</table>';
    return html + sketchHtml() + problem('floor-draw');
  }

  CHG.stackfloortype = (select) => {
    C.floorTool.type = select.value;
    refreshSide();
  };
  CHG.stackflooroffset = (input) => {
    C.floorTool.offset = input.value;
  };
  ACT.stackfloormake = () => makeFloor();

  function floorHint() {
    if (S.tool === 'roof') return roofHint();
    if (FLOOR.current.length) return '<b>Floor:</b> click the next corner, or its first corner or Enter to close the loop. Esc to stop.';
    if (FLOOR.loops.length) return '<b>Floor:</b> click to start a hole, or press Enter to make the floor. Esc to stop.';
    return '<b>Floor:</b> click the corners of its edge. Choose its type in the properties palette.';
  }

  function sketchChanged() {
    $('#hint').innerHTML = hint();
    refreshSide();
    queueDraw();
  }

  function clearSketch() {
    Object.assign(FLOOR, { loops: [], current: [], at: null, sent: null });
  }

  /** Whether a click at `at` (the model's millimetres) is within reach of `point` on screen. */
  function nearOnScreen(point, at) {
    const scale = $('#pv-canvas svg')?.getScreenCTM()?.a || 1;
    return Math.hypot(point[0] - at.x, point[1] - at.y) * scale <= CLOSE_PX;
  }

  function closeLoop() {
    if (!FLOOR.current.length) return;
    FLOOR.loops.push(FLOOR.current);
    FLOOR.current = [];
    FLOOR.at = null;
    sketchChanged();
  }

  function floorClick(event) {
    const at = toModel(event);
    const point = [snap(at.x), snap(at.y)];
    FLOOR.sent = null;
    if (FLOOR.current.length > 1 && nearOnScreen(FLOOR.current[0], at)) {
      closeLoop();
      return;
    }
    const last = FLOOR.current.at(-1);
    if (!last || last[0] !== point[0] || last[1] !== point[1]) FLOOR.current.push(point);
    sketchChanged();
  }

  /** Sends the sketch as it is: what the server makes of it, or why it refuses, comes back through the log. */
  async function makeFloor() {
    if (!FLOOR.loops.length || FLOOR.current.length || !canEdit()) return;
    let view;
    try {
      view = await floorTypesNow();
    } catch (error) {
      say(error.message);
      return;
    }
    const corners = (loop) => loop.map(([x, y]) => ({ at: [`${x} mm`, `${y} mm`] }));
    const [outer, ...holes] = FLOOR.loops;
    const args = { element: newId('F'), level: levelOf(), boundary: { outer: corners(outer), holes: holes.map(corners) } };
    const type = chosenFloorType(view);
    if (type) args.type = type.id;
    if (C.floorTool.offset.trim() !== '') args.offset = C.floorTool.offset;
    const sent = { loops: FLOOR.loops };
    Object.assign(FLOOR, { loops: [], current: [], at: null, sent });
    manage('floor.create', args, 'floor-draw', () => {
      if (FLOOR.sent === sent) FLOOR.sent = null;
      queueDraw();
    });
    sketchChanged();
  }

  // In the plan, before the elements' own handlers: the floor tool's corners.
  document.addEventListener(
    'pointerdown',
    (event) => {
      if (!sketching() || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
      event.stopPropagation();
      if (!canEdit()) say('Open a model in your team’s WIP to add to it.');
      else floorClick(event);
    },
    true,
  );
  window.addEventListener('pointermove', (event) => {
    if (!sketching() || !FLOOR.current.length || !$('#pv-canvas svg')) return;
    const at = toModel(event);
    FLOOR.at = [snap(at.x), snap(at.y)];
    queueDraw();
  });
  // Escape leaves the tool (the demo's own key) and drops the sketch; Enter closes the loop being
  // drawn, or makes the floor when none is open.
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      clearSketch();
      return;
    }
    if (!sketching() || event.key !== 'Enter' || event.target.closest?.('input,textarea,button')) return;
    event.preventDefault();
    if (FLOOR.current.length) closeLoop();
    else if (S.tool === 'roof') makeRoof();
    else makeFloor();
  });
  const toolBeforeFloors = ACT.tool;
  ACT.tool = (button) => {
    clearSketch();
    toolBeforeFloors(button);
  };

  /** The ribbon: the floor tool beside the wall tool, in the Build panel. */
  function floorRibbon(config) {
    const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
    if (!build || build.items.some((item) => item.a === 'architecture:floor')) return;
    const wall = build.items.findIndex((item) => item.a === 'architecture:wall');
    build.items.splice(wall + 1, 0, DT_('architecture', 'floor'));
  }

  const ringPath = (points) => `M${points.map(([x, y]) => `${x},${y}`).join('L')}Z`;

  /** The sketch in the plan: closed loops, the loop being drawn up to the pointer, and a boundary the server turned back. */
  function floorSketch() {
    let svg = pickInPlan();
    if (!sketching()) return svg;
    const where = S.tool === 'roof' ? 'roof-draw' : 'floor-draw';
    if (FLOOR.sent && C.problems.has(where)) {
      svg += `<path class="floor-refused" data-${S.tool}-refused="${FLOOR.sent.loops.length}" d="${FLOOR.sent.loops.map(ringPath).join('')}"/>`;
    }
    for (const loop of FLOOR.loops) svg += `<path class="ln ap floor-sketch" d="${ringPath(loop)}"/>`;
    const open = FLOOR.at && FLOOR.current.length ? [...FLOOR.current, FLOOR.at] : FLOOR.current;
    if (open.length > 1) svg += `<polyline class="ln ap dd floor-sketch" points="${open.map((p) => p.join(',')).join(' ')}"/>`;
    for (const [x, y] of FLOOR.current) svg += `<circle class="apf floor-sketch" cx="${x}" cy="${y}" r="60"/>`;
    return svg;
  }

  /**
   * A floor in the plan: its edge and holes, from its recipe as the server filled it (the outline of
   * its display, which has no holes, until that has come), in its top layer's colour seen from above.
   */
  function floorInPlan(piece, data, scale) {
    const { el: e } = piece;
    const { x, y } = placementOf(e);
    const loops = piece.fill?.recipe?.args?.loops;
    const rings = Array.isArray(loops) && loops.length
      ? loops.map((loop) => loop.map((corner) => [nearestMm(corner.at[0]), nearestMm(corner.at[1])]))
      : [piece.shape.displays[0].outline];
    // The openings and shafts that cut it, as the server says (`/filled`), are more holes.
    for (const cut of piece.fill?.cut_by?.cuts ?? []) rings.push(cut.corners.map(([cx, cy]) => [nearestMm(cx), nearestMm(cy)]));
    const placed = rings.map((ring) => ring.map(([px, py]) => [x + px, y + py]));
    const top = piece.shape.displays[0]?.parts?.[0];
    const colour = top && colourOf(top.material, 'surface');
    const fill = colour ? ` style="fill:${esc(colour)}"` : '';
    const box = around(piece);
    const chosen = S.sel === e.id;
    let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
    group += `<path class="body floor${chosen ? ' sel' : ''}" data-material="${esc(top?.material ?? '')}" data-holes="${placed.length - 1}"${fill} d="${placed.map(ringPath).join('')}"/>`;
    group += floorShapeInPlan(piece, x, y, scale);
    const claim = heldClaim(e.id);
    if (claim) {
      const theirs = claim.person !== C.me.person.id;
      group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
      if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
    }
    return `<g ${data}>${group}</g>`;
  }

  /** What the server works out of a floor, apart from what a person sets, and set by nobody. */
  function readOnlyTable(view) {
    const rows = view.parameters.filter((p) => p.from === READ_ONLY);
    if (!rows.length) return '';
    let html = '<span class="lbl">Worked out, read-only</span><table class="props" data-read-only-rows>';
    for (const p of rows) html += prow(esc(p.name), `<span class="mono" data-read-only="${esc(p.id)}">${esc(p.shown ?? '')}</span>`);
    return `${html}</table>`;
  }

  /** A floor's offset above its level, as stored. */
  function floorOffset(e) {
    const stored = e.props?.['@level.offset'];
    return `${stored === undefined ? 0 : storedMm(stored)} mm`;
  }

  /** What the properties palette shows of a selected floor. */
  function floorProperties(piece) {
    const { el: e, shape, failed } = piece;
    const view = paramsFor(piece);
    const claim = heldClaim(e.id);
    let html = typeSel('layers', 'Floor', `<b>${esc(e.name)}</b>`);
    html += '<table class="props" data-floor-element>';
    if (view) html += prow('Category', esc(view.category?.name || 'None')) + prow('Type', typeCell(view));
    html += prow('Level', esc(levelName(e))) + prow('Offset', `<span class="mono" data-floor-offset>${esc(floorOffset(e))}</span>`);
    if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
    html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
    html += floorShapeHtml(piece);
    if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
    if (view) {
      html += readOnlyTable(view);
      html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
    }
    return html;
  }

  // --- A floor's slope and slab edges, and its sketched outline ---
  // A selected floor's palette sets its slope (an arrow drawn in the plan by two clicks, with the top
  // face's two heights typed beside it: `floor.slope`; `floor.slope.clear`) and its slab edges (a
  // profile typed, then the floor's edges picked in the plan: `floor.edge`; `floor.edge.remove`).
  // The page checks nothing and works out no geometry: the plan draws the arrow from the floor's
  // record and its slab edges from its recipe as the server filled it, 3D draws what the server
  // built, the quantities are the server's read-only rows, and a refusal (a slope and slab edges on
  // one floor, a corner off the grid) is the server's reason under the part it came from.
  // A sketched outline's dimensions are changed by `floor.dimension`, which the page does not offer
  // yet: it has no way to show a sketch's dimensions (construct-model's FORMAT.md, "Floors").

  Object.assign(WORDS, {
    'floor.slope': 'sloped',
    'floor.slope.clear': 'took the slope off',
    'floor.edge': 'put a slab edge on',
    'floor.edge.remove': 'took a slab edge off',
    'floor.dimension': 'changed a dimension of',
  });
  Object.assign(STEP_WORDS, {
    'floor.slope': 'a floor’s slope',
    'floor.slope.clear': 'taking a floor’s slope off',
    'floor.edge': 'a floor’s slab edge',
    'floor.edge.remove': 'taking a slab edge off',
    'floor.dimension': 'a floor’s dimension',
  });

  C.slopeTool = { atTail: '0 mm', atHead: '-40 mm' };
  C.edgeTool = { kind: 'downturn', width: '300 mm', foot: '150 mm', depth: '400 mm' };
  // What the plan is waiting for: { floor, what: 'slope', tail: [x, y] | null } for the arrow's two
  // clicks, or { floor, what: 'edge' } for slab edges picked on the floor's sides.
  const PICK = { now: null };

  const mmPoint = (stored) => [nearestMm(stored[0]), nearestMm(stored[1])];

  /** A floor's slope as its record holds it, or null. */
  function slopeOf(e) {
    const slope = e.recipe?.args?.slope;
    if (!slope) return null;
    const at = (key) => (slope[key] === undefined ? null : `${storedMm(slope[key])} mm`);
    return {
      tail: mmPoint(slope.tail),
      head: mmPoint(slope.head),
      atTail: at('at_tail'),
      atHead: at('at_head'),
      gradient: slope.slope ? typedPitch(slope.slope) : null,
    };
  }

  /** A floor's slab edges as its record holds them: `[{ edge, kind, width, foot, depth }]`. */
  function slabEdgesOf(e) {
    const list = e.recipe?.args?.edges;
    if (!Array.isArray(list)) return [];
    const mm = (text) => (text === undefined ? '' : `${storedMm(text)} mm`);
    return list.map((s) => ({ edge: s.edge, kind: s.kind, width: mm(s.width), foot: mm(s.foot), depth: mm(s.depth) }));
  }

  /** The slope and slab edges part of a selected floor's palette. */
  function floorShapeHtml(piece) {
    const { el: e } = piece;
    const editable = canEdit();
    const slope = slopeOf(e);
    const edges = slabEdgesOf(e);
    const field = (tool, key, label) => `<input class="inp" data-chg="stackfloorfield" data-tool="${tool}" data-a="${key}" data-key="floor-${tool}-${key}" value="${esc(C[`${tool}Tool`][key])}" aria-label="${esc(label)}">`;
    let html = '<span class="lbl">Slope</span><table class="props" data-floor-slope-part>';
    if (slope) {
      const run = slope.gradient ? `rises ${esc(slope.gradient)}` : `${esc(slope.atTail)} to ${esc(slope.atHead)}`;
      html += prow('Now', `<span class="mono" data-floor-slope-now>${run}</span>`);
    } else {
      html += prow('Now', '<span class="faint" data-floor-slope-now>Flat</span>');
    }
    if (editable) {
      html += prow('Height at tail', field('slope', 'atTail', 'The top face’s height at the arrow’s tail, from the floor’s own height'));
      html += prow('Height at head', field('slope', 'atHead', 'The top face’s height at the arrow’s head'));
    }
    html += '</table>';
    if (editable) {
      const waiting = PICK.now?.floor === e.id && PICK.now.what === 'slope';
      const hint = waiting ? `<p class="pad small muted" data-floor-pick="${PICK.now.tail ? 'head' : 'tail'}">${PICK.now.tail ? 'Click where the arrow ends.' : 'Click where the arrow starts.'}</p>` : '';
      html += `${hint}<div class="pad"><button class="btn sm" data-act="stackslopepick" data-key="floor-slope-pick">${waiting ? 'Stop drawing' : slope ? 'Draw it again' : 'Draw slope arrow'}</button>`;
      if (slope) html += ' <button class="btn sm" data-act="stackslopeclear" data-key="floor-slope-clear">Clear slope</button>';
      html += '</div>';
    }
    html += problem('floor-slope');
    html += '<span class="lbl">Slab edges</span>';
    if (edges.length) {
      html += '<table class="props" data-floor-slab-edges>';
      for (const s of edges) {
        const sizes = s.kind === 'thickened' ? `${s.width} wide, ${s.foot} foot, ${s.depth} deep` : `${s.width} wide, ${s.depth} deep`;
        const remove = editable ? ` <button class="btn sm" data-act="stackedgeremove" data-edge="${esc(s.edge)}" data-key="floor-edge-remove-${esc(s.edge)}">Remove</button>` : '';
        html += prow(esc(s.edge), `<span data-slab-edge-row="${esc(s.edge)}">${esc(capital(s.kind))}: ${esc(sizes)}</span>${remove}`);
      }
      html += '</table>';
    } else {
      html += '<p class="pad small faint" data-floor-slab-edges="none">None.</p>';
    }
    if (editable) {
      const kinds = [['downturn', 'Downturn'], ['thickened', 'Thickened']].map(([id, name]) => `<option value="${id}"${id === C.edgeTool.kind ? ' selected' : ''}>${name}</option>`);
      html += '<table class="props">';
      html += prow('Edge', `<select class="inp" data-chg="stackedgekind" data-key="floor-edge-kind" aria-label="Slab edge profile">${kinds.join('')}</select>`);
      html += prow('Width', field('edge', 'width', 'Width in from the edge'));
      if (C.edgeTool.kind === 'thickened') html += prow('Foot', field('edge', 'foot', 'Width at the bottom'));
      html += prow('Depth', field('edge', 'depth', 'Depth below the floor'));
      html += '</table>';
      const waiting = PICK.now?.floor === e.id && PICK.now.what === 'edge';
      const hint = waiting ? '<p class="pad small muted" data-floor-pick="edge">Click the floor’s edges in the plan.</p>' : '';
      html += `${hint}<div class="pad"><button class="btn sm" data-act="stackedgepick" data-key="floor-edge-pick">${waiting ? 'Stop picking' : 'Pick edges'}</button></div>`;
    }
    return html + problem('floor-edge');
  }

  CHG.stackfloorfield = (input) => {
    C[`${input.dataset.tool}Tool`][input.dataset.a] = input.value;
  };
  CHG.stackedgekind = (select) => {
    C.edgeTool.kind = select.value;
    refreshSide();
  };

  /** Starts or stops waiting for clicks in the plan for the selected floor. */
  function startPick(what) {
    const stopping = PICK.now?.floor === S.sel && PICK.now.what === what;
    PICK.now = stopping || !S.sel ? null : { floor: S.sel, what, tail: null };
    clearProblems(/^floor-(slope|edge)$/);
    refreshSide();
    queueDraw();
  }
  ACT.stackslopepick = () => startPick('slope');
  ACT.stackedgepick = () => startPick('edge');
  ACT.stackslopeclear = () => {
    if (S.sel) manage('floor.slope.clear', { element: S.sel }, 'floor-slope');
  };
  ACT.stackedgeremove = (button) => {
    if (S.sel) manage('floor.edge.remove', { element: S.sel, edge: button.dataset.edge }, 'floor-edge');
  };

  /** The arrow's two clicks: its tail, then its head, which sends the slope with the heights typed. */
  function slopeClick(at) {
    const point = [snap(at.x), snap(at.y)];
    if (!PICK.now.tail) {
      PICK.now.tail = point;
      refreshSide();
      queueDraw();
      return;
    }
    const [tail, floor] = [PICK.now.tail, PICK.now.floor];
    PICK.now = null;
    if (tail[0] === point[0] && tail[1] === point[1]) {
      refreshSide();
      queueDraw();
      return;
    }
    const slope = {
      tail: tail.map((v) => `${v} mm`),
      head: point.map((v) => `${v} mm`),
      at_tail: C.slopeTool.atTail,
      at_head: C.slopeTool.atHead,
    };
    manage('floor.slope', { element: floor, slope }, 'floor-slope');
    queueDraw();
  }

  /** A click on one of the selected floor's sides while edges are being picked. */
  function edgeClick(side) {
    const tool = C.edgeTool;
    const profile = { kind: tool.kind, width: tool.width, depth: tool.depth };
    if (tool.kind === 'thickened') profile.foot = tool.foot;
    manage('floor.edge', { element: PICK.now.floor, edge: side.dataset.floorEdge, profile }, 'floor-edge');
  }

  // Before the plan's own handlers, as the floor tool's clicks are.
  document.addEventListener(
    'pointerdown',
    (event) => {
      if (!PICK.now || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
      if (S.sel !== PICK.now.floor || !canEdit()) return;
      if (PICK.now.what === 'slope') {
        event.stopPropagation();
        slopeClick(toModel(event));
        return;
      }
      // A click that is not on one of the floor's sides is the plan's own (it picks something else).
      const side = event.target.closest('[data-floor-edge]');
      if (!side) return;
      event.stopPropagation();
      edgeClick(side);
    },
    true,
  );
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && PICK.now) {
      PICK.now = null;
      refreshSide();
      queueDraw();
    }
  });

  /**
   * A floor's slope arrow and slab edges in the plan, from its record and its recipe as the server
   * filled it: the arrow with its two heights, each slab edge's plan at the floor's underside, and,
   * while edges are picked, a stripe along each side of the chosen floor to click.
   */
  function floorShapeInPlan(piece, x, y, scale) {
    const { el: e } = piece;
    let svg = '';
    for (const s of piece.fill?.recipe?.args?.edges ?? []) {
      const ring = s.top.map((p) => [x + nearestMm(p[0]), y + nearestMm(p[1])]);
      svg += `<path class="floor-slab-edge" data-slab-edge="${esc(s.id)}" data-kind="${esc(s.kind)}" d="${ringPath(ring)}"/>`;
    }
    const slope = slopeOf(e);
    if (slope) {
      const [[ax, ay], [bx, by]] = [slope.tail, slope.head].map(([px, py]) => [x + px, y + py]);
      const length = Math.hypot(bx - ax, by - ay) || 1;
      const [ux, uy] = [(bx - ax) / length, (by - ay) / length];
      const back = (n) => [bx - ux * n - uy * n * 0.4, by - uy * n + ux * n * 0.4];
      const other = (n) => [bx - ux * n + uy * n * 0.4, by - uy * n - ux * n * 0.4];
      const head = [[bx, by], back(250), other(250)];
      svg += `<g class="floor-slope" data-floor-slope="${esc(e.id)}"><line x1="${ax}" y1="${ay}" x2="${bx}" y2="${by}"/><path d="${ringPath(head)}"/>`;
      svg += `${TX(ax, ay - 120, scale, slope.atTail ?? '', 'tf', 220)}${TX(bx, by - 120, scale, slope.atHead ?? slope.gradient ?? '', 'tf', 220)}</g>`;
    }
    if (PICK.now?.what === 'edge' && PICK.now.floor === e.id) {
      for (const ring of piece.fill?.recipe?.args?.loops ?? []) {
        ring.forEach((corner, k) => {
          const next = ring[(k + 1) % ring.length];
          const [p, q] = [corner, next].map((c) => [x + nearestMm(c.at[0]), y + nearestMm(c.at[1])]);
          svg += `<line class="floor-edge-hit" data-floor-edge="${esc(corner.id)}" x1="${p[0]}" y1="${p[1]}" x2="${q[0]}" y2="${q[1]}"/>`;
        });
      }
    }
    return svg;
  }

  /** The arrow's tail once it is clicked, drawn while the head is awaited. */
  function pickInPlan() {
    const tail = PICK.now?.what === 'slope' ? PICK.now.tail : null;
    return tail ? `<circle class="apf floor-sketch" data-floor-slope-tail cx="${tail[0]}" cy="${tail[1]}" r="60"/>` : '';
  }

  /** The properties palette's floor part: the floor tool's options, or a selected floor; '' for anything else. */
  function floorPalette(piece) {
    if (S.tool === 'floor' && canEdit()) return floorToolHtml();
    if (piece && isFloor(piece.el) && !(S.tool === 'wall' && canEdit())) return floorProperties(piece);
    return '';
  }

  // --- The roof tool, and a roof in the plan and the properties palette ---
  // A roof is made over a footprint sketched by clicks in the plan, as a floor's edge is: one loop
  // of corners closed on its first corner or with Enter; Enter on no open loop, or Make roof,
  // sends one `roof.create` of the roof type, form, pitch and overhang chosen in the properties
  // palette, its base on the plan's level the offset typed there above it (construct-model's
  // FORMAT.md, "Roofs"). The page checks nothing and works out no geometry: the roof types, their
  // layers and the materials come from the server (`/roof-types`); where a model has none the page
  // has the server make its default ones through the log (`roof.defaults`); a refusal (two
  // pitches meeting at a hip, a pitch on a footprint that is not right-angled, a hole) is the server's reason in the
  // server's words, under the tool's options, with the footprint it turned back drawn in the plan.
  // In the plan a roof is its eaves outline from its recipe as the server filled it
  // (`/filled/<element>`) and, where it slopes, the lines of its top face (ridge, hips, valleys,
  // verges) as the server listed them there; a selected roof shows its type, level, form (flat,
  // mono-pitch, gable or hip), pitch, a pitch for each edge it rises from where there are two or
  // more (an uneven gable), and overhang, which are changed there (`roof.retype`, `roof.form`,
  // `roof.pitch` with or without `edge`, `roof.overhang`), what the server works out of it, and
  // Delete removes it (`roof.delete`).

  const ROOF_OP = 'roof';
  const isRoof = (record) => record?.kind === 'roof' && record?.recipe?.op === ROOF_OP;

  DTOOLS.architecture.push(['roof', 'Roof', 'family', 'Click the corners of its footprint. Close the loop on its first corner or with Enter.']);
  TOOLNAME.roof = 'roof';
  Object.assign(WORDS, {
    'roof.create': 'made a roof',
    'roof.footprint': 'changed the footprint of',
    'roof.form': 'changed the form of',
    'roof.pitch': 'changed the pitch of',
    'roof.overhang': 'changed the overhang of',
    'roof.level': 'tied to a level',
    'roof.retype': 'changed the type of',
    'roof.delete': 'deleted',
    'roof.defaults': 'made the default roof types',
    'roof.type.create': 'made a roof type',
    'roof.type.layers': 'changed the layers of a roof type',
    'roof.type.rename': 'renamed a roof type',
    'roof.type.duplicate': 'copied a roof type',
    'roof.type.delete': 'deleted a roof type',
  });
  Object.assign(STEP_WORDS, {
    'roof.create': 'making a roof',
    'roof.footprint': 'a roof’s footprint',
    'roof.form': 'a roof’s form',
    'roof.pitch': 'a roof’s pitch',
    'roof.overhang': 'a roof’s overhang',
    'roof.level': 'a roof’s level',
    'roof.retype': 'a change of a roof’s type',
    'roof.delete': 'deleting a roof',
    'roof.defaults': 'making the default roof types',
    'roof.type.create': 'making a roof type',
    'roof.type.layers': 'the layers of a roof type',
    'roof.type.rename': 'renaming a roof type',
    'roof.type.duplicate': 'copying a roof type',
    'roof.type.delete': 'deleting a roof type',
  });

  C.roofTypes = null; // { key, view } of the open model
  C.roofTool = { type: null, form: 'gable', pitch: '1:4', overhang: '300 mm', offset: '0 mm', eaves: '' };
  C.madeRoofDefaults = null; // the model the default roof types were asked for

  /** The open model's roof types and materials; null until they have come. */
  function roofTypes() {
    if (!C.model) return null;
    const model = C.model.id;
    const key = `${model}/${C.paramsEpoch}`;
    if (C.roofTypes?.key !== key && C.roofTypesLoading !== key) {
      C.roofTypesLoading = key;
      api(`${modelUrl(model)}/roof-types`)
        .then((view) => {
          C.roofTypes = { key, view };
          refreshSide();
        })
        .catch(() => {});
    }
    return C.roofTypes?.key.startsWith(`${model}/`) ? C.roofTypes.view : null;
  }

  /** The open model's roof types, read now if they have not come yet. */
  async function roofTypesNow() {
    const view = roofTypes();
    if (view) return view;
    const model = C.model.id;
    const fresh = await api(`${modelUrl(model)}/roof-types`);
    C.roofTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
    return fresh;
  }

  const chosenRoofType = (view) => view.types.find((t) => t.id === C.roofTool.type) ?? view.types[0] ?? null;

  /** Has the server make its default roof types, once for a model, where it has none. */
  function makeDefaultRoofs() {
    if (!canEdit() || C.madeRoofDefaults === C.model.id) return;
    C.madeRoofDefaults = C.model.id;
    manage('roof.defaults', {}, 'roof-draw');
  }

  const ROOF_FORMS = [['flat', 'Flat'], ['mono', 'Mono-pitch'], ['gable', 'Gable'], ['hip', 'Hip']];

  function roofToolHtml() {
    let html = typeSel('family', 'Roof', '<b>New roof</b>');
    const view = roofTypes();
    if (!view) return `${html}<p class="pad small faint">Loading the roof types.</p>`;
    if (!view.types.length) {
      queueMicrotask(makeDefaultRoofs);
      return `${html}<p class="pad small faint" data-roof-tool="making">This model has no roof types yet, so the default ones are being made.</p>${problem('roof-draw')}`;
    }
    const chosen = chosenRoofType(view);
    const here = levelOf();
    const level = (modelLevels() || []).find((l) => l.id === here)?.name ?? here;
    const types = view.types.map((t) => `<option value="${esc(t.id)}"${t.id === chosen.id ? ' selected' : ''}>${esc(t.name)}</option>`);
    const forms = ROOF_FORMS.map(([id, name]) => `<option value="${id}"${id === C.roofTool.form ? ' selected' : ''}>${name}</option>`);
    const field = (key, label, value, extra = '') => `<input class="inp" data-chg="stackroof${key}" data-key="roof-${key}" value="${esc(value)}" aria-label="${esc(label)}"${extra}>`;
    html += '<table class="props">';
    html += prow('Type', `<select class="inp" data-chg="stackrooftool" data-key="roof-type" aria-label="Roof type">${types.join('')}</select>`);
    html += prow('Form', `<select class="inp" data-chg="stackroofformtool" data-key="roof-form" aria-label="Roof form">${forms.join('')}</select>`);
    if (C.roofTool.form !== 'flat') html += prow('Pitch', field('pitch', 'Pitch, as 1:4, 25% or 30°', C.roofTool.pitch));
    if (C.roofTool.form !== 'flat') html += prow('Rises from', field('eaves', 'Edge numbers it rises from, blank for automatic', C.roofTool.eaves, ' placeholder="automatic"'));
    html += prow('Overhang', field('overhang', 'Overhang past its edges', C.roofTool.overhang));
    html += prow('Thickness', esc(chosen.thickness));
    html += prow('Level', esc(level));
    html += prow('Offset', field('offset', 'Offset of the roof’s base above its level', C.roofTool.offset));
    html += '</table>';
    html += `<span class="lbl">Layers, outside first</span><table class="props" data-roof-layers="${esc(chosen.id)}">`;
    for (const layer of chosen.layers) {
      html += prow(esc(capital(layer.function)), `${esc(layer.thickness)} · ${esc(materialName(view, layer.material))}`);
    }
    html += '</table>';
    return html + sketchHtml() + problem('roof-draw');
  }

  CHG.stackrooftool = (select) => {
    C.roofTool.type = select.value;
    refreshSide();
  };
  CHG.stackroofformtool = (select) => {
    C.roofTool.form = select.value;
    refreshSide();
  };
  for (const key of ['pitch', 'eaves', 'overhang', 'offset']) {
    CHG[`stackroof${key}`] = (input) => {
      C.roofTool[key] = input.value;
    };
  }
  ACT.stackroofmake = () => makeRoof();

  function roofHint() {
    if (FLOOR.current.length) return '<b>Roof:</b> click the next corner, or its first corner or Enter to close the loop. Esc to stop.';
    if (FLOOR.loops.length) return '<b>Roof:</b> press Enter to make the roof. Esc to stop.';
    return '<b>Roof:</b> click the corners of its footprint. Choose its type, form and pitch in the properties palette.';
  }

  /** Sends the sketch as it is: what the server makes of it, or why it refuses, comes back through the log. */
  async function makeRoof() {
    if (!FLOOR.loops.length || FLOOR.current.length || !canEdit()) return;
    let view;
    try {
      view = await roofTypesNow();
    } catch (error) {
      say(error.message);
      return;
    }
    const corners = (loop) => loop.map(([x, y]) => ({ at: [`${x} mm`, `${y} mm`] }));
    const [outer, ...holes] = FLOOR.loops;
    const tool = C.roofTool;
    const args = { element: newId('R'), level: levelOf(), form: tool.form, footprint: { outer: corners(outer), holes: holes.map(corners) } };
    const type = chosenRoofType(view);
    if (type) args.type = type.id;
    if (tool.form !== 'flat' && tool.pitch.trim() !== '') args.pitch = tool.pitch;
    if (tool.overhang.trim() !== '') args.overhang = tool.overhang;
    if (tool.offset.trim() !== '') args.offset = tool.offset;
    // Edges are numbered from 1 in the order the corners were clicked; the server names them o0, o1, ...
    const numbers = tool.eaves.split(/[\s,]+/).filter((n) => n !== '');
    if (tool.form !== 'flat' && numbers.length) args.eaves = numbers.map((n) => `o${Number(n) - 1}`);
    const sent = { loops: FLOOR.loops };
    Object.assign(FLOOR, { loops: [], current: [], at: null, sent });
    manage('roof.create', args, 'roof-draw', () => {
      if (FLOOR.sent === sent) FLOOR.sent = null;
      queueDraw();
    });
    sketchChanged();
  }

  /** The ribbon: the roof tool beside the floor tool, in the Build panel. */
  function roofRibbon(config) {
    const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
    if (!build || build.items.some((item) => item.a === 'architecture:roof')) return;
    const floor = build.items.findIndex((item) => item.a === 'architecture:floor');
    build.items.splice(floor + 1, 0, DT_('architecture', 'roof'));
  }

  /** The point `[x, y, z]` of a solid, in whole millimetres in plan. */
  const planPoint = (p) => [nearestMm(p[0]), nearestMm(p[1])];

  /**
   * A roof in the plan: its eaves outline from its recipe as the server filled it (the outline of
   * its display until that has come), in its outer layer's colour seen from above, and, where it
   * slopes, the lines of its top face (ridge, hips, valleys, verges) as the server worked them out.
   */
  function roofInPlan(piece, data, scale) {
    const { el: e } = piece;
    const { x, y } = placementOf(e);
    const args = piece.fill?.recipe?.args;
    const outline = Array.isArray(args?.outline) && args.outline.length
      ? args.outline.map((corner) => planPoint(corner.at))
      : piece.shape.displays[0].outline;
    const ring = outline.map(([px, py]) => [x + px, y + py]);
    const outer = args?.layers?.[0];
    const colour = outer && colourOf(outer.material, 'surface');
    const fill = colour ? ` style="fill:${esc(colour)}"` : '';
    const box = around(piece);
    const chosen = S.sel === e.id;
    let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
    group += `<path class="body roof${chosen ? ' sel' : ''}" data-form="${esc(args?.form ?? '')}" data-material="${esc(outer?.material ?? '')}"${fill} d="${ringPath(ring)}"/>`;
    // The ridges, hips, valleys and verges as the server gave them in the filled recipe (`lines`,
    // in grid steps); an eave is the outline itself. Nothing is worked out here.
    for (const line of Array.isArray(args?.lines) ? args.lines : []) {
      if (line.kind === 'eave') continue;
      const [a, b] = [line.from, line.to].map(planPoint);
      const ridge = line.kind === 'ridge' ? ' data-roof-ridge' : '';
      group += `<line class="roof-line roof-${esc(line.kind)}" data-roof-line="${esc(line.kind)}"${ridge} x1="${x + a[0]}" y1="${y + a[1]}" x2="${x + b[0]}" y2="${y + b[1]}"/>`;
    }
    const claim = heldClaim(e.id);
    if (claim) {
      const theirs = claim.person !== C.me.person.id;
      group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
      if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
    }
    return `<g ${data}>${group}</g>`;
  }

  /** A stored pitch typed as a person would: `slope:1/4` as 1:4, `angle:270000000` as 30°. */
  function typedPitch(stored) {
    if (stored.startsWith('slope:')) return stored.slice(6).replace('/', ':');
    if (stored.startsWith('angle:')) return `${Number(stored.slice(6)) / 9000000}°`;
    return stored;
  }

  /**
   * What the properties palette reads of a selected roof's record: its form, pitch and overhangs,
   * and the pitch of each edge it rises from (numbered from 1, as the roof tool numbers them). Its
   * form is the one the server's filled recipe names (`solved` is a hip roof) once that has come.
   */
  function roofSpecs(piece) {
    const e = piece.el;
    const edges = Array.isArray(e.recipe.args.edges) ? e.recipe.args.edges : [];
    const sloping = edges.filter((edge) => edge.pitch);
    const form = ['flat', 'mono', 'gable'][Math.min(sloping.length, 2)] ?? 'flat';
    const filled = piece.fill?.recipe?.args?.form;
    const overhangs = [...new Set(edges.map((edge) => `${storedMm(edge.overhang)} mm`))];
    return {
      form: filled === 'solved' ? 'hip' : filled ?? (sloping.length > 2 ? 'hip' : form),
      pitch: sloping[0] ? typedPitch(sloping[0].pitch) : '',
      overhang: overhangs.length === 1 ? overhangs[0] : '',
      edges: edges.flatMap((edge, at) => (edge.pitch ? [{ id: edge.id, number: at + 1, pitch: typedPitch(edge.pitch) }] : [])),
    };
  }

  /** The base's offset above its level, as stored. */
  function roofOffset(e) {
    const stored = e.props?.['@level.offset'];
    return `${stored === undefined ? 0 : storedMm(stored)} mm`;
  }

  /** What the properties palette shows of a selected roof. */
  function roofProperties(piece) {
    const { el: e, shape, failed } = piece;
    const view = paramsFor(piece);
    const types = roofTypes();
    const claim = heldClaim(e.id);
    const specs = roofSpecs(piece);
    const editable = canEdit();
    let html = typeSel('family', 'Roof', `<b>${esc(e.name)}</b>`);
    html += '<table class="props" data-roof-element>';
    if (view) html += prow('Category', esc(view.category?.name || 'None'));
    if (types && editable) {
      const options = types.types.map((t) => `<option value="${esc(t.id)}"${t.id === view?.type?.id ? ' selected' : ''}>${esc(t.name)}</option>`);
      html += prow('Type', `<select class="inp" data-chg="stackrooftype" data-key="roof-own-type" aria-label="Roof type">${options.join('')}</select>`);
    } else if (view) {
      html += prow('Type', esc(view.type?.name || 'None'));
    }
    html += prow('Level', esc(levelName(e))) + prow('Offset', `<span class="mono" data-roof-offset>${esc(roofOffset(e))}</span>`);
    if (editable) {
      const forms = ROOF_FORMS.map(([id, name]) => `<option value="${id}"${id === specs.form ? ' selected' : ''}>${name}</option>`);
      html += prow('Form', `<select class="inp" data-chg="stackroofshape" data-key="roof-own-form" aria-label="Roof form">${forms.join('')}</select>`);
      if (specs.form !== 'flat') html += prow('Pitch', `<input class="inp" data-chg="stackroofown" data-a="pitch" data-key="roof-own-pitch" value="${esc(specs.pitch)}" aria-label="Pitch, as 1:4, 25% or 30°">`);
      html += prow('Overhang', `<input class="inp" data-chg="stackroofown" data-a="overhang" data-key="roof-own-overhang" value="${esc(specs.overhang)}" placeholder="varies" aria-label="Overhang past its edges">`);
      // One pitch for every edge it rises from, or a pitch of its own for each (an uneven gable).
      if (specs.edges.length > 1) {
        for (const edge of specs.edges) {
          html += prow(`Edge ${edge.number} pitch`, `<input class="inp" data-chg="stackroofedge" data-edge="${esc(edge.id)}" data-key="roof-edge-pitch:${edge.number}" value="${esc(edge.pitch)}" aria-label="Pitch of edge ${edge.number}, as 1:4, 25% or 30°">`);
        }
      }
    } else {
      html += prow('Form', esc(specs.form));
      if (specs.pitch) html += prow('Pitch', esc(specs.pitch));
    }
    if (!shape) html += prow('Shape', esc(failed?.message || 'Not built yet'));
    html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
    html += problem('roof-edit');
    if (view?.parameters.some((p) => p.from !== READ_ONLY)) html += parametersTable(view);
    if (view) {
      html += readOnlyTable(view);
      html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
    }
    if (editable) html += '<div class="pad"><button class="btn sm" data-act="stackdelete" data-key="roof-delete">Delete roof</button></div>';
    return html;
  }

  CHG.stackrooftype = (select) => {
    if (S.sel) manage('roof.retype', { element: S.sel, type: select.value }, 'roof-edit');
  };
  CHG.stackroofshape = (select) => {
    const piece = C.elements.get(S.sel);
    if (!piece) return;
    const args = { element: S.sel, form: select.value };
    // Going from flat to a form that slopes needs a pitch: the tool's.
    if (select.value !== 'flat' && !roofSpecs(piece).pitch) args.pitch = C.roofTool.pitch;
    manage('roof.form', args, 'roof-edit');
  };
  CHG.stackroofown = (input) => {
    if (!S.sel) return;
    const cmd = input.dataset.a === 'pitch' ? 'roof.pitch' : 'roof.overhang';
    manage(cmd, { element: S.sel, [input.dataset.a]: input.value }, 'roof-edit');
    input.blur();
  };

  CHG.stackroofedge = (input) => {
    if (!S.sel) return;
    manage('roof.pitch', { element: S.sel, pitch: input.value, edge: input.dataset.edge }, 'roof-edit');
    input.blur();
  };

  /** The properties palette's roof part: the roof tool's options, or a selected roof; '' for anything else. */
  function roofPalette(piece) {
    if (S.tool === 'roof' && canEdit()) return roofToolHtml();
    if (piece && isRoof(piece.el) && !(S.tool === 'wall' && canEdit())) return roofProperties(piece);
    return '';
  }

  // --- Rooms: the Room tool, a room in the plan, and its properties ---
  // A room is a point inside a loop of walls on the plan's level. The Room tool sends one
  // `room.place` edit for a click (a name and a number typed in the properties palette, the next
  // free number offered from `/rooms`, an upper limit chosen there: a level, or a height), with the
  // point where the click is; a refusal comes back in the server's words, beside the tool. The page
  // finds no boundary and works out no area: the server fills the room's recipe (`/filled/<element>`)
  // with its loops and says what became of it (`room`: its state, and its area, perimeter and volume
  // in the document's units) each time a wall round it changes, so the room is drawn as a light fill
  // with a tag at its point giving name, number and area. A room that is not enclosed, or is in a
  // loop an earlier room has, is drawn as a point whose tag says so in words, with no area. A
  // selected room shows its name, number and upper limit to change (`room.rename`, `room.number`,
  // `room.upper`), what the server works out of it read-only, and Delete (`room.delete`).

  const isRoom = (record) => record?.kind === 'room';

  Object.assign(WORDS, {
    'room.place': 'placed a room',
    'room.rename': 'renamed a room',
    'room.number': 'renumbered a room',
    'room.move': 'moved a room',
    'room.upper': 'changed the upper limit of a room',
    'room.delete': 'deleted a room',
  });
  Object.assign(STEP_WORDS, {
    'room.place': 'placing a room',
    'room.rename': 'renaming a room',
    'room.number': 'renumbering a room',
    'room.move': 'moving a room',
    'room.upper': 'a room’s upper limit',
    'room.delete': 'deleting a room',
  });

  C.roomTool = { name: 'Room', number: null, top: undefined, height: '2400 mm' }; // number null: the one offered; top as the wall tool's
  C.roomNext = null; // { key, next }: the next free number the server offers

  /** The next free room number the server offers, once it has come; '' until then. */
  function nextRoomNumber() {
    if (!C.model) return '';
    const model = C.model.id;
    const key = `${model}/${C.head}`;
    if (C.roomNext?.key !== key && C.roomNextLoading !== key) {
      C.roomNextLoading = key;
      api(`${modelUrl(model)}/rooms`)
        .then((view) => {
          C.roomNext = { key, next: view.next_number };
          refreshSide();
        })
        .catch(() => {});
    }
    return C.roomNext?.next ?? '';
  }

  /** The upper limit the tool places to: a level's ID, or '' for a height. By default the level above the plan's. */
  function roomTop(levels) {
    const here = levelOf();
    const others = (levels || []).filter((level) => level.id !== here);
    const chosen = C.roomTool.top;
    if (chosen === '' || others.some((level) => level.id === chosen)) return chosen;
    const at = (levels || []).findIndex((level) => level.id === here);
    return (at >= 0 && levels[at + 1]?.id) || '';
  }

  /** The options of an upper limit: the other levels, and a height; `chosen` is a level's ID or ''. */
  function limitOptions(levels, own, chosen) {
    const options = (levels || [])
      .filter((level) => level.id !== own)
      .map((level) => `<option value="${esc(level.id)}"${level.id === chosen ? ' selected' : ''}>${esc(level.name)}</option>`);
    options.push(`<option value=""${chosen === '' ? ' selected' : ''}>A height</option>`);
    return options.join('');
  }

  function roomToolHtml() {
    let html = typeSel('room', 'Room', '<b>New room</b>');
    const levels = modelLevels() || [];
    const here = levelOf();
    const top = roomTop(levels);
    const base = levels.find((level) => level.id === here)?.name ?? here;
    const number = C.roomTool.number ?? nextRoomNumber();
    html += '<table class="props" data-room-tool>';
    html += prow('Name', `<input class="inp" data-chg="stackroomtoolname" data-key="room-tool-name" value="${esc(C.roomTool.name)}" aria-label="Name of the room">`);
    html += prow('Number', `<input class="inp" data-chg="stackroomtoolnumber" data-key="room-tool-number" value="${esc(number)}" aria-label="Number of the room">`);
    html += prow('Level', esc(base));
    html += prow('Upper limit', `<select class="inp" data-chg="stackroomtooltop" data-key="room-tool-top" aria-label="Upper limit of the room">${limitOptions(levels, here, top)}</select>`);
    if (top === '') {
      html += prow('Height', `<input class="inp" data-chg="stackroomtoolheight" data-key="room-tool-height" value="${esc(C.roomTool.height)}" aria-label="Height of the room">`);
    }
    html += '</table>';
    html += '<p class="pad small muted">Click inside a loop of walls. The room finds its own boundary.</p>';
    return html + problem('room-place');
  }

  CHG.stackroomtoolname = (input) => {
    C.roomTool.name = input.value;
  };
  CHG.stackroomtoolnumber = (input) => {
    C.roomTool.number = input.value;
  };
  CHG.stackroomtooltop = (select) => {
    C.roomTool.top = select.value;
    refreshSide();
  };
  CHG.stackroomtoolheight = (input) => {
    C.roomTool.height = input.value;
  };

  /** Sends the click as a room: what the server makes of it, or why it refuses, comes back through the log. */
  function placeRoom(at) {
    if (!canEdit()) {
      say('Open a model in your team’s WIP to add to it.');
      return;
    }
    const point = [snap(at.x), snap(at.y)];
    const args = { element: newId('R'), level: levelOf(), at: [`${point[0]} mm`, `${point[1]} mm`] };
    if (C.roomTool.name.trim() !== '') args.name = C.roomTool.name;
    const number = C.roomTool.number ?? C.roomNext?.next;
    if (number) args.number = number;
    const top = roomTop(modelLevels());
    if (top === '') args.height = C.roomTool.height;
    else args.top = top;
    manage('room.place', args, 'room-place', () => {
      C.roomTool.number = null;
      refreshSide();
    });
  }

  /**
   * A room in the plan: its loops from its recipe as the server filled it, a light fill with the
   * holes left out, and a tag at its point: its name, its number, and its area, or in words what
   * became of it when it has none.
   */
  function roomInPlan(piece, data, scale) {
    const { el: e } = piece;
    const info = piece.fill?.room;
    const loops = piece.fill?.recipe?.args?.loops;
    const [px, py] = (e.recipe.args.at || [0, 0]).map(storedMm);
    const chosen = S.sel === e.id;
    let group = '';
    if (Array.isArray(loops) && loops.length) {
      const rings = loops.map((loop) => loop.map((corner) => [nearestMm(corner.at[0]), nearestMm(corner.at[1])]));
      group += `<path class="room-fill${chosen ? ' sel' : ''}" data-holes="${rings.length - 1}" d="${rings.map(ringPath).join('')}"/>`;
    } else {
      group += `<circle class="room-point${chosen ? ' sel' : ''}" cx="${px}" cy="${py}" r="150"/>`;
    }
    const number = e.props?.['@room.number'] ?? '';
    const last = info?.area
      ? TX(px, py + 330, scale, esc(info.area), 'room-area', 240)
      : TX(px, py + 330, scale, esc(capital(info?.state ?? 'not worked out yet')), 'room-state', 240);
    group += `<g class="room-tag" data-room-tag="${esc(e.id)}">${TX(px, py - 330, scale, esc(e.name), 'room-name', 280)}${TX(px, py, scale, esc(number), 'room-number', 240)}${last}</g>`;
    const claim = heldClaim(e.id);
    if (claim && claim.person !== C.me.person.id) group += TX(px, py - 700, scale, `${claim.name} editing`, 'claimt', 260);
    return `<g ${data}>${group}</g>`;
  }

  /** What the properties palette shows of a selected room. */
  function roomProperties(piece) {
    const { el: e } = piece;
    const view = paramsFor(piece);
    const info = piece.fill?.room;
    const claim = heldClaim(e.id);
    const off = canEdit() ? '' : ' disabled';
    const levels = modelLevels() || [];
    const topLevel = e.props?.['@top.level'] ?? '';
    let html = typeSel('room', 'Room', `<b>${esc(e.name)}</b>`);
    html += '<table class="props" data-room-element>';
    html += prow('Name', `<input class="inp" data-chg="stackroomname" data-key="room-name" value="${esc(e.name)}" aria-label="Name of the room"${off}>`);
    html += prow('Number', `<input class="inp" data-chg="stackroomnumber" data-key="room-number" value="${esc(e.props?.['@room.number'] ?? '')}" aria-label="Number of the room"${off}>`);
    html += prow('Level', esc(levelName(e)));
    html += prow('Upper limit', `<select class="inp" data-chg="stackroomupper" data-key="room-upper" aria-label="Upper limit of the room"${off}>${limitOptions(levels, levelIdOf(e), topLevel)}</select>`);
    if (topLevel) {
      const offset = storedMm(e.props?.['@top.offset'] ?? 'length:q0:0');
      html += prow('Offset', `<input class="inp" data-chg="stackroomoffset" data-key="room-offset" value="${offset} mm" aria-label="Offset of the upper limit"${off}>`);
    } else {
      html += prow('Height', `<input class="inp" data-chg="stackroomheight" data-key="room-height" value="${storedMm(e.recipe.args.height)} mm" aria-label="Height of the room"${off}>`);
    }
    html += prow('Claimed by', claim ? esc(claim.name) : 'Nobody');
    html += '</table>';
    if (info?.words) html += `<div class="note small" data-room-words>${esc(info.words)}</div>`;
    if (view) html += readOnlyTable(view);
    if (canEdit()) html += '<div class="pad"><button class="btn sm" data-act="stackdelete" data-key="room-delete">Delete</button></div>';
    return html + problem(`room:${e.id}`);
  }

  /** The properties palette's room part: the Room tool's options, or a selected room; '' for anything else. */
  function roomPalette(piece) {
    if (S.tool === 'room' && canEdit()) return roomToolHtml();
    if (piece && isRoom(piece.el) && !(S.tool === 'wall' && canEdit())) return roomProperties(piece);
    return '';
  }

  const sendRoom = (cmd, args) => {
    if (S.sel && canEdit()) manage(cmd, { element: S.sel, ...args }, `room:${S.sel}`);
  };
  CHG.stackroomname = (input) => sendRoom('room.rename', { name: input.value });
  CHG.stackroomnumber = (input) => sendRoom('room.number', { number: input.value });
  CHG.stackroomupper = (select) => {
    const piece = C.elements.get(S.sel);
    if (select.value) sendRoom('room.upper', { top: select.value });
    else sendRoom('room.upper', { height: piece?.el.recipe.args.height ? `${storedMm(piece.el.recipe.args.height)} mm` : C.roomTool.height });
  };
  CHG.stackroomoffset = (input) => {
    const top = C.elements.get(S.sel)?.el.props?.['@top.level'];
    if (top) sendRoom('room.upper', { top, top_offset: input.value });
  };
  CHG.stackroomheight = (input) => sendRoom('room.upper', { height: input.value });

  const deleteAnElement = ACT.stackdelete;
  ACT.stackdelete = () => {
    const piece = C.elements.get(S.sel);
    if (!piece || !isRoom(piece.el)) {
      deleteAnElement();
      return;
    }
    if (!canEdit()) return;
    manage('room.delete', { element: piece.el.id }, `room:${piece.el.id}`);
    S.sel = null;
    refresh();
  };

  /** Every room of `into` filled and built again when an entry changed something other than rooms: the walls round them may have moved. */
  function roomsAgain(model, into, entry) {
    const rooms = [...into].filter(([, piece]) => isRoom(piece.el));
    if (!rooms.length) return;
    const own = new Set(rooms.map(([id]) => id));
    if (!UNDOING.has(entry.cmd) && !entry.changes.some((change) => !own.has(change.piece))) return;
    for (const [id, piece] of rooms) refill(model, into, id, piece);
  }

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
        if (seen.through < C.head) send({ t: 'watch', document: model, since: seen.through });
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

  // --- Grids and reference planes ---
  // A grid line or a reference plane is a record of the model (construct-grids' FORMAT.md), made and
  // changed only by its own commands through the same log: `grid.create`, `grid.rename`,
  // `grid.reshape`, `grid.delete`, `plane.rename`, `plane.delete`. They stand on no level, so the plan's
  // area never lists them: this finds the open model's by kind, keeps them apart from the elements
  // (so nothing that counts or picks elements sees them), and draws them under everything, a linked
  // model's as they came, seen and never picked. A grid is a thin chain line with its name in a bubble
  // at its start, a plane a dashed line, in the demo's own datum classes (the lightest pen in a muted
  // colour with a chain or a dashed pattern, as construct-styles' defaults give grids and planes); an
  // arc grid is the chain of chords the geometry builds of it. The grid tool sends two picked points
  // and no name: the server gives it the next one. Nothing is checked here: a refusal is the
  // server's reason, beside what caused it.

  const DATUM_KINDS = ['grid', 'refplane'];
  const isDatum = (record) => DATUM_KINDS.includes(record?.kind);
  C.datums = new Map(); // model id -> Map of id -> { el, shape, failed, hash, seq }: grids and planes
  // drag: an end being dragged; held: a reshape sent, drawn until the server answers; preview: the
  // pointer while a grid's first point is picked; after: the last grid this person made or renamed
  const G = { drag: null, held: null, preview: null, after: null };

  DTOOLS.architecture.push(['grid', 'Grid', 'grid', 'Pick both ends of a grid line. The server gives it the next name.']);
  TOOLNAME.grid = 'grid';
  Object.assign(WORDS, {
    'grid.create': 'made a grid',
    'grid.rename': 'renamed a grid',
    'grid.move': 'moved a grid',
    'grid.reshape': 'moved an end of a grid',
    'grid.delete': 'deleted a grid',
    'plane.create': 'made a reference plane',
    'plane.rename': 'renamed a reference plane',
    'plane.move': 'moved a reference plane',
    'plane.delete': 'deleted a reference plane',
  });
  Object.assign(STEP_WORDS, {
    'grid.create': 'making a grid',
    'grid.rename': 'renaming a grid',
    'grid.move': 'moving a grid',
    'grid.reshape': 'moving an end of a grid',
    'grid.delete': 'deleting a grid',
    'plane.create': 'making a reference plane',
    'plane.rename': 'renaming a reference plane',
    'plane.move': 'moving a reference plane',
    'plane.delete': 'deleting a reference plane',
  });

  /** A grid or a plane of the open model, by its ID. */
  const ownDatum = (id) => (id && C.model ? C.datums.get(C.model.id)?.get(id) : undefined);

  function forgetDatums() {
    C.datums.clear();
    Object.assign(G, { drag: null, held: null, preview: null, after: null });
  }

  /** Reads the grids and planes of the open model and of each live-linked one. */
  async function loadDatums() {
    const models = [C.model?.id, ...C.links.filter((link) => link.kind === 'live').map((link) => link.model)];
    try {
      for (const model of models.filter(Boolean)) {
        for (const kind of DATUM_KINDS) {
          const seen = await api(`${modelUrl(model)}/kind/${kind}`);
          if (seen.through < C.head) send({ t: 'watch', document: model, since: seen.through });
          await Promise.all(
            Object.entries(seen.pieces).map(async ([id, hash]) =>
              keepDatum(model, id, await api(`${modelUrl(model)}/record/${hash}`), hash, seen.through),
            ),
          );
        }
      }
    } catch (error) {
      console.warn(`The grids and planes couldn’t be read: ${error.message}`);
    }
  }

  /** A grid's or a plane's record arrived: it is built as any record is here, and kept. */
  async function keepDatum(model, id, record, hash, seq, mine = false) {
    const answer = await shapeFor(model, record, null);
    if (model !== C.model?.id && !C.watching.has(model)) return;
    const pieces = C.datums.get(model) ?? new Map();
    C.datums.set(model, pieces);
    const known = pieces.get(id);
    if (known && known.seq > seq) return;
    pieces.set(id, { el: record, ...answer, hash, seq });
    if (mine && record.kind === 'grid') G.after = { model, name: record.name };
    if (G.held?.id === id && G.held.applied) G.held = null;
    queueDraw();
    if (S.sel === id) {
      refreshSide();
      renderRibbon();
    }
  }

  /** An entry changed a grid or a plane this page holds: true when it was one, which this takes care of. */
  function datumChange(entry, change) {
    const pieces = C.datums.get(entry.target);
    const known = pieces?.get(change.piece);
    if (!known) return false;
    if (known.seq >= entry.seq) return true;
    if (!change.after) {
      pieces.delete(change.piece);
      if (S.sel === change.piece) {
        S.sel = null;
        refreshSide();
        renderRibbon();
      }
      return true;
    }
    const mine = entry.client === C.client && !UNDOING.has(entry.cmd);
    api(`${modelUrl(entry.target)}/record/${change.after}`)
      .then((record) => keepDatum(entry.target, change.piece, record, change.after, entry.seq, mine))
      .catch((error) => console.warn(`A grid or a plane couldn’t be read: ${error.message}`));
    return true;
  }

  /** The points a datum is drawn through: an end being dragged, a reshape awaiting its answer, or what was built. */
  function datumOutline(piece) {
    const id = piece.el.id;
    if (G.drag?.id === id && G.drag.to) {
      const outline = G.drag.outline.slice();
      outline[G.drag.end] = G.drag.to;
      return outline;
    }
    if (G.held?.id === id && (G.held.applied || C.asked.has(G.held.counter))) return G.held.outline;
    return piece.shape?.displays?.[0]?.outline ?? null;
  }

  /** One grid or plane in the plan. `how` is `own`, the open model's, or `linked`. */
  function datumSvg(piece, s, how) {
    const { el } = piece;
    const outline = datumOutline(piece);
    // A horizontal plane has no place in plan; a record that didn't build has nothing to draw.
    if (!outline || outline.length < 2) return '';
    const grid = el.kind === 'grid';
    const own = how === 'own';
    const picked = own && S.sel === el.id;
    let g = PL(outline, `ln fn ${grid ? 'dd' : 'dsh'}${picked ? ' ap' : ''}`);
    if (own) g += PL(outline, 'datum-hit');
    const [[ax, ay], [bx, by]] = outline;
    const run = Math.hypot(ax - bx, ay - by) || 1;
    const r = 420 * s;
    // The bubble sits beyond the start, on the line carried on.
    const [cx, cy] = [ax + ((ax - bx) / run) * r, ay + ((ay - by) / run) * r];
    if (grid) g += `<circle class="bub" cx="${n0(cx)}" cy="${n0(cy)}" r="${n0(r)}"/>${TX(cx, cy + 130 * s, s, esc(el.name), 'tn', 360)}`;
    else if (el.name) g += TX(ax, ay - 200 * s, s, esc(el.name), 'ta', 260, 'start');
    const claim = own && claimedByOther(el.id);
    if (claim) g += TX(cx, cy - r - 160 * s, s, `${esc(claim.name)} editing`, 'claimt', 260);
    if (picked && canEdit() && el.recipe.op === 'grid.line') {
      const h = 150 * s;
      for (const [end, [x, y]] of [outline[0], outline.at(-1)].entries()) {
        g += `<rect class="ln ap pf datum-end" data-grid-end="${end}" x="${n0(x - h)}" y="${n0(y - h)}" width="${n0(2 * h)}" height="${n0(2 * h)}"/>`;
      }
    }
    const [x0, y0] = outline[0];
    const [x1, y1] = outline.at(-1);
    const marks = `data-datum="${esc(el.id)}" data-kind="${esc(el.kind)}" data-name="${esc(el.name)}" data-x0="${x0}" data-y0="${y0}" data-x1="${x1}" data-y1="${y1}"${picked ? ' data-picked="yes"' : ''}`;
    return `<g class="datum ${how}" ${marks}>${g}</g>`;
  }

  /** Every grid and plane in view, drawn first so that the elements stay on top and are picked. */
  function datumsSvg(s) {
    if (S.cats?.grids === false) return '';
    let svg = '';
    const linked = (elements) => {
      for (const piece of elements) if (isDatum(piece.element)) svg += datumSvg(packaged(piece), s, 'linked');
    };
    for (const elements of C.fixed.values()) linked(elements);
    if (!C.model && C.file) linked(C.file.elements);
    for (const [model, pieces] of C.datums) {
      for (const piece of pieces.values()) svg += datumSvg(piece, s, model === C.model?.id ? 'own' : 'linked');
    }
    if (S.tool === 'grid' && S.pend && G.preview) svg += L(S.pend, G.preview, 'ln ap dd');
    return svg;
  }

  function gridHint() {
    return S.pend ? '<b>Grid:</b> pick its other end. Esc to stop.' : '<b>Grid:</b> pick where it starts: its name goes in a bubble there.';
  }

  /** The name of the grid this person last made or renamed in the open model, for the server's next name. */
  const lastGridName = () => (G.after?.model === C.model?.id ? G.after.name : null);

  function placeGrid(at) {
    const point = [snap(at.x), snap(at.y)];
    if (!S.pend) {
      S.pend = point;
      G.preview = point;
      $('#hint').innerHTML = hint();
      return;
    }
    const [[x0, y0], [x1, y1]] = [S.pend, point];
    S.pend = null;
    G.preview = null;
    const args = { grid: newId('G'), name: null, x0: `${x0}`, y0: `${y0}`, x1: `${x1}`, y1: `${y1}` };
    const after = lastGridName();
    if (after !== null) args.after = after;
    manage('grid.create', args, 'new-grid');
    $('#hint').innerHTML = hint();
    queueDraw();
  }

  /** A stored length (`length:q0:<steps>`, or an older whole `length:<mm>`) as typed text, exactly. */
  function typedLength(text) {
    const [, unit, steps] = String(text).split(':');
    if (unit !== 'q0') return `${unit} mm`;
    const n = Number(steps);
    const [whole, rest] = [Math.floor(Math.abs(n) / 6400), Math.abs(n) % 6400];
    // A step is 1/6400 mm, which is 15625 hundred-millionths: so the fraction is exact in 8 places.
    const fraction = rest ? `.${String(rest * 15625).padStart(8, '0').replace(/0+$/, '')}` : '';
    return `${n < 0 ? '-' : ''}${whole}${fraction} mm`;
  }

  /** A grid or a plane was picked in the plan: selected, claimed as an element is, and maybe an end taken. */
  function pickDatum(id, end) {
    if (S.sel !== id) select(id);
    if (!canEdit()) return;
    edit('element.claim', { element: id });
    const piece = ownDatum(id);
    const outline = piece && datumOutline(piece);
    if (end && piece?.el.recipe.op === 'grid.line' && outline?.length === 2) {
      G.held = null;
      G.drag = { id, end: Number(end.dataset.gridEnd), outline: outline.map((point) => [...point]), to: null };
    }
  }

  /** An end let go: the grid's two ends go to the server, the one not dragged as it is stored. */
  function letGoOfAnEnd() {
    const drag = G.drag;
    if (!drag) return;
    G.drag = null;
    const piece = ownDatum(drag.id);
    if (!piece || !drag.to || drag.to.join() === drag.outline[drag.end].join()) {
      queueDraw();
      return;
    }
    const { args } = piece.el.recipe;
    const ends = [
      [typedLength(args.x0), typedLength(args.y0)],
      [typedLength(args.x1), typedLength(args.y1)],
    ];
    ends[drag.end] = drag.to.map((value) => `${value} mm`);
    const outline = drag.outline.slice();
    outline[drag.end] = drag.to;
    const reshape = { grid: drag.id, x0: ends[0][0], y0: ends[0][1], x1: ends[1][0], y1: ends[1][1] };
    // The counter the edit is about to take: the dragged end is drawn until the server answers it.
    const counter = C.counter + 1;
    G.held = { id: drag.id, counter, outline };
    manage('grid.reshape', reshape, `datum:${drag.id}`, () => {
      if (G.held?.counter === counter) G.held.applied = true;
    });
    queueDraw();
  }

  // In the plan, before the elements' own handlers: the grid tool's points, and a pick of a grid or a plane.
  document.addEventListener(
    'pointerdown',
    (event) => {
      if (event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
      if (S.tool === 'grid') {
        event.stopPropagation();
        if (!canEdit()) say('Open a model in your team’s WIP to add to it.');
        else placeGrid(toModel(event));
        return;
      }
      const hit = S.tool === 'select' && event.target.closest('g.datum.own');
      if (!hit) return;
      event.stopPropagation();
      pickDatum(hit.dataset.datum, event.target.closest('[data-grid-end]'));
    },
    true,
  );
  window.addEventListener('pointermove', (event) => {
    if (!(G.drag || (S.tool === 'grid' && S.pend)) || !$('#pv-canvas svg')) return;
    const at = toModel(event);
    const point = [snap(at.x), snap(at.y)];
    if (G.drag) G.drag.to = point;
    else G.preview = point;
    queueDraw();
  });
  window.addEventListener('pointerup', letGoOfAnEnd);
  window.addEventListener('pointercancel', letGoOfAnEnd);

  /** What the properties palette shows of a selected grid or plane. */
  function datumProperties(piece) {
    const { el } = piece;
    const grid = el.kind === 'grid';
    const what = grid ? 'grid' : 'reference plane';
    const outline = datumOutline(piece) || [];
    const view = paramsFor(piece);
    const claim = heldClaim(el.id);
    const name = canEdit()
      ? `<input class="inp" data-chg="stackdatumname" data-key="datum-name" value="${esc(el.name)}" aria-label="Name of the ${what}">`
      : esc(el.name || 'None');
    const at = (point) => `<span class="mono">${point[0]}, ${point[1]}</span>`;
    let html = typeSel('grid', grid ? 'Grid' : 'Reference plane', `<b>${esc(el.name || `Unnamed ${what}`)}</b>`);
    html += `<table class="props">${prow('Name', name + problem(`datum:${el.id}`))}`;
    if (view?.category) html += prow('Category', esc(view.category.name));
    html += prow('Shape', el.recipe.op === 'grid.arc' ? 'Arc' : outline.length ? 'Straight' : 'Level');
    if (outline.length) html += prow('Start', at(outline[0])) + prow('End', at(outline.at(-1)));
    html += `${prow('Claimed by', claim ? esc(claim.name) : 'Nobody')}</table>`;
    if (canEdit()) {
      const drag = el.recipe.op === 'grid.line' ? 'Drag an end to move it. ' : '';
      html += `<p class="pad small faint">${drag}Delete removes it.</p>`;
    }
    return html;
  }

  CHG.stackdatumname = (input) => {
    const piece = ownDatum(S.sel);
    if (!piece) return;
    const id = piece.el.id;
    const name = input.value;
    if (piece.el.kind === 'refplane') manage('plane.rename', { plane: id, name: name === '' ? null : name }, `datum:${id}`);
    else manage('grid.rename', { grid: id, name }, `datum:${id}`);
    input.blur();
  };

  const deleteElement = ACT.stackdelete;
  ACT.stackdelete = () => {
    const piece = ownDatum(S.sel);
    if (!piece) {
      deleteElement();
      return;
    }
    if (!canEdit()) return;
    const id = piece.el.id;
    if (piece.el.kind === 'refplane') manage('plane.delete', { plane: id }, `datum:${id}`);
    else manage('grid.delete', { grid: id }, `datum:${id}`);
    S.sel = null;
    refresh();
  };

  /** The ribbon: the grid tool beside the Build panel, and Delete for a selected grid or plane. */
  function datumRibbon(config) {
    const arch = config.tabs?.find((tab) => tab.id === 'arch');
    if (arch) {
      const build = arch.panels.findIndex((panel) => panel.label === 'Build');
      arch.panels.splice(build + 1, 0, { label: 'Datum', items: [DT_('architecture', 'grid')] });
    }
    const piece = ownDatum(S.sel);
    if (piece && canEdit() && S.screen === 'plan') {
      const label = piece.el.kind === 'grid' ? 'Grids' : 'Reference planes';
      config.contextual?.push({
        id: 'sel',
        key: `sel:${S.sel}`,
        group: 'Modify',
        label,
        panels: [
          { label, items: [{ label: 'Delete', icon: 'del', act: 'stackdelete', kbd: 'Delete' }] },
          { label: 'Selection', items: [{ label: 'Deselect', icon: 'x', act: 'deselect' }] },
        ],
      });
    }
  }

  // --- The plan viewport ---

  function plan() {
    const screen = $('#sc-plan');
    if (screen.dataset.stack !== 'yes') {
      screen.dataset.stack = 'yes';
      screen.innerHTML =
        `<div class="center"><div class="canvas" id="pv-canvas"></div><div class="rb-nav" role="toolbar" aria-label="Zoom"><button data-act="zoom" data-a="in" aria-label="Zoom in" data-tl="Zoom in">${ic('zoomin')}</button><button data-act="zoom" data-a="out" aria-label="Zoom out" data-tl="Zoom out">${ic('zoomout')}</button><button data-act="zoom" data-a="fit" aria-label="Zoom to fit" data-tl="Zoom to fit">${ic('fit')}</button></div></div>` +
        `<aside class="side" id="pv-side" aria-label="Properties"></aside>`;
      watchThePlan();
    }
    HOMEV.architecture = S.view;
    $('#hint').innerHTML = hint();
    drawPlan();
    $('#pv-side').innerHTML = propertiesPalette();
    followTheLevel();
  }

  function hint() {
    if (C.viewing) return 'This browser can’t build this model, so it shows the server’s stored shapes. You can look, not edit.';
    if (C.readOnly) return 'A package is a fixed version: look, but it doesn’t change.';
    if (!C.model) return 'Pick a package from a Shared or Consumed folder to view it.';
    if (S.tool === 'grid') return gridHint();
    if (S.tool === 'floor' || S.tool === 'roof') return floorHint();
    if (S.tool === 'room') return '<b>Room:</b> click inside a loop of walls. Choose its name, number and upper limit in the properties palette. Esc to stop.';
    if (S.tool === 'wall') return S.pend ? '<b>Wall:</b> pick the end point. Esc to stop.' : '<b>Wall:</b> pick a start point. Choose its type in the properties palette.';
    if (S.tool === 'door') return '<b>Door:</b> click on a wall to place a door.';
    if (S.tool === 'family') return '<b>Family:</b> choose a family and a type in the properties palette, then click on a wall to place it.';
    if (S.tool === 'opening' || RECT_TOOLS.includes(S.tool)) return openingHint();
    return 'Click an element to see its data. Drag it to move it: starting to work on it claims it for you.';
  }

  /** Where an element is, for the palette: a layered wall's line from end to end, anything else its position. */
  function whereRow(e) {
    const { start, end } = e.recipe.args;
    if (e.recipe.op === LAYERED && Array.isArray(start) && Array.isArray(end)) {
      const point = ([x, y]) => `${e.placement.x + storedMm(x)}, ${e.placement.y + storedMm(y)}`;
      return prow('Line', `<span class="mono">${point(start)} to ${point(end)}</span>`);
    }
    return prow('Position', `<span class="mono">${placementOf(e).x}, ${placementOf(e).y}</span>`);
  }

  function propertiesPalette() {
    const piece = C.elements.get(S.sel);
    let html = '<div class="sec">';
    const floor = floorPalette(piece) || roofPalette(piece);
    const room = roomPalette(piece);
    if (floor) {
      html += floor;
    } else if (room) {
      html += room;
    } else if (S.tool === 'wall' && canEdit()) {
      html += wallToolHtml();
    } else if (S.tool === 'family' && canEdit()) {
      html += familyToolHtml();
    } else if ((S.tool === 'opening' || RECT_TOOLS.includes(S.tool)) && canEdit()) {
      html += openingToolHtml();
    } else if (piece) {
      const { el: e, shape, failed } = piece;
      const claim = heldClaim(e.id);
      const icon = ['wall', 'door', 'window'].includes(e.kind) ? e.kind : 'family';
      html += typeSel(icon, esc(e.kind), `<b>${esc(e.name)}</b>`);
      const view = paramsFor(piece);
      html += `<table class="props">${whereRow(e)}${prow('Level', esc(levelName(e)))}${prow('Recipe', `<span class="mono">${esc(e.recipe.op)}</span>`)}`;
      if (view) html += prow('Category', esc(view.category?.name || 'None')) + prow('Type', typeCell(view));
      if (shape) {
        const [min, max] = [shape.bounds.min, shape.bounds.max];
        html += prow('Size', `${max[0] - min[0]} × ${max[1] - min[1]} × ${max[2] - min[2]} mm`);
        html += prow('Body', `<span class="mono">${esc(shape.body.slice(0, 12))}</span>`);
      } else {
        html += prow('Shape', esc(failed?.message || 'Not built yet'));
      }
      for (const host of e.references) html += prow('Hosted on', esc(host.element));
      const shown = new Set((view?.parameters || []).map((p) => p.id));
      for (const [key, value] of Object.entries(e.props)) {
        if (!key.startsWith('@') && !shown.has(key)) html += prow(esc(key), esc(value));
      }
      html += prow('Claimed by', claim ? esc(claim.name) : 'Nobody');
      html += '</table>';
      if (isFamily(e)) html += familyProperties(piece);
      if (isVoid(e)) html += voidProperties(piece);
      if (view?.parameters.length) html += parametersTable(view);
      if (view) html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
    } else if (ownDatum(S.sel)) {
      html += datumProperties(ownDatum(S.sel));
    } else {
      const view = VIEWS[S.view];
      html += typeSel('plan', esc(view?.kind || ''), `<b>${esc(C.model?.name || view?.name || '')}</b>`);
      html += '<p class="pad small faint">Nothing selected.</p>';
    }
    return `${html}</div><div class="sec"><span class="lbl">Edits</span><ol class="stack-log pad" id="stack-log">${C.log.join('')}</ol></div>`;
  }

  function heldClaim(id) {
    const claim = C.claims.get(id);
    return claim && claim.until_ms > Date.now() ? claim : null;
  }

  function claimedByOther(id) {
    const claim = heldClaim(id);
    return claim && claim.person !== C.me.person.id ? claim : null;
  }

  let drawQueued = false;
  function queueDraw() {
    if (drawQueued) return;
    drawQueued = true;
    requestAnimationFrame(() => {
      drawQueued = false;
      spent('draw', () => {
        drawPlan();
        statusBar();
        if (C.three) C.three.update();
      });
    });
  }

  function drawPlan() {
    const canvas = $('#pv-canvas');
    if (!canvas || S.screen !== 'plan') return;
    if (!C.model && !C.readOnly) {
      canvas.innerHTML = `<p class="pad faint stack-later">${hint()}</p>`;
      return;
    }
    const view = VIEWS[S.view];
    const scale = AS[S.scale] || 1;
    seatFamilies();
    let shapes = '';
    shapes += drawnPlan(); // the server's picture of the open view, under everything live
    shapes += datumsSvg(scale); // grids and planes, under the elements
    const ghost = (piece) => {
      if (piece.shape && levelIdOf(piece.el) === levelOf()) shapes += outlineOf(piece, 'ln fn dsh stack-ghost');
    };
    for (const pieces of C.linked.values()) for (const piece of pieces.values()) ghost(piece);
    for (const pieces of C.fixed.values()) for (const piece of pieces) ghost(packaged(piece));

    // Floors first, then walls, so a door in a wall stays on top and can be picked.
    const inOrder = [...C.elements.values()].sort((a, b) => drawOrder(a.el) - drawOrder(b.el));
    for (const piece of inOrder) {
      const { el: e } = piece;
      const { x, y } = placementOf(e);
      const data = `class="element" data-id="${esc(e.id)}" data-x="${x}" data-y="${y}"`;
      if (isRoom(e)) {
        shapes += roomInPlan(piece, data, scale);
        continue;
      }
      if (!piece.shape) {
        // Nothing to draw yet, or a recipe that can't build: a mark, so it can still be found.
        shapes += `<g ${data}>${RC(x - 200, y - 200, 400, 400, 'ln fn dsh')}</g>`;
        continue;
      }
      if (isFloor(e)) {
        shapes += floorInPlan(piece, data, scale);
        continue;
      }
      if (isVoid(e)) {
        shapes += voidInPlan(piece, data, scale);
        continue;
      }
      if (isRoof(e)) {
        shapes += roofInPlan(piece, data, scale);
        continue;
      }
      const paint = e.kind === 'wall' ? 'wall' : e.kind === 'door' || e.kind === 'window' ? 'ln ap af' : 'ln sf';
      const box = around(piece);
      let group = S.sel === e.id ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
      // A shape of several parts (a layered wall's layers) draws each in its material's cut colour,
      // under its outline.
      const parts = piece.shape.displays[0]?.parts;
      if (parts?.length) group += partsOf(piece, parts);
      group += outlineOf(piece, `body ${parts?.length ? 'layered' : paint}${S.sel === e.id ? ' sel' : ''}`);
      const claim = heldClaim(e.id);
      if (claim) {
        const theirs = claim.person !== C.me.person.id;
        group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
        if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
      } else if (e.kind === 'door' || e.kind === 'window') {
        group += TX(box.x + box.w / 2, box.y - 200, scale, e.name, 'tf', 220);
      }
      shapes += `<g ${data}>${group}</g>`;
    }
    shapes += floorSketch();
    shapes += rectSketch();
    const underDrawing = C.drawn?.svg && C.drawn.view === C.openView ? ' drawn' : '';
    canvas.innerHTML = `${planNote()}<svg class="dw pan${underDrawing}${S.tool !== 'select' ? ' press' : ''}" viewBox="${viewBox(S.view)}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="${esc(view?.name || '')}"><rect class="paper" x="-20000" y="-20000" width="80000" height="80000"/>${shapes}</svg>`;
    for (const entry of C.arriving.splice(0)) {
      const now = Date.now();
      C.latencies.push(now - entry.author_ms);
      C.journeys.push({
        cmd: entry.cmd,
        to_server: entry.orderer_ms - entry.author_ms,
        to_page: entry.arrived_ms - entry.orderer_ms,
        to_screen: now - entry.arrived_ms,
        // The same to_page, cut in three: the server ordering the edit and writing it to this page's
        // stream, the frame on the wire until this page's message handler ran, and the handler
        // reaching the point of noting the edit. The stamps are kept so what the page was busy with
        // in between can be read with `busyIn`.
        server_late: entry.sent_ms - entry.orderer_ms,
        wire: entry.got_ms - entry.sent_ms,
        handling: entry.arrived_ms - entry.got_ms,
        orderer_ms: entry.orderer_ms,
        sent_ms: entry.sent_ms,
        got_ms: entry.got_ms,
        arrived_ms: entry.arrived_ms,
      });
      $('#hint').textContent = `${named(entry.author)}’s edit arrived in ${C.latencies.at(-1)} ms.`;
    }
  }

  function toModel(event) {
    const svg = $('#pv-canvas svg');
    const at = svg.createSVGPoint();
    at.x = event.clientX;
    at.y = event.clientY;
    return at.matrixTransform(svg.getScreenCTM().inverse());
  }

  const snap = (value) => Math.round(value / SNAP_MM) * SNAP_MM;

  /** The open model's wall types, read now if they have not come yet. */
  async function wallTypesNow() {
    const view = wallTypes();
    if (view) return view;
    const model = C.model.id;
    const fresh = await api(`${modelUrl(model)}/wall-types`);
    C.wallTypes = { key: `${model}/${C.paramsEpoch}`, view: fresh };
    return fresh;
  }

  /**
   * The wall tool's second point draws the wall: straight along x or y, whichever the second point
   * is further along, as the plan draws walls, of the chosen type, from the plan's level to the
   * chosen top. What the server makes of it, or its reason for refusing, comes back through the log.
   */
  async function placeWall(at) {
    const point = [snap(at.x), snap(at.y)];
    if (!S.pend) {
      S.pend = point;
      $('#hint').innerHTML = hint();
      return;
    }
    const [ax, ay] = S.pend;
    S.pend = null;
    $('#hint').innerHTML = hint();
    const along = Math.abs(point[0] - ax) >= Math.abs(point[1] - ay);
    const end = along ? [point[0], ay] : [ax, point[1]];
    const level = levelOf();
    let view;
    try {
      view = await wallTypesNow();
    } catch (error) {
      say(error.message);
      return;
    }
    const args = { element: newId('W'), start: [`${ax} mm`, `${ay} mm`], end: [`${end[0]} mm`, `${end[1]} mm`], level };
    const type = chosenWallType(view);
    if (type) args.type = type.id;
    const top = wallTop(modelLevels());
    if (top === '') args.height = C.wallTool.height;
    else args.top = top;
    manage('wall.draw', args, 'wall-draw');
  }

  /**
   * Where a click lands on a wall, for a door: the wall's side clicked (`exterior` or `interior`),
   * how far along the wall from its start the door's near end is when the click is its middle, how
   * thick the wall is, and the axis the wall runs along. A layered wall reads its line and faces from
   * its filled recipe; the old box runs along x, its exterior at its low y side, or along y, its
   * exterior at its high x side.
   */
  function onWall(wall, at, width) {
    if (wall.el.recipe.op === LAYERED) {
      const line = lineOf(wall);
      if (!line) return null;
      const { x, y } = wall.el.placement;
      const p = [snap(at.x) - x - line.start[0] / STEPS_PER_MM, snap(at.y) - y - line.start[1] / STEPS_PER_MM];
      const length = Math.hypot(line.d[0], line.d[1]);
      const alongIt = (p[0] * line.d[0] + p[1] * line.d[1]) / length;
      const right = (p[0] * line.d[1] - p[1] * line.d[0]) / length;
      const middle = (line.exterior + line.interior) / 2 / STEPS_PER_MM;
      const exterior = right > middle === line.exterior > line.interior;
      return {
        exterior,
        offset: Math.round(alongIt - width / 2),
        thickness: Math.ceil(Math.abs(line.exterior - line.interior) / STEPS_PER_MM),
        axis: line.axis === 1 ? 'y' : 'x',
      };
    }
    const box = around(wall);
    const along = box.w >= box.d;
    const middle = along ? box.y + box.d / 2 : box.x + box.w / 2;
    return {
      exterior: along ? at.y < middle : at.x > middle,
      offset: (along ? snap(at.x) - box.x : snap(at.y) - box.y) - width / 2,
      thickness: along ? box.d : box.w,
      axis: along ? 'x' : 'y',
    };
  }

  function placeDoor(at) {
    const wall = [...C.elements.values()].find((piece) => {
      if (piece.el.kind !== 'wall' || !piece.shape) return false;
      const box = around(piece);
      return (
        at.x >= box.x - 300 && at.x <= box.x + box.w + 300 &&
        at.y >= box.y - 300 && at.y <= box.y + box.d + 300
      );
    });
    if (!wall) {
      say('Click on a wall to place a door.');
      return;
    }
    const width = 900;
    // A door is hosted on a face of the wall, by the name the geometry gave it, not on the wall:
    // the side clicked, as far along the wall from its start as the click, less half the door, as
    // thick as the wall.
    const spot = onWall(wall, at, width);
    const face = spot && wall.shape.names.find((name) => name.role === (spot.exterior ? 'exterior' : 'interior'));
    if (!face) {
      say('That wall has no side to place a door on.');
      return;
    }
    const { offset, thickness } = spot;
    const args = { width, thickness, height: 2100, axis: spot.axis };
    edit('hosted.place', {
      element: `D-${crypto.randomUUID().slice(0, 8)}`,
      kind: 'door',
      host: wall.el.id,
      face: face.name,
      along: `${offset} mm`,
      up: '0 mm',
      recipe: recipe('door.single', args),
    });
  }

  /** Spare Construct's own operations, by what they make. */
  const recipe = (op, args) => ({ owner: 'construct', schema: 1, op, op_version: 1, args });

  function watchThePlan() {
    const canvas = $('#pv-canvas');
    let drag = null;
    let lastSent = 0;

    canvas.addEventListener('pointerdown', (event) => {
      if (event.button !== 0 || !canvas.querySelector('svg')) return;
      const at = toModel(event);
      if (S.tool !== 'select') {
        if (C.readOnly || !C.model) say('Open a model in your team’s WIP to add to it.');
        else if (S.tool === 'wall') placeWall(at);
        else if (S.tool === 'door') placeDoor(at);
        else if (S.tool === 'family') placeFamily(at);
        else if (S.tool === 'opening') placeOpening(at);
        else if (S.tool === 'room') placeRoom(at);
        else say(NOT_YET);
        return;
      }
      const hit = event.target.closest('[data-id]');
      if (!hit) {
        if (S.sel) select(null);
        return;
      }
      const id = hit.dataset.id;
      if (S.sel !== id) select(id);
      if (C.readOnly) return;
      const claim = claimedByOther(id);
      if (claim) {
        say(`${claim.name} is working on this.`);
        return;
      }
      if (isRoom(C.elements.get(id)?.el)) return; // a room is picked, not dragged: its point moves by room.move
      const { el } = C.elements.get(id);
      const placement = placementOf(el);
      edit('element.claim', { element: id });
      // A door hosted on a face slides along its wall, and is sent once, when the drag ends.
      const hosted = el.props?.['@host.along'] !== undefined && hostOf(el) !== null;
      // A floor opening and a shaft are moved by their outline in the palette, never dragged.
      drag = isVoid(el) && !hosted ? null : { id, dx: at.x - placement.x, dy: at.y - placement.y, hosted };
    });

    window.addEventListener('pointermove', (event) => {
      if (!drag) return;
      const piece = C.elements.get(drag.id);
      if (!piece || !$('#pv-canvas svg')) return;
      const at = toModel(event);
      const x = snap(at.x - drag.dx);
      const y = snap(at.y - drag.dy);
      if (drag.hosted) {
        const hosted = hostOf(piece.el);
        if (!hosted) return;
        const offset = alongOn(hosted.host, piece.el, x, y);
        if (offset === null || storedMm(piece.el.props['@host.along']) === offset) return;
        const props = { ...piece.el.props, '@host.along': `length:q0:${offset * 6400}` };
        piece.el = { ...piece.el, props };
        drag.along = offset;
        queueDraw();
        return;
      }
      const { placement } = piece.el;
      if (x === placement.x && y === placement.y) return;
      piece.el = { ...piece.el, placement: { ...placement, x, y } };
      queueDraw();
      const now = performance.now();
      if (now - lastSent >= SEND_EVERY_MS) {
        lastSent = now;
        edit('element.move', { element: drag.id, placement: piece.el.placement });
      }
    });

    const finish = () => {
      if (!drag) return;
      const piece = C.elements.get(drag.id);
      if (piece && drag.hosted) {
        if (drag.along !== undefined) {
          const move = isFamily(piece.el) ? 'family.move' : isWallOpening(piece.el) ? 'opening.move' : 'hosted.move';
          edit(move, { element: drag.id, along: `${drag.along} mm` });
        }
      } else if (piece) {
        edit('element.move', { element: drag.id, placement: piece.el.placement });
      }
      drag = null;
    };
    window.addEventListener('pointerup', finish);
    window.addEventListener('pointercancel', finish);
  }

  // --- The 3D viewport: three.js, loaded only when this view opens ---

  function threeD() {
    const screen = $('#sc-3d');
    if (screen.dataset.stack !== 'yes') {
      screen.dataset.stack = 'yes';
      screen.innerHTML = `<div class="center"><div class="canvas" id="iso-canvas"></div></div><aside class="side" id="iso-side" aria-label="Properties"></aside>`;
      startThree();
    }
    $('#hint').textContent = 'Drag to turn the view.';
    $('#iso-side').innerHTML = propertiesPalette();
    if (C.three) C.three.update();
  }

  async function startThree() {
    const THREE = await import('three');
    const { OrbitControls } = await import('three/addons/controls/OrbitControls.js');
    const { elementMeshes } = await import('/view3d.js');
    const box = $('#iso-canvas');
    const tokens = getComputedStyle(document.documentElement);
    const colour = (name) => new THREE.Color(tokens.getPropertyValue(name).trim());
    const scene = new THREE.Scene();
    scene.background = colour('--panel');
    scene.add(new THREE.AmbientLight(0xffffff, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.6);
    sun.position.set(1, 3, 2);
    scene.add(sun);
    const camera = new THREE.PerspectiveCamera(45, box.clientWidth / box.clientHeight, 100, 500000);
    camera.position.set(-6000, 14000, 20000);
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(box.clientWidth, box.clientHeight);
    box.append(renderer.domElement);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(6000, 0, 4000);
    // Turning model z into three.js's up reverses the triangles, so both sides are drawn.
    const paint = (name) =>
      new THREE.MeshLambertMaterial({ color: colour(name), side: THREE.DoubleSide });
    const paints = { wall: paint('--ink-3'), door: paint('--app'), other: paint('--sink') };
    // A part of a shape (a layer of a wall) in its material's colour seen in projection, one paint
    // per colour; a material the server doesn't list takes its element's paint.
    const byColour = new Map();
    const partPaint = (material) => {
      const colour = colourOf(material, 'surface');
      if (!colour) return null;
      if (!byColour.has(colour)) {
        byColour.set(colour, new THREE.MeshLambertMaterial({ color: new THREE.Color(colour), side: THREE.DoubleSide }));
      }
      return byColour.get(colour);
    };
    const meshes = elementMeshes(THREE, paints, partPaint);
    scene.add(meshes.group);
    C.three = {
      update() {
        seatFamilies();
        // Each where it sits: a hosted door on its wall (placementOf).
        const placed = [...C.elements.values()].filter((piece) => !isRoom(piece.el)).map((piece) => ({
          ...piece,
          el: { ...piece.el, placement: placementOf(piece.el) },
        }));
        meshes.update(placed);
      },
    };
    C.three.update();
    renderer.setAnimationLoop(() => {
      controls.update();
      renderer.render(scene, camera);
    });
  }


  // --- Placed families: a door or a window of the library, in a wall ---
  // The family tool lists the library's families and their types (`/families`), and a click on a wall
  // sends one `family.place` edit: the family, its type, the wall, the side clicked and how far along
  // the wall (construct-model's FORMAT.md, "Placed families"). A selected family shows its type and
  // the values each placed one holds in the properties palette: another type is `family.retype`, a
  // value `family.set`, a position `family.move` (a drag along its wall sends it too), the other side
  // `family.flip`, and Delete is `family.delete`. The page checks nothing: a refusal is the server's
  // words, under the options. In the plan and in 3D a family is what the browser's geometry builds
  // from its record, standing where its wall puts it, turned to face out of its side; where the wall
  // is cut (the kernel's package) the wall is built with it. Interim, as the library is (job 7.6): the
  // families are the library's baked in, and the old box door tool stays beside this one until it is
  // retired.

  const FAMILY_OP = 'family.instance';
  const isFamily = (record) => record?.recipe?.op === FAMILY_OP && record?.recipe?.owner === 'construct';
  C.library = null; // { model, list } of the open model
  C.familyTool = { family: null, type: null, values: {} }; // the tool's choices: a family's permanent ID, a type's key

  DTOOLS.architecture.push(['family', 'Family', 'family', 'Choose a family and a type in the properties palette, then click on a wall to place it.']);
  TOOLNAME.family = 'family';

  /** The ribbon: the family tool beside the door tool, in the Build panel. */
  function familyRibbon(config) {
    const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
    if (!build || build.items.some((item) => item.a === 'architecture:family')) return;
    const door = build.items.findIndex((item) => item.a === 'architecture:door');
    build.items.splice(door + 1, 0, DT_('architecture', 'family'));
  }

  /** The library's families, with their types and the values each placed one holds; null until they have come. */
  function library() {
    if (!C.model) return null;
    const model = C.model.id;
    if (C.library?.model !== model && C.libraryLoading !== model) {
      C.libraryLoading = model;
      api(`${modelUrl(model)}/families`)
        .then((list) => {
          C.library = { model, list };
          refreshSide();
        })
        .catch(() => {});
    }
    return C.library?.model === model ? C.library.list : null;
  }

  function chosenFamily(list) {
    return list.find((family) => family.id === C.familyTool.family) ?? list[0] ?? null;
  }

  function chosenFamilyType(family) {
    return family.types.find((type) => type.key === C.familyTool.type) ?? family.types[0] ?? null;
  }

  /** A stored value as a person reads it: a length in millimetres, a choice by its label. */
  function shownValue(value, stored) {
    if (stored === undefined) return value.shown;
    if (value.kind === 'length') return `${storedMm(stored)} mm`;
    if (value.kind === 'choice') return value.choices.find((choice) => `choice:${choice.key}` === stored)?.label ?? stored;
    return stored;
  }

  /** A value's input: a choice is a list of its labels, anything else typed text. */
  function valueInput(value, shown, key, chg = 'stackfamilyvalue') {
    if (value.kind === 'choice') {
      const options = value.choices.map((choice) => `<option${choice.label === shown ? ' selected' : ''}>${esc(choice.label)}</option>`);
      return `<select class="inp" data-chg="${chg}" data-key="${key}" data-a="${esc(value.key)}" aria-label="${esc(value.name)}">${options.join('')}</select>`;
    }
    return `<input class="inp" data-chg="${chg}" data-key="${key}" data-a="${esc(value.key)}" value="${esc(shown)}" aria-label="${esc(value.name)}">`;
  }

  function familyToolHtml() {
    let html = typeSel('family', 'Family', '<b>New door or window</b>');
    const list = library();
    if (!list) return `${html}<p class="pad small faint">Loading the library.</p>`;
    const family = chosenFamily(list);
    if (!family) return `${html}<p class="pad small faint">The library has no families.</p>`;
    const type = chosenFamilyType(family);
    const families = list.map((f) => `<option value="${esc(f.id)}"${f.id === family.id ? ' selected' : ''}>${esc(f.name)}</option>`);
    const types = family.types.map((t) => `<option value="${esc(t.key)}"${t.key === type.key ? ' selected' : ''}>${esc(t.name)}</option>`);
    html += '<table class="props">';
    html += prow('Family', `<select class="inp" data-chg="stackfamilychoice" data-key="family-choice" aria-label="Family">${families.join('')}</select>`);
    html += prow('Category', esc(family.category_name));
    html += prow('Family type', `<select class="inp" data-chg="stackfamilytype" data-key="family-type" aria-label="Type of the family">${types.join('')}</select>`);
    for (const value of family.values) {
      const typed = C.familyTool.values[value.key] ?? value.shown;
      html += prow(esc(value.name), valueInput(value, typed, `family-tool-value:${value.key}`, 'stackfamilytoolvalue'));
    }
    html += '</table>';
    html += '<p class="pad small muted">Click on a wall. The side you click is the side it goes on, and the click is the middle of its opening.</p>';
    return html + problem('family-place');
  }

  CHG.stackfamilychoice = (select) => {
    C.familyTool = { family: select.value, type: null, values: {} };
    refreshSide();
  };
  CHG.stackfamilytype = (select) => {
    C.familyTool.type = select.value;
  };

  /** The library's family a placed one's record names, by its recipe's content id. */
  function familyOf(record) {
    return library()?.find((family) => family.content === record.recipe.args.family) ?? null;
  }

  /** What the properties palette shows of a selected family: its type, its values, its place, its side. */
  function familyProperties(piece) {
    const { el } = piece;
    const family = familyOf(el);
    if (!family) return '<p class="pad small faint">Reading its family from the library.</p>';
    const mine = !C.readOnly && canEdit();
    const types = family.types.map((t) => `<option value="${esc(t.key)}"${t.key === el.recipe.args.type ? ' selected' : ''}>${esc(t.name)}</option>`);
    let html = '<table class="props" data-family-properties="' + esc(el.id) + '">';
    html += prow('Family', esc(family.name));
    html += prow('Family type', mine ? `<select class="inp" data-chg="stackfamilyretype" data-key="family-retype" aria-label="Type of the family">${types.join('')}</select>` : esc(family.types.find((t) => t.key === el.recipe.args.type)?.name ?? el.recipe.args.type));
    for (const value of family.values) {
      const shown = shownValue(value, el.recipe.args[`value.${value.key}`]);
      html += prow(esc(value.name), mine ? valueInput(value, shown, `family-value:${value.key}`) : esc(shown));
    }
    const along = el.props?.['@host.along'];
    const face = (el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
    if (along !== undefined) {
      html += prow('Along its wall', mine ? `<input class="inp" data-chg="stackfamilyalong" data-key="family-along" value="${storedMm(along)} mm" aria-label="Along its wall">` : `${storedMm(along)} mm`);
    }
    if (face) html += prow('Side', `${esc(face.signature ?? '')}${mine ? ' <button class="btn sm" data-act="stackfamilyflip">Flip</button>' : ''}`);
    html += '</table>';
    return html + problem(`family:${el.id}`);
  }

  CHG.stackfamilyretype = (select) => {
    if (S.sel && canEdit()) manage('family.retype', { element: S.sel, type: select.value }, `family:${S.sel}`);
  };
  CHG.stackfamilyvalue = (input) => {
    if (!S.sel || !canEdit()) return;
    const typed = input.value.trim();
    manage('family.set', { element: S.sel, values: { [input.dataset.a]: typed === '' ? null : typed } }, `family:${S.sel}`);
    input.blur();
  };
  CHG.stackfamilyalong = (input) => {
    if (S.sel && canEdit()) manage('family.move', { element: S.sel, along: input.value }, `family:${S.sel}`);
    input.blur();
  };
  ACT.stackfamilyflip = () => {
    if (S.sel && canEdit()) manage('family.flip', { element: S.sel }, `family:${S.sel}`);
  };

  // The tool's own values, kept as typed until a family is placed.
  CHG.stackfamilytoolvalue = (input) => {
    C.familyTool.values[input.dataset.a] = input.value;
  };

  /**
   * A click on a wall with the family tool: the family goes on the side clicked, its opening's
   * middle where the click is, as far along the wall from its start (the server checks that it fits).
   */
  function placeFamily(at) {
    const list = library();
    const family = list && chosenFamily(list);
    if (!family) {
      say('The library has not come yet.');
      return;
    }
    const wall = [...C.elements.values()].find((piece) => {
      if (piece.el.kind !== 'wall' || !piece.shape) return false;
      const box = around(piece);
      return at.x >= box.x - 300 && at.x <= box.x + box.w + 300 && at.y >= box.y - 300 && at.y <= box.y + box.d + 300;
    });
    if (!wall) {
      say('Click on a wall to place it.');
      return;
    }
    const spot = onWall(wall, at, 0);
    if (!spot) {
      say('That wall has no side to place it on.');
      return;
    }
    const values = {};
    for (const [key, typed] of Object.entries(C.familyTool.values)) {
      if (family.values.some((value) => value.key === key) && typed !== '') values[key] = typed;
    }
    const args = {
      element: newId('F'),
      family: family.id,
      type: chosenFamilyType(family).key,
      host: wall.el.id,
      side: spot.exterior ? 'exterior' : 'interior',
      along: `${spot.offset} mm`,
    };
    if (Object.keys(values).length) args.values = values;
    manage('family.place', args, 'family-place');
  }

  /**
   * How a placed family sits on side `role` of a layered wall, as construct-regen's `hosting::seat`
   * works it out: its origin on the wall's centre plane, `along` the line from its start, `up` from
   * the wall's base, each to the nearest millimetre, and the turn (0 to 3 quarter turns about z) that
   * points its front out of that side. Null until the wall is filled, and for a wall at an angle.
   */
  function onFamily(role, host, along, up) {
    const line = lineOf(host);
    if (!line || line.axis === null || host.el.recipe.op !== LAYERED) return null;
    const centre = Math.floor((line.exterior + line.interior) / 2);
    const [ux, uy] = line.unit;
    const right = [uy, -ux];
    const outwardOfExterior = line.exterior > line.interior ? 1 : -1;
    const outward = role === 'interior' ? -outwardOfExterior : outwardOfExterior;
    const [nx, ny] = right.map((c) => c * outward);
    const turn = { '1,0': 0, '0,1': 1, '-1,0': 2, '0,-1': 3 }[`${ny},${-nx}`];
    if (turn === undefined) return null;
    const origin = [0, 1].map((i) => nearestMm(line.start[i] + steps(along) * line.unit[i] + centre * right[i]));
    const base = host.el.placement;
    return { at: { x: base.x + origin[0], y: base.y + origin[1], z: base.z + storedMm(up) }, turn };
  }

  const turnedRecipe = (recipe, turn) => ({ ...recipe, args: { ...recipe.args, turn } });

  /** Where a placed family sits and how it is turned, on its wall (`host`, or the one it names, if it is here). */
  function seatOfPiece(piece, host) {
    const hosted = hostOf(piece.el);
    const wall = host ?? hosted?.host;
    const face = hosted?.face ?? (piece.el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
    const along = piece.el.props?.['@host.along'];
    const up = piece.el.props?.['@host.up'];
    if (!wall || !face || along === undefined || up === undefined) return null;
    const role = wall.shape?.names?.find((name) => name.name === face.name)?.role ?? face.signature;
    return onFamily(role, wall, along, up);
  }

  /** Builds each placed family again, turned as its wall seats it, where it is not built so yet. */
  function seatFamilies() {
    if (!C.model || C.viewing) return;
    for (const piece of C.elements.values()) {
      if (!isFamily(piece.el) || piece.turning || piece.failed) continue;
      const seat = seatOfPiece(piece);
      if (seat && piece.seatedTurn !== seat.turn) reseat(piece, seat.turn);
    }
  }

  async function reseat(piece, turn) {
    piece.turning = true;
    try {
      const row = C.models.get(C.model.id);
      const builder = row && (await builderFor(row));
      if (!builder) return;
      const answer = buildOn(builder, { ...piece.el, recipe: turnedRecipe(piece.el.recipe, turn) }, null, null);
      Object.assign(piece, { shape: undefined, failed: undefined }, answer, { seatedTurn: turn, stale: false });
      queueDraw();
    } finally {
      piece.turning = false;
    }
  }

  const deleteOther = ACT.stackdelete;
  ACT.stackdelete = () => {
    const piece = C.elements.get(S.sel);
    if (!piece || !isFamily(piece.el)) {
      deleteOther();
      return;
    }
    if (!canEdit()) return;
    manage('family.delete', { element: S.sel }, `family:${S.sel}`);
    S.sel = null;
    refresh();
  };

  // --- Openings and shafts: a hole in a wall, a hole in a floor, a shaft through the floors ---
  // Three tools make them, one edit each (construct-openings' FORMAT.md; the commands are
  // construct-api's `cloud/openings.rs`). The Opening tool takes a click on a wall, as the door
  // tool does: the side clicked, the click the middle of the opening, its width, height and sill
  // typed in the properties palette; `opening.place`. The Floor opening tool and the Shaft tool take
  // two clicks, the opposite corners of a rectangle drawn in the plan: the floor under the first
  // corner is the one `opening.draw` cuts, and a shaft (`shaft.create`) runs between the two levels
  // chosen in the properties palette, each with an offset. A selected opening or shaft shows its
  // size, place or reach in the palette, and a change is `opening.size`, `opening.move`,
  // `opening.outline`, `shaft.outline` or `shaft.reach`; Delete is `opening.delete` or
  // `shaft.delete`. The page checks nothing: a refusal is the server's words, beside the tool or
  // the field, and the page draws nothing wrong because it draws only what the log holds. In the
  // plan an opening in a wall is the rectangle it opens through its wall, a floor opening and a
  // shaft their outlines, and a floor is drawn with the holes the server says cut it
  // (`/filled/<floor>`, `cut_by`); in 3D the wall and the floor are what the browser's geometry
  // builds with them, cut where the package cuts (the kernel's).

  const WALL_OPENING_OP = 'opening.wall';
  const FLOOR_OPENING_OP = 'opening.floor';
  const SHAFT_OP = 'shaft';
  const ownOp = (record, op) => record?.recipe?.owner === 'construct' && record.recipe.op === op;
  const isWallOpening = (record) => ownOp(record, WALL_OPENING_OP);
  const isFloorOpening = (record) => ownOp(record, FLOOR_OPENING_OP);
  const isShaft = (record) => ownOp(record, SHAFT_OP);
  const isVoid = (record) => isWallOpening(record) || isFloorOpening(record) || isShaft(record);

  DTOOLS.architecture.push(
    ['opening', 'Opening', 'window', 'Click on a wall to cut an opening through it. Set its size in the properties palette.'],
    ['floor-opening', 'Floor opening', 'room', 'Click two opposite corners inside a floor to cut an opening through it.'],
    ['shaft', 'Shaft', 'cube', 'Click two opposite corners of its outline. Choose the levels it runs between in the properties palette.'],
  );
  Object.assign(TOOLNAME, { opening: 'opening', 'floor-opening': 'floor opening', shaft: 'shaft' });
  const RECT_TOOLS = ['floor-opening', 'shaft'];
  Object.assign(WORDS, {
    'opening.place': 'cut an opening in a wall:',
    'opening.draw': 'cut an opening in a floor:',
    'opening.size': 'resized',
    'opening.move': 'moved',
    'opening.outline': 'redrew',
    'opening.delete': 'deleted',
    'shaft.create': 'made a shaft:',
    'shaft.outline': 'redrew',
    'shaft.reach': 'changed where it starts and ends:',
    'shaft.delete': 'deleted',
  });
  Object.assign(STEP_WORDS, {
    'opening.place': 'cutting an opening in a wall',
    'opening.draw': 'cutting an opening in a floor',
    'opening.size': 'the size of an opening',
    'opening.move': 'moving an opening',
    'opening.outline': 'the outline of an opening',
    'opening.delete': 'deleting an opening',
    'shaft.create': 'making a shaft',
    'shaft.outline': 'the outline of a shaft',
    'shaft.reach': 'the reach of a shaft',
    'shaft.delete': 'deleting a shaft',
  });

  C.openingTool = { width: '1000 mm', height: '1200 mm', sill: '900 mm' }; // typed, as the server reads them
  C.shaftTool = { level: null, offset: '-300 mm', top: null, topOffset: '0 mm' };
  // The rectangle being drawn for a floor opening or a shaft: its first corner and the point under the pointer.
  const RECT = { from: null, at: null };

  /** The ribbon: the opening tools beside the family tool, in the Build panel. */
  function openingRibbon(config) {
    const build = config.tabs?.find((tab) => tab.id === 'arch')?.panels.find((panel) => panel.label === 'Build');
    if (!build || build.items.some((item) => item.a === 'architecture:opening')) return;
    const family = build.items.findIndex((item) => item.a === 'architecture:family');
    build.items.splice(family + 1, 0, ...['opening', 'floor-opening', 'shaft'].map((tool) => DT_('architecture', tool)));
  }

  /** A typed width in millimetres, for centring an opening on a click; 1000 when it does not read. */
  function typedMm(text) {
    const number = Number.parseFloat(text);
    if (!Number.isFinite(number)) return 1000;
    return /\d\s*m\s*$/.test(text) && !/mm\s*$/.test(text) ? number * 1000 : number;
  }

  /** The levels, the one the shaft starts on and the one it ends on, as the tool offers them. */
  function shaftChoice() {
    const levels = modelLevels() || [];
    const known = (id) => id && levels.some((level) => level.id === id);
    const level = known(C.shaftTool.level) ? C.shaftTool.level : levelOf();
    const above = levels[levels.findIndex((l) => l.id === level) + 1]?.id ?? level;
    const top = known(C.shaftTool.top) ? C.shaftTool.top : above;
    return { levels, level, top };
  }

  const levelOptions = (levels, chosen) =>
    levels.map((l) => `<option value="${esc(l.id)}"${l.id === chosen ? ' selected' : ''}>${esc(l.name)}</option>`).join('');

  /** The properties palette of the three tools. */
  function openingToolHtml() {
    if (S.tool === 'opening') {
      let html = typeSel('window', 'Opening', '<b>New opening in a wall</b>');
      html += '<table class="props">';
      for (const [key, label] of [['width', 'Width'], ['height', 'Height'], ['sill', 'Sill height']]) {
        html += prow(label, `<input class="inp" data-chg="stackopeningtool" data-a="${key}" data-key="opening-tool-${key}" value="${esc(C.openingTool[key])}" aria-label="${label} of the opening">`);
      }
      html += '</table><p class="pad small muted">Click on a wall. The side you click is the side it goes on, and the click is the middle of the opening.</p>';
      return html + problem('opening-place');
    }
    if (S.tool === 'floor-opening') {
      const html = typeSel('room', 'Opening', '<b>New opening in a floor</b>');
      const text = RECT.from ? 'Click the opposite corner.' : 'Click one corner of the opening, inside a floor, then the opposite corner.';
      return `${html}<p class="pad small muted" data-opening-sketch="${RECT.from ? 'open' : 'empty'}">${esc(text)}</p>${problem('opening-draw')}`;
    }
    const { levels, level, top } = shaftChoice();
    let html = typeSel('cube', 'Shaft', '<b>New shaft</b>');
    html += '<table class="props">';
    html += prow('Starts on', `<select class="inp" data-chg="stackshafttool" data-a="level" data-key="shaft-tool-level" aria-label="Level the shaft starts on">${levelOptions(levels, level)}</select>`);
    html += prow('Start offset', `<input class="inp" data-chg="stackshafttool" data-a="offset" data-key="shaft-tool-offset" value="${esc(C.shaftTool.offset)}" aria-label="Offset of the shaft’s start from its level">`);
    html += prow('Ends on', `<select class="inp" data-chg="stackshafttool" data-a="top" data-key="shaft-tool-top" aria-label="Level the shaft ends on">${levelOptions(levels, top)}</select>`);
    html += prow('End offset', `<input class="inp" data-chg="stackshafttool" data-a="topOffset" data-key="shaft-tool-top-offset" value="${esc(C.shaftTool.topOffset)}" aria-label="Offset of the shaft’s end from its level">`);
    html += '</table>';
    const text = RECT.from ? 'Click the opposite corner.' : 'Click one corner of the shaft, then the opposite corner. It cuts every floor it passes through.';
    return `${html}<p class="pad small muted" data-shaft-sketch="${RECT.from ? 'open' : 'empty'}">${esc(text)}</p>${problem('shaft-create')}`;
  }

  CHG.stackopeningtool = (input) => {
    C.openingTool[input.dataset.a] = input.value;
  };
  CHG.stackshafttool = (control) => {
    C.shaftTool[control.dataset.a] = control.value;
    refreshSide();
  };

  function openingHint() {
    if (S.tool === 'opening') return '<b>Opening:</b> click on a wall to cut an opening through it. Set its size in the properties palette.';
    if (S.tool === 'floor-opening') return RECT.from ? '<b>Floor opening:</b> click the opposite corner. Esc to stop.' : '<b>Floor opening:</b> click one corner, inside a floor.';
    return RECT.from ? '<b>Shaft:</b> click the opposite corner. Esc to stop.' : '<b>Shaft:</b> click one corner of its outline. Choose its levels in the properties palette.';
  }

  /** The wall under a click, with the margin the door tool gives. */
  function wallAt(at) {
    return [...C.elements.values()].find((piece) => {
      if (piece.el.kind !== 'wall' || !piece.shape) return false;
      const box = around(piece);
      return at.x >= box.x - 300 && at.x <= box.x + box.w + 300 && at.y >= box.y - 300 && at.y <= box.y + box.d + 300;
    });
  }

  /** A click on a wall with the Opening tool: the side clicked, the click the middle of the opening. */
  function placeOpening(at) {
    const wall = wallAt(at);
    if (!wall) {
      say('Click on a wall to cut an opening in it.');
      return;
    }
    const spot = onWall(wall, at, typedMm(C.openingTool.width));
    if (!spot) {
      say('That wall has no side to cut an opening through.');
      return;
    }
    const args = {
      element: newId('O'),
      host: wall.el.id,
      side: spot.exterior ? 'exterior' : 'interior',
      along: `${spot.offset} mm`,
      sill: C.openingTool.sill,
      width: C.openingTool.width,
      height: C.openingTool.height,
    };
    manage('opening.place', args, 'opening-place');
  }

  /** The floor whose box holds a point, if one does. */
  function floorAt(at) {
    return [...C.elements.values()].find((piece) => {
      if (!isFloor(piece.el) || !piece.shape) return false;
      const box = around(piece);
      return at.x >= box.x && at.x <= box.x + box.w && at.y >= box.y && at.y <= box.y + box.d;
    });
  }

  /** The corners of a rectangle as the commands take an outline, in typed millimetres. */
  const rectOutline = ([ax, ay], [bx, by]) =>
    [[ax, ay], [bx, ay], [bx, by], [ax, by]].map(([x, y]) => [`${x} mm`, `${y} mm`]);

  function rectClick(event) {
    const at = toModel(event);
    const point = [snap(at.x), snap(at.y)];
    if (!RECT.from) {
      Object.assign(RECT, { from: point, at: point });
      sketchChanged();
      return;
    }
    const [from, to] = [RECT.from, point];
    if (from[0] === to[0] || from[1] === to[1]) {
      say('Click the opposite corner, away from the first, so the outline has an area.');
      return;
    }
    Object.assign(RECT, { from: null, at: null });
    if (S.tool === 'shaft') {
      const { level, top } = shaftChoice();
      const args = { element: newId('S'), outline: rectOutline(from, to), level, top };
      if (C.shaftTool.offset.trim() !== '') args.offset = C.shaftTool.offset;
      if (C.shaftTool.topOffset.trim() !== '') args.top_offset = C.shaftTool.topOffset;
      manage('shaft.create', args, 'shaft-create');
    } else {
      const floor = floorAt({ x: from[0], y: from[1] });
      if (floor) manage('opening.draw', { element: newId('O'), host: floor.el.id, outline: rectOutline(from, to) }, 'opening-draw');
      else say('Start the opening’s outline inside a floor.');
    }
    sketchChanged();
  }

  // In the plan, before the elements' own handlers: the two corners of a rectangle.
  document.addEventListener(
    'pointerdown',
    (event) => {
      if (!RECT_TOOLS.includes(S.tool) || event.button !== 0 || S.screen !== 'plan' || !event.target.closest?.('#pv-canvas svg')) return;
      event.stopPropagation();
      if (!canEdit()) say('Open a model in your team’s WIP to add to it.');
      else rectClick(event);
    },
    true,
  );
  window.addEventListener('pointermove', (event) => {
    if (!RECT_TOOLS.includes(S.tool) || !RECT.from || !$('#pv-canvas svg')) return;
    const at = toModel(event);
    RECT.at = [snap(at.x), snap(at.y)];
    queueDraw();
  });
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && RECT.from) {
      Object.assign(RECT, { from: null, at: null });
      queueDraw();
    }
  });
  const toolBeforeOpenings = ACT.tool;
  ACT.tool = (button) => {
    Object.assign(RECT, { from: null, at: null });
    toolBeforeOpenings(button);
  };

  /** The rectangle being drawn, from its first corner to the pointer. */
  function rectSketch() {
    if (!RECT_TOOLS.includes(S.tool) || !RECT.from) return '';
    const [ax, ay] = RECT.from;
    const [bx, by] = RECT.at ?? RECT.from;
    return `<rect class="void sketch" x="${Math.min(ax, bx)}" y="${Math.min(ay, by)}" width="${Math.abs(bx - ax)}" height="${Math.abs(by - ay)}"/>`;
  }

  /** An opening or a shaft in the plan: the rectangle a wall opening opens through its wall, or an outline. */
  function voidInPlan(piece, data, scale) {
    const { el: e } = piece;
    const chosen = S.sel === e.id;
    const paint = `body void ${isShaft(e) ? 'shaft' : 'opening'}${chosen ? ' sel' : ''}`;
    const { x, y } = placementOf(e);
    let box;
    let drawn;
    if (isWallOpening(e)) {
      const host = hostOf(e)?.host;
      const line = host && lineOf(host);
      const thickness = line ? Math.abs(line.exterior - line.interior) / STEPS_PER_MM : (host?.el.recipe.args.thickness ?? 200);
      const wide = storedMm(e.recipe.args.width);
      const [w, d] = e.recipe.args.axis === 'y' ? [thickness, wide] : [wide, thickness];
      box = { x, y, w, d };
      drawn = RC(x, y, w, d, paint);
    } else {
      const ring = (e.recipe.args.outline || []).map(([px, py]) => [x + nearestMm(px), y + nearestMm(py)]);
      const xs = ring.map((p) => p[0]);
      const ys = ring.map((p) => p[1]);
      box = { x: Math.min(...xs), y: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), d: Math.max(...ys) - Math.min(...ys) };
      drawn = `<path class="${paint}" d="${ringPath(ring)}"/>`;
    }
    let group = chosen ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
    group += drawn;
    const claim = heldClaim(e.id);
    if (claim) {
      const theirs = claim.person !== C.me.person.id;
      group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
      if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
    }
    return `<g ${data}>${group}</g>`;
  }

  /** A length stored in grid steps as millimetres, as typed. */
  const stepsMm = (steps) => `${Number((steps / STEPS_PER_MM).toFixed(3))} mm`;

  /** The corners of an outline as an axis-aligned rectangle, `[x0, y0, x1, y1]` in grid steps, or null for any other outline. */
  function rectangleOf(outline) {
    if (!Array.isArray(outline) || outline.length !== 4) return null;
    const xs = [...new Set(outline.map((p) => p[0]))];
    const ys = [...new Set(outline.map((p) => p[1]))];
    return xs.length === 2 && ys.length === 2 ? [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] : null;
  }

  /** What the properties palette shows of a selected opening or shaft: its size, its place, its reach. */
  function voidProperties(piece) {
    const { el } = piece;
    const mine = !C.readOnly && canEdit();
    const where = esc(el.id);
    const field = (key, shown, chg, a, label) =>
      mine
        ? `<input class="inp" data-chg="${chg}" data-a="${a}" data-key="${key}" value="${esc(shown)}" aria-label="${esc(label)}">`
        : esc(shown);
    let html = `<table class="props" data-opening-properties="${where}">`;
    if (isWallOpening(el)) {
      const hosted = hostOf(el);
      html += prow('Width', field('opening-width', stepsMm(steps(el.recipe.args.width)), 'stackopeningsize', 'width', 'Width of the opening'));
      html += prow('Height', field('opening-height', stepsMm(steps(el.recipe.args.height)), 'stackopeningsize', 'height', 'Height of the opening'));
      const along = el.props?.['@host.along'];
      const sill = el.props?.['@host.up'];
      if (along !== undefined) html += prow('Along its wall', field('opening-along', `${storedMm(along)} mm`, 'stackopeningmove', 'along', 'Along its wall'));
      if (sill !== undefined) html += prow('Sill height', field('opening-sill', `${storedMm(sill)} mm`, 'stackopeningmove', 'sill', 'Sill height'));
      const face = hosted?.face ?? (el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
      if (face) html += prow('Side', `${esc(face.signature ?? '')}${mine ? ' <button class="btn sm" data-act="stackopeningflip">Flip</button>' : ''}`);
    } else {
      const rectangle = rectangleOf(el.recipe.args.outline);
      if (rectangle) {
        const [x0, y0, x1, y1] = rectangle;
        const cmd = isShaft(el) ? 'shaft' : 'opening';
        html += prow('From x', field(`${cmd}-x`, stepsMm(x0), 'stackopeningrect', 'x', 'Left edge'));
        html += prow('From y', field(`${cmd}-y`, stepsMm(y0), 'stackopeningrect', 'y', 'Top edge'));
        html += prow('Width', field(`${cmd}-w`, stepsMm(x1 - x0), 'stackopeningrect', 'w', 'Width'));
        html += prow('Depth', field(`${cmd}-d`, stepsMm(y1 - y0), 'stackopeningrect', 'd', 'Depth'));
      } else {
        html += prow('Outline', esc(plural((el.recipe.args.outline || []).length, 'corner')));
      }
    }
    if (isShaft(el)) {
      const levels = modelLevels() || [];
      const offset = (key) => `${storedMm(el.props?.[key] ?? 'length:q0:0')} mm`;
      const select = (key, chosen, label) =>
        mine
          ? `<select class="inp" data-chg="stackshaftreach" data-a="${key}" data-key="shaft-${key}" aria-label="${label}">${levelOptions(levels, chosen)}</select>`
          : esc(levels.find((l) => l.id === chosen)?.name ?? chosen ?? '');
      html += prow('Starts on', select('level', el.props?.['@level'], 'Level the shaft starts on'));
      html += prow('Start offset', field('shaft-offset', offset('@level.offset'), 'stackshaftreach', 'offset', 'Offset of the shaft’s start'));
      html += prow('Ends on', select('top', el.props?.['@top.level'], 'Level the shaft ends on'));
      html += prow('End offset', field('shaft-top-offset', offset('@top.offset'), 'stackshaftreach', 'top_offset', 'Offset of the shaft’s end'));
    }
    html += '</table>';
    return html + problem(`opening:${el.id}`);
  }

  const openingWhere = () => `opening:${S.sel}`;
  CHG.stackopeningsize = (input) => {
    if (S.sel && canEdit()) manage('opening.size', { element: S.sel, [input.dataset.a]: input.value }, openingWhere());
    input.blur();
  };
  CHG.stackopeningmove = (input) => {
    if (S.sel && canEdit()) manage('opening.move', { element: S.sel, [input.dataset.a]: input.value }, openingWhere());
    input.blur();
  };
  ACT.stackopeningflip = () => {
    const face = (C.elements.get(S.sel)?.el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
    if (S.sel && canEdit() && face) manage('opening.move', { element: S.sel, side: face.signature === 'interior' ? 'exterior' : 'interior' }, openingWhere());
  };
  /** A rectangle's field changed: the outline is sent again with that edge moved. */
  CHG.stackopeningrect = (input) => {
    const piece = C.elements.get(S.sel);
    const rectangle = piece && rectangleOf(piece.el.recipe.args.outline);
    if (!rectangle || !canEdit()) return;
    const mm = (steps) => steps / STEPS_PER_MM;
    const [x0, y0, x1, y1] = rectangle.map(mm);
    const typed = Number.parseFloat(input.value);
    const next = { x: x0, y: y0, w: x1 - x0, d: y1 - y0 };
    next[input.dataset.a] = typed;
    const corners = rectOutline([next.x, next.y], [next.x + next.w, next.y + next.d]);
    const cmd = isShaft(piece.el) ? 'shaft.outline' : 'opening.outline';
    manage(cmd, { element: S.sel, outline: corners }, openingWhere());
    input.blur();
  };
  CHG.stackshaftreach = (control) => {
    const piece = C.elements.get(S.sel);
    if (!piece || !canEdit()) return;
    const props = piece.el.props || {};
    const length = (key) => `${storedMm(props[key] ?? 'length:q0:0')} mm`;
    const args = {
      element: S.sel,
      level: props['@level'],
      offset: length('@level.offset'),
      top: props['@top.level'],
      top_offset: length('@top.offset'),
    };
    args[control.dataset.a] = control.value;
    manage('shaft.reach', args, openingWhere());
    control.blur();
  };

  const deleteBeforeOpenings = ACT.stackdelete;
  ACT.stackdelete = () => {
    const piece = C.elements.get(S.sel);
    if (!piece || !isVoid(piece.el)) {
      deleteBeforeOpenings();
      return;
    }
    if (!canEdit()) return;
    manage(isShaft(piece.el) ? 'shaft.delete' : 'opening.delete', { element: S.sel }, openingWhere());
    S.sel = null;
    refresh();
  };

  boot();
})();
