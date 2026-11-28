// Tool: rooms (the Room tool, a room in the plan, and its properties).

import { provide } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { C, api, levelIdOf, levelOf, modelUrl, say } from '../../core/state.js';
import { nearestMm, refill, storedMm } from '../../core/shapes.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS, UNDOING } from '../../core/undo.js';
import { paramsFor, refreshSide } from '../../core/parameters.js';
import { canEdit, capital, manage, newId, problem } from '../../core/problems.js';
import { heldClaim, snap } from '../../core/plan.js';
import { levelName, modelLevels } from '../project/levels.js';
import { readOnlyTable, ringPath } from './floor.js';

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

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'room', discipline: 'architecture', order: 40, place: (at) => placeRoom(at) });
register.hints({
  id: 'room',
  order: 50,
  hint: () => (S.tool === 'room' ? '<b>Room:</b> click inside a loop of walls. Choose its name, number and upper limit in the properties palette. Esc to stop.' : undefined),
});
register.palettes({ id: 'room', order: 50, always: true, palette: (piece) => roomPalette(piece) || undefined });
register.elements({ id: 'room', order: 10, owns: isRoom, inPlan: (piece, data, scale) => roomInPlan(piece, data, scale), unbuilt: true, pickOnly: true });
register.drawOrders({ id: 'room', order: 20, drawOrder: (el) => (el.kind === 'room' ? -1 : undefined) });

// What files loaded before this one use from it, bound once it has loaded.
provide({ get isRoom() { return isRoom; }, get roomsAgain() { return roomsAgain; } });
