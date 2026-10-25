//! A Verus-annotated twin of `geom-num`'s wide-integer arithmetic (`src/wide.rs`), for `verus` to
//! verify (chapter 28.7 of the kernel architecture; piece G37).
//!
//! It proves, for every width L from 1 to 4096 limbs, so for `I256`, `I320`, `I384` and `I512`:
//! `checked_add`, `checked_sub`, `checked_neg` and `checked_mul` give the exact two's-complement
//! result whenever it fits and `None` exactly when it does not, and so do `resize` and the
//! width-aware products `product_i128` and `checked_mul_i128` (piece G39); `compare`, which
//! `Ord::cmp` calls, orders values as integers; `from_i128` gives the value it is given (piece
//! G38); and the helpers they use do what each one's specification says.
//!
//! The functions below the `verus!` block, with their `#[verus_spec]` attributes and their
//! `proof!` and `proof_decl!` blocks removed, are token for token those of `src/wide.rs`:
//! `geom-gen`'s `tests/verus_twin.rs` compares them. `from_i128` and `product_i128`, associated
//! functions without `self`, which `#[verus_spec]` cannot annotate, are written inside the `verus!`
//! block instead, and the test compares each one's doc comment, attributes, visibility, parameters
//! and body. Also inside the block are the definitions
//! (the unsigned value of a limb sequence, two's complement, what fits), the lemmas, a model of
//! `Wide` with the same field, and four facts Verus is told about the standard library, each that
//! function's documented behaviour: `u64::overflowing_add`, `u64::overflowing_sub`,
//! `u64::from(bool)` and `i128::unsigned_abs`. Those, Verus and the Z3 solver it uses are what the
//! proof trusts.
//!
//! It is a module of `geom-num`'s Verus crate, `verus/lib.rs`, which the predicate twins share.
//! Verify: `verus --crate-type=lib projects/engines/geometry/crates/geom-num/verus/lib.rs`, with the
//! Verus release `kernel-dev.yml` pins; its `verus` job does this on every pull request.

use vstd::arithmetic::power2::*;
use core::cmp::Ordering;
use vstd::prelude::*;

verus! {

// ---- Trusted specifications of the standard library's integer methods the code calls.

pub assume_specification[ u64::overflowing_add ](x: u64, y: u64) -> (r: (u64, bool))
    ensures
        r.0 as int == if x as int + y as int >= 0x1_0000_0000_0000_0000 {
            x as int + y as int - 0x1_0000_0000_0000_0000
        } else {
            x as int + y as int
        },
        r.1 == (x as int + y as int >= 0x1_0000_0000_0000_0000),
;

pub assume_specification[ u64::overflowing_sub ](x: u64, y: u64) -> (r: (u64, bool))
    ensures
        r.0 as int == if (x as int - y as int) < 0 {
            x as int - y as int + 0x1_0000_0000_0000_0000
        } else {
            x as int - y as int
        },
        r.1 == ((x as int - y as int) < 0),
;

pub assume_specification[ <u64 as core::convert::From<bool>>::from ](b: bool) -> (r: u64)
    ensures
        r == if b { 1u64 } else { 0u64 },
;

pub assume_specification[ i128::unsigned_abs ](x: i128) -> (r: u128)
    ensures
        r as int == if x < 0 { -(x as int) } else { x as int },
;

// ---- The value of a limb sequence.

pub open spec fn unsigned(limbs: Seq<u64>, n: nat) -> int
    decreases n,
{
    if n == 0 {
        0
    } else {
        unsigned(limbs, (n - 1) as nat) + limbs[n - 1] as int * pow2((64 * (n - 1)) as nat) as int
    }
}

pub open spec fn base(n: nat) -> int {
    pow2((64 * n) as nat) as int
}

pub proof fn lemma_base_step(n: nat)
    ensures
        base(n + 1) == base(n) * 0x1_0000_0000_0000_0000,
        base(n) > 0,
{
    lemma_pow2_adds((64 * n) as nat, 64);
    lemma2_to64_rest();
    lemma_pow2_pos((64 * n) as nat);
}

pub proof fn lemma_unsigned_bound(s: Seq<u64>, n: nat)
    requires
        n <= s.len(),
    ensures
        0 <= unsigned(s, n) < base(n),
    decreases n,
{
    if n == 0 {
        lemma2_to64();
    } else {
        let m = (n - 1) as nat;
        lemma_unsigned_bound(s, m);
        lemma_base_step(m);
        let p = base(m);
        assert(unsigned(s, m) + s[m as int] as int * p < p * 0x1_0000_0000_0000_0000) by (nonlinear_arith)
            requires
                0 <= unsigned(s, m) < p,
                0 <= s[m as int] as int <= 0xFFFF_FFFF_FFFF_FFFF,
        ;
        assert(0 <= s[m as int] as int * p) by (nonlinear_arith)
            requires
                0 <= s[m as int] as int,
                0 < p,
        ;
    }
}

pub proof fn lemma_prefix_unchanged(s: Seq<u64>, j: nat, v: u64, n: nat)
    requires
        j < s.len(),
        n <= j,
    ensures
        unsigned(s.update(j as int, v), n) == unsigned(s, n),
    decreases n,
{
    if n > 0 {
        lemma_prefix_unchanged(s, j, v, (n - 1) as nat);
    }
}

/// The top bit of the top limb is set exactly when the unsigned value is at least half the base.
pub proof fn lemma_top_bit(s: Seq<u64>, n: nat)
    requires
        1 <= n <= s.len(),
    ensures
        (s[n - 1] >= 0x8000_0000_0000_0000) == (unsigned(s, n) >= base(n) / 2),
        base(n) == 2 * (base(n) / 2),
{
    let m = (n - 1) as nat;
    lemma_unsigned_bound(s, m);
    lemma_base_step(m);
    let p = base(m);
    let t = s[m as int] as int;
    let low = unsigned(s, m);
    assert(base(n) == p * 0x1_0000_0000_0000_0000);
    assert(base(n) / 2 == p * 0x8000_0000_0000_0000) by (nonlinear_arith)
        requires
            base(n) == p * 0x1_0000_0000_0000_0000,
    ;
    assert((t >= 0x8000_0000_0000_0000) == (low + t * p >= p * 0x8000_0000_0000_0000)) by (nonlinear_arith)
        requires
            0 <= low < p,
            0 <= t <= 0xFFFF_FFFF_FFFF_FFFF,
    ;
}

/// Two's complement: the signed value of n limbs.
pub open spec fn signed(s: Seq<u64>, n: nat) -> int {
    if unsigned(s, n) >= base(n) / 2 {
        unsigned(s, n) - base(n)
    } else {
        unsigned(s, n)
    }
}

/// Whether a value fits n limbs, two's complement.
pub open spec fn fits(v: int, n: nat) -> bool {
    -(base(n) / 2) <= v < base(n) / 2
}

/// The sign rule of two's-complement addition: with a wrapped sum r = ua + ub − c·2h, the true
/// sum of the signed values fits exactly when the operands' signs differ or the result's sign is
/// theirs, and then the result's signed value is the sum.
pub proof fn lemma_add_sign_rule(ua: int, ub: int, r: int, c: bool, h: int)
    requires
        h > 0,
        0 <= ua < 2 * h,
        0 <= ub < 2 * h,
        0 <= r < 2 * h,
        ua + ub == r + if c { 2 * h } else { 0 },
    ensures
        ({
            let va = if ua >= h { ua - 2 * h } else { ua };
            let vb = if ub >= h { ub - 2 * h } else { ub };
            let vr = if r >= h { r - 2 * h } else { r };
            let overflow = (ua >= h) == (ub >= h) && (r >= h) != (ua >= h);
            &&& (-h <= va + vb < h) == !overflow
            &&& !overflow ==> vr == va + vb
        }),
{
}

/// The same rule for subtraction: r = ua − ub + c·2h.
pub proof fn lemma_sub_sign_rule(ua: int, ub: int, r: int, c: bool, h: int)
    requires
        h > 0,
        0 <= ua < 2 * h,
        0 <= ub < 2 * h,
        0 <= r < 2 * h,
        ua - ub + (if c { 2 * h } else { 0 }) == r,
    ensures
        ({
            let va = if ua >= h { ua - 2 * h } else { ua };
            let vb = if ub >= h { ub - 2 * h } else { ub };
            let vr = if r >= h { r - 2 * h } else { r };
            let overflow = (ua >= h) != (ub >= h) && (r >= h) != (ua >= h);
            &&& (-h <= va - vb < h) == !overflow
            &&& !overflow ==> vr == va - vb
        }),
{
}

pub proof fn lemma_base_mul(i: nat, j: nat)
    ensures
        base(i + j) == base(i) * base(j),
{
    lemma_pow2_adds((64 * i) as nat, (64 * j) as nat);
    assert((64 * i) as nat + (64 * j) as nat == (64 * (i + j)) as nat);
}

pub proof fn lemma_base_monotone(i: nat, j: nat)
    requires
        i <= j,
    ensures
        base(i) <= base(j),
{
    if i < j {
        lemma_pow2_strictly_increases((64 * i) as nat, (64 * j) as nat);
    }
}

pub proof fn lemma_unsigned_prefix_le(s: Seq<u64>, n: nat, m: nat)
    requires
        n <= m <= s.len(),
    ensures
        unsigned(s, n) <= unsigned(s, m),
    decreases m - n,
{
    if n < m {
        let k = (m - 1) as nat;
        lemma_unsigned_prefix_le(s, n, k);
        lemma_base_step(k);
        assert(0 <= s[k as int] as int * base(k)) by (nonlinear_arith)
            requires
                0 <= s[k as int] as int,
                0 < base(k),
        ;
    }
}

/// Writing v at position k changes the value of the first n limbs (k < n) by (v − s[k]) · base(k).
pub proof fn lemma_update_value(s: Seq<u64>, k: nat, v: u64, n: nat)
    requires
        k < n <= s.len(),
    ensures
        unsigned(s.update(k as int, v), n) == unsigned(s, n) + (v as int - s[k as int] as int) * base(k),
    decreases n,
{
    let m = (n - 1) as nat;
    if m == k {
        lemma_prefix_unchanged(s, k, v, k);
        assert((v as int - s[k as int] as int) * base(k) == v as int * base(k) - s[k as int] as int * base(k)) by (nonlinear_arith);
    } else {
        lemma_update_value(s, k, v, m);
    }
}

/// The magnitude of a product is the product of the magnitudes, and it fits exactly as its signed
/// value does.
pub proof fn lemma_product_magnitude(x: int, y: int)
    ensures
        (if x * y < 0 { -(x * y) } else { x * y }) == (if x < 0 { -x } else { x }) * (if y < 0 { -y } else { y }),
        (x * y < 0) == ((x < 0) != (y < 0) && x != 0 && y != 0),
{
    assert((if x * y < 0 { -(x * y) } else { x * y }) == (if x < 0 { -x } else { x }) * (if y < 0 { -y } else { y })) by (nonlinear_arith);
    assert((x * y < 0) == ((x < 0) != (y < 0) && x != 0 && y != 0)) by (nonlinear_arith);
}

pub proof fn lemma_zero_limbs(s: Seq<u64>, n: nat)
    requires
        n <= s.len(),
        forall|k: int| 0 <= k < s.len() ==> s[k] == 0,
    ensures
        unsigned(s, n) == 0,
    decreases n,
{
    if n > 0 {
        lemma_zero_limbs(s, (n - 1) as nat);
        assert(s[n - 1] as int * base((n - 1) as nat) == 0) by (nonlinear_arith)
            requires s[n - 1] == 0;
    }
}


/// The magnitudes' product is at least what the rows so far and the current row up to limb j
/// account for, plus the current product at position i + j.
pub proof fn lemma_product_lower_bound(a: Seq<u64>, b: Seq<u64>, n: nat, i: nat, j: nat, ai: u64, bj: u64)
    requires
        i < n <= a.len(),
        j < n <= b.len(),
        a[i as int] == ai,
        b[j as int] == bj,
    ensures
        unsigned(a, n) * unsigned(b, n)
            >= unsigned(a, i) * unsigned(b, n) + ai as int * unsigned(b, j) * base(i)
                + ai as int * bj as int * base(i + j),
{
    lemma_unsigned_prefix_le(a, i + 1, n);
    lemma_unsigned_prefix_le(b, j + 1, n);
    lemma_unsigned_bound(a, i);
    lemma_unsigned_bound(b, j);
    lemma_base_mul(i, j);
    lemma_base_step(i);
    lemma_base_step(j);
    let (big_a, big_b) = (unsigned(a, n), unsigned(b, n));
    let (ai_, bj_) = (ai as int, bj as int);
    let (lo_a, lo_b) = (unsigned(a, i), unsigned(b, j));
    let (p_i, p_j) = (base(i), base(j));
    assert(unsigned(a, i + 1) == lo_a + ai_ * p_i);
    assert(unsigned(b, j + 1) == lo_b + bj_ * p_j);
    assert(big_a * big_b >= (lo_a + ai_ * p_i) * big_b) by (nonlinear_arith)
        requires big_a >= lo_a + ai_ * p_i, big_b >= 0, lo_a + ai_ * p_i >= 0;
    assert((lo_a + ai_ * p_i) * big_b >= lo_a * big_b + ai_ * p_i * (lo_b + bj_ * p_j)) by (nonlinear_arith)
        requires big_b >= lo_b + bj_ * p_j, lo_b + bj_ * p_j >= 0, lo_a >= 0, ai_ >= 0, p_i > 0;
    assert(ai_ * p_i * (lo_b + bj_ * p_j) == ai_ * lo_b * p_i + ai_ * bj_ * (p_i * p_j)) by (nonlinear_arith);
}

/// A product whose magnitude reaches the base does not fit.
pub proof fn lemma_too_big(x: int, y: int, mag_x: int, mag_y: int, n: nat)
    requires
        mag_x == (if x < 0 { -x } else { x }),
        mag_y == (if y < 0 { -y } else { y }),
        mag_x * mag_y >= base(n),
        base(n) > 0,
    ensures
        !fits(x * y, n),
{
    lemma_product_magnitude(x, y);
}

pub proof fn lemma_exit_at_or_above_l(
    a: Seq<u64>, b: Seq<u64>, out: Seq<u64>, i: nat, j: nat, ai: u64, bj: u64, carry: u64, prod: int, x: int, y: int,
)
    requires
        a.len() == b.len() == out.len(),
        i < a.len(),
        j < a.len(),
        i + j >= a.len(),
        a[i as int] == ai,
        b[j as int] == bj,
        prod == ai as int * bj as int,
        prod != 0 || carry != 0,
        unsigned(out, out.len()) + carry as int * base(i + j)
            == unsigned(a, i) * unsigned(b, b.len()) + ai as int * unsigned(b, j) * base(i),
        unsigned(a, a.len()) == (if x < 0 { -x } else { x }),
        unsigned(b, b.len()) == (if y < 0 { -y } else { y }),
    ensures
        !fits(x * y, a.len()),
{
    let n = a.len();
    lemma_product_lower_bound(a, b, n, i, j, ai, bj);
    lemma_unsigned_bound(out, n);
    lemma_base_monotone(n, i + j);
    lemma_base_step(n);
    let p = base(i + j);
    assert(prod >= 0) by (nonlinear_arith) requires prod == ai as int * bj as int, ai as int >= 0, bj as int >= 0;
    assert(carry as int * p + prod * p >= p) by (nonlinear_arith)
        requires prod >= 0, carry as int >= 0, prod != 0 || carry != 0, p > 0;
    assert(ai as int * bj as int * p == prod * p);
    lemma_too_big(x, y, unsigned(a, n), unsigned(b, n), n);
}

pub proof fn lemma_exit_at_carry(a: Seq<u64>, b: Seq<u64>, out: Seq<u64>, i: nat, ai: u64, carry: u64, x: int, y: int)
    requires
        a.len() == b.len() == out.len(),
        i < a.len(),
        a[i as int] == ai,
        carry != 0,
        unsigned(out, out.len()) + carry as int * base(i + b.len())
            == unsigned(a, i) * unsigned(b, b.len()) + ai as int * unsigned(b, b.len()) * base(i),
        unsigned(a, a.len()) == (if x < 0 { -x } else { x }),
        unsigned(b, b.len()) == (if y < 0 { -y } else { y }),
    ensures
        !fits(x * y, a.len()),
{
    let n = a.len();
    lemma_row_done(a, b, i, ai);
    lemma_unsigned_prefix_le(a, i + 1, n);
    lemma_unsigned_bound(out, n);
    lemma_unsigned_bound(b, n);
    lemma_base_monotone(n, i + n);
    lemma_base_step(n);
    let p = base(i + n);
    assert(carry as int * p >= p) by (nonlinear_arith) requires carry as int >= 1, p > 0;
    assert(unsigned(a, n) * unsigned(b, n) >= unsigned(a, i + 1) * unsigned(b, n)) by (nonlinear_arith)
        requires unsigned(a, n) >= unsigned(a, i + 1), unsigned(b, n) >= 0;
    lemma_too_big(x, y, unsigned(a, n), unsigned(b, n), n);
}

/// A finished row: the rows below i and row i make the rows below i + 1.
pub proof fn lemma_row_done(a: Seq<u64>, b: Seq<u64>, i: nat, ai: u64)
    requires
        i < a.len(),
        a[i as int] == ai,
    ensures
        unsigned(a, i) * unsigned(b, b.len()) + ai as int * unsigned(b, b.len()) * base(i)
            == unsigned(a, i + 1) * unsigned(b, b.len()),
{
    assert(unsigned(a, i + 1) == unsigned(a, i) + ai as int * base(i));
    assert(unsigned(a, i) * unsigned(b, b.len()) + ai as int * unsigned(b, b.len()) * base(i)
        == (unsigned(a, i) + ai as int * base(i)) * unsigned(b, b.len())) by (nonlinear_arith);
}


/// One step of a row: writing the low half of prod + old + carry at position k = i + j moves the
/// value of the limbs by (low − old) · base(k), and the high half carries to position k + 1, so the
/// limbs and the carry account for prod · base(k) more.
pub proof fn lemma_slot_step(
    before: Seq<u64>, after: Seq<u64>, k: nat, old: u64, low: u64, high: u64, carry: u64, prod: int, i: nat, j: nat,
)
    requires
        k == i + j,
        k < before.len(),
        after == before.update(k as int, low),
        before[k as int] == old,
        low as int + high as int * 0x1_0000_0000_0000_0000 == prod + old as int + carry as int,
    ensures
        unsigned(after, after.len()) + high as int * base(k + 1)
            == unsigned(before, before.len()) + carry as int * base(k) + prod * base(k),
{
    lemma_update_value(before, k, low, before.len());
    lemma_base_step(k);
    let p = base(k);
    assert((low as int - old as int) * p + high as int * (0x1_0000_0000_0000_0000 * p) == carry as int * p + prod * p) by (nonlinear_arith)
        requires low as int + high as int * 0x1_0000_0000_0000_0000 == prod + old as int + carry as int;
}


/// Where the product lands: with tot = |x| · |y| below the base b and h = b / 2, a magnitude below
/// h fits with either sign, and a magnitude of h or more fits only as −h.
pub proof fn lemma_mul_result(tot: int, b: int, x: int, y: int, negative: bool)
    requires
        b > 0,
        b == 2 * (b / 2),
        0 <= tot < b,
        tot == (if x < 0 { -x } else { x }) * (if y < 0 { -y } else { y }),
        negative == ((x < 0) != (y < 0)),
    ensures
        tot < b / 2 ==> -(b / 2) <= x * y < b / 2,
        tot < b / 2 && !negative ==> x * y == tot,
        tot < b / 2 && negative && tot == 0 ==> x * y == 0,
        tot < b / 2 && negative && tot != 0 ==> x * y == -tot,
        tot >= b / 2 ==> ((-(b / 2) <= x * y < b / 2) == (negative && tot == b / 2)),
        tot >= b / 2 && negative && tot == b / 2 ==> x * y == -(b / 2),
{
    lemma_product_magnitude(x, y);
    assert(tot == 0 ==> x == 0 || y == 0) by (nonlinear_arith)
        requires tot == (if x < 0 { -x } else { x }) * (if y < 0 { -y } else { y });
}


/// Limbs holding v's two's complement in their low two and its sign fill above have v's value.
pub proof fn lemma_from_i128(s: Seq<u64>, v: i128, low: u64, high: u64)
    requires
        2 <= s.len() <= 4096,
        s[0] == low,
        s[1] == high,
        low as int + high as int * 0x1_0000_0000_0000_0000 == (if v < 0 { v as int + 0x1_0000_0000_0000_0000_0000_0000_0000_0000 } else { v as int }),
        forall|k: int| 2 <= k < s.len() ==> s[k] == (if v < 0 { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 }),
    ensures
        signed(s, s.len()) == v as int,
{
    let n = s.len();
    lemma_fill_prefix(s, v, low, high, n);
    lemma_base_monotone(2, n);
    lemma_base_step(0);
    lemma_base_step(1);
    lemma2_to64();
    lemma_unsigned_bound(s, n);
}

/// The value of the first m limbs (m ≥ 2): v's two low halves, plus, for a negative v, the sign
/// fill's base(m) − base(2).
pub proof fn lemma_fill_prefix(s: Seq<u64>, v: i128, low: u64, high: u64, m: nat)
    requires
        2 <= m <= s.len(),
        s[0] == low,
        s[1] == high,
        forall|k: int| 2 <= k < s.len() ==> s[k] == (if v < 0 { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 }),
    ensures
        unsigned(s, m) == low as int + high as int * 0x1_0000_0000_0000_0000
            + (if v < 0 { base(m) - base(2) } else { 0 }),
    decreases m,
{
    lemma_base_step(0);
    lemma_base_step(1);
    lemma2_to64();
    if m == 2 {
        assert(unsigned(s, 0) == 0);
        assert(unsigned(s, 1) == unsigned(s, 0) + s[0] as int * pow2(0) as int);
        assert(unsigned(s, 2) == unsigned(s, 1) + s[1] as int * pow2(64) as int);
        assert(pow2(0) == 1 && pow2(64) == 0x1_0000_0000_0000_0000) by { lemma2_to64(); lemma2_to64_rest(); }
    } else {
        let k = (m - 1) as nat;
        lemma_fill_prefix(s, v, low, high, k);
        lemma_base_step(k);
        if v < 0 {
            assert(s[k as int] as int * base(k) == base(k) * 0x1_0000_0000_0000_0000 - base(k)) by (nonlinear_arith)
                requires s[k as int] as int == 0xFFFF_FFFF_FFFF_FFFF;
        } else {
            assert(s[k as int] as int * base(k) == 0) by (nonlinear_arith) requires s[k as int] as int == 0;
        }
    }
}

/// The limbs from m up to n of s, as a number of their own.
pub open spec fn part(s: Seq<u64>, m: nat, n: nat) -> int {
    unsigned(s.subrange(m as int, n as int), (n - m) as nat)
}

/// Two sequences that agree on their first n limbs have the same value there.
pub proof fn lemma_prefix_eq(s: Seq<u64>, t: Seq<u64>, n: nat)
    requires
        n <= s.len(),
        n <= t.len(),
        forall|j: int| 0 <= j < n ==> s[j] == t[j],
    ensures
        unsigned(s, n) == unsigned(t, n),
    decreases n,
{
    if n > 0 {
        lemma_prefix_eq(s, t, (n - 1) as nat);
    }
}

/// The value of the first n limbs splits at m: the first m limbs, plus base(m) times the limbs
/// from m to n, which are below base(n − m).
pub proof fn lemma_split(s: Seq<u64>, m: nat, n: nat)
    requires
        m <= n <= s.len(),
    ensures
        unsigned(s, n) == unsigned(s, m) + base(m) * part(s, m, n),
        0 <= part(s, m, n) < base((n - m) as nat),
    decreases n,
{
    let sub = s.subrange(m as int, n as int);
    lemma_unsigned_bound(sub, (n - m) as nat);
    if n == m {
        assert(part(s, m, n) == 0);
        assert(base(m) * 0 == 0) by (nonlinear_arith);
    } else {
        let k = (n - 1) as nat;
        lemma_split(s, m, k);
        let prev = s.subrange(m as int, k as int);
        lemma_prefix_eq(sub, prev, (k - m) as nat);
        assert(sub[(k - m) as int] == s[k as int]);
        assert(unsigned(sub, (n - m) as nat) == unsigned(sub, (k - m) as nat) + sub[(k - m) as int] as int * base((k - m) as nat));
        lemma_base_mul(m, (k - m) as nat);
        assert(m + (k - m) as nat == k);
        let (a, b, x, p) = (unsigned(s, m), part(s, m, k), s[k as int] as int, base((k - m) as nat));
        assert(base(m) * (b + x * p) == base(m) * b + x * (base(m) * p)) by (nonlinear_arith);
    }
}

/// Limbs from m to n that all equal the sign fill f make the number 0 (f = 0) or base(n − m) − 1
/// (f = all ones); and conversely, the number is one of those only when every limb is f.
pub proof fn lemma_part_fill(s: Seq<u64>, m: nat, n: nat, f: u64)
    requires
        m <= n <= s.len(),
        f == 0 || f == 0xFFFF_FFFF_FFFF_FFFFu64,
    ensures
        (forall|j: int| m <= j < n ==> s[j] == f)
            == (part(s, m, n) == if f == 0 { 0 } else { base((n - m) as nat) - 1 }),
    decreases n,
{
    lemma_split(s, m, n);
    if n > m {
        let k = (n - 1) as nat;
        lemma_part_fill(s, m, k, f);
        lemma_split(s, m, k);
        let sub = s.subrange(m as int, n as int);
        let prev = s.subrange(m as int, k as int);
        lemma_prefix_eq(sub, prev, (k - m) as nat);
        let p = base((k - m) as nat);
        lemma_base_step((k - m) as nat);
        assert((n - m) as nat == (k - m) as nat + 1);
        let lo = part(s, m, k);
        let x = s[k as int] as int;
        assert(part(s, m, n) == lo + x * p);
        lemma_unsigned_bound(prev, (k - m) as nat);
        if f == 0 {
            assert((lo + x * p == 0) == (lo == 0 && x == 0)) by (nonlinear_arith)
                requires 0 <= lo < p, 0 <= x, p > 0;
        } else {
            assert((lo + x * p == p * 0x1_0000_0000_0000_0000 - 1) == (lo == p - 1 && x == 0xFFFF_FFFF_FFFF_FFFF)) by (nonlinear_arith)
                requires 0 <= lo < p, 0 <= x <= 0xFFFF_FFFF_FFFF_FFFF, p > 0;
        }
    } else {
        assert(base(0) == 1) by {
            lemma2_to64();
        }
    }
}

/// If the signed value of L limbs fits M < L limbs, the limbs from M up are its sign fill.
pub proof fn lemma_fits_means_fill(u_l: int, u_m: int, h: int, bm: int, bk: int, bl: int, negative: bool)
    requires
        bm > 0,
        bk > 0,
        bl == bm * bk,
        bl == 2 * (bl / 2),
        bm == 2 * (bm / 2),
        bm <= bl / 2,
        0 <= u_m < bm,
        0 <= h < bk,
        u_l == u_m + bm * h,
        negative == (u_l >= bl / 2),
        -(bm / 2) <= (if negative { u_l - bl } else { u_l }) < bm / 2,
    ensures
        h == (if negative { bk - 1 } else { 0 }),
        negative == (u_m >= bm / 2),
{
    if negative {
        assert(bm * h > bl - bm - bm / 2 - 1) by (nonlinear_arith)
            requires u_l - bl >= -(bm / 2), u_l == u_m + bm * h, u_m < bm;
        assert(bm * (bk - 1) == bl - bm) by (nonlinear_arith) requires bl == bm * bk;
        assert(h > bk - 2) by (nonlinear_arith)
            requires bm * h > bl - bm - bm / 2 - 1, bm * (bk - 1) == bl - bm, bm > 0, bm == 2 * (bm / 2);
    } else {
        assert(bm * h < bm) by (nonlinear_arith) requires u_l < bm / 2, u_l == u_m + bm * h, u_m >= 0, bm > 0;
        assert(h < 1) by (nonlinear_arith) requires bm * h < bm, bm > 0;
    }
}

/// A dropped limb that is not the sign fill means the value does not fit M limbs.
pub proof fn lemma_resize_too_wide(s: Seq<u64>, l: nat, m: nat, negative: bool, k: nat)
    requires
        2 <= m < l <= s.len(),
        m <= k < l,
        negative == (unsigned(s, l) >= base(l) / 2),
        s[k as int] != (if negative { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 }),
    ensures
        !fits(signed(s, l), m),
{
    let f: u64 = if negative { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 };
    lemma_split(s, m, l);
    lemma_part_fill(s, m, l, f);
    lemma_unsigned_bound(s, m);
    lemma_unsigned_bound(s, l);
    lemma_base_mul(m, (l - m) as nat);
    lemma_base_monotone(m + 1, l);
    lemma_base_step(m);
    lemma_top_bit(s, l);
    lemma_top_bit(s, m);
    assert((m + (l - m) as nat) == l);
    if fits(signed(s, l), m) {
        lemma_fits_means_fill(unsigned(s, l), unsigned(s, m), part(s, m, l), base(m), base((l - m) as nat), base(l), negative);
        assert(s[k as int] == f);
    }
}

/// What the resized limbs hold: when widening, the value; when narrowing with every dropped limb
/// the sign fill, the value exactly when the sign survives, and otherwise a value that does not
/// fit.
pub proof fn lemma_resize_result(s: Seq<u64>, t: Seq<u64>, l: nat, m: nat, negative: bool)
    requires
        2 <= l <= 4096,
        2 <= m <= 4096,
        s.len() == l,
        t.len() == m,
        negative == (unsigned(s, l) >= base(l) / 2),
        forall|j: int| 0 <= j < m ==> t[j] == (if j < l { s[j] } else { if negative { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 } }),
        m < l ==> forall|j: int| m <= j < l ==> s[j] == (if negative { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 }),
    ensures
        ((unsigned(t, m) >= base(m) / 2) == negative) ==> fits(signed(s, l), m) && signed(t, m) == signed(s, l),
        ((unsigned(t, m) >= base(m) / 2) != negative) ==> !fits(signed(s, l), m),
{
    let f: u64 = if negative { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 };
    lemma_top_bit(s, l);
    lemma_top_bit(t, m);
    lemma_unsigned_bound(s, l);
    lemma_unsigned_bound(t, m);
    if m >= l {
        lemma_prefix_eq(t, s, l);
        lemma_split(t, l, m);
        lemma_part_fill(t, l, m, f);
        lemma_base_mul(l, (m - l) as nat);
        assert(l + (m - l) as nat == m);
        lemma_base_monotone(l, m);
        let bl = base(l);
        let bk = base((m - l) as nat);
        assert(bl * (if negative { bk - 1 } else { 0 }) == (if negative { base(m) - bl } else { 0 })) by (nonlinear_arith)
            requires base(m) == bl * bk;
    } else {
        lemma_prefix_eq(t, s, m);
        lemma_split(s, m, l);
        lemma_part_fill(s, m, l, f);
        lemma_base_mul(m, (l - m) as nat);
        assert(m + (l - m) as nat == l);
        lemma_base_monotone(m + 1, l);
        lemma_base_step(m);
        let bm = base(m);
        let bk = base((l - m) as nat);
        assert(bm * (if negative { bk - 1 } else { 0 }) == (if negative { base(l) - bm } else { 0 })) by (nonlinear_arith)
            requires base(l) == bm * bk;
    }
}

// ---- The width-aware products (piece G39).

/// A non-zero limb at position k ≥ l makes the value at least base(l).
pub proof fn lemma_limb_too_big(s: Seq<u64>, k: nat, l: nat)
    requires
        l <= k < s.len(),
        s[k as int] != 0,
    ensures
        unsigned(s, s.len()) >= base(l),
{
    lemma_unsigned_prefix_le(s, k + 1, s.len());
    lemma_unsigned_bound(s, k);
    lemma_base_monotone(l, k);
    lemma_base_step(k);
    assert(s[k as int] as int * base(k) >= base(k)) by (nonlinear_arith)
        requires s[k as int] as int >= 1, base(k) > 0;
}

/// Limbs copied from m into l limbs, zero above the end of m, with every limb of m from l on zero,
/// have the value of m.
pub proof fn lemma_copied_value(m: Seq<u64>, out: Seq<u64>, l: nat)
    requires
        out.len() == l,
        forall|j: int| 0 <= j < l ==> out[j] == (if j < m.len() { m[j] } else { 0u64 }),
        forall|j: int| l <= j < m.len() ==> m[j] == 0,
    ensures
        unsigned(out, l) == unsigned(m, m.len()),
{
    if m.len() <= l {
        lemma_split(out, m.len(), l);
        lemma_part_fill(out, m.len(), l, 0);
        assert(forall|j: int| m.len() <= j < l ==> out[j] == 0u64);
        lemma_prefix_eq(out, m, m.len());
        assert(base(m.len()) * 0 == 0) by (nonlinear_arith);
    } else {
        lemma_split(m, l, m.len());
        lemma_part_fill(m, l, m.len(), 0);
        lemma_prefix_eq(out, m, l);
        assert(base(l) * 0 == 0) by (nonlinear_arith);
    }
}

/// The sign rule of a product of magnitudes: with tot = |x| · |y|, x · y is −tot when exactly one
/// of x and y is negative, and tot otherwise.
pub proof fn lemma_signed_product(x: int, y: int, tot: int, negative: bool)
    requires
        tot == (if x < 0 { -x } else { x }) * (if y < 0 { -y } else { y }),
        negative == ((x < 0) != (y < 0)),
    ensures
        x * y == (if negative { -tot } else { tot }),
{
    lemma_product_magnitude(x, y);
    assert(tot == 0 ==> x == 0 || y == 0) by (nonlinear_arith)
        requires tot == (if x < 0 { -x } else { x }) * (if y < 0 { -y } else { y });
    assert(x == 0 || y == 0 ==> x * y == 0) by (nonlinear_arith);
}

/// The value of two limbs, and of their first one.
pub proof fn lemma_two_limbs(s: Seq<u64>)
    requires
        s.len() == 2,
    ensures
        unsigned(s, 0) == 0,
        unsigned(s, 1) == s[0] as int,
        unsigned(s, 2) == s[0] as int + s[1] as int * 0x1_0000_0000_0000_0000,
        base(0) == 1,
        base(1) == 0x1_0000_0000_0000_0000,
{
    lemma2_to64();
    lemma2_to64_rest();
    lemma_base_step(0);
    assert(unsigned(s, 1) == unsigned(s, 0) + s[0] as int * pow2(0) as int);
    assert(unsigned(s, 2) == unsigned(s, 1) + s[1] as int * pow2(64) as int);
}

/// The value of four limbs.
pub proof fn lemma_four_limbs(s: Seq<u64>)
    requires
        s.len() == 4,
    ensures
        unsigned(s, 4) == s[0] as int + s[1] as int * 0x1_0000_0000_0000_0000
            + (s[2] as int + s[3] as int * 0x1_0000_0000_0000_0000)
                * 0x1_0000_0000_0000_0000_0000_0000_0000_0000,
{
    lemma2_to64();
    lemma2_to64_rest();
    lemma_base_step(0);
    lemma_base_step(1);
    lemma_base_step(2);
    let w = 0x1_0000_0000_0000_0000int;
    assert(unsigned(s, 1) == unsigned(s, 0) + s[0] as int * base(0));
    assert(unsigned(s, 2) == unsigned(s, 1) + s[1] as int * base(1));
    assert(unsigned(s, 3) == unsigned(s, 2) + s[2] as int * base(2));
    assert(unsigned(s, 4) == unsigned(s, 3) + s[3] as int * base(3));
    assert(base(0) == 1);
    assert(base(1) == w);
    assert(base(2) == w * w);
    assert(base(3) == w * w * w);
    assert(w * w == 0x1_0000_0000_0000_0000_0000_0000_0000_0000);
    assert(unsigned(s, 0) == 0);
    let (s0, s1, s2, s3) = (s[0] as int, s[1] as int, s[2] as int, s[3] as int);
    let (b0, b1, b2, b3) = (base(0), base(1), base(2), base(3));
    assert(s0 * b0 == s0) by (nonlinear_arith) requires b0 == 1;
    assert(s1 * b1 == s1 * w) by (nonlinear_arith) requires b1 == w;
    assert(s2 * b2 == s2 * (w * w)) by (nonlinear_arith) requires b2 == w * w;
    assert(s3 * b3 == s3 * (w * w * w)) by (nonlinear_arith) requires b3 == w * w * w;
    assert(unsigned(s, 4) == s0 + s1 * w + s2 * (w * w) + s3 * (w * w * w));
    assert(s2 * (w * w) + s3 * (w * w * w) == (s2 + s3 * w) * 0x1_0000_0000_0000_0000_0000_0000_0000_0000) by (nonlinear_arith)
        requires w == 0x1_0000_0000_0000_0000;
}

/// Schoolbook rows of different lengths: the product of the values of c (nc limbs) and r (nr
/// limbs) is at least what rows 0 to s − 1 of r account for, plus row s up to column i, plus the
/// product at position s + i.
pub proof fn lemma_lower_bound_rows(r: Seq<u64>, nr: nat, c: Seq<u64>, nc: nat, s: nat, i: nat)
    requires
        s < nr <= r.len(),
        i < nc <= c.len(),
    ensures
        unsigned(c, nc) * unsigned(r, nr)
            >= unsigned(c, nc) * unsigned(r, s) + r[s as int] as int * unsigned(c, i) * base(s)
                + r[s as int] as int * c[i as int] as int * base(s + i),
{
    lemma_unsigned_prefix_le(r, s + 1, nr);
    lemma_unsigned_prefix_le(c, i + 1, nc);
    lemma_unsigned_bound(r, s);
    lemma_unsigned_bound(c, i);
    lemma_base_mul(s, i);
    lemma_base_step(s);
    lemma_base_step(i);
    let (big_r, big_c) = (unsigned(r, nr), unsigned(c, nc));
    let (rs, ci) = (r[s as int] as int, c[i as int] as int);
    let (lo_r, lo_c) = (unsigned(r, s), unsigned(c, i));
    let (p_s, p_i) = (base(s), base(i));
    assert(unsigned(r, s + 1) == lo_r + rs * p_s);
    assert(unsigned(c, i + 1) == lo_c + ci * p_i);
    assert(big_c * big_r >= big_c * (lo_r + rs * p_s)) by (nonlinear_arith)
        requires big_r >= lo_r + rs * p_s, big_c >= 0;
    assert(big_c * (lo_r + rs * p_s) >= big_c * lo_r + rs * p_s * (lo_c + ci * p_i)) by (nonlinear_arith)
        requires big_c >= lo_c + ci * p_i, lo_c + ci * p_i >= 0, rs >= 0, p_s > 0;
    assert(rs * p_s * (lo_c + ci * p_i) == rs * lo_c * p_s + rs * ci * (p_s * p_i)) by (nonlinear_arith);
}

/// Row s of `checked_mul_i128` reaching position L with something to add there: the product does
/// not fit L limbs.
pub proof fn lemma_exit_row(
    r: Seq<u64>, c: Seq<u64>, out: Seq<u64>, s: nat, i: nat, carry: u64, prod: int, x: int, y: int,
)
    requires
        r.len() == 2,
        c.len() == out.len(),
        s < 2,
        i < c.len(),
        i + s >= c.len(),
        prod == r[s as int] as int * c[i as int] as int,
        prod != 0 || carry != 0,
        unsigned(out, out.len()) + carry as int * base(i + s)
            == unsigned(c, c.len()) * unsigned(r, s) + r[s as int] as int * unsigned(c, i) * base(s),
        unsigned(c, c.len()) == (if x < 0 { -x } else { x }),
        unsigned(r, 2) == (if y < 0 { -y } else { y }),
    ensures
        !fits(x * y, c.len()),
{
    let n = c.len();
    lemma_lower_bound_rows(r, 2, c, n, s, i);
    lemma_unsigned_bound(out, n);
    lemma_base_monotone(n, i + s);
    lemma_base_step(n);
    let p = base(i + s);
    assert(s + i == i + s);
    assert(prod >= 0) by (nonlinear_arith)
        requires prod == r[s as int] as int * c[i as int] as int, r[s as int] as int >= 0, c[i as int] as int >= 0;
    assert(carry as int * p + prod * p >= p) by (nonlinear_arith)
        requires prod >= 0, carry as int >= 0, prod != 0 || carry != 0, p > 0;
    lemma_too_big(x, y, unsigned(c, n), unsigned(r, 2), n);
}

/// Row s of `checked_mul_i128` ending with a carry out of the top limb: the product does not fit.
pub proof fn lemma_exit_carry_row(r: Seq<u64>, c: Seq<u64>, out: Seq<u64>, s: nat, carry: u64, x: int, y: int)
    requires
        r.len() == 2,
        c.len() == out.len(),
        s < 2,
        carry != 0,
        unsigned(out, out.len()) + carry as int * base(c.len() + s)
            == unsigned(c, c.len()) * unsigned(r, s) + r[s as int] as int * unsigned(c, c.len()) * base(s),
        unsigned(c, c.len()) == (if x < 0 { -x } else { x }),
        unsigned(r, 2) == (if y < 0 { -y } else { y }),
    ensures
        !fits(x * y, c.len()),
{
    let n = c.len();
    lemma_unsigned_prefix_le(r, s + 1, 2);
    lemma_unsigned_bound(out, n);
    lemma_unsigned_bound(c, n);
    lemma_base_monotone(n, n + s);
    lemma_base_step(n);
    let p = base(n + s);
    assert(unsigned(r, s + 1) == unsigned(r, s) + r[s as int] as int * base(s));
    assert(unsigned(c, n) * unsigned(r, s) + r[s as int] as int * unsigned(c, n) * base(s)
        == unsigned(c, n) * unsigned(r, s + 1)) by (nonlinear_arith)
        requires unsigned(r, s + 1) == unsigned(r, s) + r[s as int] as int * base(s);
    assert(unsigned(c, n) * unsigned(r, 2) >= unsigned(c, n) * unsigned(r, s + 1)) by (nonlinear_arith)
        requires unsigned(r, 2) >= unsigned(r, s + 1), unsigned(c, n) >= 0;
    assert(carry as int * p >= p) by (nonlinear_arith) requires carry as int >= 1, p > 0;
    lemma_too_big(x, y, unsigned(c, n), unsigned(r, 2), n);
}

const TOP_BIT: u64 = 1 << 63;

#[derive(Clone, Copy)]
pub struct Wide<const L: usize>([u64; L]);

/// 256-bit signed integer: in-circle and in-sphere tests, three-plane denominators.
pub type I256 = Wide<4>;
/// 320-bit signed integer: three-plane point numerators.
pub type I320 = Wide<5>;
/// 384-bit signed integer: classifying a three-plane point against a plane.
pub type I384 = Wide<6>;

/// Half the base of four limbs: 2^255, the bound an `I256` value must stay below.
pub proof fn lemma_half_base_4()
    ensures
        base(4) / 2 == 57896044618658097711785492504343953926634992332820282019728792003956564819968,
{
    lemma_base_step(0);
    lemma_base_step(1);
    lemma_base_step(2);
    lemma_base_step(3);
    lemma2_to64();
}

/// Half the base of five limbs: 2^319, the bound an `I320` value must stay below.
pub proof fn lemma_half_base_5()
    ensures
        base(5) / 2 == 1067993517960455041197510853084776057301352261178326384973520803911109862890320275011481043468288,
{
    lemma_base_step(0);
    lemma_base_step(1);
    lemma_base_step(2);
    lemma_base_step(3);
    lemma_base_step(4);
    lemma2_to64();
}

/// Half the base of six limbs: 2^383, the bound an `I384` value must stay below.
pub proof fn lemma_half_base_6()
    ensures
        base(6) / 2 == 19701003098197239606139520050071806902539869635232723333974146702122860885748605305707133127442457820403313995153408,
{
    lemma_base_step(0);
    lemma_base_step(1);
    lemma_base_step(2);
    lemma_base_step(3);
    lemma_base_step(4);
    lemma_base_step(5);
    lemma2_to64();
}

impl<const L: usize> Wide<L> {
    /// The limbs, least significant first.
    pub closed spec fn limbs(self) -> Seq<u64> {
        self.0@
    }

    /// The signed value.
    pub closed spec fn value(self) -> int {
        signed(self.0@, L as nat)
    }
}


impl<const L: usize> Wide<L> {
    /// From an `i128`, exactly.
    #[must_use]
    pub fn from_i128(v: i128) -> (r: Self)
        requires
            2 <= L <= 4096,
        ensures
            r.value() == v as int,
    {
        let mut limbs = [fill(v < 0); L];
        // v's two's complement in 128 bits, read as unsigned: v itself when v is not negative, and
        // v + 2^128 when it is, made as (v − i128::MIN) + 2^127 so that no cast loses a value.
        #[allow(clippy::cast_sign_loss, reason = "each cast value is at least zero")]
        let u = if v < 0 {
            match v.checked_sub(i128::MIN) {
                Some(d) => (d as u128) | (1u128 << 127),
                None => 0,
            }
        } else {
            v as u128
        };
        let (low, high) = halves(u);
        if let Some(limb) = limbs.get_mut(0) {
            *limb = low;
        }
        if let Some(limb) = limbs.get_mut(1) {
            *limb = high;
        }
        proof {
            if v < 0 {
                let d = (v - i128::MIN) as u128;
                assert(d < 0x8000_0000_0000_0000_0000_0000_0000_0000u128);
                assert(d | (1u128 << 127) == d + 0x8000_0000_0000_0000_0000_0000_0000_0000u128) by (bit_vector)
                    requires d < 0x8000_0000_0000_0000_0000_0000_0000_0000u128;
            }
            lemma_from_i128(limbs@, v, low, high);
        }
        Self(limbs)
    }

    /// The exact product a · b of two `i128` values as a `Wide<L>`, or `None` if it does not fit:
    /// from four limbs it always fits, since |a · b| ≤ 2^127 · 2^127 = 2^254 < 2^255.
    ///
    /// The width-aware product of the generated exact paths (chapter 23.3): the same value as
    /// `Wide::from_i128(a).checked_mul(&Wide::from_i128(b))`, in four 64 × 64 → 128-bit limb
    /// products instead of [`Wide::checked_mul`]'s loops over every limb of two widened operands.
    ///
    /// Method and budget: the magnitudes' 64-bit halves |a| = a₁·2^64 + a₀ and |b| = b₁·2^64 + b₀
    /// multiply pairwise (Knuth, volume 2, section 4.3.1). Each partial product is at most
    /// (2^64 − 1)², below 2^128, so `checked_mul` on `u128` never fails. Limb 1 sums the high half
    /// of a₀b₀ and the low halves of a₀b₁ and a₁b₀: at most 3 (2^64 − 1) < 2^66, its carry at most
    /// 2. Limbs 2 and 3 sum a₁b₁, the high halves of a₀b₁ and a₁b₀ (each at most 2^64 − 2) and that
    /// carry: at most (2^64 − 1)² + 2 (2^64 − 2) + 2 = 2^128 − 1, so no `checked_add` fails either.
    /// The sign is applied last, as [`Wide::checked_mul`] applies it.
    #[must_use]
    pub(crate) fn product_i128(a: i128, b: i128) -> (r: Option<Self>)
        requires
            1 <= L <= 4096,
        ensures
            match r {
                Some(w) => fits(a as int * b as int, L as nat) && w.value() == a as int * b as int,
                None => !fits(a as int * b as int, L as nat),
            },
    {
        let negative = (a < 0) != (b < 0);
        let (a0, a1) = halves(a.unsigned_abs());
        let (b0, b1) = halves(b.unsigned_abs());
        proof {
            let m = 0xFFFF_FFFF_FFFF_FFFFint;
            assert(a0 as int * b0 as int <= m * m) by (nonlinear_arith) requires 0 <= a0 as int <= m, 0 <= b0 as int <= m;
            assert(a0 as int * b1 as int <= m * m) by (nonlinear_arith) requires 0 <= a0 as int <= m, 0 <= b1 as int <= m;
            assert(a1 as int * b0 as int <= m * m) by (nonlinear_arith) requires 0 <= a1 as int <= m, 0 <= b0 as int <= m;
            assert(a1 as int * b1 as int <= m * m) by (nonlinear_arith) requires 0 <= a1 as int <= m, 0 <= b1 as int <= m;
            assert(m * m == 0xFFFF_FFFF_FFFF_FFFE_0000_0000_0000_0001);
        }
        let p00 = u128::from(a0).checked_mul(u128::from(b0))?;
        let p01 = u128::from(a0).checked_mul(u128::from(b1))?;
        let p10 = u128::from(a1).checked_mul(u128::from(b0))?;
        let p11 = u128::from(a1).checked_mul(u128::from(b1))?;
        let (l0, h00) = halves(p00);
        let (l01, h01) = halves(p01);
        let (l10, h10) = halves(p10);
        let middle = u128::from(h00)
            .checked_add(u128::from(l01))?
            .checked_add(u128::from(l10))?;
        let (l1, carry) = halves(middle);
        proof {
            let w = 0x1_0000_0000_0000_0000int;
            // A high half of a partial product is at most 2^64 − 2, and the carry at most 2.
            assert((w - 1) * (w - 1) == 0xFFFF_FFFF_FFFF_FFFE_0000_0000_0000_0001);
            if h01 as int > w - 2 {
                assert(h01 as int * w >= (w - 1) * w) by (nonlinear_arith)
                    requires h01 as int >= w - 1, w == 0x1_0000_0000_0000_0000;
            }
            if h10 as int > w - 2 {
                assert(h10 as int * w >= (w - 1) * w) by (nonlinear_arith)
                    requires h10 as int >= w - 1, w == 0x1_0000_0000_0000_0000;
            }
            if carry as int > 2 {
                assert(carry as int * w >= 3 * w) by (nonlinear_arith)
                    requires carry as int >= 3, w == 0x1_0000_0000_0000_0000;
            }
        }
        let upper = p11
            .checked_add(u128::from(h01))?
            .checked_add(u128::from(h10))?
            .checked_add(u128::from(carry))?;
        let (l2, l3) = halves(upper);
        proof {
            let w = 0x1_0000_0000_0000_0000int;
            let (ma, mb) = (a0 as int + a1 as int * w, b0 as int + b1 as int * w);
            assert(ma * mb == p00 as int + p01 as int * w + p10 as int * w + p11 as int * (w * w)) by (nonlinear_arith)
                requires
                    p00 as int == a0 as int * b0 as int,
                    p01 as int == a0 as int * b1 as int,
                    p10 as int == a1 as int * b0 as int,
                    p11 as int == a1 as int * b1 as int,
                    ma == a0 as int + a1 as int * w,
                    mb == b0 as int + b1 as int * w;
            assert(l0 as int + l1 as int * w + (l2 as int + l3 as int * w) * (w * w)
                == p00 as int + p01 as int * w + p10 as int * w + p11 as int * (w * w)) by (nonlinear_arith)
                requires
                    l0 as int + h00 as int * w == p00 as int,
                    l01 as int + h01 as int * w == p01 as int,
                    l10 as int + h10 as int * w == p10 as int,
                    l1 as int + carry as int * w == h00 as int + l01 as int + l10 as int,
                    l2 as int + l3 as int * w == p11 as int + h01 as int + h10 as int + carry as int;
            lemma_four_limbs(seq![l0, l1, l2, l3]);
            assert(w * w == 0x1_0000_0000_0000_0000_0000_0000_0000_0000);
            lemma_signed_product(a as int, b as int, ma * mb, negative);
        }
        from_magnitude(&[l0, l1, l2, l3], negative)
    }
}

} // verus!

/// Limb-wise addition with carry; the carry out of the top limb is dropped (two's complement
/// wrap), and the caller decides overflow from the signs.
#[verus_spec(out =>
    ensures
        unsigned(out@, L as nat) == unsigned(a@, L as nat) + unsigned(b@, L as nat)
            - if unsigned(a@, L as nat) + unsigned(b@, L as nat) >= base(L as nat) { base(L as nat) } else { 0 },
)]
fn wrapping_add_limbs<const L: usize>(a: &[u64; L], b: &[u64; L]) -> [u64; L] {
    let mut out = [0u64; L];
    let mut carry = false;
    #[verus_spec(
        invariant
            unsigned(out@, i as nat) + (if carry { base(i as nat) } else { 0 })
                == unsigned(a@, i as nat) + unsigned(b@, i as nat),
    )]
    for i in 0..L {
        let x = match a.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let y = match b.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let (s1, c1) = x.overflowing_add(y);
        let (s2, c2) = s1.overflowing_add(u64::from(carry));
        proof! {
            lemma_prefix_unchanged(out@, i as nat, s2, i as nat);
            lemma_base_step(i as nat);
            let p = base(i as nat);
            let cin: int = if carry { 1 } else { 0 };
            let cout: int = if c1 || c2 { 1 } else { 0 };
            assert(x as int + y as int + cin == s2 as int + cout * 0x1_0000_0000_0000_0000);
            assert((x as int + y as int + cin) * p == s2 as int * p + cout * (0x1_0000_0000_0000_0000 * p)) by (nonlinear_arith)
                requires x as int + y as int + cin == s2 as int + cout * 0x1_0000_0000_0000_0000;
            assert(x as int * p + y as int * p + cin * p == (x as int + y as int + cin) * p) by (nonlinear_arith);
        }
        if let Some(o) = out.get_mut(i) {
            *o = s2;
        }
        proof! {
            assert(unsigned(out@, (i + 1) as nat) == unsigned(out@, i as nat) + s2 as int * base(i as nat));
            assert(unsigned(a@, (i + 1) as nat) == unsigned(a@, i as nat) + x as int * base(i as nat));
            assert(unsigned(b@, (i + 1) as nat) == unsigned(b@, i as nat) + y as int * base(i as nat));
        }
        carry = c1 || c2;
    }
    proof! {
        lemma_unsigned_bound(out@, L as nat);
        lemma_unsigned_bound(a@, L as nat);
        lemma_unsigned_bound(b@, L as nat);
    }
    out
}
/// Limb-wise subtraction with borrow; the borrow out of the top limb is dropped, and the caller
/// decides overflow from the signs.
#[verus_spec(out =>
    ensures
        unsigned(out@, L as nat) == unsigned(a@, L as nat) - unsigned(b@, L as nat)
            + if unsigned(a@, L as nat) < unsigned(b@, L as nat) { base(L as nat) } else { 0 },
)]
fn wrapping_sub_limbs<const L: usize>(a: &[u64; L], b: &[u64; L]) -> [u64; L] {
    let mut out = [0u64; L];
    let mut borrow = false;
    #[verus_spec(
        invariant
            unsigned(out@, i as nat) == unsigned(a@, i as nat) - unsigned(b@, i as nat)
                + (if borrow { base(i as nat) } else { 0 }),
    )]
    for i in 0..L {
        let x = match a.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let y = match b.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let (d1, b1) = x.overflowing_sub(y);
        let (d2, b2) = d1.overflowing_sub(u64::from(borrow));
        proof! {
            lemma_prefix_unchanged(out@, i as nat, d2, i as nat);
            lemma_base_step(i as nat);
            let p = base(i as nat);
            let bin: int = if borrow { 1 } else { 0 };
            let bout: int = if b1 || b2 { 1 } else { 0 };
            assert(x as int - y as int - bin == d2 as int - bout * 0x1_0000_0000_0000_0000);
            assert((x as int - y as int - bin) * p == d2 as int * p - bout * (0x1_0000_0000_0000_0000 * p)) by (nonlinear_arith)
                requires x as int - y as int - bin == d2 as int - bout * 0x1_0000_0000_0000_0000;
            assert(x as int * p - y as int * p - bin * p == (x as int - y as int - bin) * p) by (nonlinear_arith);
        }
        if let Some(o) = out.get_mut(i) {
            *o = d2;
        }
        proof! {
            assert(unsigned(out@, (i + 1) as nat) == unsigned(out@, i as nat) + d2 as int * base(i as nat));
            assert(unsigned(a@, (i + 1) as nat) == unsigned(a@, i as nat) + x as int * base(i as nat));
            assert(unsigned(b@, (i + 1) as nat) == unsigned(b@, i as nat) + y as int * base(i as nat));
        }
        borrow = b1 || b2;
    }
    proof! {
        lemma_unsigned_bound(out@, L as nat);
        lemma_unsigned_bound(a@, L as nat);
        lemma_unsigned_bound(b@, L as nat);
    }
    out
}

/// How two limb sequences compare as unsigned numbers: from the top limb down, the first limb that
/// differs decides.
#[verus_spec(r =>
    ensures
        (r == Ordering::Less) == (unsigned(a@, L as nat) < unsigned(b@, L as nat)),
        (r == Ordering::Greater) == (unsigned(a@, L as nat) > unsigned(b@, L as nat)),
        (r == Ordering::Equal) == (unsigned(a@, L as nat) == unsigned(b@, L as nat)),
)]
fn compare_limbs<const L: usize>(a: &[u64; L], b: &[u64; L]) -> Ordering {
    let mut i = L;
    #[verus_spec(
        invariant
            i <= L,
            a@.len() == L,
            b@.len() == L,
            unsigned(a@, L as nat) - unsigned(a@, i as nat) == unsigned(b@, L as nat) - unsigned(b@, i as nat),
        decreases i,
    )]
    while i > 0 {
        // i > 0, so i − 1 never wraps.
        i = i.wrapping_sub(1);
        let high_a = match a.get(i) {
            Some(v) => *v,
            None => 0,
        };
        let high_b = match b.get(i) {
            Some(v) => *v,
            None => 0,
        };
        proof! {
            lemma_unsigned_bound(a@, i as nat);
            lemma_unsigned_bound(b@, i as nat);
            lemma_base_step(i as nat);
            let p = base(i as nat);
            let (ua, ub) = (unsigned(a@, i as nat), unsigned(b@, i as nat));
            assert(unsigned(a@, (i + 1) as nat) == ua + high_a as int * p);
            assert(unsigned(b@, (i + 1) as nat) == ub + high_b as int * p);
            assert(high_a < high_b ==> ua + high_a as int * p < ub + high_b as int * p) by (nonlinear_arith)
                requires 0 <= ua < p, 0 <= ub < p, 0 <= high_a as int, 0 <= high_b as int;
            assert(high_a > high_b ==> ua + high_a as int * p > ub + high_b as int * p) by (nonlinear_arith)
                requires 0 <= ua < p, 0 <= ub < p, 0 <= high_a as int, 0 <= high_b as int;
        }
        if high_a != high_b {
            return if high_a < high_b { Ordering::Less } else { Ordering::Greater };
        }
    }
    Ordering::Equal
}

/// The low and high 64-bit halves of `t`, t = high · 2^64 + low.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the low half is t masked to 64 bits and the high half t shifted down 64, each below 2^64"
)]
#[verus_spec(r =>
    ensures
        r.0 as int + r.1 as int * 0x1_0000_0000_0000_0000 == t as int,
)]
fn halves(t: u128) -> (u64, u64) {
    proof! {
        assert((t & 0xFFFF_FFFF_FFFF_FFFFu128) + (t >> 64u128) * 0x1_0000_0000_0000_0000u128 == t) by (bit_vector);
        assert((t & 0xFFFF_FFFF_FFFF_FFFFu128) < 0x1_0000_0000_0000_0000u128) by (bit_vector);
        assert((t >> 64u128) < 0x1_0000_0000_0000_0000u128) by (bit_vector);
    }
    ((t & 0xFFFF_FFFF_FFFF_FFFF) as u64, (t >> 64) as u64)
}

/// The limb that sign-extends a value with this sign.
#[verus_spec(r =>
    ensures
        r == if negative { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 },
)]
const fn fill(negative: bool) -> u64 {
    if negative { u64::MAX } else { 0 }
}

/// The value with magnitude `m` (limbs little-endian, any number of them) and the given sign, or
/// `None` if it does not fit: the limbs from `L` on must be zero, and the magnitude must be below
/// 2^(64L−1), or exactly 2^(64L−1) for a negative value (the minimum value), as
/// [`Wide::checked_mul`] decides it.
#[verus_spec(r =>
    requires
        1 <= L <= 4096,
    ensures
        ({
            let v = if negative { -unsigned(m@, m@.len()) } else { unsigned(m@, m@.len()) };
            match r {
                Some(w) => fits(v, L as nat) && w.value() == v,
                None => !fits(v, L as nat),
            }
        }),
)]
fn from_magnitude<const L: usize>(m: &[u64], negative: bool) -> Option<Wide<L>> {
    let mut out = [0u64; L];
    #[verus_spec(
        invariant
            1 <= L <= 4096,
            out@.len() == L,
            forall|j: int| 0 <= j < L ==> out@[j] == (if j < k && j < m@.len() { m@[j] } else { 0u64 }),
            forall|j: int| L <= j < k ==> m@[j] == 0,
    )]
    for k in 0..m.len() {
        let limb = match m.get(k) {
            Some(v) => *v,
            None => 0,
        };
        if k < L {
            if let Some(o) = out.get_mut(k) {
                *o = limb;
            }
        } else if limb != 0 {
            proof! {
                lemma_limb_too_big(m@, k as nat, L as nat);
                lemma_base_step(L as nat);
                lemma_unsigned_bound(m@, m@.len());
            }
            return None;
        }
    }
    proof! {
        lemma_copied_value(m@, out@, L as nat);
    }
    let v = Wide::<L>(out);
    proof! {
        lemma_top_bit(v.limbs(), L as nat);
        lemma_unsigned_bound(v.limbs(), L as nat);
    }
    if !v.is_negative() {
        return Some(if negative { v.wrapping_neg() } else { v });
    }
    // Top bit set: only the exact magnitude 2^(64L−1) with a negative sign fits (the minimum
    // value), the one non-zero magnitude that negation leaves unchanged.
    let n = v.wrapping_neg();
    proof! {
        let tot = unsigned(v.limbs(), L as nat);
        assert(tot >= base(L as nat) / 2);
        assert(tot > 0);
        assert(unsigned(n.limbs(), L as nat) == base(L as nat) - tot);
        assert((base(L as nat) - tot == tot) == (tot == base(L as nat) / 2));
        assert(tot == base(L as nat) / 2 ==> n.value() == -(base(L as nat) / 2));
    }
    if negative && matches!(compare_limbs(&n.0, &v.0), Ordering::Equal) {
        proof! {
            assert(unsigned(n.limbs(), L as nat) == unsigned(v.limbs(), L as nat));
        }
        Some(n)
    } else {
        None
    }
}

impl<const L: usize> Wide<L> {
    /// Two's-complement negation, wrapping at the minimum value: the complement of every limb, plus
    /// one, carried limb by limb.
    #[verus_spec(r =>
        ensures
            unsigned(r.limbs(), L as nat) == if unsigned(self.limbs(), L as nat) == 0 { 0 } else {
                base(L as nat) - unsigned(self.limbs(), L as nat)
            },
    )]
    fn wrapping_neg(&self) -> Self {
        let mut out = [0u64; L];
        let mut carry = true;
        #[verus_spec(
            invariant
                unsigned(out@, i as nat) + (if carry { base(i as nat) } else { 0 })
                    == base(i as nat) - unsigned(self.limbs(), i as nat),
        )]
        for i in 0..L {
            let x = match self.0.get(i) {
                Some(v) => *v,
                None => 0,
            };
            let (s, c) = (!x).overflowing_add(u64::from(carry));
            proof! {
                lemma_prefix_unchanged(out@, i as nat, s, i as nat);
                lemma_base_step(i as nat);
                assert(!x == 0xFFFF_FFFF_FFFF_FFFFu64 - x) by (bit_vector);
                let p = base(i as nat);
                let cin: int = if carry { 1 } else { 0 };
                let cout: int = if c { 1 } else { 0 };
                let nx = 0xFFFF_FFFF_FFFF_FFFF - x as int;
                assert(nx + cin == s as int + cout * 0x1_0000_0000_0000_0000);
                assert((nx + cin) * p == s as int * p + cout * (0x1_0000_0000_0000_0000 * p)) by (nonlinear_arith)
                    requires nx + cin == s as int + cout * 0x1_0000_0000_0000_0000;
                assert((nx + cin) * p == 0x1_0000_0000_0000_0000 * p - p - x as int * p + cin * p) by (nonlinear_arith)
                    requires nx == 0xFFFF_FFFF_FFFF_FFFF - x as int;
            }
            if let Some(o) = out.get_mut(i) {
                *o = s;
            }
            carry = c;
        }
        proof! {
            lemma_unsigned_bound(out@, L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
        }
        Self(out)
    }

    /// The magnitude as an unsigned `64 × L`-bit number (the minimum value's magnitude, 2^(64L−1),
    /// fits as unsigned).
    #[verus_spec(r =>
        requires
            L >= 1,
        ensures
            unsigned(r@, L as nat) == if self.value() < 0 { -self.value() } else { self.value() },
    )]
    fn magnitude(&self) -> [u64; L] {
        proof! {
            lemma_top_bit(self.limbs(), L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
        }
        if self.is_negative() {
            self.wrapping_neg().0
        } else {
            self.0
        }
    }

    /// `self + other`, or `None` if the true sum does not fit.
    #[must_use]
    #[verus_spec(r =>
        requires
            L >= 1,
        ensures
            match r {
                Some(w) => fits(self.value() + other.value(), L as nat) && w.value() == self.value() + other.value(),
                None => !fits(self.value() + other.value(), L as nat),
            },
    )]
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        let r = Self(wrapping_add_limbs(&self.0, &other.0));
        proof! {
            lemma_top_bit(self.limbs(), L as nat);
            lemma_top_bit(other.limbs(), L as nat);
            lemma_top_bit(r.limbs(), L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
            lemma_unsigned_bound(other.limbs(), L as nat);
            lemma_unsigned_bound(r.limbs(), L as nat);
            let ua = unsigned(self.limbs(), L as nat);
            let ub = unsigned(other.limbs(), L as nat);
            let ur = unsigned(r.limbs(), L as nat);
            lemma_add_sign_rule(ua, ub, ur, ua + ub >= base(L as nat), base(L as nat) / 2);
        }
        // Overflow happens exactly when both operands share a sign and the result does not.
        let overflow =
            self.is_negative() == other.is_negative() && r.is_negative() != self.is_negative();
        if overflow { None } else { Some(r) }
    }

    /// `self − other`, or `None` if the true difference does not fit.
    #[must_use]
    #[verus_spec(r =>
        requires
            L >= 1,
        ensures
            match r {
                Some(w) => fits(self.value() - other.value(), L as nat) && w.value() == self.value() - other.value(),
                None => !fits(self.value() - other.value(), L as nat),
            },
    )]
    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        let r = Self(wrapping_sub_limbs(&self.0, &other.0));
        proof! {
            lemma_top_bit(self.limbs(), L as nat);
            lemma_top_bit(other.limbs(), L as nat);
            lemma_top_bit(r.limbs(), L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
            lemma_unsigned_bound(other.limbs(), L as nat);
            lemma_unsigned_bound(r.limbs(), L as nat);
            let ua = unsigned(self.limbs(), L as nat);
            let ub = unsigned(other.limbs(), L as nat);
            let ur = unsigned(r.limbs(), L as nat);
            lemma_sub_sign_rule(ua, ub, ur, ua < ub, base(L as nat) / 2);
        }
        // Overflow happens exactly when the operands differ in sign and the result's sign is not
        // self's.
        let overflow =
            self.is_negative() != other.is_negative() && r.is_negative() != self.is_negative();
        if overflow { None } else { Some(r) }
    }

    /// `−self`, or `None` for the minimum value, whose negation does not fit.
    #[must_use]
    #[verus_spec(r =>
        requires
            L >= 1,
        ensures
            match r {
                Some(w) => fits(-self.value(), L as nat) && w.value() == -self.value(),
                None => !fits(-self.value(), L as nat),
            },
    )]
    pub fn checked_neg(&self) -> Option<Self> {
        let r = self.wrapping_neg();
        proof! {
            lemma_top_bit(self.limbs(), L as nat);
            lemma_top_bit(r.limbs(), L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
            lemma_unsigned_bound(r.limbs(), L as nat);
        }
        // Only the minimum value is negative and stays negative when negated.
        let overflow = self.is_negative() && r.is_negative();
        if overflow { None } else { Some(r) }
    }

    /// How this value compares with `other`: by sign first, and for the same sign by the limbs,
    /// since two's-complement values of one sign order like their unsigned bit patterns.
    #[verus_spec(r =>
        requires
            L >= 1,
        ensures
            (r == Ordering::Less) == (self.value() < other.value()),
            (r == Ordering::Greater) == (self.value() > other.value()),
            (r == Ordering::Equal) == (self.value() == other.value()),
    )]
    fn compare(&self, other: &Self) -> Ordering {
        proof! {
            lemma_top_bit(self.limbs(), L as nat);
            lemma_top_bit(other.limbs(), L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
            lemma_unsigned_bound(other.limbs(), L as nat);
        }
        match (self.is_negative(), other.is_negative()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => compare_limbs(&self.0, &other.0),
        }
    }

    /// `self × other`, or `None` if the true product does not fit.
    #[must_use]
    #[verus_spec(r =>
        requires
            1 <= L <= 4096,
        ensures
            match r {
                Some(w) => fits(self.value() * other.value(), L as nat) && w.value() == self.value() * other.value(),
                None => !fits(self.value() * other.value(), L as nat),
            },
    )]
    pub fn checked_mul(&self, other: &Self) -> Option<Self> {
        let negative = self.is_negative() != other.is_negative();
        let a = self.magnitude();
        let b = other.magnitude();
        let mut out = [0u64; L];
        proof! {
            lemma_unsigned_bound(a@, L as nat);
            lemma_unsigned_bound(b@, L as nat);
            lemma_product_magnitude(self.value(), other.value());
            lemma_top_bit(self.limbs(), L as nat);
            lemma_top_bit(other.limbs(), L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
            lemma_unsigned_bound(other.limbs(), L as nat);
            assert(unsigned(out@, 0) == 0);
            lemma_zero_limbs(out@, L as nat);
        }
        #[verus_spec(
            invariant
                1 <= L <= 4096,
                a@.len() == L,
                b@.len() == L,
                out@.len() == L,
                unsigned(a@, L as nat) == (if self.value() < 0 { -self.value() } else { self.value() }),
                unsigned(b@, L as nat) == (if other.value() < 0 { -other.value() } else { other.value() }),
                unsigned(out@, L as nat) == unsigned(a@, i as nat) * unsigned(b@, L as nat),
        )]
        for i in 0..L {
            let ai = match a.get(i) {
                Some(v) => *v,
                None => 0,
            };
            if ai != 0 {
                let mut carry = 0u64;
                #[verus_spec(
                    invariant
                        1 <= L <= 4096,
                        a@.len() == L,
                        b@.len() == L,
                        out@.len() == L,
                        i < L,
                        ai == a@[i as int],
                        unsigned(a@, L as nat) == (if self.value() < 0 { -self.value() } else { self.value() }),
                        unsigned(b@, L as nat) == (if other.value() < 0 { -other.value() } else { other.value() }),
                        unsigned(out@, L as nat) + carry as int * base((i + j) as nat)
                            == unsigned(a@, i as nat) * unsigned(b@, L as nat)
                                + ai as int * unsigned(b@, j as nat) * base(i as nat),
                )]
                for j in 0..L {
                    let bj = match b.get(j) {
                        Some(v) => *v,
                        None => 0,
                    };
                    proof! {
                        assert(ai as int * bj as int <= 0xFFFF_FFFF_FFFF_FFFE_0000_0000_0000_0001) by (nonlinear_arith)
                            requires 0 <= ai as int <= 0xFFFF_FFFF_FFFF_FFFF, 0 <= bj as int <= 0xFFFF_FFFF_FFFF_FFFF;
                        lemma_base_mul(i as nat, j as nat);
                        lemma_base_step((i + j) as nat);
                        lemma_base_step(j as nat);
                        lemma_unsigned_bound(b@, j as nat);
                        lemma_unsigned_bound(out@, L as nat);
                        let p_i = base(i as nat);
                        let p_j = base(j as nat);
                        assert(ai as int * (unsigned(b@, j as nat) + bj as int * p_j) * p_i
                            == ai as int * unsigned(b@, j as nat) * p_i + ai as int * bj as int * (p_i * p_j)) by (nonlinear_arith);
                    }
                    // At most (2^64 − 1)², below 2^128: `checked_mul` never fails here.
                    let prod = u128::from(ai).checked_mul(u128::from(bj))?;
                    // Limb indices are below L, so their sum is below 2L: never fails.
                    let k = i.checked_add(j)?;
                    match out.get(k) {
                        Some(&old) => {
                            // old and carry are below 2^64 and prod at most (2^64 − 1)², so the
                            // sum is at most (2^64 − 1)² + 2 (2^64 − 1) = 2^128 − 1: never fails.
                            let t = prod.checked_add(u128::from(old))?.checked_add(u128::from(carry))?;
                            let (low, high) = halves(t);
                            proof_decl! {
                                let ghost before = out@;
                            }
                            if let Some(slot) = out.get_mut(k) {
                                *slot = low;
                            }
                            proof! {
                                lemma_slot_step(before, out@, k as nat, old, low, high, carry, prod as int, i as nat, j as nat);
                            }
                            carry = high;
                        }
                        // Any non-zero contribution at or above limb L makes the magnitude at least
                        // 2^(64L).
                        None => {
                            if prod != 0 || carry != 0 {
                                proof! {
                                    lemma_exit_at_or_above_l(a@, b@, out@, i as nat, j as nat, ai, bj, carry, prod as int, self.value(), other.value());
                                }
                                return None;
                            }
                            proof! {
                                assert(prod * base((i + j) as nat) == 0) by (nonlinear_arith) requires prod == 0;
                                assert(carry as int * base((i + j + 1) as nat) == 0) by (nonlinear_arith) requires carry == 0;
                            }
                        }
                    }
                }
                if carry != 0 {
                    proof! {
                        lemma_exit_at_carry(a@, b@, out@, i as nat, ai, carry, self.value(), other.value());
                    }
                    return None;
                }
                proof! {
                    lemma_row_done(a@, b@, i as nat, ai);
                }
            }
            proof! {
                if ai == 0 {
                    assert(unsigned(a@, (i + 1) as nat) == unsigned(a@, i as nat) + ai as int * base(i as nat));
                    assert(ai as int * base(i as nat) == 0) by (nonlinear_arith) requires ai == 0;
                }
            }
        }
        // The magnitude fits in 64 × L unsigned bits. It must also fit the signed range: below
        // 2^(64L−1) for a positive result, at most 2^(64L−1) for a negative one.
        let m = Self(out);
        proof! {
            lemma_top_bit(m.limbs(), L as nat);
            lemma_unsigned_bound(m.limbs(), L as nat);
            lemma_mul_result(unsigned(m.limbs(), L as nat), base(L as nat), self.value(), other.value(), negative);
        }
        if !m.is_negative() {
            return Some(if negative { m.wrapping_neg() } else { m });
        }
        // Top bit set: only the exact magnitude 2^(64L−1) with a negative sign fits (the minimum
        // value), the one non-zero magnitude that negation leaves unchanged.
        let n = m.wrapping_neg();
        proof! {
            let tot = unsigned(m.limbs(), L as nat);
            assert(tot >= base(L as nat) / 2);
            assert(tot > 0);
            assert(unsigned(n.limbs(), L as nat) == base(L as nat) - tot);
            assert((base(L as nat) - tot == tot) == (tot == base(L as nat) / 2));
            assert(tot == base(L as nat) / 2 ==> n.value() == -(base(L as nat) / 2));
        }
        if negative && matches!(compare_limbs(&n.0, &m.0), Ordering::Equal) {
            proof! {
                assert(unsigned(n.limbs(), L as nat) == unsigned(m.limbs(), L as nat));
                assert(fits(self.value() * other.value(), L as nat));
                assert(n.value() == self.value() * other.value());
            }
            Some(n)
        } else {
            proof! {
                assert(!fits(self.value() * other.value(), L as nat));
            }
            None
        }
    }

    /// `self × b` for an `i128` b, or `None` if the true product does not fit.
    ///
    /// The width-aware product of the generated exact paths (chapter 23.3): the same value as
    /// `self.checked_mul(&Wide::from_i128(b))`, by schoolbook multiplication of the magnitude's
    /// `L` limbs by the two 64-bit halves of |b| (Knuth, volume 2, section 4.3.1), with no
    /// widening of b and no pass over its sign-extension limbs. Each limb step's bound is the one
    /// [`Wide::checked_mul`] writes beside its own: a product at most (2^64 − 1)², plus a limb and
    /// a carry, at most 2^128 − 1.
    #[must_use]
    #[verus_spec(r =>
        requires
            1 <= L <= 4096,
        ensures
            match r {
                Some(w) => fits(self.value() * b as int, L as nat) && w.value() == self.value() * b as int,
                None => !fits(self.value() * b as int, L as nat),
            },
    )]
    pub(crate) fn checked_mul_i128(&self, b: i128) -> Option<Self> {
        let negative = self.is_negative() != (b < 0);
        let a = self.magnitude();
        let (b0, b1) = halves(b.unsigned_abs());
        let mut out = [0u64; L];
        proof_decl! {
            let ghost bs: Seq<u64> = seq![b0, b1];
        }
        proof! {
            lemma_unsigned_bound(a@, L as nat);
            lemma_top_bit(self.limbs(), L as nat);
            lemma_unsigned_bound(self.limbs(), L as nat);
            lemma_two_limbs(bs);
            lemma_zero_limbs(out@, L as nat);
            assert(unsigned(out@, L as nat) == unsigned(a@, L as nat) * unsigned(bs, 0)) by (nonlinear_arith)
                requires unsigned(out@, L as nat) == 0, unsigned(bs, 0) == 0;
        }
        #[verus_spec(
            invariant
                1 <= L <= 4096,
                a@.len() == L,
                out@.len() == L,
                bs.len() == 2,
                bs[0] == b0,
                bs[1] == b1,
                unsigned(a@, L as nat) == (if self.value() < 0 { -self.value() } else { self.value() }),
                unsigned(bs, 2) == (if b < 0 { -(b as int) } else { b as int }),
                unsigned(out@, L as nat) == unsigned(a@, L as nat) * unsigned(bs, shift as nat),
        )]
        for shift in 0..2 {
            let bj = if shift == 0 { b0 } else { b1 };
            proof! {
                assert(unsigned(bs, (shift + 1) as nat) == unsigned(bs, shift as nat) + bj as int * base(shift as nat));
            }
            if bj != 0 {
                let mut carry = 0u64;
                proof! {
                    assert(bj as int * unsigned(a@, 0) * base(shift as nat) == 0) by (nonlinear_arith)
                        requires unsigned(a@, 0) == 0;
                    assert(carry as int * base(shift as nat) == 0) by (nonlinear_arith) requires carry == 0;
                }
                #[verus_spec(
                    invariant
                        1 <= L <= 4096,
                        a@.len() == L,
                        out@.len() == L,
                        shift < 2,
                        bs.len() == 2,
                        bj == bs[shift as int],
                        unsigned(a@, L as nat) == (if self.value() < 0 { -self.value() } else { self.value() }),
                        unsigned(bs, 2) == (if b < 0 { -(b as int) } else { b as int }),
                        unsigned(out@, L as nat) + carry as int * base((i + shift) as nat)
                            == unsigned(a@, L as nat) * unsigned(bs, shift as nat)
                                + bj as int * unsigned(a@, i as nat) * base(shift as nat),
                )]
                for i in 0..L {
                    let ai = match a.get(i) {
                        Some(v) => *v,
                        None => 0,
                    };
                    proof! {
                        assert(ai as int * bj as int <= 0xFFFF_FFFF_FFFF_FFFE_0000_0000_0000_0001) by (nonlinear_arith)
                            requires 0 <= ai as int <= 0xFFFF_FFFF_FFFF_FFFF, 0 <= bj as int <= 0xFFFF_FFFF_FFFF_FFFF;
                        lemma_base_mul(shift as nat, i as nat);
                        lemma_base_step(i as nat);
                        lemma_unsigned_bound(a@, i as nat);
                        lemma_unsigned_bound(out@, L as nat);
                        let p_s = base(shift as nat);
                        let p_i = base(i as nat);
                        assert(unsigned(a@, (i + 1) as nat) == unsigned(a@, i as nat) + ai as int * p_i);
                        assert(bj as int * (unsigned(a@, i as nat) + ai as int * p_i) * p_s
                            == bj as int * unsigned(a@, i as nat) * p_s + ai as int * bj as int * (p_s * p_i)) by (nonlinear_arith);
                        assert((shift + i) as nat == (i + shift) as nat);
                    }
                    // At most (2^64 − 1)², below 2^128: `checked_mul` never fails here.
                    let prod = u128::from(ai).checked_mul(u128::from(bj))?;
                    // Limb indices are below L and the shift at most 1: never fails.
                    let k = i.checked_add(shift)?;
                    match out.get(k) {
                        Some(&old) => {
                            // At most (2^64 − 1)² + 2 (2^64 − 1) = 2^128 − 1: never fails.
                            let t = prod.checked_add(u128::from(old))?.checked_add(u128::from(carry))?;
                            let (low, high) = halves(t);
                            proof_decl! {
                                let ghost before = out@;
                            }
                            if let Some(slot) = out.get_mut(k) {
                                *slot = low;
                            }
                            proof! {
                                lemma_slot_step(before, out@, k as nat, old, low, high, carry, prod as int, shift as nat, i as nat);
                            }
                            carry = high;
                        }
                        // A non-zero contribution at limb L makes the magnitude at least 2^(64L).
                        None => {
                            if prod != 0 || carry != 0 {
                                proof! {
                                    lemma_exit_row(bs, a@, out@, shift as nat, i as nat, carry, prod as int, self.value(), b as int);
                                }
                                return None;
                            }
                            proof! {
                                assert(prod * base((i + shift) as nat) == 0) by (nonlinear_arith) requires prod == 0;
                                assert(carry as int * base((i + shift + 1) as nat) == 0) by (nonlinear_arith) requires carry == 0;
                            }
                        }
                    }
                }
                if carry != 0 {
                    proof! {
                        lemma_exit_carry_row(bs, a@, out@, shift as nat, carry, self.value(), b as int);
                    }
                    return None;
                }
                proof! {
                    assert(unsigned(a@, L as nat) * unsigned(bs, shift as nat) + bj as int * unsigned(a@, L as nat) * base(shift as nat)
                        == unsigned(a@, L as nat) * unsigned(bs, (shift + 1) as nat)) by (nonlinear_arith)
                        requires unsigned(bs, (shift + 1) as nat) == unsigned(bs, shift as nat) + bj as int * base(shift as nat);
                    assert(carry as int * base((L + shift) as nat) == 0) by (nonlinear_arith) requires carry == 0;
                }
            }
            proof! {
                if bj == 0 {
                    assert(bj as int * base(shift as nat) == 0) by (nonlinear_arith) requires bj == 0;
                }
            }
        }
        proof! {
            lemma_signed_product(self.value(), b as int, unsigned(out@, L as nat), negative);
        }
        from_magnitude(&out, negative)
    }

    /// The same value in a `Wide<M>`, or `None` if it does not fit.
    #[must_use]
    #[verus_spec(r =>
        requires
            2 <= L <= 4096,
            2 <= M <= 4096,
        ensures
            match r {
                Some(w) => fits(self.value(), M as nat) && w.value() == self.value(),
                None => !fits(self.value(), M as nat),
            },
    )]
    pub fn resize<const M: usize>(&self) -> Option<Wide<M>> {
        let negative = self.is_negative();
        let sign = fill(negative);
        let mut out = [sign; M];
        // The limbs both widths hold, copied; above them, the sign fill.
        #[verus_spec(
            invariant
                out@.len() == M,
                self.limbs().len() == L,
                forall|j: int| 0 <= j < i ==> out@[j] == (if j < L { self.limbs()[j] } else { sign }),
                forall|j: int| i <= j < M ==> out@[j] == sign,
        )]
        for i in 0..M {
            if i < L {
                let x = match self.0.get(i) {
                    Some(v) => *v,
                    None => 0,
                };
                if let Some(slot) = out.get_mut(i) {
                    *slot = x;
                }
            }
        }
        if M < L {
            // Narrowing: every dropped limb must be pure sign extension.
            #[verus_spec(
                invariant
                    2 <= M < L <= 4096,
                    M <= i <= L,
                    self.limbs().len() == L,
                    negative == (unsigned(self.limbs(), L as nat) >= base(L as nat) / 2),
                    sign == (if negative { 0xFFFF_FFFF_FFFF_FFFFu64 } else { 0u64 }),
                    forall|j: int| M <= j < i ==> self.limbs()[j] == sign,
            )]
            for i in M..L {
                let x = match self.0.get(i) {
                    Some(v) => *v,
                    None => 0,
                };
                if x != sign {
                    proof! {
                        lemma_resize_too_wide(self.limbs(), L as nat, M as nat, negative, i as nat);
                    }
                    return None;
                }
            }
        }
        let r = Wide::<M>(out);
        proof! {
            lemma_resize_result(self.limbs(), r.limbs(), L as nat, M as nat, negative);
        }
        // The sign must survive.
        if r.is_negative() != negative {
            return None;
        }
        Some(r)
    }

    /// Whether the value is below zero: whether the top limb's top bit is set.
    #[must_use]
    #[verus_spec(r =>
        requires
            L >= 1,
        ensures
            r == (unsigned(self.limbs(), L as nat) >= base(L as nat) / 2),
    )]
    pub fn is_negative(&self) -> bool {
        proof! {
            assert(TOP_BIT == 0x8000_0000_0000_0000u64) by (bit_vector);
            lemma_top_bit(self.limbs(), L as nat);
            assert(forall|x: u64| (x & 0x8000_0000_0000_0000u64 != 0) == (x >= 0x8000_0000_0000_0000u64)) by (bit_vector);
        }
        match self.0.last() {
            Some(top) => top & TOP_BIT != 0,
            None => false,
        }
    }
}

