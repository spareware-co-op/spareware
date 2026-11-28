// Tool: Move. Pick a base point and a target, or type a distance and an angle: `modify.move` moves
// the chosen element by that much, and what it hosts goes with it. One undo step.

import { register } from '../../core/registry.js';
import { say } from '../../core/state.js';
import { hint } from '../../core/plan.js';
import { APPLY, V, choose, chosen, degreesOf, describe, displacement, field, hintOf, leave, lengthOf, mm, modifyRibbon, panel, pointOf, send } from './common.js';

describe('move', 'moved', 'a move');

const move = (by) => send('modify.move', { by: [mm(by[0]), mm(by[1])] }, leave);

function place(at, event) {
  if (choose(event)) return;
  const point = pointOf(at);
  if (!S.pend) {
    S.pend = [point];
    $('#hint').innerHTML = hint();
    return;
  }
  const [from] = S.pend;
  const by = [point[0] - from[0], point[1] - from[1]];
  if (by[0] === 0 && by[1] === 0) {
    say('Pick a target other than the base point.');
    return;
  }
  S.pend = null;
  $('#hint').innerHTML = hint();
  move(by);
}

APPLY.move = () => {
  const distance = lengthOf(V.distance);
  const angle = degreesOf(V.angle);
  if (distance === null || angle === null) {
    say('Type a distance, such as 1200 mm, and an angle, such as 30.');
    return;
  }
  move(displacement(distance, angle));
};

hintOf('move', () =>
  !chosen()
    ? '<b>Move:</b> click the element to move.'
    : S.pend
      ? '<b>Move:</b> pick the target, or type a distance and an angle. Esc to stop.'
      : '<b>Move:</b> pick the base point, or type a distance and an angle.',
);

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'move', discipline: 'architecture', order: 140, place, commands: ['modify.move'] });
register.ribbons({ id: 'move', order: 1100, ribbon: (config) => modifyRibbon(config, 'move') });
register.palettes({
  id: 'move',
  order: 200,
  palette: () =>
    S.tool === 'move'
      ? panel('move', field('distance', 'Distance', 'Distance to move') + field('angle', 'Angle', 'Direction, degrees counter-clockwise from the right'))
      : undefined,
});
