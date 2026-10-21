// Makes the ITA report's test projects: the saved test projects with every input the report reads filled in (report settings,
// author text, registers, counts, crashes, journey times, stops, feeds, zone and link attributes, scenario roles), so each section
// and option of features/report.js is reached. Used by gen-report.mjs. Everything here is deterministic.
import { clone } from './js-report-core.mjs';

// A small seeded generator (Mulberry32), so the made-up crash records are the same every run
export const seeded = seed => { let s = seed >>> 0; return () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; };

const TS = Date.UTC(2026, 9, 14, 3, 20, 0);   // the time the made-up downloads and runs carry

/* Author text for every box the report offers: blank lines make paragraphs, '- ' lines bullets; markup is escaped */
export const TEXT = {
  purpose: 'This ITA supports a resource consent application for a mixed-use development.\n\nIt answers RMA s88(2)(b) & Schedule 4, and the tests in <E27>.',
  reason: '- E27.6.1.1 trips over threshold\n- Parking infringement (E27.6.2)\n- Access location (E27.6.4)',
  locality: 'The site is on the corner of "Main Road" and Side 2, 400 m from the town centre.',
  zoning: 'Operative AUP zone: Business - Mixed Use. Proposed PC120 rezoning to Residential - Terrace Housing.',
  active: 'Footpaths on both sides.\nShared path along Main Road (west).',
  'crash-narrative': 'Most crashes at the Main Road / Side 2 priority junction involve right-turning vehicles.',
  receiving: 'The Mill Road designation is included in the future baseline only where funded.',
  access: 'Two vehicle accesses: a left-in left-out on Side 2 and a full-movement access on Side 4.',
  parking: 'Parking is provided at 1 space per dwelling plus 12 visitor spaces.',
  staging: 'Build-out over three stages, each tied to the Side 2 upgrade.',
  construction: 'Construction traffic will use Main Road between 7:00 am and 5:00 pm.',
  walk: 'Pedestrian crossings are assessed against ANOP levels of service.',
  ssa: 'A Stage 1 Safe System Audit is recommended before detailed design.',
  alignment: 'The proposal aligns with Future Connect and the RLTP.',
  climate: 'No flood hazard affects the access routes.',
  mitigation: 'Signals at Side 2 are the preferred option; a roundabout was rejected on land take.',
  conditions: 'A condition requiring the Side 2 signals before the 200th dwelling is proposed.',
  monitoring: 'Travel plan monitoring one year after occupation.',
  conclusion: 'Effects are avoided, remedied or mitigated.',
  exec: '- Net 340 vehicle trips in the AM peak.\n- Side 2 signals needed.',
  limitations: 'No SIDRA model was prepared for the Side 4 junction.',
  expert: 'The author has 15 years of experience in transport modelling.',
  scope: 'A moderate scope is proportionate: net peak trips are 340 veh/h and no state highway is involved.',
};

export function reportBlock(P, opts = {}) {
  const sc = P.scenarios;
  const zoneIds = Object.values(sc[0].zones).filter(z => z.kind !== 'gateway').map(z => z.id);
  const r = {
    details: { site: '12 Example Road, Drury', client: 'Acme Developments & Co', author: 'A. Planner', date: '2026-10-14', pathway: opts.pathway ?? 'consent', lodgement: opts.lodgement ?? '2026-11-03' },
    scope: opts.scope ?? 'moderate',
    signoff: opts.signoff ?? { reviewer: 'B. Reviewer', date: '2026-10-18', approver: 'C. Approver', reviewed: true },
    revisions: [{ rev: 'A', date: '2026-10-01', desc: 'Draft for review', prepared: 'AP', reviewed: 'BR', approved: '' }, { rev: 'B', date: '2026-10-14', desc: 'Issued <final>', prepared: 'AP', reviewed: 'BR', approved: 'CA' }],
    engagement: [{ date: '2026-08-12', party: 'Auckland Council', topic: 'Scope of the ITA', outcome: 'Moderate scope agreed' }, { date: '2026-09-03', party: 'NZTA', topic: 'State highway access', outcome: 'No objection' }],
    text: { ...TEXT, ...Object.fromEntries(zoneIds.map((id, i) => ['hours-' + id, i ? '24 hours' : '7 am to 10 pm'])) },
    sel: {},
    mitigation: { M1: { options: 'Roundabout; priority control with a right-turn bay', trigger: 'Before the 200th dwelling', responsible: 'Applicant', mechanism: 'Condition', rltp: 'No', land: 'None' }, M2: { options: '', trigger: '', responsible: '', mechanism: '', rltp: '', land: '' } },
    ssm: {},
    crossings: [
      { name: 'Main Road west of Side 2', control: 'uncontrolled', flow: 900, length: 7.5, next: 220 },
      { name: 'Side 4 refuge', control: 'refuge', flow: 800, length: 11 },
      { name: 'Zebra at the school', control: 'zebra', flow: 500, length: 6 },
      { name: 'Signals at Main Road', control: 'signals', flow: 1200, length: 14, C: 100, g: 24, next: 150 },
    ],
    ped: { Sp: 1.2, ts: 3 },
    ...opts.extra,
  };
  P.report = r;
  return r;
}

/* Made-up CAS crashes around the controlled junctions: [lat, lng, severity, year, location, flags, speed limit, fatal, serious,
   minor, light, weather, OBJECTID, control, urban] */
export function crashesAround(P, n = 60, seed = 7) {
  const rnd = seeded(seed), sc = P.scenarios[0];
  const nodes = Object.values(sc.nodes).filter(x => x.control && x.control !== 'none');
  const sev = ['F', 'S', 'M', 'N', 'N', 'M', 'N', 'S'], out = [];
  const roads = Object.values(sc.links).map(l => l.name).filter(Boolean);
  for (let i = 0; i < n; i++) {
    const nd = nodes[Math.floor(rnd() * nodes.length)] || Object.values(sc.nodes)[0];
    const d = rnd() < 0.8 ? rnd() * 0.0003 : 0.002 + rnd() * 0.004, a = rnd() * Math.PI * 2;
    const year = 2021 + Math.floor(rnd() * 6), s = sev[Math.floor(rnd() * sev.length)];
    const flags = [rnd() < 0.15 ? 'p' : '', rnd() < 0.1 ? 'c' : '', rnd() < 0.08 ? 'm' : ''].join('');
    const road = roads.length ? roads[Math.floor(rnd() * roads.length)] : 'Main Road';
    const row = [+(nd.lat + Math.sin(a) * d).toFixed(6), +(nd.lng + Math.cos(a) * d).toFixed(6), s, year, `${road} / ${roads[Math.floor(rnd() * roads.length)] || 'Side 2'}`, flags, [50, 50, 60, 80][Math.floor(rnd() * 4)]];
    if (i % 5 !== 4) row.push(s === 'F' ? 1 : 0, s === 'S' ? 1 + (rnd() < 0.2 ? 1 : 0) : 0, s === 'M' ? 1 : 0, ['Bright sun', 'Overcast', 'Dark', 'Twilight'][Math.floor(rnd() * 4)], ['Fine', 'Light rain', 'Heavy rain', 'Fine'][Math.floor(rnd() * 4)], 100000 + i, ['Nil', 'Give way', 'Traffic signals'][i % 3], 'Urban');
    out.push(row);
  }
  return out;
}

/* The project's data block: counts, crashes, loaded datasets, journey times, stops and the registers */
export function dataBlock(P, { crashes = true, long = true } = {}) {
  const sc = P.scenarios[0], links = Object.values(sc.links).filter(l => l.name), cx = (l) => Object.values(sc.nodes).find(n => n.id === l.a);
  const at = (i) => TS - i * 3600e3 * 24;
  const counts = links.slice(0, 7).map((l, i) => {
    const n = cx(l);
    const am = i % 2 === 0;
    const k = { id: 'c' + i, src: ['nzta', 'at', 'user', 'at'][i % 4], ref: 'S' + (100 + i), name: `${l.name} near ${i + 1}`, road: l.name, date: i === 2 ? '' : `202${3 + (i % 3)}-0${1 + i}-1${i}`, adt: [18200, 6400, 0, 950, 31000, 12800, 4400][i], hv: i % 3 === 0 ? 7.5 : undefined,
      peak: [1620, 540, 410, 90, 2800, 1090, 380][i], peakTime: i === 3 ? '00:00' : am ? '07:45' : '16:30', hist: i % 2 === 0 ? [17000, 17400, 17900, 18000, 18200] : undefined, lat: n ? n.lat : -37.1, lng: n ? n.lng : 174.96 };
    if (i === 0) { k.dur = 'Continuous'; k.note = 'Continuous count'; k.directional = true; k.dir = 'NB'; }
    if (i === 1) { k.dur = '24h'; k.dir = ''; }
    if (i === 4) { k.virtual = true; k.dur = ''; }
    if (i === 5) { k.dur = '7 days'; k.role = 'validation'; }
    if (i === 6) { k.off = true; }
    return k;
  });
  const D = {
    counts, crashes: crashes ? crashesAround(P) : [], jt: [], use: { nzta: true, at: true, user: true, virtual: false, ramm: true },
    loaded: {
      osm: { at: at(30), n: 410 }, nzta: { at: at(20), n: 12 }, tms: { at: at(19), n: 4 }, at: { at: at(18), n: 33 },
      cas: { at: at(17), n: crashes ? 60 : 0, label: '2021 to 2025 plus 2026 to date', from: 2021, to: 2025, ytd: 2026, lagYear: 2025 },
      nslr: { at: at(16), n: 150 }, fc: { at: at(15), n: 88 }, controlled: { at: at(14), n: 20 }, aup: { at: at(13), n: 5 }, census: { at: at(12), n: 9 }, ags23: { at: at(11), n: 3 },
      gtfs: { at: at(10), n: 211 }, user: { at: at(9), n: 4, licence: 'Client supplied' }, extra: { source: 'Your survey firm', dataset: 'Pedestrian counts', at: at(8), n: 17, licence: 'Confidential' },
    },
    msm: { checked: true }, pt: { stops: [] },
  };
  if (long) { D.crashesLong = crashesAround(P, 40, 99); D.loaded.casLong = { at: at(7), n: 40, label: '2016 to 2025' }; }
  // journey times: one with an observed time, one with a Google typical time, one with neither
  const ids = Object.values(sc.nodes).map(n => n.id);
  D.jt = [
    { id: 'jt1', name: 'Town centre to the motorway', from: ids[0], to: ids[ids.length - 1], g: { AM: { t: 380, len: 3100, when: 'Tue 7 Oct, 8:00 am' }, PM: { t: 410, len: 3100, when: 'Tue 7 Oct, 5:00 pm' } } },
    { id: 'jt2', name: 'School to the shops', from: ids[1], to: ids[Math.min(5, ids.length - 1)], obs: { AM: 240, PM: 260, src: 'Floating car survey' } },
    { id: 'jt3', name: 'Unmeasured route', from: ids[2], to: ids[3] },
  ];
  // stops near the development zones
  const z = Object.values(sc.zones).filter(q => q.kind !== 'gateway')[0] || { lat: -37.1, lng: 174.96 };
  D.pt.stops = [
    { id: 's1', code: '1001', name: 'Main Road / Side 2', mode: 'Bus', lat: z.lat + 0.001, lng: z.lng + 0.0008, routes: ['101', '102'], busesAM: 6, fcPt: 'Frequent' },
    { id: 's2', code: '1002', name: 'School stop', lat: z.lat - 0.004, lng: z.lng + 0.002, routes: ['33'], busesAM: 2.5 },
    { id: 's3', code: '1003', name: 'Far away', lat: z.lat + 0.02, lng: z.lng, routes: [] },
  ];
  P.data = D;
  P.register = [
    { id: 'r1', ref: '', name: 'Funded Mill Road widening', type: 'network', status: '', likely: 'likely', funding: 'funded', source: 'AT RLTP 2024', date: '2026-08-01', asAt: '2026-08-01', verify: false, zones: [], links: [] },
    { id: 'r2', ref: '', name: 'Unfunded bypass <A>', type: 'network', status: '', likely: 'possible', funding: 'unfunded', source: 'Council LTP', date: '', asAt: '', verify: true, zones: [], links: [] },
    { id: 'r3', ref: '', name: 'Adjacent subdivision', type: 'ftaa', status: 'Granted (approved)', likely: 'likely', funding: null, source: 'Consent RC123', date: '2026-05-04', asAt: '2026-05-04', verify: false, zones: [], links: [] },
    { id: 'r4', ref: '', name: 'Neighbour plan change', type: 'precinct', status: 'Lodged', likely: 'possible', funding: null, source: 'PC99', date: '2026-02-04', asAt: '2026-02-04', verify: false, include: true, note: 'Author considers it likely', zones: [], links: [] },
  ];
  P.feeds = {
    vintage: { nslr: { source: 'NZTA', note: 'National Speed Limit Register', at: at(6), n: 1500, licence: 'CC BY 4.0' }, ags: { source: 'Auckland Council', note: 'AGS23 growth scenario', at: at(5), n: 20, error: 'partial', licence: 'Auckland Council open data' }, tms: { at: at(4), n: 12 }, junk: 'not an object' },
    gtfs: { stops: [{ id: 'g1', code: 'G1', name: 'GTFS stop', lat: 0, lng: 0, routes: ['9'], am: 4 }], publisher: 'Auckland Transport', date: '20261021', at: at(3), feed: { provider: 'AT', licence: 'CC BY 4.0' } },
  };
  P.landuse = { sources: { lu1: { name: 'Auckland Council land use', vintage: '2025 rating data', at: at(2), n: 300, licence: 'CC BY 4.0' } } };
  P.me = { info: { AM: { date: TS, iters: 12, matrixChange: { total: 0.08 }, tripEndR2: 0.941, stopped: 'tolerance reached' }, PM: { date: TS, iters: 9, stopped: 'iteration limit' } } };
  P.validation = { AM: { pass: true, independent: true, category: 'E', standard: 'tmdg', date: TS } };
  return D;
}

/* Link and zone attributes the report tables read */
export function decorate(P) {
  for (const sc of P.scenarios) {
    Object.values(sc.links).forEach((l, i) => {
      if (!l.name) return;
      if (i % 2 === 0) l.fc = { gt: ['Arterial', 'Collector', 'Local'][i % 3], pt: i % 4 === 0 ? 'Frequent' : 'Connector', freight: i % 3 === 0 ? '1B' : '', cycle: 'Primary', gt10: 'Arterial', pt10: 'Rapid', freight10: '2', cycle10: 'Secondary' };
      if (i % 3 === 0) { l.speedLimit = [50, 60, 80][i % 3 === 0 ? (i / 3) % 3 : 0]; l.speedSource = 'NSLR'; }
      else if (i % 3 === 1) l.nslr = { v: 50 };
      if (i % 5 === 0) l.obsSpeed = 47;
      if (i % 4 === 1) l.cycle = 'painted'; else if (i % 4 === 2) l.cycle = 'separated';
    });
  }
  for (const sc of P.scenarios) for (const z of Object.values(sc.zones)) if (z.kind !== 'gateway' && z.lu) { z.year = 2029; z.reduction = 12; z.buildout = 0.9; z.src = 'user'; }
  return P;
}

export const compact = o => JSON.parse(JSON.stringify(o));
export { clone };
