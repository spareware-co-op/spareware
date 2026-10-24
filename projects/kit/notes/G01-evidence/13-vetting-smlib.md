# Vetting: NVIDIA USD BRep (SMLib) and solidcore

*Evidence for [G01](../G01-geometry-kernel.md), the Spare Kit geometry kernel decision note. Written 2026-10-21 by a Claude Opus 5.5 research agent that read the actual source of each project at the pinned commits linked below. GPL and LGPL code is described in the agent's own words, never copied. Claims it could not confirm from a primary source are marked unverified.*

NVIDIA published SMLib, a NURBS B-rep solid modelling kernel, as USD BRep under Apache-2.0 on 2026-10-08; tangency-labs began a Rust port, solidcore, on 2026-10-12. Each subsystem was evaluated against its actual source, then challenged by an independent skeptic. Read for G01 on 2026-10-21.

## overview-legal

**Commit sha:** c9c5979f04cea8c38213004d4f04df0dcfb40d05

**Findings:** REPO FACTS (gh api, 2026-10-21). github.com/NVIDIA-Omniverse/usd-brep: created 2026-10-08, 1,747 stars, 258 forks, Apache-2.0. Default branch main. HEAD is c9c5979 "Release 1.0.0" (2026-10-09). The whole public history is three squashed nvidia-omniverse-bot commits: "Initial commit" 1a55464, "Release 0.2.0" ca6ce70 and "Release 1.0.0" c9c5979. So the public repo is a release mirror, and no development history is public. There is no .github folder, so no public CI. One PyPI release, usd-brep 1.0.0; the wheel's licence expression is "Apache-2.0 AND LicenseRef-TOST-1.0 AND BSD-2-Clause AND BSD-3-Clause AND MIT AND BSL-1.0" because it bundles OpenUSD binaries. Docs: nvidia-omniverse.github.io/usd-brep (Sphinx/Doxygen API only, no roadmap). SMLib/NLib manuals are also on docs.nvidia.com/smlib.

SIZE. 1,522 blobs, 76.4 MB. Languages by GitHub: C++ 44.9 MB, Python 1.65 MB, C 0.46 MB.
By component (lines of code I counted from the downloaded tree):
- source/SMLib: 1,136k lines of code in total, of which the kernel is about 967k. That is 707k lines in 379 Sm* files (C++ classes) and 260k lines in 84 NL_* files (NLib, C-style NURBS).
- tests/prog_test: 44k lines.
- SmPyLib (pybind11): 18k.
- SM_API (the C-style wrapper): 12.4k.
- SM_API_test: 11.9k.
- OCCT_BREP_IMPORT: 9.6k. OCCT_BREP_EXPORT: 2.6k.
- BREP_SM_USD / BREP_USD_DATA / BREP_STAGING / USD_BREP_UTILS / SM_API_USD / HYDRA_USD_BREP: about 28k together.
- tools (GUI, validators): about 20k.

Monster files: SmBrep.cpp is 1.7 MB, SmSurface.cpp 1.59 MB, SmCurve.cpp 1.26 MB, SmCurveClass.cpp 1.16 MB, SmPoly.cpp 1.11 MB and SmFace.cpp 1.1 MB.

Data: TestFiles/pt_TestFiles is 25.6 MB (205 .smb, 74 .smp, .sat, .iwi, .stl, ...). Agent docs (.agents/ plus AGENTS.md) are about 570 KB.

BUILD. NVIDIA "repo" tooling, premake5 and Packman. Windows needs the host MSVC v142 toolset (VS2019, or VS2022 with v142). Linux uses build-essential. macOS is Apple Silicon only.
- deps/target-deps.packman.xml pulls OpenUSD 25.11.
- deps/target-deps-python.packman.xml pulls Python 3.12.15 and pybind11 2.11.1.
- SMLib itself links only TBB, through use_usd_tbb() (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/premake5.lua#L1-L40). The TBB code paths are compiled out in the shipped build (.agents/operations/concurrency.md).
- No OCCT dependency. OCCT_BREP_IMPORT/EXPORT are self-written parsers and writers for OCCT's .brep text and binary format (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/OCCT_BREP_IMPORT/src/OcctAsciiParser.cpp#L1-L40).
- AlphaSignal's report of "STEP/IGES/JT via bundled OpenCASCADE" is not supported by the source tree. The repo has no STEP, IGES or JT translators.

LAYERS:
- SMLib: the kernel. Mutable pointer graph Brep→Shell→Faceuse→Loopuse→Edgeuse→Vertexuse, non-manifold radial-edge style. Analytic and NURBS curves and surfaces, trivariate B-spline volumes, bend/twist volumes, fillets, offset/shell, sweeps, healing, stitching, SSI, HLR (SmHCR), ray tracer, polygon mesh booleans and decimation.
- SM_API: SmApi* C-style functions returning SmApiStatus.
- USD bridge: omniSolid BrepArray schema.
- _omni_solid / usd_brep: Python.

INCLUDED vs EXCLUDED from commercial SMLib:
- Included: NLib (NL_*, including NL_Iges.cpp, which converts IGES entities 126/128, not a translator) and SMLib core.
- The intersection capability that SMS sold as GSNLib appears merged into Sm*Intersector. No GSNLib-named files exist; unverified whether anything was dropped.
- TSNLib appears only in two comments (SmBrep.cpp:2125, SmCurveBoundedSurface.cpp:415).
- Excluded: composite topology SmCEdge/SmCFace. "// Remove Composites" stubs appear in 46 files and 432 lines (e.g. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBrep.cpp#L17-L28).
- Also excluded: VSLib (variational deformable modelling), SDLib, the PolyMLib product as such (though SmPoly* exists), and SMS's data translators (IGES, STEP, VDAFS, SAT, OpenNURBS).

HISTORY (web, secondary sources):
- Roots: Boeing TIGER (from 1979), then Robert Blomgren's Applied Geometry (1984, later bought by Alias/SGI).
- Solid Modeling Solutions (SMS) was founded early 1998 in Bellevue, WA, by Robert Blomgren and Jim Presti.
- 1998: joint marketing agreement between IntegrityWare, GeomWare and SMS created SMLib. IntegrityWare dates Release 1.0 to Sept 1998. IntegrityWare says it added filleting, shelling/offsetting, polygonal modelling, NURBS ray-firing, primitives and hidden-curve removal.
- NLib (Wayne Tiller and Les Piegl, GeomWare; the NURBS Book code) went to SMS in late 2001.
- 2004: the IntegrityWare alliance ended, and IntegrityWare launched SOLIDS++ from the same lineage.
- Business model: sold as C++ source by subscription. SIGGRAPH exhibitor 1998–2008.
- Known users: AWV Informatik ClassCAD (20+ years, per Develop3D). AWV also published awv-informatik/smlibpy (MIT) with replacement .cpp files. Genworks GDL also appears, per the title of a genworks.com/smlib page; details unverified.
- NVIDIA acquired SMS in May 2022 and dissolved it.
- Apache release 2026-10-08. Develop3D (2026-10-13) positions it for AI-to-CAD and Omniverse/OpenUSD pipelines, not as a CAD replacement. The article has no NVIDIA quote and no roadmap.

MAINTENANCE SIGNALS. README and CONTRIBUTING say the project is "currently not accepting contributions"; issues are welcome; security goes through NVIDIA PSIRT. 21 issues/PRs in 2 weeks: 1 closed issue, 18 open issues (17 of them correctness bugs) and 2 open external PRs. Collaborator martinpwnv responds (e.g. #4, #5). No stated roadmap or release cadence (unverified beyond 1.0.0).

PROVENANCE MARKERS IN CODE. All 902 SPDX-tagged files were re-stamped "Copyright (c) 2026 NVIDIA ... SPDX Apache-2.0", erasing the original authorship lines. The history survives only in comments:
- Developer initials and dates 2004–2019: "[bd 090309]", "RMB Jul 2007", "Oct 2006 GWC Author" (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmBendVolume.h#L7).
- "Contact SMS support" (SmAttribute.h:459).
- A customer comment "JLMCC FS ... ask SMS for validity" (SmBrep.cpp:21097).
- IntegrityWare class names left in comments: IwFace, IwTree, IwPolyEdge, IwTol::GetXSectTol3d (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmTess.cpp#L4945-L5073) and IwCurve::GlobalCurveIntersect (SmCurveClass.cpp:13259). The tolerance classes SmZoneTol3d and SmXSectTol3d mirror IntegrityWare's IwTol. This is direct evidence of shared IntegrityWare code lineage.

**Robustness exactness:** The kernel uses double precision with tolerance classes throughout: SmTol3d, SmZoneTol3d, SmXSectTol3d, SmApproxTol3d and SM_EFF_ZERO (source/SMLib/inc/SmTol.h:167-181). NLib uses file-local tolerance globals, for example KN_TOL = 1e-6 (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_SrfAdv.cpp#L23).

There are zero exact or adaptive predicates: no orient2d/orient3d, no Shewchuk, no interval arithmetic, no long double.

Errors are integer status codes, with the generic SM_ERR 1001 dominating. NLib uses NL_ERROR flags. Python raises RuntimeError.

Failure modes documented by NVIDIA (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/KNOWN_ISSUES.md#L8-L81):
- remove_fillet does nothing and reports success.
- heal_brep reports success when a repair stage failed.
- About 10 seam-healing defects on closed and periodic surfaces.
- Inside-out triangles next to fillets.
- Area and volume errors of 1.4% at default accuracy, up to 20%.
- Malformed BrepArray topology can read out of bounds and crash.

Public issues add more:
- #1: boolean_union silently drops a mirrored operand and reports manifold=True.
- #2: fillet_edges corrupts another Brep.
- #3: linear_sweep returns a zero-volume non-manifold body.
- #8: fillet on a 0.001-unit box does not return.
- #12: tessellate ignores chord tolerance.
- #13: SmApiShellBrepFull terminates the process with an access violation at an exact cavity collapse.
- #16: brep_relationship gives wrong results, and boolean_intersection fails with SM_ERR on tangent cases.

Mutation conventions mix three styles: "Pure", "Consumes" and "Mutates" (.agents/docs/OVERVIEW.md). The kernel has no invalid-geometry guarantee: AssertValid is opt-in, and the USD writer emitted invalid loops (#4, confirmed by an NVIDIA collaborator). Against "never returns invalid geometry; booleans never silently fail", the codebase fails by its own documentation.

**Determinism:** Not deterministic, and the code shows why.

1. Wall-clock-seeded RNG in topology classification. SmShell::IsInnerShellOf calls srand((unsigned)time(NULL)) and uses rand() ray directions to classify shell nesting (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmShell.cpp#L880-L915). SmFace::GetPointsInFace does the same: it seeds srand(time) and dithers sample points with rand() (SmFace.cpp:21167-21360). SmRayTracer.cpp:233 also uses rand().

2. Pointer-keyed std::unordered_map in SmMapPtrToPtr / SmRelation (source/SMLib/inc/SmMapPtrToPtr.h:42,115), used from 34 files. Iteration order therefore depends on allocation addresses, which change with ASLR and the allocator.

3. One process-global SmContext with unlocked mutable mark counters. The API is not thread-safe, and NVIDIA's rule is one process per job (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/operations/concurrency.md#L12-L30).

4. NLib uses thread-local globals (NL_PRIVATE_TLS) and many file-static globals.

5. Observed in the wild: issue #10. SmApiShellBrepFull and offset_brep on one filleted box give 6 different volumes, SM_ERR or non-manifold results across fresh processes, and also between calls inside one process.

There is no fused multiply-add (FMA) or floating-point-mode control anywhere, so bit-identical results between native and WASM are not addressed. WASM is mentioned only as a worker-isolation note.

**Tests and quality:** Tests:
- C++: tests/prog_test (26 files, 44k lines), SM_API_test (11.9k), plus USD, Hydra and utils tests. 38 C++ test .cpp files in total.
- Python: 14 test files (SmPyLib/tests, tests/python).
- Validators: tools/brep_validator and brep_geometry_validator, with BA.xxx schema rules and AssertValid levels.
- Fixtures: 25.6 MB (pt_TestFiles), plus usd_TestFiles and occt_breps.
- No public CI, no fuzzing, no property tests, no determinism tests.

Code style is 1990s C and C++: macros (SER, NL_ERROR, LT/GE/EQ operator macros), raw pointers, out-parameters, 1–1.7 MB source files, and dead commented-out debug writes (WriteToFile("ISSS_Surface1.sms")).

Documentation for agents is strong:
- AGENTS.md (19 KB).
- .agents/operations (booleans, fillets, offset, healing, sweeps, tessellation, queries).
- Skills, including a 67 KB B-rep format reference and a 26 KB healing architecture doc.
- An errors.md with per-code guidance.
- An honest KNOWN_ISSUES.md.

Public development history is absent (three squashed bot commits). Issue triage is active but slow (most of the 18 open issues are unanswered).

**Legal:** LICENCE TEXT. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/LICENSE is verbatim Apache-2.0. I diffed it against apache.org; the only changes are the filled-in copyright line "Copyright (c) 2020-2026 NVIDIA CORPORATION & AFFILIATES" and "{}" instead of "[]" in the appendix. There are no added terms, no patent rider, no export-control clause and no field-of-use limits.

There is no NOTICE file at the root. The only NOTICE is proposals/NOTICE. So Apache §4(d) adds nothing beyond §4(a–c): ship a copy of the licence, mark modified files, and keep the copyright, patent and attribution notices.

SPDX TALLY over all 953 downloaded text files: 902 Apache-2.0 and 3 MIT. The MIT files are repo.sh, repo.bat and tools/repoman/repoman_bootstrapper.py (NVIDIA bootstrap scripts). Every .cpp/.h/.c/.py/.lua file carries an SPDX header. No GPL or LGPL text appears in source/.

THIRD PARTY (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/THIRD_PARTY_NOTICES.md). All of these are dependencies or tooling, not part of the kernel:
- OpenUSD 25.11 (TOST-1.0, a modified Apache licence).
- oneTBB (Apache).
- hwloc (BSD-3).
- MaterialX (Apache).
- OpenSubdiv (modified Apache).
- CPython runtime (PSF).
- HACL* (MIT).
- pybind11 (BSD-3).
- usd-validation-nvidia (Apache / CC-BY-4.0 docs).
- An optional GUI that pulls PySide6/shiboken6 under LGPLv3/GPL at run time from PyPI. It is not in the tree, but tools/smlib_gui must never be ported.

proposals/UsdSolid and proposals/cad_geometry are unmodified AOUSD snapshots under the OpenUSD Technical Proposal Supplemental Terms, not Apache. Do not copy them (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/proposals/NOTICE).

Contributions: none accepted, so there is no CLA or DCO. Security goes through PSIRT. Export control: nothing stated in the repo; unverified.

Patents. Apache §3 grants NVIDIA's patent licence for its Contribution as embodied in the Work and its derivative works. A clean re-implementation of a method is not clearly covered. I found no SMS or NVIDIA patents on SMLib methods (unverified). IntegrityWare's site has been reported to claim "patented never-fail shelling/offsetting"; I could not find a patent number, so this is unverified, and it matters for SmOffset*. Google Patents US12380642B1 (Bentley, a proprietary solid modeller served to an open-source platform) is unrelated.

RISKS AN OWNER MUST CHECK BEFORE PORTING ANY CODE. NVIDIA's Apache grant is only as good as its chain of title.

(1) Numerical Recipes-derived routines. NR code is copyrighted by Numerical Recipes Software, and its licence forbids redistribution, so it cannot be cured by NVIDIA's relicensing. Upstream comments say these routines are taken or modified from NR:
- SmMatrix::LUDecompose ("Crout's method as described in Numerical Recipies in C", https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmMatrix.cpp#L652-L662).
- SmMatrix::EigenSystemJacobiSolver ("modified from the Numerical Recipes in C, jacobi method", including NR's ROTATE macro; SmMatrix.cpp#L985-L1200).
- SmMatrix::SVDecompose and SolveWithSVDMatrices (SmMatrix.cpp#L1700-L2240).
- NLib N_SingleValueDecomposeSolve and N_RealMatrixSingleValueDecompose (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Math.cpp#L8440-L9080).
- N_FuncFindMinima (Brent, citing Press et al.; NL_Math.cpp#L9360-L9600).
- N_RealMatrixEigenValuesVectors, structurally NR tred2/tqli with ST_pythag (NL_Math.cpp#L11027-L11310).
- M_TridiagonalSystem and M_CorneredTridiagonalSystem, "Based on Numerical Recipes in C" (NL_Math.cpp#L11712-L11900).
- The smos_Pythag macro (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmMath.h#L105-L109).
- SmLocalSolve1d::SolveByBrent, which uses NR's ZEPS, SIGN and MOV3 (dbrent) macros (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmLocalSolve1d.cpp#L14-L16).

Re-implement all of these from public-domain or primary sources: EISPACK tred2/tql2, Golub–Van Loan, Brent 1973, LAPACK (BSD).

The Rust port tangency-labs/solidcore already ports EigenSystemJacobiSolver (crates/solidcore/src/geom/matrix.rs at 0a676a0) while its NOTICE says no further third-party notices apply. So it inherits this risk, and SpareKit must not take that file from solidcore either.

(2) IntegrityWare co-authorship 1998–2004. IntegrityWare's own history says the 1998 agreement gave the three firms marketing and sale rights. It says nothing about copyright assignment. IntegrityWare authored fillet, shell/offset, polygon, ray-firing, primitives and HLR, and Iw* names survive in the code. Whether SMS/NVIDIA hold copyright, or merely the right to sublicense those parts under Apache, is unverified.

(3) NLib. NVIDIA's own manual says SMS acquired "exclusive rights for maintenance, distribution, and development" from Tiller/Piegl (GeomWare). Wikipedia says "purchased". Copyright ownership is therefore unverified, though NLib algorithms are the published NURBS Book algorithms anyway.

(4) Test data provenance:
- TestFiles/pt_TestFiles/acisTest.sat was written by "Gambit 2.4.6 ... ACIS 16.0.7 ... 2009", a third-party customer file.
- Fillet/*.iwi are IntegrityWare-format files.
- 205 .smb and 74 .smp are SMS-era customer fixtures.
Do not copy any fixtures; generate our own.

(5) Trademarks: SMLib, NVIDIA, Omniverse and OpenUSD get no grant under §6, so do not use them in names.

CONCLUSION. The NVIDIA-authored or NVIDIA-owned parts are portable into an MPL-2.0 project only file by file, keeping the Apache header plus the NVIDIA copyright line and marking modifications. MPL-2.0 can carry Apache-2.0-derived files with their notices. It is not true that "everything" is cleanly portable: the NR-derived routines must be excluded, and IntegrityWare-lineage modules need owner sign-off.

**Alignment:** Checked against each Spare Kit principle:

- Iron-clad: fails. Tolerance-based floating point, no exact predicates, and documented silent failures (KNOWN_ISSUES, #1, #6) plus a process crash (#13).
- Explicit structured errors: fails. A generic SM_ERR 1001, and success is reported on failure (heal, remove_fillet).
- Deterministic: fails. srand(time) in classification, pointer-hash maps, global context, and nondeterminism observed in #10.
- Persistent naming: absent. Identity is pointers; faces are found by index or geometry (#13 selects a face "by measured bounds").
- Immutable / pure: fails. Most operations "Consume" or "Mutate" in place under a global mutable SmContext.
- No unsafe: C++ throughout. Out-of-bounds reads are documented on malformed input.
- Full scale: a single SmTol scheme with absolute tolerances. No scale-aware model is evident (would need deeper study).
- Readable: the agent docs are excellent; the code (1.7 MB files, macro dialects) is not.
- Headless: yes. The kernel is headless; the GUI is optional.
- Licence: Apache-2.0 is compatible with MPL-2.0, with the provenance exclusions listed under legal.

Under the owner's rule (take methods, never another kernel's architecture), SMLib's architecture must not be ported. The only things that fit:
- Specific numerical methods: fillet and blend solvers, healing taxonomy, offset gap-filling, the SSI marching strategy. Each one should be re-derived from its primary literature and checked against this code.
- Use of the C++ build as an out-of-process differential-test oracle, restricted to its deterministic layers (NLib evaluation, knot ops, fitting, simple analytic intersections).

solidcore shows what a 1:1 translation becomes: it inherits SMLib's tolerance algorithms and makes "agreement to 1e-9 with SMLib" its correctness target. That conflicts with SpareKit's goal of exceeding the source rather than matching its bugs.

**Strengths**

- Genuine decades-old commercial NURBS B-rep kernel. Lineage runs back through Boeing TIGER, SMS (1998) and the Piegl/Tiller NLib. About 967k lines covering fillets (variable radius, corners, custom solvers), offset/shell, sweeps, skins, healing, stitching, SSI, HLR, trivariate volumes, mass properties, tessellation and polygon booleans.
- Clean licence on paper: LICENSE is verbatim Apache-2.0 with no riders. 902 of 905 SPDX-tagged files are Apache-2.0; the other 3 are MIT bootstrap scripts. No root NOTICE file, no GPL/LGPL in source/, and no OCCT link: the OCCT .brep I/O is a self-written format parser.
- Excellent machine-readable documentation: AGENTS.md, .agents/operations per operation, skills (a 67 KB B-rep format reference, a 26 KB healing architecture doc) and an error-code guide. All of it is useful as an operation checklist for what a full kernel must provide.
- Honest KNOWN_ISSUES.md and an active public issue tracker. Together they form a ready-made corpus of hard failure cases (mirrored booleans, shell collapse, tangent cylinders, filleted-box shells, seam healing) for our own tests.
- The SMLib core depends only on TBB (and that path is compiled out), so the C++ can be built as a standalone oracle without OpenUSD, which solidcore's harness/cpp does.

**Weaknesses**

- Architecture conflicts with every Spare Kit principle: mutable pointer graph, process-global unsynchronised SmContext, in-place mutation, status codes, no persistent naming, floating-point tolerances and no exact predicates.
- Nondeterministic by construction: srand(time(NULL)) plus rand() in SmShell::IsInnerShellOf and SmFace::GetPointsInFace, and pointer-keyed std::unordered_map (SmMapPtrToPtr.h). Confirmed in practice by issue #10, where shell and offset results vary across and within processes.
- Silent and invalid results are documented by NVIDIA itself: heal reports success on failure, remove_fillet is a no-op, booleans drop operands (#1), invalid loops are exported (#4), and area/volume errors reach up to 20%.
- Provenance is not clean throughout. At least 12 routines are self-described as taken or modified from Numerical Recipes in C, which is copyrighted and not redistributable. IntegrityWare co-authored code (Iw* identifiers survive) under a marketing-only agreement. NLib rights are described as 'exclusive rights' rather than copyright. Test fixtures come from third parties (a 2009 Gambit/ACIS .sat file, IntegrityWare .iwi files).
- Every file was re-stamped 'Copyright (c) 2026 NVIDIA', erasing per-file historical authorship. That makes chain-of-title checks harder.
- The public repo is a release mirror: three squashed bot commits, no public CI, no contributions accepted, no roadmap. Future maintenance depends on NVIDIA's priorities (unverified beyond 1.0.0).
- The code is hard to read: 1–1.7 MB single source files, a C macro dialect (LT/GE/EQ, SER, NL_ERROR), commented-out debug writes and dead composite-topology stubs in 46 files.

**Verdict:** DIFFTEST_ORACLE

**Verdict reason:** Legally, the NVIDIA-owned code can be ported under Apache-2.0 with its notices kept. Three things block treating all of it as clean: about a dozen Numerical Recipes-derived routines, IntegrityWare co-authorship with unverified title, and third-party test fixtures. Architecturally, SMLib is the opposite of Spare Kit: mutable global-context pointer topology, tolerance-only floating point, status codes, no persistent naming, and documented nondeterminism (time-seeded rand, pointer-hash maps, issue #10) and silent failures. So neither PORT_CODE nor a solidcore-style translation is acceptable.

Its value to G01 is threefold:
(a) A separately built, out-of-process reference oracle for our tests. Use it only on its deterministic layers (NLib evaluation, knot insertion and removal, degree elevation, fitting, analytic intersections), and triage disagreements; never treat its answers as truth.
(b) Method study for specific algorithms, which other subsystem reviewers will judge one by one. Each must be re-derived from primary literature.
(c) A capability checklist and failure-case corpus taken from its docs, KNOWN_ISSUES and issues.

PARTNER is impossible while NVIDIA accepts no contributions.

**What to take**

- Licence-compliance template, needed only if any NVIDIA-owned code is ever ported. Keep the 'SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES' and Apache-2.0 notices, plus the 'Copyright (c) 2020-2026' line from https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/LICENSE. Mark modifications. There is no root NOTICE file to reproduce.
- Differential-test oracle, NLib only: build source/SMLib plus source/SM_API out of process and compare our NURBS evaluation, knot ops, degree elevation and fitting against it. Starting points: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvAdv.cpp , https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvFit.cpp , https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_SrfLstSqFit.cpp . Implement our own versions from The NURBS Book, not from this code.
- Failure-case corpus. Rebuild these as our own tests, with inputs we create ourselves and no copied fixtures: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/KNOWN_ISSUES.md#L8-L81 and issues #1 (mirrored union drop), #3, #10 (shell nondeterminism), #12, #13 (cavity collapse) and #16 (tangent cylinders) at github.com/NVIDIA-Omniverse/usd-brep/issues.
- Operation catalogue to study, as a checklist of what a full kernel must expose: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/operations/INDEX.md and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/skills/smlib-kernel-healing/references/architecture.md (healing taxonomy) and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/docs/errors.md (error taxonomy, to contrast with our structured errors).
- Interop format study for a future USD B-rep exchange: the omniSolid BrepArray schema (NVIDIA, Apache) at https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/schema/omniSolid/resources/schema.usda . Read the AOUSD proposals at their upstream PRs #108/#109 rather than copying proposals/, which falls under the OpenUSD supplemental terms.
- Methods reference for a clean OCCT .brep reader, if we need one: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/OCCT_BREP_IMPORT/src/OcctAsciiParser.cpp (NVIDIA Apache, no OCCT code). Take the format knowledge only.
- EXCLUDE, never port or translate, the Numerical Recipes-derived code: SmMatrix.cpp#L652-L2240 (LUDecompose, EigenSystemJacobiSolver, SVDecompose, SolveWithSVDMatrices), NL_Math.cpp#L8440-L9600 and #L11027-L11900 (SVD, Brent, tred2/tqli-style eigen, pythag, tridiagonal solvers), SmMath.h#L105-L109 and SmLocalSolve1d.cpp#L14-L16 plus SolveByBrent, all at https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/ . Do not take solidcore's crates/solidcore/src/geom/matrix.rs either. Use EISPACK, LAPACK or Golub–Van Loan derivations instead.
- EXCLUDE pending owner sign-off on IntegrityWare title: tessellation, polygon, HLR, ray-tracing, fillet and offset modules showing Iw* lineage, e.g. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmTess.cpp#L4945-L5073 . Also exclude all TestFiles/ fixtures (acisTest.sat, *.iwi, *.smb, *.smp) and tools/smlib_gui.

### Challenge 1

**Refuted:** True

**Corrected verdict:** STUDY

**Errors found**

- Incomplete list of Numerical Recipes (NR) code to exclude. SmIntegrator.h and SmIntegrator.cpp describe themselves as 'taken from Numerical Recipies in C' (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmIntegrator.cpp#L4-L8 and inc/SmIntegrator.h#L5-L9). The NR-derived functions there are: PolynomialInterpolation, which is Neville's method, NR polint (SmIntegrator.cpp#L128-L183); Trapazoid, NR trapzd (#L186); QRomberg, NR qromb (#L221); QSimpsons, NR qsimp (#L276); and RungeKutta5thOrderStep/RungeKuttaCashKarpAdvance (#L368-L430). The last two use NR rkqs's exact constants: SAFETY 0.9, PGROW -0.2, PSHRNK -0.25, ERRCON 1.89e-4 and TINY 1e-30 (#L24-L30). This integrator computes face area and mass properties (SmFace.cpp#L5937, #L6095). The review's EXCLUDE list does not include it.
- solidcore has taken on more NR-derived code than the review says. The review names only EigenSystemJacobiSolver in matrix.rs. At solidcore 0a676a04c38229b10e924b5074cb6ade8ee897cf, matrix.rs also ports LUDecompose and labels it 'Numerical Recipes `ludcmp`' (crates/solidcore/src/geom/matrix.rs#L285-L286), and labels the Jacobi solver 'Numerical Recipes `jacobi`' (#L472-L473). crates/solidcore/src/mass/integrator.rs ports SmIntegrator's Cash-Karp ODE integrator (#L1-L9). crates/solidcore/src/geom/local_solve1d.rs ports SolveByBrent along with NR's ZEPS constant (#L27, #L64-L65). solidcore's NOTICE still says 'no further third-party notices apply'.
- The omniSolid schema is mislabelled 'NVIDIA, Apache'. source/schema/omniSolid/resources/schema.usda has no SPDX or licence header. It contains all 70 attribute names from the AOUSD UsdSolid preliminary schema at proposals/UsdSolid/schema.usda, which falls under the OpenUSD Technical Proposal Supplemental Terms (proposals/NOTICE; THIRD_PARTY_NOTICES.md#L386-L404). It also repeats the BrepArray doc text word for word: line 23 of both files ('Solid boundary representation models (Breps) rigorously partition space into regions...'). The review says not to copy proposals/, yet recommends this derived file as NVIDIA Apache code (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/schema/omniSolid/resources/schema.usda#L21-L23).
- The claim that the OCCT .brep reader is 'NVIDIA Apache, no OCCT code' and a clean methods reference is overstated. OcctBinaryParser.cpp says of itself: 'It mirrors BinTools_ShapeSet::Read / BinTools_{Curve,Curve2d,Surface,Location}Set::Read byte-for-byte' (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/OCCT_BREP_IMPORT/src/OcctBinaryParser.cpp#L4-L7). OcctAsciiParser.cpp cites OCCT source functions in its comments: BRepTools_ShapeSet.cxx (#L73), GeomTools::GetReal (#L109), GeomTools_CurveSet::ReadCurve (#L200), GeomTools_Curve2dSet::ReadCurve2d (#L296), GeomTools_SurfaceSet::ReadSurface (#L387) and BRepTools_ShapeSet::ReadGeometry (#L1041). The code was written against OCCT's LGPL-2.1 sources, so under the owner's rule it is LGPL method-study material, not a clean Apache methods reference. Literal copying was not checked (unverified either way).
- Issue #13 is attributed to the NVIDIA kernel without its caveat. The reporter built with awv-informatik/smlibpy's replacement files for SmOffsetExecutive.cpp, SmProtoTopology.cpp and SmVertex.cpp, and wrote: 'This failure has been observed in the patched build above. I have not reproduced it with all AWV replacements disabled. Please keep that qualification when triaging attribution.' (github.com/NVIDIA-Omniverse/usd-brep/issues/13, 'Version and build provenance'). The review cites #13 under robustness, alignment and weaknesses as a crash in NVIDIA's kernel. By contrast, #10 does hold on the unpatched release: a 2026-10-20 comment reproduces it with the PyPI usd_brep 1.0.0 wheel on Linux, both across processes and within one process.
- 'Persistent naming: absent' misses SMLib's attribute-propagation mechanism. SmAttribute.h documents callbacks through SmAObject::Notify that edit functions (booleans, blends, offsets) raise for these events: SM_NO_SPLIT_IN_BREP, SM_NO_MERGE_IN_BREP, SM_NO_TRIM_NO_SPLIT_IN_BREP, SM_NO_COINCIDENT ('Two Topology objs in different Breps are found coincident during Boolean operation') and SM_NO_RM_FROM_BREP. SmAttribute declares virtual Split and Merge methods (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmAttribute.h#L415-L460, #L705, #L724). There are 154 SM_NO_SPLIT/MERGE/COINCIDENT references in src/. This is not persistent naming: the docs admit it covers only Regions, Faces, Edges and Vertices, and SM_API does not expose it. Still, it is the ACIS-style operation-event method that operation-derived naming in G01 builds on, and what_to_take should include it for method study.
- The oracle scope 'NLib ... fitting' is not uniformly deterministic. NLib has a hidden LCG held in a function-static variable, `static unsigned long int next = 7` (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Math.cpp#L10237-L10248). NL_SrfFit.cpp uses it for random knot jitter in point-cloud fitting (#L2818-L2846) and for random point selection (#L5258-L5282). `unsigned long` is 32-bit on Windows and 64-bit on Linux, and RAND_MAX differs (32767 on MSVC, 2^31-1 on glibc). The sequence therefore depends on the platform and on what was called earlier in the process, and the shared static is not synchronised between threads. These are exported functions (NL_SrfFit.h#L76, #L85), not called internally.
- The legal section misses NVIDIA's own scoping of the licence. THIRD_PARTY_NOTICES.md#L1-L6 reads: 'NVIDIA-authored portions of this product are licensed under the Apache License, Version 2.0, except where otherwise noted.' This bears directly on the chain-of-title risks the review raises (IntegrityWare, NLib/GeomWare, NR). The review's compliance template ('there is no root NOTICE file to reproduce') leaves this header out, although solidcore reproduces it in its NOTICE.
- Several numbers are wrong. source/SMLib has 967,217 lines in total across 463 files (375 Sm*, 83 NL_*, plus NLI_Math.h, nurbs.h, StdAfx.h/.cpp and premake5.lua). It is not '1,136k total, of which kernel ~967k'; the whole source/ tree is about 1,043k lines. There are 375 Sm* files, not 379, and 83 NL_* files, not 84. The 205 .smb files are counted across the whole repo; TestFiles/pt_TestFiles holds 188.
- Some history the review attributes to IntegrityWare is not on IntegrityWare's page. integrityware.com/products/SOLIDS++/history.html gives no release date ('Release 1.0 Sept 1998' is not there), gives no year for the end of the alliance (2004 comes from Wikipedia's Solid_Modeling_Solutions page) and does not list 'primitives'. It names filleting, shelling/offsetting, polygonal modelling, NURBS ray-firing and hidden-curve removal. These details are unverified as stated.
- Minor imprecisions. (a) The 20% area/volume error is measured at relative_accuracy 1e-1, not the default, and only for faces whose trim curves have interior knots (KNOWN_ISSUES.md#L53-L57). (b) 'Single SmTol scheme' leaves out the two-scale 'pinhole in battleship' tolerance mode (SmTol.h#L68-L74; per-Brep m_dThisLargeSmallSizeRatio, SmBrep.h#L242) and per-topology tolerance growth (SmTol::UpdateObjectTolerance, SmTol.cpp#L1232). Despite its name, GetScaledZero returns a constant 1e-12 (SmTol.cpp#L1371), so the 'absolute tolerances' conclusion stands.

**Argument:** I checked the review against usd-brep at c9c5979f04cea8c38213004d4f04df0dcfb40d05 (source tree downloaded and grepped) and solidcore at 0a676a04c38229b10e924b5074cb6ade8ee897cf.

**Confirmed.** These parts of the review hold:
- Repo metadata matches: created 2026-10-08, 1,747 stars, 258 forks, three bot commits, no .github folder, one PyPI release (1.0.0) with the stated licence expression.
- LICENSE is verbatim Apache-2.0 apart from the filled-in copyright lines. There is no root NOTICE.
- SPDX tally is about 900 Apache-2.0 files plus 3 MIT. Contributions are not accepted.
- SMLib links only TBB and tbbmalloc, and the TBB paths are compiled out. OCCT is not linked, and there are no STEP, IGES or JT translators.
- Nondeterminism sources are real:
  - srand(time) in SmShell.cpp#L888/#L905, and in SmFace::GetPointsInFace (#L21183), which is reached with lSmpCnt=3.
  - Pointer-keyed std::unordered_map in SmMapPtrToPtr.h#L42/#L115, wrapped by SmRelation.
  - An unlocked global SmContext.
- No exact predicates and no floating-point or FMA control anywhere.
- The cited NR routines in SmMatrix.cpp and NL_Math.cpp, smos_Pythag, and SolveByBrent's ZEPS/SMSIGN/MOV3 macros all exist where stated.
- The Iw* comments exist. The composite stubs number 46 files and 432 lines.
- KNOWN_ISSUES.md content matches the review.
- Issue counts match: 19 issues (18 open) and 2 PRs. An NVIDIA collaborator confirmed the writer defect in #4.
- solidcore's README claims hold: about a third ported, agreement within 1e-9.

So the architectural rejection holds, and so do the bans on PORT_CODE and PARTNER.

**Factual and legal errors.** The review is still materially wrong in places:
- The NR exclusion list misses SmIntegrator. That file is self-declared NR, contains NR's rkqs constants, and sits on the area and mass-properties path. solidcore ported it too, as well as LU and Brent.
- It recommends the omniSolid schema as 'NVIDIA Apache'. The file has no licence header and reuses the AOUSD UsdSolid proposal schema, which is under the supplemental terms.
- It presents the OCCT .brep reader as clean. By its own comments it mirrors OCCT's LGPL reader functions.
- It reports the #13 crash without the reporter's caveat that it was seen only in an AWV-patched build.
- It omits NVIDIA's 'NVIDIA-authored portions' scoping sentence.
- Its line counts are off.

**Fit.** The oracle-first verdict is weaker than the review argues:
- On the layers where SMLib is deterministic (NURBS evaluation, knot insertion and removal, degree elevation), it implements textbook Piegl–Tiller algorithms in double precision with tolerances. Spare Kit's own principles call for exact arithmetic, so its truth oracle for these layers should be exact rational evaluation. SMLib agreeing with us there would only catch gross transcription errors.
- Part of the named 'fitting' layer is not deterministic. It uses a hidden static LCG whose output depends on whether `unsigned long` is 32 or 64 bits.
- On the layers where an oracle would matter (booleans, fillets, shell and offset, shell nesting), SMLib is nondeterministic and can silently give wrong results (#1, #10, KNOWN_ISSUES). Every disagreement would need manual triage, so it cannot be an automated oracle.
- Cross-platform bit-identity is also unaddressed in the build. No -ffp-contract or /fp flag is set (premake5-utils.lua#L135-L140), and Apple-Silicon clang fuses by default. That last point is unverified by building.

**Corrected verdict: STUDY.** The value lies in four things:
1. A capability checklist from .agents/operations.
2. A failure-case corpus from KNOWN_ISSUES and the issue tracker, rebuilt with our own inputs.
3. Method study, each method re-derived from primary literature. This covers fillet and blend solvers, the healing taxonomy, offset gap-filling and SSI marching, plus two items the review missed:
   - the attribute-propagation event set in SmAttribute.h (split, merge, coincident, trim-no-split), as input to G01's operation-derived persistent naming;
   - tolerant per-topology tolerances, as input to healing imported data.
4. Breadth signals for 'every kind of geometry' (trivariate, bend and twist volumes; NURBS ray-firing; HLR).

An out-of-process oracle can stay an optional secondary tool. It is safe only for the evaluation layers and not for the RNG-based point-cloud fitting.

**Legal corrections for the review's 'what to take' list.**
- Add SmIntegrator to the NR exclusions, and do not take solidcore's matrix.rs, mass/integrator.rs or local_solve1d.rs.
- Treat omniSolid/schema.usda as AOUSD-terms material.
- Treat the OCCT parsers as LGPL method study only.
- Carry the THIRD_PARTY_NOTICES header with anything ported.

## topology-tolerance

**Commit sha:** c9c5979f04cea8c38213004d4f04df0dcfb40d05 (usd-brep "Release 1.0.0", 2026-10-09; the repo has only 3 commits). solidcore checked at 0072058ad2392fce619871d14315e93637e1cf3f (2026-10-20).

**Findings:** B = https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05

1. DATA MODEL: a Weiler-style radial-edge non-manifold model with use objects at every level (NVIDIA's SMLib manual calls it "the radial edge structure").
- 11 topology classes: Brep, Region, Shell, Face, Faceuse, Loop, Loopuse, Edge, Edgeuse, Vertex, Vertexuse. See B/source/SMLib/inc/SmTopoTypes.h#L15-L81.
- Every face has exactly 2 faceuses (SAME/OPPOSITE). B/source/SMLib/inc/SmFace.h#L98-L123, B/source/SMLib/inc/SmFaceuse.h#L30-L42
- Every loop has 2 loopuses; the first loopuse is the outer loop. B/source/SMLib/inc/SmLoopuse.h#L45-L58
- One logical face-edge incidence is a mated pair of edgeuses, so a manifold edge has 4 edgeuses and a spine edge has 6 or more. They sit in a circular radial list ordered [primary, mate, radial, mate, ...] by the right-hand rule. B/source/SMLib/inc/SmEdgeuse.h#L53-L107
- An edgeuse carries CW/CCW loop links, a vertexuse, an optional 2D B-spline UV trim (stored once per mate pair, on the SAME use), and three `mutable` cached gap records.
- An edge owns one 3D curve plus an interval. B/source/SMLib/inc/SmEdge.h#L74-L94
- A vertex owns its point and a list of vertexuses. Each vertexuse points to a shell, loopuse or edgeuse. B/source/SMLib/inc/SmVertexuse.h#L108-L137
- A shell holds exactly one kind of use: faceuses, wire edgeuses or one vertexuse. B/source/SMLib/inc/SmShell.h#L40-L61
- A region has a void flag; the first region is the infinite region. B/source/SMLib/inc/SmRegion.h#L28-L41, B/source/SMLib/inc/SmBrep.h#L66-L80
- Radial insertion is by sector (B/source/SMLib/inc/SmEdge.h#L118-L123). Sectors are ordered by face binormals, with a step-off variant for tangent faces (B/source/SMLib/inc/SmEdgeuse.h#L195-L230).
- Faces may carry any analytic, NURBS, offset or blend surface, and edges any curve type. UV trims are documented as optional derived data.
- The best single description is NVIDIA's agent reference, which lists invariants, permitted forms (wire, lamina, manifold, spine, strut, embedded, seam, vertex loop) and known validator gaps: B/.agents/skills/smlib-brep-model/references/brep-format.md#L79-L345, #L617-L688, #L903-L922

2. IDENTITY AND MEMORY: raw C++ pointers in circular, doubly linked intrusive lists.
- Every topology object derives from SmTopology, which stores owner/next/last pointers, 5 mark counters, flags, 2 user indexes and a user pointer. B/source/SMLib/inc/SmTopology.h#L30-L110, B/source/SMLib/inc/SmOwningTopology.h#L15-L26
- Every object also stores a non-owning SmContext pointer. B/source/SMLib/inc/SmObject.h#L60-L90
- My estimate (not measured) is about 100 bytes of bookkeeping per object before payload.
- Traversal state (marks) lives inside the data.
- No persistent naming. Change tracking is a context-wide callback that collects add, remove and change lists of pointers, and the header itself warns that pointers are reused. B/source/SMLib/inc/SmTrackTopologyChanges.h#L33-L80
- Notify events form a useful provenance vocabulary: ADD/RM_FROM_BREP, SPLIT_IN_BREP(parent→child1, child2), MERGE_IN_BREP(survivor, deleted), TRIM_NO_SPLIT, COINCIDENT(A obj, B obj) during booleans, CHANGE_GEOMETRY, COPY. Attributes get Split/Merge/Copy hooks. B/source/SMLib/inc/SmTypes.h#L468-L620
- The persisted indexed form SmBrepData collapses each mate pair into one record and uses a signed radial index (−i−1 selects the second member). B/source/SMLib/inc/SmBrepData.h#L655-L700
- solidcore replaces the pointers with append-only arenas and typed u32 ids (https://github.com/tangency-labs/solidcore/blob/0072058ad2392fce619871d14315e93637e1cf3f/crates/solidcore/src/topology.rs#L1-L230). It keeps the same 11-class graph, mutable `get_mut` access, and arena-index identity.

3. TOLERANCE MODEL: per-entity tolerant modelling, half-migrated.
- SmConfig.h compiles SM_USE_OLDTOL; the "consistent" SM_USE_NEWTOL model is "not yet complete". B/source/SMLib/inc/SmConfig.h#L222-L234
- SmBrep.h describes its own tolerance code as a "confused mix" of the old and new models. B/source/SMLib/inc/SmBrep.h#L160-L251
- Face, Edge and Vertex each store a `m_sZoneTol3d`.
- Three tolerance roles: ZoneTol3d; XSectTol3d = sum of two zones; ApproxTol3d = Zone/2. B/source/SMLib/inc/SmTol.h#L104-L180, #L264-L512
- The default comes from a model-size bin, from 1e-7 to 1e-3, with 1e-5 for sizes 0.5–500. B/source/SMLib/inc/SmTol.h#L559-L575
- Edge and vertex tolerances grow to cover measured gaps: max(def, connected tolerance, 1.1·gap/2, 1.1·gap − def). B/source/SMLib/inc/SmTol.h#L602-L690
- Constants: EFF_ZERO 1e-12, EFF_ZERO_PARAM 1.2e-8, angle tolerance 0.5°, gap gain 1.1, pinhole ratio 1000. B/source/SMLib/inc/SmMath.h#L37-L58
- System defaults are `thread_local` statics. B/source/SMLib/src/SmTol.cpp#L70-L73
- IsSliver, IsNecking, HasSmallRadius, HasZeroNormal, HasZeroSpeed and HasCoincidentKnots are stubs that return FALSE with "NOT IMPLEMENTED". B/source/SMLib/inc/SmTol.h#L700-L705

4. VALIDITY CHECKS
- ValidatePointers runs about 146 numbered tests; tests 112–146 check that required pointers are present. Its contract comment is at B/source/SMLib/src/SmBrep.cpp#L30740-L30890.
  - It enforces a band: Brep tolerance ≤ entity tolerance ≤ 1000 × Brep tolerance (#L30811, #L30845).
  - It calls edge ValidateGeometry, which the source itself marks "CAN CHANGE DATABASE!!!" (#L30813).
- AssertValid has levels 0/1/2 and SM_WALK (B/source/SMLib/src/SmBrep.cpp#L34754-L34766). Its header says it never changes the Brep, yet it const-casts to call ValidateAndUpdateTolerances (#L35955), which can grow edge tolerances even in check-only mode (#L33135-L33163).
- Geometric agreement is checked by sampling. Gap functions are "approximated by an array of SmGapSamples". B/source/SMLib/inc/SmGap.h#L6-L20
- Reports are structured: owner, other, test index, tolerance, measured value, parameters, distance, file and line. B/source/SMLib/inc/SmAssertArray.h#L229-L280
- A separate USD-level validator (tools/brep_validator) has rules BA_000–BA_764 and about 100 one-defect fixtures under TestFiles/brep_validator.

5. HEALING AND SEWING
- HealBrep is a staged cache/fix pipeline. B/source/SMLib/inc/SmBrep.h#L1086-L1130, B/source/SMLib/inc/SmHealProbArray.h#L76-L120
- Order: measure → Fix_TolSizes → coincident vertices → degenerate faces/edges → seam moves and splits at seams → bad sheets → loops → UV trims → infinite region.
- Several stages are stubs or disabled: CoinEdges, MissedEdgeXSects, Fix_BadGaps (a no-op), BackPointers and BadLoops. B/.agents/skills/smlib-kernel-healing/references/architecture.md#L77-L156
- SmBrep::HealBrep discards the nested status and returns SM_SUCCESS. B/source/SMLib/src/SmBrep.cpp#L20700-L20801
- Stitching is configured through a SmStitch object with a survivor-choice callback and flags: manifold-only lamina pairs, split edges at vertices, squeeze short edges, remove slivers. B/source/SMLib/inc/SmStitch.h#L21-L95
- The tolerance escalates coarse-to-fine: StitchIntoShell starts at max/100 and doubles; StitchIntoSolid starts at Brep tolerance/10 and doubles up to bbox diagonal/200. B/source/SMLib/src/SmStitch.cpp#L2670-L2711, #L2750-L2791

6. SIZE: SmBrep.cpp alone is 38,863 lines. The SMLib folder has 466 files: 239 headers plus .cpp files, NLib included.

**Robustness exactness:** Arithmetic is IEEE double throughout.
- No exact or adaptive predicates and no interval arithmetic: a search of all 239 SMLib headers and the topology .cpp files for Shewchuk/orient2d/orient3d/interval/expansion arithmetic found nothing.
- Topological connectivity is exact by object identity; geometric connectivity is tolerance-based.
- Gaps (vertex-edge, vertex-face, edge-edge loop closure, edge-face, edge-trim, vertex-trim) are measured by sampling, not bounded. Examples:
  - The fast edge compare uses 5 points.
  - Edgeuse checks use 5 or 6 samples.
  - SmGap says the exact representation "would be a function".
- Degeneracy predicates are partly stubs (SmTol.h L700-L705).
- Validator gaps that NVIDIA's own reference lists (brep-format.md L903-L922):
  - SmEdge::AssertValid dereferences a null curve after reporting it.
  - The vertex-sector check is commented out.
  - SmLoop::IsEmbedded always returns true.
  - Test labels drift from the executed predicates.
  - Coverage depends on cache state, build macros and test level.
  - The deep validators are "not safe parsers" for corrupted graphs.
- Validation mutates tolerances and caches, so a "valid" result can be produced by inflating tolerances.
- ValidateAndUpdateTolerances leaves edge gaps larger than bbox/800 unfixed. Its comment inverts the meaning of the bValidateOnly flag.
- Healing failure modes:
  - Fix_BadGaps is a no-op.
  - Fix_TolSizes changes tolerances instead of geometry.
  - HealBrep always reports success.
- Stitching failure modes reported by solidcore's upstream-issues.md: HANG-24 (zero B-rep tolerance) and HANG-33 (dMaxStitchTol3d/100 underflow) never return; UB-178 and UB-197 are use-after-free in SmStitch; UB-191 is CombineCoincidentVertices deleting its survivor when inputs alias.
- Operation-level evidence from the usd-brep issue tracker (21 issues in 2 weeks):
  - #1: a union silently drops a mirrored operand and still reports manifold=True.
  - #2: a failed fillet with foreign edges corrupts another Brep.
  - #10: a shell gives different volumes, SM_ERR, or a non-manifold body across fresh processes.
  - #12: is_manifold_solid anti-correlates with correctness.
  - #9: level-2 AssertValid leaks memory.
- Error model: SmStatus integer codes, mostly generic SM_ERR 1001, plus SmAssertReport arrays. There are no typed error variants per invariant.

**Determinism:** Not deterministic, and the source makes this structural.

Verified sources of nondeterminism at c9c5979:
- `srand((unsigned)time(NULL))` plus `rand()` in SmShell::IsInnerShellOf (B/source/SMLib/src/SmShell.cpp#L885-L912) and SmFace::GetPointsInFace (B/source/SMLib/src/SmFace.cpp#L21183-L21225). This is face sampling and shell classification, i.e. part of topology validity and region nesting.
- std::unordered_map keyed by pointers in SmMapPtrToPtr (B/source/SMLib/inc/SmMapPtrToPtr.h#L42, #L115). Iteration order follows heap layout and ASLR, and leaks into results; solidcore ND-2 observed it in offset.

Further sources:
- `thread_local` tolerance defaults (SmTol.cpp L70-L73).
- Unlocked mutable mark counters in the context. solidcore WR-2 reports that release builds do not enforce mark locks.
- A function-local static in the fillet solver (solidcore ND-3).
- Validation that rewrites tolerances, so results depend on the order of query and validate calls.

Concurrency: NVIDIA's docs say the concurrency unit is the process. Do not call the API from multiple threads; shared-memory WASM workers count as threads. B/.agents/operations/concurrency.md#L10-L30

Issue #10 shows the same input giving different results across fresh processes.

solidcore claims determinism by using a seeded kernel RNG, ordered maps and insertion-order arenas. I did not verify this, and it then intentionally differs from the C++ on these paths.

**Tests and quality:** What exists:
- C++ test executables under tests/SM_API_test (15 files; TestSmBrep.cpp is 115 KB) and tests/prog_test.
- Python tests: source/SmPyLib/tests/test_omni_solid.py (303 KB).
- tools/brep_validator_test with generate_test_files.py, and about 100 single-defect USD fixtures under TestFiles/brep_validator (for example Test_BA_580 faceuse pairing violation, BA_581 broken radial edgeuse chain, BA_582 orphan edge, BA_763 UV trims not closing a loop).
- A kernel-backed tools/brep_geometry_validator.

What I found no evidence of: fuzzing, property tests, or sanitizer CI (unverified).

Code quality:
- Very large files: SmBrep.cpp 38,863 lines, SmFace.cpp about 21k lines.
- Two tolerance models chosen by macro, with SM_NEWTOL_LINE/SM_OLDTOL_LINE markers on individual lines.
- Lots of commented-out code, "obsolete" blocks and TODO/STUB stages.
- The documentation is unusually good: .agents/ references written for AI agents, with honest lists of validator gaps and incomplete healer stages.

External audit: solidcore's plan/upstream-issues.md (Apache-2.0, third-party, many entries unconfirmed against C++ runs) catalogues 180 UB, 35 hang, 3 nondeterminism and 121 wrong-result entries across the whole kernel, 26 flagged PSIRT. Many sit in this layer: topology, healer target management, stitching, notify and tracking (for example WR-16 merge notify uses the deleted object; WR-17 StopTracking clears a foreign callback). https://github.com/tangency-labs/solidcore/blob/0072058ad2392fce619871d14315e93637e1cf3f/plan/upstream-issues.md

solidcore's difftest harness (to 1e-9 relative, with identical topology) reproduces upstream behaviour, including bugs ported "as is". By its README, topology is still "under way" and nothing works end to end.

**Legal:** Root licence and headers:
- Root LICENSE is Apache-2.0.
- Every SMLib and SM_API file I checked carries `SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES` and `SPDX-License-Identifier: Apache-2.0`. The README states "Copyright (c) 2020-2026".
- There is no root NOTICE file (raw fetch returned 404). Porting obligations are therefore the licence text, the copyright and SPDX headers, and a statement of changes, under Apache-2.0 §4.
- Apache-2.0 §3 gives a patent grant from NVIDIA as contributor.

Exceptions to Apache-2.0, per THIRD_PARTY_NOTICES.md:
- repo.sh/repo.bat/repoman bootstrapper are MIT.
- OpenUSD is under the Tomorrow Open Source Technology License 1.0.
- oneTBB is Apache, hwloc BSD, MaterialX Apache, OpenSubdiv modified Apache, pybind11 BSD.
- The optional GUI tool depends on PySide6/shiboken6 (LGPL-3 or GPL). Never use it; method study only, and it is irrelevant to this subsystem.
- proposals/UsdSolid and proposals/cad_geometry are unmodified AOUSD Geometry WG snapshots under the "OpenUSD Technical Proposal Supplemental Terms", not Apache. Their tolerance and validity rules may inform design, but do not copy their text.

Provenance:
- SMLib was a closed commercial kernel by Solid Modeling Solutions (SMLib/NLib, sold since the 1990s).
- NVIDIA acquired SMS in May 2022. NVIDIA's SMLib docs say licences are limited to legacy customers.
- A leftover comment still says "Contact SMS support" (B/source/SMLib/inc/SmAttribute.h#L459).
- NVIDIA re-licensed the code as copyright holder. Whether any SMS code carried third-party (for example NLib-origin or academic) copyrights is unverified. Grep found no non-NVIDIA copyright lines in the headers.

Patents:
- I found no Solid Modeling Solutions patents (search inconclusive; unverified).
- The radial-edge structure (Weiler, 1980s) and tolerant modelling (Jackson/Parasolid, 1995) are old enough that any patents would have expired (unverified).

solidcore:
- Apache-2.0, a derivative work with a NOTICE carrying NVIDIA's notice. A port from it inherits both notices.

What the owner must check before taking anything:
- Keep Apache headers and attribution in MPL-2.0 files, or keep ported files Apache-licensed alongside.
- Do not copy proposal text.
- Confirm "except where otherwise noted" exceptions file by file before any code-level port.

**Alignment:** Measured against each Spare Kit principle:
- **Iron-clad:** NO.
  - Validity is checked by sampling, has documented gaps, and can crash on damaged graphs.
  - Validation "passes" by inflating tolerances.
  - HealBrep masks failure with SM_SUCCESS.
  - Booleans can silently drop operands (#1).
  - A failed operation can corrupt its inputs (#2).
- **Exact/robust predicates:** NONE. Pure double arithmetic with ad hoc epsilons.
- **Explicit, structured errors:** PARTIAL. SmAssertReport is a good record shape, but statuses are generic integers.
- **Deterministic (native, WASM, any thread count):** NO. Clock-seeded rand, pointer-hash order, thread-local and global mutable state, single thread per process.
- **Persistent naming:** NO. Pointer identity with documented pointer reuse; solidcore's arena index is also not a persistent name. The notify vocabulary (split/merge/trim/coincident/copy) is the only reusable idea.
- **Immutable data, pure operations:** NO. In-place mutation everywhere, `mutable` caches and gap fields inside entities, marks stored in entities, and const validators that write.
- **No unsafe:** the C++ is raw-pointer based. solidcore achieves safe Rust only by keeping the same mutable graph in arenas.
- **Full scale:** WEAK. Five model-size bins (1e-7..1e-3), a "pinhole in battleship" flag and ratio, and no explicit units. Site-scale BIM coordinates (10⁵–10⁶ mm) would fall into the 1e-3/1e-4 bins.
- **Readable by AIs and people:** the docs are excellent; the code is not (38k-line files, two macro-selected tolerance models).
- **Headless:** yes.

What would have to change for any of it to fit:
- Replace the pointer/use graph with immutable, index- or ID-based records, keeping one record per logical face-side incidence and deriving mates and radial order.
- Give each body one explicit, typed conformance tolerance with certified (not sampled) gap bounds.
- Make validation a pure function returning structured errors.
- Make healing and stitching pure functions that return a new body plus a provenance and repair report.
- Implement radial ordering with robust comparisons.
- Remove every global, thread-local, RNG and pointer-ordered container.

Once all that changes, nothing of the code remains, only concepts. That is why the verdict is method study, not a port.

**Strengths**

- Real non-manifold expressiveness. Regions with void flags and an explicit infinite region; shells of faceuses, wires or a single vertex; spine (3+ face) edges, wire edges, vertex loops and explicit seams. Cellular region models suit BIM well (spaces and rooms as regions sharing wall faces).
- A crisp, honest invariant catalogue for a radial-edge model, with model / geometric / constructor / validator distinctions and a list of known validator gaps (B/.agents/skills/smlib-brep-model/references/brep-format.md).
- Clear separation of tolerance roles (zone, intersection = zone1 + zone2, approximation = zone/2) as strong types, plus a complete taxonomy of connected-entity gaps with an up/down/same-dimension table.
- A structured validation report record: owner, other, test id, tolerance, measured value, parameters, distance, source location.
- A provenance event vocabulary (split, merge, trim-no-split, coincident-across-operands, change-geometry, copy) with attribute Split/Merge/Copy hooks. This is directly relevant input to designing persistent naming.
- A compact flattened encoding (SmBrepData) that collapses each mate pair into one record and uses a signed radial index, as a model for a lean serialized or immutable form.
- Practical healing stage order and stitch heuristics: coarse-to-fine tolerance doubling, a lamina-pair-only manifold mode, explicit survivor policy, and reporting of the largest bridged and smallest unstitched gaps.
- About 100 one-defect negative fixtures for a B-rep exchange format (Apache-2.0), plus a clear USD validation-boundary statement.
- Apache-2.0 and headless; years of commercial use (powers ClassCAD).

**Weaknesses**

- Mutable pointer graph with intrusive lists and per-object mark counters; identity is a pointer, and pointers are reused; no persistent naming.
- Heavy redundancy: 2 faceuses per face, 2 loopuses per loop, 4 edgeuses per manifold edge, each with roughly 100 bytes of bookkeeping (my estimate) plus mutable gap caches. Many invariants exist only to keep the duplicates consistent.
- The tolerance model is mid-migration (OLDTOL compiled in, NEWTOL incomplete, called a 'confused mix' in the source). Per-entity tolerances grow to swallow gaps. Defaults are thread_local statics chosen by model-size bins. Degeneracy predicates are stubs.
- Validation is sampled, level- and macro-gated, has documented holes, can crash on damaged input, and mutates tolerances and caches despite being const. It also leaks memory at level 2 (issue #9).
- The healer has stub or disabled stages (Fix_BadGaps no-op, CoinEdges, MissedEdgeXSects, BackPointers, BadLoops), repairs by inflating tolerances, and SmBrep::HealBrep always returns SM_SUCCESS.
- Stitching can hang on zero or underflowing tolerances and has reported use-after-free bugs (solidcore HANG-24/33, UB-178/197).
- Nondeterministic: clock-seeded rand in shell and face classification, pointer-keyed unordered_map iteration, global mutable context, one thread per process; cross-process result variation reported in issue #10.
- No exact or robust predicates anywhere; generic SM_ERR status codes; failed operations can corrupt inputs (issue #2) or silently drop operands (issue #1).
- solidcore inherits all of this architecture (arena ids but the same mutable radial-edge graph and per-entity tolerances, upstream bugs ported 'as is'), and its topology port is unfinished.

**Verdict:** STUDY

**Verdict reason:** The subsystem's architecture is the opposite of ours on every core principle: mutable pointer identity, tolerant per-entity tolerances that grow to swallow gaps, sampled and side-effecting validation, nondeterministic classification, and healing that mutates in place and masks failure. Porting code, or translating it closely as solidcore does, would import exactly what G01 forbids.

There is no exact algorithm here to re-implement either: the validators, gap measures, healer and stitcher are float heuristics. So PORT_METHODS would overstate it, and it cannot serve as a difftest oracle for validity, which is sampled, mutating, gap-ridden and nondeterministic.

What is valuable is conceptual and well documented:
- the non-manifold region and shell semantics and the invariant catalogue;
- the tolerance roles and gap taxonomy;
- the structured validation report shape;
- the split/merge/coincident provenance vocabulary for naming;
- the compact mate-pair encoding;
- healing stage order and coarse-to-fine stitch escalation;
- the negative-fixture corpus pattern.

These should be studied and re-derived in our own immutable, ID-based, exact design. That makes the verdict STUDY, not a port.

**What to take**

- **Invariant catalogue for a non-manifold model** (design input for our validity spec; re-derive, do not copy): region rules 1–9, shell-type exclusivity, edge forms (wire, lamina, manifold, spine, strut, embedded), seam and vertex-loop rules, and 'topology exact by identity, geometry tolerance-aware'. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/skills/smlib-brep-model/references/brep-format.md#L79-L345 and #L617-L688. Known-gap list to design against: #L903-L922.
- **Region, void flag and infinite-region semantics** for cellular, BIM-space bodies: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmRegion.h#L28-L41 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmBrep.h#L66-L80
- **Radial sector ordering** by face binormal, with a step-off tie-break for tangent faces. Re-implement with robust or exact angular comparison and higher-order tie-breaks: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmEdgeuse.h#L195-L230 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmEdge.h#L118-L123
- **Compact encoding**: one record per logical face-edge occurrence, with a signed radial index selecting the mate side. Use it as a model for our immutable or serialized fin/coedge tables: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmBrepData.h#L655-L700
- **Gap taxonomy** (vertex-edge, vertex-face, edge-edge loop closure, edge-face, edge-trim, vertex-trim) and the up/down/same-dimension table, as the list of conformance relations our checker must bound with certified, not sampled, methods: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmBrep.h#L645-L664 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmGap.h#L6-L20
- **Typed tolerance roles** (zone / intersection = sum of zones / approximation = half a zone) as distinct Rust newtypes. Do not take the values, the thread-local defaults or the gap-growth rule: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmTol.h#L104-L180 and #L264-L512
- **Per-body tolerance contract, idea only** (AOUSD terms, do not copy text): one tolerance per body; connected entities have gap < tolerance; unconnected entities have gap > tolerance; no slivers or sub-tolerance² faces; every self-intersection marked by topology. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/proposals/UsdSolid/README.md#L376-L448
- **Structured validation-report record**: owner, other, rule id, tolerance used, measured value, parameters, distance. Map it onto our explicit error enums: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmAssertArray.h#L229-L280
- **Provenance vocabulary for persistent naming**: ADD, RM, SPLIT(parent→children), MERGE(survivor, consumed), TRIM_NO_SPLIT, COINCIDENT(A, B), CHANGE_GEOMETRY, COPY. Our pure operations should emit these as data keyed by persistent names, never pointers. The pointer-reuse hazard is an anti-pattern to avoid. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmTypes.h#L468-L620 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmTrackTopologyChanges.h#L33-L80
- **Healing stage order and per-problem attempt states** (NONE/RAN/FIXED/NOFIX), redesigned as a pure function that returns a new body plus a repair report. Do not count tolerance inflation as a repair. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmHealProbArray.h#L76-L120 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/skills/smlib-kernel-healing/references/architecture.md#L77-L156
- **Stitching heuristics**: coarse-to-fine tolerance doubling from max/100; a manifold lamina-pair-only mode; an explicit survivor policy; reporting of max bridged and min unstitched gaps. Add guards for zero or underflowing tolerance. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmStitch.cpp#L2670-L2711, #L2750-L2791 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmStitch.h#L21-L95
- **One-defect-per-file negative fixture pattern**, and these Apache-2.0 fixtures for a future USD importer test: https://github.com/NVIDIA-Omniverse/usd-brep/tree/c9c5979f04cea8c38213004d4f04df0dcfb40d05/TestFiles/brep_validator and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/tools/brep_validator_test/generate_test_files.py
- **Hazard checklist** of what our kernel must never do, from solidcore's Apache-2.0 upstream-issues log: ND-1..3, the WR topology/notify entries, and the stitch and healer UB/HANG entries. https://github.com/tangency-labs/solidcore/blob/0072058ad2392fce619871d14315e93637e1cf3f/plan/upstream-issues.md

### Challenge 1

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- Tolerance roles are not strong types in the shipped build, contrary to the strengths list and what_to_take. SM_USE_NEWTOL_STRONG_TYPES is commented out (B/source/SMLib/inc/SmConfig.h#L255-L256). SmTol3d converts to and from double automatically (B/source/SMLib/inc/SmTol.h#L139-L162). SM_TOL_CALLS also gives every role type an implicit operator double& whatever the macro (SmTol.h#L167-L173). Zone, XSect and Approx can therefore be mixed silently through double. The role split is naming only and is not enforced by the compiler. (B = https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05)
- The claim of 'two tolerance models chosen by macro, with SM_NEWTOL_LINE/SM_OLDTOL_LINE markers on individual lines' is wrong. Both markers are defined empty (SmConfig.h#L232-L233), and the selecting SM_SLASH form was removed for Linux (#L236-L249). Every NEWTOL and OLDTOL line is therefore compiled together: SmBrep's NEWTOL m_sThisZoneTol3d (SmBrep.h#L240-L243) sits beside the per-entity OLDTOL m_sZoneTol3d (SmFace.h#L123, SmEdge.h#L94, SmVertex.h#L82). Only #ifdef SM_USE_NEWTOL blocks select anything. The real mix is worse than the evaluation says.
- 'A shell holds exactly one kind of use' is contradicted by the source. A faceuse shell may also carry wire edges: SmShell.h#L57-L61 says 'FaceShells with or without attached WireEdges', and brep-format.md#L209-L221 says 'A faceuse shell with embedded wire edges is legal'.
- Wrong citation for the const-cast. B/source/SMLib/src/SmBrep.cpp#L35955 is inside SmBrep::Dump (which starts at L35638), not AssertValid. AssertValid's const-cast call to ValidateAndUpdateTolerances is at SmBrep.cpp#L34814. Also, the header does not say AssertValid 'never changes the Brep': it says it never changes it 'except to refresh cached data' (#L34758-L34759).
- Evidence misused: the 'CAN CHANGE DATABASE!!!' comment is stale for the path that runs. ValidatePointers calls pE->ValidateGeometry(FALSE) (SmBrep.cpp#L31510-L31511). Every vertex-tolerance write and UV re-parameterisation in SmEdge::ValidateGeometry is gated on bFixTolerances (SmEdge.cpp#L6186, #L6211, #L6261, #L6270), so this path does not mutate. The claim that validation mutates state still stands, but only through ValidateAndUpdateTolerances' cache side effect (SmBrep.cpp#L33155-L33158) and through mark counters.
- File counts are off. At c9c5979, source/SMLib has 463 files, not 466: 238 headers in inc/, not 239, plus 224 .cpp files and premake5.lua. Checked with git tree recursive at c9c5979.
- PSIRT count is overstated. 26 is the raw number of lines in tangency-labs/solidcore@0072058 plan/upstream-issues.md containing '[PSIRT]'. That count includes the header (L9) and five explicit 'not [PSIRT]' notes (L1431, L1441, L1600, L1638, L1647). Only 17 entries are flagged, all in their headings. The other counts are correct: UB 180, HANG 35, ND 3, WR 121.
- The stitching failure modes are quoted without upstream-issues.md's own caveats. HANG-24, UB-178 and UB-197 are 'Confirmed: no' (source inspection). UB-178 and UB-197 are also marked 'Not file-reachable'. UB-191 has 'no native reproduction', and the entry says the call itself is defined. Only HANG-33 was confirmed by a C++ run. Reading SmStitch.cpp#L2680-L2711 and #L2769-L2791, I independently confirm that HANG-24 and HANG-33 can occur: a start tolerance of 0 stays 0 under doubling.
- Issue-tracker numbers. #1–#21 include two pull requests (#17 and #18), so there are 19 issues, opened 12–20 Oct (9 days). Issue #9 (AssertValid level-2 leak) is closed as completed (2026-10-14), but nothing was pushed after c9c5979, so the pinned code is unchanged. Whether the leak is fixed elsewhere is unverified.
- Concurrency is overstated as 'one thread per process'. .agents/operations/concurrency.md#L12-L14 documents exceptions: SmApiUsdTessellateBrepArray and SmApiUsdHealFile run on worker threads, each in a private context.
- Unverified attributions. (a) 'NVIDIA's SMLib manual calls it the radial edge structure': the phrase does not appear in the repo's .agents docs, and NVIDIA's SMLib docs index does not show it. The only in-repo use is AOUSD text, proposals/UsdSolid/README.md L6 and L61 ('Radial Edge Data Model'). (b) 'NVIDIA's SMLib docs say licences are limited to legacy customers': the docs index only says 'legacy support content' and has a link for products no longer supported.
- Missed provenance point. SMLib grew out of a 1998 three-party joint-marketing agreement that combined IntegrityWare and GeomWare technology. IntegrityWare states that 'all three companies have the rights to market and sell this new kernel' and still sells SOLIDS++ built on it (integrityware.com/products/SOLIDS++/history.html). NLib was bought from GeomWare in 2001 (Wikipedia: Solid Modeling Solutions). All 463 SMLib files carry only 'Copyright (c) 2026 NVIDIA' and Apache-2.0 SPDX headers, with no IntegrityWare or GeomWare credit (my grep found no other copyright line). For a code-level port the chain of title is an open question (unverified). It does not affect method study.
- Missed caveats on the provenance vocabulary, which the evaluation recommends as input to persistent naming. (1) Notify coverage is incomplete by the source's own admission: split and merge notifications are made only on Regions, Faces, Edges and Vertices, not on uses, curves or surfaces (SmAttribute.h#L453-L461). (2) SPLIT reuses the parent object as child 1 (SmTypes.h#L524; SmTopology.cpp#L215-L218), so the parent's identity silently passes to one child. (3) The merge roles contradict each other: the documented table says pData1 is the survivor and pData2 the deleted object (SmTypes.h#L531-L532), but the consumer treats pData2 as the survivor, marked 'GWC:TODO try typo bug fix' (SmTopology.cpp#L220-L224; solidcore WR-16). (4) SM_NO_REG_PROPAGATION (SmTypes.h#L622) is missing from the evaluation's list, yet region provenance through booleans is the most BIM-relevant event.
- Missed release-build behaviour. SM_DEBUG_CODE is defined only under _DEBUG (SmConfig.h#L125-L131). In release, every in-operation SM_ASSERT_VALID self-check compiles to nothing (SmAssertArray.h#L127-L136). SM_VALIDATE_TOPOLOGY is also off (SmConfig.h#L134-L135), so SmStitch's m_bValidateResult has no effect. The file and line fields of SmAssertReport exist only under SM_DEBUG_CODE (SmAssertArray.h#L254-L257). Shipped operations do not validate their own results.
- Missed ways stitching masks failure. StitchIntoShell returns SM_SUCCESS whether or not lamina edges remain (SmStitch.cpp#L2793). When bShellIsWellFormed is FALSE it sets m_bIgnoreProblems, so failed edge glues are skipped silently (#L2762-L2766; SmStitch.h#L74-L76). The comment says 'down to 1/10000' while the code uses dDist/200 (#L2684 vs #L2688).
- Nondeterminism is understated. It is not confined to SmMapPtrToPtr: public topology queries return pointer-keyed std::unordered_set (SmFace.h#L672-L673, SmLoopuse.h#L85-L86, SmEdge.h#L299). srand(time) also resets the process-wide C rand() state (solidcore ND-1).
- Minor inaccuracies. (1) The model-size bins are not limited to 1e-7..1e-3: outside the table, SizeEstimateToZoneTol3d applies 10^floor(log10(size)/2)·1e-5 (SmTol.h#L574). (2) Site-scale coordinates of 1e5–1e6 mm fall only in the 1e-3 bin. (3) The stub list leaves out AreCoincident and AreIntersecting (SmTol.h#L707-L711). (4) SmBrep.h#L66-L80, cited for the infinite region, is the SmSetRegionVoidType enum; the explicit infinite-region pointer is m_pInfiniteRegion (SmBrep.h#L246). (5) There are about 107 Test_BA_* fixtures among 130 .usda files, and rule codes run to BA_765 (retired).

**Argument:** The STUDY verdict holds. I re-read the pinned C++ at c9c5979: all 463 SMLib files, plus the .agents references, THIRD_PARTY_NOTICES, the issue tracker, and solidcore@0072058 (README, NOTICE, topology.rs, upstream-issues.md).

**Facts that check out:**
- The radial-edge use graph: 11 classes, faceuse and loopuse pairs, edgeuses in mate/radial order, and the signed mate index in SmBrepData.
- Tolerance handling:
  - SM_USE_OLDTOL is the compiled model, and the source calls the tolerance code a "confused mix".
  - Five model-size bins and the gap-growth formula.
  - The constants, the thread_local defaults and the NOT IMPLEMENTED stubs.
- Validation:
  - The 147 ValidatePointers labels and the 1000x tolerance band.
  - The inverted bValidateOnly comment and the bbox/800 limit.
  - Sampled gap functions.
- Healing and stitching:
  - HealBrep discards the healer's status and returns SM_SUCCESS.
  - Healer stages that are TODO or disabled.
  - The stitch tolerance-doubling loops.
- Nondeterminism: srand(time) in shell and face sampling, and pointer-keyed unordered_map.
- Licensing and provenance:
  - Apache-2.0 with SPDX headers on every file, and no root NOTICE.
  - The PySide/LGPL, OpenUSD (TOST) and AOUSD-terms exceptions.
  - The NVIDIA acquisition in May 2022.
- solidcore: an append-only typed-u32 arena that keeps the same mutable graph, and the README status.
- Issues #1, #2, #10 and #12 say what the evaluation says.

**What the evaluation got wrong:**
- The tolerance "strong types" are not strong: they convert to and from double implicitly.
- The NEWTOL/OLDTOL line markers are empty macros, so both sets of fields are compiled.
- Shells are not limited to one kind of use: faceuse shells may carry wires.
- The AssertValid citation points to the wrong line (L35955 is in Dump; the real call is L34814).
- The "CAN CHANGE DATABASE" evidence is stale for the call that actually runs.
- The PSIRT count is 17 entries, not 26.
- Several solidcore UB/HANG entries are unconfirmed by solidcore's own record.
- Small count errors.
- None of these changes the conclusion. Most make it stronger.

**The case for a softer verdict.** I looked for anything best-in-class:
- A production non-manifold radial-edge model with explicit regions, void flags, an infinite region, seams, vertex loops and struts. Region provenance survives booleans via SM_NO_REG_PROPAGATION.
- It is rare among permissively licensed kernels and fits BIM cellular spaces, such as rooms sharing wall faces.
- The .agents brep-format.md is an unusually honest, source-cited invariant contract.
- About 107 one-defect USD fixtures and an 8.3k-line Python BA-rule validator that does not depend on the kernel are concrete Apache-2.0 test assets.
- SmBrepData's mate-pair encoding is a compact layout worth copying as an idea.

None of this lifts the verdict to PORT_METHODS. Every algorithm in this layer depends on float tolerances and sampling and has to be redesigned for exact predicates and an immutable, deterministic design:
- radial sector ordering by binormal with step-off;
- gap measurement;
- healing stages;
- stitch tolerance escalation.

DIFFTEST_ORACLE fails because validation mutates caches and tolerances, is gated by build macros and test level, and has documented holes, and because classification is clock-seeded. PORT_CODE fails on architecture, and there is also an unverified chain-of-title question from the 1998 IntegrityWare/GeomWare origin.

**The case for a harsher verdict.** I found more against it than the evaluation did:
- Shipped release builds compile out every in-operation validity self-check, and SM_VALIDATE_TOPOLOGY is off.
- StitchIntoShell reports success even with lamina edges left, and can silently skip failed glues.
- Public topology queries return pointer-hashed sets.
- The provenance events have incomplete coverage, reuse the parent as the first child on split, and give contradictory merge roles.

Even so, REJECT would throw away real design input: the invariant catalogue, the region semantics, the event vocabulary as a concept, and the fixture corpus. STUDY is the right label, provided the owner treats the tolerance-role and provenance items as ideas to re-derive, not as working mechanisms in SMLib.

## nurbs-core

**Commit sha:** usd-brep c9c5979f04cea8c38213004d4f04df0dcfb40d05 (Release 1.0.0, 2026-10-09). The repo has only three commits: 1a55464 initial, ca6ce70 0.2.0, c9c5979 1.0.0. solidcore main 6a019c57131d5fee6e57ab7dc4a62201a32e0216 (2026-10-20).

**Findings:** WHAT IT IS
- usd-brep is Apache-2.0 and "not accepting contributions".
- The NURBS core is NLib: 40 NL_*.cpp files, 253,923 lines including docs. They define 1,323 N_/M_ functions; 330 of these are compiled out by `#if NLIB_UNUSED`, leaving 993 live.
- The C++ object layer is 615k lines of Sm*.cpp. Examples: SmCurve.cpp 27.7k lines, SmSurface.cpp 35.4k, and the bridge files SmBSplineCurveNLib.cpp 4.9k and SmBSplineSurfaceNLib.cpp 9.0k.

WHERE NLIB COMES FROM
- NLib is Piegl and Tiller's own commercial library. It came from GeomWare, which sold it to Solid Modeling Solutions (SMS) in late 2001. Tiller was an SMS partner, and NVIDIA bought SMS in May 2022. These facts come from Wikipedia, a secondary source.
- The source still shows it: the `GW_EXPORT` macro (e.g. inc/NL_Math.h:176) and "GWC:" comments.
- The functions are close C versions of The NURBS Book pseudocode, using the book's variable names:
  - span search and basis functions (A2.1–A2.3): N_BasisFindSpan/Eval/Derivs, https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_BasisBasic.cpp#L1388-L1549 and #L2798-L3164
  - knot insertion (A5.1): N_CrvInsertKnot, https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvTool.cpp#L20-L202
  - knot refinement (A5.4): N_CrvRefine, NL_CrvTool.cpp#L1326-L1511
  - knot removal (A5.8): N_CrvRemoveKnot, #L1707-L2012
  - degree elevation (A5.9): #L3235-L3578
  - degree reduction (A5.11): #L3582-L4236
  - rational derivatives (A4.2): N_CrvDerivs, NL_CrvNurb.cpp#L913-L1205
  - global interpolation (A9.1, banded LU without pivoting): N_FitCrvInterp, https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvFit.cpp#L2049-L2212
  - surface of revolution (A8.1): N_CreateRevolvedSrf, NL_SrfCommon.cpp#L1347-L1535

DATA MODEL
- A curve stores homogeneous 4D control points plus a knot vector `{m, U*}`.
- 2D and non-rational points are marked inside the coordinates themselves: z = +DBL_MAX means 2D, w = -DBL_MAX means non-rational (inc/NL_Geometry.h#L46-L49).
- Memory comes from manual NL_STACKS arenas. Each function returns an NL_FLAG and records errors in a global NL_ERROR (src/NL_Globals.cpp#L1-L124).
- Fixed limits: degree at most 17 (NL_MAXDEG, used to size stack arrays), at most 4 derivatives, weights must lie in [1e-3, 100], at most 50 iterations.

INVENTORY BY TOPIC
1. Curve and surface types.
   - Analytic curves: SmLine, SmEllipse and SmCircle, SmParabola and SmHyperbola all inherit from SmBSplineCurve.
   - Analytic surfaces: SmPlane, SmSurfOfExtrusion, SmSurfOfRevolution (then SmCone, SmCylinder), SmSphere and SmTorus all inherit from SmBSplineSurface.
   - So an analytic shape is a NURBS plus a separate "STEP" parameterisation, with angles in degrees and a nonlinear map between the two (inc/SmSphere.h#L15-L91).
   - Procedural types: SmOffsetCurve, SmOffsetSurface (exact offset computed by an evaluator), SmIsoCurve, SmCrvOnSurf (a uv curve on a surface), SmProjectedCurve (parallel projection only), SmCompositeCurve, SmHermiteCurve, SmBlendSurface, and SmCurveBoundedSurface (used only for IGES). There are also B-spline volumes and deformation volumes.
2. Evaluation. Curve and surface basis derivatives, rational derivatives, a pole-normal routine (N_SrfEvalPtNormalAtPole), fast evaluators, and power-basis conversion.
3. Knot operations.
   - Insert, refine, remove one knot, remove all removable knots, and make several curves compatible within a tolerance (N_CrvsMakeCompatibleKnotTol, NL_CrvTool.cpp#L4245-L4419). Surface versions exist in NL_SrfTool.cpp.
   - Knot removal improves on the book's error bound in two ways. It multiplies the error by the maximum of the affected basis function (N_BasisFindGlobalMax, NL_BasisBasic.cpp#L1717-L1892). For rational curves it scales the tolerance by wmin/(1+|P|max).
4. Degree. Elevation by Bezier decomposition. Reduction one step at a time or repeatedly, with an error bound (also for Bezier pieces and surfaces). Surface reduction to a tolerance: N_SrfReduceDegreeToTol.
5. Interpolation, approximation and fitting.
   - Curves have 45 live routines: end derivatives, tangents, high-order end derivatives, closed curves, weighted and constrained least squares (A9.6, NL_CrvFit.cpp#L13246-L13964), and approximation to a stated error (N_FitCrvApprox, #L4422-L4816: interpolate, remove knots, least-squares at a higher degree, re-project the parameters, repeat). That routine is called from SmBSplineCurveNLib.cpp:469.
   - Surfaces have 23 live routines: global interpolation, least squares, approximation to a stated error (N_FitSrfApproxTol, NL_SrfFit.cpp#L6481-L6920), random points with boundary curves, points with normals, and least squares with a stiffness term (NL_SrfLstSqFit.cpp, N_AddStiffnessToSrfAMatrix from L38).
   - Compiled out: biarc, conic, parabolic-arc, sphere and plane fits, and curve and surface shaping and warping.
6. Projection and inversion.
   - NLib itself only does local Newton iteration.
   - N_CrvClosestPt (NL_CrvGeom.cpp#L20-L219) keeps the best result so far, wraps closed curves, and clamps open ones.
   - N_GetClosestPtOnSrf (NL_SrfGeom.cpp#L96-L576) runs Newton on normalised derivatives. Its comments credit this change ("works much better") to bug report [B146].
   - The global search is in the C++ layer. SmGlobalSolver splits the curve into a tree of Bezier spans with bounding boxes, tight oriented "pseudo-boxes" and tangent-cone "polar boxes". It discards a branch when the turning-angle test (under 90°) rules out a perpendicular foot. Then it solves locally (SmCurve.cpp#L10841-L11603). Surfaces use SmSurface::GlobalPointSolve (SmSurface.cpp L19980) and SmSurfacePointSolve.cpp.
7. Offsets.
   - NLib offsets curves and surfaces by sampling, interpolating and removing knots (the Piegl–Tiller 1999 method): NL_CrvOffset.cpp#L1154-L1562 and NL_SrfOffset.cpp#L612-L1290.
   - Planes, spheres, tori, cylinders and cones are offset exactly. Surfaces of revolution and extrusions are offset through their generator curve.
   - Loops, cusps and self-intersections are explicitly not checked.
   - The C++ layer also has an exact procedural SmOffsetSurface.
8. Lofting and skinning. N_CreateSkinSrf (NL_SrfAdv.cpp#L7835-L8278), plus variants that take parameters, approximate, or approximate to a tolerance (#L9572-L10054). A 2,000-line version matches G1/G2 boundary continuity (#L13148-L15231). There is also a spine routine.
9. Sweeps.
   - Translational sweep (#L11109-L11226) and swung surface (#L7701-L7829) are exact.
   - The general sweep N_CreateSweepScale (#L15237-L16375) places scaled section copies along a projection-normal frame and skins through them. Its documentation only says the result "attempts" to meet tolerance E.
   - There is also a spiral sweep.
10. Coons, Gordon and N-sided patches.
    - Bilinear Coons (#L11999-L12283). If any boundary is rational, all four boundaries are reparameterised in place.
    - Gordon, non-rational only, computed as loft + loft − tensor product (#L11702-L11994).
    - N_FillNSidedHole (#L18506-L19775): n patches, approximately G1 to an angular tolerance, meeting at a centre point and normal.
11. Revolution and circles. Quadratic circles, or quintic circles that need no interior knot, from a family of rational circle forms (quadratic, quartic, quintic arcs in NL_CrvCommon.cpp L14-L29, L251-L266, L519-L534, L741-L768, L883-L962).
12. Symbolic operations. Products and sums of B-spline functions and surfaces (NL_FuncsAdv.cpp#L1667-L2182 and #L6454-L7075), used for exact derivative and normal surfaces (NL_SrfSymbol.cpp).
13. Trimming.
    - NLib tessellates trimmed surfaces (N_TessTrimmedSrf).
    - SmTrimmingTools intersects iso-curves with trim loops. SmCrvOnSurf represents curves on surfaces.
    - Area of faces trimmed by rational circles can be off by up to 1.4% at default settings (KNOWN_ISSUES.md#L53-L55).

SOLIDCORE (the Rust port)
- Progress: https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/plan/PROGRESS.md reports 234 of 261 units ported (88.1% of upstream lines). All 48 nurbs units are ported, 45 of them "green". The README's "about a third" (L29) is out of date.
- It is a faithful line-by-line translation:
  - Array access is bounds-checked, errors are SolidError values, and a Kernel struct replaces the global context.
  - It keeps NLib's choices: the NOZ/NOW sentinels (crates/solidcore/src/nurbs/math.rs), MAX_DEGREE 17, silent clamping in find_span (nurbs/knots.rs), and the analytic-plus-synced-NURBS model.
- It catalogues 337 upstream defects in https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/plan/upstream-issues.md. About 30 are undefined behaviour and 4 are wrong results in NLib (L148-L440, L2266-L2304), and at least 5 NLib hangs.

**Robustness exactness:** ARITHMETIC AND TOLERANCES
- Everything is IEEE double. There are no exact or adaptive predicates and no interval arithmetic.
- Tolerances are absolute and mostly global mutable variables: NL_MTOL 1e-7, NL_PTOL 1e-12, NL_LUDT 1e-9 (an absolute LU pivot threshold), NL_ZDTL 1e-8, NL_WTOL 1e-3. The C++ layer adds SM_EFF_ZERO 1e-12 and SM_EFF_ZERO_PARAM 1.2e-8 (inc/SmMath.h#L38-L45).
- SmTol is a two-scale tolerance model ("pinhole in battleship") that is half migrated. The release build uses SM_USE_OLDTOL.
- Unbounded intervals are marked by the sentinel SM_INFINITE_PARAMETER = 1234567.0.
- Limits: 50 Newton iterations, recursion depth 10 or 20. Basis-function divisions are guarded by 1e-24 and fail with NL_NUM_ERR. N_FloatOpIsBad checks divisions before they happen.

SILENT FALLBACKS (violate our "explicit errors" rule)
- N_BasisFindSpan clamps out-of-range parameters to the first or last span without saying so ("assigned silently"), and steps over zero-length end spans as a bad-data workaround: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_BasisBasic.cpp#L1465-L1546
- Gordon averages the two curves at each network crossing with no consistency check (NL_SrfAdv.cpp L11856). Coons averages its corner points the same way (L12079-L12113).
- Skinning returns a finished surface together with a weight-out-of-range error.
- Coons reparameterises its input curves in place, and N_FillNSidedHole overwrites inputs that are not compatible.
- The kernel quietly turns an unknown parameterisation option into uniform; the API layer had to add a check (tests/SM_API_test/src/TestSmCurves.cpp#L918-L921).
- The general sweep only "attempts" its tolerance. Offsets ignore loops and cusps.
- Newton returns its best point so far together with NL_CON_ERR.

DEFECTS CONFIRMED BY SOLIDCORE AGAINST THE C++ BUILD
- UB-23: heap corruption when summing rational curves.
- UB-27: write to M[2i+1][-1] in a fitting routine.
- UB-99: use-after-free in skinning with boundary continuity (found by ASan).
- UB-107: surface extension returns SM_ERR in a fresh process but SM_SUCCESS after other calls in the same process.
- UB-196: knot insertion writes past its buffer.

FOUND BY READING ONLY
- UB-26: N_CreateRevolvedSrf overflows its `wi[9]` array for a quartic revolution of 241° or more (NL_SrfCommon.cpp L1408).
- HANG-2: N_BasisFindGlobalMax can loop forever.
- WR-24: N_KnotVectorIsValid accepts degenerate knot vectors.
- DOC-1: the NL_LEFT/NL_RIGHT documentation is swapped.
- WR-160 to WR-165: SmOffsetSurface has wrong coefficients in its third derivatives and several related errors.
- My own reading, not run: M_TridiagonalSystem (NL_Math.cpp#L11712-L11758) does not match the Numerical Recipes tridiagonal algorithm it cites. The forward step uses Q[j-1] where it should use P[j-1], and the backward loop stops at j>1. It is only reached through compiled-out code.

ALGORITHM QUALITY
The methods themselves are sound: they are the book's methods plus the authors' later production refinements, such as the sharper knot-removal error bound, the error-bounded fitting loop, and quintic circles.

**Determinism:** NEITHER CODE BASE MEETS OUR "BIT-IDENTICAL NATIVE AND WASM" RULE.

USD-BREP: GLOBAL MUTABLE STATE
- The global error record NL_ERROR.
- Tolerance globals that can be reassigned.
- The nInitEnd counter in NL_Util.cpp.
- File-static state in the spiral code, set on every call: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Spiral.cpp#L22-L28 and #L131-L144.
- Leftover experiment switches `static int cbi` in NL_SrfAdv.cpp (L738-L739 and L11779).

USD-BREP: RESULTS THAT DEPEND ON HISTORY OR HEAP LAYOUT
- Uninitialised reads (UB-31, UB-32), and UB-107 confirmed (the extension case above).
- In the C++ layer: `srand(time(NULL))` (ND-1), iteration order of maps keyed by pointers (ND-2), and a function-static step size fixed on first use (ND-3).
- usd-brep issue #10: the same filleted box gives different shell results across fresh processes.
- The project's own concurrency guide makes the process the unit of concurrency; in-process threading is unsafe.
- Floating point comes from the platform's C maths library (libm). The upstream release build flags for contraction (/fp and -ffp-contract) were not checked (unverified).

SOLIDCORE
- It forbids global state, clocks and address hashing, and has a seeded RNG.
- But its own measurements (https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/plan/findings.md#L46-L99) show:
  - Rust `f64::sin/cos/atan2/powf` on wasm32 differ from glibc by 1 ulp on 2.4% (cos), 17.1% (atan2) and 6.2% (powf) of inputs.
  - Which `cbrt` gets linked depends on link order.
  - It is bit-identical only on Linux x86-64 glibc against a C++ reference built with -ffp-contract=off. Wasm results are never tested. The fix is still undecided.
- What this means for us: we need our own fixed, ideally correctly rounded, implementations of the transcendental functions, and no FMA contraction, whatever we take from either code base.

**Tests and quality:** USD-BREP TESTS
- No direct unit tests of N_* routines: tests/prog_test calls none of them.
- SM_API_test and about 400 Python tests (source/SmPyLib/tests/test_omni_solid.py) check status codes, input contracts and a few properties. Examples: interpolated curves pass through their points, knot removal keeps the shape, and the same input gives the same result.
- No golden numerical values, no property-based tests, no fuzzing.

USD-BREP CODE QUALITY
- NLib's per-function documentation is good (DESCRIPTION, a calling example, ACCESS and RETURN CODES).
- The code itself is macro-heavy C (EQ, GT, NL_OUT, NL_QUIT). It uses the book's single-letter names, lets output alias input for in-place operation, manages memory by hand, and leaves debug experiments in. It cites almost no literature: two mentions of "the NURBS book" and Numerical Recipes.
- The C++ layer has very large files (SmSurface.cpp is 35k lines), deep virtual hierarchies and two parameterisations per analytic type.

SOLIDCORE TESTS
- Its difftest compares Rust and C++ unit by unit, typically on about 1.5k–4k cases per unit (fixtures plus generated fuzz cases), within 1e-9 relative. Most nlib units are bit-identical.
- Caveat, issue #294: across all units about 13.9k cases are deferred and 1,476 "passes" compare nothing. These are mostly topology and analytic units, not nlib.
- The code is largely AI-generated. Each file starts with a header mapping its Rust items to the upstream functions they come from. Clippy denies unwrap, panics and unchecked indexing, and unsafe code is forbidden in the kernel.

**Legal:** USD-BREP
- Apache-2.0 LICENSE. All 462 SMLib files carry the header "SPDX: Copyright (c) 2026 NVIDIA … Apache-2.0".
- THIRD_PARTY_NOTICES lists no NLib, GeomWare or SMS component, so NVIDIA treats the code as its own.

CHAIN OF TITLE: unverified against primary documents.
- GeomWare (Tiller) sold NLib to SMS in late 2001; NVIDIA bought SMS in May 2022 (Wikipedia; DEVELOP3D 2026-10-13).
- HarmonyWare said in 2024 that the NLib/SMLib sale created "legal and logistical challenges" for a licensee, so legacy licences existed.
- GeomWare's original notices were replaced by NVIDIA's.

THIRD-PARTY CODE INSIDE NLIB: do not port.
NL_Math.cpp contains routines that cite Numerical Recipes in C and appear to be transcribed from it:
- SVD N_RealMatrixSingleValueDecompose: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_Math.cpp#L8664-L9040. Its structure matches NR's svdcmp: rv1, anorm, scale, 30-iteration limit.
- The tridiagonal and cornered-tridiagonal solvers, marked "Based on Numerical Recipes in C" (#L11669-L11860).
- The Brent minimiser N_FuncFindMinima (#L9362-L9550).
The NR code licence forbids redistributing its code, and NVIDIA's Apache grant cannot license copyright NVIDIA does not own. Re-derive these from primary sources: Golub–Reinsch / Golub–Van Loan, the Thomas algorithm, Brent 1973.

OTHER POINTS
- The NURBS Book's text and pseudocode are Springer copyright. Methods are free to use; transcribing the text or code is not.
- Patents: Apache §3 gives a patent licence for NVIDIA's own contributions. No patent search was done (unverified).
- No GPL or LGPL code in SMLib. The OCCT translators sit outside SMLib and are irrelevant here.

SOLIDCORE
- Apache-2.0 derivative work. It keeps NVIDIA's notice in NOTICE and ATTRIBUTION.md and gives per-file provenance.
- Its test corpus ssi-corpus is MIT or Apache-2.0. Its deny.toml bans copyleft dependencies.
- No CLA seen; contributions come from many people and AI agents (unverified).
- Either code base could be used under our rule (Apache-2.0 ported with notices kept into MPL-2.0), except the NR-derived routines.

**Alignment:** Against each Spare Kit principle:
- Never return invalid geometry, never fail silently, explicit errors. Fails as code: silent clamps and averages, results returned together with an error, global error flag, about 40 catalogued undefined-behaviour/hang defects in NLib alone. The methods can be kept if every one is rewritten with checked preconditions and typed errors.
- Exact, robust predicates. Not provided: all double arithmetic with absolute tolerances. This is acceptable for approximation algorithms but must not leak into decisions; our predicate layer stays separate.
- Deterministic on native and wasm. Fails: global state, uninitialised reads, platform maths libraries, and solidcore's own wasm divergence.
- Persistent naming. Not applicable at this level. The C++ layer uses pointer identity.
- Immutable data, pure operations. Fails: in-place operation with aliased outputs, mutated inputs (Coons, N-sided fill), NL_STACKS ownership.
- No unsafe. C++. solidcore passes.
- Full scale. Fails: absolute tolerances, a fixed maximum degree of 17 in stack arrays, a hard weight range of [1e-3, 100], and sentinel values such as INFINITE_PARAMETER = 1234567.
- Readable. NLib's per-function documentation is good; the code is not. The book plus Piegl–Tiller papers explain the same methods better.
- Headless. Passes.

Architecture: reject. Analytic types are NURBS subclasses carrying a second "STEP" parameterisation in degrees, with nonlinear conversions between the two. Our kernel should have exact first-class analytic types, NURBS as one representation, and procedural types (offset, iso, curve-on-surface) as values. solidcore copies this architecture deliberately ("port faithfully"), so it does not fit either.

What would have to change: everything structural. Only the algorithms carry over: their stated mathematics, constants and fallback strategies, re-implemented in pure Rust with typed knot vectors (no sentinels), unbounded degree, no input mutation, relative and scale-aware tolerances, typed errors, and our own fixed libm.

**Strengths**

- Most complete open NURBS algorithm set I know of under a permissive licence. Beyond the basic book algorithms it has fitting to a stated error for curves and surfaces, weighted/constrained least squares, fitting with derivatives, tangents and normals, fitting with a stiffness term, degree reduction with error bounds, multi-curve compatibility within a tolerance, skinning with G1/G2 boundary continuity, Gordon, Coons, N-sided fill, swung and scaled sweeps, exact offsets for special surfaces, symbolic B-spline products, and quadratic/quartic/quintic circles.
- It is the book authors' own production implementation, so it fixes the practical details the book leaves open: default constants, the order of steps in the fitting loop, re-projecting parameters, how the rational tolerance is scaled, and the sharper knot-removal bound using basis-function maxima.
- Good per-function documentation of contracts in NLib (inputs and outputs, memory rules, return codes).
- The C++ layer's global solver pattern (Bezier tree with bounding boxes, oriented boxes and tangent cones, culling, then a local Newton solve) is a reusable way to do global projection and inversion.
- Exact procedural representations exist: SmOffsetSurface, SmIsoCurve, SmCrvOnSurf.
- Apache-2.0, so we can study it, port methods, and run it as a difftest oracle.
- solidcore's upstream-issues.md (337 entries with triggers) and its Apache-licensed fuzz fixtures are an excellent checklist of mistakes to avoid and a source of adversarial tests.
- solidcore's difftest harness, with prebuilt C++ reference libraries, makes the C++ easy to use as an oracle from Rust.

**Weaknesses**

- About 30 undefined-behaviour defects, 4 wrong-result defects and 5 or more hangs catalogued in NLib alone. Several are confirmed by running the C++: heap corruption, use-after-free, out-of-bounds writes, results that depend on call history.
- Silent fallbacks: clamped parameters, averaged mismatched networks, a surface returned together with an error flag, unknown options turned into defaults, sweeps that only 'attempt' their tolerance, offsets that ignore loops and cusps.
- Global mutable state: the NL_ERROR record, tolerance globals, file statics in the spiral code, debug statics. Not reentrant; one process is the unit of concurrency.
- Absolute tolerances, maximum degree 17 in fixed stack arrays, a hard weight range [1e-3, 100], sentinel encoding of 2D and non-rational points (DBL_MAX) and of unbounded intervals (1234567).
- Inputs are mutated in place, and the output can alias the input.
- Analytic geometry is a NURBS subclass with a second, degree-based parameterisation. This is the wrong architecture for an exact kernel.
- No unit, golden-value or property tests for the NURBS core upstream.
- Numerical Recipes-derived routines (SVD, tridiagonal solvers, Brent) are a provenance risk.
- Chain of title (GeomWare to SMS to NVIDIA) is only verified through secondary sources.
- solidcore copies upstream behaviour, sentinels and architecture on purpose, is not bit-identical on wasm (its own finding, still unresolved), has coverage gaps in difftest (#294), and its README progress figure is out of date.

**Verdict:** PORT_METHODS

**Verdict reason:** NLib's algorithms are worth having. They are Piegl and Tiller's production versions of The NURBS Book methods, plus later refinements that no other permissively licensed source collects in one place. The code and architecture are not worth having: global state, silent fallbacks, in-place mutation, sentinel encodings, absolute tolerances, a fixed maximum degree, analytic-as-NURBS with two parameterisations, about 40 known memory-safety and hang defects in NLib, and Numerical Recipes-derived numerics.

solidcore does not change this. It is a faithful translation that keeps the same semantics and architecture, and it is not bit-identical between native and wasm.

So: re-implement selected methods from the primary literature (the book and Piegl–Tiller/Tiller papers). Use NLib as the reference for production details such as constants, step order and fallbacks. Use the C++ build (via solidcore's harness) and solidcore's defect log as test oracles.

Do not translate any code closely, and do not take the class hierarchy or tolerance model.

**What to take**

- Knot removal with Tiller's sharper error bound (error times the basis-function maximum) and rational tolerance scaling tol·wmin/(1+|P|max). Re-implement from Tiller 1992 / NURBS Book §5.4, checking details against https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvTool.cpp#L1707-L2243 and NL_BasisBasic.cpp#L1717-L1892 (N_BasisFindGlobalMax). Replace its unbounded bracketing with a bounded search to avoid HANG-2.
- Curve approximation to a stated error: interpolate at degree ps, remove knots, least-squares at degree +1, re-project parameters by Newton, repeat, restart one degree higher on failure; single-knot or multiple-knot mode; closed curves by constrained least squares. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvFit.cpp#L4422-L4816, with its helpers #L11015-L11373 (N_FitRemoveKnots, which keeps a per-point error vector) and #L13246-L13964 (weighted, constrained least squares, A9.6).
- Surface approximation to a stated error and knot removal on surfaces: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_SrfFit.cpp#L6481-L6920 and #L14122-L15031. Also the stiffness-regularised least-squares fit for scattered data (NL_SrfLstSqFit.cpp, N_AddStiffnessToSrfAMatrix from L38), useful for fitting point clouds.
- Degree reduction with an error bound (A5.11 plus repeated reduction), curve and surface: NL_CrvTool.cpp#L3582-L4236, NL_SrfTool.cpp N_SrfReduceDegree (L4859-L5940) and N_SrfReduceDegreeToTol (L5946-L6069).
- Making several curves compatible within a knot tolerance (merge knots that lie within a tolerance instead of exact unions), needed before skinning: NL_CrvTool.cpp#L4245-L4419.
- Skinning with G1/G2 boundary continuity and the spine method: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_SrfAdv.cpp#L13148-L15231 (study the method only; it contains UB-99). Basic skinning #L7835-L8278 and the tolerance variant #L9572-L10054.
- N-sided hole filling (n patches approximately G1 to an angular tolerance, shared centre point and normal): NL_SrfAdv.cpp#L18506-L19775. Method from Piegl–Tiller.
- Exact constructions: swung surface #L7701-L7829 and translational sweep #L11109-L11226 in NL_SrfAdv.cpp. Gordon as loft + loft − tensor product (#L11702-L11994), but replace the silent averaging at network crossings with a consistency check that returns a typed error.
- Rational circles with no interior knot: quintic, plus quadratic and quartic forms, with weight tables: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvCommon.cpp#L14-L29, #L251-L266, #L519-L534, #L883-L962. Use the quintic option for revolution (NL_SrfCommon.cpp#L1347-L1535, but size the weights by degree; UB-26).
- Offsets: keep exact offsets for special surfaces (plane, sphere, torus, cylinder, cone; revolution and extrusion through their generator) plus sample, interpolate and remove knots as the approximation fallback (NL_SrfOffset.cpp#L612-L1290, NL_CrvOffset.cpp#L1154-L1562). Model the procedural exact offset surface on SmOffsetSurface, but re-derive its third derivatives (WR-160 to WR-165).
- Global projection and inversion: the Bezier-span tree, oriented boxes, tangent-cone and turning-angle culling, then local Newton. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmCurve.cpp#L10841-L11603. Plus the surface Newton step on normalised derivatives (NL_SrfGeom.cpp#L96-L576). Add certified bounds where we need guarantees.
- Symbolic B-spline products and sums for exact derivative and normal surfaces: NL_FuncsAdv.cpp#L1667-L2182 and #L6454-L7075.
- Test oracle: build the C++ with solidcore's harness (prebuilt reference, scripts/fetch-cpp.sh). Compare geometric invariants, not bits: interpolation residuals, Hausdorff error against the fitting tolerance, knot-removal bounds. Exclude inputs on solidcore's defect list.
- Bug checklist and adversarial inputs: https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/plan/upstream-issues.md (NLib section L148-L440, L1651-L1720, L2266-L2304) and its fixtures/nlib.* fuzz cases (Apache-2.0, keep the notice). Also its libm finding (plan/findings.md#L46-L99) as the reason we need our own fixed maths library.
- Do NOT take: the NR-derived SVD, tridiagonal and Brent routines in NL_Math.cpp (L8664-L9040, L9362-L9550, L11669-L11860); the NL_* data model and sentinels; the SmTol tolerance model; the analytic-as-NURBS/STEP hierarchy; any solidcore code, which carries the same semantics.

### Challenge 1

**Refuted:** False

**Corrected verdict:** PORT_METHODS

**Errors found**

- FACT, material: the evaluation says 'NLib itself only does local Newton iteration' and puts the global search only in the C++ layer. That is wrong. N_CrvClosestPtMultiple is live code (NL_CrvGeom.cpp has no #if guards). Its own documentation says it 'uses a global search to find a good start point for Newton iterations': it culls spans, samples points, then runs Newton from the 3 best candidates. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvGeom.cpp#L876-L1471 . Callers name it 'the global solver': NL_CrvBasic.cpp#L6446-L6454 and NL_CrvAdv.cpp#L2656. Surfaces use N_SrfProjectPts (NL_SrfGeom.cpp#L2368), which a comment says requires 'simple' surfaces (NL_CrvNurb.cpp#L4530-L4545).
- FACT, material to what_to_take: 'All 48 nurbs units are ported' does not mean NLib is ported. solidcore ports only what its CAD subset can reach. The 48 nurbs units total about 70.5k upstream lines (plan/PROGRESS.md#L82-L131), and nlib.srf_fit is only '5 fns' (plan/modules.toml#L664-L675). harness/callgraph/unused_functions.txt lists 564 NL_ functions (about 77k lines) as unreachable and not ported. That list includes most of the methods the evaluation recommends: N_FitCrvApprox (L2576), N_FitRemoveKnots (L2583), N_CrvClosestPtMultiple (L2594), N_SrfFuncMultiplySrfFunc (L2619), N_CreateSkinSrfApproxTol (L2810), N_CreateGordonSrf (L2812), N_FitSrfApproxTol (L2874), N_GetClosestPtOnSrf (L2898), N_AddStiffnessToSrfAMatrix (L2915), N_SrfReduceDegreeToTol (L2931). https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/harness/callgraph/unused_functions.txt . Three consequences: (a) solidcore has no Rust port, FFI shim or difftest runner for these. Its reference build does compile all of SMLib (harness/cpp/CMakeLists.txt#L55-L56), so an oracle needs new shims, not just 'solidcore's harness'. (b) upstream-issues.md only audits the ported subset, so it is no checklist for these routines. (c) 'About 30 UB / 4 WR / 5+ hangs in NLib' is a lower bound covering only about 70k audited lines.
- FACT: the evaluation overstates what NLib adds beyond The NURBS Book. (1) The rational knot-removal tolerance tol*wmin/(1+|P|max) (NL_CrvTool.cpp#L1783-L1784) is the book's Eq. 5.30, not an NLib refinement. Evidence: geomdl's knot_removal says 'Refer to Eq 5.30' for tol (https://github.com/orbingol/NURBS-Python/blob/9d2555ae8217b5480ee83afb526a835ac3cc3c27/geomdl/helpers.py#L622-L641), and the ISU ME625 NURBS notes state the same bound. (2) The error-bounded approximation loop (interpolate, remove knots against a per-point error vector, least squares one degree higher, re-project) is, to my knowledge, the book's §9.4 'approximation to within a specified accuracy' with A9.8 RemoveKnotsBoundCurve. I could not open the book text online, so this point is unverified. Still credible as beyond the book: the basis-function-max bound (N_BasisFindGlobalMax, used at NL_CrvTool.cpp#L1889-L1911), the quartic and quintic circles, the constants, G1/G2 skinning, N-sided fill and the stiffness-regularised fit.
- CITATION: the circle line ranges NL_CrvCommon.cpp#L14-L29, #L251-L266 and #L519-L534 point at thin wrappers. They hard-code at=0.01 and call the *Tol versions. The real constructions are N_CreateQuadraticArcTol (#L75-L247), N_CreateQuarticArcTol (#L311-L515) and N_CreateQuinticArcTol (#L581-L737). The doc at #L536-L580 says the quintic arc is 'a quintic Bezier curve' with no interior knots. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_CrvCommon.cpp
- NUMBERS: I could not reproduce '1,323 N_/M_ functions; 330 compiled out; 993 live'. Parsing the 40 NL_*.cpp files for N_/M_ definitions followed by a body, I get about 1,252, of which about 321 sit inside #if/#ifdef NLIB_UNUSED, leaving about 931 live. The NL_*.h headers declare about 1,335 unique N_/M_ names. The order of magnitude is right; the exact figures are unsupported. These confirmed exactly: 40 files with 253,923 lines; 615,306 lines of Sm*.cpp; SmCurve.cpp 27,725 lines; SmSurface.cpp 35,380.
- MISSED SILENT FAILURE, strengthens the evaluation's case: N_BasisFindSpan returns a wrong span with no error for u = NaN. All three comparisons are false (L1465 `u GE U[m-p]`, L1492 `u LE U[p]`, L1507 loop condition), so the loop is skipped. It then sets *spn = mid, the middle span (L1544), and returns NL_NO. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_BasisBasic.cpp#L1465-L1548 . solidcore's find_span reproduces this as Ok(mid) (crates/solidcore/src/nurbs/knots.rs#L286-L347). Its defect log's FindSpan entries (UB-7, UB-8, a DOC item) do not record it, so the log is incomplete even for ported code.
- MISSED INPUT MUTATION, supports the evaluation: N_CreateRevolvedSrf changes the caller's profile curve in place (N_CrvNonRatToRat(cur) at NL_SrfCommon.cpp#L1439, N_Crv2dTo3d(cur) at #L1442). That makes three examples of the problem, not just Coons and N-sided fill. N_FillNSidedHole's own doc says incompatible inputs 'WILL BE DESTROYED' (NL_SrfAdv.cpp#L18562).
- OVERSTATEMENT: 'Tolerances are absolute' is not universal. N_FitCrvApprox scales its point tolerance by the data extent: top = 1e-4 and ltop = d*top (NL_CrvFit.cpp#L4506-L4517).
- INCOMPLETE 'do not take' list: N_SingleValueDecomposeSolve, an svbksb-style SVD back-substitution, also cites 'Numerical Recipes in C', Chapter 2 (NL_Math.cpp#L8447-L8487). The quadratic solver also cites NR p.184 (#L2512-L2516), but that stable-root method is standard and low risk.
- INCOMPLETE BUG DESCRIPTION: M_TridiagonalSystem is worse than described. The forward step writes P[j-1] instead of P[j] and reads Q[j-1] instead of the previous solution (NL_Math.cpp#L11746). Back substitution stops at j>1 (#L11750-L11753). The only live caller chain runs through N_FitPeriodicCubic, which is inside #if NLIB_UNUSED from NL_CrvFit.cpp#L19083, as the evaluation says. But the function itself is compiled and exported (GW_EXPORT, NL_Math.h#L176).
- MINOR: PROGRESS.md#L5 says 234/261 units, but 120 of the 234 are 'partial'. The 88.1% is of a 387,903-line CAD subset, not all of usd-brep. The README also says 'Nothing is usable end to end yet' (README.md#L28-L33).
- MINOR PROVENANCE: the code is not purely Piegl and Tiller's. It carries SMS-era changes, e.g. 'Revision Jan 03 RMB' (NL_Math.cpp#L2512) and bug-tracker tags [B146] (NL_SrfGeom.cpp#L180) and [B405] (NL_BasisBasic.cpp#L1480). NL_Globals.cpp#L17 ('file = ../../NLib/src/NL_Globals.c') does support NLib as the origin. The GeomWare to SMS to NVIDIA chain is still only in secondary sources (Wikipedia). NVIDIA's own NLib/SMLib manual pages now redirect and could not be checked.

**Argument:** The verdict holds. Most specific claims check out against the pinned source, but the plan for oracles and checklists is built on a wrong picture of what solidcore covers.

CONFIRMED against the source:
- Repository metadata and commits: three commits (c9c5979 is 1.0.0), Apache-2.0, 1,747 stars, 'not accepting contributions'.
- Sizes: 40 NL_*.cpp files with 253,923 lines; 615,306 lines of Sm*.cpp.
- Data model: sentinels NL_NOZ = +DBL_MAX and NL_NOW = -DBL_MAX (NL_Geometry.h#L45-L50); the global NL_ERROR.
- Mutable tolerance globals (NL_Globals.cpp#L76-L97): MTOL 1e-7, PTOL 1e-12, LUDT 1e-9, ZDTL 1e-8, WMIN/WMAX 1e-3 and 100, ITLIM 50, RECURSELIM 20. NL_MAXDEG is 17 and NL_MAXDER is 4.
- C++ layer constants: SM_EFF_ZERO and SM_EFF_ZERO_PARAM, and SM_INFINITE_PARAMETER = 1234567.0. The old tolerance model is the default branch (SmTol.h#L213-L256).
- Silent fallbacks: FindSpan's clamp, its zero-length end-span hack and the 'assigned silently' comment. Gordon averages at network crossings (NL_SrfAdv.cpp#L11856). Coons averages corners and reparameterises its inputs in place. Skinning checks weights after building the surface.
- Determinism problems: the static cbi switches and the spiral file statics; srand(time(NULL)) (SmFace.cpp#L21183, SmShell.cpp#L888); usd-brep issue #10; the process-as-concurrency-unit guide.
- KNOWN_ISSUES 1.4%; THIRD_PARTY_NOTICES lists no NLib; only NVIDIA SPDX headers.
- Analytic types inherit from SmBSplineCurve/SmBSplineSurface. SmSphere actually derives from SmSurfOfRevolution and SmCylinder from SmCone. The STEP parameterisation is in degrees.
- Numerical Recipes attributions in SVD, Brent and the tridiagonal solvers.
- UB-26: for a quartic revolution of 241° or more, N_CrvGetConicData gives n = 12, which overflows wi[9].
- No N_* calls in prog_test (the one hit is a comment).
- solidcore: 337 catalogued IDs, 31 UB entries in the NLib sections, WR-22, 23, 24 and 166; the libm divergence table and the 'nothing tests the wasm build's values' finding (findings.md#L46-L99); -ffp-contract=off in the reference build; the issue #294 audit; forbid(unsafe_code) and the deny lints.

ERRORS: see errors_found. Two of them are material:
- NLib does contain a global curve-projection search (N_CrvClosestPtMultiple).
- solidcore only ports what its CAD subset can reach. Its unused_functions.txt shows that most of what_to_take is not in solidcore at all: N_FitCrvApprox, N_FitRemoveKnots, N_FitSrfApproxTol, N_SrfReduceDegreeToTol, N_CreateGordonSrf, N_CreateSkinSrfApproxTol, N_CrvClosestPtMultiple, N_GetClosestPtOnSrf, stiffness fitting and surface-function products. So solidcore offers no difftest coverage and no defect audit for those routines. The '~40 NLib defects' figure covers only the roughly 70k lines solidcore audited, and the other ~77k lines of reachable-elsewhere NLib were never reviewed.

The evaluation also credits NLib with 'refinements' that are already in The NURBS Book: Eq. 5.30's rational tolerance, and very likely the §9.4 error-bounded approximation loop with A9.8. That weakens the argument that NLib is uniquely valuable for those methods.

THE OPPOSITE VERDICT (harsher: STUDY), argued as strongly as the evidence allows:
- Many of the headline methods are published in the book or in Piegl–Tiller papers, which are cleaner sources.
- NLib's 'production details' are often exactly the silent behaviour Spare Kit forbids: clamping, NaN returning the middle span, averaging, mutating inputs, a best-so-far answer returned with an error.
- The cheap oracle path is absent for most recommended routines.
- The unaudited code probably has a defect density like the audited part.

That case falls short of overturning PORT_METHODS, for three reasons:
- Apache-2.0 allows close reading and porting with notices, which the owner's rules accept.
- Several methods really are beyond the book and hard to find permissively licensed elsewhere: G1/G2 boundary-continuity skinning, N-sided fill to an angular tolerance, the stiffness-regularised scattered-data fit, merging knots within a tolerance across several curves, the sharper bound from the basis-function maximum, quartic and quintic circles, and the C++ pseudo-box/turning-angle global solver.
- A C++ oracle is still achievable: the reference build compiles all of SMLib, it just needs new FFI shims.

The opposite generous verdicts (PORT_CODE, or relying on solidcore) are ruled out by:
- the global state, the libm and wasm divergence, the sentinels and the fixed degree limit;
- the analytic-as-NURBS hierarchy with a second STEP parameterisation;
- solidcore's deliberately faithful copy of the same semantics, NaN-in-FindSpan included.

Verdict stays PORT_METHODS, with three corrections to what_to_take:
- Cite the *Tol circle functions, not the wrappers.
- Add N_CrvClosestPtMultiple as an NLib-level global-search reference, and add N_SingleValueDecomposeSolve to the 'do not take' list.
- Treat solidcore's harness and defect log as coverage only for the units it has ported. Any other routine used as an oracle needs new FFI shims and its own adversarial audit, starting with NaN, infinite and degenerate-knot inputs.

## intersection-booleans

**Commit sha:** usd-brep c9c5979f04cea8c38213004d4f04df0dcfb40d05 (the 1.0.0 release, 2026-10-09; the repo has only 3 squashed commits). solidcore was checked at 0a676a04c38229b10e924b5074cb6ade8ee897cf. jyviko/ssi-corpus was checked at b04c8a6093a8b3505d865d1dd874ec3879ccb1f9.

**Findings:** All links below use B = https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05

SIZE. The core files are large:
- SmSurfaceIntersector.cpp: 8,698 lines.
- SmAdvSurfaceIntersector.cpp: 1,836 lines.
- SmTopologyIntersector.cpp: 9,367 lines.
- SmMerge.cpp: 7,862 lines.
- SmProtoTopology.cpp: 8,181 lines.
- SmTopologySolver.cpp: 3,467 lines.
- Curve-curve intersection and classification sit inside SmCurve.cpp (1.26 MB) and SmCurveClass.cpp (1.16 MB).
- The Newton correctors sit inside SmSurface.cpp (1.59 MB).

1) SURFACE-SURFACE INTERSECTION (SSI).

Entry point: SmSurface::GlobalSurfaceIntersect always constructs an SmAdvSurfaceIntersector, unless a subclass override handles the pair ({B}/source/SMLib/src/SmSurface.cpp#L20375-L20466).

Analytic special cases are virtual overrides that fall back to the NURBS path:
- plane-plane: SmPlane.cpp#L1942.
- cone/cylinder against plane, cone and sphere: SmCone.cpp#L2335. IntersectWithCone only handles cylinders. For parallel cylinders it returns "1 or 2 lines of length 100.0 * dTol ... (why not points)" (SmCone.cpp#L2599).
- sphere: SmSphere.cpp#L1332.
- surface of revolution: SmSurfOfRevolution.cpp#L3020.
- surface of extrusion: SmSurfOfExtrusion.cpp#L1651.
- The torus inherits the revolution path.

General algorithm (SmSurfaceIntersector::DoIntersection, {B}/source/SMLib/src/SmSurfaceIntersector.cpp#L6106-L6727):
(a) Each surface is decomposed into a Bezier-patch tree (SmSurfaceCache). Leaf pairs whose boxes overlap are found by SmGlobalSolver recursion ({B}/source/SMLib/src/SmGlobalSolver.cpp#L130-L366).
- For non-B-spline surfaces the tree is built from approximate bicubic Hermite patches made from corner derivatives. There is no error bound, and the bounding boxes are not padded ({B}/source/SMLib/src/SmSurfaceCache.cpp#L679-L911, #L4860-L4966). The source says "gwc:todo - use a better approximation algorithm" (L5057). The culling is therefore not conservative.
(b) A grazing pre-check handles a single overlapping leaf pair (GrazingBoxProcess), and a boundary-plane test classifies the pair as GRAZE, DISJOINT or UNKNOWN.
(c) Start points come from intersecting each surface's boundary iso-curves with the other surface (FindBoundaryStartPoints, L1078).
(d) Tracing is predictor-corrector (TraceCurve L4921-L5568, ComputeNextPoint L3036-L3257, RefinePoint L3260-L3392):
- The predictor is a UV tangent step.
- The corrector is a 4-unknown surface/surface/plane Newton solve, SmSSPIntersectENFO in SmSurface::LocalSurfaceIntersect. It uses at most 20 iterations, a desired accuracy of SM_EFF_ZERO_SQ, and a boundary handler that allows 3 hits ({B}/source/SMLib/src/SmSurface.cpp#L27439-L28044, Newton set-up at L27671-L27688).
- A span is accepted if the tangent angle is below the angle tolerance (default 20°) and five Hermite samples are within approxTol/2 in 3D and on both UV curves (TestSpanAccuracy L3433).
- The step size is constant (100·tol) and only adapts by ×1.5 and ÷1.5. A curvature-based step rule is commented out (L2937-L2991).
- A trace closes by snapping to start or through points (TestPoint). Re-tracing is detected by testing the second point against curves already found.
- The run hard-fails at 5,000 points (L5511-L5524).
(e) Interior loops: SmAdvSurfaceIntersector::FindInteriorCurves takes one closest-midpoint candidate per intersecting leaf pair, solves it with LocalSurfaceIntersect, and traces in both directions ({B}/source/SMLib/src/SmAdvSurfaceIntersector.cpp#L1139-L1368).
(f) Output: each curve becomes three degree-3, C0-knotted (G1 by construction) Hermite B-splines (one 3D, two UV) plus a deviation value (sm_FlushCrv, SmSurfaceIntersector.cpp#L5576-L5620). Intersection curves are therefore approximations held to a tolerance.
- The agent docs claim "SMLib uses exact NURBS surface-surface intersection ... no approximation is introduced" ({B}/.agents/operations/booleans.md#L11). That claim is false.
- The default tolerance is 1/1000 of surface size (SmSurfaceIntersector.cpp#L543-L672).

Tangent and singular handling (ComputeSingularityPoint, SmAdvSurfaceIntersector.cpp#L116-L966):
- Contact is classified with a taxonomy, CROSSING / TANGENT_POINT / TANGENT_CURVE / SINGULARITY / COINCIDENCE / TOUCHING ({B}/source/SMLib/inc/SmSurfaceIntersector.h#L52-L112).
- The classification uses principal curvatures and the intersection of the two surfaces' Dupin indicatrices (MakeDupin, L988).
- Outgoing directions go into a fixed array aSingDirections[8] (L164). The code comment reads "we have experienced examples with 8 vectors, that is of course a bug". The array is written without a bounds check (sm_AddSingDir, L54-L79).
- Saddle asymptotes are found by evaluating the hyperbola at Y=10000 (L265).
- At a start point with several branches only the first is followed ("GWC_NEED_TO_REMEMBER_THAT_MULTIPLE_CURVES_CAN_START_FROM_THIS_POINT_HERE", L868). If no branch points inward the code says "hope for the best" (L873).
- Tangent curves are refined at 10× the tolerance (RefinePoint).

2) CURVE-CURVE AND CURVE-SURFACE INTERSECTION.

SmCurve::GlobalCurveIntersect ({B}/source/SMLib/src/SmCurve.cpp#L6174):
- Closed forms exist for line/line, line/ellipse and ellipse/ellipse (AnalyticCurveIntersect, L6073-L6165).
- Otherwise it intersects Bezier-span trees with pseudo-boxes and runs Newton per leaf pair (SmCCIGlobalSolver::LocalSolve, L4881-L6065). Tangency guesses are added by sm_FindAdditionalCcTangentGuess (L4310).
- A sampled GlobalCoincidenceChecker follows, then heuristic merging of range and point solutions. The source says "It's a lot of heuristics to get right" (L6695).
- An early exit is accepted when the tangents differ by more than 20°.

SmSurface::GlobalCurveIntersect (SmSurface.cpp#L17020 onwards) has these steps:
- a degenerate-curve case;
- a line case through GlobalLineIntersect;
- "Run quick not robust coincidence check" (L17070), which drops the curve onto the surface;
- a tree/Newton path.

3) BOOLEANS.

Entry: SmApiBoolean checks only for NULL or identical operands. It chooses ManifoldBoolean if both inputs are manifold solids, otherwise NonManifoldBoolean, and returns SM_SUCCESS without validating the result ({B}/source/SM_API/src/SmApiBrep.cpp#L186-L255).

Python consumes the inputs. The kernel modifies pBrep1 in place and deletes pBrep2.

ManifoldBoolean ({B}/source/SMLib/src/SmMerge.cpp#L2510 onwards) runs as follows:
- Step 0: SmTemporaryBrepTransform translates both inputs in place if the centre is more than 1000 from the origin, then translates them back. The round trip adds rounding to the inputs (L2338-L2386).
- Step 1: SmTopologyIntersector::IntersectInsertRelate ({B}/source/SMLib/src/SmTopologyIntersector.cpp#L1092-L1270) has three stages:
  - IntersectTopology (L1580-L2250) computes shell-vertex, wire/lamina-edge and face-face intersections (SmFace::FaceIntersect, then GlobalSurfaceIntersect plus curve classifications). Every result is stored as an SmVertex/SmEdgeDefinition inside SmProtoVertex/SmProtoEdge/SmProtoFace clusters. The capture distance is max(0.01, 2000·tol), an absolute value in model units (L1146-L1159).
  - ResolveIntersections decides how many vertices and edges each cluster becomes ({B}/source/SMLib/inc/SmProtoTopology.h#L15-L100 explains the order-independence rationale).
  - ImprintIntersections then mutates both B-reps, and SetRelationships fills pointer-keyed mate maps.
- Step 2 is a fix-and-classify loop, capped at 10 outer and 10 inner passes, after which it continues:
  - Common vertices with one common edge are "single vertices". The loop tries CloseIntersectionLoop and then FixGaps.
  - Faces on common edges are classified by FindRadialSector of the edge-use against the other body's edge. The result is either a coincident face pair with sameSide/nonSolid flags (FixFoundFaceCoincidence merges the coincident faces' edges) or a containing region, with void/solid lookup tables per operation (L3160-L3628).
- Step 3: the remaining unmarked faces are flood-filled (CollectFaces) and classified by Point3DClassify of one edge sample (L3679-L3830).
  - Point3DClassify ({B}/source/SMLib/src/SmBrep.cpp#L24687-L24841) first runs a boundary solve. It then fires rays in ±X, ±Y and ±Z and takes the first success.
  - RayFire takes the nearest, lowest-dimension hit and calls LocalCurveSector (sector classification, not parity) (SmBrep.cpp#L25312-L25455).
- Steps 4 to 9: wires, insertion of the other body's kept faces, face deletion, shell void propagation, and post-process removal of redundant edges.
- An older direct path (IIRSurfs/IIREdgeBrep) survives only through CloseIntersectionLoop and an uncalled ReIntersectAtVertex.

4) LIVE SILENT-FAILURE PATHS, each verified in source:
(i) The Gauss-map loop-separation test is dead code. SolveBranch sets m_bSeperateGaussMaps=TRUE at L1383 and then tests `if(!m_bSeperateGaussMaps)` at L1401. Interior-loop detection therefore depends only on leaf size.
(ii) FindInteriorCurves skips candidates whose normals are within 5° at the leaf midpoint (L1184) or within 2° after Newton (L1311). Near-tangent interior loops are dropped without any signal.
(iii) TraceCurve reduces the step below 1e-10 and then breaks out with SM_SUCCESS, leaving a truncated curve (L5134-L5188). In DoIntersection a failed TraceCurve start point is moved to the through-point list and no error is raised (L6538, L6622-L6626).
(iv) IntersectInsertRelate reports an ImprintIntersections failure with SE_MSG. In release builds SE_MSG compiles to ((void)(a)) ({B}/source/SMLib/inc/SmMessages.h#L627-L628), and eStat is then overwritten by SetRelationships (SmTopologyIntersector.cpp#L1233-L1264). The B-reps have already been mutated by that point.
(v) The status of shell-vertex and wire-edge intersection is thrown away (SM_REF1(eStat), L1627-L1646).
(vi) CloseIntersectionLoop's status is ignored ("Was SER. [B601]", SmMerge.cpp#L2958-L2972). Open intersection loops only set m_bIntersectionLoopClosed=FALSE, and no API reads that flag.
(vii) If FindRadialSector fails, the code continues with "too difficult an edge ... just skip it" (L3208).
(viii) The edge-retry loop that classifies leftover faces always uses sFaceEdges[0] instead of [jjj] (L3764). Faces that still fail are skipped with `continue` (L3789-L3790). Such faces are on neither the keep nor the delete list, so pBrep keeps them by default.
(ix) Bug in GlobalCurveIntersect at L6594-L6599: when a coincident range reaches the end of curve 2, the code assigns curve-2 parameters to sIvl1 (curve 1's search interval). This corrupts the search domain of curve 1.
(x) Upstream issue #1 confirms the effect in practice: boolean_union(a, mirror(a)) returns a unchanged with success and manifold=True (https://github.com/NVIDIA-Omniverse/usd-brep/issues/1). PR #17 is the proposed one-line fix (planarity padding).

5) MEASURED ACCURACY, from an independent exact-truth corpus.

jyviko's corpus ran 119 SSI cases against the C++ GlobalSurfaceIntersect at tolerance 1e-6. 81 of 119 pass (https://github.com/tangency-labs/solidcore/issues/17). solidcore's scorer reproduces the same 119 verdicts. Results by class:

| class | pass |
|---|---|
| transversal | 14/14 |
| seams, poles, closed surfaces | 12/12 |
| coincident regions | 7/7 |
| patch boundaries | 8/8 |
| tangent curves | 7/10 |
| isolated tangent points | 3/7 |
| singular crossings | 3/6 |
| near-degenerate | 11/24 |
| Yang-Jia-Yan 2023 reconstructions | 5/19 |

The failures are logged as WR-74 to WR-80 in https://github.com/tangency-labs/solidcore/blob/0a676a04c38229b10e924b5074cb6ade8ee897cf/plan/upstream-issues.md#L2805-L2870:
- tangent points missed;
- tangent curves missed, duplicated, or 3.4e-5 off the surfaces;
- curves run through singular points instead of ending there;
- small loops collapsed to one TOUCHING point;
- near-coincident surfaces reported as coincident;
- SM_ERR_NULL_POINTER returned on valid input.

6) SOLIDCORE (Rust port, Apache-2.0, about one third ported).

All intersect and boolean units are partial or in progress (plan/PROGRESS.md). The port is deliberately bug-faithful so that it difftests equal to the C++ at 1e-9:
- WR-52, WR-53, WR-69 and WR-70 are marked "Rust port: Faithful".
- The SSI header says "Statuses upstream ignores are ignored" (crates/solidcore/src/intersect/adv_surface_intersector.rs L1-L60).

Its architectural changes are good: an arena, typed ids, error values, a seeded RNG, ordered maps, and wasm builds. Its algorithms and failure semantics are SMLib's.

**Robustness exactness:** ARITHMETIC. Everything is IEEE double. A search of SMLib found no exact predicates, interval arithmetic, adaptive or extended precision, or Shewchuk-style orientation tests.

TOLERANCES are a mix of absolute and relative values:
- SM_EFF_ZERO=1e-12, SM_EFF_ZERO_SQRT=1e-6, SM_EFF_ZERO_SQ=1e-24 ({B}/source/SMLib/inc/SmMath.h#L38-L45). These are used as absolute thresholds on curvature, UV distance and step size; curvature thresholds of this kind depend on units.
- The default SSI tolerance is 1/1000 of the surface size, and the default angle tolerance is 20° (the docs say 20° in one place and 30° in another).
- The topology capture distance is max(0.01, 2000·tol) in model units.
- Inputs are recentred only when the centre is more than 1000 units from the origin.

DECISIONS ARE HEURISTIC:
- 0.9 dot-product de-duplication of singular directions.
- A 20° tangency window for curve-curve.
- 5° and 2° normal-angle skips for interior loops.
- A 6×6 sample grid plus corner matching for surface coincidence (SmSurface::CoincidenceCheck, SmSurface.cpp#L7266).
- Pseudo-box and box culling for analytic surfaces built from unbounded Hermite approximations, so the culling is not certified.
- Ray-fire point classification that takes the first successful axis ray, with no vote.

DEGENERACY HANDLING:
- The contact taxonomy and Dupin indicatrix analysis are rich in intent. In practice:
  - Branches are lost at multi-branch start points.
  - MakeDupin's bottom hyperbola uses k2 for both axes (WR-69, confirmed by C++ difftest).
  - Singular directions can overflow the 8-slot array, which is an out-of-bounds write (UB-97).
  - Tangent curves are refined at 10× the tolerance.
- The project's own guidance tells users to avoid tangent and coincident configurations and to "Offset one body by a tiny amount if needed" ({B}/.agents/operations/booleans.md#L100).
- Issue #16 shows SM_ERR on tangent cylinder/box intersections and wrong touching/separate answers.

FAILURE MODES:
- Errors are status codes such as SM_ERR, SM_ERR_BAD_INTERSECTIONS and SM_ERR_NOT_CONVERGING, and many causes collapse into SM_ERR 1001.
- In release builds SE/SE_MSG compile to void and SM_ASSERT to nothing (SmMessages.h#L627-L628, #L714-L719). Asserted invariants are therefore unchecked in production.
- There is no rollback: inputs are mutated or deleted before a failure can be reported.
- Results come back partial while reporting success:
  - truncated traces;
  - skipped faces;
  - open intersection loops flagged only internally;
  - the mirrored operand dropped in issue #1.
- Hangs exist: IntersectInsertRelate never returns at tolerance 0 (HANG-25, confirmed). SmCurve::GlobalCoincidenceChecker recurses without a depth bound (HANG-10).
- Type confusion: SER inside functions that return SmBoolean turns failures into TRUE, for example in IsPointOnCurve (WR-52).
- TestSpanAccuracy compares a tangent length against an angle tolerance (WR-53).

AGAINST AN IRON-CLAD STANDARD this subsystem fails. It can return invalid or incomplete geometry with success. Its predicates are not robust. It does not certify that all intersection branches and loops were found. On an exact corpus it misses about 32% of SSI cases, concentrated in tangent, singular and near-degenerate inputs, which are exactly the cases a BIM kernel meets constantly (coplanar walls, flush faces, tangent fillets).

**Determinism:** Not deterministic, and not designed to be.

(1) Iteration order depends on pointers. SmMapPtrToPtr is a std::unordered_map keyed by raw pointers ({B}/source/SMLib/inc/SmMapPtrToPtr.h L42, L115).
- GetMappedBrepObjects and GetMappedOtherObjects iterate it to build m_sIntersectionEdges/Vertices for BooleanPostProcess ({B}/source/SMLib/src/SmTopologyIntersector.cpp#L905-L958).
- Upstream even guards this with `#ifdef SM_DEFINED_HASH_ORDER static_assert(false, "not implemented")`.
- SmBrep::Transform also iterates a pointer-keyed set (ND-2 in solidcore's upstream-issues log).

(2) The clock seeds an RNG: srand(time(NULL)) in SmFace::GetPointsInFace (SmFace.cpp#L21183) and in SmShell::IsInnerShellOf (ND-1).

(3) A function-local static is fixed by the first call (ND-3, fillet solver). Global counters and statics are mutable.

(4) The process-global SmContext holds unlocked mutable mark counters, so it is single-threaded only and the supported concurrency unit is the process ({B}/.agents/operations/concurrency.md). TBB paths are compiled out of the shipped build.

(5) The build has no floating-point contraction control. premake sets only NDEBUG and optimize "On" ({B}/premake5.lua#L69-L106) for x86_64 and aarch64. solidcore had to add -ffp-contract=off to get a reproducible reference. Bit-identical results across native and WASM, or across x86 and ARM, are therefore not established (unverified, but likely to differ with FMA on aarch64).

(6) Inputs are mutated in place by recentring, so floating-point drift on untouched entities depends on whether recentring fired.

(7) Observed in practice: issue #10 reports shell_brep and offset_brep, which go through SmMerge booleans (trace at SmMerge.cpp:672), giving different volumes, SM_ERR or non-manifold bodies for identical input across processes, and also within one process (https://github.com/NVIDIA-Omniverse/usd-brep/issues/10). The exact root cause is unverified, but it is consistent with (1) and (2).

(8) Order dependence: the ProtoTopology header admits that Merge(B1,B2) ≠ Merge(B2,B1) under the old algorithm, and ProtoTopology was introduced to reduce it. Order independence is not proven.

**Tests and quality:** TESTS.
- prog_test 'booleans' (tests/prog_test/src/smMergeTest.cpp, 86 KB) mostly checks the status plus ValidateCounts of vertex, edge and face counts, and reads stress-test files.
- SM_API_test (TestSmIntersectors.cpp, 796 lines) uses hand-crafted status and shape checks.
- 56 ssi_*.smp fixtures cover coincident cylinders, spheres and tori, tangent tori, cyl/cyl singular, rev/rev 360, Viviani-like cases and others, plus Boolean/*.smb models (BoxUCylTan, BoltMBox, BooleanBug.smp) in TestFiles/pt_TestFiles.
- Upstream has no exact-ground-truth scoring, no property or fuzz testing, and no determinism tests.
- A debug-only SM_VALIDATE_INTERSECTORS mode cross-checks the analytic overrides against the general B-spline intersector.

QUALITY.
- Comments are verbose and helpful, and the agent docs and skills are good for navigation.
- The files are enormous: SmBrep.cpp is 1.7 MB.
- Interleaved #ifdef SM_DEBUG_CODE graphics blocks and much commented-out experimental code ("GWC:DEBUG ... BACKED OUT A 6.0 to 6.5 CHANGE [CAUSES FAILURE IN GINA0148 BUG]") show decades of regression-driven patching.
- Some code is dead: the Gauss-map separation test and IIRSurfs via the uncalled ReIntersectAtVertex.
- There are no-ops such as `m_dCurveTraceDirection *= 1.0` labelled "restore" (L1355, L6564).
- The docs make false claims ("no approximation is introduced") and contradict themselves on the default angle tolerance (20° vs 30°).

ISSUES. 20 issues are open about a week after release, including:
- #1: silent dropped operand.
- #16: tangent boolean SM_ERR and wrong relationship answers.
- #10: nondeterministic shell and offset.
- #12: a boolean result with is_manifold anti-correlated.
- #4: translated operands leading to an invalid BRepArray.

The solidcore team's upstream-issues.md (3,234 lines) logs, from source reading, difftests, fuzzing and C++ runs, many crash/UB, hang, nondeterminism and wrong-result entries in exactly these units: UB-74, UB-97, UB-175, HANG-25, ND-1, ND-2, WR-52, WR-53, WR-69, WR-70, WR-74 to WR-80, WR-95, WR-99 and WR-101 (https://github.com/tangency-labs/solidcore/blob/0a676a04c38229b10e924b5074cb6ade8ee897cf/plan/upstream-issues.md).

solidcore itself has a strong difftest harness: units, fuzzing, adversarial fixtures, and an --ssi-corpus scorer in crates/difftest/src/ssi/*. Its SSI and boolean units are still partial.

**Legal:** USD-BREP.
- The root LICENSE is Apache-2.0, "Copyright (c) 2020-2026 NVIDIA CORPORATION & AFFILIATES".
- Every SMLib and SM_API file carries an `SPDX-License-Identifier: Apache-2.0` NVIDIA 2026 header.
- There is no root NOTICE file (fetching it returned 404). THIRD_PARTY_NOTICES.md covers only OpenUSD, oneTBB, hwloc, MaterialX, OpenSubdiv, CPython, HACL*, pybind11, the UsdSolid proposals and usd-validation-nvidia. None of these is part of SMLib.
- Apache-2.0 §3 gives a patent grant from NVIDIA as contributor.

PROVENANCE (from secondary sources, unverified in primary documents):
- SMLib comes from Solid Modeling Solutions (Bellevue, WA), founded in 1998 by Robert Blomgren and Jim Presti. Blomgren's earlier company, Applied Geometry (founded 1984), drew on Boeing's TIGER NURBS work.
- NLib was bought from GeomWare in late 2001. GeomWare's NLib lineage is commonly attributed to Piegl and Tiller (unverified).
- SMS had an alliance with IntegrityWare until 2004. GSNLib supplied the curve/curve and surface/surface intersection.
- NVIDIA acquired SMS in May 2022 and dissolved it.
- The original SMS, GeomWare and IntegrityWare copyright headers have been replaced by NVIDIA SPDX headers.

Before any code port, the owner should confirm that NVIDIA holds the copyright needed to relicense GeomWare- and IntegrityWare-derived portions, especially the GSNLib intersection code. The acquisition makes this likely, but it is unverified. I found no evidence of GPL, AGPL or LGPL material in these files.

PATENTS. Marching SSI, Dupin-indicatrix analysis and radial-sector classification are decades-old published techniques. No patent search was done.

OTHER PROJECTS.
- solidcore: Apache-2.0. Its NOTICE correctly attributes the work as a derivative of usd-brep c9c5979, and contributors are AI-assisted. Porting from solidcore would carry both notices.
- jyviko/ssi-corpus: dual LICENSE-APACHE and LICENSE-MIT, which GitHub reports as Apache-2.0. Its 25 cases derive from Yang, Jia & Yan, ACM TOG 42(6) 211 (2023): 6 use published data and 19 are the corpus author's own reconstructions. Reusing numeric data from a paper is generally fine, but the owner should check.

FOR SPARE KIT (MPL-2.0). Apache-2.0 code may be ported with its notices kept. The recommendation here is clean-room re-derivation of textbook methods, which carries no obligations. Any translated snippet must keep the NVIDIA SPDX and Apache notice, and the solidcore NOTICE too if it came via solidcore.

**Alignment:** Each Spare Kit principle, with what would have to change:

1. Iron-clad, never invalid, booleans never fail silently: FAILS. Live paths return success with incomplete or invalid results (findings 4.i to 4.x and issue #1). Nothing validates the result.
   - Needed: a validation gate on every result, an explicit Result type with no partial success, and a closure guarantee for intersection graphs.
2. Exact and robust predicates: FAILS. Everything is f64 with absolute epsilons and heuristic angle cut-offs, coincidence is sampled, and culling is not certified for analytic surfaces.
   - Needed: certified enclosures (Bernstein or interval bounds, with error bounds for analytic surfaces), exact or filtered predicates for orientation and classification, and certified loop detection (normal-cone or Gauss-map separation that actually runs, or a Yang-Jia-Yan-style complete SSI).
3. Explicit structured errors: PARTIAL. A status-code taxonomy exists (SM_ERR_BAD_INTERSECTIONS, SM_ERR_NOT_CONVERGING, SM_ERR_DEGENERATE_SURFACE and others), but most failures become SM_ERR 1001, the trail exists only through debug callbacks, and SER inside boolean-returning functions turns errors into TRUE.
4. Deterministic and bit-identical in native and WASM at any thread count: FAILS. Pointer-hash iteration, srand(time), global mutable context, no FP-contract control, and issue #10 observed in practice.
5. Persistent naming derived from operations: ABSENT. Identity is by pointer. SmTrackTopologyChanges records ADD, SPLIT, MERGE, TRIM, COINCIDENT, RM and CHANGE_GEOMETRY notify events keyed by pointers, which may be reused after delete. That event taxonomy is a useful method input; the mechanism is not.
6. Immutable data, pure operations: FAILS. Inputs are consumed or mutated in place (pBrep modified, pOther deleted, temporary recentring), and there is no rollback.
7. No unsafe: N/A for C++. UB-97 is an out-of-bounds write inside SSI.
8. Full scale: FAILS. Tolerances are partly relative to surface size and partly absolute (capture distance 0.01, 1e-12 and 1e-6 curvature and step thresholds), and recentring happens only beyond 1000 units.
9. Readable by AIs and people: MIXED. Good intent comments and agent skills, but 8 to 10 thousand line files, debug-graphics interleaving and dead code.
10. Headless: yes.
11. Owner's architecture rule: the algorithmic architecture is tolerance-based approximate marching feeding a mutable pointer graph with iterative repair loops (FixGaps, CloseIntersectionLoop, 10-pass caps). That is the opposite of a certify-then-construct immutable kernel.

Only isolated METHODS fit our architecture, and each needs re-derivation with certified numerics:
- the evidence-clustering concept of ProtoTopology;
- the contact taxonomy;
- the Dupin-based branch-direction analysis;
- radial-sector face classification;
- nearest-hit ray classification with a local sector test.

solidcore does not change this picture. It fixes the language-level problems (safety, arena, error values, seeded RNG, ordered maps, wasm) but keeps the SMLib algorithms and failure semantics on purpose.

**Strengths**

- Broad functional scope already integrated: analytic plus NURBS SSI with tangent, coincident and region-boundary curve types, curve-curve and curve-surface intersection, manifold and non-manifold booleans, cookie-cutter, imprint, slice, XOR, CSG trees and a polygon path. It is a complete working reference for the shape of the problem.
- On the exact corpus, transversal SSI (14/14), seams, poles and closed surfaces (12/12), coincident regions (7/7) and patch boundaries (8/8) all pass, and every returned curve lies within 1.9e-6 of both exact surfaces apart from two tangent curves (solidcore issue #17).
- A clear contact taxonomy for intersection points (CROSSING / TANGENT_POINT / TANGENT_CURVE / SINGULARITY / COINCIDENCE / TOUCHING) and curves (TOUCHING / CROSSING / TANGENT / COINCIDENT / NEAR_TANGENT / REGION_BOUNDARY), each with worked examples: SmSurfaceIntersector.h#L52-L112, SmTypes.h#L862-L880.
- The ProtoTopology design gathers every redundant intersection (face/face, edge/face, edge/edge, vertex/x) into clusters before deciding vertices and edges, aiming at order independence and tolerance-overlap resolution. This is the right idea for the insert stage of a boolean (SmProtoTopology.h#L15-L100).
- The classification design is sound in intent: radial sectors around common edges, a flood fill over unmarked faces, sameSide/nonSolid handling of coincident faces with explicit per-operation keep/delete tables, and ray classification by nearest lowest-dimension hit plus a local sector test instead of fragile parity counting.
- A Newton corrector with a plane constraint for marching, plus span acceptance that checks the 3D curve and both UV curves against both surfaces at interior samples.
- Apache-2.0 with an NVIDIA patent grant, and Python bindings that make it easy to probe as a comparator.
- An external ecosystem has already grown around it: solidcore's upstream-issues log and difftest harness, and the jyviko/ssi-corpus exact-truth benchmark.

**Weaknesses**

- Booleans fail silently on live paths: ImprintIntersections errors are discarded in release builds (SE_MSG compiles to void, then eStat is overwritten), CloseIntersectionLoop and shell/wire intersection statuses are ignored, FindRadialSector failures are skipped, unclassified faces are skipped and kept by default, and open loops only set an internal flag. SmApiBoolean returns SM_SUCCESS with no result validation. Issue #1 shows the result: a dropped operand reported as success and manifold.
- Leftover-face classification retries always use sFaceEdges[0] (SmMerge.cpp#L3764), so the retry loop is ineffective.
- The SSI completeness guarantee is missing. The Gauss-map separation test is dead code (L1383/L1401). Near-tangent interior loop candidates are skipped at 5° and 2°. Culling boxes for analytic surfaces come from unbounded Hermite approximations. Truncated traces return SM_SUCCESS. Multi-branch singular start points follow only one branch.
- Measured on an exact corpus: 81/119 SSI pass. Isolated tangent points 3/7, singular crossings 3/6, near-degenerate cases 11/24, Yang-Jia-Yan reconstructions 5/19. Tangent curves come back duplicated or 3.4e-5 off the surfaces. SM_ERR_NULL_POINTER is returned on valid inputs.
- Memory and type bugs inside the intersector: aSingDirections[8] overflow (UB-97), the MakeDupin conic error (WR-69), boolean/status confusion through SER (WR-52), and a curve-curve interval mix-up in which curve-2 parameters are assigned to sIvl1 (SmCurve.cpp#L6594-L6599).
- Nondeterminism: pointer-keyed unordered_map iteration, srand(time(NULL)), function-local statics, global mutable context, and no FP-contract control. Issue #10 shows run-to-run and in-process variance of boolean-based shell and offset.
- Approximate and tolerant geometry: intersection curves are degree-3 piecewise Hermite fits with a deviation value, despite the docs claiming exactness. The default tolerance is 1/1000 of surface size. Absolute constants (capture distance 0.01, 1e-12 and 1e-6) make behaviour depend on scale.
- Mutable in-place architecture: inputs are consumed, deleted or recentred, there is no rollback, identity is by pointer, and there is no persistent naming.
- Heuristic layers stacked by regression history (GWC:DEBUG back-outs, 10-pass repair loops, FixGaps). The code is hard to reason about and impossible to certify.
- Hangs: zero-tolerance IntersectInsertRelate (HANG-25) and unbounded GlobalCoincidenceChecker recursion (HANG-10).

**Verdict:** STUDY

**Verdict reason:** Measured against Spare Kit's iron-clad, deterministic, immutable and persistently named architecture, this subsystem fails on every core axis.

- Booleans fail silently on live code paths, and issue #1 confirms it in practice.
- Predicates are f64 heuristics with absolute epsilons.
- SSI completeness is not certified: the loop-detection test is dead code, and the run scores 81/119 on an exact corpus.
- Results are nondeterministic (pointer-hash order, srand(time); issue #10 confirms this).
- Data is mutated in place with pointer identity.

PORT_CODE is therefore out. solidcore is not the way either: it is a deliberately bug-faithful port in which upstream-ignored statuses stay ignored.

It is also not a good DIFFTEST_ORACLE. An oracle must be right on the hard cases (tangent, singular, near-degenerate), and those are where it is wrong. The right oracle for SSI is exact ground truth (jyviko/ssi-corpus). SMLib can serve at most as a non-authoritative cross-check on transversal cases.

What is worth having is a handful of design ideas and a lot of negative knowledge:
- ProtoTopology evidence clustering;
- the contact taxonomy;
- Dupin-indicatrix branch analysis;
- radial-sector classification and local-sector ray classification;
- the catalogue of failure modes, which tells us what our tests must cover.

Each must be re-derived on certified numerics in our own architecture. That is study, not porting.

**What to take**

- Method (re-derive, do not port): the contact taxonomy for SSI results, used as the checklist for our structured intersection result enum and its error variants. Point types are at https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmSurfaceIntersector.h#L52-L112 and curve types at https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmTypes.h#L862-L880. Ours needs explicit variants for isolated tangent points, tangent curves, singular nexus points, which must split curves, and coincident regions.
- Method: ProtoTopology evidence clustering. Collect every redundant intersection across dimension pairs into proto-vertices, proto-edges and proto-faces, then resolve jointly into topology, independent of order. Rationale: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmProtoTopology.h#L15-L100; producer: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmTopologyIntersector.cpp#L1580-L2250. Re-derive with certified clustering, deterministic sorted ids, and an op-derived name per proto-entity. That last part is the natural hook for persistent naming of boolean-created edges and vertices.
- Method: Dupin-indicatrix analysis to find branch directions at points with parallel normals (planar/cylindrical/elliptic/hyperbolic cases, intersections of indicatrix conics): https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmAdvSurfaceIntersector.cpp#L116-L966 and MakeDupin at #L988. Take only the idea. Fix WR-69 (the bottom hyperbola uses k2 twice), keep every branch, use an unbounded direction set, and back it with higher-order or certified local analysis.
- Method: boolean classification design. Radial-sector classification of edge-uses at common edges; sameSide/nonSolid handling of coincident face pairs with per-operation keep/delete truth tables; flood fill over unmarked faces. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmMerge.cpp#L3160-L3830. Re-implement with exact predicates. Treat any unclassifiable face as a hard structured error, never a skip.
- Method: point-in-solid classification by the nearest lowest-dimension ray hit followed by a local sector test (LocalCurveSector) instead of parity: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmBrep.cpp#L25312-L25455. Replace its first-success axis rays with a certified or generic-direction scheme.
- Baseline reference only: the marching corrector formulation, a 4-unknown surface/surface/plane Newton solve with the plane normal set to the previous tangent (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmSurface.cpp#L27439-L28044), and span acceptance by sampled 3D and UV deviation (https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmSurfaceIntersector.cpp#L3036-L3257). Our tracer must add certified step control and guaranteed branch and loop coverage.
- Coverage checklist: the analytic special-case dispatch matrix, listing which closed-form pairs to implement exactly. SmPlane::GlobalSurfaceIntersect (SmPlane.cpp#L1942), SmCone (SmCone.cpp#L2335), SmSphere (SmSphere.cpp#L1332), SmSurfOfRevolution (SmSurfOfRevolution.cpp#L3020), SmSurfOfExtrusion (SmSurfOfExtrusion.cpp#L1651), all under https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/.
- Event taxonomy for naming lineage: ADD, SPLIT, MERGE, TRIM_NO_SPLIT, COINCIDENT, RM and CHANGE_GEOMETRY in https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmTrackTopologyChanges.h#L249-L262. Use it as input for designing op-derived persistent names. Its pointer-keyed mechanism is not usable.
- Test inputs (Apache-2.0 data): the 56 ssi_*.smp fixtures and the Boolean/Solids models (BoxUCylTan1/2, BoltMBox1/2, extrudeHoles, BooleanBug.smp, boolean_trees.smp) under https://github.com/NVIDIA-Omniverse/usd-brep/tree/c9c5979f04cea8c38213004d4f04df0dcfb40d05/TestFiles/pt_TestFiles. Also the reproductions from issues #1 (mirrored union), #16 (tangent cylinder/box) and #10 (nondeterministic shell), each to become a Spare Kit regression test asserting a correct result or a typed error.
- Negative-knowledge checklist for our acceptance tests: solidcore's upstream-issues log entries UB-97, HANG-25, ND-1, ND-2, WR-52, WR-53, WR-69, WR-70 and WR-74 to WR-80 (https://github.com/tangency-labs/solidcore/blob/0a676a04c38229b10e924b5074cb6ade8ee897cf/plan/upstream-issues.md#L1056-L2870), plus the silent-failure sites listed in findings section 4.
- Real oracle (not from usd-brep): jyviko/ssi-corpus, 119 SSI cases with exact ground truth and a scorer, MIT OR Apache-2.0 (https://github.com/jyviko/ssi-corpus/tree/b04c8a6093a8b3505d865d1dd874ec3879ccb1f9). Also solidcore's difftest --ssi-corpus scorer design (https://github.com/tangency-labs/solidcore/tree/0a676a04c38229b10e924b5074cb6ade8ee897cf/crates/difftest/src/ssi). Adopt these as the SSI acceptance gate. Spare Kit must pass all 119, including tangent, singular and near-degenerate cases, where SMLib passes 81. SMLib may serve only as a secondary comparator on transversal cases.
- Do NOT take: any SMLib or solidcore SSI or boolean code, the tolerance constants, the repair loops (FixGaps, CloseIntersectionLoop, 10-pass caps), SmTemporaryBrepTransform recentring, or the consume-and-mutate API contract.

### Challenge 1

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- Wrong that 'the bounding boxes are not padded' (findings 1a). SmBSplineSurface::CalculateBoundingBox defaults bExpandPosBoxesByZoneTol3d=TRUE (source/SMLib/inc/SmBSplineSurface.h L309-317). When the flag is set it calls ExpandAbsolute by the face or B-rep tolerance, else SM_ZONE_TOL_3D=1e-5 (source/SMLib/src/SmBSplineSurface.cpp L797-807; SmMath.h L52). The surface cache computes node and Bezier-patch boxes through this call with the default flag (SmSurfaceCache.cpp L2693-2694, L2984, L2987, L5180). Upstream PR #17 and PR #18 both describe this 'collision padding', and PR #17 shows that the padding caused issue #1. The correct statement: the boxes are padded by a fixed absolute zone tolerance, not by the Hermite approximation error. Culling is still not certified, and the absolute pad adds another source of scale dependence.
- Silent-failure item 4(i) and the weakness 'SSI completeness guarantee is missing... Gauss-map separation test is dead code' get the effect backwards. m_bSeperateGaussMaps is only ever set TRUE: constructor L41, L1383, and SmTemporaryChangeValue at L1419 and L1441. So BranchIsGaussSeparate never runs, and every leaf pair takes the else branch, which saves one candidate per intersecting leaf pair (SmAdvSurfaceIntersector.cpp L1437-L1474). The dead branch is the one that would have skipped interior-curve search when the Gauss maps separate ('no need to do interior curve processing', L1407-L1416). Disabling it therefore makes the search more exhaustive, not less. solidcore rates WR-70 as 'performance and duplicate start points' (plan/upstream-issues.md L2778-2786 @0a676a0). Interior loops are actually missed because of leaf-midpoint sampling and the 5° and 2° skips (L1184, L1311). Those findings are correct.
- The SSI class table omits two classes from solidcore issue #17: 'high degree, many spans, rational' (6/6) and 'Yang, Jia & Yan 2023, published data' (5/6). The rows listed add up to 70/107, not 81/119. The 12 omitted cases include 11 passes.
- 'Misses about 32%' and listing 'near-coincident surfaces reported as coincident' as a defect leave out a caveat the corpus author gives. Issue #17 says near-degenerate is '24 | 11 (9 failures are below the 1e-5 zone tolerance)' and 'Below the 1e-5 zone tolerance, the following are reported as such and not counted as defects: angles of 1e-6 to 1e-8, offsets of 1e-7, and loops of 2e-6 or less'. WR-78 itself says 'this may be the intended tolerance semantics' (upstream-issues.md L2841-2848). Counting by the author's rule gives about 29 defects in 119 (24%), or 81/110 on cases above tolerance. The ssi-corpus README @b04c8a6 also reports SolidCraft at 47/119, so SMLib is clearly stronger than the only other engine measured. It still fails the iron-clad standard.
- Finding 1(d), 'The run hard-fails at 5,000 points', is wrong for the forward trace. TraceCurve raises SER(SM_ERR) at the 5000-point cap (SmSurfaceIntersector.cpp L5517-L5521). DoIntersection calls the forward trace as `if (TraceCurve(*pStartTSP, s3DCurves) == SM_SUCCESS)` with no else branch (L6538), and the failed start point is appended to m_vThroughPoints (L6622-L6626). Only the reverse-direction trace propagates its error, through SER at about L6557. The cap is therefore a silent path too.
- solidcore is described as 'about one third ported' with 'all intersect and boolean units partial or in progress'. The README says a third (README.md L29), but plan/PROGRESS.md at the pinned commit 0a676a0 says '236 / 261 units ported (114 green, 122 partial), 344,573 / 387,903 upstream lines (88.8%)'. intersect.surface_silhouette and intersect.surface_drop_lib_p2 are green (PROGRESS.md L242, L246). The SSI and boolean units themselves are partial, as the evaluation says.
- 'Upstream has no ... determinism tests' is not literally true. SmPyLib has repeat-call determinism tests: source/SmPyLib/tests/test_omni_solid.py L1469 test_create_circle_is_deterministic, L1815 test_create_curve_interp_points_deterministic, and L4951. None of them covers SSI or booleans, so the substantive point stands.
- Several solidcore entries are stated as established when solidcore itself marks them unconfirmed. UB-97 is 'Confirmed: no', and its shim does not run such inputs (upstream-issues.md L1085-1093). HANG-10, ND-1 and WR-52 are also 'Confirmed: no'. 'Hangs exist' also cites HANG-25, which needs a zero tolerance, which solidcore calls 'a bad argument from the calling code' (L1902-1909). These should be labelled potential or source-inspection findings.
- Issue count is off: at check time there were 18 open issues (#9 is closed) plus 2 open PRs (#17, #18), not 20 open issues. #19-#21 were filed on 2026-10-20.
- The SE_MSG framing ('ImprintIntersections errors are discarded in release builds') understates the problem. In debug builds SE and SE_MSG also 'continue processing' and only log (SmMessages.h L222-L233). The status is never propagated in any build.
- The Point3DClassify description ('first runs a boundary solve') does not apply on the boolean path. SmMerge step 3 calls Point3DClassify(sPnt, SM_EFF_ZERO, FALSE, ...) with bDoBoundaryIntersections=FALSE (SmMerge.cpp L3772), so only the ±X/±Y/±Z ray fire runs.
- Minor inaccuracies:
- SmApiValidateBooleanOperands also rejects SM_BO_EXTRACT_SEPARATE (SmApiBrep.cpp L186-L200), so it checks more than 'NULL or identical'.
- The 100·dTol stub lines for parallel cylinders apply only 'when axis interval is a point'. Otherwise full lines are returned (SmCone.cpp L2597-L2600).
- SmCone also dispatches extrusion and revolution partners (L2405, L2419).
- CloseIntersectionLoop's status is consumed to set bMadeNewTopology=FALSE but is not propagated (SmMerge.cpp L2971-L2972).
- THIRD_PARTY_NOTICES.md also lists NVIDIA bootstrap scripts (MIT, L442) and an optional GUI dependency group that includes LGPL v3 (L478). These are not SMLib, but tools/smlib_gui must stay out of any port.
- Missed silent path: PiecewiseMerge drops IntersectInsertRelate's return value entirely: `m_vTI.IntersectInsertRelate();` at SmMerge.cpp L394. This is the merge path used by shell and offset, and the issue #10 trace SmMerge.cpp:672 lies inside PiecewiseMerge (L341-L794).
- Missed silent path: ManifoldBoolean detects input tolerance violations and does nothing with them: `if (bViolatedTolerances) { SE(SM_ERR); }` at SmMerge.cpp L2625 and L2637, a no-op in release. The preceding ValidateAndUpdateTolerances(TRUE, ...) also changes the inputs' tolerances, which is another case of input mutation.
- Missed evidence strengthening the determinism finding: the clock-seeded RNG is reachable from booleans. SmBrep::TestClosureSplitRegion (SmBrep.cpp L27845; IsInnerShell call at L27997) calls SmTopologyTraverser::IsInnerShell (L1239), which uses at least 3 samples per face and calls GetPointsInFace at L1279. GetPointsInFace calls srand(time(NULL)) whenever more than one point is requested (SmFace.cpp L21179-L21184).
- Missed unit mix-up: ComputeStepSize documents 'a parametric step size' but seeds it with 100.0 * m_dThisApproxTol3d, a 3D distance (SmSurfaceIntersector.cpp L2930-L2945).
- Provenance, which strengthens the evaluation's caution. IntegrityWare's SOLIDS++ history page (integrityware.com/products/SOLIDS++/history.html) says a 1998 joint marketing agreement between IntegrityWare, GeomWare and SMS gave all three rights to market the combined kernel, and that IntegrityWare is exercising that option. The source still embeds GeomWare-era NLib code (NL_*.h/.cpp, gw_SURFACE types). All 490 SMLib and SM_API files carry the NVIDIA Apache-2.0 SPDX header, so the chain of title for relicensing is still unverified.

**Argument:** I checked the evaluation against usd-brep c9c5979, solidcore 0a676a0, ssi-corpus b04c8a6 and the upstream issues and PRs. Most of its specific source claims are correct, and STUDY is still the right verdict despite the errors listed below.

Confirmed:
- File sizes and line ranges.
- The SE/SE_MSG void macros (SmMessages.h L627-628).
- The ImprintIntersections status being overwritten (SmTopologyIntersector.cpp L1233-1264).
- SM_REF1 on the shell-vertex and wire-edge statuses (L1627-1646).
- The 'too difficult an edge' skip (SmMerge.cpp L3208).
- The sFaceEdges[0] retry bug (L3764).
- The sIvl1/sIvl2 mix-up (SmCurve.cpp L6597).
- The MakeDupin k2/k2 conic (L1120-1122).
- aSingDirections[8] with no bounds check.
- The 5°/2° skips.
- Step underflow at 1e-10 breaking out with success.
- TraceCurve failures turned into through points.
- Recentring past 1000 units.
- The 10/10 pass caps.
- Ray fire that takes the first successful axis.
- Pointer-keyed unordered_maps.
- srand(time).
- The false 'exact NURBS ... no approximation' doc claim (booleans.md L11).
- The tangent-avoidance advice (L100).
- 56 ssi_*.smp fixtures.
- No exact or robust arithmetic anywhere in the source.
- Issues #1, #10 (including offset_brep and within-process variance in a comment) and #16.
- The Apache-2.0 SPDX header on all 490 SMLib/SM_API files.

Factual errors:
- The surface-cache boxes are padded by a fixed zone tolerance. The upstream PRs #17 and #18 show that this padding caused issue #1.
- The dead Gauss-map test makes interior-loop search more exhaustive, not less. solidcore classes it as a performance/duplicate issue.
- The corpus table omits 12 cases.
- The 32% figure ignores the corpus author's own exclusion of 9 cases below tolerance. Counted the author's way the defect rate is about 24%.
- The 5000-point 'hard fail' is swallowed by DoIntersection.
- solidcore's port status is out of date: PROGRESS.md at the pinned commit reports 88.8% of lines, with 114 units green.
- Several UB/HANG/ND entries are presented as established when solidcore marks them unconfirmed.

These errors make the evaluation somewhat harsher than the evidence supports. None of them rescues SMLib against Spare Kit's principles. I also found further silent paths it missed:
- PiecewiseMerge drops IntersectInsertRelate's status (SmMerge.cpp L394).
- Tolerance violations on the inputs are ignored (L2625, L2637).
- The clock-seeded RNG is reachable from booleans through TestClosureSplitRegion, IsInnerShell and GetPointsInFace.

The strongest case for the opposite verdict is DIFFTEST_ORACLE. SMLib is the only permissively licensed, full NURBS B-rep kernel with booleans; OCCT is LGPL, which allows method study only. It passes 59/59 corpus cases in the transversal, boundary, coincident-region, seam/pole and high-degree classes. Every curve it returns lies within 1.9e-6 of the exact surfaces. It scores well above the other engine measured (81 vs 47 of 119). No exact-truth corpus exists for booleans, and an independent kernel is the cheapest broad cross-check.

That case fails as a primary role, for three reasons:
- SMLib returns wrong results while reporting success, with manifold=True (issue #1).
- It is nondeterministic on the boolean path itself (issue #10, srand(time) and pointer-hash order). An oracle whose answers change between runs and fail silently gives only weak evidence when it agrees.
- Its architecture rules out PORT_CODE and PORT_METHODS: approximate marching, a mutable pointer graph, repair loops, and pointer identity instead of persistent naming. solidcore keeps the same algorithms and failure semantics on purpose, so PORT through it is ruled out too.

STUDY therefore stands. The secondary comparator role should be widened from 'transversal only' to every class where SMLib scored 100%. It should run in fresh processes with repeated runs, and it should never be authoritative.

## operations-io

**Commit sha:** NVIDIA-Omniverse/usd-brep c9c5979f04cea8c38213004d4f04df0dcfb40d05 ("Release 1.0.0.", 2026-10-09; the repo has only 3 bot commits). tangency-labs/solidcore 6a019c57131d5fee6e57ab7dc4a62201a32e0216 (2026-10-20).

**Findings:** Base for all links: U = https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/

REPO FACTS. Created 2026-10-08. About 1,750 stars. The README says the project is "not accepting contributions". Layers: SMLib (source/SMLib, 40.2 MB of C++; solidcore counts about 388k upstream lines) → SM_API, a C-style C++ wrapper using SmApiStatus codes → SM_API_USD / BREP_SM_USD / BREP_USD_DATA → SmPyLib, a pybind11 module `_omni_solid` packaged as the `usd_brep` wheel. Origin: Solid Modeling Solutions (founded 1998, Blomgren/Presti), which bought NLib (Piegl/Tiller, GeomWare) in 2001; NVIDIA acquired SMS in May 2022 (Wikipedia; develop3d 2026-10-13). SMS also sold IGES, STEP, VDAFS, SAT and OpenNURBS translators. None of those translators is in this release.

1. FILLETS, BLENDS AND CHAMFERS
- Entry points (SM_API): SmApiFilletEdges, SmApiFilletEdgesPerEdge, SmApiVariableRadiusFillet, SmApiSurfaceSurfaceFillet, SmApiFilletPreview, SmApiSetBevelCorners (U+source/SM_API/inc/SmApiBrep.h#L432-L488), plus all-edge SmApiChamferFillet, SmApiCircularFillet and SmApiRemoveFillet (U+source/SM_API/inc/SmApiFillets.h#L24-L38).
- The design factors cleanly into four orthogonal parts:
  - Solver/offset law: SM_FS_CONST_RADIUS, VARIABLE_RADIUS, CONST_DIST (chamfer), CURVE_BASED, SURF_SURF, CONST_RADIUS_ASSISTED.
  - Cross-section generator: LINEAR, CIRCULAR, BLEND_CURVE with G1/G2/G3 and a "thumbweight" (U+source/SMLib/inc/SmFilletSolver.h#L47-L65).
  - Continuity.
  - Corner strategy.
- Rail computation is the classical rolling ball:
  - Each support surface is wrapped in an evaluator-based SmOffsetSurface: S(u,v) + d·n, extendable outside its domain (U+source/SMLib/inc/SmOffsetSurface.h).
  - The intersection curve of the two offsets is marched with Newton steps on [u1 v1 u2 v2]. Jacobian rows are F−G (U+source/SMLib/src/SmFilletStandardSolver.cpp#L190-L260).
  - StepSolve clips steps to the extended UV boxes (U+source/SMLib/src/SmFilletSolver.cpp#L5145).
  - Roll-over handling: TestRollOver L5714, CreateCliffRollOverSideFG L6518, ProcessFullRollOver L6906, ProcessRollOver L7161.
  - Pluggable self-intersection handlers (curve-blend or surface-blend; SmFilletSolver.h L519-690).
- Analytic fast paths: a constant-radius fillet becomes an exact cylinder, cone or torus when CanMakeAnalytic() holds (SmFilletSolver.h#L206-L213; .cpp L432 cylinder, L662 cone, L860 torus).
- Corner taxonomy: OPEN, DEGENERATE, 1x1, 2x2, 3x2 mixed-convexity, 4x3, Nx1, Nx1-closed, Nx2, NxN, NxN convex, NxN concave (U+source/SMLib/inc/SmFilletGeom.h#L80-L95; classes in SmFilletCorner.h#L239-L616).
  - Bevel vs blend semantics are documented at SmFilletCorner.h#L78-L110.
  - Corner patches are 3-sided SurfaceFill patches plus end caps. Only G1 is guaranteed across corner patches, whatever continuity was requested.
- Other blend machinery:
  - Variable radius through SmFilletLaw: linear and B-spline laws, chained along tangent-continuous edge sequences (SmFilletExecutive.h L446-L450, L685-L899).
  - Over-large radii are converted to corners (TestAndConvertBigRadFillet, L401).
  - Fillet recognition: SmFace::IdentifyFillet (SmFace.cpp L10677) drives remove_fillet.
- Structured error record SmFilletErrorInfo: error type, location point, vertex, edge, side and end face, corner type, fillet surfaces and curves (SmFilletExecutive.h#L36-L167; enum SmFilletGeom.h#L54-L73).
- Chamfer: SmConstantDistanceFS with a linear generator. The public API has only a symmetric "radius" (= chamfer width). The solver stores two radii (m_dFilletRadii[0/1]), but no asymmetric or distance-angle chamfer is exposed (unverified internally).
- Validity after filleting:
  - SmApiCircularFillet calls StitchAndOrient() afterwards, with the in-code comment "This operation requires stitching for some reason / DoFilleting should be examined so that it results in a valid brep" (U+source/SM_API/src/SmApiFillets.cpp#L169-L175). The return value is ignored in the fillet paths (SmApiBrep.cpp#L2079, #L2127, #L2174).
  - Validation runs only under the debug macros SM_ASSERT_VALID / SM_CHECK_VALIDITY.
  - Operations mutate the B-rep in place, with no rollback.
  - remove_fillet returns success and removes nothing (KNOWN_ISSUES.md#L10-L14; issue #6).
- Open GitHub issues on fillets:
  - #2: a foreign edge corrupts the other Brep.
  - #4: invalid BrepArray after fillet.
  - #8: a 1 mm box does not return within 90 s.
  - KNOWN_ISSUES L45-L49: inside-out triangles next to fillets.

2. OFFSET, SHELL, THICKEN AND DRAFT
- SmOffsetExecutive pipeline (U+.agents/skills/smlib-kernel-operations/references/offset-shell-internals.md):
  - Per-face offset surfaces come from CreateOffsetSurface with approximation tolerance tol/10. Analytic type is then recovered with CopyAndAddAnalytics. Optional self-intersection detection uses a global surface/surface intersection (U+source/SMLib/src/SmOffsetGeometryCreation.cpp#L118-L250).
  - Convex joins are extend-and-intersect or fillet-like faces.
  - With self_intersection=FALSE, concave overlaps are left unresolved (offset-shell-internals.md#L39-L42).
  - The pieces are then merged and a region is selected.
- Shell = offset followed by a Boolean difference (SmApiShellBrepFull, SmApiBrep.cpp#L2350).
- Thicken (sheet bodies) = ruled lamina faces (create_solid).
- No face-draft (taper existing faces) operation exists; only SmApiCreateDraftSweep and SmApiCreateTaperExtrude (SmApiBrep.h#L232, #L315).
- Verified derivative bugs in SmOffsetSurface: third-derivative coefficient 3.0 should be 1 (L1468, L1479), and Buuv is used in place of Buvv and Bvvv (L1482, L1491) (U+source/SMLib/src/SmOffsetSurface.cpp#L1464-L1492). These derivatives feed G2/G3 blend construction.
- Open issues on shell/offset:
  - #10: SmApiShellBrepFull gives different results across fresh processes, and reports success with non-manifold or wrong-volume bodies.
  - #13: the process terminates at an inward cavity collapse.
  - #21: a shell of a filleted box tessellates up to 2 mm off its surfaces.
- solidcore's defect log also records offset hangs HANG-21, -26 and -27 (surface-of-revolution offset taking minutes).

3. TESSELLATION (SmTess; U+source/SMLib/inc/SmTess.h#L742-L772)
- Phase 1 tessellates every edge once and splits the working copy's edges at the sample points, so every sample becomes a topological vertex.
- Output maps each SmVertex to exactly one 3D PolyVertex (GetOrCreatePolyVertex, U+source/SMLib/src/SmTess.cpp#L6939-L6990). The mesh is therefore crack-free by shared index; the first face's evaluated point wins.
- Phase 2, per face:
  - A Bezier-patch quadtree (SmTessSrfCache/SmTree) is refined by control-net "pseudo-box" slab thickness (the chord test, U+source/SMLib/src/SmSurfaceCache.cpp#L3575-L3600), control-polygon end-tangent angle, maximum side, aspect ratio and minimum sizes.
  - Leaves are classified in, out or on by loop containment.
  - Polygons are triangulated in UV by convex-corner clipping (U+source/SMLib/src/SmTess.cpp#L8520-L8555) or by advancing front.
  - A view-dependent silhouette driver is also available.
- Default chord tolerance is 0, meaning the chord test is off; defaults are angle 25°/25° only (U+source/SM_API/inc/SmApiTessellationParams.h#L17-L35).
- For non-B-spline surfaces the chord test is only sampled. The docs disclaim maximum edge length and aspect ratio as hard limits.
- Per-face failure reports (face index, stage, status) and partial-output opt-in are well designed (U+.agents/operations/tessellation.md).
- Open issues on tessellation tolerance: #7, #12 (about 35× coarser than the chord tolerance, silently), #20 and #21.

4. MASS PROPERTIES
- SmFace::ComputePreciseProperties turns trimmed-face integrals into line integrals along the oriented UV trim curves (Green's theorem). Each line integral uses adaptive Cash-Karp Runge-Kutta, with a property-group mask for area, first and second moments, and volume (U+source/SMLib/inc/SmFace.h#L68-L80; U+source/SMLib/src/SmFace.cpp#L6027-L6140).
- Solid volume and moments come from the divergence theorem about an origin.
- Relative accuracy is clamped to [1e-4, 1e-1] (U+source/SM_API/src/SmApiQueries.cpp#L880-L964).
- Measured errors:
  - Up to 1.4% area error on faces trimmed by circles (KNOWN_ISSUES.md#L53-L57).
  - ±3.4% volume error depending only on orientation (issue #19).
  - Area error that plateaus instead of converging (issue #11).
- SmIntegrator::IntegrateIt returns 0.0 with SM_SUCCESS when integration fails (U+source/SMLib/src/SmIntegrator.cpp#L78-L122). It is used for curve arc length (SmCurve.cpp L14697) and surface area (SmSurface.cpp L5270, L5356). solidcore confirms this as WR-94.

5. FILE I/O
- Native .smb/.smp (ASCII and binary) through SmApiReadBrepFromFile, SmApiWriteBrepToFile and SmApiReadPartFromFile (U+source/SM_API/inc/SmApiImportExport.h). solidcore lists 26 PSIRT-class crafted-file memory bugs in these readers (UB-1..6, UB-75..77, UB-89, UB-91, UB-92, UB-164, ...).
- IGES: only NLib entity-to-NURBS converters exist (types 100, 102, 104, 106, 108, 112, 114, 118, 120, 122, 124, 126, 128; U+source/SMLib/inc/NL_Iges.h). There is no IGES file reader or writer; SmBrep.cpp#L19455 refers to a missing smiges_ReadIges.
- STEP: no reader or writer. SmSTEPSurface only evaluates analytic surfaces in STEP parameterization.
- Press reports of "STEP/IGES/JT via bundled OpenCASCADE" are contradicted by the repo at this commit. OCCT_BREP_IMPORT and OCCT_BREP_EXPORT are standalone parsers and writers of OCCT's .brep format and "link no kernel SDK" (U+source/OCCT_BREP_IMPORT/premake5.lua#L3-L5). A "PRC front end" is referenced in that file but not shipped.
- Issue #5: usd_to_occt writes invalid .brep files.

6. USD INTEGRATION
- Codeless typed schema BrepArray (a Gprim) packs many B-reps as structure-of-arrays (U+source/schema/omniSolid/resources/schema.usda#L21-L178):
  - Arrays for brep, region, shell, faceuse, face, loop, edgeuse, edge, wireEdge and vertex, each with count prefix sums.
  - Radial-edge non-manifold topology: 2 faceuses per face, and an edgeuse radial cycle through nextRadialEUIndex.
  - Geometry lives in multiple-apply API schemas (NURBS, line, circle and ellipse curves; NURBS, plane, sphere, cylinder, cone and torus surfaces), matched to entities by occurrence order of the type tokens.
  - Analytic surfaces use STEP parameterization.
  - There is one brep:intersectTol3d per B-rep, so per-entity tolerances collapse to one value (AGENTS.md L183-L184).
  - Materials attach through GeomSubset face indices. There are no persistent IDs.
- The full contract, with each rule labelled stated, derived or undecided, is at U+.agents/skills/usd-brep-schema/references/schema-contract.md.
- Rendering: the hdUsdBrep Hydra procedural tessellates BrepArrays into transient meshes. SmApiUsdTessellateBrepArray uses a private context per call (U+source/SM_API_USD/inc/SmApiUsdTessellate.h).
- Validation: a Python rule-numbered validator (rules BA.000–BA.7xx, 400 KB) plus a C++ geometry validator.
- The vendored AOUSD UsdSolid draft proposal is not used at runtime.
- Issue #14: export writes cylinders as cones with semiAngle=0, which the bundled validator rejects. Malformed BrepArray data can crash import with an out-of-bounds read (KNOWN_ISSUES.md#L61-L65).

7. PYTHON API
- Opaque handles held with py::nodelete. Lifetimes belong to a process-global SmContext with no teardown, so Python garbage collection never frees kernel objects (U+source/SmPyLib/src/SmPyTypes.cpp#L384-L389).
- Docstring contracts state "Pure / Consumes / Mutates" (AGENTS.md).
- Errors are RuntimeError carrying the SM_ERR name plus a text kernel trace collected in a thread-local trail. TessellationError adds a structured per-face failures list.
- .pyi stubs are provided, plus a separate `_smlib_dev` debug module.

8. CONCURRENCY
- The process is the concurrency unit. The global SmContext holds unlocked mutable mark counters, and calls from several threads are forbidden (U+.agents/operations/concurrency.md).

9. SOLIDCORE STATUS FOR THESE AREAS
- Its README still says "about a third ported", but its PROGRESS.md at 6a019c5 reports 234/261 units ported (114 green, 120 partial; 88% of lines).
- The fillet executive, corners and solver parts p2–p5 are still open PRs. query.tess, mass.integrator, offset_* and heal.face_props are partial. nlib.iges is green.
- OCCT, USD and Python layers are explicitly out of scope. The README says nothing works end to end yet.

**Robustness exactness:** - Arithmetic: plain IEEE double throughout. A grep of SMLib finds no exact or adaptive predicates and no interval arithmetic.
- Tolerances are fixed absolute epsilons, so they depend on model scale: SM_EFF_ZERO = 1e-12 and SM_ZONE_TOL_3D = 1e-5 (U+source/SMLib/inc/SmMath.h#L38-L52). Effects seen: issue #8 (fillet does not return on a 1 mm box) and solidcore HANG-26/27 (offsets at large distances run for minutes or exhaust memory).
- Fillet rails come from Newton marching on offset-surface intersections. Degenerate cases are handled by ad-hoc branches (roll-over, cliff, big-radius-to-corner, degenerate corner), not by a classification with guarantees.
- UV triangulation uses floating-point convex-corner tests. The TriangulateFace recursion depth limit is 15, per a comment in SmTess.h.
- Validity is enforced only in debug builds (SM_ASSERT_VALID / SM_CHECK_VALIDITY around every SM_API op, e.g. SmApiBrep.cpp#L2083). Release builds return SM_SUCCESS without validating.
- Failure contracts are explicitly weak. errors.md says "The status alone does not guarantee rollback, unchanged inputs, or empty outputs". Fillets mutate in place.
- Confirmed silent or wrong results:
  - IntegrateIt returns 0 as success on failure (SmIntegrator.cpp#L112-L120; solidcore WR-94).
  - remove_fillet reports success after doing nothing.
  - heal_brep reports success when a repair failed (KNOWN_ISSUES L16-L20).
  - Shell returns success with non-manifold or wrong-volume bodies (#10).
  - Tessellation ignores the chord tolerance (#12, #20, #21).
  - Mass properties are off by ±3.4% (#19), 1.4% (KNOWN_ISSUES) and plateau (#11).
  - Offset-surface third derivatives are wrong; I verified this in source at SmOffsetSurface.cpp L1468, L1479, L1482, L1491.
  - Fillet with a foreign edge corrupts the other B-rep (#2).
  - Shell terminates the process (#13).
- solidcore's audit (plan/upstream-issues.md at 6a019c5) lists 176 UB, 34 HANG, 3 nondeterminism, 120 wrong-result and 4 docs entries across SMLib, 26 of them marked PSIRT (crafted-file memory bugs). Relevant here: UB-136 and UB-159 (fillet geometry), UB-202, UB-203, UB-186 and HANG-34 (circular and blend cross-section generators), UB-171, UB-173, UB-187 and WR-160..165 (offset surface), HANG-21/26/27 (offset).
- One improvement: NVIDIA recently hardened face property integration to propagate failure instead of dropping a boundary, with retries over 5 initial step sizes (SmFace.cpp#L6106-L6122). The scalar wrapper still hides failures.

**Determinism:** Not deterministic, and not designed to be. Verified in source:
- (a) `srand((unsigned)time(NULL))` reseeds the C RNG from the clock in SmFace::GetPointsInFace (U+source/SMLib/src/SmFace.cpp#L21183) and in SmShell::IsInnerShellOf (SmShell.cpp#L888, #L905).
- (b) A function-local static is fixed by the first call: `static double sdStaticStep = (pointType == TANGENT) ? 1e-3 : 1e-5` in SmFilletSolver::ComputePointValues (U+source/SMLib/src/SmFilletSolver.cpp#L3604). Fillet results therefore depend on process history.
- (c) Pointer-keyed std::unordered_map (U+source/SMLib/inc/SmMapPtrToPtr.h#L42) and SmRelation iterate in address-hash order. solidcore observed this order leaking into SmOffsetExecutive results (ND-2).
- (d) No floating-point determinism flags in premake (no -ffp-contract=off or /fp:precise). macOS clang defaults to FP contraction, so results across platforms differ by construction.
- (e) Global mutable SmContext: single-threaded only, with "the process is the unit" of concurrency. TBB parallel paths exist but are compiled out.
- (f) No WebAssembly build exists.

Seen in practice: issue #10 (shell volume differs between fresh processes from one input) and issue #21 (mesh changes from process to process).

solidcore removes (a)–(c): a seeded kernel.rng, BTreeMap or insertion order, no statics, and a C++ reference built with -ffp-contract=off. It targets wasm32 and keeps the upstream operation order to stay bit-faithful. ND-3 (the fillet static) is planned but the fillet units are not merged yet.

**Tests and quality:** C++ tests (tests/prog_test):
- smFilletTest.cpp (232 KB), smOffsetTest.cpp, smTessTest.cpp. Mostly read-file, fillet, then assert edge counts, with history comments such as "was 57 now 45" and "USEDTOBE = 30". Validity asserts are active only in debug builds.

Python tests (source/SmPyLib/tests/test_omni_solid.py, 400 tests):
- Fillet tests are smoke checks (face_count > 6). test_remove_fillet passes even though nothing is removed (L3052-L3100).
- Mass-property tests check boxes and spheres against closed forms.
- Shell tests check that the source survives failure.

Fixture corpus, Apache-2.0, native .smb, under TestFiles/pt_TestFiles: Fillet 98 files (11.7 MB), Shell 25, Tess 8, Defeature 12, Solids 38, Boolean 6. No STEP or IGES fixtures.

Code quality:
- Hungarian notation and huge files (SmBrep.cpp 1.7 MB, SmSurface.cpp 1.6 MB, SmTess.cpp 689 KB).
- Inline debug-drawing blocks under SM_DEBUG_CODE.
- Agent documentation is excellent and candid: .agents/operations/*.md, skills, errors.md, KNOWN_ISSUES.md, and a schema contract that labels each rule as stated, derived or undecided.

Bug intake: 21 public issues in 12 days, mostly in this subsystem (fillet, shell, tessellation, mass properties, export). All are still open except #9.

solidcore:
- Difftest compares the Rust port against the C++ to 1e-9 relative with identical topology, unit by unit.
- Its upstream-issues.md is the most thorough defect audit of SMLib available.
- It deliberately keeps upstream wrong results for parity (e.g. WR-160..164 "Rust port: Kept"; WR-94 "retained for parity").

**Legal:** Licences:
- usd-brep: Apache-2.0. Every file carries "SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES" and Apache-2.0. repo.sh, repo.bat and repoman_bootstrapper.py are MIT.
- Original Solid Modeling Solutions, GeomWare and Piegl/Tiller copyright lines have been removed. The only remnant is the GW_EXPORT (GeomWare) macro at U+source/SMLib/inc/nurbs.h#L19-L23.
- Chain of title: NVIDIA acquired SMS in May 2022, and SMS bought NLib in 2001, so NVIDIA's right to relicense is plausible. It is unverified, and the owner should treat it as an assumption.

RED FLAG, Numerical Recipes:
- U+source/SMLib/inc/SmIntegrator.h#L5-L8 says the integration "algorithms are taken from Numerical Recipies in C".
- SmIntegrator.cpp matches NR's own code, not just its method:
  - odeint/rkqs constants: SAFETY 0.9, PGROW -0.2, PSHRNK -0.25, ERRCON 1.89e-4, MAXSTP 10000, TINY 1e-30.
  - bsstep constants: SAFE1, SAFE2, REDMIN, SCALMX.
  - The polint variable names (ns, dif, dift, ho, hp, w, den, c, d), and ak2..ak6 from rkck (U+source/SMLib/src/SmIntegrator.cpp#L20-L40, #L128-L170).
- More NR-derived code: SmMatrix.cpp L654 (Crout LU), L992-L1132 (Jacobi eigen), L1708 and L2112; NL_Math.cpp L2514, L8447, L8680, L9369, L11715, L11814.
- NR's code licence does not allow redistribution in source form. Whether NVIDIA or SMS held a licence is unknown, so the Apache relicensing of these files is doubtful.
- Rule for us: never port or translate SmIntegrator, SmMatrix (LU/Jacobi) or the NR-marked NL_Math routines. Re-implement Cash–Karp, Romberg, Neville, LU and Jacobi from primary literature.
- solidcore has ported SmIntegrator (crates/solidcore/src/mass/integrator.rs) and does not mention NR anywhere in plan/, so it inherits this risk.

Other third-party material:
- Graphics Gems I routines (SmGeomUtility.cpp L6087-L6127): permissive.
- OpenUSD: Tomorrow Open Source Technology License 1.0 (Apache-2.0 with a changed trademark clause). oneTBB: Apache. pybind11: BSD. These are runtime dependencies, not SMLib.
- The optional GUI pulls PySide6/shiboken6 (LGPL/GPL). It is not shipped. Do not read it.
- OCCT is not linked or vendored. The .brep parser and writer are NVIDIA's own Apache code, but stay clear of OCCT source itself (LGPL: method study only).
- The vendored UsdSolid and cad_geometry proposals fall under the OpenUSD Technical Proposal Supplemental Terms: design study only, and the README says not to edit them.

Patents: the repo says nothing about patents (a grep finds nothing). The Apache-2.0 §3 patent grant covers NVIDIA's contributions. Rolling-ball and offset-intersection fillet methods date from the 1980s and 1990s (unverified for specific claims).

solidcore: Apache-2.0 derivative work. Its NOTICE and ATTRIBUTION.md carry NVIDIA's notice, and each file has a provenance header. Its CI is Blacksmith-sponsored.

Before any port the owner must check:
- (1) Nothing NR-derived is taken.
- (2) The Apache NOTICE and per-file attribution are kept for anything translated.
- (3) The NLib/SMS title chain.
- (4) No OCCT, LGPL or GPL code is read.

**Alignment:** Checked against each Spare Kit principle:
- Iron-clad, never invalid, no silent failures: FAILS.
  - Validity checks exist only in debug builds.
  - A fillet is "stitched for some reason" afterwards (SmApiFillets.cpp L169-L170).
  - IntegrateIt turns failure into 0.
  - Shell reports success with non-manifold bodies.
  - remove_fillet reports success after doing nothing.
  - Offset with self_intersection=FALSE leaves overlaps unresolved.
- Exact/robust predicates: FAILS. None exist; fixed absolute epsilons.
- Structured errors: PARTIAL. Integer SmStatus plus a free-text trail. Two good shapes worth copying: SmFilletErrorInfo (location, entities, corner type) and the tessellation per-face {index, stage, status} report.
- Deterministic (native and wasm, any thread count): FAILS. Clock-seeded rand, a first-call static, pointer-hash order, no FP-contract control, global context, no wasm build.
- Persistent naming: ABSENT.
  - Identity is pointers.
  - SmTrackTopologyChanges records added and removed pointers and its own comment admits pointer-reuse aliasing (U+source/SMLib/inc/SmTrackTopologyChanges.h#L35-L88).
  - USD materials bind through positional face indices.
  - solidcore swaps pointers for arena typed ids, which is index identity and still not persistent naming.
- Immutable data and pure operations: FAILS. In-place mutators with no rollback (issue #2 corrupts another B-rep), a process-global mutable context, and py::nodelete objects that are never freed.
- No unsafe: not applicable to C++; solidcore's audit counts 176 UB entries kernel-wide.
- Full scale: FAILS. Absolute tolerances break at 1 mm (#8) and at large distances (HANG-26/27).
- Readable by AIs and people: MIXED. The documentation is excellent. The code is a 40 MB Hungarian-notation C++ codebase with megabyte files and inline debug drawing.
- Fully headless: YES. Graphics are optional callbacks.
- Licence: Apache, compatible with MPL-2.0 only if the notices are kept, and the NR-derived parts are excluded.

What would have to change to make any of it fit: essentially everything architectural.
- Pure operations on immutable B-reps that return a new B-rep or a typed error.
- Persistent names derived from the generating operation (rail, cap and corner faces named from the source edge, vertex and the feature).
- Certified tolerances relative to model scale, with exact predicates for topological decisions.
- Validity checks always on in release.
- Deterministic ordering with no hidden state.
- FP-contract control so native and wasm results are bit-identical.

So the parts of this subsystem worth taking are methods and taxonomies, not code. solidcore is an architecture-faithful port that keeps upstream bugs for parity, so it inherits these misalignments. Its lint rules (no unsafe, no panics, bounded recursion, ordered maps, wasm) align with ours, but its kernel shape does not.

**Strengths**

- The fillet problem is cleanly factored into offset law (constant or variable radius, constant distance) × cross-section generator (linear, circular, G1–G3 blend with thumbweight) × continuity × corner strategy (SmFilletSolver.h L47-L65). This is a good vocabulary for our blend API and test matrix.
- The fillet corner taxonomy (1x1, 2x2, 3x2-mixed, 4x3, Nx1, Nx1-closed, Nx2, NxN convex/concave, degenerate, open) has explicit bevel vs blend semantics (SmFilletGeom.h L80-L95; SmFilletCorner.h L78-L110). It encodes about 25 years of production experience about where vertex blends fail.
- Constant-radius fillets come out as exact analytic cylinder, cone or torus surfaces when the configuration allows (CalcCylinder/Cone/TorusFilletGeom). This is essential for architecture and BIM, where most blends are plane/plane or plane/cylinder.
- Cases a naive rolling-ball implementation misses are handled explicitly: roll-over (cliff and full), big radius converted to a corner, radius laws chained along tangent edge sequences, B-spline radius laws, fillet preview, surface/surface fillets, and fillet recognition for defeaturing.
- Tessellation is crack-free by construction. Edges are sampled first and each sample is promoted to a shared topological vertex, so adjacent faces share mesh vertices by index (SmTess.h L742-L772; SmTess.cpp L6939-L6990).
- The surface chord criterion uses the Bezier control-net slab thickness (convex-hull property), a conservative and certifiable bound for polynomial patches (SmSurfaceCache.cpp L3575-L3600).
- Tessellation diagnostics are good: per-face failure index, stage and status, partial output only as an explicit opt-in, and a report with manifold edge-incidence flags.
- Mass properties use Green's-theorem boundary integration over the UV trim curves, with a property-group mask so area-only queries skip the moment work.
- The USD BrepArray schema is a compact structure-of-arrays encoding of non-manifold radial-edge B-reps, with STEP-parameterized analytics. Its contract documents each rule as stated, derived or undecided, and a rule-numbered validator (BA.xxx) goes with it. It is a realistic interop target, aligned with the AOUSD UsdSolid proposal.
- Documentation is candid: KNOWN_ISSUES.md, errors.md and the per-operation guides. Mutation contracts (Pure / Consumes / Mutates) are stated per Python function.
- Apache-2.0 fixture corpus: 98 fillet, 25 shell, 12 defeature, 8 tessellation and 38 solid models, plus 21 public issues with minimal repros, all usable as regression scenarios.

**Weaknesses**

- Silent failures: IntegrateIt returns 0 as success (SmIntegrator.cpp L112-L120), and this feeds curve length and surface area. remove_fillet does nothing and reports success. heal_brep hides failed repairs. Shell reports success with non-manifold or wrong-volume bodies (#10).
- Fillet output is not valid by construction. A post-hoc StitchAndOrient is needed 'for some reason', its return value is ignored, and validity checks exist only in debug builds. Issue #4 shows an invalid BrepArray after a fillet.
- Mutation in place with no rollback. A foreign-edge fillet corrupts the other B-rep (#2). The failure contract explicitly does not guarantee unchanged inputs.
- Offset-surface third derivatives are wrong (verified at SmOffsetSurface.cpp L1468, L1479, L1482, L1491). They feed G2/G3 blend and offset work.
- Nondeterminism: clock-seeded srand (SmFace.cpp L21183, SmShell.cpp L888/L905), a first-call static step in the fillet solver (SmFilletSolver.cpp L3604), pointer-hash iteration order, and no FP-contract flags. Results differ between processes in practice (#10, #21).
- No robust predicates. Fixed absolute epsilons (1e-12, zone 1e-5) make behaviour depend on model scale: a fillet hangs on a 1 mm box (#8), and offsets take minutes at large distances (HANG-26/27).
- Tessellation is not error-bounded by default: chord tolerance 0 means off. Even when set, it is missed on skins, boolean results and shells (#7, #12, #20, #21). Triangles next to fillets can be inside out.
- Mass properties accuracy is clamped to at best 1e-4 relative. Measured errors are 1.4% (circle-trimmed faces), ±3.4% depending on orientation (#19), and area that plateaus (#11). There are no exact closed forms for analytic faces in the default path.
- I/O gaps: no STEP or IGES file translators. The SMS translators were not released despite press claims. The native .smb readers carry 26 PSIRT-class crafted-file bugs (solidcore audit). Exporters write invalid files (#5, #14), and malformed BrepArray data can crash import.
- No face-draft operation, no asymmetric or distance-angle chamfer in the API, and corner patches guarantee only G1.
- No persistent naming. Change tracking is pointer-based with acknowledged pointer-reuse aliasing, and USD material subsets bind by positional face index.
- Process-global unlocked context, single-threaded only, and Python handles that are never freed (py::nodelete).
- Legal: Numerical Recipes-derived code in SmIntegrator, SmMatrix and NL_Math, relicensed as Apache by NVIDIA. solidcore ported SmIntegrator without flagging this.
- Upstream does not accept contributions, so partnering on fixes is limited to issue reports.

**Verdict:** STUDY

**Verdict reason:** No code in this subsystem meets the bar for porting:
- The architecture is a mutable pointer graph behind a global context, with tolerance-only arithmetic and debug-only validity. That is the opposite of iron-clad, pure, deterministic and persistently named.
- Defects are verified, and many are open: silent zero integrals, a stitch-after-fillet workaround, process-dependent shell results, tolerance-ignoring tessellation, ±3.4% mass properties, and wrong offset derivatives.
- The integration and matrix code is Numerical Recipes-derived, so it is legally unsafe to translate.

As a differential oracle it is unreliable in exactly this subsystem. Closed-form analytic results and our own invariants are better oracles. usd-brep is useful only as a secondary "does it succeed, and how many faces" comparator and as a source of hard fixtures.

Its value is method and design knowledge, which we should re-derive inside our architecture:
- The fillet factorization and corner taxonomy.
- The analytic fillet special cases.
- The roll-over and big-radius case catalogue.
- Watertight tessellation through shared edge samples, plus the control-net slab chord bound.
- The Green's-theorem property integration layout.
- The USD BrepArray schema as an interop format.
- The fixture corpus and issue repros as regression tests.

solidcore does not change this verdict. It ports SMLib's architecture faithfully, keeps upstream wrong results for parity, has not yet landed fillet corners or the executive, and inherits the Numerical Recipes problem. Its upstream-issues.md is still worth reading as a defect map.

**What to take**

- METHOD (blend API design): adopt the four-axis factorization: offset law (constant, variable B-spline law, constant distance) × section generator (linear, circular, G1–G3 conic or blend with fullness) × continuity × corner strategy. Re-specify it with persistent names for rails, caps and corner faces. Source: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFilletSolver.h#L47-L65 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/skills/smlib-kernel-fillets/references/architecture.md
- METHOD (corner handling and test matrix): the NxM corner taxonomy with bevel vs blend semantics. Use it as the classification our vertex-blend solver must cover, each class with an explicit typed failure. Sources: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFilletGeom.h#L80-L95 , https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFilletCorner.h#L78-L110 , class list at https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFilletCorner.h#L239-L616
- METHOD (exact analytic fillets): when the offset law is constant and the supports are plane/plane or coaxial plane/cylinder/cone, emit exact cylinder, cone or torus fillet surfaces instead of NURBS. Gate: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFilletSolver.h#L206-L213 ; constructions at https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmFilletSolver.cpp#L418-L1000 (re-derive the geometry ourselves; do not translate)
- CASE CATALOGUE (fillet robustness tests): the roll-over, cliff and full roll-over cases, big radius converted to a corner, and tangent-chain radius laws. Each becomes a required test plus a typed error. Sources: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmFilletSolver.cpp#L5714 (TestRollOver; CreateCliffRollOverSideFG L6518, ProcessFullRollOver L6906, ProcessRollOver L7161) and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFilletExecutive.h#L401-L450
- ERROR SHAPE: model our blend error variant on SmFilletErrorInfo (kind, failure location point, offending vertex, edge or face, corner class, partial surfaces), using persistent names instead of pointers. Source: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFilletExecutive.h#L36-L167
- METHOD (watertight tessellation): sample every edge once, make each sample a shared topological node, and have face meshes reference those nodes by id, so meshes are crack-free by construction. Pair it with a deterministic tie rule for the 3D position (evaluate on the edge curve, not on whichever face comes first). Sources: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmTess.h#L742-L772 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmTess.cpp#L6939-L6990
- METHOD (certified chord bound): for polynomial or rational Bezier patches, the control-net slab thickness bounds deviation from the base plane. Make it the default, always-on criterion and add a derivative-based bound for analytic and offset faces. Source: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmSurfaceCache.cpp#L3575-L3600
- API PATTERN (tessellation diagnostics): per-face failure {face, stage, status}, partial output only as an explicit opt-in, and a report with manifold flags. Source: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SM_API/inc/SmApiBrep.h#L120-L131 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/operations/tessellation.md
- METHOD (mass properties): boundary-integral layout over oriented UV trim loops with a property-group bitmask (area, first and second area moments, volume, first and second volume moments). Re-implement with Gauss–Legendre per Bezier span, closed forms for planar and analytic faces, and an error estimate that fails loudly. Never translate the Numerical Recipes integrator. Source: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmFace.h#L68-L80 and https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmFace.cpp#L6027-L6140
- FORMAT (USD interop): write a reader and writer for the BrepArray schema from its schema text and contract (radial-edge structure-of-arrays, STEP-parameterized analytics). Add our persistent IDs and per-entity tolerances as namespaced extension attributes. Adopt the idea of a rule-numbered validator. Sources: https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/schema/omniSolid/resources/schema.usda#L21-L178 , https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/.agents/skills/usd-brep-schema/references/schema-contract.md , https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/tools/brep_validator/README.md
- TEST CORPUS (Apache-2.0, keep NOTICE): TestFiles/pt_TestFiles/Fillet (98 files), Shell (25), Defeature (12), Tess (8), Solids (38). Convert them once, offline, to our format. Add the public issue repros as regression scenarios: #2, #4, #8, #10, #12, #13, #19, #21. Source: https://github.com/NVIDIA-Omniverse/usd-brep/tree/c9c5979f04cea8c38213004d4f04df0dcfb40d05/TestFiles/pt_TestFiles and https://github.com/NVIDIA-Omniverse/usd-brep/issues
- DEFECT MAP (read, do not port): solidcore's upstream-issues.md tells us which SMLib methods are broken, so we do not reproduce those bugs (offset surface WR-160..165, blend generators UB-186/202/203 and HANG-34, offset hangs HANG-21/26/27, nondeterminism ND-1..3, integrator WR-94). Source: https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/plan/upstream-issues.md
- DO NOT TAKE: SmIntegrator, SmMatrix LU/Jacobi and the NR-marked NL_Math routines (Numerical Recipes-derived; https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/inc/SmIntegrator.h#L5-L8). Also not: SmOffsetSurface derivative code (wrong, https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/SmOffsetSurface.cpp#L1464-L1492), pointer-based SmTrackTopologyChanges, the global SmContext and status-code API, the stitch-after-fillet workaround, and solidcore's mass/integrator.rs.

### Challenge 1

**Refuted:** False

**Corrected verdict:** STUDY

**Errors found**

- NEW, MISSED (G3 blends are wrong): the evaluation lists G1–G3 blends as a strength, but the higher-order vectors used to build G2/G3 blends come from SmSurface::ComputeHigherOrderDerivs. That function uses finite differences of first derivatives with h=1e-9 (U+source/SMLib/src/SmSurface.cpp#L8855-L8920; called at U+source/SMLib/src/SmFilletStandardSolver.cpp#L4233-L4282). The 'third derivative' it returns is (T(u)-T(u+2h))/h, which is about -2·T'. That is minus twice the second derivative, not a second difference over h². So every G3 blend is built from bogus data. Even a correct second difference at h=1e-9 would be dominated by round-off. solidcore's upstream-issues.md does not log this. Found by reading the source and checking the algebra; not run.
- OVERSTATED (offset-surface derivatives 'feed G2/G3 blend construction'): the blend generators do not call the analytic offset derivatives at all; they use the finite-difference routine above. The fillet rail Newton step (LoadJacobian) needs only first derivatives (U+source/SMLib/src/SmFilletStandardSolver.cpp#L188-L262). Whether any shipped path consumes the wrong analytic third derivatives (WR-160/161) is unverified.
- COEFFICIENT WRONG IN THE EVALUATION: it says the '3.0 should be 1' at L1468 and L1479. That holds for Cuuv at L1468. For Cuvv at L1479 the correct coefficient on Buvv×Buv is -1, because Buv×Buvv = -Buvv×Buv. solidcore WR-160 says 'coefficients 1 and -1' (S+plan/upstream-issues.md, WR-160). Source: U+source/SMLib/src/SmOffsetSurface.cpp#L1468-L1491.
- TOLERANCE MODEL MISDESCRIBED: 'Tolerances are fixed absolute epsilons' is wrong. SmTol is a documented 'Consistent Tolerance Model' with ZoneTol3d, XSectTol3d and ApproxTol3d. A ModelSizeEstimate maps to ZoneTol3d in decade bands per Context and per Brep, and a 'pinhole in a battleship' small-topology ratio is applied (U+source/SMLib/inc/SmTol.h#L380-L510, #L567-L574). Scaling a Brep re-derives its tolerance from the vertex bounding box (U+source/SMLib/src/SmBrep.cpp#L28898-L28909). Fillets use pBrep->GetTolerance() (U+source/SM_API/src/SmApiFillets.cpp#L134). What does hold: the public SM_API has no model-size setter (SmApiCreateContext() takes no arguments), the shipped build uses SM_USE_OLDTOL (solidcore upstream-issues.md header), and some paths hard-code SM_ZONE_TOL_3D (SmApiRemoveFillet, U+source/SM_API/src/SmApiFillets.cpp#L210-L215).
- THE PRAISED CHORD BOUND IS THE #8 CULPRIT: the evaluation calls the SmSurfaceCache control-net slab test 'conservative and certifiable' and wants it always on (U+source/SMLib/src/SmSurfaceCache.cpp#L3575-L3600). That test measures pPSBox, a pseudo-box that has already been padded. Community PR #18 (github.com/NVIDIA-Omniverse/usd-brep/pull/18, unmerged, its claims unverified) reports that this fixed padding can exceed the requested chord tolerance, so subdivision never terminates. It names this as the root cause of #8 and reports the fillet completing in 0.081 s once fixed. The evaluation instead attributes #8 to generic absolute epsilons. Issue #8 is also a box of side 0.001 in model units, not '1 mm'.
- ISSUE #11 MISATTRIBUTED: the evaluation lists it as an integration plateau under mass properties. The reporter shows that an independent classify_uv grid integral is also +0.59%, and that the UV trim does not match the 3D edges after occt_to_usd or import. It is an I/O/trim defect, not an integrator defect (github.com/NVIDIA-Omniverse/usd-brep/issues/11).
- ISSUE #13 NOT QUALIFIED: the crash was seen only in a build that replaced SmOffsetExecutive.cpp, SmProtoTopology.cpp and SmVertex.cpp with awv-informatik/smlibpy patched copies. The reporter states it was not reproduced with those replacements disabled (github.com/NVIDIA-Omniverse/usd-brep/issues/13).
- ISSUE COUNT: the '21 public issues' are 19 issues plus 2 community PRs, #17 and #18. Both are open even though the README says contributions are not accepted (U+README.md#L101-L103).
- PERSISTENT NAMING IS NOT A BLANK SLATE: SMLib has an attribute-propagation system with 17 SmNotifyOperation events. These include SPLIT_IN_BREP, MERGE_IN_BREP, TRIM_NO_SPLIT_IN_BREP, COINCIDENT (for booleans) and REG_PROPAGATION, dispatched to SmAttribute::Split/Merge/Coincident/Copy (U+source/SMLib/inc/SmAttribute.h#L425-L520). It is pointer-based and has admitted gaps: only regions, faces, edges and vertices are notified, and 'A missing call to Notify() is an easily fixed bug' (L455-L462). It is not persistent naming, but its event vocabulary is the obvious METHOD to study for naming that is derived from operations, and the evaluation never mentions it.
- PURITY OVERSTATED: 'Operations mutate the B-rep in place' / 'Immutable … FAILS' is only partly true. SmApiShellBrepFull copies the input into a private working Brep, with the comment 'The public API is pure' (U+source/SM_API/src/SmApiBrep.cpp#L2366-L2395). AGENTS.md classifies offset_brep, shell_brep and tessellate as Pure, booleans as Consumes, and fillets/heal/translate as Mutates (U+AGENTS.md#L142-L145). Issue #2 also states that geometric fillet failures leave the Brep untouched; only the foreign-edge path corrupts another Brep.
- CHAIN OF TITLE INCOMPLETE: IntegrityWare's history page (integrityware.com/products/SOLIDS++/history.html) describes a 1998 IntegrityWare–GeomWare–SMS agreement. It calls SMLib a 'derived 3rd party library product' of IntegrityWare's libraries, and IntegrityWare still sells SOLIDS++. The chain is therefore IntegrityWare + GeomWare/NLib + SMS → NVIDIA, not just SMS and NLib. The terms of that agreement are unknown.
- 'Only remnant is GW_EXPORT' is wrong. Other remnants: 'Contact SMS support' (U+source/SMLib/inc/SmAttribute.h#L459); 'JLMCC per FS' third-party edits (SmAttribute.h#L71, SmBrep.cpp#L21097); 'gwc:' notes (SmTess.cpp#L6965); 'send case to SMS' (SmSurface.cpp#L34865). THIRD_PARTY_NOTICES claims 'Copyright (c) 2020-2026 NVIDIA', which predates the May 2022 acquisition, and lists neither Numerical Recipes nor Graphics Gems (U+THIRD_PARTY_NOTICES.md#L1-L10).
- SOLIDCORE'S NUMERICAL RECIPES EXPOSURE IS UNDERSTATED: besides mass/integrator.rs, solidcore also ported SmMatrix bit-identically as geom.matrix, including LUDecompose and EigenSystemJacobiSolver, which are the NR Crout and Jacobi code (S+plan/modules.toml#L174-L185). It also ported NL_Math.cpp as nlib.math (green), which contains NR-marked routines (S+plan/modules.toml#L49). The DO-NOT-TAKE list should name these too.
- WASM: 'No WebAssembly build exists' is true for this repo only. NVIDIA's concurrency.md documents browser/WASM workers (U+.agents/operations/concurrency.md#L60-L66). awv-informatik/smlibpy (MIT wrapper, created 2026-10-12) ships smlibwasm, SMLib compiled to WebAssembly (README at 8b6c494a319d399ec3627f3a562bcf24dbf19414).
- DEFECT MAP MISSES SMLIBPY'S ROOT CAUSES: according to smlibpy third_party/patches/README.md (at 8b6c494, unverified by me), remove_fillet does nothing because the face test in SmVertex::GetVertexuseOfFace/GetLoopOfFace was commented out. The same lookup also feeds point classification. AddressSanitizer found a write into freed memory at the end of every Boolean (in ~SmProtoTopologyManager), and SmOffsetExecutive::RemoveExcessFaces reads freed memory when an offset collapses. Shell ends in a Boolean, so both apply to this subsystem.
- MASS PROPERTIES NUANCE: the [1e-4, 1e-1] clamp is in SmApiMassProperties only (U+source/SM_API/src/SmApiQueries.cpp#L875-L903). Face.area() and Brep.volume() accept 1e-8 to 1e-12 (issues #11 and #19). Issue #19 also shows that the adaptive error estimate is untrustworthy (1e-4 requested, 3.4e-2 delivered, no error raised). It also shows one cross-process volume change on the z cup (17512.2 vs 17504.6), which is mass-property nondeterminism the evaluation does not mention.
- CHAMFER/ASYMMETRY: SmApiSurfaceSurfaceFillet exposes independent dRadius1 and dRadius2 with lXSectType 0 = linear, i.e. a public asymmetric two-distance linear blend between surfaces (U+source/SM_API/inc/SmApiBrep.h#L462-L472). The evaluation's statement holds only for edge chamfers on a B-rep.
- WATERTIGHTNESS IS DESIGNED, NOT DELIVERED: issue #12 reports non-watertight meshes in a full build. The docs' claim that the mesh 'never deviates more than 1mm' (U+.agents/operations/tessellation.md#L36) is contradicted by #7, #12, #20 and #21. Calling the mesh 'crack-free by construction' should be qualified.
- CITATION AND MINOR SLIPS: the rkck ak2..ak6 code is at SmIntegrator.cpp#L424-L470, not L20-L40 or L128-L170. 'thumbweight' is not in SmFilletSolver.h#L47-L65; it is an API parameter (SmApiBrep.h#L432-L441), the class member is m_dBlendScale (default 1.5), and G1/G2/G3 is at SmFilletSolver.h#L471. The OCCT premake comment is at L4-L6, not L3-L5. The 'recursion depth 15' is in a commented-out member (SmTess.h#L523). The analytic fillet path is a try-cylinder, then torus, then cone sequence, not a classification (SmFilletSolver.cpp#L1950-L2010).
- FIXTURE INVENTORY INCOMPLETE FOR I/O: besides the native .smb fixtures there are 29 self-authored OCCT .brep fixtures (TestFiles/occt_breps), 18 STL files, an ACIS acisTest.sat, and about 70 ssi_*.smp surface/surface intersection fixtures. The README also says that STEP/IGES/JT fixtures are tracked through Git LFS, but none are in the tree at c9c5979.

**Argument:** FACTS. I checked the evaluation against usd-brep at c9c5979 (U = https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/) and solidcore at 6a019c5 (S = https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216/). Most claims hold.

Confirmed:
- Repo metadata: created 2026-10-08, 1,747 stars, 3 bot commits, SMLib about 40.2 MB.
- Fillet solver and generator enums, corner taxonomy, roll-over functions, and analytic cylinder/cone/torus at L432/L662/L860.
- The 'stitching for some reason' comment, and validity checks that run only under SM_DEBUG_CODE (SmAssertArray.h#L127-L139).
- IntegrateIt returning 0 with SM_SUCCESS.
- Numerical Recipes markers and constants in SmIntegrator, SmMatrix and NL_Math.
- Clock-seeded srand at SmFace.cpp#L21183 and SmShell.cpp#L888/#L905, and the first-call static at SmFilletSolver.cpp#L3604.
- Pointer-keyed unordered_map, no FP-contract flags, no WASM target in the repo.
- Tessellation defaults (chord 0, angles 25/25), per-face failure reports, and the shared PolyVertex mapping.
- py::nodelete handles and the global SmContext concurrency rule.
- OCCT .brep is a standalone parser and writer; IGES exists only as NLib entity converters; there is no STEP reader or writer.
- Python test count 400, the remove_fillet smoke test, and the fixture counts.
- solidcore: 234/261 units, 88.1% of lines, 176 UB / 120 WR / 34 HANG / 3 ND / 4 DOC, WR-94 and WR-160..165 'Kept'.

The errors are listed in errors_found. The most consequential:
- G3 blends are wrong. The blend code builds its 'third derivative' by finite differencing (SmSurface.cpp#L8855-L8920), and the result is about -2× the second derivative. The evaluation praised G1–G3 blends and pinned the offset third-derivative bug on them; that link does not exist.
- The tolerance model is a model-size-banded per-Brep scheme, not fixed epsilons.
- The chord test the evaluation calls certifiable is the likely cause of the #8 hang, through pseudo-box padding (PR #18).
- SMLib's attribute split/merge/coincident event system was missed. It is the closest thing it has to persistent naming.
- The chain of title also runs through IntegrityWare.
- solidcore also ported the NR-derived SmMatrix and NL_Math code.
- #11 and #13 are misattributed or unqualified.

FIT. Arguing against STUDY, a softer verdict could rest on:
- The SmTol 'consistent tolerance model' classification rules: vertex, edge and face neighbourhoods nest; there are separate Zone, XSect and Approx tolerances; and gap-derived runtime tolerances.
- The 17-event attribute-propagation vocabulary.
- The BrepArray schema, with about 100 rule-numbered validator fixtures.
- Exact analytic fillets, including chamfers (ConstantDistance is analytic-capable).
- Candid docs and Pure/Consumes/Mutates contracts.
- A public shell API that is pure through copy-in.

None of this is best in class. Each tolerance or naming method is pointer-based and incomplete by its own admission. PORT_CODE is ruled out by:
- debug-only validity checks;
- silent failures (IntegrateIt, remove_fillet, heal, shell);
- broken G3 blends and offset derivatives;
- use-after-free at the end of every Boolean (per smlibpy's ASan patches);
- an IntegrityWare/NR provenance cloud.

DIFFTEST_ORACLE is ruled out by:
- process-dependent results (#10, #19 z case, #21);
- an untrustworthy integrator error estimate (#19: 1e-4 requested, 3.4e-2 delivered).

PARTNER is ruled out because upstream does not accept contributions.

Arguing for something harsher than STUDY (REJECT) also fails. The Apache-2.0 fixtures, issue repros and BrepArray format are useful, and the taxonomies are worth re-deriving.

Verdict: STUDY holds. The what-to-take list should add:
- the SmTol neighbourhood-classification rules (method study);
- the SmNotifyOperation split/merge/trim/coincident vocabulary, as input to our naming design;
- smlibpy's patch notes and PR #18, for the defect map.

It should drop the claim that the offset-derivative bug affects blends, and flag G3 blends as broken.

## solidcore

**Commit sha:** 6a019c57131d5fee6e57ab7dc4a62201a32e0216

**Findings:** All links below are pinned to B = https://github.com/tangency-labs/solidcore/blob/6a019c57131d5fee6e57ab7dc4a62201a32e0216

WHAT IT IS. solidcore is a unit-by-unit transliteration of NVIDIA SMLib + SM_API (usd-brep at c9c5979, the "Release 1.0.0" commit of 2026-10-09) into safe Rust. It leaves out the OCCT translators, USD/Hydra and the Python layers. Its stated goals are in B/README.md#L23-L34: no unsafe, no panics, deterministic, no global state, wasm32, and 1e-9 relative parity with the C++ "with identical topology". The README's "about a third ported" line (B/README.md#L29) is stale. The generated checklist (B/plan/PROGRESS.md#L5) reports 234 of 261 units ported (114 green, 120 partial, 27 claimed), covering 341,813 of 387,903 assigned upstream lines (88.1%). README#L34 still holds: nothing works end to end. Primitives (CreateBox) are not ported, two-B-rep booleans still wait on merge_p2 (B/crates/solidcore/src/boolean/api_brep.rs#L43), and fillets are mostly claimed but unmerged.

SCOPE. B/plan/closure.md#L6-L19 counts 979,541 upstream SMLib+SM_API lines. A libclang call-graph closure from the SM_API entry points cuts that to 392,766 in-scope function lines, 6,068 functions, 260 units and 65 layers. Out of scope: SmApiTessellate, Poly*, Boolean2d, NonManifoldBoolean, ProjectCurve, OffsetCurve and the Intersect*/CreateSurface* APIs. Even a finished solidcore would therefore have no 2D booleans, no non-manifold ops and no tessellation API. It covers only NURBS/analytic B-rep: no subdivision, implicit fields, meshes-as-solids or point clouds.

SIZE AND LAYOUT. There are 3 crates (B/Cargo.toml).
- crates/solidcore: 309 files, 383,677 Rust lines, 14.8 MB. Modules are geom, nurbs (58 files), curve, surface (48), topology (37), intersect, boolean (10.7k lines), blend, offset, heal, mass, query, sweep and primitive. Files are split by line budget (`_p1`..`_p11`), not by concept.
- crates/solidcore-ffi: 240 files of extern "C" wrappers. This is the only crate allowed to use unsafe.
- crates/difftest: 235 unit runners.
- Supporting material: harness/cpp (250 C++ shims plus a CMake build of upstream), harness/callgraph (libclang extract -> graph -> SCC/back-edge unit planner), harness/mathprobe (libm ulp probe), fixtures/ (410 files, 106 MB) and reports/difftest (235 JSON files, 117 MB), all committed. The only runtime dependency is thiserror.

ARCHITECTURE.
- Kernel (B/crates/solidcore/src/kernel.rs#L149-L164) is the SmContext. It holds absolute tolerances (zone 1e-5, xsect 2e-5, model_size_estimate 50, large/small ratio 1000; kernel.rs#L30-L65), a SplitMix64 RNG replacing srand(time) (kernel.rs#L75-L114), caches, and a Context with traversal marks/locks, a doing_boolean flag and boxed notify callbacks (B/crates/solidcore/src/geom/context.rs#L175-L194). The kernel is threaded `&mut` through 233 signatures and `&` through about 1,680.
- Topology (B/crates/solidcore/src/topology.rs#L74-L232, #L534-L575) is a radial-edge B-rep (region/shell/face/faceuse/loop/loopuse/edge/edgeuse/vertex/vertexuse) in append-only `Vec` arenas. Typed u32 ids are raw indices (`from_index` is public). There are pub `*_mut` accessors and `add_*_record` "friend writes" that link records without validation, and 660 `&mut Brep` sites. Upstream circular lists are emulated as Vec plus a head id. SmBoolean stays an i32 with UNSURE=2. Deleted surfaces are retained.
- Geometry is concrete enums. Following SMLib, Plane and SurfOfRevolution are subclasses of BSplineSurface: analytic surfaces carry a NURBS body (B/crates/solidcore/src/surface/surface/p2.rs#L259-L278). Cylinder/cone/sphere/torus do not yet dispatch as SurfaceKind (issue #76).
- Errors: SolidError (B/crates/solidcore/src/error.rs#L97-L184) has the variants InvalidInput, NotFound, Degenerate, NotConverged, Unsupported, Upstream{code}, Nlib{code} and Unported. Each carries an `operation` path, plus an EntityRef where one applies.
- Attributes with split/copy/merge/coincident propagation through Notify are ported (geom/attribute.rs). This is SMLib's only history mechanism. There is no persistent naming.

DIFFTEST HARNESS. Each unit gets a C++ shim (`sc_<unit>_<fn>`, plain C data, exceptions caught), a guarded safe FFI wrapper, a runner with run_cpp/run_rust/generate, and JSON fixtures re-expressing upstream prog_test/SM_API_test cases.
- Comparison rule (B/crates/difftest/src/compare.rs#L1-L16, #L93-L106): floats must be bitwise equal or within |a-b| <= 1e-9*max(|a|,|b|). Both NaN passes, one NaN fails, 0 vs 1e-300 fails. An absolute floor is allowed only where a runner documents one, and no unit uses any. Integers, topology (traversal-order indices) and error strings must match exactly.
- Extras (B/crates/difftest/README.md): `--perturb` mutation sensitivity check (#L54-L60); `--fuzz N` FFI-guard fuzzing in child processes with crash/panic/timeout verdicts (#L91-L133); deferred/partial classification (#L135-L158); fixed pass criteria (#L170-L180); `--ssi-corpus` exact-ground-truth scoring (#L353-L421); deterministic reports with no timestamps (#L433-L435).
- The reference is x86-64 Linux glibc with -ffp-contract=off, and Rust keeps the upstream operation order.

MEASURED RESULTS (my aggregation of all 235 reports at this SHA):
- 518,707 cases: 493,865 passed, 24,694 deferred, 0 failed. 31,713 passing cases compared nothing (both sides errored or empty), and 4,463 had both sides error.
- max_rel_err is exactly 0.0 in 232 of 235 units. The worst is 1.78e-13 (topology.edgeuse_p2). In practice it is bit-identical, not merely within 1e-9.
- boolean.api_brep: 151 passes, 136 deferred, and zero values compared in any pass.
- SSI corpus (B/reports/ssi-corpus.json): the C++ scores 81 PASS, 25 INCOMPLETE, 11 WRONG TOPOLOGY and 2 ERROR out of 119. The Rust scores 3 PASS and 116 DEFERRED.

OPEN AUDITS.
- Issue #210: many partial B-rep units pass only on refusals and no-ops. Some ports stop at the first stub yet count their full line total.
- Issue #294: an AI-authored audit estimates about 13,900 stale or harness-caused deferrals and 1,476 empty passes.

WHO AND CADENCE. Tangency Labs is a GitHub org created 2026-10-16, apparently run by Brennen Herbruck (bherbruck), author of SolidCraft (Rust parametric CAD, about 8.2k stars, created 2026-10-07). solidcore began as bherbruck/solidcore on 2026-10-12 and was moved into the org. Other active humans include froglemonade2, Christian Svensson (bluecmd), Karl Von Richter (Embeint), Damian Grudzien, Thijs Damsma, Jackson Oursland and Irakli Kourtis (jyviko, author of the ssi-corpus). Blacksmith sponsors CI. Whether Tangency Labs is a legal entity is unverified.
- Cadence: 3,839 commits from 2026-10-12 to 2026-10-20, peaking around 500-600 a day. 305 PRs (203 merged, 73 open) from 25 authors, 118 of them by bherbruck.

AI GENERATION. 1,891 of 2,290 non-merge commits (83%) carry AI trailers: Claude Opus 5.5 about 1,400, Sonnet 5.5 209, Codex/OpenAI Codex 215, Claude Fable 50, GPT-6.x about 11. AGENTS.md is written for agent swarms (B/AGENTS.md#L1-L13: "Many agents work here at once"). STATUS.md records a local swarm of up to 10 agents. Issue #294 says outright that a human reviewed it "only at the summary level". In effect, about 384k lines were AI-written in 9 days, with the C++ difftest as the main correctness check.

**Robustness exactness:** ARITHMETIC. Everything is plain f64 in upstream operation order. There are no exact or adaptive predicates (no orient2d/3d, expansion arithmetic or interval filtering anywhere in the crate). mul_add and third-party linear algebra are deliberately banned to keep bit parity (B/crates/solidcore/src/math.rs#L21-L28). The upstream numerics are kept as they are. Example: the Graphics Gems I closed-form cubic, which uses absolute zero_tol branching and cbrt/acos (B/crates/solidcore/src/geom/polynomial.rs#L132-L176; upstream SmPolynomial.cpp says it was "taken from Graphic Gems I").

TOLERANCES. All absolute and not scale-invariant (B/crates/solidcore/src/kernel.rs#L30-L65): zone 1e-5, approx 5e-6, angle 0.5 degrees, model size 50, large/small ratio 1000. Magic sentinels: UNDEF_DOUBLE=1.23456e-61, BIG_DOUBLE=1e20, INFINITE_PARAMETER=1234567.0 (math.rs#L43-L64). The ZONE_TOL/zone_tol identifiers appear on about 1,239 lines. Clippy's float_cmp is allowed because upstream compares doubles exactly (B/Cargo.toml#L38).

DEGENERACY HANDLING AND FAILURE MODES. The port is deliberately faithful to upstream defects.
- B/plan/upstream-issues.md catalogues 337 SMLib defects: 176 UB/crash, 34 hangs, 3 nondeterminism, 120 wrong-result, 4 docs.
- UB is mostly guarded in Rust: bounds-checked reads, InvalidInput, and bounded allocation from file counts.
- Wrong results are mostly kept. By my count, 89 of 120 WR entries say "reproduces", "ported as is" or "faithful". Example: WR-1 is reproduced (upstream-issues.md#L1972-L1979).
- Some hangs are kept too. HANG-1 PeriodicWrap is "Same loop (faithful). The generator avoids such ratios." (#L1653-L1660).
- `ignored`/`ignore_status` (B/crates/solidcore/src/error.rs#L36-L50) turn upstream's ignored statuses into Ok. They are called at about 1,100 sites, so errors are silently swallowed by design.
- SM_ASSERT_VALID checks are not ported (B/crates/solidcore/src/boolean/api_brep.rs#L48). No B-rep validity is enforced on output.

UPSTREAM QUALITY CEILING. The C++ that solidcore aims to match bit for bit fails 38 of 119 exact-truth surface/surface intersection cases. It misses interior tangent points (WR-74, upstream-issues.md#L2805-L2812) and tiny loops above the zone tolerance, does not split curves at singular crossings, duplicates or collapses tangent curves, and raises SM_ERR_NULL_POINTER on 4th-order contact (issue #17; B/reports/ssi-corpus.json). A faithful port inherits all of this; booleans built on that SSI cannot be "never silently fail".

PANIC SAFETY. Lints deny unwrap, expect, panic, indexing_slicing and todo in library code (B/Cargo.toml#L20-L32). Input-driven recursion must carry an explicit depth bound because wasm has no guard page (B/AGENTS.md#L42-L48), though the crate-wide audit.recursion unit is still a draft (#273). Issues #181/#206 (32 GiB and capacity-overflow allocations) and #198 (a subtract-with-overflow panic) were found and fixed. Issue #182 (arithmetic-overflow panics in nlib.basis_adv) is still open.

ERRORS. Structured, with an operation path and entity, but reasons are &'static str. Upstream codes survive as Upstream{code:i32}, and Unported is a first-class variant (error.rs#L97-L184).

**Determinism:** What holds:
- No HashMap or HashSet anywhere in the kernel (0 hits).
- Arenas iterate in insertion order.
- No clocks in library code; the only Instant use is in a test.
- No threads; the kernel is single-threaded and Send.
- srand(time) is replaced by a seeded SplitMix64 in the Kernel (B/crates/solidcore/src/kernel.rs#L75-L114).
- Upstream's address-ordered iteration over pointer-keyed maps (4 sites; B/plan/closure.md#L697-L707) becomes id order. Rust is deterministic there, but it cannot match upstream, which is not.

What does not hold:
1. Native vs WebAssembly is NOT bit-identical, and the project documents this as an open, undecided question (B/plan/findings.md#L46-L99). Rust's f64 sin/cos/atan2/powf/tan/sinh/cosh/log10/acos call the platform libm. Measured against glibc x86-64: wasm32 differs on 2.4% (cos) to 17.1% (atan2) of inputs, up to 2 ulp. macOS differs on up to 39.4% (tan). cbrt even depends on Linux link order (compiler_builtins vs glibc). CI only runs `cargo check` for wasm and never compares wasm values. The kernel has 86 sin, 98 cos, 20 atan2, 12 powf, 9 tan and 1 cbrt call sites by my grep.
2. To stay identical to the x86-64 C++, the port emulates x86-64 behaviour of out-of-range double-to-integer casts (UB in C++) (B/plan/findings.md#L29-L45). The Rust result is the same on every target, but it reproduces an artefact rather than a defined semantic.
3. Results depend on call history: the RNG and caches live in a `&mut Kernel` that persists across operations (kernel.rs#L149-L164; RNG used in topology/face/p2.rs GetPointsInFace). Two identical operations can give different results depending on what ran earlier on the same Kernel. That is deterministic for a fixed sequence, but operations are not pure.
4. "Any thread count" holds only trivially, because nothing is parallel. Upstream's TBB paths are compiled out (B/plan/closure.md#L827-L845).

**Tests and quality:** TESTS.
- 1,526 #[test] functions in the kernel crate.
- 518,707 differential cases across 235 unit reports, 0 failed, 24,694 deferred. Generated cases use a seed of fnv1a64(unit)+i. Upstream tests are re-expressed as fixtures.
- Harness self-tests, `--perturb` sensitivity checks and `--fuzz` of the FFI guards.
- CI (.github/workflows/ci.yml) runs fmt, clippy with -D warnings, tests, wasm `cargo check`, cargo-deny, a publish dry-run, and the full difftest of every green and partial unit on a 16 vCPU Blacksmith runner.
- Toolchain pinned to 1.98.0.

COVERAGE CAVEATS, from the project's own audits:
- Issue #210 finds that most partial B-rep units pass on refusals and no-ops, some ports return Unported at the first missing callee and drop the rest of the function, and latent default-argument and logic bugs are found in code that cannot run yet.
- Issue #294 estimates about 13,900 stale or harness-caused deferrals and 1,476 passes with nothing compared. Shims replace analytic surfaces with B-spline stand-ins, so analytic branches are never compared.
- Difftest proves agreement with the C++, not correctness. Only the SSI corpus checks against exact ground truth, and there Rust can run 3 of 119 cases.
- No property-based tests against mathematical invariants and no validity checker on outputs.

CODE QUALITY.
- Strong per-file traceability: every file opens with an Upstream: line, a Rust-to-C++ symbol table and a Deviations line (B/AGENTS.md#L122-L138).
- Docs are required on all public items, and the lint discipline is high.
- But the structure is C++ transliterated: line-count file splits (`_p1`..`_p11`), i32 tri-state booleans, circular-list emulation, `owner` back-references, mutable mark/lock state, magic sentinels and about 578 UNPORTED stub sites.
- Readable for tracing back to SMLib, not as a design an AI or a person can reason about from first principles.
- About 83% of non-merge commits are AI-attributed, with human review largely at summary level. Quality rests almost entirely on the C++ oracle.

**Legal:** solidcore licence:
- Apache-2.0. LICENSE is copied verbatim from upstream.
- NOTICE (B/NOTICE) states it is a derivative of usd-brep SMLib/SM_API at c9c5979, carries NVIDIA's notice ("Copyright (c) 2020-2026 NVIDIA CORPORATION & AFFILIATES", SPDX Apache-2.0) and adds "Copyright (c) 2026 The solidcore contributors".
- B/ATTRIBUTION.md lists exactly which upstream paths were read. It excludes OCCT_BREP_*, the USD/Hydra layers, SmPyLib and proposals/, and says no GPL/LGPL was used.
- The Rust files carry no SPDX header (0 files). Provenance is the "Upstream:" doc line in each file, plus the root NOTICE.
- Upstream SMLib files, NLib (NL_*.cpp) included, carry NVIDIA SPDX Apache-2.0 headers (e.g. https://github.com/NVIDIA-Omniverse/usd-brep/blob/c9c5979f04cea8c38213004d4f04df0dcfb40d05/source/SMLib/src/NL_BasisBasic.cpp#L1-L2).

Provenance chain (web sources; Wikipedia's SMS article is flagged as weakly sourced):
- Boeing TIGER NURBS work, then Applied Geometry (Blomgren, 1984).
- Solid Modeling Solutions (Blomgren and Presti, 1998), which bought NLib from GeomWare in 2001. Wayne Tiller, co-author of The NURBS Book, was a partner.
- NVIDIA acquired SMS in May 2022 and released the code on 2026-10-08/09 as usd-brep. The repo has only 3 commits (initial, 0.2.0, 1.0.0).

Other components:
- Test corpus: jyviko/ssi-corpus at b04c8a6 is MIT OR Apache-2.0. Its 25 y23_* cases derive from Yang, Jia and Yan (2023), CC-BY, so attribution is required.
- The progress site vendors d3-force (ISC). This is not part of the kernel.
- deny.toml bans GPL, LGPL, AGPL and MPL (B/deny.toml#L2). Spare Kit's MPL-2.0 code therefore cannot flow into solidcore, and any Spareware contribution there would have to be Apache-2.0.

Before an owner ports anything, check:
(a) Keep NOTICE text from both NVIDIA and solidcore, and mark changes, per Apache-2.0 section 4. Porting into MPL-2.0 Spare Kit is allowed under the owner's rule.
(b) Apache-2.0 section 3 gives a patent grant from contributors (NVIDIA for upstream). No patent search was done (unverified).
(c) Copyright in largely AI-generated Rust is legally uncertain (unverified). The upstream Apache licence still covers the derivative, so this does not block use, but "the solidcore contributors" copyright claim may be thin.
(d) The difftest reference build needs the upstream clone at c9c5979 (Apache-2.0). Nothing GPL or LGPL is involved.

**Alignment:** Spare Kit principles against solidcore:
1. Iron-clad. Fails. solidcore uses tolerance-based f64 with absolute tolerances and no exact predicates. By design it keeps about 89 of 120 catalogued upstream wrong-result defects, and some hangs. Status-ignoring helpers swallow errors at about 1,100 sites, so a sub-step can fail silently. Output validity checks (SM_ASSERT_VALID) are not ported. The surface/surface intersection it must match fails 38 of 119 exact-truth cases in the C++. Its goal (bit parity with SMLib) is the opposite of ours (better than SMLib). Partial credit: structured SolidError with operation path and entity, guarded allocation and recursion, no panics on input.
2. Deterministic, bit-identical native and wasm, any thread count. Partial. Run-to-run deterministic on one platform, with no hashing, clocks or threads. But native and wasm differ through libm, the project's own measurement shows it (findings.md#L46-L99), and it is undecided. Results also depend on the RNG and cache history of a mutable Kernel. To meet our rule we would need our own fixed pure-Rust (ideally correctly rounded) transcendentals.
3. Persistent naming. Absent. Identity is the raw u32 arena index (`from_index` is public), which is exactly the index identity Spare Kit forbids. SMLib's attribute propagation (split/copy/merge/coincident via Notify) is the only history-like method, and is worth studying as a source of operation events.
4. Immutable data, pure operations. Fails. Breps are edited in place through pub `*_mut` accessors and unvalidated `add_*_record`. The kernel carries mutable marks/locks, a doing_boolean flag, callbacks, caches and the RNG. Booleans consume and modify operands.
5. No unsafe. Passes: #![forbid(unsafe_code)] (B/crates/solidcore/src/lib.rs#L30). Only the test-only FFI crate uses unsafe.
6. Full scale. Fails. Absolute zone 1e-5, model size 50, a 1000:1 feature ratio and parameter sentinels such as 1234567.0 do not suit km-scale BIM sites modelled to sub-mm detail.
7. Readable by AIs and people. Mixed. Upstream traceability is excellent, but this is transliterated C++ (line-split files, tri-state i32, list-head emulation) at 384k lines. That conveys how SMLib works, not a clean design.
8. Fully headless. Passes.
9. Every kind of geometry, Revit-plus and AutoCAD-class 2D. Fails on scope: NURBS/analytic B-rep only, with Boolean2d, non-manifold ops and tessellation out of scope, and no subdivision, implicit, mesh or point-cloud support.
10. Our own kernel, methods not architecture. solidcore's architecture is SMLib's architecture (radial-edge use-records, analytic-as-NURBS-subclass, context marks) in Rust clothing. Porting its code would import exactly what the owner's rule forbids.

Recommendation: port independently. Take SMLib methods (judged in the usd-brep/SMLib subsystem review, not here) and literature methods into our own architecture. Do not depend on solidcore, and do not contribute kernel code: our goals diverge, and their deny.toml bans MPL. Collaborate only on shared test assets (exact-truth corpora, defect catalogues), which needs no formal partnership. Use solidcore's green NURBS/curve units as an optional pure-Rust reference oracle on well-conditioned inputs.

**Strengths**

- Measured bit-identical parity with the C++: 518,707 differential cases, 0 failures, max_rel_err exactly 0 in 232 of 235 units. The green nlib/nurbs/curve layers (48 + 25 units) are a faithful pure-Rust copy of SMLib/NLib.
- The difftest methodology is the best part. A strict comparator with no tolerance loosening, a --perturb check that the comparator is sensitive, --fuzz of the FFI guards in child processes, deferred/partial accounting, and deterministic committed reports (B/crates/difftest/README.md#L54-L180).
- The libclang call-graph pipeline (harness/callgraph) computes the in-scope closure, splits it into units, treats SCC back-edges as stubs, and scans for global-state and nondeterminism hazards (B/plan/closure.md#L658-L725).
- A catalogue of 337 SMLib defects with triggers and locations (176 UB, 34 hangs, 120 wrong results), plus the ssi-corpus integration with a scorer that agrees 119/119 with the corpus's own score.py.
- An honest, quantitative record of the native/wasm libm divergence and of x86-64 cast-UB dependence (B/plan/findings.md#L29-L99).
- Strong Rust hygiene: forbid(unsafe_code); unwrap, expect, panic and indexing denied; allocation from untrusted counts bounded; input-driven recursion bounded for wasm; no HashMap; no global state; Apache-2.0 with a careful NOTICE and ATTRIBUTION.

**Weaknesses**

- Faithful to upstream defects by design: about 89 of 120 known wrong-result bugs are kept, some hangs too (e.g. HANG-1), and about 1,100 status-ignoring call sites swallow errors.
- No exact or robust predicates; absolute, not scale-invariant tolerances (zone 1e-5, model size 50); magic sentinels.
- Mutable in-place B-rep arenas with pub *_mut and unvalidated record insertion; identity is a raw u32 index; no persistent naming; a mutable Kernel holding marks, locks, RNG and caches makes operations non-pure.
- Native vs wasm is not bit-identical (platform libm), measured but undecided; wasm values are never tested in CI.
- Not usable end to end: no primitives, two-B-rep booleans waiting on merge_p2, fillets mostly unmerged. Rust SSI runs 3 of 119 corpus cases. The README's 'about a third' is stale (88% of lines ported), but many 'partial' units pass only on refusals and no-ops (issues #210, #294).
- Scope excludes 2D booleans, non-manifold ops and tessellation; NURBS-centric (analytic surfaces are NURBS subclasses); none of subdivision, implicit, mesh or point-cloud geometry.
- About 83% of commits are AI-attributed, about 384k lines in 9 days, with human review largely at summary level. Correctness rests almost entirely on agreement with an upstream that is itself defective.
- Transliterated C++ structure (line-split _p1.._p11 files, i32 tri-state booleans, list-head emulation). Readable for tracing to SMLib, not as a conceptual design.
- deny.toml bans MPL, so Spare Kit code cannot flow back, and their goal (bit parity with SMLib) conflicts with ours (exceed SMLib).

**Verdict:** DIFFTEST_ORACLE

**Verdict reason:** Use solidcore as a test oracle only, never as code or a dependency of the kernel. Its architecture is SMLib's in Rust form: mutable index-identified radial-edge arenas, a mutable session context, absolute tolerances, f64 without exact predicates, upstream wrong results kept on purpose, and errors swallowed at about 1,100 sites. That conflicts directly with iron-clad behaviour, pure operations, persistent naming and full scale, so PORT_CODE is ruled out. It adds no algorithms of its own; every method is SMLib's, and those are better judged and taken from usd-brep at c9c5979 directly, so PORT_METHODS belongs to the SMLib subsystem, not this one. PARTNER does not fit either: their goal is bit-faithful SMLib, ours is to exceed it, and their deny.toml bans our MPL-2.0. What it does offer is a pure-Rust, wasm-buildable, Apache-2.0 reference that matches the C++ bit for bit on the green NURBS and curve units. Spare Kit can use those as a cargo dev-dependency or recorded-output oracle on well-conditioned inputs, with no C++ toolchain. Where results disagree, exact ground truth decides, not SMLib. Its difftest, perturbation, fuzz and call-graph tooling, the libm divergence measurements and the 337-entry defect catalogue are well worth studying and using as test assets.

**What to take**

- Oracle: use the green NURBS/NLib/curve units as a reference answer in Spare Kit tests on well-conditioned inputs, pinned to this SHA (e.g. B/crates/solidcore/src/nurbs/knots.rs and the 48/48 green nlib.* and nurbs.* units in B/plan/PROGRESS.md#L62-L111). Disagreements are settled by exact or interval ground truth, never by deferring to SMLib.
- Method (test infrastructure): the strict comparator rule (bitwise or 1e-9 relative, NaN handling, abs floors only if documented, never loosened) from B/crates/difftest/src/compare.rs#L1-L106, plus the --perturb sensitivity check and the 'passes with nothing compared' accounting from B/crates/difftest/README.md#L44-L60. Adopt these in Spare Kit's own oracle and regression harness.
- Method: FFI-guard fuzzing in child processes with crash/panic/timeout verdicts and reproducible seeds (B/crates/difftest/README.md#L91-L133). Use it whenever Spare Kit cross-checks against a C++ reference.
- Evidence for a Spare Kit decision: the measured libm divergence (sin/cos/atan2/powf/tan/cbrt across glibc, Apple, wasm32 and compiler_builtins) in B/plan/findings.md#L46-L99, plus the harness/mathprobe probe (B/harness/mathprobe/README.md). This shows we must ship our own fixed, ideally correctly rounded, pure-Rust transcendentals to be bit-identical across native and wasm.
- Evidence: x86-64 vs aarch64 float-to-int UB dependence (B/plan/findings.md#L29-L45). Spare Kit must use only defined conversions (checked or saturating) and never emulate platform UB.
- Test assets: the 337-entry SMLib defect catalogue with triggers (B/plan/upstream-issues.md, e.g. HANG-1 #L1653-L1660, WR-1 #L1972-L1979, WR-74 #L2805-L2812), turned into adversarial regression cases for Spare Kit.
- Test assets: the SSI exact-ground-truth corpus (github.com/jyviko/ssi-corpus at b04c8a6, MIT OR Apache-2.0; y23_* cases CC-BY) and solidcore's Rust port of its scorer (B/crates/difftest/src/ssi/). Make it a Spare Kit SSI acceptance gate, with the C++ baseline of 81/119 in B/reports/ssi-corpus.json as the bar to beat.
- Method (porting planning): the libclang call-graph closure, unit planner and global-state/nondeterminism hazard scan (B/harness/callgraph/README.md, B/plan/closure.md#L6-L19 and #L658-L725). Reuse it when studying any large Apache C++ source.
- Engineering rules worth copying into Spare Kit's AGENTS.md: bound every allocation driven by an untrusted count, and bound every input-driven recursion because wasm32 has no stack guard page (B/AGENTS.md#L32-L48).
- Error shape as a starting point only: the operation path plus EntityRef in SolidError (B/crates/solidcore/src/error.rs#L97-L184). Do NOT take the Upstream{code} variant or the ignored/ignore_status pattern (error.rs#L36-L50).
- Study only, do not port: SMLib's attribute propagation through Notify (split/copy/merge/coincident handlers, B/crates/solidcore/src/geom/attribute.rs#L1-L5) as a catalogue of the edit events a persistent-naming scheme must observe.
- Do NOT take: the u32-index arena topology with *_mut accessors (B/crates/solidcore/src/topology.rs#L74-L232, #L534-L575), the mutable Kernel/Context (B/crates/solidcore/src/kernel.rs#L149-L164, B/crates/solidcore/src/geom/context.rs#L175-L194), the absolute tolerances (B/crates/solidcore/src/kernel.rs#L30-L65), the analytic-as-NURBS-subclass SurfaceKind (B/crates/solidcore/src/surface/surface/p2.rs#L259-L278), or numerically naive ports such as the Graphics Gems cubic (B/crates/solidcore/src/geom/polynomial.rs#L132-L176).

### Challenge 1

**Refuted:** False

**Corrected verdict:** DIFFTEST_ORACLE

**Errors found**

- Wrong count and wrong line pin for the oracle set. The evaluation says '48/48 green nlib.* and nurbs.* units in B/plan/PROGRESS.md#L62-L111' and calls the green layers '48 + 25 units'. At 6a019c5, L62-L81 is the geom section. The nurbs section is B/plan/PROGRESS.md#L82-L131 and has 45 green and 3 partial units: L123 nurbs.b_spline_curve_p3, L130 nurbs.b_spline_surface_n_lib_p2 and L131 nurbs.b_spline_surface_p2. The curve section (#L133-L158) has 20 green and 5 partial units: L138, L148, L150, L152 and L158. The proposed oracle set is therefore 65 green units, not 73. 48 and 25 are the ported counts, not the green counts.
- 'Green' is weaker than the evaluation implies. surface.sphere is green (PROGRESS.md#L188), yet B/reports/ssi-corpus.json shows its SmSphere::GlobalSurfaceIntersect still Unported, deferring 24 corpus cases. Issue #294, row 1, attributes about 9,000 deferrals to library Unported stubs that name units already green or partial. nlib.basis_adv is green (PROGRESS.md#L89), yet issue #182 records 41 fuzz cases where Rust returns an error and the C++ succeeds. The fix for those, PR #253, is not merged. Green means only that the generated and fixture cases hit no stub.
- Issue #182 is mischaracterised as an open panic. The arithmetic-overflow panics were fixed by PR #187, merged 2026-10-17. The pinned code uses checked_add at B/crates/solidcore/src/nurbs/basis_adv.rs#L154, #L299 and #L309-L310. #182 stays open only for the 41 error-vs-success mismatches. Separately, #198 was a panic in the difftest runner (topology_vertexuse), not in kernel library code.
- Determinism point 3 is overstated. 'The RNG and caches live in a &mut Kernel… two identical operations can give different results.' Caches is an empty struct whose contract is 'clearing must never change results, only speed' (B/crates/solidcore/src/kernel.rs#L116-L122), and kernel.caches is never read. The RNG is drawn at one site, rand_dither in GetPointsInFace (B/crates/solidcore/src/topology/face/p2.rs#L1136-L1141), and only when more than one point is requested. B/plan/closure.md#L664-L670 says the in-closure callers request a single point. History dependence is real but confined to that path.
- 'Attributes… This is SMLib's only history mechanism' is false. Two more are ported in B/crates/solidcore/src/topology/topology.rs#L1-L42: SmTopology::Notify with NotifyEvent (#L464-L475), and SmTrackTopologyChanges with rm_list, add_list, chg_list and notify_events (#L477-L489, #L769-L779). These are the better study source for the edit events persistent naming must observe, and the evaluation's pointer to geom/attribute.rs misses them. TrackTopologyChanges also keeps Arc<Mutex<ChangeState>> shared state (#L775).
- Error swallowing is inflated. There are 1,048 ignored/ignore_status call sites (helpers at B/crates/solidcore/src/error.rs#L38-L50), not about 1,100. Only about 490 discard the result outright: about 281 ignore_status statements and about 206 bare ignored(...) statements. The rest test the Option (if let Some…) or fall back to UNDEF or a default. Unported always propagates. The pattern mirrors upstream's ignored statuses, but 'errors silently swallowed at ~1,100 sites' overstates it.
- Hang handling is undersold. Of 34 HANG entries in B/plan/upstream-issues.md, only about 4 are kept unchanged (HANG-1 #L1653-L1660, HANG-12, HANG-32, HANG-39). At least 15 return NotConverged or InvalidInput past explicit bounds, among them HANG-2 to HANG-10, HANG-23, HANG-24, HANG-33 and HANG-34. The rest are not reached yet. The accurate summary is that the port guards UB and hangs but deliberately keeps about 88-89 of 120 wrong-result defects.
- 'Tolerances all absolute, not scale-invariant' leaves out SMLib's model-size mechanism. size_estimate_to_zone_tol_3d (B/crates/solidcore/src/geom/tol.rs#L306-L322) maps a model-size estimate to zone_tol in decade steps: 1e-3 for 5e4-5e6 units, and 10^floor(log10/2)*1e-5 beyond that. Context::new derives zone_tol from it (B/crates/solidcore/src/geom/context.rs#L236-L250). Tolerances are also settable through KernelOptions (kernel.rs#L131-L147). A 2 km site in mm gets zone 1e-3 mm. The full-scale criticism still holds, but it should rest on one tolerance per context, the 1000:1 ratio and the exactly compared parameter sentinels (math.rs#L56-L64), not on '1e-5 absolute'.
- Report arithmetic. Of the 235 files in reports/difftest, one is harness.selftest.json (28 cases, no status), so there are 234 port units and the right figure is 231 of 234 with max_rel_err 0. 120 'rejected' cases in 8 units are omitted: 493,865 + 24,694 = 518,559, not 518,707. Examples: blend.fillet_corner_p3 has 55, sweep.trans_sweep_geometry 19, topology.brep_p1 16.
- The AI share is understated and the commit counts are not at the pinned SHA. 3,762 commits are reachable from 6a019c5; 3,839 is a later HEAD. Of 2,253 non-merge commits, 1,857 carry AI co-author trailers and 393 are github-actions[bot] 'plan/PROGRESS.md: regenerate' commits. Only 3 have neither, so effectively every non-bot commit is AI-co-authored, not 83%. 'Human review at summary level' is a disclosure in #294 and about 21 PRs, not a repo-wide fact.
- The provenance chain omits IntegrityWare. Its SOLIDS++ history page (https://integrityware.com/products/SOLIDS++/history.html) says the 1998 SMLib kernel combined IntegrityWare technology (GSLib/TSLib/POPLib, plus its later fillets, shelling/offset and primitive creation) with GeomWare's, with SMS leading marketing. IntegrityWare keeps a contractual right and sells the kernel as SOLIDS++; Wikipedia says the alliance ended in 2004. Upstream files carry only NVIDIA SPDX headers, so whether NVIDIA's Apache grant fully covers the IntegrityWare-authored parts is a chain-of-title question (unverified). It belongs in legal check (b), alongside the patent question.
- Minor inaccuracies. SmBoolean does not uniformly stay i32: Tri enums at heal/face_props.rs#L43, enums or Option<bool> at heal/gap.rs#L49, bool at curve/iso_curve.rs#L34; i32 with UNSURE=2 survives in places such as curve/offset_curve.rs#L82-L83. SolidError::Unported carries no operation field (error.rs#L177-L183). The boolean module is about 13.3k lines including boolean.rs, not 10.7k. The corpus author is 'Iraklis Kourtis' (B/ATTRIBUTION.md). 'Shims replace analytic surfaces, so analytic branches are never compared' applies only to some B-rep shims (#294 row 5); surface.cone_p1 (20,702 cases), sphere and torus compare analytic cases.
- A missed point in favour of the verdict. The oracle need not be solidcore's Rust at all. harness/cpp (about 250 C-ABI shims with exceptions caught and input guards) plus the published prebuilt reference release cpp-ref-v1-c9c5979f04ce-… (B/README.md#L80-L101, fetched by scripts/fetch-cpp.sh) let Spare Kit difftest against SMLib itself without building it. That is the stronger oracle path: it removes the AI-port layer, and the Rust side remains a convenience for wasm and pure-Rust CI.

**Argument:** I checked the claims against the source at 6a019c57131d5fee6e57ab7dc4a62201a32e0216 (files and all 235 reports fetched; commits, PRs and issues through gh). The core facts hold:
- 3 crates; 383,677 lines across 304 .rs files; #![forbid(unsafe_code)].
- Radial-edge u32-index arenas with public from_index, *_mut accessors and add_*_record.
- Default tolerances 1e-5, 50, 1000, plus the sentinels.
- No HashMap; Instant appears only in a test.
- libm divergence between native and wasm, documented in findings.md#L46-L99. Wasm is only ever cargo-checked.
- Report totals: 518,707 cases, 493,865 passed, 24,694 deferred, 0 failed, 31,713 with nothing compared, worst error 1.78e-13. boolean.api_brep passes are all refusals.
- SSI corpus: C++ scores 81/25/11/2, Rust 3 PASS and 116 DEFERRED.
- 337 defects (176 UB, 34 hangs, 3 nondeterminism, 120 wrong results, 4 docs). Closure numbers and out-of-scope list match. NOTICE, ATTRIBUTION, deny.toml and upstream SPDX all as stated.

The errors listed above are real but none reverses the verdict. The most consequential:
- The oracle set is miscounted and mis-pinned: 65 green units, not 73.
- 'Green' can still hide stubs.
- The cache-history determinism claim is wrong.
- SMLib's topology Notify and TrackTopologyChanges history mechanisms are missed.
- Error swallowing is overstated.
- The model-size tolerance mechanism is omitted.
- AI authorship is understated: effectively every non-bot commit is AI-co-authored.
- IntegrityWare is missing from the provenance chain.

Case for a more generous verdict (PORT_METHODS or PORT_CODE): the NLib layer (nurbs/, about 62k lines, 45 green units) is close to pure. Only one top-level file mentions Kernel, nothing takes &mut Kernel, and it consists of value types with Result errors, bounds checks and bit parity with a 20-year commercial NLib (knots.rs#L1-L60). The port also departs from upstream where it matters for safety: hangs are bounded, UB is guarded, and allocation and recursion are capped. This is the strongest pure-Rust NURBS toolkit available under Apache-2.0. It still falls short of PORT_CODE under the owner's rule:
- f64 only, with absolute tolerance parameters, NL_MAXDEG=17 and some in-place &mut APIs.
- AI-transliterated with no independent correctness proof.
- Its algorithms are textbook (The NURBS Book), so a clean-room Spare Kit implementation checked against exact rational ground truth plus this oracle is the better route.
Its genuinely best-in-class parts are test methods (difftest rule, perturbation, fuzzing, call-graph closure, mathprobe), which the evaluation already takes.

Case for a harsher verdict (STUDY): the oracle is weaker than claimed:
- Green does not mean complete.
- nlib.basis_adv still disagrees with the C++ on 41 fuzz inputs.
- Bit identity holds only on x86-64 glibc.
- It adds no evidence beyond SMLib itself, and SMLib's SSI fails 38 of 119 exact-truth cases.
- A dev-dependency means compiling about 384k lines of fast-moving code. The crates.io release is a stale 0.0.1 pointing at bherbruck/solidcore, so Spare Kit would need a git pin.

Even so, an oracle remains useful: solidcore ships the C++ shims and a prebuilt SMLib reference, the SSI scorer port and committed fixtures. Used as recorded golden outputs on well-conditioned NURBS and curve inputs, with exact ground truth deciding disagreements, it is cheap and real.

DIFFTEST_ORACLE stands, with these corrections:
- Pin the oracle to the 65 green units, or better to the C++ through solidcore's harness/cpp and its prebuilt reference.
- Use recorded outputs rather than a live dependency.
- Study topology/topology.rs Notify and TrackTopologyChanges for naming events.
- Add IntegrityWare chain of title to the legal checks.
