'use strict';
/* Spare Construct · dialogs: welcome, import roads, settings, help, about the data, gradient profile,
   and opening and saving files. Registered settings sections, About-the-data rows, welcome
   entries, tools and commands appear in the matching dialog. */

const Dialogs = (() => {
  const S = () => Store.S;
  const { modal, closeModal, toast, busy, fact } = Kit;
  const esc = U.esc;

  /* ---------------- welcome ---------------- */
  // Projects kept in this browser (storage (2)), newest first, for the welcome dialog
  function recentHtml() {
    const open = Store.P && Store.P.id;
    const list = (typeof UI !== 'undefined' && UI.recent || []).filter(m => m.id !== open).slice(0, 5);
    if (!list.length) return '';
    return `<h3>Recent projects in this browser</h3><div class="recent">${list.map(m => `<div class="recent-row"><span class="r-name"><b>${esc(m.name)}</b><span>${esc(UI.recentLine(m))}</span></span><button type="button" class="btn small" data-w="open:${esc(m.id)}">Open</button></div>`).join('')}</div>`;
  }
  // 'Where is your study area?' at the top of the welcome dialog (onboarding): the search opens the place finder
  const finder = () => typeof Onboarding !== 'undefined' && typeof Onboarding.openFinder === 'function';
  const findHtml = () => !finder() ? '' : `<form class="w-find" data-w-find role="search" aria-labelledby="wFindLbl">
      <label id="wFindLbl" for="wFind"><b>Where is your study area?</b></label>
      <div class="w-find-row"><input id="wFind" type="search" autocomplete="off" spellcheck="false" placeholder="Address, road or suburb, e.g. Waihoehoe Road, Drury"><button type="submit" class="btn primary">Find</button></div>
      <p class="note">Pick the place, then draw a box: Spare Construct downloads the roads, counts, crashes and existing land use, and runs a base model.</p></form>`;
  // Open the finder with what was typed here, so its suggestions start straight away
  function findArea(q) {
    if (!finder()) return;
    Onboarding.openFinder();
    const i = Kit.hasDom() && U.$('#onbFindIn');
    if (i && q) { i.value = q; i.dispatchEvent(new Event('input', { bubbles: true })); i.focus(); }
  }
  function welcomeHtml() {
    const extra = Registry.list('welcome');
    return `
      ${findHtml()}
      <p>Spare Construct puts a strategic traffic model and a microsimulation on a live map. Bring in the existing streets, sketch new roads and developments, and compare scenarios by volume, ${Kit.gl('LoS')}, travel time, emissions and an indicative benefit–cost ratio.</p>
      ${recentHtml()}
      <div class="welcome">
        <button class="opt" data-w="drury"><b>Open the Drury sample</b><span>Real streets around Drury, south Auckland, with growth-area housing, a proposed east–west link and signals already set up and modelled.</span></button>
        ${extra.map(w => `<button class="opt" data-w="reg:${esc(w.id)}"><b>${esc(w.title)}</b>${w.text ? `<span>${esc(w.text)}</span>` : ''}</button>`).join('')}
        <button class="opt" data-w="cbd"><b>Start in Auckland city centre</b><span>Jumps to the CBD and opens the road import so you can bring in the streets around you.</span></button>
        <button class="opt" data-w="blank"><b>Start with a blank map</b><span>Draw everything yourself, or pan to your site and use Import roads.</span></button>
      </div>
      <p class="note">Every source and assumption is listed under <b>Menu › About the data</b>.</p>`;
  }
  // The welcome dialog does not close on a backdrop click (accessibility (2)): Esc or a choice closes it
  function welcome() {
    modal('Model new roads and their traffic', welcomeHtml(), [['Close', null]], { dismiss: false });
    if (!Kit.hasDom()) return;
    const form = U.$('#modal [data-w-find]');
    if (form) form.addEventListener('submit', e => { e.preventDefault(); findArea(U.$('#wFind').value.trim()); });
    U.$('#modal .m-body').onclick = e => {
      const b = e.target.closest('[data-w]'); if (!b) return;
      closeModal(null);
      const w = b.dataset.w;
      if (w.startsWith('open:')) return UI.openRecent(w.slice(5));
      if (w === 'drury') Sample.drury();
      if (w === 'cbd') { UI.map.setView([-36.8505, 174.7645], 16); setTimeout(importRoads, 300); }
      if (w.startsWith('reg:')) Registry.call(Registry.get('welcome', w.slice(4)), 'run');
    };
  }

  /* ---------------- import roads ---------------- */
  function importRoads() {
    const map = UI.map, b = map.getBounds(), w = U.dist([b.getCenter().lat, b.getWest()], [b.getCenter().lat, b.getEast()]), h = U.dist([b.getSouth(), b.getCenter().lng], [b.getNorth(), b.getCenter().lng]);
    const big = w * h > 36e6, multi = Store.P.scenarios.length > 1;
    modal('Import roads from OpenStreetMap', `
      <p>Downloads every road inside the current map view (${U.fmt.num(w / 1000, 1)} × ${U.fmt.num(h / 1000, 1)} km) with its class, lanes, speed limit, one-way rules, roundabouts and signals.</p>
      ${big ? '<p class="note warn">That is a large area. Zoom in for a faster, cleaner import; large networks also make the model slower.</p>' : ''}
      <label class="check"><input type="checkbox" id="imLocal" checked> Include local and residential streets</label>
      <label class="check"><input type="checkbox" id="imService"> Include service lanes and alleys</label>
      <label class="check"><input type="checkbox" id="imGw" checked> Add gateways where main roads leave the area, with estimated through traffic</label>
      ${multi ? `<label class="check"><input type="checkbox" id="imAll" checked> Add to every scenario (keeps them comparable)</label>` : ''}
      <p class="note">Road data © OpenStreetMap contributors, ODbL. Check lanes and controls on key roads after importing: OpenStreetMap is good but not survey-grade.</p>`,
      [['Cancel', null, ''], ['Import roads', () => ({ local: U.$('#imLocal').checked, service: U.$('#imService').checked, gw: U.$('#imGw').checked, all: multi ? U.$('#imAll').checked : false })]])
      .then(o => o && runImport(map.getBounds(), o));
  }
  /* The import runs as a cancellable task (results-trust (f)): Cancel aborts the download, and when a server refuses
     the busy box says so before trying the next one ('overpass-api.de busy (HTTP 429); trying maps.mail.ru…'). */
  async function runImport(bounds, o) {
    let data;
    try {
      data = await Kit.task('Downloading roads from OpenStreetMap…', (signal, status, info) => Importers.overpass(bounds, o, t => {
        const m = /from (\S+?)…/.exec(t), f = info.fail;
        status(f && m && f.host && f.host !== m[1] ? `${f.host} ${f.status ? `busy (HTTP ${f.status})` : 'did not answer'}; trying ${m[1]}…` : t);
      }));
    } catch (e) {
      if (Kit.isAbort(e)) { toast('Road import cancelled. Nothing was changed.'); return 0; }
      toast('The road download failed: ' + e.message + '. Try again, or zoom in to a smaller area.', 'err', { details: String(e.stack || e) });
      return 0;
    }
    busy('Building the network…');
    try {
      await U.sleep(10);
      const sc = Store.sc;
      Kit.snap("importing roads");
      const before = { n: new Set(Object.keys(sc.nodes)), l: new Set(Object.keys(sc.links)), z: new Set(Object.keys(sc.zones)) };
      const n = Importers.osmToScenario(sc, data);
      // gateways only at this import's new road ends, never on roads imported elsewhere earlier
      const fresh = new Set(Object.keys(sc.nodes).filter(id => !before.n.has(id)));
      const g = o.gw ? Importers.autoGateways(sc, bounds, fresh) : 0;
      if (o.all) for (const other of Store.P.scenarios) {
        if (other === sc) continue;
        for (const id of Object.keys(sc.nodes)) if (!before.n.has(id) && !other.nodes[id]) other.nodes[id] = JSON.parse(JSON.stringify(sc.nodes[id]));
        for (const id of Object.keys(sc.links)) if (!before.l.has(id) && !other.links[id]) other.links[id] = JSON.parse(JSON.stringify(sc.links[id]));
        for (const id of Object.keys(sc.zones)) if (!before.z.has(id) && !other.zones[id]) other.zones[id] = JSON.parse(JSON.stringify(sc.zones[id]));
        for (const r of Object.values(other.res)) if (r) r.stale = true;
      }
      Store.edited();
      toast(n ? `Imported ${U.fmt.int(n)} road segments${g ? ` and ${g} gateways` : ''}.` : 'No new roads found in this view.');
      Kit.log(`Imported ${n} segments from OpenStreetMap${g ? `, ${g} gateways` : ''}.`);
      if (View.colorBy !== 'class' && !Store.res()) UI.setColour('class');
      return n;
    } catch (e) {
      console.error(e);
      toast('The roads downloaded but could not be added to the network: ' + e.message + '. Undo (Ctrl+Z) if anything looks wrong, and copy the details for a bug report.', 'err', { details: String(e.stack || e) });
      return 0;
    } finally { busy(null); }
  }

  /* ---------------- settings ----------------
     settingsHtml(s, D) builds the body from a copy of the settings: inputs carry data-set="path" (data-null: blank
     means 'not set'). Each field that differs from Spare Construct's defaults D shows a dot (tooltip 'Default 0.85') and a
     per-field reset, and the header counts them (about-data (d)). Display options with data-pref are kept per
     browser, not in the project. */
  function settingsHtml(s, D = Kit.settingDefaults()) {
    const def = k => Kit.getPath(D, k);
    const same = (k, v = Kit.getPath(s, k)) => { const d = def(k); return d === undefined || (d == null && (v == null || v === '')) || d === v || (typeof d === 'number' && v !== '' && v != null && +v === d); };
    const mark = (k, label) => def(k) === undefined ? '' : `<span class="chg" data-chg="${esc(k)}" title="Changed. Default ${esc(Kit.valText(def(k)))}" ${same(k) ? 'hidden' : ''}>●</span><button type="button" class="fld-reset" data-reset="${esc(k)}" title="Reset to the default (${esc(Kit.valText(def(k)))})" aria-label="Reset ${esc(label.replace(/<[^>]+>/g, ''))} to its default" ${same(k) ? 'hidden' : ''}>↺</button>`;
    const n = (k, label, unit, step = 'any', o = {}) => { const v = Kit.getPath(s, k);
      return `<label class="fld"><span>${label}</span><span class="fld-in"><input type="number" step="${esc(step)}" data-set="${esc(k)}"${o.null ? ' data-null="1"' : ''} value="${esc(v ?? '')}"${o.ph ? ` placeholder="${esc(o.ph)}"` : ''}>${unit ? `<span class="unit">${unit}</span>` : ''}${mark(k, label)}</span></label>`; };
    const t = (k, label, ph = '') => `<label class="fld"><span>${label}</span><span class="fld-in"><input type="text" data-set="${k}" value="${esc(Kit.getPath(s, k) ?? '')}" placeholder="${esc(ph)}">${mark(k, label)}</span></label>`;
    const sel = (k, label, pairs) => `<label class="fld"><span>${label}</span><span class="fld-in"><select data-set="${k}">${Kit.opt(pairs, Kit.getPath(s, k))}</select>${mark(k, label)}</span></label>`;
    const chk = (k, label) => `<label class="check"><input type="checkbox" data-set="${k}" ${Kit.getPath(s, k) ? 'checked' : ''}> ${label}${mark(k, label)}</label>`;
    const pref = (k, label, on) => `<label class="check"><input type="checkbox" data-pref="${k}" ${on ? 'checked' : ''}> ${label}</label>`;
    const P = typeof UI !== 'undefined' && UI.pref ? UI.pref : { get: (k, d) => d };
    const rates = Object.entries(LANDUSES).filter(([k]) => s.rates[k]).map(([k, lu]) => `<tr><td>${lu.label} <span class="note">per ${lu.per === 100 ? '100 m² GFA' : lu.unit.replace(/s$/, '')}</span></td>
      ${['AM', 'PM'].map(p => `<td class="n"><input type="number" step="0.05" data-set="rates.${k}.${p}.rate" value="${esc((s.rates[k][p] || {}).rate ?? '')}" aria-label="${esc(lu.label)} ${p} rate">${mark(`rates.${k}.${p}.rate`, `${lu.label} ${p} rate`)}</td><td class="n"><input type="number" step="5" data-set="rates.${k}.${p}.inPct" value="${esc((s.rates[k][p] || {}).inPct ?? '')}" aria-label="${esc(lu.label)} ${p} percent arriving">${mark(`rates.${k}.${p}.inPct`, `${lu.label} ${p} % in`)}</td>`).join('')}</tr>`).join('');
    const extra = Registry.list('settings').map(d => `<h3>${esc(d.title)}</h3><div class="reg-set" data-set-sec="${esc(d.id)}">${Registry.html(d, 'render', s)}</div>`).join('');
    const changed = Kit.nonDefaults(s, D).length, E = s.econ || {}, mb = E.method !== 'uniform';
    const turns = s.turns ? `<h3>Turn penalties</h3><div class="set-grid">${n('turns.left', 'Left turn', 's', 0.5)}${n('turns.right', 'Right turn', 's', 0.5)}${n('turns.rightSignals', 'Right turn at signals', 's', 0.5)}
        ${sel('turns.uturn', 'U-turns', { ban: 'Only at dead ends and where allowed', ...(s.turns.uturn !== 'ban' && isFinite(+s.turns.uturn) ? { [+s.turns.uturn]: `Allowed, ${+s.turns.uturn} s penalty` } : {}), 30: 'Allowed, 30 s penalty', 60: 'Allowed, 60 s penalty' })}</div>
        <p class="note">Indicative extra time for turning at a junction, on top of junction delay.</p>` : '';
    return `
      <p class="set-count">Changed from defaults (<b>${changed}</b>). ${changed ? 'Changed fields have a dot; ↺ puts one back to its default.' : 'Every setting is at Spare Construct’s default.'}</p>
      <h3>Demand</h3><div class="set-grid">
        ${n('baseYear', 'Base year', '', 1)}${n('year', 'Default forecast year', '', 1)}${n('growth', 'Background growth', '% a year', 0.1)}
        ${sel('growthMode', 'Growth method', { linear: 'Linear: % of base-year traffic a year (MBCM)', compound: 'Compound: % of the previous year' })}
        ${n('hvPct', 'Heavy vehicles', '%', 0.5)}${n('pcuHV', `${Kit.gl('pcu')} per heavy vehicle`, '', 0.1)}
        ${n('gravityBeta', 'Trip-length sensitivity (gravity β) — higher = shorter trips', 'per min', 0.01)}${n('connectors', 'Zone access lines per zone', '', 1)}${n('connectorSpeed', 'Zone access line speed', 'km/h', 1)}${n('peakToDaily', 'Peak hour to daily factor', '×', 0.5)}
      </div>
      <p class="note">Each scenario can have its own forecast year (the Year box in the top bar). The default applies to scenarios without one.</p>
      <h3>Trip rates (vehicle trips in the peak hour)</h3>
      <div class="tbl-wrap"><table class="t compact"><tr><th>Land use</th><th class="n">AM rate</th><th class="n">AM % in</th><th class="n">PM rate</th><th class="n">PM % in</th></tr>${rates}</table></div>
      <p class="note">Defaults are indicative, in the range of NZ and RTA guide rates. Replace them with surveyed or NZTA Research Report 453 rates for real assessments.</p>
      <h3>Assignment and junctions</h3><div class="set-grid">
        ${n('bprAlpha', `Speed-flow curve α (${Kit.gl('BPR')})`, '', 0.01)}${n('bprBeta', `Speed-flow curve β (${Kit.gl('BPR')})`, '', 0.1)}${n('satFlow', 'Saturation flow', 'pcu/h/lane', 50)}${n('cycle', 'Default signal cycle', 's', 5)}
        ${n('lostPerPhase', 'Lost time per phase', 's', 1)}${n('maxIter', 'Maximum iterations', '', 1)}${n('gapTol', 'Convergence target (relative gap)', '', 0.00001)}
        ${n('busPcu', `${Kit.gl('pcu')} per bus (bus route preload)`, 'pcu', 0.1)}
        ${sel('conv.capDamp', 'Junction capacities during assignment', { msa: 'Damped (steadier, MSA)', none: 'Undamped (as before v2)' })}
        ${chk('conv.stable', 'Stop only when link flows are also stable (TAG M3.1: 98% of links change under 1% for 4 iterations)')}
        ${chk('signalOpt', 'Optimise signal timings to the flows')}
      </div>
      <p class="note">A relative gap of 0.0001 (0.01%) is the target for SIDRA hand-offs. The model says when it stops short of the target.</p>
      ${turns}
      <h3>Economics (indicative)</h3><div class="set-grid">
        ${sel('econ.method', 'Discounting', { mbcm: 'MBCM v1.7.5 (NZTA GC 25/01): 2% to year 30, 1.5% to year 100, then 1%', uniform: 'One flat rate (older projects)' })}
        ${sel('econ.activity', 'Activity (NZTA GC 25/01)', { 'non-commercial': 'Non-commercial: the schedule below, 8% sensitivity', commercial: 'Commercial: 8% for all years, 2% sensitivity' })}
        ${n('econ.vot', 'Value of time', '$ / veh·h', 0.5)}${chk('econ.votUpdate', `Apply the ${Kit.gl('MBCM')} update factor to the value of time`)}
        ${mb ? `${n('econ.periodHours.AM', 'AM peak hours a year', 'h', 10)}${n('econ.periodHours.PM', 'PM peak hours a year', 'h', 10)}${n('econ.periodHours.IP', 'Inter-peak hours a year', 'h', 10)}
          ${n('econ.rates.0.r', 'Discount rate, years 1–30', '%', 0.1)}${n('econ.rates.0.to', 'First period ends at year', '', 1)}${n('econ.rates.1.r', 'Discount rate, years 31–100', '%', 0.1)}${n('econ.rates.1.to', 'Second period ends at year', '', 1)}${n('econ.rates.2.r', 'Discount rate, from year 101', '%', 0.1)}${n('econ.sensRate', 'Sensitivity discount rate', '%', 0.5)}
          ${n('econ.benefitGrowth', 'Benefit growth', '% a year', 0.1, { null: true, ph: 'same as traffic growth' })}${chk('econ.p95', 'A P95 cost estimate has been prepared')}`
          : `${n('econ.annualHours', 'Peak hour to year', 'hours', 50)}${n('econ.discount', 'Discount rate', '%', 0.5)}`}
        ${n('econ.years', 'Evaluation period', 'years', 1)}${mb ? `${t('econ.longLifeNote', 'Why more than 40 years', 'needed for 41 to 60 years')}${t('econ.approval', 'Approval for more than 60 years', 'who approved it, and when')}` : ''}
        ${Object.keys(CLASSES).map(k => n('econ.laneKm.' + k, CLASSES[k].label + ' road cost', '$m / lane-km', 0.5)).join('')}
        ${['signals', 'roundabout', 'priority'].map(k => n('econ.node.' + k, CONTROLS[k] + ' cost', '$m', 0.1)).join('')}
      </div>
      <p class="note">An indicative efficiency rating at the level of the NZTA Investment Efficiency Rating (IER), not a funding-approval ${Kit.gl('BCR')}.</p>
      <h3>Maps</h3>
      <label class="fld"><span>LINZ API key</span><input type="password" autocomplete="off" data-key="linz" value="${esc(Kit.linzKey() || s.linzKey || '')}" placeholder="Free from basemaps.linz.govt.nz"></label>
      <p class="note">Kept in this browser only; it is not saved in project files.</p>
      <h3>Traffic counts</h3><div class="set-grid">
        ${n('data.amShare', 'AM peak hour share of daily', '%', 0.5)}${n('data.pmShare', 'PM peak hour share of daily', '%', 0.5)}${n('data.matchRadius', 'Match counts to roads within', 'm', 5)}
      </div>
      <p class="note">The shares turn daily counts into peak-hour volumes where a count has no measured peak hour of its own.</p>
      <h3>Simulation</h3><div class="set-grid">${n('sim.maxVeh', 'Vehicle limit', '', 500)}</div>
      <h3>Display (this browser only)</h3><div class="set-grid">
        ${pref('cbSafe', `Colour-blind-safe colours for ${Kit.gl('V/C')} bands and level of service`, P.get('cbSafe', false))}
        ${pref('toolNames', 'Show tool names on the tool rail', typeof document !== 'undefined' && document.body && document.body.classList ? document.body.classList.contains('tool-names') : false)}
      </div>${extra}`;
  }
  // Read the dialog's inputs into s (numbers stay numbers; data-null blanks become null)
  function readSettings(root, s) {
    for (const el of U.$$('[data-set]', root)) {
      const v = el.type === 'checkbox' ? el.checked : el.type === 'number' ? (el.value === '' ? (el.dataset.null ? null : Kit.getPath(s, el.dataset.set)) : +el.value) : el.value.trim();
      Kit.setPath(s, el.dataset.set, el.tagName === 'SELECT' && v !== '' && isFinite(+v) && typeof Kit.getPath(s, el.dataset.set) === 'number' ? +v : v);
    }
    s.connectors = U.clamp(Math.round(s.connectors), 1, 4);
    if (s.turns && s.turns.uturn !== 'ban' && isFinite(+s.turns.uturn)) s.turns.uturn = +s.turns.uturn;
    // discount segments: a blank segment (an older two-segment project with the third left empty) is dropped
    if (s.econ && Array.isArray(s.econ.rates)) s.econ.rates = s.econ.rates.filter(x => x && x.r !== '' && x.r != null && isFinite(+x.r))
      .map(x => ({ ...x, r: +x.r, to: x.to === '' || x.to == null || !isFinite(+x.to) ? null : +x.to }));
    return s;
  }
  const staleAll = () => { for (const sc of Store.P.scenarios) for (const r of Object.values(sc.res || {})) if (r) r.stale = true; };
  async function settings() {
    const s = JSON.parse(JSON.stringify(Registry.applyDefaults(S()))), D = Kit.settingDefaults();
    const p = modal('Model settings', settingsHtml(s, D), [['Reset all to defaults', 'reset', ''], ['Cancel', null, ''], ['Save settings', 'save']], true);
    if (Kit.hasDom()) {
      const root = U.$('#modal .m-body');
      // live markers: show or hide the dot and reset button as a field changes; ↺ puts the default back
      const sync = el => { const k = el.dataset.set; if (!k) return; const v = el.type === 'checkbox' ? el.checked : el.type === 'number' ? (el.value === '' ? null : +el.value) : el.value, d = Kit.getPath(D, k);
        const eq = d === v || (d == null && (v == null || v === '')) || (typeof d === 'number' && v != null && v !== '' && +v === d) || String(d) === String(v);
        for (const m of U.$$(`[data-chg="${CSS.escape(k)}"], [data-reset="${CSS.escape(k)}"]`, root)) m.hidden = eq; };
      root.addEventListener('input', e => sync(e.target));
      root.addEventListener('change', e => sync(e.target));
      root.addEventListener('click', e => {
        const b = e.target.closest('[data-reset]'); if (!b) return;
        e.preventDefault();
        const k = b.dataset.reset, d = Kit.getPath(D, k), el = U.$(`[data-set="${CSS.escape(k)}"]`, root); if (!el) return;
        if (el.type === 'checkbox') el.checked = !!d; else el.value = d ?? '';
        sync(el); el.focus();
      });
    }
    const v = await p;
    if (v === 'reset') {
      if (!(await Kit.confirmBox('Reset to defaults?', 'Reset all trip rates, economics and model settings to their defaults? Results will go out of date.', 'Reset to defaults'))) return;
      const cur = S(), keep = { period: cur.period, basemap: cur.basemap, linzKey: cur.linzKey, display: cur.display };
      Kit.snapSettings('resetting settings to the defaults');
      Store.P.settings = Registry.applyDefaults({ ...defaultSettings(), ...keep });
      delete Store.P.settingsNotice;
      staleAll(); Store.emit('settings');
      return toast('Settings reset to the defaults. Run the model again to apply them.', '', { action: { label: 'Undo', run: () => UI.undo() } });
    }
    if (v !== 'save') return;
    if (Kit.hasDom()) {
      readSettings(U.$('#modal'), s);
      for (const el of U.$$('#modal [data-pref]')) {
        if (el.dataset.pref === 'cbSafe') { UI.pref.set('cbSafe', el.checked); UI.applyTheme(); }
        if (el.dataset.pref === 'toolNames' && el.checked !== document.body.classList.contains('tool-names')) U.$('#tools [data-rail="names"]') && U.$('#tools [data-rail="names"]').click();
      }
      // the LINZ key lives in this browser (DataHub.keys), not in the project's settings
      const lk = U.$('#modal [data-key="linz"]');
      if (lk && lk.value.trim() !== Kit.linzKey()) { DataHub.setKey('linz', lk.value); s.linzKey = ''; if (S().basemap === 'linz') { UI.setBasemap('linz'); s.basemap = S().basemap; } toast(lk.value.trim() ? 'LINZ key saved in this browser.' : 'LINZ key removed.'); }
    }
    if (JSON.stringify(s) === JSON.stringify(S())) { Render.redraw(); UI.renderLegend(); return toast('No model settings changed.'); }
    Kit.snapSettings('settings');
    Store.P.settings = s;
    staleAll(); Store.emit('settings');
    toast('Settings saved. Run the model again to apply them.', '', { action: { label: 'Undo', run: () => UI.undo() } });
  }

  /* ---------------- help ---------------- */
  /* Shortcuts in groups: tools (built-in and registered, e.g. A study area), editing, the model, files, colour modes,
     and every registered command with keys (palette Ctrl+K or /, [ ] scenario, \ period, F fit, L labels, H panel,
     Ctrl+D duplicate zone …). Shift+click and Shift+drag appear when the multi-edit feature is loaded. */
  function helpKeysHtml() {
    const k = (a, b) => `<kbd class="k">${esc(a)}</kbd><span>${esc(b)}</span>`;
    const grp = (title, rows) => rows ? `<h3>${esc(title)}</h3><div class="keys">${rows}</div>` : '';
    const tools = (typeof UI !== 'undefined' && UI.tools ? UI.tools() : []).filter(t => t.key);
    const cmds = Registry.list('commands', c => c.keys && !c.bound);
    const modes = Registry.list('modes', m => !m.hidden && m.key);
    const multi = typeof Onboarding !== 'undefined' && (Onboarding.makeSel || Onboarding.toggle);
    const byGroup = new Map(); for (const c of cmds) { const g = c.group || 'More'; if (!byGroup.has(g)) byGroup.set(g, []); byGroup.get(g).push(c); }
    return grp('Tools', tools.map(t => k(t.key, t.label)).join(''))
      + grp('Editing', `${k('Enter', 'Finish the road or bus route')}${k('Backspace', 'While drawing: remove the last point')}${k('Esc', 'Cancel, clear or deselect')}${k('Delete', 'Delete the selection')}${k('Ctrl+Z / Ctrl+Y', 'Undo / redo')}`
        + (multi ? `${k('Shift+click', 'Add a road or junction to the selection, or take it out')}${k('Shift+drag', 'Select every road in a box')}` : ''))
      + grp('Model and files', `${k('Ctrl+Enter', 'Run this scenario, AM and PM')}${k('Ctrl+Shift+Enter', 'Run every scenario, AM and PM')}${k('Space', 'Start or pause the simulation')}${k('Ctrl+S', 'Save the project file')}${k('Ctrl+Shift+S', 'Save as a new file')}${k('?', 'This help')}`)
      + grp('Colour roads by', modes.map(m => k(m.key, m.label)).join(''))
      + [...byGroup].map(([g, list]) => grp(g === 'More' ? 'More shortcuts' : `${g} shortcuts`, list.map(c => k(c.keys, c.label)).join(''))).join('');
  }
  function help() {
    const terms = Object.entries(Kit.GLOSSARY).map(([t, d]) => `<dt><b>${esc(t)}</b></dt><dd>${esc(d)}</dd>`).join('');
    modal('How Spare Construct works', `
      ${helpKeysHtml()}
      <h3>The model</h3>
      <p><b>Demand.</b> Zones generate peak-hour vehicle trips from their land use and the trip rates in Settings. Gateways add traffic crossing the study boundary, grown to the forecast year. A gravity model spreads zone trips by free-flow drive time to other zones and gateways; gateway traffic not used by zones becomes through traffic. You can also import your own ${Kit.gl('OD')} matrix.</p>
      <p><b>Assignment.</b> Trips are loaded by user equilibrium (conjugate Frank-Wolfe): each driver takes the quickest route given everyone else’s choices. Link times use the ${Kit.gl('BPR')} speed-flow curve. Junction delay uses Webster and ${Kit.gl('HCM')} overflow delay at signals, HCM gap acceptance for give-way and stop approaches, and the HCM 6 entry capacity model for roundabouts. Signal cycles and green splits are optimised to the flows unless you fix them. The run stops at the convergence target in Settings and says so if it does not get there.</p>
      <p><b>Simulation.</b> Vehicles drive the assigned routes using the Intelligent Driver Model, queue at red lights, give way on minor roads, yield at roundabouts and keep left. The ${Kit.gl('GEH')} check compares simulated and modelled flows.</p>
      <p><b>Results.</b> ${Kit.gl('V/C')} bands by road, junction delay and ${Kit.gl('LoS')}, queues, travel times between any two points, drive, walk and cycle catchments, select-link analysis, cycling stress, crash risk, CO₂, and a screening benefit–cost ratio against the base (do-minimum) scenario.</p>
      <p class="note">Spare Construct is a sketch-planning tool. Its defaults are indicative: calibrate to counts and use approved models (SATURN, Aimsun, Vissim, SIDRA) for consenting and business cases.</p>
      <h3>Producing an ITA with Spare Construct</h3>
      <ol class="help-steps">
        <li><b>Set up the area.</b> ☰ Menu › Set up a study area (or the Study area tool): roads, gateways, counts, crashes and existing land use from OpenStreetMap, the Census and the MoE directory. Check network (Run ▾) lists coding problems.</li>
        <li><b>Base year (S0).</b> Give counts their roles in the Traffic data tab (calibration, validation hold-out, not used), add a screenline, and fit the matrix to counts if your peer reviewer agrees. The validation table reports against NZTA TMDG 2019 for the category you chose; fewer than 10 sites is indicative only.</li>
        <li><b>Assessment set.</b> Scenarios › Create assessment set: S1 future baseline (do-minimum, with committed growth from the register), S2 with the proposal, S3 with mitigation, all in the same year. A permitted baseline (RMA s104(2)) is its own role.</li>
        <li><b>Regulatory context.</b> In the Compliance tab set the pathway and the lodgement date (it sets the RMA regime: lodged from 30 Oct 2026 = the amended RMA), then work through E27 screening, access, state highways and the checklist. Compliance+ holds precinct staging, mitigation, conditions and the scope linter.</li>
        <li><b>Run and read.</b> Run every scenario (Ctrl+Shift+Enter). Show on map: V/C, junction delay, change vs a scenario, development traffic and development share (%). Gateway labels say <i>From counts</i> or <i>Estimated</i>.</li>
        <li><b>Report.</b> The Report tab collects the author's details, scope, text and sign-off, and runs the staging test and sensitivity pack. Open the report to print it (Chrome or Edge 131+ prints the D1 footer and version stamp on every page), or download the data pack (ZIP) and the SIDRA hand-over (Export › SIDRA volumes, loader script and volume grids).</li>
      </ol>
      <p class="note">Every result is screening-level (Disclaimer D1). Junction results are HCM 6 / Akcelik-based, indicative; not SIDRA output. Confirm them in SIDRA, and agree the model's fitness for purpose with the reviewer. Try the Drury sample (☰ Menu › Load the Drury sample) to see a complete worked example.</p>
      <h3>Glossary</h3><dl class="gloss">${terms}</dl>`, [['Close', null]], true);
  }

  /* ---------------- about the data ----------------
     An accurate data-flow statement (about-data (a)), each source with its licence and query date (f), the project's
     assumptions (settings changed from the defaults, (e)) and the rows registered by feature files. */
  const DATA_FLOW = 'Spare Construct runs in your browser; there is no Spare Construct server. To fetch data it sends the map area to OpenStreetMap (roads), NZTA, Auckland Transport, Auckland Council and Stats NZ open-data services (counts, crashes, speed limits, census, zoning); map tile requests to Esri, OpenStreetMap or LINZ; place searches to Photon/Nominatim; up to 100 points along a road you profile to Open-Meteo (ground levels, only when you ask for a gradient profile); and, only if you add your own keys, single points to TomTom and journey start and end points to Google. Fonts load from Google Fonts and the map library from unpkg. Your project, land use and results are never uploaded.';
  const KEYS_NOTE = 'The core data is free and needs no account; live speeds (TomTom) and Google travel times need your own keys and accounts.';
  /* Every external host Spare Construct itself can contact, checked against the URLs in the code (not yet against a
     browser network log). Add a row whenever a feature fetches from a new host. */
  const HOSTS = [
    ['overpass-api.de, overpass.kumi.systems, overpass.private.coffee, maps.mail.ru', 'OpenStreetMap roads, buildings and land use (Overpass API mirrors, tried in turn)', 'the map area'],
    ['services.arcgis.com, services1.arcgis.com, services2.arcgis.com', 'ArcGIS open-data services of NZTA (counts, crashes, speed limits), Auckland Transport (counts, Future Connect, controlled intersections, public transport, cycling) and Auckland Council (Unitary Plan, flood plains), and Stats NZ census layers', 'the map area'],
    ['catalogue.data.govt.nz', 'NZTA daily traffic counts (TMS) and the Ministry of Education school directory', 'the map area or count site numbers'],
    ['files.mobilitydatabase.org', 'Bus timetables (GTFS), when you load them', 'nothing about your project (a whole feed is downloaded)'],
    ['server.arcgisonline.com, tile.openstreetmap.org, basemaps.linz.govt.nz', 'Background map tiles', 'tile numbers for the view'],
    ['photon.komoot.io, nominatim.openstreetmap.org', 'Place search', 'the text you search for'],
    ['api.open-meteo.com', 'Ground levels for a gradient profile (free tier: non-commercial use only)', 'up to 100 points along the road you profile, only when you ask'],
    ['api.tomtom.com (your key)', 'Live speeds', 'single points'],
    ['routes.googleapis.com (your key)', 'Peak travel times', 'journey start and end points'],
    ['fonts.googleapis.com, fonts.gstatic.com, unpkg.com', 'Fonts and the Leaflet map library', 'nothing about your project'],
  ];
  function aboutHtml() {
    const row = (what, src, note) => `<tr><td><b>${what}</b></td><td>${src}</td><td class="note" style="white-space:normal">${note}</td></tr>`;
    const a = (href, t) => href ? `<a href="${esc(href)}" target="_blank" rel="noopener">${esc(t)}</a>` : esc(t);
    const tbl = (rows, head = ['What', 'Source', 'Notes']) => `<div class="tbl-wrap"><table class="t compact"><tr>${head.map(h => `<th>${h}</th>`).join('')}</tr>${rows}</table></div>`;
    const extra = Registry.list('about').map(d => {
      const rows = Registry.call(d, 'rows') || [], intro = typeof d.intro === 'function' ? Registry.call(d, 'intro') : d.intro;
      return `<h3>${esc(d.title)}</h3>${intro ? `<p>${intro}</p>` : ''}${tbl(rows.map(r => row(r[0], r[1], r[2] || '')).join(''))}`;
    }).join('');
    const S0 = S(), BASIS = { measured: 'Real', user: 'Yours', assumed: 'Assumed', derived: 'Derived', estimated: 'Estimated' };
    const src = Kit.dataSources().map(d => `<tr><td><b>${esc(d.what)}</b></td><td>${a(d.url, d.source)}</td><td>${esc(d.licence || '–')}</td><td>${esc(d.date || 'not loaded')}</td><td class="note" style="white-space:normal"><b>${esc(BASIS[d.basis] || d.basis)}.</b> ${esc(d.notes)}</td></tr>`).join('');
    const A = Kit.nonDefaults();
    return `
      <p>${esc(DATA_FLOW)}</p>
      <p>${esc(KEYS_NOTE)}</p>
      <h3>Where data goes</h3>
      ${tbl(HOSTS.map(([h, w, s]) => `<tr><td class="mono" style="white-space:normal">${esc(h)}</td><td>${esc(w)}</td><td class="note" style="white-space:normal">${esc(s)}</td></tr>`).join(''), ['Host', 'For', 'What Spare Construct sends'])}
      <h3>Data sources in this project</h3>
      <p class="note">Measured data is marked <b>Real</b>; anything Spare Construct estimates is marked <b>Assumed</b> and can be changed in Settings or the side panel. The query date is when you loaded it. This table is the report's data appendix.</p>
      ${tbl(src, ['What', 'Source', 'Licence', 'Queried', 'Notes'])}
      <h3>Assumptions: settings changed from Spare Construct's defaults (${A.length})</h3>
      ${A.length ? `<ul class="assume">${A.map(x => `<li>${esc(x.text)}</li>`).join('')}</ul>` : '<p class="note">None: every setting is at Spare Construct’s default.</p>'}
      <h3>Model methods</h3>
      ${tbl(`
        ${row('Route choice', 'User equilibrium, conjugate Frank-Wolfe', `Static assignment, as in SATURN or Emme. Target relative gap ${esc(S0.gapTol)}, at most ${esc(S0.maxIter)} iterations.`)}
        ${row('Link travel time', `${Kit.gl('BPR')} curve (US Bureau of Public Roads, 1964)`, `α = ${esc(S0.bprAlpha)}, β = ${esc(S0.bprBeta)}. Mid-block capacity per lane by road class: ${Object.values(CLASSES).map(c => c.label + ' ' + c.cap).join(', ')} pcu/h.`)}
        ${row('Signals', 'Webster uniform delay + HCM overflow delay; Webster cycle optimisation', `Saturation flow ${esc(S0.satFlow)} ${Kit.gl('pcu')}/h/lane, ${esc(S0.lostPerPhase)} s lost per phase.`)}
        ${row('Give way, stop, roundabouts', 'Highway Capacity Manual (6th ed.) gap acceptance and roundabout entry capacity', `Minor roads yield to the major road; roundabout entries yield to circulating traffic. ${esc(Engine.JUNCTION_LABEL || '')}.`)}
        ${row(Kit.gl('LoS', 'Level of service'), `Links: Spare Construct ${Kit.gl('V/C')} bands (0.35/0.55/0.75/0.90/1.00, not ${Kit.gl('HCM')} urban-street LoS). Junctions: HCM 6 control delay`, 'F whenever demand exceeds capacity.')}
        ${row('Simulation', 'Intelligent Driver Model (Treiber, Hennecke & Helbing, 2000)', 'Car-following with left-hand traffic, signals, give-way and roundabout yielding.')}
        ${row('Walk and cycle catchments', 'Spare Construct assumption', 'Walking 4.8 km/h and cycling 15 km/h on every road except motorways and ramps; no footpath, crossing or gradient data.')}
        ${row('Ground levels', a('https://open-meteo.com/en/docs/elevation-api', 'Open-Meteo Elevation API') + ' (Copernicus DEM GLO-90)', '<b>Real</b> but coarse (90 m grid), and the free service is for non-commercial use. Shows terrain, not design levels; use LINZ 1 m LiDAR for design.')}
        ${row('CO₂', 'Generic average-speed emission curve', `<b>Assumed</b>, indicative only. ${a('https://www.nzta.govt.nz/planning-and-investment/learning-and-resources/transport-data/vehicle-emissions-prediction-model/', 'NZTA VEPM')} is the NZ standard.`)}
        ${row(`Costs and ${Kit.gl('BCR')}`, 'Unit rates per lane-km and per junction; travel-time benefits', `<b>Assumed.</b> $${esc(S0.econ.vot)}/veh·h, ${S0.econ.method === 'uniform' ? `${esc(S0.econ.discount)}% flat` : `${Kit.gl('MBCM')} v1.7.5 discounting`} over ${esc(S0.econ.years)} years. An indicative efficiency rating, not a funding-approval BCR; use the ${a('https://www.nzta.govt.nz/resources/monetised-benefits-and-costs-manual/', 'NZTA Monetised Benefits and Costs Manual')} for business cases.`)}
        ${row('The Drury sample', 'OpenStreetMap, Stats NZ Census 2023, MoE, NZTA and Auckland Transport counts, NZTA CAS (downloaded when you load it)', '<b>Real</b> roads, existing land use, counts and crashes for the 2026 base year. <b>Illustrative</b>: the Drury East and Drury Centre development yields and staging, Options A and B, the illustrative turning count and the regulatory details. Not Auckland Transport, council or developer figures. Parts that fail to download are listed when the sample loads.')}`)}
      ${extra}
      <h3>Your data</h3>
      <p>Projects are kept in this browser (IndexedDB) and in files you save. Nothing is uploaded. API keys you add (TomTom, Google and LINZ) stay in this browser only and are never written into project files.</p>`;
  }
  const about = () => modal('About the data', aboutHtml(), [['Close', null]], true);

  /* ---------------- gradient profile ---------------- */
  async function elevation(Lk) {
    const sc = Store.sc, g = Net.geom(sc, Lk), cum = U.cumLen(g), L = cum[cum.length - 1];
    const n = Math.min(100, Math.max(3, Math.ceil(L / 20) + 1)), pts = [];
    for (let i = 0; i < n; i++) pts.push(U.along(g, cum, L * i / (n - 1)).p);
    // Open-Meteo's free tier is for non-commercial use: say so, and what is sent, before the first request
    const pref = typeof UI !== 'undefined' && UI.pref;
    if (!(pref && pref.get('elevOk', false))) {
      const v = await modal('Ground levels from Open-Meteo', `<p>The gradient profile sends ${n} points along this road (latitude and longitude only) to the Open-Meteo Elevation API, which returns ground levels from the Copernicus 90 m terrain model.</p>
        <p class="note warn">Open-Meteo’s free service is for non-commercial use only. For consultancy work, check that your organisation has an Open-Meteo licence, or use LINZ 1 m LiDAR instead.</p>
        <label class="check"><input type="checkbox" id="elevRemember"> Don’t ask again in this browser</label>`,
        [['Cancel', null, ''], ['Fetch ground levels', () => ({ remember: !!(U.$('#elevRemember') || {}).checked })]]);
      if (!v) return;
      if (v.remember && pref) pref.set('elevOk', true);
    }
    let el;
    try {
      // a cancellable task (results-trust (f)) with its own 15 s limit
      el = await Kit.task('Fetching ground levels from Open-Meteo…', async signal => {
        const ac = new AbortController(), timer = setTimeout(() => ac.abort(), 15000), stop = () => ac.abort();
        signal.addEventListener('abort', stop, { once: true });
        try {
          const url = `https://api.open-meteo.com/v1/elevation?latitude=${pts.map(p => p[0].toFixed(5)).join(',')}&longitude=${pts.map(p => p[1].toFixed(5)).join(',')}`;
          const r = await fetch(url, { signal: ac.signal }); if (!r.ok) throw new Error('HTTP ' + r.status);
          const e = (await r.json()).elevation;
          if (!Array.isArray(e) || e.length !== n || e.some(x => typeof x !== 'number' || !isFinite(x))) throw new Error('the reply had no ground levels');
          return e;
        } catch (e) { if (signal.aborted) throw Object.assign(new Error('Cancelled'), { name: 'AbortError', cancelled: true }); if (e.name === 'AbortError') throw new Error('Open-Meteo took too long to answer'); throw e; }
        finally { clearTimeout(timer); signal.removeEventListener('abort', stop); }
      });
    } catch (e) {
      if (e.cancelled) return toast('Gradient profile cancelled.');
      return toast('Could not fetch ground levels: ' + e.message + '. Try again later.', 'err');
    }
    const step = L / (n - 1);
    // smooth over ~60 m to suppress DEM noise before taking grades
    const sm = el.map((_, i) => { let s = 0, c = 0; for (let j = Math.max(0, i - 1); j <= Math.min(n - 1, i + 1); j++) { s += el[j]; c++; } return s / c; });
    const grades = sm.slice(1).map((e, i) => (e - sm[i]) / step);
    const maxG = Math.max(...grades.map(Math.abs)), rise = el[n - 1] - el[0];
    const lim = (CLASSES[Lk.cls] || CLASSES.local).rank >= 4 ? 0.08 : 0.125;
    const W = 600, H = 190, mn = Math.min(...el), mx = Math.max(...el), span = Math.max(5, mx - mn);
    const X = i => 46 + (W - 56) * i / (n - 1), Y = e => H - 26 - (H - 46) * (e - mn + span * 0.1) / (span * 1.2);
    const area = `M${X(0)},${H - 26} ` + el.map((e, i) => `L${X(i).toFixed(1)},${Y(e).toFixed(1)}`).join(' ') + ` L${X(n - 1)},${H - 26}Z`;
    const steep = grades.map((gr, i) => Math.abs(gr) > lim ? `<line x1="${X(i)}" y1="${Y(el[i])}" x2="${X(i + 1)}" y2="${Y(el[i + 1])}" stroke="var(--los-f)" stroke-width="3.5" stroke-linecap="round"/>` : '').join('');
    modal(`Gradient profile · ${esc(Lk.name || CLASSES[Lk.cls].label + ' road')}`, `
      <svg class="profile" viewBox="0 0 ${W} ${H}" role="img" aria-label="Elevation profile">
        <path d="${area}" fill="var(--accent-soft)" stroke="var(--accent)" stroke-width="1.6"/>${steep}
        <line x1="46" y1="${H - 26}" x2="${W - 10}" y2="${H - 26}" stroke="var(--line)"/>
        <text x="40" y="${Y(mx) + 4}" text-anchor="end">${Math.round(mx)} m</text><text x="40" y="${Y(mn) + 4}" text-anchor="end">${Math.round(mn)} m</text>
        <text x="46" y="${H - 8}">A</text><text x="${W - 10}" y="${H - 8}" text-anchor="end">B · ${U.fmt.len(L)}</text></svg>
      <div class="facts">${fact(U.fmt.pct(maxG, 1), 'steepest grade (≈60 m)')}${fact(U.fmt.signed(rise, 0) + ' m', 'rise A → B')}${fact(Math.round(mn) + '–' + Math.round(mx) + ' m', 'ground level range')}${fact(U.fmt.pct(lim, 1), 'guide maximum for this class')}</div>
      <p class="note">${maxG > lim ? `<b>Sections in red exceed ${U.fmt.pct(lim, 1)}.</b> Consider realignment, earthworks or a lower design speed. ` : 'Within the guide maximum grade for this road class. '}Ground levels come from the Copernicus 90 m DEM via Open-Meteo (free for non-commercial use only), so they show terrain, not design levels; use LINZ 1 m LiDAR for design.</p>`, [['Close', null]], true);
  }

  /* ---------------- files ---------------- */
  let fileMode = 'project';
  function openFile(mode) { fileMode = mode; const f = U.$('#fileIn'); f.value = ''; f.accept = mode === 'od' || mode === 'counts' ? '.csv' : mode === 'geojson' ? '.geojson,.json' : '.json'; f.click(); }
  async function onFile(e) {
    const file = e.target.files[0]; if (!file) return;
    const text = await file.text();
    try {
      if (fileMode === 'project') await openProjectText(text, file.name, file.lastModified);
      else if (fileMode === 'geojson') {
        Store.snapshot();
        const n = Importers.importGeoJSON(Store.sc, JSON.parse(text), UI.roadProps());
        Store.edited(); toast(`Imported ${n} road${n === 1 ? '' : 's'}.`);
        const sc = Store.sc, pts = Object.values(sc.nodes).map(n => [n.lat, n.lng]); if (pts.length) UI.map.fitBounds(L.latLngBounds(pts));
      } else if (fileMode === 'counts') {
        const n = DataHub.importCSV(text); View.show.counts = true; Store.emit('data');
        toast(`Imported ${n} count${n === 1 ? '' : 's'}. They are matched to the nearest road.`);
      } else if (fileMode === 'od') {
        const rows = U.parseCSV(text), sc = Store.sc, per = S().period;
        const byName = new Map(Object.values(sc.zones).map(z => [z.name.trim().toLowerCase(), z.id]));
        const cols = rows[0].slice(1).map(n => byName.get(n.trim().toLowerCase()));
        const ids = [], m = [];
        for (const r of rows.slice(1)) { const id = byName.get((r[0] || '').trim().toLowerCase()); if (!id) continue; ids.push(id); }
        const zonesAll = [...new Set([...ids, ...cols.filter(Boolean)])];
        const pos = new Map(zonesAll.map((id, i) => [id, i]));
        for (let i = 0; i < zonesAll.length; i++) m.push(new Array(zonesAll.length).fill(0));
        for (const r of rows.slice(1)) {
          const o = byName.get((r[0] || '').trim().toLowerCase()); if (!o) continue;
          r.slice(1).forEach((v, j) => { const d = cols[j]; if (d) m[pos.get(o)][pos.get(d)] = +v || 0; });
        }
        if (!zonesAll.length) throw new Error('No zone names in the CSV match zones in this scenario.');
        Store.snapshot();
        sc.odManual ||= {}; sc.odManual[per] = { zones: zonesAll, m };
        Store.edited('demand');
        toast(`Manual ${per} OD matrix loaded for ${zonesAll.length} zones. Run the model to use it.`);
      }
    } catch (err) { toast('Could not read that file: ' + err.message, 'err'); }
  }
  /* Open a project file (storage). Pending edits to the open project are saved first. A file whose project is
     already in this browser (an older copy, a colleague's copy, or your own earlier save) asks whether to replace
     the browser copy or open the file as a separate copy, so opening never overwrites work silently. The opened
     project never inherits a saved-file link: its first Save asks where to save, so it cannot write over another file.
     Returns 'opened' | 'copy' | 'replaced' | 'cancelled'. Headless (no dialog), a clash is cancelled. */
  async function openProjectText(text, fileName = 'the file', lastModified = null) {
    const P = JSON.parse(text);
    if (!P || typeof P !== 'object' || !Array.isArray(P.scenarios)) throw new Error('This file is not a Spare Construct project.');
    Kit.harden(P, null);   // settings are hardened after migrate() has merged them with the defaults (UI 'project')
    await Kit.flushSave();
    let list = [];
    try { list = await Store.projects.list(); } catch { }
    const same = typeof P.id === 'string' && P.id ? list.find(m => m.id === P.id) : null;
    let how = 'opened';
    if (same) {
      const when = t => t ? new Date(t).toLocaleString('en-NZ', { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false }) : 'an unknown time';
      const v = await modal('This project is already in this browser', `<p><b>${esc(fileName)}</b> is a copy of “${esc(same.name)}”, which this browser already keeps${same.current ? ' and is open now' : ''}.</p>
        <p>The browser copy was last changed ${esc(when(same.updated))}${lastModified ? `; the file was saved ${esc(when(lastModified))}` : ''}.</p>
        <p class="note">Replacing loses any changes in the browser copy that are not in the file. Opening as a separate copy keeps both.</p>`,
        [['Cancel', null, ''], ['Replace the browser copy', 'replace', 'danger'], ['Open as a separate copy', 'copy']]);
      if (!v) return 'cancelled';
      if (v === 'copy') { P.id = U.uid('p'); P.name = `${P.name || 'Untitled project'} (from file)`; how = 'copy'; }
      else how = 'replaced';
    }
    // the saved-file link belongs to whatever file was last saved under this id, not to the file just opened
    if (P.id) await Store.projects.setHandle(null, P.id);
    Store.setProject(P);
    toast(how === 'copy' ? `Opened ${Store.P.name} as a separate copy. The browser copy is unchanged (Menu › Open recent).` : `Opened ${Store.P.name}. Save (Ctrl+S) asks where to save the first time.`);
    return how;
  }
  function saveProject() {
    U.download((Store.P.name || 'project').replace(/[^\w\- ]+/g, '').replace(/\s+/g, '-').toLowerCase() + '.corridor.json', Store.serialise());
    toast('Project saved to your downloads.');
  }
  function bindEvents() { U.$('#fileIn').addEventListener('change', onFile); }

  return { welcome, welcomeHtml, findArea, importRoads, runImport, settings, settingsHtml, readSettings, help, helpKeysHtml, about, aboutHtml, DATA_FLOW, HOSTS, elevation, openFile, openProjectText, saveProject, bindEvents };
})();
