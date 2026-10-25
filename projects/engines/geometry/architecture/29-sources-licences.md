# 29. What we may take from each source

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: all thirteen reports, [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), [06](../../../kit/notes/G01-evidence/06-2d-drafting.md), [07](../../../kit/notes/G01-evidence/07-persistent-naming.md), [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), [10](../../../kit/notes/G01-evidence/10-github-sweep.md), [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md) and [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md); Spareware's legal rules in the [README](../../../README.md#built-legally-or-not-at-all) and [AGENTS.md](../../../AGENTS.md) section 5. Facts checked again for this chapter are marked "(checked 2026-10-21)", with the commit they were checked at.

Spare Kit is built from proven methods in our own code, and nothing enters as it is (principle 11). This chapter is the ledger behind that rule. For every project, paper, document and dataset the evidence examined, it records the exact licence, how far that licence lets us go, what the kernel plans to take, and on what conditions. Then it lists what no report has vetted yet, sets the rules every contributor follows when taking anything, and names the people worth talking to. A kernel meant to last fifty years must be able to show where every line came from.

What this chapter guarantees:

- Every line of kernel code is one of three things: our own work; a port of permissively licensed or MPL-2.0 code, rebuilt to section 3 with its notices kept; or our own implementation of a published method.
- No GPL, AGPL or LGPL code is ever copied into, linked into or shipped with Spare Kit, and no copyleft data is vendored. Copyleft programs may be used only as separate tools: oracles in CI, and ordinary development tools such as git or Valgrind.
- No knowledge taken from inside proprietary software enters the kernel. That covers decompiling, reverse engineering, and observing a program in order to copy its behaviour.
- Every outside source is recorded in the code, in the pull request and in `geometry/PROVENANCE.md`. Each record has a link pinned to a commit, the source's exact licence and how we used it. CI checks the records.
- Every shipped binary, WebAssembly bundles included, carries the notices its dependencies and ported files require.

What it never does:

- Give legal advice. Where the evidence leaves a licence, an ownership chain or a patent unclear, the row says so and the work waits for an owner.
- Decide architecture. A row's Planned column says how the kernel means to use a source and names the section that owns that choice. Sections still being drafted confirm or change it.

Terms used below:

- A **licence** is the permission a copyright holder gives.
- **Permissive** licences let us copy and change code as long as we keep the copyright notice and licence text, together called the **notice**. The exact list we accept is in 29.2.
- **Copyleft** licences (GPL, AGPL, LGPL) require anything built from the code to carry the same licence. That conflicts with Spare Kit's MPL-2.0 ([K07](K07-licence.md)).
- **MPL-2.0** is copyleft per file only. A file taken from an MPL-2.0 project stays MPL-2.0, which is Spare Kit's own licence.
- An **SPDX identifier** is a licence's standard short name, such as `BSD-3-Clause`. **NOASSERTION** is GitHub's label when it cannot match a repository's licence to a standard one. A hosting site's label is never the licence.
- **Chain of title** is the record of who owns a work's copyright. A licence is only as good as the licensor's ownership.
- An **oracle** is an outside program whose answers we compare with ours in tests.
- A **pinned link** points at one commit, so what it shows can never change.

## 29.1 The six ways we take something

Every row in this chapter gives two classes:

- **Allowed:** the furthest the licence and the chain of title let us go.
- **Planned:** what the kernel means to do, with the section that owns the choice in brackets. Planned never goes further than Allowed. Where the owning section is still being drafted, Planned is a proposal to it, not a decision.

| Class | What it means | When it is possible | Where it lands |
| --- | --- | --- | --- |
| DEPEND | A published crate in `Cargo.lock`, used as it is | Licence on the allow-list in 29.2, plus an audit (build piece "Dependency audit"). **Inside kernel crates, only `libm`**, which the spine itself names (6.4, 21), because principle 11 says nothing enters as it is. Outside the kernel (programs, renderers, tools, tests) a crate may be used as long as nothing it computes feeds a kernel result or stored geometry | The `Cargo.toml` of the crate that needs it, version pinned |
| PORT | Code translated into our Rust and rebuilt to section 3: our types, exact predicates where topology is decided, certified bounds elsewhere, deterministic maths, our errors, names and history, and our tests. The original notice is kept | Allow-listed or MPL-2.0 code with a clean chain of title, or one an owner has accepted (USD BRep, 32.8) | A file that keeps its origin licence and notice (29.20, rule 4) |
| PORT METHOD | We implement a published method in our own words and code. We copy nothing | Papers and books, and code 29.2 lets us read. LGPL sources go through the method-only procedure (rule 6) | Our own MPL-2.0 file. Its header cites the method and credits any source consulted |
| ORACLE | Used only to check our answers in tests. Never the truth: where an oracle disagrees with exact or interval ground truth, ground truth decides | Copyleft tools, C++ libraries and anything containing excluded code run as separate programs in CI and exchange only files with our code (rule 8). Allow-listed pure-Rust crates may be dev-dependencies when nothing excluded compiles in | `geom-test` and the oracle runner (section 28) |
| STUDY | Read for ideas, design lessons and failure cases. We take only ideas, and tests we write ourselves | Anything 29.2 lets us read | Design notes and our own tests |
| REJECT | Not used | The licence or provenance forbids it, or nothing in it is best in class | Recorded here so nobody re-evaluates it |

The rules in 29.20 are cited below as "rule N".

## 29.2 What each kind of licence allows

**The allow-list.** These SPDX identifiers, and no others, may appear in the kernel workspace's `Cargo.lock` or in a ported file:

- Permissive: `MIT`, `MIT-0`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `BSL-1.0`, `ISC`, `Zlib`, `0BSD`, `Unlicense`, `CC0-1.0`, `Unicode-3.0`, `Unicode-DFS-2016`.
- Copyleft per file: `MPL-2.0`.

The two Unicode licences are there because ordinary build tooling needs them: `unicode-ident`, pulled in by `proc-macro2` and `syn`, uses them. Adding an identifier needs a row in this section and a kernel owner's yes.

MPL-2.0 is copyleft per file, not permissive. Principle 11 says "We take permissive code only", and the spine's section 2 says "permissive dependencies only". This chapter accepts MPL-2.0 because it is Spare Kit's own licence, so a taken file stays under the licence the kernel already has. That widens principle 11's wording, so an owner confirms it (open question). The merge gate's licence check (28.8) uses this allow-list: permissive licences plus MPL-2.0.

Two licences in the ledger have no identifier of their own. SPDX files the EDL-1.0 text as `BSD-3-Clause`, so a file ported from JTS carries that identifier. Public domain has none, so public-domain code (Shewchuk's `predicates.c`) is taken as PORT METHOD, unless an owner adds `LicenseRef-PublicDomain`, with the dedication text, to this list.

| Licence or status | Read the source? | Take code? | Run as an oracle? |
| --- | --- | --- | --- |
| On the permissive allow-list | Yes | Yes: PORT with the notice, or DEPEND where 29.1 allows | Yes |
| MPL-2.0 | Yes | Yes. The file stays MPL-2.0 with its notices | Yes |
| LGPL (any version, with or without exceptions) | Yes, only under the method-only procedure (rule 6), as principle 11 allows | Never | Out of process only |
| GPL (any version, with or without linking exceptions) | **No, by default.** Use the authors' papers and public documentation. Only where no paper or permissive source covers a method may a kernel owner allow reading, recorded in the row, and then the method-only procedure applies. Principle 11 names only LGPL for method study, so this exception needs an owner's confirmation | Never | Out of process only |
| AGPL; non-commercial terms (PolyForm, IRIT, HLBFGS, Adobe's Research Licence); custom terms such as copyright assignment; "all rights reserved"; no licence file | No | Never | Only with an owner's yes |
| Modified, unclear or self-contradicting licences (OpenSubdiv's TOST, openNURBS, NOASSERTION, a LICENSE and a README that disagree) | Yes | Only after the condition in the source's row is met | Out of process |
| Numerical Recipes code and anything derived from it | No | Never: not from any source, and not from memory | Only inside a third-party build that contains it, run in CI after an owner's yes and never distributed (rule 8) |
| Proprietary documentation (Autodesk DXF Reference, Revit API, Parasolid, ACIS, Onshape) | Vendor-hosted public pages only, to learn a format or a concept. Third-party copies have unknown authorisation: stop and ask | No text, tables or figures. Brief quotes with links only | — |
| CC BY-ND documentation (IFC 4.3) | Yes | Link and quote briefly with credit. Never adapt the text; our docs are written fresh. IFC's own notice also allows its documentation to be used in software development with full attribution (29.14) | — |
| ISO standards (ISO 10303, ISO 16739) | Yes, from licensed copies | Implement the standard. Never copy its text. Whether code may be generated from ISO's EXPRESS schema files is unverified | — |
| Papers and books | Yes | Methods are free to use. Never transcribe text, figures or pseudocode (The NURBS Book is Springer copyright). Papers under CC BY-NC-ND are cited, never adapted | — |
| Reverse-engineered format knowledge (DWG, SKP, ACIS internals, Revit files) | No, unless an owner clears it after legal advice (README) | Never | No |

## 29.3 Numbers, predicates and exact arithmetic (`geom-num`)

From [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) section 4, [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md) section 5, [10](../../../kit/notes/G01-evidence/10-github-sweep.md) and [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| Shewchuk, `predicates.c` | Public domain, which has no SPDX identifier on the allow-list (29.2) | PORT METHOD. A PORT needs `LicenseRef-PublicDomain` and an owner's yes (29.2) | PORT METHOD (6.2), from the papers: Dekker 1971, Knuth (*The Art of Computer Programming*, volume 2) and Shewchuk 1997 | Expansion arithmetic (TwoSum, TwoProduct, Dekker split), used only on raw `f64`: for decisions at import, before data is snapped to the grid, and inside double-double arithmetic. `predicates.c` has no underflow guard; `geom-num`'s versions (G02) need no range check instead, because outside Shewchuk's range `split` and `two_prod` first move their inputs into range by exact powers of two, and a pair that cannot be exact is reported by an infinite second part with the sign of the exact error. `predicates.c` gets a credit line as a source consulted |
| Geogram (Inria): PCK, `expansion_nt`, `interval_nt` | BSD-3-Clause for Geogram's own code. Its `third_party/` folders have their own terms (row below) | PORT | PORT (6.2, 6.4) | The predicate language and filter generator (Meyer and Pion's FPG, a "floating-point filter generator"); the semi-static filters with their overflow and underflow guards; the template for Simulation of Simplicity (SoS); round-to-nearest intervals widened outward (6.4). Inria's notice kept. Not taken: `SOS_ADDRESS`, the default order, which sorts by memory address. Ours sorts by persistent name |
| Geogram exact mesh intersection, mesh CSG and `CDT_2d` | BSD-3-Clause | PORT | `CDT_2d`: PORT METHOD (17.3) for Sloan's flip insertion of constraints and the exact construction of a crossing from the two original constraint ends (`create_intersection`). Mesh intersection and CSG: STUDY | The kernel's constrained Delaunay triangulation is `geom-mesh::cdt`, ported from spade (29.12). `CDT_2d` itself is not taken: in the open build its circle test on crossing points is approximate, and its result depends on insertion order (17.3). Mesh booleans and repair use the plane-based arrangement of 11.3 (19.5, 19.7), so no section takes Geogram's mesh CSG. Any file ported later is checked to use nothing from `third_party/` |
| Geogram `third_party/tetgen`, `third_party/triangle`, `third_party/HLBFGS` | TetGen: AGPL-3.0. Triangle: may be redistributed only if "no compensation is received"; commercial use only by arrangement. HLBFGS: non-commercial only. (checked 2026-10-21 at [`40d72165`](https://github.com/BrunoLevy/geogram/tree/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/lib/geogram/third_party)) | REJECT | REJECT | Never read. The other `third_party/` folders (PoissonRecon, xatlas, OpenNL, libMeshb, amgcl, lua, rply, stb, stb_image, zlib) carry their own licences, not checked here. Check each before any use |
| GeogramPlus `exact_nt` (Tessael) | Proprietary | REJECT | REJECT | Not part of the open build |
| CGAL kernels, filtered predicates, `Lazy_exact_nt` | LGPL-3.0-or-later | PORT METHOD | PORT METHOD (6) | Method lessons: filter first and go exact only when the sign is uncertain; widen intervals with `next_up` and `next_down`; never feed rounded constructions back into decisions (6.3). Its rounding-mode switching cannot run in Rust or WebAssembly |
| CGAL packages | GPL-3.0-or-later, or commercial | Papers only; ORACLE out of process | Rows in their own areas; ORACLE out of process (28.10): Nef polyhedra on an exact kernel, and 2D arrangements with circle-segment traits | Source not read |
| Attene, Indirect_Predicates and `nfg` | LGPL. The LICENSE file is LGPL-2.1 while the README says LGPL-3.0-or-later; `nfg` is LGPL-2.1 | PORT METHOD | PORT METHOD (6.3) | Implicit points (segment–segment, line–plane, three planes), re-derived as homogeneous integers. Its design needs `-frounding-math` and changes the FPU mode, so it would not transfer anyway |
| EMBER (Trettner, Nehring-Wirxel and Kobbelt 2022); Nehring-Wirxel et al. 2021; Campen and Kobbelt 2010 | Papers. No public code found (unverified) | PORT METHOD | PORT METHOD (6, 11) | Plane-based integer geometry, with bit budgets re-derived for 40-bit coordinates ([05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) section 4). The authors are worth contacting (29.21) |
| GeometricTools (Eberly) | BSL-1.0 | PORT | PORT (6.2, 12) | Compile-time bit counting (`BSPrecision`) as a cross-check of our hand-derived budgets, and the derivations behind its intersection, distance and containment queries, each re-certified with our predicates. Notice kept |
| `robust` crate | MIT OR Apache-2.0 | PORT, DEPEND | ORACLE (dev-dependency) | An independent check of `orient2d`, `orient3d`, `incircle` and `insphere` (28.6). Its `orient3d` and `insphere` have Shewchuk's signs, the opposite of `geom-num`'s (6.2), so the check negates them. `robust` 1.2.0 is a development dependency of `geom-num`, whose `tests/attack_g28.rs` runs the check on every input it makes (G28). Not a kernel predicate: it returns an `f64` rather than a sign and has dropped the underflow guard |
| `geometry-predicates` | MIT | PORT, DEPEND | REJECT | `robust` covers it; unmaintained since 2021 |
| `bnum` | MIT OR Apache-2.0 (manifest) | PORT, DEPEND | ORACLE (dev-dependency) | We write I256, I320 and I384 ourselves in `geom-num`, as one const-generic `Wide<L>` (G02, which also defines `I512`, waiting for acceptance, 6.2): small, `no_std`, each with a written bit-budget derivation and tests at extreme values. `bnum` checks them |
| `crypto-bigint`; `ruint` | Apache-2.0 OR MIT; MIT | PORT, DEPEND | REJECT | Built for constant-time cryptography, probably slower (unverified); `ruint` is unsigned only |
| `num-bigint`, `num-rational`, with `num-traits` and `num-integer` | MIT OR Apache-2.0 | PORT, DEPEND | ORACLE (dev-dependency) | Rational arithmetic as the reference in differential testing of every predicate, of the error-free transformations, intervals and maths functions (G02, G05), and of exact mass properties |
| `dashu` (`dashu-int`, `dashu-ratio`) | MIT OR Apache-2.0 | PORT, DEPEND | ORACLE (dev-dependency) | The exact oracle of the double-double and `ExactSum` tests (G25), without its default features; 23.3, 28.6 and the board name it beside `num`. Planned as STUDY before G25 used it |
| `rug`, `gmp-mpfr-sys`; `malachite` | LGPL-3.0-or-later; LGPL-3.0-only | REJECT | REJECT | The first two link GMP and MPFR; `malachite` is derived from them |
| `twofloat` | BSD-3-Clause | PORT | ORACLE (dev-dependency) | Our double-double frame conversion (6.1) is built on `geom-num`'s TwoSum and TwoProduct and its double-double type, `DD` (G25). Not used so far: G25 checks `DD` against exact rationals (`dashu`, with `num-rational` in its attack tests), a stronger oracle |
| `hyperreal`, `hyperlimit` (csgrs workspace) | Apache-2.0; MIT OR Apache-2.0 | PORT | STUDY | Their "certainty receipt": an undecided predicate becomes a typed error, never `false`. Computable reals are not our number model |
| `apfp` ([rgeometry/apfp](https://github.com/rgeometry/apfp/tree/175ab9ac94d7713d36de38aee5659470cc23b390), release 0.1.1) | Unlicense | PORT | REJECT, kept as a cautionary example | [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)'s challenges found its staged filters unsound: the double-double stage certifies rounding noise as a sign; product magnitude bounds ignore the factors; `int_signum!` truncates at 256 bits. These come from a line-by-line simulation of the pinned source, not from running the crate. Lessons for our generator: a proven bound at every stage, magnitude bounds carried through nested products as the product of the factors' bounds, and one good idea, a CI check that generated predicate code has no allocation and no panic paths. Its counterexamples become `geom-num` regression tests. (The crates.io repository link is stale; the source is at the pinned link) |
| Papers: Shewchuk 1997; Meyer and Pion (FPG); Edelsbrunner and Mücke 1990; Devillers and Teillaud 2011; Ozaki et al. | Papers | PORT METHOD | PORT METHOD (6) | Cited by DOI in module headers |
| Papers on double-word arithmetic: Joldes, Muller and Popescu 2017 (ACM TOMS 44(2)); Muller and Rideau 2022 (TOMS 48(1), the Coq proofs); Lefèvre, Louvet, Muller, Picot and Rideau 2023 (TOMS 49(1)); Hida, Li and Bailey 2001 | Papers | PORT METHOD | PORT METHOD (G25, `geom-num`'s `dd`) | The machine-checked algorithms and bounds of `DD`, read from the authors' open copies; cited by DOI in the module header |
| QD library, release 2.3.24 (Hida, Li and Bailey) | Lawrence Berkeley National Laboratory's modified BSD licence, whose COPYING adds "If you wish to use the software for commercial purposes please contact the Technology Transfer Department" | STUDY; code only after an owner clears that clause (29.2, modified licences) | STUDY (G25): read to compare methods; nothing taken | Its default double-double addition has an unbounded relative error for operands of opposite signs, and its products and root have no published bound, so `DD` uses the proven algorithms above |
| Neal, exact summation: arXiv 1505.05571 (2015), and his C code `xsum` ([`b766b3e1`](https://github.com/radfordneal/xsum/tree/b766b3e104cdbd6769c8df033fb430700fce32dd)) | Paper; MIT, copyright 2015 to 2024 Radford M. Neal | PORT | PORT METHOD (23.4; G25, `geom-num`'s `exact_sum`), with the MIT credit in the module header | Both superaccumulators, rebuilt in checked Rust, not translated; the negative-rounding branch that xsum fixed in 2024 was rewritten from the method |

## 29.4 Deterministic maths and numerical routines (`geom-num`)

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| `libm` (rust-lang) | MIT. Files taken from FreeBSD msun also carry Sun's notice, which must be "preserved" (checked 2026-10-21 at [compiler-builtins `aeab3283`](https://github.com/rust-lang/compiler-builtins/blob/aeab328368d6ed8dc6acba47b7db6ad08be953d2/libm/src/math/sin.rs)) | PORT, DEPEND | DEPEND, the only one inside kernel crates (6.4, 21) | The source of `sqrt`, `fma`, `floor`, `ceil` and round-to-even, and of the maths functions with no proven bound, kept apart in `geom_num::math::unproven`: radian `sin` and `cos`, `tan`, `pow` and the rest (6.4). Since G05 the proven `sinpi`, `cospi`, `exp`, `ln` and `atan2` are CORE-MATH ports, and `libm`'s versions of them are unproven twins. Pure Rust, but on aarch64 with NEON it uses the processor's square root and fused multiply-add, on x86-64 with SSE2 the processor's square root and, where a check at run time finds one, its fused multiply-add, and on wasm32 its own routines; the bits agree because IEEE 754 rounds those operations correctly (6.4). Version pinned (0.2.16) by `Cargo.lock`, not yet vendored (28.13); the standard library's versions banned by lint. Sun's notice goes into the generated notices (rule 4) |
| CORE-MATH (Inria) | MIT | PORT | PORT (21) | Correctly rounded `sin`, `cos`, `sinpi`, `cospi`, `atan2`, `exp`, `log` and `pow`, ported to pure Rust with the notice kept. The long-term contract of section 21. Ported so far (G05, from commit `0ce0cbc15935e24c656280e5a46c34e89f301b7a`): `sinpi`, `cospi`, `exp`, `log` (as `ln`) and `atan2`, in safe `no_std` Rust with written proofs, the MIT notice reproduced at the start of each ported file under an SPDX line `MPL-2.0 AND MIT`, `geom-num`'s `license` field is `MPL-2.0 AND MIT`, and `projects/engines/geometry/NOTICE` lists the source, commit and files, as rule 4 asks. Radian `sin` and `cos` cannot be ported to the same proven standard from what CORE-MATH publishes: `sin.c` defers its proofs to a `sin.pdf` not in the repository, `sin.wc` marks some binades as incomplete, and `cos.wc` states its BaCSeL search only for arguments below 2^11 in magnitude, listing other hard cases above that without saying the list is complete. `pow` was not attempted. The port found three gaps in CORE-MATH itself, recorded in its modules: a `sinpi` and `cospi` table error of 2^-77.956 against a 2^-78 tolerance, `ln`'s accurate polynomial used outside its fitted range just below 1, and `atan2`'s tiny-quotient subnormal step left unproven |
| `core-math` crate | MIT | PORT, DEPEND | STUDY | It binds the C sources, and its WebAssembly build is unverified. The kernel contains no C |
| RLIBM | MIT | PORT | REJECT | 32-bit types only |
| Numerical Recipes in C, and everything derived from it (29.6) | Proprietary; its licence forbids redistribution | REJECT | REJECT | Never ported, never read for porting, never reproduced from memory. No one else's relicensing can cure this ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), overview-legal) |
| LAPACK; EISPACK; Golub and Van Loan; Brent 1973; the Thomas algorithm; Gauss–Legendre and Gauss–Kronrod rules; Cash and Karp 1990; Hairer, Nørsett and Wanner; Higham, *Accuracy and Stability of Numerical Algorithms* | LAPACK: modified BSD (not re-checked here). EISPACK: on netlib without a licence file (unverified). The rest are books and papers | PORT METHOD | PORT METHOD into `geom-num` (build pieces "Clean-sourced numerical routines") | The clean sources for LU, QR, SVD, symmetric eigenproblems, Brent root and minimum finding, tridiagonal solves, quadrature and adaptive integration, and the roots of a quadratic by the standard formula that avoids cancellation. They replace every routine excluded in 29.6. EISPACK is used as a method description until its status is known |
| COLAMD (Davis, Gilbert, Larimore and Ng 2004) | BSD-3-Clause (15.6; not re-checked here) | PORT | PORT into `geom-num` (15.6): the column ordering that keeps sparse factors small, in safe Rust, with ties broken by variable order. Notice kept | fiksi's C2Rust translation, `colamd_rs`, is not used (29.9) |
| Rapier; Box2D v3 | Apache-2.0; MIT (checked 2026-10-21) | PORT | STUDY | Their determinism method: replay hashes on every pull request, thread-count tests, and their own maths functions (21) |
| `rayon` | MIT OR Apache-2.0 | PORT, DEPEND | DEPEND only in a `std` executor crate outside the `no_std` kernel crates, handed to the kernel through a trait with fixed chunks and results collected in order. WebAssembly without threads takes the serial path. Section 23 owns this | It contains `unsafe` code and needs `std` and threads, so it cannot sit in a kernel crate (principle 13, section 21). Its floating-point `sum` and `reduce` are never used |
| wasm-bindgen-rayon | Apache-2.0 (checked 2026-10-21 at [`4eea1fa5`](https://github.com/RReverser/wasm-bindgen-rayon/tree/4eea1fa55a965ad516ef9f9e9449704c7eac91c5)) | PORT, DEPEND | DEPEND only in `geom-pool`, the `std` adapter crate outside the kernel crates, for Web Worker threads in browsers that allow shared memory (23.4) | Its threaded build needs a pinned nightly compiler today, so until that reaches stable Rust the browser runs the kernel serially (23.4) |

## 29.5 Whole kernels and kernel projects

From [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md), [10](../../../kit/notes/G01-evidence/10-github-sweep.md), [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md) and [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| OpenCascade (OCCT) | LGPL-2.1 with the OCCT exception | PORT METHOD, ORACLE | PORT METHOD and ORACLE (option A, section 4) | Methods: splitting intersection from building results (General Fuse), pave and common blocks, cells, the `BRepTools_History` contract (20.3), typed alerts, edge-first meshing. Oracle: its DRAW harness, `checkshape` and `bopargcheck`, run as separate programs in CI; never linked or shipped. Its own list of failures is a list of tests for us |
| `opencascade-rs` | LGPL-2.1 | REJECT | REJECT | A binding to OCCT |
| Parasolid public documentation (Functional Description; XT format reference) | Siemens documentation. The evidence read it on q-solid.com, a third-party copy whose authorisation is unknown ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) header) | STUDY, from vendor-hosted pages only | STUDY, once re-checked against Siemens's own public pages. The mirror is "provenance unverified: stop and ask" | Concepts only: regions and fins; exact by default with local tolerances; `max_tol`; partitions and rollback; attribute classes; grouped body checks. The same concepts are published in the literature, which the kernel's docs cite in preference to vendor pages: Weiler's radial-edge structure, Mäntylä's half-edge structures, and Jackson 1995 on tolerant modelling (exact references pinned where section 8 cites them). No text copied |
| ACIS documentation (R10) | Spatial documentation. The evidence read it on a University of Arizona site whose authorisation is unknown | STUDY, from vendor-hosted pages only | As Parasolid | Tolerant edges and vertices; history and roll-back |
| Autodesk ShapeManager | Proprietary, no public documentation | REJECT | REJECT | — |
| NVIDIA USD BRep: SMLib core (`Sm*` files), [`c9c5979`](https://github.com/NVIDIA-Omniverse/usd-brep/tree/c9c5979f04cea8c38213004d4f04df0dcfb40d05) | Apache-2.0 for "NVIDIA-authored portions" (its THIRD_PARTY_NOTICES). Every file re-stamped "Copyright (c) 2026 NVIDIA", which hides older authorship. The owner accepts NVIDIA's publication as sufficient for the whole release (32.8, decided 2026-10-24) | PORT, minus 29.6 | STUDY and ORACLE (option D, section 4); PORT where a chapter takes its method (12, 13) | Port: translated into our Rust and rebuilt to section 3, NVIDIA's notice kept (rule 4); its silent fallbacks, tolerance globals and uncertified heuristics are not ported as behaviour (12.17). Study: its operation catalogue, healing and error taxonomies, the kinds of intersection contact, and its topology events (split, merge, coincident). Its KNOWN_ISSUES file and issues become failure cases rebuilt with our own inputs. Oracle: out of process, deterministic layers only (evaluation, knot operations, degree change, fitting without random sampling), and only after an owner's yes, because the build compiles Numerical Recipes-derived routines (rule 8). For those layers exact rational evaluation is the stronger truth; SMLib mainly catches gross errors. It cannot be an automated oracle for booleans, fillets, shells or offsets, where it is nondeterministic (issue #10) ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), overview-legal challenge) |
| NLib (the `NL_*` files inside USD BRep) | Apache-2.0, as above. NLib was written by Piegl and Tiller at GeomWare. NVIDIA's own manual says SMS acquired "exclusive rights for maintenance, distribution, and development", so who holds the copyright is unverified. The owner accepts NVIDIA's publication as sufficient (32.8, decided 2026-10-24) | PORT, minus 29.6 | PORT (option D, section 4; 7, 12, 13) | Primary sources first: The NURBS Book and the Piegl–Tiller papers. NLib may be followed closely and ported: translated into our Rust and rebuilt to section 3, its silent fallbacks turned into typed errors (29.7). The spine's "notices kept" (option D, section 4) means: a ported file starts with NVIDIA's Apache-2.0 notice, its copyright lines copied exactly, and says what we changed; a module that only consulted NLib carries a credit line; either way `geometry/NOTICE` has an entry (rule 4). Numerical Recipes routines are excluded (29.6) |
| solidcore (tangency-labs), [`6a019c5`](https://github.com/tangency-labs/solidcore/tree/6a019c57131d5fee6e57ab7dc4a62201a32e0216) | Apache-2.0. Its NOTICE carries NVIDIA's notice plus "the solidcore contributors" | ORACLE out of process, STUDY. It contains Numerical Recipes-derived files (29.6), so it is never a Cargo dependency of any kind | ORACLE and STUDY | Oracle: recorded outputs of its green NURBS and curve units, made in an isolated CI container after an owner's yes (rule 8), stored as data with tool name and version, on well-conditioned inputs only. "Green" means a unit whose generated and fixture cases matched the C++ without hitting a stub; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md) shows green units can still hide gaps. The stronger path: its `harness/cpp` shims let us compare against SMLib itself. We build that reference from pinned source in our own container rather than run its prebuilt `cpp-ref` release. Study: its strict comparator, perturbation checks, child-process fuzzing, its measurements of how `libm` results differ between targets, and its 337-entry catalogue of SMLib defects (`plan/upstream-issues.md`). Its `deny.toml` bans MPL-2.0, so none of our code can flow into it |
| Truck | Apache-2.0 | PORT | STUDY | The curve and surface trait vocabulary (option C), its derivative jets (`ders.rs`) and rolling-ball surface definitions. Its issues #57, #114, #134 and #135 become our tests. Its mesh-driven booleans and pointer identity are what we avoid |
| monstertruck | Apache-2.0 | PORT | STUDY | A Truck fork with the same boolean method |
| Fornjot | 0BSD | PORT | STUDY | Its final design (typed index arenas), two-way coincidence validation and published lessons |
| SolidCraft (tangency-labs) | MIT OR Apache-2.0 | PORT | STUDY | Its catalogue of Truck failures, fuzzing design, property invariants used as hard gates, and the lineage it records during booleans. Its retry-and-nudge ladder is an anti-pattern |
| csgrs 0.20.1 (the released crate) | MIT. Its default `image-io` feature pulls in `contour_tracing`, which is EUPL-1.2 | PORT (without `image-io`) | REJECT | Float BSP booleans with epsilons |
| csgrs main and the Hyper crates other than `hypermesh` (`hyperbrep`, `hypercurve`, `hyperlimit`, `hyperreal`), [`1bd74092`](https://github.com/timschmidt/csgrs/tree/1bd74092cad1d8a52b2a4dc4fa1fae2bcc605586) | csgrs MIT. `hyperreal`, `hypercurve`, `hyperbrep`, `hyperbrep-io` and `hypersolve` Apache-2.0. `hyperlimit` and `hypertri` MIT OR Apache-2.0. Dev-only dependencies `rug` (LGPL) and `symbolica` (proprietary) | STUDY | STUDY | The closest design peer. Worth learning: certainty receipts, a builder that publishes only valid models, and `hypercurve`'s exact arc regions. The code is unreleased, changes daily, is mostly written by AI agents and has one maintainer |
| `hypermesh` (csgrs workspace, same commit) | MIT under a "Copyright (c) 2026 zalo" notice, although it descends from MPL-2.0 boolmesh: it was relicensed in a rewrite commit ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md)) | STUDY, because both licences it could be under, MIT and MPL-2.0, allow reading. Stop and ask before any PORT (rule 2) | STUDY (11, 14.5, 19.5) | Worth learning: its output closure certificate (`surface_arrangement.rs`) and its multi-operand truth DAG (`winding.rs`), which sections 11, 14 and 19 take as ideas. Confirm its provenance with the author before any code is taken (29.21) |
| cadcore | MIT. Versions 0.1.0–0.1.21 were published as non-commercial (PolyForm-Noncommercial-1.0.0, or "non-standard" for 0.1.0) and granted MIT later; crates.io still shows the old terms | Stop and ask | REJECT | Nothing best in class |
| RGeometry | Unlicense. Its optional `rug` feature is LGPL; its `f64` predicates rest on `apfp` (29.3) | PORT (without `rug`) | REJECT | A teaching library. One idea kept: property tests over tiny integer coordinates |
| vcad | Apache-2.0 | PORT | STUDY | Low priority; its depth is unverified |
| chili3d; brepkit; Aetheris | AGPL-3.0 | REJECT | REJECT | Not read |
| next.BREP.io Rust kernel; JSketcher | Custom terms that assign copyright in modifications to Autodrop3d LLC | REJECT | REJECT | Not open source in our sense |
| BRL-CAD | LGPL for the libraries (version unverified); BSD or public domain for build files and data | PORT METHOD | STUDY | Graded fallbacks: CSG evaluated by rays, then Poisson reconstruction |
| build123d; CadQuery; replicad | Apache-2.0; Apache-2.0; MIT | PORT | STUDY | Code-CAD API design and typed selectors. Their selectors breaking when topology changes is the problem section 20 solves |
| Zoo modeling-app (KCL) | MIT; its geometry engine is closed | PORT | STUDY | Language design |
| Dune 3D | GPL-3.0 | Papers and documentation only | STUDY (public documentation and running the program) | Interface ideas only; source not read |
| CADmium; datum | Elastic License 2.0 (archived); MIT | REJECT; PORT | REJECT | — |
| IntegrityWare SOLIDS++ | Proprietary | REJECT | REJECT | — |
| CADCraft (storytold), [`7fef8cbe`](https://github.com/storytold/cadcraft/tree/7fef8cbe2fb8dbafdcf6ebda8aa3ba0623bcbec1) | MIT OR Apache-2.0 (`Cargo.toml`; GitHub shows Apache-2.0). The ArtCraft brand assets are excluded | STUDY only. Its chain of title is in doubt: it was developed by observing AutoCAD, and AI-written code already reproduced Autodesk content | STUDY | Worth learning: CI-enforced crate layering, one command registry shared by GUI, CLI and MCP, its never-crash discipline (and its failure mode, silently substituting values), its hostile-input tests and its honest parity audits. REJECT: `acadrust` (written with LibreDWG's GPL source open); `xtask/data/menu-catalog.txt` (AutoCAD's menu tree); the linetype and hatch data in `crates/doc/src/library.rs`, which matches Autodesk content although it claims to be original; AutoCAD prompt wording; and the dynamic-block code and any other "as AutoCAD writes it" knowledge. The relationship question is in 29.21 |
| getcadcraft repositories (The Data Design Group, Inc.): [`cadcraft` `93b7f5eb`](https://github.com/getcadcraft/cadcraft/tree/93b7f5eb58b94e726a5d5333dd792cca8f1d7a14), [`cadcraft-rust` `3daa3406`](https://github.com/getcadcraft/cadcraft-rust/tree/3daa3406b70d010e2210f109b186f3f2d9480aaf), [`partcraft` `bdf4475d`](https://github.com/getcadcraft/partcraft/tree/bdf4475db1e0a719e0de095da9725ba24248d2db) | Apache-2.0 throughout. "CadCraft" is claimed as a trademark, and forks must rename. `cc-manifold` also ports Clipper2 (BSL-1.0), libc++ sort (Apache-2.0 WITH LLVM-exception), PhysX `tri_dist` (BSD-3-Clause), tbtSVD (MIT), disjoint-sets (Zlib) and msun (Sun's notice) ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md), formats-legal) | PORT for `cc-manifold`'s own files, notices kept | STUDY | Worth learning: `cc-manifold`'s determinism hardening (sequential paths, no fused multiply-add, ported msun) and bit-parity golden tests; its `PROVENANCE.md` as an example of written provenance. Where we need Manifold or Clipper2 methods we go to the upstream originals. The K.1 spike (partcraft's benchmark of ten closed-form parts across OCCT, Manifold, Truck and monstertruck) is re-created by us. REJECT: `cc-io-skp` (SketchUp knowledge from OpenSKP, itself reverse-engineered); the DWG and ACIS framing, learnt from the ODA specification, the output of the proprietary ODA File Converter and LibreDWG and libdxfrw run as black boxes; the Revit `.rvt` and `.rfa` fixtures; `cc-m3d` (a JavaScript transliteration with an absolute epsilon of 1e-6 m); and partcraft's `brep*.js`, thin glue over OCCT's API with nothing of its own to take |

## 29.6 Exclusions inside USD BRep and solidcore

USD BRep's Apache-2.0 grant covers "NVIDIA-authored portions", and NVIDIA re-stamped every file, which hides older authorship. The owner accepts NVIDIA's publication as sufficient for the whole release (32.8, decided 2026-10-24). So NLib, its constants and step order included, and the SMLib modules showing IntegrityWare lineage (`Iw*` names: tessellation, polygon, hidden lines, ray tracing, fillets and offsets, co-written 1998–2004 under a marketing agreement), which waited here for an owner to sign off their chain of title, are no longer excluded: they may be ported with the notice kept, like the rest of the release (29.5). The parts below stay excluded whatever their headers say, because their own origin or terms forbid our use, whoever republished them ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), overview-legal and its challenge).

| Part | Why it is excluded | What we use instead |
| --- | --- | --- |
| Numerical Recipes-derived routines: `SmMatrix::LUDecompose`, `EigenSystemJacobiSolver`, `SVDecompose` and `SolveWithSVDMatrices`; all of `SmIntegrator` (`PolynomialInterpolation`, `Trapazoid`, `QRomberg`, `QSimpsons` and the Cash–Karp step with NR's step-control constants); `SmLocalSolve1d::SolveByBrent`; `smos_Pythag`; and in NLib `N_SingleValueDecomposeSolve`, `N_RealMatrixSingleValueDecompose`, `N_FuncFindMinima`, `N_RealMatrixEigenValuesVectors`, `M_TridiagonalSystem` and `M_CorneredTridiagonalSystem`; and NLib's quadratic solver `N_SolveQuadraticEq` ([`NL_Math.cpp` L2496-L2671](https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Math.cpp#L2496-L2671)), whose comment at L2512-L2516 quotes Numerical Recipes in C, page 184 (checked 2026-10-21) | Numerical Recipes code is not open source, and NVIDIA cannot license what it does not own | The primary sources in 29.4. For the quadratic, the standard formula that avoids cancellation, as Higham gives it. An owner may instead record that the routine is not derived from Numerical Recipes |
| solidcore's `geom/matrix.rs`, `mass/integrator.rs` and `geom/local_solve1d.rs`, and its `nlib.math` unit (the port of `NL_Math.cpp`) | Ports of the routines above; the first three are labelled as NR in their own comments | The same |
| `proposals/` and `source/schema/omniSolid/resources/schema.usda` | Covered by the OpenUSD Technical Proposal Supplemental Terms. The schema repeats the AOUSD proposal | The upstream AOUSD proposals, read where they are published |
| The `OCCT_BREP_IMPORT` parsers | By their own comments, written against OCCT's LGPL reader functions | Method only (29.14) |
| The `TestFiles/` fixtures (an ACIS `.sat` file from 2009, `.iwi`, `.smb` and `.smp`) | Third-party customer files | Inputs we create ourselves |
| `tools/smlib_gui` | Pulls in PySide6 (LGPL and GPL) | — |
| The names SMLib, NVIDIA, Omniverse and OpenUSD | Trademarks. Apache-2.0 section 6 grants no trademark rights | Our own names |

## 29.7 Curves, surfaces and intersections (`geom-nurbs`, `geom-curve`, `geom-surface`, `geom-intersect`)

From [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) and [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md) (nurbs-core).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| The NURBS Book (Piegl and Tiller, 2nd edition) | Springer copyright | PORT METHOD | PORT METHOD (7, 12) | The primary source for NURBS methods. Algorithms cited by number in doc comments (section 7). Its pseudocode and text are never transcribed |
| NLib methods (licence in 29.5) | Apache-2.0 | PORT, minus 29.6 | PORT or PORT METHOD (7, 12, 13) | Methods to take, each checked against the book or the papers first, then ported from NLib where that helps, NVIDIA's notice kept (32.8): knot removal with the sharper bound from basis-function maxima; approximation to a stated error; degree reduction with error bounds; making curves compatible within a knot tolerance; skinning with G1 or G2 boundary continuity; N-sided hole filling; swung and translational surfaces; Gordon surfaces, with a consistency check in place of silent averaging; quadratic, quartic and quintic rational arcs; exact offsets of special surfaces; global projection by trees of Bézier spans with oriented boxes and tangent cones; products of B-spline functions. Each of NLib's silent fallbacks (clamped parameters, averaged crossings, changed inputs, results returned together with an error) becomes a typed error |
| geomdl (NURBS-Python) | MIT | PORT | ORACLE (out of process), STUDY | Its comments map code to algorithm numbers. Too slow to port |
| verb | MIT | PORT | STUDY | Its sampling calls `Math.random()`, so it is not deterministic |
| tinyspline | MIT | PORT | PORT (13) | Rotation-minimising frames by double reflection (Wang et al. 2008) and small curve routines. Trigonometry goes through `geom-num` |
| G+Smo | MPL-2.0; it bundles a copy of openNURBS | PORT (except the bundled openNURBS) | PORT (7, 12) | Boehm knot insertion and refinement, fitting and Coons patches. These files stay MPL-2.0 with their headers |
| curvo | MIT | PORT | STUDY, ORACLE (dev-dependency) | Its intersections are numerical optimisation, not certified. Its tests may be ported with the MIT notice |
| openNURBS (McNeel) | The LICENSE file holds "All Rights Reserved", a warranty disclaimer and the zlib text, but no grant. Permission appears only on McNeel's web page | STUDY until McNeel confirms | STUDY; `geom-3dm` waits | No code taken, and no 3dm reader written from its code, until McNeel confirms in writing that the repository's code may be redistributed inside an MPL-2.0 project (29.21). Its SubD files are not read for porting |
| SISL; GoTools (SINTEF) | AGPL-3.0. SISL adds a section 7(b) term requiring a producer line; commercial licences are sold | REJECT | REJECT | Not read. SINTEF's published papers are PORT METHOD |
| LNLib | LGPL-2.1 | PORT METHOD | PORT METHOD only where the book and papers leave a step open (7) | Method-only procedure |
| OCCT intersection and projection (`IntPatch`, `IntWalk`, `Extrema`) | LGPL-2.1 with the OCCT exception | PORT METHOD | PORT METHOD (12) | Dispatch by surface class. What not to copy: extrema seeded from a sampling grid, and the silent fallback from closed-form intersection to marching |
| Clothoids and G1fitting (Bertolazzi and Frego): the Fresnel routine | BSD-2-Clause, but [`Fresnel.cc`](https://github.com/ebertolazzi/Clothoids/blob/dafdc29acee5c4f25e777c6cffe56ecb39b0ec11/src/Fresnel.cc#L60-L78) (L60-L78 and L149-L158 at `dafdc29a`) says it was adapted from Thompson's 1997 Wiley book through Telasula's 2005 code, so its coefficient tables may not be Bertolazzi's to license | Stop and ask (chain of title, rule 2); ORACLE out of process | ORACLE (out of process) (17.8) | Not ported. The clothoid uses the general certified quadrature evaluator; a faster path may use our own series and asymptotic expansions written from DLMF §7.6 and §7.12, with derived remainder bounds (17.8) |
| Clothoids and G1fitting: the other files, and the authors' papers | BSD-2-Clause. Each file's header is to be checked for third-party provenance by an owner (17 open question) | Other files: PORT only after an owner checks their headers. Papers: PORT METHOD | PORT METHOD (17.8): G1 fitting ([2014](https://doi.org/10.1002/mma.3114)) and point projection ([2019](https://doi.org/10.1137/18M1200439)), written from the papers, for `geom-curve` and `geom-align` | Certified bounds added and maths through `geom-num` |
| Papers on surface intersection and projection: Patrikalakis, Maekawa and Cho; Barnhill and Kersey; Sederberg and Meyers; Hohmeyer; Grandine and Klein; Bartoň, Elber and Hanniel; Hass et al.; Yang, Jia and Yan; Cheng et al.; Ma and Hewitt; Hu and Wallner; Sederberg and Nishita | Papers | PORT METHOD | PORT METHOD (12) | — |
| LLNL Axom, `primal` | BSD-3-Clause | PORT | PORT (11, 19) | Generalised winding numbers for trimmed NURBS and Bézier geometry, which tell inside from outside even when an imported model has small gaps |
| Rhino SubD and ToNURBS public documentation | McNeel documentation, vendor-hosted | STUDY | STUDY (16) | Option names only, so that models round-trip |

## 29.8 Meshes, mesh booleans, spatial index and clash (`geom-poly`, `geom-mesh`, `geom-spatial`)

From [01](../../../kit/notes/G01-evidence/01-manifold-mesh-booleans.md), [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) section C, [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) and [10](../../../kit/notes/G01-evidence/10-github-sweep.md).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| Manifold | Apache-2.0 | PORT | PORT and ORACLE (option B, section 4; 19) | Ported with notices: the half-edge structure of arrays, property-vertex seams, MeshGL-style runs, and provenance by original and face IDs. Its global atomic ID counter is replaced by IDs derived from `OpId`. It runs as an oracle out of process. Its test models, samples and EMBER harness join our corpus. Its float boolean is not taken |
| manifold-rust | Apache-2.0 | PORT | STUDY, ORACLE | Its exact rational ("robust") boolean serves as an oracle. Its claims of bit-identical output and a clean Thingi10K run are unverified |
| Cherchi et al., InteractiveAndRobustMeshBooleans | MIT for the authors' code; it bundles Attene's LGPL predicates | PORT (MIT parts only) | STUDY, ORACLE (out of process) | No chained booleans, by design |
| libigl | MPL-2.0 core; GPL-3.0 in `include/igl/copyleft`; `include/igl/triangle` wraps the non-free Triangle. GitHub labels the whole repository GPL-3.0, which is not the licence | PORT (MPL files only) | PORT (19) | Fast winding numbers, signed distance, the AABB tree, `lscm`, `arap` and `planarize_quad_mesh`, each file path cited. `copyleft/` and `triangle/` are REJECT. Its CGAL-based mesh boolean runs only as an out-of-process oracle |
| Mesh arrangements (Zhou et al.); winding numbers (Jacobson et al.; Barill et al.) | Papers | PORT METHOD | PORT METHOD (11, 19) | — |
| Blender exact boolean (Trickey) | GPL-2.0-or-later | Papers only | STUDY through the papers it builds on (Zhou et al.; Cherchi et al.) | Source not read |
| OpenSCAD | GPL-2.0 with a CGAL linking exception | Papers and documentation only | STUDY (documentation) | Its CSG test corpus is neither read nor vendored |
| mcut | LGPL-3.0, or commercial | PORT METHOD | STUDY under the method-only procedure | — |
| Cork | LGPL-3.0; bundles the non-free Triangle | REJECT | REJECT | — |
| trueform | PolyForm Noncommercial, or a paid licence | REJECT | REJECT | Not read |
| MeshLib | Proprietary; non-commercial and education use only | REJECT | REJECT | — |
| MeshFix | GPL-3.0 | Papers only | STUDY through Attene's paper | Source not read |
| vcglib and MeshLab | GPL-3.0 | Papers only | STUDY through the papers | Source not read |
| csg.js; JSCAD; three-bvh-csg | MIT | PORT | REJECT for the kernel | Float BSP or float CSG. JSCAD's API and three-bvh-csg's interactive preview are worth a STUDY |
| PMP library | MIT | PORT | PORT later (19) | Remeshing, decimation, hole filling and fairing |
| CinoLib | MIT; optional hooks to TetGen (AGPL) and Triangle (non-free) | PORT (without the hooks) | STUDY | Its data structure |
| OpenMesh | BSD-3-Clause (unverified) | PORT | REJECT | PMP is its successor |
| geometry-central | MIT | PORT | STUDY; the signed heat method is PORT METHOD (19.7) | The signed heat method (inside and outside on broken meshes) lives in `geom-field`, reached through the conversion graph (16.3, 19.7). Section 19.7 takes it as a method only, because signed-heat-3d builds TetGen (row below). Stripe patterns and intrinsic triangulations: no section plans them yet |
| signed-heat-3d (Feng and Crane 2024) | MIT, but its `deps/tetgen` submodule is TetGen (AGPL-3.0); it also pulls libigl, geometry-central and amgcl (checked 2026-10-21 at [`3638fa48`](https://github.com/nzfeng/signed-heat-3d/blob/3638fa48a9317201b32d44db5b4cac8e651cc927/.gitmodules)) | PORT of its own MIT files only; TetGen is never read | PORT METHOD (19.7) | From the paper ([doi:10.1145/3658220](https://doi.org/10.1145/3658220)). The regular-grid version comes first and does not wait for our tet mesher (19.7) |
| wildmeshing-toolkit | MIT | PORT | STUDY | Local mesh edits with invariant checks and rollback |
| fTetWild | MPL-2.0. It links Geogram and libigl, with an optional TetGen path and GMP (checked 2026-10-21 at [`8118f810`](https://github.com/wildmeshing/fTetWild/blob/8118f810478e0e65a7bf2d8cecdc5e203a876e97/CMakeLists.txt)) | PORT (fTetWild's own files only) | PORT later (19) | Robust tetrahedral meshing for simulation. It inherits Geogram's and libigl's sub-folder traps: take only its own MPL files and rebuild what they call |
| fast-envelope (Wang et al. 2020) | MIT, with Marco Attene among its copyright holders (checked 2026-10-21 at [`f50eb6c1`](https://github.com/wangbolun300/fast-envelope/tree/f50eb6c1572c5865bfab3cac1acae0e27c8b4a82)). Its `indirectPredicates/` files carry no header and appear to be Attene's indirect predicates, which are LGPL (29.3) | PORT for files outside `indirectPredicates/`; that folder stop and ask (rules 1 and 2) | PORT METHOD (19.8): the polyhedral envelope with integer corners, its predicates re-derived in our own code | Its exact library sets the processor's rounding mode, which WebAssembly cannot do (19.8) |
| TetWild | MPL-2.0, but contains CGAL GPL code | STUDY | STUDY | — |
| Gmsh; Netgen; Mmg | GPL-2.0-or-later with a linking exception; LGPL-2.1; LGPL-3.0-or-later | Gmsh papers only; the others PORT METHOD | STUDY | — |
| PolyFem | MIT | PORT | REJECT for the kernel | A concern for a future analysis program |
| Instant Meshes; QuadriFlow | BSD-3-Clause; BSD-style. QuadriFlow bundles LEMON (Boost Software License), a MiniSat-derived SAT solver (MapleCOMSPS_LRB, MIT), pcg32 (Apache-2.0) and `pss` (not checked) (checked 2026-10-21 at [`810b7a09`](https://github.com/hjwdzh/QuadriFlow/tree/810b7a0967c35b0dc85b4464e3835e26a756c967/3rd)) | PORT. Bundled files keep their own notices; `pss` only after its licence is checked | PORT if quad remeshing is needed (16); STUDY | Seeds are explicit operation inputs and random numbers come from `geom-num`'s counter-based generator, so pcg32 is replaced, not ported (16.3) |
| Tight Inclusion | MIT (checked 2026-10-21 at [`6f84001a`](https://github.com/Continuous-Collision-Detection/Tight-Inclusion/tree/6f84001a790a9d3362b7c13d410037140d83fa2c)) | PORT | PORT (10.2; 19 owns it) | Named by the spine's catalogue; no report has vetted more than its licence |
| parry | Apache-2.0 | PORT | STUDY | BVH builders, GJK and EPA, and its recipe for determinism |
| `bvh` crate | MIT | PORT, DEPEND | REJECT | `f32` and nalgebra |
| FCL; coal | BSD-3-Clause | PORT | STUDY | Distance and clearance queries |
| Embree | Apache-2.0 | PORT | STUDY | Its BVH builders. No WebAssembly, and SIMD specific to each CPU |
| OpenVDB | Apache-2.0 | PORT | STUDY | Sparse level sets, for a later field cache |
| meshoptimizer | MIT | PORT | PORT of the simplifier into `geom-mesh`, as the kernel's level-of-detail operation (19) | Levels of detail are a kernel mesh operation bound by principle 5 and the edition rule (26.2), so they cannot rest on C++ through FFI |
| `meshopt` crate | MIT OR Apache-2.0; a binding to the C++ | PORT, DEPEND | DEPEND in renderer and program crates only | — |
| `rstar` | MIT OR Apache-2.0 | PORT, DEPEND | DEPEND in program crates (selection in the interface); inside the kernel, PORT or our own index, pending section 19 | — |

## 29.9 2D drafting, regions and sketch solving (`geom-2d`, `geom-sketch`)

From [06](../../../kit/notes/G01-evidence/06-2d-drafting.md), [10](../../../kit/notes/G01-evidence/10-github-sweep.md), [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md) (geo) and [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| iOverlay (`i_overlay`) | MIT OR Apache-2.0 | PORT, DEPEND | DEPEND for display only, outside the kernel (14.5): screen fills, clip masks and previews in `engine-draw2d` or program crates, labelled Approximate. ORACLE (dev-dependency, 28.10). Its `EdgeOverlay` idea (user data carried through every split) is STUDY: `geom-2d`'s own arrangement records each edge's sources the same way (14.4) | As it is, it rounds crossing points and snaps them to endpoints within a radius that grows on each pass ([06](../../../kit/notes/G01-evidence/06-2d-drafting.md) section 2). That tolerance does not come from the ladder, so it cannot decide stored 2D topology such as hatch boundaries and regions |
| iTriangle | MIT OR Apache-2.0 | PORT | STUDY | — |
| iCurve | MIT | PORT | STUDY | Topology decided on chords snapped to an integer grid, while each piece remembers its source curve. Reconsider when it matures |
| cavalier_contours | MIT OR Apache-2.0 | PORT | PORT (14.5) | Its offset structure for polylines with arc segments: a raw offset with arc joins, then cleanup, then stitching. Rebuilt on exact predicates, with its absolute epsilons and `atan2` calls removed and no distance threshold: cleanup keeps the faces with a positive winding number, decided exactly, instead of discarding slices within fixed epsilons (section 14.5). Its boolean scheme, which needs two closed polylines that do not cross themselves, is not taken; 14.5's booleans work by overlay |
| Clipper2 (C++) | BSL-1.0 | PORT | STUDY | Vatti's scanline method; exact orientation tests in 128-bit integers |
| `clipper2-rust` | BSL-1.0 | PORT, DEPEND | ORACLE (dev-dependency) | A cross-check for straight-line offsets and booleans. Its claim to match the C++ exactly is unverified |
| `tirithen/clipper2`; `geo-clipper` | MIT OR Apache-2.0; ISC | PORT, DEPEND | REJECT | Wrappers around C++ |
| kurbo | Apache-2.0 OR MIT | PORT | STUDY | A reference for curve maths; a display dependency in 29.16 |
| linesweeper | MIT and Apache-2.0 files; which applies is unverified | STUDY until clear | STUDY | A sweep with proven error bars |
| JTS | EPL-2.0 OR EDL-1.0 (the EDL text is BSD-3-Clause) | PORT under EDL-1.0 | PORT (14). Ported files carry `SPDX-License-Identifier: BSD-3-Clause`, the identifier SPDX gives the EDL-1.0 text (29.2) | Its snap-rounding noder (Hobby 1999), polygonizer that reports dangles and cut edges, validity rules and overlay design. Its XML conformance suite is also under EDL-1.0, as [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md) settled, so it is stored and run as an oracle (14.14, 28.10, 28.12) |
| GEOS | LGPL-2.1 | PORT METHOD | REJECT | We read JTS, its permissive original |
| geo, geo-types, geo-traits | MIT OR Apache-2.0 | PORT | STUDY | Predicate dispatch chosen by scalar type, its taxonomy of invalid polygons, and its regression corpus from real panics. A program's GIS import may depend on it; the kernel never does |
| CGAL `Arrangement_on_surface_2` | GPL-3.0-or-later, or commercial | Papers only; ORACLE out of process | STUDY through the papers and the book by its authors (Fogel, Halperin and Wein, *CGAL Arrangements and Their Applications*, 2012); ORACLE out of process with circle-segment traits (28.10) | The doubly connected edge list, zone insertion, sweeps, overlays, and exact arcs with numbers of the form a + b√c: the research track for exact arc topology (section 14). Source not read from now on |
| LibreCAD | GPL-2.0 | Papers only; ORACLE out of process | STUDY through projective-geometry textbooks; ORACLE out of process for drafting edits (28.10, 28.11) | Conic intersection by pencils of conics is classical. Source not read |
| Autodesk DXF Reference (2018) | Autodesk documentation, published for implementers on Autodesk's own site | PORT METHOD | PORT METHOD (24): DXF format knowledge for `geom-dxf` | The README allows published specifications. No text copied. Which snap kinds the kernel offers is section 14's choice, made from geometry and mapped to DXF's DIMASSOC codes on exchange |
| ezdxf | MIT | PORT | STUDY, ORACLE (its audit, out of process) | DXF semantics |
| SolveSpace, `libslvs`, `rust_slvs` | GPL-3.0. A Python package classifier that says MIT is not a grant | Papers only; ORACLE out of process | STUDY through the papers; ORACLE out of process (28.10) | Its methods (minimum-norm Newton step by QR of Jᵀ, degrees of freedom from rank) are standard numerical linear algebra, also covered by ezpz and fiksi. Source not read |
| FreeCAD planegcs | LGPL-2.1-or-later | PORT METHOD, ORACLE | PORT METHOD (15); ORACLE out of process (28.10) | Method-only procedure: conflict groups, telling redundant constraints from conflicting ones, dense QR below about 1,000 unknowns |
| ezpz (Zoo) | MIT | PORT | STUDY (15.16): its test problems, and its residual formulas where they meet our rules, are ported with the MIT notice. ORACLE (dev-dependency, 28.10) | Not a base to build on: it uses `faer`'s run-time SIMD, forms JᵀJ, stops at absolute tolerances, drops constraints by priority level and panics in library paths (15.16) |
| fiksi (Linebender), with its `solvi` sparse solvers | Apache-2.0 OR MIT | PORT | STUDY closely (15.16): PORT of `solvi`'s sparse QR structure and the stacked-QR step after an audit, notice kept. ORACLE (dev-dependency) | Its decomposition into rigid clusters (after Hoffmann, Lomonosov and Sitharam) and its over-constraint finder are studied, not ported: its plan can differ between runs (inferred from the code, not run), and the finder pivots on an absolute 1e-8 (15.16) |
| fiksi's `colamd_rs` | A C2Rust translation of COLAMD (BSD-3-Clause, 29.4); its own licence not checked here | STUDY until its licence is checked | REJECT (15.6) | A mechanical translation containing `unsafe`, which principle 13 rules out. COLAMD is ported from its own source instead (29.4) |
| `faer` | MIT (checked 2026-10-21) | PORT, DEPEND | REJECT (15.6) | It picks SIMD kernels at run time, so its results are not expected to be bit-identical across x86, ARM and WebAssembly (inferred from its manifests, not tested) |
| Frontier | GPL-3.0 | Papers only | STUDY through Sitharam's papers | Source not read |
| kiwi (Cassowary) | BSD-3-Clause | PORT | REJECT | Linear constraints only |
| NoteCAD; GeoSolver forks | No licence; NOASSERTION (unverified) | REJECT | REJECT | Not read |
| `2d_geometric_constraint_solver`; dlineate | MIT | PORT | REJECT | Toy-level |
| D-Cubed | Proprietary | REJECT | REJECT | — |
| Papers: Owen 1991; Hoffmann, Lomonosov and Sitharam 2001; Zou et al. 2022; Farouki and Neff 1990; Vatti 1992; Hobby 1999 | Papers | PORT METHOD | PORT METHOD (14, 15) | — |
| CADCraft's 2D geometry | MIT OR Apache-2.0 (chain of title in doubt, 29.5) | STUDY | STUDY, as what to avoid | Arcs stored as angles, one absolute epsilon, closest points and offsets computed on tessellations, and silent substitutions |

## 29.10 Naming and history (`geom-name`)

From [07](../../../kit/notes/G01-evidence/07-persistent-naming.md) and [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| FreeCAD element map | LGPL-2.1-or-later | PORT METHOD | STUDY under the method-only procedure (20) | Names built from the operation and its sources, filled in by downward and upward passes. Its random duplicate suffix, random per-document offset, per-document interning and geometric "rescue" are what section 20 forbids |
| OCCT `TNaming` and `BRepTools_History` | LGPL-2.1 with the OCCT exception | PORT METHOD | PORT METHOD (20.3, 20.4) | The history relations and their composition laws, as tested invariants. Its `Geometry` flag is the idea behind carrier references |
| Salsa | Apache-2.0 OR MIT | PORT | STUDY | Identity of tracked structs, durability and early cutoff. Its cancellation works by unwinding, which WebAssembly cannot do |
| rust-analyzer `AstId` | MIT OR Apache-2.0 | PORT | STUDY | A firewall per element: editing one element never invalidates the others |
| Onshape FeatureScript documentation | Onshape (PTC) documentation, vendor-hosted | STUDY | STUDY | Hierarchical operation IDs, and declaring which parts of an ID are unstable |
| Revit API reference | Autodesk documentation. The evidence read it on rvtdocs.com, a third-party copy whose authorisation is unknown | STUDY, from Autodesk-hosted pages only | STUDY, once re-checked on Autodesk's own API site | Public concepts only: references named by role (left, right, centre, top). The community's reverse-engineered reading of stable reference strings is not used |
| Parasolid XT format reference | As Parasolid in 29.5 | STUDY, vendor-hosted only | STUDY | How attributes behave on split, merge and transform |
| SMLib topology events (`SmAttribute`, `SmTopology::Notify`, `SmTrackTopologyChanges`) and solidcore's ports of them | Apache-2.0 | PORT for SMLib's files (32.8); STUDY for solidcore's | STUDY (20) | The list of edit events a naming scheme must observe. [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md) notes their gaps: split reuses the parent as one child, and only regions, faces, edges and vertices are notified |
| SolveSpace role remapping | GPL-3.0 | Papers only | REJECT | Section 20 has its own design |
| Papers: Capoyleas, Chen and Hoffmann 1996; Kripac 1997; Marcheix and Pierra 2002; Bidarra et al. 2005; Baba-Ali et al. 2009; Raghothama and Shapiro; Farjana and Han 2018 (CC BY-NC-ND: cited, never adapted) | Papers | PORT METHOD | PORT METHOD (20) | — |

## 29.11 Free-form, sweeps, offsets and blends (`geom-subd`, `geom-field`, `geom-sweep`, `geom-offset`, `geom-blend`, `geom-convert`)

From [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), [10](../../../kit/notes/G01-evidence/10-github-sweep.md) and [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md) (operations-io).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| OpenSubdiv (Pixar) | Tomorrow Open Source Technology License 1.0 (TOST): Apache-2.0 with section 6, on trademarks, changed. GitHub labels it NOASSERTION | ORACLE; code only after an owner-confirmed legal review | ORACLE (16) | Runs out of process in subdivision tests. Our evaluator is written from the papers (Catmull and Clark; Stam; DeRose, Kass and Truong; Nießner et al.; Peters for conversion), using OpenSubdiv's option names so that models round-trip |
| libfive | MPL-2.0 for `libfive/` (library, standard library, Python bindings); GPL-2.0-or-later for Studio and the Guile bindings | PORT (`libfive/` only) | PORT (16) | Octrees pruned by interval arithmetic, and dual contouring, for `geom-field`. The GPL parts are not read |
| Fidget | MPL-2.0 | PORT | PORT (16.2); ORACLE out of process (28.10) | Expression tapes, tape simplification from interval results, manifold dual contouring. Converted to `f64` with intervals rounded outward. Its JIT is not ported: no kernel crate generates machine code, and JIT evaluators exist only outside the kernel, for display (16.2, 23.14) |
| Manifold `LevelSet` and smoothing | Apache-2.0 | PORT | STUDY | — |
| TiGL (DLR) | Apache-2.0 | PORT (TiGL's own logic) | PORT METHOD (13.6): re-derived, not translated, because TiGL is built on OpenCascade types (13, open question 7) | Skinning with automatic parameters, curve-network interpolation and Gordon surfaces for `geom-sweep`. TiGL calls OCCT's API for lofting and approximation. Wherever it does, the replacement is PORT METHOD from published papers, never from reading OCCT |
| ShapeOp | MPL-2.0 in its file headers; the mirror read had no LICENSE file | PORT, once the licence file is confirmed | PORT (16) | Constraint projection for rationalising panels |
| Boundary-first flattening | MIT | PORT | PORT (16) | Parameterisation and unrolling |
| Topological Offsets (Zint et al. 2025), [`d75b55e`](https://github.com/wildmeshing/topological-offsets/tree/d75b55e4477dc95cd9e4781e0413d3a9d474d67d) | MIT for its own files. Its build fetches VolumeRemesher (LGPL-3.0, not verified per commit; row below) and GMP (LGPL), among other recipes such as libigl and fast-envelope, whose rows apply (13.11; checked 2026-10-21) | PORT of its own files only; VolumeRemesher and GMP PORT METHOD | PORT candidate (19.6) | Mesh offsets whose topology is guaranteed. Its distance band is checked by the exact tests of 19.10, not trusted (19.6). A design note confirms its guarantees before porting (19, open questions) |
| VolumeRemesher, [`8927a1e`](https://github.com/JcDai/VolumeRemesher/tree/8927a1e057c643f1193ed50117bb3fb311b81b09) (the repository now redirects to `wildmeshing/VolumeRemesher`) | LGPL-3.0 by its repository's licence. That commit has no root LICENSE file, so the licence is not verified per commit (13, open question 7; checked 2026-10-21) | PORT METHOD | PORT METHOD, only if the Topological Offsets port needs its method (19.6) | Method-only procedure (rule 6) |
| Lifted Surfacing of generalised sweep volumes (Ju, Zhou, Du, Carr and Ju 2025), [`Jurwen/Swept-Volume`](https://github.com/Jurwen/Swept-Volume/tree/0bb260118443545029c98db2e98278fac27ba460) | Repository Apache-2.0; core dependencies under the Adobe Research Licence (non-commercial research only) and libigl's GPL CGAL module (each LICENSE.txt read for 13.17, at `0bb2601`, `c0e7253`, `b4c4a7e`, `5933c16` and `a798adb`) | PORT METHOD from the paper and its erratum only | PORT METHOD (13.17), after a patent check (29.17) | Swept tool volumes and clearance envelopes for motions that routes 1 and 2 of 13.17 do not cover. Neither the repository nor its dependencies are read; its README's claim that all third-party libraries are open source is wrong (13.17). Its output is Approximate until a certificate is proven |
| adobe-research/mtetcol [`c0e7253`](https://github.com/adobe-research/mtetcol/tree/c0e7253dd59f5f20b37f0bff541b86d226b83ac5); adobe-research/space-time-functions [`b4c4a7e`](https://github.com/adobe-research/space-time-functions/tree/b4c4a7e54400a2fa0389b2b738301e472c06282e); adobe-research/arrangement-benchmark [`5933c16`](https://github.com/adobe-research/arrangement-benchmark/tree/5933c167f2277a10c1342e3e4c6de72bdfb94b4a); qnzhou/mtet (moving `main`, [`a798adb`](https://github.com/qnzhou/mtet/tree/a798adb85558584364f04474bc4fd54df66e1c26) when checked) | Adobe Research Licence: non-commercial research use only (13.17) | REJECT | REJECT | Never read. Lifted Surfacing's core comes from these |
| swept-volumes (Sellán); voroffset; PFPOffset | MIT; MIT; BSD-3-Clause | PORT | STUDY | — |
| offsets-and-remeshing; PCO | GPL-3.0 | Papers only | STUDY through the papers | Source not read |
| OffsetCrust | No licence | REJECT | REJECT | — |
| Curv; SDFX; fogleman/sdf | Apache-2.0 (archived); MIT; MIT | PORT | STUDY | — |
| ImplicitCAD | AGPL-3.0 | REJECT | REJECT | Not read |
| Sverchok | GPL-3.0 | Documentation only | STUDY | Cooperative cancellation between nodes, from its documentation. Source not read |
| Rhino.Compute | MIT | PORT | STUDY | Caching by content hash, which is sound only when results are deterministic |
| Evolute's tools; nTop | Commercial | REJECT | REJECT | Their papers and public talks only |
| IRIT | Free for non-commercial use only | REJECT | REJECT | Elber's papers instead |
| SMLib fillet, blend and offset solvers | Apache-2.0. Their IntegrityWare lineage is accepted with the rest of the release (32.8, decided 2026-10-24) | PORT | STUDY, and PORT where section 13 takes a method (13) | Each method starts from its primary paper (Vida, Martin and Várady; Piegl and Tiller on offsets), and may be followed closely in this code and ported, rebuilt to section 3 with NVIDIA's notice kept. Offsets and shells first stop for IntegrityWare's patent claim (29.17). Its offset-surface defects, WR-160 to WR-165 in solidcore's catalogue (WR marks a wrong result), tell us what not to reproduce |
| Papers on subdivision: Stam 1998; Lacewell and Burley 2007; Biermann, Levin and Zorin 2000 | Papers | PORT METHOD | PORT METHOD (16.1) | Section 16.1 takes their methods, not their code: any reference code the authors published is not used. Eigen-tables come from our own generator |
| Papers: Pottmann et al., *Architectural Geometry*; ShapeUp; LSCM; ARAP; Vida, Martin and Várady; Maekawa; Plantinga and Vegter; Duff; Snyder; Pasko et al.; Ju et al. (dual contouring) | Papers | PORT METHOD | PORT METHOD (13, 16) | Check 29.17 first for T-splines, watertight trimming and lattices |

## 29.12 Survey, terrain, alignments, roofs and point clouds (`geom-terrain`, `geom-align`, `geom-skeleton`, `geom-cloud`)

From [10](../../../kit/notes/G01-evidence/10-github-sweep.md) and [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) section 4.

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| spade | MIT OR Apache-2.0 | PORT, DEPEND | PORT into `geom-mesh::cdt` (19.3, 19.14): its structure rebuilt on integer predicates, with ties broken by Simulation of Simplicity, notices kept. It serves tessellation and terrain; 17.3 names it the structural model. Its floating-point crossing routine is not taken: terrain crossings use Geogram's exact construction (29.3). DEPEND in program and display crates; ORACLE (dev-dependency, 28.10) | As it is, it computes constraint crossings in floating point and limits the coordinate range, so a triangulation it decides cannot carry the proven bound of 10.2. Whether its `hashbrown` internals leak iteration order is unverified ([03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md)) |
| startin | MIT | PORT | PORT (17) | Vertex deletion and interpolation methods |
| delaunator, delaunator-rs | ISC | PORT | REJECT | No constraints |
| martini; delatin; hmm | ISC; ISC; MIT | PORT | STUDY | Level of detail for terrain display |
| tin-terrain | MIT, archived | PORT | REJECT | — |
| odrSpiral (DLR) | Apache-2.0 | PORT | PORT (17) | A small spiral reference |
| libOpenDRIVE | Apache-2.0 | PORT | STUDY, PORT (17) | Lane geometry for `geom-align` |
| pyclothoids; qaws | MIT | PORT | REJECT | — |
| clothoid-rs | No licence | REJECT | REJECT | — |
| openbimrs/ifc (`ifc-alignment`) | AGPL-3.0 | REJECT | REJECT | Not read |
| CGAL `Straight_skeleton_2`; surfer2 | GPL-3.0-or-later; GPL-3.0 | Papers only | STUDY through the papers (Aichholzer et al. 1995; Felkel and Obdržálek 1998; Palfrader and Held's papers on surfer2) | No robust skeleton exists under a permissive licence, so Spare Kit leads here (17). Source not read |
| campskeleton (Kelly) | Apache-2.0 | PORT | PORT (13.15) | Weighted straight skeletons and procedural extrusion for roofs, rebuilt on exact predicates in `geom-skeleton` |
| polyskel | LGPL-3.0 | PORT METHOD | STUDY under the method-only procedure | It has known bugs |
| kendzi-math; geo-buffer; streets-gl | BSD-3-Clause; Apache-2.0; MIT | PORT | STUDY | — |
| StrandedKitty/straight-skeleton | MIT, but it wraps CGAL (GPL) compiled to WebAssembly | REJECT | REJECT | — |
| raybevel | No licence | REJECT | REJECT | — |
| PDAL | BSD-3-Clause | PORT | Not used by the kernel. Programs may run it as an external tool | — |
| las-rs | MIT (checked 2026-10-21 at [`60a73c02`](https://github.com/gadomski/las-rs/tree/60a73c02f7ae399452f64d6f74aac7168029b075)) | PORT, DEPEND | Pending section 24: DEPEND at a `std` format edge that only decodes bytes into our types, or PORT | It needs `std`, so it cannot sit in a `no_std` kernel crate (21) |
| `laz` (laz-rs) | Apache-2.0 in its `Cargo.toml` and licence files; GitHub labels it NOASSERTION (checked 2026-10-21 at [`e82f4453`](https://github.com/tmontaigu/laz-rs/tree/e82f4453a3c83a9fb1309eb2167d3d9bad2052da)). Its README calls it a translation of LASzip, which is Apache-2.0 today | Stop and ask | Pending section 24 | Whether LASzip was under a different licence when laz-rs was translated from it is unverified, so its chain of title needs an owner's look |
| Open3D | MIT | PORT | STUDY | Section 17.10 fits many shapes by efficient RANSAC from the paper (Schnabel, Wahl and Klein 2007). Its random numbers come from `geom-num`'s counter-based generator, keyed by an explicit seed stored in the operation's input (16.3, 17.10), because `OpContext` holds no randomness (10.1). No section plans ICP registration yet. Any later port follows the same seed rule, and its results are Approximate |
| PoissonRecon | MIT | PORT | PORT (19) | From point cloud to mesh, in the Approximate class |
| PCL; Potree | BSD-3-Clause; BSD-2-Clause | PORT | STUDY | — |
| CloudCompare | GPL-2.0-or-later | Papers only | STUDY through the papers | Source not read |
| E57: ASTM E2807; the [`e57` crate](https://github.com/cry-inc/e57/tree/c759b1da81184bea3db5e093e7416729cb77cd27) (cry-inc); libE57Format | ASTM E2807: ASTM copyright, terms unverified. `e57` crate: MIT (checked 2026-10-21 at `c759b1da`). libE57Format: BSL-1.0 by its GitHub label, unverified | ASTM E2807: PORT METHOD, from a licensed copy. `e57` crate: PORT, DEPEND | DEPEND on the `e57` crate in program adapters, outside the kernel (24.11), once an owner has checked E57's terms (rule 11). libE57Format: no plan | Implement the standard; never copy its text |
| PROJ | MIT-style; its COPYING adds that "copyright information can be found in source files" (checked 2026-10-21 at [`5fa31c29`](https://github.com/OSGeo/PROJ/blob/5fa31c29f0a806568ed7bc76c1a557604db4a804/COPYING)) | PORT, DEPEND | DEPEND in Spare Construct's survey crates, outside the kernel, never called during an operation or a replay (17.1); ORACLE out of process (28.10). Whether the browser needs a pure-Rust PORT of the methods used is open (17, open questions) | Its results enter the kernel as recorded transformation results, converted once in double-double arithmetic (17.1) |
| The EPSG dataset, in PROJ's database | EPSG's own terms of use, unverified (17, open questions) | Stop and ask | Read through PROJ by program crates (17.1). Not shipped until an owner checks the terms | Frame records name coordinate systems by EPSG code, such as EPSG:2193 |
| LINZ grids and deformation models, among PROJ's grids | Believed CC BY 4.0, unverified (17, open questions) | Stop and ask | Used through PROJ (17.1) once an owner checks the licence; credited if CC BY | — |
| GeographicLib | MIT (28.10; not re-checked at a commit here) | PORT | ORACLE out of process (28.10) | Checks map projections and frame conversion |

The clothoid sources are in 29.7.

## 29.13 Drawings and hidden lines (`geom-draw`)

From [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) section 6, [06](../../../kit/notes/G01-evidence/06-2d-drafting.md) section 6 and [10](../../../kit/notes/G01-evidence/10-github-sweep.md).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| OCCT `HLRBRep` | LGPL-2.1 with the OCCT exception | PORT METHOD | PORT METHOD (18) | The edge classes: sharp, smooth, outline, visible and hidden |
| Krbn | MIT | PORT | STUDY (18.1, 18.4); ORACLE out of process for hidden lines of quadric scenes (18.11, 28.10) | Exact silhouettes and plane sections of quadric surfaces in closed form |
| hidden-lines (a Rhino plug-in) | MIT | PORT | STUDY | Line classes and snap rules for architectural drawings. It samples, so it is not exact |
| ln | MIT | PORT | REJECT | Works by sampling |
| Blender Freestyle | GPL | Papers only | STUDY through the papers | Source not read |
| FreeCAD TechDraw | LGPL-2.1-or-later | PORT METHOD | STUDY | Its references by index break when the model changes; section 18 avoids this |

## 29.14 Formats and standards (`geom-ifc`, `geom-step`, `geom-dxf`, `geom-3dm`, `geom-meshio`)

From [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) and [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md) (formats-legal).

| Source | Licence | Allowed | Planned | What, why and conditions |
| --- | --- | --- | --- | --- |
| IFC 4.3 documentation and schema (buildingSMART), [`7256780b`](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/LICENSE.md) | CC BY-ND 4.0. The same LICENSE.md says its technical documentation "may be photocopied, used in software development, or translated into another computer language" provided "full attribution is given", and that changes to the specification need written consent. IFC and ifcXML are buildingSMART trademarks. Material based on other standards is excluded from its copyright (checked 2026-10-21) | PORT METHOD | PORT METHOD (24) | Implement the schema in `geom-ifc`. Rust types may be generated from its EXPRESS schema, with full attribution. Link and quote briefly with credit; kernel docs are written fresh. "IFC" is used only to name the format |
| ISO 16739-1:2024; ISO 10303-42 and AP242 | ISO copyright | PORT METHOD | PORT METHOD (24) | Implement from licensed copies; copy no text. Whether code may be generated from ISO's EXPRESS schema files, and which edition of Part 42 is current, are unverified |
| IfcOpenShell | LGPL-3.0 | PORT METHOD | STUDY under the method-only procedure; ORACLE (IfcConvert out of process) | Its separate recipe layer ("taxonomy") and one mapping per entity. Its issues are a failure list rebuilt with our own inputs |
| web-ifc | MPL-2.0 | PORT | STUDY, ORACLE | Its fixed segment counts and absolute epsilons are what we avoid |
| That Open engine_components and fragments | MIT | PORT | STUDY | — |
| XbimEssentials, XbimGeometry | CDDL-1.0 | REJECT | REJECT | — |
| Speckle SDK | Apache-2.0; two server folders are under Speckle's Enterprise licence | PORT (Apache parts) | STUDY | Its exchange schema. Its B-rep objects are opaque proprietary blobs |
| Topologic; topologicpy | AGPL-3.0; LGPL-3.0 | Topologic papers and docs only; topologicpy PORT METHOD | STUDY | The cell-complex idea, from papers and documentation |
| val3dity | GPL-3.0 | Papers only | STUDY through ISO 19107 and Ledoux's papers | Source not read |
| ruststep; foxtrot | Apache-2.0; MIT OR Apache-2.0 | PORT | STUDY | STEP parsers generated from the EXPRESS schema, a reference for `geom-step` |
| OCCT `.brep` format | OCCT's published `brep_format.md` (LGPL documentation) | PORT METHOD | PORT METHOD, only if a program needs it (24) | Never via NVIDIA's parsers (29.6) |
| NIST STEP File Analyzer (SFA) | Terms unverified | ORACLE once the terms are checked | ORACLE (out of process) | Checks STEP files |
| 3dm | See openNURBS (29.7) | — | — | — |
| DWG: the ODA Open Design Specification, LibreDWG, acadrust, ACadSharp | The format knowledge was reverse-engineered from Autodesk's format. LibreDWG is GPL-3.0. acadrust is MPL-2.0 but was written with LibreDWG open and ports ACadSharp without its notice. ACadSharp is MIT | REJECT, pending an owner decision | REJECT | Section 27 has no DWG crate. LibreDWG and acadrust are never used, whatever the decision |
| ACIS SAT and SAB | No public specification | REJECT | REJECT | Foreign bodies may be carried only as opaque bytes, unchanged |
| SketchUp SKP (through OpenSKP) | Reverse-engineered | REJECT | REJECT | — |
| OpenUSD AOUSD proposals | OpenUSD Technical Proposal Supplemental Terms | STUDY | STUDY | Read where they are published upstream; nothing copied |
| miniz_oxide | MIT OR Zlib OR Apache-2.0 in its crate manifest (checked 2026-10-21 at [`cdd7ddd5`](https://github.com/Frommi/miniz_oxide/blob/cdd7ddd56cf57a6255203bb9f5c6a92bf1144bbf/miniz_oxide/Cargo.toml)) | PORT, DEPEND | PORT into `geom-exchange` (24.7): inflate and deflate, notice kept, run inside our size and ratio budgets | It is `no_std` and forbids `unsafe` code (24.7) |
| LandXML 1.2 schema ([landxml.org](http://www.landxml.org/schema/LandXML-1.2/LandXML-1.2.xsd)) | Terms not found; it cannot be pinned to a commit (17.13) | PORT METHOD, once an owner checks its terms (rule 11) | PORT METHOD (17.13): `geom-landxml` reads surfaces, alignments, profiles, COGO points and parcels | It names sixteen spiral types without formulas; each mapping to a curve kind waits for the curve-kind decision note (17.13) |
| The "Viennese Bend" transition curve, in IFC 4.3's alignment documentation | IFC's documentation writes "Viennese Bend (R)", a registered trademark; its owner and any patent are unverified (17, open questions) | Stop and ask | PORT METHOD of IFC's published definition (17.8), after an owner checks the trademark and patent status (29.17) | The name is used only to identify the IFC and LandXML type, never as the name of our own feature |

## 29.15 Test corpora (`geom-test`)

Test data has licences too. "Vendored" means a copy lives in our repository.

| Corpus | Licence | How we use it |
| --- | --- | --- |
| buildingSMART Sample-Test-Files | CC BY 4.0 | Vendored, with credit |
| IFC4.3.x-sample-models | CC BY-ND | Vendored unmodified, which CC BY-ND allows, with credit. Never edited |
| IFC4.x-IF alignment test set | No licence | An owner decides before any use (rule 11). Asking buildingSMART for a licence is listed in 29.21 |
| NIST MBE PMI STEP cases | US government publication; terms unverified | Fetched at test time by pinned hash, once an owner confirms the terms |
| ABC dataset | Each model belongs to its creator | An owner decides before any use |
| Thingi10K | Code Apache-2.0; each model has its own licence | Only models whose own licence allows our use, each model's licence recorded; vendored only under rule 9 |
| step-conformance (dan335), [`96f30bbe`](https://github.com/dan335/step-conformance/tree/96f30bbe09549ef37dbd0a956d70bd8a183fd5d2) | Code MIT OR Apache-2.0; `corpus/` CC0-1.0 (checked 2026-10-21). Created 2026-10-12. Its shop parts are Fusion exports from camcraft's test set, given under CC0 by the author. Its `adapters/brepkit` builds an AGPL program | `corpus/` vendored, with credit. The code is not taken and the brepkit adapter is never built |
| Manifold `test/models` and samples | Apache-2.0 | Vendored, with the notice |
| jyviko/ssi-corpus, [`b04c8a6`](https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9) | MIT OR Apache-2.0. Its 25 `y23_*` cases derive from Yang, Jia and Yan 2023 (CC BY) | Vendored, with the notice and credit to the paper. The acceptance gate for surface intersection; the bar to beat is the C++ SMLib baseline of 81 of 119 |
| solidcore defect catalogue and `fixtures/nlib.*` | Apache-2.0 | Cases rebuilt from their descriptions as our own tests. No fixture copied |
| Failure lists: SMLib KNOWN_ISSUES and issues; Truck and SolidCraft; geo; OCCT's documented failures; IfcOpenShell and web-ifc issues | Various | Inputs rebuilt from each description. No fixture copied |
| The K.1 spike corpus (getcadcraft partcraft) | Apache-2.0 | Re-created by us. Nothing copied |
| CadQuery's test cases | Apache-2.0 | Recipes rebuilt as our own operation logs and stored (28.12). Nothing copied |
| rhino3dm's sample 3dm files | MIT repository; each file's origin to check | Fetched once each file's origin is checked (28.12). openNURBS code is never read |
| [3MF conformance suites](https://github.com/3MFConsortium/test_suites/tree/f483d3beee069e7adbac125d1f7baa36b5b3e5bf) | BSD-2-Clause (24.12; not re-checked here) | Used with the notice, for 3MF reading (24.12) |
| ezdxf's example DXF files | MIT repository; each file's origin to check | Fetched once each file's origin is checked (28.12) |
| OpenSCAD's CSG corpus | GPL-2.0 | Not read, not vendored |
| Recorded oracle outputs | Data we generate | Vendored with the tool's name and version and the inputs. If a tool's licence claims rights over its output, stop and ask |
| Spare Kit's own fixtures | Not yet decided (an open question) | — |

## 29.16 Development and display tools (never kernel truth)

Nothing here enters a kernel crate.

| Source | Licence | Use | Conditions |
| --- | --- | --- | --- |
| proptest, bolero, cargo-fuzz, cargo-mutants, cargo-llvm-cov, gungraun (formerly iai-callgrind; Apache-2.0 OR MIT, checked 2026-10-21 at `a11cdc15`) with the summary model of its `gungraun-runner` crate, and serde_json, which read the gate's summary files (G18a), criterion, divan, cargo-semver-checks, cargo-deny, Kani, Verus | MIT, Apache-2.0, or both | DEPEND (development only) | — |
| Valgrind; git | GPL-2.0 | Separate development tools | Never linked; gungraun runs Valgrind as a program |
| wasmtime | Apache-2.0 WITH LLVM-exception (checked 2026-10-21 at [`5afd6358`](https://github.com/bytecodealliance/wasmtime/tree/5afd63588b1f5520391c6a37fc38dfd3ccd607c2)) | DEPEND (development only) | Counts WebAssembly fuel for the benchmark gates (23.13, 28.14) |
| FPTaylor | MIT (checked 2026-10-21 at [`b5a77cae`](https://github.com/soarlab/FPTaylor/tree/b5a77cae348400f21f83512210d9f43c4bffb381)) | A separate tool, out of process | Proves the error bounds of predicate filters, with HOL Light certificates (28.6) |
| Gappa | CeCILL or GPL (28.6; not re-checked here) | A separate tool, out of process | Proves the error bounds of predicate filters, checked by Rocq (28.6). Never linked; source not read |
| tiny-skia | BSD-3-Clause (checked 2026-10-21 at [`5d475477`](https://github.com/linebender/tiny-skia/tree/5d4754777746eef0828be166896eaf482c49f8f2)) | DEPEND (development only) | Renders before, after and overlay pictures of changed drawings for reviewers. Pixels never gate (28.15) |
| Prusti | MPL-2.0 | STUDY | A prototype |
| Creusot | Its `creusot-std` crate is LGPL-2.1-or-later | REJECT | — |
| trimesh; Polyscope; rerun | MIT; MIT; MIT OR Apache-2.0 | DEPEND (development only) | — |
| wgpu; vello; lyon; kurbo; earcut | MIT OR Apache-2.0 (earcut: ISC, MIT and Apache files) | DEPEND, in programs, for display only | GPU floating point is not bit-identical across vendors, so nothing here feeds kernel decisions |
| glam | MIT OR Apache-2.0 | DEPEND at the render boundary only | — |
| nalgebra | Apache-2.0 | REJECT in kernel types | Heavy generics; slow compiles |
| ultraviolet; cgmath | MIT OR Apache-2.0 OR Zlib; cgmath unmaintained since 2021 | REJECT | — |
| slotmap; la-arena; typed_index_collections; rpds; imbl | Zlib; MIT OR Apache-2.0; MIT OR Apache-2.0; MIT; MPL-2.0 | STUDY | We write our arenas and shared chunks in `geom-topo` (8.2) |
| generational-arena; im; earcutr; thunderdome | MPL-2.0 (archived); MPL-2.0 (archived); ISC (archived); MIT OR Apache-2.0 (last release 2023) | REJECT | — |
| nanite-webgpu | MIT | STUDY | — |

## 29.17 Patents to avoid

A patent protects a method, not code, so a clean re-implementation does not escape it. The facts below come from Google Patents (checked 2026-10-21), which itself calls its status and assignee fields an assumption, not a legal conclusion. Nobody has reviewed the claims, so no row says we are free of a patent; each says only what we avoid.

| Patent | Assignee | Title or subject | Status | What we do |
| --- | --- | --- | --- | --- |
| US 9,269,189 | Autodesk Inc | "Conversion of T-spline models to trim-free T-spline models: functional case" | Active; adjusted expiry 2034-02-01 | Avoid the claimed method: no trim-free T-spline conversion, and T-splines are not first-class (section 16). Claims not reviewed; non-infringement not established |
| US 10,614,195 | University of Texas System; University of Utah Research Foundation | Reparametrising surfaces for continuity between them (the "watertightCAD" method) | Active; anticipated expiry 2037-02-15 | Avoid the claimed method. Section 19 plans edge-first watertight tessellation; its relation to the claims has not been reviewed |
| US 10,339,266 | The same | Constructing spline surfaces for continuity between surfaces | Active; adjusted expiry 2037-08-14 | Avoid the claimed method. Claims not reviewed |
| US 10,614,256 | The same | Using a model-space trim curve for continuity between surfaces | Active; anticipated expiry 2037-02-15 | Avoid the claimed method. Claims not reviewed |
| US 10,635,088 | Autodesk Inc | "Hollow topology generation with lattices for computer aided design and manufacturing" | Active; anticipated expiry 2039-01-24 | Stop and ask before building hollowing with lattices (sections 16 and 26) |
| US 7,274,364 | Brigham Young University (original), Autodesk Inc (current); inventor Sederberg | The T-spline core | Expired; anticipated expiry 2024-03-26 | Free to use, but its later family above still applies |
| US 6,222,553; US 6,300,960; US 6,489,960; RE41677 | Pixar | Hybrid and semi-sharp subdivision | Expired; anticipated expiry 2017-08-04 | Free to use: semi-sharp creases are allowed |
| US 6,389,154 | Silicon Graphics (original), Alias Systems and Autodesk (current); inventor Jos Stam | Exact evaluation of subdivision surfaces | Expired; anticipated expiry 2018-07-15 | Free to use |
| US 2008/0043023 A1 | Application by Loop and Schaefer | Approximate Catmull–Clark patches | Abandoned in 2012; never granted | Free to use, but not CAD-grade |
| nTop's "patent-pending" implicit modelling | nTop | Unknown | Unverified | Stop and ask before copying any nTop-described feature |
| IntegrityWare's "patented never-fail shelling and offsetting" | IntegrityWare | Unknown; no number found | Unverified | Stop and ask before following SMLib's offset methods closely |
| The "Viennese Bend" transition curve | Unknown | A rail transition curve whose curvature depends on cant (17.8) | Unverified; not searched. IFC's documentation marks the name as a registered trademark | Stop and ask before implementing it (29.14) |

**Families not yet searched.** The evidence searched only subdivision, T-splines, watertight trimming and lattices. These catalogue families (10.2) have had no search at all:

- direct face edits: move, replace and delete with healing;
- G2 and G3 Class-A blends and variable fillets;
- shells, offsets and thickening (beyond IntegrityWare's claim);
- booleans, cellular models and batch cutting;
- sheet-metal bending and unfolding;
- constraint solving and sketch diagnosis;
- persistent naming and reference resolution;
- clash, clearance and continuous collision detection;
- hidden lines, sections and drawing generation;
- straight-skeleton roofs;
- terrain, corridors and alignments;
- slicing and toolpaths;
- swept volumes of general motion (Lifted Surfacing, whose authors include Adobe researchers; 13.17);
- circuit-board pours and clearance;
- point-cloud fitting and registration;
- mesh simplification, remeshing and conversion from subdivision to NURBS;
- panelisation (Evolute, Pottmann et al.).

A family enters the Build stage only after a recorded search, by whoever an owner designates, and owner review of anything found. The record lives in the patent register (build piece "Patent register"). In the United States knowingly infringing can raise damages, so some organisations leave patent reading to counsel; who searches is an owner decision.

Apache-2.0's patent grant (SMLib, Manifold, OpenSubdiv's licence) covers the licensor's own code as contributed. It does not clearly cover a re-implementation of the method.

## 29.18 Also named in the evidence, nothing taken

- gptoolbox (MIT and Apache-2.0 files): STUDY.
- baby_shark (MIT) and hexasphere.js (no licence, and irrelevant): REJECT.
- poly2tri (BSD-3-Clause): REJECT, because it fails on duplicate and collinear points.
- Formlabs `cdt` (MIT OR Apache-2.0, unmaintained), artem-ogre's CDT (MPL-2.0) and libtess2 (SGI Free Software License B 2.0; worth reading for its winding rules): STUDY.
- awv-informatik/smlibpy (MIT; replacement SMLib files) and boolmesh (MPL-2.0; `hypermesh`'s ancestor): not evaluated.
- foldhash (Zlib) and rustc-hash (Apache-2.0 OR MIT): not needed, because kernel code never iterates a hash map.
- Bentley's US 12,380,642 B1: unrelated.

## 29.19 Areas no report has vetted

The ledger covers what the evidence examined. Programs the kernel serves need more. Nothing from a source in this list, or from any other source without a row, is taken until it has a row in this chapter. The licences below are GitHub labels or general knowledge, all unverified.

| Area | Who needs it | Candidate sources and likely terms (unverified) |
| --- | --- | --- |
| Circuit-board exchange and tools | Spare Electronics | Gerber (Ucamco's published specification), IPC-2581 (IPC standard), ODB++ (Siemens); KiCad (GPL-3.0 label: papers and documentation only) |
| Slicing for 3D printing | Manufacturing | CuraEngine and PrusaSlicer (AGPL-3.0 labels: not read) |
| Machining (CAM) | Manufacturing | OpenCAMLib (LGPL-2.1 label: method only) |
| Exchange formats | Every program | glTF (Khronos specification; label NOASSERTION); 3MF (lib3mf BSD-2-Clause label; the specification's terms unknown); OpenUSD (TOST, label NOASSERTION); IGES (an old public specification, status unknown); JT (ISO 14306) |
| Fonts, line types and hatch patterns for drafting | 2D drafting (section 14) | ISO 128 (implement, never copy); Hershey fonts (believed public domain with a usage notice). CADCraft's library is rejected (29.5), so no clean source is named yet |
| Thread, gear and fastener standards | Mechanical design (`geom-thread`) | ISO and ASME thread standards, ISO and AGMA gear standards. Implement from licensed copies; how tabulated values may be used is an owner question |

Rows are added under the approval rules at the end of 29.20.

## 29.20 Rules for every contributor

These rules apply to people and AI agents alike. They add detail to AGENTS.md section 5 and change none of it.

1. **Check the licence before you read.** Find it in the source itself, at a pinned commit: the LICENSE or COPYING file, the file headers and the package manifest.
   - Where these disagree, the strictest applies, and you stop and ask. Attene's LICENSE says LGPL-2.1 while its README says LGPL-3.0-or-later. openNURBS's LICENSE grants nothing while its web page grants everything.
   - A hosting site's label is not the licence: GitHub calls libigl GPL-3.0, and OpenSubdiv and laz-rs NOASSERTION.
   - Check sub-folders and bundled code too: Geogram's `third_party/` (TetGen is AGPL, Triangle and HLBFGS are non-free), libigl's `copyleft/`, Cherchi's bundled LGPL predicates, the copy of openNURBS inside G+Smo, the CGAL code inside TetWild, and anything that links them, such as fTetWild.
   - Then look the source up in this ledger. A source that is not in it gets a new row in the same pull request.
2. **Check the chain of title.** Make sure the licensor owns what it licenses. Signs of trouble:
   - comments saying code was "taken from", "based on", "adapted from" or "mirrors" another work (Numerical Recipes in SMLib; LibreDWG in acadrust; OCCT's readers in NVIDIA's parsers; Thompson's 1997 book in the Clothoids library's `Fresnel.cc`);
   - headers re-stamped with a new owner, or credits removed;
   - a licence change in the history (cadcore, `hypermesh`), or a translation of a project whose licence later changed (laz-rs);
   - AI-written code that reproduces known content (CADCraft's linetype table).

   Any of these: stop and ask.
3. **Check every dependency.** Every crate in the kernel workspace's `Cargo.lock`, whether a normal, build or dev-dependency, must pass the licence gate (build piece "Licence and dependency gate") against the allow-list in 29.2.
   - Inside kernel crates the only DEPEND is `libm` (29.1). Any other crate a kernel crate would use is a PORT.
   - Where a crate offers a choice, such as `Apache-2.0 OR GPL-2.0-only`, we take the permissive option and record the choice in the gate.
   - Optional features that pull in copyleft or non-free code stay off and are listed: csgrs's `image-io`, RGeometry's `rug`, CinoLib's TetGen and Triangle hooks.
   - `libm`, and any later DEPEND an owner allows, gets a written audit first (build piece "Dependency audit").
4. **Keep the notice and give credit.**
   - A **ported file** starts with the origin's copyright lines copied exactly, the origin's SPDX identifier and a pinned link to the file it came from. It says what we changed, because Apache-2.0 section 4 requires changed files to say so. It keeps its origin licence, which the crate's `license` field names beside MPL-2.0 (for example `MPL-2.0 AND BSD-3-Clause`). A file ported from an MPL-2.0 project stays MPL-2.0 with its original headers. A file ported from JTS carries `SPDX-License-Identifier: BSD-3-Clause`, because SPDX files the EDL-1.0 text under that identifier (29.2).
   - A **PORT METHOD module** is MPL-2.0 and ours. It copies nothing, so it carries no other party's copyright line. It cites the method, and credits every source it consulted with a credit line.
   - `geometry/NOTICE` holds every origin licence text and every credit, and ships with every binary. A generated third-party notices file adds the notices of every crate compiled into a shipped artefact, including per-file notices such as Sun's in `libm` and the contents of any upstream Apache NOTICE file (build piece "Third-party notices").

   A ported file starts like this:

   ```rust
   //! Semi-static filter generator for exact predicates.
   //! Method: Meyer and Pion's FPG (inria-00344297); bit budgets in section 6.2.
   //! Source: PORT from Geogram PCK, BSD-3-Clause,
   //!   https://github.com/BrunoLevy/geogram/blob/40d72165ceeeb0a61977d6fdf74d929bfdd77b7b/src/bin/pck
   //!   <the origin's copyright lines, copied exactly>
   //!   Changed for Spare Kit: integer-grid inputs, Rust output, constants checked against our bit budgets.
   //! SPDX-License-Identifier: BSD-3-Clause
   ```

   A PORT METHOD module that consulted NLib starts like this:

   ```rust
   //! Knot removal for NURBS curves, with a bound on the error it introduces.
   //! Method: The NURBS Book (Piegl and Tiller, 2nd edition), chapter 5.
   //! Consulted: NVIDIA USD BRep NLib, Apache-2.0, <pinned link to the file consulted>,
   //!   for failure cases only. No code copied.
   //! SPDX-License-Identifier: MPL-2.0
   ```

   A file ported from NLib or the rest of USD BRep (32.8) is a ported file like the first example: it starts with NVIDIA's copyright lines copied exactly, the `Apache-2.0` identifier, a pinned link to the origin file and what we changed.

   The generator that builds `MAP.json` (26.3) reads these `Source:` and `Consulted:` lines into one record per source. The same records produce `geometry/PROVENANCE.md` and `geometry/NOTICE`, so they can never drift from the code:

   ```rust
   pub enum Take { Depend, Port, PortMethod, Oracle, Study }
   pub struct SourceRecord {
       pub name: String,                // "Geogram PCK"
       pub link: String,                // pinned to a commit
       pub licence: String,             // SPDX expression, exactly as the source states it
       pub take: Take,
       pub method_note: Option<String>, // path under geometry/methods/; required for PortMethod from an LGPL or GPL source
   }
   ```
5. **Record the source in the pull request.** Every pull request that takes anything has an "Outside sources" table: for each source, the pinned link, the licence, the class from 29.1, the files affected and what was taken. AGENTS.md step 6 already asks for every outside source; this table is its fixed form. Reviewers compare the table with the headers, and CI compares the headers with `PROVENANCE.md`.
6. **The method-only procedure, for LGPL sources** (and for a GPL source only where a kernel owner has allowed reading, 29.2).
   1. First look for the method in a paper or a permissive source, and use that instead if it exists (JTS rather than GEOS; papers rather than SISL).
   2. One session reads the source. A session is one person, or one agent conversation. It pins the commit and lists the files read.
   3. That session writes a method note in plain words: the problem, the steps, the invariants, the failure cases and the published sources the method rests on. The note contains no code and no pseudocode that follows the source line by line. It names nothing from the source except public API and paper names. Its only constants are published ones, or ones derived and justified in the note.
   4. A second reviewer checks the note against the source for copied expression (not for method), then commits it to `geometry/methods/` with the source's pinned link and licence.
   5. A different session writes the code from the note and the papers, without the copyleft source open or in its context. The module header cites the paper and the note. The pull request names that session and states that the source was not in its context.
   6. Tests come from our own understanding. Test files and test vectors from the copyleft project are not copied, because they are under its licence too.
7. **Never write from memory what you may not copy.** People and AI models remember code, and AI-written "clean-room" code has already reproduced proprietary content elsewhere ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md)).
   - Any routine that Numerical Recipes also contains is written from the primary source named in 29.4 and must not follow NR's structure or names: no `ZEPS`, `SIGN` or `MOV3` macros, no `svdcmp`, `tqli`, `rkqs` or `pythag` naming.
   - Every constant is derived from the named primary source and justified in the module header. Standard values from the published literature are fine when derived and cited (a step-size safety factor of 0.9 and exponents of −1/(q+1) appear throughout the ODE literature). NR's own step-control set, with ERRCON 1.89e-4, is flagged for review.
   - No data tables from memory: no linetypes, hatch patterns, fonts or menu trees, and no command names, prompts or error wording from other products.
   - No code recalled from OCCT, CGAL or any other copyleft or proprietary kernel.
   - Output that looks like a known library's code is discarded and rewritten from the paper. The banned-source scanner catches the common markers; review catches the rest.
8. **Oracles stay outside.**
   - Copyleft, C++ and other non-Rust oracles run as separate programs, each in its own CI container, and exchange only files with our code. Kernel crates never link them and releases never contain them.
   - Oracle images are built in CI from pinned source and are never published. CI checks that no oracle image is pushed anywhere. If an owner later decides to publish one, the corresponding source of every GPL and LGPL component is published with it, and no image containing Numerical Recipes-derived code is ever distributed.
   - The SMLib and solidcore oracle builds compile Numerical Recipes-derived routines. They are built and run only after an owner's yes.
   - An oracle is used only where it is deterministic. For SMLib that means evaluation, knot operations, degree change, and fitting without random sampling. NLib's point-cloud fitting uses a hidden random generator whose sequence differs by platform, so it is excluded.
   - Disagreements are settled by exact or interval ground truth, then triaged.
   - Proprietary programs are never oracles for kernel behaviour. Whether to check that our exported files open in proprietary programs is an owner decision.
9. **Data has licences.**
   - We vendor only data under CC BY 4.0, CC0, a permissive licence, in the public domain, or made by us. CC BY-ND data may be vendored only unmodified, with credit.
   - Data with no licence, or with restrictive terms, is not used until an owner decides (rule 11).
   - Recorded oracle outputs are data we generate. They are vendored with the tool's name and version and the inputs.
   - Fixtures are never copied from copyleft or proprietary projects. Failure cases from issue trackers are rebuilt from their descriptions with our own inputs.
10. **Patents.** Before building in an area named in 29.17, read its row. An active patent means: avoid the claimed method, or stop and ask. A family not yet searched waits for its recorded search before it enters the Build stage. A paper or vendor that mentions a patent, or calls a method "patent-pending", means: stop and ask.
11. **When to stop and ask an owner.** Stop, note it on the pull request, and ask the contributor to raise it with an owner (AGENTS.md section 5) when:
    - a licence is missing, unclear, modified, contradicted inside the source, or stated only outside the repository;
    - you want to DEPEND on or PORT a source that is not in this ledger, add a STUDY row for a copyleft or unclear source, or move a source to a more permissive class;
    - you want to read GPL, AGPL, non-commercial or unlicensed source;
    - the chain of title shows any sign of trouble (rule 2);
    - documentation you need is only available as a third-party copy;
    - the work needs knowledge of a closed format, anything learnt by observing, decompiling or reverse engineering a proprietary program, or a proprietary program as a reference;
    - a method might be patented;
    - a dataset has no licence, or its terms restrict use or redistribution;
    - you notice output that may reproduce remembered code or data;
    - any contact with another project about code, data or partnership is needed;
    - anyone asks to relax a rule in this chapter.

**How the ledger is kept.** After acceptance, this chapter lives on as `geometry/PROVENANCE.md`, kept current the way ARCHITECTURE.md is. Its tables are generated from the source records in the code, plus the STUDY, REJECT and unvetted rows, which are written by hand.

- Adding a STUDY or REJECT row for a source on the allow-list needs no special approval.
- A STUDY row for a copyleft, unclear or unlicensed source, and any new DEPEND or PORT row, needs a kernel owner's yes in review.
- A licence exception or a legal judgment needs a decision note an owner accepts.

This adds four entries to the file table in 26.3 and the folder tree in section 27: `geometry/PROVENANCE.md` (this ledger, generated and kept current), `geometry/NOTICE` (every origin licence text and credit, shipped with every binary), `geometry/methods/` (method notes from the method-only procedure) and `geometry/PATENTS.md` (the patent register).

## 29.21 People and projects worth contacting

None of these people or projects has agreed to anything. Listing them says only that a conversation could help. An owner decides whether and when to write; no agent sends a message on its own. Pronouns are not stated in the evidence, so this note uses "they".

| Who | Why we would write | What Spare Kit can offer | Conditions and status |
| --- | --- | --- | --- |
| The EMBER authors at RWTH Aachen's Visual Computing Institute (Philip Trettner, Julius Nehring-Wirxel, Leif Kobbelt) | To ask whether reference code for EMBER and the octree-embedded BSPs exists, and under what licence. Also their view on extending the 26-bit bounds to 40 bits, and on deterministic parallel assembly | Credit, our test corpus, and published results at building scale | No public code was found (unverified). Permissive code would be PORT with notices. Otherwise nothing changes: the method is already ours to implement |
| Tim Schmidt, author of csgrs and the Hyper crates | To compare certified-predicate APIs, closure certificates and exact oracles, and to confirm `hypermesh`'s provenance (descended from MPL-2.0 boolmesh, now MIT under a notice for someone who does not appear among its contributors) before any reuse | Shared adversarial test cases, and comparisons between exact methods | Their code is STUDY for us. Nothing of theirs is planned for our code |
| The CADCraft maintainers (storytold; author Brandon Thomas), about community issue #414 | #414 is a proposal by a community user, lpgn, for a shared B-rep kernel crate with CADCraft as its first user. No maintainer has replied; all commenters are outsiders. CADCraft's own geometry policy (`plan/adr/0001`) is not public. A reply could offer Spare Kit as that kernel, used through `engine-geometry` | A kernel CADCraft could depend on | #414 asks for an MIT or Apache kernel in craft-libs, evaluated against Truck first and built by observing SOLIDWORKS as a black box. Spare Kit is MPL-2.0 and forbids knowledge taken from proprietary programs. Requirements could reach us only as plain-language needs, never as behaviour observed in AutoCAD or SOLIDWORKS. An owner must first settle the clean-room questions in [12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md) and the "CadCraft" name conflict |
| tangency-labs (solidcore and SolidCraft) | As a courtesy, to point out the Numerical Recipes-derived routines in solidcore (29.6), and to swap adversarial cases: their 337-entry defect log, the SSI corpus and our failures | Test cases and defect reports | Their `deny.toml` bans MPL-2.0, so anything we share must carry a licence they accept, which depends on the open question about our fixtures' licence. Nothing of theirs enters our shipped code |
| Robert McNeel & Associates | Written confirmation that the "no restrictions" grant on their web page covers the openNURBS repository at a named commit, including redistribution inside MPL-2.0 work. Ideally, a LICENSE file in the repository that states the grant | Correct 3dm support in open tools | `geom-3dm` waits for this |
| NVIDIA (USD BRep maintainers) | To ask, through an issue, about the ownership chain of the NLib and IntegrityWare-lineage code and about the Numerical Recipes routines | Defect reports from our oracle runs | They welcome issues but accept no contributions. The owner decided 32.8 on 2026-10-24 without waiting for their answer, so the ownership question is now for the record only. Nothing has been sent |
| buildingSMART | To ask for a licence on the IFC4.x-IF alignment test set | Test results for alignment geometry | Until then it is not used |

Pixar is not on the list yet. OpenSubdiv's licence first goes to an owner-confirmed legal review, and we write to Pixar only if that review leaves a question.
