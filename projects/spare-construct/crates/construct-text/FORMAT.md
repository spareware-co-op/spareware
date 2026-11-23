# Text styles and text notes, version 1

A published format under [EXTENDING.md](../../../../EXTENDING.md), section 3, read with the [cloud format](../construct-model/FORMAT.md) whose element record it uses unchanged, the [parameter values](../construct-params/FORMAT.md) it stores and the [views](../construct-views/FORMAT.md) it annotates. **We may stop writing an old version. We never stop reading it.** [`src/style.rs`](src/style.rs) and [`src/note.rs`](src/note.rs) are the reader library. A document made before text existed has no record of either kind and reads as having none.

## A text style

A text style is an element record of the text notes category. It has no shape: nothing builds it.

| Field | On a style record |
| --- | --- |
| `id` | The style's permanent ID |
| `document` | The document it belongs to |
| `kind` | `text-style` |
| `name` | The name people see: trimmed, 1 to 64 characters, no control character. Unique in the document without regard to case (a rule commands keep, not one reading applies) |
| `level`, `placement`, `references` | `0`, `{"x":0,"y":0,"z":0}`, `[]` |
| `recipe` | `{"owner":"construct","schema":1,"op":"text-style","op_version":1,"args":{}}`: builds nothing; its `op_version` is this format's version |
| `props` | The entries below, and no other |

| `props` key | Value |
| --- | --- |
| `@category` | `5200`, the text notes category ([construct-categories](../construct-categories/FORMAT.md)) |
| `@text.style.font` | The font's name as text: 1 to 64 characters, trimmed, no control character. It is a name; no font file is stored |
| `@text.style.size` | The size on paper, a stored length above zero and at most 500 mm: the height of the font's em |
| `@text.style.bold`, `@text.style.italic`, `@text.style.underline` | `yesno:yes` or `yesno:no` |
| `@text.style.colour` | `#rrggbb`, lower case hexadecimal |
| `@text.style.spacing` | The line spacing as a whole percentage of the font's own line height, 50 to 300, written as a plain decimal with no leading zero (`100`) |

```json
{"id":"ST-1","document":"DOC-1","kind":"text-style","name":"General","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"text-style","op_version":1,"args":{}},"props":{"@category":"5200","@text.style.bold":"yesno:no","@text.style.colour":"#000000","@text.style.font":"Test Sans","@text.style.italic":"yesno:no","@text.style.size":"length:q0:16000","@text.style.spacing":"100","@text.style.underline":"yesno:no"},"references":[]}
```

## A text note

A text note is an element record of the text notes category, with no shape. It belongs to one view and keeps that view for life.

| Field | On a note record |
| --- | --- |
| `kind` | `text-note` |
| `name` | `Text note`, always |
| `level`, `placement`, `references` | As on a style |
| `recipe` | `{"owner":"construct","schema":1,"op":"text-note","op_version":1,"args":{}}` |

| `props` key | On | Value |
| --- | --- | --- |
| `@category` | Every note | `5200` |
| `@text.view` | Every note | The ID of the view it belongs to |
| `@text.style` | Every note | The ID of the text style it reads. Nothing of the style is copied into the note |
| `@text.x`, `@text.y` | Every note | The note's anchor in the view's own coordinates, stored length text: the top left corner of its box. Beyond a plan's level-relative model coordinates nothing is converted: the number is the view's, in the one length spelling |
| `@text.lines` | Every note | The text: its lines (each a paragraph) joined by one line feed, with no trailing line feed, not empty after trimming, at most 10,000 characters, no control character other than line feed and tab |
| `@text.width` | A wrapping note | The wrap width, a stored length above zero. Absent when the note does not wrap |
| `@text.align` | Every note | `choice:left`, `choice:centre` or `choice:right` |
| `@text.list` | Every note | `choice:none`, `choice:bullet` or `choice:number`: what each line is |
| `@text.rotation` | Every note | A stored angle (construct-params), anticlockwise about the anchor, within one turn either way |
| `@text.leader.<n>` | A note with leaders | One leader, numbered `n` (a plain decimal with no leading zero, from 1): `<arrow>;<x>,<y>;<x>,<y>...` where `arrow` is `none`, `open`, `filled`, `dot` or `slash` (the head on the last point) and each `x` and `y` is stored length text, in the view's coordinates. Two to 32 points; at most 32 leaders |

```json
{"id":"N-1","document":"DOC-1","kind":"text-note","name":"Text note","level":0,"placement":{"x":0,"y":0,"z":0},"recipe":{"owner":"construct","schema":1,"op":"text-note","op_version":1,"args":{}},"props":{"@category":"5200","@text.align":"choice:centre","@text.leader.1":"filled;length:q0:64000,length:q0:128000;length:q0:1,length:q0:-32000","@text.lines":"Line one\nLine two","@text.list":"choice:bullet","@text.rotation":"angle:405000000","@text.style":"ST-1","@text.view":"V-1","@text.width":"length:q0:512000","@text.x":"length:q0:1904000","@text.y":"length:q0:-1600000"},"references":[]}
```

A leader is not tied to an element. It is where it is drawn, in the view's coordinates; deleting an element changes no leader.

A record that is not exactly as written is refused, never repaired: a reader names the record and why. The same data is always the same bytes.

## What is not stored

Nothing about how a note is laid out is stored: not its line breaks, the width of any line, its box or its height. They are worked from the record, the style and font metrics each time ([`lay_out`](src/layout.rs)), so a style change or a font change is never stale and no number a font would decide is held here. Rotation is stored; a note's rotated bounds are not.

## The rules a command checks

- A style's name is not empty and is unique in its document, case aside; a style may change the case of its own name.
- A style's size on paper is above zero and at most 500 mm; its spacing is 50 to 300 percent.
- A note is in a view that is there and reads a style that is there; it keeps its view.
- A note has some text; a wrap width is above zero.
- A leader has two to 32 points; a note has at most 32 leaders; a leader's number is one past the largest the note has.
- A style that notes read is not deleted; the refusal names how many.
- A view that holds notes is not deleted; the refusal names how many.

These bind what commands write; reading a document never applies them, so a document always reads.

## Where a note lies

[`lay_out`](src/layout.rs) is given the note, its style and a `FontMetrics` (how wide a text is in a face, and the font's line height, in thousandths of the size) and returns each line (its marker, text, where its block starts, its indent, its width and its top) and the box. Exactly: a measured width is the metrics' thousandths of the size times the style's size, rounded **up** to a whole grid step, so a box never holds less than the text; a line is the font's line height at the style's size and spacing; where a line may break is the Unicode line breaking algorithm's (UAX #14); a word wider than the width keeps a line of its own and is not split; a list item's marker and one space hang at the left and the continuation lines sit under the first; alignment offsets are whole steps rounded down. The box's top left corner is the anchor and it runs right and down. Rotating it is the drawing's job.

## Through the edit log

The commands are construct-api's [`cloud::text`](../construct-api/src/cloud/text.rs), listed with their arguments in the [cloud format](../construct-model/FORMAT.md): `text.style.create`, `text.style.set`, `text.style.delete`, `text.note.create`, `text.note.text`, `text.note.move`, `text.note.style`, `text.note.layout`, `text.note.delete`, `text.leader.add`, `text.leader.move` and `text.leader.remove`. Each is one undo step, runs behind `model.edit`, takes the claim on every piece it writes and refuses in this crate's own words. The element commands refuse a note or a style, naming these. An undo that would leave a note on a view or a style that is gone, or a style name taken twice, is turned back.

The server reads them at `GET /api/projects/<project>/models/<model>/views/<view>/text` (`{view, styles: [{id, name, font, size, bold, italic, underline, colour, spacing, notes}], notes: [{id, style, x, y, lines, width, align, list, rotation, leaders: [{number, arrow, points}], laid}], laid_out}`), behind `model.read`. `laid` is the layout (`{lines, line_height, width, height, min_x, min_y, max_x, max_y}`) when the server has font metrics, and `null` otherwise, with `laid_out` false: nothing is guessed.

## Later versions

Text on a sheet, a leader bound to an element, rich runs inside a line (a bold word), styles that inherit from one another, and numbering that continues across notes will each be more reserved keys or records, read strictly as these are; version 1 records read with none.
