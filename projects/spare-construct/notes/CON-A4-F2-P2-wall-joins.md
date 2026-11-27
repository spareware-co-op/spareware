# CON-A4-F2-P2: Wall joins, L and T, by layer priority (first half)

**Status, 2026-11-23:** Decided for the build (piece A60, job 5.1, the first half of CON-A4-F2-P2). The rule is construct-walls' `joins` module (pure, exact integers on construct-params' `Length`); construct-regen's `joins` module finds the walls that meet and fills each layered wall's recipe with its joined layers; the kernel builder extrudes them. The stored form is unchanged: construct-model's [FORMAT.md](../crates/construct-model/FORMAT.md) ("Walls") says what is derived. The layered wall itself is [note CON-A4-F2-P1-wall-shape](CON-A4-F2-P1-wall-shape.md); the layer functions and their priorities are [note CON-A4-F2-P1-wall-layers](CON-A4-F2-P1-wall-layers.md).

## The question

Two layered walls drawn to meet at a corner, or one drawn to stop against the side of another, overlapped: each was built with square ends, so at an L the corner square was built twice and the outside corner was left empty, and at a T the abutting wall's layers ran into the other's. Job 5.1 had to settle how the layers of two meeting walls are shaped in plan so that they meet cleanly (no overlap of the two walls' solids, no gap, no sliver), how the walls that meet are found, where the answer is kept, and what is left for job 6.1.

## The rule

Where two layers of two walls meet, the stronger priority runs through and the weaker stops against it; equal priorities meet on the joint line between them (construct-walls' `join`, written with the layers in job 3.4: structure 1, substrate 2, thermal 3, finish 4, a membrane weakest). This is how layered construction is drawn in plan in general: the load-bearing core is continuous through a corner, and the boards, insulation and finishes are cut to it and turned round the corner on their own side, as ordinary timber-frame and masonry corner and junction details show them (how the cut layers are then drawn is ISO 128-50's convention for areas on cuts and sections, which the drawing job applies). Applying the pair rule to every pair of layers at once is not enough: a layer can only reach the far side of a corner by crossing the layers between, so a weak layer on the inside of the corner can never take part of the outside. The rule is applied in order of strength instead.

**An L.** Two walls whose ends meet, at a right angle. Number each wall's layers from the outside of the corner (the side away from the other wall) to the inside. The two walls' bands cross in the corner square, and every joint is a path through that square from its outer corner to its inner corner along the walls' layer faces. The path is found strongest first:

1. Take the strongest layer of each wall (the outermost of several equally strong).
2. Equal priority: the two meet on the mitre of their own crossing, from the corner where their outer faces cross to the corner where their inner faces cross. What lies outside that crossing is solved the same way with the layers outside it, and what lies inside with the layers inside it; neither ever crosses the joint of a stronger pair.
3. One is stronger: it runs through every layer of the other wall still in play, to the other wall's outer face there, and the layers of its own wall outside it run through with it; the other wall's layers in play stop against it. What lies inside is solved the same way.

So identical types mitre every layer: each finish wraps the corner on its own side, the cores meet on the mitre. A core that is stronger than everything the other wall has runs through to the outside, and the other wall's weaker layers butt against it.

```
 Same type both walls (f finish, t insulation, S structure, i lining); A from the left, B from below:

   f f f f f f f f f f f f\       A's layers end on the mitre; B's layers end on
   t t t t t t t t t t t\ f|      the same lines from the other side, so each pair
   S S S S S S S S S S\ t f|      meets face to face and the finishes wrap the
   i i i i i i i i i\ S t f|      corner on their own side.
                    |i S t f|
                    |i S t f|

 A has structure, B only insulation between finishes (B weaker throughout):

   f f f f f f f f f f f f f|     A's outer finish and structure run to B's outer
   S S S S S S S S S S S S S|     face; B's finish and insulation stop against A's
   i i i i i i i i i\ b t b|      structure; the two inner finishes mitre.
                    |b t b|
```

**A T.** A wall (the abutting wall) whose end lies on the line of another (the through wall), away from its ends, at a right angle. Each layer of the abutting wall runs into the through wall and stops at the first layer of the through wall, counting from its near face, of equal or higher priority (a membrane stops at the near face at once); the through wall's weaker layers on that side are cut back across that layer's width to let it in, so each such layer of the through wall is built in two pieces, one either side. The through wall's far side is never cut.

```
 C runs through left to right; D comes up from below:

   c c c c c c c c c c c c c      C's far finish: never cut
   S S S S S S S S S S S S S      C's structure: D's structure stops on its face
   c c c c c|S S S|c c c c c      C's near finish, cut back where D's structure enters
          d |S S S| d            D's finishes stop at C's near finish face
          d |S S S| d
```

Every corner of every outline is the crossing of two layer faces of walls along the plan's axes, so it is a grid point with no rounding at all, and the one rounding of construct-regen's `wall_plan` (each face of a wall at an angle) is not repeated. Each layer of each wall is one or more convex quadrilaterals between its two faces; the two walls' pieces tile the joined shape exactly. Layers of no thickness (membranes) have no solid, as before.

## Finding the walls that meet

- Two walls meet when one's location line comes within the model tolerance (0.01 mm, 64 grid steps) of the other's: an end within it of an end (an L), an end within it of the other's line away from its ends (a T), or the lines crossing. Only layered walls whose layers fill meet, and only walls on the same base level whose heights overlap; lines are compared in the model's plan (each wall's line plus its placement).
- What is at a point is every wall whose line is within the tolerance of it. An L needs exactly the two walls at the point, a T exactly the abutting wall's end on the through wall's line.
- A joint also needs room: each wall's two ends (the band, along it, of every wall meeting that end) must stay apart by more than the tolerance, and a T's band must lie inside the through wall clear of its ends and of every other wall that meets it there. So every piece keeps a length, and no piece is a sliver.
- Each of these is asked of the walls within two meetings of the wall being built (the walls it meets, and the walls they meet), so every wall at a joint reaches the same answer from the same records, whichever is built first: the joints are a function of the model, not of the order the walls were drawn in.
- Deterministic: walls are read in the order of their IDs, nothing is hashed, and the rule itself does not depend on which wall is called which.
- construct-spatial is not used for the search. Its index here is the regenerator's area index, made from built shapes, which joins themselves change; a search through it would depend on what had been built already. The search reads the level's walls from the regenerator's kind index (the records as regeneration has reached, in log order) and tests their lines exactly. A spatial filter over those lines is the improvement when documents grow large.

## Where the answer lives: derived, never stored

- Nothing about a join is stored in any record. The regenerator fills a layered wall's recipe before building it, as it fills its layers from its type and its height from its levels: an argument `joins` with each layer's pieces (corners as stored lengths in the wall's own frame), the wall's whole outline, the walls it is joined with, and a sentence for each wall it meets and is not joined with. A recipe with nothing meeting it has no `joins` and builds exactly as before (the same bytes). A `joins` argument in a stored record is replaced when the recipe is filled.
- The page reads the same filled recipe from `/filled/<element>`, so it builds the same joined shape.
- The kernel builder extrudes each piece (through `Engine::extrude` only; each result is the engine's checked solid) and cuts every door opening through every piece its opening reaches. The stand-in keeps its one box, its display listing one part per layer as before.
- A wall's `reads` row lists the walls within two meetings of it (`joins`), and a wall built again builds those listed before and after again, one step deep: so drawing, moving, retyping or deleting a wall rebuilds every wall it meets or met, and a change to a wall type rebuilds its walls and, through them, their neighbours.

## Left unjoined, with the reason

Each of these is built with square ends, as before, and the reason is in the filled recipe's `joins.unjoined`:

- More than two walls at a point (a third wall at a corner, two walls abutting another at one point from both sides): job 6.1.
- Two walls whose lines cross (an X): job 6.1.
- Walls that meet at an angle other than a right angle (a mitre at any angle), or at a right angle off the plan's axes: their face crossings would not fall on the grid, and the second rounding is not taken. Job 6.1.
- Walls that meet in line, end to end or overlapping.
- A T whose abutting wall has a layer stronger than every layer of the through wall: it would cut the through wall in two.
- A joint without room: a wall too short for its two ends' joints, or a T too near the through wall's end or another wall meeting it.
- Curved walls do not exist yet; old `wall.straight` boxes are never joined.

No join is ever a build failure: a joint the rule does not cover is left unjoined.

## Not decided here (job 6.1)

X joins and joins of more than two walls at a point; mitres at any angle; wrapping of layers at free ends and round openings (construct-walls' `Layer::wraps`); a person's choice of joint at a corner (butt one way or the other, or mitre) overriding the priority rule, which would be the first join data stored in a record.
