// Signing in and choosing where to work.

import { C, api, initials } from './state.js';
import { takeOverTheFrame } from './frame.js';
import { drawBrowser } from './browser.js';
import { openModel } from './open.js';
import { loadGeometry } from './shapes.js';
import { connect } from './channel.js';

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
  Object.assign(C, { pending: [], links: [], log: [], readOnly: false, owedFrom: undefined, waitingCheckpoints: [] });
  for (const map of [C.elements, C.claims, C.linked, C.fixed, C.applying]) map.clear();
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

export { boot, refreshProject };
