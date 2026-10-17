# Spare Ride: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Rides where drivers keep everything except the at-cost fee, riders see exactly where every dollar goes, and drivers own the app and vote on its fares and rules. It runs on the shared marketplace loop in the kit; this map covers only what rides add.

## Goals

- Drivers earn more per trip than on Uber, with the fee published and voted on.
- Riders see an upfront fare and its breakdown: driver, running cost, payment fee.
- No penalties for declining a trip, no hidden ranking of drivers.
- Safety and accessibility at least as good as the incumbents.

## What it uses from the shared kit

Accounts and ID checks (driver licence and vehicle checks), payments, matching and dispatch, maps and live location, messaging, ratings, disputes and safety, member voting, the open cost ledger, and the marketplace engine.

## Its own parts

### Rider app

**High level:** request a ride, see the upfront fare, watch the car arrive, share the trip with someone, pay, rate.

**Low level:** fare estimate from the route's distance and time using the fare formula drivers voted on; live car position over a WebSocket; trip sharing as a read-only link.

**Discuss:** scheduled rides; cash; rides for someone else.

### Driver app

**High level:** go online, accept or decline with no penalty, navigate, see each trip's earnings and fee before accepting, get paid out daily.

**Low level:** location sent every few seconds while online; turn-by-turn from an open routing engine; offline-tolerant trip state so a dropped signal doesn't lose a trip.

**Discuss:** instant payout cost; driver-set areas and hours.

### Dispatch

**High level:** offer each request to the nearest available driver by road time, not straight-line distance.

**Low level:** a hexagonal grid index of live drivers (H3, Apache-2.0, is the proven method); road travel times from an open routing engine (OSRM is BSD-2-Clause, Valhalla is MIT, GraphHopper is Apache-2.0) on OpenStreetMap data (ODbL, which needs attribution and share-alike for the map data); offers time out and pass to the next driver.

**Discuss:** batching shared rides; airport and event queues.

### Pricing

**High level:** a published fare formula (base, per kilometre, per minute) set by driver vote. Any busy-time pricing is voted on and shown before booking.

**Low level:** formula versions stored with every trip, so any fare can be explained later.

**Discuss:** busy-time pricing at all, or fixed fares only.

### Safety and accessibility

**High level:** driver and vehicle checks, an in-trip emergency button, trip sharing, incident reports, wheelchair-accessible vehicles and assistance animals.

**Discuss:** recording of trips; women-driver options.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Busy-time pricing | None; capped; voted multiplier | Voted, capped and shown upfront |
| Routing engine | OSRM; Valhalla; GraphHopper | Research first (piece RD03) |
| First city | Auckland; Wellington; Christchurch; a smaller city | Research first (piece RD05) |
| Taxis | Invite existing taxi companies' drivers; drivers only | Both |

The work board is [STATUS.md](STATUS.md).
