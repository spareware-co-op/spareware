# Spare Food: everything to build

*Stage: Mapping. Mapped top-down on 2026-10-18, then checked the same day against two independent reviews and the kit's own map. Every choice is open until an owner accepts a decision note.*

Spare Food replaces Uber Eats, DoorDash and local apps such as Delivereasy. Food delivery where restaurants pay only the at-cost fee, couriers keep their whole delivery fee and tips, and customers pay menu prices. Restaurants and couriers keep all the profit except running costs, and vote on its fees and rules. This map lists everything a full replacement needs, from the top down: 20 areas, 193 features a person can use, and 448 buildable parts. It covers every journey: signing up, ordering, cooking, delivering, getting paid, things going wrong (including sickness and allergic reactions), safety, access for everyone, other languages and countries, taking your data and your business with you, and the law. It adds only what food needs on top of Spare Kit (see [ARCHITECTURE.md](ARCHITECTURE.md) and the [kit map](../kit/ARCHITECTURE.md)). Where a part rests on the kit, it names the kit part it uses and holds only Food's own rules; Food uses 261 of the kit's 473 parts, listed in the [kit's feature map](../kit/FEATURES.md). Where Food needs a shared part the kit does not have yet, the Food part says the kit should own it, and the questions at the end list them all. IDs are never renumbered or reused: FOOD-A5-F11, FOOD-A10-F4-P3 and FOOD-A18-F4-P2 were retired in the first review, and FOOD-A4-F6-P2 moved to FOOD-A4-F19-P1 in the second. The same map in machine-readable form is [features.json](features.json); the open pieces of work are on the [work board](STATUS.md).

**Priority:** core is needed for a first usable release (115 features), next comes soon after (47), later can wait (31). **Kinds of part:** library, service, interface, data, integration, research, legal. **Depends on** names kit crates (kit-...), kit services (svc-...), shared engines (engine-...), kit parts by ID (KIT-...) or other Food parts by ID. **Piece** links a part to an existing work-board piece.

## Areas

| ID | Area | Features | Parts | What it covers |
| --- | --- | --- | --- | --- |
| FOOD-A1 | [Accounts, sign-up and onboarding](#food-a1-accounts-sign-up-and-onboarding) | 9 | 32 | One Spareware login for customers, restaurants and couriers, with only the checks each role needs before its first order or delivery. |
| FOOD-A2 | [Finding food](#food-a2-finding-food) | 8 | 20 | How a customer finds what to eat: nearby restaurants, search, filters and a map, ranked by published rules with no paid placement. |
| FOOD-A3 | [Menus and catalogue](#food-a3-menus-and-catalogue) | 12 | 26 | Restaurants build and keep their menus up to date, with options, allergens, photos and stock, at the same prices as in store. |
| FOOD-A4 | [Ordering and checkout](#food-a4-ordering-and-checkout) | 19 | 39 | Everything between choosing a dish and paying: the cart, delivery or pickup, address, time, notes and a clear price breakdown. |
| FOOD-A5 | [Restaurant order handling](#food-a5-restaurant-order-handling) | 12 | 26 | The restaurant's live screen: an order arrives with a sound, the kitchen accepts it and sets a time, marks it ready and hands it over. |
| FOOD-A6 | [Courier work](#food-a6-courier-work) | 13 | 37 | The courier's app: go online, see the full pay and distance before accepting, collect, deliver and get paid daily. |
| FOOD-A7 | [Dispatch and timing](#food-a7-dispatch-and-timing) | 7 | 14 | The engine that sends the right courier so they arrive as the food is ready, not before, and keeps every time estimate honest. |
| FOOD-A8 | [Live tracking and messages](#food-a8-live-tracking-and-messages) | 5 | 10 | Everyone can see where the order is up to and talk to each other without sharing phone numbers. |
| FOOD-A9 | [Pricing, fees and deals](#food-a9-pricing-fees-and-deals) | 8 | 17 | How the price is set: the menu price, a delivery fee the couriers vote on, the at-cost platform fee, and optional tips and deals. |
| FOOD-A10 | [Payments, payouts and tax](#food-a10-payments-payouts-and-tax) | 13 | 26 | The money: held by a regulated payment provider, split between the restaurant, the courier, the at-cost fee, sales tax and the provider's charge, paid out daily, and taxed as each country's law requires. |
| FOOD-A11 | [Support, refunds and disputes](#food-a11-support-refunds-and-disputes) | 9 | 23 | When something goes wrong: report it in a few taps, get a fair answer fast, and appeal to a member panel. |
| FOOD-A12 | [Ratings, trust and safety](#food-a12-ratings-trust-and-safety) | 14 | 31 | Two-way ratings, safety tools for couriers and customers, and checks that keep bad actors out, with every decision logged in public. |
| FOOD-A13 | [Restaurant business tools](#food-a13-restaurant-business-tools) | 8 | 11 | What a restaurant needs to run its delivery trade: reports, payouts, feedback, help, and its own ordering link at the same at-cost fee. |
| FOOD-A14 | [More than restaurant meals](#food-a14-more-than-restaurant-meals) | 7 | 19 | The other things the big apps deliver: groceries and convenience goods, alcohol with age checks, catering, business accounts, and couriers for other sellers. |
| FOOD-A15 | [Member voting and the open cost ledger](#food-a15-member-voting-and-the-open-cost-ledger) | 9 | 20 | Restaurants and couriers keep all the profit except running costs: they vote on fees and rules, see exactly what it costs to run, and decide where it opens next. |
| FOOD-A16 | [Apps, devices and offline](#food-a16-apps-devices-and-offline) | 8 | 14 | The three apps on phones, tablets and the web, working on old devices, weak signal and little data. |
| FOOD-A17 | [Accessibility, languages and countries](#food-a17-accessibility-languages-and-countries) | 8 | 19 | Usable by everyone, in English and other languages, and ready for any country with its own rules pack; New Zealand is the first, with te reo Māori. |
| FOOD-A18 | [Privacy, data and legal duties](#food-a18-privacy-data-and-legal-duties) | 10 | 26 | Members control their data and can take it with them, and the app meets every law that applies to a food platform. |
| FOOD-A19 | [Operations and admin](#food-a19-operations-and-admin) | 8 | 16 | The tools the co-op's small team and member volunteers use to run each city safely and at cost, built on the kit's admin console, monitoring and status page. |
| FOOD-A20 | [Headless: AI, plugins and open interface](#food-a20-headless-ai-plugins-and-open-interface) | 6 | 22 | Everything a person can do, an AI agent or a script can do through the same commands, and restaurants can add sandboxed plugins. |

## FOOD-A1 Accounts, sign-up and onboarding

One Spareware login for customers, restaurants and couriers, with only the checks each role needs before its first order or delivery.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A1-F1 Customer sign-up and sign-in | Create an account in under a minute with a passkey, email or phone, or order with just a phone number. | core | Customer account profile; Food sign-in set-up; Quick account at checkout; Phone number required; Recycled and spoofed number checks; Guest order decision |
| FOOD-A1-F2 Restaurant sign-up | A restaurant owner can apply, prove the business is real and registered for food safety, and go live. | core | Restaurant application flow; Business and owner checks; Food safety registration check; Restaurant payout set-up; Restaurant approval |
| FOOD-A1-F3 Courier sign-up | A person can apply to deliver, pass the checks the law and the members set, and start the same week. | core | Courier application flow; Identity check; Driver licence and vehicle rules; Criminal record check; Courier terms step; Getting-started lessons; Visa conditions check; Food's biometric code case; Non-biometric check option |
| FOOD-A1-F4 One account, many roles | A person can be a customer, a courier and restaurant staff on one login, and switch between them. | core | Restaurant roles; Staff invites; Courier checks reused across apps |
| FOOD-A1-F5 Free membership on joining | A restaurant or courier can become a voting member as part of signing up, free. | next | Membership offer at sign-up; Food member groups |
| FOOD-A1-F6 Account recovery and closing | A person can recover a lost login, or close an account with its data exported or erased. | core | Recovery rules for payout holders; Close account |
| FOOD-A1-F7 Minimum ages for each role | Each role has a published minimum age: couriers by vehicle type, and customer account holders. | core | Minimum ages; Age at sign-up |
| FOOD-A1-F8 Teen ordering paid by a parent | A teenager orders food with a parent paying, and is never shown restricted items. | later | Teen accounts |
| FOOD-A1-F9 Home kitchens and food trucks | A home-based food business, or a food truck whose location changes, can sell through the app. | later | Moving pickup location; Home and mobile food business rules |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A1-F1-P1 | **Customer account profile.** Food's own fields on the kit profile: saved addresses, dietary preferences and access needs, the last two saved only with consent under Food's rules for sensitive details. | data | small | KIT-A1-F5-P1, KIT-A1-F1-P1, FOOD-A18-F10-P1 |  |
| FOOD-A1-F1-P2 | **Food sign-in set-up.** Uses the kit's passkey and code sign-in screens, asking only what an order needs: a name and a phone number. | interface | small | KIT-A1-F2-P4, KIT-A1-F2-P1, KIT-A1-F2-P3 |  |
| FOOD-A1-F1-P3 | **Quick account at checkout.** A light account made from a phone number at checkout, with an offer to finish it later; on a new device it shows no past orders, addresses or saved cards until the recycled number checks pass, and true guest orders wait for the guest order decision. | service | small | KIT-A1-F1-P1, KIT-A1-F1-P3, FOOD-A1-F1-P5, FOOD-A1-F1-P6 |  |
| FOOD-A1-F1-P4 | **Phone number required.** Food asks for a confirmed phone number before the first delivery order, using the kit's phone check. | integration | small | KIT-A1-F1-P3 |  |
| FOOD-A1-F1-P5 | **Recycled and spoofed number checks.** An account tied only to a phone number shows no order history, addresses or saved cards on a new device until a second check passes, such as a passkey or an email code; a number unused for a long time must be confirmed again, and texted codes go through the kit's text cost guard. | service | small | KIT-A1-F3-P1, KIT-A1-F3-P2, KIT-A1-F11-P1, KIT-A8-F4-P4 |  |
| FOOD-A1-F1-P6 | **Guest order decision.** Whether Food takes true guest orders with no account, weighing card fraud, anti-money-laundering duties and data rights; owned here until the kit opens one shared decision for every app. | research | small | KIT-A4-F5-P1, KIT-A3-F13-P1, KIT-A18-F6-P1 |  |
| FOOD-A1-F2-P1 | **Restaurant application flow.** Steps for business details, address, trading hours, bank account and contact people. | interface | medium | KIT-A1-F7-P1, KIT-A20-F1-P1 |  |
| FOOD-A1-F2-P2 | **Business and owner checks.** Food's rule that a restaurant passes the kit's business number lookup, and its owner the kit's identity check, before going live. | integration | small | KIT-A2-F7-P1, KIT-A2-F1-P2 |  |
| FOOD-A1-F2-P3 | **Food safety registration check.** Records the restaurant's food safety registration and its expiry (example: under New Zealand's Food Act), with reminders from the kit's re-check scheduler before it lapses. | service | small | FOOD-A18-F2-P1, KIT-A2-F3-P1 | [FD01](STATUS.md) |
| FOOD-A1-F2-P4 | **Restaurant payout set-up.** Sends the restaurant to the kit's payout set-up with the payment provider as part of the application. | integration | small | KIT-A3-F3-P1 |  |
| FOOD-A1-F2-P5 | **Restaurant approval.** Reviewers approve, query or decline each application, and every decision is an operation in the log. | service | small | kit-ops, FOOD-A19-F2-P1, KIT-A22-F6-P1 |  |
| FOOD-A1-F3-P1 | **Courier application flow.** Steps for identity, visa conditions, vehicle type, licence where needed, and payout account. | interface | medium | KIT-A2-F2-P2, KIT-A1-F5-P2 |  |
| FOOD-A1-F3-P2 | **Identity check.** Food's use of the kit's identity check, with the selfie match run under the biometric code (FOOD-A1-F3-P8) and the right-to-work result passed to the visa check. | integration | small | KIT-A2-F1-P2, KIT-A2-F1-P3 |  |
| FOOD-A1-F3-P3 | **Driver licence and vehicle rules.** Food's rules for the licence class, registration and warrant or certificate of fitness each vehicle type needs, checked by the kit. | integration | small | KIT-A2-F2-P1, KIT-A2-F6-P2 |  |
| FOOD-A1-F3-P4 | **Criminal record check.** Asks for a criminal record check, with the courier's consent, only where the kit's necessity test shows the law or the job's risk supports one; a member vote alone never makes it lawful. | integration | small | KIT-A2-F9-P1, KIT-A2-F9-P2 |  |
| FOOD-A1-F3-P5 | **Courier terms step.** Shows the courier terms during sign-up and records which version was accepted. | interface | small | FOOD-A18-F6-P3, KIT-A18-F4-P1 |  |
| FOOD-A1-F3-P6 | **Getting-started lessons.** Food's short lessons on keeping food safe in transit, alcohol rules, road safety and using the app, with a short check at the end, on the kit's training and certificates. | interface | small | KIT-A25-F2-P1, KIT-A25-F2-P2 |  |
| FOOD-A1-F3-P7 | **Visa conditions check.** Food's rules on the kit's visa entitlement check: whether a visa allows this kind of courier work under the status decision, any weekly hour limit such as a student visa's, and a re-check before it ends. | integration | small | KIT-A2-F11-P1, KIT-A2-F11-P2, FOOD-A18-F3-P1 | [FD02](STATUS.md) |
| FOOD-A1-F3-P8 | **Food's biometric code case.** Food's use of the kit's biometric code compliance: why selfie matching couriers is proportionate, Food's notice text, and a person reviewing every failed match. | legal | small | KIT-A2-F1-P4 |  |
| FOOD-A1-F3-P9 | **Non-biometric check option.** Food offers the kit's non-biometric route, a video or in-person check by a person, at sign-up and at every re-check. | service | small | KIT-A2-F1-P4, KIT-A2-F4-P1 |  |
| FOOD-A1-F4-P1 | **Restaurant roles.** Food's roles inside a restaurant (owner, manager, staff) on the kit's organisation model and roles per app. | library | small | KIT-A1-F7-P1, KIT-A1-F5-P2 |  |
| FOOD-A1-F4-P2 | **Staff invites.** Food's staff permission sets, such as taking orders without seeing payouts, on the kit's team management. | interface | small | KIT-A1-F7-P2, FOOD-A1-F4-P1 |  |
| FOOD-A1-F4-P3 | **Courier checks reused across apps.** Checks a person already passed in the kit, such as identity and licence, count for Food courier work where the law allows. | service | small | KIT-A2-F2-P2, KIT-A1-F5-P2 |  |
| FOOD-A1-F5-P1 | **Membership offer at sign-up.** Offers free membership during restaurant and courier sign-up, using the kit's join flow. | interface | small | KIT-A14-F1-P3 |  |
| FOOD-A1-F5-P2 | **Food member groups.** Which voting group restaurants and couriers join in the co-op-wide member list; the list itself is the kit's. | data | small | KIT-A14-F2-P1 | [K01](../kit/STATUS.md) |
| FOOD-A1-F6-P1 | **Recovery rules for payout holders.** Food asks for the kit's extra recovery checks for anyone who receives payouts, and holds payouts for a waiting period after a recovery. | service | small | KIT-A1-F4-P1, KIT-A1-F4-P2, FOOD-A10-F12-P1 |  |
| FOOD-A1-F6-P2 | **Close account.** Settles any Food balance and open orders first, then hands over to the kit's account deletion. | service | small | KIT-A18-F2-P1, FOOD-A10-F10-P2 |  |
| FOOD-A1-F7-P1 | **Minimum ages.** The minimum age for couriers on each vehicle type and for customer account holders, set on the kit's minimum age at sign-up. | legal | small | KIT-A1-F1-P5 |  |
| FOOD-A1-F7-P2 | **Age at sign-up.** Uses the date-of-birth result from the identity check to apply each role's minimum age. | library | small | KIT-A2-F1-P3, FOOD-A1-F7-P1 |  |
| FOOD-A1-F8-P1 | **Teen accounts.** A parent links and pays for a teen account through the kit's delegated access, with alcohol and other restricted items hidden and blocked. | service | small | KIT-A1-F8-P1, FOOD-A1-F7-P1, FOOD-A3-F9-P1 |  |
| FOOD-A1-F9-P1 | **Moving pickup location.** A vendor sets today's location and hours, and the delivery area check follows it. | library | small | FOOD-A2-F1-P1, KIT-A7-F5-P1 |  |
| FOOD-A1-F9-P2 | **Home and mobile food business rules.** How home-based and mobile food businesses register under each country's food safety law (example: New Zealand's Food Act), and what their page must show. | legal | small | FOOD-A18-F2-P1 | [FD01](STATUS.md) |

## FOOD-A2 Finding food

How a customer finds what to eat: nearby restaurants, search, filters and a map, ranked by published rules with no paid placement.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A2-F1 Browse nearby restaurants | See which restaurants deliver to my address now, with the time, fee and rating for each. | core | Delivery area check; Restaurant list and cards; Food ranking factors; Open and closed state |
| FOOD-A2-F2 Search | Search for a restaurant, dish or cuisine and get sensible results even with spelling mistakes. | core | Food search fields; Search screen; Macron-aware Food search |
| FOOD-A2-F3 Filters and dietary needs | Filter by cuisine, price, distance, rating, pickup, and dietary needs such as vegan, halal or gluten-free. | core | Dietary and cuisine tags; Allergen filter; Filter and sort controls |
| FOOD-A2-F4 Map view | See restaurants on a map and choose by where they are. | next | Restaurant map |
| FOOD-A2-F5 Favourites and reorder | Save favourite restaurants and dishes, and reorder a past order in two taps. | core | Favourite dishes; Reorder from history |
| FOOD-A2-F6 Restaurant page | See a restaurant's menu, hours, address, access details, rating, fees and food safety grade before ordering. | core | Restaurant page layout; Food safety grade display; Access details; Trader details |
| FOOD-A2-F7 Collections and local picks | Browse lists such as new places, local favourites and late-night food, chosen by published rules or by members. | later | Collections service |
| FOOD-A2-F8 Suggestions a person controls | Get suggestions from my own past orders, and turn personalisation off with one switch. | later | Food suggestion rules; Personalisation switch |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A2-F1-P1 | **Delivery area check.** Work out which restaurants can deliver to an address from each restaurant's area and courier supply. | service | medium | KIT-A7-F5-P1, FOOD-A6-F1-P1 |  |
| FOOD-A2-F1-P2 | **Restaurant list and cards.** The home list showing name, photo, cuisine, time estimate, the full delivery cost and rating. | interface | medium | engine-media, KIT-A20-F1-P1 |  |
| FOOD-A2-F1-P3 | **Food ranking factors.** Food's factors (distance, time and rating) on the kit's published ranking rules, with no paid placement. | library | small | KIT-A6-F1-P3 |  |
| FOOD-A2-F1-P4 | **Open and closed state.** Show opening hours, holiday closures and paused restaurants, with ordering ahead for later. | service | small | FOOD-A5-F4-P1 |  |
| FOOD-A2-F2-P1 | **Food search fields.** Restaurants, dishes and tags indexed in the kit's search service and updated as menus change. | service | small | KIT-A6-F1-P2, FOOD-A3-F1-P1 |  |
| FOOD-A2-F2-P2 | **Search screen.** Search box with suggestions, recent searches, and results grouped by restaurant and dish. | interface | medium | FOOD-A2-F2-P1 |  |
| FOOD-A2-F2-P3 | **Macron-aware Food search.** Food's search uses the kit's accent-aware matching for names in each country's languages as restaurants write them (example: te reo Māori names with macrons in New Zealand), and indexes translated dish names once menus carry them. | service | small | KIT-A19-F1-P4, FOOD-A2-F2-P1 |  |
| FOOD-A2-F3-P1 | **Dietary and cuisine tags.** One shared list of tags restaurants apply to dishes and venues. | data | small |  |  |
| FOOD-A2-F3-P2 | **Allergen filter.** Hides dishes that contain or may contain chosen allergens, using the restaurant's own declarations, never promises a dish is safe, and points to the allergy request. | service | medium | FOOD-A3-F3-P1, FOOD-A3-F3-P5, FOOD-A4-F12-P1 | [FD01](STATUS.md) |
| FOOD-A2-F3-P3 | **Filter and sort controls.** Filter chips and sort options on the list and search screens. | interface | small | FOOD-A2-F3-P1 |  |
| FOOD-A2-F4-P1 | **Restaurant map.** Restaurant pins on the kit's map display, on open map data. | interface | small | KIT-A7-F1-P2 |  |
| FOOD-A2-F5-P1 | **Favourite dishes.** Favourite dishes and restaurants for each customer, on the kit's favourites. | data | small | KIT-A5-F12-P1 |  |
| FOOD-A2-F5-P2 | **Reorder from history.** Rebuild a past cart, flagging items that changed price or are no longer sold. | service | small | FOOD-A4-F1-P1 |  |
| FOOD-A2-F6-P1 | **Restaurant page layout.** Header, details, menu sections and reviews on one page. | interface | medium | FOOD-A3-F1-P1 |  |
| FOOD-A2-F6-P2 | **Food safety grade display.** Show the council's food grade where the council publishes one. | integration | small |  | [FD01](STATUS.md) |
| FOOD-A2-F6-P3 | **Access details.** Step-free entry, where to collect and accessible parking for pickup, shown on the restaurant page. | data | small | FOOD-A2-F6-P1 |  |
| FOOD-A2-F6-P4 | **Trader details.** The restaurant's trading name, legal name and business number on its page and on every receipt, from the kit's trader details, so a customer knows who sold them the meal. | data | small | KIT-A2-F7-P2, FOOD-A11-F5-P2 |  |
| FOOD-A2-F7-P1 | **Collections service.** Rule-based and member-chosen lists, with the rules published. | service | small | FOOD-A2-F1-P3 |  |
| FOOD-A2-F8-P1 | **Food suggestion rules.** Suggestions from the customer's own past orders, on the kit's explainable suggestions, never sold, and never using allergies, faith-based diets or access needs unless the person switches that on. | library | small | KIT-A6-F10-P1, FOOD-A18-F10-P1 |  |
| FOOD-A2-F8-P2 | **Personalisation switch.** One setting to turn personalised ranking off, and a note on why each suggestion appears. | interface | small | FOOD-A2-F8-P1, KIT-A6-F10-P1 |  |

## FOOD-A3 Menus and catalogue

Restaurants build and keep their menus up to date, with options, allergens, photos and stock, at the same prices as in store.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A3-F1 Build a menu | A restaurant can create sections and dishes with prices, descriptions and photos. | core | Menu data model; Menu editor; Dish photo rules; Menu import from files; Menu and photo rights; Open menu and order formats |
| FOOD-A3-F2 Options and add-ons | A restaurant can offer sizes, extras, choices and combos, with rules such as "pick two". | core | Option groups; Combos and meal deals |
| FOOD-A3-F3 Allergens and dietary information | A restaurant can declare allergens and dietary tags for every dish, and a customer can read them before ordering. | core | Allergen fields; Allergen display and reminder; Online allergen duty; Energy and nutrition details; Cross-contact fields |
| FOOD-A3-F4 Stock and availability | A restaurant can mark a dish sold out, sell dishes only at set times, and run separate breakfast or lunch menus. | core | Sold-out switch; Time-based menus |
| FOOD-A3-F5 Same price as in store | A customer sees the restaurant's in-store prices, and the restaurant confirms it. | core | Price match promise |
| FOOD-A3-F6 Menus in many languages | A restaurant can add translations, or check machine translations before they go live. | next | Translation fields; Suggested translations |
| FOOD-A3-F7 Many locations and brands | A restaurant group can share one menu across locations with local prices, stock and hours. | next | Shared menu with local changes |
| FOOD-A3-F8 Menu sync with point of sale | A restaurant using a point-of-sale system keeps its menu and stock in step automatically. | next | Point-of-sale menu sync |
| FOOD-A3-F9 Menu checks | Menus are checked for missing allergen details, restricted items, hidden instructions to AI agents and obvious mistakes before going live. | core | Automatic menu checks; Hidden instruction checks |
| FOOD-A3-F10 Brands show their real kitchen | A customer sees which real kitchen cooks for every brand, including delivery-only brands. | next | Kitchen behind each brand |
| FOOD-A3-F11 Packaging charges and rules | A restaurant charges for bags or containers on their own line, and menus follow local single-use plastic rules (example: New Zealand's phase-outs). | later | Packaging charge line; Single-use plastic rules |
| FOOD-A3-F12 Menu from a photo or PDF | A restaurant photographs its paper menu or uploads a PDF, and checks the dishes it reads before they go live. | next | Menu reading |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A3-F1-P1 | **Menu data model.** Sections, items, prices, tax, photos and availability as Food's own fields on the kit's listing model. | data | medium | kit-core, kit-ops, KIT-A5-F1-P1, FOOD-A3-F1-P6 |  |
| FOOD-A3-F1-P2 | **Menu editor.** Food's menu editing (sections, dishes and drag to reorder) on the kit's listing editor, for browser and tablet. | interface | medium | FOOD-A3-F1-P1, KIT-A5-F1-P2 |  |
| FOOD-A3-F1-P3 | **Dish photo rules.** Food's photo sizes and crops on the kit's file uploads. | service | small | KIT-A15-F7-P1 |  |
| FOOD-A3-F1-P4 | **Menu import from files.** Imports a menu from the restaurant's own spreadsheet or point-of-sale export, in the open menu format where it can, for the restaurant to check, never from another platform's listing. | service | medium | FOOD-A3-F1-P1, FOOD-A3-F1-P5, FOOD-A3-F1-P6 | [FD05](STATUS.md) |
| FOOD-A3-F1-P5 | **Menu and photo rights.** Menus and photos come only from the restaurant's own files, never copied from other platforms' listings or photos, and each photo records who holds its rights. | legal | small | FOOD-A3-F1-P3 |  |
| FOOD-A3-F1-P6 | **Open menu and order formats.** Compares open standards for menus and orders and ports the best fit instead of inventing one: Open Delivery from Brazil (its first repository is Apache-2.0; the licence of version 2 is *unverified*), the Beckn food and beverage specification (licence *unverified*) and schema.org's Menu types. | research | small |  | [FD05](STATUS.md) |
| FOOD-A3-F2-P1 | **Option groups.** Option groups with minimum and maximum choices, prices and nesting. | library | medium | FOOD-A3-F1-P1 |  |
| FOOD-A3-F2-P2 | **Combos and meal deals.** Bundles of items at a set price. | library | small | FOOD-A3-F2-P1 |  |
| FOOD-A3-F3-P1 | **Allergen fields.** The allergens the Food Standards Code lists, with plain-English names, on every dish, stored against an allergen list for each country. | data | small | FOOD-A3-F1-P1 | [FD01](STATUS.md) |
| FOOD-A3-F3-P2 | **Allergen display and reminder.** Show allergens on each dish and remind the customer to tell the restaurant about an allergy. | interface | small | FOOD-A3-F3-P1 | [FD01](STATUS.md) |
| FOOD-A3-F3-P3 | **Online allergen duty.** What the Food Standards Code requires for food sold online, and who must provide it. | research | small | FOOD-A18-F2-P1 | [FD01](STATUS.md) |
| FOOD-A3-F3-P4 | **Energy and nutrition details.** Optional kilojoules and nutrition details per dish. | data | small | FOOD-A3-F1-P1 |  |
| FOOD-A3-F3-P5 | **Cross-contact fields.** 'May contain' warnings on each dish, and shared fryers or surfaces for each kitchen. | data | small | FOOD-A3-F3-P1 | [FD01](STATUS.md) |
| FOOD-A3-F4-P1 | **Sold-out switch.** One tap marks an item or option sold out until a chosen time. | interface | small | FOOD-A3-F1-P1 |  |
| FOOD-A3-F4-P2 | **Time-based menus.** Menus and items available only on set days and times. | library | small | FOOD-A3-F1-P1 |  |
| FOOD-A3-F5-P1 | **Price match promise.** A restaurant states its prices match in store, shown as a badge, with a way for customers to report a difference. | service | small | svc-trust |  |
| FOOD-A3-F6-P1 | **Translation fields.** Translated names and descriptions for each language. | data | small | FOOD-A3-F1-P1 |  |
| FOOD-A3-F6-P2 | **Suggested translations.** Machine translation from the kit's translation service at cost, labelled as machine-made and offered for the restaurant to check before publishing. | service | small | KIT-A19-F10-P2, FOOD-A3-F6-P1 |  |
| FOOD-A3-F7-P1 | **Shared menu with local changes.** One main menu with price, stock and hours set per location. | library | medium | FOOD-A3-F1-P1 |  |
| FOOD-A3-F8-P1 | **Point-of-sale menu sync.** Pull menus and stock from point-of-sale systems that have open, licensable interfaces. | integration | large | kit-plugin, FOOD-A20-F4-P1, FOOD-A3-F1-P6 | [FD05](STATUS.md) |
| FOOD-A3-F9-P1 | **Automatic menu checks.** Flags missing allergens, zero prices and duplicate dishes, and blocks alcohol and other age-restricted items until alcohol delivery (FOOD-A14-F2) is built. | service | small | FOOD-A3-F1-P1, KIT-A5-F1-P3 |  |
| FOOD-A3-F9-P2 | **Hidden instruction checks.** Flags menu text, dish names and reviews that address AI agents, such as telling assistants to always choose this restaurant, and marks all menu text as data for the kit's hidden instruction guards. | service | small | FOOD-A3-F9-P1, KIT-A17-F5-P1 |  |
| FOOD-A3-F10-P1 | **Kitchen behind each brand.** Every brand links to the registered kitchen that cooks it, shown on its page with that kitchen's food safety grade. | library | small | FOOD-A3-F7-P1, FOOD-A1-F2-P3, FOOD-A2-F6-P2 |  |
| FOOD-A3-F11-P1 | **Packaging charge line.** A bag or container charge shown as its own line before checkout. | library | small | FOOD-A4-F5-P1 |  |
| FOOD-A3-F11-P2 | **Single-use plastic rules.** Which takeaway packaging each country's single-use plastic rules cover, turned into menu checks, starting with New Zealand's phase-outs. | legal | small | FOOD-A3-F9-P1 |  |
| FOOD-A3-F12-P1 | **Menu reading.** Reads dishes, prices and options from a menu photo or PDF with the co-op's AI at cost, and publishes nothing until the restaurant checks it. | service | medium | engine-pdf, KIT-A17-F6-P2, FOOD-A3-F1-P4 | [FD05](STATUS.md) |

## FOOD-A4 Ordering and checkout

Everything between choosing a dish and paying: the cart, delivery or pickup, address, time, notes and a clear price breakdown.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A4-F1 Cart | Add dishes with their options, change amounts and see the running total. | core | Cart model; Cart screen |
| FOOD-A4-F2 Delivery or pickup | Choose delivery to an address or pickup from the restaurant. | core | Delivery or pickup choice; Pickup details |
| FOOD-A4-F3 Address and drop-off details | Enter an address, pin the exact door, and leave instructions such as a gate code. | core | Drop-off pin; Drop-off instructions; Saved addresses; Rural addresses |
| FOOD-A4-F4 Order ahead | Schedule an order for a set time later today or another day. | core | Food lead times; Time slot picker; Paying for orders days ahead |
| FOOD-A4-F5 Clear price breakdown | See the full total, with every fee, before checkout begins, with menu prices including sales tax and nothing added later. | core | Food price lines; Where-the-money-goes display; Price display check; Public holiday surcharge line |
| FOOD-A4-F6 Pay | Pay by card or phone wallet, and save a method for next time. | core | Food checkout; Order placed confirmation |
| FOOD-A4-F7 Group orders | Share a cart link so friends add their own dishes, then split the bill or have one person pay. | next | Shared cart; Bill splitting; Group order screens |
| FOOD-A4-F8 Change or cancel an order | Change or cancel an order before the kitchen starts, and see what a late cancel costs before confirming. | core | Food cancel rules; Cancel flow |
| FOOD-A4-F9 Notes and requests | Add a note for the kitchen and one for the courier, and say whether cutlery is wanted. | core | Kitchen and courier notes |
| FOOD-A4-F10 Order as a gift | Send food to someone else's address with a message, and let them follow it. | later | Gift order; Refuse and block gift orders |
| FOOD-A4-F11 Reusable packaging | Choose reusable containers where a local return scheme exists. | later | Reusable container scheme research |
| FOOD-A4-F12 Allergy request the kitchen must answer | A customer states an allergy on the order, and the kitchen must accept or decline it before cooking. | core | Allergy request; Kitchen allergy answer |
| FOOD-A4-F13 Access needs travel with the order | A customer's access needs, such as 'message, don't call' or 'bring it to the door', reach the courier and the restaurant on every order. | core | Access needs; Needs on the job |
| FOOD-A4-F14 Order by phone call or text | A person without a smartphone or data orders by phone call or text, and gets status updates by text. | next | Text ordering; Phone ordering line |
| FOOD-A4-F15 Order from several restaurants at once | A customer orders from two nearby restaurants in one checkout. | later | Multi-restaurant cart |
| FOOD-A4-F16 Change where it goes | A customer changes the address after ordering, or has food brought to a place with no address, such as a park or a beach. | next | Address change after ordering; Drop-off at a place |
| FOOD-A4-F17 Past orders and receipts | A customer sees every past order with its receipt, reorders, and manages saved cards. | core | Customer order history; Saved cards |
| FOOD-A4-F18 Tell the restaurant you're here | A customer taps 'I'm here' on arrival, or asks for the order to be brought out to the car. | next | I'm here and kerbside pickup |
| FOOD-A4-F19 Pay from your bank | A customer pays straight from their bank account once the payment provider supports it. | next | Bank payment for food orders |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A4-F1-P1 | **Cart model.** Items, options, notes and totals for one restaurant, kept on the device and the server. | library | medium | kit-market, kit-sync, FOOD-A3-F2-P1 |  |
| FOOD-A4-F1-P2 | **Cart screen.** Edit items, amounts and notes with the total always in view. | interface | medium | FOOD-A4-F1-P1 |  |
| FOOD-A4-F2-P1 | **Delivery or pickup choice.** Delivery, pickup, or the restaurant's own couriers, chosen per order. | library | small | kit-market |  |
| FOOD-A4-F2-P2 | **Pickup details.** Show where to collect, the opening hours and a pickup code. | interface | small | FOOD-A4-F2-P1 |  |
| FOOD-A4-F3-P1 | **Drop-off pin.** Food's exact-door pin and drop-off checks on the kit's address search and address-from-a-pin. | interface | small | KIT-A7-F2-P2, KIT-A7-F2-P4 |  |
| FOOD-A4-F3-P2 | **Drop-off instructions.** Notes, building, floor, gate code, and hand-to-me or leave-at-door. | data | small |  |  |
| FOOD-A4-F3-P3 | **Saved addresses.** Home, work and others, with the last instructions remembered. | data | small | FOOD-A1-F1-P1 |  |
| FOOD-A4-F3-P4 | **Rural addresses.** Rural delivery numbers and road names matched to the official address data, with a map pin required wherever an address will not resolve. | data | small | KIT-A7-F2-P3, FOOD-A4-F3-P1 |  |
| FOOD-A4-F4-P1 | **Food lead times.** Food's rules for when a scheduled order goes to the kitchen, on the kit's scheduled bookings. | service | small | KIT-A5-F9-P1, FOOD-A7-F1-P1 |  |
| FOOD-A4-F4-P2 | **Time slot picker.** Choose from times the restaurant and couriers can meet. | interface | small | FOOD-A4-F4-P1 |  |
| FOOD-A4-F4-P3 | **Paying for orders days ahead.** Authorises the card close to cooking time because card holds expire, tells the customer in time if that fails, and invoices catering orders. | service | small | KIT-A3-F2-P1, FOOD-A4-F4-P1, FOOD-A10-F1-P2 |  |
| FOOD-A4-F5-P1 | **Food price lines.** Food's price lines (menu total, delivery fee, platform fee and tip) on the kit's full-price-first calculator. | library | small | KIT-A5-F3-P2, FOOD-A9-F1-P1, FOOD-A9-F2-P1, FOOD-A10-F4-P2, FOOD-A11-F5-P2 |  |
| FOOD-A4-F5-P2 | **Where-the-money-goes display.** Food's version of the kit's fee breakdown: what goes to the restaurant, the courier, the running cost and tax, with sales tax shown as included. | interface | small | KIT-A3-F4-P2, FOOD-A4-F5-P1 |  |
| FOOD-A4-F5-P3 | **Price display check.** Checks the price display against the consumer regulator's guidance on drip pricing (example: New Zealand's Commerce Commission): menu prices include sales tax, the full total appears before checkout, and sales tax reads as included, not added. | legal | small | FOOD-A4-F5-P2, KIT-A18-F8-P1 |  |
| FOOD-A4-F5-P4 | **Public holiday surcharge line.** A restaurant's public holiday surcharge as its own price line, shown on the restaurant card, the menu and the total before checkout on those days, never added at the end. | library | small | FOOD-A4-F5-P1, KIT-A19-F9-P2 |  |
| FOOD-A4-F6-P1 | **Food checkout.** The kit's shared checkout with Food's order summary, so Spareware never handles card numbers. | integration | small | KIT-A3-F1-P3 |  |
| FOOD-A4-F6-P3 | **Order placed confirmation.** Confirm the order, the hold on the money and the expected time. | interface | small | FOOD-A4-F6-P1 |  |
| FOOD-A4-F7-P1 | **Shared cart.** A cart several people edit live, with each person's items marked. | service | medium | kit-sync, FOOD-A4-F1-P1 |  |
| FOOD-A4-F7-P2 | **Bill splitting.** Each person pays their own share, or the host pays for everyone, through the payment provider. | service | medium | svc-payments, FOOD-A4-F7-P1, KIT-A3-F1-P2 |  |
| FOOD-A4-F7-P3 | **Group order screens.** Invite link, who has added what, and a lock-and-send button. | interface | medium | FOOD-A4-F7-P1 |  |
| FOOD-A4-F8-P1 | **Food cancel rules.** Food's change and cancel rules, such as free before the kitchen starts, as a policy on the kit's cancellation policies. | library | small | KIT-A5-F6-P1 |  |
| FOOD-A4-F8-P2 | **Cancel flow.** A cancel button that shows the refund before confirming. | interface | small | FOOD-A4-F8-P1 |  |
| FOOD-A4-F9-P1 | **Kitchen and courier notes.** Separate notes for each, with a cutlery and napkins switch that is off by default to cut waste. | data | small |  |  |
| FOOD-A4-F10-P1 | **Gift order.** Recipient name, phone and message, with tracking shared with the recipient only once they agree. | service | small | FOOD-A8-F1-P1 |  |
| FOOD-A4-F10-P2 | **Refuse and block gift orders.** A recipient can refuse a gift order and block its sender, and senders face limits on sending food to addresses that are not their own. | service | small | FOOD-A4-F10-P1, KIT-A10-F4-P1 |  |
| FOOD-A4-F11-P1 | **Reusable container scheme research.** How existing reusable container schemes in each country work and whether they can link in, starting with New Zealand's. | research | small |  |  |
| FOOD-A4-F12-P1 | **Allergy request.** The allergens the customer must avoid, chosen from the same list as the dish fields, attached to the order. | data | small | FOOD-A3-F3-P1, FOOD-A4-F1-P1, FOOD-A18-F10-P1 |  |
| FOOD-A4-F12-P2 | **Kitchen allergy answer.** The order screen holds an order with an allergy request until the kitchen accepts or declines it; a decline cancels and refunds in full. | interface | small | FOOD-A5-F1-P1, FOOD-A5-F1-P5 |  |
| FOOD-A4-F13-P1 | **Access needs.** Saved needs such as 'message, don't call', 'bring it to the door', a wheelchair user or a guide dog, kept with consent on the kit profile, which should hold access needs for every app, and attached to every order. | data | small | KIT-A1-F5-P1, FOOD-A1-F1-P1, FOOD-A18-F10-P1 |  |
| FOOD-A4-F13-P2 | **Needs on the job.** Shows each party only the instruction it needs, such as 'bring it to the door' for the courier and never the reason, on the courier's offer and drop-off screens and the restaurant's pickup screen. | interface | small | FOOD-A4-F13-P1, FOOD-A6-F2-P1, FOOD-A6-F4-P1, FOOD-A18-F10-P2 |  |
| FOOD-A4-F14-P1 | **Text ordering.** Food's text ordering from favourites or a past order, with status updates by text; an order on a saved card needs a PIN or an approval in the app, and texts go through the kit's text cost guard. The text channel itself should move down into the kit, since Spare Ride maps the same. | service | small | KIT-A8-F4-P2, KIT-A8-F4-P4, FOOD-A1-F1-P5, FOOD-A2-F5-P2, FOOD-A20-F1-P1 |  |
| FOOD-A4-F14-P2 | **Phone ordering line.** Food's phone ordering: a person takes the order into the same order path through the kit's phone support channels, including the national relay service (example: NZ Relay in New Zealand). The phone line itself should move down into the kit, since Spare Ride maps the same. | service | small | KIT-A21-F4-P1, FOOD-A20-F1-P1 |  |
| FOOD-A4-F15-P1 | **Multi-restaurant cart.** One checkout split into one order per restaurant, sharing one courier only when both lie on one route. | library | medium | FOOD-A4-F1-P1, KIT-A5-F11-P1, FOOD-A7-F3-P1 |  |
| FOOD-A4-F16-P1 | **Address change after ordering.** Changes the drop-off before pickup within a distance limit, showing any fee change first, and after pickup only with the courier's agreement. | service | small | FOOD-A4-F8-P1, FOOD-A9-F1-P1, FOOD-A4-F3-P1, KIT-A5-F15-P1 |  |
| FOOD-A4-F16-P2 | **Drop-off at a place.** A map pin and a short meeting description for places with no address. | library | small | FOOD-A4-F3-P1 |  |
| FOOD-A4-F17-P1 | **Customer order history.** Food's past orders on the kit's orders page, with each receipt, the price lines and a reorder button. | interface | small | KIT-A5-F8-P1, KIT-A3-F7-P1, FOOD-A2-F5-P2 |  |
| FOOD-A4-F17-P2 | **Saved cards.** The kit's saved payment methods in Food's checkout, holding only the provider's token and removable at any time. | integration | small | KIT-A3-F8-P1, FOOD-A4-F6-P1 |  |
| FOOD-A4-F18-P1 | **I'm here and kerbside pickup.** An 'I'm here' button for pickup orders, and kerbside pickup where the restaurant offers it, with a note on where the car is, so disabled customers and parents with small children need not go inside. | interface | small | FOOD-A4-F2-P2, FOOD-A5-F1-P1 |  |
| FOOD-A4-F19-P1 | **Bank payment for food orders.** Turns on the kit's bank payments for Food once the payment provider supports them (moved here from FOOD-A4-F6-P2). | integration | small | KIT-A3-F11-P1, FOOD-A4-F6-P1 |  |

## FOOD-A5 Restaurant order handling

The restaurant's live screen: an order arrives with a sound, the kitchen accepts it and sets a time, marks it ready and hands it over.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A5-F1 Receive orders | A restaurant hears and sees each new order on a tablet, phone or computer, and accepts or declines it. | core | Order screen; Loud alerts; Food order steps; Auto-accept; Rules for each unhappy step; Pickup not collected; Food that cannot be delivered |
| FOOD-A5-F2 Set and change prep times | A restaurant sets how long an order will take, and adds time when the kitchen is slammed. | core | Prep time buttons; Prep time learning |
| FOOD-A5-F3 Mark ready and hand over | A restaurant marks an order ready and confirms the right courier or customer took it. | core | Ready and collected buttons; Hand-over code rules; Courier arriving |
| FOOD-A5-F4 Pause and busy mode | A restaurant can pause new orders, close early, or add time to every order when busy. | core | Pause, hours and holidays; Busy mode |
| FOOD-A5-F5 Item runs out mid-order | A restaurant tells the customer an item has run out and offers a swap or a refund. | core | Swap or refund request |
| FOOD-A5-F6 Kitchen printing | A restaurant prints order tickets and bag labels on the kitchen printer it already has. | next | Receipt printer support; Bag labels |
| FOOD-A5-F7 Orders straight into the till | Orders go straight into the restaurant's point-of-sale system so staff never retype them. | next | Point-of-sale order connectors |
| FOOD-A5-F8 Order history and lookup | A restaurant can find any past order and see everything that happened to it. | core | Restaurant order history |
| FOOD-A5-F9 Restaurant's own couriers | A restaurant delivers with its own staff and still uses the app for tracking and payment. | next | Own-courier mode |
| FOOD-A5-F10 Many locations | A restaurant group watches orders and pauses across all its locations from one screen. | later | Multi-location view |
| FOOD-A5-F12 Fallback when a restaurant goes silent | When a restaurant's device stops answering, the restaurant is paused and phoned or texted with the order, and the customer is refunded if nobody answers. | core | Silence detection; Call or text the restaurant; Cancel and refund on no answer |
| FOOD-A5-F13 Order limits | A restaurant caps how many orders it takes in each 15 minutes, and sets a minimum order. | next | Order cap; Restaurant minimum order |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A5-F1-P1 | **Order screen.** A tablet-first app listing new, in-progress and ready orders. | interface | large | kit-ops, KIT-A20-F1-P1 |  |
| FOOD-A5-F1-P2 | **Loud alerts.** A repeating sound and screen flash until an order is accepted, even with the screen locked. | interface | small | KIT-A8-F3-P1 |  |
| FOOD-A5-F1-P3 | **Food order steps.** Food's steps on the kit's shared order steps and multi-party orders: received, accepted, preparing, ready, collected and delivered, plus declined, not accepted in time, cancelled by the customer, restaurant, courier or operator, undeliverable, returned, not collected for pickup orders, and part-refunded. | library | medium | KIT-A5-F4-P1, KIT-A5-F11-P1 |  |
| FOOD-A5-F1-P4 | **Auto-accept.** Accept orders automatically when the restaurant chooses to. | service | small | FOOD-A5-F1-P3 |  |
| FOOD-A5-F1-P5 | **Rules for each unhappy step.** For each step that goes wrong, who is told, what is refunded, who bears the cost and what the courier is paid for work already done; an order not accepted in time is cancelled and refunded at once. | library | small | FOOD-A5-F1-P3, FOOD-A10-F6-P1, FOOD-A11-F2-P3 |  |
| FOOD-A5-F1-P6 | **Pickup not collected.** A paid pickup order nobody collects is held for a published time after it is ready, then marked not collected; the customer is told, the restaurant is paid, and the restaurant decides what happens to the food under its own food safety plan. | library | small | FOOD-A5-F1-P3, FOOD-A5-F1-P5 |  |
| FOOD-A5-F1-P7 | **Food that cannot be delivered.** A safe rule for food from cancelled, undeliverable or returned orders: never resold, returned to the restaurant or thrown away as the food rules study says, alcohol always returned to the seller, and the courier never charged for it. | library | small | FOOD-A5-F1-P5, FOOD-A18-F2-P1 | [FD01](STATUS.md) |
| FOOD-A5-F2-P1 | **Prep time buttons.** Quick buttons to set a time or add minutes to an order. | interface | small | FOOD-A5-F1-P1 |  |
| FOOD-A5-F2-P2 | **Prep time learning.** Learn each restaurant's real prep times by dish, time of day and queue length. | library | medium | FOOD-A5-F1-P3 | [FD03](STATUS.md) |
| FOOD-A5-F3-P1 | **Ready and collected buttons.** Mark ready, then confirm the hand-over. | interface | small | FOOD-A5-F1-P3 |  |
| FOOD-A5-F3-P2 | **Hand-over code rules.** When Food asks for a hand-over code at the counter, on the kit's proof of handover, with a limit on wrong guesses. | service | small | KIT-A5-F7-P2, FOOD-A5-F1-P3 |  |
| FOOD-A5-F3-P3 | **Courier arriving.** The order screen shows the assigned courier's first name, photo and vehicle from assignment to hand-over, a live arrival time, and 'courier waiting' once they reach the counter, so the kitchen times the food and staff hand the bag to the right person. | interface | small | FOOD-A5-F1-P1, FOOD-A7-F1-P2, KIT-A7-F3-P3 |  |
| FOOD-A5-F4-P1 | **Pause, hours and holidays.** Pause for a set time, set regular hours and holiday closures. | service | small | kit-ops, KIT-A5-F5-P1 |  |
| FOOD-A5-F4-P2 | **Busy mode.** Add minutes to every new order and shrink the delivery area while busy. | service | small | FOOD-A5-F4-P1 |  |
| FOOD-A5-F5-P1 | **Swap or refund request.** The kitchen picks the missing item, and the customer approves a swap or a part refund in the app, on the kit's order changes. | service | medium | FOOD-A8-F3-P1, FOOD-A10-F6-P1, KIT-A5-F15-P1 |  |
| FOOD-A5-F6-P1 | **Receipt printer support.** Print to common network and Bluetooth receipt printers using their published command sets. | integration | medium | FOOD-A5-F1-P1 | [FD05](STATUS.md) |
| FOOD-A5-F6-P2 | **Bag labels.** A label with the order number and name for each bag. | interface | small | FOOD-A5-F6-P1 |  |
| FOOD-A5-F7-P1 | **Point-of-sale order connectors.** Plugins that send orders into point-of-sale systems with open, licensable interfaces. | integration | large | kit-plugin, FOOD-A20-F4-P1, FOOD-A3-F1-P6 | [FD05](STATUS.md) |
| FOOD-A5-F8-P1 | **Restaurant order history.** Food's search fields and event detail on the kit's orders page. | interface | small | KIT-A5-F8-P1 |  |
| FOOD-A5-F9-P1 | **Own-courier mode.** Assign the order to the restaurant's own courier accounts, with tracking and no courier fee taken. | service | medium | FOOD-A6-F1-P1, FOOD-A1-F4-P1 |  |
| FOOD-A5-F10-P1 | **Multi-location view.** A live view of every location's orders, pauses and prep times. | interface | medium | FOOD-A5-F1-P1 |  |
| FOOD-A5-F12-P1 | **Silence detection.** Spots a restaurant device that has stopped checking in during open hours, and pauses new orders. | service | small | FOOD-A5-F4-P1, KIT-A22-F1-P1 |  |
| FOOD-A5-F12-P2 | **Call or text the restaurant.** An automatic call or text to the restaurant's phone with the waiting order, which staff can accept by reply. | integration | small | KIT-A8-F4-P2, FOOD-A5-F1-P3 |  |
| FOOD-A5-F12-P3 | **Cancel and refund on no answer.** Cancels and refunds in full if there is still no answer after a set time, and tells the customer straight away. | service | small | FOOD-A5-F1-P5, FOOD-A5-F12-P2 |  |
| FOOD-A5-F13-P1 | **Order cap.** The most orders a restaurant takes in each 15 minutes; once it is reached, new orders get a later time or wait for the next slot. | library | small | FOOD-A5-F4-P2, FOOD-A7-F2-P1 |  |
| FOOD-A5-F13-P2 | **Restaurant minimum order.** A minimum order value set by the restaurant, shown on its card and page before anything goes in the cart. | library | small | FOOD-A4-F1-P1, FOOD-A2-F1-P2 |  |

## FOOD-A6 Courier work

The courier's app: go online, see the full pay and distance before accepting, collect, deliver and get paid daily.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A6-F1 Go online and offline | A courier starts and stops work whenever they choose, with no penalty. | core | Food availability rules; Courier home screen; Visa hour limits; Hours across Spareware apps |
| FOOD-A6-F2 See the whole offer first | A courier sees the pay, pickup, drop-off area and distance before accepting, and can decline with no penalty. | core | Job offer card; Offer timeout; No-penalty rule |
| FOOD-A6-F3 Navigate | A courier gets directions suited to a bike, e-bike, scooter, car or walking. | core | Vehicle routing profiles; Directions hand-off |
| FOOD-A6-F4 Pickup and drop-off | A courier confirms pickup with the order code, and drop-off by hand-over, photo or code. | core | Pickup check; Food proof of delivery; Customer not there; Faked location checks; Pickup notes for couriers |
| FOOD-A6-F5 Two orders at once | A courier can carry two orders going the same way, with the extra pay shown before accepting. | next | Batched job display |
| FOOD-A6-F6 Earnings and payouts | A courier sees every job's pay, tips and fees, and is paid daily. | core | Food earnings view; Daily payout; Tax statements; Mileage and expense log; Pay for wasted trips |
| FOOD-A6-F7 Paid for waiting | A courier is paid for waiting at a restaurant beyond the ready time. | next | Waiting time pay |
| FOOD-A6-F8 Busy times and booked shifts | A courier sees when and where it gets busy, and can book a block of time if members choose to offer that. | later | Food demand view; Optional shift booking |
| FOOD-A6-F9 Vehicle and gear | A courier sets their vehicle, and is only offered jobs that suit it, such as no large catering orders on a bike. | core | Vehicle profile; Jobs a courier won't carry |
| FOOD-A6-F10 Keep working without signal | A courier can carry on a job through a dead zone, and the app catches up when signal returns. | core | Offline job steps |
| FOOD-A6-F11 Both apps, one courier | A courier can take Spare Food jobs and other Spareware jobs, such as rides, from one account, if they choose. | later | Shared availability |
| FOOD-A6-F12 Safe to use while moving | A courier never needs to type or tap while moving: offers are read aloud and the screen suits a phone cradle. | core | Driving-safe mode; Safe offer timing; Phone rules on the road |
| FOOD-A6-F13 Pay the law requires for the courier's status | A courier is paid at least what the law requires for their status, with the records, breaks and leave an employee is owed if couriers are employees. | core | Minimum pay per engaged hour; Shifts and hourly pay for employees; Wage and time records; Rest and meal breaks; Holidays and leave; Payroll, income tax and retirement savings (example: New Zealand's PAYE and KiwiSaver); Union access and bargaining |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A6-F1-P1 | **Food availability rules.** Food's courier states (online, offline and on a job) on the kit's worker availability. | library | small | KIT-A6-F2-P1 |  |
| FOOD-A6-F1-P2 | **Courier home screen.** Go-online button, today's earnings and how busy it is nearby. | interface | medium | FOOD-A6-F1-P1 |  |
| FOOD-A6-F1-P3 | **Visa hour limits.** Stops offers once a courier reaches the weekly hours their visa allows, counted across every Spareware app, wherever the status decision makes that limit apply. | library | small | FOOD-A1-F3-P7, FOOD-A6-F1-P4, KIT-A24-F3-P1 |  |
| FOOD-A6-F1-P4 | **Hours across Spareware apps.** Counts a courier's online and engaged hours across every Spareware app, not only Food, for visa limits, fatigue limits and minimum pay; it reads the kit's worker availability, which should keep this as one shared worker record, since Spare Ride relies on it too. | library | small | KIT-A6-F2-P1, KIT-A24-F3-P1 |  |
| FOOD-A6-F2-P1 | **Job offer card.** Food's offer details on the kit's job offer screen: pay, any checkout tip (which can then never be cut), pickup, a rough drop-off area from the kit's approximate location, distance, time and the restaurant's pickup notes. | interface | small | KIT-A6-F4-P1, KIT-A7-F10-P1, FOOD-A9-F3-P1 |  |
| FOOD-A6-F2-P2 | **Offer timeout.** Food's offer time limit on the kit's dispatch engine, long enough to read safely before the offer passes on. | service | small | KIT-A6-F3-P2 |  |
| FOOD-A6-F2-P3 | **No-penalty rule.** Acceptance rates are never used to punish, written into Food's rules on the kit's fair offer rules. | legal | small | KIT-A6-F4-P2 | [FD02](STATUS.md) |
| FOOD-A6-F3-P1 | **Vehicle routing profiles.** Food's bike, e-bike, scooter, car and walking profiles on the kit's routing service. | integration | small | KIT-A7-F3-P2 |  |
| FOOD-A6-F3-P2 | **Directions hand-off.** Hands the route to the courier's own map app at first, and to the kit's turn-by-turn guidance once that is built. | interface | small | KIT-A7-F6-P1, FOOD-A6-F3-P1 |  |
| FOOD-A6-F4-P1 | **Pickup check.** Check the code and the number of bags, and log any wait. | interface | small | FOOD-A5-F3-P2 |  |
| FOOD-A6-F4-P2 | **Food proof of delivery.** Food's rules on the kit's proof of handover: a leave-at-door photo that frames only the door, blurs any people and is kept a short time only, or a code for valuable and age-restricted orders. | service | small | KIT-A5-F7-P2 |  |
| FOOD-A6-F4-P3 | **Customer not there.** Call, wait on a timer, then leave or return as the published rules say. | service | small | FOOD-A8-F3-P2 |  |
| FOOD-A6-F4-P4 | **Faked location checks.** Food's waiting pay, distance pay and proof of delivery use the kit's faked location checks, so a spoofed position is flagged for a person before pay or a delivery claim relies on it. | library | small | KIT-A7-F4-P3, FOOD-A6-F4-P2 |  |
| FOOD-A6-F4-P5 | **Pickup notes for couriers.** A restaurant's notes for couriers on where to park, which door to use and whether couriers may use the toilet, shown on the offer and the pickup screen. | data | small | FOOD-A6-F2-P1, FOOD-A2-F6-P1 |  |
| FOOD-A6-F5-P1 | **Batched job display.** Both drop-offs, the order to deliver them in, and the combined pay. | interface | small | FOOD-A7-F3-P1 |  |
| FOOD-A6-F6-P1 | **Food earnings view.** Food's earnings lines (delivery fees, waiting pay and tips) on the kit's earnings page. | interface | small | KIT-A3-F3-P3, KIT-A23-F2-P4 |  |
| FOOD-A6-F6-P2 | **Daily payout.** Food couriers are paid daily on the kit's payout runs, at no extra charge where the provider allows. | integration | small | KIT-A3-F3-P2 |  |
| FOOD-A6-F6-P3 | **Tax statements.** Food's figures (delivery fees, tips and sales tax) on the kit's yearly statements. | service | small | KIT-A4-F4-P1 |  |
| FOOD-A6-F6-P4 | **Mileage and expense log.** Food's work kilometres from finished jobs, without keeping the route, and expenses with a receipt photo, for a year-end tax pack. The log itself should move down into the kit, since Spare Ride maps the same. | service | small | FOOD-A6-F1-P1, KIT-A4-F4-P1, KIT-A15-F7-P1 |  |
| FOOD-A6-F6-P5 | **Pay for wasted trips.** A courier who accepted a job that is then cancelled, undeliverable or returned is paid for the distance and time already worked, at the voted rate, with the cost falling where the unhappy-step rules say. | library | small | FOOD-A9-F1-P1, FOOD-A5-F1-P5, FOOD-A6-F6-P1 | [FD02](STATUS.md) |
| FOOD-A6-F7-P1 | **Waiting time pay.** Measure time from arrival to hand-over and pay it at the voted rate. | service | small | FOOD-A6-F4-P1, FOOD-A9-F1-P1, KIT-A12-F4-P3, FOOD-A6-F4-P4 | [FD02](STATUS.md) |
| FOOD-A6-F8-P1 | **Food demand view.** Food's busy times and areas on the kit's demand maps. | interface | small | KIT-A6-F9-P1, FOOD-A7-F6-P1 |  |
| FOOD-A6-F8-P2 | **Optional shift booking.** Book blocks of time with a guaranteed hourly minimum, if members vote for it. | service | medium | svc-voting | [FD02](STATUS.md) |
| FOOD-A6-F9-P1 | **Vehicle profile.** Vehicle type, carrying space and insulated bag. | data | small | FOOD-A1-F3-P3 |  |
| FOOD-A6-F9-P2 | **Jobs a courier won't carry.** A courier can opt out of alcohol jobs, and of other restricted goods once they exist, for any reason including religion, and is then never offered them or penalised for it. | library | small | FOOD-A6-F9-P1, KIT-A6-F4-P2 |  |
| FOOD-A6-F10-P1 | **Offline job steps.** Food's courier job steps queued on the phone and replayed in order by the kit's sync engine. | library | small | KIT-A15-F4-P2 |  |
| FOOD-A6-F11-P1 | **Shared availability.** Food's offers join the kit's one worker availability, which serves every Spareware app; Food holds only its own clash rules, such as no Food offer while a courier is on a ride. | library | small | KIT-A6-F2-P1, FOOD-A6-F1-P4, FOOD-A1-F4-P3 |  |
| FOOD-A6-F12-P1 | **Driving-safe mode.** Food's courier screens in a driving-safe mode: no typing or taps while moving, large targets, and offers read aloud, shown large and signalled by vibration. The mode itself should move down into the kit, since Spare Ride maps the same. | interface | small | FOOD-A6-F2-P1, KIT-A19-F2-P1 |  |
| FOOD-A6-F12-P2 | **Safe offer timing.** Offer time limits long enough to pull over, and one large button to accept, so nobody taps fast while moving. | library | small | FOOD-A6-F2-P2 |  |
| FOOD-A6-F12-P3 | **Phone rules on the road.** What each country's road rules allow for phones in cradles on bikes, e-bikes, scooters and in cars, starting with New Zealand's. This research should be shared through the kit, since Spare Ride needs the car case. | legal | small |  |  |
| FOOD-A6-F13-P1 | **Minimum pay per engaged hour.** Food's engaged time, from accepting an offer to drop-off including waits, and its earnings fed to the kit's minimum pay top-up, wherever the status decision or a member vote applies it. | library | small | KIT-A23-F2-P1, FOOD-A6-F1-P4, FOOD-A18-F3-P1 | [FD02](STATUS.md) |
| FOOD-A6-F13-P2 | **Shifts and hourly pay for employees.** If couriers are employees: rostered shifts, hourly pay for all time on shift, and jobs offered within a shift by the same published rules. | service | medium | KIT-A23-F1-P2, KIT-A6-F2-P1, FOOD-A7-F5-P1 | [FD02](STATUS.md) |
| FOOD-A6-F13-P3 | **Wage and time records.** The wage, time, holiday and leave records employment law requires for employee couriers, kept from Food's shift and job records for as long as the law says. | data | small | KIT-A23-F2-P2, KIT-A4-F6-P1, FOOD-A6-F13-P2 |  |
| FOOD-A6-F13-P4 | **Rest and meal breaks.** Rest and meal breaks for employee couriers as employment law requires (example: New Zealand's Employment Relations Act 2000), with offers paused during each break. | library | small | KIT-A24-F3-P1, FOOD-A6-F13-P2 |  |
| FOOD-A6-F13-P5 | **Holidays and leave.** Holiday pay, public holiday pay and sick leave for employee couriers, on the kit's holiday pay. | library | small | KIT-A23-F2-P2, FOOD-A6-F13-P3 |  |
| FOOD-A6-F13-P6 | **Payroll, income tax and retirement savings (example: New Zealand's PAYE and KiwiSaver).** Employee couriers paid through the kit's payroll provider link, which handles income tax, retirement savings and payday filing (example: PAYE and KiwiSaver in New Zealand), with the kit's guidance for each status on the earnings page. | integration | small | KIT-A23-F2-P3, KIT-A23-F2-P4, FOOD-A6-F13-P2 |  |
| FOOD-A6-F13-P7 | **Union access and bargaining.** What the co-op owes as an employer if couriers are employees: union access, good-faith bargaining, and how a collective agreement sits with member votes on pay (example: New Zealand's Employment Relations Act 2000). The kit should hold this rule for every app. | legal | medium | KIT-A23-F1-P1, FOOD-A18-F3-P1 | [FD02](STATUS.md) |

## FOOD-A7 Dispatch and timing

The engine that sends the right courier so they arrive as the food is ready, not before, and keeps every time estimate honest.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A7-F1 Courier arrives as the food is ready | A courier is offered a job at the ready time minus their travel time, so food and couriers wait as little as possible. | core | Ready-time estimate; Food travel times; Food dispatch rules |
| FOOD-A7-F2 Honest delivery times | A customer sees a delivery time that holds up, and that updates as the order moves. | core | Delivery time model; Accuracy tracking; Late order remedy |
| FOOD-A7-F3 Batching | Two orders going the same way share one courier when neither ends up late. | next | Food batching limits |
| FOOD-A7-F4 Reassignment | If a courier drops a job or is held up, it passes quickly to another courier. | core | Stalled job detection and re-offer |
| FOOD-A7-F5 Fair job sharing | Jobs are shared fairly between online couriers by rules members can read and vote on. | core | Published dispatch rules; Fairness measures; Customer ratings and fair areas |
| FOOD-A7-F6 Busy-time forecasts | The co-op can see busy times coming so enough couriers are online. | later | Demand forecast |
| FOOD-A7-F7 Zones and delivery distance | Operators set where the service runs and how far each restaurant delivers. | core | Food zones; Delivery distance rules |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A7-F1-P1 | **Ready-time estimate.** Combine the restaurant's set time with its learned prep times. | library | medium | FOOD-A5-F2-P2 | [FD03](STATUS.md) |
| FOOD-A7-F1-P2 | **Food travel times.** Courier-to-restaurant travel time for each vehicle type, from the kit's routing service and arrival time estimates. | integration | small | KIT-A7-F3-P2, KIT-A7-F3-P3 |  |
| FOOD-A7-F1-P3 | **Food dispatch rules.** Food's scoring rules for the shared matching engine: distance, ready time, vehicle fit and fair sharing. | library | large | KIT-A6-F3-P3, FOOD-A7-F1-P1, FOOD-A7-F1-P2 | [FD03](STATUS.md) |
| FOOD-A7-F2-P1 | **Delivery time model.** Predict total time from prep, courier arrival and the ride to the door. | library | medium | FOOD-A7-F1-P1, FOOD-A7-F1-P2 | [FD03](STATUS.md) |
| FOOD-A7-F2-P2 | **Accuracy tracking.** Measures each estimate against the real time and publishes how accurate it was as open data. | service | small | FOOD-A7-F2-P1, kit-ops |  |
| FOOD-A7-F2-P3 | **Late order remedy.** A published remedy when an order arrives much later than the time shown at checkout, such as a part refund within limits members vote on, paid by whoever caused the delay under the who-pays rules, or from the reserve when nobody did, and never taken from the courier's fee. | library | small | FOOD-A7-F2-P2, FOOD-A11-F2-P3, FOOD-A10-F6-P1, FOOD-A15-F8-P1, svc-voting |  |
| FOOD-A7-F3-P1 | **Food batching limits.** Food's lateness limit, set by members, on the kit's batching. | library | small | KIT-A6-F7-P1, FOOD-A7-F1-P3 | [FD03](STATUS.md) |
| FOOD-A7-F4-P1 | **Stalled job detection and re-offer.** Spot a stalled job and offer it again with the food's ready state. | service | medium | KIT-A6-F3-P2, FOOD-A7-F1-P3 |  |
| FOOD-A7-F5-P1 | **Published dispatch rules.** A plain-English description of how jobs are given out, kept in step with the code. | legal | small | svc-voting, FOOD-A7-F1-P3, KIT-A10-F5-P1 | [FD03](STATUS.md) |
| FOOD-A7-F5-P2 | **Fairness measures.** Food's measures, such as earnings per online hour across couriers, on the kit's fairness checks. | service | small | KIT-A6-F5-P2 |  |
| FOOD-A7-F5-P3 | **Customer ratings and fair areas.** Decides whether couriers see a customer's rating before accepting, and tests that dispatch serves every area and group fairly. | research | small | FOOD-A7-F5-P2, FOOD-A12-F1-P2 |  |
| FOOD-A7-F6-P1 | **Demand forecast.** Forecast orders by area and hour from past orders, events and weather. | library | medium | kit-store |  |
| FOOD-A7-F7-P1 | **Food zones.** Food's service areas drawn and edited on the kit's service areas. | interface | small | KIT-A7-F5-P1 |  |
| FOOD-A7-F7-P2 | **Delivery distance rules.** Distance limits per restaurant and per zone, tightened automatically when couriers are short. | library | small | FOOD-A7-F7-P1 |  |

## FOOD-A8 Live tracking and messages

Everyone can see where the order is up to and talk to each other without sharing phone numbers.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A8-F1 Live order status | A customer sees each step from accepted to delivered, on screen and in notifications. | core | Status timeline; Status notifications; Lock-screen live status |
| FOOD-A8-F2 Courier on the map | A customer sees the courier moving on a map once the food is collected, and not before. | core | Courier location during delivery; Tracking map |
| FOOD-A8-F3 Messages and calls | Customer, courier and restaurant can message or call each other through masked numbers. | core | Order chat; Masked calling; Quick replies |
| FOOD-A8-F4 Share tracking | A customer can share a tracking link with someone else. | later | Tracking link |
| FOOD-A8-F5 Lateness alerts | A customer is told early if an order will be late, and why. | next | Delay detection |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A8-F1-P1 | **Status timeline.** Food's order steps and times on the kit's order timeline. | interface | small | KIT-A5-F4-P2, FOOD-A5-F1-P3 |  |
| FOOD-A8-F1-P2 | **Status notifications.** Food's status messages sent by push, text or email through the kit's notification router. | integration | small | KIT-A8-F3-P2 |  |
| FOOD-A8-F1-P3 | **Lock-screen live status.** Live status on the phone's lock screen, where the phone allows it. | integration | small | KIT-A8-F3-P1, FOOD-A16-F1-P3 | [K04](../kit/STATUS.md) |
| FOOD-A8-F2-P1 | **Courier location during delivery.** Shares the courier's position on the kit's live location stream only from collection to drop-off, not kept afterwards, with a trace only where the law, safety or a dispute needs it. | service | small | KIT-A7-F4-P1, KIT-A7-F4-P2 |  |
| FOOD-A8-F2-P2 | **Tracking map.** A map showing the courier, the route and the drop-off. | interface | medium | KIT-A7-F1-P2, FOOD-A8-F2-P1 |  |
| FOOD-A8-F3-P1 | **Order chat.** A Food chat for each order on the kit's chat service, closed after delivery and kept for disputes. | service | small | KIT-A8-F1-P2 |  |
| FOOD-A8-F3-P2 | **Masked calling.** Food turns on the kit's masked calls for each order until it is delivered. | integration | small | KIT-A8-F2-P1 |  |
| FOOD-A8-F3-P3 | **Quick replies.** Food's ready-made messages, such as 'on my way' and 'at the door', on the kit's chat screens, translated for the reader. | service | small | KIT-A8-F1-P3, FOOD-A8-F3-P1, FOOD-A17-F2-P3 |  |
| FOOD-A8-F4-P1 | **Tracking link.** Food's shared tracking link on the kit's trusted contact link, showing the status only, not the courier's details. | service | small | KIT-A10-F2-P1 |  |
| FOOD-A8-F5-P1 | **Delay detection.** Spot when the estimate slips past a limit and send the reason. | service | small | FOOD-A7-F2-P1 |  |

## FOOD-A9 Pricing, fees and deals

How the price is set: the menu price, a delivery fee the couriers vote on, the at-cost platform fee, and optional tips and deals.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A9-F1 Delivery fee by distance and time | A customer pays a delivery fee set by distance and time, and all of it goes to the courier. | core | Delivery fee formula; Fee history |
| FOOD-A9-F2 At-cost platform fee | A customer and a restaurant see the platform fee, last period's running cost divided by orders, and who pays it, on every order. | core | Food fee rate; Pickup fee rule; What the fee may cover; Who pays the fee; Food's card surcharge cases; First-period fee |
| FOOD-A9-F3 Tips | A customer can tip the courier before or after delivery, the courier gets all of it, and a tip the courier saw before accepting is never cut. | core | Tip flow |
| FOOD-A9-F4 Small-order and busy-time rules | Members can choose a small-order fee or a busy-time courier bonus, always shown before the order. | next | Small-order fee; Busy-time bonus |
| FOOD-A9-F5 Restaurant deals | A restaurant can run its own discounts, such as free delivery or a dish deal, paid for by the restaurant. | next | Food deal kinds; Deal set-up screen |
| FOOD-A9-F6 Promo codes and referrals | A customer can use a promo code, and invite friends for credit. | later | Promo codes; Referral credit |
| FOOD-A9-F7 Gift cards and credit | A customer can buy a gift card, and can choose credit instead of money back when refunded. | later | Food gift cards |
| FOOD-A9-F8 Member benefits instead of subscriptions | A customer could get lower fees as a free member instead of through a paid subscription, if members vote for it. | later | Customer member benefits question |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A9-F1-P1 | **Delivery fee formula.** A formula by distance, time and waiting, with a published minimum, set by courier vote only in a form the competition law check clears. | library | medium | svc-voting, KIT-A12-F4-P3, FOOD-A15-F2-P3, KIT-A23-F1-P2 | [FD02](STATUS.md) |
| FOOD-A9-F1-P2 | **Fee history.** Every past formula and when it changed, published. | data | small | svc-ledger, FOOD-A9-F1-P1 |  |
| FOOD-A9-F2-P1 | **Food fee rate.** Food's fee per order from the kit's fee calculator and fee setting, with pickup and delivery rates apart, the first period's fee from the start-up rule, and sales tax on the fee where it applies. | library | small | KIT-A3-F4-P1, KIT-A13-F3-P1, FOOD-A9-F2-P6, KIT-A4-F9-P3 |  |
| FOOD-A9-F2-P2 | **Pickup fee rule.** Pickup orders pay only the payment provider's charge. | library | small | FOOD-A9-F2-P1 |  |
| FOOD-A9-F2-P3 | **What the fee may cover.** A decision, put to members, on which costs the at-cost fee may cover: support workers, operators, panel pay, translators, legal advice, insurance, marketing, a reserve and paying for the build itself. | research | medium | KIT-A13-F1-P1 | [K06](../kit/STATUS.md) |
| FOOD-A9-F2-P4 | **Who pays the fee.** Decides whether the customer, the restaurant or both pay the platform fee, before the price calculator is built. | research | small | FOOD-A9-F2-P3 |  |
| FOOD-A9-F2-P5 | **Food's card surcharge cases.** Food's pickup and pay-at-table orders put to the kit's card fee pass-through check, so passing payment charges through stays lawful once the planned in-store surcharge ban applies. | legal | small | KIT-A3-F4-P3, FOOD-A9-F2-P2 |  |
| FOOD-A9-F2-P6 | **First-period fee.** Food's first fee, when there is no last period, set from the kit's launch cost estimate and Food's expected orders by the kit's start-up fee rule, then trued up once real costs are known. | library | small | KIT-A13-F3-P4, KIT-A13-F3-P3, FOOD-A15-F6-P2 | [K06](../kit/STATUS.md) |
| FOOD-A9-F3-P1 | **Tip flow.** Food's tip timing on the kit's tips: a tip set at checkout shows on the courier's offer and can only go up once a courier accepts; after delivery a customer can add or raise a tip for a set time, but never lower one. | interface | small | KIT-A3-F6-P1 |  |
| FOOD-A9-F4-P1 | **Small-order fee.** An optional fee below a set order value, voted by members. | library | small | svc-voting |  |
| FOOD-A9-F4-P2 | **Busy-time bonus.** A courier bonus at busy times, paid from a fee shown to the customer. | library | small | svc-voting, KIT-A12-F4-P3, FOOD-A15-F2-P3 | [FD02](STATUS.md) |
| FOOD-A9-F5-P1 | **Food deal kinds.** Food's deal kinds (free delivery, dish deals and free items) on the kit's promotions and vouchers, always paid for by the restaurant, so the courier still gets the whole delivery fee. | library | small | KIT-A5-F14-P2 |  |
| FOOD-A9-F5-P2 | **Deal set-up screen.** Create a deal and see how it performed in the restaurant app. | interface | small | FOOD-A9-F5-P1 |  |
| FOOD-A9-F6-P1 | **Promo codes.** Codes with limits, paid for by a restaurant, or from the fee only if members vote to include marketing in it. | service | small | FOOD-A9-F5-P1, FOOD-A9-F2-P3, KIT-A5-F14-P1, KIT-A5-F14-P2 |  |
| FOOD-A9-F6-P2 | **Referral credit.** Invite links with credit paid for by a restaurant, or from the fee only if members vote for it, with limits to stop abuse. | service | small | FOOD-A12-F5-P1, FOOD-A9-F2-P3, KIT-A14-F7-P1, KIT-A5-F14-P1, KIT-A5-F14-P2 |  |
| FOOD-A9-F7-P1 | **Food gift cards.** Food gift cards on the kit's gift cards, held and settled by the payment provider. | service | small | KIT-A3-F15-P2, KIT-A3-F15-P1 |  |
| FOOD-A9-F8-P1 | **Customer member benefits question.** What customers who take part as members would get, put to a vote. | research | small | svc-voting | [K01](../kit/STATUS.md) |

## FOOD-A10 Payments, payouts and tax

The money: held by a regulated payment provider, split between the restaurant, the courier, the at-cost fee, sales tax and the provider's charge, paid out daily, and taxed as each country's law requires.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A10-F1 Pay and hold | A customer's payment is approved when they order and taken on hand-over, held all the while by the payment provider. | core | Hold and capture; Failed payment handling |
| FOOD-A10-F2 Order money split | Each order's money splits automatically between the restaurant, the courier, the at-cost fee, the sales tax the platform collects and the payment provider's charge. | core | Split rules; Split record |
| FOOD-A10-F3 Payouts | Restaurants and couriers are paid daily, with every deduction shown. | core | Food payout timing; Payout statements |
| FOOD-A10-F4 Sales tax on the listed service (example: New Zealand GST) | The platform collects sales tax on food delivery where the law makes the platform collect it (example: New Zealand GST on listed services), and gives sellers what the law says they need. | core | What counts as the listed service; Food sales tax rules; Food lines for sales tax returns; Tax invoices |
| FOOD-A10-F5 Income reporting to the tax office (example: New Zealand's IRD) | The platform reports sellers' income as the digital platform rules require. | core | Food income reporting |
| FOOD-A10-F6 Refunds | A customer gets a refund quickly, and its cost falls on whoever caused the problem. | core | Food refund rules; Money back by default |
| FOOD-A10-F7 Bank chargebacks | The co-op can answer a bank chargeback with the order's evidence. | next | Food chargeback evidence |
| FOOD-A10-F8 Anti-money-laundering duties | The co-op meets the anti-money-laundering rules for online marketplaces. | core | Food money-laundering risks; Suspicious pattern checks; Checks on restaurants and couriers; Sanctions screening |
| FOOD-A10-F9 Cash on pickup | A restaurant can take cash for pickup orders if members choose to allow it. | later | Cash order record |
| FOOD-A10-F10 Debts and negative balances | A restaurant or courier whose refunds or fees exceed their next payout sees the debt, how it will be recovered and the limits on recovery. | core | Negative balance rules; Leaving while owing |
| FOOD-A10-F11 Tax set aside from payouts | A courier chooses a rate of tax to be held back from each payout, so the year-end bill is no shock. | next | Voluntary withholding with the tax office (example: New Zealand's IRD); Withholding from payouts |
| FOOD-A10-F12 Safe payout changes | A restaurant's or courier's payout account cannot be quietly redirected after an account takeover. | core | Food payout change rules; Two-person approval |
| FOOD-A10-F13 Warning before the sales tax threshold | A courier or a small restaurant is told when its sales near the sales tax registration threshold, and what registering would change. | next | Sales tax threshold warning |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A10-F1-P1 | **Hold and capture.** Food's timing on the kit's hold and release rules: approve at order, and take the money at collection or delivery. | integration | small | KIT-A3-F2-P1 |  |
| FOOD-A10-F1-P2 | **Failed payment handling.** Retry, ask for another method, or cancel before the kitchen starts. | service | small | FOOD-A10-F1-P1 |  |
| FOOD-A10-F2-P1 | **Split rules.** Food's split on the kit's multi-party orders: the restaurant gets the menu total less its own deals, the courier the delivery fee and tips, and the fee, the sales tax for the tax office and the provider's charge each get their own line. | library | medium | KIT-A5-F11-P1, KIT-A4-F1-P2, FOOD-A9-F2-P4 | [K02](../kit/STATUS.md) |
| FOOD-A10-F2-P2 | **Split record.** Every split stored as operations linked to the order, so payouts, sales tax and the kit's reconciliation agree. | data | small | kit-ops, FOOD-A10-F2-P1, KIT-A3-F9-P1 |  |
| FOOD-A10-F3-P1 | **Food payout timing.** Daily payouts to restaurants and couriers on the kit's payout runs. | integration | small | KIT-A3-F3-P2 |  |
| FOOD-A10-F3-P2 | **Payout statements.** Food's statement lines (orders, fees, refunds and tips) on the kit's earnings page and receipts. | service | small | KIT-A3-F3-P3, KIT-A3-F7-P1, FOOD-A10-F2-P2 |  |
| FOOD-A10-F4-P1 | **What counts as the listed service.** Which parts of a food order the marketplace rules cover, and how restaurants' own sales tax fits, using the seller of record decision. | legal | small | KIT-A4-F1-P1, FOOD-A11-F5-P2 | [K02](../kit/STATUS.md) |
| FOOD-A10-F4-P2 | **Food sales tax rules.** Food's sales tax rules for each order line on the kit's sales tax engine, including flat-rate credit notices to unregistered restaurants and couriers. | library | small | KIT-A4-F1-P2, KIT-A4-F1-P3, FOOD-A10-F4-P1 |  |
| FOOD-A10-F4-P4 | **Food lines for sales tax returns.** Food's order lines in the kit's operator sales tax returns. | data | small | KIT-A4-F9-P2, FOOD-A10-F4-P2 |  |
| FOOD-A10-F4-P5 | **Tax invoices.** Tax invoices for customers and business accounts, invoices to restaurants and couriers for the platform fee they pay, and buyer-created tax invoices where the platform issues them for couriers, on the kit's tax invoices. | service | small | KIT-A4-F2-P1, FOOD-A10-F4-P2, KIT-A4-F9-P3, FOOD-A11-F5-P2 |  |
| FOOD-A10-F5-P1 | **Food income reporting.** Food's seller details for couriers, and for restaurants if the rules cover them, on the kit's yearly report to the tax office. | service | small | KIT-A4-F3-P2, KIT-A4-F3-P3, FOOD-A10-F2-P2 | [K02](../kit/STATUS.md) |
| FOOD-A10-F6-P1 | **Food refund rules.** Food's refund cases and who bears each cost, on the kit's refunds. | service | small | KIT-A3-F5-P1, FOOD-A10-F2-P2 |  |
| FOOD-A10-F6-P2 | **Money back by default.** Every refund goes back the way the customer paid unless they choose credit, and credit is never the only choice, as consumer law requires (example: New Zealand's Consumer Guarantees Act). | legal | small | FOOD-A10-F6-P1, FOOD-A11-F5-P1 |  |
| FOOD-A10-F7-P1 | **Food chargeback evidence.** The order events, photos and messages Food adds to the kit's card dispute answers. | service | small | KIT-A3-F5-P2, FOOD-A10-F2-P2 |  |
| FOOD-A10-F8-P1 | **Food money-laundering risks.** Food's own risks, such as people ordering from themselves to move money, fed into the kit's anti-money-laundering programme. | legal | small | KIT-A4-F5-P1 | [K02](../kit/STATUS.md) |
| FOOD-A10-F8-P2 | **Suspicious pattern checks.** Flag patterns such as people ordering from themselves to move money. | service | small | KIT-A4-F5-P3, KIT-A10-F12-P2, FOOD-A10-F2-P2, FOOD-A10-F8-P1 |  |
| FOOD-A10-F8-P3 | **Checks on restaurants and couriers.** The customer checks the programme requires for restaurants and couriers, run through the kit's customer checks with the identity and payment providers. | service | small | KIT-A4-F5-P2, FOOD-A10-F8-P1 |  |
| FOOD-A10-F8-P4 | **Sanctions screening.** Restaurants and couriers screened against sanctions lists through the kit's sanctions screening before their first payout. | integration | small | KIT-A4-F8-P1, FOOD-A10-F8-P3 |  |
| FOOD-A10-F9-P1 | **Cash order record.** Record cash orders outside the payment provider and invoice the fee. | service | small | FOOD-A10-F2-P1, FOOD-A10-F10-P1 |  |
| FOOD-A10-F10-P1 | **Negative balance rules.** Food's limits on the kit's negative balances: which Food refunds and fees can create a debt, the most taken from one payout, and when a role is paused. | library | small | KIT-A3-F16-P2, KIT-A3-F16-P1, FOOD-A11-F2-P3 |  |
| FOOD-A10-F10-P2 | **Leaving while owing.** Food's steps when someone leaves owing money, on the kit's loss rules: a payment plan, no hidden penalties, and recovery only by lawful means. | service | small | KIT-A3-F16-P1, KIT-A3-F16-P2, FOOD-A10-F10-P1 |  |
| FOOD-A10-F11-P1 | **Voluntary withholding with the tax office (example: New Zealand's IRD).** Confirms with the tax office how voluntary withholding works when a platform pays a courier, starting with New Zealand's IRD. The rule should move down into the kit, since Spare Work maps the same. | research | small | FOOD-A18-F3-P1 | [FD02](STATUS.md) |
| FOOD-A10-F11-P2 | **Withholding from payouts.** Holds back the courier's chosen rate from each payout on the kit's payout runs and shows it on the statement. The withholding itself should move down into the kit with the rule. | service | small | FOOD-A10-F11-P1, KIT-A3-F3-P2 |  |
| FOOD-A10-F12-P1 | **Food payout change rules.** Food's use of the kit's payout change guard for restaurant owners and couriers, with Food's waiting time before money goes to a new account. | service | small | KIT-A3-F3-P4 |  |
| FOOD-A10-F12-P2 | **Two-person approval.** Staff overrides and large manual refunds need a second person to approve them. | service | small | KIT-A22-F5-P1, KIT-A22-F6-P1 |  |
| FOOD-A10-F13-P1 | **Sales tax threshold warning.** Warns a courier or an unregistered restaurant as its sales in every Spareware app near the country's sales tax registration threshold (example: $60,000 for New Zealand GST), and explains what registering changes for flat-rate credits and invoices. How platform sales count towards the threshold is for the sales tax study to confirm (*unverified*), and the warning should move down into the kit, since Spare Ride and Spare Work need it. | service | small | FOOD-A10-F4-P1, KIT-A4-F1-P3, KIT-A3-F3-P3 |  |

## FOOD-A11 Support, refunds and disputes

When something goes wrong: report it in a few taps, get a fair answer fast, and appeal to a member panel.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A11-F1 Help centre | A person can find answers to common problems in their own language. | core | Help articles; Help inside the app |
| FOOD-A11-F2 Report a problem with an order | A customer reports missing, wrong, cold, spilled or late food with photos, and small claims are refunded in minutes. | core | Problem report flow; Instant refund rules; Who pays rules |
| FOOD-A11-F3 Talk to a person | A person can chat with a support worker about a live order, and a courier gets urgent help on the road. | core | Food support cases; Support chat; Courier urgent line; Published support levels |
| FOOD-A11-F4 Disputes and member panels | A restaurant, courier or customer can dispute a decision, and the money stays held until a member panel decides. | core | Food dispute types; Food panels; Appeals; Settle first |
| FOOD-A11-F5 Consumer law | Customers get what consumer law promises (example: New Zealand's Consumer Guarantees Act and Fair Trading Act). | core | Consumer law study; Seller of record for each order |
| FOOD-A11-F6 False claims | Repeated false claims are spotted and handled fairly, with a person always deciding. | next | Claim pattern checks |
| FOOD-A11-F7 Report sickness or an allergic reaction | A customer reports food sickness or an allergic reaction, a person acts on it at once, and the restaurant is told and paused when a pattern or the law calls for it. | core | Sickness and reaction report; Urgent triage; Evidence freeze; Restaurant notice and pause; Telling the food regulator and councils |
| FOOD-A11-F8 Take it further | A customer, restaurant or courier learns how to take a dispute to a small claims tribunal (example: New Zealand's Disputes Tribunal) or another outside route. | next | Outside routes for Food |
| FOOD-A11-F9 AI help with limits | A person gets quick answers from an AI helper that says it is an AI and hands over to a person whenever asked. | next | AI helper limits |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A11-F1-P1 | **Help articles.** Food's plain-language help for customers, restaurants and couriers, in the kit's help centre. | data | small | KIT-A21-F1-P1 |  |
| FOOD-A11-F1-P2 | **Help inside the app.** Food's help links on each order screen into the kit's help centre. | interface | small | KIT-A21-F1-P1, FOOD-A11-F1-P1 |  |
| FOOD-A11-F2-P1 | **Problem report flow.** Pick the order, the items and the problem, and add photos; sickness or an allergic reaction goes to the urgent path. | interface | medium | KIT-A21-F2-P1, KIT-A15-F7-P1 |  |
| FOOD-A11-F2-P2 | **Instant refund rules.** Small claims refunded at once within published limits; larger ones go to a person. | library | medium | FOOD-A10-F6-P1 |  |
| FOOD-A11-F2-P3 | **Who pays rules.** Decide whether the restaurant, the courier, the platform or nobody bears the cost, by published rules. | library | small | FOOD-A11-F2-P2 |  |
| FOOD-A11-F3-P1 | **Food support cases.** Food's case types and order history views in the kit's support console. | interface | small | KIT-A21-F2-P1, KIT-A21-F2-P2 |  |
| FOOD-A11-F3-P2 | **Support chat.** Chat with a support worker from the order, on the kit's chat and support cases. | service | small | KIT-A21-F2-P1, KIT-A8-F1-P2, FOOD-A11-F3-P1 |  |
| FOOD-A11-F3-P3 | **Courier urgent line.** Food's urgent cases for couriers mid-job, such as a closed restaurant or a crash, on the kit's urgent support routing. | service | small | KIT-A21-F3-P1, KIT-A10-F1-P2 |  |
| FOOD-A11-F3-P4 | **Published support levels.** Support hours and response times, a phone line for restaurants during service, and the languages covered, published and measured. | service | small | KIT-A21-F4-P1, FOOD-A11-F3-P1 |  |
| FOOD-A11-F4-P1 | **Food dispute types.** Food's dispute categories and the payouts each one holds, on the kit's disputes service. | service | small | KIT-A11-F1-P1 |  |
| FOOD-A11-F4-P2 | **Food panels.** Food panels on the kit's panel selection and workspace, balanced for each case: a customer's claim against a restaurant or a courier is never heard by a panel drawn only from the side complained about, and outcomes are published without names. | service | small | KIT-A11-F4-P1, KIT-A11-F4-P2, FOOD-A11-F4-P1 |  |
| FOOD-A11-F4-P3 | **Appeals.** Food decisions go to the kit's appeals, with written reasons. | service | small | KIT-A11-F5-P1 |  |
| FOOD-A11-F4-P4 | **Settle first.** Every dispute starts with a settlement offer through the kit's settlement offers and dispute clocks, and claims under a published amount are settled by the refund rules, never sent to a panel. | library | small | KIT-A11-F3-P1, KIT-A11-F6-P1, FOOD-A11-F2-P2 |  |
| FOOD-A11-F5-P1 | **Consumer law study.** Who owes which guarantee when a restaurant, a courier and a platform share one order. | legal | small | KIT-A18-F8-P1 | [FD01](STATUS.md) |
| FOOD-A11-F5-P2 | **Seller of record for each order.** Food's case for the kit's seller of record decision: the restaurant sells the meal, and the courier or the co-op sells the delivery, which sets who charges sales tax, who owes consumer guarantees, whose name is on each receipt and who answers card disputes. | legal | small | KIT-A3-F1-P5, FOOD-A11-F5-P1 | [K02](../kit/STATUS.md) |
| FOOD-A11-F6-P1 | **Claim pattern checks.** Patterns across customers, couriers and restaurants, flagged for a person to review. | service | medium | KIT-A10-F12-P2, FOOD-A11-F2-P1 |  |
| FOOD-A11-F7-P1 | **Sickness and reaction report.** A separate, urgent report for illness or an allergic reaction, which first tells the person to call the emergency number or a health line if they need care (example: 111 or Healthline in New Zealand). | interface | small | KIT-A10-F3-P2, FOOD-A11-F2-P1 |  |
| FOOD-A11-F7-P2 | **Urgent triage.** A person reviews every sickness or reaction report straight away, at any hour. | service | medium | KIT-A21-F3-P1, KIT-A10-F1-P2, FOOD-A11-F7-P1 |  |
| FOOD-A11-F7-P3 | **Evidence freeze.** The order, the dish details, photos and chat are frozen as soon as a report is made. | service | small | KIT-A11-F2-P1, FOOD-A11-F7-P1 |  |
| FOOD-A11-F7-P4 | **Restaurant notice and pause.** Tells the restaurant, and pauses it when reports form a pattern or a council closes it. | service | small | FOOD-A5-F4-P1, FOOD-A11-F7-P2, FOOD-A1-F2-P3 |  |
| FOOD-A11-F7-P5 | **Telling the food regulator and councils.** When the law asks the restaurant or the platform to tell the food safety regulator (example: New Zealand's MPI) or the council, and how the platform helps. | legal | small | FOOD-A18-F2-P1 | [FD01](STATUS.md) |
| FOOD-A11-F8-P1 | **Outside routes for Food.** Food's links to small claims tribunals and other outside routes, on the kit's guidance. | data | small | KIT-A11-F7-P1 |  |
| FOOD-A11-F9-P1 | **AI helper limits.** The kit's help assistant answers Food questions first, says it is an AI, hands over to a person on request, and cannot go past refund limits. | service | small | KIT-A17-F7-P1, FOOD-A11-F2-P2, FOOD-A11-F3-P2 |  |

## FOOD-A12 Ratings, trust and safety

Two-way ratings, safety tools for couriers and customers, and checks that keep bad actors out, with every decision logged in public.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A12-F1 Two-way ratings | A customer rates the food and the delivery separately, and couriers, restaurants and customers rate each other. | core | Separate food and delivery ratings; Customer ratings; Couriers and restaurants rate each other; Challenge a rating |
| FOOD-A12-F2 Reviews | A customer can write a review, and the restaurant can reply in public. | next | Review and reply; Review moderation; Fake review checks |
| FOOD-A12-F3 Fair removal | No courier or restaurant is removed by an algorithm alone: they get a reason, a chance to answer and an appeal. | core | Food removal steps |
| FOOD-A12-F4 Courier safety tools | A courier can call for help, share their job with someone and report unsafe places, under a health and safety plan. | core | Emergency button; Trip sharing; Incident reports; Food's health and safety facts; Health and safety plan; Fatigue limits; Workplace safety notices; After violence or a crash |
| FOOD-A12-F5 Fraud and account safety | The platform spots stolen cards, fake accounts and shared courier accounts. | core | Fraud signals; Courier identity re-check |
| FOOD-A12-F6 Food safe in transit | Food arrives safe: sealed bags, insulated gear and published rules for couriers. | next | Tamper-evident seals; Food in transit rules |
| FOOD-A12-F7 Public moderation log | Anyone can see each removal or ban and the rule behind it, without names. | core | Moderation log |
| FOOD-A12-F8 Customer safety | A customer can report a courier or restaurant for unsafe behaviour and block them from future orders. | core | Report and block |
| FOOD-A12-F9 Insurance in place | Couriers, customers and the co-op are covered by insurance that is bought, not just studied. | core | Co-op cover; Car policy check; Income cover beyond public accident cover; Crash report and claim |
| FOOD-A12-F10 Warnings about unsafe addresses | A courier sees hazard notes on an address, such as a dog, steep steps or an unsafe entry, before accepting. | next | Address hazard notes |
| FOOD-A12-F11 Report sexual harm safely | A courier, customer or restaurant worker reports sexual harm from any order and is looked after by trained responders. | core | Food's sexual harm path |
| FOOD-A12-F12 Treated fairly whoever you are | Couriers, customers and restaurant staff all agree not to refuse or treat anyone worse because of who they are. | core | Food's non-discrimination rules |
| FOOD-A12-F13 Checks for unfair declines | Patterns of declines and cancels that point to discrimination are spotted for a person to review. | next | Food decline pattern checks |
| FOOD-A12-F14 Report a courier on the road | Anyone, with or without an account, can report a courier's riding or driving. | later | Public road report |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A12-F1-P1 | **Separate food and delivery ratings.** Food's two rating subjects (the food and the delivery) on the kit's ratings, so nobody is blamed for the other's mistake. | service | small | KIT-A9-F1-P1, KIT-A9-F3-P1 |  |
| FOOD-A12-F1-P2 | **Customer ratings.** Couriers and restaurants rate customers, and customers can see their own rating. | service | small | KIT-A9-F1-P1 |  |
| FOOD-A12-F1-P3 | **Couriers and restaurants rate each other.** Couriers rate restaurants on waits, packing and hand-over, and restaurants rate couriers, feeding the quality report and waiting pay. | service | small | KIT-A9-F1-P1 |  |
| FOOD-A12-F1-P4 | **Challenge a rating.** A courier, restaurant or customer challenges a single unfair rating with the order's facts, through the kit's rating challenge. | interface | small | KIT-A9-F5-P1, FOOD-A12-F1-P1 |  |
| FOOD-A12-F2-P1 | **Review and reply.** Food's review fields (food, packing and delivery) on the kit's reviews, with the restaurant's public reply. | service | small | KIT-A9-F2-P1 |  |
| FOOD-A12-F2-P2 | **Review moderation.** Food's review rules on the kit's moderation queue and fair review rules, including how defamation complaints about reviews are handled, with each removal logged. | service | small | KIT-A10-F6-P2, KIT-A9-F2-P2, FOOD-A12-F2-P1 |  |
| FOOD-A12-F2-P3 | **Fake review checks.** Food's signs of review rings and attacks by rival restaurants, such as orders placed only to leave a review, on the kit's fake review checks, for a person to review. | service | small | KIT-A9-F8-P1, FOOD-A12-F2-P1 |  |
| FOOD-A12-F3-P1 | **Food removal steps.** Food's warnings and rating levels on the kit's rating thresholds and statements of reasons, with review by a person and an appeal to a member panel. | legal | small | KIT-A9-F4-P1, KIT-A10-F8-P1, FOOD-A11-F4-P2 | [FD02](STATUS.md) |
| FOOD-A12-F4-P1 | **Emergency button.** The kit's emergency button on every courier and customer job screen. | interface | small | KIT-A10-F1-P1 |  |
| FOOD-A12-F4-P2 | **Trip sharing.** A courier shares the live job with a trusted contact through the kit's trusted contact link. | service | small | KIT-A10-F2-P1 |  |
| FOOD-A12-F4-P3 | **Incident reports.** Food's incident types on the kit's incident register and reports service: crashes, injuries, near misses, dog attacks, threats, harassment, and sexual harm, which goes only to the sexual harm path, with follow-up from a person. | service | small | KIT-A24-F2-P1, KIT-A10-F3-P1, FOOD-A12-F11-P1 |  |
| FOOD-A12-F4-P4 | **Food's health and safety facts.** Food's facts for the kit's health and safety duties: riding and driving at night, doorstep encounters, carrying hot or heavy orders, and severe weather. | legal | small | KIT-A24-F1-P1 | [FD02](STATUS.md) |
| FOOD-A12-F4-P5 | **Health and safety plan.** The co-op's plan for courier fatigue, violence, severe weather and crashes, as a business owing health and safety duties. | legal | medium | FOOD-A12-F4-P4 |  |
| FOOD-A12-F4-P6 | **Fatigue limits.** Food's limits on the kit's work-time limits: nudges after long hours online, and a voted limit on hours in a day, counted across every Spareware app. | library | small | KIT-A24-F3-P1, FOOD-A6-F1-P4, svc-voting, FOOD-A12-F4-P5 |  |
| FOOD-A12-F4-P7 | **Workplace safety notices.** Flags Food incidents that must be notified to the workplace safety regulator (example: New Zealand's WorkSafe), such as a death or a serious injury in a crash, and tracks the notice and keeping the scene as it was, on the kit's notifiable event reports. | service | small | KIT-A24-F2-P2, FOOD-A12-F4-P3 |  |
| FOOD-A12-F4-P8 | **After violence or a crash.** Food's steps after a courier is assaulted or crashes mid-job, on the kit's violence and crash handling: safety first, the order passed on or written off at no cost to the courier, support, the incident record and insurance. | service | small | KIT-A24-F4-P1, FOOD-A7-F4-P1, FOOD-A12-F4-P3 |  |
| FOOD-A12-F5-P1 | **Fraud signals.** Food's fraud rules (new devices, mismatched addresses and card testing) on the kit's fraud signals and the provider's screening. | service | medium | KIT-A10-F12-P2, KIT-A3-F13-P1 |  |
| FOOD-A12-F5-P2 | **Courier identity re-check.** Food's re-check timing, such as the start of some shifts, on the kit's repeat face checks, which bring the fairness testing, the non-biometric option and a person reviewing every failure. | integration | small | KIT-A2-F12-P2, KIT-A2-F12-P1, FOOD-A1-F3-P8 |  |
| FOOD-A12-F6-P1 | **Tamper-evident seals.** Seal guidance for restaurants and a prompt for customers to check the seal. | interface | small | FOOD-A5-F3-P1 | [FD01](STATUS.md) |
| FOOD-A12-F6-P2 | **Food in transit rules.** What couriers must do to keep food safe and suitable on the way. | research | small | FOOD-A18-F2-P1 | [FD01](STATUS.md) |
| FOOD-A12-F7-P1 | **Moderation log.** Food's trust decisions published through the kit's public removal log. | service | small | KIT-A10-F7-P1 |  |
| FOOD-A12-F8-P1 | **Report and block.** Report a person from the order and block them through the kit's blocking, so they are never matched again. | service | small | KIT-A10-F4-P1, KIT-A10-F3-P1 |  |
| FOOD-A12-F9-P1 | **Co-op cover.** Third-party liability and property damage cover for couriers on bikes, scooters and cars, bought before launch. | legal | medium | FOOD-A18-F3-P2 |  |
| FOOD-A12-F9-P2 | **Car policy check.** A courier on a car confirms their own policy covers delivery work, recorded as a check result. | service | small | KIT-A2-F2-P2, FOOD-A12-F9-P1 |  |
| FOOD-A12-F9-P3 | **Income cover beyond public accident cover.** Options for income cover beyond public accident cover (example: New Zealand's ACC), such as a co-op group policy. | research | small | FOOD-A18-F3-P2 |  |
| FOOD-A12-F9-P4 | **Crash report and claim.** A courier reports a crash with photos and details, and starts the insurance claim, through the kit's violence and crash handling and insurance claims link. | service | medium | KIT-A24-F4-P1, KIT-A11-F9-P2, FOOD-A12-F4-P3, FOOD-A12-F9-P1 |  |
| FOOD-A12-F10-P1 | **Address hazard notes.** Couriers add hazard notes to an address, shown on later offers to that address, on the kit's hazard notes; notes describe the place, never the people. | library | small | KIT-A24-F5-P1, FOOD-A6-F2-P1 |  |
| FOOD-A12-F11-P1 | **Food's sexual harm path.** Every Food report screen and the emergency button lead to the kit's sexual harm path, handled only by trained responders, with the reporter choosing whether police are told and the two people never matched again. | service | small | KIT-A10-F18-P2, KIT-A10-F4-P1 |  |
| FOOD-A12-F12-P1 | **Food's non-discrimination rules.** The kit's non-discrimination commitment, accepted at sign-up by couriers, customers and restaurant staff, with Food's examples, such as never refusing an order or a courier because of a name, a religion or a race. | legal | small | KIT-A10-F19-P1, FOOD-A1-F3-P5 |  |
| FOOD-A12-F13-P1 | **Food decline pattern checks.** Declines and cancels by couriers and restaurants fed to the kit's decline pattern checks, for a person to review, never for automatic penalties. | service | small | KIT-A6-F5-P3, FOOD-A12-F12-P1, FOOD-A6-F2-P3 |  |
| FOOD-A12-F14-P1 | **Public road report.** A web form with no sign-in that takes the place, the time and a short code shown on the courier's bag, which says nothing about the courier, matched to the job by the kit's reports service, with spam limits and the courier's name never shown to the reporter. | service | small | KIT-A10-F3-P1, KIT-A10-F11-P1 |  |

## FOOD-A13 Restaurant business tools

What a restaurant needs to run its delivery trade: reports, payouts, feedback, help, and its own ordering link at the same at-cost fee.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A13-F1 Sales and payout reports | A restaurant sees sales, fees, refunds and payouts by day, week and month, and downloads them. | core | Sales dashboard; Accounting export |
| FOOD-A13-F2 Menu performance | A restaurant sees which dishes sell, when they sell out and what each one earns. | next | Dish insights |
| FOOD-A13-F3 Order quality feedback | A restaurant sees its ratings, complaints and accuracy, by dish and over time, and what to fix first. | core | Quality report |
| FOOD-A13-F4 Own ordering link | A restaurant shares a link or QR code so customers order straight from its own website, at the same at-cost fee. | next | Direct ordering page; QR code and website embed |
| FOOD-A13-F5 Order at the table | A diner scans a code at the table and orders and pays from their phone. | later | Table ordering mode |
| FOOD-A13-F6 Message followers | A restaurant can tell customers who chose to follow it about a new menu, without paying for ads. | later | Follower messages |
| FOOD-A13-F7 Monthly tax summary | A restaurant gets the sales tax and income figures it needs each month. | core | Tax summary |
| FOOD-A13-F8 Set-up help and test orders | A restaurant learns the app quickly and can send a test order before going live. | core | Set-up guide; Test order mode |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A13-F1-P1 | **Sales dashboard.** Orders, sales, fees and refunds with simple charts. | interface | medium | FOOD-A10-F2-P2 |  |
| FOOD-A13-F1-P2 | **Accounting export.** Food's sales and fee lines in the kit's accounting exports. | integration | small | KIT-A4-F4-P2, FOOD-A10-F3-P2 |  |
| FOOD-A13-F2-P1 | **Dish insights.** Orders, sales and sold-out time for each dish; ratings and complaints stay in the quality report. | service | small | FOOD-A13-F1-P1 |  |
| FOOD-A13-F3-P1 | **Quality report.** Missing-item rate, prep-time accuracy, complaints and ratings by dish and over time, including couriers' ratings of waits and packing. | interface | small | FOOD-A12-F1-P1, FOOD-A5-F2-P2, FOOD-A12-F1-P3, FOOD-A11-F2-P1 |  |
| FOOD-A13-F4-P1 | **Direct ordering page.** A simple page in the restaurant's own colours and logo that uses the same menu and checkout, on a shared shop page the kit should open now, since Spare Market needs one too. | interface | medium | FOOD-A4-F1-P2 |  |
| FOOD-A13-F4-P2 | **QR code and website embed.** QR codes and a website embed for the ordering page. | interface | small | FOOD-A13-F4-P1 |  |
| FOOD-A13-F5-P1 | **Table ordering mode.** Table number on the order and pay-at-table through the same checkout, on a shared till the kit should open, since Spare Market needs one too. | interface | medium | FOOD-A13-F4-P1 |  |
| FOOD-A13-F6-P1 | **Follower messages.** Messages only to customers who opted in, within the anti-spam law. | service | small | KIT-A8-F3-P2, KIT-A8-F5-P2 |  |
| FOOD-A13-F7-P1 | **Tax summary.** Sales, sales tax handled by the platform, and fees for each month. | service | small | FOOD-A10-F4-P2 |  |
| FOOD-A13-F8-P1 | **Set-up guide.** Step-by-step set-up with checks for menu, hours and payouts. | interface | small | FOOD-A1-F2-P1 |  |
| FOOD-A13-F8-P2 | **Test order mode.** Run a test order through the whole flow without charging anyone. | service | small | FOOD-A5-F1-P3, KIT-A16-F3-P3 |  |

## FOOD-A14 More than restaurant meals

The other things the big apps deliver: groceries and convenience goods, alcohol with age checks, catering, business accounts, and couriers for other sellers.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A14-F1 Grocery and convenience shops | A shop lists thousands of items, and a picker and courier fill the order. | later | Large catalogues; Picker app; Items sold by weight; Catalogue import; Product recalls; Grocery information law |
| FOOD-A14-F2 Alcohol delivery | A customer of legal age orders alcohol from a licensed seller within the remote-sale rules, with ID checked at the door. | later | Alcohol law study; Seller licence record; Age check at order; ID check at the door; Refused delivery return; Alcohol delivery hours; Licence on the page and receipt |
| FOOD-A14-F3 Catering and large orders | A business orders food for a group days ahead and gets an invoice. | later | Large order rules |
| FOOD-A14-F4 Business accounts | A company pays for staff meals with budgets and a monthly invoice. | later | Food business accounts; Monthly invoicing |
| FOOD-A14-F5 Couriers for other sellers | A shop or restaurant with its own website books a Spare Food courier for its own orders. | later | Delivery booking commands |
| FOOD-A14-F6 Parcels and errands | A person sends a small parcel across town with a courier. | later | Parcel rules |
| FOOD-A14-F7 Medicines and other restricted goods | The co-op knows whether and how prescription medicines, vapes and other restricted goods could ever be delivered. | later | Restricted goods study |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A14-F1-P1 | **Large catalogues.** Thousands of items with barcodes, categories and weights. | data | medium | FOOD-A3-F1-P1, kit-store, KIT-A5-F1-P1, KIT-A5-F10-P1 |  |
| FOOD-A14-F1-P2 | **Picker app.** A picking list with barcode scanning and swaps by the customer's own rules. | interface | large | FOOD-A5-F5-P1 |  |
| FOOD-A14-F1-P3 | **Items sold by weight.** Price adjusted after picking, within a limit the customer agreed. | library | small | FOOD-A14-F1-P1 |  |
| FOOD-A14-F1-P4 | **Catalogue import.** Load a shop's product list from a spreadsheet or its stock system. | integration | medium | FOOD-A14-F1-P1 |  |
| FOOD-A14-F1-P5 | **Product recalls.** Items on the food regulator's recall list (example: New Zealand's MPI) are pulled from grocery listings, and buyers are told. | service | small | FOOD-A14-F1-P1 |  |
| FOOD-A14-F1-P6 | **Grocery information law.** The consumer information standards that cover groceries sold online, such as country of origin for some foods and unit pricing, and weights and measures law for items sold by weight. | legal | small | FOOD-A14-F1-P1, FOOD-A14-F1-P3 |  |
| FOOD-A14-F2-P1 | **Alcohol law study.** Remote sales under each country's alcohol law, starting with New Zealand's Sale and Supply of Alcohol Act 2012 and its regulations: what the seller and the platform must each do, and on which days and hours delivery is barred. | legal | medium |  | [FD01](STATUS.md) |
| FOOD-A14-F2-P2 | **Seller licence record.** Each seller's off-licence, its conditions and expiry. | service | small | FOOD-A1-F2-P2, KIT-A2-F2-P1 |  |
| FOOD-A14-F2-P3 | **Age check at order.** Two age declarations made by the buyer in person, never by an AI agent, one on entering the store and one just before the sale, as the remote-sale regulations ask. | interface | small | FOOD-A4-F6-P1, FOOD-A14-F2-P1, FOOD-A20-F6-P3 |  |
| FOOD-A14-F2-P4 | **ID check at the door.** The courier checks the receiver's ID, and that they are not intoxicated, before hand-over and records it; no ID means no hand-over. | service | medium | FOOD-A6-F4-P2 |  |
| FOOD-A14-F2-P5 | **Refused delivery return.** Return refused alcohol to the seller, with the refund rules applied. | service | small | FOOD-A14-F2-P4, FOOD-A10-F6-P1 |  |
| FOOD-A14-F2-P6 | **Alcohol delivery hours.** Each country pack sets its own hours. In the first pack (New Zealand): no alcohol delivered from 11pm to 6am (section 59), none on Good Friday, Easter Sunday or Christmas Day or before 1pm on Anzac Day (section 48, which section 49 applies to remote-sale deliveries), and any narrower hours in the seller's licence conditions or a local alcohol policy that the law study finds apply. | library | small | FOOD-A14-F2-P1, FOOD-A14-F2-P2 |  |
| FOOD-A14-F2-P7 | **Licence on the page and receipt.** The licence holder's name, licence number and expiry on the store page and on every receipt, with an image of the licence or a link to one. | data | small | FOOD-A14-F2-P2 |  |
| FOOD-A14-F3-P1 | **Large order rules.** Lead times, minimums and vehicle needs for big orders. | library | small | FOOD-A4-F4-P1, FOOD-A6-F9-P1 |  |
| FOOD-A14-F4-P1 | **Food business accounts.** Food's meal budgets, allowed times and staff invites on the kit's business accounts on credit. | service | small | KIT-A3-F17-P1, KIT-A1-F7-P1 |  |
| FOOD-A14-F4-P2 | **Monthly invoicing.** Food's order lines on the kit's one monthly invoice with sales tax, paid by bank transfer. | service | small | KIT-A3-F17-P1, FOOD-A10-F4-P5 |  |
| FOOD-A14-F5-P1 | **Delivery booking commands.** Book, track and pay for a Food courier through the command API, as Food's rules on a shared A-to-B delivery job type the kit must open, since Spare Market's shipping needs it too. | service | medium | FOOD-A20-F1-P1, KIT-A5-F2-P1, KIT-A6-F3-P2 |  |
| FOOD-A14-F6-P1 | **Parcel rules.** Food's size limits, banned items and proof of delivery for parcels, on the same shared A-to-B delivery job type the kit must open. | library | small | KIT-A5-F2-P1, KIT-A5-F7-P2 |  |
| FOOD-A14-F7-P1 | **Restricted goods study.** The rules for medicines, vapes, tobacco and other restricted goods. | legal | small |  |  |

## FOOD-A15 Member voting and the open cost ledger

Restaurants and couriers keep all the profit except running costs: they vote on fees and rules, see exactly what it costs to run, and decide where it opens next.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A15-F1 Vote on fees and rules | A member proposes and votes on the delivery fee formula, the platform rules and the dispatch rules. | core | Food proposal types; Proposal and vote screens; Results become settings |
| FOOD-A15-F2 A voice for each side | Couriers decide courier pay, restaurants decide restaurant rules, and both decide shared rules. | core | Who votes on what; Business members; Competition law gate for pay votes |
| FOOD-A15-F3 Open cost ledger | Anyone can see each month's running cost, the number of orders, and what the fee paid for. | core | Spare Food ledger page |
| FOOD-A15-F4 Surplus lowers the next fee | Any surplus is shown on the ledger and lowers the next period's fee. | core | Surplus rollover |
| FOOD-A15-F5 Learn from other courier co-ops | A member can read how other courier co-ops ran, what worked and what failed. | next | Courier co-op study |
| FOOD-A15-F6 Open a city | Members decide where Spare Food opens next, and it opens once enough restaurants, couriers and customers have pledged and signed up. | core | Launch readiness tracker; First city and launch plan; Whether food goes first; Restaurant outreach and pledges; Courier sign-up before launch; Waitlist for unserved addresses |
| FOOD-A15-F7 Member meetings and reports | A member can read the annual report and take part in member meetings online. | next | Online member meetings |
| FOOD-A15-F8 A reserve for bad months | Anyone can see a voted reserve for chargebacks, fraud, legal claims and slow months, and how the first months are funded. | core | Food's reserve; Start-up funding |
| FOOD-A15-F9 Emergency powers | Operators can act at once in an emergency, within published limits, and members review every such change afterwards. | core | Emergency powers rules; Emergency changes with review |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A15-F1-P1 | **Food proposal types.** What Spare Food members can vote on and who may vote on each, with every courier pay vote passing the competition law check first. | data | small | KIT-A12-F1-P1, KIT-A12-F4-P3 | [K01](../kit/STATUS.md) |
| FOOD-A15-F1-P2 | **Proposal and vote screens.** Food's proposal summaries, showing what each would change in the fees, on the kit's voting screen. | interface | small | KIT-A12-F3-P2, KIT-A12-F3-P1 |  |
| FOOD-A15-F1-P3 | **Results become settings.** Maps each passed Food vote to the setting it changes, applied by the kit. | service | small | KIT-A12-F4-P1 |  |
| FOOD-A15-F2-P1 | **Who votes on what.** Food's electorates (couriers, restaurants or both) on the kit's electorates and constitution rules. | legal | small | KIT-A12-F5-P1, KIT-A12-F5-P2, FOOD-A15-F2-P3 | [K01](../kit/STATUS.md) |
| FOOD-A15-F2-P2 | **Business members.** Who counts as the member when a restaurant is a business: one vote per company, per outlet or per owner, how franchise groups count, and whether staff vote. | legal | small | KIT-A12-F5-P2 | [K01](../kit/STATUS.md) |
| FOOD-A15-F2-P3 | **Competition law gate for pay votes.** Couriers vote on the delivery fee formula, the busy-time bonus and the waiting rate only in a form the kit's competition law check clears, such as pay set by the co-op as employer, a competition authority's authorisation (example: New Zealand's Commerce Commission), or a collective bargaining law if one applies. | legal | small | KIT-A12-F4-P3, FOOD-A18-F3-P1 | [FD02](STATUS.md) |
| FOOD-A15-F3-P1 | **Spare Food ledger page.** Food's monthly costs, order counts and resulting fee on the kit's ledger pages, as open data. | service | small | KIT-A13-F1-P3 |  |
| FOOD-A15-F4-P1 | **Surplus rollover.** Food's surplus carried into the next fee by the kit's fee setting. | library | small | KIT-A13-F3-P1, FOOD-A9-F2-P1 |  |
| FOOD-A15-F5-P1 | **Courier co-op study.** How existing courier co-ops run and their software, learning only from code whose licence allows it. | research | medium | KIT-A14-F10-P1 | [FD04](STATUS.md) |
| FOOD-A15-F6-P1 | **Launch readiness tracker.** Food's targets for restaurants, couriers and customers in each area, counted on the kit's launch areas before the area opens. The counting should move into the kit's marketplace engine for every marketplace. | service | small | KIT-A7-F5-P1, kit-market, FOOD-A19-F1-P1 |  |
| FOOD-A15-F6-P2 | **First city and launch plan.** Rank cities and plan how to sign up enough restaurants and couriers at the same time. | research | medium |  |  |
| FOOD-A15-F6-P3 | **Whether food goes first.** How Spare Food ranks against the other marketplaces as the first to build. | research | small |  | [K05](../kit/STATUS.md) |
| FOOD-A15-F6-P4 | **Restaurant outreach and pledges.** A list of restaurants to approach, and pledges to join once enough others in the area have pledged, on the same shared launch counting. | service | small | FOOD-A15-F6-P1, KIT-A7-F5-P1 |  |
| FOOD-A15-F6-P5 | **Courier sign-up before launch.** Couriers apply and pass their checks before an area opens, counted towards its target. | service | small | FOOD-A1-F3-P1, FOOD-A15-F6-P1 |  |
| FOOD-A15-F6-P6 | **Waitlist for unserved addresses.** Customers at addresses not yet served join a waitlist that counts towards opening their area on the kit's launch areas. | service | small | FOOD-A15-F6-P1, FOOD-A2-F1-P1, KIT-A7-F5-P1 |  |
| FOOD-A15-F7-P1 | **Online member meetings.** Food's papers and votes in the kit's member meetings. | service | small | KIT-A12-F7-P1 | [K01](../kit/STATUS.md) |
| FOOD-A15-F8-P1 | **Food's reserve.** Food's share and risks (chargebacks, fraud, food sickness claims and slow months) on the kit's voted reserve line, shown on the ledger. | library | small | KIT-A13-F3-P5, FOOD-A9-F2-P3 |  |
| FOOD-A15-F8-P2 | **Start-up funding.** How the first months are paid for (donations, loans or grants), shown on the ledger. | research | medium | KIT-A14-F1-P1, FOOD-A9-F2-P3, KIT-A13-F3-P3 |  |
| FOOD-A15-F9-P1 | **Emergency powers rules.** Food's list of emergency changes (pausing zones in a storm, adding a courier bonus, suspending a restaurant after an outbreak) and how long each may last. The emergency powers rule itself belongs with the constitution's voting rules in the kit. | legal | small | KIT-A12-F5-P1 | [K01](../kit/STATUS.md) |
| FOOD-A15-F9-P2 | **Emergency changes with review.** Each emergency change ends on its own after a set time and goes to members for review. | service | small | FOOD-A15-F9-P1, KIT-A12-F1-P1, FOOD-A19-F1-P2 |  |

## FOOD-A16 Apps, devices and offline

The three apps on phones, tablets and the web, working on old devices, weak signal and little data.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A16-F1 Customer app on any phone | A customer orders from a web app with no install, or from an installed phone app. | core | Food's phone needs; Customer web app; Installed customer app |
| FOOD-A16-F2 Restaurant app on a tablet | A restaurant runs the app on a cheap tablet, a phone or a computer it already owns. | core | Restaurant app shell; Shared kitchen tablets |
| FOOD-A16-F3 Courier location in the background | A courier's app keeps sharing location during a job with the phone locked, within app store rules. | core | Background location; Battery and data saver |
| FOOD-A16-F4 Work through weak signal | Each app keeps working when the network drops and catches up later. | core | Restaurant offline queue; Customer offline handling; Order clash rules |
| FOOD-A16-F5 Notifications a person chooses | Each person chooses what reaches them by push, text or email. | core | Notification settings |
| FOOD-A16-F6 Old phones and slow networks | The apps load fast on old phones and slow, costly mobile data. | next | Speed and size budget |
| FOOD-A16-F7 Courier app on a phone | A courier installs an app built for a phone cradle that keeps working through a whole shift. | core | Courier app shell |
| FOOD-A16-F8 Maps with no signal | A courier downloads map areas for places with weak signal. | next | Offline map areas |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A16-F1-P1 | **Food's phone needs.** Food's needs for the kit's phone app decision: background location for couriers, loud alerts for restaurants and lock-screen status for customers. | research | small | KIT-A20-F2-P1 | [K04](../kit/STATUS.md) |
| FOOD-A16-F1-P2 | **Customer web app.** Packs the customer screens from the other areas into the kit's app frame for any browser, with nothing to install. | interface | small | KIT-A20-F1-P2, FOOD-A4-F1-P2, FOOD-A2-F1-P2 |  |
| FOOD-A16-F1-P3 | **Installed customer app.** Food's customer app on the kit's installable app wrapper, with push and location. | interface | small | KIT-A20-F2-P2, FOOD-A16-F1-P2 |  |
| FOOD-A16-F2-P1 | **Restaurant app shell.** Food's restaurant app on the kit's installable app wrapper, keeping the screen awake and playing alerts. | interface | medium | KIT-A20-F2-P2, FOOD-A5-F1-P1 | [FD05](STATUS.md) |
| FOOD-A16-F2-P2 | **Shared kitchen tablets.** A kitchen tablet signs in to a location, not a person; staff switch by PIN, an owner signs it out remotely from the kit's device list, and payouts, bank details and staff admin never show on it. This shared device mode should move down into the kit, since Spare Market shops need it. | service | small | KIT-A1-F3-P2, KIT-A15-F1-P4, FOOD-A1-F4-P2 |  |
| FOOD-A16-F3-P1 | **Background location.** Food asks for background location only while a courier is on a job, using the kit's location permissions. | integration | small | KIT-A20-F2-P3 |  |
| FOOD-A16-F3-P2 | **Battery and data saver.** Fewer location updates while the courier is waiting. | library | small | FOOD-A16-F3-P1 |  |
| FOOD-A16-F4-P1 | **Restaurant offline queue.** Accept and mark orders while offline, synced in order once back. | library | medium | KIT-A15-F4-P2, FOOD-A5-F1-P3 |  |
| FOOD-A16-F4-P2 | **Customer offline handling.** Show the last known status and queue messages while offline. | library | small | KIT-A15-F4-P2 |  |
| FOOD-A16-F4-P3 | **Order clash rules.** Food's outcome for each clash between order changes made offline, such as a cancel while the kitchen marks the order ready, two couriers both delivering after a reassignment, or a refund while the courier confirms delivery. | library | medium | KIT-A15-F4-P3, FOOD-A5-F1-P3, FOOD-A5-F1-P5 |  |
| FOOD-A16-F5-P1 | **Notification settings.** Food's message kinds on the kit's notification settings, with safety notices always on. | interface | small | KIT-A8-F5-P1 |  |
| FOOD-A16-F6-P1 | **Speed and size budget.** Food's size and speed limits checked on every change, on the kit's light mode for slow connections. | service | small | KIT-A19-F5-P1 |  |
| FOOD-A16-F7-P1 | **Courier app shell.** Food's courier app on the kit's installable app wrapper, built for a phone cradle, with location on during jobs and the current job's map kept on the phone. | interface | medium | KIT-A20-F2-P2, FOOD-A16-F3-P1, FOOD-A6-F12-P1 |  |
| FOOD-A16-F8-P1 | **Offline map areas.** Map areas for Food's zones from the kit's offline map packs, offered to couriers who work there. | integration | small | KIT-A7-F7-P1, FOOD-A16-F7-P1, FOOD-A7-F7-P1 |  |

## FOOD-A17 Accessibility, languages and countries

Usable by everyone, in English and other languages, and ready for any country with its own rules pack; New Zealand is the first, with te reo Māori.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A17-F1 Accessible apps | A blind, low-vision, Deaf or motor-impaired person can use every app fully. | core | WCAG 2.2 AA on every screen; Accessibility checks; Message-only courier option; Display settings; Card fields tested too; Offers seen and felt |
| FOOD-A17-F2 Many languages | A person uses the apps in their own language, starting with English and the languages of each launch country (example: te reo Māori in New Zealand). | core | Food translation files; Launch country languages (example: te reo Māori); Chat translation |
| FOOD-A17-F3 Other countries | Spare Food runs in any country with that country's currency, tax, addresses and laws, set in its own country pack. | later | Food country settings; Food tax per country; Platform work law by country; Allergen lists per country |
| FOOD-A17-F4 Local formats | A person sees times, dates, money and addresses in their local way. | core | Locale formatting |
| FOOD-A17-F5 Courier and kitchen languages | Couriers and restaurant staff use the apps, and read their terms and safety lessons, in a language they read well. | next | Languages for couriers and kitchens; Terms and lessons in the courier's language |
| FOOD-A17-F6 Right-to-left languages | A person reads the apps in Arabic, Urdu or another right-to-left language. | later | Right-to-left screens |
| FOOD-A17-F7 Readable receipts and statements | A screen reader user reads every receipt, statement and invoice from Food. | next | Accessible Food documents |
| FOOD-A17-F8 Easy read and sign language | A person reads the key steps in easy-read text, or watches them in the country's sign language (example: New Zealand Sign Language). | later | Food easy read and sign language |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A17-F1-P1 | **WCAG 2.2 AA on every screen.** Food's screens built from the kit's accessible components, and tested with screen readers to WCAG 2.2 AA. | interface | small | KIT-A19-F2-P1 |  |
| FOOD-A17-F1-P2 | **Accessibility checks.** Food's screens in the kit's accessibility testing, plus regular testing with disabled customers, couriers and restaurant staff. | service | small | KIT-A19-F2-P2, FOOD-A17-F1-P1 |  |
| FOOD-A17-F1-P3 | **Message-only courier option.** A Deaf or hard-of-hearing courier can ask customers to message instead of calling. | service | small | FOOD-A8-F3-P1 |  |
| FOOD-A17-F1-P4 | **Display settings.** Food's screens follow the kit's text size, contrast and motion settings. | interface | small | KIT-A19-F3-P1, FOOD-A17-F1-P1 |  |
| FOOD-A17-F1-P5 | **Card fields tested too.** The payment provider's card fields and wallet buttons are tested to the same accessibility standard before a provider is chosen, since checkout runs inside them. | research | small | KIT-A3-F1-P1, KIT-A19-F2-P2 | [K02](../kit/STATUS.md) |
| FOOD-A17-F1-P6 | **Offers seen and felt.** Offers and alerts are shown large and signalled by vibration as well as read aloud, so Deaf and hard-of-hearing couriers miss nothing. | interface | small | FOOD-A6-F12-P1, FOOD-A17-F1-P1 |  |
| FOOD-A17-F2-P1 | **Food translation files.** All Food interface text in the kit's translation framework. | library | small | KIT-A19-F1-P1 |  |
| FOOD-A17-F2-P2 | **Launch country languages (example: te reo Māori).** A full version in each launch country's own languages, checked by fluent speakers, starting with te reo Māori for New Zealand. | data | medium | FOOD-A17-F2-P1, KIT-A19-F1-P3 |  |
| FOOD-A17-F2-P3 | **Chat translation.** Food turns on the kit's message translation for chats between couriers, customers and restaurants. | service | small | KIT-A8-F7-P1 |  |
| FOOD-A17-F3-P1 | **Food country settings.** Food's settings for each country (delivery rules, alcohol and food law, and units) on the kit's country packs. | data | medium | KIT-A19-F6-P1 | [K01](../kit/STATUS.md) |
| FOOD-A17-F3-P2 | **Food tax per country.** Food's order lines mapped to the kit's tax rules for each country. | library | small | KIT-A4-F7-P1, FOOD-A10-F4-P2 |  |
| FOOD-A17-F3-P3 | **Platform work law by country.** Courier rules in each country, such as the EU Platform Work Directive and Australia's minimum standards for gig workers. | legal | medium |  | [FD02](STATUS.md) |
| FOOD-A17-F3-P4 | **Allergen lists per country.** Allergen lists for each country, such as the EU's 14, the US's 9 and the ANZ list, plus energy units, so allergen data entered in one country fits the others. | data | small | FOOD-A3-F3-P1, KIT-A19-F6-P1 |  |
| FOOD-A17-F4-P1 | **Locale formatting.** Food's dates, times, money and addresses through the kit's local formats. | library | small | KIT-A19-F1-P2 |  |
| FOOD-A17-F5-P1 | **Languages for couriers and kitchens.** Courier and restaurant apps in the languages their users read most, such as Hindi, Punjabi, Mandarin and Korean, chosen from sign-up figures, through the kit's member translation and machine translation at cost. | data | medium | KIT-A19-F1-P3, KIT-A19-F10-P2, FOOD-A17-F2-P1 |  |
| FOOD-A17-F5-P2 | **Terms and lessons in the courier's language.** Courier terms and safety lessons in the language the courier chose, translated and checked by a person, so no courier agrees to terms they cannot read. | legal | small | FOOD-A18-F6-P3, FOOD-A1-F3-P6, FOOD-A17-F5-P1 | [FD02](STATUS.md) |
| FOOD-A17-F6-P1 | **Right-to-left screens.** Food's screens on the kit's right-to-left layout. | interface | small | KIT-A19-F7-P1, FOOD-A17-F2-P1 |  |
| FOOD-A17-F7-P1 | **Accessible Food documents.** Food's receipts, payout statements and tax invoices as the kit's accessible PDFs. | interface | small | KIT-A19-F11-P1, FOOD-A10-F3-P2, FOOD-A10-F4-P5 |  |
| FOOD-A17-F8-P1 | **Food easy read and sign language.** Easy-read text and videos in each country's sign language (example: NZ Sign Language) for ordering, signing up to deliver and reporting a problem, on the kit's easy read and sign language. | data | small | KIT-A19-F8-P1 |  |

## FOOD-A18 Privacy, data and legal duties

Members control their data and can take it with them, and the app meets every law that applies to a food platform.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A18-F1 Take or delete my data | A person downloads all their data in open formats, or has it deleted. | core | Food data export; Open data formats; Food retention rules |
| FOOD-A18-F2 Know who answers for food safety | A customer, restaurant or courier can see which food safety duties sit with the restaurant, the courier and the platform. | core | Food rules study; Food safety duties page |
| FOOD-A18-F3 Courier status and pay law | Courier terms meet employment law, including each country's test for who is a contractor (example: New Zealand's 2026 gateway test), or couriers are employed. | core | Courier status for Food; Accident cover and insurance |
| FOOD-A18-F4 Privacy law duties | The app meets privacy law (example: New Zealand's Privacy Act 2020): clear notices, access and correction requests, and breach reporting. | core | Food privacy policy; Location data limits; Indirect collection notices; Food privacy impact assessments; Food's outside processors; Police requests for Food data |
| FOOD-A18-F5 Consent for marketing | A person only gets marketing messages they agreed to, as the anti-spam law requires. | core | Consent records |
| FOOD-A18-F6 Terms and policies | Customers, restaurants and couriers agree to clear terms that avoid unfair contract terms. | core | Customer terms; Restaurant terms; Courier terms; Terms versions |
| FOOD-A18-F7 Records the law requires | The co-op keeps tax, payment and anti-money-laundering records for as long as the law requires. | core | Food records |
| FOOD-A18-F8 Limits on who sees customer data | A customer's details are seen only by those who need them, and only while they need them. | core | Restaurant data rules; Details hidden after the job |
| FOOD-A18-F9 Leave with my business | A restaurant or courier leaving the platform takes its menu, records and reputation with it. | next | Restaurant export; Courier reputation export; Leaving rules |
| FOOD-A18-F10 Care with health, faith and disability details | A person's allergies, faith-based diets and access needs are saved only with consent, shared only as much as each order needs, and deleted in one step. | core | Sensitive details rules; Least detail per order |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A18-F1-P1 | **Food data export.** Food's orders, messages, ratings, payouts and settings added to the kit's data export. | service | small | KIT-A18-F1-P1 |  |
| FOOD-A18-F1-P2 | **Open data formats.** Published formats for orders, menus and payouts. | data | small | FOOD-A3-F1-P1, FOOD-A3-F1-P6 |  |
| FOOD-A18-F1-P3 | **Food retention rules.** Food's keeping times for orders, chats, photos and payouts, on the kit's retention schedules and account deletion. | service | small | KIT-A18-F6-P1, KIT-A18-F2-P1, FOOD-A18-F7-P1 |  |
| FOOD-A18-F2-P1 | **Food rules study.** What the restaurant is responsible for and what the platform is, with sources. | research | small |  | [FD01](STATUS.md) |
| FOOD-A18-F2-P2 | **Food safety duties page.** A plain page, linked from the terms and the help centre, showing who answers for what under food safety law (example: New Zealand's Food Act). | interface | small | FOOD-A18-F2-P1, FOOD-A11-F1-P1 |  |
| FOOD-A18-F3-P1 | **Courier status for Food.** Food's facts for the kit's worker status test, such as who sets pay, who controls hours and whether couriers work for others, and the result set in the kit's worker status per app, decided with a lawyer. | legal | medium | KIT-A23-F1-P1, KIT-A23-F1-P2 | [FD02](STATUS.md) |
| FOOD-A18-F3-P2 | **Accident cover and insurance.** Public accident cover and its levies (example: New Zealand's ACC), and insurance options for couriers. | research | small |  | [FD02](STATUS.md) |
| FOOD-A18-F4-P1 | **Food privacy policy.** Food's own sections of the privacy policy and request flow, on the kit's privacy page, terms and breach response. | legal | small | KIT-A18-F3-P1, KIT-A18-F4-P2, KIT-A18-F7-P1 |  |
| FOOD-A18-F4-P3 | **Location data limits.** Food's location rules on the kit's live location: shared only during a job and not kept afterwards, unless the law, safety or a dispute needs a trace. | service | small | KIT-A7-F4-P2, FOOD-A8-F2-P1 |  |
| FOOD-A18-F4-P4 | **Indirect collection notices.** Notices for every indirect collection in Food (example: New Zealand's privacy principle 3A): licence, criminal record and identity results from providers, customer details supplied by restaurants, and proof photos that show other people. | legal | small | KIT-A18-F5-P1 |  |
| FOOD-A18-F4-P5 | **Food privacy impact assessments.** Privacy impact assessments, on the kit's template and register, for live location, selfie checks, allergy and access data, and proof photos, before each is built. | legal | small | KIT-A18-F10-P1, FOOD-A18-F10-P1 |  |
| FOOD-A18-F4-P6 | **Food's outside processors.** Food's outside processors, such as masked calls, identity checks, texts and the payment provider, on the kit's processor list, with the overseas checks for each. | legal | small | KIT-A18-F15-P2, KIT-A18-F15-P1 |  |
| FOOD-A18-F4-P7 | **Police requests for Food data.** Requests from police and courts for courier traces, addresses and order records, handled through the kit's law enforcement request process and counted in public. | service | small | KIT-A10-F9-P3, FOOD-A8-F2-P1 |  |
| FOOD-A18-F5-P1 | **Consent records.** Food's marketing consents on the kit's consent records. | data | small | KIT-A8-F5-P2 |  |
| FOOD-A18-F6-P1 | **Customer terms.** Plain-English terms of use. | legal | small |  | [FD01](STATUS.md) |
| FOOD-A18-F6-P2 | **Restaurant terms.** Terms for restaurants checked against the unfair contract terms rules. | legal | medium | KIT-A18-F4-P2 | [FD01](STATUS.md) |
| FOOD-A18-F6-P3 | **Courier terms.** Food's courier terms on the kit's contractor agreements, or an employment agreement if couriers are employees, matching the status decision. | legal | medium | KIT-A23-F1-P3, FOOD-A18-F3-P1 | [FD02](STATUS.md) |
| FOOD-A18-F6-P4 | **Terms versions.** Food's terms versions on the kit's terms and policy versions. | data | small | KIT-A18-F4-P1 |  |
| FOOD-A18-F7-P1 | **Food records.** Food's records and how long each is kept, on the kit's financial record keeping. | legal | small | KIT-A4-F6-P1 | [K02](../kit/STATUS.md) |
| FOOD-A18-F8-P1 | **Restaurant data rules.** What a restaurant sees and may keep (a name, a masked phone, and an address only for its own-courier orders), with each restaurant treated as its own privacy agency. | legal | small | FOOD-A18-F4-P1 |  |
| FOOD-A18-F8-P2 | **Details hidden after the job.** Hides the customer's name and address from the courier once the job ends, and limits restaurant views by staff role. | library | small | KIT-A15-F1-P4, FOOD-A5-F1-P3, FOOD-A18-F8-P1 |  |
| FOOD-A18-F9-P1 | **Restaurant export.** The menu in an open format, plus orders, reviews and the followers who agreed to it. | service | medium | FOOD-A18-F1-P2, KIT-A18-F1-P1 |  |
| FOOD-A18-F9-P2 | **Courier reputation export.** A signed copy of a courier's ratings, from the kit's portable reputation. | service | small | KIT-A9-F7-P1 |  |
| FOOD-A18-F9-P3 | **Leaving rules.** A notice period, a final payout, and what happens to direct-ordering links and QR codes. | library | small | FOOD-A13-F4-P2, KIT-A3-F3-P2, FOOD-A10-F10-P2 |  |
| FOOD-A18-F10-P1 | **Sensitive details rules.** Rules for health, religious and disability details: consent before saving, the least detail each party needs, no use in suggestions unless switched on, one-step deletion, and treatment as special category data where the GDPR applies. The kit should hold one such rule for every app. | legal | small | KIT-A18-F5-P1, KIT-A18-F6-P1 |  |
| FOOD-A18-F10-P2 | **Least detail per order.** Maps each saved need to the instruction each party sees on an order, such as 'bring it to the door' for the courier and the allergens for the kitchen, and nothing more. | library | small | FOOD-A18-F10-P1, KIT-A15-F1-P4 |  |

## FOOD-A19 Operations and admin

The tools the co-op's small team and member volunteers use to run each city safely and at cost, built on the kit's admin console, monitoring and status page.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A19-F1 Operations console | An operator sees live orders, couriers online and problems across a city. | core | Live operations view; Operator actions |
| FOOD-A19-F2 Review queues | Staff and volunteers work through applications, reports, menu checks and claims in one place. | core | Food review queue types |
| FOOD-A19-F3 Admin roles and audit | Every admin action is limited to its role and logged for members to inspect. | core | Admin roles; Admin audit log |
| FOOD-A19-F4 Monitoring and incidents | The team knows when something breaks and fixes it fast, and uptime is published. | core | Food monitoring; Incident playbooks; Status page; City-by-city releases; No updates mid-service |
| FOOD-A19-F5 City simulator | The team runs a whole city of simulated orders to test dispatch before launch. | next | City simulation |
| FOOD-A19-F6 Self-hosting | A member group runs its own Spare Food servers for its area. | later | Self-host package |
| FOOD-A19-F7 Security checked before each city | Each city opens only after outside testers have checked Food's own attack list. | core | Food threat model; Outside test before each launch |
| FOOD-A19-F8 Severe weather and emergencies | Operators see weather and emergency management warnings (example: New Zealand's Civil Defence), and can switch a city into an emergency mode. | next | Weather and emergency warnings; Emergency mode |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A19-F1-P1 | **Live operations view.** Food's live map and list of active orders, courier supply and late orders, in the kit's admin console. | interface | medium | KIT-A22-F6-P1, KIT-A7-F1-P2 |  |
| FOOD-A19-F1-P2 | **Operator actions.** Pause a zone, reassign a job or contact a party, each logged as an operation. | service | medium | kit-ops, FOOD-A7-F4-P1, KIT-A22-F6-P1 |  |
| FOOD-A19-F2-P1 | **Food review queue types.** Food's queues (applications, menu checks and claims) in the kit's moderation queue and admin console. | interface | small | KIT-A10-F6-P2, KIT-A22-F6-P1 |  |
| FOOD-A19-F3-P1 | **Admin roles.** Food's staff and volunteer roles, least access by default, on the kit's permission checks and access controls. | service | small | KIT-A15-F1-P4, KIT-A22-F5-P1 |  |
| FOOD-A19-F3-P2 | **Admin audit log.** Every Food admin action in the operation log through the kit's admin console, open to member auditors. | service | small | kit-ops, KIT-A22-F6-P1 |  |
| FOOD-A19-F4-P1 | **Food monitoring.** Food's checks for payment failures and dispatch delays on the kit's monitoring and alerts. | service | small | KIT-A22-F1-P1 |  |
| FOOD-A19-F4-P2 | **Incident playbooks.** Food's playbooks for payments, maps or dispatch failing in the middle of dinner, in the kit's incident response. | research | small | KIT-A22-F1-P2 |  |
| FOOD-A19-F4-P3 | **Status page.** Food on the kit's status page, with public uptime and incidents. | interface | small | KIT-A21-F5-P1 |  |
| FOOD-A19-F4-P4 | **City-by-city releases.** New versions reach one city at a time, using the kit's feature switches. | service | small | KIT-A22-F2-P2 |  |
| FOOD-A19-F4-P5 | **No updates mid-service.** Restaurant tablets and courier apps never reload for an update while orders are open or a job is under way; updates wait for a quiet moment, on the kit's update delivery. | library | small | KIT-A20-F7-P1, FOOD-A19-F4-P4 |  |
| FOOD-A19-F5-P1 | **City simulation.** Food's simulated restaurants, kitchens and couriers, run on a shared city simulator to test dispatch rules and load. The simulator itself should move down into the kit, since Spare Ride maps the same. | library | medium | FOOD-A7-F1-P3, KIT-A16-F3-P3, KIT-A7-F3-P2 | [FD03](STATUS.md) |
| FOOD-A19-F6-P1 | **Self-host package.** Food's settings and guides on the kit's self-host package. | service | small | KIT-A22-F7-P1 |  |
| FOOD-A19-F7-P1 | **Food threat model.** Food's attack list: guessing hand-over codes, promo and referral abuse, fake restaurants, rented courier accounts, faked GPS, SIM swaps and recycled numbers, a stolen or shared kitchen tablet, courier and customer collusion, scripted refund claims, scraping of menus and addresses, and stalking through location data. | research | medium | FOOD-A5-F3-P2, FOOD-A8-F2-P1, KIT-A7-F4-P3, KIT-A16-F1-P4, FOOD-A1-F1-P5, FOOD-A16-F2-P2 |  |
| FOOD-A19-F7-P2 | **Outside test before each launch.** An outside security test against the threat model before each city opens. | service | small | KIT-A22-F5-P3, FOOD-A19-F7-P1 |  |
| FOOD-A19-F8-P1 | **Weather and emergency warnings.** Weather warnings and emergency management alerts shown to operators, with suggested zone pauses, from a shared feed the kit should own, since Spare Ride, Spare Stay and Spare Work need it too. Each country has its own sources; for New Zealand, Civil Defence and MetService, whose data terms are *unverified*. | integration | small | FOOD-A19-F1-P1 |  |
| FOOD-A19-F8-P2 | **Emergency mode.** Pauses or shrinks zones, tells couriers and customers why, and ends on its own when the warning lifts. | service | small | FOOD-A19-F8-P1, FOOD-A15-F9-P2 |  |

## FOOD-A20 Headless: AI, plugins and open interface

Everything a person can do, an AI agent or a script can do through the same commands, and restaurants can add sandboxed plugins.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| FOOD-A20-F1 Command interface | Every action in every Spare Food app is a command a script can call. | core | Food command frame; Interface reference and keys; Command coverage check; Menu and restaurant commands; Order commands; Courier job commands; Money commands; Support, dispute and review commands; Member commands; Operator and admin commands |
| FOOD-A20-F2 Order through an AI assistant | A customer asks an AI assistant to order dinner, and confirms before any money moves. | next | Spare Food MCP server; Payment confirmation guard; Allergies and swaps stay with the person |
| FOOD-A20-F3 AI help for restaurants | A restaurant asks an assistant to update the menu, read sales or pause for an hour. | next | Restaurant MCP tools |
| FOOD-A20-F4 Plugins | A restaurant or member adds sandboxed plugins, such as a point-of-sale link or a loyalty card. | next | Food plugin hooks |
| FOOD-A20-F5 Order event notices | A restaurant's own systems are told about new and changed orders as they happen. | next | Order webhooks |
| FOOD-A20-F6 Fair rules for scripts and agents | Scripts and AI agents can help couriers and customers, but never grab jobs ahead of people or spend money without a person's approval. | core | Courier command limits; Commands that need approval; People-only Food actions; Agent labels for restaurants and couriers; Agent limits, sign-in and activity; Scraping limits |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| FOOD-A20-F1-P1 | **Food command frame.** The shared shape of Food's commands and queries on the kit's command schemas: naming, versions, errors and permissions, split by area into the command sets below. | library | medium | KIT-A15-F1-P3, kit-ops, kit-market |  |
| FOOD-A20-F1-P2 | **Interface reference and keys.** Food's published command reference, with access keys per restaurant on the kit's API keys. | interface | small | KIT-A16-F1-P2, KIT-A16-F3-P2, FOOD-A20-F1-P1 |  |
| FOOD-A20-F1-P3 | **Command coverage check.** A check on every change that each action on a Food screen has a matching command. | service | small | KIT-A15-F1-P3, FOOD-A20-F1-P1 |  |
| FOOD-A20-F1-P4 | **Menu and restaurant commands.** Commands and queries for menus, stock, hours, pauses and restaurant staff. | library | medium | FOOD-A20-F1-P1, FOOD-A3-F1-P1, FOOD-A5-F4-P1 |  |
| FOOD-A20-F1-P5 | **Order commands.** Commands and queries for carts, checkout, order steps, changes and cancels. | library | medium | FOOD-A20-F1-P1, FOOD-A4-F1-P1, FOOD-A5-F1-P3 |  |
| FOOD-A20-F1-P6 | **Courier job commands.** Commands and queries for going online, offers, pickup, drop-off and earnings. | library | medium | FOOD-A20-F1-P1, FOOD-A6-F1-P1, FOOD-A6-F4-P2 |  |
| FOOD-A20-F1-P7 | **Money commands.** Commands and queries for payouts, refunds, tips and statements. | library | small | FOOD-A20-F1-P1, FOOD-A10-F2-P2 |  |
| FOOD-A20-F1-P8 | **Support, dispute and review commands.** Commands and queries for problem reports, support cases, disputes, ratings and reviews. | library | small | FOOD-A20-F1-P1, FOOD-A11-F2-P1, FOOD-A12-F1-P1 |  |
| FOOD-A20-F1-P9 | **Member commands.** Commands and queries for Food proposals, votes and the ledger. | library | small | FOOD-A20-F1-P1, FOOD-A15-F1-P1 |  |
| FOOD-A20-F1-P10 | **Operator and admin commands.** Commands and queries for zones, review queues, operator actions and admin roles. | library | small | FOOD-A20-F1-P1, FOOD-A19-F1-P2, FOOD-A19-F3-P1 |  |
| FOOD-A20-F2-P1 | **Spare Food MCP server.** Food's search, order and track tools on the kit's MCP server. | service | small | KIT-A17-F1-P1, FOOD-A20-F1-P1 |  |
| FOOD-A20-F2-P2 | **Payment confirmation guard.** Any order an AI places waits for the person's approval before payment, using Food's approval list on the kit's approval steps. | service | small | KIT-A17-F2-P1, FOOD-A20-F2-P1, FOOD-A20-F6-P2 |  |
| FOOD-A20-F2-P3 | **Allergies and swaps stay with the person.** An order an agent places always carries the person's saved allergy request, and a kitchen's allergy decline, a swap offer or an allergy question goes back to the person, never answered by the agent. | library | small | FOOD-A4-F12-P1, FOOD-A5-F5-P1, FOOD-A20-F6-P3 |  |
| FOOD-A20-F3-P1 | **Restaurant MCP tools.** Menu, hours and report tools for staff's AI assistants, limited to each person's role. | service | small | KIT-A17-F1-P1, FOOD-A1-F4-P1 |  |
| FOOD-A20-F4-P1 | **Food plugin hooks.** Hooks for order events, menu sync and pricing, with declared permissions. | library | medium | KIT-A16-F5-P2, FOOD-A20-F1-P1 | [FD05](STATUS.md) |
| FOOD-A20-F5-P1 | **Order webhooks.** Food's order events sent through the kit's signed webhooks. | service | small | KIT-A16-F2-P1, FOOD-A20-F1-P1 |  |
| FOOD-A20-F6-P1 | **Courier command limits.** Food's rules on the kit's automated acceptance rules: courier scripts may go online and export earnings, but never accept offers automatically, so bots never beat people to jobs. | library | small | FOOD-A20-F1-P1, FOOD-A7-F5-P1, KIT-A16-F1-P3, KIT-A6-F4-P3 |  |
| FOOD-A20-F6-P2 | **Commands that need approval.** The Food commands that always need a person's approval: placing an order, tipping, a late cancel, accepting a swap, changing the address after pickup, a refund claim, a gift order, price changes and payout changes. | library | small | KIT-A17-F2-P1, FOOD-A20-F1-P1 |  |
| FOOD-A20-F6-P3 | **People-only Food actions.** Ratings, reviews, votes, dispute statements and alcohol age declarations come only from the person, never an agent, on the kit's people-only actions. | library | small | KIT-A17-F9-P2, FOOD-A20-F1-P1 |  |
| FOOD-A20-F6-P4 | **Agent labels for restaurants and couriers.** Orders, messages and changes an agent makes carry the kit's agent label, so the restaurant and the courier see they came from an agent acting for a named person. | library | small | KIT-A17-F9-P1, FOOD-A20-F1-P1 |  |
| FOOD-A20-F6-P5 | **Agent limits, sign-in and activity.** Food's spending limits for each agent on the kit's agent limits, agent sign-in under the MCP authorisation rules, and a list of every agent action in Food, with undo where it can be undone. | service | small | KIT-A17-F4-P1, KIT-A17-F1-P2, KIT-A17-F1-P3, KIT-A17-F3-P1 |  |
| FOOD-A20-F6-P6 | **Scraping limits.** No bulk copying of menus, addresses, reviews or courier details through Food's command API or MCP tools, on the kit's bulk copy limits. | library | small | KIT-A16-F1-P4, FOOD-A20-F1-P1 |  |

## Work-board pieces this map uses

| Piece | Parts that use it |
| --- | --- |
| [FD01](STATUS.md) | 17 |
| [FD02](STATUS.md) | 19 |
| [FD03](STATUS.md) | 7 |
| [FD04](STATUS.md) | 1 |
| [FD05](STATUS.md) | 8 |
| [K01](../kit/STATUS.md) | 8 |
| [K02](../kit/STATUS.md) | 7 |
| [K04](../kit/STATUS.md) | 2 |
| [K05](../kit/STATUS.md) | 1 |
| [K06](../kit/STATUS.md) | 2 |

## Left out on purpose

Features the big apps have that Spare Food leaves out, and why. Members can vote to change any of these.

| Left out | Why |
| --- | --- |
| Paid placement and ads | Rankings follow published rules and are never sold. |
| Paid subscriptions | Member benefits in FOOD-A9-F8 lower fees instead, if members vote for them. |
| A paid priority delivery tier | Dispatch treats every order by the same published rules. |
| Co-op-run dark stores | The co-op runs no kitchens or shops of its own and serves independent sellers. |
| Drones and robots | Not proven at scale, and tightly limited by aviation and road rules. |
| Table reservations | A booking product, not delivery, which can come later as its own feature or app. |
| Package returns | Returns belong with a shared delivery job type in the kit and with Spare Market's shipping. |
| Restaurant cash advances and loans | Lending brings in credit law and is not an at-cost running cost. |

## Questions for assessing this map and its demo

1. Could a customer, a restaurant and a courier each finish a whole order, from sign-up to payout, using only the core features?
2. Does every feature a regular Uber Eats, DoorDash or local delivery app user relies on each week have a home in this map, and does each reason in the left-out list (the last question) still hold?
3. Is the courier status decision (FD02) settled before courier terms, pay, ratings and removal are built, since it changes all of them, and does every courier pay vote pass the competition law check (KIT-A12-F4-P3) first?
4. Does the price breakdown make it obvious what the restaurant, the courier, the running cost and sales tax each get, so a customer trusts it more than the big apps?
5. Is the at-cost fee believable in the first months, when the running cost is shared over few orders?
6. Does ready-timed dispatch (FD03) have a proven, published method to port, or is it being invented?
7. Will restaurants accept a separate tablet at launch, or must point-of-sale and printer links (FD05) come first?
8. Which parts are shared with Spare Ride (courier accounts, location, routing, safety) and are they moved down into the kit rather than copied?
9. Is each legal duty in the first country pack (for New Zealand: Food Act, GST on listed services, income reporting, anti-money-laundering, Privacy Act, alcohol) covered by a piece and checked by a lawyer before launch?
10. Are groceries, alcohol and catering rightly marked later, given the extra law and the picker app they need?
11. Can every screen in the demo be used with a keyboard and a screen reader, and in each launch country's languages (example: te reo Māori)?
12. Does anything in the demo look like an incumbent's interface, name, colours or icons?
13. What does the demo show when the courier's or the restaurant's signal drops mid-order?
14. How does the co-op sign up enough restaurants and couriers in one city at the same time, and what count is enough to open?
15. Which large parts should be split further before the Design stage, and is any size clearly wrong?
16. Are the decisions on what the at-cost fee may cover (FOOD-A9-F2-P3) and who pays it (FOOD-A9-F2-P4) made before the price calculator, the money split and the ledger page are built?
17. Does the demo show what happens when an order goes wrong: a declined or unanswered order, a silent restaurant tablet, an allergy request the kitchen declines, and a report of sickness or an allergic reaction?
18. Food's core features rest on kit parts the kit marks as next or later: organisation model (KIT-A1-F7-P1, next); team management (KIT-A1-F7-P2, next); vehicle checks (KIT-A2-F6-P2, next); business number lookup (KIT-A2-F7-P1, next); trader details (KIT-A2-F7-P2, next); background checks (KIT-A2-F9-P1, next); background check link (KIT-A2-F9-P2, next); face check fairness testing (KIT-A2-F12-P1, next); repeat face checks (KIT-A2-F12-P2, next); sanctions screening (KIT-A4-F8-P1, later); scheduled and repeat bookings (KIT-A5-F9-P1, next); multi-party orders (KIT-A5-F11-P1, next); favourites (KIT-A5-F12-P1, next); order changes (KIT-A5-F15-P1, next); fairness checks (KIT-A6-F5-P2, next); turn-by-turn guidance (KIT-A7-F6-P1, next); message translation (KIT-A8-F7-P1, next); fraud signals (KIT-A10-F12-P2, next); insurance claims link (KIT-A11-F9-P2, later); privacy impact assessments (KIT-A18-F10-P1, next); update delivery (KIT-A20-F7-P1, next). Will the kit bring each one forward, or should the Food part that needs it wait?
19. Which Food parts still have no work-board piece, such as the first city and launch plan, start-up funding, insurance and restricted goods, and should new pieces be opened for them?
20. Food asks the kit to open shared parts it does not have yet: a shared worker record of hours across apps (FOOD-A6-F1-P4); a shared device mode for kitchen and shop tablets (FOOD-A16-F2-P2); an A-to-B delivery job type (FOOD-A14-F5-P1, FOOD-A14-F6-P1); a guest order decision (FOOD-A1-F1-P6); union access and bargaining duties (FOOD-A6-F13-P7); a sales tax threshold warning (FOOD-A10-F13-P1); rules for health, faith and disability details (FOOD-A18-F10-P1); access needs on the profile (FOOD-A4-F13-P1); a shop page and a till (FOOD-A13-F4-P1, FOOD-A13-F5-P1); a mileage and expense log (FOOD-A6-F6-P4); text and phone ordering (FOOD-A4-F14-P1, FOOD-A4-F14-P2); a driving-safe mode and the phone road rules (FOOD-A6-F12-P1, FOOD-A6-F12-P3); tax withholding from payouts (FOOD-A10-F11-P1, FOOD-A10-F11-P2); a city simulator (FOOD-A19-F5-P1); emergency powers rules (FOOD-A15-F9-P1); a weather and emergency warnings feed (FOOD-A19-F8-P1); launch counting, pledges and waitlists (FOOD-A15-F6-P1, FOOD-A15-F6-P4, FOOD-A15-F6-P6). Has the kit opened each one, so the Food part becomes a thin use of it?
21. Left out on purpose: paid placement and ads (rankings follow published rules and are never sold); paid subscriptions (member benefits in FOOD-A9-F8 lower fees instead, if members vote for them); a paid priority delivery tier (dispatch treats every order by the same published rules); co-op-run dark stores (the co-op runs no kitchens or shops of its own and serves independent sellers); drones and robots (not proven at scale, and tightly limited by aviation and road rules); table reservations (a booking product, not delivery, which can come later as its own feature or app); package returns (returns belong with a shared delivery job type in the kit and with Spare Market's shipping); restaurant cash advances and loans (lending brings in credit law and is not an at-cost running cost). Does each reason still hold?
