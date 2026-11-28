# CON-A4-F5-P3: Railings by a sketched path or on a stair (first half)

**Status, 2026-11-28:** Decided for the build (piece A88, the data, shape and commands half of job 14.2). The crate `construct-railings` follows it and its [FORMAT.md](../crates/construct-railings/FORMAT.md) is the stored form; construct-regen's `railings` lays a railing out along its path or its stair, its `railing_plan` reads the filled recipe for both builders, and its `kernel` module's child `railing` builds it through the engine seam; construct-api's `cloud::railings` holds the commands and the server's view. The page's railing tool, drawing railings in plan and 3D, and their section of the two-browser test are the next lane's.

## The question

A railing is the barrier at an edge people could fall from and the rail they hold on a stair. The comparison with an architect's sample model found 131 railings holding 237 balusters and 121 handrails, on stairs and along balconies and floor edges, and no job in the plan for them. Job 14.2 asks for a railing by a sketched path or hosted on a stair: top rail, handrail, balusters by spacing, heights as data (New Zealand's clause F4). Before commands and pages could make railings, this half had to settle what a railing holds, which limits it is checked by and where they come from, how it follows a stair's runs and landings exactly, where every baluster and every corner of its parts lies on the kernel's grid along a path at any angle and on any pitch, and what a railing that breaks a limit does.

## The code: F4/AS1 and D1/AS1

New Zealand's Building Code clause F4 (Safety from falling) asks for a barrier where people could fall 1 metre or more (F4.3.1), and is met by its Acceptable Solution F4/AS1 ([third edition, amendment 2, effective 1 January 2017](https://www.building.govt.nz/assets/Uploads/building-code-compliance/f-safety-of-users/f4-safety-from-falling/asvm/f4-safety-from-falling-3rd-edition-amendment2.pdf), read in full for its paragraphs 1.1 and 1.2):

- **Table 1 (paragraph 1.1.1):** in detached dwellings and within household units, 900 mm on stairs and ramps and their intermediate landings and 1000 mm on balconies, decks and the edges of floors; in all other buildings and the common areas of multi-unit dwellings, 900 mm on stairs or ramps, 800 mm within 530 mm of fixed seating and 1100 mm at all other locations. Note 1: heights are measured upright from the floor, and on stairs from the pitch line or the nosings. Note 5: a handrail on top of a stairway barrier may rise to 1100 mm on the intermediate landings, so those landings are "all other locations".
- **Paragraph 1.2.1:** in housing and other areas children under 6 are likely to frequent (housing always is, its Comment 1), no opening anywhere over the barrier's height a 100 mm sphere passes (b), and none at the triangle a stair's riser, tread and the barrier's bottom rail form a 150 mm sphere passes (c, Figure 4).
- **Paragraph 1.2.2:** in areas only for emergency or maintenance and buildings children do not frequent, openings of at most 300 mm between upright members or 460 mm between longitudinal rails.

D1/AS1 (the stair note's source) sets a handrail's top 900 mm to 1 m above the pitch line (paragraph 6.0.6). Each figure is in `rules/nz-f4-as1.json` with its clause.

## The decisions

1. **The limits are data** beside the stair's: a rule set of categories (Table 1's rows, each with its least height on a flight and on the level and the openings rules it takes) and openings rules (a sphere, or the widest openings), and the handrail's heights. A flight of a category with no flight height is held to its level height.
2. **A limit broken is a warning, never a refusal.** A railing too low or too open is drawn and built, and its warning names the limit, the value and the clause, in construct-warnings' shape (behind a type of ours while that crate was not on this branch; construct-warnings' `RULES` takes one entry per kind). What cannot be built (a path that goes nowhere, a type whose parts do not fit) is refused.
3. **A path is straight segments.** The sketch solver holds arcs, but the engine's extrusion takes polygons, so a curved rail cannot be built exactly; arcs come with curved walls (job 14.4), as a floor's boundary refuses an arc today.
4. **A railing on a stair names the stair in its recipe**, not as a hosted element on a face: it cuts nothing and the stair is built without it. Its `reads` row lists the stair and the stair's levels, and the regenerator builds again every railing whose row lists an element with a shape that changed, so a stair changed, moved or laid out again by a moved level lays its railings out again and their records are never written. A stair deleted leaves its railings a build failure naming it. Nothing of construct-stairs changed: the railing reads the stair's runs and landings from its `shape`.
5. **The line on a stair** follows each run's side from its first nosing to its last along the pitch line, and the landing's edge between; at each run's foot the line steps up one riser (the post there reaches from the floor to the rail); at a turn on the stair's inner side it runs on to the run's end before it turns, and at a half turn round the end of the well, so every flight's slope is exactly the stair's.
6. **Balusters** are `ceil(len / spacing) - 1` on each piece between two posts, measured in plan along the path (so the clear gap a sphere would pass is the spacing less a baluster's depth), justified from the start, centred or from the end, leaving out any that would stand inside a post.
7. **Every point is worked out exactly and rounded once**, a half away from zero (main's any-angle rule, 28 Nov): a point along a piece at any angle divides by `sqrt(dx^2 + dy^2)`, so it is held as a sum of at most two square roots in whole numbers of any size (num-bigint) and rounded by exact sign tests.
8. **A rail's section is its profile, on a slope and at an angle.** Its profile is set in the axis plane square to the plan axis its piece runs nearer, so it is exactly flat, and stretched there (across by `len / dx`, up by `sqrt(run^2 + rise^2) / run`) so the section square to the rail is the profile; it is swept along the piece. Every part is one extrusion, the one operation the builders need.
9. **A round profile is twelve corners** on its circle (interim): their sines and cosines are `0`, `1/2`, `sqrt(3)/2` and `1`, so each corner stays exact before its one rounding.
10. **The opening under the balusters on a flight** is the largest disc in the corner of a tread and the riser above it under the balusters' foot line, `(r + b) (g + r - sqrt(g^2 + r^2)) / r` for the flight's tallest riser, its going and the foot's height above the pitch line; where that disc would reach above the riser, the opening is taken as its whole upright height there, `b + r`, which no disc in it exceeds.

## Why

- Warnings, not refusals: a barrier's limits depend on what the place is used for, which a person may not have settled when drawing; refusing would stop the drawing, while a warning keeps the problem in view until it is fixed, as the room and wall warnings do.
- A recipe argument for the stair, not hosting: hosting is how an element sits on a face and cuts its host; a railing follows a whole stair and cuts nothing, and the `reads` row is how a room already follows its walls.
- Measuring balusters in plan matches how the code's openings are judged (between upright members), so the gap checked is the gap there is.

## Not decided here

- Handrail extensions past a flight's ends (D1/AS1 6.0.4 for accessible stairways), rails that turn down or return, infill panels and bottom rails as parts of their own.
- A railing type stored once as a type record shared by many railings, with its own commands.
- Railings on ramps (job 14.3) and on curved paths (after job 14.4).
- Whether a barrier is needed at all (a fall of 1 m or more, F4.3.1): the railing does not know the fall at its edge.
