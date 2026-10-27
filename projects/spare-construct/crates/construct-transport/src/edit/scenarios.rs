//! Assessment scenarios: model.js `Scenarios`.
//!
//! Scenario roles (existing, baseline, proposal, mitigation, sensitivity, permitted), years and staging; the
//! "Create assessment set" wizard that makes S0 to S3 for the opening and design years; sensitivity copies;
//! copies of a scenario with a filter on its roads and zones; what a scenario contains in words; and the checks on
//! a scenario (reg-receiving-env). Links carry a stage (existing, committed, unfunded, proposal, mitigation) and
//! zones a role (existing, committed, development; gateways are external).

use super::register::in_baseline;
use super::{EditError, IdSource, PATHWAYS, ZONE_ROLES, name_of, scenario_role};
use crate::graph::js_sort_by;
use crate::jsnum::{JsNum, Maybe, Num, number_to_js_string, or_default, to_fixed, value_truthy};
use crate::project::{
    Link, Project, Scenario, Zone, link_programmed, link_stage, link_unfunded, new_scenario,
};
use crate::standards::{STANDARDS, growth, growth_basis, year_of};
use crate::util::clamp;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;

pub use crate::project::{
    link_programmed as programmed, link_stage as stage_of, link_unfunded as unfunded,
};

/// `Scenarios.roleOf(sc)`: the scenario's role key when it is a known role, otherwise None.
pub fn role_of(sc: &Scenario) -> Option<&str> {
    sc.role.as_str().filter(|r| scenario_role(r).is_some())
}

/// `Scenarios.zoneRole(z)`: "external" for a gateway, else the zone's role if it has a known one, else "existing" for an
/// existing land use and "development" for anything else.
pub fn zone_role(z: &Zone) -> &str {
    if z.kind.is("gateway") {
        return "external";
    }
    if let Some(r) = z.role.as_str()
        && name_of(&ZONE_ROLES, r).is_some()
    {
        return r;
    }
    if z.existing.is_true() {
        "existing"
    } else {
        "development"
    }
}

/// `Scenarios.zoneInBaseline(z, P)`: a committed zone belongs in the baseline unless its register row says otherwise.
pub fn zone_in_baseline(z: &Zone, p: &Project) -> bool {
    match z.reg.as_str() {
        Some(r) if !r.is_empty() => in_baseline(p.register_row(r)).0,
        _ => true,
    }
}

/// `Scenarios.designYear(type, opening)`: the default design year for a consenting pathway (NZTA RR 422 s5.5; AT ITA 3.6).
pub fn design_year(pathway: &str, opening: f64) -> f64 {
    let d = &STANDARDS.design;
    let years = match pathway {
        "consent" => d.consent,
        "planchange" => d.planchange,
        "nor" => d.nor,
        "ftaa" => d.ftaa,
        _ => d.consent,
    };
    or_default(opening, 0.0) + years
}

/* ---------------- copies ---------------- */

/// The filters of `Scenarios.copy(src, f)`: a road or zone is kept when its filter says so (no filter keeps all).
#[derive(Clone, Copy, Default)]
pub struct CopyFilter<'a> {
    /// Keep a link when this is true.
    pub links: Option<&'a dyn Fn(&Link) -> bool>,
    /// Keep a zone when this is true.
    pub zones: Option<&'a dyn Fn(&Zone) -> bool>,
}

/// `Scenarios.copy(src, f)`: a copy of a scenario with only the links and zones that pass the filters. No results; ids are
/// kept so results compare link by link. Junctions left unused are pruned, and sites with fewer than two nodes left go.
pub fn copy(src: &Scenario, ids: &mut dyn IdSource, f: &CopyFilter) -> Scenario {
    let mut sc = new_scenario(&ids.uid("s"), "");
    sc.name = src.name.clone();
    sc.nodes = src.nodes.clone();
    sc.links = src.links.clone();
    sc.zones = src.zones.clone();
    sc.routes = src.routes.clone();
    // `src.odManual || null`
    sc.od_manual = match &src.od_manual {
        Maybe::Set(m) => Maybe::Set(m.clone()),
        _ => Maybe::Null,
    };
    if let Maybe::Set(s) = &src.sites {
        sc.sites = Maybe::Set(s.clone());
    }
    let mut gone: Vec<String> = Vec::new();
    if let Some(keep) = f.links {
        let drop: Vec<String> = sc
            .links
            .values()
            .filter(|l| !keep(l))
            .map(|l| l.id.clone())
            .collect();
        for id in drop {
            if let Some(l) = sc.links.remove(&id) {
                gone.push(l.a);
                gone.push(l.b);
            }
        }
    }
    if let Some(keep) = f.zones {
        let drop: Vec<String> = sc
            .zones
            .values()
            .filter(|z| !keep(z))
            .map(|z| z.id.clone())
            .collect();
        for id in drop {
            sc.zones.remove(&id);
        }
    }
    for n in &gone {
        super::net::prune_node(&mut sc, n);
    }
    let nodes = sc.nodes.clone();
    if let Some(sites) = sc.sites.get_mut() {
        let mut drop: Vec<String> = Vec::new();
        for s in sites.values_mut() {
            s.nodes.retain(|id| nodes.contains_key(id));
            if s.nodes.len() < 2 {
                drop.push(s.id.clone());
            }
        }
        for id in drop {
            sites.remove(&id);
        }
    }
    sc
}

/* ---------------- the assessment-set wizard ---------------- */

/// What `Scenarios.createSet(P, opts)` takes; every field is optional.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CreateSetOpts {
    /// Consenting pathway: "consent" (the default), "planchange", "nor" or "ftaa".
    pub pathway: Option<String>,
    /// Year of the traffic counts (default the base year).
    pub count_year: Option<f64>,
    /// Opening year (default the count year).
    pub opening: Option<f64>,
    /// Design year (default the pathway's years after opening).
    pub design: Option<f64>,
    /// Id of the scenario to copy from (default the active one).
    pub from: Option<String>,
    /// False leaves out the S3 mitigation scenarios.
    pub mitigation: Option<bool>,
    /// Periods to assess (default AM and PM).
    pub periods: Option<Vec<String>>,
    /// Build-out (% of the development zones built) by year, keyed by the year as text ("2036").
    pub buildout: HashMap<String, f64>,
}

/// What `Scenarios.createSet` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateSetResult {
    /// Ids of the scenarios made, in order.
    pub scenarios: Vec<String>,
    /// Roads and zones left out of the baseline, each { kind, id, name, reason, ... }.
    pub excluded: Vec<Value>,
    /// Warnings for the author.
    pub warnings: Vec<String>,
    /// The set record written to `P.assessment`.
    pub set: Value,
}

/// `Scenarios.createSet(P, opts)`: S0 existing (count year), then for the opening and design years S1 future baseline
/// (existing plus committed developments and funded projects), S2 = S1 plus the proposal, S3 = S2 plus mitigation.
/// Unfunded and not-granted items stay out of S1 and are listed in `excluded`. Adds the scenarios to the project.
pub fn create_set(
    p: &mut Project,
    ids: &mut dyn IdSource,
    opts: &CreateSetOpts,
) -> Result<CreateSetResult, EditError> {
    let pathway = match opts.pathway.as_deref() {
        Some(t) if name_of(&PATHWAYS, t).is_some() => t.to_string(),
        _ => "consent".to_string(),
    };
    let from = opts.from.as_deref().filter(|s| !s.is_empty());
    let want = from.or(p.active.as_str());
    let src: Scenario = p
        .scenarios
        .iter()
        .find(|s| want.is_some_and(|w| s.id == w))
        .or(p.scenarios.first())
        .ok_or_else(|| EditError("the project has no scenarios".to_string()))?
        .clone();
    let base_year = p.settings.base_year.num();
    let count_year = or_default(opts.count_year.unwrap_or(f64::NAN), base_year);
    let opening = or_default(opts.opening.unwrap_or(f64::NAN), count_year);
    let design = or_default(
        opts.design.unwrap_or(f64::NAN),
        design_year(&pathway, opening),
    );
    let periods: Vec<String> = match &opts.periods {
        Some(v) if !v.is_empty() => v.clone(),
        _ => vec!["AM".to_string(), "PM".to_string()],
    };
    let set_id = ids.uid("set");
    let mut years: Vec<f64> = Vec::new();
    for y in [opening, design] {
        if !years.iter().any(|v| *v == y || (v.is_nan() && y.is_nan())) {
            years.push(y);
        }
    }
    js_sort_by(&mut years, |a, b| a - b);
    let mut excluded: Vec<Value> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for l in src.links.values() {
        if (link_unfunded(l) || link_programmed(l)) && !l.status.is("closed") {
            excluded.push(json!({
                "kind": "link",
                "id": l.id,
                "name": l.name.str_or("Unnamed road"),
                "stage": link_stage(l),
                "funding": l.funding.as_str().filter(|f| !f.is_empty()),
                "reason": if l.funding.is("programmed") {
                    "Programmed but not funded: sensitivity scenarios only (AT ITA s3.6.1)."
                } else {
                    "Unfunded: sensitivity scenarios only (AT ITA s3.6.1)."
                },
            }));
        }
    }
    for z in src.zones.values() {
        if zone_role(z) == "committed" && !zone_in_baseline(z, p) {
            let row = z.reg.as_str().and_then(|r| p.register_row(r));
            excluded.push(json!({
                "kind": "zone",
                "id": z.id,
                "name": z.name.str_or("Unnamed zone"),
                "reason": in_baseline(row).1,
            }));
        }
    }
    let mut out: Vec<Scenario> = Vec::new();
    // make(role, year, net, lu, buildout)
    let mut make = |p: &Project,
                    ids: &mut dyn IdSource,
                    role: &str,
                    year: f64,
                    net: &[&str],
                    lu: &[&str],
                    buildout: Option<f64>| {
        let r = scenario_role(role).expect("a known role");
        let keep_link =
            |l: &Link| net.contains(&link_stage(l)) && !link_unfunded(l) && !link_programmed(l);
        let keep_zone = |z: &Zone| {
            let zr = zone_role(z);
            zr == "external" || (lu.contains(&zr) && (zr != "committed" || zone_in_baseline(z, p)))
        };
        let mut sc = copy(
            &src,
            ids,
            &CopyFilter {
                links: Some(&keep_link),
                zones: Some(&keep_zone),
            },
        );
        sc.name = Maybe::Set(format!(
            "{} \u{b7} {} {}",
            r.code,
            number_to_js_string(year),
            r.short.to_lowercase()
        ));
        sc.role = Maybe::Set(role.to_string());
        sc.year = Num::number(year);
        sc.periods = Maybe::Set(periods.clone());
        sc.code = Maybe::Set(r.code.to_string());
        sc.set_id = Maybe::Set(set_id.clone());
        sc.network = Maybe::Set(
            if net.contains(&"mitigation") {
                "mitigation"
            } else if net.contains(&"committed") {
                "committed"
            } else {
                "existing"
            }
            .to_string(),
        );
        sc.notes = Maybe::Set(JsNum::Text(format!(
            "Created by the assessment-set wizard ({}).",
            name_of(&PATHWAYS, &pathway).unwrap_or("")
        )));
        if let Some(b) = buildout {
            sc.buildout = Num::number(b);
        }
        out.push(sc);
    };
    let ex = ["existing"];
    let base = ["existing", "committed"];
    let prop = ["existing", "committed", "proposal"];
    let mit = ["existing", "committed", "proposal", "mitigation"];
    let lu_base = ["existing", "committed"];
    let lu_dev = ["existing", "committed", "development"];
    make(p, ids, "existing", count_year, &ex, &["existing"], None);
    for &y in &years {
        let b = match opts.buildout.get(&number_to_js_string(y)) {
            Some(v) => clamp(*v, 0.0, 100.0),
            None => 100.0,
        };
        make(p, ids, "baseline", y, &base, &lu_base, None);
        make(p, ids, "proposal", y, &prop, &lu_dev, Some(b));
        if opts.mitigation != Some(false) {
            make(p, ids, "mitigation", y, &mit, &lu_dev, Some(b));
        }
    }
    let ahead = design - or_default(base_year, count_year);
    let warn_ahead = or_default(STANDARDS.design.warn_ahead, 10.0);
    let validated = p
        .data
        .get()
        .and_then(|d| d.msm.get())
        .and_then(|m| m.get("validated"))
        .is_some_and(value_truthy);
    if ahead > warn_ahead && !validated {
        warnings.push(format!(
            "The design year {} is {} years after the base year and no validated regional forecast is loaded. Check forecasts more than {} years ahead against MSM or Auckland Forecasting Centre outputs.",
            number_to_js_string(design),
            number_to_js_string(ahead),
            number_to_js_string(warn_ahead)
        ));
    }
    if opening < count_year {
        warnings.push(format!(
            "The opening year {} is before the count year {}. Check both years.",
            number_to_js_string(opening),
            number_to_js_string(count_year)
        ));
    }
    let made: Vec<String> = out.iter().map(|s| s.id.clone()).collect();
    p.scenarios.extend(out);
    let set = json!({
        "id": set_id,
        "type": pathway,
        "pathway": name_of(&PATHWAYS, &pathway).unwrap_or(""),
        "countYear": count_year,
        "opening": opening,
        "design": design,
        "periods": periods,
        "created": ids.now_iso(),
        "excluded": excluded,
        "warnings": warnings,
        "scenarios": made,
    });
    p.assessment = Maybe::Set(set.clone());
    Ok(CreateSetResult {
        scenarios: made,
        excluded,
        warnings,
        set,
    })
}

/// `Scenarios.sensitivityOf(P, scId, opts)`: adds a sensitivity copy of a scenario to the project and returns its id (None when
/// there is no such scenario). With `unfunded` the unfunded projects are put back in.
pub fn sensitivity_of(
    p: &mut Project,
    ids: &mut dyn IdSource,
    sc_id: &str,
    unfunded: bool,
) -> Option<String> {
    let src = p.scenarios.iter().find(|s| s.id == sc_id)?.clone();
    let keep = |l: &Link| unfunded || !link_unfunded(l);
    let mut sc = copy(
        &src,
        ids,
        &CopyFilter {
            links: Some(&keep),
            zones: None,
        },
    );
    let src_name = match &src.name {
        Maybe::Set(n) => n.clone(),
        Maybe::Null => "null".to_string(),
        Maybe::Missing => "undefined".to_string(),
    };
    sc.name = Maybe::Set(format!(
        "{} \u{b7} sensitivity{}",
        src_name,
        if unfunded {
            " (unfunded projects in)"
        } else {
            ""
        }
    ));
    sc.role = Maybe::Set("sensitivity".to_string());
    sc.sens_of = Maybe::Set(src.id.clone());
    if !src.year.is_nullish() {
        sc.year = src.year.clone();
    }
    if !src.periods.is_nullish() {
        sc.periods = src.periods.clone();
    }
    if !src.buildout.is_nullish() {
        sc.buildout = src.buildout.clone();
    }
    let id = sc.id.clone();
    p.scenarios.push(sc);
    Some(id)
}

/* ---------------- describing ---------------- */

/// What a scenario contains, in words (`Scenarios.describe`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    /// Scenario code ("S1"), or "".
    pub id: String,
    /// Scenario id.
    pub sc_id: String,
    /// Scenario name.
    #[serde(skip_serializing_if = "Maybe::is_missing")]
    pub name: Maybe<String>,
    /// Role key, or None.
    pub role: Option<String>,
    /// Role name ("Role not set").
    pub role_label: String,
    /// Year the scenario runs in.
    pub year: f64,
    /// Land uses in it ("Existing + committed").
    pub land_use: String,
    /// Network stages in it ("Existing + committed (funded)").
    pub network: String,
    /// Periods ("AM, PM").
    pub periods: String,
    /// Build-out (%) when it has development zones.
    pub buildout: Option<f64>,
    /// Growth basis text.
    pub growth_basis: String,
    /// Background growth factor from the base year.
    pub growth_factor: f64,
    /// "2036: background growth linear, 1.5% of 2026 volume per year (x 1.150 from 2026)".
    pub text: String,
}

/// `Scenarios.describe(sc, P)`: what a scenario contains, in words, with its year and growth basis.
pub fn describe(sc: &Scenario, p: &Project) -> Description {
    let s = &p.settings;
    let role = role_of(sc);
    let r = role.and_then(scenario_role);
    let lu: Vec<&str> = {
        let mut v: Vec<&str> = Vec::new();
        for z in sc.zones.values() {
            let zr = zone_role(z);
            if zr != "external" && !v.contains(&zr) {
                v.push(zr);
            }
        }
        v
    };
    let st: Vec<&str> = {
        let mut v: Vec<&str> = Vec::new();
        for l in sc.links.values() {
            if !l.status.is("closed") {
                let k = link_stage(l);
                if !v.contains(&k) {
                    v.push(k);
                }
            }
        }
        v
    };
    let year = year_of(Some(sc), s);
    let mut parts: Vec<String> = Vec::new();
    for rr in ["existing", "committed", "development"] {
        if lu.contains(&rr) {
            let i = parts.len();
            let mut t = if i == 0 {
                let mut c = rr.chars();
                let first = c.next().expect("non-empty").to_uppercase().to_string();
                first + c.as_str()
            } else {
                rr.to_string()
            };
            if rr == "development" && !sc.buildout.is_nullish() && sc.buildout.num() != 100.0 {
                t.push_str(&format!(
                    " ({}% built)",
                    number_to_js_string(sc.buildout.num())
                ));
            }
            parts.push(t);
        }
    }
    let land_use = if parts.is_empty() {
        "None".to_string()
    } else {
        parts.join(" + ")
    };
    let nw = [
        ("existing", "Existing"),
        ("committed", "committed (funded)"),
        ("unfunded", "unfunded"),
        ("proposal", "proposal"),
        ("mitigation", "mitigation"),
    ];
    let net: Vec<&str> = nw
        .iter()
        .filter(|(k, _)| st.contains(k))
        .map(|(_, v)| *v)
        .collect();
    let network = if net.is_empty() {
        "None".to_string()
    } else {
        net.join(" + ")
    };
    let gb = growth_basis(s);
    let gf = growth(s, Some(sc));
    let periods = match sc.periods.get() {
        Some(v) => v.join(", "),
        None => "AM, PM".to_string(),
    };
    Description {
        id: sc.code.str_or("").to_string(),
        sc_id: sc.id.clone(),
        name: sc.name.clone(),
        role: role.map(str::to_string),
        role_label: r.map_or("Role not set".to_string(), |r| r.label.to_string()),
        year,
        land_use,
        network,
        periods,
        buildout: if lu.contains(&"development") {
            Some(if !sc.buildout.is_nullish() {
                sc.buildout.num()
            } else {
                100.0
            })
        } else {
            None
        },
        growth_basis: gb.clone(),
        growth_factor: gf,
        text: format!(
            "{}: background growth {}{}",
            number_to_js_string(year),
            gb,
            format!(
                " (\u{d7} {} from {})",
                to_fixed(gf, 3),
                s.base_year.js_string()
            )
        ),
    }
}

/// `Scenarios.table(P)`: the assessment scenarios table (ID, role, year, land use, network, periods, build-out), by year,
/// then without and with the proposal.
pub fn table(p: &Project) -> Vec<Description> {
    let mut rows: Vec<Description> = p.scenarios.iter().map(|sc| describe(sc, p)).collect();
    let ord = |r: &Description| {
        r.role
            .as_deref()
            .and_then(scenario_role)
            .map_or(9.0, |x| x.order)
    };
    js_sort_by(&mut rows, |a, b| {
        let d = a.year - b.year;
        if d != 0.0 && !d.is_nan() {
            d
        } else {
            ord(a) - ord(b)
        }
    });
    rows
}

/// The scenarios of one year (`Scenarios.groups`): those without the proposal, with it, and the others.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct YearGroup {
    /// The year.
    pub year: f64,
    /// Scenario ids without the proposal.
    pub without: Vec<String>,
    /// Scenario ids with the proposal.
    #[serde(rename = "with")]
    pub with_proposal: Vec<String>,
    /// Other scenario ids (sensitivity tests and scenarios with no role).
    pub other: Vec<String>,
}

/// `Scenarios.groups(P)`: the table's scenario ids grouped by year, in table order.
pub fn groups(p: &Project) -> Vec<YearGroup> {
    let mut g: Vec<YearGroup> = Vec::new();
    for r in table(p) {
        let at = g
            .iter()
            .position(|x| x.year == r.year || (x.year.is_nan() && r.year.is_nan()));
        let k = match at {
            Some(k) => k,
            None => {
                g.push(YearGroup {
                    year: r.year,
                    without: Vec::new(),
                    with_proposal: Vec::new(),
                    other: Vec::new(),
                });
                g.len() - 1
            }
        };
        let side = r
            .role
            .as_deref()
            .and_then(scenario_role)
            .map_or("other", |x| x.side);
        match side {
            "without" => g[k].without.push(r.sc_id.clone()),
            "with" => g[k].with_proposal.push(r.sc_id.clone()),
            _ => g[k].other.push(r.sc_id.clone()),
        }
    }
    g
}

/// One finding of [`check`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Check {
    /// "block", "warn" or "info".
    pub level: String,
    /// Short code ("unfunded", "programmed", "dev-in-baseline", "not-granted", "permitted", "far-year").
    pub code: String,
    /// The road, register row or zone it is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// What to do about it.
    pub text: String,
}

/// `Scenarios.check(sc, P)`: checks on a scenario (reg-receiving-env). An unfunded project outside a sensitivity scenario
/// blocks; proposed or not-granted development in a baseline warns.
pub fn check(sc: &Scenario, p: &Project) -> Vec<Check> {
    let mut out: Vec<Check> = Vec::new();
    let role = role_of(sc);
    let s = &p.settings;
    let sc_name = sc.name.js_str();
    if role != Some("sensitivity") {
        for l in sc.links.values() {
            if l.status.is("closed") {
                continue;
            }
            let nm = l.name.str_or("An unnamed road");
            if link_unfunded(l) {
                out.push(Check {
                    level: "block".into(),
                    code: "unfunded".into(),
                    id: Some(l.id.clone()),
                    text: format!("{nm} is an unfunded project, and it is in {sc_name}. AT ITA Guidelines s3.6.1 allow unfunded projects only in sensitivity scenarios. Remove it from this scenario, or make the scenario a sensitivity test."),
                });
            } else if link_programmed(l) && role != Some("existing") {
                out.push(Check {
                    level: "warn".into(),
                    code: "programmed".into(),
                    id: Some(l.id.clone()),
                    text: format!("{nm} is programmed but not yet funded. Keep it out of the do-minimum unless its funding is confirmed (record the source), or test it as a sensitivity."),
                });
            }
        }
        if let Some(rows) = p.register.get() {
            for r in rows {
                if !r.row_type.is("network") || in_baseline(Some(r)).0 {
                    continue;
                }
                let found: Vec<&String> = r
                    .links
                    .get()
                    .map(|ls| {
                        ls.iter()
                            .filter(|id| sc.links.get(id).is_some_and(|l| !l.status.is("closed")))
                            .collect()
                    })
                    .unwrap_or_default();
                if !found.is_empty()
                    && !out
                        .iter()
                        .any(|o| o.id.as_ref().is_some_and(|oid| found.contains(&oid)))
                {
                    out.push(Check {
                        level: "block".into(),
                        code: "unfunded".into(),
                        id: r.id.get().cloned(),
                        text: format!("{} (register) is in {} but is not funded. Use it in a sensitivity scenario only (AT ITA s3.6.1).", r.name.js_string(), sc_name),
                    });
                }
            }
        }
    }
    if role == Some("baseline") || role == Some("existing") {
        let short = role
            .and_then(scenario_role)
            .map_or(String::new(), |r| r.short.to_lowercase());
        for z in sc.zones.values() {
            let zr = zone_role(z);
            let nm = z.name.str_or("A zone");
            if zr == "development" {
                out.push(Check {
                    level: "warn".into(),
                    code: "dev-in-baseline".into(),
                    id: Some(z.id.clone()),
                    text: format!("{nm} is proposed development, but this is the {short} scenario. Mark it committed (with its consent in the register) or remove it."),
                });
            } else if zr == "committed" && !zone_in_baseline(z, p) {
                out.push(Check {
                    level: "warn".into(),
                    code: "not-granted".into(),
                    id: Some(z.id.clone()),
                    text: format!("{nm} is committed development that is not granted and likely (see the register). The do-minimum should include only granted and likely developments."),
                });
            }
        }
    }
    if role == Some("permitted") {
        out.push(Check {
            level: "info".into(),
            code: "permitted".into(),
            id: None,
            text: "Permitted baseline: under RMA s104(2) the consent authority may disregard these effects.".into(),
        });
    }
    let yr = year_of(Some(sc), s);
    let validated = p
        .data
        .get()
        .and_then(|d| d.msm.get())
        .and_then(|m| m.get("validated"))
        .is_some_and(value_truthy);
    if yr - s.base_year.num() > 10.0 && !validated {
        out.push(Check {
            level: "warn".into(),
            code: "far-year".into(),
            id: None,
            text: format!(
                "{} is more than 10 years after the base year and no validated regional forecast is loaded.",
                number_to_js_string(yr)
            ),
        });
    }
    out
}

/// `String(x)` of a text field that may be missing or null.
trait JsStr {
    fn js_str(&self) -> String;
}

impl JsStr for Maybe<String> {
    fn js_str(&self) -> String {
        match self {
            Maybe::Set(n) => n.clone(),
            Maybe::Null => "null".to_string(),
            Maybe::Missing => "undefined".to_string(),
        }
    }
}
