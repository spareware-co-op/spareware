// The tools, in the order the core asks them (note A96-tool-architecture.md): one line per tool module.
// A new tool adds its file under tools/<discipline>/ and its line here, at the place its order needs.

import './project/levels.js';
import './project/warnings.js';
import './project/views.js';
import './project/graphics.js';
import './project/templates.js';
import './project/schedules.js';
import './project/crop.js';
import './architecture/wall.js';
import './architecture/floor.js';
import './architecture/roof.js';
import './architecture/room.js';
import './workshop/elements.js';
import './architecture/grid.js';
import './architecture/dimension.js';
import './architecture/curtain.js';
import './architecture/door.js';
import './architecture/family.js';
import './architecture/component.js';
import './project/families.js';
import './architecture/opening.js';
import './architecture/stair.js';
