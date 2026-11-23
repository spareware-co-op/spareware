# CON-A8-F1-P1: Views are elements with a kind, a scale, a detail level and a range

**Status, 2026-11-23:** Decided for the build (piece A56). The crate `construct-views` follows it; the stored form is in its [FORMAT.md](../crates/construct-views/FORMAT.md).

## The question

A drawing is made from a view of the model: a plan cut at some height, a section along a line, a three-dimensional picture. Sheets, schedules, dimensions and the page's project browser all need views to be things with names that other things can point at, and to be able to say which elements a view shows. Nothing of the sort existed: a "view" was the plan level the page showed.

## The decisions

- **A view is an element record of the views category**, with no shape, as a level is: a name, a scale, a detail level, staging, and the data of its kind. Records are the truth (main's ruling 7); what a view shows is a pure function of the view and the model, never stored.
- **Seven kinds, each holding its own data.** Floor plan and ceiling plan (a level and a view range), section and elevation (a line, a side and a depth), three-dimensional, drafting and legend (none). The kind is a Rust enum carrying the data, so a section cannot hold a level and a plan cannot lack one. The kinds are the ones building drawing has in general (ISO 128-30 on views and sections, the reflected ceiling plan of common practice).
- **A view range is four ties.** Top, cut plane, bottom and view depth are each a level and an offset (`construct_levels::Tie`, an offset a `Length`), resolved through `height_of`. A range held as raw heights would leave a view behind when a level moves; held as ties it moves with the level and no record changes. The four must run from high to low (equal allowed). A plan is also tied to its own level by the ordinary `@level` tie, so the level is not deleted while a plan is of it; the levels a range names are counted by the same guard (construct-levels' `tied_to` reads the range ties, A61).
- **Scale is construct-styles' `Scale`, the `n` of 1:n, stored as a whole number** (`whole:100`). It is the one scale that picks a pen's printed width (`View::pen_width`, through `PenTable::width`). A scale is never a float, so 1:100 is the same on every machine. 1 to 1,000,000, as the pens accept.
- **Detail level is coarse, medium or fine**, stored as a choice. It is held now so that families can show or hide parts by it (ARCHITECTURE.md) and the drawing job can pick line weights with the scale; this crate does not use it to decide what is shown.
- **Staging and phase settings.** Every view carries the staging fields every element carries (created phase, demolished phase, design option) and may carry phase settings (the phase it shows, its filter and its option choices). A view with no settings shows every element whatever its phase. The rule is construct-phases' `visibility`, called unchanged.
- **What a view shows (`list`).** A pure function over elements given as an ID, a category, a box in the project's frame (whole millimetres, as regeneration stores it) and staging. Only model and import categories are drawn: datums and annotation have rules of their own (their extents and ownership are per view, the next jobs), and other views and analysis stand-ins are not drawn. A floor plan cuts what the plane crosses and sees what lies below it to the view depth, calling what lies below the bottom "beyond"; a ceiling plan is the mirror above the plane up to the top. A section or an elevation (one record shape, since the data is the same) cuts what its line crosses and sees what is in front within the depth and the line's two ends, at every height. A three-dimensional view shows everything. Elements are found with construct-spatial and judged exactly in grid steps, so a cut plane at 12.5 mm is not rounded to 12 or 13. The far corners of a section's region are exact to one grid step (156 nm, a fraction of the 0.01 mm model tolerance), because the offset square to an oblique line is not a whole number of steps; it is rounded outward and never inward.
- **No level on the element given to `list`.** The element's box is in the project's frame, so a plan's reach is a test on heights, and the level an element is tied to is not needed. The level link of a view is how its range is resolved.
- **Staging is JSON text in one `props` key (`@staging`)**, as the phases crate spells it, until the job that wires staging into every element record chooses the shared spelling; the view reads it in one function so that change is one place.

## Through the edit log (A61)

- **Five commands, one file.** `view.create`, `view.rename`, `view.set`, `view.duplicate` and `view.delete` are construct-api's `cloud::views`, in the shape of the level commands: one record written, one undo step, the claim taken, a refusal in `ViewError`'s words. The commands read the document with the crate's `Views::from_records`, make the change with its types, and ask `View::check` and `Views::add` or `replace` before writing, so nothing a command writes is a view the crate would refuse to read.
- **A view's name is unique among all views.** The brief said among views of one kind; the crate's rule (decided in A56) is the stricter, and a command follows the crate, so a section cannot be named as a plan is.
- **Defaults.** 1:100, medium detail, and a plan's range on its own level at 2300, 1200, 0 and -500 mm: the cut plane about 1.2 m above the floor, as the range header describes. They are constants in the command file, so a project's own defaults replace them in one place.
- **No change of kind.** A plan is not turned into a section: the data of a kind is the kind, and a new view is made instead. `view.set` refuses what the kind has no use for.
- **A set that changes nothing writes nothing,** so it is no undo step (the rule the undo's own step finder already holds).
- **The generic commands do not go behind them.** `element.create`, `element.set`, `element.move` and `element.delete` of a view are refused, naming the view command; a view is not an element with a shape, so `has_shape` is false for it and the regenerator and the package pass it over, as they pass a type.
- **Levels cannot be deleted from under a range**, nor undone from under one: construct-levels' `tied_to` counts the range ties, and an undo that would leave a view on a missing level is turned back.
- **Interim: phases.** A document stores no phases or options yet (the phases crate is values only), so a view's staging and phase settings are checked against the two phases every project starts with and no options. One function in the command file (`phases`, `options`) is replaced by the job that stores them.
- **What a view shows is read by the server.** `GET .../views` and `GET .../views/<view>`. The shapes' boxes are in each element's own frame with its base's height kept beside them (the built row's `elevation`), so the listing lifts each box by that height before asking `list`; a model whose elements stand on different levels is therefore cut at the right height.

## Why

- A level-tied range is the one way a floor plan keeps working when a storey is raised; this is what a record per view gets over a page setting.
- The kind as data, not as a string plus optional fields, makes half-formed views unrepresentable and the reader short.
- Exact integer tests (checked `i128`, grid steps) keep to the rule that no length is rounded to answer a question, and give the same answer on every machine.

## Not decided here

- How a view is drawn (jobs 7.2 and 8.1), per-view graphics rules (5.6), crop regions (7.7), view templates and overrides (8.5), and datum extents and annotation per view.
- A section's height range (today every height), a view's plan region (crop), and three-dimensional camera settings.
- The browser tree (10.5) and drawing a plan from a view (7.2).
