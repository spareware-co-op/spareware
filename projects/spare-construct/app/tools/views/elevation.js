// Tool: the elevation tool, a mark in the plan that makes the building's four elevations, or one.

import { register } from '../../core/registry.js';
import { C, say } from '../../core/state.js';
import { canEdit, manage, newId } from '../../core/problems.js';
import { around } from '../../core/shapes.js';
import { viewing } from './sectionview.js';

// --- The elevation tool ---
// One press in the plan makes the elevations of the building, as an architect names them: the north
// elevation shows the building's north face, seen from the north (`view.elevation.create`, which
// names the face and where its line starts, and the server works out the line, square to the face
// and looking at it). The elevations are as wide as the building and a metre more at each end, one
// metre off it, and see as deep as the building is, a metre more. The tool's palette chooses all
// four faces or one by direction. The building is what the open level holds; with nothing on it the
// press makes them round the point pressed, 12 m across.

DTOOLS.architecture.push(['elevation', 'Elevation', 'elev', 'Press in the plan to make the elevations of the building: all four faces, or the one the tool is set to.']);
TOOLNAME.elevation = 'elevation';

const FACES = [['north', 'North'], ['east', 'East'], ['south', 'South'], ['west', 'West']];
const MARGIN = 1000;
const T = { face: 'all', depth: '' };

/** The box round everything the open level holds, or the square round `at` when it holds nothing. */
function building(at) {
  let box = null;
  for (const piece of C.elements.values()) {
    if (!piece.shape) continue;
    const { x, y, w, d } = around(piece);
    box = box
      ? [Math.min(box[0], x), Math.min(box[1], y), Math.max(box[2], x + w), Math.max(box[3], y + d)]
      : [x, y, x + w, y + d];
  }
  return box ?? [at.x - 6000, at.y - 6000, at.x + 6000, at.y + 6000];
}

/** Where the elevation of `face` starts and how wide and deep it is: its line is off the face, its left end as one faces it. */
function lineFor(face, [x0, y0, x1, y1]) {
  const [lo, hi] = [[x0 - MARGIN, y0 - MARGIN], [x1 + MARGIN, y1 + MARGIN]];
  const across = hi[0] - lo[0];
  const down = hi[1] - lo[1];
  switch (face) {
    case 'south': return { from: [lo[0], lo[1]], width: across, depth: down };
    case 'north': return { from: [hi[0], hi[1]], width: across, depth: down };
    case 'west': return { from: [lo[0], hi[1]], width: down, depth: across };
    default: return { from: [hi[0], lo[1]], width: down, depth: across };
  }
}

/** A name for the elevation of `face` that is not in `taken`. */
function nameFor(face, taken) {
  const base = `${face[0].toUpperCase()}${face.slice(1)} elevation`;
  for (let n = 1; ; n += 1) {
    const name = n === 1 ? base : `${base} ${n}`;
    if (!taken.has(name)) return name;
  }
}

function place(at) {
  if (!canEdit()) {
    say('Open a model in your team’s WIP to add to it.');
    return;
  }
  const box = building({ x: Math.round(at.x), y: Math.round(at.y) });
  const faces = T.face === 'all' ? FACES.map(([face]) => face) : [T.face];
  const taken = new Set((C.views || []).map((view) => view.name));
  for (const face of faces) {
    const { from, width, depth } = lineFor(face, box);
    const name = nameFor(face, taken);
    taken.add(name);
    manage(
      'view.elevation.create',
      {
        view: newId('VE'),
        name,
        face,
        from: { x: `${from[0]} mm`, y: `${from[1]} mm` },
        width: `${width} mm`,
        depth: T.depth.trim() || `${depth} mm`,
      },
      'new-elevation',
    );
  }
}

/** The tool's own options in the properties palette while it is in use. */
function options() {
  if (S.tool !== 'elevation' || viewing()) return undefined;
  const off = canEdit() ? '' : ' disabled';
  const faces = [['all', 'All four faces'], ...FACES.map(([key, label]) => [key, `${label} only`])];
  const choices = faces.map(([key, label]) => `<option value="${key}"${T.face === key ? ' selected' : ''}>${label}</option>`).join('');
  let html = typeSel('elev', 'Elevation', '<b>New elevations</b>');
  html += `<table class="props">${prow('Faces', `<select class="inp" data-chg="stackelevfaces" data-key="elevation-new-faces" aria-label="Which faces a press makes elevations of"${off}>${choices}</select>`)}`;
  html += prow('Far clip', `<input class="inp" data-chg="stackelevdepth" data-key="elevation-new-depth" value="${esc(T.depth)}" placeholder="the depth of the building" aria-label="How far a new elevation sees"${off}>`, 'How far it sees from its line.');
  return `${html}</table>`;
}

CHG.stackelevfaces = (select) => {
  T.face = FACES.some(([key]) => key === select.value) ? select.value : 'all';
};
CHG.stackelevdepth = (input) => {
  T.depth = input.value;
};

// What this tool plugs into the core with (core/registry.js; note A96-tool-architecture.md).
register.tools({
  id: 'elevation',
  discipline: 'architecture',
  order: 160,
  place,
  commands: ['view.elevation.create', 'view.set', 'view.rename', 'view.delete', 'view.line.move', 'view.line.flip'],
  test: 'Sections and elevations, in the plan and opened',
});
register.hints({
  id: 'elevation',
  order: 160,
  hint: () => (S.tool === 'elevation' ? '<b>Elevation:</b> press in the plan. The building’s elevations are made at once. Esc to stop.' : undefined),
});
register.palettes({ id: 'elevation-tool', order: 160, palette: () => options() });
