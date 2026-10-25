//! Project commands: new, open, save, info, name, period, settings and the reference tables.

use super::Session;
use super::helpers::{from_jv, kit_get_path, kit_set_path, to_jv};
use super::registry::Registry;
use crate::error::{ApiError, ApiResult};
use crate::schema::*;
use construct_transport::edit::json::JsonValue;
use construct_transport::edit::{self, migrate};
use construct_transport::jsnum::{Maybe, math_round};
use construct_transport::project::{CLASSES, LANDUSES, Settings, default_settings};
use construct_transport::util::clamp;
use serde_json::{Value, json};

pub(super) fn register(r: &mut Registry) {
    r.add(
        "transport.project.new",
        "Start a new project with one empty Base case scenario and the default settings.",
        true,
        params(
            vec![("name", string("Project name (default Untitled project)."))],
            &[],
        ),
        project_new,
    );
    r.add(
        "transport.project.open",
        "Open a project from its JSON text (a .corridor.json file); older projects are upgraded as the app upgrades them.",
        true,
        params(vec![("text", string("The project file's JSON text."))], &["text"]),
        project_open,
    );
    r.add(
        "transport.project.save",
        "Give the project as JSON text, as Save writes it (results without their paths), with the file name Save suggests.",
        false,
        none(),
        project_save,
    );
    r.add(
        "transport.project.info",
        "Describe the open project: name, scenarios with their results, the current period and the undo steps.",
        false,
        none(),
        |s, _| Ok(s.project_info()),
    );
    r.add(
        "transport.project.rename",
        "Rename the project.",
        true,
        params(
            vec![(
                "name",
                string("The new name (blank gives Untitled project)."),
            )],
            &["name"],
        ),
        project_rename,
    );
    r.add(
        "transport.project.set_period",
        "Choose the peak hour that results and queries use by default (AM or PM).",
        true,
        params(
            vec![("period", choice(&["AM", "PM"], "The peak hour."))],
            &["period"],
        ),
        project_set_period,
    );
    r.add(
        "transport.settings.get",
        "Read the project settings, or one setting by its dotted path (for example econ.vot).",
        false,
        params(
            vec![(
                "path",
                string("Dotted path of one setting; leave out for all of them."),
            )],
            &[],
        ),
        settings_get,
    );
    r.add(
        "transport.settings.set",
        "Change project settings, each given as a dotted path and value, as the Settings dialog's Save does; results go out of date and Undo restores the old settings.",
        true,
        params(
            vec![("values", free_object("Settings to change: { \"growth\": 1.5, \"econ.vot\": 17.3, \"turns.uturn\": \"ban\" }."))],
            &["values"],
        ),
        settings_set,
    );
    r.add(
        "transport.settings.reset",
        "Reset every model setting to the defaults (keeping the period, basemap and display choices); results go out of date.",
        true,
        none(),
        settings_reset,
    );
    r.add(
        "transport.settings.adopt_defaults",
        "Adopt the new defaults the project's notice lists (all of them, or the named keys); an empty list keeps the project's own settings.",
        true,
        params(vec![("keys", strings("Keys of the changed settings to adopt; leave out to adopt all."))], &[]),
        settings_adopt,
    );
    r.add(
        "transport.settings.keep",
        "Keep the project's own settings and clear the notice about changed defaults.",
        true,
        none(),
        settings_keep,
    );
    r.add(
        "transport.reference.tables",
        "List the fixed tables the model uses: road classes, land uses, junction controls, statuses, scenario roles and the like.",
        false,
        none(),
        |_, _| Ok(reference_tables()),
    );
}

fn project_new(s: &mut Session, p: &Params) -> ApiResult {
    let name = p.opt_str("name")?.unwrap_or("Untitled project").to_string();
    let view = s.project.view.clone();
    let theme = s.project.extra.get("theme").cloned();
    let basemap = s.project.settings.basemap.clone();
    let mut np = edit::new_project(&mut s.ids, &name);
    // the app keeps the map view, theme and basemap of the project it replaces
    np.view = view;
    if let Some(t) = theme {
        np.extra.insert("theme".to_string(), t);
    }
    np.settings.basemap = basemap;
    let bad = s.set_project(np);
    let mut out = s.project_info();
    out["warnings"] = json!(bad);
    Ok(out)
}

fn project_open(s: &mut Session, p: &Params) -> ApiResult {
    let text = p.str("text")?;
    let np = migrate::migrate_text(text, &mut s.ids)?;
    let bad = s.set_project(np);
    let mut out = s.project_info();
    out["warnings"] = json!(bad);
    Ok(out)
}

/// `fileName()` of ui.js: the project's name as a safe file name.
fn file_name(name: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .collect();
    let base = construct_transport::jsnum::js_trim(&kept)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase();
    format!(
        "{}.corridor.json",
        if base.is_empty() {
            "project".to_string()
        } else {
            base
        }
    )
}

fn project_save(s: &mut Session, _: &Params) -> ApiResult {
    let text = s.project.serialise();
    let name = s
        .project
        .name
        .as_str()
        .filter(|n| !n.is_empty())
        .unwrap_or("project");
    Ok(json!({ "fileName": file_name(name), "bytes": text.len(), "text": text }))
}

fn project_rename(s: &mut Session, p: &Params) -> ApiResult {
    let name = p.str("name")?.trim();
    let name = if name.is_empty() {
        "Untitled project"
    } else {
        name
    };
    s.project.name = Maybe::Set(name.to_string());
    s.touch();
    Ok(json!({ "name": name }))
}

fn project_set_period(s: &mut Session, p: &Params) -> ApiResult {
    let period = p.str("period")?;
    s.project.settings.period = Maybe::Set(period.to_string());
    s.touch();
    Ok(json!({ "period": period }))
}

fn settings_json(s: &Session) -> ApiResult<JsonValue> {
    to_jv(&s.project.settings)
}

fn settings_get(s: &mut Session, p: &Params) -> ApiResult {
    let jv = settings_json(s)?;
    match p.opt_str("path")? {
        None => Ok(jv.to_value()),
        Some(path) => kit_get_path(&jv, path)
            .map(JsonValue::to_value)
            .ok_or_else(|| ApiError::not_found(format!("There is no setting {path}."))),
    }
}

/// The rules the Settings dialog applies to what it reads (`readSettings` of ui-dialogs.js).
fn tidy_settings(jv: &mut JsonValue) {
    if let Some(c) = jv.get("connectors") {
        let x = clamp(math_round(c.to_number()), 1.0, 4.0);
        if !x.is_nan() {
            jv.set("connectors", JsonValue::Number(x));
        }
    }
    if let Some(JsonValue::Object(_)) = jv.get("turns")
        && let Some(t) = jv.get_mut("turns")
        && let Some(u) = t.get("uturn")
        && !matches!(u, JsonValue::String(x) if x == "ban")
    {
        let n = u.to_number();
        if n.is_finite() {
            t.set("uturn", JsonValue::Number(n));
        }
    }
    // discount segments: a blank segment is dropped
    if let Some(econ) = jv.get_mut("econ")
        && let Some(JsonValue::Array(rates)) = econ.get_mut("rates")
    {
        let kept: Vec<JsonValue> = rates
            .iter()
            .filter(|x| {
                x.truthy()
                    && x.get("r").is_some_and(|r| {
                        !matches!(r, JsonValue::Null)
                            && !matches!(r, JsonValue::String(t) if t.is_empty())
                            && r.to_number().is_finite()
                    })
            })
            .map(|x| {
                let mut o = x.clone();
                let r = x.get("r").map_or(f64::NAN, JsonValue::to_number);
                o.set("r", JsonValue::Number(r));
                let to = match x.get("to") {
                    None | Some(JsonValue::Null) => JsonValue::Null,
                    Some(JsonValue::String(t)) if t.is_empty() => JsonValue::Null,
                    Some(t) if !t.to_number().is_finite() => JsonValue::Null,
                    Some(t) => JsonValue::Number(t.to_number()),
                };
                o.set("to", to);
                o
            })
            .collect();
        *rates = kept;
    }
}

fn settings_set(s: &mut Session, p: &Params) -> ApiResult {
    let values = p.object("values")?;
    if values.is_empty() {
        return Err(ApiError::bad_params("`values` has no settings in it"));
    }
    let mut jv = settings_json(s)?;
    let before = jv.stringify();
    for (k, v) in values {
        if !kit_set_path(&mut jv, k, JsonValue::from_value(v)) {
            return Err(ApiError::bad_params(format!(
                "The setting {k} cannot be set: a step of its path holds a plain value."
            )));
        }
    }
    tidy_settings(&mut jv);
    if jv.stringify() == before {
        return Ok(json!({ "changed": false, "message": "No model settings changed." }));
    }
    let new: Settings = from_jv(&jv)?;
    s.snap_settings("settings");
    s.project.settings = new;
    s.stale_all();
    Ok(json!({ "changed": true, "message": "Settings saved. Run the model again to apply them." }))
}

fn settings_reset(s: &mut Session, _: &Params) -> ApiResult {
    let cur = settings_json(s)?;
    let mut d = to_jv(&default_settings())?;
    for k in ["period", "basemap", "linzKey", "display"] {
        match cur.get(k) {
            Some(v) => d.set(k, v.clone()),
            None => {
                d.remove(k);
            }
        }
    }
    let new: Settings = from_jv(&d)?;
    s.snap_settings("resetting settings to the defaults");
    s.project.settings = new;
    s.project.extra.remove("settingsNotice");
    s.stale_all();
    Ok(json!({ "message": "Settings reset to the defaults. Run the model again to apply them." }))
}

fn settings_adopt(s: &mut Session, p: &Params) -> ApiResult {
    if migrate::settings_notice(&s.project).is_none() {
        return Ok(
            json!({ "adopted": 0, "message": "There is no notice about changed defaults." }),
        );
    }
    let keys = p.opt_strings("keys")?;
    if keys.as_ref().is_some_and(Vec::is_empty) {
        migrate::keep_settings(&mut s.project);
        s.touch();
        return Ok(json!({ "adopted": 0, "message": "Kept this project's own settings." }));
    }
    s.snap_settings("adopting the new defaults");
    let n = migrate::adopt_defaults(&mut s.project, keys.as_deref())?;
    if n > 0 {
        s.stale_all();
    }
    s.touch();
    Ok(json!({
        "adopted": n,
        "message": if n > 0 { "Adopted the new defaults. Results are out of date: run the model again." } else { "Kept this project's own settings." }
    }))
}

fn settings_keep(s: &mut Session, _: &Params) -> ApiResult {
    let kept = migrate::keep_settings(&mut s.project);
    if kept {
        s.touch();
    }
    Ok(json!({ "kept": kept, "message": "Kept this project's own settings." }))
}

fn pairs(t: &[(&str, &str)]) -> Value {
    Value::Array(
        t.iter()
            .map(|(k, v)| json!({ "key": k, "label": v }))
            .collect(),
    )
}

fn reference_tables() -> Value {
    json!({
        "roadClasses": CLASSES.iter().map(|c| json!({
            "key": c.key, "label": c.label, "rank": c.rank, "speed": c.speed, "lanes": c.lanes, "capPerLane": c.cap, "laneWidth": c.lw
        })).collect::<Vec<_>>(),
        "landUses": LANDUSES.iter().map(|l| json!({
            "key": l.key, "label": l.label, "unit": l.unit, "per": l.per, "group": l.group
        })).collect::<Vec<_>>(),
        "controls": pairs(&edit::CONTROLS),
        "linkStatus": pairs(&edit::STATUS),
        "cycling": pairs(&edit::CYCLE),
        "speeds": edit::SPEEDS,
        "scenarioRoles": edit::SCENARIO_ROLES.iter().map(|r| json!({
            "key": r.key, "label": r.label, "short": r.short, "code": r.code, "side": r.side
        })).collect::<Vec<_>>(),
        "zoneRoles": pairs(&edit::ZONE_ROLES),
        "linkStages": pairs(&edit::LINK_STAGES),
        "funding": pairs(&edit::FUNDING),
        "pathways": pairs(&edit::PATHWAYS),
        "registerTypes": pairs(&edit::REGISTER_TYPES),
        "routeColours": edit::ROUTE_COLOURS,
    })
}
