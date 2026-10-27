//! The model of `geom_num::grid`'s types that the predicate twins verify against (chapter 28.7;
//! pieces G36 and G38): a coordinate is an `i64` its type keeps on the grid, |v| ≤ 2^40 − 1, and
//! `get` returns it; `P2` and `P3` hold two and three of them. `geom-gen`'s `tests/verus_twin.rs`
//! checks the bound against `geom_num::grid::COORD_MAX`. Also the lemma every predicate twin uses
//! before a product.

use vstd::prelude::*;

verus! {

/// The grid's bound, `geom_num::grid::COORD_MAX`: every coordinate has magnitude at most this.
pub const COORD_MAX: i64 = 1099511627775;

/// A model of `geom_num::grid::Coord`: an `i64` that its type keeps on the grid.
#[derive(Clone, Copy)]
pub struct Coord {
    v: i64,
}

impl Coord {
    /// Its value, as an integer.
    pub closed spec fn view(self) -> int {
        self.v as int
    }

    /// Whether it is on the grid, as every `geom_num::grid::Coord` is.
    pub closed spec fn on_grid(self) -> bool {
        -COORD_MAX <= self.v <= COORD_MAX
    }

    /// `geom_num::grid::Coord::get`: its value.
    pub fn get(self) -> (r: i64)
        ensures
            r == self.view(),
            self.on_grid() ==> -COORD_MAX <= r <= COORD_MAX,
    {
        self.v
    }
}

/// A model of `geom_num::grid::P2`.
#[derive(Clone, Copy)]
pub struct P2 {
    pub x: Coord,
    pub y: Coord,
}

/// A model of `geom_num::grid::P3`.
#[derive(Clone, Copy)]
pub struct P3 {
    pub x: Coord,
    pub y: Coord,
    pub z: Coord,
}

/// Two factors of magnitude at most mx and my have a product of magnitude at most mx · my.
pub proof fn lemma_product_bound(x: int, y: int, mx: int, my: int)
    requires
        0 <= mx,
        0 <= my,
        -mx <= x <= mx,
        -my <= y <= my,
    ensures
        -(mx * my) <= x * y <= mx * my,
{
    assert(-(mx * my) <= x * y <= mx * my) by (nonlinear_arith)
        requires
            0 <= mx,
            0 <= my,
            -mx <= x <= mx,
            -my <= y <= my,
    ;
}

} // verus!
