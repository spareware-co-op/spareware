// Tool: Array. A row of copies by count and spacing (or out to an end), or a ring of them round a
// centre by count and angle. `modify.array`; one undo step. In a row: pick a base point and a second
// point, or type a distance and an angle and press Array. Round a centre: set the count and the
// angle, then click the centre.

import { register } from '../../core/registry.js';
import { say } from '../../core/state.js';
import { hint } from '../../core/plan.js';
import { APPLY, V, choose, choose_, chosen, copyKey, degreesOf, describe, displacement, field, hintOf, leave, lengthOf, mm, modifyRibbon, panel, pointOf, send, serverAngle } from './common.js';

describe('array', 'arrayed', 'an array');

/** How many in all, as typed, or null. */
function counted() {
  const count = Number(V.count);
  if (!Number.isInteger(count) || count < 2) {
    say('Type how many there are in all, the original counted: a whole number of 2 or more.');
    return null;
  }
  return count;
}

function row(by) {
  const count = counted();
  if (count === null) return;
  const vector = [mm(by[0]), mm(by[1])];
  send('modify.array', { count, copy: copyKey(), ...(V.linear === 'end' ? { span: vector } : { spacing: vector }) }, leave);
}

function ring(centre) {
  const count = counted();
  if (count === null) return;
  const degrees = degreesOf(V.step);
  if (degrees === null || degrees === 0) {
    say('Type an angle, such as 60.');
    return;
  }
  const typed = serverAngle(degrees);
  send(
    'modify.array',
    { count, copy: copyKey(), about: [mm(centre[0]), mm(centre[1])], ...(V.radial === 'total' ? { sweep: typed } : { angle: typed }) },
    leave,
  );
}

function place(at, event) {
  if (choose(event)) return;
  const point = pointOf(at);
  if (V.mode === 'radial') {
    ring(point);
    return;
  }
  if (!S.pend) {
    S.pend = [point];
    $('#hint').innerHTML = hint();
    return;
  }
  const [from] = S.pend;
  const by = [point[0] - from[0], point[1] - from[1]];
  if (by[0] === 0 && by[1] === 0) {
    say('Pick a second point away from the first.');
    return;
  }
  S.pend = null;
  $('#hint').innerHTML = hint();
  row(by);
}

APPLY.array = () => {
  if (V.mode === 'radial') {
    say('Click the centre of the array.');
    return;
  }
  const distance = lengthOf(V.distance);
  const angle = degreesOf(V.angle);
  if (distance === null || angle === null) {
    say('Type a distance, such as 1200 mm, and an angle, such as 30.');
    return;
  }
  row(displacement(distance, angle));
};

hintOf('array', () =>
  !chosen()
    ? '<b>Array:</b> click the element to copy.'
    : V.mode === 'radial'
      ? '<b>Array:</b> click the centre the copies go round.'
      : S.pend
        ? `<b>Array:</b> pick the ${V.linear === 'end' ? 'last copy’s' : 'next copy’s'} point, or type a distance and an angle.`
        : '<b>Array:</b> pick a base point, or type a distance and an angle.',
);

register.tools({ id: 'array', discipline: 'architecture', order: 180, place, commands: ['modify.array'] });
register.ribbons({ id: 'array', order: 1140, ribbon: (config) => modifyRibbon(config, 'array') });
register.palettes({
  id: 'array',
  order: 240,
  palette: () => {
    if (S.tool !== 'array') return undefined;
    const radial = V.mode === 'radial';
    const rows =
      choose_('mode', 'Array', [
        ['linear', 'In a row'],
        ['radial', 'Round a centre'],
      ]) +
      field('count', 'Count', 'How many in all, the original counted') +
      (radial
        ? choose_('radial', 'The angle is', [
            ['between', 'Between copies'],
            ['total', 'Of the whole sweep'],
          ]) + field('step', 'Angle', 'Angle, degrees counter-clockwise')
        : choose_('linear', 'Distance is', [
            ['spacing', 'Between copies'],
            ['end', 'To the last copy'],
          ]) + field('distance', 'Distance', 'Distance') + field('angle', 'Angle', 'Direction, degrees counter-clockwise from the right'));
    return panel('array', rows, !radial);
  },
});
