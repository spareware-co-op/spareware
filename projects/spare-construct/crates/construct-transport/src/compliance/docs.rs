//! The standing disclaimers, the sign-off fingerprint, CSV stamping and the expert disclosure material (packs/nz/compliance.js
//! sections 12 and 13). Every export of a human-readable table carries the disclaimer D1, the DRAFT watermark while nobody has signed
//! the inputs off, the version stamp and the model status as footer comment rows.

use super::data::c as table;
use super::dates::{long_date, today};
use super::engine;
use super::jv::{Jx, V, csv, jarr, jobj, st};
use super::record::{state, stamp, version};
use super::rx::rx;
use crate::util;

/// A 16-character fingerprint of a text (FNV-1a over its UTF-16 units with two seeds), the sign-off fingerprint.
#[must_use]
pub fn hash(text: &str) -> String {
    // The prototype works in 32-bit integers (Math.imul); the same bits in u32 give the same text
    let b_mul: u32 = 2_246_822_519;
    let mut a: u32 = 0x811c_9dc5;
    let mut b: u32 = 0x0100_0193 ^ 0x5bd1_e995;
    for c in text.encode_utf16() {
        let c = u32::from(c);
        a = (a ^ c).wrapping_mul(16_777_619);
        b = (b ^ c).wrapping_mul(b_mul);
    }
    format!("{a:08x}{b:08x}")
}

/// `inputsOf(P)`: the text of everything a sign-off covers: the settings (without the view settings), every scenario's network, zones,
/// routes and matrix, the matrix estimation, the register and the counts.
#[must_use]
pub fn inputs_of(p: &V) -> String {
    let mut s = if p.g("settings").is_obj() { p.g("settings").clone() } else { V::object() };
    for k in ["period", "basemap", "linzKey", "sim"] {
        s.remove(k);
    }
    let nn = |v: &V| if v.is_null() { V::Null } else { v.clone() };
    let mut scs: Vec<V> = Vec::new();
    for sc in p.g("scenarios").items() {
        let mut o = V::object();
        // a field the scenario lacks is left out of the text, as JSON.stringify leaves out undefined
        for (k, f) in [("id", "id"), ("n", "nodes"), ("l", "links"), ("z", "zones"), ("r", "routes")] {
            if sc.has(f) {
                o.set(k, sc.g(f).clone());
            }
        }
        o.set("od", if sc.g("odManual").t() { sc.g("odManual").clone() } else { V::Null });
        for (k, f) in [("y", "year"), ("ro", "role"), ("b", "buildout"), ("p", "periods"), ("si", "sites")] {
            o.set(k, nn(sc.g(f)));
        }
        scs.push(o);
    }
    let counts: Vec<V> = p
        .g("data")
        .g("counts")
        .items()
        .iter()
        .map(|c| jarr![c.g("id"), c.g("adt"), c.g("am"), c.g("pm"), c.g("peak"), if c.g("date").t() { c.g("date").clone() } else { V::Null }])
        .collect();
    let mut all = V::object();
    all.set("s", s);
    all.set("sc", V::Array(scs));
    all.set("me", if p.g("me").t() { p.g("me").clone() } else { V::Null });
    all.set("reg", if p.g("register").t() { p.g("register").clone() } else { V::Null });
    all.set("c", V::Array(counts));
    all.stringify()
}

/// `fingerprint(P)`: the fingerprint of the inputs a sign-off covers.
#[must_use]
pub fn fingerprint(p: &V) -> String {
    hash(&inputs_of(p))
}

/// `signState(P)`: whether the inputs are signed off. Sign-offs whose fingerprint no longer matches are stale.
#[must_use]
pub fn sign_state(p: &V) -> V {
    let c = state(p);
    let signoffs = c.g("expert").g("signoffs");
    let fp = if signoffs.jlen() > 0 { Some(fingerprint(p)) } else { None };
    let matches = |s: &V| fp.as_deref().is_some_and(|f| s.g("fp").eq_s(f));
    let valid: Vec<V> = signoffs.items().iter().filter(|s| matches(s)).cloned().collect();
    let stale: Vec<V> = signoffs.items().iter().filter(|s| !matches(s)).cloned().collect();
    let signed = valid.iter().any(|s| s.g("role").eq_s("prepared"));
    let sup = c.g("expert").g("supersession");
    let _ = sup;
    let supplementary = c.g("expert").g("supplementary");
    let prompt = if !stale.is_empty() || supplementary.t() {
        "Inputs changed after sign-off, so the sign-off is cleared. Has the opinion changed? A supplementary statement may be needed (Practice Note s9.3(f))."
    } else {
        ""
    };
    jobj! { "signed" => signed, "valid" => V::Array(valid), "stale" => V::Array(stale), "watermark" => if signed { "" } else { table("DRAFT").as_str().unwrap_or("") },
        "supplementary" => supplementary, "prompt" => prompt }
}

/// `disclaimer(id, P, opts)`: the text of a standing disclaimer for the project (the version date filled in, the tool named for
/// D12, and anything the author added after it). Empty for an id that is not one.
#[must_use]
pub fn disclaimer(id: &str, p: Option<&V>, tool: Option<&str>) -> String {
    let d = table("DISCLAIMERS").g(id);
    if !d.t() {
        return String::new();
    }
    let mut t = replace_first(&d.g("text").s(), "[version date]", &long_date(version()));
    if id == "D12"
        && let Some(tool) = tool.filter(|t| !t.is_empty()) {
            t = rx(r"\[[^\]]+\]", "").replace_with(&t, tool);
        }
    let add = p.map_or(V::Null, |p| state(p).g("notes").g(id).clone());
    if add.t() { format!("{t} {}", add.s()) } else { t }
}

/// `String.prototype.replace(text, text)`: the first occurrence only.
fn replace_first(s: &str, from: &str, to: &str) -> String {
    match s.split_once(from) {
        Some((before, after)) => format!("{before}{to}{after}"),
        None => s.to_string(),
    }
}

/// `disclaimerHtml(id, P, opts)`: the disclaimer as a paragraph, with its title.
#[must_use]
pub fn disclaimer_html(id: &str, p: Option<&V>, tool: Option<&str>) -> String {
    format!(
        "<p class=\"note disclaimer\" data-disclaimer=\"{id}\"><b>{} ({id}).</b> {}</p>",
        util::esc(&table("DISCLAIMERS").g(id).g("title").sn()),
        util::esc(&disclaimer(id, p, tool))
    )
}

/// `csvFooter(P, opts)`: the footer comment rows of a CSV export: D1, the DRAFT watermark while unsigned, the version stamp, the model
/// status of both peaks and, for a hand-over file, D12 naming the tool.
#[must_use]
pub fn csv_footer(p: &V, handover: Option<&str>) -> String {
    let sg = sign_state(p);
    let mut rows: Vec<Vec<V>> = vec![vec![st(format!("# {}", disclaimer("D1", Some(p), None)))]];
    if sg.g("watermark").t() {
        rows.push(vec![st(format!("# {}", sg.g("watermark").s()))]);
    }
    rows.push(vec![st(format!("# {}", stamp(p, &V::Null).g("footer").s()))]);
    let status: Vec<String> = ["AM", "PM"].iter().map(|per| engine::model_status(p, per).g("footer").s()).collect();
    rows.push(vec![st(format!("# {}", status.join(" \u{b7} ")))]);
    if let Some(tool) = handover.filter(|t| !t.is_empty()) {
        rows.push(vec![st(format!("# {}", disclaimer("D12", Some(p), Some(tool))))]);
    }
    csv(&rows)
}

/// `stampCSV(csv, P, opts)`: a CSV with the footer rows added; one that already carries the version stamp is returned as it is.
#[must_use]
pub fn stamp_csv(csv: &str, p: &V, handover: Option<&str>) -> String {
    if rx(r#"^"?# Version stamp: "#, "m").test(csv) {
        return csv.to_string();
    }
    let body = if csv.is_empty() { String::new() } else { format!("{}\r\n", rx(r"(\r?\n)*$", "").replace_with(csv, "")) };
    body + &csv_footer(p, handover)
}

/// `todayText(now)`: today as the stamp prints it (see [`today`]).
#[must_use]
pub fn day_text(now: &V) -> String {
    today(now)
}
