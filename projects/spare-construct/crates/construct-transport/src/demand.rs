//! Travel demand from engine.js.
//!
//! Background growth, staging, trip ends per zone (land use times trip rates, trip-generation
//! tables, gateways), the manual OD matrix, gravity distribution with gateway through
//! traffic, Furness balancing and matrix estimation factors. Port of engine.js, "demand".
//!
//! Where engine.js reads the open project (`Store.P`: the per-land-use factors and the matrix
//! estimation records) the functions take it as `p`; the settings are passed beside it because
//! a sensitivity test runs on a changed copy of them.
//!
//! Matrix estimation deltas are read from a JSON object whose keys come back sorted (this
//! workspace's `serde_json` has no `preserve_order`); the prototype visits them in file order.
//! Each pair is changed on its own, so only the order of the totals in the note's sums can
//! differ, in the last bit. A manual OD matrix without its `m` rows reads as all zeros (the
//! prototype throws). A saved project the typed [`Project`] reads has fitted factors and
//! matrix rows as objects and arrays; the prototype also tolerates an array where a factor
//! object belongs (it ignores it), which the typed project cannot hold.

// The loops and comparisons mirror the JavaScript's, index for index, so that the sums come out in the same order and
// NaN takes the same branches (`!(a > b)` is not `a <= b`)
#![allow(clippy::needless_range_loop, clippy::neg_cmp_op_on_partial_ord)]
#![allow(
    clippy::many_single_char_names,
    reason = "the matrix names are engine.js's (Z, O, D, T, f), so the port reads against it"
)]

use crate::graph::{Graph, zone_class};
use crate::jsnum::{JsNum, Maybe, Num, math_max, number_to_js_string, or_default, value_to_number, value_truthy};
use crate::project::{LANDUSES, OdMatrix, OrderedMap, Project, Scenario, Settings, Zone};
use crate::standards::{growth, year_of};
use crate::util::{clamp, dist, fmt};
use serde_json::Value;
use std::collections::HashMap;

/// A zone's trips in one period (veh/h): `o` leaving it, `d` arriving at it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TripEnds {
    /// Trips leaving the zone.
    pub o: f64,
    /// Trips arriving at the zone.
    pub d: f64,
}

/// The origin-destination demand of one scenario and period (`demand`'s result).
#[derive(Clone, Debug, PartialEq)]
pub struct Demand {
    /// Number of zones (rows and columns of `t`).
    pub z: usize,
    /// Trips from each zone (veh/h), following the matrix when it was adjusted.
    pub o: Vec<f64>,
    /// Trips to each zone (veh/h), following the matrix when it was adjusted.
    pub d: Vec<f64>,
    /// The matrix row by row: `t[i * z + j]` trips from zone i to zone j (veh/h).
    pub t: Vec<f64>,
    /// Notes on how the matrix was made (lost trips, balancing, estimation).
    pub note: String,
    /// The manual OD matrix for the period was used.
    pub manual: bool,
    /// Trips from each zone before matrix estimation (None when it did not apply).
    pub o0: Option<Vec<f64>>,
    /// Trips to each zone before matrix estimation (None when it did not apply).
    pub d0: Option<Vec<f64>>,
}

// `x != null && x !== '' && isFinite(+x)`
fn pct_ok(n: &Num) -> bool {
    match n {
        Maybe::Set(JsNum::Text(s)) if s.is_empty() => false,
        Maybe::Set(_) => n.num().is_finite(),
        _ => false,
    }
}

/// `stageOf(z, sc)`: the share of the zone built: its own `buildout` percent, times the scenario's when the zone is development.
#[must_use]
pub fn stage_of(z: Option<&Zone>, sc: Option<&Scenario>) -> f64 {
    let mut k = match z {
        Some(z) if pct_ok(&z.buildout) => clamp(z.buildout.num(), 0.0, 100.0) / 100.0,
        _ => 1.0,
    };
    if let Some(sc) = sc
        && pct_ok(&sc.buildout)
        && zone_class(z) == "development"
    {
        k *= clamp(sc.buildout.num(), 0.0, 100.0) / 100.0;
    }
    k
}

/// `luFactorsOf(period)`: the project's per-land-use trip-end factors for the period (None unless it holds a set of factors).
#[must_use]
pub fn lu_factors_of<'a>(p: &'a Project, period: &str) -> Option<&'a OrderedMap<Num>> {
    p.lu_factors.get()?.get(period)?.get()
}

/// `factorOf(F, k)`: the factor for a land use (or "gateways"), 1 when it is missing, negative or not finite.
#[must_use]
pub fn factor_of(f: Option<&OrderedMap<Num>>, k: &str) -> f64 {
    let v = f.map_or(f64::NAN, |f| f.num_at(k).num());
    if v >= 0.0 && v.is_finite() { v } else { 1.0 }
}

// lfEligible(z): an existing land-use zone whose class is existing
fn lf_eligible(z: &Zone) -> bool {
    !z.kind.is("gateway") && z.existing.is_true() && zone_class(Some(z)) == "existing"
}

// (1 - clamp(+z.reduction || 0, 0, 100) / 100) * stageOf(z, sc)
fn reduction_stage(z: &Zone, sc: &Scenario) -> f64 {
    (1.0 - clamp(z.reduction.or(0.0), 0.0, 100.0) / 100.0) * stage_of(Some(z), Some(sc))
}

/// `tripEnds(z, S, period, sc)`: a zone's trips leaving and arriving in a period.
///
/// Nothing before the zone's opening year. A gateway gives its counts times growth (and the
/// fitted gateway factor unless the counts are measured). A zone with a trip-generation table
/// switched on uses its loads; otherwise land use quantities times the settings' trip rates
/// (times the fitted land-use factor for existing zones) plus the extra trips. Reduction and
/// staging apply to every zone but a gateway.
pub fn trip_ends(p: &Project, z: &Zone, s: &Settings, period: &str, sc: &Scenario) -> TripEnds {
    let (mut o, mut d) = (0.0f64, 0.0f64);
    if z.year.num() > 0.0 && year_of(Some(sc), s) < z.year.num() {
        return TripEnds { o, d };
    }
    let f = lu_factors_of(p, period);
    let pin = format!("{period}in");
    let pout = format!("{period}out");
    if z.kind.is("gateway") {
        let measured = z.gw_src.get().is_some_and(value_truthy);
        let gf = growth(s, Some(sc)) * if f.is_some() && !measured { factor_of(f, "gateways") } else { 1.0 };
        let g = z.gw.get();
        let get = |k: &str| g.map_or(f64::NAN, |g| g.num_at(k).num());
        o = or_default(get(&pin), 0.0) * gf;
        d = or_default(get(&pout), 0.0) * gf;
    } else if let Some(load) = z
        .tg_load
        .get()
        .filter(|_| z.tg.get().is_some_and(|tg| tg.get("on").is_some_and(value_truthy)))
    {
        // A switched-on trip-generation table replaces land use times rates and the extra trips; the
        // reduction and staging still apply, as they do to every land-use zone
        o = math_max(0.0, load.num_at(&pout).or(0.0));
        d = math_max(0.0, load.num_at(&pin).or(0.0));
        let k = reduction_stage(z, sc);
        o *= k;
        d *= k;
    } else {
        let lf = if f.is_some() && lf_eligible(z) { f } else { None };
        let rates = s.rates.get();
        for lu in &LANDUSES {
            let q = z.lu.get().map_or(f64::NAN, |m| m.num_at(lu.key).num());
            let q = or_default(q, 0.0);
            if q == 0.0 {
                continue;
            }
            let Some(r) = rates
                .and_then(|r| r.get(lu.key))
                .and_then(|r| r.get())
                .and_then(|r| r.get(period))
                .and_then(|r| r.get())
            else {
                continue;
            };
            let t = q / lu.per * r.rate.num() * if lf.is_some() { factor_of(lf, lu.key) } else { 1.0 };
            d += t * r.in_pct.num() / 100.0;
            o += t * (1.0 - r.in_pct.num() / 100.0);
        }
        let extra = |k: &str| or_default(z.extra_trips.get().map_or(f64::NAN, |m| m.num_at(k).num()), 0.0);
        o += extra(&pout);
        d += extra(&pin);
        let k = reduction_stage(z, sc);
        o *= k;
        d *= k;
    }
    TripEnds { o, d }
}

/// `furness(Z, O, D, f, T)`: doubly constrained balancing (60 rounds); adds the trips into `t` and returns the totals of `o` and `d`, or None when a side is empty.
pub fn furness(z: usize, o: &[f64], d: &[f64], f: &[f64], t: &mut [f64]) -> Option<(f64, f64)> {
    let (mut so, mut sd) = (0.0f64, 0.0f64);
    for i in 0..z {
        so += at(o, i);
        sd += at(d, i);
    }
    if !(so > 0.0 && sd > 0.0) {
        return None;
    }
    let dt: Vec<f64> = d.iter().map(|&v| v * so / sd).collect();
    let mut a = vec![1.0f64; z];
    let mut b = vec![1.0f64; z];
    for _ in 0..60 {
        for i in 0..z {
            let mut s = 0.0;
            for j in 0..z {
                s += at(&b, j) * at(&dt, j) * at(f, ij(z, i, j));
            }
            set_at(&mut a, i, if s > 0.0 { 1.0 / s } else { 0.0 });
        }
        for j in 0..z {
            let mut s = 0.0;
            for i in 0..z {
                s += at(&a, i) * at(o, i) * at(f, ij(z, i, j));
            }
            set_at(&mut b, j, if s > 0.0 { 1.0 / s } else { 0.0 });
        }
    }
    for i in 0..z {
        for j in 0..z {
            let k = ij(z, i, j);
            add_at(t, k, at(&a, i) * at(o, i) * at(&b, j) * at(&dt, j) * at(f, k));
        }
    }
    Some((so, sd))
}

// x[i] as the JavaScript reads a number array: past the end (which the vectors here, sized to the
// zones, never are) it gives 0, adding nothing
fn at(x: &[f64], i: usize) -> f64 {
    x.get(i).copied().unwrap_or(0.0)
}

// x[i] for the flag arrays, false past the end
fn flag(x: &[bool], i: usize) -> bool {
    x.get(i).copied().unwrap_or(false)
}

// Position of cell (i, j) in a row-major matrix of n columns. Bound: i and j are zone indexes and
// the matrix of n * n cells is in memory, so this never overflows; past usize it is a position no
// slice reaches, which reads as 0 and takes no writes.
fn ij(n: usize, i: usize, j: usize) -> usize {
    i.checked_mul(n)
        .and_then(|r| r.checked_add(j))
        .unwrap_or(usize::MAX)
}

// x[i] += v; a position past the end takes nothing
fn add_at(x: &mut [f64], i: usize, v: f64) {
    if let Some(c) = x.get_mut(i) {
        *c += v;
    }
}

// x[i] = v; a position past the end takes nothing
fn set_at(x: &mut [f64], i: usize, v: f64) {
    if let Some(c) = x.get_mut(i) {
        *c = v;
    }
}

// O and D as the sums of the matrix's rows and columns, in the JavaScript's order
fn sum_ends(zn: usize, t: &[f64], o: &mut [f64], d: &mut [f64]) {
    o.fill(0.0);
    d.fill(0.0);
    for i in 0..zn {
        for j in 0..zn {
            let x = at(t, ij(zn, i, j));
            add_at(o, i, x);
            add_at(d, j, x);
        }
    }
}

/// `applyME(G, period, T, year)`: matrix estimation of the base year, applied to the matrix `t` over the graph's `zones`; returns a note, or "" when nothing changed.
///
/// Only pairs whose ends are gateways or existing land use are touched. A fit that recorded
/// additive deltas for another year than the scenario's is carried forward as
/// `T = max(0, T + delta)`; otherwise the fitted factors multiply the pairs that carry trips.
pub fn apply_me(p: &Project, zones: &[Zone], period: &str, t: &mut [f64], year: Option<f64>) -> String {
    let zn = zones.len();
    let Some(me) = p.me.get() else {
        return String::new();
    };
    let Some(factors) = me.factors.get(period).and_then(|f| f.get()) else {
        return String::new();
    };
    if zn < 2 {
        return String::new();
    }
    let mut pos: HashMap<&str, usize> = HashMap::new();
    for (i, z) in zones.iter().enumerate() {
        if z.kind.is("gateway") || z.existing.is_true() {
            pos.insert(z.id.as_str(), i);
        }
    }
    let info = me.info.get().and_then(|m| m.get(period)).and_then(|i| i.get());
    let by = info.and_then(|i| (i.base_year.num() > 0.0).then(|| i.base_year.num()));
    let add = match (year, by, info) {
        (Some(y), Some(by), Some(info)) if y > 0.0 && y != by => match info.delta.get() {
            Some(Value::Object(m)) => Some(m),
            _ => None,
        },
        _ => None,
    };
    let (mut n, mut before, mut after) = (0u32, 0.0f64, 0.0f64);
    let pair = |key: &str| -> Option<usize> {
        let (from, to) = key.split_once('>')?;
        let i = *pos.get(from)?;
        let j = *pos.get(to)?;
        (i != j).then(|| ij(zn, i, j))
    };
    let plural = |n: u32| if n == 1 { "" } else { "s" };
    if let Some(add) = add {
        for (key, v) in add {
            let dlt = value_to_number(v);
            if !key.contains('>') || !dlt.is_finite() || v.as_str() == Some("") || v.is_null() {
                continue;
            }
            let Some(k) = pair(key) else {
                continue;
            };
            before += at(t, k);
            set_at(t, k, math_max(0.0, at(t, k) + dlt));
            after += at(t, k);
            n = n.saturating_add(1);
        }
        return if n > 0 {
            format!(
                "Matrix estimation (base year {}): ME adjustment carried forward as additive delta to {} OD pair{} ({} \u{2192} {} veh/h).",
                number_to_js_string(by.unwrap_or(f64::NAN)),
                fmt::int(f64::from(n)),
                plural(n),
                fmt::int(before),
                fmt::int(after)
            )
        } else {
            String::new()
        };
    }
    for (key, v) in factors.iter() {
        let f = v.num();
        if !key.contains('>') || !(f >= 0.0) || !f.is_finite() {
            continue;
        }
        let Some(k) = pair(key) else {
            continue;
        };
        if !(at(t, k) > 0.0) {
            continue;
        }
        before += at(t, k);
        set_at(t, k, at(t, k) * f);
        after += at(t, k);
        n = n.saturating_add(1);
    }
    if n > 0 {
        format!(
            "Matrix estimation factors applied to {} OD pair{} ({} \u{2192} {} veh/h).",
            fmt::int(f64::from(n)),
            plural(n),
            fmt::int(before),
            fmt::int(after)
        )
    } else {
        String::new()
    }
}

/// `demand(sc, G, period, S)`: the OD matrix for a scenario and period over the graph's zones.
///
/// A manual matrix for the period is used as saved (scaled by each zone's opening year, reduction and
/// staging). Otherwise zone trip ends are fixed by land use: trips leaving a zone are shared among
/// the other zones by attraction times exp(-beta x free-flow time / 60), arrivals the zones did not
/// receive come in through the gateways, and the rest of each gateway's count is through traffic
/// between gateways, favouring far-apart pairs (Furness). Matrix estimation then scales or shifts
/// the base-year pairs, and O and D follow the adjusted matrix (O0 and D0 keep the prior).
#[must_use]
pub fn demand(p: &Project, sc: &Scenario, g: &Graph, period: &str, s: &Settings) -> Demand {
    let zn = g.zones.len();
    let mut o = vec![0.0f64; zn];
    let mut d = vec![0.0f64; zn];
    let mut t = vec![0.0f64; zn.saturating_mul(zn)];
    for ((z, oi), di) in g.zones.iter().zip(&mut o).zip(&mut d) {
        let te = trip_ends(p, z, s, period, sc);
        *oi = te.o;
        *di = te.d;
    }
    let mut note = String::new();
    let mut manual = false;
    let man = sc.od_manual.get().and_then(|m| m.get(period)).and_then(|m| m.get());
    let man_zones = man.and_then(|m| m.zones.get());
    if let (Some(man), Some(man_zones)) = (man, man_zones) {
        manual = true;
        manual_matrix(sc, s, g, man, man_zones, &mut t);
        sum_ends(zn, &t, &mut o, &mut d);
        note = "Using the manual OD matrix for this period.".to_string();
    } else if zn > 1 {
        note = gravity(sc, s, g, &o, &d, &mut t);
    }
    let me = apply_me(p, &g.zones, period, &mut t, Some(year_of(Some(sc), s)));
    let (mut o0, mut d0) = (None, None);
    if !me.is_empty() {
        // O and D follow the factored matrix, so they match T and the assigned flows; O0 / D0 keep the prior
        note = if note.is_empty() { me } else { format!("{note} {me}") };
        o0 = Some(o.clone());
        d0 = Some(d.clone());
        sum_ends(zn, &t, &mut o, &mut d);
    }
    Demand {
        z: zn,
        o,
        d,
        t,
        note,
        manual,
        o0,
        d0,
    }
}

// The manual OD matrix as saved, over the graph's zones: nothing before a zone's opening year, and
// its reduction and staging applied to its row and column (land-use zones only, as in tripEnds)
fn manual_matrix(sc: &Scenario, s: &Settings, g: &Graph, man: &OdMatrix, man_zones: &[String], t: &mut [f64]) {
    let zn = g.zones.len();
    let mut pos: HashMap<&str, usize> = HashMap::new();
    for (i, id) in man_zones.iter().enumerate() {
        pos.insert(id.as_str(), i);
    }
    let yr = year_of(Some(sc), s);
    let kz: Vec<f64> = g
        .zones
        .iter()
        .map(|z| {
            if z.year.num() > yr {
                0.0
            } else if z.kind.is("gateway") {
                1.0
            } else {
                reduction_stage(z, sc)
            }
        })
        .collect();
    let rows = man.m.get();
    for (i, zi) in g.zones.iter().enumerate() {
        for (j, zj) in g.zones.iter().enumerate() {
            let a = pos.get(zi.id.as_str());
            let b = pos.get(zj.id.as_str());
            if let (Some(&a), Some(&b)) = (a, b)
                && i != j
            {
                let v = rows
                    .and_then(|r| r.get(a))
                    .and_then(|r| r.get())
                    .and_then(|r| r.get(b))
                    .map_or(f64::NAN, super::jsnum::Maybe::num);
                set_at(t, ij(zn, i, j), or_default(v, 0.0) * at(&kz, i) * at(&kz, j));
            }
        }
    }
}

// The gravity matrix with gateway through traffic, added into t; returns the notes
fn gravity(sc: &Scenario, s: &Settings, g: &Graph, o: &[f64], d: &[f64], t: &mut [f64]) -> String {
    let zn = g.zones.len();
    let mut w = g.workspace();
    let mut sk = vec![f64::INFINITY; zn.saturating_mul(zn)];
    for i in 0..zn {
        if at(o, i) <= 0.0 {
            continue;
        }
        let Some(&src) = g.zone_node.get(i) else {
            continue;
        };
        g.spt(&g.ffc, src, &mut w, -1, -1);
        for j in 0..zn {
            let reach = g.zone_node.get(j).and_then(|&n| w.nd.get(n)).copied().unwrap_or(f64::INFINITY);
            set_at(&mut sk, ij(zn, i, j), reach);
        }
    }
    // Zone trip ends are fixed by land use. Gateway counts are the totals crossing the boundary:
    // the part not used by zone trips passes through to other gateways.
    let is_g: Vec<bool> = g.zones.iter().map(|z| z.kind.is("gateway")).collect();
    let beta = s.gravity_beta.num();
    let lost = zone_trips(o, d, t, &is_g, &|k| deterrence(&sk, beta, k));
    let (s1, over, no_gateways) = through_traffic(g, year_of(Some(sc), s), o, d, t, &is_g, &sk);
    let mut notes: Vec<String> = Vec::new();
    if lost > 1.0 {
        notes.push(format!(
            "{} veh/h from zones had nowhere to go. Add gateways or destination land uses.",
            fmt::int(lost)
        ));
    }
    if !over.is_empty() {
        notes.push(format!(
            "Zone trips leaving through {} exceed the gateway count; raise the count or check the land use.",
            over.iter().take(3).copied().collect::<Vec<_>>().join(", ")
        ));
    }
    if let Some((so, sd)) = s1
        && (so - sd).abs() / math_max(so, sd) > 0.08
    {
        notes.push(format!(
            "Through traffic entering ({}) and leaving ({}) differs and was balanced.",
            fmt::int(so),
            fmt::int(sd)
        ));
    }
    if no_gateways {
        notes.push("There are no gateways, so all trips stay inside the area.".to_string());
    }
    notes.join(" ")
}

// The skim's deterrence for cell k: exp(-beta x free-flow time / 60), 0 where the zone is not reached
fn deterrence(sk: &[f64], beta: f64, k: usize) -> f64 {
    let time = sk.get(k).copied().unwrap_or(f64::INFINITY);
    if time.is_finite() { crate::jsmath::exp(-beta * time / 60.0) } else { 0.0 }
}

// Steps 1 and 2 of the gravity model, added into t; returns the trips that had nowhere to go
fn zone_trips(o: &[f64], d: &[f64], t: &mut [f64], is_g: &[bool], f: &dyn Fn(usize) -> f64) -> f64 {
    let zn = o.len();
    let mut lost = 0.0f64;
    // 1. trips leaving each zone, to zones (by attraction) and gateways (by outbound count)
    for i in 0..zn {
        if flag(is_g,i) || at(o, i) <= 0.0 {
            continue;
        }
        let mut sum = 0.0f64;
        for j in 0..zn {
            if j != i {
                sum += at(d, j) * f(ij(zn, i, j));
            }
        }
        if sum <= 0.0 {
            lost += at(o, i);
            continue;
        }
        for j in 0..zn {
            if j != i {
                let k = ij(zn, i, j);
                add_at(t, k, at(o, i) * at(d, j) * f(k) / sum);
            }
        }
    }
    // 2. trips arriving at each zone not already supplied by other zones come in through the gateways
    for j in 0..zn {
        if flag(is_g,j) || at(d, j) <= 0.0 {
            continue;
        }
        let mut got = 0.0f64;
        for i in 0..zn {
            if !flag(is_g,i) {
                got += at(t, ij(zn, i, j));
            }
        }
        let rem = at(d, j) - got;
        if rem <= 0.5 {
            continue;
        }
        let mut sum = 0.0f64;
        for i in 0..zn {
            if flag(is_g,i) {
                sum += at(o, i) * f(ij(zn, i, j));
            }
        }
        if sum <= 0.0 {
            continue;
        }
        for i in 0..zn {
            if flag(is_g,i) {
                let k = ij(zn, i, j);
                add_at(t, k, rem * at(o, i) * f(k) / sum);
            }
        }
    }
    lost
}

// Step 3 of the gravity model: the rest of each gateway count is through traffic, favouring
// gateways on the far side of the area, balanced by Furness and added into t. Returns Furness's
// totals, the gateways whose count the zone trips exceed, and whether there are no gateways.
fn through_traffic<'a>(
    g: &'a Graph,
    yr: f64,
    o: &[f64],
    d: &[f64],
    t: &mut [f64],
    is_g: &[bool],
    sk: &[f64],
) -> (Option<(f64, f64)>, Vec<&'a str>, bool) {
    let zn = g.zones.len();
    let skim = |k: usize| sk.get(k).copied().unwrap_or(f64::INFINITY);
    let gz: Vec<&Zone> = g.zones.iter().filter(|z| z.kind.is("gateway") && !(z.year.num() > yr)).collect();
    let mut diag = 1.0f64;
    for a in &gz {
        for b in &gz {
            diag = math_max(diag, dist(&a.ll(), &b.ll()));
        }
    }
    let mut or = vec![0.0f64; zn];
    let mut dr = vec![0.0f64; zn];
    let mut f1 = vec![0.0f64; zn.saturating_mul(zn)];
    let mut over: Vec<&str> = Vec::new();
    for (gi, zg) in g.zones.iter().enumerate() {
        if !flag(is_g,gi) {
            continue;
        }
        let (mut out, mut inn) = (0.0f64, 0.0f64);
        for j in 0..zn {
            if !flag(is_g,j) {
                out += at(t, ij(zn, gi, j));
                inn += at(t, ij(zn, j, gi));
            }
        }
        set_at(&mut or, gi, math_max(0.0, at(o, gi) - out));
        set_at(&mut dr, gi, math_max(0.0, at(d, gi) - inn));
        if inn > at(d, gi) * 1.05 + 20.0 {
            over.push(zg.name.as_str().unwrap_or(""));
        }
        for (j, zj) in g.zones.iter().enumerate() {
            let k = ij(zn, gi, j);
            if !flag(is_g,j) || j == gi || !skim(k).is_finite() {
                continue;
            }
            let dd = dist(&zg.ll(), &zj.ll()) / diag;
            set_at(&mut f1, k, if dd < 0.3 { 0.0 } else { dd * dd });
        }
    }
    (furness(zn, &or, &dr, &f1, t), over, gz.is_empty())
}
