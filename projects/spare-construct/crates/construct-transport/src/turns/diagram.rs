//! The junction diagram: turns.js `Turns.diagram`, an SVG of the arms and the turning volumes.

use super::{Arm, MV, MovementRow, empty_text, junction_name, norm, rows_for};
use crate::jsmath::{cos, sin};
use crate::jsnum::{math_max, math_round, number_to_js_string, or_default};
use crate::project::{Scenario, Settings};
use crate::results::Res;
use crate::util::esc;
use std::collections::HashMap;

/// What `Turns.diagram(sc, res, nodeId, opts)` is given.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagramOpts {
    /// Size of the square image in pixels (default 240, at least 120).
    pub size: Option<f64>,
    /// "total" (default), "base" (without development traffic) or "dev" (development traffic only), so Base + Development = Total
    /// diagrams can be drawn side by side.
    pub layer: Option<String>,
    /// Text added to the element ids (so two diagrams of one junction can share a page).
    pub id_suffix: Option<String>,
}

fn f1(x: f64) -> String {
    number_to_js_string(math_round(x * 10.0) / 10.0)
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn utf16_slice(s: &str, end: usize) -> String {
    let u: Vec<u16> = s.encode_utf16().take(end).collect();
    String::from_utf16_lossy(&u)
}

/// `Turns.diagram(sc, res, nodeId, opts)`: the junction as SVG text: road bands, centre lines, road names, one arrow per movement
/// (width by volume) with the volumes by movement beside each approach, and a north point. Without movements the reason is written
/// on the diagram ([`empty_text`]).
pub fn diagram(
    sc: Option<&Scenario>,
    res: Option<&Res>,
    s: &Settings,
    node_id: &str,
    opts: &DiagramOpts,
) -> String {
    let size = math_max(120.0, or_default(opts.size.unwrap_or(f64::NAN), 240.0));
    let h = size / 2.0;
    let aw = size * 0.17;
    let layer = match opts.layer.as_deref() {
        Some("dev") => "dev",
        Some("base") => "base",
        _ => "total",
    };
    let (r0_rows, a): (Vec<MovementRow>, Vec<Arm>) = match sc {
        Some(sc) => {
            let m = rows_for(sc, res, s, node_id, None);
            (m.rows, m.arms)
        }
        None => (Vec::new(), Vec::new()),
    };
    let rows: Vec<MovementRow> = if layer == "total" {
        r0_rows
    } else {
        r0_rows
            .into_iter()
            .map(|mut r| {
                let x = if layer == "dev" { r.dev } else { r.base };
                r.v = or_default(x, 0.0);
                r
            })
            .collect()
    };
    let known = sc.is_some_and(|sc| sc.nodes.contains_key(node_id));
    let name = match sc {
        Some(sc) if known => junction_name(sc, node_id, None),
        _ => "Unknown junction".to_string(),
    };
    let period_name = res.map(|r| r.period.as_str()).filter(|p| !p.is_empty());
    let period = format!(
        "{}{}",
        period_name.map_or(String::new(), |p| format!("{p} peak")),
        match layer {
            "dev" => ", development traffic",
            "base" => ", without development traffic",
            _ => "",
        }
    );
    // Ids unique per scenario, junction and period; everything outside [A-Za-z0-9_-] is dropped (res may come from a file)
    let raw = format!(
        "tj{}-{}-{}{}{}",
        sc.map_or(String::new(), |s| s.id.clone()),
        node_id,
        period_name.unwrap_or(""),
        if layer == "total" {
            String::new()
        } else {
            format!("-{layer}")
        },
        opts.id_suffix.as_deref().unwrap_or("")
    );
    let uid: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let title = format!(
        "{}{} turning volumes (veh/h)",
        name,
        if period.is_empty() {
            String::new()
        } else {
            format!(" \u{b7} {period}")
        }
    );
    let by_key = |k: &str, x: &Arm| x.in_key == k;
    let desc: String = a
        .iter()
        .filter(|x| x.is_in)
        .map(|x| {
            let rs: Vec<&MovementRow> = rows.iter().filter(|r| by_key(&r.approach, x)).collect();
            format!(
                "{} approach ({}): {}",
                x.name,
                x.road,
                if rs.is_empty() {
                    "no traffic".to_string()
                } else {
                    rs.iter()
                        .map(|r| {
                            format!(
                                "{} {} to {}",
                                r.mv,
                                number_to_js_string(math_round(r.v)),
                                r.to_road
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            )
        })
        .collect::<Vec<_>>()
        .join(". ");
    let fs = math_max(8.0, size / 26.0);
    let font = format!("font:600 {}px var(--f-cond, sans-serif)", f1(fs));
    let p_at = |r: f64, u: [f64; 2], nrm: [f64; 2], d: f64| -> [f64; 2] {
        [h + u[0] * r + nrm[0] * d, h + u[1] * r + nrm[1] * d]
    };
    let vec = |b: f64| -> [f64; 2] {
        [
            sin(b * std::f64::consts::PI / 180.0),
            -cos(b * std::f64::consts::PI / 180.0),
        ]
    };
    let sz = number_to_js_string(size);
    let mut out: Vec<String> = vec![
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {sz} {sz}\" width=\"{sz}\" height=\"{sz}\" role=\"img\" aria-labelledby=\"{uid}-t {uid}-d\">"
        ),
        format!(
            "<title id=\"{uid}-t\">{}</title><desc id=\"{uid}-d\">{}</desc>",
            esc(&title),
            esc(if desc.is_empty() {
                "No turning volumes."
            } else {
                &desc
            })
        ),
    ];
    // Road bands: one translucent group so overlaps read as a single surface
    out.push("<g style=\"stroke:var(--muted);opacity:.2\" stroke-linecap=\"butt\">".to_string());
    for x in &a {
        let u = vec(x.b);
        let e = p_at(h - 2.0, u, [0.0, 0.0], 0.0);
        let sw = if x.conn {
            aw * 0.45
        } else if x.is_in && x.is_out {
            aw
        } else {
            aw * 0.6
        };
        out.push(format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke-width=\"{}\"{}/>",
            f1(h),
            f1(h),
            f1(e[0]),
            f1(e[1]),
            f1(sw),
            if x.conn {
                " stroke-dasharray=\"6 4\""
            } else {
                ""
            }
        ));
    }
    if !a.is_empty() {
        out.push(format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" style=\"fill:var(--muted);stroke:none\"/>",
            f1(h),
            f1(h),
            f1(aw * 0.5)
        ));
    }
    out.push("</g>".to_string());
    // Centre lines on two-way arms
    for x in &a {
        if x.is_in && x.is_out && !x.conn {
            let u = vec(x.b);
            let p = p_at(aw * 0.9, u, [0.0, 0.0], 0.0);
            let q = p_at(h - 2.0, u, [0.0, 0.0], 0.0);
            out.push(format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" style=\"stroke:var(--muted)\" stroke-width=\"1\" stroke-dasharray=\"4 4\" opacity=\".6\"/>",
                f1(p[0]),
                f1(p[1]),
                f1(q[0]),
                f1(q[1])
            ));
        }
    }
    // Road names along each arm, on the departure side, kept upright
    for x in &a {
        let u = vec(x.b);
        let dep = [u[1], -u[0]];
        let p = p_at(h * 0.71, u, dep, aw / 2.0 + fs * 0.45);
        let mut ang = x.b - 90.0;
        if norm(ang) > 90.0 && norm(ang) < 270.0 {
            ang += 180.0;
        }
        let max = math_max(4.0, (h * 0.58 / (fs * 0.5)).floor());
        let lbl = if utf16_len(&x.road) as f64 > max {
            format!("{}\u{2026}", utf16_slice(&x.road, max as usize - 1))
        } else {
            x.road.clone()
        };
        out.push(format!(
            "<text x=\"{}\" y=\"{}\" transform=\"rotate({} {} {})\" text-anchor=\"middle\" dominant-baseline=\"middle\" style=\"fill:var(--muted);{}\">{}</text>",
            f1(p[0]),
            f1(p[1]),
            f1(norm(ang)),
            f1(p[0]),
            f1(p[1]),
            font,
            esc(&lbl)
        ));
    }
    // Movement arrows: start on the arrival lane (left of inbound travel), end on the exit arm's departure lane
    let vmax = rows.iter().fold(1.0f64, |m, r| math_max(m, r.v));
    let r1 = h * 0.42;
    let lane = |mv: &str| -> f64 {
        match mv {
            "L" => 0.38,
            "T" => 0.25,
            "R" => 0.13,
            _ => 0.05,
        }
    };
    let mut arm_of: HashMap<&str, &Arm> = HashMap::new();
    for x in &a {
        arm_of.insert(x.out_key.as_str(), x);
    }
    for r in &rows {
        if r.v < 0.5 {
            continue;
        }
        let Some(ar) = a.iter().find(|x| by_key(&r.approach, x)) else {
            continue;
        };
        let Some(x) = arm_of.get(r.to.as_str()).copied() else {
            continue;
        };
        let ui = vec(ar.b);
        let uo = vec(x.b);
        let n_in = [-ui[1], ui[0]];
        let n_out = [uo[1], -uo[0]];
        // A one-way arm is a narrower band with no opposing half, so its lanes spread across the whole band
        let off = if ar.is_out {
            aw * lane(&r.mv)
        } else {
            aw * 0.6 * (lane(&r.mv) - 0.215)
        };
        let s0 = p_at(r1, ui, n_in, off);
        let e = p_at(
            r1,
            uo,
            n_out,
            if x.is_in { aw * 0.25 } else { aw * 0.6 * 0.035 },
        );
        let k = r1 * if r.mv == "U" { 0.9 } else { 0.75 };
        let c1 = [s0[0] - ui[0] * k, s0[1] - ui[1] * k];
        let c2 = [e[0] - uo[0] * k, e[1] - uo[1] * k];
        let w = 1.2 + 5.3 * r.v / vmax;
        let hl = 3.0 + w * 1.1;
        let hw = 2.0 + w * 0.9;
        let tip = [e[0] + uo[0] * hl, e[1] + uo[1] * hl];
        out.push(format!(
            "<g style=\"stroke:var(--accent);fill:var(--accent)\"><path d=\"M{} {}C{} {} {} {} {} {}\" fill=\"none\" stroke-width=\"{}\" stroke-linecap=\"round\" opacity=\".85\"/><polygon points=\"{},{} {},{} {},{}\" stroke=\"none\"/></g>",
            f1(s0[0]),
            f1(s0[1]),
            f1(c1[0]),
            f1(c1[1]),
            f1(c2[0]),
            f1(c2[1]),
            f1(e[0]),
            f1(e[1]),
            f1(w),
            f1(tip[0]),
            f1(tip[1]),
            f1(e[0] + n_out[0] * hw),
            f1(e[1] + n_out[1] * hw),
            f1(e[0] - n_out[0] * hw),
            f1(e[1] - n_out[1] * hw)
        ));
    }
    // Volumes beside each approach's arrival lane
    for x in &a {
        let rs: Vec<&MovementRow> = rows
            .iter()
            .filter(|r| by_key(&r.approach, x) && r.v >= 0.5)
            .collect();
        if rs.is_empty() {
            continue;
        }
        let mut by: HashMap<&str, f64> = HashMap::new();
        for r in &rs {
            *by.entry(r.mv.as_str()).or_insert(0.0) += r.v;
        }
        let lines: Vec<String> = MV
            .iter()
            .filter(|m| by.contains_key(**m))
            .map(|m| format!("{} {}", m, number_to_js_string(math_round(by[*m]))))
            .collect();
        let u = vec(x.b);
        let n_in = [-u[1], u[0]];
        let p = p_at(
            h * 0.6,
            u,
            n_in,
            (if x.is_out { aw / 2.0 } else { aw * 0.3 }) + fs * 0.4,
        );
        let anchor = if n_in[0] > 0.35 {
            "start"
        } else if n_in[0] < -0.35 {
            "end"
        } else {
            "middle"
        };
        let lh = fs * 1.15;
        let bh = lines.len() as f64 * lh;
        let top = if n_in[1] > 0.35 {
            p[1]
        } else if n_in[1] < -0.35 {
            p[1] - bh
        } else {
            p[1] - bh / 2.0
        };
        let spans: String = lines
            .iter()
            .enumerate()
            .map(|(i, t)| {
                format!(
                    "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                    f1(p[0]),
                    f1(top + (i as f64 + 0.8) * lh),
                    t
                )
            })
            .collect();
        out.push(format!(
            "<text text-anchor=\"{anchor}\" style=\"fill:var(--ink);{font}\">{spans}</text>"
        ));
    }
    // North point
    let nx = size - fs * 1.2;
    let ny = fs * 2.6;
    out.push(format!(
        "<g style=\"fill:var(--muted)\"><polygon points=\"{},{} {},{} {},{}\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"{}\">N</text></g>",
        f1(nx),
        f1(ny - fs * 1.4),
        f1(nx - fs * 0.45),
        f1(ny - fs * 0.3),
        f1(nx + fs * 0.45),
        f1(ny - fs * 0.3),
        f1(nx),
        f1(ny + fs * 0.7),
        font
    ));
    if a.is_empty() {
        out.push(format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"fill:var(--muted);{}\">No junction here</text>",
            f1(h),
            f1(h),
            font
        ));
    } else if !rows.iter().any(|r| r.v >= 0.5) {
        out.push(format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"fill:var(--muted);{}\">{}</text>",
            f1(h),
            f1(size - fs),
            font,
            esc(&empty_text(sc, res, node_id))
        ));
    }
    out.push("</svg>".to_string());
    out.join("")
}
