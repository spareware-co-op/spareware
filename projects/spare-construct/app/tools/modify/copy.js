// Tool: Copy. As Move, but the chosen element stays and a copy (with what it hosts) lands at the
// target. "Multiple" keeps the tool going from the same base point for more copies. One undo step each.

import { register } from '../../core/registry.js';
import { say } from '../../core/state.js';
import { hint } from '../../core/plan.js';
import { APPLY, V, check, choose, chosen, copyKey, degreesOf, describe, displacement, field, hintOf, leave, lengthOf, mm, modifyRibbon, panel, pointOf, send } from './common.js';

describe('copy', 'copied', 'a copy');

function copy(by, base) {
  send('modify.copy', { by: [mm(by[0]), mm(by[1])], copy: copyKey() }, () => {
    if (V.multiple && base) {
      S.pend = [base];
      $('#hint').innerHTML = hint();
    } else leave();
  });
}

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
  // With Multiple the base point stays for the next copy once this one is made; otherwise it is let go.
  if (!V.multiple) S.pend = null;
  $('#hint').innerHTML = hint();
  copy(by, from);
}

APPLY.copy = () => {
  const distance = lengthOf(V.distance);
  const angle = degreesOf(V.angle);
  if (distance === null || angle === null) {
    say('Type a distance, such as 1200 mm, and an angle, such as 30.');
    return;
  }
  copy(displacement(distance, angle), null);
};

hintOf('copy', () =>
  !chosen()
    ? '<b>Copy:</b> click the element to copy.'
    : S.pend
      ? `<b>Copy:</b> pick the target${V.multiple ? ' of each copy; Esc when you have enough' : ''}, or type a distance and an angle.`
      : '<b>Copy:</b> pick the base point, or type a distance and an angle.',
);

register.tools({ id: 'copy', discipline: 'architecture', order: 150, place, commands: ['modify.copy'] });
register.ribbons({ id: 'copy', order: 1110, ribbon: (config) => modifyRibbon(config, 'copy') });
register.palettes({
  id: 'copy',
  order: 210,
  palette: () =>
    S.tool === 'copy'
      ? panel(
          'copy',
          check('multiple', 'Multiple') + field('distance', 'Distance', 'Distance to copy') + field('angle', 'Angle', 'Direction, degrees counter-clockwise from the right'),
        )
      : undefined,
});
