'use strict';
/* Spare Construct · application shell: boot (browser project store), map and basemaps, top bar and menu,
   save status and banners (second tab, settings notice, out-of-date results), tools and map events
   (pointer drag, long-press, road drafting), keyboard, the Map card, model runs and simulation control.
   Panels live in ui-inspector.js, tabs in ui-drawer.js, dialogs in ui-dialogs.js and shared helpers in
   kit.js. Feature files extend all of these through Registry (docs/EXTENDING.md). */

const UI = (() => {
  let map, tiles = null, drag = null, justDragged = false, running = false, simRAF = 0, simLast = 0, hudT = 0;
  const logLines = [];
  const S = () => Store.S;
  const { toast, busy, menu, ask, confirmBox } = Kit;
  const esc = U.esc;
  // Per-viewer preferences (never shared, never needed): wrapped so a blocked storage only loses the convenience
  const pref = {
    get(k, d = null) { try { const v = localStorage.getItem('corridor.ui.' + k); return v == null ? d : JSON.parse(v); } catch { return d; } },
    set(k, v) { try { localStorage.setItem('corridor.ui.' + k, JSON.stringify(v)); } catch { } },
  };

  // cors: the tile server sends Access-Control-Allow-Origin, so tiles can go into report figures (Render.snapshot)
  const BASEMAPS = {
    light:   { label: 'Light grey', url: 'https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}', labels: 'https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Reference/MapServer/tile/{z}/{y}/{x}', attr: 'Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors', max: 16, cors: true },
    dark:    { label: 'Dark grey', url: 'https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Base/MapServer/tile/{z}/{y}/{x}', labels: 'https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Reference/MapServer/tile/{z}/{y}/{x}', attr: 'Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors', max: 16, cors: true },
    streets: { label: 'Streets (OpenStreetMap)', url: 'https://tile.openstreetmap.org/{z}/{x}/{y}.png', attr: '© <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors', max: 19, cors: true },
    aerial:  { label: 'Aerial (Esri)', url: 'https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}', attr: 'Imagery © Esri, Maxar, Earthstar Geographics', max: 19, cors: true },
    linz:    { label: 'Aerial (LINZ — needs API key)', url: 'https://basemaps.linz.govt.nz/v1/tiles/aerial/WebMercatorQuad/{z}/{x}/{y}.webp?api={key}', attr: '© <a href="https://www.linz.govt.nz/">Toitū Te Whenua LINZ</a> CC BY 4.0', max: 21, key: true, cors: true },
    topo:    { label: 'Topographic (Esri)', url: 'https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{z}/{y}/{x}', attr: 'Tiles © Esri', max: 19, cors: true },
    none:    { label: 'No basemap' },
  };
  const ICON = {
    select: '<path d="M5 3l14 7.5-6.2 1.8L10.5 19z"/>',
    road: '<path d="M7 21 10 3M17 21 14 3"/><path d="M12 5v2.5M12 10.75v2.5M12 16.5V19" stroke-dasharray="0"/>',
    zone: '<path d="M4 11.5 12 4.5l8 7V20H4z"/><path d="M10 20v-5h4v5"/>',
    gateway: '<path d="M12 3l9 9-9 9-9-9z"/><path d="M8 12h8M13 9l3 3-3 3"/>',
    bus: '<rect x="5" y="3.5" width="14" height="14" rx="2.5"/><path d="M5 10.5h14M8 17.5V20M16 17.5V20"/><circle cx="8.5" cy="14" r="1"/><circle cx="15.5" cy="14" r="1"/>',
    path: '<circle cx="6" cy="17" r="2.3"/><circle cx="18" cy="7" r="2.3"/><path d="M8.2 16.2c5-1.3 2.2-7.6 7.6-8.6"/>',
    iso: '<circle cx="12" cy="12" r="2"/><circle cx="12" cy="12" r="5.5"/><circle cx="12" cy="12" r="9" stroke-dasharray="2.5 2.5"/>',
    measure: '<path d="M3.5 16.5 16.5 3.5l4 4-13 13z"/><path d="M7.5 12.5l2 2M10.5 9.5l2 2M13.5 6.5l2 2"/>',
    tool: '<circle cx="12" cy="12" r="7"/><path d="M12 8v8M8 12h8"/>',
  };
  const TOOLS = [
    { id: 'select', key: 'V', label: 'Select and edit', short: 'Select', group: 'edit', hint: '<b>Select</b> · Click a road, junction, zone or bus route to edit it. Drag junctions and zones to move them.' },
    { id: 'road', key: 'R', label: 'Draw road', short: 'Draw road', group: 'edit', hint: '<b>Draw road</b> · Click to add points. Click an existing road to join it. Backspace removes the last point. Double-click, Enter or Finish ends the road; Esc cancels.' },
    { id: 'zone', key: 'Z', label: 'Add land-use zone', short: 'Zone', group: 'edit', hint: '<b>Zone</b> · Click to place a development zone, then set its houses, retail, offices or school in the panel.' },
    { id: 'gateway', key: 'G', label: 'Gateway (traffic entering or leaving the area)', short: 'Gateway', group: 'edit', hint: '<b>Gateway</b> · Click the end of a road where traffic enters or leaves the study area.' },
    { id: 'bus', key: 'B', label: 'Draw bus route', short: 'Bus route', group: 'edit', hint: '<b>Bus route</b> · Click junctions in order along the route. Backspace removes the last stop. Double-click, Enter or Finish ends it; Esc cancels.' },
    { id: 'path', key: 'P', label: 'Travel time A to B', short: 'Travel time', group: 'analyse', hint: '<b>Travel time</b> · Click a start point, then an end point.' },
    { id: 'iso', key: 'I', label: 'Travel-time map (drive, walk or cycle)', short: 'Travel-time map', group: 'analyse', hint: '<b>Travel-time map</b> · Click a point to map how far you can drive, walk or cycle from it.' },
    { id: 'measure', key: 'M', label: 'Measure distance', short: 'Measure', group: 'analyse', hint: '<b>Measure</b> · Click points to measure. Backspace removes the last point; Esc clears.' },
  ];
  const BUILTIN_TOOL = new Set(TOOLS.map(t => t.id));
  // Phone review mode keeps the analysis tools only (mobile-review)
  const REVIEW_TOOLS = new Set(['select', 'path', 'iso', 'measure']);
  // Built-in tools then registered ones, in rail order (edit group first)
  function tools() {
    const reg = Registry.list('tools', t => { if (!BUILTIN_TOOL.has(t.id)) return true; console.warn(`Registry.tools: '${t.id}' is a built-in tool id; pick another id.`); return false; });
    const group = g => [...TOOLS.filter(t => t.group === g), ...reg.filter(t => (t.group || 'analyse') === g)];
    const other = reg.filter(t => t.group && t.group !== 'edit' && t.group !== 'analyse');
    return [...group('edit'), ...group('analyse'), ...other];
  }
  const railTools = () => View.review ? tools().filter(t => REVIEW_TOOLS.has(t.id)) : tools();
  const regTool = () => BUILTIN_TOOL.has(View.tool) ? null : Registry.get('tools', View.tool);
  const narrow = () => Kit.narrow();

  /* ---------------- boot ---------------- */
  async function init() {
    let first = false;
    try { if (!(await Store.boot())) { Store.P = newProject(); first = true; } }
    catch (e) { console.error(e); if (!Store.P) { Store.P = newProject(); first = true; } }
    Registry.applyDefaults(S());
    Kit.harden(Store.P); moveLinzKey();
    for (const l of Registry.list('layers')) if (!(l.id in View.show)) View.show[l.id] = !!l.defaultOn;
    // tool names on the rail for the first five sessions unless chosen; review mode on phones
    const sessions = pref.get('sessions', 0) + 1; pref.set('sessions', sessions);
    document.body.classList.toggle('tool-names', pref.get('toolNames', sessions <= 5) && !narrow());
    View.review = narrow(); onResize.was = View.review;
    applyTheme();
    map = L.map('map', { zoomControl: false, doubleClickZoom: false, boxZoom: false, minZoom: 3, maxZoom: 20, zoomSnap: 0.5, wheelPxPerZoomLevel: 90 })
      .setView(Store.P.view.c, Store.P.view.z);
    L.control.zoom({ position: 'bottomright' }).addTo(map);
    L.control.scale({ imperial: false, position: 'bottomright', maxWidth: 140 }).addTo(map);
    setBasemap(S().basemap || 'light', false);
    Render.init(map);
    installUndo();
    registerBuiltins();
    buildTools(); bindTop(); bindMap(); bindKeys(); bindHud(); bindShell();
    Inspector.bindEvents(); Drawer.bindEvents(); Dialogs.bindEvents();
    bindSaveStatus();
    Store.tabs.on(() => { renderBanners(); renderSave(); });
    map.on('moveend', () => { const c = map.getCenter(); Store.P.view = { c: [U.r7(c.lat), U.r7(c.lng)], z: map.getZoom() }; Store.saveSoon(); });
    map.on('zoomend', () => renderLegend());
    Store.on(onStore);
    if (Store.res()) View.colorBy = 'vc';
    refresh();
    refreshFile();
    await refreshRecent();
    if (first) Dialogs.welcome();
    Registry.fire('init');
    log('Spare Construct ready.');
  }

  function onStore(kind) {
    if (View.sim && /^(net|undo|project|demand|settings)$/.test(kind)) stopSim();
    if (kind === 'project') {
      Registry.applyDefaults(S());
      // a project from a file or this browser's storage: values of the right types, and no keys kept in it
      Kit.harden(Store.P); moveLinzKey();
      Object.assign(View, { sel: null, hover: null, path: null, pathEnds: null, iso: null, sla: null, draft: null, measure: [] });
      map.setView(Store.P.view.c, Store.P.view.z);
      refreshRecent(); refreshFile();
    }
    if (kind === 'meta') refreshRecentSoon();
    if (/^(net|undo|demand)$/.test(kind)) { View.path = null; View.pathEnds = null; if (View.iso) View.iso = null; }
    // a project from another country (settings.country) brings its own pack's tools and top-bar items (js/packs/README.md)
    const pack = (Registry.country() || {}).id;
    if (pack !== shownPack) { shownPack = pack; buildTools(); buildTopbarItems(); }
    refresh();
    Registry.fire('store', kind);
  }
  let shownPack = null;
  function refresh() { renderTop(); Inspector.render(); Drawer.render(); renderLegend(); renderStale(); renderBanners(); renderTemplate(); Render.redraw(); }

  // The LINZ key used to be a project setting, so it went into every saved file; it now stays in this browser
  function moveLinzKey() {
    const k = S().linzKey; if (!k) return;
    if (!Kit.linzKey()) DataHub.setKey('linz', k);
    S().linzKey = '';
    Store.saveSoon();
  }
  function applyTheme() {
    const t = 'light', root = document.documentElement; // one light look across Spareware
    if (t === 'auto') delete root.dataset.theme; else root.dataset.theme = t;
    if (pref.get('cbSafe', false)) root.dataset.cb = '1'; else delete root.dataset.cb;
    Render.readTheme && Render.readTheme();
  }
  let liveLayer = null;
  function setLive(on) {
    if (liveLayer) { map.removeLayer(liveLayer); liveLayer = null; }
    View.show.live = false;
    if (!on) return;
    const url = DataHub.tomtomTiles();
    if (!url) { toast('Live traffic needs a free TomTom key. Add it in the Traffic data tab.', 'warn'); Drawer.open('data'); Inspector.render(); return; }
    liveLayer = L.tileLayer(url, { opacity: 0.85, maxZoom: 20, maxNativeZoom: 18, attribution: 'Live traffic © TomTom', zIndex: 5 }).addTo(map);
    View.show.live = true;
  }
  function setBasemap(id, save = true) {
    const b = BASEMAPS[id] || BASEMAPS.light;
    if (tiles) { map.removeLayer(tiles); tiles = null; }
    if (b.url) {
      if (b.key && !Kit.linzKey()) { toast('Add your free LINZ Basemaps API key in Settings to use LINZ aerials.', 'warn'); return setBasemap('aerial'); }
      const co = b.cors ? { crossOrigin: 'anonymous' } : {};
      const base = L.tileLayer(b.url.replace('{key}', encodeURIComponent(Kit.linzKey())), { attribution: b.attr, subdomains: b.sub || 'abc', maxZoom: 20, maxNativeZoom: b.max, ...co });
      tiles = b.labels ? L.layerGroup([base, L.tileLayer(b.labels, { maxZoom: 20, maxNativeZoom: b.max, pane: 'shadowPane', ...co })]) : base;
      tiles.addTo(map);
    }
    if (!BASEMAPS[id]) id = 'light';
    S().basemap = id;
    if (save) { Store.saveSoon(); Inspector.render(); renderLegend(); }
  }

  /* ---------------- undo with labels (results-trust (g)) ----------------
     Kit.snap(label) names an undo step; undo and redo carry the name across, say which scenario it was in,
     and say so when undo switches scenario. Settings steps (Kit.snapSettings: reset, save) restore the
     whole settings object and mark results out of date. */
  function installUndo() {
    if (Store._labelledSwap) return;
    Store._labelledSwap = true;
    const orig = Store._swap.bind(Store);
    Store._swap = function (from, to) {
      const u = from[from.length - 1]; if (!u) return false;
      if (u.sid === 'settings') {
        from.pop();
        to.push({ sid: 'settings', label: u.label, data: JSON.stringify(this.P.settings) });
        this.P.settings = JSON.parse(u.data);
        for (const s of this.P.scenarios) for (const r of Object.values(s.res || {})) if (r) r.stale = true;
        this.emit('settings');
        return true;
      }
      const ok = orig(from, to);
      if (ok && u.label && to.length) to[to.length - 1].label = u.label;
      return ok;
    };
  }
  function undo(redo = false) {
    const st = redo ? Store.redoStack : Store.undoStack, u = st[st.length - 1];
    if (!u) return toast(redo ? 'Nothing to redo.' : 'Nothing to undo.');
    const before = Store.P.active;
    if (!(redo ? Store.redo() : Store.undo())) return toast(redo ? 'Nothing to redo.' : 'That step can no longer be undone.');
    const sc = Store.P.scenarios.find(s => s.id === u.sid), what = u.label || (u.sid === 'settings' ? 'settings' : 'the last edit');
    toast(`${redo ? 'Redid' : 'Undid'}: ${what}${sc ? ` (${sc.name})` : ''}.${sc && before !== sc.id ? ` Switched to ${sc.name}.` : ''}`);
  }

  /* ---------------- built-in menu items and commands ---------------- */
  const themeLabel = () => 'Theme: ' + ({ auto: 'match system', light: 'light', dark: 'dark' }[Store.P.theme || 'auto']);
  function cycleTheme() { const o = ['auto', 'light', 'dark']; Store.P.theme = o[(o.indexOf(Store.P.theme || 'auto') + 1) % 3]; applyTheme(); Store.emit('meta'); }
  /* Menu order bands (a separator goes between bands): 0–99 project files, 100–199 import,
     200–299 export (the Export submenu sits at 200), 300–399 samples, 400–499 app. */
  function registerBuiltins() {
    const keep = (kind, def) => { if (!Registry.get(kind, def.id)) Registry[kind](def); };
    [
      { id: 'new-project', label: 'New project', order: 10, run: () => newProjectFlow() },
      { id: 'open-project', label: 'Open project file…', order: 20, run: () => Dialogs.openFile('project') },
      { id: 'projects', label: 'Projects in this browser…', order: 26, run: () => manageProjects() },
      { id: 'save-project', label: 'Save', order: 30, keys: 'Ctrl+S', run: () => saveToFile() },
      { id: 'save-as', label: 'Save as…', order: 35, keys: 'Ctrl+Shift+S', run: () => saveToFile(true) },
      { id: 'copy-project', label: 'Keep a copy in this browser…', order: 38, run: () => copyProject() },
      { id: 'import-geojson', label: 'Import roads from GeoJSON…', order: 110, run: () => Dialogs.openFile('geojson') },
      { id: 'import-od', label: 'Import OD matrix (CSV)…', order: 120, run: () => Dialogs.openFile('od') },
      { id: 'sample-drury', label: 'Load the Drury sample', order: 300, run: () => Sample.drury() },
      { id: 'settings', label: 'Settings…', order: 400, run: () => Dialogs.settings() },
      { id: 'help', label: 'Help and shortcuts…', order: 410, keys: '?', run: () => Dialogs.help() },
      { id: 'about', label: 'About the data…', order: 420, run: () => Dialogs.about() },
      { id: 'run-log', label: 'Run log…', order: 425, run: () => runLog() },
    ].forEach(d => keep('menu', d));
    [
      { id: 'run-model', label: 'Run this scenario, AM and PM', keys: 'Ctrl+Enter', bound: true, group: 'Model', run: () => runModel() },
      { id: 'run-period', label: 'Run this scenario, current peak only', group: 'Model', run: () => runModel([Store.sc], [S().period]) },
      { id: 'run-all', label: 'Run all scenarios, AM and PM', keys: 'Ctrl+Shift+Enter', bound: true, group: 'Model', run: () => runModel(Store.P.scenarios, ['AM', 'PM']) },
      { id: 'run-stale', label: 'Run only out-of-date results', group: 'Model', run: () => runStale() },
      { id: 'simulate', label: 'Start or stop the simulation', keys: 'Space', bound: true, group: 'Model', run: () => toggleSim() },
      { id: 'save-project', label: 'Save', keys: 'Ctrl+S', bound: true, group: 'Project', run: () => saveToFile() },
      { id: 'save-as', label: 'Save as…', keys: 'Ctrl+Shift+S', bound: true, group: 'Project', run: () => saveToFile(true) },
      { id: 'undo', label: 'Undo', keys: 'Ctrl+Z', bound: true, group: 'Edit', run: () => undo() },
      { id: 'redo', label: 'Redo', keys: 'Ctrl+Y', bound: true, group: 'Edit', run: () => undo(true) },
      { id: 'import-roads', label: 'Import roads from OpenStreetMap…', group: 'Project', run: () => Dialogs.importRoads() },
      { id: 'settings', label: 'Settings…', group: 'App', run: () => Dialogs.settings() },
      { id: 'help', label: 'Help and shortcuts', keys: '?', bound: true, group: 'App', run: () => Dialogs.help() },
      { id: 'about', label: 'About the data', group: 'App', run: () => Dialogs.about() },
      { id: 'run-log', label: 'Run log', group: 'App', run: () => runLog() },
      ...tools().map(t => ({ id: 'tool-' + t.id, label: 'Tool: ' + t.label, keys: t.key, bound: true, group: 'Tools', run: () => setTool(t.id, true) })),
      ...Registry.list('tabs', t => t.id !== 'log').map(t => ({ id: 'tab-' + t.id, label: 'Show tab: ' + t.label, group: 'Tabs', run: () => Drawer.open(t.id) })),
      ...Registry.list('modes', m => !m.hidden).map(m => ({ id: 'mode-' + m.id, label: 'Show on map: ' + m.label, ...(m.key ? { keys: m.key, bound: true } : {}), group: 'Map', run: () => setColour(m.id) })),
    ].forEach(d => keep('commands', d));
  }
  const exportCtx = () => ({ sc: Store.sc, res: Store.res(), period: S().period, fileBase: fileBase() });
  function exportFile(id) {
    const d = Registry.get('exports', id); if (!d) return;
    const c = exportCtx();
    if (d.enabled && !Registry.call(d, 'enabled', c)) return toast('Nothing to export yet. Run the model first.', 'warn');
    let out;
    try { out = d.build(Store.P, c); } catch (e) { console.error(e); return toast(`Could not export ${d.label}: ${e.message}`, 'err', { details: String(e.stack || e) }); }
    if (out == null) return toast('Nothing to export yet. Run the model first.', 'warn');
    const name = (d.filename && Registry.call(d, 'filename', c)) || `${c.fileBase}-${d.id}.${d.ext}`;
    if (out instanceof Uint8Array) {
      const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([out], { type: d.mime || 'application/octet-stream' })); a.download = name;
      document.body.appendChild(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
    } else U.download(name, out, d.mime || 'text/plain');
  }
  function exportItems() {
    const c = exportCtx();
    return Registry.list('exports').map(d => ({ label: d.label, run: () => exportFile(d.id), disabled: !!(d.enabled && !Registry.call(d, 'enabled', c)) }));
  }
  // The ☰ Menu: anything squeezed out of the top bar first, then the registered items in bands
  function mainMenu() {
    const items = overflowItems();
    if (items.length) items.push(null);
    const reg = Registry.list('menu').map(d => ({ order: d.order, label: typeof d.label === 'function' ? Registry.call(d, 'label') : d.label, run: d.run, danger: d.danger, keys: d.keys, disabled: !!(d.enabled && !Registry.call(d, 'enabled')) }));
    reg.push({ order: 25, label: 'Open recent', sub: recentItems() });
    reg.push({ order: 200, label: 'Export', sub: exportItems() });
    reg.sort((a, b) => a.order - b.order);
    let band = null;
    for (const it of reg) { const b = Math.floor(it.order / 100); if (band !== null && b !== band) items.push(null); band = b; items.push(it); }
    return items;
  }

  /* ---------------- projects in this browser and files (storage) ---------------- */
  let recent = [], recentT = 0, fileInfo = null;
  async function refreshRecent() { try { recent = await Store.projects.list(); } catch { recent = []; } }
  function refreshRecentSoon() { clearTimeout(recentT); recentT = setTimeout(refreshRecent, 1500); }
  const sizeText = n => !(n > 0) ? '' : n >= 1e6 ? `${U.fmt.num(n / 1e6, 1)} MB` : `${Math.max(1, Math.round(n / 1e3))} kB`;
  function agoText(t) {
    if (!t) return '';
    const d = new Date(t), now = new Date();
    const hm = d.toLocaleTimeString('en-NZ', { hour: '2-digit', minute: '2-digit', hour12: false });
    if (d.toDateString() === now.toDateString()) return 'today ' + hm;
    const y = new Date(now); y.setDate(now.getDate() - 1);
    if (d.toDateString() === y.toDateString()) return 'yesterday ' + hm;
    return d.toLocaleDateString('en-NZ', { day: 'numeric', month: 'short', year: d.getFullYear() === now.getFullYear() ? undefined : 'numeric' });
  }
  const recentLine = m => [`edited ${agoText(m.updated)}`, sizeText(m.size), `${m.scenarios || 1} scenario${m.scenarios === 1 ? '' : 's'}`].filter(Boolean).join(' · ');
  function recentItems() {
    const list = recent.filter(m => !m.current).slice(0, 8);
    const items = list.map(m => ({ label: `${m.name} · ${recentLine(m)}`, title: m.name, run: () => openRecent(m.id) }));
    if (!items.length) items.push({ label: Store.projects.backend ? 'No other projects in this browser yet' : 'This browser cannot keep projects', disabled: true });
    items.push(null, { label: 'Projects in this browser…', run: () => manageProjects() });
    return items;
  }
  async function openRecent(id) {
    stopSim();
    const ok = await Store.projects.open(id);
    if (!ok) return toast('That project could not be opened from this browser’s storage. It may have been removed in another tab.', 'err');
    toast(`Opened ${Store.P.name}.`);
  }
  async function manageProjects() {
    await refreshRecent();
    const rows = recent.map(m => `<div class="recent-row"><span class="r-name"><b>${esc(m.name)}${m.current ? ' <span class="pill">open now</span>' : ''}</b><span>${esc(recentLine(m))}</span></span>
      ${m.current ? '' : `<button class="btn small" data-rp="open:${esc(m.id)}">Open</button><button class="btn small danger" data-rp="del:${esc(m.id)}">Remove from this browser</button>`}</div>`).join('');
    const where = Store.projects.backend === 'idb' ? 'in this browser’s storage (IndexedDB)' : Store.projects.backend === 'local' ? 'in this browser’s local storage' : 'nowhere: this browser refuses storage';
    const p = Kit.modal('Projects in this browser', `<p>Projects are kept ${where}, on this computer only. Save a file to share one or keep it safe.</p><div class="recent">${rows || '<p class="note">No projects yet.</p>'}</div>`, [['Close', null]], true);
    U.$('#modal .recent').onclick = async e => {
      const b = e.target.closest('[data-rp]'); if (!b) return;
      const [act, id] = [b.dataset.rp.slice(0, b.dataset.rp.indexOf(':')), b.dataset.rp.slice(b.dataset.rp.indexOf(':') + 1)];
      const m = recent.find(x => x.id === id); if (!m) return;
      if (act === 'open') { Kit.closeModal(null); return openRecent(id); }
      if (!(await confirmBox(`Remove “${m.name}” from this browser?`, 'It is deleted from this browser’s storage. Files you saved are not affected. This cannot be undone.', 'Remove from this browser'))) return manageProjects();
      const r = await Store.projects.remove(id);
      if (!r.ok) toast(r.why, 'warn'); else toast(`Removed ${m.name} from this browser.`);
      manageProjects();
    };
    return p;
  }
  async function copyProject() {
    const v = await ask('Keep a copy in this browser under a new name', (Store.P.name || 'Untitled project') + ' (copy)');
    if (!v) return;
    const id = await Store.projects.saveAs(v);
    if (!id) return toast('This browser would not keep a copy. Save to a file instead.', 'warn', { action: { label: 'Save to file', run: () => saveToFile(true) } });
    refreshRecent(); refreshFile();
    toast(`You are now working in “${v}”. The original is kept under Menu › Open recent.`);
  }
  async function refreshFile() {
    fileInfo = null;
    try { const h = await Store.projects.getHandle(); if (h) fileInfo = { name: h.name, at: null }; } catch { }
    renderSave();
  }
  const fileName = () => (Store.P.name || 'project').replace(/[^\w\- ]+/g, '').trim().replace(/\s+/g, '-').toLowerCase() + '.corridor.json';
  /* Save (Ctrl+S) writes the project file. In Chrome and Edge the first save asks where; later saves write the
     same file without asking (the handle is kept in this browser). Save as (Ctrl+Shift+S) always asks. Other
     browsers download a copy. */
  async function saveToFile(as = false) {
    const json = Store.serialise();
    if (typeof window.showSaveFilePicker === 'function') {
      try {
        let h = as ? null : await Store.projects.getHandle();
        if (h && h.queryPermission) {
          let p = await h.queryPermission({ mode: 'readwrite' });
          if (p !== 'granted' && h.requestPermission) p = await h.requestPermission({ mode: 'readwrite' });
          if (p !== 'granted') h = null;
        }
        if (!h) {
          h = await window.showSaveFilePicker({ suggestedName: fileName(), types: [{ description: 'Spare Construct project', accept: { 'application/json': ['.json'] } }] });
          await Store.projects.setHandle(h);
        }
        const w = await h.createWritable(); await w.write(json); await w.close();
        fileInfo = { name: h.name, at: Date.now() };
        renderSave();
        toast(`Saved to ${h.name}.`);
        return true;
      } catch (e) {
        if (e && e.name === 'AbortError') return false;
        console.warn(e);
        // the saved file may have been moved or deleted: forget it, so the next Save asks where to save
        await Store.projects.setHandle(null);
        fileInfo = null;
        toast(`Could not write the file (${(e && e.message) || e}). A copy is going to your downloads instead.`, 'warn', { action: { label: 'Save as…', run: () => saveToFile(true) } });
      }
    }
    U.download(fileName(), json);
    fileInfo = { name: fileName(), at: Date.now(), download: true };
    renderSave();
    toast(`Saved ${fileName()} to your downloads.`);
    return true;
  }

  /* ---------------- top bar ---------------- */
  function bindTop() {
    U.$('#projName').addEventListener('change', e => { Store.P.name = e.target.value.trim() || 'Untitled project'; Store.emit('meta'); });
    U.$('#scSel').addEventListener('change', e => { Store.P.active = e.target.value; View.sel = null; View.sla = null; Store.emit('scenario'); });
    U.$('#scNew').addEventListener('click', () => duplicateScenario());
    U.$('#scMenuBtn').addEventListener('click', e => menu(e.currentTarget, scenarioMenu()));
    U.$('#period').addEventListener('click', e => {
      const b = e.target.closest('[data-p]'), p = b && b.dataset.p; if (!p || p === S().period) return;
      S().period = p; Store.emit('period');
    });
    U.$('#year').addEventListener('change', e => setYear(e.target.value));
    U.$('#runBtn').addEventListener('click', () => runModel());
    U.$('#runMore').addEventListener('click', e => {
      const st = staleRuns();
      menu(e.currentTarget, [
        { label: 'This scenario, AM and PM', keys: 'Ctrl+Enter', run: () => runModel() },
        { label: `This scenario, ${S().period} peak only`, run: () => runModel([Store.sc], [S().period]) },
        { label: 'All scenarios, AM and PM', keys: 'Ctrl+Shift+Enter', run: () => runModel(Store.P.scenarios, ['AM', 'PM']) },
        { label: `Only out-of-date results (${st.length})`, disabled: !st.length, run: () => runStale() },
        // the network check (onboarding lane): disconnected roads, zones without access, one-way traps, before a run
        ...(typeof Onboarding !== 'undefined' && typeof Onboarding.runCheck === 'function' ? [null, { label: 'Check network', run: () => Onboarding.runCheck(true) }] : []),
      ]);
    });
    U.$('#simBtn').addEventListener('click', () => toggleSim());
    U.$('#undoBtn').addEventListener('click', () => undo());
    U.$('#redoBtn').addEventListener('click', () => undo(true));
    U.$('#osmBtn').addEventListener('click', () => Dialogs.importRoads());
    U.$('#menuBtn').addEventListener('click', e => { menu(e.currentTarget, mainMenu()); refreshRecent(); });
    U.$('#saveState').addEventListener('click', () => { if (U.$('#saveState').classList.contains('warn')) saveToFile(); });
    buildTopbarItems();
    if (window.ResizeObserver) new ResizeObserver(() => fitTopSoon()).observe(U.$('#top'));
    window.addEventListener('resize', () => { fitTopSoon(); onResize(); });
    document.fonts && document.fonts.ready.then(fitTopSoon);
  }
  const scenarioMenu = () => [
    ['Rename scenario…', () => renameScenario()],
    ['Use as base (do-minimum) for comparisons', () => { Store.P.base = Store.sc.id; Store.emit('meta'); toast(`${Store.sc.name} is now the base (do-minimum).`); }],
    ['New empty scenario', () => { const s = newScenario('Scenario ' + (Store.P.scenarios.length + 1)); Store.P.scenarios.push(s); Store.P.active = s.id; Store.emit('scenario'); }],
    ['Clear results', () => { Store.sc.res = {}; Store.emit('results'); }],
    null,
    ['Delete scenario', () => deleteScenario(), 'danger'],
  ];
  // The Year box edits the active scenario's forecast year; scenarios without one follow Settings › Default forecast year
  function setYear(v) {
    const y = Math.round(+v), sc = Store.sc;
    if (!(y >= 1900 && y <= 2200)) { U.$('#year').value = Engine.yearOf(sc); return toast('Enter a forecast year between 1900 and 2200.', 'warn'); }
    if (y === Engine.yearOf(sc) && sc.year != null) return;
    Kit.snap(`forecast year ${y}`);
    sc.year = y;
    Store.edited('settings');
  }

  // Registered top-bar items: a button (label + run) or custom content (render + handlers)
  function buildTopbarItems() {
    for (const z of ['left', 'centre', 'right']) {
      const host = U.$(`#top .tb-reg[data-zone="${z}"]`); if (!host) continue;
      const defs = Registry.list('topbar', d => (d.zone || 'right') === z);
      host.innerHTML = defs.map(d => d.render
        ? `<span class="tb-item" data-tb="${esc(d.id)}" data-prio="3"${d.title ? ` title="${esc(d.title)}"` : ''}>${Registry.html(d, 'render')}</span>`
        : `<button class="btn tb-item" data-tb="${esc(d.id)}" data-prio="3" title="${esc(d.title || d.label)}">${d.icon ? `<span class="ico" aria-hidden="true">${esc(d.icon)}</span>` : ''}<span class="lbl">${esc(d.label)}</span></button>`).join('');
      if (host._bound) continue;
      host._bound = true;
      const def = e => { const el = e.target.closest('[data-tb]'); return el && [Registry.get('topbar', el.dataset.tb), el]; };
      host.addEventListener('click', e => { const r = def(e); if (!r || !r[0]) return; if (r[0].render) Registry.call(r[0], 'onClick', e, r[1]); else Registry.call(r[0], 'run'); });
      for (const [ev, fn] of [['change', 'onChange'], ['input', 'onInput'], ['keydown', 'onKeydown']]) host.addEventListener(ev, e => { const r = def(e); if (r && r[0]) Registry.call(r[0], fn, e, r[1]); });
    }
    fitTopSoon();
  }
  function renderTopbar(id) {
    const d = Registry.get('topbar', id), el = U.$(`#top [data-tb="${CSS.escape(id)}"]`);
    if (d && d.render && el) el.innerHTML = Registry.html(d, 'render');
  }

  /* Keep the top bar on one line: while it overflows, hide data-prio items (3 first, then 2;
     lower data-hide first) and list them at the top of the Menu instead. On a phone (≤ 820 px) the
     bar keeps only ☰, Scenario, Period and Run (mobile-review). */
  let fitQueued = false;
  // a timer, not an animation frame, so it also runs while the window is in the background
  function fitTopSoon() { if (fitQueued) return; fitQueued = true; setTimeout(() => { fitQueued = false; fitTop(); }, 0); }
  function fitTop() {
    const top = U.$('#top'), all = U.$$('[data-prio]', top);
    all.forEach(el => el.classList.remove('tb-over'));
    if (narrow()) all.forEach(el => { if (el.id !== 'period') el.classList.add('tb-over'); });
    const rank = el => +el.dataset.prio * 1000 - (+el.dataset.hide || 500);
    const order = all.slice().sort((a, b) => rank(b) - rank(a) || all.indexOf(b) - all.indexOf(a));
    for (const el of order) { if (top.scrollWidth <= top.clientWidth) break; el.classList.add('tb-over'); }
  }
  function overflowItems() {
    const out = [];
    for (const el of U.$$('#top .tb-over')) {
      const m = el.dataset.menu;
      if (m === 'rename') out.push({ label: `Rename project (${Store.P.name})…`, run: async () => { const v = await ask('Rename project', Store.P.name); if (v) { Store.P.name = v; Store.emit('meta'); } } });
      else if (m === 'scenario') out.push({ label: 'Scenario actions', sub: scenarioMenu() });
      else if (m === 'period') out.push({ label: `Switch to the ${S().period === 'AM' ? 'PM' : 'AM'} peak`, run: () => { S().period = S().period === 'AM' ? 'PM' : 'AM'; Store.emit('period'); } });
      else if (m === 'year') out.push({ label: `Forecast year (${Engine.yearOf(Store.sc)})…`, run: async () => { const v = await ask('Forecast year for ' + Store.sc.name, String(Engine.yearOf(Store.sc))); if (v) setYear(v); } });
      else if (m === 'status') continue;
      else if (el.dataset.tb) { const d = Registry.get('topbar', el.dataset.tb); if (d && d.run) out.push({ label: d.label || d.title || d.id, run: () => Registry.call(d, 'run') }); }
      else if (el.tagName === 'BUTTON') out.push({ label: el.dataset.menuLabel || el.getAttribute('aria-label') || el.textContent.trim(), run: () => el.click(), disabled: el.disabled });
    }
    return out;
  }
  // Every scenario and peak whose results are out of date
  function staleRuns() {
    const out = [];
    for (const sc of Store.P.scenarios) for (const p of ['AM', 'PM']) { const r = sc.res && sc.res[p]; if (r && r.stale) out.push([sc, p]); }
    return out;
  }
  function renderTop() {
    U.$('#projName').value = Store.P.name;
    const sel = U.$('#scSel');
    sel.innerHTML = Store.P.scenarios.map(s => `<option value="${esc(s.id)}" ${s.id === Store.P.active ? 'selected' : ''}>${esc(s.name)}${s.id === Store.P.base ? ' — base (do-minimum)' : ''}</option>`).join('');
    U.$$('#period button').forEach(b => {
      const on = b.dataset.p === S().period, r = Store.res(Store.sc, b.dataset.p);
      b.classList.toggle('on', on); b.setAttribute('aria-pressed', on);
      b.classList.toggle('need', !r || !!r.stale);
      b.title = !r ? `No ${b.dataset.p} peak results for ${Store.sc.name} yet` : r.stale ? `The ${b.dataset.p} peak results are out of date` : `${b.dataset.p} peak results are up to date`;
    });
    U.$('#year').value = Engine.yearOf(Store.sc);
    U.$('#year').title = Store.sc.year != null ? `Forecast year for ${Store.sc.name}. Background traffic grows from the base year (${S().baseYear}).` : `Forecast year for ${Store.sc.name} (the default from Settings). Background traffic grows from the base year (${S().baseYear}).`;
    const res = Store.res(), rb = U.$('#runBtn'), n = staleRuns().length;
    rb.classList.toggle('stale', !!(res && res.stale));
    rb.querySelector('.run-stale').textContent = n ? `· ${n} out of date` : '';
    rb.title = n ? `${n} result${n === 1 ? ' is' : 's are'} out of date. Run this scenario for the AM and PM peaks (Ctrl+Enter); ▾ for more.` : 'Run this scenario for the AM and PM peaks (Ctrl+Enter)';
    const sb = U.$('#simBtn');
    sb.querySelector('.ico').textContent = View.sim ? '■' : '▶';
    sb.querySelector('.lbl').textContent = View.sim ? 'Stop' : 'Simulate';
    sb.setAttribute('aria-label', View.sim ? 'Stop the simulation' : 'Simulate');
    sb.classList.toggle('on', !!View.sim);
    U.$('#undoBtn').disabled = !Store.undoStack.length;
    U.$('#redoBtn').disabled = !Store.redoStack.length;
    const lastU = Store.undoStack[Store.undoStack.length - 1];
    U.$('#undoBtn').title = lastU && lastU.label ? `Undo ${lastU.label} (Ctrl+Z)` : 'Undo (Ctrl+Z)';
    document.title = `${Store.P.name} · Spare Construct`;
    renderSave();
    fitTopSoon();
  }
  const fileBase = () => (Store.P.name + '-' + Store.sc.name).replace(/[^\w\- ]+/g, '').replace(/\s+/g, '-').toLowerCase();

  /* Save status under the project name (storage (4)): 'Saved in this browser · 14:32', the file and when it was
     saved, or amber 'Not saved: browser storage refused. Save to file' (click to save a file). */
  let saving = false;
  function bindSaveStatus() {
    // the status line is not a live region (it would announce every autosave); a failed save is announced once
    let warned = false;
    Store.onSave(() => {
      saving = false; renderSave();
      const st = Store.saveState || {};
      if (st.ok === false && !warned) { warned = true; toast('This browser refused to save the project. Save it to a file so you do not lose work.', 'warn', { action: { label: 'Save to file', run: () => saveToFile() } }); }
      if (st.ok) warned = false;
    });
    Store.on(kind => { if (kind !== 'period' && !Store.tabs.yielded) { saving = true; renderSave(); } });
  }
  function renderSave() {
    const el = U.$('#saveState'); if (!el || !Store.P) return;
    const st = Store.saveState || {}, tb = Store.tabs.state();
    const hm = t => t ? new Date(t).toLocaleTimeString('en-NZ', { hour: '2-digit', minute: '2-digit', hour12: false }) : '';
    const file = fileInfo ? `File: ${fileInfo.name}${fileInfo.at ? ` · saved ${hm(fileInfo.at)}` : ''}` : '';
    let text, cls = '', tip;
    if (tb.yielded) { text = 'Not saved here: open in another tab'; cls = 'warn'; tip = tb.text + ' Click to save a file from this tab.'; }
    else if (st.ok === false) { text = 'Not saved: browser storage refused. Save to file'; cls = 'warn'; tip = `The browser refused to store this project${st.error ? ` (${st.error})` : ''}. Click to save it to a file so you do not lose work.`; }
    else if (saving) { text = 'Saving…'; cls = 'pending'; tip = 'Changes are saved in this browser a moment after you make them.'; }
    else if (st.ok) { text = Store.saveText() + (fileInfo && fileInfo.at ? ` · ${file}` : ''); tip = `${Store.saveText()}. ${file || 'Use Menu › Save (Ctrl+S) for a file you can share.'}`; }
    else { text = file || ''; tip = file || 'Not saved yet. Changes are saved in this browser a moment after you make them.'; }
    if (el.textContent !== text) el.textContent = text;
    el.title = tip;
    el.classList.toggle('warn', cls === 'warn'); el.classList.toggle('pending', cls === 'pending');
    el.setAttribute('aria-label', tip);
  }

  /* ---------------- banners: second tab, settings notice; and the out-of-date bar ---------------- */
  let tabDismissed = '';
  function renderBanners() {
    const el = U.$('#banners'); if (!el || !Store.P) return;
    const out = [], t = Store.tabs.state();
    if (t.text && tabDismissed !== t.text) out.push(`<div class="banner" role="status"><span class="b-text">${esc(t.text)}</span><button class="btn small primary" data-bn="claim">Use this tab</button><button class="btn small" data-bn="tabclose">Close</button></div>`);
    const N = Store.P.settingsNotice;
    if (N) out.push(`<div class="banner info" role="status"><span class="b-text">${esc(N.text)}</span><button class="btn small primary" data-bn="adopt">Review and adopt</button><button class="btn small" data-bn="keep">Keep mine</button>
      <details><summary>What changed (${N.changed.length})</summary><ul>${N.changed.map(c => `<li>${esc(c.label)}: yours ${esc(c.yoursText)}; new default ${esc(c.nowText)}</li>`).join('')}</ul></details></div>`);
    const html = out.join('');
    if (el.innerHTML !== html) el.innerHTML = html;
  }
  async function reviewDefaults() {
    const N = Store.P.settingsNotice; if (!N) return;
    const body = `<p>${esc(N.text)} Tick the new defaults to adopt. Your results go out of date, so run the model again afterwards.</p>
      <div class="tbl-wrap"><table class="t compact"><tr><th></th><th>Setting</th><th>Yours</th><th>New default</th></tr>
      ${N.changed.map((c, i) => `<tr><td><input type="checkbox" data-adopt="${i}" checked aria-label="Adopt ${esc(c.label)}"></td><td>${esc(c.label)}</td><td>${esc(c.yoursText)}</td><td>${esc(c.nowText)}</td></tr>`).join('')}</table></div>
      <p class="note">The new defaults follow NZ guidance: ${Kit.gl('MBCM')} v1.7.5 discounting and linear growth, and the TAG M3.1 convergence rule. Keeping yours keeps this project's results comparable with earlier work.</p>`;
    const v = await Kit.modal('Spare Construct’s defaults changed', body, [['Cancel', null, ''], ['Keep mine', 'keep', ''], ['Adopt the ticked ones', () => U.$$('#modal [data-adopt]').filter(x => x.checked).map(x => N.changed[+x.dataset.adopt].key)]], true);
    if (v === 'keep') { Store.keepSettings(); toast('Kept this project’s own settings.'); }
    else if (Array.isArray(v)) {
      if (!v.length) { Store.keepSettings(); return toast('Kept this project’s own settings.'); }
      Kit.snapSettings('adopting the new defaults');
      const n = Store.adoptDefaults(v);
      toast(n ? `Adopted ${n} new default${n === 1 ? '' : 's'}. Results are out of date: run the model again.` : 'Kept this project’s own settings.', '', n ? { action: { label: 'Run all scenarios', run: () => runModel(Store.P.scenarios, ['AM', 'PM']) } } : {});
    }
  }
  function renderStale() {
    const el = U.$('#staleBar'); if (!el || !Store.P) return;
    const res = Store.res(), show = !!(res && res.stale && RES_MODES.has(View.colorBy));
    el.hidden = !show;
    if (show && !el.firstChild) el.innerHTML = '<span>Showing results from before your last change.</span><button type="button" class="btn small primary" data-st="run">Run model</button><kbd>Ctrl+Enter</kbd>';
  }
  function bindShell() {
    U.$('#banners').addEventListener('click', e => {
      const b = e.target.closest('[data-bn]'); if (!b) return;
      const a = b.dataset.bn;
      if (a === 'claim') { Store.tabs.claim(); tabDismissed = ''; toast('This tab saves the project now. Edits in the other tab are not saved.'); }
      if (a === 'tabclose') { tabDismissed = Store.tabs.state().text; renderBanners(); }
      if (a === 'adopt') reviewDefaults();
      if (a === 'keep') { Store.keepSettings(); toast('Kept this project’s own settings.'); }
    });
    U.$('#staleBar').addEventListener('click', e => { if (e.target.closest('[data-st="run"]')) runModel(); });
    U.$('#hint').addEventListener('click', () => { if (U.$('#hint').classList.contains('mini') || U.$('#hint').classList.contains('opened')) { View.hintOpen = !View.hintOpen; renderHint(); } });
    const lg = U.$('#legend');
    lg.addEventListener('click', mapControl);
    lg.addEventListener('change', mapControl);
    U.$('#template').addEventListener('click', e => {
      const b = e.target.closest('[data-dr]'); if (!b) return;
      if (b.dataset.dr === 'finish') finishDraft();
      if (b.dataset.dr === 'undo') popDraft();
      if (b.dataset.dr === 'cancel') { View.draft = null; renderTemplate(); Render.redrawFx(); }
    });
    U.$('#tools').addEventListener('click', e => {
      const r = e.target.closest('[data-rail]'); if (!r) return;
      if (r.dataset.rail === 'names') { const on = !document.body.classList.contains('tool-names'); document.body.classList.toggle('tool-names', on); pref.set('toolNames', on); buildTools(); setTimeout(() => map.invalidateSize(), 0); }
      if (r.dataset.rail === 'edit') toggleReview();
    });
    // the inspector bottom sheet (phones): drag its grip between half and nearly full height
    const grip = U.$('#insGrip'), ins = U.$('#inspector');
    grip.addEventListener('pointerdown', e => {
      const y0 = e.clientY; try { grip.setPointerCapture(e.pointerId); } catch { }
      const up = ev => { grip.removeEventListener('pointerup', up); const dy = ev.clientY - y0; if (dy < -40) ins.classList.add('full'); else if (dy > 120) { if (ins.classList.contains('full')) ins.classList.remove('full'); else select(null); } else if (dy > 40) ins.classList.remove('full'); };
      grip.addEventListener('pointerup', up);
    });
  }
  async function toggleReview() {
    if (View.review) {
      const ok = await Kit.modal('Edit on this screen?', '<p>Editing works best on a larger screen. On a phone, the review tools (select, travel time, travel-time map and measure) are the easiest to use.</p>', [['Cancel', false, ''], ['Edit anyway', true]]);
      if (!ok) return;
      View.review = false; View.editChosen = true;
    } else { View.review = true; View.editChosen = false; }
    if (View.review && !REVIEW_TOOLS.has(View.tool)) setTool('select');
    buildTools();
  }
  /* Review mode follows the screen only when it crosses the phone breakpoint (wide → narrow turns it on), never
     while a road or route is being drawn, and not again once the user chose 'Edit anyway' this session. A
     resize (a phone keyboard opening, a rotation) never finishes or drops a drawing. */
  function onResize() {
    const n = narrow();
    if (n !== onResize.was && !View.draft) {
      if (n && !View.editChosen) { View.review = true; if (!REVIEW_TOOLS.has(View.tool)) setTool('select'); buildTools(); }
      if (!n) { View.review = false; buildTools(); }
      onResize.was = n;
    }
    document.body.classList.toggle('ins-open', n && !!View.sel);
    renderLegend();
  }

  /* ---------------- the Map card (map-card): colour mode, key and layers, always on the map ---------------- */
  let mcOpen = null;
  function mapCardOpen() {
    const v = pref.get('mapCard', null);
    return v != null ? !!v : !(narrow() || window.innerHeight < 700);
  }
  function renderLegend() {
    const el = U.$('#legend'); if (!el || !Store.P) return;
    if (mcOpen == null) mcOpen = mapCardOpen();
    const md = Render.modeDef(View.colorBy), ae = document.activeElement, fk = ae && el.contains(ae) ? (ae.dataset.mc || ae.dataset.show || '') : '';
    el.classList.toggle('closed', !mcOpen);
    el.innerHTML = `<div class="mc-head"><button type="button" class="mc-toggle" data-mc="toggle" aria-expanded="${mcOpen}" aria-controls="mcBody"><span class="k">Key:</span> ${esc(md.label)} <span aria-hidden="true">${mcOpen ? '▾' : '▴'}</span></button></div>
      <div class="mc-body" id="mcBody">${Inspector.mapControls({ legend: Render.legend(), layersOpen: !!View.mcLayers })}</div>`;
    if (fk) { const f = el.querySelector(`[data-mc="${CSS.escape(fk)}"], [data-show="${CSS.escape(fk)}"]`); if (f) f.focus(); }
  }
  // One delegated handler for the Map card and the overview panel's copy of the same controls
  function mapControl(e) {
    const t = e.target, mc = t.closest && t.closest('[data-mc]'), k = mc && mc.dataset.mc;
    if (e.type === 'click') {
      if (k === 'toggle') { mcOpen = !mcOpen; pref.set('mapCard', mcOpen); renderLegend(); U.$('#legend [data-mc="toggle"]').focus(); return true; }
      if (k === 'layers') { View.mcLayers = !View.mcLayers; renderLegend(); Inspector.render(); const b = U.$('#legend [data-mc="layers"]'); b && b.focus(); return true; }
      return false;
    }
    if (t.dataset.show) { View.show[t.dataset.show] = t.checked; if (t.dataset.show === 'live') setLive(t.checked); Render.redraw(); renderLegend(); Inspector.render(); return true; }
    if (k === 'mode') { setColour(t.value); return true; }
    if (k === 'basemap') { setBasemap(t.value); return true; }
    if (k === 'speedBands') { S().display = { ...(S().display || {}), speedBands: t.value }; Store.saveSoon(); renderLegend(); Render.redraw(); return true; }
    if (k === 'riskKind') { View.riskKind = t.value; renderLegend(); Render.redraw(); return true; }
    if (k === 'isoMode') { View.isoMode = t.value; if (View.iso) View.iso = isoFrom(View.iso.from); renderLegend(); renderTemplate(); Render.redraw(); return true; }
    return false;
  }

  function duplicateScenario() {
    const src = Store.sc, s = JSON.parse(JSON.stringify({ ...src, res: {} }));
    const names = new Set(Store.P.scenarios.map(x => x.name));
    let n = Store.P.scenarios.length + 1; while (names.has(`Scenario ${n}`)) n++;
    s.id = U.uid('s'); s.name = `Scenario ${n}`; s.res = {};
    Store.P.scenarios.push(s); Store.P.active = s.id;
    Store.emit('scenario');
    toast(`Created ${s.name} as a copy of ${src.name}. Change it, then run the model to compare.`, '', { action: { label: 'Rename', run: () => renameScenario() } });
  }
  async function renameScenario() {
    const v = await ask('Rename scenario', Store.sc.name);
    if (v) { Store.sc.name = v; Store.emit('meta'); }
  }
  async function deleteScenario() {
    if (Store.P.scenarios.length < 2) return toast('A project needs at least one scenario.', 'warn');
    const sc = Store.sc;
    if (!(await confirmBox(`Delete “${sc.name}”?`, 'Its roads, zones and results are removed. Undo cannot bring a deleted scenario back.', 'Delete scenario'))) return;
    Store.P.scenarios = Store.P.scenarios.filter(s => s !== sc);
    Store.P.active = Store.P.scenarios[0].id;
    if (Store.P.base === sc.id) Store.P.base = Store.P.scenarios[0].id;
    Store.undoStack = Store.undoStack.filter(u => u.sid !== sc.id);
    Store.emit('project');
  }
  async function newProjectFlow() {
    if (!(await confirmBox('Start a new project?', 'The current project stays in this browser (Menu › Open recent). Save it to a file first if you want to share it.', 'Start new project'))) return;
    await Kit.flushSave();
    const P = newProject(); P.view = Store.P.view; P.theme = Store.P.theme; P.settings.basemap = S().basemap;
    Store.setProject(P);
    await refreshRecent();
    Dialogs.welcome();
  }

  /* ---------------- running the model (results-trust (e), (f)) ----------------
     runModel(scenarios, periods) or runModel([[scenario, period]…]); opts.assign passes engine options (for
     example a tighter convergence target). The main Run button runs this scenario for both peaks. */
  /* What a run reads from a scenario: its network and demand (what undo snapshots) and the model settings, without
     view-only settings. Compared before and after each run, so an edit made while the model was running leaves
     the new result marked out of date instead of passing it off as current (results-trust). */
  const VIEW_SETTINGS = new Set(['period', 'basemap', 'linzKey', 'display', 'sim']);
  function runPrint(sc) {
    const s = {}; for (const [k, v] of Object.entries(S())) if (!VIEW_SETTINGS.has(k)) s[k] = v;
    try { return JSON.stringify(typeof editable === 'function' ? editable(sc) : { n: sc.nodes, l: sc.links, z: sc.zones, r: sc.routes, od: sc.odManual, y: sc.year }) + JSON.stringify(s); }
    catch { return String(Math.random()); }
  }
  const pairsOf = (list, periods) => Array.isArray(list) && list.length && Array.isArray(list[0]) ? list : list.flatMap(sc => periods.map(p => [sc, p]));
  function runStale() { const st = staleRuns(); if (!st.length) return toast('Every result is up to date.'); return runModel(st); }
  async function runModel(list = [Store.sc], periods = ['AM', 'PM'], opts = {}) {
    if (running) return false;
    running = true;
    const pairs = pairsOf(list, periods), t0 = performance.now(), rb = U.$('#runBtn'); rb.disabled = true;
    let ok = 0, cancelled = false;
    const notConv = [], done = new Set(), donePer = new Set(), editedDuring = [];
    try {
      busy('Starting the model…', { cancel: () => { cancelled = true; } });
      for (const [sc, period] of pairs) {
        if (cancelled) break;
        if (!Object.keys(sc.links).length) { toast(`${sc.name} has no roads yet. Import or draw some first.`, 'warn'); continue; }
        if (!Object.keys(sc.zones).length) { toast(`${sc.name} has no zones or gateways, so there is no traffic to load. Add a zone (Z) or gateway (G).`, 'warn'); continue; }
        busy(`${sc.name} · ${period} peak · building network…`);
        await U.sleep(20);
        const P0 = Store.P, print0 = runPrint(sc);
        const res = await Engine.assign(sc, { period, ...(opts.assign || {}), onProgress: (it, gap, msg) => {
          if (cancelled) throw Object.assign(new Error('Cancelled'), { name: 'AbortError' });
          busy(`${sc.name} · ${period} peak · ${msg || 'iteration ' + it} · gap ${Kit.gapPct(gap)}`);
        } });
        // another project opened, or the scenario deleted, while it ran: the result belongs nowhere
        if (Store.P !== P0 || !Store.P.scenarios.includes(sc)) continue;
        if (runPrint(sc) !== print0) { res.stale = true; editedDuring.push(`${sc.name} ${period}`); }
        sc.res[period] = res;
        ok++; done.add(sc.id); donePer.add(period);
        if (res.converged === false) notConv.push(`${sc.name} ${period}`);
        log(`<b>${esc(sc.name)}</b> ${period} ${res.year}: ${U.fmt.int(res.kpi.trips)} veh/h, ${res.it} iterations, gap ${Kit.gapPct(res.gap)}${res.converged === false ? ' (not converged)' : ''}, ${res.ms} ms. ${res.note ? esc(res.note) : ''}`);
        if (res.kpi.unserved > 1) log(`${U.fmt.int(res.kpi.unserved)} veh/h could not find a route. Check one-way roads and zone access lines.`);
        Registry.fire('results', sc, period, res);
      }
    } catch (e) {
      if (Kit.isAbort(e)) cancelled = true;
      else {
        console.error(e);
        toast(`The model could not run: ${e.message}. Undo your last change, or copy the details for a bug report.`, 'err', { details: `Spare Construct model error\n${e.stack || e.message}` });
        log('Model error: ' + esc(e.stack || e.message));
      }
    } finally {
      running = false; rb.disabled = false; busy(null);
    }
    if (ok) {
      if (!RES_MODES.has(View.colorBy)) View.colorBy = 'vc';
      Store.emit('results');
      const r = Store.res(), secs = U.fmt.num((performance.now() - t0) / 1000, 1);
      const conv = notConv.length ? ` Not fully converged: ${notConv.join(', ')}; see the Summary tab.` : '';
      const kind = notConv.length ? 'warn' : '';
      const grid = ok === done.size * donePer.size;
      if (done.size > 1) toast(`Ran ${grid ? `${done.size} scenarios × ${donePer.size} peak${donePer.size === 1 ? '' : 's'}` : `${ok} model run${ok === 1 ? '' : 's'} (${done.size} scenarios)`} in ${secs} s.${conv}`, kind, { action: { label: 'Compare scenarios', run: () => Drawer.open('compare') } });
      else if (r) toast(`Model run (${[...donePer].join(' and ')}, ${secs} s): ${S().period} peak ${U.fmt.int(r.kpi.trips)} veh/h, average speed ${U.fmt.num(r.kpi.spd, 1)} km/h, ${r.kpi.over} road${r.kpi.over === 1 ? '' : 's'} over capacity.${conv}`, kind, notConv.length ? { action: { label: 'Show summary', run: () => Drawer.open('summary') } } : {});
    }
    if (editedDuring.length) toast(`You changed the model while it was running, so these results are already out of date: ${editedDuring.join(', ')}. Run the model again.`, 'warn', { action: { label: 'Run out-of-date results', run: () => runStale() } });
    if (cancelled) toast(ok ? `Model run cancelled after ${ok} of ${pairs.length} runs. Finished runs are kept.` : 'Model run cancelled. Earlier results are kept.');
    return ok > 0;
  }

  /* ---------------- simulation ---------------- */
  async function toggleSim() {
    if (View.sim) { stopSim(); return; }
    let res = Store.res();
    if (!res || res.stale || !res.paths) {
      if (!(await runModel([Store.sc], [S().period]))) return;
      res = Store.res(); if (!res) return;
    }
    if (!res.paths) return toast('This model has too many zones to simulate (limit 300).', 'warn');
    View.sim = new Sim(Store.sc, res);
    if (!View.sim.routes.length) { View.sim = null; return toast('No traffic to simulate. Check zones and gateways.', 'warn'); }
    View.simPlaying = true; View.simAcc = 0; simLast = performance.now();
    if (map.getZoom() < 15) toast('Zoom in to street level to see vehicles.');
    View.drawer = 'sim';
    U.$('#simHud').hidden = false; map.getPane('netPane').style.opacity = '0.5';
    renderTop(); Drawer.render(); renderHud();
    cancelAnimationFrame(simRAF); simRAF = requestAnimationFrame(simLoop);
  }
  function stopSim() {
    cancelAnimationFrame(simRAF);
    View.sim = null;
    U.$('#simHud').hidden = true; map.getPane('netPane').style.opacity = '';
    renderTop(); Render.redrawFx();
    if (View.drawer === 'sim') Drawer.render();
  }
  function simLoop(now) {
    const sim = View.sim; if (!sim) return;
    const dtReal = Math.min(0.1, (now - simLast) / 1000); simLast = now;
    if (View.simPlaying) {
      View.simAcc += S().sim.speed * dtReal;
      let n = 0;
      const t0 = performance.now();
      while (View.simAcc >= sim.dt && n < 120 && performance.now() - t0 < 28) { sim.step(); View.simAcc -= sim.dt; n++; }
      if (View.simAcc > 4) View.simAcc = 0;
    }
    Render.redrawFx();
    if (now - hudT > 300) { hudT = now; renderHud(); Drawer.renderSimTab(); }
    simRAF = requestAnimationFrame(simLoop);
  }
  function renderHud() {
    const sim = View.sim, h = U.$('#simHud'); if (!sim) return;
    const st = sim.stats, last = st.series[st.series.length - 1] || { n: 0, q: 0, spd: 0 };
    const avgDelay = st.done ? (st.tt - st.ff) / st.done : 0;
    const speeds = [1, 3, 6, 12, 25, 50];
    h.innerHTML = `
      <div class="hud-row" style="justify-content:space-between"><span class="hud-clock">${U.fmt.clock(sim.startClock + sim.t)}</span>
        <span class="hud-row"><button class="btn small" data-sim="play">${View.simPlaying ? '❚❚ Pause' : '▶ Play'}</button><button class="btn small" data-sim="restart" title="Restart" aria-label="Restart">↺</button><button class="btn small" data-sim="stop" title="Stop simulation" aria-label="Stop simulation">■</button></span></div>
      <div class="hud-row">${speeds.map(s => `<button class="chip ${S().sim.speed === s ? 'on' : ''}" data-speed="${s}" aria-pressed="${S().sim.speed === s}">${s}×</button>`).join('')}</div>
      <div class="hud-stats">
        <div><b>${U.fmt.int(last.n)}</b><span>on road</span></div>
        <div><b>${U.fmt.int(last.q)}</b><span>queued</span></div>
        <div><b>${U.fmt.num(last.spd, 0)}</b><span>km/h avg</span></div>
        <div><b>${U.fmt.int(st.done)}</b><span>trips done</span></div>
        <div><b>${U.fmt.num(avgDelay, 0)} s</b><span>delay / trip</span></div>
        <div><b>${U.fmt.int(last.waiting || 0)}</b><span>waiting to enter</span></div>
      </div>`;
  }
  function bindHud() {
    U.$('#pathCard').addEventListener('click', pathCardClick);
    U.$('#simHud').addEventListener('click', e => {
      const b = e.target.closest('button'); if (!b) return;
      if (b.dataset.speed) { S().sim.speed = +b.dataset.speed; renderHud(); Store.saveSoon(); return; }
      if (b.dataset.sim === 'play') { View.simPlaying = !View.simPlaying; renderHud(); }
      if (b.dataset.sim === 'stop') stopSim();
      if (b.dataset.sim === 'restart') { const res = Store.res(); View.sim = new Sim(Store.sc, res); View.simAcc = 0; renderHud(); }
    });
  }

  /* ---------------- tools ---------------- */
  function buildTools() {
    const el = U.$('#tools'), list = railTools(), names = document.body.classList.contains('tool-names');
    let g = list[0] && (list[0].group || 'analyse');
    el.innerHTML = list.map(t => {
      const sep = (t.group || 'analyse') !== g ? '<div class="tool-sep"></div>' : ''; g = t.group || 'analyse';
      const icon = ICON[t.id] || t.iconSvgPath || ICON.tool;
      return `${sep}<button class="tool" data-tool="${esc(t.id)}" title="${esc(t.label)}${t.key ? ` (${esc(t.key)})` : ''}" aria-label="${esc(t.label)}"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${icon}</svg><span class="tname">${esc(t.short || t.label)}</span>${t.key ? `<kbd>${esc(t.key)}</kbd>` : ''}</button>`;
    }).join('') +
      (View.review || narrow() ? `<div class="tool-sep"></div><button class="rail-edit ${View.review ? '' : 'on'}" data-rail="edit" title="${View.review ? 'Turn on the editing tools' : 'Back to the review tools'}">${View.review ? 'Edit' : 'Review'}</button>` : '') +
      `<button class="rail-names" data-rail="names" aria-pressed="${names}" title="${names ? 'Hide tool names' : 'Show tool names'}">${names ? '« Hide names' : '»'}</button>`;
    const keys = new Map();
    for (const t of tools()) if (t.key) { const k = t.key.toLowerCase(); if (keys.has(k)) console.warn(`Tool key ${t.key} is used by both '${keys.get(k)}' and '${t.id}'.`); else keys.set(k, t.id); }
    el.onclick = e => { const b = e.target.closest('[data-tool]'); if (b) setTool(b.dataset.tool, true); };
    setTool(list.some(t => t.id === View.tool) ? View.tool : 'select');
  }
  // user: chosen by a click or key, so it counts towards the hint shrinking after three uses
  function setTool(id, user = false) {
    const prev = regTool();
    if ((View.tool === 'road' || View.tool === 'bus') && View.draft && View.draft.pts.length >= 2 && id !== View.tool) finishDraft();
    if (prev && prev.id !== id) Registry.call(prev, 'leave');
    const same = id === View.tool;
    View.tool = id; View.snap = null;
    if (!same) View.draft = null;
    // drawing never acts on a selection (draft-bug): Backspace would otherwise delete it
    if (id === 'road' || id === 'bus') View.sel = null;
    if (id !== 'path') { View.pathPick = []; }
    if (id !== 'measure') View.measure = [];
    if (id === 'iso') { /* keep */ } else if (View.colorBy === 'iso') { View.colorBy = View.prevColour || 'class'; View.iso = null; }
    U.$$('#tools .tool').forEach(b => { b.classList.toggle('on', b.dataset.tool === id); b.setAttribute('aria-pressed', b.dataset.tool === id); });
    const c = map.getContainer(), def = tools().find(t => t.id === id) || {};
    c.className = c.className.replace(/\btool-[\w-]+/g, '').trim() + ' tool-' + id;
    c.style.cursor = def.cursor || '';
    if (user) { const u = pref.get('hints', {}); u[id] = (u[id] || 0) + 1; pref.set('hints', u); View.hintOpen = false; }
    renderHint();
    const now = regTool();
    if (now && now !== prev) Registry.call(now, 'enter');
    Inspector.render();
    renderTemplate(); renderPathCard(); renderLegend(); renderStale(); Render.redraw();
  }
  // Tool hints show in full for the first three uses, then shrink to a '?' chip (map-card)
  function renderHint() {
    const el = U.$('#hint'), def = tools().find(t => t.id === View.tool) || {}, html = def.hint || '';
    const uses = (pref.get('hints', {})[View.tool] || 0), shrinks = !!html && uses > 3, mini = shrinks && !View.hintOpen;
    el.classList.toggle('mini', mini); el.classList.toggle('opened', shrinks && !mini);
    const plain = html.replace(/<[^>]+>/g, '');
    el.innerHTML = mini ? '?' : html;
    el.title = mini ? plain + ' (click to show)' : shrinks ? 'Click to hide' : '';
    // once it shrinks, the hint is a toggle button both ways (shown in full, or the '?' chip), by mouse or keyboard
    el.setAttribute('role', shrinks ? 'button' : 'note');
    el.setAttribute('aria-label', mini ? 'Show the tool hint: ' + plain : shrinks ? plain + ' (press to hide)' : plain);
    if (shrinks) el.setAttribute('aria-expanded', String(!mini)); else el.removeAttribute('aria-expanded');
    el.tabIndex = shrinks ? 0 : -1;
  }
  // The road template and the drafting card (draft-bug): Finish / Undo point / Cancel and a live readout
  function draftHtml() {
    const d = View.draft, bus = View.tool === 'bus', n = d ? d.pts.length : 0, sc = Store.sc;
    let read = n ? `${n} ${bus ? 'stop' : 'point'}${n === 1 ? '' : 's'}` : `No ${bus ? 'stops' : 'points'} yet. Click the map to start.`;
    if (n >= 2) { let len = 0; for (let i = 1; i < n; i++) len += U.dist(d.pts[i - 1].ll, d.pts[i].ll); read += ` · ${U.fmt.len(len)}`; }
    if (n && !bus) {
      const names = new Set();
      for (const p of d.pts) {
        if (p.type === 'link' && sc.links[p.id] && sc.links[p.id].name) names.add(sc.links[p.id].name);
        if (p.type === 'node' && sc.nodes[p.id]) Net.linksAt(sc, p.id).forEach(l => l.name && names.add(l.name));
      }
      if (names.size) read += ` · joins ${[...names].slice(0, 2).join(' and ')}`;
      else if (n >= 2) read += ' · not joined to a road yet';
    }
    return `<div class="draft"><div class="lg-title" style="margin:0">${bus ? 'New bus route' : 'Drawing'}</div><div class="draft-read" aria-live="polite">${esc(read)}</div>
      <div class="actions"><button type="button" class="btn small primary" data-dr="finish" ${n >= 2 ? '' : 'disabled'}>✓ Finish ${bus ? 'route' : 'road'} (Enter)</button>
        <button type="button" class="btn small" data-dr="undo" ${n ? '' : 'disabled'}>⌫ Undo point</button>
        <button type="button" class="btn small" data-dr="cancel" ${n ? '' : 'disabled'}>✕ Cancel (Esc)</button></div></div>`;
  }
  function renderTemplate() {
    const el = U.$('#template'), t = View.template;
    el.hidden = !(View.tool === 'road' || View.tool === 'bus' || View.tool === 'iso');
    if (el.hidden) return;
    if (View.tool === 'iso') {
      el.innerHTML = `<div class="lg-title" style="margin:0">Travel-time map</div><label class="row"><span>Travel by</span><select id="tpIso">${Kit.opt({ drive: 'Driving', walk: 'Walking (4.8 km/h)', cycle: 'Cycling (15 km/h)' }, View.isoMode)}</select></label>
        <p class="note" style="margin:0">${View.isoMode === 'drive' ? 'Uses peak-hour travel times once the model has run.' : 'Spare Construct assumption: on every road except motorways and ramps; no footpath, crossing or gradient data.'}</p>`;
      el.onchange = e => { if (e.target.id === 'tpIso') { View.isoMode = e.target.value; if (View.iso) { View.iso = isoFrom(View.iso.from); Render.redraw(); } renderLegend(); renderTemplate(); } };
      return;
    }
    if (View.tool === 'bus') { el.innerHTML = draftHtml(); el.onchange = null; return; }
    el.innerHTML = `
      <div class="lg-title" style="margin:0">New road</div>
      <label class="row"><span>Class</span><select id="tpCls">${Object.entries(CLASSES).map(([k, c]) => `<option value="${k}" ${k === t.cls ? 'selected' : ''}>${c.label}</option>`).join('')}</select></label>
      <label class="row"><span>Lanes</span><select id="tpLanes">${[1, 2, 3, 4].map(n => `<option value="${n}" ${n === t.lanes ? 'selected' : ''}>${n} each way</option>`).join('')}</select></label>
      <label class="row"><span>Speed limit</span><select id="tpSpeed">${SPEEDS.map(s => `<option value="${s}" ${s === t.speed ? 'selected' : ''}>${s} km/h</option>`).join('')}</select></label>
      <label class="row"><span>Status</span><select id="tpStatus"><option value="proposed" ${t.status === 'proposed' ? 'selected' : ''}>Proposed</option><option value="existing" ${t.status === 'existing' ? 'selected' : ''}>Existing</option></select></label>
      <label class="check"><input type="checkbox" id="tpOne" ${t.oneway ? 'checked' : ''}> One-way (in drawing direction)</label>
      ${draftHtml()}`;
    el.onchange = () => {
      const cls = U.$('#tpCls').value;
      if (cls !== t.cls) { t.speed = CLASSES[cls].speed; t.lanes = Math.min(CLASSES[cls].lanes, 4); }
      Object.assign(t, { cls, lanes: cls !== t.cls ? t.lanes : +U.$('#tpLanes').value, status: U.$('#tpStatus').value, oneway: U.$('#tpOne').checked });
      if (cls === t.cls) t.speed = +U.$('#tpSpeed').value;
      t.cls = cls;
      renderTemplate();
    };
  }
  function draftChanged() { const d = U.$('#template .draft'); if (d && !U.$('#template').hidden) d.outerHTML = draftHtml(); else renderTemplate(); Render.redrawFx(); }
  // Backspace, Delete, Ctrl+Z or ⌫ Undo point while drawing: remove the last point, never the previous road
  function popDraft() {
    const d = View.draft; if (!d) return false;
    d.pts.pop();
    if (!d.pts.length) View.draft = null;
    draftChanged();
    return true;
  }
  function finishDraft() { if (View.tool === 'bus' || (View.draft && View.draft.kind === 'bus')) finishBus(); else finishRoad(); }
  function roadProps() {
    const t = View.template;
    return { cls: t.cls, lanesAB: t.lanes, lanesBA: t.oneway ? 0 : t.lanes, speed: t.speed, status: t.status };
  }

  function snapAt(cp, latlng) {
    const h = Render.hit(cp, { zones: false, nodeR: 14, routes: false, linkTol: 12 });
    if (h && h.type === 'node') return { type: 'node', id: h.id, ll: h.ll };
    if (h && h.type === 'link') return { type: 'link', id: h.id, ll: h.ll };
    return { type: 'free', ll: [latlng.lat, latlng.lng] };
  }

  function finishRoad() {
    const d = View.draft; View.draft = null; View.snap = null;
    if (!d || d.pts.length < 2) { draftChanged(); return; }
    const sc = Store.sc;
    Kit.snap('new road');
    const seq = d.pts.map((p, i) => {
      if (p.type === 'node' && sc.nodes[p.id]) return { node: p.id };
      if (p.type === 'link') {
        const nl = Net.nearestLink(sc, p.ll, 6);
        if (nl) return { node: Net.splitLink(sc, nl.id, p.ll) };
      }
      if (i === 0 || i === d.pts.length - 1) return { node: Net.addNode(sc, p.ll) };
      return { ll: p.ll };
    });
    const made = [];
    let start = seq[0].node, pts = [];
    for (let i = 1; i < seq.length; i++) {
      const s = seq[i];
      if (s.ll) { pts.push(s.ll); continue; }
      if (s.node !== start) made.push(Net.addLink(sc, start, s.node, pts, roadProps()));
      start = s.node; pts = [];
    }
    Net.autoControl(sc, seq.filter(s => s.node).map(s => s.node));
    if (!made.length) { Store.undoStack.pop(); draftChanged(); return; }
    let len = 0; for (const id of made) len += Net.len(sc, sc.links[id]);
    View.sel = { type: 'link', id: made[made.length - 1] };
    const joined = d.pts.some(p => p.type !== 'free') || Object.keys(sc.links).length === made.length;
    Store.edited();
    const undoAct = { action: { label: 'Undo', run: () => undo() } };
    if (joined) toast(`Added ${U.fmt.len(len)} of ${CLASSES[View.template.cls].label.toLowerCase()} road in ${made.length} segment${made.length > 1 ? 's' : ''}.`, '', undoAct);
    else toast(`Added ${U.fmt.len(len)} of road, but it isn’t connected to the network. Start or end on an existing road to join it.`, 'warn', undoAct);
  }
  function finishBus() {
    const d = View.draft; View.draft = null; View.snap = null;
    if (!d || d.pts.length < 2) { draftChanged(); return; }
    const sc = Store.sc;
    Kit.snap('new bus route');
    const id = U.uid('r'), n = Object.keys(sc.routes).length;
    sc.routes[id] = { id, name: `Bus route ${n + 1}`, color: ROUTE_COLOURS[n % ROUTE_COLOURS.length], freq: 6, wp: d.pts.map(p => p.id) };
    View.sel = { type: 'route', id };
    Store.edited();
    toast('Bus route added at 6 buses an hour. Set the frequency in the panel.', '', { action: { label: 'Undo', run: () => undo() } });
  }

  /* Map events. Junctions and zones are dragged with Pointer Events and pointer capture, so dragging works
     with a finger or pen as well as a mouse; a long press (500 ms, 6 px) does what a right-click does. */
  function bindMap() {
    const C = map.getContainer();
    let lp = null;
    const cancelLP = () => { if (lp) { clearTimeout(lp.t); lp = null; } };
    const suppressClick = () => { justDragged = true; setTimeout(() => { justDragged = false; }, 450); };
    C.addEventListener('pointerdown', e => {
      if (e.pointerType === 'mouse' && e.button !== 0) return;
      if (!e.target.closest || e.target.closest('.leaflet-control')) return;
      const cp = map.mouseEventToContainerPoint(e);
      if (e.pointerType !== 'mouse') {
        cancelLP();
        lp = { x: e.clientX, y: e.clientY, t: setTimeout(() => { lp = null; if (View.draft) { suppressClick(); finishDraft(); } }, 500) };
      }
      // Shift+click and Shift+drag select several objects (multi-edit), so they never start moving a junction or zone
      if (regTool() || View.tool !== 'select' || View.review || e.shiftKey) return;
      const h = Render.hit(cp);
      if (h && (h.type === 'node' || h.type === 'zone')) {
        drag = { h, start: cp, moved: false, id: e.pointerId };
        map.dragging.disable();
        try { C.setPointerCapture(e.pointerId); } catch { }
      }
    });
    C.addEventListener('pointermove', e => {
      if (lp && Math.hypot(e.clientX - lp.x, e.clientY - lp.y) > 6) cancelLP();
      if (!drag || e.pointerId !== drag.id) return;
      const cp = map.mouseEventToContainerPoint(e);
      if (!drag.moved && cp.distanceTo(drag.start) < 4) return;
      const sc = Store.sc, o = drag.h.type === 'node' ? sc.nodes[drag.h.id] : sc.zones[drag.h.id];
      if (!drag.moved) { Kit.snap(drag.h.type === 'node' ? `moving ${Kit.junctionName(drag.h.id)}` : `moving ${(o && o.name) || 'a zone'}`); drag.moved = true; C.classList.add('dragging-node'); }
      const ll = map.containerPointToLatLng(cp);
      if (o) { o.lat = U.r7(ll.lat); o.lng = U.r7(ll.lng); Render.geomChanged(); }
    });
    const endDrag = e => {
      cancelLP();
      if (!drag || (e && e.pointerId != null && e.pointerId !== drag.id)) return;
      map.dragging.enable(); C.classList.remove('dragging-node');
      try { C.releasePointerCapture(drag.id); } catch { }
      if (drag.moved) { suppressClick(); View.sel = { type: drag.h.type, id: drag.h.id }; Store.edited(drag.h.type === 'zone' ? 'demand' : 'net'); }
      drag = null;
    };
    C.addEventListener('pointerup', endDrag);
    C.addEventListener('pointercancel', endDrag);
    map.on('mousedown', e => { const rt = regTool(); if (rt) Registry.call(rt, 'down', e); });
    map.on('mousemove', e => {
      View.cursor = e.latlng;
      const rt = regTool(); if (rt) return Registry.call(rt, 'move', e);
      if (drag) return;
      const t = View.tool;
      if (t === 'select') {
        const h = Render.hit(e.containerPoint, { routes: true, counts: true });
        const key = h ? h.type + h.id : '', old = View.hover ? View.hover.type + View.hover.id : '';
        if (key !== old) { View.hover = h ? { type: h.type, id: h.id } : null; C.classList.toggle('hovering', !!h); Render.redrawFx(); }
      } else if (t === 'road' || t === 'gateway') { View.snap = snapAt(e.containerPoint, e.latlng); Render.redrawFx(); }
      else if (t === 'bus') { const h = Render.hit(e.containerPoint, { zones: false, links: false, nodeR: 16 }); View.snap = h ? { type: 'node', id: h.id, ll: h.ll } : null; Render.redrawFx(); }
      else if (t === 'measure' && View.measure.length) Render.redrawFx();
    });
    map.on('mouseup', e => { const rt = regTool(); if (rt) Registry.call(rt, 'up', e); });
    map.on('click', e => {
      if (justDragged) return;
      const rt = regTool(); if (rt) return Registry.call(rt, 'click', e);
      const t = View.tool, sc = Store.sc;
      if (t === 'select') { const h = Render.hit(e.containerPoint, { routes: true, counts: true }); select(h ? { type: h.type, id: h.id } : null); }
      else if (t === 'road') {
        const p = snapAt(e.containerPoint, e.latlng);
        View.draft ||= { kind: 'road', pts: [] };
        const last = View.draft.pts[View.draft.pts.length - 1];
        if (last && map.latLngToContainerPoint(last.ll).distanceTo(map.latLngToContainerPoint(p.ll)) < 5) return;
        View.draft.pts.push(p);
        draftChanged();
      } else if (t === 'zone') {
        const id = U.uid('z'), n = Object.values(sc.zones).filter(z => z.kind !== 'gateway').length + 1;
        Kit.snap(`new zone Zone ${n}`);
        sc.zones[id] = { id, kind: 'zone', name: `Zone ${n}`, lat: U.r7(e.latlng.lat), lng: U.r7(e.latlng.lng), lu: { residential: 200 }, connectors: null };
        View.sel = { type: 'zone', id };
        Store.edited('demand');
        toast('Zone added with 200 houses. Change its land use in the panel.', '', { action: { label: 'Undo', run: () => undo() } });
      } else if (t === 'gateway') addGateway(e);
      else if (t === 'bus') {
        const h = Render.hit(e.containerPoint, { zones: false, links: false, nodeR: 16 });
        if (!h) return toast('Click on a junction or the end of a road to add a stop.');
        View.draft ||= { kind: 'bus', pts: [] };
        const last = View.draft.pts[View.draft.pts.length - 1];
        if (last && last.id === h.id) return;
        View.draft.pts.push({ type: 'node', id: h.id, ll: h.ll });
        draftChanged();
      } else if (t === 'path') pickPath(e);
      else if (t === 'iso') pickIso(e);
      else if (t === 'measure') { View.measure.push([e.latlng.lat, e.latlng.lng]); Render.redrawFx(); }
    });
    map.on('dblclick', e => { const rt = regTool(); if (rt) return Registry.call(rt, 'dbl', e); if (View.tool === 'road') finishRoad(); else if (View.tool === 'bus') finishBus(); });
    map.on('mouseout', () => { View.cursor = null; if (View.hover) { View.hover = null; Render.redrawFx(); } });
    map.on('contextmenu', e => {
      L.DomEvent.preventDefault(e.originalEvent);
      if (View.draft) finishDraft();
    });
  }

  function nearestNodeTo(latlng, maxM = 400) {
    const sc = Store.sc, deg = Net.degree(sc);
    return Net.nearestNode(sc, [latlng.lat, latlng.lng], maxM, n => (deg.get(n.id) || 0) > 0);
  }
  function addGateway(e) {
    const sc = Store.sc;
    const h = Render.hit(e.containerPoint, { zones: false, links: false, nodeR: 18 });
    const nid = h ? h.id : nearestNodeTo(e.latlng, 80);
    if (!nid) return toast('Click on or near the end of a road.');
    const Ls = Net.linksAt(sc, nid); if (!Ls.length) return;
    const L0 = Ls.reduce((m, l) => (CLASSES[l.cls].rank > CLASSES[m.cls].rank ? l : m), Ls[0]);
    const per = { motorway: 1500, ramp: 650, arterial: 550, collector: 320 }[L0.cls] || 200;
    const inL = L0.a === nid ? L0.lanesAB : L0.lanesBA, outL = L0.a === nid ? L0.lanesBA : L0.lanesAB;
    const name = (L0.name || CLASSES[L0.cls].label) + ' gateway';
    Kit.snap(`new gateway ${name}`);
    const id = U.uid('z');
    sc.zones[id] = { id, kind: 'gateway', name, lat: U.r7(e.latlng.lat), lng: U.r7(e.latlng.lng), node: nid,
      gw: { AMin: Math.round(inL * per * 0.85 / 10) * 10, AMout: Math.round(outL * per * 0.85 / 10) * 10, PMin: Math.round(inL * per * 0.85 / 10) * 10, PMout: Math.round(outL * per * 0.85 / 10) * 10 } };
    View.sel = { type: 'zone', id };
    Store.edited('demand');
    toast('Gateway added with flows estimated from the road class. Replace them with traffic counts if you have them.', '', { action: { label: 'Undo', run: () => undo() } });
  }
  function pickPath(e) {
    const nid = nearestNodeTo(e.latlng, 300);
    if (!nid) return toast('Click closer to a road.');
    if (View.pathPick.length >= 2) { View.pathPick = []; View.path = null; }
    View.pathPick.push(nid);
    const sc = Store.sc, n = id => [sc.nodes[id].lat, sc.nodes[id].lng];
    View.pathEnds = View.pathPick.map((id, i) => [n(id), i ? 'B' : 'A']);
    if (View.pathPick.length === 2) {
      const res = Store.res();
      const r = Engine.travel(sc, res, View.pathPick[0], View.pathPick[1]);
      let base = null;
      if (sc.id !== Store.P.base && Store.res(Store.baseSc) && Store.baseSc.nodes[View.pathPick[0]] && Store.baseSc.nodes[View.pathPick[1]]) base = Engine.travel(Store.baseSc, Store.res(Store.baseSc), View.pathPick[0], View.pathPick[1]);
      View.path = r && !r.none ? { ...r, base: base && !base.none ? base : null, congested: !!res } : { none: true };
    }
    renderPathCard(); Render.redrawFx();
  }
  function renderPathCard() {
    const el = U.$('#pathCard');
    el.hidden = View.tool !== 'path' || !View.path;
    if (el.hidden) return;
    const p = View.path;
    if (p.none) { el.innerHTML = '<div class="lg-title">Travel time</div><p class="note warn">No route between these points. Check one-way roads.</p>'; return; }
    const b = p.base;
    el.innerHTML = `<div class="lg-title">Travel time A → B</div>
      <div class="facts">
        <div class="fact"><b>${U.fmt.dur(p.time)}</b><span>${p.congested ? 'peak hour' : 'free flow'}</span></div>
        <div class="fact"><b>${U.fmt.len(p.len)}</b><span>distance</span></div>
        <div class="fact"><b>${U.fmt.num(p.len / p.time * 3.6, 0)} km/h</b><span>average speed</span></div>
        <div class="fact"><b>${U.fmt.dur(p.ff)}</b><span>with no traffic</span></div>
      </div>
      ${b ? `<p class="note" style="margin-top:8px">${esc(Store.baseSc.name)}: ${U.fmt.dur(b.time)} · this scenario is <b>${U.fmt.signed((p.time - b.time) / 60, 1)} min</b>.</p>` : ''}
      ${!p.congested ? '<p class="note" style="margin-top:6px">Run the model for peak-hour times.</p>' : ''}
      ${p.google ? `<p class="note" style="margin-top:8px">Google, ${esc(p.google.when)}: <b>${U.fmt.dur(p.google.t)}</b> (${U.fmt.dur(p.google.tFree)} with no traffic). Model is <b>${U.fmt.signedPct(p.time / p.google.t - 1, 0)}</b>.</p>` : ''}
      ${p.gErr ? `<p class="note warn" style="margin-top:6px">${esc(p.gErr)}</p>` : ''}
      <div class="actions" style="margin-top:8px"><button class="btn small" data-pc="google">Check with Google</button><button class="btn small" data-pc="jt">Add to journey checks</button></div>`;
  }
  async function pathCardClick(e) {
    const b = e.target.closest('[data-pc]'); if (!b || !View.path || View.pathPick.length < 2) return;
    const sc = Store.sc, [A, B] = View.pathPick, ll = id => [sc.nodes[id].lat, sc.nodes[id].lng];
    const nm = id => (Net.linksAt(sc, id).map(l => l.name).find(Boolean)) || 'point';
    if (b.dataset.pc === 'google') {
      if (!DataHub.keys.google) { Drawer.open('data'); return toast('Add your Google Maps Platform key in the Traffic data tab first.', 'warn'); }
      b.disabled = true; b.textContent = 'Asking Google…';
      try { View.path.google = await DataHub.googleTime(ll(A), ll(B), S().period); View.path.gErr = null; } catch (err) { View.path.gErr = 'Google: ' + err.message; }
      renderPathCard();
    } else {
      const d = DataHub.D();
      const j = { id: U.uid('j'), name: `${nm(A)} → ${nm(B)}`, from: A, to: B, fromLL: ll(A), toLL: ll(B), g: {} };
      if (View.path.google) j.g[S().period] = View.path.google;
      d.jt.push(j); Store.emit('data');
      Drawer.open('data');
      toast('Added to journey-time checks in the Traffic data tab.');
    }
  }
  // Drive times use the model (congested once run); walk and cycle catchments are Spare Construct assumptions (Kit.catchment)
  function isoFrom(nid) {
    if (View.isoMode === 'walk' || View.isoMode === 'cycle') return Kit.catchment(Store.sc, nid, View.isoMode);
    const r = Engine.isochrone(Store.sc, Store.res(), nid);
    return r ? { ...r, mode: 'drive', basis: Store.res() ? 'Driving, using congested peak-hour travel times from the model.' : 'Driving at free-flow speeds; run the model for congested times.' } : null;
  }
  function pickIso(e) {
    const nid = nearestNodeTo(e.latlng, 300);
    if (!nid) return toast('Click closer to a road.');
    View.iso = isoFrom(nid);
    if (View.colorBy !== 'iso') View.prevColour = View.colorBy;
    View.colorBy = 'iso';
    renderLegend(); renderStale(); Render.redraw();
  }

  function select(h) {
    View.sel = h;
    Inspector.render(); Render.redrawFx();
    Registry.fire('select', View.sel);
  }
  function flyTo(type, id) {
    const sc = Store.sc;
    if (type === 'link' && sc.links[id]) map.fitBounds(L.latLngBounds(Net.geom(sc, sc.links[id])).pad(0.6), { maxZoom: 17 });
    else if (type === 'node' && sc.nodes[id]) map.setView([sc.nodes[id].lat, sc.nodes[id].lng], Math.max(map.getZoom(), 16));
    else if (type === 'zone' && sc.zones[id]) map.setView([sc.zones[id].lat, sc.zones[id].lng], Math.max(map.getZoom(), 15));
    else if (type === 'count') { const c = DataHub.D().counts.find(x => x.id === id); if (c) map.setView([c.lat, c.lng], Math.max(map.getZoom(), 16)); }
    select({ type, id });
  }

  function bindKeys() {
    document.addEventListener('keydown', e => {
      const typing = /^(INPUT|SELECT|TEXTAREA)$/.test(e.target.tagName) || e.target.isContentEditable;
      const modal = Kit.modalOpen();
      const ctrl = e.ctrlKey || e.metaKey;
      // the active registered tool sees keys first (return true to consume)
      const rt = regTool();
      if (rt && !modal && !typing && Registry.call(rt, 'keydown', e) === true) { e.preventDefault(); return; }
      // while drawing, Backspace, Delete and Ctrl+Z remove the last point (draft-bug), ahead of undo and delete
      if (!modal && !typing && !Kit.menuOpen() && View.draft && ((!ctrl && !e.altKey && (e.key === 'Backspace' || e.key === 'Delete')) || (ctrl && !e.shiftKey && e.key.toLowerCase() === 'z'))) { e.preventDefault(); popDraft(); return; }
      if (e.key === 'Escape') {
        if (modal) return Kit.closeModal(null);
        if (Kit.menuOpen()) return Kit.hideMenu();
        if (typing) { e.target.blur(); return; }
        if (View.draft) { View.draft = null; draftChanged(); return; }
        if (View.measure.length) { View.measure = []; Render.redrawFx(); return; }
        if (View.tool !== 'select') return setTool('select');
        if (View.sel) return select(null);
        return;
      }
      if (modal) return;
      // registered commands with their own keys (not the built-ins listed with bound: true)
      for (const c of Registry.list('commands', c => c.keys && !c.bound)) {
        if ((typing && !ctrl && !e.altKey) || !Registry.matchKeys(c.keys, e)) continue;
        if (c.enabled && !Registry.call(c, 'enabled')) continue;
        e.preventDefault(); Registry.call(c, 'run'); return;
      }
      if (ctrl && e.key.toLowerCase() === 's') { e.preventDefault(); return saveToFile(e.shiftKey); }
      if (ctrl && e.key === 'Enter') { e.preventDefault(); return e.shiftKey ? runModel(Store.P.scenarios, ['AM', 'PM']) : runModel(); }
      if (typing) return;
      if (Kit.menuOpen()) return;
      if (ctrl && e.key.toLowerCase() === 'z') { e.preventDefault(); undo(e.shiftKey); return; }
      if (ctrl && e.key.toLowerCase() === 'y') { e.preventDefault(); undo(true); return; }
      if (ctrl || e.altKey) return;
      // keys on buttons, links and the drawer tabs keep their own meaning
      const onControl = e.target.closest && e.target.closest('button, a, [role="tab"], [role="separator"], [role="button"]');
      // the shrunk '?' hint chip works like a button
      if ((e.key === 'Enter' || e.key === ' ') && e.target.id === 'hint' && e.target.getAttribute('role') === 'button') { e.preventDefault(); e.target.click(); return; }
      if (e.key === 'Enter') { if (onControl) return; if (View.tool === 'road' || View.tool === 'bus') finishDraft(); return; }
      if (e.key === 'Backspace' && View.tool === 'measure' && View.measure.length) { e.preventDefault(); View.measure.pop(); Render.redrawFx(); return; }
      /* Delete and Backspace remove the selection, but never while a drawing tool is active (draft-bug): the road
         just finished stays selected, and one key press too many must not delete it */
      if (e.key === 'Delete' || e.key === 'Backspace') {
        if (View.tool === 'road' || View.tool === 'bus') { e.preventDefault(); if (View.sel) toast('While a drawing tool is on, Delete and Backspace only remove drawn points. To delete the selection, use Delete in the side panel or switch to Select (V).'); return; }
        if (View.sel) { e.preventDefault(); deleteSelected(); }
        return;
      }
      if (e.key === ' ') { if (onControl) return; e.preventDefault(); if (View.sim) { View.simPlaying = !View.simPlaying; renderHud(); } else toggleSim(); return; }
      if (e.key === '?') return Dialogs.help();
      // number keys pick the colour mode (map-card: 1 road class … 7 change vs base … 9 cycling stress)
      if (/^[1-9]$/.test(e.key)) { const m = Registry.list('modes', m => !m.hidden && m.key === e.key)[0]; if (m) { setColour(m.id); return; } }
      const t = railTools().find(t => t.key && t.key.toLowerCase() === e.key.toLowerCase());
      if (t) setTool(t.id, true);
    });
  }

  const KIND_NAME = { link: 'road', node: 'junction', zone: 'zone', route: 'bus route' };
  function deleteSelected() {
    const s = View.sel, sc = Store.sc; if (!s) return;
    // registered selection kinds delete through their panel's onDelete(sel)
    if (!/^(link|node|zone|route)$/.test(s.type)) { const p = Registry.get('panels', s.type); if (p && p.onDelete) Registry.call(p, 'onDelete', s); return; }
    const o = s.type === 'link' ? sc.links[s.id] : s.type === 'node' ? sc.nodes[s.id] : s.type === 'zone' ? sc.zones[s.id] : sc.routes[s.id];
    if (!o) return;
    const name = s.type === 'node' ? Kit.junctionName(s.id, sc) : o.name || (s.type === 'link' ? CLASSES[o.cls].label + ' road' : KIND_NAME[s.type]);
    const what = `${s.type === 'node' ? 'junction' : s.type === 'zone' && o.kind === 'gateway' ? 'gateway' : KIND_NAME[s.type]} ${name}`;
    Kit.snap(`deleting ${what}`);
    if (s.type === 'link') Net.deleteLink(sc, s.id);
    else if (s.type === 'node') Net.deleteNode(sc, s.id);
    else if (s.type === 'zone') delete sc.zones[s.id];
    else if (s.type === 'route') delete sc.routes[s.id];
    View.sel = null; View.hover = null;
    Store.edited(s.type === 'zone' ? 'demand' : 'net');
    toast(`Deleted ${what}.`, '', { action: { label: 'Undo', run: () => undo() } });
  }
  function setColour(k) {
    View.colorBy = k;
    if (k !== 'iso') View.iso = null;
    if (RES_MODES.has(k) && !Store.res() && k !== 'select') toast('Run the model (Ctrl+Enter) to see results.');
    Inspector.render(); renderLegend(); renderStale(); Render.redraw();
  }
  function log(html) { logLines.push(`<span style="color:var(--faint)">${new Date().toLocaleTimeString('en-NZ')}</span>  ${html}`); if (logLines.length > 300) logLines.shift(); if (View.drawer === 'log' && map) Drawer.render(); }
  // Menu › Run log (wording: the log is no longer a drawer tab)
  function runLog() {
    const body = logLines.slice().reverse().map(l => `<div class="log-line">${l}</div>`).join('') || '<p class="note">Nothing yet.</p>';
    return Kit.modal('Run log', `<p class="note">Model runs, imports and data downloads in this session, newest first.</p>${body}`, [['Copy', () => { Kit.copyText(logLines.map(l => l.replace(/<[^>]+>/g, '')).join('\n')).then(ok => toast(ok ? 'Run log copied.' : 'Could not copy the log.', ok ? '' : 'warn')); return null; }, ''], ['Close', null]], true);
  }

  return {
    init, toast, busy, refresh, select, flyTo, setTool, tools, runModel, runStale, staleRuns, runImport: (b, o) => Dialogs.runImport(b, o), toggleSim, stopSim, setColour, setBasemap, setLive,
    deleteSelected, renderLegend, renderTop, renderTopbar, renderBanners, mapControl, undo, saveToFile, manageProjects, openRecent, refreshRecent, runLog, applyTheme, popDraft,
    get recent() { return recent; }, recentLine, pref,
    // after registering tools, top-bar items or layers late (tests, the console): rebuild the shell from the registry
    rebuild() { registerBuiltins(); for (const l of Registry.list('layers')) if (!(l.id in View.show)) View.show[l.id] = !!l.defaultOn; buildTools(); buildTopbarItems(); refresh(); }, openTab: id => Drawer.open(id), exportFile, exportItems, mainMenu, fitTop, roadProps, fileBase, log, logLines, BASEMAPS,
    get map() { return map; },
  };
})();
