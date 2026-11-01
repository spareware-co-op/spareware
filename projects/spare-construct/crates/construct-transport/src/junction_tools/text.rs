//! Cell and number formats of the junction tables (the `formatting` block of junctions.js) and the kit's small HTML helpers.

use super::{FIGURE_STEP, LABEL, STORAGE_AMBER, STORAGE_RED};
use crate::edit::CONTROLS;
use crate::jsnum::{math_round, number_to_js_string, to_fixed};
use crate::util::{esc, fmt};

/// `n0(x)`: a whole number, or an en dash.
pub fn n0(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => fmt::int(v),
        _ => fmt::DASH.to_string(),
    }
}

/// `d1(d)`: a delay in seconds to 1 decimal, "> 300" above 300, or an en dash.
pub fn d1(d: Option<f64>) -> String {
    match d {
        Some(v) if v.is_finite() => {
            if v > 300.0 {
                "> 300".to_string()
            } else {
                to_fixed(v, 1)
            }
        }
        _ => fmt::DASH.to_string(),
    }
}

/// `x2(x)`: a ratio to 2 decimals, or an en dash.
pub fn x2(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => to_fixed(v, 2),
        _ => fmt::DASH.to_string(),
    }
}

/// `p1(x)`: a number to 1 decimal, or an en dash.
pub fn p1(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => to_fixed(v, 1),
        _ => fmt::DASH.to_string(),
    }
}

/// `sgn(x, u, dp)`: a signed change with its unit, or an en dash when it is missing or rounds to nothing.
pub fn sgn(x: Option<f64>, u: &str, dp: usize) -> String {
    match x {
        Some(v) if v.is_finite() && v.abs() >= crate::jsmath::pow(10.0, -(dp as f64)) / 2.0 => {
            format!(
                "{}{}{}",
                if v > 0.0 { "+" } else { "\u{2212}" },
                to_fixed(v.abs(), dp),
                u
            )
        }
        _ => fmt::DASH.to_string(),
    }
}

/// `dosCol(dos, xp)`: the colour of a degree of saturation against its practical limit, or none.
pub fn dos_col(dos: Option<f64>, xp: Option<f64>) -> &'static str {
    match (dos, xp) {
        (Some(d), Some(x)) => {
            if d > 1.0 {
                "var(--danger)"
            } else if d > x {
                "var(--warn)"
            } else {
                "var(--ok)"
            }
        }
        _ => "",
    }
}

/// `dosCell(dos, xp)`: a degree of saturation, coloured against its practical limit.
pub fn dos_cell(dos: Option<f64>, xp: Option<f64>) -> String {
    let c = dos_col(dos, xp);
    if c.is_empty() {
        x2(dos)
    } else {
        format!(
            "<span style=\"color:{};font-weight:600\">{}</span>",
            c,
            x2(dos)
        )
    }
}

/// `rqCell(rq, bay)`: a queue storage ratio, coloured amber or red with a hint.
pub fn rq_cell(rq: Option<f64>, bay: bool) -> String {
    let Some(rq) = rq.filter(|v| v.is_finite()) else {
        return fmt::DASH.to_string();
    };
    let c = if rq > STORAGE_RED {
        "var(--danger)"
    } else if rq > STORAGE_AMBER {
        "var(--warn)"
    } else {
        ""
    };
    let t = if rq > STORAGE_RED {
        if bay {
            "Queue overflows the bay"
        } else {
            "Queue blocks the upstream junction"
        }
    } else if rq > STORAGE_AMBER {
        "Queue fills most of the available storage"
    } else {
        ""
    };
    format!(
        "<span{}{}>{}</span>",
        if c.is_empty() {
            String::new()
        } else {
            format!(" style=\"color:{c};font-weight:600\"")
        },
        if t.is_empty() {
            String::new()
        } else {
            format!(" title=\"{t}\"")
        },
        to_fixed(rq, 2)
    )
}

/// `r5(v)`: a figure rounded to the nearest 5 veh/h, "<5" for a small positive flow.
pub fn r5(v: f64) -> String {
    let s = FIGURE_STEP;
    if v > 0.0 && v < s / 2.0 {
        "<5".to_string()
    } else {
        number_to_js_string(math_round(v / s) * s)
    }
}

/// `ctlName(t)`: the name of a control, the control itself when unknown, or "Uncontrolled".
pub fn ctl_name(t: &str) -> String {
    for (k, v) in CONTROLS {
        if k == t {
            return v.to_string();
        }
    }
    if t.is_empty() {
        "Uncontrolled".to_string()
    } else {
        t.to_string()
    }
}

/// `perName(p)`: the name of a period.
pub fn per_name(p: &str) -> String {
    match p {
        "AM" => "AM peak".to_string(),
        "PM" => "PM peak".to_string(),
        "IP" => "Inter-peak".to_string(),
        "SAT" => "Saturday peak".to_string(),
        _ => format!("{p} period"),
    }
}

/// `Kit.losBadge(l)`: a level of service as a badge; an en dash badge for none.
pub fn los_badge(l: Option<&str>) -> String {
    match l {
        Some(l) if !l.is_empty() && l != fmt::DASH => {
            format!("<span class=\"los {}\">{}</span>", esc(l), esc(l))
        }
        _ => "<span class=\"los none\">\u{2013}</span>".to_string(),
    }
}

/// `losText(l)`: the level of service or an en dash.
pub fn los_text(l: Option<&str>) -> String {
    match l {
        Some(l) if !l.is_empty() && l != fmt::DASH => l.to_string(),
        _ => fmt::DASH.to_string(),
    }
}

/// A column heading of [`kit_table`].
#[derive(Clone, Debug, Default)]
pub struct Head {
    /// Heading HTML.
    pub label: String,
    /// Right-aligned numeric column.
    pub n: bool,
}

impl Head {
    /// A text heading.
    pub fn text(label: &str) -> Head {
        Head {
            label: label.to_string(),
            n: false,
        }
    }
    /// A numeric heading.
    pub fn num(label: &str) -> Head {
        Head {
            label: label.to_string(),
            n: true,
        }
    }
}

/// A row of [`kit_table`]: cells (HTML) and, for a clickable row, what it flies to.
#[derive(Clone, Debug, Default)]
pub struct TableRow {
    /// Cells as HTML.
    pub cells: Vec<String>,
    /// Fly target, for a clickable row.
    pub fly: Option<String>,
}

/// `Kit.table({ head, rows })`: the compact results table of the interface kit.
pub fn kit_table(head: &[Head], rows: &[TableRow]) -> String {
    let th = if head.is_empty() {
        String::new()
    } else {
        format!(
            "<tr>{}</tr>",
            head.iter()
                .map(|h| format!(
                    "<th{}>{}</th>",
                    if h.n { " class=\"n\"" } else { "" },
                    h.label
                ))
                .collect::<String>()
        )
    };
    let tr: String = rows
        .iter()
        .map(|r| {
            let attrs = match &r.fly {
                Some(f) => format!(" class=\"click \" data-fly=\"{}\"", esc(f)),
                None => String::new(),
            };
            let cells: String = r
                .cells
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let n = head.get(i).is_some_and(|h| h.n);
                    let inner = if r.fly.is_some() && i == 0 {
                        format!("<button type=\"button\" class=\"rowlink\">{c}</button>")
                    } else {
                        c.clone()
                    };
                    format!("<td{}>{}</td>", if n { " class=\"n\"" } else { "" }, inner)
                })
                .collect();
            format!("<tr{attrs}>{cells}</tr>")
        })
        .collect();
    format!("<div class=\"tbl-wrap\"><table class=\"t compact\">{th}{tr}</table></div>")
}

/// `esc(LABEL)` and friends: the junction label.
pub fn label() -> &'static str {
    LABEL
}
