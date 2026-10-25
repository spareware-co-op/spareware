# Spare Stay: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Short stays where hosts and guests pay only the at-cost fee, and hosts own the app and vote on its rules. It runs on the shared marketplace loop; this map covers only what stays add.

## Goals

- Guests see the full price, including every fee, from the first search.
- Hosts keep everything except the at-cost fee.
- Calendars stay in sync with other booking sites, so hosts can join without leaving them.
- Local rules (council registration, levies, house rules) are shown and kept.

## What it uses from Spare Kit

Accounts and ID checks, payments (deposits and refunds), matching (dates and search), maps, messaging, ratings and disputes, member voting, the open cost ledger, and the marketplace engine.

## Its own parts

### Guest app

**High level:** search by place and dates, see the full price, book, message the host, check in, review.

**Low level:** map and date search over an availability index; the full price is worked out before showing a result.

### Host app

**High level:** list a place with photos, rules and pricing; manage the calendar; accept or auto-accept bookings; see payouts.

**Low level:** calendars import and export in the iCalendar standard (RFC 5545), the format other booking sites already use for calendar sync, so double bookings are caught; nightly prices, minimum stays and cleaning fees as rules.

**Discuss:** channel managers for hosts with many listings.

### Booking and money

**High level:** the guest pays when booking; money is held until check-in; a damage deposit can be held and released after check-out; the cancellation policy is set by host vote options.

**Low level:** held money follows the payments piece K02; each booking stores the policy version it was made under.

### Trust

**High level:** guests and hosts verified; two-way reviews published at the same time so neither can retaliate; damage claims go to a member panel.

**Discuss:** host insurance offered through the co-op.

## What it's up against and the rules (researched 2026-10-18)

Not legal advice. Items marked *unverified* need checking before anyone relies on them.

| Topic | Finding | Source |
| --- | --- | --- |
| Airbnb's cut | Most hosts on the single-fee structure pay 15.5% | [Airbnb](https://www.airbnb.co.nz/help/article/1857) |
| Others | GST plus Bookabach's fee adds about 30% to the advertised rate (owners quoted); Booking.com's host rate is *unverified* | [OneRoof](https://www.oneroof.co.nz/news/bach-owners-say-airbnb-booking-fees-and-app-tax-are-crushing-them-its-just-not-worth-it-46337) |
| GST | Since 1 April 2024 the platform collects 15% GST on short stays; unregistered hosts get 8.5% back as a flat credit | [IRD](https://www.ird.govt.nz/sharing-economy/online-marketplaces/gst-on-listed-services/gst-obligations-for-marketplaces) |
| Reporting | Since 1 January 2024 platforms report hosts' income to IRD | [IRD](https://ird.govt.nz/international-tax/exchange-of-information/digital-platform-information) |
| Councils | Rules differ by council: Queenstown requires registration, with night caps by zone and records the council can see; Auckland's rules are by zone (*unverified* against council text) | [QLDC](https://www.qldc.govt.nz/services/rates/short-term-accommodation/) |
| Learn from | Fairbnb.coop (reported 15%, half to community projects); its site was offline when checked | [The News](https://www.thenews.coop/?p=171501) |
| Hardest problems | Damage cover (*unverified*); getting guests when Airbnb holds the demand | |

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Who pays the fee | Guest; host; split | Shown to both, split by vote |
| Instant booking | Default on; host choice | Host choice |
| Damage cover | Co-op pool; outside insurer; none | Research first |

The work board is [STATUS.md](STATUS.md).
