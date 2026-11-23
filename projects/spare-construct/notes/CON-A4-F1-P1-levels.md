# CON-A4-F1-P1: Levels are elements, and elements are tied to them

**Status, 2026-11-23:** Decided for the build (piece A50). The crate `construct-levels` follows it; the stored forms are in its [FORMAT.md](../crates/construct-levels/FORMAT.md), the commands in construct-api's `cloud::levels`, the reading while building in construct-regen's `levels`.

## The question

Every element record carried a plain integer `level`. A level had no name and no height, nothing could be tied to it, and moving a floor up meant editing every element on it. In a building model the level is the datum everything hangs from. The integer is in every record already stored, and records are hashed and never rewritten, so whatever replaces it must read them unchanged.

## The decisions

- A level is an element record of the levels category: a permanent ID, a name, an elevation above the project's zero (a `Length`, negative allowed) and whether it is a building storey (ISO 16739-1's building storey) or a reference datum. It has no shape.
- Names are unique in the document without regard to case, as type names are within a category.
- Levels are ordered by elevation, then name (case aside), then ID. Two levels may share an elevation, and at most one of them is a storey there. A datum at a floor's height (top of slab beside the finished floor, a linked discipline's level) is common; two storeys at one height would make "the storey above" two levels.
- An element is tied to a level by the level's ID and an offset, in its `props`; a wall's top may be tied to another level with its own offset. An element's base is the level's elevation plus the offset, by one function, `height_of`; a wall's height is the difference of two such heights. Placement `z` keeps its meaning: where the shape sits above that base.
- A wall's top tie is read through its recipe, `height` written `{"tie":"top"}`, as a parameter is read through `{"param":...}`: the recipe says where its number comes from, and untying writes the number back.
- The old integer is kept as a field and read for ever: `0` is no level (what types, the parameter table, Workshop parts and elements made without a level always carried), any other `n` the implied level `n`. An implied level's ID is `n` in decimal (`m` and the digits below zero), so the plan index rows written under the integer are already the rows of its level. Its elevation is `(n - 1) x 3000 mm` (`n x 3000 mm` below zero): levels -1, 1, 2 at -3000, 0, 3000 mm, so the old page's 3000 mm walls stack as they were drawn. It has no record until a command changes it, and then the record is the level. New records write `0`.
- Moving a level writes only the level's record. The regenerator's existing `reads` table lists, for each element, every record its shape and place read (parameter table, type, levels), and a change to any of them, or its going, rebuilds the element. The built row carries the level its cells are indexed under and how high its base sits, so a moved level shows in the derived rows; no second mechanism.
- An undo may not leave an element tied to a level that is no longer in the model.

## Why

- Records are the truth (main's ruling 7): a level is a record like a type, read strictly, with its own commands through the one log.
- A tie by ID, not a copied height: a moved level moves everything on it with one entry, and two people editing a wall and its level do not fight over the same record.
- Implied levels keep old documents reading with no command run and no migration entry, and the same answer on every replay, because the rule depends only on the records.
- The integer's own decimal text as the implied ID keeps every derived plan row written before this version valid.

## Not decided here

- Which levels a starter template makes (job 9.6), and whether a new model gets any before someone draws.
- A level's building, for a site of several buildings: one more reserved key on the level record.
- Datum extents and heads per view (job 4.5); grids and reference planes (job 3.2).
- A store regenerated before this version lists no levels in its `reads` rows until `spare-server regenerate` runs once.
