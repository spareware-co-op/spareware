# A report's record, format 1

A report of a problem, or feedback, as Spare Construct's page sends it and the co-op's private reports store keeps it (note [A100](../../notes/A100-report-a-problem.md)). Published: a record written in this shape stays readable for ever, so a change to it is a new format version, never an edit. `src/report.rs` and `src/stored.rs` are the reference for code.

Every object is closed: a field not named here is refused when the record is read. A part the person left out is not written at all.

## `Report`

| Field | Type | Holds |
| --- | --- | --- |
| `format` | number | `1` |
| `kind` | `"problem"` or `"feedback"` | |
| `summary` | text, one line, 1 to 160 characters | What a fixer reads first |
| `words` | text, up to 4000 characters | The person's own words |
| `topic` | `"modelling"`, `"views"`, `"data"`, `"sharing"`, `"speed"`, `"words"` or `"other"` | Feedback only, and feedback always |
| `failure` | `Failure` | What went wrong; never in feedback |
| `where` | `{ "project": id, "model": id }` | Present whenever `elements` or `edits` are |
| `elements` | list of `{ "id": id, "record": object }`, at most 50, each id once | The elements the failure is about and those they read, each record as the model stores it |
| `edits` | list of `Step`, at most 500, in log order | The model's edits that touched those elements |
| `program` | `{ "version": text, "engine": text }` | The server's version; the model's engine and its version (`construct.stand-in 1`) |
| `browser` | text, one line, up to 300 characters | The browser, as it names itself |
| `page_errors` | list of text, at most 200, each one line of up to 500 characters | The page's own error lines |
| `server_log` | list of text, as `page_errors` | The server's lines for the sender's own requests, each starting with its request ID |
| `screenshot` | `{ "media": "image/png" or "image/jpeg", "data": base64 }` | A picture of at most 1 MB, which its first bytes show is what `media` says |

Without its screenshot, the record's JSON is at most 256 KB.

### `Failure`

| Field | Type | Holds |
| --- | --- | --- |
| `code` | text: a source, a dot, a name of letters, digits, `.`, `_` and `-`; at most 100 | `refused.<command>` (the server turned the command back), `build.<code>` (the builder's code, construct-model's `GeometryError`), `mismatch.body` (a body built in the browser differs from the server's checkpoint), `server.failed` (the server answered "Something went wrong"), `page.error` |
| `words` | text, one line, up to 500 characters | The words the person was shown |
| `element` | id, optional | The element it is about |
| `request` | text, optional, up to 64 | The server's ID of the request it happened in |

### `Step`

| Field | Type | Holds |
| --- | --- | --- |
| `seq` | number | Its number in the model's log; each step's is above the one before |
| `by` | `"sender"` or `"other-<n>"` | Who sent it, as a stand-in: never a name or an account |
| `at_ms` | number | The log's time for it, which a replay runs it at |
| `cmd` | text of `a` to `z`, digits and dots, up to 64 | The command; never `model.`, `package.`, `link.` or `document.` |
| `args` | object | Its arguments, as sent |
| `failed` | text, one line, optional | Why it was turned back then |

### Names

A report names nobody. The sender is `sender` and every other author a stand-in, `other-1`, `other-2`, in the order they first appear in `edits`; a stand-in's name in the words is the stand-in. Any other name the server knows reads `another person`, as a whole word in any case, in `summary`, `words`, the failure's words, the steps' `failed`, `page_errors` and `server_log`. The records and the arguments are the model's inputs and are kept as they are.

## `Stored`, as the reports store keeps it

| Field | Type | Holds |
| --- | --- | --- |
| `number` | number, from 1 | |
| `sender` | id | The sender's account: the only person a stored report is kept with |
| `received_ms` | number | When the server received it, by its clock |
| `state` | `"received"`, `"reproduced"` or `{ "fixed": { "version": text } }` | Where it stands, as its sender sees it |
| `replay` | `Replay`, optional | What the server's replay found |
| `issue` | text, optional | The public issue it became, by its address |
| `screenshot` | true or false | Whether its screenshot is kept beside it |
| `report` | `Report`, without its screenshot | |

`Replay`: `engine` (what it was replayed on), `reproduced` (true when it ended in the report's failure, same code and same words), `code` and `words` (the failure it ended in, if any), `steps` (each edit's `seq`, `cmd`, `then` and `now`: why it was turned back when it was made and in the replay, when it was), `note` (anything a fixer should know).

## Where it is kept

Under the store's root `rpt`, apart from every project (`p:`) and the directory (`dir`):

| Key | Value |
| --- | --- |
| `rpt/next` | The last number given |
| `rpt/report/<number, 20 digits>` | The `Stored` report |
| `rpt/shot/<number, 20 digits>` | Its `Screenshot`, when it has one |
| `rpt/by/<account>/<number, 20 digits>` | The number, for the sender's own list |
