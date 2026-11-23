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
    'detailset', 'disc',
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
  };
  window.spare = C; // the browser test reads this

  const named = (person) => person[0].toUpperCase() + person.slice(1);
  // The level the plan shows and a new element is tied to: one chosen in Levels, or the floor plan's own.
  const levelOf = () => C.planLevel?.id ?? (S.view === 'l2' ? '2' : '1');
  const planName = () => (C.planLevel ? `${C.planLevel.name} plan` : VIEWS[S.view]?.name);
  /** The ID of the level a record stands on: its tie, or the level its old integer names (construct-levels' FORMAT.md). */
  const levelIdOf = (record) =>
    record.props?.['@level'] ?? (record.level > 0 ? String(record.level) : record.level < 0 ? `m${-record.level}` : null);
  const say = (text) => toast(esc(text));
  const initials = (name) => name.slice(0, 2).toUpperCase();

  async function api(path, options) {
    const response = await fetch(path, options);
    const body = await response.json().catch(() => ({}));
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
    for (const open of ['Projects', 'Views', 'Project files', 'Linked models']) BOPEN.add(open);

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
      $('#status-r').innerHTML = `<span id="st-log" class="mono">${esc(waiting + checks + speed)}Edit ${C.head}</span>`;
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
      const kind = piece.el.kind === 'wall' ? 'Walls' : piece.el.kind === 'door' ? 'Doors' : 'Element';
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
      `<div class="sub">Floor plans</div>${['l1', 'l2'].map((v) => leaf('view', v, VIEWS[v].name, S.screen === 'plan' && S.view === v, 'plan')).join('')}<div class="sub">3D views</div>${leaf('go', '3d', 'Default 3D', S.screen === '3d', 'cube')}`,
    );
    if (C.data) tree += group('Project files', projectFiles(leaf));
    if (C.model) tree += group('Linked models', linkedModels());
    if (C.people) {
      const names = (list) => esc(list.join(', ')) || 'None';
      tree += group(
        'Hub people',
        `<p class="pad small"><b>Users:</b> ${names(C.people.users)}<br><b>External:</b> ${names(C.people.externals)}</p>`,
      );
    }
    box.innerHTML = `<div class="tree">${tree}</div>`;
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
    C.params = C.typeList = C.manage = null;
    C.problems.clear();
    C.asked.clear();
    C.cuts = false;
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
    await loadWorkshop();
    await loadSteps();
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
   * `build_hosting`); the stand-in's builds a host as the wall alone, as its `build` does.
   */
  function buildOn(builder, record, into) {
    const guests = typeof builder.shapes.build_hosting === 'function' && into ? guestsOf(record, into) : [];
    if (!guests.length) return buildShape(builder, record.recipe);
    // Each guest by its place in the host's frame, so a host that moves builds to the same text.
    const host = JSON.stringify({ recipe: record.recipe, placement: { x: 0, y: 0, z: 0 } });
    const hosted = JSON.stringify(guests);
    const text = `${host}\n${hosted}`;
    if (!builder.hosting.has(text)) builder.hosting.set(text, JSON.parse(builder.shapes.build_hosting(host, hosted)));
    return builder.hosting.get(text);
  }

  async function shapeFor(model, record, into) {
    const row = C.models.get(model);
    const builder = row && (await builderFor(row));
    return builder ? buildOn(builder, record, into) : storedShape(model, record.id);
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
      const opening = el.recipe?.owner === 'construct' && el.recipe?.op === 'door.single';
      return opening && faces.length ? faces[0].element : null;
    }
    return along !== undefined && up !== undefined && faces.length === 1 ? faces[0].element : null;
  }

  /**
   * What is hosted in `host` among `into`'s elements, as the kernel's package takes them: each by
   * its ID, its recipe and its place in the host's frame (in whole millimetres), as the server's
   * regenerator hands them to its builder.
   */
  function guestsOf(host, into) {
    const drawn = { el: host, shape: into.get(host.id)?.shape };
    const guests = [];
    for (const [id, piece] of into) {
      if (id === host.id || hostIdOf(piece.el) !== host.id) continue;
      const face = (piece.el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
      const at = placedOn(piece.el, face, drawn);
      const placement = { x: at.x - host.placement.x, y: at.y - host.placement.y, z: at.z - host.placement.z };
      guests.push({ key: id, recipe: piece.el.recipe, placement });
    }
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

  /** Where `el` sits hosted on `face` of `host` (`{el, shape}`): `placementOf`'s rule. */
  function placedOn(el, face, host) {
    const along = el.props?.['@host.along'];
    const up = el.props?.['@host.up'];
    if (along === undefined || up === undefined) return el.placement;
    const role = host.shape?.names?.find((name) => name.name === face.name)?.role ?? face.signature;
    const thickness = host.el.recipe.args.thickness;
    const own = el.recipe.args.thickness;
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

  /** Compares what the server built with what this page holds (note A05). */
  function compareCheckpoint(entry) {
    const { through, bodies } = entry.args;
    const into = entry.target === C.model?.id ? C.elements : C.linked.get(entry.target);
    if (!into) return;
    for (const [id, body] of Object.entries(bodies)) {
      const piece = into.get(id);
      if (!piece || piece.seq > through || piece.stale) continue;
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
    const answer = await shapeFor(model, record, into);
    if (levelIdOf(record) === levelOf()) into.set(id, { el: record, ...answer, hash, seq });
    // What it is hosted in is built again with it, as the server builds a host with its guests.
    if (cutsIn(model, into)) await buildHostAgain(model, into, hostIdOf(record));
  }

  /** Whether this page builds `model`'s hosts with what is hosted in them, into `into`. */
  const cutsIn = (model, into) => C.cuts && model === C.model?.id && into === C.elements;

  /** Builds host `id` again with what is now hosted in it on this page, and lets it be compared. */
  async function buildHostAgain(model, into, id) {
    const host = id && into.get(id);
    if (!host) return;
    const answer = await shapeFor(model, host.el, into);
    if (into.get(id) !== host) return;
    Object.assign(host, { shape: undefined, failed: undefined }, answer);
    host.stale = false;
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
    socket.onmessage = (event) => receive(JSON.parse(event.data));
    socket.onclose = () => {
      C.connected = false;
      setStatus('Reconnecting', false);
      setTimeout(() => {
        if (C.socket === socket) connect();
      }, 500);
    };
  }

  function receive(message) {
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
        apply(message.entry);
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

  function apply(entry) {
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
    if (!mine && fresh) C.arriving.push({ ...entry, arrived_ms: Date.now() });
    const into = entry.target === C.model?.id ? C.elements : C.linked.get(entry.target);
    if (!into) return;
    // This page drew its own edits ahead of the log, but not an undo: the server works that out.
    const drawn = mine && !UNDOING.has(entry.cmd);
    for (const change of entry.changes) {
      if (entry.target === C.model?.id && WS.elements.has(change.piece)) {
        workshopEntry(entry, change, drawn);
        continue;
      }
      const piece = into.get(change.piece);
      if (piece && piece.seq >= entry.seq) continue;
      const stillMine = C.pending.some((edit) => edit.args.element === change.piece);
      // A host is built with what is hosted in it: while what it hosts changes here, it is not
      // compared with the server's, and once that is in hand it is built again.
      const cuts = cutsIn(entry.target, into);
      const hosts = new Set([piece && hostIdOf(piece.el), entry.cmd === 'hosted.place' && entry.args.host]);
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
    // A type or a parameter changed, or may have: what the selected element reads may have too.
    if ((/^(type|parameter|level)\./.test(entry.cmd) || UNDOING.has(entry.cmd)) && entry.target === C.model?.id) {
      C.paramsEpoch += 1;
      refreshSide();
    }
    if (/^(type|parameter|level)\.|^element\.(type|create|delete)$/.test(entry.cmd) || UNDOING.has(entry.cmd)) {
      if (entry.target === C.model?.id) loadManage();
    }
    if (entry.target === C.model?.id) {
      for (const change of entry.claims) {
        if (change.claim) C.claims.set(change.piece, change.claim);
        else C.claims.delete(change.piece);
      }
    }
    queueDraw();
  }

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
    'level.create': 'made a level',
    'level.rename': 'renamed a level',
    'level.elevation': 'moved a level',
    'level.storey': 'changed a level',
    'level.delete': 'deleted a level',
    'level.tie': 'tied to a level',
    'level.top': 'tied the top of',
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
    'level.create': 'making a level',
    'level.rename': 'renaming a level',
    'level.elevation': 'moving a level',
    'level.storey': 'a change to a level',
    'level.delete': 'deleting a level',
    'level.tie': 'a tie to a level',
    'level.top': 'a tie of a top',
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
    return record.kind !== 'type' && record.kind !== 'parameters' && record.kind !== 'level';
  }
  C.params = null; // { key, view } of the selected element
  C.paramsEpoch = 0; // goes up when a type or a parameter changes
  C.edit = edit; // the browser test defines parameters through the page's own edit path

  function paramsFor(piece) {
    if (!C.model) return null;
    const key = `${piece.el.id}/${piece.hash}/${C.paramsEpoch}`;
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
    for (const p of view.parameters) {
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
      edit('element.delete', { element: S.sel });
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
    refreshSide();
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
    const title = { types: 'Types', parameters: 'Project parameters', levels: 'Levels' }[which];
    openDlg(title, `<div id="stack-manage" class="stack-manage" data-which="${which}"><p class="small faint">Loading.</p></div>`, '<button class="btn" data-act="dgclose">Close</button>');
    loadManage();
  }
  ACT.stacktypes = () => openManage('types');
  ACT.stackparams = () => openManage('parameters');
  ACT.stacklevels = () => openManage('levels');

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
    else html = m.which === 'types' ? typesHtml(m) : m.which === 'levels' ? levelsHtml(m) : parametersHtml(m);
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
        html += `<label class="fld"><span>Formula of other parameters, such as Width - 2 * Frame (empty: a person sets the value)</span><input class="inp" data-enter="stackformula" data-a="${esc(p.id)}" data-key="formula:${esc(p.id)}" value="${esc(p.formula ?? '')}" aria-label="Formula for ${esc(p.name)}"${off}></label>`;
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
    const shown = C.planLevel && levels.find((level) => level.id === C.planLevel.id);
    if (shown && shown.name !== C.planLevel.name) {
      C.planLevel.name = shown.name;
      headerView();
    }
  }

  /** The name of the level a record stands on, from the server's list once it has come. */
  function levelName(record) {
    const id = levelIdOf(record);
    if (!id) return 'None';
    if (C.model) {
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
    }
    return C.levelList?.levels.find((level) => level.id === id)?.name ?? id;
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
    if (S.screen === 'plan' && FLOORV.includes(S.view)) {
      headerView();
      followTheLevel();
    } else {
      go('plan:l1');
    }
    renderManage();
  };

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
    if (S.tool === 'wall') return S.pend ? '<b>Wall:</b> pick the end point. Esc to stop.' : '<b>Wall:</b> pick a start point.';
    if (S.tool === 'door') return '<b>Door:</b> click on a wall to place a door.';
    return 'Click an element to see its data. Drag it to move it: starting to work on it claims it for you.';
  }

  function propertiesPalette() {
    const piece = C.elements.get(S.sel);
    let html = '<div class="sec">';
    if (piece) {
      const { el: e, shape, failed } = piece;
      const claim = heldClaim(e.id);
      const icon = e.kind === 'wall' || e.kind === 'door' ? e.kind : 'family';
      html += typeSel(icon, esc(e.kind), `<b>${esc(e.name)}</b>`);
      const view = paramsFor(piece);
      html += `<table class="props">${prow('Position', `<span class="mono">${placementOf(e).x}, ${placementOf(e).y}</span>`)}${prow('Level', esc(levelName(e)))}${prow('Recipe', `<span class="mono">${esc(e.recipe.op)}</span>`)}`;
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
      if (view?.parameters.length) html += parametersTable(view);
      if (view) html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
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
      drawPlan();
      statusBar();
      if (C.three) C.three.update();
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
    let shapes = '';
    const ghost = (piece) => {
      if (piece.shape && levelIdOf(piece.el) === levelOf()) shapes += outlineOf(piece, 'ln fn dsh stack-ghost');
    };
    for (const pieces of C.linked.values()) for (const piece of pieces.values()) ghost(piece);
    for (const pieces of C.fixed.values()) for (const piece of pieces) ghost(packaged(piece));

    // Walls first, so a door in a wall stays on top and can be picked.
    const inOrder = [...C.elements.values()].sort(
      (a, b) => Number(b.el.kind === 'wall') - Number(a.el.kind === 'wall'),
    );
    for (const piece of inOrder) {
      const { el: e } = piece;
      const { x, y } = placementOf(e);
      const data = `class="element" data-id="${esc(e.id)}" data-x="${x}" data-y="${y}"`;
      if (!piece.shape) {
        // Nothing to draw yet, or a recipe that can't build: a mark, so it can still be found.
        shapes += `<g ${data}>${RC(x - 200, y - 200, 400, 400, 'ln fn dsh')}</g>`;
        continue;
      }
      const paint = e.kind === 'wall' ? 'wall' : e.kind === 'door' ? 'ln ap af' : 'ln sf';
      const box = around(piece);
      let group = S.sel === e.id ? RC(box.x - 100, box.y - 100, box.w + 200, box.d + 200, 'halo') : '';
      group += outlineOf(piece, `body ${paint}${S.sel === e.id ? ' sel' : ''}`);
      const claim = heldClaim(e.id);
      if (claim) {
        const theirs = claim.person !== C.me.person.id;
        group += RC(box.x - 120, box.y - 120, box.w + 240, box.d + 240, `claim ${theirs ? 'theirs' : 'mine'}`);
        if (theirs) group += TX(box.x + box.w / 2, box.y - 300, scale, `${claim.name} editing`, 'claimt', 260);
      } else if (e.kind === 'door') {
        group += TX(box.x + box.w / 2, box.y - 200, scale, e.name, 'tf', 220);
      }
      shapes += `<g ${data}>${group}</g>`;
    }
    canvas.innerHTML = `<svg class="dw pan${S.tool !== 'select' ? ' press' : ''}" viewBox="${viewBox(S.view)}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="${esc(view?.name || '')}"><rect class="paper" x="-20000" y="-20000" width="80000" height="80000"/>${shapes}</svg>`;
    for (const entry of C.arriving.splice(0)) {
      const now = Date.now();
      C.latencies.push(now - entry.author_ms);
      C.journeys.push({
        cmd: entry.cmd,
        to_server: entry.orderer_ms - entry.author_ms,
        to_page: entry.arrived_ms - entry.orderer_ms,
        to_screen: now - entry.arrived_ms,
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

  function placeWall(at) {
    if (!S.pend) {
      S.pend = [snap(at.x), snap(at.y)];
      $('#hint').innerHTML = hint();
      return;
    }
    const [ax, ay] = S.pend;
    S.pend = null;
    const dx = snap(at.x) - ax;
    const dy = snap(at.y) - ay;
    const along = Math.abs(dx) >= Math.abs(dy);
    const length = Math.abs(along ? dx : dy);
    if (length < 300) {
      say('That wall is too short. Pick two points at least 300 mm apart.');
      return;
    }
    const placement = along
      ? { x: Math.min(ax, ax + dx), y: ay - 55, z: 0 }
      : { x: ax - 55, y: Math.min(ay, ay + dy), z: 0 };
    const args = { length, thickness: 110, height: 3000, axis: along ? 'x' : 'y' };
    add('wall', 'W', placement, recipe('wall.straight', args), 'Walls');
    $('#hint').innerHTML = hint();
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
    const box = around(wall);
    const along = box.w >= box.d;
    const width = 900;
    // A door is hosted on a face of the wall, by the name the geometry gave it, not on the wall:
    // the side clicked (along x the exterior is the wall's low y side, along y its high x side),
    // as far along the wall from its start as the click, less half the door, as thick as the wall.
    const middle = along ? box.y + box.d / 2 : box.x + box.w / 2;
    const exterior = along ? at.y < middle : at.x > middle;
    const face = wall.shape.names.find((name) => name.role === (exterior ? 'exterior' : 'interior'));
    if (!face) {
      say('That wall has no side to place a door on.');
      return;
    }
    const offset = (along ? snap(at.x) - box.x : snap(at.y) - box.y) - width / 2;
    const thickness = along ? box.d : box.w;
    const args = { width, thickness, height: 2100, axis: along ? 'x' : 'y' };
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

  function add(kind, tag, placement, how, category, references = []) {
    const element = `${tag}-${crypto.randomUUID().slice(0, 8)}`;
    const level = levelOf();
    edit('element.create', { element, kind, level, placement, recipe: how, references, props: { category } });
  }

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
      const { el } = C.elements.get(id);
      const placement = placementOf(el);
      edit('element.claim', { element: id });
      // A door hosted on a face slides along its wall, and is sent once, when the drag ends.
      const hosted = el.props?.['@host.along'] !== undefined && hostOf(el) !== null;
      drag = { id, dx: at.x - placement.x, dy: at.y - placement.y, hosted };
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
        const wall = hosted.host.el;
        const offset = wall.recipe.args.axis === 'y' ? y - wall.placement.y : x - wall.placement.x;
        if (storedMm(piece.el.props['@host.along']) === offset) return;
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
        if (drag.along !== undefined) edit('hosted.move', { element: drag.id, along: `${drag.along} mm` });
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
    const meshes = elementMeshes(THREE, paints);
    scene.add(meshes.group);
    C.three = {
      update() {
        // Each where it sits: a hosted door on its wall (placementOf).
        const placed = [...C.elements.values()].map((piece) => ({
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

  boot();
})();
