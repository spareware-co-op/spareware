// The 3D view's model: what three.js draws for the elements, and nothing else. connect.js owns the
// page, the camera and the lights; this module turns elements into meshes, so that putting the
// renderer of note A04 in three.js's place changes this file and connect.js's one call to it.
//
// One batched mesh per paint, each drawn in one call where the browser has WebGL's multi-draw: an
// element is in its kind's paint (wall, door, joinery and so on), and a shape made of parts (a
// layered wall's layers, its display's `parts`) has each part in its material's paint. Elements
// whose shapes have the same body share one geometry and differ only by their placement, so every
// door of one type is one shape drawn many times, as the core's instances are (CON-A8-F4-P5, piece
// A20).

/**
 * Makes the view's model.
 *
 * @param {object} THREE the three.js module.
 * @param {Record<string, object>} paints a material per element kind, with `other` for the rest.
 * @param {(material: string) => object | null} [partPaint] the paint of a part made of `material`,
 *   or null to draw it in its element's kind's paint.
 * @returns {{group: object, update: (pieces: Iterable<object>) => void}} the group to add to the
 *   scene, and `update`, which draws every piece that has a shape and drops what it drew before.
 */
export function elementMeshes(THREE, paints, partPaint = () => null) {
  const group = new THREE.Group();
  const place = new THREE.Matrix4();

  /**
   * A shape's finest display, or one part of it, as triangles three.js can light: unshared corners,
   * so faces stay flat.
   */
  function faces(mesh, part) {
    const { positions } = mesh;
    const indices = part
      ? mesh.indices.slice(part.triangles[0] * 3, (part.triangles[0] + part.triangles[1]) * 3)
      : mesh.indices;
    // Model z is three.js's up.
    const points = new Float32Array(positions.length);
    for (let i = 0; i < positions.length; i += 3) {
      points[i] = positions[i];
      points[i + 1] = positions[i + 2];
      points[i + 2] = positions[i + 1];
    }
    const shape = new THREE.BufferGeometry();
    shape.setAttribute('position', new THREE.BufferAttribute(points, 3));
    shape.setIndex(indices);
    const flat = shape.toNonIndexed();
    flat.computeVertexNormals();
    shape.dispose();
    return flat;
  }

  function update(pieces) {
    for (const batch of group.children) batch.dispose();
    group.clear();
    // paint -> shape -> { mesh, part, placed: the pieces built to it }
    const paintsUsed = new Map();
    for (const piece of pieces) {
      if (!piece.shape) continue;
      const display = piece.shape.displays[0];
      const parts = display.parts?.length ? display.parts : [null];
      parts.forEach((part, n) => {
        const paint = (part && partPaint(part.material)) || paints[piece.el.kind] || paints.other;
        if (!paintsUsed.has(paint)) paintsUsed.set(paint, new Map());
        const shapes = paintsUsed.get(paint);
        // One body may be drawn in parts that differ (the stand-in's one box of a layered wall), so
        // a part is told apart by what it is made of and where it lies too.
        const key = part ? `${piece.shape.body}#${n}#${part.material}#${part.outline.join(';')}` : piece.shape.body;
        if (!shapes.has(key)) shapes.set(key, { mesh: display.mesh, part, placed: [] });
        shapes.get(key).placed.push(piece);
      });
    }
    for (const [paint, shapes] of paintsUsed) {
      const made = [];
      let instances = 0;
      let vertices = 0;
      for (const { mesh, part, placed } of shapes.values()) {
        const shape = faces(mesh, part);
        made.push([shape, placed]);
        instances += placed.length;
        vertices += shape.getAttribute('position').count;
      }
      // The shapes have no index once their corners are unshared, so no index space is kept.
      const batch = new THREE.BatchedMesh(instances, vertices, 0, paint);
      for (const [shape, placed] of made) {
        const id = batch.addGeometry(shape);
        shape.dispose();
        for (const piece of placed) {
          const { x, y, z } = piece.el.placement;
          batch.setMatrixAt(batch.addInstance(id), place.makeTranslation(x, z, y));
        }
      }
      group.add(batch);
    }
  }

  return { group, update };
}
