// The 3D viewport, loaded only when the view opens.

import { late } from './late.js';
import { C } from './state.js';
import { placementOf } from './shapes.js';
import { propertiesPalette } from './plan.js';

// --- The 3D viewport: three.js, loaded only when this view opens ---

function threeD() {
  const screen = $('#sc-3d');
  if (screen.dataset.stack !== 'yes') {
    screen.dataset.stack = 'yes';
    screen.innerHTML = `<div class="center"><div class="canvas" id="iso-canvas"></div></div><aside class="side" id="iso-side" aria-label="Properties"></aside>`;
    startThree();
  }
  $('#hint').textContent = 'Drag to turn the view.';
  $('#iso-side').innerHTML = propertiesPalette();
  if (C.three) C.three.update();
}

async function startThree() {
  const THREE = await import('three');
  const { OrbitControls } = await import('three/addons/controls/OrbitControls.js');
  const { elementMeshes } = await import('/view3d.js');
  const box = $('#iso-canvas');
  const tokens = getComputedStyle(document.documentElement);
  const colour = (name) => new THREE.Color(tokens.getPropertyValue(name).trim());
  const scene = new THREE.Scene();
  scene.background = colour('--panel');
  scene.add(new THREE.AmbientLight(0xffffff, 1.6));
  const sun = new THREE.DirectionalLight(0xffffff, 1.6);
  sun.position.set(1, 3, 2);
  scene.add(sun);
  const camera = new THREE.PerspectiveCamera(45, box.clientWidth / box.clientHeight, 100, 500000);
  camera.position.set(-6000, 14000, 20000);
  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setSize(box.clientWidth, box.clientHeight);
  box.append(renderer.domElement);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(6000, 0, 4000);
  // Turning model z into three.js's up reverses the triangles, so both sides are drawn.
  const paint = (name) =>
    new THREE.MeshLambertMaterial({ color: colour(name), side: THREE.DoubleSide });
  const paints = { wall: paint('--ink-3'), door: paint('--app'), other: paint('--sink') };
  // A part of a shape (a layer of a wall) in its material's colour seen in projection, one paint
  // per colour; a material the server doesn't list takes its element's paint.
  const byColour = new Map();
  const partPaint = (material) => {
    const colour = late.colourOf(material, 'surface');
    if (!colour) return null;
    if (!byColour.has(colour)) {
      byColour.set(colour, new THREE.MeshLambertMaterial({ color: new THREE.Color(colour), side: THREE.DoubleSide }));
    }
    return byColour.get(colour);
  };
  const meshes = elementMeshes(THREE, paints, partPaint);
  scene.add(meshes.group);
  C.three = {
    update() {
      late.seatFamilies();
      // Each where it sits: a hosted door on its wall (placementOf).
      const placed = [...C.elements.values()].filter((piece) => !late.isRoom(piece.el)).map((piece) => ({
        ...piece,
        el: { ...piece.el, placement: placementOf(piece.el) },
      }));
      meshes.update(placed);
    },
  };
  C.three.update();
  renderer.setAnimationLoop(() => {
    controls.update();
    renderer.render(scene, camera);
  });
}

export { threeD };
