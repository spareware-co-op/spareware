# Shared kit status

**The geometry kernel is owner-built for now.** An owner is building the Spare Kit geometry kernel (`projects/engines/geometry`, pieces G01 onwards) and is not taking kernel contributions until it asks for feedback. Please don't check out, open or work on kernel pieces; the pieces below are open as usual. Follow the kernel on [issue #169](https://github.com/spareware-co-op/spareware/issues/169).

**Stage: Mapping.** Map: [ARCHITECTURE.md](ARCHITECTURE.md), one level deeper than the overview in the root [ARCHITECTURE.md](../../ARCHITECTURE.md). No code yet.

## Work board

Sizes: Small fits a little spare, Medium a fair amount, Large a lot. Who has a piece and its state live on its issue; check pieces out with `python tools/piece.py checkout <ID>` (see AGENTS.md).

| ID | Piece | Kind | Size | Done when | Issue |
| --- | --- | --- | --- | --- | --- |
| K01 | Legal entity: NZ co-operative company, incorporated society or trust | Research note | Medium | Each option compared on member ownership, voting, tax and set-up cost, with sources; flags what needs a lawyer | [#10](https://github.com/spareware-co-op/spareware/issues/10) · [note](notes/K01-legal-entity.md) |
| K02 | Payments at cost: payment providers that pay out to workers in NZ | Research note | Medium | Providers compared on fees, payouts, NZ support and what Spareware must never handle itself | [#11](https://github.com/spareware-co-op/spareware/issues/11) |
| K03 | Study platform co-ops that already exist (rides, couriers, holiday rentals) | Research note | Medium | What worked, what failed and why, with sources and current status | [#12](https://github.com/spareware-co-op/spareware/issues/12) |
| K04 | Phone apps: web apps, Tauri mobile or native | Decision note | Medium | Recommendation covering live location, push notifications and app store rules | [#13](https://github.com/spareware-co-op/spareware/issues/13) |
| K05 | First marketplace to build | Decision note | Small | Ranking of Ride, Food, Work and others on demand, regulation and effort | [#14](https://github.com/spareware-co-op/spareware/issues/14) |
| K06 | Open cost ledger: what it publishes and how it is checked | Design note | Small | A sample month's ledger and who signs it off | [#15](https://github.com/spareware-co-op/spareware/issues/15) |