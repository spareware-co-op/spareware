# Spare Rent: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Long-term renting where landlords can self-manage with good tools, tenants apply once and reuse it, and any property-manager work is priced at cost. Landlords and tenants are both members and both vote. It runs on the shared marketplace loop; this map covers only what renting adds.

## Goals

- Landlords self-manage legally without a percentage management fee.
- Tenants apply once, control their own information, and never pay fees the law doesn't allow.
- Maintenance requests go straight to trades on Spare Work.
- Legal records (bond, Healthy Homes, inspections) are kept in order automatically.

## What it uses from Spare Kit

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

## What it's up against and the rules (researched 2026-10-18)

Not legal advice. Items marked *unverified* need checking before anyone relies on them.

| Topic | Finding | Source |
| --- | --- | --- |
| Property managers' cut | Typically 7-10% of rent plus GST, plus about a week's rent as a letting fee charged to the landlord; tenants can't be charged letting fees | [Property Brokers](https://www.propertybrokers.co.nz/news/how-much-does-property-management-cost-in-new-zealand) |
| Bonds | Capped at 4 weeks' rent and lodged with Tenancy Services within 23 working days; no other security allowed. **The platform never holds bonds** | [Tenancy Services](https://tenancy.govt.nz/rent-bond-and-bills/bond), [lodging](https://www.tenancy.govt.nz/rent-bond-and-bills/bond/lodging-a-bond/about-lodging-a-bond) |
| Ending tenancies | Since 30 January 2025: 90 days' notice without cause, 42 days for stated reasons, 21 days from tenants | [Tenancy Services](https://www.tenancy.govt.nz/about-tenancy-services/news/changes-to-tenancy-terminations-now-in-force/) |
| Healthy Homes | Heating, insulation, ventilation, moisture and drainage, draught stopping | [Tenancy Services](https://www.tenancy.govt.nz/healthy-homes/) |
| Property-manager licensing | The bill was dropped in May 2024; nothing newer found (*unverified*) | [Newshub](https://newshub.co.nz/home/politics/2024/05/chl-e-swarbrick-calls-out-government-for-ministerial-overstep-after-it-asks-select-committee-to-drop-bill-regulating-property-managers.html) |
| Hardest problems | Collecting and holding rent (client-money rules *unverified*); Tenancy Tribunal disputes | |

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Rent payments through the app | Yes; no (bank to bank) | Optional; bank to bank by default |
| Co-op property managers | Members offering management at cost; self-manage only | Both |
| Tenant reviews of landlords | Public; private to the co-op | Research first (piece RT04) |

The work board is [STATUS.md](STATUS.md).
