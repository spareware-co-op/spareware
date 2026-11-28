# CON-A8-F3-P4, P6 and P3: View templates, element overrides and view filters

**Status, 2026-11-28:** Built (piece A93). The data and commands are in `construct-views` and construct-api's `cloud::view_templates` (and `cloud::views` for the effective views); the stored forms are in the crate's [FORMAT.md](../crates/construct-views/FORMAT.md); the drawing asks construct-drawing's `Looks::look_for`; the page has Templates, Filters and Overrides on the open view's line and a template's own graphics and filters.

## The question

An architect sets up a plan once (scale, detail, which categories show, how walls look) and wants thirty plans to follow it, and to change the set in one place. In one view they want one wall in red or hidden. And they want every wall rated 1 h to show in a colour without selecting them.

## The decisions

- **A template is its own record** (kind `view_template`), not a view with a flag: a view is something drawn, a template is settings only, so nothing has to ask whether a view is "really" a template, and no list of views shows one.
- **Five settings, each with an included flag**: scale, detail level, graphics rules, which categories show, filters. Not included means the view's own.
- **Tied is a read, not a write.** A tied view stores its tie and the values it showed when tied; the effective view (its own settings with the template's included ones over them) is made when the document is read (`ViewTemplates::effective`, once, in `read_world`). So a template change is one record, every tied view changes in that log entry, no claims are needed on the views and undo is one step. The cost is that every reader of views goes through `World::views` (the effective views) and every writer through `subject` (the record as stored); the two are kept apart by type and by name.
- **Applied once** copies the included settings and leaves the view free. **Untie** writes what the view showed, so untying changes nothing on screen.
- **The refusal for a tied view** is a function of the template (`tied_refusal`), asked by every command that would change an included setting, with one wording.
- **A template cannot be deleted while a view is tied** (the refusal counts them) and an undo cannot leave a tie dangling: the alternative, untying every view silently, would write many records under one person's claim.
- **Drafting views and legends take no template, filter or override** (they show no model). A template's graphics are fitted to the kind of view that follows it (no cut rules in a three-dimensional view) rather than refused, so one template serves every model view.
- **Hidden categories are a view-wide switch** (`@view.hidden`), above the graphics rules' per-edge `shown`; `resolve` folds it in, so every reader sees it.
- **An override is partial** (the parts named) and stored per view and element on the view record; it is not a template setting, so a tied view keeps its own. It beats every filter on the parts it names.
- **A filter is a rule on a category, a parameter, an operator and a value**, with a partial look; the first matching filter wins each part, a later one fills the rest. Filters are evaluated when a view is drawn, against the element's parameters read as they are then, so there is nothing to invalidate. Operators are `equals`, `contains` and `greater_than`, each only for the parameter kinds it can compare.
- **Drawing.** `Looks::look_for(element, category, edge)` defaults to `look`; the server answers it with the element's filters and override over `resolve`. When any element has its own look the drawing merges cut regions only among elements that look alike (`Looks::per_element`), so a halftone wall beside a solid wall of one material stays two hatches; with none, the old path runs and the bytes are the ones before.

## Interim

View range is not a template setting yet (a plan's range stays its own), and crop regions (piece A79) are not on main: the next job adds them as more settings. Filters are held per view or template, not as named project-wide filters shared by many views; a shared filter list is a later job if people ask for it.

## Why

- One effective-views function makes the template rule testable in one place with no store (construct-views' tests), and the commands stay thin.
- Matching on parameters through the same reader the properties panel uses means a filter can never disagree with what a person sees in the panel.
