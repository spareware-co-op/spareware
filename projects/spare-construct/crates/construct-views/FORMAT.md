# Views, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record it uses unchanged, the [parameter values](../construct-params/FORMAT.md) it stores, the [level ties](../construct-levels/FORMAT.md) it shares and the [phase fields](../construct-phases/FORMAT.md) it holds. **We may stop writing an old version. We never stop reading it.** [`src/view.rs`](src/view.rs) is the reader library.

## A view

A view is an element record like any other (the records in the database are the truth), of the views category. It has no shape: nothing builds it, and what it shows is a question asked of the model, never stored.

| Field | On a view record |
| --- | --- |
| `id` | The view's permanent ID |
| `document` | The document it belongs to |
| `kind` | `view` |
| `name` | The name people see: trimmed, not empty, unique in the document without regard to case |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]` |
| `recipe` | `{"owner":"construct","schema":1,"op":"view","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `props` | The entries below, and nothing else |

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every view | `7000`, the views category ([construct-categories](../construct-categories/FORMAT.md)) |
| `@view.kind` | Every view | `choice:` and one of `floor_plan`, `ceiling_plan`, `section`, `elevation`, `three_d`, `drafting`, `legend` |
| `@view.scale` | Every view | `whole:` and the `n` of 1:n, a whole number from 1 to 1,000,000, no leading zero: `whole:100` is 1:100 |
| `@view.detail` | Every view | `choice:coarse`, `choice:medium` or `choice:fine` |
| `@staging` | Every view | The view's own staging fields, as the [phases format](../construct-phases/FORMAT.md) spells them, as JSON text with the keys in this order: `{"created":"ph-a","demolished":null,"design_option":null}` |
| `@view.phasing` | A view with phase settings | The phases format's view settings as JSON text, keys in this order: `{"phase":"ph-b","filter":"show_new","choices":[{"set":"s","option":"o"}]}`; `choices` sorted by set ID. A view without the key shows every element whatever its phase |
| `@level`, `@level.offset` | A floor plan or a ceiling plan | The level the plan is of, and `length:q0:0`: the tie construct-levels writes for any element, so a level is not deleted while a plan is of it |
| `@view.range.top`, `@view.range.cut`, `@view.range.bottom`, `@view.range.depth` | A plan | The ID of the level each height is tied to |
| the same with `.offset` after the key | A plan | The height's offset from that level in stored length text (`length:q0:7680000` is 1,200 mm). Each pair is written together |
| `@view.line.x0`, `@view.line.y0`, `@view.line.x1`, `@view.line.y1` | A section or an elevation | The first and second end of its line in the plan, stored length text, in the project's frame |
| `@view.look` | A section or an elevation | `choice:left` or `choice:right`: the side of the line, walking from its first end to its second, that the view looks to |
| `@view.far` | A section or an elevation | How far it sees, square to the line: stored length text, above zero |

Which keys a record has depends on its kind, and no other keys are allowed: a section with a `@level`, or a plan with a line, is refused. Lengths are construct-params' stored length text in its one spelling (`length:q0:` and the grid steps); the older whole-millimetre spelling is not accepted here, since no view was ever written with it. A view range's height is `height_of` of its level plus its offset (construct-levels), so moving a level moves every view range tied to it and no view record changes.

```json
{"id":"V-1","document":"DOC-1","kind":"view","name":"Ground floor plan","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"view","op_version":1,"args":{}},"props":{"@category":"7000","@level":"L-1","@level.offset":"length:q0:0","@staging":"{\"created\":\"ph-a\",\"demolished\":null,\"design_option\":null}","@view.detail":"choice:medium","@view.kind":"choice:floor_plan","@view.range.bottom":"L-1","@view.range.bottom.offset":"length:q0:0","@view.range.cut":"L-1","@view.range.cut.offset":"length:q0:7680000","@view.range.depth":"L-1","@view.range.depth.offset":"length:q0:-3200000","@view.range.top":"L-1","@view.range.top.offset":"length:q0:14720000","@view.scale":"whole:100"},"references":[]}
```

A record that is not exactly as written is refused, never repaired: a reader names the record and why. The same view is always the same bytes.

## The rules a command checks

A view's name is unique in its document, case aside; a view may change the case of its own name. A view range must run top, cut plane, bottom, view depth from high to low (equal heights are allowed), as the levels give them. A section's or elevation's ends are two different points and its depth is above zero. A view's staging and phase settings must name phases and options the document has. These bind what commands write; reading a document never applies them, so a document always reads. Whether a view stands in a document is asked with `View::check`.

## Through the edit log

The view commands are construct-api's [`cloud::views`](../construct-api/src/cloud/views.rs), listed with their arguments in the [cloud format](../construct-model/FORMAT.md): `view.create`, `view.rename`, `view.set`, `view.duplicate` and `view.delete`. Each is one undo step, runs behind `model.edit` and takes the claim on the view it writes, and refuses in this crate's own words (`ViewError`). What they add to the rules above:

- A new view is 1:100 and medium detail unless given, and a new plan's range is its own level's, at 2300 mm (top), 1200 mm (cut plane), 0 (bottom) and -500 mm (view depth). A plan's `range` names any of its four heights as `{"level", "offset"}` with either part optional; one not named keeps what it has. Moving a plan to another `level` leaves its range where it is.
- A view's kind does not change, and an argument that is not its kind's (a `level` for a section, a range for a legend, a line for a plan) is refused, naming the kind. The line is `from` and `to` as `{"x", "y"}` typed lengths, `look` as `left` or `right`, `depth` as a typed length above zero.
- A name is unique among all the views of a document, whatever their kind, case aside. `view.set` or `view.rename` with what the view already has writes nothing and is no step.
- The element commands refuse a view, naming these. A level a plan is of, or one its range is tied to, is not deleted: construct-levels counts the ties under `RANGE_KEYS` ([`tied_to`](../construct-levels/src/levels.rs)). An undo that would leave a view on a level the model no longer has is turned back.
- Interim: a document stores no phases or design options yet. Until a job stores them through the log, a view's `staging` and `phasing` may name the two phases every project starts with (`phase-existing`, `phase-new`) and no design option exists to name, and every element is read as created in `phase-new` when a view is asked what it shows. A new view is created in `phase-new`.

The server reads them at `GET /api/projects/<project>/models/<model>/views` (a JSON list of `{id, name, kind, level, level_name, scale, detail}`, by name) and `.../views/<view>` (`{view, cut, seen, beyond}`: the sorted IDs of the elements the view cuts, sees, and a plan sees below its bottom), behind `model.read`. A floor plan is drawn at `.../views/<view>/plan` (`{view, through, svg, skipped}`: the cut at the view range's cut height, printed at the view's scale by construct-drawing, and the elements it could not draw, each with the reason), by the server's `plan` module over the kernel builder; a server built without the kernel refuses it in words. The second is [`list`](src/listing.rs) over the model's stored shapes, each box lifted by the height its level gives its base (the built row's `elevation`), the low corner rounded down and the high one up to whole millimetres.

## What a view shows

Not stored: `construct_views::list` answers it from the view and the model's elements (their category, box in whole millimetres in the project's frame, and staging), by the rules in that file's header: the category must be one a view draws (model and import categories), the phase rule is [construct-phases](../construct-phases/FORMAT.md)'s own, and the reach is the view range for a plan (cut, seen, beyond), the region of the line for a section or an elevation (cut, seen), and everything for a three-dimensional view. A drafting view and a legend show no model. Answers are sorted by element ID.

## Later versions

Graphics rules per view, crop regions, a view template and per-element overrides will each be more reserved keys on this record, read strictly as these are; version 1 records read as views with none. The shared spelling of an element's staging fields for every kind of element is the job that wires them into records; until it lands, a view's is the `@staging` key above, read in one function.
