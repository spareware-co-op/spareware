# Spare Rent: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Long-term renting where landlords can self-manage with good tools, tenants apply once and reuse it, and any property-manager work is priced at cost. Landlords and tenants are both members and both vote. It runs on the shared marketplace loop; this map covers only what renting adds.

## Goals

- Landlords self-manage legally without a percentage management fee.
- Tenants apply once, control their own information, and never pay fees the law doesn't allow.
- Maintenance requests go straight to trades on Spare Work.
- Legal records (bond, Healthy Homes, inspections) are kept in order automatically.

## What it uses from the shared kit

Accounts and ID checks, payments (rent, if both sides choose), maps, messaging, ratings, member voting, the open cost ledger, and the marketplace engine. Maintenance jobs go to Spare Work.

## Its own parts

### Listings and applications

**High level:** landlords list with photos and details; tenants keep one reusable application they control; viewings are booked in the app.

**Low level:** the tenant's application is shared per listing with their consent and only the information the law allows a landlord to ask for (piece RT02).

### The tenancy

**High level:** a tenancy agreement from the official templates, bond lodgement steps, rent schedule and receipts, inspections with dated photos, notices with the right legal timeframes.

**Low level:** every notice is an operation with its legal clock, so deadlines can't be missed; documents are stored for the times the law requires.

### Maintenance

**High level:** tenants report issues with photos; the landlord approves; a trade is booked through Spare Work; the history stays with the property.

### Healthy homes and compliance

**High level:** a checklist and evidence store for the rental standards the law requires.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Rent payments through the app | Yes; no (bank to bank) | Optional; bank to bank by default |
| Co-op property managers | Members offering management at cost; self-manage only | Both |
| Tenant reviews of landlords | Public; private to the co-op | Research first (piece RT04) |

The work board is [STATUS.md](STATUS.md).
