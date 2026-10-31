//! Canonical planes: one plane, one entry, so that "is this face on that plane" is a comparison of
//! indices (23.2, step 1), and two operands on one plane give the arrangement one plane (11.3).
//!
//! What it does: [`canonical`] divides a plane's coefficients by their greatest common divisor and
//! fixes its sign, so every plane through the same points has one form; [`Canonical::flipped`]
//! says whether the sign turned, which a face records as the side that is material. A
//! [`PlaneTable`] holds one entry per canonical plane, each at the narrowest width that holds it,
//! and [`PlaneTable::insert`] answers with the entry's index and that flip.
//!
//! Guarantees:
//!
//! - **One form.** Two planes have equal canonical forms exactly when they are the same plane,
//!   whatever points made them and whichever way they face: the coefficients of a plane are fixed
//!   up to a common factor, and the reduction takes out all of it but the sign, which the first
//!   non-zero normal component fixes as positive.
//! - **Exact, and within budget.** Reduction only shrinks coefficients, so a canonical plane is
//!   within 6.2's budget if the plane was; [`Plane::new`] checks it again.
//! - **Deterministic.** A table's indices count first insertions, and it looks planes up by their
//!   coefficients in order, never by a hash.
//! - **Nothing aborts.** A table grows fallibly: [`TableError::OutOfMemory`] when the allocator
//!   refuses, [`TableError::Full`] past 2^32 planes.
//!
//! Method: canonical plane coefficients as chapter 23.2 and 23.9 fix them
//! (`projects/engines/geometry/architecture/23-performance.md`), on the plane-based geometry of
//! Nehring-Wirxel, Trettner and Kobbelt 2021, <https://doi.org/10.1016/j.cad.2021.103015>.
//!
//! Cost: [`canonical`] is five greatest common divisors of 128-bit values and a few divisions.
//! [`PlaneTable::insert`] adds one lookup, O(log n) in the planes held. A plane whose coefficients
//! all fit `i64` takes 32 bytes, any other 64, beside its index entry.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

use geom_math::gcd_u128;
use geom_num::NumError;
use geom_num::plane::Plane;

/// A plane in canonical form, and whether the sign turned on the way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canonical {
    /// The plane: its coefficients reduced, its first non-zero normal component positive.
    pub plane: Plane,
    /// Whether the normal points the other way from the plane given.
    pub flipped: bool,
}

/// `plane` in canonical form.
///
/// # Errors
///
/// What [`Plane::new`] gives, which a plane within budget cannot reach.
pub fn canonical(plane: &Plane) -> Result<Canonical, NumError> {
    let [x, y, z] = plane.n();
    let d = plane.d();
    let divisor = [x, y, z, d]
        .iter()
        .fold(0, |g, v| gcd_u128(g, v.unsigned_abs()));
    let sign = match [x, y, z].iter().find(|v| **v != 0) {
        Some(first) if *first < 0 => -1,
        _ => 1,
    };
    let reduce = |v: i128| -> Result<i128, NumError> {
        // The divisor is at most |v| where v is not zero, and a normal is never all zero, so it is
        // at least 1 and at most 2^125: it fits i128 and leaves v's sign alone.
        let by = i128::try_from(divisor).map_err(|_| NumError::BudgetExceeded)?;
        v.checked_div(by)
            .and_then(|q| q.checked_mul(sign))
            .ok_or(NumError::BudgetExceeded)
    };
    Ok(Canonical {
        plane: Plane::new([reduce(x)?, reduce(y)?, reduce(z)?], reduce(d)?)?,
        flipped: sign < 0,
    })
}

/// A row of a [`PlaneTable`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaneIx(u32);

impl PlaneIx {
    /// The row.
    #[must_use]
    pub const fn row(self) -> u32 {
        self.0
    }
}

/// A plane as a face holds it: its table entry and which way the face looks along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Facing {
    /// The entry.
    pub plane: PlaneIx,
    /// Whether the plane given faces the other way from its entry.
    pub flipped: bool,
}

/// Why a plane could not join a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableError {
    /// The numbers layer refused the plane.
    Num(NumError),
    /// The table holds 2^32 planes.
    Full,
    /// The allocator refused.
    OutOfMemory,
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableError::Num(e) => write!(f, "a plane the numbers layer refuses: {e:?}"),
            TableError::Full => f.write_str("a plane table holds 2^32 planes"),
            TableError::OutOfMemory => f.write_str("the allocator refused a plane table's room"),
        }
    }
}

impl core::error::Error for TableError {}

impl From<NumError> for TableError {
    fn from(e: NumError) -> Self {
        TableError::Num(e)
    }
}

/// Where a plane's coefficients are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stored {
    /// A row of the `i64` coefficients.
    Small(u32),
    /// A row of the `i128` coefficients.
    Wide(u32),
}

/// One entry per canonical plane, each at the narrowest width that holds it (23.9): within a body,
/// and across the operands of one operation (23.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaneTable {
    rows: Vec<Stored>,
    small: Vec<[i64; 4]>,
    wide: Vec<[i128; 4]>,
    index: BTreeMap<([i128; 3], i128), PlaneIx>,
}

/// `v` as a table row.
fn row_of(v: usize) -> Result<u32, TableError> {
    u32::try_from(v).map_err(|_| TableError::Full)
}

impl PlaneTable {
    /// An empty table.
    #[must_use]
    pub const fn new() -> Self {
        PlaneTable {
            rows: Vec::new(),
            small: Vec::new(),
            wide: Vec::new(),
            index: BTreeMap::new(),
        }
    }

    /// The planes it holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether it holds none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The entry of `plane`, new or already held, and which way `plane` faces along it.
    ///
    /// # Errors
    ///
    /// A [`TableError`]; the table is unchanged.
    pub fn insert(&mut self, plane: &Plane) -> Result<Facing, TableError> {
        let Canonical { plane, flipped } = canonical(plane)?;
        let key = (plane.n(), plane.d());
        if let Some(ix) = self.index.get(&key) {
            return Ok(Facing {
                plane: *ix,
                flipped,
            });
        }
        let row = row_of(self.rows.len())?;
        self.rows
            .try_reserve(1)
            .map_err(|_| TableError::OutOfMemory)?;
        let [x, y, z] = plane.n();
        let narrow = (
            i64::try_from(x),
            i64::try_from(y),
            i64::try_from(z),
            i64::try_from(plane.d()),
        );
        let stored = if let (Ok(x), Ok(y), Ok(z), Ok(d)) = narrow {
            self.small
                .try_reserve(1)
                .map_err(|_| TableError::OutOfMemory)?;
            let at = row_of(self.small.len())?;
            self.small.push([x, y, z, d]);
            Stored::Small(at)
        } else {
            self.wide
                .try_reserve(1)
                .map_err(|_| TableError::OutOfMemory)?;
            let at = row_of(self.wide.len())?;
            self.wide.push([x, y, z, plane.d()]);
            Stored::Wide(at)
        };
        self.rows.push(stored);
        let ix = PlaneIx(row);
        self.index.insert(key, ix);
        Ok(Facing { plane: ix, flipped })
    }

    /// The plane at `ix`, in canonical form.
    #[must_use]
    pub fn plane(&self, ix: PlaneIx) -> Option<Plane> {
        let at = usize::try_from(ix.0).ok()?;
        let [x, y, z, d] = match *self.rows.get(at)? {
            Stored::Small(row) => self.small.get(usize::try_from(row).ok()?)?.map(i128::from),
            Stored::Wide(row) => *self.wide.get(usize::try_from(row).ok()?)?,
        };
        Plane::new([x, y, z], d).ok()
    }

    /// How many planes fit `i64` and how many need `i128`.
    #[must_use]
    pub const fn widths(&self) -> (usize, usize) {
        (self.small.len(), self.wide.len())
    }
}
