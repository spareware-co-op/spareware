// Undo and redo: this person's own steps in the open model, from the server.

import { register } from './registry.js';
import { C, api, modelUrl, say } from './state.js';
import { edit } from './channel.js';

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
  'family.turn': 'turning a family',
  'family.set': 'a value of a family',
  'family.delete': 'deleting a family',
  'family.load': 'loading a family',
  'family.reload': 'reloading a family',
  'family.swap': 'swapping a family',
  'family.generate': 'making a door or window type',
  'family.remove': 'removing a family',
  'level.create': 'making a level',
  'level.rename': 'renaming a level',
  'level.elevation': 'moving a level',
  'level.storey': 'a change to a level',
  'level.delete': 'deleting a level',
  'level.tie': 'a tie to a level',
  'level.top': 'a tie of a top',
  'wall.draw': 'drawing a wall',
  'wall.join': 'a joint of walls',
  'wall.join.end': 'where a wall joins',
  'wall.wrap': 'the wrapping of a wall',
  'wall.type.create': 'making a wall type',
  'wall.type.layers': 'the layers of a wall type',
  'wall.type.wrap': 'the wrapping of a wall type',
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

// This person's own steps in a model, read when it opens, between the tools' (register.models).
register.models({ id: 'undo', order: 30, open: () => loadSteps() });

export { STEP_WORDS, UNDOING, loadSteps, stepWords };
