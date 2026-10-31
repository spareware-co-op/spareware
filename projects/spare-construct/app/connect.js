// Connects the Spare Construct demo page to its server. The server sends the demo's own file
// unchanged, so its ribbon, docks, view tabs and status bar are the real ones; this script runs
// after it and puts live data where the sample was: the plan and 3D views, the project browser,
// the properties palette and the status bar. The page keeps only what is in view; whatever it
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
    'detailset',
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
    log: [],
    three: null,
    geometry: null, // the shape builder, or null if this browser couldn't load it
    edition: null, // what it is
    viewing: false, // this browser can't build the open model, so it draws what's stored
    checked: 0, // bodies compared with the server's
    mismatches: [], // and any that differed
  };
  window.spare = C; // the browser test reads this

  const named = (person) => person[0].toUpperCase() + person.slice(1);
  const levelOf = () => (S.view === 'l2' ? 2 : 1);
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
    for (const discipline of DISCS) discipline[4] = discipline[0] === 'architecture' ? 1 : 0;
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
      const name = S.screen === 'plan' ? VIEWS[S.view]?.name : viewName(S.screen);
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
      demoSelect(id);
    };

    const demoView = ACT.view;
    ACT.view = (button) => {
      if (FLOORV.includes(button.dataset.a)) demoView(button);
      else say(NOT_YET);
    };
    ACT.account = account;

    R.plan = plan;
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
    S.sel = null;
  }

  async function openModel(team, model) {
    forgetTheOpenModel();
    C.model = model;
    C.team = team;
    C.viewing = !canBuild(model);
    C.readOnly = C.viewing;
    C.watching.add(model.id);
    send({ t: 'watch', document: model.id, since: C.head });
    await loadPieces();
    await loadClaims();
    await loadLinks();
  }

  async function openPackage(team, folder, id) {
    forgetTheOpenModel();
    C.model = null;
    C.readOnly = true;
    C.viewing = false;
    const file = await api(fileUrl(team, folder, id));
    showPackage(file);
    C.file = file;
    say(`A fixed version: ${file.elements.length} elements.`);
  }

  /** A package carries finished shapes, so it is drawn as it came and never rebuilt. */
  const packaged = (piece) => ({ el: piece.element, ...fromStored(piece), seq: 0 });

  function showPackage(file) {
    for (const piece of file.elements) {
      if (piece.element.level === levelOf()) C.elements.set(piece.element.id, packaged(piece));
    }
  }

  // --- Shapes ---

  async function loadGeometry() {
    try {
      const shapes = await import('/shapes/construct_shapes.js');
      await shapes.default();
      C.edition = JSON.parse(shapes.edition());
      C.geometry = shapes;
    } catch (error) {
      console.warn(`This browser can’t build shapes: ${error.message}`);
    }
  }

  function canBuild(model) {
    const wanted = model.edition;
    const have = C.edition;
    return (
      C.geometry !== null &&
      wanted.engine === have.engine &&
      wanted.version === have.version &&
      wanted.name_scheme === have.name_scheme
    );
  }

  const built = new Map(); // recipe text -> what it builds to, which never changes

  function buildShape(recipe) {
    const text = JSON.stringify(recipe);
    if (!built.has(text)) built.set(text, JSON.parse(C.geometry.build(text)));
    return built.get(text);
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

  function shapeFor(model, record) {
    const row = C.models.get(model);
    return row && canBuild(row) ? buildShape(record.recipe) : storedShape(model, record.id);
  }

  /** The box around a piece in the model. */
  function around(piece) {
    const { x, y } = piece.el.placement;
    const { min, max } = piece.shape.bounds;
    return { x: x + min[0], y: y + min[1], w: max[0] - min[0], d: max[1] - min[1] };
  }

  function outlineOf(piece, paint) {
    const { x, y } = piece.el.placement;
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
    const area = `level=${C.level}&x0=${x}&y0=${y}&x1=${x + w}&y1=${y + h}`;
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

  async function loadPiece(model, id, hash, seq, into) {
    const record = await api(`${modelUrl(model)}/record/${hash}`);
    const answer = await shapeFor(model, record);
    if (record.level === levelOf()) into.set(id, { el: record, ...answer, hash, seq });
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
    logEntry(entry);
    if (entry.failed) {
      if (mine) turnBack(entry.counter, entry.failed);
      return;
    }
    if (!mine && fresh) C.arriving.push(entry);
    const into = entry.target === C.model?.id ? C.elements : C.linked.get(entry.target);
    if (!into) return;
    for (const change of entry.changes) {
      const piece = into.get(change.piece);
      if (piece && piece.seq >= entry.seq) continue;
      const stillMine = C.pending.some((edit) => edit.args.element === change.piece);
      if (!change.after) {
        into.delete(change.piece);
      } else if (piece && entry.cmd === 'element.move') {
        piece.hash = change.after;
        piece.seq = entry.seq;
        if (!(mine && stillMine)) piece.el = { ...piece.el, placement: entry.args.placement };
      } else {
        if (piece) piece.stale = true;
        loadPiece(entry.target, change.piece, change.after, entry.seq, into).then(queueDraw);
      }
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
  };

  function logEntry(entry) {
    if (!WORDS[entry.cmd]) return;
    const what = entry.failed ? `turned back: ${entry.failed}` : `${WORDS[entry.cmd]} ${entry.args.element}`;
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
  }

  function letGo(id) {
    if (id && !C.readOnly && C.claims.get(id)?.person === C.me.person.id) {
      edit('element.release', { element: id });
    }
  }

  ACT.stackdelete = () => {
    if (S.sel && !C.readOnly && C.elements.has(S.sel)) {
      edit('element.delete', { element: S.sel });
      S.sel = null;
      refresh();
    }
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
      html += `<table class="props">${prow('Position', `<span class="mono">${e.placement.x}, ${e.placement.y}</span>`)}${prow('Level', e.level)}${prow('Recipe', `<span class="mono">${esc(e.recipe.op)}</span>`)}`;
      if (shape) {
        const [min, max] = [shape.bounds.min, shape.bounds.max];
        html += prow('Size', `${max[0] - min[0]} × ${max[1] - min[1]} × ${max[2] - min[2]} mm`);
        html += prow('Body', `<span class="mono">${esc(shape.body.slice(0, 12))}</span>`);
      } else {
        html += prow('Shape', esc(failed?.message || 'Not built yet'));
      }
      for (const host of e.references) html += prow('Hosted on', esc(host.element));
      for (const [key, value] of Object.entries(e.props)) html += prow(esc(key), esc(value));
      html += prow('Claimed by', claim ? esc(claim.name) : 'Nobody');
      html += '</table>';
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
      if (piece.shape && piece.el.level === levelOf()) shapes += outlineOf(piece, 'ln fn dsh stack-ghost');
    };
    for (const pieces of C.linked.values()) for (const piece of pieces.values()) ghost(piece);
    for (const pieces of C.fixed.values()) for (const piece of pieces) ghost(packaged(piece));

    // Walls first, so a door in a wall stays on top and can be picked.
    const inOrder = [...C.elements.values()].sort(
      (a, b) => Number(b.el.kind === 'wall') - Number(a.el.kind === 'wall'),
    );
    for (const piece of inOrder) {
      const { el: e } = piece;
      const { x, y } = e.placement;
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
      C.latencies.push(Date.now() - entry.author_ms);
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
    const placement = along
      ? { x: snap(at.x) - 450, y: box.y + box.d / 2 - 150, z: 0 }
      : { x: box.x + box.w / 2 - 150, y: snap(at.y) - 450, z: 0 };
    const args = { width: 900, thickness: 300, height: 2100, axis: along ? 'x' : 'y' };
    // A door is hosted on a face of the wall, by the name the geometry gave it, not on the wall.
    const face = wall.shape.names.find((name) => name.role === 'exterior');
    const host = {
      doc: null,
      element: wall.el.id,
      name: face.name,
      kind: 'face',
      want: 'carrier',
      signature: null,
    };
    add('door', 'D', placement, recipe('door.single', args), 'Doors', [host]);
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
      const { placement } = C.elements.get(id).el;
      edit('element.claim', { element: id });
      drag = { id, dx: at.x - placement.x, dy: at.y - placement.y };
    });

    window.addEventListener('pointermove', (event) => {
      if (!drag) return;
      const piece = C.elements.get(drag.id);
      if (!piece || !$('#pv-canvas svg')) return;
      const at = toModel(event);
      const x = snap(at.x - drag.dx);
      const y = snap(at.y - drag.dy);
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
      if (piece) edit('element.move', { element: drag.id, placement: piece.el.placement });
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
    const group = new THREE.Group();
    scene.add(group);
    C.three = {
      update() {
        for (const child of group.children) child.geometry.dispose();
        group.clear();
        for (const piece of C.elements.values()) {
          if (!piece.shape) continue;
          const { positions, indices } = piece.shape.displays[0].mesh;
          const { x, y, z } = piece.el.placement;
          const points = new Float32Array(positions.length);
          for (let i = 0; i < positions.length; i += 3) {
            points[i] = positions[i] + x;
            points[i + 1] = positions[i + 2] + z;
            points[i + 2] = positions[i + 1] + y;
          }
          const shape = new THREE.BufferGeometry();
          shape.setAttribute('position', new THREE.BufferAttribute(points, 3));
          shape.setIndex(indices);
          const faces = shape.toNonIndexed();
          faces.computeVertexNormals();
          shape.dispose();
          group.add(new THREE.Mesh(faces, paints[piece.el.kind] || paints.other));
        }
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
