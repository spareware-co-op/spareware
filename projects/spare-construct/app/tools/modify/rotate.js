// Tool: Rotate. Pick the centre, then a start ray and an end ray (the turn is from one to the other,
// any angle), or pick the centre and type an angle. With Copy a turned copy is made and the element
// stays. `modify.rotate`; one undo step.

import { register } from '../../core/registry.js';
import { say } from '../../core/state.js';
import { hint } from '../../core/plan.js';
import { APPLY, V, check, choose, chosen, copyKey, degreesOf, describe, field, hintOf, leave, mm, modifyRibbon, panel, pointOf, send, serverAngle } from './common.js';

describe('rotate', 'turned', 'a turn');

const turned = (about, how) =>
  send('modify.rotate', { about: [mm(about[0]), mm(about[1])], ...how, ...(V.copy ? { copy: copyKey() } : {}) }, leave);

function place(at, event) {
  if (choose(event)) return;
  const point = pointOf(at);
  if (!S.pend) {
    S.pend = [point];
    $('#hint').innerHTML = hint();
    return;
  }
  if (S.pend.length === 1) {
    const [centre] = S.pend;
    if (point[0] === centre[0] && point[1] === centre[1]) {
      say('Pick a start ray away from the centre.');
      return;
    }
    S.pend = [centre, point];
    $('#hint').innerHTML = hint();
    return;
  }
  const [centre, start] = S.pend;
  if (point[0] === centre[0] && point[1] === centre[1]) {
    say('Pick an end ray away from the centre.');
    return;
  }
  S.pend = null;
  $('#hint').innerHTML = hint();
  const direction = (to) => [mm(to[0] - centre[0]), mm(to[1] - centre[1])];
  turned(centre, { from: direction(start), to: direction(point) });
}

APPLY.rotate = () => {
  const degrees = degreesOf(V.rotation);
  if (degrees === null) {
    say('Type an angle, such as 30.');
    return;
  }
  if (!S.pend?.length) {
    say('Pick the centre of the turn first.');
    return;
  }
  turned(S.pend[0], { angle: serverAngle(degrees) });
};

hintOf('rotate', () =>
  !chosen()
    ? '<b>Rotate:</b> click the element to turn.'
    : !S.pend
      ? '<b>Rotate:</b> pick the centre of the turn.'
      : S.pend.length === 1
        ? '<b>Rotate:</b> pick the start ray, or type an angle and press Rotate. Esc to stop.'
        : '<b>Rotate:</b> pick the end ray. Esc to stop.',
);

register.tools({ id: 'rotate', discipline: 'architecture', order: 160, place, commands: ['modify.rotate'] });
register.ribbons({ id: 'rotate', order: 1120, ribbon: (config) => modifyRibbon(config, 'rotate') });
register.palettes({
  id: 'rotate',
  order: 220,
  palette: () =>
    S.tool === 'rotate'
      ? panel('rotate', check('copy', 'Copy') + field('rotation', 'Angle', 'Angle to turn, degrees counter-clockwise'))
      : undefined,
});
