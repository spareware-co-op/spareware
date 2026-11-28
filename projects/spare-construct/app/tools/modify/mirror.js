// Tool: Mirror. Pick a wall or a grid line to mirror about, or draw the axis with two points; keep the
// original (a mirrored copy is made) or not (the element is mirrored in place). `modify.mirror`; one
// undo step.

import { register } from '../../core/registry.js';
import { C, say } from '../../core/state.js';
import { hint } from '../../core/plan.js';
import { V, check, choose, choose_, chosen, copyKey, describe, hintOf, leave, lineOfElement, mm, modifyRibbon, panel, pointOf, send, underPress } from './common.js';

describe('mirror', 'mirrored', 'a mirror');

const mirrored = (line) => send('modify.mirror', { line, ...(V.keep ? { copy: copyKey() } : {}) }, leave);

function place(at, event) {
  if (choose(event)) return;
  if (V.axis === 'pick') {
    const id = underPress(event);
    if (!id || !(C.elements.has(id) || C.datums?.get(C.model?.id)?.has(id))) {
      say('Click a wall or a grid line to mirror about, or choose to draw the axis.');
      return;
    }
    if (id === chosen()) {
      say('Pick a line other than the element you are mirroring.');
      return;
    }
    const line = lineOfElement(id);
    if (!line) {
      say('That has no line to mirror about. Pick a wall or a grid line, or draw the axis.');
      return;
    }
    mirrored(line);
    return;
  }
  const point = pointOf(at);
  if (!S.pend) {
    S.pend = [point];
    $('#hint').innerHTML = hint();
    return;
  }
  const [from] = S.pend;
  if (from[0] === point[0] && from[1] === point[1]) {
    say('Pick a second point away from the first.');
    return;
  }
  S.pend = null;
  $('#hint').innerHTML = hint();
  mirrored([
    [mm(from[0]), mm(from[1])],
    [mm(point[0]), mm(point[1])],
  ]);
}

hintOf('mirror', () =>
  !chosen()
    ? '<b>Mirror:</b> click the element to mirror.'
    : V.axis === 'pick'
      ? '<b>Mirror:</b> click the wall or grid line to mirror about.'
      : S.pend
        ? '<b>Mirror:</b> pick the axis’s second point. Esc to stop.'
        : '<b>Mirror:</b> pick the axis’s first point.',
);

register.tools({ id: 'mirror', discipline: 'architecture', order: 170, place, commands: ['modify.mirror'] });
register.ribbons({ id: 'mirror', order: 1130, ribbon: (config) => modifyRibbon(config, 'mirror') });
register.palettes({
  id: 'mirror',
  order: 230,
  palette: () =>
    S.tool === 'mirror'
      ? panel(
          'mirror',
          choose_('axis', 'Axis', [
            ['pick', 'Pick a wall or grid line'],
            ['draw', 'Draw it'],
          ]) + check('keep', 'Keep the original'),
          false,
        )
      : undefined,
});
