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

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Courier pay | Per job by distance and time; hourly guarantee | Per job, voted, with a published minimum |
| Pickup orders | Free to restaurants; tiny fee | Payment cost only |
| Shared couriers with Spare Ride | Same drivers can do both; separate | Same people, one account |

The work board is [STATUS.md](STATUS.md).
