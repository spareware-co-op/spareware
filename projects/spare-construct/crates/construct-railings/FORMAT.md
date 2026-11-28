# Railings and railing rule sets, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record and recipe a railing uses unchanged, the [levels format](../construct-levels/FORMAT.md) whose tie it carries, the [stairs format](../construct-stairs/FORMAT.md) whose stairs it follows and the [parameter values](../construct-params/FORMAT.md) whose length spelling it uses. **We may stop writing an old version. We never stop reading it.** [`src/rules.rs`](src/rules.rs), [`src/kind.rs`](src/kind.rs), [`src/railing.rs`](src/railing.rs) and [`src/record.rs`](src/record.rs) are the reader library; the decisions behind it are in [the railings note](../../notes/CON-A4-F5-P3-railings.md).

## A rule set

The limits a railing is checked by, as data: one JSON text per country's code, beside this crate as `rules/<id>.json`. The first is [`rules/nz-f4-as1.json`](rules/nz-f4-as1.json): New Zealand's Building Code clause F4 (Safety from falling) through its Acceptable Solution F4/AS1, third edition, amendment 2, effective 1 January 2017, with the handrail heights of D1/AS1 (second edition, amendment 6, paragraph 6.0.6). Another country's limits are another file of the same form and one line in `RuleSet::builtin`.

| Field | Value |
| --- | --- |
| `version` | `1` |
| `id`, `name`, `edition`, `source` | Its ID (`nz-f4-as1`), the code it comes from, the edition and amendment it holds, and where the code is published |
| `categories` | At least one, each `{"id","name","level_least","openings"}` and, where the code sets one, `"flight_least"`: where a barrier stands, its least height on the level and on a stair's flight (measured upright from the pitch line), and the openings rules it may be checked by, its default first |
| `openings` | At least one, each `{"id","name"}` and either `"sphere"` and `"stair_sphere"` (no sphere of that diameter passes: anywhere, and under the barrier's foot on a flight) or `"between_verticals"` and `"between_rails"` (the widest openings allowed: between upright members, and upright between the floor or a tread and the barrier's foot) |
| `handrail` | `{"least","most"}`: a handrail's top above the pitch line |

Each limit is `{"length":"<typed text>","clause"}`, read with construct-params' metric display settings, so a length off the grid is refused. Every clause is given, no ID twice, every openings rule a category names is in the set, no field added or missing: anything else is refused, naming what does not read.

New Zealand's figures, each cited in the file by clause (F4/AS1 Table 1 and paragraphs 1.2.1 and 1.2.2):

| Category | On a flight | On the level | Openings rules |
| --- | --- | --- | --- |
| `household-stair` (stairs, ramps and their intermediate landings, in a house) | 900 mm | 900 mm | `children` |
| `household-edge` (balconies, decks, edges of floors, in a house) | | 1000 mm | `children` |
| `stair` (stairs or ramps, other buildings; its landings are "all other locations", Note 5) | 900 mm | 1100 mm | `children`, `no-children` |
| `seating` (within 530 mm of fixed seating) | | 800 mm | `children`, `no-children` |
| `other` (all other locations, other buildings) | | 1100 mm | `children`, `no-children` |

| Openings rule | Limits |
| --- | --- |
| `children` (housing, and areas children under 6 are likely to frequent) | No 100 mm sphere passes anywhere (1.2.1 b)); no 150 mm sphere passes the opening of riser, tread and the barrier's foot on a stair (1.2.1 c), Figure 4) |
| `no-children` (areas only for emergency or maintenance, and buildings children do not frequent) | At most 300 mm between upright members (1.2.2 a)); at most 460 mm upright under the barrier's foot (1.2.2 b)) |

and a handrail's top 900 mm to 1000 mm above the pitch line (D1/AS1 6.0.6).

## A railing

A railing is an element record like any other, of the railings category:

| Field | On a railing record |
| --- | --- |
| `kind` | `railing` |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]`: its path is where it is |
| `props` | `@category` (the railings category), and its base's tie to a level (`@level`, `@level.offset`); a railing on a stair is tied as its stair's base was when it was made, and is raised or lowered to wherever the stair's base stands |
| `recipe` | `{"owner":"construct","schema":1,"op":"railing","op_version":1,"args":{...}}`, below |

The recipe's arguments, each in its one spelling (lengths as `length:q0:<steps>`):

| Argument | Value |
| --- | --- |
| `rules`, `category`, `openings` | The rule set's ID, the category's and the openings rule's: what the railing is checked by |
| `flipped` | `true` or `false`: a sketched railing's handrail on the right of its path's direction, not the left. A railing on a stair is never flipped (it flips by its side) |
| `path` | `{"points":[[x,y], ...]}`: two to 256 points in plan, straight segments on the railing's base, none of no length, over 100 m or turning straight back; or `{"stair":"<ID>","side":"left"\|"right"\|"both","inset":<length>}`: a stair's side or sides seen going up, `inset` (at most 500 mm) in from its edge |
| `type` | `{"top_rail":{"height","profile"},"balusters":{"bottom","justify","profile","spacing"},"posts":<profile>,"material":"<ID>"}` and, if it has one, `"handrail":{"height","offset","profile"}` |

A profile is `{"rectangle":[<width>,<depth>]}` (across the path; upright for a rail, along the path for a baluster or a post) or `{"round":<diameter>}`, each more than nothing and at most 500 mm. Heights are more than nothing and at most 3 m, to the top of the rail; a handrail's offset (in from the railing's line, on the side it is used from) and the balusters' foot (above the floor, or the pitch line on a flight) at least nothing and at most 1 m; the spacing more than nothing, at most 10 m and at least the balusters' depth; the material one of the default library's (construct-materials). `justify` is `start`, `centre` or `end`.

## How it is laid out

Written out in [`src/path.rs`](src/path.rs), [`src/layout.rs`](src/layout.rs) and [`src/shape.rs`](src/shape.rs), whose headers are the rule; in short:

- **Lines.** A sketched path is one line, level. A railing on a stair follows each run's side from its first nosing to its last along the pitch line and the landings' edges between, inset toward the stair; at each run's foot the line steps up one riser (its post reaches from the floor to the rail); at a turn it runs on to the run's end before it turns, and at a half turn round the end of the well, so every flight's slope is the stair's.
- **Posts** at every vertex; **balusters** on each piece between two posts, `ceil(len / spacing) - 1` of them, justified, measured in plan, leaving out any that would stand inside a post.
- **Every point** is worked out exactly and rounded once to the nearest grid step, a half away from zero (main's any-angle rule): points along a piece at any angle divide by its length, a rail's section on a slope is stretched by `sqrt(run^2 + rise^2) / run`, a round profile's corners carry `sqrt(3) / 2`.
- **Openings** are worked out exactly: between upright members, under the balusters on the level (`bottom`), and on a flight the largest disc in the corner of a tread and the riser above it under the balusters' foot.

## What the builders take

Before it is built the recipe is filled ([construct-regen's railings](../construct-regen/src/railings.rs), construct-railings' `filled`) with `material` and `pieces`, each `{"id","part","extrude":[[x,y,z], ...],"by":[x,y,z]}` in grid steps in the railing's frame: `post.<i>.<k>`, `rail.<i>.<k>`, `handrail.<i>.<k>` and `baluster.<i>.<k>.<j>` for line `i`, vertex or piece `k` and baluster `j`, each from 1; `part` is `post`, `top-rail`, `handrail` or `baluster`. A post's or a baluster's profile is in plan and swept up; a rail's is upright in the plane `x = const` or `y = const` square to the plan axis its piece runs nearer, and swept along the piece.

## What it reads back

Worked out from its layout whenever read, never stored, read-only: `railing.rail-length` (every piece of top rail along its slope, each rounded once), `railing.handrail-length` (none without a handrail), `railing.balusters`, `railing.posts` and `railing.height`.

## Its warnings

A limit broken is a warning, never a refusal, in construct-warnings' shape (a stable kind, a severity, the railing's ID, a sentence; its identity `kind|ID`), at most one of each kind per railing, for the first place that breaks it:

| Kind | Found when |
| --- | --- |
| `railing.handrail` | A handrail on a flight whose top is not within the rule set's handrail heights |
| `railing.low` | The top rail is lower than the category's least height on a flight or on the level |
| `railing.opening` | An opening a sphere of the openings rule passes, or wider than its most |

```json
{"identity":"railing.opening|R-1","kind":"railing.opening","severity":"warning","words":"Railing R-1: the opening between upright members on piece 1 of line 1 is 110 mm, and a sphere of 100 mm (F4/AS1 1.2.1 b)) must not pass it."}
```
