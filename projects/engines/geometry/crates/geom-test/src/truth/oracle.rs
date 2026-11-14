//! The analytic cases of `SolidCraft`'s oracle, as a table of formulas derived in enclosures.
//!
//! What it does: [`cases`] reads `data/solidcraft-analytic.json`, one [`Case`] for each of the 70
//! parts whose expected values `SolidCraft` works out in closed form: the shape in words, its
//! parameters, the plane regions its sections are made of, and a formula for each of its volume,
//! area, centre of mass and bounding box. [`derive`] evaluates the formulas as [`Enclosed`] values:
//! a small parser reads `+`, `-`, `*`, `/`, whole powers with `^`, brackets, decimals as exact
//! rationals, `pi`, and `sqrt`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan` and `atan2`. A region is
//! a list of loops of lines (`L`), circular arcs (`A`), closed polygons (`P`), rectangles (`R`) and
//! annular sectors (`S`); [`region_moments`] gives its area, its first and mixed moments, its
//! perimeter and its perimeter's first moment as the names `<region>_A`, `_Sx`, `_Sy`, `_Sxy`, `_P`
//! and `_Px`, for the formulas to use. A case that needs a kernel operation not yet built names the
//! board piece in `needs`, and a quantity that has no closed form is left out of its table row.
//!
//! Guarantees:
//!
//! - **Every value is derived.** Each number comes from the formulas of the table in rational
//!   arithmetic that rounds outward; no stored value of `SolidCraft` is read here, and none is the
//!   truth.
//! - **A formula that cannot be evaluated is an error, never a guess.** An unknown name, a quotient
//!   by a value that may be zero, a root of a negative number or a stray character gives an `Err`
//!   naming the case.
//! - **Regions are exact.** Lines and arcs are integrated in closed form (Green's theorem), so a
//!   region's moments are enclosures of the exact values, not sums of samples.
//!
//! Method: chapter 28.9's closed-form truth
//! (`projects/engines/geometry/architecture/28-testing.md`); the areas and moments of a region from
//! Green's theorem over its boundary, ∮ x dy for the area and ∮ x²/2 dy, ∮ xy dy and ∮ x²y/2 dy for
//! its moments, the arcs integrated as polynomials in sine and cosine; solids of revolution and
//! swept discs by Pappus's theorems; and the shapes, parameters and formulas of `SolidCraft`'s
//! analytic scripts, `tools/oracle-analytic/cases_*.py`, re-derived here from their method strings
//! (<https://github.com/tangency-labs/solidcraft/tree/6029a38ea05da8f37517da6b06624a8bec18adf5/tools/oracle-analytic>).
//!
//! Cost: a case is a few hundred big-number operations for each arc and formula, and a few
//! thousand series terms for each angle. Recursion: the parser nests as deep as the formula's
//! brackets, a few levels.
//!
//! Consulted: `SolidCraft`, Apache-2.0,
//!   <https://github.com/tangency-labs/solidcraft/tree/6029a38ea05da8f37517da6b06624a8bec18adf5/tools/oracle-analytic>,
//!   for the shapes, parameters and method formulas of its 70 analytic oracle cases, which are
//!   re-derived here; its stored values are used only to cross-check the derivation. No code
//!   copied.

#![allow(
    clippy::arithmetic_side_effects,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::too_many_lines,
    reason = "exact rational arithmetic, which cannot overflow; the region integrals keep the \
              letters of their formulas"
)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use dashu_int::{IBig, UBig};
use dashu_ratio::RBig;
use serde::Deserialize;

use super::{Enclosed, acos, asin, atan, atan2, cos, pi, sin, tan};

/// The commit of `SolidCraft` the table's shapes and formulas come from.
pub const COMMIT: &str = "6029a38ea05da8f37517da6b06624a8bec18adf5";

/// A plane region of a case: loops of segments, outer loops counterclockwise, holes clockwise.
/// Each segment is a kind (`L`, `A`, `P`, `R` or `S`) and its formulas.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionSpec {
    /// The prefix of the names the region's integrals take.
    pub name: String,
    /// The loops, each a list of segments, each a kind and its formulas.
    pub loops: Vec<Vec<Vec<String>>>,
}

/// One analytic case: a part, its parameters and the formulas of what is known of it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// The case's name in `SolidCraft`'s oracle.
    pub name: String,
    /// The part in words.
    pub shape: String,
    /// `kernel` if the kernel can be held to it today, else the board piece that brings the
    /// operation that builds it.
    pub needs: String,
    /// Names and their formulas, in order; a formula may use the names before it.
    #[serde(default)]
    pub params: Vec<[String; 2]>,
    /// The plane regions, after the parameters.
    #[serde(default)]
    pub regions: Vec<RegionSpec>,
    /// Further names and formulas, after the regions, which they may use.
    #[serde(default)]
    pub lets: Vec<[String; 2]>,
    /// The volume.
    pub volume: Option<String>,
    /// The surface area.
    pub area: Option<String>,
    /// The centre of mass.
    pub com: [Option<String>; 3],
    /// The lower corner of the bounding box.
    pub min: [Option<String>; 3],
    /// The upper corner of the bounding box.
    pub max: [Option<String>; 3],
}

/// What a case derives to: each quantity the table gives a formula for.
#[derive(Clone, Debug)]
pub struct Derived {
    /// The volume.
    pub volume: Option<Enclosed>,
    /// The surface area.
    pub area: Option<Enclosed>,
    /// The centre of mass.
    pub com: [Option<Enclosed>; 3],
    /// The lower corner of the bounding box.
    pub min: [Option<Enclosed>; 3],
    /// The upper corner of the bounding box.
    pub max: [Option<Enclosed>; 3],
}

impl Case {
    /// Whether the table derives the case's volume, area and whole centre of mass.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.volume.is_some() && self.area.is_some() && self.com.iter().all(Option::is_some)
    }
}

/// The table, read from `data/solidcraft-analytic.json`.
///
/// # Errors
///
/// A message if the file does not read as a list of cases.
pub fn cases() -> Result<Vec<Case>, String> {
    serde_json::from_str(include_str!("../../data/solidcraft-analytic.json"))
        .map_err(|e| format!("the oracle table: {e}"))
}

/// π, computed once.
fn pi_once() -> &'static Enclosed {
    static PI: OnceLock<Enclosed> = OnceLock::new();
    PI.get_or_init(pi)
}

/// A decimal literal as the exact rational it says: `2.5` is 5/2.
fn decimal(text: &str) -> Result<RBig, String> {
    let (head, tail) = text.split_once('.').unwrap_or((text, ""));
    let digits: IBig = format!("{head}{tail}")
        .parse()
        .map_err(|_| format!("`{text}` is not a number"))?;
    let mut scale = UBig::ONE;
    for _ in 0..tail.len() {
        scale *= UBig::from(10_u8);
    }
    Ok(RBig::from_parts(digits, scale))
}

/// The names a formula may use.
#[derive(Default)]
struct Scope {
    names: BTreeMap<String, Enclosed>,
}

/// A formula being read: its characters and where the reader is.
struct Parser<'a> {
    text: Vec<char>,
    at: usize,
    scope: &'a Scope,
}

/// The value of the formula `text`, with the names of `scope`.
fn evaluate(text: &str, scope: &Scope) -> Result<Enclosed, String> {
    let mut parser = Parser {
        text: text.chars().collect(),
        at: 0,
        scope,
    };
    let value = parser.expr()?;
    match parser.peek() {
        None => Ok(value),
        Some(c) => Err(format!("`{c}` is not expected in `{text}`")),
    }
}

impl Parser<'_> {
    /// The next character that is not a space, left unread.
    fn peek(&mut self) -> Option<char> {
        while self.text.get(self.at).is_some_and(|c| c.is_whitespace()) {
            self.at += 1;
        }
        self.text.get(self.at).copied()
    }

    /// Reads `wanted`, or says it is missing.
    fn expect(&mut self, wanted: char) -> Result<(), String> {
        if self.peek() == Some(wanted) {
            self.at += 1;
            Ok(())
        } else {
            Err(format!("`{wanted}` is missing"))
        }
    }

    /// A sum: terms joined by `+` and `-`.
    fn expr(&mut self) -> Result<Enclosed, String> {
        let mut value = self.term()?;
        loop {
            match self.peek() {
                Some('+') => {
                    self.at += 1;
                    value = value.add(&self.term()?);
                }
                Some('-') => {
                    self.at += 1;
                    value = value.sub(&self.term()?);
                }
                _ => return Ok(value),
            }
        }
    }

    /// A product: factors joined by `*` and `/`.
    fn term(&mut self) -> Result<Enclosed, String> {
        let mut value = self.unary()?;
        loop {
            match self.peek() {
                Some('*') => {
                    self.at += 1;
                    value = value.mul(&self.unary()?);
                }
                Some('/') => {
                    self.at += 1;
                    let divisor = self.unary()?;
                    value = value
                        .div(&divisor)
                        .ok_or("a quotient by a value that may be zero")?;
                }
                _ => return Ok(value),
            }
        }
    }

    /// A factor with its signs.
    fn unary(&mut self) -> Result<Enclosed, String> {
        match self.peek() {
            Some('-') => {
                self.at += 1;
                Ok(self.unary()?.neg())
            }
            Some('+') => {
                self.at += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    /// An atom, with an optional whole power.
    fn power(&mut self) -> Result<Enclosed, String> {
        let base = self.atom()?;
        if self.peek() != Some('^') {
            return Ok(base);
        }
        self.at += 1;
        let mut digits = String::new();
        while let Some(c) = self.text.get(self.at).copied().filter(char::is_ascii_digit) {
            digits.push(c);
            self.at += 1;
        }
        let n: u32 = digits
            .parse()
            .map_err(|_| "a power must be a whole number".to_owned())?;
        Ok(base.powi(n))
    }

    /// A number, a bracketed sum, a name or a function call.
    fn atom(&mut self) -> Result<Enclosed, String> {
        let Some(c) = self.peek() else {
            return Err("the formula ends too soon".to_owned());
        };
        if c == '(' {
            self.at += 1;
            let value = self.expr()?;
            self.expect(')')?;
            return Ok(value);
        }
        if c.is_ascii_digit() || c == '.' {
            let mut text = String::new();
            while let Some(d) = self
                .text
                .get(self.at)
                .copied()
                .filter(|d| d.is_ascii_digit() || *d == '.')
            {
                text.push(d);
                self.at += 1;
            }
            return Ok(Enclosed::of(&decimal(&text)?));
        }
        if c.is_ascii_alphabetic() || c == '_' {
            return self.name();
        }
        Err(format!("`{c}` is not expected"))
    }

    /// A name: `pi`, a name of the scope, or a function and its arguments.
    fn name(&mut self) -> Result<Enclosed, String> {
        let mut word = String::new();
        while let Some(c) = self
            .text
            .get(self.at)
            .copied()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        {
            word.push(c);
            self.at += 1;
        }
        if self.peek() == Some('(') {
            self.at += 1;
            let mut args = vec![self.expr()?];
            while self.peek() == Some(',') {
                self.at += 1;
                args.push(self.expr()?);
            }
            self.expect(')')?;
            return call(&word, &args);
        }
        if word == "pi" {
            return Ok(pi_once().clone());
        }
        self.scope
            .names
            .get(&word)
            .cloned()
            .ok_or_else(|| format!("`{word}` is not a known name"))
    }
}

/// The function `name` of `args`.
fn call(name: &str, args: &[Enclosed]) -> Result<Enclosed, String> {
    let out = match (name, args) {
        ("sqrt", [x]) => x.sqrt(),
        ("sin", [x]) => Some(sin(x)),
        ("cos", [x]) => Some(cos(x)),
        ("tan", [x]) => tan(x),
        ("atan", [x]) => atan(x),
        ("asin", [x]) => asin(x),
        ("acos", [x]) => acos(x),
        ("atan2", [y, x]) => atan2(y, x),
        _ => {
            return Err(format!(
                "`{name}` is not a function of {} values",
                args.len()
            ));
        }
    };
    out.ok_or_else(|| format!("`{name}` is outside its domain"))
}

/// The integrals over a region's boundary that Green's theorem turns into its area, its moments
/// and its perimeter's first moment.
#[derive(Clone, Debug)]
pub struct Moments {
    /// The area, ∮ x dy.
    pub area: Enclosed,
    /// ∬ x dA, as ∮ x²/2 dy.
    pub mx: Enclosed,
    /// ∬ y dA, as ∮ xy dy.
    pub my: Enclosed,
    /// ∬ xy dA, as ∮ x²y/2 dy.
    pub mxy: Enclosed,
    /// The perimeter, ∮ ds.
    pub perimeter: Enclosed,
    /// ∮ x ds, which turned about the y axis gives the area of the surface of revolution over 2π.
    pub px: Enclosed,
}

impl Moments {
    /// Nothing yet.
    fn zero() -> Moments {
        let none = Enclosed::from_whole(0);
        Moments {
            area: none.clone(),
            mx: none.clone(),
            my: none.clone(),
            mxy: none.clone(),
            perimeter: none.clone(),
            px: none,
        }
    }

    /// The sum of two.
    fn plus(&self, other: &Moments) -> Moments {
        Moments {
            area: self.area.add(&other.area),
            mx: self.mx.add(&other.mx),
            my: self.my.add(&other.my),
            mxy: self.mxy.add(&other.mxy),
            perimeter: self.perimeter.add(&other.perimeter),
            px: self.px.add(&other.px),
        }
    }
}

/// The product of all of `parts`.
fn prod(parts: &[&Enclosed]) -> Enclosed {
    parts
        .iter()
        .fold(Enclosed::from_whole(1), |all, part| all.mul(part))
}

/// The sum of all of `parts`.
fn sum(parts: &[Enclosed]) -> Enclosed {
    parts
        .iter()
        .fold(Enclosed::from_whole(0), |all, part| all.add(part))
}

/// The moments of the straight segment from `from` to `to`, parametrised by t from 0 to 1:
/// x = x₀ + t·dx, y = y₀ + t·dy, so ∮ x dy is dy(x₀ + dx/2) and the others are the polynomials in
/// t integrated over [0, 1].
#[must_use]
pub fn line_moments(from: &[Enclosed; 2], to: &[Enclosed; 2]) -> Option<Moments> {
    let [x0, y0] = from;
    let [x1, y1] = to;
    let dx = x1.sub(x0);
    let dy = y1.sub(y0);
    let length = dx.square().add(&dy.square()).sqrt()?;
    let two = Enclosed::from_whole(2);
    // x₀² + x₀dx + dx²/3, the integral over t of x².
    let xx = sum(&[x0.square(), x0.mul(&dx), dx.square().div_whole(3)]);
    let area = dy.mul(&x0.add(&dx.div_whole(2)));
    let mx = dy.mul(&xx).div_whole(2);
    let cross = sum(&[
        x0.mul(y0),
        x0.mul(&dy).add(&y0.mul(&dx)).div_whole(2),
        dx.mul(&dy).div_whole(3),
    ]);
    let my = dy.mul(&cross);
    // The integral over t of x²·(y₀ + t·dy): y₀ times xx, and dy times x₀²/2 + 2x₀dx/3 + dx²/4.
    let tail = sum(&[
        x0.square().div_whole(2),
        prod(&[&two, x0, &dx]).div_whole(3),
        dx.square().div_whole(4),
    ]);
    let mxy = dy.mul(&y0.mul(&xx).add(&dy.mul(&tail))).div_whole(2);
    let px = length.mul(&x0.add(&dx.div_whole(2)));
    Some(Moments {
        area,
        mx,
        my,
        mxy,
        perimeter: length,
        px,
    })
}

/// The moments of the circular arc about `centre` of radius `radius` from the angle `from` to the
/// angle `to` (to below from is clockwise; the sweep must be nonzero): x = cx + r cos a, y = cy +
/// r sin a, dy = r cos a da, ds = r |da|. Every integrand is a polynomial in cos a and sin a of
/// degree four at most, whose antiderivatives are the closed forms used below.
#[must_use]
pub fn arc_moments(
    centre: &[Enclosed; 2],
    radius: &Enclosed,
    from: &Enclosed,
    to: &Enclosed,
) -> Option<Moments> {
    let [cx, cy] = centre;
    let r = radius;
    let delta = to.sub(from);
    let sign = if *delta.lo() > RBig::ZERO {
        Enclosed::from_whole(1)
    } else if *delta.hi() < RBig::ZERO {
        Enclosed::from_whole(1).neg()
    } else {
        return None;
    };
    let (s0, c0, s1, c1) = (sin(from), cos(from), sin(to), cos(to));
    let two = Enclosed::from_whole(2);
    // The integrals of cos, sin·cos, cos², cos²·sin, cos³ and cos³·sin from a₀ to a₁.
    let i_c = s1.sub(&s0);
    let i_cs = s1.square().sub(&s0.square()).div_whole(2);
    let double_0 = prod(&[&two, &s0, &c0]);
    let double_1 = prod(&[&two, &s1, &c1]);
    let i_cc = delta
        .div_whole(2)
        .add(&double_1.sub(&double_0).div_whole(4));
    let i_ccs = c0.powi(3).sub(&c1.powi(3)).div_whole(3);
    let i_ccc = i_c.sub(&s1.powi(3).sub(&s0.powi(3)).div_whole(3));
    let i_cccs = c0.powi(4).sub(&c1.powi(4)).div_whole(4);
    let r2 = r.square();
    let r3 = r2.mul(r);
    let cx2 = cx.square();
    let area = prod(&[cx, r, &i_c]).add(&prod(&[&r2, &i_cc]));
    let mx = sum(&[
        prod(&[&cx2, r, &i_c]),
        prod(&[&two, cx, &r2, &i_cc]),
        prod(&[&r3, &i_ccc]),
    ])
    .div_whole(2);
    let my = prod(&[
        r,
        &sum(&[
            prod(&[cx, cy, &i_c]),
            prod(&[cx, r, &i_cs]),
            prod(&[r, cy, &i_cc]),
            prod(&[&r2, &i_ccs]),
        ]),
    ]);
    let mxy = prod(&[
        r,
        &sum(&[
            prod(&[&cx2, cy, &i_c]),
            prod(&[&cx2, r, &i_cs]),
            prod(&[&two, cx, r, cy, &i_cc]),
            prod(&[&two, cx, &r2, &i_ccs]),
            prod(&[&r2, cy, &i_ccc]),
            prod(&[&r3, &i_cccs]),
        ]),
    ])
    .div_whole(2);
    let perimeter = prod(&[r, &delta, &sign]);
    let px = prod(&[&sign, r, &cx.mul(&delta).add(&r.mul(&i_c))]);
    Some(Moments {
        area,
        mx,
        my,
        mxy,
        perimeter,
        px,
    })
}

/// The point `centre + radius·(cos angle, sin angle)`.
fn on_circle(centre: &[Enclosed; 2], radius: &Enclosed, angle: &Enclosed) -> [Enclosed; 2] {
    let [cx, cy] = centre;
    [
        cx.add(&radius.mul(&cos(angle))),
        cy.add(&radius.mul(&sin(angle))),
    ]
}

/// The moments of the closed polygon through `points`, in order.
fn polygon(points: &[[Enclosed; 2]]) -> Option<Moments> {
    let mut total = Moments::zero();
    for (from, to) in points.iter().zip(points.iter().cycle().skip(1)) {
        total = total.plus(&line_moments(from, to)?);
    }
    Some(total)
}

/// The moments of one segment of a loop: its kind and its formulas, evaluated.
fn segment_moments(kind: &str, v: &[Enclosed]) -> Result<Moments, String> {
    let out = match (kind, v) {
        ("L", [x0, y0, x1, y1]) => {
            line_moments(&[x0.clone(), y0.clone()], &[x1.clone(), y1.clone()])
        }
        ("A", [cx, cy, r, a0, a1]) => arc_moments(&[cx.clone(), cy.clone()], r, a0, a1),
        ("P", points) if points.len() >= 6 && points.len() % 2 == 0 => {
            let pairs: Vec<[Enclosed; 2]> = points
                .chunks(2)
                .filter_map(|pair| match pair {
                    [x, y] => Some([x.clone(), y.clone()]),
                    _ => None,
                })
                .collect();
            polygon(&pairs)
        }
        ("R", [u0, u1, v0, v1]) => polygon(&[
            [u0.clone(), v0.clone()],
            [u1.clone(), v0.clone()],
            [u1.clone(), v1.clone()],
            [u0.clone(), v1.clone()],
        ]),
        ("S", [cx, cy, r0, r1, a0, a1]) => sector(&[cx.clone(), cy.clone()], [r0, r1], [a0, a1]),
        _ => return Err(format!("`{kind}` with {} values is not a segment", v.len())),
    };
    out.ok_or_else(|| format!("a `{kind}` segment cannot be integrated"))
}

/// The annular sector about `centre` between the radii and the angles given (counterclockwise):
/// the outer arc, the radial line in, the inner arc back and the radial line out.
fn sector(
    centre: &[Enclosed; 2],
    radii: [&Enclosed; 2],
    angles: [&Enclosed; 2],
) -> Option<Moments> {
    let [inner, outer] = radii;
    let [from, to] = angles;
    let outer_arc = arc_moments(centre, outer, from, to)?;
    let inner_arc = arc_moments(centre, inner, to, from)?;
    let line_in = line_moments(&on_circle(centre, outer, to), &on_circle(centre, inner, to))?;
    let line_out = line_moments(
        &on_circle(centre, inner, from),
        &on_circle(centre, outer, from),
    )?;
    Some(outer_arc.plus(&line_in).plus(&inner_arc).plus(&line_out))
}

/// The moments of a region: the sum over its loops' segments, with the segments' formulas
/// evaluated in `scope`.
///
/// # Errors
///
/// A message if a formula or a segment cannot be evaluated.
pub fn region_moments(
    spec: &RegionSpec,
    scope_names: &BTreeMap<String, Enclosed>,
) -> Result<Moments, String> {
    let scope = Scope {
        names: scope_names.clone(),
    };
    let mut total = Moments::zero();
    for segment in spec.loops.iter().flatten() {
        let Some((kind, formulas)) = segment.split_first() else {
            return Err("an empty segment".to_owned());
        };
        let values = formulas
            .iter()
            .map(|f| evaluate(f, &scope))
            .collect::<Result<Vec<_>, _>>()?;
        total = total.plus(&segment_moments(kind, &values)?);
    }
    Ok(total)
}

/// The value of the optional formula, if there is one.
fn optional(formula: Option<&String>, scope: &Scope) -> Result<Option<Enclosed>, String> {
    formula.map(|f| evaluate(f, scope)).transpose()
}

/// Three optional formulas, evaluated.
fn triple(formulas: &[Option<String>; 3], scope: &Scope) -> Result<[Option<Enclosed>; 3], String> {
    let [x, y, z] = formulas;
    Ok([
        optional(x.as_ref(), scope)?,
        optional(y.as_ref(), scope)?,
        optional(z.as_ref(), scope)?,
    ])
}

/// What the table derives for `case`.
///
/// # Errors
///
/// A message naming the case if a formula cannot be evaluated.
pub fn derive(case: &Case) -> Result<Derived, String> {
    let named = |e: String| format!("{}: {e}", case.name);
    let mut scope = Scope::default();
    for [name, formula] in &case.params {
        let value = evaluate(formula, &scope).map_err(named)?;
        scope.names.insert(name.clone(), value);
    }
    for spec in &case.regions {
        let m = region_moments(spec, &scope.names).map_err(named)?;
        for (suffix, value) in [
            ("_A", m.area),
            ("_Sx", m.mx),
            ("_Sy", m.my),
            ("_Sxy", m.mxy),
            ("_P", m.perimeter),
            ("_Px", m.px),
        ] {
            scope.names.insert(format!("{}{suffix}", spec.name), value);
        }
    }
    for [name, formula] in &case.lets {
        let value = evaluate(formula, &scope).map_err(named)?;
        scope.names.insert(name.clone(), value);
    }
    Ok(Derived {
        volume: optional(case.volume.as_ref(), &scope).map_err(named)?,
        area: optional(case.area.as_ref(), &scope).map_err(named)?,
        com: triple(&case.com, &scope).map_err(named)?,
        min: triple(&case.min, &scope).map_err(named)?,
        max: triple(&case.max, &scope).map_err(named)?,
    })
}
