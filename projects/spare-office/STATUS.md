# Spare Office status

**Stage: Mapping.** Map: [ARCHITECTURE.md](ARCHITECTURE.md). No code yet.

## Work board

Sizes: Small fits a little spare, Medium a fair amount, Large a lot. Who has a piece and its state live on its issue; check pieces out with `python tools/piece.py checkout <ID>` (see AGENTS.md).

| ID | Piece | Kind | Size | Done when | Issue |
| --- | --- | --- | --- | --- | --- |
| OF01 | Open office suites compared: ArtCraft, LibreOffice, Collabora, OnlyOffice | Research note | Large | How each is built, its licence, and what we could reuse, partner on or only learn from | [#88](https://github.com/spareware-co-op/spareware/issues/88) |
| OF02 | File formats from the published standards | Research note | Medium | OpenDocument and Office Open XML: specs, patent promises, and the hardest parts to get right | [#89](https://github.com/spareware-co-op/spareware/issues/89) |
| OF03 | Live co-editing for documents and spreadsheets | Design note | Large | How co-editing works on the kit's operation log and text CRDT, with no lost work | [#90](https://github.com/spareware-co-op/spareware/issues/90) |
| OF04 | Mail and calendar on open protocols | Research note | Medium | IMAP, SMTP, JMAP, CalDAV and CardDAV: what to support first and how | [#91](https://github.com/spareware-co-op/spareware/issues/91) |
| OF05 | Chat and video meetings | Decision note | Medium | Matrix and WebRTC compared with building on the kit, including licences | [#92](https://github.com/spareware-co-op/spareware/issues/92) |
| OF06 | Spreadsheet formula engine | Research note | Medium | Which functions matter most and how open engines compute them | [#93](https://github.com/spareware-co-op/spareware/issues/93) |
| OF07 | First app to build | Decision note | Small | Write, Sheets and Mail ranked on demand, effort and what partners already cover | [#94](https://github.com/spareware-co-op/spareware/issues/94) |