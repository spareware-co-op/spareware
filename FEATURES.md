# Spareware feature maps

*Stage: Mapping. Everything here is open for discussion until an owner accepts a decision note. Built from each program's features.json on 2026-10-19; each program's own map has the detail.*

Spareware is free software for everyone, built together with the spare weekly AI tokens contributors don't finish. It is international: every app is for anyone, anywhere. On the apps where people earn a living, the workers keep all of the profit except the running costs. Workers vote on their own app's fees and rules, and major changes go to a vote of members as a poll in Discussions. Every app is practical and easy to use.

Each program has a feature map, read top-down: areas, then what a person can do in each, then the buildable parts, each marked **core** (needed for a first usable release), **next** or **later**. Every program is built on the shared kit and the shared engines, and no program depends on another. How they fit together is in [ARCHITECTURE.md](ARCHITECTURE.md); the work boards are in [STATUS.md](STATUS.md).

## Every program

| Program | Replaces | Areas | Features | Parts | Core parts | Feature map | Demo |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Shared kit and co-op portal, with the shared engines | The parts every platform rebuilds or rents | 28 | 369 | 765 | 656 | [projects/kit/FEATURES.md](projects/kit/FEATURES.md) | [Demo](https://spareware.app/kit/) |
| Spare Construct | Revit, and the add-ins people buy for it | 24 | 194 | 639 | 340 | [projects/spare-construct/FEATURES.md](projects/spare-construct/FEATURES.md) | [Demo](https://spareware.app/spare-construct/) |
| Spare Studio (working name) | Adobe's apps | 21 | 202 | 562 | 320 | [projects/spare-studio/FEATURES.md](projects/spare-studio/FEATURES.md) | [Demo](https://spareware.app/spare-studio/) |
| Spare Social (working name) | TikTok, YouTube, Instagram and Facebook | 23 | 199 | 525 | 345 | [projects/spare-social/FEATURES.md](projects/spare-social/FEATURES.md) | [Demo](https://spareware.app/spare-social/) |
| Spare Office | Microsoft 365 | 21 | 275 | 641 | 416 | [projects/spare-office/FEATURES.md](projects/spare-office/FEATURES.md) | [Demo](https://spareware.app/spare-office/) |
| Spare OS | Paying for Windows | 21 | 193 | 450 | 297 | [projects/spare-os/FEATURES.md](projects/spare-os/FEATURES.md) | [Demo](https://spareware.app/spare-os/) |
| Spare Ride | Uber | 23 | 250 | 482 | 309 | [projects/spare-ride/FEATURES.md](projects/spare-ride/FEATURES.md) | [Demo](https://spareware.app/spare-ride/) |
| Spare Food | Uber Eats, DoorDash | 20 | 193 | 448 | 328 | [projects/spare-food/FEATURES.md](projects/spare-food/FEATURES.md) | [Demo](https://spareware.app/spare-food/) |
| Spare Stay | Airbnb, Booking.com | 22 | 168 | 341 | 208 | [projects/spare-stay/FEATURES.md](projects/spare-stay/FEATURES.md) | [Demo](https://spareware.app/spare-stay/) |
| Spare Home | Agent commissions and listing fees when selling a home | 19 | 164 | 372 | 271 | [projects/spare-home/FEATURES.md](projects/spare-home/FEATURES.md) | [Demo](https://spareware.app/spare-home/) |
| Spare Rent | Property manager and listing fees | 22 | 191 | 456 | 330 | [projects/spare-rent/FEATURES.md](projects/spare-rent/FEATURES.md) | [Demo](https://spareware.app/spare-rent/) |
| Spare Work | Trades and freelance marketplaces | 22 | 153 | 397 | 280 | [projects/spare-work/FEATURES.md](projects/spare-work/FEATURES.md) | [Demo](https://spareware.app/spare-work/) |
| Spare Market | Trade Me, eBay, Etsy, Shopify | 22 | 177 | 388 | 157 | [projects/spare-market/FEATURES.md](projects/spare-market/FEATURES.md) | [Demo](https://spareware.app/spare-market/) |
| **All maps** |  | **288** | **2728** | **6466** | **4257** |  |  |

Core parts are the parts of core features. The shared kit row includes the shared engines (KIT-A27), which are built in `projects/engines` and mapped in the kit's map so every program has one place to depend on. The demos are published on the [Spareware site](https://spareware.app/).

## What each program takes from the shared kit

Each number is how many of that program's parts depend on that kit area, either by part ID or by the crate, service or engine the area builds. A blank means none. The maps were checked on 2026-10-19: every dependency resolves, and no program depends on another program.

| Kit area | Construct | Studio | Social | Office | OS | Ride | Food | Stay | Home | Rent | Work | Market | Programs |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [KIT-A1 Accounts and sign-in](projects/kit/FEATURES.md#kit-a1-accounts-and-sign-in) | 7 | 16 | 25 | 31 | 22 | 16 | 24 | 15 | 15 | 11 | 17 | 17 | 12 |
| [KIT-A2 Identity, licence and business checks](projects/kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) | 1 | 1 | 9 | 3 | 4 | 20 | 22 | 12 | 16 | 11 | 20 | 11 | 12 |
| [KIT-A3 Payments and payouts](projects/kit/FEATURES.md#kit-a3-payments-and-payouts) | 6 | 5 | 18 | 13 | 8 | 29 | 37 | 37 | 6 | 13 | 37 | 55 | 12 |
| [KIT-A4 Tax, invoices and money law](projects/kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) | 4 | 3 | 7 | 4 | 4 | 17 | 22 | 16 | 4 | 11 | 15 | 19 | 12 |
| [KIT-A5 Marketplace engine](projects/kit/FEATURES.md#kit-a5-marketplace-engine) |  | 1 | 6 | 3 |  | 21 | 33 | 37 | 18 | 14 | 33 | 51 | 10 |
| [KIT-A6 Search, matching and dispatch](projects/kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |  | 4 | 2 | 1 | 19 | 20 | 6 | 6 | 8 | 13 | 9 | 10 |
| [KIT-A7 Maps, places and live location](projects/kit/FEATURES.md#kit-a7-maps-places-and-live-location) | 3 | 1 | 7 |  | 3 | 28 | 25 | 8 | 13 | 14 | 8 | 12 | 11 |
| [KIT-A8 Messaging and notifications](projects/kit/FEATURES.md#kit-a8-messaging-and-notifications) | 6 | 4 | 19 | 37 | 4 | 22 | 16 | 14 | 22 | 32 | 24 | 22 | 12 |
| [KIT-A9 Ratings and reputation](projects/kit/FEATURES.md#kit-a9-ratings-and-reputation) |  |  |  |  |  | 7 | 10 | 9 | 3 | 6 | 10 | 7 | 7 |
| [KIT-A10 Safety, reports and moderation](projects/kit/FEATURES.md#kit-a10-safety-reports-and-moderation) | 4 | 6 | 27 | 19 | 14 | 32 | 25 | 29 | 10 | 19 | 24 | 15 | 12 |
| [KIT-A11 Disputes and appeals](projects/kit/FEATURES.md#kit-a11-disputes-and-appeals) | 2 |  | 5 | 2 | 2 | 12 | 9 | 18 | 2 | 5 | 14 | 12 | 11 |
| [KIT-A12 Member voting and meetings](projects/kit/FEATURES.md#kit-a12-member-voting-and-meetings) | 5 | 6 | 9 | 7 | 5 | 14 | 23 | 15 | 1 | 11 | 10 | 10 | 12 |
| [KIT-A13 Open cost ledger](projects/kit/FEATURES.md#kit-a13-open-cost-ledger) | 6 | 4 | 4 | 7 | 10 | 8 | 9 | 7 | 8 | 6 | 3 | 9 | 12 |
| [KIT-A14 Co-op membership and member portal](projects/kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal) | 1 | 3 | 3 |  | 8 | 11 | 5 | 7 | 2 | 4 | 4 | 4 | 11 |
| [KIT-A15 Sync, history and storage](projects/kit/FEATURES.md#kit-a15-sync-history-and-storage) | 36 | 57 | 48 | 80 | 14 | 31 | 27 | 19 | 50 | 45 | 24 | 34 | 12 |
| [KIT-A16 Plugins, open API and integrations](projects/kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations) | 4 | 15 | 10 | 11 | 5 | 9 | 11 | 8 | 7 | 5 | 17 | 14 | 12 |
| [KIT-A17 AI access](projects/kit/FEATURES.md#kit-a17-ai-access) | 12 | 8 | 13 | 10 | 13 | 11 | 13 | 13 | 13 | 17 | 12 | 11 | 12 |
| [KIT-A18 Privacy, data rights and legal duties](projects/kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) | 8 | 11 | 16 | 20 | 17 | 15 | 20 | 23 | 11 | 16 | 16 | 33 | 12 |
| [KIT-A19 Languages, accessibility and countries](projects/kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) | 8 | 8 | 18 | 15 | 6 | 16 | 19 | 16 | 20 | 17 | 20 | 20 | 12 |
| [KIT-A20 App shell and phone apps](projects/kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) | 2 | 7 | 9 | 12 |  | 14 | 10 | 7 | 2 | 4 | 2 | 3 | 11 |
| [KIT-A21 Help and member support](projects/kit/FEATURES.md#kit-a21-help-and-member-support) | 5 | 3 | 7 | 5 | 6 | 15 | 11 | 10 | 5 | 4 | 8 | 1 | 12 |
| [KIT-A22 Running it: hosting, security and admin](projects/kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) | 12 | 9 | 6 | 12 | 6 | 9 | 14 | 3 | 7 | 9 | 4 | 7 | 12 |
| [KIT-A23 Work law and worker status](projects/kit/FEATURES.md#kit-a23-work-law-and-worker-status) |  |  | 1 |  | 1 | 10 | 12 | 2 |  | 2 | 4 | 1 | 8 |
| [KIT-A24 Health and safety at work](projects/kit/FEATURES.md#kit-a24-health-and-safety-at-work) |  |  | 1 |  | 1 | 5 | 10 | 4 | 1 | 4 | 5 | 1 | 9 |
| [KIT-A25 Signatures, forms, notices and training](projects/kit/FEATURES.md#kit-a25-signatures-forms-notices-and-training) | 2 | 3 | 1 | 3 | 1 | 8 | 2 | 5 |  | 3 | 3 | 1 | 11 |
| [KIT-A26 Property records, ownership and tenancy rules](projects/kit/FEATURES.md#kit-a26-property-records-ownership-and-tenancy-rules) |  |  |  |  |  |  |  | 2 |  |  |  |  | 1 |
| [KIT-A27 Shared engines](projects/kit/FEATURES.md#kit-a27-shared-engines) | 19 | 57 | 39 | 48 | 11 | 8 | 2 | 5 | 21 | 25 | 10 | 15 | 12 |
| [KIT-A28 Own sites, public pages and feeds](projects/kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds) |  |  |  |  |  |  |  |  |  |  |  |  | 0 |

How names are counted: kit-core, kit-ops, kit-sync and kit-store under KIT-A15; kit-plugin under KIT-A16; kit-mcp under KIT-A17; kit-market under KIT-A5; svc-trust under KIT-A10; the other services under the area that builds them; and engine-render, engine-draw2d, engine-text, engine-pdf and engine-media under KIT-A27.

KIT-A26 and KIT-A28 show few links because Spare Home, Spare Rent, Spare Stay, Spare Market and others still map their own copies of the property, notice, site and feed parts the kit now holds. Switching those programs to the kit's parts is an open question in the [kit's map](projects/kit/FEATURES.md#assessing-this-map-and-its-demo).

## Order to build in

A lean for discussion, from what the maps show. The first marketplace is decided by kit piece K05, and any change to this order that counts as a major change goes to a member vote.

1. **Kit foundations.** The core features of all 12 programs depend on KIT-A1 (Accounts and sign-in), KIT-A15 (Sync, history and storage), KIT-A16 (Plugins, open API and integrations), KIT-A18 (Privacy, data rights and legal duties), KIT-A19 (Languages, accessibility and countries), KIT-A21 (Help and member support) and KIT-A22 (Running it: hosting, security and admin). Eleven programs depend on the app shell (KIT-A20) and ten on AI access (KIT-A17). These come first: permanent IDs, commands and the operation log, sign-in, privacy, languages and accessibility, the API and plugins, help, running the servers, the app shell and AI access.
2. **Shared engines (KIT-A27), in order of reach.** Counting programs whose core features need each engine: engine-media 11, engine-pdf 10, engine-draw2d 7, engine-text 6, engine-render 4. engine-text goes first because engine-pdf lays out its text with it; engine-pdf and engine-media then serve the most programs, including every marketplace for documents, receipts, photos and video. engine-draw2d and engine-render follow for the design tools, Spare Office and the floor plans. The proposed engine-formats (office files) and engine-models (text recognition, speech and voices on the device) come next, since Spare Office, Spare Studio, Spare OS and Spare Rent wait on them.
3. **The kit's money and marketplace loop.** Before any app where people earn a living goes live: payments and payouts, tax, the marketplace engine, matching, maps, messaging, ratings, safety and disputes, member voting, the open cost ledger, membership, work law and health and safety (KIT-A2 to KIT-A14, KIT-A23 to KIT-A26 and KIT-A28, core parts only). The marketplaces lean on the kit most: Ride 61%, Food 70%, Stay 69%, Home 48%, Rent 47%, Work 62%, Market 64% of their parts depend on it.
4. **The first marketplace, then the others in families that share the most kit.** K05 picks the first; it proves the whole loop. Then:
   - **Spare Ride and Spare Food together:** dispatch, live location, drivers and couriers, work-time limits, driving-safe mode and ordering by text or phone.
   - **Spare Work and Spare Market:** the shared service catalogue, category trees, banned and restricted rules, invoices with payment terms, customer lists and own sites.
   - **Spare Rent, Spare Home and Spare Stay, after Spare Work:** the property record, ownership checks, formal notices, viewings and floor plans (KIT-A26). They book Spare Work's pros through the shared catalogue, so Spare Work's catalogue entries come first.
5. **Design and everyday computing, alongside the marketplaces.** These lean on the kit far less (Construct 19%, Studio 31%, Social 46%, Office 40%, OS 25% of parts), so they can move as soon as the foundations and their engines are ready:
   - **Spare OS can start at once:** it builds on a proven open-source base (piece OS01), and only 25% of its parts touch the kit.
   - **Spare Studio's first app (piece S08) proves the engines**, then **Spare Construct** builds on the same renderer, 2D drawing, text and PDF, plus chunked storage (KIT-A15-F6) and plugins (KIT-A16-F5).
   - **Spare Office** follows once text co-editing (piece OF03) and the office formats engine are ready.
   - **Spare Social** follows once engine-media and the safety stack (KIT-A10) are ready, since it hosts members' video and posts at scale and needs reports and moderation from day one.

## Feature maps and demos

- **Shared kit and co-op portal, with the shared engines:** [feature map](projects/kit/FEATURES.md) · [demo](https://spareware.app/kit/)
- **Spare Construct:** [feature map](projects/spare-construct/FEATURES.md) · [demo](https://spareware.app/spare-construct/)
- **Spare Studio (working name):** [feature map](projects/spare-studio/FEATURES.md) · [demo](https://spareware.app/spare-studio/)
- **Spare Social (working name):** [feature map](projects/spare-social/FEATURES.md) · [demo](https://spareware.app/spare-social/)
- **Spare Office:** [feature map](projects/spare-office/FEATURES.md) · [demo](https://spareware.app/spare-office/)
- **Spare OS:** [feature map](projects/spare-os/FEATURES.md) · [demo](https://spareware.app/spare-os/)
- **Spare Ride:** [feature map](projects/spare-ride/FEATURES.md) · [demo](https://spareware.app/spare-ride/)
- **Spare Food:** [feature map](projects/spare-food/FEATURES.md) · [demo](https://spareware.app/spare-food/)
- **Spare Stay:** [feature map](projects/spare-stay/FEATURES.md) · [demo](https://spareware.app/spare-stay/)
- **Spare Home:** [feature map](projects/spare-home/FEATURES.md) · [demo](https://spareware.app/spare-home/)
- **Spare Rent:** [feature map](projects/spare-rent/FEATURES.md) · [demo](https://spareware.app/spare-rent/)
- **Spare Work:** [feature map](projects/spare-work/FEATURES.md) · [demo](https://spareware.app/spare-work/)
- **Spare Market:** [feature map](projects/spare-market/FEATURES.md) · [demo](https://spareware.app/spare-market/)
