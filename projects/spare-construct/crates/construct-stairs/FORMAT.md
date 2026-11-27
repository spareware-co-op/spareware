# Stairs and stair rule sets, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record and recipe a stair uses unchanged, the [levels format](../construct-levels/FORMAT.md) whose ties it carries and the [parameter values](../construct-params/FORMAT.md) whose length spelling it uses. **We may stop writing an old version. We never stop reading it.** [`src/rules.rs`](src/rules.rs), [`src/stair.rs`](src/stair.rs) and [`src/record.rs`](src/record.rs) are the reader library; the decisions behind it are in [the straight stairs note](../../notes/CON-A4-F5-P1-straight-stairs.md).

## A rule set

The limits a stair is laid out by, as data: one JSON text per country's code, beside this crate as `rules/<id>.json`. The first is [`rules/nz-d1-as1.json`](rules/nz-d1-as1.json): New Zealand's Building Code clause D1 (Access routes) through its Acceptable Solution D1/AS1, second edition, amendment 6, effective 1 January 2017. Another country's limits are another file of the same form and one line in `RuleSet::builtin`.

| Field | Value |
| --- | --- |
| `version` | `1` |
| `id`, `name`, `edition`, `source` | Its ID (`nz-d1-as1`), the code it comes from, the edition and amendment it holds, and where the code is published |
| `pitch_least` | `{"degrees":<whole degrees>,"clause"}`: the shallowest pitch of any stair |
| `riser_least`, `going_most`, `uniform_within`, `landing_least`, `headroom_least` | Each `{"length":"<typed text>","clause"}`: the lowest riser, the longest going, the most two risers of one flight may differ, the shortest landing, the least clear height above the pitch line |
| `categories` | At least one, each `{"id","name","pitch_most","riser_most","going_least","rise_between_landings_most"}` and, where the code sets one, `"width_least"` |

A length is written as a person types it (`"190 mm"`) and read with construct-params' metric display settings, so a length off the grid is refused; a pitch is whole degrees from 1 to 89, compared as the slope of rise `round(10^9 tan a)` over 10^9 (construct-roofs' rule for a pitch in degrees). Every clause is given, no category ID twice, no field added or missing: anything else is refused, naming what does not read.

New Zealand's figures, each cited in the file by clause:

| Category | Most pitch | Most riser | Least going | Most rise between landings | Least width |
| --- | --- | --- | --- | --- | --- |
| `accessible` | 32 degrees | 180 mm | 310 mm | 2500 mm | 900 mm |
| `common` | 37 degrees | 190 mm | 280 mm | 2500 mm | 900 mm |
| `main-private` | 37 degrees | 190 mm | 280 mm | 4000 mm | none |
| `secondary-private` | 41 degrees | 200 mm | 250 mm | 4000 mm | none |
| `minor-private` | 47 degrees | 220 mm | 220 mm | 4000 mm | none |
| `service` | 47 degrees | 220 mm | 220 mm | 4000 mm | none |

and for every stair: least pitch 23 degrees, least riser 150 mm, most going 400 mm, risers within 5 mm of each other in a flight, landings at least 900 mm long, 2000 mm clear above the pitch line.

## A stair

A stair is an element record like any other, of the stairs category:

| Field | On a stair record |
| --- | --- |
| `kind` | `stair` |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: its start is where it is |
| `recipe` | `{"owner":"construct","schema":1,"op":"stair","op_version":1,"args":{...}}`, below |
| `props` | `@category` `800`, its base's tie (`@level`, `@level.offset`) and its top's tie (`@top.level`, `@top.offset`), construct-levels' keys |

The recipe's arguments, in sorted order as construct-model writes them, each length construct-params' stored spelling `length:q0:<steps>`:

| Argument | Value |
| --- | --- |
| `category` | The category's ID in its rule set |
| `direction` | Which way its first run goes up: `+x`, `-x`, `+y` or `-y` |
| `going` | Only when set: the going of every tread. Without it the stair takes its category's least going |
| `landings` | One between each two runs, from the bottom, each `{"length":"<length>","turn":"straight"\|"left"\|"right"\|"back-left"\|"back-right"}` |
| `rise` | `{"tie":"top"}`: read from its levels when it is built |
| `rules` | Its rule set's ID |
| `runs` | At least one, at most 64, from the bottom, each `{}` (it shares the risers the others leave) or `{"risers":<n>}`, `n` at least 2 |
| `start` | `["<x>","<y>"]`: the foot of its first riser, on its right-hand side going up |
| `waist` | The least thickness of its structure under the steps, square to its underside |
| `width` | Its width |

A main private stairway at (1000, 2000) going up +y, 1100 mm wide, its first run of 8 risers and its second taking the rest, a 1200 mm landing turning left, from level `L-1` to 50 mm below level `L-2`:

```json
{"id":"S-1","document":"DOC","kind":"stair","name":"Stair 1","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"stair","op_version":1,"args":{"category":"main-private","direction":"+y","landings":[{"length":"length:q0:7680000","turn":"left"}],"rise":{"tie":"top"},"rules":"nz-d1-as1","runs":[{"risers":8},{}],"start":["length:q0:6400000","length:q0:12800000"],"waist":"length:q0:960000","width":"length:q0:7040000"}},"props":{"@category":"800","@level":"L-1","@level.offset":"length:q0:0","@top.level":"L-2","@top.offset":"length:q0:-320000"},"references":[]}
```

Arguments that are not exactly as written are refused, never repaired: one missing or added, a length in another spelling (an old whole-millimetre `length:1100`), a direction or a turn not among its names, a run with one riser or a field added. A stair is refused, in words, for: no run, or more than 64; landings not one fewer than the runs; a width, waist, going or landing of nothing or less, or over 100 m; more than 1000 risers; a start beyond the kernel's grid; a landing that turns a quarter shorter than the stair is wide.

**The layout** is worked out from its rise whenever it is built or read, never stored: the going (the stair's, else its category's least) at least its category's least and at most the rule set's most; the width at least its category's least; every landing at least the rule set's least; then the least count of risers `n` whose risers lie between the lowest and highest the limits allow on that going (the rule set's least riser or the riser at its least pitch, whichever is higher; the category's most riser or the riser at its most pitch, whichever is lower), that the runs allow (two to a run at least, a run's own count exactly) and that differ by no more than the rule set allows. A rise `H` in `n` risers is laid out as risers `floor(k H / n) - floor((k - 1) H / n)`, `k` from 1: their sum is the rise exactly and they differ by one grid step at most. Runs that give no count share what is left, the earlier ones taking one more; each run rises at most its category's most between landings. A rise no count lays out is refused naming the two limits and the nearest floor-to-floor heights above and below that would work.

**The pieces**, in the stair's base frame (its base at `z = 0`, the plan the model's): each run is its side profile lofted across its width, each landing its outline extruded up by the waist; the decisions note says where every corner is. One piece passing over another not next to it keeps the rule set's headroom above the lower's pitch line.

## What the builders take

Before it is built the regenerator fills `rise` with the height from the stair's base to its top tie and lays it out (construct-regen's `stairs`); the recipe the builders take holds only:

| Argument | Value |
| --- | --- |
| `material` | The material every piece is drawn in: `concrete-regular` |
| `pieces` | From the bottom, a run `{"id":"run.<k>","treads":<n>,"plan":[[x,y], ...],"loft":[[[x,y,z], ...],[[x,y,z], ...]]}` (its profile at its right-hand side, and the same at its left) and a landing `{"id":"landing.<k>","plan":[[x,y], ...],"extrude":[[x,y,z], ...],"by":[0,0,<waist>]}`, every number grid steps, each `plan` four corners anticlockwise from above |

## What a stair reads back

Read-only parameters, worked out from its layout whenever they are read and never stored: `stair.rise`, `stair.risers` (whole), `stair.riser-height` (the tallest), `stair.treads` (whole), `stair.going`, `stair.run-length` (the walking line in plan: treads and landings), `stair.pitch` (the tallest riser over the going, a slope) and `stair.width`.
