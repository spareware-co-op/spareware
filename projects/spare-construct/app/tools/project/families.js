// Tool: the families of the document, in a dialog (Manage, Families).

import { register } from '../../core/registry.js';
import { C } from '../../core/state.js';
import { canEdit, capital, manage, newId, plural, problem } from '../../core/problems.js';
import { manageField } from '../../core/dialogs.js';
import { isFamily } from '../architecture/family.js';

// --- The families of the document, in a dialog (Manage, Families) ---
// The server lists them (`/families`): the document's own first, each with where it came from, its
// types and how many copies are placed, then the library's it does not hold yet. Load, reload,
// generate and remove are the commands `family.load`, `family.reload`, `family.generate` and
// `family.remove` through the log (undo and redo take them back), and a refusal is the server's
// words under what was asked. The page works out nothing: a spec is sent as typed.

const TEMPLATES = [
  ['single-swing-door', 'Single swing door'],
  ['fixed-window', 'Fixed window'],
  ['sliding-window', 'Sliding window'],
];
const SPEC_MORE = [
  ['frame', 'Frame width'],
  ['frame_depth', 'Frame depth'],
  ['leaf', 'Leaf thickness'],
  ['sash', 'Sash width'],
  ['sash_depth', 'Sash depth'],
  ['track_gap', 'Track gap'],
  ['glass', 'Glass thickness'],
  ['sill_height', 'Sill height'],
  ['wall', 'Wall thickness'],
];

function familiesHtml(m) {
  const off = canEdit() ? '' : ' disabled';
  const list = m.data.families;
  const held = list.filter((family) => family.in_document);
  let html = '<p class="small muted">A family is a door or a window with its types. The ones this model holds are listed first; a family of the library comes into the model when you load it or first place it.</p>';
  html += '<h3>In this model</h3>';
  if (!held.length) html += '<p class="small faint" data-families-none>This model holds no families yet.</p>';
  for (const family of held) {
    const label = esc(family.name);
    html += `<section data-family="${esc(family.id)}"><div class="stack-line"><b>${label}</b><span class="badge n">${esc(capital(family.kind))}</span>`;
    html += `<span class="small faint">${esc(family.source)}; ${family.placed === 1 ? '1 copy' : `${family.placed} copies`} placed</span>`;
    if (family.source.startsWith('library:')) {
      html += `<button class="btn sm" data-act="stackfamilyreload" data-a="${esc(family.id)}" data-key="family-reload:${esc(family.id)}" aria-label="Reload the family ${label}"${off}>Reload</button>`;
    }
    html += `<button class="btn sm" data-act="stackfamilyremove" data-a="${esc(family.id)}" data-key="family-remove:${esc(family.id)}" aria-label="Remove the family ${label}"${off}>Remove</button></div>`;
    html += `<ul class="small" data-family-types="${esc(family.id)}">${family.types.map((type) => `<li data-family-type="${esc(type.key)}">${esc(type.name)}</li>`).join('')}</ul>`;
    html += `${problem(`famdoc:${family.id}`)}</section>`;
  }
  const library = list.filter((family) => !family.in_document);
  if (library.length) {
    html += '<h3 class="mt2">In the library</h3>';
    for (const family of library) {
      const label = esc(family.name);
      html += `<section data-library-family="${esc(family.id)}"><div class="stack-line"><b>${label}</b><span class="badge n">${esc(capital(family.kind))}</span>`;
      html += `<span class="small faint">${plural(family.types.length, 'type')}</span>`;
      html += `<button class="btn sm" data-act="stackfamilyload" data-a="${esc(family.id)}" data-key="family-load:${esc(family.id)}" aria-label="Load the family ${label}"${off}>Load</button></div>`;
      html += `${problem(`famdoc:${family.id}`)}</section>`;
    }
  }
  // A type made from a spec: a new family, or one more type of a family made so before.
  const made = held.filter((family) => family.source.startsWith('generated:'));
  const templates = TEMPLATES.map(([stem, name]) => `<option value="${stem}">${esc(name)}</option>`);
  const into = ['<option value="">A new family</option>', ...made.map((family) => `<option value="${esc(family.id)}">${esc(family.name)}</option>`)];
  html += '<h3 class="mt2">Make a door or window type</h3><div class="grid2">';
  html += `<label class="fld"><span>Kind</span><select class="inp" data-draft data-key="gen-template"${off}>${templates.join('')}</select></label>`;
  html += `<label class="fld"><span>Into</span><select class="inp" data-draft data-key="gen-family"${off}>${into.join('')}</select></label>`;
  html += `<label class="fld"><span>Name of a new family</span><input class="inp" data-draft data-enter="stackfamilygenerate" data-key="gen-name"${off}></label>`;
  html += `<label class="fld"><span>Mark of the type</span><input class="inp" data-draft data-enter="stackfamilygenerate" data-key="gen-mark"${off}></label>`;
  html += `<label class="fld"><span>Width</span><input class="inp" data-draft data-enter="stackfamilygenerate" data-key="gen-width"${off}></label>`;
  html += `<label class="fld"><span>Height</span><input class="inp" data-draft data-enter="stackfamilygenerate" data-key="gen-height"${off}></label>`;
  html += '</div>';
  html += `<label class="chk"><input type="checkbox" data-draft data-key="gen-clear"${off}>The width and height are the clear opening, not the overall size</label>`;
  html += `<details data-open="gen-more"${m.open.has('gen-more') ? ' open' : ''}><summary>More of the spec</summary><div class="grid2">`;
  for (const [key, name] of SPEC_MORE) {
    html += `<label class="fld"><span>${esc(name)}</span><input class="inp" data-draft data-enter="stackfamilygenerate" data-key="gen-${key}"${off}></label>`;
  }
  html += `<label class="fld"><span>Hand of a door</span><select class="inp" data-draft data-key="gen-hand"${off}><option value="">As the template has it</option><option value="left">Left</option><option value="right">Right</option></select></label>`;
  html += '</div></details>';
  html += `<div class="stack-line mt"><button class="btn pri sm" data-act="stackfamilygenerate" data-key="gen-go"${off}>Make type</button></div>${problem('famgen')}`;
  return html;
}

ACT.stackfamilyload = (button) => manage('family.load', { family: button.dataset.a }, `famdoc:${button.dataset.a}`);
ACT.stackfamilyreload = (button) => manage('family.reload', { family: button.dataset.a }, `famdoc:${button.dataset.a}`);
ACT.stackfamilyremove = (button) => manage('family.remove', { family: button.dataset.a }, `famdoc:${button.dataset.a}`);
ACT.stackfamilygenerate = () => {
  const field = (key) => manageField(`gen-${key}`)?.value.trim() ?? '';
  const spec = { template: field('template'), mark: field('mark'), width: field('width'), height: field('height') };
  if (manageField('gen-clear')?.checked) spec.clear = true;
  for (const [key] of SPEC_MORE) if (field(key) !== '') spec[key] = field(key);
  if (field('hand') !== '') spec.hand = field('hand');
  const into = field('family');
  const args = { family: into || newId('FAM'), spec };
  if (!into) args.name = field('name');
  manage('family.generate', args, 'famgen', () => {
    const mark = manageField('gen-mark');
    if (mark) mark.value = '';
  });
};

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

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.dialogs({ id: 'families', order: 10, html: (m) => familiesHtml(m) });
