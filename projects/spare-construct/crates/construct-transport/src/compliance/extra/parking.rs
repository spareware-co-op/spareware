//! Parking, cycle parking, end-of-trip facilities and loading (compliance-extra.js section 11).
//!
//! No minimum car parking applies: only maxima, entered per row because they vary by zone. The activities are summed and then rounded
//! 0.5 up (E27.6.2(9)); cycle parking is judged on the totals only, the rows show it unrounded.

use super::ctx::{assumptions, data, esc, esc_s, nn, num, numf, stamped};
use super::packs::{n0, r1, t_table};
use crate::compliance::data::x as xdata;
use crate::compliance::html::{ReportCtx, rt};
use crate::compliance::jv::{Jx, V, csv, jobj, merge, ns};
use std::sync::atomic::{AtomicU32, Ordering};

static UID: AtomicU32 = AtomicU32::new(1);

/// `round9(x)`: round half up (`Math.floor(x + 0.5)`), the E27.6.2(9) rounding of a total.
fn round9(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// `blankPark()`: a blank parking row (the id is a counter, the prototype's is random).
pub fn blank_park() -> V {
    jobj! {
        "id" => format!("pk{:04x}", UID.fetch_add(1, Ordering::Relaxed)), "activity" => "office", "qty" => 0, "maxCars" => V::Null, "cars" => 0, "accessible" => 0, "listedZone" => false, "garage" => false,
        "noGarage" => V::Null, "cycleVisitor" => 0, "cycleSecure" => 0, "showers" => 0, "loading" => 0, "cycleVisitorReq" => V::Null, "cycleSecureReq" => V::Null,
    }
}

/// `cycleRates(act, q, row)`: the cycle parking required for a quantity of an activity (`E27.6.2.5`, residential and office only; the rate of any
/// other activity is entered): `{ visitor, secure, src }` (null where not known).
pub fn cycle_rates(act: &V, q: f64, row: &V) -> V {
    if act.eq_s("residential") {
        let secure = if nn(row.g("noGarage")).is_some() {
            row.g("noGarage").n()
        } else if row.g("garage").t() {
            0.0
        } else {
            q
        };
        return jobj! { "visitor" => if q >= 20.0 { q / 20.0 } else { 0.0 }, "secure" => secure, "src" => "E27.6.2.5 (residential)" };
    }
    if act.eq_s("office") {
        let visitor = if q <= 200.0 {
            0.0
        } else if q <= 1000.0 {
            1.0
        } else if q <= 10000.0 {
            1.0 + (q - 1000.0) / 1000.0
        } else {
            10.0 + (q - 10000.0) / 2000.0
        };
        return jobj! { "visitor" => visitor, "secure" => q / 300.0, "src" => "E27.6.2.5 (office)" };
    }
    let v = nn(row.g("cycleVisitorReq"));
    let s = nn(row.g("cycleSecureReq"));
    jobj! { "visitor" => v, "secure" => s, "src" => if v.is_some() || s.is_some() { "entered from Table E27.6.2.5" } else { "Not encoded: enter the Table E27.6.2.5 rate for this activity" } }
}

/// `showersFor(g)`: the showers required for a non-residential gross floor area.
pub fn showers_for(g: f64) -> f64 {
    if g <= 500.0 {
        0.0
    } else if g <= 2500.0 {
        1.0
    } else if g <= 7500.0 {
        2.0
    } else {
        2.0 + 2.0 * ((g - 7500.0) / 7500.0).ceil()
    }
}

/// `accessibleFor(n)`: the accessible car parks NZS 4121:2001 Table 2 asks for with n car parks (1 up to 20, 2 for 21-50, then 1 more per 50 or part).
pub fn accessible_for(n: f64) -> f64 {
    let n = if crate::jsnum::truthy_number(n) { n } else { 0.0 };
    if n <= 0.0 {
        return 0.0;
    }
    let std = xdata("STD.accessible");
    let rows = std.g("rows").items();
    for r in rows {
        if n <= r.ix(0).n() {
            return r.ix(1).n();
        }
    }
    let last = rows.last().expect("accessible rows");
    last.ix(1).n() + ((n - last.ix(0).n()) / std.g("per").n()).ceil()
}

/// `loadingFor(grp, g)`: the loading spaces required for a gross floor area of a retail-type or other non-residential group.
pub fn loading_for(grp: &str, g: f64) -> f64 {
    if grp == "retail" {
        if g <= 300.0 {
            0.0
        } else if g <= 5000.0 {
            1.0
        } else if g <= 10000.0 {
            2.0
        } else {
            2.0 + ((g - 10000.0) / 10000.0).ceil()
        }
    } else if grp == "other" {
        if g <= 5000.0 {
            0.0
        } else if g <= 20000.0 {
            1.0
        } else if g <= 90000.0 {
            2.0
        } else {
            2.0 + ((g - 90000.0) / 40000.0).ceil()
        }
    } else {
        0.0
    }
}

/// `parking(P)`: the parking compliance table P1: `{ rows, need, have, unknownCycle, nonResGfa, totalsWhy, allComply, totalsOk, label }`.
pub fn parking(p: &V) -> V {
    let a = assumptions(p);
    let trips_per_space = a.g("tripsPerSpace").n();
    let std = xdata("STD.parking");
    let acc_std = xdata("STD.accessible");
    let park_act = xdata("PARK_ACT");
    let (mut visitor, mut secure, mut visitor_p, mut secure_p, mut showers_p, mut loading, mut loading_p, mut non_res) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut unknown_cycle = false;
    let mut rows: Vec<V> = Vec::new();
    for r0 in data(p).g("parking").g("rows").items() {
        let mut r = merge(&blank_park(), r0);
        let key = r.g("activity").s();
        let act = if park_act.has(&key) && park_act.g(&key).t() { park_act.g(&key) } else { park_act.g("other") };
        let q = n0(r.g("qty"));
        let cyc = cycle_rates(r.g("activity"), q, &r);
        let cars = n0(r.g("cars"));
        let max = nn(r.g("maxCars"));
        let over = match max {
            Some(m) if cars > m => cars - m,
            _ => 0.0,
        };
        let access4 = std.g("access4").items().iter().any(|x| x.strict_eq(r.g("activity")));
        // Accessible car parks: required where parking is provided (E27.6.3.2A(1)-(3)) or for a listed activity in a listed zone with none
        // (E27.6.3.2A(4)); the indicative number is NZS 4121:2001 Table 2, via NZBC D1/AS1 (verify)
        let acc_n = if cars > 0.0 { accessible_for(cars) } else { 1.0 };
        let acc_req = if cars > 0.0 {
            jobj! { "required" => true, "n" => acc_n, "why" => format!("Car parking is provided: accessible parking is required (E27.6.3.2A(1)-(3)); about {} for {} car parks ({}: {}; verify)", ns(acc_n), ns(cars), acc_std.g("name").s(), acc_std.g("basis").s()) }
        } else if access4 && r.g("listedZone").t() {
            jobj! { "required" => true, "n" => 1, "why" => "No on-site parking, but this activity in a listed zone needs accessible parking (E27.6.3.2A(4)): at least 1" }
        } else {
            jobj! { "required" => false, "n" => 0, "why" => if access4 { "Not required unless the zone is on the E27.6.3.2A(4) list: check it" } else { "No car parking proposed" } }
        };
        let grp = act.g("grp").s();
        let load_req = if grp == "res" { 0.0 } else { loading_for(&grp, q) };
        if grp != "res" {
            non_res += q;
        }
        if !cyc.g("visitor").nullish() {
            visitor += cyc.g("visitor").n();
        } else {
            unknown_cycle = true;
        }
        if !cyc.g("secure").nullish() {
            secure += cyc.g("secure").n();
        }
        visitor_p += n0(r.g("cycleVisitor"));
        secure_p += n0(r.g("cycleSecure"));
        showers_p += n0(r.g("showers"));
        loading += load_req;
        loading_p += n0(r.g("loading"));
        // Cycle parking is judged on the totals only (E27.6.2(9): sum the activities, then round 0.5 up); rows show it unrounded
        let mut why: Vec<String> = Vec::new();
        if over != 0.0 {
            why.push(format!("{} car parks above the AUP maximum of {} (restricted discretionary)", ns(over), ns(max.unwrap_or(f64::NAN))));
        }
        let accessible = r.g("accessible").n();
        if acc_req.g("required").t() && !(accessible > 0.0) {
            why.push("No accessible car park".to_string());
        } else if acc_req.g("required").t() && accessible < acc_req.g("n").n() {
            why.push(format!("Fewer accessible car parks ({}) than the indicative {}", ns(accessible), acc_req.g("n").s()));
        }
        if n0(r.g("loading")) < load_req {
            why.push("Too few loading spaces".to_string());
        }
        let induced = if over != 0.0 {
            jobj! {
                "trips" => over * trips_per_space,
                "text" => format!("About {} induced peak commuter trips from {} extra spaces at {} trips/space ({})", numf(over * trips_per_space, 0), ns(over), ns(trips_per_space), std.g("tripsPerSpaceBasis").s()),
                "status" => "Restricted discretionary (E27.4.1); assess the E27.8.1(5) matters and E27.8.2(4)(c): effects of the extra parking on the transport network, public transport, walking and cycling, and the trips it induces",
            }
        } else {
            V::Null
        };
        let complies = why.is_empty();
        r.set("label", act.g("label").clone());
        r.set("unit", act.g("unit").clone());
        r.set("cyc", cyc);
        r.set("accReq", acc_req);
        r.set("loadReq", V::Number(load_req));
        r.set("over", V::Number(over));
        r.set("max", max.map_or(V::Null, V::Number));
        r.set("induced", induced);
        r.set("complies", V::Bool(complies));
        r.set("why", crate::compliance::jv::strs(why));
        rows.push(r);
    }
    let showers = showers_for(non_res);
    let need = jobj! { "visitor" => round9(visitor), "secure" => round9(secure), "showers" => showers, "loading" => loading };
    let (need_visitor, need_secure) = (round9(visitor), round9(secure));
    let totals_ok = visitor_p >= need_visitor && secure_p >= need_secure && showers_p >= showers && loading_p >= loading;
    let totals_why: Vec<String> = [
        if visitor_p < need_visitor { format!("Too few visitor cycle parks in total ({} of {})", ns(visitor_p), ns(need_visitor)) } else { String::new() },
        if secure_p < need_secure { format!("Too few secure cycle parks in total ({} of {})", ns(secure_p), ns(need_secure)) } else { String::new() },
        if showers_p < showers { format!("Too few showers ({} of {})", ns(showers_p), ns(showers)) } else { String::new() },
        if loading_p < loading { format!("Too few loading spaces in total ({} of {})", ns(loading_p), ns(loading)) } else { String::new() },
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect();
    let all_comply = rows.iter().all(|r| r.g("complies").t()) && totals_ok;
    jobj! {
        "rows" => crate::compliance::jv::arr(rows),
        "need" => need,
        "have" => jobj! { "visitor" => visitor_p, "secure" => secure_p, "showers" => showers_p, "loading" => loading_p },
        "unknownCycle" => unknown_cycle,
        "nonResGfa" => non_res,
        "totalsWhy" => crate::compliance::jv::strs(totals_why),
        "allComply" => all_comply,
        "totalsOk" => totals_ok,
        "label" => format!("AUP E27.6.2-E27.6.3 ({}; verify). No minimum car parking applies: the AUP sets maxima only. Cycle parking is judged on the summed totals (E27.6.2(9)).", std.g("edition").s()),
    }
}

/// `parkingCSV(P)`: Table P1 as a stamped CSV.
pub fn parking_csv(p: &V) -> String {
    let pk = parking(p);
    let s = |x: &str| V::String(x.to_string());
    let mut rows: Vec<Vec<V>> = vec![
        vec![s("Table P1 \u{b7} Parking, cycle parking, end-of-trip and loading"), pk.g("label").clone()],
        [
            "Activity", "Quantity", "Unit", "AUP maximum car parking", "Proposed car parking", "Accessible required (indicative number, verify)", "Accessible proposed", "Cycle visitor required (unrounded)",
            "Cycle visitor proposed", "Cycle secure required (unrounded)", "Cycle secure proposed", "Loading required", "Loading proposed", "Complies", "Notes",
        ]
        .iter()
        .map(|h| s(h))
        .collect(),
    ];
    for r in pk.g("rows").items() {
        let acc = r.g("accReq");
        let mut notes: Vec<String> = r.g("why").items().iter().map(|w| w.s()).collect();
        notes.push(if r.g("induced").t() { r.g("induced").g("text").s() } else { String::new() });
        let notes = notes.into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join("; ");
        let cv = r.g("cyc").g("visitor");
        let cs = r.g("cyc").g("secure");
        rows.push(vec![
            r.g("label").clone(),
            r.g("qty").clone(),
            r.g("unit").clone(),
            if r.g("max").nullish() { s("enter") } else { r.g("max").clone() },
            r.g("cars").clone(),
            if acc.g("required").t() { V::String(format!("Yes ({})", acc.g("n").s())) } else { s("No") },
            r.g("accessible").clone(),
            if !cv.nullish() { r1(cv) } else { s("enter") },
            r.g("cycleVisitor").clone(),
            if !cs.nullish() { r1(cs) } else { s("enter") },
            r.g("cycleSecure").clone(),
            r.g("loadReq").clone(),
            r.g("loading").clone(),
            s(if r.g("complies").t() { "Yes" } else { "No" }),
            V::String(notes),
        ]);
    }
    let need = pk.g("need");
    let have = pk.g("have");
    let mut tot_notes = vec![format!("Showers: {} required, {} proposed", need.g("showers").s(), have.g("showers").s())];
    tot_notes.extend(pk.g("totalsWhy").items().iter().map(|w| w.s()));
    rows.push(vec![
        s("Total (summed, then rounded 0.5 up: E27.6.2(9))"),
        s(""),
        s(""),
        s(""),
        s(""),
        s(""),
        s(""),
        need.g("visitor").clone(),
        have.g("visitor").clone(),
        need.g("secure").clone(),
        have.g("secure").clone(),
        need.g("loading").clone(),
        have.g("loading").clone(),
        s(if pk.g("allComply").t() { "Yes" } else { "No" }),
        V::String(tot_notes.join("; ")),
    ]);
    stamped(&csv(&rows), p, None)
}

/// The report sections of this part the report lane can ask for (see [`report`]).
pub const REPORT_IDS: &[&str] = &["compliance-extra-parking"];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(id: &str, p: &V, ctx: Option<&dyn ReportCtx>) -> Option<String> {
    if id != "compliance-extra-parking" {
        return None;
    }
    let pk = parking(p);
    if pk.g("rows").items().is_empty() {
        return Some(String::new());
    }
    let dash = || "\u{2013}".to_string();
    let mut rows: Vec<Vec<String>> = pk
        .g("rows")
        .items()
        .iter()
        .map(|r| {
            let acc = r.g("accReq");
            let cv = r.g("cyc").g("visitor");
            let cs = r.g("cyc").g("secure");
            vec![
                esc(r.g("label")),
                format!("{} {}", num(r.g("qty"), 0), esc(r.g("unit"))),
                num(r.g("max"), 0),
                num(r.g("cars"), 0),
                if acc.g("required").t() { format!("Yes ({})", num(acc.g("n"), 0)) } else { "No".to_string() },
                format!("{}/{}", if !cv.nullish() { num(cv, 1) } else { dash() }, num(r.g("cycleVisitor"), 0)),
                format!("{}/{}", if !cs.nullish() { num(cs, 1) } else { dash() }, num(r.g("cycleSecure"), 0)),
                format!("{}/{}", num(r.g("loadReq"), 0), num(r.g("loading"), 0)),
                if r.g("complies").t() { "Yes" } else { "No" }.to_string(),
            ]
        })
        .collect();
    let (need, have) = (pk.g("need"), pk.g("have"));
    rows.push(vec![
        "<b>Total</b>".to_string(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        format!("{}/{}", num(need.g("visitor"), 0), num(have.g("visitor"), 0)),
        format!("{}/{}", num(need.g("secure"), 0), num(have.g("secure"), 0)),
        format!("{}/{}", num(need.g("loading"), 0), num(have.g("loading"), 0)),
        if pk.g("allComply").t() { "Yes" } else { "No" }.to_string(),
    ]);
    let tbl = t_table(
        &["Activity", "Quantity", "AUP maximum", "Proposed", "Accessible required (indicative)", "Cycle visitor req/prop", "Cycle secure req/prop", "Loading req/prop", "Complies"],
        rows,
        "",
    );
    let induced: Vec<String> = pk.g("rows").items().iter().filter(|r| r.g("induced").t()).map(|r| esc(r.g("induced").g("text"))).collect();
    let note = format!(
        "{} Row cycle figures are unrounded; the totals are summed and rounded. Showers: {} required, {} proposed. {} {} Accessible numbers: {}; verify.",
        esc(pk.g("label")),
        num(need.g("showers"), 0),
        num(have.g("showers"), 0),
        esc_s(&pk.g("totalsWhy").items().iter().map(|w| w.s()).collect::<Vec<_>>().join("; ")),
        induced.join(" "),
        esc(xdata("STD.accessible.name")),
    );
    Some(rt(ctx, "Table P1 \u{b7} Parking, cycle parking, end-of-trip and loading", &tbl, &note))
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-parking"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    (id == "compliance-extra-parking").then(|| parking_csv(p))
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, p: &V) -> Option<bool> {
    (id == "compliance-extra-parking").then(|| !data(p).g("parking").g("rows").items().is_empty())
}
