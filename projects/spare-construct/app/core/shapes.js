// Shapes: what the page builds with the geometry compiled for the browser, and its checks against the server's checkpoints.

import { late } from './late.js';
import { C, api, fileUrl, levelIdOf, levelOf, modelUrl, say } from './state.js';
import { showPackage } from './open.js';
import { letGo, send, watchSince } from './channel.js';
import { hasShape, refreshSide } from './parameters.js';
import { drawPlan, queueDraw } from './plan.js';

// --- Shapes ---

// Two packages build shapes, chosen by the engine a document's edition names: the geometry
// kernel's for a document built on it, the stand-in's otherwise. The stand-in's loads with the
// page; the kernel's, far larger, the first time a document names it. Each loads once.
const KERNEL = 'construct.kernel';
const STAND_IN_PACKAGE = '/shapes/construct_shapes.js';
const KERNEL_PACKAGE = '/shapes/kernel/construct_shapes_kernel.js';

function loadGeometry(edition) {
  const path = edition?.engine === KERNEL ? KERNEL_PACKAGE : STAND_IN_PACKAGE;
  if (!C.builders.has(path)) C.builders.set(path, loadPackage(path));
  return C.builders.get(path);
}

async function loadPackage(path) {
  try {
    const shapes = await import(path);
    await shapes.default();
    // built: recipe text -> what it builds to, which never changes; hosting: the same for a host
    // and what is hosted in it; seats: the same for where an element sits on a wall at an angle
    return { shapes, edition: JSON.parse(shapes.edition()), built: new Map(), hosting: new Map(), seats: new Map() };
  } catch (error) {
    console.warn(`This browser can’t build shapes: ${error.message}`);
    return null;
  }
}

/** A value as text with every object's keys in order, so two editions compare field by field. */
const canonical = (value) =>
  JSON.stringify(value, (_, v) =>
    v && typeof v === 'object' && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
      : v,
  );

/**
 * The package that builds `model`'s edition, or null: then this browser shows what's stored. Two
 * editions are the same only when every field is, the engine's report included (FORMAT.md, "A
 * document").
 */
async function builderFor(model) {
  const builder = await loadGeometry(model.edition);
  return builder && canonical(builder.edition) === canonical(model.edition) ? builder : null;
}

function buildShape(builder, recipe) {
  const text = JSON.stringify(recipe);
  if (!builder.built.has(text)) builder.built.set(text, JSON.parse(builder.shapes.build(text)));
  return builder.built.get(text);
}

/** What the server stored, as `buildShape` gives it. */
function fromStored({ built: row, display }) {
  const { shape, failed } = row.result;
  return shape ? { shape: { ...shape, displays: display ? [display] : [] } } : { failed };
}

/** The server's copy. Nothing yet means it hasn't regenerated this element: a checkpoint will say. */
async function storedShape(model, id) {
  try {
    return fromStored(await api(`${modelUrl(model)}/shape/${id}?lod=0`));
  } catch (error) {
    if (error.status === 404) return {};
    throw error;
  }
}

/**
 * What `record` builds to here. A wall with elements hosted in it on this page is built with them,
 * each opening cut, as the server's regenerator builds it, when the package can (the kernel's
 * `build_hosting`); the stand-in's builds a host as the wall alone, as its `build` does. A recipe
 * that reads records is built from `fill`, the recipe the server filled for it (`fillOf`).
 */
function buildOn(builder, record, into, fill) {
  const recipe = fill?.recipe ?? record.recipe;
  const guests = typeof builder.shapes.build_hosting === 'function' && into ? guestsOf(record, into, fill, builder) : [];
  if (!guests.length) return buildShape(builder, recipe);
  // Each guest by its place in the host's frame, so a host that moves builds to the same text.
  const host = JSON.stringify({ recipe, placement: { x: 0, y: 0, z: 0 } });
  const hosted = JSON.stringify(guests);
  const text = `${host}\n${hosted}`;
  if (!builder.hosting.has(text)) builder.hosting.set(text, JSON.parse(builder.shapes.build_hosting(host, hosted)));
  return builder.hosting.get(text);
}

async function shapeFor(model, record, into, fill) {
  const row = C.models.get(model);
  const builder = row && (await builderFor(row));
  if (!builder) return storedShape(model, record.id);
  if (fill?.failed) return { failed: fill.failed };
  return buildOn(builder, record, into, fill);
}

// A recipe that reads records (a layered wall's layers from its wall type, a top tied to a level, a
// parameter's value) is filled by the server, the way its regenerator fills it, and this page builds
// what comes back: it never works out a type's layers or a level's height itself. The answer says
// how far the log had got, so a checkpoint from before then is not compared with it; an entry that
// may change what such a recipe reads has it filled and built again (`fillAgain`).

/** Whether a recipe reads records: an argument that is an object, such as `{"wall":"type"}`. */
// A placed family reads the document's record of its family: the server hands back the recipe with
// the family's text in it, which is how a family no library holds is built here.
const readsRecords = (recipe) =>
  (recipe?.op === 'family.instance' && recipe?.owner === 'construct') ||
  Object.values(recipe?.args || {}).some((arg) => arg !== null && typeof arg === 'object' && !Array.isArray(arg));

/** The server's filling of `record`'s recipe in `model`: `{head, recipe}` or `{head, failed}`; null when it reads nothing. */
async function fillOf(model, record) {
  if (!readsRecords(record.recipe)) return null;
  try {
    return await api(`${modelUrl(model)}/filled/${record.id}`);
  } catch (error) {
    return { head: 0, failed: { code: 'NotFilled', message: error.message } };
  }
}

/** Notes where the server seats an embedded curtain wall: its filled answer says. */
function seatNote(id, fill) {
  if (fill?.placement) late.SEATED.set(id, fill.placement);
  else late.SEATED.delete(id);
}

/** Every element of `into` whose recipe reads records, filled and built again. */
function fillAgain(model, into) {
  for (const [id, piece] of into) {
    if (readsRecords(piece.el.recipe)) refill(model, into, id, piece);
  }
}

async function refill(model, into, id, piece) {
  piece.stale = true;
  piece.asked = (piece.asked || 0) + 1;
  const asked = piece.asked;
  const fill = await fillOf(model, piece.el);
  if (into.get(id) !== piece || piece.asked !== asked) return;
  const answer = await shapeFor(model, piece.el, into, fill);
  if (into.get(id) !== piece || piece.asked !== asked) return;
  Object.assign(piece, { shape: undefined, failed: undefined }, answer, { fill, stale: false, settled: asked });
  seatNote(id, fill);
  if ((late.isFamily(piece.el) || hostIdOf(piece.el) !== null) && late.seatOfPiece(piece)) {
    // Built as it is stored: it is not compared until it is built again as its wall seats it.
    Object.assign(piece, { stale: true, seatedKey: undefined });
    late.seatFamilies();
  }
  queueDraw();
  if (id === S.sel) refreshSide();
}

/** A stored length, `length:q0:<steps>`, in millimetres. */
const storedMm = (text) => Number(String(text).split(':').pop()) / 6400;

/** The face reference a hosted door names its wall by, and that wall, if it is on this page. */
function hostOf(el) {
  const face = (el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
  const host = face && C.elements.get(face.element);
  return host ? { face, host } : null;
}

/**
 * The ID of the element `el` is hosted on, as construct-regen's `hosting::read` reads its record:
 * a face reference with both offsets, or an older door with a face reference and neither.
 */
function hostIdOf(el) {
  const faces = (el.references || []).filter((ref) => ref.kind === 'face' && !ref.doc);
  const along = el.props?.['@host.along'];
  const up = el.props?.['@host.up'];
  if (along === undefined && up === undefined) {
    const own = el.recipe?.owner === 'construct';
    const opening = own && (el.recipe?.op === 'door.single' || el.recipe?.op === 'opening.floor');
    return opening && faces.length ? faces[0].element : null;
  }
  return along !== undefined && up !== undefined && faces.length === 1 ? faces[0].element : null;
}

/**
 * What is hosted in `host` among `into`'s elements, as the kernel's package takes them: each by
 * its ID, its recipe and its place in the host's frame (in whole millimetres), as the server's
 * regenerator hands them to its builder.
 */
function guestsOf(host, into, fill, builder = C.builder) {
  const drawn = { el: host, shape: into.get(host.id)?.shape, fill: fill ?? into.get(host.id)?.fill };
  const guests = [];
  for (const [id, piece] of into) {
    if (id === host.id || hostIdOf(piece.el) !== host.id) continue;
    // An embedded curtain wall is handed to its host as the opening it cuts, as the server hands it.
    if (late.isCurtain(piece.el) && piece.fill?.cut) {
      guests.push({ key: id, recipe: piece.fill.cut.recipe, placement: piece.fill.cut.placement });
      continue;
    }
    const face = (piece.el.references || []).find((ref) => ref.kind === 'face' && !ref.doc);
    const at = placedOn(piece.el, face, drawn, builder);
    const placement = { x: at.x - host.placement.x, y: at.y - host.placement.y, z: at.z - host.placement.z };
    // An element is handed to the builder as its wall seats it, as the server hands it: a placed
    // family turned to stand in its wall, anything in a wall at an angle with its seat.
    const seat = late.seatOfPiece(piece, drawn, builder);
    const own = piece.fill?.recipe ?? piece.el.recipe; // a family copy's filled recipe holds its family's text
    guests.push({ key: id, recipe: seat ? seat.recipe : own, placement });
  }
  // A shaft is hosted on nothing: the server says which pass through a floor, as it hands its builder.
  for (const shaft of drawn.fill?.cut_by?.shafts ?? []) guests.push({ key: shaft.key, recipe: shaft.recipe, placement: shaft.placement });
  return guests;
}

/**
 * Where an element sits. A door hosted on a face of its wall sits where its offsets put it on
 * the wall, as the server reads it (construct-regen's `hosting`): along the wall from its start,
 * up from its base, and flush with its side; so it moves with its wall, whose record alone
 * changes. Anything else, or a door whose wall is not here, sits at its own placement.
 */
function placementOf(el) {
  // A curtain wall embedded in a wall stands where the server seats it.
  if (late.SEATED.has(el.id) && late.isCurtain(el)) return late.SEATED.get(el.id);
  const hosted = hostOf(el);
  return hosted ? placedOn(el, hosted.face, hosted.host) : el.placement;
}

/** Where `el` sits hosted on `face` of `host` (`{el, shape, fill}`): `placementOf`'s rule. */
function placedOn(el, face, host, builder = C.builder) {
  const along = el.props?.['@host.along'];
  const up = el.props?.['@host.up'];
  if (along === undefined || up === undefined) return el.placement;
  const role = host.shape?.names?.find((name) => name.name === face.name)?.role ?? face.signature;
  // A wall at an angle seats what is hosted in it as the package works it out (`seatAtAngle`).
  const angled = seatAtAngle(builder, el, role, host, along, up);
  if (angled) return angled.at;
  if (el.recipe.op === late.FAMILY_OP) return late.onFamily(role, host, along, up, late.faceFamily(el))?.at ?? el.placement;
  if (host.el.recipe.op === LAYERED) return onLayered(el, role, host, along, up);
  const thickness = host.el.recipe.args.thickness;
  const own = late.isWallOpening(el) ? thickness : el.recipe.args.thickness;
  if (!Number.isFinite(thickness) || !Number.isFinite(own)) return el.placement;
  const alongY = host.el.recipe.args.axis === 'y';
  // Along x the exterior is the wall's side at y = 0; along y, its side at x = its thickness.
  const across = (role === 'exterior') !== alongY ? 0 : thickness - own;
  const [a, u] = [storedMm(along), storedMm(up)];
  const at = host.el.placement;
  return alongY
    ? { x: at.x + across, y: at.y + a, z: at.z + u }
    : { x: at.x + a, y: at.y + across, z: at.z + u };
}

// A layered wall (`wall.layered`): drawn as a line between two points, its layers read from its
// wall type (construct-model's FORMAT.md, "Walls"). Lengths in its recipe are grid steps.
const LAYERED = 'wall.layered';
const STEPS_PER_MM = 6400;
const steps = (text) => Number(String(text).split(':').pop());

/** A number of steps to the nearest whole millimetre, a half away from zero, as construct-regen's `nearest_mm`. */
function nearestMm(value) {
  const floor = Math.floor(value / STEPS_PER_MM);
  const twice = (value - floor * STEPS_PER_MM) * 2;
  return twice > STEPS_PER_MM || (twice === STEPS_PER_MM && floor >= 0) ? floor + 1 : floor;
}

/**
 * A layered wall's line and outer faces, from the recipe the server filled for it: its start in
 * steps, the axis it runs along (0 for x, 1 for y, null at an angle), which way, and its two
 * finish faces' offsets to the right of the line, in steps. Null until it is filled.
 */
function lineOf(host) {
  const args = host.fill?.recipe?.args;
  if (!args || !Array.isArray(args.layers) || !args.layers.length || !args.start || !args.end) return null;
  const start = args.start.map(steps);
  const end = args.end.map(steps);
  const d = [end[0] - start[0], end[1] - start[1]];
  const axis = d[1] === 0 ? 0 : d[0] === 0 ? 1 : null;
  const unit = [Math.sign(d[0]), Math.sign(d[1])];
  const exterior = steps(args.layers[0].exterior);
  const interior = steps(args.layers.at(-1).interior);
  return { start, end, d, axis, unit, exterior, interior };
}

/**
 * Where `el` sits on side `role` of the layered wall `host` (`{el, shape, fill}`) when that wall
 * runs at an angle, and the recipe its builder is handed with its seat: asked of the package
 * (`seat`), which works it out as construct-regen's `hosting::seat` does for the server, in the
 * wall's own frame with every point rounded once, so this page builds what the server builds.
 * `{at, recipe}` with `at` in the model, or null: the wall runs along an axis (`onLayered` and
 * `onFamily` seat that), it is not filled yet, or the package does not answer.
 */
function seatAtAngle(builder, el, role, host, along, up) {
  if (!builder || typeof builder.shapes.seat !== 'function' || host.el.recipe.op !== LAYERED || !role) return null;
  const line = lineOf(host);
  if (!line || line.axis !== null) return null;
  const wall = JSON.stringify(host.fill.recipe);
  const text = JSON.stringify([wall, el.recipe, el.id, role, along, up]);
  if (!builder.seats.has(text)) {
    builder.seats.set(text, JSON.parse(builder.shapes.seat(wall, JSON.stringify(el.recipe), el.id, role, along, up)));
  }
  const seat = builder.seats.get(text).seat;
  if (!seat) return null;
  const base = host.el.placement;
  return { at: { x: base.x + seat.placement.x, y: base.y + seat.placement.y, z: base.z + seat.placement.z }, recipe: seat.recipe };
}

/**
 * Where `el` sits on side `role` of a layered wall, as construct-regen's `hosting` puts it: along
 * the wall's line from its start the way the wall runs, flush with that side and reaching toward
 * the other, each to the nearest millimetre, and up from its base. A wall along an axis; one at an
 * angle seats through `seatAtAngle`.
 */
function onLayered(el, role, host, along, up) {
  const line = lineOf(host);
  // A plain opening goes through all of its wall, and its width is a stored length.
  const plain = late.isWallOpening(el);
  const { width, thickness } = el.recipe.args;
  if (!line || line.axis === null) return el.placement;
  const wide = plain ? steps(width) : width * STEPS_PER_MM;
  const deep = plain ? Math.abs(line.exterior - line.interior) : thickness * STEPS_PER_MM;
  if (!Number.isFinite(wide) || !Number.isFinite(deep)) return el.placement;
  const a = line.axis;
  const c = 1 - a;
  const right = [line.unit[1], -line.unit[0]][c];
  const [on, other] = role === 'interior' ? [line.interior, line.exterior] : [line.exterior, line.interior];
  const lowAlong = line.unit[a] > 0
    ? line.start[a] + steps(along)
    : line.start[a] - (steps(along) + wide);
  const face = line.start[c] + on * right;
  const back = line.start[c] + other * right;
  const lowAcross = back > face ? face : face - deep;
  const at = [0, 0];
  at[a] = nearestMm(lowAlong);
  at[c] = nearestMm(lowAcross);
  const base = host.el.placement;
  return { x: base.x + at[0], y: base.y + at[1], z: base.z + storedMm(up) };
}

/** How far along its wall a hosted door at `x`, `y` (its low corner, in the model) is from the wall's start. */
function alongOn(host, el, x, y) {
  const wall = host.el;
  if (wall.recipe.op !== LAYERED) return wall.recipe.args.axis === 'y' ? y - wall.placement.y : x - wall.placement.x;
  const line = lineOf(host);
  if (!line) return null;
  if (line.axis === null) {
    // At an angle a hosted element's origin is `along` the line from its start: the pointer's place
    // projected onto the line, to the nearest millimetre.
    const [sx, sy] = line.start.map((v) => v / STEPS_PER_MM);
    const length = Math.hypot(line.d[0], line.d[1]);
    return Math.round(((x - wall.placement.x - sx) * line.d[0] + (y - wall.placement.y - sy) * line.d[1]) / length);
  }
  const a = line.axis;
  const at = [x - wall.placement.x, y - wall.placement.y][a];
  const start = Math.round(line.start[a] / STEPS_PER_MM);
  const wide = late.isWallOpening(el) ? storedMm(el.recipe.args.width) : (el.recipe.args.width ?? 0);
  return line.unit[a] > 0 ? at - start : start - at - wide;
}

/** The box around a piece in the model. */
function around(piece) {
  const { x, y } = placementOf(piece.el);
  const { min, max } = piece.shape.bounds;
  return { x: x + min[0], y: y + min[1], w: max[0] - min[0], d: max[1] - min[1] };
}

function outlineOf(piece, paint) {
  const { x, y } = placementOf(piece.el);
  const points = piece.shape.displays[0].outline.map(([px, py]) => `${x + px},${y + py}`);
  return `<polygon class="${paint}" points="${points.join(' ')}"/>`;
}

/** Each part of a shape, filled with its material's colour where the plan cuts it. */
function partsOf(piece, parts) {
  const { x, y } = placementOf(piece.el);
  return parts
    .map((part) => {
      const points = part.outline.map(([px, py]) => `${x + px},${y + py}`).join(' ');
      const colour = late.colourOf(part.material, 'cut');
      const fill = colour ? ` style="fill:${esc(colour)}"` : '';
      return `<polygon class="part" data-material="${esc(part.material)}" data-function="${esc(part.function)}"${fill} points="${points}"/>`;
    })
    .join('');
}

// A checkpoint is compared only with what this page built from the same entries: while an entry it
// covers is still being applied here (a record on its way, a host being built again with what it
// hosts), the checkpoint waits for it. A body built before such an entry landed would otherwise be
// compared with the server's built after it: an undo that puts a window back did that to its wall.

/** Marks entry `seq` of `document` as being applied here; the function returned marks it done. */
function applying(document, seq) {
  const seqs = C.applying.get(document) ?? new Map();
  C.applying.set(document, seqs);
  seqs.set(seq, (seqs.get(seq) ?? 0) + 1);
  return () => {
    const left = seqs.get(seq) - 1;
    if (left > 0) seqs.set(seq, left);
    else seqs.delete(seq);
    compareWaiting();
  };
}

/** Whether every entry of `document` through `through` has been applied here. */
const appliedThrough = (document, through) => ![...(C.applying.get(document)?.keys() ?? [])].some((seq) => seq <= through);

/** The checkpoints that waited, compared now if they need wait no longer, in the order they came. */
function compareWaiting() {
  const waiting = C.waitingCheckpoints;
  C.waitingCheckpoints = [];
  for (const entry of waiting) compareCheckpoint(entry);
}

/** Compares what the server built with what this page holds (note A05). */
function compareCheckpoint(entry) {
  const { through, bodies } = entry.args;
  // Behind a checkpoint that waits, so the two are compared in the order they came.
  if (C.waitingCheckpoints.length || !appliedThrough(entry.target, through)) {
    C.waitingCheckpoints.push(entry);
    return;
  }
  const into = entry.target === C.model?.id ? C.elements : C.linked.get(entry.target);
  if (!into) return;
  for (const [id, body] of Object.entries(bodies)) {
    const piece = into.get(id);
    // Built from a recipe filled later in the log than this checkpoint: the next one covers it.
    if (!piece || piece.seq > through || piece.stale || (piece.fill && piece.fill.head > through) || piece.ahead || late.seatChanged(piece)) continue;
    // What is hosted in it changed after the last entry this checkpoint covers: it is built with that change, so
    // the next checkpoint compares it. (Every entry through `through` has been applied here: see `applying`.)
    if ((piece.touched ?? 0) > through) continue;
    if (piece.shape?.body === body) {
      C.checked += 1;
    } else if (C.viewing || !piece.shape) {
      // Not built here: this is the news that the server's copy is ready.
      storedShape(entry.target, id).then((answer) => {
        Object.assign(piece, { shape: undefined, failed: undefined }, answer);
        queueDraw();
      });
    } else {
      C.mismatches.push({ id, here: piece.shape.body, server: body });
      console.error(`${id} built differently here and on the server.`);
      say(`${piece.el.name} built differently here, so this shows the server’s version.`);
      storedShape(entry.target, id).then((answer) => {
        Object.assign(piece, answer);
        queueDraw();
      });
    }
  }
  queueDraw();
}

/** Fetches what is on this level. Records are named by hash, so a repeat comes from the cache. */
async function loadPieces(model = C.model?.id, into = C.elements) {
  if (!model) return;
  C.level = levelOf();
  const [x, y, w, h] = VB[S.view];
  const area = `level=${encodeURIComponent(C.level)}&x0=${x}&y0=${y}&x1=${x + w}&y1=${y + h}`;
  const seen = await api(`${modelUrl(model)}/area?${area}`);
  // Regeneration can be behind the log, so ask for what came after what this shows.
  if (seen.through < C.head) watchSince(model, seen.through);
  const ids = new Set(Object.keys(seen.pieces));
  await Promise.all(
    [...ids].map(async (id) => {
      const known = into.get(id);
      if (known && (known.hash === seen.pieces[id] || known.seq > seen.through)) return;
      await loadPiece(model, id, seen.pieces[id], seen.through, into);
    }),
  );
  for (const [id, piece] of into) {
    if (!ids.has(id) && piece.seq <= seen.through) into.delete(id);
  }
}

async function loadPiece(model, id, hash, seq, into, mine = false) {
  const record = await api(`${modelUrl(model)}/record/${hash}`);
  // A type or the parameter table has no place in the plan: the properties palette reads them.
  if (!hasShape(record)) return;
  if (late.isWorkshop(record)) {
    if (model === C.model?.id) late.keepWorkshop(id, record, hash, seq, mine);
    return;
  }
  if (late.isDatum(record)) return late.keepDatum(model, id, record, hash, seq, mine); // grids and planes: their own block
  // Its host is known only now when this page did not hold it (an undo or a redo puts it back): from
  // here the host is built with it, so a checkpoint from before this entry is not compared with that.
  const host = cutsIn(model, into) && into.get(hostIdOf(record));
  if (host) Object.assign(host, { stale: true, touched: Math.max(host.touched ?? 0, seq) });
  const fill = await fillOf(model, record);
  const answer = await shapeFor(model, record, into, fill);
  seatNote(id, fill);
  // A hosted element is built as its wall seats it once the wall is here (`seatFamilies`): a
  // placed family turned to stand in it, anything in a wall at an angle with its seat.
  const unseated = into === C.elements && (late.isFamily(record) || hostIdOf(record) !== null);
  if (levelIdOf(record) === levelOf()) into.set(id, { el: record, ...answer, hash, seq, fill, ...(unseated ? { stale: true } : {}) });
  // What it is hosted in is built again with it, as the server builds a host with its guests.
  if (cutsIn(model, into)) await buildHostAgain(model, into, hostIdOf(record));
}

/** Whether this page builds `model`'s hosts with what is hosted in them, into `into`. */
const cutsIn = (model, into) => C.cuts && model === C.model?.id && into === C.elements;

/** Builds host `id` again with what is now hosted in it on this page, and lets it be compared. */
async function buildHostAgain(model, into, id) {
  const host = id && into.get(id);
  if (!host) return;
  const asked = host.asked;
  const answer = await shapeFor(model, host.el, into, host.fill);
  // A refill (`fillAgain`) that began or ended meanwhile has the newer fill: its build stands.
  if (into.get(id) !== host || host.asked !== asked) return;
  Object.assign(host, { shape: undefined, failed: undefined }, answer);
  // A refill still in flight (`fillAgain`) keeps it out of the checkpoint compare: this build used the fill held now.
  host.stale = (host.asked || 0) !== (host.settled || 0);
}

async function loadClaims() {
  C.claims = new Map(Object.entries(await api(`${modelUrl(C.model.id)}/claims`)));
}

async function loadLinks() {
  C.links = await api(`${modelUrl(C.model.id)}/links`);
  for (const link of C.links) {
    if (link.kind === 'live') {
      const pieces = C.linked.get(link.model) || new Map();
      C.linked.set(link.model, pieces);
      C.watching.add(link.model);
      send({ t: 'watch', document: link.model, since: C.head });
      await loadPieces(link.model, pieces);
    } else {
      const file = await api(fileUrl(link.team, link.kind, link.package));
      C.fixed.set(link.id, file.elements);
    }
  }
}

/** A view changed to another level: the pieces in hand are the wrong ones. */
async function followTheLevel() {
  if (C.level === levelOf()) return;
  letGo(S.sel);
  S.sel = null;
  C.elements.clear();
  for (const pieces of C.linked.values()) pieces.clear();
  await loadPieces();
  for (const [model, pieces] of C.linked) await loadPieces(model, pieces);
  if (C.readOnly && !C.viewing && C.file) showPackage(C.file);
  drawPlan();
}

export {
  LAYERED, STEPS_PER_MM, alongOn, applying, around, buildHostAgain, buildOn, builderFor,
  compareCheckpoint, cutsIn, fillAgain, followTheLevel, fromStored, hostIdOf, hostOf, lineOf,
  loadClaims, loadGeometry, loadLinks, loadPiece, loadPieces, nearestMm, outlineOf, partsOf,
  placementOf, refill, seatAtAngle, shapeFor, steps, storedMm,
};
