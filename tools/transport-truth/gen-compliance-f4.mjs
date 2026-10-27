// Part 'f4': the pre-lodgement linter (compliance.js section 11 lint), expert disclosure, sign-off, data sources, the assumption diff
// and its JWS table, the model status badge, disclaimer checks and standards supersession (sections 12 and 13)
// -> expected/compliance-f4-lint.json, compliance-f4-expert.json, compliance-f4-diff.json, compliance-f4-jsonerr.json
import { part, write, record, variant, setPaths, plain, loadCompliance, attempt } from './gen-compliance-lib.mjs';
import { jsonCorpus } from './compliance-f4-jsoncorpus.mjs';

part('f4', async () => {
  const c = loadCompliance();
  const C = c.Compliance;
  const base = { corridor: await variant(c, 'corridor'), grid: await variant(c, 'grid') };
  const nodesOf = (n, i = 0) => Object.keys(base[n].scenarios[i].nodes);
  const linksOf = (n, i = 0) => Object.keys(base[n].scenarios[i].links);
  const zonesOf = (n, i = 0) => Object.keys(base[n].scenarios[i].zones);
  const nScen = n => base[n].scenarios.length;
  const lint = [], expert = [], diff = [];
  const add = async (list, id, call, fixture, sets, f, extra = {}) => list.push({ id, call, ...extra, ...(await record(c, fixture, sets, f)) });
  const counts = (...o) => o.map((x, i) => ({ id: 'c' + i, adt: 1000, am: 90, pm: 100, peak: 95, src: 'at', date: '2024-03-12', ...x }));
  const L = (id, fixture, sets) => add(lint, id, 'lint', fixture, sets, P => C.lint(P));

  /* ---------- lint ---------- */
  for (const n of ['corridor', 'grid']) await L(`lint ${n} as saved`, n, {});
  for (const n of ['corridor', 'grid']) {
    // counts without a date or source
    await L(`lint ${n} one count no date no source`, n, { 'data.counts': counts({ date: '', src: '' }) });
    await L(`lint ${n} two undated with source`, n, { 'data.counts': counts({ date: undefined }, { date: null }) });
    await L(`lint ${n} undated nzta`, n, { 'data.counts': counts({ date: '', src: 'nzta' }) });
    await L(`lint ${n} undated nzta and undated none`, n, { 'data.counts': counts({ date: '', src: 'nzta' }, { date: '', src: undefined }) });
    await L(`lint ${n} dated no source`, n, { 'data.counts': counts({ src: '' }) });
    await L(`lint ${n} three undated`, n, { 'data.counts': counts({ date: '' }, { date: '' }, { date: '', src: '' }) });
    await L(`lint ${n} counts fine`, n, { 'data.counts': counts({}, {}) });
    await L(`lint ${n} counts fine with hv`, n, { 'data.counts': counts({ hv: 4 }) });
    await L(`lint ${n} counts with hv zero`, n, { 'data.counts': counts({ hv: 0 }) });
    await L(`lint ${n} counts with hv text`, n, { 'data.counts': counts({ hv: '5' }) });
    await L(`lint ${n} counts with hv not a number`, n, { 'data.counts': counts({ hv: 'x' }) });
    await L(`lint ${n} counts not a list`, n, { 'data.counts': null });
    await L(`lint ${n} data without counts`, n, { data: { loaded: {} } });
    // the heavy vehicle share
    for (const hv of [6, 8, '6', 6.0, '6.0', 0, null, 6.5]) await L(`lint ${n} hvPct ${JSON.stringify(hv)}`, n, { 'settings.hvPct': hv });
  }
  // the observed peak
  const modelledNone = n => Object.fromEntries(Array.from({ length: nScen(n) }, (_, i) => [`scenarios.${i}.res`, {}]));
  const modelledAM = n => Object.fromEntries(Array.from({ length: nScen(n) }, (_, i) => [`scenarios.${i}.res.PM`, null]));
  const modelledPM = n => Object.fromEntries(Array.from({ length: nScen(n) }, (_, i) => [`scenarios.${i}.res.AM`, null]));
  const hours = (...h) => counts(...h.map(x => ({ peakTime: x })));
  for (const n of ['corridor', 'grid']) {
    const peaks = [['07:30', '08:10', '07:55'], ['12:30', '12:00', '11:45'], ['16:00', '16:30', '17:15'], ['06:00', '06:59', '06:30'], ['09:59', '09:00', '09:15'], ['10:00', '10:01', '10:30'], ['15:00', '15:30', '15:45'],
      ['18:59', '18:00', '18:20'], ['19:00', '19:05', '19:10'], ['05:59', '05:00', '05:10'], ['07:30', '08:10'], ['07:30'], ['07:00', '08:00', '08:30', '07:30'], ['12:00', '12:30', '07:00', '07:10', '07:20'],
      ['8:00', '8:30', '9:00'], ['0730', '0800', '0815'], ['07:30', '', '08:10', null, '07:50'], [7, 8, 9], ['23:15', '23:30', '23:45'], ['00:15', '00:30', '00:45']];
    for (const [i, pk] of peaks.entries()) {
      await L(`lint ${n} peak ${i} both periods`, n, { 'data.counts': hours(...pk) });
      await L(`lint ${n} peak ${i} AM only`, n, { 'data.counts': hours(...pk), ...modelledAM(n) });
      await L(`lint ${n} peak ${i} PM only`, n, { 'data.counts': hours(...pk), ...modelledPM(n) });
      await L(`lint ${n} peak ${i} none modelled`, n, { 'data.counts': hours(...pk), ...modelledNone(n) });
    }
  }
  // junctions altered without pedestrian volumes
  const cJ = ['nidc05pnh', 'nidc07wsv', 'nidc0bhhv', 'nidc0942h'].filter(id => nodesOf('corridor').includes(id));
  for (const [i, sets] of [
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals' },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', [`scenarios.0.nodes.${cJ[1]}.upgrade`]: 'roundabout' },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', 'compliance.peds': { [cJ[0]]: 120 } },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', [`scenarios.0.nodes.${cJ[1]}.upgrade`]: 'roundabout', 'compliance.peds': { [cJ[0]]: 120 } },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', 'compliance.peds': { [cJ[0]]: 0 } },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', 'compliance.peds': { [cJ[0]]: '30' } },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', 'compliance.peds': { [cJ[0]]: -5 } },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', 'compliance.peds': { [cJ[0]]: 'many' } },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: '' },
    { [`scenarios.0.links.${linksOf('corridor')[0]}.status`]: 'proposed' },
    { [`scenarios.0.links.${linksOf('corridor')[1]}.status`]: 'proposed' },
    { [`scenarios.0.links.${linksOf('corridor')[1]}.status`]: 'proposed', 'compliance.peds': { [cJ[0]]: 40 } },
    { [`scenarios.0.links.${linksOf('corridor')[1]}.status`]: 'proposed', [`scenarios.0.links.${linksOf('corridor')[2]}.status`]: 'proposed' },
    { [`scenarios.0.links.${linksOf('corridor')[1]}.status`]: 'proposed', [`scenarios.0.links.${linksOf('corridor')[9]}.status`]: 'closed' },
    { [`scenarios.0.links.${linksOf('corridor')[1]}.status`]: 'proposed', [`scenarios.0.links.${linksOf('corridor')[9]}.status`]: 'closed', [`scenarios.0.links.${linksOf('corridor')[10]}.status`]: 'closed' },
    { [`scenarios.1.nodes.${cJ[0]}.upgrade`]: 'signals' },
    { active: 's-opt', [`scenarios.1.nodes.${cJ[0]}.upgrade`]: 'signals' },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', [`scenarios.0.links.${linksOf('corridor')[1]}.status`]: 'proposed' },
    { [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals', [`scenarios.0.links.${linksOf('corridor')[1]}.status`]: 'proposed', 'compliance.peds': { [cJ[0]]: 10 } },
    { active: 'no such scenario', [`scenarios.0.nodes.${cJ[1]}.upgrade`]: 'signals' },
  ].entries()) await L(`lint corridor peds ${i}`, 'corridor', sets);
  for (const [i, sets] of [
    { active: 's-opt' },
    { active: 's-opt', 'compliance.peds': { nidc0geyw: 20 } },
    { active: 's-opt', 'compliance.peds': { nidc0hbhp: 20 } },
    { active: 's-opt', 'compliance.peds': { nidc0hbhp: 20, nidc0geyw: 20 } },
    { active: 's-dm', 'scenarios.1.nodes.nidc09da4.upgrade': 'signals' },
    { 'scenarios.0.nodes.nidc09da4.upgrade': 'signals', 'scenarios.0.nodes.nidc0ajz2.upgrade': 'signals', 'scenarios.0.nodes.nidc0egst.upgrade': 'roundabout' },
  ].entries()) await L(`lint grid peds ${i}`, 'grid', sets);
  // the base model status, sensitivity, cumulative
  for (const n of ['corridor', 'grid']) {
    await L(`lint ${n} calibrated AM`, n, { 'me.info.AM': { date: '2026-03-01' } });
    await L(`lint ${n} calibrated PM only`, n, { 'me.info.PM': { date: '2026-03-01' } });
    await L(`lint ${n} validated`, n, { 'me.info.AM': { date: '2026-03-01' }, 'validation.AM': { pass: true, independent: true, date: '2026-04-01', category: 'E' } });
    await L(`lint ${n} sensitivity on second`, n, { 'scenarios.1.role': 'sensitivity' });
    await L(`lint ${n} sensitivity on first`, n, { 'scenarios.0.role': 'sensitivity' });
    await L(`lint ${n} register not empty`, n, { register: [{ id: 'r1', name: 'Consent', type: 'consent', status: 'Granted', likely: 'likely' }] });
    await L(`lint ${n} register empty list`, n, { register: [] });
    await L(`lint ${n} register null`, n, { register: null });
  }
  await L('lint corridor no committed zone', 'corridor', Object.fromEntries(Array.from({ length: 2 }, (_, i) => [`scenarios.${i}.zones.zCom.role`, 'development'])));
  await L('lint corridor committed in one', 'corridor', { 'scenarios.0.zones.zCom.role': 'development' });
  await L('lint corridor no committed zone with register', 'corridor', { ...Object.fromEntries(Array.from({ length: 2 }, (_, i) => [`scenarios.${i}.zones.zCom.role`, 'development'])), register: [{ id: 'x', name: 'X', type: 'consent' }] });
  await L('lint grid committed zone', 'grid', { 'scenarios.2.zones.zB.role': 'committed' });
  // trip rates: source and year, and old rates not reviewed
  const rateSets = (k, o) => Object.fromEntries(['src', 'source', 'year', 'reviewed'].filter(f => o[f] !== undefined).map(f => [`settings.rates.${k}.${f}`, o[f]]));
  const rateCases = [
    ['all sourced', { ...rateSets('residential', { src: 'ITE 2022', year: 2023 }), ...rateSets('childcare', { source: 'AT study', year: 2024 }) }],
    ['one sourced', rateSets('residential', { src: 'ITE 2022', year: 2023 })],
    ['source no year', { ...rateSets('residential', { src: 'ITE', year: 0 }), ...rateSets('childcare', { src: 'ITE' }) }],
    ['year no source', { ...rateSets('residential', { year: 2023 }), ...rateSets('childcare', { year: 2021 }) }],
    ['old not reviewed', { ...rateSets('residential', { src: 'ITE', year: 2019 }), ...rateSets('childcare', { src: 'ITE', year: 2022 }) }],
    ['old reviewed', { ...rateSets('residential', { src: 'ITE', year: 2019, reviewed: true }), ...rateSets('childcare', { src: 'ITE', year: 2022, reviewed: 'yes' }) }],
    ['year 2023 not old', { ...rateSets('residential', { src: 'ITE', year: 2023 }), ...rateSets('childcare', { src: 'ITE', year: 2023 }) }],
    ['year text', { ...rateSets('residential', { src: 'ITE', year: '2019' }), ...rateSets('childcare', { src: 'ITE', year: 'n/a' }) }],
    ['year negative', rateSets('residential', { src: 'ITE', year: -1 })],
    ['rates null', { 'settings.rates': null }],
    ['rates empty', { 'settings.rates': {} }],
    ['rate entry null', { 'settings.rates.residential': null, 'settings.rates.childcare': null }],
    ['more land uses', { 'scenarios.0.zones.zDev.lu.industrial': 100, 'scenarios.0.zones.zDev.lu.office': 5, 'scenarios.0.zones.zDev.lu.hotel': 0, 'scenarios.0.zones.zDev.lu.unknownuse': 3 }],
    ['land use in an unknown table slot', { 'scenarios.0.zones.zDev.lu.constructor': 5 }],
    ['lu not positive', { 'scenarios.0.zones.zDev.lu': { residential: -3, childcare: 'x', retail: '4' } }],
  ];
  for (const [id, sets] of rateCases) await L(`lint corridor rates ${id}`, 'corridor', sets);
  for (const [id, sets] of [
    ['all sourced', { ...rateSets('residential', { src: 'ITE', year: 2023 }), ...rateSets('retail', { src: 'ITE', year: 2023 }), ...rateSets('supermarket', { src: 'ITE', year: 2023 }), ...rateSets('office', { src: 'ITE', year: 2023 }), ...rateSets('apartment', { src: 'ITE', year: 2023 }) }],
    ['two old', { ...rateSets('retail', { src: 'ITE', year: 2018 }), ...rateSets('office', { src: 'ITE', year: 2020, reviewed: false }) }],
  ]) await L(`lint grid rates ${id}`, 'grid', sets);
  // trip discounts without evidence
  for (const [i, sets] of [
    { 'scenarios.0.zones.zDev.reduction': 15 },
    { 'scenarios.0.zones.zDev.reduction': 15, 'scenarios.0.zones.zDev.reductionSrc': 'Travel plan' },
    { 'scenarios.0.zones.zDev.reduction': '10' },
    { 'scenarios.0.zones.zDev.reduction': 0 },
    { 'scenarios.0.zones.zDev.reduction': -5 },
    { 'scenarios.0.zones.zCom.reduction': 10 },
    { 'scenarios.0.zones.zEx.reduction': 10 },
    { 'scenarios.0.zones.gW.reduction': 10 },
  ].entries()) await L(`lint corridor tdm ${i}`, 'corridor', sets);
  for (const [i, sets] of [
    { 'scenarios.0.zones.zA.reduction': 15, 'scenarios.0.zones.zB.reduction': 5 },
    { 'scenarios.0.zones.zA.reduction': 15, 'scenarios.0.zones.zB.reduction': 5, 'scenarios.0.zones.zB.reductionSrc': 'x' },
  ].entries()) await L(`lint grid tdm ${i}`, 'grid', sets);
  // unfunded projects outside a sensitivity scenario
  const cl = linksOf('corridor'), gl = linksOf('grid');
  for (const [i, sets] of [
    { [`scenarios.0.links.${cl[0]}.stage`]: 'unfunded' },
    { [`scenarios.0.links.${cl[0]}.stage`]: 'unfunded', [`scenarios.1.links.${cl[1]}.stage`]: 'unfunded' },
    { [`scenarios.0.links.${cl[0]}.stage`]: 'unfunded', 'scenarios.0.role': 'sensitivity' },
    { [`scenarios.0.links.${cl[0]}.stage`]: 'unfunded', [`scenarios.0.links.${cl[0]}.status`]: 'closed' },
    { [`scenarios.0.links.${cl[0]}.stage`]: 'committed', [`scenarios.0.links.${cl[0]}.funding`]: 'unfunded' },
    { [`scenarios.0.links.${cl[0]}.stage`]: 'committed', [`scenarios.0.links.${cl[0]}.funding`]: 'programmed' },
    { [`scenarios.0.links.${cl[0]}.stage`]: 'committed', [`scenarios.0.links.${cl[0]}.funding`]: 'funded' },
    { register: [{ id: 'n1', name: 'Mill Road', type: 'network', funding: 'unfunded', links: [cl[2]] }] },
    { register: [{ id: 'n1', name: 'Mill Road', type: 'network', funding: 'funded', links: [cl[2]] }] },
    { register: [{ id: 'n1', name: 'Mill Road', type: 'network', funding: 'unfunded', links: [cl[2]] }], 'scenarios.0.role': 'sensitivity', 'scenarios.1.role': 'sensitivity' },
    { register: [{ id: 'n1', name: 'Mill Road', type: 'network', funding: 'unfunded', links: [cl[2], cl[3]] }, { id: 'n2', name: 'Other', type: 'network', links: [cl[4]] }] },
    { register: [{ id: 'n1', name: 'Mill Road', type: 'network', funding: 'unfunded', links: ['nolink'] }] },
  ].entries()) await L(`lint corridor unfunded ${i}`, 'corridor', sets);
  await L('lint grid unfunded in two', 'grid', { [`scenarios.1.links.${gl[0]}.stage`]: 'unfunded', [`scenarios.2.links.${gl[1]}.stage`]: 'unfunded', [`scenarios.2.links.${gl[2]}.stage`]: 'unfunded' });
  // everything at once
  await L('lint corridor everything', 'corridor', { 'data.counts': hours('12:00', '12:30', '13:00').map(x => ({ ...x, date: '', src: '' })), 'scenarios.0.zones.zDev.reduction': 20, [`scenarios.0.nodes.${cJ[0]}.upgrade`]: 'signals',
    [`scenarios.0.links.${cl[0]}.stage`]: 'unfunded', 'scenarios.0.zones.zDev.lu.industrial': 10, ...rateSets('residential', { src: 'ITE', year: 2018 }), ...modelledAM('corridor') });

  write('expected/compliance-f4-lint.json', { cases: lint });
  const E = (id, call, fixture, sets, f, extra) => add(expert, id, call, fixture, sets, f, extra);
  await expertCases({ c, C, base, nodesOf, linksOf, zonesOf, nScen, E, expert, hours, counts, rateSets });
  await diffCases({ c, C, base, diff, add, nodesOf });
  write('expected/compliance-f4-expert.json', { cases: expert });
  write('expected/compliance-f4-diff.json', { cases: diff });
  write('expected/compliance-f4-jsonerr.json', { cases: jsonCorpus() });
});

/* ---------- expert disclosure, sign-off, data sources, appendix G, badge, supersession, disclaimers, JWS ---------- */
async function expertCases({ c, C, base, nodesOf, linksOf, zonesOf, nScen, E, expert, hours, counts, rateSets }) {
  const scripts = [];
  // scripted cases: steps that change the project and read it back (sign-off and its clearing)
  const script = async (id, fixture, sets, steps) => {
    const P = await variant(c, fixture, sets);
    const res = [];
    for (const st of steps) {
      let r = {};
      try {
        switch (st.op) {
          case 'sets': setPaths(P, st.sets); break;
          case 'signOff': r = { ok: plain(C.signOff(P, st.who, st.now)) }; break;
          case 'clearStale': r = { ok: plain(C.clearStale(P, st.now)) }; break;
          case 'expert': r = { ok: plain(C.state(P).expert) }; break;
          case 'appendixG': r = { ok: plain(C.appendixG(P)) }; break;
          case 'jwsCSV': r = { ok: plain(C.jwsCSV(P, st.result)) }; break;
          case 'dataSources': r = { ok: plain(C.dataSources(P)) }; break;
          default: throw new Error('unknown op ' + st.op);
        }
      } catch (e) { r = { error: String(e && e.message || e) }; }
      res.push({ ...st, ...r });
    }
    expert.push({ id, call: 'script', fixture, sets, steps: res });
  };
  const who = (o = {}) => ({ name: 'Jo Planner', quals: 'MEngNZ, CPEng', ...o });
  const SIGN = [
    ['plain', [{ op: 'signOff', who: who() }, { op: 'expert' }]],
    ['with now text', [{ op: 'signOff', who: who(), now: '2026-11-05' }, { op: 'expert' }]],
    ['with now stamp', [{ op: 'signOff', who: who(), now: 1784000000000 }, { op: 'expert' }]],
    ['reviewed', [{ op: 'signOff', who: who({ role: 'reviewed' }) }, { op: 'expert' }]],
    ['approved', [{ op: 'signOff', who: who({ role: 'approved' }) }, { op: 'expert' }]],
    ['bad role', [{ op: 'signOff', who: who({ role: 'boss' }) }, { op: 'expert' }]],
    ['role is an object name', [{ op: 'signOff', who: who({ role: 'constructor' }) }, { op: 'expert' }]],
    ['role number', [{ op: 'signOff', who: who({ role: 3 }) }, { op: 'expert' }]],
    ['role blank', [{ op: 'signOff', who: who({ role: '' }) }, { op: 'expert' }]],
    ['no name', [{ op: 'signOff', who: { quals: 'MEngNZ' } }, { op: 'expert' }]],
    ['no quals', [{ op: 'signOff', who: { name: 'Jo' } }, { op: 'expert' }]],
    ['blank name', [{ op: 'signOff', who: { name: '   ', quals: 'MEngNZ' } }]],
    ['no who', [{ op: 'signOff' }, { op: 'expert' }]],
    ['empty who', [{ op: 'signOff', who: {} }, { op: 'expert' }]],
    ['trimmed', [{ op: 'signOff', who: { name: '  Jo Planner  ', quals: '\tMEngNZ\n' } }, { op: 'expert' }]],
    ['number name', [{ op: 'signOff', who: { name: 12345, quals: 'MEngNZ' } }, { op: 'expert' }]],
    ['zero name falls to author', [{ op: 'signOff', who: { name: 0, quals: 0 } }, { op: 'expert' }]],
    ['twice same role', [{ op: 'signOff', who: who() }, { op: 'signOff', who: who({ name: 'Second Person' }), now: '2026-12-01' }, { op: 'expert' }]],
    ['three roles', [{ op: 'signOff', who: who() }, { op: 'signOff', who: who({ role: 'reviewed', name: 'Rev' }) }, { op: 'signOff', who: who({ role: 'approved', name: 'App' }) }, { op: 'expert' }]],
    ['sign then change', [{ op: 'signOff', who: who() }, { op: 'sets', sets: { 'settings.growth': 3.5 } }, { op: 'clearStale', now: '2026-11-02' }, { op: 'expert' }, { op: 'clearStale' }, { op: 'expert' }]],
    ['sign then no change', [{ op: 'signOff', who: who() }, { op: 'clearStale' }, { op: 'expert' }]],
    ['clear with nothing signed', [{ op: 'clearStale' }, { op: 'expert' }]],
    ['two roles one stale', [{ op: 'signOff', who: who() }, { op: 'sets', sets: { 'settings.hvPct': 9 } }, { op: 'signOff', who: who({ role: 'reviewed', name: 'Rev' }) }, { op: 'clearStale', now: '2026-12-25' }, { op: 'expert' }]],
    ['sign change sign again', [{ op: 'signOff', who: who() }, { op: 'sets', sets: { 'settings.cycle': 90 } }, { op: 'clearStale', now: 1784000000000 }, { op: 'signOff', who: who({ name: 'Again' }) }, { op: 'expert' }, { op: 'clearStale' }, { op: 'expert' }]],
    ['stale kept in history twice', [{ op: 'signOff', who: who() }, { op: 'sets', sets: { 'settings.cycle': 90 } }, { op: 'clearStale', now: '2026-11-03' }, { op: 'signOff', who: who({ name: 'Two' }) }, { op: 'sets', sets: { 'settings.cycle': 80 } }, { op: 'clearStale', now: '2026-11-04' }, { op: 'expert' }]],
    ['change a count', [{ op: 'signOff', who: who() }, { op: 'sets', sets: { 'data.counts': [{ id: 'c1', adt: 100, am: 10, pm: 11, peak: 9, date: '2025-01-01' }] } }, { op: 'clearStale' }, { op: 'expert' }]],
    ['change a zone', [{ op: 'signOff', who: who() }, { op: 'sets', sets: { 'scenarios.0.zones.zDev.lu.residential': 700 } }, { op: 'clearStale' }, { op: 'expert' }]],
    ['change only the view', [{ op: 'signOff', who: who() }, { op: 'sets', sets: { 'settings.period': 'PM', 'settings.basemap': 'x', 'settings.linzKey': 'k', 'settings.sim.scale': 2 } }, { op: 'clearStale' }, { op: 'expert' }]],
    ['author from the record', [{ op: 'sets', sets: { 'compliance.expert.author': { name: 'Author A', quals: 'BE(Hons)', org: 'Firm Ltd' } } }, { op: 'signOff', who: { role: 'prepared' } }, { op: 'expert' }]],
    ['author name but who gives quals', [{ op: 'sets', sets: { 'compliance.expert.author': { name: 'Author A', quals: '' } } }, { op: 'signOff', who: { quals: 'MEngNZ' } }, { op: 'expert' }]],
    ['sign then appendix G', [{ op: 'signOff', who: who(), now: '2026-11-05' }, { op: 'appendixG' }]],
    ['sign, change, appendix G', [{ op: 'signOff', who: who(), now: '2026-11-05' }, { op: 'sets', sets: { 'settings.growth': 3 } }, { op: 'appendixG' }, { op: 'clearStale', now: '2026-11-06' }, { op: 'appendixG' }]],
    ['sign then jws', [{ op: 'signOff', who: who(), now: '2026-11-05' }, { op: 'jwsCSV', result: { rows: [{ key: 'growth', a: 1.5, b: 3, effect: 'worst DoS 0.5 → 0.6' }] } }]],
    ['signed with a prior record', [{ op: 'sets', sets: { 'compliance.expert.signoffs': [{ role: 'prepared', name: 'Old', quals: 'Q', date: '2026-01-01', fp: 'x' }, { role: 'reviewed', name: 'Rev', quals: 'Q', date: '2026-01-01', fp: 'y' }], 'compliance.expert.history': [{ role: 'approved', cleared: '2026-01-02' }] } }, { op: 'clearStale', now: '2026-10-30' }, { op: 'expert' }]],
  ];
  for (const n of ['corridor']) for (const [id, steps] of SIGN) await script(`script ${n} ${id}`, n, {}, steps);
  for (const [id, steps] of SIGN.slice(0, 6)) await script(`script grid ${id}`, 'grid', {}, steps);

  // data sources
  const D = (id, fixture, sets) => E(id, 'dataSources', fixture, sets, P => C.dataSources(P));
  const loaded = {
    nzta: { at: 1760000000000, n: 120, label: 'AM peak', licence: 'CC BY 4.0', source: 'NZTA TMS' },
    at: { at: '2026-05-01T10:00:00Z', n: 0, licence: 'CC BY 4.0' },
    cas: { at: 1700000000000, n: 55, label: '2021 to 2025' },
    user: { at: '2026-02-03', n: 12 },
    fc: { at: 1750000000000 },
    controlled: { at: 1750000000000, n: null, label: 'AT points', licence: '' },
    osm: { at: 1740000000000, n: 400, licence: 'ODbL', label: 'Overpass' },
    other: { at: 1740000000000, n: 4 },
  };
  for (const n of ['corridor', 'grid']) {
    await D(`dataSources ${n} as saved`, n, {});
    for (const [k, v] of Object.entries(loaded)) await D(`dataSources ${n} loaded ${k}`, n, { [`data.loaded.${k}`]: v });
    await D(`dataSources ${n} all loaded`, n, { 'data.loaded': loaded });
    await D(`dataSources ${n} loaded without at`, n, { 'data.loaded': { nzta: { n: 5 }, at: null, cas: { at: 0, n: 1 }, user: { at: '' } } });
    await D(`dataSources ${n} loaded null`, n, { 'data.loaded': null });
    await D(`dataSources ${n} loaded at invalid`, n, { 'data.loaded.nzta': { at: 'yesterday', n: 3 } });
    await D(`dataSources ${n} source text beats the name`, n, { 'data.loaded.user': { at: '2026-02-03', source: 'Council export', label: 'Q1' } });
  }
  await D('dataSources corridor osm links', 'corridor', { [`scenarios.0.links.${linksOf('corridor')[0]}.osm`]: true });
  await D('dataSources corridor osm links and osm loaded', 'corridor', { [`scenarios.0.links.${linksOf('corridor')[0]}.osm`]: { id: 1 }, 'data.loaded.osm': { at: 1740000000000, n: 5 } });
  await D('dataSources corridor osm loaded', 'corridor', { 'data.loaded.osm': { at: 1740000000000, n: 5 } });
  await D('dataSources corridor osm links in the second scenario', 'corridor', { [`scenarios.1.links.${linksOf('corridor')[3]}.osm`]: 1 });
  await D('dataSources corridor site lookup', 'corridor', { 'compliance.site.lookup': { zone: { name: 'x' }, sources: [{ source: 'AUP zones', date: '2026-05-01', licence: 'CC BY 4.0' }, { source: 'Only a name' }, { date: '2026-06-01' }, { source: 'Licence only', licence: null }] } });
  await D('dataSources corridor site lookup without sources', 'corridor', { 'compliance.site.lookup': { zone: { name: 'x' } } });
  await D('dataSources corridor overlays', 'corridor', { 'compliance.overlays': { sources: [{ source: 'Overlay A', date: '2026-04-01', licence: 'CC BY' }, {}] } });
  await D('dataSources corridor overlays without sources', 'corridor', { 'compliance.overlays': { at: 1 } });
  await D('dataSources corridor sh loaded', 'corridor', { 'compliance.net.loaded.sh': { source: 'NZTA SH network', date: '2022-01-01', n: 14, licence: 'CC BY 4.0' } });
  await D('dataSources corridor sh loaded without licence', 'corridor', { 'compliance.net.loaded.sh': { source: 'NZTA SH network', date: '2022-01-01', n: 0 } });
  await D('dataSources corridor sh loaded bare', 'corridor', { 'compliance.net.loaded.sh': { x: 1 } });
  await D('dataSources corridor everything', 'corridor', { 'data.loaded': loaded, 'compliance.site.lookup': { sources: [{ source: 'AUP zones', date: '2026-05-01', licence: 'CC BY 4.0' }] }, 'compliance.overlays': { sources: [{ source: 'Overlay A', date: '2026-04-01', licence: 'CC BY' }] },
    'compliance.net.loaded.sh': { source: 'NZTA SH network', date: '2022-01-01', n: 14, licence: 'CC BY 4.0' }, ...rateSets('residential', { src: 'ITE', year: 2023 }) });
  const noLu = n => Object.fromEntries(Array.from({ length: nScen(n) }, (_, i) => zonesOf(n, i).map(z => [`scenarios.${i}.zones.${z}.lu`, {}])).flat());
  await D('dataSources corridor no land use', 'corridor', noLu('corridor'));
  await D('dataSources grid no land use', 'grid', noLu('grid'));
  for (const [id, o] of [['source and year', { src: 'ITE 11th', year: 2023 }], ['source only', { src: 'ITE' }], ['alt source', { source: 'Alt study', year: 2019 }], ['year only', { year: 2021 }], ['year text', { src: 'x', year: '2020' }], ['year zero', { src: 'x', year: 0 }]])
    await D(`dataSources corridor rate ${id}`, 'corridor', rateSets('residential', o));
  await D('dataSources corridor rate src as number', 'corridor', { 'settings.rates.residential.src': 42, 'settings.rates.childcare.source': ['a', 'b'] });
  await D('dataSources corridor gateway land use is not counted', 'corridor', { 'scenarios.0.zones.gW.lu': { office: 100 } });
  await D('dataSources corridor second scenario land use', 'corridor', { 'scenarios.1.zones.zDev.lu.hotel': 40 });
  await D('dataSources corridor constructor land use', 'corridor', { 'scenarios.0.zones.zDev.lu.constructor': 4 });
  await D('dataSources corridor rates removed', 'corridor', { 'settings.rates': null });

  // appendix G
  const G = (id, fixture, sets) => E(id, 'appendixG', fixture, sets, P => C.appendixG(P));
  for (const n of ['corridor', 'grid']) await G(`appendixG ${n} as saved`, n, {});
  for (const [id, sets] of [
    ['author', { 'compliance.expert.author': { name: 'Jo Planner', quals: 'MEngNZ', org: 'Firm Ltd' } }],
    ['author name only', { 'compliance.expert.author': { name: 'Jo Planner' } }],
    ['author name and org', { 'compliance.expert.author': { name: 'Jo Planner', org: 'Firm Ltd' } }],
    ['author org only', { 'compliance.expert.author': { org: 'Firm Ltd', quals: 'BE' } }],
    ['tests', { 'compliance.expert.tests': 'Base year run by Jo; sensitivity by Sam.' }],
    ['limitations', { 'compliance.expert.limitations': 'Static assignment only.' }],
    ['gaps', { 'compliance.expert.gaps': 'No pedestrian counts.' }],
    ['limitations and gaps', { 'compliance.expert.limitations': 'Static assignment only.', 'compliance.expert.gaps': 'No pedestrian counts.' }],
    ['confidence level only', { 'compliance.expert.confidence': { level: 'moderate', why: '' } }],
    ['confidence', { 'compliance.expert.confidence': { level: 'high', why: 'Counts are recent and the model is validated.' } }],
    ['sensitivity scenarios', { 'scenarios.1.role': 'sensitivity', 'scenarios.1.name': 'Growth plus 50%' }],
    ['sensitivity without a name', { 'scenarios.1.role': 'sensitivity', 'scenarios.1.name': null }],
    ['two sensitivity scenarios', { 'scenarios.0.role': 'sensitivity', 'scenarios.1.role': 'sensitivity' }],
    ['calibrated', { 'me.info.AM': { date: '2026-03-01' }, 'me.info.PM': { date: '2026-03-02' } }],
    ['validated', { 'me.info.AM': { date: '2026-03-01' }, 'validation.AM': { pass: true, independent: true, date: '2026-04-01' }, 'settings.tmdg.justification': 'Local network' }],
    ['pathway and lodgement', { 'compliance.ctx.pathway': 'rc-landuse', 'compliance.ctx.lodgement': '2026-11-20', 'compliance.ctx.lodgementActual': true }],
    ['data sources', { 'data.loaded.nzta': { at: 1760000000000, n: 10, licence: 'CC BY' }, 'compliance.net.loaded.sh': { source: 'NZTA SH', date: '2022-01-01', n: 14 }, 'compliance.site.lookup': { sources: [{ source: 'AUP', date: '2026-05-01' }] } }],
    ['everything', { 'compliance.expert': { author: { name: 'Jo', quals: 'Q', org: 'O' }, tests: 'T', limitations: 'L', gaps: 'G', confidence: { level: 'low', why: 'W' }, signoffs: [], history: [], supplementary: { at: '2026-10-01', text: 'x' } }, 'scenarios.1.role': 'sensitivity' }],
    ['stale signoff', { 'compliance.expert.signoffs': [{ role: 'prepared', name: 'Old', quals: 'Q', date: '2026-01-01', fp: 'x' }] }],
    ['supplementary only', { 'compliance.expert.supplementary': { at: '2026-10-01', text: 'x' } }],
  ]) await G(`appendixG corridor ${id}`, 'corridor', sets);

  // the badge
  for (const per of ['AM', 'PM']) {
    for (const [id, sets] of [
      ['uncalibrated', {}],
      ['calibrated', { 'me.info.AM': { date: '2026-03-01' }, 'me.info.PM': { date: '2026-03-02' } }],
      ['validated', { 'me.info.AM': { date: '2026-03-01' }, 'me.info.PM': { date: '2026-03-01' }, 'validation.AM': { pass: true, independent: true, date: '2026-04-01', category: 'E' }, 'validation.PM': { pass: true, independent: true, date: '2026-04-02' } }],
      ['validation not independent', { 'me.info.AM': { date: '2026-03-01' }, 'me.info.PM': { date: '2026-03-01' }, 'validation.AM': { pass: true, independent: false }, 'validation.PM': { pass: false, independent: true } }],
      ['validation for another category', { 'me.info.AM': { date: '2026-03-01' }, 'me.info.PM': { date: '2026-03-01' }, 'validation.AM': { pass: true, independent: true, category: 'B' }, 'validation.PM': { pass: true, independent: true, standard: 'mbcm' } }],
      ['justified', { 'settings.tmdg': { category: 'b', justification: 'Quiet local roads <only>' } }],
      ['another standard', { 'settings.calib.standard': 'mbcm', 'me.info.AM': { date: '2026-03-01' }, 'validation.AM': { pass: true, independent: true, standard: 'mbcm' } }],
    ]) {
      await E(`badge ${per} ${id}`, 'badge', 'corridor', sets, P => C.badge(P, per), { period: per });
    }
  }
  await E('badge default period', 'badge', 'corridor', {}, P => C.badge(P), { period: null });

  // supersession of standards changed since the outputs were stamped
  const used = (o) => ({ 'compliance.used': o });
  for (const [id, sets, now] of [
    ['nothing used', {}, undefined],
    ['all old', used({ rma: '2026-01-01', ftaa: '2026-01-01', aup: '2026-01-01', tcd: '2026-01-01', tou: '2026-01-01' }), '2026-11-19'],
    ['all old before any date', used({ rma: '2026-01-01', ftaa: '2026-01-01', aup: '2026-01-01', tcd: '2026-01-01', tou: '2026-01-01' }), '2026-10-21'],
    ['ftaa old', used({ ftaa: '2026-10-01' }), '2026-10-30'],
    ['ftaa on the day', used({ ftaa: '2026-10-30' }), '2026-11-02'],
    ['ftaa after', used({ ftaa: '2026-11-15' }), '2026-12-01'],
    ['aup old later', used({ aup: '2026-06-30' }), '2027-07-01'],
    ['aup before date', used({ aup: '2026-06-30' }), '2027-06-29'],
    ['tou and tcd', used({ tou: '2026-11-01', tcd: '2026-10-31' }), '2026-11-18'],
    ['used from the clock', used({ ftaa: '2026-10-01', tcd: '2026-10-15' }), undefined],
    ['standards without a date', used({ rma: '2025-01-01', 'at-ita': '2025-01-01', arcf: '2025-01-01', 'ipm-2027': '2025-01-01' }), '2027-12-01'],
    ['used is a number', used({ ftaa: 20261001, tcd: true }), '2027-01-01'],
    ['used is blank', used({ ftaa: '', tcd: null }), '2027-01-01'],
    ['now a timestamp', used({ ftaa: '2026-10-01' }), 1792540800000 + 30 * 864e5],
    ['now as text with time', used({ ftaa: '2026-10-01' }), '2026-11-05T10:00:00Z'],
    ['used with a time', used({ ftaa: '2026-10-29T10:00:00Z' }), '2026-11-05'],
    ['used in an unknown standard', used({ nothing: '2026-01-01' }), '2027-12-01'],
  ]) await E(`supersession ${id}`, 'supersession', 'corridor', sets, P => C.supersession(P, now), { now: now === undefined ? null : now });

  // the disclaimers a block of HTML must carry
  const P0 = base.corridor;
  const html = {
    empty: '',
    d1: C.disclaimerHtml('D1', P0),
    d1text: `<p>${c.U.esc(C.disclaimer('D1', P0))}</p>`,
    d1partial: `<p>${c.U.esc(C.disclaimer('D1', P0).slice(0, 40))}</p>`,
    d1marked: '<div data-disclaimer="D1"></div>',
    d1markedd: '<div data-d="D1"></div>',
    d1wrongmark: '<div data-disclaimer="D2"></div>',
    d3: C.disclaimerHtml('D1', P0) + C.disclaimerHtml('D3', P0),
    d13: C.disclaimerHtml('D1', P0) + C.disclaimerHtml('D13', P0),
    d13plain: `<p>${C.disclaimer('D13', P0)}</p>` + C.disclaimerHtml('D1', P0),
    d12: C.disclaimerHtml('D1', P0) + C.disclaimerHtml('D12', P0, { tool: 'SIDRA' }),
    d12text: `<p>${c.U.esc(C.disclaimer('D12', P0, { tool: 'SIDRA'}))}</p>` + C.disclaimerHtml('D1', P0),
    tags: `<p>${C.disclaimer('D1', P0).split(' ').map(w => `<b>${w}</b>`).join(' ')}</p>`,
    entities: `<p>${C.disclaimer('D1', P0).replace(/ /g, '&nbsp;').replace(/'/g, '&#39;')}</p>`,
    entities2: `<p>${C.disclaimer('D13', P0).replace(/"/g, '&quot;').replace(/'/g, '&#x27;')}</p>`,
    amp: `<p>${C.disclaimer('D1', P0).replace(/&/g, '&amp;')}</p>`,
    spacing: `<p>${C.disclaimer('D1', P0).replace(/ /g, ' \n\t  ')}</p>`,
    all: ['D1', 'D3', 'D4', 'D5', 'D6', 'D7', 'D8', 'D9', 'D10', 'D11', 'D12', 'D13'].map(id => C.disclaimerHtml(id, P0)).join(''),
    allText: ['D1', 'D3', 'D4', 'D5', 'D6', 'D7', 'D8', 'D9', 'D10', 'D11', 'D12', 'D13'].map(id => `<p>${c.U.esc(C.disclaimer(id, P0))}</p>`).join(''),
  };
  const KINDS = [[], ['junction'], ['economics'], ['anop'], ['validation'], ['model'], ['safety'], ['cbds'], ['regulatory'], ['authority'], ['handover'], ['assumption'], ['junction', 'economics', 'anop'], ['junction', 'junction'], ['nonsense'],
    ['model', 'safety', 'cbds', 'regulatory', 'authority', 'handover', 'assumption', 'validation', 'anop', 'economics', 'junction']];
  for (const [hn, h] of Object.entries(html)) for (const [i, k] of KINDS.entries()) {
    if (hn !== 'all' && hn !== 'allText' && hn !== 'empty' && i > 4 && i < 12) continue;
    await E(`missingDisclaimers ${hn} ${i}`, 'missingDisclaimers', 'corridor', {}, () => C.missingDisclaimers(h, k), { html: h, kinds: k });
  }
  for (const [id, h] of [['no tag close', '<p data-disclaimer="D1"'], ['mark in text', 'data-disclaimer="D1"'], ['markd D3', '<i data-d="D3"></i><i data-d="D1"></i>'], ['quoted differently', "<i data-d='D1'></i>"], ['D1 vs D10', '<i data-d="D10"></i>']]) {
    await E(`missingDisclaimers ${id}`, 'missingDisclaimers', 'corridor', {}, () => C.missingDisclaimers(h, ['economics', 'regulatory']), { html: h, kinds: ['economics', 'regulatory'] });
  }

  // the JWS table
  const J = (id, fixture, sets, result) => E(id, 'jwsCSV', fixture, sets, P => C.jwsCSV(P, result), { result: result === undefined ? null : result });
  const rows = [{ key: 'growth', a: 1.5, b: 3, effect: 'worst DoS 0.5 → 0.6; worst delay 12 → 14 s' }, { key: 'growthMode', a: 'linear', b: 'compound', effect: 'a, "quoted" effect' }, { key: 'connectors', a: 2, b: null, effect: 'x' },
    { key: 'hvPct', a: 6, b: 8.25, effect: '=1+1' }, { key: 'satFlow', a: true, b: false, effect: '-5' }, { key: '+k', a: '@a', b: '\tb', effect: 'multi\nline' }, { key: 'k2', effect: 'no values' }];
  for (const n of ['corridor', 'grid']) {
    await J(`jwsCSV ${n} rows`, n, {}, { rows });
    await J(`jwsCSV ${n} no rows`, n, {}, { rows: [] });
    await J(`jwsCSV ${n} no result`, n, {}, undefined);
  }
  await J('jwsCSV corridor null result', 'corridor', {}, null);
  await J('jwsCSV corridor result without rows', 'corridor', {}, { period: 'AM' });
  await J('jwsCSV corridor rows not a list', 'corridor', {}, { rows: 'x' });
  await J('jwsCSV corridor with pathway', 'corridor', { 'compliance.ctx.pathway': 'rc-landuse', 'compliance.ctx.lodgement': '2026-11-20', 'compliance.ctx.lodgementActual': true }, { rows: rows.slice(0, 2) });
  await J('jwsCSV corridor already stamped', 'corridor', {}, { rows: [{ key: '# Version stamp: x', a: 1, b: 2, effect: 'y' }] });
  await J('jwsCSV corridor model status', 'corridor', { 'me.info.AM': { date: '2026-03-01' }, 'settings.tmdg.justification': 'ok' }, { rows: rows.slice(0, 1) });
}

/* ---------- the assumption diff ---------- */
async function diffCases({ c, C, base, diff, add, nodesOf }) {
  const P = (id, call, fixture, sets, f, extra) => add(diff, id, call, fixture, sets, f, extra);
  const parse = (id, text) => P(`parseOther ${id}`, 'parseOther', 'corridor', {}, () => C.parseOther(text), { text: text === undefined ? null : text });
  const CRLF = s => s.replace(/\n/g, '\r\n');
  const JSONS = {
    'empty': '', 'blank': '   \n\t ', 'undefined': undefined, 'null': null, 'zero': 0,
    'flat': '{"growth":3}',
    'settings wrapper': '{"settings":{"growth":3,"hvPct":8}}',
    'whole project': '{"v":3,"name":"x","settings":{"growth":3,"hvPct":8,"rates":{"office":{"AM":{"rate":3,"inPct":80},"PM":{"rate":2,"inPct":20,"extra":1}}},"junction":{"pff":0.9,"T":0.25,"losMethod":"hcm6","extra":1},"turns":{"left":3,"uturn":"allow"}},"scenarios":[]}',
    'padded': '  \n {"hvPct": 9}  \n',
    'all keys': '{"growth":1,"growthMode":"linear","hvPct":2,"pcuHV":3,"peakToDaily":4,"gravityBeta":5,"satFlow":6,"cycle":7,"lostPerPhase":8,"connectorSpeed":9,"connectors":10,"bprAlpha":11,"bprBeta":12,"baseYear":13,"junction":{"losMethod":"a","pff":1,"T":2,"queueSpace":3,"f95":4},"turns":{"left":1,"right":2,"rightSignals":3,"uturn":"x"},"rates":{"retail":{"AM":{"rate":1,"inPct":2},"PM":{"rate":3,"inPct":4}}}}',
    'not compared': '{"signalOpt":false,"maxIter":50,"junction":{"capMethod":"arc"},"econ":{"discount":4},"rates":{"office":{"AM":{"other":1}}}}',
    'proto': '{"__proto__":{"growth":9},"growth":2}',
    'proto in settings': '{"settings":{"__proto__":{"growth":9},"hvPct":3}}',
    'constructor': '{"constructor":{"prototype":{"growth":1}},"hvPct":4}',
    'prototype key': '{"rates":{"prototype":{"AM":{"rate":1}},"constructor":{"AM":{"rate":1}},"office":{"AM":{"rate":4}}}}',
    'proto deep': '{"junction":{"__proto__":{"pff":1},"pff":0.8},"turns":{"constructor":1,"left":9}}',
    'dotted keys': '{"rates.office.AM.rate":5,"junction.pff":0.7,"growth.x":1,"turns.left":4}',
    'dotted key with proto': '{"rates.__proto__.AM.rate":5,"junction..pff":0.7,".growth":1,"growth.":2}',
    'long key': JSON.stringify({ ['rates.' + 'a'.repeat(190) + '.AM.rate']: 1, growth: 2 }),
    'key just short': JSON.stringify({ ['rates.' + 'a'.repeat(170) + '.AM.rate']: 1, growth: 2 }),
    'numbers as text': '{"growth":"3","hvPct":"abc","cycle":"","baseYear":"2026"}',
    'null values': '{"growth":null,"hvPct":null,"growthMode":null}',
    'bool values': '{"growth":true,"connectors":false,"growthMode":true}',
    'array values': '{"growth":[1,2],"rates":{"office":[1]},"turns":{"left":[3]}}',
    'object value for a leaf': '{"growth":{"a":1},"hvPct":{}}',
    'settings not an object': '{"settings":5,"growth":4}',
    'settings null': '{"settings":null,"growth":4}',
    'settings array': '{"settings":[1],"growth":4}',
    'settings text': '{"settings":"x"}',
    'empty settings': '{"settings":{}}',
    'empty object': '{}',
    'nested settings': '{"settings":{"settings":{"growth":8},"growth":2}}',
    'unicode keys': '{"gr\\u006fwth":6,"\\u00e9":1}',
    'escaped text': '{"growthMode":"line\\u0061r","junction":{"losMethod":"h\\"cm"}}',
    'numbers': '{"growth":1e2,"hvPct":-0,"cycle":1.5E+1,"satFlow":0.1e-1,"baseYear":12345678901234567890,"connectors":1e999}',
    'duplicate keys': '{"growth":1,"hvPct":2,"growth":3}',
    'integer-like keys': '{"5":1,"growth":2,"1":3}',
    'array': '[1,2,3]',
    'array of settings': '[{"growth":1}]',
    'empty array': '[]',
    'nested array': '[[]]',
    'invalid 1': '{',
    'invalid 2': '[',
    'invalid 3': '{"growth":}',
    'invalid 4': '{"growth":3,}',
    'invalid 5': '{growth:3}',
    'invalid 6': "{'growth':3}",
    'invalid 7': '{"growth":3',
    'invalid 8': '{"growth":3} extra',
    'invalid 9': '[1,2',
    'invalid 10': '{"growth":01}',
    'invalid 11': '{"a":"x\ny"}',
    'invalid 12': '{"a":"\\q"}',
    'invalid 13': '{"settings":{"growth":1.5,"hvPct":}}',
    'invalid 14': '{"a":tru}',
    'invalid 15': '{"a":NaN}',
    'invalid 16': '{"settings":{"growth":3,"hvPct":8,"cycle":100,"satFlow":1800,"baseYear":2026},"x":}',
    'invalid 17': '{"growth":3}\n\n{"growth":4}',
    'invalid 18': '{"growth":1,\r\n"hvPct":x}',
    'invalid 19': '{"a":"\\u12"}',
    'invalid 20': '{"a":1.}',
    'invalid 21': '{"a":"unterminated',
    'invalid 22': '[1,]',
    'invalid 23': '[,1]',
    'invalid 24': '{"a" 1}',
    'invalid 25': '{"a":1 "b":2}',
    'invalid 26': '﻿{"growth":3}',
    'invalid 27': '{"growth":" é😀"} x',
    // key,value text
    'csv plain': 'growth,3\nhvPct,8',
    'csv crlf': CRLF('growth,3\nhvPct,8\ncycle,90\n'),
    'csv header key value': 'key,value\ngrowth,3\nhvPct,8',
    'csv header item value': 'Item,Value\ngrowth,3',
    'csv header setting label value default unit': 'setting,label,value,default,unit\ngrowth,Background growth,3,1.5,% a year\nhvPct,Heavy vehicles,8,6,%\nsatFlow,Saturation flow,2000,1800,pcu/h',
    'csv header with value third': 'Parameter,Label,Value\nbprAlpha,Alpha,0.2\nbprBeta,Beta,5',
    'csv header value first': 'value,key\n3,growth\n8,hvPct',
    'csv header no key column': 'a,value\ngrowth,3',
    'csv header key no value': 'key,amount\ngrowth,3',
    'csv header case': ' SETTING , VALUE \ngrowth,3',
    'csv comments': '# Spare Construct assumptions\n# exported\nsetting,label,value\n#skipped,x,1\ngrowth,x,3',
    'csv comments spaced': '  # note\ngrowth,3\n \t# another\nhvPct,8',
    'csv blank rows': 'growth,3\n\n\nhvPct,8\n,\n',
    'csv blank cell': 'growth,\nhvPct,(blank)\ncycle,   \nsatFlow,100',
    'csv on off': 'growthMode,on\nconnectors,off\ncycle,ON\nhvPct,Off',
    'csv numbers': 'growth,1e2\nhvPct,0x10\ncycle, 12 \nsatFlow,Infinity\nbprAlpha,-0.5\nbprBeta,.5\nbaseYear,1,5',
    'csv text': 'growthMode,compound\njunction.losMethod,sidra\nturns.uturn,allow',
    'csv quoted': '"growth","3"\n"hvPct","8,5"\n"junction.losMethod","a ""b"" c"',
    'csv quoted newline': 'growthMode,"line\nar"\ngrowth,3',
    'csv short rows': 'growth\nhvPct,8\n,5\n',
    'csv unsafe': 'foo,1\nrates.office.AM.bad,3\nrates..AM.rate,3\n__proto__.growth,3\nconstructor,1\ngrowth.x,1\nrates.office.AM.rate,3',
    'csv unsafe proto segments': 'rates.__proto__.AM.rate,3\nrates.constructor.AM.rate,3\njunction.prototype,1\nrates.prototype.PM.inPct,2',
    'csv dotted': 'rates.office.AM.rate,3\nrates.office.AM.inPct,80\nrates.retail.PM.rate,5\njunction.pff,0.8\nturns.left,3',
    'csv rate names': 'rates.out_of_town.AM.rate,3\nrates.a-b.AM.rate,3\nrates.fastfood.AM.inPct,50',
    'csv duplicate': 'growth,1\ngrowth,2\nhvPct,3',
    'csv conflict path': 'rates.office.AM.rate,3\nrates.office.AM.rate,4\n',
    'csv spaces in key': ' growth ,3\n hvPct\t,8',
    'csv header words in data': 'item,x\nkey,y\nsetting,z\ngrowth,3\nKey,1',
    'csv long key': 'rates.' + 'a'.repeat(190) + '.AM.rate,1\ngrowth,2',
    'csv only header': 'key,value',
    'csv only comments': '# a\n# b',
    'csv text not csv': 'This is not a settings file.',
    'csv one word': 'growth',
    'csv quote start': '"growth,3',
    'csv unicode': 'growth,3\nhvPct,８',
    'csv bom': '﻿growth,3',
    'csv semicolon': 'growth;3\nhvPct;8',
    'csv tabs': 'growth\t3',
    'csv json-like in csv': 'growth,{"a":1}',
    'csv value with spaces': 'growthMode,  compound  ',
    'csv number words': 'growth,three\nhvPct,1,000\ncycle,1_0',
  };
  for (const [id, text] of Object.entries(JSONS)) await parse(id, text);

  // diffParams against the saved settings
  const dp = (id, fixture, sets, other) => P(`diffParams ${id}`, 'diffParams', fixture, sets, p => C.diffParams(p, other), { other: other === undefined ? null : other });
  for (const n of ['corridor', 'grid']) {
    await dp(`${n} same flat`, n, {}, { growth: base[n].settings.growth, hvPct: base[n].settings.hvPct });
    await dp(`${n} flat changes`, n, {}, '{"growth":3,"hvPct":8,"cycle":120}');
    await dp(`${n} whole project text`, n, {}, JSONS['whole project']);
    await dp(`${n} all keys`, n, {}, JSONS['all keys']);
    await dp(`${n} csv`, n, {}, JSONS['csv header setting label value default unit']);
    await dp(`${n} csv dotted`, n, {}, JSONS['csv dotted']);
    await dp(`${n} csv on off`, n, {}, JSONS['csv on off']);
    await dp(`${n} csv numbers`, n, {}, JSONS['csv numbers']);
    await dp(`${n} proto`, n, {}, JSONS['proto']);
    await dp(`${n} empty`, n, {}, '');
    await dp(`${n} invalid`, n, {}, '{"growth":');
    await dp(`${n} no settings`, n, {}, '[1]');
  }
  await dp('corridor object other', 'corridor', {}, { growth: 3, hvPct: 8, junction: { pff: 0.9, T: 0.5, extra: 1 }, turns: { left: 3 }, rates: { office: { AM: { rate: 3 } } } });
  await dp('corridor object other equal', 'corridor', {}, { growth: 2, growthMode: 'compound', hvPct: 6, cycle: 100 });
  await dp('corridor object other as numbers text', 'corridor', {}, { growth: '2', hvPct: '8', growthMode: 'compound', cycle: 100 });
  await dp('corridor object other nonfinite and bad', 'corridor', {}, { growth: null, hvPct: 'x', cycle: true, satFlow: [1], growthMode: null, connectors: {} });
  await dp('corridor object other array equals string', 'corridor', {}, { growthMode: ['compound'], 'junction': { losMethod: ['hcm6'] } });
  await dp('corridor object other text for text', 'corridor', {}, { growthMode: 'linear', 'junction': { losMethod: 'sidra' }, turns: { uturn: 'allow' } });
  await dp('corridor object other proto', 'corridor', {}, JSON.parse('{"__proto__":{"growth":9},"constructor":{"prototype":{"growth":9}},"hvPct":7,"junction":{"constructor":1,"pff":0.5,"__proto__":{"T":9}}}'));
  await dp('corridor object other keys not in settings', 'corridor', {}, { rates: { zzz: { AM: { rate: 3 } }, office: { AM: { rate: 3, other: 1 }, MD: { rate: 3 } } }, junction: { f95: 1.9, queueSpace: 8 } });
  await dp('corridor object other dotted keys', 'corridor', {}, { 'rates.office.AM.rate': 3, 'junction.pff': 0.7, 'growth': 5 });
  await dp('corridor object empty', 'corridor', {}, {});
  await dp('corridor other number', 'corridor', {}, 5);
  await dp('corridor other null', 'corridor', {}, null);
  await dp('corridor other array', 'corridor', {}, [1, 2]);
  await dp('corridor settings null values', 'corridor', { 'settings.growth': null, 'settings.hvPct': 'x' }, { growth: 3, hvPct: 8, cycle: 90 });
  await dp('corridor settings text values', 'corridor', { 'settings.growthMode': 'linear', 'settings.junction.losMethod': 'sidra' }, { growthMode: 'linear', junction: { losMethod: 'sidra' } });
  await dp('corridor settings bool', 'corridor', { 'settings.connectors': true }, { connectors: 'true' });
  await dp('corridor rates', 'corridor', {}, '{"rates":{"residential":{"AM":{"rate":0.9,"inPct":20},"PM":{"rate":0.95,"inPct":70}},"retail":{"AM":{"rate":1.5,"inPct":60}}}}');
  await dp('corridor rates csv', 'corridor', {}, 'rates.residential.AM.rate,1.2\nrates.residential.PM.inPct,50\nrates.zzz.AM.rate,3');
  await dp('corridor not a number', 'corridor', {}, 'growth,fast\nhvPct,8');
  await dp('corridor number as text csv', 'corridor', {}, 'growth,3\nhvPct,eight');

  // runDiff: each difference alone, ranked (short runs, few differences)
  const rd = (id, fixture, sets, diffs, opts = {}, scIndex = null) => P(`runDiff ${id}`, 'runDiff', fixture, sets,
    p => C.runDiff(p, diffs, { ...opts, ...(scIndex == null ? {} : { sc: p.scenarios[scIndex] }) }), { diffs, opts, scIndex });
  await rd('corridor none', 'corridor', {}, [], { maxIter: 8 });
  await rd('corridor growth and hv', 'corridor', {}, [{ key: 'growth', a: 2, b: 5 }, { key: 'hvPct', a: 6, b: 12 }], { maxIter: 8 });
  await rd('corridor PM', 'corridor', {}, [{ key: 'growth', a: 2, b: 0 }, { key: 'satFlow', a: 1800, b: 1500 }], { maxIter: 8, period: 'PM' });
  await rd('corridor second scenario', 'corridor', {}, [{ key: 'cycle', a: 100, b: 60 }, { key: 'junction.pff', a: 0.95, b: 0.8 }], { maxIter: 8 }, 1);
  await rd('corridor rates and turns', 'corridor', {}, [{ key: 'rates.residential.AM.rate', a: 0.85, b: 2 }, { key: 'turns.left', a: 2, b: 6 }, { key: 'turns.uturn', a: 'ban', b: 'allow' }, { key: 'growthMode', a: 'compound', b: 'linear' }], { maxIter: 8 });
  await rd('corridor refused and odd keys', 'corridor', {}, [{ key: '__proto__.growth', a: 1, b: 9 }, { key: 'growth.x', a: 1, b: 9 }, { key: 'hvPct', a: 6, b: 6 }, { key: 'junction.T', a: 0.5, b: 1 }, { key: 'brandNew.deep.key', a: null, b: 3 }], { maxIter: 8 });
  await rd('corridor no maxIter in a diff', 'corridor', {}, [{ key: 'growth', a: 2, b: 3 }], { maxIter: 5 });
  await rd('corridor period from settings', 'corridor', { 'settings.period': 'PM' }, [{ key: 'bprAlpha', a: 0.15, b: 0.5 }], { maxIter: 8 });
  await rd('corridor connectors and speed', 'corridor', {}, [{ key: 'connectors', a: 2, b: 3 }, { key: 'connectorSpeed', a: 25, b: 40 }, { key: 'gravityBeta', a: 0.1, b: 0.3 }], { maxIter: 8 });
  await rd('grid growth and signals', 'grid', {}, [{ key: 'growth', a: 1.5, b: 6 }, { key: 'lostPerPhase', a: 5, b: 9 }, { key: 'junction.f95', a: 1.6, b: 2.2 }], { maxIter: 8 });
  await rd('grid third scenario PM', 'grid', {}, [{ key: 'hvPct', a: 6, b: 15 }, { key: 'peakToDaily', a: 10, b: 8 }], { maxIter: 8, period: 'PM' }, 2);
  await rd('grid from text', 'grid', {}, C.diffParams(base.grid, 'growth,4\nhvPct,10\nsatFlow,1700\nrates.retail.PM.rate,6'), { maxIter: 8 });
}
