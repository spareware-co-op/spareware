'use strict';
/* Spare Construct · the printable ITA report (global Report), its print mechanics, figure maps and the ITA data pack
   (owner: report lane; docs/spec report-skeleton, print-mechanics, data-pack, existing-environment, crash-history,
   pt-active-tables, mitigation-staging, safety-prediction, extent-and-difference, _report-outline; docs/regulatory
   _report-structure and reg-disclaimers).

   How it fits together
   - Every report section is a Registry.report entry, including Spare Construct's own (ids 'report-…'). Report.html(P)
     groups them into the AT ITA Guidelines 2025 v2 chapters (front matter, 1–6, appendices A–H) by their
     `chapter` (a number such as '2' or '2.7', a letter such as 'B', a chapter title or a known topic such as
     'Network performance'), sorts them by `order` and numbers them ('2.7', 'B3'). A feature replaces one of
     Spare Construct's sections by registering the same id. Unknown chapters go at the end of chapter 4; chapter
     'Appendices' goes to the appendix its title names ('Appendix A: …'), otherwise to Appendix I.
   - Numbering mirrors AT ITA Guidelines 2025 v2: Spare Construct's sections in chapters 1–6 carry fixed numbers (`at`:
     2.7 Crash history, 4.3 Road user safety …). Another feature's section becomes a sub-section of the Spare Construct
     section before it by order (4.2.3.1), or, with an order of 100 or more, a numbered section after Spare Construct's (4.8).
     A sub-section titled like its parent prints under the parent's heading; a Spare Construct placeholder (fallback: true,
     such as 4.5 Climate adaptation) hands its number to the first sub-section that prints. In the appendices a
     fallback section gives way when another feature's section in the same appendix prints something.
   - Every table and figure is numbered by chapter ('Table 2.3 (E1): Road network', 'Figure 4.1'), including other
     features' bare tables (captioned from the heading above them) and figures. Every table prints its source:
     ctx.table uses the section's provenance when none is given, and another feature's table without one gets a line
     naming the feature.
   - Every section that holds a junction table (DoS and LoS columns) gets Disclaimer D3 if it lacks one.
   - Assessment rules: with and without are compared only in the same year (pick() never falls back to a base of
     another year; the extent test and J3 refuse a gap). Economics compare the mitigation (option) with the
     with-proposal scenario (do-minimum); the development is never priced as a transport option. Crash sites and CEC
     predictions are per junction (DataHub.junctionOf), not per node. The pathway, lodgement date and regime come
     from the Compliance tab when it is set (Compliance.regime), so the report states one regime.
   - Security: the report window shares the app's origin, so every project value is escaped or coerced to a number,
     the footer string is CSS-escaped, and the document carries a CSP that runs only DOC_SCRIPT (by hash).
   - A section's render(P, ctx) returns HTML. ctx gives the report helpers: ctx.table(…) and ctx.figure(…)
     number tables and figures by chapter ('Table 4.3', 'Figure 2.1'), ctx.author(id, prompt) prints the
     author's own text (saved in P.report.text), ctx.disclaimer('D3') prints a standing disclaimer, and ctx.sel
     holds the scenarios being assessed. render(P, {}) also works: the ctx is filled in.
   - Report.html is synchronous and headless-safe. Report.open / Report.download build the report once with
     placeholders, draw the figures it asks for with Render.snapshot, then put the maps in place.
   - P.report holds the author's inputs: details, scope, sign-off, revisions, engagement log, crossings, Safe
     System likelihoods, the staging and sensitivity runs and the scenario choice.

   Deviations from the spec (all deliberate):
   - data-pack: the ZIP is written here by a small STORE-method (uncompressed) writer instead of JSZip, so the app
     keeps no runtime dependency. Every CSV starts with a UTF-8 BOM and a first row that carries Disclaimer D1
     (skip one row when reading: pandas skiprows=1); text that a spreadsheet would run as a formula gets a leading
     apostrophe (CSV injection). The combined tables (links.csv …) hold every scenario and period; by-scenario/ holds
     the same rows split per scenario and period, named with the period and year as the spec asks
     (drury-option-a-am-2036-links.csv). U.download (util.js, not this file) still writes the single CSV exports
     without a BOM. The SIDRA hand-over (corridor-sidra-volumes.csv and its SIDRA 11 loader load_corridor_volumes.py)
     is the junctions feature's own pair, byte-clean with no D1 row; the generic CSV reader is read_datapack.py.
   - print-mechanics: figure flow labels are rounded to the nearest 10 veh/h by Render.snapshot (labelRound: 10), and
     the with-minus-without difference plot compares with the chosen without scenario (baseScenarioId), whichever
     scenario is the project's base. Section 3.6 adds a development-share (%) figure when the 'devshare' mode exists.
   - one table, one place: compliance-scope, compliance-model-status, landuse-tripgen and landuse-distribution are
     printed inside Spare Construct's report-scope, report-model-status and sections 3.1 / 3.6 (MERGED), and left out as
     sections of their own. With the land-use feature, 3.1 prints its Table T1 and 3.6 its T2–T5 and without/with list
     (LandusePlus.tripTablesHtml / t5Html / wwHtml) instead of Spare Construct's own T1, T4 and T5.
   - Every page footer carries D1 (short form) and the regulatory version stamp (Compliance.stamp(P).footer); both are
     also printed in full at the end of the document for browsers without CSS page-margin boxes. While the scope
     linter has a blocking finding (ComplianceExtra.exportBlock), the report window's print and save buttons are
     disabled and a printout carries only the reason; Report.download and the data pack refuse too.
   - E27 tier 1: junction and extent tables carry Compliance.supplementary(P) above the table.
   - print-mechanics: each figure is one off-screen canvas from Render.snapshot (tiles loaded with crossOrigin
     'anonymous', scale bar, north arrow, legend, attribution), not one live L.map per figure; it is re-encoded as
     JPEG to keep the report small. Page x of y uses CSS page-margin boxes (Chrome 131 and later).
   - crash-history: the CAS rows that data.js keeps carry severity, year, location, speed limit and the pedestrian /
     cyclist / motorcyclist flags only. Deaths, serious-injury counts, light and weather are read from row positions
     7–14 when data.js keeps them (CX below) and print '–' with a note until then. The search areas are the assessed,
     altered and new junctions and the development accesses (zone connector nodes); CR1 counts every crash in the
     downloaded CAS area and says how many lie within 25 m of a modelled road.
   - report-skeleton: RR 422 Table 6.3 is carried only for the eight rows quoted in Spare Construct's research spec
     (marked verified: false); the rest of the table is not reproduced.
   - safety-prediction: CEC Table 7-3 valid ranges are known only for the priority T; the others print
     'range not recorded'. Qmaj and Qmin are the means of the two busiest and the remaining legs' two-way AADT
     (Spare Construct's reading). Midblock and rural models are not applied.
   - pt-active-tables: walk distances are 1.3 × straight line (Spare Construct assumption) until the walk network is
     loaded, and the 400 / 800 / 1,200 m catchments are drawn as circles of that distance ÷ 1.3 (layer
     'report-catchments'), not walking isochrones. PT-1 reads P.data.pt.stops, else the GTFS stops the feeds feature
     keeps (P.feeds.gtfs.stops). ANOP walking LoS bands are left to the AT ANOP tool (D5). */

const Report = (() => {
  const esc = s => U.esc(s);
  const N = (x, d = 0) => U.fmt.num(x, d);
  const PERIODS = ['AM', 'PM', 'IP'];
  const MAX_SITES = 20;

  /* ======================= standards and standing text (data, cross-cutting rule 4) ======================= */
  const deepFreeze = o => { Object.values(o).forEach(v => { if (v && typeof v === 'object') deepFreeze(v); }); return Object.freeze(o); };
  // Exact wording from docs/regulatory/_disclaimers.md. Frozen: the UI cannot remove or edit them (reg-disclaimers).
  const DISCLAIMERS = deepFreeze({
    version: '2026-10-08', versionText: '8 October 2026', source: 'docs/regulatory/_disclaimers.md',
    items: {
      D1: { title: 'Screening status', text: 'Spare Construct is a browser-based sketch and screening tool. It is not an approved regional transport model and it is not the Auckland Forecasting Centre Macro Strategic Model (MSM). Its results are indicative and are intended to scope, test and support a transport assessment. They must be reviewed, and where necessary confirmed with approved tools, by a suitably qualified transport planner or traffic engineer before they are relied on in any resource consent, plan change, notice of requirement, Fast-track application or funding decision.' },
      D2: { title: 'Expert evidence', text: 'Spare Construct is not an expert witness and does not give opinions. Any person who relies on these outputs in evidence remains responsible for complying with the Code of Conduct for Expert Witnesses (Environment Court Practice Note 2023, section 9), including stating the assumptions and data relied on, the uncertainty in the results, and appropriate sensitivity and uncertainty analyses for any model. This report is DRAFT until adopted and signed by a named, suitably qualified author.' },
      D3: { title: 'Junction results', text: 'Junction delay, degree of saturation, level of service and queue results are calculated by Spare Construct using HCM 6 / Akcelik-based methods. They are indicative and are not SIDRA output. Any junction result relied on for a consent decision, signal design (ATOC Traffic Signals Design Guidelines) or condition should be confirmed in SIDRA or another tool accepted by the road controlling authority.' },
      D4: { title: 'Economics', text: "The benefit-cost ratio is an indicative screening estimate following NZTA Monetised Benefits and Costs Manual v1.7.5 conventions. It is equivalent to an Indicative Efficiency Rating, not a funding-approval BCR. It is not MBCM-compliant unless the full procedures are applied to a model calibrated and validated to the NZTA Transport Model Development Guidelines and independently reviewed. IPM efficiency bands for 2027-30 are shown from NZTA's initial draft (August 2026) and may change after GPS 2027." },
      D5: { title: 'ANOP', text: "ANOP levels of service are indicative Spare Construct estimates. Confirm them in Auckland Transport's ANOP Assessment Tool using AT speed data before relying on them." },
      D6: { title: 'Calibration', text: "Calibration and validation statistics are reported against the NZTA Transport Model Development Guidelines (2019) for the stated model purpose category. Those criteria are not a pass/fail checklist, and meeting them does not by itself make a model fit for purpose. Fitness for purpose is a judgment for the analyst and peer reviewer and, where NZTA is involved, NZTA's modelling specialists." },
      D7: { title: 'Regional consistency', text: 'The AT ITA Guidelines 2025 v2 expect detailed modelling to be consistent with regional MSM forecasts where appropriate. Where this report states that MSM consistency was not checked, the forecasts should be reconciled with MSM or Auckland Forecasting Centre outputs before lodgement for any proposal with network-wide effects.' },
      D8: { title: 'Safety', text: "Crash histories are taken from NZTA CAS and the most recent months may be incomplete. Spare Construct's safety screening and audit briefing pack support, but do not replace, a Safe System Audit by an independent, qualified specialist, or sight distance assessment to Austroads Guide to Road Design Part 4A." },
      D9: { title: 'Speed limits', text: 'This cost benefit disclosure statement follows the structure of clause 3.3 of the Land Transport Rule: Setting of Speed Limits 2024. It is not an MBCM benefit-cost ratio and cannot be used in place of MBCM procedures for NLTF funding.' },
      D10: { title: 'Regulatory currency', text: 'Statutory references reflect the law and guidance as at [version date] for the pathway and lodgement date shown. Under the Planning Act 2026, consents lodged before 30 October 2026 are processed under the RMA as it stood immediately before 30 September 2026, and those lodged later are processed under the amended RMA as at lodgement. The specified transition date to the new planning system has not been set. This report is not legal or planning advice. Confirm the applicable provisions at lodgement.' },
      D11: { title: 'Authorities', text: 'Since 7 May 2026 Auckland Council is the road controlling authority for Auckland roads (Local Government (Auckland Council) Act 2009 s47), with local boards exercising specified powers on local and collector roads and Auckland Transport continuing as the transport CCO for public transport. AUP provisions that still refer to Auckland Transport are quoted as written. Confirm the reviewing and approving body for this application.' },
      D12: { title: 'Hand-over', text: 'This export is prepared for use in [SIDRA / SATURN / Aimsun / AT ANOP tool / NZTA IER tool / MBCM workbook / Safe System Audit / NZTA access application]. Results produced in that tool, not the Spare Construct estimates, should be reported where the tool is required or accepted by the reviewing authority.' },
      D13: { title: 'Assumed thresholds', text: 'Values marked "Spare Construct assumption" are Spare Construct defaults, not published rules. Agree them with the reviewing authority at scoping or replace them with project-specific values.' },
    },
  });
  const dText = (id, fill = {}) => {
    let t = DISCLAIMERS.items[id].text;
    if (id === 'D10') t = t.replace('[version date]', fill.version || DISCLAIMERS.versionText);
    if (id === 'D12' && fill.tools) t = t.replace(/\[[^\]]*\]/, fill.tools);
    return t;
  };
  const D1_SHORT = 'Indicative screening results from Spare Construct, a sketch model; not an approved regional model (Disclaimer D1)';

  // Attribution strings (print-mechanics)
  const ATTRIB = { osm: '© OpenStreetMap contributors (ODbL)', cas: 'Crash data: NZTA CAS (CC BY 4.0)', nslr: 'Speed limits: NZTA NSLR (CC BY 4.0)', at: 'Auckland Transport open data',
    council: 'Auckland Council open data (Unitary Plan, AGS23)', stats: 'Stats NZ (CC BY 4.0)', linz: 'LINZ (CC BY 4.0)', nzta: 'Traffic counts: NZTA (CC BY 4.0)' };

  // Data vintage rows: [loaded key, source, dataset, licence]
  const AT_LIC = () => (typeof DataHub !== 'undefined' && DataHub.AT_LICENCE) || 'Auckland Transport open data (confirm licence before publishing)';
  const VINTAGE = [
    ['osm', 'OpenStreetMap', 'Road network (Overpass API)', () => 'ODbL 1.0'],
    ['nzta', 'NZTA', 'State highway traffic monitoring sites (AADT)', () => 'CC BY 4.0'],
    ['tms', 'NZTA', 'TMS daily traffic counts', () => 'CC BY 4.0'],
    ['at', 'Auckland Transport', 'Traffic counts (TrafficService)', AT_LIC],
    ['cas', 'NZTA', 'Crash Analysis System (CAS_Data_Public)', () => 'CC BY 4.0'],
    ['nslr', 'NZTA', 'National Speed Limit Register', () => 'CC BY 4.0'],
    ['fc', 'Auckland Transport', 'Future Connect integrated network', AT_LIC],
    ['controlled', 'Auckland Transport', 'Controlled intersections', AT_LIC],
    ['aup', 'Auckland Council', 'Auckland Unitary Plan zones and overlays', () => 'Auckland Council open data'],
    ['census', 'Stats NZ', 'Census 2023 (SA1 and SA2)', () => 'CC BY 4.0'],
    ['ags23', 'Auckland Council', 'Auckland Growth Scenario 2023 (AGS23)', () => 'Auckland Council open data'],
    ['gtfs', 'Auckland Transport', 'GTFS timetable', AT_LIC],
    ['user', 'Project team', 'Your own counts (CSV)', () => 'Your data'],
  ];

  // NZTA RR 422 Table 6.1 scopes and Table 6.3 checklist (P Probable · L Likely · S Suggested · U Unlikely per scope)
  const SCOPES = {
    simple: { label: 'Simple', text: 'effects within the site and at its interface with the road network' },
    moderate: { label: 'Moderate', text: 'effects on a small area or neighbourhood' },
    broad: { label: 'Broad', text: 'effects on part or all of a suburb' },
    extensive: { label: 'Extensive', text: 'effects on a district or region' },
  };
  const SCOPE_KEYS = Object.keys(SCOPES);
  const LEVEL = { P: 'Probable', L: 'Likely', S: 'Suggested', U: 'Unlikely' };
  const hasRes = (P, pred = () => true) => P.scenarios.some(s => PERIODS.some(p => s.res && s.res[p] && pred(s.res[p], s)));
  const RR422 = {
    id: 'rr422-table-6.3', name: 'NZTA Research Report 422, Table 6.3 (content by scope)', edition: 'NZTA RR 422 Integrated transport assessment guidelines (2010)',
    url: 'https://www.nzta.govt.nz/assets/resources/research/reports/422/docs/422.pdf', verified: false,
    note: "Only the rows quoted in Spare Construct's research spec are carried; check the full Table 6.3 in RR 422.",
    rows: [
      { id: 'crash', label: 'Crash records', lv: 'LPPP', chapter: '2', done: P => !!(P.data && P.data.crashes && P.data.crashes.length), how: 'Load NZTA CAS crashes (Traffic data tab).' },
      { id: 'flows', label: 'Traffic flows', lv: 'LLLP', chapter: '2', done: P => !!(P.data && P.data.counts && P.data.counts.length), how: 'Load NZTA and AT counts, or add your own.' },
      { id: 'times', label: 'Travel times', lv: 'UUSL', chapter: 'B', done: P => !!(P.data && P.data.jt && P.data.jt.some(j => j.g)), how: 'Add journey-time checks with observed times.' },
      { id: 'congestion', label: 'Congestion', lv: 'USLP', chapter: '4', done: P => hasRes(P, r => r.nodes && Object.keys(r.nodes).length > 0), how: 'Run the model with junction controls set.' },
      { id: 'vehtrips', label: 'Vehicle trip generation, distribution and assignment', lv: 'UPPP', chapter: '3', done: P => hasRes(P, r => r.kpi && r.kpi.trips > 0), how: 'Add development zones and run the model.' },
      { id: 'persontrips', label: 'Person trips and modal split', lv: 'USLP', chapter: '3', done: P => P.scenarios.some(s => Object.values(s.zones).some(z => +z.reduction > 0 || z.persons || z.modeShare)), how: 'Set trip reductions for public transport and active modes, or load person-trip rates.' },
      { id: 'sensitivity', label: 'Sensitivity testing', lv: 'USLP', chapter: '4', done: P => !!(P.report && P.report.sens && P.report.sens.rows), how: 'Run the sensitivity pack (Report tab).' },
      { id: 'safety', label: 'Safety and environmental appraisal', lv: 'PPPP', chapter: '4', done: P => !!(P.data && P.data.crashes && P.data.crashes.length) && hasRes(P), how: 'Load crashes and run the model; complete the Safe System screening.' },
    ],
  };
  const levelOf = (row, scope) => row.lv[Math.max(0, SCOPE_KEYS.indexOf(scope))];

  /* Crash risk standards (crash-history, safety-prediction). verified: false = from Spare Construct's research spec. */
  const CEC = {
    id: 'cec-2026', name: 'NZTA Crash estimation compendium', edition: 'May 2026: Table 10-4 urban (≤ 70 km/h) "All" severity indices; Table 7-2 urban 50–70 km/h models',
    url: 'https://www.nzta.govt.nz/assets/resources/monetised-benefits-and-costs-manual/crash-risk-factors-guidelines-compendium.pdf', verified: false,
    si: { vehicle: { signals: 0.11, roundabout: 0.10, priority: 0.13, midblock: 0.19, generic: 0.15 }, ped: { intersection: 0.29, midblock: 0.31 }, cyclist: { intersection: 0.22, midblock: 0.26 }, mc: { intersection: 0.31, midblock: 0.39 } },
    speedRef: 50, speedNote: 'scaling factor to be verified',
    // A = b0 · Qmaj^b1 · Qmin^b2 (injury crashes a year; two-way AADT). range: CEC Table 7-3 valid flows where recorded.
    models: {
      uncontrolledT: { label: 'Uncontrolled T', b0: 2.08e-3, b1: 0.19, b2: 0.36, range: null },
      priorityCross: { label: 'Priority crossroads', b0: 1.13e-3, b1: 0.51, b2: 0.21, range: null },
      priorityT: { label: 'Priority T', b0: 4.68e-5, b1: 0.20, b2: 0.76, range: { maj: [5000, 26000], min: [1000, 5000] } },
      signalsCross: { label: 'Signalised crossroads', b0: 2.26e-3, b1: 0.14, b2: 0.46, range: null },
      signalsT: { label: 'Signalised T', b0: 1.21e-1, b1: 0.12, b2: 0.04, range: null },
      rbt1: { label: 'Roundabout, single-lane (per approach)', b0: 4.43e-4, b1: 0.58, perApproach: true, range: null },
      rbtMulti: { label: 'Roundabout, multi-lane (per approach)', b0: 7.95e-4, b1: 0.58, perApproach: true, range: null },
    },
  };
  const HRIG = {
    id: 'hrig-2013', name: 'NZTA High-risk Intersections Guide', edition: 'HRIG Table 4-1 (collective risk) and s4.2.2 (personal risk)',
    url: 'https://www.nzta.govt.nz/assets/resources/high-risk-intersections-guide/docs/high-risk-intersections-guide.pdf', verified: false,
    years: 5, fsHigh: 3, minInjury: 4, factor: 1.7, radius: 50,
    collective: [[1.6, 'High'], [1.1, 'Medium-high'], [0.6, 'Medium'], [0.3, 'Low-medium'], [-Infinity, 'Low']],
    personal: [[32, 'High', true], [16, 'Medium-high'], [10, 'Medium'], [6, 'Low-medium'], [-Infinity, 'Low']],
  };
  // Safe System screening matrix (Austroads AP-R509-16)
  const SSM = {
    id: 'austroads-ap-r509-16', name: 'Safe System assessment framework', edition: 'Austroads AP-R509-16 (2016)', url: 'https://austroads.gov.au/publications/road-safety/ap-r509-16', verified: false,
    label: 'Screening aid, not a Safe System Audit; scores not comparable between sites',
    cols: [['ror', 'Run-off-road'], ['headon', 'Head-on'], ['int', 'Intersection'], ['other', 'Other'], ['ped', 'Pedestrian'], ['cyc', 'Cyclist'], ['mc', 'Motorcyclist']],
    safe: { ror: 40, headon: 70, int: 50, other: 50, ped: 30, cyc: 30, mc: 30 },
    vehExposure: [1000, 5000, 10000], vruExposure: [10, 50, 100],
    severityBasis: 'Spare Construct assumption: severity 1 at or below the Safe System speed, 2 up to 20 km/h above, 3 up to 40 km/h above, otherwise 4',
  };
  // Pedestrian delay (pt-active-tables): signals d = (C − g)² / 2C; uncontrolled d = (e^(λ·tc) − λ·tc − 1) / λ, tc = L / Sp + ts
  const PED = { id: 'hcm-ped-delay', name: 'Pedestrian crossing delay', edition: 'HCM (NCHRP 26518 ch. 8) forms, as stated in Spare Construct spec', url: 'https://nap.nationalacademies.org/read/26518/chapter/8', verified: false, Sp: 1.2, ts: 3 };
  const AT_FREQUENT = 'Auckland Transport defines a frequent service as every 15 minutes or better, 7am to 7pm, seven days a week.';
  const WALK_FACTOR = { value: 1.3, basis: 'Spare Construct assumption: network walk distance ≈ 1.3 × straight-line distance until the walk network is loaded' };
  // Regulatory dates (as at 8 Oct 2026, docs/regulatory/_overview.md); switchable data, not inline logic
  const REG = {
    asAt: '2026-10-08', amendment: '2026-10-30', assent: '2026-09-30',
    pre: 'RMA as it stood immediately before 30 September 2026 (Planning Act 2026 Sch 1 cl 18, 20)',
    post: 'RMA as amended by the Planning Act 2026, as at lodgement (Sch 1 cl 19, 21)',
    ftaa: 'Fast-track Approvals Act 2024 as at lodgement (FTAA Sch 1 cl 16–24)',
    stamps: ['RMA (regime above)', 'Auckland Unitary Plan (operative text, July 2026)', 'AT ITA Guidelines 2025 Version 2', 'NZTA PPM ITA Guide G00907 (July 2026)', 'NZTA Transport Model Development Guidelines (2019)', 'NZTA Monetised Benefits and Costs Manual v1.7.5', 'Land Transport Rule: Setting of Speed Limits 2024', 'TCD Rule (amendment from 1 Nov 2026)'],
    authorities: [
      ['Auckland Council', 'Road controlling authority for Auckland roads since 7 May 2026 (LGACA s47)'],
      ['Local board', 'Specified road controlling authority powers on local and collector roads (LGACA s47C, Sch 4)'],
      ['Auckland Transport', 'Transport CCO, focused on public transport'],
      ['NZTA', 'State highways, where affected (approvals cannot be granted through Fast-track, FTAA s42(4))'],
      ['KiwiRail', 'Rail corridor and level crossings, where affected'],
    ],
  };
  const PATHWAY_LABEL = () => ({ ...(typeof PATHWAYS !== 'undefined' ? PATHWAYS : {}), funding: 'NLTF funding (business case)', other: 'Other' });
  const STANDARDS = { RR422, CEC, HRIG, SSM, PED, WALK_FACTOR, REG };

  /* ======================= small helpers ======================= */
  const R = {
    veh: v => (v == null || !isFinite(v) ? '–' : U.fmt.int(v)),                       // tables: whole veh/h
    fig: v => (v == null || !isFinite(v) ? '–' : U.fmt.int(Math.round(v / 10) * 10)),   // figures: nearest 10 veh/h
    adt: v => (v > 0 && isFinite(v) ? U.fmt.int(Math.round(v / 10) * 10) : '–'),          // ADT: nearest 10 vpd
    delay: d => (d == null || !isFinite(d) ? '–' : d > 300 ? '> 300' : N(d, 1)),          // s, 1 dp
    dos: x => (x == null || !isFinite(x) ? '–' : N(x, 2)),
    q: m => (m == null || !isFinite(m) ? '–' : U.fmt.int(m)),                             // queues: whole m
    pct: p => (p == null || !isFinite(p) ? '–' : N(p, 1) + '%'),                          // p already in %
    num: (x, d = 0) => (x == null || !isFinite(x) ? '–' : N(x, d)),
  };
  const day = t => { if (!t) return ''; try { return new Date(t).toISOString().slice(0, 10); } catch { return ''; } };
  const nowIso = () => new Date().toISOString();
  const ordinal = n => { const s = ['th', 'st', 'nd', 'rd'], v = n % 100; return n + (s[(v - 20) % 10] || s[v] || s[0]); };
  const toast = (m, k = '') => { try { if (typeof document !== 'undefined' && document.getElementById && document.getElementById('toasts')) return Kit.toast(m, k); } catch { } console.log('[report]', m); };
  const busy = t => { try { if (typeof document !== 'undefined' && document.getElementById && document.getElementById('busy')) Kit.busy(t); } catch { } };
  const resOf = (sc, p) => (sc && sc.res && sc.res[p]) || null;
  const periodsOf = sc => PERIODS.filter(p => resOf(sc, p));
  const yearOf = sc => (sc ? +Engine.yearOf(sc) || null : null);   // a number: project files are untrusted input
  const int = v => (v === '' || v == null || !isFinite(+v) ? '–' : U.fmt.int(+v));      // a number from the project, or '–' (never raw text)
  const yearTxt = v => (/^\d{4}$/.test(String(v ?? '').trim()) ? String(v).trim() : '–');  // a four-digit year, or '–'
  const plural = (n, one, many = one + 's') => `${U.fmt.int(n)} ${n === 1 ? one : many}`;
  const D = P => (P && P.data) || {};
  const loaded = P => D(P).loaded || {};
  // Run fn with Store.P = P (Store, DataHub and Engine read Store.P), then restore
  function withProject(P, fn) {
    if (!P || Store.P === P) return fn();
    const prev = Store.P; Store.P = P;
    try { return fn(); } finally { Store.P = prev; }
  }
  async function withProjectAsync(P, fn) {
    if (!P || Store.P === P) return fn();
    const prev = Store.P; Store.P = P;
    try { return await fn(); } finally { Store.P = prev; }
  }
  // The author's inputs, read without changing the project; ensure() creates the block for writing
  const RS_DEFAULT = () => ({ details: {}, scope: '', signoff: {}, revisions: [], engagement: [], text: {}, sel: {}, mitigation: {}, ssm: {}, crossings: [], ped: { Sp: PED.Sp, ts: PED.ts } });
  // Project files are untrusted: wrong types fall back to the defaults, and an unknown RR 422 scope counts as not chosen
  function clean(r) {
    const d = RS_DEFAULT();
    for (const k of ['revisions', 'engagement', 'crossings']) if (!Array.isArray(r[k])) r[k] = d[k];
    for (const k of ['details', 'signoff', 'text', 'sel', 'mitigation', 'ssm', 'ped']) if (!r[k] || typeof r[k] !== 'object' || Array.isArray(r[k])) r[k] = d[k];
    for (const k of ['revisions', 'engagement', 'crossings']) r[k] = r[k].filter(x => x && typeof x === 'object');
    if (!Object.prototype.hasOwnProperty.call(SCOPES, r.scope)) r.scope = '';
    // stored staging and sensitivity runs: lists that are not lists are read as empty
    const arrs = (o, keys) => { if (!o || typeof o !== 'object') return null; const x = { ...o }; for (const k of keys) if (x[k] != null && !Array.isArray(x[k])) x[k] = []; return x; };
    if (r.staging) { r.staging = arrs(r.staging, ['rows', 'periods', 'existing', 'existingWhy', 'steps']); if (r.staging && r.staging.trigger) r.staging.trigger = arrs(r.staging.trigger, ['sites', 'names', 'why', 'how']); }
    if (r.sens) r.sens = arrs(r.sens, ['rows', 'sentences', 'flags']);
    return r;
  }
  const rs = P => clean({ ...RS_DEFAULT(), ...((P && P.report && typeof P.report === 'object') ? P.report : {}) });
  function ensure(P = Store.P) {
    P.report = clean({ ...RS_DEFAULT(), ...(P.report && typeof P.report === 'object' ? P.report : {}) });
    return P.report;
  }
  const signedOff = P => { const s = rs(P).signoff || {}; return s.reviewed === true && String(s.reviewer || '').trim().length > 0; };
  const modelVersion = P => `Spare Construct v2 · settings v${(P.settings && P.settings.sv) || 1} · report ${DISCLAIMERS.version}`;

  /* ======================= scenarios and sites being assessed ======================= */
  // { with, without, mitigation, existing, base }: P.report.sel overrides, then scenario roles, then the base / active
  function pick(P = Store.P) {
    const scs = P.scenarios, S = rs(P).sel || {}, by = id => scs.find(s => s.id === id) || null;
    const role = r => scs.filter(s => s.role === r), yr = s => Engine.yearOf(s);
    const latest = list => list.slice().sort((a, b) => yr(b) - yr(a))[0] || null;
    const base = by(P.base) || scs[0];
    const w = by(S.with) || latest(role('proposal')) || (P.active !== P.base ? by(P.active) : null) || scs.find(s => s !== base) || base;
    // without: the author's choice, else a same-year baseline, else the base scenario only when it is the same year
    // (AT ITA s4.2.1.1 compares with and without the development in one year; a different year credits growth to it)
    const wo = by(S.without) || role('baseline').find(s => yr(s) === yr(w)) || (w !== base && yr(base) === yr(w) ? base : null);
    const m = by(S.mitigation) || role('mitigation').find(s => yr(s) === yr(w)) || null;
    const ex = by(S.existing) || role('existing')[0] || base;
    return { with: w, without: wo === w ? null : wo, mitigation: m === w ? null : m, existing: ex, base };
  }
  // The res.nodes key for a node or junction-site id
  function nodeKey(rs1, id) {
    for (const r of rs1) { if (r.nodes[id]) return id; for (const [k, nr] of Object.entries(r.nodes)) if (nr.site === id) return k; }
    return null;
  }
  function siteName(sc, res, id) {
    const nr = res && res.nodes && res.nodes[id];
    if (nr && nr.site && res.sites && res.sites[nr.site] && res.sites[nr.site].name) return res.sites[nr.site].name;
    if (sc && sc.nodes[id]) return Kit.junctionName(id, sc);
    // a junction-site id: its own name, else the name of its first node
    if (res && res.sites && res.sites[id] && res.sites[id].name) return res.sites[id].name;
    const st = sc && sc.sites && sc.sites[id];
    if (st) { if (st.name) return String(st.name); const n0 = (st.nodes || []).find(x => sc.nodes[x]); if (n0) return Kit.junctionName(n0, sc); }
    return 'Unknown junction';
  }
  /* The junctions the report assesses: the extent-of-effects sites when the test has been stored (sc.extent),
     otherwise signals, roundabouts and upgraded junctions, then the busiest give-way junctions (DoS 0.5 or more),
     up to MAX_SITES. Returns { ids (res.nodes keys), basis }. */
  const siteCache = { key: '', v: null };
  function assessedSites(P, sel = pick(P)) {
    const sc = sel.with; if (!sc) return { ids: [], basis: 'No scenario to assess.' };
    const key = `${P.id}|${Store.ver}|${sc.id}|${periodsOf(sc).map(p => resOf(sc, p).ts).join()}`;
    if (siteCache.key === key && Store.P === P) return siteCache.v;
    const rl = periodsOf(sc).map(p => resOf(sc, p));
    let out;
    if (!rl.length) out = { ids: [], basis: 'The model has not been run for the assessed scenario.' };
    else if (sc.extent && Array.isArray(sc.extent.sites) && sc.extent.sites.length) {
      const ids = [...new Set(sc.extent.sites.map(id => nodeKey(rl, id)).filter(Boolean))];
      out = { ids: ids.slice(0, 40), basis: `The ${ids.length} junctions inside the extent of effects (AT ITA Guidelines 2025 v2 s4.2.1.1).`, extent: true };
    } else {
      const score = new Map(), type = new Map();
      for (const r of rl) for (const [id, nr] of Object.entries(r.nodes)) {
        if (!nr || nr.type === 'none') continue;
        score.set(id, Math.max(score.get(id) || 0, nr.dos || 0)); type.set(id, nr.type);
      }
      const by = (a, b) => score.get(b) - score.get(a);
      // upgraded junctions always; then signals, roundabouts and give-way junctions at DoS 0.5 or more, busiest first,
      // so an over-capacity give-way junction is never crowded out by a quiet signal
      const must = [...score.keys()].filter(id => (sc.nodes[id] || {}).upgrade).sort(by);
      const rest = [...score.keys()].filter(id => !must.includes(id) && (score.get(id) >= 0.5 || /signals|roundabout/.test(type.get(id)))).sort(by);
      out = { ids: [...must, ...rest].slice(0, Math.max(MAX_SITES, must.length)),
        basis: `Spare Construct selection: upgraded junctions, then the busiest signals, roundabouts and give-way junctions (give way at DoS 0.5 or more), up to ${MAX_SITES}. Run the extent of effects test to set the assessed sites by AT ITA s4.2.1.1.` };
    }
    if (Store.P === P) { siteCache.key = key; siteCache.v = out; }
    return out;
  }
  // Summary figures of one junction result row
  function jSum(nr) {
    const sig = nr.type === 'signals' || nr.type === 'roundabout';
    const delay = sig ? (nr.avgDelay != null ? nr.avgDelay : nr.delay) : (nr.worstDelay != null ? nr.worstDelay : nr.delay);
    let q = 0, leg = '';
    for (const a of nr.appr || []) if ((a.q95 || 0) > q) { q = a.q95; leg = `${a.approach ? a.approach + ': ' : ''}${a.name || ''}`; }
    return { dos: nr.dos, delay, los: nr.los, losText: nr.losText || nr.los, basis: nr.losBasis || nr.delayBasis || '', q95: q, leg, xp: nr.xp, type: nr.type, vol: nr.vol, C: nr.C };
  }
  /* Development dwellings and GFA m² that the engine actually loads in a scenario: each zone's quantity × its
     build-out (Engine.stageOf: the zone's own z.buildout × the scenario's sc.buildout), and nothing for a zone that
     opens after the scenario year (Engine.tripEnds gives it no trips). So a staging condition's dwelling count is the
     number that produced the modelled trips, never more. */
  function devQuantities(sc) {
    let dwellings = 0, gfa = 0;
    const yr = Engine.yearOf(sc);
    for (const z of Object.values(sc.zones)) {
      if (Engine.zoneClass(z) !== 'development' || +z.year > yr) continue;
      const k0 = Engine.stageOf(z, sc);
      for (const [k, q] of Object.entries(z.lu || {})) {
        const L = LANDUSES[k]; if (!L || !(+q > 0)) continue;
        if (L.unit === 'dwellings' || k === 'retirement') dwellings += +q * k0;
        else if (/m² GFA/.test(L.unit)) gfa += +q * k0;
      }
    }
    return { dwellings, gfa };
  }

  /* ======================= calculations (pure, tested) ======================= */
  // HRIG product of flows: legs = two-way AADTs [major 1, major 2, minor 1, minor 2]; Qmin2 = 0 at a T
  function pof(legs) {
    const [a = 0, b = 0, c = 0, d = 0] = legs.map(x => +x || 0);
    return Math.pow(((a + b) / 2) * ((c + d) / 2), 0.4);
  }
  // HRIG personal risk: DSI equivalents (5 years) per 100 million vehicles through the junction
  const personalRisk = (dsi5, pf) => (pf > 0 ? dsi5 * 1e8 / (pf * HRIG.years * 365 * HRIG.factor) : null);
  function collectiveBand(dsi5, fs5 = 0) {
    if (fs5 >= HRIG.fsHigh) return 'High';
    return HRIG.collective.find(([cut]) => dsi5 >= cut)[1];
  }
  // { band, label, reliable }: High and Medium-high need 4 or more injury crashes in 5 years, otherwise 'indicative'
  function personalBand(pr, injury5) {
    if (pr == null || !isFinite(pr)) return { band: null, label: 'No volume', reliable: false };
    const row = HRIG.personal.find(([cut, , gt]) => (gt ? pr > cut : pr >= cut)), band = row[1];
    const reliable = injury5 >= HRIG.minInjury;
    return { band, label: (band === 'High' || band === 'Medium-high') && !reliable ? `${band} (indicative)` : band, reliable };
  }
  // CEC severity index of one crash at a site: vulnerable users first, then the vehicle index by control
  function severityIndex(flags = '', control = 'none', where = 'intersection') {
    const S = CEC.si, f = String(flags || '');
    if (f.includes('p')) return S.ped[where];
    if (f.includes('c')) return S.cyclist[where];
    if (f.includes('m')) return S.mc[where];
    if (where === 'midblock') return S.vehicle.midblock;
    return S.vehicle[control === 'signals' ? 'signals' : control === 'roundabout' ? 'roundabout' : control === 'priority' || control === 'stop' ? 'priority' : 'generic'];
  }
  // Crash prediction model: A = b0 · Qmaj^b1 · Qmin^b2 (injury crashes a year); flags flows outside the recorded range
  function cecPredict(modelId, qmaj, qmin = 0) {
    const M = CEC.models[modelId]; if (!M) return null;
    const A = M.perApproach ? M.b0 * Math.pow(Math.max(0, qmaj), M.b1) : M.b0 * Math.pow(Math.max(0, qmaj), M.b1) * Math.pow(Math.max(0, qmin), M.b2);
    let flag = '';
    if (M.range) {
      const out = [];
      if (qmaj < M.range.maj[0] || qmaj > M.range.maj[1]) out.push(`Qmaj ${U.fmt.int(qmaj)} outside ${U.fmt.int(M.range.maj[0])}–${U.fmt.int(M.range.maj[1])}`);
      if (qmin < M.range.min[0] || qmin > M.range.min[1]) out.push(`Qmin ${U.fmt.int(qmin)} outside ${U.fmt.int(M.range.min[0])}–${U.fmt.int(M.range.min[1])}`);
      if (out.length) flag = 'outside model range: ' + out.join('; ');
    } else flag = 'range not recorded (check CEC Table 7-3)';
    return { model: modelId, label: M.label, A, flag, outside: /outside/.test(flag) };
  }
  function cecModelFor(control, legs, multiLane) {
    if (control === 'roundabout') return multiLane ? 'rbtMulti' : 'rbt1';
    const cross = legs >= 4;
    if (control === 'signals') return cross ? 'signalsCross' : 'signalsT';
    if (control === 'priority' || control === 'stop') return cross ? 'priorityCross' : 'priorityT';
    return cross ? null : 'uncontrolledT';
  }
  // Pedestrian delay (s) at a crossing: o = { C, g, L (m), flow (veh/h, both directions), Sp, ts }
  function pedDelay(control, o = {}) {
    const Sp = +o.Sp || PED.Sp, ts = o.ts != null ? +o.ts : PED.ts;
    if (control === 'signals') { const C = +o.C, g = +o.g; return C > 0 && g >= 0 && g <= C ? (C - g) * (C - g) / (2 * C) : null; }
    if (control === 'zebra') return 0;
    const unc = (flow, L) => { const lam = (+flow || 0) / 3600, tc = (+L || 0) / Sp + ts; return lam > 0 ? (Math.exp(lam * tc) - lam * tc - 1) / lam : 0; };
    if (control === 'refuge') return unc((+o.flow || 0) / 2, (+o.L || 0) / 2) * 2;
    return unc(o.flow, o.L);
  }
  // Safe System exposure scores
  const band4 = (v, cuts) => (v == null || !isFinite(v) ? null : v < cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4);
  const sevHint = (speed, safe) => (!(speed > 0) ? null : speed <= safe ? 1 : speed <= safe + 20 ? 2 : speed <= safe + 40 ? 3 : 4);
  /* Staging trigger from build-out rows [{ pct, dwellings, gfa, fails: [site ids], crit: { id: ['dos', 'los', 'queue'] },
     dos: { id: worst DoS } }]. A site triggers at the first increment where it fails a criterion it passed at 0%
     (so a junction whose queue already exceeds its storage still triggers when it first exceeds practical DoS or
     reaches LoS E/F), or where a criterion it already failed at 0% gets materially worse (DoS up by WORSEN or more).
     Rows without crit (older stored runs) count every failure as one criterion. existing: sites failing at 0%. */
  const WORSEN = { dos: 0.05, basis: 'Spare Construct assumption: an existing deficiency that worsens by 0.05 DoS or more is a staging trigger' };
  function stagingTrigger(rows) {
    const sorted = rows.slice().sort((a, b) => a.pct - b.pct), r0 = sorted.find(r => r.pct === 0) || { fails: [] };
    const critOf = (r, id) => (r.crit && r.crit[id]) || ((r.fails || []).includes(id) ? ['fail'] : []);
    const base = new Set(r0.fails || []), dos0 = r0.dos || {};
    let lastPass = null;
    for (const r of sorted) {
      if (r === r0) { lastPass = r; continue; }
      const nf = [], how = {};
      for (const id of r.fails || []) {
        const was = critOf(r0, id), now = critOf(r, id), fresh = now.filter(k => !was.includes(k));
        const worse = was.length && r.dos && dos0[id] != null && r.dos[id] != null && r.dos[id] - dos0[id] >= WORSEN.dos - 1e-9;
        if (fresh.length || worse) { nf.push(id); how[id] = fresh.length ? (was.length ? 'new criterion failed: ' + fresh.join(', ') : 'new failure') : `existing deficiency worse by DoS ${N(r.dos[id] - dos0[id], 2)}`; }
      }
      if (nf.length) return { row: r, sites: nf, how, lastPass, existing: [...base] };
      lastPass = r;
    }
    return { row: null, sites: [], how: {}, lastPass, existing: [...base] };
  }
  function triggerText(trig, measure) {
    if (!trig || !trig.row) return '';
    const m = measure || '<the mitigation measure>', lp = trig.lastPass;
    if (trig.row.dwellings > 0) return `Prior to occupation of the ${ordinal(Math.floor((lp ? lp.dwellings : 0)) + 1)} dwelling, ${m}.`;
    if (trig.row.gfa > 0) return `Prior to occupation of more than ${U.fmt.int(lp ? lp.gfa : 0)} m² GFA, ${m}.`;
    return `Before the development passes ${lp ? lp.pct : 0}% build-out, ${m}.`;
  }
  // Tab-separated text of a table's rows (arrays of cell text)
  const tsv = rows => rows.map(r => r.map(c => String(c ?? '').replace(/[\t\r\n]+/g, ' ').trim()).join('\t')).join('\r\n');

  /* ======================= report document helpers (ctx) ======================= */
  // A table with <thead> (repeats on every printed page)
  function tbl({ head = [], rows = [], cls = '', empty = 'None.' } = {}) {
    const H = head.map(h => (typeof h === 'string' ? { label: h } : h));
    const th = H.length ? `<thead><tr>${H.map(h => `<th${h.n ? ' class="n"' : ''}${h.title ? ` title="${esc(h.title)}"` : ''}>${h.label}</th>`).join('')}</tr></thead>` : '';
    const body = rows.length ? rows.map(r => {
      const o = Array.isArray(r) ? { cells: r } : r;
      return `<tr${o.id ? ` id="${esc(o.id)}"` : ''}${o.cls ? ` class="${esc(o.cls)}"` : ''}>${o.cells.map((c, i) => `<td${H[i] && H[i].n ? ' class="n"' : ''}${o.span && i === o.cells.length - 1 ? ` colspan="${o.span}"` : ''}>${c ?? ''}</td>`).join('')}</tr>`;
    }).join('') : `<tr><td colspan="${Math.max(1, H.length)}" class="muted">${empty}</td></tr>`;
    return `<table class="rt${cls ? ' ' + esc(cls) : ''}">${th}<tbody>${body}</tbody></table>`;
  }
  const disc = (id, fill) => `<div class="disc" data-d="${id}"><b>${id} ${esc(DISCLAIMERS.items[id].title)}.</b> ${esc(dText(id, fill))}</div>`;
  const note = (t, warn) => `<p class="note${warn ? ' warn' : ''}">${t}</p>`;
  const para = t => `<p>${t}</p>`;
  // Author text: blank lines make paragraphs, lines starting '- ' make bullets (everything escaped)
  function authorHtml(text) {
    return String(text || '').trim().split(/\n\s*\n/).map(block => {
      const lines = block.split('\n');
      if (lines.every(l => /^\s*[-•]\s+/.test(l))) return `<ul>${lines.map(l => `<li>${esc(l.replace(/^\s*[-•]\s+/, ''))}</li>`).join('')}</ul>`;
      return `<p>${lines.map(esc).join('<br>')}</p>`;
    }).join('');
  }
  // One figure: the drawn map (an image data URL, or { src, attribution } | { error }) or a placeholder until it is drawn
  function figHtml(f, img, tno) {
    const src = img && (typeof img === 'string' ? img : img.src), okSrc = src && /^(data:image\/(png|jpeg|webp);base64,|blob:)/.test(src) ? src : null;
    const attr = [img && img.attribution, f.source].filter(Boolean).join(' · ') || ATTRIB.osm;
    const body = okSrc ? `<img src="${esc(okSrc)}" alt="${esc(f.title)}">`
      : `<div class="fig-ph">${img && img.error ? `This map could not be drawn: ${esc(img.error)}.` : 'Map figure. It is drawn when you open the report from Spare Construct (Report › Open the report), with scale bar, north arrow, legend and attribution.'}</div>`;
    return `<figure class="fig" id="${esc('f-' + f.id)}">${body}<figcaption><span class="tno">${tno}</span>${esc(f.title)}</figcaption><p class="src">${esc(attr)}</p></figure>`;
  }
  // Puts drawn maps into a built report body (figures keep their numbers), so Report.open builds the report once
  function fillFigures(body, requests, images) {
    const by = new Map(requests.map(f => [esc('f-' + f.id), f]));
    return body.replace(/<figure class="fig" id="([^"]+)">[\s\S]*?<\/figure>/g, (m, fid) => {
      const f = by.get(fid); if (!f || !images[f.id]) return m;
      const t = /<span class="tno">([^<]*)<\/span>/.exec(m);
      return figHtml(f, images[f.id], t ? t[1] : '');
    });
  }
  /* Provenance of a table that names no source: the section's own, with the scenarios and run dates it reads.
     → escaped HTML. Model outputs are 'estimated' (cross-cutting rule 1). */
  function runText(...scs) {
    return scs.filter(Boolean).map(s => { const ts = periodsOf(s).map(p => resOf(s, p).ts).filter(Boolean); return `${esc(s.name)} (${yearOf(s) || '–'}${ts.length ? `, run ${esc(day(Math.max(...ts)))}` : ', not run'})`; }).join('; ');
  }
  function sourceOf(c) {
    const s = c.sel || {}, id = (c.section || {}).id || '', model = (...x) => `Spare Construct model, ${runText(...x) || 'no scenario'}; estimated, not observed`;
    const user = 'Author entries in the Report tab (user)', settings = 'Project settings (settings.json in the data pack); Spare Construct\'s indicative defaults where unchanged';
    switch (id) {
      case 'report-regulatory': return 'Pathway and lodgement date: user entries (Compliance or Report tab). Regime, versions and authorities: Spare Construct\'s regulatory data as at 8 October 2026 (Planning Act 2026 Sch 1 cl 18–21; LGACA 2009 s47, s47C)';
      case 'report-doc-control': case 'report-engagement': case 'report-app-expert': return user;
      case 'report-data-vintage': return 'Spare Construct\'s download log for this project: the query date and record count kept when each dataset was loaded';
      case 'report-model-status': case 'report-modelling': case 'report-app-calibration': return `Calibrate.report against NZTA TMDG 2019 for ${runText(s.existing || s.base)}; derived from the loaded counts and the model`;
      case 'report-active': return `${esc(ATTRIB.osm)} (cycle facility tags); level of traffic stress and ANOP mapping: Spare Construct (estimated), ${runText(s.existing || s.base)}`;
      case 'report-speeds': return `${model(s.existing || s.base)}. Speed limits: NZTA NSLR where loaded, else the coded speed`;
      case 'report-crashes': return esc(ATTRIB.cas);
      case 'report-receiving': return 'Project register (user); each row\'s source and date as listed';
      case 'report-tripgen': case 'report-app-movements': case 'report-junctions': case 'report-extent': return model(s.without, s.with, s.mitigation);
      case 'report-scenarios': return 'Project scenarios (user): years, roles, land use and network as set in Scenarios';
      case 'report-walk': return `Crossings entered by the author in the Report tab (user); delay by the formulas below (${esc(PED.edition)}, estimated)`;
      case 'report-sensitivity': return `Spare Construct sensitivity pack (Engine.sensitivity), ${esc((rs(c.P).sens || {}).scName || '')} run ${esc(day((rs(c.P).sens || {}).at))}; estimated`;
      case 'report-economics': return `Engine.econ, NZTA MBCM v1.7.5 conventions (indicative screening); ${model(s.with, s.mitigation)}`;
      case 'report-mitigation': return `Network comparison of ${runText(s.mitigation) || 'the mitigation scenario'} with the proposal (Spare Construct); costs: Spare Construct indicative rates; other columns: ${user.toLowerCase()}`;
      case 'report-staging': return `Spare Construct staging test (Report tab), ${esc((rs(c.P).staging || {}).scName || '')} run ${esc(day((rs(c.P).staging || {}).at))}; estimated`;
      case 'report-app-settings': case 'report-app-rates': return settings;
      case 'report-app-zones': return 'Project zones and gateways (user; existing land use where tagged; gateway flows from counts where the count basis says so, otherwise Spare Construct estimates)';
      case 'report-app-network': return `Project network: ${esc(ATTRIB.osm)} import and user edits`;
      case 'report-app-assumptions': return 'Spare Construct\'s standards data; each item\'s source and edition as listed';
      case 'report-app-counts': return `${esc(ATTRIB.nzta)}; ${esc(ATTRIB.at)}; journey times: Google Routes typical times as stored with the project (predicted, not observed); modelled: Spare Construct (estimated)`;
      case 'report-app-datapack': return 'Spare Construct\'s data pack column dictionary';
      default: return `Spare Construct, from this project's inputs and model runs (${runText(s.with) || 'not run'})`;
    }
  }
  // 'Supplementary: outside E27.8.1(4A) matters of discretion' for an E27 tier 1 proposal, else ''
  const supText = P => { try { return typeof Compliance !== 'undefined' && typeof Compliance.supplementary === 'function' ? String(Compliance.supplementary(P) || '') : ''; } catch { return ''; } };
  // The full ctx: report helpers and the assessed scenarios. A partial ctx (the smoke test passes {}) is filled in.
  function context(P, opts = {}) {
    if (opts && opts._report) return opts;
    const sel = pick(P), S = P.settings;
    const ctx = {
      _report: true, P, S, opts, sel, period: opts.period || S.period, mapImages: opts.mapImages || {}, requests: [], esc, R, fmt: U.fmt,
      chapter: { id: '4', num: '4' }, section: { id: '', num: '' }, counters: { T: 0, F: 0 }, refs: {},
      periods: periodsOf(sel.with).length ? periodsOf(sel.with) : ['AM', 'PM'],
      sites: () => assessedSites(P, sel), disclaimer: disc, note, junctionLabel: Engine.JUNCTION_LABEL,
      // ctx.table(title, tableHtml | tbl options, { source, note, junction, d: ['D13'], id, landscape }). Every table prints
      // its source (provenance): o.source, else the section's own (sourceOf), so no table goes out without one.
      table(title, body, o = {}) {
        const t = typeof body === 'string' ? body : tbl(body), src = o.source || sourceOf(ctx);
        const jl = o.junction ? `<p class="src">${esc(Engine.JUNCTION_LABEL)}. ${o.losLabel ? esc(o.losLabel) : ''}</p>${disc('D3')}` : '';
        // E27 tier 1: junction results are outside the matters of discretion, so their tables are marked (Compliance.supplementary)
        const sup = (o.junction && o.sup !== false) || o.sup === true ? supText(P) : '';
        return `<figure class="tbl"${o.junction ? ' data-junction="1"' : ''}${o.id ? ` id="${esc(o.id)}"` : ''}><figcaption><span class="tno">${tnoToken(title)}</span>${tnoRest(title)}<button class="noprint copy" data-copy type="button">Copy table</button></figcaption>${sup ? `<p class="sup">${esc(sup)} (AUP E27 tier 1).</p>` : ''}${t}` +
          `<p class="src">Source: ${src}</p>${o.note ? `<p class="src">${o.note}</p>` : ''}${jl}${(o.d || []).map(id => disc(id)).join('')}</figure>`;
      },
      // ctx.figure({ id, title, mode, scenarioId, period, show, source }) → the drawn map, or a placeholder until Report.open draws it
      figure(f) {
        const id = String(f.id || `fig-${ctx.requests.length + 1}`);
        ctx.requests.push({ ...f, id });
        return figHtml({ ...f, id }, ctx.mapImages[id], '%%FNO%%');
      },
      // The author's own text for a box (P.report.text[id]); an empty box prints the prompt as 'to be completed'
      author(id, prompt) {
        const t = (rs(P).text || {})[id];
        return t && String(t).trim() ? `<div class="author">${authorHtml(t)}</div>` : `<div class="author empty"><b>Author to complete.</b> ${prompt}</div>`;
      },
      // ctx.merged(id): another feature's section printed inside this one (MERGED), with its tables numbered and
      // sourced as they would be on their own; '' when that feature is not installed or prints nothing
      merged(id) { return mergedHtml(P, ctx, id); },
    };
    return ctx;
  }
  /* Sections printed inside one of Spare Construct's own, so a table appears once: { merged section id: host section id }.
     The merged section is left out of the document while its host is registered (build). */
  const MERGED = Object.freeze({ 'compliance-scope': 'report-scope', 'compliance-model-status': 'report-model-status', 'landuse-tripgen': 'report-tripgen', 'landuse-distribution': 'report-tripgen' });
  function mergedHtml(P, ctx, id) {
    const d = Registry.get('report', id); if (!d) return '';
    const keep = ctx.section;
    let body = '';
    try { ctx.section = { id, num: '', title: d.title || '' }; body = Registry.html(d, 'render', P, ctx) || ''; } finally { ctx.section = keep; }
    if (!body.trim()) return '';
    if (d.title) body = body.replace(new RegExp(`^\\s*<h[1-4][^>]*>\\s*${escRe(esc(d.title))}\\s*</h[1-4]>`), '');
    return foreignBody(body, { ...d, title: MERGED_CAPTION[id] || d.title });
  }
  // Captions for a merged section's bare tables (instead of repeating the host section's title)
  const MERGED_CAPTION = Object.freeze({ 'compliance-model-status': 'Calibration and validation status', 'compliance-scope': 'Proportionality drivers' });
  /* Another feature's HTML as a report part: bare tables numbered and captioned, and a source line when it has none */
  function foreignBody(body, d) {
    let b = numberForeign(body, d.title);
    if (/<table/.test(b) && !/class="src"|\bSource:/i.test(b)) b += `<p class="src">Source: Spare Construct's ${esc(String(d.id).split('-')[0])} feature, from this project's inputs, loaded data and model runs (data vintage in the front matter; inputs in Appendix A).</p>`;
    return b;
  }
  // LandusePlus's trip-generation tables (Tables T1–T5) inside Spare Construct's sections 3.1 and 3.6: 'T2 Person trips'
  // headings become 'Table T2 · Person trips' so they are numbered 'Table 3.4 (T2): Person trips'
  const LP = () => (typeof LandusePlus !== 'undefined' && typeof LandusePlus.tripTablesHtml === 'function' ? LandusePlus : null);
  function lpTables(html, title) {
    const h = String(html || '').replace(/<h4>(T\d+b?)\s+([^<]*)<\/h4>/g, '<h4>Table $1 · $2</h4>');
    return foreignBody(h, { id: 'landuse-tripgen', title });
  }
  function lpParts(sc, parts) {
    const L = LP(); if (!L) return null;
    let h = String(L.tripTablesHtml(sc, { scope: (Store.P.landuse || {}).scope, parts }) || '');
    // an older LandusePlus without opts.parts prints every table: cut at T2
    const cut = h.indexOf('<h4>T2 ');
    if (cut > 0) { if (parts.length === 1 && parts[0] === 'T1') h = h.slice(0, cut); else if (!parts.includes('T1')) h = h.slice(cut); }
    return h;
  }

  /* ======================= chapters ======================= */
  const CHAPTERS = [
    { id: 'front', num: '', title: 'Front matter', alias: ['front', 'front matter', 'cover', 'executive summary', 'summary', 'limitations'] },
    { id: '1', num: '1', title: 'Introduction', alias: ['introduction', 'reason for consent', 'scope', 'scoping', 'statutory context', 'e27', 'e27 screening', 'triggers'] },
    { id: '2', num: '2', title: 'Transport and land use environment', alias: ['existing environment', 'environment', 'land use environment', 'road network', 'crash history', 'traffic volumes', 'speeds', 'zoning', 'receiving environment'] },
    { id: '3', num: '3', title: 'Description of proposal', alias: ['proposal', 'description of proposal', 'trip generation', 'trip distribution', 'distribution', 'assignment', 'access', 'parking', 'staging and sequencing', 'construction traffic', 'land use'] },
    { id: '4', num: '4', title: 'Transport assessment', alias: ['assessment', 'transport assessment', 'network performance', 'modelling', 'junctions', 'junction performance', 'safety', 'road user safety', 'sensitivity', 'economics', 'network alignment', 'policy alignment', 'climate', 'climate adaptation', 'state highway', 'state highway effects', 'anop', 'anop fit', 'freight'] },
    { id: '5', num: '5', title: 'Mitigation, conditions and monitoring', alias: ['mitigation', 'conditions', 'monitoring', 'staging triggers', 'affected parties', 'notification'] },
    { id: '6', num: '6', title: 'Conclusion', alias: ['conclusion'] },
    { id: 'P', num: 'P', app: true, title: 'Pathway annexes', alias: ['pathway', 'pathway annexes', 'annex', 'plan change', 'notice of requirement', 'fast-track', 'funding', 'speed limit', 'cbds', 'growth funding', 'time-of-use'] },
    { id: 'A', num: 'A', app: true, title: 'Model inputs and standards register', alias: ['appendix a', 'model inputs', 'inputs', 'settings', 'standards register'] },
    { id: 'B', num: 'B', app: true, title: 'Calibration and validation', alias: ['appendix b', 'calibration', 'validation', 'calibration and validation'] },
    { id: 'C', num: 'C', app: true, title: 'Full movement tables', alias: ['appendix c', 'movement tables', 'movements', 'sidra tables'] },
    { id: 'D', num: 'D', app: true, title: 'Crash listing', alias: ['appendix d', 'crash listing'] },
    { id: 'E', num: 'E', app: true, title: 'ANOP tool inputs', alias: ['appendix e', 'anop inputs', 'anop tool inputs'] },
    { id: 'F', num: 'F', app: true, title: 'Data pack manifest', alias: ['appendix f', 'data pack', 'data pack manifest'] },
    { id: 'G', num: 'G', app: true, title: 'Expert disclosure and sign-off', alias: ['appendix g', 'expert disclosure', 'expert'] },
    { id: 'H', num: 'H', app: true, title: 'Statutory timeline', alias: ['appendix h', 'statutory timeline', 'timeline', 'rfi clock'] },
    { id: 'I', num: 'I', app: true, title: 'Regulatory checklist and other material', alias: ['appendix i', 'appendices', 'regulatory checklist', 'other material'] },
  ];
  /* The chapter id of a section. Chapter 'Appendices' with a title such as 'Appendix A: Standards register' goes to
     that appendix; without a letter it goes to Appendix I. */
  function chapterOf(name, title) {
    const s = String(name || '').trim().toLowerCase();
    if (!s) return '4';
    const t = /^appendix\s+([a-h])\b/i.exec(String(title || '').trim());
    if (t && /^appendi(x|ces)\b/.test(s)) return t[1].toUpperCase();
    let m = /^(?:chapter\s+)?(\d)(?:[.\s]|$)/.exec(s);
    if (m) return +m[1] >= 1 && +m[1] <= 6 ? m[1] : +m[1] === 7 ? '6' : '4';
    m = /^appendix\s+([a-h])\b/.exec(s) || /^([a-hp])(\d+)?$/.exec(s);
    if (m) return m[1].toUpperCase();
    const ch = CHAPTERS.find(c => c.title.toLowerCase() === s || c.alias.includes(s)) || CHAPTERS.find(c => c.alias.some(a => s.includes(a) && a.length > 4));
    return ch ? ch.id : '4';
  }

  /* ======================= Spare Construct's own sections ======================= */
  const S_ = [];   // { id, chapter, order, at, title, tag, landscape, check(P) → true | false | null, render(P, ctx) }
  const AT_NUM = new Map();   // id → AT ITA Guidelines 2025 v2 number in chapters 1–6 (the report outline), from s.at
  const sec = d => S_.push(d);

  /* The pathway, lodgement date and regime: one source of truth. The Compliance tab's context (Compliance.regime)
     wins when it is set; the Report tab's own fields are the fallback when the compliance feature is absent or unset.
     → { from, pathway, label, date, regime } */
  function regCtx(P) {
    const cx = P && P.compliance && P.compliance.ctx;
    if (typeof Compliance !== 'undefined' && Compliance.regime && cx && (cx.pathway || cx.lodgement)) {
      try {
        const r = Compliance.regime(P);
        return { from: 'compliance', pathway: r.pathway || '', label: r.pathway ? r.pathwayLabel : '', date: r.date || '', regime: r.text, short: r.short };
      } catch { /* fall back to the Report tab's fields */ }
    }
    const d = rs(P).details || {}, pw = d.pathway || (P.assessment && P.assessment.type) || '';
    const regime = pw === 'ftaa' ? REG.ftaa : d.lodgement ? (d.lodgement < REG.amendment ? REG.pre : REG.post) : 'Lodgement date not stated: the regime cannot be set (Planning Act 2026 Sch 1 cl 18–21).';
    return { from: 'report', pathway: pw, label: pw ? PATHWAY_LABEL()[pw] || pw : '', date: d.lodgement || '', regime };
  }

  /* ---------- front matter ---------- */
  sec({ id: 'report-cover', chapter: 'front', order: 1, tag: 'mixed', check: P => !!(rs(P).details.author && rs(P).details.site), render(P, c) {
    const R0 = rs(P), d = R0.details || {}, sel = c.sel, rg = regCtx(P), pw = rg.label || 'Pathway not stated';
    const row = (k, v) => `<tr><th>${k}</th><td>${v || '<span class="muted">Not stated</span>'}</td></tr>`;
    const stale = P.scenarios.some(s => PERIODS.some(p => resOf(s, p) && resOf(s, p).stale));
    return `<div class="cover"><div class="kicker">Integrated Transport Assessment</div><h1 class="title">${esc(P.name)}</h1>
      <div class="subtitle">${esc(d.site || 'Site address not stated')}</div>
      <table class="meta">${row('Proposal type', esc(pw))}${row('Client', esc(d.client))}${row('Author', esc(d.author))}${row('Reviewer', esc((R0.signoff || {}).reviewer))}
        ${row('Report date', esc(d.date || day(nowIso())))}${row('Lodgement date', esc(rg.date))}${row('Assessed scenario', esc(sel.with ? `${sel.with.name} (${yearOf(sel.with)})` : ''))}${row('Model version', esc(modelVersion(P)))}</table>
      ${signedOff(P) ? `<p class="stamp ok">Reviewed by ${esc(R0.signoff.reviewer)}${R0.signoff.date ? ', ' + esc(R0.signoff.date) : ''}.</p>` : '<p class="stamp draft">DRAFT: not yet reviewed and signed off.</p>'}
      ${stale ? note('Some results are out of date: the network or demand changed after the last run. Run the model again before issuing this report.', true) : ''}
      ${disc('D1')}</div>`;
  } });
  sec({ id: 'report-contents', chapter: 'front', order: 2, title: 'Contents', tag: 'auto', numbered: false, render: () => '%%CONTENTS%%' });
  sec({ id: 'report-regulatory', chapter: 'front', order: 3, title: 'Regulatory context', tag: 'auto', check: P => { const r = regCtx(P); return !!(r.pathway && r.date); }, render(P, c) {
    const rg = regCtx(P);
    return `${c.table('Pathway, regime and version stamp', { head: ['Item', 'Value'], rows: [
      ['Pathway', esc(rg.label || 'Not stated')], ['Lodgement date', esc(rg.date || 'Not stated')], ['Regime', esc(rg.regime)],
      ['Versions relied on', REG.stamps.map(esc).join('<br>')], ['Law and guidance as at', esc(REG.asAt)]] },
      { note: rg.from === 'compliance' ? 'Pathway, lodgement date and regime from the Compliance tab.' : 'Pathway and lodgement date from the Report tab. Set them in the Compliance tab when it is installed, so every section uses one regime.' })}
      ${c.table('Authorities', { head: ['Body', 'Role'], rows: REG.authorities.map(([a, b]) => [esc(a), esc(b)]) })}
      ${disc('D10', { version: REG.asAt })}${disc('D11')}`;
  } });
  sec({ id: 'report-doc-control', chapter: 'front', order: 4, title: 'Document control', tag: 'author', check: P => rs(P).revisions.length > 0, render(P, c) {
    const rows = (rs(P).revisions || []).map(r => [esc(r.rev), esc(r.date), esc(r.desc), esc(r.prepared), esc(r.reviewed), esc(r.approved)]);
    return c.table('Document control', { head: ['Rev', 'Date', 'Description', 'Prepared', 'Reviewed', 'Approved'], rows, empty: 'No revisions recorded. Add them in the Report tab.' });
  } });
  function vintageRows(P) {
    const L = loaded(P), rows = [], seen = new Set(), fv0 = (P.feeds && P.feeds.vintage) || {}, FEED = { ags23: 'ags' };
    const anyOsm = P.scenarios.some(s => Object.values(s.links).some(l => l.osm));
    for (const [k, src, ds, lic] of VINTAGE) {
      seen.add(k);
      const x = L[k];
      if (!x && fv0[FEED[k] || k]) continue;   // loaded by the feeds feature: its own row follows
      if (k === 'osm' && !x) { rows.push([src, ds, anyOsm ? 'Imported (query date not recorded)' : 'Not used', anyOsm ? `${plural(P.scenarios[0] ? Object.values(P.scenarios[0].links).filter(l => l.osm).length : 0, 'link')}` : '–', lic()]); continue; }
      rows.push([src, ds, x && x.at ? day(x.at) : 'Not loaded', x ? (x.n != null ? U.fmt.int(x.n) : '–') + (k === 'cas' && x.label ? ` (${x.label})` : '') : '–', x && x.licence ? x.licence : lic()]);
    }
    for (const [k, x] of Object.entries(L)) if (!seen.has(k) && x && typeof x === 'object') rows.push([x.source || k, x.dataset || x.label || k, x.at ? day(x.at) : '–', x.n != null ? int(x.n) : '–', x.licence || 'See source']);
    // the official feeds (P.feeds.vintage: NSLR, TMS, GTFS, AGS23 …) and the land-use sources (P.landuse.sources) keep their own logs
    const fv = (P.feeds && P.feeds.vintage) || {};
    for (const [k, x] of Object.entries(fv)) if (x && typeof x === 'object')
      rows.push([String(x.source || k), String(x.note || x.label || k) + (x.error ? ` (failed: ${x.error})` : ''), x.at ? day(x.at) || '–' : '–', x.n != null ? int(x.n) : '–', String(x.licence || 'See source')]);
    const ls = (P.landuse && P.landuse.sources) || {};
    for (const [k, x] of Object.entries(ls)) if (x && typeof x === 'object')
      rows.push([String(x.name || k), String(x.vintage || k), x.at ? day(x.at) || '–' : '–', x.n != null ? int(x.n) : '–', String(x.licence || 'See source')]);
    return rows;
  }
  sec({ id: 'report-data-vintage', chapter: 'front', order: 5, title: 'Data sources and vintage', tag: 'auto', check: P => Object.keys(loaded(P)).length > 0, render(P, c) {
    return c.table('Data vintage', { head: ['Source', 'Dataset', 'Query date', 'Records', 'Licence'], rows: vintageRows(P).map(r => r.map(esc)) },
      { note: 'Every number in this report is tagged with its basis where it is shown: measured, derived, estimated, assumed or user-entered.' });
  } });
  // One model-status table: the compliance feature's (category, MSM check, D6, D7) when it is installed, otherwise Spare Construct's
  sec({ id: 'report-model-status', chapter: 'front', order: 6, title: 'Model purpose and status', tag: 'mixed', check: P => !!String((P.settings.tmdg || {}).justification || '').trim(), render(P, c) {
    const tm = P.settings.tmdg || {}, cm = c.merged('compliance-model-status');
    if (cm) return cm;
    const rows = c.periods.map(p => { const s = Engine.modelStatus(P, p); return [p, esc(s.label), esc(s.date ? day(s.date) : '–')]; });
    return `${para(`Model purpose: NZTA TMDG category <b>${esc(String(tm.category || 'E').toUpperCase())}</b>. ${tm.justification ? 'Justification: ' + esc(tm.justification) : '<span class="warn">The justification for this category has not been given (Settings › Calibration and validation › Why this category).</span>'}`)}
      ${c.table('Calibration and validation status', { head: ['Period', 'Status', 'Date'], rows })}${note(esc(Engine.STANDARDS.tmdg.standing))}`;
  } });
  /* One scope and proportionality section: the RR 422 scope, then the compliance feature's ITA triggers, proportionality
     statement, drivers and scoping memo when it is installed (that statement replaces the author box; the author's own
     text still prints when written). */
  sec({ id: 'report-scope', chapter: 'front', order: 7, title: 'Scope and proportionality', tag: 'mixed', check: P => !!rs(P).scope, render(P, c) {
    const s = rs(P).scope, S0 = SCOPES[s], cs = c.merged('compliance-scope'), own = String((rs(P).text || {}).scope || '').trim();
    return `${para(S0 ? `Scope (NZTA RR 422 Table 6.1): <b>${S0.label}</b>, ${esc(S0.text)}.` : '<span class="warn">The RR 422 scope (Simple, Moderate, Broad or Extensive) has not been chosen (Report tab).</span>')}
      ${cs}${cs && !own ? '' : c.author('scope', 'State why this depth of assessment is enough for the decision (RMA s88(2AA)–(2AB), Sch 4 cl 2(3)(c); from 30 Oct 2026 s18B; FTAA s44): net peak trips, the largest % change on a link, crash flags and any state highway involvement.')}`;
  } });
  sec({ id: 'report-limitations', chapter: 'front', order: 8, title: 'Limitations', tag: 'mixed', render(P, c) {
    return `${para('Spare Construct is a sketch model. Junction results are HCM 6 / Akcelik-based and indicative, not SIDRA output.')}${disc('D1')}${disc('D2')}${disc('D13')}
      ${c.author('limitations', 'Add any project-specific limitations (data gaps, untested periods, tools still to be run).')}`;
  } });
  sec({ id: 'report-exec', chapter: 'front', order: 9, title: 'Executive summary', tag: 'mixed', check: P => !!String((rs(P).text || {}).exec || '').trim(), render(P, c) {
    return `<ul class="bul">${execBullets(P, c).map(b => `<li>${b}</li>`).join('')}</ul>${c.author('exec', 'Summarise the pathway, the triggers, the key effects, the mitigation and its triggers, and whether the proposal aligns with the transport network and policy.')}`;
  } });
  function execBullets(P, c) {
    const out = [], sel = c.sel, rg = regCtx(P), w = sel.with;
    if (rg.pathway) out.push(`Pathway: ${esc(rg.label)}${rg.date ? `, lodgement ${esc(rg.date)}` : ''}. Regime: ${esc(rg.regime)}.`);
    if (w) {
      const per = periodsOf(w).map(p => { const r = resOf(w, p); return r && r.dev ? `${R.veh(r.dev.trips)} veh/h in the ${p} peak hour` : null; }).filter(Boolean);
      if (per.length) out.push(`The development generates about ${per.join(' and ')} (vehicle trips, ${yearOf(w)}, ${esc(w.name)}).`);
      const js = junctionRows(P, c, w), worst = js.filter(r => r.s).sort((a, b) => b.s.dos - a.s.dos)[0];
      const nj = junctionCount(P, c, [...new Set(js.map(r => r.id))]);
      if (worst) out.push(`${plural(nj.junctions, 'junction')} assessed${nj.junctions !== nj.nodes ? ` (${plural(nj.nodes, 'modelled node')})` : ''}; the highest degree of saturation is ${esc(worst.name)} in the ${worst.p} peak (DoS ${R.dos(worst.s.dos)}, LoS ${esc(worst.s.losText)}). Indicative, not SIDRA output.`);
    }
    const ex = extentData(P, c);
    if (ex.length) out.push(`Extent of effects (AT ITA s4.2.1.1): ${ex.map(e => `${e.p} peak ${plural(e.x.rows.filter(r => r.included).length, 'link direction')} and ${plural(e.x.sites.length, 'junction')} over the 5%/10% thresholds`).join('; ')}.`);
    const cr1 = crashYears(P);
    if (cr1) out.push(`Crash history ${cr1.from}–${cr1.to}: ${plural(cr1.total.n, 'reported crash', 'reported crashes')} in the downloaded CAS area, ${cr1.total.F + cr1.total.S} fatal or serious (DSI).`);
    const conv = P.scenarios.flatMap(s => periodsOf(s).map(p => resOf(s, p))).filter(r => r.converged === false).length;
    out.push(`Model status: ${c.periods.map(p => `${p} ${esc(Engine.modelStatus(P, p).short)}`).join(', ')}; ${conv ? `<span class="warn">${plural(conv, 'run')} did not converge</span>` : 'every stored run converged'}.`);
    const st = rs(P).staging;
    if (st && st.text) out.push(esc(st.text));
    return out;
  }

  // Junctions (not nodes) among node ids: a roundabout of four nodes counts once
  function junctionCount(P, c, ids) {
    const sc = c.sel.existing || c.sel.base, scw = c.sel.with || sc;
    return { nodes: ids.length, junctions: junctionGroups(P, sc, scw, ids).length };
  }

  /* ---------- 1 Introduction ---------- */
  sec({ id: 'report-purpose', chapter: '1', order: 10, at: '1.1', title: 'Purpose and statutory context', tag: 'author', check: P => !!String(rs(P).text.purpose || '').trim(), render: (P, c) =>
    c.author('purpose', 'Describe the proposal, who it is for, the consent or approval sought and the statutory tests it answers (for example RMA s88(2)(b) and Sch 4; s168(3A) for a notice of requirement; FTAA Sch 5 cl 5–7).') });
  sec({ id: 'report-reason', chapter: '1', order: 20, at: '1.2', title: 'Reason for consent', tag: 'mixed', check: P => !!String(rs(P).text.reason || '').trim(), render: (P, c) =>
    `${c.author('reason', 'List the AUP E27 triggers (Table E27.6.1.1 thresholds, tier and exemptions), the access, parking and loading infringements and the activity status.')}${note('The E27 screening table is added here by the compliance feature when it is installed. Never read an E27.6.1 exceedance by itself as requiring an ITA.')}` });
  sec({ id: 'report-engagement', chapter: '1', order: 30, at: '1.3', title: 'Scoping and engagement log', tag: 'author', check: P => rs(P).engagement.length > 0, render(P, c) {
    return c.table('Scoping and engagement log', { head: ['Date', 'Party', 'Topic', 'Outcome'], rows: (rs(P).engagement || []).map(r => [esc(r.date), esc(r.party), esc(r.topic), esc(r.outcome)]), empty: 'No engagement recorded yet. Add meetings and agreements in the Report tab.' });
  } });
  sec({ id: 'report-rr422', chapter: '1', order: 40, at: '1.4', title: 'ITA scope checklist', tag: 'auto', check: P => !!rs(P).scope, render(P, c) {
    const scope = rs(P).scope;
    const rows = RR422.rows.map(r => {
      const lv = SCOPE_KEYS.map(k => { const l = levelOf(r, k); return `<span class="lv lv-${l}${k === scope ? ' on' : ''}">${LEVEL[l]}</span>`; });
      const ok = r.done(P), l = scope ? levelOf(r, scope) : null;
      return { cells: [esc(r.label), ...lv, ok ? '<span class="ok">Included</span>' : l === 'P' ? `<span class="bad">Missing (Probable)</span> ${esc(r.how)}` : `<span class="muted">Not included</span> ${esc(r.how)}`], cls: !ok && l === 'P' ? 'miss' : '' };
    });
    return `${scope ? para(`Scope chosen: <b>${SCOPES[scope].label}</b>. The chosen scope's column is outlined.`) : note('Choose the scope in the Report tab to check the content against it.', true)}
      ${c.table('RR 422 Table 6.3 content checklist', { head: ['Item', ...SCOPE_KEYS.map(k => SCOPES[k].label), 'Status'], rows }, { source: `${esc(RR422.edition)}. ${esc(RR422.note)}` })}`;
  } });

  /* ---------- 2 Transport and land use environment ---------- */
  const studyBounds = P => {
    const a = P.studyArea || (P.report && P.report.bounds);
    if (a && Array.isArray(a.bounds)) return a.bounds;
    if (Array.isArray(a)) return a;
    if (a && a.south != null) return [[a.south, a.west], [a.north, a.east]];
    const sc = pick(P).with || P.scenarios[0], pts = Object.values(sc.zones).filter(z => z.kind !== 'gateway').map(z => [z.lat, z.lng]);
    if (!pts.length) return null;
    let s = 90, w = 180, n = -90, e = -180;
    for (const [la, ln] of pts) { s = Math.min(s, la); n = Math.max(n, la); w = Math.min(w, ln); e = Math.max(e, ln); }
    const pad = 0.006;   // about 600 m
    return [[s - pad, w - pad * 1.25], [n + pad, e + pad * 1.25]];
  };
  sec({ id: 'report-locality', chapter: '2', order: 10, at: '2.1', title: 'Site and locality', tag: 'mixed', check: P => !!String(rs(P).text.locality || '').trim(), render(P, c) {
    const sc = c.sel.existing || c.sel.base;
    return `${c.author('locality', 'Describe the site, its surroundings and the study area.')}
      ${c.figure({ id: 'locality', title: 'Locality and study area', mode: 'class', scenarioId: sc.id, bounds: studyBounds(P), show: { counts: false, crashes: false, connectors: false } })}`;
  } });
  sec({ id: 'report-zoning', chapter: '2', order: 20, at: '2.2', title: 'Zoning', tag: 'mixed', render: (P, c) =>
    `${c.author('zoning', 'Describe the operative AUP zoning (and any proposed zoning, such as PC120) of the site and its surroundings.')}${note('The AUP zone map is added by the land-use feature when Auckland Unitary Plan zones are loaded.')}` });
  // Named roads of a scenario, grouped (arterial and above, plus collectors)
  function roadGroups(sc) {
    const g = new Map();
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed' || !L.name) continue;
      const C = CLASSES[L.cls] || CLASSES.local; if (C.rank < 3) continue;
      const k = L.name + '|' + L.cls;
      if (!g.has(k)) g.set(k, { name: L.name, cls: L.cls, links: [], km: 0 });
      const x = g.get(k); x.links.push(L); x.km += Net.len(sc, L) / 1000;
    }
    return [...g.values()].sort((a, b) => (CLASSES[b.cls].rank - CLASSES[a.cls].rank) || b.km - a.km);
  }
  const fcText = (L, dec) => { const f = L && L.fc; if (!f) return '–'; const k = dec ? '10' : ''; return [f['gt' + k], f['pt' + k], f['freight' + k], f['cycle' + k]].map(v => esc(v || '–')).join(' / '); };
  /* The legal speed limit of a link: the feeds lane writes L.speedLimit and L.nslr.v with L.speedSource 'NSLR'
     (feeds.js); older projects may hold L.nslr.limit or a bare number. Otherwise the coded (modelled) speed. */
  const speedOf = L => {
    const n = L.nslr, lim = +L.speedLimit > 0 ? +L.speedLimit : n && typeof n === 'object' ? +(n.v || n.limit) : +n;
    if (lim > 0 && (L.speedSource === 'NSLR' || n)) return { v: lim, src: 'NSLR' };
    return { v: +L.speed || null, src: L.speedSource && L.speedSource !== 'NSLR' ? `${L.speedSource} (not NSLR)` : L.speedSrc || 'coded (not NSLR)' };
  };
  /* AUP arterial overlay and vehicle access restriction per link, from the compliance lane's AUP controls
     (P.compliance.net.aupArterial and P.compliance.overlays.var, matched to links with Compliance.conflate).
     { art(L) → true | false | null, vr(L) → type | '' | null, loaded } — null when the layer is not loaded. */
  const g0nslr = groups => groups.some(g => g.links.some(l => speedOf(l).src === 'NSLR'));
  const aupCache = { key: '', v: null };
  function aupOf(P, sc) {
    const C = P.compliance || {}, net = C.net || {}, ov = C.overlays || null, art = net.aupArterial || {};
    const loaded = !!(ov && (ov.sources || []).length) || Object.keys(art).length > 0;
    const key = `${P.id}|${sc.id}|${Store.ver}|${ov ? ov.at : ''}|${Object.keys(art).length}`;
    if (aupCache.key === key && aupCache.v) return aupCache.v;
    let vmap = {};
    if (ov && (ov.var || []).length && typeof Compliance !== 'undefined' && Compliance.conflate) {
      try { vmap = Compliance.conflate(sc, ov.var.map(v => ({ tag: v.type, pts: v.pts }))); } catch { vmap = {}; }
    }
    const v = { loaded,
      art: L => (art[L.id] || L.aupArterial === true || L.aup === true ? true : loaded || L.aupArterial === false ? false : null),
      vr: L => vmap[L.id] || L.var || L.vehicleAccessRestriction || (loaded ? '' : null) };
    aupCache.key = key; aupCache.v = v;
    return v;
  }
  sec({ id: 'report-roads', chapter: '2', order: 30, at: '2.3', title: 'Road network', tag: 'auto', render(P, c) {
    const sc = c.sel.existing || c.sel.base, groups = roadGroups(sc), A = aupOf(P, sc);
    const rows = groups.slice(0, 40).map(g => {
      const L = g.links.find(l => l.fc) || g.links[0], sp = [...new Set(g.links.map(l => speedOf(l)).map(s => `${s.v != null ? s.v : '–'} (${s.src})`))].join(', ');
      const lanes = Math.max(...g.links.map(l => Math.max(l.lanesAB | 0, l.lanesBA | 0)));
      const ar = g.links.map(A.art), aup = ar.includes(true) ? 'Yes' : ar.includes(false) ? 'No' : '–';
      const vr = [...new Set(g.links.map(A.vr).filter(Boolean))];
      return [esc(g.name), esc(CLASSES[g.cls].label), R.num(g.km, 2), fcText(L), fcText(L, true), aup, esc(vr.length ? vr.join(', ') : A.loaded ? 'None' : '–'), esc(sp), lanes];
    });
    return `${c.table('Table E1 · Road network', { head: ['Road', 'Class', { label: 'Length km', n: true }, 'Future Connect GT / PT / freight / cycle (current)', 'Future Connect (first decade)', 'AUP arterial', 'Vehicle access restriction', 'Speed limit km/h (source)', { label: 'Lanes per direction', n: true }], rows, empty: 'No named arterial or collector roads in this scenario.' },
      { source: `${ATTRIB.osm}${loaded(P).fc ? '; Future Connect: ' + esc(AT_LIC()) : ''}${loaded(P).nslr || g0nslr(groups) ? '; ' + ATTRIB.nslr : ''}${A.loaded ? '; AUP arterial and vehicle access restriction controls: ' + esc(ATTRIB.council) : ''}`, note: `${groups.length > 40 ? `The 40 longest of ${groups.length} roads are shown. ` : ''}Local roads are left out. '–' means the layer is not loaded for this study area (AUP controls: Compliance tab).` })}
      ${loaded(P).fc ? '' : note('Load AT Future Connect (Traffic data) to fill the Future Connect columns; outside Auckland, tag classes by hand.')}`;
  } });
  /* PT stops: P.data.stops or P.data.pt.stops = [{ id, code, name, mode, lat, lng, routes[], busesAM, fcPt }], else the
     GTFS stops the feeds feature keeps (P.feeds.gtfs.stops = [{ id, code, name, lat, lng, routes[], am, pm }], buses/h).
     → { list, src } */
  function stopsOf(P) {
    const own = (D(P).pt && D(P).pt.stops) || D(P).stops;
    if (Array.isArray(own) && own.length) return { list: own, src: esc(ATTRIB.at), text: ATTRIB.at };
    const g = P && P.feeds && P.feeds.gtfs;
    if (g && Array.isArray(g.stops) && g.stops.length) {
      const who = g.publisher || (g.feed && g.feed.provider) || 'the operator', dt = /^\d{8}$/.test(String(g.date || '')) ? `${g.date.slice(0, 4)}-${g.date.slice(4, 6)}-${g.date.slice(6, 8)}` : '';
      const text = `GTFS timetable from ${who}${dt ? `, service day ${dt}` : ''} (${(g.feed && g.feed.licence) || 'the publisher\'s licence'}), loaded ${day(g.at) || 'date not recorded'}`;
      return { list: g.stops.map(s => ({ ...s, busesAM: s.busesAM != null ? s.busesAM : s.am })), src: esc(text), text };
    }
    return { list: [], src: '', text: '' };
  }
  // Future Connect PT class of the nearest link (within 50 m) that has one
  function fcPtNear(sc, s) {
    let best = null, bd = 50;
    for (const L of Object.values(sc.links)) {
      if (!L.fc || !L.fc.pt || L.status === 'closed') continue;
      for (const q of Net.geom(sc, L)) { const d = U.dist(q, [+s.lat, +s.lng]); if (d < bd) { bd = d; best = L.fc.pt; } }
    }
    return best;
  }
  const siteCentre = sc => { const z = Object.values(sc.zones).filter(z => Engine.zoneClass(z) === 'development'); if (!z.length) return null; return [z.reduce((s, x) => s + x.lat, 0) / z.length, z.reduce((s, x) => s + x.lng, 0) / z.length]; };
  const anopCycle = (L, aadt) => L.cycle === 'separated' ? 'B' : L.cycle === 'painted' ? 'E' : (+L.speed <= 30 && !(aadt > 3000)) ? 'D' : +L.speed <= 50 ? 'E' : 'F';
  const AT_FACILITY = { separated: 'Protected cycle lane', painted: 'On-road cycle lane', none: 'Mixed traffic' };
  const CATCH = [400, 800, 1200];   // m of network walk; drawn as straight-line radius ÷ WALK_FACTOR (Spare Construct assumption)
  sec({ id: 'report-active', chapter: '2', order: 40, at: '2.4', title: 'Walking, cycling and public transport', tag: 'mixed', render(P, c) {
    const sc = c.sel.existing || c.sel.base, rb = resOf(sc, 'AM') || resOf(sc, 'PM'), ctr = siteCentre(c.sel.with || sc), st = stopsOf(P), stops = st.list;
    let pt;
    if (stops.length && ctr) {
      const rows = stops.map(s => ({ s, d: U.dist(ctr, [+s.lat, +s.lng]) * WALK_FACTOR.value })).filter(x => x.d <= 1200).sort((a, b) => a.d - b.d).slice(0, 30)
        .map(({ s, d }) => [esc(s.name), esc(s.code || s.id), esc(s.mode || 'Bus'), R.q(d), R.num(d / 80, 1), esc((Array.isArray(s.routes) ? s.routes : []).join(', ')), s.busesAM != null && s.busesAM !== '' ? R.num(+s.busesAM, 1) : '–', esc(s.fcPt || fcPtNear(sc, s) || '–'), d <= 400 ? '400 m' : d <= 800 ? '800 m' : '1,200 m']);
      pt = c.table('Table PT-1 · Public transport stops near the site', { head: ['Stop or station', 'Code', 'Mode', { label: 'Walk m', n: true }, { label: 'Walk min', n: true }, 'Routes', { label: 'Buses/h AM', n: true }, 'Future Connect PT', 'Within'], rows, empty: 'No stops within 1,200 m.' },
        { source: st.src, note: `${esc(WALK_FACTOR.basis)}. Walk time at 80 m/min. ${esc(AT_FREQUENT)}`, d: ['D13'] });
    } else pt = note(`Public transport stops are not loaded for this study area, so Table PT-1 is not filled. Load the bus timetable (GTFS) or AT bus and train stops (data feeds). ${esc(AT_FREQUENT)}`);
    const cy = roadGroups(sc).slice(0, 30).map(g => {
      const L = g.links[0], a = rb && rb.arcs[L.id + ':ab'], b = rb && rb.arcs[L.id + ':ba'], aadt = rb ? ((a ? a.v : 0) + (b ? b.v : 0)) * (+P.settings.peakToDaily || 10) : null;
      const lts = Engine.lts(L, rb);
      return [esc(g.name), esc(AT_FACILITY[L.cycle] || AT_FACILITY.none), lts ? `LTS ${int(lts)}` : 'Not allowed', anopCycle(L, aadt)];
    });
    const figs = ctr ? c.figure({ id: 'pt-catchments', title: `Public transport stops and the ${CATCH.map(m => U.fmt.int(m)).join(' / ')} m walking catchments of the site`, mode: 'class', scenarioId: sc.id, bounds: studyBounds(P), show: { counts: false, crashes: false, 'report-catchments': true, 'feeds-bus-stops': true },
      source: `${st.text ? st.text + '. ' : ''}Catchments: ${WALK_FACTOR.basis}` }) : '';
    return `${pt}${figs}${c.table('Table CY-1 · Cycling facilities and level of traffic stress', { head: ['Section', 'AT facility type', 'Spare Construct LTS', 'ANOP cycle LoS (indicative)'], rows: cy, empty: 'No named roads.' },
      { note: 'ANOP cycle LoS from the indicative mapping in Spare Construct spec (off-road cycleway A; protected lane B; shared path B/C; trail or shared zone C; LATM D; on-road lane E; mixed traffic D/E/F by speed and volume).', d: ['D5'] })}
      ${c.figure({ id: 'cycle-lts', title: `Cycle network: level of traffic stress, ${sc.name}`, mode: 'lts', scenarioId: sc.id, period: rb ? rb.period : undefined, bounds: studyBounds(P), show: { counts: false, crashes: false } })}
      ${c.author('active', 'Describe walking and cycling routes, crossings and public transport services serving the site.')}`;
  } });
  // E2 traffic volumes from every loaded count, and the peak-hour statement derived from them
  function countRows(P) {
    const cs = (D(P).counts || []).filter(c => !c.off);
    return cs.map(c => {
      const t = c.peak > 0 && c.peakTime && c.peakTime !== '00:00' ? c.peakTime : null, am = t && +t.slice(0, 2) < 12, h = c.hist && c.hist.filter(v => v > 0);
      const growth = h && h.length >= 2 ? (h[h.length - 1] - h[0]) / h[0] / (h.length - 1) * 100 : null;
      const dur = c.dur || (c.src === 'nzta' ? (c.virtual ? 'NZTA estimate' : /Continuous/i.test(c.note || '') ? 'Continuous' : 'Periodic') : '');
      return { c, t, am, growth, dur, oneDay: /^\s*(1\s*d|24\s*h|1\s*day)/i.test(String(dur)) };
    });
  }
  function peakStatement(P) {
    const rows = countRows(P), hours = rows.filter(r => r.t), all = rows.length;
    if (!all) return 'No traffic counts are loaded, so no peak hour can be derived. The model uses one-hour AM and PM peaks.';
    const years = rows.map(r => r.c.date ? +r.c.date.slice(0, 4) : null).filter(Boolean), yr = years.length ? `${Math.min(...years)}–${Math.max(...years)}` : 'dates not recorded';
    const mode = list => { const m = new Map(); for (const r of list) m.set(r.t, (m.get(r.t) || 0) + 1); return [...m].sort((a, b) => b[1] - a[1])[0]; };
    const hh = t => { const h = +t.slice(0, 2); return `${t}–${String(h + 1).padStart(2, '0')}:${t.slice(3)}`; };
    const am = mode(hours.filter(r => r.am)), pm = mode(hours.filter(r => !r.am));
    const src = [...new Set(rows.map(r => DataHub.SOURCES[r.c.src] ? DataHub.SOURCES[r.c.src].short : r.c.src))].join(' and ');
    if (!am && !pm) return `None of the ${plural(all, 'loaded count')} (${esc(src)}, ${yr}) reports a peak hour, so the model's one-hour AM and PM peaks are assumed.`;
    const part = (m, n, w) => m ? `${w} peak hour ${hh(m[0])} (the busiest hour at ${m[1]} of ${n} counts with a ${w} peak)` : `no ${w} peak hour reported`;
    return `Weekday ${part(am, hours.filter(r => r.am).length, 'AM')} and ${part(pm, hours.filter(r => !r.am).length, 'PM')}, derived from ${plural(hours.length, 'count')} of ${all} ${esc(src)} counts that report a peak hour (${yr}).`;
  }
  sec({ id: 'report-volumes', chapter: '2', order: 50, at: '2.5', title: 'Traffic volumes', tag: 'auto', check: P => (D(P).counts || []).length > 0, render(P, c) {
    const rows = countRows(P).map(({ c: k, t, am, growth, dur, oneDay }) => {
      const src = DataHub.SOURCES[k.src] ? DataHub.SOURCES[k.src].short : k.src;
      return [esc(k.road || ''), esc(k.name), `${esc(src)} ${esc(k.ref || '')}${k.virtual ? ' (estimate)' : ''}`, esc(k.date || (k.src === 'nzta' ? `Latest NZTA AADT year${(loaded(P).nzta || {}).at ? ` (loaded ${day(loaded(P).nzta.at)})` : ''}` : '–')), `${esc(dur || '–')}${oneDay ? ' <b class="warn" title="One day only">24H</b>' : ''}`, R.adt(k.adt),
        esc(k.dir ? `${k.dir} only` : k.directional ? '–' : 'two-way'), k.hv != null ? R.pct(+k.hv) : '–', am && t ? `${esc(t)}, ${R.veh(k.peak)}` : '–', !am && t ? `${esc(t)}, ${R.veh(k.peak)}` : '–',
        t && k.adt ? R.pct(k.peak / k.adt * 100) : '–', growth != null ? R.pct(growth) : '–'];
    });
    return `${para(esc(peakStatement(P)))}
      ${c.table('Table E2 · Traffic volumes', { head: ['Road', 'Location', 'Source / site ref', 'Count date', 'Duration', { label: 'ADT/AADT vpd', n: true }, 'Direction', { label: 'HV %', n: true }, 'AM peak start, veh/h', 'PM peak start, veh/h', { label: 'Peak % of daily', n: true }, { label: 'Linear growth %/yr', n: true }], rows, empty: 'No counts loaded. Load NZTA and AT counts in the Traffic data tab.' },
        { source: `${ATTRIB.nzta}; ${esc(ATTRIB.at)}`, note: 'ADT to the nearest 10 vpd. Growth is linear, from the NZTA five-year AADT history where available. 24H flags a one-day count.' })}
      ${c.figure({ id: 'counts', title: 'Traffic count sites', mode: 'class', scenarioId: (c.sel.existing || c.sel.base).id, bounds: studyBounds(P), show: { counts: true, crashes: false }, source: `${ATTRIB.nzta}; ${ATTRIB.at}` })}`;
  } });
  // Junction table rows for a scenario (assessed sites × periods)
  function junctionRows(P, c, sc, ids) {
    const rl = periodsOf(sc), out = [];
    if (!rl.length) return out;
    const list = ids || (sc === c.sel.with ? assessedSites(P, c.sel).ids : null) || [];
    let use = list;
    if (!ids && sc !== c.sel.with) {
      // the same junctions as the assessed scenario where they exist, otherwise its own busiest controlled junctions
      const own = assessedSites(P, { ...c.sel, with: sc }).ids;
      use = list.length ? list.filter(id => rl.some(p => resOf(sc, p).nodes[id])) : own;
      if (!use.length) use = own;
    }
    for (const id of use) for (const p of rl) {
      const r = resOf(sc, p), nr = r.nodes[id];
      out.push({ id, p, name: siteName(sc, r, id), s: nr ? jSum(nr) : null, nr, res: r });
    }
    return out;
  }
  function j2Table(P, c, sc, title) {
    const rows = junctionRows(P, c, sc), pers = periodsOf(sc), ids = [...new Set(rows.map(r => r.id))];
    if (!rows.length) return note(`${esc(sc.name)} has no junction results. Run the model.`);
    const refs = c.refs[sc.id] || (c.refs[sc.id] = {});
    let k = Object.keys(refs).length;
    const body = ids.map(id => {
      refs[id] = refs[id] || `J${++k}`;
      const first = rows.find(r => r.id === id), ctl = first.s ? CONTROLS[first.s.type] || first.s.type : '–';
      const cells = [refs[id], esc(first.name), esc(ctl)];
      for (const p of pers) {
        const r = rows.find(x => x.id === id && x.p === p), s = r && r.s;
        cells.push(s ? `${R.dos(s.dos)}${s.dos > (s.xp || 1) ? ' <b class="bad">!</b>' : ''}` : '–', s ? R.delay(s.delay) : '–', s ? esc(s.losText) : '–', s ? `${R.q(s.q95)}${s.leg ? ` <span class="muted">(${esc(s.leg)})</span>` : ''}` : '–');
      }
      return { cells, id: `j-${sc.id}-${id}` };
    });
    const head = ['Ref', 'Junction', 'Control', ...pers.flatMap(p => [{ label: `${p} worst DoS`, n: true }, { label: `${p} delay s`, n: true }, `${p} LoS`, `${p} longest Q95 m (leg)`])];
    const r0 = resOf(sc, pers[0]);
    return c.table(`${title} · ${esc(sc.name)} (${yearOf(sc)})`, { head, rows: body }, { junction: true, losLabel: r0 && r0.junction ? r0.junction.label : '',
      note: `Delay: volume-weighted average for signals and roundabouts, worst movement for give-way and stop. ! = over the practical DoS (signals 0.90, roundabouts 0.85, give-way 0.80). Queues are whole metres. Sites: ${esc(assessedSites(P, c.sel).basis)}` });
  }
  sec({ id: 'report-speeds', chapter: '2', order: 60, at: '2.6', title: 'Speeds and existing performance', tag: 'auto', landscape: true, render(P, c) {
    const sc = c.sel.existing || c.sel.base, pers = periodsOf(sc);
    // one section per road and speed limit, so '% of limit' is always against that section's own limit
    const secs = [];
    for (const g of roadGroups(sc).slice(0, 30)) {
      const by = new Map();
      for (const L of g.links) { const s = speedOf(L), k = `${s.v}|${s.src}`; if (!by.has(k)) by.set(k, { lim: s, links: [], km: 0 }); const x = by.get(k); x.links.push(L); x.km += Net.len(sc, L) / 1000; }
      const parts = [...by.values()].sort((a, b) => b.km - a.km);
      for (const x of parts) secs.push({ name: parts.length > 1 ? `${g.name} (${R.num(x.km, 2)} km at ${x.lim.v != null ? x.lim.v : '?'} km/h)` : g.name, ...x });
    }
    const rows = secs.slice(0, 40).map(g => {
      const lim = g.lim, mod = pers.map(p => { const r = resOf(sc, p); let s = 0, n = 0; for (const L of g.links) for (const d of ['ab', 'ba']) { const a = r.arcs[L.id + ':' + d]; if (a && a.v > 0) { s += a.spd * a.v; n += a.v; } } return n ? s / n : null; });
      const obs = g.links.map(l => l.obsSpeed).find(v => v > 0);
      return [esc(g.name), `${lim.v != null ? R.num(lim.v) : '–'} (${esc(lim.src)})`, ...mod.map(v => R.num(v)), ...mod.map(v => v != null && lim.v > 0 ? R.pct(v / lim.v * 100) : '–'), obs ? R.num(+obs) : '–', '–', '–'];
    });
    return `${c.table('Table E3 · Speeds', { head: ['Section', 'Limit km/h (source)', ...pers.map(p => ({ label: `Modelled ${p} km/h`, n: true })), ...pers.map(p => ({ label: `${p} % of limit`, n: true })), { label: 'Observed mean km/h', n: true }, 'ANOP LoS', '85th percentile'], rows, empty: 'No results for the existing scenario. Run it.' },
        { note: 'Modelled speeds are flow-weighted means from the existing scenario (estimated, not observed). Observed speeds and 85th percentiles come from surveys or AT speed data when loaded; ANOP LoS from the AT ANOP tool.', d: ['D5'] })}
      ${pers.length ? c.figure({ id: `speed-${sc.id}-${pers[0]}`, title: `Modelled speed as a share of the ${((P.settings.display || {}).speedBands || 'anop') === 'hcm' ? 'free-flow speed' : 'speed limit'}, ${sc.name}, ${pers[0]} peak hour ${yearOf(sc)} (indicative)`, mode: 'speed', scenarioId: sc.id, period: pers[0], bounds: studyBounds(P), show: { counts: false, crashes: false }, source: Object.values(sc.links).some(l => speedOf(l).src === 'NSLR') ? ATTRIB.nslr : 'Speed limits: coded speeds (NSLR not loaded)' }) : ''}
      ${j2Table(P, c, sc, 'Table J2 · Existing junction performance')}`;
  } });
  // Crash history: CR1 rows over the five full years plus the year to date
  /* A CAS row as data.js keeps it: [lat, lng, severity F|S|M|N, year, location, flags p/c/m, speed limit]. The fields
     crash-history asks for may follow (read only when present, '–' otherwise): [7] fatalCount, [8] seriousInjuryCount,
     [9] minorInjuryCount, [10] light, [11] weatherA, [12] OBJECTID, [13] trafficControl, [14] urban. */
  const CX = { fatal: 7, serious: 8, minor: 9, light: 10, weather: 11, id: 12, control: 13, urban: 14 };
  const cnum = (k, i) => (Array.isArray(k) && k[i] !== '' && k[i] != null && isFinite(+k[i]) ? +k[i] : null);
  const ctext = (k, i) => (Array.isArray(k) && k[i] != null && typeof k[i] !== 'object' ? String(k[i]) : '');
  const DARK = /dark/i, WET = /rain|snow|hail|sleet|wet/i;
  function crashYears(P) {
    const cr = D(P).crashes || []; if (!cr.length) return null;
    const cas = loaded(P).cas || {}, ys = cr.map(c => +c[3]).filter(y => y > 0 && y < 3000), yr = v => (+v > 1900 && +v < 3000 ? Math.floor(+v) : null);
    if (!ys.length && !yr(cas.from)) return null;
    const from = yr(cas.from) || Math.min(...ys), to = yr(cas.to) || Math.max(...ys) - (cas.ytd ? 0 : 1), ytd = yr(cas.ytd) || to + 1;
    const t = () => ({ n: 0, F: 0, S: 0, M: 0, N: 0, p: 0, c: 0, m: 0, deaths: 0, si: 0, dark: 0, wet: 0 }), years = {}, total = t(), y = t(), has = { deaths: false, si: false, light: false, weather: false };
    for (let k = from; k <= to; k++) years[k] = t();
    for (const c of cr) {
      const yy = +c[3], tgt = yy >= from && yy <= to ? years[yy] : yy === ytd ? y : null; if (!tgt || !'FSMN'.includes(c[2]) || String(c[2]).length !== 1) continue;
      const df = cnum(c, CX.fatal), ds = cnum(c, CX.serious), lt = ctext(c, CX.light), wt = ctext(c, CX.weather);
      if (df != null) has.deaths = true; if (ds != null) has.si = true; if (lt) has.light = true; if (wt) has.weather = true;
      for (const o of yy === ytd ? [tgt] : [tgt, total]) {
        o.n++; o[c[2]]++; for (const f of 'pcm') if (String(c[5] || '').includes(f) && 'FSM'.includes(c[2])) o[f]++;
        o.deaths += df || 0; o.si += ds || 0; if (DARK.test(lt)) o.dark++; if (WET.test(wt)) o.wet++;
      }
    }
    return { from, to, ytd, years, total, y, has, lagYear: yr(cas.lagYear), label: cas.label || `${from}–${to} plus ${ytd} to date` };
  }
  // How many crashes DataHub assigns to a modelled road (within 25 m), or null
  const onRoads = (P, c) => { try { const s = withProject(P, () => DataHub.safetySummary(c.sel.existing || c.sel.base)); return s ? s.onLinks : null; } catch { return null; } };
  // CR2: crashes within 50 m of each assessed, altered or new site, over the five full years
  /* Ten-year crash history at the development accesses (NZTA asks for ten years for an access on a state highway):
     D().crashesLong from DataHub.loadAll(['cas10']), within 50 m of each access site. The five-year tables above are
     unchanged. → a table, or a note saying how to load it */
  function accessLong(P, c) {
    const acc = crashSites(P, c).filter(s => s.access && s.access.length);
    if (!acc.length) return '';
    const long = D(P).crashesLong, L = loaded(P).casLong;
    if (!Array.isArray(long) || !L) return note('For an access on a state highway, NZTA asks for ten years of crash history. Load it in the Traffic data tab (Crashes › Ten years) to add it here; the five-year tables are not changed.');
    const sc = c.sel.with || c.sel.base, sc0 = c.sel.existing || c.sel.base;
    const rows = acc.map(s => {
      const home = sc.nodes[s.id] ? sc : sc0, pts = (s.nodes || [s.id]).filter(x => home.nodes[x]).map(x => [home.nodes[x].lat, home.nodes[x].lng]);
      const t = { F: 0, S: 0, M: 0, N: 0 };
      for (const k of long) { if (!pts.length) break; const d = Math.min(...pts.map(q => U.dist(q, [k[0], k[1]]))); if (d <= HRIG.radius && t[k[2]] != null) t[k[2]]++; }
      return [esc(s.name), int(t.F), int(t.S), int(t.M), int(t.N), int(t.F + t.S + t.M)];
    });
    return c.table(`Table CR4 · Ten-year crash history at development accesses (50 m radius${L.label ? ', ' + esc(L.label) : ''})`, { head: ['Access', { label: 'F', n: true }, { label: 'S', n: true }, { label: 'M', n: true }, { label: 'N', n: true }, { label: 'Injury', n: true }], rows },
      { source: `${esc(ATTRIB.cas)}; ten-year download of ${esc(day(L.at))}`, note: 'For state highway access applications (NZTA). The latest months may be incomplete (D8).' });
  }
  function crashSites(P, c) {
    if (c._crashSites && c._crashSites.P === P) return c._crashSites.v;   // once per report build (2.7, 4.3 and the tab use it)
    const v = crashSites0(P, c);
    c._crashSites = { P, v };
    return v;
  }
  function crashSites0(P, c) {
    const cy = crashYears(P); if (!cy) return [];
    const sc = c.sel.existing || c.sel.base, scw = c.sel.with || sc, cr = D(P).crashes;
    const ids = new Set(assessedSites(P, c.sel).ids);
    for (const n of Object.values(scw.nodes)) if (n.upgrade) ids.add(n.id);
    for (const L of Object.values(scw.links)) if (L.status === 'proposed') for (const e of [L.a, L.b]) if (sc.nodes[e]) ids.add(e);
    // and every development access: the road nodes the development zones connect to (crash-history search areas)
    const acc = accessesOf(P, scw);
    for (const id of acc.keys()) if (scw.nodes[id] || sc.nodes[id]) ids.add(id);
    const lr = withProject(P, () => DataHub.linkRisk(sc)) || {};
    const out = [];
    for (const g of junctionGroups(P, sc, scw, [...ids]).slice(0, 30)) {
      const id = g.id, home = scw.nodes[id] ? scw : sc, n = home.nodes[id]; if (!n) continue;
      const members = g.nodes.filter(x => home.nodes[x]), pts = members.map(x => [home.nodes[x].lat, home.nodes[x].lng]);
      const t = { F: 0, S: 0, M: 0, N: 0, injury: 0, p: 0, c: 0, m: 0, dsiEq: 0, list: [] };
      for (const k of cr) {
        if (!(k[3] >= cy.from && k[3] <= cy.to)) continue;
        // within 50 m of any node of the junction, counted once
        const d = Math.min(...pts.map(q => U.dist(q, [k[0], k[1]]))); if (d > HRIG.radius) continue;
        t[k[2]]++; t.list.push({ k, d });
        if ('FSM'.includes(k[2])) { t.injury++; t.dsiEq += severityIndex(k[5], n.control, 'intersection'); for (const f of 'pcm') if (String(k[5] || '').includes(f)) t[f]++; }
      }
      const lg = externalLegs(home, members), links = lg.flatMap(x => x.links);
      const legs = lg.map(x => { const v = x.links.map(l => (lr[l.id] && lr[l.id].aadt > 0 ? lr[l.id].aadt : null)); const s = v.filter(a => a != null);
        return { id: x.links[0].id, name: x.name, aadt: s.length ? s.reduce((a, b) => a + b, 0) : null, src: x.links.map(l => (lr[l.id] ? lr[l.id].aadtSrc : null)).find(Boolean) || null }; }).sort((a, b) => (b.aadt || 0) - (a.aadt || 0));
      const q = legs.map(l => l.aadt || 0), pf = legs.length >= 3 && q[0] > 0 ? pof([q[0], q[1], q[2], legs.length >= 4 ? q[3] : 0]) : null;
      const pr = pf ? personalRisk(t.dsiEq, pf) : null, pb = personalBand(pr, t.injury), speed = Math.max(0, ...links.map(l => +speedOf(l).v || 0));
      const zs = [...new Set(g.ids.flatMap(x => acc.get(x) || []))];
      out.push({ id, ids: g.ids, nodes: members, name: (g.name || Kit.junctionName(id, home)) + (zs.length ? ` (access to ${zs.join(', ')})` : ''), access: zs, control: n.control, speed, ...t, legs, pof: pf, pr, pband: pb, collective: collectiveBand(t.dsiEq, t.F + t.S),
        estimated: legs.some(l => l.src && /model/.test(l.src)), speedFlag: speed && speed !== CEC.speedRef ? (speed > 70 ? 'urban indices used above 70 km/h; check CEC rural indices' : CEC.speedNote) : '' });
    }
    // two sites with one name (a road with unnamed side roads) are told apart by their position
    const n0 = new Map(); for (const s of out) n0.set(s.name, (n0.get(s.name) || 0) + 1);
    for (const s of out) if (n0.get(s.name) > 1) { const nd = (scw.nodes[s.id] || sc.nodes[s.id]); if (nd) s.name += ` (${R.num(+nd.lat, 4)}, ${R.num(+nd.lng, 4)})`; }
    return out.sort((a, b) => b.dsiEq - a.dsiEq || b.injury - a.injury);
  }
  // Development accesses of a scenario: Map(road node id → [zone names]) from the zone connectors (kept per project version)
  const accCache = { key: '', v: null };
  function accessesOf(P, sc) {
    const key = `${P.id}|${Store.ver}|${sc.id}`;
    if (accCache.key === key && Store.P === P) return accCache.v;
    const out = new Map();
    try {
      const dz = Object.values(sc.zones).filter(z => z.kind !== 'gateway' && Engine.zoneClass(z) === 'development');
      const G = dz.length ? Engine.buildGraph(sc) : null;
      for (const z of dz) for (const id of (G && G.conns && G.conns[z.id]) || (Array.isArray(z.connectors) ? z.connectors : [])) if (sc.nodes[id]) out.set(id, [...(out.get(id) || []), String(z.name || z.id)]);
    } catch { /* no accesses: the assessed junctions are still searched */ }
    if (Store.P === P) { accCache.key = key; accCache.v = out; }
    return out;
  }
  /* Groups node ids into junctions: a roundabout or divided junction of several nodes is one site (DataHub.junctionOf
     on the reference scenario sc, which needs crashes loaded), else the junction site of the result (nr.site), else
     the node alone. → [{ id (first assessed node), ids (assessed nodes in it), nodes (every member node), name }] */
  function junctionGroups(P, sc, scw, ids) {
    const nr = withProject(P, () => (D(P).crashes || []).length ? DataHub.nodeRisk(sc) : null) || {};
    const rl = periodsOf(scw).map(p => resOf(scw, p)), out = [], by = new Map();
    for (const id of ids) {
      let key = null, nodes = [id], name = '';
      const j = sc.nodes[id] ? withProject(P, () => DataHub.junctionOf(sc, id)) : null;
      if (j != null && nr[j]) { key = 'j:' + j; nodes = nr[j].nodes; name = nr[j].name; }
      else { const site = rl.map(r => r.nodes[id] && r.nodes[id].site).find(Boolean); if (site) { key = 's:' + site; const r = rl.find(x => x.sites && x.sites[site]); name = r ? r.sites[site].name || '' : ''; nodes = Object.keys((rl[0] || { nodes: {} }).nodes).filter(k => rl.some(r => r.nodes[k] && r.nodes[k].site === site)); } }
      key = key || 'n:' + id;
      if (by.has(key)) { by.get(key).ids.push(id); continue; }
      const g = { id, ids: [id], nodes: [...new Set([id, ...nodes])], name: name && name !== 'Unnamed junction' ? name : '' };
      by.set(key, g); out.push(g);
    }
    return out;
  }
  /* The legs of a junction made of one or more nodes: the links that leave it (circulating ring links inside it are
     left out), with the two one-way carriageways of one road on one side merged into one leg (as DataHub does).
     → [{ links: [link], name }] */
  function externalLegs(sc, members) {
    const inside = new Set(members), seen = new Set(), legs = [];
    const ns = members.map(id => sc.nodes[id]).filter(Boolean), ctr = [ns.reduce((s, n) => s + n.lat, 0) / ns.length, ns.reduce((s, n) => s + n.lng, 0) / ns.length];
    const oneWay = L => !((L.lanesAB | 0) > 0 && (L.lanesBA | 0) > 0);
    for (const id of members) for (const L of Net.linksAt(sc, id)) {
      if (L.status === 'closed' || seen.has(L.id) || (inside.has(L.a) && inside.has(L.b))) continue;
      seen.add(L.id);
      const far = sc.nodes[inside.has(L.a) ? L.b : L.a], b = far ? U.bearing(ctr, [far.lat, far.lng]) : 0, nm = String(L.name || '').trim().toLowerCase();
      const leg = legs.find(g => nm && g.nm === nm && oneWay(L) && g.links.every(oneWay) && U.angDiff(g.b, b) < 45);
      if (leg) leg.links.push(L); else legs.push({ links: [L], name: L.name || '', nm, b });
    }
    return legs.map(({ links, name }) => ({ links, name }));
  }
  sec({ id: 'report-crashes', chapter: '2', order: 70, at: '2.7', title: 'Crash history', tag: 'mixed', landscape: true, check: P => (D(P).crashes || []).length > 0, render(P, c) {
    const cy = crashYears(P);
    if (!cy) return `${note('No crash data is loaded. Load NZTA CAS crashes in the Traffic data tab: AT ITA Guidelines s4.3.2 expect five full years.', true)}${disc('D8')}`;
    const yrRow = (lab, t, cls) => ({ cells: [lab, t.F, t.S, t.M, t.N, t.n, cy.has.deaths ? t.deaths : '–', cy.has.si ? t.si : '–', t.F + t.S], cls });
    const cr1 = [...Object.entries(cy.years).map(([y, t]) => yrRow(`${y}${+y === cy.lagYear ? ' (may be incomplete)' : ''}`, t)), yrRow(`<b>${cy.from}–${cy.to} total</b>`, cy.total, 'tot'), yrRow(`${cy.ytd} to date (YTD, incomplete)`, cy.y, 'ytd')];
    const sites = crashSites(P, c);
    const cr2 = sites.map(s => [esc(s.name), esc(CONTROLS[s.control] || s.control), s.speed || '–', s.F, s.S, s.M, s.N, s.injury, s.p, s.c, s.m, `${R.num(s.dsiEq, 2)}${s.speedFlag ? ` <span class="muted" title="${esc(s.speedFlag)}">*</span>` : ''}`, esc(s.collective),
      esc(s.legs.map(l => l.aadt ? R.adt(l.aadt) : '–').join(' / ')) + (s.estimated ? ' <span class="muted">(estimated)</span>' : ''), s.pof ? R.num(s.pof) : '–', s.pr != null ? R.num(s.pr, 1) : '–', esc(s.pband.label), s.pband.reliable ? 'Yes' : 'No (fewer than 4 injury crashes)']);
    return `${para(`Crash history from NZTA CAS for ${esc(cy.label)}. The tables use the five full years ${cy.from}–${cy.to}; the year to date is shown apart and is incomplete.`)}
      ${c.table(`Table CR1 · Crashes by year and severity, ${cy.from}–${cy.to} plus YTD`, { head: ['Year', { label: 'Fatal', n: true }, { label: 'Serious', n: true }, { label: 'Minor', n: true }, { label: 'Non-injury', n: true }, { label: 'Total', n: true }, { label: 'Deaths', n: true }, { label: 'Serious injuries', n: true }, { label: 'DSI crashes', n: true }], rows: cr1 },
        { source: esc(ATTRIB.cas), note: `Crash counts by the most severe injury. ${cy.has.deaths && cy.has.si ? 'Deaths and serious injuries are the people killed and seriously injured (CAS fatalCount and seriousInjuryCount).' : 'Deaths and serious-injury counts are not in the CAS fields Spare Construct keeps, so they show –.'} Every crash in the downloaded CAS area is counted${onRoads(P, c) != null ? `; ${plural(onRoads(P, c), 'crash', 'crashes')} of all years are within 25 m of a modelled road` : ''}.` })}
      ${c.table('Table CR2 · Crash history at assessed, altered and new sites and development accesses (50 m radius, five full years)', { head: ['Site', 'Control', { label: 'Speed km/h', n: true }, { label: 'F', n: true }, { label: 'S', n: true }, { label: 'M', n: true }, { label: 'N', n: true }, { label: 'Injury', n: true }, { label: 'Ped', n: true }, { label: 'Cyclist', n: true }, { label: 'Motorcyclist', n: true },
        { label: 'DSI equiv (5 yr)', n: true }, 'Collective band', 'Leg AADTs vpd', { label: 'PoF', n: true }, { label: 'Personal risk DSI/100M veh', n: true }, 'Personal band', 'Reliable'], rows: cr2, empty: 'No assessed sites. Run the model for the assessed scenario.' },
        { source: `${esc(ATTRIB.cas)}; ${esc(CEC.name)} (${esc(CEC.edition)}); ${esc(HRIG.name)} (${esc(HRIG.edition)})`, note: `DSI equivalents = injury crashes × CEC Table 10-4 urban severity indices (vehicle by control, pedestrian, cyclist and motorcyclist; movement codes are not public). * speed is not 50 km/h: ${esc(CEC.speedNote)}. Collective band (HRIG Table 4-1): High ≥ 1.6 or ≥ 3 fatal and serious crashes. Personal risk = DSI·10⁸ / (PoF·5·365·1.7); High and Medium-high need 4 or more injury crashes, otherwise indicative. Leg AADTs from counts, else modelled peak × ${esc(P.settings.peakToDaily)} (estimated). Standards verified: no.` })}
      ${c.table('Table CR3 · Crash conditions (five full years)', { head: [{ label: 'Dark (all crashes)', n: true }, { label: 'Wet weather (all crashes)', n: true }, { label: 'Pedestrian injury crashes', n: true }, { label: 'Cyclist injury crashes', n: true }, { label: 'Motorcyclist injury crashes', n: true }], rows: [[cy.has.light ? cy.total.dark : '–', cy.has.weather ? cy.total.wet : '–', cy.total.p, cy.total.c, cy.total.m]] },
        { note: cy.has.light && cy.has.weather ? 'Dark: CAS light is dark. Wet weather: CAS weatherA is rain, snow, hail or sleet (Spare Construct reading of "wet").' : 'Light and weather are not in the CAS fields Spare Construct keeps yet, so Dark and Wet show –.' })}
      ${accessLong(P, c)}
      ${c.figure({ id: 'crashes', title: 'Reported crashes by severity, with the 50 m search areas of the assessed sites', mode: 'class', scenarioId: (c.sel.existing || c.sel.base).id, bounds: studyBounds(P), show: { crashes: true, counts: false, 'report-sites': true }, source: ATTRIB.cas })}
      ${c.author('crash-narrative', 'AT ITA 4.3.3: explain why the system failed at the main crash sites, whether less severe crashes could become severe, and whether the proposal makes the pattern worse.')}${disc('D8')}`;
  } });
  sec({ id: 'report-receiving', chapter: '2', order: 80, at: '2.8', title: 'Future receiving environment', tag: 'auto', check: P => (P.register || []).length > 0 || !!String(rs(P).text.receiving || '').trim(), render(P, c) {
    const rows = (typeof Register !== 'undefined' ? Register.table(P) : []).map(r => [esc(r.name), esc(r.type), esc(r.status || '–'), esc(r.funding || '–'), esc(r.source), esc(r.asAt), r.included ? 'Yes' : 'No', esc(r.reason) + (r.verify ? ' <span class="warn">(verify)</span>' : '')]);
    // the permitted baseline is a separate RMA test from the future receiving environment: name it by its section
    const pb = P.scenarios.filter(s => s.role === 'permitted');
    const pbText = pb.length ? para(`<b>Permitted baseline (RMA s104(2)):</b> ${pb.map(s => `${esc(s.name)} (${yearOf(s) || '–'})`).join(', ')}. Under s104(2) the consent authority <i>may</i> disregard an adverse effect of the activity if a national environmental standard or the plan permits an activity with that effect; whether it does is a matter for the consent authority, not this model.`)
      : note('No permitted baseline scenario (RMA s104(2)) is set. If one is relied on, add a scenario with the role "Permitted baseline" so its effects are shown separately from the future receiving environment.');
    return `${c.table('Committed and consented development and funded projects', { head: ['Item', 'Type', 'Status', 'Funding', 'Source', 'As at', 'Included in S1', 'Reason'], rows, empty: 'The register is empty. Add committed developments and network projects (AT ITA s3.6.1).' })}
      ${pbText}${c.author('receiving', 'Describe the future receiving environment: committed and consented developments, funded projects, and any permitted baseline (RMA s104(2)).')}`;
  } });

  /* ---------- 3 Description of proposal ---------- */
  const devZones = sc => Object.values(sc.zones).filter(z => z.kind !== 'gateway' && Engine.zoneClass(z) !== 'existing');
  sec({ id: 'report-landuse', chapter: '3', order: 10, at: '3.1', title: 'Land-use schedule', tag: 'auto', check: P => devZones(pick(P).with).length > 0, render(P, c) {
    const sc = c.sel.with, rows = [];
    // with the land-use feature, its Table T1 (activity, hours, opening year and the source of each quantity) is the schedule
    const lp = sc && lpParts(sc, ['T1']);
    if (lp) return lpTables(lp, 'Land-use schedule');
    for (const z of devZones(sc)) for (const [k, q] of Object.entries(z.lu || {})) if (+q > 0 && LANDUSES[k]) rows.push([esc(z.name || z.id), esc(LANDUSES[k].label), R.num(+q), esc(LANDUSES[k].unit), esc(Engine.zoneClass(z)), yearTxt(z.year), esc((rs(P).text || {})['hours-' + z.id] || '–')]);
    return c.table('Table T1 · Land-use schedule', { head: ['Zone', 'Activity', { label: 'Quantity', n: true }, 'Unit', 'Class', 'Opening year', 'Hours'], rows, empty: 'No development zones in the assessed scenario.' }, { source: `Project inputs (${esc(sc.name)})` });
  } });
  sec({ id: 'report-access', chapter: '3', order: 20, at: '3.2', title: 'Access', tag: 'author', check: P => !!String(rs(P).text.access || '').trim(), render: (P, c) => c.author('access', 'Describe each vehicle, pedestrian and cycle access, its form and control, sight distances, and compliance with AUP E27.6.4.') });
  sec({ id: 'report-accessibility', chapter: '3', order: 30, at: '3.3', title: 'Accessibility and parking', tag: 'mixed', render: (P, c) =>
    `${para('Walking, cycling and public transport access is described in section 2.4, and levels of service in section 4.')}${c.author('parking', 'Describe parking, loading and accessible parking against AUP E27.6.2 and E27.6.3 (no parking minimums apply under NPS-UD Policy 11).')}` });
  sec({ id: 'report-staging-plan', chapter: '3', order: 40, at: '3.4', title: 'Staging and sequencing', tag: 'mixed', render: (P, c) =>
    `${rs(P).staging ? para('Build-out increments and the staging triggers are set out in section 5 (Table M2).') : note('The staging test has not been run (Report tab › Run the staging test).')}${c.author('staging', 'Describe the proposed staging and its sequencing with infrastructure.')}` });
  sec({ id: 'report-construction', chapter: '3', order: 50, at: '3.5', title: 'Construction traffic', tag: 'author', render: (P, c) => c.author('construction', 'Describe construction traffic volumes, routes, hours and the draft construction traffic management plan (AT ITA s3.5).') });
  const hasMode = id => { try { if (typeof Registry !== 'undefined' && Registry.get('modes', id)) return true; return typeof Render !== 'undefined' && !!Render.modeDef && (Render.modeDef(id) || {}).id === id; } catch { return false; } };
  // Development traffic figures for 3.6: the development's trips alone, and their share of each road's flow
  const devFigures = (P, c, sc) => c.periods.map(p => c.figure({ id: `dev-${sc.id}-${p}`, title: `Development traffic only (assignment of the development's trips), ${sc.name}, ${p} peak hour ${yearOf(sc)} (veh/h, rounded to the nearest 10; line width shows the flow)`, mode: 'dev', scenarioId: sc.id, period: p, bounds: studyBounds(P), show: { counts: false, crashes: false } })
    + (hasMode('devshare') ? c.figure({ id: `devshare-${sc.id}-${p}`, title: `Development traffic as a share of each road's modelled flow, ${sc.name}, ${p} peak hour ${yearOf(sc)} (%)`, mode: 'devshare', scenarioId: sc.id, period: p, bounds: studyBounds(P), show: { counts: false, crashes: false } }) : '')).join('');
  sec({ id: 'report-tripgen', chapter: '3', order: 60, at: '3.6', title: 'Trip generation, distribution and assignment', tag: 'auto', landscape: true, check: P => hasRes(P, r => r.dev && r.dev.trips > 0), render(P, c) {
    const sc = c.sel.with, S = P.settings, D0 = defaultSettings(), rows = [];
    // with the land-use feature: its person trips, mode share and gross-to-net vehicle trips (T2–T4), its gateway
    // distribution (T5) and its without/with junction list, printed once here (the feature's own sections are merged)
    const lp = sc && lpParts(sc, ['T2', 'T3', 'T4', 'T4b', 'check', 'flags']);
    if (lp) {
      const L = LP(), t5 = typeof L.t5Html === 'function' ? `<h4>T5 Development traffic by gateway</h4>${L.t5Html(sc)}` : '';
      const ww = typeof L.wwHtml === 'function' ? (c.sel.without ? `<h4>Junctions without and with the development (largest changes and new accesses)</h4>${L.wwHtml(sc, c.sel.without)}` : note(`No same-year scenario without the development for ${esc(sc.name)} (${yearOf(sc)}): choose one in the Report tab to list junction changes.`, true)) : '';
      return `${lpTables(lp + t5 + ww, 'Trip generation')}${devFigures(P, c, sc)}`;
    }
    for (const z of devZones(sc)) for (const p of ['AM', 'PM']) {
      const te = Engine.tripEnds(z, S, p, sc);
      for (const [k, q] of Object.entries(z.lu || {})) {
        const r = ((S.rates || {})[k] || {})[p], L = LANDUSES[k]; if (!r || !L || !(+q > 0)) continue;
        const gross = +q / L.per * r.rate, red = U.clamp(+z.reduction || 0, 0, 100), stage = Engine.stageOf(z, sc), open = !(+z.year > Engine.yearOf(sc));
        const net = open ? gross * (1 - red / 100) * stage : 0, dflt = (((D0.rates || {})[k] || {})[p] || {}).rate === r.rate;
        rows.push([esc(z.name || z.id), esc(L.label), p, R.num(r.rate, 2), dflt ? 'Spare Construct indicative default (replace with surveyed rates)' : 'User-entered', R.num(gross, 0), red ? `−${R.pct(red)}` : '–', stage < 1 ? `${R.pct(stage * 100)} built` : '–', R.num(net, 0), R.pct(r.inPct), R.pct(100 - r.inPct), R.num(net * r.inPct / 100, 0), R.num(net * (1 - r.inPct / 100), 0)]);
      }
      // z.extra mirrors the land-use feature's table while it is synced there (z.tg.synced 'extra'): not the author's own extra trips
      if (te && !(z.tg && z.tg.on && z.tg.synced === 'extra') && (z.extra && (z.extra[p + 'in'] || z.extra[p + 'out']))) rows.push([esc(z.name), 'Extra trips (user)', p, '–', 'User-entered', '–', '–', '–', R.num((+z.extra[p + 'in'] || 0) + (+z.extra[p + 'out'] || 0)), '–', '–', R.num(+z.extra[p + 'in'] || 0), R.num(+z.extra[p + 'out'] || 0)]);
    }
    const resL = periodsOf(sc).map(p => resOf(sc, p)).filter(r => r.dev), dd = resL.length ? Engine.devDistribution(sc, resL) : null;
    const t5 = dd ? dd.rows.map(r => [esc(r.name), esc((sc.zones[r.id] || {}).gwSrc ? 'Counts' : 'Estimated'), R.pct(r.AMin), R.pct(r.AMout), R.pct(r.PMin), R.pct(r.PMout), Object.entries(r.vph).map(([p, v]) => `${p} ${R.veh(v)}`).join(', ')]) : [];
    return `${c.table('Table T4 · Vehicle trip generation', { head: ['Zone', 'Activity', 'Period', { label: 'Rate veh trips/unit/h', n: true }, 'Rate source', { label: 'Gross veh trips', n: true }, { label: '− Reduction', n: true }, { label: 'Build-out', n: true }, { label: 'Net', n: true }, { label: 'In %', n: true }, { label: 'Out %', n: true }, { label: 'In veh/h', n: true }, { label: 'Out veh/h', n: true }], rows, empty: 'No development zones.' },
        { source: 'Trip rates: project settings (Appendix A); land-use quantities, build-out and opening years: project zones (user)', note: 'Reductions are the zone\'s trip reduction for public transport and active modes; internal and pass-by trips are not deducted separately. Person trips and mode share (Tables T2 and T3) are added by the land-use feature when person-trip rates are loaded.' })}
      ${c.table('Table T5 · Gateway distribution of development traffic', { head: ['Gateway', 'Basis', { label: 'AM in %', n: true }, { label: 'AM out %', n: true }, { label: 'PM in %', n: true }, { label: 'PM out %', n: true }, 'veh/h'], rows: t5, empty: 'Run the model for the assessed scenario to see where development traffic enters and leaves.' },
        { note: dd ? Object.entries(dd.periods).map(([p, x]) => `${p}: ${R.pct(x.internalPct)} of development trips stay inside the study area`).join('; ') + '.' : '' })}
      ${devFigures(P, c, sc)}`;
  } });

  /* ---------- 4 Transport assessment ---------- */
  sec({ id: 'report-scenarios', chapter: '4', order: 10, at: '4.1', title: 'Assessment scenarios', tag: 'auto', check: P => P.scenarios.some(s => s.role), render(P, c) {
    const t = typeof Scenarios !== 'undefined' ? Scenarios.table(P) : [];
    const rows = t.map(r => [esc(r.id || '–'), esc(r.name), esc(r.roleLabel), yearTxt(r.year), esc(r.landUse), esc(r.network), esc(r.periods), r.buildout != null ? R.pct(r.buildout) : '–', esc(r.growthBasis)]);
    const checks = typeof Scenarios !== 'undefined' ? P.scenarios.flatMap(s => Scenarios.check(s, P).filter(x => x.level !== 'info').map(x => `${x.level === 'block' ? '<b class="bad">Blocking:</b>' : '<b class="warn">Check:</b>'} ${esc(x.text)}`)) : [];
    return `${c.table('Assessment scenarios', { head: ['ID', 'Scenario', 'Role', 'Year', 'Land use', 'Network', 'Periods', { label: 'Build-out', n: true }, 'Growth basis'], rows })}
      ${para(`Assessed in this report: <b>${esc(c.sel.with ? c.sel.with.name : '–')}</b>${c.sel.without ? ` against <b>${esc(c.sel.without.name)}</b>` : ''}${c.sel.mitigation ? `, with mitigation <b>${esc(c.sel.mitigation.name)}</b>` : ''}.`)}
      ${checks.length ? `<ul class="bul">${checks.map(x => `<li>${x}</li>`).join('')}</ul>` : ''}`;
  } });
  sec({ id: 'report-modelling', chapter: '4', order: 20, at: '4.2', title: 'Model description and calibration summary', tag: 'auto', check: P => hasRes(P, r => r.converged !== false), render(P, c) {
    const S = P.settings, sc = c.sel.existing || c.sel.base;
    const reps = ['AM', 'PM'].map(p => (resOf(sc, p) ? Calibrate.report(sc, p) : null));
    const ids = [...new Set(reps.filter(Boolean).flatMap(r => r.checks.map(k => k.id)))];
    const rows = ids.map(id => { const k = reps.map(r => r && r.checks.find(x => x.id === id)); const k0 = k.find(Boolean); return [esc(k0.label), esc(k0.target || '–'), ...k.map(x => x ? esc(x.status || '–') : '–'), esc(k0.source || ''), k0.verified ? 'Yes' : 'No']; });
    const conv = P.scenarios.flatMap(s => periodsOf(s).map(p => ({ s, p, r: resOf(s, p) })));
    const bad = conv.filter(x => x.r.converged === false);
    return `${para(`Spare Construct assigns peak-hour vehicle demand (gravity distribution with gateway through-traffic) to the network by conjugate Frank-Wolfe user equilibrium, with BPR link times (α ${esc(S.bprAlpha)}, β ${esc(S.bprBeta)}) and HCM 6 / Akçelik junction delay. Background growth: ${esc(Engine.growthBasis(S))}. Convergence target: relative gap ${N(S.gapTol * 100, 3)}% with the TAG M3.1 stability rule, at most ${int(S.maxIter)} iterations. Full inputs are in Appendix A.`)}
      ${bad.length ? note(`${plural(bad.length, 'run')} did not converge: ${bad.map(x => `${esc(x.s.name)} ${x.p} (${esc(x.r.convNote || '')})`).join('; ')}. Results from these runs are not reliable.`, true) : para('Every stored run converged to its target (Appendix B).')}
      ${c.table(`Table C-SUM · Calibration summary, ${esc(sc.name)}`, { head: ['Criterion', 'Target', 'AM', 'PM', 'Source', 'Verified'], rows, empty: 'Run the existing (base-year) scenario and load counts to see the calibration summary.' },
        { note: reps.filter(Boolean).map(r => `${r.period}: ${esc(r.summary)}`).join(' ') + ' Detail in Appendix B.' })}
      ${para(esc((P.data && P.data.msm && P.data.msm.checked) ? 'MSM consistency checked (see Appendix B).' : Engine.STANDARDS.msm.notChecked))}${disc('D7')}`;
  } });
  sec({ id: 'report-walk', chapter: '4', order: 30, at: '4.2.1', title: 'Walking, cycling and public transport levels of service', tag: 'mixed', render(P, c) {
    const R0 = rs(P), ped = R0.ped || {};
    const rows = (R0.crossings || []).map(x => { const d = pedDelay(x.control, { C: x.C, g: x.g, L: x.length, flow: x.flow, Sp: ped.Sp, ts: ped.ts });
      return [esc(x.name), esc(x.control), R.veh(+x.flow), R.num(+x.length, 1), d == null ? '–' : R.num(d, 1) + (x.control === 'zebra' ? ' (drivers give way)' : ''), '–', x.next ? R.q(+x.next) : '–', '–']; });
    return `${c.table('Table WK-1 · Pedestrian crossings', { head: ['Crossing', 'Control', { label: 'Conflicting veh/h', n: true }, { label: 'Length m', n: true }, { label: 'Pedestrian delay s', n: true }, 'ANOP walk LoS (delay)', { label: 'Next crossing m', n: true }, 'ANOP LoS (spacing)'], rows, empty: 'No crossings entered. Add them in the Report tab.' },
      { note: `Signals d = (C − g)² / 2C; uncontrolled d = (e^(λ·tc) − λ·tc − 1) / λ with λ = conflicting veh/s and tc = L / ${esc(ped.Sp || PED.Sp)} m/s + ${esc(ped.ts != null ? ped.ts : PED.ts)} s; a refuge is two stages at half the flow and half the length. ANOP levels of service: confirm in the AT ANOP tool.`, d: ['D5'] })}
      ${c.author('walk', 'Assess public transport, walking and cycling levels of service with the proposal (AT ITA s4.2.1).')}`;
  } });
  /* With and without must be modelled for the same year (AT ITA s4.2.1.1); otherwise background growth would be
     counted as an effect of the proposal. → '' when they match, else the message to print. */
  function yearClash(w, wo, periods = PERIODS) {
    if (!w || !wo) return '';
    const ys = s => [yearOf(s), ...periods.map(p => resOf(s, p)).filter(Boolean).map(r => +r.year || null)].filter(Boolean);
    const a = ys(w), b = ys(wo);
    if (a.every(y => b.every(z => z === y))) return '';
    return `${wo.name} is modelled for ${b[0] || '?'} and ${w.name} for ${a[0] || '?'}. The with-and-without comparison needs both in the same year (AT ITA Guidelines s4.2.1.1); otherwise background growth would be counted as an effect of the proposal. Create a same-year without scenario (Scenarios › Create assessment set) or choose one in the Report tab, and run it.`;
  }
  // Extent test per period between the without and with scenarios (sc.extent from a stored test is used for sites)
  function extentData(P, c) {
    const w = c.sel.with, wo = c.sel.without; if (!w || !wo || yearClash(w, wo)) return [];
    return c.periods.filter(p => resOf(w, p) && resOf(wo, p)).map(p => ({ p, x: Engine.extentTest(w, resOf(wo, p), resOf(w, p), { scWithout: wo }) })).filter(e => e.x);
  }
  sec({ id: 'report-extent', chapter: '4', order: 38, at: '4.2.2', title: 'Extent of effects', tag: 'auto', landscape: true, check: P => { const s = pick(P); return !!(s.with && s.without && periodsOf(s.with).some(p => resOf(s.without, p))); }, render(P, c) {
    const yc = yearClash(c.sel.with, c.sel.without);
    if (yc) return note(esc(yc), true);
    const ex = extentData(P, c);
    if (!ex.length) return note(`The extent test needs a without and a with scenario for the same year, both run for the same period.${c.sel.with && !c.sel.without ? ` No same-year without scenario was found for ${esc(c.sel.with.name)} (${yearOf(c.sel.with)}).` : ''} Set them in the Report tab and run both.`, true);
    const X = Engine.STANDARDS.extent;
    return ex.map(({ p, x }) => {
      const inc = x.rows.filter(r => r.included), list = inc.length ? inc : x.rows.slice(0, 15), rows = list.slice(0, 60).map(r => [esc(r.name || r.link), esc(r.dir.toUpperCase()), esc([r.fc.gt, r.fc.pt, r.fc.freight].map(v => v || '–').join(' / ')) + (r.basis ? ` <span class="muted">(${esc(r.basis)})</span>` : ''),
        R.veh(r.without), R.veh(r.with), U.fmt.signed(r.d, 0), esc(r.pText), r.threshold != null ? R.pct(r.threshold * 100) : '–', r.included ? '<b>Yes</b>' : 'No']);
      return c.table(`Extent of effects · ${p} peak · ${esc(c.sel.without.name)} → ${esc(c.sel.with.name)}`, { head: ['Link', 'Dir', 'FC GT / PT / freight', { label: 'Without veh/h', n: true }, { label: 'With veh/h', n: true }, { label: 'Δ veh/h', n: true }, { label: 'Δ %', n: true }, { label: 'Threshold', n: true }, 'Included'], rows },
        { sup: true, source: `${sourceOf(c)}; Future Connect classes: ${loaded(P).fc ? esc(AT_LIC()) : 'Spare Construct defaults from road class (Future Connect not loaded)'}`, note: `${inc.length ? `${plural(inc.length, 'link direction')} and ${plural(x.sites.length, 'junction')} are included.${list.length > 60 ? ` The 60 largest of the ${U.fmt.int(list.length)} included link directions are shown; every link is in links.csv in the data pack.` : ''}` : 'No link passes the thresholds; the 15 largest changes are shown.'} Included when Δ% is over 5% (motorways, arterials, RTN/FTN, freight levels 1A–2) or 10% (collectors) and Δ ≥ ${X.floor} veh/h. The ${X.floor} veh/h floor and the freight set are Spare Construct assumptions. ${esc(x.note || '')}`, d: ['D13'] }) +
        // without and with plots per period (extent-and-difference)
        [[c.sel.without, 'without the proposal'], [c.sel.with, 'with the proposal']].map(([s, w]) => c.figure({ id: `vol-${s.id}-${p}`, title: `Modelled traffic ${w}, ${s.name}, ${p} peak hour ${yearOf(s)} (veh/h; line width shows the flow)`, mode: 'volume', scenarioId: s.id, period: p, bounds: studyBounds(P), show: { counts: false, crashes: false } })).join('') +
        // with minus without: Render.snapshot compares the with scenario with the chosen without scenario (baseScenarioId),
        // whichever scenario is the project's base
        c.figure({ id: `diff-${p}`, title: `Change in traffic, ${p} peak hour: ${c.sel.with.name} minus ${c.sel.without.name} (veh/h, rounded to the nearest 10)`, mode: 'diff', scenarioId: c.sel.with.id, baseScenarioId: c.sel.without.id, period: p, bounds: studyBounds(P), show: { counts: false, crashes: false } });
    }).join('') + (c.sel.with.extent ? '' : note('The extent sites are not stored on the scenario yet, so the assessed junctions are Spare Construct\'s own selection. Store the extent test (junctions feature) to use it for the SIDRA export and this report.'));
  } });
  sec({ id: 'report-junctions', chapter: '4', order: 39, at: '4.2.3', title: 'Junction performance', tag: 'auto', landscape: true, check: P => hasRes(P, r => Object.keys(r.nodes || {}).length > 0), render(P, c) {
    const sel = c.sel, parts = [];
    for (const [sc, t] of [[sel.without, 'Table J2 · Junction performance without the proposal'], [sel.with, 'Table J2 · Junction performance with the proposal'], [sel.mitigation, 'Table J2 · Junction performance with mitigation']]) if (sc && periodsOf(sc).length) parts.push(j2Table(P, c, sc, t));
    // J3 without vs with (same year only)
    const yc = yearClash(sel.with, sel.without);
    if (yc) parts.push(note(esc(yc), true));
    else if (sel.without && sel.with) {
      const ids = new Set(assessedSites(P, sel).ids), rows = [];
      for (const p of c.periods) {
        const a = resOf(sel.without, p), b = resOf(sel.with, p); if (!a || !b) continue;
        const ww = Engine.withWithout(a, b, { name: id => siteName(sel.with, b, id) });
        for (const r of ww.rows.filter(r => ids.has(r.id) && r.delta)) rows.push([esc(r.name), p, R.dos(r.without.dos), R.dos(r.with.dos), esc(r.delta.dosText), R.delay(r.without.delay), R.delay(r.with.delay), esc(r.delta.delayText), esc(r.delta.los), esc(r.delta.q95Text)]);
      }
      parts.push(c.table('Table J3 · Without and with the proposal', { head: ['Junction', 'Period', { label: 'DoS without', n: true }, { label: 'DoS with', n: true }, { label: 'Δ DoS', n: true }, { label: 'Delay without s', n: true }, { label: 'Delay with s', n: true }, { label: 'Δ delay', n: true }, 'LoS change', { label: 'Δ Q95', n: true }], rows }, { junction: true, note: esc(Engine.SELECT_LABEL || '') }));
    }
    // queue storage check: approach queue against the approach link length
    const sc = sel.mitigation || sel.with, qrows = [];
    if (sc) for (const x of junctionRows(P, c, sc)) if (x.nr) for (const a of x.nr.appr) {
      const L = sc.links[a.link]; if (!L || !(a.q95 > 0)) continue;
      const st = Net.len(sc, L), ratio = st > 0 ? a.q95 / st : null;
      if (ratio > 0.5) qrows.push([esc(x.name), x.p, esc(`${a.approach || ''}: ${a.name || ''}`), R.q(a.q95), R.q(st), R.num(ratio, 2), ratio > 1 ? '<b class="bad">Exceeds storage</b>' : ratio > 0.8 ? '<span class="warn">Near storage</span>' : '']);
    }
    if (sc) parts.push(c.table(`Queue storage check · ${esc(sc.name)}`, { head: ['Junction', 'Period', 'Leg', { label: 'Q95 m', n: true }, { label: 'Storage m', n: true }, { label: 'Ratio', n: true }, 'Flag'], rows: qrows, empty: 'No approach queue reaches half its storage length.' },
      { junction: true, note: 'Storage = the length of the approach link to the next junction. Rows with a ratio over 0.5 are listed.' }));
    const LOS = Engine.STANDARDS.los, m = (P.settings.junction || {}).losMethod || 'hcm6', th = LOS[m] || LOS.hcm6;
    parts.push(c.table('Level of service definitions', { head: ['LoS', 'Signals: delay s', 'Roundabouts: delay s', 'Give way and stop: delay s'], rows: 'ABCDE'.split('').map((l, i) => [l, `≤ ${th.signals[i]}`, `≤ ${th.roundabout[i]}`, `≤ ${th.sign[i]}`]).concat([['F', `> ${th.signals[4]} or DoS > 1`, `> ${th.roundabout[4]} or DoS > 1`, `> ${th.sign[4]} or DoS > 1`]]) },
      { source: `${esc(th.name)}, ${esc(th.edition)}${th.verified ? '' : ' (not yet verified)'}`, junction: true, sup: false }));
    for (const p of c.periods) if (sc) parts.push(c.figure({ id: `vc-${sc.id}-${p}`, title: `Volume/capacity and junction level of service, ${sc.name}, ${p} peak hour`, mode: 'vc', scenarioId: sc.id, period: p, bounds: studyBounds(P), show: { counts: false, crashes: false } }));
    const jx = Registry.get('report', 'junctions-performance');
    return parts.join('') + note(jx
      ? 'The junction flow-scale test (+10% on every movement) follows this section, then turning movement diagrams for the key junctions; per-turn movement summaries are in Appendix C. The sensitivity pack (section 4.6) re-runs the whole network with trip rates and total demand ±10% and background growth ±50%.'
      : 'Approach-level movement summaries are in Appendix C. The +10% flow test is part of the sensitivity pack (section 4.6).');
  } });
  sec({ id: 'report-safety', chapter: '4', order: 60, at: '4.3', title: 'Road user safety', tag: 'mixed', landscape: true, check: P => !!String(rs(P).text.ssa || '').trim(), render(P, c) {
    const cy = crashYears(P), sel = c.sel, ptd = P.settings.peakToDaily || 10;
    // two-way AADT on each leg of the junction (all its nodes; ring links inside it are not approaches)
    const legAADT = (sc, s) => { const rl = periodsOf(sc).filter(p => p !== 'IP').map(p => resOf(sc, p)), mem = (s.nodes || [s.id]).filter(x => sc.nodes[x]); if (!rl.length || !sc.nodes[s.id] || !mem.length) return null;
      return externalLegs(sc, mem).map(g => { let v = 0; for (const r of rl) for (const l of g.links) { const a = r.arcs[l.id + ':ab'], b = r.arcs[l.id + ':ba']; v += (a ? a.v : 0) + (b ? b.v : 0); } return { l: g.links[0], links: g.links, aadt: v / rl.length * ptd }; }).sort((a, b) => b.aadt - a.aadt); };
    const predict = (sc, s) => {
      const id = s.id, legs = legAADT(sc, s); if (!legs || legs.length < 3) return null;
      const n = sc.nodes[id], multi = legs.some(x => x.links.some(l => Math.max(l.lanesAB | 0, l.lanesBA | 0) >= 2)), mid = cecModelFor(n.control, legs.length, multi); if (!mid) return null;
      const q = legs.map(x => x.aadt), qmaj = (q[0] + q[1]) / 2, qmin = legs.length >= 4 ? (q[2] + q[3]) / 2 : q[2];
      const pr = CEC.models[mid].perApproach ? { ...cecPredict(mid, q[0]), A: legs.reduce((s, x) => s + cecPredict(mid, x.aadt).A, 0), flag: 'range not recorded (check CEC Table 7-3)' } : cecPredict(mid, qmaj, qmin);
      return { ...pr, dsi: pr.A * severityIndex('', n.control) };
    };
    const sites = crashSites(P, c), rows = [];
    // observed ÷ predicted compares the crash years with a prediction from the existing (base-year) volumes
    const exSc = sel.existing && sel.existing !== sel.with ? sel.existing : null;
    for (const s of sites.slice(0, 20)) {
      const a = sel.without ? predict(sel.without, s) : null, b = sel.with ? predict(sel.with, s) : null, m = sel.mitigation ? predict(sel.mitigation, s) : null, e = exSc ? predict(exSc, s) : null;
      if (!a && !b) continue;
      const obs = s.injury / HRIG.years, ref = a || b;
      rows.push([esc(s.name), esc(ref.label), `${a ? R.num(a.A, 3) : '–'} → ${b ? R.num(b.A, 3) : '–'}${m ? ` → ${R.num(m.A, 3)}` : ''}`, `${a ? R.num(a.dsi, 4) : '–'} → ${b ? R.num(b.dsi, 4) : '–'}${m ? ` → ${R.num(m.dsi, 4)}` : ''}`,
        a && b ? U.fmt.signed(b.dsi - a.dsi, 4) : '–', R.num(obs, 2), e && e.A > 0 ? R.num(obs / e.A, 2) : '–', esc(ref.flag)]);
    }
    const ssmRows = sites.slice(0, 3).map(s => ssmMatrix(P, c, s)).join('');
    return `${para(cy ? `Section 2.7 sets out the crash history for ${cy.from}–${cy.to}: ${plural(cy.total.n, 'crash', 'crashes')}, ${cy.total.F + cy.total.S} of them fatal or serious.` : 'No crash history is loaded (section 2.7).')}
      ${c.table('Effects of additional trips: predicted crashes (CEC)', { head: ['Site', 'Model', 'Predicted injury crashes/yr (without → with → mitigation)', 'Predicted DSI/yr', { label: 'Δ DSI/yr', n: true }, { label: 'Observed injury/yr', n: true }, { label: 'Observed ÷ predicted', n: true }, 'Range'], rows, empty: 'Run the without and with scenarios, and load crashes, to predict crash changes.' },
        { source: `${esc(CEC.name)}, ${esc(CEC.edition)} (coefficients verified: no)`, note: 'A = b0·Qmaj^b1·Qmin^b2 with two-way AADT (modelled peak × peak-to-daily factor); Qmaj and Qmin are the means of the two busiest and the remaining legs (Spare Construct reading). DSI from the CEC severity indices. A junction of several nodes (such as a roundabout) is one site, with the links that leave it as its approaches. Observed ÷ predicted divides the observed injury crashes a year by the prediction from the existing (base-year) scenario\'s volumes. Midblock and rural (80 km/h and over) models are not applied.' })}
      ${ssmRows}${c.author('ssa', 'AT ITA 4.3.5: is a Stage 1 or Stage 2 Safe System Audit needed? State why, and who will carry it out.')}${disc('D8')}`;
  } });
  function ssmMatrix(P, c, s) {
    const lik = (rs(P).ssm || {})[s.id] || {}, aadt = Math.max(0, ...s.legs.map(l => l.aadt || 0)), speed = s.speed;
    const vru = { ped: lik.pedDay, cyc: lik.cycDay, mc: lik.mcDay };
    const cells = SSM.cols.map(([k]) => {
      const E = ['ped', 'cyc', 'mc'].includes(k) ? band4(vru[k] != null && vru[k] !== '' ? +vru[k] : null, SSM.vruExposure) : band4(aadt || null, SSM.vehExposure);
      const Lk = +lik[k] > 0 ? +lik[k] : null, Sv = sevHint(speed, SSM.safe[k]);
      return { E, L: Lk, S: Sv, prod: E && Lk && Sv ? E * Lk * Sv : null };
    });
    const row = (lab, f) => [lab, ...cells.map(f)];
    return c.table(`Safe System screening matrix · ${esc(s.name)}`, { head: ['', ...SSM.cols.map(([, l]) => l)], rows: [
      row('Exposure (1–4)', x => x.E ?? '?'), row('Likelihood (1–4, author)', x => x.L ?? '?'), row('Severity (1–4)', x => x.S ?? '?'), row('<b>Product (max 64)</b>', x => x.prod != null ? `<b>${x.prod}</b>` : '–')] },
    { note: `<b>${esc(SSM.label)}.</b> Exposure from the busiest leg's AADT (${R.adt(aadt)} vpd; < 1k = 1, 1–5k = 2, 5–10k = 3, > 10k = 4) and from pedestrian, cyclist and motorcyclist counts per day where entered (< 10, 10–50, 50–100, > 100). Severity hints: ${esc(SSM.severityBasis)} (Safe System speeds: pedestrians, cyclists and motorcyclists 30, run-off-road 40, side impact 50, head-on 70 km/h; posted ${speed || '?'} km/h). Likelihood is the author's.`, source: `${esc(SSM.name)}, ${esc(SSM.edition)}`, d: ['D13'] });
  }
  sec({ id: 'report-alignment', chapter: '4', order: 70, at: '4.4', title: 'Network and policy alignment', tag: 'mixed', render(P, c) {
    const w = c.sel.with, fc = w ? Object.values(w.links).filter(l => l.fc && l.status !== 'closed') : [];
    const cls = new Map(); for (const l of fc) { const k = l.fc.gt || 'Unclassified'; cls.set(k, (cls.get(k) || 0) + Net.len(w, l) / 1000); }
    return `${fc.length ? c.table('Future Connect general traffic classes in the study area', { head: ['Class', { label: 'km', n: true }], rows: [...cls].sort((a, b) => b[1] - a[1]).map(([k, v]) => [esc(k), R.num(v, 2)]) }, { source: esc(AT_LIC()) }) : ''}
      ${c.author('alignment', 'Assess alignment with Future Connect, ANOP, RLTP and LTP projects, the Future Development Strategy (where required) and NPS-UD Policy 1.')}`;
  } });
  // AT ITA s4.5. A placeholder: the climate overlay (another feature) takes this number when it prints its section
  sec({ id: 'report-climate', chapter: '4', order: 75, at: '4.5', title: 'Climate adaptation', tag: 'author', fallback: true, render: (P, c) =>
    `${note('No climate hazard overlay is loaded for this project, so Spare Construct has not screened the network against flood, coastal inundation or slip hazards.')}${c.author('climate', 'AT ITA s4.5: describe how the proposal and its access routes respond to climate hazards (flooding, coastal inundation, slips, heat), and any resilience measures.')}` });
  sec({ id: 'report-sensitivity', chapter: '4', order: 80, at: '4.6', title: 'Sensitivity tests', tag: 'auto', check: P => !!(rs(P).sens && rs(P).sens.rows), render(P, c) {
    const sv = rs(P).sens;
    if (!sv || !sv.rows) return note('The sensitivity pack has not been run (Report tab › Run the sensitivity pack). AT ITA s3.6.1(v) and the Practice Note s9.3(a)(x) expect sensitivity testing.', true);
    const w = x => (x && typeof x === 'object' ? `${esc(x.name || x.id || '')}${x.dos != null ? ` (DoS ${R.dos(x.dos)})` : ''}` : esc(x || '–'));
    const rows = sv.rows.map(r => [esc(r.label), esc(r.input || '–'), r.trips != null ? R.veh(r.trips) : '–', r.vht != null ? R.num(r.vht, 1) : '–', w(r.worst), r.losEF != null ? int(r.losEF) : '–', r.over5 != null ? `${int(r.over5)} / ${int(r.over10)}` : '–', r.elasticity ? R.num(r.elasticity.vht, 2) : r.bcrText ? `BCR ${esc(r.bcr8Text)}` : '–']);
    return `${c.table(`Table SENS-1 · Sensitivity tests (${esc(sv.scName || '')}, run ${esc(day(sv.at))})`, { head: ['Test', 'Input change', { label: 'Veh trips', n: true }, { label: 'VHT veh·h', n: true }, 'Worst site (DoS)', { label: 'Sites LoS E/F', n: true }, 'Links over 5% / 10%', { label: 'VHT elasticity', n: true }], rows }, { note: esc(sv.note || ''), d: (sv.rows || []).some(r => r && (r.bcrText || r.bcr8Text)) ? ['D4'] : [] })}
      <ul class="bul">${(sv.sentences || []).map(s => `<li>${esc(s)}</li>`).join('')}</ul>${(sv.flags || []).length ? note(sv.flags.map(esc).join(' '), true) : ''}`;
  } });
  /* MBCM compares a transport option with the do-minimum at the same travel demand. The development is not a transport
     option: the do-minimum is the with-proposal scenario and the option is the mitigation, both in the same year. */
  const TRIP_TOL = 0.01;   // Spare Construct tolerance: the option's vehicle trips within 1% of the do-minimum's
  sec({ id: 'report-economics', chapter: '4', order: 84, at: '4.7', title: 'Indicative economics', tag: 'auto', check: P => { const s = pick(P); return s.with && s.mitigation ? true : null; }, render(P, c) {
    const base = c.sel.with, opt = c.sel.mitigation;
    if (!base || !opt) return note('Indicative economics compare a transport option (the mitigation scenario) with the do-minimum (the scenario with the proposal), at the same travel demand. The development itself is not a transport option, so no benefit-cost ratio is given for it. Create a mitigation scenario (Scenarios › Create assessment set) or choose one in the Report tab.');
    const opts = [opt], out = [];
    for (const o of opts) {
      const e = Engine.econ(base, o, c.period); if (!e) { out.push([esc(o.name), '<span class="muted">Run both scenarios first.</span>', '', '', '', '', '', '', '', '']); continue; }
      if (e.mismatch) { out.push([esc(o.name), esc(e.note), '', '', '', '', '', '', '', '']); continue; }
      const tb = resOf(base, c.period), to = resOf(o, c.period), tdiff = tb && to && tb.kpi && to.kpi && tb.kpi.trips > 0 ? Math.abs(to.kpi.trips - tb.kpi.trips) / tb.kpi.trips : 0;
      if (tdiff > TRIP_TOL) { out.push([esc(o.name), `<span class="warn">Not comparable: ${esc(o.name)} has ${R.veh(to.kpi.trips)} vehicle trips and the do-minimum ${esc(base.name)} has ${R.veh(tb.kpi.trips)} (${R.pct(tdiff * 100)} apart, more than ${R.pct(TRIP_TOL * 100)}). Benefits would include the change in demand. Give both the same land use.</span>`, '', '', '', '', '', '', '', '']); continue; }
      const ipm = (e.ipm || []).map(b => `${esc(b.edition)}${b.draft ? ' (draft, subject to GPS 2027)' : ''}: ${esc(b.label)}`).join('<br>');
      out.push([esc(o.name), U.fmt.money(e.pv), U.fmt.money(e.pv8), U.fmt.money(e.cap), esc(e.bcrText || Engine.bcrText(e.bcr)), esc(e.bcr8Text || '–'), e.fyrr != null ? R.pct(e.fyrr * 100) : '–', U.fmt.money(e.npv), ipm || '–', e.snr && e.snr.R != null ? `${R.num(e.snr.R, 1)}×${e.snr.ok ? '' : ' <b class="warn">low</b>'}` : '–']);
      for (const wtxt of e.warnings || []) out.push({ cells: [`<span class="warn">${esc(wtxt)}</span>`], span: 10 });
    }
    const e0 = Engine.econ(base, opts[0], c.period);
    // the discount rates and the sensitivity rate are the analysis's own (NZTA GC 25/01: non-commercial 2% / 1.5% / 1%,
    // 8% sensitivity; commercial 8%, 2% sensitivity; older projects' uniform rate), never fixed text
    const ok0 = e0 && !e0.mismatch, sens = ok0 && e0.sensRate != null ? `${e0.sensRate}%` : 'sensitivity rate';
    const cen = ok0 ? (Array.isArray(e0.rates) && e0.rates.length ? e0.rates.map(r => `${r.r}%`).join(' / ') : e0.method === 'uniform' && (P.settings.econ || {}).discount != null ? `${+P.settings.econ.discount}% flat` : 'central rates') : 'central rates';
    const gc = ok0 && e0.discount ? ` Discount rates: ${esc(e0.discount.name)} (${esc(e0.discount.edition)}).` : '';
    return `${c.table('Indicative economics (screening only)', { head: ['Option', { label: `PV benefits (${esc(cen)})`, n: true }, { label: `PV (${esc(sens)})`, n: true }, { label: 'Capital', n: true }, { label: 'BCR', n: true }, { label: `BCR at ${esc(sens)}`, n: true }, { label: 'FYRR', n: true }, { label: 'NPV', n: true }, 'IPM efficiency', { label: 'Signal to noise', n: true }], rows: out },
      { note: `${esc((e0 && e0.label) || Engine.STANDARDS.mbcm.label)}. ${esc((e0 && e0.basis) || '')}.${gc} Option against the do-minimum ${esc(base.name)} (with the proposal), ${yearOf(base) || ''}. Capital costs are Spare Construct's indicative lane-km and junction rates; the 1% trip tolerance is a Spare Construct assumption.`, d: ['D4', 'D13'] })}`;
  } });

  /* ---------- 5 Mitigation ---------- */
  // What a scenario changes against another: links added, removed or changed, and junction controls
  function networkDiff(a, b) {
    const out = [];
    if (!a || !b) return out;
    for (const L of Object.values(b.links)) {
      const o = a.links[L.id];
      if (!o) out.push({ kind: 'link', id: L.id, text: `New ${CLASSES[L.cls] ? CLASSES[L.cls].label.toLowerCase() : 'road'}: ${L.name || 'unnamed road'} (${R.q(Net.len(b, L))} m, ${L.lanesAB}+${L.lanesBA} lanes)` });
      else {
        const ch = [];
        if (o.lanesAB !== L.lanesAB || o.lanesBA !== L.lanesBA) ch.push(`lanes ${o.lanesAB}+${o.lanesBA} → ${L.lanesAB}+${L.lanesBA}`);
        if (o.speed !== L.speed) ch.push(`speed ${o.speed} → ${L.speed} km/h`);
        if (o.status !== L.status) ch.push(`${o.status} → ${L.status}`);
        if (o.cls !== L.cls) ch.push(`class ${o.cls} → ${L.cls}`);
        if (o.busLane !== L.busLane) ch.push(L.busLane ? 'bus lane added' : 'bus lane removed');
        if (o.cycle !== L.cycle) ch.push(`cycle facility ${o.cycle} → ${L.cycle}`);
        if (ch.length) out.push({ kind: 'link', id: L.id, text: `${L.name || 'Unnamed road'}: ${ch.join(', ')}` });
      }
    }
    for (const L of Object.values(a.links)) if (!b.links[L.id]) out.push({ kind: 'link', id: L.id, text: `Removed: ${L.name || 'unnamed road'}` });
    for (const n of Object.values(b.nodes)) {
      const o = a.nodes[n.id];
      if (o && (o.control !== n.control || (n.upgrade && !o.upgrade) || (n.cycle !== o.cycle && n.control === 'signals')))
        out.push({ kind: 'node', id: n.id, control: n.control, text: `${CONTROLS[n.control] || n.control} at ${Kit.junctionName(n.id, b)}${o.control !== n.control ? ` (was ${(CONTROLS[o.control] || o.control).toLowerCase()})` : ''}${n.cycle && n.control === 'signals' ? `, cycle ${n.cycle} s` : ''}` });
    }
    return out;
  }
  sec({ id: 'report-mitigation', chapter: '5', order: 10, at: '5.1', title: 'Mitigation register', tag: 'mixed', landscape: true, check: P => !!pick(P).mitigation || !!String(rs(P).text.mitigation || '').trim(), render(P, c) {
    const sel = c.sel, mit = sel.mitigation, prop = sel.with, M = rs(P).mitigation || {}, E = P.settings.econ;
    const diff = mit && prop ? networkDiff(prop, mit) : [];
    const refs = c.refs[prop ? prop.id : ''] || {};
    const rows = diff.map((d, i) => {
      const ref = `M${i + 1}`, a = M[ref] || {};
      let eff = '–';
      if (d.kind === 'node' && prop) {
        const hit = periodsOf(prop).map(p => ({ p, nr: resOf(prop, p).nodes[d.id] })).filter(x => x.nr).sort((x, y) => y.nr.dos - x.nr.dos)[0];
        if (hit) eff = `<a href="#j-${esc(prop.id)}-${esc(d.id)}">${esc(refs[d.id] || 'J')} ${hit.p} DoS ${R.dos(hit.nr.dos)}, LoS ${esc(hit.nr.losText || hit.nr.los)}</a>`;
      }
      const cost = d.kind === 'node' ? (E.node || {})[d.control] : (() => { const L = mit.links[d.id]; return L && L.status === 'proposed' && !prop.links[d.id] ? Net.len(mit, L) / 1000 * ((L.lanesAB | 0) + (L.lanesBA | 0)) * ((E.laneKm || {})[L.cls] ?? 3) : null; })();
      return [ref, esc(d.text), eff, esc(a.options || '–'), esc(a.trigger || (rs(P).staging && rs(P).staging.text) || '–'), esc(a.responsible || 'Applicant'), cost != null ? U.fmt.money(cost) : '–', esc(a.mechanism || 'Condition'), esc(a.rltp || '–'), esc(a.land || '–')];
    });
    const list = [sel.with, sel.mitigation].filter(Boolean).map(s => { const d = networkDiff(sel.without || sel.base, s); return `<li><b>${esc(s.name)}</b>: ${d.length ? d.slice(0, 12).map(x => esc(x.text)).join('; ') + (d.length > 12 ? `; and ${d.length - 12} more` : '') : 'no network changes'}.</li>`; }).join('');
    return `${c.table('Table M1 · Mitigation register', { head: ['Ref', 'Measure', 'Effect addressed', 'Options considered', 'Trigger', 'Responsible', { label: 'Est. cost NZ$', n: true }, 'Mechanism', 'In RLTP/LTP', 'Land outside applicant control'], rows, empty: mit ? 'The mitigation scenario makes no network changes against the proposal.' : 'No mitigation scenario. Create one (Scenarios › Create assessment set) or set it in the Report tab.' },
      { note: 'Seeded from the mitigation scenario\'s changes against the proposal. Costs are Spare Construct\'s indicative rates. The author completes the options, trigger, responsibility, mechanism and land columns in the Report tab.' })}
      <p class="lg">What each option changes against ${esc((sel.without || sel.base).name)}:</p><ul class="bul">${list}</ul>${c.author('mitigation', 'Explain the mitigation chosen, the options considered and why the measures are proportionate.')}`;
  } });
  sec({ id: 'report-staging', chapter: '5', order: 20, at: '5.2', title: 'Staging triggers', tag: 'auto', check: P => !!rs(P).staging, render(P, c) {
    const st = rs(P).staging;
    if (!st) return note('Run the staging test in the Report tab: it runs the proposal at 10% build-out steps and finds the first increment where an assessed junction fails.', true);
    const tr = st.trigger, isT = r => tr && +tr.pct === +r.pct;
    const rows = (st.rows || []).map(r => ({ cells: [`${int(r.pct)}%`, R.num(+r.dwellings), R.num(+r.gfa), esc(r.worst ? r.worst.name : '–'), r.worst ? R.dos(+r.worst.dos) : '–', r.worst ? esc(r.worst.los) : '–', r.queue ? 'Yes' : 'No',
      isT(r) ? `<b>${esc(st.measure || 'Mitigation')}</b>` : '–', isT(r) ? (st.resolved == null ? 'Not tested' : st.resolved ? 'Yes' : '<b class="bad">No</b>') : ''], cls: isT(r) ? 'miss' : '' }));
    const trig = tr && (tr.sites || []).length ? `<ul class="bul">${tr.sites.map((id, i) => `<li>${esc((tr.names || [])[i] || id)}: ${esc((tr.why || [])[i] || '')}${(tr.how || [])[i] ? ` (${esc(tr.how[i])})` : ''}</li>`).join('')}</ul>` : '';
    const ex = (st.existingWhy || []).length ? c.table('Existing deficiencies at 0% build-out', { head: ['Site', 'Fails at 0% because'], rows: st.existingWhy.map(x => [esc(x.name), esc(x.why || '–')]) },
      { junction: true, note: 'These sites already fail without the development. They still trigger a stage when they fail a further criterion, or when their DoS rises by 0.05 or more over the 0% value (Spare Construct assumption).', d: ['D13'] }) : '';
    return `${c.table(`Table M2 · Build-out increments (${esc(st.scName)}, ${esc((st.periods || []).join(' and '))}, run ${esc(day(st.at))})`, { head: ['Build-out', { label: 'Dwellings', n: true }, { label: 'GFA m²', n: true }, 'Worst site', { label: 'Worst DoS', n: true }, 'LoS', 'Queue > storage', 'Measure triggered', 'Resolved'], rows },
      { junction: true, note: 'A site fails when it exceeds its practical DoS, reaches LoS E or F, or its 95th-percentile queue exceeds the approach storage. Dwellings and GFA are those the model loads at each step: each zone\'s quantity × its own build-out, for zones open by the scenario year.' })}
      ${para(st.text ? `<b>${esc(st.text)}</b>` : 'No assessed site fails at any increment.')}${trig}${ex}`;
  } });
  sec({ id: 'report-conditions', chapter: '5', order: 30, at: '5.3', title: 'Conditions and monitoring', tag: 'mixed', check: P => !!String(rs(P).text.conditions || '').trim(), render(P, c) {
    const st = rs(P).staging;
    return `${st && st.text ? para(`Staging condition from the staging test (section 5.2), for the author to confirm: <b>${esc(st.text)}</b>`) : ''}
      ${c.author('conditions', 'Draft condition parameters (RMA s108, s108AA): for each measure in Table M1, the effect it addresses (its nexus), the trigger, who delivers it and how compliance is shown.')}
      ${c.author('monitoring', 'Monitoring and travel plan (RMA Sch 4 cl 6(1)(g); AUP E27.9(2)): what is monitored, when, the threshold, and what happens when it is crossed.')}`;
  } });

  /* ---------- 6 Conclusion ---------- */
  function conclusionText(P, c) {
    const s = c.sel.mitigation || c.sel.with; if (!s || !periodsOf(s).length) return 'The assessed scenario has not been run, so no conclusion can be drawn from the model.';
    const rows = junctionRows(P, c, s).filter(r => r.s), ids = [...new Set(rows.map(r => r.id))];
    if (!rows.length) return `${s.name} has no assessed junctions.`;
    const worst = rows.slice().sort((a, b) => b.s.dos - a.s.dos)[0], over = new Set(rows.filter(r => r.s.dos > (r.s.xp || 1)).map(r => r.id));
    const who = c.sel.mitigation ? 'With the proposed mitigation' : 'With the proposal', pers = periodsOf(s).join(' and ');
    const tail = `the worst is ${worst.name} (DoS ${R.dos(worst.s.dos)}, LoS ${worst.s.losText}, ${worst.p} peak)`;
    const gs = junctionGroups(P, c.sel.existing || c.sel.base, s, ids), nOver = gs.filter(g => g.ids.some(id => over.has(id))).length;
    return over.size ? `${who}, ${nOver} of ${gs.length} assessed junctions exceed their practical degree of saturation in the ${yearOf(s)} ${pers} peaks; ${tail}. Further mitigation or a staging condition is needed at those junctions.`
      : `${who}, all ${gs.length} assessed junctions operate within practical DoS in the ${yearOf(s)} ${pers} peaks; ${tail}.`;
  }
  sec({ id: 'report-conclusion', chapter: '6', order: 10, at: '6.1', title: 'Conclusion', tag: 'mixed', check: P => !!String(rs(P).text.conclusion || '').trim(), render: (P, c) =>
    `${para(esc(conclusionText(P, c)) + ' Junction results are indicative (HCM 6 / Akcelik-based), not SIDRA output.')}${c.author('conclusion', 'State whether the effects are avoided, remedied or mitigated, and whether the proposal aligns with the transport network for all modes (AT ITA s6).')}` });

  /* ---------- Appendices ---------- */
  const KEY_LABELS = {
    sv: 'Settings version', period: 'Active period', baseYear: 'Base year', year: 'Default forecast year', growth: 'Background growth (% a year)', growthMode: 'Growth method', hvPct: 'Heavy vehicles (%)', pcuHV: 'PCU per heavy vehicle',
    peakToDaily: 'Peak hour to daily factor', bprAlpha: 'BPR α', bprBeta: 'BPR β', satFlow: 'Saturation flow (pcu/h per lane)', cycle: 'Default signal cycle (s)', lostPerPhase: 'Lost time per phase (s)', signalOpt: 'Optimise signal splits',
    maxIter: 'Maximum iterations', gapTol: 'Convergence target (relative gap)', 'conv.stable': 'TAG stability rule', 'conv.chg': 'Stability: flow change', 'conv.share': 'Stability: share of links', 'conv.consec': 'Stability: iterations', 'conv.capDamp': 'Capacity damping',
    'junction.losMethod': 'LoS method', 'junction.pff': 'Peak flow factor', 'junction.T': 'Analysis period T (h)', 'junction.queueSpace': 'Queue space (m/veh)', 'junction.f95': '95th-percentile queue factor', 'junction.capMethod': 'Junction capacity method',
    'tmdg.category': 'TMDG model category', 'tmdg.justification': 'TMDG justification', gravityBeta: 'Gravity distribution β (per minute)', connectorSpeed: 'Zone connector speed (km/h)', connectors: 'Connectors per zone',
    'turns.left': 'Left-turn penalty (s)', 'turns.right': 'Right-turn penalty (s)', 'turns.rightSignals': 'Right-turn penalty at signals (s)', 'turns.uturn': 'U-turns',
    'econ.method': 'Economics method', 'econ.vot': 'Value of time ($/veh·h)', 'econ.years': 'Analysis period (years)', 'econ.discount': 'Discount rate (uniform method, %)', 'econ.sensRate': 'Sensitivity discount rate (%)', 'econ.annualHours': 'Annualisation hours (uniform method)',
    'data.amShare': 'AM peak share of daily (%)', 'data.pmShare': 'PM peak share of daily (%)', 'data.matchRadius': 'Count matching radius (m)',
  };
  function flatten(o, pre = '', out = {}) {
    for (const [k, v] of Object.entries(o || {})) {
      const key = pre ? `${pre}.${k}` : k;
      if (v && typeof v === 'object' && !Array.isArray(v)) flatten(v, key, out); else out[key] = v;
    }
    return out;
  }
  // Every setting with its default and a non-default flag (rates are listed apart; keys and the basemap are left out)
  function settingsRows(P) {
    const S = flatten(P.settings), Dd = flatten(defaultSettings()), keys = [...new Set([...Object.keys(Dd), ...Object.keys(S)])].filter(k => !/^(rates\.|linzKey$|basemap$)/.test(k)).sort();
    const seg = o => Object.entries(o || {}).map(([k, x]) => `${k} ${x == null ? 'none' : x}`).join(', ');
    const v = x => (x === undefined ? '–' : x === null ? 'none' : Array.isArray(x) ? (x.every(e => e && typeof e === 'object') ? x.map(seg).join('; ') : x.join(', ')) : typeof x === 'object' ? seg(x) : String(x));
    return keys.map(k => ({ key: k, label: KEY_LABELS[k] || k, value: v(S[k]), def: v(Dd[k]), changed: JSON.stringify(S[k]) !== JSON.stringify(Dd[k]) }));
  }
  /* Data sources, licences and basis (about-data (f)): Kit.dataSources for the core data, then each feature's own
     rows from Settings › About the data (Registry.about; their HTML is written by the features, not by data) */
  sec({ id: 'report-app-data', chapter: 'A', order: 5, title: 'Data sources, licences and basis', tag: 'auto', render(P, c) {
    const core = typeof Kit !== 'undefined' && typeof Kit.dataSources === 'function' ? Kit.dataSources(P) : [];
    const rows = core.map(d => [esc(d.what), d.url ? `<a href="${esc(d.url)}">${esc(d.source)}</a>` : esc(d.source), esc(d.licence || '–'), esc(d.date || 'Not loaded'), esc(d.basis || '–'), esc(d.notes || '')]);
    const feat = Registry.list('about').flatMap(a => { let r = []; try { r = (typeof a.rows === 'function' ? a.rows() : a.rows) || []; } catch { r = []; } return r.map(x => [esc(a.title || a.id), ...x.slice(0, 3)]); });
    return `${c.table('Data sources, licences and basis', { head: ['Data', 'Source', 'Licence', 'Loaded', 'Basis', 'Notes'], rows, empty: 'No data sources recorded.' }, { source: 'Spare Construct data-source register (Kit.dataSources); query dates from this project\'s download log' })}
      ${feat.length ? c.table('Feature data sources', { head: ['Feature', 'Data', 'Source', 'Notes'], rows: feat }, { source: 'Settings › About the data (each feature\'s own register)' }) : ''}`;
  } });
  sec({ id: 'report-app-settings', chapter: 'A', order: 10, title: 'Model settings', tag: 'auto', render(P, c) {
    // first the settings changed from Spare Construct's defaults (Kit.nonDefaults, the same list as the assumptions CSV), then every setting
    const nd = typeof Kit !== 'undefined' && typeof Kit.nonDefaults === 'function' ? Kit.nonDefaults(P.settings) : [];
    const vt = v => (typeof Kit !== 'undefined' && Kit.valText ? Kit.valText(v) : String(v));
    const ndRows = nd.map(a => [esc(a.label), `<code>${esc(a.key)}</code>`, esc(vt(a.value)), esc(vt(a.def)), esc(a.unit || '')]);
    const rows = settingsRows(P).map(r => ({ cells: [esc(r.label), `<code>${esc(r.key)}</code>`, esc(r.value), esc(r.def), r.changed ? '<b>Changed</b>' : ''], cls: r.changed ? 'chg' : '' }));
    return `${para('These are every model setting used for the runs in this report, so a second user can reproduce them from the project file (project.corridor.json in the data pack). Values that differ from Spare Construct\'s current defaults are listed first, then every setting with the changed ones marked.')}
      ${c.table('Settings changed from Spare Construct\'s defaults (project assumptions)', { head: ['Setting', 'Key', 'Value', 'Default', 'Unit'], rows: ndRows, empty: 'Every setting is at Spare Construct\'s default. The defaults are indicative: state why they suit this project or replace them.' }, { note: 'Each change is a project assumption to agree with the reviewer at scoping. The same list is the "Assumptions" CSV export.' })}
      ${c.table('Model settings', { head: ['Setting', 'Key', 'Value', 'Default', 'Non-default'], rows })}`;
  } });
  sec({ id: 'report-app-rates', chapter: 'A', order: 20, title: 'Trip rates', tag: 'auto', render(P, c) {
    const S = P.settings, Dd = defaultSettings(), rows = Object.entries(LANDUSES).map(([k, L]) => { const r = (S.rates || {})[k] || {}, d = (Dd.rates || {})[k] || {}, same = JSON.stringify(r) === JSON.stringify(d);
      return [esc(L.label), `per ${L.per === 1 ? '' : L.per + ' '}${esc(L.unit.replace(/s$/, ''))}`, r.AM ? R.num(r.AM.rate, 2) : '–', r.AM ? R.pct(r.AM.inPct) : '–', r.PM ? R.num(r.PM.rate, 2) : '–', r.PM ? R.pct(r.PM.inPct) : '–', same ? 'Spare Construct indicative default' : '<b>User-entered</b>']; });
    return c.table('Peak-hour vehicle trip rates', { head: ['Land use', 'Unit', { label: 'AM rate', n: true }, { label: 'AM in %', n: true }, { label: 'PM rate', n: true }, { label: 'PM in %', n: true }, 'Source'], rows },
      { note: 'Spare Construct\'s defaults are indicative NZ/RTA-style rates. Replace them with surveyed rates (RTA, TDB, RR 453) and state the source and data year for a real assessment.', d: ['D13'] });
  } });
  sec({ id: 'report-app-zones', chapter: 'A', order: 30, title: 'Zones and gateways', tag: 'auto', landscape: true, render(P, c) {
    const sc = c.sel.with || c.sel.base, zs = Object.values(sc.zones);
    const zrows = zs.filter(z => z.kind !== 'gateway').map(z => [esc(z.name || z.id), esc(Engine.zoneClass(z)), esc(Object.entries(z.lu || {}).filter(([, q]) => +q > 0).map(([k, q]) => `${LANDUSES[k] ? LANDUSES[k].label : k} ${N(+q)}`).join(', ') || '–'), yearTxt(z.year), z.reduction ? R.pct(+z.reduction) : '–', z.buildout != null ? R.pct(+z.buildout) : '–', esc(z.src || 'user')]);
    const g = zs.filter(z => z.kind === 'gateway').map(z => { const w = z.gw || {}; return [esc(z.name || z.id), R.veh(w.AMin), R.veh(w.AMout), R.veh(w.PMin), R.veh(w.PMout), esc(z.gwSrc || 'Estimated by Spare Construct (no matched count)')]; });
    return `${c.table(`Zone schedule · ${esc(sc.name)}`, { head: ['Zone', 'Class', 'Land use', 'Opening year', { label: 'Trip reduction', n: true }, { label: 'Build-out', n: true }, 'Source'], rows: zrows })}
      ${c.table(`Gateway flows (base year ${yearTxt(P.settings.baseYear)}, before growth)`, { head: ['Gateway', { label: 'AM in', n: true }, { label: 'AM out', n: true }, { label: 'PM in', n: true }, { label: 'PM out', n: true }, 'Count basis'], rows: g }, { note: `veh/h. Grown to each scenario's year by ${esc(Engine.growthBasis(P.settings))}.` })}`;
  } });
  sec({ id: 'report-app-network', chapter: 'A', order: 40, title: 'Junction controls and network edits', tag: 'auto', landscape: true, render(P, c) {
    const sc = c.sel.mitigation || c.sel.with || c.sel.base, assessed = new Set(assessedSites(P, { ...c.sel, with: sc }).ids);
    // approach lanes per junction (report-skeleton: 'junction controls, lanes and timings'): lanes arriving on each leg
    const lanes = n => Net.linksAt(sc, n.id).filter(L => L.status !== 'closed').map(L => `${esc(L.name || 'Unnamed road')} ${int(L.b === n.id ? L.lanesAB : L.lanesBA)}`).join(', ') || '–';
    const nodes = Object.values(sc.nodes).filter(n => (n.control && n.control !== 'none' && n.control !== 'priority') || assessed.has(n.id)).map(n => [esc(Kit.junctionName(n.id, sc)), esc(CONTROLS[n.control] || n.control || 'Uncontrolled'), lanes(n), n.cycle ? `${int(n.cycle)} s` : '–', n.fixedTiming ? 'Fixed' : n.control === 'signals' ? 'Optimised' : '–', Array.isArray(n.bans) && n.bans.length ? `${int(n.bans.length)} banned` : '–', n.upgrade ? 'Yes' : '', assessed.has(n.id) ? 'Yes' : '']);
    const edits = P.scenarios.filter(s => s !== c.sel.base).map(s => { const d = networkDiff(c.sel.base, s); return [esc(s.name), d.length, esc(d.slice(0, 8).map(x => x.text).join('; ') + (d.length > 8 ? `; and ${d.length - 8} more` : '') || 'None')]; });
    return `${c.table(`Junction controls, lanes and timings · ${esc(sc.name)}`, { head: ['Junction', 'Control', 'Approach lanes (arriving)', 'Cycle', 'Timing', 'Turn bans', 'Upgraded', 'Assessed'], rows: nodes, empty: 'Only give-way and uncontrolled junctions, none of them assessed.' }, { note: 'Every junction with signals, a roundabout or stop control, and every assessed junction. Lanes are the lanes arriving at the junction on each leg. Other give-way junctions are set from OpenStreetMap and road hierarchy; every junction and its approaches are in junction_summary.csv and junction_movements.csv.' })}
      ${c.table(`Network edits against ${esc(c.sel.base.name)}`, { head: ['Scenario', { label: 'Changes', n: true }, 'What changes'], rows: edits, empty: 'Only one scenario.' })}`;
  } });
  function assumptionRows(P) {
    const rows = [], add = (o, group) => { if (!o || typeof o !== 'object') return; if (o.name && o.edition !== undefined && o.verified !== undefined) rows.push({ group, name: o.name, edition: String(o.edition), verified: !!o.verified, corridor: /Spare Construct assumption/i.test(String(o.edition) + (o.floorBasis || '') + (o.freightBasis || '')) }); };
    const walk = (o, g) => { for (const v of Object.values(o || {})) { if (Array.isArray(v)) v.forEach(x => add(x, g)); else if (v && typeof v === 'object') { if (v.name && v.verified !== undefined) add(v, g); else walk(v, g); } } };
    walk(Engine.STANDARDS, 'Engine');
    for (const x of [RR422, CEC, HRIG, SSM, PED]) add(x, 'Report');
    rows.push({ group: 'Report', name: 'Network walk distance factor', edition: WALK_FACTOR.basis, verified: false, corridor: true });
    rows.push({ group: 'Settings', name: 'Trip rates, peak-to-daily factor, heavy-vehicle share, value of time', edition: 'Spare Construct indicative defaults unless changed (Appendix A)', verified: false, corridor: true });
    return rows;
  }
  sec({ id: 'report-app-assumptions', chapter: 'A', order: 50, title: 'Standards and assumptions register', tag: 'auto', render(P, c) {
    const rows = assumptionRows(P).map(r => [esc(r.name), esc(r.edition), r.corridor ? '<b>Spare Construct assumption</b>' : 'Published source', r.verified ? 'Yes' : 'No']);
    return c.table('Standards and assumptions register', { head: ['Item', 'Source and edition', 'Basis', 'Checked against source'], rows }, { note: 'Every threshold Spare Construct uses is held as data with its source and edition. "Checked against source: No" means it was taken from Spare Construct\'s research spec and must be checked before it is relied on.', d: ['D13'] });
  } });
  sec({ id: 'report-app-calibration', chapter: 'B', order: 10, title: 'Calibration and validation criteria', tag: 'auto', render(P, c) {
    const sc = c.sel.existing || c.sel.base, parts = [];
    for (const p of ['AM', 'PM']) {
      if (!resOf(sc, p)) continue;
      const r = Calibrate.report(sc, p);
      parts.push(c.table(`Calibration checks · ${esc(sc.name)} · ${p} peak`, { head: ['Criterion', 'Target', 'Value', 'Status', 'Source', 'Table', 'Verified'], rows: r.checks.map(k => [esc(k.label), esc(k.target || '–'), k.value == null ? '–' : typeof k.value === 'number' ? (k.kind === 'share' ? R.pct(k.value * 100) : R.num(k.value, 3)) : esc(k.value), esc(k.status || '–'), esc(k.source || ''), esc(k.table || ''), k.verified ? 'Yes' : 'No']) },
        { note: `${esc(r.summary)} ${(r.warnings || []).map(esc).join(' ')} ${(r.notes || []).map(esc).join(' ')}` }));
      const info = P.me && P.me.info && P.me.info[p];
      if (info) parts.push(c.table(`Matrix estimation · ${p} peak`, { head: ['Item', 'Value'], rows: [['Date', esc(day(info.date))], ['Iterations', esc(info.iters)], ['Matrix total change', info.matrixChange && info.matrixChange.total != null ? R.pct(info.matrixChange.total * 100) : '–'], ['Trip-end R²', info.tripEndR2 != null ? R.num(info.tripEndR2, 3) : '–'], ['Stopped because', esc(info.stopped || '–')]] },
        { note: 'Matrix estimation used the same counts as calibration, so they are not independent validation. Agree its use with the peer reviewer.' }));
    }
    return (parts.join('') || note('Run the existing (base-year) scenario with counts loaded to report calibration.')) + disc('D6');
  } });
  sec({ id: 'report-app-convergence', chapter: 'B', order: 20, title: 'Convergence', tag: 'auto', render(P, c) {
    const rows = [], series = [];
    for (const s of P.scenarios) for (const p of periodsOf(s)) {
      const r = resOf(s, p), last = (r.conv || []).slice(-4), Pm = last.length ? Math.min(...last.map(x => x.P)) : null;
      rows.push([esc(s.name), p, int(r.it), `${N(r.gap * 100, 4)}%`, Pm != null ? R.pct(Pm * 100) : '–', r.converged === false ? '<b class="bad">No</b>' : r.converged ? 'Yes' : '–', esc(r.convNote || '')]);
      if (s === c.sel.with && r.conv && r.conv.length > 1) series.push({ pts: r.conv.filter(x => x.gap > 0).map(x => [x.it, Math.min(6, -Math.log10(x.gap))]), colour: p === 'AM' ? 'var(--accent)' : 'var(--amber)', label: `${p} −log₁₀(gap)`, yMax: 6 });
    }
    const xm = Math.max(1, ...series.flatMap(s => s.pts.map(q => q[0])));
    return `${c.table('Table C1 · Convergence', { head: ['Scenario', 'Period', { label: 'Iterations', n: true }, { label: 'Final %GAP', n: true }, { label: 'P, last 4', n: true }, 'Converged', 'Note'], rows, empty: 'No stored runs.' }, { source: `${esc(Engine.STANDARDS.conv.edition)}`, note: esc(Engine.STANDARDS.conv.note) })}
      ${series.length ? `<figure class="chart">${Kit.line(series, { xMax: xm, yMax: 6, yLabel: '10⁻⁶', xLabels: ['iteration 1', `iteration ${xm}`] })}<figcaption>Gap trace, ${esc(c.sel.with.name)} (higher is better converged)</figcaption></figure>` : ''}`;
  } });
  sec({ id: 'report-app-counts', chapter: 'B', order: 30, title: 'Count data and journey times', tag: 'auto', render(P, c) {
    const cs = D(P).counts || [], by = new Map();
    for (const k of cs) { const key = `${k.src}|${k.virtual ? 'estimate' : 'count'}|${k.role || 'calibration'}`; by.set(key, (by.get(key) || 0) + 1); }
    const rows = [...by].map(([k, n]) => { const [s, m, r] = k.split('|'); return [esc(DataHub.SOURCES[s] ? DataHub.SOURCES[s].label : s), esc(m), esc(r), n]; });
    const sc = c.sel.existing || c.sel.base, jt = (D(P).jt || []).flatMap(j => ['AM', 'PM'].map(p => {
      // the reference is an observed time where the calibration feature recorded one (j.obs), else Google's typical time (predicted)
      const r = resOf(sc, p), ob = j.obs && +j.obs[p] > 0 ? { t: +j.obs[p], src: j.obs.src || 'Observed' } : null, g = j.g && j.g[p] && +j.g[p].t > 0 ? j.g[p] : null;
      if (!r || !(ob || g)) return null;
      let mt = null, ml = null; try { const t = sc.nodes[j.from] && sc.nodes[j.to] ? Engine.travel(sc, r, j.from, j.to) : null; if (t && !t.none) { mt = t.time; ml = t.len; } } catch { }
      const ref = ob ? ob.t : +g.t, k = mt != null ? DataHub.jtCheck(mt, ref) : null, len = g && g.len ? g.len : ml;
      return [esc(j.name), p, len ? R.num(len / 1000, 2) : '–', R.num(ref), ob ? `${esc(ob.src)} (observed)` : `Google Routes typical time (predicted, not observed${g.when ? ', ' + esc(g.when) : ''})`, mt != null ? R.num(mt) : '–', k ? R.pct(k.pct * 100) : '–', k ? (k.tier1 ? 'Pass' : 'Fail') : '–', k ? (k.tier2 ? 'Pass' : 'Fail') : '–'];
    })).filter(Boolean);
    return `${c.table('Counts by source, method and role', { head: ['Source', 'Method', 'Role', { label: 'Sites', n: true }], rows, empty: 'No counts loaded.' }, { note: 'Every count with its calibration result is in counts_calibration.csv in the data pack.' })}
      ${c.table('Table J1 · Journey times', { head: ['Route', 'Period', { label: 'Length km', n: true }, { label: 'Reference s', n: true }, 'Reference source', { label: 'Modelled s', n: true }, { label: 'Δ %', n: true }, 'Tier 1', 'Tier 2'], rows: jt, empty: 'No journey-time routes with an observed or Google reference time.' }, { note: `${esc((DataHub.JT_CRITERIA || {}).text || '')} Google typical times are predicted, not observed: they support but do not replace surveyed journey times.` })}`;
  } });
  sec({ id: 'report-app-movements', chapter: 'C', order: 10, title: 'Movement summaries by approach', tag: 'auto', landscape: true, render(P, c) {
    const parts = [];
    for (const sc of [c.sel.without, c.sel.with, c.sel.mitigation].filter(Boolean)) {
      const rows = [];
      for (const x of junctionRows(P, c, sc)) {
        if (!x.nr) continue;
        rows.push({ cells: [`<b>${esc(x.name)}</b>`, x.p, `<b>${esc(CONTROLS[x.nr.type] || x.nr.type)}</b>`, R.veh(x.nr.vol), '', `<b>${R.dos(x.nr.dos)}</b>`, `<b>${R.delay(jSum(x.nr).delay)}</b>`, `<b>${esc(x.nr.losText || x.nr.los)}</b>`, '', '', '', ''], cls: 'tot' });
        for (const a of x.nr.appr) {
          const L = sc.links[a.link], st = L ? Net.len(sc, L) : null;
          rows.push(['', '', esc(`${a.approach || ''}: ${a.name || ''}`), R.veh(a.v), R.veh(a.cap), R.dos(a.dos), R.delay(a.delay), esc(a.los || '–'), a.q95veh != null ? R.num(a.q95veh, 1) : '–', R.q(a.q95), a.psc != null ? R.pct(a.psc) : '–', st && a.q95 ? R.num(a.q95 / st, 2) : '–']);
        }
      }
      parts.push(c.table(`Movement summary by approach · ${esc(sc.name)}`, { head: ['Junction', 'Period', 'Approach', { label: 'Demand veh/h', n: true }, { label: 'Capacity veh/h', n: true }, { label: 'DoS', n: true }, { label: 'Delay s', n: true }, 'LoS', { label: 'Q95 veh', n: true }, { label: 'Q95 m', n: true }, { label: 'PSC %', n: true }, { label: 'Storage ratio', n: true }], rows, empty: 'No junction results.' },
        { junction: true, note: `Approach rows for the assessed junctions; bold rows are the junction as a whole. ${Registry.get('report', 'junctions-movement-summary') ? 'SIDRA-style movement summaries by turn for the key junctions follow in this appendix. ' : ''}Turn-level movements (Mov ID, turn designation) for every junction are in turning_movements.csv and the SIDRA volumes file.` }));
    }
    return parts.join('') || note('No junction results.');
  } });
  sec({ id: 'report-app-crashes', chapter: 'D', order: 10, title: 'Crash listing', tag: 'auto', landscape: true, render(P, c) {
    const cr = (D(P).crashes || []).slice().sort((a, b) => (b[3] - a[3]) || 'FSMN'.indexOf(a[2]) - 'FSMN'.indexOf(b[2]));
    const sev = { F: 'Fatal', S: 'Serious', M: 'Minor', N: 'Non-injury' };
    const rows = cr.map(k => [yearTxt(k[3]), sev[k[2]] || esc(k[2]), esc(k[4] || ''), int(k[6]), esc([...(k[5] || '')].map(f => ({ p: 'pedestrian', c: 'cyclist', m: 'motorcyclist' })[f]).join(', ') || '–'), `${R.num(k[0], 5)}, ${R.num(k[1], 5)}`]);
    return c.table(`Reported crashes (${plural(cr.length, 'crash', 'crashes')})`, { head: ['Year', 'Severity', 'Location', { label: 'Speed limit', n: true }, 'Vulnerable users', 'Lat, lng'], rows, empty: 'No crashes loaded.' }, { source: esc(ATTRIB.cas), d: ['D8'] });
  } });
  sec({ id: 'report-app-anop', chapter: 'E', order: 10, title: 'ANOP tool inputs', tag: 'auto', fallback: true, render: () => `${note('The ANOP tool inputs (anop.csv) are produced by the ANOP feature when it is installed. None are recorded in this project.')}${disc('D5')}` });
  sec({ id: 'report-app-datapack', chapter: 'F', order: 10, title: 'Data pack manifest', tag: 'auto', render(P, c) {
    const rows = Object.entries(COLS).map(([f, cols]) => [`<code>${esc(f)}</code>`, esc(FILES[f] || ''), cols.map(([k, u]) => `<code>${esc(k)}</code> ${esc(u)}`).join('; ')]);
    return `${para(`The ITA data pack (Report › Download the data pack) holds every table of this report as UTF-8 CSV, with the network, settings and project file: <code>${esc(packName(P))}</code>.`)}
      ${c.table('Data pack files and columns', { head: ['File', 'Contents', 'Columns (units)'], rows })}${disc('D12', { tools: 'SIDRA / SATURN / Aimsun / AT ANOP tool / NZTA IER tool / MBCM workbook' })}`;
  } });
  sec({ id: 'report-app-expert', chapter: 'G', order: 10, title: 'Expert disclosure and sign-off', tag: 'mixed', check: P => signedOff(P), render(P, c) {
    const so = rs(P).signoff || {}, d = rs(P).details || {};
    return `${disc('D2')}${para('Assumptions are listed in Appendix A (standards and assumptions register), methods in section 4.2, uncertainty and sensitivity in section 4.6, and limitations in the front matter.')}
      ${c.author('expert', 'State your qualifications and experience, the data and assumptions relied on, the uncertainty in the results, and your confidence in them (Practice Note 2023 s9.3).')}
      ${c.table('Sign-off', { head: ['Role', 'Name', 'Date', 'Signature'], rows: [['Author', esc(d.author || ''), esc(d.date || ''), ''], ['Reviewer', esc(so.reviewer || ''), esc(so.date || ''), so.reviewed ? 'Reviewed (ticked in Spare Construct)' : ''], ['Approved', esc(so.approver || ''), '', '']] })}
      <h3>Standing disclaimers (version ${esc(DISCLAIMERS.version)})</h3>${Object.keys(DISCLAIMERS.items).map(id => disc(id, { version: REG.asAt })).join('')}`;
  } });

  // Register Spare Construct's sections; render fills a partial ctx (the smoke test calls render(P, {}))
  const OWN = new Set();
  for (const s of S_) {
    const render = (P, ctx) => withProject(P, () => s.render(P, context(P || Store.P, ctx || {})));
    OWN.add(render); if (s.at) AT_NUM.set(s.id, s.at);
    Registry.report({ ...s, render });
  }

  /* ======================= assembling the document ======================= */
  const escRe = s => String(s).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  // A CSS string literal. '<' and '>' become CSS escapes, so a scenario name such as '</style>…' cannot close the
  // <style> element (the report window shares the app's origin); control characters are dropped.
  const cssStr = s => '"' + String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/[\r\n\f]+/g, ' ').replace(/[\u0000-\u001f\u007f]/g, '')
    .replace(/</g, '\\3C ').replace(/>/g, '\\3E ').replace(/&/g, '\\26 ') + '"';
  // Kit.table output (and other sections' tables) gets a <thead> so headers repeat on every printed page
  function fixTables(h) {
    return h.replace(/<table\b[^>]*>[\s\S]*?<\/table>/g, t => {
      if (/<thead/i.test(t)) return t;
      const m = /^(<table\b[^>]*>)\s*(<tr\b[^>]*>(?:(?!<\/tr>)[\s\S])*?<th\b[\s\S]*?<\/tr>)([\s\S]*)<\/table>$/.exec(t);
      return m ? `${m[1]}<thead>${m[2]}</thead><tbody>${m[3]}</tbody></table>` : t;
    });
  }
  /* Table and figure numbers in another feature's section: a bare <table> takes the heading just above it (or the
     section title) as its caption, and a <figcaption> without a number gets one, so every table and figure in the
     report is numbered by chapter. Tables already captioned by ctx.table (%%TNO…%%) are left alone. */
  function numberForeign(body, title) {
    const parts = body.split(/(<table\b[^>]*>|<\/table>|%%TNO(?::[^%]*)?%%)/);
    let out = '', depth = 0, pending = false;
    for (const p of parts) {
      if (/^%%TNO/.test(p)) { pending = true; out += p; continue; }
      if (/^<table\b/.test(p)) {
        if (depth++ === 0 && !pending) {
          const m = /<(h[2-5])([^>]*)>((?:(?!<\/?h[1-6][\s>])[\s\S])*?)<\/\1>((?:\s|<div\b[^>]*>|<p class="note[^"]*">(?:(?!<\/?p[\s>])[\s\S])*?<\/p>)*)$/.exec(out);
          if (m && !/class="tno"/.test(m[3])) out = out.slice(0, m.index) + `<${m[1]}${m[2]}><span class="tno">${tnoToken(m[3])}</span>${tnoRest(m[3])}</${m[1]}>${m[4]}`;
          else out += `<p class="tcap"><span class="tno">%%TNO%%</span>${esc(title || 'Table')}</p>`;
        }
        pending = false; out += p; continue;
      }
      if (p === '</table>') { depth = Math.max(0, depth - 1); out += p; continue; }
      out += p;
    }
    return out.replace(/<figcaption([^>]*)>(?!\s*<span class="tno">)/g, '<figcaption$1><span class="tno">%%FNO%%</span>');
  }
  // 'Table E1 · Road network' → the placeholder carries the outline's table id: 'Table 2.3 (E1): Road network'
  const TID = /^\s*Table\s+([A-Z][A-Za-z0-9-]*)\s*·\s*/;
  const tnoToken = title => { const m = TID.exec(String(title || '')); return m ? `%%TNO:${m[1]}%%` : '%%TNO%%'; };
  const tnoRest = title => String(title || '').replace(TID, '');
  const depthOf = num => String(num).split('.').length;
  /* Numbers in chapters 1–6 mirror AT ITA Guidelines 2025 v2: Spare Construct's own sections keep their fixed numbers
     (2.7 Crash history, 4.3 Road user safety …) whatever other features add. Another feature's section is numbered
     as a sub-section of the Spare Construct section before it ('4.2.3.1'); with an order of 100 or more it becomes a numbered
     section after Spare Construct's ('4.8'). A sub-section with the same title as its parent is printed under the parent's
     heading. A Spare Construct placeholder (fallback: true) with sub-sections gives its number to the first of them.
     Appendices and annexes are numbered in order (A1, A2 …). → [{ d, body, num, level, merged }] */
  function numberChapter(ch, done) {
    const out = [];
    if (ch.app || !ch.num) {
      const filled = done.some(x => x.body && !x.d.fallback && !/^report-/.test(x.d.id));
      let k = 0;
      for (const x of done) {
        if (!x.body || (x.d.fallback && filled)) continue;
        const numbered = !!ch.num && !!x.d.title && x.d.numbered !== false;
        out.push({ ...x, num: numbered ? `${ch.num}${ch.app ? '' : '.'}${++k}` : '', level: 2 });
      }
      return out;
    }
    const tops = done.filter(x => AT_NUM.has(x.d.id) && x.body).map(x => +AT_NUM.get(x.d.id).split('.')[1] || 0);
    let top = Math.max(0, ...tops), parent = null;
    const same = (a, b) => String(a || '').trim().toLowerCase() === String(b || '').trim().toLowerCase();
    for (const x of done) {
      if (!x.body) continue;
      const at = AT_NUM.get(x.d.id);
      if (at) { parent = { ...x, num: at, level: depthOf(at) > 2 ? 3 : 2, kids: [] }; out.push(parent); continue; }
      if ((x.d.order ?? 1000) >= 100) { parent = null; out.push({ ...x, num: x.d.title ? `${ch.num}.${++top}` : '', level: 2 }); continue; }
      if (!parent) { out.push({ ...x, num: '', level: 2 }); continue; }
      const kid = { ...x, level: 3 };
      if (same(x.d.title, parent.d.title) && !parent.d.fallback) { kid.merged = true; kid.num = ''; }
      parent.kids.push(kid); out.push(kid);
    }
    // a placeholder with sub-sections: the first sub-section takes its number, the others follow under it
    for (const p of out.filter(x => x.kids)) {
      const kids = p.kids.filter(k => !k.merged);
      if (p.d.fallback && kids.length) { p.drop = true; kids.forEach((k, i) => { k.num = i ? `${p.num}.${i}` : p.num; k.level = i ? 3 : p.level; }); continue; }
      kids.forEach((k, i) => { k.num = `${p.num}.${i + 1}`; });
    }
    return out.filter(x => !x.drop);
  }
  function build(P, opts = {}) {
    const ctx = context(P, opts), defs = Registry.list('report'), groups = new Map(CHAPTERS.map(ch => [ch.id, []]));
    for (const d of defs) {
      if (MERGED[d.id] && Registry.get('report', MERGED[d.id])) continue;   // printed inside its host section
      (groups.get(chapterOf(d.chapter, d.title)) || groups.get('4')).push(d);
    }
    const chapters = [], toc = [];
    for (const ch of CHAPTERS) {
      const list = groups.get(ch.id); if (!list.length) continue;
      // chapters 1–6: on an equal order Spare Construct's own section comes first, so another feature's section follows it
      if (ch.num && !ch.app) list.sort((a, b) => ((a.order ?? 1000) - (b.order ?? 1000)) || ((AT_NUM.has(b.id) ? 1 : 0) - (AT_NUM.has(a.id) ? 1 : 0)));
      ctx.chapter = ch; ctx.counters = { T: 0, F: 0 };
      const parts = [];
      const done = list.map(d => { ctx.section = { id: d.id, num: '', title: d.title || '' }; const body = Registry.html(d, 'render', P, ctx); return { d, body: body && body.trim() ? body : '' }; });
      for (const x of numberChapter(ch, done)) {
        const d = x.d, num = x.num, own = OWN.has(d.render);
        let body = x.body;
        if (d.title) body = body.replace(new RegExp(`^\\s*<h[1-4][^>]*>\\s*${escRe(esc(d.title))}\\s*</h[1-4]>`), '');
        // numbered tables and figures: Spare Construct's come captioned (ctx.table, ctx.figure); other features' are captioned here
        if (ch.num) body = own ? body.replace(/<figcaption([^>]*)>(?!\s*<span class="tno">)/g, '<figcaption$1><span class="tno">%%FNO%%</span>') : numberForeign(body, d.title);
        // provenance: a table from another feature without a source line says where it comes from
        if (!own && /<table/.test(body) && !/class="src"|\bSource:/i.test(body)) body += `<p class="src">Source: Spare Construct's ${esc(String(d.id).split('-')[0])} feature, from this project's inputs, loaded data and model runs (data vintage in the front matter; inputs in Appendix A).</p>`;
        // reg-disclaimers: every junction table carries D3, including tables from other features
        if (/<table/.test(body) && /\bDoS\b/.test(body) && /\bLoS\b/.test(body) && !/data-d="D3"/.test(body)) body += disc('D3');
        // … D4 on every economic table, D5 on every ANOP table and D13 wherever a 'Spare Construct assumption' is used
        const heads = (body.match(/<th\b[^>]*>[\s\S]*?<\/th>/g) || []).join(' ');
        if (/\bBCR\b/.test(heads) && !/data-d="D4"/.test(body)) body += disc('D4');
        if (/\bANOP\b/.test(heads) && !/data-d="D5"/.test(body)) body += disc('D5');
        if (/Spare Construct assumption/i.test(body) && !/data-d="D13"/.test(body)) body += disc('D13');
        const pre = ch.num ? `${ch.num}${ch.app ? '' : '.'}` : '';
        body = body.replace(/%%TNO(?::([^%]*))?%%/g, (m, id) => (ch.num ? `Table ${pre}${++ctx.counters.T}${id ? ` (${esc(id)})` : ''}: ` : ''))
          .replace(/%%FNO%%/g, () => (ch.num ? `Figure ${pre}${++ctx.counters.F}: ` : 'Figure: '));
        const tag = d.tag ? `<span class="tag noprint">${esc(d.tag)}</span>` : '';
        const shown = ch.app && d.title ? d.title.replace(/^(appendix|annex)\s+[a-z]\s*[:·.–-]\s*/i, '') : d.title;
        const hl = x.level === 3 ? 'h3' : 'h2';
        const head = d.title && !x.merged ? `<${hl} class="sec-h${x.level === 3 ? ' sub' : ''}" id="s-${esc(d.id)}">${num ? `<span class="no">${esc(num)}</span> ` : ''}${esc(shown)}${tag}</${hl}>` : '';
        if (d.title && !x.merged && d.numbered !== false && d.id !== 'report-contents') toc.push({ ch: ch.id, num, title: shown, id: d.id, level: x.level });
        parts.push(`<section class="sec${d.landscape ? ' landscape' : ''}${d.id === 'report-cover' ? ' cover-sec' : ''}${x.merged ? ' merged' : ''}" data-sec="${esc(d.id)}">${head}${body}</section>`);
      }
      if (!parts.length) continue;
      const title = ch.id === 'front' ? '' : `<h1 class="ch" id="ch-${esc(ch.id)}">${ch.app ? (ch.id === 'P' ? 'Annex' : 'Appendix') + ' ' : ''}${esc(ch.num)} ${esc(ch.title)}</h1>`;
      chapters.push({ ch, html: `<div class="chapter${ch.id === 'front' ? ' front' : ''}" data-ch="${esc(ch.id)}">${title}${parts.join('')}</div>` });
    }
    const contents = `<ol class="toc">${CHAPTERS.filter(ch => ch.id !== 'front' && chapters.some(x => x.ch === ch)).map(ch => `<li><a href="#ch-${esc(ch.id)}"><b>${ch.app ? (ch.id === 'P' ? 'Annex' : 'Appendix') + ' ' : ''}${esc(ch.num)} ${esc(ch.title)}</b></a><ol>${toc.filter(t => t.ch === ch.id).map(t => `<li${t.level === 3 ? ' class="sub"' : ''}><a href="#s-${esc(t.id)}">${esc(t.num)} ${esc(t.title)}</a></li>`).join('')}</ol></li>`).join('')}</ol>`;
    const body = fixTables(chapters.map(x => x.html).join('').replace('%%CONTENTS%%', contents));
    return { body, ctx, requests: ctx.requests, chapters: chapters.map(x => x.ch.id), toc };
  }
  const CSS = `
:root{--ink:#18201e;--muted:#56635f;--faint:#6d7b77;--line:#c9d1cd;--line-2:#e4e9e6;--panel:#ffffff;--panel-2:#f3f5f4;--accent:#0d6a4d;--amber:#b77900;--danger:#b42318;--warn:#a15c07;--ok:#1b7a46;
--los-a:#1b8a4f;--los-b:#73ad21;--los-c:#d4ab00;--los-d:#ef8a00;--los-e:#e2531f;--los-f:#b0182a;--bg:#ffffff}
@media not all{:root:not([data-theme=light]){--ink:#e5ebe8;--muted:#9caba6;--faint:#7d8b87;--line:#38433f;--line-2:#29322f;--panel:#161c1b;--panel-2:#1d2523;--accent:#3db08a;--amber:#f2b43c;--danger:#f97066;--warn:#fdb022;--ok:#4cc38a;--bg:#0e1312}}
@page{size:A4;margin:18mm 15mm 24mm 15mm;@bottom-left{content:var(--foot-l);font:7.5pt 'Segoe UI',Arial,sans-serif;color:#56635f}@bottom-right{content:"Page " counter(page) " of " counter(pages);font:7.5pt 'Segoe UI',Arial,sans-serif;color:#56635f}}
@page landscape{size:A4 landscape;margin:15mm 18mm 18mm 18mm}
*{box-sizing:border-box}html{background:var(--bg)}body{margin:0;background:var(--bg);color:var(--ink);font:10pt/1.45 'Segoe UI',Barlow,Arial,sans-serif}
main{max-width:200mm;margin:0 auto;padding:16px}
.landscape{page:landscape}@media screen{.landscape{max-width:none}}
h1.ch{font-size:20pt;margin:0 0 10px;padding-bottom:6px;border-bottom:2px solid var(--accent);color:var(--accent)}
h2{font-size:13.5pt;margin:18px 0 6px}h2 .no,h3 .no{color:var(--accent);margin-right:4px}h3{font-size:11pt;margin:14px 0 4px}h3.sec-h{font-size:12pt;margin:16px 0 5px}
p.tcap{font-weight:600;font-size:9pt;margin:8px 0 3px}.toc li.sub{margin-left:16px;font-size:9.5pt}
.chapter{break-before:page}.chapter.front{break-before:auto}h1,h2,h3,figcaption{break-after:avoid}
p{margin:6px 0}.muted{color:var(--muted)}.warn{color:var(--warn)}.bad{color:var(--danger)}.ok{color:var(--ok)}
.note{color:var(--muted);font-size:9pt}.note.warn{color:var(--warn)}
table.rt{border-collapse:collapse;width:100%;font-size:8.5pt;margin:4px 0}table.rt th{background:var(--panel-2);text-align:left;font-weight:600;border-bottom:1.5px solid var(--line);padding:3px 5px;vertical-align:bottom}
table.rt td{border-bottom:1px solid var(--line-2);padding:2.5px 5px;vertical-align:top}table.rt .n{text-align:right;font-variant-numeric:tabular-nums}
table.rt tr.tot td{background:var(--panel-2)}table.rt tr.ytd td{color:var(--muted);font-style:italic}table.rt tr.miss td{background:rgba(180,35,24,.07)}table.rt tr.chg td{background:rgba(227,154,0,.08)}
table.t{border-collapse:collapse;width:100%;font-size:8.5pt}table.t th,table.t td{border-bottom:1px solid var(--line-2);padding:2.5px 5px;text-align:left}table.t th{background:var(--panel-2)}table.t .n,table.t td.n{text-align:right}
thead{display:table-header-group}tr,figure,.disc,.author{break-inside:avoid}figure{margin:10px 0}
figure.tbl{break-inside:auto}figure.tbl figcaption,figure.fig figcaption{font-weight:600;font-size:9pt;margin:2px 0 4px}.tno{color:var(--accent)}
figure.fig img{width:100%;height:auto;border:1px solid var(--line);display:block}.fig-ph{border:1px dashed var(--line);padding:30px 14px;text-align:center;color:var(--muted);font-size:9pt;min-height:60mm;display:flex;align-items:center;justify-content:center}
.src{font-size:7.5pt;color:var(--muted);margin:2px 0}.sup{font-size:8.5pt;font-weight:600;color:var(--warn);margin:2px 0 4px}.disc{font-size:7.8pt;color:var(--muted);border-left:3px solid var(--line);padding:3px 8px;margin:6px 0}
.author{border-left:3px solid var(--accent);padding:2px 10px;margin:8px 0}.author.empty{border-left-color:var(--amber);background:rgba(227,154,0,.08);color:var(--warn);font-size:9pt;padding:6px 10px}
.tag{font-size:7pt;font-weight:500;text-transform:uppercase;letter-spacing:.05em;color:var(--muted);border:1px solid var(--line);border-radius:3px;padding:0 4px;margin-left:8px;vertical-align:middle}
.lv{display:inline-block;padding:0 4px;border-radius:3px;font-size:8pt}.lv.on{outline:1.5px solid var(--accent)}.lv-P{font-weight:700}.lv-U{color:var(--muted)}
.cover{min-height:240mm;display:flex;flex-direction:column;justify-content:center}.kicker{text-transform:uppercase;letter-spacing:.12em;color:var(--accent);font-weight:600}
.title{font-size:28pt;line-height:1.1;margin:8px 0}.subtitle{font-size:14pt;color:var(--muted);margin-bottom:18px}
table.meta{border-collapse:collapse;margin:12px 0;font-size:10pt}table.meta th{text-align:left;padding:3px 16px 3px 0;color:var(--muted);font-weight:500}table.meta td{padding:3px 0}
.stamp{font-weight:700;margin:12px 0}.stamp.draft{color:var(--danger)}.stamp.ok{color:var(--ok)}
.cover-sec{break-after:page}.toc{padding-left:18px}.toc ol{padding-left:18px;margin:2px 0 6px}.toc a{color:inherit;text-decoration:none}
ul.bul{margin:6px 0;padding-left:20px}ul.bul li{margin:3px 0}code{font-family:Consolas,'IBM Plex Mono',monospace;font-size:8pt}
.rp-watermark{position:fixed;top:38%;left:0;right:0;text-align:center;font-size:120pt;font-weight:800;letter-spacing:.1em;color:rgba(180,35,24,.09);transform:rotate(-28deg);pointer-events:none;z-index:0}
.bar{position:sticky;top:0;z-index:5;display:flex;gap:8px;align-items:center;padding:8px 16px;background:var(--panel-2);border-bottom:1px solid var(--line);font-size:9.5pt}
.bar button,.copy{font:inherit;font-size:8.5pt;padding:3px 10px;border:1px solid var(--line);border-radius:5px;background:var(--panel);color:var(--ink);cursor:pointer}.bar button.primary{background:var(--accent);color:#fff;border-color:var(--accent)}
.copy{float:right;font-size:7.5pt;padding:1px 6px}.doc-foot{margin-top:24px;padding-top:8px;border-top:1px solid var(--line);font-size:7.5pt;color:var(--muted)}.doc-foot p{margin:3px 0}.toast{position:fixed;bottom:16px;left:50%;transform:translateX(-50%);background:var(--ink);color:var(--bg);padding:8px 14px;border-radius:6px;font-size:9pt}
.mini-chart{width:100%;font-size:9px}.mini-chart text{fill:var(--muted)}figure.chart figcaption{font-size:8.5pt;color:var(--muted)}.hud-row{display:flex;gap:12px}
@media print{.noprint,.bar,.copy,.toast{display:none!important}main{max-width:none;padding:0}body{font-size:9.5pt}a{color:inherit;text-decoration:none}}`;
  // Runs inside the report window: Print (after every image has decoded), Download HTML and Copy table
  const DOC_SCRIPT = `(function(){
  function toast(t){var d=document.createElement('div');d.className='toast';d.textContent=t;document.body.appendChild(d);setTimeout(function(){d.remove()},3200)}
  function rows(t){return Array.prototype.map.call(t.rows,function(r){return Array.prototype.map.call(r.cells,function(c){return (c.innerText||c.textContent||'').replace(/[\\t\\r\\n]+/g,' ').trim()}).join('\\t')}).join('\\r\\n')}
  document.addEventListener('click',function(e){
    var c=e.target.closest('[data-copy]');
    if(c){var t=c.closest('figure').querySelector('table');if(!t)return;var s=rows(t),n=t.tBodies[0]?t.tBodies[0].rows.length:t.rows.length,done=function(){toast('Copied '+n+' rows. Paste into Excel or Word.')};
      try{if(window.ClipboardItem&&navigator.clipboard&&navigator.clipboard.write){navigator.clipboard.write([new ClipboardItem({'text/html':new Blob([t.outerHTML],{type:'text/html'}),'text/plain':new Blob([s],{type:'text/plain'})})]).then(done,function(){navigator.clipboard.writeText(s).then(done)})}else{navigator.clipboard.writeText(s).then(done)}}catch(err){toast('Your browser did not allow copying. Select the table and copy it instead.')}return}
    if(e.target.closest('[data-print]')){Promise.all(Array.prototype.map.call(document.images,function(i){return i.decode?i.decode().catch(function(){}):null})).then(function(){requestAnimationFrame(function(){window.print()})});return}
    if(e.target.closest('[data-save]')){var h='<!doctype html>\\n'+document.documentElement.outerHTML,a=document.createElement('a');a.href=URL.createObjectURL(new Blob([h],{type:'text/html'}));a.download=document.body.getAttribute('data-file')||'ita-report.html';document.body.appendChild(a);a.click();setTimeout(function(){URL.revokeObjectURL(a.href);a.remove()},1000)}
  });
})();`;
  const fileBase = P => String(P.name || 'project').replace(/[^\w\- ]+/g, '').trim().replace(/\s+/g, '-').toLowerCase() || 'project';
  // ASCII file-name part: macrons and other accents dropped ('Ōtāhuhu Road' → 'otahuhu-road')
  const slug = s => String(s ?? '').normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 40).replace(/-+$/, '');
  const ymd = (d = new Date()) => d.toISOString().slice(0, 10).replace(/-/g, '');
  const reportName = P => `${fileBase(P)}-ITA-report-${ymd()}.html`;
  const packName = P => `${fileBase(P)}-ITA-datapack-${ymd()}.zip`;

  /* The report document is static: its one script is DOC_SCRIPT, allowed by hash, and nothing else may run, load or
     post (defence in depth: the report window shares the app's origin, and every value is escaped as well). */
  const DOC_SCRIPT_HASH = 'sha256-PlIsln8wxWqGEn3AucRO7R1kNQXRifIwv7ua/46Olvw=';   // test/f-report.test.js checks it against DOC_SCRIPT
  const CSP = `default-src 'none'; img-src data: blob:; style-src 'unsafe-inline'; script-src '${DOC_SCRIPT_HASH}'; base-uri 'none'; form-action 'none'`;
  /* html(P, { mapImages: { figureId: dataURL | { src, attribution } }, period }) → a complete standalone HTML document */
  function html(P = Store.P, opts = {}) {
    return withProject(P, () => doc(P, build(P, opts)));
  }
  function doc(P, b) {
    {
      const sel = b.ctx.sel, draft = !signedOff(P);
      const runs = P.scenarios.flatMap(s => periodsOf(s).map(p => resOf(s, p).ts)).filter(Boolean), ts = runs.length ? new Date(Math.max(...runs)) : null;
      const foot = `${D1_SHORT} · ${modelVersion(P)} · run ${ts ? ts.toISOString().slice(0, 16).replace('T', ' ') : 'not run'} · ${sel.with ? sel.with.name : ''}${draft ? ' · DRAFT' : ''}`;
      // the regulatory version stamp (pathway, regime and the editions relied on) goes in every page footer under D1
      const vs = stampFooter(P), why = blockedWhy(P);
      const footCss = `${cssStr(foot)}${vs ? ` "\\A" ${cssStr(vs)}` : ''}`;
      // The scope linter's blocking finding (ComplianceExtra.exportBlock) stops printing and saving inside the report
      // window too: the buttons are disabled and a printout carries only the reason
      const bar = why
        ? `<button class="primary" type="button" disabled title="${esc(why)}">Print or save as PDF</button><button type="button" disabled title="${esc(why)}">Download HTML</button><span class="warn"><b>Printing and saving are blocked.</b> ${esc(why)} Fix it in the Compliance+ tab (scope linter), then open the report again.</span>`
        : `<button class="primary" data-print type="button">Print or save as PDF</button><button data-save type="button">Download HTML</button><span class="muted">${draft ? 'DRAFT: a reviewer has not signed this report off yet. ' : ''}In the print dialog choose A4 and turn on background graphics. Print from Chrome or Edge 131 or later: other browsers leave out the page footer that carries Disclaimer D1, the version stamp and the page numbers (both are also printed at the end of the report).</span>`;
      const blockCss = why ? `@media print{main,.rp-watermark{display:none!important}body::before{content:${cssStr('Printing is blocked: ' + why)};display:block;padding:20mm;font:12pt Arial,sans-serif}}` : '';
      return `<!doctype html>
<html lang="en-NZ"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${CSP}"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(P.name)} · Integrated Transport Assessment${draft ? ' (DRAFT)' : ''}</title>
<meta name="generator" content="${esc(modelVersion(P))}"><style>${CSS}
:root{--foot-l:${cssStr(foot)}}@page{@bottom-left{content:${footCss};white-space:pre-wrap;font-size:6.5pt}}${blockCss}</style></head>
<body data-file="${esc(reportName(P))}" data-draft="${draft ? 1 : 0}"${why ? ' data-blocked="1"' : ''}>
<div class="bar noprint">${bar}</div>
${draft ? '<div class="rp-watermark" aria-hidden="true">DRAFT</div>' : ''}
<main>${b.body}<footer class="doc-foot"><p>${esc(D1_SHORT)}. ${esc(dText('D1'))}</p>${vs ? `<p>${esc(vs)}</p>` : ''}<p>${esc(modelVersion(P))}${draft ? ' · DRAFT' : ''}</p></footer></main>
<script>${DOC_SCRIPT}</script></body></html>`;
    }
  }
  // Compliance.stamp(P).footer ('Version stamp: …'), or '' without the compliance feature or a pathway
  function stampFooter(P) { try { return typeof Compliance !== 'undefined' && typeof Compliance.stamp === 'function' ? String(Compliance.stamp(P).footer || '') : ''; } catch { return ''; } }
  // The scope linter's blocking finding, or null (no compliance-extra feature, or nothing blocking)
  function blockedWhy(P) { try { return typeof ComplianceExtra !== 'undefined' && typeof ComplianceExtra.exportBlock === 'function' ? ComplianceExtra.exportBlock(P) || null : null; } catch { return null; } }
  // The figures the sections ask for (a dry run), drawn with Render.snapshot and re-encoded as JPEG
  const toJpeg = url => new Promise(res => {
    try {
      const im = new Image();
      im.onload = () => { try { const cv = document.createElement('canvas'); cv.width = im.naturalWidth; cv.height = im.naturalHeight; const g = cv.getContext('2d'); g.fillStyle = '#ffffff'; g.fillRect(0, 0, cv.width, cv.height); g.drawImage(im, 0, 0); res(cv.toDataURL('image/jpeg', 0.86)); } catch { res(url); } };
      im.onerror = () => res(url); im.src = url;
    } catch { res(url); }
  });
  let figCache = { key: '', images: {} };
  const canDraw = () => typeof Render !== 'undefined' && Render.snapshot && typeof UI !== 'undefined' && UI.map;
  async function figures(P = Store.P, opts = {}) {
    if (!canDraw()) return {};
    return drawFigures(P, withProject(P, () => build(P, { ...opts, mapImages: {} }).requests), opts);
  }
  // Draws the requested figures (kept for the same project version, so a second open does not redraw them)
  async function drawFigures(P, reqs, opts = {}) {
    if (!canDraw()) return {};
    const out = {}, key = `${P.id}|${Store.ver}`, sigs = {};
    // a figure is reused only for the same project version and the same request (scenario, compared scenario, mode, period)
    const sig = f => JSON.stringify([f.mode, f.scenarioId, f.baseScenarioId || '', f.period || '', f.bounds || null, f.show || null]);
    for (let i = 0; i < reqs.length; i++) {
      const f = reqs[i];
      sigs[f.id] = sig(f);
      if (figCache.key === key && figCache.images[f.id] && (figCache.sigs || {})[f.id] === sigs[f.id]) { out[f.id] = figCache.images[f.id]; continue; }
      (opts.onStatus || busy)(`Drawing report maps: ${i + 1} of ${reqs.length}…`);
      try {
        const url = await Render.snapshot({ bounds: f.bounds || undefined, width: f.width || 760, height: f.height || 500, mode: f.mode, scenarioId: f.scenarioId, baseScenarioId: f.baseScenarioId || undefined, period: f.period, basemap: true, title: f.title, show: { counts: false, ...(f.show || {}) }, labelRound: 10, scale: 2 });
        const ls = Render.lastSnapshot || {};
        out[f.id] = { src: await toJpeg(url), attribution: ls.attribution || '' };
      } catch (e) { out[f.id] = { error: e.message }; }
    }
    busy(null);
    figCache = { key, images: out, sigs };
    return out;
  }
  // Opens the report in a new window, ready to print or save as PDF. The window opens at once (pop-up blockers).
  async function open(P = Store.P, opts = {}) {
    const w = typeof window !== 'undefined' && window.open ? window.open('', '_blank') : null;
    if (!w) { toast('Your browser blocked the report window. Allow pop-ups for this site, or use Report › Download HTML.', 'warn'); return null; }
    try { w.document.write('<!doctype html><meta charset="utf-8"><title>Preparing the report…</title><p style="font:16px system-ui,sans-serif;padding:24px">Preparing the ITA report and drawing its maps…</p>'); } catch { }
    try { w.opener = null; } catch { }
    try {
      const h = await built(P, opts);
      w.document.open(); w.document.write(h); w.document.close();
      return w;
    } catch (e) {
      console.error(e);
      try { w.document.body.textContent = 'The report could not be built: ' + e.message; } catch { }
      toast('The report could not be built: ' + e.message, 'err');
      return null;
    } finally { busy(null); }
  }
  // The report built once with placeholders, then the maps it asks for drawn and put in place
  async function built(P, opts = {}) {
    if (opts.mapImages) return html(P, opts);
    const b = withProject(P, () => build(P, { ...opts, mapImages: {} }));
    const images = await withProjectAsync(P, () => drawFigures(P, b.requests, opts));
    if (Object.keys(images).length) b.body = fillFigures(b.body, b.requests, images);
    return withProject(P, () => doc(P, b));
  }
  async function download(P = Store.P, opts = {}) {
    const why = blockedWhy(P);
    if (why) { toast(why, 'err'); return null; }
    const h = await built(P, opts);
    U.download(reportName(P), h, 'text/html');
    toast(`Saved ${reportName(P)}.`);
    return h;
  }

  /* ======================= data pack ======================= */
  const FILES = {
    'README.txt': 'Column dictionary with units, scenario list, model version and data vintage',
    'scenarios.csv': 'One row per scenario and period with its year, role and headline results', 'links.csv': 'Directional link results per scenario and period',
    'junction_summary.csv': 'Junction results per scenario and period', 'junction_movements.csv': 'Junction approach results per scenario and period', 'turning_movements.csv': 'Turning movements per junction, scenario and period',
    'trip_generation.csv': 'Vehicle trip generation per zone, land use and period', 'distribution.csv': 'Gateway distribution of development traffic',
    'counts_calibration.csv': 'Every count used, with the modelled value and GEH', 'calibration_summary.csv': 'Calibration checks against TMDG 2019 per period', 'route_times.csv': 'Bus route times per scenario and period',
    'corridor-sidra-volumes.csv': 'SIDRA hand-over: movement volumes per junction and period (no disclaimer row, so the loader reads it as is; D1 and D12 are in this README and in the loader script)',
    'load_corridor_volumes.py': 'SIDRA INTERSECTION 11 loader script for corridor-sidra-volumes.csv (carries D1 and D12)', 'read_datapack.py': 'Reads any data pack CSV in Python, skipping the D1 row',
    'crashes.csv': 'Every reported crash loaded from CAS', 'crash_sites.csv': 'Crash history per assessed site (HRIG)', 'provenance.csv': 'Data sources, query dates, records and licences', 'assumptions.csv': 'Standards and Spare Construct assumptions',
  };
  const COLS = {
    'scenarios.csv': [['scenario_id', 'Spare Construct scenario id'], ['scenario', 'name'], ['code', 'S0 to S3 where set'], ['role', 'existing, baseline, proposal, mitigation, sensitivity, permitted'], ['year', 'modelled year'], ['period', 'AM or PM peak hour'], ['trips_veh_h', 'veh/h'], ['vkt', 'veh·km/h'], ['vht', 'veh·h/h'], ['delay_veh_h', 'veh·h/h'], ['avg_speed_kmh', 'km/h'], ['co2_t_h', 't/h'], ['iterations', 'count'], ['gap', 'relative gap (fraction)'], ['converged', 'true or false'], ['run_at', 'ISO date-time'], ['stale', 'true if the inputs changed after the run']],
    'links.csv': [['scenario_id', ''], ['period', 'AM or PM'], ['link_id', ''], ['dir', 'AB or BA'], ['road_name', ''], ['from_node', 'node id'], ['to_node', 'node id'], ['class', 'motorway, ramp, arterial, collector, local'], ['fc_gt', 'Future Connect general traffic'], ['fc_pt', 'Future Connect public transport'], ['fc_freight', 'Future Connect freight'], ['fc_cycle', 'Future Connect cycle'], ['aup_arterial', 'true, false or blank'],
      ['lanes', 'lanes in this direction'], ['speed_limit_kmh', 'km/h'], ['speed_source', 'NSLR or coded'], ['length_m', 'm'], ['flow_veh_h', 'veh/h'], ['flow_pcu_h', 'pcu/h'], ['hv_pct', '% (model assumption)'], ['capacity_pcu_h', 'pcu/h including junction effects'], ['mid_block_capacity_pcu_h', 'pcu/h'], ['cap_source', 'class default or user'], ['vc', 'ratio'], ['speed_kmh', 'km/h'], ['speed_pct_limit', '%'], ['anop_gt_los', 'blank (ANOP tool)'], ['junction_delay_s', 's'],
      ['without_flow_veh_h', 'veh/h in the without scenario'], ['delta_veh_h', 'veh/h'], ['delta_pct', '%'], ['threshold_pct', 'extent threshold %'], ['in_study_area', 'true or false']],
    'junction_summary.csv': [['scenario_id', ''], ['period', ''], ['site_id', 'node or junction-site id'], ['junction', 'name'], ['control', 'priority, stop, roundabout, signals'], ['cycle_s', 's'], ['vol_veh_h', 'veh/h'], ['worst_dos', 'ratio'], ['delay_s', 's (average for signals and roundabouts, worst movement otherwise)'], ['los', 'A to F'], ['los_basis', ''], ['max_q95_m', 'm'], ['max_q95_leg', 'approach']],
    'junction_movements.csv': [['scenario_id', ''], ['period', ''], ['site_id', ''], ['junction', ''], ['approach', 'compass side traffic arrives from'], ['road', ''], ['link_id', ''], ['volume_veh_h', 'veh/h'], ['capacity_veh_h', 'veh/h'], ['dos', 'ratio'], ['delay_s', 's'], ['los', 'A to F'], ['q95_veh', 'veh'], ['q95_m', 'm'], ['psc_pct', '% practical spare capacity'], ['storage_m', 'approach link length m']],
    'turning_movements.csv': [['scenario_id', ''], ['period', ''], ['node_id', ''], ['junction', ''], ['from_arc', 'arc key'], ['to_arc', 'arc key'], ['movement', 'L, T, R or U'], ['leg', 'leg name'], ['turn', 'SIDRA turn designation'], ['movement_id', 'e.g. S-R2'], ['volume_veh_h', 'veh/h'], ['pcu_h', 'pcu/h'], ['development_veh_h', 'veh/h from development zones']],
    'trip_generation.csv': [['scenario_id', ''], ['period', ''], ['zone_id', ''], ['zone', ''], ['class', 'existing, committed, development, external'], ['land_use', ''], ['quantity', 'in the unit'], ['unit', ''], ['rate_veh_per_unit_h', 'veh trips per unit per hour'], ['in_pct', '%'], ['gross_veh_h', 'veh/h'], ['reduction_pct', '%'], ['buildout_pct', '%'], ['net_in_veh_h', 'veh/h'], ['net_out_veh_h', 'veh/h']],
    'distribution.csv': [['scenario_id', ''], ['gateway_id', ''], ['gateway', ''], ['am_in_pct', '%'], ['am_out_pct', '%'], ['pm_in_pct', '%'], ['pm_out_pct', '%'], ['am_veh_h', 'veh/h'], ['pm_veh_h', 'veh/h']],
    'counts_calibration.csv': [['scenario_id', ''], ['period', ''], ['count_id', ''], ['source', 'NZTA, AT or yours'], ['ref', 'site reference'], ['name', ''], ['road', ''], ['count_date', 'ISO date'], ['duration', ''], ['adt_vpd', 'veh/day'], ['observed_veh_h', 'veh/h'], ['modelled_veh_h', 'veh/h'], ['geh', ''], ['peak_basis', 'how the peak hour was derived'], ['link_id', 'matched link'], ['dir', 'matched direction']],
    'calibration_summary.csv': [['scenario_id', ''], ['period', ''], ['check', 'id'], ['criterion', ''], ['target', ''], ['value', 'number'], ['status', ''], ['source', ''], ['verified', 'true or false']],
    'route_times.csv': [['scenario_id', ''], ['period', ''], ['route_id', ''], ['route', ''], ['length_m', 'm'], ['time_s', 's'], ['speed_kmh', 'km/h']],
    'crashes.csv': [['cas_objectid', 'CAS OBJECTID; blank when Spare Construct did not keep it'], ['year', ''], ['severity', 'F, S, M or N'], ['fatal_count', 'people killed; blank when not loaded'], ['serious_count', 'people seriously injured; blank when not loaded'], ['minor_count', 'people with minor injuries; blank when not loaded'], ['speed_limit', 'km/h'], ['traffic_control', 'CAS trafficControl; blank when not loaded'], ['urban', 'CAS urban; blank when not loaded'], ['ped', 'true or false'], ['cyclist', 'true or false'], ['motorcyclist', 'true or false'], ['light', 'CAS light; blank when not loaded'], ['weather', 'CAS weatherA; blank when not loaded'], ['location1', ''], ['location2', ''], ['site_id', 'nearest assessed site within 50 m'], ['dist_m', 'm'], ['lat', 'WGS84'], ['lng', 'WGS84']],
    'crash_sites.csv': [['site_id', ''], ['type', 'junction name'], ['control', ''], ['speed_limit', 'km/h'], ['n_f', 'crashes'], ['n_s', 'crashes'], ['n_m', 'crashes'], ['n_n', 'crashes'], ['injury', 'crashes'], ['dsi_equiv_5yr', 'DSI equivalents'], ['collective_band', 'HRIG'], ['aadt_legs', 'vpd, busiest first'], ['pof', 'product of flows'], ['personal_risk', 'DSI per 100 million vehicles'], ['personal_band', 'HRIG'], ['reliable', 'true when 4 or more injury crashes']],
    'provenance.csv': [['source', ''], ['dataset', ''], ['query_date', 'ISO date'], ['records', ''], ['licence', '']],
    'assumptions.csv': [['group', ''], ['item', ''], ['source_edition', ''], ['basis', 'Spare Construct assumption or published source'], ['verified', 'true or false']],
  };
  /* RFC 4180 text: a cell with a comma, quote, CR or LF is quoted. Text that a spreadsheet would read as a formula
     (starting with = + - @, a tab or a CR, and not a number) gets a leading apostrophe, so a crafted name in a shared
     project cannot run as a formula when the CSV is opened in Excel (CSV injection). */
  const NUMLIKE = /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i;
  const csvCell = v => {
    let s = v == null ? '' : String(v);
    if (typeof v === 'string' && /^[=+\-@\t\r]/.test(s) && !NUMLIKE.test(s.trim())) s = "'" + s;
    return /[",\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s;
  };
  const csvText = rows => rows.map(r => r.map(csvCell).join(',')).join('\r\n');
  // UTF-8 BOM, then Disclaimer D1 as the first row, then the header and rows
  function csvFile(name, rows, cols = COLS[name]) {
    const head = cols.map(c => c[0]);
    return '\uFEFF' + csvText([[`# Spare Construct ITA data pack, ${name}. ${dText('D1')}`], head, ...rows]) + '\r\n';
  }
  function packFiles(P = Store.P) {
    return withProject(P, () => {
      const ctx = context(P, {}), sel = ctx.sel, S = P.settings, files = [], missing = [];
      const scs = P.scenarios, b = studyBounds(P), inB = L => { if (!b) return ''; const g = L.pts && L.pts.length ? L.pts : null; const p = g ? g[Math.floor(g.length / 2)] : null; return p ? (p[0] >= b[0][0] && p[0] <= b[1][0] && p[1] >= b[0][1] && p[1] <= b[1][1]) : ''; };
      const add = (name, rows) => files.push({ name, data: csvFile(name, rows) });
      add('scenarios.csv', scs.flatMap(s => periodsOf(s).map(p => { const r = resOf(s, p), k = r.kpi; return [s.id, s.name, s.code || '', s.role || '', r.year, p, U.r1(k.trips), U.r1(k.vkt), U.r1(k.vht), U.r1(k.dly), U.r1(k.spd), U.r3(k.co2), r.it, r.gap, r.converged !== false, day(r.ts) && new Date(r.ts).toISOString(), !!r.stale]; })));
      // links (with the extent comparison against the without scenario for the with and mitigation scenarios)
      const lr = [];
      for (const s of scs) for (const p of periodsOf(s)) {
        const A = aupOf(P, s);
        const r = resOf(s, p), wo = (s === sel.with || s === sel.mitigation) && sel.without ? resOf(sel.without, p) : null;
        const ext = wo ? Engine.extentTest(s, wo, r, { scWithout: sel.without }) : null, eRow = ext ? new Map(ext.rows.map(x => [x.key, x])) : null;
        for (const L of Object.values(s.links)) for (const d of ['ab', 'ba']) {
          const lanes = d === 'ab' ? L.lanesAB : L.lanesBA; if (!lanes) continue;
          const a = r.arcs[L.id + ':' + d]; if (!a) continue;
          const sp = speedOf(L), x = eRow && eRow.get(L.id + ':' + d), fc = L.fc || {}, cap = (+L.capPerLane || (CLASSES[L.cls] || CLASSES.local).cap) * lanes;
          lr.push([s.id, p, L.id, d.toUpperCase(), L.name || '', d === 'ab' ? L.a : L.b, d === 'ab' ? L.b : L.a, L.cls, fc.gt || '', fc.pt || '', fc.freight || '', fc.cycle || '', A.art(L) == null ? '' : A.art(L),
            lanes, sp.v, sp.src, Math.round(a.len), Math.round(a.v), Math.round(a.pcu), S.hvPct, Math.round(a.cap), cap, L.capPerLane ? 'user' : 'class default', a.vc, U.r1(a.spd), sp.v > 0 ? U.r1(a.spd / sp.v * 100) : '', '', U.r1(a.nd),
            x ? Math.round(x.without) : '', x ? Math.round(x.d) : '', x && x.p != null ? U.r1(x.p * 100) : '', x && x.threshold != null ? x.threshold * 100 : '', inB(L)]);
        }
      }
      add('links.csv', lr);
      // junctions
      const js = [], jm = [], tm = [];
      for (const s of scs) for (const p of periodsOf(s)) {
        const r = resOf(s, p);
        for (const [id, nr] of Object.entries(r.nodes)) {
          if (!nr || nr.type === 'none') continue;
          const j = jSum(nr), nm = siteName(s, r, id);
          js.push([s.id, p, nr.site || id, nm, nr.type, nr.C || '', Math.round(nr.vol || 0), U.r3(nr.dos), U.r1(j.delay), nr.los, j.basis, Math.round(j.q95), j.leg]);
          for (const a of nr.appr) { const L = s.links[a.link]; jm.push([s.id, p, nr.site || id, nm, a.approach || '', a.name || '', a.link, Math.round(a.v), a.cap, U.r3(a.dos), U.r1(a.delay), a.los, a.q95veh ?? '', a.q95, a.psc ?? '', L ? Math.round(Net.len(s, L)) : '']); }
        }
        for (const [id, list] of Object.entries(r.turns || {})) {
          if (!s.nodes[id]) continue;
          let mv = null; try { mv = typeof Turns !== 'undefined' ? Turns.movements(s, r, id) : null; } catch { mv = null; }
          const code = new Map((mv || []).map(m => [m.approach + '>' + m.to, m]));
          for (const t of list) { const m = code.get(t.from + '>' + t.to) || {}; tm.push([s.id, p, id, Kit.junctionName(id, s), t.from, t.to, t.mv, m.legName || '', m.turn || '', m.id || '', U.r1(t.v), U.r1(t.pcu), t.dev != null ? U.r1(t.dev) : '']); }
        }
      }
      add('junction_summary.csv', js); add('junction_movements.csv', jm); add('turning_movements.csv', tm);
      // one file per scenario and period as well, named with the period and year (data-pack: drury-option-a-am-2036-links.csv)
      const used = new Set();
      for (const s of scs) for (const p of periodsOf(s)) {
        const y = +resOf(s, p).year > 0 ? +resOf(s, p).year : yearOf(s) || 'year';
        const b0 = [slug(P.name) || 'project', slug(s.name) || slug(s.id) || 'scenario', p.toLowerCase(), y].join('-');
        let base = b0, k = 1;
        while (used.has(base)) base = `${b0}-${++k}`;
        used.add(base);
        for (const [table, rows] of [['links.csv', lr], ['junction_summary.csv', js], ['junction_movements.csv', jm], ['turning_movements.csv', tm]]) {
          const name = `by-scenario/${base}-${table.replace(/_/g, '-')}`, sub = rows.filter(r => r[0] === s.id && r[1] === p);
          files.push({ name, data: '﻿' + csvText([[`# Spare Construct ITA data pack, ${name}: ${s.name}, ${p} peak hour, ${y} (the columns of ${table}). ${dText('D1')}`], COLS[table].map(c => c[0]), ...sub]) + '\r\n', of: table });
        }
      }
      // trip generation and distribution
      const tg = [];
      for (const s of scs) for (const z of Object.values(s.zones)) for (const p of ['AM', 'PM']) {
        if (z.kind === 'gateway') { const g = z.gw || {}, gf = Engine.growth(S, s); tg.push([s.id, p, z.id, z.name || '', 'external', 'gateway', '', '', '', '', '', '', '', U.r1((+g[p + 'in'] || 0) * gf), U.r1((+g[p + 'out'] || 0) * gf)]); continue; }
        const red = U.clamp(+z.reduction || 0, 0, 100), st = Engine.stageOf(z, s), open = !(+z.year > Engine.yearOf(s));
        for (const [k, q] of Object.entries(z.lu || {})) {
          const rr = ((S.rates || {})[k] || {})[p], L = LANDUSES[k]; if (!rr || !L || !(+q > 0)) continue;
          const gross = +q / L.per * rr.rate, net = open ? gross * (1 - red / 100) * st : 0;
          tg.push([s.id, p, z.id, z.name || '', Engine.zoneClass(z), k, +q, L.unit, rr.rate, rr.inPct, U.r1(gross), red, U.r1(st * 100), U.r1(net * rr.inPct / 100), U.r1(net * (1 - rr.inPct / 100))]);
        }
      }
      add('trip_generation.csv', tg);
      const dist = [];
      for (const s of scs) { const rl = periodsOf(s).map(p => resOf(s, p)).filter(r => r.dev); if (!rl.length) continue; const dd = Engine.devDistribution(s, rl); for (const r of dd.rows) dist.push([s.id, r.id, r.name, U.r1(r.AMin || 0), U.r1(r.AMout || 0), U.r1(r.PMin || 0), U.r1(r.PMout || 0), r.vph.AM ?? '', r.vph.PM ?? '']); }
      add('distribution.csv', dist);
      // calibration
      const cc = [], cs = [], cal0 = sel.existing || sel.base;
      for (const p of periodsOf(cal0)) {
        const cal = DataHub.calibration(cal0, p);
        for (const x of (cal && cal.rows) || []) { const k = x.c; cc.push([cal0.id, p, k.id, k.src, k.ref || '', k.name || '', k.road || '', k.date || '', k.dur || '', k.adt || '', U.r1(x.obs), U.r1(x.model), x.geh != null ? U.r3(x.geh) : '', x.how || '', x.mt ? x.mt.link : '', x.mt ? x.mt.dir : '']); }
        const rep = Calibrate.report(cal0, p);
        for (const k of rep.checks) cs.push([cal0.id, p, k.id, k.label, k.target || '', typeof k.value === 'number' ? U.r3(k.value) : k.value ?? '', k.status || '', k.source || '', !!k.verified]);
      }
      add('counts_calibration.csv', cc); add('calibration_summary.csv', cs);
      const rt = [];
      for (const s of scs) for (const p of periodsOf(s)) for (const [id, x] of Object.entries(resOf(s, p).routes || {})) rt.push([s.id, p, id, (s.routes[id] || {}).name || '', Math.round(x.len), U.r1(x.t), U.r1(x.spd)]);
      add('route_times.csv', rt);
      // crashes
      const sites = crashSites(P, ctx), near = new Map();
      for (const st of sites) for (const { k, d } of st.list) { const o = near.get(k); if (!o || d < o.d) near.set(k, { id: st.id, d }); }
      add('crashes.csv', (D(P).crashes || []).map(k => { const n = near.get(k), [l1, l2] = String(k[4] || '').split(' / '); const f = String(k[5] || '');
        const q = i => { const v = cnum(k, i); return v == null ? '' : v; };
        return [ctext(k, CX.id), k[3], k[2], q(CX.fatal), q(CX.serious), q(CX.minor), k[6] ?? '', ctext(k, CX.control), ctext(k, CX.urban), f.includes('p'), f.includes('c'), f.includes('m'), ctext(k, CX.light), ctext(k, CX.weather), l1 || '', l2 || '', n ? n.id : '', n ? Math.round(n.d) : '', k[0], k[1]]; }));
      add('crash_sites.csv', sites.map(s => [s.id, s.name, s.control, s.speed, s.F, s.S, s.M, s.N, s.injury, U.r3(s.dsiEq), s.collective, s.legs.map(l => (l.aadt ? Math.round(l.aadt) : '')).join(' / '), s.pof ? U.r1(s.pof) : '', s.pr != null ? U.r1(s.pr) : '', s.pband.label, s.pband.reliable]));
      add('provenance.csv', vintageRows(P));
      add('assumptions.csv', assumptionRows(P).map(r => [r.group, r.name, r.edition, r.corridor ? 'Spare Construct assumption' : 'Published source', r.verified]));
      // OD matrices, SIDRA volumes, network, settings and the project file
      const odSc = sel.with;
      for (const p of periodsOf(odSc)) {
        const r = resOf(odSc, p); if (!r.T || !r.zoneIds) { missing.push(`od_${p}.csv: the OD matrix is kept only for 150 zones or fewer`); continue; }
        const Z = r.zoneIds.length, nm = id => (odSc.zones[id] || {}).name || id, rows = [[`# Spare Construct ITA data pack, OD matrix ${odSc.name} ${p} ${r.year} (veh/h). ${dText('D1')}`], ['from \\ to', ...r.zoneIds.map(nm)]];
        for (let i = 0; i < Z; i++) rows.push([nm(r.zoneIds[i]), ...Array.from({ length: Z }, (_, j) => U.r1(r.T[i * Z + j]))]);
        files.push({ name: `od_${p}.csv`, data: '\uFEFF' + csvText(rows) + '\r\n' });
      }
      /* The SIDRA hand-over: the junctions feature's volumes file and SIDRA 11 loader script, the same files as its
         exports. The volumes CSV stays byte-clean (a comment row breaks the loader); D1 and D12 travel in the loader
         script, the volume grids and this README. Without the junctions feature, Turns' export with a D1/D12 row. */
      const vol = Registry.get('exports', 'junctions-sidra-volumes'), py = Registry.get('exports', 'junctions-sidra-loader');
      if (odSc && vol && py) {
        const c0 = { sc: odSc, res: resOf(odSc, ctx.period) || resOf(odSc, periodsOf(odSc)[0]), period: ctx.period, fileBase: fileBase(P) };
        try {
          const v = vol.build(P, c0), l = py.build(P, c0);
          // a BOM (Excel shows macrons) but no comment row: the loader reads utf-8-sig with csv.DictReader
          if (typeof v === 'string' && v.trim()) files.push({ name: 'corridor-sidra-volumes.csv', data: '\uFEFF' + v.replace(/^\uFEFF/, ''), handover: true });
          else missing.push('corridor-sidra-volumes.csv: the junctions feature produced no rows');
          if (typeof l === 'string' && l.trim()) files.push({ name: 'load_corridor_volumes.py', data: l });
        } catch (e) { missing.push(`corridor-sidra-volumes.csv and load_corridor_volumes.py: ${e.message}`); }
      } else if (typeof Turns !== 'undefined' && odSc) {
        const x = Turns.sidraExport(odSc, periodsOf(odSc).map(p => resOf(odSc, p)), null, {});
        if (x.ok && x.csv) files.push({ name: 'corridor-sidra-volumes.csv', data: '\uFEFF' + csvText([[`# ${dText('D12', { tools: 'SIDRA' })} ${dText('D1')}`]]) + '\r\n' + x.csv + '\r\n' });
        else missing.push(`corridor-sidra-volumes.csv: ${x.why}`);
      }
      // anop.csv and screenlines.csv come from the features that make them (their registered exports)
      const fromExport = (name, id, feature) => {
        const d = Registry.get('exports', id), c = { sc: odSc, res: odSc ? resOf(odSc, ctx.period) || resOf(odSc, periodsOf(odSc)[0]) : null, fileBase: fileBase(P) };
        if (!d) { missing.push(`${name}: made by the ${feature} feature, which is not installed`); return; }
        let ok = true; try { ok = !d.enabled || !!d.enabled(c); } catch { ok = false; }
        if (!ok) { missing.push(`${name}: nothing to export yet (${feature} › ${d.label})`); return; }
        try {
          const t = String(d.build(P, c) || '').replace(/^\uFEFF/, '');
          if (!t.trim()) { missing.push(`${name}: ${feature} produced no rows`); return; }
          files.push({ name, data: '\uFEFF' + csvText([[`# Spare Construct ITA data pack, ${name}. ${dText('D1')}`]]) + '\r\n' + t.replace(/(\r?\n)*$/, '') + '\r\n' });
        } catch (e) { missing.push(`${name}: ${e.message}`); }
      };
      fromExport('anop.csv', 'compliance-extra-anop', 'Compliance+');
      fromExport('screenlines.csv', 'calibration-screenlines', 'Calibration');
      files.push({ name: 'read_datapack.py', data: `# ${dText('D1')}\n${PY}` });
      {
        const gs = odSc || P.scenarios[0], gj = Importers.geojson(gs, resOf(gs, ctx.period) || resOf(gs, periodsOf(gs)[0]));
        let data = gj;   // D1 travels with the network (reg-disclaimers: every exported file carries D1)
        try { const o = typeof gj === 'string' ? JSON.parse(gj) : gj; data = JSON.stringify({ type: o.type, name: `${P.name}: ${gs.name}`, disclaimer: dText('D1'), modelVersion: modelVersion(P), ...o }); } catch { }
        files.push({ name: 'network.geojson', data });
      }
      files.push({ name: 'settings.json', data: JSON.stringify({ settings: { ...S, linzKey: undefined }, modelVersion: modelVersion(P), disclaimer: dText('D1') }, null, 1) });
      files.push({ name: 'project.corridor.json', data: Store.serialise(P) });
      files.unshift({ name: 'README.txt', data: readme(P, sel, files, missing) });
      return files;
    });
  }
  const PY = `# Load Spare Construct ITA data pack CSVs in Python (standard library only).
# Every CSV is UTF-8 with a BOM, and its first row is Disclaimer D1: skip it.
# pandas: pd.read_csv(path, encoding='utf-8-sig', skiprows=1)
import csv, sys

def load(path):
    with open(path, encoding='utf-8-sig', newline='') as f:
        rows = list(csv.reader(f))
    if rows and rows[0] and rows[0][0].startswith('#'):
        rows = rows[1:]
    head, body = rows[0], rows[1:]
    return [dict(zip(head, r)) for r in body]

if __name__ == '__main__':
    path = sys.argv[1] if len(sys.argv) > 1 else 'links.csv'
    data = load(path)
    print(f'{len(data)} rows from {path}')
    for row in data[:10]:
        print(row)
`;
  // network.geojson property units (Importers.geojson); unknown keys print '-'
  const GJ_UNITS = { id: 'link id', name: 'road name', cls: 'road class', status: 'existing, proposed or closed', speed: 'km/h (coded speed)', lanesAB: 'lanes A to B', lanesBA: 'lanes B to A',
    busLane: 'true or false', cycle: 'cycle facility: none, painted or separated', length_m: 'm', vol_ab: 'veh/h A to B', vc_ab: 'volume/capacity ratio', spd_ab: 'km/h', vol_ba: 'veh/h B to A', vc_ba: 'volume/capacity ratio', spd_ba: 'km/h' };
  function readme(P, sel, files, missing) {
    const L = [], line = (s = '') => L.push(s);
    line(`Spare Construct ITA data pack: ${P.name}`); line(`Made ${nowIso().slice(0, 16).replace('T', ' ')} UTC with ${modelVersion(P)}.`); line();
    line('DISCLAIMER D1 (screening status)'); line(dText('D1')); line(); line('DISCLAIMER D12 (hand-over)'); line(dText('D12', { tools: 'SIDRA / SATURN / Aimsun / AT ANOP tool / NZTA IER tool / MBCM workbook' })); line();
    line('Format: every CSV is UTF-8 with a byte-order mark, so Excel shows macrons (for example Ōtāhuhu) correctly. The first row of every CSV is Disclaimer D1; skip one row when reading (pandas: skiprows=1). The one exception is corridor-sidra-volumes.csv, the SIDRA hand-over file: it has no disclaimer row so the SIDRA loader reads it as is (D1 and D12 above apply to it). Dates are ISO (YYYY-MM-DD). Units are listed below, not in the headers.'); line();
    line('SCENARIOS'); for (const s of P.scenarios) line(`- ${s.id}: ${s.name} (${Engine.yearOf(s)}${s.role ? ', ' + s.role : ''})${s === sel.with ? ' [assessed]' : s === sel.without ? ' [without]' : s === sel.mitigation ? ' [mitigation]' : ''}`); line();
    line('FILES'); for (const f of files) line(`- ${f.name}${FILES[f.name] ? ': ' + FILES[f.name] : f.of ? `: the columns of ${f.of}, for one scenario and period` : ''}`); line('- README.txt: this file');
    line(); line('by-scenario/<project>-<scenario>-<period>-<year>-<table>.csv: the rows of links.csv, junction_summary.csv, junction_movements.csv and turning_movements.csv split by scenario and period, so each file names its period and year. Same columns and units as the combined file.');
    if (missing.length) { line(); line('NOT INCLUDED'); for (const m of missing) line(`- ${m}`); }
    line(); line('COLUMNS (name: unit or meaning)');
    for (const [f, cols] of Object.entries(COLS)) { line(); line(f); for (const [k, u] of cols) line(`  ${k}: ${u || '-'}`); }
    // files made outside the dictionary: their header row, as written (units are in the names or listed here)
    const UNIT = { 'corridor-sidra-volumes.csv': 'veh/h where the column says so; Period AM or PM; Site is the SIDRA site number', 'anop.csv': 'AT ANOP tool v10.2 input sheet columns; LoS A to F (indicative, D5)', 'screenlines.csv': 'veh/h; diff_pct a fraction; geh unitless; completeness a fraction' };
    for (const f of files.filter(f => /\.csv$/.test(f.name) && !COLS[f.name] && !f.of && !/^od_/.test(f.name))) {
      const rows = String(f.data).replace(/^﻿/, '').split(/\r?\n/).filter(r => r && !/^"?#/.test(r)), head = rows.length ? rows[0].split(',').map(x => x.replace(/^"|"$/g, '')) : [];
      line(); line(f.name); if (UNIT[f.name]) line(`  (${UNIT[f.name]})`); for (const k of head) line(`  ${k}: ${/veh\/h|%|km\/h|\bm\b|\bs\b/.test(k) ? 'unit in the name' : '-'}`);
    }
    const gj = files.find(f => f.name === 'network.geojson');
    if (gj) { let props = []; try { const o = JSON.parse(gj.data); props = Object.keys(((o.features || [])[0] || {}).properties || {}); } catch { }
      line(); line('network.geojson (WGS84; the file carries D1 and the model version)'); line('  Link features (LineString), properties:'); for (const k of props) line(`    ${k}: ${GJ_UNITS[k] || '-'}`); line('  Junction features (Point): id, control, delay_s (s), los (A to F). Zone features (Point): id, zone (name), kind, land-use quantities in their units, gateway flows (veh/h).'); }
    line(); line('od_<period>.csv: rows are origin zones, columns destination zones, veh/h in the peak hour.');
    line('settings.json: every model setting. project.corridor.json: the full project; open it in Spare Construct (Menu › Open) to reproduce every run.');
    line(); line('DATA VINTAGE (Source | Dataset | Query date | Records | Licence)'); for (const r of vintageRows(P)) line(r.join(' | '));
    return L.join('\r\n') + '\r\n';
  }
  // STORE-method ZIP (no compression): local headers, central directory, end record. Names are UTF-8 (flag bit 11).
  const CRC_T = (() => { const t = new Uint32Array(256); for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } return t; })();
  function crc32(b) { let c = 0xFFFFFFFF; for (let i = 0; i < b.length; i++) c = CRC_T[(c ^ b[i]) & 0xFF] ^ (c >>> 8); return (c ^ 0xFFFFFFFF) >>> 0; }
  function zip(files, when = new Date()) {
    const enc = new TextEncoder(), parts = [], central = [];
    const dosT = ((when.getHours() << 11) | (when.getMinutes() << 5) | (when.getSeconds() >> 1)) & 0xFFFF;
    const dosD = ((Math.max(0, when.getFullYear() - 1980) << 9) | ((when.getMonth() + 1) << 5) | when.getDate()) & 0xFFFF;
    let off = 0;
    for (const f of files) {
      const name = enc.encode(f.name), data = typeof f.data === 'string' ? enc.encode(f.data) : new Uint8Array(f.data), crc = crc32(data);
      const h = new DataView(new ArrayBuffer(30));
      h.setUint32(0, 0x04034b50, true); h.setUint16(4, 20, true); h.setUint16(6, 0x0800, true); h.setUint16(8, 0, true); h.setUint16(10, dosT, true); h.setUint16(12, dosD, true);
      h.setUint32(14, crc, true); h.setUint32(18, data.length, true); h.setUint32(22, data.length, true); h.setUint16(26, name.length, true); h.setUint16(28, 0, true);
      parts.push(new Uint8Array(h.buffer), name, data);
      const c = new DataView(new ArrayBuffer(46));
      c.setUint32(0, 0x02014b50, true); c.setUint16(4, 20, true); c.setUint16(6, 20, true); c.setUint16(8, 0x0800, true); c.setUint16(10, 0, true); c.setUint16(12, dosT, true); c.setUint16(14, dosD, true);
      c.setUint32(16, crc, true); c.setUint32(20, data.length, true); c.setUint32(24, data.length, true); c.setUint16(28, name.length, true); c.setUint32(42, off, true);
      central.push(new Uint8Array(c.buffer), name);
      off += 30 + name.length + data.length;
    }
    const cd = central.reduce((s, a) => s + a.length, 0), e = new DataView(new ArrayBuffer(22));
    e.setUint32(0, 0x06054b50, true); e.setUint16(8, files.length, true); e.setUint16(10, files.length, true); e.setUint32(12, cd, true); e.setUint32(16, off, true);
    const all = [...parts, ...central, new Uint8Array(e.buffer)], out = new Uint8Array(all.reduce((s, a) => s + a.length, 0));
    let p = 0; for (const a of all) { out.set(a, p); p += a.length; }
    return out;
  }
  const dataPack = (P = Store.P) => zip(packFiles(P));
  function downloadPack(P = Store.P) {
    const why = blockedWhy(P);
    if (why) { toast(why, 'err'); return null; }
    const bytes = dataPack(P), name = packName(P);
    const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([bytes], { type: 'application/zip' })); a.download = name;
    document.body.appendChild(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
    toast(`Saved ${name} (${N(bytes.length / 1048576, 1)} MB).`);
    return bytes;
  }

  /* ======================= staging and sensitivity runs ======================= */
  // Sites failing in a result: over practical DoS, LoS E/F, or a 95th-percentile queue longer than the approach link
  function failing(sc, res, ids) {
    const out = [], why = {}, crit = {};
    for (const id of ids) {
      const nr = res.nodes[id]; if (!nr) continue;
      const r = [], k = [];
      if (nr.dos > (nr.xp || 1)) { r.push(`DoS ${R.dos(nr.dos)} over ${R.dos(nr.xp || 1)}`); k.push('dos'); }
      if (nr.los === 'E' || nr.los === 'F') { r.push(`LoS ${nr.los}`); k.push('los'); }
      for (const a of nr.appr || []) { const L = sc.links[a.link]; if (L && a.q95 > Net.len(sc, L)) { r.push(`queue ${R.q(a.q95)} m on ${a.name || a.approach}`); k.push('queue'); break; } }
      if (r.length) { out.push(id); why[id] = r.join(', '); crit[id] = k; }
    }
    return { ids: out, why, crit };
  }
  /* Runs the proposal at build-out steps (default 0, 10 … 100%) for its periods, finds the first increment where an
     assessed site that passes at 0% fails, re-runs the mitigation scenario at that increment, and stores the result
     in P.report.staging. opts: { steps, periods, scenario (id), mitigation (id), measure, onProgress(k, n, text) }. */
  async function staging(P = Store.P, opts = {}) {
    return withProjectAsync(P, async () => {
      const sel = pick(P), sc = (opts.scenario && P.scenarios.find(s => s.id === opts.scenario)) || sel.with;
      if (!sc) throw new Error('Choose the proposal scenario in the Report tab first.');
      const mit = opts.mitigation === null ? null : (opts.mitigation && P.scenarios.find(s => s.id === opts.mitigation)) || sel.mitigation;
      const periods = opts.periods || (periodsOf(sc).length ? periodsOf(sc) : sc.periods || ['AM', 'PM']), steps = opts.steps || [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
      const ids = opts.sites || assessedSites(P, { ...sel, with: sc }).ids;
      if (!ids.length) throw new Error('No assessed junctions: run the proposal scenario first.');
      const prog = opts.onProgress || (() => {}), n = steps.length * periods.length + (mit ? periods.length : 0), rows = [], names = {};
      let k = 0;
      const variant = (src, pct) => { const v = Scenarios.copy(src); for (const f of ['year', 'role', 'periods']) if (src[f] != null) v[f] = JSON.parse(JSON.stringify(src[f])); v.buildout = pct; v.name = `${src.name} at ${pct}%`; return v; };
      const q = devQuantities(variant(sc, 100));   // full build-out of the scenario (zones' own build-out and opening years apply)
      for (const pct of steps) {
        const v = variant(sc, pct), fails = new Set(), why = {}, crit = {}, dos = {};
        let worst = null, queue = false;
        for (const p of periods) {
          prog(++k, n, `${pct}% build-out, ${p} peak`);
          const r = await Engine.assign(v, { period: p });
          const f = failing(v, r, ids);
          for (const id of f.ids) { fails.add(id); why[id] = why[id] ? `${why[id]}; ${p}: ${f.why[id]}` : `${p}: ${f.why[id]}`; crit[id] = [...new Set([...(crit[id] || []), ...f.crit[id]])]; if (f.crit[id].includes('queue')) queue = true; }
          for (const id of ids) { const nr = r.nodes[id]; if (!nr) continue; names[id] = names[id] || siteName(v, r, id); dos[id] = Math.max(dos[id] || 0, U.r3(nr.dos || 0)); if (!worst || nr.dos > worst.dos) worst = { id, name: siteName(v, r, id), dos: U.r3(nr.dos), los: nr.losText || nr.los, period: p }; }
        }
        // the dwellings and GFA the engine loaded in this variant (not the zones' nominal totals)
        const qv = devQuantities(v);
        rows.push({ pct, dwellings: U.r1(qv.dwellings), gfa: Math.round(qv.gfa), fails: [...fails], why, crit, dos, worst, queue });
      }
      const trig = stagingTrigger(rows);
      let resolved = null;
      if (trig.row && mit) {
        const v = variant(mit, trig.row.pct); let still = false;
        // resolved when no triggered site fails a criterion it passed at 0%, or stays materially worse than at 0%
        const r0 = rows.find(r => r.pct === 0) || { crit: {}, dos: {} };
        for (const p of periods) {
          prog(++k, n, `Mitigation at ${trig.row.pct}%, ${p} peak`);
          const r = await Engine.assign(v, { period: p }), f = failing(v, r, trig.sites);
          for (const id of trig.sites) {
            const was = (r0.crit || {})[id] || [], now = f.crit[id] || [], nr = r.nodes[id];
            if (now.some(c => !was.includes(c)) || (was.length && nr && (r0.dos || {})[id] != null && nr.dos - r0.dos[id] >= WORSEN.dos - 1e-9)) still = true;
          }
        }
        resolved = !still;
      }
      const diff = mit ? networkDiff(sc, mit) : [], measure = opts.measure || (diff.length ? [...new Set(diff.slice(0, 2).map(d => (/^(New|Removed|Changed|Closed|Upgraded|Added)\b/.test(d.text) ? d.text.charAt(0).toLowerCase() + d.text.slice(1) : d.text)))].join(' and ') : '');
      const text = trig.row ? triggerText(trig, measure ? `provide ${measure}` : 'provide the mitigation in Table M1') : '';
      const out = { at: nowIso(), sc: sc.id, scName: sc.name, mitigation: mit ? mit.id : null, periods, steps, rows, trigger: trig.row ? { pct: trig.row.pct, dwellings: trig.row.dwellings, gfa: trig.row.gfa, sites: trig.sites, names: trig.sites.map(id => names[id] || id), why: trig.sites.map(id => trig.row.why[id]), how: trig.sites.map(id => trig.how[id]) } : null,
        lastPass: trig.lastPass ? trig.lastPass.pct : null, existing: trig.existing, existingWhy: trig.existing.map(id => ({ id, name: names[id] || id, why: ((rows.find(r => r.pct === 0) || {}).why || {})[id] || '' })),
        resolved, measure, text, quantities: q, worsen: WORSEN.basis };
      ensure(P).staging = out;
      if (Store.P === P && typeof Store.emit === 'function') Store.emit('meta');
      return out;
    });
  }
  /* Runs Engine.sensitivity for the assessed scenario and keeps a summary in P.report.sens. No econBase: the 8%
     discount-rate row would treat the development as a transport option against the without scenario. */
  async function sensitivity(P = Store.P, opts = {}) {
    return withProjectAsync(P, async () => {
      const sel = pick(P), sc = sel.with; if (!sc) throw new Error('Choose the assessed scenario in the Report tab first.');
      const res = await Engine.sensitivity(sc, { without: sel.without || undefined, periods: opts.periods || (periodsOf(sc).length ? periodsOf(sc) : undefined), tests: opts.tests, onProgress: opts.onProgress,
        name: id => { const r = resOf(sc, periodsOf(sc)[0]); return r ? siteName(sc, r, id) : id; } });
      if (res.error) throw new Error(res.error);
      const keep = r => { const { changes, noise, ...x } = r; return { ...x, changes: (changes || []).slice(0, 5), noise: noise ? { R: noise.R, ok: noise.ok } : null }; };
      const out = { at: nowIso(), sc: sc.id, scName: sc.name, rows: res.rows.map(keep), sentences: res.sentences, flags: res.flags, note: res.note, runs: res.runs, ms: res.ms };
      ensure(P).sens = out;
      if (Store.P === P && typeof Store.emit === 'function') Store.emit('meta');
      return out;
    });
  }

  /* ======================= completeness (checklist) ======================= */
  // Per chapter: how many of Spare Construct's checkable sections are complete
  function completeness(P = Store.P) {
    return withProject(P, () => CHAPTERS.map(ch => {
      const list = S_.filter(s => chapterOf(s.chapter) === ch.id && s.check), st = list.map(s => { let v = null; try { v = s.check(P); } catch { v = false; } return { s, v }; }).filter(x => x.v !== null);
      const missP = RR422.rows.filter(r => r.chapter === ch.id && rs(P).scope && levelOf(r, rs(P).scope) === 'P' && !r.done(P));
      return { ch, total: st.length, done: st.filter(x => x.v).length, todo: st.filter(x => !x.v).map(x => x.s.title || x.s.id), missP };
    }).filter(x => x.total || x.missP.length));
  }
  const chLabel = ch => (ch.id === 'front' ? 'Front matter' : `${ch.app ? 'Appendix ' : ''}${ch.num} ${ch.title}`);
  for (const ch of CHAPTERS.filter(c => S_.some(s => chapterOf(s.chapter) === c.id && s.check))) {
    Registry.checklist({ id: `report-ch-${ch.id.toLowerCase()}`, group: 'ITA report', order: 500 + CHAPTERS.indexOf(ch), label: chLabel(ch), source: 'AT ITA Guidelines 2025 v2',
      status(P) {
        const c = completeness(P).find(x => x.ch === ch); if (!c) return { state: 'na', text: `${chLabel(ch)}: nothing to check` };
        const state = c.missP.length ? 'warn' : c.done === c.total ? 'done' : 'todo';
        return { state, text: `${chLabel(ch)}: ${c.done} of ${c.total} complete${c.todo.length ? ` (to do: ${c.todo.slice(0, 3).join(', ')})` : ''}${c.missP.length ? ` · missing Probable: ${c.missP.map(r => r.label).join(', ')}` : ''}`,
          action: { label: 'Open', run: () => UI.openTab('report') } };
      } });
  }
  for (const r of RR422.rows) Registry.checklist({ id: `report-rr422-${r.id}`, group: 'RR 422 scope', order: 600 + RR422.rows.indexOf(r), label: r.label, source: 'NZTA RR 422 Table 6.3',
    status(P) {
      const scope = rs(P).scope; if (!scope) return { state: 'todo', text: `${r.label}: choose the RR 422 scope first`, action: { label: 'Choose', run: () => UI.openTab('report') } };
      const l = levelOf(r, scope), ok = r.done(P);
      return { state: ok ? 'done' : l === 'P' ? 'warn' : l === 'U' ? 'na' : 'todo', text: `${r.label}: ${LEVEL[l]} for ${SCOPES[scope].label} scope · ${ok ? 'included' : 'not yet included. ' + r.how}` };
    } });

  /* ======================= the Report tab ======================= */
  const openDetails = new Set(['details']);
  const fld = (label, inner, hint = '') => `<label class="fld"><span>${label}</span><span class="fld-in">${inner}</span></label>${hint ? `<p class="note" style="margin:-2px 0 6px">${hint}</p>` : ''}`;
  const inp = (path, val, type = 'text', extra = '') => `<input type="${type}" data-rf="${esc(path)}" value="${esc(val ?? '')}" ${extra}>`;
  const det = (id, title, body) => `<details data-rd="${id}"${openDetails.has(id) ? ' open' : ''} style="margin:8px 0"><summary style="cursor:pointer;font-weight:600">${title}</summary><div style="padding:6px 0 4px 4px">${body}</div></details>`;
  function tabHtml() {
    const P = Store.P, R0 = rs(P), d = R0.details || {}, so = R0.signoff || {}, sel = pick(P), rg = regCtx(P), scOpt = (k, v) => `<select data-rf="sel.${k}"><option value="">Automatic</option>${P.scenarios.map(s => `<option value="${esc(s.id)}" ${R0.sel && R0.sel[k] === s.id ? 'selected' : ''}>${esc(s.name)}</option>`).join('')}</select> <span class="note">${esc(v ? v.name : 'none')}</span>`;
    const comp = completeness(P), authors = S_.filter(s => /author|mixed/.test(s.tag || '')).flatMap(s => { const m = String(s.render).match(/c\.author\('([\w-]+)'/g) || []; return m.map(x => ({ id: x.match(/'([\w-]+)'/)[1], s })); });
    const written = authors.filter(a => String((R0.text || {})[a.id] || '').trim()).length;
    const rr = RR422.rows.map(r => { const l = R0.scope ? levelOf(r, R0.scope) : null, ok = r.done(P);
      return `<tr><td>${esc(r.label)}</td>${SCOPE_KEYS.map(k => `<td style="${k === R0.scope ? 'font-weight:700' : ''}">${LEVEL[levelOf(r, k)]}</td>`).join('')}<td>${ok ? '<span class="los A">✓</span> included' : l === 'P' ? `<b style="color:var(--danger)">Missing (Probable)</b> <span class="note">${esc(r.how)}</span>` : `<span class="note">${esc(r.how)}</span>`}</td></tr>`; }).join('');
    const st = R0.staging, sv = R0.sens;
    return `<div class="bar-h"><h3>ITA report</h3><button class="btn small primary" data-rp="open">Open the report</button><button class="btn small" data-rp="html">Download HTML</button><button class="btn small" data-rp="pack">Download the data pack (ZIP)</button>
        <span class="note">${signedOff(P) ? `Signed off by ${esc(so.reviewer)}.` : 'DRAFT until a reviewer signs off below.'} The report follows AT ITA Guidelines 2025 v2.</span></div>
      <div class="row2"><div>
        ${det('details', 'Project and author details', `
          ${fld('Site address', inp('details.site', d.site))}${fld('Client', inp('details.client', d.client))}${fld('Author', inp('details.author', d.author))}
          ${rg.from === 'compliance' ? `<p class="note">Pathway, lodgement date and regime come from the Compliance tab: ${esc(rg.label || 'pathway not set')}, ${esc(rg.date || 'lodgement date not set')}. ${esc(rg.regime)}</p>` : `
          ${fld('Pathway', `<select data-rf="details.pathway"><option value="">Not stated</option>${Kit.opt(PATHWAY_LABEL(), d.pathway || '')}</select>`)}
          ${fld('Lodgement date', inp('details.lodgement', d.lodgement, 'date'), 'Sets the RMA regime: lodged before 30 Oct 2026 = RMA as it stood before 30 Sep 2026.')}`}
          ${fld('Report date', inp('details.date', d.date, 'date'))}
          ${fld('RR 422 scope', `<select data-rf="scope"><option value="">Choose…</option>${SCOPE_KEYS.map(k => `<option value="${k}" ${R0.scope === k ? 'selected' : ''}>${SCOPES[k].label}: ${esc(SCOPES[k].text)}</option>`).join('')}</select>`)}`)}
        ${det('scen', 'Scenarios in the report', `${fld('With proposal', scOpt('with', sel.with))}${fld('Without proposal', scOpt('without', sel.without))}${fld('With mitigation', scOpt('mitigation', sel.mitigation))}${fld('Existing (base year)', scOpt('existing', sel.existing))}
          <p class="note">Automatic uses the scenario roles from the assessment set, then the base and active scenarios. Assessed junctions: ${esc(assessedSites(P, sel).basis)}</p>`)}
        ${det('signoff', 'Review and sign-off', `${fld('Reviewer', inp('signoff.reviewer', so.reviewer))}${fld('Review date', inp('signoff.date', so.date, 'date'))}${fld('Approved by', inp('signoff.approver', so.approver))}
          <label class="check"><input type="checkbox" data-rf="signoff.reviewed" ${so.reviewed ? 'checked' : ''}> Reviewed by the named reviewer (removes the DRAFT watermark)</label>`)}
        ${det('control', `Document control and engagement (${R0.revisions.length} revisions, ${R0.engagement.length} meetings)`, `
          ${Kit.table({ head: ['Rev', 'Date', 'Description', 'Prepared', 'Reviewed', 'Approved', ''], rows: R0.revisions.map((r, i) => [...['rev', 'date', 'desc', 'prepared', 'reviewed', 'approved'].map(k => inp(`revisions.${i}.${k}`, r[k], k === 'date' ? 'date' : 'text', 'style="width:100%"')), `<button class="btn small icon" data-rp="del:revisions:${i}" aria-label="Remove">✕</button>`]) })}
          <button class="btn small" data-rp="add:revisions">Add a revision</button>
          ${Kit.table({ head: ['Date', 'Party', 'Topic', 'Outcome', ''], rows: R0.engagement.map((r, i) => [...['date', 'party', 'topic', 'outcome'].map(k => inp(`engagement.${i}.${k}`, r[k], k === 'date' ? 'date' : 'text', 'style="width:100%"')), `<button class="btn small icon" data-rp="del:engagement:${i}" aria-label="Remove">✕</button>`]) })}
          <button class="btn small" data-rp="add:engagement">Add a meeting or agreement</button>`)}
        ${det('crossings', `Pedestrian crossings for Table WK-1 (${R0.crossings.length})`, `
          ${Kit.table({ head: ['Crossing', 'Control', 'Conflicting veh/h', 'Length m', 'Cycle s', 'Walk green s', 'Next crossing m', 'Delay s', ''], rows: R0.crossings.map((x, i) => [inp(`crossings.${i}.name`, x.name, 'text', 'style="width:100%"'),
            `<select data-rf="crossings.${i}.control">${Kit.opt({ uncontrolled: 'Uncontrolled', refuge: 'Refuge island', zebra: 'Zebra', signals: 'Signals' }, x.control || 'uncontrolled')}</select>`,
            ...['flow', 'length', 'C', 'g', 'next'].map(k => inp(`crossings.${i}.${k}`, x[k], 'number', 'style="width:70px"')), (() => { const v = pedDelay(x.control || 'uncontrolled', { C: x.C, g: x.g, L: x.length, flow: x.flow, Sp: (R0.ped || {}).Sp, ts: (R0.ped || {}).ts }); return v == null ? '–' : N(v, 1); })(),
            `<button class="btn small icon" data-rp="del:crossings:${i}" aria-label="Remove">✕</button>`]) })}
          <button class="btn small" data-rp="add:crossings">Add a crossing</button> ${fld('Walking speed', inp('ped.Sp', (R0.ped || {}).Sp ?? PED.Sp, 'number', 'step="0.1" style="width:70px"') + '<span class="unit">m/s</span>')}${fld('Start-up time', inp('ped.ts', (R0.ped || {}).ts ?? PED.ts, 'number', 'step="0.5" style="width:70px"') + '<span class="unit">s</span>')}`)}
      </div><div>
        <div class="lg-title">Completeness</div>
        ${comp.map(x => `<div class="lg-row"><span class="los ${x.missP.length ? 'F' : x.done === x.total ? 'A' : 'none'}">${x.missP.length ? '!' : x.done === x.total ? '✓' : '·'}</span><span>${esc(chLabel(x.ch))}: ${x.done} of ${x.total} complete${x.missP.length ? ` · <b style="color:var(--danger)">missing Probable: ${esc(x.missP.map(r => r.label).join(', '))}</b>` : ''}</span></div>`).join('')}
        <div class="lg-title" style="margin-top:10px">RR 422 Table 6.3 content by scope</div>
        <div class="tbl-wrap"><table class="t compact"><tr><th>Item</th>${SCOPE_KEYS.map(k => `<th>${SCOPES[k].label}</th>`).join('')}<th>Status</th></tr>${rr}</table></div>
        <p class="note">${esc(RR422.note)}</p>
        ${det('runs', 'Staging test and sensitivity pack', `
          <p><button class="btn small" data-rp="staging">Run the staging test</button> <span class="note">Runs ${esc(sel.with ? sel.with.name : 'the proposal')} at 10% build-out steps (about ${Math.max(1, Math.round(11 * Math.max(1, periodsOf(sel.with).length) * 0.7))} s on Drury).</span></p>
          ${st ? `<p class="note">${esc(st.text || 'No assessed junction fails at any build-out step.')} Run ${esc(day(st.at))}.</p>` : ''}
          <p><button class="btn small" data-rp="sens">Run the sensitivity pack</button> <span class="note">8 tests, about ${sel.without ? 15 : 8} s on Drury.</span></p>
          ${sv ? `<p class="note">${plural((sv.rows || []).length, 'test')} run ${esc(day(sv.at))}. ${esc((sv.flags || [])[0] || 'No conclusion changes.')}</p>` : ''}`)}
        ${det('ssm', 'Safe System screening: likelihood scores', ssmInputs(P))}
        ${det('text', `Author text (${written} of ${authors.length} written)`, authors.map(a => `<label class="fld" style="display:block"><span>${esc(a.s.title || 'Cover')}${a.id !== a.s.id.replace('report-', '') ? ` · ${esc(a.id)}` : ''}</span><textarea data-rt="${esc(a.id)}" rows="3" style="width:100%">${esc((R0.text || {})[a.id] || '')}</textarea></label>`).join(''))}
      </div></div>`;
  }
  function ssmInputs(P) {
    const sites = withProject(P, () => crashSites(P, context(P, {}))).slice(0, 3);
    if (!sites.length) return '<p class="note">Load crashes and run the assessed scenario to list the sites.</p>';
    const L = rs(P).ssm || {};
    return sites.map(s => `<div class="lg-title" style="margin-top:6px">${esc(s.name)}</div><div style="display:flex;flex-wrap:wrap;gap:6px">${SSM.cols.map(([k, l]) => `<label class="note">${esc(l)} <select data-rf="ssm.${esc(s.id)}.${k}"><option value="">?</option>${[1, 2, 3, 4].map(v => `<option ${+((L[s.id] || {})[k]) === v ? 'selected' : ''}>${v}</option>`).join('')}</select></label>`).join('')}
      ${['pedDay', 'cycDay', 'mcDay'].map((k, i) => `<label class="note">${['Pedestrians', 'Cyclists', 'Motorcyclists'][i]}/day ${inp(`ssm.${s.id}.${k}`, (L[s.id] || {})[k], 'number', 'style="width:64px"')}</label>`).join('')}</div>`).join('') + `<p class="note">${esc(SSM.label)}.</p>`;
  }
  function setField(path, raw, type) {
    const R0 = ensure(Store.P), parts = path.split('.');
    let o = R0;
    for (let i = 0; i < parts.length - 1; i++) { const k = parts[i]; if (o[k] == null || typeof o[k] !== 'object') o[k] = /^\d+$/.test(parts[i + 1]) ? [] : {}; o = o[k]; }
    const last = parts[parts.length - 1];
    o[last] = type === 'checkbox' ? !!raw : type === 'number' ? (raw === '' ? null : +raw) : String(raw).trim();
  }
  const reRender = () => { try { Drawer.render(); } catch { } };
  async function act(what) {
    const P = Store.P;
    if (what === 'open') return open(P);
    if (what === 'html') return download(P);
    if (what === 'pack') { try { downloadPack(P); } catch (e) { console.error(e); toast('The data pack could not be built: ' + e.message, 'err'); } return; }
    if (what === 'staging' || what === 'sens') {
      busy(what === 'staging' ? 'Running the staging test…' : 'Running the sensitivity pack…');
      try {
        const o = { onProgress: (k, n, t) => busy(`${what === 'staging' ? 'Staging test' : 'Sensitivity pack'}: ${t} (${k} of ${n})`) };
        const r = what === 'staging' ? await staging(P, o) : await sensitivity(P, o);
        toast(what === 'staging' ? (r.text || 'No assessed junction fails at any build-out step.') : `Sensitivity pack done: ${plural(r.rows.length, 'test')}.`);
      } catch (e) { console.error(e); toast(e.message, 'err'); } finally { busy(null); reRender(); }
      return;
    }
    const m = /^(add|del):(\w+)(?::(\d+))?$/.exec(what);
    if (m) { const R0 = ensure(P); R0[m[2]] = R0[m[2]] || []; if (m[1] === 'add') R0[m[2]].push(m[2] === 'revisions' ? { rev: String.fromCharCode(65 + R0.revisions.length), date: day(nowIso()) } : m[2] === 'crossings' ? { control: 'uncontrolled' } : { date: day(nowIso()) }); else R0[m[2]].splice(+m[3], 1); Store.emit('meta'); reRender(); }
  }
  Registry.tabs({ id: 'report', label: 'Report', order: 85, title: 'The ITA report: details, completeness, author text, staging and sensitivity', render: tabHtml,
    onClick(e) {
      const sm = e.target.closest('summary');
      if (sm) { const dd = sm.parentElement; setTimeout(() => { if (dd.open) openDetails.add(dd.dataset.rd); else openDetails.delete(dd.dataset.rd); }, 0); return true; }
      const b = e.target.closest('[data-rp]'); if (b) { act(b.dataset.rp); return true; }
    },
    onChange(e) {
      const t = e.target;
      if (t.dataset.rt) { ensure(Store.P).text[t.dataset.rt] = t.value; Store.saveSoon(); return true; }
      if (t.dataset.rf) { setField(t.dataset.rf, t.type === 'checkbox' ? t.checked : t.value, t.type === 'checkbox' ? 'checkbox' : t.type === 'number' ? 'number' : 'text'); Store.emit('meta'); return true; }
    },
  });

  /* ======================= shell entries: top bar, menu, exports, layer, about, copy buttons ======================= */
  Registry.topbar({ id: 'report-btn', label: 'Report', title: 'The ITA report: set it up, open it ready to print, or download the data pack', icon: '▤', order: 60, run: () => UI.openTab('report') });
  Registry.menu({ id: 'report-open', label: 'Open the ITA report…', order: 250, run: () => open(Store.P) });
  Registry.menu({ id: 'report-download', label: 'Download the ITA report (HTML)', order: 251, run: () => download(Store.P) });
  Registry.menu({ id: 'report-pack', label: 'Download the ITA data pack (ZIP)', order: 252, run: () => act('pack') });
  Registry.menu({ id: 'report-setup', label: 'ITA report set-up and checklist', order: 253, run: () => UI.openTab('report') });
  Registry.exports({ id: 'report-datapack', label: 'ITA data pack (ZIP)', ext: 'zip', mime: 'application/zip', order: 90, filename: () => packName(Store.P), build: P => dataPack(P) });
  Registry.exports({ id: 'report-html', label: 'ITA report (HTML, without maps)', ext: 'html', mime: 'text/html', order: 95, filename: () => reportName(Store.P), build: P => html(P, { mapImages: {} }) });   // never maps from an older version or another project
  // Assessed sites with their 50 m crash search areas (used by the crash figure; also a map layer)
  Registry.layers({ id: 'report-sites', label: 'Report: assessed junctions (50 m crash search)', order: 90, defaultOn: false,
    draw(ctx, off, size, geo) {
      const P = Store.P; if (!P) return;
      const ids = assessedSites(P).ids, T = (typeof Render !== 'undefined' && Render.theme) || {}, r = Math.max(4, HRIG.radius / (geo.mpp || 1));
      ctx.save(); ctx.strokeStyle = T.sel || '#2459d6'; ctx.fillStyle = 'rgba(36,89,214,.10)'; ctx.lineWidth = 1.5;
      for (const id of ids) { const q = geo.nodes && geo.nodes.get(id); if (!q) continue; ctx.beginPath(); ctx.arc(q.x - off.x, q.y - off.y, r, 0, 2 * Math.PI); ctx.fill(); ctx.stroke(); }
      ctx.restore();
    },
    legend: () => '<div class="lg-title">Assessed junctions</div><div class="lg-row"><span class="lg-dot" style="background:var(--sel)"></span><span>50 m crash search area</span></div>' });
  // Walking catchments of the site (pt-active-tables): 400 / 800 / 1,200 m of network walk, drawn as straight-line
  // circles of that distance ÷ the walk factor around the centre of the development zones (Spare Construct assumption)
  Registry.layers({ id: 'report-catchments', label: 'Report: walking catchments of the site', order: 91, defaultOn: false,
    draw(ctx, off, size, geo) {
      const P = Store.P; if (!P || !geo || !geo.project) return;
      const sel = pick(P), ctr = siteCentre(sel.with || sel.base); if (!ctr) return;
      const q = geo.project(ctr), mpp = geo.mpp || 1;
      ctx.save(); ctx.strokeStyle = '#0c8599'; ctx.setLineDash([6, 4]); ctx.lineWidth = 1.5; ctx.font = '600 11px Barlow, sans-serif'; ctx.fillStyle = '#0c8599'; ctx.textAlign = 'center';
      for (const m of CATCH) { const r = m / WALK_FACTOR.value / mpp; ctx.beginPath(); ctx.arc(q.x - off.x, q.y - off.y, r, 0, 2 * Math.PI); ctx.stroke(); ctx.fillText(`${U.fmt.int(m)} m`, q.x - off.x, q.y - off.y - r - 3); }
      ctx.restore();
    },
    legend: () => `<div class="lg-title">Walking catchments</div><div class="lg-row"><span class="lg-sw" style="background:#0c8599"></span><span>${CATCH.map(m => U.fmt.int(m)).join(' / ')} m walk (straight line ÷ ${WALK_FACTOR.value})</span></div>` });
  Registry.about({ id: 'report', title: 'ITA report and data pack', order: 90, intro: 'Built in your browser from the project; nothing is uploaded.',
    rows: () => [['Report structure', 'AT ITA Guidelines 2025 v2; NZTA RR 422 Table 6.3', '<b>Rules as data.</b> RR 422 rows from Spare Construct\'s research spec (verified: no).'],
      ['Crash risk', 'NZTA CEC (May 2026) severity indices; HRIG collective and personal risk', '<b>Indicative.</b> Coefficients not yet checked against the source.'],
      ['Disclaimers', 'docs/regulatory/_disclaimers.md', `Version ${esc(DISCLAIMERS.version)}; printed where the placement rules require.`]] });
  // 'Copy table' on every results table in the drawer (text/html plus TSV, writeText fallback)
  async function copyTable(t) {
    const rows = Array.from(t.rows).map(r => Array.from(r.cells).map(c => (c.innerText || c.textContent || '').trim())), text = tsv(rows), n = Math.max(0, rows.length - (t.querySelector('th') ? 1 : 0));
    try {
      if (typeof ClipboardItem !== 'undefined' && navigator.clipboard && navigator.clipboard.write) await navigator.clipboard.write([new ClipboardItem({ 'text/html': new Blob([t.outerHTML], { type: 'text/html' }), 'text/plain': new Blob([text], { type: 'text/plain' }) })]);
      else await navigator.clipboard.writeText(text);
      toast(`Copied ${n} rows. Paste into Excel or Word.`);
    } catch {
      try { await navigator.clipboard.writeText(text); toast(`Copied ${n} rows. Paste into Excel or Word.`); } catch { toast('Your browser did not allow copying. Select the table and copy it instead.', 'warn'); }
    }
    return text;
  }
  function addCopyButtons(root) {
    for (const w of root.querySelectorAll('.tbl-wrap')) {
      if (w.dataset.copyable) continue;
      w.dataset.copyable = '1';
      const t = w.querySelector('table'); if (!t || t.rows.length < 3) continue;
      const b = document.createElement('button');
      b.type = 'button'; b.className = 'btn small'; b.textContent = 'Copy table'; b.title = 'Copy this table to paste into Excel or Word';
      b.style.cssText = 'float:right;margin:0 0 4px 8px';
      b.addEventListener('click', e => { e.stopPropagation(); copyTable(t); });
      w.parentNode.insertBefore(b, w);
    }
  }
  Registry.onInit(() => {
    // headless (the harness fires init without a DOM): nothing to add buttons to
    if (typeof document === 'undefined' || !document || typeof document.getElementById !== 'function' || typeof MutationObserver === 'undefined') return;
    let body = null; try { body = document.getElementById('drawerBody'); } catch { return; }
    if (!body || typeof body.querySelectorAll !== 'function') return;
    addCopyButtons(body);
    new MutationObserver(() => addCopyButtons(body)).observe(body, { childList: true, subtree: true });
  });

  return {
    html, open, download, figures, context, build: (P = Store.P, o) => withProject(P, () => build(P, o)), chapterOf, CHAPTERS,
    packFiles, dataPack, downloadPack, zip, crc32, csvFile, COLS, FILES, packName, reportName,
    staging, stagingTrigger, triggerText, sensitivity, completeness, pick, assessedSites, crashYears, crashSites: (P = Store.P) => withProject(P, () => crashSites(P, context(P, {}))),
    pof, personalRisk, collectiveBand, personalBand, severityIndex, cecPredict, cecModelFor, pedDelay, networkDiff, settingsRows, peakStatement, conclusionText: (P = Store.P) => withProject(P, () => conclusionText(P, context(P, {}))),
    tsv, copyTable, DISCLAIMERS, disclaimer: dText, ATTRIB, SCOPES, LEVEL, RR422, STANDARDS, R, DOC_SCRIPT, CSP, WORSEN,
  };
})();
