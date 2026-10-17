# Spare Market: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Buying and selling where sellers pay only the at-cost fee, buyers' money is held until the item arrives, and small makers can run their own shop. Sellers and buyers are members and vote on its rules. It runs on the shared marketplace loop; this map covers only what buying and selling adds.

## Goals

- Listing is free; selling costs only the at-cost fee.
- Buyers are protected: payment is held until they confirm the item arrived as described.
- Scams are hard: no taking deals off the platform to avoid protection, verified sellers, clear warnings.
- Makers and small businesses can run a shop page.

## What it uses from the shared kit

Accounts and ID checks, payments (held until delivery), matching (search), maps (local pickup), messaging, ratings and disputes, member voting, the open cost ledger, and the marketplace engine.

## Its own parts

### Listings and search

**High level:** list an item with photos, condition, price or offers; buyers search by keyword, category, location and price.

**Low level:** full-text search with filters; saved searches and alerts; listings expire unless renewed.

### Buying

**High level:** buy now, make an offer, or (later) auctions; pay into a held account; choose shipping or local pickup.

**Low level:** shipping labels and tracking from courier services' published APIs, within their terms; local pickup confirmed in the app to release payment.

**Discuss:** auctions, and the consumer law that applies to them (piece MK02).

### Shops

**High level:** a shop page for makers and small businesses with their own listings, branding and reviews.

### Trust

**High level:** two-way ratings, buyer protection, scam warnings on risky messages, and a member panel for disputes.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Auctions | First release; later; never | Later |
| Business sellers | Same as everyone; shop tier | Same fee, shop page optional |
| Payment held | Always; only for shipping | Always, unless cash on pickup |

The work board is [STATUS.md](STATUS.md).
