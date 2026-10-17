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

**Waits on:** piece K02 (which providers support NZ marketplace payouts and what the law requires for held money).

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

**Waits on:** piece K01 (the legal entity sets what voting must look like).

### Open cost ledger

**High level:** every month, each app publishes what it cost to run and what the fee paid for.

**Low level:** costs pulled from hosting and provider bills, signed off by members chosen for the job, published as open data.

**Waits on:** piece K06.

## Running it

| Concern | Approach |
| --- | --- |
| Hosting | Co-op servers run at cost; any member group can run its own and connect |
| Where data lives | NZ first, with members' data kept in their region where the law asks |
| Backups | Operation logs copied off-site continuously; restores tested on a schedule |
| Security | Least access by default; every admin action is an operation in the log; outside security reviews before each app goes live |
| Monitoring | Open-source monitoring; uptime and costs published with the ledger |

The work board is [STATUS.md](STATUS.md).
