# Spare Stay: features map

*Stage: Mapping. Built from [features.json](features.json); edit that file and this page together. Mapped top-down on 2026-10-18, then checked on 2026-10-19 against an independent review and the kit's own map. The architecture is in [ARCHITECTURE.md](ARCHITECTURE.md) and the work board in [STATUS.md](STATUS.md).*

Spare Stay replaces Airbnb, Booking.com and Bookabach. Short stays where hosts keep everything except the at-cost fee, guests see the full price from the first search, and hosts own the app and vote on its rules. It runs on the kit's shared marketplace loop and named kit parts, and adds only what stays need. This map lists everything a full replacement needs, top-down so nothing is missed: each **area** is a part of the product, each **feature** is something a person can do, and each **part** is a piece that can be built on its own. It covers every journey: first sign-up, listing a place, booking, paying, the stay itself, things going wrong (lock-outs, damage, double bookings, emergencies, a host who leaves), safety, access for everyone, offline use at baches with no signal, languages and countries, taking your data and reviews with you, moderation and the law. Most of the work sits on the shared kit, which now has 26 areas: accounts, identity checks, payments, tax, the marketplace engine, search and matching, maps, messaging and calls, ratings, safety, disputes, voting, the open cost ledger, membership, sync and storage, the open API, AI access, privacy, languages and accessibility, the app shell, support, hosting and admin, work law, health and safety, signatures and training, and property records (see the [kit's feature map](../kit/FEATURES.md)). Where a part rests on the kit, it names the kit part by ID and holds only Stay's own rules; Stay uses 228 of the kit's 572 parts. Where Stay needs a shared part the kit does not have yet, the Stay part says the kit should own it. IDs are never renumbered or reused: 33 parts and one feature were retired in the review, and 4 parts moved (listed below). **Core** means needed for a first usable release, **next** soon after, **later** after that. Nothing here copies another company's design; we follow the same journeys because guests and hosts need them, and build them our own way.

**22 areas, 168 features (87 core, 57 next, 24 later), 341 parts.** **Depends on** names kit crates and shared engines (in code type), kit parts by ID (KIT-...) or other Stay parts by ID. **Piece** links a part to an existing work-board piece.

| Area | What it covers | Features | Parts |
| --- | --- | --- | --- |
| [STAY-A1 Accounts and profiles](#stay-a1-accounts-and-profiles) | One Spareware login for guests, hosts, co-hosts, cleaners and property managers, with Stay's own profile fields, business details and the checks each role needs, on the kit's accounts and identity checks. | 7 | 17 |
| [STAY-A2 Listings](#stay-a2-listings) | Hosts describe a place truthfully and fully: what it is, where it is, what it has, its rules, its accessibility and safety, and its photos, checked before it goes live. | 11 | 29 |
| [STAY-A3 Calendar and channel sync](#stay-a3-calendar-and-channel-sync) | One calendar per place that knows every booking and block, stays in sync with other booking sites, and stops double bookings. | 5 | 17 |
| [STAY-A4 Pricing and the full price](#stay-a4-pricing-and-the-full-price) | Hosts set prices with clear rules; guests see one full price, with every fee, tax and levy, from the first search. | 8 | 21 |
| [STAY-A5 Search and discovery](#stay-a5-search-and-discovery) | Guests find the right place by where, when and who, on a map or a list, ranked by rules that are published, with no paid placement. | 10 | 19 |
| [STAY-A6 Booking](#stay-a6-booking) | Guests book instantly or by request, see exactly what they agree to, and get a clear confirmation and trip page. | 12 | 23 |
| [STAY-A7 Payments, holds and payouts](#stay-a7-payments-holds-and-payouts) | Guests pay through a regulated provider, money is held within the provider's time limits, hosts are paid quickly, and the at-cost fee is shown on every booking. | 8 | 18 |
| [STAY-A8 Changes, cancellations and refunds](#stay-a8-changes-cancellations-and-refunds) | A few clear cancellation policies chosen by host vote, changes either side can ask for, and refunds worked out by rule. | 6 | 13 |
| [STAY-A9 The stay](#stay-a9-the-stay) | Everything a guest needs from arrival to check-out on one trip page that works without signal, and help when something goes wrong. | 11 | 18 |
| [STAY-A10 Messages and notifications](#stay-a10-messages-and-notifications) | Hosts and guests talk in one place, with translation, saved and scheduled messages, scam warnings, calls that hide numbers, and only the alerts each person chooses. | 7 | 10 |
| [STAY-A11 Host operations and teams](#stay-a11-host-operations-and-teams) | Tools to run one place or a hundred: a daily view, co-hosts with set permissions, cleaning and upkeep, bulk changes and insights. | 9 | 17 |
| [STAY-A12 Reviews](#stay-a12-reviews) | Two-way reviews from real stays only, published together so neither side can retaliate, with fair published rules for removal. | 8 | 13 |
| [STAY-A13 Trust, safety and damage](#stay-a13-trust-safety-and-damage) | Verified people, fraud and scam checks, help in an emergency, protection when a stay goes wrong, fair damage claims, blocking, and disputes decided by panels that are fair to both sides. | 11 | 29 |
| [STAY-A14 Local rules, tax and legal duties](#stay-a14-local-rules-tax-and-legal-duties) | Council rules, GST, income reporting, levies and consumer law built in as rules, NZ first, with each new country added as data. | 9 | 27 |
| [STAY-A15 Co-op membership, votes and the open fee](#stay-a15-co-op-membership-votes-and-the-open-fee) | Hosts are members who own the app, vote on its fees and rules within competition law, hear guests before votes that affect them, and can see what every dollar of the fee paid for. | 5 | 10 |
| [STAY-A16 Help and support](#stay-a16-help-and-support) | Answers first, then people: a help centre, support requests linked to bookings, and a phone line for urgent problems, all run at cost. | 6 | 7 |
| [STAY-A17 Accessibility, languages and regions](#stay-a17-accessibility-languages-and-regions) | Usable by everyone, in their own language, currency and formats: NZ first, ready for any country. | 4 | 4 |
| [STAY-A18 Apps, offline and open access](#stay-a18-apps-offline-and-open-access) | Phone apps and a web app for guests and hosts, working offline where it matters, with the same commands open to AI agents, plugins and scripts. | 7 | 13 |
| [STAY-A19 Your data and privacy](#stay-a19-your-data-and-privacy) | Members own their data: see it, take it with them in open formats, choose who sees it, and delete it. | 8 | 11 |
| [STAY-A20 Moderation and administration](#stay-a20-moderation-and-administration) | Stay's moderators and staff act through the kit's review queue and admin console, with every removal and the rule behind it in the public log. Monitoring and backups are the kit's, for every app. | 6 | 7 |
| [STAY-A21 Reach, direct bookings and extras](#stay-a21-reach-direct-bookings-and-extras) | Bring guests to hosts without paid ranking: a plan for the empty start, hosts' own booking pages, sharing, web search, open feeds, local tourism partners, and extras around the stay. | 7 | 13 |
| [STAY-A22 Experiences](#stay-a22-experiences) | Tours and activities hosted by members and booked alongside stays. A separate marketplace with its own safety law, mapped here as its own area so it can become its own program if members choose. | 3 | 5 |

## STAY-A1 Accounts and profiles

One Spareware login for guests, hosts, co-hosts, cleaners and property managers, with Stay's own profile fields, business details and the checks each role needs, on the kit's accounts and identity checks.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A1-F1 Sign up and sign in | Create a Spare Stay account with a passkey, email or phone, and sign in on any device. | core | Stay sign-in set-up; Stay roles; Recovery rules for payout holders; Minimum ages |
| STAY-A1-F2 Profiles | Show who you are with a name, photo, languages, a short bio and verified badges. | core | Profile page; Profile fields and visibility; Profile photo checks |
| STAY-A1-F3 Identity checks | Check your ID once, through a regulated provider, so hosts and guests can trust each other. | core | Stay's ID checks; Overseas guests' IDs; When a check is needed |
| STAY-A1-F4 Host business details | Record whether you host as a private person or a business, with the tax details income reporting needs, kept private, and show guests which one you are. | core | Host business record; Business details screens; Private or business host |
| STAY-A1-F5 Account security | Keep your account safe, with a second sign-in step required for anyone who receives payouts or sees guests' addresses. | core | Second step for hosts |
| STAY-A1-F6 Travel companions | Add the people you travel with so they see the trip details and can message the host. | next | Trip companions; Companion screens |
| STAY-A1-F7 Host and cleaner courses | Take short courses with certificates: host basics, and an orientation for cleaners. | next | Stay courses |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A1-F1-P1 | **Stay sign-in set-up**: Uses the kit's passkey-first sign-in screens, asking only what a booking needs: a name, an email and a phone number. | interface | small | KIT-A1-F2-P4, KIT-A1-F2-P1 |  |
| STAY-A1-F1-P2 | **Stay roles**: Stay's roles (guest, host, co-host, cleaner and property manager) on the kit's roles per app, each with its own permissions. | data | small | KIT-A1-F5-P2 |  |
| STAY-A1-F1-P3 | **Recovery rules for payout holders**: Stay asks for the kit's extra recovery checks for hosts and anyone else who receives payouts, and holds payouts for a waiting period after a recovery. | service | small | KIT-A1-F4-P1, KIT-A1-F4-P2, KIT-A3-F3-P4 |  |
| STAY-A1-F1-P4 | **Minimum ages**: Only adults (18 and over) can book or host, set on the kit's minimum age at sign-up and confirmed from the identity check's date of birth when a check is needed. | legal | small | KIT-A1-F1-P5, KIT-A2-F1-P3 |  |
| STAY-A1-F2-P1 | **Profile page**: A public profile with name, photo, languages, bio, member since and verified checks. | interface | small | KIT-A1-F5-P1, KIT-A2-F5-P1, `engine-media` |  |
| STAY-A1-F2-P2 | **Profile fields and visibility**: What each profile field holds and who can see it before and after a booking. | data | small | KIT-A1-F5-P1 |  |
| STAY-A1-F2-P3 | **Profile photo checks**: Photos are checked for banned content before they show; any face check runs under the Biometric Processing Privacy Code 2025 through the kit, with a person reviewing a failed match. | service | small | KIT-A10-F10-P2, KIT-A2-F1-P4, `engine-media` |  |
| STAY-A1-F3-P1 | **Stay's ID checks**: Guests and hosts check their ID once through the kit's identity check; Stay keeps only the result, and any selfie match runs under the Biometric Processing Privacy Code 2025 with a non-biometric option. | integration | small | KIT-A2-F1-P2, KIT-A2-F1-P3, KIT-A2-F1-P4 |  |
| STAY-A1-F3-P2 | **Overseas guests' IDs**: Stay's needs for the kit's identity provider choice: passports and licences from the countries NZ's visitors come from, and what each check costs. | research | small | KIT-A2-F1-P1 |  |
| STAY-A1-F3-P3 | **When a check is needed**: Stay's rules on the kit's check rules per app for when an ID check is required: every host, and guests for instant booking or high-risk stays. | legal | small | KIT-A2-F2-P2, STAY-A1-F3-P1 |  |
| STAY-A1-F4-P1 | **Host business record**: Private person or business, legal name, IRD number, date of birth for individuals, GST registration and NZBN, on the kit's seller details and held privately. | data | small | KIT-A4-F3-P2 |  |
| STAY-A1-F4-P2 | **Business details screens**: Screens to enter and update tax and business details. | interface | small | STAY-A1-F4-P1, KIT-A20-F1-P1 |  |
| STAY-A1-F4-P4 | **Private or business host**: Shows guests on the listing and the receipt whether the host is a private person or a business, with the business's trader details, since consumer law rights differ. | data | small | STAY-A1-F4-P1, KIT-A2-F7-P2 |  |
| STAY-A1-F5-P1 | **Second step for hosts**: Hosts, co-hosts and anyone else who receives payouts or sees guests' addresses must turn on the kit's second sign-in step; devices, sign-in alerts and payout change holds come from the kit. | service | small | KIT-A1-F3-P1, KIT-A1-F3-P2, KIT-A1-F3-P3, KIT-A3-F3-P4 |  |
| STAY-A1-F6-P1 | **Trip companions**: Invite co-travellers to a booking so they see the trip page and the chat. People on an order should move down into the kit, since Food, Ride and Work need the same. | service | small | KIT-A1-F1-P1, `kit-market`, STAY-A6-F2-P2 |  |
| STAY-A1-F6-P2 | **Companion screens**: Invite, list and remove travel companions. | interface | small | STAY-A1-F6-P1 |  |
| STAY-A1-F7-P1 | **Stay courses**: Short courses on the kit's training module: host basics (legal duties, safety and fair treatment) and cleaner orientation, each with a certificate shown on the profile. | interface | small | KIT-A25-F2-P1, KIT-A25-F2-P2 |  |

## STAY-A2 Listings

Hosts describe a place truthfully and fully: what it is, where it is, what it has, its rules, its accessibility and safety, and its photos, checked before it goes live.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A2-F1 Create a listing | Follow step-by-step setup to list a whole home, a room, a bach, a cabin, a campsite or a small hotel's rooms. | core | Listing data model; Listing setup steps; Place types; Draft, publish and snooze; Getting there and signal |
| STAY-A2-F2 Photos | Add photos with captions and sort them room by room. | core | Photo screens; Photo checks |
| STAY-A2-F3 Description, amenities and house rules | Describe the place, tick its amenities, and set house rules such as pets, smoking, parties, quiet hours and check-in times. | core | Amenity list; House rules; Description editor; Writing help |
| STAY-A2-F4 Location and privacy | Set the exact address for booked guests and show only an approximate area to everyone else. | core | Address and pin; Approximate location; Address check |
| STAY-A2-F5 Accessibility details | Describe step-free access, door widths, bathroom features and more, with a photo proving each one. | core | Accessibility features list; Feature photos and review; What disabled travellers need listed |
| STAY-A2-F6 Safety details | Declare smoke alarms, carbon monoxide alarms, first aid, pools, hazards and any cameras or noise monitors. | core | Safety declarations; Cameras and monitors rule; NZ safety facts |
| STAY-A2-F7 Listing quality check | Get a checklist of what is missing or unclear, and pass a review before the listing goes live. | core | Completeness checklist; Listing review |
| STAY-A2-F8 Many units and room types | List a set of identical rooms or units once, with how many are free each night. | next | Room and unit types; Unit screens |
| STAY-A2-F9 Translated listings | Show your listing in the guest's language, with your own words kept as the original. | next | Listing translation; Host's own translations |
| STAY-A2-F10 Move from another site | Bring your own photos, words and calendar link across from another booking site in a few steps. | next | Move-in helper; What can be brought across |
| STAY-A2-F11 Floor plans | Add a simple floor plan, drawn by hand or brought in from a building model. | next | Floor plans |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A2-F1-P1 | **Listing data model**: Stay's listing fields on the kit's listing model and the shared property record: place type, rooms, beds, bathrooms, guest limit, address and everything else a listing holds, changed only by operations. | data | medium | KIT-A5-F1-P1, KIT-A26-F1-P1, `kit-ops` |  |
| STAY-A2-F1-P2 | **Listing setup steps**: Step-by-step setup that saves as you go and can be finished later. | interface | medium | STAY-A2-F1-P1, KIT-A5-F1-P2 |  |
| STAY-A2-F1-P3 | **Place types**: The list of place types and what each must state, from whole homes and rooms to hotel rooms and campsites; a campsite or glamping site needs a council certificate of registration before it can go live. | data | small | STAY-A2-F1-P1 |  |
| STAY-A2-F1-P4 | **Draft, publish and snooze**: Listings stay drafts until checks pass, then go live; hosts can snooze or unlist them, and unlisting with bookings still to come follows the rule for future bookings. | service | small | STAY-A2-F1-P1, KIT-A5-F1-P3, STAY-A19-F6-P1 |  |
| STAY-A2-F1-P5 | **Getting there and signal**: Facts for remote places: mobile coverage by network, the nearest spot with signal, gravel or four-wheel-drive roads, and boat, ferry or tidal access, shown before booking and kept in the offline trip. | data | small | STAY-A2-F1-P1 |  |
| STAY-A2-F2-P1 | **Photo screens**: Upload, reorder, caption and tag photos by room, on the kit's file uploads, with image descriptions for screen readers from the kit. | interface | medium | KIT-A15-F7-P1, KIT-A19-F2-P3 |  |
| STAY-A2-F2-P3 | **Photo checks**: Stay's use of the kit's photo privacy check, adding house numbers and street signs to what it flags; copied photos are reported through the kit's copyright notices. | service | small | KIT-A15-F7-P4, KIT-A10-F21-P2 |  |
| STAY-A2-F3-P1 | **Amenity list**: A standard amenity list with exact meanings, so filters work the same in every language. | data | medium | STAY-A2-F1-P1 |  |
| STAY-A2-F3-P2 | **House rules**: Standard and custom house rules that guests accept before booking, including any body corporate rules on noise, security and common areas. | data | small | STAY-A2-F1-P1 |  |
| STAY-A2-F3-P3 | **Description editor**: Title, description and space details, with plain-language tips. | interface | small | STAY-A2-F1-P1, KIT-A5-F1-P2 |  |
| STAY-A2-F3-P4 | **Writing help**: Optional AI help to draft a description from the host's own notes and photos, on the kit's co-op AI, marked as AI and always edited by the host. | service | small | KIT-A17-F6-P2, STAY-A2-F3-P3 |  |
| STAY-A2-F4-P1 | **Address and pin**: Finds the address with the kit's address search and lets the host move the pin. | integration | small | KIT-A7-F2-P2, KIT-A7-F2-P4 |  |
| STAY-A2-F4-P2 | **Approximate location**: Stay's use of the kit's approximate location: the offset is set once per listing and never changes, so repeated searches can't average it out. | service | small | KIT-A7-F10-P1, STAY-A2-F4-P1 |  |
| STAY-A2-F4-P3 | **Address check**: Confirms the host controls the address, to stop fake listings. | service | medium | KIT-A7-F2-P3, STAY-A2-F4-P1 |  |
| STAY-A2-F5-P1 | **Accessibility features list**: Measurable features such as step-free entry, door width in centimetres and grab rails. | data | small | STAY-A2-F1-P1 |  |
| STAY-A2-F5-P2 | **Feature photos and review**: Each feature needs its own photo and is reviewed before it shows. | service | medium | STAY-A2-F5-P1, KIT-A15-F7-P1, KIT-A10-F6-P2 |  |
| STAY-A2-F5-P3 | **What disabled travellers need listed**: Research with disabled travellers and NZ disability groups on which details matter, with sources. | research | small |  |  |
| STAY-A2-F6-P1 | **Safety declarations**: Required safety facts for each place type, shown to guests before they book. | data | small | STAY-A2-F1-P1 |  |
| STAY-A2-F6-P2 | **Cameras and monitors rule**: Outdoor cameras and noise monitors must be disclosed; cameras inside are banned. | legal | small | STAY-A2-F6-P1 |  |
| STAY-A2-F6-P3 | **NZ safety facts**: NZ facts for each place: the water source (town supply, tank or bore) and whether a shared supply is registered with Taumata Arowai; pool and spa barriers under the Building Act; an evacuation scheme where a building houses more than 5 people (unless 3 or fewer household units); and smoke alarm checks. Each legal point is marked to verify. | data | small | STAY-A2-F6-P1 |  |
| STAY-A2-F7-P1 | **Completeness checklist**: Shows what is missing or unclear, using rules anyone can read. | service | small | STAY-A2-F1-P1 |  |
| STAY-A2-F7-P2 | **Listing review**: Stay's checks on the kit's listing checks: new and changed listings are checked automatically, people review flagged ones, and the automatic checks are on the AI use register. | service | small | KIT-A5-F1-P3, KIT-A10-F6-P2, STAY-A2-F1-P4 |  |
| STAY-A2-F8-P1 | **Room and unit types**: Stay's unit types on the kit's availability calendars: one listing with many identical units and a count free each night. | data | medium | KIT-A5-F5-P1, STAY-A2-F1-P1 |  |
| STAY-A2-F8-P2 | **Unit screens**: Add units, set counts and link each to its own calendar. | interface | medium | STAY-A2-F8-P1 |  |
| STAY-A2-F9-P1 | **Listing translation**: Stay's use of the kit's machine translation at cost for listing text, marked as translated, with the original one tap away. | service | small | KIT-A19-F10-P2, STAY-A2-F1-P1 |  |
| STAY-A2-F9-P2 | **Host's own translations**: Hosts write or correct the text in other languages. | interface | small | STAY-A2-F9-P1 |  |
| STAY-A2-F10-P1 | **Move-in helper**: Stay's use of the kit's bulk import: hosts bring their own photos, words and calendar links from another booking site in one flow, checked before anything goes live. | interface | medium | KIT-A5-F17-P2, STAY-A2-F1-P2, STAY-A3-F3-P2 |  |
| STAY-A2-F10-P2 | **What can be brought across**: Stay's cases for the kit's import terms: hosts bring their own photos, words and calendar links, never other sites' data or reviews. | legal | small | KIT-A5-F17-P1 |  |
| STAY-A2-F11-P1 | **Floor plans**: A simple floor plan drawn on the 2D drawing engine, or brought in from a Spare Construct model file through the kit. The plan drawer and the import should move down into the kit or an engine, since Spare Home and Spare Rent map the same; Stay never links to another program directly. | interface | medium | `engine-draw2d`, STAY-A2-F1-P1 |  |

## STAY-A3 Calendar and channel sync

One calendar per place that knows every booking and block, stays in sync with other booking sites, and stops double bookings.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A3-F1 Host calendar | See and change availability, prices and bookings for every night on one calendar, and mark nights kept for your own use. | core | Availability data; Calendar view; Date holds; Owner use blocks |
| STAY-A3-F2 Stay rules | Set minimum and maximum stays, arrival days, advance notice, how far ahead guests can book and days between stays, and change them by season. | core | Stay rule set; Bookable dates engine; Seasonal stay rules |
| STAY-A3-F3 Sync with other booking sites | Import and export calendars with other booking sites, so nights booked there close here. | core | Stay calendar export; Stay calendar import; Double-booking guard; Sync status; Double booking resolution |
| STAY-A3-F4 Channel managers | Connect a channel manager that updates your rates and availability across sites, if you run many places. | next | Channel connection API; Channel manager research; Partner sandbox; Property management system links |
| STAY-A3-F5 Personal calendar feed | Add your trips or bookings to your own calendar app. | next | Calendar feed for people |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A3-F1-P1 | **Availability data**: Stay's nights on the kit's availability calendars: each night open, blocked, booked, on hold or kept for the owner's own use, with full history. | data | small | KIT-A5-F5-P1, STAY-A2-F1-P1 |  |
| STAY-A3-F1-P2 | **Calendar view**: Month and many-listing views with tap to block, tap to price and booking details. | interface | medium | STAY-A3-F1-P1, KIT-A20-F1-P1 |  |
| STAY-A3-F1-P3 | **Date holds**: How long Stay holds dates while a guest pays or a request waits, on the kit's availability calendars, so two guests can't take the same nights. | service | small | KIT-A5-F5-P1, STAY-A3-F1-P1 |  |
| STAY-A3-F1-P4 | **Owner use blocks**: The host marks blocked nights as the owner's own use, so the yearly summary can count private, rented and unused days. | data | small | STAY-A3-F1-P1 |  |
| STAY-A3-F2-P1 | **Stay rule set**: Minimum and maximum nights, allowed arrival and departure days, advance notice, booking window and preparation days. | data | small | STAY-A3-F1-P1 |  |
| STAY-A3-F2-P2 | **Bookable dates engine**: Works out which dates a guest can book from Stay's rules and the kit's availability calendars. | library | small | KIT-A5-F5-P1, STAY-A3-F2-P1, STAY-A3-F2-P3 |  |
| STAY-A3-F2-P3 | **Seasonal stay rules**: Stay rules by date range or season, such as a seven-night minimum and Saturday arrivals over Christmas and New Year, with exceptions that let short gaps between bookings be filled. | data | small | STAY-A3-F2-P1 |  |
| STAY-A3-F3-P1 | **Stay calendar export**: Stay's export on the kit's calendar import and export: each listing's private link holds booked and blocked nights only, never guest details. | service | small | KIT-A5-F5-P2, STAY-A3-F1-P1 |  |
| STAY-A3-F3-P2 | **Stay calendar import**: Stay's import on the kit's calendar import and export: imported events block nights, and each linked calendar is refreshed on a published schedule. | service | small | KIT-A5-F5-P2, STAY-A3-F1-P1 |  |
| STAY-A3-F3-P3 | **Double-booking guard**: Spots overlaps between imported and local bookings with the kit's calendar checks and alerts the host at once. | service | small | KIT-A5-F5-P2, KIT-A8-F3-P2, STAY-A3-F3-P2 |  |
| STAY-A3-F3-P4 | **Sync status**: Shows when each linked calendar last synced and any errors. | interface | small | STAY-A3-F3-P2 |  |
| STAY-A3-F3-P5 | **Double booking resolution**: When calendar lag causes a double booking, the booking made first stands, by the times both sites record. If that is the other site's, the Spare Stay guest gets a full refund and rebooking help, the host is not penalised when the other site's calendar was late, and both guests are told plainly what happened. | legal | small | STAY-A3-F3-P3, STAY-A13-F2-P2 |  |
| STAY-A3-F4-P1 | **Channel connection API**: Stay's commands for availability, rates and bookings, published for channel managers to use with scoped tokens. | service | medium | STAY-A18-F4-P1, STAY-A3-F1-P1, STAY-A4-F1-P1, KIT-A16-F1-P2 |  |
| STAY-A3-F4-P2 | **Channel manager research**: Which channel managers NZ hosts use, what open interfaces they offer and what joining costs. | research | small |  |  |
| STAY-A3-F4-P3 | **Partner sandbox**: A test area and checklist for channel managers to connect. | interface | medium | STAY-A3-F4-P1 |  |
| STAY-A3-F4-P4 | **Property management system links**: Links to the property management systems NZ motels and small hotels use, through their published interfaces or the channel API, starting with the ones the research shows matter most. | integration | medium | STAY-A3-F4-P1, STAY-A3-F4-P2 |  |
| STAY-A3-F5-P1 | **Calendar feed for people**: Guests and hosts subscribe to their trips or bookings in any calendar app. Personal calendar feeds should move down into the kit, since Ride, Food and Work bookings need the same. | service | small | STAY-A3-F3-P1 |  |

## STAY-A4 Pricing and the full price

Hosts set prices with clear rules; guests see one full price, with every fee, tax and levy, from the first search.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A4-F1 Nightly prices | Set a base nightly price, weekend prices, and prices for seasons and events. | core | Price rules; Price settings |
| STAY-A4-F2 Discounts | Offer weekly and monthly discounts, early-booking and last-minute deals, and a discount for first guests. | next | Discount rules; Promotion codes |
| STAY-A4-F3 Price options | Offer a cheaper price that can't be refunded alongside the flexible price, and let guests choose. | next | Price options; Price option screens |
| STAY-A4-F4 Extra fees and extras | Add cleaning, extra guest and pet fees, and optional extras such as firewood or linen hire, all shown in the full price. | core | Fee rules; Optional extras; Food and alcohol extras |
| STAY-A4-F5 Full price first | See the total for your dates and guests, with every fee, tax and levy, in every search result. | core | Price engine; Price index for search; Price breakdown; Price display rules |
| STAY-A4-F6 Pricing help | Get price suggestions from open local data, with the reasons shown. | next | Price suggestions; Pricing method research; Automatic pricing; Pricing tools and competition law |
| STAY-A4-F7 Currencies | See prices in your own currency while the host is paid in theirs. | next | Currency display; Currency at payment |
| STAY-A4-F8 Rate plans for hotels, motels and B&Bs | Offer rate plans such as breakfast included, price by the number of guests and child ages, and cots or high chairs on request. | next | Rate plans; Cots and high chairs on request |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A4-F1-P1 | **Price rules**: Stay's base, weekend, seasonal and date-specific nightly prices as ordered rules on the kit's pricing rules engine. | data | medium | KIT-A5-F3-P1, STAY-A3-F1-P1 |  |
| STAY-A4-F1-P2 | **Price settings**: Screens for the base price, weekend price, seasons and changing many dates at once. | interface | medium | STAY-A4-F1-P1, STAY-A3-F1-P2 |  |
| STAY-A4-F2-P1 | **Discount rules**: Length-of-stay, early, late and first-guest discounts, applied in a published order on top of Stay's full price. | data | small | STAY-A4-F1-P1, STAY-A4-F5-P1, KIT-A5-F14-P2 |  |
| STAY-A4-F2-P2 | **Promotion codes**: Hosts make codes for returning guests or groups on the kit's promotions and vouchers, always paid by the host who made them, with limits and end dates. | service | small | KIT-A5-F14-P2, STAY-A4-F2-P1 |  |
| STAY-A4-F3-P1 | **Price options**: More than one price for the same nights, each tied to a cancellation policy. | data | small | STAY-A4-F1-P1, STAY-A8-F1-P1 |  |
| STAY-A4-F3-P2 | **Price option screens**: Set up options and show guests the difference side by side. | interface | small | STAY-A4-F3-P1, STAY-A4-F5-P3 |  |
| STAY-A4-F4-P1 | **Fee rules**: Cleaning, extra guest, pet, linen and other fees, each per stay, per night or per guest. | data | small | STAY-A4-F1-P1 |  |
| STAY-A4-F4-P2 | **Optional extras**: Extras guests can add, such as firewood, linen hire or early check-in. | data | small | STAY-A4-F4-P1 |  |
| STAY-A4-F4-P3 | **Food and alcohol extras**: Rules for extras that are food or drink. Meals and breakfast hampers must meet the Food Act 2014: small places serving few guests may be exempt from registration (to confirm), but the food must still be safe and suitable. Alcohol can't be sold as an extra without a licence. | legal | small | STAY-A4-F4-P2 |  |
| STAY-A4-F5-P1 | **Price engine**: Stay's full price on the kit's full price first: the total for any dates and guests from nightly rates, fees, the at-cost fee, GST and local levies. Discounts and rate plans plug in once they exist. | library | medium | KIT-A5-F3-P2, STAY-A4-F1-P1, STAY-A4-F4-P1, STAY-A14-F3-P1 |  |
| STAY-A4-F5-P2 | **Price index for search**: Full prices worked out ahead for common date ranges, so search stays fast. | service | medium | STAY-A4-F5-P1, STAY-A5-F1-P2 |  |
| STAY-A4-F5-P3 | **Price breakdown**: A line-by-line breakdown anyone can open: nights, fees, the at-cost fee, tax and levies. | interface | small | STAY-A4-F5-P1, KIT-A3-F4-P2 |  |
| STAY-A4-F5-P4 | **Price display rules**: Stay's cases for the kit's consumer law duties on showing the total price, in NZ, Australia, the EU and other markets. | legal | small | KIT-A18-F8-P1 |  |
| STAY-A4-F6-P1 | **Price suggestions**: Suggests prices from public and aggregated local data on demand, seasons and events, never from competing hosts' private rates, and shows why. | service | large | STAY-A4-F5-P1, STAY-A4-F6-P2, STAY-A4-F6-P4 |  |
| STAY-A4-F6-P2 | **Pricing method research**: Proven open methods for price suggestions and which data we may legally use. | research | medium |  |  |
| STAY-A4-F6-P3 | **Automatic pricing**: Optional rules that apply suggestions within limits the host sets. | service | medium | STAY-A4-F6-P1, STAY-A4-F6-P4 |  |
| STAY-A4-F6-P4 | **Pricing tools and competition law**: Price suggestions and automatic pricing use only public or aggregated data, never the pooled private rates of hosts who compete with each other, so the shared tool can't become a cartel provision under the Commerce Act; checked with a lawyer. | legal | small | KIT-A12-F4-P3 |  |
| STAY-A4-F7-P1 | **Currency display**: Shows prices in the guest's currency on the kit's currencies, at a published rate, and says which rate was used. | service | small | KIT-A3-F14-P1 |  |
| STAY-A4-F7-P2 | **Currency at payment**: The guest is charged the full price shown in their currency if they pay within a set time of searching, and is shown the new total first if the rate has moved. Refunds go back in the guest's currency, and who carries an exchange loss on a refund is published. | integration | small | KIT-A3-F14-P1, STAY-A4-F7-P1 | [K02](../kit/STATUS.md) |
| STAY-A4-F8-P1 | **Rate plans**: Rate plans with meal options such as breakfast included, prices by the number of guests, and child prices by age band, on the price rules. | data | medium | STAY-A4-F1-P1, STAY-A4-F5-P1 |  |
| STAY-A4-F8-P2 | **Cots and high chairs on request**: Guests ask for a cot, a high chair or other items with the booking, and the host confirms. | data | small | STAY-A4-F4-P2 |  |

## STAY-A5 Search and discovery

Guests find the right place by where, when and who, on a map or a list, ranked by rules that are published, with no paid placement.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A5-F1 Search by place and dates | Search by town, region, address or landmark, with dates and the number of guests. | core | Search bar; Stay search fields |
| STAY-A5-F2 Map search | Browse places on a map with full prices on the pins, and search again as you move it. | core | Map results |
| STAY-A5-F3 Filters | Filter by price, place type, rooms, beds, amenities, accessibility features, pets, instant booking and more. | core | Filter set; Filter panel |
| STAY-A5-F4 Flexible dates | Search for a weekend, a week or a month without fixed dates. | next | Flexible date search; Flexible date picker |
| STAY-A5-F5 Honest ranking | See results in an order set by published rules, with no paid placement and the rules one tap away. | core | Stay ranking factors; Sort choices |
| STAY-A5-F6 Wishlists and sharing | Save places to lists, share a list with your group and pick favourites together. | next | Wishlists; Group picks |
| STAY-A5-F7 Saved searches and alerts | Save a search and get told when a matching place opens up or the price drops. | next | Saved searches; Alert choices |
| STAY-A5-F8 Browse by theme | Browse ideas such as beach baches, ski towns, pet-friendly or accessible stays. | later | Themes; Theme pages |
| STAY-A5-F9 Recently viewed and compare | Go back to places you looked at and compare a few side by side. | later | Recently viewed; Compare places |
| STAY-A5-F10 Ask in everyday words | Search in everyday words and compare places side by side with AI help, marked as AI. | later | Everyday-words search; AI comparison |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A5-F1-P1 | **Search bar**: Place, dates and guests, with suggestions as you type from the kit's address search, ignoring missing macrons. | interface | medium | KIT-A7-F2-P2, KIT-A19-F1-P4 |  |
| STAY-A5-F1-P2 | **Stay search fields**: Stay's fields in the kit's search service: open dates, stay rules and full prices, kept fresh as calendars change, with macron-aware place names. | service | medium | KIT-A6-F1-P2, KIT-A19-F1-P4, STAY-A3-F1-P1, STAY-A3-F2-P2 |  |
| STAY-A5-F2-P1 | **Map results**: A map with price pins and clusters that refreshes as you pan. | interface | medium | KIT-A7-F1-P2, STAY-A5-F1-P2 |  |
| STAY-A5-F3-P1 | **Filter set**: Filters built from listing data, with exact meanings in every language. | data | small | STAY-A2-F3-P1, STAY-A2-F5-P1, STAY-A2-F1-P3 |  |
| STAY-A5-F3-P2 | **Filter panel**: A filter panel that shows how many places match before you apply it. | interface | medium | STAY-A5-F3-P1, STAY-A5-F1-P2 |  |
| STAY-A5-F4-P1 | **Flexible date search**: Finds open stays of a set length within a month or season. | service | medium | STAY-A5-F1-P2, STAY-A3-F2-P2 |  |
| STAY-A5-F4-P2 | **Flexible date picker**: Pick a weekend, week or month and the months you could go. | interface | small | STAY-A5-F4-P1 |  |
| STAY-A5-F5-P1 | **Stay ranking factors**: Stay's factors on the kit's published ranking rules: fit to your search, full price, reviews and reliability, with a fair start for new hosts; nobody can pay to rank. | library | small | KIT-A6-F1-P3, KIT-A6-F1-P2, STAY-A5-F1-P2 |  |
| STAY-A5-F5-P3 | **Sort choices**: Sort by full price, distance, rating or newest. | interface | small | STAY-A5-F5-P1 |  |
| STAY-A5-F6-P1 | **Wishlists**: Stay's lists on the kit's favourites: named lists of saved places, private or shared by link with a group. | service | small | KIT-A5-F12-P1 |  |
| STAY-A5-F6-P2 | **Group picks**: People on a shared list add notes and vote for their favourite. | interface | small | STAY-A5-F6-P1 |  |
| STAY-A5-F7-P1 | **Saved searches**: Stay's saved searches on the kit's saved searches: place, dates and guests, with alerts for new matches and lower prices. | service | small | KIT-A6-F8-P1, STAY-A5-F1-P2 |  |
| STAY-A5-F7-P2 | **Alert choices**: Choose how often alerts come and how they arrive. | interface | small | STAY-A5-F7-P1 |  |
| STAY-A5-F8-P1 | **Themes**: Collections built from open rules on listing data, never paid placement. | data | small | STAY-A5-F3-P1 |  |
| STAY-A5-F8-P2 | **Theme pages**: Browse pages for themes and regions. | interface | small | STAY-A5-F8-P1 |  |
| STAY-A5-F9-P1 | **Recently viewed**: A private list of places you looked at, kept on your device unless you choose to sync it. | service | small | `kit-store` |  |
| STAY-A5-F9-P2 | **Compare places**: Full price, rooms, rules, reviews and distance side by side. | interface | small | STAY-A5-F9-P1, STAY-A4-F5-P1 |  |
| STAY-A5-F10-P1 | **Everyday-words search**: Turns a request such as 'a quiet bach near Raglan for four with a dog' into search filters through Stay's MCP commands, marked as AI, with the filters shown so the person can change them. | service | medium | STAY-A18-F4-P2, STAY-A5-F3-P1, KIT-A17-F10-P1 |  |
| STAY-A5-F10-P2 | **AI comparison**: An AI summary comparing chosen places side by side, marked as AI and linked to the listing facts it used. | service | small | STAY-A5-F9-P2, STAY-A18-F4-P2, KIT-A17-F10-P1 |  |

## STAY-A6 Booking

Guests book instantly or by request, see exactly what they agree to, and get a clear confirmation and trip page.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A6-F1 Listing page | See photos, the full price, rules, accessibility and safety details, the area, reviews and the host before you book. | core | Listing page; Date picker with prices |
| STAY-A6-F2 Instant booking | Book straight away when the host has turned instant booking on. | core | Booking steps; Booking record; Instant booking conditions |
| STAY-A6-F3 Booking requests | Send a request with a message; the host accepts or declines within 24 hours. | core | Request and reply; Request screens; Pre-approval and special offers |
| STAY-A6-F4 Guests, children and pets | Say how many adults, children, infants and pets are coming, and bring a service animal. | core | Guest details; Service animal rule |
| STAY-A6-F5 Confirmation and trip page | Get a confirmation with the booking number, address, check-in time and receipt, and one page for the whole trip. | core | Confirmation; Trip page |
| STAY-A6-F6 Booking documents | Download a receipt, a GST tax invoice, and a booking letter for visas or employers. | core | Stay tax invoices; Booking letter |
| STAY-A6-F7 Book for someone else | Book a stay for another person, such as a parent or a colleague. | next | Named main guest |
| STAY-A6-F8 Split the cost | Split the payment between the people in your group. | later | Split payments; Split screens |
| STAY-A6-F9 Business travel | Book for work, put a company's details on invoices and see a team's trips together. | later | Business accounts; Travel rules |
| STAY-A6-F10 Book several rooms or places at once | Book several rooms or places for a group, a wedding or a team in one booking. | next | Group bookings |
| STAY-A6-F11 Stays paid by someone else | Have an insurer, civil defence or an agency pay for a stay, including emergency housing after a flood or fire. | later | Third-party payers; Emergency housing offers |
| STAY-A6-F12 Pre-arrival questions | Answer a host's few allowed questions before arrival, such as arrival time, a car registration for parking and access needs. | later | Pre-arrival questions |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A6-F1-P1 | **Listing page**: The full listing page with photos, full price, rules, details, reviews, map, host profile and whether the host is a private person or a business. | interface | large | STAY-A2-F1-P1, STAY-A4-F5-P1, STAY-A5-F2-P1, STAY-A2-F4-P2, STAY-A1-F4-P4 |  |
| STAY-A6-F1-P2 | **Date picker with prices**: A calendar that shows bookable nights and the total for the dates you pick. | interface | medium | STAY-A3-F2-P2, STAY-A4-F5-P1 |  |
| STAY-A6-F2-P1 | **Booking steps**: Pick dates and guests, accept the house rules and cancellation policy, pay and confirm. | interface | medium | STAY-A6-F1-P1, STAY-A7-F1-P1 |  |
| STAY-A6-F2-P2 | **Booking record**: Stay's booking on the kit's order steps, keeping the price, cancellation policy, house rule and terms versions it was made under. | data | small | KIT-A5-F4-P1, KIT-A5-F6-P1, KIT-A18-F4-P1 |  |
| STAY-A6-F2-P3 | **Instant booking conditions**: Host choices, such as needing an ID check or good reviews before a guest can book instantly. | data | small | STAY-A6-F2-P2, STAY-A1-F3-P1 |  |
| STAY-A6-F3-P1 | **Request and reply**: Requests hold the dates until a reply deadline, then expire on their own. | service | medium | STAY-A6-F2-P2, STAY-A3-F1-P3, KIT-A15-F14-P1 |  |
| STAY-A6-F3-P2 | **Request screens**: The guest's request form and the host's accept, decline or ask-a-question screens. | interface | small | STAY-A6-F3-P1 |  |
| STAY-A6-F3-P3 | **Pre-approval and special offers**: The host pre-approves an inquiry or offers a custom price. | service | small | STAY-A6-F3-P1, STAY-A4-F5-P1 |  |
| STAY-A6-F4-P1 | **Guest details**: Adults, children, infants, pets and service animals, checked against the listing's limits. | data | small | STAY-A6-F2-P2 |  |
| STAY-A6-F4-P2 | **Service animal rule**: Stay's use of the kit's assistance animal rule: assistance dogs are always allowed and never charged a pet fee. | legal | small | KIT-A10-F19-P2, STAY-A6-F4-P1 |  |
| STAY-A6-F5-P1 | **Confirmation**: Confirmation screen, email and receipt with everything agreed, using the kit's receipts. | service | small | KIT-A8-F4-P1, KIT-A3-F7-P1, STAY-A6-F2-P2 |  |
| STAY-A6-F5-P2 | **Trip page**: One page per trip with dates, address, host, chat, documents, directions and the safety and help buttons. | interface | medium | STAY-A6-F2-P2, KIT-A5-F4-P2 |  |
| STAY-A6-F6-P1 | **Stay tax invoices**: Stay's content on the kit's tax invoices: whose name and GST number appear follows the seller of record decision, delivered as accessible PDFs. | service | small | KIT-A4-F2-P1, KIT-A19-F11-P1, STAY-A7-F1-P4, STAY-A6-F2-P2 |  |
| STAY-A6-F6-P2 | **Booking letter**: A plain letter confirming the stay, for visas or employers. | service | small | `engine-pdf`, KIT-A19-F11-P1, STAY-A6-F2-P2 |  |
| STAY-A6-F7-P1 | **Named main guest**: The booker and the main guest can differ, with the main guest's contact shared with the host. | data | small | STAY-A6-F2-P2 |  |
| STAY-A6-F8-P1 | **Split payments**: Each traveller pays their share before a deadline, or the booker covers the rest, on the kit's split payments. | integration | medium | KIT-A3-F21-P1, STAY-A6-F2-P2 |  |
| STAY-A6-F8-P2 | **Split screens**: Invite people, see who has paid and send reminders. | interface | small | STAY-A6-F8-P1 |  |
| STAY-A6-F9-P1 | **Business accounts**: Company travellers, a billing contact and company tax details on the kit's organisation model, paying on account through the kit. | service | small | KIT-A1-F7-P1, KIT-A3-F17-P1, STAY-A6-F6-P1 |  |
| STAY-A6-F9-P2 | **Travel rules**: Simple company rules such as a nightly price limit, applied at booking. | data | small | STAY-A6-F9-P1 |  |
| STAY-A6-F10-P1 | **Group bookings**: One booking that holds several rooms, units or places on the same dates, priced and paid together, so a group doesn't lose dates booking piece by piece. | service | medium | STAY-A6-F2-P2, STAY-A3-F1-P3, STAY-A2-F8-P1 |  |
| STAY-A6-F11-P1 | **Third-party payers**: An insurer, civil defence or a government agency pays for a guest's stay on account, with its own invoices. | service | medium | KIT-A3-F17-P1, STAY-A6-F7-P1 |  |
| STAY-A6-F11-P2 | **Emergency housing offers**: Hosts offer places to people displaced by floods or fires, which agencies can find and book during a declared emergency. | service | small | STAY-A6-F11-P1, STAY-A8-F5-P2 |  |
| STAY-A6-F12-P1 | **Pre-arrival questions**: A short, fixed set of questions a host may ask, such as arrival time, a car registration for parking and access needs, each answer kept only until the stay ends. | data | small | STAY-A6-F2-P2, KIT-A18-F6-P1 |  |

## STAY-A7 Payments, holds and payouts

Guests pay through a regulated provider, money is held within the provider's time limits, hosts are paid quickly, and the at-cost fee is shown on every booking.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A7-F1 Pay for a booking | Pay by card, bank payment or phone wallet through a regulated payment provider. | core | Stay checkout step; Payment methods; Payment records; Who sells the stay |
| STAY-A7-F2 Pay in parts | Pay a deposit when you book and the rest closer to the stay, inside the payment provider's time limits. | core | Payment schedule; Payment plan screens |
| STAY-A7-F3 Money held until the stay | Know your money sits with the payment provider, within its time limits, until check-in, then goes to the host. | core | Stay release point; Bookings made months ahead |
| STAY-A7-F4 Damage deposits | Ask guests for a refundable deposit, held on their card or paid, and released after check-out unless you claim. | next | Stay deposit rules; Deposit release clock |
| STAY-A7-F5 Host payouts | Choose how and when you are paid, and see each payout and what was taken out. | core | Stay payout timing; Earnings page |
| STAY-A7-F6 The at-cost fee shown | See the at-cost fee on every booking, with a link to what it paid for on the open cost ledger. | core | Stay fee inputs; Fee explained |
| STAY-A7-F7 Card disputes and failed payments | Have card disputes and failed payments handled fairly, with the evidence kept. | core | Stay's dispute evidence; Failed payment rules; Losses after payout |
| STAY-A7-F8 Gift cards | Buy a gift card for someone's stay, or pay with one. | later | Gift cards at checkout |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A7-F1-P1 | **Stay checkout step**: Stay's step on the kit's checkout: the booking summary, full price, cancellation policy and payment schedule, with card numbers handled only by the provider. | interface | small | KIT-A3-F1-P3, STAY-A7-F2-P1, STAY-A4-F5-P3 |  |
| STAY-A7-F1-P2 | **Payment methods**: Stay's needs for the kit's payment provider choice: which cards, bank payments and wallets visitors to NZ use, and what each costs. | research | small | KIT-A3-F1-P1 | [K02](../kit/STATUS.md) |
| STAY-A7-F1-P3 | **Payment records**: Each payment is an operation linked to its booking. | data | small | KIT-A3-F1-P2, `kit-ops`, STAY-A6-F2-P2 |  |
| STAY-A7-F1-P4 | **Who sells the stay**: Stay's case for the kit's seller of record decision: whether the host or the co-op supplies the stay, whose name and GST number go on the tax invoice, who owes consumer guarantees and who answers card disputes, for bookings through Spare Stay, hosts' own pages and booking boxes. For GST the co-op is the deemed supplier of listed services. | legal | small | KIT-A3-F1-P5, KIT-A4-F1-P1, KIT-A18-F8-P1 | [K02](../kit/STATUS.md) |
| STAY-A7-F2-P1 | **Payment schedule**: Stay's schedule on the kit's hold time limits and stage payments: a deposit at booking and the balance charged inside the provider's hold window, with reminders. | service | small | KIT-A3-F2-P4, KIT-A3-F2-P3, STAY-A6-F2-P2 |  |
| STAY-A7-F2-P2 | **Payment plan screens**: Shows what is paid, what is due and when. | interface | small | STAY-A7-F2-P1 |  |
| STAY-A7-F3-P1 | **Stay release point**: Stay's release point on the kit's hold and release rules: the host's share is released a set time after check-in. | integration | small | KIT-A3-F2-P1, STAY-A7-F1-P3, STAY-A7-F3-P3 | [K02](../kit/STATUS.md) |
| STAY-A7-F3-P3 | **Bookings made months ahead**: The rule for stays booked further ahead than the provider can hold money (providers commonly hold funds for at most 90 days, and card authorisations lapse within days): a deposit at booking and the balance inside the hold window, or an earlier payout to the host under published risk rules. | library | small | KIT-A3-F2-P4, KIT-A3-F13-P2, STAY-A7-F2-P1 |  |
| STAY-A7-F4-P1 | **Stay deposit rules**: Stay's rules on the kit's deposits and damage holds: the host sets an amount within a voted cap, held as a card hold or a paid deposit, and card holds on long stays are re-authorised before they lapse. | integration | small | KIT-A3-F10-P1, KIT-A3-F2-P4, KIT-A12-F4-P1 |  |
| STAY-A7-F4-P2 | **Deposit release clock**: Counts the claim window from check-out and releases the deposit when it ends. | service | small | STAY-A7-F4-P1, STAY-A9-F6-P2, STAY-A13-F3-P1, KIT-A15-F14-P1 |  |
| STAY-A7-F5-P1 | **Stay payout timing**: Stay's payout date for each booking on the kit's payout runs: after check-in, or earlier under the rule for bookings made months ahead. | integration | small | KIT-A3-F3-P1, KIT-A3-F3-P2, STAY-A7-F3-P1 | [K02](../kit/STATUS.md) |
| STAY-A7-F5-P3 | **Earnings page**: Stay's lines on the kit's earnings page: each payout by place and booking, with nights, fees, GST and flat-rate credits. | interface | small | KIT-A3-F3-P3, STAY-A7-F5-P1 |  |
| STAY-A7-F6-P1 | **Stay fee inputs**: Stay's inputs to the kit's fee calculator: the voted split between guest and host, and the provider's charges passed through at cost. | library | small | KIT-A3-F4-P1, KIT-A13-F3-P1, STAY-A15-F4-P1 | [K06](../kit/STATUS.md) |
| STAY-A7-F6-P2 | **Fee explained**: Each booking's fee links to that month's cost ledger. | interface | small | STAY-A7-F6-P1, KIT-A13-F3-P2 | [K06](../kit/STATUS.md) |
| STAY-A7-F7-P1 | **Stay's dispute evidence**: Gathers each booking's evidence for the kit's card disputes: what was agreed, the house rules and policy accepted, check-in confirmation and messages. | service | small | KIT-A3-F5-P2, STAY-A6-F2-P2, STAY-A10-F1-P1 |  |
| STAY-A7-F7-P2 | **Failed payment rules**: What happens to a booking when a payment fails, with retries and notices. | service | small | STAY-A7-F2-P1, KIT-A8-F3-P2 |  |
| STAY-A7-F7-P3 | **Losses after payout**: Stay's use of the kit's loss rules: a card dispute or refund that lands after the host was paid becomes a negative balance recovered from later payouts within published limits. Payments are reconciled with the provider daily, and passing card costs through is checked so it is never a banned surcharge. | service | small | KIT-A3-F16-P1, KIT-A3-F16-P2, KIT-A3-F9-P1, KIT-A3-F4-P3 |  |
| STAY-A7-F8-P1 | **Gift cards at checkout**: Accepts the kit's gift cards for stays at checkout. | service | small | KIT-A3-F15-P2, STAY-A7-F1-P1 |  |

## STAY-A8 Changes, cancellations and refunds

A few clear cancellation policies chosen by host vote, changes either side can ask for, and refunds worked out by rule.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A8-F1 Cancellation policies | Hosts pick from a small set of policies that members vote on; guests see the policy before they book. | core | Policy set; Policy versions; Policy vote |
| STAY-A8-F2 Guest cancels | Cancel a booking and see the exact refund before you confirm. | core | Refund calculator; Cancel screens |
| STAY-A8-F3 Change a booking | Ask to change dates or the number of guests and see the new price before the other side agrees. | next | Change requests; Change screens |
| STAY-A8-F4 Host cancels | Cancel when you truly must; the guest gets a full refund and help to rebook. | core | Host cancellation rules; Similar places finder |
| STAY-A8-F5 Emergencies outside anyone's control | Get warned and get a fair refund when an official emergency, such as a tsunami, flood, cyclone or closed road, stops the stay. | core | Emergency refund rule; Official alerts; Handle every affected booking |
| STAY-A8-F6 No-shows and leaving early | Record a no-show or an early departure and settle it by the policy. | next | No-show and early departure |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A8-F1-P1 | **Policy set**: Stay's flexible, moderate and firm policies on the kit's cancellation policies, each with refund dates and amounts. | data | small | KIT-A5-F6-P1 |  |
| STAY-A8-F1-P2 | **Policy versions**: Each booking keeps the policy version it was made under. | data | small | STAY-A8-F1-P1, STAY-A6-F2-P2 |  |
| STAY-A8-F1-P3 | **Policy vote**: Hosts vote on the policy options and the default, under the competition law check, after guests have been heard. | service | small | KIT-A12-F1-P1, KIT-A12-F4-P1, STAY-A15-F2-P3, STAY-A15-F4-P2, STAY-A8-F1-P1 |  |
| STAY-A8-F2-P1 | **Refund calculator**: Works out the refund from the policy, the dates and the fees, including the at-cost fee. | library | small | STAY-A8-F1-P2, STAY-A4-F5-P1 |  |
| STAY-A8-F2-P2 | **Cancel screens**: Shows the refund, asks for a reason and confirms. | interface | small | STAY-A8-F2-P1 |  |
| STAY-A8-F3-P1 | **Change requests**: Stay's change rules on the kit's order changes: new dates or guest numbers are checked against the calendar and stay rules, priced by the price engine, and the difference charged or refunded. | service | small | KIT-A5-F15-P1, STAY-A4-F5-P1, STAY-A3-F2-P2 |  |
| STAY-A8-F3-P2 | **Change screens**: Pick new dates or guests and see the difference. | interface | small | STAY-A8-F3-P1 |  |
| STAY-A8-F4-P1 | **Host cancellation rules**: Full refund for the guest, a recorded reason, and limits on repeat cancellations set by host vote; the guest is moved through the one protection claim flow. | service | small | KIT-A3-F5-P1, KIT-A12-F4-P1, STAY-A13-F2-P2 |  |
| STAY-A8-F4-P2 | **Similar places finder**: Finds similar places for the same dates and price, used by the one protection claim flow, which decides who pays any difference. | service | small | STAY-A5-F1-P2, STAY-A4-F5-P1 |  |
| STAY-A8-F5-P1 | **Emergency refund rule**: A refund rule for stays stopped by official emergencies or closures. | legal | small | STAY-A8-F1-P1 |  |
| STAY-A8-F5-P2 | **Official alerts**: Stay's use of the kit's official alerts: civil defence and weather warnings, plus tsunami, flood, fire and road closure notices, pushed to guests staying in the affected area now and fed to the refund rule. | integration | small | KIT-A10-F22-P1 |  |
| STAY-A8-F5-P3 | **Handle every affected booking**: Applies the refund rule to every booking in the affected area and dates at once, using the kit's disaster mode. | service | medium | STAY-A8-F5-P1, KIT-A10-F22-P2, KIT-A3-F5-P1, STAY-A8-F5-P2 |  |
| STAY-A8-F6-P1 | **No-show and early departure**: Rules and screens to record and settle no-shows and early departures. | service | small | STAY-A8-F1-P2, STAY-A8-F2-P1 |  |

## STAY-A9 The stay

Everything a guest needs from arrival to check-out on one trip page that works without signal, and help when something goes wrong.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A9-F1 Check-in instructions | Get directions, the door code or key box details, Wi-Fi and parking, released early enough to reach your phone before you lose signal. | core | Check-in details; Arrival guide; Directions hand-off; Check-in secrets |
| STAY-A9-F2 House guide | Read the house manual, appliance help, rubbish and recycling days, and the host's local tips. | next | House manual; Local tips map; Evacuation information |
| STAY-A9-F3 Offline trip | See your booking, address, codes and host contact with no signal, as at many baches. | core | Offline trip copy |
| STAY-A9-F4 Smart locks | Get a door code that works only for your dates. | next | Smart lock links; Lock maker research |
| STAY-A9-F5 Report a problem | Tell the host about a problem with photos, and get a refund or a move if it isn't fixed. | core | Problem reports |
| STAY-A9-F6 Check-out | Get the check-out steps and confirm you have left. | core | Check-out steps; Stay complete |
| STAY-A9-F7 Extend a stay | Ask to add nights while you are there. | next | Extend a stay |
| STAY-A9-F8 Guest register | Hosts keep the guest details a council or another country's law asks for, and nothing more. | later | Guest register |
| STAY-A9-F9 Can't get in | Get help fast at any hour when you can't get in, with a host response deadline and rebooking if it isn't fixed. | core | Locked-out path |
| STAY-A9-F10 Lost property | Report something left behind, and get it back with the postage paid by you. | next | Lost property |
| STAY-A9-F11 Agreed extra charges | Agree small extra charges during or after a stay, such as late check-out or a broken glass, without a claim. | next | Agreed extra charges |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A9-F1-P1 | **Check-in details**: Arrival steps, codes and photos, kept private until they are released, early enough to reach a phone before it loses signal at a remote place. | data | small | STAY-A6-F2-P2, STAY-A9-F1-P4 |  |
| STAY-A9-F1-P2 | **Arrival guide**: A step-by-step arrival guide with photos. | interface | small | STAY-A9-F1-P1 |  |
| STAY-A9-F1-P3 | **Directions hand-off**: Hands directions to the door to the phone's own map app through the kit. | integration | small | KIT-A7-F6-P1 |  |
| STAY-A9-F1-P4 | **Check-in secrets**: Door codes and Wi-Fi passwords are encrypted, shown only to the guest and named team members, hidden after check-out and wiped from offline copies, with a prompt to change them between stays. | service | small | KIT-A22-F5-P5, KIT-A15-F1-P4, KIT-A15-F4-P4 |  |
| STAY-A9-F2-P1 | **House manual**: A host-written guide in sections, with photos, carried in the offline trip copy. | data | small | STAY-A2-F1-P1 |  |
| STAY-A9-F2-P2 | **Local tips map**: The host's local tips placed on a map. | interface | small | KIT-A7-F1-P2, STAY-A9-F2-P1 |  |
| STAY-A9-F2-P3 | **Evacuation information**: Where to go and how to get warnings in a tsunami, flood or fire, from official sources, in every house guide. | data | small | STAY-A9-F2-P1, STAY-A8-F5-P2 |  |
| STAY-A9-F3-P1 | **Offline trip copy**: Trip details, check-in steps, an area map and any house guide kept on the phone for places with no coverage, and wiped after the stay. | service | medium | `kit-sync`, KIT-A15-F4-P4, STAY-A6-F5-P2, STAY-A9-F1-P4 |  |
| STAY-A9-F4-P1 | **Smart lock links**: Makes a code per stay through lock makers' published interfaces, as plugins, preferring codes that work offline for the stay's dates. Smart lock links should move down into the kit, since Spare Rent and Spare Home need them too. | integration | medium | `kit-plugin`, STAY-A6-F2-P2, STAY-A9-F1-P4 |  |
| STAY-A9-F4-P2 | **Lock maker research**: Which locks sold in NZ have published interfaces and terms that allow this. | research | small |  |  |
| STAY-A9-F5-P1 | **Problem reports**: Problems reported with photos and a time, shared with the host and kept as evidence; a problem not fixed in time opens a protection claim. | service | small | KIT-A10-F3-P1, KIT-A11-F2-P1, STAY-A10-F1-P1 |  |
| STAY-A9-F6-P1 | **Check-out steps**: The host's check-out tasks and a button to confirm departure. | interface | small | STAY-A9-F1-P1 |  |
| STAY-A9-F6-P2 | **Stay complete**: Stay's use of the kit's completion and auto-confirm: check-out confirmed, or confirmed on its own at check-out time, which opens reviews and starts the deposit clock. | service | small | KIT-A5-F7-P1, STAY-A6-F2-P2 |  |
| STAY-A9-F7-P1 | **Extend a stay**: Asks for extra nights if they are free, priced by the same rules. | service | small | STAY-A8-F3-P1 |  |
| STAY-A9-F8-P1 | **Guest register**: Collects only the guest details the law of that place requires, and deletes them on time. | service | small | STAY-A6-F2-P2, STAY-A14-F9-P1, KIT-A18-F6-P1 |  |
| STAY-A9-F9-P1 | **Locked-out path**: A 'can't get in' button on the trip page alerts the host and co-hosts with a short response deadline, then reaches the kit's urgent support at any hour, and opens a protection claim with rebooking if it isn't fixed. It is kept apart from safety emergencies. | service | small | KIT-A21-F3-P1, KIT-A8-F3-P2, STAY-A13-F2-P2 |  |
| STAY-A9-F10-P1 | **Lost property**: The guest reports an item, the host confirms whether it was found and keeps it for a set time, and posts it back with postage the guest pays through the app. | service | small | STAY-A10-F1-P1, KIT-A5-F15-P2 |  |
| STAY-A9-F11-P1 | **Agreed extra charges**: Stay's use of the kit's extra charges after the job: the host asks for a late check-out fee, an extra guest or a small breakage with a photo, and the guest pays in one tap or disputes it. | service | small | KIT-A5-F15-P2, STAY-A6-F2-P2 |  |

## STAY-A10 Messages and notifications

Hosts and guests talk in one place, with translation, saved and scheduled messages, scam warnings, calls that hide numbers, and only the alerts each person chooses.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A10-F1 Host and guest chat | Message before booking, during the trip and after, with photos and files. | core | Booking chat; Inbox |
| STAY-A10-F2 Translated messages | Read messages in your own language, with the original one tap away. | next | Message translation |
| STAY-A10-F3 Saved and scheduled messages | Save replies, and schedule messages such as check-in details or a check-out reminder. | next | Saved replies; Scheduled messages |
| STAY-A10-F4 Notification choices | Choose which alerts reach you by push, email or text, and set quiet hours. | core | Alert list |
| STAY-A10-F5 Scam warnings in chat | Get warned when a message asks you to pay outside the app or to click a strange payment link. | core | Stay scam patterns; Contact details hidden before booking |
| STAY-A10-F6 Group chat for a trip | Bring your travel companions into the chat with the host. | later | Trip group chat |
| STAY-A10-F7 Calls without sharing numbers | Call the host or guest before and during the stay without either side seeing the other's number. | next | Masked calls for stays |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A10-F1-P1 | **Booking chat**: Stay's threads on the kit's chat service: one per inquiry and booking, with the host's co-hosts included. | service | small | KIT-A8-F1-P2, STAY-A6-F2-P2 |  |
| STAY-A10-F1-P2 | **Inbox**: An inbox with filters for upcoming, current and past stays. | interface | medium | STAY-A10-F1-P1, KIT-A8-F1-P3 |  |
| STAY-A10-F2-P1 | **Message translation**: Stay's use of the kit's message translation on booking threads, marked as translated, with the original one tap away. | service | small | KIT-A8-F7-P1, STAY-A10-F1-P1 |  |
| STAY-A10-F3-P1 | **Saved replies**: Reusable replies with fields such as the guest's name and dates. | data | small | STAY-A10-F1-P1 |  |
| STAY-A10-F3-P2 | **Scheduled messages**: Messages sent on triggers such as booking, the day before arrival or after check-out, labelled as automatic. Saved and scheduled messages should move down into the kit, since Spare Rent, Spare Home and Spare Work need the same. | service | medium | STAY-A10-F3-P1, `kit-ops`, STAY-A9-F1-P1 |  |
| STAY-A10-F4-P2 | **Alert list**: Every Stay event that can send an alert, with its default, on the kit's notification router and settings. | data | small | KIT-A8-F3-P2, KIT-A8-F5-P1 |  |
| STAY-A10-F5-P1 | **Stay scam patterns**: Stay's patterns for the kit's scam warnings: requests to pay by bank transfer, deposits asked for outside the app, and fake booking links. | service | small | KIT-A8-F9-P1, STAY-A10-F1-P1 |  |
| STAY-A10-F5-P2 | **Contact details hidden before booking**: Hides phone numbers, emails and links until a booking is confirmed. | service | small | STAY-A10-F1-P1 |  |
| STAY-A10-F6-P1 | **Trip group chat**: Companions join the booking chat with their own alerts. | service | small | STAY-A10-F1-P1, STAY-A1-F6-P1 |  |
| STAY-A10-F7-P1 | **Masked calls for stays**: Stay's use of the kit's masked calls from booking until a set time after check-out. | integration | small | KIT-A8-F2-P1, STAY-A6-F2-P2 |  |

## STAY-A11 Host operations and teams

Tools to run one place or a hundred: a daily view, co-hosts with set permissions, cleaning and upkeep, bulk changes and insights.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A11-F1 Today view | See today's arrivals, departures, messages and tasks at a glance. | next | Today screen |
| STAY-A11-F2 Co-hosts and teams | Add co-hosts, cleaners or a manager with only the access they need. | next | Team permissions; Team screens; Payout splits |
| STAY-A11-F3 Cleaning between stays | Schedule cleaning between stays and see when each place is ready. | next | Cleaning tasks; Cleaner's view |
| STAY-A11-F4 Upkeep and supplies | Log repairs, track supplies and keep records for each place. | next | Repair log; Supplies list |
| STAY-A11-F5 Insights | See occupancy, earnings, review scores and how guests find your place. | next | Host insights; Insight screens |
| STAY-A11-F6 Bulk changes | Change prices, rules or settings across many listings at once, and undo it. | next | Bulk edit |
| STAY-A11-F7 Host tax summary | Get a yearly summary of income, fees, GST, nights let and private use to give your accountant. | next | Yearly summary; Mixed-use asset rules |
| STAY-A11-F8 Pay cleaners and co-hosts lawfully | Know whether cleaners and co-hosts paid through the app are employees or contractors, and that the pay, tax and safety rules follow. | next | Stay worker status; Cleaning hazards and safety duties |
| STAY-A11-F9 Owners and managers | Hand a listing to a new owner or manager when a place is sold, and give owners statements for places a manager runs. | later | Listing hand-over; Owner statements |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A11-F1-P1 | **Today screen**: Arrivals, departures, current guests, unread messages and tasks for today. | interface | medium | STAY-A6-F2-P2, STAY-A10-F1-P1, STAY-A11-F3-P1 |  |
| STAY-A11-F2-P1 | **Team permissions**: Stay's team roles per listing on the kit's organisation model and delegated access (calendar, messages, prices and payouts), with a flag for a property manager who lists for owners as a listing intermediary. | service | small | KIT-A1-F7-P1, KIT-A1-F8-P1, STAY-A1-F1-P2 |  |
| STAY-A11-F2-P2 | **Team screens**: Invite team members, set what they can do and remove them. | interface | small | STAY-A11-F2-P1 |  |
| STAY-A11-F2-P4 | **Payout splits**: Splits a payout between owners, co-hosts and cleaners by set shares on the kit's multi-party orders, once the worker status of each role is settled. Moved here from STAY-A7-F5-P2. | service | medium | STAY-A7-F5-P1, STAY-A11-F2-P1, KIT-A5-F11-P1, STAY-A11-F8-P1 |  |
| STAY-A11-F3-P1 | **Cleaning tasks**: Cleaning tasks made from check-outs and given to a cleaner, with checklists and the place's hazard notes. | service | medium | STAY-A6-F2-P2, STAY-A11-F2-P1, KIT-A24-F5-P1 |  |
| STAY-A11-F3-P2 | **Cleaner's view**: A simple phone view for cleaners: jobs, checklists, photos and a ready button. | interface | medium | STAY-A11-F3-P1 |  |
| STAY-A11-F4-P1 | **Repair log**: Problems, repairs and costs for each place, linked to guest reports. | data | small | STAY-A9-F5-P1 |  |
| STAY-A11-F4-P2 | **Supplies list**: Tracks linen, toiletries and other stock. | data | small | STAY-A2-F1-P1 |  |
| STAY-A11-F5-P1 | **Host insights**: Occupancy, average nightly price, earnings and review trends for each place. | service | medium | STAY-A6-F2-P2, STAY-A7-F5-P3, STAY-A12-F1-P1 |  |
| STAY-A11-F5-P2 | **Insight screens**: Charts and tables, with export. | interface | medium | STAY-A11-F5-P1 |  |
| STAY-A11-F6-P1 | **Bulk edit**: Applies one change to many listings as one operation that can be undone. | service | medium | `kit-ops`, STAY-A2-F1-P1, STAY-A4-F1-P1 |  |
| STAY-A11-F7-P1 | **Yearly summary**: Stay's summary on the kit's yearly statements: income, fees, GST, flat-rate credits and nights per place, plus private-use, rented and unused days for the mixed-use asset rules. | service | small | KIT-A4-F4-P1, STAY-A7-F5-P3, STAY-A3-F1-P4, STAY-A11-F7-P2 |  |
| STAY-A11-F7-P2 | **Mixed-use asset rules**: How IRD's mixed-use asset rules apply to baches: they apply when a place has private and rented use and is unused for 62 days or more in the year; owners earning under $4,000 from it can opt out, but not companies. To confirm against IRD's guidance. | research | small |  |  |
| STAY-A11-F8-P1 | **Stay worker status**: Stay's facts for the kit's worker status test for cleaners and co-hosts paid through the app, with the result declared in Stay's settings, which switches on the matching pay and tax rules; decided with a lawyer before any payout split pays a worker. | legal | small | KIT-A23-F1-P1, KIT-A23-F1-P2 |  |
| STAY-A11-F8-P2 | **Cleaning hazards and safety duties**: Stay's facts for the kit's health and safety duties for cleaners, with hazard notes on each place, such as steep access, chemicals or a dog, shown before a job is accepted. | library | small | KIT-A24-F1-P1, KIT-A24-F5-P1, STAY-A11-F3-P1 |  |
| STAY-A11-F9-P1 | **Listing hand-over**: Moves a listing, its calendar and future bookings to a new owner or manager, with a published rule on whether reviews stay with the place, and each side keeping its own records. | service | small | STAY-A2-F1-P1, STAY-A19-F6-P1 |  |
| STAY-A11-F9-P2 | **Owner statements**: Monthly statements for owners whose places a manager runs: bookings, payouts, fees, GST and costs. | service | small | STAY-A7-F5-P3, STAY-A11-F2-P1, `engine-pdf` |  |

## STAY-A12 Reviews

Two-way reviews from real stays only, published together so neither side can retaliate, with fair published rules for removal.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A12-F1 Write a review | After check-out, guests review the place and hosts review the guest. | core | Stay review categories; Review screens; Review window |
| STAY-A12-F2 Published together | Reviews stay hidden until both sides have written one or the window closes. | core | Release together |
| STAY-A12-F3 Reply to a review | Reply once, in public, to a review about you. | next | Public replies |
| STAY-A12-F4 Review rules and removals | Report a review that breaks the published rules, and see why any review was removed. | core | Stay review rules; Report and decide |
| STAY-A12-F5 Badges from open rules | Earn badges, such as for reliable hosting, from criteria anyone can read. | next | Open badges; Badge criteria vote |
| STAY-A12-F6 Review summary | See a place's strengths and common complaints at a glance. | later | Review summary |
| STAY-A12-F7 Fair ratings | Not be marked down for storms or closed roads, and know the rating levels before any pause. | next | Stay rating exclusions; Stay rating thresholds |
| STAY-A12-F8 Reviews in my language | Read reviews in your own language, with the original one tap away. | next | Review translation |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A12-F1-P1 | **Stay review categories**: Stay's categories (cleanliness, accuracy, check-in, communication, location and value) on the kit's ratings service, each review tied to one completed stay. | data | small | KIT-A9-F1-P1, STAY-A6-F2-P2 |  |
| STAY-A12-F1-P2 | **Review screens**: A short review form with category ratings and private feedback. | interface | small | STAY-A12-F1-P1, KIT-A9-F1-P2 |  |
| STAY-A12-F1-P3 | **Review window**: A set number of days to review, with reminders. | service | small | STAY-A9-F6-P2, KIT-A8-F3-P2, KIT-A15-F14-P1 |  |
| STAY-A12-F2-P1 | **Release together**: Stay's review window and the rule that both reviews release at once, on the kit's ratings service, so neither side can retaliate. | service | small | KIT-A9-F1-P1, STAY-A12-F1-P3 |  |
| STAY-A12-F3-P1 | **Public replies**: One public reply per review, on the kit's reviews. | service | small | KIT-A9-F2-P1 |  |
| STAY-A12-F4-P1 | **Stay review rules**: Stay's additions to the kit's fair review rules: no reviews used as threats, nothing about protected characteristics, and nothing about other guests. | legal | small | KIT-A9-F2-P2 |  |
| STAY-A12-F4-P2 | **Report and decide**: Stay's use of the kit's rating challenge: reports go to moderators and panels that are not all hosts, and each decision and its rule go on the public log. | service | small | KIT-A9-F5-P1, STAY-A13-F4-P3, STAY-A20-F2-P1 |  |
| STAY-A12-F5-P1 | **Open badges**: Badges from published criteria such as reply time, cancellations and ratings. | service | small | STAY-A12-F1-P1, STAY-A6-F2-P2 |  |
| STAY-A12-F5-P2 | **Badge criteria vote**: Members vote on what each badge needs. | service | small | KIT-A12-F3-P1, STAY-A12-F5-P1 |  |
| STAY-A12-F6-P1 | **Review summary**: Category scores and an AI summary marked as AI, linked to the reviews it came from. | service | small | STAY-A12-F1-P1, KIT-A17-F6-P2 |  |
| STAY-A12-F7-P1 | **Stay rating exclusions**: Stay's cases for the kit's rating exclusions, such as storms, power cuts and closed roads, set aside so they don't count against the host or the guest. | library | small | KIT-A9-F3-P1, STAY-A12-F1-P1 |  |
| STAY-A12-F7-P2 | **Stay rating thresholds**: Stay's published, voted rating levels on the kit's rating thresholds, with a warning and time to improve before any pause. | library | small | KIT-A9-F4-P1, STAY-A12-F1-P1 |  |
| STAY-A12-F8-P1 | **Review translation**: Stay's use of the kit's machine translation for reviews, marked as translated. | service | small | KIT-A19-F10-P2, STAY-A12-F1-P1 |  |

## STAY-A13 Trust, safety and damage

Verified people, fraud and scam checks, help in an emergency, protection when a stay goes wrong, fair damage claims, blocking, and disputes decided by panels that are fair to both sides.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A13-F1 Help in an emergency | Call 111 or the local emergency number from the app, and reach urgent safety help at any hour during a stay. | core | Emergency button; Urgent safety support; Sexual harm path in Stay |
| STAY-A13-F2 Guest protection | Get a refund or help to rebook if you can't get in, the place isn't as described, it isn't safe or the host cancels. | core | Guest protection rules; Protection claims; Who pays for protection; Hidden camera found |
| STAY-A13-F3 Damage claims | Claim for damage with photos and receipts; the guest can reply; a fair panel decides if you can't agree. | core | Damage claims; Claim screens; Damage cover and insurance law; Settle before a panel; Published damage rules |
| STAY-A13-F4 Disputes and member panels | Take a disagreement about money, the listing or the stay to a panel that is fair to hosts and guests alike, or to an outside tribunal. | core | Stay dispute types; Stay panels; Fair panel make-up; Outside routes for stays |
| STAY-A13-F5 Fraud and scam checks | Be protected from fake listings, stolen cards and taken-over accounts. | core | Stay fraud signals; Proven scam defences; Ban evasion checks |
| STAY-A13-F6 Neighbours and parties | Neighbours can report a problem at a listing now, and party and noise rules are enforced. | next | Neighbour reports; Party rule |
| STAY-A13-F7 Fair treatment | Book and host without discrimination, under a commitment every member accepts. | core | Fair treatment commitment; Decline patterns; What hosts may choose |
| STAY-A13-F8 Host liability cover | Get liability cover for injury to guests, arranged at cost. | later | Liability cover research |
| STAY-A13-F9 Overstays and threatening guests | Know the steps when a guest won't leave or turns threatening, and get support as a host. | next | Overstay and threat steps |
| STAY-A13-F10 Spot exploitation | Learn the warning signs of human trafficking and child exploitation, and report them to NZ Police. | next | Exploitation warning signs |
| STAY-A13-F11 Block someone | Block a guest or a host, and report or block from any message. | core | Stay blocking; Report and block from chat |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A13-F1-P1 | **Emergency button**: Stay's use of the kit's emergency button, with the stay's address on screen to read out. | interface | small | KIT-A10-F1-P1, STAY-A6-F5-P2 |  |
| STAY-A13-F1-P2 | **Urgent safety support**: Stay's urgent cases for the kit's urgent safety lane during stays. | service | small | KIT-A10-F1-P2, KIT-A21-F3-P1 |  |
| STAY-A13-F1-P3 | **Sexual harm path in Stay**: The kit's sexual harm report path, reachable from the trip page, chat and the emergency screen, for guests, hosts and cleaners, with evidence kept for police only if the person chooses. | service | small | KIT-A10-F18-P2, STAY-A6-F5-P2 |  |
| STAY-A13-F2-P1 | **Guest protection rules**: What counts (can't get in, not as described, not safe, a hidden camera, the host cancels), how soon to report it, and the refund or rebooking given. | legal | small | KIT-A18-F8-P1 |  |
| STAY-A13-F2-P2 | **Protection claims**: The one claim flow for refunding or moving a guest: a problem report, a host cancellation or a lock-out opens it, the host gets a chance to fix it, then a refund or rebooking follows, paid as the funding rule says. It replaces the separate escalation in STAY-A9-F5. | service | medium | STAY-A13-F2-P1, STAY-A13-F2-P3, STAY-A8-F4-P2, KIT-A3-F5-P1, KIT-A11-F1-P1 |  |
| STAY-A13-F2-P3 | **Who pays for protection**: Who pays for refunds when the host is not at fault, the price difference when a guest is moved, and goodwill credits: the host when at fault, otherwise the kit's protection pool or a published reserve line set by member vote, with a cap per booking and per year. | legal | small | KIT-A11-F10-P1, KIT-A13-F3-P5, KIT-A3-F16-P1 |  |
| STAY-A13-F2-P4 | **Hidden camera found**: When a guest finds an undisclosed camera: an urgent report, the listing frozen at once, a full refund and rebooking, and police if needed. | service | small | STAY-A2-F6-P2, STAY-A13-F2-P2, KIT-A10-F3-P1 |  |
| STAY-A13-F3-P1 | **Damage claims**: Stay's claims on the kit's disputes and evidence store: the claim, photos and receipts, the guest's reply and time limits under the published damage rules, with any deposit held until it is decided. | service | medium | KIT-A11-F1-P1, KIT-A11-F2-P1, STAY-A9-F6-P2, STAY-A13-F3-P5 |  |
| STAY-A13-F3-P2 | **Claim screens**: Screens for both sides to add photos, receipts and replies. | interface | small | STAY-A13-F3-P1 |  |
| STAY-A13-F3-P3 | **Damage cover and insurance law**: Stay's case for the kit's insurance options: a co-op pool or an outside insurer for damage beyond the deposit, compared on cost; whether a co-op pool is insurance business needing a Reserve Bank licence under the Insurance (Prudential Supervision) Act 2010; and what financial advice rules apply if the co-op arranges cover for hosts. | research | medium | KIT-A11-F9-P1, KIT-A11-F10-P1 | [ST03](STATUS.md) |
| STAY-A13-F3-P4 | **Settle before a panel**: Stay's use of the kit's settlement offers: either side offers an amount, and an accepted offer moves the held money at once. | library | small | KIT-A11-F3-P1, STAY-A13-F3-P1 |  |
| STAY-A13-F3-P5 | **Published damage rules**: Wear and tear versus damage, depreciation of items, the evidence needed, time limits and caps, with a short course for panel members who decide damage claims. | legal | small | KIT-A25-F2-P1 |  |
| STAY-A13-F4-P1 | **Stay dispute types**: Stay's dispute types (refunds, damage and listing accuracy) and deadlines on the kit's disputes service, with money held until a decision. | service | small | KIT-A11-F1-P1, KIT-A11-F6-P1 |  |
| STAY-A13-F4-P2 | **Stay panels**: Stay's panels on the kit's panel selection, made up by the fair panel rule, with one appeal. | service | small | KIT-A11-F4-P1, KIT-A11-F5-P1, STAY-A13-F4-P3 |  |
| STAY-A13-F4-P3 | **Fair panel make-up**: While only hosts are members, panels that decide host-against-guest disputes, review removals and protection claims include guest-side members or independent adjudicators, with conflict checks so no host decides a case about a competitor or a friend. | legal | small | KIT-A10-F20-P1, KIT-A11-F4-P1 |  |
| STAY-A13-F4-P4 | **Outside routes for stays**: Stay's guidance on the kit's outside routes: the Disputes Tribunal for host and guest disputes, the Tenancy Tribunal where a long stay is a tenancy, and an evidence bundle either side can export. | data | small | KIT-A11-F7-P1, KIT-A11-F4-P5, KIT-A11-F2-P1 |  |
| STAY-A13-F5-P1 | **Stay fraud signals**: Stay's signals for the kit's fraud signals: a place already listed by someone else, a host who doesn't match the address, and bookings made far ahead on new cards. | service | small | KIT-A10-F12-P2, KIT-A3-F13-P1, KIT-A3-F3-P4 |  |
| STAY-A13-F5-P2 | **Proven scam defences**: What other marketplaces do against rental scams, and what fits a co-op, as Stay's input to the kit's scam methods. | research | small | KIT-A10-F12-P1 |  |
| STAY-A13-F5-P3 | **Ban evasion checks**: Spots a banned host or guest coming back under a new account or a family member's name, using the kit's one-membership check and restrictions across apps, for a person to review. | service | small | KIT-A10-F14-P1, KIT-A14-F1-P4, KIT-A10-F12-P2 |  |
| STAY-A13-F6-P1 | **Neighbour reports**: A public form and number for neighbours to report a current problem at a listing. | service | small | KIT-A10-F3-P1, KIT-A8-F4-P2 |  |
| STAY-A13-F6-P2 | **Party rule**: No unapproved parties, with risk checks on short local bookings over busy dates that use booking facts, never age alone, and are checked for indirect age discrimination. | legal | small | KIT-A10-F5-P1, STAY-A13-F7-P3 |  |
| STAY-A13-F7-P1 | **Fair treatment commitment**: Stay's use of the kit's non-discrimination commitment under the Human Rights Act, accepted by every member. | legal | small | KIT-A10-F19-P1 |  |
| STAY-A13-F7-P2 | **Decline patterns**: Stay's use of the kit's decline pattern checks on booking requests, listed on the AI use register, with a person's review. | service | small | KIT-A6-F5-P3, STAY-A6-F3-P1 |  |
| STAY-A13-F7-P3 | **What hosts may choose**: Whether hosts may set a minimum guest age, and what the Human Rights Act allows a host who shares their own home to choose about guests; checked with a lawyer. | legal | small | KIT-A10-F19-P1 |  |
| STAY-A13-F8-P1 | **Liability cover research**: Options for host liability insurance in NZ and abroad, with costs, as Stay's input to the kit's insurance options. | research | medium | KIT-A11-F9-P1 |  |
| STAY-A13-F9-P1 | **Overstay and threat steps**: Plain steps for a guest who overstays or refuses to leave, and for a threatening guest: when to call 111, trespass notices, and support for the host, who often lives next door; checked with a lawyer. | legal | small | KIT-A10-F1-P1, KIT-A21-F3-P1 |  |
| STAY-A13-F10-P1 | **Exploitation warning signs**: A short course for hosts and cleaners on the warning signs of human trafficking and child exploitation, and a reporting route to NZ Police. | service | small | KIT-A25-F2-P1, KIT-A10-F3-P1 |  |
| STAY-A13-F11-P1 | **Stay blocking**: Stay's use of the kit's blocking: a blocked guest can't find, request or message that host's places, and a blocked host's places don't show to that guest. | service | small | KIT-A10-F4-P1, STAY-A5-F1-P2 |  |
| STAY-A13-F11-P2 | **Report and block from chat**: Stay's use of the kit's report and block on every message in booking threads. | interface | small | KIT-A8-F6-P1, STAY-A10-F1-P1 |  |

## STAY-A14 Local rules, tax and legal duties

Council rules, GST, income reporting, levies and consumer law built in as rules, NZ first, with each new country added as data.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A14-F1 Council rules | See your council's short-stay rules for a place and record your registration or consent. | core | Council rule research; Council rule data; Registration numbers; Campground registration; Right to host |
| STAY-A14-F2 Night caps and council records | Track nights let against a council's cap, and share records with the council when the law asks. | core | Night cap tracking; Council records |
| STAY-A14-F3 GST on stays | Have GST collected correctly on every booking, with flat-rate credits passed to hosts who aren't GST registered. | core | GST on stays rules; Flat-rate credit statements; GST rules research; Large owner opt-outs; Listing intermediaries |
| STAY-A14-F4 Income reporting | Have your income reported to IRD as the law requires, and see exactly what was sent. | core | Rental reporting details; Your copy |
| STAY-A14-F5 Levies and tourist taxes | Have local levies and tourist taxes added and paid where the law asks the platform to collect them. | next | Levy rules; Levy payments |
| STAY-A14-F6 Anti-money-laundering | Keep the co-op within anti-money-laundering law for online marketplaces. | core | Stay's money-laundering risks |
| STAY-A14-F7 Terms and consumer law | Read plain terms for guests and hosts, with the terms each booking was made under kept. | core | Plain-language terms; Consumer law duties; No parity or exclusivity |
| STAY-A14-F8 Longer stays | Stay a month or more, with the app checking whether tenancy law applies. | later | Where a stay becomes a tenancy; Monthly billing; Host rental agreement |
| STAY-A14-F9 Other countries | Open Spare Stay in another country by adding its rules as data: taxes, registration, reporting and caps. | later | Country rule packs; EU short-stay rules; Australian state rules; EU accessibility and platform duties |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A14-F1-P1 | **Council rule research**: Each main NZ council's registration, consent, night cap, rates and levy rules, with sources. | research | medium |  | [ST01](STATUS.md) |
| STAY-A14-F1-P2 | **Council rule data**: Each council's rules as data the app can apply. | data | medium | STAY-A14-F1-P1 | [ST01](STATUS.md) |
| STAY-A14-F1-P3 | **Registration numbers**: Hosts enter registration or consent numbers; a listing that needs one can't go live without it. | service | small | STAY-A14-F1-P2, STAY-A2-F1-P4 | [ST01](STATUS.md) |
| STAY-A14-F1-P4 | **Campground registration**: A campsite or glamping listing used by 2 or more parties for reward needs the council's certificate of registration under the Camping-Grounds Regulations 1985 before it can go live. | legal | small | STAY-A14-F1-P3, STAY-A2-F1-P3 | [ST01](STATUS.md) |
| STAY-A14-F1-P5 | **Right to host**: Each host declares they own the place, or rent it with the landlord's written consent; body corporate rules on noise, security and common areas are shown as house rules; and the host confirms their home insurer knows about paid guests. | legal | small | STAY-A2-F3-P2 |  |
| STAY-A14-F2-P1 | **Night cap tracking**: Counts nights let each year and warns, then closes the calendar, at a cap. | service | small | STAY-A3-F1-P1, STAY-A14-F1-P2 | [ST01](STATUS.md) |
| STAY-A14-F2-P2 | **Council records**: Records a council may see, shared only as its rules require. | service | medium | STAY-A14-F2-P1 | [ST01](STATUS.md) |
| STAY-A14-F3-P1 | **GST on stays rules**: Stay's cases for the kit's GST engine: short stays as listed services, the platform's 15% GST, 8.5% flat-rate credits for unregistered hosts, opt-outs and listing intermediaries. Local levies are worked out separately. | library | small | KIT-A4-F1-P2, KIT-A4-F1-P1 |  |
| STAY-A14-F3-P2 | **Flat-rate credit statements**: Stay's statements on the kit's flat-rate credit notices, at least monthly, with lines per listing, sent to the owner or, where a listing intermediary has taken on GST, to that intermediary. | service | small | KIT-A4-F1-P3, STAY-A14-F3-P1, STAY-A14-F3-P6 |  |
| STAY-A14-F3-P4 | **GST rules research**: How GST applies to everything that is not a night's stay: cancellation fees, no-show charges, forfeited deposits, damage payments, extras such as firewood or linen hire, and trip extras or experiences that may not be listed services; from IRD's guidance. | research | small | KIT-A4-F1-P1 |  |
| STAY-A14-F3-P5 | **Large owner opt-outs**: Records opt-out notices from non-individual owners with more than $500,000 of taxable supplies, and written opt-out agreements for owners with more than 2,000 nights through one marketplace, or hostels and motels with more than $60,000 that were GST-registered before 1 April 2024. Then switches the GST engine, invoices and flat-rate credits for that owner, on the kit's opt-out agreements. | service | small | KIT-A4-F10-P1, STAY-A14-F3-P1 |  |
| STAY-A14-F3-P6 | **Listing intermediaries**: Where a property manager lists for owners and agrees to collect and pay GST itself, records the agreement and each owner's GST status, and has the manager run flat-rate credits for unregistered owners, as IRD's listing intermediary rules set out. | service | small | KIT-A4-F1-P1, STAY-A1-F1-P2, STAY-A14-F3-P1 |  |
| STAY-A14-F4-P1 | **Rental reporting details**: Stay's details for the kit's yearly IRD report: each listing's address and land title or registration number, days rented per listing, the host's tax number and date of birth for individuals, checks done by the deadline, and the excluded-seller test for listings rented more than 2,000 times; confirmed against IRD's guidance. | service | small | KIT-A4-F3-P2, KIT-A4-F3-P3, STAY-A1-F4-P1 |  |
| STAY-A14-F4-P2 | **Your copy**: Hosts see and download what was reported about them. | interface | small | STAY-A14-F4-P1, KIT-A4-F3-P3 |  |
| STAY-A14-F5-P1 | **Levy rules**: Each place's levies and tourist taxes as data for the kit's tax rules per country, such as Victoria's short stay levy. The levy engine itself should move down into the kit's tax service. | data | small | KIT-A4-F7-P1, KIT-A19-F6-P1 |  |
| STAY-A14-F5-P2 | **Levy payments**: Collects levies and pays them to the right office. | integration | medium | STAY-A14-F5-P1, KIT-A3-F1-P2 |  |
| STAY-A14-F6-P1 | **Stay's money-laundering risks**: Stay's risks for the co-op's one anti-money-laundering programme: high-value bookings paid by someone else, refunds asked to a different card, and long stays paid far ahead. | legal | small | KIT-A4-F5-P1 | [K02](../kit/STATUS.md) |
| STAY-A14-F7-P1 | **Plain-language terms**: Guest terms and host terms written plainly, on the kit's terms and privacy policy. | legal | medium | KIT-A18-F4-P2 |  |
| STAY-A14-F7-P3 | **Consumer law duties**: Stay's cases for the kit's consumer law duties: what the Fair Trading Act and Consumer Guarantees Act ask of the platform and of hosts. | research | medium | KIT-A18-F8-P1 | [MK02](../spare-market/STATUS.md) |
| STAY-A14-F7-P4 | **No parity or exclusivity**: Spare Stay never asks hosts for price parity or exclusivity, so they can list anywhere at any price. | legal | small | STAY-A14-F7-P1 |  |
| STAY-A14-F8-P1 | **Where a stay becomes a tenancy**: When a long stay falls under the Residential Tenancies Act, with sources. | research | small | KIT-A26-F3-P2 | [RT01](../spare-rent/STATUS.md) |
| STAY-A14-F8-P2 | **Monthly billing**: Monthly payments and rules for long stays that remain holiday stays. | service | medium | STAY-A7-F2-P1, STAY-A14-F8-P1 |  |
| STAY-A14-F8-P3 | **Host rental agreement**: A host's own rental agreement for long stays or larger groups, signed through the kit's signatures. | service | small | KIT-A25-F1-P2, STAY-A14-F8-P1 |  |
| STAY-A14-F9-P1 | **Country rule packs**: One pack per country: taxes, registration, reporting, guest register and consumer rules. | data | large | KIT-A19-F6-P1, KIT-A4-F7-P1, STAY-A14-F1-P2 |  |
| STAY-A14-F9-P2 | **EU short-stay rules**: The registration numbers and data sharing EU rules have asked of platforms since 20 May 2026. | research | medium |  |  |
| STAY-A14-F9-P3 | **Australian state rules**: State registers, levies and caps in Australia, the nearest next market. | research | medium |  |  |
| STAY-A14-F9-P4 | **EU accessibility and platform duties**: The European Accessibility Act for consumer online services, whose reach into accommodation booking is unsettled, and EU platform duties such as trader traceability and published ranking factors, for the EU rule pack. | research | small | KIT-A18-F13-P1 |  |

## STAY-A15 Co-op membership, votes and the open fee

Hosts are members who own the app, vote on its fees and rules within competition law, hear guests before votes that affect them, and can see what every dollar of the fee paid for.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A15-F1 Join as a member | Join the co-op as a host and become an owner with one vote. | core | Stay members and electorate; Member screens; Holiday-rental co-ops today |
| STAY-A15-F2 Vote on fees and rules | Read proposals, discuss them and vote on Spare Stay's fee, policies and rules. | core | Stay proposal types; Competition law for host votes |
| STAY-A15-F3 See what the fee paid for | Open the monthly cost ledger and see what Spare Stay cost to run. | core | Ledger page; Stay cost lines |
| STAY-A15-F4 Who pays the fee | Members decide whether guests, hosts or both pay the fee, and everyone sees the result. | core | Fee split setting; Guest voice |
| STAY-A15-F5 Guests as members | Guests can join too and have a say on matters that affect them. | later | Guest membership |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A15-F1-P1 | **Stay members and electorate**: Whether every host must be a member, how property management companies and co-hosts count, and who is in Stay's electorate, on the kit's join flow, member register and electorates. | data | small | KIT-A14-F1-P3, KIT-A14-F2-P1, KIT-A12-F5-P2 | [K01](../kit/STATUS.md) |
| STAY-A15-F1-P2 | **Member screens**: Join, see your membership and leave. | interface | small | STAY-A15-F1-P1, KIT-A14-F3-P1 |  |
| STAY-A15-F1-P3 | **Holiday-rental co-ops today**: How existing holiday-rental co-ops run and how they are doing, with sources, adding to the kit's platform co-op study. | research | small | KIT-A14-F10-P1 | [ST04](STATUS.md) |
| STAY-A15-F2-P1 | **Stay proposal types**: Stay's proposal types (the fee split, cancellation policies, badges and house rule limits) on the kit's proposals, ballots and results. | service | small | KIT-A12-F1-P1, KIT-A12-F3-P1, KIT-A12-F4-P1, STAY-A15-F2-P3 |  |
| STAY-A15-F2-P3 | **Competition law for host votes**: Hosts compete with each other, so votes on cancellation policies, discounts, the fee split or anything touching price are checked on the kit's competition law check, so they never become cartel provisions under the Commerce Act. | legal | small | KIT-A12-F4-P3 |  |
| STAY-A15-F3-P1 | **Ledger page**: Stay's monthly costs, bookings and the fee they set, on the kit's ledger pages. | interface | small | KIT-A13-F1-P3 | [K06](../kit/STATUS.md) |
| STAY-A15-F3-P2 | **Stay cost lines**: Stay's running costs and bookings in the kit's fee setting, so the fee is last period's cost divided by bookings; paid marketing counts as a cost only if members vote for it. | library | small | KIT-A13-F3-P1, KIT-A13-F2-P2 | [K06](../kit/STATUS.md) |
| STAY-A15-F4-P1 | **Fee split setting**: A voted setting for guest, host or a split, applied by the price engine. | data | small | STAY-A15-F3-P2, STAY-A4-F5-P1, KIT-A12-F4-P1, STAY-A15-F4-P2 |  |
| STAY-A15-F4-P2 | **Guest voice**: Until guests can join, guests are consulted before any vote that changes what they pay or how disputes are decided, and their views are published with the result. | service | small | KIT-A12-F11-P2 |  |
| STAY-A15-F5-P1 | **Guest membership**: Whether and how guests join, and which votes they get. | research | small | KIT-A12-F5-P2 | [K01](../kit/STATUS.md) |

## STAY-A16 Help and support

Answers first, then people: a help centre, support requests linked to bookings, and a phone line for urgent problems, all run at cost.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A16-F1 Help centre | Search plain answers to common questions in your own language. | core | Stay help topics; Article editor |
| STAY-A16-F2 Contact support | Open a support request about a booking and follow it to an answer. | core | Support requests |
| STAY-A16-F3 AI helper | Ask an AI helper about your booking or listing, with a person one tap away. | next | AI helper |
| STAY-A16-F4 Other ways to reach support | Reach a person by phone, email or NZ Relay. | core | Stay support channels |
| STAY-A16-F5 Host community | Ask other hosts for advice in member forums. | later | Host forums |
| STAY-A16-F6 Status and complaints | See whether Spare Stay is down, and complain about the co-op itself. | next | Stay status and complaints |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A16-F1-P1 | **Stay help topics**: Stay's articles on the kit's help centre, written plainly and translated. | interface | small | KIT-A21-F1-P1 |  |
| STAY-A16-F1-P2 | **Article editor**: Staff and members write and update articles, with review. | interface | small | KIT-A21-F1-P1, STAY-A16-F1-P1 |  |
| STAY-A16-F2-P1 | **Support requests**: Stay's cases on the kit's support cases, linked to bookings, with deadlines. | service | small | KIT-A21-F2-P1, STAY-A6-F2-P2 |  |
| STAY-A16-F3-P1 | **AI helper**: Stay's use of the kit's help assistant: answers from Stay's help centre and your own booking, acting only through Stay's commands, with your consent and a person one tap away. | service | small | KIT-A17-F7-P1, STAY-A16-F1-P1, STAY-A18-F4-P1 |  |
| STAY-A16-F4-P1 | **Stay support channels**: Stay's use of the kit's other support channels: phone with call-back, email, and NZ Relay for Deaf, hard of hearing and speech-impaired members. | integration | small | KIT-A21-F4-P1, STAY-A16-F2-P1 |  |
| STAY-A16-F5-P1 | **Host forums**: Host forums on the kit's member forum, moderated by published rules. | service | small | KIT-A21-F6-P1 |  |
| STAY-A16-F6-P1 | **Stay status and complaints**: Spare Stay on the kit's status page, and the kit's process for complaints about the co-op itself. | service | small | KIT-A21-F5-P1, KIT-A21-F9-P2 |  |

## STAY-A17 Accessibility, languages and regions

Usable by everyone, in their own language, currency and formats: NZ first, ready for any country.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A17-F1 Accessible app | Use every screen with a screen reader, keyboard, larger text or high contrast. | core | Stay accessible screens |
| STAY-A17-F2 Languages | Use Spare Stay in English, te reo Māori and other languages. | core | Te reo Māori |
| STAY-A17-F3 Local formats | See dates, times, prices, addresses and units the way your country writes them. | core | Time zones |
| STAY-A17-F4 Easy read, sign language and light mode | Read key steps in easy read or NZ Sign Language, and use a light mode on a weak signal. | next | Easy read and light mode in Stay |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A17-F1-P1 | **Stay accessible screens**: Stay's hardest screens (maps, calendars and date pickers) built on the kit's accessible components to WCAG 2.2 AA. | interface | medium | KIT-A19-F2-P1 |  |
| STAY-A17-F2-P2 | **Te reo Māori**: Stay's text in te reo Māori, translated and checked by fluent speakers through the kit's member translation. | data | medium | KIT-A19-F1-P1, KIT-A19-F1-P3 |  |
| STAY-A17-F3-P2 | **Time zones**: Check-in times in the place's time zone, including the Chatham Islands, with the guest's own time alongside, on the kit's time-zone engine. | library | small | KIT-A5-F5-P3, KIT-A19-F1-P2 |  |
| STAY-A17-F4-P1 | **Easy read and light mode in Stay**: Stay's key steps (booking, check-in and cancelling) in easy read and NZ Sign Language, and a light mode for weak signal, on the kit's display settings, easy read and light mode. | interface | small | KIT-A19-F8-P1, KIT-A19-F5-P1, KIT-A19-F3-P1 |  |

## STAY-A18 Apps, offline and open access

Phone apps and a web app for guests and hosts, working offline where it matters, with the same commands open to AI agents, plugins and scripts.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A18-F1 Phone apps | Use Spare Stay as a phone app, as a guest or a host. | core | Phone app approach; Guest app; Host app |
| STAY-A18-F2 Web app | Use everything in a web browser on any computer. | core | Web app; Public pages |
| STAY-A18-F3 Work offline | Change your calendar and answer messages offline, and sync when you are back in signal. | next | Offline host changes |
| STAY-A18-F4 Commands, scripts and AI agents | Use every Spare Stay action from a script or an AI agent, through the same commands a person uses, with your approval for anything that costs money. | core | Command and query API; Spare Stay MCP server; Agent guards for listing text; Agent rules for stays |
| STAY-A18-F5 Plugins | Add tools other members have made, such as a cleaning planner or a lock link, in a safe sandbox. | later | Plugin points |
| STAY-A18-F6 Connect my own tools | Have your own cleaning, accounting or other tools told about bookings, changes and check-outs. | next | Booking events |
| STAY-A18-F7 Member-run Stay servers | Run a regional Spare Stay server that shares listings, search and bookings with the main one. | later | Regional servers |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A18-F1-P1 | **Phone app approach**: Stay's needs for the kit's phone app decision: the offline trip copy, check-in codes and background sync for hosts. | research | small | KIT-A20-F2-P1 | [K04](../kit/STATUS.md) |
| STAY-A18-F1-P2 | **Guest app**: Search, booking, trips and messages on the phone. | interface | large | KIT-A20-F2-P2, STAY-A18-F4-P1 |  |
| STAY-A18-F1-P3 | **Host app**: Today, calendar, messages, listings and earnings on the phone. | interface | large | KIT-A20-F2-P2, STAY-A18-F4-P1 |  |
| STAY-A18-F2-P1 | **Web app**: The full guest and host app in the browser, sharing code with the phone apps. | interface | large | KIT-A20-F1-P2, STAY-A18-F4-P1 |  |
| STAY-A18-F2-P2 | **Public pages**: Fast public listing and place pages that work without signing in. | service | medium | STAY-A6-F1-P1 |  |
| STAY-A18-F3-P1 | **Offline host changes**: Queues calendar changes and messages offline and syncs them through the operation log. A booking made online always wins over an offline block or price change, and the host is shown which offline change failed. | service | medium | `kit-sync`, KIT-A15-F4-P3, STAY-A3-F1-P1, STAY-A10-F1-P1 |  |
| STAY-A18-F4-P1 | **Command and query API**: Stay's commands and queries on the kit's command API: the one path every screen, script and agent uses. | library | medium | KIT-A16-F1-P1, KIT-A16-F1-P5, `kit-ops`, `kit-market` |  |
| STAY-A18-F4-P2 | **Spare Stay MCP server**: Stay's commands open to AI agents on the kit's MCP server, with a person's approval before any payment. | service | small | KIT-A17-F1-P1, KIT-A17-F2-P1, STAY-A18-F4-P1 |  |
| STAY-A18-F4-P3 | **Agent guards for listing text**: Listing text, captions, house rules and messages reach agents only as data under the kit's hidden instruction guards, and a listing rule bans text aimed at agents. | library | small | KIT-A17-F5-P1, STAY-A20-F1-P2 |  |
| STAY-A18-F4-P4 | **Agent rules for stays**: Bookings and messages made by agents, including hosts' automatic replies, are labelled; reviews and votes are by people only; agents have spending limits; published rules stop bots grabbing scarce holiday dates; and the API can't be used to harvest addresses or guest details. | library | small | KIT-A17-F9-P1, KIT-A17-F9-P2, KIT-A17-F4-P1, KIT-A6-F4-P3, KIT-A16-F1-P4 |  |
| STAY-A18-F5-P1 | **Plugin points**: Stay's plugin points on the kit's plugin host, each with declared permissions. | service | small | KIT-A16-F5-P2, STAY-A18-F4-P1 |  |
| STAY-A18-F6-P1 | **Booking events**: Stay's booking, change and check-out events on the kit's webhooks, with scoped tokens for each tool. | service | small | KIT-A16-F2-P1, KIT-A16-F1-P2, STAY-A18-F4-P1 |  |
| STAY-A18-F7-P1 | **Regional servers**: How a member-run regional Stay server shares listings, search and bookings with the main one on the kit's connecting member servers, so one calendar never takes two bookings. | research | small | KIT-A22-F7-P2 |  |

## STAY-A19 Your data and privacy

Members own their data: see it, take it with them in open formats, choose who sees it, and delete it.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A19-F1 Export your data | Download everything: profile, listings, photos, bookings, messages, reviews and earnings, in open formats. | core | Listing export |
| STAY-A19-F2 Delete your account | Delete your account and data, keeping only what the law requires and saying what that is. | core | Stay deletion rules |
| STAY-A19-F3 Privacy settings | Choose what others see and what you agree to. | core | Privacy settings; Stay's privacy duties |
| STAY-A19-F4 See who opened your details | See a log of every time staff or a host's team opened your personal details. | next | Access log |
| STAY-A19-F5 Data kept in region | Have your data kept in your own region where the law asks. | next | Data residency |
| STAY-A19-F6 Bookings when a host leaves | Know your booking is honoured, handed to a co-host, or fully refunded with help to rebook if the host leaves, unlists, is banned or dies. | core | Future bookings on host exit |
| STAY-A19-F7 Take my reviews with me | Take your reviews with you as signed files others can check. | next | Portable Stay reviews |
| STAY-A19-F8 Guest data in hosts' hands | Know what hosts may do with your details, that you are told when someone else gave them, and how long each kind of stay data is kept. | core | Host duties for guest data; Notices for data from others; Stay retention schedule |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A19-F1-P2 | **Listing export**: A listing and its calendar in an open format another site can read, added to the kit's data export. | data | small | KIT-A18-F1-P1, STAY-A2-F1-P1, STAY-A3-F3-P1 |  |
| STAY-A19-F2-P1 | **Stay deletion rules**: Stay's steps before the kit's account deletion: open bookings, payouts and disputes are settled first, and future bookings follow the rule for a host who leaves. | service | small | KIT-A18-F2-P1, KIT-A18-F2-P2, STAY-A19-F6-P1 |  |
| STAY-A19-F3-P1 | **Privacy settings**: Profile visibility, marketing consent and data use choices. | interface | small | KIT-A18-F3-P1, KIT-A18-F17-P2 |  |
| STAY-A19-F3-P2 | **Stay's privacy duties**: Stay's cases for the kit's privacy duties: what the Privacy Act 2020 and overseas laws such as the GDPR ask of stays, including breach notices. | legal | small | KIT-A18-F7-P1, KIT-A18-F12-P1 |  |
| STAY-A19-F4-P1 | **Access log**: Every time support or a host's team opens a guest's or host's personal details is logged and shown to that member on the kit's privacy page. | service | small | KIT-A18-F3-P1, `kit-ops` |  |
| STAY-A19-F5-P1 | **Data residency**: Stay's use of the kit's data location, keeping members' data in their region where the law asks. | service | small | KIT-A18-F9-P1 |  |
| STAY-A19-F6-P1 | **Future bookings on host exit**: When a host deletes their account, leaves the co-op, unlists, is banned or dies, each confirmed future booking is honoured, handed to a named co-host, or cancelled with a full refund and rebooking help, and the guest is told. | service | medium | KIT-A18-F2-P1, KIT-A14-F8-P1, KIT-A18-F11-P1, STAY-A8-F4-P1, STAY-A8-F4-P2 |  |
| STAY-A19-F7-P1 | **Portable Stay reviews**: Hosts and guests export their reviews as signed files others can check, on the kit's portable reputation. | library | small | KIT-A9-F7-P1, STAY-A12-F1-P1 |  |
| STAY-A19-F8-P1 | **Host duties for guest data**: Rules hosts accept for the guest data they receive: use it only for the stay, keep it safe, and don't keep it after the retention period. | legal | small | KIT-A18-F4-P2 |  |
| STAY-A19-F8-P2 | **Notices for data from others**: Notices under information privacy principle 3A, from 1 May 2026, when Stay gets details from someone else: a booker naming companions or the main guest, neighbour reports and council records. | legal | small | KIT-A18-F5-P1 |  |
| STAY-A19-F8-P3 | **Stay retention schedule**: How long each kind of stay data is kept, on the kit's retention schedules: check-in codes, the guest register, ID results, messages and evidence. | service | small | KIT-A18-F6-P1 |  |

## STAY-A20 Moderation and administration

Stay's moderators and staff act through the kit's review queue and admin console, with every removal and the rule behind it in the public log. Monitoring and backups are the kit's, for every app.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A20-F1 Moderation | Moderators review flagged listings, photos, reviews and accounts, and act by published rules. | core | Stay review queue; Stay community rules |
| STAY-A20-F2 Public removal log | See what was removed or banned and which rule applied. | core | Public log |
| STAY-A20-F3 Appeals | Appeal any removal or ban to people who weren't part of the decision. | core | Appeals |
| STAY-A20-F4 Admin tools | Staff manage bookings, refunds and listings, with every action logged and open to review. | core | Stay admin actions |
| STAY-A20-F6 Legal requests | Have police and court requests handled by a published process, with a yearly transparency report. | core | Legal request process |
| STAY-A20-F7 Automated checks on the AI register | Know which automated checks can block a booking or a host, and ask for a person to review one. | next | Stay's automated checks |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A20-F1-P1 | **Stay review queue**: Stay's flagged listings, photos, accessibility proofs, reviews and accounts in the kit's review queue. | service | small | KIT-A10-F6-P2 |  |
| STAY-A20-F1-P2 | **Stay community rules**: Stay's listing, conduct and content rules in the kit's rules library, including a ban on listing text aimed at AI agents. | legal | small | KIT-A10-F5-P1 |  |
| STAY-A20-F2-P1 | **Public log**: Stay's listing removals and host and guest bans in the kit's public removal log, with the rule used and no personal details. | service | small | KIT-A10-F7-P1 |  |
| STAY-A20-F3-P1 | **Appeals**: Stay's use of the kit's appeals, heard by a fresh reviewer or a fair panel. | service | small | KIT-A11-F5-P1, STAY-A13-F4-P3 |  |
| STAY-A20-F4-P1 | **Stay admin actions**: Stay's booking, refund and listing actions in the kit's admin console, each a logged operation. | interface | medium | KIT-A22-F6-P1, STAY-A18-F4-P1 |  |
| STAY-A20-F6-P1 | **Legal request process**: Stay's cases for the kit's process for police and court requests (booking records, guest registers and addresses), counted in the kit's transparency reports. | legal | small | KIT-A10-F9-P3, KIT-A10-F15-P1 |  |
| STAY-A20-F7-P1 | **Stay's automated checks**: Party risk checks, fraud signals, decline-pattern flags and automatic listing checks listed on the kit's AI use register, with a person's review on request. | service | small | KIT-A17-F10-P1, KIT-A17-F10-P2 |  |

## STAY-A21 Reach, direct bookings and extras

Bring guests to hosts without paid ranking: a plan for the empty start, hosts' own booking pages, sharing, web search, open feeds, local tourism partners, and extras around the stay.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A21-F1 Share and invite | Share a listing with a link, or invite a friend to join. | core | Share links; Member invites |
| STAY-A21-F2 Found in web search | Have listing and place pages show up when people search the web. | next | Search-friendly pages |
| STAY-A21-F3 Your own booking page | Get your own booking page and web address, taking direct bookings at the same at-cost fee. | next | Direct booking site; Own web address; Booking box |
| STAY-A21-F4 Open listing feed | Let travel search sites and tourism bodies show Spare Stay listings through an open feed. | next | Open listing feed; Where listings can reach |
| STAY-A21-F5 Local tourism partners | Work with regional tourism bodies and visitor centres to reach guests. | later | Tourism partner pages |
| STAY-A21-F6 Extras around the stay | Add services from other Spareware apps, such as a cleaner or a ride from the airport, to your trip. | later | Trip extras; Cross-app add-ons for the kit |
| STAY-A21-F7 Reach from day one | Find guests from the first day without paid ranking, with a fair start for new hosts. | core | Launch reach plan; Fair start for new hosts |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A21-F1-P1 | **Share links**: Links that show a preview card with a photo and the full price. | service | small | STAY-A18-F2-P2, KIT-A20-F9-P1 |  |
| STAY-A21-F1-P2 | **Member invites**: Invite hosts and guests on the kit's invites; any reward only if members vote for it. | service | small | KIT-A14-F7-P1, KIT-A12-F4-P1 |  |
| STAY-A21-F2-P1 | **Search-friendly pages**: Fast pages for each region and listing, with structured data. Search-friendly pages should move down into the kit, since Spare Home, Spare Rent and Spare Market need them too. | service | medium | STAY-A18-F2-P2 |  |
| STAY-A21-F3-P1 | **Direct booking site**: A simple site for each host, booking through the same engine and the same seller of record rule. Own booking sites should move down into the kit, since Spare Market, Spare Food and Spare Work need the same. | interface | medium | STAY-A6-F1-P1, STAY-A6-F2-P1, STAY-A7-F1-P4 |  |
| STAY-A21-F3-P2 | **Own web address**: Connect the host's own domain, with security certificates set up; this should move down into the kit with own booking sites. | integration | small | STAY-A21-F3-P1 |  |
| STAY-A21-F3-P3 | **Booking box**: A booking box hosts can put on a site they already have. | interface | small | STAY-A6-F2-P1, STAY-A7-F1-P4 |  |
| STAY-A21-F4-P1 | **Open listing feed**: A published feed of public listing data and live full prices, free for travel search sites. Open listing feeds should move down into the kit, since Spare Home, Spare Rent and Spare Market need them too. | service | medium | STAY-A18-F2-P2, STAY-A4-F5-P1 |  |
| STAY-A21-F4-P2 | **Where listings can reach**: Which travel search and tourism sites take feeds, and on what terms. | research | small |  |  |
| STAY-A21-F5-P1 | **Tourism partner pages**: Region pages and links for tourism bodies and visitor centres. | interface | small | STAY-A21-F4-P1 |  |
| STAY-A21-F6-P1 | **Trip extras**: Adds services from other Spareware apps, such as an airport ride, a meal or a cleaner, to a trip through the kit's cross-app add-on, so Stay never links to another program. | service | small | STAY-A21-F6-P3, STAY-A6-F5-P2 |  |
| STAY-A21-F6-P3 | **Cross-app add-ons for the kit**: Stay's need for a kit part that adds another app's service to an order (an airport ride, a meal, a cleaner) as a linked order in kit-market. It belongs in the kit and should be built there, not in Stay. | research | small | `kit-market`, KIT-A5-F11-P1 |  |
| STAY-A21-F7-P1 | **Launch reach plan**: A plan for the empty start: which regions open first, whether paid marketing is an allowed running cost (by member vote), and free listing feeds to travel search sites. | research | small | KIT-A13-F3-P1 |  |
| STAY-A21-F7-P2 | **Fair start for new hosts**: New hosts with no reviews rank fairly by published rules, not last, until they have reviews. | library | small | STAY-A5-F5-P1 |  |

## STAY-A22 Experiences

Tours and activities hosted by members and booked alongside stays. A separate marketplace with its own safety law, mapped here as its own area so it can become its own program if members choose.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| STAY-A22-F1 List and book an experience | List a tour or activity by date, with group sizes, a meeting point and what to bring, and book one. | later | Experience listings; Experience booking |
| STAY-A22-F2 Run adventure activities lawfully | Show that a guided adventure activity has passed its safety audit and is registered with WorkSafe. | later | Adventure activity rules; Registration check |
| STAY-A22-F3 Guide checks | Know a guide is who they say and holds the qualifications the activity needs. | later | Guide checks |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| STAY-A22-F1-P1 | **Experience listings**: Experience fields on the kit's listing model and availability: dates and times, group sizes, a meeting point, skill and fitness levels, and what to bring. Moved here from STAY-A21-F6-P2. | data | medium | KIT-A5-F1-P1, KIT-A5-F5-P1 |  |
| STAY-A22-F1-P2 | **Experience booking**: Booking, cancelling and paying for an experience on the kit's order steps and cancellation policies. | service | medium | KIT-A5-F4-P1, KIT-A5-F6-P1, STAY-A22-F1-P1 |  |
| STAY-A22-F2-P1 | **Adventure activity rules**: Under the Health and Safety at Work (Adventure Activities) Regulations 2016, guided adventure activity operators must pass a safety audit and register with WorkSafe; an adventure listing can't go live without a current registration. | legal | small | KIT-A24-F1-P1 |  |
| STAY-A22-F2-P2 | **Registration check**: Records each operator's registration and audit expiry, with reminders from the kit's re-check scheduler. | service | small | KIT-A2-F3-P1, STAY-A22-F2-P1 |  |
| STAY-A22-F3-P1 | **Guide checks**: Identity and qualification checks for guides on the kit's identity check and public register checks. | integration | small | KIT-A2-F1-P2, KIT-A2-F2-P1 |  |

## Retired and moved IDs

These IDs are kept out of use so links never point at the wrong thing. A retired part copied a kit part; parts that rested on it now name the kit part instead.

| Old ID | Now |
| --- | --- |
| STAY-A20-F5 Running the service | Retired: monitoring, backups and usage counts are co-op-wide and belong to the kit (KIT-A22) |
| STAY-A1-F4-P3 Public register checks | Retired: uses KIT-A2-F7-P1 |
| STAY-A1-F5-P2 Security alerts | Retired: uses KIT-A1-F3-P3 |
| STAY-A1-F5-P3 Payout change hold | Retired: uses KIT-A3-F3-P4 |
| STAY-A2-F2-P2 Photo processing | Retired: uses KIT-A15-F7-P1, KIT-A15-F7-P3 |
| STAY-A2-F2-P4 Photo descriptions | Retired: uses KIT-A19-F2-P3 |
| STAY-A2-F2-P5 Floor plans | Moved to STAY-A2-F11-P1 |
| STAY-A5-F1-P3 Search engine choice | Retired: uses KIT-A6-F1-P1 |
| STAY-A5-F2-P2 Map tiles at cost | Retired: uses KIT-A7-F1-P1, KIT-A7-F1-P3 |
| STAY-A5-F5-P2 Ranking explained | Retired: uses KIT-A6-F1-P3 |
| STAY-A7-F3-P2 Rules for holding money | Retired: uses KIT-A3-F2-P2 |
| STAY-A7-F5-P2 Payout splits | Moved to STAY-A11-F2-P4 |
| STAY-A7-F5-P4 Earnings export | Retired: uses KIT-A4-F4-P2 |
| STAY-A7-F6-P3 Fee invoices for hosts | Retired: uses KIT-A4-F9-P3 |
| STAY-A7-F8-P2 Gift card rules | Retired: uses KIT-A3-F15-P1 |
| STAY-A8-F2-P3 Refund payment | Retired: uses KIT-A3-F5-P1 |
| STAY-A9-F5-P2 Escalation | Merged into the one protection claim flow, STAY-A13-F2-P2 |
| STAY-A10-F4-P1 Notification settings | Retired: uses KIT-A8-F5-P1 |
| STAY-A12-F4-P3 Fake review checks | Retired: uses KIT-A9-F8-P1 |
| STAY-A14-F3-P3 The co-op's GST returns | Retired: uses KIT-A4-F9-P2 |
| STAY-A14-F4-P3 Overseas reporting | Retired: uses KIT-A4-F7-P1 |
| STAY-A14-F7-P2 Terms versions | Retired: uses KIT-A18-F4-P1 |
| STAY-A15-F2-P2 Vote screens | Retired: uses KIT-A12-F3-P2 |
| STAY-A16-F2-P2 Support desk | Retired: uses KIT-A21-F2-P2 |
| STAY-A17-F1-P2 Accessibility testing | Retired: uses KIT-A19-F2-P2 |
| STAY-A17-F2-P1 Translation system | Retired: uses KIT-A19-F1-P1 |
| STAY-A17-F2-P3 Other scripts and right-to-left | Retired: uses KIT-A19-F7-P1 |
| STAY-A17-F3-P1 Local formats | Retired: uses KIT-A19-F1-P2 |
| STAY-A18-F1-P4 Phone alerts | Retired: uses KIT-A8-F3-P1 |
| STAY-A19-F1-P1 Data export | Retired: uses KIT-A18-F1-P1 |
| STAY-A20-F1-P3 Report buttons | Retired: uses KIT-A10-F3-P2 |
| STAY-A20-F1-P4 Known illegal content checks | Retired: uses KIT-A10-F10-P2 |
| STAY-A20-F4-P2 Admin roles | Retired: uses KIT-A22-F5-P1 |
| STAY-A20-F5-P1 Monitoring | Retired: uses KIT-A22-F1-P1 |
| STAY-A20-F5-P2 Backups and restores | Retired: uses KIT-A22-F3-P1 |
| STAY-A20-F5-P3 Usage counts without tracking | Retired: uses KIT-A22-F8-P1 |
| STAY-A21-F6-P2 Experiences | Moved to STAY-A22-F1-P1 |
| STAY-A11-F2-P3 Find a local co-host or cleaner | Retired: finding cleaners and co-hosts is Spare Work's job; hosts add the people they found as team members (STAY-A11-F2-P2), and booking a cleaner from another app waits for the kit's cross-app add-on (STAY-A21-F6-P3) |

## Questions for assessing this map and its demo

- Does every search result show the full price, with every fee, tax and levy, before a guest opens the listing?
- Can a host join without leaving the other booking sites, with calendars synced, double bookings caught, and a fair rule when one still happens?
- Is the core set small enough for a first release, and could a host and a guest complete a whole stay with only core features?
- Does the map name every legal duty the platform carries in NZ: GST on listed services, flat-rate credits, opt-outs and listing intermediaries, income reporting to IRD, anti-money-laundering, council rules and campground registration?
- Who holds the money at every step, does the rule for bookings made months ahead stay inside the provider's time limits, and does any part make the co-op hold funds itself?
- Is the seller of record settled for bookings on Spare Stay, on hosts' own pages and through booking boxes?
- Who pays for guest protection and goodwill when the host is not at fault, and is the cap published?
- Is damage cover answered, including whether a co-op pool needs a Reserve Bank licence, or is it still the biggest open risk (piece ST03)?
- While only hosts are members, are panels, review removals and fee votes fair to guests?
- Do host votes and the pricing tools stay clear of cartel provisions under the Commerce Act?
- Is the worker status of cleaners and co-hosts settled before any payout split pays them?
- How will guests find Spare Stay when the big sites hold the demand, and is the reach plan more than hope?
- Are accessibility features proven with photos, and does the app itself meet WCAG 2.2 AA?
- Does every action go through the command API, so AI agents, plugins and scripts can do what a person can, and are agents kept from grabbing scarce dates or harvesting addresses?
- Has every Stay part that copied a kit part become a thin use of it, and has the kit taken on the shared parts Stay marks to move down: people on an order, personal calendar feeds, smart lock links, floor plans, own booking sites, open listing feeds, search-friendly pages, saved and scheduled messages, the levy engine and cross-app add-ons?
- Does the trip page still work at a bach with no signal, and are door codes safe once the stay ends?
- Can a new country be added as a rule pack of data, without new code?
- Does anything in the demo copy an incumbent's names, icons, wording or screen designs?
- Which parts are large and core, and who could take them on first?

## Where this map came from

Checked on 2026-10-18 against the incumbents' published help pages and news, and the rules below, then on 2026-10-19 against an independent review. Not legal advice; anything marked *unverified* in [ARCHITECTURE.md](ARCHITECTURE.md), or marked to verify or confirm in a part, still needs checking.

| Topic | Source |
| --- | --- |
| GST collected by platforms on short stays, and flat-rate credits | [IRD](https://www.ird.govt.nz/sharing-economy/online-marketplaces/gst-on-listed-services/gst-obligations-for-marketplaces) |
| GST opt-outs for large accommodation owners and listing intermediaries | [IRD](https://www.ird.govt.nz/sharing-economy/online-marketplaces/gst-on-listed-services/opt-out-rules-accommodation) |
| Platforms report hosts' income to IRD | [IRD](https://ird.govt.nz/international-tax/exchange-of-information/digital-platform-information) |
| Mixed-use asset rules for holiday homes | [IRD](https://www.ird.govt.nz/topics/rental-income-and-paying-tax/tax-and-expenses-for-holiday-homes/mixed-use-asset-rules-for-holiday-homes) |
| Council registration and night caps (Queenstown) | [QLDC](https://www.qldc.govt.nz/services/rates/short-term-accommodation/) |
| Anti-money-laundering duties for online marketplaces | [DIA](https://www.dia.govt.nz/AML-CFT-Information-for-Online-Marketplaces) |
| Fair Trading Amendment Bill on misleading pricing, introduced May 2026 | [1News](https://www.1news.co.nz/2026/05/13/bill-to-crack-down-on-misleading-pricing-introduced/) |
| EU short-stay rules: registration numbers and data sharing from 20 May 2026 | [EUR-Lex, Regulation (EU) 2024/1028](https://eur-lex.europa.eu/eli/reg/2024/1028/oj), [MCS summary](https://mcs.pt/eu-regulation-2024-1028-on-short-term-rentals-enters-into-application-on-20-may-2026-what-changes-for-alojamento-local-platforms-and-hosts/) |
| Victoria's short stay levy, collected by the platform or host | [State Revenue Office Victoria](https://www.sro.vic.gov.au/short-stay-levy) |
| Accessibility features checked by photo | [Airbnb newsroom](https://news.airbnb.com/accessibility-review) |
| Co-host network, services and experiences | [TechCrunch](https://techcrunch.com/2024/10/16/airbnb-launches-a-network-that-lets-host-hire-other-hosts), [Rental Scale-Up](https://www.rentalscaleup.com/airbnbs-2025-summer-release-from-stays-to-services-and-a-real-life-social-network-in-the-making/) |
| Payment protection only on-platform; optional damage add-on | [Bookabach trust page](https://www.bookabach.co.nz/trust), [Bookabach travel with confidence](https://bookabach.co.nz/l/travel-with-confidence) |
| Loyalty discounts paid from hosts' margins | [Smoobu guide](https://www.smoobu.com/en/guides/booking-com/booking-com-rewards/) |
| Campground registration, adventure activity registration, insurance licensing for a damage pool, privacy principle 3A, the biometric code, evacuation schemes, pool barriers, shared water supplies, food and alcohol rules | The independent review of 2026-10-19; each is marked in its part to verify before anyone relies on it |
