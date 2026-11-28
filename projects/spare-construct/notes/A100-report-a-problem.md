# A100: Report a problem and send feedback, with the key facts and a replay

**Status, 2026-11-28:** Decided for the build and built by piece A100 (job 13.9 of the Construct plan; part: support). The opt-in counts by code (part 5 below) are designed here and are the next job.

## The question

The owner, 28 Nov 2026: people send logs about geometry or anything, and we fix them as they come in, "with the key bits so an AI does not over-read"; and "we need to sort how people will send bugs and feedback with logs". Main's five parts: (1) Help has Report a problem and Send feedback, and an error the server returns offers Send a report on the spot; (2) what is sent is shown first, each part able to be left out, capped, with no other project data; (3) it goes to the co-op cloud's private reports store, gets a number, and its sender sees its state (received, reproduced, fixed in version N); (4) the server replays the edits into an empty project to reproduce, a fixer reads the one-line summary, the code and the replay first, and a reproduced fault becomes a failing test and a generic public issue with no project data, linked to the private report; feedback is grouped by topic; (5) opt-in counts of refusals and failures by code, with no content.

## The method followed

How well-known open projects shape crash and bug reports, each for the part it does best:

- **The person sees what is sent, part by part.** Mozilla's crash reporter shows the report's details before it is sent, takes a comment, and gives back a crash ID the person can look up later ([Breakpad at Mozilla](https://wiki.mozilla.org/Breakpad), [crash-stats](https://crash-stats.mozilla.org)). Chromium's feedback form has a box for the screenshot and a box for the system information, each with a way to look at what it holds. Here: every part is a box, ticked, with "What is sent" showing its exact JSON.
- **No personal data unless asked.** Sentry's SDKs send no personal data by default (`send_default_pii` is off) and keep a trail of recent actions ("breadcrumbs") beside the error ([Sentry options](https://docs.sentry.io/platforms/javascript/configuration/options/)). Here: a report names its sender only by account, everyone else by a stand-in, and its trail is the model's own ordered log, which replays.
- **A stable signature, so faults group.** Mozilla's crash-stats groups crashes by signature and files bugs by signature. Here: a failure is a stable code, `<source>.<name>` (`refused.element.param`, `build.Unfilled`), from the codes the server already gives: a command's refusal (words today, named by its command), a build failure's code (construct-model's `GeometryError`, kept in `BuiltResult::Failed`), a checkpoint's body mismatch (note A05), and "Something went wrong" (the web layer's answer to a store failure).
- **A reproduction becomes a test that fails until the fix.** The Rust compiler keeps each known internal compiler error as a test in [`tests/crashes`](https://github.com/rust-lang/rust/tree/master/tests/crashes), which passes while the compiler still crashes and fails once it is fixed, so the fix moves it to an ordinary test ([the rustc dev guide on crash tests](https://rustc-dev-guide.rust-lang.org/tests/compiletest.html#crash-tests)). Here: a reproduced report becomes a failing test written by hand from the replay, with its own generic values, never the report's data.
- **Replay from the log.** The server already follows Replicache's and Figma's published method: one ordered edit log per project, which replays to the same bytes (the server's README). A report's edits are a slice of that log, so the same commands replay them.

## The report (crate `construct-report`, format 1)

One closed record, `Report` (`crates/construct-report/FORMAT.md`): serde refuses any field it does not name, so it can hold only what it lists, and each list has a cap.

| Part | What | Cap | Added by |
| --- | --- | --- | --- |
| `summary` | One line, what a fixer reads first | 160 characters | the person (the page fills it from the failure) |
| `words` | The person's own words | 4000 characters | the person |
| `topic` | Feedback's topic: modelling, views, data, sharing, speed, words, other | | the person |
| `failure` | `code`, `words`, `element`, `request` | one line each | the page, or the server's draft for a refusal |
| `where` | The project and model, by ID | | the server's draft |
| `elements` | The elements the failure is about and those they read, each with its record (kind, recipe, inputs) | 50 | the server's draft |
| `edits` | Every edit of the model's log that touched them, in log order: `seq`, `by` (a stand-in), `at_ms` (the log's time), `cmd`, `args`, `failed` | 500 | the server's draft |
| `program` | The server's version and the model's engine | | the server's draft |
| `browser` | The browser, as it names itself | 300 characters | the page |
| `page_errors` | The page's own error lines | 200 lines of 500 characters | the page |
| `server_log` | The server's lines for the sender's own requests | 200 lines of 500 characters | the server's draft |
| `screenshot` | A PNG or JPEG the person chooses | 1 MB | the person |

The record without its screenshot is at most 256 KB. `Report::check` refuses, in words, anything over a cap, a feedback without a topic or with a failure, edits out of log order, an edit by anything but a stand-in, and the commands that are not a model's own (`model.`, `package.`, `link.`, `document.`).

**What a report never holds.** Other project data: the elements are found from the failure outward (its element, what that element's record and reads row name: its type, its levels, its host, what it hosts, the walls it joins, the parameter table; two steps out), and the edits are only those that touched them. Names: the sender is `sender` and every other author `other-1`, `other-2`, in order of first appearance; the server takes each stand-in's name out of the edits' words when it makes the draft (so "Ben is working on this." reads "other-1 is working on this.", which is what the replay says too), and on receipt takes every other name it knows out of every text part, as a whole word in any case ("another person"). The sender's account ID is the only ID of a person it is stored with, and the server's log lines carry account IDs only (a refusal is logged by its counter, never its words, since a claim's words name who holds it).

## The consent screen

Help (its own tab) has Report a problem, Send feedback and Your reports. A failure the core tells the Help tool of (a new registry kind, `failures`: an edit turned back, a request answered "Something went wrong", a body that differs from the server's checkpoint) puts Send a report beside its toast for as long as the toast shows it. The report opens with the server's draft already in it, and lists every part with its box ticked, a line saying what it holds, and its exact JSON under "What is sent"; the screenshot is a file the person adds. Nothing is sent until Send, and an unticked part is not in the record at all. Send feedback is the same screen with a topic, a line, words, the browser and a screenshot.

## Where it is kept

The co-op cloud's private reports store: the same store as the projects, under a root of its own, `rpt/`, which no project's scan reaches and no project's reader names: `rpt/next`, `rpt/report/<number>` (the `Stored` report: number, sender, received time, state, replay, issue), `rpt/shot/<number>` (the screenshot, apart, so a list never reads pictures) and `rpt/by/<account>/<number>` (the sender's list). Every route is behind the sender's sign-in and reads only their own reports; a report about a model is accepted only from someone who may read that model. Never a public page.

| Route | Does |
| --- | --- |
| `POST /api/projects/{project}/models/{model}/report-draft` | The server's parts, for the consent screen |
| `POST /api/reports` | Checks, scrubs, numbers, keeps and replays a report; answers its number and state |
| `GET /api/reports` | The sender's reports with their states |
| `GET /api/reports/{number}` | One of the sender's reports |

## The states

`received` (kept), `reproduced` (the server's replay ended in the report's failure: same code, same words), `fixed in version N` (an operator marks it). The sender sees them in Your reports.

## The replay

`server/src/replay.rs`: the report's edits, in log order, into an empty project, in one transaction of the store that is never committed. It writes the empty project's rows, makes its model with `model.add`, then runs each edit through the same `ProjectCommands` the orderer runs, at the log's own time for it, with its entry written into the transaction as the orderer writes it (so undo, claims and every rule read the replay's own log), and finally fills and builds the failure's element when the failure is a build. Then it rolls back: the kit's `Txn` promises that a transaction dropped without commit leaves nothing behind, and a test checks that not one key outside `rpt/` changes. Access is not what a report is about, so the commands are asked through a decider that says yes. A body mismatch, a server failure and a page error happened outside what the edits decide: the replay says so, and the report's lines are what a fixer reads.

Interim: a host is built on its own in the replay, not with what it hosts (that pass reads committed rows), so a fault only a host's cut shows is not reproduced yet; and the draft reads the model's whole log to find the edits that touched the elements, which job 13.2's history of an element replaces.

## Our side

- **The fixer reads one line first**: `spare-server report line <number>` prints `Stored::line` (number and state, code and words, element IDs, how many edits, what the replay found, the engine and the version), then only the edits whose outcome differed in the replay. `report show` prints the whole record; `report replay` runs the replay again on today's server.
- **A failing test**: written by the fixer from the replay with generic values (new IDs, round numbers, no names), in the crate's own test file, failing until the fix lands, as rustc's crash tests do.
- **A generic public issue**: `spare-server report issue <number>` prints the title and body `public_issue` makes: the code, what happened in general words, how many edits the replay ran and on which engine, the commands by first use, the kinds of element, the version, and the private report's number. No IDs, arguments, records, words or names, and only for a reproduced fault. `server/scripts/report-to-issue.sh <number>` shows it, opens it with `gh issue create` once the operator says yes, and records the link (`report linked`). Run by the co-op's operators only, never on reports in this repository.
- **Fixed**: `spare-server report fixed <number> <version>`, which the sender then sees.
- **Feedback by topic**: `spare-server report feedback` lists every piece of feedback's number under its topic, for the owner to read and turn into plan jobs.

## The server's lines, by request

Every request now gets an ID (tower-http's request-id layers), answered in `x-request-id`, and runs in a span holding it, the method, the path and the signed-in account; the live channel runs in a span of its upgrade request's ID, which the welcome message tells the page. A tracing layer (`server/src/recent.rs`) keeps the last 4096 lines logged in such spans, and the draft gives a person back only the lines of their own requests. The printed log is unchanged (`RUST_LOG`).

## Counts by code (part 5, the next job)

Opt-in, per person (a setting on their account): the page counts its failures by code and the server adds one to `rpt/count/<code>/<day>` once a day per person, nothing else: no IDs, no words, no project. The owner reads the counts beside the reports, so a fault nobody reports still shows. Not built by A100: it needs the account setting (the accounts service) and a daily rollup.

## Packages

| Package | Version | Licence | For |
| --- | --- | --- | --- |
| serde, serde_json | 1.0.229, 1.0.154 | MIT OR Apache-2.0 | the record |
| base64 | 0.22.1 | MIT OR Apache-2.0 | a screenshot's bytes, checked to be a picture |
| regex | 1.13.1 | MIT OR Apache-2.0 | names as whole words, in any case |
| tower-http (`request-id`, `trace`) | 0.6.11 | MIT | request IDs and request lines |
| tracing-subscriber | 0.3.20 (already the server's) | MIT | the layer that keeps the lines |

All pinned by the lock files; none is new to the server's lock but `construct-report`.

## Not decided here

- Whether a report's sender may withdraw it (a delete route), and how long reports are kept.
- Whether the co-op's operators see reports through a page of their own rather than the commands.
