# Sheets and viewports, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record it uses unchanged, the [parameter values](../construct-params/FORMAT.md) it stores and the [views](../construct-views/FORMAT.md) it places. **We may stop writing an old version. We never stop reading it.** [`src/sheet.rs`](src/sheet.rs) and [`src/viewport.rs`](src/viewport.rs) are the reader library. A document made before sheets existed has no record of either kind and reads as having none.

## A sheet

A sheet is an element record of the sheets category. It has no shape: nothing builds it.

| Field | On a sheet record |
| --- | --- |
| `id` | The sheet's permanent ID |
| `document` | The document it belongs to |
| `kind` | `sheet` |
| `name` | The name people see: trimmed, not empty. Names are not unique |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]` |
| `recipe` | `{"owner":"construct","schema":1,"op":"sheet","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `props` | The entries below, then the sheet's parameter values |

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every sheet | `7100`, the sheets category ([construct-categories](../construct-categories/FORMAT.md)) |
| `@sheet.number` | Every sheet | The number as text: trimmed, 1 to 32 characters, no control character. Unique in the document without regard to case (a rule commands keep, not one reading applies) |
| `@sheet.paper` | Every sheet | `choice:` and one of `a0`, `a1`, `a2`, `a3`, `a4`, `a5` (ISO 216) or `custom` |
| `@sheet.orientation` | A standard size | `choice:landscape` or `choice:portrait` |
| `@sheet.width`, `@sheet.height` | A custom paper | Stored length text, each from 50 mm to 5000 mm |

Which keys a record has depends on its paper, and no other key beginning `@` is allowed. A key that does not begin `@` is a parameter value, held as construct-params holds an element's values (the parameter's ID, then its stored text); the sheet reader leaves those to construct-params, and a command that rewrites a sheet keeps them.

**How paper lengths are held.** A standard size is not stored as lengths: its key and orientation give the ISO 216 sides in whole millimetres (A1 is 841 by 594 mm landscape), exactly, in the code. A custom size is two `Length`s in the one stored spelling (`length:q0:` and the kernel's grid steps of 1/6,400 mm), so 297.5 mm is `length:q0:1904000`, never rounded. Paper has its own frame: the origin is the lower left corner, x runs right and y runs up.

```json
{"id":"S-1","document":"DOC-1","kind":"sheet","name":"Ground floor plan","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"sheet","op_version":1,"args":{}},"props":{"@category":"7100","@sheet.number":"A101","@sheet.orientation":"choice:landscape","@sheet.paper":"choice:a1"},"references":[]}
```

### Sheet parameters

The standard sheet parameters ([`STANDARD`](src/parameters.rs)) are text parameters of the instance with fixed IDs, bound to the sheets category in the document's parameter table: `sheet.drawn-by`, `sheet.checked-by`, `sheet.approved-by`, `sheet.issue-date` (text, `2026-11-24`), `sheet.revision`, `sheet.revision-date`, `sheet.revision-description` and `sheet.discipline`. The first `sheet.create` of a document writes them into the table; a value is set with `element.param` and sits in the sheet's `props` under the parameter's ID (`"sheet.drawn-by":"text:Cara"`). A title block reads them as it reads any parameter. Users may bind their own parameters to the sheets category in the ordinary way.

## A viewport

A viewport is an element record of the viewports category, with no shape.

| Field | On a viewport record |
| --- | --- |
| `kind` | `viewport` |
| `name` | `Viewport`, always |
| `level`, `placement`, `references` | As on a sheet |
| `recipe` | `{"owner":"construct","schema":1,"op":"viewport","op_version":1,"args":{}}` |

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every viewport | `7400`, the viewports category |
| `@viewport.sheet` | Every viewport | The ID of the sheet it is on |
| `@viewport.view` | Every viewport | The ID of the view it shows |
| `@viewport.x`, `@viewport.y` | Every viewport | The centre of the viewport on the paper, stored length text, from the paper's lower left corner |
| `@viewport.title` | Every viewport | `yesno:yes` or `yesno:no`: whether the view's name shows as the viewport's title |
| `@viewport.align.x`, `@viewport.align.x.anchor` | An aligned viewport | The ID of the viewport it follows across the paper, and `choice:min` (left edges level), `choice:centre` or `choice:max` (right edges level). Written together |
| `@viewport.align.y`, `@viewport.align.y.anchor` | An aligned viewport | The same up the paper (`min` is the bottom edge, `max` the top) |

A viewport has no scale of its own: it takes its view's. It has no size either: its outline is worked from the view's extent and scale each time it is asked ([`Sheets::layout`](src/layout.rs)), so a model that changes does not make a stored outline stale.

```json
{"id":"P-1","document":"DOC-1","kind":"viewport","name":"Viewport","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"viewport","op_version":1,"args":{}},"props":{"@category":"7400","@viewport.align.x":"P-0","@viewport.align.x.anchor":"choice:centre","@viewport.sheet":"S-1","@viewport.title":"yesno:no","@viewport.view":"V-1","@viewport.x":"length:q0:1920000","@viewport.y":"length:q0:1280001"},"references":[]}
```

A record that is not exactly as written is refused, never repaired: a reader names the record and why. The same data is always the same bytes.

## The rules a command checks

- A sheet's number is unique in its document, case aside; a sheet may change the case of its own number.
- Paper is a standard size or a custom size with each side from 50 mm to 5 m. New paper that would leave a viewport's centre off it is refused.
- A view is placed on one sheet only, and once: a second viewport of a placed view is refused, naming the sheet.
- A viewport's centre is on its paper, edges included. (Its outline may run off the paper: layout says so with `on_paper`, and the page may warn; the feature text does not forbid it.)
- An alignment names a viewport of the same sheet, never the viewport itself, and never round a loop; the two axes are independent. A viewport that is aligned on an axis is not placed by hand on it; the alignment is taken away first (`null`), and the position it had is the one stored. A viewport that others follow is not deleted.
- A view that is placed is not deleted. Deleting a sheet deletes its viewports in the same step.
- A viewport keeps its sheet and its view. To move a view to another sheet, delete the viewport and make another.

These bind what commands write; reading a document never applies them, so a document always reads.

## Where a viewport lies

[`Sheets::layout`](src/layout.rs) is given each placed view's scale (1:n) and extent (how wide and high what it shows is, as `Length`s, or none) and returns, for each viewport of a sheet, its centre with alignments worked out and its outline. Exactly: the outline's half width is the extent over twice `n`, rounded **up** to a whole grid step, about the centre, so the outline never holds less than the drawing; an aligned viewport's centre is the followed viewport's anchor less this one's offset to the same anchor, in whole steps; a view with no extent is a point (no outline, no width in an alignment). Because alignment is stored as data and the position worked from it, it holds when a view's extent or scale changes.

## Through the edit log

The commands are construct-api's [`cloud::sheets`](../construct-api/src/cloud/sheets.rs), listed with their arguments in the [cloud format](../construct-model/FORMAT.md): `sheet.create`, `sheet.set`, `sheet.delete`, `viewport.create`, `viewport.set`, `viewport.align` and `viewport.delete`. Each is one undo step, runs behind `model.edit`, takes the claim on every piece it writes and refuses in this crate's own words. The element commands refuse a sheet or a viewport, naming these. An undo that would leave a viewport on a view that is gone, a number taken twice or an alignment to nothing is turned back.

The server reads them at `GET /api/projects/<project>/models/<model>/sheets` (a JSON list of `{id, number, name, paper, orientation, width, height, shown, viewports}`, by number) and `.../sheets/<sheet>` (`{sheet, viewports: [{id, view, view_name, x, y, scale, title, align_x, align_y, outline, on_paper}]}`), behind `model.read`. A plan's extent for the outline is the box of the elements it cuts and sees, in whole millimetres; the other kinds of view have none yet, so their viewports have no outline.

## Later versions

Guide grids, saved positions and crop regions, sheet collections, revisions and a title block record will each be more reserved keys or records, read strictly as these are; version 1 records read with none.
