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

## What it's up against and the rules (researched 2026-10-18)

Not legal advice. Items marked *unverified* need checking before anyone relies on them.

| Topic | Finding | Source |
| --- | --- | --- |
| Trade Me | Dropped its 7.9% casual-seller success fee in March 2026; sellers now pay 2.19% through Ping, and buyers pay 99c, $1.99 or $4.99 on purchases over $20 | [Consumer NZ](https://consumer.org.nz/shopping/online-shopping/the-fine-print-whats-behind-trade-mes-move-to-scrap-success-fees) |
| Others | Facebook Marketplace charges 10% on shipped sales in the US (NZ *unverified*); eBay and Etsy NZ fees *unverified* | [VAR](https://www.valueaddedresource.net/facebook-marketplace-doubling-selling-fees/) |
| Consumer law | The Fair Trading Act 1986 applies to online selling, including saying when you sell in trade; the Consumer Guarantees Act 1993 covers in-trade sellers | [Commerce Commission](https://comcom.govt.nz/business/dealing-with-typical-situations/selling-goods-and-services/selling-online/) |
| Anti-money-laundering | Online marketplaces have had to comply with AML/CFT law since 1 June 2025 | [DIA](https://www.dia.govt.nz/AML-CFT-Information-for-Online-Marketplaces) |
| Learn from | Open Food Network (AGPL; learn from only) had a NZ instance; Fairmondo, a member-owned marketplace in Germany | [Open Food Network](https://github.com/openfoodfoundation/openfoodnetwork) |
| Hardest problems | Trade Me's network effect; fraud and disputes | |

**What this changes:** Trade Me's seller fees are now low, so Spare Market has to win on buyer fees, protection, shops for makers and member ownership, not on seller fees alone.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Auctions | First release; later; never | Later |
| Business sellers | Same as everyone; shop tier | Same fee, shop page optional |
| Payment held | Always; only for shipping | Always, unless cash on pickup |

The work board is [STATUS.md](STATUS.md).
