'use strict';
/* Spare Construct · results drawer (bottom panel). Every tab, including the built-in ones, is a
   Registry.tabs entry, so feature files add or replace tabs the same way. Tab bodies are HTML
   strings; clicks use data-fly="type:id" (fly to and select), data-sort and data-do. */

const Drawer = (() => {
  const S = () => Store.S;
  const { opt, fld, kpi, delta, losBadge, gehBadge, compass, junctionName } = Kit;

  /* ---------------- tab bodies ---------------- */
  function emptySteps() {
    const sc = Store.sc, hasRoads = Object.keys(sc.links).length > 0, hasZones = Object.keys(sc.zones).length > 0;
    const step = (done, t) => `<div class="lg-row"><span class="los ${done ? 'A' : 'none'}">${done ? '✓' : '·'}</span><span>${t}</span></div>`;
    return `<div class="bar-h"><h3>Set up ${U.esc(sc.name)}</h3></div>
      ${step(hasRoads, 'Bring in roads: <b>Import roads</b> for streets in view, or draw with the road tool (R)')}
      ${step(hasZones, 'Add traffic: land-use zones (Z) for developments and gateways (G) where roads leave the area')}
      ${step(false, 'Press <b>Run model</b> (Ctrl+Enter), then <b>Simulate</b> to watch it')}
      <p class="note" style="margin-top:10px">Or open <b>Menu › Load the Drury sample</b> to see a finished example.</p>`;
  }
  /* Engine notes rewritten as actions (results-trust (d)): each known note becomes a plain sentence saying what
     happened and what to do, with a button; anything else is shown as it is. */
  const NOTE_RULES = [
    [/([\d,]+) veh\/h from zones had nowhere to go\.[^.]*\./, m => [`${m[1]} veh/h of zone trips had nowhere to go, so they are left out. Add a gateway where a road leaves the area, or land uses they can travel to.`, [['Add a gateway (G)', 'tool:gateway']]]],
    [/Zone trips leaving through (.+?) exceed the gateway count;[^.]*\./, m => [`More zone trips leave through ${m[1]} than its gateway count allows, so Spare Construct capped them. Raise the gateway flow or check the zones' land use.`, [['Show gateways', 'tab:demand']]]],
    [/Through traffic entering \(([\d,]+)\) and leaving \(([\d,]+)\) differs and was balanced\./, m => [`Gateway traffic doesn't balance: ${m[1]} veh/h enter but ${m[2]} leave, so Spare Construct scaled through trips to match. Check gateway flows or set them from counts.`, [['Show gateways', 'tab:demand'], ...(DataHub.D().counts.length ? [['Set from counts', 'gwcounts']] : [])]]],
    [/There are no gateways, so all trips stay inside the area\./, () => ['There are no gateways, so every trip starts and ends inside the area and no through traffic is modelled. Add gateways where roads leave the area.', [['Add a gateway (G)', 'tool:gateway']]]],
  ];
  function noteActions(note) {
    if (!note) return '';
    let rest = String(note);
    const out = [];
    for (const [re, f] of NOTE_RULES) {
      const m = rest.match(re); if (!m) continue;
      rest = rest.replace(m[0], '');
      const [text, acts] = f(m);
      out.push(`<div class="act-note"><p class="note warn">${U.esc(text)}</p>${acts.map(([l, d]) => `<button type="button" class="btn small" data-do="${U.esc(d)}">${U.esc(l)}</button>`).join('')}</div>`);
    }
    rest = rest.replace(/\s+/g, ' ').trim();
    if (rest) out.push(`<div class="act-note"><p class="note">${U.esc(rest)}</p></div>`);
    return out.join('');
  }
  // Settings that differ from Spare Construct's defaults (about-data (e)): the project's assumptions
  function assumptionsHtml() {
    const A = Kit.nonDefaults();
    return `<div class="lg-title" style="margin-top:14px">Assumptions · settings changed from Spare Construct's defaults (${A.length})</div>
      ${A.length ? `<ul class="assume">${A.map(a => `<li>${U.esc(a.text)}</li>`).join('')}</ul>` : '<p class="note">Every setting is at Spare Construct’s default. The defaults are listed under Menu › About the data.</p>'}
      <div class="actions" style="margin-top:6px"><button type="button" class="btn small" data-do="assumptions">Export assumptions (CSV)</button><button type="button" class="btn small" data-do="settings">Open Settings</button></div>`;
  }
  const flyRow = (fly, cells) => ({ fly, cells });
  function summaryTab() {
    const res = Store.res(); if (!res) return emptySteps() + assumptionsHtml();
    const sc = Store.sc, k = res.kpi, isBase = sc.id === Store.P.base, br = !isBase ? Store.res(Store.baseSc) : null, bk = br && br.kpi;
    const d = (key, dd, lower = true) => bk ? delta(k[key], bk[key], dd, lower) : ['', ''];
    const top = Object.entries(res.arcs).filter(([, r]) => r.v > 0).sort((a, b) => b[1].vc - a[1].vc).slice(0, 6);
    const worstJ = Object.entries(res.nodes).sort((a, b) => b[1].delay - a[1].delay).slice(0, 6);
    const [tD, tC] = d('trips', 0, false), [vD, vC] = d('vkt', 0), [hD, hC] = d('vht', 1), [sD, sC] = d('spd', 1, false), [dD, dC] = d('dly', 1), [cD, cC] = d('co2', 2);
    const N = Kit.num, st = res.meta && res.meta.status;
    const cs = Kit.convState(res);
    return `<div class="bar-h"><h3>${U.esc(sc.name)} · ${U.esc(res.period)} peak ${U.esc(res.year)}</h3>${res.stale ? '<span class="pill stale">out of date: run again</span>' : ''}${Kit.convPill(res)}
        ${st ? `<span class="pill" title="${U.esc(st.standing || st.footer || '')}">${U.esc(st.label || st.short || st.status)}</span>` : ''}
        <span class="note">${U.fmt.int(res.it)} iterations · ${U.fmt.int(res.ms)} ms${res.manual ? ` · manual ${Kit.gl('OD')} matrix` : ''}</span></div>
      ${cs && cs.ok === false ? `<div class="act-note"><p class="note warn">${U.esc(cs.text)}</p><button type="button" class="btn small" data-do="tighter">Run again, tighter</button></div>` : ''}
      ${noteActions(res.note)}
      <div class="kpis">
        ${kpi('Vehicle trips', N(k.trips, 0) + '<small> veh/h</small>', tD, tC)}
        ${kpi('Vehicle-km in the peak hour', N(k.vkt, 0), vD, vC)}
        ${kpi(`Vehicle-hours in the peak hour (${Kit.gl('VHT')})`, N(k.vht, 1), hD, hC)}
        ${kpi('Average speed', N(k.spd, 1) + '<small> km/h</small>', sD, sC)}
        ${kpi('Total delay', N(k.dly, 1) + '<small> vehicle-hours in the peak hour</small>', dD, dC)}
        ${kpi('CO₂ in the peak hour', N(k.co2, 2) + '<small> t</small>', cD, cC)}
        ${kpi('Roads over capacity', U.fmt.int(k.over), bk ? U.fmt.signed(k.over - bk.over) + ' vs base' : `highest ${Kit.gl('V/C')} ${U.fmt.num(k.maxVC, 2)}`, bk ? (k.over > bk.over ? 'up' : k.over < bk.over ? 'down' : '') : '')}
        ${kpi(`Worst junction (${Kit.gl('LoS')})`, k.worst ? `${U.esc(k.worst.los)} <small>${U.esc(Engine.delayText ? Engine.delayText(k.worst.delay, 0) : U.fmt.int(k.worst.delay) + ' s')}</small>` : '–', k.worst ? U.esc(junctionName(k.worst.id)) : '')}
        ${k.unserved > 1 ? kpi('Trips with no route', U.fmt.int(k.unserved), 'check one-way roads and zone access lines', 'up') : ''}
      </div>
      <div class="row2" style="margin-top:14px">
        <div><div class="lg-title">Busiest roads by ${Kit.gl('V/C')}</div>${Kit.table({ head: ['Road', { label: 'veh/h', n: true }, { label: Kit.gl('V/C'), n: true }, 'Band'], rows: top.map(([key, r]) => { const [id, d2] = key.split(':'); const Lk = sc.links[id]; return flyRow('link:' + id, [`${U.esc(Lk && Lk.name || (CLASSES[(Lk || {}).cls] || CLASSES.local).label)} ${d2 === 'ab' ? '→' : '←'}`, U.fmt.int(r.v), U.fmt.num(r.vc, 2), losBadge(r.los)]); }) })}</div>
        <div><div class="lg-title">Most delayed junctions</div>${Kit.table({ head: ['Junction', 'Control', { label: 'Delay', n: true }, Kit.gl('LoS')], rows: worstJ.map(([id, n]) => flyRow('node:' + id, [U.esc(junctionName(id)), U.esc(CONTROLS[n.type] || n.type), U.esc(Engine.delayText ? Engine.delayText(n.delay, 0) : U.fmt.int(n.delay) + ' s'), losBadge(n.intersectionLos === null ? (n.los || '–') : n.los)])), empty: 'No controlled junctions.' })}
          ${worstJ.length ? `<p class="note">${U.esc(Kit.junctionLabel(res))}</p>` : ''}</div>
      </div>
      ${assumptionsHtml()}
      ${st && st.footer ? `<p class="note" style="margin-top:10px">${U.esc(st.footer)}</p>` : ''}`;
  }
  const KPI_ROWS = [
    ['trips', 'Vehicle trips (veh/h)', 0, false], ['vkt', 'Vehicle km (veh·km/h)', 0, true], ['vht', 'Vehicle hours (veh·h/h)', 1, true],
    ['spd', 'Average speed (km/h)', 1, false], ['dly', 'Delay (veh·h/h)', 1, true], ['co2', 'CO₂ (t/h)', 2, true], ['over', 'Roads over capacity', 0, true],
  ];
  function compareTab() {
    const P = Store.P, per = S().period, base = Store.baseSc, rb = Store.res(base);
    const scs = P.scenarios;
    // each column shows the year its results were modelled for (or the scenario's year if not run yet)
    const yearCell = s => { const r = Store.res(s); return U.esc(r ? `${r.year}` : `${Engine.yearOf(s)}, not run`); };
    const head2 = `<tr><th>${U.esc(per)} peak</th>${scs.map(s => `<th class="n">${U.esc(s.name)}${s.id === P.base ? ' (base)' : ''}${Store.res(s) && Store.res(s).stale ? ' *' : ''}<br><span class="note" style="text-transform:none;letter-spacing:0">${yearCell(s)}</span></th>`).join('')}</tr>`;
    const rows = KPI_ROWS.map(([k, l, d, lower]) => `<tr><td>${l}</td>${scs.map(s => {
      const r = Store.res(s); if (!r) return '<td class="n note">–</td>';
      const v = r.kpi[k], b = rb && s !== base ? rb.kpi[k] : null;
      const dv = b != null ? v - b : null, cls = dv == null || Math.abs(dv) < 1e-6 ? '' : (dv < 0) === lower ? 'down' : 'up';
      return `<td class="n">${U.fmt.num(v, d)}${dv != null ? ` <span class="${cls}" style="font-size:11px">${U.fmt.signed(dv, d)}</span>` : ''}</td>`;
    }).join('')}</tr>`).join('');
    const ec = scs.map(s => s === base ? null : Engine.econ(base, s, per));
    const caps = scs.map(s => Engine.capital(s));
    const ok = i => ec[i] && ec[i].mismatch !== 'year';
    const erow = (l, f) => `<tr><td>${l}</td>${scs.map((s, i) => `<td class="n">${f(i)}</td>`).join('')}</tr>`;
    // economics v2 (MBCM v1.7.5 with NZTA GC 25/01: 2% / 1.5% / 1%, sensitivity at the analysis's own rate, IPM bands);
    // old projects keep the uniform method, which also reports the sensitivity PV and BCR (pv8, bcr8)
    const e0 = ec.find((e, i) => e && ok(i)) || ec.find(Boolean), mb = !!(e0 && e0.method === 'mbcm');
    const bcrT = x => x == null || !isFinite(x) ? '–' : Engine.bcrText ? Engine.bcrText(x) : U.fmt.num(x, 1);
    const band = b => (b || []).filter(x => x.band).map(x => `<span class="pill" title="${U.esc([x.source, x.note].filter(Boolean).join(' · '))}">${U.esc(x.label)}${x.draft ? ' (2027–30 draft, subject to GPS 2027)' : ''}</span>`).join(' ');
    const econRows = erow('Capital cost (indicative)', i => U.fmt.money(caps[i].total)) +
      erow('Proposed road', i => U.fmt.num(caps[i].km, 2) + ' km') +
      erow('Travel-time benefit per year', i => ok(i) ? U.fmt.money(ec[i].annual) : ec[i] ? '<span class="note warn" title="Scenarios are modelled for different years">different years</span>' : '–') +
      erow(`Present value of benefits${e0 && e0.years ? `, ${e0.years} years` : ''}${mb ? ` (${Kit.gl('MBCM')} rates)` : e0 && e0.rate != null ? ` at ${e0.rate}%` : ''}`, i => ok(i) ? U.fmt.money(ec[i].pv) : '–') +
      (e0 && e0.pv8 != null ? erow(`Present value at ${U.esc(e0.sensRate)}% (sensitivity, NZTA GC 25/01)`, i => ok(i) && ec[i].pv8 != null ? U.fmt.money(ec[i].pv8) : '–') : '') +
      erow(`Benefit–cost ratio (${Kit.gl('BCR')})`, i => ok(i) && ec[i].bcr != null ? `<b>${bcrT(ec[i].bcr)}</b>${mb && ec[i].ipm ? '<br>' + band(ec[i].ipm) : ''}` : '–') +
      (e0 && e0.bcr8 != null ? erow(`${Kit.gl('BCR')} at ${U.esc(e0.sensRate)}%`, i => ok(i) && ec[i].bcr8 != null ? bcrT(ec[i].bcr8) : '–') : '') +
      (mb ? erow(`First-year rate of return (${Kit.gl('FYRR')})`, i => ok(i) && ec[i].fyrr != null ? U.fmt.pct(ec[i].fyrr, 1) : '–') : '') +
      erow('CO₂ saved per year', i => ok(i) ? U.fmt.signed(ec[i].dco2, 0) + ' t' : '–');
    const mismatch = scs.map((s, i) => ec[i] && ec[i].mismatch === 'year' ? `<p class="note warn">${U.esc(s.name)}: ${U.esc(ec[i].note)}</p>` : '').join('');
    // warnings, and a signal-to-noise check that offers to re-run both scenarios with a tighter target
    const warns = scs.map((s, i) => ok(i) ? (ec[i].warnings || []).filter(w => !(ec[i].snr && w === ec[i].snr.text)).map(w => `<p class="note warn">${U.esc(s.name)}: ${U.esc(w)}</p>`).join('') +
      (ec[i].snr && ec[i].snr.ok === false ? `<div class="act-note"><p class="note warn">${U.esc(s.name)}: ${U.esc(ec[i].snr.text)}</p><button type="button" class="btn small" data-do="snr:${U.esc(s.id)}">Re-run both tighter</button></div>` : '') : '').join('');
    // delay bar chart
    const chart = Kit.bar(scs.map(s => { const r = Store.res(s), v = r ? r.kpi.dly : null; return { label: s.name, value: v, text: v == null ? null : U.fmt.num(v, 1) + ' veh·h', colour: s.id === P.base ? 'var(--faint)' : 'var(--accent)' }; }),
      { caption: `Total delay in the ${U.esc(per)} peak hour` });
    return `<div class="bar-h"><h3>Compare scenarios</h3><button class="btn small primary" data-do="runall">Run all scenarios (AM + PM)</button><span class="note">Base: ${U.esc(base.name)}. Change it from the ⋯ menu. * = out of date.</span></div>
      <div class="row2"><div class="tbl-wrap"><table class="t">${head2}${rows}<tr><td colspan="${scs.length + 1}" style="border:0;padding-top:12px"><div class="lg-title" style="margin:0">Indicative economics</div></td></tr>${econRows}</table></div>
      <div>${chart}${mismatch}${warns}
        ${mb ? `<p class="note"><b>${U.esc(e0.label || 'Indicative efficiency rating')}.</b> ${U.esc(e0.basis || '')}. Benefits are travel-time savings at $${U.esc(S().econ.vot)}/veh·h${e0.votUpdate ? ` (with the ${Kit.gl('MBCM')} update factor)` : ''} over the modelled peak hours a year (AM ${U.fmt.int((e0.hours || {}).AM || 0)} h, PM ${U.fmt.int((e0.hours || {}).PM || 0)} h). ${U.esc(e0.disclaimer || '')}</p>`
          : `<p class="note">${e0 && e0.basis ? `<b>${U.esc(e0.label || '')}</b> ${U.esc(e0.basis)}. ` : ''}Benefits are travel-time savings in the modelled peak hour (the average of AM and PM once both are run) × ${U.fmt.int(S().econ.annualHours)} equivalent hours a year at $${U.esc(S().econ.vot)}/veh·h. A screening estimate only; use the NZTA Monetised Benefits and Costs Manual for business cases.</p>`}</div></div>`;
  }
  function compareCSV() {
    const scs = Store.P.scenarios, rows = [['period', 'kpi', ...scs.map(s => s.name)]];
    rows.push(['', 'Year', ...scs.map(s => { const r = Store.res(s, 'AM') || Store.res(s, 'PM'); return r ? r.year : Engine.yearOf(s); })]);
    for (const per of ['AM', 'PM']) for (const [k, l] of KPI_ROWS) rows.push([per, l, ...scs.map(s => { const r = Store.res(s, per); return r ? U.r3(r.kpi[k]) : ''; })]);
    rows.push(['', 'Capital cost NZ$m', ...scs.map(s => U.r3(Engine.capital(s).total))]);
    return U.csv(rows);
  }
  function linksTab() {
    const res = Store.res(), sc = Store.sc;
    if (!res) return emptySteps();
    const f = View.linkFilter.toLowerCase(), srt = View.linkSort;
    let rows = Object.entries(res.arcs).map(([key, r]) => { const [id, d] = key.split(':'); const Lk = sc.links[id]; return Lk ? { key, id, d, Lk, r } : null; }).filter(Boolean);
    if (f) rows = rows.filter(x => (x.Lk.name || '').toLowerCase().includes(f) || x.Lk.cls.includes(f));
    if (View.linkOver) rows = rows.filter(x => x.r.vc > 1);
    const val = x => ({ name: x.Lk.name || '~', cls: CLASSES[x.Lk.cls].rank, v: x.r.v, cap: x.r.cap, vc: x.r.vc, spd: x.r.spd, nd: x.r.nd, len: x.r.len })[srt.k];
    rows.sort((a, b) => { const va = val(a), vb = val(b); return (va > vb ? 1 : va < vb ? -1 : 0) * srt.dir; });
    const total = rows.length; rows = rows.slice(0, 400);
    // sort headers are buttons with aria-sort; the first cell of each row is a button (keyboard tables)
    const h = (sort, label, n, title) => ({ label, n, sort, title, sorted: srt.k === sort ? srt.dir : 0 });
    // ticked roads (multi-edit): a checkbox per row, by road (both directions tick together), and 'Select these on the map'
    const pick = picked(sc), shownIds = [...new Set(rows.map(x => x.id))], allOn = shownIds.length > 0 && shownIds.every(id => pick.has(id));
    const box = x => `<input type="checkbox" data-rsel="${U.esc(x.id)}" ${pick.has(x.id) ? 'checked' : ''} aria-label="Tick ${U.esc(x.Lk.name || 'this road')} ${x.d === 'ab' ? '→' : '←'}">`;
    const lim = x => { const c = Kit.capSource(sc, res, x.key); return c && c.src !== 'mid-block' ? `<span title="${U.esc(c.text)}">${U.esc(c.src)} at ${U.esc(c.at)}</span>` : '<span class="note">mid-block</span>'; };
    return `<div class="bar-h"><h3>Roads</h3><input id="linkFilter" placeholder="Filter by name or class" aria-label="Filter roads by name or class" value="${U.esc(View.linkFilter)}" style="width:220px">
        <label class="check"><input type="checkbox" data-do="over" ${View.linkOver ? 'checked' : ''}> Over capacity only</label><button type="button" class="btn small" data-do="selmap" ${pick.size ? '' : 'disabled title="Tick roads in the table first"'}>Select these on the map${pick.size ? ` (${U.fmt.int(pick.size)})` : ''}</button>${pick.size ? '<button type="button" class="btn small" data-do="selclear">Clear ticks</button>' : ''}<span class="note">${U.fmt.int(total)} directional links${total > 400 ? ', showing 400' : ''}</span></div>
      ${Kit.table({ compact: false, head: [{ label: `<input type="checkbox" data-rsel-all ${allOn ? 'checked' : ''} aria-label="Tick every road shown">` }, h('name', 'Road'), h('cls', 'Class'), 'Dir', h('len', 'Length', true), h('v', 'veh/h', true), h('cap', `Capacity (${Kit.gl('pcu')}/h)`, true, 'Capacity in passenger car units an hour; V/C uses pcu'), 'Limited by', h('vc', Kit.gl('V/C'), true), Kit.gl('LoS'), h('spd', 'km/h', true), h('nd', 'Junction delay', true)],
        rows: rows.map(x => ({ fly: 'link:' + x.id, flyCol: 1, cells: [box(x), `${U.esc(x.Lk.name || '—')}${x.Lk.status === 'proposed' ? ' <span class="pill proposed">new</span>' : ''}`, U.esc((CLASSES[x.Lk.cls] || CLASSES.local).label), `${U.fmt.int(x.d === 'ab' ? x.Lk.lanesAB : x.Lk.lanesBA)} × ${x.d === 'ab' ? '→' : '←'}`, U.fmt.len(x.r.len), U.fmt.int(x.r.v), U.fmt.int(x.r.cap), lim(x), U.fmt.num(x.r.vc, 2), losBadge(x.r.los), U.fmt.int(x.r.spd), x.r.nd > 0.5 ? U.fmt.int(x.r.nd) + ' s' : '–'] })),
        empty: 'No roads match the filter.' })}`;
  }
  // The ticked roads that still exist in this scenario (View.roadPick holds link ids; it is view state, never saved)
  function picked(sc = Store.sc) {
    const s = View.roadPick && typeof View.roadPick.has === 'function' ? View.roadPick : (View.roadPick = new Set());
    for (const id of [...s]) if (!sc.links[id]) s.delete(id);
    return s;
  }
  function selectPicked() {
    const ids = [...picked()]; if (!ids.length) return;
    const sel = typeof Onboarding !== 'undefined' && Onboarding.makeSel ? Onboarding.makeSel(ids, []) : ids.length === 1 ? { type: 'link', id: ids[0] } : null;
    if (!sel) return Kit.toast('Selecting several roads at once needs the multi-edit feature (onboarding).', 'warn');
    UI.select(sel);
    Kit.toast(ids.length === 1 ? 'Selected 1 road. Edit it in the side panel.' : `Selected ${U.fmt.int(ids.length)} roads. Edit them together in the side panel; one undo step reverses a change.`);
  }
  // Link results with the capacity source (results-trust (c)): cap_source, limited_by and mid_block_capacity_pcu_h
  function linksCSV(sc = Store.sc, res = Store.res()) {
    const rows = [['link_id', 'name', 'class', 'status', 'direction', 'lanes', 'speed_limit_kmh', 'length_m', 'volume_veh_h', 'volume_pcu_h', 'capacity_pcu_h', 'cap_source', 'limited_by', 'mid_block_capacity_pcu_h', 'v_c', 'los', 'speed_kmh', 'travel_time_s', 'junction_delay_s']];
    for (const L of Object.values(sc.links)) for (const d of ['ab', 'ba']) {
      const lanes = d === 'ab' ? L.lanesAB : L.lanesBA; if (!lanes) continue;
      const r = res && res.arcs[L.id + ':' + d], c = r ? Kit.capSource(sc, res, L.id + ':' + d) : null;
      rows.push([L.id, L.name, L.cls, L.status, d.toUpperCase(), lanes, L.speed, Math.round(Net.len(sc, L)), r && r.v, r && r.pcu, r && r.cap, c ? c.src : '', c && c.src !== 'mid-block' ? c.at : '', c ? c.mid : '', r && r.vc, r && r.los, r && r.spd, r && r.t, r && r.nd]);
    }
    return U.csv(rows);
  }
  function junctionsTab() {
    const res = Store.res();
    if (!res) return emptySteps();
    const rows = Object.entries(res.nodes).sort((a, b) => b[1].delay - a[1].delay);
    if (!rows.length) return '<p class="empty">No controlled junctions in this scenario. Select a junction and set its control to give way, stop, roundabout or signals.</p>';
    return `<div class="bar-h"><h3>Junctions</h3><span class="note">${Kit.gl('LoS')} from average control delay (signals and roundabouts) or the worst movement (give way and stop), ${Kit.gl('HCM')} 6 bands; F whenever demand exceeds capacity. ${U.esc(Kit.junctionLabel(res))}</span></div>
      <div class="tbl-wrap"><table class="t"><tr><th>Junction</th><th>Control</th><th class="n">veh/h</th><th class="n">Highest ${Kit.gl('DoS')}</th><th class="n">Delay</th><th>${Kit.gl('LoS')}</th><th class="n">Cycle</th><th class="n">Longest queue</th></tr>
      ${rows.map(([id, n]) => `<tr class="click" data-fly="node:${U.esc(id)}"><td>${Kit.rowlink(U.esc(junctionName(id)))}</td><td>${U.esc(CONTROLS[n.type] || n.type)}</td><td class="n">${U.fmt.int(n.vol)}</td><td class="n">${U.fmt.num(n.dos, 2)}</td><td class="n">${U.fmt.int(n.delay)} s</td><td>${losBadge(n.los)}</td><td class="n">${n.C ? U.fmt.int(n.C) + ' s' : '–'}</td><td class="n">${U.fmt.int(Math.max(0, ...(n.appr || []).map(a => +a.q95 || 0)))} m</td></tr>`).join('')}</table></div>`;
  }
  function junctionsCSV(res = Store.res()) {
    const rows = [['node_id', 'junction', 'control', 'approach', 'volume_veh_h', 'capacity_veh_h', 'dos', 'delay_s', 'los', 'queue95_m', 'cycle_s']];
    if (res) for (const [id, n] of Object.entries(res.nodes)) { const legs = Kit.legLabels(Store.sc, res, id); for (const a of n.appr) rows.push([id, junctionName(id), n.type, legs.get(a.key) || a.approach || compass(a.inB), a.v, a.cap, a.dos, a.delay, a.los, a.q95, n.C]); }
    return U.csv(rows);
  }
  function demandTab() {
    const sc = Store.sc, res = Store.res(), per = S().period, zones = Object.values(sc.zones);
    if (!zones.length) return emptySteps();
    let sla = '';
    if (View.sla && View.colorBy === 'select') {
      const zn = id => (sc.zones[id] || {}).name || id, pf = Engine.pcuF(S());
      sla = `<div><div class="lg-title">Who uses the selected road</div><div class="tbl-wrap"><table class="t compact"><tr><th>From</th><th>To</th><th class="n">veh/h</th><th class="n">Share</th></tr>
        ${View.sla.ods.slice(0, 14).map(o => `<tr><td>${U.esc(zn(o.o))}</td><td>${U.esc(zn(o.d))}</td><td class="n">${U.fmt.int(o.q / pf)}</td><td class="n">${U.fmt.pct(o.q / View.sla.tot)}</td></tr>`).join('')}</table></div>
        <button class="btn small" data-do="clearsla" style="margin-top:8px">Clear analysis</button></div>`;
    }
    const zt = zones.map(z => ({ z, am: Engine.tripEnds(z, S(), 'AM', sc), pm: Engine.tripEnds(z, S(), 'PM', sc) }));
    const tot = zt.reduce((s, x) => ({ ao: s.ao + x.am.O, ai: s.ai + x.am.D, po: s.po + x.pm.O, pi: s.pi + x.pm.D }), { ao: 0, ai: 0, po: 0, pi: 0 });
    let od = '';
    if (res && res.T && res.zoneIds.length <= 40) {
      const Z = res.zoneIds.length, nm = id => (sc.zones[id] || {}).name || '?';
      od = `<div class="lg-title" style="margin-top:12px">${Kit.gl('OD')} matrix, ${U.esc(per)} peak ${U.esc(res.year)} (veh/h)${res.manual ? ' · manual' : ' · gravity model'}</div>
        <div class="tbl-wrap"><table class="t compact"><tr><th>From \\ To</th>${res.zoneIds.map(id => `<th class="n" title="${U.esc(nm(id))}">${U.esc(nm(id).slice(0, 12))}</th>`).join('')}<th class="n">Total</th></tr>
        ${res.zoneIds.map((id, i) => `<tr><td>${U.esc(nm(id))}</td>${res.zoneIds.map((_, j) => `<td class="n">${i === j ? '' : U.fmt.int(res.T[i * Z + j])}</td>`).join('')}<td class="n"><b>${U.fmt.int(res.zoneIds.reduce((s, _, j) => s + res.T[i * Z + j], 0))}</b></td></tr>`).join('')}</table></div>`;
    } else if (res && res.zoneIds.length > 40) od = `<p class="note">The ${Kit.gl('OD')} matrix has ${U.fmt.int(res.zoneIds.length)} zones. Export it to view it.</p>`;
    const man = sc.odManual && sc.odManual[per];
    return `<div class="bar-h"><h3>Trips (${Kit.gl('OD')})</h3>
        <button class="btn small" data-do="odexport" ${res && res.T ? '' : 'disabled'}>Export ${Kit.gl('OD')} matrix (CSV)</button>
        <button class="btn small" data-do="odimport">Import ${Kit.gl('OD')} matrix…</button>
        ${man ? `<button class="btn small danger" data-do="odclear">Use gravity model again</button>` : ''}
        <span class="note">${man ? `A manual ${Kit.gl('OD')} matrix is driving the ${U.esc(per)} peak.` : 'Zone trips are spread by a gravity model on free-flow drive time; gateway traffic not used by zones passes through.'}</span></div>
      <div class="${sla ? 'row2' : ''}"><div><div class="tbl-wrap"><table class="t compact"><tr><th>Zone</th><th>Type</th><th class="n">AM out</th><th class="n">AM in</th><th class="n">PM out</th><th class="n">PM in</th></tr>
        ${zt.map(x => `<tr class="click" data-fly="zone:${U.esc(x.z.id)}"><td>${Kit.rowlink(U.esc(x.z.name))}</td><td>${x.z.kind === 'gateway' ? 'Gateway' : 'Zone'}</td><td class="n">${U.fmt.int(x.am.O)}</td><td class="n">${U.fmt.int(x.am.D)}</td><td class="n">${U.fmt.int(x.pm.O)}</td><td class="n">${U.fmt.int(x.pm.D)}</td></tr>`).join('')}
        <tr><td><b>Total</b></td><td></td><td class="n"><b>${U.fmt.int(tot.ao)}</b></td><td class="n"><b>${U.fmt.int(tot.ai)}</b></td><td class="n"><b>${U.fmt.int(tot.po)}</b></td><td class="n"><b>${U.fmt.int(tot.pi)}</b></td></tr></table></div>${od}</div>${sla}</div>`;
  }
  function odCSV(res = Store.res(), sc = Store.sc) {
    const Z = res.zoneIds.length, nm = id => (sc.zones[id] || {}).name || id;
    return U.csv([['from \\ to', ...res.zoneIds.map(nm)], ...res.zoneIds.map((id, i) => [nm(id), ...res.zoneIds.map((_, j) => res.T[i * Z + j])])]);
  }

  /* ---------------- traffic data tab ---------------- */
  function dataTab() {
    const d = DataHub.D(), k = DataHub.keys, ld = d.loaded, sc = Store.sc, res = Store.res(), per = S().period;
    const when = x => x ? `<b>${U.fmt.int(x.n)}</b> loaded ${new Date(x.at).toLocaleDateString('en-NZ', { day: 'numeric', month: 'short' })}` : 'Not loaded yet';
    const tag = t => `<span class="pill">${t}</span>`;
    const use = (key, label = 'Use in model checks') => `<label class="check" style="font-size:12.5px"><input type="checkbox" data-use="${key}" ${d.use[key] ? 'checked' : ''}> ${label}</label>`;
    const card = (title, tags, body, actions) => `<div class="src"><h5>${title}</h5><div class="chips">${tags}</div><div class="note">${body}</div><div class="actions">${actions}</div></div>`;
    const cards = [
      card('NZTA state highway counts', tag('real counts') + tag('free, no key') + tag('CC BY 4.0'), `${Kit.gl('AADT')} by direction at every state highway count site, with five years of history and heavy-vehicle share. ${when(ld.nzta)}.`, `<button class="btn small" data-do="load:nzta">Load for this area</button>${use('nzta')}${use('virtual', 'Include NZTA estimates (virtual sites)')}`),
      card('Auckland Transport counts', tag('real counts') + tag('free, no key') + tag('AT open data'), `Latest count at every AT site: daily volume, the measured peak hour and when it happened, vehicle mix and count date. ${when(ld.at)}.`, `<button class="btn small" data-do="load:at">Load for this area</button>${use('at')}`),
      card('Your own counts', tag('real counts') + tag('CSV'), `For SCATS, tube or survey counts you have. Columns <span class="mono">lat, lng, am, pm, adt, dir, name</span>; dir is NB, SB, EB or WB and optional. ${when(ld.user)}.`, `<button class="btn small" data-do="usercounts">Upload CSV</button>${use('user')}`),
      card(`Crashes · NZTA ${Kit.gl('CAS')}`, tag('real records') + tag('free, no key') + tag('CC BY 4.0'), `Every reported crash in five full years plus the year to date${ld.cas ? ` (${U.esc(Kit.casShort())})` : ''}, with its severity, shown on the map and totalled for each road and junction. ${when(ld.cas)}.${ld.casLong ? ` Ten-year history for state highway accesses: ${U.fmt.int(ld.casLong.n)} crashes${ld.casLong.label ? ` (${U.esc(ld.casLong.label)})` : ''}, ${when(ld.casLong)}.` : ''}`, `<button class="btn small" data-do="load:cas">Load for this area</button> <button class="btn small" data-do="load:cas10" title="NZTA asks for ten years of crash history for an access on a state highway; the five-year tables and risk bands are not changed">Ten years (state highway accesses)</button>`),
      card('Live traffic · TomTom', tag('live speeds') + tag('free key'), `Live congestion layer, and speed now against free-flow on any road. Get a free key at <a href="https://developer.tomtom.com/" target="_blank" rel="noopener">developer.tomtom.com</a>.`, `<input id="keyTomtom" type="password" autocomplete="off" placeholder="TomTom API key" value="${U.esc(k.tomtom || '')}" style="flex:1;min-width:150px"><label class="check" style="font-size:12.5px"><input type="checkbox" id="liveToggle" ${View.show.live ? 'checked' : ''}> Live layer</label>`),
      card('Travel times · Google', tag('typical peak times') + tag('your key'), `Google's traffic-aware drive time for the next Tuesday to Thursday at 8 am or 5 pm, to check modelled journey times. Needs a Google Maps Platform key with the Routes API turned on; Google gives a free monthly allowance. Only the times are shown, as Google's terms require.`, `<input id="keyGoogle" type="password" autocomplete="off" placeholder="Google Maps Platform key" value="${U.esc(k.google || '')}" style="flex:1;min-width:150px">`),
    ];
    // model against counts
    const cal = DataHub.calibration();
    let calHtml;
    if (!d.counts.length) calHtml = '<p class="note">Load counts to check the model against what is really on the roads.</p>';
    else if (!res) calHtml = '<p class="note">Run the model to compare it with the counts.</p>';
    else if (!cal || !cal.n) calHtml = '<p class="note">None of the counts sit on a modelled road in this scenario. Import the roads where the counts are.</p>';
    else {
      const scatter = Kit.scatter(cal.rows.map(r => ({ x: r.obs, y: r.model, colour: r.geh < 5 ? 'var(--ok)' : r.geh < 10 ? 'var(--warn)' : 'var(--danger)', fly: 'count:' + r.c.id, title: `${r.c.name}: counted ${Math.round(r.obs)}, model ${Math.round(r.model)}` })),
        { xLabel: 'counted veh/h', yLabel: 'model' });
      const worst = cal.rows.slice(0, 8).map(r => `<tr class="click" data-fly="count:${U.esc(r.c.id)}"><td>${Kit.rowlink(U.esc(String(r.c.name || '').slice(0, 46) + (r.c.dir ? ' ' + r.c.dir : '')))}</td><td class="n">${U.fmt.int(r.obs)}</td><td class="n">${U.fmt.int(r.model)}</td><td class="n">${gehBadge(r.geh)}</td></tr>`).join('');
      calHtml = `<div class="kpis" style="grid-template-columns:repeat(auto-fill,minmax(120px,1fr))">
          ${kpi('Count sites matched', U.fmt.int(cal.n))}${kpi(`${Kit.gl('GEH')} under 5`, U.fmt.pct(cal.pass), 'target 85%', cal.pass >= 0.85 ? 'down' : 'up')}
          ${kpi(cal.r2OriginCentred != null ? U.esc((DataHub.REG_LABELS || {}).r2OriginCentred || 'R² through the origin') : 'R² (OLS)', cal.r2OriginCentred != null ? cal.r2OriginCentred.toFixed(2) : cal.r2 != null ? cal.r2.toFixed(2) : '–', `target 0.85+${cal.b0 != null ? ` · slope ${cal.b0.toFixed(2)}` : ''}`, (cal.r2OriginCentred ?? cal.r2) >= 0.85 ? 'down' : 'up')}${kpi('Model vs counts total', U.fmt.signedPct(cal.totMod / cal.totObs - 1), `${U.fmt.int(cal.totObs)} counted`)}</div>
        <div class="row2" style="margin-top:10px"><div>${scatter}</div><div class="tbl-wrap"><table class="t compact"><tr><th>Worst fits</th><th class="n">Count</th><th class="n">Model</th><th class="n">${Kit.gl('GEH')}</th></tr>${worst}</table></div></div>
        <p class="note" style="margin-top:8px">Counts are today's traffic. A scenario with future growth zones will sit above them on the roads that growth uses; check fit on a base-year scenario. Where the model is well below counts on local roads, existing homes and jobs are missing: add zones for them.</p>`;
    }
    // journey-time checks
    let jtHtml = '<p class="note">Use the Travel time tool (P) and press <b>Add to journey checks</b> to compare modelled times with Google.</p>';
    if (d.jt.length) {
      const rows = d.jt.map(j => {
        let mt = null;
        if (sc.nodes[j.from] && sc.nodes[j.to]) { const r = Engine.travel(sc, res, j.from, j.to); if (r && !r.none) mt = r.time; }
        const g = j.g && j.g[per], diff = mt != null && g ? mt / g.t - 1 : null;
        return `<tr><td>${U.esc(j.name)}</td><td class="n">${U.fmt.dur(mt)}</td><td class="n">${g ? U.fmt.dur(g.t) : '–'}</td><td class="n ${diff == null ? '' : Math.abs(diff) <= 0.15 ? 'down' : 'up'}">${diff == null ? '–' : U.fmt.signedPct(diff, 0)}</td><td class="note">${g ? U.esc(g.when) : ''}</td><td><button class="btn small icon" data-do="jtdel:${U.esc(j.id)}" aria-label="Remove">✕</button></td></tr>`;
      }).join('');
      jtHtml = `<div class="tbl-wrap"><table class="t compact"><tr><th>Journey</th><th class="n">Model (${res ? 'peak' : 'free flow'})</th><th class="n">Google ${U.esc(per)}</th><th class="n">Difference</th><th>Google departure</th><th></th></tr>${rows}</table></div>
        <div class="actions" style="margin-top:6px"><button class="btn small" data-do="jtgoogle">Get Google times (${U.esc(per)} peak)</button><span class="note">${U.esc((DataHub.JT_CRITERIA && DataHub.JT_CRITERIA.text) || 'Within ±15% is a common journey-time validation target.')}</span></div>`;
    }
    return `<div class="bar-h"><h3>Traffic data</h3><button class="btn small primary" data-do="loadall">Load all free data for this area</button><button class="btn small" data-do="gwcounts" ${d.counts.length ? '' : 'disabled'}>Set gateway flows from counts</button>
        <span class="note">Counts and crashes are shared by every scenario. Keys stay in this browser only.</span></div>
      <div class="src-grid">${cards.join('')}</div>
      <div class="row2" style="margin-top:14px"><div><div class="lg-title">Model against counts · ${U.esc(per)} peak · ${U.esc(sc.name)}</div>${calHtml}</div>
        <div><div class="lg-title">Journey-time checks</div>${jtHtml}</div></div>`;
  }
  async function loadData(which) {
    // a cancellable task: Cancel aborts the downloads (results-trust (f)); whatever finished is kept
    let msgs, info = {};
    const cancelled = () => { Store.emit('data'); Kit.toast('Download cancelled. Data that had already arrived is kept.'); };
    try { msgs = await Kit.task('Downloading real-world data…', (signal, status, i) => { info = i; return DataHub.loadAll(DataHub.area(UI.map), which, t => status(t), { signal }); }); }
    catch (e) {
      if (Kit.isAbort(e) || info.cancelled) return cancelled();
      return Kit.toast('The download failed: ' + e.message + '. Try again, or zoom in to a smaller area.', 'err', { details: String(e.stack || e) });
    }
    // loadAll reports each source's failure as text, so a cancel can come back as 'did not answer': say what happened
    if (info.cancelled) return cancelled();
    if (which.includes('cas')) View.show.crashes = true;
    View.show.counts = true;
    Store.emit('data');
    Kit.toast('Loaded ' + msgs.join(', ') + '.', msgs.some(m => /failed|cancel/i.test(m)) ? 'warn' : '');
    Kit.log('Data: ' + U.esc(msgs.join(', ')));
  }
  function gatewaysFromCounts() {
    Store.snapshot();
    let n = 0;
    for (const sc of Store.P.scenarios) { const k = DataHub.gatewaysFromCounts(sc); if (sc === Store.sc) n = k; for (const r of Object.values(sc.res)) if (r) r.stale = true; }
    Store.emit('demand');
    Kit.toast(n ? `Set ${n} gateway${n === 1 ? '' : 's'} from real counts in every scenario. Run the model again.` : 'No counts found on the roads at the gateways. Load counts first, or the gateway roads have no count sites.', n ? '' : 'warn');
  }
  async function journeyGoogle() {
    const d = DataHub.D(), per = S().period;
    if (!DataHub.keys.google) return Kit.toast('Add your Google Maps Platform key first.', 'warn');
    Kit.busy('Asking Google for peak travel times…');
    let ok = 0, err = '';
    try {
      for (const j of d.jt) {
        try { const r = await DataHub.googleTime(j.fromLL, j.toLL, per); (j.g ||= {})[per] = r; ok++; }
        catch (e) { err = e.message; }
      }
    } finally { Kit.busy(null); }
    Store.emit('data');
    Kit.toast(ok ? `Got ${ok} Google travel time${ok === 1 ? '' : 's'}.` : 'Google did not answer: ' + err, ok ? '' : 'err');
  }

  /* ---------------- simulation and log ---------------- */
  function simHtml() {
    const sim = View.sim;
    if (!sim) return `<div class="bar-h"><h3>Simulation</h3><button class="btn small primary" data-do="sim">▶ Start simulation</button></div>
      <p class="note">The simulation releases vehicles on the routes found by the model and drives them with a car-following model: they queue at red lights, give way at priority junctions, yield at roundabouts and keep left. Use it to see queues form and check the model. Zoom in to street level to see individual vehicles.</p>
      ${fld('Demand', `<select id="simScale">${opt({ 0.5: '50% of modelled', 0.75: '75%', 1: '100%', 1.25: '125%', 1.5: '150%' }, S().sim.scale)}</select>`)}`;
    const st = sim.stats, ser = st.series, val = sim.validation();
    const mxN = Math.max(10, ...ser.map(s => s.n)), mxT = Math.max(60, sim.t);
    const chart = Kit.line([
      { pts: ser.map(s => [s.t, s.n]), colour: 'var(--accent)', width: 2, label: 'vehicles on road' },
      { pts: ser.map(s => [s.t, s.q]), colour: 'var(--los-e)', width: 1.6, label: 'queued' },
      { pts: ser.map(s => [s.t, s.spd]), yMax: 80, colour: 'var(--sel)', width: 1.4, dash: '4 3', label: 'average speed (0–80 km/h)' },
    ], { xMax: mxT, yMax: mxN, yLabel: U.fmt.int(mxN), xLabels: ['0 min', `${U.fmt.num(mxT / 60, 0)} min`] });
    const vtab = val ? `<div><div class="lg-title">Simulated vs modelled flow · ${U.fmt.pct(val.pass)} of links ${Kit.gl('GEH')} &lt; 5</div><div class="tbl-wrap"><table class="t compact"><tr><th>Road</th><th class="n">Model</th><th class="n">Sim</th><th class="n">GEH</th></tr>
      ${val.rows.slice(0, 10).map(r => `<tr class="click" data-fly="link:${U.esc(r.link)}"><td>${Kit.rowlink(`${U.esc(r.name || '—')} ${r.key.endsWith('ab') ? '→' : '←'}`)}</td><td class="n">${U.fmt.int(r.model)}</td><td class="n">${U.fmt.int(r.sim)}</td><td class="n" style="color:${r.geh < 5 ? 'var(--ok)' : 'var(--danger)'}">${r.geh.toFixed(1)}</td></tr>`).join('')}</table></div>
      <p class="note">${Kit.gl('GEH')} compares the two flows. Flows over the last 5 simulated minutes. Long roads from gateways take 10–15 minutes to fill.</p></div>` : '<p class="note">Model comparison appears after 7 simulated minutes.</p>';
    return `<div class="bar-h"><h3>Simulation · ${U.fmt.clock(sim.startClock + sim.t)}</h3><span class="note">${U.fmt.int(st.spawned)} released · ${U.fmt.int(st.done)} finished · ${st.teleports} moved on after gridlock</span>
        <label class="check">Colour <select id="simColour">${opt({ speed: 'by speed', type: 'by vehicle type' }, View.simColour)}</select></label></div>
      <div class="row2"><div>${chart}</div>${vtab}</div>`;
  }
  // Called by the simulation loop to refresh the tab in place
  function renderSimTab() { if (View.drawer === 'sim' && View.drawerOpen) U.$('#drawerBody').innerHTML = simHtml(); }
  function logTab() { const L = typeof UI !== 'undefined' ? UI.logLines : []; return `<div class="bar-h"><h3>Log</h3></div>${L.slice().reverse().map(l => `<div class="log-line">${l}</div>`).join('') || '<p class="note">Nothing yet.</p>'}`; }

  /* ---------------- checklist (registered items) ---------------- */
  const CK_ICON = { done: ['A', '✓'], warn: ['D', '!'], todo: ['none', '·'], na: ['none', '–'] };
  function checklistItems(P = Store.P) {
    return Registry.list('checklist').map(d => {
      const st = Registry.call(d, 'status', P) || { state: 'na', text: 'Could not be checked.' };
      return { def: d, group: d.group || 'Other', state: CK_ICON[st.state] ? st.state : 'todo', text: st.text || d.label || d.id, action: st.action };
    });
  }
  function checklistTab() {
    const items = checklistItems(), groups = [...new Set(items.map(i => i.group))];
    const done = items.filter(i => i.state === 'done').length, open = items.filter(i => i.state === 'todo' || i.state === 'warn').length;
    return `<div class="bar-h"><h3>Checklist</h3><span class="note">${done} done · ${open} to do · items come from the installed features</span></div>
      <div class="ck-grid">${groups.map(g => `<div><div class="lg-title">${U.esc(g)}</div>${items.filter(i => i.group === g).map(i => {
        const [cls, sym] = CK_ICON[i.state];
        return `<div class="lg-row ck-row"><span class="los ${cls}" aria-label="${i.state}">${sym}</span><span>${i.text}${i.def.source ? ` <span class="note">${U.esc(i.def.source)}</span>` : ''}</span>${i.action ? `<button class="btn small" data-ck="${U.esc(i.def.id)}">${U.esc(i.action.label || 'Fix')}</button>` : ''}</div>`;
      }).join('')}</div>`).join('')}</div>`;
  }

  /* The GeoJSON export carries the project's assumptions (about-data (e)): a top-level 'corridor' member with the
     settings changed from Spare Construct's defaults, the result's period, year and convergence, and the data sources. */
  function geojsonOut(P, c) {
    const g = JSON.parse(Importers.geojson(c.sc, c.res)), cs = Kit.convState(c.res);
    g.corridor = {
      project: P.name, scenario: c.sc.name, period: c.res ? c.res.period : null, year: c.res ? c.res.year : Engine.yearOf(c.sc),
      results: c.res ? (c.res.stale ? 'out of date' : 'current') : 'not run', convergence: cs ? cs.text : null,
      assumptions: Kit.nonDefaults(P.settings).map(a => ({ setting: a.key, label: a.label, value: a.value, default: a.def, unit: a.unit })),
      sources: Kit.dataSources(P).map(d => ({ what: d.what, source: d.source, licence: d.licence, queried: d.date, basis: d.basis })),
    };
    return JSON.stringify(g, null, 1);
  }

  /* ---------------- built-in tabs and exports ---------------- */
  [
    { id: 'summary', label: 'Summary', order: 10, render: summaryTab },
    { id: 'compare', label: 'Compare scenarios', order: 20, render: compareTab },
    { id: 'data', label: 'Traffic data', order: 30, render: dataTab },
    { id: 'links', label: 'Roads', order: 40, render: linksTab },
    { id: 'junctions', label: 'Junctions', order: 50, render: junctionsTab },
    { id: 'demand', label: 'Trips (OD)', order: 60, title: 'Trips by zone and the origin–destination matrix', render: demandTab },
    { id: 'sim', label: 'Simulation', order: 70, render: simHtml },
    { id: 'checklist', label: 'Checklist', order: 80, render: checklistTab, hidden: () => !Registry.list('checklist').length },
    // the run log moved to Menu › Run log (wording); the tab stays registered (hidden) for older code that opens it
    { id: 'log', label: 'Log', order: 90, render: logTab, hidden: () => View.drawer !== 'log' },
  ].forEach(Registry.tabs);
  [
    { id: 'geojson', label: 'Network + results (GeoJSON)', ext: 'geojson', mime: 'application/geo+json', order: 10, filename: c => c.fileBase + '.geojson', build: (P, c) => geojsonOut(P, c) },
    { id: 'links', label: 'Link results (CSV)', ext: 'csv', mime: 'text/csv', order: 20, stamp: true, build: (P, c) => linksCSV(c.sc, c.res) },
    { id: 'junctions', label: 'Junction results (CSV)', ext: 'csv', mime: 'text/csv', order: 30, stamp: true, build: (P, c) => junctionsCSV(c.res) },
    { id: 'comparison', label: 'Scenario comparison (CSV)', ext: 'csv', mime: 'text/csv', order: 40, stamp: true, build: () => compareCSV() },
    { id: 'assumptions', label: 'Assumptions: settings changed from defaults (CSV)', ext: 'csv', mime: 'text/csv', order: 45, stamp: true, filename: c => c.fileBase + '-assumptions.csv', build: P => Kit.assumptionsCSV(P.settings) },
    { id: 'od', label: 'OD matrix (CSV)', ext: 'csv', mime: 'text/csv', order: 50, enabled: c => !!(c.res && c.res.T), filename: c => c.fileBase + `-od-${c.res.period}.csv`, build: (P, c) => odCSV(c.res, c.sc) },
  ].forEach(Registry.exports);

  /* ---------------- rendering and events ---------------- */
  const visibleTabs = () => Registry.list('tabs', t => !(t.hidden && Registry.call(t, 'hidden')));
  // Body HTML for a tab id (headless-safe)
  function html(id = View.drawer) {
    const t = Registry.get('tabs', id) || Registry.get('tabs', 'summary');
    return Registry.html(t, 'render');
  }
  /* Tabs follow the ARIA tabs pattern (accessibility (3)): role=tab with aria-selected and aria-controls, a roving
     tabindex (Left, Right, Home, End move between tabs) and #drawerBody as the tabpanel labelled by the active tab. */
  function tabsHtml(tabs = visibleTabs(), cur = View.drawer) {
    return tabs.map(t => { const on = cur === t.id;
      return `<button type="button" class="tab ${on ? 'on' : ''}" id="tab-${U.esc(t.id)}" data-tab="${U.esc(t.id)}" role="tab" aria-selected="${on}" aria-controls="drawerBody" tabindex="${on ? 0 : -1}"${t.title ? ` title="${U.esc(t.title)}"` : ''}>${U.esc(t.label)}</button>`; }).join('');
  }
  function render(keepFocus) {
    const d = U.$('#drawer');
    const wasOpen = !d.classList.contains('closed');
    d.classList.toggle('closed', !View.drawerOpen);
    const tabs = visibleTabs();
    if (!tabs.some(t => t.id === View.drawer)) View.drawer = tabs[0] ? tabs[0].id : 'summary';
    const tabsEl = U.$('#drawer .tabs'), tabFocus = document.activeElement && tabsEl.contains(document.activeElement);
    const toggle = U.$('#drawerToggle');
    tabsEl.innerHTML = tabsHtml(tabs) + (toggle ? '' : `<button type="button" class="tab tab-toggle">${View.drawerOpen ? '▾ Hide' : '▴ Show'}</button>`);
    if (toggle) { toggle.textContent = View.drawerOpen ? '▾ Hide' : '▴ Show'; toggle.title = `${View.drawerOpen ? 'Hide' : 'Show'} the results panel`; toggle.setAttribute('aria-expanded', View.drawerOpen); toggle.setAttribute('aria-controls', 'drawerBody'); }
    if (tabFocus) { const f = U.$('#tab-' + CSS.escape(View.drawer)); f && f.focus(); }
    applyHeight();
    if (wasOpen !== View.drawerOpen) setTimeout(() => UI.map.invalidateSize(), 0);
    const body = U.$('#drawerBody');
    body.setAttribute('aria-labelledby', 'tab-' + View.drawer);
    if (!View.drawerOpen) return;
    const ae = document.activeElement, fid = keepFocus && ae && ae.id;
    const pos = ae && ae.selectionStart;
    body.innerHTML = html(View.drawer);
    if (fid) { const f = document.getElementById(fid); if (f) { f.focus(); try { f.setSelectionRange(pos, pos); } catch { } } }
  }
  /* Drawer height (map-card): drag the grip between 38 px and 70% of the window (90% on a phone, where it snaps to
     38 px, 45% and 90%), use the arrow keys on it, or double-click a tab to maximise. The height is remembered in
     this browser only. */
  const MIN_H = 38;
  const pref = () => typeof UI !== 'undefined' && UI.pref ? UI.pref : { get: (k, d) => d, set() {} };
  const phone = () => Kit.narrow();
  const maxH = () => Math.round(window.innerHeight * (phone() ? 0.9 : 0.7));
  let height = null, preMax = null;
  function applyHeight() {
    const d = U.$('#drawer'); if (!d) return;
    if (height == null) height = pref().get('drawerH', null);
    d.style.height = View.drawerOpen && height ? Math.min(maxH(), Math.max(MIN_H, height)) + 'px' : '';
  }
  function setHeight(h, save = true) {
    h = Math.round(Math.min(maxH(), Math.max(MIN_H, h)));
    if (save && phone()) { const snaps = [MIN_H, Math.round(window.innerHeight * 0.45), maxH()]; h = snaps.reduce((a, b) => Math.abs(b - h) < Math.abs(a - h) ? b : a); }
    const open = h > MIN_H + 6;
    if (open) height = h;
    if (open !== View.drawerOpen) { View.drawerOpen = open; render(); } else applyHeight();
    if (save) { if (open) pref().set('drawerH', h); setTimeout(() => UI.map.invalidateSize(), 0); }
  }
  function toggleMax() {
    const cur = U.$('#drawer').getBoundingClientRect().height;
    if (View.drawerOpen && cur >= maxH() - 4) setHeight(preMax || 270);
    else { preMax = View.drawerOpen ? cur : height; View.drawerOpen = true; setHeight(maxH()); }
  }
  function bindGrip() {
    const grip = U.$('#drawerGrip'), d = U.$('#drawer'); if (!grip) return;
    grip.addEventListener('pointerdown', e => {
      if (e.button) return;
      e.preventDefault();
      const y0 = e.clientY, h0 = d.getBoundingClientRect().height;
      try { grip.setPointerCapture(e.pointerId); } catch { }
      document.body.classList.add('resizing');
      const move = ev => setHeight(h0 + (y0 - ev.clientY), false);
      const up = ev => {
        grip.removeEventListener('pointermove', move); grip.removeEventListener('pointerup', up); grip.removeEventListener('pointercancel', up);
        document.body.classList.remove('resizing');
        setHeight(h0 + (y0 - ev.clientY), true);
      };
      grip.addEventListener('pointermove', move); grip.addEventListener('pointerup', up); grip.addEventListener('pointercancel', up);
    });
    grip.addEventListener('keydown', e => {
      const cur = View.drawerOpen ? d.getBoundingClientRect().height : MIN_H;
      const step = e.shiftKey ? 120 : 30;
      const to = { ArrowUp: cur + step, ArrowDown: cur - step, Home: MIN_H, End: maxH(), PageUp: cur + 120, PageDown: cur - 120 }[e.key];
      if (to == null) return;
      e.preventDefault(); setHeight(to, true);
    });
    grip.addEventListener('dblclick', toggleMax);
  }
  function open(id) { View.drawer = id; View.drawerOpen = true; render(); }
  // The active tab's own handler runs first; returning true skips the shared handling below
  const toTab = (e, fn) => Registry.call(Registry.get('tabs', View.drawer), fn, e, U.$('#drawerBody')) === true;

  function onClick(e) {
    if (toTab(e, 'onClick')) return;
    // the row checkboxes are handled on change; a click on one must not open the road
    if (e.target.closest('[data-rsel], [data-rsel-all]')) return;
    const fl = e.target.closest('[data-fly]');
    if (fl) { const f = fl.dataset.fly, i = f.indexOf(':'); return UI.flyTo(f.slice(0, i), f.slice(i + 1)); }
    const so = e.target.closest('[data-sort]');
    if (so) { const k = so.dataset.sort; View.linkSort = { k, dir: View.linkSort.k === k ? -View.linkSort.dir : (k === 'name' ? 1 : -1) }; return render(); }
    const ck = e.target.closest('[data-ck]');
    if (ck) { const it = checklistItems().find(i => i.def.id === ck.dataset.ck); if (it && it.action && it.action.run) { try { it.action.run(); } catch (err) { console.error(err); Kit.toast('That step could not start: ' + err.message, 'err'); } } return; }
    const b = e.target.closest('[data-do]'); if (!b) return;
    const d = b.dataset.do;
    if (d.startsWith('tool:')) return UI.setTool(d.slice(5), true);
    if (d.startsWith('tab:')) return open(d.slice(4));
    if (d === 'assumptions') return UI.exportFile('assumptions');
    if (d === 'settings') return Dialogs.settings();
    // re-run with a tighter convergence target: this scenario (summary) or both sides of a comparison (signal to noise)
    const tight = () => ({ assign: { gapTol: Math.min(+S().gapTol || 1e-4, 1e-5), maxIter: Math.max(400, +S().maxIter || 0) } });
    if (d === 'tighter') return UI.runModel([Store.sc], [S().period], tight());
    if (d.startsWith('snr:')) {
      const opt = Store.P.scenarios.find(s => s.id === d.slice(4)), ec = opt && Engine.econ(Store.baseSc, opt, S().period);
      const t = ec && ec.snr && ec.snr.tighter ? { assign: ec.snr.tighter } : tight();
      // the noise check takes the noisiest peak, which need not be the one on screen: re-run every peak both have run
      const per = ['AM', 'PM'].filter(p => Store.res(Store.baseSc, p) && opt && Store.res(opt, p));
      return UI.runModel([Store.baseSc, opt].filter(Boolean), per.length ? per : [S().period], t);
    }
    if (d === 'selmap') return selectPicked();
    if (d === 'selclear') { picked().clear(); return render(); }
    if (d === 'runall') UI.runModel(Store.P.scenarios, ['AM', 'PM']);
    if (d === 'sim') UI.toggleSim();
    if (d === 'clearsla') { View.sla = null; UI.setColour('vc'); render(); }
    if (d === 'odexport') UI.exportFile('od');
    if (d === 'odimport') Dialogs.openFile('od');
    if (d === 'odclear') { Store.snapshot(); delete Store.sc.odManual[S().period]; Store.edited('demand'); }
    if (d === 'loadall') loadData(['nzta', 'at', 'cas']);
    if (d.startsWith('load:')) loadData([d.slice(5)]);
    if (d === 'usercounts') Dialogs.openFile('counts');
    if (d === 'gwcounts') gatewaysFromCounts();
    if (d === 'jtgoogle') journeyGoogle();
    if (d.startsWith('jtdel:')) { const D = DataHub.D(); D.jt = D.jt.filter(j => j.id !== d.slice(6)); Store.emit('data'); }
  }
  function onChange(e) {
    if (toTab(e, 'onChange')) return;
    const t = e.target;
    if (t.dataset.do === 'over') { View.linkOver = t.checked; render(); }
    if (t.dataset.rsel != null) { const s = picked(); if (t.checked) s.add(t.dataset.rsel); else s.delete(t.dataset.rsel); render(true); }
    if (t.dataset.rselAll != null) { const s = picked(); for (const b of U.$$('[data-rsel]', U.$('#drawerBody'))) { if (t.checked) s.add(b.dataset.rsel); else s.delete(b.dataset.rsel); } render(true); }
    if (t.id === 'simScale') { S().sim.scale = +t.value; Store.saveSoon(); }
    if (t.id === 'simColour') { View.simColour = t.value; Render.redrawFx(); }
    if (t.dataset.use) { DataHub.D().use[t.dataset.use] = t.checked; Store.emit('data'); }
    if (t.id === 'keyTomtom') { DataHub.setKey('tomtom', t.value); if (View.show.live) UI.setLive(true); Kit.toast(t.value ? 'TomTom key saved in this browser.' : 'TomTom key removed.'); }
    if (t.id === 'keyGoogle') { DataHub.setKey('google', t.value); Kit.toast(t.value ? 'Google key saved in this browser.' : 'Google key removed.'); }
    if (t.id === 'liveToggle') { UI.setLive(t.checked); Inspector.render(); }
  }
  function bindEvents() {
    const tabsEl = U.$('#drawer .tabs');
    tabsEl.addEventListener('click', e => {
      const b = e.target.closest('[data-tab]');
      if (b) { open(b.dataset.tab); return; }
      if (e.target.closest('.tab-toggle')) { View.drawerOpen = !View.drawerOpen; render(); }
    });
    tabsEl.addEventListener('dblclick', e => { if (e.target.closest('[data-tab]')) toggleMax(); });
    tabsEl.addEventListener('keydown', e => {
      const b = e.target.closest('[role="tab"]'); if (!b) return;
      const list = [...tabsEl.querySelectorAll('[role="tab"]')], i = list.indexOf(b), n = list.length;
      const j = { ArrowRight: (i + 1) % n, ArrowLeft: (i - 1 + n) % n, Home: 0, End: n - 1 }[e.key];
      if (j == null) return;
      e.preventDefault();
      open(list[j].dataset.tab);
      const f = U.$('#tab-' + CSS.escape(list[j].dataset.tab)); f && f.focus();
    });
    const tg = U.$('#drawerToggle');
    if (tg) tg.addEventListener('click', () => { View.drawerOpen = !View.drawerOpen; render(); });
    bindGrip();
    window.addEventListener('resize', () => applyHeight());
    const body = U.$('#drawerBody');
    body.addEventListener('click', onClick);
    body.addEventListener('change', onChange);
    body.addEventListener('keydown', e => { toTab(e, 'onKeydown'); });
    body.addEventListener('input', e => { if (toTab(e, 'onInput')) return; if (e.target.id === 'linkFilter') { View.linkFilter = e.target.value; render(true); } });
  }

  return { html, render, open, bindEvents, renderSimTab, visibleTabs, tabsHtml, setHeight, checklistItems, compareCSV, junctionsCSV, linksCSV, odCSV, geojsonOut, emptySteps, noteActions, assumptionsHtml, KPI_ROWS, loadData };
})();
