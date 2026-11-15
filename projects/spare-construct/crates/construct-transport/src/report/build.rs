//! Putting the sections together: which sections the report has, their chapters and numbers, the table and figure numbering, the
//! disclaimers every table must carry and the contents list (`build` and the helpers it uses).

use super::consts::{Chapter, chapters};
use super::ctx::{Ctx, FigRequest, MapImage};
use super::host::ForeignSection;
use super::Sel;
use crate::jsnum::{is_js_space, js_trim};
use crate::util::esc;
use std::collections::HashMap;
use std::fmt::Write as _;

/// What `Report.html` and `Report.build` are given: the period the economics use and the maps already drawn.
#[derive(Clone, Debug, Default)]
pub struct BuildOpts {
    /// The period the economics use (the project's active period when unset).
    pub period: Option<String>,
    /// Maps already drawn, by figure id.
    pub map_images: HashMap<String, MapImage>,
}

/// One line of the contents list.
#[derive(Clone, Debug, PartialEq)]
pub struct TocEntry {
    /// Chapter id.
    pub ch: String,
    /// Section number.
    pub num: String,
    /// Section title as printed.
    pub title: String,
    /// Section id.
    pub id: String,
    /// 2 for a section, 3 for a sub-section.
    pub level: u8,
}

/// The built report: its body HTML, the maps its sections asked for and the chapters and contents it holds.
#[derive(Clone, Debug)]
pub struct Built {
    /// The body (chapters in order, with the contents list in place).
    pub body: String,
    /// The figures the sections asked for.
    pub requests: Vec<FigRequest>,
    /// The ids of the chapters that have sections.
    pub chapters: Vec<String>,
    /// The contents list.
    pub toc: Vec<TocEntry>,
    /// The scenarios that were assessed.
    pub sel: Sel,
}

/// A byte offset that is a character boundary at or after `i`.
fn ws_end(s: &str, i: usize) -> usize {
    let mut j = i;
    for c in s[i..].chars() {
        if is_js_space(c) {
            // Bound: an offset within s, far below usize::MAX
            j = j.saturating_add(c.len_utf8());
        } else {
            break;
        }
    }
    j
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `\bword\b` for an ASCII word: there is a word boundary on both sides.
pub(crate) fn has_word(s: &str, w: &str) -> bool {
    let mut from = 0usize;
    // Bound: offsets within s, far below usize::MAX
    while let Some(i) = s[from..].find(w) {
        let a = from.saturating_add(i);
        let b = a.saturating_add(w.len());
        let before = s[..a].chars().next_back().is_none_or(|c| !is_word(c));
        let after = s[b..].chars().next().is_none_or(|c| !is_word(c));
        if before && after {
            return true;
        }
        from = a.saturating_add(s[a..].chars().next().map_or(1, char::len_utf8));
    }
    false
}

/// `/Spare Construct assumption/i.test(s)`.
pub(crate) fn has_assumption(s: &str) -> bool {
    s.to_ascii_lowercase().contains("spare construct assumption")
}

/// `/class="src"|\bSource:/i.test(s)`: the text already says where it comes from.
pub(crate) fn has_source_line(s: &str) -> bool {
    if s.contains("class=\"src\"") {
        return true;
    }
    let low = s.to_ascii_lowercase();
    let mut from = 0usize;
    // Bound: offsets within low, far below usize::MAX
    while let Some(i) = low[from..].find("source:") {
        let a = from.saturating_add(i);
        if low[..a].chars().next_back().is_none_or(|c| !is_word(c)) {
            return true;
        }
        from = a.saturating_add(1);
    }
    false
}

/// `chapterOf(name, title)`: the chapter id of a section.
pub fn chapter_of(name: &str, title: &str) -> String {
    let s = js_trim(name).to_lowercase();
    if s.is_empty() {
        return "4".to_string();
    }
    // an appendix title names its letter: /^appendix\s+([a-h])\b/i
    let title_t = js_trim(title);
    let appendix_letter = |text: &str, ci: bool| -> Option<char> {
        let text = if ci { text.to_ascii_lowercase() } else { text.to_string() };
        let rest = text.strip_prefix("appendix")?;
        let ws = ws_end(rest, 0);
        if ws == 0 {
            return None;
        }
        let letter = rest[ws..].chars().next()?;
        if !('a'..='h').contains(&letter) {
            return None;
        }
        // Bound: an offset within rest, far below usize::MAX
        let after = rest[ws.saturating_add(letter.len_utf8())..].chars().next();
        if after.is_some_and(is_word) {
            return None;
        }
        Some(letter)
    };
    if let Some(letter) = appendix_letter(title_t, true) {
        let is_app = s.strip_prefix("appendi").is_some_and(|tail| {
            (tail.starts_with('x') && !tail[1..].chars().next().is_some_and(is_word)) || (tail.starts_with("ces") && !tail[3..].chars().next().is_some_and(is_word))
        });
        if is_app {
            return letter.to_ascii_uppercase().to_string();
        }
    }
    // /^(?:chapter\s+)?(\d)(?:[.\s]|$)/
    {
        let body = match s.strip_prefix("chapter") {
            Some(rest) if ws_end(rest, 0) > 0 => &rest[ws_end(rest, 0)..],
            _ => s.as_str(),
        };
        let mut it = body.chars();
        if let Some(digit) = it.next().filter(char::is_ascii_digit) {
            let tail = it.next();
            if tail.is_none() || tail == Some('.') || tail.is_some_and(is_js_space) {
                let num = digit.to_digit(10).unwrap_or(0);
                return if (1..=6).contains(&num) {
                    digit.to_string()
                } else if num == 7 {
                    "6".to_string()
                } else {
                    "4".to_string()
                };
            }
        }
    }
    // /^appendix\s+([a-h])\b/ or /^([a-hp])(\d+)?$/
    if let Some(letter) = appendix_letter(&s, false) {
        return letter.to_ascii_uppercase().to_string();
    }
    {
        let mut it = s.chars();
        if let Some(letter) = it.next().filter(|ch| ('a'..='h').contains(ch) || *ch == 'p') {
            let rest: String = it.collect();
            if rest.chars().all(|ch| ch.is_ascii_digit()) {
                return letter.to_ascii_uppercase().to_string();
            }
        }
    }
    let all = chapters();
    let found: Option<&Chapter> = all
        .iter()
        .find(|ch| ch.title.to_lowercase() == s || ch.alias.contains(&s.as_str()))
        .copied()
        .or_else(|| all.iter().find(|ch| ch.alias.iter().any(|alias| s.contains(alias) && alias.len() > 4)).copied());
    found.map_or("4".to_string(), |ch| ch.id.to_string())
}

/// `fixTables(h)`: a table without a head gets a <thead> from its first row when that row has header cells, so headers
/// repeat on every printed page.
#[must_use]
pub fn fix_tables(h: &str) -> String {
    // Bound: lengths and offsets within h, far below usize::MAX
    let mut out = String::with_capacity(h.len().saturating_add(64));
    let mut rest = h;
    loop {
        // the next `<table\b[^>]*>`
        let Some(start) = find_table_open(rest) else {
            out.push_str(rest);
            break;
        };
        let Some(end_rel) = rest[start..].find("</table>") else {
            out.push_str(rest);
            break;
        };
        let end = start.saturating_add(end_rel).saturating_add("</table>".len());
        out.push_str(&rest[..start]);
        out.push_str(&fix_one_table(&rest[start..end]));
        rest = &rest[end..];
    }
    out
}

/// The byte index of the next `<table` that is followed by a word boundary and a `>` somewhere after.
fn find_table_open(s: &str) -> Option<usize> {
    let mut from = 0usize;
    // Bound: offsets within s, far below usize::MAX
    while let Some(i) = s[from..].find("<table") {
        let a = from.saturating_add(i);
        let tag_end = a.saturating_add(6);
        let after = s[tag_end..].chars().next();
        if !after.is_some_and(is_word) && s[tag_end..].contains('>') {
            return Some(a);
        }
        from = a.saturating_add(1);
    }
    None
}

fn fix_one_table(t: &str) -> String {
    if t.to_ascii_lowercase().contains("<thead") {
        return t.to_string();
    }
    // ^(<table\b[^>]*>)\s*(<tr\b[^>]*>(?:(?!<\/tr>)[\s\S])*?<th\b[\s\S]*?<\/tr>)([\s\S]*)<\/table>$
    let Some(gt) = t.find('>') else {
        return t.to_string();
    };
    let open = &t[..=gt];
    // Bound: offsets within t, far below usize::MAX
    let after = ws_end(t, gt.saturating_add(1));
    let r = &t[after..];
    if !r.starts_with("<tr") || r[3..].chars().next().is_some_and(is_word) {
        return t.to_string();
    }
    let Some(trgt) = r.find('>') else {
        return t.to_string();
    };
    let body_start = trgt.saturating_add(1);
    let Some(row_end_rel) = r[body_start..].find("</tr>") else {
        return t.to_string();
    };
    let row_body_end = body_start.saturating_add(row_end_rel);
    let row_body = &r[body_start..row_body_end];
    // the row must hold a <th followed by a word boundary
    let mut has_th = false;
    let mut from = 0usize;
    while let Some(i) = row_body[from..].find("<th") {
        let a = from.saturating_add(i);
        if !row_body[a.saturating_add(3)..].chars().next().is_some_and(is_word) {
            has_th = true;
            break;
        }
        from = a.saturating_add(1);
    }
    if !has_th {
        return t.to_string();
    }
    let row_end = row_body_end.saturating_add("</tr>".len());
    let row = &r[..row_end];
    let rest = &r[row_end..];
    let Some(rest) = rest.strip_suffix("</table>") else {
        return t.to_string();
    };
    format!("{open}<thead>{row}</thead><tbody>{rest}</tbody></table>")
}

/// The heading just above a table that `numberForeign` turns into its caption: (start index, tag, attributes, inner, tail).
fn heading_before(out: &str) -> Option<(usize, String, String, String, String)> {
    let bytes = out.as_bytes();
    let mut from = 0usize;
    // Bound: every offset below is a byte offset within out (plus a few bytes), far below usize::MAX
    while let Some(i) = out[from..].find("<h") {
        let a = from.saturating_add(i);
        from = a.saturating_add(1);
        let attrs_start = a.saturating_add(3);
        let (Some(&digit), true) = (bytes.get(a.saturating_add(2)), attrs_start <= bytes.len()) else {
            continue;
        };
        if !(b'2'..=b'5').contains(&digit) {
            continue;
        }
        let tag = &out[a.saturating_add(1)..attrs_start];
        // `<h2([^>]*)>`
        let Some(gt_rel) = out[attrs_start..].find('>') else {
            continue;
        };
        let attrs_end = attrs_start.saturating_add(gt_rel);
        let attrs = &out[attrs_start..attrs_end];
        let inner_start = attrs_end.saturating_add(1);
        // the content runs to the first heading tag, which must be the matching closing tag
        let mut scan = inner_start;
        let mut close = None;
        while scan < out.len() {
            let Some(rel) = out[scan..].find('<') else {
                break;
            };
            let lt_at = scan.saturating_add(rel);
            let rest = &out[lt_at..];
            let is_heading = |text: &str| -> bool {
                let sb = text.as_bytes();
                // `<\/?h[1-6][\s>]`
                let mut pos = 1usize;
                if sb.get(pos) == Some(&b'/') {
                    pos = pos.saturating_add(1);
                }
                sb.get(pos) == Some(&b'h')
                    && sb.get(pos.saturating_add(1)).is_some_and(|c| (b'1'..=b'6').contains(c))
                    && text[pos.saturating_add(2)..].chars().next().is_some_and(|c| is_js_space(c) || c == '>')
            };
            if is_heading(rest) {
                if rest.starts_with(&format!("</{tag}>")) {
                    close = Some(lt_at);
                }
                break;
            }
            scan = lt_at.saturating_add(1);
        }
        let Some(cp) = close else {
            continue;
        };
        let inner = &out[inner_start..cp];
        let tail_start = cp.saturating_add(tag.len()).saturating_add(3);
        if let Some(()) = tail_matches(&out[tail_start..]) {
            return Some((a, tag.to_string(), attrs.to_string(), inner.to_string(), out[tail_start..].to_string()));
        }
    }
    None
}

/// `(?:\s|<div\b[^>]*>|<p class="note[^"]*">(?:(?!<\/?p[\s>])[\s\S])*?<\/p>)*$`: the tail reaches the end of the text.
fn tail_matches(t: &str) -> Option<()> {
    let mut i = 0usize;
    // Bound: every offset below is a byte offset within t, far below usize::MAX
    while i < t.len() {
        let rest = &t[i..];
        let c = rest.chars().next()?;
        if is_js_space(c) {
            i = i.saturating_add(c.len_utf8());
            continue;
        }
        if rest.starts_with("<div") && !rest[4..].chars().next().is_some_and(is_word) {
            let gt = rest.find('>')?;
            i = i.saturating_add(gt).saturating_add(1);
            continue;
        }
        if let Some(r) = rest.strip_prefix("<p class=\"note") {
            // `[^"]*">`
            let quote = r.find('"')?;
            if !r[quote..].starts_with("\">") {
                return None;
            }
            let content_start = quote.saturating_add(2);
            let body = &r[content_start..];
            // lazy content that holds no <p or </p followed by whitespace or '>', then </p>
            let mut scan = 0usize;
            let mut found = None;
            while scan < body.len() {
                let rel = body[scan..].find('<')?;
                let lt_at = scan.saturating_add(rel);
                let tag = &body[lt_at..];
                if tag.starts_with("</p>") {
                    found = Some(lt_at);
                    break;
                }
                let sb = tag.as_bytes();
                let mut pos = 1usize;
                if sb.get(pos) == Some(&b'/') {
                    pos = pos.saturating_add(1);
                }
                if sb.get(pos) == Some(&b'p') && tag[pos.saturating_add(1)..].chars().next().is_some_and(|c| is_js_space(c) || c == '>') {
                    return None;
                }
                scan = lt_at.saturating_add(1);
            }
            let close = found?;
            i = i
                .saturating_add("<p class=\"note".len())
                .saturating_add(content_start)
                .saturating_add(close)
                .saturating_add("</p>".len());
            continue;
        }
        return None;
    }
    Some(())
}

/// `numberForeign(body, title)`: table and figure numbers in another feature's section: a bare <table> takes the heading just
/// above it (or the section title) as its caption, and a <figcaption> without a number gets one.
pub fn number_foreign(body: &str, title: &str) -> String {
    // split on `<table\b[^>]*>`, `</table>` and `%%TNO(?::[^%]*)?%%`, keeping the separators
    let mut parts: Vec<&str> = Vec::new();
    let mut last = 0usize;
    let mut i = 0usize;
    let bytes = body.as_bytes();
    // Bound: offsets and token lengths within body, far below usize::MAX
    while i < bytes.len() {
        let rest = &body[i..];
        let tok = if rest.starts_with("<table") && !rest[6..].chars().next().is_some_and(is_word) {
            rest.find('>').map(|g| g.saturating_add(1))
        } else if rest.starts_with("</table>") {
            Some(8)
        } else if rest.starts_with("%%TNO") {
            tno_len(rest)
        } else {
            None
        };
        match tok {
            Some(n) => {
                let end = i.saturating_add(n);
                parts.push(&body[last..i]);
                parts.push(&body[i..end]);
                last = end;
                i = end;
            }
            None => i = i.saturating_add(rest.chars().next().map_or(1, char::len_utf8)),
        }
    }
    parts.push(&body[last..]);
    let mut out = String::new();
    let mut depth = 0usize;
    let mut pending = false;
    for p in parts {
        if p.starts_with("%%TNO") {
            pending = true;
            out.push_str(p);
            continue;
        }
        if p.starts_with("<table") && !p[6..].chars().next().is_some_and(is_word) {
            let was = depth;
            // Bound: a count of open tables in the text, far below usize::MAX
            depth = depth.saturating_add(1);
            if was == 0 && !pending {
                // Writing to a String cannot fail
                match heading_before(&out) {
                    Some((idx, tag, attrs, inner, tail)) if !inner.contains("class=\"tno\"") => {
                        out.truncate(idx);
                        let _ = write!(
                            out,
                            "<{tag}{attrs}><span class=\"tno\">{}</span>{}</{tag}>{tail}",
                            super::ctx::tno_token(&inner),
                            super::ctx::tno_rest(&inner)
                        );
                    }
                    _ => {
                        let _ = write!(
                            out,
                            "<p class=\"tcap\"><span class=\"tno\">%%TNO%%</span>{}</p>",
                            esc(if title.is_empty() { "Table" } else { title })
                        );
                    }
                }
            }
            pending = false;
            out.push_str(p);
            continue;
        }
        if p == "</table>" {
            depth = depth.saturating_sub(1);
            out.push_str(p);
            continue;
        }
        out.push_str(p);
    }
    number_figcaptions(&out)
}

/// The length of a `%%TNO(?::[^%]*)?%%` token at the start of the text.
fn tno_len(s: &str) -> Option<usize> {
    let r = s.strip_prefix("%%TNO")?;
    if r.starts_with("%%") {
        return Some(7);
    }
    let r2 = r.strip_prefix(':')?;
    let e = r2.find('%')?;
    // "%%TNO", ':', e bytes and "%%": 8 bytes besides e, which is an offset in s and far below usize::MAX
    if r2[e..].starts_with("%%") { Some(e.saturating_add(8)) } else { None }
}

/// `body.replace(/<figcaption([^>]*)>(?!\s*<span class="tno">)/g, '<figcaption$1><span class="tno">%%FNO%%</span>')`.
pub(crate) fn number_figcaptions(body: &str) -> String {
    // Bound: lengths and offsets within body, far below usize::MAX
    let mut out = String::with_capacity(body.len().saturating_add(32));
    let mut rest = body;
    while let Some(i) = rest.find("<figcaption") {
        out.push_str(&rest[..i]);
        let after = &rest[i.saturating_add("<figcaption".len())..];
        if let Some(g) = after.find('>') {
            let attrs = &after[..g];
            let tail = &after[g.saturating_add(1)..];
            let w = ws_end(tail, 0);
            let already = tail[w..].starts_with("<span class=\"tno\">");
            out.push_str("<figcaption");
            out.push_str(attrs);
            out.push('>');
            if !already {
                out.push_str("<span class=\"tno\">%%FNO%%</span>");
            }
            rest = tail;
        } else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// `foreignBody(body, d)`: another feature's HTML as a report part: bare tables numbered and captioned, and a source line when
/// it has none.
pub(crate) fn foreign_body(body: &str, id: &str, title: &str) -> String {
    let mut b = number_foreign(body, title);
    if b.contains("<table") && !has_source_line(&b) {
        // Writing to a String cannot fail
        let _ = write!(
            b,
            "<p class=\"src\">Source: Spare Construct's {} feature, from this project's inputs, loaded data and model runs (data vintage in the front matter; inputs in Appendix A).</p>",
            esc(id.split('-').next().unwrap_or(""))
        );
    }
    b
}

/// `mergedHtml(P, ctx, id)`: another feature's section printed inside one of the report's own, its tables numbered and sourced as
/// they would be on their own; empty when the feature prints nothing. `caption` is the title its bare tables take.
pub(crate) fn merged_html(_c: &mut Ctx, f: &ForeignSection, caption: &str) -> String {
    let mut body = f.html.clone();
    if js_trim(&body).is_empty() {
        return String::new();
    }
    if let Some(t) = f.title.as_deref().filter(|t| !t.is_empty()) {
        body = super::assemble::strip_leading_title(&body, t);
    }
    foreign_body(&body, &f.id, caption)
}

/// `lpTables(html, title)`: the trip generation tables of the land-use feature: 'T2 Person trips' headings become
/// 'Table T2 · Person trips' so they are numbered 'Table 3.4 (T2): Person trips'.
pub(crate) fn lp_tables(html: &str, title: &str) -> String {
    // /<h4>(T\d+b?)\s+([^<]*)<\/h4>/g
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    // Bound: offsets within html, far below usize::MAX
    while let Some(i) = rest.find("<h4>T") {
        out.push_str(&rest[..i]);
        let after = &rest[i.saturating_add(5)..];
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        let mut k = digits;
        let mut replaced = false;
        if digits > 0 {
            if after[k..].starts_with('b') {
                k = k.saturating_add(1);
            }
            let id_end = k;
            let ws = ws_end(after, k);
            if ws > k
                && let Some(lt) = after[ws..].find('<')
                && after[ws.saturating_add(lt)..].starts_with("</h4>")
            {
                let close = ws.saturating_add(lt);
                // Writing to a String cannot fail
                let _ = write!(out, "<h4>Table T{} \u{b7} {}</h4>", &after[..id_end], &after[ws..close]);
                rest = &after[close.saturating_add(5)..];
                replaced = true;
            }
        }
        if !replaced {
            out.push_str("<h4>T");
            rest = after;
        }
    }
    out.push_str(rest);
    foreign_body(&out, "landuse-tripgen", title)
}
