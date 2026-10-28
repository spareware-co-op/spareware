'use strict';
/* Spare Construct · plug-in registry. Feature files (js/features/*.js) add tabs, inspector sections,
   panels, tools, colour modes, map layers, menu and top-bar items, settings, About-the-data rows,
   checklist items, report sections, exports and commands here, and listen to app events through
   hooks, so no feature has to edit a shared file. Core code (ui*.js, render.js) reads the lists.
   See docs/EXTENDING.md for every kind with an example.

   Rules: register at load time, never touch the DOM, UI, Render or the map at load time (the test
   harness loads feature files headlessly). Ids are kebab-case and start with the feature name. */

const Registry = (() => {
  // Field types: s string, n number, f function, b boolean, a array, x string or function
  const KINDS = {
    tabs:              { key: 'id',   req: { id: 's', label: 's', render: 'f' }, opt: { order: 'n', title: 's', hidden: 'f', onClick: 'f', onChange: 'f', onInput: 'f', onKeydown: 'f' } },
    inspectorSections: { key: 'id',   req: { id: 's', types: 'a', render: 'f' }, opt: { order: 'n', onClick: 'f', onChange: 'f', onInput: 'f' } },
    panels:            { key: 'type', req: { type: 's', render: 'f' }, opt: { label: 's', onClick: 'f', onChange: 'f', onInput: 'f', onDelete: 'f' } },
    tools:             { key: 'id',   req: { id: 's', label: 's' }, opt: { key: 's', iconSvgPath: 's', hint: 's', group: 's', order: 'n', cursor: 's', down: 'f', move: 'f', up: 'f', click: 'f', dbl: 'f', keydown: 'f', enter: 'f', leave: 'f', draw: 'f' } },
    modes:             { key: 'id',   req: { id: 's', label: 's' }, opt: { group: 's', order: 'n', needsResults: 'b', perArc: 'b', hidden: 'b', proposedOutline: 'b', flowLabels: 'b', needsBase: 'b', colour: 'f', nodeColour: 'f', linkColour: 'f', labelText: 'f', prepare: 'f', legend: 'f' } },
    layers:            { key: 'id',   req: { id: 's', label: 's', draw: 'f' }, opt: { order: 'n', defaultOn: 'b', below: 'b', minZoom: 'n', hit: 'f', legend: 'f' } },
    menu:              { key: 'id',   req: { id: 's', label: 'x', run: 'f' }, opt: { order: 'n', danger: 'b', keys: 's', enabled: 'f', authorText: 'b' } },
    topbar:            { key: 'id',   req: { id: 's' }, opt: { label: 's', title: 's', order: 'n', run: 'f', render: 'f', onClick: 'f', onChange: 'f', onInput: 'f', onKeydown: 'f', zone: 's', icon: 's' } },
    settings:          { key: 'id',   req: { id: 's', title: 's', render: 'f' }, opt: { order: 'n', defaults: 'f' } },
    about:             { key: 'id',   req: { id: 's', title: 's', rows: 'f' }, opt: { order: 'n', intro: 'x' } },
    checklist:         { key: 'id',   req: { id: 's', status: 'f' }, opt: { group: 's', label: 's', order: 'n', source: 's' } },
    report:            { key: 'id',   req: { id: 's', render: 'f' }, opt: { chapter: 's', order: 'n', title: 's' } },
    exports:           { key: 'id',   req: { id: 's', label: 's', ext: 's', build: 'f' }, opt: { mime: 's', order: 'n', filename: 'f', enabled: 'f', stamp: 'b', handover: 's', authorText: 'b' } },
    commands:          { key: 'id',   req: { id: 's', label: 's', run: 'f' }, opt: { keys: 's', group: 's', bound: 'b', enabled: 'f' } },
    welcome:           { key: 'id',   req: { id: 's', title: 's', run: 'f' }, opt: { text: 's', order: 'n' } },
  };
  const HOOKS = ['init', 'store', 'select', 'results'];
  const lists = {}, hooks = {}, errors = [];
  for (const k of Object.keys(KINDS)) lists[k] = [];
  for (const h of HOOKS) hooks[h] = [];
  let seq = 0;

  const typeOk = (t, v) => t === 's' ? typeof v === 'string' && v.length > 0 : t === 'n' ? typeof v === 'number' && isFinite(v) : t === 'f' ? typeof v === 'function'
    : t === 'b' ? typeof v === 'boolean' : t === 'a' ? Array.isArray(v) : t === 'x' ? typeof v === 'string' || typeof v === 'function' : true;
  const TYPE_NAME = { s: 'a non-empty string', n: 'a number', f: 'a function', b: 'true or false', a: 'an array', x: 'a string or a function' };

  function check(kind, def) {
    const K = KINDS[kind];
    if (!def || typeof def !== 'object') throw new TypeError(`Registry.${kind}: pass an object, got ${def}`);
    const who = `Registry.${kind}(${JSON.stringify(def[K.key] ?? '?')})`;
    for (const [f, t] of Object.entries(K.req)) if (!typeOk(t, def[f])) throw new TypeError(`${who}: '${f}' must be ${TYPE_NAME[t]}`);
    for (const [f, t] of Object.entries(K.opt)) if (def[f] !== undefined && !typeOk(t, def[f])) throw new TypeError(`${who}: '${f}' must be ${TYPE_NAME[t]}`);
    if (!/^[a-z][\w-]*$/i.test(def[K.key])) throw new TypeError(`${who}: '${K.key}' may only hold letters, digits, '-' and '_'`);
    // kind-specific rules
    if (kind === 'modes' && def.perArc !== false && !def.colour && !def.linkColour) throw new TypeError(`${who}: give colour(arcKey, link, res, ctx) for a per-direction mode, or linkColour(link, ctx) with perArc: false`);
    if (kind === 'topbar' && !def.render && !(def.label && def.run)) throw new TypeError(`${who}: give label and run() for a button, or render() for custom content`);
    if (kind === 'topbar' && def.zone && !/^(left|centre|right)$/.test(def.zone)) throw new TypeError(`${who}: zone must be 'left', 'centre' or 'right'`);
    if (kind === 'tools' && def.key && def.key.length !== 1) throw new TypeError(`${who}: key must be one character`);
    if (kind === 'inspectorSections' && !def.types.every(t => typeof t === 'string')) throw new TypeError(`${who}: types must be strings such as 'link', 'node', 'zone', 'gateway', 'route', 'count', 'overview'`);
  }

  // Register (or replace, by id) one definition of a kind. A country pack's version of a core entry (the same id) sits
  // beside it instead: the pack's shows on that country's projects and the core one everywhere else.
  function add(kind, def) {
    check(kind, def);
    const K = KINDS[kind], L = lists[kind], id = def[K.key], pack = def.pack || loading || '';
    const i = L.findIndex(d => d[K.key] === id && (d.pack || '') === pack);
    const base = i >= 0 ? L[i] : L.find(d => d[K.key] === id && !d.pack);
    const entry = { order: 1000, ...def, _seq: base ? base._seq : seq++ };
    if (pack) entry.pack = pack;   // registered by a country pack's file
    if (base && def.order === undefined) entry.order = base.order;
    if (i >= 0) L[i] = entry; else L.push(entry);
    return entry;
  }
  /* Country packs (js/packs/<id>/, see js/packs/README.md). A pack's files load between beginPack and endPack, and
     everything they register, hooks included, carries the pack's id and takes part only while the project's country
     is that pack. Everything registered outside a pack is the core and works in every country. */
  const packs = [];
  let loading = null;
  function beginPack(def) {
    if (!def || !/^[a-z][a-z0-9-]*$/.test(def.id || '') || typeof def.name !== 'string' || !def.name) throw new TypeError('Registry.beginPack: give an id (a lower-case code such as nz) and a name');
    if (loading) throw new TypeError(`Registry.beginPack(${def.id}): pack '${loading}' is still loading; end it with Registry.endPack() first`);
    const i = packs.findIndex(p => p.id === def.id);
    if (i >= 0) packs[i] = { ...def }; else packs.push({ ...def });
    loading = def.id;
    return def;
  }
  function endPack() { loading = null; }
  // The project's pack: settings.country when it names an installed pack, else the first one installed
  function country() {
    let c = null;
    try { c = Store.P && Store.P.settings && Store.P.settings.country; } catch { /* the registry loaded without the store */ }
    return packs.find(p => p.id === c) || packs[0] || null;
  }
  const live = d => !d.pack || (country() || {}).id === d.pack;
  // A kind's entries for this project: the core's, with the pack's own versions in place of the ones it replaces
  function current(kind) {
    const on = (country() || {}).id, key = KINDS[kind].key, mine = new Set(lists[kind].filter(d => d.pack && d.pack === on).map(d => d[key]));
    return lists[kind].filter(d => d.pack ? d.pack === on : !mine.has(d[key]));
  }

  const byOrder = (a, b) => a.order - b.order || a._seq - b._seq;
  function list(kind, filter) {
    if (!lists[kind]) throw new TypeError(`Registry.list: unknown kind '${kind}'. Kinds: ${Object.keys(KINDS).join(', ')}`);
    const out = current(kind).sort(byOrder);
    return filter ? out.filter(filter) : out;
  }
  const get = (kind, id) => (lists[kind] ? current(kind) : []).find(d => d[KINDS[kind].key] === id) || (lists[kind] || []).find(d => d[KINDS[kind].key] === id) || null;
  function remove(kind, id) { const L = lists[kind], i = L ? L.findIndex(d => d[KINDS[kind].key] === id) : -1; if (i >= 0) L.splice(i, 1); return i >= 0; }

  // Run a feature callback without letting it break the app; errors go to the console and Registry.errors
  function report(where, e) {
    errors.push({ where, message: String(e && e.message || e), at: Date.now() });
    if (errors.length > 50) errors.shift();
    console.error(`Spare Construct plug-in ${where} failed:`, e);
  }
  function call(def, fn, ...args) {
    if (!def || typeof def[fn] !== 'function') return undefined;
    try { return def[fn](...args); } catch (e) { report(`${def.id || def.type}.${fn}`, e); return undefined; }
  }
  // Render HTML from a feature; a failure shows a short message in its place instead of an empty panel
  function html(def, fn, ...args) {
    try { const h = def[fn](...args); return h == null ? '' : String(h); }
    catch (e) { report(`${def.id || def.type}.${fn}`, e); return `<p class="note warn">${U.esc(def.title || def.label || def.id || def.type)} could not be shown: ${U.esc(e.message)}. Other results are unaffected.</p>`; }
  }

  function on(h, fn) {
    if (typeof fn !== 'function') throw new TypeError(`Registry.on${h[0].toUpperCase() + h.slice(1)}: pass a function`);
    const pack = loading;
    hooks[h].push(pack ? (...a) => live({ pack }) ? fn(...a) : undefined : fn);
    return fn;
  }
  function fire(h, ...args) { for (const fn of hooks[h] || []) { try { fn(...args); } catch (e) { report('hook ' + h, e); } } }

  // Settings defaults from registered features: fill missing keys only, never overwrite saved values
  function applyDefaults(S) {
    for (const d of list('settings')) {
      const def = call(d, 'defaults');
      if (def && typeof def === 'object') Object.assign(S, U.deepMerge(def, S));
    }
    return S;
  }

  // 'Ctrl+K', 'Ctrl+Shift+S', 'Alt+1', '?', 'F2' against a keydown event (Ctrl matches Cmd on a Mac)
  function matchKeys(keys, e) {
    if (!keys) return false;
    return String(keys).split(/\s*,\s*/).some(k => {
      const parts = k.split('+').map(s => s.trim()).filter(Boolean), key = (parts.pop() || '').toLowerCase();
      if (!key) return false;
      const m = new Set(parts.map(p => p.toLowerCase()));
      const ctrl = !!(e.ctrlKey || e.metaKey);
      if (ctrl !== (m.has('ctrl') || m.has('cmd') || m.has('meta')) || !!e.altKey !== m.has('alt')) return false;
      const ek = String(e.key || '').toLowerCase();
      // a printable symbol such as '?' already includes Shift; only compare Shift for letters and named keys
      if (key.length > 1 || /[a-z0-9]/.test(key)) { if (!!e.shiftKey !== m.has('shift')) return false; }
      return ek === key || (key === 'space' && ek === ' ') || (key === 'esc' && ek === 'escape');
    });
  }

  const api = {
    KINDS, HOOKS, errors, version: 1,
    list, get, remove, call, html, fire, applyDefaults, matchKeys,
    beginPack, endPack, country, packs: () => packs.slice(),
    onInit: fn => on('init', fn), onStore: fn => on('store', fn), onSelect: fn => on('select', fn), onResults: fn => on('results', fn),
  };
  for (const k of Object.keys(KINDS)) api[k] = def => add(k, def);
  return api;
})();
