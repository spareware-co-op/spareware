// The Modify tools' common part (Move, Copy, Rotate, Mirror, Array): what each of them reads from
// the plan and sends. It registers nothing itself; each tool's file registers the tool.
//
// A Modify tool works on the element chosen in the plan (the selection, or the first click when
// none is chosen), takes its points from clicks or from typed numbers, and sends ONE command
// (`modify.move`, `modify.copy`, `modify.rotate`, `modify.mirror` or `modify.array`, construct-model's
// FORMAT.md, "Modify"): one entry in the log, so one undo, claimed like any edit. Nothing is worked
// out here that the server works out: the page turns a click into a typed length or a direction,
// and the server does the geometry, exactly, at any angle, and refuses in words what it will not
// do (a door moved off its wall, a point past the model's limit). What a host carries goes with
// it by the server's rules.
//
// The one thing the page works out is a typed distance and direction into a displacement, which
// is never exact (a direction at 30 degrees has no whole length); it is rounded to the nearest
// whole millimetre, and the server moves by exactly that.

import { C, say } from '../../core/state.js';
import { register } from '../../core/registry.js';
import { WORDS } from '../../core/channel.js';
import { STEP_WORDS } from '../../core/undo.js';
import { refreshSide } from '../../core/parameters.js';
import { canEdit, manage, newId, problem } from '../../core/problems.js';
import { hint, snap } from '../../core/plan.js';
import { typedLength } from '../architecture/grid.js';
import { LAYERED, STEPS_PER_MM, lineOf } from '../../core/shapes.js';

/** What a person typed into the tools' panels, kept while they switch tools; and where a send is shown. */
const V = {
  distance: '',
  angle: '0',
  rotation: '90',
  multiple: false,
  copy: false,
  keep: true,
  axis: 'pick',
  count: '3',
  mode: 'linear',
  linear: 'spacing',
  radial: 'between',
  step: '60',
};
/** Where a refusal of any Modify command is shown, in the tool's panel. */
const WHERE = 'modify';
/** The tools, by their `S.tool` id: how each applies what was typed (a function), set by its file. */
const APPLY = {};

const TOOLS = ['move', 'copy', 'rotate', 'mirror', 'array'];
const LABELS = { move: 'Move', copy: 'Copy', rotate: 'Rotate', mirror: 'Mirror', array: 'Array' };
const TIPS = {
  move: 'Move the selected element: pick a base point and a target, or type a distance and an angle.',
  copy: 'Copy the selected element: pick a base point and a target, or type a distance and an angle.',
  rotate: 'Turn the selected element about a centre, by any angle: pick the centre, then two rays, or type an angle.',
  mirror: 'Mirror the selected element about a line: pick a wall or grid line, or draw the axis.',
  array: 'Copy the selected element in a row, or round a centre.',
};

const isTool = () => TOOLS.includes(S.tool);

/** The element the tools work on: the selection. */
const chosen = () => (S.sel && (C.elements.has(S.sel) || C.datums?.get(C.model?.id)?.has(S.sel)) ? S.sel : null);

/** The element id a press landed on, at its own point, or null. */
function underPress(event) {
  const hit = document.elementFromPoint(event.clientX, event.clientY) || event.target;
  return hit?.closest?.('[data-id]')?.dataset.id ?? null;
}

/**
 * Takes the first click as the choice of the element when none is chosen yet (so a person can
 * start a tool first, as in Revit's other order). True when the click was that choice.
 */
function choose(event) {
  if (chosen()) return false;
  const id = underPress(event);
  if (!id || !(C.elements.has(id) || C.datums?.get(C.model?.id)?.has(id))) {
    say('Click the element to work on first.');
    return true;
  }
  select(id);
  $('#hint').innerHTML = hint();
  return true;
}

/** A click's point in the plan, on the grid the plan snaps to. */
const pointOf = (at) => [snap(at.x), snap(at.y)];

/** A length of whole millimetres as typed: `1200 mm`. */
const mm = (value) => `${Math.round(value)} mm`;

/** A length given in grid steps (1/6400 mm) as typed, exact in eight places (as the grid tool writes one). */
function stepsText(steps) {
  const whole = Math.floor(Math.abs(steps) / STEPS_PER_MM);
  const rest = Math.abs(steps) % STEPS_PER_MM;
  const fraction = rest ? `.${String(rest * 15625).padStart(8, '0').replace(/0+$/, '')}` : '';
  return `${steps < 0 ? '-' : ''}${whole}${fraction} mm`;
}

/** A typed length in millimetres, or null: `1200`, `1200 mm`, `1.2 m`. */
function lengthOf(text) {
  const found = /^\s*(-?\d+(?:\.\d+)?)\s*(mm|m)?\s*$/.exec(String(text));
  if (!found) return null;
  return Number(found[1]) * (found[2] === 'm' ? 1000 : 1);
}

/** A typed angle in degrees, or null: `30`, `30°`, `-12.5 deg`. */
function degreesOf(text) {
  const found = /^\s*(-?\d+(?:\.\d+)?)\s*(°|deg)?\s*$/.exec(String(text));
  return found ? Number(found[1]) : null;
}

/**
 * An angle typed as a person reads one on the plan, counter-clockwise from the right, as the
 * server reads it: the plan's y runs down the screen, so the server's turn from x toward y is
 * clockwise to the eye. A full turn is the same either way, and is sent as a full turn.
 */
const serverAngle = (degrees) => (Math.abs(degrees) === 360 ? '360°' : `${degrees === 0 ? 0 : -degrees}°`);

/** The displacement a typed distance and angle (counter-clockwise on the plan) make, in whole millimetres. */
function displacement(distance, degrees) {
  const radians = (degrees * Math.PI) / 180;
  return [Math.round(distance * Math.cos(radians)), Math.round(-distance * Math.sin(radians))];
}

/** Ends the tool, as Esc does: back to selecting, the element still chosen. */
function leave() {
  ACT.seltool();
}

/** What the server's refusal of the last send says, under the panel, until it is tried again. */
const refusal = () => problem(WHERE);

/** Sends a Modify command for the chosen element; what happens after it is applied is `after`. */
function send(cmd, args, after) {
  const id = chosen();
  if (!id) {
    say('Choose the element to work on first.');
    return;
  }
  manage(cmd, { elements: [id], ...args }, WHERE, after);
}

/** The key the copies of a command are named from, fresh each time as a new element's id is. */
const copyKey = () => newId('K');

/** The chosen element's name for the panel, or what is missing. */
function chosenRow() {
  const id = chosen();
  const el = id ? (C.elements.get(id)?.el ?? C.datums?.get(C.model?.id)?.get(id)?.el) : null;
  return prow('Element', el ? `${esc(el.name)} <span class="faint">${esc(el.kind)}</span>` : '<span class="faint">None yet: click one in the plan</span>');
}

const field = (name, label, aria, text = V[name]) =>
  prow(label, `<input class="inp" data-chg="stackmodify" data-modify-field="${name}" data-key="modify-${name}" value="${esc(text)}" aria-label="${esc(aria)}">`);
const check = (name, label) =>
  prow(label, `<input type="checkbox" data-chg="stackmodify" data-modify-field="${name}" data-key="modify-${name}"${V[name] ? ' checked' : ''} aria-label="${esc(label)}">`);
const choose_ = (name, label, options) =>
  prow(
    label,
    `<select class="inp" data-chg="stackmodify" data-modify-field="${name}" data-key="modify-${name}" aria-label="${esc(label)}">${options
      .map(([value, words]) => `<option value="${value}"${V[name] === value ? ' selected' : ''}>${esc(words)}</option>`)
      .join('')}</select>`,
  );

/** A tool's panel: its heading, the rows, an Apply button for what is typed, and the server's refusal. */
function panel(tool, rows, apply = true) {
  if (!canEdit()) return undefined;
  return (
    typeSel(tool, 'Modify', `<b>${LABELS[tool]}</b>`) +
    `<table class="props" data-modify-tool="${tool}">${chosenRow()}${rows}</table>` +
    (apply ? `<div class="pad"><button class="btn sm" data-act="stackmodifyapply">${LABELS[tool]}</button></div>` : '') +
    refusal()
  );
}

CHG.stackmodify = (control) => {
  const name = control.dataset.modifyField;
  if (name) V[name] = control.type === 'checkbox' ? control.checked : control.value;
  // What the choice changes in the panel (a mode of the array, the way an axis is given) is drawn.
  if (control.tagName === 'SELECT') refreshSide();
};
ACT.stackmodifyapply = () => APPLY[S.tool]?.();
document.addEventListener('keydown', (event) => {
  const field = event.target?.closest?.('input[data-modify-field]');
  if (event.key !== 'Enter' || !field || field.type === 'checkbox') return;
  V[field.dataset.modifyField] = field.value;
  APPLY[S.tool]?.();
});

/**
 * The tools' buttons: in the Modify tab's Modify panel, where the page's own sample had them,
 * and in a Modify panel on the selection's tab or the tool's own, so a person can start one from what
 * they have picked.
 */
function modifyRibbon(config, tool) {
  const label = LABELS[tool];
  const button = {
    label,
    icon: tool,
    act: 'dtool',
    a: `architecture:${tool}`,
    tip: TIPS[tool],
    pressed: S.tool === tool,
    disabled: !canEdit(),
  };
  // The sample's buttons of these names, in the Modify tab, are these tools now.
  const arch = config.tabs?.find((tab) => tab.id === 'mod')?.panels.find((p) => p.label === 'Modify');
  for (const item of arch?.items ?? []) {
    for (const own of item.stack ?? [item]) if (own.act === 'modify' && own.a === tool) Object.assign(own, button);
  }
  // A tab that is about the selection or about a tool in use has one Modify panel of the five.
  for (const tab of config.contextual ?? []) {
    if (tab.id !== 'sel' && !(tab.id === 'tool' && isTool())) continue;
    let own = tab.panels.find((p) => p.label === 'Modify');
    if (!own) {
      own = { label: 'Modify', items: [] };
      tab.panels.push(own);
    }
    if (!own.items.some((item) => item.a === button.a)) own.items.push({ ...button });
    if (tab.id === 'tool') tab.label = `${LABELS[S.tool]}`;
  }
}

/** The line a wall or a grid line gives to mirror about: two points as typed lengths, or null. */
function lineOfElement(id) {
  const piece = C.elements.get(id);
  if (piece?.el.recipe.op === LAYERED) {
    const line = lineOf(piece);
    if (!line) return null;
    const { x, y } = piece.el.placement;
    const point = (at) => [stepsText(x * STEPS_PER_MM + at[0]), stepsText(y * STEPS_PER_MM + at[1])];
    return [point(line.start), point(line.end)];
  }
  const datum = C.datums?.get(C.model?.id)?.get(id);
  if (datum?.el.recipe.op === 'grid.line') {
    const { args } = datum.el.recipe;
    return [
      [typedLength(args.x0), typedLength(args.y0)],
      [typedLength(args.x1), typedLength(args.y1)],
    ];
  }
  return null;
}

/** What each Modify tool says to a person in the hint line when it is in use; the first line registers it. */
const hintOf = (tool, words) => register.hints({ id: tool, order: 200 + TOOLS.indexOf(tool) * 10, hint: () => (S.tool === tool ? words() : undefined) });

/** Sets up what a tool needs to appear: its name for the demo's tool tab, and its words in the log and for undo. */
function describe(tool, log, step) {
  TOOLNAME[tool] = LABELS[tool];
  WORDS[`modify.${tool}`] = log;
  STEP_WORDS[`modify.${tool}`] = step;
}

export {
  APPLY, LABELS, V, check, choose, choose_, chosen, copyKey, degreesOf, describe, displacement, field, hintOf, leave,
  lengthOf, lineOfElement, mm, modifyRibbon, panel, pointOf, send, serverAngle, underPress,
};
