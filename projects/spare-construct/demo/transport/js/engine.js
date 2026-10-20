'use strict';
/* Spare Construct · transport model engine.
   Network graph → trip generation → gravity distribution with gateway through-traffic →
   user-equilibrium assignment (Frank-Wolfe) with BPR link times and HCM-style junction delay
   (Webster/Akçelik at signals, gap acceptance at priority junctions, HCM 6 roundabout entries),
   then junction LoS, KPIs, select-link analysis, isochrones and indicative economics.
   Routing is edge-based (contract C1): labels sit on arcs, so turns carry penalties and bans.
   C1 notes: turnPenalty() returns seconds and ignores bans (turnBanned() reports those). U-turns are
   reversals onto the same link and hairpins (> 150°) at junctions (turning round a median end); a
   numeric settings.turns.uturn allows them everywhere at that penalty. Turn penalties count in VHT,
   delay and bus route times. res.turns is accumulated from the assignment's turn flows (equal to the
   sum over stored paths), lists only movements with traffic, and includes zone connector movements
   (keys c:…), so it also covers development accesses on 2-link nodes and balances approach volumes.
   Assignment is conjugate Frank-Wolfe (the junction capacities make plain Frank-Wolfe stall).
   C2 notes: a zone's opening year applies to gateways too (unopened gateways take no part in through
   traffic); the reduction applies to all of a land-use zone's trips (rates and extra); a manual OD
   matrix obeys both; matrix estimation factors also apply to a manual matrix, and res.O / res.D are
   the factored trip ends (res.O0 / res.D0 the prior ones). econ() refuses scenarios of different years.
   v2 corrections (docs/spec calc-corrections, convergence, junction-los-corrections, economics-mbcm):
   - Background growth is linear (MBCM arithmetic growth) unless settings.growthMode is 'compound'.
   - Convergence: stop when the relative gap is under gapTol AND the link-flow stability P stays above
     98% for 4 iterations (settings.conv.stable); give-way and roundabout capacities are damped by MSA
     (settings.conv.capDamp 'msa'); results carry conv[], converged, hitMax, convNote and noise95.
   - Junctions: peak flow factor and analysis period T (settings.junction) in the delay and DoS; an
     approach over capacity (DoS > 1) is LoS F whatever its delay; roundabout delay is volume-weighted;
     two-way sign control reports the worst movement, not an intersection LoS; approach capacity is veh/h
     (DoS stays a PCU ratio); back of queue from the HCM 2000 / Akçelik formulas.
   - Economics to MBCM v1.7.5 (settings.econ.method 'mbcm'): 2% to year 30 then 1.5%, 8% sensitivity,
     arithmetic benefit growth, per-period annualisation; labelled IER-equivalent, with IPM bands.
     Old projects keep 'uniform' (their saved discount rate, flat benefits) until the user adopts.
   Every threshold and factor lives in STANDARDS with its source, edition and a verified flag.
   Assessment and junction additions (docs/spec scenario-matrix, dev-tracking, extent-and-difference, sensitivity-pack,
   movement-capacity, junction-sites; docs/regulatory reg-model-status):
   - Every OD pair has a class (existing / committed / development, from zoneClass); class link flows, development
     turn flows and an optional selection (select-link, select-turn, select-zone) are averaged with the main flows,
     so they work for any number of zones. Results: cls, dev, turns[].dev, select.
   - sc.buildout (staging %) scales development zones; opts.settings / opts.demandFactor serve the sensitivity pack.
   - Junction sites (sc.sites): internal links run at free-flow time; signals and give-way sites have one controller
     over the arcs entering the site; results gain sites and siteTurns.
   - settings.junction.capMethod 'movement' (opt-in; 'arc' is the default for all projects because the movement
     method takes about 3–4× the run time on Drury): per-movement gap acceptance with the lane model, still charged to
     the entry arc; approach rows gain mov, lanes, capT and bay. Tried and rejected: movement delays on turns (the
     shared-lane costs are asymmetric and the assignment stopped converging).
   - Every result carries meta (model status, convergence, data vintages). Deviations: none of these change a
     saved project's results; detection of sites on import is opt-in (Importers.osmToScenario(sc, data, { sites })).
   Integration pass (Oct 2026, docs/reviews/*-fix.md "Needs"):
   - Junction-site movements are traced along the assignment's paths (entry arc → exit arc per OD path, averaged with
     the same steps as the link flows), so they follow the OD matrix: res.siteTurns rows carry dev, and
     res.siteTurnsBasis = 'paths'. The movement capacity method reads the traced flows too. Chaining turn shares node
     by node (siteExits, kept as a fallback for graphs without traced flows) invented U-turns on multi-node sites.
   - tripEnds: a zone with a switched-on trip-generation table (z.tg.on) loads z.tgLoad { AMin, AMout, PMin, PMout } in
     place of land use × rates + z.extra, then the reduction × staging and the opening year; sensitivity T1± scales
     z.tgLoad as well as the rates. Optional per-land-use factors (Store.P.luFactors[period] = { <land use>: f,
     gateways: f }) apply to existing-land-use zones and estimated gateways (no gwSrc) only, never to development.
   - Additive per-arc cost (s): L.toll (a number, or { AM, PM }) and opts.arcPenalty ({ <arc key or link id>: s }) for
     time-of-use charges. They steer route choice and the equilibrium (generalised cost) but are not time: res.arcs t,
     VHT, delay and bus times exclude them; res.arcs[k].toll and kpi.chargeH report them.
   - Bus preload: r.freqs[period] (else r.freq) × settings.busPcu (2.0 for new projects; projects saved before it
     existed keep 3, the earlier engine value). L.varSpeed ({ v, AM, PM }) caps the link speed in the peak it covers.
   - res.arcs gain capMid (mid-block pcu/h), capSrc ('mid-block' | 'signals' | 'give way' | 'stop' | 'roundabout') and
     capNode (the controlling junction, when a junction sets the capacity).
   - Economics follow NZTA General Circular 25/01 (effective 6 Jan 2025): non-commercial 2% years 1–30, 1.5% years
     31–100, 1% from year 101, sensitivity at 8%; commercial 8% for all years, sensitivity at 2%; every analysis
     (the 'uniform' method of older projects too) reports the sensitivity PV and BCR.
   - modelStatus is 'validated' only when the record's category and standard match the settings.
   - Matrix estimation in future years: when the fit recorded additive deltas (P.me.info[period].delta, veh/h) and the
     scenario year is not the fit's base year, T = max(0, T + delta) on the same pairs (docs/spec/matrix-estimation);
     older records without deltas keep the multiplicative factors. */

const Engine = (() => {
  /* ---------- standards as data (cross-cutting rule 4) ----------
     verified: true = checked against the cited document; false = taken from Spare Construct's research spec
     (docs/spec, Oct 2026) and still to be checked against the source before it is relied on. */
  const HCM_URL = 'https://www.txdot.gov/manuals/des/tsp/chapter-12-roundabout-analysis/12-2-motorized-vehicle-capacity-methodology/12-2-4-reporting-moes/12-2-4-4-level-of-service.html';
  const MBCM_URL = 'https://www.nzta.govt.nz/assets/resources/monetised-benefits-and-costs-manual/Monetised-benefits-and-costs-manual-volume-1-procedures.pdf';
  const TAG_URL = 'https://assets.publishing.service.gov.uk/media/6a033d074fb0713aa63ea802/tag-m3-1-highway-assignment-modelling.pdf';
  const STANDARDS = {
    los: {
      hcm6: { id: 'hcm6', name: 'HCM 6 control delay', edition: 'Highway Capacity Manual, 6th edition (2016)', url: HCM_URL, verified: true,
        signals: [10, 20, 35, 55, 80], roundabout: [10, 15, 25, 35, 50], sign: [10, 15, 25, 35, 50], overCapacityF: true,
        label: 'Level of service: HCM 6 control delay (signals A ≤ 10, B ≤ 20, C ≤ 35, D ≤ 55, E ≤ 80 s; roundabouts and give-way/stop A ≤ 10, B ≤ 15, C ≤ 25, D ≤ 35, E ≤ 50 s). An approach over capacity (DoS > 1) is F.' },
      sidra: { id: 'sidra', name: 'SIDRA Delay (roundabouts as signals)', edition: 'SIDRA Intersection LoS method "Delay (SIDRA)"', url: 'https://docs.sidrasolutions.com/intersection/intersection/sidra-glossary/', verified: false,
        signals: [10, 20, 35, 55, 80], roundabout: [10, 20, 35, 55, 80], sign: [10, 15, 25, 35, 50], overCapacityF: true,
        label: 'Level of service: SIDRA Delay method (roundabouts on the signals scale A ≤ 10 … E ≤ 80 s; give-way/stop A ≤ 10 … E ≤ 50 s). An approach over capacity (DoS > 1) is F.' },
      rta: { id: 'rta', name: 'RTA NSW delay bands (option only)', edition: 'RTA NSW Guide to Traffic Generating Developments (2002), Table 4.2', url: 'https://standards.transport.nsw.gov.au/', verified: false,
        signals: [14, 28, 42, 56, 70], roundabout: [14, 28, 42, 56, 70], sign: [14, 28, 42, 56, 70], overCapacityF: true,
        label: 'Level of service: RTA NSW delay bands (A < 14, B < 28, C < 42, D < 56, E < 70 s). An approach over capacity (DoS > 1) is F. Option only; HCM 6 is the default.' },
    },
    // Link V/C bands (map colouring only; not a junction LoS)
    linkVC: { id: 'corridor-vc', name: 'Link volume/capacity bands', edition: 'Spare Construct assumption', verified: false, cuts: [0.35, 0.55, 0.75, 0.9, 1.0] },
    practicalDoS: { id: 'sidra-xp', name: 'Practical degree of saturation', edition: 'SIDRA Intersection defaults', url: 'https://docs.sidrasolutions.com/intersection/intersection/sidra-glossary/', verified: false,
      signals: 0.90, roundabout: 0.85, sign: 0.80 },
    pff: { id: 'ttoc06-pff', name: 'Peak flow factor', edition: 'TTOC-06 Traffic Signals Modelling Guidelines; SIDRA default convention', url: 'https://www.qldc.govt.nz/media/lekcx4iu/ttoc-06-traffic-signals-modelling-guidelines.pdf', verified: false, value: 0.95, T: 0.5 },
    queue: { id: 'hcm2000-bq', name: 'Back of queue', edition: 'HCM 2000 Appendix G (signals, Akçelik form); HCM 6 eq. 20-68 (unsignalised)', url: 'https://www.txdot.gov/manuals/des/tsp/chapter-4-traffic-analysis-methodology--tools--and/4-5-moes/4-5-5-queue-length.html', verified: false,
      f95: 1.6, kB: 0.12, space: 7.0, note: 'The 95th-percentile factor of 1.6 is kept until it is checked against HCM 2000 Appendix G.' },
    conv: { id: 'tag-m31-conv', name: 'Assignment convergence', edition: 'UK TAG M3.1 (May 2026), Table 4', url: TAG_URL, verified: true,
      gap: 0.001, chg: 0.01, share: 0.98, consec: 4, gapLabel: '%GAP (TAG Delta / relative gap)',
      note: 'TAG base models: %GAP under 0.1% and P (links changing flow by under 1%) above 98% for 4 consecutive iterations. NZTA TMDG 2019 asks for convergence to be shown but sets no number.' },
    noise: { id: 'tag-m31-snr', name: 'Scenario signal to noise', edition: 'UK TAG M3.1 (May 2026)', url: TAG_URL, verified: false, min: 10,
      tighter: { gapTol: 1e-5, maxIter: 400 } },
    sidraGap: { id: 'corridor-sidra-gap', name: 'Gap needed for turn volume exports', edition: 'Spare Construct assumption', verified: false, max: 1e-4 },
    /* Discount rates: NZTA General Circular 25/01 'Update to the NZTA discount rates used for economic analysis of
       transport activities' (effective 6 January 2025; docs/references/25-01.pdf, checked against the text): "for
       non-commercial activities using discount rates of 2% for the first 30 years, 1.5% for years 31 to 100, and 1% for
       years 101 onwards"; commercial activities 8% for all years; sensitivity testing at the alternative rate (8% for
       non-commercial, 2% for commercial); "All transport economic analyses are to be sensitivity tested using an 8%
       discount rate and the results ... reported together with the other results". to: null = onwards. */
    mbcm: { id: 'mbcm-1.7.5', name: 'NZTA Monetised Benefits and Costs Manual', edition: 'v1.7.5', url: MBCM_URL, verified: false,
      rates: [{ to: 30, r: 2 }, { to: 100, r: 1.5 }, { to: null, r: 1 }], sensRate: 8, years: 40, longYears: 60, timeZero: '1 July',
      commercial: { rate: 8, sensRate: 2 },
      discount: { id: 'nzta-gc-25-01', name: 'NZTA General Circular 25/01: updated discount rates', edition: 'Effective 6 January 2025', url: 'https://www.nzta.govt.nz/resources/monetised-benefits-and-costs-manual', file: 'docs/references/25-01.pdf', verified: true },
      label: 'Indicative, IER-equivalent screening estimate; not a funding-approval BCR',
      disclaimer: 'The benefit-cost ratio is an indicative screening estimate following NZTA Monetised Benefits and Costs Manual v1.7.5 conventions. It is equivalent to an Indicative Efficiency Rating, not a funding-approval BCR. It is not MBCM-compliant unless the full procedures are applied to a model calibrated and validated to the NZTA Transport Model Development Guidelines and independently reviewed. IPM efficiency bands for 2027-30 are shown from NZTA\'s initial draft (August 2026) and may change after GPS 2027.',
      p95From: 10 },
    // MBCM update factors to July 2025 values (multipliers on the manual's base-date values)
    mbcmUpdate: { id: 'mbcm-update-2025', name: 'MBCM update factors', edition: 'July 2025', url: 'https://www.nzta.govt.nz/assets/resources/monetised-benefits-and-costs-manual/MBCM-update-factors.pdf', verified: false,
      travelTime: 1.15, pt: 1.15, walkCycle: 1.15, crash: 1.25, emissions: 1.25, voc: 1.01, behaviourChange: 1.02 },
    ipm: [
      { id: '2024-27', name: 'Investment Prioritisation Method 2024–27 efficiency rating', edition: '2024–27 NLTP (final)', url: 'https://www.nzta.govt.nz/assets/P-and-I-Knowledge-Base/docs/2024-27-IPM.pdf', verified: false, draft: false,
        bands: [{ band: 'VL', label: 'Very low', below: 1 }, { band: 'L', label: 'Low', below: 3 }, { band: 'M', label: 'Medium', below: 6 }, { band: 'H', label: 'High', below: Infinity }] },
      { id: '2027-30', name: 'Investment Prioritisation Method 2027–30 efficiency rating', edition: 'Initial draft, August 2026', url: 'https://www.nzta.govt.nz/assets/planning-and-investment/nltp/2027/docs/initial-draft-ipm-for-the-2027-30-nltp-august-2026.pdf', verified: false, draft: true,
        note: 'draft, subject to GPS 2027',
        bands: [{ band: 'VL', label: 'Very low', below: 1 }, { band: 'L', label: 'Low', below: 1.5 }, { band: 'M', label: 'Medium', below: 3 }, { band: 'H', label: 'High', below: Infinity }] },
    ],
    growth: { id: 'mbcm-arithmetic', name: 'Background traffic growth', edition: 'MBCM v1.7.5 arithmetic growth; NZ ITA practice', url: MBCM_URL, verified: false },
    // Bus route preload: buses/h × PCU per bus (docs/spec/gtfs-bus: 2.0, or 2.5 in the MSM preset). legacy = the value
    // the engine used before the setting existed (projects saved without settings.busPcu keep it).
    bus: { id: 'corridor-bus-pcu', name: 'Passenger car units per bus', edition: 'docs/spec/gtfs-bus (Spare Construct research spec)', verified: false, pcu: 2.0, msm: 2.5, legacy: 3 },
    /* Movement-based gap acceptance (docs/spec/movement-capacity): HCM base critical gap tc and follow-up
       headway tf (s) for a two-lane major road, mirrored for left-hand traffic (HCM's major left = NZ major
       right, and so on). Two-stage crossings (flush median) use tc − stage1 in each stage. */
    gaps: { id: 'hcm-twsc-gaps', name: 'Critical gaps and follow-up headways', edition: 'HCM 6 ch. 20 (TWSC) base values, two-lane major road; mirrored for left-hand traffic', url: 'https://www.trb.org/Main/Blurbs/175169.aspx', verified: false,
      majorR: { tc: 4.1, tf: 2.2 }, minorL: { tc: 6.2, tf: 3.3 }, minorT: { tc: 6.5, tf: 4.0 }, minorR: { tc: 7.1, tf: 3.5 }, stage: 1.0, storage: 1,
      label: 'Movement capacities: HCM 6 two-way stop control method (rank impedance, shared and short lanes), mirrored for NZ left-hand traffic; indicative, not SIDRA output.' },
    // HCM 6 roundabout entry lanes (pcu/h): c = A·e^(−B·vc). HCM's left (inner) lane is NZ's right lane.
    rbtLanes: { id: 'hcm6-rbt', name: 'Roundabout lane capacity', edition: 'HCM 6 ch. 22 lane-based entry capacity equations; NZ markings left lane L+T, right lane T+R', url: 'https://www.trb.org/Main/Blurbs/175169.aspx', verified: false,
      one: { A: 1380, B: 0.00102 }, oneOf2: { A: 1420, B: 0.00085 }, twoOf1: { A: 1420, B: 0.00091 }, left2: { A: 1420, B: 0.00085 }, right2: { A: 1350, B: 0.00092 },
      label: 'HCM 6 roundabout (SIDRA Standard is geometry-sensitive and will differ)' },
    // AT ITA Guidelines 2025 v2 s4.2.1.1 study-extent test; the 20 veh/h floor and the freight set are Spare Construct assumptions
    extent: { id: 'at-ita-4.2.1.1', name: 'Extent of effects (5% / 10% test)', edition: 'AT ITA Guidelines 2025 v2, s4.2.1.1', url: 'https://at.govt.nz/media/ezemv4cz/auckland-transport-integrated-transport-assessment-guidelines-2025-vestion-2.pdf', verified: false,
      p5: 0.05, p10: 0.10, floor: 20, floorBasis: 'Spare Construct assumption', freightBasis: 'Spare Construct assumption (AT does not define "freight route")', buffer: 50,
      gt5: /^(motorway|strategic( arterial)?|primary arterial|secondary arterial|arterial \(from road class\))$/i, pt5: /rapid transit|frequent transit|\b(i?rtn|ftn)\b/i, freight5: /^level (1a|1b|2)$/i, gt10: /^collector/i },
    // Difference plots (extent-and-difference): bars hidden under max(floor, 2·noise95); labels by zoom
    diff: { id: 'corridor-diff', name: 'Difference plot thresholds', edition: 'Spare Construct assumption', verified: false, floor: 20, noiseK: 2, label: 10, labelLowZoom: 50, lowZoom: 15, wMin: 1, wMax: 14 },
    // Model status (reg-model-status) and the MSM consistency check
    tmdg: { id: 'tmdg-2019', name: 'NZTA Transport Model Development Guidelines', edition: '2019', url: 'https://www.nzta.govt.nz/assets/resources/transport-model-development-guidelines/docs/tmd.pdf', verified: false,
      standing: 'TMDG criteria are not a pass/fail checklist; meeting them does not by itself make a model fit for purpose (TMDG s5.8).' },
    msm: { id: 'corridor-msm', name: 'MSM consistency threshold', edition: 'Spare Construct assumption', verified: false, pct: 20,
      notChecked: 'MSM consistency not checked: AT ITA v2 s4.2.1.1 expects consistency with MSM forecasts where appropriate.' },
    // Default design year by pathway (scenario-matrix): RR 422 5.5.2 / 5.5.3, AT ITA 3.6
    design: { id: 'rr422-design-year', name: 'Design year', edition: 'NZTA RR 422 s5.5; AT ITA Guidelines 2025 v2 s3.6', url: 'https://www.nzta.govt.nz/assets/resources/research/reports/422/docs/422.pdf', verified: false,
      consent: 5, planchange: 10, nor: 10, ftaa: 5, warnAhead: 10, msmPeakShare: 0.55, msmShareBasis: 'Spare Construct assumption' },
  };
  const LOS_SIG = STANDARDS.los.hcm6.signals, LOS_UNSIG = STANDARDS.los.hcm6.sign, LOS_VC = STANDARDS.linkVC.cuts;
  const losOf = (x, th) => { for (let i = 0; i < 5; i++) if (x <= th[i]) return 'ABCDE'[i]; return 'F'; };
  // RTA bands are 'less than' bands (14 s is B)
  const losLt = (x, th) => { for (let i = 0; i < 5; i++) if (x < th[i]) return 'ABCDE'[i]; return 'F'; };
  const pcuF = S => 1 + S.hvPct / 100 * (S.pcuHV - 1);
  // Scenario year (contract C2): sc.year where set, otherwise the project year in settings
  const curSc = () => (Store.P ? Store.sc : null);
  // Years typed into a file can arrive as text ('2036'); they are always numbers here
  const yearOf = (sc = curSc(), S = Store.S) => (sc && +sc.year > 0 ? +sc.year : +S.year);
  /* Background growth factor from the base year to the scenario year. Linear (default): g % of base-year
     traffic per year, the MBCM arithmetic convention; 'compound' (older projects) grows on the grown value.
     A year before the base year shrinks traffic the same way, never below zero. */
  function growth(S, sc = curSc()) {
    const dy = yearOf(sc, S) - +S.baseYear, g = (+S.growth || 0) / 100;
    return S.growthMode === 'compound' ? Math.pow(1 + g, dy) : Math.max(0, 1 + g * dy);
  }
  // 'linear, 2.0% of 2026 volume per year' (the report prints this basis)
  const growthBasis = S => S.growthMode === 'compound'
    ? `compound, ${(+S.growth || 0).toFixed(1)}% a year from ${S.baseYear}`
    : `linear, ${(+S.growth || 0).toFixed(1)}% of ${S.baseYear} volume per year`;
  // Junction analysis settings with fallbacks (a settings object without the block runs as before v2)
  function junctionSet(S) {
    const J = (S && S.junction) || {}, method = STANDARDS.los[J.losMethod] ? J.losMethod : 'hcm6';
    return { method, pff: +J.pff > 0 && +J.pff <= 1 ? +J.pff : 1, T: +J.T > 0 ? +J.T : 1, queueSpace: +J.queueSpace > 0 ? +J.queueSpace : STANDARDS.queue.space,
      f95: +J.f95 > 0 ? +J.f95 : STANDARDS.queue.f95, capMethod: capMethodOf(S) };
  }
  // 'movement' (per-movement gap acceptance and lanes) or 'arc' (one tc/tf per minor arc, before movement-capacity)
  const capMethodOf = S => (S && S.junction && S.junction.capMethod === 'movement' ? 'movement' : 'arc');
  // Zone class for development tracking (dev-tracking, reg-receiving-env): gateways are external; a land-use
  // zone is z.role when set, otherwise existing (z.existing) or development
  const ZCLASS = ['existing', 'committed', 'development'];
  function zoneClass(z) {
    if (!z) return 'existing';
    if (z.kind === 'gateway') return 'external';
    if (ZCLASS.includes(z.role)) return z.role;
    return z.existing === true ? 'existing' : 'development';
  }
  const zcIdx = z => { const c = zoneClass(z); return c === 'development' ? 2 : c === 'committed' ? 1 : 0; };
  // Convergence settings: the TAG stability rule and MSA capacity damping unless switched off
  function convSet(S, opts = {}) {
    const C = (S && S.conv) || {}, D = STANDARDS.conv;
    return { gapTol: +opts.gapTol > 0 ? +opts.gapTol : +S.gapTol > 0 ? +S.gapTol : 1e-4, maxIter: +opts.maxIter > 0 ? Math.round(+opts.maxIter) : +S.maxIter > 0 ? Math.round(+S.maxIter) : 200,
      stable: C.stable !== false, msa: (C.capDamp ?? 'msa') === 'msa', chg: +C.chg > 0 ? +C.chg : D.chg, share: +C.share > 0 ? +C.share : D.share, consec: +C.consec > 0 ? Math.round(+C.consec) : D.consec };
  }
  /* CO2 grams per vehicle-km against average speed (km/h): an average-speed curve, heavy vehicles ≈ 3.5× a car.
     settings.co2Factor (optional, the fleet-mix input): a multiplier on the light-vehicle part, for a local fleet mix
     (e.g. from the Motor Vehicle Register) worked out with a sourced factor set; absent or invalid = 1. */
  function ef(v, S) {
    v = U.clamp(v, 5, 130);
    const car = 3300 / v + 95 + 0.011 * v * v, hv = S.hvPct / 100, lf = S && +S.co2Factor > 0 && isFinite(+S.co2Factor) ? +S.co2Factor : 1;
    return car * lf * (1 - hv) + car * 3.5 * hv;
  }

  /* ---------- graph ---------- */
  /* Junction sites (docs/spec/junction-sites): sc.sites[id].nodes act as one intersection. Links with both
     ends in one site are internal (free-flow time only: no congestion term, no junction delay); signals and
     give-way/stop sites get one controller over the arcs entering the site; roundabout sites keep one
     controller per entry node (the ring carries the conflicts) and are reported together. Zone connectors
     never attach to site nodes. */
  function siteIndex(sc, idx) {
    const of = new Map(), list = [];
    for (const s of Object.values(sc.sites || {})) {
      if (!s || !Array.isArray(s.nodes)) continue;
      const m = s.nodes.filter(id => idx.has(id) && !of.has(idx.get(id)));
      if (m.length < 1) continue;
      for (const id of m) of.set(idx.get(id), s.id);
      list.push({ site: s, members: m.map(id => idx.get(id)) });
    }
    return { of, list };
  }
  /* A variable speed limit (L.varSpeed = { v, desc, AM, PM }, from the NSLR variable layer) caps the link speed in
     the peak it covers; it never raises a lower speed. */
  function speedOf(L, C, period) {
    let spd = Math.max(5, +L.speed || C.speed);
    const vs = L.varSpeed;
    if (period && vs && typeof vs === 'object' && vs[period] === true && +vs.v > 0) spd = Math.max(5, Math.min(spd, +vs.v));
    return spd;
  }
  // Time-of-use charge on a link as seconds of generalised cost: L.toll (s, both directions) or { AM: s, PM: s }
  function tollOf(L, period) {
    const t = L && L.toll;
    if (t == null || t === '') return 0;
    const v = typeof t === 'object' ? (period ? t[period] : null) : t;
    return +v > 0 && isFinite(+v) ? +v : 0;
  }
  // buildGraph(sc, S, period): period (optional) applies the period's variable speed limits and charges
  function buildGraph(sc, S = Store.S, period = null) {
    const ids = Object.keys(sc.nodes), N0 = ids.length;
    const idx = new Map(); ids.forEach((id, i) => idx.set(id, i));
    const ll = ids.map(id => [sc.nodes[id].lat, sc.nodes[id].lng]);
    const arcs = [], okNode = new Uint8Array(N0), SI = siteIndex(sc, idx);
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed' || L.a === L.b || !idx.has(L.a) || !idx.has(L.b)) continue;
      const C = CLASSES[L.cls] || CLASSES.local;
      const g = Net.geom(sc, L), cum = U.cumLen(g), len = Math.max(cum[cum.length - 1], 1);
      const spd = speedOf(L, C, period), v = spd / 3.6, capLane = +L.capPerLane || C.cap, toll = tollOf(L, period);
      if (C.rank < 5) { okNode[idx.get(L.a)] = 1; okNode[idx.get(L.b)] = 1; }
      const two = (L.lanesAB | 0) > 0 && (L.lanesBA | 0) > 0;
      const sa = SI.of.get(idx.get(L.a)), internal = sa != null && sa === SI.of.get(idx.get(L.b));
      for (const dir of [1, -1]) {
        const lanes = (dir > 0 ? L.lanesAB : L.lanesBA) | 0;
        if (lanes <= 0) continue;
        const bus = !!L.busLane && lanes >= 2, gl = bus ? lanes - 1 : lanes;
        const pts = dir > 0 ? g : g.slice().reverse(), c = dir > 0 ? cum : U.cumLen(pts);
        const e = Math.min(20, len / 2);
        arcs.push({
          key: L.id + (dir > 0 ? ':ab' : ':ba'), link: L.id, dir, conn: false,
          from: idx.get(dir > 0 ? L.a : L.b), to: idx.get(dir > 0 ? L.b : L.a),
          len, lanes, gl, bus, two, ff: len / v, capMid: gl * capLane, cls: L.cls, rank: C.rank, speed: spd, lw: C.lw,
          ring: !!L.ring, name: L.name || '', pts, cum: c,
          inB: U.bearing(U.along(pts, c, len - e).p, pts[pts.length - 1]), outB: U.bearing(pts[0], U.along(pts, c, e).p),
          ctrl: 'free', ...(internal ? { internal: true } : {}), ...(toll ? { toll } : {}),
        });
      }
    }
    const zones = Object.values(sc.zones).filter(z => isFinite(z.lat) && isFinite(z.lng));
    const lat0 = N0 ? ll.reduce((s, p) => s + p[0], 0) / N0 : -36.85, F = U.flat(lat0);
    // Zones only connect to the main connected network, never to an unconnected fragment
    const main = mainComponent(N0, arcs);
    const cand = [];
    for (let i = 0; i < N0; i++) if (okNode[i] && main[i] && !SI.of.has(i)) cand.push({ id: ids[i], x: F.x(ll[i]), y: F.y(ll[i]) });
    const zoneNode = [], conns = {};
    for (const z of zones) {
      const zi = ids.length;
      ids.push('z:' + z.id); ll.push([z.lat, z.lng]); idx.set('z:' + z.id, zi); zoneNode.push(zi);
      const tg = connectorTargets(sc, z, cand, F, S);
      conns[z.id] = tg;
      for (const t of tg) {
        const ti = idx.get(t); if (ti == null) continue;
        const d = Math.max(10, U.dist([z.lat, z.lng], ll[ti])), ff = d / (S.connectorSpeed / 3.6);
        const base = { conn: true, len: d, ff, capMid: Infinity, gl: 1, lanes: 1, zone: z.id, ctrl: 'free', rank: 0 };
        const bo = U.bearing([z.lat, z.lng], ll[ti]), bd = U.bearing(ll[ti], [z.lat, z.lng]);
        arcs.push({ ...base, key: `c:${z.id}:${t}:o`, from: zi, to: ti, pts: [[z.lat, z.lng], ll[ti]], inB: bo, outB: bo });
        arcs.push({ ...base, key: `c:${z.id}:${t}:d`, from: ti, to: zi, pts: [ll[ti], [z.lat, z.lng]], inB: bd, outB: bd });
      }
    }
    const N = ids.length, M = arcs.length;
    const head = new Int32Array(N + 1);
    for (const a of arcs) head[a.from + 1]++;
    for (let i = 0; i < N; i++) head[i + 1] += head[i];
    const out = new Int32Array(M), fill = head.slice(0, N), to = new Int32Array(M), ffc = new Float64Array(M);
    const into = Array.from({ length: N }, () => []), outR = Array.from({ length: N }, () => []);
    arcs.forEach((a, i) => {
      a.i = i; out[fill[a.from]++] = i; to[i] = a.to; ffc[i] = a.ff;
      if (!a.conn) { into[a.to].push(i); outR[a.from].push(i); }
    });
    const isC = new Uint8Array(N); zoneNode.forEach(i => { isC[i] = 1; });
    const keyIdx = new Map(); arcs.forEach((a, i) => keyIdx.set(a.key, i));
    // Links meeting at each node (connectors not counted): 1 = dead end, ≥ 3 = junction
    const deg = new Int32Array(N), nodes = ids.map(id => sc.nodes[id] || null);
    for (let n = 0; n < N; n++) {
      if (isC[n]) continue;
      const s = new Set(); for (const i of into[n]) s.add(arcs[i].link); for (const i of outR[n]) s.add(arcs[i].link);
      deg[n] = s.size;
    }
    const G = { ids, idx, ll, arcs, N, M, head, out, to, ffc, into, outR, isC, zones, zoneNode, keyIdx, conns, ctl: new Map(), nodes, deg, turnS: turnSet(S),
      siteOf: SI.of, sites: SI.list, capMethod: capMethodOf(S), S, period };
    buildTurns(G);
    G.sm = siteMoveIndex(G);
    setupControls(sc, G, S);
    return G;
  }
  /* Site movements traced along paths: for each junction site its entry arcs (from outside onto a member node) and
     exit arcs (from a member node out of the site), and one slot per (entry, exit) pair. entS/entI and extS/extI map an
     arc to its site and position (−1 where none); an arc between two neighbouring sites is the exit of one and the
     entry of the other. Zone connectors are not entries or exits (they never attach to site nodes). */
  function siteMoveIndex(G) {
    if (!G.sites.length) return null;
    const M = G.M, entS = new Int32Array(M).fill(-1), entI = new Int32Array(M), extS = new Int32Array(M).fill(-1), extI = new Int32Array(M);
    const list = [], of = new Map();
    let n = 0;
    G.sites.forEach(({ site, members }, s) => {
      const ms = new Set(members), inc = [], exits = [];
      for (const m of members) {
        for (const i of G.into[m]) if (!ms.has(G.arcs[i].from)) inc.push(i);
        for (const i of G.outR[m]) if (!ms.has(G.arcs[i].to)) exits.push(i);
      }
      inc.forEach((a, k) => { entS[a] = s; entI[a] = k; });
      exits.forEach((a, k) => { extS[a] = s; extI[a] = k; });
      list.push({ id: site.id, inc, exits, off: n, nx: exits.length });
      of.set(site.id, s);
      n += inc.length * exits.length;
    });
    return { list, of, entS, entI, extS, extI, n };
  }
  /* Traced site-movement flows (pcu/h by G.sm slot) rebuilt from a result's res.siteTurns, × f: for re-evaluating
     junctions on a fresh graph (set G.smX to it before Engine.updateNodeCaps so the movement method reads them). */
  function siteFlows(G, res, f = 1) {
    if (!G.sm || !res || !res.siteTurns) return null;
    const x = new Float64Array(G.sm.n), pf = pcuF(G.S || Store.S);
    for (const [id, rows] of Object.entries(res.siteTurns)) {
      const s = G.sm.of.get(id); if (s == null || !Array.isArray(rows)) continue;
      const st = G.sm.list[s], ei = new Map(st.inc.map((a, k) => [G.arcs[a].key, k])), xi = new Map(st.exits.map((a, k) => [G.arcs[a].key, k]));
      for (const t of rows) { const e = ei.get(t.from), o = xi.get(t.to); if (e == null || o == null) continue; x[st.off + e * st.nx + o] += (t.pcu != null && isFinite(+t.pcu) ? +t.pcu : (+t.v || 0) * pf) * f; }
    }
    return x;
  }
  // Map(entry arc → Map(exit arc → pcu/h)) of one site from traced site-movement flows xm
  function tracedExits(G, s, xm) {
    const st = G.sm.list[s], out = new Map();
    st.inc.forEach((a, e) => {
      const m = new Map();
      for (let x = 0; x < st.nx; x++) { const v = xm[st.off + e * st.nx + x]; if (v > 0) m.set(st.exits[x], v); }
      out.set(a, m);
    });
    return out;
  }

  // Flags the nodes of the largest weakly connected piece of network
  function mainComponent(N, arcs) {
    const p = new Int32Array(N); for (let i = 0; i < N; i++) p[i] = i;
    const find = i => { while (p[i] !== i) { p[i] = p[p[i]]; i = p[i]; } return i; };
    for (const a of arcs) { const x = find(a.from), y = find(a.to); if (x !== y) p[x] = y; }
    const size = new Map(); let best = -1, bs = 0;
    for (let i = 0; i < N; i++) { const r = find(i), s = (size.get(r) || 0) + 1; size.set(r, s); if (s > bs) { bs = s; best = r; } }
    const m = new Uint8Array(N);
    for (let i = 0; i < N; i++) m[i] = find(i) === best ? 1 : 0;
    return m;
  }
  function connectorTargets(sc, z, cand, F, S = Store.S) {
    if (z.node && sc.nodes[z.node]) return [z.node];
    const k = z.kind === 'gateway' ? 1 : U.clamp((z.connectors || S.connectors || 2) | 0, 1, 4);
    const zx = F.x([z.lat, z.lng]), zy = F.y([z.lat, z.lng]);
    const best = [];
    for (const c of cand) {
      const d = Math.hypot(c.x - zx, c.y - zy);
      if (best.length < k || d < best[best.length - 1].d) {
        best.push({ id: c.id, d }); best.sort((a, b) => a.d - b.d);
        if (best.length > k) best.pop();
      }
    }
    return best.map(b => b.id);
  }

  // Connector targets for every zone without building a full graph (used for drawing)
  function connectorMap(sc) {
    const ids = Object.keys(sc.nodes), idx = new Map(ids.map((id, i) => [id, i])), arcs = [];
    const deg = new Map();
    for (const L of Object.values(sc.links)) {
      if (L.status === 'closed' || !idx.has(L.a) || !idx.has(L.b)) continue;
      arcs.push({ from: idx.get(L.a), to: idx.get(L.b) });
      if ((CLASSES[L.cls] || CLASSES.local).rank >= 5) continue;
      deg.set(L.a, 1); deg.set(L.b, 1);
    }
    const main = mainComponent(ids.length, arcs), inSite = siteIndex(sc, idx).of;
    const nodes = Object.values(sc.nodes).filter(n => deg.has(n.id) && main[idx.get(n.id)] && !inSite.has(idx.get(n.id)));
    const lat0 = nodes.length ? nodes[0].lat : -36.85, F = U.flat(lat0);
    const cand = nodes.map(n => ({ id: n.id, x: F.x([n.lat, n.lng]), y: F.y([n.lat, n.lng]) }));
    const m = {};
    for (const z of Object.values(sc.zones)) m[z.id] = connectorTargets(sc, z, cand, F);
    return m;
  }

  function hashOffset(s) { let h = 7; for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0; return Math.abs(h) % 97; }

  // A site's control: its own, or the strongest of its nodes' controls
  const CTL_RANK = { none: 0, priority: 1, stop: 2, roundabout: 3, signals: 4 };
  function siteControl(sc, s) {
    if (s.control in CTL_RANK && s.control !== 'none') return s.control;
    let best = 'none';
    for (const id of s.nodes) { const c = (sc.nodes[id] || {}).control || 'none'; if ((CTL_RANK[c] || 0) > CTL_RANK[best]) best = c; }
    return best;
  }
  const nameKey = s => String(s || '').toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();
  /* Legs of a site: the arcs entering and leaving it, grouped by road name (within 90°) or bearing (±35°), so
     both carriageways of a divided road on one side form one leg. Bearing = outward from the site. */
  function siteLegs(G, inc, exits) {
    const it = [...inc.map(i => ({ i, inn: true, b: ((G.arcs[i].inB + 180) % 360 + 360) % 360 })), ...exits.map(i => ({ i, inn: false, b: ((G.arcs[i].outB % 360) + 360) % 360 }))];
    for (const x of it) x.nm = nameKey(G.arcs[x.i].name);
    const p = it.map((_, k) => k), find = k => { while (p[k] !== k) k = p[k] = p[p[k]]; return k; };
    for (let a = 0; a < it.length; a++) for (let b = a + 1; b < it.length; b++) {
      const d = U.angDiff(it[a].b, it[b].b);
      if ((it[a].nm && it[a].nm === it[b].nm && d <= 90) || d <= 35) { const x = find(a), y = find(b); if (x !== y) p[y] = x; }
    }
    const groups = new Map();
    it.forEach((x, k) => { const r = find(k), g = groups.get(r); if (g) g.push(x); else groups.set(r, [x]); });
    return [...groups.values()].map((g, k) => {
      let sx = 0, sy = 0; for (const x of g) { sx += Math.sin(x.b * Math.PI / 180); sy += Math.cos(x.b * Math.PI / 180); }
      const names = g.map(x => G.arcs[x.i].name).filter(Boolean), cnt = {};
      for (const n of names) cnt[n] = (cnt[n] || 0) + 1;
      return { k, b: Math.round(((Math.atan2(sx, sy) * 180 / Math.PI) + 360) % 360), name: Object.keys(cnt).sort((a, b) => cnt[b] - cnt[a])[0] || '',
        rank: Math.max(...g.map(x => G.arcs[x.i].rank || 0)), ins: g.filter(x => x.inn).map(x => x.i), outs: g.filter(x => !x.inn).map(x => x.i) };
    });
  }

  function setupControls(sc, G, S) {
    // Sites with one controller (signals, give-way, stop); roundabout sites keep one controller per entry node
    const one = new Map();
    for (const { site, members } of G.sites) {
      const type = siteControl(sc, site);
      if (members.length > 1 && (type === 'signals' || type === 'priority' || type === 'stop')) one.set(site.id, { site, members, type });
    }
    for (let n = 0; n < G.N; n++) {
      if (G.isC[n]) continue;
      const sid = G.siteOf.get(n);
      if (sid != null && one.has(sid)) continue;
      // Internal links of a (roundabout) site run free; only ring arcs and arcs from outside are entries
      const inc = sid == null ? G.into[n] : G.into[n].filter(i => !G.arcs[i].internal || G.arcs[i].ring);
      if (!inc.length) continue;
      const node = sc.nodes[G.ids[n]], type = node.control || 'none';
      if (type === 'none') continue;
      const links = new Set();
      for (const i of inc) links.add(G.arcs[i].link);
      for (const i of G.outR[n]) links.add(G.arcs[i].link);
      if (links.size < (type === 'signals' ? 2 : 3)) continue;
      // Motorway merges and diverges run free; the downstream link's capacity governs them
      if (type !== 'signals' && [...inc, ...G.outR[n]].every(i => G.arcs[i].rank >= 5)) continue;
      makeCtl(G, S, type, n, inc, node, null);
    }
    for (const { site, members, type } of one.values()) {
      const ms = new Set(members), inc = [], exits = [];
      for (const n of members) {
        for (const i of G.into[n]) if (!ms.has(G.arcs[i].from)) inc.push(i);
        for (const i of G.outR[n]) if (!ms.has(G.arcs[i].to)) exits.push(i);
      }
      if (inc.length < 2) continue;
      const rep = members.slice().sort((a, b) => G.deg[b] - G.deg[a] || (G.ids[a] < G.ids[b] ? -1 : 1))[0];
      const legs = siteLegs(G, inc, exits);
      if (type !== 'signals' && legs.length < 3) continue;
      makeCtl(G, S, type, rep, inc, site, { site: site.id, members, mset: ms, legs, exits });
    }
    if (G.capMethod === 'movement') for (const ctl of G.ctl.values()) prepMoves(sc, G, ctl);
  }

  // One controller over the entry arcs inc at node n (or at site rep n); src = the node or site object
  function makeCtl(G, S, type, n, inc, src, site) {
    const ext = site || {}, key = site ? site.site : G.ids[n];
    if (site) { site.legIn = new Map(); site.legOut = new Map(); for (const l of site.legs) { for (const i of l.ins) site.legIn.set(i, l.k); for (const i of l.outs) site.legOut.set(i, l.k); } }
    if (type === 'signals') {
      let ref = G.arcs[inc[0]];
      for (const i of inc) if (G.arcs[i].rank > ref.rank) ref = G.arcs[i];
      const phases = new Set();
      for (const i of inc) {
        const a = G.arcs[i], d = U.angDiff(a.inB, ref.inB);
        a.phase = Math.min(d, 180 - d) < 45 ? 0 : 1; phases.add(a.phase);
      }
      const single = phases.size === 1;
      const ctl = {
        type, n, arcs: inc, single, C: +src.cycle || S.cycle, lost: S.lostPerPhase * 2,
        split: single ? [0.75, 0.25] : (Array.isArray(src.split) && src.split.length === 2 ? src.split : [0.5, 0.5]),
        offset: hashOffset(key), src, ...ext,
      };
      G.ctl.set(n, ctl);
      for (const i of inc) { const a = G.arcs[i]; a.ctrl = 'sig'; a.node = n; if (single) a.phase = 0; }
      applySignal(ctl, G);
    } else if (type === 'roundabout') {
      const ringIn = inc.filter(i => G.arcs[i].ring), ringOut = G.outR[n].filter(i => G.arcs[i].ring);
      for (const i of inc) {
        const a = G.arcs[i]; a.node = n;
        if (a.ring) { a.ctrl = 'free'; continue; }
        a.ctrl = 'rbt';
        // Mapped roundabouts: circulating flow past this entry = ring flow leaving the node minus this entry's own flow
        if (ringIn.length && ringOut.length) { a.ringOut = ringOut; a.ringIn = ringIn; a.conf = ringIn.map(j => [j, 1]); }
        else a.conf = inc.filter(j => j !== i).map(j => [j, 0.5]);
      }
      G.ctl.set(n, { type, n, arcs: inc, src, ...ext });
    } else if (site) {
      // A give-way or stop site: the major road is the top-ranked pair of legs (both carriageways of each)
      const pr = l => Math.max(...l.ins.map(i => G.arcs[i].cls === 'ramp' ? 3.5 : G.arcs[i].rank), -1);
      const withIn = site.legs.filter(l => l.ins.length), maxRank = Math.max(...withIn.map(pr));
      const top = withIn.filter(l => pr(l) === maxRank);
      let ml;
      if (top.length <= 2 && top.length < withIn.length) ml = top;
      else {
        let bp = null, bd = -1;
        for (let p = 0; p < top.length; p++) for (let q = p + 1; q < top.length; q++) { const d = U.angDiff(top[p].b, top[q].b); if (d > bd) { bd = d; bp = [top[p], top[q]]; } }
        ml = bp && bd > 100 ? bp : [top[0]];
      }
      const major = ml.flatMap(l => l.ins), ms = new Set(major);
      for (const i of inc) {
        const a = G.arcs[i]; a.node = n;
        if (ms.has(i)) { a.ctrl = 'free'; a.major = true; }
        else { a.ctrl = 'minor'; a.stop = type === 'stop'; a.conf = major.map(j => [j, 1]); }
      }
      G.ctl.set(n, { type, n, arcs: inc, major, src, ...ext });
    } else {
      // Ramps give way at their terminals with local roads
      const pr = i => G.arcs[i].cls === 'ramp' ? 3.5 : G.arcs[i].rank;
      let maxRank = 0;
      for (const i of inc) maxRank = Math.max(maxRank, pr(i));
      const top = inc.filter(i => pr(i) === maxRank);
      let major;
      if (top.length <= 2 && top.length < inc.length) major = top;
      else {
        let bp = null, bd = -1;
        for (let p = 0; p < top.length; p++) for (let q = p + 1; q < top.length; q++) {
          const d = U.angDiff(G.arcs[top[p]].inB, G.arcs[top[q]].inB);
          if (d > bd) { bd = d; bp = [top[p], top[q]]; }
        }
        major = bp && bd > 100 ? bp : [top[0]];
      }
      const ms = new Set(major);
      for (const i of inc) {
        const a = G.arcs[i]; a.node = n;
        if (ms.has(i)) { a.ctrl = 'free'; a.major = true; }
        else { a.ctrl = 'minor'; a.stop = type === 'stop'; a.conf = major.map(j => [j, 1]); }
      }
      G.ctl.set(n, { type, n, arcs: inc, major, src });
    }
  }

  function applySignal(ctl, G) {
    const g = ctl.C - ctl.lost;
    for (const i of ctl.arcs) {
      const a = G.arcs[i];
      a.C = ctl.C;
      a.gC = Math.max(0.08, g * ctl.split[a.phase || 0] / ctl.C);
    }
  }

  /* ---------- link and junction performance ---------- */
  /* Travel time (s) on an arc carrying x pcu/h: BPR running time plus delay at the downstream junction.
     Junction delay uses the peak 15-minute flow rate x / PFF and the analysis period T (h) in the
     overflow term (settings.junction; PFF 1 and T 1 h when the block is missing, as before v2). */
  function arcTime(a, x, S) {
    if (a.conn || a.internal) return a.ff;
    let t = a.ff * (1 + S.bprAlpha * Math.pow(x / a.capMid, S.bprBeta));
    if (a.ctrl === 'free') return a.mj ? t + majorDelay(a.mj, x, S) : t;
    const J = S.junction, pff = J && J.pff > 0 && J.pff <= 1 ? J.pff : 1, T = J && J.T > 0 ? J.T : 1, q = x / pff;
    if (a.ctrl === 'sig') {
      const c = Math.max(1, a.gl * S.satFlow * a.gC), X = q / c, g = a.gC;
      const d1 = 0.5 * a.C * (1 - g) * (1 - g) / (1 - Math.min(1, X) * g);                     // uniform delay (Webster)
      const d2 = 900 * T * ((X - 1) + Math.sqrt((X - 1) * (X - 1) + 4 * X / (c * T)));          // overflow delay, k = 0.5 (HCM)
      t += d1 + d2;
    } else if (a.ctrl === 'minor' || a.ctrl === 'rbt') {
      const c = Math.max(1, a.capN || a.capMid), X = q / c, h = 3600 / c;                       // HCM 6 control delay
      t += h + 900 * T * ((X - 1) + Math.sqrt((X - 1) * (X - 1) + h * X / (450 * T))) + (a.stop ? 5 : a.ctrl === 'rbt' ? 5 * Math.min(1, X) : 2);
    }
    return t;
  }
  // res.arcs capSrc names (as Kit.capSource shows them)
  const CAP_SRC = { signals: 'signals', priority: 'give way', stop: 'stop', roundabout: 'roundabout' };
  function effCap(a, S) {
    if (a.conn) return Infinity;
    if (a.ctrl === 'sig') return Math.min(a.capMid, a.gl * S.satFlow * a.gC);
    if (a.ctrl === 'minor' || a.ctrl === 'rbt') return Math.min(a.capMid, a.capN || a.capMid);
    return a.capMid;
  }
  /* Gap-acceptance capacities depend on the conflicting flow; refreshed from the previous iterate.
     w < 1 damps the update (MSA: capN ← capN + (capNew − capN)·w with w = 1/k): these capacities make
     the costs non-separable, and undamped updates make the assignment oscillate. */
  function updateNodeCaps(G, xt, w = 1, xT = null) {
    if (G.capMethod === 'movement' && G.tmv) return movementCaps(G, xt, w, xT || new Float64Array(G.nT));
    for (const a of G.arcs) {
      const cap = arcCap(a, xt);
      if (cap != null) a.capN = w < 1 && a.capN > 0 ? a.capN + (cap - a.capN) * w : cap;
    }
  }
  // One gap-acceptance capacity per arc (capMethod 'arc'): minor arcs against all major flow; roundabout entries against circulating flow
  function arcCap(a, xt) {
    if (a.ctrl === 'minor') {
      let vc = 0; for (const [j, f] of a.conf) vc += xt[j] * f;
      vc = Math.max(vc, 1);
      const tc = a.stop ? 6.5 : 6.0, tf = a.stop ? 3.5 : 3.2;
      const cp = vc * Math.exp(-vc * tc / 3600) / (1 - Math.exp(-vc * tf / 3600));
      return Math.max(80, Math.min(a.capMid, cp * (1 + 0.6 * (a.gl - 1))));
    }
    if (a.ctrl === 'rbt') {
      let vc = 0;
      if (a.ringOut) { for (const j of a.ringOut) vc += xt[j]; vc = Math.max(0, vc - xt[a.i]); }
      else for (const [j, f] of a.conf) vc += xt[j] * f;
      const per = a.gl > 1 ? 1420 * Math.exp(-0.00091 * vc) : 1380 * Math.exp(-0.00102 * vc);
      return Math.min(a.capMid, Math.max(50, per * a.gl));
    }
    return null;
  }

  /* ---------- movement-based capacity (docs/spec/movement-capacity; settings.junction.capMethod 'movement') ----------
     Priority junctions (HCM 6 two-way stop control, mirrored for left-hand traffic): rank 1 major T and L;
     rank 2 major R (opposed by the opposing T + L, NZ 2012 rule) and minor L; rank 3 minor T; rank 4 minor R.
     Potential capacity c_p = v_c·e^(−v_c·t_c/3600) / (1 − e^(−v_c·t_f/3600)); rank impedance c_m = c_p·Π p0
     (p0 = 1 − v/c_m; rank 4 through HCM's p′ = 0.65p″ − p″/(p″+3) + 0.6√p″). Conflicting flows (LHT; n = the
     major stream passing the minor leg on the near side, f = the far stream, o = the opposite minor):
       minor L  v_c = T_n/N + 0.5·L_n(into this leg)
       minor T  v_c = 2(R_n + R_f) + T_n + 0.5·L_n(this leg) + L_n(other) + T_f + L_f
       minor R  v_c = 2(R_n + R_f) + T_n + 0.5·L_n(this leg) + L_n(other) + T_f + 0.5·L_f + 0.5(L_o + T_o)
       major R  v_c = T + L of the opposing major approach; U-turns count with R.
     A flush median makes minor T and R two-stage (Brilon / HCM, one vehicle of storage, t_c − 1 s per stage).
     Lanes come from the per-approach lane model (node.lanes or site.lanes keyed by the approach link id:
     { lanes: [{ d: 'L'|'T'|'R'|'LT'|'TR'|'LTR', len: bay m }], median, slip, src }) or NZ default markings
     (one lane LTR; two lanes LT + TR). Movement flows share lanes so lane degrees of saturation (Σ v/c) are
     equal; shared-lane capacity is then Σv / Σ(v/c_m). Short lanes and flares interpolate between separate and
     shared operation by min(1, N_s / Q95 of the short lane), N_s = floor(bay length / queue space). On the
     major road a right turn sharing a lane blocks it: that lane's through capacity × p0 (with a bay, by the
     chance the bay overflows); the right turners' delay and the blocking delay are charged to the major
     approach. Roundabouts: HCM 6 lane equations by entry and circulating lanes (left lane L+T, right T+R).
     Flows are PCU at the peak flow rate (÷ PFF). Each entry's capN = Σv / (its degree of saturation), so the
     arc delay functions are unchanged; the movement detail stays on the arc (mvRes) for junction tables.
     Not SIDRA output; the HV adjustment to t_c is not applied (flows are already PCU). */
  const MVC = ['', 'L', 'T', 'R', 'U'];
  const cpot = (vc, tc, tf) => (vc > 0.01 ? vc * Math.exp(-vc * tc / 3600) / (1 - Math.exp(-vc * tf / 3600)) : 3600 / tf);
  const p0of = (v, c) => (c > 0 ? Math.max(0, 1 - v / c) : 0);
  const pPrime = p => Math.min(1, Math.max(0, 0.65 * p - p / (p + 3) + 0.6 * Math.sqrt(Math.max(0, p))));
  // Two-stage gap acceptance (HCM 6 / Brilon): stage capacities cI, cII, one-stage cm, major right turners vL using the median, storage k
  function twoStage(cI, cII, cm, vL, k = 1) {
    const a = k > 0 ? 1 - 0.32 * Math.exp(-1.3 * Math.sqrt(k)) : 1, den = cII - vL - cm, y = (cI - cm) / den;
    if (!isFinite(y) || Math.abs(den) < 1e-6) return Math.max(0, a * (cII - vL));
    if (Math.abs(y - 1) < 1e-6) return Math.max(0, a / (k + 1) * (k * (cII - vL) + cm));
    return Math.max(0, a / (Math.pow(y, k + 1) - 1) * (y * (Math.pow(y, k) - 1) * (cII - vL) + (y - 1) * cm));
  }
  // 95th-percentile queue (veh) and control delay (s, without the stop/give-way constant) of an unsignalised lane (HCM 6 eq. 20-68, 20-64)
  const q95v = (v, c, T) => { if (!(v > 0) || !(c > 0)) return 0; const x = v / c; return Math.max(0, 900 * T * ((x - 1) + Math.sqrt((x - 1) * (x - 1) + (3600 / c) * x / (150 * T))) * c / 3600); };
  const dHCM = (v, c, T) => { c = Math.max(1, c); const X = v / c, h = 3600 / c; return h + 900 * T * ((X - 1) + Math.sqrt((X - 1) * (X - 1) + h * X / (450 * T))); };

  // Lane model for an approach: the stored one (any of L T R U, 'L+T' style accepted) or NZ default markings
  function laneModel(raw, gl) {
    gl = Math.max(1, gl | 0);
    let lanes = (gl === 1 ? ['LTR'] : ['LT', ...Array(gl - 2).fill('T'), 'TR']).map(d => ({ d, len: 0 })), src = 'default';
    if (raw && Array.isArray(raw.lanes) && raw.lanes.length) {
      const L = raw.lanes.map(l => ({ d: String((l && typeof l === 'object' ? (l.d ?? l.mv) : l) || '').toUpperCase().replace(/U/g, 'R').replace(/[^LTR]/g, ''),
        len: Math.max(0, +(l && l.len) || 0) })).filter(l => l.d);
      if (L.length) { lanes = L.map(l => ({ d: [...new Set(l.d)].join(''), len: l.len })); src = raw.src || 'user'; }
    }
    const slip = !!(raw && raw.slip);
    if (slip) { lanes = lanes.map(l => ({ ...l, d: l.d.replace('L', '') || l.d })).filter(l => l.d); lanes.unshift({ d: 'L', len: 0, slip: true }); }
    return { lanes, median: !!(raw && raw.median), slip, lmSrc: src };
  }

  /* Spreads movement flows v {L, T, R} over lanes so lane degrees of saturation X = Σ v/c are as equal as the
     markings allow (movements with fewest lane choices first). cap(lane, m) = capacity of m in that lane. */
  function allocLanes(lanes, v, cap) {
    const n = lanes.length;
    if (n === 1) {   // one lane takes everything
      const l = lanes[0]; let x = 0;
      if (v.L > 0) x += v.L / Math.max(1e-6, cap(l, 'L')); if (v.T > 0) x += v.T / Math.max(1e-6, cap(l, 'T')); if (v.R > 0) x += v.R / Math.max(1e-6, cap(l, 'R'));
      return { X: [x], use: [{ L: v.L, T: v.T, R: v.R }] };
    }
    const X = new Array(n).fill(0), use = lanes.map(() => ({ L: 0, T: 0, R: 0 }));
    const mvs = ['L', 'T', 'R'].filter(m => v[m] > 0).map(m => {
      let ls = []; lanes.forEach((l, i) => { if (l.d.includes(m)) ls.push(i); });
      if (!ls.length) ls = m === 'L' ? [0] : m === 'R' ? [n - 1] : lanes.map((_, i) => i);   // a movement the markings leave out uses the nearest lane
      return { m, ls };
    }).sort((a, b) => a.ls.length - b.ls.length);
    for (const { m, ls } of mvs) {
      if (ls.length === 1) { const i = ls[0]; use[i][m] += v[m]; X[i] += v[m] / Math.max(1e-6, cap(lanes[i], m)); continue; }
      // exact water-filling: raise the lowest lanes together to the level h where Σ (h − X_i)·c_i = v
      const o = ls.map(i => ({ i, x: X[i], c: Math.max(1e-6, cap(lanes[i], m)) })).sort((p, q) => p.x - q.x);
      let sc = 0, sxc = 0, h = 0, k = 0;
      for (; k < o.length; k++) {
        sc += o[k].c; sxc += o[k].x * o[k].c; h = (v[m] + sxc) / sc;
        if (k + 1 >= o.length || h <= o[k + 1].x) break;
      }
      for (let j = 0; j <= k && j < o.length; j++) { const q = (h - o[j].x) * o[j].c; use[o[j].i][m] += q; X[o[j].i] = h; }
    }
    return { X, use };
  }
  /* Approach capacity (pcu/h) of a set of lanes. Short lanes (len > 0) interpolate between separate operation
     and shared operation (their movements join the neighbouring full lane) by min(1, N_s / Q95). */
  // detail false (during assignment iterations) leaves out the lane rows, which only the junction tables need
  function approachCap(lanes, v, cap, T, qs, detail = true) {
    const tot = v.L + v.T + v.R, sep = allocLanes(lanes, v, cap);
    let Xs = 0; for (const x of sep.X) if (x > Xs) Xs = x;
    const best = l => { let c = 0; for (const m of l.d) c = Math.max(c, cap(l, m)); return c; };
    const cSep = tot > 0 && Xs > 0 ? tot / Xs : lanes.reduce((s, l) => s + best(l), 0);
    const lanesOut = (A, f = 1) => (detail ? lanes.map((l, i) => { const u = A.use[i], lv = u.L + u.T + u.R; return { d: l.d, len: l.len || 0, slip: !!l.slip, v: lv, X: A.X[i] * f, c: lv > 0 && A.X[i] > 0 ? lv / A.X[i] : best(l), use: u }; }) : null);
    let nShort = 0; for (const l of lanes) if (l.len > 0) nShort++;
    if (!nShort || nShort === lanes.length || tot <= 0) return { X: tot > 0 ? Xs : 0, cap: cSep, r: 1, lanes: lanesOut(sep) };
    const shortI = lanes.map((l, i) => (l.len > 0 ? i : -1)).filter(i => i >= 0), full = lanes.map((l, i) => i).filter(i => !(lanes[i].len > 0));
    const merged = full.map(i => ({ ...lanes[i] }));
    for (const i of shortI) { const j = i < full[0] ? 0 : i > full[full.length - 1] ? merged.length - 1 : full.findIndex(f => f > i); merged[Math.max(0, j)].d = [...new Set(merged[Math.max(0, j)].d + lanes[i].d)].join(''); }
    const sh = allocLanes(merged, v, cap), Xh = Math.max(0, ...sh.X), cSh = Xh > 0 ? tot / Xh : cSep;
    let r = 1, q95s = 0;
    for (const i of shortI) {
      const u = sep.use[i], lv = u.L + u.T + u.R, c = lv > 0 && sep.X[i] > 0 ? lv / sep.X[i] : 0, q = q95v(lv, c, T), Ns = Math.floor(lanes[i].len / qs);
      q95s = Math.max(q95s, q);
      r = Math.min(r, q > 1e-9 ? Math.min(1, Ns / q) : 1);
    }
    const capA = cSh + (cSep - cSh) * r, X = tot / capA;
    return { X, cap: capA, r, q95short: q95s, lanes: lanesOut(sep, Xs > 0 ? X / Xs : 1) };
  }

  /* Movement capacities at a give-way or stop junction. ent: [{ major, inB, leg, lanes, median, stop, sLane, thru,
     gaps?, v: {L, T, R} pcu/h at the peak rate (U with R), Lto: { exit leg: left-turn flow } }]. Sets e.res. */
  // The fixed part: which major stream is near / far for each minor approach, the opposite minor and the
  // opposing major, and each approach's gap values (STANDARDS.gaps with any per-site e.gaps override)
  function prioGeom(ent, GP) {
    const majors = ent.filter(e => e.major), minors = ent.filter(e => !e.major);
    const far = (a, list) => list.filter(x => x !== a && U.angDiff(x.inB, a.inB) > 120).sort((p, q) => U.angDiff(q.inB, a.inB) - U.angDiff(p.inB, a.inB))[0] || null;
    const pick = h => (majors.length ? majors.reduce((b, m) => (U.angDiff(m.inB, h) < U.angDiff(b.inB, h) ? m : b)) : null);
    for (const e of ent) {
      e.g = {}; for (const k of ['majorR', 'minorL', 'minorT', 'minorR']) e.g[k] = { ...GP[k], ...((e.gaps && e.gaps[k]) || {}) };
      if (e.major) { e._opp = far(e, majors); continue; }
      let n = pick(e.inB - 90), f = pick(e.inB + 90);
      if (n && n === f) { if (U.angDiff(n.inB, e.inB - 90) <= U.angDiff(n.inB, e.inB + 90)) f = null; else n = null; }
      e._n = n; e._f = f; e._o = far(e, minors);
    }
    ent.geom = { majors, minors };
    return ent.geom;
  }
  function priorityCaps(ent, o = {}) {
    const GP = o.gaps || STANDARDS.gaps, T = o.T || 0.25, qs = o.qs || STANDARDS.queue.space, st = GP.stage ?? 1, k2 = GP.storage ?? 1;
    const { majors, minors } = ent.geom || prioGeom(ent, GP), gap = (e, key) => e.g[key];
    for (const e of ent) e.res = { mov: {} };
    for (const m of majors) {
      const opp = m._opp, vc = opp ? opp.v.T + opp.v.L : 0, g = gap(m, 'majorR'), cp = cpot(vc, g.tc, g.tf);
      m.res.mov.R = { vc, cp, cm: cp, p0: p0of(m.v.R, cp), rank: 2 };
    }
    const V = (m, k) => (m ? m.v[k] : 0), Lin = (m, leg) => (m ? m.Lto[leg] || 0 : 0), p0R = m => (m && m.res.mov.R ? m.res.mov.R.p0 : 1);
    for (const a of minors) {   // rank 2: minor L
      const n = a._n, vc = V(n, 'T') / Math.max(1, (n && n.thru) || 1) + 0.5 * Lin(n, a.leg), g = gap(a, 'minorL'), cp = cpot(vc, g.tc, g.tf);
      a.res.mov.L = { vc, cp, cm: cp, p0: p0of(a.v.L, cp), rank: 2 };
    }
    const stages = a => { const n = a._n, Ln = Lin(n, a.leg); return 2 * V(n, 'R') + V(n, 'T') + 0.5 * Ln + Math.max(0, V(n, 'L') - Ln); };
    for (const a of minors) {   // rank 3: minor T
      const f = a._f, vcI = stages(a), vcII = 2 * V(f, 'R') + V(f, 'T') + V(f, 'L'), g = gap(a, 'minorT'), cp = cpot(vcI + vcII, g.tc, g.tf);
      const fI = p0R(a._n), fII = p0R(f);
      let cm = cp * fI * fII;
      if (a.median) cm = twoStage(cpot(vcI, g.tc - st, g.tf) * fI, cpot(vcII, g.tc - st, g.tf) * fII, cm, V(f, 'R'), k2);
      a.res.mov.T = { vc: vcI + vcII, cp, cm, p0: p0of(a.v.T, cm), rank: 3, twoStage: !!a.median };
    }
    for (const a of minors) {   // rank 4: minor R
      const f = a._f, o2 = a._o, vcI = stages(a), vcII = 2 * V(f, 'R') + V(f, 'T') + 0.5 * V(f, 'L') + 0.5 * (o2 ? o2.v.L + o2.v.T : 0);
      const g = gap(a, 'minorR'), cp = cpot(vcI + vcII, g.tc, g.tf);
      const fI = p0R(a._n), p2 = fI * p0R(f) * (o2 && o2.res.mov.T ? o2.res.mov.T.p0 : 1), fac = pPrime(p2) * (o2 && o2.res.mov.L ? o2.res.mov.L.p0 : 1);
      let cm = cp * fac;
      if (a.median) cm = twoStage(cpot(vcI, g.tc - st, g.tf) * fI, cpot(vcII, g.tc - st, g.tf) * (fI > 0 ? fac / fI : fac), cm, V(f, 'R'), k2);
      a.res.mov.R = { vc: vcI + vcII, cp, cm, p0: p0of(a.v.R, cm), rank: 4, twoStage: !!a.median };
    }
    for (const a of minors) {
      const r = approachCap(a.lanes, a.v, (l, m) => a.res.mov[m].cm, T, qs, o.detail !== false);
      Object.assign(a.res, r, { v: a.v });
    }
    for (const m of majors) {
      const R = m.res.mov.R, vR = m.v.R, tot = m.v.L + m.v.T + vR, p0 = p0of(vR, R.cm);
      const ex = m.lanes.filter(l => l.d === 'R'), shared = !ex.length;
      let rb = shared ? 0 : 1, q95R = 0;
      const bay = ex.length && ex.every(l => l.len > 0) ? Math.max(...ex.map(l => l.len)) : 0;
      if (bay) { q95R = q95v(vR, R.cm * ex.length, T); rb = q95R > 1e-9 ? Math.min(1, Math.floor(bay / qs) / q95R) : 1; }
      const pEff = p0 + (1 - p0) * rb, tl = m.lanes.filter(l => l.d !== 'R');
      // the lane a right turner blocks: the one it shares, or (with a bay) the full lane next to the bay
      const lastT = tl.length - 1, blocked = (l, i) => l.d.includes('R') || (!!bay && i === lastT);
      const tlL = (tl.length ? tl : [{ d: 'LT', len: 0 }]).map((l, i) => ({ d: l.d.replace('R', '') || 'T', len: 0, c: m.sLane * (blocked(l, i) ? Math.max(0.02, pEff) : 1), blk: blocked(l, i) }));
      const al = allocLanes(tlL, { L: m.v.L, T: m.v.T, R: 0 }, l => l.c);
      const XTL = Math.max(0, ...al.X), XR = vR > 0 ? vR / (R.cm * Math.max(1, ex.length)) : 0, X = Math.max(XTL, XR);
      let shTL = 0; tlL.forEach((l, i) => { if (l.blk) shTL += al.use[i].L + al.use[i].T; });
      const capT = tlL.reduce((s, l) => s + (l.d.includes('T') ? l.c : 0), 0);
      Object.assign(m.res, { X, XR, XTL, cap: tot > 0 && X > 0 ? tot / X : capT + R.cm, capT, p0, pEff, rb, bay, q95R, sR: tot > 0 ? vR / tot : 0, shTL: tot > 0 ? shTL / tot : 0,
        lanes: o.detail === false ? null : tlL.map((l, i) => ({ d: l.d, len: 0, v: al.use[i].L + al.use[i].T, X: al.X[i], c: l.c, use: al.use[i], blocked: l.blk })).concat(ex.map(l => ({ d: 'R', len: l.len, v: vR / ex.length, X: XR, c: R.cm, use: { L: 0, T: 0, R: vR / ex.length } }))), v: m.v });
    }
    return ent;
  }
  // Roundabout entries by HCM 6 lane equations; e.vc = circulating flow (pcu/h, peak rate), e.nc = circulating lanes
  function rbtCaps(ent, o = {}) {
    const R = STANDARDS.rbtLanes, T = o.T || 0.25, qs = o.qs || STANDARDS.queue.space;
    for (const e of ent) {
      const ne = e.lanes.filter(l => !l.slip).length;
      let j = 0;
      const lanes = e.lanes.map(l => {
        if (l.slip) return { ...l, c: e.sLane };   // left-turn bypass: no circulating conflict (Spare Construct simplification)
        const q = ne === 1 ? (e.nc >= 2 ? R.oneOf2 : R.one) : e.nc <= 1 ? R.twoOf1 : j === ne - 1 ? R.right2 : R.left2;
        j++;
        return { ...l, c: q.A * Math.exp(-q.B * e.vc) };
      });
      e.res = { ...approachCap(lanes, e.v, l => l.c, T, qs, o.detail !== false), vc: e.vc, nc: e.nc, v: e.v, mov: {}, laneCap: lanes.map(l => l.c) };
    }
    return ent;
  }
  // Delay (s) a major approach adds for right turners and the through traffic they block, at x pcu/h
  function majorDelay(mj, x, S) {
    if (!(mj.sR > 0) || !(mj.cmR > 0)) return 0;
    const J = S.junction, pff = J && J.pff > 0 && J.pff <= 1 ? J.pff : 1, T = J && J.T > 0 ? J.T : 1;
    const vR = mj.sR * x / pff, p0 = Math.max(0, 1 - vR / mj.cmR), pE = p0 + (1 - p0) * mj.rb, dR = dHCM(vR, mj.cmR, T) + 2;
    return mj.sR * dR + mj.shTL * (1 - pE) * dR;
  }

  // Entry data for a junction controller (static), and the per-iteration movement flows
  function prepMoves(sc, G, ctl) {
    if (ctl.type === 'signals') return;
    const site = ctl.site ? (sc.sites || {})[ctl.site] : null;
    const ent = [];
    for (const i of ctl.arcs) {
      const A = G.arcs[i]; if (A.ring || A.ctrl === 'free' && !A.major) continue;
      const node = sc.nodes[G.ids[A.to]] || {}, raw = (site && site.lanes && site.lanes[A.link]) || (node.lanes && node.lanes[A.link]) || null;
      const e = { i, major: !!A.major, inB: A.inB, leg: ctl.site ? 'g' + ctl.legIn.get(i) : A.link, gl: Math.max(1, A.gl), sLane: A.capMid / Math.max(1, A.gl),
        stop: !!A.stop, gaps: (site && site.gaps) || node.gaps || null, ...laneModel(raw, A.gl), v: { L: 0, T: 0, R: 0 }, Lto: {} };
      e.thru = Math.max(1, e.lanes.filter(l => l.d.includes('T') && !(l.len > 0)).length);
      A.lmSrc = e.lmSrc;
      if (!ctl.site) {
        e.mvT = [];
        for (let t = G.tOff[i]; t < G.tOff[i + 1]; t++) if (G.tpen[t] !== INF) { const B = G.arcs[G.tTo[t]]; e.mvT.push([t, MVC[G.tmv[t]], B.conn ? 'c:' + B.zone : B.link]); }
      }
      if (ctl.type === 'roundabout') e.nc = A.ringIn && A.ringIn.length ? Math.max(...A.ringIn.map(j => Math.max(1, G.arcs[j].gl))) : 0;
      ent.push(e);
    }
    if (ctl.type === 'roundabout') { const mx = Math.min(2, Math.max(1, ...ent.map(e => e.gl))); for (const e of ent) if (!e.nc) e.nc = mx; }
    ctl.ent = ent;
  }
  /* Site movements by chaining the internal links' turn shares node by node (Map(entry arc → Map(exit arc → pcu/h))).
     Fallback only, for a graph without traced site flows (G.smX): chaining shares invents U-turns on multi-node sites
     (a share of the flow arriving at a ring node is sent back the way it came), so assign() traces the movements along
     the paths instead. */
  function siteExits(G, ctl, xT) {
    const out = new Map();
    const walk = (m, a, f, depth) => {
      let tot = 0; for (let t = G.tOff[a]; t < G.tOff[a + 1]; t++) tot += xT[t];
      if (!(tot > 0)) return;
      for (let t = G.tOff[a]; t < G.tOff[a + 1]; t++) {
        const v = xT[t]; if (!(v > 0)) continue;
        const b = G.tTo[t], g = f * v / tot;
        if (G.arcs[b].internal && depth < 8) walk(m, b, g, depth + 1); else m.set(b, (m.get(b) || 0) + g);
      }
    };
    for (const a of ctl.arcs) {
      const m = new Map();
      for (let t = G.tOff[a]; t < G.tOff[a + 1]; t++) {
        const v = xT[t]; if (!(v > 0)) continue;
        const b = G.tTo[t];
        if (G.arcs[b].internal) walk(m, b, v, 1); else m.set(b, (m.get(b) || 0) + v);
      }
      out.set(a, m);
    }
    return out;
  }
  // Movement (L/T/R/U) of a site movement from entry arc a to exit arc b
  const siteMv = (G, ctl, a, b) => { const B = G.arcs[b]; if (!B.conn && ctl.legIn.get(a) === ctl.legOut.get(b)) return 'U'; return turnType(G.arcs[a].inB, B.outB); };
  function ctlFlows(G, ctl, xT, pff) {
    const s = ctl.site && G.sm && G.smX ? G.sm.of.get(ctl.site) : null;
    const ex = ctl.site ? (s != null ? tracedExits(G, s, G.smX) : siteExits(G, ctl, xT)) : null;
    for (const e of ctl.ent) {
      const v = { L: 0, T: 0, R: 0 }, Lto = {};
      const add = (m, f, leg) => { const k = m === 'U' ? 'R' : m || 'T'; v[k] += f; if (k === 'L') Lto[leg] = (Lto[leg] || 0) + f; };
      if (!ex) { for (const [t, m, leg] of e.mvT) { const f = xT[t] / pff; if (f > 0) add(m, f, leg); } }
      else for (const [b, f] of ex.get(e.i) || []) { const B = G.arcs[b]; add(siteMv(G, ctl, e.i, b), f / pff, B.conn ? 'c:' + B.zone : 'g' + ctl.legOut.get(b)); }
      e.v = v; e.Lto = Lto;
    }
  }
  function movementCaps(G, xt, w, xT) {
    const S = G.S, J = junctionSet(S), damp = (old, nw) => (w < 1 && old != null && isFinite(old) ? old + (nw - old) * w : nw);
    for (const ctl of G.ctl.values()) {
      if (!ctl.ent) continue;
      ctlFlows(G, ctl, xT, J.pff);
      if (ctl.type === 'roundabout') {
        for (const e of ctl.ent) {
          const A = G.arcs[e.i]; let vc = 0;
          if (A.ringOut) { for (const j of A.ringOut) vc += xt[j]; vc = Math.max(0, vc - xt[A.i]); } else for (const [j, f] of A.conf) vc += xt[j] * f;
          e.vc = vc / J.pff;
        }
        rbtCaps(ctl.ent, { T: J.T, qs: J.queueSpace, detail: G.mvDetail !== false });
      } else priorityCaps(ctl.ent, { T: J.T, qs: J.queueSpace, detail: G.mvDetail !== false });
      for (const e of ctl.ent) {
        const A = G.arcs[e.i], r = e.res;
        A.mvRes = r;
        if (e.major) {
          const mj = A.mj || (A.mj = { sR: null, shTL: null, cmR: null, rb: 0 });
          mj.sR = damp(mj.sR, r.sR); mj.shTL = damp(mj.shTL, r.shTL); mj.cmR = damp(mj.cmR, r.mov.R.cm); mj.rb = r.rb;
        } else A.capN = damp(A.capN, Math.min(A.capMid, Math.max(ctl.type === 'roundabout' ? 50 : 80, r.cap)));
      }
    }
  }

  /* ---------- junction results (docs/spec/junction-los-corrections) ---------- */
  const JUNCTION_LABEL = 'HCM 6 / Akcelik-based, indicative; not SIDRA output';
  const LETTERS = 'ABCDEF';
  /* One junction's approaches and summary. Units: v and cap are veh/h (cap = PCU capacity / pcuF, so
     demand and capacity share units); dos is the PCU ratio at the peak flow rate (flow / PFF). Approach
     LoS is F whenever DoS > 1, otherwise from delay; signal and roundabout junctions report the
     volume-weighted mean delay and its LoS (delay only); two-way give-way/stop junctions report no
     intersection LoS, only the worst movement ('F (side road, South approach)'). Back of queue (95th
     percentile): signals per lane Q1 + Q2 (HCM 2000 / Akçelik) × f95; unsignalised and roundabout
     entries by HCM 6 eq. 20-68, shared over the entry lanes. Queue length = vehicles × queueSpace. */
  function junctionResult(G, ctl, xt, arcsRes, S, J, pf) {
    const LOS = STANDARDS.los[J.method], XP = STANDARDS.practicalDoS, type = ctl.type;
    const isSig = type === 'signals', isRbt = type === 'roundabout', twsc = !isSig && !isRbt;
    const th = isSig ? LOS.signals : isRbt ? LOS.roundabout : LOS.sign, lf = J.method === 'rta' ? losLt : losOf;
    const xp = isSig ? XP.signals : isRbt ? XP.roundabout : XP.sign;
    const appr = [];
    let sv = 0, sd = 0, worstDos = 0, w = null;
    for (const ai of ctl.arcs) {
      const A = G.arcs[ai], r = arcsRes[A.key], nd = Math.max(0, r.nd);
      // movement method: the arc carries its movement detail; a major approach with right turners is reported too
      const mv = J.capMethod === 'movement' && A.mvRes && !A.ring ? A.mvRes : null, mjr = !!(mv && A.major && A.mj && mv.v && mv.v.R > 0);
      const free = A.ctrl === 'free' && !mjr;
      const capP = mjr ? mv.cap : effCap(A, S), flowP = xt[ai] / J.pff, dos = flowP / capP, capV = capP / pf, gl = Math.max(1, A.gl);
      let qv = 0;
      if (mv && !free && A.ctrl !== 'sig') qv = Math.max(0, ...mv.lanes.map(l => q95v(l.v, l.c, J.T)));
      else if (A.ctrl === 'sig') {
        const cL = capV / gl, vL = flowP / pf / gl, X = dos, gC = A.gC, C = A.C, sL = S.satFlow / pf;
        const Q1 = vL * C / 3600 * (1 - gC) / (1 - Math.min(1, X) * gC);
        const kB = STANDARDS.queue.kB * Math.pow(sL * gC * C / 3600, 0.7), cT = Math.max(1e-9, cL * J.T);
        const Q2 = 0.25 * cL * J.T * ((X - 1) + Math.sqrt((X - 1) * (X - 1) + 8 * kB * X / cT));
        qv = (Q1 + Q2) * J.f95;
      } else if (!free) {
        const c = Math.max(1, capV), x = dos, T = J.T;
        qv = 900 * T * ((x - 1) + Math.sqrt((x - 1) * (x - 1) + (3600 / c) * x / (150 * T))) * c / 3600 / gl;
      }
      const losDelay = free ? '–' : lf(nd, th), los = free ? '–' : LOS.overCapacityF && dos > 1 ? 'F' : losDelay;
      const row = { key: A.key, link: A.link, name: A.name, inB: Math.round(A.inB), approach: approachName(A.inB), v: r.v, cap: Math.round(capV), capPcu: Math.round(capP),
        dos: U.r3(dos), xp: free ? null : xp, psc: free || !(dos > 0) ? null : Math.round((xp / dos - 1) * 100), dosBand: free ? null : dos <= xp ? 'ok' : dos <= 1 ? 'near' : 'over',
        delay: U.r1(nd), los, losDelay, q95: Math.round(Math.max(0, qv) * J.queueSpace), q95veh: U.r1(Math.max(0, qv)), phase: A.phase, major: !!A.major, ctrl: A.ctrl };
      if (mv && !free) Object.assign(row, movRows(A, mv, type, th, lf, LOS, J, pf, xp));
      appr.push(row);
      if (free) continue;
      sv += r.v; sd += r.v * nd;
      if (dos > worstDos) worstDos = dos;
      if (!w || LETTERS.indexOf(los) > LETTERS.indexOf(w.los) || (los === w.los && nd > w.delay)) w = row;
    }
    const avg = sv > 0 ? sd / sv : 0, worstD = w ? w.delay : 0;
    const los = twsc ? (w ? w.los : 'A') : lf(avg, th);
    return {
      type, appr, delay: U.r1(twsc ? worstD : avg), delayBasis: twsc ? 'worst movement' : 'average', avgDelay: U.r1(avg), worstDelay: U.r1(worstD),
      worstLeg: w ? { key: w.key, name: w.name, approach: w.approach, delay: w.delay, dos: w.dos, los: w.los } : null,
      dos: U.r3(worstDos), vol: Math.round(appr.reduce((s, a) => s + a.v, 0)),
      los, intersectionLos: twsc ? null : los, losBasis: twsc ? 'worst movement' : 'intersection delay',
      losText: twsc ? `${los} (${w && w.major ? 'major road right turn' : 'side road'}${w ? `, ${w.approach} approach` : ''})` : los,
      xp, method: J.method, capMethod: J.capMethod, capLabel: J.capMethod === 'movement' ? (isRbt ? STANDARDS.rbtLanes.label : isSig ? null : STANDARDS.gaps.label) : null,
      C: isSig ? Math.round(ctl.C) : null, split: isSig ? ctl.split.map(U.r3) : null,
      ...(ctl.site ? { site: ctl.site, nodes: ctl.members.map(n => G.ids[n]) } : {}),
    };
  }
  /* Movement rows for one approach (J1): demand veh/h (hourly), capacity veh/h and DoS at the peak flow rate,
     delay s, LoS, Q95; lane rows; through capacity and the right-turn bay on major approaches. */
  function movRows(A, mv, type, th, lf, LOS, J, pf, xp) {
    const T = J.T, k = J.pff / pf, extra = type === 'roundabout' ? null : A.stop ? 5 : 2, isMaj = !!A.major;
    const laneD = l => dHCM(l.v, l.c, T) + (extra == null ? 5 * Math.min(1, l.X) : extra);
    const dR = isMaj && mv.mov.R ? dHCM(mv.v.R, mv.mov.R.cm, T) + 2 : 0;
    const rows = [];
    for (const m of ['L', 'T', 'R']) {
      const v = mv.v[m] || 0, cm = mv.mov[m] ? mv.mov[m].cm : null;
      if (!(v > 0.05) && cm == null) continue;
      const ls = mv.lanes.filter(l => l.use[m] > 0);
      let X = ls.length ? Math.max(...ls.map(l => l.X)) : 0, d = 0, q = ls.length ? Math.max(...ls.map(l => q95v(l.v, l.c, T))) : 0;
      if (isMaj && m === 'R') { X = mv.XR; d = dR; q = q95v(v, cm, T); }
      else if (isMaj) { const b = ls.filter(l => l.blocked).reduce((s, l) => s + l.use[m], 0); d = v > 0 ? b / v * (1 - mv.pEff) * dR : 0; q = 0; }
      else d = v > 0 ? ls.reduce((s, l) => s + l.use[m] * laneD(l), 0) / v : 0;
      const capPcu = v > 0.05 && X > 0 ? v / X : (cm != null ? cm : 0);
      rows.push({ mv: m, v: U.r1(v * k), cap: Math.round(capPcu / pf), dos: U.r3(X), delay: U.r1(d), los: X > 1 ? 'F' : lf(d, th), q95veh: U.r1(q), q95: Math.round(q * J.queueSpace),
        psc: X > 0 ? Math.round((xp / X - 1) * 100) : null, rank: mv.mov[m] ? mv.mov[m].rank : 1, ...(mv.mov[m] ? { vc: Math.round(mv.mov[m].vc / pf), twoStage: !!mv.mov[m].twoStage } : {}) });
    }
    return { mov: rows, lanes: mv.lanes.map(l => ({ d: l.d, len: l.len || 0, slip: !!l.slip, v: U.r1(l.v * k), cap: Math.round(l.c / pf), dos: U.r3(l.X), ...(l.blocked ? { blocked: true } : {}) })),
      laneSrc: A.lmSrc || null, capMethod: 'movement',
      ...(isMaj ? { capT: Math.round(mv.capT / pf), bay: mv.bay || 0, rightBlock: U.r3(1 - mv.pEff) } : {}), ...(mv.r < 1 ? { shortLaneFactor: U.r3(mv.r) } : {}) };
  }
  // Delay for display: never above 300 s, and over-capacity approaches say so ('> 300 s · over capacity (DoS 1.42)').
  // CSVs keep the exact value.
  function delayText(d, dos) {
    if (d == null || !isFinite(d)) return '–';
    const t = d > 300 ? '> 300 s' : `${Math.round(d)} s`;
    return dos > 1 ? `${t} · over capacity (DoS ${(+dos).toFixed(2)})` : t;
  }
  // 'Not converged: reached 200 iterations at gap 0.20% (target 0.01%)'
  function convText(o) {
    const pct = g => (g * 100 >= 0.005 || !(g > 0) ? (g * 100).toFixed(2) : (g * 100).toPrecision(2)) + '%';
    if (o.converged) return `Converged: gap ${pct(o.gap)} (target ${pct(o.gapTol)})${o.stable ? `, flows stable (P > ${Math.round(o.share * 100)}%) for ${o.consec} iterations` : ''}, ${o.it} iterations.`;
    if (o.hitMax) return `Not converged: reached ${o.it} iterations at gap ${pct(o.gap)} (target ${pct(o.gapTol)})${o.stable && o.gap <= o.gapTol ? '; flows not yet stable' : ''}.`;
    return `Not converged: gap ${pct(o.gap)} (target ${pct(o.gapTol)}).`;
  }

  /* ---------- turns ---------- */
  // Indicative turn penalties (s); settings.turns overrides them (older projects have no such block)
  const TURN_DEF = { left: 2, right: 6, rightSignals: 4, uturn: 'ban' };
  const INF = Infinity;   // a closure constant: global lookups in the search loop are slow in some hosts
  const turnSet = S => ({ ...TURN_DEF, ...((S && S.turns) || {}) });
  const arcOf = (G, a) => typeof a === 'number' ? G.arcs[a] : a;
  // Movement from an arrival heading to a departure heading. Left-hand traffic: a positive turn is a right turn.
  function turnType(inB, outB) {
    const t = U.turn(inB, outB), m = Math.abs(t);
    return m <= 30 ? 'T' : m > 150 ? 'U' : t > 0 ? 'R' : 'L';
  }
  // Movement from arc a into arc b at node a.to. Reversing onto the same link is a U-turn, and so is a
  // hairpin (> 150°) onto another link at a junction (≥ 3 links): on divided roads mapped as two one-way
  // carriageways that is turning round the end of the median. At a 2-link node a hairpin is a bend.
  function movement(G, a, b) {
    a = arcOf(G, a); b = arcOf(G, b);
    if (a.conn || b.conn) return null;
    if (a.link === b.link) return 'U';
    const m = turnType(a.inB, b.outB);
    return m !== 'U' || G.deg[a.to] >= 3 ? m : U.turn(a.inB, b.outB) > 0 ? 'R' : 'L';
  }
  // The uturn setting: 'ban', or a penalty in seconds (a number typed as text is accepted). null = banned.
  const uPen = T => (T && T.uturn !== 'ban' && T.uturn !== '' && T.uturn != null && isFinite(+T.uturn) ? Math.max(0, +T.uturn) : null);
  // Seconds added going from arc a into arc b at node a.to (bans are separate: see turnBanned)
  function turnPenalty(G, a, b) {
    a = arcOf(G, a); b = arcOf(G, b);
    if (a.conn || b.conn) return 0;
    const T = G.turnS || turnSet(Store.S), deg = G.deg[a.to], node = G.nodes[a.to], ctl = (node && node.control) || 'none';
    const right = ctl === 'signals' ? T.rightSignals : T.right, up = uPen(T);
    // Reversing on the spot: free at a dead end, otherwise the U-turn penalty (the right-turn one under 'ban')
    if (a.link === b.link) return deg <= 1 ? 0 : Math.max(0, +(up != null ? up : right) || 0);
    if (deg < 3 || ctl === 'roundabout') return 0;   // roundabout entry delay is already in the arc time
    const mv = movement(G, a, b);
    if (mv === 'T') return 0;
    const p = mv === 'L' ? T.left : mv === 'R' ? right : up != null ? up : right;
    return Math.max(0, +p || 0);
  }
  // Explicit bans, and U-turns (see movement) except at dead ends, where node.allowU, or when the
  // uturn setting is a penalty rather than 'ban'
  function banned(G, a, b) {
    if (a.conn || b.conn) return false;
    const n = a.to, node = G.nodes[n];
    if (node && Array.isArray(node.bans) && node.bans.includes(a.link + '>' + b.link)) return true;
    if (uPen(G.turnS) != null || G.deg[n] <= 1 || (node && node.allowU === true)) return false;
    return movement(G, a, b) === 'U';
  }
  /* Per-arc turn table aligned with the node out-lists: the turn from arc a into out[k] of node a.to is
     t = tOff[a] + k - head[a.to]; tpen[t] = penalty in seconds, Infinity if banned; tTo[t] = the arc entered. */
  function buildTurns(G) {
    const { arcs, head, out, M } = G, tOff = new Int32Array(M + 1);
    for (let a = 0; a < M; a++) { const n = arcs[a].to; tOff[a + 1] = tOff[a] + head[n + 1] - head[n]; }
    // tmv[t]: the movement (index into MVC: 1 L, 2 T, 3 R, 4 U); zone connector turns by bearing
    const nT = tOff[M], tpen = new Float64Array(nT), tTo = new Int32Array(nT), tmv = new Uint8Array(nT);
    for (let a = 0; a < M; a++) {
      const A = arcs[a], h = head[A.to], o = tOff[a] - h;
      for (let k = h, e = head[A.to + 1]; k < e; k++) {
        const B = arcs[out[k]];
        tTo[o + k] = out[k];
        tpen[o + k] = banned(G, A, B) ? INF : turnPenalty(G, A, B);
        tmv[o + k] = MVC.indexOf((A.conn || B.conn ? turnType(A.inB, B.outB) : movement(G, A, B)) || 'T');
      }
    }
    Object.assign(G, { tOff, tpen, tTo, tmv, nT });
  }
  function turnIdx(G, a, b) {
    a = arcOf(G, a); b = arcOf(G, b);
    const h = G.head[a.to];
    for (let k = h, e = G.head[a.to + 1]; k < e; k++) if (G.out[k] === b.i) return G.tOff[a.i] + k - h;
    return -1;
  }
  const turnBanned = (G, a, b) => { const t = turnIdx(G, a, b); return t < 0 || G.tpen[t] === INF; };
  // Compass side that traffic arriving on heading inB comes from: heading north = the South approach
  function approachName(inB, eight = false) {
    const s = (((+inB || 0) + 180) % 360 + 360) % 360, n = eight ? 8 : 4;
    const names = eight ? ['North', 'NE', 'East', 'SE', 'South', 'SW', 'West', 'NW'] : ['North', 'East', 'South', 'West'];
    return names[Math.round(s / (360 / n)) % n];
  }

  /* ---------- shortest paths ---------- */
  // Scratch arrays for edge-based searches: arc labels (dist, pred arc, pt turn used) and node results
  // (nd = best arrival time, na = the arc it arrives on)
  function workspace(G) {
    return { dist: new Float64Array(G.M), pred: new Int32Array(G.M), pt: new Int32Array(G.M), nd: new Float64Array(G.N), na: new Int32Array(G.N), heap: new U.Heap(G.M + 16), src: -1 };
  }
  /* Edge-based Dijkstra from node src. Entering arc b from arc a costs tpen + cost[b]; banned turns are
     skipped and zone centroids are never routed through. fromArc (optional, arriving at src) makes the
     first turn obey the same rules, for chained waypoints. Stops once target is reached. */
  function spt(G, cost, src, W, target = -1, fromArc = -1) {
    const { head, out, to, isC, tOff, tpen } = G, { dist, pred, pt, nd, na, heap } = W;
    dist.fill(Infinity); pred.fill(-1); pt.fill(-1); nd.fill(Infinity); na.fill(-1); heap.clear();
    W.src = src; nd[src] = 0;
    const h0 = head[src];
    for (let k = h0, e = head[src + 1]; k < e; k++) {
      const p = fromArc >= 0 ? tpen[tOff[fromArc] + k - h0] : 0;
      if (p === INF) continue;
      const b = out[k], d = p + cost[b];
      if (d < dist[b]) { dist[b] = d; heap.push(d, b); }
    }
    while (heap.n) {
      const a = heap.pop(), da = heap.top;
      if (da > dist[a]) continue;
      const v = to[a];
      if (da < nd[v]) { nd[v] = da; na[v] = a; }
      if (v === target) break;
      if (isC[v]) continue;   // never route through a zone centroid
      const h = head[v], o = tOff[a] - h;
      for (let k = h, e = head[v + 1]; k < e; k++) {
        const p = tpen[o + k]; if (p === INF) continue;
        const b = out[k], d = da + p + cost[b];
        if (d < dist[b]) { dist[b] = d; pred[b] = a; pt[b] = o + k; heap.push(d, b); }
      }
    }
  }
  // Arcs from W.src to node v on the last search, or null if v was not reached
  function pathTo(G, W, v) {
    if (v === W.src) return [];
    let a = W.na[v];
    if (a < 0) return null;
    const p = [];
    while (a >= 0 && p.length <= G.M) { p.push(a); a = W.pred[a]; }
    return p.reverse();
  }
  // Node-array interface for other callers: dist and pred per node (pred = arc arriving on the best path)
  // Each pred array keeps its own search tree, so trace() follows the search that filled that array.
  // Node predecessors alone do not make a valid tree when routing is edge-based.
  const trees = new WeakMap();
  function dijkstra(G, cost, src, dist, pred, heap, target = -1) {
    let W = trees.get(pred);
    if (!W || W.G !== G) trees.set(pred, W = Object.assign(workspace(G), { G }));
    spt(G, cost, src, W, target);
    dist.set(W.nd); pred.set(W.na);
  }
  // Arcs from src to dst on the tree that dijkstra() left in pred; null if unreached or pred is foreign
  function trace(G, pred, src, dst) {
    const W = trees.get(pred);
    if (!W || W.G !== G || W.src !== src) return null;
    return pathTo(G, W, dst);
  }
  function resolveRoute(G, r, cost = G.ffc) {
    const wp = (r.wp || []).map(id => G.idx.get(id)).filter(i => i != null);
    const W = workspace(G), arcs = [];
    for (let k = 1; k < wp.length; k++) {
      const last = arcs.length ? arcs[arcs.length - 1] : -1, from = last >= 0 && G.arcs[last].to === wp[k - 1] ? last : -1;
      spt(G, cost, wp[k - 1], W, wp[k], from);
      // A waypoint that can only be left by a banned turn: start that leg afresh rather than lose the route
      if (from >= 0 && !isFinite(W.nd[wp[k]])) spt(G, cost, wp[k - 1], W, wp[k]);
      const p = isFinite(W.nd[wp[k]]) ? pathTo(G, W, wp[k]) : null;
      if (p) arcs.push(...p);
    }
    return arcs;
  }

  /* ---------- demand ---------- */
  // A zone makes no trips before its opening year; its reduction (%) is the share of vehicle trips
  // replaced by public transport and active modes. Gateways carry counts, so no reduction there.
  /* Staging (scenario-matrix): sc.buildout % of the development zones is built in this scenario; a zone's own
     z.buildout % (e.g. a committed development part-built by the scenario year) applies on top. */
  function stageOf(z, sc) {
    let k = z && z.buildout != null && z.buildout !== '' && isFinite(+z.buildout) ? U.clamp(+z.buildout, 0, 100) / 100 : 1;
    if (sc && sc.buildout != null && sc.buildout !== '' && isFinite(+sc.buildout) && zoneClass(z) === 'development') k *= U.clamp(+sc.buildout, 0, 100) / 100;
    return k;
  }
  /* Per-land-use trip-end factors fitted to counts (calibrate-workflow): Store.P.luFactors[period] = { <LANDUSES key>: f,
     gateways: f }. They scale the land-use trips of existing-land-use zones (z.existing, class existing) and the
     counts of estimated gateways (no gwSrc) only, as matrix estimation does: development and committed zones keep
     their own trip rates, and gateways set from counts keep their counts. Bad values count as 1. */
  function luFactorsOf(period) {
    const P = typeof Store !== 'undefined' ? Store.P : null, f = P && P.luFactors && P.luFactors[period];
    return f && typeof f === 'object' && !Array.isArray(f) ? f : null;
  }
  const factorOf = (F, k) => { const v = F ? +F[k] : NaN; return v >= 0 && isFinite(v) ? v : 1; };
  const lfEligible = z => !!z && z.kind !== 'gateway' && z.existing === true && zoneClass(z) === 'existing';
  function tripEnds(z, S, period, sc = curSc()) {
    let O = 0, D = 0;
    if (+z.year > 0 && yearOf(sc, S) < +z.year) return { O, D };
    const F = luFactorsOf(period);
    if (z.kind === 'gateway') {
      const g = z.gw || {}, gf = growth(S, sc) * (F && !z.gwSrc ? factorOf(F, 'gateways') : 1);
      O = (+g[period + 'in'] || 0) * gf; D = (+g[period + 'out'] || 0) * gf;
    } else if (z.tg && z.tg.on && z.tgLoad && typeof z.tgLoad === 'object') {
      /* A switched-on trip-generation table (landuse-plus: T4 after pass-by, plus freight) replaces land use × rates +
         z.extra; the reduction and staging still apply, as they do to every land-use zone */
      O = Math.max(0, +z.tgLoad[period + 'out'] || 0); D = Math.max(0, +z.tgLoad[period + 'in'] || 0);
      const k = (1 - U.clamp(+z.reduction || 0, 0, 100) / 100) * stageOf(z, sc);
      O *= k; D *= k;
    } else {
      const lf = F && lfEligible(z) ? F : null;
      for (const k of Object.keys(LANDUSES)) {
        const q = +((z.lu || {})[k]) || 0; if (!q) continue;
        const r = ((S.rates || {})[k] || {})[period]; if (!r) continue;
        const T = q / LANDUSES[k].per * r.rate * (lf ? factorOf(lf, k) : 1);
        D += T * r.inPct / 100; O += T * (1 - r.inPct / 100);
      }
      O += +((z.extra || {})[period + 'out']) || 0;
      D += +((z.extra || {})[period + 'in']) || 0;
      const k = (1 - U.clamp(+z.reduction || 0, 0, 100) / 100) * stageOf(z, sc);
      O *= k; D *= k;
    }
    return { O, D };
  }

  function demand(sc, G, period, S) {
    const Z = G.zones.length, O = new Float64Array(Z), D = new Float64Array(Z), T = new Float64Array(Z * Z);
    G.zones.forEach((z, i) => { const t = tripEnds(z, S, period, sc); O[i] = t.O; D[i] = t.D; });
    let note = '', manual = false;
    const man = sc.odManual && sc.odManual[period];
    if (man && man.zones) {
      manual = true;
      const pos = new Map(man.zones.map((id, i) => [id, i]));
      // The C2 zone rules hold here too: nothing before a zone's opening year, and its reduction applies
      // to its row and column (land-use zones only, as in tripEnds)
      const yr = yearOf(sc, S), kz = G.zones.map(z => +z.year > yr ? 0 : z.kind === 'gateway' ? 1 : (1 - U.clamp(+z.reduction || 0, 0, 100) / 100) * stageOf(z, sc));
      for (let i = 0; i < Z; i++) for (let j = 0; j < Z; j++) {
        const a = pos.get(G.zones[i].id), b = pos.get(G.zones[j].id);
        if (a != null && b != null && i !== j) T[i * Z + j] = (+((man.m[a] || [])[b]) || 0) * kz[i] * kz[j];
      }
      O.fill(0); D.fill(0);
      for (let i = 0; i < Z; i++) for (let j = 0; j < Z; j++) { O[i] += T[i * Z + j]; D[j] += T[i * Z + j]; }
      note = 'Using the manual OD matrix for this period.';
    } else if (Z > 1) {
      const W = workspace(G), sk = new Float64Array(Z * Z).fill(Infinity);
      for (let i = 0; i < Z; i++) {
        if (O[i] <= 0) continue;
        spt(G, G.ffc, G.zoneNode[i], W);
        for (let j = 0; j < Z; j++) sk[i * Z + j] = W.nd[G.zoneNode[j]];
      }
      // Zone trip ends are fixed by land use. Gateway counts are the totals crossing the boundary:
      // the part not used by zone trips passes through to other gateways.
      const isG = G.zones.map(z => z.kind === 'gateway');
      const F = k => isFinite(sk[k]) ? Math.exp(-S.gravityBeta * sk[k] / 60) : 0;
      const notes = [];
      let lost = 0;
      // 1. trips leaving each zone, to zones (by attraction) and gateways (by outbound count)
      for (let i = 0; i < Z; i++) {
        if (isG[i] || O[i] <= 0) continue;
        let s = 0;
        for (let j = 0; j < Z; j++) if (j !== i) s += D[j] * F(i * Z + j);
        if (s <= 0) { lost += O[i]; continue; }
        for (let j = 0; j < Z; j++) if (j !== i) T[i * Z + j] += O[i] * D[j] * F(i * Z + j) / s;
      }
      // 2. trips arriving at each zone not already supplied by other zones come in through the gateways
      for (let j = 0; j < Z; j++) {
        if (isG[j] || D[j] <= 0) continue;
        let got = 0; for (let i = 0; i < Z; i++) if (!isG[i]) got += T[i * Z + j];
        const rem = D[j] - got; if (rem <= 0.5) continue;
        let s = 0; for (let i = 0; i < Z; i++) if (isG[i]) s += O[i] * F(i * Z + j);
        if (s <= 0) continue;
        for (let i = 0; i < Z; i++) if (isG[i]) T[i * Z + j] += rem * O[i] * F(i * Z + j) / s;
      }
      // 3. the rest of each gateway count is through traffic, favouring gateways on the far side of the area
      const yr = yearOf(sc, S), gz = G.zones.filter(z => z.kind === 'gateway' && !(+z.year > yr));   // open gateways only
      let diag = 1;
      for (const a of gz) for (const b of gz) diag = Math.max(diag, U.dist([a.lat, a.lng], [b.lat, b.lng]));
      const Or = new Float64Array(Z), Dr = new Float64Array(Z), f1 = new Float64Array(Z * Z);
      const over = [];
      for (let g = 0; g < Z; g++) {
        if (!isG[g]) continue;
        let out = 0, inn = 0;
        for (let j = 0; j < Z; j++) { if (!isG[j]) { out += T[g * Z + j]; inn += T[j * Z + g]; } }
        Or[g] = Math.max(0, O[g] - out); Dr[g] = Math.max(0, D[g] - inn);
        if (inn > D[g] * 1.05 + 20) over.push(G.zones[g].name);
        for (let j = 0; j < Z; j++) {
          if (!isG[j] || j === g || !isFinite(sk[g * Z + j])) continue;
          const d = U.dist([G.zones[g].lat, G.zones[g].lng], [G.zones[j].lat, G.zones[j].lng]) / diag;
          f1[g * Z + j] = d < 0.3 ? 0 : d * d;
        }
      }
      const s1 = furness(Z, Or, Dr, f1, T);
      if (lost > 1) notes.push(`${U.fmt.int(lost)} veh/h from zones had nowhere to go. Add gateways or destination land uses.`);
      if (over.length) notes.push(`Zone trips leaving through ${over.slice(0, 3).join(', ')} exceed the gateway count; raise the count or check the land use.`);
      if (s1 && Math.abs(s1.o - s1.d) / Math.max(s1.o, s1.d) > 0.08) notes.push(`Through traffic entering (${U.fmt.int(s1.o)}) and leaving (${U.fmt.int(s1.d)}) differs and was balanced.`);
      if (!gz.length) notes.push('There are no gateways, so all trips stay inside the area.');
      note = notes.join(' ');
    }
    const me = applyME(G, period, T, yearOf(sc, S));
    let O0 = null, D0 = null;
    if (me) {
      // O and D follow the factored matrix, so they match T and the assigned flows; O0 / D0 keep the prior
      note = (note ? note + ' ' : '') + me;
      O0 = O.slice(); D0 = D.slice(); O.fill(0); D.fill(0);
      for (let i = 0; i < Z; i++) for (let j = 0; j < Z; j++) { O[i] += T[i * Z + j]; D[j] += T[i * Z + j]; }
    }
    return { Z, O, D, T, note, manual, O0, D0 };
  }
  /* Matrix estimation factors (contract C2) scale base-year OD pairs: both ends existing land use or
     gateways. New development keeps its unadjusted trips. Returns a note, or '' if nothing changed.
     year (optional, the scenario year): when the fit recorded additive deltas (Store.P.me.info[period].delta =
     { '<zoneId>><zoneId>': veh/h }, with its baseYear) and year is not that base year, the estimation is carried forward
     as T = max(0, T + delta) on the same pairs (docs/spec/matrix-estimation: futures inherit meDelta), so growth and
     new trips are not multiplied by base-year corrections. Without a year, or for records without deltas, the factors. */
  function applyME(G, period, T, year = null) {
    const P = typeof Store !== 'undefined' ? Store.P : null, me = P && P.me && P.me[period], Z = G.zones.length;
    if (!me || typeof me !== 'object' || Z < 2) return '';
    const pos = new Map();
    G.zones.forEach((z, i) => { if (z.kind === 'gateway' || z.existing === true) pos.set(z.id, i); });
    const info = P.me.info && typeof P.me.info === 'object' ? P.me.info[period] : null, by = info && +info.baseYear > 0 ? +info.baseYear : null;
    const add = year != null && +year > 0 && by != null && +year !== by && info.delta && typeof info.delta === 'object' && !Array.isArray(info.delta) ? info.delta : null;
    let n = 0, before = 0, after = 0;
    if (add) {
      for (const [key, v] of Object.entries(add)) {
        const s = key.indexOf('>'), d = +v;
        if (s < 0 || !isFinite(d) || v === '' || v == null) continue;
        const i = pos.get(key.slice(0, s)), j = pos.get(key.slice(s + 1));
        if (i == null || j == null || i === j) continue;
        before += T[i * Z + j]; T[i * Z + j] = Math.max(0, T[i * Z + j] + d); after += T[i * Z + j]; n++;
      }
      return n ? `Matrix estimation (base year ${by}): ME adjustment carried forward as additive delta to ${U.fmt.int(n)} OD pair${n === 1 ? '' : 's'} (${U.fmt.int(before)} → ${U.fmt.int(after)} veh/h).` : '';
    }
    for (const [key, v] of Object.entries(me)) {
      const s = key.indexOf('>'), f = +v;
      if (s < 0 || !(f >= 0) || !isFinite(f)) continue;
      const i = pos.get(key.slice(0, s)), j = pos.get(key.slice(s + 1));
      if (i == null || j == null || i === j || !(T[i * Z + j] > 0)) continue;
      before += T[i * Z + j]; T[i * Z + j] *= f; after += T[i * Z + j]; n++;
    }
    return n ? `Matrix estimation factors applied to ${U.fmt.int(n)} OD pair${n === 1 ? '' : 's'} (${U.fmt.int(before)} → ${U.fmt.int(after)} veh/h).` : '';
  }
  // Doubly-constrained Furness balancing; adds the trips into T. Returns totals, or null if a side is empty.
  function furness(Z, O, D, f, T) {
    let so = 0, sd = 0;
    for (let i = 0; i < Z; i++) { so += O[i]; sd += D[i]; }
    if (!(so > 0 && sd > 0)) return null;
    const Dt = D.map(v => v * so / sd);
    const A = new Float64Array(Z).fill(1), B = new Float64Array(Z).fill(1);
    for (let it = 0; it < 60; it++) {
      for (let i = 0; i < Z; i++) { let s = 0; for (let j = 0; j < Z; j++) s += B[j] * Dt[j] * f[i * Z + j]; A[i] = s > 0 ? 1 / s : 0; }
      for (let j = 0; j < Z; j++) { let s = 0; for (let i = 0; i < Z; i++) s += A[i] * O[i] * f[i * Z + j]; B[j] = s > 0 ? 1 / s : 0; }
    }
    for (let i = 0; i < Z; i++) for (let j = 0; j < Z; j++) T[i * Z + j] += A[i] * O[i] * B[j] * Dt[j] * f[i * Z + j];
    return { o: so, d: sd };
  }

  /* ---------- results by site, and the model status metadata ---------- */
  const SELECT_LABEL = 'Frank-Wolfe path splits are not unique; select-link results are indicative.';
  // res.sites: each junction site with its legs (keys of the arcs entering and leaving on each), its
  // controller node, and for roundabout sites (one controller per entry) the entries reported together.
  // res.siteTurns: movements entry → exit through the site (veh/h), traced along the assignment's paths (sm = the
  // traced flows { x, dev } by site-movement slot); dev = development traffic in the movement.
  function siteResults(sc, G, xT, pf, nodesRes, sm = null) {
    if (!G.sites.length) return null;
    const sites = {}, siteTurns = {}, J = junctionSet(G.S);
    G.sites.forEach(({ site, members }, s) => {
      const st = G.sm.list[s], inc = st.inc, exits = st.exits;
      const ctl = [...G.ctl.values()].find(c => c.site === site.id);
      const legs = ctl ? ctl.legs : siteLegs(G, inc, exits), legIn = new Map(), legOut = new Map();
      for (const l of legs) { l.ins.forEach(i => legIn.set(i, l.k)); l.outs.forEach(i => legOut.set(i, l.k)); }
      const pseudo = { arcs: inc, legIn, legOut }, rows = [];
      if (sm) inc.forEach((a, e) => {
        for (let x = 0; x < st.nx; x++) {
          const k = st.off + e * st.nx + x, f = sm.x[k], v = U.r1(f / pf); if (!(v > 0)) continue;
          const b = exits[x], dv = sm.dev ? U.r1(sm.dev[k] / pf) : 0;
          rows.push({ from: G.arcs[a].key, to: G.arcs[b].key, mv: siteMv(G, pseudo, a, b), v, pcu: U.r1(f), ...(dv > 0 ? { dev: Math.min(v, dv) } : {}) });
        }
      });
      else { const ex = siteExits(G, pseudo, xT); for (const a of inc) for (const [b, f] of ex.get(a) || []) { const v = U.r1(f / pf); if (v > 0) rows.push({ from: G.arcs[a].key, to: G.arcs[b].key, mv: siteMv(G, pseudo, a, b), v, pcu: U.r1(f) }); } }
      if (rows.length) siteTurns[site.id] = rows;
      const names = [...new Set(legs.slice().sort((p, q) => q.rank - p.rank).map(l => l.name).filter(Boolean))];
      const repN = ctl ? ctl.n : members.slice().sort((a, b) => G.deg[b] - G.deg[a] || (G.ids[a] < G.ids[b] ? -1 : 1))[0];
      let result = null;
      if (!ctl) {   // per-node controllers (roundabouts): the approaches from outside the site, together
        const keys = new Set(inc.map(i => G.arcs[i].key)), appr = [];
        for (const n of members) { const nr = nodesRes[G.ids[n]]; if (nr) for (const a of nr.appr) if (keys.has(a.key)) appr.push(a); }
        if (appr.length) {
          const ctlA = appr.filter(a => a.ctrl !== 'free'), sv = ctlA.reduce((s, a) => s + a.v, 0), avg = sv > 0 ? ctlA.reduce((s, a) => s + a.v * a.delay, 0) / sv : 0;
          const type = (nodesRes[G.ids[members.find(n => nodesRes[G.ids[n]])]] || {}).type || 'roundabout', LOS = STANDARDS.los[J.method];
          const th = type === 'signals' ? LOS.signals : type === 'roundabout' ? LOS.roundabout : LOS.sign, los = (J.method === 'rta' ? losLt : losOf)(avg, th);
          result = { type, appr, delay: U.r1(avg), avgDelay: U.r1(avg), delayBasis: 'average', dos: U.r3(Math.max(0, ...ctlA.map(a => a.dos))), vol: Math.round(appr.reduce((s, a) => s + a.v, 0)), los, intersectionLos: los, losText: los };
        }
      }
      sites[site.id] = { id: site.id, name: site.name || names.slice(0, 3).join(' / ') || 'Unnamed junction', nodes: members.map(n => G.ids[n]), rep: G.ids[repN],
        type: ctl ? ctl.type : siteControl(sc, site), atSiteNo: site.atSiteNo ?? null, one: !!ctl,
        legs: legs.map(l => ({ k: l.k, name: l.name, b: l.b, ins: l.ins.map(i => G.arcs[i].key), outs: l.outs.map(i => G.arcs[i].key) })), ...(result ? { result } : {}) };
    });
    return { sites, siteTurns, siteTurnsBasis: sm ? 'paths' : 'shares' };
  }

  /* Model status (docs/regulatory/reg-model-status): Uncalibrated; Calibrated (matrix estimation fitted, Store.P.me.info);
     Validated (Store.P.validation[period] = { pass: true, independent: true, category, standard, … } written by the
     validation step, i.e. independent counts meet the targets). The TMDG category and justification come from
     settings.tmdg, the criteria from settings.calib.standard ('tmdg' by default). A record made for another category
     or standard than the settings now name is out of date: the status falls back to Calibrated or Uncalibrated and
     says why (stale), so an old pass is never shown under the current category. Records without a category or
     standard (older projects) are taken as made for the current settings. */
  const STD_NAME = { tmdg: 'TMDG', tag: 'UK TAG M3.1', rms: 'RMS 2013', atap: 'ATAP' };
  function modelStatus(P = Store.P, period = 'AM') {
    const S = (P && P.settings) || {}, tm = S.tmdg || {}, cat = String(tm.category || 'E').toUpperCase(), just = String(tm.justification || '').trim();
    const std = String((S.calib && S.calib.standard) || 'tmdg');
    const info = P && P.me && P.me.info && P.me.info[period], val = P && P.validation && P.validation[period];
    const vCat = val && val.category != null && val.category !== '' ? String(val.category).toUpperCase() : null, vStd = val && val.standard ? String(val.standard) : null;
    const matches = !!val && (vCat == null || vCat === cat) && (vStd == null || vStd === std);
    const status = val && val.pass === true && val.independent && matches ? 'validated' : info ? 'calibrated' : 'uncalibrated';
    const short = { uncalibrated: 'Uncalibrated', calibrated: 'Calibrated', validated: 'Validated' }[status];
    const stdText = s => (s === 'tmdg' ? `TMDG Category ${vCat || cat}` : STD_NAME[s] || s);
    const label = { uncalibrated: 'Uncalibrated', calibrated: 'Calibrated: base matched to counts used in calibration',
      validated: `Validated: independent validation counts meet the ${stdText(vStd || std)} targets` }[status];
    const stale = val && val.pass === true && val.independent && !matches
      ? `The validation was recorded for ${vStd && vStd !== 'tmdg' ? STD_NAME[vStd] || vStd : `TMDG Category ${vCat || cat}`}; the settings now name ${std !== 'tmdg' ? STD_NAME[std] || std : `TMDG Category ${cat}`}. Validate the base year again.` : '';
    return { status, short, label, period, category: status === 'validated' && vCat ? vCat : cat, standard: std, justification: just, date: (val && val.date) || (info && info.date) || null,
      ...(stale ? { stale } : {}),
      footer: `Model status (${period}): ${short} · TMDG category ${cat}${just ? '' : ' (justification not given)'}`, standing: STANDARDS.tmdg.standing };
  }
  const SRC_NAME = { nzta: 'NZTA state highway counts', at: 'Auckland Transport counts', user: 'Your counts', fc: 'AT Future Connect', controlled: 'AT controlled intersections' };
  // Metadata every result carries for the model-status badge: convergence, data vintages, calibration status
  function modelMeta(sc, period, S, c) {
    const P = Store.P, L = (P && P.data && P.data.loaded) || {}, day = t => (t ? new Date(t).toISOString().slice(0, 10) : null), vintages = {};
    for (const k of Object.keys(SRC_NAME)) if (L[k] && L[k].at) vintages[k] = { source: L[k].source || SRC_NAME[k], date: day(L[k].at), n: L[k].n ?? null, ...(L[k].licence ? { licence: L[k].licence } : {}) };
    if (L.cas && L.cas.at) vintages.cas = { source: 'NZTA Crash Analysis System', date: day(L.cas.at), n: L.cas.n ?? null, window: L.cas.label || null };
    return { status: modelStatus(P, period), convergence: { converged: c.converged, hitMax: c.hitMax, gap: c.gap, gapTol: c.gapTol, it: c.it, note: c.convNote },
      vintages, baseYear: S.baseYear, year: yearOf(sc, S), growthBasis: growthBasis(S), losMethod: junctionSet(S).method, capMethod: capMethodOf(S) };
  }

  /* ---------- assignment ---------- */
  /* opts: period, onProgress, gapTol, maxIter; settings (a settings object used instead of Store.S, for
     sensitivity tests); demandFactor (scales the whole OD matrix); select = { arc: key } | { turn: [fromKey,
     toKey] } | { zone: zoneId } for an on-the-fly select-link / select-turn / select-zone analysis, tracked
     through the assignment so it works for any number of zones (docs/spec/dev-tracking).
     Development tracking: every OD pair carries a class, development if either end is a development zone,
     committed if either end is committed (and neither is development), otherwise existing; each class has its
     own link flows (and development its own turn flows) on the same costs, averaged with the same steps. */
  // Buses an hour on a route in a period: the period's own frequency (GTFS: r.freqs = { AM, PM }), else r.freq
  const busFreq = (r, period) => { const f = r && r.freqs && typeof r.freqs === 'object' ? r.freqs[period] : null; return f != null && f !== '' && isFinite(+f) ? Math.max(0, +f) : Math.max(0, +(r && r.freq) || 0); };
  // PCU per bus for the route preload: settings.busPcu, or the earlier engine value for a settings object without it
  const busPcuOf = S => (S && +S.busPcu > 0 && isFinite(+S.busPcu) ? +S.busPcu : STANDARDS.bus.legacy);
  async function assign(sc, opts = {}) {
    const S = opts.settings || Store.S, period = opts.period || S.period, prog = opts.onProgress || (() => {});
    const t0 = performance.now();
    const G = buildGraph(sc, S, period);
    const dem = demand(sc, G, period, S);
    const dF = +opts.demandFactor > 0 ? +opts.demandFactor : 1;
    if (dF !== 1) for (let k = 0; k < dem.T.length; k++) dem.T[k] *= dF;
    const pf = pcuF(S), Z = dem.Z, M = G.M;
    const pre = new Float64Array(M), busPcu = busPcuOf(S);
    const routes = Object.values(sc.routes || {});
    const routeArcs = routes.map(r => resolveRoute(G, r));
    routes.forEach((r, k) => { const f = busFreq(r, period); for (const ai of routeArcs[k]) if (!G.arcs[ai].bus) pre[ai] += f * busPcu; });
    /* Additive generalised cost per arc (s): time-of-use charges (L.toll via buildGraph, and opts.arcPenalty keyed by
       arc key or link id). Fixed per traversal, so they add no curvature; they are not time (see results). */
    const apen = new Float64Array(M), ap = opts.arcPenalty && typeof opts.arcPenalty === 'object' ? opts.arcPenalty : null;
    let hasPen = false;
    for (let a = 0; a < M; a++) {
      const A = G.arcs[a]; if (A.conn) continue;
      let p = A.toll || 0;
      if (ap) { const v = ap[A.key] != null ? +ap[A.key] : ap[A.link] != null ? +ap[A.link] : 0; if (v > 0 && isFinite(v)) p += v; }
      if (p > 0) { apen[a] = p; hasPen = true; }
    }
    const x = new Float64Array(M), y = new Float64Array(M), cost = new Float64Array(M), xt = new Float64Array(M);
    const W = workspace(G), nT = G.nT, { nd, na, pred, pt } = W;
    // Turn flows ride along with the link flows; pc = the fixed turn penalties (banned turns carry nothing)
    const xT = new Float64Array(nT), yT = new Float64Array(nT), pc = G.tpen.map(p => p === Infinity ? 0 : p);
    const odList = [], zc = G.zones.map(zcIdx), isGw = G.zones.map(z => z.kind === 'gateway');
    const clsTrips = [0, 0, 0], devGw = new Map(), devZone = new Map();
    // Selection: a zone (OD pairs with either end there), an arc or a turn (paths that use it)
    const sel = opts.select || null, selZone = sel && sel.zone != null ? G.zones.findIndex(z => z.id === sel.zone) : -1;
    const selArc = sel && sel.arc != null && G.keyIdx.has(sel.arc) ? G.keyIdx.get(sel.arc) : -1;
    let selTurn = -1;
    if (sel && Array.isArray(sel.turn)) { const a = G.keyIdx.get(sel.turn[0]), b = G.keyIdx.get(sel.turn[1]); selTurn = a != null && b != null ? turnIdx(G, a, b) : -1; }
    const selOn = !!sel && (selZone >= 0 || selArc >= 0 || selTurn >= 0);
    let trips = 0, nOD = 0;
    for (let i = 0; i < Z; i++) {
      const l = [];
      for (let j = 0; j < Z; j++) {
        const q = dem.T[i * Z + j]; if (!(q > 0.0005)) continue;
        const c = Math.max(zc[i], zc[j]);
        l.push([j, q * pf, c, nOD++]); trips += q; clsTrips[c] += q;
        if (c === 2) {   // development trips: where they cross the boundary, and each development zone's own trips
          const g = isGw[i] ? i : isGw[j] ? j : -1;
          if (g >= 0) { const r = devGw.get(g) || { in: 0, out: 0 }; if (g === i) r.in += q; else r.out += q; devGw.set(g, r); }
          if (zc[i] === 2) { const r = devZone.get(i) || { O: 0, D: 0 }; r.O += q; devZone.set(i, r); }
          if (zc[j] === 2) { const r = devZone.get(j) || { O: 0, D: 0 }; r.D += q; devZone.set(j, r); }
        }
      }
      odList.push(l);
    }
    const storePaths = Z <= 300;
    const paths = storePaths ? new Map() : null;
    let unserved = 0;
    const setXt = () => { for (let a = 0; a < M; a++) xt[a] = x[a] + pre[a]; };
    const costs = () => { for (let a = 0; a < M; a++) cost[a] = arcTime(G.arcs[a], x[a] + pre[a], S) + apen[a]; };
    // Tracked flows [x, s (conjugate direction), y (all-or-nothing)]: committed and development link flows, development
    // turn flows, and the selection's link, turn and OD flows; all are averaged with the main flows' steps
    const trk = [], track = n => { const t = [new Float64Array(n), new Float64Array(n), new Float64Array(n)]; trk.push(t); return t; };
    const [xC1, , yC1] = track(M), [xC2, , yC2] = track(M), [xTd, , yTd] = track(nT);
    const none = [null, null, null], [xS, , yS] = selOn ? track(M) : none, [xTS, , yTS] = selOn ? track(nT) : none, [xOD, , yOD] = selOn ? track(nOD) : none;
    /* Junction-site movements traced along each path (entry arc → exit arc), all traffic and development traffic.
       Walking a path backwards, an exit arc opens its site and the site's entry arc closes it; an arc between two
       sites closes the downstream one before it opens the upstream one. */
    const SM = G.sm && G.sm.n > 0 ? G.sm : null, [xM, , yM] = SM ? track(SM.n) : none, [xMd, , yMd] = SM ? track(SM.n) : none;
    const pend = SM ? new Int32Array(SM.list.length).fill(-1) : null, opened = [];
    if (SM) G.smX = xM;
    const aon = np => {
      y.fill(0); yT.fill(0); unserved = 0;
      for (const t of trk) t[2].fill(0);
      for (let i = 0; i < Z; i++) {
        const list = odList[i]; if (!list.length) continue;
        spt(G, cost, G.zoneNode[i], W);
        for (const [j, q, c, od] of list) {
          const dn = G.zoneNode[j];
          if (!isFinite(nd[dn])) { unserved += q; continue; }
          const p = np ? [] : null, yc = c === 2 ? yC2 : c === 1 ? yC1 : null;
          for (let ai = na[dn]; ai >= 0; ai = pred[ai]) {
            y[ai] += q;
            if (pt[ai] >= 0) { yT[pt[ai]] += q; if (c === 2) yTd[pt[ai]] += q; }
            if (yc) yc[ai] += q;
            if (p) p.push(ai);
            if (SM) {
              let s = SM.entS[ai];
              if (s >= 0 && pend[s] >= 0) { const st = SM.list[s], k = st.off + SM.entI[ai] * st.nx + pend[s]; yM[k] += q; if (c === 2) yMd[k] += q; pend[s] = -1; }
              s = SM.extS[ai];
              if (s >= 0) { pend[s] = SM.extI[ai]; opened.push(s); }
            }
          }
          if (opened.length) { for (const s of opened) pend[s] = -1; opened.length = 0; }
          if (selOn) {
            let hit = i === selZone || j === selZone;
            if (!hit) for (let ai = na[dn]; ai >= 0; ai = pred[ai]) if (ai === selArc || (selTurn >= 0 && pt[ai] === selTurn)) { hit = true; break; }
            if (hit) { yOD[od] += q; for (let ai = na[dn]; ai >= 0; ai = pred[ai]) { yS[ai] += q; if (pt[ai] >= 0) yTS[pt[ai]] += q; } }
          }
          if (np) np.push([i * Z + j, p.reverse(), q]);
        }
      }
    };
    // Path sets are Maps od → Map(arc list → {arcs, w}); add(np, w, into) adds an AON result scaled by w
    const addPaths = (np, w, into = paths) => {
      for (const [od, arcsP, q] of np) {
        let m = into.get(od); if (!m) into.set(od, m = new Map());
        const k = arcsP.join('.'), p = m.get(k);
        if (p) p.w += w * q; else m.set(k, { arcs: arcsP, w: w * q });
      }
    };
    const scalePaths = (set, f) => { for (const m of set.values()) for (const [kk, p] of m) { p.w *= f; if (p.w < 0.002) m.delete(kk); } };
    /* Conjugate Frank-Wolfe (Mitradjieva & Lindberg 2013): the search direction s mixes the previous one
       with the new all-or-nothing flows y, using the diagonal of the cost Jacobian, which converges much
       faster than plain Frank-Wolfe near equilibrium. Turn penalties are fixed costs, so no curvature. */
    const s = new Float64Array(M), sT = new Float64Array(nT), hs = new Float64Array(M);
    let sP = storePaths ? new Map() : null, hasS = false;
    const direction = np => {
      let al = 0;
      if (hasS) {
        let num = 0, den = 0;
        for (let a = 0; a < M; a++) {
          const ds = s[a] - x[a]; if (ds === 0) continue;
          const A = G.arcs[a], q = x[a] + pre[a], h = Math.max(1, q * 0.01);
          hs[a] = (arcTime(A, q + h, S) + apen[a] - cost[a]) / h;
          num += ds * hs[a] * (y[a] - x[a]); den += ds * hs[a] * (y[a] - s[a]);
        }
        al = den !== 0 ? num / den : 0;
        al = al > 0.99 ? 0.99 : al > 0 ? al : 0;
      }
      for (let a = 0; a < M; a++) s[a] = al * s[a] + (1 - al) * y[a];
      for (let t = 0; t < nT; t++) sT[t] = al * sT[t] + (1 - al) * yT[t];
      for (const [, st, yt] of trk) for (let k = 0; k < st.length; k++) st[k] = al * st[k] + (1 - al) * yt[k];
      if (np) { if (al > 0) scalePaths(sP, al); else sP = new Map(); addPaths(np, 1 - al, sP); }
      hasS = true;
    };
    const derivative = lam => {
      let r = 0;
      for (let a = 0; a < M; a++) { const d = s[a] - x[a]; if (d !== 0) r += d * (arcTime(G.arcs[a], x[a] + lam * d + pre[a], S) + apen[a]); }
      for (let t = 0; t < nT; t++) if (pc[t]) r += (sT[t] - xT[t]) * pc[t];
      return r;
    };
    const lineSearch = () => {
      if (derivative(1) <= 0) return 1;
      let lo = 0, hi = 1;
      for (let k = 0; k < 20; k++) { const m = (lo + hi) / 2; if (derivative(m) > 0) hi = m; else lo = m; }
      return (lo + hi) / 2;
    };

    /* Convergence (docs/spec/convergence): relative gap, plus the TAG stability measure P = share of road
       arcs (flow > 1 pcu/h) whose flow changed by less than 1% in the last update. With the stability rule
       on, a phase stops only when gap < gapTol and P > 98% for 4 consecutive iterations. */
    const CV = convSet(S, opts), conv = [], xPrev = new Float64Array(M), hist = [];
    let consecP = 0, kCap = 0, lastP = null, hitMax = false;
    const caps = () => { kCap++; updateNodeCaps(G, xt, CV.msa && kCap > 1 ? 1 / kCap : 1, xT); };
    G.mvDetail = false;   // lane rows for the junction tables are built on the last capacity update only
    setXt(); caps(); costs();
    let np = storePaths ? [] : null;
    aon(np); x.set(y); xT.set(yT); for (const [xx, , yy] of trk) xx.set(yy);
    if (np) addPaths(np, 1);
    let gap = 1, it = 1;
    const iterate = async (maxIt, tol = CV.gapTol, final = true) => {
      hasS = false;   // costs changed (signal timings): start the conjugate directions afresh
      consecP = 0; hitMax = false; kCap = 0;
      for (let k = 0; k < maxIt; k++) {
        setXt(); caps(); costs();
        np = storePaths ? [] : null;
        aon(np);
        let num = 0, den = 0, vh = 0;
        for (let a = 0; a < M; a++) { num += cost[a] * (x[a] - y[a]); den += cost[a] * x[a]; if (!G.arcs[a].conn) vh += x[a] * (cost[a] - apen[a]); }
        for (let t = 0; t < nT; t++) if (pc[t]) { num += pc[t] * (xT[t] - yT[t]); den += pc[t] * xT[t]; vh += pc[t] * xT[t]; }
        gap = den > 0 ? Math.abs(num / den) : 0;
        it++;
        conv.push({ it, gap: +gap.toPrecision(3), P: lastP == null ? null : U.r3(lastP), vht: U.r1(vh / pf / 3600) });
        if (gap < tol && k > 1 && (!CV.stable || !final || consecP >= CV.consec)) break;
        if (k === maxIt - 1) hitMax = true;
        direction(np);
        const lam = lineSearch();
        xPrev.set(x);
        for (let a = 0; a < M; a++) x[a] += lam * (s[a] - x[a]);
        for (let t = 0; t < nT; t++) xT[t] += lam * (sT[t] - xT[t]);
        for (const [xx, st] of trk) for (let k = 0; k < xx.length; k++) xx[k] += lam * (st[k] - xx[k]);
        let nP = 0, okP = 0;
        for (let a = 0; a < M; a++) {
          if (G.arcs[a].conn || !(xPrev[a] > 1)) continue;
          nP++; if (Math.abs(x[a] - xPrev[a]) < CV.chg * xPrev[a]) okP++;
        }
        lastP = nP ? okP / nP : 1;
        consecP = lastP > CV.share ? consecP + 1 : 0;
        hist.push(Float64Array.from(x)); if (hist.length > 6) hist.shift();
        if (storePaths) {
          scalePaths(paths, 1 - lam);
          for (const [od, m] of sP) {
            let pm = paths.get(od); if (!pm) paths.set(od, pm = new Map());
            for (const [kk, p] of m) { const e = pm.get(kk); if (e) e.w += lam * p.w; else pm.set(kk, { arcs: p.arcs, w: lam * p.w }); }
          }
        }
        if (it % 2 === 0) { prog(it, gap); await U.sleep(0); }
      }
    };
    /* Signal optimisation re-times the signals twice after the first equilibrium. Under the v2 rule
       (CV.stable) the first two phases only need a loose gap (they are followed by new timings) and the
       whole run shares one budget of maxIter iterations, the last phase getting what is left; before v2
       each phase had its own budget (maxIter, then maxIter / 3 twice) and the same target. */
    const sigNodes = [...G.ctl.values()].filter(c => c.type === 'signals' && !c.single), sigOpt = !!(S.signalOpt && sigNodes.length);
    const loose = Math.max(CV.gapTol, 1e-3), third = Math.max(3, Math.round(CV.maxIter / 3));
    if (CV.stable && sigOpt) await iterate(third, loose, false); else await iterate(CV.maxIter);
    // Optimise signal cycle and green splits to the assigned flows (Webster), then re-equilibrate
    if (sigOpt) {
      for (let round = 0; round < 2; round++) {
        setXt();
        for (const ctl of sigNodes) {
          const node = ctl.src || sc.nodes[G.ids[ctl.n]];   // a site's own cycle and fixed timing, or the node's
          if (node.fixedTiming) continue;
          const yv = [0.02, 0.02];
          for (const ai of ctl.arcs) { const a = G.arcs[ai]; yv[a.phase] = Math.max(yv[a.phase], xt[ai] / (a.gl * S.satFlow)); }
          const Y = Math.min(0.92, yv[0] + yv[1]);
          ctl.C = +node.cycle || Math.round(U.clamp((1.5 * ctl.lost + 5) / (1 - Y), 50, 140));
          const s0 = U.clamp(yv[0] / (yv[0] + yv[1]), 0.2, 0.8);
          ctl.split = [s0, 1 - s0];
          applySignal(ctl, G);
        }
        prog(it, gap, 'Optimising signal timings');
        if (!CV.stable) await iterate(Math.round(CV.maxIter / 3));
        else if (round === 0) await iterate(third, loose, false);
        else await iterate(Math.max(third, CV.maxIter - it + 1));
      }
    }
    // Reported capacities follow the final flows exactly (no damping)
    G.mvDetail = true; setXt(); updateNodeCaps(G, xt, 1, xT); costs();
    const converged = !hitMax && gap <= CV.gapTol;
    // Assignment noise: 95th percentile of the change in arc flow (veh/h) over the last five updates
    let noise95 = null;
    if (hist.length === 6) {
      const d = [];
      for (let a = 0; a < M; a++) if (!G.arcs[a].conn) d.push(Math.abs(hist[5][a] - hist[0][a]) / pf);
      d.sort((p, q) => p - q);
      noise95 = d.length ? U.r1(d[Math.min(d.length - 1, Math.round(0.95 * (d.length - 1)))]) : null;
    }
    const convNote = convText({ converged, hitMax, gap, it, gapTol: CV.gapTol, stable: CV.stable, share: CV.share, consec: CV.consec });

    /* results */
    const arcsRes = {};
    let vkt = 0, vht = 0, dly = 0, co2 = 0, over = 0, maxVC = 0, laneKm = 0, chargeH = 0;
    for (let a = 0; a < M; a++) {
      const A = G.arcs[a]; if (A.conn) continue;
      // t is time only: a charge (apen) steers routes but is money, not time
      const tot = xt[a], veh = x[a] / pf, cap = effCap(A, S), t = cost[a] - apen[a];
      const run = A.ff * (1 + S.bprAlpha * Math.pow(tot / A.capMid, S.bprBeta));
      const vc = tot / cap, spd = A.len / t * 3.6;
      // What sets the capacity: the mid-block, or the junction at the arc's downstream end (results-trust (c))
      const nodeCap = A.ctrl === 'sig' ? A.gl * S.satFlow * A.gC : A.ctrl === 'minor' || A.ctrl === 'rbt' ? (A.capN || A.capMid) : Infinity;
      const capCtl = nodeCap < A.capMid && A.node != null ? G.ctl.get(A.node) : null;
      arcsRes[A.key] = { v: U.r1(veh), pcu: U.r1(tot), cap: Math.round(cap), vc: U.r3(vc), t: U.r1(t), ff: U.r1(A.ff), spd: U.r1(spd), nd: U.r1(t - run) + 0, len: Math.round(A.len), los: losOf(vc, LOS_VC), bus: U.r1(pre[a] / busPcu),
        capMid: Math.round(A.capMid), capSrc: capCtl ? CAP_SRC[capCtl.type] || 'junction' : 'mid-block', ...(capCtl ? { capNode: G.ids[capCtl.n] } : {}), ...(apen[a] > 0 ? { toll: U.r1(apen[a]) } : {}) };
      if (apen[a] > 0) chargeH += veh * apen[a] / 3600;
      vkt += veh * A.len / 1000; vht += veh * t / 3600; dly += veh * (t - A.ff) / 3600;
      co2 += veh * A.len / 1000 * ef(spd, S) / 1e6;
      laneKm += A.lanes * A.len / 1000;
      if (vc > 1) over++;
      if (vc > maxVC) maxVC = vc;
    }
    // Turn penalties are time spent at junctions, so they count in VHT and delay as they do in routing
    let tpenH = 0;
    for (let t = 0; t < nT; t++) if (pc[t] && xT[t] > 0) tpenH += xT[t] / pf * pc[t] / 3600;
    vht += tpenH; dly += tpenH;
    const J = junctionSet(S), nodesRes = {};
    let worst = null;
    for (const ctl of G.ctl.values()) {
      const nr = junctionResult(G, ctl, xt, arcsRes, S, J, pf);
      nodesRes[G.ids[ctl.n]] = nr;
      if (!worst || nr.delay > worst.delay) worst = { id: G.ids[ctl.n], delay: nr.delay, los: nr.los };
    }
    /* Turning movements with traffic at every junction: nodes with ≥ 3 links, or a road node where a zone
       connects and makes a third arm (a development access). Zone movements are included (keys c:…), so
       turning totals add up to the approach volumes; rows under 0.05 veh/h are left out to keep saves small. */
    const turns = {}, connIn = new Map();
    for (const A of G.arcs) if (A.conn && !G.isC[A.to]) { const l = connIn.get(A.to); if (l) l.push(A.i); else connIn.set(A.to, [A.i]); }
    for (let n = 0; n < G.N; n++) {
      const ci = connIn.get(n) || [];
      if (G.isC[n] || (G.deg[n] < 3 && !(ci.length && G.deg[n] >= 2))) continue;
      const list = [];
      for (const a of [...G.into[n], ...ci]) for (let t = G.tOff[a]; t < G.tOff[a + 1]; t++) {
        const v = U.r1(xT[t] / pf);
        if (!(v > 0) || G.tpen[t] === Infinity) continue;
        const b = G.tTo[t], A = G.arcs[a], B = G.arcs[b], dv = U.r1(xTd[t] / pf);
        list.push({ from: A.key, to: B.key, mv: A.conn || B.conn ? turnType(A.inB, B.outB) : movement(G, a, b), v, pcu: U.r1(xT[t]), ...(dv > 0 ? { dev: Math.min(v, dv) } : {}) });
      }
      if (list.length) turns[G.ids[n]] = list;
    }
    /* Development tracking (dev-tracking): link flows by class where any is committed or development (other
       links are all existing traffic), each development zone's trips, and where development trips cross the
       boundary (gateway in = arriving through it, out = leaving through it). veh/h. */
    const cls = { names: ZCLASS, trips: clsTrips.map(U.r1), arcs: {} };
    for (let a = 0; a < M; a++) {
      const A = G.arcs[a]; if (A.conn) continue;
      const tot = x[a] / pf, c1 = xC1[a] / pf, c2 = xC2[a] / pf;
      if (tot > 0.05 && (c1 > 0.05 || c2 > 0.05)) cls.arcs[A.key] = [U.r1(Math.max(0, tot - c1 - c2)), U.r1(c1), U.r1(c2)];
    }
    const dev = { trips: U.r1(clsTrips[2]), zones: {}, gateways: {}, viaGateways: 0 };
    for (const [i, r] of devZone) dev.zones[G.zones[i].id] = { O: U.r1(r.O), D: U.r1(r.D) };
    for (const [g, r] of devGw) { dev.gateways[G.zones[g].id] = { in: U.r1(r.in), out: U.r1(r.out) }; dev.viaGateways += r.in + r.out; }
    dev.viaGateways = U.r1(dev.viaGateways);
    let select = null;
    if (sel) {
      if (!selOn) select = { kind: sel.zone != null ? 'zone' : sel.arc != null ? 'arc' : 'turn', error: 'The selected zone, link or turn is not in this network (or the turn is banned).' };
      else {
        const flows = {}, ods = [], sTurns = {};
        for (let a = 0; a < M; a++) if (!G.arcs[a].conn && xS[a] / pf > 0.05) flows[G.arcs[a].key] = U.r1(xS[a] / pf);
        // tot counts every OD pair; the list keeps pairs over 0.05 veh/h, largest first, at most 2,000
        let tot = 0, nSel = 0;
        for (let i = 0; i < Z; i++) for (const [j, , , od] of odList[i]) { const q = xOD[od] / pf; if (q > 1e-6) { tot += q; nSel++; } if (q > 0.05) ods.push({ o: G.zones[i].id, d: G.zones[j].id, q: U.r1(q) }); }
        for (const [id, list] of Object.entries(turns)) for (const r of list) { const t = turnIdx(G, G.keyIdx.get(r.from), G.keyIdx.get(r.to)), v = t >= 0 ? xTS[t] / pf : 0; if (v > 0.05) (sTurns[id] ||= []).push({ from: r.from, to: r.to, mv: r.mv, v: U.r1(v) }); }
        ods.sort((p, q) => q.q - p.q);
        select = { kind: selZone >= 0 ? 'zone' : selArc >= 0 ? 'arc' : 'turn', key: sel.zone ?? sel.arc ?? sel.turn, flows, turns: sTurns, ods: ods.slice(0, 2000), odPairs: nSel, tot: U.r1(tot), label: SELECT_LABEL };
      }
    }
    const siteRes = siteResults(sc, G, xT, pf, nodesRes, SM ? { x: xM, dev: xMd } : null);
    const routesRes = {};
    routes.forEach((r, k) => {
      let t = 0, len = 0;
      const ra = routeArcs[k];
      for (let q = 0; q < ra.length; q++) {
        const ai = ra[q], A = G.arcs[ai]; len += A.len; t += A.bus ? A.ff * 1.1 : cost[ai] - apen[ai];
        if (q) { const ti = turnIdx(G, ra[q - 1], ai); if (ti >= 0 && isFinite(G.tpen[ti])) t += G.tpen[ti]; }
      }
      t += len / 400 * 20;
      routesRes[r.id] = { len: Math.round(len), t: Math.round(t), spd: len > 0 ? U.r1(len / t * 3.6) : null, freq: busFreq(r, period) };
    });
    const zoneIds = G.zones.map(z => z.id);
    if (SM) delete G.smX;
    return {
      period, year: yearOf(sc, S), ts: Date.now(), it, gap, ms: Math.round(performance.now() - t0), stale: false,
      converged, hitMax, gapTol: CV.gapTol, maxIter: CV.maxIter, stableRule: CV.stable, convNote, conv, noise95, growth: { factor: growth(S, sc), basis: growthBasis(S) },
      junction: { method: J.method, label: STANDARDS.los[J.method].label, pff: J.pff, T: J.T, queueSpace: J.queueSpace, f95: J.f95, honest: JUNCTION_LABEL, capMethod: J.capMethod,
        ...(J.capMethod === 'movement' ? { capLabel: STANDARDS.gaps.label, rbtLabel: STANDARDS.rbtLanes.label } : {}) },
      arcs: arcsRes, nodes: nodesRes, turns, routes: routesRes, cls, dev, ...(select ? { select } : {}), ...(siteRes ? siteRes : {}),
      scenario: { id: sc.id, role: sc.role || null, buildout: sc.buildout ?? null, demandFactor: dF },
      meta: modelMeta(sc, period, S, { converged, hitMax, gap, gapTol: CV.gapTol, it, convNote }),
      kpi: { trips, vkt, vht, dly, spd: vht > 0 ? vkt / vht : 0, co2, over, maxVC, unserved: unserved / pf, worst, laneKm, ...(hasPen ? { chargeH } : {}) },
      ...(routes.length ? { busPcu } : {}),
      zoneIds, O: Array.from(dem.O), D: Array.from(dem.D), ...(dem.O0 ? { O0: Array.from(dem.O0), D0: Array.from(dem.D0) } : {}), T: Z <= 150 ? Array.from(dem.T, v => U.r1(v)) : null, note: dem.note, manual: dem.manual,
      arcKeys: G.arcs.map(a => a.key),
      paths: storePaths ? [...paths].map(([od, m]) => ({ i: (od / Z) | 0, j: od % Z, list: [...m.values()].filter(p => p.w > 0.01) })) : null,
    };
  }

  /* ---------- queries on results ---------- */
  function costFromRes(G, res) {
    const S = Store.S, c = new Float64Array(G.M);
    for (let i = 0; i < G.M; i++) {
      const a = G.arcs[i], r = res && res.arcs[a.key];
      c[i] = r ? r.t : arcTime(a, 0, S);
    }
    return c;
  }
  function travel(sc, res, fromId, toId) {
    const G = buildGraph(sc, Store.S, (res && res.period) || null), cost = costFromRes(G, res);
    const s = G.idx.get(fromId), t = G.idx.get(toId);
    if (s == null || t == null) return null;
    const W = workspace(G);
    spt(G, cost, s, W, t);
    if (!isFinite(W.nd[t])) return { none: true };
    const p = pathTo(G, W, t);
    let len = 0, ff = 0, run = 0;
    const pts = [];
    for (const ai of p) { const a = G.arcs[ai]; len += a.len; ff += a.ff; run += cost[ai]; pts.push(...(pts.length ? a.pts.slice(1) : a.pts)); }
    // time includes turn penalties; pen is their share
    return { arcs: p.map(i => G.arcs[i].key), pts, time: W.nd[t], ff, len, pen: Math.max(0, W.nd[t] - run) };
  }
  // Node times = the earliest arrival on any arc into the node
  function isochrone(sc, res, fromId) {
    const G = buildGraph(sc, Store.S, (res && res.period) || null), cost = costFromRes(G, res), s = G.idx.get(fromId);
    if (s == null) return null;
    const W = workspace(G);
    spt(G, cost, s, W);
    const times = new Map();
    for (let i = 0; i < G.N; i++) if (!G.isC[i] && isFinite(W.nd[i])) times.set(G.ids[i], W.nd[i]);
    return { from: fromId, times };
  }
  function selectLink(res, key) {
    if (!res || !res.paths || !res.arcKeys) return null;
    const ai = res.arcKeys.indexOf(key);
    if (ai < 0) return null;
    const flows = {}, ods = [];
    let tot = 0;
    for (const od of res.paths) {
      let q = 0;
      for (const p of od.list) {
        if (!p.arcs.includes(ai)) continue;
        q += p.w;
        for (const a of p.arcs) { const k = res.arcKeys[a]; flows[k] = (flows[k] || 0) + p.w; }
      }
      if (q > 0.05) { ods.push({ o: res.zoneIds[od.i], d: res.zoneIds[od.j], q }); tot += q; }
    }
    ods.sort((a, b) => b.q - a.q);
    return { key, flows, ods, tot };
  }

  /* ---------- extent of effects and difference plots (docs/spec/extent-and-difference) ---------- */
  // A link's Future Connect classes: conflated or set by the user (L.fc), otherwise from the road class (Spare Construct assumption)
  function fcOf(L, decade = false) {
    const f = L && L.fc;
    if (f && (f.gt || f.pt || f.freight || f.gt10 || f.pt10 || f.freight10)) {
      const pick = k => (decade && f[k + '10'] != null ? f[k + '10'] : f[k]) || null;
      return { gt: pick('gt'), pt: pick('pt'), freight: pick('freight'), basis: f.user ? 'set by the user' : f.src || 'AT Future Connect' };
    }
    const gt = { motorway: 'Motorway', ramp: 'Motorway', arterial: 'Arterial (from road class)', collector: 'Collector (from road class)', local: 'Local (from road class)' }[L && L.cls] || null;
    return { gt, pt: null, freight: null, basis: 'road class (Spare Construct assumption; no Future Connect class)' };
  }
  // The AT ITA 4.2.1.1 threshold for a link: 5% (GT motorway / strategic / primary / secondary arterial, PT RTN /
  // interim RTN / FTN, or freight level 1A / 1B / 2), 10% (collector), otherwise none
  function extentClass(fc) {
    const X = STANDARDS.extent, why = [];
    if (fc.gt && X.gt5.test(fc.gt)) why.push(`general traffic ${fc.gt}`);
    if (fc.pt && X.pt5.test(fc.pt)) why.push(`public transport ${fc.pt}`);
    if (fc.freight && X.freight5.test(fc.freight)) why.push(`freight ${fc.freight} (${X.freightBasis})`);
    if (why.length) return { p: X.p5, cls: '5%', why };
    if (fc.gt && X.gt10.test(fc.gt)) return { p: X.p10, cls: '10%', why: [`general traffic ${fc.gt}`] };
    return { p: null, cls: null, why: [] };
  }
  /* The extent test per directional link: Δ = with − without, p = Δ / without; a link is included when p is over its
     threshold and Δ ≥ the floor (20 veh/h, Spare Construct assumption against tiny bases); a junction (node or site) is
     included when any of its approaches is. sc: the with scenario (link classes and geometry; opts.scWithout for links
     that only exist without). opts: { floor, decade ('first' = first-decade classes) }. veh/h. */
  function extentTest(sc, without, withRes, opts = {}) {
    const X = STANDARDS.extent, floor = opts.floor != null ? +opts.floor : X.floor, decade = opts.decade === 'first', scW = opts.scWithout || null;
    if (!without || !withRes) return null;
    const keys = new Set([...Object.keys(without.arcs || {}), ...Object.keys(withRes.arcs || {})]), rows = [];
    for (const key of keys) {
      const id = key.slice(0, key.lastIndexOf(':')), L = sc.links[id] || (scW && scW.links[id]); if (!L) continue;
      const v0 = (without.arcs[key] || {}).v || 0, v1 = (withRes.arcs[key] || {}).v || 0, d = v1 - v0, p = v0 > 0 ? d / v0 : d > 0 ? Infinity : 0;
      const fc = fcOf(L, decade), ec = extentClass(fc), included = ec.p != null && p > ec.p && d >= floor;
      if (!(v0 > 0 || v1 > 0)) continue;
      rows.push({ key, link: id, name: L.name || '', dir: key.slice(-2), fc: { gt: fc.gt, pt: fc.pt, freight: fc.freight }, basis: fc.basis, without: U.r1(v0), with: U.r1(v1), d: U.r1(d),
        p: isFinite(p) ? U.r3(p) : null, pText: isFinite(p) ? `${p >= 0 ? '+' : '−'}${Math.abs(p * 100).toFixed(1)}%` : 'new', threshold: ec.p, cls: ec.cls, why: ec.why, included });
    }
    rows.sort((a, b) => (b.included - a.included) || b.d - a.d);
    const inc = new Set(rows.filter(r => r.included).map(r => r.key)), sites = [], siteOf = {};
    for (const [nid, nr] of Object.entries(withRes.nodes || {})) {
      const hit = nr.appr.filter(a => inc.has(a.key)).map(a => a.key);
      if (hit.length) { const id = nr.site || nid; sites.push(id); siteOf[id] = { node: nid, site: nr.site || null, approaches: hit }; }
    }
    // the suggested study area: the included links' extent with a 50 m buffer
    let s = Infinity, w = Infinity, n = -Infinity, e = -Infinity;
    for (const r of rows) if (r.included) { const L = sc.links[r.link]; if (!L || !sc.nodes[L.a] || !sc.nodes[L.b]) continue; for (const q of Net.geom(sc, L)) { s = Math.min(s, q[0]); n = Math.max(n, q[0]); w = Math.min(w, q[1]); e = Math.max(e, q[1]); } }
    const pad = X.buffer / 111320, padL = isFinite(s) ? X.buffer / (111320 * Math.cos((s + n) / 2 * Math.PI / 180)) : 0;
    return { period: withRes.period, rows, links: [...inc], sites, siteOf, floor, decade: decade ? 'first decade' : 'current',
      bounds: isFinite(s) ? [[s - pad, w - padL], [n + pad, e + padL]] : null, buffer: X.buffer,
      label: `AT ITA Guidelines 2025 v2 s4.2.1.1: 5% on motorways, strategic, primary and secondary arterials, RTN and FTN routes and freight levels 1A, 1B and 2; 10% on collectors; changes under ${floor} veh/h left out (Spare Construct assumption). The freight set is a Spare Construct assumption.`,
      sameGap: without.gapTol === withRes.gapTol, note: without.gapTol !== withRes.gapTol ? 'Without and with ran at different convergence targets; re-run both at the same target before relying on the extent.' : '' };
  }
  /* Difference plot data: per arc Δ veh/h, Δ V/C and Δ junction delay; bars hidden under max(20, 2 × noise95);
     labels from 10 veh/h (50 at zoom below 15); bar width clamp(k·|Δ|, 1, 14) px (k px per veh/h). */
  function difference(base, opt, o = {}) {
    if (!base || !opt) return null;
    const D = STANDARDS.diff, n95 = Math.max(+base.noise95 || 0, +opt.noise95 || 0), hide = Math.max(D.floor, D.noiseK * n95);
    const k = o.k != null ? +o.k : 0.02, zoom = o.zoom != null ? +o.zoom : 16, labelMin = zoom < D.lowZoom ? D.labelLowZoom : D.label, arcs = {};
    for (const key of new Set([...Object.keys(base.arcs || {}), ...Object.keys(opt.arcs || {})])) {
      const a = base.arcs[key] || {}, b = opt.arcs[key] || {}, d = (b.v || 0) - (a.v || 0);
      const show = Math.abs(d) >= hide;
      arcs[key] = { d: U.r1(d), dvc: U.r3((b.vc || 0) - (a.vc || 0)), dnd: U.r1((b.nd || 0) - (a.nd || 0)), show, label: show && Math.abs(d) >= labelMin,
        w: U.r1(U.clamp(k * Math.abs(d), D.wMin, D.wMax)), sign: d > 0 ? 'up' : d < 0 ? 'down' : '' };
    }
    return { arcs, hide: U.r1(hide), noise95: n95, labelMin, basis: `Bars under ${U.fmt.int(hide)} veh/h are hidden (the larger of ${D.floor} veh/h and twice the assignment noise, ${U.r1(n95)} veh/h).` };
  }
  /* 'Without vs with development' per junction (dev-tracking (d)): Max DoS, average delay, LoS and longest Q95, each
     without / with / Δ. Δ texts: '+12.2 s', 'B to D', '–' when unchanged. opts.name(id) names a junction. */
  function withWithout(without, withRes, opts = {}) {
    if (!without || !withRes) return null;
    const name = opts.name || (id => ((withRes.sites || {})[(withRes.nodes[id] || {}).site] || {}).name || id), rows = [];
    const pick = nr => nr ? { dos: nr.dos, delay: nr.avgDelay != null ? nr.avgDelay : nr.delay, los: nr.los, losText: nr.losText || nr.los, q95: Math.max(0, ...nr.appr.map(a => a.q95 || 0)) } : null;
    const sgn = (x, u, dp = 1) => (Math.abs(x) < Math.pow(10, -dp) / 2 ? '–' : `${x > 0 ? '+' : '−'}${Math.abs(x).toFixed(dp)}${u}`);
    for (const id of new Set([...Object.keys(without.nodes || {}), ...Object.keys(withRes.nodes || {})])) {
      const a = pick(without.nodes[id]), b = pick(withRes.nodes[id]); if (!a && !b) continue;
      const row = { id, site: (withRes.nodes[id] || without.nodes[id]).site || null, name: name(id), period: withRes.period, without: a, with: b };
      if (a && b) row.delta = { dos: U.r3(b.dos - a.dos), dosText: sgn(b.dos - a.dos, '', 2), delay: U.r1(b.delay - a.delay), delayText: sgn(b.delay - a.delay, ' s'),
        los: a.los === b.los ? '–' : `${a.los} to ${b.los}`, q95: b.q95 - a.q95, q95Text: sgn(b.q95 - a.q95, ' m', 0) };
      rows.push(row);
    }
    rows.sort((p, q) => ((q.delta && q.delta.delay) || 0) - ((p.delta && p.delta.delay) || 0));
    return { rows, note: without.gapTol !== withRes.gapTol ? `Without (gap target ${without.gapTol}) and with (${withRes.gapTol}) ran at different convergence targets. Re-run both at the same target.` : '', label: SELECT_LABEL };
  }
  /* Gateway distribution of development traffic (dev-tracking (b)): per gateway the % of development trips arriving
     (in) and leaving (out) through it, for each period, and the veh/h. Shares are of the trips that cross the
     boundary; internalPct = development trips that stay inside the study area. resList: [AM, PM] results. */
  function devDistribution(sc, resList) {
    const list = [].concat(resList || []).filter(r => r && r.dev), ids = new Set(), periods = {};
    for (const r of list) {
      const g = r.dev.gateways || {}, tin = Object.values(g).reduce((s, x) => s + x.in, 0), tout = Object.values(g).reduce((s, x) => s + x.out, 0);
      periods[r.period] = { in: tin, out: tout, trips: r.dev.trips, internalPct: r.dev.trips > 0 ? U.r1(100 * (1 - (tin + tout) / r.dev.trips)) : null };
      Object.keys(g).forEach(k => ids.add(k));
    }
    const rows = [...ids].map(id => {
      const row = { id, name: (sc && sc.zones[id] && sc.zones[id].name) || id, vph: {} };
      for (const r of list) {
        const x = (r.dev.gateways || {})[id] || { in: 0, out: 0 }, P = periods[r.period];
        row[r.period + 'in'] = P.in > 0 ? 100 * x.in / P.in : 0; row[r.period + 'out'] = P.out > 0 ? 100 * x.out / P.out : 0; row.vph[r.period] = U.r1(x.in + x.out);
      }
      return row;
    }).sort((a, b) => Object.values(b.vph).reduce((s, v) => s + v, 0) - Object.values(a.vph).reduce((s, v) => s + v, 0));
    return { rows, periods, label: 'Shares of the development trips that cross the study-area boundary; trips that stay inside are counted separately (internal).' };
  }

  /* ---------- sensitivity pack (docs/spec/sensitivity-pack) ----------
     Runs the with-proposal scenario under each test (a copy of the scenario and of the settings; nothing is saved to
     the project) and returns Table SENS-1. Tests: T1± trip rates ±10%, T2 PT and active-mode reductions removed,
     T3± gravity β ±25%, T4± background growth ±50% of the rate, T5 unfunded projects removed (8 runs per period);
     optional D± total demand ±10% and T6 an alternative mode share (opts.modeShare, % of vehicle trips replaced).
     opts: { periods, tests (ids), without (do-minimum scenario: adds the links over 5% / 10%), econBase (adds the 8%
     discount row from Engine.econ), modeShare, name(id), gapTol, maxIter, onProgress(k, n, label) }. */
  const SENS = [
    { id: 'T1+', test: 'T1', label: 'Trip rates +10%', input: 'trip rates × 1.10', rel: 0.10, S: S => scaleRates(S, 1.1), sc: sc => scaleTg(sc, 1.1) },
    { id: 'T1-', test: 'T1', label: 'Trip rates −10%', input: 'trip rates × 0.90', rel: -0.10, S: S => scaleRates(S, 0.9), sc: sc => scaleTg(sc, 0.9) },
    { id: 'T2', test: 'T2', label: 'PT and active-mode reductions removed', input: 'zone reductions set to 0%', rel: null, sc: sc => { for (const z of Object.values(sc.zones)) z.reduction = 0; } },
    { id: 'T3+', test: 'T3', label: 'Gravity β +25%', input: 'β × 1.25', rel: 0.25, S: S => { S.gravityBeta *= 1.25; } },
    { id: 'T3-', test: 'T3', label: 'Gravity β −25%', input: 'β × 0.75', rel: -0.25, S: S => { S.gravityBeta *= 0.75; } },
    { id: 'T4+', test: 'T4', label: 'Background growth +50%', input: 'growth rate × 1.5', rel: 0.5, S: S => { S.growth = (+S.growth || 0) * 1.5; } },
    { id: 'T4-', test: 'T4', label: 'Background growth −50%', input: 'growth rate × 0.5', rel: -0.5, S: S => { S.growth = (+S.growth || 0) * 0.5; } },
    { id: 'T5', test: 'T5', label: 'Unfunded projects removed', input: 'unfunded links and register items out', rel: null, sc: (sc, P) => dropUnfunded(sc, P) },
    { id: 'D+', test: 'D', label: 'Total demand +10%', input: 'OD matrix × 1.10', rel: 0.10, df: 1.1 },
    { id: 'D-', test: 'D', label: 'Total demand −10%', input: 'OD matrix × 0.90', rel: -0.10, df: 0.9 },
    { id: 'T6', test: 'T6', label: 'Alternative mode share', input: 'mode share', rel: null, sc: (sc, P, o) => { for (const z of Object.values(sc.zones)) if (z.kind !== 'gateway') z.reduction = U.clamp(+o.modeShare || 0, 0, 100); } },
  ];
  const SENS_DEFAULT = ['T1+', 'T1-', 'T2', 'T3+', 'T3-', 'T4+', 'T4-', 'T5'];
  function scaleRates(S, f) { for (const r of Object.values(S.rates || {})) for (const p of Object.values(r || {})) if (p && isFinite(+p.rate)) p.rate = +p.rate * f; }
  // Trip-generation tables load z.tgLoad in place of the rates, so T1 scales those loads too (on the scenario copy)
  function scaleTg(sc, f) { for (const z of Object.values(sc.zones || {})) if (z && z.tg && z.tg.on && z.tgLoad && typeof z.tgLoad === 'object') for (const k of Object.keys(z.tgLoad)) if (isFinite(+z.tgLoad[k])) z.tgLoad[k] = +z.tgLoad[k] * f; }
  function dropUnfunded(sc, P) {
    const isU = L => (typeof Scenarios !== 'undefined' ? Scenarios.unfunded(L) || Scenarios.programmed(L) : L.stage === 'unfunded');
    const regL = new Set((P && P.register || []).filter(r => r.type === 'network' && !(typeof Register !== 'undefined' && Register.inBaseline(r).included)).flatMap(r => r.links || []));
    for (const L of Object.values(sc.links)) if (isU(L) || regL.has(L.id)) delete sc.links[L.id];
  }
  const cloneSc = sc => JSON.parse(JSON.stringify({ ...sc, res: {} }));
  async function sensitivity(sc, opts = {}) {
    const P = Store.P, S0 = Store.S, t0 = performance.now(), prog = opts.onProgress || (() => {});
    const periods = opts.periods || sc.periods || ['AM', 'PM'], tests = (opts.tests || SENS_DEFAULT).map(id => SENS.find(t => t.id === id)).filter(Boolean);
    if (tests.some(t => t.id === 'T6') && !(+opts.modeShare >= 0 && opts.modeShare !== '' && opts.modeShare != null)) return { error: 'T6 needs the alternative mode share (% of vehicle trips replaced).' };
    const ao = { gapTol: opts.gapTol, maxIter: opts.maxIter }, nRuns = (tests.length + 1) * periods.length * (opts.without ? 2 : 1);
    let k = 0;
    const run = async (t, label) => {
      const S = JSON.parse(JSON.stringify(S0)), s1 = cloneSc(sc), w1 = opts.without ? cloneSc(opts.without) : null;
      if (t && t.S) t.S(S);
      if (t && t.sc) { t.sc(s1, P, opts); if (w1) t.sc(w1, P, opts); }
      const out = { with: {}, without: {} };
      for (const p of periods) {
        prog(++k, nRuns, `${label}: ${p}`);
        out.with[p] = await assign(s1, { ...ao, period: p, settings: S, demandFactor: t && t.df });
        if (w1) { prog(++k, nRuns, `${label}: ${p} (without)`); out.without[p] = await assign(w1, { ...ao, period: p, settings: S, demandFactor: t && t.df }); }
      }
      return out;
    };
    const name = opts.name || (id => id);
    const summary = (o, label) => {
      let trips = 0, vht = 0, worst = null; const ef = new Set(), state = {};
      let n5 = 0, n10 = 0;
      for (const p of periods) {
        const r = o.with[p]; trips += r.kpi.trips; vht += r.kpi.vht;
        for (const [id, nr] of Object.entries(r.nodes)) {
          const key = nr.site || id;
          if (!worst || nr.dos > worst.dos) worst = { id: key, name: name(key), dos: nr.dos, period: p };
          if (nr.los === 'E' || nr.los === 'F') ef.add(key);
          state[p + '|' + key] = { band: Math.max(0, ...nr.appr.map(a => (a.dosBand === 'over' ? 2 : a.dosBand === 'near' ? 1 : 0))), los: nr.los, dos: nr.dos, name: name(key), p };
        }
        if (o.without[p]) { const x = extentTest(sc, o.without[p], r); for (const row of x.rows) if (row.included) { if (row.cls === '5%') n5++; else n10++; } }
      }
      return { label, trips, vht, worst, losEF: ef.size, over5: opts.without ? n5 : null, over10: opts.without ? n10 : null, state, res: o };
    };
    const central = summary(await run(null, 'Central'), 'Central');
    const rows = [], sentences = [], flags = [];
    for (const t of tests) {
      const s = summary(await run(t, t.label), t.label);
      const dT = central.trips > 0 ? (s.trips - central.trips) / central.trips : 0, dV = central.vht > 0 ? (s.vht - central.vht) / central.vht : 0;
      const el = t.rel ? { trips: U.r3(dT / t.rel), vht: U.r3(dV / t.rel) } : null;
      // a conclusion changes when a junction crosses its practical DoS (or capacity) or changes LoS
      const changed = [];
      for (const [key, a] of Object.entries(central.state)) {
        const b = s.state[key]; if (!b) continue;
        if (b.band > a.band) changed.push(`${b.name} ${b.band === 2 ? 'exceeds capacity' : 'exceeds practical DoS'} (${b.dos.toFixed(2)}, ${b.p})`);
        else if (b.band < a.band) changed.push(`${b.name} drops below ${a.band === 2 ? 'capacity' : 'practical DoS'} (${b.dos.toFixed(2)}, ${b.p})`);
        else if (a.los !== b.los) changed.push(`${b.name} moves from LoS ${a.los} to ${b.los} (${b.p})`);
      }
      const noise = periods.map(p => snr(central.res.with[p], s.res.with[p])).filter(Boolean).sort((p, q) => p.R - q.R)[0] || null;
      const pc = x => `${x >= 0 ? '+' : '−'}${Math.abs(x * 100).toFixed(1)}%`;
      const sentence = `${t.label}: total vehicle trips ${pc(dT)}, VHT ${pc(dV)}${el ? ` (VHT elasticity ${el.vht.toFixed(2)})` : ''}. ` +
        (changed.length ? `Conclusion sensitive: at ${t.label.toLowerCase()} ${changed[0]}${changed.length > 1 ? `, and ${changed.length - 1} more` : ''}.` : 'No junction crosses its practical DoS or changes LoS.') +
        (noise && !noise.ok ? ` ${noise.text}` : '');
      rows.push({ id: t.id, test: t.test, label: t.label, input: t.id === 'T6' ? `zone reductions set to ${+opts.modeShare}%` : t.input, trips: U.r1(s.trips), vht: U.r1(s.vht), dTrips: U.r3(dT), dVht: U.r3(dV),
        worst: s.worst, losEF: s.losEF, over5: s.over5, over10: s.over10, elasticity: el, sensitive: changed.length > 0, changes: changed, noise, sentence });
      sentences.push(sentence);
      if (changed.length) flags.push(`Conclusion sensitive: at ${t.label.toLowerCase()} ${changed[0]}.`);
    }
    if (opts.econBase) {
      const ec = periods.map(p => econ(opts.econBase, sc, p)).find(e => e && e.method === 'mbcm');
      if (ec) rows.push({ id: 'R8', test: 'R8', label: `Discount rate ${ec.sensRate}% (MBCM sensitivity)`, input: `${ec.sensRate}% for all ${ec.years} years`, bcr: ec.bcr, bcr8: ec.bcr8, bcrText: ec.bcrText, bcr8Text: ec.bcr8Text,
        sentence: `At ${ec.sensRate}%, the BCR is ${ec.bcr8Text} (central ${ec.bcrText}).`, elasticity: null, sensitive: !!(ec.ipm || []).some((b, i) => b.band !== (ec.ipm8[i] || {}).band) });
    }
    return { label: 'Table SENS-1: sensitivity tests', periods, central: { trips: U.r1(central.trips), vht: U.r1(central.vht), worst: central.worst, losEF: central.losEF, over5: central.over5, over10: central.over10 },
      rows, sentences, flags, runs: k, ms: Math.round(performance.now() - t0), note: 'Each test changes one input; elasticity = (ΔKPI / KPI) / (Δinput / input). Each run is checked for signal to noise against the central run (TAG M3.1).' };
  }

  /* ---------- MSM consistency (reg-model-status) ----------
     msm = { src, date, links: { linkId: { growth: % a year } } } (P.data.msm). Spare Construct's implied growth per link is
     linear: ((with / base − 1) / years) × 100, from a base-year result and a future result. A link is flagged when the
     two differ by more than pct % of the MSM rate (default 20%, Spare Construct assumption). */
  function msmCheck(baseRes, futRes, msm, opts = {}) {
    const M = STANDARDS.msm, pct = opts.pct != null ? +opts.pct : M.pct;
    if (!msm || !msm.links || !Object.keys(msm.links).length) return { checked: false, text: M.notChecked, rows: [] };
    const dy = (futRes && futRes.year) - (baseRes && baseRes.year);
    if (!(dy > 0)) return { checked: false, text: 'MSM consistency needs a base-year result and a later-year result.', rows: [] };
    const rows = [];
    for (const [id, m] of Object.entries(msm.links)) {
      const v0 = ['ab', 'ba'].reduce((s, d) => s + ((baseRes.arcs[id + ':' + d] || {}).v || 0), 0), v1 = ['ab', 'ba'].reduce((s, d) => s + ((futRes.arcs[id + ':' + d] || {}).v || 0), 0);
      if (!(v0 > 0) || !isFinite(+m.growth)) continue;
      const c = (v1 / v0 - 1) / dy * 100, g = +m.growth, diff = g !== 0 ? Math.abs(c - g) / Math.abs(g) * 100 : Math.abs(c) > 0.5 ? Infinity : 0;
      rows.push({ link: id, corridor: U.r3(c), msm: g, diffPct: isFinite(diff) ? U.r1(diff) : null, flagged: diff > pct });
    }
    const n = rows.filter(r => r.flagged).length;
    return { checked: true, pct, basis: M.edition, rows, flagged: n, text: `${n} of ${rows.length} links differ from MSM growth by more than ${pct}% (Spare Construct assumption).` };
  }

  /* ---------- appraisal ---------- */
  function capital(sc) {
    const E = Store.S.econ;
    let links = 0, nodes = 0, km = 0;
    for (const L of Object.values(sc.links)) if (L.status === 'proposed' && sc.nodes[L.a] && sc.nodes[L.b]) {
      const k = Net.len(sc, L) / 1000;
      km += k; links += k * ((L.lanesAB | 0) + (L.lanesBA | 0)) * (E.laneKm[L.cls] ?? 3);
    }
    for (const n of Object.values(sc.nodes)) if (n.upgrade) nodes += E.node[n.control] ?? 0;
    return { total: links + nodes, links, nodes, km };
  }
  /* ---------- MBCM discounting (docs/spec/economics-mbcm) ----------
     Continuous-discounting factors for a benefit stream starting at time zero (1 July):
     USPWF_i(n) = (1 − (1+i)^−n) / ln(1+i)  (a constant benefit of 1 a year)
     AGPWF_i(n) = ln(1+i)^−2 − n(1+i)^−n / ln(1+i) − (1+i)^−n / ln(1+i)^2  (a benefit growing by 1 a year)
     With a schedule of rates ([{to: 30, r: 2}, {to: 60, r: 1.5}]) each segment is discounted to time zero,
     and the growing stream carries on from the value it had reached: for n > 30,
     USPWF = USPWF_2%(30) + 1.02^−30·USPWF_1.5%(n − 30) and
     AGPWF = AGPWF_2%(30) + 30·1.02^−30·USPWF_1.5%(n − 30) + 1.02^−30·AGPWF_1.5%(n − 30).
     Years past the last segment use the last rate; a segment with to: null (or Infinity) runs on to the end.
     PV = B0·(USPWF + R·AGPWF), R = arithmetic growth. */
  function uspwf1(i, n) { if (!(n > 0)) return 0; if (!(i > 0)) return n; return (1 - Math.pow(1 + i, -n)) / Math.log(1 + i); }
  function agpwf1(i, n) {
    if (!(n > 0)) return 0; if (!(i > 0)) return n * n / 2;
    const L = Math.log(1 + i), v = Math.pow(1 + i, -n);
    return 1 / (L * L) - n * v / L - v / (L * L);
  }
  function pwf(rates, n) {
    const segs = (Array.isArray(rates) ? rates : [{ to: n, r: +rates || 0 }]).filter(s => s && typeof s === 'object')
      .map(s => ({ to: s.to == null || s.to === '' ? Infinity : +s.to, r: +s.r / 100 })).filter(s => s.to > 0 && isFinite(s.r)).sort((a, b) => a.to - b.to);
    if (!segs.length) segs.push({ to: n, r: 0 });
    let t0 = 0, d = 1, U1 = 0, A1 = 0;
    for (let k = 0; k < segs.length && t0 < n; k++) {
      const end = k === segs.length - 1 ? n : Math.min(n, segs[k].to), len = end - t0, i = segs[k].r;
      if (len <= 0) continue;
      const u = uspwf1(i, len);
      U1 += d * u; A1 += d * (t0 * u + agpwf1(i, len));
      d *= Math.pow(1 + i, -len); t0 = end;
    }
    return { uspwf: U1, agpwf: A1 };
  }
  // Analysis period: 40 years by default; up to 60 with a justification note (and a 40-year comparison);
  // more than 60 only with NZTA's written approval recorded (reg-economics)
  function econPeriod(E = Store.S.econ) {
    const M = STANDARDS.mbcm, want = Math.round(+E.years || M.years);
    if (want <= M.years) return { years: Math.max(1, want), ok: true, why: '' };
    if (want <= M.longYears) return String(E.longLifeNote || '').trim()
      ? { years: want, ok: true, why: '', compare40: true }
      : { years: M.years, ok: false, why: `A ${want}-year period needs a justification note (long-life assets). ${M.years} years were used.` };
    return String(E.approval || '').trim()
      ? { years: want, ok: true, why: '', compare40: true }
      : { years: String(E.longLifeNote || '').trim() ? M.longYears : M.years, ok: false, why: `More than ${M.longYears} years needs NZTA's written approval, recorded in the economics settings. ${String(E.longLifeNote || '').trim() ? M.longYears : M.years} years were used.` };
  }
  // IPM efficiency band of a BCR in each edition (2024–27 final, 2027–30 draft); banded on the unrounded value
  function ipmBand(bcr, edition) {
    const eds = edition ? STANDARDS.ipm.filter(e => e.id === edition) : STANDARDS.ipm;
    return eds.map(e => {
      if (bcr == null || !isFinite(bcr)) return { edition: e.id, draft: e.draft, band: null, label: 'No BCR', note: e.note || '' };
      const b = e.bands.find(x => bcr < x.below);
      return { edition: e.id, draft: e.draft, band: b.band, label: b.label, note: e.note || '', source: e.name, url: e.url, verified: e.verified };
    });
  }
  // BCR for display: one decimal below 10, a whole number at 10 or above (12.4 → '12')
  const bcrText = x => x == null || !isFinite(x) ? '–' : Math.abs(x) >= 10 ? String(Math.round(x)) : x.toFixed(1);
  /* Signal to noise of a scenario difference: R = (|ΔVHT| / VHT_base) / max(gap_base, gap_option).
     Under 10 the difference may be assignment noise (TAG M3.1). */
  function snr(bRes, oRes) {
    if (!bRes || !oRes || !(bRes.kpi && oRes.kpi) || !(bRes.kpi.vht > 0)) return null;
    const g = Math.max(+bRes.gap || 0, +oRes.gap || 0), d = Math.abs(bRes.kpi.vht - oRes.kpi.vht) / bRes.kpi.vht;
    const R = g > 0 ? d / g : Infinity, min = STANDARDS.noise.min;
    return { R, ok: R >= min, min, text: R >= min ? '' : `Difference may be assignment noise (signal-to-noise ${R < 10 ? R.toFixed(1) : Math.round(R)}×; TAG M3.1 asks for ≥${min}×). Re-run both with a tighter convergence target.`,
      tighter: { gapTol: Math.min(Store.S.gapTol || 1, STANDARDS.noise.tighter.gapTol), maxIter: STANDARDS.noise.tighter.maxIter } };
  }
  // Turn-volume exports (SIDRA) need a well-converged run
  function exportReady(res) {
    const max = STANDARDS.sidraGap.max;
    if (!res) return { ok: false, why: 'Run the model first.' };
    if (!(res.gap <= max)) return { ok: false, why: `The run's gap is ${(res.gap * 100).toPrecision(2)}%; turn volumes for SIDRA need ${(max * 100).toPrecision(1)}% or better. Re-run with a tighter convergence target.` };
    return { ok: true, why: '' };
  }

  /* Indicative economics. method 'mbcm' (new projects): annual benefit = Σ_p ΔVHT_p × VoT × H_p over the
     modelled periods (H_p = equivalent hours a year, settings econ.periodHours); PV at the MBCM 2%/1.5%
     schedule and at 8% (always), with benefits growing arithmetically at R; BCR, NPV, first-year rate of
     return, IPM bands, signal to noise and warnings. method 'uniform' (older projects until they adopt the
     new defaults): the average of the modelled peaks × annualHours, flat, at the saved discount rate. */
  function econ(baseSc, optSc, period) {
    const S = Store.S, E = S.econ, M = STANDARDS.mbcm;
    const pairs = ['AM', 'PM'].map(p => [Store.res(baseSc, p), Store.res(optSc, p), p]).filter(([b, o]) => b && o);
    if (!pairs.length || !Store.res(baseSc, period) || !Store.res(optSc, period)) return null;
    // Benefits only make sense between scenarios modelled for the same year; otherwise growth reads as benefit
    const yb = r => (r.year > 0 ? r.year : null);
    const odd = pairs.find(([b, o]) => (yb(b) || yearOf(baseSc)) !== (yb(o) || yearOf(optSc)));
    const cap = capital(optSc).total - capital(baseSc).total;
    const noise = pairs.map(([b, o, p]) => ({ period: p, ...snr(b, o) })).filter(x => x.R != null).sort((a, b) => a.R - b.R)[0] || null;
    if (odd) {
      const baseYear = yb(odd[0]) || yearOf(baseSc), optYear = yb(odd[1]) || yearOf(optSc);
      return { mismatch: 'year', baseYear, optYear, dvht: null, annual: null, pv: null, cap, bcr: null, dco2: null, periods: pairs.length, label: M.label,
        note: `The base is modelled for ${baseYear} and the option for ${optYear}. Set both to the same year to work out benefits.` };
    }
    const avg = f => pairs.reduce((s, [b, o]) => s + f(b.kpi) - f(o.kpi), 0) / pairs.length;
    const dvht = avg(k => k.vht), bcrOf = pv => cap > 0.001 ? pv / cap : null;
    const GC = M.discount, commercial = E.activity === 'commercial';
    if (E.method !== 'mbcm') {
      /* Older projects keep their saved flat-benefit discounting; GC 25/01 asks every analysis for the 8% sensitivity,
         so it is reported beside it (the central figures are unchanged) */
      const annual = dvht * E.annualHours * E.vot / 1e6, flat = rr => { const r = rr / 100, n = E.years; return r > 0 ? (1 - Math.pow(1 + r, -n)) / r : n; };
      const n = E.years, pv = annual * flat(E.discount), sensRate = commercial ? M.commercial.sensRate : +E.sensRate > 0 ? +E.sensRate : M.sensRate, pv8 = annual * flat(sensRate);
      return { method: 'uniform', dvht, annual, pv, pv8, cap, bcr: bcrOf(pv), bcr8: bcrOf(pv8), bcrText: bcrText(bcrOf(pv)), bcr8Text: bcrText(bcrOf(pv8)), sensRate, dco2: avg(k => k.co2) * E.annualHours, periods: pairs.length, years: n, rate: E.discount, snr: noise,
        label: M.label, basis: `${E.discount}% a year over ${n} years, flat benefits (the project's saved settings, not MBCM v1.7.5); ${sensRate}% sensitivity (${GC.name})`,
        standard: { name: GC.name, edition: GC.edition, url: GC.url, verified: GC.verified }, warnings: noise && !noise.ok ? [noise.text] : [] };
    }
    const H = { AM: 600, PM: 600, IP: 0, ...(E.periodHours || {}) }, warnings = [];
    const vot = (+E.vot || 0) * (E.votUpdate ? STANDARDS.mbcmUpdate.travelTime : 1);
    const dvhtBy = {}, done = new Set(pairs.map(p => p[2]));
    let annual = 0, dco2 = 0;
    for (const [b, o, p] of pairs) {
      dvhtBy[p] = b.kpi.vht - o.kpi.vht;
      annual += dvhtBy[p] * vot * (+H[p] || 0) / 1e6;
      dco2 += (b.kpi.co2 - o.kpi.co2) * (+H[p] || 0);
    }
    const missing = Object.keys(H).filter(p => +H[p] > 0 && !done.has(p));
    if (missing.length) warnings.push(`${missing.join(' and ')} ${missing.length > 1 ? 'are' : 'is'} not modelled, so ${missing.map(p => `${U.fmt.int(H[p])} h a year (${p})`).join(' and ')} ${missing.length > 1 ? 'are' : 'is'} left out of the benefits. Run ${missing.length > 1 ? 'those periods' : 'that period'} or set its hours to 0.`);
    const P = econPeriod(E);
    if (!P.ok) warnings.push(P.why);
    // Benefits grow arithmetically with background traffic (or the user's own benefit growth rate)
    const R = E.benefitGrowth != null && E.benefitGrowth !== '' && isFinite(+E.benefitGrowth) ? +E.benefitGrowth / 100 : (+S.growth || 0) / 100;
    if (S.growthMode === 'compound' && (E.benefitGrowth == null || E.benefitGrowth === '')) warnings.push('Traffic growth is set to compound; MBCM benefits grow arithmetically, so its rate is used as the arithmetic rate.');
    /* NZTA GC 25/01: non-commercial activities 2% years 1–30, 1.5% years 31–100, 1% from 101 (the saved schedule) with
       8% sensitivity; commercial activities (settings econ.activity 'commercial') 8% for all years with 2% sensitivity */
    const rates = commercial ? [{ to: null, r: M.commercial.rate }] : Array.isArray(E.rates) && E.rates.length ? E.rates : M.rates;
    const sens = commercial ? M.commercial.sensRate : +E.sensRate > 0 ? +E.sensRate : M.sensRate;
    const f = pwf(rates, P.years), f8 = pwf([{ to: null, r: sens }], P.years);
    const pv = annual * (f.uspwf + R * f.agpwf), pv8 = annual * (f8.uspwf + R * f8.agpwf);
    const bcr = bcrOf(pv), bcr8 = bcrOf(pv8);
    const segText = rs => { const s = rs.filter(x => x && typeof x === 'object').map(x => ({ to: x.to == null || x.to === '' ? Infinity : +x.to, r: +x.r })).sort((a, b) => a.to - b.to); let from = 1;
      return s.map((x, k) => { const t = !isFinite(x.to) ? (k ? `${x.r}% from year ${from}` : `${x.r}% for all years`) : k ? `${x.r}% to year ${x.to}` : `${x.r}% years 1–${x.to}`; from = x.to + 1; return t; }).join(', then '); };
    const out = { method: 'mbcm', dvht, dvhtBy, annual, pv, pv8, cap, bcr, bcr8, npv: pv - cap, npv8: pv8 - cap, fyrr: cap > 0.001 ? annual / cap : null,
      dco2, periods: pairs.length, years: P.years, rates, sensRate: sens, activity: commercial ? 'commercial' : 'non-commercial', R, vot, votUpdate: !!E.votUpdate, hours: H, factors: { ...f, uspwf8: f8.uspwf, agpwf8: f8.agpwf },
      ipm: ipmBand(bcr), ipm8: ipmBand(bcr8), bcrText: bcrText(bcr), bcr8Text: bcrText(bcr8), snr: noise, incomplete: missing, lstar: !!missing.length || !(cap > 0.001),
      label: M.label, disclaimer: M.disclaimer, standard: { name: M.name, edition: M.edition, url: M.url, verified: M.verified }, discount: { name: GC.name, edition: GC.edition, url: GC.url, verified: GC.verified },
      basis: `MBCM ${M.edition}: ${commercial ? 'commercial activity, ' : ''}${segText(rates)}; ${sens}% sensitivity (${GC.name}); ${P.years} years from ${M.timeZero}; benefits growing ${(R * 100).toFixed(1)}% of year-one benefit a year (arithmetic)`,
      warnings };
    if (P.compare40) {
      const f40 = pwf(rates, M.years), pv40 = annual * (f40.uspwf + R * f40.agpwf);
      out.compare40 = { years: M.years, pv: pv40, bcr: bcrOf(pv40) };
    }
    // A band boundary between the central and 8% results makes the band uncertain
    out.ipm.forEach((b, k) => { const s = out.ipm8[k]; if (b.band && s.band && b.band !== s.band) warnings.push(`The ${b.edition}${b.draft ? ' (draft)' : ''} band changes from ${b.label} to ${s.label} at the ${sens}% sensitivity rate.`); });
    if (cap >= M.p95From && !E.p95) warnings.push(`The cost is ${U.fmt.money(cap)}, at least $${M.p95From}m: an MBCM evaluation needs a P95 cost estimate.`);
    if (out.lstar) warnings.push('L*: the inputs are incomplete, so the efficiency rating is provisional.');
    if (noise && !noise.ok) warnings.push(noise.text);
    if (!E.votUpdate) out.votNote = `Value of time $${E.vot}/veh·h as entered. Use MBCM values for the evaluation year (update factor to ${STANDARDS.mbcmUpdate.edition}: ×${STANDARDS.mbcmUpdate.travelTime}).`;
    return out;
  }

  // Cycling level of traffic stress, 1 (all ages) to 4 (confident riders only); 0 = cycling not allowed
  function lts(L, res) {
    const C = CLASSES[L.cls] || CLASSES.local, sp = +L.speed || C.speed, lanes = Math.max(L.lanesAB | 0, L.lanesBA | 0);
    if (C.rank >= 5) return 0;
    let aadt = null;
    if (res) { const a = res.arcs[L.id + ':ab'], b = res.arcs[L.id + ':ba']; aadt = ((a ? a.v : 0) + (b ? b.v : 0)) * Store.S.peakToDaily; }
    if (L.cycle === 'separated') return 1;
    if (L.cycle === 'painted') return sp <= 50 && lanes <= 1 ? 2 : sp <= 50 ? 3 : 4;
    if (sp <= 30 && lanes <= 1 && (aadt == null || aadt < 3000)) return 1;
    if (sp <= 40 && lanes <= 1 && (aadt == null || aadt < 6000)) return 2;
    if (sp <= 50 && lanes <= 1) return 3;
    return 4;
  }

  return { buildGraph, connectorMap, applySignal, arcTime, effCap, dijkstra, trace, resolveRoute, workspace, spt, pathTo,
    turnType, turnPenalty, turnBanned, movement, approachName, TURN_DEF, yearOf, tripEnds, demand, applyME, assign, travel, isochrone, selectLink, capital, econ, lts, losOf, pcuF, growth, ef, LOS_SIG, LOS_UNSIG, LOS_VC,
    STANDARDS, JUNCTION_LABEL, growthBasis, junctionSet, convSet, junctionResult, delayText, convText, updateNodeCaps,
    uspwf: uspwf1, agpwf: agpwf1, pwf, econPeriod, ipmBand, bcrText, snr, exportReady,
    mov: { cpot, twoStage, pPrime, priorityCaps, rbtCaps, laneModel, approachCap, allocLanes, q95: q95v, delay: dHCM, majorDelay }, zoneClass, stageOf, modelStatus, modelMeta, siteLegs, SELECT_LABEL,
    fcOf, extentClass, extentTest, difference, withWithout, devDistribution, sensitivity, SENS, SENS_DEFAULT, msmCheck, capMethodOf,
    busPcuOf, busFreq, tollOf, speedOf, luFactorsOf, CAP_SRC, siteFlows };
})();
