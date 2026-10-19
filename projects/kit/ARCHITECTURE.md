# Shared kit: architecture map

*Stage: Mapping. The overview is in the root [ARCHITECTURE.md](../../ARCHITECTURE.md); this map goes one level deeper into each part. Every choice is open until an owner accepts a decision note.*

The kit is two things: libraries compiled into every app (`crates/`), and services that run once and every app connects to (`services/`). Build a part here once, and every program gets it.

## Libraries (compiled into every app)

| Crate | Job | Key method |
| --- | --- | --- |
| kit-core | Permanent IDs, units, errors, common types | 128-bit time-ordered IDs, so offline work never collides |
| kit-ops | Every change is an operation: commands, transactions, undo, the operation log | One path for people, AI, plugins and sync |
| kit-store | Chunked storage, spatial index, paging from disk | Load only what is in use |
| kit-sync | Sync, history, branches, offline work | Server-ordered operation log; deterministic replay |
| kit-plugin | Sandboxed add-ins | WebAssembly components with declared permissions |
| kit-mcp | AI access | An MCP server over each app's commands |
| kit-market | The marketplace loop | List, match, agree, hold, do, confirm, pay out, rate, dispute |

## Services (run once, shared by every app)

### Accounts

**High level:** one Spareware login for every app. Members own their data and can export or delete it.

**Low level:** standard sign-in protocols (OpenID Connect) so other apps and add-ins can use Spareware logins; passkeys first, passwords as fallback; identity and licence checks through regulated verification providers, storing only the result, not the documents.

**Discuss:** which identity checks each app truly needs.

### Payments

**High level:** customers pay, money is held where the law allows, workers are paid out, and the at-cost fee is taken and shown. A regulated payment provider handles every card and every held dollar; Spareware never holds money itself.

**Low level:** each payment is an operation linked to the job; payouts batch daily; refunds and disputes follow the marketplace loop; fees pass through at cost.

**Researched 2026-10-18 (not legal advice; *unverified* items need checking):**

| Topic | Finding | Source |
| --- | --- | --- |
| GST on listed services | For ride-sharing, food delivery and short stays the platform collects 15% GST and must tell unregistered sellers their flat-rate credits at least monthly. The payments service needs a GST engine | [IRD](https://www.ird.govt.nz/sharing-economy/online-marketplaces/gst-on-listed-services/gst-obligations-for-marketplaces) |
| Reporting income | Platforms report sellers' income to IRD (since 1 January 2024) | [IRD](https://ird.govt.nz/international-tax/exchange-of-information/digital-platform-information) |
| Licensing to hold money | NZ has no general payments licence: Financial Service Providers Register registration plus anti-money-laundering law. Whether holding funds makes the co-op a reporting entity is *unverified*. Safest design: a regulated payment provider holds the money | [Interest.co.nz](https://interest.co.nz/opinion/79619/plugging-anti-money-laundering-loophole-should-be-key-plank-combating-misuse) |
| Anti-money-laundering | Online marketplaces must comply since 1 June 2025; DIA has been the sole supervisor since 1 July 2026 | [DIA](https://www.dia.govt.nz/AML-CFT-Information-for-Online-Marketplaces), [Simpson Grierson](https://www.simpsongrierson.com/insights-news/legal-updates/dia-takes-the-reins-what-reporting-entities-need-to-know-about-the-new-amlcft-guidance-package) |
| Stripe Connect (NZ) | Cards 2.7% + 30c domestic, 3.7% + 30c international; payouts free when Stripe handles pricing, or NZ$2 per active account plus 0.25% + NZ$0.25 per payout when the platform does | [Stripe](https://stripe.com/en-nz/connect/pricing) |
| Bank payments | Open banking went live with the big banks on 1 December 2025; Akahu's payment endpoint is limited to approved apps paying into their own accounts; Windcave's marketplace payouts are *unverified* | [Akahu](https://developers.akahu.nz/docs/making-a-payment) |

**Lean:** a regulated provider (Stripe Connect is the proven option) holds and splits every payment; the kit adds the GST engine, IRD reporting and the at-cost fee. Piece K02 confirms this with a lawyer.

### Matching and dispatch

**High level:** one engine pairs requests with offers. Each app supplies its own rules: nearest driver, ready-timed courier, open dates, best quote.

**Low level:** a live geospatial index (hexagonal grid), scoring rules as plugins per app, offers that time out and pass on.

### Maps and location

**High level:** addresses, search by place, routing, live location and maps, all on open data so no app depends on a paid map provider.

**Low level:** OpenStreetMap data (ODbL: attribution and share-alike for the data); map display with MapLibre (BSD-3-Clause); routing with an open engine (piece RD03); address search with an open geocoder whose licence suits the core (Pelias is MIT; Nominatim is GPL, so it would run only as a separate service).

### Messaging and notifications

**High level:** in-app chat, push notifications, email and SMS, with each member choosing what reaches them.

**Low level:** phone push goes through Apple's and Google's push services, which every phone app must use; email and SMS through providers paid at cost.

### Trust and safety

**High level:** two-way ratings, reports, content labels, member panels for disputes, appeals, and a public log of every removal or ban and the rule behind it.

**Low level:** reports and decisions are operations, so the public log is automatic; tools for detecting known illegal content are shared here (Spare Social piece M07).

### Member voting

**High level:** members propose changes, vote, and see results. Each app's workers vote on that app's fees and rules; co-op-wide matters go to every member.

**Low level:** one member, one vote; eligibility from membership records; proposals have published quorum and timing; results become operations that change the app's settings.

**Waits on:** piece K01, now in review: [the note](notes/K01-legal-entity.md) recommends a NZ co-operative company with one member, one $1 share, one vote.

### Open cost ledger

**High level:** every month, each app publishes what it cost to run and what the fee paid for.

**Low level:** costs pulled from hosting and provider bills, signed off by members chosen for the job, published as open data.

**Waits on:** piece K06.

### Bug reports

**High level:** every app has a "Report a bug" button. The report goes straight to an issue in the Spareware repo on GitHub, with no waiting for anyone to sort it first, so contributors' AI agents can pick it up with spare tokens and fix it fast, all the time. A person still reviews every fix before it merges, as with all work. The reporter needs no GitHub account and can follow the issue to see when it's fixed.

**Low level:**

- One kit library draws the form in every app: what happened, what you expected, the steps, and an optional screenshot. The app adds its name, version and device itself, and, only if the person agrees, the log and the operations leading up to the problem, with personal data stripped (the same consent and stripping as the opt-in crash reports, KIT-A22-F9-P1).
- Apps never hold a GitHub key. They send the report to a small kit service, which files the issue through a GitHub App allowed to create issues and nothing else.
- Each issue is labelled `bug` and with its program, and gets a piece ID the moment it is filed, so `tools/piece.py` checks it out like any other piece and two agents never fix the same bug.
- A report that matches an open bug (the same crash signature or close wording) is added to that issue instead of opening a new one.
- Reporting is also a command, so AI agents and add-ins report bugs through the same path as people.
- Security problems never go to a public issue; they are held privately for an owner.

**To be decided:**

- Whether reporting needs a Spareware account (to stop spam) or is open to anyone.
- Where screenshots and logs live, since they can show private details: on the public issue after stripping, or kept privately and shown only to the agent fixing it.

## Running it

| Concern | Approach |
| --- | --- |
| Hosting | Co-op servers run at cost; any member group can run its own and connect |
| Where data lives | NZ first, with members' data kept in their region where the law asks |
| Backups | Operation logs copied off-site continuously; restores tested on a schedule |
| Security | Least access by default; every admin action is an operation in the log; outside security reviews before each app goes live |
| Monitoring | Open-source monitoring; uptime and costs published with the ledger |

The work board is [STATUS.md](STATUS.md).
