//! The small HTML builders the compliance report sections and the shared table helper (`Kit.table`) use. Every function returns
//! the markup the prototype writes, character for character; text that came from data is escaped by the caller (as the prototype
//! does with `esc`), markup passed in is written as it is.

use crate::util::esc as esc_text;

/// A report context the report lane passes to a section (`ctx.table(title, html, { note })`); without one a table is a plain figure.
pub trait ReportCtx {
    /// The report lane's table block with a caption and a source note.
    fn table(&self, title: &str, html: &str, note: &str) -> String;
}

/// `rT(ctx, title, tblHtml, noteText)` of the Compliance+ report sections: the table in the report lane's block, or a figure with a caption.
pub fn rt(ctx: Option<&dyn ReportCtx>, title: &str, tbl: &str, note_text: &str) -> String {
    match ctx {
        Some(c) => c.table(title, tbl, note_text),
        None => format!(
            "<figure class=\"tbl\"><figcaption>{title}</figcaption>{tbl}{}</figure>",
            if note_text.is_empty() {
                String::new()
            } else {
                format!("<p class=\"src\">{note_text}</p>")
            }
        ),
    }
}

/// A column heading of [`kit_table`]: its text (markup) and whether the column is numeric (right-aligned).
#[derive(Clone, Debug, Default)]
pub struct Th {
    /// The heading, written as given.
    pub label: String,
    /// A numeric column (`n`).
    pub n: bool,
}

/// A text heading.
pub fn th(label: &str) -> Th {
    Th {
        label: label.to_string(),
        n: false,
    }
}

/// A numeric column heading.
pub fn thn(label: &str) -> Th {
    Th {
        label: label.to_string(),
        n: true,
    }
}

/// A row of [`kit_table`]: the cells (markup), an optional fly-to target, a class and the cell the fly-to link wraps.
#[derive(Clone, Debug, Default)]
pub struct Tr {
    /// The cells, written as given.
    pub cells: Vec<String>,
    /// `fly`: the row flies the map to this target (`link:l1`, `node:n2`).
    pub fly: Option<String>,
    /// `cls`: a class for the row.
    pub cls: String,
    /// `flyCol`: the column whose cell becomes a link (0 when not given).
    pub fly_col: usize,
}

/// A plain row.
pub fn tr<S: Into<String>>(cells: impl IntoIterator<Item = S>) -> Tr {
    Tr {
        cells: cells.into_iter().map(Into::into).collect(),
        ..Default::default()
    }
}

/// `Kit.table({ head, rows, compact, cls, empty })`: a table wrapped for scrolling; with no rows, the `empty` text in one cell (or nothing).
pub fn kit_table(head: &[Th], rows: &[Tr], compact: bool, cls: &str, empty: &str) -> String {
    let thc = |h: &Th| if h.n { "n" } else { "" };
    let th_row = if head.is_empty() {
        String::new()
    } else {
        format!(
            "<tr>{}</tr>",
            head.iter()
                .map(|h| if thc(h).is_empty() {
                    format!("<th>{}</th>", h.label)
                } else {
                    format!("<th class=\"{}\">{}</th>", thc(h), h.label)
                })
                .collect::<String>()
        )
    };
    let body: String = rows
        .iter()
        .map(|o| {
            let attrs = format!(
                "{}{}",
                if o.fly.is_some() || !o.cls.is_empty() {
                    format!(
                        " class=\"{}{}\"",
                        if o.fly.is_some() { "click " } else { "" },
                        esc_text(&o.cls)
                    )
                } else {
                    String::new()
                },
                o.fly
                    .as_ref()
                    .map_or(String::new(), |f| format!(" data-fly=\"{}\"", esc_text(f)))
            );
            let cells: String = o
                .cells
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let td = if head.get(i).is_some_and(|h| h.n) {
                        "<td class=\"n\">"
                    } else {
                        "<td>"
                    };
                    if o.fly.is_some() && i == o.fly_col {
                        format!("{td}<button type=\"button\" class=\"rowlink\">{c}</button></td>")
                    } else {
                        format!("{td}{c}</td>")
                    }
                })
                .collect();
            format!("<tr{attrs}>{cells}</tr>")
        })
        .collect();
    let body = if body.is_empty() && !empty.is_empty() {
        format!(
            "<tr><td class=\"note\" colspan=\"{}\">{empty}</td></tr>",
            head.len().max(1)
        )
    } else {
        body
    };
    format!(
        "<div class=\"tbl-wrap\"><table class=\"t{}{}\">{th_row}{body}</table></div>",
        if compact { " compact" } else { "" },
        if cls.is_empty() {
            String::new()
        } else {
            format!(" {}", esc_text(cls))
        }
    )
}

/// The report tables of the Compliance module (`T(head, rows)`): headings are escaped, cells are written as given.
pub fn report_table(head: &[&str], rows: &[Vec<String>]) -> String {
    format!(
        "<table class=\"rt\"><thead><tr>{}</tr></thead><tbody>{}</tbody></table>",
        head.iter()
            .map(|h| format!("<th>{}</th>", esc_text(h)))
            .collect::<String>(),
        rows.iter()
            .map(|r| format!(
                "<tr>{}</tr>",
                r.iter()
                    .map(|c| format!("<td>{c}</td>"))
                    .collect::<String>()
            ))
            .collect::<String>()
    )
}

/// `P_(t)`: an escaped paragraph.
pub fn p_(t: &str) -> String {
    format!("<p>{}</p>", esc_text(t))
}

/// The pill of the Compliance module: tone "warn" and "bad" use the proposed and closed classes, "ok" is green.
pub fn pill(text: &str, tone: &str) -> String {
    format!(
        "<span class=\"pill{}\"{}>{text}</span>",
        match tone {
            "warn" => " proposed",
            "bad" => " closed",
            _ => "",
        },
        if tone == "ok" {
            " style=\"color:var(--ok)\""
        } else {
            ""
        }
    )
}

/// The pill of Compliance+: tones map onto the classes the stylesheet defines (ok, warn, bad).
pub fn pill_x(text: &str, tone: &str) -> String {
    format!(
        "<span class=\"pill{}\">{text}</span>",
        match tone {
            "ok" => " conv ok",
            "warn" => " proposed",
            "bad" => " closed",
            _ => "",
        }
    )
}

/// `note(t, warn)` of Compliance+: a note paragraph (the text is markup).
pub fn note(t: &str, warn: bool) -> String {
    format!(
        "<p class=\"note{}\">{t}</p>",
        if warn { " warn" } else { "" }
    )
}
