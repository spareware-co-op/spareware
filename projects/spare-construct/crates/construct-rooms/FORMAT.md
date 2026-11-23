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

## What it measures

Worked out whenever it is read, never stored, all exact: its **area** in square grid steps (the outer loop's less each hole's, by the shoelace sum; an odd twice-area is refused), its **perimeter** (every edge of every loop, each to the nearest grid step: exact along an axis), and its **volume** in cubic grid steps (area times height). A person reads them in their units, rounded only there.
