# G231: exact rank by Bareiss elimination, a spike

Status: spike written, measured, recommendation below.

## The question

Chapter 15.8 decides a sketch's degrees of freedom, redundancy and conflicts from a pivoted QR of its
Jacobian with an SVD confirmation, at tolerances. Its "exact rigidity" paragraph and chapter 31's risk
table ask whether the rank could instead be decided exactly, in rationals, at a witness point, and
what that costs on blocks of about 1,000 unknowns. G232 will check the kernel's verdicts "against the
exact-rank Jacobian in rationals at seeded points". This spike answers whether that check is
affordable, and where an exact rank belongs.

## What was built

`geom_test::exact_rank` (a development crate; no kernel crate depends on it):

- `jacobian`: the Jacobian of a sketch job's rules at its starting points, which are doubles and so
  exact dyadic rationals. Each rule is written in its polynomial form (a length by its square, a
  point's distance from a line by the square of the cross product, parallel as a cross product,
  right angles as a dot product, midpoint and mirror as polynomials), each row scaled to whole
  numbers. An angle rule has no polynomial form over the rationals (sine and cosine are not
  rational) and is refused.
- `bareiss_rank`: Bareiss' fraction-free elimination (Mathematics of Computation 22, 1968) on
  `dashu-int` big integers (MIT OR Apache-2.0, the version the NURBS reference pins), skipping dead
  columns, with every division checked to be exact.
- `gauss_rank`: plain elimination over `dashu-ratio` rationals, the independent reference.

## What was measured

- On 200 random matrices of every shape up to 9 by 9 and every rank, including rank-deficient ones
  with dead columns, Bareiss and plain elimination give one rank, and it is never above the rank the
  matrix was built with.
- On the generated families (`geom_test::oracle::families`), for every case whose rules are
  polynomial (every family but the growth family, whose angle rules are refused) the exact rank is
  exactly what the family is built to have, and it is the kernel's diagnosis: the unknowns less the
  rank are the degrees of freedom, and the equations less the rank are the redundant and conflicting
  groups. No disagreement in 400 cases at the seed in `tests/exact_rank.rs`, and the same night is a
  replay entry of the golden store, `geom-test/sketch-families`.
- Cost on a chain of lengths and level or plumb lines (`print_the_cost_table`, release build on one
  core):

```text
points unknowns rank bits products milliseconds
10 20 20 97 4940 0
20 40 40 204 41080 2
40 80 80 401 334960 16
80 160 160 820 2705120 132
160 320 320 1670 21743040 1416
250 500 500 2595 83083500 4763
330 660 660 3434 191228620 11250
500 1000 1000 5202 665667000 44731
```

"bits" is the widest number met; "products" counts big-integer multiplications. Time grows with the
cube of the size (160 to 500 points is 3.1 times the unknowns and 32 times the time).

## What it shows

1. **Correctness is not in doubt.** The elimination is exact, and agrees with the pivoted QR on every
   family. The pivoted QR's tolerance (2^-12 of the largest column) never had to decide a case the
   exact rank decided otherwise.
2. **Cost is polynomial and affordable in a test.** A chain is the sparsest case, and a block of 1,000
   unknowns costs 45 seconds in a release build on one core, 5 seconds at 500 unknowns, 130
   milliseconds at 160. A dense block would cost more, so the figure is a floor for a banded one.
3. **Too slow to run on every solve.** The pivoted QR does the same block in milliseconds, so an exact
   rank belongs where a test can pay seconds, not in the solver's path.

## Recommendation

- **Use the exact rank in tests, not in the solver.** G232's law ("freedoms, redundancy and conflicts
  against the exact-rank Jacobian in rationals at seeded points") is affordable as stated: its sketches
  are the families' sizes (a dozen points), each well under a millisecond. Use `exact_rank::jacobian` and `bareiss_rank` as
  they are, extended with the rule forms G232 adds.
- **Do not put Bareiss in a kernel crate for the solver's verdicts.** The pivoted QR with the SVD
  confirmation is already right on every case, is orders of magnitude cheaper, and its tolerance is
  stated and reported. An exact answer would add cost and a second place for a verdict to come from.
- **If an exact verdict is ever wanted for a large block, do it per block and modularly.** Run the
  elimination on each Dulmage-Mendelsohn block (15.5), where sizes are small, and for a block over
  about 500 unknowns take the rank modulo a few random primes: the rank over Q is at least the rank
  modulo any prime that does not divide the right minors, so the largest rank found is a certified
  lower bound and equals the rank with overwhelming probability. That is a separate piece if wanted;
  nothing in G220a to G232 needs it.
- **Angles stay out.** An exact rank needs polynomial forms; angle rules enter through a rational
  parametrisation of the rotation (a rational point on the unit circle), which G232 can add where its
  tests need one.

## Open risks

- The witness point is generic only with high probability; a special configuration (three points on a
  line under a triangle's lengths) has a lower exact rank than the generic one. 15.8 reports those as
  `special`; the exact check at a witness shifted by a seeded amount, as the kernel does, sees the
  generic rank.
- Entries are doubles read as exact dyadics, so the "truth" is not met exactly at those points. The
  rank is a property of the polynomial forms' gradients, which are exact at the point whether or not
  the rules hold there; it equals the rank at a true solution only up to the genericity above.
- The cost table is for one core and one family of shapes; a sketch of other structure (a dense grid
  of mutual distances) costs more per unknown.

## Sources

E. H. Bareiss, "Sylvester's identity and multistep integer-preserving Gaussian elimination",
Mathematics of Computation 22 (1968), <https://doi.org/10.2307/2004533>. J. Edmonds, "Systems of
distinct representatives and linear algebra", Journal of Research of the NBS 71B (1967), for the size
of the numbers fraction-free elimination makes. `dashu` 0.6 (MIT OR Apache-2.0). Chapter 15.8 and
chapter 31's risk table in `projects/engines/geometry/architecture/`.
