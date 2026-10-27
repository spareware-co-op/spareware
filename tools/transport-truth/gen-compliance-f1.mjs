// Part 'f1': the scope and wording linter, the precinct staging tracker, effects, the mitigation register, condition parameters and the trigger
// finder (compliance-extra.js sections 5 to 7) -> expected/compliance-f1-lint.json, -precinct.json, -mitigation.json, -trigger.json and
// -trigger-anop.json (the ANOP criterion needs the ANOP part of the port), fixtures/data/compliance-f1-*.json.
import { part, write, record, saveFixture, withResults, run, open, plain, loadCompliance } from './gen-compliance-lib.mjs';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const JS = join(here, '..', '..', 'projects', 'spare-construct', 'demo', 'transport', 'js', 'features', 'compliance-extra.js');

// the random ids the prototype makes (U.uid) cannot be reproduced: they are recorded as '<uid>' and the Rust test normalises the same way
const UID = /^(mit|cond)[0-9a-z]{6,}$/;
const normUids = v => JSON.parse(JSON.stringify(plain(v)), (k, x) => (typeof x === 'string' && UID.test(x) ? '<uid>' : x));

// a report context the report lane would pass (the Rust test has the same one)
const REPORT_CTX = { table: (title, html, o) => `<section class="rt"><h3>${title}</h3>${html}<p class="src">${(o && o.note) || ''}</p></section>` };

const PATHWAYS = ['', 'rc-landuse', 'rc-subdivision', 'precinct', 'pc-private', 'pc-council', 'nor', 'outline-plan', 'ftaa-consent', 'ftaa-nor', 'nltf', 'speed-limit', 'dc-iff', 'tou'];
const DATES = ['', '2026-05-01', '2026-10-29', '2026-10-30', '2027-06-01', '2028-02-01'];
const ctxSets = (pathway, lodgement, more) => ({ 'compliance.ctx.pathway': pathway, 'compliance.ctx.lodgement': lodgement, ...(more || {}) });

/* ===================================================================================== 5. the linter */
const LINT_SENTENCES = [
  // scope-1a
  'The proposal changes retail distribution across the district.', 'Trade competition from the new supermarket is not considered.', 'There is demand for the development in this area.', 'There is demand for the project.',
  'The viability of the centre may be affected.', 'Views from private properties are not an effect.', 'Loss of views from private land.',
  // scope-cl7
  'Pedestrian safety is assessed under clause 7(1)(f) of Schedule 4.', 'Under cl. 7 (1) (f) the safety of the road is assessed.', 'Matters under cl 7(1)(f) are covered.', 'Safety is covered by cl 7(1)(a).',
  // scope-95a9
  'Special circumstances apply under section 95A(9).', 'We rely on s95A (9) for notification.', 'There are special circumstances here.', 'Section 95B(10) is relied on.',
  // scope-1c
  'The National Policy Direction requires this.', 'The Natural Environment Act sets the standard.', 'The NEA applies to the site.', 'The Planning Act 2026 national standards apply to the site.',
  'The Planning Act 2026, through its instruments, directs this.', 'The national standards apply under the Planning Act.', 'The Planning Act 2026 sets the process.',
  'We have not relied on the National Policy Direction.', 's104(1C) prevents reliance on the NEA.', 'Section 104(1C) bars the use of the Natural Environment Act.', 'This prevents reliance on the NEA.',
  'No regard is given to the NEA.', 'Nor do we apply the National Policy Direction.', 'The National Policy Direction is cited, without relying on it.', 'The NEA is excluded from reliance.',
  // scope-e27-ita
  'An ITA is required under E27.6.1.', 'E27.6.1 requires an integrated transport assessment.', 'The integrated transport assessment is mandatory under E27.6.1.', 'An ITA is required.', 'E27.6.1 may require an ITA.',
  'The activity requires an ITA because E27.6.1 is exceeded.',
  // scope-rca
  'Auckland Transport is the road controlling authority.', 'Auckland Transport, as RCA, must approve it.', 'AT is the RCA for this road.', 'The works (AT) are the road controlling authority.',
  'The road controlling authority is Auckland Transport.', 'The road controlling authority, AT, agreed.', 'The road controlling authority was AT.', 'Auckland Council is the road controlling authority.',
  // scope-mbcm
  'The BCR is MBCM compliant.', 'The result is compliant with the MBCM.', 'The BCR follows MBCM-compliant methods.', 'The BCR follows the MBCM.',
  // scope-tmdg
  'The TMDG statistics show the model is fit for purpose.', 'Transport Model Development Guidelines show fitness for purpose.', 'The TMDG statistics are reported.', 'The model is fit-for-purpose.',
  // scope-rasf
  'RASF is mandatory for developers.', 'The Roads and Streets Framework must comply.', 'The RASF is a useful guide.', 'The Roads and Streets Framework requires developers to follow it.', 'RASF is binding on the applicant.',
  // none
  'Traffic effects are less than minor.', 'The road is safe.',
];
const LONG = 'The National Policy Direction ' + 'applies to this very long sentence that goes on and on, '.repeat(6) + 'and ends here.';
const EXACT = n => 'The NEA applies. '.slice(0, 0) + 'The NEA applies' + 'x'.repeat(Math.max(0, n - 'The NEA applies'.length - 1)) + '.';

async function lintPart(c, X) {
  const cases = [];
  const add = async (id, call, fixture, sets, args, f) => cases.push({ id, call, args, ...(await record(c, fixture, sets, f)) });
  const all = LINT_SENTENCES.join(' ');
  // every pathway and lodgement date with all the sentences at once
  for (const pw of PATHWAYS) for (const d of DATES) {
    await add(`lint all ${pw || 'none'} ${d || 'blank'}`, 'lint', 'corridor', ctxSets(pw, d), { text: all }, P => X.lint(P, all));
  }
  // each sentence in the contexts where the rules differ
  const CTXS = [['rc-landuse', '2026-11-15'], ['rc-landuse', '2026-09-01'], ['nor', '2026-11-15'], ['nor', ''], ['ftaa-consent', '2026-11-15'], ['pc-private', '2026-11-15'], ['', '']];
  for (const s of LINT_SENTENCES) for (const [pw, d] of CTXS) {
    await add(`lint one ${pw || 'none'} ${d || 'blank'}: ${s}`, 'lint', 'corridor', ctxSets(pw, d), { text: s }, P => X.lint(P, s));
  }
  // long sentences, line breaks, exact lengths, punctuation, empty text, text that is not text
  const tx = [LONG, EXACT(240), EXACT(241), EXACT(238), 'The NEA applies.\nThe NEA applies again!  And again? Yes.\r\nThe NEA.', '', '   ', '\n\n', 'no sentence ends here and the NEA is cited', 'The NEA applies…and the NEA again', 'The NEA applies. ' + 'x'.repeat(100) + '. The NEA again with more words to make it longer than eighty characters in total. The NEA.'];
  for (const t of tx) for (const [pw, d] of [['nor', '2026-11-15'], ['nor', '']]) {
    await add(`lint text ${JSON.stringify(t).slice(0, 40)} ${pw} ${d || 'blank'}`, 'lint', 'corridor', ctxSets(pw, d), { text: t }, P => X.lint(P, t));
  }
  // the project's own texts: the Compliance+ text box, the report's author text, the mitigation and options registers
  const authorSets = {
    'complianceExtra.lint.text': 'The NEA applies to the site. Auckland Transport is the road controlling authority.',
    'report.text': { assessment: 'RASF is mandatory for developers.', blank: '   ', num: 5, summary: 'The model is fit for purpose per the TMDG statistics.' },
    'complianceExtra.mitigation': [{ id: 'm-a', ref: 'M9', measure: 'The NEA requires a signal.', options: 'ITA is required, E27.6.1 says so.', trigger: 'Special circumstances apply.' }, { id: 'm-b', measure: '', options: 'MBCM compliant BCR', trigger: 5 }, { id: 'm-c', ref: '  ', measure: 'Retail distribution effects.' }],
    'complianceExtra.options': [{ name: 'Option A', notes: 'Trade competition is high.' }, { notes: 'The viability of the centre.' }, { name: 'B', notes: '' }],
  };
  for (const [pw, d] of CTXS.slice(0, 4)) await add(`lint sources ${pw || 'none'} ${d || 'blank'}`, 'lint', 'corridor', ctxSets(pw, d, authorSets), { text: null }, P => X.lint(P));
  await add('lint sources text arg wins', 'lint', 'corridor', ctxSets('nor', '2026-11-15', authorSets), { text: 'The NEA.' }, P => X.lint(P, 'The NEA.'));
  await add('lint blank project', 'lint', 'corridor', {}, { text: null }, P => X.lint(P));
  // the lint box holding a number
  await add('lint text box number', 'lint', 'corridor', ctxSets('nor', '2026-11-15', { 'complianceExtra.lint.text': 5 }), {}, P => X.lint(P));
  await add('lint text box blank', 'lint', 'corridor', ctxSets('nor', '2026-11-15', { 'complianceExtra.lint.text': '  \n ' }), {}, P => X.lint(P));
  // asAt from the report date (scope-rca): before and after 7 May 2026
  for (const rd of ['', '2026-05-06', '2026-05-07', '2026-12-01', 'x']) {
    await add(`lint rca reportDate ${rd || 'blank'}`, 'lint', 'corridor', ctxSets('rc-landuse', '', { 'complianceExtra.ctx.reportDate': rd }), { text: 'Auckland Transport is the road controlling authority.' }, P => X.lint(P, 'Auckland Transport is the road controlling authority.'));
  }
  // MBCM: the rule applies unless the full procedures are applied to a validated model
  const mb = 'The BCR is MBCM compliant.';
  for (const [full, val] of [[false, false], [true, false], [true, true], [false, true]]) {
    const s = { 'complianceExtra.ctx.mbcmFull': full };
    if (val) { s['validation.AM'] = { pass: true, independent: true }; }
    await add(`lint mbcm full ${full} validated ${val}`, 'lint', 'corridor', ctxSets('rc-landuse', '2026-11-15', s), { text: mb }, P => X.lint(P, mb));
  }
  await add('lint mbcm validated PM only', 'lint', 'corridor', ctxSets('rc-landuse', '2026-11-15', { 'complianceExtra.ctx.mbcmFull': true, 'validation.PM': { pass: true, independent: true } }), { text: mb }, P => X.lint(P, mb));
  await add('lint mbcm validation not independent', 'lint', 'corridor', ctxSets('rc-landuse', '2026-11-15', { 'complianceExtra.ctx.mbcmFull': true, 'validation.AM': { pass: true } }), { text: mb }, P => X.lint(P, mb));
  // ignore and override: warnings can be dismissed, blocking findings only released by an override that gives a reason
  const keyOf = (id, sen) => id + '|' + sen.slice(0, 80);
  const s1c = 'The National Policy Direction requires this.', s1a = 'The proposal changes retail distribution across the district.';
  const ignoreAll = { [keyOf('scope-1c', s1c)]: true, [keyOf('scope-1a', s1a)]: true };
  for (const [pw, d] of [['rc-landuse', '2026-11-15'], ['nor', '']]) {
    await add(`lint ignore ${pw} ${d || 'blank'}`, 'lint', 'corridor', ctxSets(pw, d, { 'complianceExtra.lint.ignore': ignoreAll }), { text: s1c + ' ' + s1a }, P => X.lint(P, s1c + ' ' + s1a));
  }
  for (const reason of ['The sentence only describes the law.', '   ', '', 5, null]) {
    await add(`lint override ${JSON.stringify(reason)}`, 'lint', 'corridor', ctxSets('nor', '2026-11-15', { 'complianceExtra.lint.override': { [keyOf('scope-1c', s1c)]: { reason, at: '2026-10-05' }, [keyOf('scope-1a', s1a)]: { reason: 'warn only', at: '2026-10-06' } } }), { text: s1c + ' ' + s1a }, P => X.lint(P, s1c + ' ' + s1a));
  }
  await add('lint override without at', 'lint', 'corridor', ctxSets('nor', '2026-11-15', { 'complianceExtra.lint.override': { [keyOf('scope-1c', s1c)]: { reason: 'ok' } } }), { text: s1c }, P => X.lint(P, s1c));
  await add('lint ignore and override sources', 'lint', 'corridor', ctxSets('nor', '2026-11-15', { 'complianceExtra.lint.text': s1c + ' The NEA applies. ' + s1a, 'complianceExtra.lint.override': { [keyOf('scope-1c', s1c)]: { reason: 'law', at: '2026-10-05' } }, 'complianceExtra.lint.ignore': ignoreAll }), {}, P => X.lint(P));
  // overrideLint: records, trims and clears
  const ov = [['k1', 'because'], ['k1', '  because  '], ['k2', ''], ['k2', '   '], ['k3', null], ['k4', 5], ['k5|with bar', 'a, "quoted" reason']];
  for (const [key, reason] of ov) {
    await add(`overrideLint ${key} ${JSON.stringify(reason)}`, 'overrideLint', 'corridor', {}, { key, reason }, P => { const r = X.overrideLint(P, key, reason); return { r, override: P.complianceExtra.lint.override }; });
    await add(`overrideLint clears ${key} ${JSON.stringify(reason)}`, 'overrideLint', 'corridor', { 'complianceExtra.lint.override': { [key]: { reason: 'old', at: '2026-01-01' }, other: { reason: 'x', at: 'y' } } }, { key, reason }, P => { const r = X.overrideLint(P, key, reason); return { r, override: P.complianceExtra.lint.override }; });
  }
  await add('overrideLint on a bare project', 'overrideLint', 'corridor', { complianceExtra: null }, { key: 'a', reason: 'b' }, P => { const r = X.overrideLint(P, 'a', 'b'); return { r, record: P.complianceExtra.lint }; });
  // exportBlock
  for (const [pw, d, extra] of [['nor', '2026-11-15', { 'complianceExtra.lint.text': s1c }], ['nor', '', { 'complianceExtra.lint.text': s1c }], ['rc-landuse', '2026-11-15', { 'complianceExtra.lint.text': s1a }], ['nor', '2026-11-15', {}],
    ['nor', '2026-11-15', { 'complianceExtra.lint.text': s1c, 'complianceExtra.lint.override': { [keyOf('scope-1c', s1c)]: { reason: 'law', at: '2026-10-05' } } }],
    ['nor', '2026-11-15', { 'complianceExtra.lint.text': 'The NEA applies. The National Policy Direction applies.', 'complianceExtra.lint.override': { [keyOf('scope-1c', 'The NEA applies.')]: { reason: 'law', at: '2026-10-05' } } }]]) {
    await add(`exportBlock ${pw} ${d || 'blank'} ${JSON.stringify(extra).slice(0, 50)}`, 'exportBlock', 'corridor', ctxSets(pw, d, extra), {}, P => X.exportBlock(P));
  }
  // lintCSV and the export
  const exportsOf = id => X.__internal.EXPORTS.find(e => e[0] === id);
  const exportCase = async (idx, id, fixture, sets) => add(`export ${id} ${idx}`, 'export', fixture, sets, { id }, P => {
    const e = exportsOf(id); let enabled; try { enabled = !!c.Store.P && !!e[4](); } catch { enabled = false; }
    return { enabled, text: e[3](P) };
  });
  const csvSets = [ctxSets('nor', '2026-11-15', { 'complianceExtra.lint.text': LINT_SENTENCES.join(' ') + ' ' + LONG, 'complianceExtra.lint.override': { [keyOf('scope-1c', s1c)]: { reason: 'law, "quoted"', at: '2026-10-05' } } }),
    ctxSets('rc-landuse', '2026-11-15', { 'complianceExtra.lint.text': all }), ctxSets('', ''), ctxSets('nor', '2026-11-15', authorSets), ctxSets('=cmd', '', { 'complianceExtra.lint.text': '=1+1 The NEA. +cmd' })];
  for (const [i, s] of csvSets.entries()) {
    await add(`lintCSV ${i}`, 'lintCSV', 'corridor', s, {}, P => X.lintCSV(P));
    await exportCase(i, 'lint', 'corridor', s);
  }
  write('expected/compliance-f1-lint.json', { cases });
}

/* ===================================================================================== 6. the precinct tracker */
const NAMES = { 'scenarios.0.links.lidc0dlsv.name': 'Great South Rd', 'scenarios.0.links.lidc0lb8r.name': 'Waihoehoe Rd' };
const CUSTOM = { id: 'I999', name: 'Test Precinct', table: 'Table T.1', rule: 'T.6(1)', deemed: 'T.6(2)', ita: 'T.9', monitor: 'T.10', node: ['Great South', 'Waihoehoe'], nodeLabel: 'GS / W', pc120: true,
  rows: [{ row: 'a', dw: 100, retail: 1000, commercial: null, community: null, up: [{ id: 'u1', name: 'Upgrade one', status: 'A5' }] },
    { row: 'b', dw: 500, retail: null, commercial: 200, community: 50, up: [{ id: 'u2', name: 'Upgrade two', status: 'A6' }, { id: 'u3', name: 'Upgrade three', status: 'A5' }] }] };
const ALL_I450 = Object.fromEntries(['a1', 'a2', 'b1', 'c1', 'c2', 'd1', 'e1', 'e2', 'f1', 'f2', 'f3'].map(k => [k, 'operational']));
const pcs = (more, id = 'I450') => ({ 'complianceExtra.precinct.id': id, ...more });
const PROP = 'complianceExtra.precinct.proposal', UPG = 'complianceExtra.precinct.upgrades', REG = 'complianceExtra.precinct.register', CUS = 'complianceExtra.precinct.custom', IPL = 'complianceExtra.precinct.ipLos', IPN = 'complianceExtra.precinct.ipNode';

async function precinctPart(c, X) {
  const cases = [];
  const add = async (id, call, fixture, sets, args, f) => cases.push({ id, call, args, ...(await record(c, fixture, sets, f)) });
  { const P = open(c, 'corridor'); await run(c, P, ['AM', 'PM', 'IP']); saveFixture('f1-corridor-ip', P); }
  const PROJECTS = [
    ['none', 'corridor', {}],
    ['I450', 'corridor', pcs({})],
    ['I451', 'corridor', pcs({}, 'I451')],
    ['I450 grid', 'grid', pcs({})],
    ['unknown id', 'corridor', pcs({}, 'I999')],
    ['proposal over row a', 'corridor', pcs({ [PROP]: { ref: 'R1', dwellings: 800, retail: 0, commercial: 0, community: 0 } })],
    ['proposal GFA in row a (no cap)', 'corridor', pcs({ [PROP]: { dwellings: 100, retail: 99999, commercial: 99999, community: 99999 } })],
    ['row b within', 'corridor', pcs({ [UPG]: { a1: 'operational', a2: 'operational' }, [PROP]: { dwellings: 1000, retail: 20000 } })],
    ['row b breach A5', 'corridor', pcs({ [UPG]: { a1: 'operational', a2: 'operational' }, [PROP]: { dwellings: 1400, retail: 25000, commercial: 7000, community: 900 } })],
    ['row c breach A6', 'corridor', pcs({ [UPG]: { a1: 'operational', a2: 'operational', b1: 'operational', c1: 'operational' }, [PROP]: { dwellings: 100, retail: 40000 } })],
    ['all operational beyond', 'corridor', pcs({ [UPG]: ALL_I450, [PROP]: { dwellings: 6000, retail: 100000, commercial: 50000, community: 11000 } })],
    ['all operational within', 'corridor', pcs({ [UPG]: ALL_I450, [PROP]: { dwellings: 4000 } })],
    ['all operational beyond A9', 'corridor', pcs({ [UPG]: ALL_I450, 'complianceExtra.precinct.amendment': true, [PROP]: { dwellings: 7000 } })],
    ['construction no augier', 'corridor', pcs({ [UPG]: { a1: 'construction', a2: 'applicant' }, [PROP]: { dwellings: 100 } })],
    ['construction augier', 'corridor', pcs({ [UPG]: { a1: 'construction', a2: 'applicant' }, 'complianceExtra.precinct.augier': true, [PROP]: { dwellings: 800 } })],
    ['mixed states', 'corridor', pcs({ [UPG]: { a1: 'operational', a2: 'construction', b1: 'applicant', c1: 'weird', c2: 5 }, 'complianceExtra.precinct.augier': true })],
    ['amendment breach', 'corridor', pcs({ 'complianceExtra.precinct.amendment': true, [PROP]: { dwellings: 900 } })],
    ['amendment no breach', 'corridor', pcs({ 'complianceExtra.precinct.amendment': true })],
    ['I451 breach', 'corridor', pcs({ [PROP]: { dwellings: 900 } }, 'I451')],
    ['register', 'corridor', pcs({ [REG]: [{ ref: 'RC1', dwellings: 300, retail: 0, commercial: 10, community: 5, date: '2026-01-02', enabled: '300 dwellings' }, { ref: 'RC2', dwellings: '250', retail: '100', date: '2026-03-04' }, { ref: 'RC3', dwellings: 'abc', retail: null },
      { ref: 'EX', dwellings: 1000, excluded: true }, { ref: 'EX2', dwellings: 5, excluded: false }, { ref: 'EX3', dwellings: 7, excluded: 'yes' }, { dwellings: 1 }, null, 'text', 0] })],
    ['register pushes into row b', 'corridor', pcs({ [REG]: [{ ref: 'RC1', dwellings: 700 }, { ref: 'RC2', dwellings: 30, retail: 25000 }], [UPG]: { a1: 'operational', a2: 'operational' }, [PROP]: { dwellings: 100 } })],
    ['proposal text values', 'corridor', pcs({ [PROP]: { dwellings: '12', retail: 'abc', commercial: -5, community: null } })],
    ['proposal blank', 'corridor', pcs({ [PROP]: null })],
    ['register blank', 'corridor', pcs({ [REG]: null, [UPG]: null })],
    ['ipLos C', 'corridor', pcs({ [IPL]: 'c' })],
    ['ipLos D', 'corridor', pcs({ [IPL]: 'D' })],
    ['ipLos E', 'corridor', pcs({ [IPL]: 'e' })],
    ['ipLos F', 'corridor', pcs({ [IPL]: 'F' })],
    ['ipLos spaces', 'corridor', pcs({ [IPL]: ' d ' })],
    ['ipLos unknown', 'corridor', pcs({ [IPL]: 'z' })],
    ['ipLos with names', 'corridor', pcs({ ...NAMES, [IPL]: 'e' })],
    ['ipNode picked', 'corridor', pcs({ [IPN]: 'nidc07wsv', [IPL]: 'b' })],
    ['ipNode unknown', 'corridor', pcs({ [IPN]: 'nope', [IPL]: 'b' })],
    ['names find the junction', 'corridor', pcs({ ...NAMES })],
    ['names two junctions', 'corridor', pcs({ ...NAMES, 'scenarios.0.links.lidc0gzyt.name': 'Great South Rd', 'scenarios.0.links.lidc0phd9.name': 'Waihoehoe Rd', 'scenarios.0.links.lidc0fkhj.name': 'Great South Rd' })],
    ['names one road only', 'corridor', pcs({ 'scenarios.0.links.lidc0dlsv.name': 'Great South Rd' })],
    ['names in the option', 'corridor', pcs({ ...NAMES, 'scenarios.1.links.lidc0dlsv.name': 'Great South Rd', 'scenarios.1.links.lidc0lb8r.name': 'Waihoehoe Rd', active: 's-opt' })],
    ['modelled inter-peak', 'f1-corridor-ip', pcs({ ...NAMES })],
    ['modelled inter-peak, LoS entered too', 'f1-corridor-ip', pcs({ ...NAMES, [IPL]: 'a' })],
    ['modelled inter-peak, picked node', 'f1-corridor-ip', pcs({ [IPN]: 'nidc07wsv' })],
    ['modelled inter-peak, picked node without result', 'f1-corridor-ip', pcs({ [IPN]: 'nidc08ie4', [IPL]: 'f' })],
    ['modelled inter-peak, option active', 'f1-corridor-ip', pcs({ ...NAMES, 'scenarios.1.links.lidc0dlsv.name': 'Great South Rd', 'scenarios.1.links.lidc0lb8r.name': 'Waihoehoe Rd', active: 's-opt' })],
    ['site precinct I450', 'corridor', { 'compliance.site.precinctOverride': 'I450 Drury Centre Precinct' }],
    ['site precinct I451', 'corridor', { 'compliance.site.precinctOverride': 'I451 Drury East' }],
    ['site precinct by name only', 'corridor', { 'compliance.site.precinctOverride': 'Drury East Precinct' }],
    ['site precinct lookup', 'corridor', { 'compliance.site.lookup': { precincts: [{ name: 'I451 Drury East Precinct' }], at: '2026-10-01' } }],
    ['site precinct other', 'corridor', { 'compliance.site.precinctOverride': 'Pukekohe Precinct' }],
    ['author choice beats the site', 'corridor', pcs({ 'compliance.site.precinctOverride': 'I451 Drury East' })],
    ['author choice unknown, site wins', 'corridor', pcs({ 'compliance.site.precinctOverride': 'I451 Drury East' }, 'zzz')],
    ['custom precinct', 'corridor', pcs({ [CUS]: [CUSTOM] }, 'I999')],
    ['custom precinct breach', 'corridor', pcs({ [CUS]: [CUSTOM], [PROP]: { dwellings: 300, retail: 2000, commercial: 300, community: 80 } }, 'I999')],
    ['custom precinct upgrades', 'corridor', pcs({ [CUS]: [CUSTOM], [UPG]: { u1: 'operational', u2: 'operational', u3: 'operational' }, [PROP]: { dwellings: 900 } }, 'I999')],
    ['custom precinct names', 'corridor', pcs({ ...NAMES, [CUS]: [CUSTOM] }, 'I999')],
    ['custom precinct without optional fields', 'corridor', pcs({ [CUS]: [{ id: 'X1', rows: CUSTOM.rows }], [PROP]: { dwellings: 200 } }, 'X1')],
    ['custom precinct partial status', 'corridor', pcs({ [CUS]: [{ ...CUSTOM, id: 'X2', status: { A5: 'Only A5' }, note: 'Check the table.', pc120: false }], [PROP]: { dwellings: 200 } }, 'X2')],
    ['custom precinct partial status A6', 'corridor', pcs({ [CUS]: [{ ...CUSTOM, id: 'X2', status: { A5: 'Only A5' } }], [PROP]: { dwellings: 200 }, [UPG]: { u1: 'operational' } }, 'X2')],
    ['custom precinct empty node', 'corridor', pcs({ [CUS]: [{ ...CUSTOM, id: 'X3', node: [] }] }, 'X3')],
    ['custom precinct no node', 'corridor', pcs({ [CUS]: [{ ...CUSTOM, id: 'X4', node: null }] }, 'X4')],
    ['custom replaces I450', 'corridor', pcs({ [CUS]: [{ ...CUSTOM, id: 'I450', name: 'Mine' }] })],
    ['custom invalid entries ignored', 'corridor', pcs({ [CUS]: [null, { id: 'A' }, { rows: [] }, { id: '', rows: [] }, { id: 'B', rows: 'x' }, 5, CUSTOM] }, 'I999')],
    ['custom with number id', 'corridor', pcs({ [CUS]: [{ ...CUSTOM, id: '123', name: 'Numbered' }], 'compliance.site.precinctOverride': 'Plan 123 area' }, 'none')],
    ['custom empty name matches the site', 'corridor', { [CUS]: [{ ...CUSTOM, id: 'Q1', name: '' }] }],
    ['custom null name does not', 'corridor', { [CUS]: [{ ...CUSTOM, id: 'Q2', name: null }] }],
    ['option scenario active', 'corridor', pcs({ active: 's-opt', [PROP]: { dwellings: 100 } })],
  ];
  for (const [name, fixture, sets] of PROJECTS) {
    await add(`precinctCheck ${name}`, 'precinctCheck', fixture, sets, {}, P => X.precinctCheck(P));
    await add(`precinctOf ${name}`, 'precinctOf', fixture, sets, {}, P => X.precinctOf(P));
    await add(`precinctCSV ${name}`, 'precinctCSV', fixture, sets, {}, P => X.precinctCSV(P));
    await add(`report ${name}`, 'report', fixture, sets, { id: 'compliance-extra-precinct', ctx: false }, P => X.REPORT.find(r => r.id === 'compliance-extra-precinct').render(P));
    await add(`report ctx ${name}`, 'report', fixture, sets, { id: 'compliance-extra-precinct', ctx: true }, P => X.REPORT.find(r => r.id === 'compliance-extra-precinct').render(P, REPORT_CTX));
    await add(`export ${name}`, 'export', fixture, sets, { id: 'precinct' }, P => {
      const e = X.__internal.EXPORTS.find(x => x[0] === 'precinct'); let enabled; try { enabled = !!c.Store.P && !!e[4](); } catch { enabled = false; }
      return { enabled, text: e[3](P) };
    });
  }
  for (const [name, fixture, sets] of PROJECTS.filter((x, i) => i % 5 === 1)) {
    await add(`precinctList ${name}`, 'precinctList', fixture, sets, {}, P => X.precinctList(P));
    await add(`precinctCheck option ${name}`, 'precinctCheck', fixture, sets, { sc: 1 }, P => X.precinctCheck(P, P.scenarios[1]));
  }
  // mixTrips: trips of a mix of dwellings and floor area
  const MIXES = [{ dw: 100, retail: 0, commercial: 0, community: 0 }, { dw: 710 }, { dw: 1300, retail: 24000, commercial: 6000, community: 800 }, {}, { dw: 'abc', retail: '500' }, { dw: -50, retail: 1e6 }, { dw: null, commercial: 1000.5 }, { community: 2000 }];
  for (const [i, mix] of MIXES.entries()) for (const [fx, sets, sci] of [['corridor', {}, 0], ['corridor', {}, 1], ['grid', {}, 0], ['grid', { 'scenarios.0.buildout': 50 }, 0], ['corridor', { 'scenarios.0.year': 2040 }, 0]]) {
    await add(`mixTrips ${i} ${fx} ${sci} ${JSON.stringify(sets)}`, 'mixTrips', fx, sets, { mix, sc: sci }, P => X.mixTrips(mix, P.scenarios[sci]));
  }
  write('expected/compliance-f1-precinct.json', { cases });
}

/* ===================================================================================== 7. effects, mitigation, conditions */
const MIT = 'complianceExtra.mitigation', CON = 'complianceExtra.conditions', TRG = 'complianceExtra.triggers';
const EID_AM = 'J:nidc05pnh:AM', EID_PM = 'J:nidc05pnh:PM', EID_L = 'L:lidc0ngt6:ba:AM';
const MIT_ROWS = [
  { id: 'm-a', ref: 'M1', measure: 'Signalise the junction', effect: EID_AM, options: 'Roundabout, signals, a bus lane', nonInfra: true, trigger: 'Before 300 dwellings', responsible: 'applicant', cost: 1.25, funder: 'applicant', rltp: 'RLTP 2024', mechanism: 'condition', committed: true, landOutside: false },
  { id: 'm-b', measure: 'Widen Side 2S', effect: 'J:gone:AM', responsible: 'at', funder: 'council', committed: false, mechanism: 's84a', cost: null, landOutside: true },
  { id: 'm-c', measure: '', effect: '', responsible: '', funder: 'applicant', mechanism: '', committed: null },
  { id: 'm-d', ref: 'X', measure: 'Third measure', effect: EID_L, responsible: 'applicant', funder: '', mechanism: 'weird', cost: 0 },
  { id: 'm-e', measure: 'Fourth measure', effect: EID_PM, responsible: 'nzta', funder: 'applicant', committed: 'maybe', mechanism: 'none', trigger: 'At 100 dwellings, then yearly', rltp: 'In the LTP' },
  { id: 'm-f', measure: 'Fifth measure', funder: null, responsible: null, mechanism: null, effect: null },
  { id: 'm-g', measure: 'Sixth <b>measure</b> & "more"', funder: 'kiwirail', responsible: 'other', mechanism: 'designation', committed: true, cost: 12.345 },
  { measure: 'No id' },
];
const COND_ROWS = [
  { id: 'c-a', mitigation: 'm-a', metric: 'dwellings occupied', threshold: '300 dwellings', survey: 'Turning counts', frequency: 'Annual', response: 'Signals operational', effect: '', agreed: false, review: '' },
  { id: 'c-b', mitigation: 'm-b', effect: EID_PM, agreed: false },
  { id: 'c-c', mitigation: 'nope', effect: '', agreed: true, review: 'Custom review clause' },
  { id: 'c-d', mitigation: '', effect: 'J:gone:AM', agreed: false, metric: '', threshold: '' },
  { mitigation: 'm-c' },
  { id: 'c-f', mitigation: 'm-d', agreed: true },
  { id: 'c-g' },
  { id: 'c-h', mitigation: 'm-e', effect: '', agreed: false, threshold: 5 },
];
const FTAA = { 'compliance.ctx.pathway': 'ftaa-consent', 'compliance.ctx.lodgement': '2026-11-15' };
const exportOf = (c, X, id) => P => {
  const e = X.__internal.EXPORTS.find(x => x[0] === id); let enabled; try { enabled = !!c.Store.P && !!e[4](); } catch { enabled = false; }
  return { enabled, text: e[3](P) };
};
const reportOf = (X, id, ctx) => P => X.REPORT.find(r => r.id === id).render(P, ctx);

async function mitigationPart(c, X) {
  const cases = [];
  const add = async (id, call, fixture, sets, args, f) => cases.push({ id, call, args, ...(await record(c, fixture, sets, f)) });
  saveFixture('f1-grid-heavy', await withResults(c, 'grid', { rerun: true, mutate: P => { P.scenarios.forEach((sc, i) => { const m = [1, 4, 10][i]; for (const z of Object.values(sc.zones)) if (z.lu) for (const k of Object.keys(z.lu)) z.lu[k] *= m; }); } }));
  // a project with trigger finder results, for the tables, CSV and report
  const trigP = await withResults(c, 'corridor'); c.Store.P = trigP;
  trigP.scenarios[1].role = 'mitigation';
  const trig = await X.triggerFinder(trigP, { dos: 0.5, step: 20, upgrades: ['s-opt'] });
  saveFixture('f1-corridor-trig', trigP);
  const noneP = await withResults(c, 'corridor'); c.Store.P = noneP;
  await X.triggerFinder(noneP, { step: 50 });
  saveFixture('f1-corridor-trig-none', noneP);
  const grid = await withResults(c, 'grid'); c.Store.P = grid;
  await X.triggerFinder(grid, { dos: 0.3, step: 50 });
  saveFixture('f1-grid-trig', grid);

  /* effects */
  const NODE = 'scenarios.0.res.AM.nodes.', ARC1 = 'scenarios.1.res.AM.arcs.lidc0chmq:ab.', ARC0 = 'scenarios.0.res.AM.arcs.lidc0chmq:ab.';
  const OPT = { active: 's-opt', base: 'sidc01d0p' }, REV = { active: 'sidc01d0p', base: 's-opt' };
  const EFFECT_SETS = [
    ['corridor', {}], ['corridor', OPT], ['corridor', REV], ['corridor', { active: 's-opt', base: 's-opt' }], ['corridor', { 'scenarios.0.role': 'baseline', active: 's-opt', base: 's-opt' }],
    ['corridor', { 'scenarios.0.res.PM.mismatch': true }], ['corridor', { 'scenarios.0.res.PM': null }], ['corridor', { 'scenarios.0.res': {} }],
    ['corridor', { [NODE + 'nidc07wsv.dos']: 0.6, [NODE + 'nidc07wsv.xp']: 0.5 }], ['corridor', { [NODE + 'nidc07wsv.dos']: 0.95 }], ['corridor', { [NODE + 'nidc07wsv.dos']: 0.9 }], ['corridor', { [NODE + 'nidc07wsv.dos']: 0.91, [NODE + 'nidc07wsv.xp']: 0 }],
    ['corridor', { [NODE + 'nidc0942h.los']: 'E' }], ['corridor', { [NODE + 'nidc0942h.los']: 'e' }], ['corridor', { [NODE + 'nidc0942h.los']: 'F', [NODE + 'nidc0942h.losText']: 'F (custom text)' }], ['corridor', { [NODE + 'nidc0942h.los']: null, [NODE + 'nidc0942h.dos']: 1.5 }],
    ['corridor', { ...REV, [NODE + 'nidc07wsv.delay']: 40 }], ['corridor', { ...REV, [NODE + 'nidc07wsv.delay']: 22.4 }], ['corridor', { ...REV, [NODE + 'nidc07wsv.delay']: 23 }],
    ['corridor', { ...OPT, [ARC1 + 'vc']: 0.95 }], ['corridor', { ...OPT, [ARC1 + 'vc']: 0.95, [ARC0 + 'vc']: 0.93 }], ['corridor', { ...OPT, [ARC1 + 'vc']: 0.95, [ARC0 + 'vc']: 0.9 }], ['corridor', { ...OPT, [ARC1 + 'vc']: 0.9 }], ['corridor', { [ARC0 + 'vc']: 0.95 }],
    ['corridor', { ...OPT, [ARC1 + 'vc']: 0.95, 'scenarios.1.links.lidc0chmq.name': null }], ['corridor', { ...OPT, [ARC1 + 'vc']: 0.95, 'scenarios.1.links.lidc0chmq.name': null, 'scenarios.1.links.lidc0chmq.cls': 'zzz' }],
    ['corridor', { ...OPT, [ARC1 + 'vc']: 0.95, 'scenarios.1.links.lidc0chmq': null }],
    ['corridor', { 'scenarios.0.res.AM.nodes': null, 'scenarios.0.res.AM.arcs': null }],
    ['grid', {}], ['grid', { active: 's-opt', base: 's-dm' }],
    ['f1-grid-heavy', {}], ['f1-grid-heavy', { active: 's-opt', base: 's-dm' }], ['f1-grid-heavy', { active: 's-opt', base: 'sidc01pnz' }], ['f1-grid-heavy', { active: 's-dm', base: 'sidc01pnz', 'scenarios.2.role': 'mitigation' }],
    ['f1-grid-heavy', { active: 's-opt', base: 's-dm', 'scenarios.2.res.AM.nodes.nidc0ajz2.dos': 0.97, 'scenarios.2.res.AM.nodes.nidc0csv8.delay': 60, 'scenarios.2.res.PM.nodes.nidc0krpm.los': 'E' }],
    ['f1-corridor-ip', {}], ['f1-corridor-ip', OPT],
  ];
  for (const [i, [fx, sets]] of EFFECT_SETS.entries()) {
    await add(`effects ${i} ${fx} ${JSON.stringify(sets).slice(0, 90)}`, 'effects', fx, sets, {}, P => X.effects(P));
    await add(`effects sc ${i} ${fx}`, 'effects', fx, sets, { sc: 1 }, P => X.effects(P, P.scenarios[1]));
  }

  /* the register and the conditions */
  const mk = (rows, cond, more) => ({ [MIT]: rows, [CON]: cond, ...(more || {}) });
  const TABLE_PROJECTS = [
    ['rows', 'corridor', mk(MIT_ROWS, COND_ROWS)],
    ['rows ftaa', 'corridor', mk(MIT_ROWS, COND_ROWS, FTAA)],
    ['rows ftaa nor', 'corridor', mk(MIT_ROWS, COND_ROWS, { 'compliance.ctx.pathway': 'ftaa-nor' })],
    ['rows rc', 'corridor', mk(MIT_ROWS, COND_ROWS, { 'compliance.ctx.pathway': 'rc-landuse' })],
    ['rows option active', 'corridor', mk(MIT_ROWS, COND_ROWS, OPT)],
    ['rows reverse', 'corridor', mk(MIT_ROWS, COND_ROWS, REV)],
    ['rows grid', 'f1-grid-heavy', mk(MIT_ROWS, COND_ROWS)],
    ['empty', 'corridor', {}],
    ['conditions only', 'corridor', mk([], COND_ROWS)],
    ['mitigation only', 'corridor', mk(MIT_ROWS, [])],
    ['one measure', 'corridor', mk([{ measure: 'Only one' }], [])],
    ['one condition', 'corridor', mk([], [{ metric: 'x' }])],
    ['triggers only', 'f1-corridor-trig', {}],
    ['triggers and rows', 'f1-corridor-trig', mk(MIT_ROWS, COND_ROWS, FTAA)],
    ['triggers none found', 'f1-corridor-trig-none', mk(MIT_ROWS.slice(0, 2), COND_ROWS.slice(0, 2))],
    ['triggers grid', 'f1-grid-trig', {}],
  ];
  // hand-made trigger results: what the tables and CSV print for each shape of row
  const t0 = JSON.parse(JSON.stringify(plain(trig)));
  const tv = f => { const t = JSON.parse(JSON.stringify(t0)); f(t); return t; };
  const TRIG_SHAPES = [
    ['as run', t => t], ['no copy', t => { t.copy = ''; }], ['one period', t => { t.periods = ['AM']; t.scName = 'Name <b> & "x"'; }], ['no existing', t => { for (const r of t.rows) r.existing = []; }],
    ['existing undefined', t => { for (const r of t.rows) delete r.existing; }], ['worsened', t => { for (const r of t.rows) for (const e of r.existing) e.worsenedAt = 0; }],
    ['no values at 100', t => { for (const r of t.rows) for (const e of r.existing) { e.at100 = null; e.at0 = null; } }],
    ['no first', t => { for (const r of t.rows) { r.first = null; } }],
    ['no resolved', t => { for (const r of t.rows) delete r.resolved; }],
    ['resolved null state', t => { for (const r of t.rows) r.resolved = { state: null, text: 'Nothing to resolve' }; }],
    ['first without node values', t => { for (const r of t.rows) if (r.first) { r.first.name = 'A & B'; r.first.why = 'x <y>'; r.first.dwellings = 1234.6; } }],
    ['los missing', t => { for (const r of t.rows) for (const e of r.existing || []) { if (e.at0) delete e.at0.los; if (e.at100) e.at100.los = ''; } }],
    ['no rows', t => { t.rows = []; }],
  ];
  for (const [n, f] of TRIG_SHAPES) TABLE_PROJECTS.push([`trigger shape ${n}`, 'corridor', { [TRG]: tv(f), [MIT]: [{ measure: 'Measure with <b>' }] }]);
  for (const [name, fixture, sets] of TABLE_PROJECTS) {
    await add(`mitigationTable ${name}`, 'mitigationTable', fixture, sets, {}, P => X.mitigationTable(P));
    await add(`conditionTable ${name}`, 'conditionTable', fixture, sets, {}, P => X.conditionTable(P));
    await add(`mitigationCSV ${name}`, 'mitigationCSV', fixture, sets, {}, P => X.mitigationCSV(P));
    for (const id of ['compliance-extra-mitigation', 'compliance-extra-triggers']) {
      await add(`report ${id} ${name}`, 'report', fixture, sets, { id, ctx: false }, reportOf(X, id));
      await add(`report ctx ${id} ${name}`, 'report', fixture, sets, { id, ctx: true }, reportOf(X, id, REPORT_CTX));
    }
    await add(`export ${name}`, 'export', fixture, sets, { id: 'mitigation' }, exportOf(c, X, 'mitigation'));
  }

  /* seeding the register and the conditions */
  const SEED_PROJECTS = [
    ['corridor mitigation scenario', 'corridor', { 'scenarios.1.role': 'mitigation' }],
    ['corridor no mitigation scenario', 'corridor', {}],
    ['corridor mitigation is the option', 'corridor', { 'scenarios.1.role': 'mitigation', active: 's-opt', base: 'sidc01d0p' }],
    ['corridor already seeded', 'corridor', { 'scenarios.1.role': 'mitigation', [MIT]: [{ id: 'm-x', seed: 'diff:node:nidc05pnh', measure: 'Seeded earlier', effect: '' }, { id: 'm-y', seed: 'eff:J:nidc05pnh:AM', measure: '', effect: EID_AM }] }],
    ['corridor effects linked', 'corridor', { 'scenarios.1.role': 'mitigation', [MIT]: [{ id: 'm-x', measure: 'Linked', effect: EID_AM }, { id: 'm-y', measure: 'Linked too', effect: 'L:lidc0ngt6:ba:PM' }] }],
    ['corridor econ costs', 'corridor', { 'scenarios.1.role': 'mitigation', 'settings.econ.node': { signals: 1.8, roundabout: 2.2 } }],
    ['corridor no econ', 'corridor', { 'scenarios.1.role': 'mitigation', 'settings.econ': null }],
    ['corridor nodes upgraded', 'corridor', { 'scenarios.1.role': 'mitigation', 'scenarios.1.nodes.nidc0942h.upgrade': true, 'scenarios.1.nodes.nidc07wsv.control': 'roundabout' }],
    ['corridor new road and lanes', 'corridor', { 'scenarios.1.role': 'mitigation', 'scenarios.1.links.lidc0ngt6.lanesAB': 3, 'scenarios.1.links.lidc0lb8r.busLane': true, 'scenarios.1.links.lnew1': { id: 'lnew1', a: 'nidc03abu', b: 'nidc0kim3', name: 'New connection', cls: 'collector', lanesAB: 1, lanesBA: 1, speed: 50, status: 'proposed' }, 'scenarios.1.links.lnew2': { id: 'lnew2', a: 'nidc0mlwa', b: 'nidc0olv4', cls: 'local', lanesAB: 2, lanesBA: 1, status: 'proposed' }, 'scenarios.1.links.lnew3': { id: 'lnew3', a: 'nidc0mlwa', b: 'nidc0olv4', cls: 'collector', status: 'closed' }, 'settings.econ.laneKm': { local: 2, collector: null } }],
    ['grid mitigation scenario', 'f1-grid-heavy', { 'scenarios.2.role': 'mitigation', active: 's-dm', base: 'sidc01pnz' }],
    ['grid mitigation scenario, lane costs', 'f1-grid-heavy', { 'scenarios.2.role': 'mitigation', active: 's-dm', base: 'sidc01pnz', 'settings.econ.laneKm': { collector: 4.5 }, 'scenarios.2.links.lidc01h5wz.lanesAB': 2 }],
    ['grid effects only', 'f1-grid-heavy', {}],
    ['trigger found', 'f1-corridor-trig', { [MIT]: [{ id: 'm-x', measure: 'Signals at Side 2', trigger: 'Before 100 dwellings', effect: EID_AM }, { id: 'm-y', measure: '', effect: '' }, { id: 'm-z', measure: 'Third', effect: 'x' }] }],
    ['no trigger', 'corridor', { [MIT]: [{ id: 'm-x', measure: 'Signals at Side 2', trigger: 'Before 100 dwellings', effect: EID_AM }, { id: 'm-y', measure: 'Second', trigger: '' }, { id: 'm-z', measure: 'Third', trigger: 0 }, { measure: 'No id' }] }],
    ['conditions exist', 'corridor', { [MIT]: [{ id: 'm-x', measure: 'Signals' }, { id: 'm-y', measure: 'Other' }, { id: 'm-z', measure: 'Third' }], [CON]: [{ mitigation: 'm-x' }, { id: 'c-2' }] }],
    ['conditions exist, row without id', 'corridor', { [MIT]: [{ measure: 'No id' }, { id: 'm-y', measure: 'Other' }], [CON]: [{ id: 'c-2' }] }],
    ['trigger first null', 'corridor', { [MIT]: [{ id: 'm-x', measure: 'Signals' }], [TRG]: { rows: [{ first: null }] } }],
    ['trigger first before text', 'corridor', { [MIT]: [{ id: 'm-x', measure: 'Signals' }], [TRG]: { rows: [{ first: { before: '1234.7' } }] } }],
    ['trigger rows empty', 'corridor', { [MIT]: [{ id: 'm-x', measure: 'Signals' }], [TRG]: { rows: [] } }],
    ['bare project', 'corridor', { complianceExtra: null }],
  ];
  for (const [name, fixture, sets] of SEED_PROJECTS) {
    await add(`seedMitigation ${name}`, 'seedMitigation', fixture, sets, {}, P => {
      const n1 = X.seedMitigation(P), rows1 = normUids(P.complianceExtra.mitigation), n2 = X.seedMitigation(P);
      return { n1, rows1, n2, rows2: normUids(P.complianceExtra.mitigation) };
    });
    await add(`seedConditions ${name}`, 'seedConditions', fixture, sets, {}, P => {
      const n1 = X.seedConditions(P), rows1 = normUids(P.complianceExtra.conditions), n2 = X.seedConditions(P);
      return { n1, rows1, n2, rows2: normUids(P.complianceExtra.conditions) };
    });
    await add(`seed both ${name}`, 'seedBoth', fixture, sets, {}, P => {
      const a = X.seedMitigation(P), b = X.seedConditions(P);
      return { a, b, mitigation: normUids(P.complianceExtra.mitigation), conditions: normUids(P.complianceExtra.conditions), table: normUids(X.mitigationTable(P)), cond: normUids(X.conditionTable(P)) };
    });
  }

  /* failsAt, storageOf and ordinal */
  const CRITS = [
    { dos: 0.95, los: 'D', queue: true, minStorage: 50 }, { dos: 0.3, los: 'B', queue: true, minStorage: 0 }, { dos: null, los: '', queue: false }, { dos: 0.5 }, { queue: true, minStorage: 1e9 }, { los: 'F' }, { dos: '0.4', los: 'A', queue: true },
    { dos: 0.1, los: 'C', queue: 1, minStorage: null }, { dos: 0.95, los: 'D', queue: true, minStorage: 50, anop: true, worsen: { dos: 0.05, delay: 10 } }, {}, { los: 'Z', dos: 0 },
  ];
  for (const fx of ['corridor', 'f1-grid-heavy', 'f1-corridor-ip']) for (const [si, per] of [[0, 'AM'], [0, 'PM'], [1, 'AM'], [2, 'PM'], [0, 'IP']]) for (const [ci, crit] of CRITS.entries()) for (const only of [null, ['nidc05pnh', 'nidc0ajz2', 'nidc0942h'], []]) {
    if (only && ci % 3) continue;
    await add(`failsAt ${fx} ${si} ${per} crit ${ci} ${only ? only.length : 'all'}`, 'failsAt', fx, {}, { sc: si, period: per, crit, only }, P => {
      const sc = P.scenarios[si]; if (!sc || !sc.res[per]) return null;
      return X.failsAt(sc, sc.res[per], crit, only ? new Set(only) : null);
    });
  }
  for (const fx of ['corridor', 'grid']) for (const sets of [{}, { 'scenarios.0.nodes.nidc04vgo.control': 'signals' }, { 'scenarios.0.links.lidc0dlsv.ring': true }, { 'scenarios.0.nodes.nidc067js.control': 'priority', 'scenarios.0.nodes.nidc03abu.control': 'none' }, { 'scenarios.0.links.lidc0chmq.status': 'closed' }, { 'scenarios.0.links.lidc0dlsv.pts': [[-36.9, 174.7], [-36.91, 174.71]] }]) {
    await add(`storageOf ${fx} ${JSON.stringify(sets)}`, 'storageOf', fx, sets, {}, P => {
      const out = {};
      for (const sc of P.scenarios) for (const n of Object.values(sc.nodes)) for (const L of Object.values(sc.links)) if (L.a === n.id || L.b === n.id) out[`${sc.id}|${n.id}|${L.id}`] = X.storageOf(sc, L.id, n.id);
      out.missing = X.storageOf(P.scenarios[0], 'nope', 'x');
      return out;
    });
  }
  const ordSrc = readFileSync(JS, 'utf8').match(/const ordinal = (n => \{[^\n]*\});/);
  const ordinal = new Function('F', 'return ' + ordSrc[1])(c.U.fmt);
  const NUMS = [...Array.from({ length: 131 }, (_, i) => i), 1000, 1001, 1002, 1003, 1011, 1012, 1013, 1021, 1022, 1023, 1111, 12345, 99999, 1e6, -1, -11, -21, 1.5, 21.5, 11.2, 'NaN', 'Infinity', '-Infinity', 0.4, 100.5];
  for (const n of NUMS) cases.push({ id: `ordinal ${n}`, call: 'ordinal', args: { n }, ok: ordinal(typeof n === 'string' ? +n : n) });
  write('expected/compliance-f1-mitigation.json', { cases });
}

/* ===================================================================================== 7. the trigger finder */
async function triggerPart(c, X) {
  const cases = [];
  const add = async (id, fixture, sets, opts, f) => cases.push({ id, call: 'triggerFinder', args: { opts }, ...(await record(c, fixture, sets, f)) });
  // the result, and whether it was kept in the project's record
  const runWith = opts => async P => {
    const r = await X.triggerFinder(P, opts);
    const saved = P.complianceExtra && P.complianceExtra.triggers;
    return { r, saved: saved == null ? 'none' : JSON.stringify(plain(saved)) === JSON.stringify(plain(r)) ? 'same' : 'other' };
  };
  const MITIGATION = { 'scenarios.1.role': 'mitigation' };
  // the roundabout at Side 6N is signalised in the option and the Side 4N junction is the same in both, so a upgrade can clear one site and leave another
  const PARTLY = { 'scenarios.0.nodes.nidc07wsv.control': 'priority', 'scenarios.1.nodes.nidc07wsv.control': 'priority', 'scenarios.1.nodes.nidc0942h.control': 'signals', 'scenarios.0.zones.zDev.lu.residential': 1500 };
  const RUNS = [
    ['default', 'corridor', {}, {}],
    ['dos 0.5 step 20', 'corridor', {}, { dos: 0.5, step: 20 }],
    ['upgrade s-opt', 'corridor', {}, { dos: 0.5, step: 20, upgrades: ['s-opt'] }],
    ['mitigation scenario is the upgrade', 'corridor', MITIGATION, { dos: 0.5, step: 25 }],
    ['step the option', 'corridor', {}, { scId: 's-opt', dos: 0.5, step: 50, periods: ['AM'] }],
    ['los B, no queue', 'corridor', {}, { los: 'B', queue: false, step: 50 }],
    ['worsen at once', 'corridor', {}, { worsenDos: 0, step: 50 }],
    ['nothing worsens, queue with no minimum', 'corridor', {}, { worsenDelay: 1000, worsenDos: 1000, queue: true, minStorage: 0, step: 50 }],
    ['one node', 'corridor', {}, { nodes: ['nidc05pnh'], dos: 0.3, step: 50 }],
    ['grid default', 'grid', {}, {}],
    ['grid upgrade', 'grid', {}, { dos: 0.3, step: 25, upgrades: ['s-opt'], scId: 's-dm' }],
    ['unknown scenario, repeated upgrades', 'corridor', {}, { scId: 'nope', upgrades: ['s-opt', 's-opt', 'sidc01d0p'], step: 100, dos: 0.5 }],
    ['step 33.3', 'corridor', {}, { step: 33.3, dos: 0.5 }],
    ['step above the maximum', 'corridor', {}, { step: 200, dos: 0.5 }],
    ['build-out 40', 'corridor', { 'scenarios.0.buildout': 40 }, { dos: 0.3, step: 50 }],
    ['build-out text', 'corridor', { 'scenarios.0.buildout': 'abc' }, { dos: 0.3, step: 50 }],
    ['register measure in the copy', 'corridor', { [MIT]: [{ id: 'm-x', measure: '' }, { id: 'm-y', measure: 'Signalise Side 2' }] }, { dos: 0.5, step: 50 }],
    ['inter-peak', 'f1-corridor-ip', {}, { periods: ['IP'], step: 50, dos: 0.3 }],
    ['minimum storage text', 'corridor', {}, { queue: true, minStorage: 'abc', step: 50, dos: 0.5 }],
    ['not saved', 'corridor', {}, { save: false, dos: 0.5, step: 50 }],
    ['save null', 'corridor', {}, { save: null, dos: 0.5, step: 50 }],
    ['no criteria', 'corridor', {}, { dos: null, los: null, queue: false, step: 50 }],
    ['unknown upgrade', 'corridor', {}, { dos: 0.3, upgrades: ['nonexistent'], step: 50 }],
    ['criteria as text', 'corridor', {}, { dos: '0.5', los: 'C', step: 50 }],
    ['assignment options', 'corridor', {}, { gapTol: 0.01, maxIter: 20, step: 50, dos: 0.5 }],
    ['no periods to run', 'f1-corridor-ip', { 'scenarios.0.res.AM': null, 'scenarios.0.res.PM': null }, { step: 50 }],
    ['no results, default periods', 'corridor', { 'scenarios.0.res': {} }, { step: 50, dos: 0.5 }],
    ['periods given empty', 'corridor', {}, { periods: [], step: 50, dos: 0.5 }],
    ['nodes given empty', 'corridor', {}, { nodes: [], step: 50, dos: 0.5 }],
    ['option active, base case do-minimum', 'corridor', { active: 's-opt', base: 'sidc01d0p' }, { dos: 0.5, step: 50 }],
    ['resolved: still fails', 'corridor', MITIGATION, { dos: 0.72, step: 20, worsenDos: 1000, worsenDelay: 1000, upgrades: ['s-opt'] }],
    ['resolved: no failure with the upgrade', 'corridor', { ...MITIGATION, 'scenarios.1.zones.zDev.lu.residential': 100 }, { dos: 0.72, step: 20, worsenDos: 1000, worsenDelay: 1000 }],
    ['resolved: fails earlier', 'corridor', { ...MITIGATION, 'scenarios.1.zones.zDev.lu.residential': 1500 }, { dos: 0.72, step: 20, worsenDos: 1000, worsenDelay: 1000 }],
    ['resolved: moves later', 'corridor', { ...MITIGATION, 'scenarios.1.zones.zDev.lu.residential': 450 }, { dos: 0.72, step: 20, worsenDos: 1000, worsenDelay: 1000 }],
    ['resolved: nothing to resolve', 'corridor', { ...MITIGATION, 'scenarios.1.zones.zDev.lu.residential': 100 }, { dos: 0.6, step: 20, worsenDos: 1000, worsenDelay: 1000 }],
    ['resolved: partly', 'corridor', { ...MITIGATION, ...PARTLY, 'scenarios.1.zones.zDev.lu.residential': 2600 }, { dos: 0.8, step: 25, worsenDos: 1000, worsenDelay: 1000, upgrades: ['s-opt'], periods: ['AM'] }],
    ['resolved: partly lost to another site', 'corridor', { ...MITIGATION, ...PARTLY, 'scenarios.1.zones.zDev.lu.residential': 4500 }, { dos: 0.8, step: 25, worsenDos: 1000, worsenDelay: 1000, upgrades: ['s-opt'], periods: ['AM'] }],
    ['grid three scenarios', 'f1-grid-heavy', { active: 's-opt', base: 's-dm', 'scenarios.1.role': 'mitigation' }, { dos: 0.9, step: 50 }],
  ];
  for (const [name, fixture, sets, opts] of RUNS) await add(`triggerFinder ${name}`, fixture, sets, opts, runWith(opts));
  write('expected/compliance-f1-trigger.json', { cases });

  // the ANOP criterion needs the ANOP part (anopCorridors, anopLoS, hierOf, anopPref, anopFit): its own file
  const acases = [];
  const addA = async (id, fixture, sets, opts, f) => acases.push({ id, call: 'triggerFinder', args: { opts }, ...(await record(c, fixture, sets, f)) });
  const HEAVY = n => ({ active: 's-opt', base: 'sidc01d0p', 'scenarios.1.zones.zDev.lu.residential': n, 'scenarios.1.zones.zDev.lu.retail': n * 10 });
  const NOCRIT = { dos: 50, los: 'F', queue: false };
  const ARUNS = [
    ['anop drop at 75%', 'corridor', HEAVY(3000), { anop: true, scId: 's-opt', step: 25, ...NOCRIT }],
    ['anop drop at 25%', 'corridor', HEAVY(8000), { anop: true, scId: 's-opt', step: 25, ...NOCRIT }],
    ['anop on, no drop', 'corridor', HEAVY(1000), { anop: true, scId: 's-opt', step: 25, ...NOCRIT }],
    ['anop drop, with upgrade', 'corridor', { ...HEAVY(3000), 'scenarios.0.role': 'mitigation' }, { anop: true, scId: 's-opt', upgrades: ['sidc01d0p'], step: 25, ...NOCRIT }],
    ['anop drop, with the usual criteria', 'corridor', HEAVY(3000), { anop: true, scId: 's-opt', step: 25 }],
    ['anop drop, AM only', 'corridor', HEAVY(3000), { anop: true, scId: 's-opt', step: 25, periods: ['AM'], ...NOCRIT }],
    ['anop on, the stepped scenario is the do-minimum', 'corridor', {}, { anop: true, dos: 0.5, step: 50 }],
    ['anop on, the stepped scenario is the do-minimum with no criteria', 'corridor', {}, { anop: true, step: 50, ...NOCRIT }],
    ['anop on, grid', 'grid', { active: 's-opt', base: 'sidc01pnz' }, { anop: true, scId: 's-opt', step: 50, ...NOCRIT }],
    ['anop on, heavy grid', 'f1-grid-heavy', { active: 's-opt', base: 'sidc01pnz' }, { anop: true, scId: 's-opt', step: 50, ...NOCRIT }],
    ['anop off', 'corridor', HEAVY(3000), { anop: false, scId: 's-opt', step: 25, ...NOCRIT }],
  ];
  for (const [name, fixture, sets, opts] of ARUNS) await addA(`triggerFinder ${name}`, fixture, sets, opts, runWith(opts));
  write('expected/compliance-f1-trigger-anop.json', { cases: acases });
}

part('f1', async () => {
  const c = loadCompliance();
  const X = c.ComplianceExtra;
  await lintPart(c, X);
  await precinctPart(c, X);
  await mitigationPart(c, X);
  await triggerPart(c, X);
});
