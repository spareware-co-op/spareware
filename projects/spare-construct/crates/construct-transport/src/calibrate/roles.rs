//! Count roles and observation methods of the calibration panel: which counts calibrate the model, which are held
//! out to validate it, which set gateway flows and which are not used (`Calibration.roleOf`, `holdOut` and friends).

use super::common::{calib_cfg, CalibCfg, hash};
use super::criteria::{METHODS, corridor, panel_method, role_name};
use super::store::CalibStore;
use crate::datahub::{AsText, Count, Data, Matches, matches, source};
use crate::graph::js_sort_by;
use crate::jsnum::{Maybe, string_to_number};
use crate::project::{Project, Scenario, Settings, road_class};
use serde_json::Value;
use std::cell::OnceCell;
use std::collections::HashSet;
use std::rc::Rc;

/// The counts the matrix estimation of a period saw (`meUsed`).
#[derive(Clone, Debug, PartialEq)]
pub enum MeUsed {
    /// No estimation record for the period.
    None,
    /// An estimation outside the panel, which saw every count.
    All,
    /// An estimation from the panel, which saw these counts.
    Ids(HashSet<String>),
}

/// Everything the calibration panel reads from a project: its counts, their roles, the settings; the state behind `Calibration`'s closures.
pub struct Ctx<'a> {
    /// The project (`Store.P`).
    pub p: &'a Project,
    /// Its counts and data switches (`D()`).
    pub data: Data,
    /// Its calibration store (`store()`).
    pub store: CalibStore,
    /// Its calibration settings (`cfg()`).
    pub cfg: CalibCfg,
    gw: OnceCell<HashSet<String>>,
}

impl<'a> Ctx<'a> {
    /// Reads the project's counts, roles and calibration settings.
    pub fn new(p: &'a Project) -> Ctx<'a> {
        Ctx {
            p,
            data: Data::read(p),
            store: CalibStore::read(p),
            cfg: calib_cfg(Some(&p.settings)),
            gw: OnceCell::new(),
        }
    }
    /// `Store.S`.
    pub fn s(&self) -> &Settings {
        &self.p.settings
    }
    /// `DataHub.matches(sc)`.
    pub fn matches(&self, sc: &Scenario) -> Matches {
        matches(sc, self.s(), &self.data)
    }
    /// `gatewayIds()`: counts that set gateway flows (each gateway's `gwSrc` text names its counts).
    pub fn gateway_ids(&self) -> &HashSet<String> {
        self.gw.get_or_init(|| {
            let mut srcs: Vec<String> = Vec::new();
            for s in &self.p.scenarios {
                for z in s.zones.values() {
                    if z.kind.is("gateway")
                        && let Some(Value::String(g)) = z.gw_src.get()
                        && !g.is_empty()
                    {
                        srcs.push(g.clone());
                    }
                }
            }
            let mut out = HashSet::new();
            if !srcs.is_empty() {
                for c in &self.data.counts {
                    let l = count_label(c);
                    if srcs.iter().any(|g| g.contains(&l)) {
                        out.insert(c.id.clone());
                    }
                }
            }
            out
        })
    }
    /// `baseRole(c)`: the role the user (or the hold-out action) gave; "me" when none was given.
    pub fn base_role(&self, c: &Count) -> &'static str {
        match self.store.role(&c.id).as_text() {
            Some(r) if r != "gateway" => match r {
                "me" => "me",
                "valid" => "valid",
                "off" => "off",
                _ => "me",
            },
            _ => "me",
        }
    }
    /// `meUsed(period)`: the counts matrix estimation saw in this period.
    pub fn me_used(&self, period: &str) -> MeUsed {
        let info = self
            .p
            .me
            .get()
            .and_then(|m| m.info.get())
            .and_then(|i| i.get(period))
            .and_then(|i| i.get());
        match info {
            None => MeUsed::None,
            Some(i) => match i.extra.get("countIds") {
                Some(Value::Array(a)) => MeUsed::Ids(a.iter().map(crate::jsnum::value_to_js_string).collect()),
                _ => MeUsed::All,
            },
        }
    }
    /// `tefUsed(period)`: counts that fitted trip-end factors now applied to the trip rates (any scenario) in this period.
    pub fn tef_used(&self, period: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        for s in &self.p.scenarios {
            let r = s
                .extra
                .get("calib")
                .and_then(|c| c.get("tripEndFactors"))
                .and_then(|t| t.get(period));
            if let Some(r) = r
                && r.get("applied").is_some_and(crate::jsnum::value_truthy)
                && let Some(Value::Array(a)) = r.get("countIds")
            {
                out.extend(a.iter().map(crate::jsnum::value_to_js_string));
            }
        }
        out
    }
    /// `roleOf(c, period, raw)`: the role that counts for statistics in a period.
    ///
    /// Gateway use always wins (it sets demand), and a validation count that matrix estimation saw (an estimation run
    /// outside the panel saw every count) or that fitted applied trip-end factors is calibration. `raw` gives the role
    /// without those two rules.
    pub fn role_of(&self, c: &Count, period: &str, raw: bool) -> &'static str {
        let r = self.base_role(c);
        if r == "off" {
            return "off";
        }
        if self.gateway_ids().contains(&c.id) {
            return "gateway";
        }
        if raw || r != "valid" {
            return r;
        }
        let used = match self.me_used(period) {
            MeUsed::None => false,
            MeUsed::All => true,
            MeUsed::Ids(s) => s.contains(&c.id),
        };
        if used || self.tef_used(period).contains(&c.id) {
            "me"
        } else {
            "valid"
        }
    }
    /// `seenBy(c, period)`: why a count given the validation role is a calibration count in this period ("" when it is not).
    pub fn seen_by(&self, c: &Count, period: &str) -> &'static str {
        if self.base_role(c) != "valid" || self.role_of(c, period, false) != "me" {
            return "";
        }
        let used = match self.me_used(period) {
            MeUsed::None => false,
            MeUsed::All => true,
            MeUsed::Ids(s) => s.contains(&c.id),
        };
        if used { "seen by ME" } else { "used by trip-end factors" }
    }
}

/// `countLabel(c)`: how a count is named in a gateway's `gwSrc` ("NZTA 12 Great South Road", "NZTA estimate V1 ...").
pub fn count_label(c: &Count) -> String {
    let short = match c.src.as_text().and_then(source) {
        Some(s) => s.short.to_string(),
        None => c.src.js_string(),
    };
    format!(
        "{}{} {} {}",
        short,
        if c.virtual_.truthy() { " estimate" } else { "" },
        if c.reference.truthy() { c.reference.js_string() } else { String::new() },
        c.name.js_string()
    )
}

/// `methodOf(c, period)`: the observation method of a count in a period, from the panel's four methods (an importer's own `obs_method` wins; a user count is a measured hour when it has that period's hour; an AT count when its counted peak falls in the period's window).
pub fn method_of(c: &Count, period: &str) -> &'static str {
    if let Some(m) = c.obs_method.as_text()
        && let Some(pm) = panel_method(m)
    {
        return pm.key;
    }
    if c.virtual_.truthy() {
        return "virtual";
    }
    if c.src_is("user") {
        let v = if period == "AM" { &c.am } else { &c.pm };
        return if v.truthy() { "measured_hour" } else { "adt_factored" };
    }
    if c.src_is("at") && c.peak.truthy() && c.peak_time.truthy() {
        let pt = c.peak_time.js_string();
        let u: Vec<u16> = pt.encode_utf16().collect();
        let part = |a: usize, b: Option<usize>| -> f64 {
            let end = b.unwrap_or(u.len()).min(u.len());
            if a >= end {
                0.0
            } else {
                string_to_number(&String::from_utf16_lossy(&u[a..end]))
            }
        };
        let h = part(0, Some(2)) + part(3, None) / 60.0;
        if (period == "AM" && h >= 6.0 && h < 10.0) || (period == "PM" && h >= 14.5 && h < 19.0) {
            return "measured_hour";
        }
    }
    "adt_factored"
}

/// `METHODS[method].weight` for a count in a period, as the panel gives matrix estimation (0.4 for a method it does not list).
pub fn method_weight(c: &Count, period: &str) -> f64 {
    panel_method(method_of(c, period)).map_or(0.4, |m| m.weight)
}

fn sync_value(p: &mut Project, roles: &[(String, &'static str)]) {
    if let Some(d) = p.data.get_mut()
        && let Some(Value::Array(arr)) = d.extra.get_mut("counts")
    {
        for (i, (_, r)) in roles.iter().enumerate() {
            if let Some(Value::Object(o)) = arr.get_mut(i) {
                o.insert("role".to_string(), Value::String((*r).to_string()));
            }
        }
    }
}

/// `syncCounts()`: mirrors the roles onto the counts (`c.role`), so other features can read them.
pub fn sync_counts(p: &mut Project) {
    let roles: Vec<(String, &'static str)> = {
        let ctx = Ctx::new(p);
        let period = p.settings.period.as_str().filter(|x| !x.is_empty()).unwrap_or("AM").to_string();
        ctx.data
            .counts
            .iter()
            .map(|c| (c.id.clone(), ctx.role_of(c, &period, true)))
            .collect()
    };
    sync_value(p, &roles);
}

/// `setRole(id, role)`: gives a count a role ("valid" or "off"; "me" or anything else clears it), then mirrors the roles.
pub fn set_role(p: &mut Project, id: &str, role: &str) {
    let mut st = CalibStore::read(p);
    if role == "me" || role_name(role).is_none() || role == "gateway" {
        st.roles.remove(id);
    } else {
        st.roles.insert(id.to_string(), Maybe::Set(crate::jsnum::JsNum::Text(role.to_string())));
    }
    st.write(p);
    sync_counts(p);
}

/// `holdOut(sc, every)`: holds out about 25% of the counts for validation (count-roles).
///
/// Eligible counts (in use, not virtual, matched, not gateway counts, not switched off) are sorted by road-class rank,
/// then by a hash of the id, and every `every`th becomes "valid" (deterministic; Spare Construct convention). The rest
/// become calibration counts; every role except "not used" is cleared first. Returns (validation, calibration) counts.
pub fn hold_out(p: &mut Project, sc_id: &str, every: usize) -> (usize, usize) {
    let (ids, mut st) = {
        let ctx = Ctx::new(p);
        let sc = p.scenario(sc_id).expect("scenario");
        let m = ctx.matches(sc);
        let gw = ctx.gateway_ids();
        struct E {
            c: Rc<Count>,
            rank: f64,
            h: u32,
        }
        let mut el: Vec<E> = ctx
            .data
            .counts
            .iter()
            .filter(|c| ctx.data.in_use(c) && !c.virtual_.truthy() && m.get(&c.id).is_some() && !gw.contains(&c.id) && ctx.base_role(c) != "off")
            .map(|c| {
                let l = sc.links.get(&m.get(&c.id).expect("matched").link);
                E {
                    c: c.clone(),
                    rank: l.map_or(0.0, |l| road_class(l.cls.as_str()).rank),
                    h: hash(&c.id),
                }
            })
            .collect();
        js_sort_by(&mut el, |a, b| {
            let d = b.rank - a.rank;
            if d != 0.0 && !d.is_nan() { d } else { a.h as f64 - b.h as f64 }
        });
        (el.into_iter().map(|e| e.c.id.clone()).collect::<Vec<String>>(), ctx.store.clone())
    };
    // a fresh hold-out: every role except 'not used' is cleared first, so the result depends only on the counts
    let keys: Vec<String> = st.roles.keys().map(String::from).collect();
    for id in keys {
        if !st.roles.num_at(&id).as_text().is_some_and(|t| t == "off") {
            st.roles.remove(&id);
        }
    }
    let mut k = 0;
    for (i, id) in ids.iter().enumerate() {
        if i % every == every - 1 {
            st.roles.insert(id.clone(), Maybe::Set(crate::jsnum::JsNum::Text("valid".to_string())));
            k += 1;
        } else {
            st.roles.remove(id);
        }
    }
    st.write(p);
    sync_counts(p);
    (k, ids.len() - k)
}

/// `holdOut` with the default of one in four (`CORRIDOR.holdEvery`).
pub fn hold_out_default(p: &mut Project, sc_id: &str) -> (usize, usize) {
    hold_out(p, sc_id, corridor::HOLD_EVERY)
}

/// `resetRoles()`: clears every role except "not used".
pub fn reset_roles(p: &mut Project) {
    let mut st = CalibStore::read(p);
    let keys: Vec<String> = st.roles.keys().map(String::from).collect();
    for id in keys {
        if !st.roles.num_at(&id).as_text().is_some_and(|t| t == "off") {
            st.roles.remove(&id);
        }
    }
    st.write(p);
    sync_counts(p);
}

#[allow(dead_code)]
fn _methods() -> usize {
    METHODS.len()
}
