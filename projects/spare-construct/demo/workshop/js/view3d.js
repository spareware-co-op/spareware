/* Spare Construct · Workshop: the 3D view, drawn with three.js 0.170.0 (MIT), the same version the Spare Construct app
   loads. It draws what a builder answered (build.js, or the kernel's builder) and never works anything out itself:
   a prism is its outline lifted by its height, a cylinder its circle, each placed by the body's map. */

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { then } from './build.js';

const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#999999';

function geometryOf(b) {
  if (b.kind === 'prism') {
    const shape = new THREE.Shape(b.outline.map((p) => new THREE.Vector2(p[0], p[1])));
    const g = new THREE.ExtrudeGeometry(shape, { depth: b.height, bevelEnabled: false });
    g.translate(0, 0, b.z);
    return g;
  }
  const g = new THREE.CylinderGeometry(b.radius, b.radius, b.height, 64, 1, false);
  g.rotateX(Math.PI / 2);
  g.translate(b.center[0], b.center[1], b.z + b.height / 2);
  return g;
}
const matrixOf = (m) => new THREE.Matrix4().set(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11], 0, 0, 0, 1);

export function createView(host, { onPick } = {}) {
  const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  host.appendChild(renderer.domElement);
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(css('--panel'));
  const camera = new THREE.PerspectiveCamera(35, 1, 1, 100000);
  camera.up.set(0, 0, 1);
  camera.position.set(260, -320, 220);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = false;
  controls.addEventListener('change', () => draw());
  scene.add(new THREE.HemisphereLight(0xffffff, 0xd8d4cc, 2.2));
  const sun = new THREE.DirectionalLight(0xffffff, 1.6);
  sun.position.set(300, -200, 500);
  scene.add(sun);
  const grid = new THREE.GridHelper(400, 40, css('--rule'), css('--hairline'));
  grid.rotateX(Math.PI / 2);
  scene.add(grid);
  const group = new THREE.Group();
  scene.add(group);
  let last = null, picks = [];

  function size() {
    const w = Math.max(1, host.clientWidth), h = Math.max(1, host.clientHeight);
    renderer.setSize(w, h, false);
    renderer.domElement.style.width = w + 'px';
    renderer.domElement.style.height = h + 'px';
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    draw();
  }
  function draw() { renderer.render(scene, camera); }
  new ResizeObserver(size).observe(host);

  /* items: [{ body, at (an extra map, for an assembly), tone, key }] */
  function show(items, { fit = false } = {}) {
    for (const o of [...group.children]) { group.remove(o); o.geometry && o.geometry.dispose(); }
    picks = [];
    const tones = { body: css('--sink'), sel: css('--app'), a: css('--sink'), b: css('--app-tint'), c: css('--panel-alt'), ghost: css('--hairline') };
    const edge = new THREE.LineBasicMaterial({ color: css('--ink') });
    for (const it of items) {
      const g = geometryOf(it.body);
      const m = matrixOf(it.at ? then(it.body.m, it.at) : it.body.m);
      const mat = new THREE.MeshStandardMaterial({ color: tones[it.tone] || tones.body, roughness: 0.75, metalness: 0.05,
        transparent: it.tone === 'ghost', opacity: it.tone === 'ghost' ? 0.35 : 1, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 });
      const mesh = new THREE.Mesh(g, mat);
      mesh.matrixAutoUpdate = false;
      mesh.matrix.copy(m);
      mesh.userData = { key: it.key };
      group.add(mesh);
      picks.push(mesh);
      const lines = new THREE.LineSegments(new THREE.EdgesGeometry(g, 25), edge);
      lines.matrixAutoUpdate = false;
      lines.matrix.copy(m);
      group.add(lines);
    }
    group.updateMatrixWorld(true);
    if (fit || !last) look('iso');
    last = items;
    draw();
  }

  function bounds() {
    const box = new THREE.Box3();
    for (const o of picks) box.expandByObject(o);
    if (box.isEmpty()) box.set(new THREE.Vector3(-50, -50, 0), new THREE.Vector3(50, 50, 50));
    return box;
  }
  /* iso, front (looking along +y), top, right (looking along -x) */
  function look(side) {
    const box = bounds(), c = box.getCenter(new THREE.Vector3()), r = Math.max(30, box.getSize(new THREE.Vector3()).length() * 0.62);
    const dir = { iso: [0.62, -0.78, 0.5], front: [0, -1, 0.0001], top: [0, -0.0001, 1], right: [1, 0, 0.0001] }[side] || [0.62, -0.78, 0.5];
    const d = new THREE.Vector3(...dir).normalize().multiplyScalar(r / Math.tan((camera.fov * Math.PI) / 360));
    camera.position.copy(c).add(d);
    controls.target.copy(c);
    controls.update();
    draw();
  }

  const ray = new THREE.Raycaster();
  let down = null;
  renderer.domElement.addEventListener('pointerdown', (e) => { down = [e.clientX, e.clientY]; });
  renderer.domElement.addEventListener('pointerup', (e) => {
    if (!down || Math.hypot(e.clientX - down[0], e.clientY - down[1]) > 4) return;
    const r = renderer.domElement.getBoundingClientRect();
    ray.setFromCamera(new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1), camera);
    const hit = ray.intersectObjects(picks, false)[0];
    if (onPick) onPick(hit ? hit.object.userData.key : null);
  });

  size();
  return { show, look, draw, canvas: renderer.domElement };
}
