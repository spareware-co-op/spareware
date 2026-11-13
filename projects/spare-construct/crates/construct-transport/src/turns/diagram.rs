//! The junction diagram: turns.js `Turns.diagram`, an SVG of the arms and the turning volumes.

use super::{Arm, MV, MovementRow, empty_text, junction_name, norm, rows_for};
use crate::jsmath::{cos, sin};
use crate::jsnum::{f64_to_usize, math_max, math_round, number_to_js_string, or_default, usize_to_f64};
use crate::project::{Scenario, Settings};
use crate::results::Res;
use crate::util::esc;
use std::collections::HashMap;
use std::fmt::Write as _;

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

// The image's geometry: half its size (the centre), the arm width, the font size and the text style.
struct Frame {
    half: f64,
    aw: f64,
    fs: f64,
    font: String,
}

impl Frame {
    // `pAt(r, u, nrm, d)`: the point r from the centre along u, then d along nrm
    fn p_at(&self, r: f64, u: [f64; 2], nrm: [f64; 2], d: f64) -> [f64; 2] {
        [self.half + u[0] * r + nrm[0] * d, self.half + u[1] * r + nrm[1] * d]
    }
}

// `vec(b)`: the screen direction of a bearing (y down)
fn unit(b: f64) -> [f64; 2] {
    [
        sin(b * std::f64::consts::PI / 180.0),
        -cos(b * std::f64::consts::PI / 180.0),
    ]
}

// `byKey(r.approach, x)`: the row's traffic arrives along the arm
fn arrives_on(row: &MovementRow, arm: &Arm) -> bool {
    arm.in_key == row.approach
}

// The lane offset of a movement on its approach, as a share of the arm width
fn lane(mv: &str) -> f64 {
    match mv {
        "L" => 0.38,
        "T" => 0.25,
        "R" => 0.13,
        _ => 0.05,
    }
}

// Ids unique per scenario, junction and period; everything outside [A-Za-z0-9_-] is dropped (res may come from a file)
fn element_id(sc: Option<&Scenario>, node_id: &str, period_name: Option<&str>, layer: &str, suffix: &str) -> String {
    let raw = format!(
        "tj{}-{}-{}{}{}",
        sc.map_or(String::new(), |scn| scn.id.clone()),
        node_id,
        period_name.unwrap_or(""),
        if layer == "total" {
            String::new()
        } else {
            format!("-{layer}")
        },
        suffix
    );
    raw.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect()
}

// The diagram's description: each approach's movements in words
fn describe(arms: &[Arm], rows: &[MovementRow]) -> String {
    arms.iter()
        .filter(|arm| arm.is_in)
        .map(|arm| {
            let rs: Vec<&MovementRow> = rows.iter().filter(|row| arrives_on(row, arm)).collect();
            format!(
                "{} approach ({}): {}",
                arm.name,
                arm.road,
                if rs.is_empty() {
                    "no traffic".to_string()
                } else {
                    rs.iter()
                        .map(|row| {
                            format!(
                                "{} {} to {}",
                                row.mv,
                                number_to_js_string(math_round(row.v)),
                                row.to_road
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            )
        })
        .collect::<Vec<_>>()
        .join(". ")
}

// The opening svg element with its title and description
fn svg_head(size: f64, uid: &str, title: &str, desc: &str) -> Vec<String> {
    let sz = number_to_js_string(size);
    vec![
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {sz} {sz}\" width=\"{sz}\" height=\"{sz}\" role=\"img\" aria-labelledby=\"{uid}-t {uid}-d\">"
        ),
        format!(
            "<title id=\"{uid}-t\">{}</title><desc id=\"{uid}-d\">{}</desc>",
            esc(title),
            esc(if desc.is_empty() {
                "No turning volumes."
            } else {
                desc
            })
        ),
    ]
}

// Road bands (one translucent group so overlaps read as a single surface), then centre lines on two-way arms
fn draw_bands(out: &mut Vec<String>, fr: &Frame, arms: &[Arm]) {
    let (half, aw) = (fr.half, fr.aw);
    out.push("<g style=\"stroke:var(--muted);opacity:.2\" stroke-linecap=\"butt\">".to_string());
    for arm in arms {
        let dir = unit(arm.b);
        let end = fr.p_at(half - 2.0, dir, [0.0, 0.0], 0.0);
        let sw = if arm.conn {
            aw * 0.45
        } else if arm.is_in && arm.is_out {
            aw
        } else {
            aw * 0.6
        };
        out.push(format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke-width=\"{}\"{}/>",
            f1(half),
            f1(half),
            f1(end[0]),
            f1(end[1]),
            f1(sw),
            if arm.conn {
                " stroke-dasharray=\"6 4\""
            } else {
                ""
            }
        ));
    }
    if !arms.is_empty() {
        out.push(format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" style=\"fill:var(--muted);stroke:none\"/>",
            f1(half),
            f1(half),
            f1(aw * 0.5)
        ));
    }
    out.push("</g>".to_string());
    for arm in arms {
        if arm.is_in && arm.is_out && !arm.conn {
            let dir = unit(arm.b);
            let from = fr.p_at(aw * 0.9, dir, [0.0, 0.0], 0.0);
            let to = fr.p_at(half - 2.0, dir, [0.0, 0.0], 0.0);
            out.push(format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" style=\"stroke:var(--muted)\" stroke-width=\"1\" stroke-dasharray=\"4 4\" opacity=\".6\"/>",
                f1(from[0]),
                f1(from[1]),
                f1(to[0]),
                f1(to[1])
            ));
        }
    }
}

// Road names along each arm, on the departure side, kept upright
fn draw_names(out: &mut Vec<String>, fr: &Frame, arms: &[Arm]) {
    let (half, aw, fs) = (fr.half, fr.aw, fr.fs);
    for arm in arms {
        let dir = unit(arm.b);
        let dep = [dir[1], -dir[0]];
        let pos = fr.p_at(half * 0.71, dir, dep, aw / 2.0 + fs * 0.45);
        let mut ang = arm.b - 90.0;
        if norm(ang) > 90.0 && norm(ang) < 270.0 {
            ang += 180.0;
        }
        let max = math_max(4.0, (half * 0.58 / (fs * 0.5)).floor());
        // max is a whole number of at least 4 here (a NaN max fails the test)
        let lbl = if usize_to_f64(utf16_len(&arm.road)) > max {
            format!("{}\u{2026}", utf16_slice(&arm.road, f64_to_usize(max).saturating_sub(1)))
        } else {
            arm.road.clone()
        };
        out.push(format!(
            "<text x=\"{}\" y=\"{}\" transform=\"rotate({} {} {})\" text-anchor=\"middle\" dominant-baseline=\"middle\" style=\"fill:var(--muted);{}\">{}</text>",
            f1(pos[0]),
            f1(pos[1]),
            f1(norm(ang)),
            f1(pos[0]),
            f1(pos[1]),
            fr.font,
            esc(&lbl)
        ));
    }
}

// Movement arrows: start on the arrival lane (left of inbound travel), end on the exit arm's departure lane
fn draw_arrows(out: &mut Vec<String>, fr: &Frame, arms: &[Arm], rows: &[MovementRow]) {
    let aw = fr.aw;
    let vmax = rows.iter().fold(1.0f64, |top, row| math_max(top, row.v));
    let r1 = fr.half * 0.42;
    let mut arm_of: HashMap<&str, &Arm> = HashMap::new();
    for arm in arms {
        arm_of.insert(arm.out_key.as_str(), arm);
    }
    for row in rows {
        if row.v < 0.5 {
            continue;
        }
        let Some(ar) = arms.iter().find(|arm| arrives_on(row, arm)) else {
            continue;
        };
        let Some(to) = arm_of.get(row.to.as_str()).copied() else {
            continue;
        };
        let ui = unit(ar.b);
        let uo = unit(to.b);
        let n_in = [-ui[1], ui[0]];
        let n_out = [uo[1], -uo[0]];
        // A one-way arm is a narrower band with no opposing half, so its lanes spread across the whole band
        let off = if ar.is_out {
            aw * lane(&row.mv)
        } else {
            aw * 0.6 * (lane(&row.mv) - 0.215)
        };
        let s0 = fr.p_at(r1, ui, n_in, off);
        let end = fr.p_at(
            r1,
            uo,
            n_out,
            if to.is_in { aw * 0.25 } else { aw * 0.6 * 0.035 },
        );
        let reach = r1 * if row.mv == "U" { 0.9 } else { 0.75 };
        let c1 = [s0[0] - ui[0] * reach, s0[1] - ui[1] * reach];
        let c2 = [end[0] - uo[0] * reach, end[1] - uo[1] * reach];
        let width = 1.2 + 5.3 * row.v / vmax;
        let hl = 3.0 + width * 1.1;
        let hw = 2.0 + width * 0.9;
        let tip = [end[0] + uo[0] * hl, end[1] + uo[1] * hl];
        out.push(format!(
            "<g style=\"stroke:var(--accent);fill:var(--accent)\"><path d=\"M{} {}C{} {} {} {} {} {}\" fill=\"none\" stroke-width=\"{}\" stroke-linecap=\"round\" opacity=\".85\"/><polygon points=\"{},{} {},{} {},{}\" stroke=\"none\"/></g>",
            f1(s0[0]),
            f1(s0[1]),
            f1(c1[0]),
            f1(c1[1]),
            f1(c2[0]),
            f1(c2[1]),
            f1(end[0]),
            f1(end[1]),
            f1(width),
            f1(tip[0]),
            f1(tip[1]),
            f1(end[0] + n_out[0] * hw),
            f1(end[1] + n_out[1] * hw),
            f1(end[0] - n_out[0] * hw),
            f1(end[1] - n_out[1] * hw)
        ));
    }
}

// Volumes beside each approach's arrival lane
fn draw_volumes(out: &mut Vec<String>, fr: &Frame, arms: &[Arm], rows: &[MovementRow]) {
    let (half, aw, fs) = (fr.half, fr.aw, fr.fs);
    for arm in arms {
        let rs: Vec<&MovementRow> = rows
            .iter()
            .filter(|row| arrives_on(row, arm) && row.v >= 0.5)
            .collect();
        if rs.is_empty() {
            continue;
        }
        let mut by: HashMap<&str, f64> = HashMap::new();
        for row in &rs {
            *by.entry(row.mv.as_str()).or_insert(0.0) += row.v;
        }
        let lines: Vec<String> = MV
            .iter()
            .filter_map(|mv| by.get(*mv).map(|vol| format!("{} {}", mv, number_to_js_string(math_round(*vol)))))
            .collect();
        let dir = unit(arm.b);
        let n_in = [-dir[1], dir[0]];
        let pos = fr.p_at(
            half * 0.6,
            dir,
            n_in,
            (if arm.is_out { aw / 2.0 } else { aw * 0.3 }) + fs * 0.4,
        );
        let anchor = if n_in[0] > 0.35 {
            "start"
        } else if n_in[0] < -0.35 {
            "end"
        } else {
            "middle"
        };
        let lh = fs * 1.15;
        let bh = usize_to_f64(lines.len()) * lh;
        let top = if n_in[1] > 0.35 {
            pos[1]
        } else if n_in[1] < -0.35 {
            pos[1] - bh
        } else {
            pos[1] - bh / 2.0
        };
        let mut spans = String::new();
        for (i, line) in lines.iter().enumerate() {
            // Writing to a String cannot fail
            let _ = write!(
                spans,
                "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                f1(pos[0]),
                f1(top + (usize_to_f64(i) + 0.8) * lh),
                line
            );
        }
        let font = &fr.font;
        out.push(format!(
            "<text text-anchor=\"{anchor}\" style=\"fill:var(--ink);{font}\">{spans}</text>"
        ));
    }
}

// The north point in the top right corner
fn draw_north(out: &mut Vec<String>, fr: &Frame, size: f64) {
    let fs = fr.fs;
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
        fr.font
    ));
}

/// `Turns.diagram(sc, res, nodeId, opts)`: the junction as SVG text: road bands, centre lines, road names, one arrow per movement
/// (width by volume) with the volumes by movement beside each approach, and a north point. Without movements the reason is written
/// on the diagram ([`empty_text`]).
#[must_use]
pub fn diagram(
    sc: Option<&Scenario>,
    res: Option<&Res>,
    s: &Settings,
    node_id: &str,
    opts: &DiagramOpts,
) -> String {
    let size = math_max(120.0, or_default(opts.size.unwrap_or(f64::NAN), 240.0));
    let half = size / 2.0;
    let aw = size * 0.17;
    let layer = match opts.layer.as_deref() {
        Some("dev") => "dev",
        Some("base") => "base",
        _ => "total",
    };
    let (r0_rows, arms): (Vec<MovementRow>, Vec<Arm>) = match sc {
        Some(sc) => {
            let mvs = rows_for(sc, res, s, node_id, None);
            (mvs.rows, mvs.arms)
        }
        None => (Vec::new(), Vec::new()),
    };
    let rows: Vec<MovementRow> = if layer == "total" {
        r0_rows
    } else {
        r0_rows
            .into_iter()
            .map(|mut row| {
                let pick = if layer == "dev" { row.dev } else { row.base };
                row.v = or_default(pick, 0.0);
                row
            })
            .collect()
    };
    let known = sc.is_some_and(|sc| sc.nodes.contains_key(node_id));
    let name = match sc {
        Some(sc) if known => junction_name(sc, node_id, None),
        _ => "Unknown junction".to_string(),
    };
    let period_name = res.map(|rs| rs.period.as_str()).filter(|pn| !pn.is_empty());
    let period = format!(
        "{}{}",
        period_name.map_or(String::new(), |pn| format!("{pn} peak")),
        match layer {
            "dev" => ", development traffic",
            "base" => ", without development traffic",
            _ => "",
        }
    );
    let uid = element_id(sc, node_id, period_name, layer, opts.id_suffix.as_deref().unwrap_or(""));
    let title = format!(
        "{}{} turning volumes (veh/h)",
        name,
        if period.is_empty() {
            String::new()
        } else {
            format!(" \u{b7} {period}")
        }
    );
    let desc = describe(&arms, &rows);
    let fs = math_max(8.0, size / 26.0);
    let fr = Frame {
        half,
        aw,
        fs,
        font: format!("font:600 {}px var(--f-cond, sans-serif)", f1(fs)),
    };
    let mut out = svg_head(size, &uid, &title, &desc);
    draw_bands(&mut out, &fr, &arms);
    draw_names(&mut out, &fr, &arms);
    draw_arrows(&mut out, &fr, &arms, &rows);
    draw_volumes(&mut out, &fr, &arms, &rows);
    draw_north(&mut out, &fr, size);
    if arms.is_empty() {
        out.push(format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"fill:var(--muted);{}\">No junction here</text>",
            f1(half),
            f1(half),
            fr.font
        ));
    } else if !rows.iter().any(|row| row.v >= 0.5) {
        out.push(format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"fill:var(--muted);{}\">{}</text>",
            f1(half),
            f1(size - fs),
            fr.font,
            esc(&empty_text(sc, res, node_id))
        ));
    }
    out.push("</svg>".to_string());
    out.join("")
}
