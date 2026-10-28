'use strict';
/* Spare Construct · Transport on the frame's ribbon. Opened as transport/?ribbon inside Spare Construct, this page hides
   its own tool strip and the frame shows Transport's tabs in its ribbon, in the same place as every other discipline's,
   so the view tabs around the map do not move when the discipline changes. The frame builds those tabs from what is
   registered here (tools, top-bar items, menu items and exports, plug-ins included) and runs them through the same
   functions the tool strip calls, so nothing behaves differently. Without ?ribbon the tool strip shows as before. */
const FrameRibbon = (() => {
  const on = document.documentElement.classList.contains('in-ribbon');   // set in index.html before the page draws
  const subs = new Set();
  let t = 0, watching = false;
  function changed() { clearTimeout(t); t = setTimeout(() => subs.forEach(f => { try { f(); } catch (e) { console.error('Spare Construct ribbon update failed:', e); } }), 30); }
  // Model edits, runs and selections come through the store; the tool, Simulate and Undo show on the (hidden) strip
  function watch() {
    if (watching) return;
    watching = true;
    Store.on(changed);
    const top = document.getElementById('top');
    if (top && window.MutationObserver) new MutationObserver(changed).observe(top, { subtree: true, attributes: true, childList: true, characterData: true });
  }
  const label = d => typeof d.label === 'function' ? Registry.call(d, 'label') : d.label;
  const enabled = d => !(d.enabled && !Registry.call(d, 'enabled'));
  return {
    on,
    subscribe(fn) { subs.add(fn); watch(); return () => subs.delete(fn); },
    state() {
      const u = Store.undoStack, last = u[u.length - 1], eb = document.getElementById('engineBadge');
      return { tool: View.tool, sim: !!View.sim, stale: UI.staleRuns().length, undo: u.length ? (last && last.label) || 'the last change' : '', redo: Store.redoStack.length > 0,
        engine: eb ? eb.textContent.trim() : '', period: Store.S.period, scenario: Store.sc ? Store.sc.name : '' };
    },
    tools: () => UI.tools().map(d => ({ id: d.id, label: d.label, short: d.short || d.label, key: d.key || '', group: d.group || 'analyse', svg: d.iconSvgPath || '' })),
    setTool: id => UI.setTool(id, true),
    run: () => UI.runModel(),
    // The same choices as the ▾ beside Run model
    runChoices() {
      const st = UI.staleRuns(), p = Store.S.period;
      return [
        { label: 'This scenario, AM and PM', keys: 'Ctrl+Enter', run: () => UI.runModel() },
        { label: `This scenario, ${p} peak only`, run: () => UI.runModel([Store.sc], [p]) },
        { label: 'All scenarios, AM and PM', keys: 'Ctrl+Shift+Enter', run: () => UI.runModel(Store.P.scenarios, ['AM', 'PM']) },
        { label: `Only out-of-date results (${st.length})`, disabled: !st.length, run: () => UI.runStale() },
      ];
    },
    simulate: () => UI.toggleSim(),
    importRoads: () => Dialogs.importRoads(),
    undo: () => UI.undo(),
    redo: () => UI.undo(true),
    topbar: () => Registry.list('topbar', d => !!d.run).map(d => ({ id: d.id, label: d.label || d.title || d.id, tip: d.title || '', run: () => Registry.call(d, 'run') })),
    menu: () => Registry.list('menu').map(d => ({ id: d.id, label: label(d), keys: d.keys || '', danger: !!d.danger, disabled: !enabled(d), run: () => Registry.call(d, 'run') })),
    // The whole ☰ Menu, as the strip shows it: separators are null, and Open recent and Export have sub-lists
    mainMenu: () => UI.mainMenu(),
    exports: () => UI.exportItems(),
    refreshRecent: () => UI.refreshRecent(),
  };
})();
window.FrameRibbon = FrameRibbon;
