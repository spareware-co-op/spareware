# Spare Home: everything that needs building

*Stage: Mapping. Generated with [features.json](features.json); the two files say the same thing. Every choice is open until an owner accepts a decision note.*

Spare Home lets an owner sell their home themselves, with every service an agent's commission pays for available separately at a published price. It gives sellers and buyers the tools and never acts as anyone's agent. It replaces real estate agents' percentage commissions and the listing fees of the big property sites, such as Trade Me Property, realestate.co.nz and OneRoof. This map lists every feature area a full replacement needs, what a person can do in each, and the buildable parts under each feature, from first sign-up to settlement day.

New Zealand comes first; every country-specific rule sits in a rules pack, starting with the New Zealand pack, so other countries can be added. The line Spare Home never crosses: it gives tools and books licensed people, and it never negotiates or acts for anyone in a sale (piece [HM01](STATUS.md)). Every tool that does more than publish an advert waits on HM01's answer, and each has a fallback ready if the answer is no. House money, such as the deposit and the price, never passes through Spare Home; it goes through the parties' lawyers.

Spare Home is a thin layer on the shared kit. Where the kit already maps a part, the Home part only adds Home's own rules and data to it and names the kit part it builds on. Parts Spare Home shares with other programs that the kit does not map yet are listed in [Parts that belong in the kit](#parts-that-belong-in-the-kit). Professional services are booked from one shared service catalogue that belongs in the kit, where Spare Work's pros are listed too; Spare Home never calls Spare Work directly. No core part depends on a kit part marked next or later.

**Priority:** core is needed for a first usable release; next comes soon after; later can wait. **Depends on** names shared kit crates (kit-...), kit services (svc-...), kit parts (KIT-...), shared engines (engine-...) or other Home parts by ID. **Piece** links to an existing work-board piece where one already covers the part.

**Size of the map:** 19 areas, 164 features (104 core), 372 parts.

## Areas

| ID | Area | Features | Parts |
| --- | --- | --- | --- |
| HOME-A1 | [Accounts, identity and authority to sell](#home-a1-accounts-identity-and-authority-to-sell) | 9 | 32 |
| HOME-A2 | [Property record and listing](#home-a2-property-record-and-listing) | 11 | 30 |
| HOME-A3 | [Photos, floor plans, video and tours](#home-a3-photos-floor-plans-video-and-tours) | 10 | 24 |
| HOME-A4 | [Pricing and market information](#home-a4-pricing-and-market-information) | 7 | 16 |
| HOME-A5 | [Buyer search and the listing page](#home-a5-buyer-search-and-the-listing-page) | 12 | 25 |
| HOME-A6 | [Reach and marketing](#home-a6-reach-and-marketing) | 7 | 18 |
| HOME-A7 | [Viewings and open homes](#home-a7-viewings-and-open-homes) | 10 | 24 |
| HOME-A8 | [Enquiries and the buyer register](#home-a8-enquiries-and-the-buyer-register) | 6 | 11 |
| HOME-A9 | [Documents and disclosure](#home-a9-documents-and-disclosure) | 7 | 17 |
| HOME-A10 | [Offers and methods of sale](#home-a10-offers-and-methods-of-sale) | 9 | 21 |
| HOME-A11 | [From agreement to settlement](#home-a11-from-agreement-to-settlement) | 12 | 24 |
| HOME-A12 | [Professional services](#home-a12-professional-services) | 9 | 18 |
| HOME-A13 | [Guides and checklists](#home-a13-guides-and-checklists) | 6 | 12 |
| HOME-A14 | [Payments, fees and the open cost ledger](#home-a14-payments-fees-and-the-open-cost-ledger) | 6 | 14 |
| HOME-A15 | [Trust, safety and moderation](#home-a15-trust-safety-and-moderation) | 7 | 12 |
| HOME-A16 | [Law, the agency line and countries](#home-a16-law-the-agency-line-and-countries) | 9 | 21 |
| HOME-A17 | [Access for everyone: accessibility, languages, phones and offline](#home-a17-access-for-everyone-accessibility-languages-phones-and-offline) | 11 | 19 |
| HOME-A18 | [Your data, AI and plugins](#home-a18-your-data-ai-and-plugins) | 6 | 16 |
| HOME-A19 | [Running Spare Home: support, admin and member votes](#home-a19-running-spare-home-support-admin-and-member-votes) | 10 | 18 |

## HOME-A1 Accounts, identity and authority to sell

Everyone uses one Spareware login. Sellers prove who they are, online or in person, and that they own and control the home or may sell it, before a listing goes live, and high-risk actions need a recent second sign-in step.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A1-F1** Sign up and sign in | Create one Spareware login and choose to sell, buy, or both. | core | P1, P2, P3, P4, P5 |
| **HOME-A1-F2** Verify identity | Prove who you are once, through a regulated checking provider or in person through your lawyer, before listing a home or making an offer, wherever you live. | core | P1, P2, P3, P4 |
| **HOME-A1-F3** Prove you own the home | Show you are the registered owner and that you control the home before the listing can go live, while the owner's address of record is told so a real owner can object. | core | P1, P2, P3, P4, P5, P6, P7, P8, P9 |
| **HOME-A1-F4** Sell on someone else's behalf | List a home as a trustee, executor, attorney or company director, with evidence of authority. | core | P1, P2 |
| **HOME-A1-F5** Sell with co-owners | Invite every co-owner to approve the listing and each big decision. | core | P1, P2 |
| **HOME-A1-F6** Let someone help | Give a family member, lawyer or adviser limited access to your sale and take it back at any time. | core | P1, P2 |
| **HOME-A1-F7** Close or pass on an account | Close your account when you are done, or let an executor take over a sale after a death. | next | P1, P2 |
| **HOME-A1-F8** Confirm high-risk actions | Confirm with a recent second sign-in step before you record an accepted offer, submit or withdraw an offer, bid, change the price, share documents, add a helper or change your lawyer. | core | P1, P2, P3, P4 |
| **HOME-A1-F9** Sell as a lender under a mortgage | As a bank or other lender, list a home you are selling under a mortgage, with the evidence and notices the law requires. | next | P1, P2 |

### Parts for HOME-A1

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A1-F1-P1 | **Home roles on the shared account**: Adds Home roles to the kit's roles per app: seller, co-owner, buyer, helper, tenant, lawyer, professional, body corporate manager, licensed agent, auctioneer, mortgagee or lender, moderator and admin. | library | small | svc-accounts, KIT-A1-F5-P2 |  |
| HOME-A1-F1-P2 | **Home sign-up choices**: Adds the sell, buy or both choice to the kit's passkey-first sign-up and sign-in screens, asking only for what the chosen role needs. | interface | small | KIT-A1-F1-P2, KIT-A1-F2-P4, HOME-A1-F1-P1 |  |
| HOME-A1-F1-P3 | **Home contact and privacy settings**: Adds Home's choices, such as who may see your name on a listing and how buyers may reach you, to the kit's profile page. | interface | small | KIT-A1-F5-P3, svc-messaging |  |
| HOME-A1-F1-P4 | **Tenant access by link**: Lets a tenant read sale notices and answer viewing requests through a private link, with no account needed. | library | small | svc-accounts, HOME-A1-F1-P1, KIT-A15-F8-P1 |  |
| HOME-A1-F1-P5 | **Home's minimum age**: Declares Home's minimum age in the kit: 18 to list, offer or bid, the age at which a person in NZ can make a contract that binds them, while browsing needs no account. | data | small | KIT-A1-F1-P5 |  |
| HOME-A1-F2-P1 | **Identity check for Home actions**: Uses the kit's identity check link and keeps only the kit's pass or fail result, never the documents. | integration | small | KIT-A2-F1-P2, KIT-A2-F1-P3 |  |
| HOME-A1-F2-P2 | **Check level for each action**: Sets which actions need which check: browsing none, booking a viewing a confirmed phone number, and listing, offering or bidding a full identity check by any route in this feature. | data | small | HOME-A1-F1-P1 |  |
| HOME-A1-F2-P3 | **Identity checked in person**: Lets a person with no digital ID or smartphone have their identity checked in person by their lawyer or another trusted checker, recorded as the same pass or fail result as a digital check. Every app needs this path, so it belongs in the kit beside KIT-A2-F1. | integration | medium | KIT-A2-F1-P3, HOME-A12-F2-P3 |  |
| HOME-A1-F2-P4 | **Identity for people overseas**: Lets sellers and buyers who live outside NZ prove who they are in the first release, through their NZ lawyer's anti-money-laundering identity check or a provider that covers their country, instead of waiting for KIT-A2-F10, which is marked later. Every app with members overseas needs this, so it belongs in the kit. | integration | medium | KIT-A2-F1-P3, HOME-A1-F2-P3 |  |
| HOME-A1-F3-P1 | **Title and owner data terms**: Finds which LINZ title and owner datasets Spare Home may use, under which licence and at what cost, where the owner's postal address of record may lawfully come from, and the same for other countries' land registers. Done once with Spare Rent's title research (RENT-A1-F3-P3), and in one research piece with HOME-A2-F1-P3, HOME-A4-F1-P1 and HOME-A5-F3-P1. | research | small |  |  |
| HOME-A1-F3-P2 | **Owner name match**: Matches the verified name against the owners on the record of title and flags any mismatch, but a name match alone never earns the checked owner badge. It is one ownership check shared with Spare Rent (RENT-A1-F3-P2) and belongs in the kit. | integration | medium | HOME-A1-F2-P1, HOME-A1-F3-P1, svc-accounts, HOME-A16-F4-P4 |  |
| HOME-A1-F3-P3 | **Ownership review queue**: Lets a trained member settle the cases the automatic match cannot, with every decision logged. One review queue shared with Spare Rent that belongs in the kit. | interface | small | HOME-A1-F3-P2, svc-trust |  |
| HOME-A1-F3-P4 | **Checked owner badge**: Shows buyers which ownership checks passed, read from the kit's check results store, in wording checked under consumer law, only when the name match, proof of control and the notice to the owner's address of record all pass, and without showing who the owner is. It becomes Home's entry on the kit's verified badge (KIT-A2-F5-P1) once the kit moves that badge to core. | interface | small | HOME-A1-F3-P2, HOME-A1-F3-P5, HOME-A16-F5-P1, KIT-A2-F1-P3, HOME-A1-F3-P8 |  |
| HOME-A1-F3-P5 | **Proof of control**: Confirms the seller controls the home through a match on the rates assessment number or rates notice, confirmation from the seller's lawyer, or a code posted to the property. A posted code counts only when the seller declares no one else lives there, and never for a tenanted home. Shared with Spare Rent; belongs in the kit. | integration | medium | HOME-A1-F3-P2, HOME-A2-F1-P2, KIT-A2-F1-P3 |  |
| HOME-A1-F3-P6 | **Extra checks for higher-risk sales**: Sends vacant land, absentee or overseas owners and contact details changed recently to the review queue and asks the seller's lawyer to confirm. | library | small | HOME-A1-F3-P5, HOME-A1-F3-P3 |  |
| HOME-A1-F3-P7 | **Owners whose details are withheld**: Gives owners whose details are withheld from the public register for safety, and sellers with protection orders, a checked path through their lawyer with the address hidden by default (which part of the law applies is unverified). | library | small | HOME-A1-F3-P3, HOME-A15-F5-P1 |  |
| HOME-A1-F3-P8 | **Notice to the owner's address of record**: Whenever a listing is started, posts a letter with a code to the owner's postal address on the council's rating record, telling them a listing was started and how to stop it, and holds the listing for a set time. Where that address may lawfully come from is settled in HOME-A1-F3-P1 (unverified). Shared with Spare Rent; belongs in the kit. | integration | medium | HOME-A1-F3-P1, HOME-A2-F1-P2, svc-messaging |  |
| HOME-A1-F3-P9 | **Lawyer confirmation when others live there**: Requires the seller's lawyer to confirm the seller is the registered owner whenever the home is tenanted, someone other than the seller lives there, or the only proof of control is a code posted to the property, so a tenant or a relative with the same name cannot pass. | library | small | HOME-A1-F3-P5, HOME-A11-F3-P1 |  |
| HOME-A1-F4-P1 | **Kinds of authority to sell**: Lists each kind of seller who is not the plain owner, such as a trustee, executor, attorney or company director, and the evidence each must give, per country, on one model of who owns a property and who may act for it. Shared with Spare Rent (RENT-A1-F3-P1 and RENT-A1-F4-P1), it belongs in the kit; it does not wait for the kit's organisation model (KIT-A1-F7-P1), which is marked next. | data | small | HOME-A1-F3-P1 |  |
| HOME-A1-F4-P2 | **Authority evidence upload**: Collects and checks trust deeds, probate, powers of attorney and company records, then keeps only the reviewed result and deletes the documents, as with identity checks. Shared with Spare Rent; belongs in the kit. | interface | medium | HOME-A1-F4-P1, kit-store, HOME-A1-F3-P3, HOME-A16-F4-P2 |  |
| HOME-A1-F5-P1 | **Co-owner invitations**: Invites each owner on the title to join the sale and confirms they are who the title says. One invitation flow shared with Spare Rent (RENT-A1-F4-P2) that belongs in the kit. | library | medium | svc-accounts, HOME-A1-F3-P2, HOME-A16-F4-P4 |  |
| HOME-A1-F5-P2 | **Decisions that need every owner**: Sets which steps need all owners to agree, such as going live, changing the price and accepting an offer, on one every-owner approval step shared with Spare Rent that belongs in the kit. | library | small | HOME-A1-F5-P1, kit-ops |  |
| HOME-A1-F6-P1 | **Home helper permissions**: Adds Home's sets of actions for one sale, such as reading enquiries or uploading documents, as access levels on the kit's sharing and access. Acting in the seller's name waits for the kit's delegated access (KIT-A1-F8-P1), which is marked next. | library | medium | KIT-A15-F8-P1, kit-ops |  |
| HOME-A1-F6-P2 | **Who did what**: Shows the seller every action taken on their sale and by whom. | interface | small | HOME-A1-F6-P1, kit-ops |  |
| HOME-A1-F7-P1 | **Closing with a sale under way**: Stops an account closing while a listing, offer or sale is live, offers a hand-over or export, then uses Home's data deletion. | library | small | HOME-A18-F1-P1, HOME-A18-F2-P1, HOME-A2-F4-P1 |  |
| HOME-A1-F7-P2 | **Executor hand-over**: Moves a live sale to an executor through the kit's after-death account handling, once the authority evidence is checked. | library | small | HOME-A1-F4-P2, HOME-A1-F6-P1, KIT-A18-F11-P1 |  |
| HOME-A1-F8-P1 | **High-risk action list**: Lists the Home commands with legal, money or safety effect: recording an accepted agreement, submitting or withdrawing an offer document, registering as a bidder, placing a bid, sending one update to every interested buyer, giving a licensed agent the buyer register, handing a live sale to another site, withdrawing or relisting, changing the price, marking sold, publishing the sale price, revealing the exact address, sharing documents, approving a viewing at the home, adding a helper, changing a lawyer or their bank details, and changing the proof-of-control details. Placing an auction bid is a people-only action that no agent may take. | data | small | HOME-A18-F3-P1, KIT-A17-F9-P2 |  |
| HOME-A1-F8-P2 | **Fresh sign-in step**: Asks for a sign-in with the second step within the last few minutes before any listed action, whether a person or an agent starts it. | library | small | HOME-A1-F8-P3, HOME-A1-F8-P1, HOME-A1-F8-P4 |  |
| HOME-A1-F8-P3 | **Recent sign-in check**: A check any app can put on a command: the person must have signed in with their second step within the last few minutes. KIT-A1-F3-P1 says only who must have a second step, and KIT-A1-F11-P1 and KIT-A3-F3-P4 already assume this check, so it belongs in the kit. | library | small | KIT-A1-F3-P1, KIT-A1-F3-P2 |  |
| HOME-A1-F8-P4 | **Second step set-up for sellers and buyers**: Walks a seller or buyer through setting up a second sign-in step, with an option that needs no smartphone, before their first high-risk action rather than in the middle of it. Every app with high-risk actions needs it, so it belongs in the kit. | interface | small | KIT-A1-F3-P1, HOME-A1-F8-P3 |  |
| HOME-A1-F9-P1 | **Mortgagee sale rules**: Sets out the notices a lender must give before selling under a mortgage, its duty to take reasonable care to get the best price reasonably obtainable, and the evidence Spare Home should ask for, with sources. | research | small |  |  |
| HOME-A1-F9-P2 | **Lender seller path**: Adds lenders as a kind of seller, checks their evidence through the authority evidence upload and shows buyers plainly that it is a mortgagee sale. | library | small | HOME-A1-F9-P1, HOME-A1-F4-P2 |  |

## HOME-A2 Property record and listing

A listing starts from a property record filled in from open data. The seller adds what only they know, sets a price and method of sale, and controls when it goes live.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A2-F1** Start from your address | Type your address and get a property record already filled in from open data. | core | P1, P2, P3, P4 |
| **HOME-A2-F2** Write the listing | Write a headline, description and key facts, with prompts for what buyers look for. | core | P1, P2, P3 |
| **HOME-A2-F3** Set price and method of sale | Choose an asking price, a price range, 'enquiries over' a figure or offers by negotiation, with deadline sales, tenders and auctions added as their tools arrive. | core | P1, P2, P3, P4, P5 |
| **HOME-A2-F4** Go live, pause and withdraw | Publish, pause, mark under offer or sold, or withdraw the listing at any time, and confirm now and then that it is still for sale. | core | P1, P2, P3, P4, P5 |
| **HOME-A2-F5** Change the listing openly | Edit details, price and photos, with a change history buyers can see. | core | P1 |
| **HOME-A2-F6** List the common kinds of property | List houses, units with a body corporate, cross-leases, leasehold land, rural and lifestyle blocks and bare sections, and get a clear message for kinds not supported yet. | core | P1, P4 |
| **HOME-A2-F7** List new builds | List a home that is not built yet, or many homes in one development. | next | P1 |
| **HOME-A2-F8** Relist and pass the record on | Relist later without starting again, or hand the property record to the buyer at settlement. | next | P1, P2 |
| **HOME-A2-F9** Declare any agency agreement | Say whether the home is or was listed with an agent, and see what that agent could still claim. | core | P1, P2, P3 |
| **HOME-A2-F10** Keep your home's record before selling | Claim your home and build its record, documents and plans before you decide to sell. | later | P1 |
| **HOME-A2-F11** List Māori freehold land and unusual tenures | List Māori freehold land, company-share flats, licence to occupy and stratum in freehold, with the extra steps and disclosures each needs. | next | P1, P2, P3 |

### Parts for HOME-A2

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A2-F1-P1 | **Address lookup**: Finds the property from a typed address using the kit's open address search. | integration | small | svc-maps |  |
| HOME-A2-F1-P2 | **Shared property record**: Holds land, buildings, rooms, title, rates, zoning and past sales for one property from rental to sale, as one record shared with Spare Rent (replacing RENT-A2-F1-P2) and Spare Stay that belongs in the kit. | library | large | kit-core, kit-ops, kit-store |  |
| HOME-A2-F1-P3 | **Open property data sources**: Lists the NZ open datasets for parcels, addresses, land area, building outlines, zoning, aerial imagery and rating values, with licences, costs and update rates. Done once with Spare Rent's parcel research for the shared property record, and in one research piece with HOME-A1-F3-P1, HOME-A4-F1-P1 and HOME-A5-F3-P1. | research | medium |  |  |
| HOME-A2-F1-P4 | **Open data import**: Fills the record from LINZ and council data, showing the source and date beside every fact. | integration | medium | HOME-A2-F1-P2, HOME-A2-F1-P3, svc-maps, HOME-A2-F1-P1 |  |
| HOME-A2-F2-P1 | **Home listing**: A kit-market listing with home facts, a method of sale, open homes and a status. | library | medium | kit-market, HOME-A2-F1-P2 |  |
| HOME-A2-F2-P2 | **Listing editor**: Walks the seller through each section with a live preview on phone and desktop, saving every change as it goes. | interface | large | HOME-A2-F2-P1, kit-ops, kit-sync |  |
| HOME-A2-F2-P3 | **Checks on stated facts**: Warns when stated land area, floor area, bedrooms or zoning differ from the record, so claims stay honest. | library | medium | HOME-A2-F1-P2, HOME-A2-F2-P1 |  |
| HOME-A2-F3-P1 | **Methods of sale**: Defines each method, such as asking price, price range, enquiries over, negotiation, deadline, tender and auction, and the dates and settings each needs, offering deadline, tender and auction only once their tools in HOME-A10-F5 and HOME-A10-F6 are live. | library | small | HOME-A2-F2-P1, HOME-A16-F6-P1 |  |
| HOME-A2-F3-P2 | **Price display rules**: Sets how each method may show a price, following each country's rules on price indications. | data | small | HOME-A2-F3-P1, HOME-A2-F3-P3, HOME-A16-F6-P1 |  |
| HOME-A2-F3-P3 | **GST status of the sale**: Records whether GST applies to the price, such as a new build sold by a registered developer, land sold between registered parties at zero rate, or a mixed-use block, and labels the price, for example 'plus GST if any'. | data | small | HOME-A2-F3-P4, HOME-A2-F1-P2 |  |
| HOME-A2-F3-P4 | **Tax on home sales**: Sets out, with sources and a tax adviser's check, when GST applies to a home's price, including compulsory zero-rating between registered parties, residential land withholding tax for offshore sellers, the land transfer tax statement, and the income tax rules a seller should ask their accountant about, such as the bright-line test. One research piece for every tax question in Spare Home. | research | small |  |  |
| HOME-A2-F3-P5 | **Hidden search price**: For a listing with no shown price, such as negotiation, deadline, tender or auction, asks the seller for an expected price that only places the listing in price-filtered searches and is never shown. It is checked against the rating value and recent similar sales, and must be updated when the seller's price changes, so it cannot be set low to draw buyers in. | library | small | HOME-A2-F3-P1, HOME-A4-F3-P1, HOME-A4-F1-P2 |  |
| HOME-A2-F4-P1 | **Listing status steps**: Moves a listing through draft, coming soon, live, paused, under offer, unconditional, sold, sold elsewhere, expired and withdrawn, each step an operation. | library | small | HOME-A2-F2-P1, kit-ops |  |
| HOME-A2-F4-P2 | **Checks before going live**: Holds a listing back until every gate passes: a checked owner with proof of control or approved authority evidence, the notice to the owner's address of record, every co-owner's agreement, the seller declaration, the required disclosures, the listing checks, any earlier agency agreement declared, a supported tenure, the listing fee paid or, once HOME-A14-F3 exists, set to be paid at settlement, and, for a tenanted home, the written notice sent to tenants. | library | medium | HOME-A2-F4-P1, HOME-A1-F3-P2, HOME-A1-F3-P5, HOME-A1-F4-P2, HOME-A1-F5-P2, HOME-A9-F3-P1, HOME-A16-F7-P2, HOME-A2-F4-P3, HOME-A2-F9-P1, HOME-A14-F1-P2, HOME-A7-F5-P3, HOME-A1-F3-P8, HOME-A1-F3-P9, HOME-A2-F6-P4 |  |
| HOME-A2-F4-P3 | **Listing check pipeline**: Runs every listing check in one pass before going live and after each change, on top of the kit's listing checks: stated facts, listing rules, wording, fake listing signs and the same home listed twice. | library | medium | KIT-A5-F1-P3, HOME-A2-F2-P3, HOME-A15-F4-P1, HOME-A16-F5-P2, HOME-A15-F1-P2, HOME-A15-F1-P3 |  |
| HOME-A2-F4-P4 | **Stale listing check**: Asks the seller at set intervals to confirm the home is still for sale and expires the listing if they do not answer. | library | small | HOME-A2-F4-P1, svc-messaging |  |
| HOME-A2-F4-P5 | **Title notices check**: Flags caveats, notices of claim under the Property (Relationships) Act and other notices registered on the title, and tells the seller to see their lawyer before going live, since these can stop a sale late. | library | small | HOME-A9-F2-P1, HOME-A2-F1-P4 |  |
| HOME-A2-F5-P1 | **Public change history**: Shows buyers price changes, relists and days on market, drawn from the operation log, with personal details kept erasable by the kit's method. | library | small | kit-ops, kit-sync, HOME-A2-F4-P1, KIT-A15-F1-P6 |  |
| HOME-A2-F6-P1 | **Fields for each property type**: Adds the facts each supported property type and tenure needs, such as levies, lease terms, flats plans, land use, water rights and covenants. | data | medium | HOME-A2-F1-P2 |  |
| HOME-A2-F6-P4 | **Tenure gate**: Blocks a listing whose tenure is not supported yet, such as Māori freehold land, company-share flats, licence to occupy and stratum in freehold, with a clear message on why and where to get help, and says that retirement-village units are out of scope. | library | small | HOME-A2-F6-P1, HOME-A2-F1-P4 |  |
| HOME-A2-F7-P1 | **New build and off-the-plan listings**: Lists homes not yet built, with plans, finish dates and many units under one development. | library | medium | HOME-A2-F2-P1, HOME-A2-F6-P1 |  |
| HOME-A2-F8-P1 | **Relist from a past sale**: Starts a new listing from the last one, keeping the photos, plans and documents the seller chooses that each file's licence allows. | library | small | HOME-A2-F2-P1, HOME-A2-F1-P2, HOME-A3-F8-P2 |  |
| HOME-A2-F8-P2 | **Record hand-over to the buyer**: Passes the plans, documents and appliance details to the new owner when the seller agrees, as one hand-over shared with Spare Rent (replacing RENT-A2-F8-P1) that belongs in the kit. | library | medium | HOME-A2-F1-P2, svc-accounts, HOME-A3-F8-P2 |  |
| HOME-A2-F9-P1 | **Earlier agency questions**: Asks whether the home is or was listed with an agent, when that agency ended and whether it was sole or general, and warns that the agent may claim commission on a buyer they introduced. | library | small | HOME-A2-F2-P1, HOME-A2-F9-P2 |  |
| HOME-A2-F9-P2 | **Agency clause rules**: Confirms from the Real Estate Authority's standard clauses that an agent may claim commission on a private sale to a buyer they introduced within 6 months of the agency ending, or 12 months for rural. | research | small |  |  |
| HOME-A2-F9-P3 | **Buyers an agent introduced**: Lets the seller flag buyers an earlier agent introduced, so the seller and their lawyer check before accepting an offer from one. | library | small | HOME-A2-F9-P1, HOME-A8-F3-P1 |  |
| HOME-A2-F10-P1 | **Owner's home page**: Lets a checked owner build the home's record, documents and plans ahead of any sale, ready for a first listing or a relisting. | interface | medium | HOME-A1-F3-P5, HOME-A2-F1-P2, HOME-A9-F1-P1 |  |
| HOME-A2-F11-P1 | **Selling Māori freehold land**: Sets out the extra legal steps for Māori land, such as offering to preferred groups first and Māori Land Court steps. | research | medium |  |  |
| HOME-A2-F11-P2 | **Older and unusual tenures**: Sets out the documents and disclosures for company-share flats, licence to occupy, stratum in freehold and cross-leases whose flats plan no longer matches what was built. | research | medium |  |  |
| HOME-A2-F11-P3 | **Fields and steps for unusual tenures**: Adds the facts, documents and steps these tenures need, such as company shares, occupation licences and the preferred buyer groups for Māori land, and lifts the tenure gate for each one once it is ready. | data | medium | HOME-A2-F11-P1, HOME-A2-F11-P2, HOME-A2-F6-P1, HOME-A2-F6-P4 |  |

## HOME-A3 Photos, floor plans, video and tours

Buyers decide on photos and plans first. Sellers get the same quality buyers expect, with honest labels on anything edited, a record of who owns each file and what it may be used for, and floor plans straight from a Spare Arch model.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A3-F1** Add and arrange photos | Upload photos from a phone or camera, put them in order and add captions. | core | P1, P2, P3, P4 |
| **HOME-A3-F2** Label edited photos honestly | Mark any edited, staged or AI-changed photo so buyers know what they are looking at. | core | P1, P2, P3 |
| **HOME-A3-F3** Add a floor plan | Draw a floor plan in the app, upload one you already have, or bring one in from Spare Arch. | core | P1, P2, P4, P5, P6 |
| **HOME-A3-F4** Add video | Upload a walk-through video with captions. | next | P1, P2 |
| **HOME-A3-F5** Offer a 3D walk-through | Let buyers walk through the home in 3D on any device. | later | P1, P2, P3 |
| **HOME-A3-F6** Add aerial photos | Add drone or aerial photos taken by a booked professional. | later | P1 |
| **HOME-A3-F7** Keep media at cost | Have photos, plans and video stored and served cheaply, with old sales archived. | core | P1 |
| **HOME-A3-F8** Know who owns your photos and plans | Record who owns each photo, plan and video and what it may be used for, so nothing is reused without the right to. | core | P1, P2 |
| **HOME-A3-F9** Furnish empty rooms in the app | Add virtual furniture to photos of empty rooms, always labelled. | next | P1 |
| **HOME-A3-F10** Make a plan from a phone scan | Scan rooms with your phone and get a floor plan to check and fix. | next | P1, P2 |

### Parts for HOME-A3

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A3-F1-P1 | **Photo upload and ordering**: Uploads in the background, survives a dropped connection, and lets the seller drag photos into order. | interface | medium | engine-media, kit-store, kit-sync |  |
| HOME-A3-F1-P2 | **Home image sizes**: Makes the sizes Spare Home pages need through the kit's file uploads and engine-media and keeps the original, with hidden location data removed by the kit's photo metadata stripping. | service | medium | engine-media, KIT-A15-F7-P1, KIT-A15-F7-P3 |  |
| HOME-A3-F1-P3 | **Privacy check on photos**: Flags faces, family photos, number plates, mail, valuables, alarm panels, key safes and keys so the seller can blur them before going live, a check shared with Spare Rent and Spare Stay that belongs in the kit's media service. | library | medium | engine-media, HOME-A3-F1-P2 |  |
| HOME-A3-F1-P4 | **Photo tips**: Shows short tips for light, angles and tidying, and which rooms buyers most want to see. | interface | small |  |  |
| HOME-A3-F2-P1 | **Edit labels**: Records what was changed in each photo and shows a clear label to buyers. | library | small | HOME-A3-F1-P2 |  |
| HOME-A3-F2-P2 | **Virtually furnished photos**: Labels photos furnished by a stager or another tool as virtually furnished and keeps the bare photo one tap away. | library | small | HOME-A3-F2-P1 |  |
| HOME-A3-F2-P3 | **Content credentials**: Reads content credentials on uploaded photos before hidden data is stripped, and labels edited or AI-changed images automatically, alongside the seller's own declaration. | library | small | HOME-A3-F2-P1, KIT-A15-F7-P1 |  |
| HOME-A3-F3-P1 | **Simple plan drawer**: Lets a seller trace rooms with set sizes, doors and windows with no design skills needed, built once on engine-draw2d and shared with Spare Rent and Spare Stay. | interface | large | engine-draw2d, kit-ops |  |
| HOME-A3-F3-P2 | **Plans from Spare Arch**: Imports the listing plan Spare Arch exports in an open format and places it on the listing. Spare Arch owns the conversion from a model (ARCH-A11-F7-P3) and Spare Home only imports; the importer is built on engine-draw2d and shared with Spare Rent and Spare Stay. | integration | small | engine-draw2d, HOME-A3-F3-P4 | [HM04](STATUS.md) |
| HOME-A3-F3-P4 | **Plan output**: Draws a plan as an image, a PDF and a plain text room list for screen readers. | library | medium | engine-draw2d, engine-pdf, engine-text, HOME-A17-F1-P4 |  |
| HOME-A3-F3-P5 | **Room and floor areas**: Works out room and floor areas from a drawn, imported or uploaded plan, with how they were measured stated beside them. | library | small | HOME-A3-F3-P1, HOME-A2-F2-P3, HOME-A3-F3-P2, HOME-A3-F3-P6 |  |
| HOME-A3-F3-P6 | **Upload an existing plan**: Takes a PDF or image of a plan from a draughtsperson, builder or the council property file, records its copyright owner and licence, and lets the seller add room names and sizes. | library | small | kit-store, HOME-A3-F8-P2, HOME-A3-F3-P4 |  |
| HOME-A3-F4-P1 | **Home video settings**: Converts and streams walk-through videos through engine-media and the kit's storage at cost, keeping the original. | service | medium | engine-media, KIT-A15-F7-P2 |  |
| HOME-A3-F4-P2 | **Captions**: Uses the kit's captions to draft captions from the soundtrack for the seller to fix and approve. | library | small | KIT-A19-F4-P1, HOME-A3-F4-P1 |  |
| HOME-A3-F5-P1 | **3D walk-through viewer**: Shows a 3D tour in the browser and on phones, with a plain fallback for slow devices. | interface | large | engine-render |  |
| HOME-A3-F5-P2 | **Walk-through from a Spare Arch model**: Makes a walk-through from an exported Spare Arch model without a camera visit. | integration | medium | engine-render, HOME-A3-F3-P2 |  |
| HOME-A3-F5-P3 | **Capturing tours from photos**: Finds open methods for building a tour from 360 photos or phone scans, and what they cost to run. | research | medium |  |  |
| HOME-A3-F6-P1 | **Drone rules**: Sets out aviation and privacy rules for drone photos over homes and neighbours. | research | small |  |  |
| HOME-A3-F7-P1 | **Media archive rule**: Moves sold listings' media to cheaper storage through the kit's storage at cost, with the cost shown in the ledger. | service | medium | KIT-A15-F7-P2, svc-ledger |  |
| HOME-A3-F8-P1 | **Media rights research**: Settles who owns professional photos and plans, what licence the seller gets, and whether media may be reused at relisting, sent to other sites, handed to the buyer, published as open data or reused from an earlier agent's campaign. | research | small |  |  |
| HOME-A3-F8-P2 | **Licence on each file**: Records the owner and licence of each photo, plan and video and blocks any use the licence does not allow. Shared with Spare Rent and Spare Stay; belongs in the kit's file service. | library | small | HOME-A3-F8-P1, HOME-A3-F1-P2 |  |
| HOME-A3-F9-P1 | **Virtual furniture tool**: Lets the seller add furniture to photos of empty rooms, using furniture models the co-op has the right to use, always labelled, with the bare photo one tap away. | library | medium | HOME-A3-F2-P2, engine-media |  |
| HOME-A3-F10-P1 | **Plans from a phone scan**: Finds open, accurate methods for turning a phone room scan into a floor plan. | research | medium |  |  |
| HOME-A3-F10-P2 | **Plans from a phone scan**: Turns a phone room scan into a plan in the plan drawer, using the method chosen in the research, for the seller to check and fix. | library | large | HOME-A3-F10-P1, HOME-A3-F3-P1 |  |

## HOME-A4 Pricing and market information

Agents win listings with appraisals. Spare Home gives sellers and buyers the evidence to judge price themselves, says plainly how every figure was worked out, and books a registered valuer when a formal value is needed.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A4-F1** See comparable sales | See recent sales of similar homes nearby to help set or judge a price. | core | P1, P2, P3, P4 |
| **HOME-A4-F2** Get a price guide | Get a plain price range with the reasons behind it, not a single number. | next | P1, P2, P3 |
| **HOME-A4-F3** See rating values and past sales | See the council rating value and the home's past sales on the listing. | core | P1, P2 |
| **HOME-A4-F4** Publish your sale price openly | Choose to add your final sale price to an open dataset anyone can use. | next | P1, P2 |
| **HOME-A4-F5** Read the local market | See how many homes are selling in an area, how fast, and how prices are moving. | later | P1, P2 |
| **HOME-A4-F6** Book a registered valuer | Book a registered valuer at a published price when a court, the seller or a buyer needs a formal value, knowing that lenders usually use valuers from their own panel. | core | P1 |
| **HOME-A4-F7** See what selling will cost | Work out the full cost of selling with Spare Home next to a typical commission. | core | P1, P2 |

### Parts for HOME-A4

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A4-F1-P1 | **Where sale prices can come from**: Finds which sale price data can be legally used and shown, such as council valuation rolls and land records, at what cost and under what terms. Part of one data research piece with HOME-A1-F3-P1, HOME-A2-F1-P3 and HOME-A5-F3-P1. | research | medium |  |  |
| HOME-A4-F1-P2 | **Similar sales finder**: Picks recent nearby sales by size, type, age and land, and says why each one was picked. | library | medium | kit-store, svc-maps, HOME-A4-F1-P1, HOME-A2-F1-P2 |  |
| HOME-A4-F1-P3 | **Similar sales view**: Shows the chosen sales on a map and a list, and lets the seller add or remove any of them. | interface | medium | HOME-A4-F1-P2, svc-maps |  |
| HOME-A4-F1-P4 | **Fallback without sale prices**: Shows rating values and earlier Spare Home sales, and says plainly what is missing, until a lawful and affordable source of sale prices is confirmed. | library | small | HOME-A4-F3-P1, HOME-A4-F3-P2 |  |
| HOME-A4-F2-P1 | **Explainable price range**: Works out a likely range from similar sales and the home's facts, keeping every step visible. | library | large | HOME-A4-F1-P2 |  |
| HOME-A4-F2-P2 | **How the range was worked out**: Shows the sales, adjustments and confidence behind the range in plain words. | interface | small | HOME-A4-F2-P1 |  |
| HOME-A4-F2-P3 | **Not a valuation**: Wording that makes clear the guide is not a registered valuation or advice, checked by a lawyer. | legal | small | HOME-A4-F2-P1 |  |
| HOME-A4-F3-P1 | **Rating values import**: Brings in each council's rating values and dates, where the terms allow, labelled with their source. | integration | medium | HOME-A4-F1-P1, HOME-A2-F1-P2 |  |
| HOME-A4-F3-P2 | **Past sales of the home**: Shows past sale dates and prices from records that may be shown, and from earlier Spare Home sales. | library | small | HOME-A2-F1-P2, HOME-A4-F3-P1 |  |
| HOME-A4-F4-P1 | **Open sales dataset**: Publishes agreed sale prices as open data with the seller's and buyer's consent, so price evidence is no longer locked away. | data | medium | kit-store, HOME-A2-F1-P2 |  |
| HOME-A4-F4-P2 | **Consent to publish**: Asks both sides at settlement to agree before a price is published, and lets them choose a delay. | library | small | HOME-A4-F4-P1, HOME-A11-F8-P1 |  |
| HOME-A4-F5-P1 | **Area market figures**: Works out sales counts, days on market and price movement by suburb from data that may be shown. | library | medium | HOME-A4-F4-P1, HOME-A4-F1-P1 |  |
| HOME-A4-F5-P2 | **Area market page**: Shows the figures as simple charts with sources and dates. | interface | medium | HOME-A4-F5-P1 |  |
| HOME-A4-F6-P1 | **Valuers in the catalogue**: Adds registered valuers as entries on the one booking flow and files each report in the property's document vault. | integration | small | HOME-A12-F1-P2, HOME-A9-F1-P1 |  |
| HOME-A4-F7-P1 | **Selling cost calculator**: Adds the at-cost fee, the chosen services and any commission an earlier agent could still claim, and shows the typical commission for the same price beside it. | interface | small | HOME-A14-F1-P1, HOME-A12-F1-P1, HOME-A2-F9-P1 |  |
| HOME-A4-F7-P2 | **Typical commission figures**: Checks current agent commission rates and minimums with sources, so the comparison is fair. | research | small |  |  |

## HOME-A5 Buyer search and the listing page

Buyers must find homes as easily as on the big property sites. Search is fast, works on a map, never sells its ranking, and every listing page carries the facts buyers need about the home and the area.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A5-F1** Search by map and filters | Search by drawing on a map, by suburb or school zone, and filter by price, bedrooms, land, type and features. | core | P1, P2, P3, P4 |
| **HOME-A5-F2** Read a full listing page | See photos, plans, facts, documents, open homes, price history and how to make an offer on one page. | core | P1, P2 |
| **HOME-A5-F3** Check the area | See school zones, natural hazards, zoning, transport and nearby services for a home. | core | P1, P2, P3, P4 |
| **HOME-A5-F4** Save searches and get alerts | Save a search and get told when a matching home is listed or a saved home changes. | next | P1, P2 |
| **HOME-A5-F5** Shortlist, take notes and compare | Keep a shortlist with private notes, share it with a partner and compare homes side by side. | next | P1, P2 |
| **HOME-A5-F6** Check travel times | See how long it takes to get to work, school or family from a home by car, bus, bike or on foot. | next | P1 |
| **HOME-A5-F7** Work out what you can afford | Use plain calculators for repayments and the upfront costs of buying, clearly marked as not financial advice. | next | P1, P2 |
| **HOME-A5-F8** Share a listing | Share a listing by link, message or a printed sheet. | core | P1, P2 |
| **HOME-A5-F9** Search sold homes | Search recent sales and deadline or auction results nearby, where the data may be shown. | next | P1 |
| **HOME-A5-F10** Find a home that suits your access needs | See step-free entry, level access and accessible bathrooms on listings, filter by them, and ask for an accessible viewing. | next | P1, P2, P3 |
| **HOME-A5-F11** See facts for investors | See the current rent, when the tenancy ends and the Healthy Homes status of a tenanted home. | later | P1 |
| **HOME-A5-F12** Post what you are looking for | As a buyer, post a private brief of the home you want, which owners who are not yet selling can answer. | later | P1 |

### Parts for HOME-A5

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A5-F1-P1 | **Home search needs**: Sets what home search needs from the kit's search engine choice, such as drawn areas, school zones and many filters. | research | small | KIT-A6-F1-P1 |  |
| HOME-A5-F1-P2 | **Home search index**: Indexes live listings by place, price and features on the kit's search service, using the hidden search price for listings with no shown price, and updates within seconds of a change. | service | large | HOME-A5-F1-P1, KIT-A6-F1-P2, HOME-A2-F2-P1, svc-maps, HOME-A2-F3-P5 |  |
| HOME-A5-F1-P3 | **Search screens**: Map and list search that works on phones, with filters that read well to screen readers. | interface | large | HOME-A5-F1-P2, svc-maps |  |
| HOME-A5-F1-P4 | **Home ranking settings**: Uses the kit's published ranking rules so results follow the buyer's choice only, never payment, and say how the order was worked out. | data | small | KIT-A6-F1-P3, HOME-A5-F1-P2 |  |
| HOME-A5-F2-P1 | **Listing page**: Shows every part of a listing in a fast, accessible page that prints and shares cleanly. | interface | large | HOME-A2-F2-P1, HOME-A3-F3-P4, HOME-A2-F5-P1 |  |
| HOME-A5-F2-P2 | **Key facts block**: Shows the facts a country's rules require up front, with each fact's source. | interface | small | HOME-A5-F2-P1, HOME-A2-F3-P2 |  |
| HOME-A5-F3-P1 | **Area data sources**: Lists open sources for natural hazards, such as flood, liquefaction and coastal hazards, school zones and transport, with licences. Part of one data research piece with HOME-A1-F3-P1, HOME-A2-F1-P3 and HOME-A4-F1-P1. | research | medium |  |  |
| HOME-A5-F3-P2 | **Area data layers**: Loads and refreshes each area dataset and links it to properties, showing source and date. | integration | large | HOME-A5-F3-P1, svc-maps, HOME-A2-F1-P2 |  |
| HOME-A5-F3-P3 | **Area tab on the listing**: Shows the layers on a map with plain words on what each one means and does not mean. | interface | medium | HOME-A5-F3-P2, HOME-A5-F2-P1, HOME-A5-F3-P4 |  |
| HOME-A5-F3-P4 | **Aerial photos and boundary lines**: Adds LINZ's open aerial imagery and property boundary lines to the kit's maps, so buyers can see a section's boundaries on an aerial photo. Spare Rent, Spare Stay and Spare Work need it too, so it belongs in the kit beside KIT-A7-F1. | integration | medium | KIT-A7-F1-P2, HOME-A2-F1-P3 |  |
| HOME-A5-F4-P1 | **Home saved search fields**: Adds home filters and changes worth watching, such as price drops, to the kit's saved searches. | library | small | KIT-A6-F8-P1, HOME-A5-F1-P2 |  |
| HOME-A5-F4-P2 | **Listing alerts**: Sends alerts by push, email or SMS as the buyer chooses, at the pace they choose. | service | small | HOME-A5-F4-P1, svc-messaging |  |
| HOME-A5-F5-P1 | **Shortlist notes and sharing**: Adds private notes and sharing with chosen people to the kit's favourites, synced across devices. | library | small | KIT-A5-F12-P1, kit-sync, HOME-A16-F4-P4 |  |
| HOME-A5-F5-P2 | **Side-by-side compare**: Puts chosen homes' facts, costs and area data in one table. | interface | medium | HOME-A5-F5-P1, HOME-A5-F2-P1 |  |
| HOME-A5-F6-P1 | **Travel times**: Works out travel times from a home to places the buyer names, using the kit's open routing. | library | medium | svc-maps |  |
| HOME-A5-F7-P1 | **Buyer calculators**: Repayment, deposit and buying cost calculators that never recommend a lender or product. | interface | small |  |  |
| HOME-A5-F7-P2 | **Not financial advice**: Wording and limits that keep calculators general, checked against financial advice law. | legal | small | HOME-A5-F7-P1 |  |
| HOME-A5-F8-P1 | **Share links**: Makes clean share links and previews that work in messages and on social networks. | library | small | HOME-A5-F2-P1 |  |
| HOME-A5-F8-P2 | **Printable listing sheet**: Makes a one-page PDF of the listing for buyers to print. | library | small | engine-pdf, HOME-A5-F2-P1 |  |
| HOME-A5-F9-P1 | **Sold homes search**: Adds sold homes and results to search using only prices that may be shown, such as earlier Spare Home sales, the open sales dataset and licensed data. | library | medium | HOME-A5-F1-P2, HOME-A4-F1-P1, HOME-A4-F4-P1 |  |
| HOME-A5-F10-P1 | **Accessible-home facts**: Adds facts such as step-free entry, level access, door widths and an accessible bathroom to the property record. | data | small | HOME-A2-F1-P2 |  |
| HOME-A5-F10-P2 | **Access filters**: Lets buyers filter search by the accessible-home facts. | library | small | HOME-A5-F1-P2, HOME-A5-F10-P1 |  |
| HOME-A5-F10-P3 | **Accessible viewing requests**: Lets a buyer ask for a viewing that suits their access needs, such as a quiet time or help at a step. | library | small | HOME-A7-F2-P1 |  |
| HOME-A5-F11-P1 | **Tenanted home facts**: Shows the current rent, the tenancy end date and Healthy Homes compliance status, shared within the tenancy and privacy rules. | data | small | HOME-A7-F5-P1, HOME-A16-F4-P4, HOME-A2-F1-P2 |  |
| HOME-A5-F12-P1 | **Buyer briefs**: Holds buyers' briefs without their contact details, lets checked owners answer through the masked chat, and offers briefs only if HM01 finds they are not agency work. | library | medium | HOME-A8-F1-P1, HOME-A2-F10-P1, HOME-A16-F1-P1 |  |

## HOME-A6 Reach and marketing

Buyers' attention lives on the big property sites, so sellers need reach beyond Spare Home. This area covers feeding other sites, printed and social marketing, and showing sellers how their listing is doing.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A6-F1** Appear on other property sites | Send your listing to other property sites that take private listings, paying only what they charge. | next | P1, P2, P3, P4, P5 |
| **HOME-A6-F2** Print brochures and flyers | Make a brochure, flyer or open home handout from the listing in one step. | core | P1, P2 |
| **HOME-A6-F3** Put up a sign | Order a for-sale signboard with a QR code that opens the listing. | core | P1, P2, P3 |
| **HOME-A6-F4** Share on social networks | Post the listing to social networks and Spare Social with ready-made images and text. | next | P1, P2 |
| **HOME-A6-F5** See how your listing is doing | See views, saves, enquiries, viewings and offers over time, and what buyers look at most. | next | P1, P2 |
| **HOME-A6-F6** Sell quietly or tease first | Share a private link before going public, or sell only to people you invite. | next | P1 |
| **HOME-A6-F7** Be found through web search | Have your listing found through web search and opened by anyone without signing in, at an address that keeps working after the sale. | core | P1, P2, P3 |

### Parts for HOME-A6

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A6-F1-P1 | **Which sites take private listings**: Finds which sites accept private sellers' listings, at what price and on what terms. | research | small |  | [HM02](STATUS.md) |
| HOME-A6-F1-P2 | **Shared listing feed service**: One feed service that sends listings, changes and withdrawals to other sites, with each site's and country's feed format as a plugin. It belongs in the kit, built once for Spare Home, Spare Rent (RENT-A3-F5-P2), Spare Stay (STAY-A21-F4-P1) and Spare Market (MKT-A14-F9-P2). | integration | large | kit-plugin, kit-ops, kit-market |  |
| HOME-A6-F1-P3 | **Where my listing appears**: Shows the seller each site the listing is on, its status there and what it cost. | interface | small | HOME-A6-F1-P2, HOME-A6-F1-P5 |  |
| HOME-A6-F1-P4 | **Pass-through site costs**: Charges the seller exactly what each outside site charges, shown before they choose. | library | small | HOME-A6-F1-P2, HOME-A14-F2-P1, HOME-A6-F1-P5 |  |
| HOME-A6-F1-P5 | **Home feed formats**: Maps Home's listing fields, status steps and media licences to each property site's feed format, as plugins on the shared feed service, sending only media the licence allows. | data | small | HOME-A6-F1-P2, HOME-A6-F1-P1, HOME-A2-F2-P1, HOME-A2-F4-P1, HOME-A3-F8-P2 |  |
| HOME-A6-F2-P1 | **Brochure maker**: Lays out photos, plans and facts in our own print designs and makes a tagged, accessible print-ready PDF. | library | medium | engine-pdf, engine-text, HOME-A3-F3-P4, HOME-A17-F1-P4 |  |
| HOME-A6-F2-P2 | **Printers in the catalogue**: Adds printers as entries on the one booking flow and sends them the print-ready file. | integration | small | HOME-A6-F2-P1, HOME-A12-F1-P2 |  |
| HOME-A6-F3-P1 | **QR codes and short links**: Makes QR codes and short links that open the listing and count scans without tracking people, a tool every app needs that belongs in the kit. | library | small | HOME-A5-F2-P1, KIT-A20-F4-P1 |  |
| HOME-A6-F3-P2 | **Signboard artwork**: Makes signboard artwork in our own designs, sized for common boards. | library | small | engine-draw2d, engine-pdf, HOME-A6-F3-P1 |  |
| HOME-A6-F3-P3 | **Council sign rules**: Checks council rules on the size and placing of property signs. | research | small |  |  |
| HOME-A6-F4-P1 | **Social share pack**: Makes square and tall images and short text for posts, in our own designs. | library | small | engine-media, HOME-A5-F8-P1 |  |
| HOME-A6-F4-P2 | **Posting to networks**: Posts to Spare Social and other networks through their open sharing links or published interfaces. | integration | small | HOME-A6-F4-P1 |  |
| HOME-A6-F5-P1 | **Listing figures**: Counts views, saves and enquiries in totals only through the kit's private usage counts, without tracking individual buyers across sites. | library | medium | HOME-A2-F2-P1, kit-ops, KIT-A22-F8-P1 |  |
| HOME-A6-F5-P2 | **Seller dashboard**: Shows the figures as simple charts, with tips when a listing is getting little interest. | interface | small | HOME-A6-F5-P1 |  |
| HOME-A6-F6-P1 | **Private and coming-soon listings**: Hides a listing from search and shares it only by link or invitation, with the same checks as a public one. | library | small | HOME-A2-F4-P1, HOME-A5-F2-P1 |  |
| HOME-A6-F7-P1 | **Public listing pages**: Serves every public listing as a fast page that anyone can open without signing in and search engines can index, with clean share previews, leaving out private and coming-soon listings, hidden addresses, the owner's identity and media the licence forbids. | interface | medium | HOME-A5-F2-P1, HOME-A5-F8-P1, HOME-A15-F5-P1, HOME-A3-F8-P2, HOME-A6-F7-P2 |  |
| HOME-A6-F7-P2 | **Web search basics**: Page titles, descriptions, structured listing data in an open vocabulary, site maps and tidy addresses for every marketplace's public pages. It belongs in the kit, built once with Spare Market's web search basics (MKT-A13-F5-P2) and used by Spare Rent and Spare Stay too. | library | medium | kit-market |  |
| HOME-A6-F7-P3 | **Addresses that last**: Keeps each listing's address working for good: after a sale or withdrawal it says so and shows only the facts, photos and price the seller and each file's licence allow, and a relisting reuses the same address. | library | small | HOME-A6-F7-P1, HOME-A2-F4-P1, HOME-A3-F8-P2 |  |

## HOME-A7 Viewings and open homes

Sellers host their own viewings. Spare Home books them, runs the open home register on a phone, keeps sellers and visitors safe, and gets the tenant's agreement before anyone enters a rented home.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A7-F1** Set viewing times | Set open home times and the hours you are free for private viewings. | core | P1, P2, P3 |
| **HOME-A7-F2** Book a viewing | Book a private viewing or register for an open home, and get reminders. | core | P1, P2, P3 |
| **HOME-A7-F3** Run an open home | Check visitors in on a phone at the door, even with no signal, and see who came. | core | P1, P2, P3, P4 |
| **HOME-A7-F4** Stay safe at viewings | Choose to admit only checked buyers, share your viewing plans with a friend, and call for help fast. | core | P1, P2, P3 |
| **HOME-A7-F5** Show a home with tenants in it | Tell tenants about the sale in writing, get their agreement before anyone enters, and book viewings, photos and visits only at times they agree. | core | P1, P2, P3, P4, P5, P6 |
| **HOME-A7-F6** Plan a day of open homes | As a buyer, see the open homes you have saved on a map and plan a route between them. | next | P1 |
| **HOME-A7-F7** View by live video | Walk a distant buyer through the home on a live video call. | next | P1 |
| **HOME-A7-F8** Hear what buyers thought | Ask buyers for feedback after a viewing and see it in one place. | next | P1 |
| **HOME-A7-F9** Have a host show the home | Book someone to host your viewings or open homes, if the agency line allows it. | later | P1 |
| **HOME-A7-F10** Warn visitors about hazards | Tell visitors and professionals about hazards before they come, such as a dog, a pool, building work, unsafe areas or known meth contamination. | next | P1 |

### Parts for HOME-A7

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A7-F1-P1 | **Viewing slots**: Holds open home slots and private viewing hours on the kit's availability calendars and blocks times already booked. | library | medium | KIT-A5-F5-P1, kit-market, HOME-A2-F2-P1 |  |
| HOME-A7-F1-P2 | **Open home scheduler**: Lets the seller add, move or cancel open homes and tells everyone booked or interested. | interface | medium | HOME-A7-F1-P1, svc-messaging |  |
| HOME-A7-F1-P3 | **Viewing calendar sync**: Shares viewings to the seller's own calendar through the kit's calendar import and export. | integration | small | HOME-A7-F1-P1, KIT-A5-F5-P2 |  |
| HOME-A7-F2-P1 | **Viewing booking**: Books a slot, asks the seller to approve if they choose, and sends confirmation and reminders. | interface | medium | HOME-A7-F1-P1, svc-messaging, HOME-A1-F2-P2, HOME-A16-F1-P1 |  |
| HOME-A7-F2-P2 | **Changes and no-shows**: Handles cancelling, moving and no-shows, and frees the slot for others. | library | small | HOME-A7-F2-P1 |  |
| HOME-A7-F2-P3 | **Bots and bulk bookings**: Applies the kit's member-voted rules for automated booking to viewing slots and enquiries, so scripts and agents cannot grab scarce viewings or send bulk enquiries. | library | small | KIT-A6-F4-P3, KIT-A17-F4-P1, HOME-A7-F2-P1 |  |
| HOME-A7-F3-P1 | **Visitor register**: Records who came and when, from a check-in code or the seller's phone, as one register shared with Spare Rent's open homes that belongs in the kit. | library | medium | kit-ops, HOME-A7-F2-P1 |  |
| HOME-A7-F3-P2 | **Door check-in screen**: A simple phone screen for checking visitors in, kept working offline by Home's one offline store (HOME-A17-F4-P1). | interface | medium | HOME-A7-F3-P1, HOME-A17-F4-P1 |  |
| HOME-A7-F3-P3 | **Register privacy notice**: Tells visitors what is collected, why, and when it is deleted, as privacy law requires. | legal | small | HOME-A7-F3-P1 |  |
| HOME-A7-F3-P4 | **Check-in codes that cannot be swapped**: Shows a check-in code on the seller's own screen that changes for each open home, never a printed sticker, so a swapped code cannot send visitors to a fake page. | library | small | HOME-A7-F3-P2 |  |
| HOME-A7-F4-P1 | **Checked buyers only**: Lets a seller accept bookings only from people with a confirmed identity. | library | small | HOME-A1-F2-P2, HOME-A7-F2-P1 |  |
| HOME-A7-F4-P2 | **Viewing safety settings**: Uses the kit's trusted contact link and emergency button for viewings and adds a check-in timer for the end of each viewing. | library | medium | KIT-A10-F2-P1, KIT-A10-F1-P1, svc-messaging |  |
| HOME-A7-F4-P3 | **Hosting safely**: Short guidance on securing valuables, medicines and keys, viewing in pairs, and keeping visitors safe, such as shutting dogs away, closing pool gates and marking unsafe areas. | data | small |  |  |
| HOME-A7-F5-P1 | **Tenancy rules for selling**: Encodes the duty to tell tenants in writing, the tenant's permission before anyone enters to take photos, value or inspect, and the tenant's right to refuse photos of their belongings, decline open homes and auctions at the home, ask for viewings by appointment only, be present and ask for a rent reduction. It also holds the notice periods after a sale, as one tenancy rules pack shared with Spare Rent that belongs in the kit. | data | medium | KIT-A19-F9-P1, HOME-A16-F6-P1 | [RT01](../spare-rent/STATUS.md) |
| HOME-A7-F5-P2 | **Tenant's view**: Lets tenants receive notices, approve or decline each viewing, open home, photo visit and professional visit, refuse photos of their belongings, ask to be present, ask for a rent reduction, and sign the agreed schedule. | interface | medium | HOME-A7-F5-P1, HOME-A7-F1-P1, svc-messaging, HOME-A1-F1-P4, HOME-A16-F4-P4 |  |
| HOME-A7-F5-P3 | **Written notice to tenants**: Makes and sends the written notice the law requires and keeps proof it was sent. | library | small | HOME-A7-F5-P1, svc-messaging |  |
| HOME-A7-F5-P4 | **Entry waits for the tenant**: Holds every photographer, valuer, inspector and viewing booking at a tenanted home until the tenant agrees to the time, blocks open homes and on-site auctions the tenant has declined, and keeps photos of the tenant's belongings off the listing if they refuse them. | library | small | HOME-A7-F5-P1, HOME-A7-F5-P2, HOME-A7-F1-P1, HOME-A12-F1-P2 |  |
| HOME-A7-F5-P5 | **Tenancy facts on the listing**: Shows whether the home is tenanted or vacant, periodic or fixed term, when a fixed term ends, and whether it is sold with the tenancy or with vacant possession, with nothing about the tenants themselves. | data | small | HOME-A7-F5-P1, HOME-A2-F2-P1 |  |
| HOME-A7-F5-P6 | **Vacant possession dates**: Links the tenancy notice to the settlement date: at least 42 days' written notice to end a periodic tenancy once a sale needs vacant possession, 90 days' notice to market the home empty, and fixed terms passing to the buyer. It warns when a settlement date comes before the tenancy can lawfully end. | library | small | HOME-A7-F5-P1, HOME-A11-F1-P1 |  |
| HOME-A7-F6-P1 | **Open home route planner**: Lists saved homes' open home times on a map and orders them into a route that fits. | interface | medium | HOME-A5-F5-P1, HOME-A7-F1-P1, HOME-A5-F6-P1 |  |
| HOME-A7-F7-P1 | **Live video viewings**: Books and runs a live video walk-through on the kit's call service, with no recording unless both agree. | integration | medium | KIT-A8-F11-P2, HOME-A7-F2-P1 |  |
| HOME-A7-F8-P1 | **Viewing feedback**: Sends a short optional question set after a viewing and groups the answers for the seller. | library | small | HOME-A7-F2-P1, svc-messaging, HOME-A16-F1-P1 |  |
| HOME-A7-F9-P1 | **Viewing hosts**: Adds viewing and open home hosts as entries on the one booking flow, offered only if HM01 finds that showing a home for a seller is not agency work, or else only through licensed agents. | data | small | HOME-A12-F1-P2, HOME-A16-F1-P1, HOME-A7-F2-P1 |  |
| HOME-A7-F10-P1 | **Home hazard notes**: Adds hazard notes to viewings and professional bookings on the kit's hazard notes, shown before a visit is confirmed. | library | small | KIT-A24-F5-P1, HOME-A7-F2-P1, HOME-A12-F1-P2 |  |

## HOME-A8 Enquiries and the buyer register

An agent's buyer list and phone are a big part of what a commission buys. Spare Home gives the seller a private inbox, a simple register of interested buyers and fair ways to keep them all informed, without sharing phone numbers.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A8-F1** Message the seller | Ask the seller a question through the app without either side sharing a phone number or email. | core | P1, P2 |
| **HOME-A8-F2** Answer common questions once | Publish answers to common questions, and get draft replies made only from facts you have approved. | next | P1, P2 |
| **HOME-A8-F3** Keep a buyer register | See every interested buyer, what they viewed, what documents they asked for and whether they have offered. | core | P1, P2, P3, P4 |
| **HOME-A8-F4** Tell every interested buyer at once | Send the same update to every interested buyer at the same moment, such as a price change, an offer or a deadline. | core | P1 |
| **HOME-A8-F5** Show you are ready to buy | As a buyer, share proof of finance and your lawyer's details with a seller if you choose. | next | P1 |
| **HOME-A8-F6** Block and report | Block anyone who is rude or unsafe, and report them. | core | P1 |

### Parts for HOME-A8

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A8-F1-P1 | **Private contact for listings**: Uses the kit's chat and masked calls so neither side sees the other's number or email until they choose. | library | small | KIT-A8-F1-P2, KIT-A8-F2-P1 |  |
| HOME-A8-F1-P2 | **Enquiry inbox**: One inbox for every question about a listing, with quick replies and files. | interface | medium | HOME-A8-F1-P1, svc-messaging, HOME-A2-F2-P1 |  |
| HOME-A8-F2-P1 | **Listing questions and answers**: Lets the seller publish answers to common questions on the listing so they are asked once. | library | small | HOME-A5-F2-P1 |  |
| HOME-A8-F2-P2 | **Draft replies**: Drafts a reply from the listing's approved facts and documents for the seller to edit and send, treats buyers' messages as data and never as instructions, and never discusses price for the seller. | library | medium | kit-mcp, HOME-A8-F2-P1, HOME-A18-F4-P2, HOME-A16-F1-P1, HOME-A18-F4-P5, KIT-A17-F5-P1 |  |
| HOME-A8-F3-P1 | **Buyer register**: Builds a private list of interested buyers from enquiries, viewings, document requests and offers. | library | medium | HOME-A8-F1-P2, HOME-A7-F3-P1, kit-ops, HOME-A16-F1-P1 |  |
| HOME-A8-F3-P2 | **Buyer register screen**: Shows the register with status, notes and next steps for each buyer. | interface | medium | HOME-A8-F3-P1, HOME-A8-F3-P4 |  |
| HOME-A8-F3-P3 | **Register privacy limits**: Keeps buyers' details only for this sale and deletes them after, as privacy law requires. | legal | small | HOME-A8-F3-P1 |  |
| HOME-A8-F3-P4 | **Lawful criteria only**: Shows and filters buyers only by lawful criteria such as finance, conditions, dates and overseas-buyer status, never by attributes protected by human rights law. | library | small | HOME-A8-F3-P1, HOME-A16-F5-P1 |  |
| HOME-A8-F4-P1 | **Fair updates to all buyers**: Sends one message to every registered buyer at the same time and keeps a record of who was told what and when, with personal details kept erasable by the kit's method. | library | small | HOME-A8-F3-P1, svc-messaging, HOME-A16-F1-P1, KIT-A15-F1-P6 |  |
| HOME-A8-F5-P1 | **Buyer readiness card**: Holds a buyer's lawyer, finance approval and timing, shared only with sellers they pick. | library | small | svc-accounts, kit-store, HOME-A10-F9-P1 |  |
| HOME-A8-F6-P1 | **Block and report on listings**: Uses the kit's chat reporting and blocking and attaches the listing to the report. | library | small | KIT-A8-F6-P1, KIT-A10-F4-P1, HOME-A8-F1-P1 |  |

## HOME-A9 Documents and disclosure

Buyers and their lawyers need the title, council records, reports and the seller's honest disclosures. Spare Home helps the seller gather them, says what the law requires them to disclose, and shares documents with buyers safely.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A9-F1** Keep every document in one place | Store the title, council records, consents, reports, plans and warranties for the home in one private vault. | core | P1, P2 |
| **HOME-A9-F2** Order the title and official records | Order the record of title, the council's property report and the natural hazard claims history from inside the app at the issuer's price. | core | P1, P2, P3, P4 |
| **HOME-A9-F3** Know what you must disclose | Answer a guided checklist of what the law requires a seller to disclose and record your answers. | core | P1, P2, P3 |
| **HOME-A9-F4** Sell a unit with a body corporate | Collect the body corporate's disclosure statements and give them to buyers on time. | core | P1, P2, P3 |
| **HOME-A9-F5** Share documents with buyers | Give each buyer access to the documents you choose and see who opened what. | core | P1, P2 |
| **HOME-A9-F6** Share reports buyers can rely on | Commission a building or other report once and let buyers rely on it, if the report writer agrees. | next | P1, P2 |
| **HOME-A9-F7** List what stays with the house | List the chattels and fittings that stay, with photos. | core | P1 |

### Parts for HOME-A9

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A9-F1-P1 | **Shared document vault**: Stores documents with the property record, sorted by type, with versions and dates, as one vault shared with Spare Rent (replacing RENT-A2-F6-P1) that belongs in the kit. | library | medium | kit-store, HOME-A2-F1-P2 |  |
| HOME-A9-F1-P2 | **Document types**: Lists each document type, what it shows, who issues it and how old it may be, per country, including the Natural Hazards Commission claims history. | data | small | HOME-A9-F1-P1 |  |
| HOME-A9-F2-P1 | **Title ordering**: Orders the official record of title from LINZ and files it in the vault. | integration | medium | HOME-A9-F1-P1, HOME-A1-F3-P1, HOME-A14-F2-P1 |  |
| HOME-A9-F2-P2 | **How each council supplies reports**: Lists how each council takes orders for land information reports, the price and the time taken. | research | medium |  |  |
| HOME-A9-F2-P3 | **Council report ordering**: Orders the council's land information report where the council allows it, or guides the seller through the council's own form. | integration | large | HOME-A9-F2-P2, HOME-A9-F1-P1, HOME-A14-F2-P1 |  |
| HOME-A9-F2-P4 | **Natural hazard claims history**: Helps the seller request the home's claims history from the Natural Hazards Commission, formerly EQC, before listing and files it in the vault. | integration | small | HOME-A9-F1-P1 |  |
| HOME-A9-F3-P1 | **Disclosure rules**: Encodes what sellers must disclose, such as known weathertightness problems, unconsented work and unrepaired natural hazard damage, per country and property type, in the rules-pack format. | data | large | HOME-A2-F6-P1, HOME-A16-F6-P1 | [HM05](STATUS.md) |
| HOME-A9-F3-P2 | **Disclosure checklist**: Asks the seller each question in plain words and turns the answers into a disclosure statement for buyers. | interface | medium | HOME-A9-F3-P1, HOME-A9-F1-P1, HOME-A17-F1-P4 |  |
| HOME-A9-F3-P3 | **Lawyer check of the questions**: A lawyer's review of the questions and wording, repeated when the law changes. | legal | medium | HOME-A9-F3-P1, HOME-A19-F9-P1 |  |
| HOME-A9-F4-P1 | **Unit title disclosure pack**: Gathers the pre-contract statement, minutes, accounts and levies from the body corporate and gives them to every buyer before they sign. | library | medium | HOME-A9-F1-P1, HOME-A9-F3-P1 |  |
| HOME-A9-F4-P2 | **Unit title deadlines**: Tracks when the pre-settlement and any extra statements are due and warns the seller early. | library | small | HOME-A9-F4-P1, HOME-A11-F1-P1 |  |
| HOME-A9-F4-P3 | **Body corporate manager's side**: Sends the request for disclosure statements to the body corporate manager, takes their fee at the price they publish, lets them upload straight into the vault, and tracks the pre-settlement statement so it arrives on time. | library | medium | HOME-A9-F4-P1, HOME-A9-F4-P2, HOME-A12-F1-P5, HOME-A12-F8-P1 |  |
| HOME-A9-F5-P1 | **Document sharing**: Gives chosen buyers, or anyone who asks, access to chosen documents and records each opening. | library | medium | HOME-A9-F1-P1, HOME-A8-F3-P1, HOME-A1-F8-P2 |  |
| HOME-A9-F5-P2 | **Buyer watermarks**: Stamps each shared copy with the buyer's name and date to discourage passing it on, keeping the copy accessible. | library | small | engine-pdf, HOME-A9-F5-P1, HOME-A17-F1-P4 |  |
| HOME-A9-F6-P1 | **Relying on seller reports**: Finds whether and how report writers let buyers rely on a report the seller paid for, and what that costs. | research | small |  |  |
| HOME-A9-F6-P2 | **Pre-sale reports in the catalogue**: Adds building inspections, meth tests and other reports as entries on the one booking flow, delivered straight into the vault. | integration | small | HOME-A12-F1-P2, HOME-A9-F6-P1, HOME-A12-F8-P1 |  |
| HOME-A9-F7-P1 | **Chattels list**: Builds the list of what stays, with photos, for the listing and for the lawyers' agreement. | library | small | HOME-A9-F1-P1, HOME-A2-F2-P1 |  |

## HOME-A10 Offers and methods of sale

The seller sees every offer in one place and runs a fair process. Offers come as documents from buyers' lawyers; the binding agreement is drawn up and signed through lawyers, never by the app, and the app never negotiates for anyone. Every tool here does more than publish an advert, so each waits on the agency-line research (HM01).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A10-F1** Register interest | Tell the seller you intend to make an offer, so you hear about any other offers or deadlines. | core | P1 |
| **HOME-A10-F2** Record every offer | As a seller, see every offer in one place with its price, conditions, dates and expiry; as a buyer, see where your offer stands. | core | P1, P2, P3, P4, P5, P6 |
| **HOME-A10-F3** Run a fair multi-offer | When more than one buyer offers, tell every buyer, set one deadline and treat everyone the same. | core | P1, P2 |
| **HOME-A10-F4** Compare offers | See offers side by side by price, conditions, deposit, finance and dates. | core | P1 |
| **HOME-A10-F5** Sell by deadline or tender | Set a date when all offers are opened together, with the option to accept one early. | next | P1, P2 |
| **HOME-A10-F6** Sell by auction | Hold an auction run by a licensed auctioneer, in person or online, with registered bidders. | later | P1, P2, P3, P4 |
| **HOME-A10-F7** Record counter-offers | Record each counter-offer your lawyer sends or receives, with its expiry. | core | P1 |
| **HOME-A10-F8** Accept an offer | Record that you have signed an agreement through your lawyer, tell other buyers, and move to the conditions stage. | core | P1, P2, P3 |
| **HOME-A10-F9** Buy together or through an entity | Make an offer as joint buyers, a trust, a company or a nominee, with each person's identity and authority checked. | core | P1 |

### Parts for HOME-A10

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A10-F1-P1 | **Registered interest**: Records a buyer's intent to offer and adds them to every fair update. | library | small | HOME-A8-F3-P1, HOME-A8-F4-P1, HOME-A16-F1-P1 |  |
| HOME-A10-F2-P1 | **Offer register design**: Decides how offers are recorded and shown fairly, leaving the contract to lawyers. | research | small |  | [HM03](STATUS.md) |
| HOME-A10-F2-P2 | **Offer register**: Records who offered what, when, on what conditions and until when, against the buying person or entity, as unchangeable operations, with personal details kept erasable by the kit's method. | library | medium | HOME-A10-F2-P1, kit-ops, HOME-A2-F2-P1, HOME-A16-F1-P1, HOME-A10-F9-P1, HOME-A2-F3-P3, KIT-A15-F1-P5, KIT-A15-F1-P6 |  |
| HOME-A10-F2-P3 | **Offers from lawyers**: Takes a signed offer document from the buyer or their lawyer and drafts a summary of its key terms, labelled unconfirmed until a lawyer confirms it. The document's text is treated as data by the kit's hidden instruction guards and never as instructions to any AI step. | integration | medium | HOME-A10-F2-P2, HOME-A9-F1-P1, HOME-A11-F3-P1, HOME-A10-F2-P6, KIT-A17-F5-P1 |  |
| HOME-A10-F2-P4 | **Seller's offers screen**: Shows all offers together with expiry clocks, and tells the seller when one is about to lapse. | interface | medium | HOME-A10-F2-P2, HOME-A11-F1-P1 |  |
| HOME-A10-F2-P5 | **Buyer's offer status**: Shows a buyer whether their offer was received, seen, countered, accepted or declined. | interface | small | HOME-A10-F2-P2 |  |
| HOME-A10-F2-P6 | **Standard agreement form licence**: Finds the copyright and licence terms of the standard NZ sale and purchase agreement form, so Spare Home never copies its text or layout into guides, templates or offer screens. | research | small |  |  |
| HOME-A10-F3-P1 | **Multi-offer process**: A published process: every buyer told at once, one deadline, a chance for best offers, and acknowledgements recorded. | data | medium | HOME-A10-F2-P1, HOME-A16-F1-P1 |  |
| HOME-A10-F3-P2 | **Running a multi-offer**: Sends the notices, collects acknowledgements and opens offers together at the deadline. | library | small | HOME-A10-F3-P1, HOME-A8-F4-P1, HOME-A10-F2-P2, HOME-A16-F1-P1 |  |
| HOME-A10-F4-P1 | **Offer comparison**: Lays offers out side by side without ranking or recommending any of them. | interface | medium | HOME-A10-F2-P2, HOME-A16-F1-P2, HOME-A8-F3-P4, HOME-A16-F1-P1 |  |
| HOME-A10-F5-P1 | **Deadline and tender mode**: Keeps offers sealed until the set time, then opens them together for the seller, showing every deadline in NZ time from the kit's NZ country pack, including the Chatham Islands, and in the buyer's own time zone. | library | medium | HOME-A10-F2-P2, HOME-A11-F1-P1, HOME-A10-F3-P1, HOME-A16-F1-P1, KIT-A19-F9-P1 |  |
| HOME-A10-F5-P2 | **Tender and deadline rules**: Checks the legal and practical rules for deadline sales and tenders in each country. | research | small |  |  |
| HOME-A10-F6-P1 | **Auction law**: Sets out who may run a home auction, rules on seller bids and reserves, and online auction duties. | research | medium |  |  |
| HOME-A10-F6-P2 | **Bidder registration**: Registers and checks bidders before the auction, with their lawyer named. | library | medium | HOME-A1-F2-P1, HOME-A8-F5-P1, HOME-A10-F6-P1, HOME-A16-F1-P1, HOME-A10-F9-P1 |  |
| HOME-A10-F6-P3 | **Home auction room on the shared auction engine**: Runs a home auction with the booked licensed auctioneer, in the room or online, on one live auction engine shared with Spare Market's auctions (MKT-A6): the server clock, live bids, public bid history and fake bid checks. The engine belongs in the kit; Home adds the auctioneer's controls and registered bidders only. | library | large | HOME-A10-F6-P2, HOME-A16-F1-P1, kit-market, kit-ops, svc-messaging, KIT-A7-F9-P1 |  |
| HOME-A10-F6-P4 | **Fake and undisclosed seller bids**: Flags bids from accounts linked to the seller or to each other, on top of the shared engine's fake bid checks, and allows a bid for the seller only when the auctioneer makes it openly as the auction rules allow, so undisclosed seller bids are caught. | library | medium | HOME-A10-F6-P3, HOME-A10-F6-P1 |  |
| HOME-A10-F7-P1 | **Counter-offer record**: Records each counter-offer document and its expiry against the original offer. | library | small | HOME-A10-F2-P2, HOME-A10-F2-P3, HOME-A16-F1-P1 |  |
| HOME-A10-F8-P1 | **Recording a sale agreement**: Records the signed agreement from the lawyers, marks the listing under offer and tells other buyers fairly. | library | small | HOME-A10-F2-P2, HOME-A2-F4-P1, HOME-A8-F4-P1, HOME-A16-F1-P1, HOME-A1-F8-P2 |  |
| HOME-A10-F8-P2 | **Electronic signing of land contracts**: Answers one question for the kit's electronic signature rules research (KIT-A25-F1-P1): when electronic signatures are valid for land sale agreements in NZ and other countries, so the app records the lawyers' signed agreements correctly. | research | small |  |  |
| HOME-A10-F8-P3 | **Back-up offers**: Keeps other buyers' interest on record in case the first agreement falls through. | library | small | HOME-A10-F2-P2, HOME-A10-F1-P1, HOME-A16-F1-P1 |  |
| HOME-A10-F9-P1 | **Buying entities**: Records offers against the buying entity, such as joint buyers, a trust or a company, with each person's identity check and evidence of their authority to sign for it, on the shared model of who may act for whom (HOME-A1-F4-P1) rather than the kit's organisation model, which is marked next. | library | medium | svc-accounts, HOME-A1-F2-P1, HOME-A1-F4-P1 |  |

## HOME-A11 From agreement to settlement

After the agreement is signed, both sides track conditions and dates, the lawyers handle the deposit and the money, and the move happens on settlement day. Spare Home keeps everyone on the same page and never touches the house money.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A11-F1** Track conditions and dates | See every condition and due date in the agreement, with reminders before each one. | core | P1, P2, P3, P4, P5 |
| **HOME-A11-F2** Keep the house money safe | See where the deposit and the price are held, by whom, and get warned about payment scams. | core | P1, P2, P3, P4 |
| **HOME-A11-F3** Give the lawyers a shared workspace | Invite both lawyers to see the file, upload the agreement and confirm conditions. | core | P1, P2 |
| **HOME-A11-F4** Do the pre-settlement inspection | Book the buyer's final inspection and check off chattels and agreed repairs. | core | P1 |
| **HOME-A11-F5** Settle and move | Follow a settlement day checklist for keys, meter readings, utilities and moving out. | core | P1, P2, P3 |
| **HOME-A11-F6** Hand over a tenanted home | Pass the tenancy, bond and tenancy records to the new owner, and the new owner's details to the tenant, as the law requires. | core | P1, P2 |
| **HOME-A11-F7** Recover when a sale falls through | When a condition is not met and the agreement ends, relist quickly and tell back-up buyers. | core | P1 |
| **HOME-A11-F8** Close the sale | Mark the home sold, rate the professionals and archive the sale. | core | P1 |
| **HOME-A11-F9** Keep a record for tax | Download a sale record with dates, price and costs for your accountant, without tax advice from the app. | next | P1, P2 |
| **HOME-A11-F10** Get help moving | Book movers, cleaners and storage at published prices. | later | P1 |
| **HOME-A11-F11** Know where to take a dispute about the sale | Find the right place for a dispute about the sale itself, which Spare Home does not decide. | core | P1 |
| **HOME-A11-F12** See linked sales | See both sets of dates when a buyer must sell first or the seller is also buying, and both sales are on Spare Home. | later | P1 |

### Parts for HOME-A11

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A11-F1-P1 | **Legal date engine**: Works out due dates in working days as the signed agreement defines them, on the kit's working days and notice periods, skipping the Christmas period the agreement leaves out and the anniversary day of the region it names, and sends reminders. | library | medium | kit-ops, KIT-A19-F9-P2, HOME-A11-F1-P2, svc-messaging |  |
| HOME-A11-F1-P2 | **Agreement day rules**: Adds only what the standard sale agreement defines on top of the kit's NZ public holidays, regional anniversary days and working days, such as the Christmas period it leaves out of working days, kept in the NZ rules pack. | data | small | KIT-A19-F9-P1, KIT-A19-F9-P2 |  |
| HOME-A11-F1-P3 | **Conditions tracker**: Shows each condition, who must act, the due date and whether it is met, for both sides and their lawyers. | interface | medium | HOME-A11-F1-P1, HOME-A10-F8-P1 |  |
| HOME-A11-F1-P4 | **Lawyer confirms each date**: Shows each computed due date as unconfirmed until a party's lawyer confirms it, and says so on every reminder. | library | small | HOME-A11-F1-P1, HOME-A11-F3-P2 |  |
| HOME-A11-F1-P5 | **Late settlement dates**: Tracks a late settlement: the settlement notice a party's lawyer sends, the days it gives, and the date penalty interest runs from as the agreement sets it, each shown as unconfirmed until a lawyer confirms it. | library | small | HOME-A11-F1-P1, HOME-A11-F1-P4 |  |
| HOME-A11-F2-P1 | **House money stays with lawyers**: Product rules that stop deposits or the price ever being paid through Spare Home and point to the lawyers' trust accounts. | library | small | kit-ops |  |
| HOME-A11-F2-P2 | **Trust account rules**: Confirms how lawyers' and agents' trust accounts must hold deposits, and when they are released. | research | small |  |  |
| HOME-A11-F2-P3 | **Payment detail warnings**: Shows the lawyers' trust account details only after they pass the independent check (HOME-A11-F2-P4), and warns loudly if anyone sends new details, leaving message warnings to HOME-A15-F2-P1. | library | medium | HOME-A11-F2-P1, HOME-A11-F3-P1, HOME-A15-F2-P1, HOME-A1-F8-P2, HOME-A11-F2-P4 |  |
| HOME-A11-F2-P4 | **Trust account details checked another way**: Confirms a lawyer's trust account details through a channel that does not come from the account itself, such as a call-back to the firm's number as published on the Law Society register, and shows them as unchecked until then. | library | medium | HOME-A12-F2-P3, HOME-A11-F3-P1 |  |
| HOME-A11-F3-P1 | **Lawyer register check**: Uses the kit's public register checks and re-checks to confirm a lawyer or conveyancer holds a current practising certificate, and HOME-A12-F2-P3 to prove the account holder is that person. | integration | small | KIT-A2-F2-P1, KIT-A2-F3-P1, HOME-A12-F2-P3 |  |
| HOME-A11-F3-P2 | **Lawyers' workspace**: Gives each side's lawyer the agreement, documents, dates and messages for this sale only. | interface | medium | HOME-A11-F3-P1, HOME-A1-F6-P1, HOME-A9-F1-P1, HOME-A11-F1-P3, HOME-A16-F4-P4 |  |
| HOME-A11-F4-P1 | **Final inspection checklist**: Books the inspection and lets the buyer tick off chattels and repairs with photos. | interface | small | HOME-A7-F2-P1, HOME-A9-F7-P1 |  |
| HOME-A11-F5-P1 | **Settlement day checklist**: Lists every step on the day for both sides, with key hand-over confirmed by both. | library | small | HOME-A11-F1-P3 |  |
| HOME-A11-F5-P2 | **Change of address checklist**: Reminds both sides to move power, internet and other accounts and to update the electoral roll, which the law requires after a move, IRD and their driver licence address, with no paid referrals. | data | small | HOME-A11-F5-P1 |  |
| HOME-A11-F5-P3 | **Insurance until possession**: Reminds the seller to keep the home insured until the buyer takes possession, since the standard agreement leaves the risk with the seller until then, and the buyer to have cover from that day, in wording checked by a lawyer. | data | small | HOME-A11-F5-P1, HOME-A13-F3-P2 |  |
| HOME-A11-F6-P1 | **Tenancy hand-over**: Guides the bond transfer through Tenancy Services, tells the tenant who the new owner is and when they take over, and helps the new owner give the tenant their name, contact details and address for service, as one flow shared with Spare Rent (RENT-A2-F8-P1) on the shared property record. | library | small | HOME-A7-F5-P1, HOME-A7-F5-P3, HOME-A2-F1-P2, HOME-A7-F5-P6 |  |
| HOME-A11-F6-P2 | **Tenancy records to the buyer**: Passes the tenancy agreement, which the law requires, and with the seller's agreement the Healthy Homes records, insulation statement and condition report, to the buyer through the shared document vault, sharing only what the new landlord needs about the tenants. | library | small | HOME-A11-F6-P1, HOME-A9-F1-P1, HOME-A16-F4-P4 |  |
| HOME-A11-F7-P1 | **Sale fell through**: Records the end of the agreement from the lawyers, puts the listing back live and tells back-up buyers fairly. | library | small | HOME-A10-F8-P1, HOME-A10-F8-P3, HOME-A2-F4-P1, HOME-A8-F4-P1 |  |
| HOME-A11-F8-P1 | **Closing a sale**: Marks the listing sold, starts the rating requests and archives the file. | library | small | HOME-A2-F4-P1, HOME-A12-F5-P1 |  |
| HOME-A11-F9-P1 | **Sale record pack**: Collects the dates, price and costs of the sale into one file the seller can give their accountant. | library | small | HOME-A9-F1-P1, HOME-A14-F4-P1 |  |
| HOME-A11-F9-P2 | **Tax notes for the sale record**: Adds plain notes from the one tax research piece (HOME-A2-F3-P4) on what to ask an accountant, such as the bright-line test, to the sale record, without tax advice. | data | small | HOME-A2-F3-P4, HOME-A11-F9-P1 |  |
| HOME-A11-F10-P1 | **Movers in the catalogue**: Adds movers, cleaners and storage as entries on the one booking flow and suggests them near settlement. | integration | small | HOME-A12-F1-P2, HOME-A11-F1-P1 |  |
| HOME-A11-F11-P1 | **Routes for sale disputes**: Points disputes about the sale to the parties' lawyers, the Disputes Tribunal and the courts, the Real Estate Authority for licensed agents, the Law Society for lawyers and the valuers' registration board, and says the member panel decides only service disputes. Shown in the kit's help centre, and added to the kit's outside routes (KIT-A11-F7-P1) when that is built. | data | small | KIT-A21-F1-P1 |  |
| HOME-A11-F12-P1 | **Linked sale dates**: Links two sales on Spare Home and shows their key dates to the people in both who agree. | library | small | HOME-A11-F1-P3, HOME-A10-F2-P2 |  |

## HOME-A12 Professional services

Everything an agent's commission pays for is offered separately at a published price: photographers, lawyers, valuers, inspectors, stagers, printers, body corporate managers, auctioneers and licensed agents. Every service is booked from one shared service catalogue on the kit's marketplace engine, where Spare Work's pros are listed too, so a pro keeps one account, one check and one payout, and Spare Home never calls Spare Work directly.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A12-F1** Book a professional at a published price | Book a photographer, lawyer, valuer, building inspector, stager, cleaner or sign installer at a price shown up front. | core | P1, P2, P3, P4, P5 |
| **HOME-A12-F2** Know the pro is qualified | See that lawyers, valuers, agents and other pros hold the licence or membership their work needs. | core | P1, P2, P3 |
| **HOME-A12-F3** Choose a package | Pick a bundle of services for a typical sale, priced as the sum of its parts. | next | P1 |
| **HOME-A12-F4** Use a licensed agent at a fixed fee | Hire a licensed agent for a fixed, published fee for just the parts you want, such as negotiation. | next | P1, P2, P3 |
| **HOME-A12-F5** Rate the professionals | Rate each professional after the job, and read others' ratings before booking. | core | P1 |
| **HOME-A12-F6** Book services as a buyer | As a buyer, book a building inspector, lawyer or valuer for a home you want. | next | P1 |
| **HOME-A12-F7** Arrange finance and insurance | As a buyer, find a mortgage adviser or insurance broker who is registered and licensed, and see any commission they are paid. | next | P1, P2 |
| **HOME-A12-F8** Deliver the work into the sale | As a professional, deliver photos, reports, valuations or statements straight into the seller's vault or listing, which confirms the job. | core | P1 |
| **HOME-A12-F9** Let pros in while you are away | Arrange key or key-safe access for a booked professional or viewing host when you cannot be there. | next | P1 |

### Parts for HOME-A12

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A12-F1-P1 | **Home-sale services list**: Lists each service a sale may need, what it includes and when in the sale it is usually needed, as Home's entries in the shared service catalogue (HOME-A12-F1-P5). | data | medium | kit-market, HOME-A12-F1-P5 |  |
| HOME-A12-F1-P2 | **Booking on the shared marketplace engine**: Books services as kit-market orders from the shared service catalogue, finding pros through the kit's search service and availability calendars, so a professional has one account and one payout across apps and Spare Home never calls Spare Work directly. | integration | medium | kit-market, KIT-A6-F1-P2, KIT-A5-F5-P1, HOME-A12-F1-P1, HOME-A12-F1-P4, HOME-A12-F1-P5 |  |
| HOME-A12-F1-P3 | **Booking screens**: Shows nearby pros with prices, ratings and free times, and books in a few taps. | interface | medium | HOME-A12-F1-P2, svc-maps |  |
| HOME-A12-F1-P4 | **Conduct rules for lawyers and agents**: Finds whether lawyers, conveyancers and licensed agents may take bookings and payments through a platform under their conduct rules on referral fees, fee sharing, trust accounts and client care information (unverified). | research | small |  |  |
| HOME-A12-F1-P5 | **Shared service catalogue**: One catalogue of professional and local services on the kit's marketplace engine that Spare Home, Spare Work, Spare Rent and Spare Stay all book from, with each pro's checks, prices and reviews kept once. It is the catalogue Spare Work maps as WORK-A6-F1-P3 and WORK-A19-F4-P1; no kit part maps it yet, so it belongs in the kit beside the marketplace engine (KIT-A5). | library | large | kit-market, KIT-A5-F1-P1, KIT-A2-F2-P2, KIT-A9-F1-P1 |  |
| HOME-A12-F2-P1 | **Home register list**: Sets which public register applies to each kind of professional, such as practising certificates for lawyers, and uses the kit's register checks and re-checks. | integration | small | KIT-A2-F2-P1, KIT-A2-F3-P1, HOME-A12-F2-P2 |  |
| HOME-A12-F2-P2 | **What each service needs**: Lists which licence, insurance or standard each kind of service needs, per country, declared in the kit's check rules per app. | data | small | HOME-A12-F1-P1, KIT-A2-F2-P2 |  |
| HOME-A12-F2-P3 | **Proof the pro is the registered person**: Confirms that whoever runs a lawyer, conveyancer, licensed agent or valuer account is the person on the register, through contact details the register or the firm publishes, such as a code to the firm's published email, an email on the firm's own domain or a call-back to its published number, never through details the account gave. It belongs in the kit beside the register checks (KIT-A2-F2). | integration | medium | KIT-A2-F2-P1, KIT-A2-F1-P3, HOME-A12-F2-P1 |  |
| HOME-A12-F3-P1 | **Service packages**: Bundles common services into packages with no markup and lets the seller swap any part. | library | small | HOME-A12-F1-P1, HOME-A12-F1-P2 |  |
| HOME-A12-F4-P1 | **Fixed-fee licensed agents**: Lets licensed agents offer set tasks at fixed published fees, never a percentage. | integration | medium | HOME-A12-F1-P2, HOME-A12-F2-P1, HOME-A16-F1-P1, HOME-A12-F1-P4 |  |
| HOME-A12-F4-P2 | **Hand over to a licensed agent**: Gives a booked licensed agent access to the offers and buyer register for the negotiation only. | integration | small | HOME-A1-F6-P1, HOME-A12-F4-P1, HOME-A10-F7-P1 |  |
| HOME-A12-F4-P3 | **What the agent does**: Records the tasks the seller has hired the agent for and the agency agreement the agent provides. | library | small | HOME-A12-F4-P1, HOME-A9-F1-P1 |  |
| HOME-A12-F5-P1 | **Ratings for pros**: Uses the kit's ratings for services booked through Spare Home, under Home's rules on what can be rated. | library | small | KIT-A9-F1-P1, HOME-A15-F6-P1, HOME-A12-F1-P2 |  |
| HOME-A12-F6-P1 | **Buyer bookings**: Lets buyers use the one booking flow for a listed home, with access to the home arranged with the seller and, in a tenanted home, held until the tenant agrees. | interface | small | HOME-A12-F1-P2, HOME-A7-F2-P1, HOME-A7-F5-P4 |  |
| HOME-A12-F7-P1 | **Advisers and brokers**: Adds mortgage advisers and insurance brokers to the catalogue, checked on the Financial Service Providers Register and for the licence financial advice law requires. | integration | small | HOME-A12-F1-P2, KIT-A2-F2-P1, HOME-A12-F7-P2 |  |
| HOME-A12-F7-P2 | **Adviser commission policy**: Sets the policy on commissions advisers receive from lenders and insurers: shown in full to the buyer and never shared with Spare Home, in keeping with the at-cost promise. | legal | small |  |  |
| HOME-A12-F8-P1 | **Delivery into the vault**: Lets a booked professional upload the result straight to the seller's vault or listing with its licence and reliance terms, and counts delivery as the job done so payment can be released. | library | medium | HOME-A12-F1-P2, HOME-A9-F1-P1, HOME-A3-F8-P2, KIT-A5-F7-P2 |  |
| HOME-A12-F9-P1 | **Key access for bookings**: Shares a key-safe code or key pick-up details only with the booked person for the booked time, logs each use, and reminds the seller to change the code afterwards. | library | small | HOME-A12-F1-P2, HOME-A7-F2-P1 |  |

## HOME-A13 Guides and checklists

Selling privately feels risky because people do not know the steps. Spare Home walks sellers and buyers through every legal and practical step, tied to where their own sale is up to.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A13-F1** Follow a step-by-step selling guide | Follow a guide from deciding to sell to settlement day, showing what is done and what comes next. | core | P1, P2, P3, P4 |
| **HOME-A13-F2** Check you are ready to list | Tick through a readiness checklist before going live: documents, photos, disclosures, lawyer and co-owners. | core | P1 |
| **HOME-A13-F3** Follow a buyer's guide | As a buyer, follow a guide to finance, lawyers, inspections, offers and settlement. | core | P1, P2, P3 |
| **HOME-A13-F4** Understand the jargon | Tap any property term to see what it means in plain words. | core | P1 |
| **HOME-A13-F5** Get guides for your country | Get the guide that matches your country and region's rules. | next | P1 |
| **HOME-A13-F6** Ask someone who has done it | Ask questions of members who have sold or bought privately, and get support from the co-op. | next | P1, P2 |

### Parts for HOME-A13

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A13-F1-P1 | **Every step of selling privately**: Sets out every legal and practical step of a private sale, with sources. | research | medium |  | [HM05](STATUS.md) |
| HOME-A13-F1-P2 | **Selling guide**: Short plain pages for each step, kept up to date as rules change. | data | large | HOME-A13-F1-P1, HOME-A13-F1-P4, HOME-A10-F2-P6, HOME-A19-F9-P1 |  |
| HOME-A13-F1-P3 | **Guide tied to your sale**: Shows the guide beside the seller's own sale and ticks off steps as they happen. | interface | medium | HOME-A13-F1-P2, HOME-A2-F4-P1, HOME-A11-F1-P3 |  |
| HOME-A13-F1-P4 | **Tax steps for sellers and buyers**: Explains residential land withholding tax for offshore sellers, whose IR1101 form goes to the conveyancer, and the land transfer tax statement every buyer and seller gives their lawyer, with sources. | data | small | HOME-A13-F1-P1, HOME-A2-F3-P4 |  |
| HOME-A13-F2-P1 | **Ready-to-list checklist**: Builds a checklist from the property type and country, and links each item to where it is done. | library | small | HOME-A2-F4-P2, HOME-A13-F1-P2, HOME-A2-F9-P1 |  |
| HOME-A13-F3-P1 | **Buying guide**: Short plain pages for each buying step, including what to ask and what to check. | data | medium | HOME-A13-F1-P1, HOME-A13-F1-P4 |  |
| HOME-A13-F3-P2 | **Insurance before going unconditional**: Reminds buyers to confirm they can insure the home before going unconditional, with a stronger warning in hazard areas. | data | small | HOME-A13-F3-P1, HOME-A5-F3-P2 |  |
| HOME-A13-F3-P3 | **First-home steps**: Explains KiwiSaver first-home withdrawals, first-home schemes, and that banks use valuers from their own panel. | data | small | HOME-A13-F3-P1 |  |
| HOME-A13-F4-P1 | **Plain words glossary**: Explains terms such as title, chattels, unconditional and settlement, linked wherever they appear. | data | small |  |  |
| HOME-A13-F5-P1 | **Guides per country**: Packages the guides with each country's rules pack so a new country brings its own steps. | library | medium | HOME-A13-F1-P2, HOME-A16-F6-P1 |  |
| HOME-A13-F6-P1 | **Home topics in the member forum**: Adds a moderated home-sale space to the kit's member forum, where past sellers and buyers help new ones. | library | medium | KIT-A21-F6-P1 |  |
| HOME-A13-F6-P2 | **Help that is not advice**: Rules and wording so member help stays general and never becomes legal, financial or agency advice. | legal | small | HOME-A13-F6-P1, HOME-A16-F1-P1 |  |

## HOME-A14 Payments, fees and the open cost ledger

Sellers pay a small at-cost fee and the published price of any services they book. Payments go through the kit's regulated payment provider. The house price and deposit never pass through Spare Home.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A14-F1** Pay the at-cost listing fee | Pay a listing fee worked out from what Spare Home costs to run, shown before you list. | core | P1, P2, P3 |
| **HOME-A14-F2** Pay for services | Pay for booked services, with the money held until the job is done, and know who you are buying from. | core | P1, P2, P3, P4 |
| **HOME-A14-F3** Pay when the sale settles | Choose to pay the fee and services from the sale money at settlement, through your lawyer. | next | P1, P2, P3 |
| **HOME-A14-F4** Get proper invoices and receipts | Get a GST invoice or receipt for every payment. | core | P1 |
| **HOME-A14-F5** Get a refund or raise a dispute | Ask for a refund when a listing or service is cancelled, or raise a dispute about a service. | core | P1, P2 |
| **HOME-A14-F6** See where the fee goes | See what Spare Home cost to run each month and what the fee paid for. | core | P1 |

### Parts for HOME-A14

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A14-F1-P1 | **Home listing fee rule**: Sets Home's fee unit, one live listing, in the kit's fee calculator: last period's running cost divided by listings. | library | small | KIT-A3-F4-P1, svc-ledger, kit-market |  |
| HOME-A14-F1-P2 | **Paying the listing fee**: Takes the fee through the kit's payments service when the listing goes live, unless the seller has chosen to pay at settlement (HOME-A14-F3). | integration | small | HOME-A14-F1-P1, svc-payments | [K02](../kit/STATUS.md) |
| HOME-A14-F1-P3 | **Fee rules for relisting, pausing and expiry**: Sets the published rules members vote on for how the fee applies when a sale falls through and the home is relisted, when a listing is paused, expires or is withdrawn, and when the fee is to be paid at settlement, so the going-live gate knows when the fee counts as settled. | library | small | HOME-A14-F1-P1, HOME-A2-F4-P1 |  |
| HOME-A14-F2-P1 | **Service payments**: Takes payment for services through the kit's held-payment loop and pays the professional when the job is confirmed. | integration | medium | svc-payments, kit-market, HOME-A12-F1-P2, HOME-A12-F8-P1, HOME-A12-F1-P4 | [K02](../kit/STATUS.md) |
| HOME-A14-F2-P2 | **Seller of record for booked services**: Sets, on the kit's seller of record per app, who sells each service booked through Home, the professional or the co-op, which decides who charges GST, whose name is on the receipt and who answers a card dispute. | library | small | KIT-A3-F1-P5, HOME-A12-F1-P5, HOME-A14-F2-P1 | [K02](../kit/STATUS.md) |
| HOME-A14-F2-P3 | **Services in the yearly IRD report**: Includes services booked through Home, such as photography, inspections and conveyancing, in the kit's yearly report of sellers' income to IRD under the digital platform reporting rules, as the kit's reporting rules settle which services and sellers are covered. | integration | small | KIT-A4-F3-P1, KIT-A4-F3-P3, HOME-A14-F2-P1 |  |
| HOME-A14-F2-P4 | **Which ledger carries a catalogue booking**: Records each booking from the shared service catalogue against the app it was booked from, through the kit's usage metering, so its fee and cost show once, in Spare Home's ledger, and are not counted again by Spare Work. The rule is confirmed in the kit's ledger design. | library | small | KIT-A13-F2-P2, HOME-A12-F1-P5, svc-ledger |  |
| HOME-A14-F3-P1 | **Pay at settlement**: Sends one final statement to the seller's lawyer to pay from the sale money, so nothing is paid up front. | integration | medium | HOME-A14-F2-P1, HOME-A11-F3-P2, HOME-A14-F3-P3, HOME-A12-F1-P4 |  |
| HOME-A14-F3-P2 | **Paying from sale money**: Checks how lawyers can pay third parties from sale money and what happens if the sale falls through. | research | small |  |  |
| HOME-A14-F3-P3 | **Is paying at settlement credit**: Checks whether letting sellers pay Spare Home's fees at settlement counts as credit under consumer credit law, and what happens if the sale never settles. | research | small |  |  |
| HOME-A14-F4-P1 | **Home invoice details**: Adds Home's listing and service details to the kit's tax invoices and receipts, kept in the account. | library | small | KIT-A4-F2-P1, KIT-A3-F7-P1, HOME-A14-F2-P1 |  |
| HOME-A14-F5-P1 | **Home refund rules**: Sets Home's published refund rules for cancelled listings and services, carried out by the kit's refunds. | library | small | KIT-A3-F5-P1, HOME-A14-F1-P1 |  |
| HOME-A14-F5-P2 | **Service disputes**: Sends disputes about a booked service to the kit's disputes service, with the payment held until a member panel decides. | integration | small | KIT-A11-F1-P1, HOME-A14-F2-P1 |  |
| HOME-A14-F6-P1 | **Cost ledger page**: Shows Spare Home's monthly costs and fee from the open cost ledger. | interface | small | svc-ledger, KIT-A13-F1-P3 | [K06](../kit/STATUS.md) |

## HOME-A15 Trust, safety and moderation

Homes are the biggest thing most people buy, so fake listings and money scams do real harm. Spare Home checks owners, watches for scams, moderates listings to published rules and logs every removal in public.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A15-F1** Stop fake listings | Trust that every live listing was placed by a checked owner of a real home. | core | P1, P2, P3 |
| **HOME-A15-F2** Avoid money scams | Get clear warnings about deposit and bank detail scams at every step where money is mentioned. | core | P1 |
| **HOME-A15-F3** Report a listing or a person | Report a listing, message or person and see what happened to your report. | core | P1 |
| **HOME-A15-F4** Moderate listings fairly | Have listings checked against published rules, with every removal and its reason logged in public. | core | P1, P2, P3 |
| **HOME-A15-F5** Keep people's privacy | Hide your exact address until a viewing is booked, keep it private when the home is empty, and keep visitors' and neighbours' details private. | core | P1, P2 |
| **HOME-A15-F6** Rate the right things | Rate professionals after a job, while buyers and sellers are never publicly scored. | core | P1 |
| **HOME-A15-F7** Report reused photos or plans | As a photographer, architect or other rights owner, report a listing that uses your photos or plans without permission. | next | P1 |

### Parts for HOME-A15

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A15-F1-P1 | **Home scam patterns**: Researches home-sale scams, such as seller impersonation, fake listings and deposit redirection, and feeds them into the kit's research on scam methods (KIT-A10-F12-P1). | research | medium |  |  |
| HOME-A15-F1-P2 | **Fake listing checks**: Flags photos reused without a recorded licence, prices far below the market and details that do not match the record, for review, as Home rules on the kit's listing checks. They feed the kit's fraud signals (KIT-A10-F12-P2) once the kit builds them. | library | medium | HOME-A15-F1-P1, HOME-A3-F1-P2, HOME-A4-F1-P2, HOME-A1-F3-P2, HOME-A3-F8-P2, KIT-A5-F1-P3 |  |
| HOME-A15-F1-P3 | **Same home listed twice**: Catches the same property listed twice, whether by a co-owner, an agent or a scammer, and holds the newer listing for review. | library | small | HOME-A2-F1-P2, HOME-A15-F1-P2 |  |
| HOME-A15-F2-P1 | **Warnings wherever money comes up**: Shows a plain warning about deposit and bank detail scams at every step where money comes up, such as offers, the lawyers' workspace and settlement, and pinned in every sale chat. It adds the kit's scam warnings in chat (KIT-A8-F9-P1) once the kit builds them. | library | small | HOME-A11-F2-P1, HOME-A15-F1-P1, HOME-A8-F1-P1, KIT-A8-F1-P3 |  |
| HOME-A15-F3-P1 | **Home report reasons**: Adds home-sale report reasons to the kit's report forms and tells the reporter the outcome. | interface | small | KIT-A10-F3-P1, KIT-A10-F3-P2 |  |
| HOME-A15-F4-P1 | **Listing rules**: Published rules on what listings may say and show, such as no discrimination, no misleading claims and no one else's private details. | data | medium | HOME-A16-F5-P1 |  |
| HOME-A15-F4-P2 | **Listing moderation settings**: Sends reports and flagged listings to the kit's moderation queue with Home's listing rules attached to each case. | interface | medium | KIT-A10-F6-P2, HOME-A15-F4-P1, HOME-A15-F3-P1 |  |
| HOME-A15-F4-P3 | **Listing appeals**: Lets a seller appeal a removal to a member panel through the kit's appeals. | library | small | KIT-A11-F5-P1, HOME-A15-F4-P2 |  |
| HOME-A15-F5-P1 | **Address hiding**: Uses the kit's approximate location to show only the street or suburb until a checked buyer books a viewing, if the seller chooses. | library | small | KIT-A7-F10-P1, HOME-A5-F2-P1, HOME-A7-F2-P1 |  |
| HOME-A15-F5-P2 | **Hide that a home is empty**: Never shows that a home is empty, that the owner is overseas or that it is a deceased estate unless the seller chooses, offers to show open home times only to registered buyers, and warns before going live if photos show an alarm panel, key safe or keys. | library | small | HOME-A15-F5-P1, HOME-A3-F1-P3, HOME-A7-F1-P1 |  |
| HOME-A15-F6-P1 | **What can be rated**: Sets that only professionals' services are rated, and that buyers and sellers are not given public scores. | data | small | KIT-A9-F1-P1 |  |
| HOME-A15-F7-P1 | **Rights owner notices**: Adds Home's per-file licence records to the kit's copyright notices, so a rights owner can report a copied photo or plan, such as an earlier agent's campaign photos or an architect's plans, and the seller can answer with the licence on record. | library | small | KIT-A10-F21-P2, HOME-A3-F8-P2 |  |

## HOME-A16 Law, the agency line and countries

Spare Home must stay a set of tools and never become an unlicensed agent. This area turns the law into product rules: agency law, anti-money-laundering, overseas buyers, privacy, consumer law and each country's own rules.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A16-F1** Stay on the right side of agency law | Use every tool knowing Spare Home never negotiates, advises on price or acts for anyone in the sale. | core | P1, P2, P3, P4 |
| **HOME-A16-F2** Meet anti-money-laundering law | Have identity checks done by the people the law requires, such as lawyers, with Spare Home doing only its own share. | core | P1, P2 |
| **HOME-A16-F3** Know the rules for overseas buyers | As a buyer, learn early whether you may need consent to buy and which statement your lawyer needs. | core | P1, P2 |
| **HOME-A16-F4** Have your privacy protected | Know what Spare Home keeps about you, why, for how long, and ask to see or correct it. | core | P1, P2, P3, P4 |
| **HOME-A16-F5** Trust that listings follow consumer law | Rely on listings that avoid misleading claims and discrimination. | core | P1, P2 |
| **HOME-A16-F6** Keep each country's rules in a pack | Sell under rules kept in a country pack, starting with New Zealand, so a new country is added without rewriting the app. | core | P1, P3 |
| **HOME-A16-F7** Agree to clear terms | Read short, plain terms and confirm what you declare as a seller. | core | P1, P2, P3 |
| **HOME-A16-F8** Prepare the first other countries | Know that the rules-pack format has been tested against Australia's states and the UK before any country after NZ is added. | next | P1 |
| **HOME-A16-F9** See when a seller is in trade | As a buyer, see when a seller sells homes in trade, such as a developer or investor. | next | P1 |

### Parts for HOME-A16

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A16-F1-P1 | **Where tools end and agency begins**: What the app may and may not do under real estate law, with sources and a lawyer's view, for every tool that does more than publish an advert, such as offers, the buyer register, viewings and AI replies. | research | medium |  | [HM01](STATUS.md) |
| HOME-A16-F1-P2 | **Agency guardrails**: Product rules that stop the app, its staff and its AI tools from negotiating, recommending offers or advising on price for a seller. | library | medium | HOME-A16-F1-P1, kit-ops |  |
| HOME-A16-F1-P3 | **Legal sign-off**: A lawyer's review of the guardrails and the wording on every screen that touches offers or price. | legal | medium | HOME-A16-F1-P2 |  |
| HOME-A16-F1-P4 | **A fallback for each tool**: Keeps a ready fallback, behind the kit's feature switches, for each tool HM01 might find to be agency work: offers kept to a register only, the buyer register and multi-offer run only through a booked fixed-fee licensed agent (HOME-A12-F4), or the tool switched off. | library | medium | HOME-A16-F1-P1, KIT-A22-F2-P2 |  |
| HOME-A16-F2-P1 | **Is Spare Home a reporting entity**: Finds whether Spare Home has anti-money-laundering duties of its own, on top of the kit's anti-money-laundering programme, and what lawyers and agents must check. | research | medium | KIT-A4-F5-P1 |  |
| HOME-A16-F2-P2 | **Sharing checks with lawyers**: Lets a person share their identity check result with their lawyer, with consent, to save repeating it. | integration | small | HOME-A1-F2-P1, HOME-A11-F3-P2, HOME-A16-F2-P1 |  |
| HOME-A16-F3-P1 | **Overseas buyer rules**: Sets out who may buy residential land without consent and the statement every buyer gives their lawyer. | research | small |  |  |
| HOME-A16-F3-P2 | **Overseas buyer notice**: Shows buyers plain information about these rules on listings and offers, without judging their case. | data | small | HOME-A16-F3-P1 |  |
| HOME-A16-F4-P1 | **Privacy impact assessment**: A privacy impact assessment for Spare Home under NZ privacy law, done before launch, including the notice duty for details collected from someone else (IPP 3A, since 1 May 2026), redone for each new country, and added to the kit's register of assessments (KIT-A18-F10-P1) once that exists. | legal | medium |  |  |
| HOME-A16-F4-P2 | **How long data is kept**: Sets how long each kind of Home data is kept after a sale, carried out by the kit's retention schedules. | data | small | HOME-A16-F4-P1, KIT-A18-F6-P1 |  |
| HOME-A16-F4-P3 | **Privacy request clock**: Adds Home's data to the kit's privacy page and tracks the legal reply time in working days on the kit's working days and notice periods. | library | small | KIT-A18-F3-P1, KIT-A19-F9-P2 |  |
| HOME-A16-F4-P4 | **Notices for details from others**: Tells people whose details Spare Home got from someone else, such as tenants named by a seller, owners and co-owners on the title, partners on a shared shortlist and lawyers named by a party, as IPP 3A requires. | library | small | KIT-A18-F5-P1, HOME-A16-F4-P1, svc-messaging |  |
| HOME-A16-F5-P1 | **Consumer and advertising law**: Adds the home-sale questions to the kit's consumer law for platforms: who is in trade in a private sale, the platform's own duties, and human rights rules for listings. | research | medium | KIT-A18-F8-P1 |  |
| HOME-A16-F5-P2 | **Listing wording checks**: Flags wording in listings that may mislead or discriminate before it goes live. | library | small | HOME-A16-F5-P1, HOME-A2-F2-P2 |  |
| HOME-A16-F6-P1 | **Rules-pack format for home sales**: Home's layer on the kit's NZ country pack, in a format able to hold required listing facts such as the UK's material information Parts A, B and C, disclosures, energy certificates, methods of sale, price display and underquoting rules, cooling-off periods, redress scheme membership, notary-led conveyancing, deeds registries as well as title registers, working-day rules, time zones for deadlines and auctions, and the agency line. It moves onto the kit's country packs (KIT-A19-F6-P1) when they are built. | library | large | KIT-A19-F9-P1, kit-plugin |  |
| HOME-A16-F6-P3 | **New Zealand rules pack**: The first pack, there from day one, holding NZ disclosures, price display and GST labels, tenancy rules, methods of sale including 'enquiries over', and the agreement's working-day rules, so no NZ rule is hard-coded. | data | large | HOME-A16-F6-P1, HOME-A9-F3-P1, HOME-A2-F3-P1, HOME-A2-F3-P2, HOME-A2-F3-P3, HOME-A7-F5-P1, HOME-A11-F1-P2, KIT-A19-F9-P1 |  |
| HOME-A16-F7-P1 | **Terms of use**: Plain terms for sellers, buyers and professionals, written by a lawyer and kept in the kit's terms and policy versions. | legal | medium | HOME-A16-F1-P1, KIT-A18-F4-P1, KIT-A18-F4-P2 |  |
| HOME-A16-F7-P2 | **Seller declaration**: A declaration the seller makes that they may sell and that their facts and disclosures are true. | legal | small | HOME-A16-F7-P1, HOME-A9-F3-P1, KIT-A18-F4-P1 |  |
| HOME-A16-F7-P3 | **Professional indemnity cover**: Adds professional indemnity cover for Home's guides, disclosure checklist, price guide and due date engine to the kit's co-op insurance, which lists only directors and officers, cyber and public liability cover, with the limits the terms state. | legal | small | KIT-A14-F17-P1, HOME-A16-F7-P1 |  |
| HOME-A16-F8-P1 | **First other countries**: Researches the rules for Australia's states and the UK as the first packs after NZ, to test the rules-pack format before it is fixed. | research | large | HOME-A16-F6-P1 |  |
| HOME-A16-F9-P1 | **Trader status for sellers**: Asks whether a seller sells homes in trade and shows buyers the trader details the kit keeps, if the Commerce Commission's guidance on traders on platforms covers land listings (unverified). | library | small | KIT-A2-F7-P2, HOME-A16-F5-P1 |  |

## HOME-A17 Access for everyone: accessibility, languages, phones and offline

Anyone should be able to sell or buy, whatever their abilities, language, device or signal. Spare Home works with screen readers, in many languages, as an installed phone app, and offline where it matters most.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A17-F1** Use it with any ability | Use every screen with a screen reader, keyboard, zoom or switch device. | core | P1, P2, P3, P4 |
| **HOME-A17-F2** Use it in your language | Use Spare Home in English and te reo Māori, and find places typed with or without macrons. | core | P1, P2, P4 |
| **HOME-A17-F3** Install it on your phone | Install Spare Home on a phone and get notifications, without needing a big download. | core | P1 |
| **HOME-A17-F4** Keep working offline | Draft a listing, check in open home visitors and read saved homes with no signal. | core | P1 |
| **HOME-A17-F5** Choose how you are told | Choose which updates reach you, and by push, email or text. | core | P1 |
| **HOME-A17-F6** Use it on slow connections | Browse and list on a slow or costly connection with a low data mode. | next | P1 |
| **HOME-A17-F7** See your own units and formats | See areas, money, dates and addresses the way your country writes them. | core | P1 |
| **HOME-A17-F8** Work offline on site | Show listing facts and chosen documents at the door, run the pre-settlement inspection and draw a floor plan with no signal. | later | P1 |
| **HOME-A17-F9** Read guides the easy way | Read easy-read versions of the selling and buying guides, and watch the video guides in NZ Sign Language. | later | P1, P2 |
| **HOME-A17-F10** Read listings in your language | Read listings and messages translated into your language, and use Spare Home in more community languages. | next | P1, P2, P3 |
| **HOME-A17-F11** Read scanned documents with a screen reader | Have scanned council reports, titles and older reports read by a screen reader. | next | P1 |

### Parts for HOME-A17

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A17-F1-P1 | **Home screens to the access standard**: Builds every Home screen to WCAG 2.2 AA from the kit's accessible components. | library | medium | KIT-A19-F2-P1 |  |
| HOME-A17-F1-P2 | **Room-aware photo descriptions**: Uses the kit's image descriptions, adding room names from the plan, for the seller to check so blind buyers know what is shown. | library | small | KIT-A19-F2-P3, HOME-A3-F1-P1 |  |
| HOME-A17-F1-P3 | **Accessibility testing**: Tests with disabled users and tools before each release. | research | small | HOME-A17-F1-P1, KIT-A19-F2-P2 |  |
| HOME-A17-F1-P4 | **Accessible documents**: Makes brochures, disclosure statements, plans and watermarked copies as tagged, accessible PDFs. | library | medium | engine-pdf, KIT-A19-F2-P1 |  |
| HOME-A17-F2-P1 | **Home words in translation files**: Keeps every Home word on screen in the kit's translation framework. | library | medium | KIT-A19-F1-P1 |  |
| HOME-A17-F2-P2 | **Te reo Māori**: A full te reo Māori version, written with fluent speakers. | data | medium | HOME-A17-F2-P1 |  |
| HOME-A17-F2-P4 | **Place names with or without macrons**: Uses the kit's macron-aware matching for suburbs, streets and school zones in Home search and addresses. | library | small | KIT-A19-F1-P4, HOME-A5-F1-P2, svc-maps |  |
| HOME-A17-F3-P1 | **Home in the phone app**: Puts Spare Home in the installable phone app the co-op chooses for all marketplaces. | interface | large | KIT-A20-F2-P2, kit-sync | [K04](../kit/STATUS.md) |
| HOME-A17-F4-P1 | **Offline work**: Keeps drafts, the open home register and saved homes on the device and syncs when back online. | library | medium | kit-sync, kit-store, KIT-A15-F4-P2 |  |
| HOME-A17-F5-P1 | **Home notification events**: Adds Home's events, such as new offers and due dates, to the kit's notification settings, with quiet hours. | library | small | KIT-A8-F5-P1 |  |
| HOME-A17-F6-P1 | **Low data mode**: Serves smaller images and holds back video unless asked. | library | small | HOME-A3-F1-P2, KIT-A19-F5-P1 |  |
| HOME-A17-F7-P1 | **Home units and formats**: Uses the kit's local formats for areas, money, dates and addresses and adds land units such as hectares. | library | small | KIT-A19-F1-P2, HOME-A17-F2-P1 |  |
| HOME-A17-F8-P1 | **More offline on site**: Adds the chosen documents, the pre-settlement inspection checklist and the plan drawer to Home's one offline store (HOME-A17-F4-P1), which already holds drafts, the open home register and saved homes. | library | medium | HOME-A17-F4-P1, HOME-A11-F4-P1, HOME-A3-F3-P1 |  |
| HOME-A17-F9-P1 | **Easy-read guides**: Easy-read versions of the selling and buying guides. | data | medium | HOME-A13-F1-P2, HOME-A13-F3-P1, KIT-A19-F8-P1 |  |
| HOME-A17-F9-P2 | **NZ Sign Language video guides**: NZ Sign Language versions of the video guides. | data | medium | HOME-A13-F1-P2, KIT-A19-F8-P1 |  |
| HOME-A17-F10-P1 | **Listing translation**: Translates listings on request on the kit's machine translation at cost, and messages through the kit's message translation, labelled as machine translated. | integration | medium | KIT-A19-F10-P2, KIT-A8-F7-P1, HOME-A17-F2-P1, HOME-A5-F2-P1 |  |
| HOME-A17-F10-P2 | **The original wording counts**: Labels translated listings and disclosures with a note that the seller's original wording is the one that counts. | legal | small | HOME-A17-F10-P1 |  |
| HOME-A17-F10-P3 | **First community languages**: Early translations of Home's screens into Chinese, Hindi, Samoan and Tongan, written with fluent speakers. | data | medium | HOME-A17-F2-P1 |  |
| HOME-A17-F11-P1 | **Text layer for scanned documents**: Adds a text layer to scanned documents in the vault, such as land information reports, titles and older consents, using an open text recognition engine, marked as machine read so the original scan stays the record. | library | medium | HOME-A9-F1-P1, engine-pdf |  |

## HOME-A18 Your data, AI and plugins

Members own their data and can take it with them, even in the middle of a sale. Everything a person can do, an AI agent or script can do through the same commands, inside the same agency guardrails, and a person confirms every high-risk command.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A18-F1** Export everything | Download your listing, photos, plans, documents, messages and payments in open formats. | core | P1, P2, P3 |
| **HOME-A18-F2** Delete your data | Delete your account and data, except what the law says must be kept. | core | P1, P2 |
| **HOME-A18-F3** Script any action | Do anything the app can do through its command interface. | core | P1, P2 |
| **HOME-A18-F4** Use an AI assistant | Let an AI assistant draft a listing, sort photos, summarise documents or track dates, within the same limits as a person. | core | P1, P2, P3, P4, P5, P6, P7 |
| **HOME-A18-F5** Add plugins | Add plugins that bring new tools, such as other plan formats or local data, with only the permissions you grant. | later | P1 |
| **HOME-A18-F6** Follow a sale from your own system | As a lawyer or buyer, have your own tools follow offers and due dates without checking the app. | later | P1 |

### Parts for HOME-A18

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A18-F1-P1 | **Home export contents**: Adds Home data to the kit's data export: JSON for data, PDF for documents, and the original photos and plans. | library | medium | KIT-A18-F1-P1, HOME-A9-F1-P1 |  |
| HOME-A18-F1-P2 | **Export screen**: Lets a person start a Home export from the kit's privacy page and download it when ready. | interface | small | HOME-A18-F1-P1, KIT-A18-F3-P1 |  |
| HOME-A18-F1-P3 | **Take a live sale elsewhere**: Hands a live listing to a licensed agent or another site in an open listing format, passing on buyer register entries only for buyers who consent. | library | medium | HOME-A18-F1-P1, HOME-A8-F3-P3, HOME-A3-F8-P2 |  |
| HOME-A18-F2-P1 | **Home data deletion**: Adds Home's data and keeping rules to the kit's account deletion, erases personal details in the offer register, the record of who was told what and the public change history by the kit's method while those logs stay intact, and explains anything kept and for how long. | library | small | KIT-A18-F2-P1, HOME-A16-F4-P2, KIT-A15-F1-P5, KIT-A15-F1-P6 |  |
| HOME-A18-F2-P2 | **What stays public after deletion**: Decides what happens to a home's public price history, days on market and published sale price when the seller deletes their account. | legal | small | HOME-A16-F4-P1, HOME-A2-F5-P1 |  |
| HOME-A18-F3-P1 | **Home command set**: Defines every Home action as a kit operation with a published schema, used by screens, AI and plugins alike. | library | large | kit-ops, KIT-A15-F1-P3 |  |
| HOME-A18-F3-P2 | **Open listing data**: Publishes live public listings as open, read-only data, leaving out hidden addresses, private and coming-soon listings, the owner's identity and media the licence forbids, with rate limits and terms against bulk scraping. | integration | medium | HOME-A18-F3-P1, HOME-A5-F1-P2, HOME-A15-F5-P1, HOME-A3-F8-P2, KIT-A16-F1-P3 |  |
| HOME-A18-F4-P1 | **AI access**: Exposes the Home commands to AI agents through the kit's MCP server, with the person's own permissions, and marks all text written by others, such as listings, offer documents, reports and buyers' messages, as data under the kit's hidden instruction guards. | integration | medium | kit-mcp, HOME-A18-F3-P1, KIT-A17-F1-P1, KIT-A17-F5-P1 |  |
| HOME-A18-F4-P2 | **Guardrail tests for AI tools**: Tests every Home AI tool against the agency guardrails (HOME-A16-F1-P2) before each release, with prompts that try to make it negotiate, recommend an offer or advise on price, including prompts hidden in listings and documents. | library | small | HOME-A16-F1-P2, HOME-A18-F4-P1, KIT-A17-F5-P1 |  |
| HOME-A18-F4-P3 | **Listing drafts**: Drafts a description and photo order from the facts and photos for the seller to edit. | library | medium | HOME-A18-F4-P1, HOME-A2-F2-P2, HOME-A2-F2-P3 |  |
| HOME-A18-F4-P4 | **Person confirms agent actions**: Holds any high-risk command an AI agent or script calls until the person confirms it in the app with a fresh sign-in. | library | small | KIT-A17-F2-P1, HOME-A1-F8-P2, HOME-A18-F4-P1 |  |
| HOME-A18-F4-P5 | **Home agent limits**: Sets Home's rate limits for each agent on the kit's agent limits, and uses the kit's agent labels on every message, offer and booking an agent makes. | library | small | KIT-A17-F4-P1, KIT-A17-F9-P1, KIT-A16-F1-P3, HOME-A18-F4-P1 |  |
| HOME-A18-F4-P6 | **Document summaries**: Summarises land information reports, building reports and titles in plain words, giving the page each point comes from and flagging anything unconfirmed, and never says whether a problem matters or what to do about it, which stays with the buyer's lawyer and inspector. | library | medium | HOME-A18-F4-P1, HOME-A9-F1-P1, KIT-A17-F5-P1, HOME-A18-F4-P7 |  |
| HOME-A18-F4-P7 | **Summary wording rules**: Wording rules, checked by a lawyer, so a summary of a hazard notice, a weathertightness report or a title never reads as legal or building advice, with a set note on every summary. | legal | small | HOME-A16-F1-P2 |  |
| HOME-A18-F5-P1 | **Plugin points**: Defines where plugins may add to Home, such as importers, data layers and checks, run in the kit's sandbox. | library | medium | kit-plugin, HOME-A18-F3-P1, KIT-A16-F5-P2 |  |
| HOME-A18-F6-P1 | **Sale events**: Sends offer, condition and due-date events through the kit's webhooks to tools the person has allowed. | integration | small | KIT-A16-F2-P1, HOME-A18-F3-P1, HOME-A11-F1-P1 |  |

## HOME-A19 Running Spare Home: support, admin and member votes

The co-op needs to support members, run the service safely and let members decide its fees and rules. Every admin action is an operation in the log, so nothing happens out of sight.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **HOME-A19-F1** Get help from a person | Ask the co-op's support team for help by message, and see your request's progress. | core | P1, P2, P3 |
| **HOME-A19-F2** Run admin with a full record | As a trusted member, fix problems, review cases and change settings, with every action recorded. | core | P1, P2 |
| **HOME-A19-F3** Vote on fees and rules | As a member, propose and vote on Spare Home's fee, listing rules and multi-offer process. | core | P1, P2, P3, P4 |
| **HOME-A19-F4** Watch health and costs | See uptime, problems and running costs, published with the cost ledger. | core | P1 |
| **HOME-A19-F5** Open region by region | Turn Spare Home on in a new region once its council data, rules and professionals are ready. | next | P1, P2 |
| **HOME-A19-F6** Bring partners on board | As a law firm, valuer or other pro, join and set up your services for home sales. | next | P1 |
| **HOME-A19-F7** Handle legal requests | Have takedown demands, court orders and police requests handled to a published process. | core | P1 |
| **HOME-A19-F8** Keep it secure | Trust that Spare Home has been checked by outside security reviewers before it goes live. | core | P1 |
| **HOME-A19-F9** Keep rules and data current | Trust that rules, guides and data are kept up to date, each with a named owner and a version. | core | P1, P2 |
| **HOME-A19-F10** See how sales on Spare Home are going | See published figures on days on market, sales that fell through and sale prices against rating values for Spare Home sales. | next | P1 |

### Parts for HOME-A19

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| HOME-A19-F1-P1 | **Home support context**: Attaches the sale's context to help requests in the kit's support cases when the member allows. | interface | medium | KIT-A21-F2-P1, HOME-A1-F6-P1 |  |
| HOME-A19-F1-P2 | **Help pages**: Short answers to common questions, searchable and linked from each screen. | data | small | HOME-A13-F1-P2, KIT-A21-F1-P1 |  |
| HOME-A19-F1-P3 | **Urgent help on settlement day**: Opens a path outside office hours for payment scams in progress and settlement-day problems, telling the person first to call their bank and lawyer straight away. | library | small | KIT-A21-F3-P1, HOME-A11-F2-P3 |  |
| HOME-A19-F2-P1 | **Home admin tools**: Adds ownership review and listing moderation to the kit's admin console, each action an operation in the log. | interface | medium | KIT-A22-F6-P1, HOME-A1-F3-P3, HOME-A15-F4-P2 |  |
| HOME-A19-F2-P2 | **Home admin roles**: Defines Home's admin roles in the kit's permission checks, giving each only the actions its job needs, reviewed on a schedule. | library | small | KIT-A15-F1-P4, KIT-A22-F5-P1, HOME-A19-F2-P1 |  |
| HOME-A19-F3-P1 | **Home settings members vote on**: Links the kit's results-to-settings step to Home's fee and rules, so a passed vote changes them. | library | small | KIT-A12-F4-P1, HOME-A14-F1-P1, HOME-A15-F4-P1, HOME-A19-F3-P3 |  |
| HOME-A19-F3-P2 | **Who votes on what**: Decides which members vote on Home matters, such as past sellers and professionals who work on it. | research | small |  |  |
| HOME-A19-F3-P3 | **Settings the law fixes**: Lists which Home settings members may vote on and which the law fixes, such as the agency guardrails, keeping house money out of the app, disclosure duties and tenant notices. | data | small | HOME-A16-F1-P2 |  |
| HOME-A19-F3-P4 | **Membership for one-time sellers and buyers**: Decides whether sellers and buyers must join the co-op to list or offer, and how many one-time sellers affect the co-operative test that transacting members hold at least 60% of voting rights. | research | small | KIT-A14-F2-P3 |  |
| HOME-A19-F4-P1 | **Home health and cost figures**: Feeds Home's uptime, errors and costs into the kit's monitoring and the ledger. | service | small | KIT-A22-F1-P1, KIT-A13-F2-P2 |  |
| HOME-A19-F5-P1 | **Region settings**: Holds each region's data sources, rules pack and launch state. | data | small | HOME-A16-F6-P1, HOME-A5-F3-P2 |  |
| HOME-A19-F5-P2 | **Enough professionals**: Shows whether a region has enough photographers, lawyers and other pros signed up before it opens. | library | small | HOME-A19-F5-P1, HOME-A12-F1-P2 |  |
| HOME-A19-F6-P1 | **Partner set-up**: Helps law firms and other pros set up their accounts, prices and service areas for home sales. | interface | medium | HOME-A12-F1-P2, HOME-A12-F2-P1 |  |
| HOME-A19-F7-P1 | **Home legal requests**: Handles takedown demands, court orders and police requests about listings through the kit's legal notice handling and law enforcement process. | library | small | KIT-A10-F9-P2, KIT-A10-F9-P3 |  |
| HOME-A19-F8-P1 | **Home security review**: Covers Spare Home in the kit's outside security reviews before launch and after big changes, with home-sale fraud in scope. | research | medium | KIT-A22-F5-P3 |  |
| HOME-A19-F9-P1 | **Rule and guide versions**: Names who updates each rules pack, disclosure question set and guide when the law or a source changes, stamps each with a version, and fixes which version applies to a sale already under way. | library | small | HOME-A16-F6-P1 |  |
| HOME-A19-F9-P2 | **Data import alerts**: Alerts staff when a data import fails or a source goes stale, and shows each fact's date on listings. | service | small | HOME-A2-F1-P4, HOME-A5-F3-P2, KIT-A22-F1-P1 |  |
| HOME-A19-F10-P1 | **Published sale results**: Publishes days on market, the share of sales that fell through and sale price against rating value for Spare Home sales, in totals only, through the kit's public reporting pipeline, using only prices the parties agreed to publish. | library | small | KIT-A13-F9-P1, HOME-A2-F5-P1, HOME-A4-F3-P1, HOME-A4-F4-P2 |  |

## Parts that belong in the kit

A program never depends on another program, so anything two programs both need moves down into the kit or an engine. Other programs also need these Home parts, and the kit does not map them yet. Each should be built once there, with Spare Home adding only its own rules.

| ID | Part | Also needed by |
| --- | --- | --- |
| HOME-A1-F2-P3 | **Identity checked in person** | every app |
| HOME-A1-F2-P4 | **Identity for people overseas** | every app with members overseas |
| HOME-A1-F3-P2 | **Owner name match** | Spare Rent |
| HOME-A1-F3-P3 | **Ownership review queue** | Spare Rent |
| HOME-A1-F3-P5 | **Proof of control** | Spare Rent |
| HOME-A1-F3-P8 | **Notice to the owner's address of record** | Spare Rent |
| HOME-A1-F4-P1 | **Kinds of authority to sell** | Spare Rent |
| HOME-A1-F4-P2 | **Authority evidence upload** | Spare Rent |
| HOME-A1-F5-P1 | **Co-owner invitations** | Spare Rent |
| HOME-A1-F5-P2 | **Decisions that need every owner** | Spare Rent |
| HOME-A1-F8-P3 | **Recent sign-in check** | every app with high-risk actions |
| HOME-A1-F8-P4 | **Second step set-up for sellers and buyers** | every app with high-risk actions |
| HOME-A2-F1-P2 | **Shared property record** | Spare Rent, Spare Stay, Spare Work |
| HOME-A2-F8-P2 | **Record hand-over to the buyer** | Spare Rent |
| HOME-A3-F1-P3 | **Privacy check on photos** | Spare Rent, Spare Stay |
| HOME-A3-F3-P1 | **Simple plan drawer** | Spare Rent, Spare Stay |
| HOME-A3-F3-P2 | **Plans from Spare Arch** | Spare Rent, Spare Stay |
| HOME-A3-F8-P2 | **Licence on each file** | Spare Rent, Spare Stay |
| HOME-A5-F3-P4 | **Aerial photos and boundary lines** | Spare Rent, Spare Stay, Spare Work |
| HOME-A6-F1-P2 | **Shared listing feed service** | Spare Rent, Spare Stay, Spare Market |
| HOME-A6-F3-P1 | **QR codes and short links** | every app |
| HOME-A6-F7-P2 | **Web search basics** | Spare Market, Spare Rent, Spare Stay |
| HOME-A7-F3-P1 | **Visitor register** | Spare Rent |
| HOME-A7-F5-P1 | **Tenancy rules for selling** | Spare Rent |
| HOME-A9-F1-P1 | **Shared document vault** | Spare Rent |
| HOME-A10-F6-P3 | **Home auction room on the shared auction engine** | Spare Market (the auction engine) |
| HOME-A11-F6-P1 | **Tenancy hand-over** | Spare Rent |
| HOME-A12-F1-P5 | **Shared service catalogue** | Spare Work, Spare Rent, Spare Stay |
| HOME-A12-F2-P3 | **Proof the pro is the registered person** | Spare Work and every app that checks registers |

## Moved parts

These parts moved out of a core feature into a new next feature. Their old IDs are not reused.

| Old ID | New ID | Part |
| --- | --- | --- |
| HOME-A2-F6-P2 | HOME-A2-F11-P1 | **Selling Māori freehold land** |
| HOME-A2-F6-P3 | HOME-A2-F11-P2 | **Older and unusual tenures** |
| HOME-A3-F3-P3 | HOME-A3-F10-P1 | **Plans from a phone scan** |
| HOME-A16-F6-P2 | HOME-A16-F8-P1 | **First other countries** |
| HOME-A17-F2-P3 | HOME-A17-F10-P1 | **Listing translation** |
| HOME-A17-F2-P6 | HOME-A17-F10-P2 | **The original wording counts** |
| HOME-A17-F2-P5 | HOME-A17-F10-P3 | **First community languages** |

## Questions to ask when assessing this map and its demo

- Can one seller sell one home from first sign-up to settlement day using only the core features, and can one buyer buy it?
- Does any screen, message or AI tool look like Spare Home negotiating, recommending an offer or advising on price for a seller? It must not (HM01).
- Does every tool that does more than publish an advert, such as offers, the buyer register, viewings and AI replies, wait on HM01's answer, and is each tool's fallback (HOME-A16-F1-P4) ready behind a feature switch if the answer is no?
- Does any screen take, hold or pass on a deposit or the house price? It must not; that money goes through the lawyers.
- Could someone who only shares the owner's name, such as a tenant or an adult child living in the home, get the checked owner badge? What proves they control the home, and is the owner's address of record told when a listing starts?
- Can anything go live without passing every gate: identity, ownership with proof of control or authority to sell, every co-owner, the seller declaration, disclosures, the listing checks, any earlier agency agreement, the fee and, for a tenanted home, the tenant notice?
- How will buyers find these listings if most of them look on the big property sites, and is the reach question (HM02) answered before launch?
- Are price figures honest: every source and date shown, and clearly not a registered valuation?
- Is the full cost of selling shown before listing, and how does it compare with a typical commission on the same home?
- Does the multi-offer process treat every buyer the same, and does it record who was told what and when?
- Are tenants told in writing, asked before anyone enters to take photos, value or inspect, and able to refuse photos of their belongings and open homes or auctions at the home?
- For units, are the body corporate statements given to buyers before they sign, and the later ones on time?
- Which parts depend on data we may not be allowed to use, such as sale prices and owner names, and what happens if we cannot get it?
- Does it work with a screen reader, in te reo Māori, on a cheap phone, and offline at an open home with no signal?
- Is every New Zealand rule kept in a rules pack, so another country can be added without rewriting the app?
- Can a member export everything and delete their account, and is what the law makes us keep explained?
- Can an AI agent do everything a person can through the same commands, and is it held to the same limits?
- Are all designs, words, templates and icons our own, with other companies named only to say what we replace?
- Which research and legal parts must be finished before any code starts, and who signs them off?
- Is the core list small enough to build first, or should some core features move to next?
- When an AI agent or script calls a command with legal or safety effect, does a person confirm it with a fresh sign-in?
- Is every computed due date confirmed by a lawyer before anyone relies on it?
- Are the parts Spare Home shares with other programs, such as the property record, document vault, ownership and authority checks, co-owner approvals, media licences, plans, the open home register, tenancy rules, listing feeds, the auction engine and the service catalogue, built once in the kit?
- Does the research that gates core features have a work-board piece? Only HM01 to HM05 exist today. Still needed: the one data piece (HOME-A1-F3-P1, HOME-A2-F1-P3, HOME-A4-F1-P1, HOME-A5-F3-P1), the one tax piece (HOME-A2-F3-P4), council reports (HOME-A9-F2-P2), anti-money-laundering (HOME-A16-F2-P1), the privacy impact assessment (HOME-A16-F4-P1), consumer law (HOME-A16-F5-P1), lawyers taking bookings (HOME-A12-F1-P4), trust accounts (HOME-A11-F2-P2), media rights (HOME-A3-F8-P1) and the terms and seller declaration (HOME-A16-F7).
- Could a fake lawyer, conveyancer or agent account plant false trust account details, and are bank details checked through a channel that does not come from the account?
- Can any AI step that reads an offer document, report, listing or message written by someone else be steered by instructions hidden in it?
- Does any core Home part depend on a kit part marked next or later?

## Sources used for this map (researched 2026-10-18, not legal advice)

- [Settled: selling privately](https://www.settled.govt.nz/selling-a-home/selling-your-property/selling-privately/): the seller's lawyer prepares the agreement and holds the deposit in a trust account.
- [Real Estate Agents Act 2008](https://legislation.govt.nz/act/public/2008/66/en/latest/), section 4: agency work is work done in trade for someone else to bring about a transaction; publishing advertising and general materials that help owners sell are among the exclusions (wording to be confirmed in piece HM01).
- [Real Estate Authority: unlicensed agency work](https://www.rea.govt.nz/news/media-releases/rea-warns-that-undertaking-real-estate-agency-work-without-a-license-from-rea-is-an-offence/): doing agency work without a licence is an offence, which is why every tool beyond publishing an advert waits on HM01.
- [UK Enterprise and Regulatory Reform Act 2013, section 70 notes](https://legislation.gov.uk/ukpga/2013/24/section/70/notes?view=plain): in the UK, portals that let buyers and sellers make contact may be in scope of agency law.
- [Real Estate Authority: standard clauses for agency agreements](https://www.rea.govt.nz/real-estate-professionals/listing-a-property/standard-clauses-for-residential-and-rural-agency-agreements/): an agent may claim commission on a private sale to a buyer they introduced within 6 months of the agency ending, or 12 months for rural (checked 2026-10-18).
- [Real Estate Authority: multi-offers](https://rea.govt.nz/real-estate-professionals/the-sales-process-and-general-guidance/multi-offers): no fixed process in law; buyers should be told and treated fairly.
- [Settled: selling a unit title property](https://settled.govt.nz/selling-a-home/selling-your-property/selling-a-unit-title-property): pre-contract and pre-settlement disclosure statements.
- [Tenancy Services: selling a rental property](https://tenancy.govt.nz/ending-a-tenancy/change-of-landlord-or-tenant/selling-a-rental-property): tell tenants in writing; get the tenant's permission before entering to take photos or bring in a valuer or building expert; tenants may refuse photos of their belongings, refuse open homes and auctions at the home, ask for viewings by appointment only and be present; at least 42 days' written notice to end a periodic tenancy when a buyer needs vacant possession, or 90 days to market the home empty; the new owner gives the tenant their name, contact details and address for service (checked 2026-10-19).
- [Rainey Collins: working days and days](https://www.raineycollins.co.nz/your-resources/topics/archives/timeframes-in-agreements-are-crucial-working-days-vs-days): buyers are caught out by how the agreement counts working days. The exact Christmas period the standard agreement excludes is *unverified*.
- [PwC: new indirect collection notices](https://www.pwc.co.nz/insights-and-publications/2026-publications/new-indirect-collection-notification-obligations-privacy-act-2025.html): IPP 3A of the Privacy Amendment Act 2025 applies from 1 May 2026 to personal information collected from someone else.
- [IRD: residential land withholding tax](https://www.ird.govt.nz/property/buying-and-selling/residential-land-withholding-tax/when-rlwt-is-deducted): offshore sellers give the IR1101 form to the conveyancer.
- [Natural Hazards Commission: buying or selling a home with previous claims](https://www.naturalhazards.govt.nz/insurance-and-claims/claims/buying-or-selling-a-home-with-previous-claims/): sellers are advised to request the claims history before listing.
- [LINZ residential land statement](https://www.linz.govt.nz/sites/default/files/doc/LNZ-6320-oio-residential-land-statement-2026-FA2-interactive.pdf): every buyer of residential land completes one for their lawyer.
- [LINZ Data Service](https://www.linz.govt.nz/node/6141): titles, parcels and addresses under CC BY 4.0; owner names only under the restricted licence for personal data.
- [National Trading Standards material information](https://www.nationaltradingstandards.uk/uploads/Material%20information%20-%20Part%20A%20-%20updated%20July%202022.pdf) and [a UK portal's guide to it](https://support.zoopla.co.uk/hc/en-gb/articles/9478309980049): the UK's required listing facts, with Part A on the first page of a listing and Parts B and C within one click; an example of what a country rules pack must hold.
- Incumbent feature sets checked: NZ private-sale and flat-fee agency services (packages, brochures, photography, sites, upgrades such as aerial photos, video, 3D tours and auctioneers); property data sites (estimates, sales history, sold search, rating values, flood zones, schools, zoning, owner pages); UK online agency apps (seller-approved viewing bookings, hosted viewings, offers seen as they arrive, messaging); online negotiation and auction platforms (pre-registered buyers, visible bids, countdown, digital contracts).

The work board is [STATUS.md](STATUS.md). The overview map is [ARCHITECTURE.md](ARCHITECTURE.md).
