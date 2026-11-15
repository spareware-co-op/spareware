/* Spare Construct · Workshop: what the prototype edits.

   A part is its recipe: parameters, then an ordered list of features, then the end of the history. Its solids are
   built from the recipe every time, so the recipe is the truth and the shapes are derived, as for every Spare
   Construct element. The same JSON goes to the geometry kernel's builder (crate construct-workshop) and to this
   prototype's own (build.js), so the two can be compared feature by feature.

   An assembly places parts and joins them with joints that move within limits. Drawings are made from either. */

/* What waits for the geometry kernel, and the kernel pieces it waits for (projects/engines/geometry/architecture,
   chapter 30). The ribbon shows each one, unavailable, with this text, and the feature map (CON-A27) says the same. */
export const WAITS = {
  revolve: { label: 'Revolve', icon: 'ws-revolve', what: "the kernel's revolve (G255)" },
  sweep: { label: 'Sweep', icon: 'ws-sweep', what: "the kernel's sweeps (G256, G257)" },
  fillet: { label: 'Fillet', icon: 'ws-fillet', what: "the kernel's blends (G263 to G265) and persistent edge names" },
  chamfer: { label: 'Chamfer', icon: 'ws-chamfer', what: "the kernel's blends (G263) and persistent edge names" },
  shell: { label: 'Shell', icon: 'ws-shell', what: "the kernel's offsets (G275, G277)" },
  hole: { label: 'Hole', icon: 'ws-hole', what: "the kernel's booleans (G130 to G133)" },
  join: { label: 'Join', icon: 'ws-join', what: "the kernel's booleans (G130 to G133)" },
  cut: { label: 'Cut', icon: 'ws-cut', what: "the kernel's booleans (G130 to G133)" },
  intersect: { label: 'Intersect', icon: 'ws-intersect', what: "the kernel's booleans (G130 to G133)" },
  constraints: { label: 'Constraints', icon: 'ws-constraint', what: "the kernel's sketch solver (G217 to G228)" },
  onface: { label: 'Sketch on a face', icon: 'ws-onface', what: "persistent face names in the history (kernel chapter 20)" },
  outline: { label: 'Extrude any outline', icon: 'ws-extrude', what: "the kernel's extrude (G255) or its booleans (G130, G131); today it extrudes convex outlines and circles" },
  turn: { label: 'Pattern round an axis', icon: 'ws-pattern', what: "accepting a Certified result for turns other than quarter turns (the kernel's transform.pattern can)" },
  mates: { label: 'Joints on faces', icon: 'ws-joint', what: "the kernel's assembly mates (G479) and persistent face names; here a joint's axis is typed in" },
  pinslot: { label: 'Pin in slot', icon: 'ws-joint', what: "the kernel's assembly mates (G479)" },
  ball: { label: 'Ball joint', icon: 'ws-joint', what: "the kernel's assembly mates (G479)" },
  interference: { label: 'Interference', icon: 'clash', what: "the kernel's booleans (G130 to G133) and exact clash (CON-A13)" },
  hidden: { label: 'Hidden lines', icon: 'eyeoff', what: "the kernel's drawing engine (G394, G396)" },
  step: { label: 'STEP file', icon: 'down', what: "the kernel's STEP export (G429)" },
  mesh: { label: 'Mesh to print', icon: 'down', what: "the kernel's certified tessellation (G288)" },
};

/* The feature kinds the prototype builds, with what the kernel does for each. */
export const KINDS = {
  sketch: { label: 'Sketch', icon: 'ws-sketch' },
  extrude: { label: 'Extrude', icon: 'ws-extrude', kernel: 'make.prism, or make.cylinder for a circle' },
  mirror: { label: 'Mirror', icon: 'mirror', kernel: 'transform.mirror' },
  pattern: { label: 'Pattern', icon: 'ws-pattern', kernel: 'transform.pattern' },
  move: { label: 'Move', icon: 'move', kernel: 'transform.move' },
};
export const kindOf = (f) => KINDS[f.type] || WAITS[f.type] || { label: f.type, icon: 'gear' };

/* Materials, by density in kg per cubic metre. */
export const MATERIALS = {
  steel: { label: 'Mild steel', density: 7850 },
  stainless: { label: 'Stainless steel', density: 8000 },
  aluminium: { label: 'Aluminium', density: 2700 },
  brass: { label: 'Brass', density: 8500 },
  nylon: { label: 'Nylon', density: 1150 },
  oak: { label: 'Oak', density: 750 },
};

const P = (name, value, note = '') => ({ name, value, note });

/* The sample project: three parts and the assembly they make, a hinged arm on a bracket with a slide block. */
export function sample() {
  return {
    name: 'Hinged stop',
    units: 'mm',
    parts: [
      {
        id: 'bracket', name: 'Bracket', material: 'steel',
        params: [P('width', 120, 'Along x'), P('depth', 60, 'Along y'), P('thickness', 8), P('boss_radius', 12),
          P('boss_height', 30), P('boss_offset', 40, 'From the middle to each boss'), P('lug_pitch', 20)],
        features: [
          { id: 'f1', type: 'sketch', name: 'Base outline', z: 0, shape: 'rect', w: 'width', h: 'depth', cx: 0, cy: 0 },
          { id: 'f2', type: 'extrude', name: 'Base plate', sketch: 'f1', depth: 'thickness', op: 'new' },
          { id: 'f3', type: 'sketch', name: 'Boss circle', z: 'thickness', shape: 'circle', r: 'boss_radius', cx: 'boss_offset', cy: 0 },
          { id: 'f4', type: 'extrude', name: 'Boss', sketch: 'f3', depth: 'boss_height', op: 'new' },
          { id: 'f5', type: 'mirror', name: 'Second boss', of: 'f4', plane: 'x', at: 0 },
          { id: 'f6', type: 'sketch', name: 'Lug outline', z: 'thickness', shape: 'polygon', n: 6, r: 6, cx: 0, cy: -20 },
          { id: 'f7', type: 'extrude', name: 'Lug', sketch: 'f6', depth: 10, op: 'new' },
          { id: 'f8', type: 'pattern', name: 'Lug row', of: 'f7', count: 3, dx: 0, dy: 'lug_pitch', dz: 0 },
          { id: 'f9', type: 'hole', name: 'Mounting holes', note: 'Four 8.5 mm clearance holes through the base' },
          { id: 'f10', type: 'fillet', name: 'Round the plate corners', note: '5 mm on the four upright edges' },
        ],
        end: 10,
      },
      {
        id: 'arm', name: 'Arm', material: 'aluminium',
        params: [P('length', 110), P('arm_width', 20), P('arm_thickness', 8), P('eye_radius', 12)],
        features: [
          { id: 'a1', type: 'sketch', name: 'Arm outline', z: 0, shape: 'rect', w: 'length', h: 'arm_width', cx: 55, cy: 0 },
          { id: 'a2', type: 'extrude', name: 'Arm bar', sketch: 'a1', depth: 'arm_thickness', op: 'new' },
          { id: 'a3', type: 'sketch', name: 'Eye', z: 0, shape: 'circle', r: 'eye_radius', cx: 0, cy: 0 },
          { id: 'a4', type: 'extrude', name: 'Eye', sketch: 'a3', depth: 'arm_thickness', op: 'new' },
          { id: 'a5', type: 'sketch', name: 'Tip outline', z: 0, shape: 'outline', pts: [[100, -10], [118, -6], [118, 6], [100, 10]] },
          { id: 'a6', type: 'extrude', name: 'Tip', sketch: 'a5', depth: 'arm_thickness', op: 'new' },
          { id: 'a7', type: 'hole', name: 'Pivot hole', note: 'A 10 mm hole through the eye' },
        ],
        end: 7,
      },
      {
        id: 'block', name: 'Slide block', material: 'nylon',
        params: [P('block_length', 30), P('block_width', 16), P('block_height', 16)],
        features: [
          { id: 'b1', type: 'sketch', name: 'Block outline', z: 0, shape: 'rect', w: 'block_length', h: 'block_width', cx: 0, cy: 0 },
          { id: 'b2', type: 'extrude', name: 'Block', sketch: 'b1', depth: 'block_height', op: 'new' },
          { id: 'b3', type: 'chamfer', name: 'Lead-in chamfer', note: '1 mm on the top edges' },
        ],
        end: 3,
      },
    ],
    assembly: {
      name: 'Hinged stop',
      instances: [
        { id: 'i1', part: 'bracket', name: 'Bracket', at: [0, 0, 0], grounded: true },
        { id: 'i2', part: 'arm', name: 'Arm', at: [0, 0, 0] },
        { id: 'i3', part: 'block', name: 'Slide block', at: [0, 0, 0] },
      ],
      joints: [
        { id: 'j1', name: 'Arm hinge', type: 'hinge', a: 'i1', b: 'i2', origin: [40, 0, 38], axis: 'z', min: -30, max: 120, value: 35, unit: 'deg' },
        { id: 'j2', name: 'Block slide', type: 'slider', a: 'i1', b: 'i3', origin: [-30, 21, 8], axis: 'x', min: -12, max: 8, value: 0, unit: 'mm' },
      ],
    },
  };
}

/* A new feature's recipe, with sizes a person then changes. */
let seq = 0;
export function newId(part) {
  const used = new Set(part.features.map((f) => f.id));
  let id;
  do { id = 'n' + (++seq); } while (used.has(id));
  return id;
}

export function newFeature(part, type, opts = {}) {
  const id = newId(part);
  const count = part.features.filter((f) => f.type === type).length + 1;
  const name = (opts.name || (KINDS[type] || WAITS[type]).label) + ' ' + count;
  switch (type) {
    case 'sketch': {
      const shape = opts.shape || 'rect';
      const label = { rect: 'Rectangle', circle: 'Circle', polygon: 'Polygon', outline: 'Outline' }[shape];
      const nth = part.features.filter((f) => f.type === 'sketch' && f.shape === shape).length + 1;
      const base = { id, type, name: `${label} ${nth}`, z: opts.z || 0, shape, cx: 0, cy: 0 };
      if (shape === 'rect') return Object.assign(base, { w: 40, h: 20 });
      if (shape === 'circle') return Object.assign(base, { r: 10 });
      if (shape === 'polygon') return Object.assign(base, { n: 6, r: 12 });
      return Object.assign(base, { pts: [[-20, -10], [20, -10], [25, 10], [-15, 15]] });
    }
    case 'extrude': return { id, type, name, sketch: opts.of || '', depth: 10, op: 'new' };
    case 'mirror': return { id, type, name, of: opts.of || '', plane: 'x', at: 0 };
    case 'pattern': return { id, type, name, of: opts.of || '', count: 3, dx: 20, dy: 0, dz: 0 };
    case 'move': return { id, type, name, of: opts.of || '', dx: 10, dy: 0, dz: 0 };
    default: return { id, type, name, note: '' };
  }
}
