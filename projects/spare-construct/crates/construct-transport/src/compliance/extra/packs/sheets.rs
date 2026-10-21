//! The state highway, MBCM, RLTP and MSM packs and the hand-over manifest (compliance-extra.js section 19, items 5 to 8).

use super::{distinct, or_blank, pack_csv, r1, r1f, r3, section, sv};
use crate::compliance::data::{m as model, x as xdata};
use crate::compliance::engine;
use crate::compliance::extra::anop::anop;
use crate::compliance::extra::ctx::{disclaimer, is_sh, link_name, lu_of, node_name, pair, periods_of, pct, res_of, stage_of};
use crate::compliance::extra::policy::affected_links;
use crate::compliance::extra::shared::econ_of;
use crate::compliance::jv::{Jx, V, arr, jobj};
use crate::compliance::{net, record};
use crate::jsnum::{math_round, number_to_js_string};

/// `x.toExponential(2)`: "1.23e-4" (the exponent is always signed).
fn to_exponential2(x: f64) -> String {
    if !x.is_finite() {
        return number_to_js_string(x);
    }
    let t = format!("{:.2e}", x);
    let (m, e) = t.split_once('e').expect("exponent");
    let e: i32 = e.parse().expect("exponent digits");
    format!("{}e{}{}", m, if e < 0 { "-" } else { "+" }, e.abs())
}

/// `shPack(P)`: the NZTA state highway impact pack (CSV): volumes with and without the proposal, the state highway junctions and the
/// fields of the access application.
pub fn sh_pack(p: &V) -> String {
    let pr = pair(p);
    let mut rows: Vec<Vec<V>> = Vec::new();
    for per in periods_of(pr.opt).into_iter().filter(|x| res_of(pr.base, x).is_some()) {
        let rb = res_of(pr.base, per).expect("base results");
        let ro = res_of(pr.opt, per).expect("option results");
        for l in pr.opt.g("links").vals() {
            if !is_sh(p, l) {
                continue;
            }
            for d in ["ab", "ba"] {
                let k = format!("{}:{}", l.g("id").s(), d);
                let (a, b) = (rb.g("arcs").g(&k), ro.g("arcs").g(&k));
                if !a.t() && !b.t() {
                    continue;
                }
                let v0 = if a.t() { a.g("v").n() } else { 0.0 };
                let v1 = if b.t() { b.g("v").n() } else { 0.0 };
                rows.push(vec![
                    V::String(link_name(l)),
                    sv(if d == "ab" { "A\u{2192}B" } else { "B\u{2192}A" }),
                    sv(per),
                    r1f(v0),
                    r1f(v1),
                    r1f(v1 - v0),
                    r1(&pct(v1, v0).map_or(V::Null, V::Number)),
                    if b.t() { r3(b.g("vc")) } else { sv("") },
                    if b.t() { r1(b.g("spd")) } else { sv("") },
                ]);
            }
        }
    }
    let mut nodes: Vec<Vec<V>> = Vec::new();
    for per in periods_of(pr.opt) {
        let ro = res_of(pr.opt, per).expect("option results");
        for (id, nr) in ro.g("nodes").entries() {
            if net::road_links(pr.opt, id).iter().any(|l| is_sh(p, l)) {
                nodes.push(vec![V::String(node_name(pr.opt, id)), sv(per), r3(nr.g("dos")), if nr.g("losText").t() { nr.g("losText").clone() } else { nr.g("los").clone() }, r1(nr.g("delay"))]);
            }
        }
    }
    let fields: Vec<Vec<V>> = [
        ["Applicant and site address", ""],
        ["Proposed access location and form", "Compliance tab \u{203a} Access"],
        ["Traffic generated (peak hour)", "Development class flows (results)"],
        ["Effects on the state highway", "Tables above"],
        ["Crash history", "CAS (Traffic data tab)"],
    ]
    .iter()
    .map(|r| vec![sv(r[0]), sv(r[1])])
    .collect();
    pack_csv(
        p,
        "NZTA state highway impact pack",
        "an NZTA land-use or access application",
        &[
            section(
                "State highway volumes, with and without the proposal",
                None,
                &["State highway section", "Direction", "Peak", "Without (veh/h)", "With (veh/h)", "Change (veh/h)", "Change (%)", "v/c with", "Speed with (km/h)"],
                rows,
                Some("No state highway in the model, or no results for both scenarios."),
            ),
            section("State highway intersections", Some(xdata("STD.labels.junction").s()), &["Intersection", "Peak", "DoS", "LoS", "Delay (s)"], nodes, None),
            section("Application fields to complete", None, &["Field", "From Spare Construct"], fields, None),
        ],
    )
}

/// `mbcmPack(P)`: the MBCM transport modelling checks worksheet, the SP workbook inputs and the IER tool fields (CSV).
pub fn mbcm_pack(p: &V) -> String {
    let pr = pair(p);
    let mut checks: Vec<Vec<V>> = Vec::new();
    let mut sp: Vec<Vec<V>> = Vec::new();
    for s in distinct(&[Some(pr.base), Some(pr.opt)]) {
        for per in periods_of(s) {
            let r = res_of(s, per).expect("results");
            let cal = engine::calibrate_report(p, s, per);
            let name = s.g("name").clone();
            let row = |check: &str, value: V, crit: V, status: V| vec![name.clone(), sv(per), sv(check), value, crit, status];
            checks.push(row(
                "Convergence (relative gap)",
                if !r.g("gap").nullish() { V::String(to_exponential2(r.g("gap").n())) } else { sv("") },
                sv("\u{2264} 1e-4 (TAG)"),
                sv(if r.g("converged").strict_eq(&V::Bool(false)) { "Not converged" } else { "Converged" }),
            ));
            checks.push(row(
                "Iterations",
                r.g("it").clone(),
                V::String(format!("\u{2264} {}", if r.g("maxIter").t() { r.g("maxIter").s() } else { String::new() })),
                sv(if r.g("hitMax").t() { "Hit the limit" } else { "OK" }),
            ));
            checks.push(row("Noise (95th percentile, veh/h)", r1(r.g("noise95")), sv("Benefits must exceed noise"), sv("")));
            checks.push(row(
                "GEH < 5 share",
                if cal.t() && !cal.g("geh5").nullish() { V::String(format!("{}%", number_to_js_string(math_round(cal.g("geh5").n() * 100.0)))) } else { sv("n/a") },
                sv("85% (TMDG)"),
                if cal.t() { cal.g("status").clone() } else { sv("not calibrated") },
            ));
            checks.push(row("Model status", engine::model_status(p, per).g("label").clone(), sv("Validated for MBCM full procedures"), sv("")));
            let kpi = r.g("kpi");
            sp.push(vec![name.clone(), sv(per), V::Number(engine::year_of(p, s)), r1(kpi.g("vht")), r1(kpi.g("vkt")), r3(kpi.g("co2")), r1(kpi.g("trips"))]);
        }
    }
    let e = econ_of(p, pr.base, pr.opt);
    let ier: Vec<Vec<V>> = if e.t() && !e.g("mismatch").t() {
        let ipm: Vec<String> = e
            .g("ipm")
            .items()
            .iter()
            .map(|b| format!("{}: {}{}", b.g("edition").s(), b.g("band").s(), if b.g("draft").t() { " (draft)" } else { "" }))
            .collect();
        vec![
            vec![sv("Benefits PV (NZ$m)"), r3(e.g("pv"))],
            vec![sv("Costs PV (NZ$m)"), r3(e.g("cap"))],
            vec![sv("BCR (indicative)"), if e.g("bcrText").t() { e.g("bcrText").clone() } else { r1(e.g("bcr")) }],
            vec![sv("BCR at 8%"), or_blank(e.g("bcr8Text"))],
            vec![sv("IPM efficiency band"), V::String(ipm.join("; "))],
            vec![sv("Method"), or_blank(e.g("label"))],
            vec![sv("Disclaimer"), V::String(disclaimer("D4", p, None))],
        ]
    } else {
        vec![vec![sv("Economics"), if e.t() && e.g("mismatch").t() { e.g("note").clone() } else { sv("Run the base and the option for the same year") }]]
    };
    pack_csv(
        p,
        "MBCM transport modelling checks, SP workbook and IER fields",
        "the NZTA IER tool / MBCM workbook",
        &[
            section("Transport modelling checks worksheet", None, &["Scenario", "Period", "Check", "Value", "Criterion", "Status"], checks, Some("Run the model.")),
            section("SP workbook inputs (per modelled peak hour)", None, &["Scenario", "Period", "Year", "VHT (veh\u{b7}h)", "VKT (veh-km)", "CO2e (t)", "Trips"], sp, None),
            section("IER tool fields", None, &["Field", "Value"], ier, None),
        ],
    )
}

/// `rltpSheet(P)`: the RLTP activity sheet (CSV, LTMA s16(3)(e)).
pub fn rltp_sheet(p: &V) -> String {
    let st = record::state(p);
    let x = crate::compliance::extra::ctx::data(p).g("rltp").clone();
    let region = if st.g("ctx").g("region").t() { st.g("ctx").g("region").s() } else { "auckland".to_string() };
    let auck = region != "other";
    let rows: Vec<Vec<V>> = vec![
        vec![sv("Activity"), or_blank(p.g("name"))],
        vec![
            sv("Submitting organisation"),
            sv(if auck { "Auckland Council (road controlling authority, LGACA s47)" } else { "The approved organisation (road controlling authority)" }),
        ],
        vec![sv("RLTP prepared by"), sv(if auck { "Auckland Regional Transport Committee (ARTC)" } else { "The regional transport committee" })],
        vec![sv("Objective"), or_blank(x.g("objective"))],
        vec![sv("Measures"), or_blank(x.g("measures"))],
        vec![sv("Cost per year (NZ$)"), or_blank(x.g("costPerYear"))],
        vec![sv("Duration"), or_blank(x.g("duration"))],
        vec![sv("Funding sources"), or_blank(x.g("sources"))],
        vec![sv("Statutory basis"), sv("Land Transport Management Act 2003 s16(3)(e)")],
    ];
    pack_csv(p, "RLTP activity sheet", "the regional land transport plan", &[section("Activity (LTMA s16(3)(e))", None, &["Field", "Value"], rows, None)])
}

/// `msmPack(P)`: the MSM / Auckland Forecasting Centre request pack (CSV): zone land use, scenario definitions and the links of interest.
pub fn msm_pack(p: &V) -> String {
    let pr = pair(p);
    let sc = pr.opt;
    let landuses = model("LANDUSES");
    let keys = landuses.keys();
    let aff = affected_links(p);
    let mut head: Vec<String> = ["Zone", "Latitude", "Longitude", "Class", "Opening year", "Build-out (%)"].iter().map(|x| x.to_string()).collect();
    for k in &keys {
        head.push(format!("{} ({})", landuses.g(k).g("label").s(), landuses.g(k).g("unit").s()));
    }
    let zones: Vec<Vec<V>> = sc
        .g("zones")
        .vals()
        .into_iter()
        .filter(|z| !z.g("kind").eq_s("gateway"))
        .map(|z| {
            let mut row = vec![
                if z.g("name").t() { z.g("name").clone() } else { z.g("id").clone() },
                z.g("lat").clone(),
                z.g("lng").clone(),
                sv(net::zone_class(z)),
                or_blank(z.g("year")),
                V::Number(math_round(stage_of(z, sc) * 100.0)),
            ];
            for k in &keys {
                let q = lu_of(z, k);
                row.push(if crate::jsnum::truthy_number(q) { V::Number(q) } else { sv("") });
            }
            row
        })
        .collect();
    let scenarios: Vec<Vec<V>> = p
        .g("scenarios")
        .items()
        .iter()
        .map(|s| {
            let d = engine::scenario_describe(p, s);
            vec![s.g("name").clone(), d.g("text").clone()]
        })
        .collect();
    let interest: Vec<Vec<V>> = aff
        .ids
        .iter()
        .filter(|id| sc.g("links").g(&id.s()).t())
        .map(|id| {
            let l = sc.g("links").g(&id.s());
            let (a, b) = (sc.g("nodes").g(&l.g("a").s()), sc.g("nodes").g(&l.g("b").s()));
            vec![
                V::String(link_name(l)),
                l.g("cls").clone(),
                V::String(format!("{}, {}", a.g("lat").s(), a.g("lng").s())),
                V::String(format!("{}, {}", b.g("lat").s(), b.g("lng").s())),
            ]
        })
        .collect();
    let head_refs: Vec<&str> = head.iter().map(|h| h.as_str()).collect();
    pack_csv(
        p,
        "MSM / Auckland Forecasting Centre request pack",
        "the Auckland Forecasting Centre (MSM)",
        &[
            section("Zone land use", None, &head_refs, zones, None),
            section("Scenario definition", None, &["Scenario", "Definition"], scenarios, None),
            section("Links of interest", Some(aff.basis.clone()), &["Road", "Class", "From (lat, lng)", "To (lat, lng)"], interest, None),
        ],
    )
}

/// `handoverManifest(P)`: the hand-over packs and whether each is ready: `[{ id, pack, tool, exportId, ready, why? }]`.
pub fn handover_manifest(p: &V) -> V {
    let pr = pair(p);
    let sidra = {
        let sc = pr.opt;
        let r: Vec<&V> = [res_of(sc, "AM"), res_of(sc, "PM")].into_iter().flatten().collect();
        if !r.is_empty() { engine::sidra_export(p, sc, &r) } else { jobj! { "ok" => false, "why" => "Run the model" } }
    };
    let ready_res = !periods_of(pr.opt).is_empty();
    let sidra_ok = sidra.t() && sidra.g("ok").t();
    arr([
        jobj! {
            "id" => "sidra", "pack" => "SIDRA volumes", "tool" => "SIDRA", "exportId" => "junctions-sidra-volumes", "ready" => sidra_ok,
            "why" => if sidra.t() && !sidra.g("ok").t() { sidra.g("why").clone() } else { sv("") },
        },
        jobj! { "id" => "tsdg", "pack" => "ATOC TSDG s2.5 signal pack", "tool" => "SIDRA (signal design)", "exportId" => "compliance-extra-tsdg", "ready" => ready_res },
        jobj! { "id" => "anop", "pack" => "ANOP tool inputs (anop.csv and speeds)", "tool" => "AT ANOP tool v10.2", "exportId" => "compliance-extra-anop", "ready" => !anop(p).g("rows").items().is_empty() },
        jobj! { "id" => "ssa", "pack" => "Safe System Audit briefing pack", "tool" => "Safe System Audit", "exportId" => "compliance-extra-ssa", "ready" => ready_res },
        jobj! {
            "id" => "sh", "pack" => "NZTA state highway impact pack", "tool" => "NZTA access application", "exportId" => "compliance-extra-sh",
            "ready" => pr.opt.g("links").vals().into_iter().any(|l| is_sh(p, l)),
        },
        jobj! { "id" => "mbcm", "pack" => "MBCM checks, SP workbook and IER fields", "tool" => "NZTA IER tool / MBCM workbook", "exportId" => "compliance-extra-mbcm", "ready" => ready_res },
        jobj! { "id" => "rltp", "pack" => "RLTP activity sheet", "tool" => "RLTP", "exportId" => "compliance-extra-rltp", "ready" => crate::compliance::extra::ctx::data(p).g("rltp").g("objective").t() },
        jobj! { "id" => "msm", "pack" => "MSM / AFC request pack", "tool" => "MSM (Auckland Forecasting Centre)", "exportId" => "compliance-extra-msm", "ready" => true },
    ])
}
