'use strict';
/* Spare Construct · map rendering. Two canvas layers ride on the Leaflet map: the network (redrawn on
   pan/zoom/edit) and a live overlay (hover, drafts, paths, simulated vehicles). Colour modes and
   extra map layers come from Registry (the built-in modes are registered here), hit testing and
   the legend live here too, and Render.snapshot draws a figure image for reports.
   Left-hand traffic: each direction is drawn on its own left side. */

const CanvasLayer = L.Layer.extend({
  initialize(draw, opts) { this._draw = draw; L.setOptions(this, opts); this._pad = (opts && opts.padding) ?? 0.35; },
  onAdd(map) {
    this._map = map;
    this._c = L.DomUtil.create('canvas', 'cv-layer leaflet-zoom-animated');
    this.getPane().appendChild(this._c);
    map.on('moveend resize', this._reset, this);
    map.on('zoomanim', this._anim, this);
    this._reset();
  },
  onRemove(map) { map.off('moveend resize', this._reset, this); map.off('zoomanim', this._anim, this); this._c.remove(); },
  _anim(e) {
    const m = this._map, scale = m.getZoomScale(e.zoom), off = m._latLngBoundsToNewLayerBounds(this._llb, e.zoom, e.center).min;
    L.DomUtil.setTransform(this._c, off, scale);
  },
  _reset() {
    const m = this._map, size = m.getSize(), pad = size.multiplyBy(this._pad).round();
    const tl = m.containerPointToLayerPoint(pad.multiplyBy(-1)).round(), full = size.add(pad.multiplyBy(2));
    this._origin = tl; this._size = full;
    this._llb = L.latLngBounds(m.layerPointToLatLng(tl), m.layerPointToLatLng(tl.add(full)));
    L.DomUtil.setPosition(this._c, tl);
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    this._dpr = dpr;
    this._c.width = Math.round(full.x * dpr); this._c.height = Math.round(full.y * dpr);
    this._c.style.width = full.x + 'px'; this._c.style.height = full.y + 'px';
    this.redraw();
  },
  redraw() {
    if (!this._map || !this._size) return;
    const ctx = this._c.getContext('2d');
    ctx.setTransform(this._dpr, 0, 0, this._dpr, 0, 0);
    ctx.clearRect(0, 0, this._size.x, this._size.y);
    this._draw(ctx, this._map.getPixelOrigin().add(this._origin), this._size);
  },
});

const Render = (() => {
  let map, net, fx, cache = { key: '' }, gv = 0, fxQueued = false, netQueued = false, lastSnap = null;
  let T = {};
  const LOS_COL = () => ({ A: T.losA, B: T.losB, C: T.losC, D: T.losD, E: T.losE, F: T.losF });

  function readTheme() {
    const cs = getComputedStyle(document.documentElement), v = n => cs.getPropertyValue(n).trim();
    T = {
      sel: v('--sel'), amber: v('--amber'), ink: v('--ink'), panel: v('--panel'), muted: v('--muted'), faint: v('--faint'), halo: v('--halo'),
      accent: v('--accent'), danger: v('--danger'), ok: v('--ok'), warn: v('--warn'), line: v('--line'), bg: v('--bg'),
      losA: v('--los-a'), losB: v('--los-b'), losC: v('--los-c'), losD: v('--los-d'), losE: v('--los-e'), losF: v('--los-f'),
      dark: matchMedia('(prefers-color-scheme: dark)').matches && document.documentElement.dataset.theme !== 'light' || document.documentElement.dataset.theme === 'dark',
    };
  }

  function init(m) {
    map = m;
    readTheme();
    matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => { readTheme(); redraw(); });
    map.createPane('netPane').style.zIndex = 410;
    map.createPane('fxPane').style.zIndex = 420;
    net = new CanvasLayer((c, o, s) => drawNet(c, o, s), { pane: 'netPane', padding: 0.35 }).addTo(map);
    fx = new CanvasLayer(drawFx, { pane: 'fxPane', padding: 0.1 }).addTo(map);
  }

  /* ---------- geometry in world pixels at a zoom ---------- */
  // g = { z, nodes Map id→point, links [{L, xy, minx…, rank}], byId, zones Map, deg, mpp, conns, routes, routePx, project(ll) }
  function buildGeo(sc, z, lat, old = {}, ver = `${Store.ver}|${sc.id}|${gv}`) {
    const nodes = new Map();
    for (const n of Object.values(sc.nodes)) nodes.set(n.id, map.project([n.lat, n.lng], z));
    const links = [], byId = new Map();
    for (const Lk of Object.values(sc.links)) {
      const A = nodes.get(Lk.a), B = nodes.get(Lk.b);
      if (!A || !B) continue;
      const xy = [A.x, A.y];
      for (const p of Lk.pts || []) { const q = map.project(p, z); xy.push(q.x, q.y); }
      xy.push(B.x, B.y);
      let minx = Infinity, miny = Infinity, maxx = -Infinity, maxy = -Infinity;
      for (let i = 0; i < xy.length; i += 2) { minx = Math.min(minx, xy[i]); maxx = Math.max(maxx, xy[i]); miny = Math.min(miny, xy[i + 1]); maxy = Math.max(maxy, xy[i + 1]); }
      const o = { L: Lk, xy, minx, miny, maxx, maxy, rank: (CLASSES[Lk.cls] || CLASSES.local).rank };
      links.push(o); byId.set(Lk.id, o);
    }
    links.sort((a, b) => a.rank - b.rank || (a.L.status === 'proposed') - (b.L.status === 'proposed'));
    const zones = new Map();
    for (const zn of Object.values(sc.zones)) zones.set(zn.id, map.project([zn.lat, zn.lng], z));
    const g = {
      z, nodes, links, byId, zones, deg: Net.degree(sc), sc,
      mpp: 156543.03392 * Math.cos(lat * Math.PI / 180) / Math.pow(2, z),
      connVer: old.connVer, conns: old.conns, routeVer: old.routeVer, routes: old.routes,
      project: ll => map.project(ll, z),
    };
    // connectors and bus-route paths only change with the model, not the zoom
    if (g.connVer !== ver) { g.conns = Engine.connectorMap(sc); g.connVer = ver; g.routes = null; }
    if (!g.routes) {
      const G = Object.keys(sc.routes).length ? Engine.buildGraph(sc) : null;
      g.routes = {};
      if (G) for (const r of Object.values(sc.routes)) g.routes[r.id] = Engine.resolveRoute(G, r).map(i => G.arcs[i].pts).flat();
    }
    g.routePx = {};
    for (const [id, pts] of Object.entries(g.routes)) {
      const xy = [];
      for (const p of pts) { const q = map.project(p, z); if (xy.length && Math.abs(xy[xy.length - 2] - q.x) < 0.01 && Math.abs(xy[xy.length - 1] - q.y) < 0.01) continue; xy.push(q.x, q.y); }
      g.routePx[id] = xy;
    }
    return g;
  }
  function geo() {
    const sc = Store.sc, z = map.getZoom(), key = `${z}|${Store.ver}|${sc.id}|${gv}`;
    if (cache.key === key) return cache;
    cache = buildGeo(sc, z, map.getCenter().lat, cache);
    cache.key = key;
    return cache;
  }
  const geomChanged = () => { gv++; redrawNet(); redrawFx(); };

  /* ---------- drawing helpers ---------- */
  // Stroke a polyline offset d px to the left of its direction of travel (reverse = draw B→A)
  function pathOffset(ctx, xy, off, d, reverse) {
    const n = xy.length / 2, P = new Array(xy.length);
    for (let i = 0; i < n; i++) { const k = reverse ? n - 1 - i : i; P[2 * i] = xy[2 * k] - off.x; P[2 * i + 1] = xy[2 * k + 1] - off.y; }
    ctx.beginPath();
    if (!d) { ctx.moveTo(P[0], P[1]); for (let i = 1; i < n; i++) ctx.lineTo(P[2 * i], P[2 * i + 1]); return P; }
    for (let i = 0; i < n; i++) {
      let n1x = 0, n1y = 0, n2x = 0, n2y = 0;
      if (i > 0) { const dx = P[2 * i] - P[2 * i - 2], dy = P[2 * i + 1] - P[2 * i - 1], l = Math.hypot(dx, dy) || 1; n1x = dy / l; n1y = -dx / l; }
      if (i < n - 1) { const dx = P[2 * i + 2] - P[2 * i], dy = P[2 * i + 3] - P[2 * i + 1], l = Math.hypot(dx, dy) || 1; n2x = dy / l; n2y = -dx / l; }
      if (i === 0) { n1x = n2x; n1y = n2y; }
      if (i === n - 1) { n2x = n1x; n2y = n1y; }
      let ax = n1x + n2x, ay = n1y + n2y; const al = Math.hypot(ax, ay) || 1; ax /= al; ay /= al;
      const k = 1 / Math.max(0.35, ax * n1x + ay * n1y);
      const x = P[2 * i] + ax * d * k, y = P[2 * i + 1] + ay * d * k;
      i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
    }
    return P;
  }
  function midpoint(xy, off, reverse) {
    const n = xy.length / 2;
    let tot = 0; const seg = [];
    for (let i = 0; i < n - 1; i++) { const l = Math.hypot(xy[2 * i + 2] - xy[2 * i], xy[2 * i + 3] - xy[2 * i + 1]); seg.push(l); tot += l; }
    let h = tot / 2;
    for (let i = 0; i < n - 1; i++) {
      if (h <= seg[i] || i === n - 2) {
        const f = seg[i] ? h / seg[i] : 0;
        let x = xy[2 * i] + (xy[2 * i + 2] - xy[2 * i]) * f - off.x, y = xy[2 * i + 1] + (xy[2 * i + 3] - xy[2 * i + 1]) * f - off.y;
        let dx = xy[2 * i + 2] - xy[2 * i], dy = xy[2 * i + 3] - xy[2 * i + 1];
        if (reverse) { dx = -dx; dy = -dy; }
        const l = Math.hypot(dx, dy) || 1;
        return { x, y, dx: dx / l, dy: dy / l, len: tot };
      }
      h -= seg[i];
    }
    return { x: xy[0] - off.x, y: xy[1] - off.y, dx: 1, dy: 0, len: tot };
  }
  const carriagewayM = Lk => ((Lk.lanesAB | 0) + (Lk.lanesBA | 0)) * (CLASSES[Lk.cls] || CLASSES.local).lw;
  function linkWidth(Lk, g) {
    const C = CLASSES[Lk.cls] || CLASSES.local, zf = g.z >= 15 ? 1 : g.z >= 13 ? 0.75 : 0.5;
    return Math.max(C.minPx * zf, carriagewayM(Lk) / g.mpp);
  }
  function rampColor(x, stops) { for (const [t, c] of stops) if (x <= t) return c; return stops[stops.length - 1][1]; }
  function arrow(ctx, x, y, dx, dy, s) {
    ctx.beginPath();
    ctx.moveTo(x + dx * s, y + dy * s);
    ctx.lineTo(x - dx * s * 0.6 + dy * s * 0.7, y - dy * s * 0.6 - dx * s * 0.7);
    ctx.lineTo(x - dx * s * 0.6 - dy * s * 0.7, y - dy * s * 0.6 + dx * s * 0.7);
    ctx.closePath(); ctx.fill();
  }
  function label(ctx, text, x, y, opts = {}) {
    ctx.font = opts.font || '500 11px "IBM Plex Mono", monospace';
    ctx.textAlign = opts.align || 'center'; ctx.textBaseline = 'middle';
    ctx.lineWidth = 3.5; ctx.strokeStyle = T.halo; ctx.lineJoin = 'round';
    ctx.strokeText(text, x, y);
    ctx.fillStyle = opts.color || T.ink; ctx.fillText(text, x, y);
  }
  // A small pill (gateway flow source): theme panel fill, ok or warn edge and text, so it reads in both themes
  function badge(ctx, b, x, y) {
    ctx.save();
    ctx.font = '600 9.5px Barlow, sans-serif'; ctx.textAlign = 'left'; ctx.textBaseline = 'middle';
    const w = ctx.measureText(b.text).width + 10, h = 14, col = b.tone === 'ok' ? T.ok : (T.warn || T.amber);
    roundRect(ctx, x, y - h / 2, w, h, 7);
    ctx.globalAlpha = 0.95; ctx.fillStyle = T.panel; ctx.fill(); ctx.globalAlpha = 1;
    ctx.lineWidth = 1.2; ctx.strokeStyle = col; ctx.stroke();
    ctx.fillStyle = col; ctx.fillText(b.text, x + 5, y + 0.5);
    ctx.restore();
  }
  /* Figure labels kept apart: higher road classes and bigger values first, a label that would overlap one already
     placed (or run off the figure) is left out. labels: [text, x, y, rank, |value|] */
  function declutter(ctx, labels, size) {
    ctx.font = '500 11px "IBM Plex Mono", monospace';
    const placed = [], out = [];
    for (const l of labels.slice().sort((a, b) => (b[3] || 0) - (a[3] || 0) || (b[4] || 0) - (a[4] || 0))) {
      const w = ctx.measureText(l[0]).width + 6, h = 15, x0 = l[1] - w / 2, y0 = l[2] - h / 2;
      if (x0 < 2 || y0 < 2 || x0 + w > size.x - 2 || y0 + h > size.y - 2) continue;
      if (placed.some(p => x0 < p[2] && p[0] < x0 + w && y0 < p[3] && p[1] < y0 + h)) continue;
      placed.push([x0, y0, x0 + w, y0 + h]); out.push(l);
    }
    return out;
  }

  /* ---------- built-in colour modes (registered like any feature's) ----------
     colour(arcKey, link, res, ctx) → { c colour, w width px | null, v value for labels, casing dark outline colour }.
     ctx = { z, sc, res, base (base scenario result), period, T (theme colours), S (settings) } plus what prepare() adds.
     key is the number key that picks the mode (map-card: 1–9). */
  const NONE = () => ({ c: T.line, w: 1.4 });
  const span = z => z >= 16 ? 30 : z >= 14 ? 20 : 12;
  const legendRows = {
    sw: (c, t, cls = '') => `<div class="lg-row"><span class="lg-sw ${cls}" style="background:${c}"></span><span>${t}</span></div>`,
    dot: (c, t) => `<div class="lg-row"><span class="lg-dot" style="background:${c}"></span><span>${t}</span></div>`,
  };
  const { sw, dot } = legendRows;
  const gl = (t, l) => Kit.gl(t, l);
  const needRes = res => !res ? `<div class="lg-note">Run the model to see results.</div>` : '';
  // LoS B, C and D carry dark text (accessibility (5)); A, E and F white
  const losInk = l => (l === 'B' || l === 'C' || l === 'D') ? '#1b1b1b' : '#fff';
  const losNode = (big, small) => (id, n, res, c) => {
    const nr = res && res.nodes[id]; if (!nr || c.z < 13) return null;
    if (small && (nr.los === 'A' || nr.los === 'B')) return { c: LOS_COL()[nr.los], r: c.z >= 16 ? 4 : 3, lw: 1.5 };
    const r = big(nr, c.z);
    return { c: LOS_COL()[nr.los], r, text: r >= 8 ? nr.los : null, ink: losInk(nr.los) };
  };
  // V/C bands are drawn wider from A to F (3 → 7 px at zoom 16) so they read without colour (accessibility (7))
  const VC_W = { A: 3, B: 3.8, C: 4.6, D: 5.4, E: 6.2, F: 7 };
  const vcZoom = z => z >= 16 ? 1 : z >= 15 ? 0.85 : z >= 14 ? 0.7 : 0.55;
  // Speed bands (map-card): ANOP % of the posted limit, or HCM 6 % of free-flow speed
  const SPEED_BANDS = {
    anop: { label: 'ANOP: % of the posted limit', cuts: [90, 70, 50, 40, 30], of: 'limit', src: 'AT Auckland Network Operating Plan (2022) level of service bands', srcHtml: () => `AT ${gl('ANOP', 'Auckland Network Operating Plan')} (2022) level of service bands` },
    hcm: { label: 'HCM 6: % of free-flow speed', cuts: [80, 67, 50, 40, 30], of: 'free-flow', src: 'HCM 6th edition urban street LoS by travel speed (Exhibit 16-4)', srcHtml: () => `${gl('HCM')} 6th edition urban street ${gl('LoS')} by travel speed (Exhibit 16-4)` },
  };
  // Disclaimer D5 (docs/regulatory/_disclaimers.md), carried wherever ANOP bands are shown
  const anopNote = () => `${gl('ANOP')} levels of service are indicative Spare Construct estimates. Confirm them in Auckland Transport’s ANOP Assessment Tool using AT speed data before relying on them.`;
  const speedBasis = () => SPEED_BANDS[(Store.S.display || {}).speedBands] || SPEED_BANDS.anop;
  const bandOf = (p, cuts) => 'ABCDE'[cuts.findIndex(c => p > c)] || 'F';
  // One name per concept (wording): the zone roles' own labels, shortened where they carry an explanation
  const ZCLASS_LABEL = { existing: 'Existing land use', committed: 'Committed development', development: 'Proposed development', external: 'Gateway' };
  const isoTitle = () => ({ walk: 'Walk time from point', cycle: 'Cycle time from point' }[View.iso && View.iso.mode] || 'Drive time from point');
  /* Flow labels: whole veh/h on the live map; report figures (Render.snapshot) round them to ctx.labelRound veh/h
     (10 by default), half away from zero, so +15 → +20 and −15 → −20. A value that rounds to nothing is 0. */
  function roundFlow(v, step = 10) {
    const s = +step > 0 ? +step : 1, x = +v;
    if (!isFinite(x)) return 0;
    const r = Math.sign(x) * Math.round(Math.abs(x) / s) * s;
    return r === 0 ? 0 : +r.toFixed(6);
  }
  const roundOf = c => (c && +c.labelRound > 1 ? +c.labelRound : 0);
  const flowInt = (v, c) => U.fmt.int(roundOf(c) ? roundFlow(v, roundOf(c)) : v);
  /* Development share (%): development veh/h on the arc ÷ all veh/h on it × 100, from the development class of the
     flow (res.cls.arcs[key][2], as the Development traffic mode uses). Arcs missing from res.cls.arcs carry no
     committed or development traffic, so their share is 0. Bands in LoS order (the colour-blind palette applies). */
  const DS_BANDS = [[5, 'losA', 'Under 5%'], [10, 'losB', '5–10%'], [25, 'losC', '10–25%'], [50, 'losD', '25–50%'], [Infinity, 'losF', '50% or more']];
  const devShareBand = pct => { const i = DS_BANDS.findIndex(b => pct < b[0]); return i < 0 ? DS_BANDS.length - 1 : i; };
  function devShare(res, key) {
    const r = res && res.arcs && res.arcs[key];
    if (!r || !res.cls) return null;
    const a = res.cls.arcs && res.cls.arcs[key], dev = a ? Math.max(0, +a[2] || 0) : 0, tot = Math.max(0, +r.v || 0);
    const pct = tot > 0 ? Math.min(100, dev / tot * 100) : 0;
    return { dev, tot, pct, band: devShareBand(pct) };
  }
  // Gateway labels: whether the gateway's flows come from counts (z.gwSrc, set by DataHub.gatewaysFromCounts) or are estimated
  const gatewayBadge = zn => (zn && zn.kind === 'gateway' ? (zn.gwSrc ? { text: 'From counts', tone: 'ok' } : { text: 'Estimated', tone: 'warn' }) : null);
  [
    { id: 'class', label: 'Road class', order: 10, key: '1', perArc: false, legend: () => `<div class="lg-title">Road class</div>${Object.values(CLASSES).map(c => sw(c.color, c.label)).join('')}${sw('', 'Proposed road', 'dash')}` },
    { id: 'status', label: 'Existing / proposed', order: 20, key: '2', perArc: false, linkColour: Lk => Lk.status === 'proposed' ? T.amber : T.faint,
      legend: () => `<div class="lg-title">Status</div>${sw(T.faint, 'Existing')}${sw(T.amber, 'Proposed')}${sw(T.danger, 'Closed', '')}` },
    { id: 'volume', label: 'Traffic volume', order: 30, key: '3', needsResults: true, flowLabels: true,
      prepare: (res, c) => { let mv = 1; for (const r of Object.values(res.arcs)) mv = Math.max(mv, r.v); c.volScale = mv / span(c.z); },
      colour: (key, Lk, res, c) => { const r = res && res.arcs[key]; if (!r) return NONE(); return { c: rampColor(r.vc, [[0.55, '#4c6ef5'], [0.75, '#7048e8'], [0.9, '#ae3ec9'], [1, '#e64980'], [Infinity, '#c92a2a']]), w: 1.5 + r.v / c.volScale, v: r.v }; },
      labelText: (v, c) => flowInt(v, c),
      legend: res => `<div class="lg-title">Traffic volume</div><div class="lg-note" style="margin:0 0 6px">Line width = vehicles per hour. Colour = how full the road is (${gl('V/C')}).</div>${sw('#4c6ef5', 'Under 55% of capacity')}${sw('#7048e8', '55–75%')}${sw('#ae3ec9', '75–90%')}${sw('#e64980', '90–100%')}${sw('#c92a2a', 'Over capacity')}${needRes(res)}` },
    { id: 'vc', label: 'V/C bands (A–F)', order: 40, key: '4', needsResults: true,
      colour: (key, Lk, res, c) => {
        const r = res && res.arcs[key]; if (!r) return NONE();
        const w = (VC_W[r.los] || 3) * vcZoom(c.z);
        return { c: LOS_COL()[r.los], w, v: r.los >= 'D' ? `${r.los} ${r.vc.toFixed(2)}` : r.vc, casing: r.los === 'E' || r.los === 'F' ? '#1b1b1b' : null };
      },
      labelText: v => typeof v === 'number' ? v.toFixed(2) : v, nodeColour: losNode((nr, z) => z >= 15 ? 8 : 6, true),
      legend: res => `<div class="lg-title">Volume / capacity (${gl('V/C')}) bands</div>${[['A', '≤ 0.35'], ['B', '≤ 0.55'], ['C', '≤ 0.75'], ['D', '≤ 0.90'], ['E', '≤ 1.00'], ['F', '> 1.00']].map(([l, t]) => `<div class="lg-row"><span class="lg-sw" style="background:${LOS_COL()[l]};height:${Math.round(VC_W[l])}px${l === 'E' || l === 'F' ? ';box-shadow:0 0 0 1px #1b1b1b' : ''}"></span><span><b>${l}</b>&nbsp; ${gl('V/C')} ${t}</span></div>`).join('')}<div class="lg-note">Bands use Spare Construct ${gl('V/C')} thresholds 0.35/0.55/0.75/0.90/1.00; not HCM urban-street ${gl('LoS')}. Wider lines are fuller roads; E and F have a dark edge and, zoomed in, a letter.</div><div class="lg-note">Circles: junction ${gl('LoS')} by delay. ${U.esc(Engine.JUNCTION_LABEL || 'HCM 6 / Akcelik-based, indicative; not SIDRA output')}.</div>${needRes(res)}` },
    { id: 'speed', label: 'Speed (% of limit)', order: 50, key: '5', needsResults: true,
      prepare: (res, c) => { c.sb = speedBasis(); },
      colour: (key, Lk, res, c) => {
        const r = res && res.arcs[key]; if (!r) return NONE();
        // ANOP: against the legal limit (NSLR once loaded, else the road's limit); HCM: against free-flow speed
        const ref = c.sb.of === 'limit' ? (+Lk.speedLimit || +Lk.speed || 50) : (r.ff > 0 ? r.len / r.ff * 3.6 : (+Lk.speed || 50));
        return { c: LOS_COL()[bandOf(r.spd / ref * 100, c.sb.cuts)], v: r.spd };
      },
      labelText: v => Math.round(v) + '',
      legend: res => { const b = speedBasis(), k = b.cuts;
        return `<div class="lg-title">Speed · ${b.of === 'limit' ? '% of the speed limit' : '% of free-flow speed'}</div>${['A', 'B', 'C', 'D', 'E'].map((l, i) => sw(LOS_COL()[l], `<b>${l}</b>&nbsp; over ${k[i]}%`)).join('')}${sw(LOS_COL().F, `<b>F</b>&nbsp; ${k[4]}% or less`)}
          <label class="lg-ctl">Bands <select data-mc="speedBands">${Kit.opt({ anop: 'ANOP (% of limit)', hcm: 'HCM 6 (% of free-flow)' }, (Store.S.display || {}).speedBands || 'anop')}</select></label><div class="lg-note">${b.srcHtml ? b.srcHtml() : U.esc(b.src)}. Modelled peak-hour speeds, indicative.${b.of === 'limit' ? ' Measured against the legal speed limit (NZTA speed limit register where loaded).' : ''}</div>${b.of === 'limit' ? `<div class="lg-note">${anopNote()}</div>` : ''}${needRes(res)}`; } },
    { id: 'delay', label: 'Junction delay', order: 60, key: '6', needsResults: true,
      colour: (key, Lk, res) => res && res.arcs[key] ? { c: T.dark ? '#3b4643' : '#c9d1cd', w: 2 } : NONE(),
      nodeColour: losNode(nr => U.clamp(4 + Math.sqrt(nr.vol) / 4, 6, 20), false),
      legend: res => `<div class="lg-title">Junction delay · ${gl('LoS')}</div>${[['A', '≤ 10 s'], ['B', '≤ 20 s'], ['C', '≤ 35 s'], ['D', '≤ 55 s'], ['E', '≤ 80 s'], ['F', '> 80 s']].map(([l, t]) => dot(LOS_COL()[l], `<b>${l}</b>&nbsp; ${t} (signals)`)).join('')}<div class="lg-note">Give-way, stop and roundabout junctions use the unsignalised scale: A ≤ 10 s … F > 50 s; F whenever demand exceeds capacity. Circle size = traffic through the junction. ${U.esc(Engine.JUNCTION_LABEL || 'Indicative; not SIDRA output')}.</div>${needRes(res)}` },
    // needsBase: with a compared scenario named (Render.snapshot baseScenarioId) that has no result, draw the network plain
    { id: 'diff', label: 'Change vs base', order: 70, key: '7', needsResults: true, flowLabels: true, needsBase: true,
      prepare: (res, c) => { let md = 1; if (c.base) for (const [k, r] of Object.entries(res.arcs)) md = Math.max(md, Math.abs(r.v - ((c.base.arcs[k] || {}).v || 0))); c.diffScale = md / span(c.z); },
      colour: (key, Lk, res, c) => {
        const r = res && res.arcs[key], b = c.base && c.base.arcs[key], dv = (r ? r.v : 0) - (b ? b.v : 0);
        if (Math.abs(dv) < 15) return { c: T.line, w: 1.4, v: dv };
        return { c: dv > 0 ? '#d6336c' : '#1c7ed6', w: 1.5 + Math.abs(dv) / c.diffScale, v: dv, isNew: !b };
      },
      // rounded figure labels that come to 0 are left out
      labelText: (v, c) => { const s = roundOf(c); if (!s) return U.fmt.signed(v); const r = roundFlow(v, s); return r ? U.fmt.signed(r) : null; },
      legend: (res, c) => {
        const bn = U.esc(c.baseSc ? c.baseSc.name : 'the base');
        // a figure that names the scenario to compare with (with minus without)
        if (c.compare) {
          const title = `<div class="lg-title">Change vs ${bn}</div>`;
          if (!c.base) return `${title}<div class="lg-note">Run ${bn} for this period.</div><div class="lg-note">The network is shown without the change until then.</div>`;
          return `${title}${sw('#d6336c', 'More traffic')}${sw('#1c7ed6', 'Less traffic')}${sw(T.line, 'Within ±15 veh/h')}<div class="lg-note">${U.esc(c.sc.name)} minus ${bn}, veh/h. Line width = size of the change.</div>${c.baseSc && c.sc.id === c.baseSc.id ? '<div class="lg-note">Pick another scenario to compare with.</div>' : ''}${!res ? `<div class="lg-note">Run ${U.esc(c.sc.name)} for this period.</div>` : ''}`;
        }
        return `<div class="lg-title">Change vs ${bn}</div>${sw('#d6336c', 'More traffic')}${sw('#1c7ed6', 'Less traffic')}${sw(T.line, 'Within ±15 veh/h')}<div class="lg-note">Line width = size of the change.</div>${c.sc.id === Store.P.base ? '<div class="lg-note">Pick another scenario to compare it with the base.</div>' : ''}${!c.base ? '<div class="lg-note">Run the base scenario too.</div>' : ''}`;
      } },
    // Development traffic (dev-tracking): the development class of each link's flow
    { id: 'dev', label: 'Development traffic', order: 75, key: '8', needsResults: true, flowLabels: true,
      prepare: (res, c) => { let mv = 1; for (const a of Object.values((res.cls || {}).arcs || {})) mv = Math.max(mv, a[2] || 0); c.devScale = mv / span(c.z); },
      colour: (key, Lk, res, c) => {
        const a = res && res.cls && res.cls.arcs && res.cls.arcs[key], d = a ? a[2] || 0 : 0;
        if (d < 1) return { c: T.line, w: 1.4 };
        return { c: '#c2255c', w: 1.5 + d / c.devScale, v: d };
      },
      labelText: (v, c) => flowInt(v, c),
      legend: res => `<div class="lg-title">Development traffic</div>${sw('#c2255c', 'Trips to and from proposed development zones')}${sw(T.line, 'Under 1 veh/h')}<div class="lg-note">Line width = development veh/h. Set each zone’s type (existing land use, committed or proposed development) in the zone panel.</div>${res && !res.cls ? '<div class="lg-note">Run the model again to see development traffic.</div>' : ''}${needRes(res)}` },
    // Development share (%): how much of each road's flow is development traffic. Width = all veh/h; quiet arcs faint
    { id: 'devshare', label: 'Development share (%)', order: 76, needsResults: true,
      prepare: (res, c) => { let mv = 1; for (const r of Object.values(res.arcs)) mv = Math.max(mv, r.v); c.dsScale = mv / span(c.z); },
      colour: (key, Lk, res, c) => {
        const s = devShare(res, key), TT = (c && c.T) || T; if (!s) return NONE();
        if (s.tot < 10) return { c: TT.line, w: 1.4 };
        return { c: TT[DS_BANDS[s.band][1]], w: 2 + s.tot / (c.dsScale || 1), v: s.pct, casing: s.band === DS_BANDS.length - 1 ? '#1b1b1b' : null };
      },
      labelText: v => (v >= 0.5 ? `${Math.round(v)}%` : null),
      legend: (res, c) => { const TT = (c && c.T) || T;
        return `<div class="lg-title">Development share (%)</div><div class="lg-note" style="margin:0 0 6px">Share of each road’s modelled flow that is development traffic (estimated). Line width = all vehicles per hour.</div>${DS_BANDS.map(([, k, t], i) => `<div class="lg-row"><span class="lg-sw" style="background:${TT[k]}${i === DS_BANDS.length - 1 ? ';box-shadow:0 0 0 1px #1b1b1b' : ''}"></span><span>${t}</span></div>`).join('')}${sw(TT.line, 'Under 10 veh/h in total')}<div class="lg-note">Development traffic = trips to and from proposed development zones. Set each zone’s type in the zone panel.</div>${res && !res.cls ? '<div class="lg-note">Run the model again to see development traffic.</div>' : ''}${needRes(res)}`; } },
    // Converted to a plain registered mode: cycling stress needs no model run (it uses volumes when there are some)
    { id: 'lts', label: 'Cycling stress', order: 80, key: '9', proposedOutline: false,
      colour: (key, Lk, res, c) => ({ c: [c.T.faint, c.T.losA, c.T.losB, c.T.losD, c.T.losF][Engine.lts(Lk, res)], w: null }),
      legend: (res, c) => `<div class="lg-title">Cycling stress (${gl('LTS')})</div>${sw(c.T.losA, '1 · Suitable for all ages')}${sw(c.T.losB, '2 · Most adults')}${sw(c.T.losD, '3 · Confident riders')}${sw(c.T.losF, '4 · Strong and fearless')}${sw(c.T.faint, 'No cycling (motorway)')}` },
    // Crash risk from NZTA CAS (DataHub.linkRisk), KiwiRAP bands, indicative; short or quiet links get no band
    { id: 'risk', label: 'Crash risk', order: 85, perArc: false, proposedOutline: false,
      linkColour: (Lk, c) => { const R = DataHub.D().crashes.length ? DataHub.linkRisk(c.sc) : null; if (!R) return T.line; const b = DataHub.linkBand(R[Lk.id], View.riskKind); return b.band ? b.colour : T.line; },
      legend: (res, c) => {
        if (!DataHub.D().crashes.length) return `<div class="lg-title">Crash risk</div><div class="lg-note">Load crashes in the Traffic data tab (NZTA ${gl('CAS')}) to see crash risk.</div>`;
        const seen = new Map();
        for (const v of [0, 1e-3, 0.03, 0.05, 0.08, 0.15, 0.3, 0.6, 1, 2, 4.5, 6, 8, 10, 20, 50, 100]) { const b = DataHub.riskBand(v, View.riskKind); if (b.band && !seen.has(b.band)) seen.set(b.band, b); }
        const basis = DataHub.riskBand(null, View.riskKind).basis;
        return `<div class="lg-title">Crash risk · ${View.riskKind === 'personal' ? 'personal' : 'collective'}</div>${[...seen.values()].map(b => sw(b.colour, U.esc(b.label))).join('')}${sw(T.line, 'No band (under 500 m, quiet or no data)')}
          <label class="lg-ctl">Measure <select data-mc="riskKind">${Kit.opt({ collective: 'Collective (fatal and serious crashes per km a year)', personal: 'Personal (fatal and serious crashes per 100 million vehicle-km)' }, View.riskKind)}</select></label><div class="lg-note">${U.esc(basis)}. Reported crashes, ${U.esc(Kit.casText())}.</div>`;
      } },
    { id: 'iso', label: 'Travel time from point', order: 900, hidden: true,
      colour: (key, Lk) => {
        const iso = View.iso; if (!iso) return { c: T.faint };
        const tA = iso.times.get(Lk.a), tB = iso.times.get(Lk.b), t = Math.min(tA ?? Infinity, tB ?? Infinity);
        if (!isFinite(t)) return { c: T.line, w: 1.5 };
        return { c: rampColor(t / 60, [[2, '#0b7a75'], [5, '#2a9d8f'], [10, '#8ab17d'], [15, '#e9c46a'], [20, '#f4a261'], [30, '#e76f51'], [Infinity, '#9d0208']]) };
      },
      legend: res => `<div class="lg-title">${isoTitle()}</div>${[['#0b7a75', '0–2 min'], ['#2a9d8f', '2–5 min'], ['#8ab17d', '5–10 min'], ['#e9c46a', '10–15 min'], ['#f4a261', '15–20 min'], ['#e76f51', '20–30 min'], ['#9d0208', '30 min +']].map(([c, t]) => sw(c, t)).join('')}
        <label class="lg-ctl">Travel by <select data-mc="isoMode">${Kit.opt({ drive: 'Driving', walk: 'Walking', cycle: 'Cycling' }, View.isoMode)}</select></label>
        <div class="lg-note">${View.iso && View.iso.basis ? U.esc(View.iso.basis) : res ? 'Uses congested peak-hour travel times.' : 'Free-flow times; run the model for congested times.'}</div>` },
    { id: 'select', label: 'Select-link analysis', order: 910, hidden: true, needsResults: true, flowLabels: true,
      prepare: (res, c) => { if (View.sla) { let mf = 1; for (const f of Object.values(View.sla.flows)) mf = Math.max(mf, f); c.slaScale = mf / (c.z >= 15 ? 26 : 16); } },
      colour: (key, Lk, res, c) => { const f = View.sla && View.sla.flows[key]; return f ? { c: '#c2255c', w: 1.5 + f / c.slaScale, v: f } : { c: T.line, w: 1.2 }; },
      labelText: (v, c) => flowInt(v / Engine.pcuF(Store.S), c),
      legend: () => `<div class="lg-title">Select-link analysis</div>${sw('#c2255c', 'Traffic using the selected road')}<div class="lg-note">${View.sla ? U.fmt.int(View.sla.tot / Engine.pcuF(Store.S)) + ' veh/h use it.' : 'Select a road and press “Who uses this road”.'}</div>` },
  ].forEach(d => Registry.modes({ ...d, builtin: true }));
  const modeDef = id => Registry.get('modes', id) || Registry.get('modes', 'class');

  /* ---------- network layer ---------- */
  // o = { sc, res, baseRes, baseSc, mode, period, g } — the live map passes the current state; snapshot passes its own
  // (snapshotState) with compare, plain, labels, labelRound and declutter as well
  const liveState = () => ({ sc: Store.sc, res: Store.res(), baseRes: Store.res(Store.baseSc), baseSc: Store.baseSc, mode: View.colorBy, period: Store.S.period, g: geo() });
  function modeCtx(o, z) { return { z, sc: o.sc, res: o.res, base: o.baseRes, baseSc: o.baseSc, period: o.period, T, S: Store.S, compare: !!o.compare, labelRound: +o.labelRound > 1 ? +o.labelRound : 1 }; }
  /* The state a figure is drawn from (Render.snapshot), without geometry. baseScenarioId names the scenario to
     compare with (diff mode, ctx.base and ctx.baseSc) instead of the project's base; when it has no result for the
     period, modes with needsBase draw the network plain and the legend says to run it. labels: true | false
     (default true for flow-label modes, otherwise as on the live map); labelRound: veh/h step (default 10). */
  function snapshotState(o = {}) {
    const P = Store.P, find = id => (id != null && P ? P.scenarios.find(s => s.id === id) || null : null);
    const sc = find(o.scenarioId) || Store.sc, period = o.period || Store.S.period;
    const cmp = find(o.baseScenarioId), baseSc = cmp || Store.baseSc;
    const mode = o.mode || View.colorBy, md = modeDef(mode);
    const res = Store.res(sc, period), baseRes = Store.res(baseSc, period);
    return {
      sc, period, mode, res, baseRes, baseSc, compare: !!cmp, plain: !!(cmp && !baseRes && md.needsBase),
      labels: o.labels !== undefined ? !!o.labels : md.flowLabels ? true : undefined,
      labelRound: o.labelRound === undefined ? 10 : +o.labelRound > 0 ? +o.labelRound : 1, declutter: true,
    };
  }
  function drawLayers(ctx, off, size, g, below) {
    for (const l of Registry.list('layers', l => !!l.below === below && View.show[l.id] && !(l.minZoom > g.z))) {
      ctx.save();
      try { l.draw(ctx, off, size, g, g.z); } catch (e) { reportOnce('layer ' + l.id, e); }
      ctx.restore();
    }
  }
  const reported = new Set();
  function reportOnce(where, e) { if (reported.has(where)) return; reported.add(where); Registry.errors.push({ where, message: String(e && e.message || e), at: Date.now() }); console.error(`Spare Construct plug-in ${where} failed:`, e); }

  function drawNet(ctx, off, size, o = liveState()) {
    if (!Store.P) return;
    const g = o.g, sc = o.sc, z = g.z, res = o.res;
    const md = modeDef(o.mode);
    const x0 = off.x - 20, y0 = off.y - 20, x1 = off.x + size.x + 20, y1 = off.y + size.y + 20;
    const vis = q => !(q.maxx < x0 || q.minx > x1 || q.maxy < y0 || q.miny > y1);
    ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    const perArc = md.perArc !== false && !!md.colour;
    const plain = !perArc || (!res && md.needsResults) || !!o.plain;
    const mc = modeCtx(o, z);
    // Labels: on the live map at zoom 16+ when switched on; a figure can force them on (from zoom 12) or off
    const showLabels = o.labels === undefined ? View.show.labels && z >= 16 : !!o.labels && z >= 12;
    if (perArc && !plain && md.prepare) { try { md.prepare(res, mc); } catch (e) { reportOnce('mode ' + md.id + '.prepare', e); } }
    const colour = (key, Lk) => { try { return md.colour(key, Lk, res, mc) || NONE(); } catch (e) { reportOnce('mode ' + md.id + '.colour', e); return NONE(); } };
    // Out-of-date results are drawn faded (results-trust (a)); the banner over the map says why
    const fade = !!(res && res.stale && md.needsResults);
    drawLayers(ctx, off, size, g, true);
    if (fade) ctx.globalAlpha = 0.45;
    const labels = [];
    for (const q of g.links) {
      if (!vis(q)) continue;
      const Lk = q.L, C = CLASSES[Lk.cls] || CLASSES.local, w = linkWidth(Lk, g);
      if (z < 12 && C.rank <= 2 && Lk.status !== 'proposed') continue;
      if (Lk.status === 'closed') {
        ctx.setLineDash([4, 4]); ctx.strokeStyle = T.danger; ctx.globalAlpha = 0.7; ctx.lineWidth = Math.max(1.5, w * 0.5);
        pathOffset(ctx, q.xy, off, 0); ctx.stroke(); ctx.setLineDash([]); ctx.globalAlpha = fade ? 0.45 : 1;
        continue;
      }
      if (plain) {
        // Plain network view
        if (Lk.status === 'proposed') {
          ctx.setLineDash([7, 5]); ctx.strokeStyle = T.amber; ctx.lineWidth = w + 6;
          pathOffset(ctx, q.xy, off, 0); ctx.stroke(); ctx.setLineDash([]);
        }
        ctx.strokeStyle = md.linkColour ? (Registry.call(md, 'linkColour', Lk, mc) || C.color) : C.color;
        ctx.lineWidth = w;
        pathOffset(ctx, q.xy, off, 0); ctx.stroke();
        if (w >= 16 && z >= 17) laneMarkings(ctx, q, off, g);
        if (z >= 16 && (!(Lk.lanesAB > 0) || !(Lk.lanesBA > 0))) {
          const m = midpoint(q.xy, off, !(Lk.lanesAB > 0));
          ctx.fillStyle = T.dark ? 'rgba(255,255,255,.85)' : 'rgba(255,255,255,.95)';
          arrow(ctx, m.x, m.y, m.dx, m.dy, Math.min(7, Math.max(4, w * 0.35)));
        }
        if (Lk.busLane && z >= 15) {
          ctx.strokeStyle = '#c92a2a'; ctx.lineWidth = Math.max(1.5, w * 0.18); ctx.setLineDash([2, 3]);
          pathOffset(ctx, q.xy, off, w * 0.42); ctx.stroke(); ctx.setLineDash([]);
        }
        continue;
      }
      // Per-direction results view
      const two = Lk.lanesAB > 0 && Lk.lanesBA > 0;
      for (const dir of [1, -1]) {
        const lanes = dir > 0 ? Lk.lanesAB : Lk.lanesBA;
        if (!(lanes > 0)) continue;
        const key = Lk.id + (dir > 0 ? ':ab' : ':ba');
        const m = colour(key, Lk);
        const ww = m.w != null ? m.w : Math.max(two ? 2.4 : 3, two ? w / 2 : w);
        if (Lk.status === 'proposed' && md.proposedOutline !== false) {
          ctx.setLineDash([6, 4]); ctx.strokeStyle = T.amber; ctx.lineWidth = ww + 4;
          pathOffset(ctx, q.xy, off, two ? ww / 2 + 1 : 0, dir < 0); ctx.stroke(); ctx.setLineDash([]);
        }
        // a dark edge (casing) so a band reads without its colour, e.g. V/C E and F
        if (m.casing) { ctx.strokeStyle = m.casing; ctx.lineWidth = ww + 2; pathOffset(ctx, q.xy, off, two ? ww / 2 + 1 : 0, dir < 0); ctx.stroke(); }
        ctx.strokeStyle = m.c; ctx.lineWidth = ww;
        pathOffset(ctx, q.xy, off, two ? ww / 2 + 1 : 0, dir < 0); ctx.stroke();
        if (showLabels && m.v != null && md.labelText) {
          const mp = midpoint(q.xy, off, dir < 0);
          if (mp.len > 60) {
            const d = (two ? ww + 10 : ww / 2 + 10);
            const txt = Registry.call(md, 'labelText', m.v, mc);
            if (txt != null) labels.push([String(txt), mp.x + mp.dy * d, mp.y - mp.dx * d, q.rank, Math.abs(+m.v) || 0]);
          }
        }
      }
    }
    ctx.globalAlpha = 1;
    // Zone connectors
    if (View.show.zones && View.show.connectors && z >= 13) {
      ctx.setLineDash([3, 4]); ctx.lineWidth = 1.2; ctx.strokeStyle = T.muted; ctx.globalAlpha = 0.7;
      for (const [zid, targets] of Object.entries(g.conns || {})) {
        const p = g.zones.get(zid); if (!p) continue;
        for (const t of targets) {
          const q = g.nodes.get(t); if (!q) continue;
          ctx.beginPath(); ctx.moveTo(p.x - off.x, p.y - off.y); ctx.lineTo(q.x - off.x, q.y - off.y); ctx.stroke();
        }
      }
      ctx.setLineDash([]); ctx.globalAlpha = 1;
    }
    // Bus routes
    if (View.show.routes) {
      for (const r of Object.values(sc.routes)) {
        const xy = g.routePx[r.id]; if (!xy || xy.length < 4) continue;
        ctx.strokeStyle = r.color || '#7048e8'; ctx.lineWidth = z >= 15 ? 3.5 : 2.5; ctx.globalAlpha = 0.9;
        pathOffset(ctx, xy, off, z >= 15 ? 9 : 5); ctx.stroke(); ctx.globalAlpha = 1;
      }
    }
    // Junctions
    const showNodes = View.show.nodes && (z >= 15 || View.tool === 'road' || View.tool === 'bus' || View.tool === 'gateway');
    const nodeCol = md.nodeColour && (res || !md.needsResults) ? md : null;
    for (const [id, p] of g.nodes) {
      const x = p.x - off.x, y = p.y - off.y;
      if (x < -20 || y < -20 || x > size.x + 20 || y > size.y + 20) continue;
      const n = sc.nodes[id], d = g.deg.get(id) || 0;
      const nc = nodeCol && Registry.call(nodeCol, 'nodeColour', id, n, res, mc);
      if (nc) {
        ctx.beginPath(); ctx.arc(x, y, nc.r || 6, 0, Math.PI * 2);
        if (fade) ctx.globalAlpha = 0.45;
        ctx.fillStyle = nc.c; ctx.fill(); ctx.lineWidth = nc.lw || 2; ctx.strokeStyle = T.halo; ctx.stroke();
        if (nc.text) { ctx.fillStyle = nc.ink || '#fff'; ctx.font = `700 ${Math.round((nc.r || 6) * 1.05)}px Barlow, sans-serif`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(nc.text, x, y + 0.5); }
        ctx.globalAlpha = 1;
        continue;
      }
      if (z >= 14 && n.control !== 'none') { nodeSymbol(ctx, n.control, x, y, z); continue; }
      if (showNodes && d !== 2) {
        ctx.beginPath(); ctx.arc(x, y, d >= 3 ? 3.6 : 3, 0, Math.PI * 2);
        ctx.fillStyle = T.panel; ctx.fill(); ctx.lineWidth = 1.4; ctx.strokeStyle = T.ink; ctx.stroke();
      }
    }
    if (View.show.crashes && z >= 13) drawCrashes(ctx, off, size, z);
    drawLayers(ctx, off, size, g, false);
    for (const l of o.declutter ? declutter(ctx, labels, size) : labels) label(ctx, l[0], l[1], l[2]);
    if (View.show.counts && z >= 12) drawCounts(ctx, off, size, z, sc, o.period);
    // Zones
    if (View.show.zones && z >= 11) {
      for (const zn of Object.values(sc.zones)) {
        const p = g.zones.get(zn.id); if (!p) continue;
        const x = p.x - off.x, y = p.y - off.y;
        if (zn.kind === 'gateway') {
          ctx.beginPath(); ctx.moveTo(x, y - 8); ctx.lineTo(x + 8, y); ctx.lineTo(x, y + 8); ctx.lineTo(x - 8, y); ctx.closePath();
          ctx.fillStyle = T.ink; ctx.fill(); ctx.lineWidth = 2; ctx.strokeStyle = T.halo; ctx.stroke();
          ctx.fillStyle = T.panel; ctx.font = '700 9px Barlow, sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText('G', x, y + 0.5);
        } else {
          // existing land use: hollow; committed development: amber; proposed development: solid accent (Engine.zoneClass)
          const zc = Engine.zoneClass ? Engine.zoneClass(zn) : 'development', ex = zc === 'existing';
          const fill = ex ? T.panel : zc === 'committed' ? T.amber : T.accent, glyph = ex ? T.muted : '#fff';
          ctx.fillStyle = fill; ctx.strokeStyle = ex ? T.muted : T.halo; ctx.lineWidth = ex ? 1.6 : 2;
          ctx.beginPath(); ctx.rect(x - 7, y - 7, 14, 14); ctx.fill(); ctx.stroke();
          if (ex) { ctx.strokeStyle = T.halo; ctx.lineWidth = 1; ctx.strokeRect(x - 8.5, y - 8.5, 17, 17); }
          ctx.fillStyle = glyph; ctx.fillRect(x - 3, y - 1, 6, 4); ctx.beginPath(); ctx.moveTo(x - 4, y - 1); ctx.lineTo(x, y - 4.5); ctx.lineTo(x + 4, y - 1); ctx.fill();
        }
        if (z >= 14) {
          const t = res && res.zoneIds ? res.zoneIds.indexOf(zn.id) : -1;
          const sub = t >= 0 ? `${U.fmt.int(res.O[t])} out · ${U.fmt.int(res.D[t])} in` : '';
          label(ctx, zn.name, x + 12, y - (sub ? 6 : 0), { align: 'left', font: '600 12px Barlow, sans-serif' });
          if (sub) label(ctx, sub, x + 12, y + 8, { align: 'left', font: '500 10.5px "IBM Plex Mono", monospace', color: T.muted });
          const bd = gatewayBadge(zn);
          if (bd) badge(ctx, bd, x + 12, y + (sub ? 22 : 14));
        }
      }
    }
  }

  const SEV_COL = { F: '#1b1b1b', S: '#e03131', M: '#f59f00', N: '#9aa5a1' };
  const GEH_COL = geh => geh < 5 ? '#2f9e44' : geh < 10 ? '#f08c00' : '#e03131';
  function drawCrashes(ctx, off, size, z) {
    const cr = DataHub.D().crashes; if (!cr.length) return;
    const r = z >= 16 ? 3.4 : z >= 14 ? 2.6 : 2;
    for (const sev of ['N', 'M', 'S', 'F']) {
      ctx.fillStyle = SEV_COL[sev]; ctx.strokeStyle = sev === 'F' ? '#e03131' : T.halo; ctx.lineWidth = sev === 'F' ? 1.8 : 1;
      const rr = sev === 'F' || sev === 'S' ? r + 1.2 : r;
      for (const c of cr) {
        if (c[2] !== sev) continue;
        const p = map.project([c[0], c[1]], z), x = p.x - off.x, y = p.y - off.y;
        if (x < -6 || y < -6 || x > size.x + 6 || y > size.y + 6) continue;
        ctx.beginPath(); ctx.arc(x, y, rr, 0, 7); ctx.fill(); ctx.stroke();
      }
    }
  }
  function drawCounts(ctx, off, size, z, sc = Store.sc, period = Store.S.period) {
    const d = DataHub.D(); if (!d.counts.length) return;
    const cal = DataHub.calibration(sc, period), m = DataHub.matches(sc);
    const byId = new Map(cal && cal.rows ? cal.rows.map(r => [r.c.id, r]) : []);
    const s = z >= 15 ? 6.5 : 5;
    for (const c of d.counts) {
      const p = map.project([c.lat, c.lng], z), x = p.x - off.x, y = p.y - off.y;
      if (x < -10 || y < -10 || x > size.x + 10 || y > size.y + 10) continue;
      const row = byId.get(c.id), used = DataHub.inUse(c) && m.has(c.id);
      const col = row ? GEH_COL(row.geh) : DataHub.SOURCES[c.src].colour;
      roundRect(ctx, x - s, y - s, 2 * s, 2 * s, 2.5);
      ctx.fillStyle = used ? col : T.panel; ctx.fill();
      ctx.lineWidth = 2; ctx.strokeStyle = used ? T.halo : col;
      if (!used) ctx.setLineDash([2, 2]);
      ctx.stroke(); ctx.setLineDash([]);
      // three bars: a count-station glyph
      ctx.fillStyle = used ? '#fff' : col;
      const bw = s * 0.28;
      [[-0.55, 0.35], [-0.14, 0.7], [0.27, 0.5]].forEach(([dx, hgt]) => ctx.fillRect(x + dx * s, y + s * 0.5 - hgt * s, bw, hgt * s));
      if (z >= 16 && View.show.labels) {
        const pv = DataHub.periodVol(c, period);
        if (pv) label(ctx, row ? `${U.fmt.int(pv.v)} | ${U.fmt.int(row.model)}` : U.fmt.int(pv.v), x, y - s - 8, { font: '600 10.5px "IBM Plex Mono", monospace', color: row ? GEH_COL(row.geh) : T.ink });
      }
    }
  }

  function laneMarkings(ctx, o, off, g) {
    const Lk = o.L, lw = (CLASSES[Lk.cls] || CLASSES.local).lw / g.mpp, total = (Lk.lanesAB | 0) + (Lk.lanesBA | 0);
    ctx.lineWidth = Math.max(1, 0.12 / g.mpp * 1.2); ctx.strokeStyle = 'rgba(255,255,255,.85)';
    // carriageway centre line sits between the A→B lanes (on the left of A→B) and the B→A lanes
    const half = total * lw / 2;
    for (let i = 1; i < total; i++) {
      const d = half - i * lw;            // offset from carriageway centre toward the left of A→B
      const isCentre = i === (Lk.lanesAB | 0) && (Lk.lanesBA | 0) > 0;
      ctx.setLineDash(isCentre ? [] : [3 / g.mpp, 9 / g.mpp]);
      ctx.strokeStyle = isCentre ? 'rgba(255,255,255,.95)' : 'rgba(255,255,255,.75)';
      pathOffset(ctx, o.xy, off, d); ctx.stroke();
    }
    ctx.setLineDash([]);
  }

  function nodeSymbol(ctx, type, x, y, z) {
    const s = z >= 16 ? 1.25 : 1;
    ctx.lineWidth = 1.6;
    if (type === 'signals') {
      ctx.fillStyle = '#20262a'; ctx.strokeStyle = T.halo;
      roundRect(ctx, x - 4.5 * s, y - 9 * s, 9 * s, 18 * s, 2.5 * s); ctx.fill(); ctx.stroke();
      [['#f03e3e', -5], ['#fab005', 0], ['#37b24d', 5]].forEach(([c, dy]) => { ctx.beginPath(); ctx.arc(x, y + dy * s, 2 * s, 0, 7); ctx.fillStyle = c; ctx.fill(); });
    } else if (type === 'roundabout') {
      ctx.beginPath(); ctx.arc(x, y, 7 * s, 0, 7); ctx.fillStyle = T.panel; ctx.fill(); ctx.lineWidth = 2.6 * s; ctx.strokeStyle = '#1c7ed6'; ctx.stroke();
      ctx.beginPath(); ctx.arc(x, y, 2.5 * s, 0, 7); ctx.fillStyle = '#1c7ed6'; ctx.fill();
    } else if (type === 'stop') {
      ctx.beginPath();
      for (let i = 0; i < 8; i++) { const a = Math.PI / 8 + i * Math.PI / 4; const px = x + Math.cos(a) * 7 * s, py = y + Math.sin(a) * 7 * s; i ? ctx.lineTo(px, py) : ctx.moveTo(px, py); }
      ctx.closePath(); ctx.fillStyle = '#c92a2a'; ctx.fill(); ctx.strokeStyle = T.halo; ctx.stroke();
    } else if (type === 'priority') {
      ctx.beginPath(); ctx.moveTo(x - 6 * s, y - 4.5 * s); ctx.lineTo(x + 6 * s, y - 4.5 * s); ctx.lineTo(x, y + 6 * s); ctx.closePath();
      ctx.fillStyle = '#fff'; ctx.fill(); ctx.strokeStyle = '#c92a2a'; ctx.lineWidth = 2 * s; ctx.stroke();
    }
  }
  function roundRect(ctx, x, y, w, h, r) {
    ctx.beginPath(); ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
  }

  /* ---------- live overlay ---------- */
  function drawFx(ctx, off, size) {
    if (!Store.P) return;
    const g = geo(), z = g.z;
    ctx.lineCap = 'round'; ctx.lineJoin = 'round';
    const P = ll => { const q = map.project(ll, z); return [q.x - off.x, q.y - off.y]; };
    // highlight hover and selection
    for (const [obj, col, alpha] of [[View.hover, T.sel, 0.28], [View.sel, T.sel, 0.5]]) {
      if (!obj) continue;
      if (obj.type === 'link') {
        const o = g.byId.get(obj.id); if (!o) continue;
        ctx.strokeStyle = col; ctx.globalAlpha = alpha; ctx.lineWidth = linkWidth(o.L, g) + 10;
        pathOffset(ctx, o.xy, off, 0); ctx.stroke(); ctx.globalAlpha = 1;
      } else if (obj.type === 'node' || obj.type === 'zone') {
        const p = obj.type === 'node' ? g.nodes.get(obj.id) : g.zones.get(obj.id); if (!p) continue;
        ctx.beginPath(); ctx.arc(p.x - off.x, p.y - off.y, 13, 0, 7); ctx.lineWidth = 3; ctx.strokeStyle = col; ctx.globalAlpha = alpha + 0.3; ctx.stroke(); ctx.globalAlpha = 1;
      } else if (obj.type === 'count') {
        const c = DataHub.D().counts.find(x => x.id === obj.id); if (!c) continue;
        const p = map.project([c.lat, c.lng], z);
        ctx.beginPath(); ctx.arc(p.x - off.x, p.y - off.y, 13, 0, 7); ctx.lineWidth = 3; ctx.strokeStyle = col; ctx.globalAlpha = alpha + 0.3; ctx.stroke(); ctx.globalAlpha = 1;
        const mt = DataHub.matches().get(c.id), o = mt && g.byId.get(mt.link);
        if (o && obj === View.sel) { ctx.strokeStyle = col; ctx.globalAlpha = 0.35; ctx.lineWidth = linkWidth(o.L, g) + 8; pathOffset(ctx, o.xy, off, 0); ctx.stroke(); ctx.globalAlpha = 1; }
      } else if (obj.type === 'route') {
        const xy = g.routePx[obj.id]; if (!xy) continue;
        ctx.strokeStyle = col; ctx.globalAlpha = alpha; ctx.lineWidth = 10; pathOffset(ctx, xy, off, z >= 15 ? 9 : 5); ctx.stroke(); ctx.globalAlpha = 1;
      }
    }
    // A→B travel path
    if (View.path && View.path.pts) {
      ctx.strokeStyle = T.halo; ctx.lineWidth = 9; ctx.beginPath(); View.path.pts.forEach((ll, i) => { const [x, y] = P(ll); i ? ctx.lineTo(x, y) : ctx.moveTo(x, y); }); ctx.stroke();
      ctx.strokeStyle = T.sel; ctx.lineWidth = 5; ctx.stroke();
    }
    if (View.pathEnds) for (const [ll, t] of View.pathEnds) { const [x, y] = P(ll); pin(ctx, x, y, t); }
    // drafts (road / bus / measure)
    const draft = View.draft;
    if (draft && draft.pts.length) {
      const pts = draft.pts.map(p => P(p.ll));
      if (View.cursor && draft.kind !== 'done') pts.push(P(View.snap ? View.snap.ll : [View.cursor.lat, View.cursor.lng]));
      ctx.strokeStyle = draft.kind === 'bus' ? '#7048e8' : T.amber; ctx.lineWidth = draft.kind === 'bus' ? 4 : Math.max(4, (View.template ? View.template.lanesAB + View.template.lanesBA : 2) * 3.4 / g.mpp);
      ctx.globalAlpha = 0.75; ctx.setLineDash([8, 6]);
      ctx.beginPath(); pts.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.stroke(); ctx.setLineDash([]); ctx.globalAlpha = 1;
      for (const p of draft.pts) { const [x, y] = P(p.ll); ctx.beginPath(); ctx.arc(x, y, p.type === 'free' ? 3.5 : 5, 0, 7); ctx.fillStyle = p.type === 'free' ? T.panel : T.amber; ctx.fill(); ctx.lineWidth = 1.5; ctx.strokeStyle = T.ink; ctx.stroke(); }
      if (draft.kind === 'road' && pts.length >= 2) {
        let len = 0; const ll = draft.pts.map(p => p.ll); if (View.cursor) ll.push(View.snap ? View.snap.ll : [View.cursor.lat, View.cursor.lng]);
        for (let i = 1; i < ll.length; i++) len += U.dist(ll[i - 1], ll[i]);
        const [x, y] = pts[pts.length - 1];
        label(ctx, U.fmt.len(len), x + 14, y - 14, { align: 'left' });
      }
    }
    if (View.measure && View.measure.length) {
      const ll = [...View.measure]; if (View.cursor && View.tool === 'measure') ll.push([View.cursor.lat, View.cursor.lng]);
      const pts = ll.map(P);
      ctx.strokeStyle = T.ink; ctx.lineWidth = 2; ctx.setLineDash([6, 4]);
      ctx.beginPath(); pts.forEach(([x, y], i) => i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)); ctx.stroke(); ctx.setLineDash([]);
      let d = 0; for (let i = 1; i < ll.length; i++) d += U.dist(ll[i - 1], ll[i]);
      pts.forEach(([x, y]) => { ctx.beginPath(); ctx.arc(x, y, 3.5, 0, 7); ctx.fillStyle = T.panel; ctx.fill(); ctx.strokeStyle = T.ink; ctx.lineWidth = 1.5; ctx.stroke(); });
      const [lx, ly] = pts[pts.length - 1];
      label(ctx, U.fmt.len(d), lx + 12, ly - 12, { align: 'left', font: '600 12px "IBM Plex Mono", monospace' });
    }
    // snap indicator
    if (View.snap && (View.tool === 'road' || View.tool === 'bus' || View.tool === 'gateway') && View.snap.type !== 'free') {
      const [x, y] = P(View.snap.ll);
      ctx.beginPath(); ctx.arc(x, y, 8, 0, 7); ctx.lineWidth = 2.5; ctx.strokeStyle = T.sel; ctx.stroke();
    }
    // the active registered tool's own overlay (e.g. a rectangle being dragged)
    const rt = Registry.get('tools', View.tool);
    if (rt && rt.draw) { ctx.save(); try { rt.draw(ctx, off, size, g, z); } catch (e) { reportOnce('tool ' + rt.id + '.draw', e); } ctx.restore(); }
    if (View.sim) drawSim(ctx, off, size, g);
  }

  function pin(ctx, x, y, t) {
    ctx.beginPath(); ctx.arc(x, y - 14, 9, Math.PI * 0.8, Math.PI * 2.2); ctx.lineTo(x, y); ctx.closePath();
    ctx.fillStyle = T.sel; ctx.fill(); ctx.lineWidth = 2; ctx.strokeStyle = T.halo; ctx.stroke();
    ctx.fillStyle = '#fff'; ctx.font = '700 10px Barlow, sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(t, x, y - 14);
  }

  // Simulated vehicles and signal heads
  function drawSim(ctx, off, size, g) {
    const sim = View.sim, z = g.z, mpp = g.mpp, pxm = 1 / mpp;
    if (z < 14) return;
    if (!sim._px || sim._pxz !== z || sim._pxv !== gv) {
      sim._px = sim.G.arcs.map(a => a.conn ? null : a.pts.map(p => map.project(p, z)));
      sim._pxz = z; sim._pxv = gv;
    }
    const colourBy = View.simColour || 'speed';
    const drawV = (v, A, li, nl) => {
      const px = sim._px[A.i]; if (!px) return;
      const s = U.clamp(v.s - v.len / 2, 0, A.len);
      // locate segment
      const cum = A.cum; let lo = 0, hi = cum.length - 1;
      while (hi - lo > 1) { const m = (lo + hi) >> 1; if (cum[m] <= s) lo = m; else hi = m; }
      const f = (s - cum[lo]) / ((cum[lo + 1] - cum[lo]) || 1);
      const a = px[lo], b = px[lo + 1] || a;
      let dx = b.x - a.x, dy = b.y - a.y; const l = Math.hypot(dx, dy) || 1; dx /= l; dy /= l;
      const lw = A.lw || 3.3;
      const lat = A.two ? (nl - li - 0.5) * lw : (nl - li - 0.5) * lw - nl * lw / 2;
      const x = a.x + (b.x - a.x) * f - off.x + dy * lat * pxm, y = a.y + (b.y - a.y) * f - off.y - dx * lat * pxm;
      if (x < -20 || y < -20 || x > size.x + 20 || y > size.y + 20) return;
      let col;
      if (colourBy === 'type') col = v.type === 'bus' ? (v.r.color || '#7048e8') : v.type === 'hv' ? '#ae3ec9' : (T.dark ? '#e9ecef' : '#343a40');
      else { const ratio = v.v / (A.speed / 3.6); col = v.type === 'bus' ? (v.r.color || '#7048e8') : ratio < 0.15 ? '#e03131' : ratio < 0.45 ? '#f76707' : ratio < 0.75 ? '#fab005' : '#2f9e44'; }
      ctx.fillStyle = col;
      const Lp = Math.max(v.len * pxm, v.type === 'car' ? 5 : 9), Wp = Math.max((v.type === 'car' ? 1.9 : 2.5) * pxm, v.type === 'car' ? 3 : 3.6);
      if (z < 15) { ctx.beginPath(); ctx.arc(x, y, 2.2, 0, 7); ctx.fill(); ctx.stroke(); return; }
      const hx = dx * Lp / 2, hy = dy * Lp / 2, wx = dy * Wp / 2, wy = -dx * Wp / 2;
      ctx.beginPath(); ctx.moveTo(x + hx + wx, y + hy + wy); ctx.lineTo(x + hx - wx, y + hy - wy); ctx.lineTo(x - hx - wx, y - hy - wy); ctx.lineTo(x - hx + wx, y - hy + wy); ctx.closePath(); ctx.fill(); ctx.stroke();
    };
    ctx.lineWidth = 0.9; ctx.strokeStyle = T.dark ? 'rgba(0,0,0,.85)' : 'rgba(20,24,23,.9)';
    sim.forEachVehicle(drawV);
    if (z >= 15) {
      for (const ctl of sim.G.ctl.values()) {
        if (ctl.type !== 'signals') continue;
        for (const ai of ctl.arcs) {
          const A = sim.G.arcs[ai], px = sim._px[ai]; if (!px) continue;
          const b = px[px.length - 1], a = px[px.length - 2];
          let dx = b.x - a.x, dy = b.y - a.y; const l = Math.hypot(dx, dy) || 1; dx /= l; dy /= l;
          const lat = (A.two ? A.lanes * A.lw : A.lanes * A.lw / 2) * pxm + 4;
          const back = Math.min(12 * pxm, 14);
          const x = b.x - off.x - dx * back + dy * lat, y = b.y - off.y - dy * back - dx * lat;
          const st = sim.signal(A);
          ctx.beginPath(); ctx.arc(x, y, z >= 17 ? 5 : 3.5, 0, 7);
          ctx.fillStyle = st === 'G' ? '#2f9e44' : st === 'A' ? '#fab005' : '#e03131'; ctx.fill();
          ctx.lineWidth = 1.5; ctx.strokeStyle = '#111'; ctx.stroke();
        }
      }
    }
  }

  /* ---------- hit testing ---------- */
  // opt.layers (default: same as opt.counts, i.e. the select tool) also asks the visible registered layers
  function hit(cp, opt = {}) {
    const g = geo(), w = map.containerPointToLayerPoint(cp).add(map.getPixelOrigin()), X = w.x, Y = w.y;
    let best = null;
    if (opt.counts && View.show.counts && g.z >= 12) for (const c of DataHub.D().counts) {
      const p = map.project([c.lat, c.lng], g.z), d = Math.hypot(p.x - X, p.y - Y);
      if (d < 9 && (!best || d < best.d)) best = { type: 'count', id: c.id, d };
    }
    if (best) return best;
    if (opt.layers ?? opt.counts) for (const l of Registry.list('layers', l => l.hit && View.show[l.id] && !(l.minZoom > g.z))) {
      const h = Registry.call(l, 'hit', X, Y, g);
      if (h && h.type && h.id != null && (!best || (h.d ?? 0) < best.d)) best = { d: 0, ...h };
    }
    if (best) return best;
    if (opt.zones !== false && View.show.zones) for (const [id, p] of g.zones) {
      const d = Math.hypot(p.x - X, p.y - Y);
      if (d < 11 && (!best || d < best.d)) best = { type: 'zone', id, d };
    }
    if (best) return best;
    const nodeR = opt.nodeR ?? (g.z >= 14 ? 9 : 0);
    if (opt.nodes !== false && nodeR > 0) for (const [id, p] of g.nodes) {
      const d = Math.hypot(p.x - X, p.y - Y);
      if (d < nodeR && (!best || d < best.d)) best = { type: 'node', id, d, ll: [Store.sc.nodes[id].lat, Store.sc.nodes[id].lng] };
    }
    if (best) return best;
    if (opt.routes && View.show.routes) for (const [id, xy] of Object.entries(g.routePx || {})) {
      for (let i = 0; i + 3 < xy.length; i += 2) {
        const r = U.segDist(X, Y, xy[i], xy[i + 1], xy[i + 2], xy[i + 3]);
        if (r.d < 6 && (!best || r.d < best.d)) best = { type: 'route', id, d: r.d };
      }
    }
    if (best && best.d < 4) return best;
    if (opt.links !== false) for (const o of g.links) {
      if (X < o.minx - 14 || X > o.maxx + 14 || Y < o.miny - 14 || Y > o.maxy + 14) continue;
      if (g.z < 12 && o.rank <= 2 && o.L.status !== 'proposed') continue;
      const tol = Math.max(opt.linkTol || 0, linkWidth(o.L, g) / 2 + 5);
      for (let i = 0; i + 3 < o.xy.length; i += 2) {
        const r = U.segDist(X, Y, o.xy[i], o.xy[i + 1], o.xy[i + 2], o.xy[i + 3]);
        if (r.d < tol && (!best || r.d < best.d)) { const q = map.unproject([r.x, r.y], g.z); best = { type: 'link', id: o.L.id, d: r.d, ll: [q.lat, q.lng] }; }
      }
    }
    return best;
  }

  /* ---------- legend ---------- */
  // o = { sc, period, res, mode } and, for a figure, snapshotState's baseSc, baseRes, compare, labels and labelRound
  function legend(o = {}) {
    const sc = o.sc || Store.sc, period = o.period || Store.S.period, res = o.res !== undefined ? o.res : Store.res(sc, period);
    const baseSc = o.baseSc || Store.baseSc, md = modeDef(o.mode || View.colorBy);
    const base = o.baseRes !== undefined ? o.baseRes : Store.res(baseSc, period);
    const lc = { z: map ? map.getZoom() : 15, sc, res, base, baseSc, period, T, S: Store.S, compare: !!o.compare, labelRound: +o.labelRound > 1 ? +o.labelRound : 1 };
    let h = md.legend ? Registry.html(md, 'legend', res, lc) : `<div class="lg-title">${U.esc(md.label)}</div>`;
    if (!md.builtin && md.needsResults && !res) h += `<div class="lg-note">Run the model to see results.</div>`;
    if (o.labels && md.flowLabels && lc.labelRound > 1 && res && !o.plain) h += `<div class="lg-note">Labels: veh/h, rounded to the nearest ${U.fmt.int(lc.labelRound)}.</div>`;
    const d = DataHub.D();
    if (View.show.counts && d.counts.length) {
      const cal = DataHub.calibration(sc, period);
      h += cal && cal.n ? `<div class="lg-title" style="margin-top:10px">Count sites · model fit (${gl('GEH')})</div>${dot('#2f9e44', 'Under 5 · good')}${dot('#f08c00', '5–10 · check')}${dot('#e03131', 'Over 10 · poor')}<div class="lg-note">Label: counted | modelled veh/h. Dashed = not used.</div>`
        : `<div class="lg-title" style="margin-top:10px">Count sites</div>${Object.entries(DataHub.SOURCES).filter(([k]) => d.counts.some(c => c.src === k)).map(([, s]) => dot(s.colour, s.label)).join('')}`;
    }
    // the CAS window loaded: '2021–2025, plus 2026 to date' (never 'the last 5 years', which is 6 calendar years)
    const casYears = U.esc(Kit.casShort());
    if (View.show.crashes && d.crashes.length) h += `<div class="lg-title" style="margin-top:10px">Crashes, ${casYears}</div><div class="lg-row" style="gap:10px;flex-wrap:wrap">${[['F', 'Fatal'], ['S', 'Serious'], ['M', 'Minor'], ['N', 'Non-injury']].map(([k, l]) => `<span class="lg-row" style="margin:0;gap:4px"><span class="lg-dot" style="width:9px;height:9px;background:${SEV_COL[k]}"></span>${l}</span>`).join('')}</div>`;
    // Zone symbols by class: existing land use (hollow), committed development (amber), proposed development (accent)
    if (View.show.zones && Engine.zoneClass) {
      const cl = new Set(Object.values(sc.zones).filter(z => z.kind !== 'gateway').map(z => Engine.zoneClass(z)));
      const sq = (bg, bd) => `<span class="lg-sw lg-sq" style="background:${bg};border-color:${bd}"></span>`;
      const row = (k, s1) => cl.has(k) ? `<div class="lg-row">${s1}<span>${ZCLASS_LABEL[k]}</span></div>` : '';
      if (cl.size) h += `<div class="lg-title" style="margin-top:10px">Zones</div>${row('existing', sq(T.panel, T.muted))}${row('committed', sq(T.amber, T.amber))}${row('development', sq(T.accent, T.accent))}`;
    }
    for (const l of Registry.list('layers', l => l.legend && View.show[l.id])) { const lh = Registry.html(l, 'legend', lc); if (lh) h += `<div class="lg-layer" style="margin-top:10px">${lh}</div>`; }
    return h;
  }

  /* ---------- figure snapshot for reports ----------
     snapshot({ bounds, width = 800, height = 560, mode, scenarioId, period, basemap = true, scale = 2, theme = 'light',
                title, footer, legend = true, padding = 24, show: { counts: false, … },
                baseScenarioId, labels, labelRound = 10 }) → Promise<PNG data URL>.
     baseScenarioId: compare with that scenario (diff, ctx.base, ctx.baseSc) instead of the project's base; see snapshotState.
     labels: true | false; flow-label modes (volume, diff, dev, select) default to true, drawn from zoom 12 and kept apart.
     labelRound: flow labels round to this many veh/h (1 = whole veh/h, as on the live map).
     Tiles are fetched with crossOrigin 'anonymous'; a tile server that refuses CORS leaves its tiles out (never taints the
     canvas) and the attribution says so. Render.lastSnapshot holds { z, tiles, missing, basemap, attribution }. */
  const loadImg = (url, ms) => new Promise(resolve => {
    const im = new Image(); let done = false;
    const fin = ok => { if (done) return; done = true; resolve(ok ? im : null); };
    im.crossOrigin = 'anonymous'; im.decoding = 'async';
    im.onload = () => fin(true); im.onerror = () => fin(false);
    setTimeout(() => fin(false), ms);
    im.src = url;
  });
  function tileUrl(tpl, z, x, y, key) { return tpl.replace('{z}', z).replace('{x}', x).replace('{y}', y).replace('{s}', 'abc'[(x + y) % 3]).replace('{key}', encodeURIComponent(key || '')); }
  const stripTags = s => String(s || '').replace(/<[^>]*>/g, '');
  const cssVar = s => String(s || '').replace(/var\((--[\w-]+)\)/g, (m, v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim() || m);

  async function snapshot(o = {}) {
    if (!map) throw new Error('Render.snapshot needs the map. Call it after the app has started (UI.init).');
    const st = snapshotState(o), sc = st.sc, period = st.period;
    const W = Math.round(o.width || 800), H = Math.round(o.height || 560), k = o.scale || 2, pad = o.padding ?? 24;
    let b = o.bounds ? L.latLngBounds(o.bounds) : null;
    if (!b || !b.isValid()) { const pts = Object.values(sc.nodes).map(n => [n.lat, n.lng]); b = pts.length ? L.latLngBounds(pts) : map.getBounds(); }
    // the largest (fractional) zoom at which the bounds fit inside the figure
    const a0 = map.project(b.getNorthWest(), 0), b0 = map.project(b.getSouthEast(), 0);
    const fit = Math.min((W - 2 * pad) / Math.max(1e-9, Math.abs(b0.x - a0.x)), (H - 2 * pad) / Math.max(1e-9, Math.abs(b0.y - a0.y)));
    const z = U.clamp(Math.log2(fit), 3, o.maxZoom ?? 18);
    const c = map.project(b.getCenter(), z), off = L.point(c.x - W / 2, c.y - H / 2);
    const theme = o.theme || 'light';
    // basemap: true = the current one (dark grey becomes light grey on a light figure), a BASEMAPS id, or false
    const BM = (typeof UI !== 'undefined' && UI.BASEMAPS) || {};
    let bmId = o.basemap === false ? 'none' : typeof o.basemap === 'string' ? o.basemap : (Store.S.basemap || 'light');
    if (o.basemap === true || o.basemap === undefined) { if (theme === 'light' && bmId === 'dark') bmId = 'light'; if (theme === 'dark' && bmId === 'light') bmId = 'dark'; }
    const bm = BM[bmId] && BM[bmId].url && !(BM[bmId].key && !Kit.linzKey()) ? BM[bmId] : null;
    // load tiles at the integer zoom below z, scaled up to fit
    const info = { z, tiles: 0, missing: 0, basemap: bm ? bmId : 'none', attribution: '' };
    const layers = [];
    if (bm) {
      let tz = Math.min(Math.floor(z), bm.max || 19), f = Math.pow(2, z - tz);
      let x0 = Math.floor(off.x / f / 256), x1 = Math.floor((off.x + W) / f / 256), y0 = Math.floor(off.y / f / 256), y1 = Math.floor((off.y + H) / f / 256);
      while ((x1 - x0 + 1) * (y1 - y0 + 1) > 120 && tz > 3) { tz--; f = Math.pow(2, z - tz); x0 = Math.floor(off.x / f / 256); x1 = Math.floor((off.x + W) / f / 256); y0 = Math.floor(off.y / f / 256); y1 = Math.floor((off.y + H) / f / 256); }
      const n = Math.pow(2, tz), jobs = [];
      for (const tpl of [bm.url, bm.labels].filter(Boolean)) {
        const list = [];
        for (let x = x0; x <= x1; x++) for (let y = y0; y <= y1; y++) {
          if (y < 0 || y >= n) continue;
          const xx = ((x % n) + n) % n;
          list.push({ x, y, i: jobs.length });
          jobs.push(loadImg(tileUrl(tpl, tz, xx, y, Kit.linzKey()), o.timeout || 10000));
        }
        layers.push(list);
      }
      const imgs = await Promise.all(jobs);
      for (const list of layers) for (const t of list) t.img = imgs[t.i];
      info.tiles = imgs.length; info.missing = imgs.filter(i => !i).length;
      info.tz = tz; info.f = f;
    }
    // everything below is synchronous, so the theme switch never reaches the screen
    const root = document.documentElement, prevTheme = root.dataset.theme;
    root.dataset.theme = theme; readTheme();
    let url;
    try {
      const cv = document.createElement('canvas');
      cv.width = Math.round(W * k); cv.height = Math.round(H * k);
      const ctx = cv.getContext('2d');
      ctx.scale(k, k);
      ctx.fillStyle = T.bg || '#e8ebe9'; ctx.fillRect(0, 0, W, H);
      if (bm) {
        const sz = 256 * info.f;
        for (const list of layers) for (const t of list) if (t.img) ctx.drawImage(t.img, t.x * sz - off.x, t.y * sz - off.y, sz + 0.5, sz + 0.5);
      }
      // the network, with the requested scenario, period and colour mode
      const show = View.show, saved = { ...show };
      if (o.show) Object.assign(show, o.show);
      try {
        const g = buildGeo(sc, z, b.getCenter().lat, {}, 'snap');
        ctx.save();
        drawNet(ctx, off, { x: W, y: H }, { ...st, g });
        ctx.restore();
        // furniture: title, legend, north arrow, scale bar, attribution, footer
        let top = 10;
        if (o.title) { top = drawTitle(ctx, o.title, W) + 8; }
        if (o.legend !== false) drawLegendBox(ctx, legend(st), 10, top, Math.min(220, W * 0.36), H - top - 60);
        northArrow(ctx, W - 30, top + 22);
        const mpp = 156543.03392 * Math.cos(b.getCenter().lat * Math.PI / 180) / Math.pow(2, z);
        scaleBar(ctx, 12, H - 34, mpp, Math.min(160, W * 0.3));
        const attr = [bm ? stripTags(bm.attr) : '', 'Road network © OpenStreetMap contributors (ODbL)'];
        if (show.crashes && DataHub.D().crashes.length) attr.push('Crash data: NZTA CAS (CC BY 4.0)');
        if (show.counts && DataHub.D().counts.length) attr.push('Counts: NZTA (CC BY 4.0), Auckland Transport open data');
        if (bm && info.missing) attr.push(info.missing === info.tiles ? 'Basemap not shown: the tile server did not allow it' : `${info.missing} basemap tiles missing`);
        if (o.attribution) attr.push(o.attribution);
        info.attribution = attr.filter(Boolean).join(' · ');
        footerText(ctx, info.attribution, o.footer ?? 'Spare Construct sketch model, indicative only', W, H);
      } finally { Object.keys(show).forEach(kk => { if (!(kk in saved)) delete show[kk]; }); Object.assign(show, saved); }
      url = cv.toDataURL('image/png');
    } finally {
      if (prevTheme === undefined) delete root.dataset.theme; else root.dataset.theme = prevTheme;
      readTheme();
    }
    lastSnap = info;
    return url;
  }
  function drawTitle(ctx, text, W) {
    ctx.font = '600 15px Barlow, sans-serif'; ctx.textAlign = 'left'; ctx.textBaseline = 'top';
    const w = Math.min(W - 70, ctx.measureText(text).width + 20);
    ctx.fillStyle = T.panel; ctx.globalAlpha = 0.92; roundRect(ctx, 10, 10, w, 28, 5); ctx.fill(); ctx.globalAlpha = 1;
    ctx.fillStyle = T.ink; ctx.fillText(text, 20, 16, W - 90);
    return 38;
  }
  function wrapText(ctx, text, maxW) {
    const words = String(text).split(/\s+/), lines = []; let cur = '';
    for (const w of words) { const t = cur ? cur + ' ' + w : w; if (ctx.measureText(t).width > maxW && cur) { lines.push(cur); cur = w; } else cur = t; }
    if (cur) lines.push(cur);
    return lines;
  }
  // Draws the HTML legend (lg-title / lg-row with lg-sw or lg-dot / lg-note) onto the canvas
  function drawLegendBox(ctx, html, x, y, w, maxH) {
    if (!html) return;
    const doc = new DOMParser().parseFromString(`<div>${html}</div>`, 'text/html'), rows = [];
    const walk = el => {
      for (const ch of el.children) {
        const cl = ch.classList;
        if (cl.contains('lg-title')) rows.push({ t: 'title', text: ch.textContent.trim() });
        else if (cl.contains('lg-note')) rows.push({ t: 'note', text: ch.textContent.trim() });
        else if (cl.contains('lg-row')) {
          const inner = [...ch.querySelectorAll(':scope > .lg-row')];
          const items = (inner.length ? inner : [ch]).map(r => { const s = r.querySelector('.lg-sw, .lg-dot'); return { dot: !!(s && s.classList.contains('lg-dot')), dash: !!(s && s.classList.contains('dash')), c: s ? cssVar(s.style.background) : '', text: r.textContent.trim() }; });
          rows.push({ t: 'row', items });
        } else walk(ch);
      }
    };
    walk(doc.body.firstChild);
    const lay = [];
    let h = 10;
    for (const r of rows) {
      if (r.t === 'title') { ctx.font = '600 10px "Barlow Condensed", Barlow, sans-serif'; h += lay.length ? 6 : 0; lay.push({ ...r, y: h }); h += 14; }
      else if (r.t === 'note') { ctx.font = '400 10px Barlow, sans-serif'; const L = wrapText(ctx, r.text, w - 20); lay.push({ ...r, lines: L, y: h }); h += L.length * 12 + 2; }
      else { lay.push({ ...r, y: h }); h += 16; }
      if (h > maxH) break;
    }
    h += 6;
    ctx.save();
    ctx.fillStyle = T.panel; ctx.globalAlpha = 0.94; roundRect(ctx, x, y, w, h, 6); ctx.fill(); ctx.globalAlpha = 1;
    ctx.strokeStyle = T.line; ctx.lineWidth = 1; ctx.stroke();
    ctx.textBaseline = 'middle'; ctx.textAlign = 'left';
    for (const r of lay) {
      const yy = y + r.y + 6;
      if (r.t === 'title') { ctx.font = '600 10px "Barlow Condensed", Barlow, sans-serif'; ctx.fillStyle = T.muted; ctx.fillText(r.text.toUpperCase(), x + 10, yy); }
      else if (r.t === 'note') { ctx.font = '400 10px Barlow, sans-serif'; ctx.fillStyle = T.muted; r.lines.forEach((l, i) => ctx.fillText(l, x + 10, yy + i * 12)); }
      else {
        let xx = x + 10;
        for (const it of r.items) {
          if (it.dash) { ctx.strokeStyle = T.amber; ctx.lineWidth = 4; ctx.setLineDash([5, 3]); ctx.beginPath(); ctx.moveTo(xx, yy); ctx.lineTo(xx + 22, yy); ctx.stroke(); ctx.setLineDash([]); }
          else if (it.dot) { ctx.fillStyle = it.c || T.faint; ctx.beginPath(); ctx.arc(xx + 6, yy, 5, 0, 7); ctx.fill(); }
          else if (it.c) { ctx.fillStyle = it.c; roundRect(ctx, xx, yy - 3, 22, 6, 3); ctx.fill(); }
          ctx.font = '400 11px Barlow, sans-serif'; ctx.fillStyle = T.ink;
          const tx = xx + (it.dot ? 16 : 30);
          ctx.fillText(it.text, tx, yy, x + w - tx - 6);
          xx = tx + ctx.measureText(it.text).width + 10;
        }
      }
    }
    ctx.restore();
  }
  function northArrow(ctx, x, y) {
    ctx.save();
    ctx.fillStyle = T.panel; ctx.globalAlpha = 0.92; ctx.beginPath(); ctx.arc(x, y, 17, 0, 7); ctx.fill(); ctx.globalAlpha = 1;
    ctx.strokeStyle = T.line; ctx.lineWidth = 1; ctx.stroke();
    ctx.fillStyle = T.ink; ctx.beginPath(); ctx.moveTo(x, y - 12); ctx.lineTo(x + 6, y + 6); ctx.lineTo(x, y + 2); ctx.lineTo(x - 6, y + 6); ctx.closePath(); ctx.fill();
    ctx.font = '700 9px Barlow, sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillStyle = T.panel; ctx.fillText('N', x, y - 1);
    ctx.restore();
  }
  function scaleBar(ctx, x, y, mpp, maxPx) {
    const maxM = mpp * maxPx, p = Math.pow(10, Math.floor(Math.log10(maxM)));
    const m = [5, 2, 1].map(f => f * p).find(v => v <= maxM) || p, px = m / mpp;
    ctx.save();
    ctx.fillStyle = T.panel; ctx.globalAlpha = 0.9; roundRect(ctx, x - 4, y - 14, px + 50, 22, 4); ctx.fill(); ctx.globalAlpha = 1;
    ctx.strokeStyle = T.ink; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(x, y - 6); ctx.lineTo(x, y); ctx.lineTo(x + px, y); ctx.lineTo(x + px, y - 6); ctx.stroke();
    ctx.font = '500 10px "IBM Plex Mono", monospace'; ctx.fillStyle = T.ink; ctx.textBaseline = 'alphabetic'; ctx.textAlign = 'left';
    ctx.fillText(m >= 1000 ? `${m / 1000} km` : `${m} m`, x + px + 6, y);
    ctx.restore();
  }
  function footerText(ctx, attr, foot, W, H) {
    ctx.save();
    ctx.font = '400 9.5px Barlow, sans-serif'; ctx.textBaseline = 'bottom';
    const lines = wrapText(ctx, attr, W * 0.62).slice(-3);
    const bw = Math.max(...lines.map(l => ctx.measureText(l).width)) + 12, bh = lines.length * 11 + 6;
    ctx.fillStyle = T.panel; ctx.globalAlpha = 0.88; ctx.fillRect(W - bw, H - bh, bw, bh); ctx.globalAlpha = 1;
    ctx.fillStyle = T.muted; ctx.textAlign = 'right';
    lines.forEach((l, i) => ctx.fillText(l, W - 6, H - 3 - (lines.length - 1 - i) * 11));
    if (foot) { ctx.textAlign = 'left'; ctx.font = '600 10px Barlow, sans-serif'; ctx.fillStyle = T.ink; ctx.fillText(foot, 12, H - 6); }
    ctx.restore();
  }

  function redrawNet() { if (netQueued) return; netQueued = true; requestAnimationFrame(() => { netQueued = false; net && net.redraw(); }); }
  function redrawFx() { if (fxQueued) return; fxQueued = true; requestAnimationFrame(() => { fxQueued = false; fx && fx.redraw(); }); }
  function redraw() { redrawNet(); redrawFx(); }
  return { init, redraw, redrawNet, redrawFx, geomChanged, hit, legend, readTheme, snapshot, snapshotState, modeCtx, modeDef,
    roundFlow, devShare, devShareBand, DEVSHARE_BANDS: DS_BANDS.map(([max, token, label]) => ({ max, token, label })), gatewayBadge, get zoom() { return map.getZoom(); }, get lastSnapshot() { return lastSnap; }, get theme() { return T; } };
})();
