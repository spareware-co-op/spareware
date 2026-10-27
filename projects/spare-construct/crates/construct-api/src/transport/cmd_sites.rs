//! Junction site commands: detect, merge, split, remove a junction from a site, name and set.

use super::Session;
use super::helpers::val;
use super::registry::Registry;
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::edit::sites::{self, DetectOpts, SiteRules};
use construct_transport::jsnum::{Maybe, Num, math_max};
use construct_transport::project::SiteSource;
use serde_json::{Value, json};

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    r.add(
        "transport.sites.list",
        "List the junction sites of a scenario (junction nodes that act as one intersection) with their names and the nodes in each.",
        false,
        params(vec![("scenario", sc())], &[]),
        sites_list,
    );
    r.add(
        "transport.sites.detect",
        "Group the junction nodes of each intersection into sites automatically (sites made by hand are kept).",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("rules", free_object("Rule values to change: near, wide, at, diameter (metres), dualBearing (degrees), dualGap (metres).")),
            ],
            &[],
        ),
        sites_detect,
    );
    r.add(
        "transport.sites.merge",
        "Merge junction nodes and/or sites into one site made by hand.",
        true,
        params(
            vec![
                ("scenario", sc()),
                (
                    "items",
                    strings("Junction node ids and/or site ids: at least two nodes result."),
                ),
            ],
            &["items"],
        ),
        sites_merge,
    );
    r.add(
        "transport.sites.split",
        "Split a site back into separate junctions.",
        true,
        params(
            vec![("scenario", sc()), ("site", string("Site id."))],
            &["site"],
        ),
        sites_split,
    );
    r.add(
        "transport.sites.remove_node",
        "Take one junction out of a site; with fewer than two nodes left the site is split.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("site", string("Site id.")),
                ("node", string("Junction id.")),
            ],
            &["site", "node"],
        ),
        sites_remove_node,
    );
    r.add(
        "transport.sites.set",
        "Change a site's name, control, cycle time, fixed-timing flag or AT site number, as the site panel does.",
        true,
        params(
            vec![
                ("scenario", sc()),
                ("site", string("Site id.")),
                ("values", free_object("Fields: name, control (blank clears), cycle (blank clears), fixedTiming, atSiteNo (blank clears; commas make a list).")),
            ],
            &["site", "values"],
        ),
        sites_set,
    );
}

fn site_json(
    sc: &construct_transport::project::Scenario,
    site: &construct_transport::project::Site,
) -> Value {
    let mut v = val(site);
    v["displayName"] = json!(sites::name(sc, site));
    v
}

fn sites_list(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sc = &s.project.scenarios[i];
    let list: Vec<Value> = sc
        .sites
        .get()
        .map(|m| m.values().map(|x| site_json(sc, x)).collect())
        .unwrap_or_default();
    Ok(json!({ "scenario": sc.id, "count": list.len(), "sites": list }))
}

fn rules_from(p: &Params) -> ApiResult<SiteRules> {
    let mut r = SiteRules::default();
    if let Some(o) = p.opt_object("rules")? {
        for (k, v) in o {
            let x = v
                .as_f64()
                .ok_or_else(|| ApiError::bad_params(format!("`rules.{k}` must be a number")))?;
            match k.as_str() {
                "near" => r.near = x,
                "wide" => r.wide = x,
                "at" => r.at = x,
                "diameter" => r.diameter = x,
                "dualBearing" => r.dual_bearing = x,
                "dualGap" => r.dual_gap = x,
                _ => {
                    return Err(ApiError::bad_params(format!(
                        "`rules.{k}` is not a rule; the rules are near, wide, at, diameter, dualBearing and dualGap"
                    )));
                }
            }
        }
    }
    Ok(r)
}

fn sites_detect(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let rules = rules_from(p)?;
    let points = sites::at_points(&s.project);
    s.snap("");
    let r = sites::detect(
        &mut s.project.scenarios[i],
        &mut s.ids,
        &DetectOpts { rules, points },
    );
    s.edited();
    Ok(json!({ "sites": r.sites, "nodes": r.nodes, "kept": r.kept }))
}

fn sites_merge(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let items = p.opt_strings("items")?.unwrap_or_default();
    if items.len() < 2 {
        return Err(ApiError::bad_params(
            "Give two or more junctions or sites to merge.",
        ));
    }
    s.snap("");
    match sites::merge(&mut s.project.scenarios[i], &mut s.ids, &items) {
        Some(id) => {
            s.edited();
            let sc = &s.project.scenarios[i];
            let site = &sc.sites.get().expect("just made")[id.as_str()];
            Ok(json!({ "site": id, "name": sites::name(sc, site), "nodes": site.nodes }))
        }
        None => {
            s.discard_snap();
            Err(ApiError::invalid_state(
                "Fewer than two junction nodes result, so no site was made.",
            ))
        }
    }
}

fn sites_split(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let id = p.str("site")?;
    if !s.project.scenarios[i]
        .sites
        .get()
        .is_some_and(|m| m.contains_key(id))
    {
        return Err(ApiError::not_found(format!("There is no site {id}.")));
    }
    s.snap("");
    sites::split(&mut s.project.scenarios[i], id);
    s.edited();
    Ok(json!({ "split": id }))
}

fn sites_remove_node(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let (id, node) = (p.str("site")?, p.str("node")?);
    let Some(site) = s.project.scenarios[i].sites.get().and_then(|m| m.get(id)) else {
        return Err(ApiError::not_found(format!("There is no site {id}.")));
    };
    if !site.nodes.iter().any(|n| n == node) {
        return Err(ApiError::not_found(format!(
            "Junction {node} is not in site {id}."
        )));
    }
    s.snap("");
    let sc = &mut s.project.scenarios[i];
    let remaining = {
        let site = sc
            .sites
            .get_mut()
            .and_then(|m| m.get_mut(id))
            .expect("checked");
        site.nodes.retain(|n| n != node);
        site.nodes.len()
    };
    if remaining < 2 {
        sites::split(sc, id);
    } else if let Some(site) = sc.sites.get_mut().and_then(|m| m.get_mut(id)) {
        let mut src = site.src.get().cloned().unwrap_or(SiteSource {
            nodes: Maybe::Missing,
            at: Maybe::Missing,
            rule: Maybe::Missing,
            at_site_no: Maybe::Missing,
            extra: Default::default(),
        });
        src.nodes = Maybe::Set("user".to_string());
        site.src = Maybe::Set(src);
    }
    s.edited();
    Ok(json!({ "site": id, "removed": node, "split": remaining < 2 }))
}

fn sites_set(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.target(p)?;
    let id = p.str("site")?.to_string();
    let values = p.object("values")?;
    if values.is_empty() {
        return Err(ApiError::bad_params("`values` has no fields in it"));
    }
    if !s.project.scenarios[i]
        .sites
        .get()
        .is_some_and(|m| m.contains_key(&id))
    {
        return Err(ApiError::not_found(format!("There is no site {id}.")));
    }
    for k in values.keys() {
        if !["name", "control", "cycle", "fixedTiming", "atSiteNo"].contains(&k.as_str()) {
            return Err(ApiError::bad_params(format!(
                "`values.{k}` is not a site field; the fields are name, control, cycle, fixedTiming and atSiteNo"
            )));
        }
    }
    s.snap("");
    let mut model = false;
    let site = s.project.scenarios[i]
        .sites
        .get_mut()
        .and_then(|m| m.get_mut(&id))
        .expect("checked");
    for (k, v) in values {
        let text = || match v {
            Value::String(t) => t.trim().to_string(),
            Value::Null => String::new(),
            o => construct_transport::jsnum::value_to_js_string(o),
        };
        match k.as_str() {
            "name" => site.name = Maybe::Set(text()),
            "control" => {
                let t = match v {
                    Value::String(t) => t.clone(),
                    _ => String::new(),
                };
                site.control = if t.is_empty() {
                    Maybe::Null
                } else {
                    Maybe::Set(t)
                };
                model = true;
            }
            "cycle" => {
                site.cycle = match v {
                    Value::Null => Maybe::Null,
                    Value::String(t) if t.is_empty() => Maybe::Null,
                    o => {
                        let x = construct_transport::jsnum::value_to_number(o);
                        Num::number(math_max(0.0, if x.is_nan() || x == 0.0 { 0.0 } else { x }))
                    }
                };
                model = true;
            }
            "fixedTiming" => {
                site.fixed_timing = match v {
                    Value::Bool(b) => (*b).into(),
                    _ => {
                        return Err(ApiError::bad_params(
                            "`values.fixedTiming` must be true or false",
                        ));
                    }
                };
                model = true;
            }
            _ => {
                let t = text();
                site.at_site_no = if t.is_empty() {
                    Maybe::Null
                } else if t.contains(',') {
                    Maybe::Set(Value::Array(
                        t.split(',')
                            .map(|x| x.trim())
                            .filter(|x| !x.is_empty())
                            .map(|x| json!(x))
                            .collect(),
                    ))
                } else {
                    Maybe::Set(json!(t))
                };
            }
        }
    }
    if model {
        s.edited();
    } else {
        s.touch();
    }
    let sc = &s.project.scenarios[i];
    Ok(json!({ "site": site_json(sc, &sc.sites.get().expect("sites")[id.as_str()]) }))
}
