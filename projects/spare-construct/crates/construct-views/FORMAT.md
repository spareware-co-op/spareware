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

## What a view shows

Not stored: `construct_views::list` answers it from the view and the model's elements (their category, box in whole millimetres in the project's frame, and staging), by the rules in that file's header: the category must be one a view draws (model and import categories), the phase rule is [construct-phases](../construct-phases/FORMAT.md)'s own, and the reach is the view range for a plan (cut, seen, beyond), the region of the line for a section or an elevation (cut, seen), and everything for a three-dimensional view. A drafting view and a legend show no model. Answers are sorted by element ID.

## Later versions

Graphics rules per view, crop regions, a view template and per-element overrides will each be more reserved keys on this record, read strictly as these are; version 1 records read as views with none. The shared spelling of an element's staging fields for every kind of element is the job that wires them into records; until it lands, a view's is the `@staging` key above, read in one function.
