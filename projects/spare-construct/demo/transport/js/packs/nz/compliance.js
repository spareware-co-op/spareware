'use strict';
/* Spare Construct · compliance (owner: compliance lane). The Compliance drawer tab and the project's regulatory context
   (docs/regulatory): pathway, lodgement date, regime and version stamp (reg-regime-stamp); authority resolver and
   affected operators (reg-authority); E27 trigger screening with tiers, the qualified 100 veh/h rule, exemptions,
   precinct redirect, PC120 overlay and policy matrix (e27-screening, reg-e27-extend); ITA trigger, scope and
   proportionality (reg-ita-scope); access and frontage checks (reg-access); state highway and NZTA approvals
   (reg-sh); the committed-development register UI (reg-receiving-env; data and rules in model.js Register);
   FDS consistency (reg-fds); RFI linter and statutory clocks (reg-rfi-clock); expert disclosure, sign-off and
   assumption diff (reg-expert); the model status badge (reg-model-status); standing disclaimers (reg-disclaimers);
   the standards register (reg-standards); and the 87-item regulatory checklist (docs/regulatory/_checklist.md).

   Project data lives in Store.P.compliance (blank() below). Nothing here changes model inputs or results: the
   checks read the network, zones, results, counts and crashes. Thresholds and rules are versioned data (DATA)
   with a source and a verified flag; a value Spare Construct chose itself is labelled 'Spare Construct assumption' (D13).

   Deviations from the spec (also in the lane report):
   - Table E27.6.4.2.1: the maximum is '1 per 25 m (or 50 m) of frontage or part thereof' in the July 2026 text,
     so Spare Construct rounds up (ceil). The spec said floor; both give 2 for a 50 m frontage.
   - SISD (NZTA PPM Access guide Table 8-1) is computed with the Austroads GRD Part 4A formula (decision time 5.5 s,
     d = 0.36) at posted + 10 km/h; it gives 226 m at 80 km/h, as the spec's acceptance test. Tables 8-2 and 8-3
     (spacing) are not encoded: the guide could not be downloaded to check them, so Spare Construct asks for a manual check.
   - CSV exports get D1, the DRAFT watermark and the version stamp as FOOTER comment rows (the spec says header
     row comment): a header row would break Spare Construct's own OD and count importers. Stamping is opt-in: this file's
     own exports, the human-readable tables in STAMP.tables and any export registered with stamp: true. Machine
     hand-over files (SIDRA volumes, turning movements, OD matrices: STAMP.handover) stay byte-clean, because a
     comment row breaks the SIDRA 11 loader; D1 and D12 for them travel with the hand-over (data pack header,
     loader script, volume grids). An export registered with stamp: true and handover: '<tool>' also gets D12.
   - The assumption diff runs each difference as an unsaved scenario variant (Engine.assign with a settings
     override) rather than adding scenarios to the project; 'Save as scenarios' is not offered because scenarios
     do not carry their own settings.
   - State highways come from the NZTA SH_Network layer (digitised 2022), conflated to model links, plus motorway
     and ramp classes and the user's own ticks. LAR, NZTA designation and noise-contour layers are flagged for a
     manual check when no open layer was found (the AUP Designation layer is used for the designation at the site).
   - A zone connected straight to a motorway (GRPA s84) is confirmed with a reason in this tab (State highways); the
     warning stays in the tab and checklist SH-03 until then. The network editor itself (core shell) does not block.
   - The pavement test (G00907 s5.4.6) is a total-traffic proxy while the model has one HV share for all traffic;
     the author can give the development's HV share for the added traffic.
   - The FDS is read from the live layer or entered by hand with the strategy page; it is not digitised here (no
     copy of the strategy was available to check against).
   - Report: 'report-regulatory' replaces the report lane's regime table (same id), so the pathway, lodgement date and
     regime have one source (this tab). The register (2.8), limitations and front-matter scope stay with the report
     lane; this file adds the E27 table, discretion, ITA scope, access, state highway, FDS, operator, standards,
     expert-disclosure, timeline, model-status (Appendix B) and checklist sections.
   - CHECKLIST below is the table in docs/regulatory/_checklist.md, copied verbatim (the 'Spare Construct fills' column split
     into fills and how). Keep the two in step when the doc changes. */

const Compliance = (() => {
  const esc = s => U.esc(s == null ? '' : String(s));
  const DAY = 864e5;
  const pad2 = n => String(n).padStart(2, '0');
  const isoUTC = t => { const d = new Date(t); return `${d.getUTCFullYear()}-${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())}`; };
  const parseDay = s => { const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(String(s || '')); return m ? Date.UTC(+m[1], +m[2] - 1, +m[3]) : null; };
  const validDay = s => parseDay(s) != null;
  // Today in New Zealand as YYYY-MM-DD (now can be a Date, a timestamp or a YYYY-MM-DD string, for tests)
  let nzDay = null;   // one formatter: building an Intl.DateTimeFormat per call is slow
  function today(now) {
    if (typeof now === 'string' && validDay(now)) return now.slice(0, 10);
    const t = now instanceof Date ? now : new Date(now == null ? Date.now() : now);
    try {
      nzDay ||= new Intl.DateTimeFormat('en-NZ', { timeZone: 'Pacific/Auckland', year: 'numeric', month: 'numeric', day: 'numeric' });
      const p = Object.fromEntries(nzDay.formatToParts(t).map(x => [x.type, x.value]));
      if (+p.year > 0) return `${p.year}-${pad2(p.month)}-${pad2(p.day)}`;
    } catch { }
    return isoUTC(t.getTime());
  }
  const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
  const longDate = s => { const t = parseDay(s); if (t == null) return '–'; const d = new Date(t); return `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()}`; };
  const dayName = s => { const t = parseDay(s); return t == null ? '' : WEEKDAYS[new Date(t).getUTCDay()]; };
  const num = (v, d = 0) => (v == null || !isFinite(v) ? '–' : U.fmt.num(v, d));

  /* ====================================================================================================
     1. Regulatory data (versioned). Each source says whether its numbers were checked against the text.
     ==================================================================================================== */
  const VERSION = '2026-10-08';   // the regulatory data is stated as at this date (docs/regulatory/_overview.md)
  const ASSUMPTION = 'Spare Construct assumption';
  const SRC = {
    planningAct: { name: 'Planning Act 2026 (2026 No 70), Schedule 1 and Schedule 12', url: 'https://www.legislation.govt.nz/act/public/2026/70/en/latest/', verified: false },
    rma: { name: 'Resource Management Act 1991', url: 'https://www.legislation.govt.nz/act/public/1991/69/en/latest/', verified: true,
      note: 's2 working day, s88(3), s92A(1), s92AA(1), s92(3A) and Sch 1 cl 23(1)–(2) checked against the legislation.govt.nz text on 9 Oct 2026; s103B evidence periods from the spec, not checked' },
    ftaa: { name: 'Fast-track Approvals Act 2024 (2024 No 56), version as at 10 July 2026', url: 'https://www.legislation.govt.nz/act/public/2024/56/en/latest/', verified: false,
      note: 'The FTAA has no own working-day definition, so the Legislation Act 2019 s13 definition applies (checked 9 Oct 2026). s54, s55(2) and s67 periods from the spec.' },
    legAct: { name: 'Legislation Act 2019, s13 (working day)', url: 'https://www.legislation.govt.nz/act/public/2019/58/en/latest/', verified: true },
    matariki: { name: 'Te Kāhui o Matariki Public Holiday Act 2022, Schedule 1', url: 'https://www.legislation.govt.nz/act/public/2022/14/en/latest/', verified: true },
    lgaca: { name: 'Local Government (Auckland Council) Act 2009, as amended by 2026 No 15 (in force 7 May 2026)', url: 'https://www.legislation.govt.nz/act/public/2026/15/en/latest/', verified: false },
    e27: { name: 'Auckland Unitary Plan (Operative in part), E27 Transport, July 2026 print', url: 'https://unitaryplan.aucklandcouncil.govt.nz/Images/Auckland%20Unitary%20Plan%20Operative/Chapter%20E%20Auckland-wide/4.%20Infrastructure/E27%20Transport.pdf', verified: true,
      note: 'Table E27.6.1.1, E27.6.1(1)–(2), E27.6.4.1–E27.6.4.3 and Table E27.4.1 (A3)–(A8) checked against the July 2026 PDF on 9 Oct 2026. The AUP modification schedule 2026 lists no E27 change after 8 May 2026 (PC79 fully operative; cl 20A to E27.3(6)). Check the operative text at lodgement.' },
    i450: { name: 'AUP I450 Drury Centre Precinct', url: 'https://unitaryplan.aucklandcouncil.govt.nz/images/Auckland%20Unitary%20Plan%20Operative/Chapter%20I%20Precincts/4.%20South/I450%20Drury%20Centre%20Precinct.pdf', verified: true, note: 'I450.6(2)(b): E27.6.1 does not apply (checked 9 Oct 2026)' },
    i451: { name: 'AUP I451 Drury East Precinct', url: 'https://unitaryplan.aucklandcouncil.govt.nz/images/Auckland%20Unitary%20Plan%20Operative/Chapter%20I%20Precincts/4.%20South/I451%20Drury%20East%20Precinct.pdf', verified: true, note: 'I451.6(2): E27.6.1 does not apply (checked 9 Oct 2026)' },
    atIta: { name: 'Auckland Transport ITA Guidelines, 2025 Version 2', url: 'https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf', verified: false },
    nztaIta: { name: 'NZTA Planning Policy Manual: ITA Guide G00907 (July 2026)', url: 'https://www.nzta.govt.nz/assets/resources/planning-policy-manual/docs/planning-policy-manual-integrated-transport-assessment-guide-july-2026.pdf', verified: false,
      note: 'Table 1 and Table 2 values from Spare Construct\'s regulatory research (docs/regulatory/framework.json); the PDF could not be downloaded to check them.' },
    nztaAccess: { name: 'NZTA PPM: Access onto the state highway from private property (26 Sept 2025)', url: 'https://www.nzta.govt.nz/assets/resources/planning-policy-manual/docs/PPM-access-onto-the-state-highway-guide-from-private-property.pdf', verified: false },
    austroads: { name: 'Austroads Guide to Road Design Part 4A, SISD (decision time 5.5 s, d = 0.36)', url: 'https://austroads.gov.au/publications/road-design/agrd04a', verified: false },
    grpa: { name: 'Government Roading Powers Act 1989', url: 'https://www.legislation.govt.nz/act/public/1989/0075/latest/whole.html', verified: false },
    pn2023: { name: 'Environment Court Practice Note 2023, Code of Conduct for Expert Witnesses (s9)', url: 'https://environmentcourt.govt.nz/assets/Practice-Note-2023-.pdf', verified: false },
    fds: { name: 'Auckland Future Development Strategy 2023–2053', url: 'https://www.aucklandcouncil.govt.nz/plans-projects-policies-reports-bylaws/Documents/future-development-strategy.pdf', verified: false },
    // Reference PDFs saved in docs/references (ref): checked against those copies
    gc2501: { name: 'NZTA General Circular 25/01: discount rates (effective 6 January 2025)', url: 'https://www.nzta.govt.nz/assets/resources/general-circulars/docs/25-01.pdf', ref: 'docs/references/25-01.pdf', verified: true,
      note: 'Checked against the saved PDF on 9 Oct 2026: non-commercial activities 2% for years 1–30, 1.5% for years 31–100 and 1% from year 101; commercial activities 8%; every transport economic analysis is also sensitivity-tested at 8% and reported with the other results.' },
    gc2601: { name: 'NZTA General Circular 26/01: MBCM v1.7.5 (effective 29 May 2026)', url: 'https://www.nzta.govt.nz/assets/resources/general-circulars/docs/26-01.pdf', ref: 'docs/references/26-01.pdf', verified: true,
      note: 'Checked against the saved PDF on 9 Oct 2026: MBCM volumes 1 and 2 v1.7.5 apply to BCR calculations that start on or after 29 May 2026; CEC 2nd edition v4 released the same day; 2025 update factors to a 1 July 2025 base date.' },
    aupMods: { name: 'Auckland Council: Plan modification schedule for the Auckland Unitary Plan (2026)', url: 'https://www.aucklandcouncil.govt.nz/content/dam/ac/docs/unitary-plan/aup-modification-schedule-2026.pdf', ref: 'docs/references/aup-modification-schedule-2026.pdf', verified: true,
      note: 'Checked against the saved PDF on 9 Oct 2026: PC79 (transport provisions: E24, E27, E38, Chapter J, Appendix 17) fully operative 8 May 2026, with a cl 20A correction to E27.3(6) the same day and no later E27 entry; PC120 notified 3 Nov 2025, Council\'s proposed amendments and partial withdrawal notified 27 Aug 2026, PC120 map viewer updated 17 Sep 2026; I450 and I451 are among the precinct chapters PC120 affects.' },
  };

  // Planning Act 2026 transition (reg-regime-stamp). specifiedTransitionDate stays null until an Order in Council.
  const REGIME = {
    assent: '2026-09-30', commence: '2026-10-30', mustBeAfter: '2027-12-31', specifiedTransitionDate: { auckland: null, other: null },
    recheck: 'Re-check against Planning Act Sch 12 Pt 2: s104(1A)-(1D), s92(2AA), s95A(9) repealed, Sch 4 cl 7(3)', source: SRC.planningAct,
  };
  const PATHWAYS = {
    'rc-landuse':     { label: 'Resource consent: land use', kind: 'rc', proc: 'consent' },
    'rc-subdivision': { label: 'Resource consent: subdivision', kind: 'rc', proc: 'consent' },
    precinct:         { label: 'Precinct staging consent (I450, I451 and similar)', kind: 'rc', proc: 'consent' },
    'pc-private':     { label: 'Private plan change', kind: 'pc', proc: 'plan' },
    'pc-council':     { label: 'Council plan change', kind: 'pc', proc: 'plan' },
    nor:              { label: 'Notice of requirement (designation)', kind: 'nor', proc: 'nor' },
    'outline-plan':   { label: 'Outline plan of works', kind: 'nor', proc: 'nor' },
    'ftaa-consent':   { label: 'Fast-track: substantive resource consent', kind: 'ftaa', proc: 'consent' },
    'ftaa-nor':       { label: 'Fast-track: notice of requirement', kind: 'ftaa', proc: 'nor' },
    nltf:             { label: 'NLTF funding (RLTP, NLTP, business case)', kind: 'fund', proc: null, law: 'Land Transport Management Act 2003 s20; MBCM v1.7.5' },
    'speed-limit':    { label: 'Speed limit change', kind: 'speed', proc: null, law: 'Land Transport Rule: Setting of Speed Limits 2024 (as at 15 Jan 2025)' },
    'dc-iff':         { label: 'Growth funding (development contributions, IFF levy)', kind: 'dc', proc: null, law: 'Local Government Act 2002 ss197AB-199; Infrastructure Funding and Financing Act 2020' },
    tou:              { label: 'Time-of-use charging scheme', kind: 'tou', proc: null, law: 'LTMA Part 2 subpart 3 (ss65A-65ZO), in force 18 Nov 2026' },
  };
  const ACTIVITY = { '': 'Not set', P: 'Permitted', C: 'Controlled', RD: 'Restricted discretionary', D: 'Discretionary', NC: 'Non-complying', U: 'Not provided for (unlisted in the zone)' };

  // Documents in the version stamp (reg-regime-stamp); ids match STANDARDS rows
  const STAMP_DOCS = [
    ['aup', 'Auckland Unitary Plan', 'July 2026 print'],
    ['at-ita', 'AT ITA Guidelines', '2025 v2'],
    ['nzta-ita', 'NZTA ITA Guide', 'G00907 July 2026'],
    ['tmdg', 'NZTA TMDG', '2019'],
    ['mbcm', 'NZTA MBCM', 'v1.7.5'],
    ['cec', 'NZTA Crash Estimation Compendium', '2nd ed v4'],
    ['speed-rule', 'Setting of Speed Limits Rule', 'as at 15 Jan 2025'],
    ['tcd', 'TCD Rule consolidation', '24 Sep 2026'],
  ];

  // Auckland governance from 7 May 2026 (reg-authority), as dated data
  const AUTH = {
    transfer: '2026-05-07', transitionalEnd: '2026-11-07', source: SRC.lgaca,
    before: { rca: 'Auckland Transport (road controlling authority)' },
    after: {
      rca: 'Auckland Council (road controlling authority, LGACA s47)',
      board: 'Local board (LGACA s47C, Sch 4: local and collector roads)',
      cco: 'Transport CCO (Auckland Transport): public transport (s43, s43A)',
    },
    nzta: 'NZ Transport Agency Waka Kotahi (state highways)', kiwirail: 'KiwiRail (rail and level crossings)',
    other: { rca: 'The district or city council (road controlling authority)' },
    reportName: 'Auckland Council (road controlling authority) and transport CCO (public transport)',
    arcf: { name: 'Auckland Roading Classification Framework (LGACA s48)', value: null, warning: 'not yet published' },
    levelCrossingM: 30,
    // AUP text that still names Auckland Transport is quoted as written, with this footnote (reg-authority, D11)
    footnote: 'since 7 May 2026 Auckland Council is the road controlling authority and Auckland Transport is the transport CCO for public transport (LGACA s47, s43A); the AUP text is quoted as written (D11).',
  };

  // AUP E27.6.1 (checked against the July 2026 print). Exceeding means more than the threshold ('exceeds').
  const E27 = {
    version: 'E27 July 2026 text — check the operative text at lodgement', source: SRC.e27,
    rows: [
      { id: 'TA1', key: 'dwellings', label: 'Residential: dwellings, threshold 1', thr: 40, unit: 'dwellings', tier: 1 },
      { id: 'T1', key: 'dwellings', label: 'Residential: dwellings, threshold 2', thr: 100, unit: 'dwellings', tier: 2 },
      { id: 'T1A', key: 'ird', label: 'Integrated residential development, threshold 1', thr: 100, unit: 'units', tier: 1 },
      { id: 'T2', key: 'ird', label: 'Integrated residential development, threshold 2', thr: 500, unit: 'units', tier: 2 },
      { id: 'T2A', key: 'visitor', label: 'Visitor accommodation, threshold 1', thr: 60, unit: 'units', tier: 1 },
      { id: 'T3', key: 'visitor', label: 'Visitor accommodation, threshold 2', thr: 100, unit: 'units', tier: 2 },
      { id: 'T3A', key: 'subdivision', label: 'Residential subdivision, threshold 1', thr: 40, unit: 'dwellings of capacity', tier: 1 },
      { id: 'T3B', key: 'subdivision', label: 'Residential subdivision, threshold 2', thr: 100, unit: 'dwellings of capacity', tier: 2 },
      { id: 'T4', key: 'primary', label: 'Education: primary', thr: 167, unit: 'students', tier: 2 },
      { id: 'T5', key: 'secondary', label: 'Education: secondary', thr: 333, unit: 'students', tier: 2 },
      { id: 'T6', key: 'tertiary', label: 'Education: tertiary', thr: 500, unit: 'students', tier: 2 },
      { id: 'T7', key: 'office', label: 'Office', thr: 5000, unit: 'm² GFA', tier: 2 },
      { id: 'T8', key: 'retailDT', label: 'Retail: drive-through', thr: 333, unit: 'm² GFA', tier: 2 },
      { id: 'T8A', key: 'retail', label: 'Retail (not drive-through)', thr: 1667, unit: 'm² GFA', tier: 2 },
      { id: 'T9', key: 'warehouse', label: 'Industrial: warehousing and storage', thr: 20000, unit: 'm² GFA', tier: 2 },
      { id: 'T10', key: 'industrial', label: 'Industrial: other', thr: 10000, unit: 'm² GFA', tier: 2 },
    ],
    catchAll: { id: 'E27.6.1(1)(b)', thr: 100, unit: 'veh/h in any hour' },
    exemptZone: /City Centre Zone|Metropolitan Centre Zone|Town Centre Zone|Terrace Housing and Apartment Building/i,
    openSpace: /^Open Space/i,
    wording: 'Exceeding E27.6.1 requires a transport assessment under E27.8.2; under AT ITA Guidelines 2025 v2 it is not by itself an ITA trigger.',
    tier1Scope: 'E27.8.1(4A) / E27.8.2(3A): effects on active modes and public transport stop infrastructure in the immediate vicinity',
    tier2Scope: 'E27.8.1(4) / E27.8.2(3): effects on the transport network for all modes, particularly at peak times',
    tier1Area: { m: 400, basis: 'Spare Construct assumption; E27.8.2(3A) says immediate vicinity' },
    s104C: 's104C: consideration and conditions limited to the matters of discretion',
    supplementary: 'Supplementary: outside E27.8.1(4A) matters of discretion',
    pc120Status: 'PC120 status (AUP modification schedule 2026): notified 3 Nov 2025; Council proposed amendments and partial withdrawal notified 27 Aug 2026; PC120 map viewer updated 17 Sep 2026. Check the site in the current PC120 viewer.',
    // land uses Spare Construct maps to Table E27.6.1.1 (LANDUSES key → quantity key); others are unlisted activities
    map: { residential: 'dwellings', apartment: 'dwellings', retirement: 'ird', hotel: 'visitor', school: 'school', tertiary: 'tertiary', office: 'office',
      retail: 'retail', supermarket: 'retail', fastfood: 'fastfood', warehouse: 'warehouse', industrial: 'industrial' },
  };
  // Precincts with their own transport provisions (editable list in Settings › Regulatory context)
  const PRECINCTS = [
    { id: 'I450', name: 'Drury Centre', match: '^Drury Centre', rule: 'I450.6(2)(b)', ita: 'I450.9(4)', source: SRC.i450 },
    { id: 'I451', name: 'Drury East', match: '^Drury East\\b', rule: 'I451.6(2)', ita: 'I451.9', source: SRC.i451 },
  ];
  // E27.2 objectives and E27.3 policies (paraphrased; quote the plan in the report), with Spare Construct's evidence sections
  const POLICY = [
    { ref: 'E27.2(1)', text: 'Land use and all modes of transport are integrated, and the adverse effects of traffic generation on the network are managed.', evidence: ['3.6 Trip generation', '4.2 Modelling'] },
    { ref: 'E27.2(2)', text: 'An integrated transport network including public transport, walking, cycling, private vehicles and freight is provided for.', evidence: ['2.4 Active modes and public transport', '4.2 Multimodal level of service'] },
    { ref: 'E27.2(5)', text: 'Pedestrian safety and amenity along public footpaths is prioritised.', evidence: ['3.2 Access', '4.3 Road user safety'] },
    { ref: 'E27.2(6)', text: 'Road/rail crossings operate safely with neighbouring land use and development.', evidence: ['3.2 Access (level crossings)'] },
    { ref: 'E27.3(1)', when: 'outside', text: 'Development generating trips with potentially more than minor effects, outside the City Centre, Metropolitan Centre, Town Centre and THAB zones and the Centre Fringe Office Control, manages its effects and integrates with the network (travel planning, alternatives to car trips, staging, local improvements).', evidence: ['4.2 Modelling', '5.1 Mitigation', '5.2 Staging triggers', '5.4 Travel plan'] },
    { ref: 'E27.3(2)', when: 'major', text: 'Major proposals for discretionary consent prepare an integrated transport assessment covering pedestrians, cyclists, public transport users, freight and motorists.', evidence: ['4.2 Modelling', '4.3 Road user safety', '4.5 Network alignment'] },
  ];

  // NZTA G00907 Table 1 base thresholds (ITA Lite / ITA Full) and Table 2 depth (from the regulatory research; verify)
  const NZTA_T1 = {
    source: SRC.nztaIta, label: 'NZTA ITA Guide G00907 July 2026',
    rows: [
      { key: 'res', label: 'Residential (incl. retirement villages and visitor accommodation)', lite: 30, full: 120, unit: 'dwellings or units' },
      { key: 'edu', label: 'Education', lite: 100, full: 300, unit: 'students' },
      { key: 'office', label: 'Office', lite: 1250, full: 4000, unit: 'm² GFA' },
      { key: 'retail', label: 'Retail (not drive-through)', lite: 500, full: 1000, unit: 'm² GFA' },
      { key: 'warehouse', label: 'Warehousing and storage', lite: 5000, full: 10000, unit: 'm² GFA' },
      { key: 'industrial', label: 'Industrial (other)', lite: 2500, full: 5000, unit: 'm² GFA' },
      { key: 'other', label: 'Other activities (busiest hour)', lite: 20, full: 100, unit: 'veh/h', fullDay: 1000 },
    ],
    driveThrough: 'ITA Full likely regardless of GFA',
    order: 'Scoping order (G00907 s4): the district plan first, then Table 1, then any council guidance. In Auckland, the AUP and AT ITA Guidelines govern scoping.',
    depth: { Basic: 'Basic', Targeted: 'Targeted', Moderate: 'Moderate', Comprehensive: 'Comprehensive' },
  };
  // Recommended assessment level from the busiest-hour development trips (Spare Construct assumption bands, D13)
  const LEVELS = ['Screening memo', 'Transport assessment', 'Integrated transport assessment (ITA)', 'ITA with a network model'];
  const LEVEL_TRIPS = { basis: ASSUMPTION, cuts: [20, 100, 500] };

  // AUP E27.6.4 access rules (checked against the July 2026 print)
  const ACCESS = {
    source: SRC.e27, intersectionM: 10, levelCrossingM: 30, nearM: 15, nearBasis: `${ASSUMPTION}: a mapped control or arterial within 15 m of the access counts as fronting it`,
    perM: { normal: 25, restricted: 50 },
    sepAdj: 2, sepSame: 6, combined: 6,
    widths: {   // [min crossing, max crossing, min formed access] in m, by zone group and parking spaces served
      residential: [{ to: 2, min: 2.75, max: 3.0, formed: 2.5, ref: 'T149' }, { to: 9, min: 3.0, max: 3.5, formed: 3.0, ref: 'T150' }, { to: Infinity, min: 5.5, max: 6.0, formed: 5.5, ref: 'T151' }],
      centres: [{ to: 9, min: 3.0, max: 3.5, formed: 3.0, ref: 'T152' }, { to: Infinity, min: 5.5, max: 6.0, formed: 5.5, ref: 'T153' }],
      business: [{ to: 9, min: 3.7, max: 4.0, formed: 3.0, ref: 'T154' }, { to: Infinity, min: 6.0, max: 7.0, formed: 6.0, ref: 'T155', hv: 9.0 }],
      rural: [{ to: Infinity, min: 3.0, max: 6.0, formed: null, ref: 'T156', hv: 9.0 }],
    },
    passing: { rural: { len: 100, width: 5.5, every: 100, ref: 'T147' }, other: { len: 50, every: 50, ref: 'T148' } },
    evidenceM: 50, evidenceBasis: `${ASSUMPTION}: bus stops, bus lanes and cycleways within 50 m`,
  };
  // State highway checks (reg-sh)
  const SH = {
    source: SRC.nztaAccess, itaLabel: 'NZTA ITA Guide G00907 July 2026', accessLabel: 'NZTA PPM Access guide 26 Sept 2025',
    // SISD (m) by design speed (km/h): Austroads formula, decision time 5.5 s, d = 0.36 — as PPM Access guide Table 8-1 uses (verify)
    sisd: { 40: 77, 50: 104, 60: 131, 70: 161, 80: 192, 90: 226, 100: 262, 110: 300, 120: 341 }, sisdAdd: 10,
    typeC: { daily: 50, hcvWeek: 1 }, design: { daily: 250, hourly: 20 }, hcvPcu: { share: 20, pcu: 3 },
    pavement: 0.20, noiseM: 100, crashYears: 10, carDays: 15,
  };
  // Operator thresholds (reg-authority): Spare Construct assumption defaults, editable in Settings
  const OPS = { dvc: 0.05, ddelay: 10, basis: ASSUMPTION };

  // Matariki Observance Day (Te Kāhui o Matariki Public Holiday Act 2022, Schedule 1; checked 9 Oct 2026)
  const MATARIKI = { 2022: '06-24', 2023: '07-14', 2024: '06-28', 2025: '06-20', 2026: '07-10', 2027: '06-25', 2028: '07-14', 2029: '07-06', 2030: '06-21', 2031: '07-11',
    2032: '07-02', 2033: '06-24', 2034: '07-07', 2035: '06-29', 2036: '07-18', 2037: '07-10', 2038: '06-25', 2039: '07-15', 2040: '07-06', 2041: '07-19', 2042: '07-11',
    2043: '07-03', 2044: '06-24', 2045: '07-07', 2046: '06-29', 2047: '07-19', 2048: '07-03', 2049: '06-25', 2050: '07-15', 2051: '06-30', 2052: '06-21' };
  // Working-day definitions: RMA s2 (20 Dec – 10 Jan) and Legislation Act 2019 s13 for the FTAA (25 Dec – 2 Jan, plus New Year Mondayisation)
  const WD = {
    rma: { name: 'RMA s2 working day', source: SRC.rma, from: [12, 20], to: [1, 10], newYear: false },
    ftaa: { name: 'Legislation Act 2019 s13 working day (FTAA)', source: SRC.legAct, from: [12, 25], to: [1, 2], newYear: true },
  };
  // Statutory clocks (reg-rfi-clock). n = working days after the event; back = working days before it.
  const CLOCKS = {
    lodged: { label: 'Application lodged', kind: 'rma', rules: [{ id: 's88(3)', n: 10, text: 'Completeness: the council may return an incomplete application within 10 working days (s88(3))' }] },
    s92: { label: 's92 further-information request received', kind: 'rma', rules: [
      { id: 's92A', n: 15, text: 'Respond within 15 working days of the date of the request: provide it, agree to provide it, or refuse (s92A(1))' },
      { id: 's92AA', after: 's92A', months: 3, text: 'Return risk: the council may treat the application as incomplete 3 months after the response deadline if there is no response (s92AA(1)(b))' }] },
    cl21: { label: 'Private plan change request received (Sch 1 cl 21)', kind: 'rma', rules: [{ id: 'cl 23(1)', n: 20, text: 'The council may require further information within 20 working days (Sch 1 cl 23(1))' }] },
    cl23: { label: 'Information provided under Sch 1 cl 23', kind: 'rma', rules: [{ id: 'cl 23(2)', n: 15, text: 'The council may require additional information within 15 working days of receiving it (Sch 1 cl 23(2))' }] },
    ftaaInvite: { label: 'FTAA comments invited (s53)', kind: 'ftaa', rules: [{ id: 's54', n: 20, text: 'Comments are due 20 working days after the invitation (s54)' }] },
    ftaaComments: { label: 'FTAA comments received', kind: 'ftaa', rules: [{ id: 's55(2)', n: 5, text: 'The applicant may respond within 5 working days (s55(2))' }] },
    ftaaS67: { label: 'FTAA s67 direction (panel request)', kind: 'ftaa', rules: [{ id: 's67', max: 10, text: 'Panel-set date, no later than 10 working days after the direction (s67(3)); information not received is treated as declined (s67(6))' }] },
    hearing: { label: 'Hearing', kind: 'rma', back: [
      { id: 's103B council', n: 15, text: 'Council officer report due 15 working days before the hearing (s103B)' },
      { id: 's103B applicant', n: 10, text: 'Applicant expert evidence due 10 working days before the hearing (s103B)' },
      { id: 's103B submitters', n: 5, text: 'Submitter expert evidence due 5 working days before the hearing (s103B)' },
      { id: 's92(3A)', n: 10, text: 's92 information or reports available 10 working days before the hearing (s92(3A))' }] },
    works: { label: 'State highway works start', kind: 'rma', back: [{ id: 'CAR', n: 15, text: 'Spare Construct access request to NZTA at least 15 working days before works (PPM Access guide s7.4)' }] },
  };

  // Standing disclaimers (docs/regulatory/_disclaimers.md, exact wording) and where each one goes (reg-disclaimers)
  const DISCLAIMERS = {
    D1: { title: 'Screening status', place: 'Title page, every report page footer and every CSV export', text: 'Spare Construct is a browser-based sketch and screening tool. It is not an approved regional transport model and it is not the Auckland Forecasting Centre Macro Strategic Model (MSM). Its results are indicative and are intended to scope, test and support a transport assessment. They must be reviewed, and where necessary confirmed with approved tools, by a suitably qualified transport planner or traffic engineer before they are relied on in any resource consent, plan change, notice of requirement, Fast-track application or funding decision.' },
    D2: { title: 'Expert evidence', place: 'Limitations section and Appendix G', text: 'Spare Construct is not an expert witness and does not give opinions. Any person who relies on these outputs in evidence remains responsible for complying with the Code of Conduct for Expert Witnesses (Environment Court Practice Note 2023, section 9), including stating the assumptions and data relied on, the uncertainty in the results, and appropriate sensitivity and uncertainty analyses for any model. This report is DRAFT until adopted and signed by a named, suitably qualified author.' },
    D3: { title: 'Junction results', place: 'Every junction table', text: 'Junction delay, degree of saturation, level of service and queue results are calculated by Spare Construct using HCM 6 / Akcelik-based methods. They are indicative and are not SIDRA output. Any junction result relied on for a consent decision, signal design (ATOC Traffic Signals Design Guidelines) or condition should be confirmed in SIDRA or another tool accepted by the road controlling authority.' },
    D4: { title: 'Economics', place: 'Every economic table and the IPM panel', text: 'The benefit-cost ratio is an indicative screening estimate following NZTA Monetised Benefits and Costs Manual v1.7.5 conventions. It is equivalent to an Indicative Efficiency Rating, not a funding-approval BCR. It is not MBCM-compliant unless the full procedures are applied to a model calibrated and validated to the NZTA Transport Model Development Guidelines and independently reviewed. IPM efficiency bands for 2027-30 are shown from NZTA\'s initial draft (August 2026) and may change after GPS 2027.' },
    D5: { title: 'ANOP', place: 'ANOP tables', text: 'ANOP levels of service are indicative Spare Construct estimates. Confirm them in Auckland Transport\'s ANOP Assessment Tool using AT speed data before relying on them.' },
    D6: { title: 'Calibration', place: 'Validation appendix', text: 'Calibration and validation statistics are reported against the NZTA Transport Model Development Guidelines (2019) for the stated model purpose category. Those criteria are not a pass/fail checklist, and meeting them does not by itself make a model fit for purpose. Fitness for purpose is a judgment for the analyst and peer reviewer and, where NZTA is involved, NZTA\'s modelling specialists.' },
    D7: { title: 'Regional consistency', place: 'Model description', text: 'The AT ITA Guidelines 2025 v2 expect detailed modelling to be consistent with regional MSM forecasts where appropriate. Where this report states that MSM consistency was not checked, the forecasts should be reconciled with MSM or Auckland Forecasting Centre outputs before lodgement for any proposal with network-wide effects.' },
    D8: { title: 'Safety', place: 'Safety section and SSA pack', text: 'Crash histories are taken from NZTA CAS and the most recent months may be incomplete. Spare Construct\'s safety screening and audit briefing pack support, but do not replace, a Safe System Audit by an independent, qualified specialist, or sight distance assessment to Austroads Guide to Road Design Part 4A.' },
    D9: { title: 'Speed limits', place: 'CBDS output', text: 'This cost benefit disclosure statement follows the structure of clause 3.3 of the Land Transport Rule: Setting of Speed Limits 2024. It is not an MBCM benefit-cost ratio and cannot be used in place of MBCM procedures for NLTF funding.' },
    D10: { title: 'Regulatory currency', place: 'Regulatory context section', text: 'Statutory references reflect the law and guidance as at [version date] for the pathway and lodgement date shown. Under the Planning Act 2026, consents lodged before 30 October 2026 are processed under the RMA as it stood immediately before 30 September 2026, and those lodged later are processed under the amended RMA as at lodgement. The specified transition date to the new planning system has not been set. This report is not legal or planning advice. Confirm the applicable provisions at lodgement.' },
    D11: { title: 'Authorities', place: 'Authority table', text: 'Since 7 May 2026 Auckland Council is the road controlling authority for Auckland roads (Local Government (Auckland Council) Act 2009 s47), with local boards exercising specified powers on local and collector roads and Auckland Transport continuing as the transport CCO for public transport. AUP provisions that still refer to Auckland Transport are quoted as written. Confirm the reviewing and approving body for this application.' },
    D12: { title: 'Hand-over', place: 'Exports', text: 'This export is prepared for use in [SIDRA / SATURN / Aimsun / AT ANOP tool / NZTA IER tool / MBCM workbook / Safe System Audit / NZTA access application]. Results produced in that tool, not the Spare Construct estimates, should be reported where the tool is required or accepted by the reviewing authority.' },
    D13: { title: 'Assumed thresholds', place: 'Any value marked Spare Construct assumption', text: 'Values marked "Spare Construct assumption" are Spare Construct defaults, not published rules. Agree them with the reviewing authority at scoping or replace them with project-specific values.' },
  };
  const DRAFT = 'DRAFT: screening output not adopted by a suitably qualified person';

  // Standards register (reg-standards): next known change dates drive supersession alerts
  const STANDARDS = [
    { id: 'rma', doc: 'Resource Management Act 1991', edition: 'As amended by the Planning Act 2026 from 30 Oct 2026 (Sch 12 Pt 2)', effective: '2026-10-30', url: SRC.rma.url, verified: false, next: { date: null, what: 'Specified transition date to the Planning Act regime: not yet set (must be after 31 Dec 2027)' } },
    { id: 'ftaa', doc: 'Fast-track Approvals Act 2024', edition: 'Version as at 10 July 2026', effective: '2026-07-10', url: SRC.ftaa.url, verified: false, next: { date: '2026-10-30', what: 'Planning Act 2026 Sch 12 Pt 3 amendments' } },
    { id: 'aup', doc: 'Auckland Unitary Plan (Operative in part)', edition: 'July 2026 print', effective: '2026-07-01', url: SRC.e27.url, verified: true, next: { date: '2027-06-30', what: 'PC120 decisions due by June 2027 (may change zoning exemptions and the I450/I451 staging tables)' },
      note: 'AUP modification schedule 2026: PC79 transport provisions fully operative 8 May 2026 and no later E27 change; PC120 amendments and partial withdrawal notified 27 Aug 2026 (map viewer updated 17 Sep 2026).' },
    { id: 'at-ita', doc: 'Auckland Transport ITA Guidelines', edition: '2025 Version 2', effective: '2025-01-01', url: SRC.atIta.url, verified: false, next: { date: null, what: 'AT guidance is moving to the Council website, subject to Council review under LGACA Sch 1AA cl 19' } },
    { id: 'nzta-ita', doc: 'NZTA PPM ITA Guide G00907', edition: 'July 2026', effective: '2026-07-01', url: SRC.nztaIta.url, verified: false, next: null },
    { id: 'nzta-access', doc: 'NZTA PPM: Access onto the state highway from private property', edition: '26 Sept 2025', effective: '2025-09-26', url: SRC.nztaAccess.url, verified: false, next: null },
    { id: 'tcd', doc: 'Land Transport Rule: Traffic Control Devices 2004', edition: 'Consolidation 24 Sep 2026', effective: '2026-09-24', url: 'https://www.nzta.govt.nz/resources/rules/traffic-control-devices-index', verified: false, next: { date: '2026-11-01', what: 'Heavy Vehicle Productivity Reform Amendment 2026 (No 2) comes into force' } },
    { id: 'tou', doc: 'Time-of-use charging (LTMA ss65A-65ZO, 2025 No 64)', edition: 'In force 18 Nov 2026', effective: '2026-11-18', url: 'https://www.legislation.govt.nz/act/public/2025/0064/latest/', verified: false, next: { date: '2026-11-18', what: 'The time-of-use provisions come into force' } },
    { id: 'ipm-2027', doc: 'Investment Prioritisation Method 2027–30', edition: 'Initial draft, August 2026', effective: '2026-08-01', url: 'https://www.nzta.govt.nz/assets/planning-and-investment/nltp/2027/docs/initial-draft-ipm-for-the-2027-30-nltp-august-2026.pdf', verified: false, next: { date: null, what: 'Final after GPS 2027 (consultation early 2027)' } },
    { id: 'ssa', doc: 'NZTA Safe System Audit Guidelines', edition: 'Interim, October 2022', effective: '2022-10-01', url: 'https://www.nzta.govt.nz/resources/safe-system-audit-guidelines/', verified: false, next: null },
    { id: 'ssi', doc: 'NZTA Safe System Intervention toolkit', edition: 'v2, September 2021', effective: '2021-09-01', url: 'https://www.nzta.govt.nz/', verified: false, next: null },
    { id: 'tmdg', doc: 'NZTA Transport Model Development Guidelines', edition: '2019 (1 Sept 2019)', effective: '2019-09-01', url: 'https://www.nzta.govt.nz/assets/resources/transport-model-development-guidelines/docs/tmd.pdf', verified: false, next: null },
    { id: 'mbcm', doc: 'NZTA Monetised Benefits and Costs Manual', edition: 'v1.7.5, volumes 1 and 2 (BCRs started on or after 29 May 2026, GC 26/01)', effective: '2026-05-29', url: 'https://www.nzta.govt.nz/resources/monetised-benefits-and-costs-manual/', verified: true, next: null, note: SRC.gc2601.note },
    { id: 'gc2501', doc: 'NZTA General Circular 25/01: discount rates', edition: '6 January 2025', effective: '2025-01-06', url: SRC.gc2501.url, verified: true, next: null, note: SRC.gc2501.note },
    { id: 'cec', doc: 'NZTA Crash Estimation Compendium', edition: '2nd edition v4 (released 29 May 2026, GC 26/01)', effective: '2026-05-29', url: 'https://www.nzta.govt.nz/resources/crash-estimation-compendium/', verified: true, next: null, note: 'The MBCM page link text "version 3" is wrong; v4 is current (GC 26/01).' },
    { id: 'speed-rule', doc: 'Land Transport Rule: Setting of Speed Limits 2024', edition: 'As at 15 January 2025', effective: '2025-01-15', url: 'https://www.nzta.govt.nz/assets/resources/rules/docs/land-transport-rule-setting-of-speed-limits-2024-as-at-15-january-2025.pdf', verified: false, next: null },
    { id: 'arcf', doc: 'Auckland Roading Classification Framework (LGACA s48)', edition: 'Not yet published', effective: null, url: SRC.lgaca.url, verified: false, next: { date: null, what: 'Publication pending; local-board powers on local and collector roads depend on it' } },
  ];
  const PLANNING_ONLY = 'Spare Construct makes planning-level checks only against design standards; it does not certify designs.';

  /* What the reference PDFs in docs/references say about an item (the item text stays verbatim from _checklist.md).
     FUND-04: GC 25/01 asks for the 8% test on every transport economic analysis; the checklist's 'for full
     procedures' comes from the MBCM v1.7.5 research and was not checked against the manual. */
  const REF_NOTES = {
    'RC-01': 'AUP modification schedule 2026: PC120 amendments and partial withdrawal notified 27 Aug 2026; PC120 map viewer updated 17 Sep 2026.',
    'RC-02': 'AUP modification schedule 2026: PC79 fully operative 8 May 2026; no later E27 change listed.',
    'FUND-03': 'GC 26/01: MBCM v1.7.5 applies to BCRs started on or after 29 May 2026; update factors to a 1 July 2025 base date; CEC 2nd edition v4 released 29 May 2026.',
    'FUND-04': 'GC 25/01: 2% for years 1–30, 1.5% for years 31–100, 1% from year 101 (non-commercial; commercial 8%); it asks for an 8% sensitivity test on every transport economic analysis, reported with the results. Check the MBCM v1.7.5 s1.9 wording.',
    'FUND-08': 'GC 26/01: CEC 2nd edition v4 released 29 May 2026 with MBCM v1.7.5; crash procedures moved into MBCM volume 1.',
  };
  // The 87-item regulatory checklist (docs/regulatory/_checklist.md): [id, pathway, item, source, fills, how, evidence]
  const CHECKLIST = [
    ['X-01', 'All', 'Record the pathway, the lodgement date (planned or actual) and the statutory version the assessment is written against (pre-assent RMA, transition RMA, FTAA as when made)', 'Planning Act 2026 Sch 1 cl 1-4, 18-21; FTAA Sch 1 cl 16-24 (inserted by Planning Act Sch 12 Pt 1)', 'partial', 'the planner enters pathway and date; Spare Construct derives and stamps the regime and document versions', 'Regime and version stamp on the title page and in table footers'],
    ['X-02', 'All (Auckland)', 'Name the correct reviewer and approver: Auckland Council as road controlling authority; the local board for local and collector roads; the transport CCO (AT) for PT; NZTA for state highways; KiwiRail for rail. Name the requiring authority correctly after the transfer date', 'LGACA 2009 ss43, 43A, 47, 47A-47D, Sch 4, Sch 1AA cl 3, 8, 10 (2026 No 15, in force 7 May 2026)', 'partial', 'a per-link authority resolver; the planner confirms whether a cl 8(2) direction has ended the transitional period', 'Authority table and an affected-operator list'],
    ['X-03', 'All RMA and FTAA', 'State why the depth of transport assessment is proportionate to the scale and significance of effects', 'RMA s88(2AA)-(2AB) (from 20 Oct 2025), Sch 4 cl 1(2), 2(3)(c); s18B (from 30 Oct 2026); FTAA s44', 'partial', 'Spare Construct drafts the paragraph from screening metrics (trips, % link change, crash flags, state highway involvement); the author confirms it', 'Proportionality statement in section 1'],
    ['X-04', 'All', 'A suitably qualified, named author adopts the assessment and complies with the expert Code of Conduct', 'AUP E27.9(4); Environment Court Practice Note 2023 s9.3', 'manual', 'Spare Construct provides the sign-off block and disclosure appendix', 'Qualifications, sign-off, Code of Conduct statement'],
    ['X-05', 'All', 'Identify the data, facts, assumptions and literature relied on, with dates and sources; supply all model input data', 'Practice Note 2023 s9.3(a)(ii)-(v); AT ITA Guidelines 2025 v2 s4.2', 'auto', 'provenance register and full input export', 'Assumptions register, data vintage table, input data pack'],
    ['X-06', 'All modelled evidence', 'Run sensitivity and uncertainty analyses on trip generation, mode share, assignment and background growth. Run a funded-only network if none are run, and a no-PT-uplift case where PT improvements are assumed', 'Practice Note 2023 s9.3(a)(x); AT ITA v2 s3.6.1(v), Appendix 1; RMA s32(2)(c)', 'auto', 'sensitivity battery with tipping points', 'Sensitivity table and tornado chart'],
    ['X-07', 'All modelled evidence', 'Report model calibration and validation against the agreed TMDG purpose category (A-G). Do not present meeting the criteria as proof that the model is fit for purpose', 'NZTA Transport Model Development Guidelines (1 Sept 2019) s5.3-5.8, Tables 1-5', 'auto', 'GEH bands, R² and slope, %RMSE, journey times against the category; the author justifies the category', 'Validation appendix and a model-status badge on every output'],
    ['X-08', 'ITAs in Auckland', 'Make detailed modelling consistent with regional MSM forecasts where appropriate; scope with Council and AT (and NZTA)', 'AT ITA v2 s4.2.1.1, s3.6 (MSM may be used for large developments)', 'partial', 'growth comparison if MSM, AFC or AGS23 data are loaded; otherwise \'not checked\'', 'MSM consistency statement and growth comparison table'],
    ['X-09', 'All (Auckland)', 'Agree scope early: study area, periods, data, trip-rate method, committed developments, tools', 'AT ITA v2 \'Scoping an ITA\', \'Early Engagement\', s3.6.1(iii)', 'partial', 'Spare Construct produces the scoping memo; the planner records the agreement', 'Scoping memo and engagement log'],
    ['RC-01', 'Resource consent (Auckland)', 'Identify zone, controls and precinct under operative and PC120-proposed maps before testing E27.6.1', 'AUP E27.6.1(2), E27.4 preamble (July 2026); Record of AUP modifications 2026 (PC120 text affected)', 'auto', 'AUP GIS lookup, with a dual-layer warning where PC120 proposes a change', 'Zoning and precinct statement with map'],
    ['RC-02', 'Resource consent (Auckland)', 'Test Table E27.6.1.1 thresholds, apply the exemptions, and report tier 1 (TA1, T1A, T2A, T3A only) or tier 2', 'AUP Table E27.4.1 (A3), E27.6.1(1)(a), (2), Table E27.6.1.1 (PC79 operative 8 May 2026)', 'auto', 'from land-use inputs', 'E27 trigger table (Rule / Threshold / Quantity / Exceeded / Tier / Consequence)'],
    ['RC-03', 'Resource consent (Auckland)', 'Apply the 100 veh/h catch-all only to unlisted activities that need controlled or RD consent where the zone has no transport assessment requirement. Flag that an ITA \'may\' be needed for unprovided or non-complying activities at 100 veh/h or more', 'AUP E27.6.1(1)(b), E27.9(5)', 'partial', 'Spare Construct computes the busiest-hour two-way volume; the planner sets activity status', 'Peak-hour generation with rate source'],
    ['RC-04', 'Resource consent (Auckland)', 'Scope the assessment to the matters of discretion: tier 2 all-mode network at peak times, mitigation and trip characteristics; tier 1 active modes and PT stop infrastructure in the immediate vicinity', 'AUP E27.8.1(4), (4A), E27.8.2(3), (3A); RMA s104C(1), (3)', 'partial', 'Spare Construct produces the network outputs; the vicinity audit (footpaths, crossings, stop amenity) is a site task', 'Assessment scoped to the matters of discretion'],
    ['RC-05', 'Resource consent (Auckland)', 'Assess against the E27.2 objectives and E27.3 policies. Under E27.3(2), major discretionary proposals prepare an ITA covering pedestrians, cyclists, PT users, freight and motorists', 'RMA Sch 4 cl 2(1)(g), 2(2); AUP E27.2, E27.3(1)-(2)', 'partial', 'Spare Construct auto-fills a policy matrix with links to its evidence; the planner writes the evaluation', 'Objectives and policies table'],
    ['RC-06', 'Resource consent', 'AEE transport content: actual or potential effects, mitigation, affected persons and consultation, monitoring, and alternatives if a significant adverse effect is likely. Put safety under cl 6(1)(b) and 7(1)(a), not cl 7(1)(f)', 'RMA Sch 4 cl 6(1)(a), (b), (e), (f), (g), 7(1)(a), (b), (e)', 'partial', 'base vs with-development vs mitigated comparison, and a monitoring template; consultation is manual', 'AEE transport section'],
    ['RC-07', 'Resource consent', 'Define the receiving environment: existing network plus committed projects plus granted consents likely to be implemented. Permitted baseline optional', 'RMA s104(1)(a), s104(2); Queenstown Lakes DC v Hawthorn Estate [2006] NZRMA 424 (CA)', 'partial', 'the register is entered or imported by the planner; scenarios are built automatically', 'Committed-development register with inclusion reasons'],
    ['RC-08', 'ITAs', 'Include cumulative enabled or consented development. Include only planned and funded projects in future networks unless sensitivity-tested. Assess construction, interim years and full build-out', 'AT ITA v2 s3.6.1, s4.2, s4.2.1; NZTA ITA Guide G00907 s5.4', 'auto', 'scenario matrix with funding-status gating', 'Scenario matrix'],
    ['RC-09', 'ITAs (Auckland)', 'Set the study area to include arterial, rapid or frequent transit and freight routes with demand increase over 5%, and collectors with increase over 10%', 'AT ITA v2 s4.2.1.1', 'auto', 'extent test using Future Connect classes', 'Extent table and map'],
    ['RC-10', 'ITAs', 'Generate person trips first, then mode share, distribution and timing. Source and date every rate; review pre-2020 and 2020-22 rates; give evidence for any WFH or TDM discount; estimate freight separately; justify periods covering both development and network peaks', 'AT ITA v2 s3.6.1(i)-(vi), Appendix 1', 'partial', 'Spare Construct computes and warns; rate choice and justification are the author\'s', 'Trip generation tables T2-T5'],
    ['RC-11', 'ITAs (large developments)', 'Multimodal LOS: PT LOS and delay, PT reach within 400 m-1.2 km walk, pedestrian and cycle LOS, freight on Future Connect routes, queue safety. ANOP tool recommended where any mode\'s LOS is affected', 'AT ITA v2 s4.1, s4.2.1-4.2.3; ANOP Application v1 (July 2025)', 'partial', 'bus delay, catchments and ANOP speed/reliability bands; active-mode LOS needs a site audit', 'Multimodal LOS tables and ANOP inputs'],
    ['RC-12', 'ITAs', 'Review the latest 5 years of CAS data near the site and for each altered intersection or access, using a Safe System risk approach. Assess safety effects of the extra trips separately', 'AT ITA v2 s4.3.2-4.3.4', 'auto', 'CAS extract, DSI and risk metrics; the author writes the narrative', 'Crash tables CR1-CR3 and Safe System commentary'],
    ['RC-13', 'ITAs', 'Assess sight distance for new or altered accesses, intersections and crossings to Austroads GRD Part 4A at operating speed, allowing for pending speed-limit changes', 'AT ITA v2 s4.3.1; AT Practice Note 09 ed. 1.1 (Apr 2026); TDM EDC Roadway v2 s4.1', 'partial', 'calculation from NSLR or modelled speed; the site survey is manual', 'Sight distance calculation'],
    ['RC-14', 'ITAs with infrastructure to vest', 'Confirm whether a stage 1 or 2 Safe System Audit is needed. It is done by an independent qualified specialist', 'AT ITA v2 s4.3.5; NZTA Safe System Audit Guidelines (interim, Oct 2022)', 'manual', 'Spare Construct exports a briefing pack only', 'SSA report with designer responses, or a confirmation it is not required'],
    ['RC-15', 'Resource consent (Auckland)', 'Parking: supply against the maximums (no minimums), accessible parking including the E27.6.3.2A(4) activity and zone list, and induced peak trips where a maximum is exceeded', 'AUP E27.6.2(1), Tables E27.6.2.1-E27.6.2.4, E27.6.3.2A(1)-(4), E27.8.1(5), E27.8.2(4)(c); NPS-UD Policy 11, cl 3.38', 'partial', 'rate checks are automatic; the induced-trip estimate is labelled indicative', 'Parking compliance table P1'],
    ['RC-16', 'Resource consent (Auckland)', 'Loading spaces by activity and GFA. One small loading space for 10+ dwellings with arterial-only access and no individual pedestrian access', 'AUP E27.6.2(8), Tables E27.6.2.7, E27.6.2.7A, E27.8.1(8)', 'auto', '', 'Loading row in table P1'],
    ['RC-17', 'Resource consent (Auckland)', 'Vehicle access restrictions: within 10 m of an intersection, mapped VAR controls, arterial frontage, within 30 m of a level crossing; City Centre and Key Retail Frontage new crossings non-complying unless excepted', 'AUP E27.6.4.1(1)-(3), Table E27.6.4.1.1, Table E27.4.1 (A5), (A7), (A8); E27.8.2(11)', 'auto', 'geometry and AUP layers; the pedestrian count is a field input', 'Access compliance table and E27.8.2(11) evidence'],
    ['RC-18', 'Resource consent (Auckland)', 'Crossing number, separation and width per frontage; passing bays; queues at gates, drive-throughs and dispensers must not reach the road reserve', 'AUP E27.6.4.2, E27.6.4.3(1)-(2), Tables E27.6.4.2.1, E27.6.4.3.1, E27.6.4.3.2, E27.8.1(9)', 'partial', 'queue from the junction or microsimulation module; widths and stacking length are entered', 'Queue vs stacking table'],
    ['RC-19', 'Resource consent (Auckland)', 'A travel plan where the trip threshold or parking maximums are exceeded (if council requires it)', 'AUP E27.9(2)', 'partial', 'template with modelled mode shares; targets and measures are the author\'s', 'Travel plan'],
    ['RC-20', 'ITAs', 'Mitigation table: measure, problem, options including non-infrastructure, cost, funder, RLTP/LTP status, trigger, mechanism, concept drawing, implementation plan. Uncommitted or third-party measures get \'limited consideration\'', 'AT ITA v2 s3.4, s5.1, s5.2, Appendix 3', 'partial', 'the effect link and trigger come from the model; costs and drawings are manual', 'Mitigation tables M1 and M2'],
    ['RC-21', 'ITAs', 'Construction and temporary disruption effects, with a draft CTMP or TMP, detour modelling and PT diversions', 'AT ITA v2 s3.5', 'partial', 'temporary-network scenario; the CTMP is manual', 'Construction traffic section and draft CTMP'],
    ['RC-22', 'Resource consent and NoR lodged from 30 Oct 2026', 'Consider consistency with the FDS 2023-2053 (or a decided regional spatial plan): location, timing and reliance on FDS-assumed infrastructure. For earlier lodgements, address out-of-sequence development as policy context', 'RMA s104(1D), s168A(3C), s171(1D) (Planning Act Sch 12 Pt 2); AT ITA v2 s5.1 \'Out of Sequence Development\'', 'partial', 'FDS layer comparison once digitised; funding narrative is manual', 'FDS consistency paragraph'],
    ['RC-23', 'District land use consents lodged from 30 Oct 2026', 'Leave out effects listed in s104(1A) (retail distribution, trade competition, lack of demand or viability, views, residential design matters). Do not rely on Planning Act or NEA instruments', 'RMA s104(1A)-(1C), s92(2AA), s95D(f), s95E(2)(d), Sch 4 cl 7(3)', 'auto', 'text linter', 'Clean scope; linter log'],
    ['RC-24', 'Resource consent', 'Identify affected network operators and the scale of effect on their assets, as input to notification advice. For applications lodged from 30 Oct 2026, AUP cross-references to s95A(9) have no target; s95B(10) remains', 'AUP C1.13(4), E27.5(1), (2), (5); RMA s95B(10), s95E; Planning Act Sch 12 Pt 2', 'auto', 'operator list from links and junctions over an editable change threshold (Spare Construct assumption)', 'Affected-operator table with map'],
    ['RC-25', 'Urban subdivision (Auckland)', 'Safe, legible access; walkable connected layout; effects of significant traffic increase on existing roads; rear-site access widths, lengths and pedestrian access', 'AUP E38.12.2(a), (e), (g), (k); E38.8.1.2, Table E38.8.1.2.1; E27 rows T3A/T3B', 'partial', 'yield to trips, link change and connectivity metrics; layout review is manual', 'Subdivision transport section'],
    ['RC-26', 'ITAs', 'Show alignment with Future Connect, ANOP, RASF, FDS, RLTP and structure or precinct plans; highlight inconsistencies; conclude on alignment across all modes', 'AT ITA v2 \'Statutory and Policy Framework\', s4.1, s4.4, s6; RASF v2 (not mandated for developers)', 'partial', 'layer overlays and draft text; the conclusion is the author\'s', 'Network alignment section'],
    ['RC-27', 'ITAs with assets to vest', 'Climate adaptation: resilience of relied-on and vested assets to hazards over their life', 'AT ITA v2 s4.5; NPS-UD Objective 8, Policies 1(f), 6(c)', 'partial', 'hazard overlay on relied-on links', 'Climate adaptation table'],
    ['PS-01', 'Precinct staging (I450/I451)', 'Cumulative approved plus proposed dwellings and GFA against the staging-table caps, with prerequisite upgrade status; every application states its enabled yield', 'AUP I450.6.2(1)-(4), Table I450.6.2.1, I450.9(5); I451.6.2, Table I451.6.2.1, I451.9', 'partial', 'the register is planner-maintained; the cap test and activity status are automatic', 'Precinct register and cap table'],
    ['PS-02', 'Precinct staging (I450/I451)', 'Infringing applications: inter-peak LoS \'generally no worse than\' D at Waihoehoe Rd / Great South Rd overall; trip generation similar to or less than the table mix; PT operation; construction traffic', 'AUP I450.9(4), I451.9', 'auto', 'LoS and trip-equivalence tests; road controlling authority engagement is manual', 'Precinct ITA tables'],
    ['PS-03', 'Precinct staging (I450/I451)', 'Where an upgrade is under construction or applicant-delivered, agree under s108AA(1)(a), on an Augier basis, to conditions barring occupation or s224(c) until it operates', 'AUP I450.6.2(2); RMA s108AA(1)(a)', 'manual', 'Spare Construct flags when this is needed', 'Applicant agreement letter'],
    ['PC-01', 'Plan change', 's32: options, efficiency and effectiveness, benefits and costs quantified where practicable, risk of acting or not acting', 'RMA s32(1)-(2)', 'partial', 'option comparison with indicative monetisation; the evaluation is the planner\'s', 's32 transport table'],
    ['PC-02', 'Private plan change', 'Request content including effects taking into account Sch 4 cl 6-7; respond to cl 23 requests', 'RMA Sch 1 cl 22, 23(1)-(6), 25', 'partial', 'cl 23 response pack from versioned runs', 'Plan-change ITA and cl 23 responses'],
    ['PC-03', 'Plan change', 'Assume full zoned build-out; staged upgrades with effects-based triggers; road function table; monitoring', 'AT ITA v2 scoping table, s3.4, s3.6.1', 'auto', 'trigger finder; road function table is a draft', 'Staging trigger table (I450 style)'],
    ['PC-04', 'Plan change', 'Check process timing: council plan stop to the specified transition date (exemptions s80U-80W); private plan change notification bar from 1 Jan 2028 (s80YA)', 'RMA ss80O-80W, 80YA (Planning Act Sch 12 Pt 2)', 'auto', 'date check against the entered notification date', 'Timing note'],
    ['PC-05', 'Plan change', 'Have regard to the RLTP and the emissions reduction plan (ERP2, which sets no VKT target); give effect to the NPS-UD (Policy 1 accessibility and emissions)', 'RMA s74(2)(b)(i), (d), s75(3); ERP2 (amended Jan 2026) ch 9; NPS-UD Policy 1', 'partial', 'VKT, CO2e and accessibility outputs, labelled as information', 'Policy alignment table'],
    ['NOR-01', 'Notice of requirement', 'AEE with alternatives and reasonable necessity where the requiring authority lacks a sufficient interest in the land; otherwise alternatives only if a significant adverse effect is likely', 'RMA s168(3A)-(3C), s171(1)(b) (s171(1)(c) repealed 21 Aug 2025)', 'partial', 'multi-option comparison; the land-interest status is entered', 'Alternatives assessment'],
    ['NOR-02', 'Notice of requirement', 'Benefits of infrastructure, identification in strategic planning documents (RLTP, FDS), provider\'s business-case material, best-practice methods, proportionate conditions', 'NPS-I 2025 Policies 1, 3, 4, 7 (in force 15 Jan 2026)', 'partial', 'benefits metrics and RLTP references; business-case material is manual', 'NPS-I section'],
    ['NOR-03', 'Notice of requirement', 'Forecast to horizons covering the 10-year designation lapse', 'RMA s184(1)', 'auto', '+10/+20/+30-year scenarios from AGS23 or FDS growth', 'Long-horizon forecast tables'],
    ['FT-01', 'Fast-track', 'AEE to FTAA Sch 5 cl 6-7, an assessment against plan objectives, policies, rules and standards (including E27), and proposed conditions', 'FTAA Sch 5 cl 5(1)(g)-(h), (k), (2)-(4), (4A), cl 6(2), cl 7', 'partial', 'compliance matrix and conditions parameters; the planner finalises', 'Fast-track transport package'],
    ['FT-02', 'Fast-track', 'Quantify regional or national benefits against residual adverse transport impacts, with and without conditions', 'FTAA s81(4), s85(3)-(4), Sch 5 cl 17', 'partial', 'benefit metrics and indicative BCR next to worst residual effects', 'Benefits vs adverse impacts table'],
    ['FT-03', 'Fast-track', 'Staging and infrastructure-adequacy triggers that can be conditioned on the applicant only; check the 2-year default lapse', 'FTAA s84A, s43(2)(b), s81(5), Sch 5 cl 26', 'auto', 'capacity-trigger solver', 'Staging and condition table'],
    ['FT-04', 'Fast-track', 'Reviewer pack ready for s53 invitees (local authority, requiring authorities with adjacent designations such as NZTA and KiwiRail) within 20 working days; able to answer s67 requests within a panel-set date of up to 10 working days', 'FTAA s53(2)(l), (2A), s54, s55(2), s56, s67(3), (6)', 'auto', 'bundle export and rapid re-run checklist', 'Reviewer pack and RFI log'],
    ['FT-05', 'Fast-track NoR', 'Effects and mitigation, reasonable necessity, alternatives considered, consultation, proposed conditions', 'FTAA Sch 5 cl 12(1)', 'partial', 'option comparison', 'NoR transport assessment'],
    ['FT-06', 'Fast-track', 'Flag that GRPA crossing-place and s93 approvals, and corridor access approvals, sit outside Fast-track', 'FTAA s42(4); GRPA ss91, 93', 'auto', 'flag when a state highway access or LAR is involved', 'NZTA engagement checklist'],
    ['SH-01', 'NZTA state highway', 'Identify NZTA trigger cases and seek written approval under s95E(3)(a)', 'NZTA PPM Access guide (26 Sept 2025) s7.2; RMA s95E', 'auto', 'intersect site, accesses and assigned trips with state highway links', 'State highway impact table (veh/day, veh/h, HCV % by link and intersection)'],
    ['SH-02', 'NZTA state highway', 'Requiring authority consent for works on designated land that would prevent or hinder the designated work', 'RMA s176(1)(b), s178', 'auto', 'flag from the designation layer; a \'prevent or hinder\' assessment is manual', 's176 consent'],
    ['SH-03', 'NZTA state highway', 'LAR crossing place notice (s91), s93 approval before s224(c), motorway special access (s84)', 'GRPA 1989 ss84, 85, 88-93 (as at 19 May 2026); PPM Access guide s7.3', 'auto', 'flag from the LAR layer, plus an alternative local-road access test', 'NZTA authorisation'],
    ['SH-04', 'NZTA state highway', 'Access design: SISD, intersection and access spacing, access type, PCU adjustment above 20% HCV, 10-year crash history', 'PPM Access guide s8, s8.1, Tables 8-1 to 8-3, 9-1', 'auto', 'from posted or measured speed and modelled access volumes; DSI contour figures need digitising', 'Access technical table'],
    ['SH-05', 'NZTA state highway', 'Scope by the district plan or council guidance first, then NZTA Table 1 ITA Lite/Full and Table 2 depth', 'NZTA PPM ITA Guide G00907 (July 2026) s4, s4.1 Table 1, s5.4 Table 2', 'auto', '', 'Scope statement'],
    ['SH-06', 'NZTA state highway', 'Pavement impact assessment where daily heavy-vehicle trips rise by more than 20% on affected roads; light VKT reported', 'NZTA ITA Guide s5.4.3, s5.4.6', 'auto', 'per-link heavy-vehicle change and VKT', 'Heavy-vehicle and VKT table'],
    ['SH-07', 'NZTA state highway', 'Noise and reverse sensitivity for noise-sensitive activities near a state highway (effects areas up to 100 m)', 'NZTA PPM Managing effects (26 Sept 2025) s5.2.2', 'partial', 'buffer flag; acoustic design is manual', 'Noise flag'],
    ['SH-08', 'NZTA state highway', 'Spare Construct access request at least 15 working days before works', 'PPM Access guide s7.4', 'manual', '', 'Spare Construct access request and ATWOSH'],
    ['RCA-01', 'Road controlling authority (Auckland)', 'Signal design submission: counts, base and future models per option, delay, LOS and queue lengths, design-life sensitivity, phasing and timings', 'ATOC TSDG v5.0 ss2.3-2.5, s3.10', 'partial', 'export in s2.5 layout with queue statistic labelled; final design in approved tools', 'Signal design report'],
    ['RCA-02', 'Road controlling authority (Auckland)', 'Assets to vest comply with the current TDM and Practice Notes; departures listed', 'AT ITA v2 s3.2; AT TDM page', 'manual', 'Spare Construct keeps a standards register only', 'Compliance statement naming versions'],
    ['FUND-01', 'NLTF funding', 'Options and alternatives assessed against a do-minimum, including demand and optimisation options; GPS consistency; efficiency and effectiveness', 'LTMA s20(2); MBCM v1.7.5 s1.5 (intervention hierarchy)', 'partial', 'option sets on identical inputs', 'Options assessment'],
    ['FUND-02', 'NLTF funding', 'RLTP activity information: cost by year, duration, funding sources, objective, monitoring measures', 'LTMA s16(3)(d)-(f), (4); LGACA s38A', 'partial', 'sheet from cost and phasing inputs', 'RLTP activity sheet'],
    ['FUND-03', 'NLTF funding', 'State MBCM version, time zero, base date, update factors, CEC version, and procedure (full or simplified)', 'GC 26/01 (29 May 2026); MBCM vol 1 v1.7.5 s1.6', 'auto', '', 'Economic settings block'],
    ['FUND-04', 'NLTF funding', 'Discount at 2% (years 1-30) and 1.5% (from year 31). 8% sensitivity required for full procedures. P95 cost test; P95 estimate mandatory for total cost of $10m or more', 'MBCM vol 1 v1.7.5 s1.3, s1.9, s7.2, ch 5; GC 25/01; Minimum information for improvements s9.6', 'auto', 'P95 cost is entered', 'Sensitivity table'],
    ['FUND-05', 'NLTF funding', 'BCR_N and NPV for every shortlisted option, FYRR for the preferred option, BCR_G where there is third-party funding; exclude transfers, GST, residual values and land-value uplift', 'MBCM vol 1 v1.7.5 s1.7, s1.8, s1.10, s6.1-6.2', 'auto', '', 'Economic summary'],
    ['FUND-06', 'NLTF funding', 'Incremental BCR analysis for mutually exclusive options, with target from Table 104 and a +1.0 target test', 'MBCM vol 1 v1.7.5 s6.3, Table 104', 'auto', '', 'Incremental BCR ladder'],
    ['FUND-07', 'NLTF funding', 'Choose a fixed or variable trip matrix (VTM probable for large road schemes in major centres); rule of half; RR717 screening for new roads at early stages', 'MBCM vol 1 v1.7.5 s2.2-2.5, s2.13, Table 4', 'partial', 'Spare Construct warns; the justification is the author\'s', 'Demand method statement'],
    ['FUND-08', 'NLTF funding', 'Crash analysis from CAS: 5 years normally (10 years ÷ 2 below 1,500 vpd), method A, B or C, CEC 2nd edition v4', 'MBCM vol 1 v1.7.5 s3.1, Tables 6-7; CEC 2nd ed v4 (May 2026)', 'auto', '', 'Crash benefit table'],
    ['FUND-09', 'NLTF funding', 'Efficiency rating with WEBs excluded at stage 1; BCR rounding; supporting calculations to avoid L*', '2024-27 IPM p9, p27; initial draft IPM 2027-30 pp8-11, 32-33; Minimum information for improvements p10', 'auto', 'dual band display, with the draft labelled', 'IPM profile'],
    ['FUND-10', 'NLTF funding (2027-30)', 'Stage 2 quantitative GPS metrics: freight and general traffic travel time or reliability change at network level, minutes to key destinations, houses enabled, DSI reduction per $100m', 'Initial draft IPM 2027-30 Appendix 1 pp24-29', 'partial', 'modelled metrics; destination significance and housing classes are entered', 'GPS stage 2 metrics table'],
    ['FUND-11', 'NLTF funding', 'Model documentation: Transport modelling checks worksheet per period; TMDG validation for the purpose category', 'MBCM vol 1 v1.7.5 s4.1, s4.3; TMDG', 'partial', 'pre-filled worksheet and validation report; peer review is manual', 'Completed worksheet'],
    ['FUND-12', 'NLTF funding', 'Safe System audits at key stages for NLTF improvement and renewal activities, attached to the TIO application', 'NZTA 2024-27 NLTP Safe System Audit Policy (June 2024)', 'manual', 'Spare Construct exports an audit pack', 'Audit report with responses'],
    ['FUND-13', 'NLTF funding (2027-30)', 'Programme Investment and LCLR eligibility: activities under $50m, BCR above 1, caps per activity class, no splitting', 'NZTA Programme Investment guidance (6 Jul 2026) pp4, 7, 9; LCLR guidance v1 s5.1, 6', 'auto', 'flags', 'Programme schedule'],
    ['SPD-01', 'Speed limit change', 'CBDS per road in cl 3.3 structure; not for new roads or school-gate limits; updated if the limit changes after consultation', 'Setting of Speed Limits Rule 2024 cl 3.2, 3.3; NZTA Rule guidance s5', 'partial', 'crashes, speeds and journey time are automatic; implementation cost is entered', 'CBDS table'],
    ['SPD-02', 'Speed limit change', 'Limit within the Schedule 3 range for the Rule class; Schedule 1 minimum length; 110/120 for NZTA only; variable limit reason', 'Setting of Speed Limits Rule 2024 cl 4.3-4.9, Schedules 1 and 3', 'auto', 'the Rule class is entered per link (ONF offered only as a suggestion)', 'Speed validation table'],
    ['SPD-03', 'Speed limit change', 'School variable limits and travel windows near schools', 'Setting of Speed Limits Rule 2024 cl 1.4, 5.1-5.4', 'auto', 'from NSLR variable records', 'School speed layer'],
    ['SPD-04', 'Speed limit change (Auckland)', 'Route local and collector road speed decisions to the local board', 'LGACA s47C, Sch 4', 'auto', 'via the authority resolver', 'Decision-maker field'],
    ['DC-01', 'Growth funding', 'Causation and beneficiary evidence for DCs, including the 6-month policy window for Fast-track approvals', 'LGA ss102A(3), 197AB, 198B, 199, 199AA, Sch 1AA cl 71', 'partial', 'select-link and select-zone shares; legal allocation is the council\'s', 'Causation and beneficiary table'],
    ['DC-02', 'Growth funding', 'IFF levy: benefits inside and outside the levy area and contribution to need', 'IFF Act 2020 ss8, 18, 18A', 'auto', 'benefit incidence for a user-drawn polygon', 'Benefit incidence table and BCR_G'],
    ['TOU-01', 'Time-of-use charging', 'Representative travel times and volumes by charging area; network, PT and distributional impacts; monetised summary', 'LTMA ss65D(1), (3), 65ZD (2025 No 64, in force 18 Nov 2026)', 'partial', 'screening only; scheme approval needs full modelling', 'ToU impact assessment tables'],
    ['EV-01', 'Resource consent and plan change', 'Track further-information deadlines: s92A 15 working days; s92AA 3-month return risk; s88(3) 10 working-day completeness; Sch 1 cl 23 20 and 15 working days. Respond citing s92(2B) proportionality and s92(2AA) where relevant', 'RMA s88(3), s92(2B), (2AA), s92A, s92AA; Sch 1 cl 23', 'auto', 'working-day calculator; the response is the author\'s', 'RFI log'],
    ['EV-02', 'Hearings', 'Evidence deadlines: council 15, applicant 10, submitter experts 5 working days before the hearing; s92 material 10 working days before', 'RMA s103B, s92(3A)', 'auto', '', 'Hearing timeline and frozen evidence snapshot'],
    ['EV-03', 'Hearings and conferencing', 'Will-say and JWS: key facts, assumptions, methods, agreed and disagreed issues with reasons', 'Practice Note 2023 s9.4(f)-(i), s9.5', 'partial', 'assumption diff and per-difference runs', 'JWS input table'],
    ['EV-04', 'Conditions (RMA and FTAA)', 'Each transport condition tied to an effect or rule, or agreed by the applicant; measurable trigger, survey method and timing; review clause where needed', 'RMA s108(3)-(4), s108AA(1), s128(1)(a); Practice Note s10.4; FTAA s83, s84A', 'partial', 'condition parameters; the wording is the planner\'s', 'Draft conditions schedule'],
  ].map(([id, pathway, item, source, fills, how, evidence]) => ({ id, pathway, item, source, fills, how, evidence, group: id.split('-')[0], refNote: REF_NOTES[id] || '' }));
  const CK_GROUPS = { X: 'Cross-cutting', RC: 'Resource consent and ITA', PS: 'Precinct staging', PC: 'Plan change', NOR: 'Notice of requirement', FT: 'Fast-track', SH: 'State highways (NZTA)',
    RCA: 'Road controlling authority', FUND: 'NLTF funding', SPD: 'Speed limits', DC: 'Growth funding', TOU: 'Time-of-use charging', EV: 'Evidence, hearings and clocks' };

  // Live data sources (all free, CORS, read at runtime; ArcGIS domains resolved from the layer's ?f=json)
  const AC = 'https://services1.arcgis.com/n4yPwebTjJCmXB6W/arcgis/rest/services', NZ = 'https://services.arcgis.com/CXBb7LAjgIIdcsPt/arcgis/rest/services';
  const LAYERS = {
    zone: { url: `${AC}/Unitary_Plan_Base_Zone/FeatureServer/0`, name: 'AUP base zones (Auckland Council)', fields: 'ZONE,GROUPZONE,NAME' },
    precinct: { url: `${AC}/Precincts/FeatureServer/0`, name: 'AUP precincts (Auckland Council)', fields: 'PRECINCT,SUBPRECINCT,TYPE' },
    cfoc: { url: `${AC}/Centre_Fringe_Office_Control/FeatureServer/0`, name: 'AUP Centre Fringe Office Control (Auckland Council)', fields: 'OBJECTID' },
    pc120: { url: `${AC}/PC120Revised_PROD_PropertySummary/FeatureServer/0`, name: 'PC120 property summary, Council\'s proposed amendments (Auckland Council)', fields: 'FORMATTEDADDRESS,Zone1,Zone2,Zone3,Precinct,Change' },
    fds: { url: `${AC}/Future_Urban_Areas/FeatureServer/0`, name: 'FDS 2023 future urban areas and timing (Auckland Council)', fields: 'fds_strategicareatype,fds_strategicareaname,fds_timing' },
    board: { url: `${AC}/AucklandCouncilBoard/FeatureServer/0`, name: 'Local board areas (Auckland Council)', fields: 'BOARD' },
    designation: { url: `${AC}/Designation/FeatureServer/0`, name: 'AUP designations (Auckland Council)', fields: 'NAME,SUBTYPE,SCHEDULE' },
    arterial: { url: `${AC}/Arterial_Roads/FeatureServer/0`, name: 'AUP arterial roads (planning maps)', fields: 'OBJECTID' },
    var: { url: `${AC}/Vehicle_Access_Restriction_Control/FeatureServer/0`, name: 'AUP Vehicle Access Restriction Control', fields: 'TYPE' },
    frontage: { url: `${AC}/Building_Frontage_Control/FeatureServer/0`, name: 'AUP Building Frontage Control (Key Retail / General Commercial)', fields: 'TYPE' },
    lx: { url: `${AC}/Level_Crossings_With_Sightlines_Control/FeatureServer/0`, name: 'AUP Level Crossings with Sightlines Control', fields: 'OBJECTID' },
    sh: { url: `${NZ}/SH_Network/FeatureServer/0`, name: 'NZTA state highway network (SH_Network, digitised 2022)', fields: 'State_Highway,Name' },
  };
  const AC_LICENCE = 'Auckland Council open data, CC BY 4.0 as stated on the Council open data portal (confirm before publishing)';
  const NZTA_LICENCE = 'NZTA open data, CC BY 4.0 as stated on the NZTA open data portal (confirm before publishing)';
  const TIMEOUT = 20000;

  /* ====================================================================================================
     2. Project record (Store.P.compliance) and small helpers
     ==================================================================================================== */
  function blank() {
    return {
      v: 1,
      ctx: { pathway: '', lodgement: '', lodgementActual: false, activityStatus: '', region: 'auckland', landInterest: '', cl82Date: '', transferDate: '', notificationDate: '',
        occupation: null, context: 'urban', zoningConsistent: true, vest: false, raCouncil: false, devHvPct: null },
      site: { lat: null, lng: null, src: '', lookup: null, zoneOverride: '', precinctOverride: '', pc120Override: '', zoneTA: false, unlisted: false, school: 'primary', driveThrough: false,
        fdsOverride: { name: '', timing: '', source: '', page: '' } },
      q: {}, access: [], net: { sh: {}, shUser: {}, aupArterial: {}, loaded: {} }, overlays: null, peds: {}, mwConfirm: {},
      scoping: {}, proportion: { text: '', confirmed: false, by: '', date: '' },
      fds: { infra: [], fundingGap: '', rsp: { name: '', date: '' }, pair: null },
      rfi: { events: [], hearing: '', worksDate: '' },
      expert: { author: { name: '', quals: '', org: '' }, signoffs: [], history: [], confidence: { level: '', why: '' }, limitations: '', gaps: '', tests: '', supplementary: null },
      diff: { text: '', items: [], result: null, other: '' },
      checks: {}, notes: {}, used: {}, outputs: null, shown: {},
    };
  }
  const isObj = v => v && typeof v === 'object' && !Array.isArray(v);
  function fill(dst, def) { for (const [k, v] of Object.entries(def)) { if (!(k in dst) || dst[k] === undefined) dst[k] = v; else if (isObj(v) && isObj(dst[k])) fill(dst[k], v); } return dst; }
  // The project's compliance record, created and filled with defaults on first use (like DataHub.D()); a record
  // already filled is returned as it is (state() runs for every link in operators())
  const filled = new WeakSet();
  function state(P = Store.P) {
    if (isObj(P.compliance) && filled.has(P.compliance)) return P.compliance;
    if (!isObj(P.compliance)) P.compliance = {};
    fill(P.compliance, blank()); filled.add(P.compliance); return P.compliance;
  }
  const own = (o, k) => !!o && Object.prototype.hasOwnProperty.call(o, k);
  const S_ = P => Object.assign({ dvc: OPS.dvc, ddelay: OPS.ddelay, vicinity: E27.tier1Area.m, evidence: ACCESS.evidenceM, precincts: '' }, ((P && P.settings) || {}).compliance || {});
  const activeSc = P => (P.scenarios.find(s => s.id === P.active) || P.scenarios[0]);
  const baseSc = P => (P.scenarios.find(s => s.id === P.base) || P.scenarios[0]);
  const resOf = (sc, period) => (sc && sc.res && sc.res[period]) || null;
  const periodsOf = sc => ['AM', 'PM'].filter(p => sc && sc.res && sc.res[p]);
  const pw = P => PATHWAYS[state(P).ctx.pathway] || null;
  const inAuckland = P => state(P).ctx.region !== 'other';
  const roadLinks = (sc, nid) => Net.linksAt(sc, nid).filter(L => L.status !== 'closed');
  // Open links at every node in one pass (Net.linksAt scans every link, so calling it per node is slow)
  function degrees(sc) { const m = new Map(); for (const L of Object.values(sc.links)) { if (L.status === 'closed') continue; m.set(L.a, (m.get(L.a) || 0) + 1); if (L.b !== L.a) m.set(L.b, (m.get(L.b) || 0) + 1); } return m; }
  const pill = (text, tone) => `<span class="pill${tone === 'warn' ? ' proposed' : tone === 'bad' ? ' closed' : ''}"${tone === 'ok' ? ' style="color:var(--ok)"' : ''}>${text}</span>`;
  const tag = text => `<span class="prov assumed" title="${esc(DISCLAIMERS.D13.text)}">${esc(text)}</span>`;
  const srcLink = s => s ? `<a href="${esc(s.url)}" target="_blank" rel="noopener">${esc(s.name)}</a>${s.verified ? '' : ' <span class="note">(verify)</span>'}` : '';
  // FNV-1a over a string (two seeds, 16 hex characters): the sign-off fingerprint
  function hash(str) {
    const imul = Math.imul, B = 2246822519 | 0;   // local: a global lookup per character is slow (in the test vm, 100×)
    let a = 0x811c9dc5 | 0, b = (0x01000193 ^ 0x5bd1e995) | 0;
    for (let i = 0; i < str.length; i++) { const c = str.charCodeAt(i); a = imul(a ^ c, 16777619); b = imul(b ^ c, B); }
    return (a >>> 0).toString(16).padStart(8, '0') + (b >>> 0).toString(16).padStart(8, '0');
  }

  /* ---------- geometry ---------- */
  function flatAt(lat) { const F = U.flat(lat); return ll => [F.x(ll), F.y(ll)]; }
  // Distance (m) from a point to a polyline [[lat, lng]…], with the bearing of the nearest segment
  function toLine(P, pts, f) {
    const p = f(P); let best = { d: Infinity, b: null };
    for (let i = 1; i < pts.length; i++) {
      const a = f(pts[i - 1]), c = f(pts[i]), r = U.segDist(p[0], p[1], a[0], a[1], c[0], c[1]);
      if (r.d < best.d) best = { d: r.d, b: U.bearing(pts[i - 1], pts[i]) };
    }
    return best;
  }
  function inRing(ll, ring) {   // ring [[lat, lng]…]
    let inside = false;
    for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
      const [yi, xi] = ring[i], [yj, xj] = ring[j];
      if ((yi > ll[0]) !== (yj > ll[0]) && ll[1] < (xj - xi) * (ll[0] - yi) / ((yj - yi) || 1e-12) + xi) inside = !inside;
    }
    return inside;
  }
  function toPolys(P, rings, f) { let d = Infinity; for (const r of rings) { if (inRing(P, r)) return 0; d = Math.min(d, toLine(P, r, f).d); } return d; }
  // Shortest distance (m) between two polylines [[lat, lng]…] (0 where they cross)
  function lineToLine(A, B, f) {
    const a = A.map(f), b = B.map(f), o = (p, q, r) => (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    const pt = (p, s, e) => U.segDist(p[0], p[1], s[0], s[1], e[0], e[1]).d;
    if (a.length === 1 || b.length === 1) { const [p, L] = a.length === 1 ? [a[0], b] : [b[0], a]; let d = Infinity; for (let i = 1; i < L.length; i++) d = Math.min(d, pt(p, L[i - 1], L[i])); return L.length === 1 ? Math.hypot(p[0] - L[0][0], p[1] - L[0][1]) : d; }
    let d = Infinity;
    for (let i = 1; i < a.length; i++) for (let j = 1; j < b.length; j++) {
      const p1 = a[i - 1], p2 = a[i], q1 = b[j - 1], q2 = b[j];
      if ((o(q1, q2, p1) > 0) !== (o(q1, q2, p2) > 0) && (o(p1, p2, q1) > 0) !== (o(p1, p2, q2) > 0)) return 0;
      d = Math.min(d, pt(p1, q1, q2), pt(p2, q1, q2), pt(q1, p1, p2), pt(q2, p1, p2));
    }
    return d;
  }
  /* Shortest distance (m) from a polyline to rings (0 when a vertex is inside one or the line crosses an edge), or to
     open lines when open is true. Shapes further than `within` m (bounding boxes) are skipped and give Infinity. */
  const shapeBox = new WeakMap();
  function boxOf(pts) { if (shapeBox.has(pts)) return shapeBox.get(pts); let s = Infinity, n = -Infinity, w = Infinity, e = -Infinity; for (const p of pts) { s = Math.min(s, p[0]); n = Math.max(n, p[0]); w = Math.min(w, p[1]); e = Math.max(e, p[1]); } const b = { s, n, w, e }; shapeBox.set(pts, b); return b; }
  function lineToPolys(pts, shapes, f, within = Infinity, open = false) {
    const lb = boxOf(pts), pad = Math.min(within, 1e5) / 111320, padLng = pad / Math.max(0.2, Math.cos(lb.s * Math.PI / 180));
    let d = Infinity;
    for (const r of shapes) {
      if (!r || !r.length) continue;
      const rb = boxOf(r);
      if (isFinite(within) && (rb.s > lb.n + pad || rb.n < lb.s - pad || rb.w > lb.e + padLng || rb.e < lb.w - padLng)) continue;
      if (!open && pts.some(p => inRing(p, r))) return 0;
      const closed = !open && r.length > 2 && (r[0][0] !== r[r.length - 1][0] || r[0][1] !== r[r.length - 1][1]) ? r.concat([r[0]]) : r;
      d = Math.min(d, lineToLine(pts, closed, f));
    }
    return d;
  }
  function boundsOf(sc) {
    let s = Infinity, w = Infinity, n = -Infinity, e = -Infinity;
    for (const nd of Object.values(sc.nodes)) { s = Math.min(s, nd.lat); n = Math.max(n, nd.lat); w = Math.min(w, nd.lng); e = Math.max(e, nd.lng); }
    return isFinite(s) ? { s, w, n, e } : null;
  }
  /* Conflate polylines to model links: a link matches when 2 of its 3 points (¼, ½, ¾) lie within radius of a
     line with the line's bearing within 30° (either direction). Returns { linkId: feature tag }. */
  function conflate(sc, lines, { radius = 15, bearing = 30 } = {}) {
    const out = {}, b = boundsOf(sc); if (!b || !lines.length) return out;
    const f = flatAt((b.s + b.n) / 2), padLat = 200 / 111320, padLng = 200 / (111320 * Math.cos((b.s + b.n) / 2 * Math.PI / 180));
    const near = lines.map(l => ({ tag: l.tag, pts: l.pts.filter(p => p[0] > b.s - padLat && p[0] < b.n + padLat && p[1] > b.w - padLng && p[1] < b.e + padLng) })).filter(l => l.pts.length > 1);
    if (!near.length) return out;
    for (const L of Object.values(sc.links)) {
      if (!sc.nodes[L.a] || !sc.nodes[L.b]) continue;
      const g = Net.geom(sc, L), cum = U.cumLen(g), len = cum[cum.length - 1]; if (!(len > 0)) continue;
      let best = null;
      for (const l of near) {
        let hits = 0;
        for (const s of [0.25, 0.5, 0.75]) {
          const a = U.along(g, cum, len * s), lb = U.bearing(g[a.i], g[a.i + 1]), r = toLine(a.p, l.pts, f);
          if (r.d <= radius && (U.angDiff(lb, r.b) <= bearing || U.angDiff(lb, r.b) >= 180 - bearing)) hits++;
        }
        if (hits >= 2) { best = l.tag; break; }
      }
      if (best != null) out[L.id] = best;
    }
    return out;
  }

  /* ---------- ArcGIS queries (AbortController time limit, fail soft with a message that says what to do) ---------- */
  async function agFetch(url, timeout = TIMEOUT) {
    const host = new URL(url).host, ac = typeof AbortController !== 'undefined' ? new AbortController() : null, tm = ac ? setTimeout(() => ac.abort(), timeout) : null;
    const late = `${host} did not answer within ${Math.round(timeout / 1000)} s. Try again later.`;
    let r, j;
    // the time limit covers the body too: a stalled response must not hang the lookup
    try {
      try { r = await fetch(url, ac ? { signal: ac.signal } : undefined); }
      catch (e) { throw new Error(e && e.name === 'AbortError' ? late : `${host} could not be reached (${(e && e.message) || e}). Check your connection and try again.`); }
      if (!r.ok) throw new Error(`${host} answered HTTP ${r.status}. Try again later.`);
      try { j = await r.json(); }
      catch (e) { throw new Error(e && e.name === 'AbortError' ? late : `${host} sent an answer Spare Construct could not read (not JSON). Try again later, or enter the values by hand.`); }
    } finally { if (tm) clearTimeout(tm); }
    if (j && j.error) throw new Error(`${host}: ${j.error.message || 'the query failed'}. Try again later.`);
    return j;
  }
  const metaCache = new Map();
  // Coded-value domains for a layer, from the query response or the layer's ?f=json (rule 5)
  async function domains(layer, j) {
    let fields = j && Array.isArray(j.fields) && j.fields.some(f => f.domain) ? j.fields : null;
    if (!fields) {
      if (!metaCache.has(layer.url)) metaCache.set(layer.url, agFetch(layer.url + '?f=json').catch(() => ({ fields: [] })));
      fields = (await metaCache.get(layer.url)).fields || [];
    }
    const out = {};
    for (const f of fields) if (f.domain && Array.isArray(f.domain.codedValues)) out[f.name] = Object.fromEntries(f.domain.codedValues.map(c => [String(c.code), c.name]));
    return out;
  }
  async function atPoint(layer, lat, lng) {
    const q = new URLSearchParams({ geometry: `${lng},${lat}`, geometryType: 'esriGeometryPoint', inSR: '4326', spatialRel: 'esriSpatialRelIntersects', outFields: layer.fields, returnGeometry: 'false', f: 'json' });
    const j = await agFetch(`${layer.url}/query?${q}`);
    return { features: (j.features || []).map(x => x.attributes || {}), dom: await domains(layer, j) };
  }
  async function inBox(layer, b, geometry = true) {
    const out = [];
    for (let page = 0, offset = 0; page < 20; page++) {
      const q = new URLSearchParams({ where: '1=1', geometry: [b.w, b.s, b.e, b.n].map(v => (+v).toFixed(6)).join(','), geometryType: 'esriGeometryEnvelope', inSR: '4326', spatialRel: 'esriSpatialRelIntersects',
        outFields: layer.fields, returnGeometry: String(geometry), outSR: '4326', f: 'json', resultOffset: String(offset) });
      const j = await agFetch(`${layer.url}/query?${q}`);
      const f = j.features || []; out.push(...f);
      if (!j.exceededTransferLimit || !f.length) break;
      offset += f.length;
    }
    return out;
  }
  const paths = g => ((g && (g.paths || g.rings)) || []).map(p => p.map(q => [U.r7(q[1]), U.r7(q[0])]));
  const prov = (layer, licence = AC_LICENCE, now) => ({ source: layer.name, url: layer.url, date: today(now), licence, basis: 'measured' });

  /* ====================================================================================================
     3. Regime and version stamp (reg-regime-stamp)
     ==================================================================================================== */
  function regime(P) {
    const C = state(P), c = C.ctx, p = PATHWAYS[c.pathway], d = validDay(c.lodgement) ? c.lodgement.slice(0, 10) : null;
    const std = REGIME.specifiedTransitionDate[c.region === 'other' ? 'other' : 'auckland'];
    const stdText = std ? `Specified transition date: ${longDate(std)}` : `Specified transition date: not yet set (must be after ${longDate(REGIME.mustBeAfter).replace(/^(\d+) (\w{3})\w* /, '$1 $2 ')})`;
    const base = { pathway: c.pathway, pathwayLabel: p ? p.label : 'Pathway not set', date: d, transitionText: stdText, side: null, clause: null };
    if (!p) return { ...base, code: 'unset', text: 'Choose the pathway to set the regime.', short: 'Pathway not set' };
    if (!p.proc) return { ...base, code: 'other', text: `Not an RMA process: ${p.law}.`, short: p.law };
    if (!d) return { ...base, code: 'undated', text: 'Enter the lodgement date (planned or actual) to set the regime.', short: 'Lodgement date not set' };
    const side = d < REGIME.commence ? 'pre' : std && d >= std ? 'after' : 'transition';
    const nor = p.proc === 'nor';
    if (side === 'after') return { ...base, side, code: 'planning-act', text: 'Planning Act 2026 regime (after the specified transition date). Spare Construct does not yet cover it: get planning advice.', short: 'Planning Act 2026' };
    if (p.kind === 'ftaa') {
      const clause = side === 'pre' ? (nor ? 'FTAA Sch 1 cl 19' : 'FTAA Sch 1 cl 17') : (nor ? 'FTAA Sch 1 cl 20' : 'FTAA Sch 1 cl 18');
      return { ...base, side, clause, code: 'ftaa-' + side,
        text: side === 'pre' ? `FTAA and RMA as when made (${clause})` : `FTAA as at lodgement, with the RMA as amended by the Planning Act 2026 (${clause})`,
        short: side === 'pre' ? 'FTAA as when made' : 'FTAA, transition RMA' };
    }
    if (p.kind === 'pc') return { ...base, side, code: 'plan-' + side, clause: null,
      text: side === 'pre' ? 'RMA plan-change process as in force before 30 Oct 2026 (check the Planning Act 2026 Sch 1 transition for plan changes)' : 'RMA plan-change process as amended by the Planning Act 2026 Sch 12 Pt 2, including the plan stop (ss80P-80W) and s80YA',
      short: side === 'pre' ? 'Pre-assent RMA' : 'Transition RMA' };
    const clause = side === 'pre' ? (nor ? 'Planning Act Sch 1 cl 20' : 'Planning Act Sch 1 cl 18') : (nor ? 'Planning Act Sch 1 cl 21' : 'Planning Act Sch 1 cl 19');
    return { ...base, side, clause, code: 'rma-' + side,
      text: side === 'pre' ? `RMA as in force immediately before 30 Sep 2026 (${clause})` : `RMA as at lodgement incl. Planning Act Sch 12 Pt 2 (${clause})`,
      short: side === 'pre' ? 'Pre-assent RMA' : 'Transition RMA' };
  }
  // The version stamp printed on the title page and in every exported table footer
  function stamp(P, now) {
    const r = regime(P), C = state(P), d = today(now);
    const rma = r.side === 'pre' ? 'as in force immediately before 30 Sep 2026' : r.side === 'transition' ? `as at ${longDate(r.date)}` : '–', ftaa = 'as at 10 Jul 2026';
    // an edition whose next known change has passed is marked, so a stamp printed later does not look current
    const ed = (id, e) => { const s = STANDARDS.find(x => x.id === id), n = s && s.next; return n && n.date && d >= n.date ? `${e} (changed ${longDate(n.date)}: check)` : e; };
    const docs = STAMP_DOCS.map(([id, name, e]) => [name, ed(id, e)]);
    const lines = [
      ['Pathway', r.pathwayLabel], ['Lodgement', r.date ? `${longDate(r.date)}${C.ctx.lodgementActual ? '' : ' (planned)'}` : 'not set'], ['Regime', r.text],
      ['RMA version', rma], ['FTAA version', ftaa], ['Planning Act', r.transitionText],
      ...docs, ['Spare Construct regulatory data', `as at ${longDate(VERSION)}`],
    ];
    const text = `${r.short}${r.clause ? ` (${r.clause})` : ''} · ${rma !== '–' ? `RMA ${rma} · ` : ''}FTAA ${ftaa} · ${r.transitionText} · ` + docs.map(([n, e]) => `${n} ${e}`).join(' · ') + ` · Spare Construct regulatory data ${VERSION}`;
    return { regime: r, lines, text, footer: `Version stamp: ${text}`, at: d };
  }
  // Remember the lodgement date and regime that outputs were generated under (called after each model run and export)
  // The standards an output relies on: the stamped documents plus the pathway's own law
  const LAW_DOCS = { rc: ['rma'], pc: ['rma'], nor: ['rma'], ftaa: ['ftaa', 'rma'], tou: ['tou'], fund: ['ipm-2027'], speed: ['speed-rule'] };
  function usedDocs(P) { const p = PATHWAYS[state(P).ctx.pathway]; return [...new Set(STAMP_DOCS.map(([id]) => id).concat(p ? LAW_DOCS[p.kind] || [] : []))]; }
  // 'used' is the date outputs were last generated against each document, so regenerating them clears a supersession warning
  function recordOutputs(P, now) {
    const C = state(P), r = regime(P), d = today(now);
    C.outputs = { at: d, lodgement: r.date, side: r.side, regime: r.code };
    for (const id of usedDocs(P)) C.used[id] = d;
    return C.outputs;
  }
  // The re-check banner: outputs were generated before the lodgement date moved across 30 Oct 2026 (RMA pathways only)
  function recheck(P) {
    const C = state(P), o = C.outputs, r = regime(P), p = PATHWAYS[C.ctx.pathway];
    const sideOf = d => (d ? (d < REGIME.commence ? 'pre' : 'post') : null);
    const was = o && sideOf(o.lodgement), now = sideOf(r.date);
    const show = !!(was && now && was !== now && p && p.proc);
    return { show, text: show ? REGIME.recheck : '', since: o ? o.at : null };
  }

  /* ====================================================================================================
     4. Authorities and affected operators (reg-authority)
     ==================================================================================================== */
  function authorities(P, now) {
    const C = state(P), c = C.ctx, d = validDay(c.lodgement) ? c.lodgement.slice(0, 10) : today(now), site = siteInfo(P);
    const rows = [];
    if (!inAuckland(P)) {
      rows.push({ role: 'Road controlling authority', name: AUTH.other.rca, basis: 'Local Government Act 1974 / Land Transport Act' });
    } else if (d < AUTH.transfer) {
      rows.push({ role: 'Road controlling authority', name: AUTH.before.rca, basis: 'LGACA 2009 (before 7 May 2026)' });
    } else {
      rows.push({ role: 'Road controlling authority', name: AUTH.after.rca, basis: 'LGACA 2009 s47 (from 7 May 2026)' });
      rows.push({ role: 'Local and collector roads (speed, signals, crossings, closures)', name: site.board ? `${site.board} Local Board (LGACA s47C, Sch 4)` : AUTH.after.board, basis: 'LGACA s47C, Sch 4' });
      rows.push({ role: 'Public transport', name: AUTH.after.cco, basis: 'LGACA s43, s43A' });
    }
    rows.push({ role: 'State highways', name: AUTH.nzta, basis: 'Land Transport Management Act 2003; GRPA 1989' });
    rows.push({ role: 'Rail and level crossings', name: AUTH.kiwirail, basis: 'Railways Act 2005' });
    const endBy = [AUTH.transitionalEnd, validDay(c.cl82Date) ? c.cl82Date : null].filter(Boolean).sort()[0];
    const transitional = inAuckland(P) && d >= AUTH.transfer && d < endBy;
    let requiring = null;
    if (inAuckland(P) && PATHWAYS[c.pathway] && PATHWAYS[c.pathway].proc === 'nor') {
      const t = validDay(c.transferDate) ? c.transferDate : null;
      requiring = !t ? 'For former Auckland Transport designations: the transport CCO until the transfer date under LGACA Sch 1AA cl 10 (not entered yet), then Auckland Council.'
        : d < t ? `For former Auckland Transport designations: the transport CCO (Auckland Transport) until ${longDate(t)} (Sch 1AA cl 10).` : `For former Auckland Transport designations: Auckland Council, since ${longDate(t)} (Sch 1AA cl 10).`;
    }
    return { date: d, rows, transitional, transitionalEnd: endBy,
      transitionalText: transitional ? `Transitional period: Auckland Transport keeps transitional functions until ${longDate(endBy)} (the earlier of 7 Nov 2026 and any cl 8(2) direction). Confirm whether a direction has ended it early.` : '',
      requiring, arcf: `${AUTH.arcf.name}: ${AUTH.arcf.warning}. Road classes for local-board decisions use an interim source until it is.`,
      reportName: d >= AUTH.transfer && inAuckland(P) ? AUTH.reportName : 'Auckland Transport (road controlling authority)', disclaimer: DISCLAIMERS.D11.text };
  }
  // State highway status of a link: the user's tick, the NZTA layer (a matched link with a blank highway number still counts), or the motorway/ramp class
  function shOf(P, L) {
    const n = state(P).net, has = Object.prototype.hasOwnProperty.call(n.sh, L.id), tag = has ? String(n.sh[L.id] || '') : '';
    if (n.shUser[L.id] === true) return { sh: true, ref: tag ? 'SH' + tag : 'SH', basis: 'marked by the user' };
    if (n.shUser[L.id] === false) return { sh: false, basis: 'marked not a state highway by the user' };
    if (has) return { sh: true, ref: tag ? 'SH' + tag : 'SH', basis: 'NZTA SH_Network layer (conflated)' };
    if (L.cls === 'motorway' || L.cls === 'ramp') return { sh: true, ref: 'SH (motorway)', basis: 'motorway or ramp road class (assumed NZTA)' };
    return { sh: false, basis: n.loaded.sh ? 'not on the NZTA layer' : 'NZTA layer not loaded' };
  }
  // The project facts linkAuthority needs, computed once per operators() call rather than once per link
  function authCtx(P, sc) {
    const A = authorities(P), auck = inAuckland(P);
    return { A, site: siteInfo(P), auck, post: auck && A.date >= AUTH.transfer, routes: Object.values(sc.routes || {}).map(r => new Set(r.wp || [])) };
  }
  // Per-link resolver: operator, KiwiRail where within 30 m of a level crossing, and the decision-maker for speed, signals, crossings and closures
  function linkAuthority(P, L, sc = activeSc(P), X = authCtx(P, sc)) {
    const { site, auck, post } = X, sh = shOf(P, L);
    const also = [];
    const lxD = levelCrossingDist(P, sc, L);
    if (lxD != null && lxD <= AUTH.levelCrossingM) also.push(AUTH.kiwirail);
    if (auck && post && (L.busLane || X.routes.some(r => r.has(L.a) && r.has(L.b)))) also.push(AUTH.after.cco);
    if (sh.sh) return { operator: 'nzta', name: AUTH.nzta, sh, also, decision: { who: AUTH.nzta, basis: 'State highway' } };
    const name = !auck ? AUTH.other.rca : post ? AUTH.after.rca : AUTH.before.rca;
    const art = state(P).net.aupArterial[L.id];
    const cls = art ? 'arterial' : L.cls, basis = art ? 'AUP planning-map arterial overlay (interim: the s48 framework is not yet published)' : 'Spare Construct road class (interim: the s48 framework is not yet published)';
    const board = auck && post && (cls === 'local' || cls === 'collector');
    return { operator: 'rca', name, sh, also, decision: { who: board ? (site.board ? `${site.board} Local Board` : 'The local board') + ' (LGACA s47C, Sch 4)' : auck && post ? 'Auckland Council governing body' : name, basis, cls } };
  }
  const alsoKey = n => (n === AUTH.kiwirail ? 'kiwirail' : 'cco');
  /* Affected operators: links and junctions whose with-development change passes the threshold (Spare Construct assumption)
     in either modelled peak (opts.period for one only). Each asset keeps the peak(s) it passes in. A link on a bus
     route or with a bus lane also counts for the transport CCO; one within 30 m of a level crossing for KiwiRail. */
  function operators(P, opts = {}) {
    const sc = opts.sc || activeSc(P), base = opts.base || baseSc(P), S = S_(P), C = state(P);
    const periods = opts.period ? [opts.period] : ['AM', 'PM'];
    const pairs = periods.map(p => ({ p, w: resOf(sc, p), wo: sc === base ? null : resOf(base, p) })), run = pairs.filter(x => x.w && x.wo);
    const groups = new Map(), add = (key, name) => { if (!groups.has(key)) groups.set(key, { key, name, assets: 0, links: [], nodes: [], _l: new Map(), _n: new Map() }); return groups.get(key); };
    const X = authCtx(P, sc), byLink = new Map();
    // one row per asset and group: the largest change, with every peak it passes in
    const put = (map, list, id, row, size) => {
      const o = map.get(id);
      if (!o) { const r = { ...row, periods: [row.period] }; map.set(id, r); list.push(r); return; }
      if (!o.periods.includes(row.period)) o.periods.push(row.period);
      if (size(row) > size(o)) Object.assign(o, row, { periods: o.periods });
    };
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed') continue;
      const a = linkAuthority(P, L, sc, X), g = add(a.operator, a.name), extra = a.also.map(n => add(alsoKey(n), n));
      g.assets++; for (const x of extra) x.assets++;
      byLink.set(L.id, { a, g, extra });
      for (const { p, w, wo } of run) for (const d of ['ab', 'ba']) {
        const r1 = w.arcs[L.id + ':' + d], r0 = wo.arcs[L.id + ':' + d]; if (!r1 && !r0) continue;
        const dvc = ((r1 || {}).vc || 0) - ((r0 || {}).vc || 0); if (!(dvc >= S.dvc)) continue;
        const row = { id: L.id, dir: d, period: p, name: L.name || CLASSES[L.cls].label, dvc: U.r3(dvc), v0: U.r1((r0 || {}).v || 0), v1: U.r1((r1 || {}).v || 0) };
        for (const x of [g, ...extra]) put(x._l, x.links, L.id + ':' + d, row, r => r.dvc);
      }
    }
    for (const { p, w, wo } of run) for (const [nid, n1] of Object.entries(w.nodes || {})) {
      const n0 = (wo.nodes || {})[nid], dd = (n1.delay || 0) - ((n0 || {}).delay || 0); if (!(dd >= S.ddelay)) continue;
      const at = roadLinks(sc, nid).map(L => byLink.get(L.id)).filter(Boolean), sh = at.find(x => x.a.operator === 'nzta');
      const gs = new Set([sh ? sh.g : at[0] ? at[0].g : add('rca', linkAuthority(P, { id: '', cls: 'local' }, sc, X).name)]);
      for (const x of at) for (const e of x.extra) gs.add(e);
      const row = { id: nid, period: p, name: Kit.junctionName(nid, sc), dd: U.r1(dd), d0: U.r1((n0 || {}).delay || 0), d1: U.r1(n1.delay || 0) };
      for (const g of gs) put(g._n, g.nodes, nid, row, r => r.dd);
    }
    if ((C.access || []).some(a => { const x = accessGeo(P, sc, a); return x && x.lxDist != null && x.lxDist <= AUTH.levelCrossingM; })) add('kiwirail', AUTH.kiwirail).access = true;
    const order = { nzta: 0, rca: 1, cco: 2, kiwirail: 3 };
    const peaks = ps => ps.join(' and ') + ' peak' + (ps.length > 1 ? 's' : '');
    const done = run.map(x => x.p), periodText = peaks(done.length ? done : periods), missing = pairs.filter(x => !(x.w && x.wo)).map(x => x.p);
    const note = !run.length ? (sc === base ? 'Make another scenario active (with development) to compare it with the base scenario.' : `Run ${sc.name} and ${base.name} for the ${peaks(periods)} to compare.`)
      : missing.length ? `Only the ${periodText} compared: run both scenarios for the ${peaks(missing)} too.` : '';
    return { period: (done.length ? done : periods).join('+'), periods: done, periodText, compared: run.length > 0, withName: sc.name, withoutName: base.name,
      rows: [...groups.values()].sort((a, b) => order[a.key] - order[b.key]).map(({ _l, _n, ...g }) => ({ ...g, affected: g.links.length + g.nodes.length > 0 || !!g.access })),
      thresholds: { dvc: S.dvc, ddelay: S.ddelay, basis: OPS.basis },
      label: `Affected when the with-development change (${sc.name} against ${base.name}, ${periodText}) is a V/C increase of ${S.dvc} or more, or a junction delay increase of ${S.ddelay} s or more (${OPS.basis}). Links on a modelled bus route or with a bus lane also count for the transport CCO, and links within 30 m of a level crossing for KiwiRail. Feeds Sch 4 cl 6(1)(f) and s95E advice.`,
      note };
  }
  // The map layer redraws often: reuse the last answer until the project changes (Store.ver) or new results land
  let opsMemo = null;
  function operatorsCached(P) {
    const sc = activeSc(P), base = baseSc(P), r = s => s.res || {}, key = [Store.ver, sc.id, base.id, r(sc).AM, r(sc).PM, r(base).AM, r(base).PM];
    if (opsMemo && opsMemo.P === P && opsMemo.key.every((v, i) => v === key[i])) return opsMemo.out;
    opsMemo = { P, key, out: operators(P) };
    return opsMemo.out;
  }

  /* ====================================================================================================
     5. Site lookup: AUP zone, precinct, CFOC, PC120, FDS, local board, designation; AUP controls; SH layer
     ==================================================================================================== */
  function sitePoint(P, sc = activeSc(P)) {
    const C = state(P);
    if (isFinite(C.site.lat) && isFinite(C.site.lng) && C.site.lat != null) return { lat: +C.site.lat, lng: +C.site.lng, src: C.site.src || 'set by the user' };
    const dev = Object.values(sc.zones).filter(z => z.kind !== 'gateway' && Scenarios.zoneRole(z) === 'development');
    if (dev.length) return { lat: dev.reduce((s, z) => s + z.lat, 0) / dev.length, lng: dev.reduce((s, z) => s + z.lng, 0) / dev.length, src: `centre of ${dev.length} development zone${dev.length === 1 ? '' : 's'}` };
    const b = boundsOf(sc); return b ? { lat: (b.s + b.n) / 2, lng: (b.w + b.e) / 2, src: 'centre of the network' } : null;
  }
  async function lookupSite(P, opts = {}) {
    const C = state(P), pt = opts.point || sitePoint(P); if (!pt) throw new Error('Add roads or a development zone first, so Spare Construct knows where the site is.');
    const out = { lat: U.r7(pt.lat), lng: U.r7(pt.lng), src: pt.src, at: today(opts.now), errors: [], sources: [] };
    const job = async (k, fn) => { try { await fn(); out.sources.push(prov(LAYERS[k], AC_LICENCE, opts.now)); } catch (e) { out.errors.push(`${LAYERS[k].name}: ${e.message}`); } };
    await Promise.all([
      job('zone', async () => {
        const r = await atPoint(LAYERS.zone, pt.lat, pt.lng), D = r.dom.ZONE || {};
        const zs = r.features.map(a => ({ code: a.ZONE, name: D[String(a.ZONE)] || `Zone ${a.ZONE}`, group: (r.dom.GROUPZONE || {})[String(a.GROUPZONE)] || '' }));
        out.zone = zs.find(z => !/^(Road|Strategic Transport Spare Construct Zone|Water)$/i.test(z.name)) || zs[0] || null; out.zones = zs;
      }),
      job('precinct', async () => {
        const r = await atPoint(LAYERS.precinct, pt.lat, pt.lng), D = r.dom.PRECINCT || {};
        out.precincts = r.features.map(a => ({ code: a.PRECINCT, name: D[String(a.PRECINCT)] || `Precinct ${a.PRECINCT}`, sub: a.TYPE === 2 }));
      }),
      job('cfoc', async () => { out.cfoc = (await atPoint(LAYERS.cfoc, pt.lat, pt.lng)).features.length > 0; }),
      job('pc120', async () => {
        const f = (await atPoint(LAYERS.pc120, pt.lat, pt.lng)).features[0];
        out.pc120 = f ? { zone: f.Zone1 || null, zones: [f.Zone1, f.Zone2, f.Zone3].filter(Boolean), precinct: f.Precinct || null, address: String(f.FORMATTEDADDRESS || '').replace(/\r/g, ', '), note: f.Change || '' } : null;
      }),
      job('fds', async () => { const f = (await atPoint(LAYERS.fds, pt.lat, pt.lng)).features[0]; out.fds = f ? { name: f.fds_strategicareaname, type: f.fds_strategicareatype, timing: f.fds_timing } : null; }),
      job('board', async () => { const f = (await atPoint(LAYERS.board, pt.lat, pt.lng)).features[0]; out.board = f ? f.BOARD : null; }),
      job('designation', async () => {
        const r = await atPoint(LAYERS.designation, pt.lat, pt.lng), D = r.dom.SUBTYPE || {};
        out.designations = r.features.map(a => ({ name: a.NAME || '', by: D[String(a.SUBTYPE)] || '', schedule: a.SCHEDULE || '' }));
      }),
    ]);
    if (!out.sources.length) throw new Error(`The Auckland Council map services could not be read: ${out.errors[0] || 'no answer'}. Enter the zone and precinct by hand, or try again later.`);
    // a layer that failed this time keeps its earlier answer for the same point, with that answer's date
    const prev = C.site.lookup, FIELDS = { zone: ['zone', 'zones'], precinct: ['precincts'], cfoc: ['cfoc'], pc120: ['pc120'], fds: ['fds'], board: ['board'], designation: ['designations'] };
    out.kept = [];
    if (prev && prev.lat === out.lat && prev.lng === out.lng) for (const [k, fs] of Object.entries(FIELDS)) {
      if (out.sources.some(s => s.source === LAYERS[k].name)) continue;
      const old = (prev.sources || []).find(s => s.source === LAYERS[k].name); if (!old) continue;
      for (const f of fs) if (f in prev) out[f] = prev[f];
      out.sources.push(old); out.kept.push(`${LAYERS[k].name}: kept the answer from ${longDate(old.date)}`);
    }
    C.site.lookup = out; C.site.lat = out.lat; C.site.lng = out.lng; C.site.src = C.site.src || pt.src;
    return out;
  }
  // Arterial overlay, VAR, frontage and level-crossing controls for the study area, kept for the access checks
  async function loadOverlays(P, opts = {}) {
    const C = state(P), sc = opts.sc || activeSc(P), b = opts.bounds || boundsOf(sc); if (!b) throw new Error('Add roads first: the AUP controls are loaded for the network\'s extent.');
    const o = { at: today(opts.now), bounds: b, arterials: [], var: [], frontage: [], lx: [], errors: [], sources: [] };
    const job = async (k, fn) => { try { await fn(await inBox(LAYERS[k], b)); o.sources.push(prov(LAYERS[k], AC_LICENCE, opts.now)); } catch (e) { o.errors.push(`${LAYERS[k].name}: ${e.message}`); } };
    const VT = { 1: 'Motorway Interchange Control', 2: 'General', 3: 'Adjacent to Level Crossings' }, FT = { 1: 'Key Retail Frontage', 2: 'General Commercial Frontage' };
    await Promise.all([
      job('arterial', f => { o.arterials = f.flatMap(x => paths(x.geometry)); }),
      job('var', f => { o.var = f.flatMap(x => paths(x.geometry).map(p => ({ type: VT[(x.attributes || {}).TYPE] || 'General', pts: p }))); }),
      job('frontage', f => { o.frontage = f.flatMap(x => paths(x.geometry).map(p => ({ type: FT[(x.attributes || {}).TYPE] || 'Frontage control', pts: p }))); }),
      job('lx', f => { o.lx = f.map(x => paths(x.geometry)).filter(r => r.length); }),
    ]);
    if (!o.sources.length) throw new Error(`The AUP control layers could not be read: ${o.errors[0] || 'no answer'}. Enter the access controls by hand, or try again later.`);
    C.overlays = o;
    C.net.aupArterial = Object.fromEntries(Object.keys(conflate(sc, o.arterials.map(p => ({ tag: 1, pts: p })))).map(id => [id, true]));
    return o;
  }
  async function loadSH(P, opts = {}) {
    const C = state(P), sc = opts.sc || activeSc(P), b0 = opts.bounds || boundsOf(sc); if (!b0) throw new Error('Add roads first: state highways are matched to the network\'s roads.');
    const pad = 0.01, b = { s: b0.s - pad, w: b0.w - pad, n: b0.n + pad, e: b0.e + pad };
    const f = await inBox(LAYERS.sh, b);
    const lines = f.flatMap(x => paths(x.geometry).map(p => ({ tag: String((x.attributes || {}).State_Highway || '').replace(/^SH\s*/i, ''), pts: p })));
    C.net.sh = conflate(sc, lines);
    C.net.loaded.sh = { at: today(opts.now), n: Object.keys(C.net.sh).length, features: f.length, ...prov(LAYERS.sh, NZTA_LICENCE, opts.now) };
    return C.net.loaded.sh;
  }
  // Zone, precinct, PC120 and FDS for the site: the user's entries win over the lookup
  function siteInfo(P) {
    const C = state(P), L = C.site.lookup || {}, s = C.site;
    const zone = s.zoneOverride ? { name: s.zoneOverride, src: 'entered' } : L.zone ? { ...L.zone, src: 'AUP base zone layer' } : null;
    const precinctName = s.precinctOverride || ((L.precincts || []).map(p => p.name)[0]) || '';
    const list = PRECINCTS.concat(parsePrecincts(S_(P).precincts));
    const tp = precinctName ? list.find(p => new RegExp(p.match, 'i').test(precinctName)) || null : null;
    const pc120 = s.pc120Override ? { zone: s.pc120Override, src: 'entered' } : L.pc120 ? { ...L.pc120, src: LAYERS.pc120.name } : null;
    // FDS area: the author's entry from the strategy document (with its page) wins over the live layer
    const fo = s.fdsOverride || {}, fds = fo.name ? { name: fo.name, timing: fo.timing || '', source: fo.source || SRC.fds.name, page: fo.page || '', src: 'entered' } : L.fds ? { ...L.fds, src: LAYERS.fds.name } : null;
    return { zone, precinct: precinctName || null, transportPrecinct: tp, cfoc: !!L.cfoc, pc120, fds, board: L.board || null, designations: L.designations || [], looked: !!C.site.lookup, at: L.at || null };
  }
  // Extra transport precincts from settings: lines 'I123 | Name | rule'
  function parsePrecincts(text) {
    return String(text || '').split(/\n/).map(l => l.split('|').map(s => s.trim())).filter(a => a[0] && a[1])
      .map(([id, name, rule]) => ({ id, name, match: '^' + name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), rule: rule || `${id} standards`, ita: `${id}.9`, source: { name: `${id} ${name} (added in Settings)`, url: '', verified: false } }));
  }

  /* ====================================================================================================
     6. E27 trigger screening (e27-screening, reg-e27-extend)
     ==================================================================================================== */
  const devZones = sc => Object.values(sc.zones).filter(z => z.kind !== 'gateway' && Scenarios.zoneRole(z) === 'development');
  // Proposal quantities from the development zones (full yield), with the author's overrides; busiest-hour trips from the rates
  function quantities(P, sc = activeSc(P)) {
    const C = state(P), S = P.settings, lu = {}, zones = devZones(sc);
    for (const z of zones) for (const [k, v] of Object.entries(z.lu || {})) lu[k] = (lu[k] || 0) + (+v || 0);
    const sub = C.ctx.pathway === 'rc-subdivision', dt = !!C.site.driveThrough, sec = C.site.school === 'secondary';
    const q = { dwellings: sub ? 0 : (lu.residential || 0) + (lu.apartment || 0), ird: lu.retirement || 0, visitor: lu.hotel || 0, subdivision: sub ? (lu.residential || 0) + (lu.apartment || 0) : 0,
      primary: sec ? 0 : lu.school || 0, secondary: sec ? lu.school || 0 : 0, tertiary: lu.tertiary || 0, office: lu.office || 0,
      retail: (lu.retail || 0) + (lu.supermarket || 0) + (dt ? 0 : lu.fastfood || 0), retailDT: dt ? lu.fastfood || 0 : 0, warehouse: lu.warehouse || 0, industrial: lu.industrial || 0 };
    const src = Object.fromEntries(Object.keys(q).map(k => [k, 'development zones']));
    for (const [k, v] of Object.entries(C.q || {})) if (v != null && v !== '' && isFinite(+v) && k in q) { q[k] = +v; src[k] = 'entered'; }
    /* Two-way trips by period, as Engine.tripEnds loads them: (land use × Settings rate + trips entered directly in
       z.extra, which is also where sourced trip-generation tables go) × (1 − trip reduction), at full build-out
       (staging and opening year ignored). Unlisted: the activities Table E27.6.1.1 does not list; a zone's entered
       trips follow its unlisted share, and count as unlisted when the zone has no land use. */
    const trips = {}, unlisted = {};
    let direct = false, directOnly = false;
    for (const p of ['AM', 'PM']) {
      let t = 0, u = 0;
      for (const z of zones) {
        const red = 1 - U.clamp(+z.reduction || 0, 0, 100) / 100;
        let zl = 0, zu = 0;
        for (const [k, v] of Object.entries(z.lu || {})) {
          const r = ((S.rates || {})[k] || {})[p], L = LANDUSES[k]; if (!r || !L) continue;
          const x = (+v || 0) / L.per * r.rate; zl += x; if (!E27.map[k]) zu += x;
        }
        const ex = (+((z.extra || {})[p + 'in']) || 0) + (+((z.extra || {})[p + 'out']) || 0);
        if (ex) { direct = true; if (!(zl > 0)) directOnly = true; }
        const zt = Math.max(0, zl + ex), share = zl > 0 ? zu / zl : 1;
        t += zt * red; u += zt * share * red;
      }
      trips[p] = U.r1(t); unlisted[p] = U.r1(u);
    }
    const peak = Math.max(0, ...Object.values(trips)), peakP = Object.keys(trips).find(p => trips[p] === peak) || null;
    const unlistedPeak = Math.max(0, ...Object.values(unlisted));
    const unlistedUses = Object.keys(lu).filter(k => lu[k] > 0 && !E27.map[k]).map(k => (LANDUSES[k] || { label: k }).label);
    if (directOnly) unlistedUses.push('Trips entered directly (no land use)');
    return { q, src, lu, zones: zones.length, trips, peak, peakPeriod: peakP, unlistedPeak, unlistedUses, direct,
      basis: `busiest modelled hour (AM or PM) from the trip rates in Settings${direct ? ' plus trips entered directly' : ''}, after trip reductions, at full build-out; indicative unless the rates are replaced` };
  }
  function exemptionOf(zoneName, cfoc, status) {
    if (zoneName && E27.exemptZone.test(zoneName)) return { rule: 'E27.6.1(2)(a)', text: `Exempt E27.6.1(2)(a): ${zoneName}` };
    if (cfoc) return { rule: 'E27.6.1(2)(a)', text: 'Exempt E27.6.1(2)(a): Centre Fringe Office Control' };
    if (zoneName && E27.openSpace.test(zoneName) && status === 'P') return { rule: 'E27.6.1(2)(c)', text: 'Exempt E27.6.1(2)(c): permitted in an H7 open space zone' };
    return null;
  }
  function e27(P, sc = activeSc(P)) {
    const C = state(P), site = siteInfo(P), Q = quantities(P, sc), status = C.ctx.activityStatus || '';
    const rows = E27.rows.map(r => { const q = Q.q[r.key] || 0; return { ...r, q, src: Q.src[r.key], exceeded: q > r.thr }; });
    const out = { version: E27.version, wording: E27.wording, source: E27.source, rows, quantities: Q, site, applies: true, exempt: null, redirect: null, tier: null, notes: [], s104C: E27.s104C, supplementary: false };
    if (site.transportPrecinct) {
      const p = site.transportPrecinct;
      out.applies = false;
      out.redirect = { precinct: p, text: `E27.6.1 does not apply (${p.rule}): use the precinct staging tracker`, cite: `E27.4; ${p.rule}` };
      for (const r of rows) r.consequence = 'Not applied (precinct)';
    } else {
      out.exempt = exemptionOf(site.zone && site.zone.name, site.cfoc, status);
      if (!out.exempt && C.site.zoneTA) out.exempt = { rule: 'E27.6.1(2)(d)', text: 'Exempt E27.6.1(2)(d): the zone or precinct rules already require trip generation effects to be assessed' };
      if (out.exempt) { out.applies = false; for (const r of rows) r.consequence = 'Not applied (exempt)'; }
    }
    const hit = rows.filter(r => r.exceeded);
    // E27.6.1(1)(b): unlisted activities needing controlled or RD consent, where the zone has no transport assessment requirement; never permitted
    const unlisted = C.site.unlisted || Q.unlistedUses.length > 0, peakU = C.site.unlisted ? Q.peak : Q.unlistedPeak;
    const ca = { id: E27.catchAll.id, thr: E27.catchAll.thr, peak: U.r1(peakU), unlisted, status, applies: false, exceeded: false, why: '' };
    if (!out.applies) ca.why = 'Not applied (' + (out.redirect ? 'precinct' : 'exempt') + ').';
    else if (!unlisted) ca.why = 'Every activity is listed in Table E27.6.1.1.';
    else if (status === 'P') ca.why = 'Does not apply to activities permitted in the zone.';
    else if (!['C', 'RD'].includes(status)) ca.why = status ? `Applies only to controlled or restricted discretionary activities (this one is ${String(ACTIVITY[status] || status).toLowerCase()}).` : 'Set the activity status to test it (it applies only to controlled or restricted discretionary activities).';
    else if (C.site.zoneTA) ca.why = 'The zone already requires a transport assessment.';
    else { ca.applies = true; ca.exceeded = peakU > E27.catchAll.thr; ca.why = ca.exceeded ? `Exceeded: ${num(peakU)} veh/h in the busiest modelled hour (more than 100). Restricted discretionary (A3).` : `Not exceeded: ${num(peakU)} veh/h in the busiest modelled hour.`; }
    out.catchAll = ca;
    /* Tier 1 only if every exceeded row is TA1, T1A, T2A or T3A. The catch-all is not one of them, so exceeding it
       (alone or with a tier 1 row) means E27.8.2(3): all modes, peak times (reg-e27-extend (1), e27-screening). */
    if (out.applies && (hit.length || ca.exceeded)) {
      out.tier = !ca.exceeded && hit.every(r => r.tier === 1) ? 1 : 2;
      for (const r of rows) r.consequence = r.exceeded ? `RD (Table E27.4.1 (A3)); ${out.tier === 1 ? 'E27.8.2(3A)' : 'E27.8.2(3)'}` : 'Not exceeded';
      out.exceededText = `${[hit.length ? 'Table E27.6.1.1' : '', ca.exceeded ? 'E27.6.1(1)(b)' : ''].filter(Boolean).join(' and ')} exceeded`;
      out.consequence = out.tier === 1 ? `Restricted discretionary. Tier 1: ${E27.tier1Scope}. Study area defaults to a ${S_(P).vicinity} m walking catchment (${E27.tier1Area.basis}).`
        : `Restricted discretionary. Tier 2: ${E27.tier2Scope}.${ca.exceeded && hit.length && hit.every(r => r.tier === 1) ? ' The E27.6.1(1)(b) catch-all is not a tier 1 row, so the all-mode network assessment applies.' : ''}`;
      out.supplementary = out.tier === 1;
    } else if (out.applies) for (const r of rows) r.consequence = 'Not exceeded';
    // E27.9 notes: 'may' wording only
    if (out.applies && (hit.length || ca.exceeded)) {
      out.notes.push('E27.9(2): the council may require a travel plan.');
      out.notes.push('E27.9(4): the council may require a transport assessment prepared by a suitably qualified transport planner or traffic engineer.');
    }
    if ((status === 'NC' || status === 'U') && Q.peak >= 100) {
      out.notes.push('E27.9(5): may need an ITA under the AT ITA Guidelines in force (a non-complying or unprovided activity generating 100 veh/h or more in any hour).');
      out.notes.push(`E27.9(5) footnote: the AUP names the Auckland Transport ITA Guidelines (2025 Version 2 is the version in force); ${AUTH.footnote}`);
    }
    out.notes.push(E27.s104C + '.');
    // PC120 overlay: both zonings, and whether the exemption differs
    if (site.pc120 && site.pc120.zone) {
      const opName = site.zone && site.zone.name, prName = site.pc120.zone, exOp = !!exemptionOf(opName, site.cfoc, status), exPr = !!exemptionOf(prName, site.cfoc, status);
      out.pc120 = { operative: opName || 'not known', proposed: prName, differs: !!opName && opName !== prName, exemptDiffers: exOp !== exPr,
        text: exOp !== exPr ? `PC120 changes the E27.6.1(2) position: operative ${opName || '?'} is ${exOp ? 'exempt' : 'not exempt'}, PC120 proposed ${prName} would be ${exPr ? 'exempt' : 'not exempt'}. Test both until PC120 is decided (due by June 2027).`
          : opName && opName !== prName ? `PC120 proposes ${prName} (operative: ${opName}). The E27.6.1(2) exemption is the same under both.` : `PC120 shows ${prName}; no zoning change at the site.`, status: E27.pc120Status };
    }
    // E27.2 / E27.3 policy matrix
    const exemptZone = !!exemptionOf(site.zone && site.zone.name, site.cfoc, '') && !(site.zone && E27.openSpace.test(site.zone.name));
    const major = out.tier === 2 || Q.peak >= 100;
    out.matrix = POLICY.map(p => ({ ...p, applies: p.when === 'outside' ? !exemptZone : p.when === 'major' ? status === 'D' && major : true,
      why: p.when === 'outside' ? (exemptZone ? 'Site is in an excluded zone or control' : 'Site is outside the excluded zones') : p.when === 'major' ? (status === 'D' ? (major ? 'Discretionary and major (Spare Construct reading: E27 tier 2 or 100 veh/h or more)' : 'Discretionary, not major') : 'Applies to discretionary proposals') : 'Applies' }));
    return out;
  }

  /* ====================================================================================================
     7. ITA trigger, scope and proportionality (reg-ita-scope)
     ==================================================================================================== */
  // Largest % change on a link between the base scenario and the active one (AT 5%/10% classes where loaded)
  function linkChange(P, sc = activeSc(P)) {
    const base = baseSc(P); if (sc === base) return null;
    let best = null;
    for (const p of ['AM', 'PM']) {
      const w = resOf(sc, p), wo = resOf(base, p); if (!w || !wo) continue;
      const x = typeof Engine.extentTest === 'function' ? Engine.extentTest(sc, wo, w, { scWithout: base }) : null;
      for (const r of (x ? x.rows : [])) if (r.p != null && r.d >= (x.floor || 20) && (!best || r.p > best.p)) best = { p: r.p, name: r.name || 'unnamed road', link: r.link, period: p, included: r.included, cls: r.cls, extent: x };
      if (x && best && best.period === p) best.included = x.links.length;
    }
    return best;
  }
  function shInvolved(P, sc = activeSc(P)) {
    const C = state(P), shIds = new Set(Object.values(sc.links).filter(L => shOf(P, L).sh).map(L => L.id));
    const why = [];
    for (const a of C.access) { const g = accessGeo(P, sc, a); if (g && g.link && shIds.has(g.link.id)) why.push(`access ${a.name || ''} is on a state highway`.replace('  ', ' ')); }
    for (const p of ['AM', 'PM']) {
      const r = resOf(sc, p); if (!r || !r.cls) continue;
      let dv = 0; for (const [k, v] of Object.entries(r.cls.arcs || {})) if (shIds.has(k.slice(0, k.lastIndexOf(':')))) dv = Math.max(dv, v[2] || 0);
      if (dv >= 1) { why.push(`development traffic uses a state highway (up to ${num(dv)} veh/h in the ${p} peak)`); break; }
    }
    const pt = sitePoint(P, sc);
    if (pt && shIds.size) {
      const f = flatAt(pt.lat); let d = Infinity;
      for (const id of shIds) d = Math.min(d, toLine([pt.lat, pt.lng], Net.geom(sc, sc.links[id]), f).d);
      if (d <= SH.noiseM) why.push(`the site is within ${SH.noiseM} m of a state highway (${num(d)} m)`);
    }
    return { involved: why.length > 0, why, inArea: shIds.size > 0, links: [...shIds] };
  }
  function table1(Q) {
    const q = Q.q, res = q.dwellings + q.ird + q.visitor + q.subdivision, edu = q.primary + q.secondary + q.tertiary;
    const amt = { res, edu, office: q.office, retail: q.retail, warehouse: q.warehouse, industrial: q.industrial, other: Q.unlistedPeak };
    const rows = NZTA_T1.rows.map(r => ({ ...r, q: U.r1(amt[r.key] || 0), lShare: (amt[r.key] || 0) / r.lite, fShare: (amt[r.key] || 0) / r.full }));
    const L = rows.reduce((s, r) => s + r.lShare, 0), F = rows.reduce((s, r) => s + r.fShare, 0);
    const dt = q.retailDT > 0, used = rows.filter(r => r.q > 0);
    const res1 = rows.find(r => r.key === 'res');
    let result, text;
    if (dt) { result = 'Full'; text = `${NZTA_T1.driveThrough}: the proposal has a drive-through component.`; }
    else if (F >= 1) { result = 'Full'; text = used.length === 1 ? `ITA Full threshold (${num(used[0].full)}) reached: ${num(used[0].q)} ${used[0].unit}.` : `ITA Full threshold reached in aggregate (${num(F * 100)}% of Full, mixed use assessed proportionately).`; }
    else if (L >= 1) { result = 'Lite'; text = used.length === 1 ? `ITA Lite threshold (${num(used[0].lite)}) reached: ${num(used[0].q)} ${used[0].unit}.` : `ITA Lite threshold reached in aggregate (${num(L * 100)}% of Lite, mixed use assessed proportionately).`; }
    else { result = 'None'; text = `Below the Table 1 ITA Lite thresholds (${num(L * 100)}% of Lite in aggregate).`; }
    return { rows, lite: U.r3(L), full: U.r3(F), result, text, driveThrough: dt, res: res1, source: NZTA_T1.source, order: NZTA_T1.order, label: NZTA_T1.label };
  }
  function depthOf(result, consistent, context) {
    if (result === 'None') return { depth: null, text: 'No Table 2 depth (below Table 1): the district plan and council guidance still apply.' };
    const urbanish = context === 'urban' || context === 'periurban';
    let d;
    if (result === 'Lite') d = consistent ? (context === 'periurban' ? 'Targeted' : 'Basic') : (context === 'rural' ? 'Targeted' : 'Moderate');
    else d = !consistent && urbanish ? 'Comprehensive' : 'Moderate';
    return { depth: d, text: `G00907 Table 2: ${d} (${result}, ${consistent ? 'consistent with' : 'inconsistent with'} zoning, ${({ urban: 'urban', periurban: 'peri-urban', rural: 'rural' })[context] || context}).` };
  }
  function itaScope(P, sc = activeSc(P)) {
    const C = state(P), c = C.ctx, p = PATHWAYS[c.pathway] || null, E = e27(P, sc), Q = E.quantities, r = regime(P), status = c.activityStatus;
    const aup = [];
    if (E.redirect) aup.push(E.redirect.text + '.');
    else if (E.exempt) aup.push(E.exempt.text + '.');
    else if (E.tier) aup.push(`${E.exceededText} (tier ${E.tier}): ${E.consequence}`, E.wording);
    else aup.push('No E27.6.1 trip-generation threshold is exceeded.');
    aup.push(...E.notes.filter(n => /^E27\.9/.test(n)));
    const major = E.tier === 2 || Q.peak >= 100, ch = linkChange(P, sc), shI = shInvolved(P, sc);
    const at = [];
    if (p && p.kind === 'pc') at.push('Plan change (AT ITA v2 scoping table: an ITA is likely)');
    if (p && p.kind === 'nor') at.push(`${p.label} (AT ITA v2: an ITA is likely and should consider alternative sites, routes or methods)`);
    if ((status === 'D' || status === 'NC') && major) at.push(`${ACTIVITY[status]} major development (major: E27 tier 2 or 100 veh/h or more in the busiest hour; ${ASSUMPTION})`);
    if (status === 'D' && major) at.push('E27.3(2): major proposals for discretionary consent prepare an integrated transport assessment');
    if (E.redirect) at.push(`Precinct requirement: an application infringing the staging standard needs an ITA under the AT ITA Guidelines in force (${E.redirect.precinct.ita})`);
    const netFx = (ch && ch.p > 0.05) || Q.peak >= 100;
    // data strings stay raw here; every renderer escapes them once
    if (p && p.kind === 'ftaa' && netFx) at.push(`Fast-track application with network effects (${ch && ch.p > 0.05 ? `+${num(ch.p * 100, 1)}% on ${ch.name}` : `${num(Q.peak)} veh/h`}; AT ITA v2 says the ITA should follow the guidelines)`);
    const nzta = (shI.involved || !inAuckland(P)) ? table1(Q) : null;
    const depth = nzta ? depthOf(nzta.result, c.zoningConsistent !== false, c.context || 'urban') : null;
    // recommended level: busiest-hour trips (Spare Construct assumption bands), raised by the AT, NZTA and E27 results
    let lvl = Q.peak >= LEVEL_TRIPS.cuts[2] ? 3 : Q.peak >= LEVEL_TRIPS.cuts[1] ? 2 : Q.peak >= LEVEL_TRIPS.cuts[0] ? 1 : 0;
    const raise = (n, why) => { if (n > lvl) { lvl = n; reasons.push(why); } }, reasons = [`${num(Q.peak)} veh/h in the busiest hour (bands 20 / 100 / 500 veh/h, ${ASSUMPTION})`];
    if (E.tier) raise(1, `E27 tier ${E.tier} exceeded`);
    if (nzta && nzta.result === 'Lite') raise(1, 'NZTA Table 1 ITA Lite');
    if (nzta && nzta.result === 'Full') raise(2, 'NZTA Table 1 ITA Full');
    if (at.length) raise(2, 'AT ITA v2 likely case');
    if (ch && ch.p > 0.05) raise(1, `link change over 5% (${ch.name})`);
    const crash = crashFlags(P, sc);
    const drivers = [
      ['Net vehicle trips, busiest modelled hour', `${num(Q.peak)} veh/h${Q.peakPeriod ? ` (${Q.peakPeriod})` : ''}`],
      ['Largest % change on a modelled link', ch ? `+${num(ch.p * 100, 1)}% on ${ch.name} (${ch.period})` : 'not computed (needs results for a base and a with-development scenario)'],
      ['Crash flags', crash.text],
      ['State highway involvement', shI.involved ? shI.why.join('; ') : shI.inArea ? 'state highway in the study area, not used by the development in the results' : 'none found'],
      ['Precinct', E.redirect ? `${E.redirect.precinct.id} ${E.redirect.precinct.name}` : (E.site.precinct || 'none')],
    ];
    const level = LEVELS[lvl];
    return { aup, at, nzta, depth, level, levelIndex: lvl, reasons, drivers, change: ch, sh: shI, crash, e27: E, paragraph: proportionality(P, { level, drivers, regime: r, p, nzta, depth }), never: 'E27.6.1 exceedance alone is not stated as an ITA trigger.' };
  }
  function crashFlags(P, sc) {
    if (typeof DataHub === 'undefined' || !((P.data || {}).crashes || []).length) return { n: null, text: 'crash data not loaded' };
    let s = null; try { s = DataHub.safetySummary(sc); } catch { s = null; }
    if (!s) return { n: null, text: 'crash data not loaded' };
    const hi = (s.topLinks || []).filter(l => l.collective && /high/.test(l.collective.band || '')).length + (s.topNodes || []).filter(n => n.band && /high/.test(n.band.band || '')).length;
    return { n: hi, dsi: s.dsi, text: `${hi} link${hi === 1 ? '' : 's'} or junction${hi === 1 ? '' : 's'} in the medium-high or high risk band; ${num(s.dsi)} DSI crashes in the study area (${s.years && s.years.from ? `${s.years.from}–${s.years.to}` : 'CAS window'})` };
  }
  // The proportionality paragraph (author confirms or edits). Names s88(2AA) for consents; FTAA s44 for Fast-track.
  function proportionality(P, o) {
    const C = state(P);
    if (C.proportion.text) return { text: C.proportion.text, edited: true, confirmed: !!C.proportion.confirmed, by: C.proportion.by, date: C.proportion.date };
    const ftaa = o.p && o.p.kind === 'ftaa', trans = o.regime && o.regime.side === 'transition';
    const law = ftaa ? 'section 44 of the Fast-track Approvals Act 2024 (sufficient detail)' : `RMA s88(2AA)–(2AB) and Schedule 4 clause 2(3)(c)${trans ? ', and s18B (succinct documents with sufficient information) as the application is lodged on or after 30 October 2026' : ''}`;
    const d = Object.fromEntries(o.drivers);
    const text = `The depth of this transport assessment is set in proportion to the scale and significance of the proposal's transport effects, as ${law} require${ftaa ? 's' : ''}. ` +
      `The proposal generates about ${d['Net vehicle trips, busiest modelled hour']} of net vehicle trips. The largest change on any modelled link is ${d['Largest % change on a modelled link']}. ` +
      `Crash flags: ${d['Crash flags']}. State highway involvement: ${d['State highway involvement']}. Precinct: ${d['Precinct']}. ` +
      `${o.nzta ? `${o.nzta.text} ${o.depth && o.depth.depth ? o.depth.text + ' ' : ''}` : ''}On that basis a${/^[AEIOU]/.test(o.level) ? 'n' : ''} ${o.level.toLowerCase().replace('ita', 'ITA')} is proposed. [Author to confirm and record agreement at scoping.]`;
    return { text, edited: false, confirmed: !!C.proportion.confirmed, by: C.proportion.by, date: C.proportion.date };
  }
  // Scoping memo items (reg-ita-scope (f)): proposed from the project; each can be locked with agreed by / date
  const SCOPE_ITEMS = [['area', 'Study area'], ['periods', 'Periods'], ['years', 'Assessment years'], ['data', 'Data sources'], ['rates', 'Trip-rate method'], ['committed', 'Committed developments'], ['tools', 'Tools'], ['stakeholders', 'Stakeholders']];
  function scopingMemo(P, sc = activeSc(P)) {
    const C = state(P), b = boundsOf(sc), L = ((P.data || {}).loaded) || {}, A = P.assessment;
    const years = [...new Set(P.scenarios.map(s => (typeof Engine !== 'undefined' ? Engine.yearOf(s, P.settings) : s.year)))].sort().join(', ');
    const data = [['nzta', 'NZTA counts'], ['at', 'AT counts'], ['cas', 'NZTA CAS crashes'], ['fc', 'AT Future Connect'], ['user', 'own counts']].filter(([k]) => L[k] && L[k].at).map(([k, n]) => `${n} (${today(L[k].at)})`).join('; ') || 'none loaded yet';
    const auth = authorities(P);
    const proposed = {
      area: b ? `Network extent ${num(U.dist([b.s, b.w], [b.s, b.e]) / 1000, 1)} × ${num(U.dist([b.s, b.w], [b.n, b.w]) / 1000, 1)} km; extend to links over the AT 5%/10% test` : 'not set',
      periods: (A && A.periods ? A.periods : ['AM', 'PM']).join(', ') + ' peak hour (one hour each)',
      years: years || '–', data, rates: 'Spare Construct indicative peak-hour vehicle trip rates (Settings); replace with sourced, dated rates (AT ITA v2 Appendix 1)',
      committed: (P.register || []).length ? `${P.register.length} register rows; included rows by AT ITA s3.6.1 rules` : 'register empty',
      tools: 'Spare Construct (screening); SIDRA for junctions where required; MSM consistency check where network-wide',
      stakeholders: auth.rows.map(r => r.name).join('; '),
    };
    return SCOPE_ITEMS.map(([k, label]) => { const s = C.scoping[k] || {}; return { key: k, label, value: s.value || proposed[k], proposed: proposed[k], edited: !!s.value, locked: !!s.locked, by: s.by || '', date: s.date || '' }; });
  }

  /* ====================================================================================================
     8. Access and frontage (reg-access)
     ==================================================================================================== */
  // The nearest road; among roads within 1 m of the nearest, an existing one wins (a proposed site road that meets the
  // frontage road at the access is the access itself, not the frontage)
  function nearestLink(sc, ll, f) {
    const all = [];
    for (const L of Object.values(sc.links)) { if (L.status === 'closed' || !sc.nodes[L.a] || !sc.nodes[L.b]) continue; all.push({ L, d: toLine(ll, Net.geom(sc, L), f).d }); }
    if (!all.length) return null;
    const dMin = Math.min(...all.map(x => x.d)), near = all.filter(x => x.d <= dMin + 1);
    return near.find(x => x.L.status !== 'proposed') || near.sort((a, b) => a.d - b.d)[0];
  }
  function accessPoint(sc, a) {
    if (a.node && sc.nodes[a.node]) return [sc.nodes[a.node].lat, sc.nodes[a.node].lng];
    return isFinite(a.lat) && isFinite(a.lng) && a.lat != null ? [+a.lat, +a.lng] : null;
  }
  // Shortest distance from anywhere on the link (not only its midpoint) to a level-crossing control area or VAR line
  function levelCrossingDist(P, sc, L) {
    const o = state(P).overlays; if (!o || !((o.lx || []).length || (o.var || []).some(v => /Level/.test(v.type)))) return null;
    const g = Net.geom(sc, L); if (!g || !g.length) return null;
    const f = flatAt(g[0][0]), lines = (o.var || []).filter(v => /Level/.test(v.type)).map(v => v.pts);
    const d = Math.min(lineToPolys(g, o.lx || [], f, 200), lineToPolys(g, lines, f, 200, true));
    return isFinite(d) ? d : null;
  }
  // Geometry facts for an access point: nearest intersection, frontage link, mapped controls within 15 m, level crossing
  function accessGeo(P, sc, a) {
    const ll = accessPoint(sc, a); if (!ll) return null;
    const f = flatAt(ll[0]), o = state(P).overlays, out = { ll };
    // the access's own node is the crossing, not 'the nearest intersection' (E27.6.4.1(3)(a))
    let dI = Infinity; const deg = degrees(sc);
    for (const n of Object.values(sc.nodes)) { if (n.id === a.node || (deg.get(n.id) || 0) < 3) continue; const d = U.dist(ll, [n.lat, n.lng]); if (d < dI) { dI = d; out.intersection = n.id; } }
    out.intersectionDist = isFinite(dI) ? dI : null;
    // frontage road: the user's choice, else the nearest existing road
    const pick = a.frontageLink && sc.links[a.frontageLink] && sc.links[a.frontageLink].status !== 'closed' ? sc.links[a.frontageLink] : null;
    const nl = pick ? { L: pick, d: toLine(ll, Net.geom(sc, pick), f).d } : nearestLink(sc, ll, f);
    out.link = nl ? nl.L : null; out.linkDist = nl ? nl.d : null; out.linkPicked = !!pick;
    out.controls = [];
    if (o) {
      for (const v of o.var) if (toLine(ll, v.pts, f).d <= ACCESS.nearM) out.controls.push('VAR ' + v.type);
      for (const v of o.frontage) if (toLine(ll, v.pts, f).d <= ACCESS.nearM) out.controls.push(v.type);
      out.arterial = o.arterials.some(p => toLine(ll, p, f).d <= ACCESS.nearM);
      let dx = toPolys(ll, o.lx, f); for (const v of o.var) if (/Level/.test(v.type)) dx = Math.min(dx, toLine(ll, v.pts, f).d);
      out.lxDist = isFinite(dx) ? dx : null;
    }
    if (!out.arterial && out.link && state(P).net.aupArterial[out.link.id]) out.arterial = true;
    out.controls = [...new Set(out.controls)];
    return out;
  }
  function zoneGroup(name) {
    if (!name) return null;
    if (/^Residential/i.test(name) || /Large Lot|Rural and Coastal Settlement/i.test(name)) return 'residential';
    if (/^Rural|Future Urban/i.test(name)) return 'rural';
    if (/General Business|Business Park|Industry/i.test(name)) return 'business';
    return 'centres';
  }
  function accessCheck(P, a, sc = activeSc(P)) {
    const C = state(P), g = accessGeo(P, sc, a) || {}, site = siteInfo(P), flags = [], cityCentre = site.zone && /City Centre Zone/i.test(site.zone.name || '');
    const val = (k, auto) => (a[k] != null && a[k] !== '' && isFinite(+a[k]) ? +a[k] : auto);
    const dInt = val('intersectionDist', g.intersectionDist), dLx = val('lxDist', g.lxDist);
    const controls = new Set(g.controls || []); if (a.var) controls.add('VAR ' + a.var); if (a.krf) controls.add('Key Retail Frontage'); if (a.gcf) controls.add('General Commercial Frontage');
    const artBy = a.arterial === true || a.arterial === 'yes' ? true : a.arterial === false || a.arterial === 'no' ? false : null;
    const arterial = artBy != null ? artBy : !!g.arterial;
    const add = (rule, status, text) => flags.push({ rule, status, text });
    // E27.6.4.1(1): City Centre VAR General or Key Retail Frontage
    const krf = controls.has('Key Retail Frontage'), ccVar = cityCentre && controls.has('VAR General');
    if (krf || ccVar) {
      const ex = a.exception === 'relocate' || a.exception === 'only';
      if (a.existing) add('E27.6.4.1(1)(b)(i)', 'RD', `${krf ? 'Key Retail Frontage' : 'City Centre VAR General'}: use of an existing crossing for a new or intensified activity is restricted discretionary (A6).`);
      else add('E27.6.4.1(1)', ex ? 'RD' : 'NC', ex ? `New crossing on a ${krf ? 'Key Retail Frontage' : 'City Centre VAR General'} to ${a.exception === 'only' ? 'give the only means of access' : 'relocate or amalgamate crossings'}: restricted discretionary (A7).` : `New crossing on a ${krf ? 'Key Retail Frontage' : 'City Centre VAR General'}: non-complying (A8).`);
    }
    // E27.6.4.1(3): within 10 m of an intersection, mapped VAR controls, arterial frontage, within 30 m of a level crossing
    if (dInt != null && dInt < ACCESS.intersectionM) add('E27.6.4.1(3)(a)', 'RD', `${num(dInt, 1)} m from the nearest intersection (under 10 m): restricted discretionary (A5).`);
    for (const c of controls) if (/^VAR /.test(c) && !(cityCentre && c === 'VAR General')) add('E27.6.4.1(3)(b)', 'RD', `Mapped Vehicle Access Restriction (${c.slice(4)}): restricted discretionary (A5).`);
    if (arterial) add('E27.6.4.1(3)(c)', 'RD', 'Frontage to an arterial road on the AUP planning maps: restricted discretionary (A5).');
    if (dLx != null && dLx < ACCESS.levelCrossingM) add('E27.6.4.1(3)(d)', 'RD', `${num(dLx, 1)} m from a level crossing (under 30 m from the limit line): restricted discretionary (A5). KiwiRail is an affected operator.`);
    // Table E27.6.4.2.1: number and separation of crossings ('or part thereof' → round up)
    const frontage = val('frontage', null), n = val('crossings', null);
    let max = null, basis = '';
    if (frontage != null) {
      if (krf || ccVar) { max = 0; basis = 'no crossings permitted (T143)'; }
      else if (controls.size || arterial || dInt != null && dInt < ACCESS.intersectionM || dLx != null && dLx < ACCESS.levelCrossingM || controls.has('General Commercial Frontage')) { max = Math.ceil(frontage / ACCESS.perM.restricted); basis = '1 per 50 m of frontage or part thereof (T144)'; }
      else { max = Math.ceil(frontage / ACCESS.perM.normal); basis = '1 per 25 m of frontage or part thereof (T146)'; }
      if (n != null && n > max) add('Table E27.6.4.2.1', 'RD', `${n} crossings on a ${num(frontage)} m frontage: breach (maximum ${max}, ${basis}).`);
    }
    const sepAdj = val('sepAdj', null), sepSame = val('sepSame', null), comb = val('combinedWidth', null);
    if (sepAdj != null && sepAdj < ACCESS.sepAdj && !(comb != null && comb <= ACCESS.combined)) add('Table E27.6.4.2.1', 'RD', `${num(sepAdj, 1)} m from a crossing on the adjacent site: breach (minimum 2 m unless the combined width is 6 m or less).`);
    if (sepSame != null && sepSame < ACCESS.sepSame) add('Table E27.6.4.2.1', 'RD', `${num(sepSame, 1)} m between crossings on the same site: breach (minimum 6 m).`);
    // Table E27.6.4.3.2 widths
    const grp = a.zoneGroup || zoneGroup(site.zone && site.zone.name) || 'centres', spaces = val('spaces', null), width = val('width', null), formed = val('formed', null);
    const band = spaces != null ? (ACCESS.widths[grp] || ACCESS.widths.centres).find(b => spaces <= b.to) : null;
    if (band && width != null) {
      const mx = a.hv && band.hv ? band.hv : band.max;
      if (width < band.min || width > mx) add(`Table E27.6.4.3.2 (${band.ref})`, 'RD', `Crossing width ${num(width, 2)} m: outside ${num(band.min, 2)}–${num(mx, 2)} m for ${spaces} parking spaces (${grp}).`);
    }
    if (band && band.formed != null && formed != null && formed < band.formed) add(`Table E27.6.4.3.2 (${band.ref})`, 'RD', `Formed access width ${num(formed, 2)} m: under the ${num(band.formed, 2)} m minimum.`);
    // Table E27.6.4.3.1 passing bays
    const len = val('length', null);
    if (len != null) {
      const rural = grp === 'rural', pb = rural ? ACCESS.passing.rural : ACCESS.passing.other;
      const needs = rural ? len > pb.len && (formed == null || formed < pb.width) : len > pb.len && !(formed != null && formed >= 5.5);
      if (needs && !a.passingBays) add(`Table E27.6.4.3.1 (${pb.ref})`, 'RD', `Access ${num(len)} m long${rural ? ' and under 5.5 m wide' : ''}: passing bays needed at no more than ${pb.every} m intervals${rural ? '' : ` (Spare Construct reading: not needed where the formed width is already 5.5 m)`}. Tick 'passing bays provided' if they are.`);
    }
    // E27.6.4.3(2): on-site queue at gates, drive-throughs and dispensers
    const q95 = val('q95', null), stack = val('stacking', null);
    if (q95 != null && stack != null && q95 > stack) add('E27.6.4.3(2)', 'RD', `Queues into road reserve: E27.6.4.3(2) (95th-percentile queue ${num(q95, 1)} m against ${num(stack, 1)} m stacking). Breach, restricted discretionary; matters of discretion E27.8.1(9).`);
    // The map-based tests need the AUP control layers; without them (or the author's own check) they are not 'complies'
    const o = C.overlays, loaded = k => !!(o && (o.sources || []).some(s => s.source === LAYERS[k].name)), byHand = !!a.controlsChecked, unchecked = [];
    if (!byHand) {
      if (!loaded('var') && !a.var) unchecked.push('mapped VAR controls (E27.6.4.1(3)(b))');
      if (!loaded('arterial') && artBy == null && !(g.link && C.net.aupArterial[g.link.id])) unchecked.push('arterial frontage (E27.6.4.1(3)(c))');
      if (!loaded('lx') && !loaded('var') && !(a.lxDist != null && a.lxDist !== '')) unchecked.push('level crossings (E27.6.4.1(3)(d))');
      if (!loaded('frontage') && !a.krf && !a.gcf) unchecked.push('Key Retail and General Commercial Frontage (E27.6.4.1(1), Table E27.6.4.2.1)');
    }
    const worst = flags.some(x => x.status === 'NC') ? 'NC' : flags.length ? 'RD' : unchecked.length ? 'unchecked' : 'ok';
    const uncheckedText = unchecked.length ? `Not checked: ${unchecked.join('; ')}. Load the AUP controls (Compliance › Access), or check them on the planning maps and tick 'AUP controls checked by hand'.` : '';
    const out = { id: a.id, name: a.name || 'Access', geo: g, flags, worst, unchecked, uncheckedText, max, basis, frontageLink: g.link || null, sh: g.link ? shOf(P, g.link).sh : false };
    // the E27.8.2(11)(a) evidence pack scans every road: built only when something reads it (the access card)
    let ev; Object.defineProperty(out, 'evidence', { enumerable: true, get: () => (ev === undefined ? (ev = evidencePack(P, sc, a, g)) : ev) });
    return out;
  }
  // E27.8.2(11)(a) evidence pack for an access
  function evidencePack(P, sc, a, g) {
    if (!g || !g.link) return null;
    const L = g.link, S = P.settings, f = flatAt(g.ll[0]), R = S_(P).evidence, rows = [];
    const vol = p => { const r = resOf(sc, p); return r ? ((r.arcs[L.id + ':ab'] || {}).v || 0) + ((r.arcs[L.id + ':ba'] || {}).v || 0) : null; };
    const spd = p => { const r = resOf(sc, p); if (!r) return null; const x = [r.arcs[L.id + ':ab'], r.arcs[L.id + ':ba']].filter(Boolean); return x.length ? x.reduce((s, y) => s + y.spd, 0) / x.length : null; };
    const am = vol('AM'), pm = vol('PM'), pk = Math.max(am || 0, pm || 0);
    rows.push(['Frontage road', `${L.name || CLASSES[L.cls].label} (${CLASSES[L.cls].label.toLowerCase()})`, 'model network']);
    rows.push(['Speed limit', L.nslr && L.nslr.limit ? `${L.nslr.limit} km/h` : `${L.speed} km/h`, L.nslr && L.nslr.limit ? 'NZTA National Speed Limit Register' : 'road attribute (check the NSLR)']);
    const sp = spd('AM') ?? spd('PM'); rows.push(['Operating speed (modelled)', sp != null ? `${num(sp)} km/h` : 'run the model', 'Spare Construct assignment, indicative']);
    rows.push(['Traffic volume', am != null || pm != null ? `AM ${num(am)} · PM ${num(pm)} veh/h two-way · about ${num(pk * (S.peakToDaily || 10))} veh/day` : 'run the model', `model; daily = busiest peak × ${S.peakToDaily || 10} (Settings)`]);
    rows.push(['Heavy vehicle share', `${num(S.hvPct, 1)}%`, 'Settings default unless counts say otherwise']);
    let crash = 'crash data not loaded';
    try { const lr = typeof DataHub !== 'undefined' && (P.data || {}).crashes && P.data.crashes.length ? DataHub.linkRisk(sc) : null, r = lr && lr[L.id];
      if (r) { const vkm = r.aadt > 0 ? r.aadt * 365 * r.km * r.years : 0; crash = vkm > 0 ? `${num(r.crashes / vkm * 1e8, 1)} crashes per 100 million veh-km (${r.crashes} crashes, ${num(r.years, 1)} years)` : `${r.crashes} crashes; no volume for a rate`; } } catch { }
    rows.push(['Crash rate (CAS)', crash, 'NZTA CAS; most recent months may be incomplete (D8)']);
    rows.push(['Pedestrian count', a.peds != null && a.peds !== '' ? `${a.peds} people/h` : 'enter a count (field survey)', 'field input']);
    const near = (pred) => Object.values(sc.links).filter(x => x.status !== 'closed' && pred(x) && sc.nodes[x.a] && sc.nodes[x.b] && toLine(g.ll, Net.geom(sc, x), f).d <= R);
    const stops = Object.values(sc.routes || {}).filter(r => (r.wp || []).some(id => sc.nodes[id] && U.dist(g.ll, [sc.nodes[id].lat, sc.nodes[id].lng]) <= R));
    rows.push(['Bus routes / stops within ' + R + ' m', stops.length ? stops.map(r => r.name).join(', ') : 'none modelled', `modelled routes' waypoints, not AT stop data (${ASSUMPTION} radius)`]);
    rows.push(['Bus lanes within ' + R + ' m', near(x => x.busLane).length ? 'yes' : 'none modelled', ACCESS.evidenceBasis]);
    rows.push(['Cycleways within ' + R + ' m', near(x => x.cycle && x.cycle !== 'none').length ? 'yes' : 'none modelled', ACCESS.evidenceBasis]);
    return rows;
  }

  /* ====================================================================================================
     9. State highways and NZTA approvals (reg-sh)
     ==================================================================================================== */
  function sisd(speed, measured85) {
    const v = measured85 > 0 ? Math.ceil(measured85 / 10) * 10 : (+speed || 0) + SH.sisdAdd, key = Math.min(120, Math.max(40, Math.round(v / 10) * 10));
    return { design: v, m: SH.sisd[key], basis: measured85 > 0 ? `measured 85th percentile ${measured85} km/h` : `posted ${speed} km/h + ${SH.sisdAdd} km/h`, source: SRC.austroads };
  }
  // Table 9-1: Type C only with fewer than 50 movements a day, at most 1 HCV movement a week AND the SISD met
  function accessType({ daily, peak, hcvWeek, sisdOk }) {
    if (daily > SH.design.daily || peak > SH.design.hourly) return { type: 'design', text: 'intersection design required' };
    if (daily < SH.typeC.daily && (hcvWeek || 0) <= SH.typeC.hcvWeek) {
      if (sisdOk === true) return { type: 'C', text: 'Type C' };
      if (sisdOk === false) return { type: 'E', text: 'Type E (the SISD is not met, so not Type C)' };
      return { type: 'C?', text: 'Type C if the SISD is met (enter the available sight distance); otherwise Type E' };
    }
    return { type: 'E', text: 'Type E' };
  }
  const pavement = (hv0, hv1) => ({ change: hv0 > 0 ? (hv1 - hv0) / hv0 : hv1 > 0 ? Infinity : 0, flagged: hv0 > 0 ? (hv1 - hv0) / hv0 > SH.pavement : hv1 > 0,
    text: `${num(hv0)} → ${num(hv1)} heavy vehicles a day${hv0 > 0 ? ` (${U.fmt.signedPct((hv1 - hv0) / hv0)})` : ''}` });
  // Modelled movements (veh/h, both directions) through an access placed at a node: the zone-connector turns there
  function accessVolume(sc, a) {
    if (!a || !a.node) return null;
    let best = null;
    for (const p of ['AM', 'PM']) {
      const r = resOf(sc, p); if (!r || !r.turns) continue;
      const v = (r.turns[a.node] || []).filter(m => /^c:/.test(m.from) || /^c:/.test(m.to)).reduce((s, m) => s + (+m.v || 0), 0);
      if (!best || v > best.peak) best = { peak: U.r1(v), period: p };
    }
    return best && best.peak > 0 ? best : null;
  }
  /* Zones pinned to motorway nodes (GRPA s84: no private access to a motorway without special authorisation). The
     warning stays (tab, checklist SH-03, a toast when it first appears) until the author confirms it with a reason (reg-sh:
     'requires confirmation'); the network editor itself belongs to the core shell. */
  const mwKey = (zone, node) => `${zone}~${node}`.replace(/[^w~-]/g, '_');   // safe as one path segment
  function motorwayConnectors(sc, P = Store.P) {
    const out = [], ok = (P && isObj(P.compliance) && P.compliance.mwConfirm) || {};
    for (const z of Object.values(sc.zones)) {
      if (z.kind === 'gateway' || !z.node || !sc.nodes[z.node]) continue;
      const ls = roadLinks(sc, z.node);
      if (!(ls.length && ls.every(L => L.cls === 'motorway' || L.cls === 'ramp'))) continue;
      const key = mwKey(z.id, z.node), cf = own(ok, key) && ok[key] && ok[key].confirmed ? ok[key] : null;
      out.push({ zone: z.id, name: z.name || 'A zone', node: z.node, key, confirmed: !!cf, by: cf ? cf.by || '' : '', date: cf ? cf.date || '' : '', reason: (own(ok, key) && ok[key] && ok[key].reason) || '',
        text: `${z.name || 'A zone'} connects straight to a motorway. GRPA s84 allows access to a motorway only by special authorisation where no other road access is reasonably available (NZTA PPM Access guide s4). Move the connector to a local road, or confirm below why none is available.` });
    }
    return out;
  }
  function shCheck(P, sc = activeSc(P)) {
    const C = state(P), inv = shInvolved(P, sc), p = PATHWAYS[C.ctx.pathway], base = baseSc(P), S = P.settings, ptd = S.peakToDaily || 10;
    const acc = C.access.map(a => accessCheck(P, a, sc)).filter(x => x.sh);
    const triggers = [];
    for (const x of acc) { const a = C.access.find(y => y.id === x.id) || {}; triggers.push(a.existing ? (a.changed ? `Changed access: ${x.name}` : `Land-use change at an existing access: ${x.name}`) : `New access: ${x.name}`); }
    if (inv.why.some(w => /development traffic/.test(w))) triggers.push('Nearby development whose traffic may affect the state highway');
    const desig = (siteInfo(P).designations || []).filter(d => /Transport Agency|Waka Kotahi|NZTA|KiwiRail/i.test(d.by + ' ' + d.name));
    const motorwayAcc = acc.some(x => x.frontageLink && x.frontageLink.cls === 'motorway');
    const approvals = [
      { id: 's95E', label: 'Written approval from NZTA as an affected party (RMA s95E(3)(a))', needed: triggers.length > 0 },
      { id: 's176', label: desig.length ? `Requiring authority consent on designated land (RMA s176(1)(b), or s178 if not yet confirmed): ${desig.map(d => `${d.by} ${d.name}`.trim()).join('; ')}` : 'Requiring authority consent on designated land (RMA s176(1)(b) / s178): no NZTA or KiwiRail designation found at the site point; check the planning maps', needed: desig.length > 0, check: !desig.length },
      { id: 'grpa91', label: 'Crossing place authorised by NZTA notice (GRPA s91) and s93 approval before the s224(c) certificate, where the frontage is a limited access road (no open LAR layer: check the LAR status)', needed: acc.length > 0, check: true },
      { id: 'grpa84', label: 'Motorway special access authorisation (GRPA s84)', needed: motorwayAcc },
      { id: 'car', label: `Spare Construct access request to NZTA at least ${SH.carDays} working days before works (PPM Access guide s7.4)`, needed: acc.length > 0 },
    ];
    if (p && p.kind === 'ftaa') approvals.push({ id: 'ftaa', label: 'GRPA crossing-place (s91) and s93 approvals and corridor access approvals are outside Fast-track (FTAA s42(4)): seek them from NZTA separately', needed: acc.length > 0 || triggers.length > 0 });
    // impact table per state highway link with and without
    const shIds = inv.links, rows = [], dh = C.ctx.devHvPct, devHv = dh != null && dh !== '' && isFinite(+dh) ? U.clamp(+dh, 0, 100) : null;
    for (const id of shIds) {
      const L = sc.links[id] || base.links[id]; if (!L) continue;
      const r = { id, name: L.name || shOf(P, L).ref, ref: shOf(P, L).ref, per: {} };
      for (const per of ['AM', 'PM']) {
        const w = resOf(sc, per), wo = sc === base ? null : resOf(base, per), v = res => res ? ((res.arcs[id + ':ab'] || {}).v || 0) + ((res.arcs[id + ':ba'] || {}).v || 0) : null;
        r.per[per] = { with: v(w), without: v(wo) };
      }
      const pk = k => Math.max(r.per.AM[k] || 0, r.per.PM[k] || 0);
      r.dayWith = pk('with') * ptd; r.dayWithout = r.per.AM.without != null || r.per.PM.without != null ? pk('without') * ptd : null;
      r.hvPct = S.hvPct;
      // The model carries one HV share for all traffic, so the pavement test is a total-traffic proxy unless the
      // author gives the development's own HV share (applied to the added traffic only)
      if (r.dayWithout != null) {
        const hv0 = r.dayWithout * S.hvPct / 100, hv1 = hv0 + (r.dayWith - r.dayWithout) * (devHv != null ? devHv : S.hvPct) / 100;
        r.pavement = { ...pavement(hv0, hv1), proxy: devHv == null };
      } else r.pavement = null;
      if (r.per.AM.with != null || r.per.PM.with != null) rows.push(r);
    }
    rows.sort((a, b) => (b.dayWith - (b.dayWithout || 0)) - (a.dayWith - (a.dayWithout || 0)));
    const nodes = [], shNodes = new Set(shIds.flatMap(id => { const L = sc.links[id]; return L && L.status !== 'closed' ? [L.a, L.b] : []; }));
    for (const per of ['AM', 'PM']) {
      const w = resOf(sc, per), wo = sc === base ? null : resOf(base, per); if (!w) continue;
      for (const [nid, n] of Object.entries(w.nodes || {})) if (shNodes.has(nid)) nodes.push({ id: nid, period: per, name: Kit.junctionName(nid, sc), d1: n.delay, d0: wo && wo.nodes[nid] ? wo.nodes[nid].delay : null, los: n.los });
    }
    const typed = v => v != null && v !== '' && isFinite(+v);
    const tech = acc.map(x => {
      const a = C.access.find(y => y.id === x.id) || {}, L = x.frontageLink, sd = sisd(L.nslr && L.nslr.limit || L.speed, +a.speed85 || 0);
      const avail = a.sightAvail != null && a.sightAvail !== '' ? +a.sightAvail : null, ok = avail == null ? null : avail >= sd.m;
      // movements through the access: the author's figures, else the modelled zone-connector turns at the access node
      const mod = accessVolume(sc, a), hcvShare = +a.hcvPct || 0;
      const peak = typed(a.peak) ? +a.peak : mod ? mod.peak : 0, daily = typed(a.daily) ? +a.daily : mod ? mod.peak * ptd : 0;
      const volBasis = typed(a.daily) || typed(a.peak) ? 'entered' : mod ? `modelled: ${num(mod.peak)} veh/h in the ${mod.period} peak through the access node; a day = busiest peak × ${ptd} (Settings), indicative` : 'no movements entered or modelled';
      const pcuNote = hcvShare > SH.hcvPcu.share ? `HCV share ${hcvShare}% is over 20%: count each HCV as ${SH.hcvPcu.pcu} PCU` : '';
      const factor = hcvShare > SH.hcvPcu.share ? 1 + hcvShare / 100 * (SH.hcvPcu.pcu - 1) : 1;
      return { id: x.id, name: x.name, road: L.name || shOf(P, L).ref, posted: L.nslr && L.nslr.limit || L.speed, sisd: sd, available: avail, sisdOk: ok, daily: U.r1(daily), peak: U.r1(peak), volBasis,
        type: accessType({ daily: daily * factor, peak: peak * factor, hcvWeek: +a.hcvWeek || 0, sisdOk: ok }), pcuNote,
        spacing: 'Intersection and access spacing (Tables 8-2 and 8-3): check by hand; Spare Construct has not encoded them', crashYears: SH.crashYears };
    });
    const noise = inv.why.find(w => /within/.test(w)) && ['residential', 'apartment', 'retirement', 'hotel', 'school', 'childcare', 'medical'].some(k => devZones(sc).some(z => (z.lu || {})[k] > 0));
    return { involved: inv.involved, why: inv.why, inArea: inv.inArea, triggers, approvals, rows, nodes, tech, noise: !!noise, motorway: motorwayConnectors(sc, P),
      devHvPct: devHv, pavementBasis: devHv == null ? `Heavy vehicles a day from one HV share for all traffic (${num(S.hvPct, 1)}%, Settings): a total-traffic proxy. Enter the development's HV share, or use an HV forecast, for the G00907 s5.4.6 test.` : `Heavy vehicles a day: existing traffic at ${num(S.hvPct, 1)}% (Settings), added traffic at the development's ${num(devHv, 1)}% (entered).`,
      labels: [SH.itaLabel, SH.accessLabel], loaded: C.net.loaded.sh || null };
  }

  /* ====================================================================================================
     10. FDS consistency (reg-fds)
     ==================================================================================================== */
  function fdsCheck(P, sc = activeSc(P)) {
    const C = state(P), site = siteInfo(P), r = regime(P), p = PATHWAYS[C.ctx.pathway];
    const occ = +C.ctx.occupation || (P.assessment && P.assessment.opening) || (typeof Engine !== 'undefined' ? Engine.yearOf(sc, P.settings) : null);
    const rsp = C.fds.rsp && C.fds.rsp.name ? C.fds.rsp : null, ref = rsp ? `${rsp.name}${rsp.date ? ` (decided ${longDate(rsp.date)})` : ''}` : 'the Auckland Future Development Strategy 2023–2053';
    const f = site.fds, timing = f ? String(f.timing || '') : '', start = (/(\d{4})\s*\+/.exec(timing) || [])[1] ? +(/(\d{4})\s*\+/.exec(timing))[1] : null;
    let status = 'unknown', text;
    if (!site.looked && !f) text = 'Look up the site to read the FDS future urban areas layer, or enter the FDS area and timing by hand.';
    else if (!f) { status = 'outside'; text = 'The site is not in an FDS future urban area (existing urban area or outside the strategy\'s growth areas).'; }
    else if (/removal/i.test(timing)) { status = 'removal'; text = `${f.name}: the FDS marks this area for removal from future urbanisation.`; }
    else if (/live zoned/i.test(timing)) { status = 'live'; text = `${f.name}: already live zoned in the FDS.`; }
    else if (start && occ && occ < start) { status = 'out'; text = `${f.name}: FDS timing ${timing}, proposed occupation ${occ}: out of sequence.`; }
    else if (start) { status = 'in'; text = `${f.name}: FDS timing ${timing}, proposed occupation ${occ || '?'}: in sequence.`; }
    else text = `${f.name}: FDS timing ${timing || 'not stated'}.`;
    const red = /red flag/i.test(timing);
    const infra = (C.fds.infra || []).map(i => { const row = (P.register || []).find(x => x.id === i.reg); const inc = row ? Register.inBaseline(row) : null;
      return { ...i, name: i.name || (row && row.name) || 'Unnamed project', funding: row && row.funding ? FUNDING[row.funding] || row.funding : 'not recorded', included: inc ? inc.included : null, reason: inc ? inc.reason : 'not in the register' }; });
    /* s104(1D) governs resource consent decisions and s171(1D) / s168A(3C) notices of requirement, for lodgements
       from 30 Oct 2026 (Fast-track: through FTAA Sch 5, to verify). Plan changes and non-RMA pathways get the AT ITA
       s5.1 policy-context paragraph: s104 does not apply to a plan change. */
    const trans = r.side === 'transition' || r.side === 'after', nor = p && p.proc === 'nor', consent = p && p.proc === 'consent', ftaa = p && p.kind === 'ftaa';
    const cite = !trans ? null : nor ? (C.ctx.raCouncil ? 's168A(3C)' : 's171(1D)') : consent ? 's104(1D)' : null;
    const heading = cite ? `Consistency with ${rsp ? 'the regional spatial plan' : 'the Future Development Strategy'} (RMA ${cite})` : 'Policy context: AT ITA s5.1 out-of-sequence development';
    const page = f && f.page ? ` (${f.source || SRC.fds.name}, p ${f.page})` : '';
    const who = ftaa ? 'the panel to take into account (through FTAA Sch 5; verify)' : nor ? 'the territorial authority to consider' : 'the consent authority to have regard to';
    const body = `${cite ? `RMA ${cite}, inserted by the Planning Act 2026 Sch 12 Pt 2, requires ${who} whether the proposal is consistent with ${ref}.` : `The AT ITA Guidelines 2025 v2 (s5.1) ask out-of-sequence development to show how its transport needs will be met, with ${ref} as the reference.`} ` +
      `${text}${page}${red ? ' The FDS red-flags this area.' : ''} ${infra.length ? `The proposal relies on ${infra.length} FDS-assumed infrastructure item${infra.length === 1 ? '' : 's'}: ${infra.map(i => `${i.name} (${i.funding})`).join('; ')}.` : 'No FDS-assumed infrastructure has been listed yet.'} [Author to complete.]`;
    return { status, text, occupation: occ, timing, start, red, infra, cite, heading, paragraph: body, outOfSequence: status === 'out', fundingPrompt: status === 'out' || status === 'removal' ? 'Explain how the capital and operating funding gap for the infrastructure this site needs early will be met (AT ITA s5.1).' : '', reference: ref,
      applies: !p || !!p.proc, source: f ? f.src || null : null };
  }
  // Paired runs with and without the FDS-assumed infrastructure (links of the register rows), not saved
  async function fdsPair(P, opts = {}) {
    const C = state(P), sc = opts.sc || activeSc(P), period = opts.period || P.settings.period || 'AM';
    const ids = new Set((C.fds.infra || []).flatMap(i => ((P.register || []).find(r => r.id === i.reg) || {}).links || []).concat((C.fds.infra || []).flatMap(i => i.links || [])));
    if (!ids.size) throw new Error('List the FDS-assumed infrastructure and link it to register rows with roads (or give its road ids) first.');
    const without = Scenarios.copy(sc, { links: L => !ids.has(L.id) }); without.year = sc.year; without.name = sc.name + ' without FDS infrastructure';
    const a = await Engine.assign(sc, { period }), b = await Engine.assign(without, { period });
    const K = [['trips', 'Vehicle trips (veh/h)', 0], ['vht', 'Vehicle hours (veh·h)', 1], ['dly', 'Delay (veh·h)', 1], ['over', 'Roads over capacity', 0], ['spd', 'Average speed (km/h)', 1]];
    const rows = K.map(([k, l, d]) => ({ key: k, label: l, with: U.r1(a.kpi[k]), without: U.r1(b.kpi[k]), d: U.r1(b.kpi[k] - a.kpi[k]), dp: d }));
    rows.push({ key: 'worst', label: 'Worst junction delay (s)', with: a.kpi.worst ? U.r1(a.kpi.worst.delay) : null, without: b.kpi.worst ? U.r1(b.kpi.worst.delay) : null, d: a.kpi.worst && b.kpi.worst ? U.r1(b.kpi.worst.delay - a.kpi.worst.delay) : null, dp: 0 });
    C.fds.pair = { at: today(opts.now), period, removed: [...ids], rows, gap: [a.gap, b.gap] };
    return C.fds.pair;
  }

  /* ====================================================================================================
     11. RFI linter and statutory clocks (reg-rfi-clock)
     ==================================================================================================== */
  function lint(P) {
    const C = state(P), S = P.settings, d = P.data || {}, out = [], sc = activeSc(P), add = (id, text, fix) => out.push({ id, text, fix });
    const counts = d.counts || [];
    const undated = counts.filter(c => !c.date && c.src !== 'nzta');
    if (undated.length) add('count-date', `${undated.length} count${undated.length === 1 ? '' : 's'} without a date${undated.some(c => !c.src) ? ' or source' : ''} (count without date)`, 'Record the survey date and source for each count (AT ITA v2 s4.2).');
    if (counts.some(c => !c.src)) add('count-source', 'Counts without a source', 'Record where each count came from.');
    const hrs = counts.map(c => /^(\d{2}):/.exec(c.peakTime || '')).filter(Boolean).map(m => +m[1]);
    if (hrs.length >= 3) {
      const mode = [...hrs.reduce((m, h) => m.set(h, (m.get(h) || 0) + 1), new Map())].sort((a, b) => b[1] - a[1])[0][0];
      const modelled = new Set(); for (const s of P.scenarios) for (const p of periodsOf(s)) modelled.add(p);
      const inAM = mode >= 6 && mode <= 9, inPM = mode >= 15 && mode <= 18;
      if (!((inAM && (modelled.has('AM') || !modelled.size)) || (inPM && (modelled.has('PM') || !modelled.size)))) add('peak', `The observed peak in the counts is around ${pad2(mode)}:00 (${hrs.length} counts), which is not one of the modelled periods (AM 06–09, PM 15–18; ${ASSUMPTION})`, 'Assess the development and network peaks (AT ITA v2 s3.6.1(vi)); consider an inter-peak or weekend period.');
    }
    if (+S.hvPct === defaultSettings().hvPct && !counts.some(c => c.hv > 0)) add('hv', `Heavy vehicle share is the Spare Construct default (${S.hvPct}%) and no count gives one`, 'Use classified counts for the HV share.');
    const altered = new Set();
    for (const n of Object.values(sc.nodes)) if (n.upgrade) altered.add(n.id);
    for (const L of Object.values(sc.links)) if (L.status === 'proposed') for (const id of [L.a, L.b]) if (roadLinks(sc, id).length >= 3) altered.add(id);
    const noPeds = [...altered].filter(id => !(C.peds[id] > 0));
    if (noPeds.length) add('peds', `${noPeds.length} altered junction${noPeds.length === 1 ? '' : 's'} without pedestrian volumes`, 'Enter a pedestrian count for each altered junction (select the junction › Compliance).');
    const ms = typeof Engine !== 'undefined' && Engine.modelStatus ? Engine.modelStatus(P, 'AM') : null;
    if (!ms || ms.status === 'uncalibrated') add('base', 'The base model is uncalibrated', 'Fit the matrix to counts and validate against independent counts (TMDG 2019).');
    const roles = new Set(P.scenarios.map(s => s.role));
    if (!roles.has('sensitivity')) add('sens', 'No sensitivity scenario', 'Run the sensitivity pack or add a sensitivity scenario (Practice Note s9.3(a)(x); AT ITA v2 s3.6.1(v)).');
    if (!P.scenarios.some(s => Object.values(s.zones).some(z => Scenarios.zoneRole(z) === 'committed')) && !(P.register || []).length) add('cumulative', 'No cumulative (committed development) scenario', 'Add committed developments to the register and a future baseline (AT ITA v2 s3.6.1).');
    const used = new Set(); for (const z of devZones(sc)) for (const [k, v] of Object.entries(z.lu || {})) if (+v > 0) used.add(k);
    const noSrc = [...used].filter(k => { const r = (S.rates || {})[k] || {}; return !(r.src || r.source) || !(r.year > 0); });
    if (noSrc.length) add('rates', `Trip rates without a source or year: ${noSrc.map(k => (LANDUSES[k] || { label: k }).label).join(', ')}`, 'Record the source and survey year of each rate used (AT ITA v2 Appendix 1).');
    const old = [...used].filter(k => { const r = (S.rates || {})[k] || {}; return r.year > 0 && r.year <= 2022 && !r.reviewed; });
    if (old.length) add('rates-old', `Pre-2020 or 2020–22 trip rates not reviewed: ${old.map(k => (LANDUSES[k] || { label: k }).label).join(', ')}`, 'Review older rates against post-2022 travel patterns (AT ITA v2 s3.6.1).');
    const tdm = devZones(sc).filter(z => +z.reduction > 0 && !z.reductionSrc);
    if (tdm.length) add('tdm', `${tdm.length} zone${tdm.length === 1 ? '' : 's'} with a WFH, TDM or mode-shift discount without evidence`, 'Give evidence for each trip discount (AT ITA v2 s3.6.1(iv)).');
    const blocks = P.scenarios.filter(s => s.role !== 'sensitivity').flatMap(s => Scenarios.check(s, P).filter(x => x.level === 'block' && x.code === 'unfunded').map(x => ({ s, x })));
    if (blocks.length) add('unfunded', `Unfunded projects in a do-minimum or option scenario (${blocks.length})`, 'Unfunded projects belong in sensitivity scenarios only (AT ITA s3.6.1).');
    return out;
  }
  /* Working days. kind 'rma' (RMA s2: 20 Dec – 10 Jan excluded) or 'ftaa' (Legislation Act 2019 s13: 25 Dec – 2 Jan,
     plus the Monday (and Tuesday) after a 1 January that falls on a Friday or weekend). Both exclude weekends, Waitangi
     Day, Good Friday, Easter Monday, Anzac Day, the Sovereign's birthday, Matariki and Labour Day, with Waitangi and
     Anzac Mondayised. */
  function easter(y) {
    const a = y % 19, b = Math.floor(y / 100), c = y % 100, d = Math.floor(b / 4), e = b % 4, f = Math.floor((b + 8) / 25), g = Math.floor((b - f + 1) / 3);
    const h = (19 * a + b - d - g + 15) % 30, i = Math.floor(c / 4), k = c % 4, l = (32 + 2 * e + 2 * i - h - k) % 7, m = Math.floor((a + 11 * h + 22 * l) / 451);
    const mo = Math.floor((h + l - 7 * m + 114) / 31), da = ((h + l - 7 * m + 114) % 31) + 1;
    return Date.UTC(y, mo - 1, da);
  }
  const nthMonday = (y, m0, n) => { const first = new Date(Date.UTC(y, m0, 1)).getUTCDay(), off = (8 - first) % 7; return Date.UTC(y, m0, 1 + off + 7 * (n - 1)); };
  const holCache = new Map();
  function holidays(y) {
    if (holCache.has(y)) return holCache.get(y);
    const H = new Map(), put = (t, n) => H.set(isoUTC(t), n);
    const mond = (t, n) => { put(t, n); const w = new Date(t).getUTCDay(); if (w === 6) put(t + 2 * DAY, n + ' (observed)'); if (w === 0) put(t + DAY, n + ' (observed)'); };
    mond(Date.UTC(y, 1, 6), 'Waitangi Day'); const E = easter(y); put(E - 2 * DAY, 'Good Friday'); put(E + DAY, 'Easter Monday');
    mond(Date.UTC(y, 3, 25), 'Anzac Day'); put(nthMonday(y, 5, 1), "Sovereign's birthday"); put(nthMonday(y, 9, 4), 'Labour Day');
    if (MATARIKI[y]) put(parseDay(`${y}-${MATARIKI[y]}`), 'Matariki'); else H.set('matariki-unknown', String(y));
    holCache.set(y, H); return H;
  }
  function excluded(t, kind = 'rma') {
    const d = new Date(t), w = d.getUTCDay(), y = d.getUTCFullYear(), m = d.getUTCMonth() + 1, day = d.getUTCDate(), W = WD[kind] || WD.rma;
    if (w === 0 || w === 6) return w === 0 ? 'Sunday' : 'Saturday';
    const h = holidays(y).get(isoUTC(t)); if (h) return h;
    if ((m === W.from[0] && day >= W.from[1]) || (m === W.to[0] && day <= W.to[1])) return kind === 'ftaa' ? 'Christmas and New Year period (25 Dec – 2 Jan)' : 'Christmas period (20 Dec – 10 Jan)';
    if (W.newYear && m === 1) {   // Legislation Act s13: if 1 Jan is a Friday, the following Monday; if a Saturday or Sunday, the following Monday and Tuesday
      const j1 = new Date(Date.UTC(y, 0, 1)).getUTCDay();
      if (j1 === 5 && day === 4) return 'New Year (1 Jan fell on a Friday)';
      // Saturday: Monday 3 and Tuesday 4 Jan. Sunday: Monday 2 (already in 25 Dec – 2 Jan) and Tuesday 3 Jan
      if ((j1 === 6 && (day === 3 || day === 4)) || (j1 === 0 && day === 3)) return 'New Year (1 Jan fell on a weekend)';
    }
    return null;
  }
  const isWorkingDay = (s, kind) => { const t = parseDay(s); return t != null && !excluded(t, kind); };
  function addWorkingDays(s, n, kind = 'rma') {
    let t = parseDay(s); if (t == null) return null;
    const step = n >= 0 ? 1 : -1; let left = Math.abs(n), guard = 0;
    while (left > 0 && guard++ < 2000) { t += step * DAY; if (!excluded(t, kind)) left--; }
    return isoUTC(t);
  }
  function workingDaysBetween(a, b, kind = 'rma') { let t = parseDay(a), e = parseDay(b), n = 0; if (t == null || e == null) return null; const s = e >= t ? 1 : -1; while (t !== e) { t += s * DAY; if (!excluded(t, kind)) n += s; } return n; }
  function addMonths(s, n) { const t = parseDay(s); if (t == null) return null; const d = new Date(t), y = d.getUTCFullYear(), m = d.getUTCMonth() + n, day = d.getUTCDate(); const last = new Date(Date.UTC(y, m + 1, 0)).getUTCDate(); return isoUTC(Date.UTC(y, m, Math.min(day, last))); }
  function clockFor(ev) {
    const def = CLOCKS[ev.kind]; if (!def || !validDay(ev.date)) return null;
    const kind = def.kind, out = [], warns = [];
    if (holidays(+ev.date.slice(0, 4)).has('matariki-unknown') || holidays(+ev.date.slice(0, 4) + 1).has('matariki-unknown')) warns.push('Matariki is not in the Act\'s schedule for this year: check the dates by hand.');
    for (const r of def.rules || []) {
      if (r.months) { const prev = out.find(o => o.id === r.after); if (prev) out.push({ id: r.id, date: addMonths(prev.date, r.months), text: r.text }); continue; }
      if (r.max) {
        const limit = addWorkingDays(ev.date, r.max, kind), set = validDay(ev.set) ? ev.set : limit;
        if (set > limit) warns.push(`The panel-set date ${longDate(set)} is more than ${r.max} working days after the direction (latest ${longDate(limit)}, s67(3)).`);
        out.push({ id: r.id, date: set, limit, text: r.text });
        continue;
      }
      out.push({ id: r.id, date: addWorkingDays(ev.date, r.n, kind), text: r.text });
    }
    for (const r of def.back || []) out.push({ id: r.id, date: addWorkingDays(ev.date, -r.n, kind), text: r.text });
    return { ...ev, label: def.label, wd: WD[kind].name, rows: out, warns };
  }
  function clocks(P) {
    const C = state(P), ev = (C.rfi.events || []).slice();
    if (validDay(C.rfi.hearing) && !ev.some(e => e.kind === 'hearing')) ev.push({ id: 'hearing', kind: 'hearing', date: C.rfi.hearing });
    if (validDay(C.rfi.worksDate)) ev.push({ id: 'works', kind: 'works', date: C.rfi.worksDate });
    if (validDay(C.ctx.lodgement) && C.ctx.lodgementActual && !ev.some(e => e.kind === 'lodged') && PATHWAYS[C.ctx.pathway] && PATHWAYS[C.ctx.pathway].kind === 'rc') ev.push({ id: 'lodged', kind: 'lodged', date: C.ctx.lodgement });
    return ev.map(clockFor).filter(Boolean).sort((a, b) => a.date < b.date ? -1 : 1);
  }
  function rfiTemplate(P) {
    const r = regime(P), p = PATHWAYS[state(P).ctx.pathway], s2aa = r.side === 'transition' && p && p.kind === 'rc' && state(P).ctx.pathway !== 'rc-subdivision';
    return `Response to the request for further information dated [date]. The applicant has provided the information below in proportion to the scale and significance of the transport effects (RMA s92(2B)).` +
      (s2aa ? ' Under s92(2AA) the council may not request information about effects that must be disregarded under s104(1A) for a district land use consent lodged on or after 30 October 2026; items [x] are outside that scope.' : '') +
      ' [For each item: the request, the response, and where the evidence is in the assessment.]';
  }

  /* ====================================================================================================
     12. Expert disclosure, sign-off and assumption diff (reg-expert)
     ==================================================================================================== */
  function inputsOf(P) {
    const S = { ...(P.settings || {}) }; for (const k of ['period', 'basemap', 'linzKey', 'sim']) delete S[k];
    return JSON.stringify({ s: S, sc: P.scenarios.map(sc => ({ id: sc.id, n: sc.nodes, l: sc.links, z: sc.zones, r: sc.routes, od: sc.odManual || null, y: sc.year ?? null, ro: sc.role ?? null, b: sc.buildout ?? null, p: sc.periods ?? null, si: sc.sites ?? null })),
      me: P.me || null, reg: P.register || null, c: ((P.data || {}).counts || []).map(c => [c.id, c.adt, c.am, c.pm, c.peak, c.date || null]) });
  }
  const fingerprint = P => hash(inputsOf(P));
  const ROLES = { prepared: 'Prepared by', reviewed: 'Reviewed by', approved: 'Approved by' };
  function signOff(P, who = {}, now) {
    const C = state(P), name = String(who.name || C.expert.author.name || '').trim(), quals = String(who.quals || C.expert.author.quals || '').trim(), role = ROLES[who.role] ? who.role : 'prepared';
    if (!name || !quals) throw new Error('Enter the name and qualifications of the person signing off.');
    const s = { role, name, quals, date: today(now), fp: fingerprint(P) };
    C.expert.signoffs = C.expert.signoffs.filter(x => x.role !== role).concat(s);
    C.expert.supplementary = null;
    return s;
  }
  function signState(P) {
    const C = state(P), fp = C.expert.signoffs.length ? fingerprint(P) : null;
    const valid = C.expert.signoffs.filter(s => s.fp === fp), stale = C.expert.signoffs.filter(s => s.fp !== fp);
    const signed = valid.some(s => s.role === 'prepared');
    return { signed, valid, stale, watermark: signed ? '' : DRAFT, supplementary: C.expert.supplementary,
      prompt: stale.length || C.expert.supplementary ? 'Inputs changed after sign-off, so the sign-off is cleared. Has the opinion changed? A supplementary statement may be needed (Practice Note s9.3(f)).' : '' };
  }
  // Clear sign-offs whose inputs changed; keep them in history (called from the store hook)
  function clearStale(P, now) {
    const C = state(P); if (!C.expert.signoffs.length) return 0;
    const fp = fingerprint(P), stale = C.expert.signoffs.filter(s => s.fp !== fp); if (!stale.length) return 0;
    C.expert.history.push(...stale.map(s => ({ ...s, cleared: today(now), reason: 'inputs changed after sign-off' })));
    C.expert.signoffs = C.expert.signoffs.filter(s => s.fp === fp);
    C.expert.supplementary = { at: today(now), text: 'Inputs changed after sign-off. Has the opinion changed? A supplementary statement may be needed (Practice Note s9.3(f)).' };
    return stale.length;
  }
  // Every data source with its date (Appendix G and the data vintage table)
  function dataSources(P) {
    const L = ((P.data || {}).loaded) || {}, C = state(P), rows = [];
    const N = { nzta: 'NZTA state highway counts (TMS)', at: 'Auckland Transport traffic counts', cas: 'NZTA Crash Analysis System', user: 'Counts imported from a file', fc: 'AT Future Connect network classes', controlled: 'AT controlled intersections',
      osm: 'Road network (OpenStreetMap contributors, ODbL, via the Overpass API) as imported and edited' };
    for (const [k, v] of Object.entries(L)) if (v && v.at) rows.push({ source: v.source || N[k] || k, date: today(v.at), detail: [v.n != null ? `${v.n} records` : '', v.label || '', v.licence || ''].filter(Boolean).join(' · '), basis: k === 'user' ? 'user' : 'measured' });
    const site = C.site.lookup; if (site) for (const s of site.sources || []) rows.push({ source: s.source, date: s.date, detail: s.licence, basis: 'measured' });
    if (C.overlays) for (const s of C.overlays.sources || []) rows.push({ source: s.source, date: s.date, detail: s.licence, basis: 'measured' });
    if (C.net.loaded.sh) rows.push({ source: C.net.loaded.sh.source, date: C.net.loaded.sh.date, detail: `${C.net.loaded.sh.n} links matched · ${C.net.loaded.sh.licence}`, basis: 'measured' });
    // the road network and the trip rates carry a date too (reg-expert: every data source with its date)
    if (!(L.osm && L.osm.at)) {
      const osm = P.scenarios.some(s => Object.values(s.links).some(l => l.osm));
      rows.push({ source: 'Road network (OpenStreetMap contributors, ODbL) as imported and edited', date: osm ? 'import date not recorded' : 'drawn in Spare Construct', detail: 'Spare Construct network', basis: osm ? 'derived' : 'user' });
    }
    const S = P.settings || {}, used = new Set();
    for (const s of P.scenarios) for (const z of Object.values(s.zones)) if (z.kind !== 'gateway') for (const [k, v] of Object.entries(z.lu || {})) if (+v > 0) used.add(k);
    for (const k of used) {
      const r = (S.rates || {})[k] || {}, src = r.src || r.source, yr = r.year > 0 ? String(r.year) : null;
      rows.push({ source: `Trip rate: ${(LANDUSES[k] || { label: k }).label}`, date: yr ? `survey year ${yr}` : src ? 'year not recorded' : `Spare Construct default (as at ${longDate(VERSION)})`,
        detail: src ? String(src) : 'Spare Construct indicative rate (Settings); replace with a sourced, dated rate', basis: src ? 'user' : 'assumed' });
    }
    if (!used.size) rows.push({ source: 'Trip rates (Settings)', date: `Spare Construct default (as at ${longDate(VERSION)})`, detail: 'no land use in the zones yet', basis: 'assumed' });
    rows.push({ source: 'Spare Construct regulatory data (thresholds, rules and disclaimers)', date: longDate(VERSION), detail: 'sources and verified flags in Appendix A', basis: 'derived' });
    return rows;
  }
  function appendixG(P) {
    const C = state(P), E = C.expert, st = signState(P), ms = ['AM', 'PM'].map(p => Engine.modelStatus(P, p)), st0 = stamp(P);
    const sens = P.scenarios.filter(s => s.role === 'sensitivity').map(s => s.name);
    return {
      title: 'Appendix G: Expert disclosure material', disclaimer: DISCLAIMERS.D2.text,
      sections: [
        ['Qualifications', E.author.name ? `${E.author.name}${E.author.quals ? ', ' + E.author.quals : ''}${E.author.org ? ' (' + E.author.org + ')' : ''}` : '[Author to enter name and qualifications]'],
        ['Data, facts and assumptions relied on', dataSources(P).map(r => `${r.source} (${r.date}; ${r.basis}${r.detail ? '; ' + r.detail : ''})`).join('\n')],
        ['Literature and standards, with editions', st0.lines.slice(2).map(([a, b]) => `${a}: ${b}`).join('\n')],
        ['Tests run and by whom', E.tests || '[Author to list the tests and who ran them]'],
        ['Model description and status', `Spare Construct static assignment (conjugate Frank-Wolfe), HCM 6 / Akcelik-based junction delay (indicative, not SIDRA output). ${ms.map(m => m.footer).join('; ')}. ${Engine.STANDARDS.tmdg.standing}`],
        ['Sensitivity and uncertainty', sens.length ? `Sensitivity scenarios: ${sens.join(', ')}.` : 'No sensitivity scenario recorded yet (Practice Note s9.3(a)(x)).'],
        ['Limitations and knowledge gaps', [E.limitations, E.gaps].filter(Boolean).join('\n') || '[Author to state limitations and knowledge gaps]'],
        ['Confidence', E.confidence.level ? `${E.confidence.level}: ${E.confidence.why || '[justify]'}` : '[Author to choose high, moderate or low and justify it (Practice Note s9.3(e))]'],
      ],
      signoffs: st.valid, watermark: st.watermark, prompt: st.prompt,
    };
  }
  /* Assumption diff: compare this project's parameters with another party's (Spare Construct JSON, a settings object or a
     CSV 'key,value' template), run each difference alone, rank by effect, and export the JWS input table. */
  const DIFF_KEYS = [/^growth$/, /^growthMode$/, /^hvPct$/, /^pcuHV$/, /^peakToDaily$/, /^gravityBeta$/, /^satFlow$/, /^cycle$/, /^lostPerPhase$/, /^connectorSpeed$/, /^connectors$/,
    /^rates\.\w+\.(AM|PM)\.(rate|inPct)$/, /^junction\.(losMethod|pff|T|queueSpace|f95)$/, /^turns\.(left|right|rightSignals|uturn)$/, /^bprAlpha$/, /^bprBeta$/, /^baseYear$/];
  /* The other party's file is untrusted: only the parameters Spare Construct compares (DIFF_KEYS) are read, a path segment
     such as __proto__ is refused, and every object is built without a prototype, so nothing can reach
     Object.prototype (which would, for example, mark every checklist item done). */
  const BAD_SEG = /^(__proto__|constructor|prototype)$/;
  const safeKey = k => typeof k === 'string' && k.length < 200 && !k.split('.').some(s => !s || BAD_SEG.test(s)) && DIFF_KEYS.some(re => re.test(k));
  function flatten(o, pre = '', out = Object.create(null)) {
    for (const [k, v] of Object.entries(o || {})) { if (BAD_SEG.test(k)) continue; const p = pre ? pre + '.' + k : k; if (isObj(v)) flatten(v, p, out); else out[p] = v; }
    return out;
  }
  const cell = v => { const s = String(v == null ? '' : v).trim(); return s === '' || s === '(blank)' ? null : s === 'on' ? true : s === 'off' ? false : isFinite(+s) ? +s : s; };
  function parseOther(text) {
    const t = String(text || '').trim(); if (!t) throw new Error('Paste the other party\'s Spare Construct project file, settings JSON or a key,value CSV.');
    const o = Object.create(null), put = (k, v) => { if (!safeKey(k)) return; const p = k.split('.'); let x = o; for (let i = 0; i < p.length - 1; i++) { if (!own(x, p[i]) || !isObj(x[p[i]])) x[p[i]] = Object.create(null); x = x[p[i]]; } x[p[p.length - 1]] = v; };
    if (/^[{[]/.test(t)) {
      let j; try { j = JSON.parse(t); } catch (e) { throw new Error('That is not valid JSON: ' + e.message); }
      const s = j && isObj(j.settings) ? j.settings : j; if (!isObj(s)) throw new Error('That JSON holds no settings. Paste a Spare Construct project file or a settings object.');
      const f = flatten(s); for (const k of Object.keys(f)) put(k, f[k]); return o;
    }
    // a key,value template, or Spare Construct's own 'Assumptions (CSV)' export (setting,label,value,default,unit): read the value column
    const rows = U.parseCSV(t).filter(r => r.length && !/^\s*#/.test(String(r[0] || '')));
    let ki = 0, vi = 1;
    const h = (rows[0] || []).map(x => String(x).trim().toLowerCase());
    if (h.includes('value') && h.some(x => /^(setting|key|item|parameter)$/.test(x))) { ki = h.findIndex(x => /^(setting|key|item|parameter)$/.test(x)); vi = h.indexOf('value'); rows.shift(); }
    for (const r of rows) { const k = String(r[ki] == null ? '' : r[ki]).trim(); if (!k || /^(item|key|setting)$/i.test(k)) continue; put(k, cell(r[vi])); }
    return o;
  }
  function diffParams(P, other) {
    const A = flatten(P.settings), B = flatten(isObj(other) ? other : parseOther(other)), out = [];
    for (const k of Object.keys(B)) {
      if (!safeKey(k) || !(k in A) || String(A[k]) === String(B[k])) continue;
      // a numeric setting only takes a number (a label or a typo is not a difference to run)
      if (typeof A[k] === 'number' && !(typeof B[k] === 'number' && isFinite(B[k]))) continue;
      out.push({ key: k, a: A[k], b: B[k] });
    }
    return out;
  }
  async function runDiff(P, diffs, opts = {}) {
    const sc = opts.sc || activeSc(P), period = opts.period || P.settings.period || 'AM', t0 = Date.now();
    const metric = r => { let dos = 0, dl = 0, q = 0; for (const n of Object.values(r.nodes || {})) { dos = Math.max(dos, n.dos || 0); dl = Math.max(dl, n.delay || 0); for (const a of n.appr || []) q = Math.max(q, a.q95 || 0); } return { dos: U.r3(dos), delay: U.r1(dl), queue: U.r1(q), over: r.kpi.over, vht: U.r1(r.kpi.vht) }; };
    const central = metric(await Engine.assign(sc, { period, settings: P.settings, maxIter: opts.maxIter }));
    const rows = [];
    for (let i = 0; i < diffs.length; i++) {
      const d = diffs[i], S2 = JSON.parse(JSON.stringify(P.settings)); Kit.setPath(S2, d.key, d.b);
      opts.onProgress && opts.onProgress(i + 1, diffs.length, d.key);
      const m = metric(await Engine.assign(sc, { period, settings: S2, maxIter: opts.maxIter }));
      const rel = (x, y) => (y > 0 ? Math.abs(x - y) / y : Math.abs(x - y) > 0 ? 1 : 0);
      const score = rel(m.dos, central.dos) + rel(m.delay, central.delay) + rel(m.queue, central.queue) + rel(m.over, central.over);
      rows.push({ ...d, m, score: U.r3(score), name: `${sc.name} · ${d.key} = ${d.b}`,
        effect: `worst DoS ${central.dos} → ${m.dos}; worst delay ${num(central.delay)} → ${num(m.delay)} s; longest queue ${num(central.queue)} → ${num(m.queue)} m; roads over capacity ${central.over} → ${m.over}` });
    }
    rows.sort((a, b) => b.score - a.score);
    return { period, central, rows, ms: Date.now() - t0, note: 'Each difference is run alone against this project (one-at-a-time), as an unsaved scenario variant.' };
  }
  function jwsCSV(P, result) {
    const rows = [['Item', 'Party A (this project)', 'Party B', 'Effect', 'Agreed?']];
    for (const r of (result && result.rows) || []) rows.push([r.key, r.a, r.b, r.effect, '']);
    return stampCSV(U.csv(rows), P);
  }

  /* ====================================================================================================
     13. Model status badge, disclaimers, CSV stamping, standards register
     ==================================================================================================== */
  function badge(P, period = 'AM') {
    const m = Engine.modelStatus(P, period), tone = m.status === 'validated' ? 'ok' : m.status === 'calibrated' ? '' : 'warn';
    return { ...m, html: `${pill(esc(m.short), tone)} <span class="note">${esc(m.footer)}</span>`, text: m.footer };
  }
  function disclaimer(id, P, opts = {}) {
    const d = DISCLAIMERS[id]; if (!d) return '';
    let t = d.text.replace('[version date]', longDate(VERSION));
    if (id === 'D12' && opts.tool) t = t.replace(/\[[^\]]+\]/, opts.tool);
    const add = P && state(P).notes[id]; return add ? `${t} ${add}` : t;
  }
  const disclaimerHtml = (id, P, opts) => `<p class="note disclaimer" data-disclaimer="${id}"><b>${esc(DISCLAIMERS[id].title)} (${id}).</b> ${esc(disclaimer(id, P, opts))}</p>`;
  // Footer comment rows for a CSV export: D1, the DRAFT watermark when unsigned, the version stamp and model status
  function csvFooter(P, opts = {}) {
    const st = signState(P), rows = [['# ' + disclaimer('D1', P)]];
    if (st.watermark) rows.push(['# ' + st.watermark]);
    rows.push(['# ' + stamp(P).footer]);
    rows.push(['# ' + ['AM', 'PM'].map(p => Engine.modelStatus(P, p).footer).join(' · ')]);
    if (opts.handover) rows.push(['# ' + disclaimer('D12', P, { tool: opts.handover })]);
    return U.csv(rows);
  }
  // Idempotent: a CSV that already carries the version stamp is returned as it is
  function stampCSV(csv, P, opts) { const s = String(csv == null ? '' : csv); if (/^"?# Version stamp: /m.test(s)) return s; return (s ? s.replace(/(\r?\n)*$/, '') + '\r\n' : '') + csvFooter(P, opts); }
  // Disclaimers a block of HTML must carry, by what it shows (used by the export test and the report lane)
  /* A disclaimer counts as present when the HTML marks it (data-disclaimer / data-d) or carries its opening words.
     The HTML is decoded first (D13's quotes arrive as &quot;) and only the text before any [placeholder] is compared
     (D12 with the tool filled in). */
  const decode = h => String(h).replace(/&quot;/g, '"').replace(/&#39;|&#x27;/g, "'").replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&');
  function missingDisclaimers(html, kinds = []) {
    const need = new Set(['D1', ...kinds.map(k => ({ junction: 'D3', economics: 'D4', anop: 'D5', validation: 'D6', model: 'D7', safety: 'D8', cbds: 'D9', regulatory: 'D10', authority: 'D11', handover: 'D12', assumption: 'D13' })[k]).filter(Boolean)]);
    const raw = String(html), text = decode(raw.replace(/<[^>]*>/g, ' ')).replace(/\s+/g, ' ');
    return [...need].filter(id => {
      if (new RegExp(`data-(disclaimer|d)="${id}"`).test(raw)) return false;
      const head = DISCLAIMERS[id].text.split('[')[0].replace(/\s+/g, ' ').trim().slice(0, 60);
      return !text.includes(head);
    });
  }
  function supersession(P, now) {
    const C = state(P), d = today(now), out = [];
    for (const s of STANDARDS) if (C.used[s.id] && s.next && s.next.date && d >= s.next.date && C.used[s.id] < s.next.date)
      out.push({ id: s.id, doc: s.doc, edition: s.edition, next: s.next, used: C.used[s.id], text: `${s.doc} (${s.edition}) has changed since this project's outputs were stamped on ${longDate(C.used[s.id])}: ${s.next.what} (${longDate(s.next.date)}). Re-check the affected outputs before relying on them.` });
    return out;
  }

  /* ====================================================================================================
     14. The 87-item checklist (auto where Spare Construct computes it, manual otherwise)
     ==================================================================================================== */
  function applies(item, X) {
    const p = X.p, k = p ? p.kind : null, id = item.id, w = item.pathway;
    const rc = k === 'rc', pc = k === 'pc', nor = k === 'nor', ftaa = k === 'ftaa', rmaish = rc || pc || nor || ftaa;
    if (/\(Auckland\)/.test(w) && !X.auck) return false;
    switch (item.group) {
      case 'X': return /RMA and FTAA/.test(w) ? rmaish : /modelled evidence/.test(w) ? true : /ITAs in Auckland/.test(w) ? X.auck && (!p || rmaish) && X.itaLikely : true;
      case 'RC':
        if (/lodged from 30 Oct 2026/.test(w)) return (rc || nor || (ftaa && /NoR/.test(w))) && X.trans && (!/District land use/.test(w) || X.pathway === 'rc-landuse');
        if (/Urban subdivision/.test(w)) return X.pathway === 'rc-subdivision';
        if (/^ITAs/.test(w)) return (!p || rmaish) && X.itaLikely && (!/to vest/.test(w) || !!X.C.ctx.vest);
        if (/Resource consent/.test(w)) return rc || (ftaa && X.pathway === 'ftaa-consent');
        return true;
      case 'PS': return X.pathway === 'precinct' || X.precinct;
      case 'PC': return /Private/.test(w) ? X.pathway === 'pc-private' : pc;
      case 'NOR': return X.pathway === 'nor' || X.pathway === 'outline-plan';
      case 'FT': return /NoR/.test(w) ? X.pathway === 'ftaa-nor' : id === 'FT-06' ? ftaa && X.sh : ftaa;
      case 'SH': return X.sh;
      case 'RCA': return X.auck && rmaish && (id !== 'RCA-02' || !!X.C.ctx.vest);
      case 'FUND': return k === 'fund';
      case 'SPD': return k === 'speed';
      case 'DC': return k === 'dc';
      case 'TOU': return k === 'tou';
      case 'EV': return id === 'EV-01' ? rc || pc : rmaish;
    }
    return true;
  }
  // Auto evaluators: id → X → { state, text } (null = no automatic status; the author records it)
  const AUTO = {
    'X-01': X => X.r.code === 'unset' || X.r.code === 'undated' ? { state: 'todo', text: X.r.text } : { state: 'done', text: `${X.r.pathwayLabel}: ${X.r.text}` },
    'X-02': X => ({ state: X.r.date ? 'done' : 'todo', text: X.auth.transitional ? 'Authority table ready; confirm whether a cl 8(2) direction ended the transitional period.' : 'Authority table and affected-operator list ready.' }),
    'X-03': X => X.C.proportion.confirmed ? { state: 'done', text: `Proportionality statement confirmed${X.C.proportion.by ? ' by ' + X.C.proportion.by : ''}.` } : { state: 'todo', text: 'Drafted from the screening metrics; the author must confirm it.' },
    'X-04': X => X.sign.signed ? { state: 'done', text: `Signed off by ${X.sign.valid.find(s => s.role === 'prepared').name}.` } : { state: X.sign.prompt ? 'warn' : 'todo', text: X.sign.prompt || 'Not signed off: exports carry the DRAFT watermark.' },
    'X-05': X => { const L = X.lint.filter(l => /count-|rates$/.test(l.id)); return L.length ? { state: 'warn', text: L.map(l => l.text).join('; ') } : { state: 'done', text: 'Data sources are dated (data vintage table in Appendix G).' }; },
    'X-06': X => X.lint.some(l => l.id === 'sens') ? { state: 'todo', text: 'No sensitivity scenario yet.' } : { state: 'done', text: 'A sensitivity scenario exists.' },
    'X-07': X => ({ state: X.status.status === 'validated' ? 'done' : 'warn', text: `${X.status.label}. TMDG category ${X.status.category}${X.status.justification ? '' : ' (justification not given)'}.` }),
    'X-08': X => (X.P.data && X.P.data.msm && X.P.data.msm.links ? { state: 'done', text: 'MSM growth loaded for comparison.' } : { state: 'warn', text: Engine.STANDARDS.msm.notChecked }),
    'X-09': X => { const m = X.memo, n = m.filter(i => i.locked).length; return { state: n === m.length ? 'done' : 'todo', text: `${n} of ${m.length} scoping items agreed and locked.` }; },
    'RC-01': X => X.site.looked || X.site.zone ? { state: X.e27.pc120 && X.e27.pc120.exemptDiffers ? 'warn' : 'done', text: `${X.site.zone ? X.site.zone.name : 'zone not known'}${X.site.precinct ? ' · ' + X.site.precinct : ''}${X.e27.pc120 ? ' · ' + X.e27.pc120.text : ' · PC120 layer not read'}` } : { state: 'todo', text: 'Look up the site (AUP zone, precinct and PC120).' },
    'RC-02': X => X.e27.redirect ? { state: 'na', text: X.e27.redirect.text + '.' } : X.e27.exempt ? { state: 'done', text: X.e27.exempt.text + '.' } : { state: 'done', text: X.e27.tier ? `E27 thresholds exceeded: tier ${X.e27.tier}.` : 'No E27.6.1 threshold exceeded.' },
    'RC-03': X => X.C.ctx.activityStatus ? { state: 'done', text: `${X.e27.catchAll.why} Busiest hour ${num(X.e27.quantities.peak)} veh/h.` } : { state: 'todo', text: 'Set the activity status to test E27.6.1(1)(b) and E27.9(5).' },
    'RC-04': X => ({ state: X.e27.tier ? 'done' : 'na', text: X.e27.tier ? (X.e27.tier === 1 ? E27.tier1Scope : E27.tier2Scope) + '.' : 'No E27 trigger: no matters of discretion from E27.6.1.' }),
    'RC-05': X => ({ state: 'todo', text: `${X.e27.matrix.filter(m => m.applies).length} objectives and policies apply; the planner writes the evaluation.` }),
    'RC-07': X => (X.P.register || []).length ? { state: 'done', text: `${X.P.register.length} register rows, each with an inclusion reason.` } : { state: 'todo', text: 'The committed-development register is empty.' },
    'RC-08': X => { const b = X.P.scenarios.flatMap(s => Scenarios.check(s, X.P)).filter(x => x.level === 'block'); return X.P.assessment ? { state: b.length ? 'warn' : 'done', text: b.length ? `${b.length} blocking scenario check${b.length === 1 ? '' : 's'}.` : 'Scenario matrix created; funding gating passes.' } : { state: 'todo', text: 'Create the assessment scenario set.' }; },
    'RC-09': X => X.sc.extent ? { state: 'done', text: `Extent stored: ${(X.sc.extent.sites || []).length} sites, ${(X.sc.extent.links || []).length} links.` } : { state: 'todo', text: 'Run the extent test (5% / 10%) and store the extent.' },
    'RC-12': X => { const c = ((X.P.data || {}).loaded || {}).cas; return c && c.at ? { state: 'done', text: `CAS loaded: ${c.label || `${c.from}–${c.to}`}.` } : { state: 'todo', text: 'Load CAS crashes for the study area.' }; },
    'RC-17': X => { if (!X.C.access.length) return { state: 'todo', text: 'Add the proposed access points.' };
      const nc = X.acc.filter(a => a.unchecked.length).length;
      return { state: X.acc.some(a => a.worst !== 'ok') ? 'warn' : 'done', text: `${X.acc.length} access point${X.acc.length === 1 ? '' : 's'}: ${X.acc.filter(a => a.worst === 'NC').length} non-complying, ${X.acc.filter(a => a.worst === 'RD').length} restricted discretionary${nc ? `; ${nc} not fully checked (AUP controls not loaded or not checked by hand)` : ''}.` }; },
    'RC-18': X => !X.C.access.length ? { state: 'todo', text: 'Add the proposed access points with frontage, crossings, widths and queues.' } : { state: X.acc.some(a => a.flags.some(f => /Table|E27\.6\.4\.3/.test(f.rule))) ? 'warn' : 'done', text: 'Crossing number, width, passing bays and queues checked from the entered values.' },
    'RC-22': X => { const f = X.fds; return { state: f.status === 'unknown' ? 'todo' : f.outOfSequence || f.status === 'removal' ? 'warn' : 'done', text: f.text }; },
    'RC-24': X => { const o = X.ops; return o.compared ? { state: 'done', text: o.rows.filter(r => r.affected).map(r => r.name).join('; ') || 'No operator over the threshold.' } : { state: 'todo', text: o.note }; },
    'SH-03': X => { const m = (X.shc.motorway || []).filter(x => !x.confirmed); return m.length ? { state: 'warn', text: `${m.length} zone${m.length === 1 ? '' : 's'} connected straight to a motorway: GRPA s84 special authorisation, or move the connector (confirm in Compliance › State highways).` } : null; },
    'SH-01': X => ({ state: X.shc.triggers.length ? 'warn' : 'done', text: X.shc.triggers.length ? X.shc.triggers.join('; ') + ': seek NZTA written approval (s95E(3)(a)).' : 'No NZTA trigger case found.' }),
    'SH-04': X => X.shc.tech.length ? { state: 'done', text: X.shc.tech.map(t => `${t.name}: SISD ${t.sisd.m} m, ${t.type.text}`).join('; ') } : { state: 'na', text: 'No state highway access.' },
    'SH-05': X => ({ state: 'done', text: X.ita.nzta ? X.ita.nzta.text + (X.ita.depth ? ' ' + X.ita.depth.text : '') : NZTA_T1.order }),
    'SH-06': X => { const f = X.shc.rows.filter(r => r.pavement && r.pavement.flagged); return { state: f.length ? 'warn' : 'done', text: (f.length ? `${f.length} state highway link${f.length === 1 ? '' : 's'} with heavy vehicles up more than 20%: pavement assessment.` : 'No state highway link with heavy vehicles up more than 20%.') + (X.shc.devHvPct == null && X.shc.rows.length ? ' Total-traffic proxy (one HV share for all traffic): enter the HV share of the development traffic.' : '') }; },
    'SH-07': X => ({ state: X.shc.noise ? 'warn' : 'done', text: X.shc.noise ? 'Noise-sensitive activity within 100 m of a state highway: acoustic assessment.' : 'No noise-sensitive activity within 100 m of a state highway found.' }),
    'SPD-04': X => ({ state: 'done', text: 'Local and collector roads route to the local board in the authority resolver.' }),
    'FUND-03': X => ({ state: X.P.settings.econ && X.P.settings.econ.method === 'mbcm' ? 'done' : 'warn', text: X.P.settings.econ && X.P.settings.econ.method === 'mbcm' ? 'MBCM v1.7.5 settings block printed with the economics.' : 'This project uses the older uniform discounting; adopt the MBCM defaults.' }),
    'EV-01': X => { const c = X.clocks.filter(c => /s92|cl2[13]|lodged/.test(c.kind)); return c.length ? { state: 'done', text: `${c.length} further-information clock${c.length === 1 ? '' : 's'} tracked.` } : { state: 'todo', text: 'Record s92 or cl 23 requests to track their deadlines.' }; },
    'EV-02': X => X.clocks.some(c => c.kind === 'hearing') ? { state: 'done', text: 'Hearing evidence dates calculated.' } : { state: 'todo', text: 'Enter the hearing date for the s103B evidence dates.' },
  };
  function checklist(P, opts = {}) {
    const C = state(P), sc = activeSc(P), r = regime(P), p = PATHWAYS[C.ctx.pathway] || null, site = siteInfo(P);
    // X carries the project facts; the expensive checks are computed once, on first use (lazy getters)
    const X = { P, C, sc, r, p, site, pathway: C.ctx.pathway, auck: inAuckland(P), trans: r.side === 'transition', precinct: !!site.transportPrecinct };
    const lazy = {}, def = (k, fn) => Object.defineProperty(X, k, { enumerable: true, get: () => (k in lazy ? lazy[k] : (lazy[k] = fn())) });
    def('e27', () => e27(P, sc)); def('ita', () => itaScope(P, sc)); def('shc', () => shCheck(P, sc)); def('auth', () => authorities(P)); def('sign', () => signState(P)); def('lint', () => lint(P));
    def('status', () => Engine.modelStatus(P, 'AM')); def('memo', () => scopingMemo(P, sc)); def('acc', () => C.access.map(a => accessCheck(P, a, sc))); def('fds', () => fdsCheck(P, sc));
    def('ops', () => operators(P)); def('clocks', () => clocks(P)); def('sh', () => shInvolved(P, sc).involved);
    // an ITA is likely when the recommended level is an ITA, an AT 'ITA likely' case applies, or the pathway is a plan change or NoR
    def('itaLikely', () => { const s = X.ita; return s.levelIndex >= 2 || s.at.length > 0 || !!(p && (p.kind === 'pc' || p.kind === 'nor')); });
    const rows = CHECKLIST.map(it => {
      let ok;
      try { ok = applies(it, X); } catch (e) { console.error(`Compliance: could not test whether ${it.id} applies:`, e); ok = true; }
      // the author's record: own properties only, and only the four states (a project file is untrusted)
      const man = own(C.checks, it.id) && isObj(C.checks[it.id]) ? C.checks[it.id] : {};
      let st = null, err = '';
      if (ok && AUTO[it.id]) { try { st = AUTO[it.id](X); } catch (e) { err = e.message; } }
      const manual = own(man, 'state') && CK_STATES.includes(man.state);
      const state = !ok ? 'na' : manual ? man.state : st ? st.state : 'todo';
      // text is plain: renderers escape it once
      return { ...it, groupLabel: CK_GROUPS[it.group] || it.group, applies: ok, state, auto: !!st && !manual, text: st ? String(st.text || '') : err ? `Could not be checked: ${err}` : '',
        note: own(man, 'note') ? String(man.note || '') : '', by: own(man, 'by') ? String(man.by || '') : '', date: own(man, 'date') ? String(man.date || '') : '' };
    });
    Object.defineProperty(rows, 'X', { value: X, enumerable: false });   // the computed facts, so a caller can reuse them
    return rows;
  }
  const CK_STATES = ['done', 'todo', 'warn', 'na'];
  function checklistCSV(P) {
    const rows = [['ID', 'Pathway', 'Item', 'Source', 'Spare Construct fills', 'Applies', 'Status', 'Spare Construct finding', 'Author note', 'By', 'Date', 'Evidence']];
    for (const r of checklist(P)) rows.push([r.id, r.pathway, r.item, r.source, r.fills, r.applies ? 'yes' : 'no', r.state, r.text, r.note, r.by, r.date, r.evidence]);
    return stampCSV(U.csv(rows), P);
  }

  /* ====================================================================================================
     15. The Compliance tab (HTML strings; events by data-cdo, data-cf, data-csec and data-ck-*)
     ==================================================================================================== */
  const ui = { sec: 'overview', ck: 'applicable', busy: '' };
  const SECS = [['overview', 'Overview'], ['context', 'Pathway and dates'], ['site', 'Site and E27'], ['scope', 'ITA scope'], ['access', 'Access'], ['sh', 'State highways'],
    ['authority', 'Authorities'], ['register', 'Receiving environment'], ['fds', 'FDS'], ['rfi', 'RFI and clocks'], ['expert', 'Expert sign-off'], ['standards', 'Standards and disclaimers'], ['checklist', 'Checklist']];
  const F = {
    sel: (path, pairs, val) => `<select data-cf="${esc(path)}">${Kit.opt(pairs, val ?? '')}</select>`,
    txt: (path, val, ph = '', w = '100%') => `<input data-cf="${esc(path)}" value="${esc(val ?? '')}" placeholder="${esc(ph)}" style="width:${w}">`,
    num: (path, val, ph = '', w = '90px') => `<input type="number" step="any" data-cf="${esc(path)}" data-t="num" value="${val == null || val === '' ? '' : esc(val)}" placeholder="${esc(ph)}" style="width:${w}">`,
    date: (path, val) => `<input type="date" data-cf="${esc(path)}" value="${esc(val || '')}">`,
    chk: (path, val, label) => `<label class="check"><input type="checkbox" data-cf="${esc(path)}" data-t="bool" ${val ? 'checked' : ''}> ${label}</label>`,
    area: (path, val, rows = 3, ph = '') => `<textarea data-cf="${esc(path)}" rows="${rows}" style="width:100%" placeholder="${esc(ph)}">${esc(val || '')}</textarea>`,
    btn: (act, label, cls = '') => `<button class="btn small ${cls}" data-cdo="${esc(act)}">${label}</button>`,
  };
  const row = (label, inner) => `<label class="fld" style="grid-template-columns:170px minmax(0,1fr);margin:4px 0"><span>${label}</span><span class="fld-in">${inner}</span></label>`;
  const h4 = t => `<div class="lg-title" style="margin-top:12px">${t}</div>`;
  const stIcon = s => ({ done: '<span class="los A" aria-label="done">✓</span>', warn: '<span class="los D" aria-label="check">!</span>', todo: '<span class="los none" aria-label="to do">·</span>', na: '<span class="los none" aria-label="not applicable">–</span>' })[s] || '';
  const fl = s => s === 'NC' ? pill('Non-complying', 'bad') : s === 'RD' ? pill('Restricted discretionary', 'warn') : s === 'ok' ? pill('Complies', 'ok') : s === 'unchecked' ? pill('Not checked: AUP controls not loaded', 'warn') : '';
  const accStatus = a => a.worst === 'ok' ? 'Complies' : a.worst === 'NC' ? 'Non-complying' : a.worst === 'RD' ? 'Restricted discretionary' : 'Not checked: AUP controls not loaded';

  function secOverview(P) {
    // one checklist pass; its computed facts (E27, ITA scope, authorities, sign-off) are reused below
    const all = checklist(P), X = all.X, r = X.r, st = stamp(P), rc = recheck(P), A = X.auth, sup = supersession(P), sign = X.sign, ck = all.filter(i => i.applies);
    const done = ck.filter(i => i.state === 'done').length, warn = ck.filter(i => i.state === 'warn').length;
    const E = X.e27, I = X.ita;
    return `${rc.show ? Kit.note(`<b>${esc(rc.text)}.</b> Outputs were generated on ${longDate(rc.since)} for a lodgement on the other side of 30 Oct 2026.`, true) : ''}
      ${sup.map(s => Kit.note(esc(s.text), true)).join('')}
      ${sign.prompt ? Kit.note(esc(sign.prompt), true) : ''}
      <div class="kpis">
        ${Kit.kpi('Pathway', esc(r.pathwayLabel), esc(r.date ? longDate(r.date) : 'no lodgement date'))}
        ${Kit.kpi('Regime', esc(r.short), esc(r.clause || ''))}
        ${Kit.kpi('E27 screening', E.redirect ? 'Precinct' : E.exempt ? 'Exempt' : E.tier ? `Tier ${E.tier}` : 'Not exceeded', esc(E.redirect ? E.redirect.precinct.id : E.exempt ? E.exempt.rule : ''))}
        ${Kit.kpi('Suggested level', esc(I.level), esc(`${num(I.e27.quantities.peak)} veh/h busiest hour`))}
        ${Kit.kpi('Checklist', `${done}<small> / ${ck.length}</small>`, `${warn} to check`, warn ? 'up' : '')}
        ${Kit.kpi('Sign-off', sign.signed ? 'Signed' : 'Draft', sign.signed ? esc(sign.valid[0].name) : 'exports carry DRAFT', sign.signed ? 'down' : 'up')}
      </div>
      <div class="row2" style="margin-top:10px"><div>${h4('Version stamp')}${Kit.table({ rows: st.lines.map(([a, b]) => [esc(a), esc(b)]) })}</div>
      <div>${h4('Model status')}${['AM', 'PM'].map(p => `<p>${badge(P, p).html}</p>`).join('')}<p class="note">${esc(Engine.STANDARDS.tmdg.standing)}</p>
      ${h4('Reviewer and approver')}${Kit.table({ rows: A.rows.map(x => [esc(x.role), esc(x.name)]) })}${A.transitionalText ? Kit.note(esc(A.transitionalText), true) : ''}</div></div>`;
  }
  function secContext(P) {
    const C = state(P), c = C.ctx, r = regime(P), pwOpts = { '': 'Choose…', ...Object.fromEntries(Object.entries(PATHWAYS).map(([k, v]) => [k, v.label])) };
    return `<div class="row2"><div>
      ${row('Pathway', F.sel('ctx.pathway', pwOpts, c.pathway))}
      ${row('Lodgement date', `${F.date('ctx.lodgement', c.lodgement)} ${F.chk('ctx.lodgementActual', c.lodgementActual, 'lodged (actual)')}`)}
      ${row('Activity status', F.sel('ctx.activityStatus', ACTIVITY, c.activityStatus))}
      ${row('Region', F.sel('ctx.region', { auckland: 'Auckland', other: 'Outside Auckland' }, c.region))}
      ${row('Context', F.sel('ctx.context', { urban: 'Urban', periurban: 'Peri-urban', rural: 'Rural' }, c.context))}
      ${row('Zoning consistency', F.chk('ctx.zoningConsistent', c.zoningConsistent !== false, 'the proposal is consistent with the zoning'))}
      ${row('Proposed occupation year', F.num('ctx.occupation', c.occupation, 'year'))}
      ${PATHWAYS[c.pathway] && PATHWAYS[c.pathway].proc === 'nor' ? row('Land interest (NoR)', F.sel('ctx.landInterest', { '': 'Not recorded', yes: 'Sufficient interest in the land', no: 'No sufficient interest' }, c.landInterest)) + row('Requiring authority', F.chk('ctx.raCouncil', c.raCouncil, 'the council is the requiring authority (s168A)')) + row('Designation transfer date', F.date('ctx.transferDate', c.transferDate)) : ''}
      ${PATHWAYS[c.pathway] && PATHWAYS[c.pathway].kind === 'pc' ? row('Notification date', F.date('ctx.notificationDate', c.notificationDate)) : ''}
      ${row('cl 8(2) direction date', `${F.date('ctx.cl82Date', c.cl82Date)} <span class="note">if Council ended AT's transitional role early</span>`)}
      ${row('Assets to vest', F.chk('ctx.vest', c.vest, 'roads or infrastructure will vest in the council'))}
      </div><div>${h4('Regime')}<p><b>${esc(r.text)}</b></p><p class="note">${esc(r.transitionText)}</p>
      ${PATHWAYS[c.pathway] && PATHWAYS[c.pathway].kind === 'pc' && validDay(c.notificationDate) && c.notificationDate >= '2028-01-01' && c.pathway === 'pc-private' ? Kit.note('Private plan changes notified from 1 Jan 2028 need a Ministerial exemption until the transition date (s80YA).', true) : ''}
      ${disclaimerHtml('D10', P)}<p class="note">Regulatory data ${esc(VERSION)} · ${srcLink(SRC.planningAct)}</p></div></div>`;
  }
  function secSite(P) {
    const C = state(P), s = C.site, si = siteInfo(P), E = e27(P), Q = E.quantities, pt = sitePoint(P), L = s.lookup;
    const qrow = (k, label, unit) => [esc(label), num(Q.q[k]) + ' ' + unit, Q.src[k] === 'entered' ? '<span class="prov user">entered</span>' : '<span class="prov derived">zones</span>', F.num('q.' + k, C.q[k], 'override')];
    return `<div class="row2"><div>
      ${h4('Site')}<p class="note">${pt ? `${num(pt.lat, 5)}, ${num(pt.lng, 5)} · ${esc(pt.src)}` : 'No site yet.'}</p>
      <div class="actions" style="display:flex;gap:6px;flex-wrap:wrap">${F.btn('site-lookup', 'Look up AUP zone, precinct, PC120 and FDS', 'primary')}${F.btn('site-here', 'Use map centre')}${F.btn('site-sel', 'Use selected zone or junction')}</div>
      ${L ? `<p class="note">Looked up ${esc(longDate(L.at))}.${L.errors.length ? ' ' + esc(L.errors.join(' ')) : ''}${(L.kept || []).length ? ' ' + esc(L.kept.join('; ')) + '.' : ''}</p>` : ''}
      ${Kit.table({ rows: [
        ['Zone', si.zone ? esc(si.zone.name) + ` <span class="prov ${si.zone.src === 'entered' ? 'user' : 'measured'}">${si.zone.src === 'entered' ? 'entered' : 'AUP map'}</span>` : '–'],
        ['Precinct', esc(si.precinct || 'none') + (si.transportPrecinct ? ` · ${pill('transport provisions', 'warn')}` : '')],
        ['Centre Fringe Office Control', si.cfoc ? 'yes' : L ? 'no' : '–'],
        ['PC120 proposed zone', si.pc120 ? esc(si.pc120.zone || '–') : L ? 'no PC120 record at this point' : '–'],
        ['FDS area', si.fds ? `${esc(si.fds.name)} · ${esc(si.fds.timing)}` : L ? 'not in an FDS future urban area' : '–'],
        ['Local board', esc(si.board || '–')],
      ] })}
      ${row('Zone (override)', F.txt('site.zoneOverride', s.zoneOverride, 'e.g. Residential - Mixed Housing Urban Zone'))}
      ${row('Precinct (override)', F.txt('site.precinctOverride', s.precinctOverride, 'e.g. Drury Centre'))}
      ${row('PC120 zone (override)', F.txt('site.pc120Override', s.pc120Override, 'proposed zone'))}
      ${F.chk('site.zoneTA', s.zoneTA, 'Zone or precinct rules already require a trip generation assessment (E27.6.1(2)(d))')}
      ${F.chk('site.unlisted', s.unlisted, 'The activity is not listed in Table E27.6.1.1')}
      ${F.chk('site.driveThrough', s.driveThrough, 'Fast food GFA is drive-through (T8)')}
      ${row('School type', F.sel('site.school', { primary: 'Primary (T4)', secondary: 'Secondary (T5)' }, s.school))}
      ${h4('Proposal quantities')}${Kit.table({ head: ['Quantity', { label: 'Value', n: true }, 'From', 'Override'], rows: [
        qrow('dwellings', 'Dwellings', 'dwellings'), qrow('ird', 'Integrated residential (retirement)', 'units'), qrow('visitor', 'Visitor accommodation', 'units'), qrow('subdivision', 'Subdivision capacity', 'dwellings'),
        qrow('primary', 'Primary students', ''), qrow('secondary', 'Secondary students', ''), qrow('tertiary', 'Tertiary students', ''), qrow('office', 'Office', 'm²'), qrow('retail', 'Retail (not drive-through)', 'm²'),
        qrow('retailDT', 'Drive-through retail', 'm²'), qrow('warehouse', 'Warehousing', 'm²'), qrow('industrial', 'Other industrial', 'm²')] })}
      <p class="note">Busiest hour: ${num(Q.peak)} veh/h (${esc(Q.peakPeriod || '–')}), ${esc(Q.basis)}.</p>
      </div><div>
      ${h4('AUP E27 transport triggers')}<p class="note">${esc(E.version)}</p>
      ${E.redirect ? Kit.note(`<b>${esc(E.redirect.text)}.</b> (${esc(E.redirect.cite)})`, true) : ''}${E.exempt ? Kit.note(`<b>${esc(E.exempt.text)}</b>`) : ''}
      ${E.pc120 ? Kit.note(`${esc(E.pc120.text)}<br><span class="note">${esc(E.pc120.status)}</span>`, E.pc120.exemptDiffers) : ''}
      ${Kit.table({ head: ['Rule', { label: 'Threshold', n: true }, { label: 'Proposal', n: true }, 'Exceeded', 'Tier', 'Consequence'], rows: E.rows.filter(r => r.q > 0 || r.exceeded).map(r => [r.id, `${num(r.thr)} ${esc(r.unit)}`, num(r.q), r.exceeded ? pill('exceeded', 'warn') : 'no', r.tier, esc(r.consequence)]), empty: 'No listed activity in the development zones.' })}
      <p><b>${esc(E.catchAll.id)}</b>: ${esc(E.catchAll.why)}</p>
      ${E.tier ? `<p><b>${esc(E.consequence)}</b></p>` : ''}
      ${E.notes.map(n => `<p class="note">${esc(n)}</p>`).join('')}<p class="note"><i>${esc(E.wording)}</i></p>
      ${E.supplementary ? Kit.note(`Network level-of-service results are marked <b>${esc(E27.supplementary)}</b>.`) : ''}
      ${h4('Objectives and policies matrix (E27.2, E27.3)')}${Kit.table({ head: ['Provision', 'Applies', 'Evidence'], rows: E.matrix.map(m => [`<b>${m.ref}</b> ${esc(m.text)}`, m.applies ? 'yes' : `no <span class="note">${esc(m.why)}</span>`, esc(m.evidence.join('; '))]) })}
      <p class="note">${srcLink(E27.source)}</p></div></div>`;
  }
  function secScope(P) {
    const C = state(P), I = itaScope(P), memo = scopingMemo(P), pa = I.paragraph;
    return `<div class="row2"><div>
      ${h4('(a) AUP')}${I.aup.map(t => `<p class="note">${esc(t)}</p>`).join('')}
      ${h4('(b) AT ITA Guidelines 2025 v2: ITA likely')}${I.at.length ? I.at.map(t => `<p>• ${esc(t)}</p>`).join('') : '<p class="note">No AT "ITA likely" case found.</p>'}
      ${h4('(c) NZTA G00907 Table 1')}${I.nzta ? `<p><b>${esc(I.nzta.text)}</b></p>${Kit.table({ head: ['Activity', { label: 'Lite', n: true }, { label: 'Full', n: true }, { label: 'Proposal', n: true }], rows: I.nzta.rows.filter(r => r.q > 0).map(r => [esc(r.label), num(r.lite), num(r.full), num(r.q) + ' ' + esc(r.unit)]) })}<p class="note">${esc(I.nzta.order)} ${srcLink(SRC.nztaIta)}</p>`
        : `<p class="note">Applies where a state highway is affected or the site is outside Auckland (${I.sh.inArea ? 'a state highway is in the study area but the development does not use it in the results' : 'no state highway found'}).</p>`}
      ${h4('(d) Table 2 depth')}<p class="note">${esc(I.depth ? I.depth.text : 'Not applicable.')}</p>
      ${h4('(e) Recommended level')}<p><b>${esc(I.level)}</b> ${tag(ASSUMPTION)}</p><p class="note">${esc(I.reasons.join('; '))}.</p>
      ${Kit.table({ head: ['Driver', 'Value'], rows: I.drivers.map(([a, b]) => [esc(a), esc(b)]) })}
      </div><div>
      ${h4('Proportionality statement')}${F.area('proportion.text', pa.text, 7)}
      <div style="display:flex;gap:6px;align-items:center;flex-wrap:wrap;margin-top:6px">${F.chk('proportion.confirmed', C.proportion.confirmed, 'Confirmed by the author')} ${F.txt('proportion.by', C.proportion.by, 'name', '140px')} ${F.date('proportion.date', C.proportion.date)} ${pa.edited ? F.btn('prop-reset', 'Redraft from the metrics') : ''}</div>
      <p class="note">Cites RMA s88(2AA) and Sch 4 cl 2(3)(c)${regime(P).side === 'transition' ? ' and s18B' : ''}. ${esc(I.never)}</p>
      ${h4('(f) Scoping memo')}${Kit.table({ head: ['Item', 'Proposed', 'Agreed by', 'Date', ''], rows: memo.map(m => [esc(m.label), m.locked ? esc(m.value) : F.txt(`scoping.${m.key}.value`, m.edited ? m.value : '', m.proposed), m.locked ? esc(m.by) : F.txt(`scoping.${m.key}.by`, m.by, 'name', '110px'), m.locked ? esc(m.date) : F.date(`scoping.${m.key}.date`, m.date), F.btn(`scope-lock:${m.key}`, m.locked ? 'Unlock' : 'Lock')]) })}
      <div style="margin-top:6px">${F.btn('export:compliance-scoping', 'Export scoping memo (CSV)')}</div></div></div>`;
  }
  function secAccess(P) {
    const C = state(P), sc = activeSc(P), rows = C.access.map((a, i) => ({ a, i, r: accessCheck(P, a, sc) })), o = C.overlays;
    return `<div class="bar-h"><h3>Access and frontage (E27.6.4)</h3>${F.btn('acc-add', 'Add access at the selected junction or map centre', 'primary')}${F.btn('load-aup', o ? 'Reload AUP controls' : 'Load AUP arterial, VAR and frontage controls')}
      <span class="note">${o ? `AUP controls loaded ${esc(longDate(o.at))}: ${o.arterials.length} arterial lines, ${o.var.length} VAR, ${o.frontage.length} frontage, ${o.lx.length} level-crossing areas.${o.errors.length ? ' ' + esc(o.errors.join(' ')) : ''}` : 'Load the AUP controls to test VAR, frontage, arterial and level-crossing rules from the maps.'}</span></div>
      ${rows.length ? rows.map(({ a, i, r }) => `<div class="card" style="padding:8px 10px;margin:8px 0">
        <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap"><b>${esc(a.name || 'Access ' + (i + 1))}</b> ${fl(r.worst)} ${r.sh ? pill('state highway', 'warn') : ''}
          <span class="note">${r.geo.ll ? `nearest intersection ${num(r.geo.intersectionDist, 1)} m · frontage road ${esc(r.frontageLink ? r.frontageLink.name || CLASSES[r.frontageLink.cls].label : '–')}${r.geo.controls && r.geo.controls.length ? ' · ' + esc(r.geo.controls.join(', ')) : ''}${r.geo.lxDist != null ? ` · level crossing ${num(r.geo.lxDist)} m` : ''}` : 'no location'}</span>
          <span class="spacer" style="flex:1"></span>${F.btn('acc-del:' + i, 'Remove', 'danger')}</div>
        <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:2px 14px;margin-top:6px">
          ${row('Name', F.txt(`access.${i}.name`, a.name))}${row('Frontage (m)', F.num(`access.${i}.frontage`, a.frontage))}${row('Crossings on frontage', F.num(`access.${i}.crossings`, a.crossings))}
          ${row('Crossing width (m)', F.num(`access.${i}.width`, a.width))}${row('Parking spaces served', F.num(`access.${i}.spaces`, a.spaces))}${row('Formed access width (m)', F.num(`access.${i}.formed`, a.formed))}
          ${row('Access length (m)', F.num(`access.${i}.length`, a.length))}${row('Q95 at gate or drive-through (m)', F.num(`access.${i}.q95`, a.q95))}${row('Stacking length (m)', F.num(`access.${i}.stacking`, a.stacking))}
          ${row('From adjacent crossing (m)', F.num(`access.${i}.sepAdj`, a.sepAdj))}${row('Combined width (m)', F.num(`access.${i}.combinedWidth`, a.combinedWidth))}${row('From own other crossing (m)', F.num(`access.${i}.sepSame`, a.sepSame))}
          ${row('Intersection distance (override, m)', F.num(`access.${i}.intersectionDist`, a.intersectionDist))}${row('Level crossing (override, m)', F.num(`access.${i}.lxDist`, a.lxDist))}
          ${row('Zone group', F.sel(`access.${i}.zoneGroup`, { '': 'From the site zone', residential: 'Residential', centres: 'Centres, Mixed Use, other', business: 'General Business, Business Park, Industrial', rural: 'Rural' }, a.zoneGroup))}
          ${row('Exception (E27.6.4.1(1))', F.sel(`access.${i}.exception`, { '': 'None', relocate: 'Relocate or amalgamate crossings', only: 'No other means of access' }, a.exception))}
          ${row('Pedestrians (people/h)', F.num(`access.${i}.peds`, a.peds))}
          ${row('Frontage road', F.sel(`access.${i}.frontageLink`, { '': 'Nearest existing road', ...frontageChoices(sc, a, r) }, a.frontageLink))}
          ${row('Mapped VAR (by hand)', F.sel(`access.${i}.var`, { '': o ? 'From the AUP layer' : 'None entered', General: 'General', 'Motorway Interchange Control': 'Motorway Interchange Control', 'Adjacent to Level Crossings': 'Adjacent to Level Crossings' }, a.var))}
          ${row('Arterial frontage (by hand)', F.sel(`access.${i}.arterial`, { '': o ? 'From the AUP layer' : 'Not entered', yes: 'Yes: AUP arterial', no: 'No' }, a.arterial === true ? 'yes' : a.arterial === false ? 'no' : a.arterial))}
          <div>${F.chk(`access.${i}.existing`, a.existing, 'Existing crossing')} ${F.chk(`access.${i}.changed`, a.changed, 'Access changed')} ${F.chk(`access.${i}.passingBays`, a.passingBays, 'Passing bays provided')} ${F.chk(`access.${i}.hv`, a.hv, 'Large heavy vehicles')} ${F.chk(`access.${i}.krf`, a.krf, 'Key Retail Frontage')} ${F.chk(`access.${i}.gcf`, a.gcf, 'General Commercial Frontage')} ${F.chk(`access.${i}.controlsChecked`, a.controlsChecked, 'AUP controls checked by hand (tick or enter those that apply)')}</div>
        </div>
        ${r.flags.length ? Kit.table({ head: ['Rule', 'Status', 'Finding'], rows: r.flags.map(f => [esc(f.rule), fl(f.status), esc(f.text)]) }) : '<p class="note">No breach found from the values entered and the maps loaded.</p>'}
        ${r.uncheckedText ? Kit.note(esc(r.uncheckedText), true) : ''}
        ${r.max != null ? `<p class="note">Maximum crossings: ${r.max} (${esc(r.basis)}).</p>` : ''}
        ${r.evidence ? `<details><summary class="note">E27.8.2(11)(a) evidence</summary>${Kit.table({ head: ['Item', 'Value', 'Source'], rows: r.evidence.map(x => x.map(esc)) })}</details>` : ''}
      </div>`).join('') : '<p class="note">No access points yet. Select a junction (or move the map to the access) and add one.</p>'}
      <p class="note">${esc(ACCESS.nearBasis)}. Vehicle crossing approval comes from the road controlling authority (E27.6.4.2 Note 1 names Auckland Transport: ${esc(AUTH.footnote)}) ${srcLink(SRC.e27)}${r0(rows)}</p>`;
  }
  // Roads an access can front: those at its node, else those within 30 m of it
  function frontageChoices(sc, a, r) {
    const ll = r.geo && r.geo.ll; if (!ll) return {};
    const f = flatAt(ll[0]), ls = a.node && sc.nodes[a.node] ? roadLinks(sc, a.node) : Object.values(sc.links).filter(L => L.status !== 'closed' && sc.nodes[L.a] && sc.nodes[L.b] && toLine(ll, Net.geom(sc, L), f).d <= 30);
    return Object.fromEntries(ls.slice(0, 12).map(L => [L.id, `${L.name || CLASSES[L.cls].label}${L.status === 'proposed' ? ' (proposed)' : ''}`]));
  }
  const r0 = rows => rows.some(x => x.r.sh) ? ' State highway accesses also go to the State highways section.' : '';
  function secSH(P) {
    const X = shCheck(P), C = state(P);
    return `<div class="bar-h"><h3>State highways and NZTA approvals</h3>${F.btn('load-sh', X.loaded ? 'Reload NZTA state highway layer' : 'Load NZTA state highway layer', 'primary')}
      <span class="note">${X.loaded ? `${X.loaded.n} links matched to ${esc(X.loaded.source)} on ${esc(longDate(X.loaded.date))}.` : 'Without the layer, only motorways, ramps and your ticks count as state highways.'} ${esc(X.labels.join(' · '))}</span></div>
      ${X.motorway.map(m => m.confirmed ? Kit.note(`${esc(m.text)} <b>Confirmed</b>${m.by ? ' by ' + esc(m.by) : ''}${m.date ? ' on ' + esc(longDate(m.date)) : ''}: ${esc(m.reason || 'no reason recorded')}. ${F.btn('mw-unconfirm:' + m.key, 'Withdraw')}`)
        : Kit.note(`${esc(m.text)}<br>${F.txt(`mwConfirm.${m.key}.reason`, m.reason, 'why no other road access is reasonably available', '60%')} ${F.btn('mw-confirm:' + m.key, 'Confirm the motorway access')}`, true)).join('')}
      <div class="row2"><div>${h4('NZTA trigger (PPM Access guide s7.2)')}${X.triggers.length ? X.triggers.map(t => `<p>• ${esc(t)}</p>`).join('') : `<p class="note">${X.involved ? esc(X.why.join('; ')) : 'No trigger case found.'}</p>`}
      ${h4('Approvals checklist')}${Kit.table({ head: ['Approval', 'Needed'], rows: X.approvals.map(a => [esc(a.label), a.needed ? pill('yes', 'warn') : a.check ? 'check' : 'no']) })}
      ${X.noise ? Kit.note('Noise-sensitive activity within 100 m of a state highway: reverse sensitivity and acoustic design (PPM Managing effects s5.2.2).', true) : ''}
      ${h4('Access technical check')}${X.tech.length ? Kit.table({ head: ['Access', 'Speed', { label: 'SISD', n: true }, 'Available', 'Type', 'Notes'], rows: X.tech.map(t => [esc(t.name), `${esc(t.posted)} km/h (${esc(t.sisd.basis)})`, esc(t.sisd.m) + ' m', t.available != null ? `${esc(t.available)} m ${t.sisdOk ? pill('met', 'ok') : pill('not met', 'bad')}` : 'enter it', `${esc(t.type.text)}<br><span class="note">${num(t.daily)} movements/day, ${num(t.peak)}/h: ${esc(t.volBasis)}</span>`, esc([t.pcuNote, t.spacing, `Use ${t.crashYears} years of CAS data`].filter(Boolean).join('. '))]) })
        + C.access.map((a, i) => accessCheck(P, a).sh ? `<div style="display:flex;gap:10px;flex-wrap:wrap">${esc(a.name || 'Access ' + (i + 1))}: ${row('Movements/day', F.num(`access.${i}.daily`, a.daily))}${row('Peak movements/h', F.num(`access.${i}.peak`, a.peak))}${row('HCV movements/week', F.num(`access.${i}.hcvWeek`, a.hcvWeek))}${row('HCV %', F.num(`access.${i}.hcvPct`, a.hcvPct))}${row('Measured 85th %ile (km/h)', F.num(`access.${i}.speed85`, a.speed85))}${row('Sight distance available (m)', F.num(`access.${i}.sightAvail`, a.sightAvail))}</div>` : '').join('')
        : '<p class="note">No access on a state highway.</p>'}<p class="note">${srcLink(SRC.nztaAccess)}; SISD ${srcLink(SRC.austroads)}</p></div>
      <div>${h4('State highway impact table')}${X.rows.length ? Kit.table({ head: ['Link', { label: 'AM with', n: true }, { label: 'AM Δ', n: true }, { label: 'PM with', n: true }, { label: 'PM Δ', n: true }, { label: 'veh/day', n: true }, { label: 'HCV %', n: true }, 'Pavement'],
        rows: X.rows.slice(0, 30).map(r => ({ fly: 'link:' + r.id, cells: [esc(`${r.ref} ${r.name}`), num(r.per.AM.with), r.per.AM.without != null ? U.fmt.signed(r.per.AM.with - r.per.AM.without, 0) : '–', num(r.per.PM.with), r.per.PM.without != null ? U.fmt.signed(r.per.PM.with - r.per.PM.without, 0) : '–', num(r.dayWith), num(r.hvPct, 1), r.pavement ? (r.pavement.flagged ? pill(r.pavement.proxy ? 'over 20% (proxy)' : 'over 20%', 'warn') : r.pavement.proxy ? 'ok (proxy)' : 'ok') : '–'] })) }) + `<p class="note">veh/h two-way; veh/day = busiest peak × ${num(P.settings.peakToDaily || 10)}. Pavement test: heavy vehicles a day up more than 20% (G00907 s5.4.6). ${esc(X.pavementBasis)}</p>${row('Development HV share (%)', F.num('ctx.devHvPct', C.ctx.devHvPct, 'e.g. 12'))}`
        : '<p class="note">Run the model for the state highway impact table.</p>'}
      ${X.nodes.length ? h4('State highway junctions') + Kit.table({ head: ['Junction', 'Period', { label: 'Delay with (s)', n: true }, { label: 'Without (s)', n: true }, 'LoS'], rows: X.nodes.slice(0, 20).map(n => ({ fly: 'node:' + n.id, cells: [esc(n.name), n.period, num(n.d1), num(n.d0), Kit.losBadge(n.los)] })) }) + disclaimerHtml('D3', P) : ''}
      </div></div>`;
  }
  function secAuthority(P) {
    const A = authorities(P), O = operators(P);
    return `<div class="row2"><div>${h4('Authorities')}${Kit.table({ head: ['Role', 'Body', 'Basis'], rows: A.rows.map(r => [esc(r.role), esc(r.name), esc(r.basis)]) })}
      ${A.transitionalText ? Kit.note(esc(A.transitionalText), true) : ''}${A.requiring ? Kit.note(esc(A.requiring)) : ''}<p class="note">${esc(A.arcf)}</p>
      <p class="note">Report wording: "${esc(A.reportName)}" where earlier reports said "AT review". AUP text naming Auckland Transport is quoted as written, with a footnote on the 2026 transfer.</p>${disclaimerHtml('D11', P)}</div>
      <div>${h4(`Affected operators · ${esc(O.periodText)}`)}<p class="note">${esc(O.label)} ${tag(ASSUMPTION)}</p>${O.note ? Kit.note(esc(O.note), true) : ''}
      ${Kit.table({ head: ['Operator', { label: 'Links in area', n: true }, { label: 'Links over threshold', n: true }, { label: 'Junctions over threshold', n: true }], rows: O.rows.map(r => [esc(r.name) + (r.affected ? ' ' + pill('affected', 'warn') : ''), num(r.assets), num(r.links.length), num(r.nodes.length)]) })}
      ${O.rows.flatMap(r => r.links.slice(0, 8).map(l => ({ fly: 'link:' + l.id, cells: [esc(r.name), esc(l.name), `+${l.dvc} V/C`] })).concat(r.nodes.slice(0, 8).map(n => ({ fly: 'node:' + n.id, cells: [esc(r.name), esc(n.name), `+${num(n.dd, 1)} s`] })))).length ? Kit.table({ head: ['Operator', 'Asset', 'Change'], rows: O.rows.flatMap(r => r.links.slice(0, 8).map(l => ({ fly: 'link:' + l.id, cells: [esc(r.name), esc(l.name), `+${l.dvc} V/C`] })).concat(r.nodes.slice(0, 8).map(n => ({ fly: 'node:' + n.id, cells: [esc(r.name), esc(n.name), `+${num(n.dd, 1)} s`] })))) }) : ''}
      <p class="note">Turn on <b>Affected operators</b> under Show on map to see them. Thresholds: Settings › Regulatory context.</p></div></div>`;
  }
  function secRegister(P) {
    const rows = Register.table(P), sc = activeSc(P), checks = P.scenarios.flatMap(s => Scenarios.check(s, P).map(x => ({ s, x }))).filter(o => o.x.level !== 'info');
    const R = P.register || [];
    return `<div class="bar-h"><h3>Future receiving environment (report section 2.8)</h3>${F.btn('reg-add', 'Add a register row', 'primary')}${F.btn('reg-presets', 'Add South Auckland presets')}${F.btn('export:compliance-register', 'Export (CSV)')}
      <span class="note">Do-minimum: granted and likely developments and funded projects only. Unfunded projects only in sensitivity scenarios (AT ITA s3.6.1). Permitted baseline: RMA s104(2), may disregard.</span></div>
      ${checks.filter(o => o.x.level === 'block').map(o => Kit.note(`<b>Blocking:</b> ${esc(o.x.text)}`, true)).join('')}
      ${Kit.table({ head: ['Name', 'Type', 'Status', 'Likelihood', 'Funding', 'Source and date', 'Included in S1', 'Reason', ''], rows: R.map((r, i) => {
        const t = rows[i] || {};
        return [`${F.txt(`@reg.${i}.name`, r.name, 'name', '150px')}${r.verify ? ' ' + pill('verify', 'warn') : ''}`, F.sel(`@reg.${i}.type`, REGISTER_TYPES, r.type), F.txt(`@reg.${i}.status`, r.status, 'granted / lodged', '110px'),
          F.sel(`@reg.${i}.likely`, { '': '–', likely: 'Likely', possible: 'Possible', unlikely: 'Unlikely' }, r.likely), r.type === 'network' ? F.sel(`@reg.${i}.funding`, { '': '–', ...FUNDING }, r.funding) : '–',
          `${F.txt(`@reg.${i}.source`, r.source, 'source', '140px')} ${F.date(`@reg.${i}.date`, r.date)}${r.asAt ? ` <span class="note">as at ${esc(r.asAt)}</span>` : ''}`, t.included ? pill('yes', 'ok') : 'no', esc(t.reason || ''), F.btn('reg-del:' + i, '✕', 'danger')];
      }), empty: 'No committed developments or network projects yet.' })}
      ${checks.filter(o => o.x.level === 'warn').slice(0, 10).map(o => `<p class="note warn">${esc(o.s.name)}: ${esc(o.x.text)}</p>`).join('')}
      ${roleList(P)}
      <p class="note">Scenario for "Included in S1": the future baseline built by the assessment-set wizard. ${esc(sc.name)} is active.</p>`;
  }
  // Each scenario with its role; the permitted baseline carries 'RMA s104(2): may disregard' (reg-receiving-env)
  function roleList(P) {
    const R = typeof SCENARIO_ROLES !== 'undefined' ? SCENARIO_ROLES : {};
    const label = s => { const l = (R[s.role] || {}).label || s.role; return s.role === 'permitted' && !/s104(2)/.test(l) ? `${l} (RMA s104(2): may disregard)` : l; };
    const L = P.scenarios.filter(s => s.role);
    return L.length ? `<p class="note">Scenarios: ${L.map(s => `${esc(s.name)}: <b>${esc(label(s))}</b>`).join(' · ')}</p>` : '<p class="note">No scenario has a role yet (do-minimum, proposal, sensitivity, permitted baseline): set them with the assessment-set wizard.</p>';
  }
  function secFDS(P) {
    const C = state(P), X = fdsCheck(P), R = P.register || [];
    return `<div class="row2"><div>${h4('FDS timing at the site')}<p><b>${esc(X.text)}</b>${X.red ? ' ' + pill('red-flagged in the FDS', 'warn') : ''}</p>
      ${X.fundingPrompt ? Kit.note(esc(X.fundingPrompt), true) + F.area('fds.fundingGap', C.fds.fundingGap, 3, 'How the capital and operating funding gap will be met') : ''}
      ${row('Decided regional spatial plan', `${F.txt('fds.rsp.name', C.fds.rsp.name, 'name (replaces the FDS when set)', '200px')} ${F.date('fds.rsp.date', C.fds.rsp.date)}`)}
      ${h4('FDS area by hand (from the strategy document)')}<p class="note">Use this when the layer lookup finds nothing or its fields have changed. ${X.source ? 'Now from: ' + esc(X.source) + '.' : ''}</p>
      ${row('FDS area', F.txt('site.fdsOverride.name', C.site.fdsOverride.name, 'e.g. Drury-Ōpāheke'))}${row('Timing band', F.txt('site.fdsOverride.timing', C.site.fdsOverride.timing, 'e.g. 2030+ or Live zoned'))}
      ${row('Source and page', `${F.txt('site.fdsOverride.source', C.site.fdsOverride.source, 'FDS 2023–2053', '170px')} p ${F.txt('site.fdsOverride.page', C.site.fdsOverride.page, 'page', '60px')}`)}
      ${h4('FDS-assumed infrastructure the site relies on')}${Kit.table({ head: ['Project', 'Register row', 'Funding', 'In S1', ''], rows: (C.fds.infra || []).map((it, i) => [F.txt(`fds.infra.${i}.name`, it.name, 'project', '150px'), F.sel(`fds.infra.${i}.reg`, { '': '–', ...Object.fromEntries(R.map(r => [r.id, r.name])) }, it.reg), esc(X.infra[i].funding), X.infra[i].included == null ? '–' : X.infra[i].included ? 'yes' : 'no', F.btn('fds-del:' + i, '✕', 'danger')]), empty: 'None listed.' })}
      <div style="display:flex;gap:6px;margin-top:6px">${F.btn('fds-add', 'Add infrastructure')}${F.btn('fds-pair', 'Compare with and without it (' + (P.settings.period || 'AM') + ')')}</div>
      ${C.fds.pair ? Kit.table({ head: ['Measure', { label: 'With', n: true }, { label: 'Without', n: true }, { label: 'Change', n: true }], rows: C.fds.pair.rows.map(r => [esc(r.label), num(r.with, r.dp), num(r.without, r.dp), r.d == null ? '–' : U.fmt.signed(r.d, r.dp)]) }) + `<p class="note">${esc(C.fds.pair.period)} peak, ${esc(longDate(C.fds.pair.at))}; not saved as scenarios.</p>` : ''}
      </div><div>${h4(esc(X.heading))}<p>${esc(X.paragraph)}</p><p class="note">${srcLink(SRC.fds)} · FDS layer: ${esc(LAYERS.fds.name)}</p></div></div>`;
  }
  function secRFI(P) {
    const C = state(P), L = lint(P), K = clocks(P), kinds = Object.fromEntries(Object.entries(CLOCKS).filter(([k]) => k !== 'hearing' && k !== 'works').map(([k, v]) => [k, v.label]));
    return `<div class="row2"><div>${h4(`Pre-lodgement check (${L.length} item${L.length === 1 ? '' : 's'})`)}${L.length ? Kit.table({ head: ['Item', 'What to do'], rows: L.map(l => [esc(l.text), esc(l.fix)]) }) : '<p class="note">Nothing flagged.</p>'}
      <p class="note">A list to reduce further-information requests; nothing here blocks an export.</p>
      ${h4('Response template')}<p class="note">${esc(rfiTemplate(P))}</p></div>
      <div>${h4('Statutory clocks')}
      <div style="display:flex;gap:6px;flex-wrap:wrap;align-items:center"><select id="cfEvKind">${Kit.opt(kinds, 's92')}</select><input type="date" id="cfEvDate"> ${F.btn('rfi-add', 'Add event', 'primary')}</div>
      ${row('Hearing date', F.date('rfi.hearing', C.rfi.hearing))}${row('State highway works start', F.date('rfi.worksDate', C.rfi.worksDate))}
      ${K.length ? K.map(k => `<div class="card" style="padding:6px 10px;margin:6px 0"><b>${esc(k.label)}</b> · ${esc(longDate(k.date))} ${k.id !== 'hearing' && k.id !== 'works' && k.id !== 'lodged' ? F.btn('rfi-del:' + k.id, '✕', 'danger') : ''}
        ${k.kind === 'ftaaS67' ? row('Panel-set date', `<input type="date" data-cfev="${esc(k.id)}" value="${esc(k.set || '')}">`) : ''}
        ${Kit.table({ head: ['Rule', 'Date', 'What'], rows: k.rows.map(r => [esc(r.id), `<b>${esc(longDate(r.date))}</b> <span class="note">${esc(dayName(r.date))}</span>`, esc(r.text)]) })}${k.warns.map(w => Kit.note(esc(w), true)).join('')}<p class="note">${esc(k.wd)}</p></div>`).join('') : '<p class="note">No events yet.</p>'}
      <p class="note">Working days exclude weekends, public holidays (Waitangi and Anzac Mondayised) and 20 Dec – 10 Jan (RMA s2) or 25 Dec – 2 Jan (Legislation Act s13, for the FTAA). ${srcLink(SRC.rma)}</p></div></div>`;
  }
  function secExpert(P) {
    const C = state(P), E = C.expert, st = signState(P), G = appendixG(P), D = C.diff;
    return `<div class="row2"><div>
      ${st.watermark ? Kit.note(`<b>${esc(st.watermark)}</b>`, true) : Kit.note(`Signed: ${st.valid.map(s => `${ROLES[s.role]} ${esc(s.name)} (${esc(s.quals)}) on ${esc(longDate(s.date))}`).join('; ')}`)}
      ${st.prompt ? Kit.note(esc(st.prompt), true) : ''}
      ${h4('Author')}${row('Name', F.txt('expert.author.name', E.author.name))}${row('Qualifications', F.txt('expert.author.quals', E.author.quals, 'e.g. BE(Civil), CMEngNZ, CPEng'))}${row('Organisation', F.txt('expert.author.org', E.author.org))}
      <div style="display:flex;gap:6px;flex-wrap:wrap;align-items:center;margin:6px 0"><select id="cfSignRole">${Kit.opt(Object.fromEntries(Object.entries(ROLES).map(([k, v]) => [k, v])), 'prepared')}</select>${F.btn('sign', 'Sign off as the author above', 'primary')}${E.signoffs.length ? F.btn('sign-clear', 'Withdraw sign-offs', 'danger') : ''}</div>
      ${row('Confidence', F.sel('expert.confidence.level', { '': 'Choose…', High: 'High', Moderate: 'Moderate', Low: 'Low' }, E.confidence.level))}${F.area('expert.confidence.why', E.confidence.why, 2, 'Why (Practice Note s9.3(e))')}
      ${F.area('expert.tests', E.tests, 2, 'Tests run and by whom')}${F.area('expert.limitations', E.limitations, 2, 'Limitations')}${F.area('expert.gaps', E.gaps, 2, 'Knowledge gaps')}
      ${h4('Assumption diff (joint witness statement)')}${F.area('diff.text', D.text, 4, 'Paste the other party\'s Spare Construct project file or settings JSON, or a key,value CSV such as: growth,2.5')}
      <div style="display:flex;gap:6px;margin:6px 0">${F.btn('diff-parse', 'List differences')}${D.items.length ? F.btn('diff-run', `Run ${D.items.length} difference${D.items.length === 1 ? '' : 's'} (${P.settings.period || 'AM'})`, 'primary') : ''}${D.result ? F.btn('export:compliance-jws', 'Export JWS table (CSV)') : ''}</div>
      ${D.items.length && !D.result ? Kit.table({ head: ['Item', 'This project', 'Other party'], rows: D.items.map(d => [esc(d.key), esc(d.a), esc(d.b)]) }) : ''}
      ${D.result ? Kit.table({ head: ['Item', 'A', 'B', 'Effect', { label: 'Rank score', n: true }], rows: D.result.rows.map(r => [esc(r.key), esc(r.a), esc(r.b), esc(r.effect), num(r.score, 2)]) }) + `<p class="note">${esc(D.result.note)}</p>` : ''}
      </div><div>${h4(esc(G.title))}${G.sections.map(([t, b]) => `<p><b>${esc(t)}.</b><br>${esc(b).replace(/\n/g, '<br>')}</p>`).join('')}${disclaimerHtml('D2', P)}</div></div>`;
  }
  function secStandards(P) {
    const sup = supersession(P), C = state(P);
    return `${sup.map(s => Kit.note(esc(s.text), true)).join('')}<div class="row2"><div>${h4('Standards register (Appendix A)')}
      ${Kit.table({ head: ['Document', 'Edition', 'Effective', 'Next known change', 'Used'], rows: STANDARDS.map(s => [`<a href="${esc(s.url)}" target="_blank" rel="noopener">${esc(s.doc)}</a>${s.verified ? '' : ' <span class="note">verify</span>'}${s.note ? `<br><span class="note">${esc(s.note)}</span>` : ''}`, esc(s.edition), esc(s.effective ? longDate(s.effective) : '–'), s.next ? `${s.next.date ? esc(longDate(s.next.date)) + ': ' : ''}${esc(s.next.what)}` : '–', C.used[s.id] ? esc(longDate(C.used[s.id])) : '–']) })}
      <p class="note">${esc(PLANNING_ONLY)}</p>
      <p class="note">CSV footers (D1, DRAFT, version stamp, model status) go on this tab's exports and on human-readable tables (${esc(STAMP.tables.join(', '))}). Hand-over files stay byte-clean for the tools that read them (${esc(Object.entries(STAMP.handover).map(([k, v]) => `${k} → ${v}`).join('; '))}); their D1 and D12 travel with the hand-over.</p></div>
      <div>${h4('Standing disclaimers')}<p class="note">These cannot be removed. You can add text after any of them.</p>
      ${Object.entries(DISCLAIMERS).map(([id, d]) => `<details><summary><b>${id}</b> ${esc(d.title)} <span class="note">· ${esc(d.place)}</span></summary><p class="note">${esc(disclaimer(id))}</p>${F.area('notes.' + id, C.notes[id], 2, 'Add text after this disclaimer (optional)')}</details>`).join('')}</div></div>`;
  }
  function secChecklist(P) {
    const all = checklist(P), list = all.filter(i => ui.ck === 'all' || (ui.ck === 'todo' ? i.applies && (i.state === 'todo' || i.state === 'warn') : i.applies));
    const groups = [...new Set(list.map(i => i.group))], ap = all.filter(i => i.applies);
    const chip = (k, l) => `<button class="chip ${ui.ck === k ? 'on' : ''}" data-ckf="${k}">${l}</button>`;
    return `<div class="bar-h"><h3>Regulatory checklist</h3><div class="chips">${chip('applicable', `Applies (${ap.length})`)}${chip('todo', `To do (${ap.filter(i => i.state === 'todo' || i.state === 'warn').length})`)}${chip('all', `All ${all.length}`)}</div>
      ${F.btn('export:compliance-checklist', 'Export (CSV)')}<span class="note">${ap.filter(i => i.state === 'done').length} done · auto items are filled by Spare Construct; record the rest yourself</span></div>
      ${groups.map(g => `${h4(esc(CK_GROUPS[g] || g))}${Kit.table({ head: ['', 'ID', 'Item', 'Spare Construct', 'Status', 'Note'], rows: list.filter(i => i.group === g).map(i => [stIcon(i.state), `<b>${i.id}</b>`,
        `${esc(i.item)}<br><span class="note">${esc(i.source)} · evidence: ${esc(i.evidence)}${i.refNote ? ` · checked: ${esc(i.refNote)}` : ''}</span>`, `<span class="note">${esc(i.fills)}${i.how ? ': ' + esc(i.how) : ''}</span>${i.text ? `<br>${esc(i.text)}` : ''}`,
        i.applies ? `<select data-ck-state="${i.id}">${Kit.opt({ auto: i.auto ? 'Auto' : 'Not recorded', done: 'Done', todo: 'To do', warn: 'Check', na: 'Not applicable' }, (state(P).checks[i.id] || {}).state || 'auto')}</select>` : 'n/a',
        i.applies ? `<input data-ck-note="${i.id}" value="${esc(i.note)}" placeholder="note" style="width:140px">` : '']) })}`).join('') || '<p class="note">Nothing to show.</p>'}`;
  }
  function html(P = Store.P) {
    const sec = SECS.some(s => s[0] === ui.sec) ? ui.sec : 'overview';
    const body = { overview: secOverview, context: secContext, site: secSite, scope: secScope, access: secAccess, sh: secSH, authority: secAuthority, register: secRegister, fds: secFDS, rfi: secRFI, expert: secExpert, standards: secStandards, checklist: secChecklist }[sec];
    let inner; try { inner = body(P); } catch (e) { console.error(e); inner = Kit.note(`This section could not be shown: ${esc(e.message)}. Other sections are unaffected.`, true); }
    return `<div class="bar-h"><h3>Compliance</h3><span class="note">${esc(regime(P).short)} · regulatory data as at ${esc(longDate(VERSION))}${ui.busy ? ' · ' + esc(ui.busy) : ''}</span></div>
      <div class="chips" style="margin:-2px 0 10px">${SECS.map(([k, l]) => `<button class="chip ${k === sec ? 'on' : ''}" data-csec="${k}">${l}</button>`).join('')}</div>${inner}`;
  }

  /* ---------- tab events ---------- */
  const redraw = () => { if (typeof Drawer !== 'undefined' && View.drawer === 'compliance') Drawer.render(); };
  const changed = () => { Store.emit('meta'); };
  async function job(label, fn) {
    ui.busy = label + '…'; redraw();
    try { const r = await fn(); return r; }
    catch (e) { Kit.toast(e.message, 'err'); }
    finally { ui.busy = ''; changed(); }
  }
  function onClick(e) {
    const sec = e.target.closest('[data-csec]'); if (sec) { ui.sec = sec.dataset.csec; redraw(); return true; }
    const ckf = e.target.closest('[data-ckf]'); if (ckf) { ui.ck = ckf.dataset.ckf; redraw(); return true; }
    const b = e.target.closest('[data-cdo]'); if (!b) return false;
    const P = Store.P, C = state(P), d = b.dataset.cdo, [act, arg] = [d.split(':')[0], d.slice(d.indexOf(':') + 1)];
    if (act === 'export') { UI.exportFile(arg); return true; }
    if (act === 'site-lookup') { job('Looking up the site', () => lookupSite(P)).then(r => r && Kit.toast(`Site: ${r.zone ? r.zone.name : 'zone not found'}${r.precincts && r.precincts.length ? ' · ' + r.precincts[0].name : ''}${r.errors.length ? ` (${r.errors.length} layer${r.errors.length === 1 ? '' : 's'} could not be read)` : ''}.`, r.errors.length ? 'warn' : '')); return true; }
    if (act === 'site-here') { const c = UI.map.getCenter(); Object.assign(C.site, { lat: U.r7(c.lat), lng: U.r7(c.lng), src: 'map centre', lookup: null }); changed(); return true; }
    if (act === 'site-sel') {
      const s = View.sel, sc = Store.sc, o = s && (s.type === 'zone' || s.type === 'gateway' ? sc.zones[s.id] : s.type === 'node' ? sc.nodes[s.id] : null);
      if (!o) { Kit.toast('Select a zone or a junction on the map first.', 'warn'); return true; }
      Object.assign(C.site, { lat: U.r7(o.lat), lng: U.r7(o.lng), src: s.type === 'node' ? 'selected junction' : `zone ${o.name || ''}`.trim(), lookup: null }); changed(); return true;
    }
    if (act === 'load-aup') { job('Loading AUP controls', () => loadOverlays(P)).then(o => o && Kit.toast(`AUP controls loaded: ${o.arterials.length} arterial lines, ${o.var.length} VAR, ${o.frontage.length} frontage.`, o.errors.length ? 'warn' : '')); return true; }
    if (act === 'load-sh') { job('Loading the NZTA state highway layer', () => loadSH(P)).then(r => r && Kit.toast(`${r.n} model links matched to state highways.`)); return true; }
    if (act === 'acc-add') {
      const s = View.sel, sc = Store.sc, node = s && s.type === 'node' && sc.nodes[s.id] ? s.id : null, c = node ? null : UI.map.getCenter();
      C.access.push({ id: U.uid('acc'), name: `Access ${C.access.length + 1}`, node, lat: node ? null : U.r7(c.lat), lng: node ? null : U.r7(c.lng) });
      changed(); return true;
    }
    if (act === 'acc-del') { C.access.splice(+arg, 1); changed(); return true; }
    if (act === 'mw-confirm' || act === 'mw-unconfirm') {
      if (BAD_SEG.test(arg)) return true;
      const m = own(C.mwConfirm, arg) && isObj(C.mwConfirm[arg]) ? C.mwConfirm[arg] : (C.mwConfirm[arg] = {});
      if (act === 'mw-unconfirm') m.confirmed = false;
      else if (!String(m.reason || '').trim()) { Kit.toast('Say why no other road access is reasonably available first (GRPA s84).', 'warn'); return true; }
      else Object.assign(m, { confirmed: true, by: C.expert.author.name || '', date: today() });
      changed(); return true;
    }
    if (act === 'scope-lock') { const s = C.scoping[arg] ||= {}; if (s.locked) s.locked = false; else { const m = scopingMemo(P).find(x => x.key === arg); Object.assign(s, { value: s.value || m.proposed, locked: true, by: s.by || C.expert.author.name || '', date: s.date || today() }); } changed(); return true; }
    if (act === 'prop-reset') { C.proportion.text = ''; changed(); return true; }
    if (act === 'reg-add') { (P.register ||= []).push({ id: U.uid('reg'), ref: '', name: 'New row', type: 'consent', yield: null, rates: null, status: '', likely: '', funding: null, source: '', date: today(), zones: [], links: [] }); changed(); return true; }
    if (act === 'reg-presets') { const have = new Set((P.register ||= []).map(r => r.id)), add = Register.presets().filter(r => !have.has(r.id)); P.register.push(...add); Kit.toast(add.length ? `Added ${add.length} preset rows, each tagged verify.` : 'The presets are already in the register.'); changed(); return true; }
    if (act === 'reg-del') { P.register.splice(+arg, 1); changed(); return true; }
    if (act === 'fds-add') { C.fds.infra.push({ name: '', reg: '' }); changed(); return true; }
    if (act === 'fds-del') { C.fds.infra.splice(+arg, 1); changed(); return true; }
    if (act === 'fds-pair') { job('Running with and without the FDS infrastructure', () => fdsPair(P)); return true; }
    if (act === 'rfi-add') {
      const k = U.$('#cfEvKind').value, dt = U.$('#cfEvDate').value;
      if (!validDay(dt)) { Kit.toast('Choose the date of the event first.', 'warn'); return true; }
      C.rfi.events.push({ id: U.uid('ev'), kind: k, date: dt }); changed(); return true;
    }
    if (act === 'rfi-del') { C.rfi.events = C.rfi.events.filter(x => x.id !== arg); changed(); return true; }
    if (act === 'sign') { try { const s = signOff(P, { role: U.$('#cfSignRole').value }); Kit.toast(`${ROLES[s.role]} ${s.name}: signed off. Changing any input clears it.`); changed(); } catch (err) { Kit.toast(err.message, 'warn'); } return true; }
    if (act === 'sign-clear') { C.expert.history.push(...C.expert.signoffs.map(s => ({ ...s, cleared: today(), reason: 'withdrawn' }))); C.expert.signoffs = []; changed(); return true; }
    if (act === 'diff-parse') { try { C.diff.items = diffParams(P, C.diff.text); C.diff.result = null; Kit.toast(C.diff.items.length ? `${C.diff.items.length} difference${C.diff.items.length === 1 ? '' : 's'} found.` : 'No differences in the parameters Spare Construct compares.'); } catch (err) { Kit.toast(err.message, 'warn'); } changed(); return true; }
    if (act === 'diff-run') { job('Running each difference', async () => { C.diff.result = await runDiff(P, C.diff.items, { onProgress: (i, n, k) => { ui.busy = `difference ${i} of ${n}: ${k}`; Kit.busy(`Assumption diff · ${i} of ${n} · ${k}`); } }); Kit.busy(null); }); return true; }
    return false;
  }
  function onChange(e) {
    const t = e.target, P = Store.P, C = state(P);
    if (t.dataset.ckState) { const c = C.checks[t.dataset.ckState] ||= {}; c.state = t.value === 'auto' ? '' : t.value; c.by = C.expert.author.name || c.by || ''; c.date = today(); changed(); return true; }
    if (t.dataset.ckNote) { (C.checks[t.dataset.ckNote] ||= {}).note = t.value.trim(); changed(); return true; }
    if (t.dataset.cfev) { const ev = C.rfi.events.find(x => x.id === t.dataset.cfev); if (ev) { ev.set = t.value; changed(); } return true; }
    const path = t.dataset.cf; if (!path) return false;
    if (path.split('.').some(x => BAD_SEG.test(x))) return true;   // never write through __proto__ and the like
    const v = t.dataset.t === 'num' ? (t.value === '' ? null : +t.value) : t.dataset.t === 'bool' ? t.checked : t.value.trim();
    if (path.startsWith('@reg.')) { const [, i, k] = path.split('.'); const r = (P.register || [])[+i]; if (r) { r[k] = v === '' ? null : v; if (k === 'type' && v !== 'network') r.funding = null; } changed(); return true; }
    Kit.setPath(C, path, v);
    if (path === 'proportion.text' && v !== '') C.proportion.confirmed = false;
    changed(); return true;
  }

  /* ====================================================================================================
     16. Report sections, checklist items, exports, settings, inspector sections, map layer and hooks
     ==================================================================================================== */
  /* Report sections (Registry.report). The report lane numbers sections and prints each title as the heading, so the
     bodies carry no heading of their own. Chapters follow js/packs/nz/report.js: 'front', '1'–'6', 'A'–'H', 'P'.
     'report-regulatory' replaces the report lane's simpler regime table (same id, the registry contract), so the
     pathway, lodgement date and regime come from one place: this tab. */
  const T = (head, rows) => `<table class="rt"><thead><tr>${head.map(h => `<th>${esc(h)}</th>`).join('')}</tr></thead><tbody>${rows.map(r => `<tr>${r.map(c => `<td>${c == null ? '' : c}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  const P_ = t => `<p>${esc(t)}</p>`;
  const D_ = (id, P) => `<div class="disc" data-d="${id}" data-disclaimer="${id}"><b>${id} ${esc(DISCLAIMERS[id].title)}.</b> ${esc(disclaimer(id, P))}</div>`;
  const REPORT = [
    { id: 'report-regulatory', chapter: 'front', order: 3, title: 'Regulatory context', tag: 'auto', render: P => { const s = stamp(P), A = authorities(P), sg = signState(P);
      return `${sg.watermark ? `<p class="warn"><b>${esc(sg.watermark)}</b></p>` : ''}${T(['Item', 'Value'], s.lines.map(([a, b]) => [esc(a), esc(b)]))}${recheck(P).show ? `<p class="warn"><b>${esc(REGIME.recheck)}.</b></p>` : ''}
        <h3>Authorities</h3>${T(['Role', 'Body', 'Basis'], A.rows.map(r => [esc(r.role), esc(r.name), esc(r.basis)]))}${A.transitionalText ? P_(A.transitionalText) : ''}${A.requiring ? P_(A.requiring) : ''}${P_(A.arcf)}${D_('D10', P)}${D_('D11', P)}`; } },
    { id: 'compliance-model-status', chapter: 'B', order: 5, title: 'Model purpose and status', tag: 'auto', render: P => { const m = ['AM', 'PM'].map(p => Engine.modelStatus(P, p)), S = P.settings.tmdg || {}, msm = (P.data || {}).msm;
      return `${T(['Period', 'Status', 'TMDG category', 'Date'], m.map(x => [x.period, esc(x.label), esc(x.category), esc(x.date || '–')]))}${P_(`TMDG category ${String(S.category || 'E').toUpperCase()}. Justification: ${S.justification || '[author to justify the TMDG category (Settings › Calibration and validation › Why this category)]'}`)}
        ${P_(Engine.STANDARDS.tmdg.standing)}${msm && msm.links ? '' : P_(Engine.STANDARDS.msm.notChecked)}${D_('D6', P)}${D_('D7', P)}`; } },
    { id: 'compliance-e27', chapter: '1', order: 25, title: 'AUP E27 transport triggers', tag: 'auto', render: P => { const E = e27(P), A = state(P).access.map(a => accessCheck(P, a));
      return `${E.redirect ? P_(E.redirect.text + '.') : ''}${E.exempt ? P_(E.exempt.text + '.') : ''}${E.pc120 ? P_(E.pc120.text) + `<p class="note">${esc(E.pc120.status)}</p>` : ''}
        ${T(['Rule', 'Threshold', 'Proposal quantity', 'Exceeded', 'Tier', 'Consequence'], E.rows.filter(r => r.q > 0 || r.exceeded).map(r => [r.id, `${num(r.thr)} ${esc(r.unit)}`, num(r.q), r.exceeded ? 'Yes' : 'No', r.tier, esc(r.consequence)]))}
        ${P_(`${E.catchAll.id}: ${E.catchAll.why}`)}${E.tier ? P_(E.consequence) : ''}${E.notes.map(P_).join('')}${P_(E.wording)}<p class="note"><em>${esc(E.version)}</em></p>
        ${A.some(a => a.flags.length) ? '<h3>Access infringements (E27.6.4)</h3>' + T(['Access', 'Rule', 'Status', 'Finding'], A.flatMap(a => a.flags.map(f => [esc(a.name), esc(f.rule), f.status, esc(f.text)]))) : ''}${D_('D13', P)}`; } },
    { id: 'compliance-discretion', chapter: '1', order: 26, title: 'Matters of discretion and scope limits', tag: 'auto', render: P => { const E = e27(P), r = regime(P);
      return `${P_(E.tier === 1 ? `${E27.tier1Scope}. ${E.consequence}` : E.tier === 2 ? E27.tier2Scope + '.' : 'No E27.6.1 matters of discretion are triggered.')}${E.supplementary ? P_(`Network level-of-service sections are ${E27.supplementary}.`) : ''}${P_(E27.s104C + '.')}
        ${r.side === 'transition' ? P_('For a district land use consent lodged from 30 October 2026, the effects listed in s104(1A) are disregarded (s104(1A), s92(2AA)), and Planning Act and NEA instruments are not relied on (s104(1C)).') : ''}
        <h3>Objectives and policies (E27.2, E27.3)</h3>${T(['Provision', 'Applies', 'Evidence'], E.matrix.map(m => [`${m.ref} ${esc(m.text)}`, m.applies ? 'Yes' : `No (${esc(m.why)})`, esc(m.evidence.join('; '))]))}
        <p class="note">The objectives and policies are paraphrased; quote the plan text in the evaluation.</p>`; } },
    { id: 'compliance-scope', chapter: '1', order: 40, title: 'ITA scope and proportionality', tag: 'mixed', render: P => { const I = itaScope(P);
      return `${I.aup.map(P_).join('')}${I.at.length ? '<h3>AT ITA Guidelines 2025 v2: ITA likely</h3><ul>' + I.at.map(t => `<li>${esc(t)}</li>`).join('') + '</ul>' : ''}
        ${I.nzta ? `<h3>${esc(NZTA_T1.label)} Table 1</h3>${P_(I.nzta.text)}${I.depth ? P_(I.depth.text) : ''}${P_(I.nzta.order)}` : ''}
        <h3>Proportionality statement</h3>${P_(I.paragraph.text)}${I.paragraph.confirmed ? P_(`Confirmed by ${I.paragraph.by || 'the author'}${I.paragraph.date ? ' on ' + longDate(I.paragraph.date) : ''}.`) : '<p class="warn">[Not yet confirmed by the author.]</p>'}
        ${T(['Driver', 'Value'], I.drivers.map(([a, b]) => [esc(a), esc(b)]))}${P_(`Recommended level: ${I.level} (${ASSUMPTION} bands).`)}
        <h3>Scoping memo</h3>${T(['Item', 'Proposed', 'Agreed by', 'Date'], scopingMemo(P).map(m => [esc(m.label), esc(m.value), esc(m.by), esc(m.date)]))}${D_('D13', P)}`; } },
    { id: 'compliance-access', chapter: '3', order: 25, title: 'Access compliance (E27.6.4)', tag: 'auto', render: P => { const A = state(P).access.map(a => accessCheck(P, a));
      if (!A.length) return '';
      return `${T(['Access', 'Status', 'Findings'], A.map(a => [esc(a.name), esc(accStatus(a)), [...a.flags.map(f => esc(`${f.rule}: ${f.text}`)), a.uncheckedText ? `<i>${esc(a.uncheckedText)}</i>` : ''].filter(Boolean).join('<br>') || 'No breach found'] ))}${P_(ACCESS.nearBasis + '.')}${D_('D13', P)}`; } },
    // Marks the network level-of-service sections of chapter 4 (extent of effects, junction performance) for an E27 tier 1 proposal
    { id: 'compliance-network-scope', chapter: '4', order: 35, title: 'Scope of the network assessment (E27 tier 1)', tag: 'auto', render: P => { const E = e27(P); if (!E.supplementary) return '';
      return `${P_(`E27 tier 1: only TA1, T1A, T2A or T3A is exceeded, so the matters of discretion are ${E27.tier1Scope.replace(/^[^:]+: /, '')} (E27.8.1(4A), E27.8.2(3A)).`)}${P_(`The network level-of-service sections that follow (extent of effects, junction performance) are marked "${E27.supplementary}". ${E27.s104C}.`)}`; } },
    { id: 'compliance-sh', chapter: '4', order: 440, title: 'State highway effects', tag: 'auto', render: P => { const X = shCheck(P);
      if (!X.inArea && !X.involved) return '';
      return `${X.triggers.length ? '<ul>' + X.triggers.map(t => `<li>${esc(t)}</li>`).join('') + '</ul>' : P_('No NZTA trigger case found.')}
        ${T(['Approval', 'Needed'], X.approvals.map(a => [esc(a.label), a.needed ? 'Yes' : a.check ? 'Check' : 'No']))}
        ${X.rows.length ? T(['Link', 'AM with (veh/h)', 'AM without (veh/h)', 'PM with (veh/h)', 'PM without (veh/h)', 'With (veh/day)', 'HCV %', 'Pavement'], X.rows.slice(0, 40).map(r => [esc(`${r.ref} ${r.name}`), num(r.per.AM.with), num(r.per.AM.without), num(r.per.PM.with), num(r.per.PM.without), num(r.dayWith), num(r.hvPct, 1), r.pavement ? esc(r.pavement.text) + (r.pavement.flagged ? ': pavement assessment needed' : '') + (r.pavement.proxy ? ' (total-traffic proxy)' : '') : '–'])) + P_(X.pavementBasis) : ''}
        ${X.tech.length ? T(['Access', 'Required SISD (m)', 'Basis', 'Movements a day', 'Access type'], X.tech.map(t => [esc(t.name), esc(t.sisd.m), esc(t.sisd.basis), `${num(t.daily)} (${esc(t.volBasis)})`, esc(t.type.text)])) : ''}${X.noise ? P_('A noise-sensitive activity is within 100 m of a state highway: assess reverse sensitivity and acoustic design (NZTA PPM Managing effects s5.2.2).') : ''}
        ${P_(X.labels.join('; '))}${D_('D12', P).replace(/\[[^\]]*\]/, 'an NZTA access application')}`; } },
    { id: 'compliance-fds', chapter: '4', order: 450, title: 'Future Development Strategy consistency', tag: 'mixed', render: P => { const X = fdsCheck(P), C = state(P); if (!X.applies) return '';
      return `<p><b>${esc(X.heading)}</b></p>${P_(X.paragraph)}${X.fundingPrompt ? P_(C.fds.fundingGap || `[${X.fundingPrompt}]`) : ''}${C.fds.pair ? T(['Measure', 'With', 'Without', 'Change'], C.fds.pair.rows.map(r => [esc(r.label), num(r.with, r.dp), num(r.without, r.dp), r.d == null ? '–' : U.fmt.signed(r.d, r.dp)])) : ''}`; } },
    { id: 'compliance-operators', chapter: '5', order: 600, title: 'Affected operators and notification input', tag: 'auto', render: P => { const O = operators(P);
      return `${P_(O.label)}${O.note ? `<p class="warn">${esc(O.note)}</p>` : ''}${T(['Operator', 'Links in area', 'Links over threshold', 'Junctions over threshold', 'Affected'], O.rows.map(r => [esc(r.name), num(r.assets), num(r.links.length), num(r.nodes.length), r.affected ? 'Yes' : 'No']))}
        ${regime(P).side === 'transition' ? P_('For applications lodged from 30 October 2026, AUP cross-references to s95A(9) have no target (repealed); s95B(10) remains.') : ''}${D_('D11', P)}${D_('D13', P)}`; } },
    { id: 'compliance-standards', chapter: 'A', order: 60, title: 'Regulatory standards register', tag: 'auto', render: P => `${supersession(P).map(s => `<p class="warn">${esc(s.text)}</p>`).join('')}${T(['Document', 'Edition', 'Effective', 'Next known change', 'Checked against source'], STANDARDS.map(s => [esc(s.doc), esc(s.edition), esc(s.effective ? longDate(s.effective) : '–'), esc(s.next ? `${s.next.date ? longDate(s.next.date) + ': ' : ''}${s.next.what}` : '–'), s.verified ? 'Yes' : 'No']))}${P_(PLANNING_ONLY)}` },
    { id: 'compliance-expert', chapter: 'G', order: 20, title: 'Expert disclosure material', tag: 'mixed', render: P => { const G = appendixG(P);
      return `${G.watermark ? `<p class="warn"><b>${esc(G.watermark)}</b></p>` : ''}${G.prompt ? `<p class="warn">${esc(G.prompt)}</p>` : ''}${G.sections.map(([t, b]) => `<h3>${esc(t)}</h3><p>${esc(b).replace(/\n/g, '<br>')}</p>`).join('')}
        <h3>Sign-off</h3>${G.signoffs.length ? T(['Role', 'Name', 'Qualifications', 'Date'], G.signoffs.map(s => [ROLES[s.role], esc(s.name), esc(s.quals), esc(longDate(s.date))])) : P_('Not signed off.')}${D_('D2', P)}`; } },
    { id: 'compliance-timeline', chapter: 'H', order: 10, title: 'Statutory timeline', tag: 'auto', render: P => { const K = clocks(P);
      return K.length ? T(['Event', 'Date', 'Rule', 'Due', 'What'], K.flatMap(k => k.rows.map(r => [esc(k.label), esc(longDate(k.date)), esc(r.id), esc(longDate(r.date)), esc(r.text)]))) + P_('Working days exclude weekends, public holidays and the statutory Christmas period (RMA s2; Legislation Act 2019 s13 for the FTAA).') : P_('No statutory events recorded.'); } },
    { id: 'compliance-checklist', chapter: 'P', order: 990, title: 'Regulatory compliance checklist', tag: 'mixed', render: P => { const all = checklist(P), L = all.filter(i => i.applies);
      return `${T(['ID', 'Item', 'Source', 'Status', 'Spare Construct finding or note'], L.map(i => [i.id, esc(i.item), esc(i.source + (i.refNote ? ` (checked: ${i.refNote})` : '')), esc(CK_STATE[i.state] || i.state), esc([i.text, i.note].filter(Boolean).join(' · '))]))}<p class="note">Items that do not apply to the ${esc(regime(P).pathwayLabel.toLowerCase())} pathway are left out (${all.length - L.length} of ${CHECKLIST.length}).</p>`; } },
  ];
  const CK_STATE = { done: 'Done', todo: 'To do', warn: 'Check', na: 'Not applicable' };
  const EXPORTS = [
    { id: 'compliance-checklist', label: 'Regulatory checklist (CSV)', ext: 'csv', mime: 'text/csv', order: 80, filename: c => `${c.fileBase}-regulatory-checklist.csv`, build: P => checklistCSV(P) },
    { id: 'compliance-scoping', label: 'Scoping memo (CSV)', ext: 'csv', mime: 'text/csv', order: 81, authorText: true, filename: c => `${c.fileBase}-scoping-memo.csv`, build: P => stampCSV(U.csv([['Item', 'Proposed', 'Agreed by', 'Date', 'Locked'], ...scopingMemo(P).map(m => [m.label, m.value, m.by, m.date, m.locked ? 'yes' : 'no'])]), P) },
    { id: 'compliance-register', label: 'Committed-development register (CSV)', ext: 'csv', mime: 'text/csv', order: 82, enabled: () => !!(Store.P.register || []).length, filename: c => `${c.fileBase}-register.csv`,
      build: P => stampCSV(U.csv([['Name', 'Type', 'Status', 'Likelihood', 'Funding', 'Source', 'As at', 'Verify', 'Included in S1', 'Reason'], ...Register.table(P).map(r => [r.name, r.type, r.status, r.likely, r.funding, r.source, r.asAt, r.verify ? 'verify' : '', r.included ? 'yes' : 'no', r.reason])]), P) },
    { id: 'compliance-timeline', label: 'Statutory timeline (CSV)', ext: 'csv', mime: 'text/csv', order: 83, enabled: () => clocks(Store.P).length > 0, filename: c => `${c.fileBase}-statutory-timeline.csv`,
      build: P => stampCSV(U.csv([['Event', 'Event date', 'Rule', 'Due', 'What'], ...clocks(P).flatMap(k => k.rows.map(r => [k.label, k.date, r.id, r.date, r.text]))]), P) },
    { id: 'compliance-jws', label: 'Joint witness statement inputs (CSV)', ext: 'csv', mime: 'text/csv', order: 84, authorText: true, enabled: () => !!state(Store.P).diff.result, filename: c => `${c.fileBase}-jws-inputs.csv`, build: P => jwsCSV(P, state(P).diff.result) },
  ];
  /* CSV stamping is opt-in (reg-disclaimers, reg-regime-stamp). Human-readable tables get D1, the DRAFT watermark,
     the version stamp and the model status as footer comment rows. Machine hand-over files stay byte-clean: a comment
     row breaks the SIDRA 11 loader (junctions lane) and Spare Construct's own OD importer, so their D1 and D12 travel with
     the hand-over (data pack header, loader script, volume grids). Another lane opts in with stamp: true (and
     handover: '<tool>' for D12), or opts out with stamp: false; the definition is replaced through the registry,
     never edited in place. */
  const STAMP = {
    tables: ['links', 'junctions', 'comparison', 'assumptions', 'landuse-tripgen', 'calibration-counts', 'calibration-summary', 'calibration-turns', 'calibration-screenlines', 'onboarding-changes'],
    handover: { 'junctions-sidra-volumes': 'SIDRA', 'junctions-turning': 'SIDRA', od: 'SATURN / Aimsun', 'calibration-cordon-csv': 'SATURN / Aimsun' },
  };
  const stamps = d => !d._stamped && d.stamp !== false && /^csv$/i.test(d.ext) && !own(STAMP.handover, d.id) && (d.stamp === true || STAMP.tables.includes(d.id));
  function stampExports() {
    for (const d of Registry.list('exports')) {
      if (!stamps(d)) continue;
      const build = d.build, handover = typeof d.handover === 'string' && d.handover ? d.handover : null;
      Registry.exports({ ...d, _stamped: true, build: (P, c) => {
        const out = build(P, c); if (typeof out !== 'string') return out;
        try { recordOutputs(P); return stampCSV(out, P, handover ? { handover } : undefined); } catch (e) { console.error('Compliance stamp failed:', e); return out; }
      } });
    }
  }

  function register() {
    Registry.tabs({ id: 'compliance', label: 'Compliance', order: 85, title: 'Pathway, regime, E27 triggers, ITA scope, access, state highways and the regulatory checklist',
      render: () => html(Store.P), onClick, onChange });
    for (const r of REPORT) Registry.report(r);
    // the report lane may load after this file; re-register the replaced section once every script has loaded
    Registry.onInit(() => Registry.report(REPORT.find(r => r.id === 'report-regulatory')));
    for (const x of EXPORTS) Registry.exports({ ...x, _stamped: true });
    Registry.settings({ id: 'compliance', title: 'Regulatory context', order: 60,
      defaults: () => ({ compliance: { dvc: OPS.dvc, ddelay: OPS.ddelay, vicinity: E27.tier1Area.m, evidence: ACCESS.evidenceM, precincts: '' } }),
      render: s => { const c = s.compliance || {}; return `<div class="set-grid">
        <label class="fld"><span>Affected operator: V/C increase</span><span class="fld-in"><input type="number" step="0.01" data-set="compliance.dvc" value="${esc(c.dvc)}"><span class="unit">${ASSUMPTION}</span></span></label>
        <label class="fld"><span>Affected operator: delay increase</span><span class="fld-in"><input type="number" step="1" data-set="compliance.ddelay" value="${esc(c.ddelay)}"><span class="unit">s · ${ASSUMPTION}</span></span></label>
        <label class="fld"><span>Tier 1 study area</span><span class="fld-in"><input type="number" step="50" data-set="compliance.vicinity" value="${esc(c.vicinity)}"><span class="unit">m walking · ${ASSUMPTION}</span></span></label>
        <label class="fld"><span>Access evidence radius</span><span class="fld-in"><input type="number" step="10" data-set="compliance.evidence" value="${esc(c.evidence)}"><span class="unit">m · ${ASSUMPTION}</span></span></label>
        <label class="fld" style="grid-column:1/-1"><span>More transport precincts</span><span class="fld-in"><textarea data-set="compliance.precincts" rows="2" style="width:100%" placeholder="One per line: I123 | Precinct name | rule (I450 Drury Centre and I451 Drury East are built in)">${esc(c.precincts)}</textarea></span></label></div>`; } });
    Registry.about({ id: 'compliance', title: 'Regulatory data', order: 70, intro: `Regulatory rules are stored as dated data (as at ${longDate(VERSION)}); map layers are read live for your site.`,
      rows: () => [
        ['AUP zones, precincts and controls', `<a href="${LAYERS.zone.url}" target="_blank" rel="noopener">Auckland Council ArcGIS services</a>`, `<b>Real.</b> Base zones, precincts, Centre Fringe Office Control, arterials, VAR, frontage and level-crossing controls, designations. ${esc(AC_LICENCE)}.`],
        ['PC120 proposed zoning', `<a href="${LAYERS.pc120.url}" target="_blank" rel="noopener">${esc(LAYERS.pc120.name)}</a>`, '<b>Real.</b> Council\'s proposed amendments to PC120; check the decision (due by June 2027).'],
        ['FDS timing', `<a href="${LAYERS.fds.url}" target="_blank" rel="noopener">${esc(LAYERS.fds.name)}</a>`, '<b>Live layer, not checked.</b> FDS 2023–2053 future urban areas and timing bands; the layer and field names (fds_strategicareaname, fds_timing) are assumed and have no test fixture. If the lookup finds nothing, enter the area, timing and strategy page by hand (Compliance › FDS).'],
        ['NZTA general circulars 25/01 and 26/01', srcLink(SRC.gc2501) + ' · ' + srcLink(SRC.gc2601), 'Checked against the copies saved in docs/references on 9 Oct 2026: discount rates; MBCM v1.7.5 and CEC 2nd edition v4 from 29 May 2026.'],
        ['AUP modification schedule 2026', srcLink(SRC.aupMods), 'Checked against the saved copy on 9 Oct 2026: PC79 (E27) fully operative 8 May 2026; PC120 amendments and partial withdrawal notified 27 Aug 2026.'],
        ['State highways', `<a href="${LAYERS.sh.url}" target="_blank" rel="noopener">${esc(LAYERS.sh.name)}</a>`, `<b>Real</b>, digitised 2022, conflated to model links. ${esc(NZTA_LICENCE)}.`],
        ['E27 thresholds and access rules', srcLink(SRC.e27), 'Checked against the July 2026 print. Check the operative text at lodgement.'],
        ['Working days and Matariki', srcLink(SRC.rma) + ' · ' + srcLink(SRC.legAct) + ' · ' + srcLink(SRC.matariki), 'Checked against the legislation text on 9 Oct 2026.'],
        ['NZTA ITA Guide Table 1 and 2, SISD', srcLink(SRC.nztaIta) + ' · ' + srcLink(SRC.austroads), '<b>Not verified</b> against the PDFs (download blocked); labelled verify.'],
      ] });
    Registry.checklist({ id: 'compliance-context', group: 'Regulatory', order: 10, label: 'Pathway, lodgement date and regime', source: 'Planning Act 2026 Sch 1 cl 18-21',
      status: P => { const r = regime(P); return { state: r.code === 'unset' || r.code === 'undated' ? 'todo' : 'done', text: r.code === 'unset' || r.code === 'undated' ? r.text : `${esc(r.pathwayLabel)}: ${esc(r.text)}`, action: { label: 'Open', run: () => { ui.sec = 'context'; UI.openTab('compliance'); } } }; } });
    Registry.checklist({ id: 'compliance-e27', group: 'Regulatory', order: 20, label: 'E27 trigger screening', source: 'AUP E27.6.1',
      status: P => { const E = e27(P), s = siteInfo(P); return { state: s.zone || s.looked ? 'done' : 'todo', text: s.zone || s.looked ? `E27 screening: ${E.redirect ? esc(E.redirect.text) : E.exempt ? esc(E.exempt.text) : E.tier ? `tier ${E.tier} triggered` : 'no threshold exceeded'}` : 'E27 screening: look up the site zone first', action: { label: 'Open', run: () => { ui.sec = 'site'; UI.openTab('compliance'); } } }; } });
    Registry.checklist({ id: 'compliance-scope', group: 'Regulatory', order: 30, label: 'Proportionality statement', source: 'RMA s88(2AA)',
      status: P => { const c = state(P).proportion; return { state: c.confirmed ? 'done' : 'todo', text: c.confirmed ? 'Proportionality statement confirmed' : 'Confirm the proportionality statement', action: { label: 'Open', run: () => { ui.sec = 'scope'; UI.openTab('compliance'); } } }; } });
    Registry.checklist({ id: 'compliance-signoff', group: 'Regulatory', order: 40, label: 'Expert sign-off', source: 'Practice Note 2023 s9.3',
      status: P => { const s = signState(P); return { state: s.signed ? 'done' : s.prompt ? 'warn' : 'todo', text: s.signed ? `Signed off by ${esc(s.valid[0].name)}` : s.prompt ? 'Sign-off cleared: inputs changed' : 'Not signed off (exports carry DRAFT)', action: { label: 'Open', run: () => { ui.sec = 'expert'; UI.openTab('compliance'); } } }; } });
    Registry.checklist({ id: 'compliance-standards', group: 'Regulatory', order: 50, label: 'Standards current', source: 'Standards register',
      status: P => { const s = supersession(P); return { state: s.length ? 'warn' : 'done', text: s.length ? `${s.length} standard${s.length === 1 ? '' : 's'} changed since outputs were stamped` : 'No standard has changed since outputs were stamped', action: { label: 'Open', run: () => { ui.sec = 'standards'; UI.openTab('compliance'); } } }; } });
    Registry.checklist({ id: 'compliance-items', group: 'Regulatory', order: 60, label: 'Regulatory checklist', source: 'docs/regulatory/_checklist.md',
      status: P => { const L = checklist(P).filter(i => i.applies), d = L.filter(i => i.state === 'done').length; return { state: d === L.length ? 'done' : L.some(i => i.state === 'warn') ? 'warn' : 'todo', text: `Regulatory checklist: ${d} of ${L.length} applicable items done`, action: { label: 'Open', run: () => { ui.sec = 'checklist'; UI.openTab('compliance'); } } }; } });
    Registry.inspectorSections({ id: 'compliance-link', types: ['link'], order: 80, render: (sc, sel) => {
      const L = sc.links[sel.id]; if (!L || !Store.P) return '';
      const a = linkAuthority(Store.P, L, sc), u = state(Store.P).net.shUser[L.id];
      return Kit.sec('Authority', `<p class="note">${esc(a.name)}${a.also.length ? ' · also ' + esc(a.also.join(', ')) : ''}<br>Decisions on speed, signals, crossings and closures: ${esc(a.decision.who)}<br><span class="note">${esc(a.decision.basis || '')}</span></p>
        <label class="fld"><span>State highway</span><span class="fld-in"><select data-csh="${esc(L.id)}">${Kit.opt({ '': `Automatic (${a.sh.sh ? 'yes' : 'no'}: ${a.sh.basis})`, yes: 'Yes', no: 'No' }, u === true ? 'yes' : u === false ? 'no' : '')}</select></span></label>`);
    }, onChange(e) { const t = e.target.closest('[data-csh]'); if (!t) return false; const n = state(Store.P).net.shUser; if (t.value === '') delete n[t.dataset.csh]; else n[t.dataset.csh] = t.value === 'yes'; Store.emit('meta'); return true; } });
    Registry.inspectorSections({ id: 'compliance-node', types: ['node'], order: 80, render: (sc, sel) => {
      if (!Store.P || !sc.nodes[sel.id]) return '';
      const C = state(Store.P), acc = C.access.find(a => a.node === sel.id);
      return Kit.sec('Compliance', `<label class="fld"><span>Pedestrians (peak hour)</span><span class="fld-in"><input type="number" min="0" data-cped="${esc(sel.id)}" value="${esc(C.peds[sel.id] ?? '')}"><span class="unit">people/h</span></span></label>
        <p class="note">Field count for the RFI check at altered junctions.</p>${acc ? `<p class="note">Site access: ${esc(acc.name)} (${fl(accessCheck(Store.P, acc, sc).worst)})</p>` : `<button class="btn small" data-cacc="${esc(sel.id)}">Use as a site access</button>`}`);
    }, onChange(e) { const t = e.target.closest('[data-cped]'); if (!t) return false; const C = state(Store.P); if (t.value === '') delete C.peds[t.dataset.cped]; else C.peds[t.dataset.cped] = +t.value; Store.emit('meta'); return true; },
    onClick(e) { const b = e.target.closest('[data-cacc]'); if (!b) return false; const C = state(Store.P); C.access.push({ id: U.uid('acc'), name: `Access ${C.access.length + 1}`, node: b.dataset.cacc }); Store.emit('meta'); Kit.toast('Added as a site access. Fill in the frontage and crossings in Compliance › Access.'); return true; } });
    Registry.layers({ id: 'compliance-ops', label: 'Affected operators', order: 60, defaultOn: false,
      draw(ctx, off, size, geo) {
        if (!Store.P) return;
        const O = operatorsCached(Store.P), css = n => { try { return getComputedStyle(document.documentElement).getPropertyValue(n).trim() || '#c92a2a'; } catch { return '#c92a2a'; } };
        const col = { nzta: css('--accent'), rca: css('--danger'), cco: css('--ok'), kiwirail: css('--amber') };
        ctx.lineCap = 'round';
        for (const r of O.rows) for (const l of r.links) { const g = geo.byId && geo.byId.get(l.id); if (!g) continue; ctx.strokeStyle = col[r.key] || css('--danger'); ctx.globalAlpha = 0.55; ctx.lineWidth = 9; ctx.beginPath(); for (let i = 0; i < g.xy.length; i += 2) { const x = g.xy[i] - off.x, y = g.xy[i + 1] - off.y; i ? ctx.lineTo(x, y) : ctx.moveTo(x, y); } ctx.stroke(); }
        ctx.globalAlpha = 1;
        for (const a of state(Store.P).access) { const ll = accessPoint(Store.sc, a); if (!ll) continue; const q = geo.project(ll), x = q.x - off.x, y = q.y - off.y; ctx.fillStyle = css('--amber'); ctx.strokeStyle = css('--ink'); ctx.lineWidth = 1.5; ctx.fillRect(x - 5, y - 5, 10, 10); ctx.strokeRect(x - 5, y - 5, 10, 10); }
      },
      legend: () => `<div class="lg-title">Affected operators</div><div class="lg-row"><span class="lg-sw" style="background:var(--accent)"></span><span>NZTA</span></div><div class="lg-row"><span class="lg-sw" style="background:var(--danger)"></span><span>Road controlling authority</span></div><div class="lg-row"><span class="lg-sw" style="background:var(--amber)"></span><span>Site access</span></div><div class="lg-note">${ASSUMPTION} thresholds</div>` });
    Registry.onInit(stampExports);
    Registry.onResults(sc => { if (Store.P) recordOutputs(Store.P); });
    const INPUT_KINDS = /^(net|demand|settings|undo|data|scenario)$/;
    const warned = new Set();
    Registry.onStore(kind => {
      const P = Store.P; if (!P) return;
      if (INPUT_KINDS.test(kind) && state(P).expert.signoffs.length && clearStale(P)) Kit.toast('Sign-off cleared: inputs changed after it. Has the opinion changed? A supplementary statement may be needed (Practice Note s9.3(f)).', 'warn');
      if (kind === 'project') for (const s of supersession(P)) Kit.toast(s.text, 'warn');
      if (/^(net|demand|undo)$/.test(kind)) for (const m of motorwayConnectors(Store.sc, P)) if (!m.confirmed && !warned.has(m.zone + m.node)) { warned.add(m.zone + m.node); Kit.toast(m.text, 'warn'); }
    });
  }
  register();

  return {
    DATA: { VERSION, SRC, REGIME, PATHWAYS, ACTIVITY, STAMP_DOCS, AUTH, E27, PRECINCTS, POLICY, NZTA_T1, LEVELS, ACCESS, SH, OPS, MATARIKI, WD, CLOCKS, DISCLAIMERS, DRAFT, STANDARDS, CHECKLIST, LAYERS, STAMP },
    // for other lanes' network LoS sections and tabs (reg-e27-extend (6)): the label to print, or '' when not tier 1
    supplementary: (P = Store.P) => { try { return e27(P).supplementary ? E27.supplementary : ''; } catch { return ''; } },
    state, blank, today, regime, stamp, recordOutputs, recheck,
    authorities, linkAuthority, shOf, operators,
    sitePoint, lookupSite, loadOverlays, loadSH, siteInfo, conflate,
    quantities, e27, itaScope, table1, depthOf, proportionality, scopingMemo, linkChange, shInvolved, accessVolume,
    accessCheck, accessGeo, evidencePack, sisd, accessType, pavement, motorwayConnectors, shCheck,
    fdsCheck, fdsPair, lint, isWorkingDay, addWorkingDays, workingDaysBetween, holidays, clocks, clockFor, rfiTemplate,
    fingerprint, signOff, signState, clearStale, dataSources, appendixG, diffParams, parseOther, runDiff, jwsCSV,
    badge, disclaimer, disclaimerHtml, csvFooter, stampCSV, missingDisclaimers, supersession, stampExports,
    checklist, checklistCSV, html, ui, onClick, onChange,
  };
})();
