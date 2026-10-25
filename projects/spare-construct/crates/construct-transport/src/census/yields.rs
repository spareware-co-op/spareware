//! Zoned yield: the land use a zoned area can hold at full build-out (landuse-plus.js "zoned yield").

use super::consts::{GFA_USES, NO_YIELD, NO_YIELD_GROUPS, aup_name, std as std_, yield_row};
use crate::edit::IdSource;
use crate::jsnum::{Maybe, Num, json_number, math_round, number_to_js_string, value_to_js_string, value_to_number, value_truthy};
use crate::project::{OrderedMap, Project, Zone, land_use};
use crate::util::fmt;
use serde_json::{Map, Value, json};

/// The yield of a zone code: the default row with the user's changes from `P.landuse.yields` (the answer of `yieldDef`).
#[derive(Clone, Debug, PartialEq)]
pub struct YieldDef {
    /// Dwellings per developable hectare.
    pub dph: Option<f64>,
    /// "residential", "apartment" or "mixed".
    pub homes: Option<String>,
    /// GFA divided by developable land.
    pub far: Option<f64>,
    /// GFA shares by land use.
    pub mix: Option<Vec<(String, f64)>>,
    /// Quantity per developable hectare by land use.
    pub per: Option<Vec<(String, f64)>>,
    /// A structure-plan assumption.
    pub assumed: bool,
    /// Note on the yield.
    pub note: Option<String>,
    /// Developable share of the land (%).
    pub net_pct: f64,
    /// The user changed the yield.
    pub user: bool,
    /// The GFA ratio needs the user to choose what the floor area is used for.
    pub needs_use: bool,
}

fn key_of(code: f64) -> String {
    number_to_js_string(code)
}

fn landuse_of(p: &Project) -> Option<&Value> {
    p.extra.get("landuse")
}

/// `yieldDef(code, P)`: the default yield of an AUP zone code with the project's own entries over it.
pub fn yield_def(code: f64, p: Option<&Project>) -> YieldDef {
    let base = yield_row(code);
    let over = p
        .and_then(landuse_of)
        .and_then(|l| l.get("yields"))
        .filter(|y| value_truthy(y))
        .and_then(|y| y.get(key_of(code)))
        .filter(|o| value_truthy(o))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let num = |k: &str| -> Option<f64> {
        let v = over.get(k)?;
        if v.as_str() == Some("") || v.is_null() {
            return None;
        }
        let x = value_to_number(v);
        x.is_finite().then_some(x)
    };
    let mut d = YieldDef {
        dph: base.and_then(|b| b.dph),
        homes: base.and_then(|b| b.homes).map(str::to_string),
        far: base.and_then(|b| b.far),
        mix: base.and_then(|b| b.mix).map(|m| m.iter().map(|(k, v)| (k.to_string(), *v)).collect()),
        per: base.and_then(|b| b.per).map(|m| m.iter().map(|(k, v)| (k.to_string(), *v)).collect()),
        assumed: base.is_some_and(|b| b.assumed),
        note: base.and_then(|b| b.note).map(str::to_string),
        net_pct: std_::YIELDS_NET_PCT,
        user: false,
        needs_use: false,
    };
    if let Some(x) = num("dph") {
        d.dph = Some(x);
    }
    if let Some(x) = num("far") {
        d.far = Some(x);
    }
    d.net_pct = num("netPct").unwrap_or(std_::YIELDS_NET_PCT);
    let use_ = over.get("use").and_then(Value::as_str).filter(|u| GFA_USES.contains(u));
    d.user = ["dph", "far", "netPct"].iter().any(|k| num(k).is_some()) || use_.is_some();
    if d.far.is_some_and(|f| f > 0.0) && !base.is_some_and(|b| b.mix.is_some()) {
        match use_ {
            Some(u) => d.mix = Some(vec![(u.to_string(), 1.0)]),
            None => d.needs_use = true,
        }
    }
    d
}

/// The land use a zoned area can hold (`yieldOf`'s answer).
#[derive(Clone, Debug, PartialEq)]
pub struct Yield {
    /// Zone code.
    pub code: f64,
    /// Zone name.
    pub name: String,
    /// Area (ha).
    pub ha: f64,
    /// Developable share (%).
    pub net_pct: f64,
    /// Dwellings per hectare, if any.
    pub dph: Option<f64>,
    /// GFA ratio, if any.
    pub far: Option<f64>,
    /// Land use quantities by key, in the order they were worked out.
    pub lu: Vec<(String, f64)>,
    /// Spare Construct has a default yield for the zone.
    pub known: bool,
    /// The user changed the yield.
    pub user: bool,
    /// A structure-plan assumption, not zoned capacity.
    pub assumed: bool,
    /// The GFA ratio needs a use chosen.
    pub needs_use: bool,
    /// What the yield is.
    pub label: String,
    /// Notes.
    pub note: String,
    /// How the yield was worked out.
    pub basis: String,
}

impl Yield {
    /// The answer as JSON.
    pub fn to_value(&self) -> Value {
        let n = |x: Option<f64>| x.map_or(Value::Null, json_number);
        json!({
            "code": json_number(self.code), "name": self.name, "ha": json_number(self.ha), "netPct": json_number(self.net_pct), "dph": n(self.dph), "far": n(self.far),
            "lu": lu_value(&self.lu), "known": self.known, "user": self.user, "assumed": self.assumed, "needsUse": self.needs_use, "label": self.label, "note": self.note, "basis": self.basis,
        })
    }
}

fn lu_value(lu: &[(String, f64)]) -> Value {
    Value::Object(lu.iter().map(|(k, v)| (k.clone(), json_number(*v))).collect::<Map<_, _>>())
}

fn set(lu: &mut Vec<(String, f64)>, k: &str, v: f64) {
    match lu.iter_mut().find(|(x, _)| x == k) {
        Some(e) => e.1 = v,
        None => lu.push((k.to_string(), v)),
    }
}

/// `yieldOf(code, ha, P, name)`: land use from a zoned area: dwellings = ha \u{d7} developable % \u{d7} dph; GFA = ha \u{d7} 10,000 m\u{b2} \u{d7} developable % \u{d7} GFA ratio, split by mix.
pub fn yield_of(code: f64, ha: f64, p: Option<&Project>, name: Option<&str>) -> Yield {
    let d = yield_def(code, p);
    let hav = crate::jsnum::or_default(ha, 0.0);
    let net = hav * d.net_pct / 100.0;
    let mut lu: Vec<(String, f64)> = Vec::new();
    if d.dph.is_some_and(|x| x > 0.0) {
        let n = math_round(net * d.dph.expect("dph"));
        if d.homes.as_deref() == Some("mixed") {
            let r = math_round(n / 2.0);
            set(&mut lu, "residential", r);
            set(&mut lu, "apartment", n - r);
        } else {
            set(&mut lu, d.homes.as_deref().unwrap_or("residential"), n);
        }
    }
    if d.far.is_some_and(|x| x > 0.0) {
        for (k, s) in d.mix.iter().flatten() {
            set(&mut lu, k, math_round(net * 1e4 * d.far.expect("far") * s / 10.0) * 10.0);
        }
    }
    for (k, per_ha) in d.per.iter().flatten() {
        set(&mut lu, k, math_round(net * per_ha));
    }
    lu.retain(|(_, v)| *v > 0.0);
    let known = yield_row(code).is_some();
    let mut note_parts: Vec<String> = Vec::new();
    if let Some(n) = d.note.as_ref().filter(|n| !n.is_empty()) {
        note_parts.push(n.clone());
    }
    if d.needs_use {
        note_parts.push("Choose what the floor area is used for, so the GFA ratio gives a land use".to_string());
    }
    let mut basis = format!("{} ha \u{d7} {}% developable", fmt::num(hav, 2), number_to_js_string(d.net_pct));
    if d.dph.is_some_and(crate::jsnum::truthy_number) {
        basis.push_str(&format!(" \u{d7} {} dwellings/ha", number_to_js_string(d.dph.expect("dph"))));
    }
    if d.far.is_some_and(crate::jsnum::truthy_number) {
        basis.push_str(&format!(" \u{d7} GFA ratio {}", number_to_js_string(d.far.expect("far"))));
    }
    if let Some(per) = &d.per {
        basis.push_str(" \u{d7} ");
        basis.push_str(&per.iter().map(|(k, v)| format!("{} {}/ha", number_to_js_string(*v), land_use(k).map_or("", |l| l.unit))).collect::<Vec<_>>().join(", "));
    }
    Yield {
        code,
        name: name.filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| aup_name(code).map_or_else(|| format!("AUP zone {}", number_to_js_string(code)), str::to_string)),
        ha: hav,
        net_pct: d.net_pct,
        dph: d.dph.filter(|x| crate::jsnum::truthy_number(*x)),
        far: d.far.filter(|x| crate::jsnum::truthy_number(*x)),
        lu,
        known,
        user: d.user,
        assumed: d.assumed,
        needs_use: d.needs_use,
        label: if d.user { std_::YIELDS_USER.to_string() } else if known { std_::YIELDS_LABEL.to_string() } else { std_::YIELDS_NONE.to_string() },
        note: note_parts.join(". "),
        basis,
    }
}

/// One zoned area of the capacity table.
#[derive(Clone, Debug, PartialEq)]
pub struct CapRow {
    /// The area's yield.
    pub y: Yield,
    /// Zone group ("Residential").
    pub group: String,
    /// "zoned", "missing", "assumed" or "none".
    pub kind: &'static str,
    /// What the row's yield is.
    pub label: String,
    /// The yield with the user's proposed zone in place of the operative one.
    pub proposed: Option<Yield>,
}

fn add_lu(tot: &mut Vec<(String, f64)>, lu: &[(String, f64)]) {
    for (k, v) in lu {
        let cur = tot.iter().find(|(x, _)| x == k).map_or(0.0, |e| e.1);
        set(tot, k, cur + v);
    }
}

/// Every zoned area in the study area with a yield, and the totals (`zonedCapacity`'s answer).
#[derive(Clone, Debug, PartialEq)]
pub struct ZonedCapacity {
    /// One row per zoned area.
    pub rows: Vec<CapRow>,
    /// Full build-out of the zoned capacity.
    pub total: Vec<(String, f64)>,
    /// The structure-plan assumptions, totalled separately.
    pub assumed_total: Vec<(String, f64)>,
    /// Full build-out with the proposed zones in place (None when none was proposed).
    pub proposed_total: Option<Vec<(String, f64)>>,
}

impl ZonedCapacity {
    /// The rows of a kind.
    pub fn of_kind(&self, kind: &str) -> Vec<&CapRow> {
        self.rows.iter().filter(|r| r.kind == kind).collect()
    }
    /// The answer as JSON, in the prototype's shape.
    pub fn to_value(&self) -> Value {
        let row = |r: &CapRow| -> Value {
            let mut v = r.y.to_value();
            v["group"] = json!(r.group);
            v["kind"] = json!(r.kind);
            v["label"] = json!(r.label);
            v["proposed"] = r.proposed.as_ref().map_or(Value::Null, |p| {
                let mut pv = p.to_value();
                pv["label"] = json!(std_::YIELDS_PROPOSED);
                pv
            });
            v
        };
        let list = |k: &str| Value::Array(self.rows.iter().filter(|r| r.kind == k).map(row).collect());
        json!({
            "rows": self.rows.iter().map(row).collect::<Vec<_>>(), "zoned": list("zoned"), "assumed": list("assumed"), "missing": list("missing"),
            "total": lu_value(&self.total), "assumedTotal": lu_value(&self.assumed_total), "proposedTotal": self.proposed_total.as_ref().map_or(Value::Null, |t| lu_value(t)),
            "proposedRows": Value::Array(self.rows.iter().filter(|r| r.proposed.is_some()).map(row).collect()),
            "label": std_::YIELDS_LABEL, "netLabel": std_::YIELDS_NET_LABEL,
        })
    }
}

fn no_yield(code: f64, group: &str) -> bool {
    NO_YIELD.iter().any(|c| f64::from(*c) == code) || NO_YIELD_GROUPS.iter().any(|g| g.eq_ignore_ascii_case(group))
}

/// `proposedOf(code, P)`: the zone code the user proposes in place of an operative one, if it has a yield.
pub fn proposed_of(code: f64, p: &Project) -> Option<f64> {
    let l = landuse_of(p)?;
    let t = l.get("proposed").filter(|x| value_truthy(x)).and_then(|x| x.get(key_of(code)))?;
    if !value_truthy(t) {
        return None;
    }
    let tn = value_to_number(t);
    if tn == code {
        return None;
    }
    let has_default = yield_row(tn).is_some();
    let has_user = l.get("yields").is_some_and(|y| y.get(key_of(tn)).is_some_and(value_truthy));
    (has_default || has_user).then_some(tn)
}

/// The zoned areas of the last fill (`P.landuse.aupSummary`).
pub fn aup_summary(p: &Project) -> Vec<Value> {
    landuse_of(p).and_then(|l| l.get("aupSummary")).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// `zonedCapacity(summary, opts)`: every zoned area in the study area with a kind: 'zoned' (a yield counted in full build-out), 'missing' (no
/// default and none entered), 'assumed' (Future Urban: a structure-plan assumption, totalled separately) or 'none' (roads, water, open space).
/// `codes` limits the zones.
pub fn zoned_capacity(p: &Project, summary: &[Value], codes: Option<&[f64]>) -> ZonedCapacity {
    let rows: Vec<CapRow> = summary
        .iter()
        .filter(|r| codes.is_none_or(|c| c.contains(&r.get("code").map_or(f64::NAN, value_to_number))))
        .map(|r| {
            let code = r.get("code").map_or(f64::NAN, value_to_number);
            let ha = r.get("ha").map_or(f64::NAN, value_to_number);
            let name = r.get("name").filter(|x| value_truthy(x)).map(value_to_js_string);
            let group = r.get("group").filter(|x| value_truthy(x)).map_or(String::new(), value_to_js_string);
            let y = yield_of(code, ha, Some(p), name.as_deref());
            let none = no_yield(code, &group);
            let to = if none { None } else { proposed_of(code, p) };
            let kind = if none {
                "none"
            } else if y.assumed {
                "assumed"
            } else if !y.lu.is_empty() {
                "zoned"
            } else {
                "missing"
            };
            let label = if none { std_::YIELDS_NOT_LAND.to_string() } else if y.assumed { std_::YIELDS_ASSUMED.to_string() } else { y.label.clone() };
            CapRow { proposed: to.map(|t| yield_of(t, ha, Some(p), None)), y, group, kind, label }
        })
        .collect();
    let (mut total, mut assumed_total, mut proposed_total) = (Vec::new(), Vec::new(), Vec::new());
    for r in &rows {
        if r.kind == "zoned" {
            add_lu(&mut total, &r.y.lu);
        }
        if r.kind == "assumed" {
            add_lu(&mut assumed_total, &r.y.lu);
        }
        match &r.proposed {
            Some(pr) => add_lu(&mut proposed_total, &pr.lu),
            None if r.kind == "zoned" => add_lu(&mut proposed_total, &r.y.lu),
            None => {}
        }
    }
    let any = rows.iter().any(|r| r.proposed.is_some());
    ZonedCapacity { rows, total, assumed_total, proposed_total: any.then_some(proposed_total) }
}

/// `applyYield(z, code, ha, P)`: fills a zone's land use from the zoned yield of an AUP zone code and a site area, with the source of each quantity.
pub fn apply_yield(z: &mut Zone, p: &Project, ids: &dyn IdSource, code: f64, ha: f64) -> Yield {
    let y = yield_of(code, ha, Some(p), None);
    let mut lu: OrderedMap<Num> = OrderedMap::new();
    for (k, q) in &y.lu {
        lu.insert(k.clone(), Num::number(*q));
    }
    z.lu = Maybe::Set(lu);
    z.extra.insert("site".into(), json!({ "code": json_number(code), "ha": json_number(ha) }));
    let mut src = Map::new();
    let source = if y.user {
        format!("User-entered yield for {}", y.name)
    } else {
        format!("{} ({}{})", std_::YIELDS_LABEL, y.name, if y.assumed { format!("; {}", std_::YIELDS_ASSUMED) } else { String::new() })
    };
    let today = ids.today();
    for (k, q) in &y.lu {
        src.insert(
            k.clone(),
            json!({ "source": source, "basis": if y.user { "user" } else { "assumed" }, "date": today, "q": json_number(*q),
                "note": format!("{}{}", y.basis, if y.note.is_empty() { String::new() } else { format!(". {}", y.note) }) }),
        );
    }
    z.extra.insert("luSrc".into(), Value::Object(src));
    y
}
