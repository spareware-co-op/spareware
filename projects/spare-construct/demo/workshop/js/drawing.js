/* Spare Construct · Workshop: a drawing sheet of a part or an assembly, as SVG, in millimetres on an A3 sheet.

   Front, top and right views in third-angle projection and a pictorial view, each drawn from the bodies a builder
   answered, with overall dimensions (the geometry kernel's bounding box when it built the part or posed the whole
   assembly), a parts list and balloons for an assembly, and a title block. Every edge is
   drawn: removing the hidden ones, exactly, waits for the kernel's drawing engine (G394, G396), and the sheet says so. */

import { apply, then } from './build.js';

const SHEET = [420, 297];
const SCALES = [[5, 1], [2, 1], [1, 1], [1, 2], [1, 5], [1, 10], [1, 20], [1, 50]];
const C30 = Math.cos(Math.PI / 6), S30 = 0.5;
const PROJ = {
  front: (p) => [p[0], -p[2]],
  top: (p) => [p[0], -p[1]],
  right: (p) => [p[1], -p[2]],
  iso: (p) => [(p[0] - p[1]) * C30, -((p[0] + p[1]) * S30 + p[2])],
};
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const n1 = (v) => (Math.round(v * 100) / 100).toString();

/* Every edge of a body in model space, as polylines. */
function edges(body, at) {
  const m = at ? then(body.m, at) : body.m;
  const P = (x, y, z) => apply(m, [x, y, z]);
  if (body.kind === 'prism') {
    const lo = body.outline.map((p) => P(p[0], p[1], body.z)), hi = body.outline.map((p) => P(p[0], p[1], body.z + body.height));
    return { loops: [lo, hi], lines: lo.map((p, i) => [p, hi[i]]) };
  }
  const N = 64, lo = [], hi = [];
  for (let i = 0; i < N; i++) {
    const a = (2 * Math.PI * i) / N, x = body.center[0] + body.radius * Math.cos(a), y = body.center[1] + body.radius * Math.sin(a);
    lo.push(P(x, y, body.z));
    hi.push(P(x, y, body.z + body.height));
  }
  return { loops: [lo, hi], lines: [], round: true };
}

function viewOf(items, side) {
  const f = PROJ[side], polys = [];
  for (const it of items) {
    const e = edges(it.body, it.at);
    for (const loop of e.loops) polys.push({ pts: loop.map(f), closed: true });
    for (const l of e.lines) polys.push({ pts: l.map(f), closed: false });
    if (e.round) {
      // the two generators on the outline: the extremes across the projected axis
      const a = e.loops[0].map(f), b = e.loops[1].map(f);
      const ax = [b[0][0] - a[0][0], b[0][1] - a[0][1]], len = Math.hypot(ax[0], ax[1]);
      if (len > 1e-9) {
        const nrm = [-ax[1] / len, ax[0] / len];
        let lo = 0, hi = 0;
        a.forEach((p, i) => {
          const d = p[0] * nrm[0] + p[1] * nrm[1];
          if (d < a[lo][0] * nrm[0] + a[lo][1] * nrm[1]) lo = i;
          if (d > a[hi][0] * nrm[0] + a[hi][1] * nrm[1]) hi = i;
        });
        polys.push({ pts: [a[lo], b[lo]] }, { pts: [a[hi], b[hi]] });
      }
    }
  }
  const box = [Infinity, Infinity, -Infinity, -Infinity];
  for (const p of polys) for (const q of p.pts) { box[0] = Math.min(box[0], q[0]); box[1] = Math.min(box[1], q[1]); box[2] = Math.max(box[2], q[0]); box[3] = Math.max(box[3], q[1]); }
  return { polys, box };
}

const path = (pts, closed, k, ox, oy) => 'M' + pts.map((p) => `${n1(ox + p[0] * k)} ${n1(oy + p[1] * k)}`).join('L') + (closed ? 'Z' : '');

function dim(x1, y1, x2, y2, text, side) {
  // a dimension between two points on one line, drawn off to one side by 8 mm, with arrows and its text
  const horiz = Math.abs(y2 - y1) < 1e-9;
  const off = side * 8;
  const a = horiz ? [x1, y1 + off] : [x1 + off, y1], b = horiz ? [x2, y2 + off] : [x2 + off, y2];
  const t = horiz ? `<text x="${n1((a[0] + b[0]) / 2)}" y="${n1(a[1] - 1.2)}" text-anchor="middle">${text}</text>`
    : `<text x="${n1(a[0] - 1.2)}" y="${n1((a[1] + b[1]) / 2)}" text-anchor="middle" transform="rotate(-90 ${n1(a[0] - 1.2)} ${n1((a[1] + b[1]) / 2)})">${text}</text>`;
  const ar = (p, q) => {
    const d = Math.hypot(q[0] - p[0], q[1] - p[1]) || 1, ux = (q[0] - p[0]) / d, uy = (q[1] - p[1]) / d;
    return `<path class="ar" d="M${n1(p[0])} ${n1(p[1])}L${n1(p[0] + ux * 3 - uy * 0.9)} ${n1(p[1] + uy * 3 + ux * 0.9)}L${n1(p[0] + ux * 3 + uy * 0.9)} ${n1(p[1] + uy * 3 - ux * 0.9)}Z"/>`;
  };
  return `<g class="dim"><path d="M${n1(x1)} ${n1(y1)}L${n1(a[0])} ${n1(a[1])}M${n1(x2)} ${n1(y2)}L${n1(b[0])} ${n1(b[1])}M${n1(a[0])} ${n1(a[1])}L${n1(b[0])} ${n1(b[1])}"/>${ar(a, b)}${ar(b, a)}${t}</g>`;
}

/* items: [{ body, at, item }] where item numbers an assembly's parts; info: { title, project, material, mass, list, dims,
   size }, size the geometry kernel's box of the whole, when it gave one: it sets the scale and the overall sizes */
export function sheet(items, info) {
  const [W, H] = SHEET, m = 10;
  const views = { front: viewOf(items, 'front'), top: viewOf(items, 'top'), right: viewOf(items, 'right'), iso: viewOf(items, 'iso') };
  const w = (v) => v.box[2] - v.box[0], h = (v) => v.box[3] - v.box[1];
  const room = [250, 200];
  // the sizes along x, y and z: the kernel's box when it gave one (info.size, [[x, y, z] least, [x, y, z] greatest],
  // its query.bounding_box of the bodies), or what the views drawn from the bodies' outlines span
  const s = info.size, ext = s ? [0, 1, 2].map((i) => s[1][i] - s[0][i]) : [w(views.front), h(views.top), h(views.front)];
  let scale = SCALES[SCALES.length - 1];
  for (const sc of SCALES) {
    const k = sc[0] / sc[1];
    if ((ext[0] + ext[1]) * k + 30 <= room[0] && (ext[1] + ext[2]) * k + 30 <= room[1]) { scale = sc; break; }
  }
  const k = scale[0] / scale[1];
  const g = 18;   // the gap between views, room for dimensions
  const fx = m + 24, fy = m + 24 + h(views.top) * k + g;   // the front view's top left
  const place = {
    front: [fx - views.front.box[0] * k, fy - views.front.box[1] * k],
    top: [fx - views.top.box[0] * k, m + 24 - views.top.box[1] * k],
    right: [fx + w(views.front) * k + g - views.right.box[0] * k, fy - views.right.box[1] * k],
  };
  const isoRoom = [W - m - 175, m + 20, 160, 120];
  const ki = Math.min(isoRoom[2] / Math.max(1, w(views.iso)), isoRoom[3] / Math.max(1, h(views.iso)));
  place.iso = [isoRoom[0] - views.iso.box[0] * ki + (isoRoom[2] - w(views.iso) * ki) / 2, isoRoom[1] - views.iso.box[1] * ki];
  let out = `<svg xmlns="http://www.w3.org/2000/svg" class="sheet" viewBox="0 0 ${W} ${H}" role="img" aria-label="Drawing of ${esc(info.title)}">`;
  out += `<rect class="paper" x="0" y="0" width="${W}" height="${H}"/><rect class="frame" x="${m}" y="${m}" width="${W - 2 * m}" height="${H - 2 * m}"/>`;
  for (const side of ['front', 'top', 'right', 'iso']) {
    const v = views[side], [ox, oy] = place[side], kk = side === 'iso' ? ki : k;
    out += `<g class="view">` + v.polys.map((p) => `<path d="${path(p.pts, p.closed, kk, ox, oy)}"/>`).join('') + '</g>';
    const label = { front: 'Front', top: 'Top', right: 'Right', iso: 'Pictorial, not to scale' }[side];
    out += `<text class="vl" x="${n1(ox + v.box[0] * kk)}" y="${n1(oy + v.box[3] * kk + (side === 'iso' ? 6 : 13))}">${label}</text>`;
  }
  if (info.dims !== false) {
    const F = views.front, T = views.top, [ox, oy] = place.front, [tx, ty] = place.top;
    out += dim(ox + F.box[0] * k, oy + F.box[3] * k, ox + F.box[2] * k, oy + F.box[3] * k, n1(ext[0]), 1);
    out += dim(ox + F.box[0] * k, oy + F.box[1] * k, ox + F.box[0] * k, oy + F.box[3] * k, n1(ext[2]), -1);
    out += dim(tx + T.box[0] * k, ty + T.box[1] * k, tx + T.box[0] * k, ty + T.box[3] * k, n1(ext[1]), -1);
    if (s) out += `<text class="note" x="${m + 4}" y="${H - m - 8}">Overall sizes from the geometry kernel's bounding box of the bodies.</text>`;
  }
  // balloons for an assembly's parts, on the pictorial view
  const seen = new Set();
  for (const it of items) {
    if (!it.item || seen.has(it.item)) continue;
    seen.add(it.item);
    const b = it.body.bbox ? edges(it.body, it.at).loops[1].map(PROJ.iso) : [];
    if (!b.length) continue;
    const c = b.reduce((s, p) => [s[0] + p[0] / b.length, s[1] + p[1] / b.length], [0, 0]);
    const p = [place.iso[0] + c[0] * ki, place.iso[1] + c[1] * ki], q = [p[0] + 14, p[1] - 14 - 6 * seen.size];
    out += `<g class="bal"><path d="M${n1(p[0])} ${n1(p[1])}L${n1(q[0] - 3.2)} ${n1(q[1] + 2)}"/><circle cx="${n1(p[0])}" cy="${n1(p[1])}" r="0.7"/><circle class="o" cx="${n1(q[0])}" cy="${n1(q[1])}" r="4"/><text x="${n1(q[0])}" y="${n1(q[1] + 1.4)}" text-anchor="middle">${it.item}</text></g>`;
  }
  // the title block, and a parts list above it
  const tb = [W - m - 180, H - m - 34, 180, 34];
  const row = (y, l, v, x = tb[0], w2 = 180) => `<text class="tl" x="${x + 2}" y="${y}">${l}</text><text class="tv" x="${x + 2}" y="${y + 5.2}">${esc(v)}</text>`;
  out += `<g class="tb"><rect x="${tb[0]}" y="${tb[1]}" width="${tb[2]}" height="${tb[3]}"/><path d="M${tb[0]} ${tb[1] + 12}H${tb[0] + 180}M${tb[0]} ${tb[1] + 23}H${tb[0] + 180}M${tb[0] + 90} ${tb[1] + 12}V${tb[1] + 34}M${tb[0] + 135} ${tb[1] + 12}V${tb[1] + 34}"/>`;
  out += row(tb[1] + 4, 'DRAWING', info.title) + row(tb[1] + 16, 'PROJECT', info.project) + row(tb[1] + 16, 'SCALE', `${scale[0]}:${scale[1]}, A3`, tb[0] + 90)
    + row(tb[1] + 16, 'PROJECTION', 'Third angle', tb[0] + 135) + row(tb[1] + 27, 'MATERIAL', info.material || 'See the parts list') + row(tb[1] + 27, 'MASS', info.mass || '', tb[0] + 90)
    + row(tb[1] + 27, 'UNITS', 'mm', tb[0] + 135) + '</g>';
  if (info.list && info.list.length) {
    const lh = 6, y0 = tb[1] - 4 - lh * (info.list.length + 1);
    out += `<g class="pl"><rect x="${tb[0]}" y="${y0}" width="180" height="${lh * (info.list.length + 1)}"/>`;
    out += `<text class="tl" x="${tb[0] + 2}" y="${y0 + 4.2}">ITEM</text><text class="tl" x="${tb[0] + 16}" y="${y0 + 4.2}">PART</text><text class="tl" x="${tb[0] + 90}" y="${y0 + 4.2}">MATERIAL</text><text class="tl" x="${tb[0] + 160}" y="${y0 + 4.2}">QTY</text>`;
    info.list.forEach((r, i) => {
      const y = y0 + lh * (i + 1);
      out += `<path d="M${tb[0]} ${y}H${tb[0] + 180}"/><text x="${tb[0] + 2}" y="${y + 4.2}">${r.item}</text><text x="${tb[0] + 16}" y="${y + 4.2}">${esc(r.name)}</text><text x="${tb[0] + 90}" y="${y + 4.2}">${esc(r.material)}</text><text x="${tb[0] + 160}" y="${y + 4.2}">${r.qty}</text>`;
    });
    out += '</g>';
  }
  out += `<text class="note" x="${m + 4}" y="${H - m - 4}">Every edge is drawn. Removing hidden lines exactly waits for the kernel's drawing engine (G394, G396).</text>`;
  return out + '</svg>';
}
