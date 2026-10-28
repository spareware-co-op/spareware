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
