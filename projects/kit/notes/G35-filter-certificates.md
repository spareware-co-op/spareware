# G35: Machine-checked proofs of the filters' error bounds, with Gappa and FPTaylor

Piece G35 of the kernel's work board (chapter 30), for chapter 28.6 of the [kernel architecture](../../engines/geometry/architecture/28-testing.md). Status: spike done for Gappa and Rocq; HOL Light's check of FPTaylor's certificates in progress (section 4). Piece G40 extends the problems to every generated filter and labels each one (section 7).

## The answer in plain words

Each kernel predicate first tries a fast floating-point shortcut, a filter, and answers only when the computed value is further from zero than an error bound; otherwise it falls back to exact integers. The filter is only safe if that bound really covers every rounding error. This piece proves it by machine for orient3d (and orient2d): Gappa proves the bound for real binary64 arithmetic at every scale, and the Rocq prover checks Gappa's proof, relying on nothing but Rocq's four standard axioms of classical real numbers. FPTaylor independently bounds the same error and writes a certificate for HOL Light. Both tools agree with the hand derivation in the generated code, and FPTaylor shows it is about a fifth more cautious than it needs to be for orient3d. The problems are generated from the same descriptions as the filters, and a test holds every line of them to the shipped code. The recommended pipeline is Gappa and Rocq as the proof of record, with FPTaylor and HOL Light as the independent check.

## 1. The question

Every generated predicate has an `f64` filter: it evaluates the description in floating point, computes a bound from the run-time maxima of the coordinate differences, and returns a sign only when the computed value is beyond that bound. Chapter 28.6 asks that this claim be proven, not only tested, by two independent tools run out of process: FPTaylor (MIT), whose results HOL Light checks as proof certificates, and Gappa (CeCILL or GPL), whose proofs the Rocq prover (formerly Coq) checks. A filter is called proven only once its certificate checks; until then it says "checked, not proven". The pipeline is chapter 28's proposal, and this piece is its spike: prove orient3d's filter with both tools, in containers, and choose the pipeline. Chapter 28.6 warns that FPTaylor last changed in 2024 and that its HOL Light set-up is old.

## 2. What a filter claims

For orient3d (`geom_num::generated::orient3d::filter`) the filter computes the nine coordinate differences exactly (whole numbers of magnitude at most 2^41 − 2), then 14 rounded operations to `det`, then the axis maxima m_x, m_y and m_z of the differences' magnitudes, the scale `scale = fl(fl(m_x · m_y) · m_z)` and the bound `bound = fl(C · scale)`, with C = 0x1.c000000000007p-49, the generated constant. It returns the sign of `det` when |det| > bound, `Zero` when the scale is zero, and nothing otherwise.

The claim is: whenever it returns a sign, that sign is the exact determinant D's. It follows from |det − D| ≤ bound, plus the zero case: a zero maximum makes a column of differences zero, so D = 0. The generated module's written proof derives |det − D| ≤ E · S, with S = m_x · m_y · m_z and E = 28u + 52u² + 48u³ + 22u⁴ + 4u⁵ (u = 2^−53), and bound ≥ E · S because C ≥ E · (1 + 2u)^k covers the k = 3 roundings between the maxima and the bound.

Both tools have to cope with the scale. The inputs are any whole numbers up to 2^41 in magnitude, and the error grows with the product of the maxima, so a bound of the form E · S is a statement about every scale at once. Rounding is not scale-invariant in general, but its error model fl(t) = t(1 + e), |e| ≤ u, is, whenever nothing underflows; here every value is a whole number or zero, so nothing does, and nothing comes near overflow (the certificate bounds every intermediate below 2^127).

## 3. The two pipelines

geom-gen writes both problems from the predicate's derivation (`crates/geom-gen/src/predicate_certificates.rs`, generators `gappa-orient2d`, `gappa-orient3d`, `fptaylor-orient2d` and `fptaylor-orient3d`, output in `crates/geom-gen/generated/`), and `tests/certificate_problems.rs` holds each problem to the committed filter, reading both as text: every operation in order, the inputs and their maxima, the constant's bits, the order of the scale's products, and the goal's numbers. `.github/workflows/kernel-certificates.yml` runs each tool in a container pinned by digest, with the scripts in `crates/geom-gen/certificates/`.

Both problems check the same numbers. With R = (k + 1)u, which bounds the computed bound's relative error ((1 + u)^k − 1 < (k + 1)u), and K = C · (1 − R), the goal is |det − D| ≤ K · S. Then bound ≥ C · S · (1 − R) = K · S ≥ |det − D|. Both K and R are dyadic and written exactly: for orient3d, K = 17747108403195219502253192773625 · 2^−152 ≈ (28u + 112u²) and R = 4u.

### 3.1 Gappa, checked by Rocq

Gappa proves enclosures of real expressions that contain rounding operators, and writes a Rocq proof of what it proved, which Rocq checks with Gappa's support library over Flocq's formalisation of IEEE 754 rounding. Melquiond and Pion used it in 2007 to certify CGAL's semi-static filters for orient2d, orient3d, incircle and insphere. Their key step is homogeneity: they proved the bound on the normalised problem with special rounding operators whose only theorems stay true at every scale. Current Gappa (1.8.3) has no such operators, but it has a predicate for the same induction, `LIN(a, b)`, "a = b · ε with ε in an interval", written `a // b`, with rules for sums and products of such quantities and for a rounding of one.

The problem states the filter exactly as the generated code computes it, in Gappa's `float<ieee_64, ne>`: each operation as `rnd(...)`, the same operations evaluated exactly beside it, the hypotheses that every difference and maximum is a whole number (`@FIX(x, 0)`, so no rounding underflows), that each maximum is between 1 and 2^41 − 2, and that each difference is at most its axis maximum in magnitude (`ba_x // m_x in [-1, 1]`). The goal is `det - e_det // S in [-K, K]` and `bound -/ C * S in [-R, R]`. Hints tie each order of a product of maxima to the canonical one (Gappa's rules match scales as written) and split each product's error into an operand's error times the other operand.

What the Rocq proof covers: the goal, for binary64 round-to-nearest with Flocq's semantics, for every input satisfying the hypotheses. Rocq's `Print Assumptions` on the final lemma lists only the four axioms of Rocq's classical reals (excluded middle, functional extensionality and the two of `ClassicalDedekindReals`); every hint's equality is proved, nothing is admitted. What it does not cover, and the written argument does: that no value overflows (Gappa's format has no upper exponent limit; the certificate bounds every intermediate below 2^127), that the translations are exact (bounded by 2^41 − 2 < 2^53), and the zero-scale case.

### 3.2 FPTaylor, checked by HOL Light

FPTaylor bounds the round-off error of an expression over a box of inputs by a symbolic Taylor expansion in the rounding errors, with global optimisation of the first-order term. Its `formal/` directory is a HOL Light procedure that checks the certificate FPTaylor writes when `proof-record = true`. Proof recording exists only up to FPTaylor v0.9.3 (2020); later versions dropped it (FPTaylor's `formal/README.md`).

The problem is the normalised one: every difference in [−1, 1], so S = 1, with the same operations in the same order as definitions. Because the error model without its subnormal term is homogeneous of degree one in each axis, and the filter's values never underflow, a bound B on the normalised problem bounds the error by B · S at every scale. HOL Light checks the certificate against K, the same number as Gappa's goal.

What the HOL Light certificate covers: the bound for every input in the box, under FPTaylor's error model. What it does not: the link from that model to IEEE rounding. FPTaylor's checker defines its rounding function by choice, as some function satisfying the model (`formal/build_tform2.hl`, "Temporary rounding definitions"; even the identity satisfies it), so the certificate is a theorem about the model, and the normalisation argument above is written, not checked. It is still an independent check of the constant: a different tool, a different method (Taylor forms and branch and bound, against interval rewriting) and a different prover.

Practicalities found: FPTaylor saves certificates as marshalled OCaml `Num` values, and HOL Light uses `Num` below OCaml 4.14 but Zarith from 4.14 on (its Makefile); and FPTaylor's 2020 checker calls `compare` where an opened `List` shadows it with `List.compare`, new in OCaml 4.12, so both run on OCaml 4.11. camlp5 8.04 needs PCRE2, Re and camlp-streams, which HOL Light's loader for OCaml before 4.14 does not load, so the session loads camlp5 through findlib first, as HOL Light's own loader for 4.14 does. HOL Light now ships the inequality verifier FPTaylor's checker needs (`Formal_ineqs`). Loading HOL Light's multivariate analysis takes about two hours before any certificate is checked.

## 4. Evidence

Runs of `kernel-certificates.yml` on pull request #262.

| Run | Pipeline | orient2d | orient3d |
| --- | --- | --- | --- |
| 40303875495 | Gappa 1.8.3, Rocq 9.1.1, coq-gappa 1.9.0, Flocq 4.2.2 (hand-written problems, goals left to Gappa) | error ±(2^54 + 1) · 2^−105 = ±(4u + 2u²); bound within ±(2u + 2^−106) of C · S; Rocq checked | error ±1008806316530991315 · 2^−108 = ±(28u + 52.75u²); bound within ±(3u + 97 · 2^−111); Rocq checked |
| 40306012279 | the same, with `Print Assumptions` | only the four classical-reals axioms | only the four classical-reals axioms |
| 40308045670 | Gappa and Rocq on the generated problems, with the goals stated: error within K · S, bound within R | proved; Rocq checked | proved; Rocq checked |
| 40304947857 | FPTaylor v0.9.3, bound only | 4u + 16u² (0x1.0000000000002p-51), certificate written | 22.1u (0x1.617d000000003p-49), certificate written |

What the numbers say:

- Gappa's error bound equals the derivation's E for orient2d, and exceeds it for orient3d only in the second-order term, by 0.75u², from rounding its intervals outward. The computed bound is at least (28u + 140u²) · S for orient3d, 87.25u² · S above Gappa's error, and (4u + 16u²) · S for orient2d, 14u² · S above (both checked in exact rationals).
- FPTaylor finds orient3d's worst error near 22.1u · S, not 28u: the derivation adds every operation's worst case at once, and no input reaches them all together. The constant is sound and about 27% larger than it needs to be. Tightening it would change the filter's answers on some inputs, so it would be a new generator version (chapter 26.2), not an edit; it is noted for G40.

(HOL Light's check of both certificates and the run on the generated problems: to record.)

## 5. Options and recommendation

| | Gappa + Rocq | FPTaylor + HOL Light |
| --- | --- | --- |
| What the proof is about | IEEE binary64 rounding (Flocq), the filter's own operations, every scale through `LIN` | FPTaylor's error model on the normalised problem; scale and the link to IEEE by written argument |
| Tightness | The derivation's bound to second order | Tighter than the derivation (global optimisation) |
| Maintenance | Gappa 1.8.3 (September 2026), coq-gappa 1.9.0, Rocq 9.1: current | FPTaylor v0.9.3 (2020) for certificates; HOL Light current; needs OCaml 4.11 |
| Cost in CI | A few minutes per predicate | About two hours to load HOL Light, then minutes per certificate |
| Licences | CeCILL or GPL (Gappa), LGPL (Rocq, coq-gappa): run out of process only | MIT (FPTaylor), BSD-style (HOL Light) |

Recommendation: Gappa and Rocq as the proof of record for every filter, run on every pull request that touches the generator or a generated module; FPTaylor and HOL Light as the independent check, run nightly and when a constant changes, because its proof is about the model, its tool is frozen at 2020, and it costs two hours a run. A filter is labelled proven when Rocq has checked its Gappa proof; FPTaylor's certificate is a second line of evidence and a measure of how tight the constant is. Piece G40 extends the problems to every filter and writes the labels.

## 6. Open risks and next steps

- **Filters the problems do not state yet.** None since piece G40 (section 7). A future filter whose scale has a term formed from more than two inputs, beside terms of other degrees, would need a new kind of case hypothesis; the generator refuses it with the reason, and its module cannot be generated until it is proven or has an open issue.
- **Gappa's licence class.** Gappa is CeCILL or GPL and Rocq and coq-gappa are LGPL. They run as separate programs in CI and never touch the kernel's code, as chapter 29.2's row for Gappa allows; owner action G75 asks an owner to confirm copyleft tools as CI-only oracles in general.
- **FPTaylor's frozen certificate path.** If v0.9.3 stops building, the HOL Light check goes with it; the Gappa proof does not depend on it.
- **The constant's room.** FPTaylor's 22.1u for orient3d suggests a tighter constant; that is a change of generator version, decided with G40 and measured against the filter's hit rate (chapter 23.2).

## 7. Every filter (piece G40)

Piece G40 extends the problems to every generated filter (orient2d, orient3d, incircle, insphere and plane_side), runs them every night as well as on every change, and labels each module.

- **One problem per case where the scale takes a larger-of.** incircle's scale is the larger of m_x³·m_y and m_x·m_y³, insphere's the largest of three such monomials, and plane_side's the largest of four terms of mixed degree, max{m_d, m_z·m_n, m_y·m_n, m_x·m_n}. A `LIN` goal needs one S, so each of these filters gets one problem per case (`predicate_certificates::gappa_cases`): one per group whose maximum is the largest when the monomials share a degree (incircle x and y; insphere x, y and z), and one per monomial that is the largest of the result's otherwise (plane_side d, z-n, y-n and x-n). Since some way of resolving the larger-ofs gives the largest monomial, the cases cover every input.
- **What a case may assume.** In its case, a problem's S is the largest monomial, and it states the filter's scale with each larger-of resolved to S's side. The filter's computed scale is at least that way's (a larger-of returns the larger computed side, and rounding and multiplying by a non-negative maximum are monotone), so the filter's bound is at least the one the problem proves sufficient. The hypotheses are what the case implies: in a group case every input is at most the largest maximum (`ad_y // m_x`); in a term case every input, or product of two inputs, that forms one of the other terms is at most S (`nx * p_x // S`), the steps above inheriting the bound through Gappa's rules. `tests/certificate_problems.rs` justifies every hypothesis of every problem from the module's own text, and checks there is one problem for each way the module's scale resolves.
- **Zero maxima.** A single problem or a group case assumes every maximum at least 1, as G35 did: there every monomial contains every group, so a zero maximum makes the exact value zero. plane_side's terms do not share their groups (a zero m_y leaves the offset), so a term case assumes at least 1 only for S's own maxima and lets the others be 0. A zero S makes every term zero and so the exact value zero (a rounded input's copy is zero only for a zero input).
- **Rounded inputs.** plane_side takes the `f64` copies of integers up to 125 bits wide. Its problems state the exact integer (`e_nx`, a whole number within its declared bits), the copy `nx = rndx(e_nx)`, and each maximum over the copies as the filter computes it, at most the input's bound rounded to the nearest `f64` (2^83 for a bound of 2^83 − 1). The copy's rounding is written `rndx = float<53, ne>`, binary64's precision without exponent bounds, because Gappa's relative-error rule for `float<ieee_64, ne>` needs the value bounded away from zero, and an input may be zero; Gappa's `rel_floatx` rule holds for every value. For a whole number below 2^1024 the two roundings agree: both keep zero exactly, and every other whole number is at least 1, inside binary64's normal range, where binary64 rounds to 53 bits. Like the translations' exactness, that is written here, not checked by Rocq. To find which of Gappa's rules a problem can lean on, Gappa 1.8.3's source (CeCILL or GPL) was read (`src/proofs/basic_proof.cpp`, `src/proofs/rewriting.cpp`, `src/arithmetic/float.cpp`); nothing from it is in the repository, which holds only problems in Gappa's input language.
- **The labels.** Each generated module's guarantees end with its label (`predicate_certificates::proof_sentence`), which MAP.json carries: proven, naming its Gappa problems and its FPTaylor problem or the open issue for one, when the problems state the filter in every case; otherwise "checked, not proven" with the open issue `predicate_certificates::UNPROVEN` gives it, and without one the module cannot be generated. A test holds each label to the problems the registry generates. `kernel-certificates.yml` runs every problem on each change to the generator or a generated module, and every night.
- **FPTaylor for a scale of several monomials ([issue #265](https://github.com/spareware-co-op/spareware/issues/265)).** The normalised problem divides each input by its group's maximum, and is homogeneous only when S is one monomial. In incircle's case x the bound needed is K · m_x³ · m_y, which on inputs divided by m_x is K · t for t = m_y / m_x in (0, 1], and FPTaylor bounds an absolute error over a box. So orient2d and orient3d have both tools, and incircle, insphere and plane_side Gappa with Rocq; the issue sets out how a second tool could cover them.

## Sources

- G. Melquiond and S. Pion, "Formally certified floating-point filters for homogeneous geometric predicates", RAIRO Theoretical Informatics and Applications 41 (2007) 57–69, [doi:10.1051/ita:2007005](https://doi.org/10.1051/ita:2007005).
- Gappa's manual, <https://gappa.gitlabpages.inria.fr/gappa/>; Gappa 1.8.3 (release tarball, SHA-256 `9b3e14e95a5b6aa0ee909b1d1e5c1fac04042024f29b66377cfb9e1923b25f5c`) and coq-gappa 1.9.0, run and used to check proofs; Gappa's source read only for which rules it has (section 7), nothing taken from it.
- A. Solovyev, M. S. Baranowski, I. Briggs, C. Jacobsen, Z. Rakamarić and G. Gopalakrishnan, "Rigorous Estimation of Floating-Point Round-off Errors with Symbolic Taylor Expansions", ACM TOPLAS 41 (2018), [doi:10.1145/3230733](https://doi.org/10.1145/3230733); FPTaylor (MIT) v0.9.3 at [`f85a7a7`](https://github.com/soarlab/FPTaylor/tree/f85a7a7797990fe5fb3436071afaa72c3f05629b), its `formal/` checker read for what it proves.
- HOL Light at [`d30e823`](https://github.com/jrh13/hol-light/tree/d30e8239887b2348a063f82232a864de1cc2df15), with its bundled Formal_ineqs; its Makefile and loaders read for the OCaml and camlp5 set-up.
- Containers: `rocq/rocq-prover:9.1` and `ocaml/opam:ubuntu-24.04-ocaml-4.11`, pinned by digest in the workflow.
