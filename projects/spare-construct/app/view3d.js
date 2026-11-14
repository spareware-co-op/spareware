// The 3D view's model: what three.js draws for the elements, and nothing else. connect.js owns the
// page, the camera and the lights; this module turns elements into meshes, so that putting the
// renderer of note A04 in three.js's place changes this file and connect.js's one call to it.
//
// One batched mesh per element type (wall, door, joinery and so on), each drawn in one call
// where the browser has WebGL's multi-draw. Elements whose shapes have the same body share one
// geometry and differ only by their placement, so every door of one type is one shape drawn many
// times, as the core's instances are (CON-A8-F4-P5, piece A20).

/**
 * Makes the view's model.
 *
 * @param {object} THREE the three.js module.
 * @param {Record<string, object>} paints a material per element kind, with `other` for the rest.
 * @returns {{group: object, update: (pieces: Iterable<object>) => void}} the group to add to the
 *   scene, and `update`, which draws every piece that has a shape and drops what it drew before.
 */
export function elementMeshes(THREE, paints) {
  const group = new THREE.Group();
  const place = new THREE.Matrix4();

  /** A shape's finest display as triangles three.js can light: unshared corners, so faces stay flat. */
  function faces(mesh) {
    const { positions, indices } = mesh;
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
    // kind -> body -> the pieces built to that body
    const kinds = new Map();
    for (const piece of pieces) {
      if (!piece.shape) continue;
      const kind = piece.el.kind;
      if (!kinds.has(kind)) kinds.set(kind, new Map());
      const bodies = kinds.get(kind);
      if (!bodies.has(piece.shape.body)) bodies.set(piece.shape.body, []);
      bodies.get(piece.shape.body).push(piece);
    }
    for (const [kind, bodies] of kinds) {
      const shapes = [];
      let instances = 0;
      let vertices = 0;
      for (const placed of bodies.values()) {
        const shape = faces(placed[0].shape.displays[0].mesh);
        shapes.push([shape, placed]);
        instances += placed.length;
        vertices += shape.getAttribute('position').count;
      }
      // The shapes have no index once their corners are unshared, so no index space is kept.
      const batch = new THREE.BatchedMesh(instances, vertices, 0, paints[kind] || paints.other);
      for (const [shape, placed] of shapes) {
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
