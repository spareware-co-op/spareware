//! The Safe System Audit briefing pack (compliance-extra.js section 19, item 4).

use super::{pack_csv, r1, section, sv};
use crate::compliance::data::{m as model, x as xdata};
use crate::compliance::engine;
use crate::compliance::extra::ctx::{disclaimer, link_name, node_name, pair, posted, res_of};
use crate::compliance::extra::shared::{changed_links, network_diff};
use crate::compliance::jv::{Jx, NULL, V};
use crate::compliance::{net, record};

/// `tsdg`-style label of a control: `CONTROLS[control] || control`.
pub(super) fn control_label(control: &V) -> V {
    let controls = model("CONTROLS");
    let k = control.s();
    if controls.has(&k) && controls.g(&k).t() {
        controls.g(&k).clone()
    } else {
        control.clone()
    }
}

/// `ssaPack(P)`: the Safe System Audit briefing pack (CSV): layout changes, volumes by turn, legal and operating speeds, crash history and
/// conflict points for the roads and junctions the proposal changes.
pub fn ssa_pack(p: &V) -> String {
    let pr = pair(p);
    let sc = pr.mit.unwrap_or(pr.opt);
    let ch = changed_links(pr.base, sc);
    let mut nodes: Vec<String> = Vec::new();
    for id in &ch {
        let l = sc.g("links").g(id);
        for n in [l.g("a"), l.g("b")] {
            let nid = n.s();
            if net::road_links(sc, &nid).len() >= 3 && !nodes.contains(&nid) {
                nodes.push(nid);
            }
        }
    }
    let r = res_of(sc, "AM").or_else(|| res_of(sc, "PM"));
    let risk = engine::link_risk(p, pr.base);
    let peds_state = record::state(p);
    let peds = peds_state.g("peds");
    let cp = xdata("STD.conflicts");
    let conflicts = |id: &str| -> V {
        let n = sc.g("nodes").g(id);
        let legs = net::road_links(sc, id).len();
        let k = if n.g("control").eq_s("roundabout") {
            if legs >= 4 {
                "roundabout4"
            } else {
                "roundabout3"
            }
        } else if legs >= 4 {
            "x4"
        } else {
            "x3"
        };
        cp.g(k).clone()
    };
    let diff = network_diff(pr.base, sc);
    let mut layout: Vec<Vec<V>> = diff
        .items()
        .iter()
        .map(|d| vec![d.g("text").clone()])
        .collect();
    for id in ch.iter().filter(|id| !pr.base.g("links").g(id).t()) {
        layout.push(vec![V::String(format!(
            "New road {}",
            link_name(sc.g("links").g(id))
        ))]);
    }
    let volumes: Vec<Vec<V>> = match r {
        Some(r) => nodes
            .iter()
            .flat_map(|id| {
                let turns = if r.g("turns").t() && r.g("turns").g(id).t() {
                    r.g("turns").g(id)
                } else {
                    &NULL
                };
                turns
                    .items()
                    .iter()
                    .map(|t| {
                        let ped = peds.g(id);
                        vec![
                            V::String(node_name(sc, id)),
                            r.g("period").clone(),
                            t.g("from").clone(),
                            t.g("to").clone(),
                            t.g("mv").clone(),
                            r1(t.g("v")),
                            if ped.nullish() { sv("") } else { ped.clone() },
                        ]
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
        None => vec![],
    };
    let speeds: Vec<Vec<V>> = ch
        .iter()
        .map(|id| {
            let l = sc.g("links").g(id);
            let x = r.map_or(&NULL, |r| r.g("arcs").g(&format!("{id}:ab")));
            let lim = |k: &str| {
                if l.g("cx").t() && l.g("cx").g(k).t() {
                    l.g("cx").g(k).clone()
                } else {
                    sv("")
                }
            };
            vec![
                V::String(link_name(l)),
                V::Number(posted(l)),
                lim("limit"),
                lim("meanSpeed"),
                if x.t() { r1(x.g("spd")) } else { sv("") },
            ]
        })
        .collect();
    let crash_rows: Vec<Vec<V>> = match &risk {
        Some(risk) => ch
            .iter()
            .filter(|id| risk.g(id).t())
            .map(|id| {
                let x = risk.g(id);
                vec![
                    V::String(link_name(sc.g("links").g(id))),
                    x.g("crashes").clone(),
                    x.g("F").clone(),
                    x.g("S").clone(),
                    x.g("M").clone(),
                    x.g("N").clone(),
                    x.g("dsi").clone(),
                ]
            })
            .collect(),
        None => vec![],
    };
    let conflict_rows: Vec<Vec<V>> = nodes
        .iter()
        .map(|id| {
            let n = sc.g("nodes").g(id);
            vec![
                V::String(node_name(sc, id)),
                control_label(n.g("control")),
                V::Number(net::road_links(sc, id).len() as f64),
                conflicts(id),
            ]
        })
        .collect();
    pack_csv(
        p,
        "Safe System Audit briefing pack",
        "a Safe System Audit",
        &[
            section("Layout changes", Some(xdata("STD.labels.ssa").s()), &["Change"], layout, Some("No layout changes against the do-minimum.")),
            section(
                "Volumes by mode and turn",
                Some("Vehicles from the model (veh/h); pedestrians from field counts entered in the Compliance tab; cycle and bus volumes are not modelled.".to_string()),
                &["Junction", "Period", "From", "To", "Movement", "Vehicles (veh/h)", "Pedestrians (people/h)"],
                volumes,
                Some("Run the model."),
            ),
            section(
                "Operating and legal speeds",
                Some(xdata("STD.labels.meanSpeed").s()),
                &["Road", "Legal limit (km/h)", "Proposed limit (km/h)", "Measured mean speed (km/h)", "Modelled speed (km/h, placeholder)"],
                speeds,
                None,
            ),
            section(
                "CAS crash history (changed roads)",
                Some(disclaimer("D8", p, None)),
                &["Road", "Crashes", "Fatal", "Serious", "Minor", "Non-injury", "DSI"],
                crash_rows,
                Some(if risk.is_some() { "No crashes on the changed roads." } else { "Load NZTA CAS crashes." }),
            ),
            section(
                "Conflict points",
                Some(format!("{} ({} counts: crossroads 32, T 9, roundabout 8 or 6)", xdata("STD.conflicts.edition").s(), xdata("ASSUMPTION").s())),
                &["Junction", "Control", "Legs", "Vehicle conflict points"],
                conflict_rows,
                None,
            ),
        ],
    )
}
