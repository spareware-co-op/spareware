# 32. Decisions for the owner

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Proposed in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Almost all of the set's 289 decisions and 212 open questions are technical. Kernel owners settle them in the pieces listed in [31.7](31-risks.md#317-open-technical-questions). The twelve below need the owner, because each one fixes a promise to users, spends money, carries legal weight, or decides who decides. Each says in plain words what is being decided, gives the options and our recommendation with the reason, and names the chapters it changes.

Accepting G01 changes architecture decisions: it replaces three choices in Spare Construct's map. The README puts such a change to a vote of the members, which an owner opens. Section 32.5 recommends one vote that covers G01, these decisions and one consolidated note of spine amendments, rather than a poll for each.

One standing rule is not reopened here. DWG and other formats known only through reverse engineering stay rejected unless an owner clears them after legal advice (README).

No agent acts on any of these, writes to anyone, or signs anything. The owner decides.

| | Decision | Recommendation | Chapters | Needed before |
| --- | --- | --- | --- | --- |
| 32.1 | The grid ladder (decision 1) | Three steps, as written | 6, 14, 17, 19, 24 | G02, under way and built to option A |
| 32.2 | Exact arcs in 2D from the start | Yes, gated by an early benchmark, with named fallbacks; never grid-rounded centres | 6, 7, 9, 14, 15, 17, 18, 24 | G192, the first geom-2d piece |
| 32.3 | The tolerance numbers, how errors add up, and what "certified" means | Numbers as written; results re-proven, fits still add; interim states only when labelled | 6, 9, 11, 12, 13, 15, 16, 19, 28 | The first curved piece in phase 5 |
| 32.4 | The same-answer guarantee: which computers, what it covers, who pays | Chapter 21's list, and Windows on Arm once its CI leg merges; valid results only; co-op-funded test machines. **Decided 2026-10-27: A, at no new cost (hosted runners; owner's machines)** | 21, 23, 26, 28 | G10, the CI matrix |
| 32.5 | Who decides: kernel owners, and how spine changes are accepted | At least two named people; one member vote, then delegation | 23, 26, 27, 28, 29 | The first kernel merge (G02) |
| 32.6 | Licence lines for code and tools | Run copyleft tools as separate programs; never compile Numerical Recipes code; keep CC0, take the Unlicense only beside another licence | 28, 29 | G113, the oracle runner |
| 32.7 | Patent searches: who, when, and who pays | A funded patent lawyer, booleans first, with a named fallback | 11, 13, 16, 17, 19, 29 | G123, the first geom-poly piece |
| 32.8 | How closely we follow NLib | Methods from the books; NLib for failure cases only. **Decided 2026-10-24: A, for all of the SMLib release:** ported, notices kept; Numerical Recipes routines and fixtures still excluded | 4, 12, 13, 16, 29 | G56, the first NURBS piece |
| 32.9 | Who we write to | EMBER, Schmidt and McNeel now; the rest in a stated order | 11, 14, 16, 17, 24, 29 | Varies |
| 32.10 | Revit truth data, and watching proprietary programs | Co-op models only; watching only checks our results against our own rules | 14, 28, 29 | G460, the Revit harness |
| 32.11 | The licence of our test cases | CC0 if the vote confirms test cases are not "core", otherwise MPL-2.0; strict rules for outside data | 28, 29 | G97, the fixture format |
| 32.12 | Pieces built ahead of the board: merge order, cost rises for correctness, goldens before the first edition | Merge in the board's order; accept a recorded rise; kernel owners approve goldens until the first Stable release | 23, 28, 30 | The next merge after G02 |

The board (30.5) also turns into a piece every narrower question that chapters 6 to 29 leave to an owner. Each is decided in its own section below, under the owner's standing rule of 2026-10-27: "Research and choose the best options or use the research we have". The stored research comes first (`projects/kit/notes/G01-evidence/`, chapter 29), then the chapters the piece names, and fresh reading only where those are not enough; each section says which.

| | Owner action | Decision | Chapters | Needed by |
| --- | --- | --- | --- | --- |
| 32.13 | G08: the crate table and layers | Chapter 27's proposed crate table, thirteen layers and rules, and 30.5's four amendments, all as proposed | 5, 27, 30 | G09, the scaffold |
| 32.14 | G44: rotations | One rational rotation type for 2D and 3D, integer quaternions; general angles the closest within ε_ang, Certified; kept as a transform where possible; applied with one rounding, planes committed, so 6.2's 384 bits stand | 6, 7, 14 | G46, rational directions and rotations |
| 32.15 | G71: time budgets | Chapter 28's reading: only work units and logical memory are budgets; a time limit is the caller's deadline through cancellation, never a replayable outcome | 10, 23, 28 | G83, budgets in the operation contract |
| 32.16 | G72: the reference machine and the kernel's shares | The owner's spare tower, timed on its 8 performance cores; 23.1's shares, browser estimates and 5 µs overhead accepted until the scenario benchmarks measure them | 23, 28 | G107, the reference runner |
| 32.6 | G75: copyleft tools as CI oracles | Yes, as 32.6 recommends: CGAL, OpenCascade, IfcConvert and the others run in CI as separate programs only, from a separate repository, never linked, shipped or pushed to a registry | 24, 28, 29 | G113, the oracle runner |
| 32.17 | G76: arena indices in name order | Accepted as 8.2 proposes: the builder assigns indices in persistent-name order in every operation, so arena order, tie order and hash order are one; a shuffle test holds every operation to it | 8, 23, 24 | G81, the builder |
| 32.18 | G77: the CBOR tag and the file name | Tag 0x534B4746 ("SKGF"), unassigned, requested from IANA by the owner; the name SKG stays; the extension becomes `.skgf`, because Skrooge already uses `.skg` | 24 | G93, the native format specification |
| 32.19 | G78: NameScheme v1 | SHA-256 cut to 16 bytes over a domain prefix and the name's deterministic CBOR; fixed variant numbers; NFC under Unicode 18.0.0; `Bounded` gains `ends`, collisions stay `Weak` | 11, 20, 24 | G87, persistent names |
| 32.20 | G79: hash functions | SHA-256 for content and, cut to 16 bytes, for NameIds; RustCrypto's `sha2` 0.11 approved, added to the no_std allow-list with its audit by G86 | 20, 23, 24 | G86, the canonical encoder and safe hash |
| 32.21 | G120: committed planes and how bounds add up | Committed planes and points stay Certified; 32.3's option B: bounds per entity, added along its own history, against each step's stored inputs; ε_step = 16q joins the ladder | 6, 9, 11, 13 | G142 and G143 |
| 32.22 | G121: the predicate step ladder | Identity, narrow exact and axis steps before the filter join 6.2; each step checked against the exact path; a filter switched off per target only where the count benchmarks show it loses | 6, 23 | G124, the predicate step ladder |
| 32.23 | G157: SMLib as a CI oracle | Yes, NLib's evaluation, knot operations and degree elevation only, out of process; its Numerical Recipes routines stubbed out, never compiled (32.6's third question: no) | 12, 29 | G161, the SMLib and geomdl oracles |
| 32.24 | G184: the pattern and linetype library process | Authored from written rules (ISO 128, standards, textbooks) or own design, never from vendor or CADCraft files; our own names; a provenance log per entry; a kernel owner's review in each pull request | 14, 29 | G211, our own library |
| 32.25 | G185: ellipses in 2D | A curve kind stored by centre and major axis on the grid and a reduced axis ratio p/q, q at most about 2^21; file ratios become the nearest such fraction, Certified unless exact | 7, 14, 26 | G190, exact conic predicates |
| 32.26 | G186: depth-one 2D numbers | Integer carriers, depth-one points and rational-centre arcs accepted into 6.2, at most 337 bits; 32.2 decided A with its fallbacks | 6, 14, 15 | G187, the 2D predicates |
| 32.27 | G238: toolkit homes before a second program | No exception: `geom-thread`, `geom-stair` and `geom-civil` start in Spare Construct on the facade, and enter the kernel under 26.1 when a second program needs them | 17, 26, 27 | G258, G281 and G378 |
| 32.28 | G239: what the Sweep kind stores | The whole definition: profile, path, the full frame rule, twist and scale laws, bit-exact station frames, a tapered revolve's profile morph; the helix kind gains a radius rate, so conical helices are exact | 7, 13, 24 | G254, frame rules and laws |
| 32.29 | G240: 576- and 768-bit predicates | Accepted into 6.2 for skeleton (`I576`) and mitre-offset (`I768`) event order only, each with a written derivation, a certificate, extreme tests and a filter, and measured; `I512` and `geom-math`'s wide stages accepted as built | 6, 13 | G251, the exact straight skeleton |
| 32.30 | G241: the close_near_contacts cap | ε_heal's values as a second meaning: default 640q, at most 6,400q; larger requests refused | 6, 11 | G249, the close_near_contacts repair |
| 32.31 | G285: the contact benchmark's licence | Wang et al.'s 2021 benchmark has no licence (all rights reserved), so it is not stored or run until the authors clear it, on a request the owner sends; G297 tests meanwhile on the authors' Apache-2.0 time-of-impact sample and our own exact cases | 19, 29 | G297, time of first contact |
| 32.32 | G312: USD exchange home | `geom-meshio` owns USD subdivision exchange; Chaikin creasing travels as `uniform token spareware:creaseMethod = "chaikin"` on the Mesh prim, absent meaning uniform | 16, 24 | G354, SubD exchange |
| 32.33 | G357: alignment curve kinds | One transition kind with a closed set of curvature laws (Helmert, Bloss, cosine, sine, the cant-coupled transition, IFC's polynomial spirals, the cubic spiral); the cubic parabola a polynomial curve; gradient and segmented reference curves compositions in `geom-align`; descriptive names, file enumerators kept as data | 7, 17, 26 | G371, transition kinds |
| 32.34 | G358: TIN tie order | Plan position, x then y, for a TIN's Delaunay ties, as chapter 17 asks; name order everywhere else | 6, 17, 21 | G362, the exact Delaunay core |
| 32.35 | G384: smooth-edge marking | An explicit, logged operation marks edges Smooth below an angle θ the caller passes and the log records; no kernel default, no ladder rung | 6, 18 | G398, line classes and continuity records |
| 32.36 | G385: view predicates beyond 384 bits | `I576` for grid-axis views and `I640` for any direction and perspective, accepted on 32.29's conditions (derivation, certificate, extreme tests, filter, measured) | 6, 18 | G388, exact view predicates |
| 32.37 | G407: openNURBS licence confirmation | Ask McNeel in writing (request drafted, the owner sends it); nothing ported until the reply; open until then | 16, 24, 29 | G438, Rhino .3dm |
| 32.38 | G408: LandXML terms | No terms found: the schema is implemented from its definitions, never copied or generated from, and fetched only to validate; LINZ's examples fetch-only until their page states terms | 17, 24, 29 | G437, LandXML 1.2 mapping |
| 32.39 | G409: generating code from STEP and IFC schemas | Part 42 is the 2025 edition; IFC schemas generated from with attribution; ISO's STEP schemas only with a file's own grant or ISO's permission, requested in a draft the owner sends | 24, 29 | G416, EXPRESS code generation |
| 32.40 | G447: Gerber and IPC-2581 terms | No second program needs board exchange, so G474 and G475 wait; Gerber implemented exactly as published, no extensions; IPC-2581 only after its own text or IPC's word | 24, 26, 29 | G474 and G475, board exchange |
| 32.41 | G448: AutoCAD truth tests | 32.10's ruling stands; no seat is bought or borrowed; G464 checks drafting by 28.3's laws, the one-root reference and LibreCAD and ezdxf, and adds AutoCAD when a co-op seat and a cleared terms check exist | 14, 28, 32 | G464, drafting truth tests |
| 32.42 | G449: the wall-clock merge rule | Accepted: a nightly miss confirmed by a same-night rerun blocks merges to `main` except the fix or a revert; deliberate rises recorded as 32.12 says; no run, no block | 23, 28 | G452, wall-clock and scaling gates |

## 32.1 The grid ladder (decision 1)

**What it is.** Every point the kernel stores sits on an invisible grid of tiny steps. The step size decides which measurements are stored exactly, and how large an area one coordinate frame can cover. A finer step makes more units exact but shrinks the area.

| Option | What it means | For | Against |
| --- | --- | --- | --- |
| A. Three steps, as written | 156.25 nm for buildings, sites and roads (±171.8 km per frame); 6.25 nm for parts and circuit boards (±6.87 km); 0.25 nm kept in reserve. Each step is 25 times finer than the one above, so moving to a finer step loses nothing | Millimetres, 0.01 mm and 1/256 inch exact for buildings; microns and mils exact for parts; a 50 km road fits in one frame | Gerber board files give coordinates to 1 nm, which rounds by up to 3.1 nm at 6.25 nm; the rounding is recorded |
| B. One grid of 31.25 nm for everything | A single step size | Microns exact everywhere; simpler to explain | ±34 km per frame, so long roads need several frames; mils are not exact. The fit bound is 64 steps, so it becomes 2 µm everywhere: five times looser than 0.4 µm for parts, and five times tighter, so costlier, than 0.01 mm for buildings |
| C. As A, but circuit boards use the 0.25 nm step | Boards get a finer frame | Millimetre Gerber files (1 nm) are exact; ±275 m is ample for any board | Boards differ from other parts; a board placed in a large building moves more of the building into the finer frame. Inch Gerber files resolve one micro-inch (25.4 nm), which is exact at neither 6.25 nm nor 0.25 nm (24) |

**Recommendation: A.** It covers every unit Spareware's programs use, and it keeps a 50 km road corridor in one frame. The Gerber rounding is far below anything a board maker can produce, and it is recorded. If exact Gerber round trips ever matter, C needs no change to the core, because any frame can already pick any step.

**Changes:** chapter 6, and chapters 14, 17, 19 and 24, which build on it.

## 32.2 Exact arcs in 2D from the start

**What it is.** In a drawing, lines and arcs meet at fillets, cross each other and snap to each other. The kernel can answer "do these touch?" exactly for arcs too. Or it can keep every circle centre on the grid, as the brief literally says, which leaves hairline gaps at fillets. Exact answers mean no gaps at fillets, exact snaps and exact offsets. The cost is very wide numbers for some constructions, possibly thousands of bits. That cost has not been measured on any target, and even the bit widths are estimates, not yet derived (14).

| Option | What it means | Kind of change |
| --- | --- | --- |
| A. Exact lines, arcs and ellipses from the start | Chapter 14 as written. Drawn and imported circles keep their centres on the grid. Constructed circles (fillets, tangent circles, three-point and bulge arcs) store their centres as exact fractions | None: as written |
| B1. Ellipse pairs certified | Decisions between two ellipses get a proven bound instead of an exact answer. Lines and arcs stay exact | An exception to principle 1, written down |
| B2. A coarser angle target for constructions that stay in 2D | Constructed tangencies are placed to a looser angle than ε_ang, so their fractions need fewer bits. Every decision stays exact | A change to the tolerance ladder (6.5) |
| B3. Certified decisions on constructed circles | Decisions that involve a constructed circle get a proven bound instead of an exact answer | An exception to principle 1, written down |
| C. Circle centres rounded to the grid (the brief's literal wording) | Every fillet then misses its lines. A 5 mm fillet at the building step opens a 47 µm gap, five times the fit bound, with a 0.54° kink. At the parts step the gap is about 9 µm, 24 times that step's fit bound (14) | Breaks the tolerance ladder |

**Recommendation: A, gated by piece "2D-01"'s benchmark.** Each fallback applies only to the class of predicate whose cost fails. For ellipse pairs, use B1. For constructed circles, try B2 first, because it keeps every decision exact, and use B3 only if B2 is not enough. Never take C. Why: this choice is the base of drafting, sketches (15), survey linework (17), drawings (18) and file exchange (24). Changing it later would change stored files and every 2D operation. That is the mid-course re-architecture that sank Fornjot (R1). C fails the tolerance ladder anyway.

**Decided 2026-10-28 with owner action G186 (32.26): A, gated by piece 2D-01's benchmark,** with the fallbacks as recommended, each only for the class of predicate whose cost fails, and never C.

**Changes:** chapters 14, 15, 17, 18 and 24; section 6.1, which gains an exception for rational centres; section 7, where an arc may then be stored by three points or by its end points and a bulge; and section 9's table.

## 32.3 The tolerance numbers, how errors add up, and what "certified" means

**What it is.** Curved shapes cannot be stored exactly, so each one carries a proven error bound. The numbers set how close is close enough:

- the fit bound: 0.01 mm for buildings, 0.4 µm for parts and boards;
- the healing bound: gaps in imported files are closed up to 0.1 mm by default, 1 mm at most, for buildings, and 25 times less for parts;
- the angle bound: about 6 × 10^-11 radians.

The second question is whether each operation's error is added to the error before it. Read literally, a curved part would refuse further booleans after about 128 of them (11), and a fillet on a fillet would eventually refuse too.

| Option | What it means |
| --- | --- |
| A. The numbers as written; errors add on every operation | The literal reading |
| B. The numbers as written; each result is re-proven against the exact surfaces it is built on | Booleans and procedural kinds add no error, because faces never move and new edges are defined by the surfaces that meet there (11). Fits still add along each face's own history: each fitting step may spend up to a quarter of the fit bound (13.2). The derived values the chapters need get names on the ladder: that per-step share, a floor of one grid step, a smallest triangle height of 8 steps, and the sketch solver's constants |
| C. Looser numbers, for example 0.1 mm for buildings | Curved work is cheaper and less exact |

**Recommendation: B.** The promise then reads: curved geometry stays within 0.01 mm of each step's exact result on its stored inputs (6.5).
- Through booleans and procedural operations, that holds however long the history.
- Through fitted operations, such as lofts, knot merging and subdivision-to-NURBS conversion, it holds until the fits on one face use up the fit bound. That is about four fits at full budget. The next fit then refuses with ExactnessNotAccepted, section 22's kind for a bound past the accepted one, and its remedies, never silently.
- It is not a bound against replaying the whole history exactly. Near-tangent blends and shallow intersections can amplify an error in their inputs (13).

Why: it keeps the 0.01 mm promise without loosening it, and says plainly where it stops. Confirm the fit bound as the default for road earthworks only after the corridor benchmark (17).

**What "certified" allows.** Four chapters ask what principle 2 accepts. The rule behind all four answers: an interim or weaker state is allowed only when it is labelled where users and code can see it, never silently.

| Question | Recommendation |
| --- | --- |
| A Taylor-style bound: an evaluated value plus a proven bound on the second derivative (12.12) | Counts as certified, provided the derivative bound itself comes from interval or Bernstein arithmetic. It is a proof, not sampling |
| Rotation-minimising frames: the shape is certified against its stored stations, but its closeness to the ideal frame is measured, not proven (13) | Acceptable, because the stored definition is the shape. The measured difference is reported and labelled "measured". A proven bound stays a research piece |
| Filters labelled "checked, not proven", if neither FPTaylor nor Gappa works (28) | Acceptable while the module is below Stable, labelled, and backed by the exact oracle. Before the module reaches Stable, each filter is proven or switched off, so that predicate always takes the exact path |
| Field meshes at sharp CSG edges, where no certified method exists (16) | Refuse with remedies (finer depth, the B-rep route, or explicit Approximate), as chapter 16 does. No new class for "distance certified, topology unproven" |

**Decided 2026-10-28, how errors add up (owner action G120, 32.21): B,** by 13.2's one rule: bounds per entity, added only along its own history, each step measured against its stored inputs, with ε_step = 16q as each step's share. Committed planes and points stay Certified.

**Changes:** chapters 6, 9, 11, 12, 13, 15, 16, 19 and 28.

## 32.4 The same-answer guarantee: which computers, what it covers, and who pays

**What it is.** The kernel promises bit-identical results everywhere. That is what lets a shared model be replayed from its log and agree on every machine. A kind of computer is really covered only if a test machine of that kind checks every change, and those machines cost money every month. A second question is what the promise covers when a better checker finds an old result was invalid.

| Option | What it means |
| --- | --- |
| A. Chapter 21's list | Windows, Linux and macOS on Intel and AMD chips; Linux and macOS on Arm chips; WebAssembly, which every browser runs. Old 32-bit PCs and some 32-bit Arm chips that break the floating-point rules are left out |
| B. A shorter list, to save cost | For example, dropping Intel Macs when GitHub retires those test machines. GitHub retired its macOS 13 image on 4 December 2025 and said x86-64 macOS ends when the macOS 15 image retires in fall 2027 ([changelog, 2025-09-19](https://github.blog/changelog/2025-09-19-github-actions-macos-13-runner-image-is-closing-down/)); its [runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), read 2026-10-22, also lists a `macos-26-intel` image, so the date is not settled. The kernel's x86-64 macOS leg (`macos-15-intel`) then needs a self-hosted machine, or this option |
| C. A longer list | Adding Windows on Arm, phones or RISC-V as test machines become available. G02's kernel CI already runs a Windows on Arm leg (`windows-11-arm`) on every pull request (21) |

**Recommendation: A now, with three rules, and Windows on Arm added once G02's CI merges.** Windows on Arm then meets the first rule below and costs nothing; if the owner declines it, the leg is removed.
- A kind of computer joins the list only when a test machine checks it on every pull request. Today that means the native targets, and WebAssembly in Wasmtime and Node (21). Browsers run the same WebAssembly and are checked nightly in Chrome, Firefox and Safari (23).
- A kind of computer leaves the list only at a new kernel edition, announced in advance.
- Within an edition, results never change for valid results. If a better checker finds that an old operation version returned an invalid body, replaying it now fails, and that change is reported element by element, like an upgrade (26.2, 28).

**Who pays.** Recommendation: fuzzing on Google's free OSS-Fuzz if it accepts the project, otherwise on co-op machines. The co-op funds the CI runners, a dedicated benchmark machine and the licensed Revit and AutoCAD machines (28), with a line in K06 once owner action "CI hosting and cost" records the cost. Today every pull-request leg runs on GitHub's standard hosted runners, which are free and unlimited for public repositories ([runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), read 2026-10-22), and this repository is public. So the per-pull-request matrix costs nothing until GitHub's Intel macOS images end; the benchmark machine, nightly fuzzing, the oracle registry and the licensed machines still need the co-op's money.

Why: a guarantee without a machine that checks it is a hope, not a guarantee. And no kernel can promise to keep returning a body it now knows is invalid.

**Decided 2026-10-27 (owner action G07, under the owner's standing rule to decide by research: "Research and choose the best options or use the research we have"): A, as recommended, at no new cost.** Every pull-request leg runs on GitHub's standard hosted runners, free for this public repository, Windows on Arm included. Fuzzing goes to OSS-Fuzz if it accepts the project, and meanwhile runs on GitHub (ClusterFuzzLite, kernel-fuzz-campaign.yml) and on the owner's own machine overnight. The reference benchmark machine is the owner's existing workstation (G72 documents its spec), and the licensed Revit machine is the owner's existing one; nothing is bought. The question returns when GitHub ends its Intel macOS images (2027). Recorded by piece G10. On 2026-10-28 the owner made a second machine available, a spare tower free each night from 19:00 to 08:00, and it became the reference benchmark machine in place of the workstation, which stays the build and Revit machine; the oracle images keep their layers in the oracle repository's free Actions cache rather than a paid registry. 28.17's "As built (G10)" gives the machines and the reasons.

**Updated 2026-10-28 (owner action G07, closed).** The same day, the organisation moved from GitHub's free plan to GitHub Enterprise, with one seat, at US$21 a month ([pricing](https://github.com/pricing), read 2026-10-28). Nothing else about where checks run changes: every workflow still runs on GitHub's standard hosted runners, and their minutes stay free because this repository is public. What the plan buys is capacity. GitHub's free plan runs 20 jobs at once, 5 of them on macOS; Enterprise runs 500, 50 of them on macOS ([Actions limits](https://docs.github.com/en/actions/reference/limits), read 2026-10-28). One push to a kernel pull request starts about 125 jobs, so on the free plan each push waited hours behind the others; now a push runs whole. The plan also includes 50 GB of private package storage ([GitHub Packages billing](https://docs.github.com/en/billing/concepts/product-billing/github-packages), read 2026-10-28), but the oracle images do not use it: 29.20's rule 8 says they are never pushed anywhere, a private registry included, because a copy of a GPL program handed to anyone else is a distribution that would bring its source obligations. They stay in the oracle repository's Actions cache, which only that repository's workflows read; an image the cache evicts (10 GB per repository, entries unused for a week) is rebuilt from its pinned source, which costs time, not money. K06 records the plan as the kernel's one bought cost. The question returns when GitHub ends its Intel macOS images (2027), or if the seat count changes.

**Changes:** chapters 21, 23, 26 and 28, and K06.

## 32.5 Who decides: kernel owners, and how spine changes are accepted

**What it is.** Two gaps.
- The chapters send dozens of approvals to "a kernel owner": changing a recorded correct result, accepting a reference implementation, adding a source to the ledger, reviewing a method note or a proof. Nobody is named. Agents never approve or merge (AGENTS.md section 7), and nothing merges on AI review alone (README).
- The README puts any change to an architecture decision to a member vote, and AGENTS.md section 7 allows a change to ARCHITECTURE.md decisions only through a decision note an owner has accepted. G01 replaces three choices in Spare Construct's map, and 31.7 proposes dozens of amendments to the spine (chapters 6 to 10, 20 and 27). Nothing yet says who accepts them.

| Option for kernel owners | What it means |
| --- | --- |
| A. The owner alone | Every approval waits for one person |
| B. A small named group of at least two people | At least one of them can review computational geometry and proofs. Agents may prepare reviews but never count as an approval |
| C. A larger rotating group | More capacity, less continuity |

**Recommendation.**
- **Kernel owners: B, named before the first kernel code merges.** Proposed as a new rule: every change to the foundation crates (geom-num, geom-poly, geom-check, geom-name, the topology builder and the 2D arrangement) needs a kernel owner's review. 28.8 defines this set only for mutation and coverage today. If no kernel owner can review a proof deliverable (R25), it goes to a paid or academic reviewer, recorded on the piece.
- **Spine changes: one vote, then delegation.**
  1. G01, the decisions in this section, and one consolidated amendment note go to a single member vote. The note gathers every spine amendment in 31.7 that can be settled now, and the crate list (27).
  2. After that vote, kernel owners accept decision notes for operations, queries, kinds, formats, toolkits and crates, as 26.1 allows. That includes spine amendments that wait on a piece's measurement.
  3. The principles (section 3), the grid and tolerance ladders (6.1, 6.5), the same-answer and edition promises (21, 26.2) and the licence stay with the owner, and go to a member vote when a change is major.
- **Spare Construct's targets:** kernel owners and Spare Construct's owners set the kernel's share of each target together. 23.1's proposed figures stand until then.
- **Already waiting on kernel owners:** the goldens that G02 to G06, G25 and G32 set or change (28.13, 32.12), and the review of every change to the benchmark gate's workflow and `geom-bench`, on which the gate's integrity rests (23.13).

Why: without names, every approval falls to the owner, which is the bottleneck in R22. With two people, one can review the other's work. One vote keeps the README's rule without a poll for every amendment.

**Changes:** chapters 23, 26, 27, 28 and 29.

## 32.6 Licence lines for code and tools

**What it is.** GPL, AGPL and LGPL licences require anything built from the code to carry the same licence. Spare Kit never copies, links or ships such code. Five related questions remain.

| Question | Recommendation | Why |
| --- | --- | --- |
| May CI run GPL and LGPL programs (CGAL, OpenCascade, IfcConvert, SolveSpace, FreeCAD's solver, LibreCAD, Gappa) as separate programs whose answers we compare with ours? Their harnesses would live in a separate repository, their container images would never be published, and GPL test data would be fetched at test time and never stored | Yes | Running a program is not copying it. No exact outside check exists for curved booleans, and these tools catch bugs no permissive tool can. Without them the set of outside checks shrinks to permissive tools |
| May a contributor read GPL source for a method nothing else covers? | Only with an owner's recorded yes, under the method-only procedure (29's rule). Otherwise, papers and documentation only | This keeps principle 11's line without blocking a method that has no other source, and every exception is on record |
| May unpublished test builds of SMLib and solidcore compile their routines derived from Numerical Recipes? Rule 8 (29.20) allows it today after an owner's yes | No: the owner does not give that yes. Stub the routines out, or leave those builds out | Compiling makes copies, and that code's terms do not allow them, published or not. The cost is real. Stubbing out LU, SVD, the Brent minimiser and the integrator leaves SMLib checking only evaluation, knot operations and degree change. Fitting and mass properties lose it, and probably parts of intersection (29). 12.17's SMLib timing reference, and its proposed runs on SMLib's best classes, fall back to OpenCascade run out of process. The SSI gate itself is unaffected, because it is scored against the corpus's exact truth. The alternative is rule 8 as written: unpublished builds after an owner's yes, never distributed, accepting that private copies of that code are still made |
| Is the method-only procedure the standard for every LGPL source? (One session reads the source and writes a plain-words note, a reviewer checks the note, and a different session writes the code) | Yes | It costs a second session. It is also the only evidence we can show if anyone claims we copied |
| Should the Unlicense and CC0-1.0 stay on the code allow-list (29.2), given doubts about public-domain dedications in some jurisdictions? | Keep CC0-1.0. Take Unlicense code under its other permissive licence where one is offered, as many Rust crates do. Unlicense-only code becomes a licence exception for an owner (29.20) | CC0 carries a fallback licence for places where a dedication to the public domain does not work; the Unlicense has none. This is our reading, for counsel to confirm |

**Decided 2026-10-28 (owner action G75): the first question, yes, as recommended.** GPL and LGPL programs (CGAL, OpenCascade, IfcConvert, SolveSpace, FreeCAD's solver, LibreCAD, Gappa and the like) may run in CI as out-of-process oracles. Read with 24.12, 28.10, 29.2 and 29.20's rule 8, and evidence report [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) on what each oracle catches.
- **What it allows.** Each program runs as a separate process in its own CI container and exchanges only files with our code. Its harness lives in a separate repository under the licence the program requires; our repository holds only the jobs and the comparison code (28.10).
- **What it never allows.** No kernel crate, test crate or release links, compiles in or ships any of it. Its container images are built from pinned source and never pushed to any registry, private ones included (rule 8; owner action G07 keeps them in the oracle repository's Actions cache). GPL test data is fetched at test time and never stored. An oracle is never the truth: where it disagrees with exact or interval ground truth, ground truth decides (29.1).
- **Why yes.** Running a program is not copying it, so the kernel's licence line (principle 11, no GPL or AGPL code) is untouched. No exact outside check exists for curved booleans, and these tools catch bugs no permissive tool can: OpenCascade's `checkshape` and `bopargcheck`, CGAL's exact Nef polyhedra and arrangements, IfcConvert on our IFC output. Refusing them would shrink the outside checks to the permissive tools alone.

The other four questions of this section stay as they are: reading GPL source still needs an owner's recorded yes per method; the Numerical Recipes routines in oracle builds are answered by owner action G157 (32.23): stubbed out, never compiled; the method-only procedure and the Unlicense and CC0-1.0 rows are not part of this action.

**Changes:** chapters 28 and 29, and the oracle pieces of chapters 11, 12, 15, 16, 18 and 24.

## 32.7 Patent searches: who searches, when, and who pays

**What it is.** A patent protects a method, so writing our own code does not escape it. Searches so far covered only subdivision, T-splines, watertight trimming and lattices, and even there 18 adopted methods were never searched (13, 16). Sixteen families have had no search, booleans among them. The EMBER method sits under the whole flat-solid core (11, 29.17). In the United States, knowing about a patent can raise the damages if it is later found to be infringed. That is why some organisations leave patent reading to lawyers. The co-op has no budget for this: K06's cost model leaves legal advice out, because no public rates were found.

| Option | What it means |
| --- | --- |
| A. Agents search and record what they find in PATENTS.md; the owner reviews | Cheap, but agents then read patents |
| B. A patent lawyer searches the core families first (booleans, then blends, offsets and naming) and records conclusions; agents do not read patents | Costs money; protects the co-op best |
| C. No searches; avoid only patents we already know of | The CI gate in 29 becomes meaningless |

**Recommendation: B, funded by the co-op.**
- The lawyer covers the boolean core, EMBER first, before the first geom-poly code merges, then the other families in build order. The same lawyer can check the trade mark and any patent on "Viennese Bend" before the spiral piece (17).
- Chapter 29's CI gate stays: no family reaches the Build stage without a recorded search and an owner's review.
- No cost estimate exists. The owner sets a budget, and K06 gains a line for legal advice.
- If no lawyer can be funded in time, the fallback is A for every family, with the owner accepting in writing the knowledge risk described above. Under no option is the boolean core built unsearched.

Why: one patent on the boolean core would touch every model ever saved, and a search costs little next to rebuilding the core.

**Changes:** chapter 29, the pieces of every family in chapters 11, 13, 16, 17 and 19, and K06.

## 32.8 NLib: how closely we may follow it

**What it is.** NVIDIA opened SMLib in October 2026 under Apache-2.0. Its NURBS library, NLib, is the best permissive source of production NURBS methods. But NVIDIA's grant covers only "NVIDIA-authored portions". NLib came to NVIDIA through GeomWare and SMS, some modules came from IntegrityWare, and some routines are derived from Numerical Recipes. Who owns what has not been verified ([13], 29).

| Option | What it means |
| --- | --- |
| A. Sign off the ownership now and port NLib closely | Fastest; carries the ownership risk into the kernel |
| B. Keep chapter 29's narrowing | Consult NLib only for which methods exist, their failure cases and test ideas. Re-derive every method from The NURBS Book and Piegl and Tiller's papers. Ask NVIDIA through an issue, and revisit when they answer |
| C. Do not consult NLib at all | Safest; loses its failure cases and test ideas |

**Recommendation: B.** The published book and papers give the same methods. A kernel that can never be relicensed cannot afford code whose owner is unclear. B still keeps NLib's real value to us: its failure cases and test ideas. "Notices kept" then means a credit line in each module that consulted NLib, plus an entry in geometry/NOTICE. It does not mean NVIDIA's copyright line, because those modules copy nothing and our contributors keep their copyright.

**Decided by the owner, 2026-10-24: A, for all of the SMLib release.** The owner's words: "How do you get Nvidia authored code? If you can see it just make it in rust. You trying to get to the same thing anyway". In plain words: NVIDIA published this code under Apache-2.0, and the owner accepts that publication as enough. We are building the same methods anyway, so where the code is in front of us we may translate it rather than rebuild it blind.
- The decision covers all of NVIDIA's published SMLib release (USD BRep): the SMLib core, NLib and the IntegrityWare-lineage (`Iw*`) modules. The gates that waited for an owner to sign off their chain of title are lifted (29.5, 29.6).
- This code may be ported, as 29.1's PORT class defines: translated into our Rust and rebuilt to section 3, with our types, exact predicates where topology is decided, certified bounds, deterministic maths, and typed errors in place of SMLib's silent fallbacks, tolerance globals and heuristics. Each ported file keeps its Apache-2.0 notice, NVIDIA's copyright lines included, says what we changed, and has an entry in `geometry/NOTICE` (29.20, rule 4).
- Two exclusions stand, because they rest on where the code came from, not on who published it. The routines derived from Numerical Recipes, in SMLib's core and in NLib, as 29.6 and 12.17 name them (in NLib: `N_SingleValueDecomposeSolve`, `N_RealMatrixSingleValueDecompose`, `N_FuncFindMinima`, `N_RealMatrixEigenValuesVectors` and its pythag helper, `M_TridiagonalSystem` and `M_CorneredTridiagonalSystem`): Numerical Recipes' own licence forbids reuse whoever republished them, so they are written from the mathematics. And SMLib's fixtures, which are SMS-era customer files.
- Nothing else changes. The NURBS Book and the Piegl–Tiller papers stay the first reference. The critique of SMLib's intersector in 12.17 stands: its uncertified heuristics are not ported as behaviour. IntegrityWare's patent claim on shelling and offsetting still needs its search and an owner's review first (29.17, 32.7). The oracle builds still wait on 32.6 (rule 8). Every other source keeps its own gate.
- The risk left: if a predecessor owner ever showed that NVIDIA lacked the right, the affected ports would be rewritten from the primary sources, which their Source and credit lines make findable (R17).

**Changes:** section 4 (option D), and chapters 12, 13, 16 and 29.

## 32.9 Who we write to, and when

**What it is.** Section 29.21 lists people and projects worth a conversation. None has agreed to anything. Only the owner decides whether to write, and no agent sends anything.

| Who | Why we would write | Recommendation | Changes |
| --- | --- | --- | --- |
| EMBER's authors at RWTH Aachen (Philip Trettner, Julius Nehring-Wirxel, Leif Kobbelt) | To ask for reference code or test data under a permissive licence, and for their view on 40-bit bounds and deterministic parallel assembly | Write now. Leave any patent question to the lawyer of 32.7 | 11, 28 |
| Tim Schmidt, author of csgrs and the Hyper crates | To compare certified-predicate methods and exact oracles, confirm hypermesh's history before any reuse, and swap hard test cases | Write now. It costs little, and nothing of theirs is planned for our code | 11, 29 |
| Robert McNeel & Associates | For written confirmation that openNURBS's "no restrictions" grant covers the repository at a named commit, including use inside MPL-2.0 work | Write now. 3dm exchange and Rhino subdivision exchange wait for it | 16, 24, 29 |
| NVIDIA | About who owns NLib and the IntegrityWare-lineage code, and about the Numerical Recipes routines | Open a GitHub issue once 32.8 is decided. 32.8 was decided on 2026-10-24 without waiting for NVIDIA, so the ownership question is now for the record only, and pointing out the Numerical Recipes routines is a courtesy. Nothing has been sent; the owner decides whether and when | 12, 29 |
| tangency-labs (solidcore and SolidCraft) | As a courtesy, to point out the Numerical Recipes-derived routines in solidcore, and to swap hard test cases: their 337-entry defect log, the SSI corpus and our failures | Write after 32.11 is decided. Their deny.toml bans MPL-2.0, so anything we share must carry a licence they accept | 29 |
| buildingSMART | For a licence on the IFC4.x-IF alignment test set, and agreement on spiral parameters in IfcCurveSegment | Write when the alignment pieces start | 17, 24 |
| Pixar | About OpenSubdiv's licence (TOST 1.0) | First have a legal review of TOST, confirmed by the owner. Write to Pixar only if the review leaves a question. Until then OpenSubdiv is a test oracle only, and our evaluator comes from papers either way | 16, 29 |
| CADCraft's maintainers, about issue #414 | To offer Spare Kit as the shared kernel #414 asks for | Last, and only after the clean-room questions in [12] and the "CadCraft" name conflict are settled. #414 asks for an MIT or Apache kernel built by observing SOLIDWORKS, and both are outside our rules (32.10) | 14, 29 |

**Order.** McNeel and EMBER's authors first, because work waits on them; then Tim Schmidt. NVIDIA, tangency-labs and buildingSMART follow when the decision or piece named in their row is reached. Pixar is contacted only if the TOST review leaves a question, and CADCraft last.

## 32.10 Revit truth data, and watching proprietary programs

**What it is.** To show that Spare Construct matches or beats Revit, the plan compares our geometry with Revit's on the same buildings. Revit's geometry comes out through its published API (28.11). Four questions follow: whose models are used; whether the geometry Revit computes from our models may be published in our open repository; how far watching Revit and AutoCAD may go before it becomes taking knowledge from them; and whether we may check that our exported files open in proprietary programs.

The facts as they stand:
- Models the co-op authors are ours.
- The geometry Revit exports from them is the output of Autodesk's program. Whether its terms let us publish that output is the API terms check (CON-A18-F2-P2), which has not been done.
- Spare Construct's map plans to use anonymised real projects as test fixtures. But building geometry identifies its project even with the names removed, and AGENTS.md forbids client data.
- CADCraft's test scenarios were developed partly by observing AutoCAD from outside, and the owner has been asked to rule on whether that counts as knowledge taken from proprietary software ([12], 14).

| Option | What it means |
| --- | --- |
| A. Co-op-authored models only | A generated 230,000-element building serves as the reference model. Revit exports are published only if the terms check clears in writing. Otherwise they stay on the co-op's licensed machine, the comparisons run there, and only our own measurements are committed |
| B. As A, plus anonymised real projects | More realistic data; client and legal exposure |
| C. No Revit comparisons | Closed-form truth and open oracles only; the strongest outside check on building geometry is lost |

**Recommendation: A, and the same rule for AutoCAD drafting exports.** That needs a Spare Construct feature for the AutoCAD terms check, beside CON-A18-F2-P2.

**The ruling on watching proprietary programs.** Watching a proprietary program from outside is allowed only to check our results against our own specification, on co-op models. It never sets behaviour, expected values or requirements. A difference counts as our bug only when our own rules say so. So CADCraft's code and test scenarios are not used, even with recomputed expected values.

**Our exports in proprietary programs.** Checking that our exported files open in Revit, AutoCAD or other programs is allowed on a licensed machine once that program's terms check clears, as an interoperability check only. The published format specification decides any dispute, and the program's behaviour never becomes a requirement (29.20, rule 8).

Why: this protects clients and the co-op, and keeps the rule against learning from inside proprietary programs, while still giving us the strongest outside check on building geometry we have. Until the terms clear, the cost is that Revit comparisons run on the licensed machine, not on every pull request.

**Changes:** chapter 28 (28.11 and 28.12), chapters 14 and 29, and Spare Construct's CON-A2-F9 and CON-A18-F2.

## 32.11 The licence of our test cases, and outside data with unclear terms

**What it is.** Spare Kit will collect thousands of test cases: the shapes that once broke something, kept forever. They could carry MPL-2.0, like the code. Or they could carry CC0, a dedication to the public domain, so that other kernels can use them too; some kernels ban MPL-2.0 outright, solidcore among them. But the README says Spareware's licence is "Decided, and never to change": MPL-2.0 for the core, and Apache-2.0 for the SDK, examples and plugin templates. CC0 test cases would add a third licence. That fits only if test cases count as data outside the core, the way the SDK is carved out.

Separately, some outside test data has unclear terms:
- the ABC dataset, whose models belong to their creators under Onshape's terms;
- the IFC4.x-IF alignment set, which has no licence;
- Thingi10K, with a licence per model, some non-commercial;
- NIST's STEP files, with no US copyright but possible foreign rights;
- the CC BY-ND IFC samples;
- step-conformance's parts, exported from a commercial program and given under CC0.

| Option | What it means |
| --- | --- |
| A. Our test cases under MPL-2.0 | Matches the code and the README as it stands. Projects that ban MPL-2.0, such as solidcore, cannot take them; we can still describe a hard case in plain words for them to rebuild |
| B. Our test cases under CC0, decided before the first one merges | Anyone can use them. Needs the member vote to confirm that test cases are not "core" under the README's licence rule, and changes the licence line in AGENTS.md section 6 for test data |
| For outside data: (i) a strict rule | Use only CC0, CC BY, permissive or public-domain data, and CC BY-ND data unmodified. Everything else is fetch-only once cleared, or not used at all |
| For outside data: (ii) a looser rule | Also allow non-commercial data in tests |

**Recommendation: B with rule (i), but only if the vote confirms that test cases are data outside the core.** If it does not, take A with rule (i). For outside data, in practice:
- ABC waits for a legal reading of Onshape's terms.
- IFC4.x-IF waits for buildingSMART.
- From Thingi10K, only the permissive and CC BY models are used.
- NIST files and the CC BY-ND samples are fetch-only.
- step-conformance's CC0 parts are stored, unless counsel sees a third-party right.

Why: shared hard cases make every kernel better, ours included, but not at the cost of a licence rule the README calls permanent. Non-commercial data cannot sit in a co-op's public CI.

**Changes:** chapters 28 and 29, and AGENTS.md section 6 if B is taken.

## 32.12 Pieces built ahead of the board

**What it is.** The owner's rule for the board is to build layer by layer, and prove each layer before the next one leans on it (30). Pieces have been built faster than that. G03 and G05 started before the board was numbered and build phase-2 work (points, frames and the proven maths functions) before gate 1 (G43). G09, G11, G25 and G32 were built on G02's unmerged branch before the pieces they need had merged or been recorded; G09 needs the owner's crate-table decision (G08), which is not recorded. All of them are proposed as pull requests stacked on G02's (30.0). Three questions follow, and each touches a rule only the owner holds.

1. **Merge order.** May pieces that build phase-2 work merge before gate 1? And may a crate's pull request edit the manifests of crates in other pieces when the scaffold check asks for it (42 manifests when `geom-run` merges, 26 when `geom-math` does), which AGENTS.md's "stay inside the piece" does not foresee?
2. **Deliberate cost rises.** Principle 12 blocks any speed regression over 5%. G02's second round made results more correct and some operations slower: against the tree before it, 66 benchmarks rose more than 5% in WebAssembly fuel, among them the I256-to-i128 conversion by 632%, `Exactness::combine` by 201% and `abs` by 136%, from checked steps, finiteness checks and `Result` returns (G06's measurement). No rule says how a deliberate rise for correctness is accepted; `geom-bench`'s procedure asks for an owner's decision in the decision log (23.13).
3. **Goldens before the first edition.** 28.13 versions goldens per kernel edition and per operation version, keeping old ones beside new. No edition exists yet, the first goldens are single constants per crate, and they already move on purpose: G02 changed `geom-num`'s within its own pull request, G05 changes it again for the move to CORE-MATH, and G03, G04, G06, G25 and G32 add goldens of their own.

| Question | Options | Recommendation |
| --- | --- | --- |
| Merge order | A. Merge in the board's order: phase-1 pieces as they pass review, G09 once G08 is recorded, and the phase-2 parts of G03 and G05 after gate 1, their pull requests kept open and rebased until then. B. Merge each piece when it passes its own checks and review, and run gate 1 over whatever has merged | A. It is the owner's rule, and the gates exist so that nothing leans on a layer before it is proven. The cross-crate manifest edits are mechanical and spelled out by the scaffold check, so allow them in the pull request of the crate that merges, recorded there |
| Deliberate cost rises | A. Never: a correctness change that costs more than 5% waits for a faster version. B. Allowed when the pull request shows the rise comes from a correctness change and lists every benchmark it moves, and a kernel owner records the acceptance in the decision log; the gate's limits do not change | B. A correct answer that costs more is worth more than a fast wrong one, and the record keeps principle 12's purpose: no rise passes unseen |
| Goldens before the first edition | A. A kernel owner approves each change in its pull request, which says what moved and why and replaces the constant; editions and operation versions start with the first Stable release (G482). B. Treat every change now as a new edition | A. Nobody depends on a released result yet, and 26.2's edition promise is made to users of released versions |

**Changes:** chapters 23 (23.13), 28 (28.13) and 30 (30.0), and AGENTS.md if cross-piece edits are allowed.

## 32.13 The crate table and layers (owner action G08)

**What it is.** Chapter 27 copies G01's crate table and then proposes amendments to it. They add about twenty crates, give every crate one of thirteen layers with the edges allowed inside each, make every dependency list complete, name one owner for each thing that had two or none, and change four rules. Chapter 30.5 adds four more: a new crate, `geom-make`, for primitives, transforms and patterns, and three edges in layer 12 (`geom-p21` on `geom-format`; `geom-exchange` on `geom-format`, `geom-p21`, `geom-intersect` and `geom-check`; `geom-heal` on `geom-exchange`). The board and G09's scaffold were built to all of them while this decision waited (30.0, 32.12).

| Option | What it means |
| --- | --- |
| A. Accept every item as proposed | Chapter 27's proposed tables and rules, with 30.5's amendments, become the crate table. The scaffold already builds exactly this |
| B. Accept with changes | Take the alternatives the chapters raised: the box tree as a module of `geom-math`, linear algebra inside `geom-num`, the thread adapter as `engine-geometry-pool` beside a `no_std` facade, or no `geom-combine` |
| C. Keep G01's table | Reject the amendments; the twenty crates fold back into G01's rows |

**Decided 2026-10-28 (owner action G08): A, every item as proposed.** Read with 31.7's chapter 27 questions, chapters 11, 19 and 23 on the box tree, 15, 16 and 29 on linear algebra, 23.4 on the thread adapter, and 32.5 and R23 on why one crate list matters.
- **Each amendment closes a gap the chapters found, and no alternative closes it better.** No crate owned the types every operation returns, and they must sit below every crate that uses them, so `geom-op`. The box tree had five homes with different tie rules, and putting it in `geom-math` would hang `geom-run` under geometry, so one tree in `geom-bvh`. Linear algebra was placed twice, and inside `geom-num` it would break that crate's one job, so `geom-linalg`. Primitives had no home below the checker, so `geom-make`. The foreign-function bindings need `unsafe`, which every kernel crate and the facade forbid, so three binding crates hold it, audited, outside the kernel.
- **The thread adapter stays `geom-pool`**, in layer 13 beside the facade, with `engine-geometry` on it behind a `threads` feature. 23.4's `engine-geometry-pool` with a `no_std` facade was the alternative. `geom-pool` keeps `std` and rayon out of every kernel crate just as well, and it is what G04 and G33 built, merged and tested; renaming it now would change working code for no gain.
- **It is already proven.** G09's scaffold check reads every `Cargo.toml` on every pull request and fails on any edge the table does not list, any edge that breaks the layer rule, and any cycle. It has passed on main since G09 merged. Every new crate is scaffolded, `geom-run`, `geom-pool` and `geom-gen` hold merged code, and pieces under way build into `geom-linalg` (G49, G50). Option C would undo that work and leave the gaps open.
- **The rules are accepted as written:** a crate depends only on lower layers or along an allowed same-layer edge, and only on public interfaces; `unsafe` only in the binding crates, at the foreign-function boundary; every `geom-*` crate `no_std` except `geom-pool`, with the facade, the bindings and the development crates allowed `std`; checked arithmetic and one lint posture on every crate that ships; a toolkit enters the kernel when a second program needs it; and one development crate, `geom-gen`, for every generator whose output is committed.

What the accepted table settles among 31.7's chapter 27 questions: `geom-combine` for the cross-representation boolean, the dispatch and the validity query; one box tree in `geom-bvh`, which withdraws the Morton cell index in `geom-mesh` and the R-tree in `geom-2d`; `geom-linalg`, so the board's linear algebra is built there and nowhere in `geom-num`; `geom-profile`; `geom-panel` and `geom-analysis`; `geom-landxml`; `geom-exchange`, `geom-p21` and `geom-heal`, with OQ-24.6's EXPRESS generator as a module of `geom-gen` and `geom-cloudio` withdrawn; every row's complete dependency list, chapters 12, 13, 17, 18 and 19's additions among them; the maximal-face merge in `geom-poly`, shared by `geom-bool` and `geom-mesh`; `geom-spatial` on `geom-intersect` directly for closed-form distances, with no trait in the facade; swept volumes with no new crate; and the drawing checker in `geom-draw`.

One question the proposal left open is settled here too: **which lint posture the binding crates carry.** They carry `geom-num`'s `clippy.toml` and `[lints]` tables like every crate that ships, because those bans are about determinism and panics, which a binding must not bring in either, with one change: `unsafe_code = "deny"` in place of `"forbid"`, so each audited block can lift it with an `#[allow(unsafe_code)]` on the narrowest item, a `// SAFETY:` comment saying why it is sound, and clippy's `undocumented_unsafe_blocks` denied. The scaffold check gains that row when G453 builds the first binding crate.

Not settled here: whether `geom-civil`, `geom-thread` and `geom-stair` enter the kernel now as exceptions to 26.1's two-program rule (owner action G238); and 31.7's questions this choice does not touch, such as a `geom-proof` crate (after the Verus spike) and the check that chapter 27 and `table.rs` agree (G15 or the merge gate).

**What changed with it.** Chapter 27's tables now read as accepted; 30.5 and G01 sections 5 and 27 point here; the kernel's `ARCHITECTURE.md` lists the thirteen layers; every crate header that said "Proposed in chapter 27, pending the owner's decision G08" (or chapter 30) no longer does, and the scaffold check now refuses a header that still carries that mark; the chapters chapter 27 lists under "Changes elsewhere" state the accepted homes.

**Changes:** chapter 27, 30.5, 31.7, G01 sections 5 and 27, the kernel's `ARCHITECTURE.md`, `engine-geometry`'s scaffold test and every crate header, and chapters 12 to 19 and 23 to 28 where chapter 27 lists them.

## 32.14 Rotations (owner action G44)

**What it is.** Section 7 says a rotation other than a quarter turn "rounds to the grid and the result says so". Chapter 14 says any other rotation uses the closest rational rotation within ε_ang, keeps rotated points as rational implicit points wherever rounding would flip a decision, and proposes one rotation type for 2D and 3D: rational rotations from Pythagorean triples, as quaternions in 3D (31.7, chapter 6). Its own note warns that rotated planes then exceed 6.2's 384-bit budget, so 6.2 and 11 must agree. G03 built `geom-math`'s typed transforms to section 7's reading: an exact, a rational (`RationalTransform`, entries and denominator at most 2^40) and a certified variant, each applied by rounding the exact image once.

| Option | What it means |
| --- | --- |
| A. Round every rotation to the grid | Section 7 as written. Simple, but a rotated building loses exact square corners and parallel walls, which chapter 14 and the plane-based core rely on |
| B. Rational rotations everywhere, carried exactly | Chapter 14 read to the end: rotated points and planes stay rational. Planes then pass 6.2's 83-bit normal and 125-bit offset, so every boolean on rotated geometry needs predicates wider than 384 bits |
| C. One rational rotation type, kept as a transform where possible, applied with section 7's single rounding and 6.3's re-check | Rotations are exact in fractions; a rotated body keeps its own frame; geometry is rotated only when it must be, and then rounds or commits like any other construction |

**Decided 2026-10-28 (owner action G44): C.** Read with chapters 6, 7, 13 and 14, G03's `rational.rs` and `typed.rs`, and Canny, Donald and Ressler's rational rotation method ([doi:10.1145/142675.142726](https://doi.org/10.1145/142675.142726)); the 3D form is the Euler–Rodrigues parametrisation, under which a rotation matrix is rational exactly when it comes from an integer quaternion.
- **One type, in 2D and 3D: a rational rotation held as an integer quaternion q = (w, x, y, z).** Its matrix is M(q)/|q|², with M(q) the integer Euler–Rodrigues matrix, so it is exact in fractions and composes exactly by quaternion product. A 2D rotation is the case (w, 0, 0, z): the Pythagorean direction (w² − z², 2wz)/(w² + z²). Quarter turns stay exact grid transforms.
- **A general angle uses the closest rational rotation within the requested bound, by default ε_ang (2^-34 rad), and is Certified in angle.** In 2D, Canny, Donald and Ressler's best approximations of tan(θ/2) give a Pythagorean direction with w² + z² near 2^35, inside `RationalTransform`'s 2^40 budget (14.3 says about log₂(1/δ) bits). In 3D, rounding 2^35 times the unit quaternion to integers moves it by at most 1, so the rotation angle moves by at most about 2^-34; components stay below 2^36, |q|² below 2^72, and a grid point's image, Σ M(q)ᵢⱼ pⱼ, stays below 3·2^112, inside `i128`. G46 builds both, may shrink the 3D numbers with a better simultaneous approximation, and proves each angle bound against an oracle. Where |q|² passes 2^40, the rotation is its own variant of the typed transform rather than a `RationalTransform`. A composition that outgrows the components' budget is re-approximated once to the requested bound, as 14.10 says.
- **A rotation stays a transform wherever it can.** A frame record, an insert or an instance holds the rotation, and the geometry stays exact in its own frame (6.1, 14.10, 19.12). Most rotated work in buildings is a plan rotation about the vertical axis, which this keeps exact end to end.
- **When a rotation must be applied, it follows the rules every construction follows.** A grid point's exact image is rounded to the grid once, ties to even, and the result says whether it moved (section 7, as G03 built); where rounding would flip a decision, the operation keeps the rational implicit point instead (6.3, as 14.10 asks). A rotated plane stays exact only if its coefficients fit 6.2's plane budget; any other is committed by 13.2's plane commit (G142, under owner action G120's ruling). So 6.2's 384-bit budget stands, and no boolean needs wider predicates because of a rotation. That answers chapter 14's note: 6.2 and 11 agree because rotated planes never enter the exact core uncommitted.

Why: exact rotations where they cost nothing, and an honest Certified label where they cannot be exact, without widening the predicates every boolean pays for. Option A throws away square corners a rational rotation keeps for free; option B makes every boolean on rotated geometry wider and slower for a precision the commit already proves within q/2.

**Changes:** sections 6.2 and 7, 14.10, 31.7, and piece G46's scope (both forms, the 3D bound and the wider variant).

## 32.15 Time budgets (owner action G71)

**What it is.** Section 10.1 lists three budgets in `OpContext`: work units, memory and time. A budget that stops an operation decides its result, and a result decided by a clock differs from machine to machine and run to run, which would break replay and the same-answer guarantee (21, 32.4). Chapter 28 reads 10.1 this way: only work units, and memory counted in units the kernel tallies itself, may decide a replayable outcome; a time limit is the caller's deadline, turned into a cancellation through the cancellation token, so the kernel never reads a clock; and a run stopped that way is never written to the log as the operation's result (28.13). 23.10 and `geom-run` (G23) are built to that reading. The owner is asked to confirm it.

| Option | What it means |
| --- | --- |
| A. Confirm chapter 28's reading | Time is never a budget the kernel keeps. A deadline is the caller's `Cancel`; a cancelled operation publishes nothing and logs nothing as its result |
| B. A kernel time budget | The kernel reads a clock and fails with a budget error when time runs out. Results then depend on the machine's speed and load |

**Decided 2026-10-28 (owner action G71): A, chapter 28's reading.** Read with 10.1, 21, 23.10 and 28.13, `geom-run`'s `cancel.rs` and `meter.rs`, and SolidCraft's attempt budget, which gives up after a fixed number of tries rather than on a clock ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft).
- **Only work units and logical memory are budgets.** They are counted by the kernel, the same on every machine, thread count and build mode, so `BudgetExceeded` may be stored as model state and replays identically (23.10).
- **Time belongs to the caller.** A time limit is a deadline the caller's `Cancel` checks; the kernel polls `Cancel` at the work meter's fixed steps and reads no clock itself. Cancellation is the only stop that is not deterministic, so it can never reach the model: a cancelled operation publishes nothing, writes nothing to the operation log as its result, and leaves its inputs unchanged (10.1, transactional).
- **Callers who want a reproducible limit use work units.** A program that wants "give up on this fillet if it is too hard" sets a work budget, whose failure is the same everywhere; a person's patience is a deadline, whose stop is not recorded.

Why: a result that depends on how fast a machine was is not a result the kernel can promise to replay. B would make every budgeted operation's outcome a property of the hardware.

**Changes:** 10.1 (`OpContext` carries work and memory budgets, and a deadline only through its cancellation token), 28.13 and 31.7. Piece G83 builds the budgets into the operation contract to this rule.

## 32.16 The reference machine and the kernel's shares (owner action G72)

**What it is.** Section 23.1 gives the kernel a share of each of Spare Construct's ten targets: 1 s of the 3 s open, 4 ms of the 16 ms door move, 600 ms of the 1 s level change, and so on, plus browser estimates and at most 5 µs of overhead for a trivial operation. The shares are for native builds on a reference machine of 8 cores, which 31.7 proposes as an x86-64 desktop with 32 GB and NVMe, with an aarch64 laptop nightly. Piece G107 times every benchmark on it nightly and G452 turns those timings into a gate, so both wait for the machine and the shares. Section 32.5 has the kernel's owners and Spare Construct's owners set the shares together.

**Decided 2026-10-28 (owner action G72).** Read with 23.1, 23.13, 28.17 and R5.
- **The reference machine is the owner's spare tower**, which G10 documented and owner action G07 placed (28.17): an Intel Core Ultra 7 265 with 8 performance and 12 efficient cores, 64 GB of DDR5-5600 and a 1 TB NVMe drive, free for the kernel every night from 19:00 to 08:00. It meets 31.7's proposal and costs nothing (K06).
- **"8 cores" means its 8 performance cores.** The 8-thread timings run on those cores only and the 1-thread timings on one of them, so a run never lands half on the slower cores. WSL 2's virtual processors are not tied to host cores (28.17), so G107 takes the timed runs from the native Windows build of each benchmark, pinned to the performance cores with a processor affinity mask, or shows that WSL 2 runs give the same figures. The build workstation stays the build machine and is not a timing reference.
- **No aarch64 reference yet.** The co-op owns no Arm laptop and buys none (32.4). aarch64 keeps its instruction-count gate on GitHub's hosted Arm runners on every pull request (28.17); an Arm timing reference is added when the co-op has such a machine to lend.
- **The shares are accepted as proposed**: the ten targets' kernel shares of 23.1, its browser estimates and the 5 µs overhead per trivial operation, for native builds on this machine. They stand until the scenario benchmarks measure them; then the measured figures replace them by a decision note, which the kernel's and Spare Construct's owners accept together (32.5). The owner holds both roles today, so accepting them here is that joint decision.

Why: the machine exists, is lent at no cost, and is an 8-performance-core x86-64 desktop as 23.1 asked, so nothing better is waiting to be bought. The shares were derived from the targets and the cost of the work behind each (23.1), and the first measurements will test them; holding G107 and G452 back for a better guess would only delay those measurements. One caution is recorded with the choice: this is a fast 2025 desktop, so a share it meets can be missed on an older laptop, which is what 23.1's browser estimates and Spare Construct's own targets per device are for.

**Changes:** 23.1, 28.17, R5 and 31.7. Pieces G107 and G452 build to this machine and these shares.

## 32.17 Arena indices in name order (owner action G76)

**What it is.** A body stores each kind of entity in an arena addressed by a 32-bit index. G01's decision note says indices are "assigned in operation order", which leaves the order to how each operation happens to run. Section 8.2 proposes an amendment: the builder assigns indices in persistent-name order (20.2), as the boolean rebuild already does (11.3), with fins, loops and shells, which have no names, following a fixed rule from the names of what they join. Then index order is name order, so a tie broken by index is broken by name (23.7), and the tables a body record writes in arena order (24.2) are in the name order 23.8's content hash needs. 31.7 asks the same question from chapters 23 and 24: canonical body encoding in arena order or in name order.

| Option | What it means |
| --- | --- |
| A. Accept: indices in name order, in every operation | A body's arenas depend only on what it contains. A full re-cut, a local re-cut and a cached re-cut give the same indices on any number of threads, so the same bytes and the same content hash |
| B. Keep operation order, and sort by name only when writing | Indices follow how each operation ran; the encoder sorts every table by name and rewrites every position reference. Indices then differ from the record's positions, and two build paths to the same body differ in memory |
| C. Keep operation order everywhere | The content hash depends on how a body was built, so the same shape made two ways has two hashes and replay from a cache can differ from replay without it |

**Decided 2026-10-28 (owner action G76): A, as proposed.** Read with 8.2, 11.3, 20.2, 23.4, 23.5, 23.7, 23.8, 24.2 and 24.3, and evidence report [03](../../../kit/notes/G01-evidence/03-rust-kernels-crates.md) on how Truck, Fornjot and OpenCascade lost determinism through identity by address or build order.
- **It makes one order the order of everything.** Arena order, the order ties are broken in, and the order tables are written and hashed in are all name order. A reloaded body equals the in-memory one index for index (24.2), and the local re-cut of 23.5 and the arrangement cache of 11.6 can only be exact if their results equal a full rebuild's, which A makes true by construction rather than by care in each operation.
- **B costs as much and proves less.** Sorting at write time is the same sort, done later, with every position reference rewritten, and it still leaves two build paths to one body unequal in memory, which the replay and cache tests would then have to tolerate.
- **The cost is known and bounded.** Names are hashes, so entities near each other in space are not near each other in memory, and adding or removing an entity shifts the indices after it, so a new version shares fewer chunks with the one before. 23.9's benchmarks measure both. If the cost matters, the remedy is a canonical key placed before the name, such as a spatial cell, as 17.3 chunks terrain; never a return to build order.
- **It is enforced, not hoped for.** The builder (G81) sorts by name before a body is published, whatever order a parallel builder's `count_then_fill` filled its scratch arrays in. Its tests include the invariant 31.7 asks for: every operation's output is the same when its inputs' arenas are shuffled, checked by a shuffle test on every operation's fixtures.

**Changes:** 8.2 (the amendment is accepted), 24.2 (positions are name order), 31.7, and piece G81's done-when.

## 32.18 The CBOR tag and the file name (owner action G77)

**What it is.** A standalone native file opens with the 12-byte label of [RFC 9277](https://www.rfc-editor.org/rfc/rfc9277): the CBOR Sequence tag 55800, a four-byte protocol tag of our own, and the byte string `BOR` (24.2). Chapter 24 proposes the tag `0x534B4746`, the ASCII letters "SKGF", the format's name SKG (Spare Kit Geometry) and the extension `.skg`, and asks an owner to register the tag with IANA and check the name and the extension for clashes.

**Decided 2026-10-28 (owner action G77).** Checked against IANA's CBOR tags registry, RFC 9277 and RFC 8949 section 9.2 (all read 2026-10-28), and a search for other uses of the name and the extension.
- **The tag is `0x534B4746` (1397442374), "SKGF".** It is unassigned: IANA's registry lists 1347571543 to 1398229315 as one unassigned block. It lies in the first-come-first-served range (32768 and up), in RFC 9277's four-byte range for protocol tags (0x01000000 to 0xFFFFFFFF), with no zero byte, as RFC 9277 advises, and its four bytes spell a mnemonic, as RFC 9277 encourages. A first-come-first-served tag needs only a request to IANA using RFC 8949's template; the request is drafted in this piece's pull request, and the owner sends it, because it is a request to an outside body made in the co-op's name. Until IANA confirms it, the specification (G93) names the tag as requested.
- **The name SKG stays.** No file format, geometry program or CAD tool by that name was found. The letters are also a brand of massage devices and Thessaloniki airport's code, neither in software. A trade-mark search was not made; the name is the format's, inside the kernel, not a product name.
- **The extension changes from `.skg` to `.skgf`.** `.skg` is already taken: it is the native document format of Skrooge, KDE's personal finance manager ([skrooge.org](https://skrooge.org/features/)), packaged by the main Linux distributions, so on a Linux desktop a `.skg` file would open in a finance program. No use of `.skgf` was found, and it matches the four letters of the file's own label, so the name on the file and the bytes inside it say the same thing.

**Changes:** 24.1, 24.2, 24.5's container check, 31.7 and the glossary. Piece G93 writes the tag and the extension into SPEC.md and REGISTRY.md.

## 32.19 NameScheme v1 (owner action G78)

**What it is.** A persistent name, a `NameId`, is a 128-bit hash of a canonical encoding of how an entity was made (20.1). Files keep names forever, so the hash, how it is cut to 128 bits, the bytes it hashes and the rules for any text inside them can never change within a scheme: a change is a new `NameScheme`, recorded in each file's header (24.4). Chapter 24 asks for the text rules (NFC and a Unicode version), chapter 23 and owner action G79 choose the hash, and section 11's open question 3 asks whether `Bounded` also holds the faces that end an edge, because a cut corrugated sheet makes hundreds of edges between the same two faces.

**Decided 2026-10-28 (owner action G78).** Read with 11.9, OQ-11.3, 20.1 to 20.4, 23.8, 24.2 to 24.4, 26.2, evidence report [07](../../../kit/notes/G01-evidence/07-persistent-naming.md), RFC 8949 section 4.2.1, NIST SP 800-107 Rev. 1 section 5.1, and the Unicode Normalization Forms annex (UAX #15).
- **The hash: SHA-256, cut to its first 16 bytes.** The `NameId` is the leftmost 128 bits of SHA-256 (FIPS 180-4) over the scheme's domain prefix and the name's canonical bytes, as NIST SP 800-107 allows a SHA-256 digest to be truncated. It is the same function as the content hash (owner action G79, 32.20), so one approved implementation serves both. Accidental collisions are out of reach (about 1 in 10^21 for a billion names), and a deliberate one would cost about 2^64 hashes; either way it cannot pass silently, because the name graph stores each node's canonical bytes beside its `NameId` and refuses a second, different encoding under the same `NameId` with an error (G87), never merging the two.
- **What is hashed:** the 17 ASCII bytes `SKG NameScheme v1`, then one zero byte, then the canonical encoding of the `TopoName`. The prefix keeps names apart from content IDs and from any later scheme.
- **The canonical encoding** is 24.3's deterministic CBOR (RFC 8949's core deterministic rules). A `TopoName` is an array whose first item is its variant's fixed number, `Created` 0, `Split` 1, `Merged` 2, `Bounded` 3, `Weak` 4, followed by the variant's fields in their declared order. A `NameId` is a 16-byte byte string; sets and sorted lists are arrays sorted by the bytes of each element's encoding, sets without repeats; an `OpPath` is an array of the `OpId`'s 16 bytes followed by its labels; a role is an array of its name, as text, followed by its arguments; `PieceKey::ByTools` is `[0, tools, side]`; an `Ordinal` is `[n, of]`. These numbers and orders are fixed for v1. A new variant or key takes a new number; a new field in an existing variant is a new scheme. G87 builds it and G93 writes it into the specification as CDDL.
- **Text: NFC, under Unicode 18.0.0, forever for v1.** Text hashed into a name (role names, labels) must be in Normalization Form C under Unicode 18.0.0, and the writer refuses other text (24.3). Unicode's stability policy keeps NFC fixed for characters already assigned, but a code point unassigned today may later gain a decomposition, so the version is part of the scheme: v1 checks with 18.0.0's tables for as long as it exists, and a later Unicode version arrives only with a new scheme. The approved check is the `unicode-normalization` crate at 0.1.26, whose tables are Unicode 18.0.0 (MIT OR Apache-2.0, with Unicode-3.0 data), built without its `std` feature; G87 adds it to `deny-no-std.toml` with its audit, and moving it to another version within v1 is refused unless its tables are still 18.0.0.
- **The `TopoName` variants are 20.2's, with `Bounded` gaining the faces that end an edge:** `Bounded { kind, by: SortedSet<NameId>, ends: SortedSet<NameId> }`. `by` holds the faces the edge or vertex lies between and `ends` the faces that meet it at its two ends, empty for a vertex or a closed edge, so the edges a wall's openings cut between the same two faces are told apart by the faces that end them. OQ-11.3's `ord: u16` is not taken: edges that still collide take `Weak`, whose `u32` ordinals already count millions and follow 20.2's one ordinal order, so a second ordinal inside `Bounded` would only duplicate it.

Why: SHA-256 is the hash the format and every golden already use, so names add no second algorithm; truncation keeps names at the 16 bytes 20.1 promises. Fixing the variant numbers, the prefix and the Unicode version now is what lets a name written today resolve in fifty years (26.2).

**Changes:** 11.9 and OQ-11.3, 20.1 and 20.2, 24.3, 31.7, and pieces G86, G87 and G93.

## 32.20 Hash functions (owner action G79)

**What it is.** The kernel hashes in two places. Content IDs and cache keys hash a body's canonical bytes (23.8, 24.2), and `NameId`s hash a name's canonical encoding (20.1). Chapter 23 left the content hash open between SHA-256 and BLAKE3, and noted that both common Rust crates use unsafe SIMD code, so the choice is also which implementation a `no_std` kernel crate may depend on (31.7). Owner action G78 has already cut the `NameId` from the same function (32.19).

| Option | For | Against |
| --- | --- | --- |
| A. SHA-256 for both, from RustCrypto's `sha2` | A FIPS standard with fifty years of tools behind it; already 24.2's algorithm 1, every golden digest in the repository and Python's built-in `hashlib`, which the second reader of 24.6 uses; one function to approve | Slower than BLAKE3 in software, about 200 to 400 MB/s against several GB/s; fast where the processor has SHA instructions |
| B. BLAKE3 for both, from the `blake3` crate | Several times faster; a tree hash, so chunks hash in parallel | A second algorithm beside every existing golden; not in Python's standard library; its `pure` build is the safe one, and its SIMD paths are larger to audit |
| C. Hand-written SHA-256 in safe Rust | No dependency | The owner's rule forbids home-made hashes (29.1, AGENTS.md section 4), and the repository already removed one |

**Decided 2026-10-28 (owner action G79): A.** Read with 20.1, 23.8, 24.2, 24.6, 28.13, 29.1 and 29.20's rule 3, the `sha2` 0.11.0 source that `Cargo.lock` already holds, and the `blake3` crate's features.
- **The content hash is SHA-256**, as 24.2's algorithm 1 already says, and **the `NameId` hash is SHA-256 cut to 16 bytes** (32.19). The content ID keeps its algorithm code, so a faster or stronger hash can join later as algorithm 2 without breaking a stored file.
- **The approved implementation is RustCrypto's `sha2`**, at the 0.11 release `Cargo.lock` pins (0.11.0 today; MIT OR Apache-2.0), with `default-features = false`, `no_std`, and its `digest` dependency chain. It is pure Rust, written to FIPS 180-4, and the most used SHA-2 crate in Rust. Its `unsafe` code is confined to the processor-specific backends (x86 SHA extensions, AArch64 SHA2), each behind a runtime check of the processor's features, and to its unrolled software path's table read; its compact software path has none. Every backend computes the same function, so the bits never depend on the machine; the tests check the NIST test vectors on every target, natively and in WebAssembly, where it runs its software path. The kernel's own code stays free of `unsafe`; the crate's is read in its audit, as `libm`'s was (G24).
- **How it enters the kernel.** Piece G86, which builds the canonical encoder and the safe hash, adds `sha2` and the crates it brings to `deny-no-std.toml` with the four-clause record that file asks for, its audit committed beside `libm`'s, and the scaffold table's `registry` naming the kernel crates that may use it (`geom-op` for names, `geom-format` for content). BLAKE3 is not used now.

Why: one well-known function for names, content and goldens, already in the format and in every tool that might ever check a file, beats a faster hash that would add a second algorithm everywhere. The speed difference matters little where these hashes run: names are short, and content hashing runs in background compaction, on SHA instructions where the processor has them.

**Changes:** 24.2, 31.7 and the board's G86. Piece "Content hashes and cache keys" still decides the name-ordered bucket tree.

## 32.21 Committed planes and how bounds add up (owner action G120)

**What it is.** Two of chapter 13's open questions, which section 11's first question shares.
- **Question 1.** A 1.5° draft or a 30° roof needs a plane whose ideal coefficients are irrational, so 13.2 commits it once to integer coefficients within 6.2's plane budget and proves how far each face then lies from the ideal. Should a committed plane whose deviation over its face is within q/2, and a committed point (at most q/2 per coordinate), count as Exact rather than Certified?
- **Question 4.** Section 9 says bounds add up, but not against what. Read literally, each curved boolean adds about ε_fit/74, so a curved part refuses further booleans after about 74 of them (11, OQ-11.1; R6). 32.3 recommends its option B: each result is re-proven against the exact surfaces it is built on, so booleans and procedural kinds add nothing, and only commits and fits add, each step spending at most a quarter of the fit bound. 13.2 proposes the rule in detail and the ladder rung behind it, ε_step = 16q, which 6.5 lists as proposed.

**Decided 2026-10-28 (owner action G120).** Read with 6.3, 6.5, 9, 11.3, 11.4 and OQ-11.1, 13.2, 17.3, 19.3, 32.3, R6 and R11, and `geom-num`'s ladder module (G22) and `Exactness` (G02).
- **Question 1: no. A committed plane and a committed point stay Certified, with their proven deviation.** Exact keeps one meaning: the stored result is the operation's exact result on its stored inputs, with nothing rounded anywhere. A commit rounds, by at most q/2 per coordinate for a point and by a per-face proof for a plane, which may exceed q/2 across a large face (13.2), so calling it Exact would hide a deviation that a later step can amplify (32.3, "what certified allows"). It costs little: the caller's `Accept` already takes Certified with ε_fit by default (11.1), interactive commands accept it on record (14.3), and the deviation stays as small as the commit makes it. 11.3, 11.4, 13.2, 17.3, 17.6 and 19.3 stay as written.
- **Question 4: 32.3's option B, by 13.2's one rule.** Each entity, face and edge, carries its own bound, and bounds add only along that entity's own history: an edge filleted after a loft carries both terms, a face the fillet never touched keeps its own, and a body's bound is the largest of its entities'. Each operation's own term is measured against the exact operation applied to its stored inputs, so booleans and procedural faces (sweeps, rolling-ball blends, offsets, intersection curves) add nothing, because their faces do not move and their new edges are defined by the surfaces that meet there; only commits and fits add. When an entity's sum would pass the caller's ceiling (ε_fit by default), the operation fails with `ExactnessNotAccepted`, naming the earlier steps that used the budget (6.5, 22). The bound is against each step's stored inputs, not against replaying the whole history exactly, as 32.3 says. This settles OQ-11.1 the same way.
- **ε_step joins the ladder: 16q, ε_fit/4** (2.5 µm at q0, 0.1 µm at q1): the most one operation's own commits and fits may add to an entity's bound. So about four fitting steps at full budget fit on one face's history, as 32.3 says, and every "a quarter of the fit bound" in the chapters now has one name. It moves into 6.5's table and into `geom-num`'s ladder as `ladder::PER_OPERATION`, beside `ladder::FIT`, checked when the crate compiles to be exactly a quarter of it, and `geom-gen`'s grid-steps table recomputes it.

Why: one meaning of Exact is worth more than a few more results labelled Exact, and R11's risk, a kernel too strict to use, is met by `Accept`'s default, not by relabelling. Counting bounds per entity against stored inputs is what makes 32.3's promise true: geometry stays within ε_fit of each step's exact result however long the boolean history, and refuses honestly when fits use up the budget.

**Changes:** 6.5 (ε_step accepted), 11.4 and OQ-11.1, 13.2 (both questions settled; ε_step adopted, so "ε_fit until adopted" no longer applies), 31.7, and `geom-num`'s ladder module with `geom-gen`'s grid-steps table. Pieces G142 (plane commit) and G143 (per-entity bounds) build to it.

## 32.22 The predicate step ladder (owner action G121)

**What it is.** Section 6.2 says every predicate runs its `f64` filter first and falls through to the exact integer path only when the filter cannot certify a sign. In a building many calls are exactly zero, which no filter can certify, so they all pay for the exact path. Section 23.2 proposes, as an addition to 6.2, three cheaper steps before the filter: identity (faces on one canonical plane share its entry, so "is this face on that plane" is an index comparison), narrow exact (a magnitude check proves every intermediate fits in 63 bits and the predicate runs in `i64`), and axis (classifying against a plane square to a frame axis is one comparison). It also proposes switching a filter off on a target where it measures slower than the exact path it saves, as G28 found for orient2d on 64-bit targets other than WebAssembly, and filters whose inputs are rounded copies of wider integers carrying that rounding in their error bound.

**Decided 2026-10-28 (owner action G121): accepted as an addition to 6.2, with two conditions.** Read with 6.2, 23.2, 23.3, 28.6 and 28.7, evidence report [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) and G28's measurement of orient2d.
- **The ladder.** Each predicate takes the first step that settles it: identity, narrow exact, axis, filter, exact. Every step returns the sign the exact path returns, so the ladder changes only speed, never an answer, and principle 1 is untouched.
- **Condition 1: each step is checked against the exact path.** A step that answers is tested against the exact integer evaluation at the extremes, on degenerate inputs and one step from them, and on random inputs (28.6), as the filters already are; the narrow step's magnitude check is integer arithmetic, never a floating-point estimate. A step that cannot show this is not used.
- **Condition 2: a filter is switched off only by measurement, per target, at build time.** Where the instruction-count benchmarks (23.3, the count gate) show on all four input classes that a predicate's exact path costs no more than its filter plus the fall-through, that predicate skips its filter on that target, chosen by `target_arch` when the crate is built, never at run time. The choice is recorded beside the benchmark that justifies it, and the count gate re-checks it on every pull request. Since every step returns the same sign, the switch cannot change a result on any target.
- **Rounded-input filters are accepted as built.** A filter whose `f64` inputs are rounded copies of wider integers carries |x̃ − x| ≤ 2^-53·|x| per input in its error bound (`geom-gen`'s `rounded` inputs, G27 and G29), and tests put coefficients at their full widths with results near zero.

Why: an exact zero is the commonest case in a building, and the cheapest exact answer is the one the data already holds. The conditions keep the speed-up from ever costing a sign: every step is proven against the exact path, and a filter is dropped only where a reproducible count says it loses.

**Changes:** 6.2 (the ladder joins it), 23.2, 31.7, and piece G124, which builds the three steps and the step statistics.

## 32.23 SMLib as a CI oracle (owner action G157)

**What it is.** SMLib's NURBS library, NLib, is the most complete permissive NURBS code outside our own (32.8, [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md)). Run as a separate program, it can check our NURBS evaluation, knot insertion and removal and degree elevation on every change (28.10). But SMLib and NLib contain routines derived from Numerical Recipes, whose licence forbids copies (29.6), and 29.20's rule 8 lets its oracle build run only after an owner's yes. Section 32.6's third question asks whether such unpublished test builds may compile those routines.

**Decided 2026-10-28 (owner action G157): yes to the oracle, no to compiling the Numerical Recipes routines.** Read with 12.17, 28.10, 29.5, 29.6, 29.20's rule 8, 32.6, 32.8 and evidence report 13.
- **What runs.** SMLib's reference build, limited to NLib's evaluation, knot insertion and removal and degree elevation, built from pinned usd-brep and solidcore source in the oracle repository's own isolated container, never from the prebuilt `cpp-ref`, run out of process in fresh processes, and never authoritative: exact truth settles every disagreement (12.17, 28.10).
- **What is never compiled.** The routines 29.6 names as derived from Numerical Recipes, in SMLib's core and in NLib (`N_SingleValueDecomposeSolve`, `N_RealMatrixSingleValueDecompose`, `N_FuncFindMinima`, `N_RealMatrixEigenValuesVectors` and its pythag helper, `M_TridiagonalSystem`, `M_CorneredTridiagonalSystem`, and SMLib's matrix, integrator and Brent routines), are replaced in the oracle build by stubs that stop the program with a message. A comparison that reaches a stub is reported as not covered, never as passed. This is 32.6's answer to its third question: copies of that code are not made, published or not.
- **What never counts.** Fitting (NLib's point-cloud fitting draws from a hidden, platform-dependent random generator), booleans, fillets, offsets, validity and the intersector, which is nondeterministic on some paths, stay out (rule 8). A class of comparison is used only while repeated runs of it agree bit for bit. 12.17's proposals to run SMLib's best SSI classes and NLib projection are not part of this yes.
- **Its image stays private** in the oracle repository's Actions cache, like every oracle image (rule 8; owner action G07).

Why: an independent NURBS implementation catches errors in our evaluation and knot algorithms that our own tests share blind spots with (R24), at no licence cost once the excluded routines are stubbed. Compiling those routines, even privately, makes the copies their licence forbids; stubbing them loses nothing the NLib-layer comparisons need, since evaluation, knot operations and degree change do not call them.

**Changes:** 12.17, 28.10, 29.20's rule 8, 32.6's third question, 31.7, and piece G161, which builds the SMLib and geomdl oracles.

## 32.24 How our hatch patterns and linetypes are authored (owner action G184)

**What it is.** Drafting needs a library of hatch patterns and linetypes. The well-known libraries are a CAD vendor's files, and CADCraft's library, labelled its own, was found to match them in names, descriptions and values ([12](../../../kit/notes/G01-evidence/12-vetting-cadcraft.md), formats-legal), so it is rejected (29.5). Chapter 14 says ours is our own, with a provenance log for every entry, and 29.19 names no clean source yet. The owner sets how entries are authored and reviewed.

**Decided 2026-10-28 (owner action G184): a clean-authoring process, reviewed in each pull request.**
- **Sources an author may use:** a published rule implemented, never a file copied: the line rules of ISO 128 (dash and gap lengths as multiples of the line width) and its conventions for cut sections, national drawing standards and textbooks that state a convention in words or dimensions, and our own designs. The standard's text is never reproduced, only the rule applied.
- **Sources an author may not open while authoring:** any vendor's pattern or linetype files (`.pat`, `.lin` and their equivalents), CADCraft's library, and any other library without a row in chapter 29. An imported DXF hatch carries its own pattern lines (14), so reading files never needs a library.
- **Names are our own and descriptive** (for example `cut-45-single` or `insulation-batt`), never a vendor's pattern or linetype name. An import keeps whatever name its file gives, as data.
- **Each entry's provenance log** records: who authored it and when; the rule it implements, by clause or page, or "own design"; how each value was derived from that rule; and a statement that no excluded source was opened. The log is committed beside the entry and the library's map lists it.
- **Legal review** is a kernel owner's review of each entry's log in its pull request against these rules, recorded there; no lawyer is engaged and nothing is bought. An entry whose log is incomplete, or that cites an excluded source, does not merge. If anyone shows an entry matches a vendor's file, it is removed and re-authored by someone who has not seen that file.
- **Licence:** the library is part of the kernel's data and carries the kernel's licence, MPL-2.0 (32.11 decides test cases separately).

Why: patterns and linetypes are short lists of numbers, so the risk is not one value but a whole library's selection, names and values copied together. Authoring from written rules, naming our own way and logging every derivation is the evidence that ours is independent, and it costs only the log.

**Changes:** 14.6's library bullet, 29.19's drafting row, 31.7, and piece G211.

## 32.25 Ellipses in 2D (owner action G185)

**What it is.** Section 7 lists the ellipse and elliptical arc among the curve kinds, by parameter rather than polar angle, and 26.1 says a curve kind enters with a decision note. Chapter 14 stores an ellipse so that its decisions can be exact: centre and major-axis vector on the grid, and the axis ratio as a fraction p/q of bounded size, so the ellipse's equation has whole-number coefficients and piece G190 can decide crossings, tangency and order exactly. DXF stores the ratio as a double, so an import must choose a fraction (31.7).

**Decided 2026-10-28 (owner action G185): the ellipse is a curve kind stored by a bounded axis ratio, as chapter 14 says; this section is its decision note under 26.1.** Read with 7, 14.1, 14.2, 24.11, 26.1, 32.2 and evidence report [06](../../../kit/notes/G01-evidence/06-2d-drafting.md).
- **Stored:** the centre and the major-axis vector in whole grid steps, the axis ratio as a reduced fraction p/q with 0 < p ≤ q, and an elliptical arc's ends as parameters on the ellipse, never polar angles. The same kind serves 2D and 3D, a 3D ellipse being one in a plane's frame (principle 8).
- **The bound on q** is the one chapter 14 derives: about √(2a/q_step) for a semi-major axis of a steps, at most about 2^21, which keeps the minor axis within a step of any ratio while keeping the equation's coefficients inside G190's fixed widths.
- **A ratio from a file** (DXF's double, IFC's two radii as reals) becomes the nearest fraction within that bound, found by continued fractions; the result is Exact when the file's value is already such a fraction, otherwise Certified with the recorded deviation of the minor axis, and the import report says so (24.8).
- **A non-uniform scale** of a circle or ellipse makes an ellipse of this kind where the principal axes come out on the grid; otherwise the result is a rational choice and Certified (14.10).
- **If G190's benchmark fails** for ellipse pairs, 32.2's fallback B1 applies to them alone: decisions between two ellipses become certified, lines and arcs stay exact.

Why: one exact form for the conics drafting draws most, so snaps, offsets and crossings on ellipses are as trustworthy as on arcs, and the bound keeps every predicate fixed-width.

**Changes:** 7's kind list, 14.2, 31.7, and pieces G190 and 2D-04.

## 32.26 Depth-one 2D numbers (owner action G186)

**What it is.** Section 6.1 stores every coordinate as whole grid steps. Exact 2D work (32.2's option A) needs three more forms, each an exception like 6.3's for planes, and chapter 15 needs the same ones for sketches. An **integer carrier** is a line n·x = c with |n| below 2^41 and |c| below 2^82. A **depth-one point** is where two carriers meet, stored as homogeneous integers (X, Y, W) with |X| and |Y| below 2^124 and |W| below 2^83, so a T-junction lies exactly on its line. A **rational-centre arc** has its centre at a depth-one point. Their predicates, from the side of a carrier (168 bits) to a depth-one point against an arc's circle (337 bits), all fit inside 6.2's 384-bit budget (15.10). The forms wait on 32.2 and on an owner's acceptance as an addition to 6.2.

**Decided 2026-10-28 (owner action G186): accepted as an addition to 6.2, with 32.2's option A.** Read with 6.1 to 6.3, 14.1 to 14.3, 15.10, 32.2 and R3.
- **32.2 is decided at the same time: A,** exact lines, arcs and ellipses from the start, gated by piece 2D-01's benchmark, with its fallbacks used only for the class of predicate whose cost fails (B1 for ellipse pairs; B2, then B3, for constructed circles), and never C. These forms are A's machinery; accepting them without A would have no use, and the board has no other piece that records A.
- **The forms and their budgets** are 15.10's: carriers below 2^41 and 2^82, depth-one points below 2^124 and 2^83, and carriers never defined from depth-one points, which keeps every predicate at a fixed width, the widest 337 bits. Each is generated by `geom-gen` with its bit-range certificate, like every predicate (6.2), and lives in `geom-num`, below every user (14.1).
- **Stored, not only computed.** These forms are stored values in 2D bodies, sketches and files, an explicit exception to "every stored coordinate is whole grid steps", as 6.3's planes are; a point becomes grid steps only when it must (export, a curved construction) and then by 6.3's commit and re-check.
- **What it makes exact:** 15.10's rows that use carriers, depth-one points and rational centres, T-junctions and fillets among them, are Exact rather than bounded.

Why: rounding a T-junction or a fillet centre to the grid leaves gaps that every later operation inherits (14, R3); these forms close them at a fixed, measured width inside the budget booleans already pay for.

**Changes:** 6.2, 14.1, 15.10, 32.2, 31.7, and pieces G187 and 2D-04.

## 32.27 Toolkit homes before a second program (owner action G238)

**What it is.** Section 26.1's rule: a domain toolkit enters the kernel when a second program needs it, and until then it lives in the one program that does. Chapters 13 and 17 give three toolkits kernel crates now: `geom-thread` (threads and helical features), `geom-stair` (stairs) and `geom-civil` (corridor daylighting, ground profiles by station, mass haul and sight distance, which combine terrain with alignments). Today only Spare Construct needs any of them. 27 and OQ-17.4 ask the owner to record an exception, or move each into Spare Construct with no kernel crate changing.

**Decided 2026-10-28 (owner action G238): no exception. All three follow 26.1's rule and start in Spare Construct.** Read with 13.8, 13.16, 17 (introduction, 17.5, 17.9), 26.1, 27's toolkit rule, R1 and R27.
- **The rule's reason holds for each.** A toolkit is built only from the kernel's public operations and kinds (26.1). Everything these three compose (sweeps, helices, booleans, terrain, alignments, grading) is a kernel operation the facade exposes, so Spare Construct can build them on `engine-geometry` with nothing missing, and the kernel's scope stays where R1 says it must: finished layers, not one program's features.
- **chapter 17's reason does not need the kernel.** It places `geom-civil` above `geom-terrain` and `geom-align` so that neither depends on the other; a toolkit in Spare Construct leaves them just as independent.
- **What moving later costs, and how it is kept small.** When a second program needs one, it enters the kernel as a toolkit crate under 26.1, with its operations' names and versions carried over unchanged, so models that recorded them replay; until then each operation is versioned in Spare Construct under the same edition rules (26.2).
- **What changes.** No kernel crate: the table's toolkit rows stay deferred, as G01's table already lists them ("added when two programs need them"). Board pieces G258 (`geom-thread`), G281 (`geom-stair`) and G378 (`geom-civil` compositions) are built in Spare Construct's crates first, on the facade, and return to this board when a second program needs them; they stay listed in 30.5 as not gating.

Why: the two-program rule is what keeps the kernel finishable (R1), and none of the three needs anything the facade does not give a program.

**Changes:** 17's introduction, 27's toolkit rule, 30.5's not-gating list and 31.7.

## 32.28 What the Sweep kind stores (owner action G239)

**What it is.** A swept surface is a procedural kind: its stored definition is the shape, and any NURBS is only a cached approximation with its bound (7, 13.3). Chapter 13 asks what that definition holds, its fifth question: the frame rule, the twist and scale laws, the station frames, and for a tapered revolve the profile morph IFC defines, and whether a conical helix (tapered springs, scrolls, tapered pipe threads) becomes a curve kind of its own.

**Decided 2026-10-28 (owner action G239): the Sweep kind stores its whole definition, and the helix kind gains a radius rate.** Read with 7, 13.3, 13.4, 13.6, 13.9, 24.3 and 32.3.
- **The Sweep kind stores:** its profile and path by reference; the `FrameRule` in full (its kind and every parameter it was given, never a name alone); the twist and scale laws as law curves of their own, exact where given exactly; for a rotation-minimising frame, the station frames F_0 … F_n bit for bit, as the `f64` rotations the double-reflection steps produced, because the stored definition is the shape and its closeness to the ideal frame is measured and labelled (32.3); and for a tapered revolve, IFC's profile morph, each pair of corresponding points moving with radius and height linear in the angle. 24.3's canonical bytes encode each field, and piece "geom-format: body records" writes it.
- **Draft surfaces** are Sweep or Ruled faces, whichever their construction is, never fitted NURBS.
- **The helix kind gains a radius rate:** a helix stores its axis frame, start radius, radius change per turn, pitch, handedness and parameter range, so a conical helix is exact like a cylindrical one (radius rate zero), and conical springs, scrolls and tapered pipe threads are Exact helices swept, not Certified Sweep faces. Section 7's helix entry becomes this one kind; its decision note under 26.1 is this section.

Why: a procedural face is only as replayable as its stored definition is complete, so every rule and law that shaped it is stored, and nothing is left to recompute differently later (26.2). Tapered threads are common enough in manufacturing that an exact form pays for itself.

**Changes:** 7's helix entry, 13.4, 13.6, 31.7, and pieces G254 (frame rules and laws) and "Helical sweeps and geom-thread".

## 32.29 Predicates of 576 and 768 bits (owner action G240)

**What it is.** Section 6.2 sets 384 bits as the budget for booleans. Chapter 13's exact straight skeleton (roofs, 2D offsets) orders events by height, two three-plane points compared, which needs 13B+27 = 547 bits at B = 40, so `I576`; its 3D mitre offsets order the times at which four moving planes meet, about 720 bits, so `I768` (6.2's table; 13.11, 13.15). 31.7 adds the wider integers already in code: `geom-num`'s `I512` total path (G02) and `geom-math`'s exact stages up to 2,560 bits (G03).

**Decided 2026-10-28 (owner action G240): accepted, as an addition to 6.2, on 6.2's own conditions.** Read with 6.2, 13.11, 13.15, 23.2, 23.3, R4, R7 and G42's WebAssembly measurements.
- **`I576` for skeleton event order and `I768` for mitre-offset event order** join 6.2's list, used only by those predicates. 384 bits stays the budget for booleans and every arrangement predicate.
- **Each meets what the 384-bit tests meet:** a written derivation of its bit budget in its module header, generated by `geom-gen` with a bit-range certificate checked by its independent checker, a test with every coordinate at ±(2^40 − 1), and a generated filter, so the wide path runs only where the filter cannot decide. G251 writes the derivations ("our derivations, to be written out", 31.7) before the types ship; if a derivation comes out wider, the type grows with it, never the other way round.
- **Their cost is measured, not assumed:** each joins the instruction-count benchmarks natively and in WebAssembly (23.3), and the count gate holds them like every predicate. A product in `I576` or `I768` costs about 5 or 9 times one in 256 bits (6.2), paid only by skeleton and mitre events.
- **The wider integers already in code are accepted as built,** each keeping the derivation and extreme tests it shipped with: `I512` as the total path of plane classification and commit, which grid input never reaches, and `geom-math`'s exact stages up to 2,560 bits for its general transforms. Each joins 23's cost model when measured; 6.2's list names them.

Why: exact roof and offset events are what make 13's skeleton exact once its planes are committed; without them those decisions would need tolerances. Fixed widths keep each predicate allocation-free and provable, and the filter keeps the wide path rare.

**Changes:** 6.2's list, 13.11 and 13.15, 31.7, and pieces G251 and G26's successors.

## 32.30 The close_near_contacts cap (owner action G241)

**What it is.** Booleans never treat near-coincident faces as coincident beyond the contact radius (11.5). A caller who wants two nearly touching operands to touch runs `close_near_contacts(d)` first: an explicit, logged repair that adopts one carrier for each pair within d and returns Certified with the movement it measured. Its largest d needs a default and a cap (OQ-11.7): reuse import healing's values, 640q by default and at most 6,400q, or add a rung of its own.

**Decided 2026-10-28 (owner action G241): ε_heal's values, as a second meaning of that rung.** The default d is 640q (0.1 mm at q0, 4 µm at q1) and no call may pass 6,400q (1 mm at q0, 40 µm at q1); a larger request is refused with `ExactnessNotAccepted`, naming the cap. Read with 6.5, 11.5, 24.9 and OQ-11.7.
- **It is the same act as healing.** Both close gaps that should not be there by moving geometry onto one carrier, explicitly, once, on record, and both are bounded by what a person would call a modelling gap rather than design intent. One rung for both keeps the ladder small (principle 10) and means a gap import would heal is a gap the repair can close.
- **The operation's own guarantees stay:** it measures and reports the actual movement, the result is Certified by that movement, and the operation's tolerance ceiling still applies when it is smaller. A caller who passes a smaller d gets a smaller repair.
- In code it uses `geom-num`'s `ladder::HEAL_DEFAULT` and `ladder::HEAL_CAP`; no new constant.

Why: a separate rung would be a second number for the same judgement; reusing ε_heal's values keeps one answer to "how big a gap is a mistake".

**Changes:** 6.5 (ε_heal's row and the proposed rungs), 11.5 and OQ-11.7, 31.7, and piece G249.

## 32.31 The contact benchmark's licence (owner action G285)

**What it is.** Piece G297 builds time of first contact by the Tight Inclusion method (MIT) and promises no false negative on Wang et al.'s 2021 benchmark for continuous collision detection: about 12,000 point-triangle and 15,000 edge-edge handcrafted queries and tens of millions from simulations ([doi:10.1145/3460775](https://doi.org/10.1145/3460775)). 19 asks an owner to check the benchmark's licence before it is used in tests, and 31.7 lists it among unverified licence facts.

**Checked 2026-10-28.** The benchmark is published on NYU's Faculty Digital Archive ([collection](https://archive.nyu.edu/handle/2451/61518): the handcrafted, simulation, rounded and full-scene datasets). The [handcrafted record](https://archive.nyu.edu/handle/2451/61519) states no licence, only the archive's default: "protected by copyright, with all rights reserved, unless otherwise indicated". The authors' [Sample-Scalable-CCD-Data](https://github.com/Continuous-Collision-Detection/Sample-Scalable-CCD-Data/tree/d1de594b017d98b37be67f02f74e541e74804109) repository, a sample of their later time-of-impact dataset with exact ground truth (six simulation scenes, symbolic roots), is under Apache-2.0. Tight Inclusion's code is MIT (29.3).

**Decided 2026-10-28 (owner action G285).** Read with 19.12, 28.4, 29.20 and 32.11's rule (i).
- **The 2021 benchmark is not stored and not run in CI** until its terms are cleared: all rights reserved is neither permissive nor CC, so 32.11's rule (i) makes it at most fetch-only once cleared. Clearing means the authors putting it under a licence 29.2 allows, or granting the co-op written permission to use it in tests; a request to them is drafted in this piece's pull request, and the owner sends it, since it goes to outside people in the co-op's name.
- **Meanwhile G297 tests on what is cleared:** the Apache-2.0 time-of-impact sample, stored with its notice (29.20, rule 4), Tight Inclusion's own MIT tests, and our own degenerate cases (parallel edges, touching at the start or end of the step, contacts at grid extremes), each with exact truth computed with `dashu` or `num-rational`. G297's done-when says so; the 2021 benchmark joins as a fetch-only corpus (G114) the day it is cleared.

Why: the method is permissive and the promise is the same; only the data waits, and data with no licence cannot sit in a co-op's public CI (32.11).

**Changes:** 19.12, 29.3's Tight Inclusion row, 31.7, and G297's done-when.

## 32.32 USD exchange home (owner action G312)

**What it is.** Subdivision nets need an exchange format that keeps creases, corners and OpenSubdiv's options: OBJ has no creases, 3dm waits on McNeel (32.9), and DXF's MESH is unverified (24.11). USD's Catmull-Clark meshes carry all of them except the creasing method, so Chaikin creasing, which 16.1 stores, has nowhere to go. No crate owns USD (16, questions 14 and 19; 31.7).

**Decided 2026-10-28 (owner action G312).** Read with 16.1, 24.8, 24.11, 27's crate table, 32.9, evidence report [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) section 1, and OpenUSD's `UsdGeomMesh` schema as 16.1 cites it.
- **`geom-meshio` owns USD subdivision exchange**, beside OBJ's subdivision nets, as board piece G354 already places it. USD's other content (scene graphs, materials, animation) stays with programs; the kernel maps only geometry, as for every format (24).
- **Chaikin creasing travels as one namespaced attribute on the Mesh prim:** `uniform token spareware:creaseMethod = "chaikin"`, written only when the net uses Chaikin. Absent, or `"uniform"`, means OpenSubdiv's uniform creasing, USD's own behaviour, so every other USD reader sees a valid mesh and loses only the creasing method, which it could not use anyway. A namespaced attribute rather than `customData` is chosen because USD composes and round-trips attributes through every layer operation, and the namespace keeps it clear of USD's own names.
- **How it reads and writes USD is G354's and G439's**, with the terms check 24.11 asks for (OpenUSD's TOST-1.0 licence and the AOUSD specification's terms) done before any USD source or schema text is used. A pure-Rust reader under a permissive licence, such as the MIT `openusd` crate, is checked there against 29's rules before anything is written by hand.

Why: one crate for subdivision exchange keeps OBJ and USD on one mapping of `geom-subd`'s options; a namespaced attribute is the smallest addition that round-trips through USD tools and degrades safely in readers that do not know it.

**Changes:** 16.1, 24.11, 31.7, and pieces G354 and G439.

## 32.33 Alignment curve kinds (owner action G357)

**What it is.** Road and rail alignments use transition curves beyond the clothoid: Helmert (two quadratic halves), Bloss, cosine and sine transitions, the curve IFC calls VIENNESEBEND, IFC's polynomial spirals and a cubic spiral variant (17.8). IFC 4.3 also builds 3D alignment curves from others: the gradient curve (a vertical profile over a horizontal alignment) and the segmented reference curve (cant over a gradient curve). LandXML 1.2 names sixteen spiral types without formulas. Section 26.1 asks a decision note for each new curve kind; chapter 17 asks which of these are named kinds and which compositions (its open question 5; 31.7). The trade-mark status of the VIENNESEBEND name is to be checked first.

**Decided 2026-10-28 (owner action G357).** Read with 7, 17.8, 17.13, 24.11, 26.1, IFC 4.3's `IfcAlignmentHorizontalSegmentTypeEnum` as 17.8 cites it, and LandXML 1.2's schema. This section is the decision note 26.1 requires.
- **One transition kind with a closed set of curvature laws,** in `geom-curve`: clothoid, Helmert, Bloss, cosine, sine, the cant-coupled transition (IFC's VIENNESEBEND), IFC's second-, third- and seventh-order polynomial spirals, and the cubic spiral variant. Each law is a variant of one closed enum, so every law shares one evaluation (heading as the exact integral of curvature, position by certified Gauss–Legendre quadrature), one bound and one kind contract, and a new law is a new variant with its own decision note. Results are Certified, as 17.8's table says.
- **The cubic parabola** (IFC's CUBIC, y = x³/6RL) is not a transition kind: it is a polynomial curve, exact in its own parameter, so it is stored as one; its length stays certified.
- **The gradient curve and the segmented reference curve are compositions, not kinds:** `geom-align` stores the horizontal alignment, the vertical profile and the cant as their own segment lists, and the 3D curve is evaluated from them by station, so a change to the profile never rewrites a stored curve. `geom-ifc` maps IFC's two entities onto them and back.
- **The name.** "Wiener Bogen" (the Viennese bend) is reported to be a registered trade mark; that is unverified, but the kernel needs no brand. The law is named for what it is, the cant-coupled transition, and only IFC's VIENNESEBEND and railML's and LandXML's own enumerator values are read and written, as the files' data. As 17.8 says, it takes Δψ and h from its cant segment and becomes a seventh-order polynomial spiral that needs no cant to evaluate.
- **LandXML's spirals** map as 17.13 reads them: clothoid, bloss, cosine, sinusoid, biquadratic, weinerBogen, cubic and cubicParabola to the laws above, the `rev` types run from the other end. sineHalfWave, biquadraticParabola, japaneseCubic and radioid have no published formula, so they stay LandXML data, and an operation that must evaluate one fails with an error naming it, never a guess. A published definition, with its own decision note, makes one a law.

Why: one kind with a closed set of laws keeps every transition on one proven evaluator and one test kit; compositions keep IFC's layered alignment model editable; and a descriptive name keeps a possible trade mark out of the kernel.

**Changes:** 7's curve list, 17.8, 17.13, 31.7, and piece G371.

## 32.34 TIN tie order (owner action G358)

**What it is.** When four or more survey points lie exactly on one circle, a Delaunay triangulation has more than one valid set of triangles, and the kernel must choose one deterministically. Section 6.2 breaks every tie by symbolic perturbation ordered by persistent names (21). Chapter 17 objects for terrain: two separate imports of the same points get different names and may choose different diagonals, so identical surveys can give different surfaces. It asks to order these ties by plan position instead, x then y (17.3; 31.7).

**Decided 2026-10-28 (owner action G358): plan position, for TIN vertices.** Read with 6.2, 17.2, 17.3, 21 and Devillers and Teillaud's perturbation for Delaunay ([doi:10.1016/j.comgeo.2010.09.010](https://doi.org/10.1016/j.comgeo.2010.09.010)).
- **A TIN's ties are broken by the vertices' plan coordinates, x then y,** in Devillers and Teillaud's symbolic perturbation. A TIN has at most one vertex at a plan position (17.3's duplicate rules merge or refuse the rest), so plan position is a total order on its vertices, and a persistent key in 6.2's sense: it is the data the surface is built from, and it changes only when the point itself moves.
- **What it gives:** the same points give the same surface whatever they are called and however they were imported, so two surveys, a re-import or another program's TIN of the same points compare triangle for triangle. A renamed point still changes nothing, as before.
- **Everything else keeps name order.** The exception is for the Delaunay ties of TINs only, where the vertices are survey points whose positions are the identity; 6.2's rule stands for every other tie, where the same geometry under different names should stay distinct (20.2).

Why: for terrain the point set is the model; a surface that depends on point names makes identical surveys disagree, which the name rule exists to prevent elsewhere.

**Changes:** 6.2 (the exception), 17.3, 31.7 and piece G362.

## 32.35 Smooth-edge marking (owner action G384)

**What it is.** A drawing shows an edge where faces meet tangent but curvature jumps (a fillet's edge) as Smooth, from the continuity record, which says tangent only within ε_ang. Imported fillets are rarely that tangent, and a faceted IFC element's triangle edges are not coplanar, so without more information they draw Sharp, which is what the data says. Chapter 18's fourth question: mark such edges Smooth by an explicit, logged operation whose angle is its own recorded parameter, with no kernel default, or by a named import angle on 6.5's ladder (18.7; 31.7).

**Decided 2026-10-28 (owner action G384): the logged operation, with no kernel default.** Read with 6.5, 18.7, 21, 24.8 and principle 10.
- **The operation** marks edges Smooth where adjacent faces turn by less than an angle θ the caller passes. θ is recorded in the operation log like any other input, so replay marks the same edges, and the marking is an attribute edit (18.7), never a change to geometry or to its exactness class.
- **No kernel default and no ladder rung.** The right θ depends on the source (a STEP fillet, a coarse IFC mesh, a scanned surface) and on the drawing's use, which only a program knows. A program that wants a default for its imports sets one and records it in the log, as every default is recorded (21). The kernel's only angle rung, ε_ang, stays what it is, far too tight for this.
- **Source data first:** where an import carries its own smoothing (shared vertex normals in an IFC triangulated face set), that marks edges Smooth without the operation, as 18.7 says.

Why: a hidden import angle would be a tolerance no one chose and that differs from one program's expectations to the next; a logged parameter is a decision on record, which replays and can be changed.

**Changes:** 6.5's proposed rungs, 18.7, 31.7 and piece G398.

## 32.36 View predicates beyond 384 bits (owner action G385)

**What it is.** Exact hidden lines for flat geometry decide every visibility event by the sign of an integer expression on implicit points made from face planes and derived planes through each outline edge (18.6). The deepest test, ordering two events along one edge, needs about 13B + 27 = 547 bits on a view along a grid axis and 15B + 33 = 633 bits for any direction or a perspective, so `I576` and `I640`: past 6.2's 384 bits. Chapter 18 objects to the limit for these predicates; the alternative is exact visibility on grid-axis views only, certified elsewhere (31.7).

**Decided 2026-10-28 (owner action G385): accepted for every view, on the conditions owner action G240 set (32.29).** Read with 6.2, 18.6, 23.2, 23.3, 32.29, R4 and R7.
- **`I576` for grid-axis views and `I640` for any direction and for perspective** join 6.2's list, used only by the view predicates. Plans, sections and elevations, the commonest drawings, take the narrower width.
- **The same conditions as 32.29:** a written derivation of each budget (18.6 gives the estimates; piece G388 writes them out, and a wider result widens the type), generation by `geom-gen` with a checked certificate, a test with every coordinate at ±(2^40 − 1), a generated filter so the wide path runs only near degenerate cases, and instruction counts natively and in WebAssembly under the count gate.
- **Exact everywhere, not only on axis views.** An axis view is no more important than an axonometric or a perspective a person checks a design by, and certified visibility would let an edge flicker between visible and hidden in exactly the near-degenerate cases a drawing checker must not miss (18.10).

Why: drawings are a contract document; exact visibility makes every view the same on every machine and every replay, and the filter keeps the cost to the rare near-degenerate events.

**Changes:** 6.2's list, 18.6, 31.7 and piece G388.

## 32.37 openNURBS licence confirmation (owner action G407)

**What it is.** Reading and writing Rhino's `.3dm` files (G438) means porting openNURBS, McNeel's toolkit for them. Its terms contradict each other: the repository's [LICENSE](https://github.com/mcneel/opennurbs/blob/eb92af3ba1806b0a34a99aba0d3bda83e3d46083/LICENSE) says "All Rights Reserved" and grants nothing, while McNeel's [developer page](https://developer.rhino3d.com/guides/opennurbs/what-is-opennurbs/) says "There are no restrictions. Neither copyright nor copyleft restrictions apply" and "Commercial use is encouraged" (both read 2026-10-28). 29.20's first rule stops on such a conflict, and 32.9 recommends writing to McNeel now for written confirmation that the grant covers the repository at a named commit, including use inside MPL-2.0 work.

**Decided 2026-10-28 (owner action G407): ask McNeel for written confirmation; port nothing until it arrives.**
- **The request** is drafted in this piece's pull request. It names the commit, the use (porting the reader and writer into an MPL-2.0 kernel, with McNeel's notice kept) and the two texts that disagree. The owner sends it, since it is a letter to an outside company in the co-op's name; nothing has been sent.
- **Until the reply,** openNURBS stays as 29 lists it: read for the format only, run out of process as an oracle, never ported. G438 does not start, and it does not gate its phase (30.5).
- **When the reply comes,** it is recorded here, in 29's openNURBS row and in `geometry/NOTICE`, word for word with its date; a yes lets G438 port with McNeel's notice kept, and a no or a narrower grant is recorded and G438 is re-planned from the published format description alone.

This action stays open until McNeel answers.

**Changes:** 29's openNURBS row, 31.7, and piece G438.

## 32.38 LandXML terms (owner action G408)

**What it is.** `geom-landxml` (G437) reads and writes LandXML 1.2 surfaces, alignments, profiles, COGO points and parcels (17.13). The schema is published at landxml.org with no terms found, and it cannot be pinned to a commit (29). New Zealand's survey authority, LINZ, publishes LandXML reference documents and examples for surveyors and developers; LINZ licenses almost all its published data under CC BY 4.0, but the LandXML example pages state no terms of their own (both checked 2026-10-28; landxml.org did not answer that day).

**Decided 2026-10-28 (owner action G408).** Read with 17.13, 24.11, 29.20's rules 1 and 11, and 32.11's rule (i).
- **The schema is implemented, never copied.** LandXML 1.2 is a published exchange format; `geom-landxml` maps the elements it needs from the schema's definitions, as 29 already classes it (PORT METHOD), with no code generated from the XSD and no copy of the XSD in the repository. A test that validates our output fetches the schema at test time, records its SHA-256, and fails if it changes.
- **LINZ's examples are fetch-only** until their own download page states terms. If those terms are CC BY 4.0, as LINZ's other data is, the files may be stored with LINZ's attribution under 32.11's rule (i); until then they are fetched by the corpus register (G114) and never committed. Co-op-authored LandXML files are the stored fixtures meanwhile.
- **The sixteen spiral names** are file data, mapped as 32.33 says.

Why: implementing a published format from its definitions copies nothing, and data whose terms are unstated stays out of a public repository, as for every outside dataset (32.11).

**Changes:** 29's LandXML and LINZ rows, 31.7, and piece G437.

## 32.39 Generating code from STEP and IFC schemas (owner action G409)

**What it is.** Piece G416 generates Rust types and validators from the EXPRESS schemas of IFC 4.3, IFC4, IFC2x3 and the STEP application protocols, so readers and writers never hand-type thousands of entities. 29 records that ISO's terms for generating code from its EXPRESS files are unverified, and that it is unknown which edition of ISO 10303-42, the geometry and topology resource every STEP geometry entity comes from, is current (29, question 11; 31.7).

**Checked 2026-10-28.** ISO's catalogue lists [ISO 10303-42:2025](https://www.iso.org/standard/91386.html), edition 9, as published; the 2024 edition replaced the 2022 one and the 2025 edition replaces it. ISO's machine-readable EXPRESS listings (the SMRL archives under `standards.iso.org/iso/10303/`) are marked informative, and no blanket licence for them was found: ISO's default terms forbid reproduction without written permission, and only some schema files carry a grant of their own in their headers. buildingSMART's IFC 4.3 [LICENSE.md](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/LICENSE.md) is CC BY-ND 4.0 and adds that its technical documentation may be "used in software development, or translated into another computer language" with full attribution (29.14).

**Decided 2026-10-28 (owner action G409).** Read with 24.6, 24.11, 29.14, 29.20 and evidence report [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md).
- **Part 42's current edition is ISO 10303-42:2025.** `geom-exchange`'s mapping follows its entity definitions, and still reads files written against earlier editions, whose entities it maps where they agree and reports where they do not.
- **IFC schemas may be generated from,** under buildingSMART's software-development permission with full attribution in every generated file's header; G416 confirms the same text in IFC4's and IFC2x3's licences at pinned commits before generating from them.
- **ISO's STEP schemas are not generated from until permission is in hand:** a schema file whose own header grants use for implementation may be used, and G416 records that header per file; otherwise the co-op asks ISO's copyright office, on a request drafted in this piece's pull request that the owner sends. Until then G416 generates the IFC types, and `geom-step`'s geometry entities are written from the standard's normative text, which is implementing a standard, not copying it (29's rule for ISO standards); whether to buy a licensed copy for that is the owner's when `geom-step`'s pieces start.

Why: generated types are the safe way to read thousands of entities, so the permission matters; IFC grants it, ISO's files must be asked for, and nothing waits on ISO that IFC can serve first.

**Changes:** 29's ISO and IFC rows, 31.7, and piece G416.

## 32.40 Gerber and IPC-2581 terms (owner action G447)

**What it is.** Circuit-board exchange (G474, G475) reads and writes Gerber X2 and X3 and IPC-2581. Ucamco publishes the Gerber specification free to download; its section 1.4 grants no licence to the document's intellectual property and asks interface developers not to make "derivative versions, modifications, or extensions" or "alternative interpretations of the data" without Ucamco's approval. IPC-2581 revision C is sold by IPC, and that the format is licence-free comes only from secondary sources (24.11). By 26.1 the crates enter the kernel only when two programs need board exchange; the owner records whether that is so (24's question 12).

**Decided 2026-10-28 (owner action G447).** Read with 24.11, 26.1, 29.19, 29.20 and Spareware's STATUS.md.
- **No second program needs board exchange today.** No Spareware program designs or draws circuit boards, so `geom-gerber` and `geom-ipc2581` stay deferred, G474 and G475 do not start, and they stay not gating (30.5). The first program that needs boards uses `geom-2d` through the facade in its own adapter, as 24.11 says; a second one brings the crates into the kernel.
- **Gerber, when built,** is implemented exactly as the published specification defines it, with no extension, private attribute or reinterpretation of the data, which is what section 1.4 asks; our own data travels in the format's standard attributes or not at all. The revision implemented is pinned and its section numbers cited in the reader's header.
- **IPC-2581, when built,** waits for its terms to be read in the standard itself: the co-op buys a copy then, which is the owner's spending decision at that time, or IPC confirms in writing that implementation needs no licence. Nothing is built from secondary descriptions.

Why: the two-program rule decides the timing, and both formats can be implemented faithfully without taking anything their owners reserve; nothing needs money or a commitment until a program needs boards.

**Changes:** 24.11, 29.19's board row, 31.7, and pieces G474 and G475.

## 32.41 AutoCAD truth tests (owner action G448)

**What it is.** Piece G464 runs at least 20 co-op command sequences in AutoCAD and in Spare Kit and compares the DXF. It needs an owner's ruling that running AutoCAD for comparison is allowed under its terms, and a licensed seat (28.11, 32.10). The first Stable release, G482, needs G464.

**Decided 2026-10-28 (owner action G448).** Read with 14.14, 28.3, 28.9, 28.10, 28.11, 29.20's rule 8 and 32.10.
- **The ruling stays 32.10's:** watching AutoCAD from outside is allowed only to check our results against our own rules, on co-op command sequences, on a seat the co-op itself is licensed for, and only after AutoCAD's terms are checked for that use (the terms check 32.10 asks Spare Construct to add beside CON-A18-F2-P2). It never sets behaviour, an expected value or a tolerance (28.11).
- **No seat is bought or borrowed now.** The co-op has no AutoCAD licence of its own, and a seat licensed to anyone else is not the co-op's to use. Buying one is a spending decision for when the terms check clears.
- **So drafting truth does not wait for AutoCAD.** G464 checks the command sequences by what 28.11 already names as the truth for drafting: 28.3's 2D laws, 28.9's one-root arrangement reference, and LibreCAD and ezdxf out of process (28.10), each difference triaged against our own rules. The AutoCAD comparison joins G464 when a co-op seat and a cleared terms check both exist, and the first Stable release does not wait for it.

Why: a comparison that can never make AutoCAD's behaviour our requirement adds least where the open checks already decide correctness, and it is not worth a licence the co-op does not hold or terms nobody has read.

**Changes:** 28.11, 31.7, and G464's done-when.

## 32.42 The wall-clock merge rule (owner action G449)

**What it is.** Principle 12 blocks any speed regression over 5%, but wall-clock time on shared CI machines varies by more than that, so pull requests are gated on instruction, cache, heap and fuel counts, which do not vary (23.13). Wall-clock time and scaling are measured nightly on the reference machine against 23.1's shares. Spare Construct says a target miss blocks any change that misses it; per pull request that is impossible to measure, so 23.13 proposes that a nightly miss blocks further merges to `main`, except the fix or a revert, until the target is met again.

**Decided 2026-10-28 (owner action G449): accepted, with a confirmation run.** Read with 23.1, 23.13, 28.17, 32.12 and 32.16.
- **A miss is a confirmed miss:** a nightly result over a share is rerun the same night on the reference machine (32.16); only if the rerun also misses does the block start, so one noisy run never stops the co-op.
- **While blocked,** only a pull request that fixes the miss, or reverts the change bisection names as its cause, may merge to `main`, and its own nightly run must meet the share again. G452 enforces it with a required status that the nightly job sets and clears.
- **A deliberate rise is not a miss.** A correctness change that costs time is accepted the way 32.12 accepts a count rise: recorded by a kernel owner with the shares it moves, and the share itself updated by a decision note (32.16), never by lifting the block.
- **If the reference machine is unavailable** (it is lent, 19:00 to 08:00), no run means no block; the count gates still hold every pull request.

Why: a target that only warns is not a target, and blocking the next merges rather than the guilty one is the only way to enforce a nightly measurement; the confirmation run keeps noise from blocking work.

**Changes:** 23.13, 31.7, and piece G452.

## 32.43 Gate 1: ground and numbers proven (G43)

**What it is.** Gate 1 closes phase 1 (30.3): before any phase-2 piece leans on the numbers layer, every condition of G43's done-when holds, and a kernel owner records the result here with the CI runs that show it (30.1). This is the first gate, so its record also sets the form: one row per condition of the done-when, in its words, with the evidence and the CI run; then every merge gate with the violation that is seeded to prove it can fail; then the readings the gate took.

**Result: not yet passed (drafted 2026-10-29).** GitHub Actions started no job for most of 2026-10-28 after the organisation moved to GitHub Enterprise, and runs again from 2026-10-29; no CI run on the gate's commit exists yet. Every check that can run on the owner's workstation was run there on G42's branch with this piece's changes: the verification suite and the thread-count tests, Verus (113 verified, 0 errors, with the release CI pins), and each planted violation of the table below that can run without Docker: rustfmt, Verus, Kani three ways and libFuzzer each caught theirs; Gungraun, under Valgrind 3.22 on Ubuntu 24.04, reported the planted slowdown of `sign/of_i128/typical` (instructions 1,054 to 1,247, +18.3%; estimated cycles +27.7%) and Wasmtime's fuel comparison reported it too (2,678 to 3,830, +43.0%, the only rise), each passing the same command with nothing planted; and `guard prove --golden-run --target wasm32-wasip1 --release` tripped under Node's WASI (`geom-num/determinism` alone differed from the store; the eleven other goldens agreed). ClusterFuzzLite's seeded job needs Docker, which the workstation does not have; there, the target it fuzzes, built by `.clusterfuzzlite/build.sh`'s own `cargo bolero build-clusterfuzz`, crashed on the planted line. Its first CI run (run 40795242357) built the planted tree and found the crash, reproducibly, through `variants_fuzz` (tpi_side's law broken), but its address sanitizer reports the abort in its own words, not libFuzzer's, so the verdict now reads ClusterFuzzLite's report and the target's "broken law". G39 (#243), G42 (#260) and G40 (#264) have merged, and the certificates' gates have their seeded violations (#368): on the workstation, Gappa 1.8.3 refused orient2d's problem with its bound halved ("some properties were not satisfied", its best enclosure the shipped K, ±4.4409e-16) and Rocq refused what Gappa then wrote ("The reference l1 was not found"), and HOL Light refused orient2d's certificate with its bounds halved at the first of its eight ("prove_bound_cond"). The gate passes when every row below names a green CI run on the gate's commit. `geom-test`'s `tests/attack_g43.rs` holds this record to the board, the workflows and the tests: a condition missing, a cited test or job gone, or a pull-request job with no row fails it, and so does a result of "passed" while a row is open.

| Condition | Evidence | CI run |
| --- | --- | --- |
| Every predicate in geom-num passes 28.6's extreme-value tests and the rational oracle on every pull request | `geom-num/tests/verification.rs::every_predicate_is_verified_in_every_class`: the twelve public predicates by five classes, each cell a test that now must reach its predicate in its code (it counted the matrix's own strings before; tightening it found three gaps, closed here: the perturbed orient3d and insphere at their sign corners, insphere's ties against an oracle, `geom-num/tests/verification.rs::perturbed_ties_match_a_symbolic_oracle`, and the three-plane point one step from degenerate in `geom-num/tests/attack_g29.rs::one_step_from_degenerate_is_decided_exactly`). `kernel.yml` runs them on every pull request that touches the kernel, on six runners in debug and release and on wasm32 under wasmtime and Node | pending |
| Wide integers and orientation predicates are proven in Verus | `kernel-dev.yml` job `verus` verifies `geom-num/verus/lib.rs`: the wide integers' twin (G37) and the twins of orient2d, orient3d, incircle and insphere (G38), held to the shipped code token for token by `geom-gen/tests/verus_twin.rs::any_change_to_either_side_is_seen`; G39 (merged, #243) added the plane-based and implicit-point predicates | pending |
| The filters of orient2d, orient3d, incircle and insphere are proven by at least one tool, and every other filter is labelled 'checked, not proven' with an open issue | `kernel-certificates.yml`: Gappa's proofs, checked by Rocq, of nine of the eleven generated filters, these four among them (G35, G40); tpi_side and lpi_side say "checked, not proven" in their module headers and in `MAP.json`, naming #292, which is open | pending |
| The determinism matrix is green on all seven targets, both build modes and 1, 2, 3 and 8 threads | `kernel.yml` jobs `test` (x86-64 and arm64 Linux, Windows and macOS, debug and release, and release tuned to the runner's CPU) and `wasm` (wasmtime and Node, three modes), each running every golden of the store (`geom-test`'s `golden run`); `geom-test/tests/attack_g10.rs::the_matrix_has_every_runner_in_both_modes` holds the legs. The thread counts are one constant, `geom_test::golden::THREAD_COUNTS`, which every same-bits-on-any-thread-count test now runs (`geom-run/tests/golden.rs::golden_hash_is_the_same_on_every_target_thread_count_and_chunk_size`, `geom-pool/tests/attack_g33.rs::attack_rayon_pools_give_the_serial_answer_at_every_thread_count_and_chunk_size`, `geom-math/tests/determinism.rs::golden_hash_is_the_same_on_every_thread_count`, `geom-num/tests/attack_g05.rs::identical_bits_on_every_thread_count`) | pending |
| Every merge gate fails on a seeded violation | The table below: every job of every workflow that runs on a pull request, with the test or planted violation that makes it fail | pending |
| No unexplained mutant survives in geom-num | `kernel-quality.yml`'s full run (`workflow_dispatch` on the gate's commit): every mutant of geom-num (5,147 on G42's branch), judged against the ledger, which keeps 22 survivors, 21 equivalent and 1 `hangs`, each with its reason; no full run has been made yet, only each piece's mutants in its diff | pending |
| Budgets give identical results serially and in parallel | `geom-run/tests/attack_g23.rs::attack_serial_and_parallel_phases_agree_exactly` and `geom-run/tests/attack_g23.rs::attack_serial_and_parallel_batches_agree_exactly` (answers, exhaustion errors and the meters' use, serially and on 2, 3 and 8 threads and a backwards pool, at chunk sizes 1 to 4,096), `geom-run/tests/determinism.rs::random_cases_match_the_phase_model` and the golden above, which hashes every outcome | pending |

Every merge gate and the violation seeded to make it fail. R: the real tool runs on a planted violation; J: the gate's judge runs on seeded input or seeded tool output; `trip <name>`: the job itself runs its tool on the planted violation of that name (`geom-test`'s `trip`, gate 1), first, or last where it compares with a run the job measured.

| Workflow | Job | Seeded violation |
| --- | --- | --- |
| `kernel.yml` | `test` | J: `geom-test/tests/attack_g19.rs::a_run_that_does_not_prove_its_goldens_is_refused`; R: the job `guard` |
| `kernel.yml` | `guard` | R: the job itself (`libm`'s `sin` changed by one bit must fail a golden); J: `geom-test/tests/attack_g20.rs::a_trip_counts_only_for_the_right_reason` |
| `kernel.yml` | `wasm` | R: each leg first runs `guard prove --golden-run --target wasm32-wasip1 --release`: `libm`'s `sin` changed by one bit must make the leg's own `golden run` fail, under wasmtime and under Node, with `geom-num/determinism`'s digest differing from the store's; J: `geom-test/tests/attack_g19.rs::a_run_that_does_not_prove_its_goldens_is_refused` |
| `kernel.yml` | `lint` | `trip fmt`; R: `engine-geometry/tests/scaffold/gate.rs::gate_clippy_fails_on_each_seeded_violation` |
| `kernel-build.yml` | `list` | Not a gate: it lists the crates the other jobs build, and fails when it lists none |
| `kernel-build.yml` | `build` | R: `engine-geometry/tests/scaffold/gate.rs::gate_build_fails_on_seeded_violations` |
| `kernel-build.yml` | `no-std` | R: `engine-geometry/tests/scaffold/gate.rs::gate_build_fails_on_seeded_violations` |
| `kernel-build.yml` | `lints` | The tools of `kernel.yml`'s `lint`: `trip fmt`; R: `engine-geometry/tests/scaffold/gate.rs::gate_clippy_fails_on_each_seeded_violation` |
| `kernel-build.yml` | `scaffold` | J: `engine-geometry/tests/scaffold/main.rs::the_real_check_refuses_global_state_seeded_into_the_repository` |
| `kernel-gate.yml` | `rules` | J: `engine-geometry/tests/scaffold/gate.rs::gate_workflow_fails_on_seeded_faults`, `engine-geometry/tests/scaffold/deny.rs::each_rule_refuses_its_seeded_violation` |
| `kernel-gate.yml` | `deny` | R: `engine-geometry/tests/scaffold/deny.rs::gate_deny_licences_fail_on_seeded_violations`, `engine-geometry/tests/scaffold/deny.rs::gate_deny_advisories_fail_on_seeded_violations` |
| `kernel-gate.yml` | `clippy` | R: `engine-geometry/tests/scaffold/gate.rs::gate_clippy_fails_on_each_seeded_violation`, `engine-geometry/tests/scaffold/gate.rs::gate_clippy_fails_on_a_ban_that_lapsed` |
| `kernel-gate.yml` | `builds` | R: `engine-geometry/tests/scaffold/gate.rs::gate_build_fails_on_seeded_violations` |
| `kernel-gate.yml` | `recursion` | R: `engine-geometry/tests/scaffold/recursion.rs::gate_recursion_fails_on_seeded_violations` |
| `kernel-gate.yml` | `gate` | J: `engine-geometry/tests/scaffold/gate.rs::gate_workflow_fails_on_seeded_faults`, `engine-geometry/tests/attack_g12.rs::attack_gate_workflow_passes_jobs_that_cannot_fail` |
| `kernel-dev.yml` | `native` | J: `geom-test/tests/dependency_rule.rs::seeded_violations_are_caught`; R: `geom-fuzz/tests/predicates.rs::bolero_fails_on_a_finding`, `geom-gen/tests/write_check.rs::a_hand_edited_output_fails_check`, `geom-test/tests/map_files.rs::the_binary_exits_0_1_and_2`, `geom-test/tests/scan_seeded.rs::a_seeded_file_is_found_with_its_file_and_line`, `geom-test/tests/ledger_seeded.rs::the_binary_exits_0_1_and_2` |
| `kernel-dev.yml` | `wasm` | The tests of `native` on wasm32: a failing test fails it, and the checks they test are seeded under `native` |
| `kernel-dev.yml` | `verus` | `trip verus` |
| `kernel-golden.yml` | `store` | J: `geom-test/tests/attack_g19.rs::seeded_violations_of_the_real_tree_are_found`; R: `geom-test/tests/attack_g19.rs::golden_diff_holds_a_change_to_the_owner_rule_end_to_end` |
| `kernel-golden.yml` | `fixtures` | J: `geom-test/tests/fixture_tool.rs::a_tree_that_breaks_a_rule_fails_its_check`; R: `geom-test/tests/fixture_tool.rs::diff_holds_a_pull_request_to_the_rules` |
| `kernel-bench.yml` | `scope` | Not a gate: it decides which count jobs run, from the same paths as the workflow's trigger |
| `kernel-bench.yml` | `checks` | The tools of `kernel.yml`'s `lint`: `trip fmt`; R: `engine-geometry/tests/scaffold/gate.rs::gate_clippy_fails_on_each_seeded_violation` |
| `kernel-bench.yml` | `instructions` | `trip gungraun`, on both legs; J: `geom-bench/tests/attack_g18a.rs::a_rise_over_5_percent_on_a_count_of_200_fails`, `geom-bench/tests/attack_g18a.rs::the_command_line_passes_fails_and_refuses_as_the_rule_says` |
| `kernel-bench.yml` | `fuel` | `trip fuel`; J: `geom-bench/src/counts.rs::growth_passes_the_stated_order_and_fails_the_next`, `geom-bench/src/counts.rs::compare_finds_rises_listings_removals_and_additions` |
| `kernel-fuzz.yml` | `libfuzzer` | `trip libfuzzer` |
| `kernel-fuzz.yml` | `kani` | `trip kani-fuzz` |
| `kernel-fuzz.yml` | `kani-num` | `trip kani-num` |
| `kernel-fuzz.yml` | `kani-op` | `trip kani-op` |
| `kernel-fuzz.yml` | `kani-format` | `trip kani-format` |
| `kernel-fuzz.yml` | `kani-limbs` | `trip kani-limbs` |
| `kernel-fuzz.yml` | `clusterfuzzlite` | `trip clusterfuzzlite`, run by the job `clusterfuzzlite-seeded` through this job's actions' own images with this job's inputs: the actions build a fresh clone of the pull request, which a planted line never reaches |
| `kernel-fuzz.yml` | `clusterfuzzlite-seeded` | `trip clusterfuzzlite`: the job is `clusterfuzzlite`'s seeded run, and fails if ClusterFuzzLite's build and fuzzing stop finding the planted crash |
| `kernel-quality.yml` | `mutants-plan` | J: `geom-test/tests/quality_gates.rs::shard_plans_fit_the_budget_and_a_matrix` |
| `kernel-quality.yml` | `mutants` | R: `geom-test/tests/seeded_gates.rs::a_seeded_survivor_fails_the_mutation_gate_until_a_test_catches_it`; J: `geom-test/tests/kernel_quality_workflow.rs::every_gate_is_judged_and_tool_failures_are_not_swallowed` |
| `kernel-quality.yml` | `mutants-2` | The tests of `mutants`: shards 256 on, with its steps word for word (J: `geom-test/tests/kernel_quality_workflow.rs::every_shard_and_partition_runs_once`) |
| `kernel-quality.yml` | `mutants-judge` | R: `geom-test/tests/seeded_gates.rs::a_pull_request_that_adds_a_survivor_fails_the_diff_gate`; J: `geom-test/tests/quality_gates.rs::stale_entries_fail` |
| `kernel-quality.yml` | `coverage` | R: `geom-test/tests/seeded_gates.rs::seeded_uncovered_code_fails_the_coverage_gate_until_tests_reach_it` |
| `kernel-quality.yml` | `gates-seeded` | R: the job itself, which runs `geom-test/tests/seeded_gates.rs` with the real tools |
| `kernel-quality.yml` | `miri` | Not a gate: Miri runs weekly (28.17), and its pull-request leg only checks its own set-up when that changes. One test is ignored under Miri: `geom-pool/tests/attack_g33.rs::attack_the_pool_is_reentrant`, while the lock holds rayon-core 1.13.0, which can free a scope while a job still holds a protected reference to it (rayon-rs/rayon#1328, Tree Borrows; fixed by rayon-rs/rayon#1330, in no release yet). Every rayon scope has the race, so rayon-core is upgraded to the first release that contains rayon-rs/rayon#1330, and the ignore is then dropped |
| `kernel-certificates.yml` | `gappa` | `trip gappa`: orient2d's Gappa problem with its claimed bound K halved, which Gappa must fail to prove (its report names the property and the best enclosure it found, the shipped K), so that its Rocq output holds no theorem and Rocq refuses it |
| `kernel-certificates.yml` | `fptaylor` | `trip hol-light`, on orient2d's leg: every bound of `FPTaylor`'s certificate halved as HOL Light's session loads it, which HOL Light must refuse to prove |
| `kernel-certificates.yml` | `fptaylor-problems` | Not a gate: it lists the FPTaylor problems, one `fptaylor` job each |
| `kernel-fuzz-campaign.yml` | `campaign` | Not a gate: nightly, or on a pull request only when it is labelled `fuzz-campaign` |
| `kernel-variants.yml` | `native` | Not a gate: timings for the variant report, on a pull request only when it is labelled `variant-bench` (its header: never a gate) |
| `kernel-variants.yml` | `wasm` | Not a gate: as `native` |
| `kernel-variants.yml` | `browsers` | Not a gate: as `native` |
| `kernel-variants.yml` | `report` | Not a gate: as `native` |

**Readings the gate took:**
- **"Every predicate in geom-num"** is every public function of `geom-num` that returns a predicate's sign: the explicit-point predicates, `Plane::side`, `side_of_tpi`, `side_of_lpi`, `Ssi::orient2d` and the four perturbed predicates, the twelve rows of the matrix. Avnaim et al.'s signs (`geom_num::avnaim`, G42) are 23.3's reference for the exact paths, not a predicate the kernel answers with, and the generated modules' `filter`, `exact` and `value` are the predicates' parts; both are held to the predicates' exact signs on random and extreme inputs (`geom-num/tests/attack_g42.rs`, `geom-num/tests/generated_predicates.rs`).
- **"All seven targets"** are chapter 21's five native targets and wasm32 under wasmtime and under Node. Windows on Arm, which 32.4 accepted, is an eighth, and runs on every pull request with the others. WebAssembly runs the thread-count tests on one thread until a threaded target is tested (21, 30.3).
- **"Every merge gate"** is every job of a workflow that runs on a pull request and can fail it (28.17's first tier), including the ones branch protection does not require (it requires only `kernel-gate.yml`'s `gate`, since the others run only when their paths change). A job that decides nothing (a list, a scope) is not a gate. A job that only runs tests fails when a test fails; the checks those tests make are seeded where their own rows say.
- **The full mutation run** is the evidence for geom-num, not the sum of the diffs: a pull request that only weakens a test mutates nothing in its diff, so only the full run shows that no unexplained mutant survives.

**Changes:** this record; `geom-num`'s verification suite and `tests/attack_g29.rs`; the thread-count tests of `geom-run`, `geom-pool`, `geom-math` and `geom-num`; `geom-test`'s `rust_source::references`, `golden::THREAD_COUNTS`, `trip`, `guard prove --golden-run` and `tests/attack_g43.rs`; the `trip` steps of `kernel.yml`, `kernel-dev.yml`, `kernel-fuzz.yml`, `kernel-bench.yml` and `kernel-certificates.yml`, the wasm legs' guard step, and `kernel-fuzz.yml`'s job `clusterfuzzlite-seeded` with `.clusterfuzzlite/seeded.sh`; `geom-bench/tests/attack_g18a.rs`'s count of the instruction job's measured runs, which the seed raises to seven; issues #367 and #368.
