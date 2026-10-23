//! Turning movements, junction diagrams, turning counts and the SIDRA export.

use super::Session;
use super::cmd_run::{held, pick};
use super::registry::Registry;
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::results::Res;
use construct_transport::turns::{self, DiagramOpts, SidraOpts};
use serde_json::{Value, json};

pub(super) fn register(r: &mut Registry) {
    let sc = || string("Scenario id or name (default: the active scenario).");
    let per = || {
        choice(
            &["AM", "PM"],
            "Peak hour (default the project's current period).",
        )
    };
    r.add(
        "transport.turns.movements",
        "A junction's turning movements from the run, by approach: left, through, right and U-turn flows with SIDRA leg and movement codes. The id is a junction or a junction site.",
        false,
        params(vec![("scenario", sc()), ("period", per()), ("id", string("Junction id, or junction site id."))], &["id"]),
        turns_movements,
    );
    r.add(
        "transport.turns.junction_name",
        "The name of a junction: its own name, or the roads that meet there, highest class first.",
        false,
        params(
            vec![("scenario", sc()), ("node", string("Junction id."))],
            &["node"],
        ),
        |s, p| {
            let i = s.pick_scenario(p, "scenario")?;
            let n = p.str("node")?;
            let sc = &s.project.scenarios[i];
            if !sc.nodes.contains_key(n) {
                return Err(ApiError::not_found(format!(
                    "There is no junction {n} in this scenario."
                )));
            }
            Ok(json!({ "node": n, "name": turns::junction_name(sc, n, None) }))
        },
    );
    r.add(
        "transport.turns.diagram",
        "A junction diagram as SVG text: the arms, the turning volumes and a north point.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("period", per()),
                ("node", string("Junction id.")),
                ("size", number("Image size in pixels (default 240, at least 120).")),
                ("layer", choice(&["total", "base", "dev"], "total, base (without development traffic) or dev (development traffic only).")),
                ("idSuffix", string("Text added to element ids so two diagrams can share a page.")),
            ],
            &["node"],
        ),
        turns_diagram,
    );
    r.add(
        "transport.turns.count_csv",
        "A classic turning-count sheet for one junction as CSV text: a row per approach with left, through, right and U-turn counts, totals and heavy vehicles.",
        false,
        params(vec![("scenario", sc()), ("period", per()), ("node", string("Junction id."))], &["node"]),
        turns_count_csv,
    );
    r.add(
        "transport.turns.sidra_csv",
        "The turning movements as CSV for SIDRA, one row per movement. Refused unless the runs are converged to 0.01% and current, unless forced.",
        false,
        params(
            vec![
                ("scenario", sc()),
                ("periods", array(json!({ "type": "string", "enum": ["AM", "PM"] }), "Periods to include (default every period that has a result).")),
                ("nodes", strings("Junction ids and/or site ids (default the extent-test sites, otherwise every junction).")),
                ("codes", boolean("Add the SIDRA leg, turn designation and movement ID columns.")),
                ("all", boolean("Every junction, even when the scenario's extent test lists sites.")),
                ("force", boolean("Make the CSV even when the runs are not ready.")),
            ],
            &[],
        ),
        turns_sidra_csv,
    );
}

fn turns_movements(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let res = held(s, i, &period)?;
    let id = p.str("id")?;
    let sc = &s.project.scenarios[i];
    let is_site = res.sites.as_ref().is_some_and(|m| m.contains_key(id));
    if !is_site && !sc.nodes.contains_key(id) {
        return Err(ApiError::not_found(format!(
            "There is no junction or site {id} in this scenario."
        )));
    }
    let m = if is_site {
        turns::site_rows(sc, Some(res), &s.project.settings, id)
    } else {
        turns::rows_for(sc, Some(res), &s.project.settings, id, None)
    };
    let name = if is_site {
        res.sites
            .as_ref()
            .and_then(|x| x.get(id))
            .map(|x| x.name.clone())
            .unwrap_or_default()
    } else {
        turns::junction_name(sc, id, None)
    };
    let empty = if m.rows.is_empty() {
        Some(turns::empty_text(Some(sc), Some(res), id))
    } else {
        None
    };
    Ok(json!({
        "scenario": sc.id, "period": period, "id": id, "site": is_site, "name": name, "empty": empty,
        "rows": m.rows.iter().map(|r| r.to_value()).collect::<Vec<_>>(),
        "arms": m.arms.iter().map(|a| a.to_value(true)).collect::<Vec<_>>(),
    }))
}

fn turns_diagram(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let node = p.str("node")?;
    let sid = s.sid(i);
    let sc = &s.project.scenarios[i];
    if !sc.nodes.contains_key(node) {
        return Err(ApiError::not_found(format!(
            "There is no junction {node} in this scenario."
        )));
    }
    let opts = DiagramOpts {
        size: p.opt_f64("size")?,
        layer: p.opt_str("layer")?.map(str::to_string),
        id_suffix: p.opt_str("idSuffix")?.map(str::to_string),
    };
    let svg = turns::diagram(
        Some(sc),
        s.result(&sid, &period),
        &s.project.settings,
        node,
        &opts,
    );
    Ok(json!({ "scenario": sid, "period": period, "node": node, "svg": svg }))
}

fn turns_count_csv(s: &mut Session, p: &Params) -> ApiResult {
    let (i, period) = pick(s, p)?;
    let node = p.str("node")?;
    let sid = s.sid(i);
    let sc = &s.project.scenarios[i];
    if !sc.nodes.contains_key(node) {
        return Err(ApiError::not_found(format!(
            "There is no junction {node} in this scenario."
        )));
    }
    let text = turns::turn_count_csv(Some(sc), s.result(&sid, &period), &s.project.settings, node);
    Ok(
        json!({ "scenario": sid, "period": period, "node": node, "withResults": s.result(&sid, &period).is_some(), "text": text }),
    )
}

fn turns_sidra_csv(s: &mut Session, p: &Params) -> ApiResult {
    let i = s.pick_scenario(p, "scenario")?;
    let sid = s.sid(i);
    let periods: Vec<String> = match p.opt_strings("periods")? {
        Some(v) => v,
        None => ["AM", "PM"]
            .iter()
            .filter(|x| s.result(&sid, x).is_some())
            .map(|x| x.to_string())
            .collect(),
    };
    let list: Vec<&Res> = periods.iter().filter_map(|x| s.result(&sid, x)).collect();
    let nodes = p.opt_strings("nodes")?;
    let opts = SidraOpts {
        codes: p.flag("codes")?,
        all: p.flag("all")?,
        force: p.flag("force")?,
    };
    let e = turns::sidra_export(
        Some(&s.project.scenarios[i]),
        &list,
        &s.project.settings,
        nodes.as_deref(),
        &opts,
    );
    Ok(
        json!({ "scenario": sid, "ok": e.ok, "why": e.why, "text": e.csv.map_or(Value::Null, Value::String) }),
    )
}
