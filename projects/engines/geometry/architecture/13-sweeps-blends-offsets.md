# 13. Sweeps, lofts, blends, offsets and shells

*Part of the [Spare Kit geometry kernel architecture](../ARCHITECTURE.md). Decided in [G01](../../../kit/notes/G01-geometry-kernel.md); section numbers match across the set.*

Evidence: [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) sections 2, 7 and 8; [04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md) sections 1 and 3; [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md) section 4; [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md) parts A1, A3 and B; [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md) section 4; [10](../../../kit/notes/G01-evidence/10-github-sweep.md) sections 1 and 3; [11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), parts "nurbs-core" and "operations-io". Parasolid's public Functional Description was read by [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md) on a third-party copy whose authorisation is unknown. Section 29.5 allows it only from Siemens-hosted pages, so this chapter links no copy. Each concept cites the published literature where there is some; a concept that only Parasolid documents is marked "pending owner check (rule 11)" until it is re-checked on Siemens's own pages (29.20, rule 11). Papers, the IFC 4.3 source at [7256780](https://github.com/buildingSMART/IFC4.3.x-development/tree/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d) and other pinned code are linked where they are used.

This chapter covers every operation that makes a solid from profiles or edits a solid locally: extrude, revolve, sweeps along paths and rails, helical sweeps, lofts, network and developable surfaces, the IFC sweeps, fillets, chamfers and blends, offsets, shells, thickening, draft, face edits, straight-skeleton roofs, sheet metal and the swept volumes of moving tools. The operations live in `geom-sweep`, `geom-blend`, `geom-offset` and `geom-skeleton`, with the domain toolkits `geom-stair`, `geom-thread` and `geom-sheet` on top (section 27).

Pieces that several of these crates need live in the core, so no operation depends on another's internals (section 5):

| Shared piece | Crate |
| --- | --- |
| Frame rules, laws and station frames (stored inside the Sweep kind) | `geom-surface` |
| Surface extension (13.9) | `geom-surface` |
| Skinning, Gordon combination, Coons patches, N-sided fills | `geom-nurbs` and `geom-surface` |
| Plane commit (13.2) | `geom-poly`, with its predicates in `geom-num` |
| The arrangement step of a local operation and the classification of topology changes | `geom-bool` |
| Records of topology changes in the history | `geom-name` |
| Isoclines and other level curves | `geom-intersect` (section 12) |

Open question 3 lists the dependency changes this needs in section 27.

Building elements are Spare Construct's compositions: the kernel knows nothing about walls (section 5). So that every discipline can see what it stands on, this table lists each building element family with the kernel operations it uses and who owns them:

| Element family | Kernel operations it uses | Owner |
| --- | --- | --- |
| Walls and their joins (L, T, X, mitre; layer clean-up) | Extruded layer profiles (13.4); plan booleans for layer joins (section 14); 3D booleans and shared faces (11.5); attach top and base by "up to" ends and face replacement (13.4, 13.13) | Spare Construct, on `geom-sweep`, `geom-2d`, `geom-bool` and `geom-offset` |
| Curtain walls | Surface division and panels (16.4); mullion and transom profiles swept along the grid lines (13.4); booleans (section 11) | Spare Construct, on `geom-panel` (16.6), `geom-sweep` and `geom-bool` |
| Structural framing: beams, columns, braces, copes, cutbacks and connections | Member profiles extruded or swept along the member line (13.4); copes and cutbacks by half-spaces and booleans (11.1) | Spare Construct, on `geom-sweep` and `geom-bool` |
| MEP runs, fittings and transitions | Swept disks and swept profiles, with torus elbows (13.4); lofts and developable transitions (13.6); tees and take-offs by booleans (section 11); ductwork flat patterns (13.16) | Spare Construct, on `geom-sweep`, `geom-bool` and `geom-sheet` |
| Rebar | Swept disks with fillet-radius bends, one batch call per schedule (13.4, 13.7) | Spare Construct, on `geom-sweep` |
| Stairs and railings | Extrusions, stringer sweeps, helical stairs and rail path blends (13.8) | `geom-stair`; code checks in Spare Construct |
| Roofs | Straight skeletons (13.15), thickened into layers (13.11); dormers and openings by booleans (section 11) | Spare Construct, on `geom-skeleton`, `geom-offset` and `geom-bool` |
| Floors, slabs and foundations | Extruded sketched regions (13.4, section 15); booleans with the toposolid for excavation (17.6) | Spare Construct, on `geom-sweep` and `geom-bool` |
| Toposolids and site | Terrain solids, strata and their booleans (17.6) | `geom-terrain` |

## 13.1 One way to build them all

Every operation here runs the same five steps:

1. **Plan.** Check everything that can be checked before building: profiles closed, flat and not crossing themselves; frames defined along the path; sections compatible; radii against curvature; which faces will vanish or clash. A failed plan returns an error saying where and why (13.20), and nothing is built.
2. **Carriers.** Make the new surfaces and curves, each in its exact kind where one exists (section 7): a fillet between two planes is a cylinder, the offset of a sphere is a sphere.
3. **Arrangement.** Hand the new faces, extended past their final size (13.9), and the faces they meet to the arrangement engine (section 11), which intersects them (section 12) and builds the loops. Intersections known by construction, such as a fillet's contact curves or an extrusion's side edges, are passed in as known, never searched for again.
4. **Select** the cells the operation asks for: the material side of a fillet, the region swept by a profile.
5. **Name and check**: emit names and history (13.19, section 20), run the validity checker, return.

**A local operation is a geometry step followed by one arrangement step.** There is no local patching and no second boolean engine. OpenCascade's fillets rebuild topology with a legacy boolean of their own, and its offset's global self-intersection step is commented out; SMLib stitches fillet results afterwards "for some reason" and ignores the return value ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 7; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io"). On the one engine, every topology change below is decided by the same exact predicates and certified intersections as a boolean.

**Fast path.** When the plan proves the result valid by construction (an extrusion of a valid region between distance ends, a revolve of a valid profile clear of the axis, a sweep that passes both self-overlap tests in 13.3 and touches nothing else), step 3 builds the faces directly instead of arranging them. The checker still runs, and the diagnostics record the skip. This matters when 230,000 elements are built in one batch.

**Rules every operation here keeps:**

- Inputs are never modified, and no tolerance is ever raised. On a Tolerant body (section 6.5) an operation works within the existing tolerances; a construction that would need a larger one fails with `ToleranceExceeded`, remedy "heal first".
- No retries with nudged operands, jitter or growing fuzz. That is how SolidCraft and IfcOpenShell rescue failing results; IfcOpenShell's OpenCascade path retries with the fuzz multiplied by ten, up to 10^4 times the precision ([11](../../../kit/notes/G01-evidence/11-vetting-suggested-projects.md), SolidCraft; [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), A3).
- Marching and tracing loops are bounded, check the budget and the cancellation token at each step, and use fixed step rules: no clock, no seed, no function-local state. SMLib's fillet solver fixes a step size on its first call, and its shell gives different results in different processes ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io"); section 21 forbids both.
- Batch first (section 10.1): 3,000 rebar bars, 40 fillet edges or 200 sheet-metal unfolds in one call, results in input order, one result per independent item.
- Each local operation declares the topology changes it may make and reports those it made (13.14).

## 13.2 Exactness, committed planes and error bounds

**The class follows what the result is made of** (section 9):

| Result made of | Class | Examples |
| --- | --- | --- |
| Planes built exactly from grid data | Exact | Grid profiles extruded by integer vectors; tapers, chamfers and roof slopes given as rise over run on axis-aligned edges; gables; mitres whose planes have integer normals; offsets of planes whose normal has a whole-number length |
| Committed planes (below) | Certified, with the proven deviation | Drafts and slopes given as angles; profiles turned by a frame; faces moved along sloping normals |
| Analytic or procedural curved faces | Certified | Revolves, fillets, pipes, offsets of cylinders, threads |
| Fitted NURBS faces | Certified | Lofts, networks, developable lofts, Class-A corner patches |
| Meshes or fields | Certified where interval bounds prove it, otherwise Approximate, only if the caller accepts it | General swept volumes (13.17), the mesh and field routes (13.18) |

A profile placed by a frame that is not a grid rotation is rounded to the grid first, as section 7 says for rotations, and the result is Certified.

**The caller accepts the class.** Section 9 and principle 3 say a result is never made less exact without being asked, so this chapter uses 14.3's rule. Each schema states the classes its operation can return (the table above), and each input states the least exact result the caller takes, with 14.3's `Accept`. A result below it returns `ExactnessNotAccepted`, with the bound it would reach and what would keep it Exact: a slope given as rise over run, an axis-aligned hinge, a turn by a multiple of 90°. So a 1.5° draft on an Exact body is built only when the caller accepts Certified, and interactive commands pass that acceptance explicitly, on record in the operation log, as 14.3's drafting commands do. Moving `Accept` into the shared contract of 10.1, so that every chapter uses this one rule, is proposed for section 32.

**Committed planes.** Some flat constructions need planes whose ideal coefficients are irrational: a 1.5° draft, a 30° roof, a profile turned by a rotating frame, a face moved 10 mm along a sloping normal. Section 6 commits points only, so this chapter commits such a plane once, to integer coefficients within the plane budget of section 6.2 (normal 2B+3 bits, offset 3B+5 bits, [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4):

- **A plane that must contain an exact edge** e (a draft hinge, an eave) takes the normal N = e × t, where t is the integer vector nearest the ideal tilt that keeps N within budget. Its offset is N·p for a vertex p of the edge, exact, so the edge lies on the plane exactly. The tilt resolution is about |e|·2^-(2B+3) radians, so over the face's own extent D the plane misses the ideal by at most about |e|·D·2^-(2B+3) steps, plus the rounding of t. The commit routine proves the bound for each face. For a 100 mm hinge on a 100 mm face it is about 2^-44 of a step; across a whole ±2^40-step frame it can reach about 1.5 steps, which is why it is computed per face and never assumed.
- **A free plane**, moved by a distance δ along its normal, takes the integer normal nearest the ideal direction and the offset round(N·p + δ|N|), with |N| bracketed by an exact integer square root. Its error is the direction error over the face's extent plus at most 1/(2|N|) of a step.
- **After the commit nothing rounds again.** Vertices stay implicit (section 6.3) and later booleans reuse the planes (section 11).
- **What is certified** is the Hausdorff distance between the committed faces and the ideal faces, computed from the plane errors and the angles at each vertex. Where two faces meet at a sharp wedge the vertex slides by about the plane error divided by the sine of the angle, and that counts; where faces are nearly coplanar the slide stays within the surface and does not.
- **Topology check.** A plane has no implicit form to fall back to (section 6.3), so every predicate that decides the result is evaluated twice: exactly on the committed planes, and with interval bounds on the ideal ones. If the ideal sign is certain and the committed sign differs, the commit has changed the shape, and the operation fails with `ExactnessNotAccepted` (`CommitFlip`, section 22). If the ideal sign cannot be certified because the ideal is a designed tie (four roof planes meeting at one apex, slopes given as angles), the exact committed sign stands, any edge it makes shorter than the short-edge warning is reported (section 6.5), and the result stays within its bound.
- The result is Certified with the proven deviation, just as a committed point makes a result Certified elsewhere in the set (11.3, 11.4, 17.3). Open question 1 asks whether sections 6 and 9 should let both committed points and committed planes within q/2 count as Exact. If owners adopt that, the class column above changes, and so do 11.3, 11.4, 17.3, 17.6 and 19.3.

**Error bounds through a history.** Section 9 says bounds add up, but not yet against what. One rule must serve every chapter, settled by a decision note on section 9 (this chapter's open question 4, and section 11's open question 1). Until then this chapter uses the same interim rule as 11.4:

- A result's bound is the sum of its operands' bounds and the operation's own term. If the sum would pass ε_fit, the operation fails with `ExactnessNotAccepted` (`Bound`, section 22), naming the earlier steps that used the budget, and offers Certified with the larger bound only if the caller accepts it.
- The operation's own term is measured against the exact operation applied to its stored inputs. Procedural faces (sweeps, rolling-ball blends, offsets, intersection curves) are stored as their definitions, so building on them adds no error; their cached NURBS is not the shape. Error enters only where something is committed or fitted: planes, vertices, the control points of fitted surfaces, a rational curve approximated for a network.
- **ε_step is proposed, not adopted.** This chapter proposes a ladder rung for each operation's own commits and fits: the operation step bound, ε_step = 16q, which is ε_fit/4 (0.0025 mm at q0, 0.1 µm at q1). It is not on section 6.5's ladder, and section 27 takes every tolerance from that ladder, so it waits for an owner's decision (section 32, open question 4). Until then every use of ε_step in this chapter reads ε_fit, within the summed bound above.
- **The proposed one rule** (open question 4): keep bounds per face and per edge, and add them only along each entity's own history. An edge filleted after a loft would carry both bounds; a face the fillet never touched would keep its own; the body's ε would be the largest. Section 11's open question 1, certifying every entity directly against its carriers, points the same way. If owners adopt it, 11.4 and this section change together.

## 13.3 Profiles, paths and frames

A **profile** is a region (an outer loop and any holes) from the 2D layer (section 14) or a sketch (section 15), placed in 3D by a frame. An open profile makes a sheet. The plan checks that it is flat, closed where it must be and does not cross itself (`InvalidProfile`, naming the crossing), and that no edge, angle or radius is zero (`DegenerateInput`). A **path** (IFC: directrix) is an oriented wire of curves of any kind. A **frame rule** says how the profile turns as it travels:

```rust
// In geom-surface, because the Sweep kind stores it (section 7).
enum FrameRule {
    Translate,                                    // the profile never turns
    RotationMinimising(Stations),                 // least twist; double reflection; station frames stored
    Frenet,                                       // follows curvature; undefined where curvature is zero
    FixedReference { reference: GridVec3, axis: ProfileAxis }, // IFC fixed reference (x); up-vector (y)
    SurfaceNormal(SurfaceRef),                    // IFC surface-curve sweeps
    Placements(Vec<StationPlacement>),            // IFC linear placements, supplied by geom-align
    Guide(CurveRef),                              // the profile's x-axis points at a guide curve (13.5)
    TwoRail { rails: [CurveRef; 2], anchors: [GridPoint2; 2], height: Option<Law> }, // 13.5
    Helical { axis: Axis, pitch: Law, radius: Law }, // a rigid screw motion when both laws are constant
}
struct Laws { twist: Option<Law>, scale: Option<Law> } // functions of arc length; smooth paths only
```

Twist and scale laws act on top of any rule. As in Parasolid's sweep, they need a tangent-continuous path (pending owner check, rule 11).

**Rotation-minimising frames** turn the profile as little as possible. They use the double-reflection method of Wang, Jüttler, Zheng and Liu ([ACM TOG 27(1), 2008, doi:10.1145/1330511.1330513](https://doi.org/10.1145/1330511.1330513)), which tinyspline (MIT) implements ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 3). Each step is two reflections using arithmetic and one square root, so it is bit-identical everywhere, with fourth-order global error against the mathematical frame. So that the Sweep kind is well defined, the frame is defined by its stored stations:

- Stations s_0 < … < s_n carry frames F_0 … F_n, stored in the surface (bit-exact `f64` rotations; section 24 owns the encoding, open question 5). F_{i+1} is one double-reflection step from F_i.
- **Between stations**, the frame at s is the double-reflection step from F_i to s, turned about the tangent by a cubic correction that is zero at both stations and whose end slopes make the frame's spin about the tangent exactly zero there. The rest of a frame's turning is fixed by the bending of the path, so the frame meets its stations exactly and is smooth (C1) along the whole path. This interpolation rule is ours, built on Wang et al.'s step.
- The stored definition, not the continuous mathematical frame, is the shape, and the surface is certified against it. Stations are placed by a deterministic rule until the measured twist difference from the continuous frame, times the profile's radius, is below ε_step (proposed; ε_fit until adopted, 13.2). That difference is measured, not proven, and is reported as measured (open question 6). Because frames are stored, a file reopens to the same surface whatever later improvements the rule gets (section 26.2).
- On a closed path the closing angle is spread at a constant twist rate and reported, or the sweep fails if the caller asked for no twist.

**Frenet frames** flip at inflections and do not exist on straight pieces; the plan finds every zero of curvature with certified bounds and fails with `FrameUndefined` at that station. **Fixed-reference frames** fail where the tangent turns parallel to the reference. IFC requires a tangent-continuous directrix for them ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), A1), and the kernel enforces it.

**Corners** in the path are mitred on the bisecting plane, as IFC's swept disks (a half-angle mitre) and Parasolid's sweep do.

- A mitre joins cleanly only when the outgoing frame is the mirror image of the incoming one across the bisecting plane. That holds for translation and rotation-minimising frames, where the corner is a turn about the binormal, and for Frenet frames at flat corners. It fails for up-vector and fixed-reference frames where a 3D corner both turns and changes slope, the handrail at a landing; there the two sections would leave a gap or an overlap. The plan tests the mirror condition, exactly on grid data and otherwise with certified bounds within ε_ang, and fails with `MitreMismatch`, offering a rotation-minimising frame, a G1 path blend (a gooseneck), or a short ruled transition between the two sections.
- A very sharp corner makes a long mitre spike; IFC's informal proposition calls it a "nearly impossible miter". The caller sets a mitre limit (mitre length over profile extent, with a documented default in the schema); past it the plan fails with `MitreTooSharp`, offering a path blend or a split.
- A flat profile swept with the `Translate` rule along a polyline is plane-based. It is Exact when the mitre planes have integer normals (both path directions of whole-number length), otherwise committed (13.2).

**Self-overlap is checked first**, in two tests:

- **Local folding.** A sweep folds where a profile point lies further toward the centre of curvature than the radius of curvature: 1 − κ(s)·⟨p, N(s)⟩ ≤ 0, for profile point p, curvature κ and principal normal N. The plan proves the left side positive from interval bounds on curvature and the profile's extent in the direction of N at each station. A profile lying on the outside of a tight bend passes, which a plain extent-against-radius test would wrongly reject.
- **Global overlap**, where the path comes back near itself, uses section 12's self-intersection test on the swept faces.

An overlapping sweep fails with `SelfIntersection` at its station, offering a smaller profile, a split path, or the swept-volume operation (13.17), which resolves overlap by design. **Touching is not overlapping.** Closed-coil springs and rope strands touch themselves along curves. Where certified intersection finds contact without crossing, the caller chooses: a self-union, in which the arrangement engine fuses the touching parts (the result may be a General body, section 8), or a `SelfContact` error giving the clearance needed.

## 13.4 Extrude, revolve, sweep and helical sweep

**Extrude.** Each profile edge makes one side face of the kind its curve dictates. A line gives a plane. An arc gives a cylinder, or a linear-extrusion surface (an elliptic cylinder) when the vector is oblique. A NURBS curve gives a linear-extrusion surface. A grid polygon extruded by an integer vector is plane-based and Exact, because each side plane comes from an exact edge and the exact vector. That covers most of a building.

- **Ends** can be a distance, symmetric, or up to a face or body (Revit's attach top and base). For "up to", the target's surface is extended by default (13.9) and the extrusion is trimmed by the arrangement engine, exactly when the target is flat. If even the extended target does not cut the whole profile, the operation fails with `UpToTargetMissed`, naming the profile edges left uncovered. An option asks for the target face itself, within its boundary.
- **Taper**: each side plane contains its profile edge and leans by the draft angle. It is Exact for a slope given as rise over run on an axis-aligned edge, otherwise committed (13.2); arcs become cones. When the taper is steep enough that edges shrink to nothing, the result is a weighted straight skeleton (13.15), Kelly and Wonka's procedural extrusion ([ACM TOG 30(2), 2011, doi:10.1145/1944846.1944854](https://doi.org/10.1145/1944846.1944854)). Curved profiles tapered past that point need a curved skeleton, which is not yet solved (13.22); until then the operation fails with `CurvedSkeletonUnsupported`.
- IFC's tapered extrusion is a different thing, a ruled loft between a start and an end profile (13.7).

**Revolve.** Each new face's kind is decided by exact predicates on the profile edge and the axis, never recognised afterwards with a tolerance as openNURBS does ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 1). For a profile in a plane through the axis: a line parallel to the axis gives a cylinder; meeting it at an angle, a cone; perpendicular, a plane; an arc centred on the axis, a sphere; any other arc, a torus, including the lemon and apple forms. Anything else gives a surface of revolution. Parasolid also makes new surfaces analytic where it can (pending owner check, rule 11).

- A profile touching the axis makes a pole; one crossing it fails with `ProfileCrossesAxis`, naming the edge.
- A full turn makes seams. A partial turn makes two caps, which are committed unless the angle is a multiple of 90°.
- The NURBS forms of these surfaces follow The NURBS Book: surfaces of revolution, swung and translational surfaces in chapters 8 and 10, and rational arcs in chapter 7, with Piegl and Tiller's [A menagerie of rational B-spline circles (IEEE CG&A 9(5), 1989, doi:10.1109/38.35537)](https://doi.org/10.1109/38.35537). The primary paper for a quintic arc with no interior knot is still to be pinned (unverified). NLib has these constructions ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "nurbs-core"), but it is consulted for failure cases only until an owner signs off its title (29.5); a module that consulted it carries a credit line and no NVIDIA copyright line (rule 4). NLib's revolve changes its caller's profile in place; ours never does.

**Sweep along a path.** One side face per profile edge per path segment, so each face has one kind: a circle along a line is a cylinder, along an arc a torus; a line along a line is a plane. Anything else is section 7's procedural Sweep surface (profile, path, frame rule, laws, stored stations) with a cached NURBS certified within ε_fit. Parasolid's sweep likewise simplifies to analytic surfaces where possible (pending owner check, rule 11).

**Swept disks** (rebar, pipes, ducts, cables): a circle or annulus along lines and arcs gives cylinders and tori, with half-angle mitres at corners. Where a fillet radius is given, each polyline corner becomes an arc of that radius in the path, so the bend is a torus segment tangent to both straight parts: a rebar bend (13.7). An arc whose radius does not exceed the disk radius fails in the plan, as IFC requires. A rebar schedule is one batch call, and bars of one shape share geometry placed many times (section 19). IfcOpenShell's failures here (inside-out caps, skipped arc directrices, doubled end caps, elbows drawn as doughnuts; [08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), A3) are required test cases.

**Helical sweeps.** The `Helical` rule moves the profile about an axis while it rises.

- With constant pitch and radius the motion is a rigid screw motion. A straight profile edge in a plane through the axis makes a ruled helicoid, and where it meets a coaxial cylinder the edge is an exact helix (section 7), so the crests and roots of parallel threads stay exact kinds. The profile may also lie across the axis: the involute helicoid of a helical gear is the screw sweep of a circular involute, a kind section 7 already has. Gear generation itself belongs to a gear toolkit (section 26).
- Variable pitch (closed-end and progressive springs) and varying radius (conical springs, scrolls) give Certified Sweep faces. Whether a conical helix should become an exact curve kind is for section 7 (open question 5).
- **Tapered pipe threads** (NPT, and ISO 7 / BSPT) need no new law. Their published thread forms keep a constant pitch with the form's bisector perpendicular to the axis, so the flanks are the same ruled helicoids, and the taper is cut by coaxial cones in the arrangement step (unverified here against the standards; `geom-thread`'s data must confirm it). Crest and root edges are then helicoid-cone intersection curves, Certified.
- `geom-thread` takes thread-form tables as input data. A cosmetic thread is an attribute on a cylinder; a modelled thread is a helical sweep united with or cut from it, with run-outs made by the arrangement engine. Springs, coils, screw conveyors and spiral-stair stringers are the same operation, and touching coils follow the self-contact rule (13.3).

## 13.5 Rail sweeps

Sweeps steered by curves rather than laws are core tools for façades, product design and car surfacing.

- **Guide.** At each station the plane normal to the path meets the guide curve at one point, found by certified curve-plane intersection (section 12). The profile's x-axis points at that point, so the twist follows the guide.
- **Two rails.** The profile has two anchor points. At each station it is rotated and scaled uniformly so the anchors land on the corresponding points of the two rails, matched by the path-normal plane or, on request, by equal arc-length fractions. An optional height law scales the profile across the anchor line as well.
- **Method**: the swept surfaces with scaling of The NURBS Book, section 10.4. NLib's general sweep is consulted for failure cases only until an owner signs off NLib's title (29.5), with a credit line and no NVIDIA copyright line (rule 4) ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "nurbs-core"). NLib's sweep only "attempts" its tolerance; ours is stored as the procedural Sweep surface (path, rails, rule) with a certified cache, so it is Certified.
- A station plane that misses a rail or meets it more than once, or rails that meet so the scale goes to zero, fails with `RailMismatch`, naming the station.

## 13.6 Lofts, swept blends, network surfaces and developable lofts

A **loft** passes a surface through a sequence of sections.

- **Correspondence first.** Sections are matched point to point by an explicit map, by IFC tags, or by a documented automatic rule (equal arc-length fractions from the alignment of least twist). Sections that cannot be matched fail with `SectionsIncompatible`, naming them; Revit says only "Can't create form element" ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), B).
- **Ends and closure.** A loft may start or end at a point (a nose or an apex), pointed or tangent to a given plane, and may close back on its first section, as Parasolid's loft allows (pending owner check, rule 11).
- **Ruled lofts** between polygons make one face per matched edge pair: a plane when its four corners are coplanar (an exact `orient3d` test), so the loft stays Exact, otherwise a ruled surface, Certified.
- **Smooth lofts (skinning)** follow The NURBS Book, chapter 10: make the sections compatible (A5.9, A5.4), choose parameters by chord length or the centripetal rule, interpolate across them (A9.1) ([04](../../../kit/notes/G01-evidence/04-nurbs-surfaces.md), section 3). Making many curves compatible by merging nearby knots, and skinning with G1 or G2 end conditions, are taken from The NURBS Book first (chapters 5 and 10). NLib has both ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "nurbs-core"), but it is consulted for failure cases only until an owner signs off its title (29.5), with a credit line and no NVIDIA copyright line (rule 4). Merging knots changes the curves, so each merge is certified within ε_step (proposed; ε_fit until adopted, 13.2) and counted in the step's bound. Ours check every precondition, never change their inputs (NLib's Coons and N-sided fills overwrite theirs), and use our own solvers, never NLib's routines derived from Numerical Recipes.
- The plan runs section 12's self-intersection test on the skin before returning it.
- **Swept blend** (Revit's term): sections placed along a path by a frame rule, then lofted. **Guides** make a loft a network surface.

**Network (Gordon) surfaces** interpolate a grid of profiles and guides, following TiGL (Apache-2.0; [CTiglInterpolateCurveNetwork.cpp L66-L128](https://github.com/DLR-SC/tigl/blob/f05c1e301bb462df1135f9c5778771a7ebe22d2d/src/geometry/CTiglInterpolateCurveNetwork.cpp#L66-L128), [CTiglGordonSurfaceBuilder.cpp L154-L215](https://github.com/DLR-SC/tigl/blob/f05c1e301bb462df1135f9c5778771a7ebe22d2d/src/geometry/CTiglGordonSurfaceBuilder.cpp#L154-L215); Siggel et al., [doi:10.1007/s11786-019-00401-y](https://doi.org/10.1007/s11786-019-00401-y)). Intersect every profile with every guide; sort them; reparametrise each curve so all crossings sit at shared parameters; skin the profiles, skin the guides, interpolate the crossings as a tensor-product surface; raise all three to common degrees and knots; the result is profiles plus guides minus the tensor product, control point by control point. TiGL is built on OpenCascade types, so its method is re-derived, not translated. Our changes:

- Crossings come from certified curve-curve intersection (section 12), and TiGL's end-snapping constants (1e-5 and 0.001) become exact or certified tests. TiGL's reparametrisation is an approximation; ours carries a certified bound.
- Each profile must cross each guide exactly once, or exactly twice where the network is closed in that direction and the seam meets the first and last curve. Zero crossings, or more than two, fail with `NetworkInconsistent`, as TiGL throws in both cases. Networks closed in both directions are left unsupported by TiGL and fail the same way here until a method is chosen.
- Curves that miss each other by more than ε_fit fail with `NetworkInconsistent`, naming the pair and the gap. NLib silently averages them at each crossing ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "nurbs-core").
- The control-point sum holds only for non-rational curves on common knots, so rational inputs such as exact circles are first approximated within ε_step (proposed; ε_fit until adopted, 13.2), and the result says so.

Coons patches (The NURBS Book, section 10.6) and N-sided fills (Piegl and Tiller's n patches meeting G1 within an angular bound, [The Visual Computer 15(2), 1999, doi:10.1007/s003710050163](https://doi.org/10.1007/s003710050163), or the transfinite patch of 13.10; NLib consulted for failure cases only, 29.5) close loft ends and the holes left by face deletion (13.13). Fitted surfaces commit their control points to the grid and record the certified residual, so lofts and networks are Certified.

**Developable lofts** connect two curves with a surface that unrolls flat without stretching: square-to-round duct transitions, gored elbows, lofted flanges. A ruled loft is generally not developable, so sheet metal could not unfold it. Methods: Aumann's developable Bézier patches between two boundary curves ([CAGD 8(5), 1991, doi:10.1016/0167-8396(91)90014-3](https://doi.org/10.1016/0167-8396(91)90014-3)), and for polygon-to-curve transitions the classical split into flat triangles and conical fans, each fan a ruled surface whose rulings meet at a polygon corner and so developable by construction. A gored elbow is cylinder segments cut by mitre planes. Developability (the tangent plane constant along each ruling) is certified; `geom-sheet` unrolls the result exactly in kind (13.16). Two curves with no developable connection fail with `NotDevelopable`, offering a split, a ruled loft that will not unroll, or section 16's unrolling with a strain map.

## 13.7 IFC sweeps

| IFC entity | Kernel operation | Notes |
| --- | --- | --- |
| `IfcExtrudedAreaSolid` | Extrude | Exact for grid polygon profiles |
| `IfcExtrudedAreaSolidTapered` | Ruled loft between the two profiles | Planes where coplanar, ruled faces otherwise |
| `IfcRevolvedAreaSolid` | Revolve | The axis may not cross the profile's interior |
| `IfcRevolvedAreaSolidTapered` | Cylindrical ruled sweep (below) | Not implemented by IfcOpenShell ([#3540](https://github.com/IfcOpenShell/IfcOpenShell/issues/3540)) |
| `IfcSweptDiskSolid` | Swept disk | Half-angle mitres; arc radius above disk radius; the directrix may not cross itself |
| `IfcSweptDiskSolidPolygonal` | Swept disk with `FilletRadius` bends | Torus bends at every polyline corner (below) |
| `IfcFixedReferenceSweptAreaSolid` | Sweep, `FixedReference` on x | Directrix must be tangent-continuous |
| `IfcSurfaceCurveSweptAreaSolid` | Sweep, `SurfaceNormal` | |
| `IfcDirectrixDerivedReferenceSweptAreaSolid` | Sweep, `Placements` from the directrix and its cant | Cant from `IfcSegmentedReferenceCurve` (section 17) |
| `IfcSectionedSolidHorizontal`, `IfcSectionedSurface`, `IfcSectionedSpine` | Sectioned sweep | `IfcSectionedSolid` is their abstract parent, never instantiated |

- **Tapered revolve.** IFC defines each lateral face as a ruled surface between corresponding edges of the start and end sections, constructed in cylindrical coordinates ([IfcRevolvedAreaSolidTapered.md](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcRevolvedAreaSolidTapered.md)). So a pair of corresponding points moves with its radius r and height z changing linearly with the angle, from the start section to the end section. A Cartesian loft along the arc gives different geometry, so it is not used. The face is stored as the Sweep kind with a morph in cylindrical coordinates (open question 5); an oracle test checks points at many angles against the linear rule within the certified bound.
- **Fillet radius on polygonal swept disks.** `FilletRadius` rounds every transition between polyline segments ([IfcSweptDiskSolidPolygonal.md](https://github.com/buildingSMART/IFC4.3.x-development/blob/7256780bd95fcc5b3653e07b3e8ec2f8389c9f3d/docs/schemas/resource/IfcGeometricModelResource/Entities/IfcSweptDiskSolidPolygonal.md)): each corner becomes a tangent arc in the directrix and a torus segment in the solid. The plan checks the formal rule CorrectRadii (fillet radius at least the disk radius) and the informal one (fillet radius at most the start and end segments' lengths and half the shortest inner segment).
- **Every IFC rule the kernel can decide is checked in the plan**: all formal propositions, and the informal ones on arc radius, self-crossing directrices and the axis of a revolve. A failure returns `PropositionViolated`, naming the rule and the value range that would satisfy it.
- StartParam and EndParam, from the abstract `IfcDirectrixCurveSweptAreaSolid`, trim the path.
- A **sectioned sweep** follows IFC 4.3 ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), A1): points with the same tag are interpolated linearly between neighbouring sections, consecutive profiles have the same number of points, the profile plane comes from `IfcAxis2PlacementLinear` rather than the path tangent, and tangent breaks are mitred. `geom-align` evaluates the alignment, stations and placements (section 17), and `geom-sweep` builds the solid, so `geom-sweep` knows nothing about roads; a corridor is a sectioned sweep.
- The IFC item tree stays the source and the solid its evaluated geometry, IfcOpenShell's recipe pattern ([08](../../../kit/notes/G01-evidence/08-standards-coverage-testing.md), A3; section 24).

## 13.8 Stairs and railings

`geom-stair` adds no geometry kinds. Treads, risers and landings are extruded profiles (Exact). Stringers are sweeps along the stair line, and spiral stairs use the `Helical` rule with treads as a rotated pattern. Railings are profiles swept along a path offset from the nosing line with an up-vector frame; where a corner both turns and changes slope the mitre test fails (13.3), and the toolkit joins the parts with G1 or G2 path blends (goosenecks, easings) instead. Balusters are instances at arc-length stations along the rail. Riser and going rules, headroom and code checks belong to Spare Construct.

## 13.9 Surface extension

Many steps extend surfaces: the arrangement step (faces extended past their final size), "up to" ends, offsets and draft (neighbours re-intersected), and healing by growth. `geom-surface` owns extension:

- Planes, cylinders and linear extrusions extend by definition. A cone extends up to its apex, never past it. Spheres and tori are already complete in their parameter domain; poles stay poles.
- Procedural kinds extend by their own definition: a sweep by extending its path, a rolling-ball blend by extending its supports and spine, an offset by extending its base surface.
- NURBS extend by the natural continuation of the end span, Hu, Tai and Zhang's extension by unclamping ([CAD 34, 2002, doi:10.1016/S0010-4485(01)00108-7](https://doi.org/10.1016/S0010-4485(01)00108-7)), which keeps every derivative of the end span. Shetty and White's curvature-continuous extension ([CAD 23(7), 1991, doi:10.1016/0010-4485(91)90046-Y](https://doi.org/10.1016/0010-4485(91)90046-Y)) is the option when a softer continuation is wanted. Both are constructions on the control points, then committed; Certified.
- An extension that would fold back on itself or degenerate is found by section 12's self-intersection test and fails with `ExtensionFailed`, naming the face and the distance reached. Parasolid can extend "as far as possible" while keeping the surface valid (pending owner check, rule 11); Spare Kit offers that only when asked, and reports the distance reached.

## 13.10 Fillets, chamfers and blends

A **blend** replaces a sharp edge, or the gap between two faces, with a new face meeting both supports. Its specification rests on the published blending literature: Choi and Ju's rolling ball and the survey of Vida, Martin and Várady, both cited below. SMLib splits the fillet problem in a similar way (offset law, cross-section, continuity and corner strategy; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io", SmFilletSolver.h), but its fillet modules show IntegrityWare lineage, so they are STUDY under 29.6: concepts and failure cases only, never followed closely until an owner signs off their title. The specification folds continuity into the section, and adds Parasolid's overflow concepts (pending owner check, rule 11) and trimlines for Class-A work:

```rust
struct BlendSpec {
    law: OffsetLaw,        // Constant(r), Variable(law along the chain), Distances(d1, d2), DistanceAngle(d, a)
    trim: TrimLine,        // RollingBall (the ball's contacts), ConstantChord(w), HoldLines(a curve on each support)
    section: Section,      // Linear (chamfer), Circular, Conic { rho }, Curvature { cont: G2 | G3, fullness }
    corners: CornerPolicy, // setback lengths or a patch, with a continuity target
    overflow: Overflows,   // which of smooth, cliff, cliff-end and notch are allowed, in what order
}
```

**Feasibility first.** OpenCascade finds problems only while building and can return a shape with a hole after a failed corner ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 7). Spare Kit plans first:

1. **Chain** the edges into tangent-continuous chains, with tangency certified (exact for analytic pairs). An edge whose faces are nearly tangent, within the band where the ball's path is ill-conditioned (section 12 measures it), is treated as smooth only where tangency is certified. Otherwise the plan fails with `IllConditioned`, giving the band and offering to heal the edge to tangent, leave it out, or use a radius that clears the band. ACIS can treat near-tangent edges as tangent by option ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 2); Spare Kit does so only when the tangency is proven.
2. **Classify** convexity along each chain, and each end vertex's corner class. Where an edge's faces become tangent at one point and its convexity flips, the edge is split at the certified tangency point: the two parts blend on opposite sides and pinch to zero width there, since the ball touches both faces at one point. Where the faces are tangent along a stretch rather than a point, the plan fails with `MixedConvexity`. OpenCascade's offset rejects edges that are partly sharp and partly tangent outright ([02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 7).
3. **Bound the radius.** On the concave side the ball cannot exceed the supports' smallest radius of curvature across the edge; it must fit before reaching an edge it may not overflow; neighbouring setbacks must fit along each edge. Every bound is certified.
4. **Group by interaction.** Each chain's footprint is the band between its contact curves, extended by its setbacks. Chains whose footprints overlap, or that share a vertex, form one cluster, even with no shared vertex: two fillets eating one narrow face, or the opposite edges of a thin rib.
5. **Report** a certified largest feasible radius per chain. This is also a query, the blend preview, so a person or an AI can ask what fits. An infeasible request returns `RadiusTooLarge` with that radius as the remedy, the error section 22 shows.

**Building the blend:**

- **Exact analytic blends first.** At constant radius: two planes give a cylinder; a plane and a cylinder give a torus when the plane is perpendicular to the axis, a cylinder when parallel; a plane and a sphere, or a cone whose axis is perpendicular to the plane, give a torus. The case is chosen by exact tests on integer normals and axes, not by trying a cylinder, then a torus, then a cone as SMLib does ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io", Challenge 1). Most architectural blends land here.
- **Rolling ball** otherwise: the ball's centre runs along the intersection of the supports offset by r (Choi and Ju, [CAD 21(4), 1989, doi:10.1016/0010-4485(89)90046-8](https://doi.org/10.1016/0010-4485(89)90046-8); survey by Vida, Martin and Várady, [CAD 26(5), 1994, doi:10.1016/0010-4485(94)90023-X](https://doi.org/10.1016/0010-4485(94)90023-X)), traced by section 12's certified tracer, not marched from a guess. The face is section 7's procedural rolling-ball kind with a cached NURBS within ε_fit.
- **Variable radius**: a radius law along the chain, linear or a B-spline through chosen values; at each station the centre lies in the cross-section plane at distance r(s) from both supports.
- **Chamfers**: two distances, or a distance and an angle, with the extent defined by offset supports so it stays meaningful on curved faces, as Parasolid defines it (pending owner check, rule 11). Between planes a chamfer is a plane: Exact for distances along axis-aligned planes, otherwise committed. Asymmetric chamfers are first-class; SMLib's edge chamfers lack them.
- **Conic sections**, set by rho in (0, 1): elliptic below 0.5, parabolic at 0.5, hyperbolic above.
- **Trimlines** decide where the blend meets its supports. Rolling-ball contacts are the default. A **chordal fillet** keeps the distance between its two contact curves constant, a common styling choice. **Hold lines** let the designer draw the contact curve on each support, as SMLib's curve-based law does, and the section fits between them.
- **Class-A blends**: cross-sections matching the supports' curvature (G2) or also its rate of change (G3), so reflections run unbroken across the joins, by Hartmann's parametric Gn blending ([The Visual Computer 12, 1996, doi:10.1007/s003710050057](https://doi.org/10.1007/s003710050057); [17(1), 2001, doi:10.1007/PL00013398](https://doi.org/10.1007/PL00013398)) with a fullness parameter, between any of the trimlines above. G3 needs the supports' third derivatives, which every kind provides with certified bounds through its derivative trait (section 7). SMLib takes its G3 data from a finite difference that returns about minus twice the second derivative, so its G3 blends are built from wrong data (found by reading the source and checking the algebra, not by running it; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io", Challenge 1). Finite differences are never used here. The procedural blend is exactly G2 or G3 along its contact curves; its cached NURBS reports certified deviations in position, normal angle and curvature; each new edge records the continuity it was built with (section 18); the continuity query (section 10.2) measures the result.
- **Bridge and face-face blends.** A bridge joins two free surface edges with G1, G2 or G3 to each, by the same Gn method on the edges' cross-boundary derivatives, fitted and Certified. A face-face blend joins faces that share no edge, such as a boss and a wall, with the same specification. Matching one surface to another and G2 extension, which Class-A work also needs, belong to section 16.

**Overflows.** A blend overflows when a contact curve would leave its support face. Parasolid's public description has four overflow types and distinguishes internal overflow (inside the blend's run) from external overflow (at its end) (pending owner check, rule 11; the survey of Vida, Martin and Várady discusses the topological issues of blending, but whether it covers these types is unverified). In our words: **smooth** (carry on across a smooth edge onto the next face), **cliff** (across a sharp edge, one side stays on the face and the other runs along the edge), **cliff-end** (where a cliff does not fit, the blend's end becomes a cliff along the edges it meets) and **notch** (keep the blend's surface and trim it by the faces it runs into). The plan finds every overflow, internal and external, and resolves each by the caller's ordered policy; the default follows Parasolid's published order (pending owner check, rule 11), smooth then cliff then notch, with cliff-end tried where a cliff does not fit. If no allowed type works, `OverflowNotAllowed` names the boundary edge and the type that would. SMLib's roll-over, cliff and "big radius becomes a corner" cases are a required test catalogue, rebuilt with our own inputs (29.6) ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io").

**Corners.** Where blended edges meet, a corner patch joins them.

- Setback corners follow Várady and Rockwood ([CAD 29, 1997, doi:10.1016/S0010-4485(96)00070-X](https://doi.org/10.1016/S0010-4485(96)00070-X)): each blend stops short of the vertex and an n-sided patch fills the gap, by transfinite interpolation (Várady, Rockwood and Salvi, [CAD 43(11), 2011, doi:10.1016/j.cad.2011.08.028](https://doi.org/10.1016/j.cad.2011.08.028)) or Piegl and Tiller's n-sided fill (13.6). Three equal-radius blends at a convex vertex of three planes meet in an exact sphere patch, since the ball touching all three planes is tangent to all three cylinders.
- SMLib's corner taxonomy is our required test matrix, each class with a typed failure, taken as concepts and failure cases only (29.6) and built from our own inputs. In SMLib's names: 1x1, 2x2, 3x2 of mixed convexity, 4x3, Nx1, Nx1 closed, Nx2, NxN (general, convex, concave), degenerate and open ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io", SmFilletGeom.h).
- Corner patches are certified G1; G2 where the class is proven; elsewhere `CornerUnsupported` offers a G1 corner (13.22).

**Never a hole.** A blend returns a valid body or changes nothing. In a batch, each interaction cluster succeeds or fails as one. The caller chooses all-or-nothing or "every cluster that works", each failed cluster returned with its error; either way the result passes the checker. A tangent chain gets one blend face per edge, joined by smooth edges, so a name survives when the chain grows or shrinks (13.19).

## 13.11 Offsets, shells and thickening

**Offsets of surfaces stay in kind:**

| Surface | Offset by d |
| --- | --- |
| Plane | A plane: exact when d is whole steps and the normal has a whole-number length (axis-aligned or Pythagorean), otherwise committed (13.2) |
| Cylinder, sphere, torus | The same kind with its radius (for a torus, its tube radius) r ± d; a radius reaching zero collapses the face (13.14) |
| Cone | A coaxial cone with the same half-angle α, its apex moved along the axis by d / sin α |
| Surface of revolution | Revolution of the generator's offset |
| Linear extrusion | Extrusion of the directrix's offset, when the direction is normal to the directrix plane |
| Rolling-ball blend | Rolling-ball blend of radius r ± d on the offset supports |
| NURBS and other kinds | The procedural offset kind, S + d·n, with a cached NURBS |

The cache follows Piegl and Tiller's sample, interpolate and remove-knots method ([CAD 31(2), 1999, doi:10.1016/S0010-4485(98)00066-9](https://doi.org/10.1016/S0010-4485(98)00066-9)), certified by interval bounds. NLib's offsets skip loops and cusps, and OpenCascade's self-intersection step is commented out ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "nurbs-core"; [02](../../../kit/notes/G01-evidence/02-occt-parasolid-acis.md), section 7). Spare Kit handles both:

- **Local self-intersection**, where d exceeds a principal radius of curvature on the concave side and the offset folds into cusps, is found in the plan from certified curvature bounds.
- **Global self-intersection**, where distant parts meet, is found by section 12's self-intersection test and certified intersection with neighbours, following Seong, Elber and Kim's trimming ([CAD 38(3), 2006, doi:10.1016/j.cad.2005.08.002](https://doi.org/10.1016/j.cad.2005.08.002); survey by Maekawa, [doi:10.1016/S0010-4485(99)00013-5](https://doi.org/10.1016/S0010-4485(99)00013-5)).

Both are trimmed by the arrangement step and reported as topology changes. SMLib's offset-surface third derivatives have wrong coefficients ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io"); ours are derived symbolically and tested against the exact kinds.

**Stop and ask first.** IntegrityWare claims a patented "never-fail" shelling and offsetting method (no number found), and SMLib's offset and shell modules show IntegrityWare lineage. So those modules are STUDY under 29.6, concepts and failure cases only, and nobody follows SMLib's offset methods closely without first stopping to ask an owner (29.17). The methods here come from the papers cited.

**Offsetting a body** uses one of two joins:

- **Round join**: the true offset, the boundary of the solid grown or shrunk by a ball (its Minkowski sum with the ball, or erosion by it). Outward offsets round the convex edges and vertices with cylinder, pipe and sphere faces; inward offsets round the concave ones. It is unique at every distance.
- **Intersect join** (mitre): what slabs, roof build-ups and wall layers want. It is the offset surface of the body's weighted 3D straight skeleton at time d (Barequet, Eppstein, Goodrich and Vaxman, [ESA 2008, doi:10.1007/978-3-540-87744-8_13](https://doi.org/10.1007/978-3-540-87744-8_13); [arXiv:0805.0022](https://arxiv.org/abs/0805.0022)). Each face's plane moves at its own speed; per-face distances are the weights.
  - **No events before d.** The plan finds the earliest event, a time when four moving planes meet. When it is later than d, the result is the offset planes re-intersected locally. Vertices where more than three faces meet are split first by the paper's rule (its section 4.3): cut the faces around the vertex with a plane (two planes at a saddle vertex) and take the split from the weighted 2D straight skeleton of that cross-section, which the authors show always gives a unique valid split.
  - **Events before d.** The paper's event-driven simulation: events where four planes meet, of five types (edge, hole, split, edge-split and vertex), each new vertex of degree above three re-split by the cross-section rule. The paper treats general position only. Meetings of more than four planes, common on exact architectural data, are resolved by the same cross-section rule at the meeting point; where that does not settle them, `OffsetEventUnresolved` offers the round join or a smaller distance (13.22).
  - **Exact ordering.** The time at which four moving planes meet is a ratio of two 4×4 determinants, and comparing two such times needs about 720 bits at 40-bit coordinates (open question 2), so fixed 768-bit integers. A plane's speed is its normal's length |n|; where that is not a whole number it is committed, and the result is Certified (13.2). Axis-aligned bodies stay Exact.
  - The 3D skeleton lives in `geom-skeleton`, beside the 2D one (13.15). Its worst case is superquadratic (the paper's Theorem 6), so it runs under the operation's budget.
  - **Curved bodies**: carriers are offset in kind, extended (13.9) and re-intersected locally; faces that vanish or clash are resolved by the taxonomy (13.14). This equals the skeleton definition where events do not interact. Where the plan finds interacting events on curved faces, it fails with `OffsetEventUnresolved`.
- **Per-face distances**, as in Parasolid's hollowing; distance zero leaves a face where it is.
- **Offset steps.** When only some faces are offset and the edge between an offset face and a fixed one is smooth, a side face can be added along it, Parasolid's offset step (pending owner check, rule 11); role `step(edge)`.

**Shell (hollow)**: offset the faces and keep the region between the old and new boundaries. **Pierce faces** (distance zero) open the shell: an open box is a block shelled with its top pierced. A pierce face joined smoothly to an offset face gets a side face along that edge, so the wall keeps its thickness. Parasolid documents that a blend smaller than the shell thickness is absent from the inner skin (pending owner check, rule 11); here that falls out of 13.14, because the offset face turns inside out and is removed.

**Thicken**: a sheet becomes a solid by offsetting one side or both, with rim faces at open boundaries (ruled in general, planar on flat sheets). It makes roof build-ups, curved façade panels and sheet-metal parts. A smoothly varying thickness is a field shell (section 16), chosen explicitly.

**Mesh offsets** use Topological Offsets (Zint, Chen, Zhu, Zorin and Schneider, [ACM TOG 44(4), 2025, doi:10.1145/3731157](https://doi.org/10.1145/3731157); MIT code at [wildmeshing/topological-offsets@d75b55e](https://github.com/wildmeshing/topological-offsets/tree/d75b55e4477dc95cd9e4781e0413d3a9d474d67d)), in section 19. Its build pulls VolumeRemesher ([JcDai/VolumeRemesher@8927a1e](https://github.com/JcDai/VolumeRemesher/tree/8927a1e057c643f1193ed50117bb3fb311b81b09), LGPL-3.0 by its repository's licence) and GMP (LGPL), which are method only (section 29).

## 13.12 Draft

Moulded and cast parts need faces leaning away from the pull direction by a draft angle. Two stages, after the concepts in Parasolid's public description (pending owner check, rule 11), then a check:

1. **Analysis**: split faces along isoclines, where the normal makes exactly 90° minus the draft angle with the pull direction (level curves, section 12), and mark the faces that need draft (the draft-angle query, section 10.2).
2. **Taper**: replace each such surface with an **isocline surface**, one whose steepest slope makes the draft angle with the pull everywhere, through a fixed reference: an edge (the hinge), a neighbouring face (they share one draft surface and merge), or a parting surface (a step taper).
   - A plane through a straight hinge becomes a plane through that hinge: Exact for a slope given as rise over run on an axis-aligned hinge, otherwise committed.
   - A cylinder along the pull becomes a coaxial cone.
   - Any other reference curve gets the constant-slope surface through it: the envelope of cones with their apex on the curve, their axis along the pull and half-angle 90° minus the draft. It is developable and ruled, its rulings the generators of those cones, and is stored as the Sweep kind with a certified cache. A general ruled surface through the curve would meet the angle only along the curve, so it is not used. Where the reference curve is itself steeper than the draft allows, no such surface exists: `DraftUndefined` at that point.
   - Reference edges off the parting surface create step faces; faces drafted both ways from a parting line split along it.
3. **Check**: the draft-angle query certifies the angle over each whole drafted face.

The arrangement step re-intersects neighbours, extended as needed (13.9), and the taxonomy resolves clashes. SMLib has no face draft at all ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io").

## 13.13 Face edits: move, replace, delete and heal

- **Move or transform faces**: transform their carriers, extend neighbours (13.9), re-intersect. Rotations that are not multiples of 90° commit their planes.
- **Replace a face's surface**, for example a wall's top replaced by the underside of the roof it attaches to (Revit's attach top). Exact when the new surface is flat and grid-exact.
- **Delete faces and heal the wound**, the hole their loops leave. Parasolid's public description offers capping, two variants of growing, growing with shrinkage, and leaving a face with no geometry (pending owner check, rule 11). Spare Kit offers **grow** (the surrounding faces extend along their own surfaces until they close the hole; the caller may list which neighbours may grow, which covers both of Parasolid's grow variants), **grow or shrink**, and **cap** (a new face: a plane when the loop is planar, by an exact test, otherwise a Coons or N-sided fill). Grow removes a fillet, chamfer or boss. Loops heal together or independently on request. It never leaves a face without geometry.
- **Limits of growing loops together.** Parasolid documents limits (pending owner check, rule 11): at most two loops, one of them on a single surface, with restrictions where loops have three-edge vertices. Spare Kit grows loops together wherever the extended neighbours close the wound with one unambiguous cell, which the arrangement step decides. Where they do not, `HealConfigurationUnsupported` names the loops and offers healing them independently or capping.
- **Defeature**: find blends, chamfers, holes and bosses by their roles (13.19) or, on imported bodies, by exact kind and certified tangency to both neighbours, then delete and grow. SMLib's `remove_fillet` reports success and removes nothing ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io"); ours either removes the faces asked for or fails.

## 13.14 The topology-change taxonomy

Local operations can change which faces, edges and vertices exist. Parasolid documents broad classes of such change (pending owner check, rule 11); this is Spare Kit's own list. Every local operation declares the classes it may make, and the caller may narrow the set. A change outside it fails with `TopologyChangeNotAllowed`, naming the class and the entities. Every change made is reported and recorded in the history (section 20).

| Class | What happens | Example | Names |
| --- | --- | --- | --- |
| Vertex split | A vertex where four or more faces meet becomes several, joined by new edges (13.11's cross-section rule) | Offsetting a four-sided pyramid's apex | New edges named by the faces they separate |
| Face vanishes | A face shrinks to a line or point, or turns inside out, and is removed | A blend smaller than the shell thickness | Deleted |
| Self-overlap cut | Part of a face would cross itself and is cut away along new edges | Offsetting a swept face beyond its curvature | `Split` |
| Clash trimmed | Faces moved into each other are trimmed against each other; faces left inside material are removed | Tapering a cylinder outwards; offsetting a chamfer | `Split`, or deleted |
| Overshoot removed | Faces that pass right through each other are removed with the features they bound | Large inward offsets of thin features | Deleted |
| Faces merge | Neighbours that end on one carrier become one face | Draft with a reference face; healing by growth | `Merged` |
| Face added | A new face keeps the body closed | Round joins, offset and draft steps, caps, rims | `Created`, with a role |
| Loop changes | A hole in a face closes, or one face becomes several | Offsetting a plate with a small hole | Loop deleted, or `Split` |
| Body splits | One solid becomes several | A shell thicker than a narrow waist | Only when allowed |

```rust
// Recorded by geom-name; classified from the arrangement by geom-bool.
enum TopologyChange {
    VertexSplit      { vertex: NameId, edges: SortedVec<NameId> },
    FaceVanished     { face: NameId, why: Vanish },              // collapsed, or turned inside out
    SelfOverlapCut   { face: NameId, along: SortedVec<NameId> },
    ClashTrimmed     { faces: SortedSet<NameId> },
    OvershootRemoved { faces: SortedSet<NameId> },
    FacesMerged      { faces: SortedSet<NameId>, into: NameId },
    FaceAdded        { face: NameId, role: Role },
    LoopChanged      { face: NameId, change: LoopChange },
    BodySplit        { lumps: SortedVec<NameId> },
}
```

Every class is decided from the arrangement (section 11): exact on flat bodies, certified on curved ones.

## 13.15 Straight skeletons and roofs (`geom-skeleton`)

A footprint roof carries one plane up from each eave at that edge's slope. Its ridges, hips and valleys form the **straight skeleton** of the footprint (Aichholzer, Aurenhammer, Alberts and Gärtner, [J.UCS 1(12), 1995, doi:10.1007/978-3-642-80350-5_65](https://doi.org/10.1007/978-3-642-80350-5_65)), weighted by the slopes (Eppstein and Erickson, [DCG 22, 1999, doi:10.1007/PL00009479](https://doi.org/10.1007/PL00009479)). The same skeleton makes tapered extrusions past collapse (13.4) and splits offset vertices (13.11).

**Method**: the weighted skeleton of campskeleton (Apache-2.0, [twak/campskeleton@c5e88c9](https://github.com/twak/campskeleton/tree/c5e88c9c7f1d0fb600e99efce43f60d47690f6a6)), as Kelly and Wonka publish it. Each eave carries a plane rising at its slope, and the roof is built by **events**, each a meeting of three planes, in order of height. Weights cover what architects need: a gable is a vertical plane, a negative weight leans outwards (overhangs, flared walls), and height events change slope part-way up (mansard and gambrel roofs). Several planes meeting at one point are resolved together by ordering their edges into chains around the point ([CoSitedCollision.java](https://github.com/twak/campskeleton/blob/c5e88c9c7f1d0fb600e99efce43f60d47690f6a6/src/org/twak/camp/CoSitedCollision.java)). Campskeleton works in doubles and treats events within 0.00001 in height as simultaneous ([CollisionQ.java L60-L110](https://github.com/twak/campskeleton/blob/c5e88c9c7f1d0fb600e99efce43f60d47690f6a6/src/org/twak/camp/CollisionQ.java#L60-L110); [10](../../../kit/notes/G01-evidence/10-github-sweep.md), sections 1 and 3).

**Ours is exact once the planes are set.** After the roof planes are committed (13.2), nothing further rounds: every event point is an implicit three-plane point (section 6.3). CGAL's skeleton (GPL; method only) offers an exact path with square-root number types, and its [manual](https://doc.cgal.org/latest/Straight_skeleton_2/index.html) warns that offset polygons built with inexact constructions can self-intersect. Ours avoids square roots for axis-aligned eaves with rise-over-run slopes and commits the planes otherwise. Event validity uses the existing point-against-plane predicate. Ordering events by height needs one more, comparing the heights of two three-plane points: 13B+27 = 547 bits at 40-bit coordinates (a coordinate numerator needs 7B+14 bits and the denominator 6B+12, [05](../../../kit/notes/G01-evidence/05-exact-arithmetic-determinism.md), section 4), so fixed 576-bit integers, beyond section 6.2's 384-bit deepest test (open question 2).

**Weights need care.** Biedl, Held, Huber, Kaaser and Palfrader show that a weighted skeleton can be non-planar and contain cycles, and behaves like the unweighted one only under restrictions on the weights ([CGTA 48(2), 2015, doi:10.1016/j.comgeo.2014.08.006](https://doi.org/10.1016/j.comgeo.2014.08.006)). Positive weights use the method above. Negative and mixed weights are allowed, but the result must pass the checker's self-intersection group. A result that fails, or a co-sited event the chain rule cannot order, returns `SkeletonUndefined`, naming the event and offering changed weights or a roof built by booleans.

**Ties that mean something.** A square hip roof has one apex where four planes meet; that tie is real. Simultaneous events are found exactly (a zero sign) and handled together, never perturbed (section 6.2). Planes are exact when eaves run along grid axes and slopes are given as rise over run. Otherwise each class of (eave direction, slope) is committed once, and the commits are invariant under the footprint's exact grid symmetries (90° rotations and mirrors), so symmetric roofs stay symmetric and their designed ties survive. Where neither rule applies, two ideally coincident events may land a fraction of a step apart; the roof is still a valid exact solid within its certified bound, and the short-edge warning reports the tiny edge (section 6.5).

**Height events** make a new plane through the line where the old plane reaches that height. With axis-aligned eaves, rise-over-run slopes and grid heights, its coefficients are products of the slope terms and stay integer; the plan checks their size against the predicate budget and commits the plane where it would not fit (Certified).

**Curved eaves.** A sloped arc eave makes a cone. A circular footprint with one slope is one exact cone. Footprints mixing arcs and lines (bays, turrets meeting a hip roof) make events among planes and cones that no published exact or certified skeleton method covers (13.22). Until `geom-skeleton` has one, they fail with `CurvedSkeletonUnsupported`, offering a polygonal eave within a stated deviation or a roof built by booleans.

**Output**: a plane-based roof solid, or roof surfaces as a sheet to thicken into layers (13.11), with faces `roof(eave edge)` and `gable(edge)` and edges classed as ridge, hip or valley for drawings and schedules. Dormers, openings and meeting roofs are booleans (section 11).

## 13.16 Sheet metal (`geom-sheet`)

A sheet-metal part is flat flanges joined by bends, all of one thickness T.

- **Make**: a base flange is a thickened region; an edge flange is a bent section swept along an edge; a contour flange is a profile of lines and arcs swept along an edge and thickened, its arcs becoming bends. Bends are coaxial cylinder pairs of radii R and R + T; conical bends, where flanges diverge, are coaxial cone pairs. Hems, jogs and reliefs are sweeps and booleans. Lofted transitions are developable lofts (13.6).
- **Unfold**: each bend unrolls to a strip of length (R + K·T)·θ, the neutral-fibre rule, where θ is the bend angle and the K-factor or bend table is the manufacturer's input, never a kernel constant. Flanges move rigidly into the flat. Cylinders and circular cones unroll exactly in kind (a cone to a circular sector), and developable lofts unroll with certified arc lengths, both through `geom-surface` (section 16); the flat pattern is Certified. **Refold** maps the flat back, so features cut in the flat fold into place.
- **Recognise**: native parts carry roles (`flange`, `bend.inner`, `bend.outer`, `relief`); imported parts are recognised by exact kind and coaxial bend pairs, and a face that is neither fails with `NotDevelopable`. Doubly curved formed features unroll only by section 16's strain-mapped methods, when asked.
- Rigid-flex circuit boards bend the same way; `geom-board` reuses these bends rather than keeping its own (section 26).

## 13.17 Swept volumes of moving tools

A swept volume is the space a body passes through as it moves: a cutter's material removal, a door's swing, a robot's reach. The caller states the least exactness class they accept (`Accept`, 13.2), and the most exact route that applies runs. Routes 1 and 2 live in `geom-sweep`; route 3 is a space-time field in `geom-field`. This chapter proposes one owner for the whole operation, `geom-sweep`: its entry point picks the route and calls `geom-field` for route 3, on a tool the caller has already turned into an exact distance field (13.18), because `geom-convert` depends on `geom-sweep` and cannot be called from it. Section 27 should name this owner and the added dependency, and section 16.2's swept volumes should point here (open question 3).

1. **A flat-faced body moving in straight segments**: the Minkowski sum of the body with each segment, united by the arrangement engine. Its planes are the body's own, moved by integer vectors, and planes spanned by its edges and the segment, so it is plane-based and Exact.
2. **Round tools and turning bodies** (flat, ball, bull-nose and tapered cutters, and bodies turning about a fixed axis):
   - Along a straight line in any direction: the Minkowski sum of the tool and the segment, made of planes, cylinders, spheres, cones and linear extrusions of circles for flat, ball and tapered tools, all exact kinds. A bull-nose tool's outline along an oblique line is not a circle, so its extrusion is Certified.
   - Along an arc in a plane perpendicular to the tool axis: for a tool that is symmetric about its axis this motion is a rotation about the arc's axis, so the volume is the revolve of the tool's radial outline, made of planes, cylinders, cones, spheres and tori, exact.
   - Along a helix (helical ramping, thread milling): a screw motion, so the faces are helical sweeps, Certified.
   - A body turning about a fixed axis, such as a door: the revolve of its radial outline in the half-planes through that axis, plus its end positions. A flat-faced body's outline is lines and hyperbolas (edges skew to the axis), so the faces are planes, cylinders, cones and surfaces of revolution, exact kinds; curved bodies give Certified faces.
3. **Anything else** (five-axis motion, bodies that turn while they travel, deforming bodies): the Lifted Surfacing method of Ju, Zhou, Du, Carr and Ju ([ACM TOG 44, 2025, doi:10.1145/3763360](https://doi.org/10.1145/3763360), with its [erratum](https://doi.org/10.1145/3795144)). The body is an implicit function of space and time; the method lifts the sweep into space-time and extracts a watertight surface free of self-intersections.
   - **Its assumptions hold only for some inputs.** The method needs a smooth time-varying implicit function satisfying a genericity assumption ([README](https://github.com/Jurwen/Swept-Volume/blob/0bb260118443545029c98db2e98278fac27ba460/README.md)). B-rep bodies enter as exact distance fields (section 9), and sharp-edged tools such as flat end mills give non-smooth zero sets, so they are smoothed first. A motion found to break genericity fails with `GenericityViolated`.
   - **Its output is Approximate.** The method merges arrangement cells below a volume or face-count threshold and stops refining below minimum tetrahedron sizes (the README's options: 1e-5 volume, 200 faces, 2e-5 edge length). Section 6.5's ladder has no volume, face-count or edge-length rungs, so we replace them with operation parameters derived from the deviation ε the caller accepts: a minimum edge length of k·q steps with k·q ≤ ε, a merge volume of (k·q)³, and a face-count limit stated in the call. All three are recorded with the operation in the operation log, and are proposed as rungs for section 6.5 (section 32). Merging and snapping by size are still approximation steps. Certifying the result with interval bounds is new work (13.22) and is never claimed until proven.
   - **Legal: paper only.** The repository's own files are Apache-2.0, but the method's core comes from dependencies under Adobe's Research Licence, which allows non-commercial research use only: [adobe-research/mtetcol@c0e7253](https://github.com/adobe-research/mtetcol/tree/c0e7253dd59f5f20b37f0bff541b86d226b83ac5), [adobe-research/space-time-functions@b4c4a7e](https://github.com/adobe-research/space-time-functions/tree/b4c4a7e54400a2fa0389b2b738301e472c06282e), [adobe-research/arrangement-benchmark@5933c16](https://github.com/adobe-research/arrangement-benchmark/tree/5933c167f2277a10c1342e3e4c6de72bdfb94b4a) and qnzhou/mtet, fetched at its moving `main` branch ([a798adb](https://github.com/qnzhou/mtet/tree/a798adb85558584364f04474bc4fd54df66e1c26) when checked). Each LICENSE.txt was read; the README's statement that all third-party libraries are open source is wrong. The build also switches on libigl's GPL CGAL module ([libigl.cmake](https://github.com/Jurwen/Swept-Volume/blob/0bb260118443545029c98db2e98278fac27ba460/cmake/libigl.cmake)). None of these is ever read. The method is re-implemented from the paper and its erratum in our own words, and a patent check comes first (open question 7).
   - Sellán, Aigerman and Jacobson's continuation method (MIT, [doi:10.1145/3450626.3459780](https://doi.org/10.1145/3450626.3459780)) is studied as a second method.

Toolpath strategies and stock simulation belong to a manufacturing program or toolkit (section 26); vehicle swept paths are 2D swept areas over an alignment (section 17).

## 13.18 Which representations each operation accepts

Profiles and paths are curves, so extrusions, sweeps, lofts, skeletons and sheet metal take curves and return B-rep. Operations that edit a body depend on its representation (section 9):

| Operation | B-rep | Exact mesh | Subdivision | Field |
| --- | --- | --- | --- | --- |
| Fillet, chamfer, blend | This chapter | Not supported: `RepresentationUnsupported`, offering the field route | Creases and sharpness are section 16's; a true fillet converts to B-rep first (exact on regular regions), explicitly | Smooth-min blends (section 16): a different shape from a rolling ball, Certified against its own definition when meshed |
| Offset, shell, thicken | This chapter | Topological Offsets (section 19) | Offset of the limit surface as the procedural offset kind (section 16) | f − d on a true distance field; field shells for variable thickness (section 16); Certified when meshed |
| Draft, face edits | This chapter | Not supported | Not supported | Not supported |
| Swept volume | Routes 1 and 2, or route 3 through an exact distance field | Route 1 for its plane-based form | Route 3 | Route 3 |

A B-rep operation that fails never switches route by itself. Its error offers the mesh or field route as a remedy, the caller chooses, and the result's class follows section 9. [09](../../../kit/notes/G01-evidence/09-freeform-subd-implicit.md), section 4 lists an implicit smooth-min blend as a fallback flagged approximate; here it is an explicit route, never automatic. The mesh route matters for scanned meshes thickened for 3D printing.

## 13.19 Names each operation emits

Roles are part of each operation's contract (section 20.2), and a role is scoped by the operation that made it. Faces an operation only modifies keep their names. New edges and vertices are named by the faces they bound unless listed here; unchanged and one-to-one modified ones keep their names (20.2). A role that refers to an input names it by that input's persistent name, never by its position in a list, so inserting a section or a guide renames nothing else (section 20).

| Operation | Faces created | Named edges and vertices |
| --- | --- | --- |
| Extrude | `cap.start`, `cap.end`, `side(profile edge)` | |
| Revolve | `side(profile edge)`; `cap.start`, `cap.end` on partial turns | `seam(profile edge)` on full turns, `pole(profile vertex)` |
| Sweep, swept disk, rail and helical sweeps | `side(profile edge, path edge)`, `cap.start`, `cap.end`; `bend(path vertex)` for fillet-radius bends | |
| Loft, swept blend | `skin(section edge chain)`, `cap(end section)` at each open end | `section(section curve)` |
| Developable loft | `flat(input edge, input edge)` for the two input edges a flat piece spans, `fan(polygon vertex)` | |
| Network surface | `network` | `profile(curve)`, `guide(curve)` |
| Fillet, chamfer, blend | `roll(edge)` (one face per edge, also within a tangent chain), `chamfer(edge)`, `corner(vertex)`, `cliff(edge)`, `bridge(edge, edge)` | `contact(edge, face)` |
| Offset | `join(edge)`, `join(vertex)` for round joins; `step(edge)` for offset steps | Vertex-split edges, by the faces they separate |
| Shell | `inner(face)`, `rim(edge)`, `side(edge)` | |
| Thicken | `back(face)`, `rim(edge)` | |
| Draft | `step(edge)` | |
| Face delete | `cap(loop)` when capped | |
| Roof | `roof(eave edge)`, `gable(edge)` | Edges classed ridge, hip or valley |
| Sheet metal | `flange(edge)`, `bend.inner(edge)`, `bend.outer(edge)`, `relief(vertex)` | |
| Swept volume | `envelope(tool face)`, `start`, `end` | |

A fillet deletes the edge it rounds, so `roll(edge)` names the new face by that edge's name, which lives on in the history. A carrier reference to "the exterior face of the wall" survives a draft or an offset because the face keeps its name (section 20.4).

## 13.20 Failures and remedies

Every error has section 22's shape: kind, operation step, entities by name, evidence (station, parameter, point, the bound exceeded) and remedies.

| Error | Typical cause | Remedies offered |
| --- | --- | --- |
| `InvalidProfile` | Profile not flat, not closed, or crossing itself | The crossing point; close or split the profile |
| `DegenerateInput` | Zero length, angle or radius; coincident anchors | The entity and value |
| `FrameUndefined` | Frenet frame on a straight piece; fixed reference parallel to the tangent | Rotation-minimising frame; another reference |
| `MitreMismatch` | Frames not mirror images at a 3D corner | Rotation-minimising frame; a path blend; a ruled transition |
| `MitreTooSharp` | Mitre longer than the mitre limit | A path blend; split the sweep |
| `SelfIntersection` | Profile folding at a tight bend; path crossing itself; offset beyond curvature where trimming is not allowed | The largest size or distance that fits; split the path; the swept-volume operation |
| `SelfContact` | Coils or strands touching | Allow self-union; the clearance needed |
| `ProfileCrossesAxis` | Revolve profile on both sides of the axis | Trim the profile at the axis |
| `UpToTargetMissed` | Target does not cut the whole profile | The uncovered edges; extend the target; a distance end |
| `SectionsIncompatible` | Loft sections that cannot be matched | Add correspondence points; a ruled loft |
| `NetworkInconsistent` | Profile and guide miss by more than ε_fit, or cross zero or too many times | The pair, the gap or the count |
| `RailMismatch` | A station misses a rail, or the rails meet | The station; trim the rails |
| `NotDevelopable` | Unfolding or lofting where no developable surface exists | Split; a ruled loft; section 16 unrolling with a strain map |
| `PropositionViolated` | An IFC rule fails, such as CorrectRadii | The rule and the range that satisfies it |
| `ExtensionFailed` | Extending a face would fold or degenerate | The distance reached; extend as far as valid, explicitly |
| `RadiusTooLarge` | Blend radius beyond the certified feasible radius | The largest radius that fits, per chain |
| `IllConditioned` | Near-tangent faces without certified tangency | The band; heal to tangent; leave the edge out; a larger radius |
| `MixedConvexity` | Faces tangent along a stretch of the edge | Split the edge there; leave it out |
| `OverflowNotAllowed` | A blend must cross an edge by a forbidden overflow | The overflow type that would work |
| `CornerUnsupported` | A corner class or continuity the corner methods cannot make | A G1 corner; setbacks; another grouping |
| `OffsetEventUnresolved` | Mitre-offset events the cross-section rule cannot settle | The round join; a smaller distance |
| `DraftUndefined` | Reference curve steeper than the draft | Another reference; a smaller angle |
| `TopologyChangeNotAllowed` | A change outside the allowed set | Allow that class; a smaller distance or angle |
| `HealConfigurationUnsupported` | Loops that cannot be grown together | Heal loops independently; cap |
| `SkeletonUndefined` | Weights that give no valid roof | Changed weights; a roof by booleans |
| `CurvedSkeletonUnsupported` | Arc eaves meeting other eaves; curved taper past collapse | A polygonal eave within a stated deviation; booleans |
| `RepresentationUnsupported` | The body's representation has no route for this operation | The routes that exist (13.18) |
| `GenericityViolated` | A motion the swept-volume method cannot handle | Split the motion |
| `ExactnessNotAccepted` | The result would be less exact than the caller accepted, such as a committed plane on a call that accepts only Exact (13.2); a commit or fit beyond its bound; a commit that would flip a decision; a history past ε_fit (section 22) | The bound it would reach; what keeps it Exact (a rise-over-run slope, an axis-aligned edge, a multiple of 90°); a coarser preview; accept Certified, or a larger bound, explicitly |
| `ToleranceExceeded` | Tolerant entities needing growth (section 6.5, Tolerant data only) | Heal first; raise ε_heal up to its cap (section 22) |
| `FrameRangeExceeded` | The result would leave the frame's ±2^40-step range | A frame on a coarser step, which rounds the geometry and lowers its class, so the caller must ask for it |

## 13.21 What these operations never do

- Return a body with a hole, a missing face or a face without geometry.
- Change their inputs or any other body.
- Grow a tolerance, or retry with nudged inputs or wider fuzz until something succeeds.
- Ignore a cusp, loop or self-intersection in an offset or a sweep.
- Average an inconsistent network, or accept a fit that misses its bound.
- Turn an analytic face into NURBS behind the caller's back, lower a result's class unasked, or switch to another representation by themselves.
- Take derivatives by finite differences.
- Stitch or patch a result afterwards so it passes the checker.
- Guess which face a reference meant.

## 13.22 What is not solved in the open, how Spare Kit leads, and hand-offs

- **Curvature-continuous corners.** No published method makes G2 corner patches for every corner class on trimmed B-reps; SMLib guarantees only G1 ([13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io"). Spare Kit leads with the corner-class matrix as a test suite, feasibility that predicts corner failure before building, and measured continuity on every result. Meanwhile: certified G1 corners for every class, G2 where proven, otherwise a typed error offering G1.
- **Offsets of trimmed NURBS solids with guaranteed topology.** Topological Offsets does this for meshes; nobody has done it for exact B-reps. Ours are certified wherever section 12's certified intersection succeeds; where it cannot certify (near-tangent self-contact), the operation fails and offers the mesh route explicitly.
- **Mitre offsets of general polyhedra.** They are not uniquely defined in the mathematics, and the one published algorithm covers general position only (Barequet et al. 2008). Spare Kit leads with exact event ordering and the authors' cross-section rule applied to degenerate meetings too, and reports every split. Meanwhile it promises the no-event case and general-position events, and a typed error otherwise.
- **Curved skeletons.** Arc eaves and curved tapers past collapse need events among planes and cones. No exact or certified method is published. The approach is events from certified intersections (section 12), ordered by certified height intervals, with unresolvable ties as typed errors. Meanwhile: exact cones for circular footprints, `CurvedSkeletonUnsupported` otherwise.
- **Exact skeletons and offsets for arbitrary edge directions** need square roots. Committed planes with symmetry-preserving commits give valid roofs now; exact sign tests over square-root expressions are a research item for `geom-skeleton`.
- **Weighted skeletons with negative weights** can be ill-defined (Biedl et al. 2015). Spare Kit accepts them only when the result passes the checker.
- **Swept volumes of general motion with a proven distance bound.** Lifted Surfacing promises watertight output free of self-intersections, not a distance bound. Interval certification of its output is new work, tested against routes 1 and 2 wherever both apply; meanwhile route 3 is Approximate.
- **A proven bound between stored rotation-minimising frames and the mathematical frame** needs Wang et al.'s error analysis carried out with interval-bounded path derivatives. Meanwhile the difference is measured and reported.

**Hand-offs to other chapters**: wrapping and embossing profiles onto curved faces, surface matching and G2 extension go to section 16; parting lines come from section 18's silhouettes, and the parting and shut-off surfaces of a mould are ruled sweeps of those lines built here, inside a mould toolkit (section 26); mesh offsets go to section 19; circuit-board bends reuse `geom-sheet`.

**Before building.** Most families in this chapter have had no patent search. Section 29.17 lists G2, G3 and variable blends, shells, offsets and thickening, direct face edits, sheet-metal unfolding and straight-skeleton roofs as never searched, and draft (grouped with offsets in 10.2), developable lofts, transfinite n-sided patches, Topological Offsets and Lifted Surfacing have had no search either. Each enters the Build stage only after a recorded search and owner review (29.17, rule 10). This chapter proposes an owner-action build piece, "Patent search for section 13 families", on which the blend, offset and shell, draft, face-edit, skeleton and sheet-metal pieces depend (section 30). IntegrityWare's claim is a stop-and-ask (13.11).

**Tests** follow section 28. The corpus starts with SMLib's fillet, shell and defeature cases and its issue repros, rebuilt from their descriptions as our own inputs; no `TestFiles/` fixture is copied or converted (29.6; [13](../../../kit/notes/G01-evidence/13-vetting-smlib.md), "operations-io"). SMLib and NLib serve as oracles only in rule 8's deterministic layers (evaluation, knot operations, degree change, fitting without random sampling), after an owner's yes; never for sweeps, blends, shells or offsets. It continues with the IfcOpenShell sweep failures; buildingSMART samples with tapered revolves and filleted rebar; and generated fixtures for the corner matrix, the five offset event types, mansard and negative-weight roofs, railings at landings, tapered threads and duct transitions. Property tests include: offsetting by d then −d returns the original wherever no topology change was reported; every blend face is tangent to its supports within its certified angular bound; every derivative used by a blend comes from the kind's derivative trait and matches the symbolic derivative on analytic kinds; every sweep's volume matches its closed form where one exists (Pappus's theorem for revolves, a capsule for a ball swept along a line); every committed plane lies within its proven deviation over its face.
