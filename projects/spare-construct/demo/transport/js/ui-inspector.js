'use strict';
/* Spare Construct · inspector (the right-hand panel): one panel per selected object (road, junction,
   zone, gateway, bus route, count) or the scenario overview, plus registered panels for new kinds
   and registered inspector sections. Fields bound with Kit.bind are written by applyField. */

const Inspector = (() => {
  const S = () => Store.S;
  const { opt, fld, bind, fact, losBadge, gehBadge, compass } = Kit;
  const SECTIONS = '<!--sections-->';
  const ZCLASS_TEXT = { existing: 'Existing land use', committed: 'Committed development', development: 'Proposed development', external: 'Gateway' };
  // Built-in show/hide toggles; registered layers are listed after these
  const SHOW = [['nodes', 'Junctions'], ['zones', 'Zones'], ['connectors', 'Zone access lines'], ['routes', 'Bus routes'], ['labels', 'Labels'], ['counts', 'Traffic counts'], ['crashes', 'Crashes'], ['live', 'Live traffic']];

  // Which panel type a selection shows: 'link' | 'node' | 'zone' | 'gateway' | 'route' | 'count' | 'overview' | a registered type
  function kindOf(sel, sc = Store.sc) {
    if (!sel) return 'overview';
    if (sel.type === 'link') return sc.links[sel.id] ? 'link' : null;
    if (sel.type === 'node') return sc.nodes[sel.id] ? 'node' : null;
    if (sel.type === 'zone') return sc.zones[sel.id] ? (sc.zones[sel.id].kind === 'gateway' ? 'gateway' : 'zone') : null;
    if (sel.type === 'route') return sc.routes[sel.id] ? 'route' : null;
    if (sel.type === 'count') return DataHub.D().counts.some(c => c.id === sel.id) ? 'count' : null;
    return Registry.get('panels', sel.type) ? sel.type : null;
  }

  // Inspector HTML for a selection (null = scenario overview). Pure: reads the store only.
  function html(sel = View.sel, sc = Store.sc) {
    let kind = kindOf(sel, sc), h;
    if (!kind) { kind = 'overview'; sel = null; }
    if (kind === 'link') h = linkPanel(sc, sc.links[sel.id]);
    else if (kind === 'node') h = nodePanel(sc, sc.nodes[sel.id]);
    else if (kind === 'zone') h = zonePanel(sc, sc.zones[sel.id]);
    else if (kind === 'gateway') h = gatewayPanel(sc, sc.zones[sel.id]);
    else if (kind === 'route') h = routePanel(sc, sc.routes[sel.id]);
    else if (kind === 'count') h = countPanel(sc, DataHub.D().counts.find(c => c.id === sel.id));
    else if (kind === 'overview') h = overviewPanel(sc);
    else { h = Registry.html(Registry.get('panels', kind), 'render', sc, sel); if (!h) { kind = 'overview'; sel = null; h = overviewPanel(sc); } }
    const secs = sectionsHtml(kind, sc, sel);
    h = h.includes(SECTIONS) ? h.replace(SECTIONS, secs) : h + secs;
    return { html: h, kind, sel };
  }
  function sectionsHtml(kind, sc, sel) {
    return Registry.list('inspectorSections', d => d.types.includes(kind)).map(d => {
      const h = Registry.html(d, 'render', sc, sel);
      return h ? `<div class="reg-sec" data-sec="${U.esc(d.id)}">${h}</div>` : '';
    }).join('');
  }

  let shownKind = 'overview';
  // Writes into #insBody (the phone bottom sheet keeps its grip above it), or #inspector without one
  function render() {
    const el = U.$('#insBody') || U.$('#inspector'), sc = Store.sc;
    const ae = document.activeElement, focusKey = ae && el.contains(ae) && ae.dataset.k ? ae.dataset.k : null;
    const r = html(View.sel, sc);
    if (!r.sel) View.sel = null;
    shownKind = r.kind;
    el.innerHTML = r.html;
    if (focusKey) { const f = el.querySelector(`[data-k="${CSS.escape(focusKey)}"]`); if (f) f.focus(); }
    // the phone and tablet-portrait bottom sheet (CSS max-width: 820px) opens with a selection; wider, it is never set
    document.body.classList.toggle('ins-open', Kit.narrow() && !!View.sel);
  }

  const head = (kind, title, o, id, extra = '') => `<div class="ins-head"><div class="ins-kind">${kind}${extra}<span style="margin-left:auto"><button class="btn small icon" data-act="close" title="Close (Esc)" aria-label="Close">✕</button></span></div>
    <input class="ins-title" ${bind(o, id, 'name')} value="${U.esc(title)}" placeholder="Unnamed" aria-label="Name"></div>`;

  function linkPanel(sc, Lk) {
    const C = CLASSES[Lk.cls] || CLASSES.local, len = Net.len(sc, Lk), res = Store.res(), g = Net.geom(sc, Lk);
    const bAB = U.bearing(g[0], g[g.length - 1]);
    const dirs = [['ab', Lk.lanesAB, compass(bAB)], ['ba', Lk.lanesBA, compass(bAB + 180)]];
    const E = S().econ, cost = Lk.status === 'proposed' ? len / 1000 * ((Lk.lanesAB | 0) + (Lk.lanesBA | 0)) * (E.laneKm[Lk.cls] ?? 3) : null;
    const lanesSel = (k, v) => `<select ${bind('link', Lk.id, k, 'int')}>${[0, 1, 2, 3, 4, 5, 6].map(n => `<option ${n === (v | 0) ? 'selected' : ''}>${n}</option>`).join('')}</select>`;
    let resHtml = '';
    if (res) {
      const live = dirs.filter(d => d[1] > 0);
      const rows = live.map(([d, , name]) => {
        const r = res.arcs[Lk.id + ':' + d];
        if (!r) return `<tr><td>${name}</td><td colspan="5" class="note">Not in the last run</td></tr>`;
        return `<tr><td>${name}</td><td class="n">${U.fmt.int(r.v)}</td><td class="n">${U.fmt.int(r.cap)}</td><td class="n">${U.fmt.num(r.vc, 2)}</td><td>${losBadge(r.los)}</td><td class="n">${U.fmt.int(r.spd)}</td></tr>`;
      }).join('');
      // Why each direction's capacity is what it is (results-trust (c)), e.g. limited by a give way downstream
      const caps = live.map(([d, , name]) => { const c = Kit.capSource(sc, res, Lk.id + ':' + d); return c ? `<p class="note${c.src !== 'mid-block' ? ' cap-src' : ''}">${live.length > 1 ? `<b>${name}:</b> ` : ''}${U.esc(c.text)}</p>` : ''; }).join('');
      const tot = dirs.reduce((s, [d]) => s + ((res.arcs[Lk.id + ':' + d] || {}).v || 0), 0);
      resHtml = `<div class="ins-sec"><h4><span>${U.esc(S().period)} peak ${U.esc(res.year)}</span><span>${res.stale ? '<span class="pill stale">out of date</span> ' : ''}${Kit.convPill(res)}</span></h4>
        <div class="tbl-wrap"><table class="t compact"><tr><th>Direction</th><th class="n">veh/h</th><th class="n" title="Capacity in passenger car units an hour">Cap (${Kit.gl('pcu')}/h)</th><th class="n">${Kit.gl('V/C')}</th><th>${Kit.gl('LoS')}</th><th class="n">km/h</th></tr>${rows}</table></div>
        <p class="note">Traffic is in vehicles an hour; capacity and ${Kit.gl('V/C')} count each heavy vehicle as ${U.esc(U.fmt.num(+S().pcuHV || 0, 1))} ${Kit.gl('pcu')}.</p>
        ${caps}
        <div class="facts">${fact(U.fmt.int(tot * S().peakToDaily), `daily traffic (${Kit.gl('AADT')} est.)`)}${fact(Engine.lts(Lk, res) ? Kit.gl('LTS') + ' ' + U.esc(Engine.lts(Lk, res)) : '—', `cycling stress (${Kit.gl('LTS')})`)}</div>
        ${vdChart(Lk, res, live)}
        <div class="actions"><button class="btn small" data-act="sla" ${res.paths ? '' : 'disabled title="Run the model again to enable"'}>Who uses this road</button></div></div>`;
    }
    return `${head(`${U.esc(C.label)} road <span class="pill ${U.esc(Lk.status)}">${U.esc(STATUS[Lk.status] || Lk.status)}</span>`, Lk.name, 'link', Lk.id)}
      <div class="ins-sec"><h4>Road</h4>
        ${fld('Class', `<select ${bind('link', Lk.id, 'cls')}>${opt(Object.fromEntries(Object.entries(CLASSES).map(([k, c]) => [k, c.label])), Lk.cls)}</select>`)}
        ${fld('Status', `<select ${bind('link', Lk.id, 'status')}>${opt(STATUS, Lk.status)}</select>`)}
        ${fld('Speed limit', `<select ${bind('link', Lk.id, 'speed', 'int')}>${opt(Object.fromEntries((SPEEDS.includes(Lk.speed) ? SPEEDS : [...SPEEDS, Lk.speed].sort((a, b) => a - b)).map(s => [s, s + ' km/h'])), Lk.speed)}</select>`)}
        <p class="note">Speed limit from ${U.esc(speedSource(Lk))}.</p>
        ${fld('Lanes', `<span class="pair" style="width:100%"><label>${dirs[0][2]}${lanesSel('lanesAB', Lk.lanesAB)}</label><label>${dirs[1][2]}${lanesSel('lanesBA', Lk.lanesBA)}</label></span>`)}
        ${fld('Capacity', `<input type="number" min="100" max="2600" step="50" ${bind('link', Lk.id, 'capPerLane', 'numnull')} value="${U.esc(Lk.capPerLane ?? '')}" placeholder="${U.esc(C.cap)}">`, `${Kit.gl('pcu')}/h/lane`)}
        <label class="check"><input type="checkbox" ${bind('link', Lk.id, 'busLane', 'bool')} ${Lk.busLane ? 'checked' : ''} ${Math.max(Lk.lanesAB, Lk.lanesBA) < 2 ? 'disabled' : ''}> Kerbside bus lane ${Math.max(Lk.lanesAB, Lk.lanesBA) < 2 ? '<span class="note">(needs 2+ lanes)</span>' : ''}</label>
        ${fld('Cycling', `<select ${bind('link', Lk.id, 'cycle')}>${opt(CYCLE, Lk.cycle || 'none')}</select>`)}
        ${crossSection(Lk, dirs)}
        <div class="facts">${fact(U.fmt.len(len), 'length')}${fact(U.fmt.dur(len / ((Lk.speed || C.speed) / 3.6)), 'at the speed limit')}${cost != null ? fact(U.fmt.money(cost), 'indicative cost') : ''}${fact(U.fmt.num(carriageway(Lk), 1) + ' m', 'carriageway')}</div>
      </div>
      ${resHtml}
      ${realData(sc, { type: 'link', id: Lk.id })}
      ${SECTIONS}
      <div class="ins-sec"><div class="actions">
        <button class="btn small" data-act="reverse" title="Swap the lanes in each direction">Swap directions</button>
        <button class="btn small" data-act="elev">Gradient profile</button>
        <button class="btn small" data-act="zoom">Zoom to</button>
        <button class="btn small danger" data-act="delete">Delete road</button>
      </div></div>`;
  }
  // Where the speed limit came from (link inspector extras)
  function speedSource(Lk) {
    if (Lk.speedSource === 'NSLR') return `the NZTA National Speed Limit Register${Lk.speedLimit != null && Lk.speedLimit !== Lk.speed ? ` (legal limit ${Lk.speedLimit} km/h; the model uses ${Lk.speed} km/h)` : ''}`;
    if (Lk.speedSource) return Lk.speedSource;
    if (Lk.status === 'proposed') return 'your design (proposed road)';
    return Lk.osm || Lk.osmId || Lk.way ? 'OpenStreetMap (its maxspeed tag, or the road-class default where there is none)' : 'you, or the road-class default';
  }
  /* Volume–delay chart: the mid-block speed-flow curve (BPR) with each direction's modelled point. Junction delay
     is on top of this; the chart shows travel time as a multiple of free-flow time against V/C. */
  function vdChart(Lk, res, live) {
    const a = +S().bprAlpha || 0.15, b = +S().bprBeta || 4, f = x => 1 + a * Math.pow(x, b);
    const pts = live.map(([d, , name]) => { const r = res.arcs[Lk.id + ':' + d]; return r ? { name, vc: r.vc, mid: f(r.vc), tot: r.ff > 0 ? r.t / r.ff : null } : null; }).filter(Boolean);
    if (!pts.length) return '';
    const xMax = Math.max(1.4, ...pts.map(p => Math.min(3, p.vc) + 0.1)), yMax = Math.max(f(xMax), ...pts.map(p => Math.min(8, p.tot || 1))) * 1.08;
    const W = 300, H = 132, X = x => 34 + (W - 44) * x / xMax, Y = y => H - 22 - (H - 34) * (y - 1) / Math.max(0.1, yMax - 1);
    let path = ''; for (let i = 0; i <= 40; i++) { const x = xMax * i / 40; path += `${i ? 'L' : 'M'}${X(x).toFixed(1)},${Y(f(x)).toFixed(1)}`; }
    const col = ['var(--sel)', 'var(--amber)'];
    const dots = pts.map((p, i) => `<circle cx="${X(Math.min(p.vc, xMax)).toFixed(1)}" cy="${Y(p.mid).toFixed(1)}" r="4.5" fill="${col[i]}" stroke="var(--panel)" stroke-width="1.5"><title>${U.esc(p.name)}: V/C ${U.fmt.num(p.vc, 2)}, mid-block time ${U.fmt.num(p.mid, 2)} × free flow${p.tot ? `; with junction delay ${U.fmt.num(p.tot, 2)} ×` : ''}</title></circle>` +
      (p.tot && p.tot > p.mid * 1.02 ? `<circle cx="${X(Math.min(p.vc, xMax)).toFixed(1)}" cy="${Y(Math.min(p.tot, yMax)).toFixed(1)}" r="3.5" fill="none" stroke="${col[i]}" stroke-width="1.6"/>` : '')).join('');
    return `<div><svg class="vd-chart" viewBox="0 0 ${W} ${H}" role="img" aria-label="Volume–delay curve with this road's modelled points">
        <line x1="34" y1="${H - 22}" x2="${W - 10}" y2="${H - 22}" stroke="var(--line)"/><line x1="34" y1="10" x2="34" y2="${H - 22}" stroke="var(--line)"/>
        <line x1="${X(1)}" y1="10" x2="${X(1)}" y2="${H - 22}" stroke="var(--line)" stroke-dasharray="3 3"/>
        <path d="${path}" fill="none" stroke="var(--muted)" stroke-width="1.6"/>${dots}
        <text x="30" y="14" text-anchor="end">${U.fmt.num(yMax, 1)}×</text><text x="30" y="${H - 20}" text-anchor="end">1×</text>
        <text x="34" y="${H - 6}">V/C 0</text><text x="${X(1)}" y="${H - 6}" text-anchor="middle">1.0</text><text x="${W - 10}" y="${H - 6}" text-anchor="end">${U.fmt.num(xMax, 1)}</text></svg>
      <p class="note">Travel time as a multiple of free-flow time. Line: the speed-flow curve (${Kit.gl('BPR')} α ${a}, β ${b}). Filled dots: this road${pts.length > 1 ? `'s two directions (${pts.map((p, i) => `${['blue', 'amber'][i]} ${U.esc(p.name.toLowerCase())}`).join(', ')})` : ''}; open dots add junction delay at the far end.</p></div>`;
  }
  // Crash risk for a road from NZTA CAS (DataHub.linkRisk), with both measures and their basis
  function crashRisk(sc, Lk) {
    if (!DataHub.D().crashes.length || !DataHub.linkRisk) return '';
    let R; try { R = DataHub.linkRisk(sc); } catch (e) { console.warn(e); return ''; }
    const r = R && R[Lk.id]; if (!r) return '';
    const c = DataHub.linkBand(r, 'collective'), p = DataHub.linkBand(r, 'personal');
    const band = b => b.band ? `<span class="pill" style="border-left:8px solid ${U.esc(b.colour)}">${U.esc(b.label)}</span>` : `<span class="note">${U.esc(b.label || 'no band')}</span>`;
    return `<div class="lg-title" style="margin:4px 0 0">Crash risk (indicative)</div>
      <div class="facts">${fact(band(c), `collective · ${U.fmt.num(r.dsiPerKmYr, 3)} fatal and serious crashes per km a year`)}${fact(band(p), `personal · ${U.fmt.num(r.personal, 1)} fatal and serious crashes per 100 million vehicle-km`)}</div>
      <p class="note">${U.esc(c.basis || '')}${p.basis && p.basis !== c.basis ? ' ' + U.esc(p.basis) : ''}</p>`;
  }
  const carriageway = Lk =>((Lk.lanesAB | 0) + (Lk.lanesBA | 0)) * (CLASSES[Lk.cls] || CLASSES.local).lw;
  function crossSection(Lk, dirs) {
    const C = CLASSES[Lk.cls] || CLASSES.local, lw = C.lw, items = [];
    const fp = C.rank >= 5 ? 0 : 2.4, cyc = Lk.cycle === 'separated' ? 2.2 : Lk.cycle === 'painted' ? 1.6 : 0;
    const side = C.rank >= 5 ? { w: 2.5, t: 'shoulder' } : null;
    const push = (w, kind, label) => items.push({ w, kind, label });
    if (fp) push(fp, 'fp', 'Footpath'); else if (side) push(side.w, 'sh', '');
    if (cyc && Lk.lanesAB > 0) push(cyc, 'cy', '');
    for (let i = 0; i < (Lk.lanesAB | 0); i++) push(lw, i === 0 && Lk.busLane && Lk.lanesAB >= 2 ? 'bus' : 'lane', '↑');
    if (Lk.lanesAB > 0 && Lk.lanesBA > 0) push(C.rank >= 4 && (Lk.lanesAB + Lk.lanesBA) >= 4 ? 1.5 : 0, 'med', '');
    for (let i = (Lk.lanesBA | 0) - 1; i >= 0; i--) push(lw, i === 0 && Lk.busLane && Lk.lanesBA >= 2 ? 'bus' : 'lane', '↓');
    if (cyc && Lk.lanesBA > 0) push(cyc, 'cy', '');
    if (fp) push(fp, 'fp', 'Footpath'); else if (side) push(side.w, 'sh', '');
    const tot = items.reduce((s, i) => s + i.w, 0) || 1, W = 300, H = 62, k = (W - 16) / tot;
    let x = 8;
    const fill = { fp: 'var(--line)', cy: '#2f9e44', lane: '#3a4341', bus: '#b3262d', med: '#7c8a6c', sh: '#59625f' };
    const parts = items.map(it => {
      const w = it.w * k, r = `<rect x="${x.toFixed(1)}" y="16" width="${Math.max(0, w - 0.6).toFixed(1)}" height="30" fill="${fill[it.kind]}" rx="1.5"/>` +
        (it.label === '↑' || it.label === '↓' ? `<text x="${(x + w / 2).toFixed(1)}" y="36" text-anchor="middle" fill="#fff" font-size="13">${it.label}</text>` : '');
      x += w; return r;
    }).join('');
    return `<svg class="xsec" viewBox="0 0 ${W} ${H}" role="img" aria-label="Cross-section: ${dirs[0][2]} lanes on the left, driving on the left">
      ${parts}<text x="8" y="11" font-size="10" fill="var(--muted)">${U.esc(dirs[0][2])} ↑</text><text x="${W - 8}" y="11" font-size="10" fill="var(--muted)" text-anchor="end">↓ ${U.esc(dirs[1][2])}</text>
      <text x="${W / 2}" y="58" font-size="10" fill="var(--muted)" text-anchor="middle">${U.fmt.num(tot, 1)} m kerb to kerb${fp ? ' incl. footpaths' : ''}</text></svg>`;
  }

  function nodePanel(sc, n) {
    const res = Store.res(), nr = res && res.nodes[n.id], Ls = Net.linksAt(sc, n.id);
    const names = [...new Set(Ls.map(l => l.name).filter(Boolean))];
    let appr = '';
    if (nr) {
      // Two-way give way and stop have no intersection LoS: show the worst movement's (Engine losText)
      const losHead = nr.intersectionLos === null && nr.losText ? `${Kit.gl('LoS')} ${U.esc(nr.losText)}` : nr.los ? `${Kit.gl('LoS')} ${U.esc(nr.los)}` : '';
      const basis = nr.delayBasis === 'worst movement' ? 'worst movement delay' : nr.delayBasis === 'average' || nr.type === 'signals' ? 'average delay' : 'worst approach delay';
      const dTxt = d => Engine.delayText ? Engine.delayText(d, 0) : U.fmt.int(d) + ' s';
      const dosCol = a => a.dosBand === 'over' ? 'var(--danger)' : a.dosBand === 'near' ? 'var(--warn)' : '';
      // each approach by the leg its traffic comes from, as SIDRA names it ('South: Great South Road'; nz-conventions)
      const legs = Kit.legLabels(sc, res, n.id), legOf = a => legs.get(a.key) || a.approach || compass(a.inB);
      appr = `<div class="ins-sec"><h4><span>${U.esc(S().period)} peak · ${losHead}</span><span>${res.stale ? '<span class="pill stale">out of date</span> ' : ''}${Kit.convPill(res)}</span></h4>
        <div class="facts">${fact(U.esc(Engine.delayText ? Engine.delayText(nr.delay, nr.dos) : U.fmt.num(nr.delay, 0) + ' s'), basis)}${fact(U.fmt.num(nr.dos, 2), `highest ${Kit.gl('DoS')}`)}${nr.C ? fact(U.fmt.int(nr.C) + ' s', 'cycle time') + fact((nr.split || []).map(s => U.fmt.pct(s)).join(' / '), 'green split') : ''}</div>
        <div class="tbl-wrap"><table class="t compact"><tr><th>Approach</th><th class="n">veh/h</th><th class="n">${Kit.gl('DoS')}</th><th class="n">Delay</th><th>${Kit.gl('LoS')}</th><th class="n">${Kit.gl('Q95', 'Queue')}</th></tr>
        ${nr.appr.map(a => `<tr><td title="${U.esc([a.approach, a.name].filter(Boolean).join(' · '))}">${U.esc(legOf(a))}${a.major ? ' · major' : ''}</td><td class="n">${U.fmt.int(a.v)}</td><td class="n"${dosCol(a) ? ` style="color:${dosCol(a)};font-weight:600"` : ''}>${U.fmt.num(a.dos, 2)}</td><td class="n">${U.esc(dTxt(a.delay))}</td><td>${losBadge(a.los)}</td><td class="n">${U.fmt.int(a.q95)} m</td></tr>`).join('')}
        </table></div><p class="note">Queue is an indicative 95th-percentile back of queue per lane. ${U.esc(Kit.junctionLabel(res))}</p></div>`;
    } else if (res && n.control !== 'none') appr = `<div class="ins-sec"><p class="note">No junction results here. Run the model again after changing controls.</p></div>`;
    return `${head('Junction', names.join(' / ') || 'Junction', 'node', n.id)}
      <div class="ins-sec"><h4>Control</h4>
        ${fld('Control', `<select ${bind('node', n.id, 'control')}>${opt(CONTROLS, n.control)}</select>`)}
        ${n.control === 'signals' ? `${fld('Cycle time', `<input type="number" min="40" max="180" step="5" ${bind('node', n.id, 'cycle', 'numnull')} value="${U.esc(n.cycle ?? '')}" placeholder="auto">`, 's')}
          <label class="check"><input type="checkbox" ${bind('node', n.id, 'fixedTiming', 'bool')} ${n.fixedTiming ? 'checked' : ''}> Keep equal green splits (don’t optimise)</label>` : ''}
        <label class="check"><input type="checkbox" ${bind('node', n.id, 'upgrade', 'bool')} ${n.upgrade ? 'checked' : ''}> New or upgraded junction (adds ${U.fmt.money(S().econ.node[n.control] ?? 0)} to cost)</label>
        <p class="note">${Ls.length} road${Ls.length === 1 ? '' : 's'} meet here. Give way and stop make the lower-class road yield; with equal roads the straight-through pair has priority.</p>
      </div>
      ${appr}
      ${realData(sc, { type: 'node', id: n.id })}
      ${SECTIONS}
      <div class="ins-sec"><div class="actions"><button class="btn small" data-act="zoom">Zoom to</button><button class="btn small danger" data-act="delete">Delete junction and its roads</button></div></div>`;
  }

  /* ---------- real-world data in the inspector ---------- */
  const SEV = { F: ['fatal', '#1b1b1b'], S: ['serious', '#e03131'], M: ['minor', '#f59f00'], N: ['non-injury', '#9aa5a1'] };
  function modelAt(res, mt) {
    if (!res || !mt) return null;
    const a = res.arcs[mt.link + ':ab'], b = res.arcs[mt.link + ':ba'];
    return mt.dir === 'ab' ? (a ? a.v : null) : mt.dir === 'ba' ? (b ? b.v : null) : (a ? a.v : 0) + (b ? b.v : 0);
  }
  function crashLine(cr, R) {
    const tot = cr.F + cr.S + cr.M + cr.N;
    const when = U.esc(Kit.casText());
    if (!tot) return `<p class="note">No reported crashes within ${R} m, ${when} (NZTA ${Kit.gl('CAS')}).</p>`;
    const vru = cr.list.filter(c => c[5]).length;
    return `<div class="facts">${fact(U.fmt.int(cr.F + cr.S), 'fatal and serious crashes')}${fact(U.fmt.int(tot), 'reported crashes')}</div>
      <p class="note">${['F', 'S', 'M', 'N'].filter(k => cr[k]).map(k => `<span class="sev-dot" style="background:${SEV[k][1]}" aria-hidden="true"></span> ${U.fmt.int(cr[k])} ${SEV[k][0]}`).join(' · ')}${vru ? ` · ${U.fmt.int(vru)} involving a pedestrian, cyclist or motorcyclist` : ''}. Within ${R} m, ${when}, NZTA Crash Analysis System; the latest months may be incomplete.</p>`;
  }
  function realData(sc, sel) {
    const d = DataHub.D(), parts = [];
    if (sel.type === 'link' && d.counts.length) {
      const m = DataHub.matches(sc), res = Store.res();
      const here = d.counts.filter(c => (m.get(c.id) || {}).link === sel.id);
      for (const c of here) {
        const mt = m.get(c.id), pv = DataHub.periodVol(c, S().period), mv = modelAt(res, mt);
        const geh = pv && mv != null ? Math.sqrt(2 * (mv - pv.v) ** 2 / Math.max(1, mv + pv.v)) : null, src = DataHub.SOURCES[c.src];
        parts.push(`<div style="display:grid;gap:2px"><div><span class="pill" style="color:${U.esc((src || {}).colour || '')}">${U.esc((src || {}).short || c.src)}</span> <b class="mono">${pv ? U.fmt.int(pv.v) : '–'}</b> veh/h counted${c.dir ? ' ' + U.esc(c.dir) : ''}${mv != null ? ` · model <b class="mono">${U.fmt.int(mv)}</b> · ${Kit.gl('GEH')} ${gehBadge(geh)}` : ''}${!DataHub.inUse(c) ? ' <span class="pill">not used</span>' : ''}</div>
          <div class="note">${U.esc(c.name)}${c.adt ? ` · ${U.fmt.int(c.adt)} veh/day` : ''}${pv ? ' · ' + U.esc(pv.how) : ''} · <a href="#" data-act="count" data-id="${U.esc(c.id)}">details</a></div></div>`);
      }
      if (!here.length) parts.push('<p class="note">No traffic count on this road.</p>');
    }
    if (d.crashes.length) { const R = sel.type === 'node' ? 40 : 25, cr = DataHub.crashesNear(sc, sel, R); if (cr) parts.push(crashLine(cr, R)); if (sel.type === 'link') parts.push(crashRisk(sc, sc.links[sel.id])); }
    if (sel.type === 'link') parts.push(`<div class="actions"><button class="btn small" data-act="live">Live speed now (TomTom)</button></div><div id="liveOut" class="note"></div>`);
    if (!d.counts.length && !d.crashes.length) parts.unshift(`<p class="note">Load real counts and crashes in the <a href="#" data-act="datatab">Data tab</a>.</p>`);
    return `<div class="ins-sec"><h4>Real-world data</h4>${parts.join('')}</div>`;
  }
  function countPanel(sc, c) {
    const src = DataHub.SOURCES[c.src], mt = DataHub.matches(sc).get(c.id), Lk = mt && sc.links[mt.link], res = Store.res();
    const per = p => { const pv = DataHub.periodVol(c, p), r = Store.res(sc, p), mv = modelAt(r, mt); const geh = pv && mv != null ? Math.sqrt(2 * (mv - pv.v) ** 2 / Math.max(1, mv + pv.v)) : null;
      return `<tr><td>${p} peak</td><td class="n">${pv ? U.fmt.int(pv.v) : '–'}</td><td class="n">${mv != null ? U.fmt.int(mv) : '–'}</td><td class="n">${gehBadge(geh)}</td></tr><tr><td colspan="4" class="note" style="white-space:normal;border:0;padding-top:0">${pv ? U.esc(pv.how) : ''}</td></tr>`; };
    let hist = '';
    if (c.hist && c.hist.some(Boolean)) {
      const mx = Math.max(...c.hist.filter(Boolean)), W = 280, H = 70, bw = (W - 20) / 5;
      hist = `<svg viewBox="0 0 ${W} ${H + 16}" style="width:100%;height:auto">${c.hist.map((v, i) => v ? `<rect x="${10 + i * bw + 4}" y="${H - H * v / mx}" width="${bw - 8}" height="${H * v / mx}" rx="2" fill="${src.colour}" opacity="${i === 4 ? 1 : 0.55}"/><text x="${10 + i * bw + bw / 2}" y="${H + 12}" text-anchor="middle" font-size="9.5" fill="var(--muted)">${U.fmt.num(v / 1000, 1)}k</text>` : '').join('')}</svg>
        <p class="note">${Kit.gl('AADT')} for the last five years, oldest first.</p>`;
    }
    return `<div class="ins-head"><div class="ins-kind">Traffic count · ${U.esc(src.label)}<span style="margin-left:auto"><button class="btn small icon" data-act="close" aria-label="Close">✕</button></span></div><div style="font:600 17px/1.25 var(--f-ui)">${U.esc(c.name)}</div></div>
      <div class="ins-sec"><h4>Count</h4>
        <div class="facts">${fact(c.adt ? U.fmt.int(c.adt) : '–', c.src === 'nzta' ? `${Kit.gl('AADT')} (latest year)` : 'average daily traffic')}${fact(c.hv != null ? U.fmt.num(c.hv, 1) + '%' : '–', 'heavy vehicles')}${c.peak ? fact(U.fmt.int(c.peak) + ' at ' + U.esc(c.peakTime || ''), 'counted peak hour') : ''}${c.dir ? fact(U.esc(c.dir), 'direction') : fact('Both', 'directions')}</div>
        <p class="note">${U.esc(c.note || '')}${c.ref ? ` · site ${U.esc(c.ref)}` : ''}${c.date ? ` · counted ${U.esc(c.date)}` : ''}</p>
        ${hist}
      </div>
      <div class="ins-sec"><h4>Against the model</h4>
        ${Lk ? `<p class="note">Matched to <a href="#" data-act="sellink" data-id="${U.esc(Lk.id)}">${U.esc(Lk.name || (CLASSES[Lk.cls] || CLASSES.local).label + ' road')}</a>${mt.dir !== 'both' ? ' (' + (mt.dir === 'ab' ? 'A→B' : 'B→A') + ')' : ''}, ${Math.round(mt.d)} m away.</p>` : '<p class="note warn">No model road within the match distance. Import or draw the road here.</p>'}
        ${Lk ? `<div class="tbl-wrap"><table class="t compact"><tr><th></th><th class="n">Counted</th><th class="n">Model</th><th class="n">${Kit.gl('GEH')}</th></tr>${per('AM')}${per('PM')}</table></div>` : ''}
        <label class="check"><input type="checkbox" data-act="counttoggle" data-id="${U.esc(c.id)}" ${c.off ? '' : 'checked'}> Use this count</label>
        <p class="note">${Kit.gl('GEH')} under 5 is the usual target for a calibrated model.${!res ? ' Run the model to compare.' : ''}</p>
      </div>`;
  }

  function zonePanel(sc, z) {
    const t = p => Engine.tripEnds(z, S(), p, sc);
    const am = t('AM'), pm = t('PM');
    const zc = Engine.zoneClass ? Engine.zoneClass(z) : null, roles = typeof ZONE_ROLES !== 'undefined' ? ZONE_ROLES : null;
    // With the land-use feature loaded, its own zone section (grouped land uses, live trip table, provenance) replaces
    // the plain land-use inputs and the trip-rate note here
    const lp = typeof LandusePlus !== 'undefined';
    return `${head(`Land-use zone${zc ? ` <span class="pill zc">${U.esc(ZCLASS_TEXT[zc] || zc)}</span>` : ''}`, z.name, 'zone', z.id)}
      <div class="ins-sec"><h4>Development</h4>
        ${roles ? `${fld('Zone type', `<select ${bind('zone', z.id, 'role')}>${opt({ '': `Automatic (${(ZCLASS_TEXT[zc] || '').toLowerCase()})`, ...roles }, z.role || '')}</select>`)}<p class="note">Existing land use is part of the base year; committed and proposed development are added on top and shown as development traffic.</p>` : ''}
        ${lp ? '' : Object.entries(LANDUSES).map(([k, lu]) => fld(lu.label, `<input type="text" inputmode="decimal" autocomplete="off" ${bind('zone', z.id, 'lu.' + k, 'num')} value="${U.esc((z.lu || {})[k] || '')}" placeholder="0" aria-label="${U.esc(lu.label)} (${U.esc(lu.unit)})">`, lu.unit)).join('')}
        ${fld('Zone access lines', `<select ${bind('zone', z.id, 'connectors', 'numnull')}>${opt({ '': `Default (${S().connectors})`, 1: 'to the 1 nearest junction', 2: 'to the 2 nearest', 3: 'to the 3 nearest', 4: 'to the 4 nearest' }, z.connectors ?? '')}</select>`)}
      </div>
      <div class="ins-sec"><h4>Peak-hour vehicle trips</h4>
        <div class="tbl-wrap"><table class="t compact"><tr><th></th><th class="n">Out</th><th class="n">In</th><th class="n">Total</th></tr>
          <tr><td>AM peak</td><td class="n">${U.fmt.int(am.O)}</td><td class="n">${U.fmt.int(am.D)}</td><td class="n">${U.fmt.int(am.O + am.D)}</td></tr>
          <tr><td>PM peak</td><td class="n">${U.fmt.int(pm.O)}</td><td class="n">${U.fmt.int(pm.D)}</td><td class="n">${U.fmt.int(pm.O + pm.D)}</td></tr></table></div>
        ${lp ? '' : '<p class="note">From the trip rates in Settings. These are indicative defaults; use surveyed rates for an assessment.</p>'}
      </div>
      ${SECTIONS}
      <div class="ins-sec"><div class="actions"><button class="btn small" data-act="zoom">Zoom to</button><button class="btn small danger" data-act="delete">Delete zone</button></div></div>`;
  }
  function gatewayPanel(sc, z) {
    const g = z.gw || {}, gf = Engine.growth(S(), sc);
    const inp = k => `<input type="text" inputmode="decimal" autocomplete="off" ${bind('zone', z.id, 'gw.' + k, 'num')} value="${U.esc(g[k] ?? 0)}" aria-label="${k.startsWith('AM') ? 'AM' : 'PM'} peak ${k.endsWith('in') ? 'entering' : 'leaving'}, veh/h">`;
    return `${head('Gateway', z.name, 'zone', z.id)}
      <div class="ins-sec"><h4>Traffic crossing the boundary (veh/h, ${U.esc(S().baseYear)})</h4>
        <div class="tbl-wrap"><table class="t compact"><tr><th></th><th class="n">Entering</th><th class="n">Leaving</th></tr>
          <tr><td>AM peak</td><td class="n">${inp('AMin')}</td><td class="n">${inp('AMout')}</td></tr>
          <tr><td>PM peak</td><td class="n">${inp('PMin')}</td><td class="n">${inp('PMout')}</td></tr></table></div>
        <p class="note">Grown ${U.fmt.signedPct(gf - 1)} to ${U.esc(Engine.yearOf(sc))} at ${U.esc(S().growth)}% a year. Use counts where you have them.</p>
        <p class="note">${z.gwSrc ? `<b>From real counts:</b> ${U.esc(z.gwSrc)}.` : '<b>Estimated</b> from the road class and lanes. Load counts in the Traffic data tab and press <b>Set gateway flows from counts</b> to use real numbers.'}</p>
        <p class="note">Zone trips use these flows first; whatever is left passes through to gateways on the other side of the area.</p>
      </div>
      ${SECTIONS}
      <div class="ins-sec"><div class="actions"><button class="btn small" data-act="zoom">Zoom to</button><button class="btn small danger" data-act="delete">Delete gateway</button></div></div>`;
  }
  function routePanel(sc, r) {
    const res = Store.res(), rr = res && res.routes && res.routes[r.id];
    return `${head('Bus route', r.name, 'route', r.id)}
      <div class="ins-sec"><h4>Service</h4>
        ${fld('Frequency', `<input type="number" min="0" max="120" step="1" ${bind('route', r.id, 'freq', 'num')} value="${U.esc(r.freq)}">`, 'buses/h')}
        ${fld('Colour', `<input type="color" ${bind('route', r.id, 'color')} value="${U.esc(r.color)}">`)}
        <p class="note">Each bus counts as 3 ${Kit.gl('pcu')} in general traffic, except where it runs in a bus lane.</p>
        ${rr ? `<div class="facts">${fact(U.fmt.dur(rr.t), 'end-to-end run time')}${fact(U.fmt.len(rr.len), 'route length')}${fact(U.fmt.num(rr.spd, 1) + ' km/h', 'average speed')}${fact(+r.freq > 0 ? U.fmt.int(60 / r.freq) + ' min' : '—', 'headway')}</div>` : '<p class="note">Run the model for bus run times.</p>'}
      </div>
      ${SECTIONS}
      <div class="ins-sec"><div class="actions"><button class="btn small danger" data-act="delete">Delete route</button></div></div>`;
  }

  function overviewPanel(sc) {
    const links = Object.values(sc.links), res = Store.res();
    let km = 0, pkm = 0;
    for (const Lk of links) { if (!sc.nodes[Lk.a] || !sc.nodes[Lk.b]) continue; const l = Net.len(sc, Lk) / 1000; km += l; if (Lk.status === 'proposed') pkm += l; }
    const cap = Engine.capital(sc), zones = Object.values(sc.zones), gw = zones.filter(z => z.kind === 'gateway').length;
    const ctrls = Object.values(sc.nodes).filter(n => n.control === 'signals' || n.control === 'roundabout').length;
    const modes = Registry.list('modes', m => !m.hidden), layers = Registry.list('layers');
    const BM = typeof UI !== 'undefined' && UI.BASEMAPS || {};
    return `<div class="ins-head"><div class="ins-kind">Scenario ${sc.id === Store.P.base ? '<span class="pill">Base (do-minimum)</span>' : ''}</div>
        <input class="ins-title" ${bind('scenario', sc.id, 'name')} value="${U.esc(sc.name)}" aria-label="Scenario name"></div>
      <div class="ins-sec"><h4>Show on map</h4>
        <div class="chips">${modes.map(m => `<button class="chip ${View.colorBy === m.id ? 'on' : ''}" data-colour="${U.esc(m.id)}">${U.esc(m.label)}</button>`).join('')}</div>
        <div class="chips">${[...SHOW, ...layers.map(l => [l.id, l.label])].map(([k, l]) => `<label class="check" style="font-size:12.5px"><input type="checkbox" data-show="${U.esc(k)}" ${View.show[k] ? 'checked' : ''}> ${U.esc(l)}</label>`).join('')}</div>
        ${fld('Basemap', `<select id="basemapSel">${opt(Object.fromEntries(Object.entries(BM).map(([k, b]) => [k, b.label])), S().basemap)}</select>`)}
      </div>
      <div class="ins-sec"><h4>Network</h4>
        <div class="facts">${fact(U.fmt.num(km, 1) + ' km', 'of road')}${fact(U.fmt.num(pkm, 2) + ' km', 'proposed')}${fact(U.fmt.int(zones.length - gw), 'zones')}${fact(U.fmt.int(gw), 'gateways')}${fact(U.fmt.int(ctrls), 'signals & roundabouts')}${fact(U.fmt.money(cap.total), 'indicative capital cost')}</div>
        ${!links.length ? `<p class="note">No roads yet. Use <b>Import roads</b> to bring in the streets in view, or draw new ones with the road tool (R).</p>` : ''}
        ${links.length && !zones.length ? `<p class="note">Add land-use zones (Z) and gateways (G) so the model has traffic to load.</p>` : ''}
        ${res && res.stale ? `<p class="note warn">Results are out of date. Run the model again.</p>` : ''}
      </div>
      ${SECTIONS}
      <div class="ins-sec"><h4>Notes</h4><textarea rows="3" ${bind('scenario', sc.id, 'notes')} placeholder="What this scenario tests">${U.esc(sc.notes || '')}</textarea></div>`;
  }

  /* ---------- the Map card body (map-card) ----------
     mapControls({ legend, layersOpen }) → 'Colour roads by' (numbered 1–9, results modes grouped under 'After a model
     run'), the key, and a 'Layers' disclosure with the show/hide boxes and the basemap. ui.js handles every control
     through one delegated listener on #legend (data-mc, data-show). */
  function mapControls({ legend = '', layersOpen = false } = {}) {
    const modes = Registry.list('modes', m => !m.hidden), cur = View.colorBy;
    const o = m => `<option value="${U.esc(m.id)}" ${m.id === cur ? 'selected' : ''}>${m.key ? m.key + ' · ' : ''}${U.esc(m.label)}</option>`;
    const first = modes.filter(m => !m.needsResults && (m.order || 0) < 30), after = modes.filter(m => m.needsResults), rest = modes.filter(m => !first.includes(m) && !after.includes(m));
    const hiddenCur = modes.some(m => m.id === cur) ? '' : (d => d ? `<option value="${U.esc(d.id)}" selected>${U.esc(d.label)}</option>` : '')(Registry.get('modes', cur));
    const BM = typeof UI !== 'undefined' && UI.BASEMAPS || {};
    const layers = [...SHOW, ...Registry.list('layers').map(l => [l.id, l.label])];
    return `<label class="mc-mode"><span>Colour roads by</span><select data-mc="mode" aria-label="Colour roads by (keys 1 to 9)">${hiddenCur}${first.map(o).join('')}${after.length ? `<optgroup label="After a model run">${after.map(o).join('')}</optgroup>` : ''}${rest.map(o).join('')}</select></label>
      <div class="mc-legend">${legend}</div>
      <button type="button" class="mc-layers-btn" data-mc="layers" aria-expanded="${!!layersOpen}">Layers ${layersOpen ? '▾' : '▸'}</button>
      ${layersOpen ? `<div class="mc-layers">${layers.map(([k, l]) => `<label class="check"><input type="checkbox" data-show="${U.esc(k)}" ${View.show[k] ? 'checked' : ''}> ${U.esc(l)}</label>`).join('')}
        ${fld('Basemap', `<select data-mc="basemap" aria-label="Basemap">${opt(Object.fromEntries(Object.entries(BM).map(([k, b]) => [k, b.label])), S().basemap)}</select>`)}</div>` : ''}`;
  }

  /* ---------- events ---------- */
  // Registered sections and panels see the event first; returning true stops the core handling
  function toFeature(e, fn) {
    const se = e.target.closest('[data-sec]');
    if (se && Registry.call(Registry.get('inspectorSections', se.dataset.sec), fn, e, se) === true) return true;
    const p = Registry.get('panels', shownKind);
    return !!(p && Registry.call(p, fn, e, U.$('#inspector')) === true);
  }
  function bindEvents() {
    const ins = U.$('#inspector');
    ins.addEventListener('input', e => { toFeature(e, 'onInput'); });
    ins.addEventListener('change', e => {
      if (toFeature(e, 'onChange')) return;
      const t = e.target;
      if (t.id === 'basemapSel') return UI.setBasemap(t.value);
      if (t.dataset.show) { View.show[t.dataset.show] = t.checked; if (t.dataset.show === 'live') UI.setLive(t.checked); Render.redraw(); UI.renderLegend(); return; }
      if (t.dataset.act === 'counttoggle') { const c = DataHub.D().counts.find(x => x.id === t.dataset.id); if (c) { c.off = !t.checked; Store.emit('data'); } return; }
      if (t.dataset.k) applyField(t);
    });
    ins.addEventListener('click', e => {
      if (toFeature(e, 'onClick')) return;
      const b = e.target.closest('[data-act],[data-colour]'); if (!b) return;
      if (b.dataset.colour) { UI.setColour(b.dataset.colour); return; }
      const s = View.sel, sc = Store.sc, act = b.dataset.act;
      if (b.tagName === 'A') e.preventDefault();
      if (act === 'counttoggle') return;
      if (act === 'count') return UI.select({ type: 'count', id: b.dataset.id });
      if (act === 'sellink') return UI.flyTo('link', b.dataset.id);
      if (act === 'datatab') return UI.openTab('data');
      if (act === 'live' && s) {
        const out = U.$('#liveOut'), Lk = sc.links[s.id], g = Net.geom(sc, Lk), mid = U.along(g, U.cumLen(g), U.polyLen(g) / 2).p;
        out.textContent = 'Asking TomTom…';
        DataHub.tomtomSpeed(mid).then(r => {
          out.innerHTML = r.closed ? '<b>Road closed</b> right now (TomTom).' : `Now <b class="mono">${Math.round(r.now)} km/h</b> against <b class="mono">${Math.round(r.free)} km/h</b> free-flow (${Math.round(r.now / r.free * 100)}%) · TomTom, ${new Date(r.at).toLocaleTimeString('en-NZ', { hour: 'numeric', minute: '2-digit' })}${r.conf < 0.6 ? ' · low confidence' : ''}.`;
        }).catch(err => { out.innerHTML = U.esc(err.message) + (DataHub.keys.tomtom ? '' : ' <a href="#" data-act="datatab">Open the Data tab</a>'); });
        return;
      }
      if (act === 'close') return UI.select(null);
      if (act === 'delete') return UI.deleteSelected();
      if (act === 'zoom' && s) return UI.flyTo(s.type, s.id);
      if (act === 'reverse' && s) { Store.snapshot(); Net.reverse(sc, s.id); Store.edited(); return; }
      if (act === 'elev' && s) return Dialogs.elevation(sc.links[s.id]);
      if (act === 'sla' && s) {
        const res = Store.res(), Lk = sc.links[s.id];
        const key = Lk.lanesAB > 0 ? s.id + ':ab' : s.id + ':ba';
        const a = Engine.selectLink(res, key), b2 = Lk.lanesAB > 0 && Lk.lanesBA > 0 ? Engine.selectLink(res, s.id + ':ba') : null;
        if (!a) return Kit.toast('Run the model again to enable select-link analysis.', 'warn');
        if (b2) { for (const [k, f] of Object.entries(b2.flows)) a.flows[k] = (a.flows[k] || 0) + f; a.ods.push(...b2.ods); a.tot += b2.tot; a.ods.sort((x, y) => y.q - x.q); }
        View.sla = a; UI.setColour('select'); UI.openTab('demand');
        Kit.toast(`${U.fmt.int(a.tot / Engine.pcuF(S()))} veh/h use this road. Their full routes are shown.`);
      }
    });
  }

  /* Write one bound field (Kit.bind) back to the model with undo. Kinds: link, node, zone, route,
     scenario, project. Types: str, num, int, numnull, bool. data-emit="kind" emits that kind instead
     of marking results out of date (use it for fields that do not change the model). */
  // 'lanes on Waihoehoe Road': names the undo step (results-trust (g))
  const FIELD_NAME = { lanesAB: 'lanes', lanesBA: 'lanes', speed: 'speed limit', cls: 'road class', status: 'status', capPerLane: 'capacity', busLane: 'bus lane', cycle: 'cycling facility',
    control: 'junction control', 'node.cycle': 'cycle time', fixedTiming: 'signal timing', upgrade: 'upgrade flag', connectors: 'zone access lines', freq: 'bus frequency', name: 'name', notes: 'notes', role: 'zone type' };
  function undoLabel(kind, o, k, id) {
    const f = FIELD_NAME[kind + '.' + k] || FIELD_NAME[k] || (k.startsWith('lu.') ? 'land use' : k.startsWith('gw.') ? 'gateway flows' : k.split('.').pop());
    const who = kind === 'node' ? Kit.junctionName(id) : kind === 'scenario' ? 'the scenario' : (o && o.name) || (kind === 'link' ? 'an unnamed road' : 'it');
    return `${f} on ${who}`;
  }
  // '12,000' → 12000, '5 000 m²' → 5000, blank → null, anything else NaN (the onboarding lane's parser when it is loaded)
  function parseNum(v) {
    if (typeof Onboarding !== 'undefined' && typeof Onboarding.parseNum === 'function') return Onboarding.parseNum(v);
    if (typeof v === 'number') return isFinite(v) ? v : NaN;
    const s = String(v ?? '').trim().toLowerCase().replace(/s*(m2|m²|sq.?s*m|sqm).?$/, '').replace(/[s,]/g, '');
    if (!s) return null;
    return /^[-+]?(d+.?d*|.d+)$/.test(s) ? +s : NaN;
  }
  function invalid(t, kind, o, k, id) {
    const lu = k.startsWith('lu.') && typeof LANDUSES !== 'undefined' && LANDUSES[k.slice(3)];
    const what = lu ? `${lu.label.toLowerCase()} (${lu.unit})` : undoLabel(kind, o, k, id).replace(/ on .*$/, '');
    if (t.setAttribute) { t.setAttribute('aria-invalid', 'true'); t.title = 'Enter a number, for example 12000 or 12,000'; }
    Kit.toast(`Enter a number for ${what}, for example 12000 or 12,000. Nothing was changed.`, 'warn', { persist: false });
  }
  function applyField(t) {
    const sc = Store.sc, kind = t.dataset.obj, id = t.dataset.id, k = t.dataset.k, ty = t.dataset.t;
    const o = kind === 'link' ? sc.links[id] : kind === 'node' ? sc.nodes[id] : kind === 'zone' ? sc.zones[id] : kind === 'route' ? sc.routes[id] : kind === 'scenario' ? sc : kind === 'project' ? Store.P : null;
    if (!o) return;
    let v = t.type === 'checkbox' ? t.checked : t.value;
    if (ty === 'num' || ty === 'int' || ty === 'numnull') {
      // a number box the browser could not read (badInput) or text that is not a number: say so, save nothing
      const x = t.validity && t.validity.badInput ? NaN : parseNum(v);
      if (Number.isNaN(x)) { invalid(t, kind, o, k, id); return false; }
      if (t.removeAttribute) t.removeAttribute('aria-invalid');
      v = x == null ? (ty === 'numnull' ? null : 0) : Math.max(0, ty === 'int' ? Math.round(x) : x);
    } else if (ty === 'str' && typeof v === 'string') v = v.trim();
    if (Kit.getPath(o, k) === v) return;
    if (kind !== 'project') Kit.snap(undoLabel(kind, o, k, id));
    if (kind === 'zone' && k.startsWith('gw.')) o.gwSrc = null;
    const prevCls = o.cls;
    Kit.setPath(o, k, v);
    if (kind === 'link') {
      if (k === 'cls' && CLASSES[v] && (o.speed === CLASSES[prevCls].speed)) o.speed = CLASSES[v].speed;
      if ((o.lanesAB | 0) + (o.lanesBA | 0) === 0) { o[k] = 1; Kit.toast('A road needs at least one lane.', 'warn'); }
      if (Math.max(o.lanesAB, o.lanesBA) < 2) o.busLane = false;
    }
    if (kind === 'node' && k === 'control') o.upgrade = v !== 'none' && v !== 'priority';
    const meta = k === 'name' || k === 'notes' || k === 'color' || kind === 'project';
    if (t.dataset.emit) Store.emit(t.dataset.emit);
    else if (meta) Store.emit('meta'); else Store.edited(kind === 'zone' ? 'demand' : 'net');
  }

  return { html, render, kindOf, bindEvents, applyField, parseNum, modelAt, crashLine, mapControls, vdChart, crashRisk, speedSource, SECTIONS, get shown() { return shownKind; } };
})();
