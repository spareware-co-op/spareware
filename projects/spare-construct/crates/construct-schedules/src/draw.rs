//! A schedule drawn on paper: the live table laid out as text and rules.
//!
//! [`draw`] takes a [`Table`] and gives what a sheet shows: the schedule's name, the column
//! headings, the group headers, the rows with their cells, the footers and the grand total, as
//! pieces of text at exact places and the lines between them. Places are lengths from the table's
//! top left corner, across and down, in grid steps ([`construct_params::Length`]); the sheet puts
//! the corner where the placement says. Nothing is stored: a sheet asks again each time, so it
//! always shows the schedule as it is.
//!
//! The layout is a stated rule, not a measurement of any typeface: rows are [`ROW_HEIGHT`] tall,
//! text is [`TEXT_HEIGHT`] high, and a column is as wide as its longest text at [`CHAR_WIDTH`] a
//! character plus [`PADDING`] each side. A page that draws the text in a typeface of its own fits
//! it to the column. The same table always gives the same drawing.

use crate::table::{Footer, Item, Table};
use construct_params::Length;

/// How tall a row is: 5 mm.
pub const ROW_HEIGHT: Length = Length::from_mm(5);
/// How high the text is: 2.5 mm.
pub const TEXT_HEIGHT: Length = Length::from_steps(16_000);
/// How wide a character is taken to be: 1.5 mm.
pub const CHAR_WIDTH: Length = Length::from_steps(9_600);
/// The space either side of a cell's text: 1 mm.
pub const PADDING: Length = Length::from_mm(1);

/// A piece of text at a place on the drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawnText {
    /// The left edge of the text, from the table's left edge.
    pub x: Length,
    /// The top of its row, from the table's top edge.
    pub y: Length,
    /// The text.
    pub text: String,
    /// Whether it is set heavier (the title, headings, headers and footers).
    pub bold: bool,
}

/// A straight line, horizontal or vertical, between two places.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawnRule {
    /// Where it starts, across.
    pub x1: Length,
    /// Where it starts, down.
    pub y1: Length,
    /// Where it ends, across.
    pub x2: Length,
    /// Where it ends, down.
    pub y2: Length,
}

/// A drawn schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drawn {
    /// How wide the table is.
    pub width: Length,
    /// How tall the table is.
    pub height: Length,
    /// The text, row by row.
    pub texts: Vec<DrawnText>,
    /// The lines.
    pub rules: Vec<DrawnRule>,
}

/// What a row of the drawing is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Line {
    /// The schedule's name, across the table.
    Title,
    /// A group's header, across the table.
    Group,
    /// The column headings.
    Heading,
    /// A row of the table.
    Body,
    /// A footer or the grand total.
    Foot,
}

struct Row {
    line: Line,
    cells: Vec<String>,
}

impl Row {
    fn spans(&self) -> bool {
        matches!(self.line, Line::Title | Line::Group)
    }
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn width_of(text: &str) -> Length {
    let chars = count(text.chars().count());
    Length::from_steps(chars.saturating_mul(CHAR_WIDTH.steps()))
}

fn add(a: Length, b: Length) -> Length {
    Length::from_steps(a.steps().saturating_add(b.steps()))
}

/// A footer's cells: each column's totals as `sum: 12`, joined; the footer's label leads the first
/// column's cell.
fn footer_cells(footer: &Footer, columns: usize) -> Vec<String> {
    let mut cells = vec![String::new(); columns];
    for total in &footer.totals {
        let (Some(cell), Some(shown)) = (cells.get_mut(total.column), total.shown.as_ref()) else {
            continue;
        };
        if !cell.is_empty() {
            cell.push_str("  ");
        }
        cell.push_str(total.kind.key());
        cell.push_str(": ");
        cell.push_str(shown);
    }
    let label = match footer.level {
        None => format!("Grand total ({})", footer.elements),
        Some(_) => format!(
            "{}: {} ({})",
            footer.name,
            footer.shown.as_deref().unwrap_or("none"),
            footer.elements
        ),
    };
    // The label leads the first cell, which may hold that column's totals as well.
    if let Some(first) = cells.first_mut() {
        *first = if first.is_empty() {
            label
        } else {
            format!("{label}  {first}")
        };
    }
    cells
}

/// The rows of the drawing: the title, the headings, each item and the grand total.
fn rows_of(table: &Table, title: &str, width_columns: usize) -> Vec<Row> {
    let counted = !table.itemize;
    let mut rows = vec![Row {
        line: Line::Title,
        cells: vec![title.to_owned()],
    }];
    let mut heading: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
    if counted {
        heading.push("Count".to_owned());
    }
    rows.push(Row {
        line: Line::Heading,
        cells: heading,
    });
    for item in &table.items {
        rows.push(match item {
            Item::Header(header) => Row {
                line: Line::Group,
                cells: vec![format!(
                    "{}: {}",
                    header.name,
                    header.shown.as_deref().unwrap_or("none")
                )],
            },
            Item::Row(row) => {
                let mut cells: Vec<String> = row
                    .cells
                    .iter()
                    .map(|cell| cell.shown.clone().unwrap_or_default())
                    .collect();
                if counted {
                    cells.push(row.count.to_string());
                }
                Row {
                    line: Line::Body,
                    cells,
                }
            }
            Item::Footer(footer) => Row {
                line: Line::Foot,
                cells: footer_cells(footer, width_columns),
            },
        });
    }
    if let Some(grand) = &table.grand {
        rows.push(Row {
            line: Line::Foot,
            cells: footer_cells(grand, width_columns),
        });
    }
    rows
}

/// Where each column begins across the table, and its right edge last: each column as wide as
/// its longest text, the last widened when a spanning row is wider than all of them.
fn column_edges(rows: &[Row], width_columns: usize) -> Vec<Length> {
    let mut widths: Vec<Length> = vec![Length::from_steps(0); width_columns.max(1)];
    for row in rows.iter().filter(|row| !row.spans()) {
        for (width, text) in widths.iter_mut().zip(&row.cells) {
            let need = add(width_of(text), add(PADDING, PADDING));
            if need > *width {
                *width = need;
            }
        }
    }
    let spanning = rows
        .iter()
        .filter(|row| row.spans())
        .map(|row| {
            row.cells.first().map_or(Length::from_steps(0), |text| {
                add(width_of(text), add(PADDING, PADDING))
            })
        })
        .max()
        .unwrap_or(Length::from_steps(0));
    let across = widths
        .iter()
        .fold(Length::from_steps(0), |total, width| add(total, *width));
    if spanning > across
        && let Some(last) = widths.last_mut()
    {
        *last = add(
            *last,
            Length::from_steps(spanning.steps().saturating_sub(across.steps())),
        );
    }
    let mut edges = vec![Length::from_steps(0)];
    for width in &widths {
        let last = edges.last().copied().unwrap_or(Length::from_steps(0));
        edges.push(add(last, *width));
    }
    edges
}

/// The drawing of a table under a title.
#[must_use]
pub fn draw(table: &Table, title: &str) -> Drawn {
    let width_columns = table
        .columns
        .len()
        .saturating_add(usize::from(!table.itemize));
    let rows = rows_of(table, title, width_columns);

    let edges = column_edges(&rows, width_columns);
    let wide = edges.last().copied().unwrap_or(Length::from_steps(0));
    let tall = Length::from_steps(count(rows.len()).saturating_mul(ROW_HEIGHT.steps()));

    let mut texts = Vec::new();
    let mut rules = Vec::new();
    let mut run: Option<Length> = None;
    let flush = |run: &mut Option<Length>, until: Length, rules: &mut Vec<DrawnRule>| {
        if let Some(from) = run.take() {
            for edge in &edges {
                rules.push(DrawnRule {
                    x1: *edge,
                    y1: from,
                    x2: *edge,
                    y2: until,
                });
            }
        }
    };
    for (at, row) in rows.iter().enumerate() {
        let top = Length::from_steps(count(at).saturating_mul(ROW_HEIGHT.steps()));
        let bottom = add(top, ROW_HEIGHT);
        let bold = row.line != Line::Body;
        if row.spans() {
            flush(&mut run, top, &mut rules);
            if let Some(text) = row.cells.first() {
                texts.push(DrawnText {
                    x: PADDING,
                    y: top,
                    text: text.clone(),
                    bold,
                });
            }
        } else {
            run.get_or_insert(top);
            for (edge, text) in edges.iter().zip(&row.cells) {
                if !text.is_empty() {
                    texts.push(DrawnText {
                        x: add(*edge, PADDING),
                        y: top,
                        text: text.clone(),
                        bold,
                    });
                }
            }
        }
        rules.push(DrawnRule {
            x1: Length::from_steps(0),
            y1: bottom,
            x2: wide,
            y2: bottom,
        });
    }
    flush(&mut run, tall, &mut rules);
    rules.push(DrawnRule {
        x1: Length::from_steps(0),
        y1: Length::from_steps(0),
        x2: wide,
        y2: Length::from_steps(0),
    });
    Drawn {
        width: wide,
        height: tall,
        texts,
        rules,
    }
}
