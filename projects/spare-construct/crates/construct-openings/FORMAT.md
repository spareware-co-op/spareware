# Openings and shafts, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record and recipe an opening and a shaft use unchanged, the [levels format](../construct-levels/FORMAT.md) whose ties a shaft carries and the [parameter values](../construct-params/FORMAT.md) whose length spelling they use. **We may stop writing an old version. We never stop reading it.** [`src/record.rs`](src/record.rs) and [`src/outline.rs`](src/outline.rs) are the reader library; the decisions behind it are in [the openings note](../../notes/CON-A4-F3-P2-openings-and-shafts.md).

Each record is of the openings category (`props` `@category` `1600`), `level` `0`, and its recipe's owner is `construct`, op version `1`. A recipe of another version, or with an argument not listed here, is refused, never repaired.

## A wall opening

A rectangular hole through a wall that is not a door or a window. Kind `opening`, recipe `opening.wall`.

| Recipe argument | Value |
| --- | --- |
| `width` | Its width along its wall, a stored length (`length:q0:<steps>`), above 0 |
| `height` | Its height from its sill to its head, a stored length, above 0 |
| `axis` | `x` or `y`: the plan axis its wall runs along |

It is hosted on a side of its wall exactly as a door is (the cloud format's "Hosted elements"): one `face` reference to the wall's `exterior` or `interior`, `@host.along` its position along the wall from the wall's start and `@host.up` its sill height above the wall's base. It moves with its wall. It is cut through the wall's whole thickness, every layer, on the door's own path, and is refused in the same terms (named `opening <ID>` where a door is `door <ID>`): outside its wall, across or touching an end, its head at or above the wall's top, below its base, overlapping or touching another opening or a door, across its wall's axis, within the engine's tolerance of a face, or in a wall at an angle.

## A floor opening

A hole through a floor, given by its outline. Kind `opening`, recipe `opening.floor`.

| Recipe argument | Value |
| --- | --- |
| `outline` | Its outline in its floor's own frame: an array of corners `[x,y]` in whole grid steps (6,400 to the millimetre), in the order drawn, either way round |

It is hosted on its floor by one reference of kind `face` (`doc` `null`) to the floor's top face (`top` on the kernel's edition, `box.top` on the stand-in's), `want` `carrier`, and no `@host.*` keys; it sits where its floor does and moves with it.

## A shaft

A closed outline between a base level and a top level, each with an offset, cutting every floor it passes through. Kind `shaft`, recipe `shaft`.

| Where | Value |
| --- | --- |
| recipe `outline` | As a floor opening's, from the shaft's placement |
| recipe `height` | `{"tie":"top"}`: filled before it is built with the height from its base to its top, as a stored length |
| `props` `@level`, `@level.offset` | Its base's tie |
| `props` `@top.level`, `@top.offset` | Its top's tie |
| `placement` | Where its outline is in plan, in whole millimetres; `z` `0` (its base is its tie's) |

It is hosted on nothing. It cuts every floor of its document that it passes wholly through (its base at or below the floor's bottom face and its top at or above its top face) and whose outline it lies inside in plan; it passes by a floor it misses in height, or only touches, and one it lies beside in plan. One whose top is at or below its base is refused (`ShaftNoHeight`, `ShaftTopBelowBase`); one ending inside a floor's thickness is that floor's build failure (`ShaftEndsInFloor`).

## Outlines

At least three corners, no two one after the other in one place, and no two edges crossing, touching or running over each other: a floor's loop checks (construct-slabs' boundary) asked of the outline alone. In its floor, with the floor's own holes, every outline must pass the floor's hole checks: wholly inside the outer loop touching nothing of it, and apart from every other hole and outline. A corner written as an object (`{"arc":...}`) is a curved edge, refused (`CurvedOutline`) until the engine seam's extrusion takes curves. A corner that is not two whole numbers is refused (`BadOpening`).

## Commands

The commands that make, change and delete them are construct-api's `cloud/openings.rs` (`opening.place`, `opening.draw`, `opening.size`, `opening.move`, `opening.outline`, `opening.delete`, `shaft.create`, `shaft.outline`, `shaft.reach`, `shaft.delete`), listed with their arguments in the [cloud format](../construct-model/FORMAT.md), "Commands". Every refusal they pass on is [`OpeningError`](src/error.rs)'s words or the kernel builder's; a floor's area, volume and mass after the cut are construct-slabs' own, worked out of the boundary with the cutting outlines as more holes ([`with_cuts`](src/measure.rs)).

## What a host holds after the cut

Worked out exactly in construct-params' units and never stored ([`src/measure.rs`](src/measure.rs)): a wall layer's side is its length by its height less every opening's width by height, its volume that by its thickness; a floor layer's top is its outer loop's area less its holes' and its cut outlines', its volume that by its thickness.
