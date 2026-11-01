//! Turns: movement types, penalties and bans, and the turn table (engine.js "turns").
//!
//! Left-hand traffic: a positive turn is a right turn. U-turns are reversals onto the same
//! link and hairpins (over 150 degrees) at junctions; a numeric settings.turns.uturn allows
//! them everywhere at that penalty.

use super::{Arc, Graph};
use crate::jsnum::{JsNum, Maybe, Num, math_max, math_round, or_default};
use crate::project::{Node, Settings};
use crate::util::turn;
use engine_sim::{Network, Turns};

/// Default left-turn penalty (s), `TURN_DEF.left`.
pub const TURN_DEF_LEFT: f64 = 2.0;
/// Default right-turn penalty (s), `TURN_DEF.right`.
pub const TURN_DEF_RIGHT: f64 = 6.0;
/// Default right-turn penalty at signals (s), `TURN_DEF.rightSignals`.
pub const TURN_DEF_RIGHT_SIGNALS: f64 = 4.0;

/// Movement names by index (`MVC`): 1 L, 2 T, 3 R, 4 U.
pub const MVC: [&str; 5] = ["", "L", "T", "R", "U"];

/// Turn penalty settings (`turnSet(S)`): TURN_DEF with settings.turns over it, values as saved.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnSet {
    /// Left turn (s).
    pub left: Num,
    /// Right turn (s).
    pub right: Num,
    /// Right turn at signals (s).
    pub right_signals: Num,
    /// U-turns: "ban" or a penalty.
    pub uturn: Num,
}

/// `turnSet(S)`: `{ ...TURN_DEF, ...S.turns }`.
pub fn turn_set(s: &Settings) -> TurnSet {
    let t = s.turns.get();
    let pick = |v: Option<&Num>, d: Num| match v {
        None | Some(Maybe::Missing) => d,
        Some(x) => x.clone(),
    };
    TurnSet {
        left: pick(t.map(|t| &t.left), Num::number(TURN_DEF_LEFT)),
        right: pick(t.map(|t| &t.right), Num::number(TURN_DEF_RIGHT)),
        right_signals: pick(
            t.map(|t| &t.right_signals),
            Num::number(TURN_DEF_RIGHT_SIGNALS),
        ),
        uturn: pick(
            t.map(|t| &t.uturn),
            Maybe::Set(JsNum::Text("ban".to_string())),
        ),
    }
}

/// `uPen(T)`: the U-turn penalty (s) when U-turns are allowed, None when they are banned ("ban", "", null or not a number).
pub fn u_pen(t: &TurnSet) -> Option<f64> {
    match &t.uturn {
        Maybe::Set(JsNum::Text(s)) if s == "ban" || s.is_empty() => None,
        Maybe::Set(v) => {
            let n = v.to_number();
            if n.is_finite() {
                Some(math_max(0.0, n))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `turnType(inB, outB)`: movement from an arrival heading to a departure heading: "T" within 30 degrees, "U" over 150, else "R" or "L".
pub fn turn_type(in_b: f64, out_b: f64) -> &'static str {
    let t = turn(in_b, out_b);
    let m = t.abs();
    if m <= 30.0 {
        "T"
    } else if m > 150.0 {
        "U"
    } else if t > 0.0 {
        "R"
    } else {
        "L"
    }
}

/// The parts of the graph the turn rules read.
pub(crate) struct TurnCtx<'a> {
    pub arcs: &'a [Arc],
    pub deg: &'a [i32],
    pub nodes: &'a [Option<Node>],
    pub turn_s: &'a TurnSet,
}

impl TurnCtx<'_> {
    pub fn movement(&self, a: &Arc, b: &Arc) -> Option<&'static str> {
        if a.conn || b.conn {
            return None;
        }
        if a.link == b.link {
            return Some("U");
        }
        let m = turn_type(a.in_b, b.out_b);
        if m != "U" || self.deg[a.to] >= 3 {
            Some(m)
        } else if turn(a.in_b, b.out_b) > 0.0 {
            Some("R")
        } else {
            Some("L")
        }
    }

    pub fn penalty(&self, a: &Arc, b: &Arc) -> f64 {
        if a.conn || b.conn {
            return 0.0;
        }
        let t = self.turn_s;
        let deg = self.deg[a.to];
        let ctl = self.nodes[a.to]
            .as_ref()
            .map_or("none", |n| n.control.str_or("none"));
        let right = if ctl == "signals" {
            &t.right_signals
        } else {
            &t.right
        };
        let up = u_pen(t);
        // up is a number when set, so +up || 0 is up || 0
        let up_or_right = |up: Option<f64>| match up {
            Some(u) => or_default(u, 0.0),
            None => right.or(0.0),
        };
        if a.link == b.link {
            return if deg <= 1 {
                0.0
            } else {
                math_max(0.0, up_or_right(up))
            };
        }
        if deg < 3 || ctl == "roundabout" {
            return 0.0;
        }
        let mv = self.movement(a, b);
        match mv {
            Some("T") => 0.0,
            Some("L") => math_max(0.0, t.left.or(0.0)),
            Some("R") => math_max(0.0, right.or(0.0)),
            _ => math_max(0.0, up_or_right(up)),
        }
    }

    pub fn banned(&self, a: &Arc, b: &Arc) -> bool {
        if a.conn || b.conn {
            return false;
        }
        let n = a.to;
        let node = self.nodes[n].as_ref();
        if let Some(node) = node
            && let Maybe::Set(serde_json::Value::Array(bans)) = &node.bans
        {
            let key = format!("{}>{}", a.link, b.link);
            if bans.iter().any(|v| v.as_str() == Some(key.as_str())) {
                return true;
            }
        }
        if u_pen(self.turn_s).is_some()
            || self.deg[n] <= 1
            || node.is_some_and(|n| n.allow_u.is_true())
        {
            return false;
        }
        self.movement(a, b) == Some("U")
    }
}

/// `buildTurns(G)`: the turn table (penalty, or infinity when banned, for every arc pair) and each turn's movement.
pub(crate) fn build_turns(net: &Network, cx: &TurnCtx) -> (Turns, Vec<u8>) {
    let turns = Turns::new(net, |a, b| {
        let (aa, bb) = (&cx.arcs[a], &cx.arcs[b]);
        if cx.banned(aa, bb) {
            f64::INFINITY
        } else {
            cx.penalty(aa, bb)
        }
    });
    let mut tmv = vec![0u8; turns.len()];
    for a in 0..net.m {
        let aa = &cx.arcs[a];
        for t in turns.of(a) {
            let bb = &cx.arcs[turns.to[t] as usize];
            let m = if aa.conn || bb.conn {
                Some(turn_type(aa.in_b, bb.out_b))
            } else {
                cx.movement(aa, bb)
            };
            let m = m.unwrap_or("T");
            tmv[t] = MVC.iter().position(|&x| x == m).unwrap_or(0) as u8;
        }
    }
    (turns, tmv)
}

impl Graph {
    pub(crate) fn turn_ctx(&self) -> TurnCtx<'_> {
        TurnCtx {
            arcs: &self.arcs,
            deg: &self.deg,
            nodes: &self.nodes,
            turn_s: &self.turn_s,
        }
    }
    /// `movement(G, a, b)`: movement from arc a into arc b at a's end node ("L", "T", "R" or "U"); None when either is a connector.
    ///
    /// Reversing onto the same link is a U-turn, and so is a hairpin onto another link at a
    /// junction of 3 or more links; at a 2-link node a hairpin is a bend.
    pub fn movement(&self, a: usize, b: usize) -> Option<&'static str> {
        self.turn_ctx().movement(&self.arcs[a], &self.arcs[b])
    }
    /// `turnPenalty(G, a, b)`: seconds added going from arc a into arc b (bans are separate).
    pub fn turn_penalty(&self, a: usize, b: usize) -> f64 {
        self.turn_ctx().penalty(&self.arcs[a], &self.arcs[b])
    }
    /// `banned(G, a, b)`: an explicit ban, or a U-turn while U-turns are banned (not at dead ends or where node.allowU).
    pub fn banned(&self, a: usize, b: usize) -> bool {
        self.turn_ctx().banned(&self.arcs[a], &self.arcs[b])
    }
    /// `turnIdx(G, a, b)`: index of the turn from arc a into arc b, or -1.
    pub fn turn_idx(&self, a: usize, b: usize) -> i32 {
        self.search.turns.index(&self.search.net, a, b)
    }
    /// `turnBanned(G, a, b)`: true when there is no such turn or it is banned.
    pub fn turn_banned(&self, a: usize, b: usize) -> bool {
        self.search.turns.banned(&self.search.net, a, b)
    }
}

/// `approachName(inB, eight)`: compass side traffic arriving on heading inB comes from (heading north is the South approach).
pub fn approach_name(in_b: f64, eight: bool) -> &'static str {
    let s = ((or_default(in_b, 0.0) + 180.0) % 360.0 + 360.0) % 360.0;
    let n = if eight { 8.0 } else { 4.0 };
    let i = (math_round(s / (360.0 / n)) % n) as usize;
    if eight {
        ["North", "NE", "East", "SE", "South", "SW", "West", "NW"][i]
    } else {
        ["North", "East", "South", "West"][i]
    }
}
