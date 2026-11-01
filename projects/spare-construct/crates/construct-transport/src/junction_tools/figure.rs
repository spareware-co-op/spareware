//! The turning diagrams: the report figure with 'AM (PM)' labels rounded to 5 veh/h, the turn ban editor's diagram (the
//! calculations behind it: which turns exist, which are banned) and the lane editor's approach data.

use super::text::r5;
use super::text::{ctl_name, per_name};
use super::{
    Entry, FIGURE_STEP, Leg, TURN_ORDER, Tools, entry_for, legs_of, moves_of, node_legs, norm,
};
use crate::edit::sites;
use crate::graph::{js_sort_by, turn_set, turn_type, u_pen};
use crate::jsmath::{cos, sin};
use crate::jsnum::{math_max, math_round, number_to_js_string, value_truthy};
use crate::junction::{LaneModel, lane_model};
use crate::project::Scenario;
use crate::results::Res;
use crate::turns::{Arm, LEG_CODES, movement_id, movements, turn_code};
use crate::util::{self, esc};
use serde_json::{Map, Value, json};

const PI: f64 = std::f64::consts::PI;

/// `vec(b)`: the unit vector of a bearing in screen coordinates.
fn vec(b: f64) -> [f64; 2] {
    [sin(b * PI / 180.0), -cos(b * PI / 180.0)]
}

/// `f1(x)`: x rounded to 1 decimal.
fn f1(x: f64) -> f64 {
    math_round(x * 10.0) / 10.0
}

fn s1(x: f64) -> String {
    number_to_js_string(f1(x))
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.slice(0, n)` by UTF-16 code units.
fn slice16(s: &str, n: usize) -> String {
    let u: Vec<u16> = s.encode_utf16().take(n).collect();
    String::from_utf16_lossy(&u)
}

struct Arrow {
    s0: [f64; 2],
    e: [f64; 2],
    c1: [f64; 2],
    c2: [f64; 2],
    uo: [f64; 2],
    n_out: [f64; 2],
}

fn lane_of(mv: &str) -> f64 {
    match mv {
        "L" => 0.38,
        "T" => 0.25,
        "R" => 0.13,
        "U" => 0.05,
        _ => 0.25,
    }
}

/// `arrow(h, R1, aw, bIn, bOut, mv, inTwo, outTwo)`: the curve of one movement.
fn arrow(
    h: f64,
    r1: f64,
    aw: f64,
    b_in: f64,
    b_out: f64,
    mv: &str,
    in_two: bool,
    out_two: bool,
) -> Arrow {
    let ui = vec(b_in);
    let uo = vec(b_out);
    let n_in = [-ui[1], ui[0]];
    let n_out = [uo[1], -uo[0]];
    let lane = lane_of(mv);
    let p = |r: f64, u: [f64; 2], n: [f64; 2], d: f64| {
        [h + u[0] * r + n[0] * d, h + u[1] * r + n[1] * d]
    };
    let s0 = p(
        r1,
        ui,
        n_in,
        if in_two {
            aw * lane
        } else {
            aw * 0.6 * (lane - 0.215)
        },
    );
    let e = p(r1, uo, n_out, if out_two { aw * 0.25 } else { aw * 0.02 });
    let k = r1 * if mv == "U" { 0.9 } else { 0.75 };
    Arrow {
        s0,
        e,
        c1: [s0[0] - ui[0] * k, s0[1] - ui[1] * k],
        c2: [e[0] - uo[0] * k, e[1] - uo[1] * k],
        uo,
        n_out,
    }
}

fn path_d(a: &Arrow) -> String {
    format!(
        "M{} {}C{} {} {} {} {} {}",
        s1(a.s0[0]),
        s1(a.s0[1]),
        s1(a.c1[0]),
        s1(a.c1[1]),
        s1(a.c2[0]),
        s1(a.c2[1]),
        s1(a.e[0]),
        s1(a.e[1])
    )
}

fn arrow_head(a: &Arrow, w: f64) -> String {
    let hl = 3.0 + w * 1.1;
    let hw = 2.0 + w * 0.9;
    let t = [a.e[0] + a.uo[0] * hl, a.e[1] + a.uo[1] * hl];
    format!(
        "{},{} {},{} {},{}",
        s1(t[0]),
        s1(t[1]),
        s1(a.e[0] + a.n_out[0] * hw),
        s1(a.e[1] + a.n_out[1] * hw),
        s1(a.e[0] - a.n_out[0] * hw),
        s1(a.e[1] - a.n_out[1] * hw)
    )
}

/// The north arrow both diagrams end with.
fn north(size: f64, fs: f64, font: &str, muted: &str) -> String {
    let nx = size - fs * 1.2;
    let ny = fs * 2.6;
    format!(
        "<g style=\"fill:{muted}\"><polygon points=\"{},{} {},{} {},{}\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"{}\">N</text></g></svg>",
        s1(nx),
        s1(ny - fs * 1.4),
        s1(nx - fs * 0.45),
        s1(ny - fs * 0.3),
        s1(nx + fs * 0.45),
        s1(ny - fs * 0.3),
        s1(nx),
        s1(ny + fs * 0.7),
        font
    )
}

/// Options of [`Tools::figure`].
#[derive(Clone, Copy, Debug, Default)]
pub struct FigureOpts<'a> {
    /// Diagram size in pixels (300 when unset or 0).
    pub size: Option<f64>,
    /// "dev" or "base" for the development or the without-development traffic; anything else is the total.
    pub layer: Option<&'a str>,
}

struct Pick<'a> {
    e: Entry<'a>,
    rows: Vec<(crate::turns::MovementRow, f64)>,
    legs: Vec<Leg>,
}

struct Mv {
    r: crate::turns::MovementRow,
    a: f64,
    b: f64,
}

impl<'a> Tools<'a> {
    /// `figure(sc, resA, resB, id, opts)`: the junction's turning diagram for reports: peak-hour volumes by movement, 'AM (PM)'
    /// labels rounded to 5 veh/h, and its note.
    pub fn figure(
        &self,
        sc: &Scenario,
        res_a: Option<&'a Res>,
        res_b: Option<&'a Res>,
        id: &str,
        opts: FigureOpts,
    ) -> String {
        let size = match opts.size {
            Some(s) if s != 0.0 && !s.is_nan() => s,
            _ => 300.0,
        };
        let h = size / 2.0;
        let aw = size * 0.15;
        let r1 = h * 0.42;
        let fs = math_max(9.0, size / 27.0);
        let font = format!("font:600 {}px var(--f-cond, sans-serif)", s1(fs));
        let layer = match opts.layer {
            Some("dev") => "dev",
            Some("base") => "base",
            _ => "total",
        };
        let st = self.st();
        let pick = |res: Option<&'a Res>| -> Option<Pick<'a>> {
            let res = res?;
            let e = entry_for(sc, res, id)?;
            let rows = moves_of(sc, st, res, &e)
                .into_iter()
                .map(|r| {
                    let v = match layer {
                        "total" => r.v,
                        "dev" => or0(r.dev),
                        _ => or0(r.base),
                    };
                    (r, v)
                })
                .collect();
            let legs = legs_of(sc, st, res, &e);
            Some(Pick { e, rows, legs })
        };
        let pa = pick(res_a);
        let pb = pick(res_b);
        let Some(base) = pa.as_ref().or(pb.as_ref()) else {
            return "<p class=\"note\">No turning volumes at this junction. Run the model first.</p>".to_string();
        };
        // legs by code, the first arm of each code
        let mut legs: Vec<(usize, &Arm)> = Vec::new();
        for l in &base.legs {
            if !legs.iter().any(|(o, _)| *o == l.arm.o) {
                legs.push((l.arm.o, &l.arm));
            }
        }
        let leg_of = |o: usize| legs.iter().find(|(k, _)| *k == o).map(|(_, a)| *a);
        let mut mv: Vec<Mv> = Vec::new();
        for (side, p) in [(0usize, &pa), (1usize, &pb)] {
            if let Some(p) = p {
                for (r, v) in &p.rows {
                    let i = match mv.iter().position(|m| m.r.id == r.id) {
                        Some(i) => i,
                        None => {
                            mv.push(Mv {
                                r: r.clone(),
                                a: 0.0,
                                b: 0.0,
                            });
                            mv.len() - 1
                        }
                    };
                    if side == 0 {
                        mv[i].a += v;
                    } else {
                        mv[i].b += v;
                    }
                }
            }
        }
        let name = base.e.name.clone();
        let two = |o: usize| leg_of(o).is_none_or(|a| a.is_in && a.is_out);
        let label = format!(
            "{}: turning volumes {}{}, veh/h",
            name,
            res_a.map_or("", |r| r.period.as_str()),
            res_b.map_or(String::new(), |r| format!(" ({})", r.period))
        );
        let mut s: Vec<String> = vec![
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {size} {size}\" width=\"{size}\" height=\"{size}\" role=\"img\" aria-label=\"{}\">",
                esc(&label),
                size = number_to_js_string(size)
            ),
            "<g style=\"stroke:var(--muted, #56635f);opacity:.2\">".to_string(),
        ];
        for (o, a) in &legs {
            let u = vec(a.b);
            s.push(format!(
                "<line x1=\"{h}\" y1=\"{h}\" x2=\"{}\" y2=\"{}\" stroke-width=\"{}\"/>",
                s1(h + u[0] * (h - 2.0)),
                s1(h + u[1] * (h - 2.0)),
                s1(if two(*o) { aw } else { aw * 0.6 }),
                h = number_to_js_string(h)
            ));
        }
        s.push(format!(
            "<circle cx=\"{h}\" cy=\"{h}\" r=\"{}\" style=\"fill:var(--muted, #56635f)\"/></g>",
            s1(aw * 0.5),
            h = number_to_js_string(h)
        ));
        let vmax = mv.iter().fold(1.0, |m, x| math_max(m, math_max(x.a, x.b)));
        for m in &mv {
            let ai = LEG_CODES.iter().position(|c| *c == m.r.leg);
            let xi = LEG_CODES.iter().position(|c| *c == m.r.to_leg);
            let (Some(a), Some(x)) = (ai.and_then(leg_of), xi.and_then(leg_of)) else {
                continue;
            };
            if math_max(m.a, m.b) < 0.5 {
                continue;
            }
            let g = arrow(h, r1, aw, a.b, x.b, &m.r.mv, two(a.o), two(x.o));
            let w = 1.2 + 4.5 * math_max(m.a, m.b) / vmax;
            s.push(format!(
                "<g style=\"stroke:var(--accent, #0d6a4d);fill:var(--accent, #0d6a4d)\"><path d=\"{}\" fill=\"none\" stroke-width=\"{}\" stroke-linecap=\"round\" opacity=\".85\"/><polygon points=\"{}\" stroke=\"none\"/></g>",
                path_d(&g),
                s1(w),
                arrow_head(&g, w)
            ));
        }
        for (_, a) in &legs {
            let u = vec(a.b);
            let dep = [u[1], -u[0]];
            let p = [
                h + u[0] * h * 0.78 + dep[0] * (aw / 2.0 + fs * 0.5),
                h + u[1] * h * 0.78 + dep[1] * (aw / 2.0 + fs * 0.5),
            ];
            let mut ang = a.b - 90.0;
            if norm(ang) > 90.0 && norm(ang) < 270.0 {
                ang += 180.0;
            }
            let lbl = if utf16_len(&a.road) > 22 {
                format!("{}\u{2026}", slice16(&a.road, 21))
            } else {
                a.road.clone()
            };
            s.push(format!(
                "<text x=\"{}\" y=\"{}\" transform=\"rotate({} {} {})\" text-anchor=\"middle\" dominant-baseline=\"middle\" style=\"fill:var(--muted, #56635f);{}\">{}</text>",
                s1(p[0]),
                s1(p[1]),
                s1(norm(ang)),
                s1(p[0]),
                s1(p[1]),
                font,
                esc(&lbl)
            ));
            let mut sel: Vec<&Mv> = mv
                .iter()
                .filter(|m| m.r.leg == a.leg && math_max(m.a, m.b) >= 0.5)
                .collect();
            let ix = |t: &str| {
                TURN_ORDER
                    .iter()
                    .position(|x| *x == t)
                    .map_or(-1.0, |i| i as f64)
            };
            js_sort_by(&mut sel, |p2, q| ix(p2.r.turn) - ix(q.r.turn));
            let lines: Vec<String> = sel
                .iter()
                .map(|m| {
                    format!(
                        "{} {}{}",
                        m.r.turn,
                        if pa.is_some() { r5(m.a) } else { String::new() },
                        if pb.is_some() {
                            if pa.is_some() {
                                format!(" ({})", r5(m.b))
                            } else {
                                r5(m.b)
                            }
                        } else {
                            String::new()
                        }
                    )
                })
                .collect();
            if lines.is_empty() {
                continue;
            }
            let n_in = [-u[1], u[0]];
            let q = [
                h + u[0] * h * 0.56 + n_in[0] * (aw / 2.0 + fs * 0.4),
                h + u[1] * h * 0.56 + n_in[1] * (aw / 2.0 + fs * 0.4),
            ];
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
                q[1]
            } else if n_in[1] < -0.35 {
                q[1] - bh
            } else {
                q[1] - bh / 2.0
            };
            s.push(format!(
                "<text text-anchor=\"{}\" style=\"fill:var(--ink, #18201e);{}\">{}</text>",
                anchor,
                font,
                lines
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!(
                        "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                        s1(q[0]),
                        s1(top + (i as f64 + 0.8) * lh),
                        esc(t)
                    ))
                    .collect::<String>()
            ));
        }
        s.push(north(size, fs, &font, "var(--muted, #56635f)"));
        let lab = format!(
            "{}{}",
            if pa.is_some() {
                res_a.map_or("", |r| r.period.as_str())
            } else {
                ""
            },
            if pb.is_some() {
                match res_b {
                    Some(r) if pa.is_some() => format!(" ({})", r.period),
                    Some(r) => r.period.clone(),
                    None => String::new(),
                }
            } else {
                String::new()
            }
        );
        format!(
            "{}<p class=\"note\">{}: {} peak-hour turning volumes{}, veh/h, rounded to the nearest {}.</p>",
            s.join(""),
            esc(&name),
            esc(&lab),
            match layer {
                "dev" => ", development traffic only",
                "base" => ", without development traffic",
                _ => "",
            },
            number_to_js_string(FIGURE_STEP)
        )
    }
}

fn or0(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 { 0.0 } else { x }
}

/* ---------------- turn bans ---------------- */

/// One turn a junction allows or bans (`moves`).
#[derive(Clone, Debug)]
pub struct MoveRow {
    /// "<from link id>><to link id>": the key the node's ban list holds.
    pub key: String,
    /// The arm traffic comes in along.
    pub from: Arm,
    /// The arm it leaves along.
    pub to: Arm,
    /// Movement: L, T, R or U.
    pub mv: &'static str,
    /// Turn designation.
    pub turn: &'static str,
    /// Movement id.
    pub id: String,
    /// Banned, by the list or by the default U-turn rule.
    pub banned: bool,
    /// Banned in the node's list.
    pub explicit: bool,
    /// Banned by the default U-turn rule.
    pub rule: bool,
    /// Flow (veh/h).
    pub v: f64,
    /// Heavy vehicles (veh/h).
    pub hv: f64,
    /// Arc key arriving.
    pub in_key: String,
    /// Arc key leaving.
    pub out_key: String,
}

impl MoveRow {
    /// As JSON (the arms by id).
    pub fn to_value(&self) -> Value {
        use crate::jsnum::plain_number;
        json!({
            "key": self.key, "from": self.from.id, "to": self.to.id, "mv": self.mv, "turn": self.turn, "id": self.id,
            "banned": self.banned, "explicit": self.explicit, "rule": self.rule, "v": plain_number(self.v), "hv": plain_number(self.hv),
            "inKey": self.in_key, "outKey": self.out_key,
        })
    }
}

impl<'a> Tools<'a> {
    /// `moves(sc, res, nodeId, set)`: the turns a junction allows (every entry arm to every exit arm), with their flows and
    /// whether each is banned (by the node's list, or by the default no-U-turn rule). Empty below three roads.
    pub fn moves(&self, sc: &Scenario, res: Option<&Res>, node_id: &str) -> Vec<MoveRow> {
        let Some(n) = sc.nodes.get(node_id) else {
            return Vec::new();
        };
        let legs = node_legs(sc, res, node_id);
        let a: Vec<&Arm> = legs.iter().map(|l| &l.arm).filter(|a| !a.conn).collect();
        let mut ids: Vec<&str> = Vec::new();
        for x in &a {
            if !ids.contains(&x.id.as_str()) {
                ids.push(&x.id);
            }
        }
        let deg = ids.len();
        if deg < 3 {
            return Vec::new();
        }
        let u_ok = u_pen(&turn_set(self.st())).is_some();
        let bans: Vec<String> = match n.bans.get() {
            Some(Value::Array(l)) => l
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let flows = movements(sc, res, self.st(), node_id);
        let mut out: Vec<MoveRow> = Vec::new();
        for ar in &a {
            if !ar.is_in {
                continue;
            }
            for x in &a {
                if !x.is_out {
                    continue;
                }
                let mut mv = if ar.id == x.id {
                    "U"
                } else {
                    turn_type(norm(ar.b + 180.0), x.b)
                };
                if mv == "U" && ar.id != x.id && deg < 3 {
                    mv = if util::turn(norm(ar.b + 180.0), x.b) > 0.0 {
                        "R"
                    } else {
                        "L"
                    };
                }
                let key = format!("{}>{}", ar.id, x.id);
                let explicit = bans.contains(&key);
                let rule = !explicit && mv == "U" && !u_ok && !n.allow_u.is_true();
                let f = flows
                    .iter()
                    .find(|r| r.approach == ar.in_key && r.to == x.out_key);
                out.push(MoveRow {
                    key,
                    from: (*ar).clone(),
                    to: (*x).clone(),
                    mv,
                    turn: turn_code(ar.o, x.o),
                    id: movement_id(ar.o, x.o),
                    banned: explicit || rule,
                    explicit,
                    rule,
                    v: f.map_or(0.0, |f| f.v),
                    hv: f.map_or(0.0, |f| f.hv),
                    in_key: ar.in_key.clone(),
                    out_key: x.out_key.clone(),
                });
            }
        }
        let ix = |t: &str| {
            TURN_ORDER
                .iter()
                .position(|x| *x == t)
                .map_or(-1.0, |i| i as f64)
        };
        js_sort_by(&mut out, |p, q| {
            let d = p.from.o as f64 - q.from.o as f64;
            if d != 0.0 { d } else { ix(p.turn) - ix(q.turn) }
        });
        out
    }

    /// `banSvg(sc, nodeId, list, size)`: the turn ban editor's diagram: an arrow for each turn, dashed and crossed when banned
    /// (U-turns banned by the default rule are not drawn).
    pub fn ban_svg(&self, node_id: &str, list: &[MoveRow], size: Option<f64>) -> String {
        let size = match size {
            Some(s) if s != 0.0 && !s.is_nan() => s,
            _ => 240.0,
        };
        let h = size / 2.0;
        let aw = size * 0.17;
        let r1 = h * 0.42;
        let fs = math_max(8.0, size / 26.0);
        let font = format!("font:600 {}px var(--f-cond, sans-serif)", s1(fs));
        let mut arms: Vec<&Arm> = Vec::new();
        for m in list {
            for x in [&m.from, &m.to] {
                match arms.iter().position(|a| a.id == x.id) {
                    Some(i) => arms[i] = x,
                    None => arms.push(x),
                }
            }
        }
        let mut s: Vec<String> = vec![format!(
            "<svg viewBox=\"0 0 {size} {size}\" width=\"{size}\" height=\"{size}\" role=\"group\" aria-label=\"Turns at this junction. Click an arrow to ban or allow that turn.\"><g style=\"stroke:var(--muted);opacity:.2\">",
            size = number_to_js_string(size)
        )];
        for a in &arms {
            let u = vec(a.b);
            s.push(format!(
                "<line x1=\"{h}\" y1=\"{h}\" x2=\"{}\" y2=\"{}\" stroke-width=\"{}\"/>",
                s1(h + u[0] * (h - 2.0)),
                s1(h + u[1] * (h - 2.0)),
                s1(if a.is_in && a.is_out { aw } else { aw * 0.6 }),
                h = number_to_js_string(h)
            ));
        }
        s.push(format!(
            "<circle cx=\"{h}\" cy=\"{h}\" r=\"{}\" style=\"fill:var(--muted)\"/></g>",
            s1(aw * 0.5),
            h = number_to_js_string(h)
        ));
        for a in &arms {
            let u = vec(a.b);
            let dep = [u[1], -u[0]];
            let p = [
                h + u[0] * h * 0.74 + dep[0] * (aw / 2.0 + fs * 0.45),
                h + u[1] * h * 0.74 + dep[1] * (aw / 2.0 + fs * 0.45),
            ];
            let mut ang = a.b - 90.0;
            if norm(ang) > 90.0 && norm(ang) < 270.0 {
                ang += 180.0;
            }
            let road = if utf16_len(&a.road) > 16 {
                format!("{}\u{2026}", slice16(&a.road, 15))
            } else {
                a.road.clone()
            };
            s.push(format!(
                "<text x=\"{}\" y=\"{}\" transform=\"rotate({} {} {})\" text-anchor=\"middle\" dominant-baseline=\"middle\" style=\"fill:var(--muted);{}\">{}</text>",
                s1(p[0]),
                s1(p[1]),
                s1(norm(ang)),
                s1(p[0]),
                s1(p[1]),
                font,
                esc(&format!("{} {}", a.leg, road))
            ));
        }
        for m in list {
            if m.rule {
                continue; // U-turns banned by the default rule are not drawn; the switch beside the diagram allows them
            }
            let g = arrow(
                h,
                r1,
                aw,
                m.from.b,
                m.to.b,
                m.mv,
                m.from.is_in && m.from.is_out,
                m.to.is_in && m.to.is_out,
            );
            let col = if m.banned {
                "var(--danger)"
            } else {
                "var(--accent)"
            };
            let w = 2.2;
            let tip = format!(
                "{}: {} to {} \u{b7} {}",
                m.id,
                m.from.leg_label,
                m.to.leg_label,
                if m.banned {
                    "banned. Click to allow"
                } else {
                    "allowed. Click to ban"
                }
            );
            let mut e = format!(
                "<g data-jban=\"{}\" data-node=\"{}\" style=\"cursor:pointer;stroke:{};fill:{}\"><title>{}</title>",
                esc(&m.key),
                esc(node_id),
                col,
                col,
                esc(&tip)
            );
            e.push_str(&format!(
                "<path d=\"{}\" fill=\"none\" stroke=\"transparent\" stroke-width=\"12\" style=\"pointer-events:stroke\"/>",
                path_d(&g)
            ));
            e.push_str(&format!(
                "<path d=\"{}\" fill=\"none\" stroke-width=\"{}\"{} stroke-linecap=\"round\"/><polygon points=\"{}\" stroke=\"none\"{}/>",
                path_d(&g),
                number_to_js_string(w),
                if m.banned { " stroke-dasharray=\"4 3\" opacity=\".7\"" } else { " opacity=\".9\"" },
                arrow_head(&g, w),
                if m.banned { " opacity=\".7\"" } else { "" }
            ));
            if m.banned {
                e.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" dominant-baseline=\"middle\" style=\"fill:var(--danger);stroke:none;{}\">\u{2715}</text>",
                    s1((g.s0[0] + g.e[0] * 2.0) / 3.0),
                    s1((g.s0[1] + g.e[1] * 2.0) / 3.0),
                    font
                ));
            }
            e.push_str("</g>");
            s.push(e);
        }
        s.push(north(size, fs, &font, "var(--muted)"));
        s.join("")
    }
}

/* ---------------- lanes ---------------- */

/// The source of each lane attribute of an approach: where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneSources {
    /// Lane uses.
    pub lanes: String,
    /// Bay lengths.
    pub len: String,
    /// Slip lane.
    pub slip: String,
    /// Flush median.
    pub median: String,
}

/// One approach link into a node, for the lane editor: the lanes on the road, the stored lane model and where it is stored.
#[derive(Clone, Debug)]
pub struct ApproachInfo {
    /// Link id.
    pub link: String,
    /// The approach's leg label.
    pub leg_label: String,
    /// General traffic lanes on the road.
    pub gl: f64,
    /// The stored lane model, if any.
    pub raw: Option<Value>,
    /// The lane model used.
    pub model: LaneModel,
    /// Stored on the junction site.
    pub on_site: bool,
    /// The lanes the editor shows: (movements, bay length).
    pub edit_lanes: Vec<(String, f64)>,
    /// Where each attribute came from.
    pub src: LaneSources,
}

impl ApproachInfo {
    /// As JSON.
    pub fn to_value(&self) -> Value {
        use crate::jsnum::plain_number;
        let mut m = Map::new();
        m.insert("link".into(), json!(self.link));
        m.insert("leg".into(), json!(self.leg_label));
        m.insert("gl".into(), plain_number(self.gl));
        if let Some(r) = &self.raw {
            m.insert("raw".into(), r.clone());
        } else {
            m.insert("raw".into(), Value::Null);
        }
        m.insert("model".into(), self.model.to_value());
        m.insert("onSite".into(), json!(self.on_site));
        m.insert(
            "editLanes".into(),
            Value::Array(
                self.edit_lanes
                    .iter()
                    .map(|(d, l)| json!({ "d": d, "len": plain_number(*l) }))
                    .collect(),
            ),
        );
        m.insert(
            "src".into(),
            json!({ "lanes": self.src.lanes, "len": self.src.len, "slip": self.src.slip, "median": self.src.median }),
        );
        Value::Object(m)
    }
}

fn truthy_text(v: Option<&Value>) -> Option<String> {
    match v {
        Some(x) if value_truthy(x) => Some(crate::jsnum::value_to_js_string(x)),
        _ => None,
    }
}

/// `approaches(sc, nodeId)`: the approach links into a node, with lanes on the road, the stored lane model and where it is stored.
pub fn node_approaches(sc: &Scenario, node_id: &str) -> Vec<ApproachInfo> {
    let Some(n) = sc.nodes.get(node_id) else {
        return Vec::new();
    };
    let site = sites::of(sc, node_id);
    let mut out = Vec::new();
    for leg in node_legs(sc, None, node_id) {
        let a = &leg.arm;
        if a.conn || !a.is_in {
            continue;
        }
        let Some(link_id) = &a.link else {
            continue;
        };
        let Some(l) = sc.links.get(link_id) else {
            continue;
        };
        let lanes = if l.a == node_id {
            l.lanes_ba.int32()
        } else {
            l.lanes_ab.int32()
        } as f64;
        let gl = math_max(
            1.0,
            if l.bus_lane.truthy() && lanes >= 2.0 {
                lanes - 1.0
            } else {
                lanes
            },
        );
        let site_raw = site.and_then(|s| s.lanes.get()).and_then(|m| m.get(&l.id));
        let on_site = site_raw.is_some_and(value_truthy);
        let raw: Option<Value> = if on_site {
            site_raw.cloned()
        } else {
            n.lanes
                .get()
                .and_then(|m| m.get(&l.id))
                .filter(|v| value_truthy(v))
                .cloned()
        };
        let model = lane_model(raw.as_ref(), gl);
        let field = |k: &str| raw.as_ref().and_then(|r| r.get(k));
        let src = match &raw {
            Some(_) => truthy_text(field("src")).unwrap_or_else(|| "user".to_string()),
            None => "default".to_string(),
        };
        let by = field("srcBy").filter(|v| value_truthy(v));
        let src_of = |k: &str| -> String {
            if let Some(t) = truthy_text(by.and_then(|b| b.get(k))) {
                return t;
            }
            if raw.is_none() {
                return "default".to_string();
            }
            if k == "lanes" {
                return src.clone();
            }
            if src != "user" {
                return if field(k).is_some_and(|v| !v.is_null()) {
                    src.clone()
                } else {
                    "default".to_string()
                };
            }
            if by.is_none() && field(k).is_some_and(value_truthy) {
                "user".to_string()
            } else {
                "default".to_string()
            }
        };
        let edit_lanes: Vec<(String, f64)> = match field("lanes") {
            Some(Value::Array(ls)) if !ls.is_empty() => ls
                .iter()
                .map(|x| {
                    let d = truthy_text(x.get("d"))
                        .or_else(|| truthy_text(x.get("mv")))
                        .unwrap_or_default();
                    let len = crate::jsnum::or_default(
                        x.get("len").map_or(f64::NAN, crate::jsnum::value_to_number),
                        0.0,
                    );
                    (d.to_uppercase().replace('+', ""), len)
                })
                .collect(),
            _ => {
                let names: Vec<String> = if gl == 1.0 {
                    vec!["LTR".to_string()]
                } else {
                    let mut v = vec!["LT".to_string()];
                    for _ in 0..(gl as usize).saturating_sub(2) {
                        v.push("T".to_string());
                    }
                    v.push("TR".to_string());
                    v
                };
                names.into_iter().map(|d| (d, 0.0)).collect()
            }
        };
        out.push(ApproachInfo {
            link: l.id.clone(),
            leg_label: a.leg_label.clone(),
            gl,
            raw: raw.clone(),
            model,
            on_site,
            edit_lanes,
            src: LaneSources {
                lanes: src_of("lanes"),
                len: src_of("len"),
                slip: src_of("slip"),
                median: src_of("median"),
            },
        });
    }
    out
}

/// `ctlName` re-exported for the figure's callers.
#[allow(dead_code)]
pub(crate) fn _names() -> (fn(&str) -> String, fn(&str) -> String) {
    (ctl_name, per_name)
}
