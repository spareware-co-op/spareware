# Spare Ride status

**Stage: Mapping.** Map: [ARCHITECTURE.md](ARCHITECTURE.md). Runs on the shared marketplace loop in the kit (root [ARCHITECTURE.md](../../ARCHITECTURE.md), "The marketplace pattern"). No code yet.

## Work board

Sizes: Small fits a little spare, Medium a fair amount, Large a lot. Who has a piece and its state live on its issue; check pieces out with `python tools/piece.py checkout <ID>` (see AGENTS.md).

| ID | Piece | Kind | Size | Done when | Issue |
| --- | --- | --- | --- | --- | --- |
| RD01 | Licences for drivers and for the platform in NZ | Research note | Medium | Every licence, check and record the law needs, with sources; flags what needs a lawyer | [#46](https://github.com/spareware-co-op/spareware/issues/46) |
| RD02 | Insurance for passengers, drivers and the co-op | Research note | Medium | Options and costs from insurers who cover ride-hailing in NZ | [#47](https://github.com/spareware-co-op/spareware/issues/47) |
| RD03 | Routing engine: OSRM, Valhalla or GraphHopper | Decision note | Medium | Compared on travel-time accuracy, speed, licence and running cost on NZ map data | [#48](https://github.com/spareware-co-op/spareware/issues/48) |
| RD04 | Driver co-ops that already exist | Research note | Medium | What worked, what failed and why, with current status and sources | [#49](https://github.com/spareware-co-op/spareware/issues/49) |
| RD05 | First city | Decision note | Small | Cities ranked on riders, drivers, competition and rules | [#50](https://github.com/spareware-co-op/spareware/issues/50) |
| RD06 | Fare formula and the drivers' vote | Design note | Small | How the formula is set, shown and changed, with worked examples | [#51](https://github.com/spareware-co-op/spareware/issues/51) |