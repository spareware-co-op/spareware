//! Matching counts to model links (`DataHub.match`, `DataHub.matches`) and the road-name normaliser (`DataHub.norm`).

use super::{Count, Data, dir_bearing, or_text};
use crate::jsnum::{is_js_space, math_max, math_min};
use crate::project::{OrderedMap, Scenario, Settings};
use crate::util::{LatLng, ang_diff, bearing, flat, seg_dist};
use std::rc::Rc;

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

// s.replace(/\(.*?\)/g, ' '): a bracketed piece without a line break in it becomes a space
fn strip_brackets(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '(' {
            let mut j = i + 1;
            let mut found = None;
            while j < cs.len() {
                match cs[j] {
                    ')' => {
                        found = Some(j);
                        break;
                    }
                    '\n' | '\r' | '\u{2028}' | '\u{2029}' => break,
                    _ => j += 1,
                }
            }
            if let Some(j) = found {
                out.push(' ');
                i = j + 1;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

// s.replace(/\bword\b/g, repl)
fn replace_word(s: &str, word: &str, repl: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let w: Vec<char> = word.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        let end = i + w.len();
        if end <= cs.len()
            && cs[i..end] == w[..]
            && (i == 0 || !is_word(cs[i - 1]))
            && (end == cs.len() || !is_word(cs[end]))
        {
            out.push_str(repl);
            i = end;
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

// s.replace(/\bsh\s*(\d+)/g, 'sh$1')
fn replace_sh(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == 's' && i + 1 < cs.len() && cs[i + 1] == 'h' && (i == 0 || !is_word(cs[i - 1])) {
            let mut j = i + 2;
            while j < cs.len() && is_js_space(cs[j]) {
                j += 1;
            }
            let d0 = j;
            while j < cs.len() && cs[j].is_ascii_digit() {
                j += 1;
            }
            if j > d0 {
                out.push_str("sh");
                out.extend(&cs[d0..j]);
                i = j;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// `DataHub.norm(s)`: a road name for comparison: lower case, bracketed text removed, the usual abbreviations written out (rd, st, ave, dr, pl, cres, hwy, pde, tce, ln, mwy, "sh 16"), anything but letters and digits a space, spaces collapsed.
pub fn norm(s: &str) -> String {
    let mut t = strip_brackets(&s.to_lowercase());
    for (w, r) in [("rd", "road"), ("st", "street")] {
        t = replace_word(&t, w, r);
    }
    // \baves?\b: "aves" first (the one that can match is the one a word boundary follows)
    t = replace_word(&t, "aves", "avenue");
    t = replace_word(&t, "ave", "avenue");
    for (w, r) in [
        ("dr", "drive"),
        ("pl", "place"),
        ("cres", "crescent"),
        ("hwy", "highway"),
        ("pde", "parade"),
        ("tce", "terrace"),
        ("ln", "lane"),
        ("mwy", "motorway"),
    ] {
        t = replace_word(&t, w, r);
    }
    t = replace_sh(&t);
    // [^a-z0-9 ] to a space (one per UTF-16 unit), then \s+ to one space, then trim
    let mut out = String::with_capacity(t.len());
    let mut last_space = true;
    for c in t.chars() {
        let keep = c.is_ascii_lowercase() || c.is_ascii_digit();
        if keep {
            out.push(c);
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// `norm(x || y)` for loose values: the first that is truthy, as text, normalised.
pub fn norm_of(a: &crate::jsnum::Num, b: &crate::jsnum::Num) -> String {
    norm(&or_text(a, b))
}

/// Where a count was matched: the link, the direction it counts, how far it is from the road (m) and the score that chose it.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// Link id.
    pub link: String,
    /// "ab", "ba" or "both".
    pub dir: &'static str,
    /// Distance from the count to the link (m).
    pub d: f64,
    /// Distance plus the penalties for a class or name mismatch (smaller wins).
    pub score: f64,
    /// The link's name matches the count's road.
    pub named: bool,
}

/// Counts matched to links by count id, in the order the counts were matched.
pub type Matches = OrderedMap<Match>;

/// `Net.project(sc, L, ll).d`: distance (m) from a point to the nearest point on a polyline of [lat, lng].
pub fn project_dist(g: &[LatLng], ll: &LatLng) -> f64 {
    let f = flat(ll[0]);
    let px = f.x(ll);
    let py = f.y(ll);
    let mut best = f64::INFINITY;
    for i in 0..g.len().saturating_sub(1) {
        let r = seg_dist(px, py, f.x(&g[i]), f.y(&g[i]), f.x(&g[i + 1]), f.y(&g[i + 1]));
        if r.d < best {
            best = r.d;
        }
    }
    best
}

struct Item<'a> {
    link: &'a crate::project::Link,
    g: Vec<LatLng>,
    la0: f64,
    la1: f64,
    ln0: f64,
    ln1: f64,
    nm: String,
    rank: f64,
    b_ab: f64,
}

/// `DataHub.match(sc, counts)`: each count matched to the best link within the match radius.
///
/// Links that are closed or have a missing end node are left out. A count is a candidate for a link within the
/// radius (NZTA counts 70 m at least); NZTA ramp counts only for ramps and mainline counts only for other roads
/// (the road class adds 30 m below arterial); other counts with a road name need the link's name to match unless
/// they sit within 12 m of it (a mismatch adds 25 m). A directional count matches a link carrying traffic within
/// 80 degrees of its direction, and the direction it counts. A link of the count's own name wins whenever one is in range.
pub fn match_counts(sc: &Scenario, s: &Settings, counts: &[Rc<Count>]) -> Matches {
    let r = s.data.get().map_or(40.0, |d| d.match_radius.or(40.0));
    let mut items: Vec<Item> = Vec::new();
    for lk in sc.links.values() {
        if lk.status.is("closed") || !sc.nodes.contains_key(&lk.a) || !sc.nodes.contains_key(&lk.b) {
            continue;
        }
        let g = sc.geom(lk);
        let (mut la0, mut la1, mut ln0, mut ln1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for p in &g {
            la0 = math_min(la0, p[0]);
            la1 = math_max(la1, p[0]);
            ln0 = math_min(ln0, p[1]);
            ln1 = math_max(ln1, p[1]);
        }
        let b_ab = bearing(&g[0], &g[g.len() - 1]);
        items.push(Item {
            link: lk,
            nm: norm(&lk.name.as_str().unwrap_or("")),
            rank: lk.class().rank,
            b_ab,
            g,
            la0,
            la1,
            ln0,
            ln1,
        });
    }
    let pad = math_max(r, 70.0) / 90000.0 + 0.0002;
    let mut out: Matches = OrderedMap::new();
    for c in counts {
        let cn = norm_of(&c.road, &c.name);
        let ll = c.ll();
        let nzta = c.src_is("nzta");
        let mut best: Option<Match> = None;
        let mut best_named: Option<Match> = None;
        for it in &items {
            if ll[0] < it.la0 - pad || ll[0] > it.la1 + pad || ll[1] < it.ln0 - pad * 1.3 || ll[1] > it.ln1 + pad * 1.3 {
                continue;
            }
            let d = project_dist(&it.g, &ll);
            if d > (if nzta { math_max(r, 70.0) } else { r }) {
                continue;
            }
            let named = !it.nm.is_empty() && !cn.is_empty() && {
                let first_two = cn.split(' ').take(2).collect::<Vec<_>>().join(" ");
                cn.contains(&it.nm) || it.nm.contains(&first_two)
            };
            let mut score = d;
            if nzta {
                // NZTA ramp sites belong on ramps, mainline sites on the motorway or highway itself
                if c.ramp.truthy() != it.link.cls.is("ramp") {
                    continue;
                }
                score += if it.rank >= 4.0 { 0.0 } else { 30.0 };
            } else if !cn.is_empty() && !it.nm.is_empty() && !named {
                if d > 12.0 {
                    continue; // a named count on a differently named road only counts if it sits right on it
                }
                score += 25.0;
            }
            let mut dir = "both";
            if c.directional.truthy() && c.dir.truthy() {
                let want = dir_bearing(&c.dir.js_string());
                let ab = if it.link.lanes_ab.num() > 0.0 {
                    ang_diff(it.b_ab, want)
                } else {
                    999.0
                };
                let ba = if it.link.lanes_ba.num() > 0.0 {
                    ang_diff((it.b_ab + 180.0) % 360.0, want)
                } else {
                    999.0
                };
                if math_min(ab, ba) > 80.0 {
                    continue; // this road doesn't carry traffic in the counted direction
                }
                dir = if ab <= ba { "ab" } else { "ba" };
            } else if !(it.link.lanes_ab.num() > 0.0 && it.link.lanes_ba.num() > 0.0) {
                dir = if it.link.lanes_ab.num() > 0.0 { "ab" } else { "ba" };
            }
            let cand = Match {
                link: it.link.id.clone(),
                dir,
                d,
                score,
                named,
            };
            if named && best_named.as_ref().is_none_or(|b| score < b.score) {
                best_named = Some(cand.clone());
            }
            if best.as_ref().is_none_or(|b| score < b.score) {
                best = Some(cand);
            }
        }
        // a road of the count's own name wins whenever one is in range
        if let Some(m) = best_named.or(best) {
            out.insert(c.id.clone(), m);
        }
    }
    out
}

/// `DataHub.matches(sc)`: the project's counts matched to the scenario's links.
pub fn matches(sc: &Scenario, s: &Settings, data: &Data) -> Matches {
    match_counts(sc, s, &data.counts)
}
