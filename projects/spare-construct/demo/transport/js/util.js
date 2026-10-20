'use strict';
/* Spare Construct · shared helpers: ids, geodesy, priority queue, number formats, small DOM utilities. */
const U = (() => {
  let seq = 0;
  const uid = (p = '') => p + Date.now().toString(36).slice(-4) + (seq++).toString(36) + Math.random().toString(36).slice(2, 5);

  const R = 6371008.8, D2R = Math.PI / 180;
  function dist(a, b) {
    const dLat = (b[0] - a[0]) * D2R, dLng = (b[1] - a[1]) * D2R;
    const s = Math.sin(dLat / 2) ** 2 + Math.cos(a[0] * D2R) * Math.cos(b[0] * D2R) * Math.sin(dLng / 2) ** 2;
    return 2 * R * Math.asin(Math.min(1, Math.sqrt(s)));
  }
  function cumLen(pts) {
    const c = [0];
    for (let i = 1; i < pts.length; i++) c.push(c[i - 1] + dist(pts[i - 1], pts[i]));
    return c;
  }
  const polyLen = pts => { const c = cumLen(pts); return c[c.length - 1]; };
  function bearing(a, b) {
    const p1 = a[0] * D2R, p2 = b[0] * D2R, dl = (b[1] - a[1]) * D2R;
    const y = Math.sin(dl) * Math.cos(p2), x = Math.cos(p1) * Math.sin(p2) - Math.sin(p1) * Math.cos(p2) * Math.cos(dl);
    return (Math.atan2(y, x) / D2R + 360) % 360;
  }
  const angDiff = (a, b) => { const d = Math.abs(a - b) % 360; return d > 180 ? 360 - d : d; };
  // Signed turn from heading a to heading b, degrees, positive = clockwise (a right turn)
  const turn = (a, b) => ((b - a + 540) % 360) - 180;
  // Flat projection in metres around a reference latitude (accurate to ~0.1% across a few km)
  function flat(lat0) {
    const kx = 111320 * Math.cos(lat0 * D2R), ky = 110574;
    return { x: ll => ll[1] * kx, y: ll => ll[0] * ky, kx, ky };
  }
  // Point at distance s along a polyline with cumulative lengths cum
  function along(pts, cum, s) {
    const n = pts.length, L = cum[n - 1];
    if (s <= 0) return { p: pts[0], i: 0, f: 0 };
    if (s >= L) return { p: pts[n - 1], i: n - 2, f: 1 };
    let lo = 0, hi = n - 1;
    while (hi - lo > 1) { const m = (lo + hi) >> 1; if (cum[m] <= s) lo = m; else hi = m; }
    const f = (s - cum[lo]) / ((cum[lo + 1] - cum[lo]) || 1);
    return { p: [pts[lo][0] + (pts[lo + 1][0] - pts[lo][0]) * f, pts[lo][1] + (pts[lo + 1][1] - pts[lo][1]) * f], i: lo, f };
  }
  function segDist(px, py, ax, ay, bx, by) {
    const dx = bx - ax, dy = by - ay, l2 = dx * dx + dy * dy;
    let t = l2 ? ((px - ax) * dx + (py - ay) * dy) / l2 : 0;
    t = t < 0 ? 0 : t > 1 ? 1 : t;
    const x = ax + t * dx, y = ay + t * dy;
    return { d: Math.hypot(px - x, py - y), t, x, y };
  }

  // Binary min-heap of (number key, int value). Callers skip stale entries (lazy deletion).
  class Heap {
    constructor(cap = 1024) { this.k = new Float64Array(cap); this.v = new Int32Array(cap); this.n = 0; this.top = 0; }
    clear() { this.n = 0; }
    push(key, val) {
      if (this.n === this.k.length) {
        const k = new Float64Array(this.n * 2), v = new Int32Array(this.n * 2);
        k.set(this.k); v.set(this.v); this.k = k; this.v = v;
      }
      const K = this.k, V = this.v;
      let i = this.n++;
      while (i > 0) { const p = (i - 1) >> 1; if (K[p] <= key) break; K[i] = K[p]; V[i] = V[p]; i = p; }
      K[i] = key; V[i] = val;
    }
    pop() {
      const K = this.k, V = this.v, rv = V[0];
      this.top = K[0];
      const n = --this.n;
      if (n > 0) {
        const key = K[n], val = V[n];
        let i = 0;
        for (;;) {
          let c = 2 * i + 1;
          if (c >= n) break;
          if (c + 1 < n && K[c + 1] < K[c]) c++;
          if (K[c] >= key) break;
          K[i] = K[c]; V[i] = V[c]; i = c;
        }
        K[i] = key; V[i] = val;
      }
      return rv;
    }
  }

  const nfs = {};
  const num = (x, d = 0) => {
    if (x == null || !isFinite(x)) return '–';
    const f = nfs[d] || (nfs[d] = new Intl.NumberFormat('en-NZ', { minimumFractionDigits: d, maximumFractionDigits: d }));
    return f.format(x);
  };
  const fmt = {
    num,
    int: x => num(Math.round(x)),
    pct: (x, d = 0) => isFinite(x) ? num(x * 100, d) + '%' : '–',
    // values in NZ$ millions
    money: m => {
      if (m == null || !isFinite(m)) return '–';
      const a = Math.abs(m), s = m < 0 ? '−' : '';
      return a >= 1000 ? `${s}$${num(a / 1000, 2)}b` : a >= 10 ? `${s}$${num(a, 1)}m` : a >= 0.1 ? `${s}$${num(a, 2)}m` : `${s}$${num(a * 1000, 0)}k`;
    },
    dur: s => {
      if (s == null || !isFinite(s)) return '–';
      if (s < 90) return num(s, 0) + ' s';
      const m = s / 60;
      return m < 90 ? num(m, 1) + ' min' : num(m / 60, 1) + ' h';
    },
    len: m => m == null || !isFinite(m) ? '–' : m < 1000 ? num(m) + ' m' : num(m / 1000, 2) + ' km',
    signed: (x, d = 0) => { if (x == null || !isFinite(x)) return '–'; const r = +x.toFixed(d); return (r > 0 ? '+' : r < 0 ? '−' : '±') + num(Math.abs(r), d); },
    signedPct: (x, d = 1) => { if (x == null || !isFinite(x)) return '–'; const r = +(x * 100).toFixed(d); return (r > 0 ? '+' : r < 0 ? '−' : '±') + num(Math.abs(r), d) + '%'; },
    clock: s => {
      s = Math.max(0, Math.floor(s));
      const h = Math.floor(s / 3600) % 24, m = Math.floor(s / 60) % 60, ss = s % 60;
      return [h, m, ss].map(v => String(v).padStart(2, '0')).join(':');
    },
  };

  const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  function download(name, text, type = 'application/json') {
    const b = new Blob([text], { type });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(b); a.download = name;
    document.body.appendChild(a); a.click();
    setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
  }
  function debounce(fn, ms) { let t; return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); }; }
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  const clamp = (x, a, b) => x < a ? a : x > b ? b : x;
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const r7 = x => Math.round(x * 1e7) / 1e7;
  const r1 = x => Math.round(x * 10) / 10;
  const r3 = x => Math.round(x * 1000) / 1000;
  /* RFC 4180 CSV. A cell with a comma, quote, CR or LF (a bare CR too) is quoted. Text that a spreadsheet would read
     as a formula (starting with = + - @, a tab or a CR, and not a plain number) gets a leading apostrophe, so a crafted
     name in a shared project cannot run as a formula when the CSV is opened in Excel (CSV injection; the same rule as
     the report's data pack). Numbers, and text that is a number such as '-5' or '+1.5e3', are left as they are. */
  const NUMLIKE = /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i;
  function csvCell(v) {
    let s = v == null ? '' : String(v);
    if (typeof v === 'string' && /^[=+\-@\t\r]/.test(s) && !NUMLIKE.test(s.trim())) s = "'" + s;
    return /[",\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
  }
  function csv(rows) {
    return rows.map(r => r.map(csvCell).join(',')).join('\r\n');
  }
  function parseCSV(text) {
    const rows = []; let row = [], cur = '', q = false;
    for (let i = 0; i < text.length; i++) {
      const c = text[i];
      if (q) {
        if (c === '"' && text[i + 1] === '"') { cur += '"'; i++; } else if (c === '"') q = false; else cur += c;
      } else if (c === '"') q = true;
      else if (c === ',') { row.push(cur); cur = ''; }
      else if (c === '\n' || c === '\r') { if (c === '\r' && text[i + 1] === '\n') i++; row.push(cur); rows.push(row); row = []; cur = ''; }
      else cur += c;
    }
    if (cur !== '' || row.length) { row.push(cur); rows.push(row); }
    return rows.filter(r => r.some(v => v.trim() !== ''));
  }
  function deepMerge(base, over) {
    if (over == null || typeof over !== 'object' || Array.isArray(over)) return over === undefined ? base : over;
    const out = Array.isArray(base) ? [...base] : { ...base };
    for (const k of Object.keys(over)) {
      out[k] = (base && typeof base[k] === 'object' && base[k] !== null && !Array.isArray(base[k])) ? deepMerge(base[k], over[k]) : over[k];
    }
    return out;
  }

  return { uid, dist, cumLen, polyLen, bearing, angDiff, turn, flat, along, segDist, Heap, fmt, esc, download, debounce, $, $$, clamp, sleep, r7, r1, r3, csv, csvCell, parseCSV, deepMerge };
})();
