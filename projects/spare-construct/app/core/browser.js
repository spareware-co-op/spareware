// The project browser.

import { late } from './late.js';
import { C, api, modelUrl, post, say } from './state.js';
import { refreshProject } from './hub.js';
import { loadLinks } from './shapes.js';
import { redraw } from './dialogs.js';

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
    `<div class="sub">Floor plans</div>${['l1', 'l2'].map((v) => leaf('view', v, VIEWS[v].name, S.screen === 'plan' && S.view === v && !C.openView, 'plan')).join('')}<div class="sub">3D views</div>${leaf('go', '3d', 'Default 3D', S.screen === '3d', 'cube')}${late.modelViews(leaf)}`,
  );
  if (C.model) tree += group('Schedules', late.modelSchedules(leaf));
  if (C.data) tree += group('Project files', projectFiles(leaf));
  if (C.model) tree += group('Warnings', `<div data-warnings>${late.warningsList()}</div>`);
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

export { drawBrowser };
