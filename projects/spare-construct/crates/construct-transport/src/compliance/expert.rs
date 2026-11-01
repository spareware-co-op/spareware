//! Expert disclosure, sign-off, the assumption diff and the model status badge (packs/nz/compliance.js sections 12 and 13).
//!
//! The sign-off covers the inputs by their fingerprint ([`super::docs::fingerprint`]): changing an input clears it. The assumption diff
//! compares this project's parameters with another party's file, which is untrusted: only the parameters Spare Construct compares are
//! read, a path segment such as `__proto__` is refused, and the file's JSON is read by the same rules and error messages as the
//! prototype's `JSON.parse` (see [`parse_json`]).

mod jsonparse;

use super::data::c as table;
use super::dates::{long_date, long_date_v, now_ms, today};
use super::docs::{fingerprint, sign_state, stamp_csv};
use super::engine;
use super::html::pill;
use super::jv::{Jx, V, arr_at, csv, jobj, lower, merge, ns, obj_at, parse_csv, slice, st, trim};
use super::record::{active_sc, esc, numf, stamp, state, state_mut, version};
use super::rfi::{engine_view, is_object_key, landuse_label, rate_of};
use super::rx::rx;
use crate::graph::js_sort_by;
use crate::jsnum::{math_max, truthy_number};
use crate::util;

/// `${o[k]}`: a field printed in a template string (a field that is not there prints as undefined).
fn tpl(o: &V, k: &str) -> String {
    if o.has(k) {
        o.g(k).s()
    } else {
        "undefined".to_string()
    }
}

/// `o[k] = src[field]` unless the source field is undefined (JSON drops it).
fn put_if(o: &mut V, k: &str, src: &V, field: &str) {
    if src.has(field) {
        o.set(k, src.g(field).clone());
    }
}

/// `x || ''` read as text: the value's text when it is truthy, else nothing.
fn or_blank(v: &V) -> String {
    if v.t() { v.s() } else { String::new() }
}

/// `x || {}`: the value when it is truthy, else an empty object.
fn or_obj(v: &V) -> V {
    if v.t() { v.clone() } else { V::object() }
}

/* ====================================================================================================
12. Expert disclosure, sign-off and assumption diff (reg-expert)
==================================================================================================== */

/// `signOff(P, who, now)`: records a sign-off of the inputs for a role by a named person with qualifications (`{ role, name, quals, date, fp }`); an error text when the name or qualifications are missing. Changes the project.
pub fn sign_off(p: &mut V, who: &V, now: &V) -> Result<V, String> {
    state_mut(p);
    let c = state(p);
    let author = c.g("expert").g("author");
    // String(who.x || author.x || '').trim()
    let pick = |a: &V, b: &V| -> String {
        let t = if a.t() {
            a.s()
        } else if b.t() {
            b.s()
        } else {
            String::new()
        };
        trim(&t).to_string()
    };
    let name = pick(who.g("name"), author.g("name"));
    let quals = pick(who.g("quals"), author.g("quals"));
    // ROLES[who.role]: the three roles (and, as on any object, the names every object has)
    let key = who.g("role").s();
    let role =
        if matches!(key.as_str(), "prepared" | "reviewed" | "approved") || is_object_key(&key) {
            who.g("role").clone()
        } else {
            st("prepared")
        };
    if name.is_empty() || quals.is_empty() {
        return Err("Enter the name and qualifications of the person signing off.".to_string());
    }
    let s = jobj! { "role" => &role, "name" => name.as_str(), "quals" => quals.as_str(), "date" => today(now), "fp" => fingerprint(p) };
    let e = obj_at(state_mut(p), "expert");
    let mut kept: Vec<V> = e
        .g("signoffs")
        .items()
        .iter()
        .filter(|x| !x.g("role").strict_eq(&role))
        .cloned()
        .collect();
    kept.push(s.clone());
    e.set("signoffs", V::Array(kept));
    e.set("supplementary", V::Null);
    Ok(s)
}

/// `clearStale(P, now)`: clears the sign-offs whose inputs changed, keeping them in history; the number cleared. Changes the project.
pub fn clear_stale(p: &mut V, now: &V) -> usize {
    state_mut(p);
    let c = state(p);
    let signoffs = c.g("expert").g("signoffs");
    if signoffs.jlen() == 0 {
        return 0;
    }
    let fp = fingerprint(p);
    let stale: Vec<V> = signoffs
        .items()
        .iter()
        .filter(|s| !s.g("fp").eq_s(&fp))
        .cloned()
        .collect();
    if stale.is_empty() {
        return 0;
    }
    let d = today(now);
    let e = obj_at(state_mut(p), "expert");
    let history = arr_at(e, "history");
    for s in &stale {
        history.push(merge(
            s,
            &jobj! { "cleared" => d.as_str(), "reason" => "inputs changed after sign-off" },
        ));
    }
    let kept: Vec<V> = e
        .g("signoffs")
        .items()
        .iter()
        .filter(|s| s.g("fp").eq_s(&fp))
        .cloned()
        .collect();
    e.set("signoffs", V::Array(kept));
    e.set("supplementary", jobj! { "at" => d.as_str(), "text" => "Inputs changed after sign-off. Has the opinion changed? A supplementary statement may be needed (Practice Note s9.3(f))." });
    stale.len()
}

/// `dataSources(P)`: every data source with its date and basis: `[{ source, date, detail, basis }]`.
pub fn data_sources(p: &V) -> V {
    let c = state(p);
    let loaded = or_obj(p.g("data").g("loaded"));
    let mut rows: Vec<V> = Vec::new();
    let names: [(&str, &str); 7] = [
        ("nzta", "NZTA state highway counts (TMS)"),
        ("at", "Auckland Transport traffic counts"),
        ("cas", "NZTA Crash Analysis System"),
        ("user", "Counts imported from a file"),
        ("fc", "AT Future Connect network classes"),
        ("controlled", "AT controlled intersections"),
        (
            "osm",
            "Road network (OpenStreetMap contributors, ODbL, via the Overpass API) as imported and edited",
        ),
    ];
    for (k, v) in loaded.entries() {
        if v.t() && v.g("at").t() {
            let source = if v.g("source").t() {
                v.g("source").clone()
            } else {
                names
                    .iter()
                    .find(|n| n.0 == k)
                    .map_or_else(|| st(k), |n| st(n.1))
            };
            let detail = [
                if !v.g("n").nullish() {
                    format!("{} records", v.g("n").s())
                } else {
                    String::new()
                },
                or_blank(v.g("label")),
                or_blank(v.g("licence")),
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" \u{b7} ");
            rows.push(jobj! { "source" => source, "date" => today(v.g("at")), "detail" => detail, "basis" => if k == "user" { "user" } else { "measured" } });
        }
    }
    // a source the lookup or the overlays recorded: its own source, date and licence
    let measured = |s: &V| {
        let mut o = V::object();
        put_if(&mut o, "source", s, "source");
        put_if(&mut o, "date", s, "date");
        put_if(&mut o, "detail", s, "licence");
        o.set("basis", st("measured"));
        o
    };
    let site = c.g("site").g("lookup");
    if site.t() {
        for s in site.g("sources").items() {
            rows.push(measured(s));
        }
    }
    if c.g("overlays").t() {
        for s in c.g("overlays").g("sources").items() {
            rows.push(measured(s));
        }
    }
    let sh = c.g("net").g("loaded").g("sh");
    if sh.t() {
        let mut o = V::object();
        put_if(&mut o, "source", sh, "source");
        put_if(&mut o, "date", sh, "date");
        o.set(
            "detail",
            st(format!(
                "{} links matched \u{b7} {}",
                tpl(sh, "n"),
                tpl(sh, "licence")
            )),
        );
        o.set("basis", st("measured"));
        rows.push(o);
    }
    // the road network and the trip rates carry a date too (reg-expert: every data source with its date)
    if !(loaded.g("osm").t() && loaded.g("osm").g("at").t()) {
        let osm = p
            .g("scenarios")
            .items()
            .iter()
            .any(|s| s.g("links").vals().iter().any(|l| l.g("osm").t()));
        rows.push(jobj! { "source" => "Road network (OpenStreetMap contributors, ODbL) as imported and edited", "date" => if osm { "import date not recorded" } else { "drawn in Spare Construct" },
            "detail" => "Spare Construct network", "basis" => if osm { "derived" } else { "user" } });
    }
    let settings = or_obj(p.g("settings"));
    let mut used: Vec<String> = Vec::new();
    for s in p.g("scenarios").items() {
        for z in s.g("zones").vals() {
            if !z.g("kind").eq_s("gateway") {
                for (k, v) in z.g("lu").entries() {
                    if v.n() > 0.0 && !used.iter().any(|u| u == k) {
                        used.push(k.to_string());
                    }
                }
            }
        }
    }
    for k in &used {
        let r = rate_of(&settings, k);
        let src = if r.g("src").t() {
            r.g("src").clone()
        } else {
            r.g("source").clone()
        };
        let yr = if r.g("year").n() > 0.0 {
            Some(r.g("year").s())
        } else {
            None
        };
        let label = landuse_label(k).unwrap_or_else(|| "undefined".to_string());
        let date = match &yr {
            Some(y) => format!("survey year {y}"),
            None if src.t() => "year not recorded".to_string(),
            None => format!("Spare Construct default (as at {})", long_date(version())),
        };
        rows.push(jobj! { "source" => format!("Trip rate: {label}"), "date" => date,
            "detail" => if src.t() { src.s() } else { "Spare Construct indicative rate (Settings); replace with a sourced, dated rate".to_string() }, "basis" => if src.t() { "user" } else { "assumed" } });
    }
    if used.is_empty() {
        rows.push(jobj! { "source" => "Trip rates (Settings)", "date" => format!("Spare Construct default (as at {})", long_date(version())), "detail" => "no land use in the zones yet", "basis" => "assumed" });
    }
    rows.push(jobj! { "source" => "Spare Construct regulatory data (thresholds, rules and disclaimers)", "date" => long_date(version()), "detail" => "sources and verified flags in Appendix A", "basis" => "derived" });
    V::Array(rows)
}

/// `appendixG(P)`: the expert disclosure material: `{ title, disclaimer, sections, signoffs, watermark, prompt }`.
pub fn appendix_g(p: &V) -> V {
    let c = state(p);
    let e = c.g("expert");
    let sg = sign_state(p);
    let ev = engine_view(p);
    let ms: Vec<V> = ["AM", "PM"]
        .iter()
        .map(|per| engine::model_status(&ev, per))
        .collect();
    let st0 = stamp(p, &V::Null);
    let sens: Vec<String> = p
        .g("scenarios")
        .items()
        .iter()
        .filter(|s| s.g("role").eq_s("sensitivity"))
        .map(|s| s.g("name").sn())
        .collect();
    let author = e.g("author");
    let qualifications = if author.g("name").t() {
        format!(
            "{}{}{}",
            author.g("name").s(),
            if author.g("quals").t() {
                format!(", {}", author.g("quals").s())
            } else {
                String::new()
            },
            if author.g("org").t() {
                format!(" ({})", author.g("org").s())
            } else {
                String::new()
            }
        )
    } else {
        "[Author to enter name and qualifications]".to_string()
    };
    let sources = data_sources(p)
        .items()
        .iter()
        .map(|r| {
            format!(
                "{} ({}; {}{})",
                tpl(r, "source"),
                tpl(r, "date"),
                tpl(r, "basis"),
                if r.g("detail").t() {
                    format!("; {}", r.g("detail").s())
                } else {
                    String::new()
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let standards = st0
        .g("lines")
        .items()
        .iter()
        .skip(2)
        .map(|l| format!("{}: {}", l.ix(0).s(), l.ix(1).s()))
        .collect::<Vec<_>>()
        .join("\n");
    let limits = [e.g("limitations"), e.g("gaps")]
        .iter()
        .filter(|x| x.t())
        .map(|x| x.s())
        .collect::<Vec<_>>()
        .join("\n");
    let tests = if e.g("tests").t() {
        e.g("tests").clone()
    } else {
        st("[Author to list the tests and who ran them]")
    };
    let conf = e.g("confidence");
    let standing = engine::standards_table().g("tmdg").g("standing").s();
    let sections = V::Array(vec![
        V::Array(vec![st("Qualifications"), st(qualifications)]),
        V::Array(vec![
            st("Data, facts and assumptions relied on"),
            st(sources),
        ]),
        V::Array(vec![
            st("Literature and standards, with editions"),
            st(standards),
        ]),
        V::Array(vec![st("Tests run and by whom"), tests]),
        V::Array(vec![
            st("Model description and status"),
            st(format!(
                "Spare Construct static assignment (conjugate Frank-Wolfe), HCM 6 / Akcelik-based junction delay (indicative, not SIDRA output). {}. {}",
                ms.iter()
                    .map(|m| m.g("footer").s())
                    .collect::<Vec<_>>()
                    .join("; "),
                standing
            )),
        ]),
        V::Array(vec![
            st("Sensitivity and uncertainty"),
            st(if sens.is_empty() {
                "No sensitivity scenario recorded yet (Practice Note s9.3(a)(x)).".to_string()
            } else {
                format!("Sensitivity scenarios: {}.", sens.join(", "))
            }),
        ]),
        V::Array(vec![
            st("Limitations and knowledge gaps"),
            st(if limits.is_empty() {
                "[Author to state limitations and knowledge gaps]".to_string()
            } else {
                limits
            }),
        ]),
        V::Array(vec![
            st("Confidence"),
            st(if conf.g("level").t() {
                format!(
                    "{}: {}",
                    conf.g("level").s(),
                    if conf.g("why").t() {
                        conf.g("why").s()
                    } else {
                        "[justify]".to_string()
                    }
                )
            } else {
                "[Author to choose high, moderate or low and justify it (Practice Note s9.3(e))]"
                    .to_string()
            }),
        ]),
    ]);
    jobj! { "title" => "Appendix G: Expert disclosure material", "disclaimer" => table("DISCLAIMERS.D2.text"), "sections" => sections, "signoffs" => sg.g("valid"), "watermark" => sg.g("watermark"), "prompt" => sg.g("prompt") }
}

/* ---- the assumption diff ---- */

/// The parameters Spare Construct compares (`DIFF_KEYS`).
const DIFF_KEYS: [&str; 17] = [
    r"^growth$",
    r"^growthMode$",
    r"^hvPct$",
    r"^pcuHV$",
    r"^peakToDaily$",
    r"^gravityBeta$",
    r"^satFlow$",
    r"^cycle$",
    r"^lostPerPhase$",
    r"^connectorSpeed$",
    r"^connectors$",
    r"^rates\.\w+\.(AM|PM)\.(rate|inPct)$",
    r"^junction\.(losMethod|pff|T|queueSpace|f95)$",
    r"^turns\.(left|right|rightSignals|uturn)$",
    r"^bprAlpha$",
    r"^bprBeta$",
    r"^baseYear$",
];

/// `BAD_SEG.test(s)`: a path segment that could reach an object's prototype.
fn bad_seg(s: &str) -> bool {
    matches!(s, "__proto__" | "constructor" | "prototype")
}

/// `safeKey(k)`: a dotted path of one of the compared parameters, with no empty or refused segment.
fn safe_key(k: &str) -> bool {
    k.encode_utf16().count() < 200
        && !k.split('.').any(|s| s.is_empty() || bad_seg(s))
        && DIFF_KEYS.iter().any(|re| rx(re, "").test(k))
}

/// `flatten(o, pre, out)`: the leaves of an object by dotted path (a refused key is skipped; an array is a leaf).
fn flatten(o: &V, pre: &str, out: &mut V) {
    for (k, v) in o.entries() {
        if bad_seg(k) {
            continue;
        }
        let p = if pre.is_empty() {
            k.to_string()
        } else {
            format!("{pre}.{k}")
        };
        if v.is_obj() {
            flatten(v, &p, out);
        } else {
            out.set(p, v.clone());
        }
    }
}

/// `cell(v)`: a CSV cell as a value: blank and "(blank)" are null, on and off are booleans, a number is a number.
fn cell(v: Option<&String>) -> V {
    let s = trim(v.map_or("", |s| s.as_str()));
    if s.is_empty() || s == "(blank)" {
        V::Null
    } else if s == "on" {
        V::Bool(true)
    } else if s == "off" {
        V::Bool(false)
    } else {
        let n = st(s).n();
        if n.is_finite() { V::Number(n) } else { st(s) }
    }
}

/// `put(k, v)` of `parseOther`: writes a compared parameter into the nested object at its dotted path.
fn put(o: &mut V, k: &str, v: V) {
    if !safe_key(k) {
        return;
    }
    let p: Vec<&str> = k.split('.').collect();
    let mut x = o;
    for seg in &p[..p.len() - 1] {
        if !x.has(seg) || !x.g(seg).is_obj() {
            x.set(*seg, V::object());
        }
        x = x.get_mut(seg).expect("segment set above");
    }
    x.set(p[p.len() - 1], v);
}

/// `JSON.parse(text)` as V8 reads it: the value, or the message V8's error carries (`Unexpected token '}', "{"a":}" is not valid JSON`).
pub fn parse_json(text: &str) -> Result<V, String> {
    jsonparse::parse(text)
}

/// `parseOther(text)`: the other party's parameters (a Spare Construct project file, a settings object or a key,value CSV) as an object of only the parameters compared; an error text when it cannot be read.
pub fn parse_other(text: &str) -> Result<V, String> {
    let t = trim(text);
    if t.is_empty() {
        return Err("Paste the other party's Spare Construct project file, settings JSON or a key,value CSV.".to_string());
    }
    let mut o = V::object();
    if rx(r"^[{[]", "").test(t) {
        let j = jsonparse::parse(t).map_err(|e| format!("That is not valid JSON: {e}"))?;
        let s = if j.t() && j.g("settings").is_obj() {
            j.g("settings")
        } else {
            &j
        };
        if !s.is_obj() {
            return Err("That JSON holds no settings. Paste a Spare Construct project file or a settings object.".to_string());
        }
        let mut f = V::object();
        flatten(s, "", &mut f);
        for (k, v) in f.entries() {
            put(&mut o, k, v.clone());
        }
        return Ok(o);
    }
    // a key,value template, or Spare Construct's own 'Assumptions (CSV)' export (setting,label,value,default,unit): read the value column
    let re_hash = rx(r"^\s*#", "");
    let mut rows: Vec<Vec<String>> = parse_csv(t)
        .into_iter()
        .filter(|r| !r.is_empty() && !re_hash.test(r.first().map_or("", |s| s.as_str())))
        .collect();
    let (mut ki, mut vi) = (0usize, 1usize);
    let h: Vec<String> = rows
        .first()
        .map(|r| r.iter().map(|x| lower(trim(x))).collect())
        .unwrap_or_default();
    let re_key = rx(r"^(setting|key|item|parameter)$", "");
    if h.iter().any(|x| x == "value") && h.iter().any(|x| re_key.test(x)) {
        ki = h.iter().position(|x| re_key.test(x)).unwrap_or(0);
        vi = h.iter().position(|x| x == "value").unwrap_or(1);
        rows.remove(0);
    }
    let re_head = rx(r"^(item|key|setting)$", "i");
    for r in &rows {
        let k = trim(r.get(ki).map_or("", |s| s.as_str())).to_string();
        if k.is_empty() || re_head.test(&k) {
            continue;
        }
        put(&mut o, &k, cell(r.get(vi)));
    }
    Ok(o)
}

/// `diffParams(P, other)`: the parameters that differ from the other party's (text or an object): `[{ key, a, b }]`.
pub fn diff_params(p: &V, other: &V) -> Result<V, String> {
    let mut a = V::object();
    flatten(p.g("settings"), "", &mut a);
    let parsed;
    let src = if other.is_obj() {
        other
    } else {
        parsed = parse_other(&or_blank(other))?;
        &parsed
    };
    let mut b = V::object();
    flatten(src, "", &mut b);
    let mut out: Vec<V> = Vec::new();
    for (k, bv) in b.entries() {
        if !safe_key(k) || !a.has(k) || a.g(k).s() == bv.s() {
            continue;
        }
        // a numeric setting only takes a number (a label or a typo is not a difference to run)
        if a.g(k).is_num() && !(bv.is_num() && bv.finite()) {
            continue;
        }
        out.push(jobj! { "key" => k, "a" => a.g(k), "b" => bv });
    }
    Ok(V::Array(out))
}

/// The field or item of an object or array (`t[k]`), None where it is not there.
fn child<'a>(t: &'a V, k: &str) -> Option<&'a V> {
    match t {
        V::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => t.get(k),
    }
}

/// The field or item of an object or array for changing.
fn child_mut<'a>(t: &'a mut V, k: &str) -> Option<&'a mut V> {
    match t {
        V::Array(a) => k.parse::<usize>().ok().and_then(move |i| a.get_mut(i)),
        _ => t.get_mut(k),
    }
}

/// `t[k] = v` on an object or array (false for a name on an array, which a JSON copy would not keep).
fn set_child(t: &mut V, k: &str, v: V) -> bool {
    match t {
        V::Array(a) => match k.parse::<usize>() {
            Ok(i) => {
                while a.len() <= i {
                    a.push(V::Null);
                }
                a[i] = v;
                true
            }
            Err(_) => false,
        },
        _ => {
            t.set(k, v);
            true
        }
    }
}

/// `Kit.setPath(o, k, v)`: sets a dotted path, making an object of a missing or empty step; a path with a refused segment, and a step that holds a plain value, stop the write (false).
fn kit_set_path(o: &mut V, k: &str, v: V) -> bool {
    let p: Vec<&str> = k.split('.').collect();
    if !(o.is_obj() || o.is_arr()) || p.iter().any(|x| bad_seg(x)) {
        return false;
    }
    let mut t: &mut V = o;
    for seg in &p[..p.len() - 1] {
        if !child(t, seg).is_some_and(|x| x.t()) && !set_child(t, seg, V::object()) {
            return false;
        }
        t = child_mut(t, seg).expect("step set above");
        if !(t.is_obj() || t.is_arr()) {
            return false;
        }
    }
    set_child(t, p[p.len() - 1], v)
}

/// The worst junction values of a result and its totals (`metric(r)` of `runDiff`).
fn metric(r: &V) -> V {
    // `x || 0`
    let or0 = |v: &V| {
        let x = v.n();
        if truthy_number(x) { x } else { 0.0 }
    };
    let (mut dos, mut dl, mut q) = (0.0, 0.0, 0.0);
    for n in r.g("nodes").vals() {
        dos = math_max(dos, or0(n.g("dos")));
        dl = math_max(dl, or0(n.g("delay")));
        for a in n.g("appr").items() {
            q = math_max(q, or0(a.g("q95")));
        }
    }
    let mut o = jobj! { "dos" => util::r3(dos), "delay" => util::r1(dl), "queue" => util::r1(q) };
    put_if(&mut o, "over", r.g("kpi"), "over");
    o.set("vht", V::Number(util::r1(r.g("kpi").g("vht").n())));
    o
}

/// `runDiff(P, diffs, opts)`: runs each difference alone against the project and ranks them by effect: `{ period, central, rows, ms, note }`.
///
/// `opts` can hold `sc` (the scenario, the active one by default), `period` (the project's by default) and `maxIter` for the assignments.
pub fn run_diff(p: &V, diffs: &V, opts: &V) -> V {
    let sc = if opts.g("sc").t() {
        opts.g("sc").clone()
    } else {
        active_sc(p).clone()
    };
    let period = if opts.g("period").t() {
        opts.g("period").s()
    } else if p.g("settings").g("period").t() {
        p.g("settings").g("period").s()
    } else {
        "AM".to_string()
    };
    let t0 = now_ms();
    let mut run_opts = jobj! { "period" => period.as_str() };
    put_if(&mut run_opts, "maxIter", opts, "maxIter");
    let central = metric(&engine::assign(p, &sc, Some(p.g("settings")), &run_opts));
    let mut rows: Vec<V> = Vec::new();
    for d in diffs.items() {
        let mut s2 = p.g("settings").clone();
        kit_set_path(&mut s2, &d.g("key").s(), d.g("b").clone());
        let m = metric(&engine::assign(p, &sc, Some(&s2), &run_opts));
        let rel = |x: f64, y: f64| {
            if y > 0.0 {
                (x - y).abs() / y
            } else if (x - y).abs() > 0.0 {
                1.0
            } else {
                0.0
            }
        };
        let score = rel(m.g("dos").n(), central.g("dos").n())
            + rel(m.g("delay").n(), central.g("delay").n())
            + rel(m.g("queue").n(), central.g("queue").n())
            + rel(m.g("over").n(), central.g("over").n());
        let arrow = "\u{2192}";
        let effect = format!(
            "worst DoS {} {arrow} {}; worst delay {} {arrow} {} s; longest queue {} {arrow} {} m; roads over capacity {} {arrow} {}",
            ns(central.g("dos").n()),
            ns(m.g("dos").n()),
            numf(central.g("delay").n(), 0),
            numf(m.g("delay").n(), 0),
            numf(central.g("queue").n(), 0),
            numf(m.g("queue").n(), 0),
            tpl(&central, "over"),
            tpl(&m, "over")
        );
        let name = format!(
            "{} \u{b7} {} = {}",
            tpl(&sc, "name"),
            tpl(d, "key"),
            tpl(d, "b")
        );
        rows.push(merge(
            d,
            &jobj! { "m" => &m, "score" => util::r3(score), "name" => name, "effect" => effect },
        ));
    }
    js_sort_by(&mut rows, |a, b| b.g("score").n() - a.g("score").n());
    jobj! { "period" => period, "central" => central, "rows" => V::Array(rows), "ms" => now_ms() - t0,
    "note" => "Each difference is run alone against this project (one-at-a-time), as an unsaved scenario variant." }
}

/// `jwsCSV(P, result)`: the joint witness statement input table as a stamped CSV.
pub fn jws_csv(p: &V, result: &V) -> String {
    let mut rows: Vec<Vec<V>> = vec![
        [
            "Item",
            "Party A (this project)",
            "Party B",
            "Effect",
            "Agreed?",
        ]
        .iter()
        .map(|x| st(*x))
        .collect(),
    ];
    for r in result.g("rows").items() {
        rows.push(vec![
            r.g("key").clone(),
            r.g("a").clone(),
            r.g("b").clone(),
            r.g("effect").clone(),
            st(""),
        ]);
    }
    // text where a list of rows should be is read a character at a time, and a character has none of the fields
    if let V::String(s) = result.g("rows") {
        for _ in s.chars() {
            rows.push(vec![V::Null, V::Null, V::Null, V::Null, st("")]);
        }
    }
    stamp_csv(&csv(&rows), p, None)
}

/* ====================================================================================================
13. Model status badge, disclaimers, standards supersession
==================================================================================================== */

/// `badge(P, period)`: the model status badge: the model status with `html` and `text`.
pub fn badge(p: &V, period: &str) -> V {
    let m = engine::model_status(&engine_view(p), period);
    let tone = if m.g("status").eq_s("validated") {
        "ok"
    } else if m.g("status").eq_s("calibrated") {
        ""
    } else {
        "warn"
    };
    merge(
        &m,
        &jobj! { "html" => format!("{} <span class=\"note\">{}</span>", pill(&esc(m.g("short")), tone), esc(m.g("footer"))), "text" => m.g("footer") },
    )
}

/// `decode(h)`: the HTML entities a disclaimer's text arrives with, as plain text.
fn decode(h: &str) -> String {
    h.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// `missingDisclaimers(html, kinds)`: the standing disclaimers a block of HTML must carry but does not, by what it shows.
///
/// A disclaimer counts as present when the HTML marks it (`data-disclaimer` or `data-d`) or carries its opening words. The HTML is decoded
/// first and only the text before any placeholder is compared. A kind that is not one of the known ones asks for nothing.
pub fn missing_disclaimers(html: &str, kinds: &[&str]) -> Vec<String> {
    let map: [(&str, &str); 11] = [
        ("junction", "D3"),
        ("economics", "D4"),
        ("anop", "D5"),
        ("validation", "D6"),
        ("model", "D7"),
        ("safety", "D8"),
        ("cbds", "D9"),
        ("regulatory", "D10"),
        ("authority", "D11"),
        ("handover", "D12"),
        ("assumption", "D13"),
    ];
    let mut need: Vec<&str> = vec!["D1"];
    for k in kinds {
        if let Some(m) = map.iter().find(|m| m.0 == *k)
            && !need.contains(&m.1)
        {
            need.push(m.1);
        }
    }
    let spaces = rx(r"\s+", "g");
    let text = spaces.replace_with(&decode(&rx(r"<[^>]*>", "g").replace_with(html, " ")), " ");
    need.into_iter()
        .filter(|id| {
            if rx(&format!("data-(disclaimer|d)=\"{id}\""), "").test(html) {
                return false;
            }
            let full = table("DISCLAIMERS").g(id).g("text").s();
            let before = full.split('[').next().unwrap_or("");
            let head = slice(trim(&spaces.replace_with(before, " ")), 0, Some(60));
            !text.contains(&head)
        })
        .map(|id| id.to_string())
        .collect()
}

/// `supersession(P, now)`: the standards that changed since the project's outputs were stamped: `[{ id, doc, edition, next, used, text }]`.
pub fn supersession(p: &V, now: &V) -> V {
    let c = state(p);
    let d = today(now);
    let mut out: Vec<V> = Vec::new();
    for s in table("STANDARDS").items() {
        let used = c.g("used").g(&s.g("id").s());
        let next = s.g("next");
        // `used < next.date` is a text comparison for text, and false for anything else (a date is not a number)
        if used.t()
            && next.t()
            && next.g("date").t()
            && d.as_str() >= next.g("date").s().as_str()
            && used.is_str()
            && used.s().as_str() < next.g("date").s().as_str()
        {
            out.push(jobj! { "id" => s.g("id"), "doc" => s.g("doc"), "edition" => s.g("edition"), "next" => next, "used" => used,
                "text" => format!("{} ({}) has changed since this project's outputs were stamped on {}: {} ({}). Re-check the affected outputs before relying on them.",
                    s.g("doc").s(), s.g("edition").s(), long_date_v(used), next.g("what").s(), long_date_v(next.g("date"))) });
        }
    }
    V::Array(out)
}
