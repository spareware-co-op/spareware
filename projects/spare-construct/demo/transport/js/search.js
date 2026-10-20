'use strict';
/* Spare Construct · place search. Looks up New Zealand places and addresses with OpenStreetMap Nominatim
   (free, no key) and reads typed coordinates such as "-37.10, 174.95" without a request.
   Nominatim's usage policy forbids search-as-you-type, so call geocode on Enter or a button press,
   never while the user is typing. It also allows at most one request a second, so requests queue
   at least 1.1 s apart and answers are cached by query. Pass { signal } (an AbortSignal) to drop a
   queued request that a newer search has replaced: it is never sent. Common NZ abbreviations are
   expanded first ('Waihoehoe Rd' → 'Waihoehoe Road'). Results are at least about 500 m across. */

const Search = (() => {
  const NOMINATIM = 'https://nominatim.openstreetmap.org/search';
  const GAP = 1100, CACHE_MAX = 200, MIN_BOX = 250;
  const cache = new Map();          // normalised query → Promise of results
  let next = 0, queue = Promise.resolve();
  const aborted = () => Object.assign(new Error('The search was replaced by a newer one.'), { name: 'AbortError' });

  // Each request waits for its turn; the chain never rejects, so one failure cannot block the rest.
  // A request aborted while it waits gives up its turn without using the rate allowance.
  function turn(signal) {
    const t = queue.then(async () => {
      const w = next - Date.now();
      if (w > 0 && !(signal && signal.aborted)) await U.sleep(w);
      if (signal && signal.aborted) return false;
      next = Date.now() + GAP;
      return true;
    });
    queue = t.then(() => {}, () => {});
    return t;
  }

  // A box about 500 m across around a point, as [[s, w], [n, e]]
  function boxAround(lat, lng, m = MIN_BOX) {
    const dLat = m / 110574, dLng = m / (111320 * Math.max(0.05, Math.cos(lat * Math.PI / 180)));
    return [[U.r7(lat - dLat), U.r7(lng - dLng)], [U.r7(lat + dLat), U.r7(lng + dLng)]];
  }

  /* "-37.10, 174.95", "-37.10 174.95", "174.95, -37.10" and "37.1 S, 174.95 E" → result, else null.
     Without hemisphere letters the pair is read as lat, lng, swapped when only that makes sense
     (first value beyond ±90, or a New Zealand longitude first and a southern latitude second). */
  function parseCoords(q) {
    const m = /^\s*([-+]?\d{1,3}(?:\.\d+)?)\s*°?\s*([NSEW])?\s*[,;\s]\s*([-+]?\d{1,3}(?:\.\d+)?)\s*°?\s*([NSEW])?\s*$/i.exec(String(q || ''));
    if (!m) return null;
    const val = (n, h) => { const v = parseFloat(n); return h && /[SW]/i.test(h) ? -Math.abs(v) : v; };
    let a = val(m[1], m[2]), b = val(m[3], m[4]);
    const ha = (m[2] || '').toUpperCase(), hb = (m[4] || '').toUpperCase();
    let swap;
    if (ha || hb) swap = /[EW]/.test(ha) || /[NS]/.test(hb);
    else swap = (Math.abs(a) > 90 && Math.abs(b) <= 90) || (a >= 160 && b < 0 && b >= -90);
    if (swap) [a, b] = [b, a];
    if (!isFinite(a) || !isFinite(b) || Math.abs(a) > 90 || Math.abs(b) > 180) return null;
    return { name: `${a.toFixed(5)}, ${b.toFixed(5)}`, lat: U.r7(a), lng: U.r7(b), bounds: boxAround(a, b), type: 'coordinates' };
  }

  // Road-type abbreviations as NZ addresses write them (the same list DataHub uses to match counts)
  const ABBR = [['rd', 'Road'], ['ave', 'Avenue'], ['av', 'Avenue'], ['pl', 'Place'], ['cres', 'Crescent'], ['hwy', 'Highway'], ['tce', 'Terrace'],
    ['pde', 'Parade'], ['ln', 'Lane'], ['mwy', 'Motorway'], ['blvd', 'Boulevard']];
  // St, Dr and Ct are also Saint, Doctor and Court names, so they expand only at the end of a part
  const ABBR_END = [['st', 'Street'], ['dr', 'Drive'], ['ct', 'Court'], ['cl', 'Close'], ['cr', 'Crescent'], ['sq', 'Square']];
  // Always after another word, so 'St Heliers' and 'Rd 2' style names keep their first word
  const expand = q => {
    let s = String(q);
    for (const [a, full] of ABBR) s = s.replace(new RegExp(`(\\S\\s+)${a}\\.?(?=\\s|,|$)`, 'gi'), `$1${full}`);
    for (const [a, full] of ABBR_END) s = s.replace(new RegExp(`(\\S\\s+)${a}\\.?(?=\\s*(?:,|$))`, 'gi'), `$1${full}`);
    return s;
  };

  // "12, St Marks Road, Remuera, Auckland, 1050, New Zealand / Aotearoa" → "12 St Marks Road, Remuera, Auckland"
  function shorten(display) {
    const parts = String(display || '').split(',').map(s => s.trim())
      .filter((s, i) => s && !/^new zealand(\s*\/\s*aotearoa)?$/i.test(s) && !/^aotearoa(\s*\/\s*new zealand)?$/i.test(s) && !(i > 0 && /^\d{4}$/.test(s)));
    if (parts.length > 1 && /^\d+[a-z]?(?:[-/]\d+[a-z]?)?$/i.test(parts[0])) parts.splice(0, 2, parts[0] + ' ' + parts[1]);
    return parts.slice(0, 3).join(', ') || String(display || '');
  }

  function toResult(r) {
    const lat = parseFloat(r.lat), lng = parseFloat(r.lon);
    if (!isFinite(lat) || !isFinite(lng)) return null;
    const bb = (r.boundingbox || []).map(parseFloat);    // [south, north, west, east]
    let bounds = bb.length === 4 && bb.every(isFinite) ? [[bb[0], bb[2]], [bb[1], bb[3]]] : boxAround(lat, lng);
    // An address is a few metres across: give it a study-sized box instead
    const small = (bounds[1][0] - bounds[0][0]) * 110574 < 2 * MIN_BOX && (bounds[1][1] - bounds[0][1]) * 111320 * Math.cos(lat * Math.PI / 180) < 2 * MIN_BOX;
    if (small) bounds = boxAround(lat, lng);
    return { name: shorten(r.display_name || r.name), lat: U.r7(lat), lng: U.r7(lng), bounds, type: r.addresstype || r.type || r.category || 'place' };
  }

  const copy = list => list.map(x => ({ ...x, bounds: x.bounds.map(p => p.slice()) }));

  async function request(q, signal) {
    if (!(await turn(signal))) throw aborted();
    const url = NOMINATIM + '?' + new URLSearchParams({ format: 'jsonv2', countrycodes: 'nz', limit: '6', addressdetails: '0', 'accept-language': 'en', q });
    let r;
    try { r = await fetch(url); }
    catch (e) { throw new Error(`The place search could not be reached (${e.message}). Check your internet connection and try again.`); }
    if (r.status === 429) throw new Error('The place search is busy (HTTP 429). Wait a few seconds and try again.');
    if (!r.ok) throw new Error(`The place search answered HTTP ${r.status}. Try again shortly.`);
    const j = await r.json();
    return (Array.isArray(j) ? j : []).map(toResult).filter(Boolean);
  }

  // → [{ name, lat, lng, bounds: [[s, w], [n, e]], type }]; [] for an empty query. opts.signal: an AbortSignal
  async function geocode(q, opts = {}) {
    const s = String(q ?? '').trim(), signal = opts.signal;
    if (!s) return [];
    const c = parseCoords(s);
    if (c) return [c];
    if (signal && signal.aborted) throw aborted();
    const qx = expand(s), key = qx.toLowerCase().replace(/\s+/g, ' ');
    for (let tries = 0; ; tries++) {
      let p = cache.get(key);
      if (!p) {
        p = request(qx, signal);
        cache.set(key, p);
        p.catch(() => cache.delete(key));     // failures (and dropped searches) are not remembered
        if (cache.size > CACHE_MAX) cache.delete(cache.keys().next().value);
      }
      try { return copy(await p); }
      catch (e) {
        // Someone else's search for the same text was dropped while this one still wants an answer
        if (e.name === 'AbortError' && !(signal && signal.aborted) && tries < 1) continue;
        throw e;
      }
    }
  }

  return { geocode, parseCoords, shorten, expand, GAP };
})();
