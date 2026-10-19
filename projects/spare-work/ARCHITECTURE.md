# Spare Work: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Trades and freelance work where the worker keeps everything except the at-cost fee, payment for each stage is held safely until the work is done, and licences are checked against the public registers. Workers own the app and vote on its rules. It runs on the shared marketplace loop; this map covers only what work adds.

Spare Work is also the services layer for the other programs: maintenance for Spare Rent, photographers and lawyers for Spare Home, and building trades for designs from Spare Construct.

## Goals

- Customers post a job once and get real quotes; no paying to read leads.
- Trades and freelancers keep everything except the at-cost fee.
- Stage payments are held until each stage is signed off.
- Licences and registrations are checked, not just claimed.

## What it uses from the shared kit

Accounts and ID checks (licence and registration checks), payments (held stage payments), matching, maps, messaging, ratings and disputes, member voting, the open cost ledger, and the marketplace engine.

## Its own parts

### Jobs and quotes

**High level:** customers describe the job with photos (or attach Spare Construct drawings); matching workers quote; the customer accepts one quote.

**Low level:** quotes are versioned operations; a quote becomes stages with amounts and sign-off conditions.

**Discuss:** fixed-price services (for example a building inspection) as listings instead of quotes.

### Stage payments

**High level:** the customer pays each stage into a held account; the worker is paid when the stage is signed off; disputes go to a member panel.

**Low level:** follows payments piece K02 for how money can legally be held.

### Licences and credentials

**High level:** licences and trade registrations are checked against the public registers and shown on profiles with their expiry dates.

**Discuss:** which registers have usable public lookups (piece WK02).

### Freelance and remote work

**High level:** design, writing, software and other remote work with the same quote, stage and payment flow.

### Paperwork

**High level:** GST-correct invoices, receipts and records generated from each job.

## What it's up against and the rules (researched 2026-10-18)

Not legal advice. Items marked *unverified* need checking before anyone relies on them.

| Topic | Finding | Source |
| --- | --- | --- |
| Builderscrack | Owned by hipages since 2021; trades pay $79, $99 or $149 plus GST a month for lead tokens | [SmartCompany](https://www.smartcompany.com.au/finance/mergers-and-acquisitions/hipages-acquires-new-zealand-tradie-platform-builderscrack/), [plans](https://builderscrack.co.nz/subscription) |
| Upwork and Fiverr | Reportedly 0-15% from freelancers plus about 5% from clients (Upwork) and 20% from sellers plus 5.5% from buyers (Fiverr); secondary sources | [Golance](https://golance.com/blogs/upwork-fees-explained-2026), [VaultLeap](https://vaultleap.com/blog/fiverr-fees-explained-2026) |
| Licensed work | Restricted building work must be done or supervised by a licensed building practitioner (Building Act 2004); electrical, plumbing and gas registration and the Construction Contracts Act are *unverified* | [Hutt City](https://www.huttcity.govt.nz/property-and-building/building-consents/before-you-start/restricted-building-work-that-needs-to-be-done-by-professionals) |
| Employee or contractor | The 2026 gateway test applies (see Spare Ride) | See Spare Ride |
| Learn from | Up & Go cleaning co-op (worker keeps 95 cents in the dollar); Loconomics (MPL-2.0, archived January 2026); Stocksy photo co-op (artists get 50-75% of sales) | [Up & Go](https://upandgo.coop/), [Loconomics](https://github.com/loconomics/loconomics), [Stocksy](https://www.stocksy.com/ideas/?p=6290) |
| Hardest problems | Vetting and licence checks; customers and trades going around the platform after the first job | |

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Leads | Free to quote; paid leads | Free to quote, always |
| Held payments | Every job; optional | Optional, default on for jobs over a set size |
| Remote work | First release; later | Later |

The work board is [STATUS.md](STATUS.md).
