//! What two quadrics make of each other, read from the pencil of their matrices.
//!
//! What it does: two quadrics are two symmetric 4 × 4 matrices A and B, and [`classify`] reads the
//! family tA + B, the pencil, exactly. The roots of det(tA + B), how often each repeats and the rank
//! of the member there are the pencil's Segre symbol, and the signature of the members around them
//! says which of that symbol's real forms this is. Together they give [`Topology`]: how many curves
//! of each kind the two quadrics share, in real projective space, how many isolated points and how
//! many points where the parts meet, with no curve computed. [`filtered`] gives the same answer in
//! `f64` intervals where those decide. [`revolution`] writes a sphere, cylinder or cone as a matrix.
//!
//! Guarantees:
//!
//! - **Every pencil.** A pencil whose determinant is not zero has one of the fourteen symbols of
//!   [`Segre`]. One whose determinant is zero, every member singular, is two cones over one apex, read
//!   as the pencil of their base conics, or two cones that share a line and touch along it.
//! - **Empty is decided.** Two real quadrics share no real point exactly when some member is definite
//!   (Finsler and Calabi), and a member is definite on a whole gap between roots, so a sample of each
//!   gap decides it.
//! - **The budget.** A sphere, cylinder or cone of 40-bit coordinates, 53-bit radius and slope, has
//!   entries below 2^192 in the 3 × 3 block, 2^234 beside it and 2^276 in the corner; a determinant's
//!   products weigh 852 bits whichever entries they take, and 24 terms over 16 choices of row, with
//!   a basis of members up to `b + 8a` adding 13 bits, make B = 876. A Sturm chain's values stay below 2^(13B + 50) =
//!   2^11438 (`crate::sturm`), and two roots stand at least 2^(−3B − 9) apart, by Mahler's bound with
//!   a discriminant of at least 1, so a dyadic sample of 2638 bits lands in any gap; the largest sign
//!   read is then below 2^(852 + 23 + 4·2638) = 2^11427. [`Big`], 11776 bits, holds them all, and
//!   every step is checked, so a broken budget is an error, never a wrong answer.
//! - **Off the budget, an error.** [`classify`] takes any integer matrices, and refuses a pair whose
//!   values do not fit.
//! - **The filter never guesses.** [`filtered`] answers only where every sign it reads is certain.
//!
//! Method: Dupont, Lazard, Lazard and Petitjean, J. Symbolic Computation 43(3), 2008, parts I and II
//! (<https://doi.org/10.1016/j.jsc.2007.10.006>, <https://doi.org/10.1016/j.jsc.2007.10.012>), taken
//! from the papers alone: the classification of real pencils by Segre symbol and inertia. Each row of
//! [`Segre`] is derived in chapter 12.7 from Uhlig's canonical pairs (Linear Algebra Appl. 14, 1976,
//! <https://doi.org/10.1016/0024-3795(76)90066-3>), with Finsler's theorem for emptiness. Inertia by
//! Descartes' rule on the characteristic polynomial, which is exact for a real-rooted polynomial.
//!
//! Cost: a determinant and the sums of principal minors are a few hundred products of 16-limb
//! integers; a chain is [`crate::sturm`]'s, at [`LIMBS`] limbs; each sample takes a bisection, up to
//! 2638 evaluations of a chain, and in practice a few dozen. Stack: about 150 kB. Recursion: none.

#![allow(
    clippy::many_single_char_names,
    reason = "the names are the papers': a pencil's t, its members and their entries"
)]

use crate::error::NumError;
use crate::grid::P3;
use crate::interval::Interval;
use crate::quadric::{self, Cone, Cylinder, Ring, Sphere};
use crate::sign::Sign;
use crate::sturm::{self, Chain, TERMS};
use crate::wide::Wide;

/// The limbs of a chain's integers and of every sign read: 11776 bits, for the budget of the module
/// header.
pub const LIMBS: usize = 184;

/// The integers of a chain, beyond chapter 6.2's widths and listed in 31.7.
pub type Big = Wide<LIMBS>;

/// The integers of a matrix and of a determinant: 1024 bits, past the budget's B.
pub type Entry = Wide<16>;

/// A symmetric 4 × 4 matrix, on the coordinates (x, y, z, w).
pub type Form<R> = [[R; 4]; 4];

/// The most bits a sample may need: 3B + 10 for the budget's B.
const DEPTH: u32 = 2640;

/// The members `b + k·a` tried for a basis, k from −`REACH` to `REACH`: 17 values, and a determinant
/// has at most four roots, so three in a row are clear unless it is zero.
const REACH: i64 = 8;

/// Whether bit `at` of `mask` is set.
fn bit(mask: u8, at: usize) -> bool {
    u32::try_from(at)
        .ok()
        .and_then(|at| mask.checked_shr(at))
        .is_some_and(|m| m & 1 == 1)
}

/// How many positive and negative eigenvalues a symmetric matrix has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inertia {
    /// Positive.
    pub positive: u8,
    /// Negative.
    pub negative: u8,
}

impl Inertia {
    /// How many are not zero.
    #[must_use]
    pub const fn rank(self) -> u8 {
        self.positive.saturating_add(self.negative)
    }

    /// Whether every eigenvalue that is not zero has one sign.
    #[must_use]
    pub const fn one_sided(self) -> bool {
        self.positive == 0 || self.negative == 0
    }

    /// How far the two counts stand apart.
    #[must_use]
    pub const fn lean(self) -> u8 {
        self.positive.abs_diff(self.negative)
    }
}

/// A pencil's Segre symbol: the sizes of the Jordan blocks of each root, a parenthesis gathering the
/// blocks of one root. Members of a rank below four sit at the roots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segre {
    /// [1111]: four simple roots.
    Simple,
    /// [211]: one double root, of rank three.
    Double,
    /// [31]: one triple root, of rank three.
    Triple,
    /// [22]: two double roots, each of rank three.
    TwoDoubles,
    /// [4]: one fourfold root, of rank three.
    Fourfold,
    /// [(11)11]: one double root, of rank two.
    Pair,
    /// [(11)2]: two double roots, of ranks two and three.
    PairAndDouble,
    /// [(11)(11)]: two double roots, each of rank two.
    TwoPairs,
    /// [(21)1]: one triple root, of rank two.
    TriplePair,
    /// [(31)]: a fourfold root of rank two, the other member not zero on its kernel.
    FourfoldPair,
    /// [(22)]: a fourfold root of rank two, the other member zero on its kernel.
    FourfoldTwoBlocks,
    /// [(111)1]: one triple root, of rank one.
    TripleRankOne,
    /// [(211)]: a fourfold root of rank one.
    FourfoldRankOne,
    /// [(1111)]: one member is zero: the two are one quadric.
    Same,
    /// Every member singular over one apex: the pencil of the base conics decides.
    OneApex,
    /// Every member singular with no shared apex: two cones touching along a line.
    TouchingCones,
}

/// How an irreducible quartic stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quartic {
    /// Smooth, of one or two closed branches.
    Smooth {
        /// How many.
        ovals: u8,
    },
    /// One closed branch crossing itself at a real node.
    Crossing,
    /// One closed branch, and the node standing apart as an isolated point.
    Isolated,
    /// One closed branch with a cusp.
    Cusp,
}

/// What two quadrics share, over the reals, in projective space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Topology {
    /// An irreducible quartic, if that is what they share.
    pub quartic: Option<Quartic>,
    /// Twisted cubics.
    pub cubics: u8,
    /// Conics.
    pub conics: u8,
    /// Lines.
    pub lines: u8,
    /// Conics along which the two touch.
    pub double_conics: u8,
    /// Lines along which the two touch.
    pub double_lines: u8,
    /// Isolated points.
    pub points: u8,
    /// Real points where parts meet.
    pub meets: u8,
    /// Whether the two are one quadric.
    pub same: bool,
}

/// What [`classify`] finds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pencil {
    /// The symbol.
    pub segre: Segre,
    /// The distinct real roots of the determinant, or of the base conics' for [`Segre::OneApex`].
    pub real_roots: u8,
    /// What the two share.
    pub topology: Topology,
}

/// The quadric α|w|² − β(w·e)² − γ = 0, for w = x − c: a sphere with β = 0, a cylinder with
/// α = β·|e|², and a cone with γ = 0. Each is one line of [`revolution`]'s arithmetic.
///
/// A sphere of radius r is α = 1, β = 0, γ = r². A cylinder of radius r about direction e is
/// α = |e|², β = 1, γ = r²|e|². A cone of slope s/t is α = t²|e|², β = s² + t², γ = 0.
///
/// Budget: the module header's. `None` if a step does not fit.
#[must_use]
pub fn revolution<R: Ring>(c: [R; 3], e: [R; 3], alpha: R, beta: R, gamma: R) -> Option<Form<R>> {
    let zero = R::of(0)?;
    let mut form = [[zero; 4]; 4];
    let cell = |form: &Form<R>, r: usize, s: usize| form.get(r).and_then(|row| row.get(s)).copied();
    for (r, row) in form.iter_mut().enumerate().take(3) {
        for (s, slot) in row.iter_mut().enumerate().take(3) {
            let along = beta.times(*e.get(r)?)?.times(*e.get(s)?)?;
            *slot = if r == s {
                alpha.minus(along)?
            } else {
                zero.minus(along)?
            };
        }
    }
    let mut corner = zero.minus(gamma)?;
    for r in 0..3 {
        let mut image = zero;
        for s in 0..3 {
            image = image.plus(cell(&form, r, s)?.times(*c.get(s)?)?)?;
        }
        corner = corner.plus(image.times(*c.get(r)?)?)?;
        let side = zero.minus(image)?;
        put(&mut form, r, 3, side)?;
        put(&mut form, 3, r, side)?;
    }
    put(&mut form, 3, 3, corner)?;
    Some(form)
}

/// Writes one cell.
fn put<R: Ring>(form: &mut Form<R>, r: usize, c: usize, v: R) -> Option<()> {
    let row: &mut [R; 4] = form.get_mut(r)?;
    let cell: &mut R = row.get_mut(c)?;
    *cell = v;
    Some(())
}

/// `sphere` about the point `origin`. `None` if a step does not fit, which the budget rules out.
#[must_use]
pub fn sphere_form(sphere: &Sphere, origin: P3) -> Option<Form<Entry>> {
    let c: [Entry; 3] = quadric::minus(sphere.centre, origin)?;
    let r = quadric::size::<Entry>(sphere.radius.0)?;
    revolution(c, [Entry::ZERO; 3], Entry::one(), Entry::ZERO, r.times(r)?)
}

/// `cylinder` about the point `origin`. As [`sphere_form`].
#[must_use]
pub fn cylinder_form(cylinder: &Cylinder, origin: P3) -> Option<Form<Entry>> {
    let c: [Entry; 3] = quadric::minus(cylinder.base, origin)?;
    let e: [Entry; 3] = quadric::minus(cylinder.along, cylinder.base)?;
    let n = quadric::inner(e, e)?;
    let r = quadric::size::<Entry>(cylinder.radius.0)?;
    revolution(c, e, n, Entry::one(), r.times(r)?.times(n)?)
}

/// `cone` about the point `origin`. As [`sphere_form`].
#[must_use]
pub fn cone_form(cone: &Cone, origin: P3) -> Option<Form<Entry>> {
    let c: [Entry; 3] = quadric::minus(cone.apex, origin)?;
    let e: [Entry; 3] = quadric::minus(cone.along, cone.apex)?;
    let n = quadric::inner(e, e)?;
    let s = quadric::size::<Entry>(cone.slope.rise)?;
    let t = quadric::size::<Entry>(cone.slope.run)?;
    let t2 = t.times(t)?;
    revolution(c, e, t2.times(n)?, s.times(s)?.plus(t2)?, Entry::ZERO)
}

/// Two matrices, each row of a determinant taken from one or the other by a bit of `mask`.
struct Rows<'a, R> {
    first: &'a Form<R>,
    second: &'a Form<R>,
    mask: u8,
}

impl<R: Ring> Rows<'_, R> {
    fn at(&self, r: usize, c: usize) -> Option<R> {
        let from = if bit(self.mask, r) {
            self.second
        } else {
            self.first
        };
        from.get(r)?.get(c).copied()
    }

    fn two(&self, [p, q]: [usize; 2], [a, b]: [usize; 2]) -> Option<R> {
        let first = self.at(p, a)?.times(self.at(q, b)?)?;
        first.minus(self.at(p, b)?.times(self.at(q, a)?)?)
    }

    fn three(&self, [p, q, r]: [usize; 3], [a, b, c]: [usize; 3]) -> Option<R> {
        let one = self.at(p, a)?.times(self.two([q, r], [b, c])?)?;
        let two = self.at(p, b)?.times(self.two([q, r], [a, c])?)?;
        let three = self.at(p, c)?.times(self.two([q, r], [a, b])?)?;
        one.minus(two)?.plus(three)
    }

    /// Laplace's expansion on the first two rows: six products of two minors.
    fn four(&self) -> Option<R> {
        const PAIRS: [([usize; 2], [usize; 2], bool); 6] = [
            ([0, 1], [2, 3], true),
            ([0, 2], [1, 3], false),
            ([0, 3], [1, 2], true),
            ([1, 2], [0, 3], true),
            ([1, 3], [0, 2], false),
            ([2, 3], [0, 1], true),
        ];
        let mut out = R::of(0)?;
        for (top, bottom, plus) in PAIRS {
            let term = self.two([0, 1], top)?.times(self.two([2, 3], bottom)?)?;
            out = if plus {
                out.plus(term)?
            } else {
                out.minus(term)?
            };
        }
        Some(out)
    }

    /// The minor on the rows and columns of the bits of `set`.
    fn principal(&self, set: u8) -> Option<R> {
        let mut at = [0_usize; 4];
        let mut size = 0_usize;
        for index in 0..4_usize {
            if bit(set, index) {
                *at.get_mut(size)? = index;
                size = size.checked_add(1)?;
            }
        }
        let [a, b, c, _] = at;
        match size {
            1 => self.at(a, a),
            2 => self.two([a, b], [a, b]),
            3 => self.three([a, b, c], [a, b, c]),
            _ => self.four(),
        }
    }
}

/// The sums of the principal minors of tC + S: `out[k]` is the sum over minors of k + 1 rows, as a
/// polynomial in t, lowest first. Each coefficient of a minor is a sum over which rows hold t.
fn minors<R: Ring>(c: &Form<R>, s: &Form<R>) -> Option<[[R; TERMS]; 4]> {
    let zero = R::of(0)?;
    let mut out = [[zero; TERMS]; 4];
    for set in 1_u8..16 {
        let size = usize::try_from(set.count_ones()).ok()?.checked_sub(1)?;
        for mask in 0_u8..16 {
            if mask & !set != 0 {
                continue;
            }
            let rows = Rows {
                first: s,
                second: c,
                mask,
            };
            let power = usize::try_from(mask.count_ones()).ok()?;
            let slot = out.get_mut(size)?.get_mut(power)?;
            *slot = slot.plus(rows.principal(set)?)?;
        }
    }
    Some(out)
}

/// The determinant of `m`.
fn determinant<R: Ring>(m: &Form<R>) -> Option<R> {
    Rows {
        first: m,
        second: m,
        mask: 0,
    }
    .four()
}

/// `a + k·b`, cell by cell.
fn plus_times<R: Ring>(a: &Form<R>, k: i64, b: &Form<R>) -> Option<Form<R>> {
    let mut out = *a;
    let scale = R::of(k)?;
    for (row, line) in out.iter_mut().zip(b) {
        for (slot, cell) in row.iter_mut().zip(line) {
            *slot = slot.plus(scale.times(*cell)?)?;
        }
    }
    Some(out)
}

/// Two members that span a pencil.
type Basis<R> = (Form<R>, Form<R>);

/// A basis (C, S) of the pencil of `a` and `b` with C and C ± S all passing `clear`, so that neither
/// infinity nor ±1 is a root: S = `a`, and C, C + S and C − S three members in a row. `None` if there
/// are no such three.
///
/// # Errors
///
/// [`NumError::BudgetExceeded`] if a step did not fit.
fn basis<R: Ring>(
    a: &Form<R>,
    b: &Form<R>,
    clear: impl Fn(&Form<R>) -> Option<bool>,
) -> Result<Option<Basis<R>>, NumError> {
    let mut run = 0_u8;
    for k in -REACH..=REACH {
        let member = plus_times(b, k, a).ok_or(NumError::BudgetExceeded)?;
        run = if clear(&member).ok_or(NumError::BudgetExceeded)? {
            run.saturating_add(1)
        } else {
            0
        };
        if run == 3 {
            let middle = plus_times(&member, -1, a).ok_or(NumError::BudgetExceeded)?;
            return Ok(Some((middle, *a)));
        }
    }
    Ok(None)
}

/// The inertia of `form`, if the sign of each sum of its principal minors is certain.
#[must_use]
pub fn inertia_certain(form: &Form<Interval>) -> Option<Inertia> {
    let mut sums = [Interval::of(0)?; 4];
    for set in 1_u8..16 {
        let rows = Rows {
            first: form,
            second: form,
            mask: 0,
        };
        let size = usize::try_from(set.count_ones()).ok()?.checked_sub(1)?;
        let slot = sums.get_mut(size)?;
        *slot = slot.plus(rows.principal(set)?)?;
    }
    let mut signs = [Sign::Zero; 4];
    for (slot, sum) in signs.iter_mut().zip(sums) {
        *slot = sum.certain_sign()?;
    }
    Some(inertia_of(signs))
}

/// How many times the sign changes along `signs`, zeros skipped.
fn changes(signs: [Sign; 5]) -> u8 {
    let mut count = 0_u8;
    let mut last = Sign::Zero;
    for s in signs {
        if s != Sign::Zero {
            if last != Sign::Zero && s != last {
                count = count.saturating_add(1);
            }
            last = s;
        }
    }
    count
}

/// The inertia of a symmetric matrix from the signs of the sums of its principal minors, by Descartes'
/// rule: x⁴ − e₁x³ + e₂x² − e₃x + e₄ has only real roots, so its sign changes count the positive ones
/// exactly, and the changes of x⁴ + e₁x³ + e₂x² + e₃x + e₄ the negative ones.
#[must_use]
pub fn inertia_of(signs: [Sign; 4]) -> Inertia {
    let [a, b, c, d] = signs;
    Inertia {
        positive: changes([
            Sign::Positive,
            a.mul(Sign::Negative),
            b,
            c.mul(Sign::Negative),
            d,
        ]),
        negative: changes([Sign::Positive, a, b, c, d]),
    }
}

/// A point of the pencil's line, (num : den) with `den` not negative, in [`Big`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct At {
    num: Big,
    den: Big,
}

impl At {
    /// The whole number `v`.
    fn whole(v: i64) -> At {
        At {
            num: Big::from_i64(v),
            den: Big::one(),
        }
    }
}

/// The value at (num : den) of a polynomial, homogenised to degree `degree`.
fn homogeneous(terms: &[Big; TERMS], degree: usize, at: &At) -> Option<Big> {
    let mut power = Big::one();
    let mut sum = Big::ZERO;
    for c in terms.iter().take(degree.checked_add(1)?).rev() {
        sum = sum
            .checked_mul(&at.num)?
            .checked_add(&c.checked_mul(&power)?)?;
        power = power.checked_mul(&at.den)?;
    }
    Some(sum)
}

/// The pencil's arithmetic on whole numbers.
struct Exact {
    /// The sums of principal minors, the last the determinant.
    minors: [[Entry; TERMS]; 4],
    /// The determinant's degree: four, or three on the base conics of one apex.
    degree: usize,
    /// The chain of the determinant.
    chain: Chain<LIMBS>,
    /// The real roots, each once.
    real: u8,
}

/// `terms` in [`Big`]s.
fn widened(terms: &[Entry; TERMS]) -> Option<[Big; TERMS]> {
    let mut out = [Big::ZERO; TERMS];
    for (slot, c) in out.iter_mut().zip(terms) {
        *slot = c.resize()?;
    }
    Some(out)
}

/// `terms` reversed, as a polynomial of degree `degree`: its roots are the inverses.
fn reversed(terms: &[Big; TERMS], degree: usize) -> Option<[Big; TERMS]> {
    let mut out = [Big::ZERO; TERMS];
    for (i, slot) in out.iter_mut().enumerate().take(degree.checked_add(1)?) {
        *slot = *terms.get(degree.checked_sub(i)?)?;
    }
    Some(out)
}

impl Exact {
    /// The arithmetic of the basis (C, S), of a pencil of `degree` by `degree` matrices.
    fn new(c: &Form<Entry>, s: &Form<Entry>, degree: usize) -> Result<Exact, NumError> {
        let sums = minors(c, s).ok_or(NumError::BudgetExceeded)?;
        let top = degree.checked_sub(1).ok_or(NumError::Degenerate)?;
        let determinant = sums.get(top).ok_or(NumError::Degenerate)?;
        let chain = Chain::of(&widened(determinant).ok_or(NumError::BudgetExceeded)?)?;
        let real = chain.roots().distinct;
        Ok(Exact {
            minors: sums,
            degree,
            chain,
            real,
        })
    }

    /// The determinant.
    fn determinant(&self) -> Option<[Big; TERMS]> {
        widened(self.minors.get(self.degree.checked_sub(1)?)?)
    }

    /// The inertia of the member at `at`.
    fn inertia(&self, at: &At) -> Option<Inertia> {
        let mut signs = [Sign::Zero; 4];
        for (k, (slot, terms)) in signs.iter_mut().zip(&self.minors).enumerate() {
            *slot = homogeneous(&widened(terms)?, k.checked_add(1)?, at)?.signum();
        }
        Some(inertia_of(signs))
    }

    /// The inertia of the members just to one side of the root `at`, from each sum's first
    /// derivative that is not zero there.
    fn beside(&self, at: &At, left: bool) -> Option<Inertia> {
        let mut signs = [Sign::Zero; 4];
        for (slot, terms) in signs.iter_mut().zip(&self.minors) {
            *slot = sturm::sign_beside(&widened(terms)?, &at.num, &at.den, left)?;
        }
        Some(inertia_of(signs))
    }

    /// Whether some gap between roots holds a definite member.
    fn definite(&self) -> Result<bool, NumError> {
        let size = u8::try_from(self.degree).map_err(|_| NumError::BudgetExceeded)?;
        let full = |i: Inertia| i.rank() == size && i.one_sided();
        for side in [1, -1] {
            let i = self
                .inertia(&At::whole(side))
                .ok_or(NumError::BudgetExceeded)?;
            if full(i) {
                return Ok(true);
            }
        }
        let terms = self.determinant().ok_or(NumError::Degenerate)?;
        let flipped = reversed(&terms, self.degree).ok_or(NumError::BudgetExceeded)?;
        let back = Chain::of(&flipped)?;
        for (chain, turned) in [(&self.chain, false), (&back, true)] {
            let inside = between(chain, &At::whole(-1), &At::whole(1))?;
            for gap in 1..inside {
                let at = sample(chain, gap)?;
                let point = if turned {
                    upright(&at).ok_or(NumError::BudgetExceeded)?
                } else {
                    at
                };
                if full(self.inertia(&point).ok_or(NumError::BudgetExceeded)?) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

/// (den : num), with the second not negative.
fn upright(at: &At) -> Option<At> {
    if at.num.is_negative() {
        Some(At {
            num: at.den.checked_neg()?,
            den: at.num.checked_neg()?,
        })
    } else {
        Some(At {
            num: at.den,
            den: at.num,
        })
    }
}

/// How many distinct real roots lie strictly between `lo` and `hi`.
fn between(chain: &Chain<LIMBS>, lo: &At, hi: &At) -> Result<u8, NumError> {
    chain
        .roots_in(&lo.num, &lo.den, &hi.num, &hi.den)
        .ok_or(NumError::BudgetExceeded)
}

/// A dyadic point of (−1, 1) past exactly `gap` roots and not a root, by bisection: the gap has a
/// width of at least 2^(−3B − 9), so [`DEPTH`] halvings reach inside it.
fn sample(chain: &Chain<LIMBS>, gap: u8) -> Result<At, NumError> {
    let two = Big::from_i64(2);
    let (mut lo, mut hi, mut den) = (Big::from_i64(-1), Big::one(), Big::one());
    let start = At::whole(-1);
    for _ in 0..DEPTH {
        let num = lo.checked_add(&hi).ok_or(NumError::BudgetExceeded)?;
        den = den.checked_mul(&two).ok_or(NumError::BudgetExceeded)?;
        let at = At { num, den };
        let below = between(chain, &start, &at)?;
        let root = chain
            .vanishes_at(&at.num, &at.den)
            .ok_or(NumError::BudgetExceeded)?;
        if below == gap && !root {
            return Ok(at);
        }
        let up = below < gap;
        lo = lo.checked_mul(&two).ok_or(NumError::BudgetExceeded)?;
        hi = hi.checked_mul(&two).ok_or(NumError::BudgetExceeded)?;
        if up {
            lo = num;
        } else {
            hi = num;
        }
    }
    Err(NumError::BudgetExceeded)
}

/// `terms` over the greatest common divisor of its coefficients.
fn primitive<const N: usize>(terms: &[Big; N]) -> Option<[Big; N]> {
    let mut common = Big::ZERO;
    for c in terms {
        common = sturm::gcd(&common, c)?;
    }
    if common.is_zero() {
        return None;
    }
    let mut out = *terms;
    for slot in &mut out {
        *slot = sturm::div_exact(slot, &common)?;
    }
    Some(out)
}

/// The root −num/den in lowest terms, with a positive second part.
fn root_of(num: &Big, den: &Big) -> Option<At> {
    let [n, d] = primitive(&[*num, *den])?;
    if d.is_negative() {
        Some(At {
            num: n,
            den: d.checked_neg()?,
        })
    } else {
        Some(At {
            num: n.checked_neg()?,
            den: d,
        })
    }
}

/// The sign of X + Y√d, for d not negative.
fn sign_with_root(x: &Big, y: &Big, d: &Big) -> Option<Sign> {
    let (sx, sy) = (x.signum(), y.signum());
    if sy == Sign::Zero || d.is_zero() {
        return Some(sx);
    }
    if sx == Sign::Zero {
        return Some(sy);
    }
    if sx == sy {
        return Some(sx);
    }
    let gap = x
        .checked_mul(x)?
        .checked_sub(&y.checked_mul(y)?.checked_mul(d)?)?;
    Some(match gap.signum() {
        Sign::Positive => sx,
        Sign::Negative => sy,
        Sign::Zero => Sign::Zero,
    })
}

/// The sign of `terms`, of degree at most two, at the root (−g₁ + σ√Δ)/(2g₂) of g₂t² + g₁t + g₀, with
/// Δ its discriminant, after reducing `terms` modulo the quadratic to αt + β over g₂².
fn sign_on_quadratic(terms: &[Big; TERMS], g: &[Big; 3], up: bool) -> Option<Sign> {
    let [g0, g1, g2] = g;
    let c0: &Big = terms.first()?;
    let c1: &Big = terms.get(1)?;
    let c2: &Big = terms.get(2)?;
    // c₂t² = c₂(−g₁t − g₀)/g₂, so the value times g₂ is (c₁g₂ − c₂g₁)t + c₀g₂ − c₂g₀.
    let alpha = c1.checked_mul(g2)?.checked_sub(&c2.checked_mul(g1)?)?;
    let beta = c0.checked_mul(g2)?.checked_sub(&c2.checked_mul(g0)?)?;
    let delta = g1
        .checked_mul(g1)?
        .checked_sub(&Big::from_i64(4).checked_mul(g2)?.checked_mul(g0)?)?;
    // Times 2g₂ again: α(−g₁ + σ√Δ) + 2g₂β, over 2g₂², which is positive.
    let x = alpha
        .checked_mul(&g1.checked_neg()?)?
        .checked_add(&Big::from_i64(2).checked_mul(g2)?.checked_mul(&beta)?)?;
    let y = if up { alpha } else { alpha.checked_neg()? };
    sign_with_root(&x, &y, &delta)
}

const EMPTY: Topology = Topology {
    quartic: None,
    cubics: 0,
    conics: 0,
    lines: 0,
    double_conics: 0,
    double_lines: 0,
    points: 0,
    meets: 0,
    same: false,
};

/// [1111]: a real elliptic curve has two branches when its discriminant is positive, four or no real
/// roots, and one otherwise; with four, a definite member empties it.
fn simple(pencil: &Exact) -> Result<Topology, NumError> {
    if pencil.real == 4 && pencil.definite()? {
        return Ok(EMPTY);
    }
    let ovals = if pencil.real == 2 { 1 } else { 2 };
    Ok(Topology {
        quartic: Some(Quartic::Smooth { ovals }),
        ..EMPTY
    })
}

/// One double root at `a`: its rank says [211] or [(11)11].
fn one_double(pencil: &Exact, a: &At) -> Result<(Segre, Topology), NumError> {
    let root = pencil.inertia(a).ok_or(NumError::BudgetExceeded)?;
    let left = pencil.beside(a, true).ok_or(NumError::BudgetExceeded)?;
    let right = pencil.beside(a, false).ok_or(NumError::BudgetExceeded)?;
    if root.rank() == 3 {
        // The cone at the root has its apex on the curve, and the curve is its base conic mapped
        // over the apex. Definite, it has no base. Otherwise the plane the other quadric touches
        // there cuts the base in the node's tangents: real exactly when the members beside lean
        // neither way.
        let quartic = if root.one_sided() {
            None
        } else if right.lean() == 0 {
            Some(Quartic::Crossing)
        } else {
            Some(Quartic::Isolated)
        };
        let points = u8::from(quartic.is_none());
        return Ok((
            Segre::Double,
            Topology {
                quartic,
                points,
                ..EMPTY
            },
        ));
    }
    // Two planes, real if the member is indefinite. A change of signature across the root is two
    // eigenvalues of one sign passing through zero: their line misses the other quadric.
    let crossing = left == right;
    let topology = if root.one_sided() {
        if crossing {
            Topology { points: 2, ..EMPTY }
        } else {
            EMPTY
        }
    } else if crossing {
        Topology {
            conics: 2,
            meets: 2,
            ..EMPTY
        }
    } else if pencil.real == 1 {
        Topology { conics: 1, ..EMPTY }
    } else if pencil.definite()? {
        EMPTY
    } else {
        Topology { conics: 2, ..EMPTY }
    };
    Ok((Segre::Pair, topology))
}

/// A triple root at `a`.
fn triple(pencil: &Exact, a: &At) -> Result<(Segre, Topology), NumError> {
    let root = pencil.inertia(a).ok_or(NumError::BudgetExceeded)?;
    Ok(match root.rank() {
        3 => (
            Segre::Triple,
            Topology {
                quartic: Some(Quartic::Cusp),
                ..EMPTY
            },
        ),
        2 if root.one_sided() => (Segre::TriplePair, Topology { points: 1, ..EMPTY }),
        2 => (
            Segre::TriplePair,
            Topology {
                conics: 2,
                meets: 1,
                ..EMPTY
            },
        ),
        _ if pencil.definite()? => (Segre::TripleRankOne, EMPTY),
        _ => (
            Segre::TripleRankOne,
            Topology {
                double_conics: 1,
                ..EMPTY
            },
        ),
    })
}

/// A fourfold root at `a`. `flat` says whether the other member is zero on the kernel.
fn fourfold(pencil: &Exact, a: &At, flat: bool) -> Result<(Segre, Topology), NumError> {
    let root = pencil.inertia(a).ok_or(NumError::BudgetExceeded)?;
    let beside = pencil.beside(a, false).ok_or(NumError::BudgetExceeded)?;
    Ok(match root.rank() {
        3 => (
            Segre::Fourfold,
            Topology {
                cubics: 1,
                lines: 1,
                meets: 1,
                ..EMPTY
            },
        ),
        2 if flat && root.one_sided() => (
            Segre::FourfoldTwoBlocks,
            Topology {
                double_lines: 1,
                ..EMPTY
            },
        ),
        2 if flat => (
            Segre::FourfoldTwoBlocks,
            Topology {
                double_lines: 1,
                lines: 2,
                meets: 2,
                ..EMPTY
            },
        ),
        2 if beside.lean() == 0 => (
            Segre::FourfoldPair,
            Topology {
                conics: 1,
                lines: 2,
                meets: 1,
                ..EMPTY
            },
        ),
        2 => (Segre::FourfoldPair, Topology { conics: 1, ..EMPTY }),
        1 if beside.lean() == 0 => (
            Segre::FourfoldRankOne,
            Topology {
                double_lines: 2,
                meets: 1,
                ..EMPTY
            },
        ),
        1 => (Segre::FourfoldRankOne, Topology { points: 1, ..EMPTY }),
        _ if pencil.definite()? => (Segre::Same, EMPTY),
        _ => (
            Segre::Same,
            Topology {
                same: true,
                ..EMPTY
            },
        ),
    })
}

/// Two double roots, the roots of the quadratic `g`.
fn two_doubles(
    pencil: &Exact,
    c: &Form<Entry>,
    s: &Form<Entry>,
    g: &[Big; 3],
) -> Result<(Segre, Topology), NumError> {
    let [g0, g1, g2] = g;
    let delta = g1
        .checked_mul(g1)
        .zip(
            Big::from_i64(4)
                .checked_mul(g2)
                .and_then(|v| v.checked_mul(g0)),
        )
        .and_then(|(a, b)| a.checked_sub(&b))
        .ok_or(NumError::BudgetExceeded)?;
    let low = cofactors_vanish(c, s, g).ok_or(NumError::BudgetExceeded)?;
    if delta.is_negative() {
        // Conjugate roots. Rank three: the cubic and its chord, whose ends are conjugate. Rank two:
        // the four lines are two real skew lines and a conjugate pair.
        return Ok(if low {
            (Segre::TwoPairs, Topology { lines: 2, ..EMPTY })
        } else {
            (
                Segre::TwoDoubles,
                Topology {
                    cubics: 1,
                    lines: 1,
                    ..EMPTY
                },
            )
        });
    }
    if let Some([first, second]) = rational_roots(g, &delta)? {
        let (one, two) = (
            pencil.inertia(&first).ok_or(NumError::BudgetExceeded)?,
            pencil.inertia(&second).ok_or(NumError::BudgetExceeded)?,
        );
        if one.rank() != two.rank() {
            let (pair, other) = if one.rank() == 2 {
                (first, two)
            } else {
                (second, one)
            };
            let left = pencil.beside(&pair, true).ok_or(NumError::BudgetExceeded)?;
            let right = pencil
                .beside(&pair, false)
                .ok_or(NumError::BudgetExceeded)?;
            let topology = if left == right {
                Topology {
                    conics: 1,
                    lines: 2,
                    meets: 3,
                    ..EMPTY
                }
            } else if other.one_sided() {
                Topology { points: 1, ..EMPTY }
            } else {
                Topology {
                    conics: 1,
                    points: 1,
                    ..EMPTY
                }
            };
            return Ok((Segre::PairAndDouble, topology));
        }
    }
    if !low {
        return Ok((
            Segre::TwoDoubles,
            Topology {
                cubics: 1,
                lines: 1,
                meets: 2,
                ..EMPTY
            },
        ));
    }
    // Rank two at both: e₂ is the product-sum of the two eigenvalues left, negative where they differ.
    let second = pencil
        .minors
        .get(1)
        .and_then(widened)
        .ok_or(NumError::BudgetExceeded)?;
    let mixed = |up: bool| -> Result<bool, NumError> {
        let sign = sign_on_quadratic(&second, g, up).ok_or(NumError::BudgetExceeded)?;
        Ok(sign == Sign::Negative)
    };
    let real = u8::from(mixed(true)?).saturating_add(u8::from(mixed(false)?));
    Ok((
        Segre::TwoPairs,
        match real {
            2 => Topology {
                lines: 4,
                meets: 4,
                ..EMPTY
            },
            1 => Topology { points: 2, ..EMPTY },
            _ => EMPTY,
        },
    ))
}

/// The two roots of `g`, if its discriminant `delta` is a square.
fn rational_roots(g: &[Big; 3], delta: &Big) -> Result<Option<[At; 2]>, NumError> {
    let Some(root) = sturm::square_root(delta) else {
        return Ok(None);
    };
    let [_, g1, g2] = g;
    let made = (|| {
        let den = Big::from_i64(2).checked_mul(g2)?;
        Some([
            root_of(&g1.checked_sub(&root)?, &den)?,
            root_of(&g1.checked_add(&root)?, &den)?,
        ])
    })();
    made.map(Some).ok_or(NumError::BudgetExceeded)
}

/// Whether every minor of three rows of tC + S vanishes at the roots of `g`, the test of a rank of
/// two or less on roots that need not be real.
fn cofactors_vanish(c: &Form<Entry>, s: &Form<Entry>, g: &[Big; 3]) -> Option<bool> {
    let [g0, g1, g2] = g;
    for rows in ROWS {
        for columns in ROWS {
            // The minor as a cubic in t, row by row a choice of matrix.
            let mut terms = [Big::ZERO; 4];
            for mask in 0_u8..16 {
                if mask & 8 != 0 {
                    continue;
                }
                let picked = Rows {
                    first: s,
                    second: c,
                    mask: spread(rows, mask),
                };
                let power = usize::try_from(mask.count_ones()).ok()?;
                let value: Big = picked.three(rows, columns)?.resize()?;
                let slot = terms.get_mut(power)?;
                *slot = slot.checked_add(&value)?;
            }
            // Remainder by g: two steps of pseudo-division.
            let [t0, t1, t2, t3] = terms;
            let r2 = t2.checked_mul(g2)?.checked_sub(&t3.checked_mul(g1)?)?;
            let r1 = t1.checked_mul(g2)?.checked_sub(&t3.checked_mul(g0)?)?;
            let r0 = t0.checked_mul(g2)?;
            let low1 = r1.checked_mul(g2)?.checked_sub(&r2.checked_mul(g1)?)?;
            let low0 = r0.checked_mul(g2)?.checked_sub(&r2.checked_mul(g0)?)?;
            if !low1.is_zero() || !low0.is_zero() {
                return Some(false);
            }
        }
    }
    Some(true)
}

/// The rows of each minor of three rows of four, in the adjugate's order.
const ROWS: [[usize; 3]; 4] = [[1, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]];

/// A mask of positions among `rows`, as a mask of row numbers.
fn spread(rows: [usize; 3], mask: u8) -> u8 {
    let mut out = 0_u8;
    for (position, row) in rows.into_iter().enumerate() {
        if bit(mask, position) {
            out |= u32::try_from(row)
                .ok()
                .and_then(|r| 1_u8.checked_shl(r))
                .unwrap_or(0);
        }
    }
    out
}

/// The kernel of `form`, a primitive whole vector, for a rank of three: a row of its adjugate.
fn kernel(form: &Form<Entry>) -> Option<[Big; 4]> {
    let rows = Rows {
        first: form,
        second: form,
        mask: 0,
    };
    for out in ROWS {
        let mut vector = [Big::ZERO; 4];
        for (j, (slot, columns)) in vector.iter_mut().zip(ROWS).enumerate() {
            let minor: Big = rows.three(out, columns)?.resize()?;
            *slot = if j % 2 == 0 {
                minor
            } else {
                minor.checked_neg()?
            };
        }
        if vector.iter().any(|v| !v.is_zero()) {
            return primitive(&vector);
        }
    }
    None
}

/// `form · v`.
fn image(form: &Form<Entry>, v: &[Big; 4]) -> Option<[Big; 4]> {
    let mut out = [Big::ZERO; 4];
    for (slot, row) in out.iter_mut().zip(form) {
        for (cell, x) in row.iter().zip(v) {
            let cell: Big = cell.resize()?;
            *slot = slot.checked_add(&cell.checked_mul(x)?)?;
        }
    }
    Some(out)
}

/// The two quadrics modulo the shared kernel `v`: their forms on three coordinates, which is what
/// their base conics are. A padded row of zeros keeps them 4 × 4, so their inertia reads as before.
fn on_base(form: &Form<Entry>, v: &[Big; 4]) -> Option<Form<Entry>> {
    // The last part: a grid apex has a one there.
    let pivot = v.iter().rposition(|x| !x.is_zero())?;
    let lead = *v.get(pivot)?;
    let others: [usize; 3] = match pivot {
        0 => [1, 2, 3],
        1 => [0, 2, 3],
        2 => [0, 1, 3],
        _ => [0, 1, 2],
    };
    // Each new axis is lead·eⱼ − vⱼ·e_pivot, which keeps the numbers whole.
    let axis = |j: usize| -> Option<[Big; 4]> {
        let mut out = [Big::ZERO; 4];
        *out.get_mut(j)? = lead;
        *out.get_mut(pivot)? = v.get(j)?.checked_neg()?;
        Some(out)
    };
    let mut out = [[Entry::ZERO; 4]; 4];
    for (r, &i) in others.iter().enumerate() {
        let across = image(form, &axis(i)?)?;
        for (s, &j) in others.iter().enumerate() {
            let other = axis(j)?;
            let mut sum = Big::ZERO;
            for (x, y) in other.iter().zip(&across) {
                sum = sum.checked_add(&x.checked_mul(y)?)?;
            }
            *out.get_mut(r)?.get_mut(s)? = sum.resize()?;
        }
    }
    Some(out)
}

/// Whether the first three rows of `m` are independent.
fn clear_three(m: &Form<Entry>) -> Option<bool> {
    let rows = Rows {
        first: m,
        second: m,
        mask: 0,
    };
    Some(!rows.principal(7)?.is_zero())
}

/// One apex: the pencil of base conics, whose base points are the shared lines through it.
fn one_apex(a: &Form<Entry>, b: &Form<Entry>, v: &[Big; 4]) -> Result<Pencil, NumError> {
    let on = |f: &Form<Entry>| on_base(f, v).ok_or(NumError::BudgetExceeded);
    let (p, q) = (on(a)?, on(b)?);
    let Some((c, s)) = basis(&p, &q, clear_three)? else {
        return Err(NumError::Degenerate);
    };
    let pencil = Exact::new(&c, &s, 3)?;
    let last = pencil.chain.last();
    let (terms, degree) = last.ok_or(NumError::Degenerate)?;
    let apex = Topology { points: 1, ..EMPTY };
    let topology = match degree {
        0 if pencil.real == 3 && pencil.definite()? => apex,
        0 if pencil.real == 3 => Topology {
            lines: 4,
            meets: 1,
            ..EMPTY
        },
        0 => Topology {
            lines: 2,
            meets: 1,
            ..EMPTY
        },
        1 => {
            let [g0, g1, ..] = primitive(terms).ok_or(NumError::BudgetExceeded)?;
            let root = root_of(&g0, &g1).ok_or(NumError::BudgetExceeded)?;
            let i = pencil.inertia(&root).ok_or(NumError::BudgetExceeded)?;
            let left = pencil.beside(&root, true).ok_or(NumError::BudgetExceeded)?;
            let right = pencil
                .beside(&root, false)
                .ok_or(NumError::BudgetExceeded)?;
            match i.rank() {
                2 if i.one_sided() => Topology {
                    double_lines: 1,
                    ..EMPTY
                },
                2 => Topology {
                    double_lines: 1,
                    lines: 2,
                    meets: 1,
                    ..EMPTY
                },
                _ if left.rank() == 3 && left.one_sided() => apex,
                _ if right.rank() == 3 && right.one_sided() => apex,
                _ => Topology {
                    double_lines: 2,
                    meets: 1,
                    ..EMPTY
                },
            }
        }
        _ => {
            let [_, g1, g2, ..] = primitive(terms).ok_or(NumError::BudgetExceeded)?;
            let twice = Big::from_i64(2)
                .checked_mul(&g2)
                .ok_or(NumError::BudgetExceeded)?;
            let root = root_of(&g1, &twice).ok_or(NumError::BudgetExceeded)?;
            let i = pencil.inertia(&root).ok_or(NumError::BudgetExceeded)?;
            match i.rank() {
                2 => Topology {
                    lines: 2,
                    meets: 1,
                    ..EMPTY
                },
                1 => Topology {
                    double_lines: 1,
                    ..EMPTY
                },
                _ if pencil.definite()? => apex,
                _ => Topology {
                    same: true,
                    ..EMPTY
                },
            }
        }
    };
    Ok(Pencil {
        segre: Segre::OneApex,
        real_roots: pencil.real,
        topology,
    })
}

/// Whether `v` is a kernel vector of `form`.
fn holds(form: &Form<Entry>, v: &[Big; 4]) -> Option<bool> {
    Some(image(form, v)?.iter().all(Wide::is_zero))
}

/// Every member singular. A member of rank three has one kernel vector: all members share it, or
/// the kernel moves along a line of the two.
fn singular(a: &Form<Entry>, b: &Form<Entry>) -> Result<Pencil, NumError> {
    let mut found = None;
    for k in [0, 1, -1, 2, -2] {
        let member = plus_times(a, k, b).ok_or(NumError::BudgetExceeded)?;
        found = kernel(&member);
        if found.is_some() {
            break;
        }
    }
    let v = found.ok_or(NumError::Degenerate)?;
    let shared = holds(a, &v)
        .zip(holds(b, &v))
        .ok_or(NumError::BudgetExceeded)?;
    if shared.0 && shared.1 {
        return one_apex(a, b, &v);
    }
    Ok(Pencil {
        segre: Segre::TouchingCones,
        real_roots: 0,
        topology: Topology {
            conics: 1,
            double_lines: 1,
            meets: 1,
            ..EMPTY
        },
    })
}

/// Whether the minors of three rows all vanish twice at the fourfold root `a`: the third invariant
/// factor of tC + S is t for [(31)] and t² for [(22)], whose kernel the other member is zero on.
fn flat_on_kernel(c: &Form<Entry>, s: &Form<Entry>, a: &At) -> Option<bool> {
    let (p, q) = (&a.num, &a.den);
    let square = [
        p.checked_mul(p)?,
        Big::from_i64(-2).checked_mul(p)?.checked_mul(q)?,
        q.checked_mul(q)?,
    ];
    cofactors_vanish(c, s, &square)
}

/// What the two quadrics `a` and `b` share, exactly.
///
/// # Errors
///
/// [`NumError::Degenerate`] for a pair every member of whose pencil has a rank below three.
/// [`NumError::BudgetExceeded`] for values past the module header's budget, which a sphere, cylinder
/// or cone of 40-bit coordinates cannot reach.
pub fn classify(a: &Form<Entry>, b: &Form<Entry>) -> Result<Pencil, NumError> {
    let clear = |m: &Form<Entry>| Some(!determinant(m)?.is_zero());
    let Some((c, s)) = basis(a, b, clear)? else {
        return singular(a, b);
    };
    let pencil = Exact::new(&c, &s, 4)?;
    let (terms, degree) = pencil.chain.last().ok_or(NumError::Degenerate)?;
    let (segre, topology) = match degree {
        0 => (Segre::Simple, simple(&pencil)?),
        1 => {
            let [g0, g1, ..] = primitive(terms).ok_or(NumError::BudgetExceeded)?;
            let root = root_of(&g0, &g1).ok_or(NumError::BudgetExceeded)?;
            one_double(&pencil, &root)?
        }
        2 => {
            let [g0, g1, g2, ..] = primitive(terms).ok_or(NumError::BudgetExceeded)?;
            let delta = g1
                .checked_mul(&g1)
                .zip(
                    Big::from_i64(4)
                        .checked_mul(&g2)
                        .and_then(|v| v.checked_mul(&g0)),
                )
                .and_then(|(x, y)| x.checked_sub(&y))
                .ok_or(NumError::BudgetExceeded)?;
            if delta.is_zero() {
                let twice = Big::from_i64(2)
                    .checked_mul(&g2)
                    .ok_or(NumError::BudgetExceeded)?;
                let root = root_of(&g1, &twice).ok_or(NumError::BudgetExceeded)?;
                triple(&pencil, &root)?
            } else {
                two_doubles(&pencil, &c, &s, &[g0, g1, g2])?
            }
        }
        _ => {
            let [_, _, g2, g3, ..] = primitive(terms).ok_or(NumError::BudgetExceeded)?;
            let thrice = Big::from_i64(3)
                .checked_mul(&g3)
                .ok_or(NumError::BudgetExceeded)?;
            let root = root_of(&g2, &thrice).ok_or(NumError::BudgetExceeded)?;
            let rank = pencil
                .inertia(&root)
                .ok_or(NumError::BudgetExceeded)?
                .rank();
            let flat =
                rank == 2 && flat_on_kernel(&c, &s, &root).ok_or(NumError::BudgetExceeded)?;
            fourfold(&pencil, &root, flat)?
        }
    };
    Ok(Pencil {
        segre,
        real_roots: pencil.real,
        topology,
    })
}

/// The sum of the principal minors at a point, homogenised, in intervals.
fn homogeneous_interval(
    terms: &[Interval; TERMS],
    degree: usize,
    num: i64,
    den: i64,
) -> Option<Interval> {
    let (p, q) = (Interval::of(num)?, Interval::of(den)?);
    let mut power = Interval::of(1)?;
    let mut sum = Interval::of(0)?;
    for c in terms.iter().take(degree.checked_add(1)?).rev() {
        sum = sum.mul(p).ok()?.add(c.mul(power).ok()?).ok()?;
        power = power.mul(q).ok()?;
    }
    Some(sum)
}

/// Whether the member at (num : den) is definite, if intervals tell. All its sums of principal
/// minors are positive, or they alternate from a negative: one certain sign out of either pattern is
/// a certain no, and a sum that is truly zero, which no interval proves, needs no other answer.
fn definite_filtered(sums: &[[Interval; TERMS]; 4], num: i64, den: i64) -> Option<bool> {
    let (mut upward, mut downward) = (true, true);
    let (mut sure_up, mut sure_down) = (true, true);
    for (k, terms) in sums.iter().enumerate() {
        let sign = homogeneous_interval(terms, k.checked_add(1)?, num, den)?.certain_sign();
        let wanted = if k % 2 == 0 {
            Sign::Negative
        } else {
            Sign::Positive
        };
        upward &= sign.is_none_or(|s| s == Sign::Positive);
        downward &= sign.is_none_or(|s| s == wanted);
        sure_up &= sign == Some(Sign::Positive);
        sure_down &= sign == Some(wanted);
    }
    if sure_up || sure_down {
        Some(true)
    } else if upward || downward {
        None
    } else {
        Some(false)
    }
}

/// `f` with its largest bound scaled to one, so that a chain's products stay within a double.
fn scaled(f: &[Interval; TERMS]) -> Option<[Interval; TERMS]> {
    let mut top = 0.0_f64;
    for c in f {
        for end in [c.lo(), c.hi(), -c.lo(), -c.hi()] {
            top = if end > top { end } else { top };
        }
    }
    let scale = Interval::point(top).ok()?;
    let mut out = *f;
    for slot in &mut out {
        *slot = slot.div(scale).ok()?;
    }
    Some(out)
}

/// The finest walk around the line the filter tries before it gives way to the exact path.
const FILTER_STEPS: i64 = 1 << 14;

/// The point `i` of a walk of 2·`steps` around the line, by the diamond: t from 0 up to infinity, then
/// from minus infinity back to 0.
fn walked(i: i64, steps: i64) -> Option<(i64, i64)> {
    if i <= steps {
        Some((i, steps.checked_sub(i)?))
    } else {
        Some((i.checked_sub(steps.checked_mul(2)?)?, i.checked_sub(steps)?))
    }
}

/// A point of each of the four gaps between the four real roots of `f`, a quartic form whose sign is
/// the same at (p : q) and (−p : −q): the first point of each run of one sign on a walk of 2·`steps`
/// points. `None` unless the walk sees all four changes.
fn gaps(f: &[Interval; TERMS], steps: i64) -> Option<[(i64, i64); 5]> {
    let mut out = [(0_i64, 0_i64); 5];
    let (mut runs, mut first, mut last) = (0_usize, None, None);
    for i in 0..steps.checked_mul(2)? {
        let (p, q) = walked(i, steps)?;
        let Some(sign) = homogeneous_interval(f, 4, p, q)?.certain_sign() else {
            continue;
        };
        if sign == Sign::Zero {
            continue;
        }
        first = first.or(Some(sign));
        if last != Some(sign) {
            *out.get_mut(runs)? = (p, q);
            runs = runs.checked_add(1)?;
        }
        last = Some(sign);
    }
    // Around the loop the last run joins the first when they agree.
    let around = if first == last {
        runs.checked_sub(1)?
    } else {
        runs
    };
    (around == 4).then_some(out)
}

/// What the two quadrics share where intervals decide it: a pencil with four simple roots, each
/// sign proven by outward rounding. `None` otherwise, for the exact path or, off the grid, the
/// general paths.
#[must_use]
pub fn filtered(a: &Form<Interval>, b: &Form<Interval>) -> Option<Topology> {
    let clear = |m: &Form<Interval>| {
        let sign = determinant(m)?.certain_sign();
        Some(sign.is_some_and(|s| s != Sign::Zero))
    };
    let (c, s) = basis(a, b, clear).ok()??;
    let sums = minors(&c, &s)?;
    let f = scaled(sums.get(3)?)?;
    let roots = sturm::filtered(&f, 4)?;
    let smooth = |ovals: u8| {
        Some(Topology {
            quartic: Some(Quartic::Smooth { ovals }),
            ..EMPTY
        })
    };
    if roots.distinct != 4 {
        return smooth(if roots.distinct == 2 { 1 } else { 2 });
    }
    let mut steps = 64_i64;
    while steps <= FILTER_STEPS {
        if let Some(points) = gaps(&f, steps) {
            for (p, q) in points {
                if (p, q) != (0, 0) && definite_filtered(&sums, p, q)? {
                    return Some(EMPTY);
                }
            }
            return smooth(2);
        }
        steps = steps.checked_mul(4)?;
    }
    None
}
