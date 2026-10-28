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

## 32.43 Gate 1: ground and numbers proven (G43)

**What it is.** Gate 1 closes phase 1 (30.3): before any phase-2 piece leans on the numbers layer, every condition of G43's done-when holds, and a kernel owner records the result here with the CI runs that show it (30.1). This is the first gate, so its record also sets the form: one row per condition of the done-when, in its words, with the evidence and the CI run; then every merge gate with the violation that is seeded to prove it can fail; then the readings the gate took.

**Result: not yet passed (drafted 2026-10-29).** GitHub Actions has started no job since the organisation moved to GitHub Enterprise on 2026-10-28, so no CI run below exists yet. Every check that can run on the owner's workstation was run there on G42's branch with this piece's changes: the verification suite and the thread-count tests, Verus (113 verified, 0 errors, with the release CI pins), and each planted violation of the table below (rustfmt, Verus, Kani three ways and libFuzzer each caught theirs). The gate passes when G39, G40 and G42 have merged, every row below names a green CI run on the gate's commit, and #368's four gates have their seeded runs. `geom-test`'s `tests/attack_g43.rs` holds this record to the board, the workflows and the tests: a condition missing, a cited test or job gone, or a pull-request job with no row fails it, and so does a result of "passed" while a row is open.

| Condition | Evidence | CI run |
| --- | --- | --- |
| Every predicate in geom-num passes 28.6's extreme-value tests and the rational oracle on every pull request | `geom-num/tests/verification.rs::every_predicate_is_verified_in_every_class`: the twelve public predicates by five classes, each cell a test that now must reach its predicate in its code (it counted the matrix's own strings before; tightening it found three gaps, closed here: the perturbed orient3d and insphere at their sign corners, insphere's ties against an oracle, `geom-num/tests/verification.rs::perturbed_ties_match_a_symbolic_oracle`, and the three-plane point one step from degenerate in `geom-num/tests/attack_g29.rs::one_step_from_degenerate_is_decided_exactly`). `kernel.yml` runs them on every pull request that touches the kernel, on six runners in debug and release and on wasm32 under wasmtime and Node | pending |
| Wide integers and orientation predicates are proven in Verus | `kernel-dev.yml` job `verus` verifies `geom-num/verus/lib.rs`: the wide integers' twin (G37) and the twins of orient2d, orient3d, incircle and insphere (G38), held to the shipped code token for token by `geom-gen/tests/verus_twin.rs::any_change_to_either_side_is_seen`; G39 adds the plane-based and implicit-point predicates | pending |
| The filters of orient2d, orient3d, incircle and insphere are proven by at least one tool, and every other filter is labelled 'checked, not proven' with an open issue | `kernel-certificates.yml`: Gappa's proofs, checked by Rocq, of nine of the eleven generated filters, these four among them (G35, G40); tpi_side and lpi_side say "checked, not proven" in their module headers and in `MAP.json`, naming #292, which is open | pending, on G40's merge |
| The determinism matrix is green on all seven targets, both build modes and 1, 2, 3 and 8 threads | `kernel.yml` jobs `test` (x86-64 and arm64 Linux, Windows and macOS, debug and release, and release tuned to the runner's CPU) and `wasm` (wasmtime and Node, three modes), each running every golden of the store (`geom-test`'s `golden run`); `geom-test/tests/attack_g10.rs::the_matrix_has_every_runner_in_both_modes` holds the legs. The thread counts are one constant, `geom_test::golden::THREAD_COUNTS`, which every same-bits-on-any-thread-count test now runs (`geom-run/tests/golden.rs::golden_hash_is_the_same_on_every_target_thread_count_and_chunk_size`, `geom-pool/tests/attack_g33.rs::attack_rayon_pools_give_the_serial_answer_at_every_thread_count_and_chunk_size`, `geom-math/tests/determinism.rs::golden_hash_is_the_same_on_every_thread_count`, `geom-num/tests/attack_g05.rs::identical_bits_on_every_thread_count`) | pending |
| Every merge gate fails on a seeded violation | The table below: every job of every workflow that runs on a pull request, with the test or planted violation that makes it fail | pending; #368 open |
| No unexplained mutant survives in geom-num | `kernel-quality.yml`'s full run (`workflow_dispatch` on the gate's commit): every mutant of geom-num (5,147 on G42's branch), judged against the ledger, which keeps 22 survivors, 21 equivalent and 1 `hangs`, each with its reason; no full run has been made yet, only each piece's mutants in its diff | pending |
| Budgets give identical results serially and in parallel | `geom-run/tests/attack_g23.rs::attack_serial_and_parallel_phases_agree_exactly` and `geom-run/tests/attack_g23.rs::attack_serial_and_parallel_batches_agree_exactly` (answers, exhaustion errors and the meters' use, serially and on 2, 3 and 8 threads and a backwards pool, at chunk sizes 1 to 4,096), `geom-run/tests/determinism.rs::random_cases_match_the_phase_model` and the golden above, which hashes every outcome | pending |

Every merge gate and the violation seeded to make it fail. R: the real tool runs on a planted violation; J: the gate's judge runs on seeded input or seeded tool output; `trip <name>`: the job itself first runs its tool on the planted violation of that name (`geom-test`'s `trip`, gate 1).

| Workflow | Job | Seeded violation |
| --- | --- | --- |
| `kernel.yml` | `test` | J: `geom-test/tests/attack_g19.rs::a_run_that_does_not_prove_its_goldens_is_refused`; R: the job `guard` |
| `kernel.yml` | `guard` | R: the job itself (`libm`'s `sin` changed by one bit must fail a golden); J: `geom-test/tests/attack_g20.rs::a_trip_counts_only_for_the_right_reason` |
| `kernel.yml` | `wasm` | J: `geom-test/tests/attack_g19.rs::a_run_that_does_not_prove_its_goldens_is_refused`; open: #368 |
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
| `kernel-bench.yml` | `scope` | Not a gate: it decides which count jobs run, from the same paths as the workflow's trigger |
| `kernel-bench.yml` | `checks` | The tools of `kernel.yml`'s `lint`: `trip fmt`; R: `engine-geometry/tests/scaffold/gate.rs::gate_clippy_fails_on_each_seeded_violation` |
| `kernel-bench.yml` | `instructions` | J: `geom-bench/tests/attack_g18a.rs::a_rise_over_5_percent_on_a_count_of_200_fails`, `geom-bench/tests/attack_g18a.rs::the_command_line_passes_fails_and_refuses_as_the_rule_says`; open: #368 |
| `kernel-bench.yml` | `fuel` | J: `geom-bench/src/counts.rs::growth_passes_the_stated_order_and_fails_the_next`, `geom-bench/src/counts.rs::compare_finds_rises_listings_removals_and_additions`; open: #368 |
| `kernel-fuzz.yml` | `libfuzzer` | `trip libfuzzer` |
| `kernel-fuzz.yml` | `kani` | `trip kani-fuzz` |
| `kernel-fuzz.yml` | `kani-num` | `trip kani-num` |
| `kernel-fuzz.yml` | `kani-limbs` | `trip kani-limbs` |
| `kernel-fuzz.yml` | `clusterfuzzlite` | Open: #368 |
| `kernel-quality.yml` | `mutants-plan` | J: `geom-test/tests/quality_gates.rs::shard_plans_fit_the_budget_and_a_matrix` |
| `kernel-quality.yml` | `mutants` | R: `geom-test/tests/seeded_gates.rs::a_seeded_survivor_fails_the_mutation_gate_until_a_test_catches_it`; J: `geom-test/tests/kernel_quality_workflow.rs::every_gate_is_judged_and_tool_failures_are_not_swallowed` |
| `kernel-quality.yml` | `mutants-judge` | R: `geom-test/tests/seeded_gates.rs::a_pull_request_that_adds_a_survivor_fails_the_diff_gate`; J: `geom-test/tests/quality_gates.rs::stale_entries_fail` |
| `kernel-quality.yml` | `coverage` | R: `geom-test/tests/seeded_gates.rs::seeded_uncovered_code_fails_the_coverage_gate_until_tests_reach_it` |
| `kernel-quality.yml` | `gates-seeded` | R: the job itself, which runs `geom-test/tests/seeded_gates.rs` with the real tools |
| `kernel-quality.yml` | `miri` | Not a gate: Miri runs weekly (28.17), and its pull-request leg only checks its own set-up when that changes |
| `kernel-certificates.yml` | `gappa` | Open: #368, after G40 |
| `kernel-certificates.yml` | `fptaylor` | Open: #368, after G40 |
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

**Changes:** this record; `geom-num`'s verification suite and `tests/attack_g29.rs`; the thread-count tests of `geom-run`, `geom-pool`, `geom-math` and `geom-num`; `geom-test`'s `rust_source::references`, `golden::THREAD_COUNTS`, `trip` and `tests/attack_g43.rs`; the `trip` steps of `kernel.yml`, `kernel-dev.yml` and `kernel-fuzz.yml`; issues #367 and #368.
