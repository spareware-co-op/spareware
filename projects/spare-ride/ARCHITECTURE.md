# Spare Ride: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Rides where drivers keep everything except the at-cost fee, riders see exactly where every dollar goes, and drivers own the app and vote on its fares and rules. It runs on the shared marketplace loop in the kit; this map covers only what rides add.

## Goals

- Drivers earn more per trip than on Uber, with the fee published and voted on.
- Riders see an upfront fare and its breakdown: driver, running cost, payment fee.
- No penalties for declining a trip, no hidden ranking of drivers.
- Safety and accessibility at least as good as the incumbents.

## What it uses from Spare Kit

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

## What it's up against and the rules (researched 2026-10-18)

Not legal advice. Items marked *unverified* need checking before anyone relies on them.

| Topic | Finding | Source |
| --- | --- | --- |
| Uber's cut | Uber's NZ driver guide illustrates a 28% service fee on the fare (incl. GST), plus a separate rider fee; the current real rate is *unverified* | [Uber NZ](https://www.uber.com/nz/en/blog/understanding-earnings-post-1-april-2024/) |
| Platform licence | Anyone running a driver-passenger app needs a small passenger service licence; NZTA checks the people in control are fit and proper; a NZ-resident representative is required | [NZTA](https://nzta.govt.nz/commercial-driving/transport-service-licences/small-passenger-service/) |
| Drivers | A P endorsement (full licence held 2 years, medical, fit and proper) and a vehicle with a Certificate of Fitness | [Consumer Protection](https://consumerprotection.govt.nz/help-product-service/cars/taxis-and-other-ride-share-services) |
| Employee or contractor | The Supreme Court held Uber drivers are employees (2025 NZSC 162). The Employment Relations Amendment Act 2026 adds a five-part gateway test for contractors; whether app drivers pass it is *unverified* | [Simpson Grierson](https://www.simpsongrierson.com/insights-news/legal-updates/from-rideshare-to-rights-the-supreme-courts-uber-decision), [Lane Neave](https://www.laneneave.co.nz/news-events/major-reform-to-employment-law-the-employment-relations-amendment-act-2026/) |
| GST | Ride-sharing is a "listed service": the platform collects 15% GST | [IRD](https://www.ird.govt.nz/sharing-economy/online-marketplaces/gst-on-listed-services/gst-obligations-for-marketplaces) |
| Learn from | The Drivers Cooperative (New York, Denver) is operating; Namma Yatri (India) runs zero-commission and its code is AGPL, so we learn from it but never copy it | [Drivers Cooperative](https://drivers.coop/), [Namma Yatri](https://github.com/nammayatri/nammayatri) |
| Hardest problems | Drivers' employment status; enough riders and drivers at once; insurance and ACC (*unverified*) | |

**What this changes:** the co-op itself needs the small passenger service licence, and the drivers' status is a first-order decision: drivers as worker-members employed by the co-op, or independent contractors meeting the gateway test. Added to the open decisions.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Drivers' status | Worker-members employed by the co-op; independent contractors meeting the 2026 gateway test | Research first, with a lawyer (piece RD01) |
| Busy-time pricing | None; capped; voted multiplier | Voted, capped and shown upfront |
| Routing engine | OSRM; Valhalla; GraphHopper | Research first (piece RD03) |
| First city | Auckland; Wellington; Christchurch; a smaller city | Research first (piece RD05) |
| Taxis | Invite existing taxi companies' drivers; drivers only | Both |

The work board is [STATUS.md](STATUS.md).
