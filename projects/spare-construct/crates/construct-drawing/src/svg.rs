//! A drawing as SVG, so a person can see it.
//!
//! What it does: [`to_svg`] writes a drawing as one SVG document, by hand in a few lines (SVG is
//! plain text, so no library is needed). Coordinates are model millimetres, the view's x to the
//! right and its y up the sheet (SVG's y runs down, so it is written negated); the document's
//! `width` and `height` are the paper size at the scale given, in millimetres, with a margin of
//! 5 mm of paper all round. Each line is an SVG `line` (a segment) or `path` (an arc), drawn in its
//! pen's colour at its pen's printed width at that scale ([`crate::pens`]), carrying its element
//! and its face's role as `data-element` and `data-role`. Regions are not filled yet (note A12's
//! step 4), and line patterns are drawn solid.
//!
//! Numbers: a length of the drawing in millimetres is `steps / 6400`, a terminating decimal, and is
//! written exactly; so is a point of the paper size at a scale whose denominator has no prime
//! factor but 2 and 5 (1:50, 1:100, 1:200). Any other value, an arc's radius or a fraction of a
//! step from a sloped cut, is written to the nearest nanometre, the one rounding of an export
//! (chapter 18.3 of the geometry architecture). The same drawing and scale always give the same
//! bytes.

use std::fmt::Write as _;

use construct_styles::Scale;

use crate::DrawingError;
use crate::drawing::{Drawing, Shape, Turn, ViewPoint};
use crate::pens::{Styles, style};

/// `num / den` as a decimal: exact when it terminates, else to nine places.
fn decimal(num: i128, den: i128) -> Result<String, DrawingError> {
    if den <= 0 {
        return Err(DrawingError::BadPoint);
    }
    let negative = num < 0;
    let n = num.unsigned_abs();
    let d = den.unsigned_abs();
    let whole = n.checked_div(d).ok_or(DrawingError::TooBig)?;
    let mut rest = n.checked_rem(d).ok_or(DrawingError::TooBig)?;
    let mut digits = String::new();
    for _ in 0..9 {
        if rest == 0 {
            break;
        }
        rest = rest.checked_mul(10).ok_or(DrawingError::TooBig)?;
        let digit = rest.checked_div(d).ok_or(DrawingError::TooBig)?;
        rest = rest.checked_rem(d).ok_or(DrawingError::TooBig)?;
        digits.push(char::from_digit(u32::try_from(digit).unwrap_or(0), 10).unwrap_or('0'));
    }
    let mut whole = whole;
    if rest != 0 {
        // Nearest at nine places, ties away from zero: add one in the last place when the rest is
        // at least half.
        let twice = rest.checked_mul(2).ok_or(DrawingError::TooBig)?;
        if twice >= d {
            let mut bytes: Vec<u8> = digits.into_bytes();
            let mut carry = true;
            for b in bytes.iter_mut().rev() {
                if !carry {
                    break;
                }
                if *b == b'9' {
                    *b = b'0';
                } else {
                    *b = b.saturating_add(1);
                    carry = false;
                }
            }
            if carry {
                whole = whole.checked_add(1).ok_or(DrawingError::TooBig)?;
            }
            digits = String::from_utf8(bytes).map_err(|_| DrawingError::TooBig)?;
        }
    }
    let digits = digits.trim_end_matches('0');
    let sign = if negative && (whole != 0 || !digits.is_empty()) {
        "-"
    } else {
        ""
    };
    Ok(if digits.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{digits}")
    })
}

/// Grid steps in a millimetre, as the drawing's coordinates count them: construct-params'
/// `Length::STEPS_PER_MM` (a test holds them equal).
pub const PER_MM: i128 = 6_400;

/// `v / den` steps in millimetres, as text.
fn mm(v: i128, den: i128) -> Result<String, DrawingError> {
    decimal(v, den.checked_mul(PER_MM).ok_or(DrawingError::TooBig)?)
}

/// A point's x and its SVG y (negated), in millimetres.
fn xy(p: ViewPoint) -> Result<(String, String), DrawingError> {
    Ok((
        mm(p.x, p.den)?,
        mm(p.y.checked_neg().ok_or(DrawingError::TooBig)?, p.den)?,
    ))
}

/// A point as millimetres in floating point, for an arc's flags and radius only.
#[allow(
    clippy::cast_precision_loss,
    reason = "display only: an arc's radius and which way round it goes"
)]
fn approx(p: ViewPoint) -> (f64, f64) {
    let d = p.den as f64 * PER_MM as f64;
    (p.x as f64 / d, p.y as f64 / d)
}

/// The least and greatest of the drawing's points along each axis, as `(num, den)`.
fn extent(drawing: &Drawing) -> Result<Option<[ViewPoint; 2]>, DrawingError> {
    let mut points = Vec::new();
    for line in drawing.lines() {
        match line.shape {
            Shape::Segment { start, end } => points.extend([start, end]),
            Shape::Arc {
                centre, start, end, ..
            } => points.extend([centre, start, end]),
        }
    }
    let less = |a: i128, ad: i128, b: i128, bd: i128| -> Result<bool, DrawingError> {
        Ok(a.checked_mul(bd).ok_or(DrawingError::TooBig)?
            < b.checked_mul(ad).ok_or(DrawingError::TooBig)?)
    };
    let mut it = points.into_iter();
    let Some(first) = it.next() else {
        return Ok(None);
    };
    let (mut lo, mut hi) = (
        (first.x, first.den, first.y, first.den),
        (first.x, first.den, first.y, first.den),
    );
    for p in it {
        if less(p.x, p.den, lo.0, lo.1)? {
            (lo.0, lo.1) = (p.x, p.den);
        }
        if less(p.y, p.den, lo.2, lo.3)? {
            (lo.2, lo.3) = (p.y, p.den);
        }
        if less(hi.0, hi.1, p.x, p.den)? {
            (hi.0, hi.1) = (p.x, p.den);
        }
        if less(hi.2, hi.3, p.y, p.den)? {
            (hi.2, hi.3) = (p.y, p.den);
        }
    }
    // Over one denominator per corner: x / d and y / d.
    let corner = |(x, xd, y, yd): (i128, i128, i128, i128)| -> Result<ViewPoint, DrawingError> {
        ViewPoint::new(
            x.checked_mul(yd).ok_or(DrawingError::TooBig)?,
            y.checked_mul(xd).ok_or(DrawingError::TooBig)?,
            xd.checked_mul(yd).ok_or(DrawingError::TooBig)?,
        )
    };
    Ok(Some([corner(lo)?, corner(hi)?]))
}

/// The document's opening tag: the paper size at 1:`n` and the box round the drawing in model
/// millimetres, grown by 5 mm of paper on every side.
fn opening(drawing: &Drawing, n: i128) -> Result<String, DrawingError> {
    let big = || DrawingError::TooBig;
    // The margin: 5 mm of paper, in steps of the model.
    let margin = n
        .checked_mul(5)
        .and_then(|m| m.checked_mul(PER_MM))
        .ok_or_else(big)?;
    let [lo, hi] = extent(drawing)?.unwrap_or([ViewPoint::grid(0, 0), ViewPoint::grid(0, 0)]);
    let both = lo.den.checked_mul(hi.den).ok_or_else(big)?;
    let twice = margin
        .checked_mul(2)
        .and_then(|m| m.checked_mul(both))
        .ok_or_else(big)?;
    // Over the denominator `both`: the far corner less the near one, and both margins.
    let span = |far: i128, near: i128| {
        far.checked_mul(lo.den)
            .zip(near.checked_mul(hi.den))
            .and_then(|(f, m)| f.checked_sub(m))
            .and_then(|d| d.checked_add(twice))
            .ok_or_else(big)
    };
    let size = ViewPoint::new(span(hi.x, lo.x)?, span(hi.y, lo.y)?, both)?;
    // The box's corner in SVG's frame: x from lo.x, y from -hi.y, less the margin.
    let less = |v: i128, den: i128| {
        margin
            .checked_mul(den)
            .and_then(|m| v.checked_sub(m))
            .ok_or_else(big)
    };
    let left = less(lo.x, lo.den)?;
    let top = less(hi.y.checked_neg().ok_or_else(big)?, hi.den)?;
    let paper = |v: i128| {
        size.den
            .checked_mul(n)
            .ok_or_else(big)
            .and_then(|d| mm(v, d))
    };
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"{} {} {} {}\">",
        paper(size.x)?,
        paper(size.y)?,
        mm(left, lo.den)?,
        mm(top, hi.den)?,
        mm(size.x, size.den)?,
        mm(size.y, size.den)?,
    ))
}

/// An arc's SVG path data: its radius, and the flags for which way and how far round it goes,
/// from the floating-point values of its points (display only).
fn arc(
    centre: ViewPoint,
    start: ViewPoint,
    end: ViewPoint,
    turn: Turn,
) -> Result<String, DrawingError> {
    let ((x1, y1), (x2, y2)) = (xy(start)?, xy(end)?);
    let (c, a, b) = (approx(centre), approx(start), approx(end));
    let radius = (a.0 - c.0).hypot(a.1 - c.1);
    let from = (a.1 - c.1).atan2(a.0 - c.0);
    let to = (b.1 - c.1).atan2(b.0 - c.0);
    let mut sweep = match turn {
        Turn::Counterclockwise => to - from,
        Turn::Clockwise => from - to,
    };
    if sweep <= 0.0 {
        sweep += std::f64::consts::TAU;
    }
    let large = u8::from(sweep > std::f64::consts::PI);
    // The sheet's y is negated, so counter-clockwise on the drawing is SVG's positive sweep.
    let flag = u8::from(turn == Turn::Counterclockwise);
    Ok(format!(
        "M {x1} {y1} A {radius:.9} {radius:.9} 0 {large} {flag} {x2} {y2}"
    ))
}

/// `drawing` as an SVG document at `scale`, each line at its pen's printed width there.
///
/// # Errors
///
/// What [`crate::pens::style`] gives for a line it cannot style, and [`DrawingError::TooBig`]
/// for a number past 128 bits.
pub fn to_svg(drawing: &Drawing, styles: Styles<'_>, scale: Scale) -> Result<String, DrawingError> {
    let n = i128::from(scale.denominator());
    let mut out = opening(drawing, n)?;
    out.push('\n');
    for line in drawing.lines() {
        let pen = style(styles, line, scale)?;
        // Printed micrometres at 1:n are n micrometres of the model each: µm × n / 1000 mm.
        let width = i128::from(pen.width.get())
            .checked_mul(n)
            .ok_or(DrawingError::TooBig)?;
        let stroke = format!(
            "stroke=\"#{:02x}{:02x}{:02x}\" stroke-width=\"{}\" stroke-linecap=\"round\"",
            pen.colour.0,
            pen.colour.1,
            pen.colour.2,
            decimal(width, 1000)?
        );
        let data = format!(
            "data-element=\"{}\" data-role=\"{}\"",
            line.source.element.as_str(),
            line.source.role
        );
        match line.shape {
            Shape::Segment { start, end } => {
                let ((x1, y1), (x2, y2)) = (xy(start)?, xy(end)?);
                let _ = writeln!(
                    out,
                    "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" {stroke} {data}/>"
                );
            }
            Shape::Arc {
                centre,
                start,
                end,
                turn,
            } => {
                let d = arc(centre, start, end, turn)?;
                let _ = writeln!(out, "<path d=\"{d}\" fill=\"none\" {stroke} {data}/>");
            }
        }
    }
    out.push_str("</svg>\n");
    Ok(out)
}
