# Method note: outward widening of intervals in round-to-nearest arithmetic

| Field | Value |
| --- | --- |
| Source | CGAL's interval arithmetic support, `FPU.h`: its widening of a rounded bound with the next representable double when the rounding mode cannot be changed |
| Licence | LGPL-3.0-or-later OR LicenseRef-Commercial |
| Ledger row | Chapter 29.3, "CGAL kernels, filtered predicates, `Lazy_exact_nt`" (PORT METHOD, owned by chapter 6): "widen intervals with `next_up` and `next_down`" |
| Pinned link | https://github.com/CGAL/cgal/blob/cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa/Number_types/include/CGAL/FPU.h |
| Files read | At commit `cbc3b81d4607c6f3a9d1f3a47d8709a2be3e6bfa`: `Number_types/include/CGAL/FPU.h` lines 347 to 358 |
| Read by | Two sessions: the G01 research session, one Claude Opus 5.5 agent conversation, on 2026-10-21, which described these lines in plain words in evidence report 05, section on intervals (`projects/kit/notes/G01-evidence/05-exact-arithmetic-determinism.md`); and the session of build piece G02, which consulted the same lines for `geom-num`'s interval module and credits them in that module's header |
| Written by | The G16 session, one Claude Opus 5.5 agent conversation, on 2026-10-24, from report 05, the published sources below and the derivation in the header of `projects/engines/geometry/crates/geom-num/src/interval.rs`. It did not open the CGAL source |
| Second review | The approval of the pull request from piece G16 that adds this note: its reviewer checks the note against the lines above for copied expression, not for method (29.20, rule 6, step 4) |
| Code written from it | `projects/engines/geometry/crates/geom-num/src/interval.rs`, written by the G02 session before the method-only procedure had a home. That session consulted the CGAL lines itself, so step 5 of rule 6 (code written by a session that never had the source in its context) is not met for this module. Its widening is one line per bound, and its header derives it from the published faithful-rounding argument below, not from CGAL. Whether a session without the source should rewrite the widening anyway is an owner's decision (29.20, rule 11), raised on the pull request that adds this note |

## The problem

An interval `[lo, hi]` must contain the exact real result of every operation on its operands. Computed in floating point, a bound is rounded, and the rounded bound may fall inside the exact result. The usual remedy changes the processor's rounding direction, rounding lower bounds down and upper bounds up. Spare Kit cannot: Rust makes changing the rounding mode undefined behaviour, and WebAssembly has no rounding modes at all (report 05, section 7). The bounds must be made safe in round-to-nearest arithmetic alone.

## The method

Compute each bound with the ordinary correctly rounded operation, rounding to nearest. Then move the lower bound down by one representable double and the upper bound up by one, with the IEEE 754 operations next down and next up. Nothing else changes: no rounding mode, no relative factor, no further rounded operation.

Why one step is enough: rounding to nearest is faithful. The rounded result `r` of an exact value `x` is `x` itself or one of the two doubles on either side of `x`. Either way the double below `r` is at most `x`, and the double above `r` is at least `x`. This holds at binade boundaries, in the subnormal range and at zero alike, so the widened bounds enclose the exact result of any operation IEEE 754 rounds correctly: addition, subtraction, multiplication, division, square root and fused multiply-add.

Compared with the other round-to-nearest remedy, widening each bound by the relative factor one plus or minus the unit roundoff (Harris; Geogram's round-to-nearest interval class), one step outward is tighter, and it needs no second rounded operation, whose own rounding the relative widening has to account for.

## Invariants

1. **Inclusion.** For every operation built from correctly rounded IEEE 754 operations, the widened interval contains the exact result over its operands.
2. **Tight.** Each widened bound is the first or second double beyond the exact bound: the rounding lands on the exact bound or one double either side of it, and the widening moves one double outward.
3. **No rounding mode.** The method uses only round-to-nearest arithmetic, comparisons, and next up and next down, which are integer operations on the bits; so it gives the same bits on every target.
4. **Overflow is reported.** Next up of the largest finite double is infinity; a widened bound that leaves the finite doubles is an error, never a silent infinite interval.

## Failure cases

- **Exact results are widened too.** `3 - 3` is exactly zero, yet both bounds move off it, so an interval never proves an exact zero; zero is decided by the exact predicates (chapter 6.2). Tests must not expect a degenerate interval from a rounded operation.
- **Quantities never negative.** Widening can push the lower bound of a squared length just below zero, where a square root refuses. The squaring and length operations must use what they know (a square is never negative) to keep the bound at zero.
- **Signed zero.** Next down of either zero is the negative smallest subnormal; a zero bound should be stored as positive zero so that equal intervals have equal bits.
- **Functions without a proven bound.** One step outward covers a function only when it is correctly rounded, or faithfully rounded. A function whose error is not proven (the radian sine and cosine of a general maths library) cannot be enclosed this way and must be refused.
- **Overflow at the top of the range.** A rounded bound equal to the largest finite double widens to infinity (invariant 4).

## Published sources

- S. M. Rump, P. Zimmermann, S. Boldo and G. Melquiond, "Computing predecessor and successor in rounding to nearest", *BIT Numerical Mathematics* 49(2), 2009, 419 to 431, <https://doi.org/10.1007/s10543-009-0218-z>: computing the neighbouring doubles of a rounded result in round-to-nearest arithmetic, used to simulate interval operations.
- IEEE Std 754-2019, *IEEE Standard for Floating-Point Arithmetic*, sections 4.3 (rounding to nearest), 5.3.1 (nextUp and nextDown) and 5.4.1 (the correctly rounded operations).
- R. E. Moore, R. B. Kearfott and M. J. Cloud, *Introduction to Interval Analysis*, SIAM, 2009, <https://doi.org/10.1137/1.9780898717716>: the inclusion property.
- E. Harris on interval arithmetic in round-to-nearest, as `geom-num`'s interval module cites it: *Overload* 19(103), June 2011, <https://accu.org/journals/overload/19/103/harris_1974/>.
