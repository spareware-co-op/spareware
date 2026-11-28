// The drawing surface: the plan viewport, its redraw, picking and dragging, and the properties palette.

import { late } from './late.js';
import { entries, firstAnswer } from './registry.js';
import {
  C, NOT_YET, SEND_EVERY_MS, SNAP_MM, levelIdOf, levelOf, named, say, spent,
} from './state.js';
import { packaged } from './open.js';
import {
  LAYERED, alongOn, around, followTheLevel, hostOf, outlineOf, partsOf, placementOf, storedMm,
} from './shapes.js';
import { edit } from './channel.js';
import { parametersTable, paramsFor, typeCell } from './parameters.js';

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
  // The tool in use says what to do next (register.hints): the first that answers.
  const told = firstAnswer('hints', (entry) => entry.hint());
  if (told !== undefined) return told;
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
  // A tool's part (register.palettes): the first that answers; one marked `always` is asked even
  // after an earlier answer, as the page always asked it.
  let part;
  for (const entry of entries('palettes')) {
    if (part !== undefined && !entry.always) continue;
    const answer = entry.palette(piece);
    if (part === undefined) part = answer;
  }
  let chosen;
  if (part !== undefined) {
    html += part;
  } else if (piece) {
    const { el: e, shape, failed } = piece;
    const claim = heldClaim(e.id);
    const icon = ['wall', 'door', 'window'].includes(e.kind) ? e.kind : 'family';
    html += typeSel(icon, esc(e.kind), `<b>${esc(e.name)}</b>`);
    const view = paramsFor(piece);
    html += `<table class="props">${whereRow(e)}${prow('Level', esc(late.levelName(e)))}${prow('Recipe', `<span class="mono">${esc(e.recipe.op)}</span>`)}`;
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
    if (e.recipe.op === LAYERED) html += late.wallJoinsHtml(piece);
    // Rows of the element's own kind (register.properties): every answer.
    for (const entry of entries('properties')) {
      const rows = entry.properties(piece);
      if (rows !== undefined) html += rows;
    }
    if (view?.parameters.length) html += parametersTable(view);
    if (view) html += '<div class="pad"><button class="btn sm" data-act="stacktypes">Types…</button> <button class="btn sm" data-act="stackparams">Project parameters…</button></div>';
  } else if ((chosen = firstAnswer('selections', (entry) => entry.selection(S.sel))) !== undefined) {
    // A selection that is not an element (register.selections): the first that answers.
    html += chosen;
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

// The drawing surface draws at most once a frame, however many edits, entries and timers ask in
// it: drawPlan() asks for the plan, queueDraw() for the plan, the status bar and the 3D view.
let drawQueued = false; // what the next frame draws: false, 'plan' or 'all'
function queueDraw() {
  askToDraw('all');
}

function drawPlan() {
  askToDraw('plan');
}

/** Asks the next frame to draw `what`; the frame draws once, the most that was asked of it. */
function askToDraw(what) {
  const first = !drawQueued;
  if (what === 'all' || first) drawQueued = what;
  if (!first) return;
  requestAnimationFrame(() => {
    const all = drawQueued === 'all';
    drawQueued = false;
    spent('draw', () => {
      drawPlanNow();
      if (all) {
        statusBar();
        if (C.three) C.three.update();
      }
    });
  });
}

/** Draws the plan now: only the frame askToDraw asked for calls it. */
function drawPlanNow() {
  const canvas = $('#pv-canvas');
  if (!canvas || S.screen !== 'plan') return;
  if (!C.model && !C.readOnly) {
    canvas.innerHTML = `<p class="pad faint stack-later">${hint()}</p>`;
    return;
  }
  const view = VIEWS[S.view];
  const scale = AS[S.scale] || 1;
  for (const layer of entries('layers')) if (layer.before) layer.before();
  let shapes = '';
  // Under the elements (register.layers): the server's picture of the open view, then grids and planes.
  for (const layer of entries('layers')) if (layer.under) shapes += layer.under(scale);
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
    // An element of a tool's own kind is drawn by its tool (register.elements); one marked
    // `unbuilt` (a room) even before it has a shape.
    const unbuilt = entries('elements').find((entry) => entry.unbuilt && entry.inPlan && entry.owns(e));
    if (unbuilt) {
      shapes += unbuilt.inPlan(piece, data, scale);
      continue;
    }
    if (!piece.shape) {
      // Nothing to draw yet, or a recipe that can't build: a mark, so it can still be found.
      shapes += `<g ${data}>${RC(x - 200, y - 200, 400, 400, 'ln fn dsh')}</g>`;
      continue;
    }
    const owner = entries('elements').find((entry) => !entry.unbuilt && entry.inPlan && entry.owns(e));
    if (owner) {
      shapes += owner.inPlan(piece, data, scale);
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
  // Over the elements: dimensions, then what a tool in use sketches.
  for (const layer of entries('layers')) if (layer.over) shapes += layer.over(scale);
  const underDrawing = C.drawn?.svg && C.drawn.view === C.openView ? ' drawn' : '';
  canvas.innerHTML = `${entries('layers').map((layer) => (layer.note ? layer.note() : '')).join('')}<svg class="dw pan${underDrawing}${S.tool !== 'select' ? ' press' : ''}" viewBox="${viewBox(S.view)}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="${esc(view?.name || '')}"><rect class="paper" x="-20000" y="-20000" width="80000" height="80000"/>${shapes}</svg>`;
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

/** Spare Construct's own operations, by what they make. */
const recipe = (op, args) => ({ owner: 'construct', schema: 1, op, op_version: 1, args });

/** An element dragged to where its record now says: a family that stands is moved by its own command. */
function sendMove(piece) {
  const { id, placement } = piece.el;
  if (late.isStanding(piece.el)) edit('family.move', { element: id, x: `${placement.x} mm`, y: `${placement.y} mm` });
  else edit('element.move', { element: id, placement });
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
      else placeWith(at, event);
      return;
    }
    const hit = pressed(event).closest('[data-id]');
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
    if (entries('elements').some((entry) => entry.pickOnly && entry.owns(C.elements.get(id)?.el))) return; // picked, not dragged (register.elements, pickOnly): a room's point moves by room.move, a stair by stair.direction
    const { el } = C.elements.get(id);
    const placement = placementOf(el);
    edit('element.claim', { element: id });
    // A door hosted on a face slides along its wall, and is sent once, when the drag ends.
    const hosted = el.props?.['@host.along'] !== undefined && hostOf(el) !== null;
    // A floor opening and a shaft are moved by their outline in the palette, never dragged.
    drag = entries('elements').some((entry) => entry.drags && entry.owns(el) && !entry.drags(el, hosted)) ? null : { id, dx: at.x - placement.x, dy: at.y - placement.y, hosted };
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
      // The shape of what is hosted in a wall at an angle depends on where it is along it, so a
      // piece moved ahead of the log is not compared with a checkpoint from before the move.
      piece.el = { ...piece.el, props };
      piece.ahead = true;
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
      sendMove(piece);
    }
  });

  const finish = () => {
    if (!drag) return;
    const piece = C.elements.get(drag.id);
    if (piece && drag.hosted) {
      if (drag.along !== undefined) {
        const move = entries('elements').find((entry) => entry.move && entry.owns(piece.el))?.move ?? 'hosted.move';
        edit(move, { element: drag.id, along: `${drag.along} mm` });
      }
    } else if (piece) {
      sendMove(piece);
    }
    drag = null;
  };
  window.addEventListener('pointerup', finish);
  window.addEventListener('pointercancel', finish);
}

/** What a press picks: the element under its own point, as the drawing is when it is pressed. */
const pressed = (event) => document.elementFromPoint(event.clientX, event.clientY) || event.target;

/** How the plan stacks what it draws (register.drawOrders, the first answer): floors under walls,
 *  walls under the rest, so a door in a wall stays on top. */
const drawOrder = (el) => {
  const rank = firstAnswer('drawOrders', (entry) => entry.drawOrder(el));
  return rank !== undefined ? rank : el.kind === 'wall' ? 1 : 2;
};

/** A click in the plan while a tool is in use: the tool places what it makes (register.tools). */
function placeWith(at, event) {
  const tool = entries('tools').find((entry) => entry.id === S.tool && entry.place);
  if (tool) tool.place(at, event);
  else say(NOT_YET);
}

export {
  claimedByOther, drawPlan, heldClaim, hint, plan, propertiesPalette, queueDraw, recipe, snap,
  toModel,
};
