// The connection and the ordered log: the live channel, edits up the socket, entries back from the log.

import { late } from './late.js';
import { entries } from './registry.js';
import { C, api, modelUrl, named, say, spent } from './state.js';
import { setStatus } from './frame.js';
import {
  applying, buildHostAgain, compareCheckpoint, cutsIn, fillAgain, hostIdOf, loadPiece,
} from './shapes.js';
import { UNDOING, loadSteps } from './undo.js';
import { refreshSide } from './parameters.js';
import { done, refused } from './problems.js';
import { loadManage } from './dialogs.js';
import { drawPlan, queueDraw } from './plan.js';

// --- The live channel ---

/**
 * Asks for what the log holds after `through`, the point regeneration had reached when a read was
 * made. The socket may not be open yet, and `send` drops what it cannot send, so the point is
 * kept for the hello that opens it: otherwise a page that read a lagging view would stay on it.
 */
function watchSince(model, through) {
  C.owedFrom = Math.min(C.owedFrom ?? through, through);
  send({ t: 'watch', document: model, since: through });
}

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
    send({ t: 'hello', client: C.client, documents: [...C.watching], since: Math.min(C.head, C.owedFrom ?? C.head) });
    C.owedFrom = undefined;
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
  if (edit && (late.WS.elements.has(edit.args.element) || late.isWorkshop(edit.args))) {
    await late.workshopTurnBack(edit, reason);
    return;
  }
  const piece = edit && C.elements.get(edit.args.element);
  if (piece?.hash && C.model) {
    piece.el = await api(`${modelUrl(C.model.id)}/record/${piece.hash}`);
    piece.ahead = false;
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
    if (entry.target === C.model?.id && late.WS.elements.has(change.piece)) {
      late.workshopEntry(entry, change, drawn);
      continue;
    }
    if (late.datumChange(entry, change)) continue; // a grid or a plane this page holds
    const piece = into.get(change.piece);
    if (piece && piece.seq >= entry.seq) continue;
    const stillMine = C.pending.some((edit) => edit.args.element === change.piece);
    // A host is built with what is hosted in it: while what it hosts changes here, it is not
    // compared with the server's, and once that is in hand it is built again.
    const cuts = cutsIn(entry.target, into);
    const placing = ['hosted.place', 'family.place', 'family.move', 'opening.place', 'opening.draw', 'opening.move', 'curtain.embed'].includes(entry.cmd);
    const hosts = new Set([piece && hostIdOf(piece.el), placing && entry.args.host]);
    if (cuts) {
      // A host is built with what is hosted in it: until a checkpoint after this entry, it is not compared.
      for (const id of hosts) if (id && into.get(id)) Object.assign(into.get(id), { stale: true, touched: entry.seq });
    }
    const again = () => Promise.all([...hosts].map((id) => buildHostAgain(entry.target, into, id)));
    if (!change.after) {
      into.delete(change.piece);
      if (cuts) again().then(queueDraw).finally(applying(entry.target, entry.seq));
    } else if (piece && entry.cmd === 'element.move') {
      piece.hash = change.after;
      piece.seq = entry.seq;
      if (!(mine && stillMine)) piece.el = { ...piece.el, placement: entry.args.placement };
      // A door hosted by its position sits apart from its host's frame, so a move of either
      // moves it in that frame.
      if (cuts) {
        hosts.add(change.piece);
        again().then(queueDraw).finally(applying(entry.target, entry.seq));
      }
    } else {
      if (piece) piece.stale = true;
      loadPiece(entry.target, change.piece, change.after, entry.seq, into, drawn)
        .then(() => (cuts ? again() : undefined))
        .then(() => {
          queueDraw();
          if (change.piece === S.sel) refreshSide();
        })
        .finally(applying(entry.target, entry.seq));
    }
  }
  late.roomsAgain(entry.target, into, entry);
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
  const families = /^family\./.test(entry.cmd) || UNDOING.has(entry.cmd);
  if (reads || families || /^element\.(type|create|delete)$|^wall\.draw$|^roof\.(create|delete|retype)$|^view\.graphics\./.test(entry.cmd)) {
    if (entry.target === C.model?.id) loadManage();
  }
  // The document's families, as the server lists them, may have changed: what is held, placed or made.
  if (families && entry.target === C.model?.id) late.reloadLibrary();
  if (entry.target === C.model?.id) {
    for (const change of entry.claims) {
      if (change.claim) C.claims.set(change.piece, change.claim);
      else C.claims.delete(change.piece);
    }
    for (const hook of entries('logs')) hook.entry(entry); // what each tool keeps beside the elements (register.logs)
  }
  queueDraw();
}

/** The commands that can change where walls meet: a wall made, moved, retyped, changed or deleted. */
const MEETS_WALLS = /^(element\.(create|move|set|type|delete)|wall\.draw)$/;

/** The commands that change a type, a parameter or a level: what a recipe that reads records reads. */
const READ_BY_RECIPES = /^(type|parameter|level|wall\.type|floor\.type|roof\.type|curtain\.type|opening|shaft)\.|^(floor|roof|curtain)\.defaults$/;

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
  'family.load': 'loaded a family',
  'family.reload': 'reloaded a family',
  'family.swap': 'swapped a family for another type',
  'family.generate': 'made a door or window type',
  'family.remove': 'removed a family',
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

export { WORDS, connect, edit, letGo, send, watchSince };
