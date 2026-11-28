# Rooms, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record and recipe a room uses unchanged, the [levels format](../construct-levels/FORMAT.md) whose ties it carries and the [parameter values](../construct-params/FORMAT.md) whose length spelling it uses. **We may stop writing an old version. We never stop reading it.** [`src/room.rs`](src/room.rs) is the reader library and [`src/plane.rs`](src/plane.rs) the search that finds a room's loop; the decisions behind them are in [the rooms note](../../notes/CON-A4-F7-P1-rooms.md).

## A room

A room is an element record like any other, of the rooms category:

| Field | On a room record |
| --- | --- |
| `kind` | `room` |
| `name` | Its name, as people call it (`Kitchen`) |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: its point is where the room is |
| `recipe` | `{"owner":"construct","schema":1,"op":"room","op_version":1,"args":{"at":[x,y],"height":<limit>,"walls":{"room":"walls"}}}` |
| `props` | `@category` `3300`, `@room.number` its number as text (`101`, `G.04`; empty when it has none), its base's tie `@level` and `@level.offset`, and when its upper limit is a level, its top's tie `@top.level` and `@top.offset` (construct-levels) |

| Recipe argument | Value |
| --- | --- |
| `at` | The room's point, `[x, y]`, each a length in construct-params' stored spelling (`length:q0:<steps>`), in the model's plan |
| `height` | `{"tie":"top"}`: the room reaches its top tie (the level above, or any level, with an offset), and the regenerator fills the height from its base to it, exactly, as a stored length. Or a stored length: a given height above its base. `{"slope":...}` is refused (`SlopedLimit`) until the kernel can bound a room by a sloped plane |
| `walls` | `{"room":"walls"}`: the room's loop is found from the walls of its level before it is built |

```json
{"owner":"construct","schema":1,"op":"room","op_version":1,"args":{"at":["length:q0:12800000","length:q0:80000"],"height":{"tie":"top"},"walls":{"room":"walls"}}}
```

**Where it sits.** Its base stands at its level's elevation plus its offset; its volume runs from there up to its upper limit.

## Its loop

Before it is built, `walls` is found ([construct-regen's `rooms`](../construct-regen/src/rooms.rs)) from every wall whose base is on the room's level, as regeneration has reached them, each read by its outline in plan exactly as the builders lay it out: a layered wall by the four corners of its outer faces, the old box wall by its box. A wall that does not build is not there; a door or a window in a wall does not cut its outline, so an opening never breaks a loop. The search ([`find`](src/plane.rs)) is exact: the walls' outlines together are solid, the room is the open region round its point, and its boundary runs along each wall's finish face toward the room. Walls that touch at one corner only close the loop there. A wall that ends inside the room is walked round; an island wall or a column inside it is a hole.

Filled, the recipe the builders take holds only:

| Argument | Value |
| --- | --- |
| `height` | A stored length, above zero |
| `loops` | The outer loop anticlockwise seen from above, then each hole clockwise, each loop from its least corner (by x, then y) and the holes in that order; each corner `{"at":[x,y],"wall":<the ID of the wall whose face runs from it to the next corner>}`, in grid steps. Corners in one straight run are one edge |

```json
{"height":"length:q0:15360000","loops":[[{"at":[0,0],"wall":"w-s"},{"at":[19200000,0],"wall":"w-e"},{"at":[19200000,12800000],"wall":"w-n"},{"at":[0,12800000],"wall":"w-w"}],[{"at":[6400000,6400000],"wall":"c-1"},{"at":[6400000,7040000],"wall":"c-1"},{"at":[7040000,7040000],"wall":"c-1"},{"at":[7040000,6400000],"wall":"c-1"}]]}
```

## What becomes of a room

Its built row says, and the record is never changed or dropped:

| Outcome | Built row |
| --- | --- |
| Enclosed | Its shape: on the kernel's edition one solid, its loops extruded from its base up to its height, faces named `bottom`, `top` and `side.<k>` (each edge of each loop in loop order, from 0) under `construct/room`; on the stand-in's, the box around its outer loop, faces `box.<role>` |
| Not enclosed | The build failure `NotEnclosed`: the walls round its point do not close |
| Redundant | The build failure `Redundant`, naming the room whose ID comes first in the same loop; when that room goes, this one takes the loop |
| Refused | `PointInWall`, `CurvedWall`, `SlopedLimit`, `OffGrid` (a corner where two faces cross off the length grid), `AreaNotExact`, `NoHeight`, `NoLevel`, `BadRoom`, each in words |

Its `reads` row lists its levels and the walls its loop runs along. A wall that changes, is made or is deleted builds again every room on the levels it was and is on, and every room whose row lists it; a room that changes builds again the other rooms of its levels.

## Room separation lines

A room separation line is an element record of kind `room_line`, in the rooms category, with no name: a segment of no thickness on a level that bounds the space on both sides of it, as a wall's face bounds one side. It is drawn in a plan view of its level and splits what the walls leave open into rooms.

| Field | On a room separation line record |
| --- | --- |
| `kind`, `name`, `level`, `placement`, `references` | `room_line`, empty, `0`, `{"x":0,"y":0,"z":0}`, `[]` |
| `recipe` | `{"owner":"construct","schema":1,"op":"room.line","op_version":1,"args":{"from":[x,y],"to":[x,y]}}`, each end two stored lengths |
| `props` | `@category` `3300`, and its base tie `@level` and `@level.offset` (construct-levels) |

A room reads every line on its level as it reads a wall. Where a line runs inside a wall's outline or along a wall's face the wall stands and the line is dropped there; where two lines lie along each other the one whose ID comes first stands. A line with one end loose is cut out of the face's walk, and so is one that ties one loop to another (a column to a wall), so every loop a room has is simple. A line that crosses nothing or bounds nothing is kept and changes nothing. A line may run at any angle: where it meets a wall's face, or another line, off the length grid, the corner is rounded once, to the nearest grid step (a half up), and the area of a loop with such a corner is then rounded once, a half up, instead of refused; a corner between two walls only is still refused (`OffGrid`). A room's point on a line is refused (`PointOnLine`). A line that moves or goes builds again every room on its level, and a room whose loop runs along it lists it in its `reads` row. Its display is its two ends, flat (construct-regen's `flat`).

## Room volume

A room's volume is its area times its height less what the floors in its space take. Its base stands at its level's elevation plus its offset and its upper limit is the level above, or a height. A floor takes, over each point where it stands inside the room, the part of its slab between the room's base and its upper limit: `max(0, min(top, upper) - max(top - thickness, base))`, where `top` is its top face there (flat, or one plane for a sloped floor, its thickness plumb). So a floor on the room's own level, its top at the base, takes nothing; the floor of the level above, its top at the upper limit, takes its thickness from the room's top (a sloped one by its slope); a raised floor takes what stands above the base. A floor's openings and shafts are holes in what it takes. It is worked out exactly (rationals; construct-rooms' `net_volume`) and rounded once, to a whole cubic grid step, a half up. It is read as the room's `Volume` parameter and as the Rooms schedule's Volume field, and the filled recipe a builder takes is unchanged: the room's solid is still its loops extruded to its height. Not counted, with their places kept: a floor's slab edges (thickened or downturned, hanging below its underside), and ceilings, which are not built yet (job 6.8): a room is bounded by its levels and floors only until they are, and a ceiling's underside will be one more surface in the same integral.

## Areas

An area scheme, an area boundary line and an area are three more element records, in the areas category (`3400`). Area boundaries are their own records, never room lines; an area is found by the very search a room is, over area boundary lines alone: walls bound no area, because a boundary is drawn or picked, not read.

| Record | Kind | Recipe | Its `props` and name |
| --- | --- | --- | --- |
| Area scheme | `area_scheme`, no shape | `area.scheme`, no arguments | `@category` `3400`, `@area.measure` `gross`, `rentable` or `other`; its name is the scheme's name, unique in the document without regard to case |
| Area boundary line | `area_line` | `area.line`, `from` and `to` as a room separation line's | `@category` `3400`, `@area.scheme` the scheme's ID, its base tie |
| Area | `area` | `area`, args `at` (`[x, y]`, two stored lengths) and `lines` (`{"area":"lines"}`, filled before it is built) | `@category` `3400`, `@area.number` its number as text, unique in its scheme, `@area.scheme`, its base tie; its name is its name |

An area plan is a view (the [views format](../construct-views/FORMAT.md): kind `area_plan`, a plan's level and range, and `@area.scheme`). Filled, an area's recipe holds only `loops`, as a room's: the outer loop anticlockwise, then each hole clockwise, each corner `{"at":[x,y],"wall":<the ID of the line whose side runs from it>}` in grid steps, with no height. An area measures its **area** (square grid steps, the holes taken away) and **perimeter** exactly as a room does and has no volume. What becomes of an area is a room's: enclosed, not enclosed (`NotEnclosed`), redundant beside an earlier area of its scheme and level (`Redundant`, the ID first), or refused. Its built row's display is its outer loop, flat. An area is found from the boundary lines of its own scheme and level only, so two schemes' areas never mix, and a changed line or area builds again the areas of its level. An Areas schedule lists each area with `Number`, `Scheme`, `State`, `Area` and `Perimeter` fields, exact; a boundary line is no row of it, as a separation line is none of the Rooms schedule's. A boundary line picked from a wall (`arealine.pick`) is the face of the wall's outline in plan nearest the point clicked, as the walls build now; where two picked walls' faces do not meet at a corner a line drawn across closes the loop (trimming and extending at corners is a later job).

## What it measures

Worked out whenever it is read, never stored, all exact: its **area** in square grid steps (the outer loop's less each hole's, by the shoelace sum; an odd twice-area is refused), its **perimeter** (every edge of every loop, each to the nearest grid step: exact along an axis), and its **volume** in cubic grid steps (area times height less what the floors in its space take: "Room volume"). A person reads them in their units, rounded only there.

**Written by commands.** `room.place`, `room.rename`, `room.number`, `room.move`, `room.upper` and `room.delete` (construct-model's FORMAT.md, "Commands") write this record and no other; a number is unique in its document (the commands refuse one another room has), and the next free number is one more than the largest number made only of digits, or 1 (`next_number`).
