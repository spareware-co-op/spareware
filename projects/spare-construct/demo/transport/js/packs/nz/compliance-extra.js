'use strict';
/* Spare Construct · Compliance+ (owner: compliance-extra lane). The second compliance module (docs/regulatory, docs/spec):
   - statutory scope and wording linter (reg-scope-lint)
   - precinct staging tracker: I450 Drury Centre and I451 Drury East as data, others addable (reg-precinct)
   - mitigation register, trigger finder and condition parameters (reg-conditions, mitigation-staging)
   - options and alternatives across the intervention hierarchy (reg-alternatives)
   - Fast-track package (reg-ftaa); plan change mode: process dates, s32 table, full build-out (reg-plan-change)
   - speed limit validator and CBDS (reg-speed); ANOP indicative multimodal LoS and fit (anop)
   - parking, accessible, cycle, end-of-trip and loading (parking-compliance, reg-parking-ext)
   - urban subdivision (reg-subdivision); policy alignment (reg-policy-alignment)
   - IPM 2027-30 stage 2 metrics (reg-gps-metrics); funding attribution (reg-funding-attribution)
   - time-of-use screening (reg-tou); climate hazard assessment table (climate-overlay); hand-over packs (reg-handover)

   Everything is a pure function of the project (so it tests headlessly), plus the "Compliance+" drawer tab, link and zone
   inspector sections, report sections, exports, settings, about rows and checklist items. The Compliance lane exposes no
   section API, so these live in their own tab. Rules and thresholds are versioned data (STD) with source, edition and a
   verified flag; a value Spare Construct chose itself says 'Spare Construct assumption' (D13). Spare Construct never writes condition wording,
   only parameters.

   Data: project-level under Store.P.complianceExtra; per-link attributes under L.cx (rule class, proposed limit, variable
   reason, new road, school gate, measured mean speed, RASF place/movement, ONF); per-zone under z.cx (zoned capacity,
   income proxy). The regulatory context (pathway, lodgement and notification dates, land interest) is read from the
   Compliance tab (Compliance.state(P).ctx and Compliance.regime(P)); this tab keeps fallbacks for when it is absent.

   Deviations from the specs (also in the lane report):
   - Spare Construct models AM and PM only. The I450.9(4) inter-peak test uses an 'IP' result if one exists, otherwise the
     inter-peak LoS the author enters (from SIDRA).
   - The number of accessible car parks is indicative (NZS 4121:2001 Table 2 via NZBC D1/AS1, marked verify): E27.6.3.2A
     says when they are required; the Building Code sets how many.
   - E27.6.2.5 cycle rates for food and beverage and other retail are not encoded (the spec says 'as listed in the
     source'); enter them. AUP parking maxima are entered per row (they vary by zone). The optional parking-supply
     mode-share constraint (reg-parking-ext) is not built.
   - Time-of-use charges are modelled as extra free-flow time on the charged links (charge ÷ value of time), because the
     assignment has no toll term; the time actually added is measured per arc (the 5 km/h speed floor caps it on short
     links, which are listed). One elastic-demand step, applied as one factor to the whole OD matrix. Light and heavy
     charges are averaged with the HV share: Spare Construct has one vehicle class, so HCV and general-traffic travel times are
     also the same (reg-gps-metrics says so on the output). Benefits are rule-of-half user benefits on generalised cost.
   - Reliability (85th/median) is N/A unless the author enters observed ratios; Spare Construct does not model variability.
   - ANOP speeds are per direction; Table ANOP-1 and anop.csv carry the worse direction, the ANOP speeds export both. */

const ComplianceExtra = (() => {
  const esc = s => U.esc(s == null ? '' : String(s)), F = U.fmt;
  const num = (v, d = 0) => (v == null || !isFinite(v) ? '–' : F.num(v, d));
  const isObj = v => v && typeof v === 'object' && !Array.isArray(v);
  const nn = v => (v == null || v === '' || !isFinite(+v) ? null : +v);
  const LETTERS = ['A', 'B', 'C', 'D', 'E', 'F'];
  const C_ = () => (typeof Compliance !== 'undefined' ? Compliance : null);   // the Compliance lane may load after this file

  /* =====================================================================================
     1. Rules and standards as data (cross-cutting rule 4)
     ===================================================================================== */
  const VERSION = '2026-10-09';
  const ASSUMPTION = 'Spare Construct assumption';
  const SRC = {
    rma: 'https://www.legislation.govt.nz/act/public/1991/69/en/latest/',
    pa: 'https://www.legislation.govt.nz/act/public/2026/70/en/latest/',
    ftaa: 'https://www.legislation.govt.nz/act/public/2024/56/en/latest/',
    e27: 'https://unitaryplan.aucklandcouncil.govt.nz/Images/Auckland%20Unitary%20Plan%20Operative/Chapter%20E%20Auckland-wide/4.%20Infrastructure/E27%20Transport.pdf',
    e38: 'https://unitaryplan.aucklandcouncil.govt.nz/images/Auckland%20Unitary%20Plan%20Operative/Chapter%20E%20Auckland-wide/6.%20Subdivision/E38%20Subdivision%20-%20Urban.pdf',
    i450: 'https://unitaryplan.aucklandcouncil.govt.nz/images/Auckland%20Unitary%20Plan%20Operative/Chapter%20I%20Precincts/4.%20South/I450%20Drury%20Centre%20Precinct.pdf',
    i451: 'https://unitaryplan.aucklandcouncil.govt.nz/images/Auckland%20Unitary%20Plan%20Operative/Chapter%20I%20Precincts/4.%20South/I451%20Drury%20East%20Precinct.pdf',
    pc120: 'https://www.aucklandcouncil.govt.nz/content/dam/ac/docs/unitary-plan/aup-modification-schedule-2026.pdf',
    atita: 'https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf',
    speed: 'https://www.nzta.govt.nz/assets/resources/rules/docs/land-transport-rule-setting-of-speed-limits-2024-as-at-15-january-2025.pdf',
    speedGuide: 'https://www.nzta.govt.nz/assets/Safety/docs/speed-management-resources/guidance-setting-of-speed-limits-rule-2024.pdf',
    anop: 'https://at.govt.nz/media/1991103/application-of-the-auckalnd-network-operating-plan-august-2022.pdf',
    anopTool: 'https://at.govt.nz/media/dd1ivvio/anop-assessment-tool-v10-2.xlsx',
    ipm27: 'https://www.nzta.govt.nz/assets/planning-and-investment/nltp/2027/docs/initial-draft-ipm-for-the-2027-30-nltp-august-2026.pdf',
    mbcm: 'https://www.nzta.govt.nz/assets/resources/monetised-benefits-and-costs-manual/Monetised-benefits-and-costs-manual-volume-1-procedures.pdf',
    minister: 'https://www.mcert.govt.nz/assets/Uploads/Transport/Ministerial-statement-on-land-transport-13-August-2026.pdf',
    lga: 'https://www.legislation.govt.nz/act/public/2002/84/en/latest/',
    iff: 'https://www.legislation.govt.nz/act/public/2020/47/en/latest/',
    ltma: 'https://www.legislation.govt.nz/act/public/2003/118/en/latest/',
    tou: 'https://www.legislation.govt.nz/act/public/2025/0064/latest/',
    tsdg: 'https://at.govt.nz/media/vqdmxxib/auckland-transport-traffic-signals-design-guidelines-v5.pdf',
    ssa: 'https://www.nzta.govt.nz/resources/safe-system-audit-guidelines-for-transport-projects',
    rasf: 'https://at.govt.nz/media/1983549/roads-and-streets-framework-may-2020-web.pdf',
    npsud: 'https://environment.govt.nz/publications/national-policy-statement-on-urban-development-2020-updated-may-2022/',
    npsi: 'https://environment.govt.nz/assets/publications/RMA/npsi-2025.pdf',
    gps: 'https://www.transport.govt.nz/assets/Uploads/Government-Policy-Statement-on-land-transport-2024-FINAL.pdf',
    erp2: 'https://environment.govt.nz/assets/publications/climate-change/ERP2/New-Zealands-second-emissions-reduction-plan-Amended-January-2026.pdf',
    drurydc: 'https://www.aucklandcouncil.govt.nz/content/dam/ac/docs/plans-projects-policies-reports-bylaws/misc/drury-development-contributions-policy-transport-assessment.pdf',
    gc2501: 'https://www.nzta.govt.nz/assets/resources/general-circulars/docs/25-01.pdf',
    gc2601: 'https://www.nzta.govt.nz/assets/resources/general-circulars/docs/26-01.pdf',
    nilsson: 'https://lup.lub.lu.se/search/publication/21612',
  };
  /* Reference documents saved with the project docs (docs/references) and what they change here:
     - NZTA General Circular 25/01 (6 Jan 2025): MBCM discounting 2% for years 1-30, 1.5% after, 8% sensitivity. The time-of-use
       screening discounts with the project's MBCM schedule (Engine.STANDARDS.mbcm) instead of a flat rate.
     - NZTA General Circular 26/01 (29 May 2026): MBCM v1.7.5 for BCRs commenced on or after 29 May 2026; expanded toll-road
       procedures (vol 1 pp 230-232), BCR_N / BCR_G subsidy guidance (pp 243-244), CO2 values updated to 2025 (lower), new crash
       trend factors for speed-limit categories (pp 68-69). Cited on the time-of-use and CBDS outputs.
     - AUP modification schedule 2026: PC79 (E27, E38 transport provisions) fully operative 8 May 2026; PC120 partial withdrawal
       notified 27 Aug 2026 (withdrawn precinct chapters watermarked 3 Sep 2026, map viewer 17 Sep 2026) and still lists I450,
       I451 and E38 as amended; Mill Rd designation 6786 lapse date extended (s184, 21 Sep 2026). */
  const REFS = {
    gc2501: { id: 'nzta-gc-25-01', name: 'NZTA General Circular 25/01: discount rates', edition: '6 Jan 2025', url: SRC.gc2501, verified: true },
    gc2601: { id: 'nzta-gc-26-01', name: 'NZTA General Circular 26/01: MBCM v1.7.5', edition: '29 May 2026', url: SRC.gc2601, verified: true },
    aupMods: { id: 'aup-mods-2026', name: 'AUP plan modification schedule', edition: 'as downloaded Oct 2026 (entries to 21 Sep 2026)', url: SRC.pc120, verified: true,
      pc79: 'PC79 transport provisions (E27, E38) fully operative 8 May 2026', pc120: 'PC120 partial withdrawal notified 27 Aug 2026; withdrawn precinct chapters watermarked 3 Sep 2026' },
  };
  const STD = {
    dates: { id: 'cx-dates', name: 'Statutory commencement dates', edition: 'as at 9 Oct 2026', verified: true,
      transition: '2026-10-30', rca: '2026-05-07', tou: '2026-11-18', privatePcBar: '2028-01-01',
      source: 'Planning Act 2026 Sch 1 cl 18-21 and Sch 12; LGACA s47; LTMA ss65A-65ZO commencement; RMA s80YA' },
    labels: {
      junction: 'HCM 6 / Akcelik-based, indicative; not SIDRA output',
      anop: 'Indicative — derived from AT ANOP tool v10.2 logic; confirm in AT\'s tool.',
      s32: 's32(2)(b) quantification where practicable; not MBCM-compliant',
      vkt: 'information for RMA s74(2)(d) / NPS-UD 1(e); not a pass/fail test (no VKT target in ERP2 ch 9)',
      cbds: 'Rule CBDS: not an MBCM BCR',
      ssa: 'Supports, does not replace, an independent Safe System Audit',
      gps: 'initial draft IPM 2027-30, subject to GPS 2027',
      tou: 'screening; scheme approval requires full modelling and its own business case',
      funding: 'legal allocation is the council\'s decision',
      s84a: 'binds applicant only (s84A(3))',
      limited: 'limited consideration (AT ITA s5.1)',
      pc120: 'PC120 lists this precinct as amended; check proposed text',
      pc120Withdrawal: 'Council notified a partial withdrawal of PC120 on 27 Aug 2026 (withdrawn precinct chapters are watermarked from 3 Sep 2026): check whether this chapter is still proposed (AUP modification schedule 2026)',
      e38pc120: 'PC120 annotates some E38 provisions as amended; check the proposed text',
      onf: 'ONF suggestion: similar but not identical to the Rule class',
      meanSpeed: 'Mean operating speed is measured, not modelled: a modelled speed is shown only as a placeholder',
      dcMethod: 'Council-draft method (Drury DC transport assessment); method-agnostic, ready for the development-levy methodology',
    },
    // Trigger finder defaults (reg-conditions): Spare Construct assumptions, editable per run
    /* minStorage: the queue test ignores storage shorter than this (junction-internal coding such as a roundabout circulating
       link or the gap between the carriageways of a divided road); storage is measured to the upstream junction.
       worsen: an existing deficiency (failing at 0%) counts as a trigger once the development worsens it this much (the
       report lane's WORSEN rule uses the same ΔDoS 0.05). anop: optional 'ANOP fit drops a class' criterion. */
    trigger: { id: 'cx-trigger', name: 'Staging trigger criteria', edition: ASSUMPTION, verified: false, step: 5, dos: 0.95, los: 'D', queue: true, anop: false,
      minStorage: 50, maxChain: 3000, worsen: { dos: 0.05, delay: 10 } },
    precinctTest: { id: 'cx-precinct-test', name: 'Precinct ITA tests (I450.9(4), I451.9)', edition: 'AUP July 2026 print', verified: false,
      los: 'D', similar: 1.05, similarBasis: `${ASSUMPTION}: modelled trips within 5% of the table mix count as 'similar'` },
    // Setting of Speed Limits Rule 2024 (as at 15 Jan 2025): Schedule 3 classes, Schedule 1 lengths, cl 4.3(2), 4.6, 4.9
    speed: { id: 'speed-rule-2024', name: 'Land Transport Rule: Setting of Speed Limits 2024', edition: 'consolidated as at 15 Jan 2025', url: SRC.speed, verified: false,
      classes: {
        'urban-street': { t: 1, label: 'Urban street', min: 50, max: 50 }, civic: { t: 1, label: 'Civic space', min: 10, max: 20 },
        'urban-connector': { t: 1, label: 'Urban connector', min: 50, max: 80 }, transit: { t: 1, label: 'Transit corridor', min: 80, max: 100 },
        'peri-urban': { t: 2, label: 'Peri-urban road', min: 50, max: 80 }, stopping: { t: 2, label: 'Stopping place', min: 50, max: 80 },
        rural: { t: 2, label: 'Rural road', min: 80, max: 100 }, 'rural-connector': { t: 2, label: 'Rural connector', min: 80, max: 100 },
        interregional: { t: 2, label: 'Interregional connector', min: 100, max: 100 }, expressway: { t: 2, label: 'Expressway', min: 100, max: 120 },
        beach: { t: 3, label: 'Beach', min: 10, max: 60 }, 'low-volume': { t: 3, label: 'Low-volume or low-speed road', min: 10, max: 30 },
        unsealed: { t: 3, label: 'Unsealed rural road', min: 60, max: 80 }, 'urban-active': { t: 3, label: 'Urban street with significant walking or cycling, or no footpaths', min: 40, max: 40 },
        'isz-urban': { t: 3, label: 'Urban intersection speed zone', min: 30, max: 70, crashHistory: true }, 'isz-rural': { t: 3, label: 'Rural intersection speed zone', min: 60, max: 70, crashHistory: true },
        hill: { t: 3, label: 'Mountainous or hill corridor', min: 60, max: 80 },
      },
      minLen: [[40, 300], [60, 500], [70, 700], [80, 800], [100, 2000], [120, 3000]],   // limit ≤ k → minimum section length (m)
      exceptions: { impracticable: 'impracticable (cl 4.6)', below40: 'limit under 40 km/h', school: 'school gate limit', variable: 'part of a variable limit' },
      nztaOnly: [110, 120],
      varReasons: { users: 'mix of road users', volume: 'changing volumes or congestion', ttm: 'emergency or temporary traffic management', turning: 'turning or crossing crash risk',
        climate: 'climate or weather', school: 'school', marae: 'marae', beach: 'beach or riverbed driving' },
      onf: { 'urban-street': ['Local street', 'Activity street'], civic: ['Civic space'], 'urban-connector': ['Urban connector', 'Main street'], transit: ['Transit corridor'],
        'peri-urban': ['Peri-urban road'], stopping: ['Stopping place'], rural: ['Rural road'], 'rural-connector': ['Rural connector'], interregional: ['Interregional connector'] },
      cbdsSkip: 'No CBDS is needed for new roads, school-gate limits or reversals (NZTA Rule guidance)',
    },
    // ANOP tool v10.2 (Aug 2022 application guide; hidden sheet) — spec anop
    anop: { id: 'anop-v10.2', name: 'AT ANOP Assessment Tool', edition: 'v10.2 (application guide Aug 2022)', url: SRC.anopTool, verified: false,
      speedBands: [[90, 'A'], [70, 'B'], [50, 'C'], [40, 'D'], [30, 'E']],   // % of posted, strictly above
      relBands: [[1.1, 'A'], [1.25, 'B'], [1.5, 'C'], [1.75, 'D'], [2.0, 'E']], relBasis: `85th/median bands between A ≤ 1.1 and F > 2.0 are interpolated (${ASSUMPTION}); confirm in the tool`,
      crossBands: [50, 100, 200, 400, 800], crossDelay: [10, 20, 30, 45, 60],
      score: { 'A+': 7, A: 6, B: 5, C: 4, D: 3, E: 2, F: 1, 'F-': 0 },
      cycleFacility: { separated: 'A', painted: 'C', none: { arterial: 'E', other: 'D' }, basis: `${ASSUMPTION}: separated cycleway A, painted lane C, none D (E on arterials); from AT CyclingService TYPEOFFACILITY where loaded` },
    },
    // E27.6.2.5-.7 (July 2026 text, which includes PC79, fully operative 8 May 2026; verify)
    parking: { id: 'e27-parking', name: 'AUP E27 parking, cycle, end-of-trip and loading', edition: 'July 2026 print (PC79 fully operative 8 May 2026)', url: SRC.e27, verified: false,
      access4: ['childcare', 'school', 'tertiary', 'medical', 'hospital', 'community'], tripsPerSpaceBasis: `${ASSUMPTION}: induced peak commuter trips per extra space (E27.8.2(4)(c))` },
    // Number of accessible car parks: NZS 4121:2001 Table 2, cited by NZBC D1/AS1 (E27.6.3.2A defers to the Building Code). Verify.
    accessible: { id: 'nzs4121-t2', name: 'NZS 4121:2001 Table 2 (accessible car parks), via NZBC D1/AS1', edition: 'NZS 4121:2001', verified: false,
      rows: [[20, 1], [50, 2]], per: 50, basis: '1 for up to 20 car parks, 2 for 21-50, then 1 more for every 50 or part' },
    // Nilsson power model (CBDS crash forecast): fatal ∝ v^4, fatal + serious ∝ v^3, all injury ∝ v^2
    nilsson: { id: 'nilsson-power-model', name: 'Power model of speed and road safety', edition: 'Nilsson G (2004), Lund University Bulletin 221', url: SRC.nilsson, verified: false, fatal: 4, dsi: 3, injury: 2 },
    e38: { id: 'e38-rear', name: 'AUP E38 Table E38.8.1.2.1 rear site access', edition: 'July 2026 print', url: SRC.e38, verified: false,
      rows: [{ max: 1, legal: 3.0, formed: 2.5, len: 50 }, { max: 5, legal: 3.5, formed: 3.0, len: 50 }, { max: 10, legal: 6.9, formed: 5.5, len: 100, ped: 1.4 }], maxSites: 10 },
    gps: { id: 'ipm-2027-30-app1', name: 'IPM 2027-30 Appendix 1 GPS ratings', edition: 'Initial draft, August 2026', url: SRC.ipm27, verified: false, draft: true, by: 2035, complete: 0.75 },
    tou: { id: 'ltma-tou', name: 'LTMA Part 2 subpart 3 (time-of-use charging)', edition: '2025 No 64, in force 18 Nov 2026', url: SRC.tou, verified: true,
      appraisal: 'User benefit by the rule of half on generalised cost (time plus the charge); the charge is a transfer, so revenue offsets the cost in BCR_G and nets out of BCR_N (MBCM v1.7.5 toll-road and BCR_N/BCR_G guidance, NZTA GC 26/01)' },
    ftaa: { id: 'ftaa-clock', name: 'FTAA timeframes', edition: 'FTAA 2024 as at 10 Jul 2026', url: SRC.ftaa, verified: false, s54: 20, s55: 5, s67: 10, lapseYears: 2, dcMonths: 6 },
    conflicts: { id: 'conflict-points', name: 'Vehicle conflict points by junction form', edition: 'FHWA roundabout guide convention', verified: false, x4: 32, x3: 9, roundabout4: 8, roundabout3: 6 },
  };

  // NZ disclaimers used here (exact wording, docs/regulatory/_disclaimers.md); the Compliance lane's copy wins when present
  const D = {
    D1: 'Spare Construct is a browser-based sketch and screening tool. It is not an approved regional transport model and it is not the Auckland Forecasting Centre Macro Strategic Model (MSM). Its results are indicative and are intended to scope, test and support a transport assessment. They must be reviewed, and where necessary confirmed with approved tools, by a suitably qualified transport planner or traffic engineer before they are relied on in any resource consent, plan change, notice of requirement, Fast-track application or funding decision.',
    D3: 'Junction delay, degree of saturation, level of service and queue results are calculated by Spare Construct using HCM 6 / Akcelik-based methods. They are indicative and are not SIDRA output. Any junction result relied on for a consent decision, signal design (ATOC Traffic Signals Design Guidelines) or condition should be confirmed in SIDRA or another tool accepted by the road controlling authority.',
    D4: 'The benefit-cost ratio is an indicative screening estimate following NZTA Monetised Benefits and Costs Manual v1.7.5 conventions. It is equivalent to an Indicative Efficiency Rating, not a funding-approval BCR. It is not MBCM-compliant unless the full procedures are applied to a model calibrated and validated to the NZTA Transport Model Development Guidelines and independently reviewed. IPM efficiency bands for 2027-30 are shown from NZTA\'s initial draft (August 2026) and may change after GPS 2027.',
    D5: 'ANOP levels of service are indicative Spare Construct estimates. Confirm them in Auckland Transport\'s ANOP Assessment Tool using AT speed data before relying on them.',
    D8: 'Crash histories are taken from NZTA CAS and the most recent months may be incomplete. Spare Construct\'s safety screening and audit briefing pack support, but do not replace, a Safe System Audit by an independent, qualified specialist, or sight distance assessment to Austroads Guide to Road Design Part 4A.',
    D9: 'This cost benefit disclosure statement follows the structure of clause 3.3 of the Land Transport Rule: Setting of Speed Limits 2024. It is not an MBCM benefit-cost ratio and cannot be used in place of MBCM procedures for NLTF funding.',
    D12: 'This export is prepared for use in [tool]. Results produced in that tool, not the Spare Construct estimates, should be reported where the tool is required or accepted by the reviewing authority.',
    D13: 'Values marked "Spare Construct assumption" are Spare Construct defaults, not published rules. Agree them with the reviewing authority at scoping or replace them with project-specific values.',
  };
  function disclaimer(id, P = Store.P, tool) {
    const C = C_();
    try { if (C && C.DATA && C.DATA.DISCLAIMERS && C.DATA.DISCLAIMERS[id]) return C.disclaimer(id, P, tool ? { tool } : {}); } catch { }
    const t = D[id] || '';
    return tool ? t.replace('[tool]', tool) : t;
  }
  const disc = (id, P, tool) => `<p class="note disclaimer" data-disclaimer="${id}"><b>${id}.</b> ${esc(disclaimer(id, P, tool))}</p>`;

  // Spare Construct assumptions the user can change (Settings › Compliance+); registered defaults only fill gaps
  const ASSUME = {
    tripsPerSpace: 0.5,      // induced peak commuter trips per extra parking space (E27.8.2(4)(c))
    walkSpeed: 1.33,         // m/s
    cycleSpeed: 15,          // km/h, for accessibility catchments
    catchMin: 30,            // minutes, accessibility catchment (NPS-UD 1(c))
    shBuffer: 200,           // m: a state highway this close to a development zone suggests NZTA as an s53(2)(l) invitee
    meanShare: 0.5,          // estimated change in mean speed as a share of the limit change (CBDS)
    // Nilsson (2004) power model exponents (CBDS forecast): fatal and serious crashes ∝ v^3, injury crashes ∝ v^2. These
    // replace 'crashElasticity' (4, Nilsson's fatality exponent, wrongly applied to all DSI crashes), which is no longer read.
    dsiExponent: 3,
    injuryExponent: 2,
    elasticity: -0.3,        // demand elasticity to generalised cost (time-of-use screening)
    vocPerKm: 0.30,          // NZ$ per vehicle-km
    co2PerT: 100,            // NZ$ per tonne CO2e
    crashPerKm: 0.05,        // NZ$ social cost of crashes per vehicle-km
    affectedPct: 5,          // % flow change that makes a link 'affected' when no extent test is stored
    speedUp: 5,              // km/h modelled speed increase that counts as 'raises vehicle speed' (policy table)
  };
  const SET_LABELS = {
    tripsPerSpace: ['Induced trips per extra parking space', 'trips/space'], walkSpeed: ['Walking speed', 'm/s'], cycleSpeed: ['Cycling speed', 'km/h'],
    catchMin: ['Accessibility catchment', 'min'], shBuffer: ['State highway proximity for NZTA as invitee', 'm'], meanShare: ['Mean-speed change as a share of the limit change', '×'],
    dsiExponent: ['Fatal and serious crashes: power-model exponent (Nilsson)', '×'], injuryExponent: ['Injury crashes: power-model exponent (Nilsson)', '×'], elasticity: ['Demand elasticity to generalised cost', '×'], vocPerKm: ['Vehicle operating cost', 'NZ$/veh-km'],
    co2PerT: ['Carbon value', 'NZ$/t CO2e'], crashPerKm: ['Crash cost', 'NZ$/veh-km'], affectedPct: ['Affected link: flow change', '%'], speedUp: ['Speed increase that counts as raising speed', 'km/h'],
  };
  // Only keys Spare Construct still uses are read (a saved project may keep retired keys such as crashElasticity)
  const A = (P = Store.P) => { const s = (P && P.settings && P.settings.complianceExtra) || {}, out = { ...ASSUME };
    for (const k of Object.keys(ASSUME)) if (s[k] != null && s[k] !== '' && isFinite(+s[k])) out[k] = +s[k];
    return out; };

  /* =====================================================================================
     2. Project record (Store.P.complianceExtra)
     ===================================================================================== */
  const BLANK = () => ({
    v: 1,
    ctx: { pathway: '', lodgement: '', notificationDate: '', landInterest: '', sigEffect: false, reportDate: '', mbcmFull: false },
    lint: { text: '', ignore: {}, override: {} },
    precinct: { id: '', register: [], proposal: { ref: '', dwellings: 0, retail: 0, commercial: 0, community: 0 }, upgrades: {}, augier: false, amendment: false, ipLos: '', ipNode: '', custom: [] },
    mitigation: [], conditions: [], triggers: null, trigOpts: { scId: '', upgrades: null, anop: false },
    options: [],
    ftaa: { nztaDesignation: false, kiwirailDesignation: false, invitation: '', approval: '', jobs: '', reliability: '', matrix: {} },
    planChange: { kind: '', notification: '' },
    speed: { cost: 0, consultationChanged: false, tool: 'Spare Construct sketch model', cbds: null },
    anop: { project: '', assessor: '', option: '', corridors: [] },
    parking: { rows: [] },
    subdivision: { lots: 0, accesses: [] },
    policy: { rltp: '', fdsNote: '', npsiDoc: '' },
    gps: { routes: [], destinations: [], stages: [], dsiReduction: null, collective: '', cost: null },
    funding: { assets: [], assetRoads: '', polygon: [], levy: 0, approval: '', employment: { existing: null, future: null }, result: null, shares: null },
    tou: { polygon: [], kind: 'cordon', light: 2, heavy: 6, period: 'AM', cost: 0, result: null },
    tsdg: { pct: 10, years: 10, result: null },
    rltp: { objective: '', measures: '', duration: '', costPerYear: '', sources: '' },
  });
  function fill(dst, def) { for (const [k, v] of Object.entries(def)) { if (!(k in dst) || dst[k] === undefined) dst[k] = v; else if (isObj(v) && isObj(dst[k])) fill(dst[k], v); } return dst; }
  function data(P = Store.P) {
    if (!P) return BLANK();
    if (!isObj(P.complianceExtra)) P.complianceExtra = {};
    return fill(P.complianceExtra, BLANK());
  }

  /* =====================================================================================
     3. Dates, regulatory context and authority (read from the Compliance lane when present)
     ===================================================================================== */
  const isISO = s => typeof s === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(s.slice(0, 10)) && s.length >= 10;
  const day = s => (isISO(s) ? s.slice(0, 10) : '');
  const dOf = s => new Date(day(s) + 'T00:00:00Z');
  const isoOf = d => d.toISOString().slice(0, 10);
  function today() { try { return new Intl.DateTimeFormat('en-CA', { timeZone: 'Pacific/Auckland', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date()); } catch { return isoOf(new Date()); } }
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const longDate = s => { if (!isISO(s)) return '–'; const d = dOf(s); return `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} ${d.getUTCFullYear()}`; };
  function addMonths(s, m) {
    if (!isISO(s)) return null;
    const d = dOf(s), dd = d.getUTCDate(); d.setUTCDate(1); d.setUTCMonth(d.getUTCMonth() + m);
    const last = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 0)).getUTCDate(); d.setUTCDate(Math.min(dd, last));
    return isoOf(d);
  }
  // Working days: the Compliance lane's calendar (Matariki, Mondayisation, the Act's Christmas period) when present.
  // The fallback excludes weekends and 25 Dec – 2 Jan only, and says so.
  function addWorkingDays(s, n, kind = 'ftaa') {
    const C = C_();
    try { if (C && C.addWorkingDays) { const r = C.addWorkingDays(day(s), n, kind); if (isISO(r)) return { date: r, basis: kind === 'ftaa' ? 'Legislation Act 2019 s13 working days' : 'RMA s2 working days' }; } } catch { }
    if (!isISO(s)) return { date: null, basis: '' };
    const d = dOf(s); let k = 0, g = 0;
    while (k < n && g++ < 2000) { d.setUTCDate(d.getUTCDate() + 1); const w = d.getUTCDay(), md = isoOf(d).slice(5); if (w !== 0 && w !== 6 && !(md >= '12-25' || md <= '01-02')) k++; }
    return { date: isoOf(d), basis: 'Weekends and 25 Dec – 2 Jan excluded only (public holidays not checked): confirm the date' };
  }
  const PATHS = {
    '': 'Not set', 'rc-landuse': 'Resource consent: land use', 'rc-subdivision': 'Resource consent: subdivision', precinct: 'Precinct staging consent',
    'pc-private': 'Private plan change', 'pc-council': 'Council plan change', nor: 'Notice of requirement', 'outline-plan': 'Outline plan of works',
    'ftaa-consent': 'Fast-track: resource consent', 'ftaa-nor': 'Fast-track: notice of requirement', nltf: 'NLTF funding', 'speed-limit': 'Speed limit change',
    'dc-iff': 'Growth funding (DC, IFF levy)', tou: 'Time-of-use charging scheme',
  };
  /* The context every check here uses. Fields: pathway, label, kind (rc | pc | nor | ftaa | fund | speed | dc | tou),
     lodgement, notification, asAt, side ('pre' | 'transition' | 'after' | null), landInterest ('yes' | 'no' | ''),
     sigEffect, mbcmFull, source ('Compliance tab' | 'Compliance+'). */
  function context(P = Store.P) {
    const own = data(P).ctx, C = C_();
    let ext = null, reg = null;
    try { if (C && C.state) { ext = C.state(P).ctx; reg = C.regime(P); } } catch { ext = null; }
    const pick = k => (ext && ext[k] != null && ext[k] !== '' ? ext[k] : own[k]);
    const pathway = String(pick('pathway') || ''), lodgement = day(pick('lodgement')), notification = day(pick('notificationDate'));
    const asAt = day(own.reportDate) || lodgement || today();
    const kind = /^rc-|^precinct$/.test(pathway) ? 'rc' : /^pc-/.test(pathway) ? 'pc' : pathway === 'nor' || pathway === 'outline-plan' ? 'nor' : /^ftaa/.test(pathway) ? 'ftaa'
      : pathway === 'nltf' ? 'fund' : pathway === 'speed-limit' ? 'speed' : pathway === 'dc-iff' ? 'dc' : pathway === 'tou' ? 'tou' : '';
    const side = reg && reg.side ? reg.side : lodgement ? (lodgement < STD.dates.transition ? 'pre' : 'transition') : null;
    return {
      pathway, label: PATHS[pathway] || pathway || 'Not set', kind, lodgement, notification, asAt, side,
      post: (lodgement || asAt) >= STD.dates.transition, rmaProc: ['rc', 'nor', 'ftaa'].includes(kind) || pathway === 'ftaa-consent',
      norLike: pathway === 'nor' || pathway === 'outline-plan' || pathway === 'ftaa-nor', ftaa: kind === 'ftaa',
      landInterest: String(pick('landInterest') || ''), sigEffect: !!own.sigEffect, mbcmFull: !!own.mbcmFull,
      regimeText: reg ? reg.text : !lodgement ? 'Lodgement date not set: regime not determined' : side === 'pre' ? 'RMA as in force immediately before 30 Sep 2026' : 'RMA as at lodgement incl. Planning Act Sch 12 Pt 2',
      source: ext ? 'Compliance tab' : 'Compliance+',
    };
  }
  // Version stamp line (the Compliance lane's when present)
  function stamp(P = Store.P) {
    const C = C_();
    try { if (C && C.stamp) return C.stamp(P).text; } catch { }
    const c = context(P);
    return `${c.label} · ${c.regimeText} · AUP July 2026 print · AT ITA Guidelines 2025 v2 · MBCM v1.7.5 · Speed Rule as at 15 Jan 2025 · Compliance+ data ${VERSION} · as at ${c.asAt}`;
  }
  const isSH = (L, P = Store.P) => { const C = C_(); try { if (C && C.shOf) return !!C.shOf(P, L).sh; } catch { } return !!L && (L.cls === 'motorway' || L.cls === 'ramp' || /\b(SH|State Highway)\s?\d/i.test(L.name || '')); };
  function authorityOf(L, P = Store.P, sc = Store.sc) {
    const C = C_();
    try { if (C && C.linkAuthority) { const a = C.linkAuthority(P, L, sc); return { name: a.name, decision: a.decision.who, nzta: a.operator === 'nzta' }; } } catch { }
    if (isSH(L, P)) return { name: 'NZTA (state highway)', decision: 'NZTA', nzta: true };
    const post = context(P).asAt >= STD.dates.rca;
    return { name: post ? 'Auckland Council (road controlling authority)' : 'Auckland Transport (road controlling authority)', decision: post && (L.cls === 'local' || L.cls === 'collector') ? 'The local board (LGACA s47C, Sch 4)' : post ? 'Auckland Council governing body' : 'Auckland Transport', nzta: false };
  }

  /* =====================================================================================
     4. Shared model helpers
     ===================================================================================== */
  const PERIODS = ['AM', 'PM', 'IP'];
  const periodsOf = sc => PERIODS.filter(p => sc && sc.res && sc.res[p] && !sc.res[p].mismatch);
  const resOf = (sc, p) => (sc && sc.res && sc.res[p]) || null;
  const scById = (P, id) => (P.scenarios || []).find(s => s.id === id) || null;
  const activeSc = P => scById(P, P.active) || P.scenarios[0];
  const baseSc = P => scById(P, P.base) || P.scenarios[0];
  const roleSc = (P, role) => P.scenarios.find(s => s.role === role) || null;
  function pair(P = Store.P) {
    const opt = activeSc(P), mit = roleSc(P, 'mitigation');
    let base = baseSc(P);
    if (base === opt) base = roleSc(P, 'baseline') && roleSc(P, 'baseline') !== opt ? roleSc(P, 'baseline') : base;
    return { base, opt, mit, same: base === opt };
  }
  const nodeName = (sc, id) => { try { return Kit.junctionName(id, sc); } catch { } const n = [...new Set(Net.linksAt(sc, id).map(l => l.name).filter(Boolean))]; return n.length ? n.slice(0, 3).join(' / ') : 'Unnamed junction'; };
  const roadLinks = (sc, id) => Net.linksAt(sc, id).filter(L => L.status !== 'closed');
  const isDev = z => z && z.kind !== 'gateway' && Engine.zoneClass(z) === 'development';
  const devZones = sc => Object.values(sc.zones || {}).filter(isDev);
  const luOf = (z, k) => +((z.lu || {})[k]) || 0;
  const dwellings = (z, sc) => (luOf(z, 'residential') + luOf(z, 'apartment') + luOf(z, 'retirement')) * Engine.stageOf(z, sc);
  const sumDw = sc => devZones(sc).reduce((s, z) => s + dwellings(z, sc), 0);
  const GFA_KEYS = ['retail', 'supermarket', 'fastfood', 'office', 'medical', 'industrial', 'warehouse'];
  const sumGfa = sc => devZones(sc).reduce((s, z) => s + GFA_KEYS.reduce((t, k) => t + luOf(z, k), 0) * Engine.stageOf(z, sc), 0);
  const cloneSc = (sc, over = {}) => { const c = JSON.parse(JSON.stringify({ ...sc, res: {} })); return Object.assign(c, over); };
  const arcLink = key => key.slice(0, key.lastIndexOf(':'));
  const linkName = L => (L && L.name) || (L && CLASSES[L.cls] ? `Unnamed ${CLASSES[L.cls].label.toLowerCase()} road` : 'Unnamed road');
  const posted = L => +L.speedLimit || (L.nslr && +L.nslr.v) || +L.speed || (CLASSES[L.cls] || CLASSES.local).speed;
  const pct = (a, b) => (b > 0 && isFinite(a) ? (a - b) / b * 100 : null);
  const losWorse = (a, b) => LETTERS.indexOf(a) > LETTERS.indexOf(b);
  const csv = rows => U.csv(rows);
  // A CSV with the version stamp and disclaimers in footer comment rows (Compliance.stampCSV when present)
  function stamped(text, P = Store.P, tool) {
    const C = C_();
    try { if (C && C.stampCSV) return C.stampCSV(text, P, tool ? { handover: tool } : {}); } catch { }
    const foot = [['# ' + disclaimer('D1', P)], ['# Version stamp: ' + stamp(P)]];
    if (tool) foot.push(['# ' + disclaimer('D12', P, tool)]);
    return String(text).replace(/(\r?\n)*$/, '') + '\r\n' + csv(foot);
  }
  async function runSc(sc, p, opts = {}) { return Engine.assign(sc, { period: p, ...opts }); }
  const T = o => Kit.table(o);
  const note = (t, warn) => `<p class="note${warn ? ' warn' : ''}">${t}</p>`;
  // Tones map onto the pill classes css/app.css defines (as the Compliance lane does): ok → .conv.ok, warn → .proposed, bad → .closed
  const PILL_CLS = { ok: ' conv ok', warn: ' proposed', bad: ' closed' };
  const pill = (t, tone) => `<span class="pill${PILL_CLS[tone] || ''}">${t}</span>`;
  const verify = s => (s && s.verified === false ? ' <span class="pill warn" title="Not yet checked against the source document">verify</span>' : '');
  const link = (url, t) => `<a href="${esc(url)}" target="_blank" rel="noopener">${esc(t)}</a>`;
  const assumed = t => `${t} <span class="note">(${ASSUMPTION})</span>`;

  /* =====================================================================================
     5. Statutory scope and wording linter (reg-scope-lint)
     Each rule: when(ctx) says whether it applies; re (and also, both in one sentence) finds it; level 'block' stops
     exports until the text is changed.
     ===================================================================================== */
  const RCA_RE = [/\bAuckland Transport\b[^.;]{0,30}?\b(?:road controlling authority|RCA)\b/i, /\bAT\b(?:\s*\))?[^.;]{0,20}?\b(?:road controlling authority|RCA)\b/, /\broad controlling authority\b[^.;]{0,6}(?:\(|,|:|is|was)?\s*(?:the\s+)?(?:Auckland Transport\b|AT\b)/];
  const LINT = [
    { id: 'scope-1a', rule: 'Out-of-scope effects for a district land use consent', source: 'RMA s104(1A), s92(2AA) (Planning Act 2026 Sch 12 Pt 2)', level: 'warn',
      when: c => (c.kind === 'rc' || c.pathway === 'ftaa-consent') && c.pathway !== 'rc-subdivision' && c.side === 'transition',
      re: /\b(retail distribution|trade competition|demand for (?:the )?(?:project|proposal|development|activity)|viability of (?:the )?(?:project|proposal|development|activity|centre)|views? from private (?:property|properties|land))\b/i,
      suggest: 'Remove it: for consents lodged from 30 Oct 2026 these effects are out of scope (s104(1A)), and a further-information request about them is barred (s92(2AA)). Traffic effects stay in scope.' },
    { id: 'scope-cl7', rule: 'Traffic safety placed under Sch 4 cl 7(1)(f)', source: 'RMA Sch 4 cl 6(1)(b), 7(1)(a)', level: 'warn', when: () => true,
      re: /\b(?:cl(?:ause)?\.?\s*7\s*\(1\)\s*\(f\))/i, also: /\bsafety\b/i,
      suggest: 'Assess traffic safety under Sch 4 cl 6(1)(b) and cl 7(1)(a) (effects on people and the community), not cl 7(1)(f).' },
    { id: 'scope-95a9', rule: 's95A(9) special circumstances', source: 'RMA s95A(9) (repealed from 30 Oct 2026); AUP E27.5, C1.13; s95B(10)', level: 'warn',
      when: c => c.post && c.kind !== 'pc', re: /\bs(?:ection)?\s?95A\s*\(9\)|\bspecial circumstances\b/i,
      suggest: 's95A(9) is repealed for applications lodged from 30 Oct 2026, so the AUP E27.5 / C1.13 cross-references to it have no target. s95B(10) (limited notification special circumstances) remains: cite that if it is what you mean.' },
    /* Blocks only in the transition regime (lodged from 30 Oct 2026, before the specified transition date): s104(1C) does not
       exist for earlier lodgements. Without a lodgement date it warns. A sentence that disclaims reliance ('we have not relied
       on …', 's104(1C) prevents reliance on …') is correct and is skipped (skip). */
    { id: 'scope-1c', rule: 'Planning Act or NEA instrument cited in a transition consent', source: 'RMA s104(1C) (Planning Act 2026 Sch 12 Pt 2)',
      level: c => (c.side === 'transition' ? 'block' : 'warn'),
      when: c => (c.kind === 'rc' || c.kind === 'nor' || c.kind === 'ftaa') && (c.side === 'transition' || !c.lodgement),
      re: [/\bNational Policy Direction\b/i, /\bNatural Environment Act\b/i, /\bNEA\b/, /\bPlanning Act 2026\b[^.;]{0,40}\b(?:national standards?|regional plans?|instruments?|directions?)\b/i, /\bnational standards?\b[^.;]{0,30}\bPlanning Act\b/i],
      skip: [/\b(?:not|never|no|nor|without)\b[^.;]{0,50}\b(?:rel(?:y|ied|ies|iance|ying)|cit(?:e|ed|es|ing)|regard|consider(?:ed|ing)?|appl(?:y|ied|ies|ying)|give effect)\b/i,
        /\bs(?:ection)?\s?104\s*\(1C\)[^.;]{0,80}\b(?:bars?|prevents?|prohibits?|precludes?|must not|cannot|may not|does not (?:allow|permit))\b/i,
        /\b(?:bars?|prevents?|prohibits?|precludes?|excludes?)\b[^.;]{0,60}\breliance\b/i],
      suggest: 'Remove the citation: a consent or NoR processed under the RMA transition regime must not rely on National Policy Direction or other Planning Act or NEA instruments (s104(1C)). Report and package exports stay blocked until it is removed, or until the author records why the sentence is correct (Override).',
      warnText: 'Set the lodgement date: if this is lodged on or after 30 Oct 2026, s104(1C) bars reliance on Planning Act or NEA instruments and this citation blocks the report exports.' },
    { id: 'scope-e27-ita', rule: '"ITA required" tied to E27.6.1', source: 'AUP E27.6.1, E27.9 (an ITA may be required)', level: 'warn', when: () => true,
      re: /\b(?:an? )?(?:ITA|integrated transport assessment)\s+(?:is\s+)?(?:required|mandatory)\b|\brequires? an? (?:ITA|integrated transport assessment)\b/i, also: /E27\.6\.1/,
      suggest: 'Say "E27.6.1 may require an ITA": exceeding a Table E27.6.1.1 threshold makes the activity restricted discretionary, and the assessment matters decide what information is needed.' },
    { id: 'scope-rca', rule: 'Auckland Transport named as road controlling authority', source: 'LGACA 2009 s47 (from 7 May 2026)', level: 'warn',
      when: c => c.asAt >= STD.dates.rca, re: RCA_RE,
      suggest: 'Since 7 May 2026 Auckland Council is the road controlling authority (LGACA s47), with local boards on local and collector roads; Auckland Transport continues as the transport CCO. Quote AUP text that names Auckland Transport as written.' },
    { id: 'scope-mbcm', rule: '"MBCM-compliant" BCR without the full procedure and a validated model', source: 'NZTA MBCM v1.7.5; D4', level: 'warn',
      when: (c, P) => !(c.mbcmFull && modelValidated(P)), re: /\bMBCM[- ]?compliant\b|\bcompliant with (?:the )?MBCM\b/i,
      suggest: 'Call it an indicative BCR (equivalent to an Indicative Efficiency Rating). It is MBCM-compliant only when the full procedures are applied to a TMDG-validated, independently reviewed model.' },
    { id: 'scope-tmdg', rule: 'TMDG statistics presented as fitness for purpose', source: 'NZTA TMDG 2019; D6', level: 'warn', when: () => true,
      re: /\bTMDG\b|\bTransport Model Development Guidelines\b/i, also: /\bfit[- ]for[- ]purpose\b|\bfitness for purpose\b/i,
      suggest: 'Report the TMDG statistics as evidence, not as a pass that proves fitness for purpose: that is the analyst\'s and peer reviewer\'s judgment (D6).' },
    { id: 'scope-rasf', rule: 'RASF described as mandatory for developers', source: 'AT Roads and Streets Framework v2 (not mandated for developers)', level: 'warn', when: () => true,
      re: /\bRASF\b|\bRoads and Streets Framework\b/i, also: /\b(?:mandatory|must comply|is required|requires developers?|obliged|binding)\b/i,
      suggest: 'Describe the RASF as advisory: it is not mandated for developers (RASF v2). Use it to explain the street\'s place and movement role.' },
  ];
  const modelValidated = P => { try { return ['AM', 'PM'].some(p => Engine.modelStatus(P, p).status === 'validated'); } catch { return false; } };
  // The text the linter reads: this tab's text box, the report's author text, and free text in the mitigation and options registers
  function lintSources(P = Store.P) {
    const cx = data(P), out = [];
    if (String(cx.lint.text || '').trim()) out.push({ where: 'Compliance+ text', text: cx.lint.text });
    const rt = (P.report && P.report.text) || {};
    for (const [k, v] of Object.entries(rt)) if (typeof v === 'string' && v.trim()) out.push({ where: `Report: ${k}`, text: v });
    for (const m of cx.mitigation) for (const k of ['measure', 'options', 'trigger']) if (m[k]) out.push({ where: `Mitigation ${m.ref || ''}`.trim(), text: String(m[k]) });
    for (const o of cx.options) if (o.notes) out.push({ where: `Option ${o.name || ''}`.trim(), text: String(o.notes) });
    return out;
  }
  const sentences = t => String(t).split(/(?<=[.!?])\s+|\n+/).map(s => s.trim()).filter(Boolean);
  /* lint(P, text?) → { findings [{ id, rule, source, level, where, sentence, match, suggest }], blocking, applies [ids] }.
     With text, only that text is checked (the project still sets the regime). */
  /* A blocking finding can be released only by an author override that records why the sentence is correct
     (data.lint.override[key] = { reason, at }); warnings can be dismissed (data.lint.ignore[key]). */
  function lint(P = Store.P, text) {
    const c = context(P), lx = data(P).lint, ign = lx.ignore || {}, ovr = lx.override || {}, findings = [];
    const applies = LINT.filter(r => { try { return r.when(c, P); } catch { return false; } });
    const srcs = text != null ? [{ where: 'Text', text }] : lintSources(P);
    for (const s of srcs) for (const sen of sentences(s.text)) for (const r of applies) {
      const res = Array.isArray(r.re) ? r.re : [r.re];
      const m = res.map(x => x.exec(sen)).find(Boolean);
      if (!m || (r.also && !r.also.test(sen)) || (r.skip && r.skip.some(x => x.test(sen)))) continue;
      const key = r.id + '|' + sen.slice(0, 80), level = typeof r.level === 'function' ? r.level(c) : r.level;
      if (ign[key] && level !== 'block') continue;
      const o = level === 'block' && ovr[key] && String(ovr[key].reason || '').trim() ? ovr[key] : null;
      findings.push({ id: r.id, key, rule: r.rule, source: r.source, level, where: s.where, sentence: sen.length > 240 ? sen.slice(0, 237) + '…' : sen, match: m[0],
        suggest: level === 'block' || !r.warnText ? r.suggest : r.warnText, overridden: o ? { reason: String(o.reason), at: o.at || '' } : null });
    }
    return { findings, blocking: findings.filter(f => f.level === 'block' && !f.overridden), applies: applies.map(r => r.id), context: c };
  }
  // Record (or clear, with an empty reason) the author's reason for releasing a blocking finding
  function overrideLint(P = Store.P, key, reason) {
    const lx = data(P).lint; lx.override = lx.override || {};
    if (String(reason || '').trim()) lx.override[key] = { reason: String(reason).trim(), at: today() }; else delete lx.override[key];
    return lx.override[key] || null;
  }
  // Exports that carry author text are refused while a blocking finding remains (reg-scope-lint acceptance 2)
  function exportBlock(P = Store.P) {
    if (!P) return null;
    const b = lint(P).blocking;
    return b.length ? `Export blocked by the scope linter: ${b[0].rule} ("${b[0].match}" in ${b[0].where}). ${b[0].suggest}` : null;
  }

  /* =====================================================================================
     6. Precinct staging tracker (reg-precinct)
     Rows cap development until the listed upgrades (this row's and every earlier row's) are operational (I450.6.2(1)).
     An upgrade under construction or delivered by the applicant counts only with the s108AA(1)(a) Augier agreement.
     ===================================================================================== */
  const UPGRADE_STATUS = { operational: 'Operational', construction: 'Under construction', applicant: 'Applicant-delivered', 'not-started': 'Not started' };
  const I450_ROWS = [
    { row: 'a', dw: 710, retail: null, commercial: null, community: null, up: [{ id: 'a1', name: 'Interim signals at Great South Rd / Waihoehoe Rd', status: 'A5' }, { id: 'a2', name: 'Interim Waihoehoe Rd upgrade', status: 'A5' }] },
    { row: 'b', dw: 1300, retail: 24000, commercial: 6000, community: 800, up: [{ id: 'b1', name: 'SH1 Papakura–Drury six-laning', status: 'A5' }] },
    { row: 'c', dw: 1800, retail: 32000, commercial: 8700, community: 1000, up: [{ id: 'c1', name: 'Drury Central station', status: 'A5' }, { id: 'c2', name: 'SH1 – Creek Rd slip lane', status: 'A6' }] },
    { row: 'd', dw: 3300, retail: 56000, commercial: 17900, community: 2000, up: [{ id: 'd1', name: 'Ultimate Waihoehoe Rd upgrade', status: 'A6' }] },
    { row: 'e', dw: 3800, retail: 64000, commercial: 21000, community: 2400, up: [{ id: 'e1', name: 'Mill Rd southern connection', status: 'A6' }, { id: 'e2', name: 'Drury South interchange', status: 'A6' }] },
    { row: 'f', dw: 5800, retail: 97000, commercial: 47000, community: 10000, up: [{ id: 'f1', name: 'Mill Rd north', status: 'A6' }, { id: 'f2', name: 'Waihoehoe East', status: 'A6' }, { id: 'f3', name: 'Ōpāheke northern connection', status: 'A6' }] },
  ];
  const PRECINCTS = {
    I450: { id: 'I450', name: 'Drury Centre', plan: 'Precinct Plan 4', table: 'Table I450.6.2.1', rule: 'I450.6.2(1)', deemed: 'I450.6.2(2)', ita: 'I450.9(4)', monitor: 'I450.9(5)',
      status: { A5: 'Non-complying (I450.4.1 (A5))', A6: 'Discretionary (I450.4.1 (A6))', A9: 'Amendment that breaches the standard (I450.4.1 (A9))' },
      node: ['Great South', 'Waihoehoe'], nodeLabel: 'Great South Rd / Waihoehoe Rd', pc120: true, source: SRC.i450, edition: 'AUP(OP) July 2026 print', verified: false, rows: I450_ROWS },
    I451: { id: 'I451', name: 'Drury East', plan: 'Precinct Plan 2', table: 'Table I451.6.2.1', rule: 'I451.6.2(1)', deemed: 'I451.6.2(2)', ita: 'I451.9', monitor: 'I451.9',
      status: { A5: 'Non-complying (check the I451 activity table)', A6: 'Discretionary (check the I451 activity table)', A9: 'Amendment that breaches the standard (check the I451 activity table)' },
      node: ['Great South', 'Waihoehoe'], nodeLabel: 'Great South Rd / Waihoehoe Rd', pc120: true, source: SRC.i451, edition: 'AUP(OP) July 2026 print', verified: false,
      note: 'Drury East uses the same thresholds as I450 (Table I451.6.2.1). The upgrade list is copied from I450: check it against Table I451.6.2.1.', rows: I450_ROWS },
  };
  const MEASURES = [['dw', 'dwellings', 'Dwellings'], ['retail', 'retail', 'Retail GFA (m²)'], ['commercial', 'commercial', 'Other commercial GFA (m²)'], ['community', 'community', 'Community GFA (m²)']];
  function precinctList(P = Store.P) {
    const out = { ...PRECINCTS };
    for (const p of data(P).precinct.custom || []) if (p && p.id && Array.isArray(p.rows)) out[p.id] = { status: { A5: 'Non-complying', A6: 'Discretionary', A9: 'Amendment' }, verified: false, edition: 'entered by the user', ...p, custom: true };
    return out;
  }
  function precinctOf(P = Store.P) {
    const cx = data(P), list = precinctList(P);
    if (cx.precinct.id && list[cx.precinct.id]) return list[cx.precinct.id];
    const C = C_();
    try { if (C && C.siteInfo) { const s = C.siteInfo(P), pr = s && (s.precinct || {}); const id = typeof pr === 'string' ? pr : pr.id || pr.code || ''; const k = Object.keys(list).find(x => String(id).includes(x) || String(pr.name || '').includes(list[x].name)); if (k) return list[k]; } } catch { }
    return null;
  }
  const deemedMet = (st, augier) => st === 'operational' || ((st === 'construction' || st === 'applicant') && !!augier);
  // The I450.9(4) junction in a scenario: the user's pick, or the node whose roads include both names
  function precinctNode(P, sc, pr) {
    const cx = data(P);
    if (cx.precinct.ipNode && sc.nodes[cx.precinct.ipNode]) return cx.precinct.ipNode;
    if (!pr || !pr.node) return null;
    const hits = Object.keys(sc.nodes).filter(id => { const names = roadLinks(sc, id).map(L => L.name || '').join(' | '); return pr.node.every(n => names.includes(n)); });
    return hits.sort((a, b) => roadLinks(sc, b).length - roadLinks(sc, a).length)[0] || null;
  }
  const mixTrips = (mix, sc) => {
    const z = { id: '_mix', kind: 'zone', lu: { residential: +mix.dw || 0, retail: +mix.retail || 0, office: +mix.commercial || 0, medical: +mix.community || 0 } };
    return ['AM', 'PM'].map(p => { const e = Engine.tripEnds(z, Store.S, p, sc); return { p, trips: e.O + e.D }; });
  };
  /* precinctCheck(P) → { precinct, rows (with upgrade states), cumulative, proposal, total, row (applicable), breach, status, why[],
     headroom, ip { node, name, los, source, pass }, trips { rows, ratio, pass }, banner, warnings[] } */
  function precinctCheck(P = Store.P, sc = activeSc(P)) {
    const pr = precinctOf(P); if (!pr) return null;
    const cx = data(P), pc = cx.precinct, ups = pc.upgrades || {};
    const reg = (pc.register || []).filter(r => r && r.excluded !== true);
    const sum = k => reg.reduce((s, r) => s + (+r[k] || 0), 0);
    const cumulative = Object.fromEntries(MEASURES.map(([k, f]) => [k, sum(f)]));
    const proposal = Object.fromEntries(MEASURES.map(([k, f]) => [k, +((pc.proposal || {})[f === 'dwellings' ? 'dwellings' : f]) || 0]));
    const total = Object.fromEntries(MEASURES.map(([k]) => [k, cumulative[k] + proposal[k]]));
    const rows = pr.rows.map(r => ({ ...r, up: r.up.map(u => { const st = ups[u.id] || 'not-started'; return { ...u, state: st, stateLabel: UPGRADE_STATUS[st] || st, met: deemedMet(st, pc.augier), needsAugier: (st === 'construction' || st === 'applicant') && !pc.augier }; }) }));
    rows.forEach(r => { r.met = r.up.every(u => u.met); });
    const idx = rows.findIndex(r => !r.met), row = idx < 0 ? rows[rows.length - 1] : rows[idx], beyond = idx < 0;
    const why = [];
    for (const [k, , label] of MEASURES) { const cap = row[k]; if (cap != null && total[k] > cap) why.push(`${label}: ${F.int(total[k])} against the row (${row.row}) cap of ${F.int(cap)}`); }
    const breach = why.length > 0;
    const unmet = row.up.filter(u => !u.met);
    let status = null;
    if (breach) status = pc.amendment ? 'A9' : beyond ? 'A6' : unmet.some(u => u.status === 'A5') ? 'A5' : 'A6';
    const headroom = Object.fromEntries(MEASURES.map(([k]) => [k, row[k] == null ? null : row[k] - total[k]]));
    // inter-peak LoS at the named junction (I450.9(4)): an IP result if there is one, otherwise the LoS the author enters
    const nodeId = precinctNode(P, sc, pr), ipRes = resOf(sc, 'IP'), nr = nodeId && ipRes && ipRes.nodes ? ipRes.nodes[nodeId] : null;
    const los = nr ? (nr.intersectionLos || nr.los) : (pc.ipLos || '').toUpperCase();
    const ip = { node: nodeId, name: nodeId ? nodeName(sc, nodeId) : pr.nodeLabel, los: los || null, source: nr ? `modelled inter-peak (${STD.labels.junction})` : los ? 'entered by the author (inter-peak, e.g. SIDRA)' : 'not available',
      pass: los ? !losWorse(los, STD.precinctTest.los) : null, rule: pr.ita, test: `inter-peak overall LoS generally no worse than ${STD.precinctTest.los}` };
    // trip equivalence: cumulative + proposed mix against the table mix at the same row
    let trips = null;
    try {
      const mixRow = { dw: row.dw, retail: row.retail || 0, commercial: row.commercial || 0, community: row.community || 0 };
      const a = mixTrips(total, sc), b = mixTrips(mixRow, sc);
      const rr = a.map((x, i) => ({ period: x.p, proposed: x.trips, table: b[i].trips, ratio: b[i].trips > 0 ? x.trips / b[i].trips : null }));
      trips = { rows: rr, pass: rr.every(x => x.ratio == null || x.ratio <= STD.precinctTest.similar), basis: STD.precinctTest.similarBasis, mapping: `${ASSUMPTION}: community GFA uses the medical-centre rate and other commercial the office rate` };
    } catch (e) { trips = { rows: [], pass: null, error: e.message }; }
    const warnings = [];
    if (rows.some(r => r.up.some(u => u.needsAugier))) warnings.push(`An upgrade under construction or delivered by the applicant counts only with express agreement under s108AA(1)(a), on an Augier basis, to conditions preventing occupation or s224(c) certification until it is operational (${pr.deemed}). Tick the agreement to rely on it.`);
    if (beyond && total.dw > row.dw) warnings.push(`The total exceeds the final row of ${pr.table}: the table does not provide for it.`);
    if (pr.note) warnings.push(pr.note);
    return { precinct: pr, rows, cumulative, proposal, total, row, rowIndex: idx < 0 ? rows.length - 1 : idx, breach, why, status, statusText: status ? pr.status[status] : 'Complies with the staging standard', headroom, ip, trips,
      banner: pr.pc120 ? STD.labels.pc120 : '', pc120Note: pr.pc120 ? STD.labels.pc120Withdrawal : '', warnings, register: reg, monitorText: `State the dwellings and GFA this application enables (${pr.monitor}).` };
  }

  /* =====================================================================================
     7. Effects, mitigation register, condition parameters and the trigger finder
     (reg-conditions, mitigation-staging)
     ===================================================================================== */
  const EFFECT = { id: 'cx-effects', name: 'Effect screening for the mitigation register', edition: ASSUMPTION, verified: false, dos: 0.9, dDelay: 10, vc: 0.9, dvc: 0.05,
    basis: `${ASSUMPTION}: a junction is an effect when its DoS is over 0.9 (or its own practical DoS), it reaches LoS E or F, or its delay rises 10 s or more; a road when v/c is over 0.9 and rises 0.05 or more` };
  /* Effects from the scenario difference (do-minimum → option). Stable ids: J:<nodeId>:<period> and L:<arcKey>:<period>. */
  function effects(P = Store.P, sc) {
    const pr = pair(P), opt = sc || pr.opt, base = pr.base, out = [];
    for (const p of periodsOf(opt)) {
      const ro = opt.res[p], rb = base && base !== opt ? resOf(base, p) : null;
      for (const [id, nr] of Object.entries(ro.nodes || {})) {
        const b = rb && rb.nodes && rb.nodes[id], dD = b ? nr.delay - b.delay : null, xp = nr.xp || EFFECT.dos;
        if (!(nr.dos > Math.min(xp, EFFECT.dos) || /[EF]/.test(nr.los || '') || (dD != null && dD >= EFFECT.dDelay))) continue;
        out.push({ id: `J:${id}:${p}`, kind: 'junction', node: id, period: p, dos: nr.dos, los: nr.los, delay: nr.delay, dDelay: dD,
          text: `${nodeName(opt, id)} ${p}: DoS ${num(nr.dos, 2)}, LoS ${nr.losText || nr.los}${dD != null ? `, ${F.signed(dD, 1)} s against ${base.name}` : ''}` });
      }
      for (const [key, a] of Object.entries(ro.arcs || {})) {
        const b = rb && rb.arcs && rb.arcs[key], dvc = b ? a.vc - b.vc : a.vc;
        if (!(a.vc > EFFECT.vc && dvc >= EFFECT.dvc)) continue;
        const L = opt.links[arcLink(key)];
        out.push({ id: `L:${key}:${p}`, kind: 'link', key, period: p, vc: a.vc, text: `${linkName(L)} (${key.endsWith(':ab') ? 'A→B' : 'B→A'}) ${p}: v/c ${num(a.vc, 2)}` });
      }
    }
    return out.sort((x, y) => (y.dos || y.vc || 0) - (x.dos || x.vc || 0));
  }
  const MECHANISMS = { condition: 'Consent condition', precinct: 'Precinct rule', agreement: 'Funding or development agreement', s84a: 'FTAA s84A condition', designation: 'Designation (NoR)', none: 'None' };
  const RESPONSIBLE = { applicant: 'Applicant', council: 'Auckland Council', at: 'Auckland Transport', nzta: 'NZTA', kiwirail: 'KiwiRail', other: 'Other third party' };
  // committed: true (applicant commits, Y), false (N) or null (not recorded): only the author can record a commitment
  const blankMit = () => ({ id: U.uid('mit'), ref: '', measure: '', effect: '', options: '', nonInfra: false, trigger: '', responsible: 'applicant', cost: null, funder: 'applicant', rltp: '', mechanism: 'condition', committed: null, landOutside: false });
  const committedText = v => (v === true ? 'Y' : v === false ? 'N' : 'Not recorded');
  // Mitigation register rows (AT ITA Appendix 3), each with its effect text and the limited-consideration tag
  function mitigationTable(P = Store.P) {
    const cx = data(P), eff = new Map(effects(P).map(e => [e.id, e])), ftaa = context(P).ftaa;
    return cx.mitigation.map((m, i) => {
      const e = eff.get(m.effect), third = (m.funder && m.funder !== 'applicant') || (m.responsible && m.responsible !== 'applicant');
      const limited = m.committed === false || third;
      return { ...m, ref: m.ref || `M${i + 1}`, effectText: e ? e.text : m.effect ? `${m.effect} (no longer in the results: re-run or relink)` : 'Not linked', effectFound: !!e,
        limited, tag: limited ? STD.labels.limited : '', mechanismText: MECHANISMS[m.mechanism] || m.mechanism || '–', responsibleText: RESPONSIBLE[m.responsible] || m.responsible || '–',
        s84a: ftaa && m.mechanism === 's84a' ? STD.labels.s84a : '' };
    });
  }
  // What the mitigation scenario changes against the option: junction controls, upgrades and new roads
  function networkDiff(a, b) {
    if (!a || !b || a === b) return [];
    const out = [];
    for (const n of Object.values(b.nodes)) {
      const o = a.nodes[n.id];
      if (o && (o.control !== n.control || (n.upgrade && !o.upgrade))) out.push({ kind: 'node', id: n.id, text: `${CONTROLS[n.control] || n.control} at ${nodeName(b, n.id)}${o.control !== n.control ? ` (was ${(CONTROLS[o.control] || o.control).toLowerCase()})` : ''}` });
    }
    for (const L of Object.values(b.links)) {
      const o = a.links[L.id];
      if (!o && L.status !== 'closed') out.push({ kind: 'link', id: L.id, text: `New road: ${linkName(L)} (${F.len(Net.len(b, L))})` });
      else if (o && ((L.lanesAB | 0) + (L.lanesBA | 0) > (o.lanesAB | 0) + (o.lanesBA | 0))) out.push({ kind: 'link', id: L.id, text: `Extra lanes on ${linkName(L)}` });
      else if (o && L.busLane && !o.busLane) out.push({ kind: 'link', id: L.id, text: `Bus lane on ${linkName(L)}` });
    }
    return out;
  }
  // Seed rows from the mitigation scenario's changes and from unaddressed effects; never overwrites a row
  function seedMitigation(P = Store.P) {
    const cx = data(P), pr = pair(P), eff = effects(P), seen = new Set(cx.mitigation.map(m => m.seed).filter(Boolean)), linked = new Set(cx.mitigation.map(m => m.effect));
    let n = 0;
    const capital = (sc, d) => { const E = P.settings.econ || {}; if (d.kind === 'node') return (E.node || {})[sc.nodes[d.id].control] ?? null; const L = sc.links[d.id]; return L ? Net.len(sc, L) / 1000 * ((L.lanesAB | 0) + (L.lanesBA | 0)) * ((E.laneKm || {})[L.cls] ?? 3) : null; };
    if (pr.mit && pr.mit !== pr.opt) for (const d of networkDiff(pr.opt, pr.mit)) {
      const seed = `diff:${d.kind}:${d.id}`; if (seen.has(seed)) continue;
      const e = d.kind === 'node' ? eff.find(x => x.node === d.id) : eff.find(x => x.key && arcLink(x.key) === d.id);
      cx.mitigation.push({ ...blankMit(), seed, measure: d.text, effect: e ? e.id : '', cost: capital(pr.mit, d), options: 'Seeded from the mitigation scenario: record the options considered, including non-infrastructure ones.' }); n++;
      if (e) linked.add(e.id);
    }
    for (const e of eff) {
      const seed = `eff:${e.id}`; if (seen.has(seed) || linked.has(e.id)) continue;
      cx.mitigation.push({ ...blankMit(), seed, measure: '', effect: e.id, mechanism: 'none', options: 'Effect not yet addressed: record a measure or why none is needed.' }); n++;
    }
    return n;
  }
  const blankCond = () => ({ id: U.uid('cond'), mitigation: '', metric: 'dwellings occupied', threshold: '', survey: '', frequency: '', response: '', effect: '', agreed: false, review: '' });
  /* Condition parameter rows (never wording). Each must reference a modelled effect (s108AA(1)(b) nexus) or carry the
     applicant's agreement (s108AA(1)(a)); FTAA conditions are labelled s84A applicant-only. */
  function conditionTable(P = Store.P) {
    const cx = data(P), ids = new Set(effects(P).map(e => e.id)), c = context(P), mits = mitigationTable(P);
    return cx.conditions.map((r, i) => {
      const m = mits.find(x => x.id === r.mitigation), effect = r.effect || (m && m.effect) || '';
      const nexus = effect && ids.has(effect) ? 'effect' : r.agreed ? 'agreed' : 'missing';
      return { ...r, ref: `C${i + 1}`, measure: m ? m.measure : '', effect, nexus, basis: nexus === 'effect' ? `Nexus to modelled effect ${effect} (s108AA(1)(b))` : nexus === 'agreed' ? 'Applicant agreement (s108AA(1)(a))' : 'No nexus: link a modelled effect or record the applicant\'s agreement',
        review: r.review || `Review under s128(1)(a) if the monitored ${r.metric || 'metric'} passes ${r.threshold || 'the threshold'} before the measure is in place`, ftaa: c.ftaa ? STD.labels.s84a : '', ok: nexus !== 'missing' };
    });
  }
  function seedConditions(P = Store.P) {
    const cx = data(P), have = new Set(cx.conditions.map(c => c.mitigation)), trig = cx.triggers; let n = 0;
    for (const m of mitigationTable(P)) {
      if (have.has(m.id) || !m.measure) continue;
      const first = trig && trig.rows && trig.rows[0] && trig.rows[0].first;
      // never record the applicant's s108AA(1)(a) agreement for them: the author ticks it (no nexus is shown until then)
      cx.conditions.push({ ...blankCond(), mitigation: m.id, effect: m.effect, agreed: false,
        threshold: first ? `${F.int(first.before)} dwellings` : m.trigger || '', survey: 'Building consent and s224(c) records; turning counts at the affected junction', frequency: 'Annual', response: `${m.measure} operational before the threshold` }); n++;
    }
    return n;
  }

  /* Trigger finder: step build-out (default 5%) for a scenario, with and without each upgrade (any other scenario, by
     default the mitigation scenario), and report the first increment where a criterion fails at a site that passes at 0%,
     or where the development worsens a site that already fails at 0% (an existing deficiency) by STD.trigger.worsen.
     opts { scId, upgrades [scId], step, periods, dos, los, queue, minStorage, anop, worsenDos, worsenDelay, nodes [ids],
     onProgress, gapTol, maxIter, save } */
  // Queue storage on an approach: the approach road back to the upstream junction, chained through simple nodes (two roads,
  // no control) up to maxChain m. null for a roundabout circulating link (junction-internal).
  function storageOf(sc, linkId, nodeId) {
    let L = sc.links[linkId], at = nodeId, m = 0, n = 0;
    if (!L || L.ring) return null;
    const seen = new Set();
    while (L && !seen.has(L.id) && m < STD.trigger.maxChain) {
      seen.add(L.id); m += Net.len(sc, L); n++;
      const far = L.a === at ? L.b : L.a, node = sc.nodes[far], others = roadLinks(sc, far).filter(x => x.id !== L.id);
      if (!node || others.length !== 1 || (node.control && node.control !== 'none')) break;
      at = far; L = others[0];
    }
    return { m, links: n };
  }
  function failsAt(sc, res, crit, only) {
    const out = [], minSt = crit.minStorage != null ? crit.minStorage : STD.trigger.minStorage;
    for (const [id, nr] of Object.entries(res.nodes || {})) {
      if (only && !only.has(id)) continue;
      const why = [], los = nr.intersectionLos || nr.los;
      if (crit.dos != null && nr.dos > crit.dos) why.push(`DoS ${num(nr.dos, 2)} > ${crit.dos}`);
      if (crit.los && los && losWorse(los, crit.los)) why.push(`LoS ${los} worse than ${crit.los}`);
      if (crit.queue) for (const a of nr.appr || []) {
        const st = storageOf(sc, a.link, id);
        if (!st || st.m < minSt || !(a.q95 > st.m)) continue;
        why.push(`95th-percentile queue ${F.int(a.q95)} m > ${F.int(st.m)} m storage back to the upstream junction on ${a.name || linkName(sc.links[a.link])}`); break;
      }
      if (why.length) out.push({ id, why, dos: nr.dos, los, delay: nr.delay });
    }
    return out;
  }
  const FIT_RANK = { High: 3, Moderate: 2, Low: 1 };
  // The ANOP fit of one corridor for a project result against the do-minimum result
  function anopFitOf(scB, rB, scJ, rJ, c) {
    const lb = anopLoS(scB, rB, c), lj = anopLoS(scJ, rJ, c);
    return anopFit(MODES.map(m => { const hs = hierOf(scJ, c, m), pf = anopPref(m, hs.h, c.landUse || 'other') || {}; return { base: lb[m].los, pref: pf.pref || null, proj: lj[m].los }; }));
  }
  async function triggerFinder(P = Store.P, opts = {}) {
    const pr = pair(P), sc = scById(P, opts.scId) || pr.opt, T_ = STD.trigger;
    const step = Math.max(1, Math.min(50, +opts.step || T_.step));
    const crit = { dos: opts.dos !== undefined ? opts.dos : T_.dos, los: opts.los !== undefined ? opts.los : T_.los, queue: opts.queue !== undefined ? !!opts.queue : T_.queue,
      minStorage: nn(opts.minStorage) != null ? +opts.minStorage : T_.minStorage, anop: opts.anop !== undefined ? !!opts.anop : T_.anop,
      worsen: { dos: nn(opts.worsenDos) != null ? +opts.worsenDos : T_.worsen.dos, delay: nn(opts.worsenDelay) != null ? +opts.worsenDelay : T_.worsen.delay } };
    const periods = (opts.periods && opts.periods.length ? opts.periods : periodsOf(sc).length ? periodsOf(sc).filter(p => p !== 'IP') : ['AM', 'PM']);
    const ups = (opts.upgrades || (pr.mit && pr.mit !== sc ? [pr.mit.id] : [])).map(id => scById(P, id)).filter((s, i, a) => s && s !== sc && a.indexOf(s) === i);
    const only = opts.nodes && opts.nodes.length ? new Set(opts.nodes) : null;
    // the ANOP criterion compares each step with the do-minimum, so it needs a do-minimum other than the stepped scenario
    const anopOn = crit.anop && pr.base && pr.base !== sc, cors = anopOn ? anopCorridors(P) : [];
    const variants = [{ id: sc.id, label: `${sc.name} (without upgrade)`, sc, upgrade: false }, ...ups.map(u => ({ id: u.id, label: `${u.name} (with upgrade)`, sc: u, upgrade: true }))];
    const steps = []; for (let k = 0; k <= 100 + 1e-9; k += step) steps.push(Math.round(k * 100) / 100); if (steps[steps.length - 1] !== 100) steps.push(100);
    const base = sc.buildout != null && sc.buildout !== '' && isFinite(+sc.buildout) ? +sc.buildout : 100;
    const rows = [], t0 = Date.now(); let done = 0; const total = variants.length * steps.length * periods.length;
    const prog = (v, k, p) => { done++; if (opts.onProgress) try { opts.onProgress(Math.min(1, done / total), `${v.label}: ${k}% build-out, ${p}`); } catch { } };
    // one build-out step: failing sites, every junction's metrics and (optionally) the ANOP fits
    async function evalStep(v, k) {
      const c = cloneSc(v.sc, { buildout: base * k / 100 }), fails = [], metrics = {}, fits = {};
      for (const p of periods) {
        const r = await runSc(c, p, { gapTol: opts.gapTol, maxIter: opts.maxIter });
        for (const f of failsAt(c, r, crit, only)) fails.push({ ...f, period: p, key: `${f.id}|${p}`, name: nodeName(c, f.id) });
        for (const [id, nr] of Object.entries(r.nodes || {})) metrics[`${id}|${p}`] = { dos: nr.dos, delay: nr.delay, los: nr.intersectionLos || nr.los };
        const rb = anopOn ? resOf(pr.base, p) : null;
        if (rb) for (const cor of cors) { try { fits[`${cor.id}|${p}`] = { fit: anopFitOf(pr.base, rb, c, r, cor), name: cor.name, period: p }; } catch { } }
        prog(v, k, p);
      }
      return { c, fails, metrics, fits };
    }
    for (const v of variants) {
      let existing = null, fit0 = null, first = null, lastPass = null, last = null; const trace = [];
      for (const k of steps) {
        const s = await evalStep(v, k); last = { pct: k, metrics: s.metrics };
        const dw = sumDw(s.c), gfa = sumGfa(s.c);
        if (existing == null) { existing = new Map(s.fails.map(f => [f.key, { id: f.id, key: f.key, name: f.name, period: f.period, why: f.why.join('; '), at0: s.metrics[f.key] }])); fit0 = s.fits; }
        const newF = s.fails.filter(f => !existing.has(f.key));
        // an existing deficiency counts once the development worsens it (ΔDoS or Δdelay, Spare Construct assumption)
        for (const e of existing.values()) {
          const m = s.metrics[e.key]; if (!m || !e.at0) continue;
          if (m.dos - e.at0.dos >= crit.worsen.dos || m.delay - e.at0.delay >= crit.worsen.delay) {
            if (e.worsenedAt == null) e.worsenedAt = k;
            newF.push({ id: e.id, key: e.key, name: e.name, period: e.period, worsened: true, dos: m.dos, los: m.los,
              why: [`existing deficiency worsened by the development: DoS ${num(e.at0.dos, 2)} → ${num(m.dos, 2)}, delay ${num(e.at0.delay, 0)} → ${num(m.delay, 0)} s`] });
          }
        }
        // the ANOP fit drops a class against its 0% value
        if (anopOn) for (const [key, f] of Object.entries(s.fits)) { const f0 = fit0 && fit0[key]; if (f0 && f0.fit && f.fit && FIT_RANK[f.fit] < FIT_RANK[f0.fit]) newF.push({ id: 'anop:' + key, key: 'anop:' + key, name: `${f.name} (ANOP corridor)`, period: f.period, anop: true, why: [`ANOP fit drops from ${f0.fit} to ${f.fit}`] }); }
        trace.push({ pct: k, dwellings: dw, gfa, fails: newF.length, keys: newF.map(f => f.key), worst: newF[0] || null });
        if (newF.length) { first = { pct: k, dwellings: dw, gfa, before: Math.floor((lastPass ? lastPass.dwellings : 0)) + 1, sites: newF, node: newF[0].id, key: newF[0].key, name: newF[0].name, period: newF[0].period, why: newF[0].why.join('; '), worsened: !!newF[0].worsened }; break; }
        lastPass = { pct: k, dwellings: dw, gfa };
      }
      // existing deficiencies with their 0% and full build-out values (one more run per period when the search stopped early)
      if (existing.size && last && last.pct !== 100) last = { pct: 100, metrics: (await evalStep(v, 100)).metrics };
      const ex = [...existing.values()].map(e => ({ id: e.id, name: e.name, period: e.period, why: e.why, at0: e.at0 || null, at100: last ? last.metrics[e.key] || null : null, worsenedAt: e.worsenedAt ?? null }));
      rows.push({ variant: v.id, label: v.label, upgrade: v.upgrade, first, lastPass, existing: ex, trace,
        text: first ? `First fails at ${first.pct}% build-out (${F.int(first.dwellings)} dwellings): ${first.name}, ${first.period} peak (${first.why}).` : `No assessed site fails up to 100% build-out (${F.int(trace.length ? trace[trace.length - 1].dwellings : 0)} dwellings).` });
    }
    // Resolved Y/N (mitigation-staging Table M2): does each upgrade clear the trigger of the variant without it?
    const w0 = rows[0] && rows[0].first;
    for (const r of rows.slice(1)) {
      if (!w0) { r.resolved = { state: null, text: 'Nothing to resolve: no trigger without the upgrade' }; continue; }
      const t = r.trace.find(x => x.pct === w0.pct);
      r.resolved = !r.first ? { state: 'Y', text: `Yes: no assessed site fails up to 100% build-out with ${r.label}` }
        : r.first.pct > w0.pct ? { state: 'Y', text: `Yes: the first failure moves from ${w0.pct}% to ${r.first.pct}% build-out` }
        : r.first.pct < w0.pct ? { state: 'N', text: `No: fails earlier, at ${r.first.pct}% (${r.first.name}, ${r.first.period})` }
        : t && !t.keys.includes(w0.key) ? { state: 'Partly', text: `Partly: ${w0.name} is resolved, but ${r.first.name} fails at the same build-out (${r.first.pct}%)` }
        : { state: 'N', text: `No: ${w0.name} still fails at ${w0.pct}% build-out` };
    }
    const res = { at: new Date().toISOString(), scId: sc.id, scName: sc.name, step, periods, criteria: crit,
      basis: `${ASSUMPTION} criteria: worst DoS above ${crit.dos}, LoS worse than ${crit.los}${crit.queue ? `, or a 95th-percentile queue longer than the storage back to the upstream junction (storage under ${crit.minStorage} m is not tested: usually junction-internal coding)` : ''}; an existing deficiency counts when the development raises its DoS by ${crit.worsen.dos} or its delay by ${crit.worsen.delay} s${anopOn ? '; or an ANOP corridor fit drops a class' : crit.anop ? '; the ANOP criterion needs a do-minimum other than the stepped scenario' : ''}`,
      rows, ms: Date.now() - t0, label: STD.labels.junction };
    const w = rows[0] && rows[0].first, m = data(P).mitigation.find(x => x.measure);
    res.copy = w ? `Prior to occupation of the ${ordinal(w.before)} dwelling, ${m ? m.measure : '<measure>'} (parameter only: the condition wording is for the planner).` : '';
    if (opts.save !== false) data(P).triggers = res;
    return res;
  }
  const ordinal = n => { const s = ['th', 'st', 'nd', 'rd'], v = n % 100; return F.int(n) + (s[(v - 20) % 10] || s[v] || s[0]); };

  /* =====================================================================================
     8. Options and alternatives (reg-alternatives), plan change mode (reg-plan-change), Fast-track package (reg-ftaa)
     ===================================================================================== */
  const HIER = { integrate: 'Integrate land use and transport', maintain: 'Maintain and operate', demand: 'Manage demand', optimise: 'Optimise', capacity: 'New capacity' };
  const HIER_SRC = 'MBCM v1.7.5 s1.5 intervention hierarchy; Ministerial statement on land transport, 13 Aug 2026';
  const routeTime = (sc, ps) => ps.reduce((s, p) => s + Object.values((resOf(sc, p) || {}).routes || {}).reduce((t, r) => t + (+r.t || 0), 0), 0);
  /* A difference (option − base) per modelled peak hour, kept per period: AM and PM are never added together.
     { by { AM, PM }, mean (of the modelled peak hours), text ('AM −720.7, PM −482.7 veh·h per peak hour') } */
  function perPeriod(base, opt, f, ps, d = 1, unit = '') {
    const by = {};
    for (const p of ps) { const b = resOf(base, p), o = resOf(opt, p); if (b && o) by[p] = f(opt, p) - f(base, p); }
    const v = Object.values(by), mean = v.length ? v.reduce((s, x) => s + x, 0) / v.length : null;
    return { by, mean, text: v.length ? `${Object.entries(by).map(([p, x]) => `${p} ${F.signed(x, d)}`).join(', ')}${unit ? ' ' + unit : ''}` : '–' };
  }
  const kpiP = k => (sc, p) => (((resOf(sc, p) || {}).kpi || {})[k] || 0);
  const routeP = (sc, p) => routeTime(sc, [p]);
  const negPP = pp => ({ by: Object.fromEntries(Object.entries(pp.by).map(([p, v]) => [p, -v])), mean: pp.mean == null ? null : -pp.mean });
  const ppText = (pp, d, unit) => (Object.keys(pp.by).length ? `${Object.entries(pp.by).map(([p, x]) => `${p} ${F.signed(x, d)}`).join(', ')}${unit ? ' ' + unit : ''}` : '–');
  function changedLinks(a, b) {
    const ids = new Set();
    for (const L of Object.values(b.links)) { const o = a.links[L.id]; if (!o || o.status !== L.status || o.lanesAB !== L.lanesAB || o.lanesBA !== L.lanesBA || o.speed !== L.speed || o.busLane !== L.busLane || o.cycle !== L.cycle) ids.add(L.id); }
    for (const n of Object.values(b.nodes)) { const o = a.nodes[n.id]; if (o && (o.control !== n.control || (n.upgrade && !o.upgrade))) for (const L of roadLinks(b, n.id)) ids.add(L.id); }
    return ids;
  }
  function crashesOn(P, sc, ids) {
    let risk = null; try { risk = DataHub.linkRisk(sc); } catch { }
    if (!risk) return null;
    const t = { crashes: 0, dsi: 0 }; for (const id of ids) { const r = risk[id]; if (r) { t.crashes += r.crashes || 0; t.dsi += r.dsi || 0; } }
    return t;
  }
  const econOf = (P, base, sc) => { try { const p = periodsOf(sc).find(x => resOf(base, x)) || 'AM'; return Engine.econ(base, sc, p); } catch { return null; } };
  const devMix = sc => JSON.stringify(devZones(sc).map(z => [z.id, z.lu, Engine.stageOf(z, sc)]).sort());
  // The option rows: the user's tagged options, or every scenario other than the do-minimum when none are tagged
  function optionList(P = Store.P) {
    const cx = data(P), base = pair(P).base;
    if (cx.options.length) return cx.options.map(o => ({ ...o, sc: scById(P, o.scId) }));
    return P.scenarios.filter(s => s !== base && s.role !== 'existing' && s.role !== 'baseline').map(s => ({ id: 'sc:' + s.id, name: s.name, scId: s.id, hierarchy: s.role === 'mitigation' ? 'capacity' : '', cost: null, notes: '', sc: s, auto: true }));
  }
  /* alternatives(P) → { base, rows [{ id, name, hierarchy, label, dVht, worst, dRoute, cycleKm, cas, dsi, dVkt, dCo2, bcr, ipm, cost, identical }], warnings[], sections[] } */
  function alternatives(P = Store.P) {
    const base = pair(P).base, c = context(P), rows = [], warnings = [];
    for (const o of optionList(P)) {
      const sc = o.sc, ps = sc ? periodsOf(sc).filter(p => resOf(base, p)) : [];
      const row = { id: o.id, name: o.name || (sc && sc.name) || 'Option', hierarchy: o.hierarchy || '', label: HIER[o.hierarchy] || 'Not tagged', scName: sc ? sc.name : 'No scenario', ran: ps.length > 0, notes: o.notes || '' };
      if (sc && ps.length) {
        // per modelled peak hour, each period on its own (the means fill the numeric columns)
        row.pp = { vht: perPeriod(base, sc, kpiP('vht'), ps, 1, 'veh·h/h'), vkt: perPeriod(base, sc, kpiP('vkt'), ps, 0, 'veh-km/h'), co2: perPeriod(base, sc, kpiP('co2'), ps, 2, 't/h'), route: perPeriod(base, sc, routeP, ps, 0, 's') };
        row.dVht = row.pp.vht.mean; row.dVkt = row.pp.vkt.mean; row.dCo2 = row.pp.co2.mean; row.dRoute = row.pp.route.mean; row.periods = ps;
        const w = ps.flatMap(p => Object.entries(resOf(sc, p).nodes || {}).map(([id, nr]) => ({ id, p, dos: nr.dos, los: nr.intersectionLos || nr.los }))).sort((a, b) => b.dos - a.dos)[0];
        row.worst = w ? `${nodeName(sc, w.id)} ${w.p}: DoS ${num(w.dos, 2)}, LoS ${w.los}` : '–';
        const ch = changedLinks(base, sc);
        row.cycleKm = [...ch].map(id => sc.links[id]).filter(L => L && L.cycle && L.cycle !== 'none' && (!base.links[L.id] || base.links[L.id].cycle !== L.cycle)).reduce((s, L) => s + Net.len(sc, L) / 1000, 0);
        row.cas = crashesOn(P, base, ch);
        const e = econOf(P, base, sc);
        row.bcr = e && !e.mismatch ? (e.bcrText || num(e.bcr, 1)) : e && e.mismatch ? 'years differ' : '–';
        row.ipm = e && e.ipm && e.ipm.length ? e.ipm.map(b => `${b.band}${b.draft ? ' (draft)' : ''}`).join(' / ') : '–';
        row.cost = o.cost != null && o.cost !== '' ? +o.cost : e ? e.cap : null;
        row.identical = Engine.yearOf(sc) === Engine.yearOf(base) && devMix(sc) === devMix(base);
        if (!row.identical) warnings.push(`${row.name} does not use identical inputs to ${base.name} (year or development land use differ): options must be compared on the same inputs.`);
      }
      row.dsi = o.dsi != null && o.dsi !== '' ? +o.dsi : null;
      rows.push(row);
    }
    const tags = new Set(rows.map(r => r.hierarchy));
    if (rows.length && !tags.has('demand') && !tags.has('optimise')) warnings.unshift('No demand-management or optimisation option was tested. The intervention hierarchy expects them to be considered before new capacity (MBCM s1.5; Ministerial statement 13 Aug 2026).');
    if (rows.some(r => !r.hierarchy)) warnings.push('Tag every option with its place in the intervention hierarchy.');
    return { base, rows, warnings, sections: alternativeSections(P, c), label: STD.labels.vkt, source: HIER_SRC };
  }
  // Which alternatives sections the document needs, by pathway and the author's flags
  function alternativeSections(P = Store.P, c = context(P)) {
    const out = [], add = (id, title, ref, include, why) => out.push({ id, title, ref, include, why });
    if (c.kind === 'rc' || c.pathway === 'ftaa-consent') add('cl6-1a', 'Alternative locations or methods', 'RMA Sch 4 cl 6(1)(a)', c.sigEffect, c.sigEffect ? 'The author flags a likely significant adverse effect.' : 'Shown only if the author flags a likely significant adverse effect.');
    if (c.pathway === 'nor' || c.pathway === 'outline-plan') {
      if (c.landInterest === 'no') { add('s168-3b-b', 'Alternative sites, routes and methods', 'RMA s168(3B)(b), s171(1)(b)', true, 'The requiring authority has no sufficient interest in the land.'); add('necessity', 'Reasonable necessity', 'RMA s171(1)(c)', true, 'Required with the alternatives assessment.'); }
      else if (c.landInterest === 'yes') add('s168-3b-c', 'Alternative sites, routes and methods', 'RMA s168(3B)(c)', c.sigEffect, c.sigEffect ? 'Sufficient land interest, but the author flags a likely significant adverse effect.' : 'Sufficient land interest and no significant adverse effect flagged: not required.');
      else add('s168-3b', 'Alternative sites, routes and methods', 'RMA s168(3B), s171(1)(b)', null, 'Record whether the requiring authority has sufficient interest in the land (Compliance tab).');
    }
    if (c.pathway === 'ftaa-nor') { add('ftaa-12g', 'Alternatives', 'FTAA Sch 5 cl 12(1)(g)', true, 'Fast-track NoR.'); add('ftaa-12h', 'Reasonable necessity', 'FTAA Sch 5 cl 12(1)(h)', true, 'Fast-track NoR.'); }
    if (c.kind === 'pc') add('s32-1b', 'Reasonably practicable options', 'RMA s32(1)(b)', true, 'Plan change evaluation.');
    if (c.kind === 'fund') add('ltma-20', 'Alternatives and options', 'LTMA s20(2)(c)(ii)', true, 'NLTF activity.');
    return out;
  }

  /* ---------- plan change mode ---------- */
  const zonedOf = (z, proposed) => { const x = (z.cx || {})[proposed ? 'zonedProposed' : 'zoned']; return isObj(x) ? x : null; };
  const DW_KEYS = ['residential', 'apartment', 'retirement'];
  /* A zone's full build-out land use: the entered zoned capacity merged over its current land use. Entered dwelling capacity
     (of any type) replaces all of the zone's dwellings, so dwelling types are not double counted; every other land use that
     has no entered capacity (school, medical, supermarket …) is kept. Blank entries are ignored. */
  function mergeZoned(current, zoned) {
    const entered = Object.fromEntries(Object.entries(zoned || {}).filter(([k, v]) => LANDUSES[k] && v != null && v !== '' && isFinite(+v)).map(([k, v]) => [k, +v]));
    const out = { ...(current || {}) }, dwIn = DW_KEYS.some(k => k in entered);
    if (dwIn) for (const k of DW_KEYS) delete out[k];
    Object.assign(out, entered);
    return { lu: out, replaced: Object.keys(entered).concat(dwIn ? DW_KEYS.filter(k => !(k in entered) && +((current || {})[k]) > 0) : []), kept: Object.keys(current || {}).filter(k => !(k in entered) && !(dwIn && DW_KEYS.includes(k)) && +current[k] > 0) };
  }
  /* Full build-out: every zone with entered zoned capacity takes that capacity at 100% (interim stages are build-out %).
     PC120 proposed capacity is used when selected, labelled proposed. */
  function fullBuildout(P = Store.P, sc = activeSc(P)) {
    const proposed = !!data(P).planChange.useProposed, zones = [];
    for (const z of Object.values(sc.zones)) {
      if (z.kind === 'gateway') continue;
      const zc = zonedOf(z, proposed) || (proposed ? zonedOf(z, false) : null); if (!zc) continue;
      const m = mergeZoned(z.lu, zc); if (!m.replaced.length) continue;
      zones.push({ id: z.id, name: z.name || z.id, lu: m.lu, entered: zc, replaced: m.replaced, kept: m.kept, current: z.lu || {}, proposed: !!zonedOf(z, proposed) && proposed });
    }
    const dw = zones.reduce((s, z) => s + DW_KEYS.reduce((t, k) => t + (+z.lu[k] || 0), 0), 0);
    const gfa = zones.reduce((s, z) => s + GFA_KEYS.reduce((t, k) => t + (+z.lu[k] || 0), 0), 0);
    return { zones, dwellings: dw, gfa, proposed, label: proposed ? 'Zoned capacity: PC120 proposed (labelled proposed; check the decision, due by June 2027)' : 'Zoned capacity: operative zoning (entered yields)',
      rule: 'Entered dwelling capacity replaces the zone\'s dwellings of every type; other land uses without an entered capacity are kept' };
  }
  function createFullBuildout(P = Store.P, sc = activeSc(P)) {
    const fb = fullBuildout(P, sc); if (!fb.zones.length) return null;
    const ns = Scenarios.copy(sc);
    for (const z of fb.zones) if (ns.zones[z.id]) { ns.zones[z.id].lu = { ...z.lu }; delete ns.zones[z.id].buildout; }
    Object.assign(ns, { name: `${sc.name} · full build-out${fb.proposed ? ' (PC120 proposed)' : ''}`, role: sc.role || 'proposal', buildout: 100, year: sc.year, notes: `${fb.label}. ${fb.rule}. Created by Compliance+ plan change mode.` });
    P.scenarios.push(ns);
    return ns;
  }
  function planChange(P = Store.P) {
    const c = context(P), pc = data(P).planChange, kind = c.kind === 'pc' ? c.pathway : pc.kind || '', n = c.notification || day(pc.notification);
    const proc = [];
    if (kind === 'pc-council') proc.push(n && n < STD.dates.transition ? { level: 'info', text: `Notified ${longDate(n)}, before 30 Oct 2026: check the Planning Act 2026 Sch 1 transition for plan changes.` }
      : { level: 'warn', text: 'Plan stop until the specified transition date (s80P) unless exempt (s80U-80W).' });
    if (kind === 'pc-private') proc.push(n && n >= STD.dates.privatePcBar ? { level: 'warn', text: `Notified ${longDate(n)}: Ministerial exemption required (s80YA).` }
      : { level: 'info', text: `A private plan change request notified on or after ${longDate(STD.dates.privatePcBar)} needs a Ministerial exemption (s80YA)${n ? `; this one is notified ${longDate(n)}` : '; enter the notification date'}.` });
    if (!kind) proc.push({ level: 'info', text: 'Choose a plan change pathway (Compliance tab) to check the process dates.' });
    const fb = fullBuildout(P), alt = alternatives(P), E = P.settings.econ || {}, hrs = E.periodHours || { AM: 600, PM: 600 }, a = A(P);
    const s32 = alt.rows.filter(r => r.ran).map(r => {
      const o = optionList(P).find(x => x.id === r.id) || {}, sc = o.sc, ps = sc ? periodsOf(sc).filter(p => resOf(alt.base, p)) : [];
      const annH = ps.reduce((s, p) => s + (((resOf(sc, p) || {}).kpi || {}).vht - ((resOf(alt.base, p) || {}).kpi || {}).vht) * (+hrs[p] || 0), 0);
      const annVkt = ps.reduce((s, p) => s + (((resOf(sc, p) || {}).kpi || {}).vkt - ((resOf(alt.base, p) || {}).kpi || {}).vkt) * (+hrs[p] || 0), 0);
      const unf = sc ? Object.values(sc.links).filter(L => L.status !== 'closed' && Scenarios.unfunded(L)) : [];
      return { name: r.name, label: r.label, travel: `${ppText(negPP(r.pp.vht), 1, 'veh·h saved per modelled peak hour')}; about ${F.signed(-annH, 0)} veh·h a year`,
        safety: r.cas ? `${r.cas.crashes} crashes (${r.cas.dsi} DSI) on changed roads in the CAS window; predicted change ${r.dsi != null ? F.signed(r.dsi, 1) + ' DSI/yr' : 'not modelled'}` : 'CAS not loaded',
        voc: `${F.signed(annVkt * a.vocPerKm / 1e6, 2)} NZ$m a year (${F.signed(annVkt / 1e6, 2)} M veh-km × NZ$${a.vocPerKm}/km, ${ASSUMPTION})`,
        cost: r.cost != null ? `${F.money(r.cost)}${o.timing ? `, ${o.timing}` : ''}` : 'Not costed', timing: o.timing || '',
        acting: unf.length ? `Relies on ${unf.length} unfunded project${unf.length === 1 ? '' : 's'} (${[...new Set(unf.map(linkName))].slice(0, 3).join(', ')}): effects return if they are not funded` : 'Relies on no unfunded project',
        notActing: `Do-minimum: ${effects(P, alt.base).length} effects screened in ${alt.base.name}` };
    });
    const policy = [
      { ref: 'RMA s74(2)(b)(i)', policy: 'Regional land transport plan', text: rltpRelied(P) },
      { ref: 'RMA s74(2)(d); ERP2', policy: 'Emissions reduction plan (information only)', text: `ΔVKT and ΔCO2e are reported in the s32 table and alternatives table: ${STD.labels.vkt}.` },
      { ref: 'NPS-UD Policy 1(c)', policy: 'Good accessibility for all people (jobs, services, open space) by all modes', text: 'See the accessibility rows in the policy alignment table.' },
      { ref: 'NPS-UD Policy 1(e)', policy: 'Supports reductions in greenhouse gas emissions', text: `Information only: ${STD.labels.vkt}.` },
      { ref: 'NPS-I Policies 10-11', policy: 'Infrastructure integrated with land use and funded', text: `Network projects relied on: ${rltpRelied(P)}` },
    ];
    const sc = activeSc(P), roads = Object.values(sc.links).filter(L => L.status === 'proposed' || (L.cx && (L.cx.rasf || L.cx.fn))).map(L => ({ id: L.id, name: linkName(L), cls: (CLASSES[L.cls] || CLASSES.local).label,
      fn: (L.cx && L.cx.fn) || ({ motorway: 'Strategic (motorway)', ramp: 'Strategic (motorway ramp)', arterial: 'Arterial: movement with place where it passes centres', collector: 'Collector: connects neighbourhoods to arterials', local: 'Local street: access and place' }[L.cls] || 'Local street'),
      rasf: (L.cx && L.cx.rasf) || '–', fc: L.fc && L.fc.gt ? L.fc.gt : '–', speed: posted(L), lanes: (L.lanesAB | 0) + (L.lanesBA | 0), cycle: CYCLE[L.cycle] || 'None', status: STATUS[L.status] || L.status }));
    const lintN = lint(P).findings.length, stat = (() => { try { return Engine.modelStatus(P, 'AM').label; } catch { return '–'; } })();
    const cl23 = [
      ['Model status and purpose', stat], ['Sensitivity tests', P.report && P.report.sens && P.report.sens.rows ? `${P.report.sens.rows.length} tests run` : 'Not run: run the sensitivity pack'],
      ['Full build-out', fb.zones.length ? `${F.int(fb.dwellings)} dwellings, ${F.int(fb.gfa)} m² GFA (${fb.label})` : 'Zoned capacity not entered'],
      ['Options assessed (s32(1)(b))', `${alt.rows.length} option${alt.rows.length === 1 ? '' : 's'}`], ['Scope and wording', lintN ? `${lintN} linter finding${lintN === 1 ? '' : 's'} open` : 'No linter findings'],
      ['Staging triggers', data(P).triggers ? 'Trigger finder run' : 'Not run'],
    ];
    return { kind, notification: n, process: proc, buildout: fb, s32, s32Label: STD.labels.s32, policy, roads, cl23, triggers: data(P).triggers };
  }
  function rltpRelied(P = Store.P) {
    const sc = activeSc(P), funded = Object.values(sc.links).filter(L => Scenarios.linkStage(L) === 'committed' && L.status !== 'closed');
    const reg = (P.register || []).filter(r => /transport|road|network/i.test(r.type || ''));
    const names = [...new Set([...funded.map(L => `${linkName(L)} (${FUNDING[L.funding] || 'funding not recorded'})`), ...reg.map(r => `${r.name} (${r.status || 'status not recorded'})`)])];
    return names.length ? names.slice(0, 8).join('; ') + (names.length > 8 ? `; and ${names.length - 8} more` : '') : 'None recorded: mark committed network projects (road inspector › stage) or add them to the register.';
  }

  /* ---------- Fast-track package ---------- */
  function invitees(P = Store.P) {
    const cx = data(P).ftaa, sc = activeSc(P), a = A(P), out = [{ who: 'Auckland Council', basis: 'FTAA s53(2) (relevant local authority)', auto: true }];
    let shNear = null;
    const zs = devZones(sc);
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed' || !isSH(L, P)) continue;
      // point-to-segment distance, so a sparsely digitised motorway is still found
      for (const z of zs) { const d = Net.project(sc, L, [z.lat, z.lng]).d; if (d <= a.shBuffer && (!shNear || d < shNear.d)) shNear = { d, L, z }; }
    }
    if (shNear || cx.nztaDesignation) out.push({ who: 'NZTA', basis: `FTAA s53(2)(l): designation on or adjacent to the site (${cx.nztaDesignation ? 'marked by the author' : `${linkName(shNear.L)} is ${F.int(shNear.d)} m from ${shNear.z.name || 'a development zone'}; within ${a.shBuffer} m (${ASSUMPTION})`})`, auto: !cx.nztaDesignation });
    if (cx.kiwirailDesignation) out.push({ who: 'KiwiRail', basis: 'FTAA s53(2)(l): rail designation on or adjacent to the site (marked by the author)', auto: false });
    return out;
  }
  function ftaaTimeline(P = Store.P) {
    const f = data(P).ftaa, S = STD.ftaa, rows = [];
    if (isISO(f.invitation)) {
      const s54 = addWorkingDays(f.invitation, S.s54), s55 = s54.date ? addWorkingDays(s54.date, S.s55) : { date: null };
      rows.push({ step: 'Invitation to comment sent (s53)', date: f.invitation, rule: 'FTAA s53' }, { step: 'Comments due', date: s54.date, rule: `FTAA s54: ${S.s54} working days`, basis: s54.basis },
        { step: 'Applicant response to comments due', date: s55.date, rule: `FTAA s55(2): ${S.s55} working days`, basis: s55.basis });
    } else rows.push({ step: 'Invitation to comment', date: null, rule: 'FTAA s53-55', basis: 'Enter the invitation date to work out the s54 and s55(2) dates.' });
    rows.push({ step: 'Comments on draft conditions', date: null, rule: `FTAA s67: no more than ${S.s67} working days`, basis: 'Counted from when the panel issues draft conditions.' });
    if (isISO(f.approval)) rows.push({ step: 'Approval lapses (unless the panel sets another period)', date: addMonths(f.approval, 12 * S.lapseYears), rule: `FTAA Sch 5 cl 26: ${S.lapseYears} years by default` },
      { step: 'Development contributions policy window ends', date: addMonths(f.approval, S.dcMonths), rule: `LGA s102A(3): ${S.dcMonths} months from approval` });
    return rows;
  }
  /* ftaaPackage(P) → { matrix, aee, conditions, s85 { benefits, impacts }, s84a, reviewer { assumptions, sources, sensitivity, model }, invitees, timeline, notes } */
  function ftaaPackage(P = Store.P) {
    const c = context(P), pr = pair(P), cx = data(P), opt = pr.opt, base = pr.base, after = pr.mit || opt, ps = periodsOf(opt).filter(p => resOf(base, p));
    const matrix = [];
    const C = C_();
    try { if (C && C.e27) { const e = C.e27(P); matrix.push({ rule: 'AUP E27.6.1 trip generation', req: 'Table E27.6.1.1 thresholds', result: e.redirect ? 'Not applicable (precinct)' : e.exempt ? `Not applicable (exempt, ${e.exempt.rule || 'E27.6.1(2)'})` : e.tier ? 'Infringes (restricted discretionary)' : 'Complies', evidence: 'Compliance tab › E27 screening', auto: true }); } } catch { }
    const pk = parking(P); if (pk.rows.length) matrix.push({ rule: 'AUP E27.6.2 parking, cycle and loading', req: 'Tables E27.6.2.3-E27.6.2.7', result: pk.allComply ? 'Complies' : 'Infringes', evidence: 'Compliance+ › Parking (Table P1)' });
    const pc = precinctCheck(P); if (pc) matrix.push({ rule: `AUP ${pc.precinct.rule} staging`, req: pc.precinct.table, result: pc.breach ? `Infringes (${pc.status})` : 'Complies', evidence: 'Compliance+ › Precinct' });
    // Objectives and policies need the author's judgment: 'To assess (author)' until a result is recorded (data.ftaa.matrix)
    const recorded = cx.ftaa.matrix || {};
    try { if (C && C.DATA && C.DATA.POLICY) for (const p of C.DATA.POLICY) { const r = recorded[p.ref] || {}; matrix.push({ rule: `AUP ${p.ref}`, ref: p.ref, req: p.text, result: MATRIX_RESULT[r.result] || 'To assess (author)', authorNote: r.note || '', evidence: [...(p.evidence || []), r.note].filter(Boolean).join('; '), author: true }); } } catch { }
    if (!matrix.length) matrix.push({ rule: 'AUP E27 rules and standards', req: 'Set the site and pathway in the Compliance tab', result: 'Not assessed', evidence: '–' });
    const aee = [['Description of the activity and its transport components', 'FTAA Sch 5 cl 6(1)(a)'], ['Assessment of actual and potential transport effects', 'FTAA Sch 5 cl 6-7'], ['Mitigation and monitoring', 'FTAA Sch 5 cl 6(1)'], ['Persons affected and consultation', 'FTAA Sch 5 cl 6(1)']]
      .map(([t, r]) => ({ title: t, ref: r }));
    if (c.pathway === 'ftaa-nor') aee.push({ title: 'Alternatives', ref: 'FTAA Sch 5 cl 12(1)(g)' }, { title: 'Reasonable necessity', ref: 'FTAA Sch 5 cl 12(1)(h)' });
    const dv = negPP(perPeriod(base, opt, kpiP('vht'), ps)), dPt = perPeriod(base, after, routeP, periodsOf(after).filter(p => resOf(base, p))), e = econOf(P, base, opt);
    let risk = null; try { risk = DataHub.linkRisk(base); } catch { }
    const hot = risk ? Object.entries(risk).filter(([, r]) => r.dsi > 0).sort((a, b) => b[1].dsi - a[1].dsi).slice(0, 3).map(([id, r]) => `${linkName(base.links[id])} (${r.dsi} DSI)`) : [];
    const worst = periodsOf(after).flatMap(p => Object.entries(resOf(after, p).nodes || {}).map(([id, nr]) => ({ id, p, nr }))).sort((a, b) => b.nr.dos - a.nr.dos).slice(0, 3);
    const s85 = {
      benefits: [['Travel time saved', ps.length ? `${ppText(dv, 1, 'veh·h per modelled peak hour')} (${base.name} → ${opt.name})` : 'Run the do-minimum and the option'],
        ['Reliability', cx.ftaa.reliability || 'Not modelled: enter from observed data'], ['Jobs reachable', cx.ftaa.jobs || 'Enter from the accessibility analysis (policy alignment table)'],
        ['Houses enabled', `${F.int(sumDw(opt))} dwellings in ${opt.name}`], ['DSI change', 'Not modelled in Spare Construct: enter from the safety assessment'],
        ['Indicative BCR', e && !e.mismatch && e.bcr != null ? `${e.bcrText || num(e.bcr, 1)} (${D4short})` : e && e.mismatch ? e.note : 'Not available']],
      impacts: [['Worst junctions after mitigation', worst.length ? `${worst.map(w => `${nodeName(after, w.id)} ${w.p} DoS ${num(w.nr.dos, 2)}, LoS ${w.nr.intersectionLos || w.nr.los}`).join('; ')} (${STD.labels.junction})` : 'Not run'],
        ['DSI hotspots (CAS)', hot.length ? hot.join('; ') : 'CAS not loaded or no DSI crashes'],
        ['Public transport delay', Object.keys(dPt.by).length ? `${ppText({ by: Object.fromEntries(Object.entries(dPt.by).map(([p, v]) => [p, v / 60])) }, 1, 'min per modelled peak hour, summed over the modelled bus routes')}` : '–']],
      note: `FTAA s85(3): regional or national benefits are weighed against residual adverse impacts after mitigation, so both are shown. Junction results: ${STD.labels.junction}.`, junctionLabel: STD.labels.junction,
    };
    const s84a = [];
    for (const r of (cx.triggers && cx.triggers.rows) || []) s84a.push({ stage: r.label, trigger: r.first ? `Before ${F.int(r.first.before)} dwellings (${r.first.pct}% build-out)` : 'No trigger up to full build-out', measure: r.upgrade ? 'With the upgrade' : 'Without the upgrade' });
    for (const m of mitigationTable(P)) if (m.trigger) s84a.push({ stage: m.ref, trigger: m.trigger, measure: m.measure });
    if (pc) for (const r of pc.rows) s84a.push({ stage: `${pc.precinct.id} row (${r.row})`, trigger: `${F.int(r.dw)} dwellings`, measure: r.up.map(u => `${u.name} (${u.stateLabel})`).join('; ') });
    const sens = P.report && P.report.sens && P.report.sens.rows ? P.report.sens : null;
    const loaded = (P.data && P.data.loaded) || {};
    return {
      matrix, aee, conditions: conditionTable(P), s85, s84a, invitees: invitees(P), timeline: ftaaTimeline(P),
      reviewer: { assumptions: Object.entries(A(P)).map(([k, v]) => [SET_LABELS[k] ? SET_LABELS[k][0] : k, v, SET_LABELS[k] ? SET_LABELS[k][1] : '', ASSUMPTION]),
        sources: Object.entries(loaded).map(([k, v]) => [k, v && v.at ? new Date(v.at).toISOString().slice(0, 10) : '', v && (v.n != null) ? v.n : '', (v && (v.licence || v.source)) || '']),
        sensitivity: sens, model: 'project.json (the full Spare Construct project: network, zones, settings, counts and results)' },
      notes: ['GRPA approvals sit outside Fast-track: NZTA consent under GRPA s91 and s93 (crossing places) and s176 consent on designated land cannot be granted by the panel (FTAA s42(4)). Apply to NZTA separately.',
        `Lapse: a Fast-track approval lapses ${STD.ftaa.lapseYears} years after it commences unless the panel sets another period (FTAA Sch 5 cl 26).`],
      conditionsLabel: STD.labels.s84a, ftaa: c.ftaa,
    };
  }
  const D4short = 'indicative, equivalent to an IER; not a funding-approval BCR';
  const MATRIX_RESULT = { consistent: 'Consistent (author)', inconsistent: 'Inconsistent (author)', 'not-relevant': 'Not relevant (author)' };

  /* =====================================================================================
     9. Speed limit validator and CBDS (reg-speed)
     ===================================================================================== */
  const SP = STD.speed;
  const minLenFor = v => { for (const [k, m] of SP.minLen) if (v <= k) return m; return SP.minLen[SP.minLen.length - 1][1]; };
  const limitOf = L => (L.cx && nn(L.cx.limit)) || posted(L);
  /* Sections: links sharing L.cx.section, otherwise connected links with the same name and limit (one section each) */
  function speedSections(sc) {
    const links = Object.values(sc.links).filter(L => L.status !== 'closed' && L.cx && (L.cx.ruleClass || nn(L.cx.limit) != null || L.cx.variable));
    const key = L => L.cx.section ? 's:' + L.cx.section : `n:${L.name || L.id}|${limitOf(L)}`;
    const groups = new Map();
    for (const L of links) { const k = key(L); if (!groups.has(k)) groups.set(k, []); groups.get(k).push(L); }
    const out = [];
    for (const [k, Ls] of groups) {
      if (k.startsWith('s:')) { out.push(Ls); continue; }
      const left = new Set(Ls.map(L => L.id));
      while (left.size) {   // connected components by shared nodes
        const seed = [...left][0], comp = [], st = [seed]; left.delete(seed);
        while (st.length) { const id = st.pop(), L = sc.links[id]; comp.push(L); for (const o of Ls) if (left.has(o.id) && (o.a === L.a || o.a === L.b || o.b === L.a || o.b === L.b)) { left.delete(o.id); st.push(o.id); } }
        out.push(comp);
      }
    }
    return out;
  }
  /* Road length of a section (Schedule 1 minimum, CBDS journey time): the two one-way carriageways of a divided road (same
     name, opposite heading, within 60 m, as DataHub pairs them for crash risk) count once, as half their summed length. */
  const PAIR_M = 60;
  function roadLen(sc, Ls) {
    const oneWay = L => ((L.lanesAB | 0) > 0) !== ((L.lanesBA | 0) > 0);
    const head = L => { const g = Net.geom(sc, L), b = U.bearing(g[0], g[g.length - 1]); return (L.lanesAB | 0) > 0 ? b : (b + 180) % 360; };
    let len = 0;
    for (const L of Ls) {
      const l = Net.len(sc, L);
      if (!oneWay(L)) { len += l; continue; }
      const g = Net.geom(sc, L), mid = U.along(g, U.cumLen(g), U.polyLen(g) / 2).p, b = head(L);
      const paired = Ls.some(O => O !== L && oneWay(O) && (O.name || '') === (L.name || '') && U.angDiff(b, head(O)) > 135 && Net.project(sc, O, mid).d <= PAIR_M);
      len += paired ? l / 2 : l;
    }
    return len;
  }
  // The current (existing) limit of a road: from the do-minimum where the road exists there
  const currentOf = (P, L) => { const B = pair(P).base.links[L.id]; return posted(B || L); };
  /* speedCheck(P, sc) → rows [{ ids, name, ruleClass, classLabel, limit, current, len, minLen, flags [{ id, text, rule }], decision, onf }] */
  function speedCheck(P = Store.P, sc = activeSc(P)) {
    const rows = [];
    for (const Ls of speedSections(sc)) {
      const L = Ls[0], x = L.cx || {}, limit = limitOf(L), cls = SP.classes[x.ruleClass], len = roadLen(sc, Ls), flags = [];
      if (!cls) flags.push({ id: 'class', text: 'No Rule class: choose one from Schedule 3 Tables 1-3 (road inspector › Speed limit).', rule: 'cl 4.5' });
      else if (limit < cls.min || limit > cls.max) flags.push({ id: 'range', text: `${limit} km/h is outside the Schedule 3 range for ${cls.label} (${cls.min === cls.max ? cls.min : `${cls.min}–${cls.max}`} km/h).`, rule: 'cl 4.5, Schedule 3' });
      if (cls && cls.crashHistory && !x.crashHistory) flags.push({ id: 'isz', text: `${cls.label}s need a history of high-risk crash types: record it.`, rule: 'Schedule 3 Table 3' });
      const min = minLenFor(limit), exc = x.exception && SP.exceptions[x.exception] ? x.exception : limit < 40 ? 'below40' : x.school ? 'school' : x.variable ? 'variable' : '';
      if (len < min && !exc) flags.push({ id: 'length', text: `The ${limit} km/h section is ${F.int(len)} m long, shorter than the Schedule 1 minimum of ${F.int(min)} m.`, rule: 'cl 4.6, Schedule 1' });
      const auth = authorityOf(L, P, sc);
      if (SP.nztaOnly.includes(limit) && !auth.nzta) flags.push({ id: 'nzta', text: `${limit} km/h may be set only by NZTA, on roads designed, built and maintained for it; this road is controlled by ${auth.name}.`, rule: 'cl 4.3(2)' });
      if (x.variable && !SP.varReasons[x.varReason]) flags.push({ id: 'variable', text: 'A variable limit needs a cl 4.9(1)(b) reason (or Director approval).', rule: 'cl 4.9(1)(b)' });
      const onf = x.onf ? Object.entries(SP.onf).filter(([, v]) => v.some(n => n.toLowerCase() === String(x.onf).toLowerCase())).map(([k]) => SP.classes[k].label) : [];
      rows.push({ ids: Ls.map(l => l.id), name: linkName(L), ruleClass: x.ruleClass || '', classLabel: cls ? cls.label : 'Not set', limit, current: currentOf(P, L), len, minLen: min, exception: exc ? SP.exceptions[exc] : '',
        flags, ok: !flags.length, decision: auth.decision, onf: onf.length ? `${onf.join(' or ')} (${STD.labels.onf})` : '' });
    }
    return rows;
  }
  // School speed records from the NSLR variable layer (feeds lane puts them on L.varSpeed)
  function schoolSpeeds(sc = Store.sc) {
    return Object.values(sc.links).filter(L => L.varSpeed && /school/i.test(L.varSpeed.desc || L.varSpeed.v)).map(L => ({ id: L.id, name: linkName(L), v: +L.varSpeed.v,
      cat: +L.varSpeed.v <= 30 ? 'Category 1 (30 km/h)' : +L.varSpeed.v <= 60 ? 'Category 2 (60 km/h or less)' : 'Check category', windows: L.varSpeed.desc || '–', len: Net.len(sc, L) }));
  }
  /* Crashes on a section's links by year and severity: each crash goes to its nearest existing road within 25 m, or to a
     nearby road its crash location names (as DataHub assigns crashes for crash risk). null when no crashes are loaded. */
  function sectionCrashes(P, sc, Ls) {
    let cr = (P && P.data && Array.isArray(P.data.crashes)) ? P.data.crashes : [];
    if (!cr.length && P === Store.P) try { cr = (DataHub.D().crashes) || []; } catch { }
    if (!cr.length) return null;
    const R = 25, pad = R / 90000, ids = new Set(Ls.map(L => L.id)), by = {};
    const links = Object.values(sc.links).filter(L => L.status !== 'proposed' && sc.nodes[L.a] && sc.nodes[L.b]);
    const boxes = new Map(links.map(L => { let a = Infinity, b = -Infinity, c = Infinity, d = -Infinity; for (const p of Net.geom(sc, L)) { a = Math.min(a, p[0]); b = Math.max(b, p[0]); c = Math.min(c, p[1]); d = Math.max(d, p[1]); } return [L.id, [a, b, c, d]]; }));
    const near = (L, c) => { const b = boxes.get(L.id); return !!b && !(c[0] < b[0] - pad || c[0] > b[1] + pad || c[1] < b[2] - pad * 1.3 || c[1] > b[3] + pad * 1.3); };
    const norm = s => { try { return DataHub.norm(s); } catch { return String(s || '').toLowerCase(); } };
    for (const c of cr) {
      if (!Ls.some(L => near(L, c))) continue;
      const ds = [];
      for (const L of links) if (near(L, c)) { const d = Net.project(sc, L, [c[0], c[1]]).d; if (d <= R) ds.push([L, d]); }
      if (!ds.length) continue;
      ds.sort((x, y) => x[1] - y[1]);
      let best = ds[0];
      const loc = norm(String(c[4] || '').split(' / ')[0]);
      if (loc && norm(best[0].name) !== loc) { const alt = ds.find(([L, d]) => { const nm = norm(L.name); return d <= best[1] + 5 && nm && (nm === loc || nm.includes(loc) || loc.includes(nm)); }); if (alt) best = alt; }
      if (!ids.has(best[0].id)) continue;
      const y = +c[3] || 0, t = by[y] || (by[y] = { F: 0, S: 0, M: 0, N: 0 });
      t['FSMN'.includes(c[2]) ? c[2] : 'N']++;
    }
    return by;
  }
  /* cl 3.3 crash history: the full calendar years of the CAS window (from the year the limit or road existed, `since`, when
     later), with the year to date reported but not counted (incomplete). */
  function crashHistory(P, sc, Ls, since) {
    const by = sectionCrashes(P, sc, Ls); if (!by) return null;
    const cas = (P.data && P.data.loaded && P.data.loaded.cas) || {}, ys = Object.keys(by).map(Number).filter(y => y > 0);
    const from = +cas.from > 0 ? +cas.from : ys.length ? Math.min(...ys) : null, fullTo = +cas.to > 0 ? +cas.to : ys.length ? Math.max(...ys) : null;
    const ytdY = +cas.ytd > 0 ? +cas.ytd : +cas.to > 0 ? +cas.to + 1 : null;
    const y0 = from != null ? (since ? Math.max(from, since) : from) : null, n = y0 != null && fullTo != null ? Math.max(0, fullTo - y0 + 1) : 0;
    const hist = { F: 0, S: 0, M: 0, N: 0 };
    for (let y = y0; n && y <= fullTo; y++) for (const k of 'FSMN') hist[k] += (by[y] || {})[k] || 0;
    const ytd = ytdY && ytdY > fullTo && by[ytdY] ? 'FSMN'.split('').reduce((s, k) => s + by[ytdY][k], 0) : 0;
    return { hist, years: n, from: y0, to: fullTo, since: since || null, ytdYear: ytdY, ytd, lagYear: cas.lagYear || null,
      period: n ? `${y0}–${fullTo} (${n} full year${n === 1 ? '' : 's'}${since && since > from ? `, from ${since}, when the limit or road began; part of ${since} may be under the earlier limit` : ''})` : `No full year of CAS data${since ? ` since ${since}` : ''}` };
  }
  /* cbds(P, ids, opts) → { ok, why, headings [{ id, title, rule, rows }], watermark, decision, ... }. Not offered for new roads or
     school-gate limits. The proposed limit, school and new-road flags are read from the scenario the speed check uses (the
     active scenario, opts.sc); the existing road, its current limit, crash history, modelled speed and AADT from the
     do-minimum. Mean operating speed must be measured: a modelled speed is labelled as a placeholder. */
  function cbds(P = Store.P, ids, opts = {}) {
    const pr = pair(P), sc = opts.sc || activeSc(P), base = pr.base, a = A(P), cx = data(P).speed, NL = STD.nilsson;
    const Ls = (ids || []).map(id => sc.links[id] || base.links[id]).filter(Boolean);
    if (!Ls.length) return { ok: false, why: 'Choose the road section (links with a proposed limit).' };
    const isNew = Ls.find(L => L.status === 'proposed' || (L.cx && L.cx.newRoad) || !base.links[L.id] || base.links[L.id].status === 'proposed');
    if (isNew) return { ok: false, why: `${linkName(isNew)} is a new road: ${SP.cbdsSkip}.`, newRoad: true };
    const school = Ls.find(L => (L.cx && L.cx.school) || (base.links[L.id].cx && base.links[L.id].cx.school));
    if (school) return { ok: false, why: `${linkName(school)} is a school-gate limit: ${SP.cbdsSkip}.` };
    const Bs = Ls.map(L => base.links[L.id]), cxOf = (L, k) => nn((L.cx || {})[k]) ?? nn((base.links[L.id].cx || {})[k]);
    const len = roadLen(base, Bs), v0 = posted(Bs[0]), v1 = limitOf(Ls[0]);
    let risk = null; try { risk = DataHub.linkRisk(base); } catch { }
    const since = Ls.map(L => cxOf(L, 'since')).filter(Boolean).sort((x, y) => y - x)[0] || null;
    const ch = crashHistory(P, base, Bs, since), hist = ch ? ch.hist : { F: 0, S: 0, M: 0, N: 0 }, yrs = ch && ch.years > 0 ? ch.years : null;
    const dsiYr = yrs ? (hist.F + hist.S) / yrs : null, injYr = yrs ? (hist.F + hist.S + hist.M) / yrs : null, nonYr = yrs ? hist.N / yrs : null;
    // mean operating speed: measured (author or MegaMaps), otherwise the modelled speed as a placeholder
    const meas = Ls.map(L => cxOf(L, 'meanSpeed')).filter(Boolean), mm = Bs.map(L => nn(L.megamaps && L.megamaps.mean)).filter(Boolean);
    const r0 = resOf(base, 'AM') || resOf(base, 'PM');
    const modelled = r0 ? (() => { let d = 0, t = 0; for (const L of Bs) for (const k of ['ab', 'ba']) { const x = r0.arcs[L.id + ':' + k]; if (x && x.t > 0) { d += x.len; t += x.t; } } return t > 0 ? d / t * 3.6 : null; })() : null;
    const m0 = meas.length ? meas.reduce((s, v) => s + v, 0) / meas.length : mm.length ? mm.reduce((s, v) => s + v, 0) / mm.length : modelled;
    const srcTxt = Ls.map(L => (L.cx && L.cx.meanSpeedSrc) || ((base.links[L.id].cx || {}).meanSpeedSrc)).filter(Boolean)[0];
    const mSrc = meas.length ? `measured (${srcTxt || 'entered by the author'})` : mm.length ? 'MegaMaps' : modelled != null ? `modelled: ${STD.labels.meanSpeed}` : 'not available';
    const m1 = m0 != null ? Math.max(5, m0 + a.meanShare * (v1 - v0)) : null;
    // Nilsson power model: fatal and serious crashes ∝ v^dsiExponent, injury crashes ∝ v^injuryExponent; non-injury held
    const k = m0 && m1 ? m1 / m0 : 1, dsi1 = dsiYr == null ? null : dsiYr * Math.pow(k, a.dsiExponent), inj1 = injYr == null ? null : injYr * Math.pow(k, a.injuryExponent);
    const all0 = injYr == null ? null : injYr + nonYr, all1 = inj1 == null ? null : inj1 + nonYr;
    const aadtOf = L => { const r = risk && risk[L.id]; if (r && r.aadt > 0) return { v: r.aadt, src: r.aadtSrc || 'count' }; const x = r0 && ['ab', 'ba'].reduce((s, d) => s + ((r0.arcs[L.id + ':' + d] || {}).v || 0), 0); return x ? { v: x * 10, src: `modelled peak hour × 10 (${ASSUMPTION})` } : null; };
    const aadt = aadtOf(Bs[0]);
    const dtVeh = m0 && m1 ? len / (m1 / 3.6) - len / (m0 / 3.6) : null;
    const histRows = ch ? [['Fatal', hist.F], ['Serious', hist.S], ['Minor', hist.M], ['Non-injury', hist.N], ['Period', ch.period],
      ...(ch.ytd ? [['Not counted', `${ch.ytd} crash${ch.ytd === 1 ? '' : 'es'} in ${ch.ytdYear} to date (YTD, incomplete)`]] : []), ...(ch.lagYear ? [['Note', `${ch.lagYear} may be incomplete (CAS entry lag)`]] : [])]
      : [['CAS', 'Not loaded: load NZTA CAS crashes (Traffic data tab)']];
    const method = `Nilsson power model on mean speed (${NL.edition}): fatal and serious crashes ∝ v^${a.dsiExponent}, injury crashes ∝ v^${a.injuryExponent}; non-injury crashes held at the current rate (the model covers injury crashes only); ${ASSUMPTION} exponents (D13). Not an MBCM crash procedure: MBCM v1.7.5 crash trend factors for speed-limit categories (NZTA GC 26/01) are not applied`;
    const headings = [
      { id: 'history', title: 'Crashes by severity over the previous 5 years', rule: 'cl 3.3', rows: histRows, basis: 'Each crash on its nearest existing road within 25 m (DataHub method); full calendar years only' },
      { id: 'forecast', title: 'Forecast annual crashes and DSIs over the next 5 years, with and without the change', rule: 'cl 3.3',
        rows: yrs ? [['Without the change: DSI per year', num(dsiYr, 2)], ['With the change: DSI per year', num(dsi1, 2)], ['Without: injury crashes per year', num(injYr, 2)], ['With: injury crashes per year', num(inj1, 2)],
          ['Without: all crashes per year', num(all0, 2)], ['With: all crashes per year', num(all1, 2)], ['Over 5 years: DSI saved', num((dsiYr - dsi1) * 5, 1)], ['Method', method]]
          : [['Forecast', ch ? 'No full year of crash history to forecast from' : 'Load NZTA CAS crashes to forecast'], ['Method', method]] },
      { id: 'speed', title: 'Current mean operating speed and estimated new mean speed', rule: 'cl 3.3',
        rows: [['Current limit', `${v0} km/h`], ['Proposed limit', `${v1} km/h`], ['Current mean operating speed', m0 != null ? `${num(m0, 1)} km/h (${mSrc})` : 'Not available: enter a measured mean speed'], ['Estimated new mean speed', m1 != null ? `${num(m1, 1)} km/h (current + ${a.meanShare} × limit change; ${ASSUMPTION})` : '–']] },
      { id: 'time', title: 'Journey time change: per vehicle and AADT-weighted', rule: 'cl 3.3',
        rows: [['Section length', `${F.len(len)} (a divided road counted once)`], ['Per vehicle', dtVeh != null ? `${F.signed(dtVeh, 1)} s` : '–'], ['AADT', aadt ? `${F.int(aadt.v)} veh/day (${aadt.src})` : 'Not available'],
          ['AADT-weighted', dtVeh != null && aadt ? `${F.signed(dtVeh * aadt.v * 365 / 3600, 0)} veh·h a year` : '–']] },
      { id: 'cost', title: 'Implementation cost', rule: 'cl 3.3', rows: [['Cost', +cx.cost > 0 ? `NZ$${F.int(+cx.cost)}` : 'Enter the implementation cost']] },
      { id: 'tool', title: 'Tool and assumptions', rule: 'cl 3.3', rows: [['Tool', cx.tool || 'Spare Construct sketch model'], ['Counterfactual', 'The limit does not change'], ['Scenarios', `Proposed limit from ${sc.name}; existing road, crashes and modelled speed from ${base.name}`],
        ['Assumptions', `Mean-speed share ${a.meanShare}; power-model exponents ${a.dsiExponent} (fatal and serious) and ${a.injuryExponent} (injury); ${ASSUMPTION} values (D13)`]] },
    ];
    const warn = [];
    if (v1 === v0) warn.push(`The proposed limit equals the current limit (${v0} km/h): set the proposed limit on ${sc.name} (road inspector › Speed limit).`);
    if (!meas.length && !mm.length) warn.push('No measured mean operating speed: a modelled speed is not a mean operating speed. Enter a measured speed (road inspector › Speed limit) or load MegaMaps.');
    if (cx.consultationChanged) warn.push('The proposed limit changed after consultation: re-run this CBDS (cl 3.3(7)).');
    return { ok: true, ids: Ls.map(L => L.id), name: linkName(Ls[0]), current: v0, proposed: v1, history: ch, headings, warn, watermark: STD.labels.cbds, disclaimer: disclaimer('D9', P), decision: authorityOf(Ls[0], P, sc).decision };
  }

  /* =====================================================================================
     10. ANOP indicative multimodal LoS and fit (anop)
     ===================================================================================== */
  const AN = STD.anop, SCORE = AN.score, MODES = ['walk', 'cycle', 'pt', 'freight', 'gt'];
  const MODE_LABEL = { walk: 'Walking', cycle: 'Cycling', pt: 'Public transport', freight: 'Freight', gt: 'General traffic' };
  const LU = { 'city-metro': 'City or Metropolitan Centre', 'town-local': 'Town or Local Centre', special: 'THAB, education, hospital, stadium or RTN station', other: 'Outside centres' };
  const PREF = {
    gt: { motorway: ['C', 'C', 'C'], arterial: ['E', 'D', 'C'], collector: ['F', 'E', 'D'], local: ['F', 'F', 'E'] },
    freight: { l1: ['E', 'D', 'C'], l2: ['F', 'E', 'D'] },
    pt: { rtn: ['B', 'B', 'B'], ftn: ['C', 'C', 'C'], connector: ['D', 'D', 'D'], local: ['E', 'E', 'E'], peak: ['D', 'D', 'D'], none: ['F', 'F', 'E'] },
    cycle: { regional: ['B', 'B', 'B'], connector: ['D', 'D', 'D'], local: ['D', 'D', 'D'], none: ['E', 'E', 'E'] },
    walk: { primary: ['B', 'B', 'B'], secondary: ['C', 'C', 'C'], tertiary: ['D', 'D', 'D'], none: ['D', 'D', 'D'] },
  };
  const HIER_LABEL = { motorway: 'Motorway', arterial: 'Arterial', collector: 'Collector', local: 'Local', l1: 'Level 1A/1B', l2: 'Level 2/3/Access', rtn: 'RTN', ftn: 'FTN / IRTN / other strategic', connector: 'Connector', peak: 'Peak', none: 'None', regional: 'Regional / Major', primary: 'Primary', secondary: 'Secondary', tertiary: 'Tertiary' };
  const fromScore = s => Object.keys(SCORE).find(k => SCORE[k] === Math.max(0, Math.min(7, Math.round(s)))) || 'F';
  const colOf = lu => (lu === 'city-metro' ? 0 : lu === 'town-local' ? 1 : 2);
  /* Hierarchy of a corridor for a mode: length-weighted over the links conflated with AT Future Connect (L.fc); links
     without Future Connect data are left out of the tally and the share is stated. With no Future Connect on the corridor,
     general traffic uses the road class and the other modes a stated default. Walking is entered by the author. */
  const ROAD_H = L => (L.cls === 'motorway' || L.cls === 'ramp' ? 'motorway' : L.cls === 'arterial' ? 'arterial' : L.cls === 'collector' ? 'collector' : 'local');
  function hierOf(sc, c, mode) {
    if (mode === 'walk') return { h: PREF.walk[c.walk] ? c.walk : 'none', src: 'entered by the author (Future Connect has no walking layer)' };
    const tally = new Map(); let all = 0, fcLen = 0, roadFill = 0;
    for (const id of c.links) {
      const L = sc.links[id]; if (!L) continue; const w = Net.len(sc, L); all += w;
      if (!L.fc) continue;
      const fc = L.fc; let h = null; fcLen += w;
      if (mode === 'gt') { const v = fc.gt || ''; h = /motorway/i.test(v) ? 'motorway' : /arterial/i.test(v) ? 'arterial' : /collector/i.test(v) ? 'collector' : v ? 'local' : null; if (!h) { h = ROAD_H(L); roadFill += w; } }
      if (mode === 'freight') { const v = fc.freight || ''; h = /rail/i.test(v) ? null : /1A|1B/i.test(v) ? 'l1' : 'l2'; }
      if (mode === 'pt') { const v = fc.pt || ''; h = /rapid/i.test(v) ? 'rtn' : /frequent|ftn|irtn|strategic/i.test(v) ? 'ftn' : /connector/i.test(v) ? 'connector' : /peak/i.test(v) ? 'peak' : /local/i.test(v) ? 'local' : 'none'; }
      if (mode === 'cycle') { const v = fc.cycle || ''; h = /regional|major/i.test(v) ? 'regional' : /connector/i.test(v) ? 'connector' : /local|sports/i.test(v) ? 'local' : 'none'; }
      if (h) tally.set(h, (tally.get(h) || 0) + w);
    }
    if (fcLen > 0 && tally.size) {
      const best = [...tally].sort((a, b) => b[1] - a[1])[0][0], share = all > 0 ? fcLen / all : 1;
      return { h: best, src: `AT Future Connect${share < 0.995 ? ` (${Math.round(share * 100)}% of the corridor length has Future Connect data; the rest is left out)` : ''}${roadFill > 0 ? '; road class where Future Connect gives no general traffic class' : ''}` };
    }
    if (mode === 'gt') {
      for (const id of c.links) { const L = sc.links[id]; if (L) tally.set(ROAD_H(L), (tally.get(ROAD_H(L)) || 0) + Net.len(sc, L)); }
      const best = [...tally].sort((a, b) => b[1] - a[1])[0];
      return { h: best ? best[0] : 'local', src: 'road class (Future Connect not loaded)' };
    }
    const h = mode === 'freight' ? 'l2' : 'none';
    return { h, src: `default ${HIER_LABEL[h]} (Future Connect not loaded; ${ASSUMPTION}: load it in Traffic data)`, assumed: true };
  }
  // Preferred and acceptable LoS (tool's hidden sheet, with the land-use adjustment for freight and general traffic)
  function anopPref(mode, h, lu) {
    const row = (PREF[mode] || {})[h]; if (!row) return null;
    let pref = row[colOf(lu)];
    const adj = mode === 'gt' || mode === 'freight' ? (h === 'motorway' ? 0 : lu === 'city-metro' ? 2 : lu === 'town-local' || lu === 'special' ? 1 : 0) : 0;
    if (adj) pref = fromScore(Math.max(1, SCORE[pref] - adj));
    const acc = fromScore(Math.max(1, SCORE[pref] - (SCORE[pref] >= SCORE.B ? 2 : 1)));
    return { pref, acc, adj };
  }
  const bandSpeed = p => { if (p == null) return null; for (const [k, l] of AN.speedBands) if (p > k) return l; return 'F'; };
  const bandRel = r => { if (r == null || !(r > 0)) return null; for (const [k, l] of AN.relBands) if (r <= k) return l; return 'F'; };
  const worseOf = (...ls) => ls.filter(Boolean).sort((a, b) => SCORE[a] - SCORE[b])[0] || null;
  // The posted limit of a link and where it comes from (spec anop: the NSLR limit; the modelled speed only as a stated fallback)
  const postedOf = L => (+L.speedLimit > 0 ? { v: +L.speedLimit, src: 'legal limit' } : L.nslr && +L.nslr.v > 0 ? { v: +L.nslr.v, src: 'NSLR' }
    : +L.speed > 0 ? { v: +L.speed, src: 'modelled link speed' } : { v: (CLASSES[L.cls] || CLASSES.local).speed, src: 'road-class default speed' });
  // The corridor's axis: the bearing between its two furthest-apart nodes; arcs within 90° of it run in one direction
  function corridorAxis(sc, ids) {
    const pts = []; for (const id of ids) { const L = sc.links[id]; if (!L) continue; for (const n of [L.a, L.b]) { const x = sc.nodes[n]; if (x) pts.push([x.lat, x.lng]); } }
    if (pts.length < 2) return null;
    const far = from => pts.reduce((b, p) => (U.dist(from, p) > U.dist(from, b) ? p : b), pts[0]), A_ = far(pts[0]), B_ = far(A_);
    return U.dist(A_, B_) > 1 ? U.bearing(A_, B_) : null;
  }
  const bound = b => { try { const t = Kit.compass(b); if (t) return t; } catch { } return ['Northbound', 'Eastbound', 'Southbound', 'Westbound'][Math.round((((b % 360) + 360) % 360) / 90) % 4]; };
  /* Speed as % of the posted limit, per direction (spec anop): Σlength / Σ(link time incl. junction delay) against the
     length-weighted limit. Returns { dirs [{ name, spd, limit, pct, len }], spd, limit and pct of the worse direction, worst,
     limitSrc, fallback (true when a posted limit had to come from the modelled link speed) }. */
  function corridorSpeed(sc, res, ids, pt) {
    const axis = corridorAxis(sc, ids), acc = [{ d: 0, t: 0, lim: 0 }, { d: 0, t: 0, lim: 0 }], srcs = new Set();
    for (const id of ids) {
      const L = sc.links[id]; if (!L) continue;
      const g = Net.geom(sc, L), bAB = U.bearing(g[0], g[g.length - 1]), ps = postedOf(L);
      for (const k of ['ab', 'ba']) {
        const x = res && res.arcs[id + ':' + k]; if (!x || !(x.len > 0)) continue;
        const b = k === 'ab' ? bAB : (bAB + 180) % 360, i = axis == null || U.angDiff(b, axis) <= 90 ? 0 : 1;
        acc[i].d += x.len; acc[i].t += pt && L.busLane ? x.ff : x.t; acc[i].lim += x.len * ps.v; srcs.add(ps.src);
      }
    }
    const dirs = acc.map((a, i) => (a.t > 0 && a.d > 0 ? { name: axis == null ? 'Both directions' : bound(i ? (axis + 180) % 360 : axis), spd: a.d / a.t * 3.6, limit: a.lim / a.d, len: a.d } : null))
      .filter(Boolean).map(x => ({ ...x, pct: x.spd / x.limit * 100 }));
    if (!dirs.length) return null;
    const w = dirs.slice().sort((a, b) => a.pct - b.pct)[0], fallback = [...srcs].some(x => /modelled|default/.test(x));
    return { spd: w.spd, limit: w.limit, pct: w.pct, dirs, worst: w.name, limitSrc: [...srcs].join(' and '), fallback };
  }
  const dirText = (g, suffix = '') => (g ? `${g.dirs.map(d => `${d.name} ${num(d.pct, 0)}% (${num(d.spd, 0)} of ${num(d.limit, 0)} km/h, ${bandSpeed(d.pct)})`).join('; ')}${g.dirs.length > 1 ? '; worse direction used' : ''}${suffix}${g.fallback ? `; posted limit from the ${g.limitSrc} where no NSLR limit is loaded (${ASSUMPTION})` : ''}` : 'no result');
  function anopLoS(sc, res, c) {
    const out = {};
    const g = corridorSpeed(sc, res, c.links, false), p = corridorSpeed(sc, res, c.links, true), rel = c.rel || {};
    out.gt = { los: worseOf(bandSpeed(g && g.pct), bandRel(rel.gt)), metric: dirText(g), dirs: g ? g.dirs : [], rel: bandRel(rel.gt) ? `85th/median ${rel.gt}` : 'reliability N/A' };
    out.freight = { los: worseOf(bandSpeed(g && g.pct), bandRel(rel.freight)), metric: g ? dirText(g) + ' (general traffic speed)' : 'no result', dirs: g ? g.dirs : [], rel: bandRel(rel.freight) ? `85th/median ${rel.freight}` : 'reliability N/A' };
    out.pt = { los: worseOf(bandSpeed(p && p.pct), bandRel(rel.pt)), metric: dirText(p, ` (bus lanes at free-flow; ${ASSUMPTION})`), dirs: p ? p.dirs : [], rel: bandRel(rel.pt) ? `85th/median ${rel.pt}` : 'reliability N/A' };
    // walking: crossing opportunity spacing and crossing delay (bands halved in centres and outside schools)
    let len = 0; const sig = new Set();
    for (const id of c.links) { const L = sc.links[id]; if (!L) continue; len += Net.len(sc, L); for (const n of [L.a, L.b]) if ((sc.nodes[n] || {}).control === 'signals') sig.add(n); }
    const nX = nn(c.crossings) != null ? +c.crossings : sig.size, half = c.centre || c.school ? 0.5 : 1, spacing = len / Math.max(1, nX + 1);
    const sp = AN.crossBands.findIndex(b => spacing <= b * half), wl1 = LETTERS[sp < 0 ? 5 : sp];
    const dl = nn(c.crossDelay) != null ? LETTERS[(i => (i < 0 ? 5 : i))(AN.crossDelay.findIndex(b => +c.crossDelay < b))] : null;
    out.walk = { los: worseOf(wl1, dl), metric: `crossing every ${F.int(spacing)} m (${nX} crossing${nX === 1 ? '' : 's'}${nn(c.crossings) == null ? ' at signals' : ''})${dl ? `, delay ${c.crossDelay} s` : ''}` };
    // cycling: facility-based, length-weighted (Spare Construct mapping)
    let cs = 0, cl = 0;
    for (const id of c.links) { const L = sc.links[id]; if (!L) continue; const w = Net.len(sc, L), f = AN.cycleFacility, l = L.cycle === 'separated' ? f.separated : L.cycle === 'painted' ? f.painted : L.cls === 'arterial' ? f.none.arterial : f.none.other; cs += SCORE[l] * w; cl += w; }
    out.cycle = { los: cl ? fromScore(cs / cl) : null, metric: `facility-based (${AN.cycleFacility.basis})` };
    return out;
  }
  /* The fit rule. modes [{ base, pref, proj }] (letters; null = not assessed) → 'High' | 'Moderate' | 'Low' */
  function anopFit(modes) {
    const m = modes.filter(x => x.base && x.pref && x.proj).map(x => ({ b: SCORE[x.base], p: SCORE[x.pref], j: SCORE[x.proj] }));
    if (!m.length) return null;
    const dB = m.map(x => Math.max(0, x.p - x.b)), dJ = m.map(x => Math.max(0, x.p - x.j));
    const improves = m.some(x => x.j > x.b), worsens = m.some(x => x.j < x.b), falls = dJ.reduce((s, v) => s + v, 0) < dB.reduce((s, v) => s + v, 0), noRise = dJ.every((v, i) => v <= dB[i]);
    if ((improves && !worsens) || (falls && noRise)) return 'High';
    if (falls || noRise) return 'Moderate';
    return 'Low';
  }
  // Corridors: the author's list, or named arterials and collectors at least 300 m long in the active scenario
  function anopCorridors(P = Store.P) {
    const cx = data(P).anop; if (cx.corridors.length) return cx.corridors;
    const sc = activeSc(P), by = new Map();
    for (const L of Object.values(sc.links)) if (L.name && L.status !== 'closed' && (L.cls === 'arterial' || L.cls === 'collector')) { if (!by.has(L.name)) by.set(L.name, []); by.get(L.name).push(L); }
    return [...by].map(([name, Ls]) => ({ id: 'auto:' + name, name, links: Ls.map(L => L.id), landUse: 'other', walk: 'none', auto: true, len: Ls.reduce((s, L) => s + Net.len(sc, L), 0) }))
      .filter(c => c.len >= 300).sort((a, b) => b.len - a.len).slice(0, 12);
  }
  /* anop(P) → { rows [{ corridor, period, modes [{ mode, hier, pref, acc, base, proj, defBase, defProj, metric }], fit }], label } */
  const NO_FIT = 'Not assessed: no option to compare (the do-minimum and the option are the same scenario)';
  function anop(P = Store.P) {
    const pr = pair(P), rows = [], same = pr.base === pr.opt;
    for (const c of anopCorridors(P)) for (const p of periodsOf(pr.opt).filter(x => resOf(pr.base, x))) {
      const lb = anopLoS(pr.base, resOf(pr.base, p), c), lj = anopLoS(pr.opt, resOf(pr.opt, p), c), modes = [];
      for (const m of MODES) {
        const hs = hierOf(pr.opt, c, m), pf = anopPref(m, hs.h, c.landUse || 'other') || {}, base = lb[m].los, proj = lj[m].los;
        modes.push({ mode: m, label: MODE_LABEL[m], hier: HIER_LABEL[hs.h] || hs.h, hierKey: hs.h, hierSrc: hs.src, pref: pf.pref || null, acc: pf.acc || null, base, proj,
          defBase: pf.pref && base ? Math.max(0, SCORE[pf.pref] - SCORE[base]) : null, defProj: pf.pref && proj ? Math.max(0, SCORE[pf.pref] - SCORE[proj]) : null, metric: lj[m].metric, baseMetric: lb[m].metric });
      }
      rows.push({ corridor: c, period: p, modes, fit: same ? null : anopFit(modes), fitText: same ? NO_FIT : '', landUse: LU[c.landUse || 'other'] });
    }
    return { rows, base: pr.base.name, opt: pr.opt.name, same, note: same ? NO_FIT : '', label: STD.labels.anop, disclaimer: disclaimer('D5', P) };
  }
  // anop.csv in the v10.2 input sheet order
  function anopCSV(P = Store.P) {
    const cx = data(P).anop, r = anop(P), head = ['project', 'location', 'option', 'period', 'date', 'assessor', 'walk_net', 'cycle_net', 'pt_net', 'freight_net', 'gt_net', 'land_use',
      ...MODES.map(m => `base_los_${m}`), ...MODES.map(m => `manual_pref_${m}`), ...MODES.map(m => `project_los_${m}`), ...MODES.map(m => `notes_${m}`)];
    const rows = r.rows.map(x => { const M = Object.fromEntries(x.modes.map(m => [m.mode, m]));
      return [cx.project || P.name || '', x.corridor.name, cx.option || r.opt, x.period, today(), cx.assessor || '', ...MODES.map(m => M[m].hier), x.landUse,
        ...MODES.map(m => M[m].base || 'N/A'), ...MODES.map(() => 'Use ANOP'), ...MODES.map(m => M[m].proj || 'N/A'), ...MODES.map(m => `${M[m].metric}; indicative Spare Construct estimate`)]; });
    return csv([head, ...rows]);
  }

  /* =====================================================================================
     11. Parking, cycle parking, end-of-trip and loading (parking-compliance, reg-parking-ext)
     No minimum car parking: maxima only (entered per row, they vary by zone). Sum the activities, then round 0.5 up (E27.6.2(9)).
     ===================================================================================== */
  const PARK_ACT = {
    residential: { label: 'Residential', unit: 'dwellings', grp: 'res' }, office: { label: 'Office', unit: 'm² GFA', grp: 'other' },
    retail: { label: 'Retail (other)', unit: 'm² GFA', grp: 'retail', cycle: false }, food: { label: 'Food and beverage', unit: 'm² GFA', grp: 'retail', cycle: false },
    industrial: { label: 'Industrial', unit: 'm² GFA', grp: 'retail', cycle: false }, childcare: { label: 'Care centre', unit: 'm² GFA', grp: 'other', cycle: false },
    school: { label: 'Education facility', unit: 'm² GFA', grp: 'other', cycle: false }, tertiary: { label: 'Tertiary education', unit: 'm² GFA', grp: 'other', cycle: false },
    medical: { label: 'Healthcare facility', unit: 'm² GFA', grp: 'other', cycle: false }, hospital: { label: 'Hospital', unit: 'm² GFA', grp: 'other', cycle: false },
    community: { label: 'Community facility', unit: 'm² GFA', grp: 'other', cycle: false }, other: { label: 'Other non-residential', unit: 'm² GFA', grp: 'other', cycle: false },
  };
  const round9 = x => Math.floor(x + 0.5);
  function cycleRates(act, q, row) {
    if (act === 'residential') return { visitor: q >= 20 ? q / 20 : 0, secure: nn(row.noGarage) != null ? +row.noGarage : row.garage ? 0 : q, src: 'E27.6.2.5 (residential)' };
    if (act === 'office') return { visitor: q <= 200 ? 0 : q <= 1000 ? 1 : q <= 10000 ? 1 + (q - 1000) / 1000 : 10 + (q - 10000) / 2000, secure: q / 300, src: 'E27.6.2.5 (office)' };
    const v = nn(row.cycleVisitorReq), s = nn(row.cycleSecureReq);
    return { visitor: v, secure: s, src: v != null || s != null ? 'entered from Table E27.6.2.5' : 'Not encoded: enter the Table E27.6.2.5 rate for this activity' };
  }
  const showersFor = g => (g <= 500 ? 0 : g <= 2500 ? 1 : g <= 7500 ? 2 : 2 + 2 * Math.ceil((g - 7500) / 7500));
  // NZS 4121:2001 Table 2 (STD.accessible): 1 up to 20 car parks, 2 for 21-50, then 1 more per 50 or part
  const accessibleFor = n => { n = +n || 0; if (n <= 0) return 0; for (const [k, v] of STD.accessible.rows) if (n <= k) return v; const last = STD.accessible.rows[STD.accessible.rows.length - 1]; return last[1] + Math.ceil((n - last[0]) / STD.accessible.per); };
  const loadingFor = (grp, g) => grp === 'retail' ? (g <= 300 ? 0 : g <= 5000 ? 1 : g <= 10000 ? 2 : 2 + Math.ceil((g - 10000) / 10000))
    : grp === 'other' ? (g <= 5000 ? 0 : g <= 20000 ? 1 : g <= 90000 ? 2 : 2 + Math.ceil((g - 90000) / 40000)) : 0;
  const blankPark = () => ({ id: U.uid('pk'), activity: 'office', qty: 0, maxCars: null, cars: 0, accessible: 0, listedZone: false, garage: false, noGarage: null, cycleVisitor: 0, cycleSecure: 0, showers: 0, loading: 0, cycleVisitorReq: null, cycleSecureReq: null });
  /* parking(P) → { rows (Table P1), totals { visitor, secure, showers, loading } (required and proposed), allComply, label } */
  function parking(P = Store.P) {
    const a = A(P), rows = [];
    const tot = { visitor: 0, secure: 0, visitorP: 0, secureP: 0, showersP: 0, loading: 0, loadingP: 0, nonRes: 0, unknownCycle: false };
    for (const r0 of data(P).parking.rows) {
      const r = { ...blankPark(), ...r0 };
      const act = PARK_ACT[r.activity] || PARK_ACT.other, q = +r.qty || 0, cyc = cycleRates(r.activity, q, r);
      const cars = +r.cars || 0, max = nn(r.maxCars), over = max != null && cars > max ? cars - max : 0;
      const access4 = STD.parking.access4.includes(r.activity);
      // Accessible car parks: required where parking is provided (E27.6.3.2A(1)-(3)) or for a listed activity in a listed zone
      // with none (E27.6.3.2A(4)); the indicative number is NZS 4121:2001 Table 2, via NZBC D1/AS1 (verify)
      const accN = cars > 0 ? accessibleFor(cars) : 1;
      const accReq = cars > 0 ? { required: true, n: accN, why: `Car parking is provided: accessible parking is required (E27.6.3.2A(1)-(3)); about ${accN} for ${cars} car parks (${STD.accessible.name}: ${STD.accessible.basis}; verify)` }
        : access4 && r.listedZone ? { required: true, n: 1, why: 'No on-site parking, but this activity in a listed zone needs accessible parking (E27.6.3.2A(4)): at least 1' } : { required: false, n: 0, why: access4 ? 'Not required unless the zone is on the E27.6.3.2A(4) list: check it' : 'No car parking proposed' };
      const loadReq = act.grp === 'res' ? 0 : loadingFor(act.grp, q);
      if (act.grp !== 'res') tot.nonRes += q;
      if (cyc.visitor != null) tot.visitor += cyc.visitor; else tot.unknownCycle = true;
      if (cyc.secure != null) tot.secure += cyc.secure;
      tot.visitorP += +r.cycleVisitor || 0; tot.secureP += +r.cycleSecure || 0; tot.showersP += +r.showers || 0; tot.loading += loadReq; tot.loadingP += +r.loading || 0;
      // Cycle parking is judged on the totals only (E27.6.2(9): sum the activities, then round 0.5 up); rows show it unrounded
      const why = [];
      if (over) why.push(`${over} car parks above the AUP maximum of ${max} (restricted discretionary)`);
      if (accReq.required && !(+r.accessible > 0)) why.push('No accessible car park');
      else if (accReq.required && +r.accessible < accReq.n) why.push(`Fewer accessible car parks (${+r.accessible}) than the indicative ${accReq.n}`);
      if ((+r.loading || 0) < loadReq) why.push('Too few loading spaces');
      rows.push({ ...r, label: act.label, unit: act.unit, cyc, accReq, loadReq, over, max,
        induced: over ? { trips: over * a.tripsPerSpace, text: `About ${num(over * a.tripsPerSpace, 0)} induced peak commuter trips from ${over} extra spaces at ${a.tripsPerSpace} trips/space (${STD.parking.tripsPerSpaceBasis})`, status: 'Restricted discretionary (E27.4.1); assess the E27.8.1(5) matters and E27.8.2(4)(c): effects of the extra parking on the transport network, public transport, walking and cycling, and the trips it induces' } : null,
        complies: !why.length, why });
    }
    const showers = showersFor(tot.nonRes), need = { visitor: round9(tot.visitor), secure: round9(tot.secure), showers, loading: tot.loading };
    const totalsOk = tot.visitorP >= need.visitor && tot.secureP >= need.secure && tot.showersP >= showers && tot.loadingP >= tot.loading;
    const totalsWhy = [tot.visitorP < need.visitor ? `Too few visitor cycle parks in total (${tot.visitorP} of ${need.visitor})` : '', tot.secureP < need.secure ? `Too few secure cycle parks in total (${tot.secureP} of ${need.secure})` : '',
      tot.showersP < showers ? `Too few showers (${tot.showersP} of ${showers})` : '', tot.loadingP < tot.loading ? `Too few loading spaces in total (${tot.loadingP} of ${tot.loading})` : ''].filter(Boolean);
    return { rows, need, have: { visitor: tot.visitorP, secure: tot.secureP, showers: tot.showersP, loading: tot.loadingP }, unknownCycle: tot.unknownCycle, nonResGfa: tot.nonRes, totalsWhy,
      allComply: rows.every(r => r.complies) && totalsOk, totalsOk, label: `AUP E27.6.2-E27.6.3 (${STD.parking.edition}; verify). No minimum car parking applies: the AUP sets maxima only. Cycle parking is judged on the summed totals (E27.6.2(9)).` };
  }

  /* =====================================================================================
     12. Urban subdivision mode (reg-subdivision, AUP E38)
     ===================================================================================== */
  function distSkim(sc, fromIds) {
    const G = Engine.buildGraph(sc), cost = new Float64Array(G.M); for (let i = 0; i < G.M; i++) cost[i] = G.arcs[i].len || 0;
    const W = Engine.workspace(G), out = new Map();
    for (const id of fromIds) { const s = G.idx.get(id); if (s == null) continue; Engine.spt(G, cost, s, W); out.set(id, { G, nd: Float64Array.from(W.nd) }); }
    return out;
  }
  const blankAccess = () => ({ id: U.uid('acc'), name: '', sites: 1, legal: null, formed: null, len: null, ped: null });
  function rearAccess(a) {
    const n = +a.sites || 0, flags = [], R = STD.e38;
    if (n > R.maxSites) flags.push(`${n} rear sites share this access: no more than ${R.maxSites} rear sites per access (Table E38.8.1.2.1)`);
    const row = R.rows.find(r => n <= r.max) || R.rows[R.rows.length - 1];
    if (nn(a.formed) != null && +a.formed < row.formed) flags.push(`Formed width ${num(+a.formed, 1)} m is below ${num(row.formed, 1)} m for ${n} site${n === 1 ? '' : 's'}`);
    if (nn(a.legal) != null && +a.legal < row.legal) flags.push(`Legal width ${num(+a.legal, 1)} m is below ${num(row.legal, 1)} m for ${n} site${n === 1 ? '' : 's'}`);
    if (nn(a.len) != null && +a.len > row.len) flags.push(`Length ${F.int(+a.len)} m is over the ${row.len} m maximum`);
    if (row.ped && n > 5 && !(nn(a.ped) >= row.ped)) flags.push(`Needs separated pedestrian access at least ${row.ped} m wide`);
    return { ...a, row, flags, ok: !flags.length };
  }
  /* subdivision(P) → { lots, trips, links (E38.12.2(g)), connectivity (E38.12.2(e)), accesses, warning } */
  function subdivision(P = Store.P) {
    const cx = data(P).subdivision, pr = pair(P), sc = pr.opt, a = A(P), lots = +cx.lots || 0;
    let trips = [];
    try { trips = ['AM', 'PM'].map(p => { const e = Engine.tripEnds({ id: '_lots', kind: 'zone', lu: { residential: lots } }, Store.S, p, sc); return { period: p, O: e.O, D: e.D, total: e.O + e.D }; }); } catch { }
    const links = [];
    for (const p of periodsOf(sc).filter(x => resOf(pr.base, x) && pr.base !== sc)) {
      const rb = resOf(pr.base, p), ro = resOf(sc, p);
      for (const L of Object.values(pr.base.links)) {
        if (!sc.links[L.id] || L.status === 'proposed') continue;
        const vb = ['ab', 'ba'].reduce((s, d) => s + ((rb.arcs[L.id + ':' + d] || {}).v || 0), 0), vo = ['ab', 'ba'].reduce((s, d) => s + ((ro.arcs[L.id + ':' + d] || {}).v || 0), 0);
        const d = vo - vb; if (d < 1) continue;
        links.push({ id: L.id, name: linkName(L), period: p, base: vb, opt: vo, d, pct: pct(vo, vb), significant: d >= 20 && (pct(vo, vb) == null || pct(vo, vb) >= a.affectedPct) });
      }
    }
    links.sort((x, y) => y.d - x.d);
    // connectivity indicators for the development zones
    const dz = devZones(sc), con = { zones: dz.length };
    if (dz.length) {
      const c = [dz.reduce((s, z) => s + z.lat, 0) / dz.length, dz.reduce((s, z) => s + z.lng, 0) / dz.length], R = 800;
      const inter = Object.keys(sc.nodes).filter(id => roadLinks(sc, id).length >= 3 && U.dist([sc.nodes[id].lat, sc.nodes[id].lng], c) <= R).length;
      con.density = inter / (Math.PI * (R / 1000) ** 2); con.densityBasis = `junctions (3 or more roads) within ${R} m of the development centroid, per km²`;
      const stops = new Set(); for (const r of Object.values(sc.routes || {})) for (const n of r.wp || []) if (sc.nodes[n]) stops.add(n);
      for (const n of Object.values(sc.nodes)) if (n.stop) stops.add(n.id);
      const schools = Object.values(sc.zones).filter(z => luOf(z, 'school') > 0 && z.kind !== 'gateway');
      const sk = distSkim(sc, dz.map(z => 'z:' + z.id)); const dirs = [], wStop = [], wSchool = [];
      for (const z of dz) {
        const s = sk.get('z:' + z.id); if (!s) continue; const { G, nd } = s;
        for (const o of Object.values(sc.zones)) { if (o === z || o.kind === 'gateway') continue; const i = G.idx.get('z:' + o.id), cr = U.dist([z.lat, z.lng], [o.lat, o.lng]); if (i != null && isFinite(nd[i]) && cr > 200 && cr < 3000) dirs.push(nd[i] / cr); }
        const near = ids => Math.min(...ids.map(id => { const i = G.idx.get(id); return i != null ? nd[i] : Infinity; }));
        if (stops.size) { const d = near([...stops]); if (isFinite(d)) wStop.push(d / a.walkSpeed / 60); }
        const sch = schools.filter(o => o !== z); if (sch.length) { const d = near(sch.map(o => 'z:' + o.id)); if (isFinite(d)) wSchool.push(d / a.walkSpeed / 60); }
      }
      const avg = x => (x.length ? x.reduce((s, v) => s + v, 0) / x.length : null);
      con.directness = avg(dirs); con.directnessBasis = 'road-network distance ÷ straight-line distance to other zones 200 m to 3 km away (1.0 is perfectly direct)';
      con.walkStop = avg(wStop); con.walkSchool = avg(wSchool); con.walkBasis = `road-network distance at ${a.walkSpeed} m/s (${ASSUMPTION}); bus stops from modelled bus routes`;
    }
    // AUP Table E27.6.1.1 subdivision rows (T3A, T3B): from the Compliance lane's E27 data when loaded
    let e27Rows = null; try { const C = C_(); if (C && C.DATA && C.DATA.E27 && Array.isArray(C.DATA.E27.rows)) e27Rows = C.DATA.E27.rows.filter(r => r.key === 'subdivision'); } catch { }
    const e27src = e27Rows && e27Rows.length ? 'Compliance tab E27 data' : 'Compliance+ copy (verify)';
    if (!e27Rows || !e27Rows.length) e27Rows = [{ id: 'T3A', thr: 40, tier: 1 }, { id: 'T3B', thr: 100, tier: 2 }];
    const e27 = e27Rows.map(r => ({ id: r.id, thr: r.thr, tier: r.tier, exceeded: lots > r.thr }));
    const hit = e27.filter(r => r.exceeded).sort((a, b) => b.thr - a.thr)[0];
    const e27Text = lots > 0 ? (hit ? `${F.int(lots)} lots exceed Table E27.6.1.1 row ${hit.id} (${F.int(hit.thr)} dwellings of capacity): restricted discretionary under E27.6.1, tier ${hit.tier}; check the E27.6.1(2) exemptions and any precinct (${e27src})` : `${F.int(lots)} lots are within the Table E27.6.1.1 subdivision thresholds (${e27.map(r => `${r.id} ${r.thr}`).join(', ')}; ${e27src})`) : '';
    return { lots, trips, e27, e27Text, tripsBasis: 'Lot yield as dwellings at the project\'s residential trip rate; AUP E27 rows T3A/T3B decide whether the subdivision triggers E27.6.1', links: links.slice(0, 20),
      linksBasis: `'Significant increase' screen: at least 20 veh/h and ${a.affectedPct}% (${ASSUMPTION}); E38.12.2(g) is a judgment`, connectivity: con, accesses: (cx.accesses || []).map(rearAccess), warning: STD.labels.e38pc120 };
  }

  /* =====================================================================================
     13. Zone-to-zone travel times from a result (shortest paths on the assigned link times)
     ===================================================================================== */
  function skim(sc, res, fromIds, toIds) {
    const G = Engine.buildGraph(sc), S = Store.S, cost = new Float64Array(G.M);
    for (let i = 0; i < G.M; i++) { const a = G.arcs[i], r = res && res.arcs[a.key]; cost[i] = r ? r.t : Engine.arcTime(a, 0, S); }
    const W = Engine.workspace(G), out = new Map();
    for (const f of fromIds) {
      const s = G.idx.get('z:' + f); if (s == null) continue; Engine.spt(G, cost, s, W); const m = new Map();
      for (const t of toIds) { const i = G.idx.get('z:' + t); if (i != null && isFinite(W.nd[i])) m.set(t, W.nd[i]); }
      out.set(f, m);
    }
    return out;
  }
  const jobsOf = z => (z.jobs && +z.jobs.n) || +z.employees || 0;

  /* =====================================================================================
     14. Policy alignment (reg-policy-alignment)
     ===================================================================================== */
  const placeOf = L => { const x = L.cx || {}; if (nn(x.place) != null) return +x.place; const m = /P\s?([1-3])/i.exec(x.rasf || ''); return m ? +m[1] : null; };
  function affectedLinks(P = Store.P) {
    const pr = pair(P), sc = pr.opt, a = A(P);
    if (sc.extent && Array.isArray(sc.extent.links) && sc.extent.links.length) return { ids: new Set(sc.extent.links.map(x => (typeof x === 'string' ? arcLink(x.includes(':') ? x : x + ':ab') : x.link || x.id))), basis: 'stored extent-of-effects test (AT 5%/10% rule)' };
    const ids = new Set();
    for (const p of periodsOf(sc).filter(x => resOf(pr.base, x) && pr.base !== sc)) {
      const rb = resOf(pr.base, p), ro = resOf(sc, p);
      for (const [k, x] of Object.entries(ro.arcs)) { const b = rb.arcs[k]; const d = x.v - (b ? b.v : 0); if (d >= 20 && (!b || !(b.v > 0) || d / b.v * 100 >= a.affectedPct)) ids.add(arcLink(k)); }
    }
    for (const id of changedLinks(pr.base, sc)) ids.add(id);
    return { ids, basis: `flow change of at least 20 veh/h and ${a.affectedPct}%, or a changed road (${ASSUMPTION}; run the extent test for the AT rule)` };
  }
  function policy(P = Store.P) {
    const pr = pair(P), sc = pr.opt, base = pr.base, a = A(P), aff = affectedLinks(P), C = C_();
    let fds = null; try { if (C && C.fdsCheck) fds = C.fdsCheck(P); } catch { }
    const an = anop(P), fitLow = new Set(an.rows.filter(r => r.fit === 'Low').flatMap(r => r.corridor.links));
    const ps = periodsOf(sc).filter(p => resOf(base, p));
    const rows = [];
    for (const id of aff.ids) {
      const L = sc.links[id]; if (!L) continue;
      const B = base.links[id], fc = L.fc || {}, flags = [], place = placeOf(L);
      const spdUp = ps.some(p => { const o = resOf(sc, p), b = resOf(base, p); return ['ab', 'ba'].some(d => { const x = o.arcs[id + ':' + d], y = b.arcs[id + ':' + d]; return x && y && x.spd - y.spd >= a.speedUp; }); }) || (B && (+L.speed > +B.speed || limitOf(L) > posted(B)));
      if (place != null && place >= 3 && spdUp) flags.push(`The proposal raises vehicle speed on a high-Place (P${place}) RASF section`);
      if (B && (L.lanesAB | 0) + (L.lanesBA | 0) > (B.lanesAB | 0) + (B.lanesBA | 0) && fc.gt && !/arterial|motorway/i.test(fc.gt)) flags.push(`Adds traffic lanes where Future Connect classes the road ${fc.gt}`);
      if (fc.pt && /rapid|frequent|ftn|rtn/i.test(fc.pt) && !L.busLane && ps.some(p => { const x = resOf(sc, p).arcs[id + ':ab'], y = resOf(base, p).arcs[id + ':ab']; return x && y && x.t > y.t * 1.1; })) flags.push(`Slows a ${fc.pt} link by more than 10% without bus priority`);
      if (fitLow.has(id)) flags.push('ANOP fit is Low on this corridor');
      rows.push({ id, name: linkName(L), fc: ['gt', 'pt', 'freight', 'cycle'].map(k => fc[k] || '–'), rasf: (L.cx && L.cx.rasf) || (place != null ? `P${place}` : '–'), onf: (L.cx && L.cx.onf) || '–', flags });
    }
    rows.sort((x, y) => y.flags.length - x.flags.length || x.name.localeCompare(y.name));
    // conclusion checklist: alignment by mode from the ANOP results
    const modes = MODES.map(m => {
      const ms = an.rows.map(r => r.modes.find(x => x.mode === m)).filter(x => x && x.proj && x.acc);
      if (an.same) return { mode: MODE_LABEL[m], state: 'Not assessed', why: an.note };
      if (!ms.length) return { mode: MODE_LABEL[m], state: 'Not assessed', why: 'Run the model for the base and the option to assess (ANOP).' };
      const below = ms.filter(x => SCORE[x.proj] < SCORE[x.acc]), rise = ms.filter(x => x.defProj > x.defBase);
      return { mode: MODE_LABEL[m], state: below.length ? 'Not aligned' : rise.length ? 'Partly aligned' : 'Aligned', why: below.length ? `${below.length} corridor-period${below.length === 1 ? '' : 's'} below acceptable LoS` : rise.length ? `deficit rises on ${rise.length}` : 'at or above acceptable LoS, no deficit rises' };
    });
    // NPS-UD 1(c): jobs and services reachable from the development zones within the catchment, by mode
    const acc = accessibility(P);
    // per modelled peak hour, each period on its own (never AM + PM)
    const dVkt = perPeriod(base, sc, kpiP('vkt'), ps), dCo2 = perPeriod(base, sc, kpiP('co2'), ps), dVhtSaved = negPP(perPeriod(base, sc, kpiP('vht'), ps));
    const e = econOf(P, base, sc), haz = climate(P);
    const policyRows = [
      { ref: 'AT ITA s4.4, s6', policy: 'Network alignment conclusion, every mode', text: modes.map(m => `${m.mode}: ${m.state}`).join('; ') },
      { ref: 'NPS-UD Policy 1(c)', policy: 'Accessibility to jobs and services by walking, cycling, public transport and car', text: acc.text },
      { ref: 'NPS-UD Policy 1(e)', policy: 'Supports reductions in greenhouse gas emissions', text: ps.length ? `ΔVKT ${ppText(dVkt, 0, 'veh-km')} and ΔCO2e ${ppText(dCo2, 2, 't')} per modelled peak hour: ${STD.labels.vkt}.` : 'Run the base and the option.', info: true },
      { ref: 'NPS-I Policy 3', policy: 'Infrastructure identified in a strategic planning document', text: data(P).policy.npsiDoc || rltpRelied(P) },
      { ref: 'NPS-I Policy 7(1)(d)', policy: 'Method statement', text: 'Spare Construct sketch model: static user-equilibrium assignment, HCM 6 / Akcelik-based junctions, trip rates with stated sources; indicative, to be confirmed with approved tools (D1).' },
      { ref: 'FDS (RMA s104(1D), s168A(3C), s171(1D))', policy: 'Consistency with the Future Development Strategy', text: fds ? fds.text : 'Look up FDS timing in the Compliance tab.' },
      { ref: 'GPS 2024', policy: 'Economic growth and productivity', text: ps.length ? `${ppText(dVhtSaved, 1, 'veh·h')} saved per modelled peak hour` : '–' },
      { ref: 'GPS 2024', policy: 'Maintenance and resilience', text: haz.rows.length ? `${new Set(haz.rows.map(r => r.link)).size} assessed roads in flood or coastal hazard areas${haz.rows.some(r => r.only) ? ', including an only access' : ''}` : haz.loaded ? 'No assessed road in a mapped hazard area' : 'Check hazards (Traffic data › Data sources)' },
      { ref: 'GPS 2024', policy: 'Safety', text: (() => { const c = crashesOn(P, base, changedLinks(base, sc)); return c ? `${c.crashes} crashes (${c.dsi} DSI) on changed roads in the CAS window` : 'Load CAS crashes'; })() },
      { ref: 'GPS 2024', policy: 'Value for money', text: e && !e.mismatch && e.bcr != null ? `Indicative BCR ${e.bcrText || num(e.bcr, 1)} (${D4short})` : e && e.mismatch ? e.note : 'Not available' },
    ];
    return { rows, basis: aff.basis, modes, policy: policyRows, accessibility: acc, fds, label: STD.labels.vkt, inconsistencies: rows.filter(r => r.flags.length).length };
  }
  function accessibility(P = Store.P) {
    const pr = pair(P), sc = pr.opt, a = A(P), dz = devZones(sc), dests = Object.values(sc.zones).filter(z => z.kind !== 'gateway');
    if (!dz.length) return { text: 'No development zones.', rows: [] };
    const jobs = z => jobsOf(z) || 0, svc = z => (luOf(z, 'school') > 0 || luOf(z, 'medical') > 0 || luOf(z, 'retail') + luOf(z, 'supermarket') > 0 ? 1 : 0);
    const haveJobs = dests.some(z => jobs(z) > 0);
    const w = dests.map(z => ({ z, w: haveJobs ? jobs(z) : GFA_KEYS.reduce((s, k) => s + luOf(z, k), 0) }));
    const lim = a.catchMin * 60, dist = distSkim(sc, dz.map(z => 'z:' + z.id)), r = periodsOf(sc)[0] ? resOf(sc, periodsOf(sc)[0]) : null;
    const car = r ? skim(sc, r, dz.map(z => z.id), dests.map(z => z.id)) : null;
    const tally = { walk: [0, 0], cycle: [0, 0], car: [0, 0] };
    for (const z of dz) {
      const d = dist.get('z:' + z.id);
      for (const { z: o, w: wt } of w) {
        const i = d ? d.G.idx.get('z:' + o.id) : null, m = i != null ? d.nd[i] : Infinity;
        if (m / a.walkSpeed <= lim) { tally.walk[0] += wt; tally.walk[1] += svc(o); }
        if (m / (a.cycleSpeed / 3.6) <= lim) { tally.cycle[0] += wt; tally.cycle[1] += svc(o); }
        const t = car && car.get(z.id) ? car.get(z.id).get(o.id) : null;
        if (t != null && t <= lim) { tally.car[0] += wt; tally.car[1] += svc(o); }
      }
    }
    const n = dz.length, unit = haveJobs ? 'jobs' : 'm² commercial GFA (no job data: fill employment in the Land use tab)';
    const rows = [['Walking', tally.walk], ['Cycling', tally.cycle], ['Public transport', null], ['Car', car ? tally.car : null]].map(([m, t]) => ({ mode: m, value: t ? t[0] / n : null, services: t ? t[1] / n : null }));
    return { rows, unit, text: rows.map(x => `${x.mode}: ${x.value == null ? (x.mode === 'Public transport' ? 'not modelled (use the AT journey planner or MSM)' : 'run the model') : `${F.int(x.value)} ${haveJobs ? 'jobs' : 'm² GFA'}, ${num(x.services, 1)} zones with services`}`).join('; ') + ` within ${a.catchMin} min (average over development zones; ${ASSUMPTION} speeds).`, basis: unit };
  }

  /* =====================================================================================
     15. IPM 2027-30 stage 2 GPS metrics (reg-gps-metrics)
     ===================================================================================== */
  const SIG = { road: 'Road', regional: 'Regionally significant', national: 'Nationally significant' };
  function freightRating(saving, sig) {
    if (sig === 'national' && saving > 30) return 'Very High';
    if (sig === 'national' && saving > 10) return 'High';
    if (sig === 'regional' && saving > 30) return 'Medium';
    if (saving > 5) return 'Low';
    return 'Below the Low threshold';
  }
  const gtRating = saving => (saving > 5 ? 'High' : saving > 0 ? 'Medium' : 'No improvement');
  /* Routes: the author's (links by id, or by road names), otherwise the scheme network (arterials and motorways) with its
     significance NOT set: Spare Construct never asserts a route's significance. An unset class is rated as 'Road' and says so. */
  const roadsToLinks = (sc, names) => { const want = new Set(String(names || '').split(/[,;\n]+/).map(x => x.trim().toLowerCase()).filter(Boolean)); return want.size ? Object.values(sc.links).filter(L => L.status !== 'closed' && want.has(String(L.name || '').toLowerCase())).map(L => L.id) : []; };
  function gpsRoutes(P = Store.P) {
    const sc = activeSc(P), own = (data(P).gps.routes || []).filter(r => r && (r.name || r.roads || (r.links || []).length))
      .map(r => ({ ...r, links: Array.isArray(r.links) && r.links.length ? r.links : roadsToLinks(sc, r.roads) }));
    if (own.length) return own;
    const ids = Object.values(sc.links).filter(L => (L.cls === 'arterial' || L.cls === 'motorway') && L.status !== 'closed').map(L => L.id);
    return ids.length ? [{ id: 'auto', name: 'Scheme network (arterials and motorways)', links: ids, significance: '', auto: true }] : [];
  }
  function gpsMetrics(P = Store.P) {
    const pr = pair(P), cx = data(P).gps, ps = periodsOf(pr.opt).filter(p => resOf(pr.base, p)), out = { routes: [], label: STD.labels.gps, verified: STD.gps.verified };
    const time = (sc, p, ids) => ids.reduce((s, id) => s + ['ab', 'ba'].reduce((t, d) => { const x = resOf(sc, p).arcs[id + ':' + d]; return t + (x ? x.t : 0); }, 0), 0);
    let tb = 0, to = 0, sig = 'road', set = false;
    for (const r of gpsRoutes(P)) {
      const ids = (r.links || []).filter(id => pr.base.links[id] && pr.opt.links[id]); if (!ids.length || !ps.length) continue;
      const b = ps.reduce((s, p) => s + time(pr.base, p, ids), 0), o = ps.reduce((s, p) => s + time(pr.opt, p, ids), 0), saving = b > 0 ? (b - o) / b * 100 : 0;
      if (SIG[r.significance]) set = true;
      tb += b; to += o; if (r.significance === 'national' || (r.significance === 'regional' && sig === 'road')) sig = r.significance;
      out.routes.push({ name: r.name || r.roads || 'Route', significance: SIG[r.significance] || 'Not set', base: b, opt: o, saving, auto: !!r.auto });
    }
    const saving = tb > 0 ? (tb - to) / tb * 100 : null;
    out.network = { saving, significance: set ? SIG[sig] : `Not set (rated as Road, ${ASSUMPTION}: set each route's significance)`, sigSet: set, hcv: saving == null ? null : freightRating(saving, sig), gt: saving == null ? null : gtRating(saving),
      note: 'Measured at scheme network level (sum over the routes), not per segment. Spare Construct has one vehicle class, so HCV and general-traffic travel times are the same. Reliability is not modelled: enter it from observed data.' };
    // minutes saved to tagged destinations, trip-weighted over origins
    out.destinations = [];
    const dests = cx.destinations.filter(d => pr.opt.zones[d.zoneId]);
    if (dests.length && ps.length) {
      const p = ps[0], ro = resOf(pr.opt, p), rb = resOf(pr.base, p), orig = (ro.zoneIds || []).filter(id => pr.base.zones[id]);
      const sb = skim(pr.base, rb, orig, dests.map(d => d.zoneId)), so = skim(pr.opt, ro, orig, dests.map(d => d.zoneId)), Z = (ro.zoneIds || []).length;
      for (const d of dests) {
        let w = 0, s = 0; const j = ro.zoneIds.indexOf(d.zoneId);
        for (const o of orig) { const i = ro.zoneIds.indexOf(o), t0 = sb.get(o) && sb.get(o).get(d.zoneId), t1 = so.get(o) && so.get(o).get(d.zoneId); if (t0 == null || t1 == null) continue; const q = ro.T && j >= 0 ? ro.T[i * Z + j] || 0 : 1; w += q; s += q * (t0 - t1); }
        out.destinations.push({ name: pr.opt.zones[d.zoneId].name || d.zoneId, significance: SIG[d.significance] || 'Road', minutes: w > 0 ? s / w / 60 : null, period: p });
      }
    }
    // houses enabled by stage, counted when 75% complete by 2035
    const stages = cx.stages.length ? cx.stages : devZones(pr.opt).map(z => ({ name: z.name || z.id, dwellings: dwellings(z, pr.opt), year75: +z.year || null, derived: true }));
    out.houses = { rows: stages.map(s => ({ ...s, counted: +s.year75 > 0 && +s.year75 <= STD.gps.by })), by: STD.gps.by, basis: `A stage counts when 75% complete by ${STD.gps.by}${cx.stages.length ? '' : '; years are the zone opening years: replace them with the year each stage is 75% complete'}` };
    out.houses.total = out.houses.rows.filter(r => r.counted).reduce((s, r) => s + (+r.dwellings || 0), 0);
    const cost = nn(cx.cost) != null ? +cx.cost : (() => { try { return Engine.capital(pr.opt).total - Engine.capital(pr.base).total; } catch { return null; } })();
    out.dsi = { reduction: nn(cx.dsiReduction), cost, per100m: nn(cx.dsiReduction) != null && cost > 0 ? +cx.dsiReduction / cost * 100 : null, collective: cx.collective || 'Not entered' };
    return out;
  }

  /* =====================================================================================
     16. Funding attribution (reg-funding-attribution)
     ===================================================================================== */
  const inPoly = (pt, poly) => { let c = false; for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) { const [yi, xi] = poly[i], [yj, xj] = poly[j]; if (((yi > pt[0]) !== (yj > pt[0])) && pt[1] < (xj - xi) * (pt[0] - yi) / (yj - yi) + xi) c = !c; } return c; };
  function growthShare(P = Store.P, sc = activeSc(P)) {
    const em = data(P).funding.employment || {};
    let He = 0, Hf = 0;
    for (const z of Object.values(sc.zones)) { if (z.kind === 'gateway') continue; const h = luOf(z, 'residential') + luOf(z, 'apartment') + luOf(z, 'retirement'); const k = Engine.zoneClass(z); if (k === 'existing') He += h; Hf += h * (k === 'existing' ? 1 : Engine.stageOf(z, sc)); }
    const jz = Object.values(sc.zones).filter(z => z.kind !== 'gateway');
    const Ee = nn(em.existing) != null ? +em.existing : jz.filter(z => Engine.zoneClass(z) === 'existing').reduce((s, z) => s + jobsOf(z), 0);
    const Ef = nn(em.future) != null ? +em.future : jz.reduce((s, z) => s + jobsOf(z), 0);
    const den = Hf + 0.5 * Ef, share = den > 0 ? ((Hf - He) + 0.5 * (Ef - Ee)) / den : null;
    return { He, Hf, Ee, Ef, share, formula: '((future − existing) households + 0.5 × (future − existing) employment) ÷ (future households + 0.5 × future employment)', label: STD.labels.dcMethod };
  }
  /* funding(P) → { assets [{ name, period, existing, committed, development, devShare }], growth, incidence { inside, outside, insideShare, zones }, bcrG, dc, caveat } */
  // The upgraded assets: chosen link ids, or roads named in funding.assetRoads, or (by default) the roads the option changes
  function fundingAssets(P = Store.P, sc = pair(P).opt) {
    const cx = data(P).funding, own = [...new Set([...(cx.assets || []).filter(id => sc.links[id]), ...roadsToLinks(sc, cx.assetRoads)])];
    return own.length ? { ids: own, basis: 'chosen assets' } : { ids: [...changedLinks(pair(P).base, sc)], basis: 'roads the option changes (choose the upgraded assets to change this)' };
  }
  /* Select-link shares by development area (reg-funding-attribution): one select-zone assignment per development zone and
     period (Engine.assign select { zone }), giving the flow on each upgraded asset from trips to or from that zone. A trip
     between two development zones counts in both, so area shares can add to more than the development total (stated). */
  async function fundingShares(P = Store.P, opts = {}) {
    const pr = pair(P), sc = pr.opt, FA = fundingAssets(P, sc), ids = FA.ids, zs = devZones(sc), periods = opts.periods || periodsOf(sc).filter(p => p !== 'IP');
    if (!ids.length) return { ok: false, why: 'No upgraded assets: choose them (road names) or make the option change a road.' };
    if (!zs.length) return { ok: false, why: 'No development zones in the option.' };
    if (!periods.length) return { ok: false, why: 'Run the option first.' };
    const flow = (r, id) => ['ab', 'ba'].reduce((t, d) => t + (((r.arcs || {})[id + ':' + d] || {}).v || 0), 0);
    const out = { ok: true, at: new Date().toISOString(), scId: sc.id, periods, basis: FA.basis, assets: [], method: 'Select-zone assignment per development zone (Engine select { zone }); a trip between two development zones counts for both' };
    let done = 0; const total = zs.length * periods.length;
    for (const p of periods) {
      const r = resOf(sc, p) || await runSc(sc, p);
      const tot = Object.fromEntries(ids.map(id => [id, flow(r, id)])), by = Object.fromEntries(ids.map(id => [id, []]));
      for (const z of zs) {
        const rz = await runSc(sc, p, { select: { zone: z.id }, gapTol: opts.gapTol, maxIter: opts.maxIter }), fl = (rz.select && rz.select.flows) || {};
        for (const id of ids) { const v = ['ab', 'ba'].reduce((t, d) => t + (+fl[id + ':' + d] || 0), 0); if (v > 0) by[id].push({ zone: z.id, name: z.name || z.id, v, share: tot[id] > 0 ? v / tot[id] : null }); }
        done++; if (opts.onProgress) try { opts.onProgress(done / total, `${z.name || z.id}, ${p}`); } catch { }
      }
      for (const id of ids) if (tot[id] > 0) out.assets.push({ id, name: linkName(sc.links[id]), period: p, total: tot[id], areas: by[id].sort((a, b) => b.v - a.v) });
    }
    if (opts.save !== false) data(P).funding.shares = out;
    return out;
  }
  function funding(P = Store.P) {
    const pr = pair(P), cx = data(P).funding, sc = pr.opt, ps = periodsOf(sc);
    const FA = fundingAssets(P, sc), ids = FA.ids;
    const assets = [];
    for (const p of ps) { const r = resOf(sc, p), cls = r.cls && r.cls.arcs; if (!cls) continue;
      for (const id of ids) { const f = [0, 0, 0]; for (const d of ['ab', 'ba']) { const x = cls[id + ':' + d]; const v = ((r.arcs[id + ':' + d] || {}).v) || 0; if (x) { f[0] += x[0]; f[1] += x[1]; f[2] += x[2]; } else f[0] += v; }
        const t = f[0] + f[1] + f[2]; if (t > 0) assets.push({ id, name: linkName(sc.links[id]), period: p, existing: f[0], committed: f[1], development: f[2], devShare: f[2] / t }); } }
    // benefit incidence: rule-of-half OD time savings, half to each end, inside vs outside the levy polygon
    let incidence = null; const poly = (cx.polygon || []).filter(x => Array.isArray(x) && x.length === 2);
    const p0 = ps.find(p => resOf(pr.base, p));
    if (poly.length >= 3 && p0 && pr.base !== sc) {
      const ro = resOf(sc, p0), rb = resOf(pr.base, p0), zo = ro.zoneIds || [], zb = rb.zoneIds || [], Zo = zo.length, Zb = zb.length;
      if (ro.T && rb.T) {
        const ids2 = zo.filter(id => sc.zones[id]), sb = skim(pr.base, rb, ids2.filter(id => pr.base.zones[id]), ids2), so = skim(sc, ro, ids2, ids2), by = new Map();
        for (const i of ids2) for (const j of ids2) {
          const t0 = sb.get(i) && sb.get(i).get(j), t1 = so.get(i) && so.get(i).get(j); if (t0 == null || t1 == null || i === j) continue;
          const qo = ro.T[zo.indexOf(i) * Zo + zo.indexOf(j)] || 0, ib = zb.indexOf(i), jb = zb.indexOf(j), qb = ib >= 0 && jb >= 0 ? rb.T[ib * Zb + jb] || 0 : 0;
          const ben = 0.5 * (qb + qo) * (t0 - t1) / 3600; if (!ben) continue;
          for (const z of [i, j]) by.set(z, (by.get(z) || 0) + ben / 2);
        }
        let inside = 0, outside = 0; const zones = [];
        for (const [id, b] of by) { const z = sc.zones[id], isIn = inPoly([z.lat, z.lng], poly); if (isIn) inside += b; else outside += b; zones.push({ id, name: z.name || id, benefit: b, inside: isIn }); }
        const tot = inside + outside;
        incidence = { period: p0, inside, outside, insideShare: tot ? inside / tot : null, zones: zones.sort((a, b) => b.benefit - a.benefit).slice(0, 15), basis: 'Rule-of-half veh·h saved per modelled peak hour, half to each trip end; the same share applies to PV benefits (one discounting profile)' };
      } else incidence = { error: 'Benefit incidence needs the OD matrix (150 zones or fewer).' };
    }
    const e = p0 ? econOf(P, pr.base, sc) : null, levy = +cx.levy || 0;
    const bcrG = e && !e.mismatch && e.pv != null ? (e.cap - levy > 0 ? { value: e.pv / (e.cap - levy), text: `${num(e.pv / (e.cap - levy), 1)} (PV benefits ${F.money(e.pv)} ÷ (cost ${F.money(e.cap)} − levy ${F.money(levy)}))` } : { value: null, text: 'The levy covers the cost: BCR_G is not meaningful' }) : { value: null, text: e && e.mismatch ? e.note : 'Run the base and the option' };
    const appr = day(cx.approval) || day(data(P).ftaa.approval);
    const sh = cx.shares && cx.shares.ok && cx.shares.scId === sc.id ? cx.shares : null;
    return { assets, assetsBasis: FA.basis, shares: sh, growth: growthShare(P, sc), incidence, bcrG, levy,
      dc: appr ? { approval: appr, until: addMonths(appr, STD.ftaa.dcMonths), text: `Fast-track approval ${longDate(appr)}: the development contributions policy window under LGA s102A(3) runs to ${longDate(addMonths(appr, STD.ftaa.dcMonths))}.` } : null,
      caveat: STD.labels.funding };
  }

  /* =====================================================================================
     17. Time-of-use screening (reg-tou). Available from 18 Nov 2026 (commencement).
     ===================================================================================== */
  function touCharged(sc, poly, kind) {
    const out = new Map();
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed') continue;
      const A_ = sc.nodes[L.a], B_ = sc.nodes[L.b]; if (!A_ || !B_) continue;
      const ia = inPoly([A_.lat, A_.lng], poly), ib = inPoly([B_.lat, B_.lng], poly);
      if (kind === 'area' ? ia && ib : ia !== ib) out.set(L.id, kind === 'area' ? 'inside' : ia ? 'leaves' : 'enters');
    }
    return out;
  }
  /* Zone-to-zone generalised and real times on a time-of-use run. cost = the link's real time (res.arcs t, which never
     includes the charge) + the full charge as time on each charged link, so the skim's route choice sees the true
     charge. Per OD: gc (s), n (charged links crossed), real (gc − n × pen). (applied is kept for older callers: an arc's
     time already added by an earlier speed-based method, taken off with its congestion factor.) */
  function touSkim(G, r, charged, applied, pen, zs, S) {
    const cost = new Float64Array(G.M), chg = new Uint8Array(G.M);
    for (let i = 0; i < G.M; i++) {
      const a = G.arcs[i], x = r && r.arcs[a.key], ap = applied ? applied.get(a.key) || 0 : 0;
      let t = x ? x.t : Engine.arcTime(a, 0, S);
      if (x && ap > 0 && x.ff > 0) t = Math.max(0.1, t - ap * Math.max(1, (x.t - (x.nd || 0)) / x.ff));
      if (charged && !a.conn && charged.has(a.link)) { chg[i] = 1; t += pen; }
      cost[i] = t;
    }
    const W = Engine.workspace(G), out = new Map();
    for (const f of zs) {
      const s = G.idx.get('z:' + f); if (s == null) continue;
      Engine.spt(G, cost, s, W); const m = new Map();
      for (const t of zs) {
        if (t === f) continue; const i = G.idx.get('z:' + t); if (i == null || !isFinite(W.nd[i])) continue;
        let n = 0; if (charged) for (const ai of Engine.pathTo(G, W, i) || []) if (chg[ai]) n++;
        m.set(t, { gc: W.nd[i], n, real: W.nd[i] - n * pen });
      }
      out.set(f, m);
    }
    return out;
  }
  /* Time-of-use screening (reg-tou). The charge (light and heavy averaged by the HV share: Spare Construct has one vehicle class) is
     an additive generalised-cost term on each charged road (charge ÷ value of time, in seconds), passed to the assignment
     as opts.arcPenalty: it steers route choice and the equilibrium, but res.arcs t, VHT and delay never include it, so
     real times come straight from the run and the whole charge reaches every charged arc (no speed floor, no leak).
     The gravity distribution does not see the charge; the demand response is one elastic step on generalised time,
     which sets a demand factor for the network run. Benefits: rule
     of half per OD pair on generalised cost (time plus the charge); the charge is a transfer, so national benefits are user
     benefit + revenue (time saved less the deadweight loss of priced-off trips) + VOC, CO2 and safety from the VKT change,
     and BCR_G counts revenue against the cost (MBCM v1.7.5, NZTA GC 26/01). Discounting at the project's MBCM schedule
     (2% to year 30, then 1.5%: NZTA GC 25/01), benefits flat. */
  async function touRun(P = Store.P, opts = {}) {
    const cx = data(P).tou, now = opts.today || today();
    if (now < STD.dates.tou) return { ok: false, why: `Time-of-use schemes can be proposed from ${longDate(STD.dates.tou)}, when LTMA ss65A-65ZO commence.` };
    const poly = (cx.polygon || []).filter(x => Array.isArray(x) && x.length === 2);
    if (poly.length < 3) return { ok: false, why: 'Draw or enter the charging area (at least 3 points).' };
    const sc = pair(P).opt, period = opts.period || cx.period || 'AM', S = P.settings, E = S.econ || {}, a = A(P), hv = (+S.hvPct || 0) / 100;
    const charge = (+cx.light || 0) * (1 - hv) + (+cx.heavy || 0) * hv, vot = +E.vot || 32, pen = charge / vot * 3600;
    const charged = touCharged(sc, poly, cx.kind === 'area' ? 'area' : 'cordon'); if (!charged.size) return { ok: false, why: 'No road crosses (or lies inside) the charging area.' };
    const r0 = resOf(sc, period) || await runSc(sc, period);
    if (!r0.T || !(r0.zoneIds || []).length) return { ok: false, why: 'The screening needs the OD matrix, which Spare Construct keeps for 150 zones or fewer.' };
    // the charge as an additive cost on every charged link, both directions (Engine.assign opts.arcPenalty, seconds)
    const arcPenalty = Object.fromEntries([...charged.keys()].map(id => [id, pen]));
    const G0 = Engine.buildGraph(sc, S), G1 = G0, applied = null, leaks = [];
    const zs = r0.zoneIds.filter(id => sc.zones[id]), Z = r0.zoneIds.length, zi = new Map(r0.zoneIds.map((id, k) => [id, k]));
    const s0 = touSkim(G0, r0, null, null, 0, zs, S);
    const r1 = await runSc(sc, period, { arcPenalty }), s1 = touSkim(G1, r1, charged, applied, pen, zs, S);
    // one elastic-demand step on generalised time
    const ods = []; let T0 = 0, T1 = 0;
    for (const i of zs) for (const j of zs) {
      if (i === j) continue;
      const q = r0.T[zi.get(i) * Z + zi.get(j)] || 0, b0 = s0.get(i) && s0.get(i).get(j), b1 = s1.get(i) && s1.get(i).get(j);
      if (!(q > 0) || !b0 || !b1 || !(b0.gc > 0)) continue;
      const q1 = q * Math.pow(b1.gc / b0.gc, a.elasticity); T0 += q; T1 += q1;
      ods.push({ i, j, from: sc.zones[i].name || i, to: sc.zones[j].name || j, q0: q, q1, t0: b0.gc });
    }
    const dF = T0 > 0 ? T1 / T0 : 1;
    const r2 = await runSc(sc, period, { arcPenalty, demandFactor: dF }), s2 = touSkim(G1, r2, charged, applied, pen, zs, S);
    // rule of half per OD on generalised cost; revenue and the deadweight loss of priced-off trips (NZ$ per modelled hour)
    let user = 0, time = 0, revenue = 0, dwl = 0; const inc = new Map();
    for (const o of ods) {
      const b = s2.get(o.i) && s2.get(o.i).get(o.j); if (!b) continue;
      const pay = b.n * charge; Object.assign(o, { gc1: b.gc, t1: b.real, n: b.n, paid: b.n > 0, pay });
      user += 0.5 * (o.q0 + o.q1) * (o.t0 - b.gc) / 3600 * vot; time += 0.5 * (o.q0 + o.q1) * (o.t0 - b.real) / 3600 * vot;
      revenue += o.q1 * pay; dwl += 0.5 * (o.q0 - o.q1) * pay;
      const z = inc.get(o.i) || { id: o.i, name: o.from, trips: 0, paidTrips: 0, dGc: 0, dReal: 0, income: sc.zones[o.i].cx && sc.zones[o.i].cx.income };
      z.trips += o.q0; if (o.paid) z.paidTrips += o.q1; z.dGc += o.q0 * (b.gc - o.t0); z.dReal += o.q0 * (b.real - o.t0); inc.set(o.i, z);
    }
    // network VKT, real VHT and CO2 (res.arcs t is real time: the charge is never in it)
    const realT = (x, k, ap) => (ap && ap.get(k) > 0 && x.ff > 0 ? Math.max(0.1, x.t - ap.get(k) * Math.max(1, (x.t - (x.nd || 0)) / x.ff)) : x.t);
    const byNet = (r, sh, ap) => { let vkt = 0, vht = 0, co2 = 0; for (const [k, x] of Object.entries(r.arcs)) { const L = sc.links[arcLink(k)]; if (!L || (sh != null && isSH(L, P) !== sh) || !(x.len > 0)) continue; const t = realT(x, k, ap); vkt += x.v * x.len / 1000; vht += x.v * t / 3600; co2 += x.v * x.len / 1000 * Engine.ef(x.len / t * 3.6, S) / 1e6; } return { vkt, vht, co2 }; };
    const n0 = byNet(r0, null, null), n2 = byNet(r2, null, applied), pt = r => Object.values(r.routes || {}).reduce((s, x) => s + (+x.t || 0), 0);
    const hrs = (((E.periodHours || {})[period]) ?? 600) || 600, mb = (Engine.STANDARDS && Engine.STANDARDS.mbcm) || {};
    const rates = Array.isArray(E.rates) && E.rates.length ? E.rates : mb.rates || [{ to: 30, r: 2 }, { to: 60, r: 1.5 }], yrs = Math.max(1, Math.round(+E.years || mb.years || 40));
    const m = x => x * hrs / 1e6;
    const annual = { user: m(user), time: m(time), charges: -m(revenue + dwl), deadweight: m(dwl), revenue: m(revenue),
      voc: m((n0.vkt - n2.vkt) * a.vocPerKm), co2: m((n0.co2 - n2.co2) * a.co2PerT), safety: m((n0.vkt - n2.vkt) * a.crashPerKm) };
    const ext = annual.voc + annual.co2 + annual.safety;
    annual.total = annual.user + annual.revenue + ext;   // national: the charge is a transfer
    annual.government = annual.user + ext;               // BCR_G numerator: revenue goes against the cost
    let uspwf = yrs; try { uspwf = Engine.pwf(rates, yrs).uspwf; } catch { }
    const pvB = annual.total * uspwf, pvG = annual.government * uspwf, pvR = annual.revenue * uspwf, cost = +cx.cost || 0;
    const paid = ods.filter(o => o.paid), unpaid = ods.filter(o => !o.paid && o.t1 != null);
    const res = { ok: true, at: new Date().toISOString(), period, charge, pen, vot, kind: cx.kind, charged: charged.size, demandFactor: dF, leaks,
      sample: paid.sort((x, y) => y.q0 - x.q0).slice(0, 12).concat(unpaid.sort((x, y) => y.q0 - x.q0).slice(0, 4)).map(o => ({ from: o.from, to: o.to, q0: o.q0, q1: o.q1, t0: o.t0, t1: o.t1, paid: o.paid, pay: o.pay })),
      impacts: { sh: [byNet(r0, true, null), byNet(r2, true, applied)], local: [byNet(r0, false, null), byNet(r2, false, applied)], pt: [pt(r0), pt(r2)], total: [n0, n2] },
      incidence: [...inc.values()].map(z => ({ ...z, dtAvg: z.trips ? z.dReal / z.trips : 0, dGcAvg: z.trips ? z.dGc / z.trips : 0, paidShare: z.trips ? z.paidTrips / z.trips : 0 })).sort((x, y) => y.paidTrips - x.paidTrips).slice(0, 20),
      annual, revenue: annual.revenue, pvB, pvG, pvR, cost, uspwf, years: yrs, rates,
      bcrN: cost > 0 ? pvB / cost : null, bcrG: cost - pvR > 0 ? pvG / (cost - pvR) : null, bcrGText: cost > 0 && cost - pvR <= 0 ? 'Revenue covers the cost: BCR_G is not meaningful' : '',
      label: STD.labels.tou, appraisal: STD.tou.appraisal,
      method: `Charge NZ$${num(charge, 2)} (light ${cx.light}, heavy ${cx.heavy}, HV ${num(hv * 100, 0)}%; one vehicle class) as ${num(pen / 60, 1)} min at NZ$${vot}/h of generalised cost on ${charged.size} charged road${charged.size === 1 ? '' : 's'}${cx.kind === 'area' ? '' : ' (both directions of each cordon crossing)'}, in route choice and the equilibrium (Spare Construct's additive arc cost; travel times, VHT and delay exclude it)`
        + `; the gravity distribution does not see the charge; one elastic step (elasticity ${a.elasticity}, ${ASSUMPTION}) gives a demand factor of ${num(dF, 3)}, applied to every OD pair in the network run`
        + `. ${STD.tou.appraisal}. Discounted at ${rates.map((s, k) => `${s.r}%${k ? ` to year ${s.to}` : ` years 1-${s.to}`}`).join(', then ')} over ${yrs} years, benefits flat (NZTA GC 25/01)` };
    if (opts.save !== false) cx.result = res;
    return res;
  }

  /* =====================================================================================
     18. Climate hazard assessment table (climate-overlay; data from the feeds lane)
     ===================================================================================== */
  function climate(P = Store.P, sc = activeSc(P)) {
    let rows = null; const f = P.feeds && P.feeds.hazards && P.feeds.hazards[sc.id];
    if (f && Array.isArray(f.rows)) rows = f.rows;
    try { if (!rows && typeof Feeds !== 'undefined' && Feeds.mem && Feeds.mem.hazards) rows = Feeds.hazards.links(sc); } catch { }
    const out = (rows || []).map(r => { const L = sc.links[r.link]; const vest = L && L.status === 'proposed';
      return { ...r, vest, assess: r.only ? 'Only access: confirm an alternative route in a flood event or raise the road above the design flood level' : vest ? 'Proposed road (asset to vest): design levels for the climate-adjusted event' : 'Relied-on road: note the hazard and the detour' }; });
    return { rows: out, loaded: !!rows, source: 'Auckland Council flood plains and coastal inundation (feeds lane)', note: 'Length is the part of each road centreline inside the hazard area (AT ITA s4.5).' };
  }

  /* =====================================================================================
     19. Hand-over packs (reg-handover): each CSV carries a manifest, the version stamp and D12
     ===================================================================================== */
  const r1 = v => (v == null || !isFinite(v) ? '' : Math.round(v * 10) / 10);
  const r3 = v => (v == null || !isFinite(v) ? '' : Math.round(v * 1000) / 1000);
  function packCSV(P, title, tool, sections) {
    const rows = [[`# ${title}`], ['# Manifest', sections.map(s => s.title).join(' | ')], ['# Project', P.name || ''], ['# Prepared', today()], ['# Version stamp', stamp(P)]];
    for (const s of sections) { rows.push([]); rows.push([`## ${s.title}`]); if (s.note) rows.push([`# ${s.note}`]); rows.push(s.head); for (const r of s.rows) rows.push(r); if (!s.rows.length) rows.push([s.empty || 'None']); }
    return stamped(csv(rows), P, tool);
  }
  const signalNodes = sc => Object.values(sc.nodes).filter(n => n.control === 'signals').map(n => n.id);
  // (2) ATOC TSDG s2.5 signal pack
  async function runDesignLife(P = Store.P, opts = {}) {
    const cx = data(P).tsdg, sc = pair(P).opt, pc = +opts.pct || +cx.pct || 10, out = {};
    for (const p of opts.periods || ['AM', 'PM']) out[p] = await runSc(cloneSc(sc), p, { demandFactor: 1 + pc / 100 });
    const res = { at: new Date().toISOString(), pct: pc, years: +cx.years || 10, scId: sc.id, nodes: Object.fromEntries(Object.entries(out).map(([p, r]) => [p, r.nodes])) };
    if (opts.save !== false) cx.result = res;
    return res;
  }
  function tsdgPack(P = Store.P) {
    const pr = pair(P), sc = pr.opt, ids = signalNodes(sc).length ? signalNodes(sc) : Object.keys((resOf(sc, 'AM') || resOf(sc, 'PM') || { nodes: {} }).nodes).slice(0, 10);
    const counts = ((P.data && P.data.counts) || []).filter(c => ids.some(id => { const n = sc.nodes[id]; return n && U.dist([n.lat, n.lng], [c.lat, c.lng]) <= 150; }));
    const perf = [], stat = '95th percentile back of queue (m)';
    for (const s of [pr.base, pr.opt, pr.mit].filter((x, i, a) => x && a.indexOf(x) === i)) for (const p of periodsOf(s)) for (const id of ids) {
      const nr = (resOf(s, p).nodes || {})[id]; if (!nr) continue;
      for (const a of nr.appr || []) perf.push([s.name, nodeName(s, id), p, a.approach || a.name || '', r1(a.delay), 'average control delay', a.los, r3(a.dos), stat, a.q95]);
    }
    const dl = data(P).tsdg.result, life = [];
    if (dl) for (const [p, nodes] of Object.entries(dl.nodes)) for (const id of ids) { const nr = nodes[id]; if (nr) for (const a of nr.appr || []) life.push([`+${dl.pct}% (design life ${dl.years} years)`, nodeName(sc, id), p, a.approach || a.name || '', r1(a.delay), a.los, r3(a.dos), stat, a.q95]); }
    const timing = ids.map(id => { const n = sc.nodes[id], nr = (resOf(sc, 'AM') || { nodes: {} }).nodes[id] || {}; return [nodeName(sc, id), CONTROLS[n.control] || n.control, n.cycle || nr.C || '', nr.split ? JSON.stringify(nr.split) : 'optimised by Spare Construct (Webster)']; });
    return packCSV(P, 'ATOC Traffic Signals Design Guidelines s2.5 signal pack', 'SIDRA (ATOC TSDG signal design)', [
      { title: 'Counts by class and period', head: ['Site', 'Source', 'Date', 'Daily (veh/day)', 'Peak hour (veh/h)', 'Peak time', 'Heavy (%)', 'Light (veh/day)', 'Heavy (veh/day)'],
        rows: counts.map(c => [c.name, c.src, c.date || '', c.adt, c.peak || '', c.peakTime || '', c.hv ?? '', c.hv != null ? Math.round(c.adt * (1 - c.hv / 100)) : '', c.hv != null ? Math.round(c.adt * c.hv / 100) : '']), empty: 'No counts within 150 m of the signals: load counts or add turning counts.' },
      { title: 'Per-option delay, LoS and queue', note: `${STD.labels.junction}. Queue statistic: ${stat}.`, head: ['Option', 'Junction', 'Period', 'Approach', 'Delay (s)', 'Delay statistic', 'LoS', 'DoS', 'Queue statistic', 'Queue (m)'], rows: perf, empty: 'Run the model.' },
      { title: 'Design-life run', note: dl ? `Demand +${dl.pct}% for a ${dl.years}-year design life` : 'Run the design-life test in Compliance+ › Hand-over packs.', head: ['Run', 'Junction', 'Period', 'Approach', 'Delay (s)', 'LoS', 'DoS', 'Queue statistic', 'Queue (m)'], rows: life },
      { title: 'Phasing and timings', head: ['Junction', 'Control', 'Cycle (s)', 'Phase splits'], rows: timing },
    ]);
  }
  // (3) ANOP tool inputs: speed as % of posted and 85th/median ratio by mode and corridor
  function anopInputsCSV(P = Store.P) {
    const pr = pair(P), rows = [];
    for (const c of anopCorridors(P)) for (const p of periodsOf(pr.opt)) {
      // one row per direction (spec anop: speed per direction)
      const r = resOf(pr.opt, p), g = corridorSpeed(pr.opt, r, c.links, false), b = corridorSpeed(pr.opt, r, c.links, true), rel = c.rel || {};
      const put = (mode, x, rv) => { for (const d of (x && x.dirs) || [null]) rows.push([c.name, d ? d.name : '–', p, mode, r1(d && d.pct), r1(d && d.spd), r1(d && d.limit), x ? x.limitSrc : '', rv || 'N/A']); };
      put('General traffic', g, rel.gt); put('Freight', g, rel.freight); put('Public transport', b, rel.pt);
    }
    return packCSV(P, 'AT ANOP tool inputs', 'the AT ANOP tool', [{ title: 'Average speed and reliability by mode, corridor and direction', note: `${STD.labels.anop} Speed = Σlength / Σ(link time + junction delay) per direction. Reliability is N/A unless observed (TomTom or AT data). Where no NSLR limit is loaded the posted limit falls back to the modelled link speed (${ASSUMPTION}; see the source column).`, head: ['Spare Construct', 'Direction', 'Period', 'Mode', 'Speed (% of posted)', 'Average speed (km/h)', 'Posted limit (km/h, length-weighted)', 'Posted limit source', '85th/median journey time'], rows }]);
  }
  // (4) Safe System Audit briefing pack
  function ssaPack(P = Store.P) {
    const pr = pair(P), sc = pr.mit || pr.opt, ch = changedLinks(pr.base, sc), nodes = new Set([...ch].flatMap(id => [sc.links[id].a, sc.links[id].b]).filter(id => roadLinks(sc, id).length >= 3));
    const r = resOf(sc, 'AM') || resOf(sc, 'PM'); let risk = null; try { risk = DataHub.linkRisk(pr.base); } catch { }
    const peds = (() => { try { const C = C_(); return C ? C.state(P).peds : {}; } catch { return {}; } })();
    const cp = STD.conflicts, conflicts = id => { const n = sc.nodes[id], legs = roadLinks(sc, id).length; return n.control === 'roundabout' ? (legs >= 4 ? cp.roundabout4 : cp.roundabout3) : legs >= 4 ? cp.x4 : cp.x3; };
    return packCSV(P, 'Safe System Audit briefing pack', 'a Safe System Audit', [
      { title: 'Layout changes', note: STD.labels.ssa, head: ['Change'], rows: networkDiff(pr.base, sc).map(d => [d.text]).concat([...ch].filter(id => !pr.base.links[id]).map(id => [`New road ${linkName(sc.links[id])}`])), empty: 'No layout changes against the do-minimum.' },
      { title: 'Volumes by mode and turn', note: 'Vehicles from the model (veh/h); pedestrians from field counts entered in the Compliance tab; cycle and bus volumes are not modelled.', head: ['Junction', 'Period', 'From', 'To', 'Movement', 'Vehicles (veh/h)', 'Pedestrians (people/h)'],
        rows: r ? [...nodes].flatMap(id => (r.turns && r.turns[id] || []).map(t => [nodeName(sc, id), r.period, t.from, t.to, t.mv, r1(t.v), peds[id] ?? ''])) : [], empty: 'Run the model.' },
      { title: 'Operating and legal speeds', note: STD.labels.meanSpeed, head: ['Road', 'Legal limit (km/h)', 'Proposed limit (km/h)', 'Measured mean speed (km/h)', 'Modelled speed (km/h, placeholder)'],
        rows: [...ch].map(id => { const L = sc.links[id], x = r && r.arcs[id + ':ab']; return [linkName(L), posted(L), L.cx && L.cx.limit || '', L.cx && L.cx.meanSpeed || '', x ? r1(x.spd) : '']; }) },
      { title: 'CAS crash history (changed roads)', note: disclaimer('D8', P), head: ['Road', 'Crashes', 'Fatal', 'Serious', 'Minor', 'Non-injury', 'DSI'],
        rows: risk ? [...ch].filter(id => risk[id]).map(id => { const x = risk[id]; return [linkName(sc.links[id]), x.crashes, x.F, x.S, x.M, x.N, x.dsi]; }) : [], empty: risk ? 'No crashes on the changed roads.' : 'Load NZTA CAS crashes.' },
      { title: 'Conflict points', note: `${STD.conflicts.edition} (${ASSUMPTION} counts: crossroads 32, T 9, roundabout 8 or 6)`, head: ['Junction', 'Control', 'Legs', 'Vehicle conflict points'],
        rows: [...nodes].map(id => [nodeName(sc, id), CONTROLS[sc.nodes[id].control] || sc.nodes[id].control, roadLinks(sc, id).length, conflicts(id)]) },
    ]);
  }
  // (5) NZTA state highway impact pack (reg-sh fields)
  function shPack(P = Store.P) {
    const pr = pair(P), rows = [];
    for (const p of periodsOf(pr.opt).filter(x => resOf(pr.base, x))) {
      const rb = resOf(pr.base, p), ro = resOf(pr.opt, p);
      for (const L of Object.values(pr.opt.links)) { if (!isSH(L, P)) continue; for (const d of ['ab', 'ba']) { const k = L.id + ':' + d, a = rb.arcs[k], b = ro.arcs[k]; if (!a && !b) continue; const v0 = a ? a.v : 0, v1 = b ? b.v : 0;
        rows.push([linkName(L), d === 'ab' ? 'A→B' : 'B→A', p, r1(v0), r1(v1), r1(v1 - v0), r1(pct(v1, v0)), r3(b && b.vc), r1(b && b.spd)]); } }
    }
    const nodes = []; for (const p of periodsOf(pr.opt)) for (const [id, nr] of Object.entries(resOf(pr.opt, p).nodes || {})) if (roadLinks(pr.opt, id).some(L => isSH(L, P))) nodes.push([nodeName(pr.opt, id), p, r3(nr.dos), nr.losText || nr.los, r1(nr.delay)]);
    return packCSV(P, 'NZTA state highway impact pack', 'an NZTA land-use or access application', [
      { title: 'State highway volumes, with and without the proposal', head: ['State highway section', 'Direction', 'Peak', 'Without (veh/h)', 'With (veh/h)', 'Change (veh/h)', 'Change (%)', 'v/c with', 'Speed with (km/h)'], rows, empty: 'No state highway in the model, or no results for both scenarios.' },
      { title: 'State highway intersections', note: STD.labels.junction, head: ['Intersection', 'Peak', 'DoS', 'LoS', 'Delay (s)'], rows: nodes },
      { title: 'Application fields to complete', head: ['Field', 'From Spare Construct'], rows: [['Applicant and site address', ''], ['Proposed access location and form', 'Compliance tab › Access'], ['Traffic generated (peak hour)', 'Development class flows (results)'], ['Effects on the state highway', 'Tables above'], ['Crash history', 'CAS (Traffic data tab)']] },
    ]);
  }
  // (6) MBCM transport modelling checks worksheet, SP-workbook-shaped and IER-tool fields
  function mbcmPack(P = Store.P) {
    const pr = pair(P), checks = [], sp = [];
    for (const s of [pr.base, pr.opt].filter((x, i, a) => a.indexOf(x) === i)) for (const p of periodsOf(s)) {
      const r = resOf(s, p); let cal = null; try { cal = Calibrate.report(s, p); } catch { }
      checks.push([s.name, p, 'Convergence (relative gap)', r.gap != null ? r.gap.toExponential(2) : '', '≤ 1e-4 (TAG)', r.converged === false ? 'Not converged' : 'Converged'], [s.name, p, 'Iterations', r.it, `≤ ${r.maxIter || ''}`, r.hitMax ? 'Hit the limit' : 'OK'],
        [s.name, p, 'Noise (95th percentile, veh/h)', r1(r.noise95), 'Benefits must exceed noise', ''], [s.name, p, 'GEH < 5 share', cal && cal.geh5 != null ? `${Math.round(cal.geh5 * 100)}%` : 'n/a', '85% (TMDG)', cal ? cal.status : 'not calibrated'],
        [s.name, p, 'Model status', (() => { try { return Engine.modelStatus(P, p).label; } catch { return ''; } })(), 'Validated for MBCM full procedures', '']);
      sp.push([s.name, p, Engine.yearOf(s), r1(r.kpi.vht), r1(r.kpi.vkt), r3(r.kpi.co2), r1(r.kpi.trips)]);
    }
    const e = econOf(P, pr.base, pr.opt);
    const ier = e && !e.mismatch ? [['Benefits PV (NZ$m)', r3(e.pv)], ['Costs PV (NZ$m)', r3(e.cap)], ['BCR (indicative)', e.bcrText || r1(e.bcr)], ['BCR at 8%', e.bcr8Text || ''], ['IPM efficiency band', (e.ipm || []).map(b => `${b.edition}: ${b.band}${b.draft ? ' (draft)' : ''}`).join('; ')], ['Method', e.label || ''], ['Disclaimer', disclaimer('D4', P)]] : [['Economics', e && e.mismatch ? e.note : 'Run the base and the option for the same year']];
    return packCSV(P, 'MBCM transport modelling checks, SP workbook and IER fields', 'the NZTA IER tool / MBCM workbook', [
      { title: 'Transport modelling checks worksheet', head: ['Scenario', 'Period', 'Check', 'Value', 'Criterion', 'Status'], rows: checks, empty: 'Run the model.' },
      { title: 'SP workbook inputs (per modelled peak hour)', head: ['Scenario', 'Period', 'Year', 'VHT (veh·h)', 'VKT (veh-km)', 'CO2e (t)', 'Trips'], rows: sp },
      { title: 'IER tool fields', head: ['Field', 'Value'], rows: ier },
    ]);
  }
  // (7) RLTP activity sheet (LTMA s16(3)(e))
  function rltpSheet(P = Store.P) {
    const x = data(P).rltp; let region = 'auckland'; try { const C = C_(); if (C) region = C.state(P).ctx.region || 'auckland'; } catch { }
    const auck = region !== 'other';
    return packCSV(P, 'RLTP activity sheet', 'the regional land transport plan', [{ title: 'Activity (LTMA s16(3)(e))', head: ['Field', 'Value'], rows: [
      ['Activity', P.name || ''], ['Submitting organisation', auck ? 'Auckland Council (road controlling authority, LGACA s47)' : 'The approved organisation (road controlling authority)'],
      ['RLTP prepared by', auck ? 'Auckland Regional Transport Committee (ARTC)' : 'The regional transport committee'], ['Objective', x.objective || ''], ['Measures', x.measures || ''],
      ['Cost per year (NZ$)', x.costPerYear || ''], ['Duration', x.duration || ''], ['Funding sources', x.sources || ''], ['Statutory basis', 'Land Transport Management Act 2003 s16(3)(e)']] }]);
  }
  // (8) MSM / AFC request pack
  function msmPack(P = Store.P) {
    const pr = pair(P), sc = pr.opt, keys = Object.keys(LANDUSES), aff = affectedLinks(P);
    return packCSV(P, 'MSM / Auckland Forecasting Centre request pack', 'the Auckland Forecasting Centre (MSM)', [
      { title: 'Zone land use', head: ['Zone', 'Latitude', 'Longitude', 'Class', 'Opening year', 'Build-out (%)', ...keys.map(k => `${LANDUSES[k].label} (${LANDUSES[k].unit})`)],
        rows: Object.values(sc.zones).filter(z => z.kind !== 'gateway').map(z => [z.name || z.id, z.lat, z.lng, Engine.zoneClass(z), z.year || '', Math.round(Engine.stageOf(z, sc) * 100), ...keys.map(k => luOf(z, k) || '')]) },
      { title: 'Scenario definition', head: ['Scenario', 'Definition'], rows: P.scenarios.map(s => { let t = ''; try { t = Scenarios.describe(s, P).text; } catch { t = s.name; } return [s.name, t]; }) },
      { title: 'Links of interest', note: aff.basis, head: ['Road', 'Class', 'From (lat, lng)', 'To (lat, lng)'], rows: [...aff.ids].filter(id => sc.links[id]).map(id => { const L = sc.links[id], a = sc.nodes[L.a], b = sc.nodes[L.b]; return [linkName(L), L.cls, `${a.lat}, ${a.lng}`, `${b.lat}, ${b.lng}`]; }) },
    ]);
  }
  function handoverManifest(P = Store.P) {
    let sidra = null; try { const sc = pair(P).opt, r = [resOf(sc, 'AM'), resOf(sc, 'PM')].filter(Boolean); sidra = r.length ? Turns.sidraExport(sc, r, null, {}) : { ok: false, why: 'Run the model' }; } catch (e) { sidra = { ok: false, why: e.message }; }
    return [
      { id: 'sidra', pack: 'SIDRA volumes', tool: 'SIDRA', exportId: 'junctions-sidra-volumes', ready: !!(sidra && sidra.ok), why: sidra && !sidra.ok ? sidra.why : '' },
      { id: 'tsdg', pack: 'ATOC TSDG s2.5 signal pack', tool: 'SIDRA (signal design)', exportId: 'compliance-extra-tsdg', ready: periodsOf(pair(P).opt).length > 0 },
      { id: 'anop', pack: 'ANOP tool inputs (anop.csv and speeds)', tool: 'AT ANOP tool v10.2', exportId: 'compliance-extra-anop', ready: anop(P).rows.length > 0 },
      { id: 'ssa', pack: 'Safe System Audit briefing pack', tool: 'Safe System Audit', exportId: 'compliance-extra-ssa', ready: periodsOf(pair(P).opt).length > 0 },
      { id: 'sh', pack: 'NZTA state highway impact pack', tool: 'NZTA access application', exportId: 'compliance-extra-sh', ready: Object.values(pair(P).opt.links).some(L => isSH(L, P)) },
      { id: 'mbcm', pack: 'MBCM checks, SP workbook and IER fields', tool: 'NZTA IER tool / MBCM workbook', exportId: 'compliance-extra-mbcm', ready: periodsOf(pair(P).opt).length > 0 },
      { id: 'rltp', pack: 'RLTP activity sheet', tool: 'RLTP', exportId: 'compliance-extra-rltp', ready: !!(data(P).rltp.objective) },
      { id: 'msm', pack: 'MSM / AFC request pack', tool: 'MSM (Auckland Forecasting Centre)', exportId: 'compliance-extra-msm', ready: true },
    ];
  }

  /* =====================================================================================
     20. CSV exports for the registers and tables
     ===================================================================================== */
  function lintCSV(P = Store.P) { const L = lint(P); return stamped(csv([['Rule', 'Level', 'Where', 'Text', 'Source', 'Suggested change', 'Author override'], ...L.findings.map(f => [f.rule, f.level === 'block' ? (f.overridden ? 'Blocks export (overridden)' : 'Blocks export') : 'Check', f.where, f.sentence, f.source, f.suggest, f.overridden ? `${f.overridden.reason} (${f.overridden.at})` : ''])]), P); }
  function precinctCSV(P = Store.P) {
    const c = precinctCheck(P); if (!c) return stamped(csv([['No precinct selected']]), P);
    const rows = [[`${c.precinct.id} ${c.precinct.name}: ${c.precinct.table}`], [c.banner], [c.pc120Note], [], ['Row', 'Dwellings cap', 'Retail GFA cap (m²)', 'Other commercial GFA cap (m²)', 'Community GFA cap (m²)', 'Upgrade', 'Status', 'Deemed met', 'Activity status on breach'],
      ...c.rows.flatMap(r => r.up.map(u => [r.row, r.dw ?? '', r.retail ?? '', r.commercial ?? '', r.community ?? '', u.name, u.stateLabel, u.met ? 'yes' : 'no', u.status])), [],
      ['Register: consent ref', 'Dwellings', 'Retail GFA (m²)', 'Other commercial GFA (m²)', 'Community GFA (m²)', 'Date', 'Enabled yield'], ...c.register.map(r => [r.ref, r.dwellings, r.retail, r.commercial, r.community, r.date, r.enabled || '']),
      ['Proposal', c.proposal.dw, c.proposal.retail, c.proposal.commercial, c.proposal.community], ['Total', c.total.dw, c.total.retail, c.total.commercial, c.total.community], [],
      ['Applicable row', c.row.row], ['Result', c.breach ? `Breach: ${c.why.join('; ')}` : 'Within the caps'], ['Activity status', c.statusText],
      ['Inter-peak LoS test', `${c.ip.name}: ${c.ip.los || 'not available'} (${c.ip.source}) ${c.ip.pass == null ? '' : c.ip.pass ? 'passes' : 'fails'} ${c.ip.rule}`],
      ...(c.trips.rows || []).map(t => [`Trip equivalence ${t.period}`, `${r1(t.proposed)} veh/h proposed against ${r1(t.table)} veh/h table mix (ratio ${r3(t.ratio)})`])];
    return stamped(csv(rows), P);
  }
  const mval = x => (x ? `DoS ${r3(x.dos)}, delay ${r1(x.delay)} s, LoS ${x.los || ''}` : '');
  function mitigationCSV(P = Store.P) {
    const m = mitigationTable(P), c = conditionTable(P), t = data(P).triggers;
    const rows = [['Table M1 · Mitigation register'], ['Ref', 'Measure', 'Effect addressed', 'Options considered', 'Non-infrastructure option', 'Trigger', 'Responsible', 'Cost (NZ$m)', 'Funder', 'RLTP/LTP status', 'Mechanism', 'Committed by applicant', 'Land outside applicant control', 'Tag'],
      ...m.map(r => [r.ref, r.measure, r.effectText, r.options, r.nonInfra ? 'Y' : 'N', r.trigger, r.responsibleText, r.cost ?? '', RESPONSIBLE[r.funder] || r.funder || '', r.rltp, r.mechanismText, committedText(r.committed), r.landOutside ? 'Y' : 'N', [r.tag, r.s84a].filter(Boolean).join('; ')]), [],
      ['Condition parameters (Spare Construct never writes condition wording)'], ['Ref', 'Measure', 'Trigger metric', 'Threshold', 'Survey method', 'Frequency', 'Response', 'Nexus or agreement', 'Review clause suggestion', 'Label'],
      ...c.map(r => [r.ref, r.measure, r.metric, r.threshold, r.survey, r.frequency, r.response, r.basis, r.review, r.ftaa])];
    if (t) {
      rows.push([], ['Table M2 · Trigger finder', t.basis, t.label], ['Variant', 'First failing build-out (%)', 'Dwellings at trigger', 'Before dwelling', 'Site', 'Period', 'Why', 'Resolved by the upgrade'],
        ...t.rows.map(r => [r.label, r.first ? r.first.pct : 'none', r.first ? r.first.dwellings : '', r.first ? r.first.before : '', r.first ? r.first.name : '', r.first ? r.first.period : '', r.first ? r.first.why : r.text, r.resolved ? r.resolved.text : '']));
      const ex = t.rows.flatMap(r => (r.existing || []).map(e => [r.label, e.name, e.period, mval(e.at0), mval(e.at100), e.worsenedAt != null ? `${e.worsenedAt}%` : 'not worsened beyond the threshold', e.why]));
      rows.push([], ['Existing deficiencies (fail at 0% build-out)'], ['Variant', 'Junction', 'Period', 'At 0%', 'At 100%', 'Worsened by the development from', 'Why it fails at 0%'], ...(ex.length ? ex : [['None']]));
    }
    return stamped(csv(rows), P);
  }
  function optionsCSV(P = Store.P) {
    const a = alternatives(P), pv = (r, k, p, f) => (r.pp && r.pp[k] && r.pp[k].by[p] != null ? f(r.pp[k].by[p]) : '');
    return stamped(csv([['Appraisal summary (EAST/AST-ready)', `Do-minimum: ${a.base.name}`, 'Values are per modelled peak hour, each period on its own'], ['Option', 'Intervention hierarchy', 'Scenario', 'ΔVHT AM (veh·h/h)', 'ΔVHT PM (veh·h/h)', `Worst junction (${STD.labels.junction})`, 'Bus route time change AM (min)', 'Bus route time change PM (min)', 'New cycle facility (km)', 'CAS crashes on changed roads', 'CAS DSI', 'Predicted DSI change', 'ΔVKT AM (veh-km/h, information)', 'ΔVKT PM (veh-km/h, information)', 'ΔCO2e AM (t/h, information)', 'ΔCO2e PM (t/h, information)', 'Indicative BCR', 'IPM band', 'Cost (NZ$m)', 'Notes'],
      ...a.rows.map(r => [r.name, r.label, r.scName, pv(r, 'vht', 'AM', r1), pv(r, 'vht', 'PM', r1), r.worst || '', pv(r, 'route', 'AM', x => r1(x / 60)), pv(r, 'route', 'PM', x => r1(x / 60)), r.cycleKm != null ? r3(r.cycleKm) : '', r.cas ? r.cas.crashes : '', r.cas ? r.cas.dsi : '', r.dsi ?? 'not modelled',
        pv(r, 'vkt', 'AM', r1), pv(r, 'vkt', 'PM', r1), pv(r, 'co2', 'AM', r3), pv(r, 'co2', 'PM', r3), r.bcr || '', r.ipm || '', r.cost != null ? r3(r.cost) : '', r.notes]),
      [], ...a.warnings.map(w => ['Warning', w]), ['Label', STD.labels.vkt], ['Junction results', disclaimer('D3', P)]]), P);
  }
  // The Fast-track package as files (reg-ftaa); ftaaBundle is the JSON form, ftaaZip the ZIP the export downloads
  function ftaaFiles(P = Store.P) {
    const f = ftaaPackage(P);
    return { f, files: {
      'compliance-matrix.csv': csv([['Rule', 'Requirement', 'Comply / infringe', 'Evidence'], ...f.matrix.map(r => [r.rule, r.req, r.result, r.evidence])]),
      'aee-outline.csv': csv([['Section', 'Reference'], ...f.aee.map(s => [s.title, s.ref])]),
      'conditions.csv': csv([['Ref', 'Measure', 'Metric', 'Threshold', 'Survey', 'Frequency', 'Response', 'Nexus', 'Label'], ...f.conditions.map(r => [r.ref, r.measure, r.metric, r.threshold, r.survey, r.frequency, r.response, r.basis, STD.labels.s84a])]),
      's85-panel.csv': csv([['Side', 'Item', 'Value'], ...f.s85.benefits.map(([k, v]) => ['Benefit', k, v]), ...f.s85.impacts.map(([k, v]) => ['Residual impact', k, v]), ['Label', 'Junction results', STD.labels.junction]]),
      's84a-staging.csv': csv([['Stage', 'Trigger', 'Measure'], ...f.s84a.map(r => [r.stage, r.trigger, r.measure])]),
      'invitees.csv': csv([['Invitee', 'Basis'], ...f.invitees.map(i => [i.who, i.basis])]),
      'timeline.csv': csv([['Step', 'Date', 'Rule', 'Basis'], ...f.timeline.map(t => [t.step, t.date || '', t.rule, t.basis || ''])]),
      'notes.txt': f.notes.join('\n'),
      'reviewer/assumptions.csv': csv([['Assumption', 'Value', 'Unit', 'Basis'], ...f.reviewer.assumptions]),
      'reviewer/sources.csv': csv([['Data', 'Loaded', 'Records', 'Licence or source'], ...f.reviewer.sources]),
      // Engine.sensitivity gives dVht as a fraction of the central run's VHT: printed as a percentage
      'reviewer/sensitivity.csv': f.reviewer.sensitivity ? csv([['Test', 'Input', 'Trips', 'VHT (veh·h)', 'VHT change (% of the central run)', 'Sentence'], ...f.reviewer.sensitivity.rows.map(r => [r.label || r.test || r.id, r.input || '', r1(r.trips), r1(r.vht), r.dVht != null && isFinite(r.dVht) ? `${(r.dVht * 100).toFixed(1)}%` : '', r.sentence || ''])]) : 'Sensitivity tests have not been run: run the sensitivity pack (Report tab) before lodging.',
      'reviewer/model-inputs.json': Store.serialise(P),
    } };
  }
  function ftaaBundle(P = Store.P) {
    const { files } = ftaaFiles(P);
    return JSON.stringify({ format: 'corridor-ftaa-package', version: 1, prepared: today(), project: P.name || '', stamp: stamp(P), disclaimers: { D1: disclaimer('D1', P), D3: disclaimer('D3', P), D12: disclaimer('D12', P, 'a Fast-track application') }, manifest: Object.keys(files), files }, null, 1);
  }
  const hasZip = () => typeof Report !== 'undefined' && Report && typeof Report.zip === 'function';
  // A real ZIP (the report lane's Report.zip); the JSON bundle when the report module is not loaded
  function ftaaZip(P = Store.P) {
    if (!hasZip()) return ftaaBundle(P);
    const { files } = ftaaFiles(P), list = Object.keys(files);
    const readme = [`Spare Construct Fast-track package (${P.name || 'project'}), prepared ${today()}.`, `Version stamp: ${stamp(P)}`, '', `D1. ${disclaimer('D1', P)}`, `D3. ${disclaimer('D3', P)}`, `D12. ${disclaimer('D12', P, 'a Fast-track application')}`, '', 'Manifest:', ...list.map(n => `  ${n}`)].join('\r\n');
    return Report.zip([{ name: 'README.txt', data: readme }, ...list.map(n => ({ name: n, data: files[n] }))]);
  }
  function planChangeCSV(P = Store.P) {
    const pc = planChange(P);
    return stamped(csv([['Plan change process'], ...pc.process.map(p => [p.level === 'warn' ? 'Warning' : 'Note', p.text]), [],
      ['s32 transport table', pc.s32Label], ['Option', 'Hierarchy', 'Travel time', 'Safety', 'Vehicle operating cost', 'Infrastructure cost and timing', 'Risk of acting', 'Risk of not acting'], ...pc.s32.map(r => [r.name, r.label, r.travel, r.safety, r.voc, r.cost, r.acting, r.notActing]), [],
      ['Policy rows'], ['Reference', 'Policy', 'Spare Construct evidence'], ...pc.policy.map(r => [r.ref, r.policy, r.text]), [],
      ['Draft road function table'], ['Road', 'Class', 'Draft function', 'RASF', 'Future Connect (general traffic)', 'Limit (km/h)', 'Lanes', 'Cycling', 'Status'], ...pc.roads.map(r => [r.name, r.cls, r.fn, r.rasf, r.fc, r.speed, r.lanes, r.cycle, r.status]), [],
      ['Full build-out', pc.buildout.label, `${pc.buildout.dwellings} dwellings`, `${pc.buildout.gfa} m² GFA`, pc.buildout.rule], [], ['Sch 1 cl 23 response pack'], ['Item', 'Status'], ...pc.cl23]), P);
  }
  function speedCSV(P = Store.P) {
    const rows = [['Speed limit validation (Setting of Speed Limits Rule 2024)'], ['Road', 'Rule class', 'Current limit', 'Proposed limit', 'Section length (m, a divided road counted once)', 'Schedule 1 minimum (m)', 'Exception', 'Decision-maker', 'Flags', 'ONF suggestion'],
      ...speedCheck(P).map(r => [r.name, r.classLabel, r.current, r.limit, Math.round(r.len), r.minLen, r.exception, r.decision, r.flags.map(f => `${f.text} (${f.rule})`).join(' | '), r.onf])];
    const cb = data(P).speed.cbds;
    if (cb && cb.ok) { rows.push([], [`Cost benefit disclosure statement: ${cb.name}`, STD.labels.cbds]); for (const h of cb.headings) { rows.push([h.title, h.rule]); for (const r of h.rows) rows.push(['', ...r]); } rows.push(['Decision-maker', cb.decision], ...cb.warn.map(w => ['Warning', w]), ['Watermark', STD.labels.cbds], ['D9', disclaimer('D9', P)]); }
    return stamped(csv(rows), P);
  }
  function parkingCSV(P = Store.P) {
    const pk = parking(P);
    return stamped(csv([['Table P1 · Parking, cycle parking, end-of-trip and loading', pk.label], ['Activity', 'Quantity', 'Unit', 'AUP maximum car parking', 'Proposed car parking', 'Accessible required (indicative number, verify)', 'Accessible proposed', 'Cycle visitor required (unrounded)', 'Cycle visitor proposed', 'Cycle secure required (unrounded)', 'Cycle secure proposed', 'Loading required', 'Loading proposed', 'Complies', 'Notes'],
      ...pk.rows.map(r => [r.label, r.qty, r.unit, r.max ?? 'enter', r.cars, r.accReq.required ? `Yes (${r.accReq.n})` : 'No', r.accessible, r.cyc.visitor != null ? r1(r.cyc.visitor) : 'enter', r.cycleVisitor, r.cyc.secure != null ? r1(r.cyc.secure) : 'enter', r.cycleSecure, r.loadReq, r.loading, r.complies ? 'Yes' : 'No', [...r.why, r.induced ? r.induced.text : ''].filter(Boolean).join('; ')]),
      ['Total (summed, then rounded 0.5 up: E27.6.2(9))', '', '', '', '', '', '', pk.need.visitor, pk.have.visitor, pk.need.secure, pk.have.secure, pk.need.loading, pk.have.loading, pk.allComply ? 'Yes' : 'No', [`Showers: ${pk.need.showers} required, ${pk.have.showers} proposed`, ...pk.totalsWhy].join('; ')]]), P);
  }
  function policyCSV(P = Store.P) {
    const p = policy(P), g = gpsMetrics(P), f = funding(P);
    return stamped(csv([['Policy alignment', p.basis], ['Road', 'FC general traffic', 'FC public transport', 'FC freight', 'FC cycling', 'RASF', 'ONF', 'Inconsistencies'], ...p.rows.map(r => [r.name, ...r.fc, r.rasf, r.onf, r.flags.join('; ')]), [],
      ['Reference', 'Policy', 'Spare Construct evidence'], ...p.policy.map(r => [r.ref, r.policy, r.text]), [],
      ['IPM stage 2 GPS metrics', g.label], ['Route', 'Significance', 'Base (s)', 'Option (s)', 'Saving (%)'], ...g.routes.map(r => [r.name, r.significance, r1(r.base), r1(r.opt), r1(r.saving)]),
      ['Scheme network', g.network.significance, '', '', r1(g.network.saving)], ['HCV rating', g.network.hcv || ''], ['General traffic rating', g.network.gt || ''], ['Note', g.network.note],
      ['Destination', 'Significance', 'Minutes saved'], ...g.destinations.map(d => [d.name, d.significance, r3(d.minutes)]),
      ['Houses enabled (75% complete by 2035)', g.houses.total, g.houses.basis], ['DSI reduction per NZ$100m', g.dsi.per100m != null ? r1(g.dsi.per100m) : 'enter the DSI reduction'], [],
      ['Funding attribution', f.caveat, f.assetsBasis], ['Asset', 'Period', 'Existing (veh/h)', 'Committed (veh/h)', 'Development (veh/h)', 'Development share'], ...f.assets.map(a => [a.name, a.period, r1(a.existing), r1(a.committed), r1(a.development), r3(a.devShare)]),
      ...(f.shares ? [[], ['Select-link shares by development area', f.shares.method], ['Asset', 'Period', 'Total (veh/h)', 'Development area', 'Flow (veh/h)', 'Share'], ...f.shares.assets.flatMap(a => a.areas.map(z => [a.name, a.period, r1(a.total), z.name, r1(z.v), r3(z.share)]))] : []),
      ['Growth share', f.growth.share != null ? r3(f.growth.share) : '', f.growth.formula, f.growth.label], ['Benefit inside the levy area', f.incidence && f.incidence.insideShare != null ? `${Math.round(f.incidence.insideShare * 100)}%` : 'draw the levy area'], ['BCR_G (levy as Cp)', f.bcrG.text], ...(f.dc ? [['DC policy window', f.dc.text]] : [])]), P);
  }
  function touCSV(P = Store.P) {
    const r = data(P).tou.result; if (!r || !r.ok) return stamped(csv([['Time-of-use screening has not been run.']]), P);
    const A_ = r.annual;
    return stamped(csv([['Time-of-use screening', r.label], ['Method', r.method], [], ['Sampled trips (s65D(3))'], ['From', 'To', 'Trips before (veh/h)', 'Trips after (veh/h)', 'Travel time before (min)', 'Travel time after (min)', 'Charge paid (NZ$)', 'Charged'],
      ...r.sample.map(o => [o.from, o.to, r1(o.q0), r1(o.q1), r1(o.t0 / 60), r1(o.t1 / 60), o.pay != null ? r3(o.pay) : '', o.paid ? 'yes' : 'no']), [],
      ['Network impacts (real travel times)', 'VKT before', 'VKT after', 'VHT before', 'VHT after'], ['State highways', r1(r.impacts.sh[0].vkt), r1(r.impacts.sh[1].vkt), r1(r.impacts.sh[0].vht), r1(r.impacts.sh[1].vht)], ['Local roads', r1(r.impacts.local[0].vkt), r1(r.impacts.local[1].vkt), r1(r.impacts.local[0].vht), r1(r.impacts.local[1].vht)], ['Bus routes (total time, s)', r1(r.impacts.pt[0]), r1(r.impacts.pt[1])], [],
      ['Incidence by zone'], ['Zone', 'Trips (veh/h)', 'Charged share', 'Average travel time change (s)', 'Average generalised cost change (s, charge as time)', 'Income proxy'], ...r.incidence.map(z => [z.name, r1(z.trips), r3(z.paidShare), r1(z.dtAvg), r1(z.dGcAvg), z.income ?? '']), [],
      ['Monetised summary (s65ZD), NZ$m a year', 'User benefit (rule of half, charge included)', 'of which time', 'of which charges paid', 'Revenue (transfer)', 'Deadweight loss', 'VOC', 'CO2', 'Safety', 'National total'],
      ['', r3(A_.user), r3(A_.time), r3(A_.charges), r3(A_.revenue), r3(A_.deadweight), r3(A_.voc), r3(A_.co2), r3(A_.safety), r3(A_.total)], ['Appraisal', r.appraisal],
      ['BCR_N', r.bcrN != null ? r1(r.bcrN) : 'enter the scheme cost'], ['BCR_G (revenue as Cp)', r.bcrG != null ? r1(r.bcrG) : r.bcrGText || 'enter the scheme cost']]), P);
  }
  function climateCSV(P = Store.P) { const c = climate(P); return stamped(csv([['Climate adaptation (AT ITA s4.5)', c.note], ['Link', 'Future Connect role', 'Length in hazard (m)', 'Hazard layer', 'Only access', 'Assessment'], ...c.rows.map(r => [r.name, r.fc, r.len, r.layer, r.only ? 'Y' : 'N', r.assess])]), P); }

  /* =====================================================================================
     21. The Compliance+ tab (render functions return HTML strings; handlers below)
     ===================================================================================== */
  const ui = { sec: 'overview', cbdsIds: '' };
  const SECTIONS = [
    ['overview', 'Overview'], ['lint', 'Scope linter'], ['precinct', 'Precinct staging'], ['mitigation', 'Mitigation and conditions'], ['options', 'Options and alternatives'],
    ['ftaa', 'Fast-track package'], ['planchange', 'Plan change'], ['speed', 'Speed limits and CBDS'], ['anop', 'ANOP'], ['parking', 'Parking'], ['subdivision', 'Subdivision'],
    ['policy', 'Policy, GPS and funding'], ['tou', 'Time-of-use'], ['climate', 'Climate'], ['handover', 'Hand-over packs'],
  ];
  const inp = (path, v, t = 'str', extra = '') => `<input ${t === 'num' || t === 'numnull' ? 'type="number" step="any"' : t === 'date' ? 'type="date"' : 'type="text"'} data-cx="${esc(path)}" data-cxt="${t}" value="${esc(v == null ? '' : v)}" ${extra}>`;
  const sel = (path, opts, v) => `<select data-cx="${esc(path)}" data-cxt="str">${Kit.opt(opts, v == null ? '' : String(v))}</select>`;
  const chk = (path, v, label) => `<label class="check"><input type="checkbox" data-cx="${esc(path)}" data-cxt="bool" ${v ? 'checked' : ''}> ${label}</label>`;
  const fld = (label, inner, unit = '') => `<label class="fld"><span>${label}</span><span class="fld-in">${inner}${unit ? `<span class="unit">${unit}</span>` : ''}</span></label>`;
  const btn = (act, label, arg = '', cls = 'btn small') => `<button class="${cls}" data-cx-do="${esc(act)}" data-arg="${esc(arg)}">${label}</button>`;
  const h = t => `<h4>${t}</h4>`;
  const resOK = P => periodsOf(pair(P).opt).length > 0;
  function ctxLine(P) {
    const c = context(P), pr = pair(P), stale = [pr.base, pr.opt].some(s => periodsOf(s).some(p => s.res[p].stale));
    return `${stale ? note('Results are out of date: run the model again before relying on these tables.', true) : ''}<p class="note">Pathway <b>${esc(c.label)}</b> · lodgement ${c.lodgement ? esc(longDate(c.lodgement)) : 'not set'} · ${esc(c.regimeText)} · set in the ${esc(c.source === 'Compliance tab' ? 'Compliance tab' : 'fields below')}. Do-minimum <b>${esc(pair(P).base.name)}</b>, option <b>${esc(pair(P).opt.name)}</b>.</p>`;
  }
  function secOverview(P) {
    const c = context(P), L = lint(P), pc = precinctCheck(P), m = mitigationTable(P), cd = conditionTable(P), al = alternatives(P), sp = speedCheck(P), pk = parking(P);
    const row = (sec, what, state, text) => ({ cells: [`<a href="#" data-cx-sec="${sec}">${what}</a>`, state === 'ok' ? pill('OK', 'ok') : state === 'warn' ? pill('Check', 'warn') : state === 'block' ? pill('Blocks export', 'bad') : pill('–'), text] });
    const own = c.source !== 'Compliance tab' ? `<div class="set-grid">${fld('Pathway', sel('ctx.pathway', PATHS, data(P).ctx.pathway))}${fld('Lodgement date', inp('ctx.lodgement', data(P).ctx.lodgement, 'date'))}${fld('Notification date', inp('ctx.notificationDate', data(P).ctx.notificationDate, 'date'))}${fld('Land interest (NoR)', sel('ctx.landInterest', { '': 'Not recorded', yes: 'Sufficient', no: 'Not sufficient' }, data(P).ctx.landInterest))}</div>` : '';
    return `${ctxLine(P)}${own}<div class="set-grid">${chk('ctx.sigEffect', data(P).ctx.sigEffect, 'Likely significant adverse effect (author\'s judgment: shows the alternatives sections)')}${chk('ctx.mbcmFull', data(P).ctx.mbcmFull, 'MBCM full procedures applied (for the "MBCM-compliant" check)')}${fld('Report date (as at)', inp('ctx.reportDate', data(P).ctx.reportDate, 'date'))}</div>` +
      T({ head: ['Module', 'State', 'Summary'], rows: [
        row('lint', 'Scope and wording', L.blocking.length ? 'block' : L.findings.length ? 'warn' : 'ok', L.findings.length ? `${L.findings.length} finding${L.findings.length === 1 ? '' : 's'}` : 'No findings'),
        row('precinct', 'Precinct staging', pc ? (pc.breach ? 'warn' : 'ok') : '', pc ? `${esc(pc.precinct.id)}: ${pc.breach ? esc(pc.statusText) : 'within the caps'}` : 'No precinct selected'),
        row('mitigation', 'Mitigation register', m.some(x => !x.effectFound && x.effect) || cd.some(x => !x.ok) ? 'warn' : m.length ? 'ok' : '', `${m.length} measure${m.length === 1 ? '' : 's'}, ${cd.length} condition parameter row${cd.length === 1 ? '' : 's'}`),
        row('options', 'Options and alternatives', al.warnings.length ? 'warn' : al.rows.length ? 'ok' : '', `${al.rows.length} option${al.rows.length === 1 ? '' : 's'}${al.warnings.length ? `; ${esc(al.warnings[0])}` : ''}`),
        row('speed', 'Speed limits', sp.some(r => r.flags.length) ? 'warn' : sp.length ? 'ok' : '', sp.length ? `${sp.filter(r => r.flags.length).length} of ${sp.length} sections flagged` : 'No proposed limits'),
        row('parking', 'Parking (Table P1)', pk.rows.length ? (pk.allComply ? 'ok' : 'warn') : '', pk.rows.length ? (pk.allComply ? 'Complies' : 'Infringes or incomplete') : 'No activities entered'),
      ] });
  }
  function secLint(P) {
    const L = lint(P);
    const act = f => f.level === 'block' ? (f.overridden ? `${pill('Overridden', 'warn')} <span class="note">${esc(f.overridden.reason)} (${esc(f.overridden.at)})</span> ${btn('lint-override-clear', 'Clear', f.key)}` : btn('lint-override', 'Override (record why)', f.key)) : btn('lint-ignore', 'Dismiss', f.key);
    return `${h('Statutory scope and wording linter')}<p class="note">Checks this text, the report's author text and the registers' free text against the rules for the pathway and lodgement date. A blocking finding stops the outputs that carry author text (the ITA report and data pack, the Fast-track package and the s32 pack) until it is fixed, or until the author records why the sentence is correct. Data exports are not blocked.</p>
      <textarea data-cx="lint.text" data-cxt="str" rows="5" style="width:100%" placeholder="Paste text to check (for example a draft AEE section)">${esc(data(P).lint.text)}</textarea>
      ${T({ head: ['Rule', 'Where', 'Text', 'Suggested change', ''], rows: L.findings.map(f => ({ cells: [`${f.level === 'block' ? pill('Blocks export', 'bad') : pill('Check', 'warn')} ${esc(f.rule)}<br><span class="note">${esc(f.source)}</span>`, esc(f.where), `…${esc(f.sentence)}… <b>(${esc(f.match)})</b>`, esc(f.suggest), act(f)] })), empty: 'No findings for this pathway and date.' })}
      <p class="note">Rules applied: ${L.applies.length} of ${LINT.length} (the others do not apply to ${esc(L.context.label)}${L.context.lodgement ? '' : ' without a lodgement date'}).</p>`;
  }
  function secPrecinct(P) {
    const cx = data(P).precinct, list = precinctList(P), c = precinctCheck(P);
    const head = `${h('Precinct staging tracker')}<div class="set-grid">${fld('Precinct', sel('precinct.id', { '': 'Choose (or set the site in the Compliance tab)', ...Object.fromEntries(Object.values(list).map(p => [p.id, `${p.id} ${p.name}`])) }, cx.id))}
      ${chk('precinct.augier', cx.augier, 'The applicant agrees under s108AA(1)(a), on an Augier basis, to conditions preventing occupation until upgrades operate')}${chk('precinct.amendment', cx.amendment, 'This application amends an existing consent')}</div>`;
    if (!c) return head + note('Choose a precinct to track its staging table.');
    const pr = c.precinct, U_ = cx.upgrades || {};
    const caps = T({ head: ['Row', { label: 'Dwellings', n: true }, { label: 'Retail m²', n: true }, { label: 'Other commercial m²', n: true }, { label: 'Community m²', n: true }, 'Prerequisite upgrades', 'If breached'],
      rows: c.rows.map(r => ({ cls: r === c.row ? 'sel' : '', cells: [`(${esc(r.row)})`, num(r.dw), r.retail == null ? '–' : num(r.retail), r.commercial == null ? '–' : num(r.commercial), r.community == null ? '–' : num(r.community),
        r.up.map(u => `${esc(u.name)} ${sel(`precinct.upgrades.${u.id}`, UPGRADE_STATUS, U_[u.id] || 'not-started')}${u.needsAugier ? ' ' + pill('needs Augier agreement', 'warn') : u.met ? ' ' + pill('met', 'ok') : ''}`).join('<br>'), esc([...new Set(r.up.map(u => u.status))].join(' / '))] })) });
    const reg = T({ head: ['Consent ref', { label: 'Dwellings', n: true }, { label: 'Retail m²', n: true }, { label: 'Other commercial m²', n: true }, { label: 'Community m²', n: true }, 'Date', 'Enabled yield', ''],
      rows: cx.register.map((r, i) => [inp(`precinct.register.${i}.ref`, r.ref), inp(`precinct.register.${i}.dwellings`, r.dwellings, 'num'), inp(`precinct.register.${i}.retail`, r.retail, 'num'), inp(`precinct.register.${i}.commercial`, r.commercial, 'num'), inp(`precinct.register.${i}.community`, r.community, 'num'), inp(`precinct.register.${i}.date`, r.date, 'date'), inp(`precinct.register.${i}.enabled`, r.enabled), btn('del', '✕', `precinct.register.${i}`)]),
      empty: 'No approved development registered yet.' });
    const p = cx.proposal;
    return `${head}${c.banner ? note(`<b>${esc(c.banner)}</b> (${link(SRC.pc120, 'AUP modifications 2026')}). ${esc(c.pc120Note)}.`, true) : ''}${note(`${esc(pr.table)} (${esc(pr.edition)})${verify(pr)}. Each row's cap applies until its upgrades and every earlier row's are operational (${esc(pr.rule)}).`)}
      ${caps}${h('Register of approved development')}${reg}${btn('add-reg', 'Add a consent')}
      ${h('This application')}<div class="set-grid">${fld('Reference', inp('precinct.proposal.ref', p.ref))}${fld('Dwellings', inp('precinct.proposal.dwellings', p.dwellings, 'num'))}${fld('Retail GFA', inp('precinct.proposal.retail', p.retail, 'num'), 'm²')}${fld('Other commercial GFA', inp('precinct.proposal.commercial', p.commercial, 'num'), 'm²')}${fld('Community GFA', inp('precinct.proposal.community', p.community, 'num'), 'm²')}
      ${fld('Inter-peak LoS (from SIDRA)', sel('precinct.ipLos', { '': 'Not entered', A: 'A', B: 'B', C: 'C', D: 'D', E: 'E', F: 'F' }, cx.ipLos))}</div>
      ${note(`<b>${c.breach ? `Breach of row (${esc(c.row.row)}): ${esc(c.why.join('; '))}. ${esc(c.statusText)}.` : `Within the row (${esc(c.row.row)}) caps: headroom ${num(c.headroom.dw)} dwellings.`}</b>`, c.breach)}
      ${T({ head: ['Test', 'Result'], rows: [[`Inter-peak LoS at ${esc(c.ip.name)} (${esc(c.ip.rule)})`, c.ip.los ? `${esc(c.ip.los)} (${esc(c.ip.source)}): ${c.ip.pass ? pill('passes', 'ok') : pill('fails', 'bad')} ${esc(c.ip.test)}` : 'No inter-peak result: enter the SIDRA inter-peak LoS above'],
        ...(c.trips.rows || []).map(t => [`Trip equivalence, ${esc(t.period)} peak`, `${num(t.proposed)} veh/h proposed mix against ${num(t.table)} veh/h table mix (ratio ${num(t.ratio, 2)}) ${t.ratio <= STD.precinctTest.similar ? pill('similar or less', 'ok') : pill('higher', 'warn')}`])] })}
      ${note(`${esc(c.trips.basis || '')}. ${esc(c.trips.mapping || '')}. ${esc(c.monitorText)}`)}${c.warnings.map(w => note(esc(w), true)).join('')}
      <details><summary>Add another precinct as data (JSON)</summary><textarea data-cx-json="precinct.custom" rows="4" style="width:100%">${esc(JSON.stringify(cx.custom || [], null, 1))}</textarea><p class="note">A list of { id, name, table, rule, ita, monitor, node: [road names], rows: [{ row, dw, retail, commercial, community, up: [{ id, name, status: 'A5' | 'A6' }] }] }.</p></details>`;
  }
  const mvalH = x => (x ? `DoS ${num(x.dos, 2)}, ${num(x.delay, 0)} s, LoS ${esc(x.los || '–')}` : '–');
  function triggerTables(t) {
    const main = T({ head: ['Variant', 'First failing build-out', { label: 'Dwellings', n: true }, 'Site and reason', 'Resolved by the upgrade'], rows: t.rows.map(r => [esc(r.label), r.first ? `${r.first.pct}%` : 'None', r.first ? F.int(r.first.dwellings) : '–', r.first ? `${esc(r.first.name)} ${esc(r.first.period)}: ${esc(r.first.why)}` : esc(r.text), r.resolved ? esc(r.resolved.text) : r.upgrade ? '–' : '']) });
    const ex = t.rows.flatMap(r => (r.existing || []).map(e => [esc(r.label), esc(e.name), esc(e.period), mvalH(e.at0), mvalH(e.at100), e.worsenedAt != null ? pill(`worsened from ${e.worsenedAt}%`, 'bad') : 'not beyond the threshold']));
    return main + (ex.length ? `${h('Existing deficiencies (fail at 0% build-out)')}${T({ head: ['Variant', 'Junction', 'Period', 'At 0%', 'At 100%', 'Development effect'], rows: ex, compact: true })}` : '');
  }
  function secMitigation(P) {
    const m = mitigationTable(P), cd = conditionTable(P), eff = effects(P), t = data(P).triggers, c = context(P), to = data(P).trigOpts || {}, pr = pair(P);
    const effOpts = { '': 'Not linked', ...Object.fromEntries(eff.map(e => [e.id, e.text])) };
    for (const r of data(P).mitigation) if (r.effect && !effOpts[r.effect]) effOpts[r.effect] = `${r.effect} (not in the current results)`;
    const COMMIT = { '': 'Not recorded', yes: 'Y', no: 'N' }, cv = v => (v === true ? 'yes' : v === false ? 'no' : '');
    const mrows = data(P).mitigation.map((r, i) => { const x = m[i]; return [esc(x.ref), inp(`mitigation.${i}.measure`, r.measure), sel(`mitigation.${i}.effect`, effOpts, r.effect), inp(`mitigation.${i}.options`, r.options), inp(`mitigation.${i}.trigger`, r.trigger),
      sel(`mitigation.${i}.responsible`, RESPONSIBLE, r.responsible), inp(`mitigation.${i}.cost`, r.cost, 'numnull'), sel(`mitigation.${i}.funder`, RESPONSIBLE, r.funder), inp(`mitigation.${i}.rltp`, r.rltp), sel(`mitigation.${i}.mechanism`, MECHANISMS, r.mechanism),
      `<select data-cx="mitigation.${i}.committed" data-cxt="tri">${Kit.opt(COMMIT, cv(r.committed))}</select>` + chk(`mitigation.${i}.nonInfra`, r.nonInfra, 'non-infra') + chk(`mitigation.${i}.landOutside`, r.landOutside, 'land outside'), x.tag ? pill(esc(x.tag), 'warn') : '', btn('del', '✕', `mitigation.${i}`)]; });
    const mOpts = { '': '–', ...Object.fromEntries(m.map(x => [x.id, `${x.ref} ${x.measure || ''}`])) };
    const crows = data(P).conditions.map((r, i) => { const x = cd[i]; return [esc(x.ref), sel(`conditions.${i}.mitigation`, mOpts, r.mitigation), inp(`conditions.${i}.metric`, r.metric), inp(`conditions.${i}.threshold`, r.threshold), inp(`conditions.${i}.survey`, r.survey), inp(`conditions.${i}.frequency`, r.frequency), inp(`conditions.${i}.response`, r.response),
      sel(`conditions.${i}.effect`, effOpts, r.effect) + chk(`conditions.${i}.agreed`, r.agreed, 'applicant agrees (s108AA(1)(a))'), x.ok ? esc(x.basis) : pill(esc(x.basis), 'bad'), btn('del', '✕', `conditions.${i}`)]; });
    const stepSc = scById(P, to.scId) || pr.opt, upSel = Array.isArray(to.upgrades) ? to.upgrades : pr.mit && pr.mit !== stepSc ? [pr.mit.id] : [];
    const scOpts = Object.fromEntries(P.scenarios.map(s => [s.id, s.name]));
    const ups = P.scenarios.filter(s => s !== stepSc).map(s => `<label class="check"><input type="checkbox" data-cx-up="${esc(s.id)}" ${upSel.includes(s.id) ? 'checked' : ''}> ${esc(s.name)}${s.role === 'mitigation' ? ' (mitigation)' : ''}</label>`).join('');
    const trig = t ? triggerTables(t) + note(`${esc(t.basis)}. ${esc(t.label)}. ${t.copy ? `Parameter: ${esc(t.copy)}` : ''}`) : note('Not run yet.');
    return `${ctxLine(P)}${h('Mitigation register (AT ITA Appendix 3)')}${note(esc(EFFECT.basis))}<div class="tbl-wrap">${T({ head: ['Ref', 'Measure', 'Effect addressed', 'Options considered (incl. non-infrastructure)', 'Trigger', 'Responsible', 'Cost NZ$m', 'Funder', 'RLTP/LTP status', 'Mechanism', 'Committed by applicant / flags', 'Tag', ''], rows: mrows, empty: 'No measures yet.' })}</div>
      ${btn('add-mit', 'Add a measure')} ${btn('seed-mit', 'Seed from effects and the mitigation scenario')}
      ${h('Condition parameters')}${note(`Spare Construct never writes condition wording, only parameters. Each row needs a modelled effect (s108AA(1)(b)) or the applicant's agreement (s108AA(1)(a)), which only the author can record.${c.ftaa ? ` Fast-track: conditions ${esc(STD.labels.s84a)}.` : ''}`)}
      <div class="tbl-wrap">${T({ head: ['Ref', 'Measure', 'Trigger metric', 'Threshold', 'Survey method', 'Frequency', 'Response', 'Nexus', 'Basis', ''], rows: crows, empty: 'No condition parameters yet.' })}</div>${btn('add-cond', 'Add a row')} ${btn('seed-cond', 'Seed from the register')}
      ${h('Trigger finder')}<div class="set-grid">${fld('Scenario to step', sel('trigOpts.scId', scOpts, stepSc.id))}${fld('Step', `<input type="number" id="cx-step" value="${STD.trigger.step}" min="1" max="50">`, '% build-out')}${fld('Worst DoS above', `<input type="number" id="cx-dos" step="0.01" value="${STD.trigger.dos}">`)}${fld('LoS worse than', `<select id="cx-los">${Kit.opt({ D: 'D', C: 'C', E: 'E' }, STD.trigger.los)}</select>`)}
        <label class="check"><input type="checkbox" id="cx-queue" checked> 95th-percentile queue longer than the storage to the upstream junction</label>${fld('Ignore storage shorter than', `<input type="number" id="cx-minst" value="${STD.trigger.minStorage}" min="0">`, 'm')}${chk('trigOpts.anop', to.anop, 'ANOP fit drops a class')}</div>
      <div class="set-grid"><span class="note">With each upgrade (scenario):</span> ${ups || '<span class="note">Add a mitigation scenario to test an upgrade.</span>'}</div>
      ${note(`${ASSUMPTION} defaults (D13). An existing deficiency counts once the development raises its DoS by ${STD.trigger.worsen.dos} or its delay by ${STD.trigger.worsen.delay} s. Each step runs the model, so this takes a while.`)}${btn('triggers', 'Find the triggers', '', 'btn')}${trig}`;
  }
  function secOptions(P) {
    const a = alternatives(P), cx = data(P), scs = Object.fromEntries([['', 'Choose'], ...P.scenarios.map(s => [s.id, s.name])]);
    const ed = cx.options.map((o, i) => [inp(`options.${i}.name`, o.name), sel(`options.${i}.scId`, scs, o.scId), sel(`options.${i}.hierarchy`, { '': 'Not tagged', ...HIER }, o.hierarchy), inp(`options.${i}.cost`, o.cost, 'numnull'), inp(`options.${i}.timing`, o.timing), inp(`options.${i}.dsi`, o.dsi, 'numnull'), inp(`options.${i}.notes`, o.notes), btn('del', '✕', `options.${i}`)]);
    return `${ctxLine(P)}${h('Options')}${T({ head: ['Option', 'Scenario', 'Intervention hierarchy', 'Cost NZ$m', 'Timing', 'Predicted DSI change /yr', 'Notes', ''], rows: ed, empty: 'No options tagged: the table below uses every scenario other than the do-minimum.' })}${btn('add-opt', 'Add an option')}
      ${a.warnings.map(w => note(esc(w), true)).join('')}
      ${h(`Multi-criteria table (against ${esc(a.base.name)})`)}<div class="tbl-wrap">${altTable(a)}</div>${note(`Values per modelled peak hour, each period on its own. VKT and CO2e are ${esc(STD.labels.vkt)}. Junction results: ${esc(STD.labels.junction)}. ${esc(HIER_SRC)}.`)}
      ${h('Sections the document needs')}${T({ head: ['Section', 'Reference', 'Include', 'Why'], rows: a.sections.map(s => [esc(s.title), esc(s.ref), s.include === true ? pill('Yes', 'ok') : s.include === false ? 'No' : pill('Decide', 'warn'), esc(s.why)]), empty: 'No alternatives section is required for this pathway.' })}`;
  }
  const altTable = a => T({ head: ['Option', 'Hierarchy', 'ΔVHT veh·h/h', 'Worst junction', 'Bus time Δ min', { label: 'New cycle km', n: true }, 'CAS (DSI)', 'Predicted DSI', 'ΔVKT veh-km/h', 'ΔCO2e t/h', 'BCR', 'IPM band', { label: 'Cost', n: true }],
    rows: a.rows.map(r => r.ran ? [esc(r.name), esc(r.label), esc(ppText(r.pp.vht, 1)), esc(r.worst), esc(ppText({ by: Object.fromEntries(Object.entries(r.pp.route.by).map(([p, v]) => [p, v / 60])) }, 1)), num(r.cycleKm, 2), r.cas ? `${r.cas.crashes} (${r.cas.dsi})` : '–', r.dsi != null ? F.signed(r.dsi, 1) : 'not modelled', esc(ppText(r.pp.vkt, 0)), esc(ppText(r.pp.co2, 2)), esc(r.bcr), esc(r.ipm), F.money(r.cost)] : [esc(r.name), esc(r.label), 'Run this scenario and the do-minimum', '', '', '', '', '', '', '', '', '', '']), empty: 'No options.' });
  const MATRIX_OPTS = { '': 'To assess (author)', consistent: 'Consistent', inconsistent: 'Inconsistent', 'not-relevant': 'Not relevant' };
  function secFtaa(P) {
    const f = ftaaPackage(P), cx = data(P).ftaa, rec = cx.matrix || {};
    const mrow = r => [esc(r.rule), esc(r.req), r.author ? `<select data-cx="ftaa.matrix.${esc(r.ref)}.result" data-cxt="str">${Kit.opt(MATRIX_OPTS, (rec[r.ref] || {}).result || '')}</select> ${inp(`ftaa.matrix.${r.ref}.note`, (rec[r.ref] || {}).note || '', 'str', 'placeholder="evidence or reason"')}` : esc(r.result), esc(r.evidence)];
    return `${ctxLine(P)}${f.ftaa ? '' : note('This project is not on a Fast-track pathway: the package is shown for reference.', true)}
      <div class="set-grid">${chk('ftaa.nztaDesignation', cx.nztaDesignation, 'NZTA designation on or adjacent to the site')}${chk('ftaa.kiwirailDesignation', cx.kiwirailDesignation, 'KiwiRail designation on or adjacent to the site')}${fld('Invitation to comment sent', inp('ftaa.invitation', cx.invitation, 'date'))}${fld('Approval date', inp('ftaa.approval', cx.approval, 'date'))}${fld('Jobs reachable (s85)', inp('ftaa.jobs', cx.jobs))}${fld('Reliability (s85)', inp('ftaa.reliability', cx.reliability))}</div>
      ${h('s85(3) benefits and residual impacts')}<div class="cols2">${T({ head: ['Benefit', 'Spare Construct estimate'], rows: f.s85.benefits.map(([k, v]) => [esc(k), esc(v)]) })}${T({ head: ['Residual adverse impact (after mitigation)', 'Spare Construct estimate'], rows: f.s85.impacts.map(([k, v]) => [esc(k), esc(v)]) })}</div>${note(esc(f.s85.note))}
      ${h('s53 invitees')}${T({ head: ['Invitee', 'Basis'], rows: f.invitees.map(i => [esc(i.who), esc(i.basis)]) })}
      ${h('Timeline')}${T({ head: ['Step', 'Date', 'Rule', 'Basis'], rows: f.timeline.map(t => [esc(t.step), t.date ? esc(longDate(t.date)) : '–', esc(t.rule), esc(t.basis || '')]) })}
      ${h('Compliance matrix (Sch 5 cl 5(1)(h), (3))')}${T({ head: ['Rule', 'Requirement', 'Result', 'Evidence'], rows: f.matrix.map(mrow) })}${note('Objectives and policies need the author\'s assessment: record a result for each before lodging.')}
      ${h('s84A staging')}${T({ head: ['Stage', 'Trigger', 'Measure'], rows: f.s84a.map(r => [esc(r.stage), esc(r.trigger), esc(r.measure)]), empty: 'Run the trigger finder or add triggers to the mitigation register.' })}
      ${f.notes.map(n => note(esc(n))).join('')}${f.reviewer.sensitivity ? '' : note('The reviewer pack has no sensitivity runs yet: run the sensitivity pack (Report tab).', true)}${btn('export', `Download the Fast-track package (${hasZip() ? 'ZIP' : 'JSON bundle'})`, 'compliance-extra-ftaa', 'btn')}`;
  }
  function secPlanChange(P) {
    const pc = planChange(P), cx = data(P).planChange;
    return `${ctxLine(P)}${pc.process.map(p => note(esc(p.text), p.level === 'warn')).join('')}<div class="set-grid">${context(P).kind === 'pc' ? '' : fld('Plan change type', sel('planChange.kind', { '': 'Not a plan change', 'pc-private': 'Private', 'pc-council': 'Council' }, cx.kind)) + fld('Notification date', inp('planChange.notification', cx.notification, 'date'))}${chk('planChange.useProposed', cx.useProposed, 'Use PC120 proposed capacity (labelled proposed)')}</div>
      ${h('Full build-out')}${note(`${esc(pc.buildout.label)}: ${F.int(pc.buildout.dwellings)} dwellings and ${F.int(pc.buildout.gfa)} m² GFA in ${pc.buildout.zones.length} zone${pc.buildout.zones.length === 1 ? '' : 's'}. ${esc(pc.buildout.rule)}. Enter zoned capacity per zone (zone inspector › Zoned capacity). Interim stages: set a scenario's build-out %.`)}${pc.buildout.zones.length ? btn('fullbuild', 'Create the full build-out scenario') : ''}
      ${h('s32 transport table')}${note(`<b>${esc(pc.s32Label)}</b>`)}<div class="tbl-wrap">${T({ head: ['Option', 'Hierarchy', 'Travel time', 'Safety', 'Vehicle operating cost', 'Infrastructure cost and timing', 'Risk of acting', 'Risk of not acting'], rows: pc.s32.map(r => [esc(r.name), esc(r.label), esc(r.travel), esc(r.safety), esc(r.voc), esc(r.cost), esc(r.acting), esc(r.notActing)]), empty: 'Run the do-minimum and at least one option.' })}</div>
      ${h('Policy rows')}${T({ head: ['Reference', 'Policy', 'Spare Construct evidence'], rows: pc.policy.map(r => [esc(r.ref), esc(r.policy), esc(r.text)]) })}
      ${h('Draft road function table')}${T({ head: ['Road', 'Class', 'Draft function', 'RASF', 'Future Connect', 'Limit km/h', 'Lanes', 'Cycling'], rows: pc.roads.map(r => ({ cells: [esc(r.name), esc(r.cls), esc(r.fn), esc(r.rasf), esc(r.fc), num(r.speed), num(r.lanes), esc(r.cycle)], fly: 'link:' + r.id })), empty: 'No proposed roads.' })}
      ${h('Sch 1 cl 23 response pack')}${T({ head: ['Item', 'Status'], rows: pc.cl23.map(([a, b]) => [esc(a), esc(b)]) })}`;
  }
  function secSpeed(P) {
    const rows = speedCheck(P), sch = schoolSpeeds(activeSc(P)), cb = data(P).speed.cbds, cx = data(P).speed;
    return `${ctxLine(P)}${note(`Set each road's Rule class and proposed limit in the road inspector (Speed limit), on the option scenario. ${esc(STD.speed.name)}, ${esc(STD.speed.edition)}${verify(STD.speed)}. A divided road's two carriageways count once in the section length.`)}
      ${T({ head: ['Road', 'Rule class', 'Limit km/h', { label: 'Section m', n: true }, { label: 'Minimum m', n: true }, 'Decision-maker', 'Flags'], rows: rows.map(r => ({ cells: [esc(r.name), esc(r.classLabel) + (r.onf ? `<br><span class="note">${esc(r.onf)}</span>` : ''), `${num(r.current)} → ${num(r.limit)}`, F.int(r.len), F.int(r.minLen), esc(r.decision), r.flags.length ? r.flags.map(f => `${pill('Flag', 'warn')} ${esc(f.text)} <span class="note">${esc(f.rule)}</span>`).join('<br>') : pill('OK', 'ok')], fly: 'link:' + r.ids[0] })), empty: 'No road has a Rule class or proposed limit yet.' })}
      ${h('School speed limits (NSLR variable records)')}${T({ head: ['Road', 'Limit', 'Category', 'Travel windows'], rows: sch.map(s => [esc(s.name), `${num(s.v)} km/h`, esc(s.cat), esc(s.windows)]), empty: 'No school variable limits loaded (Traffic data › NSLR).' })}
      ${h('Cost benefit disclosure statement (cl 3.3)')}<div class="set-grid">${fld('Road section', `<select id="cx-cbds">${Kit.opt(Object.fromEntries([['', 'Choose a section'], ...rows.map(r => [r.ids.join(','), `${r.name} (${r.current} → ${r.limit} km/h)`])]), ui.cbdsIds)}</select>`)}${fld('Implementation cost', inp('speed.cost', cx.cost, 'num'), 'NZ$')}${fld('Tool', inp('speed.tool', cx.tool))}${chk('speed.consultationChanged', cx.consultationChanged, 'The limit changed after consultation')}</div>${btn('cbds', 'Generate the CBDS', '', 'btn')}
      ${cb ? (cb.ok ? cbdsHtml(cb) : note(esc(cb.why), true)) : ''}`;
  }
  const cbdsHtml = cb => `<div class="watermarked" data-watermark="${esc(cb.watermark)}"><p class="note"><b>${esc(cb.watermark)}</b> · ${esc(cb.name)} · decision-maker ${esc(cb.decision)}</p>${cb.headings.map(x => `${h(esc(x.title) + ` <span class="note">${esc(x.rule)}</span>`)}${T({ head: ['Item', 'Value'], rows: x.rows.map(r => [esc(r[0]), esc(r[1])]) })}${x.basis ? `<p class="note">${esc(x.basis)}</p>` : ''}`).join('')}${cb.warn.map(w => note(esc(w), true)).join('')}<p class="note disclaimer" data-disclaimer="D9"><b>D9.</b> ${esc(cb.disclaimer)}</p></div>`;
  function anopTable(a) {
    return T({ head: ['Spare Construct', 'Period', 'Mode', 'Future Connect', 'Baseline LoS', 'Preferred', 'Acceptable', 'With proposal', 'Δ deficit', 'Fit'], compact: true,
      rows: a.rows.flatMap(r => r.modes.map((m, i) => [i ? '' : `${esc(r.corridor.name)}<br><span class="note">${esc(r.landUse)}</span>`, i ? '' : esc(r.period), esc(m.label), `${esc(m.hier)}<br><span class="note">${esc(m.hierSrc || '')}</span>`, esc(m.base || 'N/A'), esc(m.pref || '–'), esc(m.acc || '–'), `${esc(m.proj || 'N/A')} <span class="note">${esc(m.metric)}</span>`,
        m.defBase != null && m.defProj != null ? F.signed(m.defProj - m.defBase, 0) : '–', i ? '' : `<b>${esc(r.fit || r.fitText || '–')}</b>`])), empty: 'Run the do-minimum and the option to assess ANOP.' });
  }
  function secAnop(P) {
    const a = anop(P), cx = data(P).anop;
    const ed = cx.corridors.map((c, i) => [inp(`anop.corridors.${i}.name`, c.name), sel(`anop.corridors.${i}.landUse`, LU, c.landUse || 'other'), sel(`anop.corridors.${i}.walk`, { none: 'None', primary: 'Primary', secondary: 'Secondary', tertiary: 'Tertiary' }, c.walk || 'none'),
      inp(`anop.corridors.${i}.crossings`, c.crossings, 'numnull'), inp(`anop.corridors.${i}.crossDelay`, c.crossDelay, 'numnull'), chk(`anop.corridors.${i}.centre`, c.centre, 'centre') + chk(`anop.corridors.${i}.school`, c.school, 'school'), inp(`anop.corridors.${i}.rel.gt`, (c.rel || {}).gt, 'numnull'), `${(c.links || []).length} roads`, btn('del', '✕', `anop.corridors.${i}`)]);
    return `${ctxLine(P)}${note(`<b>${esc(a.label)}</b> Speeds are per direction; the worse direction sets the LoS.`)}${a.same ? note(esc(a.note), true) : ''}<div class="set-grid">${fld('Project', inp('anop.project', cx.project))}${fld('Assessor', inp('anop.assessor', cx.assessor))}${fld('Option', inp('anop.option', cx.option))}</div>
      ${h('Corridors')}${cx.corridors.length ? T({ head: ['Spare Construct', 'Land use', 'Walking hierarchy', 'Crossings', 'Crossing delay s', 'Halve bands', 'GT 85th/median', 'Roads', ''], rows: ed }) : note('Using named arterials and collectors over 300 m. Keep them to edit land use, walking hierarchy and crossings.')}${cx.corridors.length ? '' : btn('anop-keep', 'Keep these corridors')}
      ${h('Table ANOP-1')}<div class="tbl-wrap">${anopTable(a)}</div>${disc('D5', P)}${btn('export', 'Download anop.csv', 'compliance-extra-anop')}`;
  }
  function secParking(P) {
    const pk = parking(P), rows = data(P).parking.rows;
    const ed = rows.map((r, i) => { const x = pk.rows[i]; return [sel(`parking.rows.${i}.activity`, Object.fromEntries(Object.entries(PARK_ACT).map(([k, v]) => [k, v.label])), r.activity), inp(`parking.rows.${i}.qty`, r.qty, 'num') + `<span class="unit">${esc(x.unit)}</span>`, inp(`parking.rows.${i}.maxCars`, r.maxCars, 'numnull'), inp(`parking.rows.${i}.cars`, r.cars, 'num'),
      `${x.accReq.required ? pill(`Required: ${num(x.accReq.n)}`, 'warn') : 'No'} ${inp(`parking.rows.${i}.accessible`, r.accessible, 'num')}${STD.parking.access4.includes(r.activity) ? chk(`parking.rows.${i}.listedZone`, r.listedZone, 'listed zone (E27.6.3.2A(4))') : ''}`,
      `${x.cyc.visitor != null ? num(x.cyc.visitor, 1) : inp(`parking.rows.${i}.cycleVisitorReq`, r.cycleVisitorReq, 'numnull')} / ${inp(`parking.rows.${i}.cycleVisitor`, r.cycleVisitor, 'num')}`, `${x.cyc.secure != null ? num(x.cyc.secure, 1) : inp(`parking.rows.${i}.cycleSecureReq`, r.cycleSecureReq, 'numnull')} / ${inp(`parking.rows.${i}.cycleSecure`, r.cycleSecure, 'num')}`,
      `${num(x.loadReq)} / ${inp(`parking.rows.${i}.loading`, r.loading, 'num')}`, `${inp(`parking.rows.${i}.showers`, r.showers, 'num')}`, x.complies ? pill('Yes', 'ok') : `${pill('No', 'bad')} ${esc(x.why.join('; '))}`, btn('del', '✕', `parking.rows.${i}`)]; });
    const ind = pk.rows.filter(r => r.induced);
    return `${note(esc(pk.label))}<div class="tbl-wrap">${T({ head: ['Activity', 'Quantity', 'AUP maximum', 'Proposed car parks', 'Accessible (indicative number, verify)', 'Cycle visitor req (unrounded) / prop', 'Cycle secure req (unrounded) / prop', 'Loading req / prop', 'Showers prop', 'Complies', ''], rows: ed, empty: 'Add each activity (Table P1).' })}</div>${btn('add-park', 'Add an activity')}
      ${rows.length ? note(`Totals (summed, then 0.5 rounded up, E27.6.2(9)): visitor cycle ${pk.need.visitor} required / ${pk.have.visitor} proposed; secure ${pk.need.secure} / ${pk.have.secure}; showers ${pk.need.showers} / ${pk.have.showers} (E27.6.2.6 on ${F.int(pk.nonResGfa)} m² non-residential GFA); loading ${pk.need.loading} / ${pk.have.loading}.${pk.totalsWhy.length ? ' ' + esc(pk.totalsWhy.join('; ')) + '.' : ''}${pk.unknownCycle ? ' Some cycle rates are not encoded: enter them from Table E27.6.2.5.' : ''}`, !pk.totalsOk) : ''}
      ${note(`Accessible car parks: ${esc(STD.accessible.name)} (${esc(STD.accessible.basis)})${verify(STD.accessible)}.`)}${ind.map(r => note(`<b>${esc(r.label)}:</b> ${esc(r.induced.text)}. ${esc(r.induced.status)}.`, true)).join('')}`;
  }
  function secSubdivision(P) {
    const s = subdivision(P), cx = data(P).subdivision, c = s.connectivity;
    return `${ctxLine(P)}${note(esc(s.warning), true)}<div class="set-grid">${fld('Lots (dwellings)', inp('subdivision.lots', cx.lots, 'num'))}</div>
      ${s.trips.length ? note(`Trips from ${F.int(s.lots)} lots: ${s.trips.map(t => `${esc(t.period)} ${num(t.total)} veh/h`).join(', ')}. ${esc(s.tripsBasis)}.`) : ''}${s.e27Text ? note(esc(s.e27Text) + '.', s.e27.some(r => r.exceeded)) : ''}
      ${h('Traffic change on existing roads (E38.12.2(g))')}${T({ head: ['Road', 'Period', { label: 'Without veh/h', n: true }, { label: 'With veh/h', n: true }, { label: 'Change', n: true }, { label: '%', n: true }, 'Screen'], rows: s.links.map(l => ({ cells: [esc(l.name), esc(l.period), num(l.base), num(l.opt), F.signed(l.d, 0), l.pct == null ? 'new' : F.signed(l.pct, 0), l.significant ? pill('Check', 'warn') : ''], fly: 'link:' + l.id })), empty: 'Run the do-minimum and the option.' })}${note(esc(s.linksBasis))}
      ${h('Connectivity (E38.12.2(e))')}${T({ head: ['Indicator', 'Value', 'Basis'], rows: [['Intersection density', c.density != null ? `${num(c.density, 0)} per km²` : '–', esc(c.densityBasis || '')], ['Route directness', c.directness != null ? num(c.directness, 2) : '–', esc(c.directnessBasis || '')], ['Walk to a bus stop', c.walkStop != null ? `${num(c.walkStop, 1)} min` : 'No bus routes modelled', esc(c.walkBasis || '')], ['Walk to a school', c.walkSchool != null ? `${num(c.walkSchool, 1)} min` : 'No school zone', esc(c.walkBasis || '')]] })}
      ${h('Rear site access (Table E38.8.1.2.1)')}${T({ head: ['Access', 'Rear sites', 'Legal width m', 'Formed width m', 'Length m', 'Pedestrian m', 'Check', ''], rows: (cx.accesses || []).map((x, i) => [inp(`subdivision.accesses.${i}.name`, x.name), inp(`subdivision.accesses.${i}.sites`, x.sites, 'num'), inp(`subdivision.accesses.${i}.legal`, x.legal, 'numnull'), inp(`subdivision.accesses.${i}.formed`, x.formed, 'numnull'), inp(`subdivision.accesses.${i}.len`, x.len, 'numnull'), inp(`subdivision.accesses.${i}.ped`, x.ped, 'numnull'),
        s.accesses[i].ok ? pill('OK', 'ok') : s.accesses[i].flags.map(f => `${pill('Flag', 'warn')} ${esc(f)}`).join('<br>'), btn('del', '✕', `subdivision.accesses.${i}`)]), empty: 'No rear accesses entered.' })}${btn('add-access', 'Add a rear access')}`;
  }
  const SIG_OPTS = { '': 'Not set', road: 'Road', regional: 'Regionally significant', national: 'Nationally significant' };
  function secPolicy(P) {
    const p = policy(P), g = gpsMetrics(P), f = funding(P), fx = data(P).funding, gx = data(P).gps, sc = activeSc(P);
    const zOpts = { '': 'Choose a zone', ...Object.fromEntries(Object.values(sc.zones).filter(z => z.kind !== 'gateway').map(z => [z.id, z.name || z.id])) };
    const routesEd = T({ head: ['Route', 'Roads (names, comma-separated)', 'Significance', ''], rows: (gx.routes || []).map((r, i) => [inp(`gps.routes.${i}.name`, r.name), inp(`gps.routes.${i}.roads`, r.roads), sel(`gps.routes.${i}.significance`, SIG_OPTS, r.significance || ''), btn('del', '✕', `gps.routes.${i}`)]), empty: 'No routes defined: the scheme network (arterials and motorways) is used with no significance class.' });
    const destEd = T({ head: ['Destination zone', 'Significance', ''], rows: (gx.destinations || []).map((d, i) => [sel(`gps.destinations.${i}.zoneId`, zOpts, d.zoneId), sel(`gps.destinations.${i}.significance`, SIG_OPTS, d.significance || ''), btn('del', '✕', `gps.destinations.${i}`)]), empty: 'No tagged destinations.' });
    const stageEd = T({ head: ['Stage', 'Dwellings', 'Year 75% complete', ''], rows: (gx.stages || []).map((s, i) => [inp(`gps.stages.${i}.name`, s.name), inp(`gps.stages.${i}.dwellings`, s.dwellings, 'num'), inp(`gps.stages.${i}.year75`, s.year75, 'numnull'), btn('del', '✕', `gps.stages.${i}`)]), empty: 'No stages entered: the development zones and their opening years are used.' });
    const sh = f.shares ? T({ head: ['Asset', 'Period', { label: 'Total veh/h', n: true }, 'Development area', { label: 'veh/h', n: true }, { label: 'Share', n: true }], compact: true, rows: f.shares.assets.flatMap(a => a.areas.map((z, k) => [k ? '' : esc(a.name), k ? '' : esc(a.period), k ? '' : num(a.total), esc(z.name), num(z.v), F.pct(z.share)])) }) + note(esc(f.shares.method)) : '';
    return `${ctxLine(P)}${h('Network and policy alignment')}${note(`Affected roads: ${esc(p.basis)}. ${p.inconsistencies} with an inconsistency to highlight (AT ITA).`)}
      <div class="tbl-wrap">${T({ head: ['Road', 'FC general traffic', 'FC public transport', 'FC freight', 'FC cycling', 'RASF (advisory)', 'ONF', 'Inconsistencies'], rows: p.rows.slice(0, 40).map(r => ({ cells: [esc(r.name), ...r.fc.map(esc), esc(r.rasf), esc(r.onf), r.flags.length ? r.flags.map(x => `${pill('Flag', 'warn')} ${esc(x)}`).join('<br>') : ''], fly: 'link:' + r.id })), empty: 'No affected roads: run the do-minimum and the option.' })}</div>
      ${T({ head: ['Reference', 'Policy', 'Spare Construct evidence'], rows: p.policy.map(r => [esc(r.ref), esc(r.policy), esc(r.text) + (r.info ? ` ${pill('information only')}` : '')]) })}
      ${h(`IPM stage 2 GPS metrics (${esc(g.label)})`)}${T({ head: ['Route', 'Significance', { label: 'Base s', n: true }, { label: 'Option s', n: true }, { label: 'Saving %', n: true }], rows: g.routes.map(r => [esc(r.name), esc(r.significance), num(r.base), num(r.opt), num(r.saving, 1)]), empty: 'Run the do-minimum and the option.' })}
      ${g.network.saving != null ? note(`Scheme network: ${num(g.network.saving, 1)}% travel-time saving; significance ${esc(g.network.significance)}. HCV rating <b>${esc(g.network.hcv)}</b>; general traffic <b>${esc(g.network.gt)}</b> (${esc(g.label)}). ${esc(g.network.note)}`, !g.network.sigSet) : ''}
      <details><summary>Routes, destinations and stages</summary>${routesEd}${btn('add-route', 'Add a route')}${destEd}${btn('add-dest', 'Add a destination')}${stageEd}${btn('add-stage', 'Add a stage')}</details>
      ${T({ head: ['Destination', 'Significance', 'Minutes saved'], rows: g.destinations.map(d => [esc(d.name), esc(d.significance), num(d.minutes, 2)]), empty: 'Tag destination zones (Routes, destinations and stages) to report minutes saved.' })}
      ${note(`Houses enabled: ${F.int(g.houses.total)} (${esc(g.houses.basis)}). DSI reduction per NZ$100m: ${g.dsi.per100m != null ? num(g.dsi.per100m, 2) : 'enter the DSI reduction'}; collective risk ${esc(g.dsi.collective)}.`)}
      <div class="set-grid">${fld('DSI reduction', inp('gps.dsiReduction', gx.dsiReduction, 'numnull'), 'DSI a year')}${fld('Scheme cost', inp('gps.cost', gx.cost, 'numnull'), 'NZ$m')}${fld('Collective risk', inp('gps.collective', gx.collective))}</div>
      ${h('Funding attribution')}${note(`<b>${esc(f.caveat)}</b>`, true)}<div class="set-grid">${fld('Upgraded assets (road names)', inp('funding.assetRoads', fx.assetRoads, 'str', 'placeholder="e.g. Waihoehoe Road, Mill Road"'))}</div>${note(`Assets: ${esc(f.assetsBasis)}.`)}
      ${T({ head: ['Asset', 'Period', { label: 'Existing', n: true }, { label: 'Committed', n: true }, { label: 'Development', n: true }, { label: 'Development share', n: true }], rows: f.assets.slice(0, 30).map(a => [esc(a.name), esc(a.period), num(a.existing), num(a.committed), num(a.development), F.pct(a.devShare)]), empty: 'Run the option.' })}
      ${btn('fund-shares', 'Run select-link shares by development area')}${sh}
      ${note(`Growth share ${f.growth.share != null ? F.pct(f.growth.share, 1) : '–'}: ${esc(f.growth.formula)} (households ${F.int(f.growth.He)} → ${F.int(f.growth.Hf)}, jobs ${F.int(f.growth.Ee)} → ${F.int(f.growth.Ef)}). ${esc(f.growth.label)}.`)}
      <div class="set-grid">${fld('Levy (PV)', inp('funding.levy', fx.levy, 'num'), 'NZ$m')}${fld('Existing jobs', inp('funding.employment.existing', fx.employment.existing, 'numnull'))}${fld('Future jobs', inp('funding.employment.future', fx.employment.future, 'numnull'))}${fld('Fast-track approval', inp('funding.approval', fx.approval, 'date'))}</div>
      ${fld('Levy area (lat, lng; one point per line)', `<textarea data-cx-poly="funding.polygon" rows="3">${esc((fx.polygon || []).map(p => p.join(', ')).join('\n'))}</textarea>`)}
      ${f.incidence ? (f.incidence.error ? note(esc(f.incidence.error), true) : note(`Benefit inside the levy area: <b>${F.pct(f.incidence.insideShare)}</b>, outside ${F.pct(1 - f.incidence.insideShare)} (${esc(f.incidence.basis)}).`)) : ''}
      ${note(`BCR_G with the levy as Cp: ${esc(f.bcrG.text)}.`)}${f.dc ? note(esc(f.dc.text)) : ''}`;
  }
  function secTou(P) {
    const cx = data(P).tou, r = cx.result, avail = today() >= STD.dates.tou, A_ = r && r.ok ? r.annual : null;
    return `${ctxLine(P)}${note(`<b>${esc(STD.labels.tou)}</b>. ${avail ? '' : `Available from ${esc(longDate(STD.dates.tou))} (LTMA ss65A-65ZO commencement).`}`, !avail)}
      <div class="set-grid">${fld('Scheme', sel('tou.kind', { cordon: 'Cordon charge', area: 'Area charge' }, cx.kind))}${fld('Light vehicles', inp('tou.light', cx.light, 'num'), 'NZ$')}${fld('Heavy vehicles', inp('tou.heavy', cx.heavy, 'num'), 'NZ$')}${fld('Period', sel('tou.period', { AM: 'AM peak', PM: 'PM peak' }, cx.period))}${fld('Scheme cost (PV)', inp('tou.cost', cx.cost, 'num'), 'NZ$m')}</div>
      ${fld('Charging area (lat, lng; one point per line)', `<textarea data-cx-poly="tou.polygon" rows="3">${esc((cx.polygon || []).map(p => p.join(', ')).join('\n'))}</textarea>`)}${avail ? btn('tou', 'Run the screening', '', 'btn') : ''}
      ${r && r.ok ? `${note(esc(r.method))}${(r.leaks || []).length ? note(`The charge is only partly represented on ${r.leaks.length} short charged arc${r.leaks.length === 1 ? '' : 's'} (5 km/h speed floor): treat the diversion around them as understated.`, true) : ''}${h('Sampled trips (s65D(3))')}${T({ head: ['From', 'To', { label: 'Trips before', n: true }, { label: 'Trips after', n: true }, { label: 'Time before min', n: true }, { label: 'Time after min', n: true }, { label: 'Charge NZ$', n: true }], rows: r.sample.map(o => [esc(o.from), esc(o.to), num(o.q0, 1), num(o.q1, 1), num(o.t0 / 60, 1), num(o.t1 / 60, 1), o.paid ? num(o.pay, 2) : '–']) })}
        ${T({ head: ['Network (real travel times)', { label: 'VKT before', n: true }, { label: 'VKT after', n: true }, { label: 'VHT before', n: true }, { label: 'VHT after', n: true }], rows: [['State highways', num(r.impacts.sh[0].vkt), num(r.impacts.sh[1].vkt), num(r.impacts.sh[0].vht, 1), num(r.impacts.sh[1].vht, 1)], ['Local roads', num(r.impacts.local[0].vkt), num(r.impacts.local[1].vkt), num(r.impacts.local[0].vht, 1), num(r.impacts.local[1].vht, 1)], ['Bus routes (min)', '', '', num(r.impacts.pt[0] / 60, 1), num(r.impacts.pt[1] / 60, 1)]] })}
        ${note(`Monetised (NZ$m a year): user benefit ${num(A_.user, 2)} (time ${num(A_.time, 2)}, charges paid ${num(A_.charges, 2)}); revenue ${num(A_.revenue, 2)} (a transfer); deadweight loss ${num(A_.deadweight, 2)}; VOC ${num(A_.voc, 2)}, CO2 ${num(A_.co2, 2)}, safety ${num(A_.safety, 2)}; national total ${num(A_.total, 2)}. BCR_N ${r.bcrN != null ? num(r.bcrN, 1) : 'enter the cost'}; BCR_G (revenue as Cp) ${r.bcrG != null ? num(r.bcrG, 1) : esc(r.bcrGText || 'enter the cost')}.`)}` : r && !r.ok ? note(esc(r.why), true) : ''}`;
  }
  function secClimate(P) {
    const c = climate(P);
    return `${note(esc(c.note))}${T({ head: ['Link', 'Future Connect role', { label: 'Length in hazard m', n: true }, 'Hazard layer', 'Only access', 'Assessment'], rows: c.rows.map(r => ({ cells: [esc(r.name), esc(r.fc), F.int(r.len), esc(r.layer), r.only ? '<b>Yes</b>' : 'No', esc(r.assess)], fly: 'link:' + r.link })), empty: c.loaded ? 'No modelled road lies in a mapped flood plain or coastal inundation area.' : 'Check hazards first (Traffic data › Data sources › Flood and coastal hazards).' })}`;
  }
  function secHandover(P) {
    const m = handoverManifest(P), cx = data(P).tsdg, rl = data(P).rltp;
    return `${note(esc(disclaimer('D12', P, 'the named tool')))}${T({ head: ['Pack', 'For', 'State', ''], rows: m.map(x => [esc(x.pack), esc(x.tool), x.ready ? pill('Ready', 'ok') : pill(x.why ? esc(x.why) : 'Needs inputs', 'warn'), btn('export', 'Download', x.exportId)]) })}
      ${h('Design-life run (TSDG)')}<div class="set-grid">${fld('Demand growth', inp('tsdg.pct', cx.pct, 'num'), '%')}${fld('Design life', inp('tsdg.years', cx.years, 'num'), 'years')}</div>${btn('designlife', 'Run the design-life test')}${cx.result ? note(`Run ${esc(cx.result.at.slice(0, 10))}: +${cx.result.pct}% demand.`) : ''}
      ${h('RLTP activity sheet')}<div class="set-grid">${fld('Objective', inp('rltp.objective', rl.objective))}${fld('Measures', inp('rltp.measures', rl.measures))}${fld('Cost per year', inp('rltp.costPerYear', rl.costPerYear), 'NZ$')}${fld('Duration', inp('rltp.duration', rl.duration))}${fld('Funding sources', inp('rltp.sources', rl.sources))}</div>`;
  }
  const RENDER = { overview: secOverview, lint: secLint, precinct: secPrecinct, mitigation: secMitigation, options: secOptions, ftaa: secFtaa, planchange: secPlanChange, speed: secSpeed, anop: secAnop,
    parking: secParking, subdivision: secSubdivision, policy: secPolicy, tou: secTou, climate: secClimate, handover: secHandover };
  function sectionHtml(id, P = Store.P) { try { return (RENDER[id] || secOverview)(P); } catch (e) { console.error(e); return note(`This section could not be shown: ${esc(e.message)}. Re-run the model, then reopen the tab.`, true); } }
  function tabHtml() {
    const P = Store.P; if (!P) return '';
    const L = lint(P);
    return `<div class="bar-h"><h3>Compliance+</h3><span class="note">Second compliance module: scope, staging, conditions, options and hand-over packs</span></div>
      ${L.blocking.length ? note(`<b>Report and package exports are blocked:</b> ${esc(L.blocking[0].rule)} ("${esc(L.blocking[0].match)}"). <a href="#" data-cx-sec="lint">Open the linter</a>.`, true) : ''}
      <div class="seg" role="tablist">${SECTIONS.map(([id, label]) => `<button class="btn small${ui.sec === id ? ' on' : ''}" data-cx-sec="${id}" role="tab" aria-selected="${ui.sec === id}">${label}</button>`).join('')}</div>
      <div class="cx-body">${sectionHtml(ui.sec, P)}</div>`;
  }

  /* ---------- handlers ---------- */
  const rerender = () => { try { Drawer.render(); } catch { } };
  function parseVal(t) {
    const ty = t.dataset.cxt;
    if (ty === 'bool') return !!t.checked;
    if (ty === 'num') return t.value === '' ? 0 : +t.value || 0;
    if (ty === 'numnull') return t.value === '' ? null : (isFinite(+t.value) ? +t.value : null);
    if (ty === 'date') return t.value || '';
    if (ty === 'tri') return t.value === 'yes' ? true : t.value === 'no' ? false : null;   // Y / N / not recorded
    return String(t.value || '').trim();
  }
  function setField(path, v, P = Store.P) {
    const root = data(P);
    if (/\.committed$/.test(path)) v = v === true ? true : v === false ? false : null;
    Kit.setPath(root, path, v);
    Store.emit('meta');
  }
  const parsePoly = s => String(s || '').split(/\n+/).map(l => l.split(/[,\s]+/).filter(Boolean).map(Number)).filter(p => p.length === 2 && p.every(isFinite));
  function onChange(e) {
    const t = e.target;
    if (t.dataset.cx) { setField(t.dataset.cx, parseVal(t)); rerender(); return true; }
    if (t.dataset.cxUp) {   // trigger finder: the upgrade scenarios to test
      const to = data(Store.P).trigOpts, pr = pair(Store.P), stepSc = scById(Store.P, to.scId) || pr.opt;
      const cur = new Set(Array.isArray(to.upgrades) ? to.upgrades : pr.mit && pr.mit !== stepSc ? [pr.mit.id] : []);
      if (t.checked) cur.add(t.dataset.cxUp); else cur.delete(t.dataset.cxUp);
      to.upgrades = [...cur]; Store.emit('meta'); return true;
    }
    if (t.dataset.cxPoly) { setField(t.dataset.cxPoly, parsePoly(t.value)); rerender(); return true; }
    if (t.dataset.cxJson) { try { const v = JSON.parse(t.value || '[]'); if (!Array.isArray(v)) throw new Error('expected a list'); setField(t.dataset.cxJson, v); rerender(); } catch (er) { Kit.toast(`Could not read the precinct data: ${er.message}. Check the JSON and try again.`, 'err'); } return true; }
    if (t.id === 'cx-cbds') { ui.cbdsIds = t.value; return true; }
    return false;
  }
  async function busy(text, fn) { try { Kit.busy(text); return await fn(); } catch (e) { console.error(e); Kit.toast(`${text.replace(/…$/, '')} failed: ${e.message}`, 'err'); return null; } finally { Kit.busy(null); } }
  // Synchronous so a click elsewhere in the tab (data-fly rows) falls through to the drawer's own handling
  function onClick(e) {
    const s = e.target.closest('[data-cx-sec]'); if (s) { e.preventDefault(); ui.sec = s.dataset.cxSec; rerender(); return true; }
    const b = e.target.closest('[data-cx-do]'); if (!b) return false;
    act_(b.dataset.cxDo, b.dataset.arg).catch(er => { console.error(er); Kit.toast(`That did not work: ${er.message}`, 'err'); });
    return true;
  }
  async function act_(act, arg) {
    const P = Store.P, cx = data(P);
    if (act === 'del') { const p = arg.split('.'), i = +p.pop(), arr = Kit.getPath(cx, p.join('.')); if (Array.isArray(arr)) arr.splice(i, 1); Store.emit('meta'); }
    else if (act === 'lint-ignore') { cx.lint.ignore[arg] = true; Store.emit('meta'); }
    else if (act === 'lint-override') {
      const reason = await Kit.ask('Why is this sentence correct? (recorded with the date; it releases the export block)', '');
      if (reason && String(reason).trim()) { overrideLint(P, arg, reason); Store.emit('meta'); Kit.toast('Override recorded. It is listed in the linter findings and the linter CSV.'); }
    }
    else if (act === 'lint-override-clear') { overrideLint(P, arg, ''); Store.emit('meta'); }
    else if (act === 'add-reg') { cx.precinct.register.push({ id: U.uid('reg'), ref: '', dwellings: 0, retail: 0, commercial: 0, community: 0, date: '', enabled: '' }); Store.emit('meta'); }
    else if (act === 'add-mit') { cx.mitigation.push(blankMit()); Store.emit('meta'); }
    else if (act === 'seed-mit') { const n = seedMitigation(P); Store.emit('meta'); Kit.toast(n ? `Added ${n} row${n === 1 ? '' : 's'} to the mitigation register.` : 'Nothing new to add: every effect and mitigation change is in the register.'); }
    else if (act === 'add-cond') { cx.conditions.push(blankCond()); Store.emit('meta'); }
    else if (act === 'seed-cond') { const n = seedConditions(P); Store.emit('meta'); Kit.toast(n ? `Added ${n} condition parameter row${n === 1 ? '' : 's'}. Tick the applicant's agreement only where the applicant has given it.` : 'Every measure already has a row, or no measure is named.'); }
    else if (act === 'add-opt') { cx.options.push({ id: U.uid('opt'), name: `Option ${cx.options.length + 1}`, scId: '', hierarchy: '', cost: null, timing: '', dsi: null, notes: '' }); Store.emit('meta'); }
    else if (act === 'add-park') { cx.parking.rows.push(blankPark()); Store.emit('meta'); }
    else if (act === 'add-access') { cx.subdivision.accesses.push(blankAccess()); Store.emit('meta'); }
    else if (act === 'add-route') { cx.gps.routes.push({ id: U.uid('rt'), name: `Route ${cx.gps.routes.length + 1}`, roads: '', significance: '' }); Store.emit('meta'); }
    else if (act === 'add-dest') { cx.gps.destinations.push({ zoneId: '', significance: '' }); Store.emit('meta'); }
    else if (act === 'add-stage') { cx.gps.stages.push({ name: `Stage ${cx.gps.stages.length + 1}`, dwellings: 0, year75: null }); Store.emit('meta'); }
    else if (act === 'anop-keep') { cx.anop.corridors = anopCorridors(P).map(c => ({ ...c, id: U.uid('cor'), auto: false })); Store.emit('meta'); }
    else if (act === 'fullbuild') { const ns = createFullBuildout(P); if (ns) { Store.emit('scenario'); Kit.toast(`Created "${ns.name}". Run it to see the full build-out.`); } }
    else if (act === 'export') { if (typeof UI !== 'undefined') UI.exportFile(arg); return true; }
    else if (act === 'cbds') { const ids = (ui.cbdsIds || (document.getElementById && document.getElementById('cx-cbds') || {}).value || '').split(',').filter(Boolean); cx.speed.cbds = cbds(P, ids); Store.emit('meta'); }
    else if (act === 'triggers') {
      const g = id => (document.getElementById && document.getElementById(id)) || {}, to = cx.trigOpts || {};
      const opts = { scId: to.scId || undefined, upgrades: Array.isArray(to.upgrades) ? to.upgrades : undefined, anop: !!to.anop,
        step: +g('cx-step').value || STD.trigger.step, dos: isFinite(parseFloat(g('cx-dos').value)) ? +g('cx-dos').value : STD.trigger.dos, los: g('cx-los').value || STD.trigger.los, queue: g('cx-queue').checked !== false,
        minStorage: isFinite(parseFloat(g('cx-minst').value)) ? +g('cx-minst').value : STD.trigger.minStorage, onProgress: (f, t) => Kit.busy(`Finding triggers… ${Math.round(f * 100)}% (${t})`) };
      const r = await busy('Finding triggers…', () => triggerFinder(P, opts)); if (r) { Store.emit('meta'); Kit.toast(r.rows[0] ? r.rows[0].text : 'Done.'); }
    }
    else if (act === 'fund-shares') { const r = await busy('Running select-link shares…', () => fundingShares(P, { onProgress: (f, t) => Kit.busy(`Select-link shares… ${Math.round(f * 100)}% (${t})`) })); if (r) { Store.emit('meta'); if (!r.ok) Kit.toast(r.why, 'warn'); } }
    else if (act === 'designlife') { const r = await busy('Running the design-life test…', () => runDesignLife(P)); if (r) { Store.emit('meta'); Kit.toast(`Design-life run done (+${r.pct}% demand).`); } }
    else if (act === 'tou') { const r = await busy('Running the time-of-use screening…', () => touRun(P)); if (r) { Store.emit('meta'); if (!r.ok) Kit.toast(r.why, 'warn'); } }
    rerender();
    return true;
  }

  /* =====================================================================================
     22. Inspector sections (road and zone), report sections, exports, settings, checklist
     ===================================================================================== */
  function linkSectionHtml(sc, sel_) {
    const L = sc.links[sel_.id]; if (!L || !Store.P) return '';
    const x = L.cx || {}, b = (k, t) => `${Kit.bind('link', L.id, 'cx.' + k, t)} data-emit="meta"`;
    const clsOpts = { '': 'Not set', ...Object.fromEntries(Object.entries(STD.speed.classes).map(([k, v]) => [k, `Table ${v.t}: ${v.label} (${v.min === v.max ? v.min : `${v.min}–${v.max}`})`])) };
    const row = speedCheck(Store.P, sc).find(r => r.ids.includes(L.id));
    return Kit.sec('Speed limit and street role', `
      ${Kit.fld('Rule class', `<select ${b('ruleClass', 'str')}>${Kit.opt(clsOpts, x.ruleClass || '')}</select>`)}
      ${Kit.fld('Proposed limit', `<input type="number" step="10" ${b('limit', 'numnull')} value="${esc(x.limit ?? '')}">`, 'km/h')}
      ${Kit.fld('Section name', `<input type="text" ${b('section', 'str')} value="${esc(x.section || '')}" placeholder="joins links into one section">`)}
      ${Kit.fld('Variable limit reason', `<select ${b('varReason', 'str')}>${Kit.opt({ '': x.variable ? 'Not given' : '–', ...STD.speed.varReasons }, x.varReason || '')}</select>`)}
      ${Kit.fld('Schedule 1 exception', `<select ${b('exception', 'str')}>${Kit.opt({ '': 'None', ...STD.speed.exceptions }, x.exception || '')}</select>`)}
      <label class="check"><input type="checkbox" ${b('variable', 'bool')} ${x.variable ? 'checked' : ''}> Variable limit</label>
      <label class="check"><input type="checkbox" ${b('newRoad', 'bool')} ${x.newRoad ? 'checked' : ''}> New road (no CBDS)</label>
      <label class="check"><input type="checkbox" ${b('school', 'bool')} ${x.school ? 'checked' : ''}> School-gate limit (no CBDS)</label>
      ${Kit.fld('Measured mean speed', `<input type="number" step="0.1" ${b('meanSpeed', 'numnull')} value="${esc(x.meanSpeed ?? '')}">`, 'km/h')}
      ${Kit.fld('Mean speed source', `<input type="text" ${b('meanSpeedSrc', 'str')} value="${esc(x.meanSpeedSrc || '')}" placeholder="survey, date">`)}
      ${Kit.fld('Current limit or road since', `<input type="number" step="1" ${b('since', 'numnull')} value="${esc(x.since ?? '')}" placeholder="year, if within the CAS window">`, 'year')}
      ${Kit.fld('RASF place / movement', `<input type="text" ${b('rasf', 'str')} value="${esc(x.rasf || '')}" placeholder="e.g. P3 M2 (advisory)">`)}
      ${Kit.fld('ONF category', `<input type="text" ${b('onf', 'str')} value="${esc(x.onf || '')}">`)}
      ${row ? (row.flags.length ? row.flags.map(f => Kit.note(`${esc(f.text)} (${esc(f.rule)})`, true)).join('') : Kit.note(`Section ${F.int(row.len)} m at ${row.limit} km/h: no flags.`)) : ''}${row && row.onf ? Kit.note(esc(row.onf)) : ''}`);
  }
  function zoneSectionHtml(sc, sel_) {
    const z = sc.zones[sel_.id]; if (!z || z.kind === 'gateway' || !Store.P) return '';
    const x = z.cx || {}, main = ['residential', 'apartment', 'retail', 'office', 'industrial'].filter(k => LANDUSES[k]), more = Object.keys(LANDUSES).filter(k => !main.includes(k));
    const f = (grp, k) => Kit.fld(`${esc(LANDUSES[k].label)}`, `<input type="number" min="0" ${Kit.bind('zone', z.id, `cx.${grp}.${k}`, 'numnull')} data-emit="meta" value="${esc((x[grp] || {})[k] ?? '')}">`, esc(LANDUSES[k].unit));
    const set = grp => `${main.map(k => f(grp, k)).join('')}<details><summary>Other land uses</summary>${more.map(k => f(grp, k)).join('')}</details>`;
    return Kit.sec('Zoned capacity (plan change)', `${set('zoned')}<details><summary>PC120 proposed capacity</summary>${set('zonedProposed')}</details>
      ${Kit.fld('Income proxy', `<input type="number" min="0" ${Kit.bind('zone', z.id, 'cx.income', 'numnull')} data-emit="meta" value="${esc(x.income ?? '')}">`, 'NZ$ median household')}
      ${Kit.note('Full build-out (Compliance+ › Plan change): an entered capacity replaces that land use; entered dwelling capacity replaces the zone\'s dwellings of every type; land uses left blank keep their current quantity. The income proxy is used for time-of-use incidence.')}`);
  }
  const rT = (ctx, title, tblHtml, noteText = '') => (ctx && typeof ctx.table === 'function' ? ctx.table(title, tblHtml, { note: noteText }) : `<figure class="tbl"><figcaption>${title}</figcaption>${tblHtml}${noteText ? `<p class="src">${noteText}</p>` : ''}</figure>`);   // the report sets the section heading
  const REPORT = [
    { id: 'compliance-extra-parking', chapter: 'parking', order: 35, title: 'Parking, cycle parking and loading', render: (P, c) => { const pk = parking(P); if (!pk.rows.length) return '';
      return rT(c, 'Table P1 · Parking, cycle parking, end-of-trip and loading', T({ head: ['Activity', 'Quantity', 'AUP maximum', 'Proposed', 'Accessible required (indicative)', 'Cycle visitor req/prop', 'Cycle secure req/prop', 'Loading req/prop', 'Complies'],
        rows: pk.rows.map(r => [esc(r.label), `${num(r.qty)} ${esc(r.unit)}`, num(r.max), num(r.cars), r.accReq.required ? `Yes (${num(r.accReq.n)})` : 'No', `${r.cyc.visitor != null ? num(r.cyc.visitor, 1) : '–'}/${num(r.cycleVisitor)}`, `${r.cyc.secure != null ? num(r.cyc.secure, 1) : '–'}/${num(r.cycleSecure)}`, `${num(r.loadReq)}/${num(r.loading)}`, r.complies ? 'Yes' : 'No'])
          .concat([['<b>Total</b>', '', '', '', '', `${num(pk.need.visitor)}/${num(pk.have.visitor)}`, `${num(pk.need.secure)}/${num(pk.have.secure)}`, `${num(pk.need.loading)}/${num(pk.have.loading)}`, pk.allComply ? 'Yes' : 'No']]) }),
        `${esc(pk.label)} Row cycle figures are unrounded; the totals are summed and rounded. Showers: ${num(pk.need.showers)} required, ${num(pk.have.showers)} proposed. ${esc(pk.totalsWhy.join('; '))} ${pk.rows.filter(r => r.induced).map(r => esc(r.induced.text)).join(' ')} Accessible numbers: ${esc(STD.accessible.name)}; verify.`); } },
    { id: 'compliance-extra-precinct', chapter: 'staging and sequencing', order: 45, title: 'Precinct staging', render: (P, c) => { const x = precinctCheck(P); if (!x) return '';
      return `<p class="note"><b>${esc(x.banner)}</b> ${esc(x.pc120Note)}.</p>${rT(c, `${esc(x.precinct.table)} staging against approved and proposed development`, T({ head: ['Row', 'Dwellings cap', 'Prerequisite upgrades', 'Status'], rows: x.rows.map(r => [`(${esc(r.row)})`, num(r.dw), r.up.map(u => `${esc(u.name)} (${esc(u.stateLabel)})`).join('; '), r.met ? 'Deemed met' : 'Not met']) }),
        `Total ${F.int(x.total.dw)} dwellings (${F.int(x.cumulative.dw)} approved + ${F.int(x.proposal.dw)} proposed). ${x.breach ? esc(`Breach of row (${x.row.row}): ${x.why.join('; ')}. ${x.statusText}.`) : 'Within the applicable row.'} Inter-peak LoS at ${esc(x.ip.name)}: ${esc(x.ip.los || 'not available')} (${esc(x.ip.source)}).`)}`; } },
    { id: 'compliance-extra-subdivision', chapter: 'land use', order: 36, title: 'Urban subdivision (E38)', render: P => { const s = subdivision(P); if (!(s.lots > 0) && !(s.accesses || []).length) return '';
      return `<p class="note">${esc(s.warning)}</p>${s.e27Text ? `<p class="note">${esc(s.e27Text)}.</p>` : ''}${T({ head: ['Rear access', 'Sites', 'Check'], rows: s.accesses.map(a => [esc(a.name || '–'), num(a.sites), a.ok ? 'Complies' : esc(a.flags.join('; '))]), empty: 'No rear accesses.' })}<p class="note">Connectivity: intersection density ${s.connectivity.density != null ? num(s.connectivity.density, 0) + ' per km²' : '–'}, route directness ${s.connectivity.directness != null ? num(s.connectivity.directness, 2) : '–'}.</p>`; } },
    { id: 'compliance-extra-anop', chapter: 'anop', order: 35, title: 'ANOP multimodal fit (indicative)', render: (P, c) => { const a = anop(P); if (!a.rows.length) return '';
      return `${rT(c, 'Table ANOP-1 · Multimodal level of service and fit', anopTable(a), `${esc(a.label)} Speeds per direction; the worse direction sets the LoS.${a.same ? ' ' + esc(a.note) + '.' : ''}`)}${disc('D5', P)}`; } },
    { id: 'compliance-extra-policy', chapter: 'policy alignment', order: 72, title: 'Policy alignment', render: (P, c) => { const p = policy(P); if (!p.rows.length && !periodsOf(pair(P).opt).length) return '';
      return `${rT(c, 'Policy alignment', T({ head: ['Reference', 'Policy', 'Spare Construct evidence'], rows: p.policy.map(r => [esc(r.ref), esc(r.policy), esc(r.text)]) }), esc(STD.labels.vkt))}${p.inconsistencies ? rT(c, 'Inconsistencies with Future Connect, ANOP, RASF and FDS', T({ head: ['Road', 'Inconsistency'], rows: p.rows.filter(r => r.flags.length).map(r => [esc(r.name), esc(r.flags.join('; '))]) })) : ''}`; } },
    { id: 'compliance-extra-climate', chapter: 'climate adaptation', order: 75, title: 'Climate adaptation', render: (P, c) => { const x = climate(P); if (!x.loaded) return '';
      return rT(c, 'Assessed roads in flood plains and coastal inundation areas', T({ head: ['Link', 'FC role', 'Length in hazard (m)', 'Hazard layer', 'Only access', 'Assessment'], rows: x.rows.map(r => [esc(r.name), esc(r.fc), F.int(r.len), esc(r.layer), r.only ? 'Yes' : 'No', esc(r.assess)]), empty: 'No assessed road lies in a mapped hazard area.' }), `${esc(x.note)} Source: ${esc(x.source)}.`); } },
    { id: 'compliance-extra-alternatives', chapter: 'assessment', order: 85, title: 'Options and alternatives', render: (P, c) => { const a = alternatives(P); if (!a.rows.length) return '';
      return `${rT(c, 'Options against the intervention hierarchy', altTable(a), `Values per modelled peak hour, each period on its own. ${esc(HIER_SRC)}. VKT and CO2e: ${esc(STD.labels.vkt)}. Junction results: ${esc(STD.labels.junction)}.`)}${disc('D3', P)}${a.warnings.map(w => `<p class="note warn">${esc(w)}</p>`).join('')}${a.sections.filter(s => s.include).map(s => `<p class="note">${esc(s.title)} (${esc(s.ref)}): ${esc(s.why)}</p>`).join('')}`; } },
    { id: 'compliance-extra-mitigation', chapter: 'mitigation', order: 12, title: 'Mitigation register and condition parameters', render: (P, c) => { const m = mitigationTable(P), cd = conditionTable(P); if (!m.length && !cd.length) return '';
      return `${rT(c, 'Mitigation register (AT ITA Appendix 3)', T({ head: ['Ref', 'Measure', 'Effect addressed', 'Options considered', 'Trigger', 'Responsible', 'Cost NZ$m', 'Funder', 'RLTP/LTP', 'Mechanism', 'Committed', 'Land outside control'], rows: m.map(r => [esc(r.ref), esc(r.measure) + (r.tag ? `<br><i>${esc(r.tag)}</i>` : ''), esc(r.effectText), esc(r.options), esc(r.trigger), esc(r.responsibleText), r.cost != null ? num(r.cost, 2) : '–', esc(RESPONSIBLE[r.funder] || r.funder || ''), esc(r.rltp), esc(r.mechanismText) + (r.s84a ? ` (${esc(r.s84a)})` : ''), committedText(r.committed), r.landOutside ? 'Y' : 'N']) }), esc(EFFECT.basis))}
        ${cd.length ? rT(c, 'Condition parameters (not condition wording)', T({ head: ['Ref', 'Measure', 'Trigger metric', 'Threshold', 'Survey', 'Frequency', 'Response', 'Nexus', 'Review'], rows: cd.map(r => [esc(r.ref), esc(r.measure), esc(r.metric), esc(r.threshold), esc(r.survey), esc(r.frequency), esc(r.response), esc(r.basis) + (r.ftaa ? `; ${esc(r.ftaa)}` : ''), esc(r.review)]) })) : ''}`; } },
    { id: 'compliance-extra-triggers', chapter: 'staging triggers', order: 22, title: 'Trigger finder', render: (P, c) => { const t = data(P).triggers; if (!t) return '';
      return `${rT(c, `Build-out at which the criteria first fail (${esc(t.scName)}, ${esc(t.periods.join(' and '))})`, triggerTables(t), `${esc(t.basis)}. ${esc(t.label)}. ${esc(t.copy || '')}`)}${disc('D3', P)}`; } },
    { id: 'compliance-extra-planchange', chapter: 'plan change', order: 25, title: 'Plan change: process, s32 and full build-out', render: (P, c) => { const pc = planChange(P); if (!pc.kind) return '';
      return `${pc.process.map(p => `<p class="note${p.level === 'warn' ? ' warn' : ''}">${esc(p.text)}</p>`).join('')}${rT(c, 's32 transport evaluation', T({ head: ['Option', 'Travel time', 'Safety', 'VOC', 'Cost and timing', 'Risk of acting', 'Risk of not acting'], rows: pc.s32.map(r => [esc(r.name), esc(r.travel), esc(r.safety), esc(r.voc), esc(r.cost), esc(r.acting), esc(r.notActing)]), empty: 'Run the options.' }), `<b>${esc(pc.s32Label)}</b>`)}
        ${rT(c, 'Policy matters', T({ head: ['Reference', 'Policy', 'Evidence'], rows: pc.policy.map(r => [esc(r.ref), esc(r.policy), esc(r.text)]) }))}<p class="note">Full build-out: ${F.int(pc.buildout.dwellings)} dwellings, ${F.int(pc.buildout.gfa)} m² GFA (${esc(pc.buildout.label)}; ${esc(pc.buildout.rule)}).</p>`; } },
    { id: 'compliance-extra-ftaa', chapter: 'fast-track', order: 30, title: 'Fast-track package', render: (P, c) => { const f = ftaaPackage(P); if (!f.ftaa) return '';
      return `${rT(c, 's85(3) regional benefits and residual impacts', T({ head: ['Side', 'Item', 'Spare Construct estimate'], rows: [...f.s85.benefits.map(([k, v]) => ['Benefit', esc(k), esc(v)]), ...f.s85.impacts.map(([k, v]) => ['Residual impact', esc(k), esc(v)])] }), esc(f.s85.note))}${disc('D3', P)}
        ${rT(c, 'Compliance matrix (FTAA Sch 5 cl 5(1)(h), (3))', T({ head: ['Rule', 'Requirement', 'Result', 'Evidence'], rows: f.matrix.map(r => [esc(r.rule), esc(r.req), esc(r.result), esc(r.evidence)]) }))}
        ${rT(c, 's53 invitees', T({ head: ['Invitee', 'Basis'], rows: f.invitees.map(i => [esc(i.who), esc(i.basis)]) }))}${f.notes.map(n => `<p class="note">${esc(n)}</p>`).join('')}`; } },
    { id: 'compliance-extra-speed', chapter: 'speed limit', order: 40, title: 'Speed limits and CBDS', render: (P, c) => { const rows = speedCheck(P), cb = data(P).speed.cbds; if (!rows.length && !(cb && cb.ok)) return '';
      return `${rows.length ? rT(c, 'Speed limit validation', T({ head: ['Road', 'Rule class', 'Limit', 'Section (m)', 'Flags'], rows: rows.map(r => [esc(r.name), esc(r.classLabel), `${num(r.current)} → ${num(r.limit)}`, F.int(r.len), r.flags.length ? esc(r.flags.map(f => f.text).join(' ')) : 'None']) }), 'A divided road counts once in the section length.') : ''}${cb && cb.ok ? cbdsHtml(cb) : ''}`; } },
    { id: 'compliance-extra-gps', chapter: 'funding', order: 50, title: 'IPM stage 2 GPS metrics', render: P => { const c = context(P); if (c.kind !== 'fund') return ''; const g = gpsMetrics(P);
      return `<p class="note"><b>${esc(g.label)}</b></p>${T({ head: ['Metric', 'Value', 'Draft rating'], rows: [['Scheme network travel-time saving', g.network.saving != null ? `${num(g.network.saving, 1)}% (${esc(g.network.significance)})` : '–', `HCV ${esc(g.network.hcv || '–')}; general traffic ${esc(g.network.gt || '–')}`], ['Houses enabled (75% by 2035)', F.int(g.houses.total), ''], ['DSI reduction per NZ$100m', g.dsi.per100m != null ? num(g.dsi.per100m, 2) : '–', esc(g.dsi.collective)]] })}<p class="note">${esc(g.network.note)}</p>`; } },
    { id: 'compliance-extra-funding', chapter: 'growth funding', order: 60, title: 'Funding attribution', render: P => { const c = context(P); if (c.kind !== 'dc') return ''; const f = funding(P);
      return `<p class="note"><b>${esc(f.caveat)}</b></p><p>Growth share ${f.growth.share != null ? F.pct(f.growth.share, 1) : '–'} (${esc(f.growth.formula)}; ${esc(f.growth.label)}). ${f.incidence && f.incidence.insideShare != null ? `Benefit inside the levy area ${F.pct(f.incidence.insideShare)}.` : ''} BCR_G with the levy as Cp: ${esc(f.bcrG.text)}.</p>${f.shares ? T({ head: ['Asset', 'Period', 'Development area', 'Share of the asset flow'], rows: f.shares.assets.flatMap(a => a.areas.slice(0, 5).map(z => [esc(a.name), esc(a.period), esc(z.name), F.pct(z.share)])) }) + `<p class="note">${esc(f.shares.method)}.</p>` : ''}`; } },
    { id: 'compliance-extra-tou', chapter: 'time-of-use', order: 70, title: 'Time-of-use screening', render: (P, c) => { const r = data(P).tou.result; if (!r || !r.ok) return '';
      return `<p class="note"><b>${esc(r.label)}</b></p>${rT(c, 'Sampled trips before and after the charge (s65D(3))', T({ head: ['From', 'To', 'Trips before', 'Trips after', 'Time before (min)', 'Time after (min)', 'Charge (NZ$)'], rows: r.sample.map(o => [esc(o.from), esc(o.to), num(o.q0, 1), num(o.q1, 1), num(o.t0 / 60, 1), num(o.t1 / 60, 1), o.paid ? num(o.pay, 2) : '–']) }), esc(r.method))}`; } },
    { id: 'compliance-extra-anop-inputs', chapter: 'appendix e', order: 20, title: 'ANOP tool inputs (anop.csv)', render: P => { const a = anop(P); if (!a.rows.length) return '';
      return `<p>The ANOP tool inputs are in <code>anop.csv</code> (Export › Compliance+: ANOP tool inputs), in the v10.2 input sheet column order: ${a.rows.length} corridor-period rows (the worse direction). Per-direction speeds are in the ANOP speeds export.</p>${disc('D5', P)}`; } },
  ];
  const EXPORTS = [
    ['lint', 'Compliance+: scope linter findings (CSV)', 'csv', lintCSV, () => true],
    ['precinct', 'Compliance+: precinct staging (CSV)', 'csv', precinctCSV, () => !!precinctOf(Store.P)],
    ['mitigation', 'Compliance+: mitigation register and conditions (CSV)', 'csv', mitigationCSV, () => data(Store.P).mitigation.length > 0 || data(Store.P).conditions.length > 0],
    ['options', 'Compliance+: options appraisal, EAST/AST-ready (CSV)', 'csv', optionsCSV, () => alternatives(Store.P).rows.length > 0],
    ['ftaa', 'Compliance+: Fast-track package (ZIP)', 'zip', ftaaZip, () => true],
    ['planchange', 'Compliance+: plan change s32 pack (CSV)', 'csv', planChangeCSV, () => true],
    ['speed', 'Compliance+: speed limit validation and CBDS (CSV)', 'csv', speedCSV, () => speedCheck(Store.P).length > 0 || !!(data(Store.P).speed.cbds)],
    ['anop', 'Compliance+: ANOP tool inputs, anop.csv', 'csv', P => stamped(anopCSV(P), P, 'the AT ANOP tool v10.2'), () => anop(Store.P).rows.length > 0],
    ['anop-inputs', 'Compliance+: ANOP speeds and reliability (CSV)', 'csv', anopInputsCSV, () => resOK(Store.P)],
    ['parking', 'Compliance+: parking Table P1 (CSV)', 'csv', parkingCSV, () => data(Store.P).parking.rows.length > 0],
    ['policy', 'Compliance+: policy, GPS metrics and funding (CSV)', 'csv', policyCSV, () => resOK(Store.P)],
    ['tou', 'Compliance+: time-of-use screening (CSV)', 'csv', touCSV, () => !!(data(Store.P).tou.result && data(Store.P).tou.result.ok)],
    ['climate', 'Compliance+: climate hazard table (CSV)', 'csv', climateCSV, () => climate(Store.P).loaded],
    ['tsdg', 'Hand-over: ATOC TSDG signal pack (CSV)', 'csv', tsdgPack, () => resOK(Store.P)],
    ['ssa', 'Hand-over: Safe System Audit briefing (CSV)', 'csv', ssaPack, () => resOK(Store.P)],
    ['sh', 'Hand-over: NZTA state highway impact pack (CSV)', 'csv', shPack, () => resOK(Store.P)],
    ['mbcm', 'Hand-over: MBCM checks, SP and IER fields (CSV)', 'csv', mbcmPack, () => resOK(Store.P)],
    ['rltp', 'Hand-over: RLTP activity sheet (CSV)', 'csv', rltpSheet, () => true],
    ['msm', 'Hand-over: MSM / AFC request pack (CSV)', 'csv', msmPack, () => true],
  ];
  /* Outputs that carry author text are refused while the linter has a blocking finding (reg-scope-lint): the ITA report and
     data pack (the report lane's exports and ☰ Menu downloads), the Fast-track package and the plan change s32 pack. Data
     exports (GeoJSON, link CSV, OD matrix, SIDRA volumes …) hold no author text and are not blocked. Another lane opts an
     export or menu item in with `authorText: true`. */
  const GUARD = { exports: ['report-html', 'report-datapack', 'compliance-extra-ftaa', 'compliance-extra-planchange'], menu: ['report-download', 'report-pack'] };
  // this file's own exports that carry author-written text (conditions, mitigation descriptions): opted in with authorText: true
  const AUTHOR_TEXT = ['mitigation'];
  const guarded = new WeakSet();
  function guardExports() {
    for (const d of Registry.list('exports')) {
      if (guarded.has(d) || !(GUARD.exports.includes(d.id) || d.authorText === true)) continue;
      const build = d.build;
      d.build = (P, c) => { const why = exportBlock(P || Store.P); if (why) throw new Error(why); return build(P, c); };
      guarded.add(d);
    }
    for (const d of Registry.list('menu')) {
      if (guarded.has(d) || !(GUARD.menu.includes(d.id) || d.authorText === true)) continue;
      const run = d.run;
      d.run = (...a) => { const why = exportBlock(Store.P); if (why) { try { Kit.toast(why, 'err'); } catch { } return undefined; } return run(...a); };
      guarded.add(d);
    }
  }
  function register() {
    Registry.tabs({ id: 'compliance-extra', label: 'Compliance+', order: 86, title: 'Scope linter, precinct staging, mitigation and conditions, options, Fast-track, plan change, speed limits, ANOP, parking, subdivision, policy and hand-over packs',
      render: tabHtml, onClick, onChange });
    Registry.inspectorSections({ id: 'compliance-extra-link', types: ['link'], order: 85, render: linkSectionHtml });
    Registry.inspectorSections({ id: 'compliance-extra-zone', types: ['zone'], order: 85, render: zoneSectionHtml });
    for (const r of REPORT) Registry.report(r);
    // the Fast-track ZIP falls back to the JSON bundle when the report module (Report.zip) is not loaded: name it to match
    const extOf = (id, ext) => (ext === 'zip' && !hasZip() ? 'json' : ext);
    for (const [id, label, ext, build, enabled] of EXPORTS) Registry.exports({ id: 'compliance-extra-' + id, label, ext, ...(ext === 'csv' ? { mime: 'text/csv' } : {}), order: 120 + EXPORTS.findIndex(x => x[0] === id),
      enabled: () => { try { return !!Store.P && enabled(); } catch { return false; } }, filename: c => `${c.fileBase}-${id === 'anop' ? 'anop' : 'cx-' + id}.${extOf(id, ext)}`, build: P => build(P || Store.P), _stamped: true, ...(AUTHOR_TEXT.includes(id) ? { authorText: true } : {}) });
    Registry.settings({ id: 'compliance-extra', title: 'Compliance+ assumptions', order: 62, defaults: () => ({ complianceExtra: { ...ASSUME } }),
      render: s => `<div class="set-grid">${Object.entries(SET_LABELS).map(([k, [label, unit]]) => `<label class="fld"><span>${label}</span><span class="fld-in"><input type="number" step="any" data-set="complianceExtra.${k}" value="${esc(((s.complianceExtra || {})[k]) ?? ASSUME[k])}"><span class="unit">${unit} · ${ASSUMPTION}</span></span></label>`).join('')}</div><p class="note">${esc(D.D13)}</p>` });
    Registry.about({ id: 'compliance-extra', title: 'Compliance+ rules', order: 72, intro: `Rules are stored as dated data (as at ${longDate(VERSION)}). Values marked 'verify' have not yet been checked against the source document.`,
      rows: () => [
        ['Precinct staging (I450, I451)', link(SRC.i450, 'AUP I450 Drury Centre') + ' · ' + link(SRC.i451, 'AUP I451 Drury East'), `<b>Rules as data</b>, ${esc(PRECINCTS.I450.edition)}; verify. ${esc(STD.labels.pc120)}.`],
        ['Speed limits', link(SRC.speed, 'Setting of Speed Limits Rule 2024'), `Schedules 1 and 3, cl 4.3-4.9 (${esc(STD.speed.edition)}); verify.`],
        ['ANOP', link(SRC.anopTool, 'AT ANOP Assessment Tool v10.2'), esc(STD.labels.anop)],
        ['Parking, cycle and loading', link(SRC.e27, 'AUP E27'), 'Tables E27.6.2.5-E27.6.2.7, E27.6.3.2A (July 2026 text); verify. Maxima are entered.'],
        ['Rear site access', link(SRC.e38, 'AUP E38'), 'Table E38.8.1.2.1; verify.'],
        ['GPS ratings', link(SRC.ipm27, 'Initial draft IPM 2027-30'), esc(STD.labels.gps)],
        ['Time-of-use', link(SRC.tou, 'LTMA (Time of Use Charging) Amendment Act 2025'), `In force 18 Nov 2026 (ss65A-65ZO). ${esc(STD.tou.appraisal)}.`],
        ['Discounting and MBCM version', `${link(SRC.gc2501, 'NZTA GC 25/01')} · ${link(SRC.gc2601, 'NZTA GC 26/01')}`, 'MBCM v1.7.5 for BCRs commenced on or after 29 May 2026; 2% to year 30, then 1.5%; 8% sensitivity.'],
        ['AUP modifications', link(SRC.pc120, 'AUP plan modification schedule 2026'), `${esc(REFS.aupMods.pc79)}. ${esc(REFS.aupMods.pc120)}.`],
        ['CBDS crash forecast', link(SRC.nilsson, 'Nilsson (2004) power model'), `Fatal and serious crashes ∝ v^${ASSUME.dsiExponent}, injury crashes ∝ v^${ASSUME.injuryExponent} (defaults; ${ASSUMPTION}).`],
      ] });
    Registry.checklist({ id: 'compliance-extra-lint', group: 'Regulatory', order: 70, label: 'Scope and wording linter', source: 'RMA s104(1A)-(1C); LGACA s47',
      status: P => { const L = lint(P); return { state: L.blocking.length ? 'warn' : L.findings.length ? 'warn' : 'done', text: L.blocking.length ? `Exports blocked: ${esc(L.blocking[0].rule)}` : L.findings.length ? `${L.findings.length} wording finding${L.findings.length === 1 ? '' : 's'} to check` : 'No wording findings', action: { label: 'Open', run: () => { ui.sec = 'lint'; UI.openTab('compliance-extra'); } } }; } });
    Registry.checklist({ id: 'compliance-extra-precinct', group: 'Regulatory', order: 71, label: 'Precinct staging', source: 'AUP I450.6.2, I451.6.2',
      status: P => { const c = precinctCheck(P); return c ? { state: c.breach ? 'warn' : 'done', text: c.breach ? `${esc(c.precinct.id)}: breach of row (${esc(c.row.row)}), ${esc(c.statusText)}` : `${esc(c.precinct.id)}: within the caps`, action: { label: 'Open', run: () => { ui.sec = 'precinct'; UI.openTab('compliance-extra'); } } } : { state: 'na', text: 'No precinct staging standard applies (none selected)' }; } });
    Registry.checklist({ id: 'compliance-extra-conditions', group: 'Mitigation', order: 72, label: 'Condition parameters have a nexus', source: 'RMA s108AA(1)',
      status: P => { const c = conditionTable(P); const bad = c.filter(x => !x.ok).length; return { state: !c.length ? 'todo' : bad ? 'warn' : 'done', text: !c.length ? 'No condition parameters yet' : bad ? `${bad} row${bad === 1 ? '' : 's'} without an effect or agreement` : 'Every row has a nexus or agreement', action: { label: 'Open', run: () => { ui.sec = 'mitigation'; UI.openTab('compliance-extra'); } } }; } });
    Registry.checklist({ id: 'compliance-extra-options', group: 'Assessment', order: 73, label: 'Intervention hierarchy options', source: 'MBCM s1.5',
      status: P => { const a = alternatives(P); return { state: !a.rows.length ? 'na' : a.warnings.length ? 'warn' : 'done', text: !a.rows.length ? 'No options to compare' : a.warnings.length ? esc(a.warnings[0]) : 'Options cover the hierarchy', action: { label: 'Open', run: () => { ui.sec = 'options'; UI.openTab('compliance-extra'); } } }; } });
    Registry.onInit(guardExports);
    Registry.onInit(() => { if (typeof UI !== 'undefined') setTimeout(guardExports, 0); });
  }
  register();

  return {
    VERSION, SRC, STD, REFS, D, ASSUME, LINT, GUARD, PRECINCTS, HIER, PARK_ACT, MODES, PREF, LU,
    data, context, stamp, disclaimer, authorityOf, addWorkingDays, assumptions: A,
    lint, overrideLint, exportBlock, precinctList, precinctOf, precinctCheck, mixTrips,
    effects, mitigationTable, networkDiff, seedMitigation, conditionTable, seedConditions, triggerFinder, failsAt, storageOf,
    optionList, alternatives, alternativeSections, perPeriod, mergeZoned, fullBuildout, createFullBuildout, planChange, invitees, ftaaTimeline, ftaaPackage, ftaaFiles, ftaaBundle, ftaaZip,
    speedSections, speedCheck, roadLen, schoolSpeeds, sectionCrashes, crashHistory, cbds, minLenFor,
    anopPref, anopFit, anopLoS, anopCorridors, anop, anopCSV, corridorSpeed, corridorAxis, hierOf,
    parking, cycleRates, showersFor, loadingFor, accessibleFor, subdivision, rearAccess,
    skim, policy, accessibility, gpsRoutes, gpsMetrics, freightRating, gtRating, growthShare, fundingAssets, fundingShares, funding, inPoly, touRun, touCharged, touSkim, climate,
    runDesignLife, tsdgPack, anopInputsCSV, ssaPack, shPack, mbcmPack, rltpSheet, msmPack, handoverManifest,
    lintCSV, precinctCSV, mitigationCSV, optionsCSV, planChangeCSV, speedCSV, parkingCSV, policyCSV, touCSV, climateCSV,
    tabHtml, sectionHtml, linkSectionHtml, zoneSectionHtml, REPORT, guardExports, onClick, onChange, ui,
  };
})();
