# CON-A4-F2-P3: Wall quantities

**Status, 28 Nov 2026:** Decided and built (issue 1331). A layered wall reads back its length, its one-face area, its volume and each layer's volume: as read-only parameters on the page and as schedule fields. The code is construct-regen's `wall_measure` (the numbers) and `inserts` (the openings); the schedule fields are in construct-schedules' `QUANTITIES`; the server reads them beside a floor's and a room's.

## The question

A wall had no quantity a person or a schedule could read: a schedule of walls refused a length, an area or a volume ("Walls have no field quantity:Length") while floors, roofs and rooms each gave theirs. An architect expects a wall schedule with length, area and volume, and per-layer volumes for a material take-off, counted after the doors and windows are cut out and after the wall joints are made.

## The convention

Nothing in the repository settled which face a wall's area is of, so it is decided here.

- **Length** is the wall's reference line, from its start to its end, in grid steps. Exact for a wall along an axis; for a wall at any other angle the nearest whole grid step to the true length (an integer square root), as a floor's perimeter is.
- **Area** is one face, the reference line's: the length times the height, less each opening's width times its height. It is not the area of either finished side (those differ by the wall's thickness at the ends and at joints); it is the one number that does not depend on which side is meant or on the location line chosen.
- **Volume** is per layer, summed. A layer's volume is its plan area as built times the wall's height, less each opening's width times the layer's thickness times the opening's height. The plan area is the sum of the layer's joined pieces ([note CON-A4-F2-P2](CON-A4-F2-P2-wall-joins.md)) when the wall's joints are made, else its strip, so a joint's trimming (a mitre, a layer stopped against another wall's) is counted. A layer with no thickness (a membrane) has none. For a wall with no joints this is exactly the rule construct-openings' `wall_layer` states, the face less the openings, times the thickness; a test pins the two together.
- **Openings** are the doors, windows and plain openings hosted in the wall, as its builder places them: the opening is the width and height the builder cuts, through the whole thickness, so in a layer it is the box of its width along the wall and the layer's thickness across. They are read from the document as it stands, so deleting a door or moving a window changes the wall's area and volume at once.
- **A joint an opening reaches into** is refused, not guessed. Where the box an opening takes from a layer does not lie wholly inside one of the layer's pieces (the opening reaches the cut-back of a layer around a joint, or a mitre), the cut is not a plain box and no exact volume is given: the wall still reads its length and its face area, and its volume row has no value and says why. This is the same case the warning `insert.crosses_joined_wall` reports ([note CON-A2-F7-P1](CON-A2-F7-P1-warnings.md)).

Every sum is whole-number arithmetic through construct-params' `Length`, `Area` and `Volume`: nothing is rounded except a line's own length at an angle, and nothing is stored (the rows are worked out each time). The old box wall (`wall.straight`) is a stand-in with no layers and has no quantities.

## Where it is read

- Parameters: `wall.length`, `wall.area`, `wall.volume` and `wall.layer-<k>.volume` (k from 1 at the exterior, named with the layer's function and material), each marked `read-only`, added by the server to a wall's parameters beside a floor's.
- Schedules: `Length`, `Area` and `Volume` for the Walls category; the per-layer volumes are not fields (their number follows the type's layers), as for floors and roofs.
- A test in construct-api builds the issue's wall (8000 mm long, 3000 mm high, layers 110, 40, 0, 90 and 10 mm, a 900 by 2100 mm door and a 1200 by 1200 mm window) through the commands and reads 20,670,000 mm2 of face and 5,167,500,000 mm3 of volume, then deletes the door and the window and reads the numbers follow.

## Not decided here

A wall's area of each finished side, and its volume by material (a take-off by material adds a wall's layers of one material across walls; the per-layer rows are what it reads).
