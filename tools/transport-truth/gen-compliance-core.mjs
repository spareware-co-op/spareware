// Part 'core': the Compliance module's checks that read the project (authorities, site, E27, ITA scope, access, state highways, FDS, operators,
// working days and clocks, disclaimers and CSV stamping, the checklist, report sections and exports)
// -> expected/compliance-core-*.json
import { part, write, record, loadCompliance } from './gen-compliance-lib.mjs';

part('core', async () => {
  const c = loadCompliance();
  const C = c.Compliance;
  const reg = id => c.Registry.get('report', id);
  const exp = id => c.Registry.get('exports', id);
  const cases = [];
  const add = async (id, call, fixture, sets, f, arg) => cases.push({ id, call, ...(arg !== undefined ? { arg } : {}), ...(await record(c, fixture, sets, f)) });
  const NOW = '2026-10-21';

  // projects: the fixtures' node ids for placing things
  const ids = {};
  for (const name of ['corridor', 'grid']) {
    const P = await (await import('./gen-compliance-lib.mjs')).withResults(c, name);
    ids[name] = { nodes: Object.keys(P.scenarios[0].nodes), links: Object.keys(P.scenarios[0].links), scs: P.scenarios.map(s => s.id), zones: Object.values(P.scenarios[0].zones).map(z => [z.id, z.kind, z.lat, z.lng]),
      first: Object.values(P.scenarios[0].nodes)[0], links0: Object.values(P.scenarios[0].links).slice(0, 3).map(l => l.id) };
  }
  const lookup = (zone, extra = {}) => ({ lat: -37.0, lng: 174.9, src: 'test', at: '2026-10-01', errors: [], kept: [], sources: [{ source: 'AUP base zones (Auckland Council)', url: 'u', date: '2026-10-01', licence: 'CC', basis: 'measured' }],
    zone: zone ? { code: 1, name: zone, group: '' } : null, zones: zone ? [{ code: 1, name: zone, group: '' }] : [], precincts: [], cfoc: false, pc120: null, fds: null, board: 'Papakura', designations: [], ...extra });
  const nodeLL = (name, k = 0) => { const n = ids[name].first; return [n.lat + k * 0.0004, n.lng + k * 0.0004]; };
  const overlays = name => {
    const a = nodeLL(name, 0), b = nodeLL(name, 1), z = nodeLL(name, 3);
    return { at: '2026-10-05', bounds: { s: a[0] - 0.01, w: a[1] - 0.01, n: a[0] + 0.01, e: a[1] + 0.01 }, arterials: [[a, b]], var: [{ type: 'General', pts: [a, b] }, { type: 'Adjacent to Level Crossings', pts: [z, [z[0] + 0.0002, z[1]]] }],
      frontage: [{ type: 'Key Retail Frontage', pts: [a, b] }], lx: [[z, [z[0] + 0.0001, z[1] + 0.0001], [z[0] + 0.0001, z[1] - 0.0001]]], errors: [],
      sources: ['AUP arterial roads (planning maps)', 'AUP Vehicle Access Restriction Control', 'AUP Building Frontage Control (Key Retail / General Commercial)', 'AUP Level Crossings with Sightlines Control'].map(s => ({ source: s, date: '2026-10-05', licence: 'CC', url: '', basis: 'measured' })) };
  };
  const accesses = name => {
    const n = ids[name].nodes;
    return [
      { id: 'a1', name: 'Site access', node: n[1], frontage: 60, crossings: 3, width: 3.2, spaces: 5, formed: 2.8, length: 70, q95: 12, stacking: 8, sepAdj: 1, sepSame: 3, combinedWidth: 7, peds: 40 },
      { id: 'a2', name: 'Rear lane', lat: nodeLL(name, 2)[0], lng: nodeLL(name, 2)[1], frontage: 20, crossings: 1, width: 6, spaces: 12, formed: 6, zoneGroup: 'business', hv: true, passingBays: true, controlsChecked: true, existing: true, changed: true },
      { id: 'a3', name: 'City', lat: nodeLL(name, 1)[0], lng: nodeLL(name, 1)[1], krf: true, exception: 'only', var: 'General', arterial: 'yes', intersectionDist: 4, lxDist: 12, frontage: 30, crossings: 2 },
      { id: 'a4', name: 'Rural', node: n[2], zoneGroup: 'rural', length: 150, formed: 4, width: 7, spaces: 3, arterial: 'no', krf: false, exception: 'relocate' },
      { id: 'a5', name: '', lat: null, lng: null },
    ];
  };

  for (const name of ['corridor', 'grid']) {
    const sc = ids[name].scs[1];
    const base = { active: sc };
    const zoneSets = z => ({ ...base, 'compliance.site.lookup': lookup(z) });
    const statuses = ['', 'P', 'C', 'RD', 'D', 'NC', 'U'];
    // authorities, site and sh marks
    for (const [pw, d, region] of [['', '', 'auckland'], ['rc-landuse', '2026-04-01', 'auckland'], ['rc-landuse', '2026-11-01', 'auckland'], ['nor', '2026-11-01', 'auckland'], ['nor', '2026-06-01', 'other']]) {
      for (const cl of ['', '2026-09-01']) {
        await add(`authorities ${name} ${pw} ${d} ${region} ${cl}`, 'authorities', name, { ...base, 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': d, 'compliance.ctx.region': region, 'compliance.ctx.cl82Date': cl, 'compliance.ctx.transferDate': '2026-12-01', 'compliance.site.lookup': lookup('Business - Town Centre Zone') }, P => C.authorities(P, NOW));
      }
    }
    await add(`siteInfo ${name}`, 'siteInfo', name, { ...base, 'compliance.site.lookup': lookup('Residential - Mixed Housing Urban Zone', { precincts: [{ code: 1, name: 'Drury East Precinct', sub: false }], pc120: { zone: 'Business - City Centre Zone', zones: [], precinct: null, address: '1 A St\r2 B', note: '' }, fds: { name: 'Drury', type: 't', timing: '2030+' }, cfoc: true }), 'settings.compliance.precincts': 'I999 | Test.Precinct (x) | I999.6\nbad\n | no | x' }, P => C.siteInfo(P));
    await add(`siteInfo overrides ${name}`, 'siteInfo', name, { ...base, 'compliance.site.zoneOverride': 'Open Space - Sport', 'compliance.site.precinctOverride': 'Drury Centre', 'compliance.site.pc120Override': 'X', 'compliance.site.fdsOverride': { name: 'N', timing: '', source: '', page: '4' } }, P => C.siteInfo(P));
    await add(`sitePoint ${name}`, 'sitePoint', name, base, P => C.sitePoint(P));
    await add(`sitePoint set ${name}`, 'sitePoint', name, { 'compliance.site.lat': -37.1, 'compliance.site.lng': 174.2 }, P => C.sitePoint(P));
    for (const [k, v] of [['', null], ['x', true], ['y', false]]) await add(`shOf ${name} ${k}`, 'shOf', name, { ...base, ...(k ? { [`compliance.net.shUser.${ids[name].links[0]}`]: v } : {}), [`compliance.net.sh.${ids[name].links[1]}`]: '1' }, P => ids[name].links.slice(0, 4).map(id => C.shOf(P, P.scenarios[0].links[id])));
    await add(`linkAuthority ${name}`, 'linkAuthority', name, { ...base, 'compliance.ctx.lodgement': '2026-11-01', 'compliance.site.lookup': lookup('x') , 'compliance.overlays': overlays(name) }, P => ids[name].links.map(id => C.linkAuthority(P, P.scenarios[0].links[id], P.scenarios[1])));
    // E27 across zones, statuses and overrides
    const zones = ['', 'Business - City Centre Zone', 'Residential - Terrace Housing and Apartment Buildings Zone', 'Open Space - Sport and Active Recreation Zone', 'Business - General Business Zone'];
    for (const z of zones) for (const s of statuses) {
      await add(`e27 ${name} ${z || 'none'} ${s || 'unset'}`, 'e27', name, { ...(z ? zoneSets(z) : base), 'compliance.ctx.activityStatus': s }, P => C.e27(P));
    }
    for (const [lab, sets] of [
      ['overrides tier1', { 'compliance.q.dwellings': 50, 'compliance.q.office': '', 'compliance.q.bogus': 3 }],
      ['overrides tier2', { 'compliance.q.dwellings': 150, 'compliance.q.retail': 2000, 'compliance.q.secondary': 400 }],
      ['unlisted', { 'compliance.site.unlisted': true, 'compliance.ctx.activityStatus': 'RD' }],
      ['unlisted ta', { 'compliance.site.unlisted': true, 'compliance.ctx.activityStatus': 'C', 'compliance.site.zoneTA': true }],
      ['precinct', { 'compliance.site.precinctOverride': 'Drury East' }],
      ['subdivision', { 'compliance.ctx.pathway': 'rc-subdivision', 'compliance.q.subdivision': 120 }],
      ['dt school', { 'compliance.site.driveThrough': true, 'compliance.site.school': 'secondary', 'compliance.q.retailDT': 400 }],
      ['pc120', { 'compliance.site.lookup': lookup('Business - City Centre Zone', { pc120: { zone: 'Residential - Mixed Housing Urban Zone', zones: [], precinct: null, address: '', note: '' } }) }],
      ['pc120 same', { 'compliance.site.lookup': lookup('Residential - Mixed Housing Urban Zone', { pc120: { zone: 'Residential - Mixed Housing Urban Zone', zones: [], precinct: null, address: '', note: '' } }) }],
      ['vicinity', { 'settings.compliance.vicinity': 600, 'compliance.q.dwellings': 50 }],
    ]) for (const s of ['D', 'NC', '']) await add(`e27 ${name} ${lab} ${s}`, 'e27', name, { ...base, 'compliance.ctx.activityStatus': s, ...sets }, P => C.e27(P));
    // ITA scope with every pathway
    for (const pw of ['', 'rc-landuse', 'pc-private', 'nor', 'ftaa-consent', 'precinct']) for (const [cx, zc] of [['urban', true], ['rural', false], ['periurban', false]]) {
      await add(`itaScope ${name} ${pw} ${cx} ${zc}`, 'itaScope', name, { ...base, 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': '2026-11-05', 'compliance.ctx.context': cx, 'compliance.ctx.zoningConsistent': zc, 'compliance.ctx.activityStatus': 'D', 'compliance.q.dwellings': 130, 'compliance.site.lookup': lookup('x') }, P => C.itaScope(P));
    }
    await add(`itaScope outside ${name}`, 'itaScope', name, { ...base, 'compliance.ctx.region': 'other', 'compliance.q.office': 5000, 'compliance.proportion.text': 'Custom text', 'compliance.proportion.confirmed': true }, P => C.itaScope(P));
    await add(`itaScope other ${name}`, 'itaScope', name, { ...base, 'compliance.ctx.region': 'other', 'compliance.q.dwellings': 20, 'compliance.q.retailDT': 10 }, P => C.itaScope(P));
    await add(`scopingMemo ${name}`, 'scopingMemo', name, { ...base, 'compliance.scoping.area': { value: 'Custom', locked: true, by: 'A', date: '2026-10-01' }, 'data.loaded.nzta': { at: '2026-09-01T00:00:00Z' }, register: [{ id: 'r1', name: 'x' }] }, P => C.scopingMemo(P));
    await add(`table1 ${name}`, 'table1', name, base, P => [{ q: { dwellings: 35 }, unlistedPeak: 0 }, { q: { dwellings: 150, office: 100 }, unlistedPeak: 30 }, { q: { retailDT: 5 }, unlistedPeak: 0 }, { q: { dwellings: 0 }, unlistedPeak: 0 }, { q: { dwellings: 10, office: 200, retail: 100, primary: 40 }, unlistedPeak: 0 }].map(Q => C.table1({ ...Q, q: { dwellings: 0, ird: 0, visitor: 0, subdivision: 0, primary: 0, secondary: 0, tertiary: 0, office: 0, retail: 0, retailDT: 0, warehouse: 0, industrial: 0, ...Q.q } })));
    await add(`depthOf ${name}`, 'depthOf', name, base, () => ['None', 'Lite', 'Full'].flatMap(r => [true, false].flatMap(k => ['urban', 'periurban', 'rural', 'x'].map(x => C.depthOf(r, k, x)))));
    await add(`sisd ${name}`, 'sisd', name, base, () => [[50, 0], [80, 0], ['x', 0], [100, 87], [200, 0], [10, 0], [60, 95.5]].map(([s, m]) => C.sisd(s, m)));
    await add(`accessType ${name}`, 'accessType', name, base, () => [{ daily: 300, peak: 5 }, { daily: 10, peak: 30 }, { daily: 20, hcvWeek: 0, sisdOk: true }, { daily: 20, hcvWeek: 0, sisdOk: false }, { daily: 20, hcvWeek: 0 }, { daily: 80, hcvWeek: 0 }, { daily: 20, hcvWeek: 3, sisdOk: true }].map(x => C.accessType(x)));
    await add(`pavement ${name}`, 'pavement', name, base, () => [[0, 0], [0, 5], [100, 110], [100, 130], [100, 60], [50.5, 61.2]].map(([a, b]) => C.pavement(a, b)));
    // access
    for (const k of [0, 1, 2, 3]) for (const ov of [false, true]) {
      await add(`accessCheck ${name} ${k} ${ov}`, 'accessCheck', name, { ...base, 'compliance.access': accesses(name), ...(ov ? { 'compliance.overlays': overlays(name) } : {}), 'compliance.site.lookup': lookup(k % 2 ? 'Residential - Mixed Housing Urban Zone' : 'Business - City Centre Zone') }, P => C.accessCheck(P, C.state(P).access[k]), k);
    }
    await add(`accessCheck blank ${name}`, 'accessCheck', name, { ...base, 'compliance.access': accesses(name) }, P => C.accessCheck(P, C.state(P).access[4]), 4);
    await add(`accessGeo ${name}`, 'accessGeo', name, { ...base, 'compliance.access': accesses(name), 'compliance.overlays': overlays(name), 'compliance.net.aupArterial': { [ids[name].links[0]]: true } }, P => C.state(P).access.map(a => C.accessGeo(P, P.scenarios[1], a)));
    // state highways
    for (const mark of [false, true]) {
      const shSets = mark ? Object.fromEntries(ids[name].links0.map(id => [`compliance.net.shUser.${id}`, true])) : {};
      await add(`shCheck ${name} ${mark}`, 'shCheck', name, { ...base, ...shSets, 'compliance.access': accesses(name).map((a, i) => ({ ...a, daily: 30 + i * 100, peak: 4, hcvPct: 25, hcvWeek: i, speed85: i ? 0 : 63, sightAvail: i === 1 ? 300 : '' })), 'compliance.ctx.devHvPct': mark ? 12 : null, 'compliance.site.lookup': lookup('x', { designations: [{ name: 'SH1', by: 'NZ Transport Agency', schedule: 'x' }] }), 'compliance.ctx.pathway': 'ftaa-consent', 'compliance.net.loaded.sh': { at: '2026-10-01', n: 2 } }, P => C.shCheck(P));
      await add(`shInvolved ${name} ${mark}`, 'shInvolved', name, { ...base, ...shSets }, P => C.shInvolved(P));
    }
    await add(`motorway ${name}`, 'motorwayConnectors', name, { ...base }, P => { const sc = P.scenarios[1]; for (const l of Object.values(sc.links)) l.cls = 'motorway'; return C.motorwayConnectors(sc, P); });
    // fds
    for (const [lab, fds] of [['none', null], ['out', { name: 'Area', type: 't', timing: '2040+' }], ['in', { name: 'Area', type: 't', timing: '2020+' }], ['live', { name: 'Area', type: 't', timing: 'Live zoned' }], ['removal', { name: 'Area', type: 't', timing: 'Removal 2050' }], ['red', { name: 'Area', type: 't', timing: 'Red flag 2030+' }], ['text', { name: 'Area', type: 't', timing: '' }]]) for (const pw of ['', 'rc-landuse', 'nor', 'ftaa-consent', 'pc-private']) {
      await add(`fdsCheck ${name} ${lab} ${pw}`, 'fdsCheck', name, { ...base, 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': '2026-11-10', 'compliance.site.lookup': lookup('x', { fds }), 'compliance.fds.infra': [{ name: '', reg: 'r1' }, { name: 'Own', reg: '' }], 'compliance.fds.rsp': lab === 'red' ? { name: 'RSP', date: '2026-12-01' } : { name: '', date: '' }, register: [{ id: 'r1', name: 'Reg road', type: 'network', funding: 'funded', links: [] }], 'compliance.ctx.occupation': lab === 'in' ? 2035 : '' }, P => C.fdsCheck(P));
    }
    // operators
    for (const [dvc, dd] of [[0.05, 10], [0.0, 0], [0.5, 100]]) await add(`operators ${name} ${dvc}`, 'operators', name, { ...base, 'settings.compliance.dvc': dvc, 'settings.compliance.ddelay': dd, [`compliance.net.shUser.${ids[name].links[2]}`]: true, 'compliance.ctx.lodgement': '2026-11-01', 'compliance.access': accesses(name), 'compliance.overlays': overlays(name) }, P => C.operators(P));
    await add(`operators same ${name}`, 'operators', name, { active: ids[name].scs[0] }, P => C.operators(P));
    await add(`operators AM ${name}`, 'operators', name, base, P => C.operators(P, { period: 'AM' }));
  }

  // working days, holidays and clocks
  await add('workingDays', 'workingDays', 'corridor', {}, () => {
    const out = [];
    for (const kind of ['rma', 'ftaa']) for (const d of ['2026-12-18', '2026-12-24', '2027-01-01', '2027-01-04', '2026-06-30', '2026-04-03', '2026-02-06', '2025-01-01', '2028-01-01', '2052-12-20', '2053-12-24']) for (const n of [1, 5, 10, 15, 20, -5, -15, 0]) out.push([kind, d, n, C.addWorkingDays(d, n, kind)]);
    out.push(['x', 'rubbish', 5, C.addWorkingDays('rubbish', 5)]);
    out.push(['bw', C.workingDaysBetween('2026-10-01', '2026-11-30'), C.workingDaysBetween('2026-11-30', '2026-10-01'), C.workingDaysBetween('2026-10-01', '2026-10-01'), C.workingDaysBetween('x', '2026-10-01')]);
    out.push(['iw', ['2026-10-21', '2026-10-24', '2026-12-25', '2027-06-25', 'x'].map(d => [C.isWorkingDay(d), C.isWorkingDay(d, 'ftaa')])]);
    return out;
  });
  await add('holidays', 'holidays', 'corridor', {}, () => Object.fromEntries([2021, 2022, 2026, 2027, 2028, 2052, 2053, 2100, 1999].map(y => [y, [...C.holidays(y)]])));
  const evs = [{ kind: 'lodged', date: '2026-11-02' }, { kind: 's92', date: '2026-12-10' }, { kind: 'cl21', date: '2026-09-01' }, { kind: 'cl23', date: '2027-01-12' }, { kind: 'ftaaInvite', date: '2026-12-01' }, { kind: 'ftaaComments', date: '2026-10-20' },
    { kind: 'ftaaS67', date: '2026-10-21' }, { kind: 'ftaaS67', date: '2026-10-21', set: '2026-12-31' }, { kind: 'ftaaS67', date: '2026-10-21', set: '2026-10-28' }, { kind: 'hearing', date: '2027-02-10' }, { kind: 'works', date: '2026-12-22' }, { kind: 'bogus', date: '2026-10-01' }, { kind: 's92', date: 'x' }, { kind: 's92', date: '2053-06-01' }];
  await add('clockFor', 'clockFor', 'corridor', {}, () => evs.map(e => C.clockFor(e)));
  await add('clocks', 'clocks', 'corridor', { 'compliance.rfi.events': evs.map((e, i) => ({ id: 'e' + i, ...e })), 'compliance.rfi.hearing': '2027-03-01', 'compliance.rfi.worksDate': '2027-01-15', 'compliance.ctx.pathway': 'rc-landuse', 'compliance.ctx.lodgement': '2026-11-02', 'compliance.ctx.lodgementActual': true }, P => C.clocks(P));
  await add('clocks empty', 'clocks', 'corridor', {}, P => C.clocks(P));
  for (const pw of ['rc-landuse', 'rc-subdivision', 'pc-private']) for (const d of ['2026-10-01', '2026-11-20']) await add(`rfiTemplate ${pw} ${d}`, 'rfiTemplate', 'corridor', { 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': d }, P => C.rfiTemplate(P));

  // disclaimers, CSV stamping, sign state
  await add('disclaimers', 'disclaimer', 'corridor', { 'compliance.notes.D1': 'Extra note.' }, P => Object.keys(C.DATA.DISCLAIMERS).concat(['D99']).map(id => [C.disclaimer(id, P), C.disclaimer(id, undefined), C.disclaimer(id, P, { tool: 'SIDRA 11' }), C.disclaimerHtml === undefined ? null : (C.DATA.DISCLAIMERS[id] ? C.disclaimerHtml(id, P, { tool: 'X' }) : null)]));
  for (const signed of [false, true]) await add(`csv ${signed}`, 'csv', 'corridor', {}, P => {
    if (signed) C.state(P).expert.signoffs = [{ role: 'prepared', name: 'A', quals: 'B', date: '2026-10-01', fp: C.fingerprint(P) }, { role: 'reviewed', name: 'C', quals: 'D', date: '2026-10-01', fp: 'old' }];
    return { footer: C.csvFooter(P), handover: C.csvFooter(P, { handover: 'SIDRA' }), stamp: C.stampCSV('a,b\r\n1,2\r\n\r\n', P), stamp2: C.stampCSV('', P, { handover: 'X' }), twice: C.stampCSV(C.stampCSV('a,b', P), P), sign: C.signState(P), fp: C.fingerprint(P) };
  });
  await add('fingerprint', 'fingerprint', 'grid', {}, P => { const a = C.fingerprint(P); P.settings.growth = 9; const b = C.fingerprint(P); P.scenarios[0].role = 'x'; return [a, b, C.fingerprint(P)]; });

  // the checklist, report sections and exports across pathways
  const pws = [['', ''], ['rc-landuse', '2026-11-05'], ['rc-subdivision', '2026-09-01'], ['precinct', '2026-11-05'], ['pc-private', '2026-11-05'], ['nor', '2026-11-05'], ['ftaa-consent', '2026-11-05'], ['ftaa-nor', '2026-09-01'], ['nltf', ''], ['speed-limit', ''], ['dc-iff', ''], ['tou', '']];
  for (const name of ['corridor', 'grid']) for (const [pw, d] of pws) {
    const sets = { active: ids[name].scs[1], 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': d, 'compliance.ctx.activityStatus': 'D', 'compliance.ctx.vest': true, 'compliance.site.lookup': lookup('Residential - Mixed Housing Urban Zone', { precincts: pw === 'precinct' ? [{ code: 1, name: 'Drury East', sub: false }] : [] }), 'compliance.access': accesses(name).slice(0, 3), 'compliance.overlays': overlays(name),
      'compliance.checks.X-01': { state: 'warn', note: 'n', by: 'me', date: '2026-10-01' }, 'compliance.checks.X-02': { state: 'bogus', note: 5 }, 'compliance.rfi.hearing': '2027-03-01', register: [{ id: 'r1', name: 'x', type: 'consent', status: 'granted', likely: 'likely' }], assessment: { periods: ['AM', 'PM'], opening: 2031 } };
    await add(`checklist ${name} ${pw || 'none'}`, 'checklist', name, sets, P => C.checklist(P).map(r => ({ ...r })));
    await add(`checklistCSV ${name} ${pw || 'none'}`, 'checklistCSV', name, sets, P => C.checklistCSV(P));
    for (const r of c.Registry.list('report').filter(r => /^(report-regulatory|compliance-)/.test(r.id) && !/^compliance-extra/.test(r.id))) {
      await add(`report ${r.id} ${name} ${pw || 'none'}`, 'report', name, sets, P => r.render(P), r.id);
    }
  }
  const rsets = { active: ids.grid.scs[1], 'compliance.ctx.pathway': 'rc-landuse', 'compliance.ctx.lodgement': '2026-11-05', register: [{ id: 'r1', name: 'Reg', type: 'consent', status: 'granted', likely: 'likely', source: 's', date: '2026-01-01' }, { id: 'r2', name: 'Net', type: 'network', funding: 'funded', verify: true }],
    'compliance.rfi.hearing': '2027-03-01', 'compliance.scoping.area': { value: 'X', locked: true, by: 'A', date: '2026-10-01' } };
  for (const id of ['compliance-checklist', 'compliance-scoping', 'compliance-register', 'compliance-timeline']) await add(`export ${id}`, 'export', 'grid', rsets, P => exp(id).build(P), id);
  await add('exportEnabled', 'exportEnabled', 'grid', rsets, P => ['compliance-checklist', 'compliance-scoping', 'compliance-register', 'compliance-timeline', 'compliance-jws'].map(id => (exp(id).enabled || (() => true))()));
  await add('exportEnabled none', 'exportEnabled', 'corridor', {}, P => ['compliance-checklist', 'compliance-scoping', 'compliance-register', 'compliance-timeline', 'compliance-jws'].map(id => (exp(id).enabled || (() => true))()));
  write('expected/compliance-core.json', { cases });
});
