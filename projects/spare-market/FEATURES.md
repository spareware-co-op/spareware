# Spare Market: everything to build

*Stage: Mapping. Mapped top-down on 2026-10-18, then checked on 2026-10-19 against an independent review and the kit's own map. Every choice is open until an owner accepts a decision note.*

This is the full map of what Spare Market needs, worked top-down: the areas a complete replacement for Trade Me, eBay, Etsy, Facebook Marketplace and Shopify must cover, the features a person uses in each, and the parts to build for each feature. It follows every journey: joining, listing, searching, buying, paying, shipping, pickup, returns, disputes, running a shop and its counter, moderation, member votes, many languages and countries, access for everyone, offline use, data export and the law. The Market runs on the shared kit, so where a part rests on the kit it names the kit part it uses by ID and holds only the Market's own rules: the Market uses 214 of the kit's 572 parts, listed in the [kit's feature map](../kit/FEATURES.md). Where the Market needs a shared part the kit does not have yet, the part says the kit should own it, and the questions at the end list them all. New Zealand comes first; every part is built so other countries can be added as rule sets, not rewrites. Vehicles are mapped here; property, rentals, jobs and trade services are mapped in Spare Home, Spare Rent and Spare Work. Everything is our own design: other companies are named only to say what we replace. IDs are never renumbered or reused; the parts this review handed to the kit are listed under [Retired IDs](#retired-ids).

**In numbers:** 22 areas, 177 features (74 core, 56 next, 47 later) and 388 parts. The same map is in [features.json](features.json) for tools and the demo.

**How to read it.** Core means needed for a first usable release; next means soon after; later means once the rest works. Each part has a kind (library, service, interface, data, integration, research or legal) and a size (small, medium or large). Depends on names a kit crate (kit-...), a shared engine (engine-...), a kit part by ID (KIT-...) or another Market part by ID (MKT-...). Piece links to the work board in [STATUS.md](STATUS.md) or the [kit's work board](../kit/STATUS.md).

## Areas

| ID | Area | Features | Parts |
| --- | --- | --- | --- |
| MKT-A1 | [Accounts and seller set-up](#mkt-a1-accounts-and-seller-set-up) | 8 | 21 |
| MKT-A2 | [Listing items](#mkt-a2-listing-items) | 14 | 39 |
| MKT-A3 | [Categories and item details](#mkt-a3-categories-and-item-details) | 6 | 17 |
| MKT-A4 | [Search and discovery](#mkt-a4-search-and-discovery) | 11 | 24 |
| MKT-A5 | [Buying, offers and checkout](#mkt-a5-buying-offers-and-checkout) | 11 | 25 |
| MKT-A6 | [Auctions](#mkt-a6-auctions) | 7 | 16 |
| MKT-A7 | [Payments, held money and fees](#mkt-a7-payments-held-money-and-fees) | 10 | 18 |
| MKT-A8 | [Shipping, pickup and delivery](#mkt-a8-shipping-pickup-and-delivery) | 10 | 28 |
| MKT-A9 | [Orders, returns and after-sale](#mkt-a9-orders-returns-and-after-sale) | 7 | 16 |
| MKT-A10 | [Messages and questions](#mkt-a10-messages-and-questions) | 5 | 8 |
| MKT-A11 | [Ratings, buyer protection and disputes](#mkt-a11-ratings-buyer-protection-and-disputes) | 6 | 12 |
| MKT-A12 | [Safety, scams and banned items](#mkt-a12-safety-scams-and-banned-items) | 8 | 15 |
| MKT-A13 | [Shop websites](#mkt-a13-shop-websites) | 13 | 37 |
| MKT-A14 | [Shop selling tools](#mkt-a14-shop-selling-tools) | 12 | 29 |
| MKT-A15 | [Till for physical shops](#mkt-a15-till-for-physical-shops) | 6 | 11 |
| MKT-A16 | [Seller tools, insights and links](#mkt-a16-seller-tools-insights-and-links) | 10 | 18 |
| MKT-A17 | [Moderation, support and administration](#mkt-a17-moderation-support-and-administration) | 5 | 6 |
| MKT-A18 | [Members, fees and the cost ledger](#mkt-a18-members-fees-and-the-cost-ledger) | 6 | 9 |
| MKT-A19 | [Countries, languages, currencies and tax](#mkt-a19-countries-languages-currencies-and-tax) | 6 | 10 |
| MKT-A20 | [Access, phones and offline](#mkt-a20-access-phones-and-offline) | 4 | 5 |
| MKT-A21 | [Data, privacy and legal duties](#mkt-a21-data-privacy-and-legal-duties) | 6 | 8 |
| MKT-A22 | [Vehicles and other big items](#mkt-a22-vehicles-and-other-big-items) | 6 | 16 |

## MKT-A1 Accounts and seller set-up

Joining with one Spareware login, a public trading profile, the checks a seller needs before selling, business accounts with staff, and leaving cleanly.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A1-F1 Sign up and sign in | Join with one Spareware login, using a passkey or a password, and start browsing or selling straight away. | core | Market sign-up flow; Market member record; Second step for sellers; Market age rules |
| MKT-A1-F2 Trading profile | Show a public profile with a name, area, photo, join date, ratings, listings and how quickly they reply. | core | Profile page; Profile privacy settings |
| MKT-A1-F3 Verify who you are | Earn verified badges by confirming a phone number, an address and identity through a regulated check. | core | Contact checks before trading; When the Market asks for an identity check; Address check; Where badges show |
| MKT-A1-F4 Say you sell in trade | Declare that you sell as a business, so buyers see it on every listing and the right consumer law applies. | core | In-trade declaration; In-trade prompts |
| MKT-A1-F5 Set up payouts | Connect a bank account for payouts through the payment provider's own sign-up, without the Market seeing card or bank secrets. | core | Payout set-up from the Market; Market seller checks |
| MKT-A1-F6 Business accounts and staff | Add staff to a business account with their own logins and only the Market permissions they need. | next | Market business roles; Staff action log |
| MKT-A1-F7 Block a member or set buyer rules | Block a member, or set rules on who may buy, so unwanted buyers cannot buy, make offers, bid or message you. | core | Block enforcement; Buyer rules |
| MKT-A1-F8 Pause or close an account | Pause selling for a holiday, or close the account once orders, held money, disputes and anything owed to customers are settled. | core | Holiday mode; Account closure flow; Estate handover |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A1-F1-P1 | **Market sign-up flow**: The Market's steps on the kit's sign-up flow: pick a region and accept the Market's plain-language terms. | interface | small | KIT-A1-F1-P2, KIT-A18-F4-P1 |  |
| MKT-A1-F1-P2 | **Market member record**: The Market's own settings for a member: region, buying and selling status, preferences and limits. | data | small | kit-core, KIT-A1-F5-P1, KIT-A1-F5-P2 |  |
| MKT-A1-F1-P3 | **Second step for sellers**: Requires the kit's second sign-in step before a member can receive payouts or run a shop, because trading accounts are a target for takeover. | library | small | KIT-A1-F3-P1, KIT-A1-F3-P2, KIT-A1-F3-P3 |  |
| MKT-A1-F1-P4 | **Market age rules**: The minimum age to buy and to sell on the Market, and which items only adults may buy or sell, set on the kit's minimum age and its rules for young people. | legal | small | KIT-A1-F1-P5, KIT-A1-F8-P2 |  |
| MKT-A1-F2-P1 | **Profile page**: The public page for a member: name, suburb or town, photo, join date, badges, ratings and current listings. | interface | small | MKT-A1-F1-P2, MKT-A11-F1-P1 |  |
| MKT-A1-F2-P2 | **Profile privacy settings**: Choose what others see, such as town only and never the street address, and whether past sales are shown. | interface | small | MKT-A1-F1-P2, KIT-A7-F10-P1 |  |
| MKT-A1-F3-P1 | **Contact checks before trading**: Requires a phone number and email confirmed with the kit's checks before a member's first sale or purchase. | library | small | KIT-A1-F1-P3 |  |
| MKT-A1-F3-P2 | **When the Market asks for an identity check**: Sends members to the kit's identity check before payouts above a set amount, before selling vehicles or high-value items, and for the verified badge. | library | small | KIT-A2-F1-P2, KIT-A2-F1-P3 |  |
| MKT-A1-F3-P3 | **Address check**: Confirms an address by a code sent by post or a trusted record, so buyers know a seller is real; hosts, home sellers and landlords need the same check, so it should move down into the kit as an identity check. | integration | small | KIT-A2-F1-P3, KIT-A7-F2-P3 |  |
| MKT-A1-F3-P4 | **Where badges show**: Shows the kit's verified badges on Market profiles, listings, offers and chat headers. | interface | small | KIT-A2-F5-P1, MKT-A1-F2-P1 |  |
| MKT-A1-F4-P1 | **In-trade declaration**: Requires in-trade sellers to say so, and shows the kit's trader details on every listing, as the Fair Trading Act requires. | data | small | KIT-A2-F7-P2, KIT-A2-F7-P1, MKT-A1-F1-P2, KIT-A18-F8-P1 |  |
| MKT-A1-F4-P3 | **In-trade prompts**: Spot members who sell like a business, such as many new identical items, and ask them to declare. | service | medium | MKT-A1-F4-P1, MKT-A4-F1-P2 |  |
| MKT-A1-F5-P1 | **Payout set-up from the Market**: Starts the kit's payout set-up at a seller's first sale, so they can list before it is done but cannot be paid until it is. | integration | small | KIT-A3-F3-P1 |  |
| MKT-A1-F5-P2 | **Market seller checks**: Which Market sellers the kit's customer checks cover and when, such as before high payouts or many high-value sales, under the co-op's one anti-money-laundering programme. | legal | small | KIT-A4-F5-P1, KIT-A4-F5-P2, KIT-A4-F5-P5, MKT-A7-F6-P1 |  |
| MKT-A1-F6-P1 | **Market business roles**: The Market's roles on the kit's organisation model: owner, manager, packer and till operator, and what each may do with listings, orders and money. | data | small | KIT-A1-F7-P1, KIT-A1-F5-P2 |  |
| MKT-A1-F6-P3 | **Staff action log**: Every staff action recorded as an operation, so the owner can see who changed what. | service | small | kit-ops, MKT-A1-F6-P1 |  |
| MKT-A1-F7-P2 | **Block enforcement**: Checks the kit's block list and the seller's buyer rules at offer, bid, purchase and message time. | service | small | kit-market, KIT-A10-F4-P1 |  |
| MKT-A1-F7-P3 | **Buyer rules**: Seller-set rules on who may buy: verified buyers only, buyers in areas the seller ships to, buyers with no unpaid or no-show record, and a limit on items per buyer. | library | small | MKT-A1-F7-P2, MKT-A1-F3-P4, MKT-A8-F1-P1, MKT-A8-F2-P4 |  |
| MKT-A1-F8-P1 | **Holiday mode**: Hide listings or mark them as away, with an away message and a return date. | service | small | MKT-A2-F1-P1 |  |
| MKT-A1-F8-P2 | **Account closure flow**: Before closing, checks open orders, held money, disputes and anything a shop still owes customers (gift card balances, store credit, pre-orders, subscriptions and layby), and offers a full data export first. | service | medium | MKT-A21-F1-P1, MKT-A7-F1-P2, KIT-A3-F16-P2, KIT-A18-F2-P2 |  |
| MKT-A1-F8-P3 | **Estate handover**: The Market's steps on the kit's after-death handling: listings ended, open orders finished and held money paid to the estate. | service | small | KIT-A18-F11-P1, MKT-A7-F1-P2, KIT-A3-F3-P2 |  |

## MKT-A2 Listing items

Putting an item up for sale in a few minutes from a phone, with good photos, an honest description, the right price type, and help from the Market to get it right.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A2-F1 Create a listing | List an item with a title, photos, description, condition, price and pickup or shipping in a few minutes. | core | Listing record; Listing commands; Listing form; Listing page; Listing pages found by web search |
| MKT-A2-F2 Add photos and video | Take photos straight from the phone camera, crop and reorder them, and add a short video. | core | Market photo rules; Photo editor; Short video; Photo limits and clean-up |
| MKT-A2-F3 Choose how to price | Sell at a fixed price, accept offers above a minimum, give it away free, or ask for a swap. | core | Price types; Auto-accept and auto-decline |
| MKT-A2-F4 Edit, end, relist and expire | Change a listing, end it early, mark it sold elsewhere, and relist it when it expires. | core | Listing lifecycle rules; Change history for buyers; On hold for a buyer |
| MKT-A2-F5 Check a listing before it goes live | Get told before posting if an item is banned, restricted, or needs extra details by law. | core | Market listing checks; Product safety fields |
| MKT-A2-F6 Help me write it | Get a suggested title, category and details from the photos, and a price guide from similar sold items. | next | Suggestions from photos; Price guide; AI help labels |
| MKT-A2-F7 Sell several of one item, or in options | List one item in several sizes or colours, or many of the same, with stock counted down as each sells. | next | Variants and quantity; Option picker |
| MKT-A2-F8 Drafts, templates and scheduling | Save a draft, start from an old listing or a template, and choose when a listing starts. | next | Drafts that work offline; Templates and sell similar; Scheduled start |
| MKT-A2-F9 Say how it was made | Mark an item as handmade, designed by the seller, vintage, made with a production partner, or made with AI, so buyers know. | next | Making declaration; Making checks |
| MKT-A2-F10 Made to order, personalised and digital items | Sell items made after the order, ask the buyer for personal details such as a name to engrave, or sell a digital download. | next | Personalisation fields; Making time; Digital file delivery; Digital goods rules |
| MKT-A2-F11 Post a wanted ad | Say what you are looking for and let sellers who have it reply. | next | Wanted ad record; Wanted ad alerts; Wanted ads search |
| MKT-A2-F12 Share a listing | Share any listing by link with a photo card, to a community group, a chat or any social site. | core | Share cards |
| MKT-A2-F13 Swap or trade | Offer an item for a swap, or a swap plus money, and agree the trade in the chat. | next | Swap offers; Swap handover |
| MKT-A2-F14 Hire out an item | Hire out tools, cameras, party gear and other items for a day or a week, with a deposit held until they come back. | later | Hire listings; Hire deposits and damage; Hire terms |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A2-F1-P1 | **Listing record**: The Market's listing on top of the kit's marketplace listing: item details, price type, quantity, location, shipping and state. | data | medium | kit-market, kit-core, KIT-A5-F1-P1 |  |
| MKT-A2-F1-P2 | **Listing commands**: Create, edit, end, relist and mark sold as operations, so people, AI agents and add-ins all use one path. | library | medium | kit-ops, KIT-A15-F1-P2, MKT-A2-F1-P1 |  |
| MKT-A2-F1-P3 | **Listing form**: A short step-by-step form for phones and computers that asks only what the chosen category needs. | interface | medium | MKT-A2-F1-P2, MKT-A3-F2-P1, KIT-A5-F1-P2 |  |
| MKT-A2-F1-P4 | **Listing page**: What buyers see: photos, price, condition, details, seller, shipping and pickup, questions and the full price before buying. | interface | medium | MKT-A2-F1-P1, MKT-A1-F2-P1 |  |
| MKT-A2-F1-P5 | **Listing pages found by web search**: Every listing page rendered on the server, with product data markup, site maps and a share card, so web search and shared links bring buyers in; shop sites reuse the same pages. | service | medium | MKT-A2-F1-P4, KIT-A20-F4-P1 |  |
| MKT-A2-F2-P1 | **Market photo rules**: The Market's photo count, sizes and crops on the kit's file uploads, which resume on a bad connection, strip location data and run the safety scans. | integration | small | KIT-A15-F7-P1, KIT-A15-F7-P3, KIT-A15-F7-P4, engine-media |  |
| MKT-A2-F2-P2 | **Photo editor**: Crop, rotate, straighten, brighten and reorder photos before posting. | interface | small | MKT-A2-F2-P1 |  |
| MKT-A2-F2-P3 | **Short video**: A clip of up to a minute per listing, compressed for phones. | service | medium | engine-media, MKT-A2-F2-P1 |  |
| MKT-A2-F2-P4 | **Photo limits and clean-up**: A per-listing photo limit, and clean-up a set time after a listing ends, on the kit's storage at cost. | data | small | KIT-A15-F7-P2, MKT-A2-F2-P1 |  |
| MKT-A2-F3-P1 | **Price types**: Fixed price, fixed price with offers, free and swap, with room for auctions later. | library | small | kit-market, KIT-A5-F3-P1, MKT-A2-F1-P1 |  |
| MKT-A2-F3-P2 | **Auto-accept and auto-decline**: Seller-set limits that accept offers above one amount and decline offers below another. | service | small | MKT-A5-F2-P1 |  |
| MKT-A2-F4-P1 | **Listing lifecycle rules**: How long a listing runs, renewals, and what may change once there are offers, questions or bids. | library | small | MKT-A2-F1-P2 |  |
| MKT-A2-F4-P2 | **Change history for buyers**: Material changes made after a buyer has engaged are shown on the listing. | interface | small | kit-ops, MKT-A2-F1-P4 |  |
| MKT-A2-F4-P3 | **On hold for a buyer**: Mark an item as on hold for a named buyer for a set time, so others see it is spoken for. | service | small | MKT-A2-F1-P2 |  |
| MKT-A2-F5-P1 | **Market listing checks**: Runs the Market's item rules and category fields through the kit's listing checks, and explains any problem in plain words. | library | small | KIT-A5-F1-P3, MKT-A12-F1-P2, MKT-A2-F1-P2 |  |
| MKT-A2-F5-P2 | **Product safety fields**: Extra fields the law asks for on some items, such as safety warnings, or the maker's details for buyers in the EU. | data | small | MKT-A3-F2-P1, MKT-A12-F1-P2 |  |
| MKT-A2-F6-P1 | **Suggestions from photos**: Suggests category, title and details from the photos and words; the seller confirms every field and nothing posts without them. | service | medium | MKT-A3-F1-P1, KIT-A17-F6-P2, engine-media |  |
| MKT-A2-F6-P2 | **Price guide**: A range of what similar items sold for recently, shown while pricing. | service | medium | MKT-A16-F4-P1 |  |
| MKT-A2-F6-P3 | **AI help labels**: How AI-made or AI-edited listing photos are labelled, and what the seller must confirm, under the kit's AI tools policy. | research | small | KIT-A17-F6-P1 |  |
| MKT-A2-F7-P1 | **Variants and quantity**: Options such as size and colour, each with its own price, stock and photos. | data | medium | MKT-A2-F1-P1 |  |
| MKT-A2-F7-P2 | **Option picker**: Lets buyers choose an option and see its price and stock. | interface | small | MKT-A2-F7-P1, MKT-A2-F1-P4 |  |
| MKT-A2-F8-P1 | **Drafts that work offline**: Drafts saved on the device and synced, so a listing can be started with no signal. | service | small | kit-sync, MKT-A2-F1-P1 |  |
| MKT-A2-F8-P2 | **Templates and sell similar**: Start a new listing from a saved template or a past listing. | service | small | MKT-A2-F1-P2 |  |
| MKT-A2-F8-P3 | **Scheduled start**: Pick a date and time for a listing to go live. | service | small | MKT-A2-F1-P2 |  |
| MKT-A2-F9-P1 | **Making declaration**: Our own set of making labels on each listing, with the partner named where one was used. | data | small | MKT-A2-F1-P1 |  |
| MKT-A2-F9-P2 | **Making checks**: Spot checks and member reports for false handmade or vintage claims. | service | medium | MKT-A2-F9-P1, MKT-A17-F1-P1 |  |
| MKT-A2-F10-P1 | **Personalisation fields**: Seller-set questions the buyer answers at checkout, with length limits. | data | small | MKT-A2-F1-P1 |  |
| MKT-A2-F10-P2 | **Making time**: Days needed to make an item before dispatch, shown and used in delivery estimates; when making takes longer than a card hold lasts, the buyer pays a deposit then the balance. | data | small | MKT-A2-F1-P1, KIT-A3-F2-P4, KIT-A3-F2-P3 |  |
| MKT-A2-F10-P3 | **Digital file delivery**: Secure download links after payment, with a download limit and the file kept safe from sharing links. | service | medium | KIT-A15-F7-P1, MKT-A9-F1-P1 |  |
| MKT-A2-F10-P4 | **Digital goods rules**: GST on digital items, refunds for downloads, and the seller confirming they hold the rights. | legal | small | KIT-A4-F1-P2, KIT-A18-F8-P1 |  |
| MKT-A2-F11-P1 | **Wanted ad record**: An ask on the kit's marketplace loop: what is wanted, a price range, area and an end date. | data | small | kit-market, MKT-A1-F1-P2 |  |
| MKT-A2-F11-P2 | **Wanted ad alerts**: Shows a wanted ad to sellers listing similar items, and tells the asker about matching new listings through the saved search matcher (MKT-A4-F4-P2). | service | small | MKT-A2-F11-P1, MKT-A4-F4-P2 |  |
| MKT-A2-F11-P3 | **Wanted ads search**: Wanted ads in their own search tab, with replies going to the chat. | interface | small | MKT-A2-F11-P1, MKT-A4-F1-P4, MKT-A10-F2-P1 |  |
| MKT-A2-F12-P1 | **Share cards**: A share link and photo card for every listing, ready for community groups, chats and social sites, including through open social protocols. | interface | small | MKT-A2-F1-P5, KIT-A20-F9-P1 |  |
| MKT-A2-F13-P1 | **Swap offers**: Swap offers on the kit's offers: an item for an item, or an item plus money, with buyer protection only on the money part. | library | small | KIT-A5-F3-P3, MKT-A5-F2-P1 |  |
| MKT-A2-F13-P2 | **Swap handover**: Both sides confirm the swap with their own pickup codes, and both rate the trade. | library | small | MKT-A7-F1-P4, MKT-A11-F1-P1 |  |
| MKT-A2-F14-P1 | **Hire listings**: A hire price per day or week, an availability calendar and a return date, on the kit's availability calendars. | library | medium | KIT-A5-F5-P1, MKT-A2-F1-P1 |  |
| MKT-A2-F14-P2 | **Hire deposits and damage**: A deposit held with the kit's deposits and damage holds, a return check with photos, and a damage claim that goes to a dispute if the two sides disagree. | library | small | KIT-A3-F10-P1, KIT-A5-F7-P2, KIT-A11-F1-P1 |  |
| MKT-A2-F14-P3 | **Hire terms**: A plain hire agreement, and the consumer law that applies when an owner hires out items in trade. | legal | small | KIT-A18-F8-P1 |  |

## MKT-A3 Categories and item details

A shared category tree and the item details each kind of item needs, so filters work, listings can be compared, and known products fill themselves in.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A3-F1 Browse categories | Find items by walking a category tree, in their own language. | core | Category tree; Open classification sources; Category browse pages; Category change proposals |
| MKT-A3-F2 Item details by category | Fill in the details that matter for each kind of item, such as size, brand, capacity or year. | core | Item fields per category; Units and sizes |
| MKT-A3-F3 Describe the condition | Choose a clear condition grade with a short definition, and point out flaws with photos. | core | Condition scale; Flaw notes |
| MKT-A3-F4 Scan a barcode or book number | Scan a barcode or ISBN to fill in the details of a known product. | next | Barcode scanner; Product catalogue; Open product data sources |
| MKT-A3-F5 Parts that fit | Check whether a vehicle, appliance or device part fits your model before buying. | later | Fit data; Fit data licences; My vehicle filter |
| MKT-A3-F6 Proof for valuable items | Add serial numbers, certificates and proof of origin for collectables and valuable items. | later | Proof of origin fields; Independent checkers; Stolen goods check |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A3-F1-P1 | **Category tree**: Our own versioned category tree with translations, built from open product classifications where their licences allow. | data | medium | kit-core |  |
| MKT-A3-F1-P2 | **Open classification sources**: Which open product classifications we may build on, under which licences, and how to credit them. | research | small |  |  |
| MKT-A3-F1-P3 | **Category browse pages**: Category pages with sub-categories, counts and the most useful filters for each. | interface | small | MKT-A3-F1-P1, MKT-A4-F1-P2 |  |
| MKT-A3-F1-P4 | **Category change proposals**: Members propose new or merged categories; moderators apply them and listings move with them. | service | small | MKT-A3-F1-P1, MKT-A17-F4-P1, KIT-A12-F1-P1 |  |
| MKT-A3-F2-P1 | **Item fields per category**: A schema of required and optional fields for each category, with allowed values. | data | medium | MKT-A3-F1-P1 |  |
| MKT-A3-F2-P2 | **Units and sizes**: Metric first, with clothing and shoe size charts and conversions for other countries. | library | small | kit-core |  |
| MKT-A3-F3-P1 | **Condition scale**: Our own condition grades per kind of item, each with a plain definition. | data | small | MKT-A3-F1-P1 |  |
| MKT-A3-F3-P2 | **Flaw notes**: A place to list flaws and link each to a photo. | interface | small | MKT-A2-F1-P3, MKT-A2-F2-P1 |  |
| MKT-A3-F4-P1 | **Barcode scanner**: Reads barcodes and ISBNs with the phone camera, in the browser. | interface | small | engine-media |  |
| MKT-A3-F4-P2 | **Product catalogue**: Shared records of known products, built by members and from open product data whose licences allow it. | data | large | MKT-A3-F2-P1 |  |
| MKT-A3-F4-P3 | **Open product data sources**: Which open product and book databases exist, their licences and what they cover in New Zealand. | research | small |  |  |
| MKT-A3-F5-P1 | **Fit data**: Records of which part fits which model, from open or properly licensed data and seller input. | data | large | MKT-A3-F4-P2 |  |
| MKT-A3-F5-P2 | **Fit data licences**: Where vehicle and appliance fit data can be obtained, at what cost and under what terms. | research | medium |  |  |
| MKT-A3-F5-P3 | **My vehicle filter**: Save a vehicle or model and filter search to parts that fit it. | interface | small | MKT-A3-F5-P1, MKT-A4-F1-P4 |  |
| MKT-A3-F6-P1 | **Proof of origin fields**: Serial numbers, certificates and history attached to a listing and carried to the buyer. | data | small | MKT-A3-F2-P1 |  |
| MKT-A3-F6-P2 | **Independent checkers**: Independent authenticators for high-value items such as watches and trading cards: how they check, what it costs, and how the item travels. | research | medium |  |  |
| MKT-A3-F6-P3 | **Stolen goods check**: Check serial numbers against public stolen-property lists where they exist. | integration | medium | MKT-A12-F1-P2 |  |

## MKT-A4 Search and discovery

Finding the right item fast: keyword search, filters, distance, fair ranking with published rules and no paid placement, saved searches, watchlists and feeds people choose.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A4-F1 Search by keyword | Search all listings by words, with spelling forgiven and results in under a second. | core | Market search fields; Market words and synonyms; Search page |
| MKT-A4-F2 Filter and sort | Narrow results by category, price, condition, distance, shipping, seller type and item details. | core | Filters from item fields; Distance search |
| MKT-A4-F3 Fair ranking, published | See how results are ordered, knowing no one can pay to push other sellers down. | core | Market ranking factors; Why this result; Ranking changes by vote |
| MKT-A4-F4 Saved searches and alerts | Save a search and get told when a new matching item is listed or a price drops. | core | New listing matcher; Alert settings |
| MKT-A4-F5 Watchlist and favourites | Watch an item to follow its price and end time, and follow favourite sellers and shops. | core | Watchlist; Watch alerts; Lists to share |
| MKT-A4-F6 Browse and feeds you choose | Browse new items nearby or in followed categories and sellers, with the feed's rules shown. | next | Home feed; Follow sellers, shops and categories; Recently viewed and similar items |
| MKT-A4-F7 Search by photo | Take a photo of something and find similar items for sale. | later | Image similarity index; Open image model choice |
| MKT-A4-F8 Map view | See items for pickup on a map of the area. | later | Listings map |
| MKT-A4-F9 Search from other apps and AI agents | Let an AI assistant, price comparer or another app search listings through the open command API. | next | Search commands; Open listing feed |
| MKT-A4-F10 One Market across member-run servers | Search and trade across every server that runs the Market, including ones members run themselves. | later | Search across servers; Trust across servers |
| MKT-A4-F11 Search in your own language | Find listings written in another language, and search in your own. | later | Search across languages |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A4-F1-P2 | **Market search fields**: Puts every live listing, its item fields, price, location and seller into the kit's search service within seconds of a change, with the Market's filters and sorting. | library | small | KIT-A6-F1-P2, MKT-A2-F1-P1, MKT-A3-F2-P1 |  |
| MKT-A4-F1-P3 | **Market words and synonyms**: Market synonyms and word stems for item names and brands, on the kit's macron-aware matching. | library | small | KIT-A19-F1-P4, MKT-A4-F1-P2 |  |
| MKT-A4-F1-P4 | **Search page**: Search box, results, filters and sorting that work on a phone and with a screen reader. | interface | medium | MKT-A4-F1-P2, KIT-A19-F2-P1 |  |
| MKT-A4-F2-P1 | **Filters from item fields**: Filters built from each category's fields, with counts per value. | service | medium | MKT-A4-F1-P2, MKT-A3-F2-P1 |  |
| MKT-A4-F2-P2 | **Distance search**: Search by distance from a town or suburb, without revealing anyone's exact address. | service | medium | KIT-A7-F10-P1, MKT-A4-F1-P2 |  |
| MKT-A4-F3-P1 | **Market ranking factors**: The Market's factors (relevance, newness, distance and seller reliability) on the kit's published ranking rules, with no paid placement. | library | small | KIT-A6-F1-P3, MKT-A4-F1-P2 |  |
| MKT-A4-F3-P2 | **Why this result**: Shows the kit's match explanation on each Market result. | interface | small | KIT-A6-F5-P1, MKT-A4-F3-P1, MKT-A4-F1-P4 |  |
| MKT-A4-F3-P3 | **Ranking changes by vote**: Changes to the ranking formula go to a member vote and are logged. | service | small | KIT-A12-F4-P1, MKT-A4-F3-P1, MKT-A18-F4-P1 |  |
| MKT-A4-F4-P2 | **New listing matcher**: Checks each new or cheaper listing against members' saved searches, including wanted ads, and sends alerts through the kit's notifications. | service | small | KIT-A6-F8-P1, MKT-A4-F1-P2, KIT-A8-F3-P2 |  |
| MKT-A4-F4-P3 | **Alert settings**: Choose instant, daily or weekly alerts, and by push, email or none. | interface | small | KIT-A6-F8-P1, KIT-A8-F5-P1 |  |
| MKT-A4-F5-P1 | **Watchlist**: Items a member is watching, with notes, on the kit's favourites. | data | small | KIT-A5-F12-P1, MKT-A2-F1-P1 |  |
| MKT-A4-F5-P2 | **Watch alerts**: Alerts when a watched item is ending, drops in price, gets an offer to watchers, or sells. | service | small | MKT-A4-F5-P1, KIT-A8-F3-P2 |  |
| MKT-A4-F5-P3 | **Lists to share**: Group saved items into named lists and share a list by link. | interface | small | MKT-A4-F5-P1 |  |
| MKT-A4-F6-P1 | **Home feed**: Newest first by default, with feeds members choose, such as near me or followed shops, and the rules of each shown. | service | medium | MKT-A4-F1-P2, MKT-A4-F3-P1 |  |
| MKT-A4-F6-P2 | **Follow sellers, shops and categories**: Follow lists that feed the home page and alerts. | data | small | KIT-A5-F12-P1, MKT-A1-F1-P2 |  |
| MKT-A4-F6-P3 | **Recently viewed and similar items**: A history of viewed items, and similar items on each listing page. | service | medium | MKT-A4-F1-P2, MKT-A2-F1-P4 |  |
| MKT-A4-F7-P1 | **Image similarity index**: Finds listings that look like a photo, using an open image model. | service | large | engine-media, MKT-A4-F1-P2 |  |
| MKT-A4-F7-P2 | **Open image model choice**: Open image models compared on licence, accuracy and running cost. | research | medium | KIT-A17-F6-P1 |  |
| MKT-A4-F8-P1 | **Listings map**: Approximate pickup areas on an open map, never exact addresses. | interface | medium | KIT-A7-F1-P2, KIT-A7-F10-P1, MKT-A4-F1-P2 |  |
| MKT-A4-F9-P1 | **Search commands**: Search and read commands with rate limits, offered to AI agents through the kit's MCP server. | library | small | KIT-A17-F1-P1, KIT-A16-F1-P3, MKT-A4-F1-P2, MKT-A16-F7-P1 |  |
| MKT-A4-F9-P2 | **Open listing feed**: A standard product feed of a seller's listings that web search and comparison sites can read. | integration | medium | MKT-A2-F1-P1 |  |
| MKT-A4-F10-P1 | **Search across servers**: How listings on member-run servers join one search, so the Market keeps its network when it is split across servers. | research | medium | KIT-A22-F7-P2, MKT-A4-F1-P2 |  |
| MKT-A4-F10-P2 | **Trust across servers**: How held payments, ratings and bans carry across servers, so a scammer banned on one cannot trade on another. | research | medium | KIT-A22-F7-P2, KIT-A10-F14-P1, KIT-A9-F7-P1, KIT-A3-F2-P1 |  |
| MKT-A4-F11-P1 | **Search across languages**: Finds listings written in another language, such as Samoan, Mandarin or Hindi, by matching translated words through the kit's translation service, and shows the original with a translation. | library | medium | KIT-A19-F10-P2, MKT-A4-F1-P2, MKT-A10-F6-P1 |  |

## MKT-A5 Buying, offers and checkout

Getting from wanting an item to having paid for it: buy now, offers and counter offers, one cart across sellers, payment choices, cash on pickup, gifts, causes and business buying.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A5-F1 Buy now | Buy a fixed-price item in a few taps, with the full price shown before paying. | core | Buy command; Market price lines; Checkout screens; Order confirmation; Address book |
| MKT-A5-F2 Make an offer | Offer a lower price, and go back and forth with counter offers until one side accepts or it runs out. | core | Market offer rules; Offer screens; Seller offers to watchers |
| MKT-A5-F3 Pay the way you want | Pay by card, bank payment or phone wallet, with saved methods kept by the payment provider. | core | Market payment methods; Bank payments in the Market; Pay later options |
| MKT-A5-F4 Cash on pickup | Agree to pay cash at pickup, knowing buyer protection does not cover it. | core | Cash on pickup choice |
| MKT-A5-F5 One cart across sellers | Put items from several sellers in one cart and pay once. | next | Cart; Split payment by seller; Combined shipping |
| MKT-A5-F6 Ask for something custom | Ask a maker for a custom item, agree the price and time in the chat, and buy it from a listing made just for you. | next | Custom request; Private listing for one buyer |
| MKT-A5-F7 Buy as a gift | Send an item as a gift with a note, gift wrap if the seller offers it, and a receipt without prices. | next | Gift options; Gift receipt |
| MKT-A5-F8 Buy for a business | Get a tax invoice with GST details when buying from a GST-registered seller. | next | Market tax invoices |
| MKT-A5-F9 Give to a cause | Sell for a registered charity and send part or all of the price to it, or add a donation at checkout. | later | Donation split; Donation tax credits |
| MKT-A5-F10 Pre-order and back in stock | Order something not yet made or in stock, or ask to be told when it is back. | later | Pre-order terms; Back in stock alerts |
| MKT-A5-F11 A fair chance at limited stock | Have a fair chance at limited releases and scarce items, without resellers' bots buying them first. | next | Limits and fair queues |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A5-F1-P1 | **Buy command**: Reserves the item or stock while paying and stops two people buying the last one. | library | medium | kit-market, KIT-A5-F4-P1, MKT-A2-F1-P2 |  |
| MKT-A5-F1-P2 | **Market price lines**: The Market's price lines (item, shipping, buyer fee and GST) on the kit's full-price-first calculator, shown before the pay button and never added later. | library | small | KIT-A5-F3-P2, MKT-A7-F2-P1, MKT-A8-F1-P1, KIT-A4-F1-P2 |  |
| MKT-A5-F1-P3 | **Checkout screens**: Delivery or pickup choice, address, payment and a final check, short enough for a phone. | interface | medium | KIT-A3-F1-P3, MKT-A5-F1-P1, MKT-A5-F1-P5, MKT-A7-F1-P2 |  |
| MKT-A5-F1-P4 | **Order confirmation**: A confirmation page and message to both sides with what happens next. | service | small | MKT-A9-F1-P1, KIT-A8-F3-P2 |  |
| MKT-A5-F1-P5 | **Address book**: Saved delivery addresses with a default, and gift addresses kept apart so they are never used by mistake. | data | small | MKT-A1-F1-P2, KIT-A7-F2-P2, KIT-A19-F1-P2 |  |
| MKT-A5-F2-P1 | **Market offer rules**: The Market's rules on the kit's offers: expiry times, how many counters, and payment taken when an offer is accepted. | library | small | KIT-A5-F3-P3 |  |
| MKT-A5-F2-P2 | **Offer screens**: Make, accept, decline and counter offers from the listing or the chat. | interface | small | MKT-A5-F2-P1, MKT-A10-F2-P1 |  |
| MKT-A5-F2-P3 | **Seller offers to watchers**: A seller sends one lower price to everyone watching an item, for a limited time. | service | small | MKT-A5-F2-P1, MKT-A4-F5-P1 |  |
| MKT-A5-F3-P1 | **Market payment methods**: Turns on card, phone wallet and bank payment for the Market through the kit's checkout and saved methods; the Market never sees card numbers. | integration | small | KIT-A3-F1-P3, KIT-A3-F1-P2, KIT-A3-F8-P1 |  |
| MKT-A5-F3-P2 | **Bank payments in the Market**: Turns on the kit's bank payments for Market orders, starting with high-value items where card costs are highest. | integration | small | KIT-A3-F11-P1 |  |
| MKT-A5-F3-P3 | **Pay later options**: Whether buy now pay later fits co-op values, and the lending law that would apply. | research | small |  |  |
| MKT-A5-F4-P1 | **Cash on pickup choice**: A seller setting and a clear warning to the buyer that protection does not apply. | interface | small | MKT-A2-F1-P1, MKT-A11-F2-P1, KIT-A3-F18-P1 |  |
| MKT-A5-F5-P1 | **Cart**: A cart that holds items from many sellers and many shops, kept across devices. | data | small | MKT-A5-F1-P1, kit-sync |  |
| MKT-A5-F5-P2 | **Split payment by seller**: One payment split into a held amount per seller, each with its own fee. | service | medium | KIT-A5-F11-P1, KIT-A3-F2-P1, MKT-A7-F1-P2, MKT-A5-F5-P1 |  |
| MKT-A5-F5-P3 | **Combined shipping**: One shipping price when several items come from the same seller. | service | medium | MKT-A8-F1-P1, MKT-A5-F5-P1 |  |
| MKT-A5-F6-P1 | **Custom request**: A buyer describes what they want, with photos, and the seller answers with a price and making time. | interface | small | MKT-A10-F2-P1 |  |
| MKT-A5-F6-P2 | **Private listing for one buyer**: A listing only the named buyer can see and buy, made from the agreed request. | service | small | MKT-A2-F1-P2 |  |
| MKT-A5-F7-P1 | **Gift options**: Gift note, wrap choice and a separate delivery address on the order. | data | small | MKT-A9-F1-P1 |  |
| MKT-A5-F7-P2 | **Gift receipt**: A receipt without prices that still allows a return. | library | small | engine-pdf, MKT-A9-F1-P1 |  |
| MKT-A5-F8-P1 | **Market tax invoices**: Tax invoices from the kit for each sale by a GST-registered seller, and for the buyer fee to GST-registered buyers. | library | small | KIT-A4-F2-P1, MKT-A7-F9-P1 |  |
| MKT-A5-F9-P2 | **Donation split**: Sends the chosen share of a sale to the charity through the payment provider. | service | medium | KIT-A5-F11-P1, KIT-A3-F3-P2, MKT-A7-F3-P1 |  |
| MKT-A5-F9-P3 | **Donation tax credits**: A purchase whose proceeds go to a charity is not a gift and earns no donation tax credit; only a separate donation at checkout might, with the charity issuing the receipt. | legal | small | MKT-A5-F9-P2 |  |
| MKT-A5-F10-P1 | **Pre-order terms**: Expected dispatch date, the right to cancel if it is late, and when payment is taken: within the provider's hold time limits, or as a deposit then the balance for long waits. | data | small | MKT-A2-F7-P1, KIT-A18-F8-P1, KIT-A3-F2-P4, KIT-A3-F2-P3 |  |
| MKT-A5-F10-P2 | **Back in stock alerts**: Tells waiting buyers when stock returns. | service | small | KIT-A8-F3-P2, KIT-A5-F10-P1 |  |
| MKT-A5-F11-P1 | **Limits and fair queues**: Per-person limits and a fair queue for limited releases, with published rules on when agents and scripts may buy or bid, on the kit's automated acceptance rules. | library | small | KIT-A6-F4-P3, KIT-A17-F4-P1, MKT-A5-F1-P1, MKT-A1-F3-P2 |  |

## MKT-A6 Auctions

Timed auctions with reserves, automatic bidding, fair endings and checks against fake bids, built only after the consumer law research says how.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A6-F1 Run an auction | Set a starting price, a reserve and an end time, and let buyers bid. | later | Auction law; Auction engine; Auction settings |
| MKT-A6-F2 Bid and auto-bid | Bid now, or set a top amount and let the Market bid for you only as much as needed. | later | Automatic bidding; Live bid updates; Bid screens |
| MKT-A6-F3 Fair endings | Have the end time extend when a bid comes in at the last moment, so last-second bids do not decide it. | later | Auto-extend rule; Server clock |
| MKT-A6-F4 Reserve and buy now | Hide a reserve price, show when it is met, and offer a buy now price until bidding passes it. | later | Reserve and buy now logic |
| MKT-A6-F5 Bid history and fake bid checks | See every bid with bidder names hidden, and trust that fake bids from the seller's friends are caught. | later | Public bid history; Fake bid detection; Bid withdrawal rules |
| MKT-A6-F6 When the winner does not pay | Offer the item to the next highest bidder if the winner backs out. | later | Next bidder offers; Unpaid item record |
| MKT-A6-F7 When the reserve is not met | Offer the item to the top bidder when the reserve is not met, or relist it automatically. | later | Offer after the reserve is not met; Automatic relist |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A6-F1-P1 | **Auction law**: What consumer law, auction rules and in-trade disclosure mean for online auctions run by the Market. | legal | medium | KIT-A18-F8-P1 | [MK02](STATUS.md) |
| MKT-A6-F1-P2 | **Auction engine**: Bids as operations, ordered by the server so every bidder sees the same truth. | library | large | kit-market, kit-ops, kit-sync, MKT-A6-F1-P1 |  |
| MKT-A6-F1-P3 | **Auction settings**: Start price, reserve, buy now price, length and end time on the listing form. | interface | small | MKT-A6-F1-P2, MKT-A2-F1-P3 |  |
| MKT-A6-F2-P1 | **Automatic bidding**: Bids for a member up to their hidden top amount, one step at a time. | library | medium | MKT-A6-F1-P2 |  |
| MKT-A6-F2-P2 | **Live bid updates**: Pushes new bids and the current price to everyone watching within a second. | service | medium | KIT-A15-F5-P1, MKT-A6-F1-P2 |  |
| MKT-A6-F2-P3 | **Bid screens**: Place a bid, set a top amount, and see whether you are winning. | interface | small | MKT-A6-F1-P2 |  |
| MKT-A6-F3-P1 | **Auto-extend rule**: Adds time when a bid arrives in the last minutes, up to a limit, published in the rules. | library | small | MKT-A6-F1-P2 |  |
| MKT-A6-F3-P2 | **Server clock**: One clock on the server decides when bids arrive and when auctions close. | service | small | kit-sync, MKT-A6-F1-P2 |  |
| MKT-A6-F4-P1 | **Reserve and buy now logic**: Reserve met flags, buy now removal rules and seller limits on lowering a reserve. | library | small | MKT-A6-F1-P2 |  |
| MKT-A6-F5-P1 | **Public bid history**: Every bid with amount and time, bidders shown by a short code. | interface | small | MKT-A6-F1-P2 |  |
| MKT-A6-F5-P2 | **Fake bid detection**: Flags bids from accounts linked to the seller, by device, payment or address signals. | service | medium | MKT-A12-F2-P2, MKT-A6-F1-P2 |  |
| MKT-A6-F5-P3 | **Bid withdrawal rules**: When a bid may be withdrawn, under the auction rules and consumer law. | legal | small | MKT-A6-F1-P1 |  |
| MKT-A6-F6-P1 | **Next bidder offers**: Offers the item to the runner-up at their highest bid. | service | small | MKT-A6-F1-P2, MKT-A5-F2-P1 |  |
| MKT-A6-F6-P2 | **Unpaid item record**: Records unpaid wins, which count in seller standards and limits. | data | small | MKT-A11-F1-P1, MKT-A6-F1-P2 |  |
| MKT-A6-F7-P1 | **Offer after the reserve is not met**: Lets the seller offer the item at a set price to the highest bidder, then to other bidders and watchers, for a limited time. | library | small | MKT-A6-F4-P1, MKT-A5-F2-P1 |  |
| MKT-A6-F7-P2 | **Automatic relist**: Relists an unsold item with the same or changed settings, up to a set number of times. | library | small | MKT-A6-F1-P2, MKT-A2-F4-P1 |  |

## MKT-A7 Payments, held money and fees

The buyer's money held by a regulated provider until the item arrives, who the seller of record is, the fee and who pays it, payouts, refunds, card disputes, receipts and the Market's anti-money-laundering risks, all on the kit's payments.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A7-F1 Money held until it arrives | Pay into a held account; the seller is paid when the buyer confirms, or when the waiting time runs out. | core | Market hold and release rules; Release times; Pickup code that works offline |
| MKT-A7-F2 See the fee | See the exact fee before listing and before paying, who pays it, and what it pays for. | core | Market fee lines; Fee rules by vote; Market fee model |
| MKT-A7-F3 Get paid | Get paid to a bank account on a set schedule, with every deduction listed. | core | Market payout timing |
| MKT-A7-F4 Refunds | Give a full or part refund in a few taps, back to the original payment method. | core | Market refund reasons; Refund screens |
| MKT-A7-F5 Receipts and records | Get a receipt for every payment and a yearly statement of sales and fees. | core | What a Market receipt shows |
| MKT-A7-F6 Anti-money-laundering duties met | Trade knowing the Market meets anti-money-laundering law without making honest trade harder. | core | Market risks for the co-op's programme; Market signals for suspicious activity |
| MKT-A7-F7 Card disputes | Have card disputes answered on time with evidence from the order, and know who carries a lost one. | core | Market evidence for card disputes; Market loss rule |
| MKT-A7-F8 Protection pool | Know buyers are refunded even when a seller cannot pay back, from a small pool shown on the cost ledger. | next | Market protection pool |
| MKT-A7-F9 Know who the seller is | Know who sold you the item, who charges GST on it, and who answers for it if something goes wrong. | core | Market seller of record |
| MKT-A7-F10 Pay safely for a car or other big item | Pay for a car, boat or machinery by low-cost bank payment into a held account, or by a deposit then the balance. | next | Held bank payment for big items; Deposit then balance |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A7-F1-P2 | **Market hold and release rules**: When a Market order's money is held, released or refunded: on the buyer's confirmation, after the release time, on a pickup code or on a panel decision, using the kit's hold and release rules. | library | medium | KIT-A3-F2-P1, MKT-A9-F1-P1, MKT-A9-F1-P2 |  |
| MKT-A7-F1-P3 | **Release times**: The Market's release times on the kit's auto-confirm: a set number of days after tracked delivery or dispatch, paused by any problem report. | library | small | KIT-A5-F7-P1, KIT-A15-F14-P1, MKT-A7-F1-P2 |  |
| MKT-A7-F1-P4 | **Pickup code that works offline**: The buyer's signed pickup code, on the kit's proof of handover, checked on the seller's phone with no signal and synced later to release the money. | library | medium | KIT-A5-F7-P2, kit-sync, MKT-A7-F1-P2 |  |
| MKT-A7-F2-P1 | **Market fee lines**: The Market's buyer and seller fee lines on the kit's fee calculator, following the fee model (MKT-A7-F2-P4). | library | small | KIT-A3-F4-P1, MKT-A7-F2-P4 |  |
| MKT-A7-F2-P3 | **Fee rules by vote**: Fee rate changes come from member votes and take effect on a published date. | service | small | KIT-A12-F4-P1, MKT-A7-F2-P1, MKT-A18-F3-P2 |  |
| MKT-A7-F2-P4 | **Market fee model**: Decides who pays (buyer, seller or both), a flat fee per order or one based on value, how percentage card costs, the protection pool and fraud losses are shared, and the fee on free items, $2 items and $30,000 vehicles, since one flat fee breaks at both ends of the price range. | research | medium | KIT-A13-F3-P1, KIT-A3-F4-P3, KIT-A13-F3-P5 |  |
| MKT-A7-F3-P1 | **Market payout timing**: Pays a seller through the kit's payout runs once each order's money is released, with held and available amounts shown apart on the kit's earnings page. | library | small | KIT-A3-F3-P2, KIT-A3-F3-P3, MKT-A7-F1-P2 |  |
| MKT-A7-F4-P1 | **Market refund reasons**: Full and part refunds on the kit's refunds, each linked to the order and a Market reason. | library | small | KIT-A3-F5-P1, MKT-A9-F1-P1 |  |
| MKT-A7-F4-P2 | **Refund screens**: Refund from the order with the amount, reason and a note to the buyer. | interface | small | MKT-A7-F4-P1 |  |
| MKT-A7-F5-P1 | **What a Market receipt shows**: The seller of record, GST and the fee on every receipt from the kit, for online and counter sales alike, printed or emailed. | library | small | KIT-A3-F7-P1, MKT-A7-F9-P1 |  |
| MKT-A7-F6-P1 | **Market risks for the co-op's programme**: The Market risks the co-op's one anti-money-laundering programme must cover, such as high-value goods, gift cards, stored balances and fake sales used to move money, and which duties the payment provider carries. | legal | small | KIT-A4-F5-P1 |  |
| MKT-A7-F6-P3 | **Market signals for suspicious activity**: Market patterns, such as repeat sales between the same accounts or prices far above value, sent to the kit's suspicious activity reports. | library | small | KIT-A4-F5-P3, KIT-A10-F12-P2, MKT-A7-F6-P1 |  |
| MKT-A7-F7-P1 | **Market evidence for card disputes**: Sends tracking, messages, photos and the pickup record to the kit's card disputes within the card network's deadlines. | library | small | KIT-A3-F5-P2, MKT-A9-F1-P1, KIT-A11-F2-P1 |  |
| MKT-A7-F7-P2 | **Market loss rule**: Who carries a lost card dispute in the Market: the seller, as a negative balance recovered from future payouts, or the protection reserve when the seller kept every rule, on the kit's loss rules. | legal | small | KIT-A3-F16-P1, KIT-A3-F16-P2, KIT-A13-F3-P5 |  |
| MKT-A7-F8-P1 | **Market protection pool**: What the Market's protection pool covers and its limits, on the kit's protection pool. | research | small | KIT-A11-F10-P1, KIT-A11-F10-P2 |  |
| MKT-A7-F9-P1 | **Market seller of record**: The Market's choice on the kit's seller of record: the trader is the seller, except where the Market is the deemed supplier, for low-value imported goods and its own buyer fee. This sets who charges GST, who owes Consumer Guarantees Act duties, whose name is on the receipt and who answers card disputes. | legal | medium | KIT-A3-F1-P5, MKT-A19-F2-P1, KIT-A18-F8-P1 |  |
| MKT-A7-F10-P1 | **Held bank payment for big items**: Pays a high-value order by bank payment into a held account, where card costs would be too high, and releases it on the same rules as other orders. | integration | medium | KIT-A3-F11-P1, KIT-A3-F2-P1, KIT-A3-F2-P2, MKT-A7-F1-P2 |  |
| MKT-A7-F10-P2 | **Deposit then balance**: A deposit held when the deal is agreed and the balance paid at handover, for items such as vehicles and machinery. | library | small | KIT-A3-F2-P3, KIT-A3-F10-P1, MKT-A7-F10-P1 |  |

## MKT-A8 Shipping, pickup and delivery

Getting the item from seller to buyer: shipping prices, courier labels, tracking, safe pickups with a handover code, local delivery, overseas shipping and return labels.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A8-F1 Choose how it gets there | Offer pickup, or set shipping prices for each part of the country. | core | Shipping profiles; Delivery estimates; Rural and island delivery; Shipping cost after the sale |
| MKT-A8-F2 Pick up safely | Arrange a pickup time and place, with suggested public meeting spots and a code to confirm the handover. | core | Pickup arranging; Public meeting spots; No-show handling; Pickup address shown at the right step |
| MKT-A8-F3 Track it | Follow the parcel from the tracking number the seller adds, with dispatch starting the payment release timer. | core | Tracking number entry |
| MKT-A8-F4 Courier rates, labels and tracking | Get courier rates by size and weight, buy a label in the app, print it or show a code at a drop-off point, and follow it automatically. | next | Courier research; Courier connectors; Rate calculator; Tracking updates; Label printing; Label payment; Courier collection and pickup points; ID-checked delivery |
| MKT-A8-F5 Packing slips and bulk dispatch | Print packing slips and labels for many orders at once. | next | Packing slip; Bulk dispatch screen |
| MKT-A8-F6 Return labels | Send a prepaid return label when a return is accepted. | next | Return label purchase |
| MKT-A8-F7 Local delivery | Have a local item delivered the same day by a nearby independent courier. | later | Local delivery as a shared delivery job; Large item freight |
| MKT-A8-F8 Use a fulfilment warehouse | Send stock to a warehouse that packs and ships orders for the seller. | later | Warehouse connectors |
| MKT-A8-F9 Ship overseas | Sell to buyers in other countries with customs forms filled from the listing. | later | Customs declarations; Duty and tax at checkout; Country shipping limits |
| MKT-A8-F10 Know what can be posted | See before posting which items couriers refuse or charge more for, such as lithium batteries, aerosols and perfume, and how to send them. | next | Posting dangerous goods in NZ; Dangerous goods flags |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A8-F1-P1 | **Shipping profiles**: Reusable sets of shipping options and prices per seller, attached to listings. | data | medium | MKT-A2-F1-P1 |  |
| MKT-A8-F1-P2 | **Delivery estimates**: Making time plus typical transit time, shown as a date range. | library | small | MKT-A8-F1-P1, KIT-A19-F9-P2 |  |
| MKT-A8-F1-P3 | **Rural and island delivery**: Rural, island and remote area surcharges handled and shown up front. | data | small | MKT-A8-F1-P1, KIT-A7-F2-P3, KIT-A7-F5-P1 |  |
| MKT-A8-F1-P4 | **Shipping cost after the sale**: When shipping is worked out after a sale, the seller sends the cost and the buyer pays it in the app. | service | small | MKT-A7-F1-P2, MKT-A9-F1-P1, KIT-A5-F15-P1 |  |
| MKT-A8-F2-P1 | **Pickup arranging**: Propose and agree a time and place in the chat, with reminders. | interface | small | MKT-A10-F2-P1, KIT-A8-F3-P2 |  |
| MKT-A8-F2-P2 | **Public meeting spots**: Suggested well-lit public places that have agreed to be listed, on an open map. | data | small | KIT-A7-F1-P2 |  |
| MKT-A8-F2-P4 | **No-show handling**: If a buyer does not turn up, the seller can cancel and relist, and the no-show is recorded. | service | small | MKT-A9-F1-P1, MKT-A7-F4-P1, KIT-A5-F6-P1 |  |
| MKT-A8-F2-P5 | **Pickup address shown at the right step**: Shows the exact pickup address only when an order reaches pickup, through the kit's approximate location, never as free text in the chat; until then buyers see only the suburb or town. | library | small | KIT-A7-F10-P1, MKT-A9-F1-P2, MKT-A1-F2-P2 |  |
| MKT-A8-F3-P1 | **Tracking number entry**: Seller adds the courier and tracking number when posting by hand, and the buyer gets a tracking link. | interface | small | MKT-A9-F1-P1, MKT-A7-F1-P3 |  |
| MKT-A8-F4-P1 | **Courier research**: Which couriers have published APIs and terms that allow this. | research | small |  | [MK03](STATUS.md) |
| MKT-A8-F4-P2 | **Courier connectors**: One connector per courier for rates, labels and tracking, built as add-ins so more can be added. | integration | large | MKT-A8-F4-P1, KIT-A16-F5-P2 |  |
| MKT-A8-F4-P3 | **Rate calculator**: Works out courier rates from parcel size, weight and both addresses, at checkout and on the listing form. | service | medium | MKT-A8-F1-P1, MKT-A8-F4-P2 |  |
| MKT-A8-F4-P4 | **Tracking updates**: Pulls tracking events from courier links, tells the buyer, and starts the release timer on delivery. | service | medium | MKT-A8-F4-P2, KIT-A8-F3-P2, MKT-A9-F1-P1, MKT-A7-F1-P3 |  |
| MKT-A8-F4-P5 | **Label printing**: Labels for thermal printers and plain A4 paper. | library | small | engine-pdf |  |
| MKT-A8-F4-P6 | **Label payment**: Label cost taken from the seller's balance or card, at the courier's price. | service | small | KIT-A3-F1-P2, MKT-A8-F4-P2, MKT-A7-F3-P1 |  |
| MKT-A8-F4-P7 | **Courier collection and pickup points**: Books a courier to collect a parcel from the seller's door, and lets buyers choose delivery to a parcel locker or pickup point, which suits rural buyers. | integration | medium | MKT-A8-F4-P2, MKT-A8-F4-P3 |  |
| MKT-A8-F4-P8 | **ID-checked delivery**: A courier service that checks the receiver's ID and age and takes a signature, never leaving the parcel at the door, for alcohol, vapes and other R18 items. | integration | small | MKT-A8-F4-P2, MKT-A12-F1-P5 |  |
| MKT-A8-F5-P1 | **Packing slip**: A packing slip per order with items, options and gift notes. | library | small | engine-pdf, MKT-A9-F1-P1 |  |
| MKT-A8-F5-P2 | **Bulk dispatch screen**: Select many orders, buy labels, print and mark dispatched in one go. | interface | medium | MKT-A8-F4-P5, MKT-A9-F1-P1 |  |
| MKT-A8-F6-P1 | **Return label purchase**: Buys a return label from the courier and sends it to the buyer, paid by whoever the return policy says. | integration | small | MKT-A8-F4-P2, MKT-A9-F5-P2 |  |
| MKT-A8-F7-P1 | **Local delivery as a shared delivery job**: Offers local Market deliveries to independent couriers on the kit's shared delivery job type, which Spare Food's couriers also carry, with the kit's worker status, health and safety and insurance rules. | service | small | KIT-A5-F18-P1, KIT-A6-F3-P3, KIT-A23-F1-P2, KIT-A24-F1-P1, KIT-A11-F9-P1, MKT-A9-F1-P1 |  |
| MKT-A8-F7-P2 | **Large item freight**: Options for furniture and other large items: freight firms, movers and member helpers. | research | small |  |  |
| MKT-A8-F8-P1 | **Warehouse connectors**: Sends orders to a third-party warehouse and brings stock levels and tracking back, built as add-ins. | integration | medium | KIT-A16-F5-P2, MKT-A9-F1-P1, KIT-A5-F10-P1 |  |
| MKT-A8-F9-P1 | **Customs declarations**: Customs forms filled from item details, value and tariff codes. | library | medium | MKT-A3-F2-P1, MKT-A8-F4-P2, KIT-A19-F6-P1 |  |
| MKT-A8-F9-P2 | **Duty and tax at checkout**: Import duty and tax worked out and shown at checkout, so buyers pay no surprise charges on arrival. | service | large | KIT-A4-F1-P2, KIT-A4-F7-P1, MKT-A19-F5-P2, MKT-A19-F5-P3 |  |
| MKT-A8-F9-P3 | **Country shipping limits**: Items that may not be sent to, or from, each country. | data | medium | MKT-A12-F1-P2, KIT-A19-F6-P1, MKT-A19-F5-P2, MKT-A19-F5-P3 |  |
| MKT-A8-F10-P1 | **Posting dangerous goods in NZ**: What NZ couriers and the postal service allow for lithium batteries, aerosols, perfume, nail polish and other dangerous goods sent within New Zealand, and what they charge extra. | research | small | MKT-A8-F4-P1 |  |
| MKT-A8-F10-P2 | **Dangerous goods flags**: Flags listings in categories that carry dangerous goods, shows the courier rules on the listing form, and offers only shipping services that accept them. | library | small | MKT-A8-F10-P1, MKT-A3-F2-P1, MKT-A8-F1-P1 |  |

## MKT-A9 Orders, returns and after-sale

Everything after the pay button: order lists for both sides, confirming arrival, cancelling, returns under the seller's policy and the law, reporting problems, custom and digital orders.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A9-F1 See my orders | See every purchase and sale with its status in one list. | core | Order record; Market order states; Market order lists |
| MKT-A9-F2 Confirm it arrived | Confirm the item arrived as described, which pays the seller. | core | Confirm received; Confirm reminders |
| MKT-A9-F3 Change or cancel an order | Change the address or items, or ask to cancel, before dispatch, with an instant refund if the seller agrees. | core | Market cancellation rules |
| MKT-A9-F4 Report a problem | Report non-arrival, damage or not as described, and get guided steps before a dispute. | core | Problem report flow; Seller response window; Courier claims |
| MKT-A9-F5 Return an item | Return an item for a fault, or for a change of mind where the seller allows it. | next | Return policies; Return requests; Your rights guide |
| MKT-A9-F6 Follow a custom or digital order | Follow the making of a custom item, and download digital items. | next | Making updates; Downloads page |
| MKT-A9-F8 Ship in parts and exchange online | Send an order in more than one parcel, or swap an item for another size or colour without a refund and a new order. | later | Part and split shipments; Online exchanges |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A9-F1-P1 | **Order record**: The Market's order on the kit's order steps: items, options, prices, fees, shipping, payment and messages. | data | medium | KIT-A5-F4-P1, kit-ops |  |
| MKT-A9-F1-P2 | **Market order states**: The Market's states on the kit's order steps (paid, dispatched, delivered, received, problem, disputed, refunded and closed) and the allowed moves between them. | library | small | MKT-A9-F1-P1, KIT-A5-F4-P1 |  |
| MKT-A9-F1-P3 | **Market order lists**: Bought and sold lists on the kit's orders page, with what needs doing next on each Market order. | interface | small | MKT-A9-F1-P1, KIT-A5-F8-P1, KIT-A5-F4-P2 |  |
| MKT-A9-F2-P1 | **Confirm received**: One tap to confirm, which releases the held payment. | service | small | MKT-A7-F1-P2, MKT-A9-F1-P1, KIT-A5-F7-P1 |  |
| MKT-A9-F2-P2 | **Confirm reminders**: Gentle reminders to confirm, and a notice before automatic release. | service | small | KIT-A8-F3-P2, MKT-A9-F1-P1, MKT-A7-F1-P3 |  |
| MKT-A9-F3-P1 | **Market cancellation rules**: Who may cancel a Market order and when, set on the kit's cancellation policies, and how a cancel counts in seller standards. | library | small | KIT-A5-F6-P1, MKT-A9-F1-P1, MKT-A7-F4-P1 |  |
| MKT-A9-F4-P1 | **Problem report flow**: Asks what went wrong, gathers photos, pauses the payment release and tells the seller. | interface | medium | MKT-A9-F1-P1, KIT-A11-F1-P1, MKT-A7-F1-P2, KIT-A15-F7-P1 |  |
| MKT-A9-F4-P2 | **Seller response window**: A set time for the seller to fix it, by refund, replacement or return, before the case moves on. | library | small | MKT-A9-F1-P1, KIT-A11-F6-P1 |  |
| MKT-A9-F4-P3 | **Courier claims**: Guides the seller through claiming from the courier for a lost or damaged parcel, with the evidence from the order. | service | small | MKT-A9-F1-P1 |  |
| MKT-A9-F5-P1 | **Return policies**: Seller's change of mind policy, with the consumer law minimum shown for in-trade sellers. | data | small | MKT-A1-F1-P2, MKT-A1-F4-P1, KIT-A18-F8-P1 |  |
| MKT-A9-F5-P2 | **Return requests**: Request, approve, ship back, receive and refund, each step an operation. | service | medium | MKT-A9-F1-P1, MKT-A7-F4-P1, MKT-A9-F5-P1 |  |
| MKT-A9-F5-P3 | **Your rights guide**: Explains a buyer's rights in plain words, depending on whether the seller is in trade. | interface | small | KIT-A18-F8-P1 |  |
| MKT-A9-F6-P1 | **Making updates**: Seller posts progress and photos on a made-to-order item, linked to the order. | service | small | MKT-A9-F1-P1, MKT-A10-F2-P1 |  |
| MKT-A9-F6-P2 | **Downloads page**: A buyer's digital purchases with their download links. | interface | small | MKT-A9-F1-P1 |  |
| MKT-A9-F8-P1 | **Part and split shipments**: Ships some items of an order now and the rest later, or from different places, each parcel with its own tracking, and releases the held money for each parcel as it arrives. | library | small | MKT-A9-F1-P2, MKT-A8-F3-P1, MKT-A7-F1-P2, KIT-A5-F11-P1 |  |
| MKT-A9-F8-P2 | **Online exchanges**: Swaps a returned item for another size, colour or option, charging or refunding any price difference, with stock moved in one step. | library | small | MKT-A9-F5-P2, MKT-A2-F7-P1, KIT-A5-F15-P1, KIT-A5-F10-P1 |  |

## MKT-A10 Messages and questions

Talking about an item: public questions on listings, private chat linked to the item or order on the kit's chat, warnings on risky messages, notifications people choose and translation, with reporting through the kit.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A10-F1 Ask a question in public | Ask the seller a question on the listing, with the answer shown to every buyer. | core | Public questions; Question checks |
| MKT-A10-F2 Chat with a buyer or seller | Message privately about an item, with photos and offers in the chat. | core | Market chat threads; Quick replies |
| MKT-A10-F3 Warnings on risky messages | Get a clear warning when a message asks to pay off the platform, shares a strange link, or asks for codes. | core | Market scam patterns; Contact detail masking |
| MKT-A10-F4 Notifications you choose | Choose which updates arrive by push, email or text, and set quiet hours. | core | Market notification types |
| MKT-A10-F6 Translate messages | Read a message or listing in your own language with one tap. | later | Translate listings and questions |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A10-F1-P1 | **Public questions**: Questions and answers stored with the listing and shown on its page. | data | small | MKT-A2-F1-P1, MKT-A2-F1-P4 |  |
| MKT-A10-F1-P2 | **Question checks**: Screens public questions for abuse and contact details before they show. | service | small | MKT-A17-F1-P1, MKT-A12-F2-P2 |  |
| MKT-A10-F2-P1 | **Market chat threads**: Private conversations on the kit's chat service, each linked to a listing or order, with the item, price, offers and order status shown at the top. | library | small | KIT-A8-F1-P2, KIT-A8-F1-P3, MKT-A2-F1-P1 |  |
| MKT-A10-F2-P3 | **Quick replies**: One-tap questions and answers such as asking if an item is still available, in the member's language. | interface | small | MKT-A10-F2-P1, KIT-A8-F1-P3, KIT-A19-F1-P1 |  |
| MKT-A10-F3-P1 | **Market scam patterns**: The Market's own patterns, such as requests to pay off the platform, fake payment links and requests for codes, set as rules on the kit's scam warnings in chat. | library | small | KIT-A8-F9-P1, KIT-A10-F12-P2, MKT-A10-F2-P1 |  |
| MKT-A10-F3-P3 | **Contact detail masking**: Masks phone numbers, emails and street addresses in early messages; contact details are shared once a sale is agreed, and the pickup address only through the pickup step. | service | small | MKT-A10-F2-P1, MKT-A8-F2-P5 |  |
| MKT-A10-F4-P1 | **Market notification types**: Every Market notification listed with a default channel and urgency. | data | small | KIT-A8-F3-P2, KIT-A8-F5-P1 |  |
| MKT-A10-F6-P1 | **Translate listings and questions**: Translates listing text and public questions on request through the kit's machine translation, with the original one tap away; chat uses the kit's message translation. | integration | small | KIT-A19-F10-P2, KIT-A8-F7-P1, MKT-A2-F1-P4 |  |

## MKT-A11 Ratings, buyer protection and disputes

Trust between strangers: two-way ratings from real trades only, buyer protection claims, disputes decided by a member panel while money stays held, and seller standards earned from the record.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A11-F1 Rate each other | Rate the other side after each trade, with a short comment and a reply. | core | Market trade ratings; Rating display |
| MKT-A11-F2 Claim buyer protection | Claim a refund when an item does not arrive or is not as described. | core | Protection rules; Claim flow |
| MKT-A11-F3 Disputes decided by members | Take a disagreement to a panel of trained members, with the money held until they decide. | core | Decision to money; Evidence pack and outside routes |
| MKT-A11-F4 Seller standards | See which sellers dispatch on time and answer quickly, earned from their record, not bought. | next | Performance measures; Reliability badges; Limits for low standards |
| MKT-A11-F5 Review a product | Review the product itself, separate from the trade rating, so later buyers of the same item can judge it. | next | Product reviews; Review display |
| MKT-A11-F6 Honest ratings | Trust that ratings come only from real trades and that bought or swapped ratings are caught. | next | Market fake rating rules |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A11-F1-P1 | **Market trade ratings**: Ratings on the kit's ratings service tied to a completed Market order, including cash, swap and pickup trades, revealed together so no one rates in revenge. | library | small | KIT-A9-F1-P1, MKT-A9-F1-P1 |  |
| MKT-A11-F1-P3 | **Rating display**: Score, count and recent comments on profiles and listings, with replies. | interface | small | MKT-A11-F1-P1, MKT-A1-F2-P1, KIT-A9-F6-P1, KIT-A9-F1-P2 |  |
| MKT-A11-F2-P1 | **Protection rules**: What is covered, up to what amount, by when, and what is not, such as cash trades. | legal | medium | MKT-A7-F1-P2 | [MK01](STATUS.md) |
| MKT-A11-F2-P2 | **Claim flow**: Opens a claim from a problem report once the seller's response window has passed. | service | medium | MKT-A11-F2-P1, MKT-A7-F1-P2, MKT-A9-F1-P1, MKT-A9-F4-P2, KIT-A11-F1-P1 |  |
| MKT-A11-F3-P4 | **Decision to money**: Turns a panel decision or a settlement on a Market order into a release, a refund or a split of the held money. | library | small | KIT-A11-F3-P1, KIT-A3-F2-P1, MKT-A7-F1-P2, MKT-A7-F4-P1 |  |
| MKT-A11-F3-P6 | **Evidence pack and outside routes**: Exports a case's evidence as one pack for the Disputes Tribunal, and links to report scams to Netsafe and Police, alongside the kit's guidance on outside routes. | library | small | KIT-A11-F7-P1, KIT-A11-F2-P1, engine-pdf |  |
| MKT-A11-F4-P1 | **Performance measures**: Dispatch time, reply time, cancellations and problem rates per seller. | service | medium | MKT-A9-F1-P1, MKT-A10-F2-P1 |  |
| MKT-A11-F4-P2 | **Reliability badges**: Badges earned from the measures, shown on profiles and used in ranking. | interface | small | MKT-A11-F4-P1, MKT-A4-F3-P1 |  |
| MKT-A11-F4-P3 | **Limits for low standards**: Warnings, then listing limits, for sellers who fall below the published standards. | library | small | MKT-A11-F4-P1, KIT-A9-F4-P1 |  |
| MKT-A11-F5-P1 | **Product reviews**: Reviews of the product itself, separate from the trade rating, from buyers who bought it, with photos through the kit's file uploads, linked to the listing or catalogue record, on the kit's reviews. | data | small | KIT-A9-F2-P1, KIT-A15-F7-P1, MKT-A9-F1-P1, MKT-A3-F4-P2 |  |
| MKT-A11-F5-P2 | **Review display**: Product reviews on listing pages and shop pages, with the seller's reply. | interface | small | MKT-A11-F5-P1, MKT-A2-F1-P4 |  |
| MKT-A11-F6-P1 | **Market fake rating rules**: Rules for rings of accounts rating each other through fake sales, set on the kit's fake review checks. | library | small | KIT-A9-F8-P1, MKT-A11-F1-P1, MKT-A11-F5-P1 |  |

## MKT-A12 Safety, scams and banned items

Keeping the Market safe and legal: rules on banned and restricted items, scam detection, reports, fakes and rights owners, product recalls, known illegal images, and safety for in-person trades.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A12-F1 Know what can be sold | Know what may not be sold, and what needs a licence or an age check, before listing. | core | Restricted goods law; Item rules as data; Licence and age checks; Rules pages; Age check at handover; Second-hand dealer prompts |
| MKT-A12-F2 Stop scams | Be protected from fake listings, fake payment emails and account takeovers. | core | Market scam rules; Safe trading guide |
| MKT-A12-F3 Report a listing or member | Report a listing, member, review or shop page in two taps, with a reason. | core | Report links on everything |
| MKT-A12-F4 Known illegal images blocked | Have known illegal images blocked at upload, wherever in the Market they are uploaded. | core | Every upload checked |
| MKT-A12-F5 Fakes and rights owners | Let brand and copyright owners report fakes or copied photos, and let sellers answer. | core | Rights notices on listings and shops |
| MKT-A12-F6 Product recalls | Have listings of recalled products found and taken down, and buyers told. | next | Recall list link; Recall matching |
| MKT-A12-F7 Safer in-person trades | Share a pickup plan with a friend and get safety tips for meeting a stranger. | next | Share my pickup |
| MKT-A12-F8 Account takeover protection | Have unusual sign-ins and payout changes stopped before money moves. | core | Big change holds |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A12-F1-P1 | **Restricted goods law**: Every NZ law limiting online sales, with sources: firearms and restricted weapons, fireworks, alcohol, vapes, medicines, food (Food Act registration), live animals and pets, livestock (NAIT transfers), plants and seeds (biosecurity), wildlife, taonga tūturu (Protected Objects Act: registered collectors only), event tickets (resale bans for major events), publications that need classification labels, second-hand electrical goods, recalled goods, goods with compulsory safety standards and stolen goods. | legal | large |  |  |
| MKT-A12-F1-P2 | **Item rules as data**: Banned, restricted and licence-needed items written as rules linked to categories and keywords, versioned and voted on. | data | medium | MKT-A12-F1-P1, MKT-A3-F1-P1, KIT-A10-F5-P1, KIT-A12-F4-P4 |  |
| MKT-A12-F1-P3 | **Licence and age checks**: Checks a seller's licence, or a buyer's age, for items that need one, through the kit's age checks and public register checks. | library | small | KIT-A2-F8-P1, KIT-A2-F2-P1, KIT-A2-F13-P1, MKT-A12-F1-P2 |  |
| MKT-A12-F1-P4 | **Rules pages**: Plain pages listing what can and cannot be sold and why. | interface | small | MKT-A12-F1-P2 |  |
| MKT-A12-F1-P5 | **Age check at handover**: Checks age again when an R18 item changes hands, since a check at purchase alone lets parcels reach under-18s at the door: ID shown at pickup, or a courier that checks ID and takes a signature; until that courier service is linked (MKT-A8-F4-P8), R18 items are pickup only. | library | small | MKT-A12-F1-P3, MKT-A7-F1-P4 |  |
| MKT-A12-F1-P6 | **Second-hand dealer prompts**: Asks members who trade second-hand goods often enough to be presumed dealers (6 or more trading days, or over $2,000, in 12 months) for their dealer's licence, and shows it on their listings. | library | small | MKT-A21-F6-P3, MKT-A1-F4-P3 |  |
| MKT-A12-F2-P2 | **Market scam rules**: Signals specific to buying and selling, such as prices far below value, reused photos and a rush to move to another app, set as rules on the kit's fraud signals, which hold risky listings for review. | library | small | KIT-A10-F12-P2, MKT-A2-F1-P1 |  |
| MKT-A12-F2-P4 | **Safe trading guide**: Short plain advice on spotting scams, shown where people need it. | interface | small |  |  |
| MKT-A12-F3-P1 | **Report links on everything**: A report link on every listing, profile, message, review, shop page, blog post and storefront page, using the kit's report forms. | interface | small | KIT-A10-F3-P2, KIT-A10-F3-P1, KIT-A10-F3-P3, MKT-A17-F1-P1 |  |
| MKT-A12-F4-P1 | **Every upload checked**: Sends every Market upload, from listing, review and chat photos to shop page images, blog images and theme files, through the kit's file service and its known-content checks. | integration | small | KIT-A15-F7-P1, KIT-A10-F10-P2 |  |
| MKT-A12-F5-P1 | **Rights notices on listings and shops**: A notice link on every listing and shop page, and takedown and restore of Market listings, on the kit's copyright and trade mark notices, acted on promptly as the Copyright Act's rules for hosts require. | integration | small | KIT-A10-F21-P2, MKT-A2-F1-P4, MKT-A17-F1-P1 |  |
| MKT-A12-F6-P1 | **Recall list link**: Pulls government product recall lists, starting with New Zealand's; Spare Food needs food recalls too, so the feed should move down into the kit. | integration | medium | MKT-A12-F1-P2 |  |
| MKT-A12-F6-P2 | **Recall matching**: Matches recalls to listings and past orders by product, brand and model. | service | medium | MKT-A3-F4-P2, MKT-A4-F1-P2, MKT-A9-F1-P1 |  |
| MKT-A12-F7-P1 | **Share my pickup**: Sends the pickup time, place and listing to a chosen contact through the kit's trusted contact link, and checks in afterwards. | integration | small | KIT-A10-F2-P1, MKT-A8-F2-P1 |  |
| MKT-A12-F8-P1 | **Big change holds**: Holds big listing changes, such as mass edits or price changes, after a new-device sign-in until confirmed, alongside the kit's payout change guard. | service | small | KIT-A3-F3-P4, KIT-A1-F3-P3, KIT-A1-F3-P4, MKT-A2-F1-P2 |  |

## MKT-A13 Shop websites

Any business can run its own online shop on Spare Market at cost: its own web address and look, pages and blog, a fast and safe storefront found by web search, checkout with buyer protection, its products also in Market search, and a clean way to move out.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A13-F1 Open a shop | Turn a seller account into a shop with a name, logo, about page and policies. | next | Shop builder design; Shop record; Shop set-up guide; Shop page in the Market |
| MKT-A13-F2 Choose a look | Pick a theme, change colours, fonts and layout, and see changes before publishing. | next | Theme format; Starter themes; Visual editor; Theme add-ins |
| MKT-A13-F3 Use your own web address | Use your own domain name, with a secure certificate set up automatically and a storefront kept safe from card-skimming scripts. | next | Custom domains; Domain buying; Free shop address; Storefront security |
| MKT-A13-F4 Pages, blog and menus | Add pages, blog posts and menus to tell the shop's story. | next | Pages and blog; Menus |
| MKT-A13-F5 Fast shop pages found by web search | Have shop pages that load fast on a phone and show up well in web search. | next | Storefront server; Shop web search basics; Redirects from old addresses; Shop search and filters; Storefront API |
| MKT-A13-F6 Checkout on the shop's site | Let customers buy on the shop's own site with buyer protection, including guests, and a payout timing that suits a business. | next | Storefront checkout; Shopper accounts; Guest checkout decision; Held or normal payouts for shop sales; Disputes for guest buyers |
| MKT-A13-F7 Talk to shoppers | Answer shoppers' questions from a chat box on the shop's own site, in the same inbox as Market messages. | next | Storefront chat |
| MKT-A13-F8 Shop products in Market search | Have the shop's products appear in Spare Market search to reach more buyers. | next | Shared listings |
| MKT-A13-F9 Shop policies and legal pages | Publish returns, shipping, privacy and terms pages from plain templates that meet the law. | next | Policy templates; Accessibility statement |
| MKT-A13-F10 Move your shop out | Take the whole shop to another host in open formats, including its web address. | next | Whole-shop export; Domain move-out |
| MKT-A13-F11 Sell in other languages and countries | Show the shop's own product text, pages and blog in other languages, with prices set per country. | later | Translated shop content; Prices per country |
| MKT-A13-F12 Pay for hosting at cost | See what the shop's site costs to host each month and pay only that. | next | Hosting cost per shop; Monthly hosting bill |
| MKT-A13-F13 Work with others on a shop | Let an outside designer or bookkeeper work on the shop, hide the shop behind a password until launch, and see visitor counts without trackers. | later | Outside helpers; Password page before launch; Shop visitor counts |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A13-F1-P1 | **Shop builder design**: How a shop is set up and run end to end on the kit, replacing Shopify for a small business. The shop pages should move down into the kit, since Spare Food's direct ordering page needs the same (FOOD-A13-F4-P1). | research | large |  | [MK06](STATUS.md) |
| MKT-A13-F1-P2 | **Shop record**: A shop owning listings, a storefront, settings, staff and policies, on a seller or business account. | data | medium | MKT-A1-F1-P2, MKT-A2-F1-P1, MKT-A13-F1-P1, KIT-A1-F7-P1 |  |
| MKT-A13-F1-P3 | **Shop set-up guide**: A short guided set-up: name, logo, look, first products, shipping, payouts and policies. | interface | medium | MKT-A13-F1-P2 |  |
| MKT-A13-F1-P4 | **Shop page in the Market**: A shop page inside Spare Market with its products, ratings and about text. | interface | small | MKT-A13-F1-P2, MKT-A1-F2-P1 |  |
| MKT-A13-F2-P1 | **Theme format**: Our own open theme format of templates, blocks and settings, with a published specification. | library | large | MKT-A13-F1-P1 |  |
| MKT-A13-F2-P2 | **Starter themes**: A set of themes designed from scratch for different kinds of shop, all accessible and fast. | interface | large | MKT-A13-F2-P1, KIT-A19-F2-P1 |  |
| MKT-A13-F2-P3 | **Visual editor**: Drag blocks, change settings and preview on phone and desktop, with undo and published versions. | interface | large | MKT-A13-F2-P1, kit-ops |  |
| MKT-A13-F2-P4 | **Theme add-ins**: Themes and blocks from other designers, run as sandboxed add-ins and kept off checkout pages. | library | medium | KIT-A16-F5-P2, KIT-A16-F6-P2, MKT-A13-F2-P1, MKT-A13-F3-P4 |  |
| MKT-A13-F3-P1 | **Custom domains**: Connect a domain, check its records and issue free security certificates automatically; Spare Stay and Spare Social map their own web addresses too (STAY-A21-F3-P2, SOC-A1-F3-P1), so this should move down into the kit as one domain service. | service | medium | MKT-A13-F5-P1 |  |
| MKT-A13-F3-P2 | **Domain buying**: Whether the co-op can sell domain names at cost through a registrar, and the rules for .nz names. | research | small |  |  |
| MKT-A13-F3-P3 | **Free shop address**: A free web address for shops without a domain, on a separate domain registered on the Public Suffix List, so no shop can read another shop's or the Market's cookies. | service | small | MKT-A13-F5-P1, MKT-A13-F3-P4 |  |
| MKT-A13-F3-P4 | **Storefront security**: Serves shop sites apart from the Market's own domain, keeps theme and add-in scripts off checkout pages, and sets content security rules and script checks on every page that leads to payment, as the card industry's rules for payment pages ask. | library | medium | MKT-A13-F5-P1, KIT-A3-F1-P3, KIT-A3-F1-P6, KIT-A22-F5-P3 |  |
| MKT-A13-F4-P1 | **Pages and blog**: Pages and blog posts with text, images and product blocks. | service | medium | MKT-A13-F1-P2, engine-text, KIT-A15-F7-P1, MKT-A12-F4-P1 |  |
| MKT-A13-F4-P2 | **Menus**: Header and footer menus edited by drag and drop. | interface | small | MKT-A13-F2-P1 |  |
| MKT-A13-F5-P1 | **Storefront server**: Renders shop pages on the server from the theme and listings, cached close to buyers. | service | large | MKT-A13-F2-P1, MKT-A13-F1-P2, MKT-A4-F1-P2 |  |
| MKT-A13-F5-P2 | **Shop web search basics**: Uses the Market's server-rendered listing pages, product data markup and site maps (MKT-A2-F1-P5) for shop pages, with page titles, descriptions and tidy web addresses set per shop. | library | small | MKT-A13-F5-P1, MKT-A2-F1-P5 |  |
| MKT-A13-F5-P3 | **Redirects from old addresses**: Keeps old product and page links working after a shop moves in, so it keeps its web search ranking. | service | small | MKT-A13-F5-P1 |  |
| MKT-A13-F5-P4 | **Shop search and filters**: Search and filters inside a shop's own site. | interface | medium | MKT-A13-F5-P1, MKT-A4-F1-P2 |  |
| MKT-A13-F5-P5 | **Storefront API**: A public read-only API of a shop's products, collections, prices and stock, with cart and checkout hand-off, for shops that build their own front end, generated from the Market's command schemas. | library | medium | KIT-A16-F1-P1, KIT-A16-F1-P3, MKT-A16-F7-P1, MKT-A13-F6-P1 |  |
| MKT-A13-F6-P1 | **Storefront checkout**: The Market checkout styled to the shop's theme, on the shop's own address. | interface | medium | MKT-A5-F1-P3, MKT-A13-F5-P1 |  |
| MKT-A13-F6-P2 | **Shopper accounts**: Shoppers sign in with a Spareware login or, once the guest checkout decision allows it, check out as guests. | service | medium | KIT-A1-F1-P1, MKT-A13-F5-P1, MKT-A13-F6-P3 |  |
| MKT-A13-F6-P3 | **Guest checkout decision**: Whether shop sites take orders from guests with no account, weighing card fraud, anti-money-laundering duties and data rights; owned here until the kit opens one shared decision, since Spare Food waits on the same one (FOOD-A1-F1-P6). | legal | small | KIT-A4-F5-P1, KIT-A3-F13-P1, KIT-A18-F5-P1 |  |
| MKT-A13-F6-P4 | **Held or normal payouts for shop sales**: Decides whether sales on a shop's own site use held payments or pay out within days like other shop builders, since a business whose money waits for each delivery loses cash flow; the answer may depend on the shop's record. | research | small | KIT-A3-F2-P1, KIT-A3-F2-P2, KIT-A3-F3-P2, MKT-A7-F1-P2 |  |
| MKT-A13-F6-P5 | **Disputes for guest buyers**: Lets a buyer who checked out as a guest open a problem report and a dispute from a link in the receipt, and take part by email without a Spareware login. | library | small | KIT-A11-F1-P1, MKT-A13-F6-P2, MKT-A9-F4-P1 |  |
| MKT-A13-F7-P1 | **Storefront chat**: A chat box on the shop's site that feeds the seller's Market inbox. | interface | small | MKT-A10-F2-P1, MKT-A13-F5-P1 |  |
| MKT-A13-F8-P1 | **Shared listings**: One product shown in the shop and in the Market, with one stock count. | library | small | MKT-A13-F1-P2, MKT-A4-F1-P2, KIT-A5-F10-P1 |  |
| MKT-A13-F9-P1 | **Policy templates**: Our own plain-language policy templates, checked by a lawyer, filled from the shop's settings. | legal | medium | KIT-A18-F8-P1, KIT-A18-F5-P1, KIT-A18-F14-P1 |  |
| MKT-A13-F9-P2 | **Accessibility statement**: An accessibility statement for each shop as our own choice: the European Accessibility Act exempts microenterprises that provide services (fewer than 10 staff and no more than EUR 2 million turnover or balance sheet), which covers most shops; the statement says who provides the service, the shop for its own site or the Market for the shared checkout. | legal | small | KIT-A19-F2-P4 |  |
| MKT-A13-F10-P1 | **Whole-shop export**: Exports products and variants, images, pages, blog posts, redirects, discount codes and the theme in the open theme format, plus customers who agreed to it, as open files. | service | medium | MKT-A21-F1-P1, MKT-A13-F2-P1, MKT-A14-F1-P2, MKT-A14-F4-P1 |  |
| MKT-A13-F10-P2 | **Domain move-out**: Steps to point the shop's domain at a new host, with the old links redirected for a set time. | interface | small | MKT-A13-F3-P1, MKT-A13-F5-P3 |  |
| MKT-A13-F11-P1 | **Translated shop content**: The seller's product text, pages and blog posts in several languages, written by the seller or machine-translated through the kit and marked as such. | service | medium | MKT-A13-F4-P1, KIT-A19-F10-P2, KIT-A19-F1-P1 |  |
| MKT-A13-F11-P2 | **Prices per country**: Prices set per country, or worked out from one price, with the tax each country's rules ask for. | library | small | KIT-A3-F14-P1, KIT-A4-F7-P1, MKT-A13-F1-P2 |  |
| MKT-A13-F12-P1 | **Hosting cost per shop**: Measures each shop's storefront traffic, domain and storage, prices it at cost from the ledger, and shows the amount before the shop opens. | library | small | KIT-A13-F2-P3, KIT-A13-F3-P6, MKT-A13-F5-P1 |  |
| MKT-A13-F12-P2 | **Monthly hosting bill**: Charges each shop its hosting cost monthly through the kit's regular payments, with a receipt and notice before a site is paused for a failed payment. | integration | small | KIT-A3-F12-P1, KIT-A3-F12-P2, KIT-A15-F7-P5, MKT-A13-F12-P1 |  |
| MKT-A13-F13-P1 | **Outside helpers**: Time-limited access for an outside designer or bookkeeper to the theme, products or reports only, through the kit's delegated access. | library | small | KIT-A1-F8-P1, MKT-A13-F1-P2 |  |
| MKT-A13-F13-P2 | **Password page before launch**: Keeps a shop site behind a password page until the owner opens it. | interface | small | MKT-A13-F5-P1 |  |
| MKT-A13-F13-P3 | **Shop visitor counts**: Visits, top pages and sources for the shop site, counted in totals with no tracking cookies, through the kit's private usage counts. | service | small | KIT-A22-F8-P1, MKT-A13-F5-P1, MKT-A16-F3-P1 |  |

## MKT-A14 Shop selling tools

The tools a shop runs its business with: products and collections, one stock count across places, discounts, gift cards, customers and newsletters, subscriptions, wholesale, and selling on other channels.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A14-F1 Products and collections | Organise products into collections by hand or by rules such as tag, type or price. | next | Collections; Product spreadsheet import and export; Product tags and types; Custom product fields |
| MKT-A14-F2 One stock count across places | Keep one stock count across the online shop, Spare Market and the shop counter, by location. | next | Locations and transfers; Low stock alerts; Stock takes; Buying stock in |
| MKT-A14-F3 Discounts and sales | Run sales, discount codes, buy-some-get-some deals and free shipping offers. | next | Market discount types; Discount screens; Honest pricing checks |
| MKT-A14-F4 Customers and newsletters | Keep a customer list, see past orders, and email only people who agreed to hear from the shop. | next | Customer records; Email newsletters; Unfinished checkout reminders; Customer groups |
| MKT-A14-F5 Gift cards and store credit | Sell and redeem shop gift cards, and give store credit instead of a refund when the buyer agrees. | later | Shop gift cards |
| MKT-A14-F6 Subscriptions | Sell items that repeat every week or month, with the customer able to pause or cancel any time. | later | Shop subscriptions; Subscription self-service |
| MKT-A14-F7 Wholesale to other businesses | Give business customers their own prices, minimum orders and pay-later terms. | later | Business price lists; Wholesale invoices; Quotes and draft orders |
| MKT-A14-F8 Bundles and made-up products | Sell sets of items together, or products built from chosen parts. | later | Bundles |
| MKT-A14-F9 Sell on other channels | Keep product feeds for search and comparison sites up to date from one place. | later | Channel feeds |
| MKT-A14-F10 Sell live on video | Show items on a live video and let viewers buy them while they watch. | later | Live selling approach; Live selling stream |
| MKT-A14-F11 Layby | Offer layby: the buyer pays in parts and takes the item once it is paid off, with the rights the Fair Trading Act gives. | later | Layby law; Layby payments |
| MKT-A14-F12 Dropshipping and print on demand | Sell products a supplier makes or sends for you, with that shown to buyers. | later | Supplier-fulfilled products; Supplier shown to buyers |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A14-F1-P1 | **Collections**: Manual and rule-based groups of products for menus, pages and sales. | data | small | MKT-A2-F1-P1, MKT-A13-F1-P2 |  |
| MKT-A14-F1-P2 | **Product spreadsheet import and export**: Add or update products in bulk from an open spreadsheet format. | service | medium | MKT-A2-F1-P2, KIT-A5-F17-P2 |  |
| MKT-A14-F1-P3 | **Product tags and types**: Free tags and product types for organising and filtering. | data | small | MKT-A2-F1-P1 |  |
| MKT-A14-F1-P4 | **Custom product fields**: Shop-defined extra fields on products, shown by themes and readable by add-ins. | data | small | MKT-A2-F1-P1, MKT-A13-F2-P1 |  |
| MKT-A14-F2-P2 | **Locations and transfers**: Shops, stores and storage places, with stock moved between them. | data | medium | KIT-A5-F10-P1 |  |
| MKT-A14-F2-P3 | **Low stock alerts**: Alerts when stock falls below a set level. | service | small | KIT-A5-F10-P1, KIT-A8-F3-P2 |  |
| MKT-A14-F2-P4 | **Stock takes**: Count stock with a phone scanner and fix differences, each fix logged. | interface | medium | KIT-A5-F10-P1, MKT-A3-F4-P1 |  |
| MKT-A14-F2-P5 | **Buying stock in**: Purchase orders to suppliers and receiving stock against them. | interface | medium | KIT-A5-F10-P1 |  |
| MKT-A14-F3-P1 | **Market discount types**: Percent, amount, buy-some-get-some and free shipping discounts, with dates, limits and who can use them, as rules on the kit's promotions and vouchers. | library | small | KIT-A5-F14-P2, MKT-A5-F1-P3 |  |
| MKT-A14-F3-P2 | **Discount screens**: Create, schedule and end discounts, and see how they did. | interface | small | MKT-A14-F3-P1 |  |
| MKT-A14-F3-P3 | **Honest pricing checks**: Checks that a was-price was a real recent price, as the Fair Trading Act requires. | legal | small | KIT-A5-F14-P1, KIT-A18-F8-P1, MKT-A14-F3-P1 |  |
| MKT-A14-F4-P1 | **Customer records**: A shop's customers with their orders and consent, kept to what the Privacy Act allows; restaurants, hosts and trades need the same list, so it should move down into the kit. | data | medium | MKT-A9-F1-P1, MKT-A21-F3-P5, KIT-A18-F5-P1, KIT-A18-F17-P1 |  |
| MKT-A14-F4-P2 | **Email newsletters**: Simple newsletters with consent and a working unsubscribe, as anti-spam law requires; they should move down into the kit with the customer list. | service | medium | KIT-A8-F4-P1, KIT-A8-F5-P2, MKT-A14-F4-P1 |  |
| MKT-A14-F4-P3 | **Unfinished checkout reminders**: A reminder about a left cart, only for shoppers who agreed to messages. | service | small | KIT-A8-F3-P2, KIT-A8-F5-P2, MKT-A5-F5-P1, MKT-A14-F4-P1 |  |
| MKT-A14-F4-P4 | **Customer groups**: Groups such as regulars or wholesale, used by discounts and price lists. | data | small | MKT-A14-F4-P1 |  |
| MKT-A14-F5-P1 | **Shop gift cards**: Shop-branded gift cards and store credit on the kit's gift cards, redeemable online and at the till, with balances counted in the closure checks. | library | small | KIT-A3-F15-P2, MKT-A13-F1-P2 |  |
| MKT-A14-F6-P1 | **Shop subscriptions**: Products that repeat every week or month on the kit's regular payments and repeat orders, with stock set aside for each delivery. | library | small | KIT-A3-F12-P1, KIT-A5-F9-P1, MKT-A9-F1-P1 |  |
| MKT-A14-F6-P2 | **Subscription self-service**: Customers skip, pause, change or cancel in a few taps. | interface | medium | MKT-A14-F6-P1, KIT-A1-F1-P1 |  |
| MKT-A14-F7-P1 | **Business price lists**: Prices and minimums per business customer or group. | data | medium | MKT-A13-F1-P2, MKT-A14-F4-P1 |  |
| MKT-A14-F7-P2 | **Wholesale invoices**: Invoices due in set days, with reminders and part payments, on the kit's business accounts on credit; invoices with payment terms should move down into the kit, since trades and restaurants need them too. | library | small | KIT-A3-F17-P1, KIT-A4-F2-P1, MKT-A14-F7-P1 |  |
| MKT-A14-F7-P3 | **Quotes and draft orders**: Build an order for a customer and send a link to pay. | interface | medium | MKT-A9-F1-P1 |  |
| MKT-A14-F8-P1 | **Bundles**: A bundle with its own price that takes stock from its parts. | data | small | MKT-A2-F7-P1, KIT-A5-F10-P1 |  |
| MKT-A14-F9-P2 | **Channel feeds**: Keeps product feeds for search and comparison sites up to date. | integration | medium | MKT-A4-F9-P2 |  |
| MKT-A14-F10-P1 | **Live selling approach**: Whether to build live selling on the shared media engine or on a live video service in the kit, never on another program, and what it costs to run. | research | small |  |  |
| MKT-A14-F10-P2 | **Live selling stream**: A live video with listings pinned on screen, quick buying and a chat for questions. | service | large | engine-media, MKT-A2-F1-P1, MKT-A5-F1-P1, MKT-A10-F2-P1 |  |
| MKT-A14-F11-P1 | **Layby law**: The Fair Trading Act's layby rules: what the agreement must say, the buyer's right to cancel, what a seller may keep, and what happens if the shop closes. | legal | small | KIT-A18-F8-P1 |  |
| MKT-A14-F11-P2 | **Layby payments**: Payments in parts on the kit's stage payments, with the item set aside in stock until it is paid off or the layby is cancelled, online or at the till. | library | small | KIT-A3-F2-P3, KIT-A5-F10-P1, MKT-A14-F11-P1 |  |
| MKT-A14-F12-P1 | **Supplier-fulfilled products**: Products made or sent by a supplier or print partner, with the order passed to them and the tracking brought back. | library | medium | MKT-A9-F1-P1, MKT-A2-F9-P1, KIT-A16-F5-P2 |  |
| MKT-A14-F12-P2 | **Supplier shown to buyers**: Shows on the listing that a supplier makes or sends the item, with its delivery time, in line with the making declaration. | interface | small | MKT-A2-F9-P1, MKT-A8-F1-P2 |  |

## MKT-A15 Till for physical shops

A shop counter on the kit's card readers and shared till, on a phone, tablet or computer, sharing stock with the online shop and the Market and working when the internet drops.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A15-F1 Sell at the counter | Ring up sales, take card or cash, and print or email a receipt. | later | Till research; Market till screens; Cash and cash counts; Counter returns and exchanges |
| MKT-A15-F2 Keep selling offline | Keep taking sales when the internet drops, and catch up when it is back. | later | Offline till; Offline card payments |
| MKT-A15-F3 Staff and shifts | Give staff a quick PIN, limit what they can do, and close the day with a count. | later | End of day report |
| MKT-A15-F4 Barcodes and price labels | Scan barcodes to sell and print price labels. | later | Barcode selling; Price label printing |
| MKT-A15-F5 Order online, collect in store | Let customers order online and collect from the shop. | later | Click and collect |
| MKT-A15-F6 Market stalls and pop-ups | Take payments at a weekend market stall with just a phone. | later | Stall mode |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A15-F1-P1 | **Till research**: Card readers and point-of-sale options from payment providers that fit at-cost pricing, including NZ EFTPOS through the local switch and the in-store card surcharge rules, as the Market's input to the kit's card readers and shared till. | research | medium | KIT-A3-F4-P3, KIT-A3-F19-P2 | [MK07](STATUS.md) |
| MKT-A15-F1-P2 | **Market till screens**: The shop's products, stock, discounts and returns on the kit's shared till, on a phone, tablet or computer. | interface | medium | KIT-A3-F19-P2, KIT-A5-F10-P1, MKT-A14-F3-P1, MKT-A15-F1-P1 |  |
| MKT-A15-F1-P4 | **Cash and cash counts**: Cash payments, change, opening float and cash drawer counts. | interface | small | MKT-A15-F1-P2 |  |
| MKT-A15-F1-P6 | **Counter returns and exchanges**: Refund or swap an item at the counter, online or in-store purchases alike. | service | small | MKT-A9-F5-P2, MKT-A15-F1-P2 |  |
| MKT-A15-F2-P1 | **Offline till**: Sales and stock changes saved on the device and synced in order once back online. | library | medium | kit-sync, MKT-A15-F1-P2, KIT-A5-F10-P1 |  |
| MKT-A15-F2-P2 | **Offline card payments**: What the payment provider allows for card payments with no connection, and the risk. | research | small | MKT-A15-F1-P1 |  |
| MKT-A15-F3-P2 | **End of day report**: Sales, refunds, payments and cash for the day, matched to the payment provider. | service | small | MKT-A15-F1-P2, engine-pdf, KIT-A3-F9-P1 |  |
| MKT-A15-F4-P1 | **Barcode selling**: Scan with a camera or a plug-in scanner to add items to a sale. | interface | small | MKT-A3-F4-P1, MKT-A15-F1-P2 |  |
| MKT-A15-F4-P2 | **Price label printing**: Price and barcode labels from product records. | library | small | engine-pdf, MKT-A8-F4-P5 |  |
| MKT-A15-F5-P1 | **Click and collect**: A pickup choice at checkout, a ready message, and a collect step at the till. | service | small | MKT-A9-F1-P1, KIT-A5-F10-P1, MKT-A15-F1-P2 |  |
| MKT-A15-F6-P2 | **Stall mode**: A cut-down till for a stall: few products, quick sales, works offline. | interface | small | MKT-A15-F1-P2 |  |

## MKT-A16 Seller tools, insights and links

Running a busy selling account: a dashboard of what needs doing, bulk tools, sales insights, sold prices, accounting links, add-ins, the open command API for AI agents, and moving in from other platforms.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A16-F1 Seller dashboard | See what needs doing today: orders to send, questions to answer, offers to reply to. | core | Seller dashboard |
| MKT-A16-F2 Bulk listing and edits | List or change hundreds of items at once, from a spreadsheet or in a table. | next | Bulk edit table; Bulk changes as one operation |
| MKT-A16-F3 Sales insights | See views, watchers, offers and sales over time, and where buyers came from. | next | Seller analytics; Insight screens |
| MKT-A16-F4 Sold prices | Look up what similar items sold for. | next | Sold price data; Sold price search; Open price data |
| MKT-A16-F5 Link accounting software | Send sales, fees, refunds and payouts to accounting software. | next | Accounting connectors; Which accounting software first |
| MKT-A16-F6 Add-ins | Install add-ins that extend a shop or the selling tools, with clear permissions. | next | Market add-in points |
| MKT-A16-F7 AI agents and scripts | Let an AI agent or a script list, reprice, answer questions and dispatch through the same commands as the app. | core | Market command schemas; Agent safeguards in Market commands |
| MKT-A16-F8 Set up automations | Have routine jobs done automatically, such as tagging big orders, messaging buyers or reordering stock. | later | Automation rules; Automation screens |
| MKT-A16-F9 Move in from another platform | Bring listings and products over from another marketplace or shop builder using your own export files. | next | Selling in both places |
| MKT-A16-F10 Connect your own systems | Get order and stock events in your own systems, and test them without real money. | next | Market webhook events; Market test mode |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A16-F1-P1 | **Seller dashboard**: One page of to-dos across listings, orders, offers, questions and disputes. | interface | medium | MKT-A9-F1-P1, MKT-A2-F1-P1, MKT-A5-F2-P1 |  |
| MKT-A16-F2-P1 | **Bulk edit table**: Change price, stock, shipping or category for many listings at once. | interface | medium | MKT-A2-F1-P2 |  |
| MKT-A16-F2-P2 | **Bulk changes as one operation**: Bulk changes grouped so they can be checked first and undone together. | library | small | kit-ops, MKT-A2-F1-P2 |  |
| MKT-A16-F3-P1 | **Seller analytics**: Counts of views, watchers, offers and sales per listing, without tracking buyers across sites. | service | medium | MKT-A2-F1-P1, MKT-A9-F1-P1, KIT-A22-F8-P1 |  |
| MKT-A16-F3-P2 | **Insight screens**: Charts and tables of the counts, with downloads. | interface | medium | MKT-A16-F3-P1 |  |
| MKT-A16-F4-P1 | **Sold price data**: Final prices of completed sales, kept without buyer names, searchable by item. | data | medium | MKT-A9-F1-P1, MKT-A4-F1-P2 |  |
| MKT-A16-F4-P2 | **Sold price search**: Search sold items and see a price range. | interface | small | MKT-A16-F4-P1 |  |
| MKT-A16-F4-P3 | **Open price data**: Whether to publish summarised sold prices as open data for members and researchers. | research | small | MKT-A16-F4-P1 |  |
| MKT-A16-F5-P2 | **Accounting connectors**: Sends Market sales, fees, refunds and payouts to accounting software through the kit's common connectors. | integration | small | KIT-A16-F8-P1, KIT-A4-F4-P2, MKT-A16-F5-P3 |  |
| MKT-A16-F5-P3 | **Which accounting software first**: The accounting software NZ small sellers use most, and the terms of their APIs. | research | small |  |  |
| MKT-A16-F6-P1 | **Market add-in points**: The places add-ins can hook in: listings, orders, checkout, shipping, storefront and reports. | library | medium | KIT-A16-F5-P2, MKT-A16-F7-P1 |  |
| MKT-A16-F7-P1 | **Market command schemas**: Every Market action described once as a command and query on the kit's command schemas, so the app, AI agents, add-ins and scripts share one path and the kit generates the API from it. | library | medium | KIT-A15-F1-P3, KIT-A16-F1-P1 |  |
| MKT-A16-F7-P2 | **Agent safeguards in Market commands**: Wires the kit's agent safeguards into the Market: approval before an agent buys, offers, bids or refunds; agent labels on offers, questions, answers and messages; ratings and dispute statements made by people only; listing and chat text marked untrusted; and spending limits per agent. | library | medium | KIT-A17-F1-P1, KIT-A17-F2-P1, KIT-A17-F9-P1, KIT-A17-F9-P2, KIT-A17-F5-P1, KIT-A17-F4-P1, MKT-A16-F7-P1 |  |
| MKT-A16-F8-P1 | **Automation rules**: When something happens, such as a new order or low stock, runs a set action through the command API; restaurants, hosts and trades need the same, so automations should move down into the kit. | library | medium | kit-ops, MKT-A16-F7-P1 |  |
| MKT-A16-F8-P2 | **Automation screens**: Build, test and switch rules on and off, with a log of what each rule did. | interface | medium | MKT-A16-F7-P1 |  |
| MKT-A16-F9-P3 | **Selling in both places**: How sellers can keep listings on another site while trying the Market, within that site's terms. | research | medium |  |  |
| MKT-A16-F10-P1 | **Market webhook events**: Order paid, dispatched, delivered and refunded, offer received and stock low, sent as signed webhooks through the kit. | library | small | KIT-A16-F2-P1, MKT-A9-F1-P2, MKT-A5-F2-P1, MKT-A14-F2-P3 |  |
| MKT-A16-F10-P2 | **Market test mode**: Test listings, test orders and test payments in the kit's test mode, kept out of search and the cost ledger. | library | small | KIT-A16-F3-P3, MKT-A2-F1-P1 |  |

## MKT-A17 Moderation, support and administration

Running the Market fairly in the open on the kit's trust, support and admin services: what can be reported and moderated, help articles, admin views, Police requests and moderator seats.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A17-F1 Moderation queue | Moderators review reported and flagged listings, messages, members, shop pages and reviews, with the rule cited for each decision. | core | Market report types |
| MKT-A17-F3 Help and support | Find answers in a help centre and reach a person when stuck. | core | Market help articles |
| MKT-A17-F4 Admin console | Staff see and fix accounts, listings, orders and payments, with every action logged. | core | Market admin views; Settings and rule switches |
| MKT-A17-F5 Requests from authorities | Have requests from Police and regulators, such as questions about stolen goods, answered lawfully from the first week, and see how many there were. | core | Market requests from authorities |
| MKT-A17-F6 Moderators chosen by members | Members choose moderators and panel members for the Market, trained and supported through the kit. | next | Market moderator and panel seats |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A17-F1-P1 | **Market report types**: What the kit's one review queue receives from the Market (listings, messages, members, shop pages, blog posts, theme files, review photos and chat photos), each with the rule it may break and its urgency. | library | small | KIT-A10-F6-P2, KIT-A10-F5-P1, MKT-A12-F1-P2 |  |
| MKT-A17-F3-P1 | **Market help articles**: The Market's articles in the kit's help centre, in plain language, linked from the screens where people get stuck. | data | small | KIT-A21-F1-P1 |  |
| MKT-A17-F4-P1 | **Market admin views**: Views of members, listings, orders, held payments and shops on the kit's admin console, each action an operation with a stated reason. | interface | medium | KIT-A22-F6-P1, MKT-A9-F1-P1 |  |
| MKT-A17-F4-P3 | **Settings and rule switches**: Change Market settings and rules from voted results, with history. | service | small | KIT-A12-F4-P1, MKT-A17-F4-P1, KIT-A22-F2-P2 |  |
| MKT-A17-F5-P1 | **Market requests from authorities**: How the Market answers Police about stolen goods and gives Police the second-hand dealer records the law may require, under the kit's published process for law enforcement requests. | legal | small | KIT-A10-F9-P3, MKT-A21-F6-P3 |  |
| MKT-A17-F6-P1 | **Market moderator and panel seats**: How many moderators and panel members the Market needs, their terms and who elects them, run on the kit's elections. | library | small | KIT-A12-F6-P1, KIT-A14-F6-P1 |  |

## MKT-A18 Members, fees and the cost ledger

What makes the Market a co-op: buyers and sellers join as members and vote on fees and rules from the first release, see each month what it cost to run and how the Market is doing, and help get their own town started.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A18-F1 See what it cost | See each month what the Market cost to run and how the fee was set from it. | core | Market cost page |
| MKT-A18-F2 Get a town started | Help a town get enough sellers and buyers at once for the Market to be useful there. | core | Launch plan; Invites and local launch page |
| MKT-A18-F3 Join the co-op | Become a member with one share and a vote on the Market's fees and rules, from the first release. | core | Market electorate; Start-up rules |
| MKT-A18-F4 Vote on fees and rules | Propose and vote on changes to fees, ranking, item rules and protection limits. | core | Market proposals |
| MKT-A18-F5 Community buy-and-sell groups | Run a buy-and-sell group for a school, club or suburb inside the Market. | later | Group marketplaces; Fundraising sales |
| MKT-A18-F6 See how the Market is doing | See each month, region by region, how the Market is doing: listings, sales, time to sell, disputes, fraud losses and support times. | core | Monthly Market figures |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A18-F1-P1 | **Market cost page**: The Market's monthly costs, jobs and resulting fee, from the open cost ledger. | interface | small | KIT-A13-F1-P3, KIT-A13-F3-P2 |  |
| MKT-A18-F2-P1 | **Launch plan**: How marketplaces got their first users, which town or community to start in, and how to beat a strong incumbent there. | research | medium |  |  |
| MKT-A18-F2-P2 | **Invites and local launch page**: Invite links, a local page showing who is selling nearby, and help for community groups to move over. | interface | small | MKT-A4-F1-P2, MKT-A1-F1-P2, KIT-A14-F7-P1 |  |
| MKT-A18-F3-P2 | **Market electorate**: Who votes on the Market's fees and rules (sellers, buyers, shops as businesses and their staff), whether a business has one vote or each person does, and what buyer-members vote on, set on the kit's electorates. | legal | small | KIT-A12-F5-P2, KIT-A14-F1-P4, KIT-A14-F1-P5 |  |
| MKT-A18-F3-P3 | **Start-up rules**: How the first fee, ranking rules and item rules are set before there are members to vote, from the kit's start-up fee rule, and when the first member vote replaces them. | legal | small | KIT-A13-F3-P4, KIT-A12-F5-P2, MKT-A18-F3-P2 |  |
| MKT-A18-F4-P1 | **Market proposals**: What Market members may propose (fees, ranking, item rules and protection limits), with quorum and timing, on the kit's proposals service. | library | small | KIT-A12-F1-P1, MKT-A18-F3-P2 |  |
| MKT-A18-F5-P1 | **Group marketplaces**: Listings visible to a group's members, with group admins and group rules. | service | medium | MKT-A1-F1-P2, MKT-A4-F1-P2, MKT-A17-F1-P1, KIT-A6-F1-P4 |  |
| MKT-A18-F5-P2 | **Fundraising sales**: Group sales where the proceeds go to the school or club. | service | small | MKT-A7-F3-P1 |  |
| MKT-A18-F6-P1 | **Monthly Market figures**: Active listings, sell-through, time to sale, dispute and fraud loss rates and support response times, by region each month, published as open data through the kit's public reporting pipeline. | service | small | KIT-A13-F9-P1, MKT-A9-F1-P1, MKT-A11-F2-P2 |  |

## MKT-A19 Countries, languages, currencies and tax

New Zealand first, international by design: English and te reo Māori from the start, GST worked out right, and each new country added as a rule set for its language, currency, tax, consumer law and platform duties.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A19-F1 Use it in your language | Use the Market in English or te reo Māori, with more languages added by members. | core | Te reo Māori translation; Next languages |
| MKT-A19-F2 GST worked out right | Have GST worked out and shown correctly on every sale and fee. | core | Market GST rules; GST threshold warning |
| MKT-A19-F3 Overseas sellers to NZ buyers | Buy from overseas sellers with GST collected at checkout, as the law asks of marketplaces. | later | Low-value import GST |
| MKT-A19-F4 Prices in your currency | See prices in your own currency, while sellers are paid in theirs. | later | Prices shown in your currency |
| MKT-A19-F5 Rules for each country | Sell in another country with its consumer, safety, tax and platform rules built in. | later | EU platform duties; Australia and UK duties; Business seller checks for the EU |
| MKT-A19-F6 Income reporting | Have the Market report sellers' income to tax authorities wherever the law requires it. | later | Goods reporting watch |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A19-F1-P2 | **Te reo Māori translation**: The Market's interface text in te reo Māori, done with fluent speakers through the kit's member translation, macrons always right. | data | medium | KIT-A19-F1-P1, KIT-A19-F1-P3, KIT-A18-F16-P1 |  |
| MKT-A19-F1-P5 | **Next languages**: Chooses the next languages from New Zealand's most spoken, starting with Samoan, Mandarin and Hindi, and finds fluent members to translate and review them. | research | small | KIT-A19-F1-P3, MKT-A19-F1-P2 |  |
| MKT-A19-F2-P1 | **Market GST rules**: When GST applies to goods sales, the buyer fee and shipping, for registered and unregistered sellers, and the marketplace rules for low-value imported goods, as the Market's input to the kit's GST rules. | legal | medium | KIT-A4-F1-P1 |  |
| MKT-A19-F2-P4 | **GST threshold warning**: Warns a seller whose sales look like a taxable activity as they near the NZ$60,000 GST registration threshold across every Spareware app, and explains what registering changes; Spare Food maps the same warning (FOOD-A10-F13-P1), so it should move down into the kit. | library | small | KIT-A4-F1-P2, KIT-A3-F3-P3, MKT-A19-F2-P1 |  |
| MKT-A19-F3-P1 | **Low-value import GST**: Collects and returns GST on imported goods of NZ$1,000 or less sold through the Market by overseas sellers. | service | medium | KIT-A4-F1-P2, MKT-A19-F2-P1, MKT-A7-F9-P1 |  |
| MKT-A19-F4-P2 | **Prices shown in your currency**: Shows a converted price as a guide on listings, and the exact amount charged at checkout, on the kit's currencies per country. | integration | small | KIT-A3-F14-P1 |  |
| MKT-A19-F5-P2 | **EU platform duties**: Trader checks, compliance by design and product safety duties for marketplaces selling in the EU. | legal | medium | KIT-A18-F13-P1 |  |
| MKT-A19-F5-P3 | **Australia and UK duties**: Consumer, tax and platform duties for marketplaces in Australia and the UK. | legal | medium | KIT-A18-F13-P1 |  |
| MKT-A19-F5-P4 | **Business seller checks for the EU**: Collect and check business sellers' details before they sell to EU buyers. | service | medium | KIT-A2-F7-P2, MKT-A1-F4-P1, MKT-A19-F5-P2 |  |
| MKT-A19-F6-P1 | **Goods reporting watch**: Tracks when platform income reporting starts to cover sales of goods, in New Zealand or another country, for the kit's platform reporting rules; IRD has not brought in the OECD extension for goods in New Zealand. | research | small | KIT-A4-F3-P1 |  |

## MKT-A20 Access, phones and offline

Working for everyone, everywhere: screen readers, keyboards and large text, an installable phone app with camera and notifications, poor connections and offline drafts, and a simple mode.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A20-F1 Works for everyone | Use every part with a screen reader, keyboard, voice control or large text. | core | Photo descriptions; Time limits people can extend |
| MKT-A20-F2 On your phone | Install the Market on a phone from the browser or an app store, with camera, location and notifications. | core | Market phone app |
| MKT-A20-F3 Works on a poor connection | Browse, draft listings and read messages offline or on slow mobile data. | next | Offline cache |
| MKT-A20-F4 Simple mode | Use a simple mode with bigger buttons and fewer choices. | later | Simple mode |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A20-F1-P4 | **Photo descriptions**: Prompts sellers to describe listing photos, on the kit's image descriptions. | interface | small | KIT-A19-F2-P3, MKT-A2-F2-P1 |  |
| MKT-A20-F1-P5 | **Time limits people can extend**: Warns before a checkout hold, an offer or an auction runs out and lets people extend it, or bid automatically, as WCAG 2.2 success criterion 2.2.1 asks. | library | small | MKT-A5-F1-P1, MKT-A5-F2-P1, KIT-A19-F2-P1 |  |
| MKT-A20-F2-P2 | **Market phone app**: The Market in the kit's installable app, using the camera for photos and scanning. | interface | medium | KIT-A20-F2-P2, KIT-A19-F2-P1 |  |
| MKT-A20-F3-P1 | **Offline cache**: Recently viewed listings, drafts and messages kept on the device and synced when back online. | library | medium | kit-sync, KIT-A15-F4-P2 |  |
| MKT-A20-F4-P1 | **Simple mode**: A cut-down layout for buying and selling basics, for people new to online trading. | interface | medium | KIT-A19-F2-P1 |  |

## MKT-A21 Data, privacy and legal duties

Members own their data and can take it or delete it, on the kit's data, privacy and legal parts; the Market adds its own rules on top: who sees a buyer's details, checks before an account closes, consumer law in the product, its terms and the records second-hand dealer law asks for.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A21-F1 Take your data | Download all your listings, photos, orders, messages and ratings in open formats, with your order history as a spreadsheet. | core | Market data export |
| MKT-A21-F2 Delete your account and data | Delete your account and personal data, keeping only what the law makes us keep. | core | Deletion checks |
| MKT-A21-F3 Privacy controls | See what the Market holds about you and who can see it, and know sellers may use your details only for your order. | core | Buyer details for sellers |
| MKT-A21-F4 Consumer law built in | Trade knowing the Market and its sellers meet consumer law without needing to know it. | core | Law to product rules |
| MKT-A21-F5 Plain terms | Read short, plain terms of trade and rules, with changes voted on and shown before they apply. | core | Market terms and rules; Rule change notices |
| MKT-A21-F6 Records the law needs | Have the records tax, anti-money-laundering, consumer and second-hand dealer law ask for kept for as long as they ask. | core | Market records the law needs; Second-hand dealer law |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A21-F1-P1 | **Market data export**: Everything a member has in the Market (listings, photos, orders, also as a spreadsheet, messages and ratings) on the kit's data export. | library | small | KIT-A18-F1-P1 |  |
| MKT-A21-F2-P1 | **Deletion checks**: Runs the closure checks for open orders, held money, disputes and gift card balances before the kit's account deletion, which keeps only the records the law requires. | library | small | MKT-A1-F8-P2, KIT-A18-F2-P1, KIT-A18-F6-P1 |  |
| MKT-A21-F3-P5 | **Buyer details for sellers**: A private seller sees a buyer's name and address only for that order, may not reuse them, and loses access once the order closes; shops keep customer details only as their customer list's consent allows. | library | small | MKT-A9-F1-P2, MKT-A8-F2-P5, KIT-A18-F5-P1 |  |
| MKT-A21-F4-P2 | **Law to product rules**: Turns the kit's consumer law duties into rules in the Market: in-trade labels, guarantees shown, unfair claims flagged and honest pricing. | library | medium | KIT-A18-F8-P1, MKT-A1-F4-P1 |  |
| MKT-A21-F5-P1 | **Market terms and rules**: The Market's terms for buyers, sellers and shops in plain words, checked by a lawyer, on the kit's terms and policy versions. | legal | small | KIT-A18-F4-P1, KIT-A18-F4-P2, KIT-A18-F8-P1 |  |
| MKT-A21-F5-P2 | **Rule change notices**: Tells members about voted fee and rule changes before they apply, with a summary of what changed and when. | service | small | KIT-A8-F8-P1, KIT-A12-F4-P2, MKT-A18-F4-P1 |  |
| MKT-A21-F6-P1 | **Market records the law needs**: Which Market records tax, anti-money-laundering, consumer and second-hand dealer law ask for, and for how long, kept through the kit's financial record keeping and retention schedules. | legal | small | KIT-A4-F6-P1, KIT-A18-F6-P1, MKT-A21-F6-P3 |  |
| MKT-A21-F6-P3 | **Second-hand dealer law**: What the Secondhand Dealers and Pawnbrokers Act 2004 asks of the Market: whether it counts as an internet auction provider, which must record every person offering second-hand goods (name, date of birth, phone and email), keep each sale's details for 12 months from when bids close and let Police see them; and when in-trade second-hand sellers need a dealer's licence, presumed after 6 or more trading days, or over $2,000, in 12 months. | legal | medium |  |  |

## MKT-A22 Vehicles and other big items

Cars, motorbikes, boats, caravans and machinery need their own details, checks and paperwork: plate lookup, money owing checks, the notices dealers must give, viewings and test drives, and the change of ownership.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| MKT-A22-F1 List a vehicle by its plate | Enter a number plate and have the make, model, year and registration details filled in. | next | Plate lookup; Vehicle fields; Warrant of fitness rule |
| MKT-A22-F2 Check for money owing | Check whether money is owed on a vehicle, or it is reported stolen, before buying. | next | Money owing check; Stolen vehicle check; Vehicle check report |
| MKT-A22-F3 Dealers on the Market | Let registered motor vehicle traders list their stock with the notices the law requires. | later | Vehicle trader law; Trader register check; Dealer notices; Dealer stock feed |
| MKT-A22-F4 Viewings and test drives | Book a viewing or test drive, with safety tips and a check that the driver is licensed. | later | Viewing booking; Test drive safety |
| MKT-A22-F5 Change of ownership | Get guided through the official change of ownership and a written sale agreement. | next | Ownership change guide; Sale agreement |
| MKT-A22-F6 Boats, caravans and machinery | List boats, caravans, trailers and machinery with their own details and checks. | later | Fields for other vehicles; Freight and towing quotes |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MKT-A22-F1-P2 | **Plate lookup**: Fills the listing's make, model, year and registration details from the kit's vehicle data after the seller enters the plate. | integration | small | KIT-A2-F6-P2, MKT-A3-F2-P1 |  |
| MKT-A22-F1-P3 | **Vehicle fields**: Odometer, warrant of fitness or certificate of fitness expiry, registration expiry, road user charges status for diesel and electric vehicles, fuel, transmission and import history. | data | small | MKT-A3-F2-P1 |  |
| MKT-A22-F1-P4 | **Warrant of fitness rule**: Tells sellers a sold vehicle needs a warrant of fitness issued within one month before delivery, unless the buyer agrees in writing, with that waiver in the sale agreement, and shows the date to buyers. | library | small | MKT-A22-F1-P3, MKT-A12-F1-P2, MKT-A22-F5-P2 |  |
| MKT-A22-F2-P1 | **Money owing check**: Searches the Personal Property Securities Register for finance against the vehicle, at the register's own cost. | integration | medium | MKT-A22-F1-P3 |  |
| MKT-A22-F2-P2 | **Stolen vehicle check**: Checks the plate or VIN against the public list of stolen vehicles. | integration | small | MKT-A22-F1-P3 |  |
| MKT-A22-F2-P3 | **Vehicle check report**: One page showing the check results and their date, shared with the buyer. | interface | small | engine-pdf, MKT-A22-F2-P1, MKT-A22-F2-P2 |  |
| MKT-A22-F3-P1 | **Vehicle trader law**: The Motor Vehicle Sales Act and consumer information rules: who must register as a trader, and the notices buyers must get. | legal | medium | KIT-A18-F8-P1 |  |
| MKT-A22-F3-P2 | **Trader register check**: Checks a dealer against the public Motor Vehicle Traders Register. | integration | small | MKT-A22-F3-P1, MKT-A1-F4-P1 |  |
| MKT-A22-F3-P3 | **Dealer notices**: The required information notices filled from the listing and shown and signed before sale. | library | small | engine-pdf, MKT-A22-F3-P1 |  |
| MKT-A22-F3-P4 | **Dealer stock feed**: Keeps a dealer's stock in step from their own system through a standard file or the command API. | integration | medium | MKT-A14-F1-P2, MKT-A16-F7-P1 |  |
| MKT-A22-F4-P1 | **Viewing booking**: Pick a time from the seller's free slots, with reminders and the address shared only once booked. | interface | small | MKT-A10-F2-P1, KIT-A5-F5-P1, KIT-A7-F10-P1 |  |
| MKT-A22-F4-P2 | **Test drive safety**: What sellers should check before a test drive, such as the driver licence and insurance, and how to do it safely. | research | small |  |  |
| MKT-A22-F5-P1 | **Ownership change guide**: Step-by-step help and reminders for both sides to complete the official online change of ownership. | interface | small | KIT-A8-F3-P2, MKT-A9-F1-P1 |  |
| MKT-A22-F5-P2 | **Sale agreement**: Our own plain sale agreement for private vehicle sales, with the buyer's written warrant of fitness waiver where it applies, filled from the listing and signed through the kit's signatures. | library | small | KIT-A25-F1-P2, KIT-A18-F8-P1, MKT-A22-F1-P3 |  |
| MKT-A22-F6-P1 | **Fields for other vehicles**: Hours, hull and engine details, towing weights and certificates for each kind. | data | small | MKT-A3-F2-P1 |  |
| MKT-A22-F6-P2 | **Freight and towing quotes**: Ask freight firms and towing services for quotes to move a big item. | integration | medium | KIT-A16-F5-P2, KIT-A7-F3-P2, KIT-A6-F6-P1 |  |

## Questions for assessing this map and its demo

- Walk through selling a used item and buying one from a stranger: is every step there, from the first photo to the rating?
- Is the core set small enough to build first, yet good enough that a Trade Me or Facebook Marketplace user would switch?
- Should money always be held, even for local pickup, or should cash on pickup stay as an uncovered choice?
- Should auctions stay at later, given how many New Zealand buyers expect them?
- Is no paid placement in search the right rule, or should members vote on promotion at cost?
- Shops: start with Market sellers who want a shop page, or go after small businesses on Shopify with their own web addresses from day one?
- Is a till for physical shops worth building ourselves, or should the payment provider's own till be linked instead?
- Who sits on dispute panels, how are they chosen and trained, and are they paid?
- Which restricted items are never allowed at launch, such as firearms, alcohol and vapes, and which need licence or age checks, given that R18 items stay pickup only until an ID-checked courier service is linked (MKT-A12-F1-P5, MKT-A8-F4-P8)?
- Do the anti-money-laundering duties change how held payments, payouts or seller checks must work?
- Which town or community should the launch plan target first, and how does the Market beat Trade Me's network effect there?
- Is AI help for writing a listing from photos core or next, and what must the seller always confirm?
- Should vehicles be in the first release, given how much of Trade Me's traffic they bring, or wait until the money owing and dealer rules are built?
- Which countries come after New Zealand, and which of their rules (EU, UK, Australia) shape the core now so nothing is rebuilt later?
- Do the demo screens look like our own design, with nothing copied from any incumbent's interface?
- Who pays the Market's fee, and is it flat or based on value (MKT-A7-F2-P4)? Does it stay fair for a free item, a $2 item and a $30,000 car, now that Trade Me charges sellers 2.19%?
- Who is the seller of record (MKT-A7-F9-P1): the trader, except where the Market is the deemed supplier for low-value imported goods and its own buyer fee? Does a lawyer agree, since it sets who charges GST, who owes consumer guarantees and who answers card disputes?
- Does the Market count as an internet auction provider under the Secondhand Dealers and Pawnbrokers Act 2004 from launch, even before auctions, and how does it collect sellers' dates of birth for Police without collecting more than it needs (MKT-A21-F6-P3)?
- Should sales on a shop's own site use held payments or pay out within days (MKT-A13-F6-P4), and may shops take guest orders (MKT-A13-F6-P3)?
- Hire of items between members, such as tools, cameras and party gear, is mapped here as MKT-A2-F14 (later). Should it stay in the Market or move to Spare Rent?
- Pieces: the kit now maps MK01, MK04 and MK05 on its own parts (KIT-A3-F2-P1, KIT-A10-F12-P1 and P2, KIT-A6-F1-P1 and P2), and the Market's copies are gone. Should those pieces move to the kit's board, with the Market keeping only MK01's protection limits (MKT-A11-F2-P1)? MK06 and MK07 now also feed kit parts (stock, and card readers and a shared till). MK02 now covers platform, seller and auction law; should restricted goods law (MKT-A12-F1-P1, large, no piece) get a piece of its own?
- The Market asks the kit to open shared parts it does not have yet: an address check (MKT-A1-F3-P3); shop web pages (MKT-A13-F1-P1), which Spare Food's direct ordering page also needs; custom domains (MKT-A13-F3-P1), which Spare Stay and Spare Social also map; a guest checkout decision (MKT-A13-F6-P3), which Spare Food waits on too; customer lists and newsletters (MKT-A14-F4-P1, MKT-A14-F4-P2); invoices with payment terms (MKT-A14-F7-P2); automations (MKT-A16-F8-P1); recall feeds (MKT-A12-F6-P1); and a GST threshold warning (MKT-A19-F2-P4), which Spare Food also maps. Has the kit opened each one, so the Market part becomes a thin use of it?
- Left out on purpose: paid placement, promoted listings and ads (rankings follow published rules and are never sold); paid tiers for sellers or shops (every seller pays the same at-cost fee, and shops pay only their hosting cost); loans and cash advances to sellers (lending brings in credit law and is not a running cost); tracking pixels and cross-site tracking (counts are kept in totals only); selling or sharing members' data. Does each reason still hold?
- Are the part sizes believable for the people and spare tokens the co-op has, and what is the first piece of code to write?
- Are any parts missing, duplicated, or in the wrong area, and does every part have the dependencies it really needs?

## Retired IDs

These IDs were retired on 2026-10-19 because the kit already maps the same part, or because two Market parts did the same job. They are never reused.

| Retired ID | Now covered by |
| --- | --- |
| MKT-A1-F4-P2 | KIT-A2-F7-P1 |
| MKT-A1-F6-P2 | KIT-A1-F7-P2 |
| MKT-A1-F7-P1 | KIT-A10-F4-P1 |
| MKT-A4-F1-P1 | KIT-A6-F1-P1 |
| MKT-A4-F4-P1 | KIT-A6-F8-P1 |
| MKT-A5-F4-P2 | KIT-A3-F18-P1 |
| MKT-A5-F9-P1 | KIT-A2-F7-P3 |
| MKT-A7-F1-P1 | KIT-A3-F2-P1 |
| MKT-A7-F2-P2 | KIT-A3-F4-P2, KIT-A13-F3-P2 |
| MKT-A7-F3-P2 | KIT-A3-F3-P3 |
| MKT-A7-F5-P2 | KIT-A4-F4-P1 |
| MKT-A7-F6-P2 | KIT-A4-F5-P1, MKT-A7-F6-P1 |
| MKT-A7-F8-P2 | KIT-A11-F10-P2 |
| MKT-A8-F2-P3 | MKT-A7-F1-P4 |
| MKT-A9-F3-P2 | KIT-A5-F15-P1 |
| MKT-A9-F7-P1 | MKT-A21-F1-P1 |
| MKT-A10-F2-P2 | KIT-A8-F1-P3 |
| MKT-A10-F3-P2 | KIT-A8-F9-P1 |
| MKT-A10-F4-P2 | KIT-A8-F5-P1 |
| MKT-A10-F5-P1 | KIT-A8-F6-P1 |
| MKT-A11-F1-P2 | KIT-A9-F1-P1, MKT-A11-F1-P1 |
| MKT-A11-F3-P1 | KIT-A11-F1-P1 |
| MKT-A11-F3-P2 | KIT-A11-F2-P1 |
| MKT-A11-F3-P3 | KIT-A11-F4-P1, KIT-A11-F4-P2 |
| MKT-A11-F3-P5 | KIT-A11-F5-P1 |
| MKT-A11-F6-P2 | KIT-A9-F2-P2 |
| MKT-A12-F2-P1 | KIT-A10-F12-P1 |
| MKT-A12-F2-P3 | KIT-A10-F11-P1 |
| MKT-A12-F2-P5 | KIT-A8-F4-P5, KIT-A8-F3-P3 |
| MKT-A12-F3-P2 | KIT-A10-F6-P2 |
| MKT-A12-F5-P2 | KIT-A10-F21-P2 |
| MKT-A12-F5-P3 | KIT-A10-F21-P1 |
| MKT-A14-F2-P1 | KIT-A5-F10-P1 |
| MKT-A14-F5-P2 | KIT-A3-F15-P1 |
| MKT-A14-F9-P1 | MKT-A2-F12-P1 |
| MKT-A15-F1-P3 | KIT-A3-F19-P2 |
| MKT-A15-F1-P5 | MKT-A7-F5-P1 |
| MKT-A15-F3-P1 | KIT-A1-F7-P6 |
| MKT-A15-F6-P1 | KIT-A3-F19-P1 |
| MKT-A16-F5-P1 | KIT-A4-F4-P2 |
| MKT-A16-F6-P2 | KIT-A16-F6-P1 |
| MKT-A16-F7-P3 | KIT-A16-F1-P2, KIT-A16-F1-P3 |
| MKT-A16-F9-P1 | KIT-A5-F17-P2 |
| MKT-A16-F9-P2 | KIT-A5-F17-P1 |
| MKT-A17-F1-P2 | KIT-A10-F6-P3 |
| MKT-A17-F1-P3 | KIT-A10-F7-P1 |
| MKT-A17-F2-P1 | KIT-A11-F5-P1, KIT-A10-F8-P1 |
| MKT-A17-F3-P2 | KIT-A21-F2-P1 |
| MKT-A17-F3-P3 | KIT-A21-F4-P1 |
| MKT-A17-F4-P2 | KIT-A22-F5-P1 |
| MKT-A17-F4-P4 | KIT-A3-F9-P1 |
| MKT-A17-F5-P2 | KIT-A10-F15-P1 |
| MKT-A17-F6-P2 | KIT-A25-F2-P1, KIT-A10-F6-P4 |
| MKT-A17-F7-P1 | KIT-A21-F5-P1 |
| MKT-A18-F1-P2 | KIT-A13-F2-P2 |
| MKT-A18-F3-P1 | KIT-A14-F1-P3 |
| MKT-A18-F4-P2 | KIT-A12-F4-P1 |
| MKT-A18-F4-P3 | MKT-A21-F5-P2 |
| MKT-A19-F1-P1 | KIT-A19-F1-P1 |
| MKT-A19-F1-P3 | KIT-A19-F1-P3 |
| MKT-A19-F1-P4 | KIT-A19-F7-P1 |
| MKT-A19-F2-P2 | KIT-A4-F1-P2 |
| MKT-A19-F2-P3 | KIT-A4-F9-P3 |
| MKT-A19-F3-P2 | KIT-A4-F9-P2 |
| MKT-A19-F4-P1 | KIT-A3-F1-P4 |
| MKT-A19-F5-P1 | KIT-A19-F6-P1 |
| MKT-A19-F6-P2 | KIT-A4-F3-P2 |
| MKT-A19-F7-P1 | KIT-A19-F1-P2 |
| MKT-A19-F8-P1 | KIT-A18-F9-P1 |
| MKT-A20-F1-P1 | KIT-A19-F2-P1 |
| MKT-A20-F1-P2 | KIT-A19-F2-P1 |
| MKT-A20-F1-P3 | KIT-A19-F2-P2 |
| MKT-A20-F2-P1 | KIT-A20-F2-P1 |
| MKT-A20-F2-P3 | KIT-A8-F3-P1 |
| MKT-A20-F3-P2 | KIT-A19-F5-P1 |
| MKT-A21-F1-P2 | KIT-A9-F7-P1 |
| MKT-A21-F3-P1 | KIT-A18-F5-P1, KIT-A18-F14-P1 |
| MKT-A21-F3-P2 | KIT-A18-F3-P1 |
| MKT-A21-F3-P3 | KIT-A18-F3-P1 |
| MKT-A21-F3-P4 | KIT-A18-F7-P1 |
| MKT-A21-F4-P1 | KIT-A18-F8-P1 |
| MKT-A21-F6-P2 | KIT-A18-F6-P1 |
| MKT-A21-F7-P1 | KIT-A22-F5-P3 |
| MKT-A21-F7-P2 | KIT-A22-F5-P3 |
| MKT-A22-F1-P1 | KIT-A2-F6-P1 |
| MKT-A9-F7 (feature) | its parts, as above |
| MKT-A10-F5 (feature) | its parts, as above |
| MKT-A17-F2 (feature) | its parts, as above |
| MKT-A17-F7 (feature) | its parts, as above |
| MKT-A19-F7 (feature) | its parts, as above |
| MKT-A19-F8 (feature) | its parts, as above |
| MKT-A21-F7 (feature) | its parts, as above |

## Sources checked

Researched on 2026-10-18 and 2026-10-19. Not legal advice; items marked *unverified* need checking before anyone relies on them.

- Trade Me's payment system, seller and buyer fees, the higher buyer protection cap for Ping and Afterpay, and the new way to pay for shipping after a sale (August 2026): [Trade Me help](https://help.trademe.co.nz/hc/en-us/community/posts/17334835563919-A-new-way-to-handle-payment-for-shipping), [Buyer Protection](https://help.trademe.co.nz/hc/en-us/articles/360007000712), [Consumer NZ](https://consumer.org.nz/shopping/online-shopping/the-fine-print-whats-behind-trade-mes-move-to-scrap-success-fees). The $5,000 cap appears in news coverage; the help article still shows $2,500 (*unverified*).
- Platform income reporting in NZ covers rentals and personal services; the sale of goods has not been adopted at this stage: [IRD](https://ird.govt.nz/international-tax/exchange-of-information/digital-platform-information), [Deloitte](https://www.deloitte.com/nz/en/services/tax/perspectives/february-2025-platform-economy-changes-continue.html). Whether this has changed since early 2025 is *unverified*.
- GST on low-value imported goods: marketplaces collect GST on goods of NZ$1,000 or less sold by overseas sellers to NZ consumers, since 1 December 2019: [PwC](https://www.pwc.co.nz/insights-and-publications/subscribed-publications/tax-tips/2019-06-new-zealands-new-gst-on-lvig.html), [Russell McVeagh](https://russellmcveagh.com/Insights/February-2019/New-Zealand-proceeds-with-reform-of-GST-on-low-val). Later changes are *unverified*.
- Consumer law: since June 2014 the Consumer Guarantees Act covers goods bought by online auction from in-trade sellers, and the Fair Trading Act requires online sellers to say when they sell in trade: [Mondaq](https://www.mondaq.com/newzealand/consumer-trading-unfair-trading/294812/online-auctions-consumer-rights-at-last), [Consumer NZ](https://consumer.org.nz/consumer-rights-and-campaigns/your-rights/auctions-online), [Commerce Commission](https://comcom.govt.nz/business/dealing-with-typical-situations/selling-goods-and-services/selling-online/).
- Anti-money-laundering duties for online marketplaces since 1 June 2025: [DIA](https://www.dia.govt.nz/AML-CFT-Information-for-Online-Marketplaces).
- Vapes: since 17 June 2025 NZ online stores may not show images of vaping products, and disposable vapes are banned: [Ministry of Health](https://www.health.govt.nz/regulation-legislation/vaping-herbal-smoking-and-smokeless-tobacco/requirements/recent-changes-to-smokefree-laws). Online rules for alcohol and firearms are *unverified* and sit with piece MK02.
- EU duties for marketplaces: trader traceability and compliance by design (Digital Services Act articles 30 to 32) and product safety information on listings (General Product Safety Regulation): [Prighter](https://prighter.com/resources/laws/dsa/articles/article-30), [Global Compliance News](https://www.globalcompliancenews.com/wp-content/uploads/sites/43/2025/01/Digital-Services-Act-from-a-products-perspective.pdf).
- Accessibility: the European Accessibility Act has applied to e-commerce since 28 June 2025, with EN 301 549 (which includes WCAG 2.1 AA) as the working standard: [Bird & Bird](https://www.twobirds.com/en/insights/2025/a-guide-to-navigating-the-european-accessibility-act-for-online-retailers-service-providers-and-plat), [Taylor Wessing](https://www.taylorwessing.com/en/insights-and-events/insights/2025/09/european-accessibility-act-requirements-for-e-commerce-services).
- Vehicles: money owing checks on the Personal Property Securities Register, the Motor Vehicle Traders Register, the notices traders must give under the Motor Vehicle Sales Act, the warrant of fitness a sold vehicle needs (issued within one month before delivery, unless the buyer agrees in writing), and the online change of ownership: [PPSR](https://ppsr.companiesoffice.govt.nz/help-centre/information-for-debtors-and-consumers/how-the-ppsr-can-help-when-buying-used-goods/), [govt.nz](https://www.govt.nz/browse/transport/selling-a-vehicle/), [Consumer Protection](https://www.consumerprotection.govt.nz/assets/PDFs/CP-Fit-for-the-road-June2014.pdf). Which notices apply online, and current vehicle data access terms, are *unverified*.
- Courier links: NZ Post publishes label and tracking APIs that need a business account: [NZ Post](https://www.nzpost.co.nz/business/ecommerce/shipping-apis). Other couriers are *unverified* and sit with piece MK03.
- Shop builder scope (storefront, checkout, shipping, point of sale, markets, B2B, inventory by location, apps for subscriptions and reviews): [Shopify Markets](https://www.shopify.com.au/markets) and [a features overview](https://www.charleagency.com/articles/features-for-shopify/), read for scope only.
- Second-hand dealer law: internet auction providers need no licence but must record each person offering second-hand goods (full name, date of birth, phone and email), keep each sale's record for 12 months from when bids close and let Police see it: [NZ Police](https://www.police.govt.nz/advice-services/businesses-and-organisations/secondhand-dealers-and-pawnbrokers/information-0). A person is presumed to deal in second-hand goods after 6 or more trading days in 12 months; the $2,000 test comes from a summary of the Act and is *unverified* against its text: [Sprintlaw](https://sprintlaw.co.nz/business-law-library/legislation/nz/secondhand-dealers-and-pawnbrokers-act-2004/).
- Warrant of fitness on sale: no more than one month old when the buyer takes the vehicle, or the buyer accepts an older one in writing (Land Transport Rule: Vehicle Standards Compliance 2002, rule 9.12): [Consumer NZ](https://www.consumer.org.nz/articles/paperwork-when-buying-a-used-car), [govt.nz](https://www.govt.nz/browse/transport/selling-a-vehicle/).
- Age checks at delivery: in 2024 couriers left vapes at the door without checking age when the seller had not bought the proof-of-age service: [1News Fair Go](https://www.1news.co.nz/2024/03/05/fair-go-kids-get-vapes-delivered-straight-to-their-front-door/). The Ministry of Health encourages online sellers of vaping products to verify age: [Ministry of Health](https://www.health.govt.nz/regulation-legislation/vaping-herbal-smoking-and-smokeless-tobacco/retailers-of-vaping-and-tobacco-products/sale-of-vaping-and-smoking-products-to-minors). Whether its guidance asks for a check at delivery itself is *unverified*.
- European Accessibility Act: microenterprises that provide services (fewer than 10 staff and no more than EUR 2 million turnover or balance sheet) are exempt from its accessibility requirements (Directive (EU) 2019/882, article 4(5)), so a statement from a small shop is our own choice.
