//! The scope and wording linter (compliance-extra.js section 5): each rule says when it applies to the project's pathway and
//! lodgement date, finds its wording in the texts the author wrote (the Compliance+ text box, the report's author text and the
//! mitigation and options registers) and either asks for a check or blocks the outputs that carry author text.

use super::ctx::{self, stamped};
use crate::compliance::data::{re_of, x as xdata};
use crate::compliance::engine;
use crate::compliance::html::ReportCtx;
use crate::compliance::jv::{Jx, V, csv, jobj, obj_at, trim};
use crate::compliance::rx::rx;

/// `String.prototype.length` of text (UTF-16 code units).
fn u16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.slice(a, b)` by UTF-16 code unit (a lone surrogate left by the cut prints as the replacement character).
fn u16_slice(s: &str, a: usize, b: usize) -> String {
    let u: Vec<u16> = s.encode_utf16().collect();
    let (a, b) = (a.min(u.len()), b.min(u.len()));
    if a >= b {
        String::new()
    } else {
        String::from_utf16_lossy(&u[a..b])
    }
}

/// `modelValidated(P)`: the model is validated for the AM or PM peak (false when the status cannot be read).
fn model_validated(p: &V) -> bool {
    let run = || {
        ["AM", "PM"]
            .iter()
            .any(|per| engine::model_status(p, per).g("status").eq_s("validated"))
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).unwrap_or(false)
}

/// The `when(c, P)` of a rule: whether it applies to the context (and project), by rule id.
fn rule_applies(id: &str, c: &V, p: &V) -> bool {
    let kind = c.g("kind");
    match id {
        "scope-1a" => {
            (kind.eq_s("rc") || c.g("pathway").eq_s("ftaa-consent"))
                && !c.g("pathway").eq_s("rc-subdivision")
                && c.g("side").eq_s("transition")
        }
        "scope-95a9" => c.g("post").t() && !kind.eq_s("pc"),
        "scope-1c" => {
            (kind.eq_s("rc") || kind.eq_s("nor") || kind.eq_s("ftaa"))
                && (c.g("side").eq_s("transition") || !c.g("lodgement").t())
        }
        "scope-rca" => c.g("asAt").s().as_str() >= xdata("STD.dates.rca").s().as_str(),
        "scope-mbcm" => !(c.g("mbcmFull").t() && model_validated(p)),
        // scope-cl7, scope-e27-ita, scope-tmdg and scope-rasf apply to every project
        _ => true,
    }
}

/// The level of a rule for a context (`r.level`, a function for the rule that blocks only in the transition regime).
fn rule_level(r: &V, c: &V) -> String {
    if r.g("level").is_str() {
        return r.g("level").s();
    }
    if r.g("id").eq_s("scope-1c") {
        return if c.g("side").eq_s("transition") {
            "block"
        } else {
            "warn"
        }
        .to_string();
    }
    "warn".to_string()
}

/// The text the linter reads (`lintSources(P)`): the Compliance+ text box, the report's author text and free text in the mitigation and
/// options registers: `[{ where, text }]`.
pub fn lint_sources(p: &V) -> Vec<V> {
    let cx = ctx::data(p);
    let mut out: Vec<V> = Vec::new();
    let t = cx.g("lint").g("text");
    let tt = if t.t() { t.s() } else { String::new() };
    if !trim(&tt).is_empty() {
        out.push(jobj! { "where" => "Compliance+ text", "text" => t });
    }
    let rt = p.g("report").g("text");
    for (k, v) in rt.entries() {
        if v.is_str() && !trim(&v.s()).is_empty() {
            out.push(jobj! { "where" => format!("Report: {k}"), "text" => v });
        }
    }
    for m in cx.g("mitigation").items() {
        for k in ["measure", "options", "trigger"] {
            if m.g(k).t() {
                let r = m.g("ref");
                out.push(jobj! { "where" => trim(&format!("Mitigation {}", if r.t() { r.s() } else { String::new() })), "text" => m.g(k).s() });
            }
        }
    }
    for o in cx.g("options").items() {
        if o.g("notes").t() {
            let n = o.g("name");
            out.push(jobj! { "where" => trim(&format!("Option {}", if n.t() { n.s() } else { String::new() })), "text" => o.g("notes").s() });
        }
    }
    out
}

/// `sentences(t)`: the text split after a full stop, exclamation or question mark and at line breaks, each trimmed, blanks left out.
pub fn sentences(t: &V) -> Vec<String> {
    rx(r"(?<=[.!?])\s+|\n+", "")
        .split(&t.s())
        .into_iter()
        .map(|s| trim(&s).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `lint(P, text)`: the findings of the statutory scope and wording linter for the project's texts (or only `text`): `{ findings, blocking, applies, context }`.
/// Each finding is `{ id, key, rule, source, level, where, sentence, match, suggest, overridden }`.
pub fn lint(p: &V, text: Option<&str>) -> V {
    let c = ctx::context(p);
    let cx = ctx::data(p);
    let lx = cx.g("lint");
    let (ign, ovr) = (lx.g("ignore"), lx.g("override"));
    let rules = xdata("LINT");
    let applies: Vec<&V> = rules
        .items()
        .iter()
        .filter(|r| rule_applies(&r.g("id").s(), &c, p))
        .collect();
    let srcs: Vec<V> = match text {
        Some(t) => vec![jobj! { "where" => "Text", "text" => t }],
        None => lint_sources(p),
    };
    let mut findings: Vec<V> = Vec::new();
    for s in &srcs {
        for sen in sentences(s.g("text")) {
            for r in &applies {
                let res: Vec<&V> = if r.g("re").is_arr() {
                    r.g("re").items().iter().collect()
                } else {
                    vec![r.g("re")]
                };
                let Some(m) = res.iter().find_map(|x| re_of(x).exec(&sen)) else {
                    continue;
                };
                if r.g("also").t() && !re_of(r.g("also")).test(&sen) {
                    continue;
                }
                if r.g("skip").t() && r.g("skip").items().iter().any(|x| re_of(x).test(&sen)) {
                    continue;
                }
                let key = format!("{}|{}", r.g("id").s(), u16_slice(&sen, 0, 80));
                let level = rule_level(r, &c);
                if ign.g(&key).t() && level != "block" {
                    continue;
                }
                let reason = ovr.g(&key).g("reason");
                let released = level == "block"
                    && ovr.g(&key).t()
                    && !trim(&if reason.t() {
                        reason.s()
                    } else {
                        String::new()
                    })
                    .is_empty();
                let sentence = if u16_len(&sen) > 240 {
                    format!("{}\u{2026}", u16_slice(&sen, 0, 237))
                } else {
                    sen.clone()
                };
                let suggest = if level == "block" || !r.g("warnText").t() {
                    r.g("suggest")
                } else {
                    r.g("warnText")
                };
                let overridden = if released {
                    let o = ovr.g(&key);
                    jobj! { "reason" => o.g("reason").s(), "at" => if o.g("at").t() { o.g("at").clone() } else { V::String(String::new()) } }
                } else {
                    V::Null
                };
                findings.push(jobj! {
                    "id" => r.g("id"), "key" => key.as_str(), "rule" => r.g("rule"), "source" => r.g("source"), "level" => level.as_str(), "where" => s.g("where"),
                    "sentence" => sentence, "match" => m.text(), "suggest" => suggest, "overridden" => overridden,
                });
            }
        }
    }
    let blocking: Vec<V> = findings
        .iter()
        .filter(|f| f.g("level").eq_s("block") && !f.g("overridden").t())
        .cloned()
        .collect();
    jobj! { "findings" => findings, "blocking" => blocking, "applies" => applies.iter().map(|r| r.g("id").clone()).collect::<Vec<V>>(), "context" => c }
}

/// `overrideLint(P, key, reason)`: records (or, with an empty reason, clears) the author's reason for releasing a blocking finding. Changes the project.
/// Returns the override now held for the key (`{ reason, at }`), or null.
pub fn override_lint(p: &mut V, key: &str, reason: &str) -> V {
    let cx = ctx::data_mut(p);
    let lx = obj_at(cx, "lint");
    let ov = obj_at(lx, "override");
    if !trim(reason).is_empty() {
        ov.set(
            key,
            jobj! { "reason" => trim(reason), "at" => ctx::today() },
        );
    } else {
        ov.remove(key);
    }
    let held = ov.g(key);
    if held.t() { held.clone() } else { V::Null }
}

/// `exportBlock(P)`: why exports that carry author text are refused, None when none is blocked.
pub fn export_block(p: &V) -> Option<String> {
    let l = lint(p, None);
    let f = l.g("blocking").items().first()?;
    Some(format!(
        "Export blocked by the scope linter: {} (\"{}\" in {}). {}",
        f.g("rule").s(),
        f.g("match").s(),
        f.g("where").s(),
        f.g("suggest").s()
    ))
}

/// `lintCSV(P)`: the findings as a CSV with the version stamp and disclaimers in footer rows.
pub fn lint_csv(p: &V) -> String {
    let l = lint(p, None);
    let s = |x: &str| V::String(x.to_string());
    let mut rows: Vec<Vec<V>> = vec![
        [
            "Rule",
            "Level",
            "Where",
            "Text",
            "Source",
            "Suggested change",
            "Author override",
        ]
        .iter()
        .map(|h| s(h))
        .collect(),
    ];
    for f in l.g("findings").items() {
        let level = if f.g("level").eq_s("block") {
            if f.g("overridden").t() {
                "Blocks export (overridden)"
            } else {
                "Blocks export"
            }
        } else {
            "Check"
        };
        let ov = f.g("overridden");
        rows.push(vec![
            f.g("rule").clone(),
            s(level),
            f.g("where").clone(),
            f.g("sentence").clone(),
            f.g("source").clone(),
            f.g("suggest").clone(),
            if ov.t() {
                s(&format!("{} ({})", ov.g("reason").s(), ov.g("at").s()))
            } else {
                s("")
            },
        ]);
    }
    stamped(&csv(&rows), p, None)
}

/// The report sections of this part the report lane can ask for (see [`report`]); the linter has none.
pub const REPORT_IDS: &[&str] = &[];

/// The HTML of a report section of this part (`REPORT[id].render(P, ctx)`); None when the id is not one of this part's. A section that has
/// nothing to say gives an empty string, as the prototype's does.
pub fn report(_id: &str, _p: &V, _ctx: Option<&dyn ReportCtx>) -> Option<String> {
    None
}

/// The exports of this part (`EXPORTS`, ids as the prototype registers them: "compliance-extra-<id>").
pub const EXPORT_IDS: &[&str] = &["compliance-extra-lint"];

/// The text of an export of this part (`build(P)`); None when the id is not one of this part's.
pub fn export_build(id: &str, p: &V) -> Option<String> {
    (id == "compliance-extra-lint").then(|| lint_csv(p))
}

/// Whether an export of this part is offered for the project (`enabled()`); None when the id is not one of this part's.
pub fn export_enabled(id: &str, _p: &V) -> Option<bool> {
    (id == "compliance-extra-lint").then_some(true)
}
