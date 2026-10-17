# Spare Food: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Food delivery where restaurants pay only the at-cost fee instead of a large commission, couriers keep their whole delivery fee, and customers pay menu prices, not marked-up ones. Restaurants and couriers own the app and vote on its rules. It runs on the shared marketplace loop; this map covers only what food adds.

## Goals

- Restaurants keep far more of each order than on the big delivery apps.
- Couriers see the delivery fee and distance before accepting, and keep all of it.
- Menu prices match the restaurant's own prices.
- Pickup orders cost restaurants almost nothing.

## What it uses from the shared kit

Accounts and ID checks, payments, matching and dispatch, maps and live location, messaging, ratings and disputes, member voting, the open cost ledger, and the marketplace engine.

## Its own parts

### Customer app

**High level:** browse nearby restaurants, order for delivery or pickup, track the order, pay, rate.

**Low level:** menus with options and allergens; live order status; courier position on a map once collected.

### Restaurant app

**High level:** receive orders, set prep times, pause when busy, edit menus and prices, see payouts and the fee.

**Low level:** a tablet or browser app with sound alerts; order state as operations (received, accepted, ready, collected); prep-time learning per restaurant.

**Discuss:** printing to kitchen printers; point-of-sale integrations.

### Courier app

**High level:** go online, accept deliveries with the fee shown, navigate, confirm pickup and drop-off, get paid out daily.

**Low level:** the same live location, routing and offline-tolerant job state as Spare Ride.

### Dispatch

**High level:** assign a courier so they arrive as the food is ready, not before.

**Low level:** courier offered the job at (ready time minus travel time); two orders from the same restaurant can be batched; the ETA updates as prep runs late.

**Discuss:** restaurants using their own couriers through the app.

### Pricing

**High level:** customer pays the menu price, a delivery fee that goes to the courier (set by distance and time, voted by couriers), and the at-cost platform fee.

**Discuss:** small-order fees; tipping.

## What it's up against and the rules (researched 2026-10-18)

Not legal advice. Items marked *unverified* need checking before anyone relies on them.

| Topic | Finding | Source |
| --- | --- | --- |
| Uber Eats' cut | 30% with Uber delivery, 16% with the restaurant's own staff, 8% for pickup | [Uber Eats NZ](https://merchants.ubereats.com/nz/en/pricing/) |
| Delivereasy | About 20% (2020-21 figure; current *unverified*) | [Re: News](https://renews.co.nz/restaurants-are-begging-us-not-to-order-through-delivery-companies/) |
| Food safety | The Food Act 2014 duties sit with the kitchens (registration, safe and suitable food); whether a pure software platform must register is *unverified* | [MPI](https://www.mpi.govt.nz/dmsdocument/3634/send) |
| GST | Food delivery is a "listed service": the platform collects GST | [IRD](https://www.ird.govt.nz/sharing-economy/online-marketplaces/gst-on-listed-services/gst-obligations-for-marketplaces) |
| Couriers | The same employee-or-contractor law as Spare Ride applies | See Spare Ride |
| Learn from | CoopCycle's software uses the "Coopyleft" licence: not open source by the usual definition and limited to worker-owned co-ops whose couriers are employees. Learn from it only, unless a lawyer clears its terms | [CoopCycle](https://github.com/coopcycle/coopcycle-web), [Coopyleft](https://coopcycle.org/en/coopyleft/) |
| Hardest problems | Enough restaurants and couriers at once; courier status; a three-way payment split | |

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Couriers' status | Worker-members employed by the co-op; independent contractors meeting the 2026 gateway test | Research first, with a lawyer (piece FD02) |
| Courier pay | Per job by distance and time; hourly guarantee | Per job, voted, with a published minimum |
| Pickup orders | Free to restaurants; tiny fee | Payment cost only |
| Shared couriers with Spare Ride | Same drivers can do both; separate | Same people, one account |

The work board is [STATUS.md](STATUS.md).
