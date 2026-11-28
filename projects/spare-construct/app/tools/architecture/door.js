// Tool: the door tool.

import { late } from '../../core/late.js';
import { register } from '../../core/registry.js';
import { say } from '../../core/state.js';
import { edit } from '../../core/channel.js';
import { recipe } from '../../core/plan.js';
import { onWall } from './wall.js';

function placeDoor(at) {
  const wall = late.wallAt(at);
  if (!wall) {
    say('Click on a wall to place a door.');
    return;
  }
  const width = 900;
  // A door is hosted on a face of the wall, by the name the geometry gave it, not on the wall:
  // the side clicked, as far along the wall from its start as the click, less half the door, as
  // thick as the wall.
  const spot = onWall(wall, at, width);
  const face = spot && wall.shape.names.find((name) => name.role === (spot.exterior ? 'exterior' : 'interior'));
  if (!face) {
    say('That wall has no side to place a door on.');
    return;
  }
  const { offset, thickness } = spot;
  const args = { width, thickness, height: 2100, axis: spot.axis };
  edit('hosted.place', {
    element: `D-${crypto.randomUUID().slice(0, 8)}`,
    kind: 'door',
    host: wall.el.id,
    face: face.name,
    along: `${offset} mm`,
    up: '0 mm',
    recipe: recipe('door.single', args),
  });
}

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({ id: 'door', discipline: 'architecture', order: 80, place: (at) => placeDoor(at) });
register.hints({ id: 'door', order: 80, hint: () => (S.tool === 'door' ? '<b>Door:</b> click on a wall to place a door.' : undefined) });
