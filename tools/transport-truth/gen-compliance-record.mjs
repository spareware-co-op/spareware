// Part 'record': the compliance record, the regime and the version stamp (compliance.js sections 2 and 3) -> expected/compliance-record.json
import { part, write, record, baseFixtures, loadCompliance } from './gen-compliance-lib.mjs';

part('record', async () => {
  const c = loadCompliance();
  const C = c.Compliance;
  await baseFixtures(c);
  const cases = [];
  const add = async (id, call, fixture, sets, f) => cases.push({ id, call, ...(await record(c, fixture, sets, f)) });
  const pathways = Object.keys(C.DATA.PATHWAYS);
  const dates = ['', '2026-10-29', '2026-10-30', '2027-06-01', '2028-01-01', 'not a date'];
  // the regime for every pathway and lodgement date, in Auckland and outside it
  for (const pw of ['', ...pathways]) for (const d of dates) {
    await add(`regime ${pw || 'none'} ${d || 'blank'}`, 'regime', 'corridor', { 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': d }, P => C.regime(P));
  }
  for (const pw of ['rc-landuse', 'pc-private', 'ftaa-nor', 'nor']) for (const d of ['2026-10-29', '2027-06-01']) {
    await add(`regime other ${pw} ${d}`, 'regime', 'grid', { 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': d, 'compliance.ctx.region': 'other' }, P => C.regime(P));
  }
  // a Planning Act specified transition date that has been set (the data holds none yet)
  // the stamp on different days (an edition whose next change has passed is marked), planned and actual lodgement
  for (const now of [undefined, '2026-10-21', '2026-11-18', '2027-01-01', '2027-07-01']) {
    for (const [pw, d, act] of [['', '', false], ['rc-landuse', '2026-09-01', false], ['rc-landuse', '2026-11-20', true], ['ftaa-consent', '2026-12-01', false], ['tou', '', false], ['nltf', '2026-11-01', true]]) {
      await add(`stamp ${now || 'clock'} ${pw || 'none'} ${d || 'blank'} ${act}`, 'stamp', 'corridor', { 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': d, 'compliance.ctx.lodgementActual': act },
        P => C.stamp(P, now));
    }
  }
  // today() for every kind of moment
  for (const now of ['2026-10-21', '2026-10-21T10:00:00Z', 1792540800000, 1792540800000 + 11.5 * 3600e3, 1784000000000, 1775000000000, 'rubbish']) {
    await add(`today ${now}`, 'today', 'corridor', {}, () => C.today(now));
  }
  await add('today clock', 'today', 'corridor', {}, () => C.today());
  // the record with its defaults, and a record with some entries already made
  await add('state blank', 'state', 'corridor', {}, P => C.state(P));
  await add('state partial', 'state', 'grid', { 'compliance.ctx.pathway': 'precinct', 'compliance.site.zoneOverride': 'Business - City Centre Zone', 'compliance.q.office': 1200, 'compliance.expert.author.name': 'A Planner' }, P => C.state(P));
  // outputs recorded under a regime, and the re-check banner when the lodgement moves across 30 Oct 2026
  for (const [pw, was, now] of [['rc-landuse', '2026-10-01', '2026-11-15'], ['rc-landuse', '2026-11-15', '2026-10-01'], ['rc-landuse', '2026-10-01', '2026-10-02'], ['nltf', '2026-10-01', '2026-11-15'], ['', '2026-10-01', '2026-11-15'], ['pc-council', '2026-11-15', '2026-10-01']]) {
    await add(`recheck ${pw || 'none'} ${was} -> ${now}`, 'recheck', 'corridor', { 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': now,
      'compliance.outputs': { at: '2026-10-05', lodgement: was, side: was < '2026-10-30' ? 'pre' : 'transition', regime: 'x' } }, P => C.recheck(P));
  }
  await add('recheck no outputs', 'recheck', 'corridor', { 'compliance.ctx.pathway': 'rc-landuse', 'compliance.ctx.lodgement': '2026-11-15' }, P => C.recheck(P));
  for (const [pw, d] of [['rc-landuse', '2026-11-15'], ['ftaa-consent', '2026-09-01'], ['tou', ''], ['nltf', '2026-10-01'], ['', '']]) {
    await add(`recordOutputs ${pw || 'none'}`, 'recordOutputs', 'corridor', { 'compliance.ctx.pathway': pw, 'compliance.ctx.lodgement': d },
      P => { const o = C.recordOutputs(P, '2026-10-21'); return { o, used: C.state(P).used, docs: [...new Set(C.DATA.STAMP_DOCS.map(([id]) => id))].length }; });
  }
  write('expected/compliance-record.json', { cases });
});
