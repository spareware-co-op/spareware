# Spare Rent: features map

*Stage: Mapping. Made from [features.json](features.json), which is the source; edit that file and rebuild this page. The architecture is in [ARCHITECTURE.md](ARCHITECTURE.md) and the work board in [STATUS.md](STATUS.md).*

Spare Rent replaces property managers' percentage fees and letting fees, paid rental listings such as Trade Me Property, and paid landlord software. Landlords self-manage legally with good tools, renters apply once and control their own information, and any management work is done by co-op members at cost. Spare Rent never holds a bond, renters never pay a fee, and tenancy law is built in as rules and legal clocks so no duty or deadline is missed. This map lists everything a full replacement needs, top down so nothing is missed: each **area** is a part of the product, each **feature** is something a person can do, and each **part** is a piece that can be built on its own. It was checked against what property managers, listing sites and landlord software offer, and against every journey: first sign-up, finding a home, applying, signing, moving in, paying rent, repairs, inspections, rent rises, ending a tenancy, the Tenancy Tribunal, admin, payments, safety, accessibility, offline, languages and countries, data export, moderation and legal duties. An independent review then added the bond system as it works since 29 June 2026, people-only decisions on applicants and on every step with legal effect, guest access for tenants who don't join, co-op managers as workers, platform income reporting, re-letting with tenants in place, and the Tribunal's strike, retaliation and disaster rules. Nothing here copies another company's design; we follow the same journeys because renters and landlords need them, and build them our own way.

Spare Rent keeps only its own rules. Where the shared kit already builds something, the Spare Rent part says what Spare Rent adds and depends on the kit part by its ID (KIT-...), listed in the [kit's feature map](../kit/FEATURES.md); this map uses 141 kit parts. Parts Spare Rent shares with other programs are mapped once here and proposed for the kit: the property record, document vault, floor plans, visitor register and tenancy rules shared with Spare Home, formal notices and proof of service, read-only bank feeds and text recognition. A program never depends on another program, so links to Spare Work and Spare Stay go through their public command APIs. IDs are never reused or renumbered.

**How Spare Rent uses the marketplace loop.** Listings, viewings, applications and offers run on the kit's marketplace engine. Tenancies skip the loop's pay and hold, auto-confirm and member-panel steps: bonds go only to Tenancy Services, rent goes bank to bank unless both sides choose a regulated payment provider, and only the Tenancy Tribunal decides tenancy disputes. Paid manager and helper jobs (RENT-A15-F2) use the whole loop: the payment is held, the job is confirmed, the helper is paid out, and disputes go to the kit's disputes service. Member panels hear only complaints about the platform and about those paid jobs.

**Core** means needed for a first usable release, **next** soon after, **later** after that. Kinds: library, service, interface, data, integration, research, legal. Sizes: small, medium, large. Legal points were researched on 2026-10-18 and 2026-10-19 and are not legal advice; the research and legal parts say what still needs checking.

**22 areas, 191 features (123 core, 51 next, 17 later), 456 parts.**

## Areas at a glance

| ID | Area | What it covers | Features | Core | Parts |
| --- | --- | --- | --- | --- | --- |
| RENT-A1 | [Accounts, roles and getting started](#rent-a1-accounts-roles-and-getting-started) | One Spareware login for renters, landlords, managers and helpers, with roles per property, the identity and authority checks each role needs, and a guided start for new and existing tenancies. | 8 | 7 | 30 |
| RENT-A2 | [Properties and records](#rent-a2-properties-and-records) | Each rental property has one record that holds its details, rooms, chattels, keys, insurance and documents, stays with the property through every tenancy, and passes to the next owner if it is sold. | 9 | 7 | 26 |
| RENT-A3 | [Listings and advertising](#rent-a3-listings-and-advertising) | Landlords advertise a home truthfully and fully, with the rent stated, every cost shown, the Healthy Homes status visible and the advertising rules checked before it goes live, then reach renters on Spare Rent and elsewhere. | 11 | 8 | 28 |
| RENT-A4 | [Search for renters](#rent-a4-search-for-renters) | Renters find homes by place, price, travel time and what they need, see the full cost first, and get told the moment a matching home is listed. | 8 | 4 | 18 |
| RENT-A5 | [Viewings, applications and choosing tenants](#rent-a5-viewings-applications-and-choosing-tenants) | Renters book viewings, keep one application they control and share only what the law lets a landlord ask; landlords compare applicants fairly, offer the home and let everyone else know, and unused application data is deleted on schedule. | 13 | 11 | 40 |
| RENT-A6 | [Tenancy agreements and moving in](#rent-a6-tenancy-agreements-and-moving-in) | A tenancy agreement built from the official template with every required statement, signed online, then one move-in checklist for money, keys, utilities, pets and the home guide. | 8 | 5 | 23 |
| RENT-A7 | [Bonds](#rent-a7-bonds) | Spare Rent never holds a bond. Since 29 June 2026 bond transactions are online only: Spare Rent does what the Tenancy Bond API supports with each landlord's own RealMe consent, hands the rest to Bond Hub, tells tenants that Tenancy Services will contact them directly about refunds, and keeps the legal clocks so none is late. | 7 | 6 | 22 |
| RENT-A8 | [Rent and money](#rent-a8-rent-and-money) | Rent goes bank to bank by default and is matched automatically; both sides see what is due and paid, get receipts and early warnings, agree payment plans, and handle rent rises and allowed charges within the law. Rent and bonds are not marketplace jobs: they skip the loop's pay and hold, auto-confirm and member panels. | 10 | 6 | 29 |
| RENT-A9 | [Notices, legal clocks and tenancy changes](#rent-a9-notices-legal-clocks-and-tenancy-changes) | Every legal deadline in a tenancy is a clock both sides can see; every formal notice comes from a template with the right dates and proof of service; and changes during a tenancy, from new flatmates to hard situations, follow the law step by step. | 10 | 9 | 33 |
| RENT-A10 | [Inspections and condition reports](#rent-a10-inspections-and-condition-reports) | Move-in, routine and move-out inspections captured room by room with dated photos, done offline if needed, signed by both sides, and compared at the end so the bond is settled on evidence. | 7 | 5 | 17 |
| RENT-A11 | [Maintenance and repairs](#rent-a11-maintenance-and-repairs) | Tenants report problems with photos, urgent ones are escalated, landlords approve and book a trade on Spare Work or their own, access is arranged with the right notice, and every repair stays with the property's history. | 9 | 6 | 17 |
| RENT-A12 | [Healthy Homes and compliance](#rent-a12-healthy-homes-and-compliance) | A checklist and evidence store for every Healthy Homes standard, a heating calculator, the compliance and insulation statements each agreement needs, and a clear view of gaps for landlords and tenants. | 7 | 5 | 13 |
| RENT-A13 | [Ending tenancies, disputes and the Tribunal](#rent-a13-ending-tenancies-disputes-and-the-tribunal) | Tenancies end with the right notice and one move-out flow; disagreements are settled with structured offers where possible, made enforceable through FastTrack Resolution, or go to the Tenancy Tribunal with an evidence pack built from records already kept. Only the Tribunal decides tenancy disputes; member panels handle only complaints about the platform and paid manager and helper jobs. | 8 | 5 | 16 |
| RENT-A14 | [Landlord accounts and tax](#rent-a14-landlord-accounts-and-tax) | Landlords track income and costs per property without separate accounting software, capture receipts, match bank transactions, and get a year-end summary ready for their tax return or their accountant. | 10 | 6 | 18 |
| RENT-A15 | [Co-op property managers and helpers](#rent-a15-co-op-property-managers-and-helpers) | Landlords who don't want to self-manage can hire a co-op member to manage at a published, at-cost price, or book single services such as letting, viewings or inspections; managers get a workspace for many owners, and handovers lose nothing. | 9 | 0 | 19 |
| RENT-A16 | [Messages, notifications and documents](#rent-a16-messages-notifications-and-documents) | One message thread per tenancy kept as a record, formal notices kept apart from chat, each person choosing how they are reached, and every document kept for exactly as long as the law requires. | 8 | 6 | 14 |
| RENT-A17 | [Reviews, trust and safety](#rent-a17-reviews-trust-and-safety) | Fair, checked reviews tied to real tenancies, protection from rental scams, reports anyone can make and follow, help with discrimination and harassment, and moderation with a public log and appeals. | 7 | 6 | 14 |
| RENT-A18 | [Flatmates, rooms and other ways to rent](#rent-a18-flatmates-rooms-and-other-ways-to-rent) | Finding and living with flatmates, splitting rent and bills, and later the other ways people rent: boarding houses, community housing providers and commercial leases. | 8 | 1 | 12 |
| RENT-A19 | [Data, AI and plugins](#rent-a19-data-ai-and-plugins) | Everything a person can do is a command that scripts, AI assistants and plugins can use too; members can export all their data in open formats and bring records in from elsewhere. | 7 | 2 | 14 |
| RENT-A20 | [Access for everyone](#rent-a20-access-for-everyone) | Spare Rent works for everyone: with a screen reader or keyboard, in plain words, easy read and NZ Sign Language, in te reo Māori and other languages, offline, on any phone and on slow connections, by paper and phone for people who are offline, and with a trusted helper for anyone who needs one. | 9 | 7 | 16 |
| RENT-A21 | [Countries and rule packs](#rent-a21-countries-and-rule-packs) | Tenancy law lives in rule packs, not in code, layered on the kit's country packs: New Zealand first, then other countries, states, provinces and cities, each with its own notices, clocks, caps, statements, bond scheme and duties, lawyer-checked and tested in the open. | 8 | 2 | 15 |
| RENT-A22 | [Running the co-op: membership, fees, support and legal duties](#rent-a22-running-the-co-op-membership-fees-support-and-legal-duties) | Landlords, renters and managers join as members and vote; the at-cost fee and the monthly ledger are public; people get help when they are stuck; and the co-op meets its own legal duties for privacy, security, terms and legal requests. | 10 | 9 | 22 |

Work-board pieces this map uses: [RT01](STATUS.md), [RT02](STATUS.md), [RT03](STATUS.md), [RT04](STATUS.md), [RT05](STATUS.md), [K01](../kit/STATUS.md), [K02](../kit/STATUS.md), [K04](../kit/STATUS.md), [K06](../kit/STATUS.md), [HM02](../spare-home/STATUS.md), [HM04](../spare-home/STATUS.md), [MK04](../spare-market/STATUS.md), [MK05](../spare-market/STATUS.md), [M04](../spare-social/STATUS.md). Spare Rent's own pieces are on its [STATUS.md](STATUS.md); the others are on the kit's or another program's board, because their findings apply here too. Parts with no piece have no work-board item yet, including the Tenancy Bond API research (RENT-A7-F1-P1); the last question below asks for those pieces.

## RENT-A1 Accounts, roles and getting started

One Spareware login for renters, landlords, managers and helpers, with roles per property, the identity and authority checks each role needs, and a guided start for new and existing tenancies.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A1-F1 Sign up and sign in | Create a Spareware account with a passkey and say whether you are renting, letting or managing. | core | Sign-up and sign-in screens; First-run guide per role; Renting profile fields; Account recovery; Tenancy Services registration check |
| RENT-A1-F2 Verify who you are | Prove your identity once through a regulated checker, so others see a verified badge without seeing your documents. | core | Identity check hookup; Verified badge; Which checks each role needs |
| RENT-A1-F3 Prove you can let a property | Show you own the property or have the owner's written authority before a listing goes live. | core | Authority-to-let record; Ownership evidence check; Title data access; Owner's signed authority |
| RENT-A1-F4 Landlords as people, trusts and companies | Let a property as one person, joint owners, a family trust or a company, with each owner's share recorded. | core | Landlord entity model; Co-owner invitations; Address for service |
| RENT-A1-F5 Roles and shared access | Give a co-owner, manager, accountant, trade or flatmate exactly the access they need, and take it back any time. | core | Role and permission model; Invite and revoke screen; Access log; Agent while the landlord is overseas |
| RENT-A1-F6 Bring an existing tenancy | Move a tenancy you already have into Spare Rent, with its bond, agreement and rent history. | core | Existing tenancy wizard; Find an existing bond; Upload old records; Invite current tenants |
| RENT-A1-F7 Switch from a property manager | Leave a property manager and take over, with tenants and Tenancy Services told about the change. | next | Handover checklist; Bond change hand-off; Notice of new contact details |
| RENT-A1-F8 Close your account | Leave Spare Rent and take your records, while the other side keeps theirs and records the law says to keep are kept for that time only. | core | Close-account flow; Legal holds on records; The other side keeps the shared record; If Spare Rent itself closes |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A1-F1-P1 | **Sign-up and sign-in screens**: The kit's passkey sign-up and sign-in screens, with Spare Rent's choice of renting, letting or managing added. | interface | small | KIT-A1-F1-P2 (Sign-up flow), KIT-A1-F2-P1 (Passkey sign-in), KIT-A1-F2-P4 (Sign-in screens) |  |
| RENT-A1-F1-P2 | **First-run guide per role**: A short guided start that differs for a renter, a landlord and a manager. | interface | small | RENT-A1-F1-P1 (Sign-up and sign-in screens), KIT-A20-F6-P1 (First-run guide) |  |
| RENT-A1-F1-P3 | **Renting profile fields**: Preferred name and contact details on the kit's profile; the email for formal notices lives only in the address-for-service record (RENT-A16-F4-P1). | data | small | KIT-A1-F5-P1 (Profile model) |  |
| RENT-A1-F1-P4 | **Account recovery**: Uses the kit's recovery codes and assisted recovery; Spare Rent adds nothing of its own. | service | small | KIT-A1-F4-P1 (Recovery codes and backup methods), KIT-A1-F4-P2 (Assisted recovery) |  |
| RENT-A1-F1-P5 | **Tenancy Services registration check**: During landlord set-up, checks the landlord is registered with Tenancy Services and has the RealMe login the bond system needs; registers them through the Tenancy Bond API, which allows it, or guides them through Bond Hub. | interface | small | RENT-A1-F1-P1 (Sign-up and sign-in screens), RENT-A7-F1-P6 (Landlord consent to the bond system) |  |
| RENT-A1-F2-P1 | **Identity check hookup**: Sends a person to the kit's identity check when Spare Rent's rules for their role need it, and keeps only the result. | integration | small | KIT-A2-F1-P2 (Identity check link), KIT-A2-F1-P3 (Check results store) |  |
| RENT-A1-F2-P2 | **Verified badge**: The kit's verified badge shown on Spare Rent profiles, listings and applications. | interface | small | KIT-A2-F5-P1 (Verified badge), RENT-A1-F2-P1 (Identity check hookup) |  |
| RENT-A1-F2-P3 | **Which checks each role needs**: Which identity checks renters, landlords and managers truly need and what the law allows, declared as Spare Rent's rules in the kit's check rules per app. | research | small | KIT-A2-F2-P2 (Check rules per app) |  |
| RENT-A1-F3-P1 | **Authority-to-let record**: Records who owns the property and who may let it: owner, co-owner, trustee, director or manager. | data | medium | kit-core, kit-ops |  |
| RENT-A1-F3-P2 | **Ownership evidence check**: Checks the owner's name against a land title or rates record the owner supplies. | service | medium | RENT-A1-F3-P1 (Authority-to-let record), svc-trust, KIT-A2-F7-P1 (Business number lookup) |  |
| RENT-A1-F3-P3 | **Title data access**: Research how land title records can be checked, at what cost and under what terms. | research | small |  |  |
| RENT-A1-F3-P4 | **Owner's signed authority**: An owner signs an authority that lets a manager or helper act for them. | interface | small | RENT-A1-F3-P1 (Authority-to-let record), RENT-A6-F2-P1 (E-signing) |  |
| RENT-A1-F4-P1 | **Landlord entity model**: Landlords who are joint owners, trusts or companies, built on the kit's organisation model with each owner's share added. | data | small | KIT-A1-F7-P1 (Organisation model) |  |
| RENT-A1-F4-P2 | **Co-owner invitations**: Invite co-owners to confirm their share and their say in decisions. | interface | small | RENT-A1-F4-P1 (Landlord entity model), svc-messaging |  |
| RENT-A1-F4-P3 | **Address for service**: Takes the landlord's addresses for service from the one address-for-service record (RENT-A16-F4-P1) into agreements and notices. | data | small | RENT-A1-F4-P1 (Landlord entity model), RENT-A16-F4-P1 (Address for service record) |  |
| RENT-A1-F5-P1 | **Role and permission model**: Spare Rent's roles per property and per tenancy, granted through the kit's roles and checked by the kit's permission checks on every command. | data | small | KIT-A1-F5-P2 (Roles per app), KIT-A15-F1-P4 (Permission checks) |  |
| RENT-A1-F5-P2 | **Invite and revoke screen**: Invite someone to a role, see who has access and remove it in one step. | interface | small | RENT-A1-F5-P1 (Role and permission model) |  |
| RENT-A1-F5-P3 | **Access log**: Owners and tenants see who opened or changed their records and when. | interface | small | RENT-A1-F5-P1 (Role and permission model), kit-ops, KIT-A18-F3-P1 (Privacy page) |  |
| RENT-A1-F5-P4 | **Agent while the landlord is overseas**: Prompts a landlord leaving New Zealand for more than 21 days in a row to appoint a local agent, gives the tenant the agent's name, contact details and address for service, and starts the bond record update; not doing this is an unlawful act. | service | small | RENT-A1-F5-P1 (Role and permission model), RENT-A16-F4-P1 (Address for service record), RENT-A7-F5-P1 (Change of tenant or landlord), RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A1-F6-P1 | **Existing tenancy wizard**: Steps through the property, the tenants, the agreement, the bond and the rent already paid. | interface | medium | RENT-A2-F1-P2 (Rental fields on the shared property record), RENT-A6-F1-P1 (Agreement builder) |  |
| RENT-A1-F6-P2 | **Find an existing bond**: Finds the bonds linked to the landlord through their consent to the bond system, and matches one by its number; lookups are checked against what the API really offers. | integration | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A7-F1-P6 (Landlord consent to the bond system) |  |
| RENT-A1-F6-P3 | **Upload old records**: Upload and tag an existing agreement, reports and receipts. | interface | small | RENT-A2-F6-P1 (Shared document vault) |  |
| RENT-A1-F6-P4 | **Invite current tenants**: Invites tenants to join; if they don't, the tenancy still runs through guest links (RENT-A16-F8-P1). | interface | small | svc-messaging, RENT-A16-F8-P1 (Guest links for tenancies) |  |
| RENT-A1-F7-P1 | **Handover checklist**: Keys, records, bond, rent account and open jobs to collect from the old manager. | interface | small | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A1-F7-P2 | **Bond change hand-off**: Starts the change of landlord on the bond in the one bond change part (RENT-A7-F5-P1). | integration | small | RENT-A7-F5-P1 (Change of tenant or landlord) |  |
| RENT-A1-F7-P3 | **Notice of new contact details**: Tells tenants who their landlord contact and address for service now are. | service | small | RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A1-F8-P1 | **Close-account flow**: The kit's account deletion, with Spare Rent's notes on what is exported, what the other side keeps and how long records are held. | interface | small | KIT-A18-F2-P1 (Account deletion), RENT-A19-F1-P1 (Full export) |  |
| RENT-A1-F8-P2 | **Legal holds on records**: Tenancy and tax record types added to the kit's retention schedules and legal holds, deleted when their time is up. | service | small | KIT-A18-F6-P1 (Retention schedules), RENT-A16-F5-P1 (Retention rules) |  |
| RENT-A1-F8-P3 | **The other side keeps the shared record**: When a landlord or tenant closes their account, the other side keeps their copy of the agreement, receipts, notices, messages and reports; only the leaver's own profile and private data are removed. | service | medium | RENT-A1-F8-P1 (Close-account flow), KIT-A18-F2-P1 (Account deletion), KIT-A15-F1-P6 (Erasable personal data) |  |
| RENT-A1-F8-P4 | **If Spare Rent itself closes**: A wind-down plan that keeps every record the law says to keep available and exportable for its full time, even if Spare Rent stops running. | legal | small | RENT-A19-F1-P1 (Full export), KIT-A18-F6-P1 (Retention schedules), KIT-A22-F3-P2 (Disaster recovery) |  |

## RENT-A2 Properties and records

Each rental property has one record that holds its details, rooms, chattels, keys, insurance and documents, stays with the property through every tenancy, and passes to the next owner if it is sold.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A2-F1 Add a property | Add a property by searching its address, then confirm its type, rooms and features. | core | Address search; Rental fields on the shared property record; Add-property screen; Parcel and title data |
| RENT-A2-F2 Rooms and chattels | List each room and the chattels the landlord provides, which then feed the agreement, inspections and tax. | core | Room, chattel and appliance list; Chattel templates; Room and chattel editor |
| RENT-A2-F3 Buildings, units and body corporates | Manage a block of flats or a unit title, with the body corporate rules attached to every tenancy. | next | Building with many units; Body corporate details; Rules given to tenants |
| RENT-A2-F4 Keys and access | Track who holds which keys, fobs and codes, and when they were handed back. | core | Key register; Key handover record |
| RENT-A2-F5 Insurance | Keep the insurance on the property and produce the insurance statement every tenancy agreement needs. | core | Insurance policy record; Insurance statement; Renewal reminders |
| RENT-A2-F6 Property documents vault | Store certificates, consents, manuals, warranties and plans against the property. | core | Shared document vault; Document types and expiry dates; Vault browser |
| RENT-A2-F7 Lawful dwelling check | See a warning if a place may not be lawful to rent as a home, before anyone moves in. | core | Unlawful and unconsented dwellings; Owner declaration and evidence |
| RENT-A2-F8 Sell or hand over a property | Pass the property record, tenancy and history to a new owner when it is sold with tenants in place. | next | Property handover; Sale notices to tenants; Bond change hand-off on sale; Tax prompts on sale |
| RENT-A2-F9 Portfolio view | See every property you let on one screen, with rent, compliance and anything that needs you. | core | Portfolio dashboard; Needs-you list |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A2-F1-P1 | **Address search**: The kit's address search, pinned on the map, with a check for units and flats at the same address. | integration | small | KIT-A7-F2-P2 (Address search), KIT-A7-F2-P3 (NZ address data) |  |
| RENT-A2-F1-P2 | **Rental fields on the shared property record**: Spare Rent's rental fields, such as units, heating and parking, on one property record shared with Spare Home (HOME-A2-F1-P2) and Spare Stay; proposed to move into the kit once. | data | medium | kit-core, kit-ops, kit-store |  |
| RENT-A2-F1-P3 | **Add-property screen**: A short form that fills itself from the address and asks only what is missing. | interface | small | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A2-F1-P4 | **Parcel and title data**: Parcel and title datasets, licences and update rates, done once with Spare Home's open property data research (HOME-A2-F1-P3); addresses come from the kit's NZ address data. | research | small | KIT-A7-F2-P3 (NZ address data) |  |
| RENT-A2-F2-P1 | **Room, chattel and appliance list**: Rooms and the chattels and appliances in each, with make, model, purchase date, manual and warranty, as the one register of them. | data | small | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A2-F2-P2 | **Chattel templates**: Common chattels ready to pick, such as a heat pump, oven, curtains or a dishwasher. | data | small | RENT-A2-F2-P1 (Room, chattel and appliance list) |  |
| RENT-A2-F2-P3 | **Room and chattel editor**: Add, move and photograph rooms and chattels. | interface | small | RENT-A2-F2-P1 (Room, chattel and appliance list), engine-media |  |
| RENT-A2-F3-P1 | **Building with many units**: One building record that holds many units, with shared areas and shared costs. | data | medium | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A2-F3-P2 | **Body corporate details**: Body corporate contact, levies and operational rules stored against the unit. | data | small | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A2-F3-P3 | **Rules given to tenants**: Body corporate rules attached to the agreement so tenants receive them. | service | small | RENT-A6-F1-P1 (Agreement builder) |  |
| RENT-A2-F4-P1 | **Key register**: Every key, fob, remote and code, and who holds it now. | data | small | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A2-F4-P2 | **Key handover record**: Both sides confirm a key handover with a signature and the date. | interface | small | RENT-A6-F2-P1 (E-signing) |  |
| RENT-A2-F5-P1 | **Insurance policy record**: Insurer, cover and excess amounts for the building and the landlord's chattels. | data | small | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A2-F5-P2 | **Insurance statement**: Fills the insurance details a tenancy agreement must include. | service | small | RENT-A2-F5-P1 (Insurance policy record), engine-pdf |  |
| RENT-A2-F5-P3 | **Renewal reminders**: Reminds the landlord before a policy lapses. | service | small | RENT-A2-F5-P1 (Insurance policy record), svc-messaging |  |
| RENT-A2-F6-P1 | **Shared document vault**: Spare Rent's use of one document vault on the kit's file service, shared with Spare Home (HOME-A9-F1-P1) and proposed to move into the kit once; Spare Rent adds only tenancy document types. | service | small | KIT-A15-F7-P1 (File uploads), KIT-A15-F8-P1 (Sharing and access), kit-store, RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A2-F6-P2 | **Document types and expiry dates**: Each document has a type, who may see it and an optional expiry date. | data | small | RENT-A2-F6-P1 (Shared document vault) |  |
| RENT-A2-F6-P3 | **Vault browser**: Find, preview and share documents by type and date. | interface | small | RENT-A2-F6-P1 (Shared document vault) |  |
| RENT-A2-F7-P1 | **Unlawful and unconsented dwellings**: Research what tenancy and building law say about renting a place that is not consented as a home. | research | small |  | [RT01](STATUS.md) |
| RENT-A2-F7-P2 | **Owner declaration and evidence**: The owner confirms the dwelling is lawful and can attach consents and certificates. | interface | small | RENT-A2-F6-P1 (Shared document vault), RENT-A2-F7-P1 (Unlawful and unconsented dwellings) |  |
| RENT-A2-F8-P1 | **Property handover**: Passes the record and history to the new owner, the old owner keeping only their own accounts, as one flow shared with Spare Home's tenancy hand-over (HOME-A11-F6-P1) and proposed for the kit. | service | medium | RENT-A2-F1-P2 (Rental fields on the shared property record), kit-ops |  |
| RENT-A2-F8-P2 | **Sale notices to tenants**: Tells tenants the property is for sale and later who the new landlord is, through the formal notice builder; the same notice Spare Home sends (HOME-A7-F5-P3). | service | small | RENT-A9-F2-P2 (Notice builder), RENT-A9-F2-P5 (Service, not just delivery) |  |
| RENT-A2-F8-P3 | **Bond change hand-off on sale**: Starts the change of landlord in the one bond change part (RENT-A7-F5-P1) when ownership changes. | integration | small | RENT-A7-F5-P1 (Change of tenant or landlord), RENT-A2-F8-P1 (Property handover) |  |
| RENT-A2-F8-P4 | **Tax prompts on sale**: Prompts the landlord about tax when a rental is sold, such as the bright-line test and depreciation recovered, by the rules in force on the sale date. | service | small | RENT-A14-F4-P2 (Rental tax rules), RENT-A14-F4-P3 (Tax rules by year) |  |
| RENT-A2-F9-P1 | **Portfolio dashboard**: Properties, tenancies, rent status and compliance at a glance; the one compliance overview across properties. | interface | medium | RENT-A2-F1-P2 (Rental fields on the shared property record), RENT-A12-F1-P2 (Standards model) |  |
| RENT-A2-F9-P2 | **Needs-you list**: A list built from legal clocks, arrears, repairs and expiring documents, soonest first. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A8-F1-P1 (Rent ledger) |  |

## RENT-A3 Listings and advertising

Landlords advertise a home truthfully and fully, with the rent stated, every cost shown, the Healthy Homes status visible and the advertising rules checked before it goes live, then reach renters on Spare Rent and elsewhere.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A3-F1 Create a listing | Write a rental listing with the rent, dates, photos and features, guided so it follows the advertising rules. | core | Listing model; Listing editor; Advertising rules check; Advertising rules research |
| RENT-A3-F2 Photos, video and floor plans | Add photos, a video walk-through and a floor plan, sized for phones and stripped of hidden location data. | core | Listing photo upload; Privacy prompts on photos; Video walk-through; Floor plans |
| RENT-A3-F3 Every cost up front | Show the rent, bond, pet bond, rent in advance and other costs a renter will pay before they apply. | core | Full-cost panel; Healthy Homes summary; Power, internet and water facts |
| RENT-A3-F4 Price the rent fairly | See what similar homes nearby rent for, from public bond data, before setting the rent. | core | Market rent data; Rent guide in the editor; Bond data licence and detail |
| RENT-A3-F5 Reach more renters | Publish once and send the listing to other property sites where their terms allow. | next | Which sites take rental feeds; Listing feed export; Public listing pages |
| RENT-A3-F6 Listing status and dates | Mark a listing as open, under offer, let or withdrawn, and set the date the home is available. | core | Listing lifecycle; Close when signed |
| RENT-A3-F7 Answer enquiries | Answer renters' questions in one inbox, with saved answers for common questions. | core | Enquiry inbox; Saved replies and public answers |
| RENT-A3-F8 Verified listings | Show renters that a listing comes from a verified person with authority to let. | core | Verified-listing badge; Scam checks on new listings |
| RENT-A3-F9 Listing fee at cost | See the at-cost fee for a listing, if there is one, and what it paid for. | core | Listing fee from the fee model; Fee shown before publishing |
| RENT-A3-F10 Show the area, not the address | Show only an approximate location until a viewing is booked, which protects tenants and empty homes. | next | Approximate location option |
| RENT-A3-F11 Listing statistics | See how many people viewed, asked about, booked and applied for a listing, and be told when a listing looks stale. | next | Listing statistics; Stale listing check |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A3-F1-P1 | **Listing model**: A rental listing on the shared marketplace engine: the home, rent, dates, terms and who may apply. | data | medium | kit-market, RENT-A2-F1-P2 (Rental fields on the shared property record), KIT-A5-F1-P1 (Listing model) |  |
| RENT-A3-F1-P2 | **Listing editor**: A guided editor that fills what it can from the property record and explains each field. | interface | medium | RENT-A3-F1-P1 (Listing model), KIT-A5-F1-P2 (Listing editor) |  |
| RENT-A3-F1-P3 | **Advertising rules check**: Checks the rent is stated, no bids are invited and the wording does not discriminate, before publishing. | service | medium | RENT-A3-F1-P1 (Listing model), RENT-A21-F1-P1 (Rule pack format), KIT-A5-F1-P3 (Listing checks) |  |
| RENT-A3-F1-P4 | **Advertising rules research**: Research the rules for rental adverts: rent shown, the bidding ban, the Human Rights Act and the Fair Trading Act. | research | small |  | [RT01](STATUS.md) |
| RENT-A3-F2-P1 | **Listing photo upload**: Listing photos go through the kit's file uploads and photo metadata stripping, which remove hidden location data; inspection photos use their own evidence pipeline (RENT-A10-F1-P5), which keeps it. | service | small | KIT-A15-F7-P1 (File uploads), KIT-A15-F7-P3 (Photo metadata stripping), engine-media |  |
| RENT-A3-F2-P2 | **Privacy prompts on photos**: Prompts the landlord when a photo shows people, mail, documents, number plates or a tenant's belongings; one check shared with Spare Home (HOME-A3-F1-P3) and proposed for the kit's media service. | service | small | engine-media, RENT-A3-F2-P1 (Listing photo upload) |  |
| RENT-A3-F2-P3 | **Video walk-through**: Upload a short video tour that plays on any phone. | service | medium | engine-media, kit-store |  |
| RENT-A3-F2-P4 | **Floor plans**: A simple floor plan from the plan drawer shared with Spare Home (HOME-A3-F3-P1, HOME-A3-F3-P2) and Spare Stay, built once on the 2D drawing engine and proposed for the kit; plans can come from Spare Construct. | library | medium | engine-draw2d | [HM04](../spare-home/STATUS.md) |
| RENT-A3-F3-P1 | **Full-cost panel**: Rent, bond, pet bond if any, rent in advance and expected outgoings, worked out from the rule pack. | interface | small | RENT-A3-F1-P1 (Listing model), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A3-F3-P2 | **Healthy Homes summary**: The home's Healthy Homes status shown on the listing. | interface | small | RENT-A12-F1-P2 (Standards model) |  |
| RENT-A3-F3-P3 | **Power, internet and water facts**: Heating type, insulation, fibre availability and whether water is metered. | data | small | RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A3-F4-P1 | **Market rent data**: Imports the public rental bond statistics by area, dwelling type and bedrooms; it uses only public data and never pools members' private rents. | integration | medium | svc-maps, RENT-A3-F4-P3 (Bond data licence and detail) |  |
| RENT-A3-F4-P2 | **Rent guide in the editor**: Shows the typical range for similar homes next to the rent field. | interface | small | RENT-A3-F4-P1 (Market rent data), RENT-A3-F1-P1 (Listing model) |  |
| RENT-A3-F4-P3 | **Bond data licence and detail**: Research the licence of the bond statistics and how fine-grained the public data is. | research | small |  |  |
| RENT-A3-F5-P1 | **Which sites take rental feeds**: Research which property sites accept listings by feed, at what cost and on what terms. | research | small |  | [HM02](../spare-home/STATUS.md) |
| RENT-A3-F5-P2 | **Listing feed export**: A listing feed in an open, documented format for sites that accept it. | integration | medium | RENT-A3-F1-P1 (Listing model), RENT-A3-F5-P1 (Which sites take rental feeds) |  |
| RENT-A3-F5-P3 | **Public listing pages**: Fast public pages that search engines can read and people can share. | interface | small | RENT-A3-F1-P1 (Listing model) |  |
| RENT-A3-F6-P1 | **Listing lifecycle**: Open, under offer, let and withdrawn states on the marketplace engine, each one an operation. | service | small | kit-market, RENT-A3-F1-P1 (Listing model) |  |
| RENT-A3-F6-P2 | **Close when signed**: Closes the listing and tells waiting applicants when an agreement is signed. | service | small | RENT-A6-F2-P1 (E-signing), svc-messaging |  |
| RENT-A3-F7-P1 | **Enquiry inbox**: All questions for a listing in one place, linked to the person's profile. | interface | medium | svc-messaging, RENT-A3-F1-P1 (Listing model) |  |
| RENT-A3-F7-P2 | **Saved replies and public answers**: Reusable replies, and answers the landlord can publish on the listing for everyone. | interface | small | svc-messaging |  |
| RENT-A3-F8-P1 | **Verified-listing badge**: Shown only when identity and authority to let are both confirmed. | service | small | RENT-A1-F2-P1 (Identity check hookup), RENT-A1-F3-P1 (Authority-to-let record), svc-trust |  |
| RENT-A3-F8-P2 | **Scam checks on new listings**: Checks for copied photos, rent far below market and other signs of a fake listing. | service | medium | svc-trust, RENT-A3-F4-P1 (Market rent data) | [MK04](../spare-market/STATUS.md) |
| RENT-A3-F9-P1 | **Listing fee from the fee model**: Shows the listing part of Spare Rent's one fee model (RENT-A22-F2-P1), paid by the landlord, if there is one. | service | small | RENT-A22-F2-P1 (Fee model) |  |
| RENT-A3-F9-P2 | **Fee shown before publishing**: The fee and a link to the ledger shown before the landlord confirms. | interface | small | RENT-A3-F9-P1 (Listing fee from the fee model) |  |
| RENT-A3-F10-P1 | **Approximate location option**: Shows an approximate area on the listing and map until a viewing is booked, using the kit's approximate location; recommended for occupied and empty homes. | interface | small | KIT-A7-F10-P1 (Approximate location), RENT-A3-F1-P1 (Listing model), RENT-A5-F1-P1 (Viewing slots) |  |
| RENT-A3-F11-P1 | **Listing statistics**: Views, enquiries, viewings booked and applications per listing, counted without tracking anyone. | service | small | RENT-A3-F1-P1 (Listing model), KIT-A22-F8-P1 (Private usage counts) |  |
| RENT-A3-F11-P2 | **Stale listing check**: Asks the landlord to update or withdraw a listing that has been open a long time with no activity, or whose home has been let. | service | small | RENT-A3-F6-P1 (Listing lifecycle), RENT-A3-F11-P1 (Listing statistics) |  |

## RENT-A4 Search for renters

Renters find homes by place, price, travel time and what they need, see the full cost first, and get told the moment a matching home is listed.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A4-F1 Search rentals | Search homes to rent by place, rent, bedrooms, pets, furnishing and move-in date. | core | Rental search needs; Rental listings in the search service; Search screen and filters; Results with full cost |
| RENT-A4-F2 Map search | Browse rentals on a map and draw the area you want to live in. | core | Map with listings; Draw-an-area filter |
| RENT-A4-F3 Travel time | Filter homes by travel time to work or school by car, bus, bike or on foot. | next | Travel-time areas; Public transport timetables |
| RENT-A4-F4 Neighbourhood facts | See school zones, shops, transport and natural hazard information near a home. | next | School enrolment zones; Hazard and flood layers; Neighbourhood data sources |
| RENT-A4-F5 Saved searches and alerts | Save a search and get told the moment a matching home is listed. | core | Rental saved searches; New-match alerts |
| RENT-A4-F6 Shortlist and compare | Keep a shortlist of homes and compare them side by side. | core | Shortlist; Compare view |
| RENT-A4-F7 Find an accessible home | Find homes with step-free access, accessible bathrooms and other features you need. | next | Accessibility features on listings; Accessibility filters |
| RENT-A4-F8 Search through your own AI | Let your own AI assistant search and shortlist homes for you through the same commands. | next | Search commands for AI |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A4-F1-P1 | **Rental search needs**: Spare Rent's filters and ranking needs, fed into the kit's search engine choice. | research | small | KIT-A6-F1-P1 (Search engine choice) | [MK05](../spare-market/STATUS.md) |
| RENT-A4-F1-P2 | **Rental listings in the search service**: Rental listings and their filters declared to the kit's one search service, so Spare Rent runs no search engine of its own. | service | medium | KIT-A6-F1-P2 (Search service), RENT-A3-F1-P1 (Listing model), RENT-A4-F1-P1 (Rental search needs) |  |
| RENT-A4-F1-P3 | **Search screen and filters**: Filters that match how renters choose, saved between visits. | interface | medium | RENT-A4-F1-P2 (Rental listings in the search service) |  |
| RENT-A4-F1-P4 | **Results with full cost**: Each result shows weekly rent and the total needed to move in. | interface | small | RENT-A4-F1-P2 (Rental listings in the search service), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A4-F2-P1 | **Map with listings**: Open map tiles with listing pins that cluster when zoomed out. | interface | medium | svc-maps, RENT-A4-F1-P2 (Rental listings in the search service), KIT-A7-F1-P2 (Map display) |  |
| RENT-A4-F2-P2 | **Draw-an-area filter**: Draw a shape on the map to search only inside it. | interface | small | svc-maps, RENT-A4-F1-P2 (Rental listings in the search service) |  |
| RENT-A4-F3-P1 | **Travel-time areas**: Areas reachable within a set time, from the open routing engine. | service | medium | svc-maps, KIT-A7-F3-P2 (Routing service) |  |
| RENT-A4-F3-P2 | **Public transport timetables**: Imports published transit timetables so bus and train times count. | integration | medium | svc-maps |  |
| RENT-A4-F4-P1 | **School enrolment zones**: Shows which school zones a home is in, from open government data. | integration | small | svc-maps |  |
| RENT-A4-F4-P2 | **Hazard and flood layers**: Shows council hazard maps where their licences allow. | integration | medium | svc-maps |  |
| RENT-A4-F4-P3 | **Neighbourhood data sources**: Research open neighbourhood datasets across councils and their licences. | research | small |  |  |
| RENT-A4-F5-P1 | **Rental saved searches**: Spare Rent's filters in the kit's saved searches, with how often to be told. | data | small | KIT-A6-F8-P1 (Saved searches), RENT-A4-F1-P2 (Rental listings in the search service) |  |
| RENT-A4-F5-P2 | **New-match alerts**: New-listing alerts sent through the kit's saved search alerts by push, email or text. | service | small | KIT-A6-F8-P1 (Saved searches), RENT-A4-F5-P1 (Rental saved searches), svc-messaging |  |
| RENT-A4-F6-P1 | **Shortlist**: Save homes, add private notes and see when a listing changes. | interface | small | RENT-A3-F1-P1 (Listing model) |  |
| RENT-A4-F6-P2 | **Compare view**: Rent, costs, size, travel time and Healthy Homes status side by side. | interface | small | RENT-A3-F1-P1 (Listing model), RENT-A12-F1-P2 (Standards model) |  |
| RENT-A4-F7-P1 | **Accessibility features on listings**: Structured fields for step-free entry, door widths, level showers and more. | data | small | RENT-A3-F1-P1 (Listing model) |  |
| RENT-A4-F7-P2 | **Accessibility filters**: Search by the accessibility features that matter to you. | interface | small | RENT-A4-F1-P2 (Rental listings in the search service) |  |
| RENT-A4-F8-P1 | **Search commands for AI**: Search, shortlist and alert commands exposed on the MCP server. | service | small | kit-mcp, RENT-A4-F1-P2 (Rental listings in the search service), KIT-A17-F1-P1 (MCP server), KIT-A17-F4-P1 (Agent limits) |  |

## RENT-A5 Viewings, applications and choosing tenants

Renters book viewings, keep one application they control and share only what the law lets a landlord ask; landlords compare applicants fairly, offer the home and let everyone else know, and unused application data is deleted on schedule.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A5-F1 Book a viewing | Pick a viewing time from the landlord's slots, or ask for a private viewing. | core | Viewing slots; Booking screen and reminders; Calendar export; Fair booking limits |
| RENT-A5-F2 Run an open home | Check people in at an open home and message everyone if the time changes. | core | Check-in; Change and cancel messages; Safety for whoever hosts a viewing |
| RENT-A5-F3 Virtual viewings | View a home by live video or a recorded walk-through when you can't be there. | next | Live video viewing; Video viewing needs |
| RENT-A5-F4 Stay safe at viewings | Share your viewing details with someone you trust and report anything unsafe. | core | Share-my-viewing link; Report from a viewing |
| RENT-A5-F5 Reusable renter profile | Keep one renter profile you control, and reuse it for every application. | core | What landlords may ask, and when; Renter profile store; Profile builder; Stage rules; Limits built into the profile |
| RENT-A5-F6 Apply for a home | Apply in one step, sharing only what this landlord may ask for, and see who has looked at it. | core | Per-listing consent share; Apply screen; Who-looked log; View-only shared documents |
| RENT-A5-F7 Apply as a household | Apply together with flatmates or family as one household. | core | Household application; Invite co-applicants |
| RENT-A5-F8 References | Ask past landlords and employers for a reference that comes back in a standard form. | core | Reference requests; Renter-held reference store |
| RENT-A5-F9 Checks with consent | Run a credit or background check only when the renter agrees, and show them the result. | next | Credit reporter hookup; Credit and record check rules; Collected-from-others notice |
| RENT-A5-F10 Compare applicants fairly | Shortlist applicants against stated criteria, with tools that reduce bias. | core | Criteria set first; Applicant comparison board; Hide names and photos; Discrimination grounds in housing; People decide, not software |
| RENT-A5-F11 Offer and decline | Offer the home to the chosen applicant and tell the others kindly and quickly. | core | Offer with expiry; Decline messages |
| RENT-A5-F12 Application data deleted on time | Have applications that didn't go ahead deleted on a published schedule, and see and correct what is held. | core | Application retention schedule; See and correct |
| RENT-A5-F13 Re-let while tenants still live there | Show the home and take photos while the current tenants still live there, with their consent, the notice the law requires and allowed hours. | core | Tenant consent to viewings; Viewing notice and hours; Consent for interior photos; Rules for occupied homes |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A5-F1-P1 | **Viewing slots**: Open homes and private slots with limits on numbers, on the marketplace engine. | service | medium | kit-market, RENT-A3-F1-P1 (Listing model), KIT-A5-F5-P1 (Availability calendars) |  |
| RENT-A5-F1-P2 | **Booking screen and reminders**: Book, change or cancel a viewing, with a reminder before it starts. | interface | small | RENT-A5-F1-P1 (Viewing slots), svc-messaging |  |
| RENT-A5-F1-P3 | **Calendar export**: Adds viewings to any calendar app through the kit's calendar import and export. | integration | small | KIT-A5-F5-P2 (Calendar import and export), RENT-A5-F1-P1 (Viewing slots) |  |
| RENT-A5-F1-P4 | **Fair booking limits**: Per-person caps on viewing bookings and applications, plus the kit's rules for automated acceptance, so scripts and agents can't take every viewing slot or mass-apply to listings. | library | small | RENT-A5-F1-P1 (Viewing slots), KIT-A6-F4-P3 (Automated acceptance rules), KIT-A17-F4-P1 (Agent limits), KIT-A10-F11-P1 (Spam limits) |  |
| RENT-A5-F2-P1 | **Check-in**: Checks attendees in by code or from the booking list, with a walk-in option, on one visitor register shared with Spare Home's open homes (HOME-A7-F3-P1) and proposed for the kit. | interface | small | RENT-A5-F1-P1 (Viewing slots) |  |
| RENT-A5-F2-P2 | **Change and cancel messages**: Tells every booked person at once if a viewing moves or is cancelled. | service | small | RENT-A5-F1-P1 (Viewing slots), svc-messaging |  |
| RENT-A5-F2-P3 | **Safety for whoever hosts a viewing**: A landlord or helper hosting a viewing alone can share where they are with a trusted contact, raise an alert and check in afterwards. | interface | small | RENT-A5-F1-P1 (Viewing slots), KIT-A10-F2-P1 (Trusted contact link), KIT-A10-F1-P1 (Emergency button) |  |
| RENT-A5-F3-P1 | **Live video viewing**: A live video viewing on the kit's video calls, booked like any other viewing. | integration | small | KIT-A8-F11-P2 (Call service), RENT-A5-F1-P1 (Viewing slots) |  |
| RENT-A5-F3-P2 | **Video viewing needs**: Spare Rent's needs for video viewings, such as filming from a phone while walking, fed into the kit's call method choice. | research | small | KIT-A8-F11-P1 (Call method choice) |  |
| RENT-A5-F4-P1 | **Share-my-viewing link**: Sends the address and time of a viewing to someone the renter chooses. | interface | small | RENT-A5-F1-P1 (Viewing slots) |  |
| RENT-A5-F4-P2 | **Report from a viewing**: Report unsafe behaviour or a suspected scam straight from the booking. | interface | small | svc-trust, RENT-A5-F1-P1 (Viewing slots) |  |
| RENT-A5-F5-P1 | **What landlords may ask, and when**: Research the privacy and tenancy limits on what landlords may ask renters at each stage. | research | small |  | [RT02](STATUS.md) |
| RENT-A5-F5-P2 | **Renter profile store**: Profile, references and documents owned by the renter, stored encrypted. | data | medium | kit-store, svc-accounts |  |
| RENT-A5-F5-P3 | **Profile builder**: Builds the profile step by step and explains why each item may be asked. | interface | medium | RENT-A5-F5-P2 (Renter profile store), RENT-A5-F5-P1 (What landlords may ask, and when), RENT-A5-F5-P5 (Limits built into the profile) |  |
| RENT-A5-F5-P4 | **Stage rules**: Only contact details before a viewing, and more only once a renter chooses to apply. | service | small | RENT-A5-F5-P1 (What landlords may ask, and when), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A5-F5-P5 | **Limits built into the profile**: Blocks detailed bank statements, allows only one proof of ability to pay besides a credit report, asks for ID copies only when a credit or criminal record check needs them, and never asks about Human Rights Act grounds, family violence history or social media. | library | medium | RENT-A5-F5-P1 (What landlords may ask, and when), RENT-A5-F5-P2 (Renter profile store), RENT-A5-F5-P4 (Stage rules) |  |
| RENT-A5-F6-P1 | **Per-listing consent share**: Shares a copy of the allowed parts of the profile with one landlord, for one listing, for a set time. | service | medium | RENT-A5-F5-P2 (Renter profile store), kit-ops |  |
| RENT-A5-F6-P2 | **Apply screen**: Shows exactly what will be shared before the renter sends it. | interface | small | RENT-A5-F6-P1 (Per-listing consent share) |  |
| RENT-A5-F6-P3 | **Who-looked log**: Renters see who opened their application and when. | interface | small | RENT-A5-F6-P1 (Per-listing consent share), RENT-A1-F5-P1 (Role and permission model) |  |
| RENT-A5-F6-P4 | **View-only shared documents**: Shared documents are view-only, watermarked to one landlord and one listing, expire automatically and log every access. | service | medium | RENT-A5-F6-P1 (Per-listing consent share), KIT-A15-F8-P1 (Sharing and access), engine-pdf |  |
| RENT-A5-F7-P1 | **Household application**: Links several renter profiles into one application, each sharing their own part. | data | small | RENT-A5-F5-P2 (Renter profile store), RENT-A5-F6-P1 (Per-listing consent share) |  |
| RENT-A5-F7-P2 | **Invite co-applicants**: Invite others to join the household application. | interface | small | svc-messaging |  |
| RENT-A5-F8-P1 | **Reference requests**: A request link that collects a reference in a standard form. | interface | small | svc-messaging |  |
| RENT-A5-F8-P2 | **Renter-held reference store**: The one store of references the renter holds and reuses: from past landlords and employers, built from tenancy records (RENT-A13-F7-P1) and from rent history (RENT-A8-F10-P1), each shared only with the referee's and renter's consent. | data | small | RENT-A5-F5-P2 (Renter profile store) |  |
| RENT-A5-F9-P1 | **Credit reporter hookup**: Runs a consented credit check with a licensed credit reporter, and a criminal record check through the kit's background check link where one is justified; the landlord pays and the renter is never charged. | integration | medium | RENT-A5-F6-P1 (Per-listing consent share), KIT-A2-F9-P2 (Background check link), RENT-A22-F2-P4 (Renters never pay), RENT-A5-F9-P2 (Credit and record check rules) |  |
| RENT-A5-F9-P2 | **Credit and record check rules**: Research the credit reporting code, criminal record checks and Tenancy Tribunal decision searches. | research | small |  | [RT02](STATUS.md) |
| RENT-A5-F9-P3 | **Collected-from-others notice**: Tells the renter when information about them is collected from someone else, as IPP 3A requires. | service | small | svc-messaging, KIT-A18-F5-P1 (Collection notices) |  |
| RENT-A5-F10-P1 | **Criteria set first**: The landlord sets selection criteria before opening applications. | data | small | RENT-A3-F1-P1 (Listing model) |  |
| RENT-A5-F10-P2 | **Applicant comparison board**: Compare applications against the criteria, with notes kept private. | interface | medium | RENT-A5-F6-P1 (Per-listing consent share) |  |
| RENT-A5-F10-P3 | **Hide names and photos**: An option to compare without names, photos or ages showing. | interface | small | RENT-A5-F6-P1 (Per-listing consent share) |  |
| RENT-A5-F10-P4 | **Discrimination grounds in housing**: Research the Human Rights Act grounds that apply to choosing tenants. | research | small |  | [RT02](STATUS.md) |
| RENT-A5-F10-P5 | **People decide, not software**: No automated scoring, ranking or rejection of applicants: a person decides against the criteria they set beforehand; an AI agent may summarise but never declines anyone, and each AI use is in the kit's AI use register. | library | small | RENT-A5-F10-P1 (Criteria set first), RENT-A19-F4-P2 (Actions only a person approves), KIT-A17-F10-P1 (AI use register), KIT-A17-F10-P2 (Human review of AI decisions) |  |
| RENT-A5-F11-P1 | **Offer with expiry**: An offer the applicant accepts or declines by a set time, on the marketplace engine. | service | small | kit-market, RENT-A5-F6-P1 (Per-listing consent share) |  |
| RENT-A5-F11-P2 | **Decline messages**: Tells unsuccessful applicants promptly and sets when their shared data is deleted. | service | small | svc-messaging, RENT-A16-F5-P1 (Retention rules), KIT-A18-F6-P1 (Retention schedules) |  |
| RENT-A5-F12-P1 | **Application retention schedule**: Deletes shared application copies a set time after the decision. | service | small | RENT-A16-F5-P1 (Retention rules), RENT-A5-F6-P1 (Per-listing consent share), KIT-A18-F6-P1 (Retention schedules) |  |
| RENT-A5-F12-P2 | **See and correct**: Renters see every copy a landlord holds and can ask for corrections. | interface | small | RENT-A5-F6-P1 (Per-listing consent share), KIT-A18-F3-P1 (Privacy page) |  |
| RENT-A5-F13-P1 | **Tenant consent to viewings**: Asks the current tenant to consent to viewings, which they can't refuse unreasonably but can attach reasonable conditions to, and records the answer and conditions. | service | small | RENT-A9-F4-P1 (Entry notice check), RENT-A5-F13-P4 (Rules for occupied homes), RENT-A16-F8-P1 (Guest links for tenancies) |  |
| RENT-A5-F13-P2 | **Viewing notice and hours**: Gives the notice the law requires and keeps viewings within allowed hours and the tenant's conditions. | service | small | RENT-A5-F13-P1 (Tenant consent to viewings), RENT-A5-F1-P1 (Viewing slots), RENT-A9-F1-P2 (Legal clock engine), RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A5-F13-P3 | **Consent for interior photos**: Asks the tenant before photos or video show their belongings, and holds those photos back until they agree or the photos are retaken. | service | small | RENT-A3-F2-P2 (Privacy prompts on photos), RENT-A5-F13-P1 (Tenant consent to viewings) |  |
| RENT-A5-F13-P4 | **Rules for occupied homes**: The rules for viewings and photos with tenants in place, kept once in the NZ rule pack and shared with Spare Home's tenancy rules (HOME-A7-F5-P1). | data | small | RENT-A21-F1-P3 (New Zealand rule pack) | [RT01](STATUS.md) |

## RENT-A6 Tenancy agreements and moving in

A tenancy agreement built from the official template with every required statement, signed online, then one move-in checklist for money, keys, utilities, pets and the home guide.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A6-F1 Build a tenancy agreement | Create a tenancy agreement from the official template, with every required statement filled in. | core | Agreement builder; Official template and allowed terms; Required statements; Extra terms checker; Fixed-term or periodic choice |
| RENT-A6-F2 Sign online | Sign the agreement on any device, with a copy kept for every party. | core | E-signing; Tenancy documents and signature law; Signed copies for everyone; Sign together on one device |
| RENT-A6-F3 Money due at move-in | See exactly what is due before moving in, with rent in advance capped by the law. | core | Move-in money schedule; How to pay |
| RENT-A6-F4 Move-in checklist | Follow one checklist for keys, the entry inspection, bond, utilities and smoke alarms. | core | Checklist engine; Checklist screen |
| RENT-A6-F5 Utilities and internet | Set up power, gas, water and internet at move-in, with meter readings recorded. | next | Meter readings; Utility switching links; Fibre status at move-in |
| RENT-A6-F6 Home guide | Give tenants a guide to the home: appliances, bins, heating and who to call. | next | Home guide editor; Home guide for tenants |
| RENT-A6-F7 Pets | Ask to keep a pet, get an answer within the legal time, and add a pet bond if one applies. | core | Pet request and answer; Pet bond amount; Assistance animals |
| RENT-A6-F8 Guarantors | Add a guarantor who agrees to cover rent if the tenant can't. | later | Guarantor sign-off; Guarantor rules |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A6-F1-P1 | **Agreement builder**: Fills the official template from the property, listing and applicants, and renders a PDF. | service | large | RENT-A2-F1-P2 (Rental fields on the shared property record), RENT-A21-F1-P1 (Rule pack format), engine-pdf, RENT-A18-F8-P1 (Which law applies) |  |
| RENT-A6-F1-P2 | **Official template and allowed terms**: Research the official template's licence, which terms may be added and which the law voids. | research | small |  | [RT01](STATUS.md) |
| RENT-A6-F1-P3 | **Required statements**: Adds the insurance, insulation and Healthy Homes statements the law requires. | service | small | RENT-A6-F1-P1 (Agreement builder), RENT-A2-F5-P1 (Insurance policy record), RENT-A12-F1-P2 (Standards model) |  |
| RENT-A6-F1-P4 | **Extra terms checker**: Flags added terms the law would void, such as late fees or a ban on minor changes. | service | medium | RENT-A6-F1-P1 (Agreement builder), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A6-F1-P5 | **Fixed-term or periodic choice**: Explains the difference and sets the right clocks for the choice made. | interface | small | RENT-A6-F1-P1 (Agreement builder), RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A6-F2-P1 | **E-signing**: The kit's signatures with audit trail, applied to Spare Rent's agreements, plans and notices. | service | small | KIT-A25-F1-P2 (Signatures with audit trail), RENT-A6-F1-P1 (Agreement builder), RENT-A6-F2-P2 (Tenancy documents and signature law) |  |
| RENT-A6-F2-P2 | **Tenancy documents and signature law**: Which tenancy documents may be signed electronically, checked against the kit's electronic signature rules. | research | small | KIT-A25-F1-P1 (Electronic signature rules) |  |
| RENT-A6-F2-P3 | **Signed copies for everyone**: Each party gets the signed agreement in their documents straight away. | service | small | RENT-A6-F2-P1 (E-signing), RENT-A2-F6-P1 (Shared document vault) |  |
| RENT-A6-F2-P4 | **Sign together on one device**: Both parties sign on one device at the move-in inspection with no signal, and the signatures sync later with their times kept. | interface | small | RENT-A6-F2-P1 (E-signing), KIT-A15-F4-P2 (Sync engine), RENT-A10-F4-P1 (Offline capture) |  |
| RENT-A6-F3-P1 | **Move-in money schedule**: Bond, pet bond and rent in advance, checked against the legal caps. | service | small | RENT-A6-F1-P1 (Agreement builder), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A6-F3-P2 | **How to pay**: Shows the landlord's bank details and the bond payment route, never the platform's account. | interface | small | RENT-A7-F1-P2 (Tenancy Bond API connection) |  |
| RENT-A6-F4-P1 | **Checklist engine**: Builds the move-in list from the rule pack and the tenancy, and ticks items as they happen. | service | small | RENT-A21-F1-P1 (Rule pack format), kit-ops |  |
| RENT-A6-F4-P2 | **Checklist screen**: The same list for landlord and tenant, showing who does what. | interface | small | RENT-A6-F4-P1 (Checklist engine) |  |
| RENT-A6-F5-P1 | **Meter readings**: Dated meter readings with photos at move-in and move-out. | interface | small | engine-media |  |
| RENT-A6-F5-P2 | **Utility switching links**: Research utility comparison and connection services whose terms allow linking. | research | small |  |  |
| RENT-A6-F5-P3 | **Fibre status at move-in**: Shows whether fibre is connected at move-in and links to the fibre request (RENT-A9-F6-P1). | data | small | RENT-A9-F6-P1 (Minor change and fibre requests) |  |
| RENT-A6-F6-P1 | **Home guide editor**: A simple guide built from the property record, chattels and contacts. | interface | small | RENT-A2-F2-P1 (Room, chattel and appliance list) |  |
| RENT-A6-F6-P2 | **Home guide for tenants**: The guide in the tenant's app, readable offline. | interface | small | kit-sync |  |
| RENT-A6-F7-P1 | **Pet request and answer**: A request with details of the pet, answered within the legal time on reasonable grounds. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A6-F7-P2 | **Pet bond amount**: Works out the pet bond cap and any conditions, then hands the lodgement to the pet bond part (RENT-A7-F3-P1). | service | small | RENT-A21-F1-P1 (Rule pack format), RENT-A7-F3-P1 (Pet bond lodgement) |  |
| RENT-A6-F7-P3 | **Assistance animals**: Assistance dogs are not pets: no request and no pet bond. | data | small | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A6-F8-P1 | **Guarantor sign-off**: A guarantor signs their own part of the agreement. | interface | small | RENT-A6-F2-P1 (E-signing) |  |
| RENT-A6-F8-P2 | **Guarantor rules**: Research what tenancy law allows from a guarantor. | research | small |  | [RT01](STATUS.md) |

## RENT-A7 Bonds

Spare Rent never holds a bond. Since 29 June 2026 bond transactions are online only: Spare Rent does what the Tenancy Bond API supports with each landlord's own RealMe consent, hands the rest to Bond Hub, tells tenants that Tenancy Services will contact them directly about refunds, and keeps the legal clocks so none is late.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A7-F1 Lodge a bond | Lodge the bond with Tenancy Services from inside the app, within the legal time. | core | Bond API access, terms and coverage; Tenancy Bond API connection; Lodgement form; Lodgement clock; Bond Hub hand-off; Landlord consent to the bond system; MBIE API Access Agreement; Supported operations table; What tenants will hear from Tenancy Services |
| RENT-A7-F2 Bonds paid by Work and Income | Handle bonds paid partly or fully by Work and Income. | core | Split-payer bond record; Government-paid bonds |
| RENT-A7-F3 Pet bonds | Lodge a pet bond of up to two weeks' rent when a pet is agreed, and get it back when the pet leaves. | core | Pet bond lodgement; Pet bond refund |
| RENT-A7-F4 Top up a bond | Top up the bond when the rent rises, where the agreement allows it. | next | Top-up calculator and lodgement |
| RENT-A7-F5 Change who is on a bond | Record a tenant leaving or joining, or a new landlord, on the bond. | core | Change of tenant or landlord; Signatures for the change |
| RENT-A7-F6 Refund a bond | Propose how the bond is split at the end; Tenancy Services then asks each tenant directly to agree or dispute it. | core | Refund split proposal; Refund request; When the split isn't agreed; Deduction check |
| RENT-A7-F7 Bond records | See every bond, its number and its status in one place. | core | Bond status sync; Bond list |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A7-F1-P1 | **Bond API access, terms and coverage**: Research registering with MBIE, its API Access Agreement, the sandbox, each landlord's RealMe consent, and which bond operations the API supports today. | research | small |  |  |
| RENT-A7-F1-P2 | **Tenancy Bond API connection**: Calls the Tenancy Bond API with the co-op's subscription key and each landlord's own consent token, for the operations the API supports. | integration | large | RENT-A7-F1-P1 (Bond API access, terms and coverage), RENT-A7-F1-P6 (Landlord consent to the bond system), RENT-A7-F1-P7 (MBIE API Access Agreement), RENT-A7-F1-P8 (Supported operations table), kit-ops |  |
| RENT-A7-F1-P3 | **Lodgement form**: Fills the lodgement from the agreement for both sides to check. | interface | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A6-F1-P1 (Agreement builder) |  |
| RENT-A7-F1-P4 | **Lodgement clock**: Counts the working days left to lodge and warns before the deadline. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A7-F1-P2 (Tenancy Bond API connection) |  |
| RENT-A7-F1-P5 | **Bond Hub hand-off**: Sends the landlord to Bond Hub with the details ready to copy for any operation the API doesn't support, or when it is down; there are no paper bond forms since 29 June 2026. | interface | small | RENT-A7-F1-P8 (Supported operations table) |  |
| RENT-A7-F1-P6 | **Landlord consent to the bond system**: Each landlord logs in with the RealMe account they use with Tenancy Services so Spare Rent can reach their bonds; the consent token is kept per landlord, encrypted, renewed and can be withdrawn. | service | medium | RENT-A7-F1-P1 (Bond API access, terms and coverage), KIT-A22-F5-P1 (Secrets and access), KIT-A22-F5-P5 (Encryption and keys) |  |
| RENT-A7-F1-P7 | **MBIE API Access Agreement**: The co-op signs MBIE's API Access Agreement before its subscription is approved, and keeps its subscription keys secret. | legal | small | RENT-A7-F1-P1 (Bond API access, terms and coverage), KIT-A14-F1-P1 (Legal entity) |  |
| RENT-A7-F1-P8 | **Supported operations table**: Which bond operations, such as lodging, top-ups, refunds, change of tenant and change of landlord, go through the API and which through Bond Hub, kept current and shown before each step. | data | small | RENT-A7-F1-P1 (Bond API access, terms and coverage) |  |
| RENT-A7-F1-P9 | **What tenants will hear from Tenancy Services**: Tells tenants to expect official emails from bonds@tenancy.govt.nz and texts from 2910 about their bond and refund, to keep their contact details current with Tenancy Services, and how to tell those messages from scams. | service | small | svc-messaging, RENT-A17-F2-P2 (Scam guide) |  |
| RENT-A7-F2-P1 | **Split-payer bond record**: Records who paid which share of a bond, so refunds go to the right payer. | data | small | RENT-A7-F1-P2 (Tenancy Bond API connection) |  |
| RENT-A7-F2-P2 | **Government-paid bonds**: Research how bonds paid by Work and Income are lodged, transferred and refunded. | research | small |  |  |
| RENT-A7-F3-P1 | **Pet bond lodgement**: The one part that lodges or tops up a pet bond, through the API or the Bond Hub hand-off. | integration | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A7-F1-P5 (Bond Hub hand-off) |  |
| RENT-A7-F3-P2 | **Pet bond refund**: Starts the pet bond refund when the pet is no longer at the home. | integration | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A7-F1-P5 (Bond Hub hand-off) |  |
| RENT-A7-F4-P1 | **Top-up calculator and lodgement**: Works out the allowed top-up and lodges it. | integration | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A21-F1-P1 (Rule pack format), RENT-A7-F1-P5 (Bond Hub hand-off) |  |
| RENT-A7-F5-P1 | **Change of tenant or landlord**: The one part for changes of tenant or landlord on a bond: through the API where it supports them, otherwise through Bond Hub. | integration | medium | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A7-F1-P5 (Bond Hub hand-off) |  |
| RENT-A7-F5-P2 | **Signatures for the change**: Leaving and joining tenants sign their part of the change. | interface | small | RENT-A6-F2-P1 (E-signing) |  |
| RENT-A7-F6-P1 | **Refund split proposal**: The landlord proposes deductions with evidence, checked against the liability rules; the tenant sees the proposal here but agrees or disputes it directly with Tenancy Services. | interface | medium | RENT-A10-F1-P1 (Condition report model), RENT-A8-F1-P1 (Rent ledger), RENT-A7-F6-P4 (Deduction check) |  |
| RENT-A7-F6-P2 | **Refund request**: Submits the landlord's refund request through the API or Bond Hub; Tenancy Services then contacts each tenant by email or text to agree or dispute it. | integration | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A7-F1-P5 (Bond Hub hand-off), RENT-A7-F6-P1 (Refund split proposal) |  |
| RENT-A7-F6-P3 | **When the split isn't agreed**: Explains the next steps and builds the evidence pack for the Tribunal. | service | small | RENT-A13-F4-P1 (Evidence pack builder) |  |
| RENT-A7-F6-P4 | **Deduction check**: Checks proposed deductions against the damage-liability rules: for careless damage when the landlord is insured, the lower of the insurance excess or four weeks' rent (to confirm), the pet damage rules since December 2025, and fair wear and tear. | service | medium | RENT-A2-F5-P1 (Insurance policy record), RENT-A10-F3-P2 (Fair wear and tear and liability guide), RENT-A21-F1-P3 (New Zealand rule pack) |  |
| RENT-A7-F7-P1 | **Bond status sync**: Reads each bond's status on a schedule through the landlord's consent token. | integration | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A7-F1-P6 (Landlord consent to the bond system) |  |
| RENT-A7-F7-P2 | **Bond list**: Bonds per property and tenancy, with their numbers and amounts. | interface | small | RENT-A7-F1-P2 (Tenancy Bond API connection) |  |

## RENT-A8 Rent and money

Rent goes bank to bank by default and is matched automatically; both sides see what is due and paid, get receipts and early warnings, agree payment plans, and handle rent rises and allowed charges within the law. Rent and bonds are not marketplace jobs: they skip the loop's pay and hold, auto-confirm and member panels.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A8-F1 Rent schedule | Set the rent, how often it is paid and when, and see what is due and paid. | core | Rent ledger; Rent schedule over time; Rent screen; No hold on rent |
| RENT-A8-F2 Bank-to-bank rent tracking | Have rent paid straight to the landlord's bank and matched to the tenancy automatically. | core | Bank feed import; Payment matching; Read-only bank feeds; Tenant's payment set-up; Several payers |
| RENT-A8-F3 Pay rent through the app | Choose to pay rent by direct debit or card through a regulated payment provider, at cost. | next | Rent collection via the payments service; Client money rules; Opt in and out; Failed and part payments; Anti-money-laundering duties for rent |
| RENT-A8-F4 Receipts and statements | Get a receipt for every payment and a rent statement whenever you ask. | core | Receipts and statements |
| RENT-A8-F5 Reminders and arrears | Get a friendly reminder before rent is due, and early warning when it is late. | core | Arrears detection; Reminder schedule; Notice to remedy unpaid rent; Late-rent strikes |
| RENT-A8-F6 Payment plans | Agree a plan to catch up on missed rent, and track it. | core | Payment plan tracking; Signed plan |
| RENT-A8-F7 Rent rises | Propose a rent rise and be told if it breaks the once-a-year rule or the notice period. | core | Rent rise check; Rent rise notice; Market comparison |
| RENT-A8-F8 Other charges | Recharge water use and other allowed costs, with the bill attached. | next | Allowed-charges rules; Charge with evidence |
| RENT-A8-F9 Rent reductions | Record a rent reduction when part of the home can't be used. | next | Rent adjustment record |
| RENT-A8-F10 Rent history you own | Keep your own record of paying rent on time to use in future applications. | later | Portable rent history; Rent history and credit records |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A8-F1-P1 | **Rent ledger**: Every rent charge and payment for a tenancy, as operations that can't be silently changed. | data | medium | kit-ops, kit-core |  |
| RENT-A8-F1-P2 | **Rent schedule over time**: Rent amounts and frequencies with the dates each applied. | service | small | RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F1-P3 | **Rent screen**: The same view of rent due, paid and owing for landlord and tenant. | interface | medium | RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F1-P4 | **No hold on rent**: Tenancies skip the marketplace loop's pay-and-hold, auto-confirm and member-panel steps; only paid manager and helper jobs (RENT-A15-F2) use them. | data | small | kit-market, RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F2-P1 | **Bank feed import**: Imports bank transactions by file, or by read-only bank feeds proposed as one kit service that Spare Stay hosts, Spare Work trades and Spare Market sellers also need for their books. | integration | medium | kit-store, RENT-A8-F2-P3 (Read-only bank feeds) |  |
| RENT-A8-F2-P2 | **Payment matching**: Matches incoming payments to tenancies by reference, amount and payer, including rent paid straight from Work and Income and rent split between several payers. | service | medium | RENT-A8-F2-P1 (Bank feed import), RENT-A8-F1-P1 (Rent ledger), RENT-A8-F2-P5 (Several payers) |  |
| RENT-A8-F2-P3 | **Read-only bank feeds**: Research read-only bank feed providers, terms and cost, and accreditation under the Customer and Product Data Act where it applies, done once for the kit rather than per app. | research | small |  |  |
| RENT-A8-F2-P4 | **Tenant's payment set-up**: Shows tenants the account, reference and amount to set up an automatic payment. | interface | small | RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F2-P5 | **Several payers**: Records who pays which share of the rent, such as each flatmate or Work and Income, so matching and arrears count the whole rent. | data | small | RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F3-P1 | **Rent collection via the payments service**: Collects rent through the regulated provider and pays it straight to the landlord. | integration | medium | svc-payments, RENT-A8-F1-P1 (Rent ledger), KIT-A3-F1-P2 (Payment provider link), KIT-A3-F11-P1 (Bank payments), KIT-A3-F12-P1 (Regular payments) | [K02](../kit/STATUS.md) |
| RENT-A8-F3-P2 | **Client money rules**: Research whether rent passing through a provider makes anyone a holder of client money. | research | small | svc-payments | [K02](../kit/STATUS.md) |
| RENT-A8-F3-P3 | **Opt in and out**: Both sides agree before rent runs through the app, and either can go back to bank to bank. | interface | small | svc-payments |  |
| RENT-A8-F3-P4 | **Failed and part payments**: A failed or part payment through the provider shows as owing, tells both sides, and never adds a charge for the tenant. | service | small | RENT-A8-F3-P1 (Rent collection via the payments service), RENT-A8-F5-P1 (Arrears detection) |  |
| RENT-A8-F3-P5 | **Anti-money-laundering duties for rent**: Whether rent moving through a provider brings Spare Rent under the anti-money-laundering duties for online marketplaces, and how any yearly threshold applies (to confirm). | legal | small | KIT-A4-F5-P1 (Anti-money-laundering programme) | [K02](../kit/STATUS.md) |
| RENT-A8-F4-P1 | **Receipts and statements**: Generates receipts and rent statements as PDFs from the ledger. | service | small | RENT-A8-F1-P1 (Rent ledger), engine-pdf, KIT-A19-F11-P1 (Accessible PDFs) |  |
| RENT-A8-F5-P1 | **Arrears detection**: Spots missed or short payments and counts how long rent has been owing. | service | small | RENT-A8-F1-P1 (Rent ledger), RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A8-F5-P2 | **Reminder schedule**: Reminders both sides can tune, never more often than agreed. | service | small | svc-messaging |  |
| RENT-A8-F5-P3 | **Notice to remedy unpaid rent**: Prepares the formal notice when rent is owing, with the right dates. | service | small | RENT-A8-F5-P1 (Arrears detection), RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A8-F5-P4 | **Late-rent strikes**: Counts each time rent is 5 or more working days late within 90 days and how long rent is in arrears, so the landlord knows when the Tribunal grounds are met (3 occasions in 90 days, or 21 days in arrears), with the required notice for each occasion. | service | small | RENT-A8-F5-P1 (Arrears detection), RENT-A9-F1-P2 (Legal clock engine), RENT-A9-F2-P1 (Notice templates) |  |
| RENT-A8-F6-P1 | **Payment plan tracking**: A plan of catch-up payments matched against the ledger. | service | small | RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F6-P2 | **Signed plan**: Both sides sign the plan, each approving it in person, so it can be shown later or turned into a FastTrack Resolution order (RENT-A13-F3-P3). | interface | small | RENT-A6-F2-P1 (E-signing), RENT-A19-F4-P2 (Actions only a person approves) |  |
| RENT-A8-F7-P1 | **Rent rise check**: Checks the date of the last rise, the notice period and any fixed-term terms. | service | small | RENT-A21-F1-P1 (Rule pack format), RENT-A9-F1-P2 (Legal clock engine), RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F7-P2 | **Rent rise notice**: Prepares and serves the written notice with the date the new rent starts. | service | small | RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A8-F7-P3 | **Market comparison**: Shows how the proposed rent compares with similar homes nearby. | interface | small | RENT-A3-F4-P1 (Market rent data) |  |
| RENT-A8-F8-P1 | **Allowed-charges rules**: Which charges a landlord may pass on, such as metered water, and which are banned, such as late fees. | service | small | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A8-F8-P2 | **Charge with evidence**: Adds a charge to the ledger with the bill attached for the tenant to see. | interface | small | RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F9-P1 | **Rent adjustment record**: A dated adjustment with its reason, agreed by both sides or ordered by the Tribunal. | data | small | RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F10-P1 | **Portable rent history**: A tenant-held summary of on-time payments, kept in the renter-held reference store and shared like a reference. | data | small | RENT-A5-F8-P2 (Renter-held reference store), RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A8-F10-P2 | **Rent history and credit records**: Research whether rent history can be reported to credit reporters and what consent that needs. | research | small |  |  |

## RENT-A9 Notices, legal clocks and tenancy changes

Every legal deadline in a tenancy is a clock both sides can see; every formal notice comes from a template with the right dates and proof of service; and changes during a tenancy, from new flatmates to hard situations, follow the law step by step.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A9-F1 Legal clocks | See every legal deadline in a tenancy with a countdown, and get warned before it passes. | core | How deadlines are tracked and shown; Legal clock engine; Tenancy working days; Law by date; Clocks screen |
| RENT-A9-F2 Send a notice | Send any formal notice from a template that fills itself in and gives the right dates. | core | Notice templates; Notice builder; Proof of service; Anti-social behaviour strikes; Service, not just delivery; Retaliation warning |
| RENT-A9-F3 Receive a notice | See notices you receive explained in plain words, with what you can do and by when. | core | Plain-language explainers; Notice inbox; Challenge a retaliatory notice |
| RENT-A9-F4 Notice to enter | Give notice to enter for an inspection or repairs, with the legal hours and limits checked. | core | Entry notice check; Tenant reply |
| RENT-A9-F5 Change who lives there | Add, remove or replace a tenant, including assignment and subletting requests. | core | Assignment and subletting requests; Change of tenant |
| RENT-A9-F6 Ask for changes to the home | Ask for a minor change or fibre, and get an answer within the legal time. | core | Minor change and fibre requests; Request screen |
| RENT-A9-F7 Fixed terms ending | Choose to renew, go periodic or end a fixed term, with the notice window shown. | core | Fixed-term end window; Renewal |
| RENT-A9-F8 Hard situations | Handle family violence withdrawal, a tenant's death, a mortgagee sale or a home destroyed by a disaster, with the right steps and privacy. | core | Family violence withdrawal; Death of a tenant and mortgagee sale; Steps and evidence for hard cases; Home destroyed or uninhabitable; One event, many tenancies |
| RENT-A9-F9 Law changes | Get told when tenancy law changes and what it means for your tenancies, and be told and put right if a rule was ever wrong. | core | Rule updates with change notes; Affected-tenancy alerts; Rule error incidents; Cover for rule errors |
| RENT-A9-F10 Change or end the agreement by agreement | Change a term partway through, or end a fixed term early, when both sides agree. | next | Variation by agreement; Ending a fixed term early by agreement |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A9-F1-P1 | **How deadlines are tracked and shown**: Design note on tracking and showing each legal deadline so none is missed. | research | medium |  | [RT03](STATUS.md) |
| RENT-A9-F1-P2 | **Legal clock engine**: Tenancy clocks on the kit's working days and notice periods, adding tenancy rules such as extra days for how a notice was served; proposed, with proof of service, as one kit formal-notices service shared with Spare Home. | library | medium | KIT-A19-F9-P2 (Working days and notice periods), kit-ops, RENT-A21-F1-P1 (Rule pack format), RENT-A9-F1-P1 (How deadlines are tracked and shown), RENT-A9-F1-P3 (Tenancy working days) |  |
| RENT-A9-F1-P3 | **Tenancy working days**: Tenancy law's own working-day rule, which also skips 25 December to 15 January (to confirm under RT01), layered on the kit's NZ holiday data. | data | small | KIT-A19-F9-P1 (NZ country pack), KIT-A19-F9-P2 (Working days and notice periods) |  |
| RENT-A9-F1-P4 | **Law by date**: Applies the rules in force when a tenancy was signed or a notice was given. | library | medium | RENT-A21-F1-P1 (Rule pack format), RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A9-F1-P5 | **Clocks screen**: A timeline of every running clock for a tenancy, the same for both sides. | interface | small | RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A9-F2-P1 | **Notice templates**: Every notice type in tenancy law as a template with its required content. | data | medium | RENT-A21-F1-P1 (Rule pack format) | [RT01](STATUS.md) |
| RENT-A9-F2-P2 | **Notice builder**: Fills a notice from the tenancy, works out the dates and renders a PDF. | service | medium | RENT-A9-F1-P2 (Legal clock engine), engine-pdf |  |
| RENT-A9-F2-P3 | **Proof of service**: Records how a notice was served, by the agreed email for service, post or hand, and when it counts as received; proposed for the kit's formal-notices service with Spare Home. | service | medium | RENT-A9-F2-P2 (Notice builder), RENT-A9-F1-P2 (Legal clock engine), RENT-A16-F4-P1 (Address for service record), svc-messaging |  |
| RENT-A9-F2-P4 | **Anti-social behaviour strikes**: Notices of anti-social behaviour with their required content, a count of notices within 90 days, and a clock for the 28 days the landlord has to apply to the Tribunal after the third. | service | small | RENT-A9-F2-P1 (Notice templates), RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A9-F2-P5 | **Service, not just delivery**: Delivery in the app is not legal service unless the law or the agreement allows it: every formal notice also goes by an allowed method, the agreed email for service, post or hand, and the app says which was used. | library | small | RENT-A9-F2-P3 (Proof of service), RENT-A16-F4-P1 (Address for service record) |  |
| RENT-A9-F2-P6 | **Retaliation warning**: Warns a landlord about to give notice soon after the tenant complained or asked for something, such as a repair or a Healthy Homes question, that the Tribunal can declare a retaliatory notice of no effect and award exemplary damages. | service | small | RENT-A9-F2-P2 (Notice builder), RENT-A11-F1-P1 (Repair job model), RENT-A12-F7-P1 (Tenant compliance view) |  |
| RENT-A9-F3-P1 | **Plain-language explainers**: A short explanation for each notice type and the receiver's options. | data | small | RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A9-F3-P2 | **Notice inbox**: Formal notices kept apart from chat, with read receipts; the one place notices are shown, and a read receipt is never treated as proof of service. | interface | small | RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A9-F3-P3 | **Challenge a retaliatory notice**: When a termination notice arrives, shows the tenant they can apply to the Tribunal within 12 months if they think it is retaliatory, to have it set aside and seek exemplary damages, and that applying within 28 working days also lets them ask for the notice to be cancelled, with both clocks running. | interface | small | RENT-A9-F3-P1 (Plain-language explainers), RENT-A9-F1-P2 (Legal clock engine), RENT-A13-F4-P3 (Guided application) |  |
| RENT-A9-F4-P1 | **Entry notice check**: Checks notice time, allowed hours and how often inspections can happen. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A9-F4-P2 | **Tenant reply**: The tenant can confirm or suggest another time. | interface | small | svc-messaging |  |
| RENT-A9-F5-P1 | **Assignment and subletting requests**: A request flow with the legal reply time and reasons for any refusal. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A9-F5-P2 | **Change of tenant**: Updates the agreement and hands the bond change to the one bond change part (RENT-A7-F5-P1). | service | small | RENT-A6-F1-P1 (Agreement builder), RENT-A7-F5-P1 (Change of tenant or landlord) |  |
| RENT-A9-F6-P1 | **Minor change and fibre requests**: Requests with the legal reply time and allowed reasons for saying no. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A9-F6-P2 | **Request screen**: Describe the change with photos and see the answer and any conditions. | interface | small | engine-media |  |
| RENT-A9-F7-P1 | **Fixed-term end window**: Shows when notice can be given before the term ends and what happens if none is. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A9-F7-P2 | **Renewal**: A new or varied agreement, with fresh required statements. | service | small | RENT-A6-F1-P1 (Agreement builder) |  |
| RENT-A9-F8-P1 | **Family violence withdrawal**: A private flow for a tenant to leave with the short legal notice, with details kept from others. | service | medium | RENT-A9-F1-P2 (Legal clock engine), RENT-A1-F5-P1 (Role and permission model) |  |
| RENT-A9-F8-P2 | **Death of a tenant and mortgagee sale**: Step-by-step guides with the clocks each one starts. | service | small | RENT-A21-F1-P1 (Rule pack format), RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A9-F8-P3 | **Steps and evidence for hard cases**: Research what each hard situation needs in evidence and notices. | research | small |  | [RT01](STATUS.md) |
| RENT-A9-F8-P4 | **Home destroyed or uninhabitable**: Rent abates; the landlord may end the tenancy with 7 days' notice and the tenant with 2; if only part of the home is damaged, the Tribunal decides. | service | medium | RENT-A9-F1-P2 (Legal clock engine), RENT-A9-F2-P2 (Notice builder), RENT-A21-F1-P3 (New Zealand rule pack), RENT-A8-F9-P1 (Rent adjustment record) |  |
| RENT-A9-F8-P5 | **One event, many tenancies**: Handles a flood or earthquake that hits many tenancies at once, in bulk, for landlords and managers. | service | small | RENT-A9-F8-P4 (Home destroyed or uninhabitable), RENT-A15-F3-P2 (Bulk actions) |  |
| RENT-A9-F9-P1 | **Rule updates with change notes**: Each rule pack update comes with a plain note of what changed and from when. | service | small | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A9-F9-P2 | **Affected-tenancy alerts**: Tells landlords and tenants whose tenancies a change affects. | service | small | RENT-A21-F1-P1 (Rule pack format), svc-messaging |  |
| RENT-A9-F9-P3 | **Rule error incidents**: When a rule or clock is found wrong: find every affected tenancy, tell both sides, correct the record and log the incident publicly. | service | medium | RENT-A21-F1-P4 (Rule pack tests), RENT-A9-F9-P2 (Affected-tenancy alerts), KIT-A22-F1-P2 (Incident response), KIT-A13-F9-P1 (Public reporting pipeline) |  |
| RENT-A9-F9-P4 | **Cover for rule errors**: The co-op carries liability cover for losses caused by a wrong rule or clock. | legal | small | KIT-A14-F17-P1 (Co-op insurance) |  |
| RENT-A9-F10-P1 | **Variation by agreement**: Change a term partway through, such as the rent day, parking or an added term, with both sides signing and the rule pack checking the change. | service | small | RENT-A6-F1-P4 (Extra terms checker), RENT-A6-F2-P1 (E-signing), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A9-F10-P2 | **Ending a fixed term early by agreement**: Both sides agree to end a fixed term early; the landlord may recover only reasonable costs, which the app checks. | service | small | RENT-A13-F1-P1 (Ending flow), RENT-A21-F1-P3 (New Zealand rule pack), RENT-A8-F8-P1 (Allowed-charges rules) |  |

## RENT-A10 Inspections and condition reports

Move-in, routine and move-out inspections captured room by room with dated photos, done offline if needed, signed by both sides, and compared at the end so the bond is settled on evidence.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A10-F1 Entry inspection | Record the home's condition at move-in, room by room, with dated photos both sides sign. | core | Condition report model; Room-by-room capture; Tenant's own additions; Signed report; Evidence photo pipeline; Privacy in inspection photos |
| RENT-A10-F2 Routine inspections | Plan and run routine inspections within the legal limits, and share the report. | core | Inspection planner; Checklist templates; Report sharing and follow-ups |
| RENT-A10-F3 Exit inspection and comparison | Compare the home at move-out with the entry report, photo by photo. | core | Side-by-side comparison; Fair wear and tear and liability guide |
| RENT-A10-F4 Inspect offline | Do a whole inspection with no signal, and sync when back online. | core | Offline capture |
| RENT-A10-F5 Smoke alarms and safety checks | Record smoke alarm checks and other safety checks at each inspection. | core | Safety check items; Safety duties |
| RENT-A10-F6 Book an inspector | Book a co-op member to inspect for a landlord who lives away, at cost. | next | Inspection job |
| RENT-A10-F7 Contamination test records | Store methamphetamine and other contamination test results with the property. | later | Test result records; Contamination rules |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A10-F1-P1 | **Condition report model**: Rooms, items, condition, notes and dated photos, each change an operation. | data | medium | kit-ops, RENT-A2-F2-P1 (Room, chattel and appliance list) |  |
| RENT-A10-F1-P2 | **Room-by-room capture**: Walks through each room and item with the camera and quick condition choices. | interface | medium | RENT-A10-F1-P1 (Condition report model), engine-media |  |
| RENT-A10-F1-P3 | **Tenant's own additions**: The tenant adds photos and comments before signing. | interface | small | RENT-A10-F1-P1 (Condition report model) |  |
| RENT-A10-F1-P4 | **Signed report**: Both sides sign and get the report as a PDF. | service | small | RENT-A10-F1-P1 (Condition report model), RENT-A6-F2-P1 (E-signing), engine-pdf |  |
| RENT-A10-F1-P5 | **Evidence photo pipeline**: Inspection photos keep their capture time and place, are hashed when taken, the server records when it received them, and a wrong device clock is flagged; kept apart from the listing pipeline that strips location. | library | medium | RENT-A10-F1-P2 (Room-by-room capture), KIT-A15-F7-P1 (File uploads), engine-media, kit-sync |  |
| RENT-A10-F1-P6 | **Privacy in inspection photos**: Prompts the inspector to avoid photographing tenants' belongings, and shares reports only with the parties to the tenancy. | service | small | RENT-A3-F2-P2 (Privacy prompts on photos), RENT-A10-F1-P1 (Condition report model) |  |
| RENT-A10-F2-P1 | **Inspection planner**: Schedules inspections within the legal frequency and sends the notice to enter. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A10-F2-P2 | **Checklist templates**: Inspection checklists for each kind of home, which landlords can adjust. | data | small | RENT-A10-F1-P1 (Condition report model) |  |
| RENT-A10-F2-P3 | **Report sharing and follow-ups**: Shares the report with the tenant and turns issues into repair jobs. | service | small | RENT-A10-F1-P1 (Condition report model), RENT-A11-F1-P1 (Repair job model), RENT-A10-F1-P6 (Privacy in inspection photos) |  |
| RENT-A10-F3-P1 | **Side-by-side comparison**: Entry and exit photos and notes side by side for every item. | interface | medium | RENT-A10-F1-P1 (Condition report model) |  |
| RENT-A10-F3-P2 | **Fair wear and tear and liability guide**: Plain guidance on fair wear and tear and on what tenants are liable for, including the careless damage cap and pet damage. | data | small | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A10-F4-P1 | **Offline capture**: Stores the report and photos on the device and uploads them later, keeping the times taken through the evidence photo pipeline. | service | medium | kit-sync, RENT-A10-F1-P1 (Condition report model), RENT-A10-F1-P5 (Evidence photo pipeline) |  |
| RENT-A10-F5-P1 | **Safety check items**: Smoke alarm tests, battery dates and other safety items with dates. | data | small | RENT-A10-F1-P1 (Condition report model) |  |
| RENT-A10-F5-P2 | **Safety duties**: Research the smoke alarm and other safety duties of landlords and tenants. | research | small |  | [RT01](STATUS.md) |
| RENT-A10-F6-P1 | **Inspection job**: An inspection posted as a job on the marketplace engine and matched to a nearby helper. | service | small | kit-market, svc-matching, RENT-A15-F1-P1 (Manager listings), RENT-A15-F9-P1 (Worker status) |  |
| RENT-A10-F7-P1 | **Test result records**: Test results, the lab and the standard used, stored with the property. | data | small | RENT-A2-F6-P1 (Shared document vault) |  |
| RENT-A10-F7-P2 | **Contamination rules**: Research the current contamination rules and testing standards. | research | small |  | [RT01](STATUS.md) |

## RENT-A11 Maintenance and repairs

Tenants report problems with photos, urgent ones are escalated, landlords approve and book a trade on Spare Work or their own, access is arranged with the right notice, and every repair stays with the property's history.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A11-F1 Report a problem | Report a repair with photos or video, and follow its progress. | core | Repair job model; Report screen; Progress for both sides |
| RENT-A11-F2 Urgent repairs | Mark a repair as urgent and get clear steps if the landlord can't be reached. | core | Urgency triage and escalation; Tenant's urgent repair rights |
| RENT-A11-F3 Approve and book a trade | Approve a job and send it to a trade on Spare Work or to your own trade. | core | Send to Spare Work; Own trades list; Quotes and approval |
| RENT-A11-F4 Access for repairs | Agree a time for a trade to visit, with the right notice given. | core | Access time and notice; Hazard notes for visits |
| RENT-A11-F5 Job done | Confirm the job is finished, see the invoice and record the cost. | core | Completion sign-off; Invoice to expenses |
| RENT-A11-F6 Appliances and warranties | Keep a list of appliances with manuals, warranties and service dates. | next | Warranty and manual fields; Warranty and service reminders |
| RENT-A11-F7 Planned maintenance | Plan regular jobs such as gutters, heat pump servicing and smoke alarm batteries. | next | Recurring job planner |
| RENT-A11-F8 Insurance claims | Start an insurance claim with the photos and reports already gathered. | later | Claim pack |
| RENT-A11-F9 Repair history | Keep every repair with the property, so the next owner or manager sees it. | core | History on the property |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A11-F1-P1 | **Repair job model**: A repair request with its photos, urgency, status and history. | data | small | kit-ops, RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A11-F1-P2 | **Report screen**: Describe the problem, add photos or video and say when you're home. | interface | small | RENT-A11-F1-P1 (Repair job model), engine-media |  |
| RENT-A11-F1-P3 | **Progress for both sides**: Both sides see each step: reported, approved, booked, done. | interface | small | RENT-A11-F1-P1 (Repair job model) |  |
| RENT-A11-F2-P1 | **Urgency triage and escalation**: Urgent jobs alert the landlord and any backup contact until someone responds. | service | small | RENT-A11-F1-P1 (Repair job model), svc-messaging |  |
| RENT-A11-F2-P2 | **Tenant's urgent repair rights**: Research what a tenant may do and claim back when urgent repairs can't wait. | research | small |  | [RT01](STATUS.md) |
| RENT-A11-F3-P1 | **Send to Spare Work**: Posts the job to Spare Work through Spare Work's public command API (WORK-A19-F3-P1), never a code link, and follows its status; a person approves the spending first. | integration | medium | RENT-A11-F1-P1 (Repair job model), KIT-A16-F1-P1 (Command API), RENT-A19-F4-P2 (Actions only a person approves) |  |
| RENT-A11-F3-P2 | **Own trades list**: The landlord's own trusted trades, contacted from the job. | data | small | RENT-A11-F1-P1 (Repair job model) |  |
| RENT-A11-F3-P3 | **Quotes and approval**: Compare quotes and approve one, within any owner approval limit. | interface | small | RENT-A11-F3-P1 (Send to Spare Work), RENT-A19-F4-P2 (Actions only a person approves) |  |
| RENT-A11-F4-P1 | **Access time and notice**: Offers times that suit the tenant and sends the repair entry notice. | service | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A11-F4-P2 | **Hazard notes for visits**: Hazards to know before entering, such as a dog or a known risk in the home, from the kit's hazard notes, shown to trades and inspectors before they accept. | service | small | KIT-A24-F5-P1 (Hazard notes), RENT-A11-F4-P1 (Access time and notice) |  |
| RENT-A11-F5-P1 | **Completion sign-off**: For jobs sent to Spare Work, shows Spare Work's own sign-off by both parties (WORK-A10-F6-P3, WORK-A19-F3-P2); for the landlord's own trades, the tenant and landlord each confirm with photos. | interface | small | RENT-A11-F1-P1 (Repair job model), RENT-A11-F3-P1 (Send to Spare Work) |  |
| RENT-A11-F5-P2 | **Invoice to expenses**: The trade's invoice goes into the landlord's expense ledger against the property. | service | small | RENT-A11-F1-P1 (Repair job model), RENT-A14-F1-P1 (Expense ledger) |  |
| RENT-A11-F6-P1 | **Warranty and manual fields**: Manuals, warranties and service dates kept on the one chattel and appliance list (RENT-A2-F2-P1); no second register. | data | small | RENT-A2-F2-P1 (Room, chattel and appliance list), RENT-A2-F6-P1 (Shared document vault) |  |
| RENT-A11-F6-P2 | **Warranty and service reminders**: Reminds the landlord before a warranty ends or a service is due. | service | small | svc-messaging |  |
| RENT-A11-F7-P1 | **Recurring job planner**: Creates jobs on a schedule and books them like any other job. | service | small | RENT-A11-F1-P1 (Repair job model) |  |
| RENT-A11-F8-P1 | **Claim pack**: Gathers photos, reports and invoices for an insurer as one PDF. | service | small | RENT-A11-F1-P1 (Repair job model), RENT-A10-F1-P1 (Condition report model), engine-pdf |  |
| RENT-A11-F9-P1 | **History on the property**: All jobs, costs and photos linked to the property record. | data | small | RENT-A2-F1-P2 (Rental fields on the shared property record), RENT-A11-F1-P1 (Repair job model) |  |

## RENT-A12 Healthy Homes and compliance

A checklist and evidence store for every Healthy Homes standard, a heating calculator, the compliance and insulation statements each agreement needs, and a clear view of gaps for landlords and tenants.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A12-F1 Healthy Homes checklist | Work through each Healthy Homes standard and see what is done and what isn't. | core | Checklist and evidence design; Standards model; Checklist screen |
| RENT-A12-F2 Heating calculator | Work out the heating the main living room needs from its size, windows, insulation and the rules that apply to this home and tenancy date. | core | Heating requirement calculation; Room measurement entry; Formulas and test cases |
| RENT-A12-F3 Evidence store | Keep photos, invoices and installer certificates as proof for each standard. | core | Evidence per standard |
| RENT-A12-F4 Compliance statements | Produce the Healthy Homes compliance statement and insulation statement for each new or renewed agreement. | core | Compliance statement; Insulation statement |
| RENT-A12-F5 Exemptions and work plans | Record exemptions and plan the work to close any gaps. | next | Exemption records; Work plan |
| RENT-A12-F6 Portfolio compliance | See the compliance state of every property you let. | next | Compliance panel in the portfolio |
| RENT-A12-F7 Renters see the standards | As a renter, see the home's compliance and ask about any gaps. | core | Tenant compliance view |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A12-F1-P1 | **Checklist and evidence design**: Design note for the checklist and evidence store for the rental standards. | research | small |  | [RT05](STATUS.md) |
| RENT-A12-F1-P2 | **Standards model**: Heating, insulation, ventilation, moisture and drainage, and draught stopping, as checkable items, with each version of the standards and the date it applies from. | data | medium | RENT-A21-F1-P1 (Rule pack format), RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A12-F1-P3 | **Checklist screen**: Each standard with its items, status and evidence, per property. | interface | medium | RENT-A12-F1-P2 (Standards model) |  |
| RENT-A12-F2-P1 | **Heating requirement calculation**: Applies the right heating formula by dwelling type, build date and tenancy date, including the different formula for post-2008 homes and some apartments and the 2026 changes, tested against worked examples. | library | medium | kit-core, RENT-A12-F2-P3 (Formulas and test cases), RENT-A9-F1-P4 (Law by date) |  |
| RENT-A12-F2-P2 | **Room measurement entry**: Enter room size, windows and walls, or take them from a floor plan. | interface | small | engine-draw2d, RENT-A3-F2-P4 (Floor plans) |  |
| RENT-A12-F2-P3 | **Formulas and test cases**: Research each version of the heating, ventilation and moisture rules, including the 2022, September 2025 and 2026 changes, and collect worked examples to test against. | research | small |  | [RT05](STATUS.md) |
| RENT-A12-F3-P1 | **Evidence per standard**: Evidence files linked to each item, kept for the time the law requires. | service | small | RENT-A2-F6-P1 (Shared document vault), RENT-A12-F1-P2 (Standards model), RENT-A16-F5-P1 (Retention rules) |  |
| RENT-A12-F4-P1 | **Compliance statement**: Fills the statement from the checklist for the agreement. | service | small | RENT-A12-F1-P2 (Standards model), engine-pdf |  |
| RENT-A12-F4-P2 | **Insulation statement**: Fills the insulation statement from the insulation records. | service | small | RENT-A12-F1-P2 (Standards model), engine-pdf |  |
| RENT-A12-F5-P1 | **Exemption records**: Exemptions with their reason and evidence. | data | small | RENT-A12-F1-P2 (Standards model) |  |
| RENT-A12-F5-P2 | **Work plan**: Turns gaps into jobs for Spare Work trades. | service | small | RENT-A12-F1-P2 (Standards model), RENT-A11-F3-P1 (Send to Spare Work) |  |
| RENT-A12-F6-P1 | **Compliance panel in the portfolio**: The compliance part of the portfolio dashboard (RENT-A2-F9-P1): gaps and expiring evidence per property, with no separate dashboard. | interface | small | RENT-A2-F9-P1 (Portfolio dashboard), RENT-A12-F1-P2 (Standards model) |  |
| RENT-A12-F7-P1 | **Tenant compliance view**: The checklist and statement in plain words, with a button to ask about a gap. | interface | small | RENT-A12-F1-P2 (Standards model), svc-messaging |  |

## RENT-A13 Ending tenancies, disputes and the Tribunal

Tenancies end with the right notice and one move-out flow; disagreements are settled with structured offers where possible, made enforceable through FastTrack Resolution, or go to the Tenancy Tribunal with an evidence pack built from records already kept. Only the Tribunal decides tenancy disputes; member panels handle only complaints about the platform and paid manager and helper jobs.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A13-F1 End a tenancy | Give or receive notice to end and follow every step to move-out. | core | Ending flow; Move-out checklist |
| RENT-A13-F2 Final inspection and bond | Do the final inspection and settle the bond in one flow. | core | Exit-to-refund flow |
| RENT-A13-F3 Settle it between you | Settle a disagreement with structured offers before going to the Tribunal. | core | Structured offers; Signed settlement; FastTrack Resolution |
| RENT-A13-F4 Tenancy Tribunal pack | Build an evidence pack for the Tenancy Tribunal from the records already kept. | core | Evidence pack builder; Tribunal and mediation steps; Guided application |
| RENT-A13-F5 Track orders | Record Tribunal orders and their dates, see rehearing and appeal deadlines, and get help when an order isn't paid. | next | Order record; Rehearing and appeal clocks; Getting an order paid |
| RENT-A13-F6 Abandoned homes and goods | Follow the legal steps when a home or goods seem to be abandoned. | later | Abandonment steps |
| RENT-A13-F7 References after a tenancy | Give or ask for a reference when a tenancy ends. | next | Reference from the tenancy record |
| RENT-A13-F8 Complaints about the platform | Raise a complaint about a fee, review or account decision with a member panel. | core | Platform disputes; Which disputes go where |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A13-F1-P1 | **Ending flow**: The right notice and steps for each reason and tenancy type, with the clock running. | service | medium | RENT-A9-F2-P2 (Notice builder), RENT-A9-F1-P2 (Legal clock engine), RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A13-F1-P2 | **Move-out checklist**: Keys, meter readings, final inspection, final rent and bond, in one list. | interface | small | RENT-A6-F4-P1 (Checklist engine) |  |
| RENT-A13-F2-P1 | **Exit-to-refund flow**: Leads from the exit comparison straight into the bond refund split. | service | small | RENT-A10-F1-P1 (Condition report model), RENT-A7-F6-P1 (Refund split proposal) |  |
| RENT-A13-F3-P1 | **Structured offers**: Each side makes clear offers with evidence and can accept part of one, on the kit's settlement offers with no money held. | interface | small | KIT-A11-F3-P1 (Settlement offers), svc-messaging |  |
| RENT-A13-F3-P2 | **Signed settlement**: An agreed settlement signed and kept with the tenancy. | service | small | RENT-A6-F2-P1 (E-signing) |  |
| RENT-A13-F3-P3 | **FastTrack Resolution**: Turns an agreed settlement of money owed into an application for a Mediator's Order sealed by the Tribunal, stating the amount, the repayment plan, when payments start and what happens on a missed payment; the tenant is told before filing, a mediator then checks the terms with them, and the filing fee is $28. | service | medium | RENT-A13-F3-P2 (Signed settlement), RENT-A8-F6-P1 (Payment plan tracking), RENT-A13-F4-P2 (Tribunal and mediation steps) |  |
| RENT-A13-F4-P1 | **Evidence pack builder**: Agreement, rent ledger, notices, reports, photos and messages gathered into one indexed PDF. | service | medium | engine-pdf, RENT-A8-F1-P1 (Rent ledger), RENT-A9-F2-P2 (Notice builder), RENT-A10-F1-P1 (Condition report model), RENT-A16-F1-P2 (Message export) |  |
| RENT-A13-F4-P2 | **Tribunal and mediation steps**: Research the application steps, formats and fees for mediation and the Tribunal. | research | small |  | [RT01](STATUS.md) |
| RENT-A13-F4-P3 | **Guided application**: Walks a person through applying on the official site with their pack ready. | interface | small | RENT-A13-F4-P1 (Evidence pack builder) |  |
| RENT-A13-F5-P1 | **Order record**: The order, its amounts and dates, with clocks for each. | data | small | RENT-A9-F1-P2 (Legal clock engine), RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A13-F5-P2 | **Rehearing and appeal clocks**: Clocks for applying for a rehearing or appealing after an order, with the exact timeframes confirmed under RT01. | service | small | RENT-A13-F5-P1 (Order record), RENT-A9-F1-P2 (Legal clock engine), RENT-A21-F1-P3 (New Zealand rule pack) |  |
| RENT-A13-F5-P3 | **Getting an order paid**: Guided steps to enforce an unpaid order through the courts' civil enforcement, with the order and records ready. | interface | small | RENT-A13-F5-P1 (Order record), RENT-A13-F4-P1 (Evidence pack builder) |  |
| RENT-A13-F6-P1 | **Abandonment steps**: Guided steps, records and clocks for abandoned homes and goods. | service | small | RENT-A21-F1-P1 (Rule pack format), RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A13-F7-P1 | **Reference from the tenancy record**: A reference built from facts on record, saved into the renter-held reference store. | service | small | RENT-A5-F5-P2 (Renter profile store), RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A13-F8-P1 | **Platform disputes**: Complaints about Spare Rent itself go through the kit's complaints about the co-op and its disputes service to a member panel, with appeal. | service | small | KIT-A21-F9-P2 (Complaints about the co-op), KIT-A11-F1-P1 (Disputes service), svc-voting |  |
| RENT-A13-F8-P2 | **Which disputes go where**: Tenancy disputes go only to the Tenancy Tribunal and never to a member panel; panels handle only complaints about the platform and paid manager and helper jobs. | data | small | RENT-A13-F8-P1 (Platform disputes), RENT-A8-F1-P4 (No hold on rent) |  |

## RENT-A14 Landlord accounts and tax

Landlords track income and costs per property without separate accounting software, capture receipts, match bank transactions, and get a year-end summary ready for their tax return or their accountant.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A14-F1 Income and costs | Track rent and costs per property in one place. | core | Expense ledger; Tax-return categories; Accounts screen |
| RENT-A14-F2 Receipt capture | Photograph a receipt and have its amount, date and supplier filled in. | core | Receipt reading; Shared text recognition choice |
| RENT-A14-F3 Bank transactions | Import bank transactions and match them to rent and costs. | core | Categorising rules |
| RENT-A14-F4 Year-end tax summary | Get a year-end summary ready for your rental tax return. | core | Year-end summary; Rental tax rules; Tax rules by year |
| RENT-A14-F5 Depreciation of chattels | Work out depreciation on chattels at the published rates. | next | Depreciation calculator |
| RENT-A14-F6 Co-owner splits | Split income and costs between co-owners by their shares. | next | Ownership share splitting |
| RENT-A14-F7 Share with your accountant | Export accounts in open formats or give your accountant read-only access. | core | Open-format accounts export; Accountant access |
| RENT-A14-F8 Owner statements | Get a monthly statement when a manager runs the property. | next | Owner statement |
| RENT-A14-F9 Cash flow forecast | Plan rent, costs and cash flow for the year ahead. | later | Forecast |
| RENT-A14-F10 Platform income reporting | Know whether Spare Rent must report landlords' and managers' income to IRD, and see what was sent. | core | Is Spare Rent a reporting platform?; Seller tax details; Yearly report and seller copies |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A14-F1-P1 | **Expense ledger**: Costs per property and category, as operations alongside the rent ledger. | data | medium | kit-ops, RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A14-F1-P2 | **Tax-return categories**: Categories that line up with the rental income tax return. | data | small | RENT-A14-F1-P1 (Expense ledger) |  |
| RENT-A14-F1-P3 | **Accounts screen**: Income, costs and net position per property and across all of them. | interface | medium | RENT-A14-F1-P1 (Expense ledger), RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A14-F2-P1 | **Receipt reading**: Reads receipts on the device with one shared text recognition engine, proposed as one engine or kit part with Spare Office, Spare OS and Spare Studio; Spare Rent adds only receipt fields. | service | small | engine-media, RENT-A14-F1-P1 (Expense ledger), RENT-A14-F2-P2 (Shared text recognition choice) |  |
| RENT-A14-F2-P2 | **Shared text recognition choice**: Research open text recognition engines, their licences and accuracy, done once for every program that needs one rather than for receipts alone. | research | small |  |  |
| RENT-A14-F3-P1 | **Categorising rules**: Rules that learn from the landlord's choices to categorise transactions. | service | small | RENT-A8-F2-P1 (Bank feed import), RENT-A14-F1-P1 (Expense ledger) |  |
| RENT-A14-F4-P1 | **Year-end summary**: Income, deductible costs and depreciation per property for the tax year, as a PDF and data file. | service | medium | RENT-A14-F1-P1 (Expense ledger), RENT-A8-F1-P1 (Rent ledger), engine-pdf, RENT-A14-F4-P3 (Tax rules by year), KIT-A19-F11-P1 (Accessible PDFs) |  |
| RENT-A14-F4-P2 | **Rental tax rules**: Research rental income tax rules for each tax year: interest deductions, depreciation, loss ring-fencing, and tax on sale such as the bright-line test. | research | medium |  |  |
| RENT-A14-F4-P3 | **Tax rules by year**: Applies each tax year's own rules, such as interest deduction limits that changed year by year, so a summary never mixes one year's rules with another's. | library | small | RENT-A14-F4-P2 (Rental tax rules) |  |
| RENT-A14-F5-P1 | **Depreciation calculator**: Applies published rates to each chattel from its cost and date. | library | small | RENT-A2-F2-P1 (Room, chattel and appliance list) |  |
| RENT-A14-F6-P1 | **Ownership share splitting**: Splits each line by ownership share for each owner's own return. | service | small | RENT-A1-F4-P1 (Landlord entity model), RENT-A14-F1-P1 (Expense ledger) |  |
| RENT-A14-F7-P1 | **Open-format accounts export**: Spare Rent's accounts in the kit's accounting export formats, as CSV and JSON that accounting tools can import. | service | small | KIT-A4-F4-P2 (Accounting exports), RENT-A14-F1-P1 (Expense ledger) |  |
| RENT-A14-F7-P2 | **Accountant access**: A read-only accountant role for the properties the landlord chooses. | service | small | RENT-A1-F5-P1 (Role and permission model) |  |
| RENT-A14-F8-P1 | **Owner statement**: Rent received, costs paid and the manager's at-cost fee, each month. | service | small | RENT-A8-F1-P1 (Rent ledger), RENT-A14-F1-P1 (Expense ledger), engine-pdf |  |
| RENT-A14-F9-P1 | **Forecast**: Projects rent and planned costs, including planned maintenance and insurance. | service | small | RENT-A8-F1-P1 (Rent ledger), RENT-A14-F1-P1 (Expense ledger) |  |
| RENT-A14-F10-P1 | **Is Spare Rent a reporting platform?**: Whether IRD's digital platform information rules cover Spare Rent: renting out property, residential included, and personal services count; advertising alone does not, but agreements, rent tracking and paid manager services may. | research | small | KIT-A4-F3-P1 (Platform reporting rules) |  |
| RENT-A14-F10-P2 | **Seller tax details**: If it is, collects and checks landlords' and co-op managers' tax details through the kit. | service | small | KIT-A4-F3-P2 (Seller details), RENT-A14-F10-P1 (Is Spare Rent a reporting platform?) |  |
| RENT-A14-F10-P3 | **Yearly report and seller copies**: Files the yearly report through the kit by 7 February and gives each landlord and manager a copy of what was sent. | service | small | KIT-A4-F3-P3 (Yearly IRD report), RENT-A14-F10-P2 (Seller tax details) |  |

## RENT-A15 Co-op property managers and helpers

Landlords who don't want to self-manage can hire a co-op member to manage at a published, at-cost price, or book single services such as letting, viewings or inspections; managers get a workspace for many owners, and handovers lose nothing.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A15-F1 Find a manager at cost | Find a co-op member who manages properties near you at a published, at-cost price. | next | Manager listings; Manager profile |
| RENT-A15-F2 Book single services | Book only what you need, such as letting, hosting viewings, an inspection or a Tribunal pack. | next | Service menu and booking; Pay and pay out at cost; Job disputes |
| RENT-A15-F3 Manager workspace | As a manager, run many properties for many owners from one workspace. | next | Multi-owner workspace; Bulk actions |
| RENT-A15-F4 Owner approvals | Ask the owner to approve costs or decisions above a set limit. | next | Approval limits and requests |
| RENT-A15-F5 Management agreement | Sign a clear management agreement that sets out duties and price. | next | Management agreement template; A manager's legal duties |
| RENT-A15-F6 Change or end management | Move a property between managers, or back to the owner, with no lost records. | next | Handover between managers |
| RENT-A15-F7 Rate managers and helpers | Rate a manager or helper after the work. | next | Two-way ratings |
| RENT-A15-F8 Rent collected by a manager | Let a manager collect rent for an owner, through a regulated provider only. | later | Client money and trust accounts; Collected rent split and payout |
| RENT-A15-F9 Managers and helpers as workers | Work as a co-op manager, inspector or viewing host with clear status, safety, checks, insurance and tax handled. | next | Worker status; Safety when working alone; Right to work and safety check; Professional indemnity cover; Tax invoices and income reporting |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A15-F1-P1 | **Manager listings**: Managers and helpers listed on the marketplace engine and matched by area and service. | service | medium | kit-market, svc-matching, svc-maps |  |
| RENT-A15-F1-P2 | **Manager profile**: Services, areas, price, verified checks and ratings. | interface | small | RENT-A15-F1-P1 (Manager listings), RENT-A1-F2-P1 (Identity check hookup) |  |
| RENT-A15-F2-P1 | **Service menu and booking**: Fixed-price single services booked through the marketplace loop. | service | medium | kit-market, RENT-A15-F1-P1 (Manager listings) |  |
| RENT-A15-F2-P2 | **Pay and pay out at cost**: The landlord pays through the kit's payments, the money is held until the job is confirmed, and the helper is paid out minus only the at-cost fee. | integration | small | KIT-A3-F1-P2 (Payment provider link), KIT-A3-F2-P1 (Hold and release rules), KIT-A3-F3-P2 (Payout runs), KIT-A4-F9-P3 (GST on the fee) |  |
| RENT-A15-F2-P3 | **Job disputes**: A poor inspection or a missed viewing goes through the kit's disputes service, with the payment held until it is settled. | service | small | RENT-A15-F2-P2 (Pay and pay out at cost), KIT-A11-F1-P1 (Disputes service), KIT-A11-F4-P1 (Panel selection) |  |
| RENT-A15-F3-P1 | **Multi-owner workspace**: All the manager's properties, tenancies, clocks and jobs, filtered by owner. | interface | large | RENT-A1-F5-P1 (Role and permission model), RENT-A9-F1-P2 (Legal clock engine) |  |
| RENT-A15-F3-P2 | **Bulk actions**: Send notices, plan inspections and review rents across many tenancies at once. | service | medium | kit-ops, RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A15-F4-P1 | **Approval limits and requests**: The owner sets limits; anything above asks the owner first. | service | small | RENT-A1-F5-P1 (Role and permission model), svc-messaging |  |
| RENT-A15-F5-P1 | **Management agreement template**: A plain agreement between owner and manager, signed online. | data | small | RENT-A6-F2-P1 (E-signing) |  |
| RENT-A15-F5-P2 | **A manager's legal duties**: A lawyer's view on the duties of someone managing a tenancy for a landlord. | legal | medium |  |  |
| RENT-A15-F6-P1 | **Handover between managers**: Moves access, open jobs and clocks to the new manager in one step. | service | small | RENT-A1-F5-P1 (Role and permission model), kit-ops |  |
| RENT-A15-F7-P1 | **Two-way ratings**: Ratings and reviews through the kit, revealed together, one reply each, made only by people and tied to real jobs. | service | small | KIT-A9-F1-P1 (Ratings service), KIT-A9-F2-P1 (Reviews), KIT-A17-F9-P2 (People-only actions) |  |
| RENT-A15-F8-P1 | **Client money and trust accounts**: Research the client money and trust account rules for managers collecting rent. | research | medium | svc-payments, KIT-A4-F5-P1 (Anti-money-laundering programme) | [K02](../kit/STATUS.md) |
| RENT-A15-F8-P2 | **Collected rent split and payout**: Rent split between owner and the manager's at-cost fee by the payment provider. | integration | medium | svc-payments, RENT-A8-F1-P1 (Rent ledger), KIT-A3-F11-P1 (Bank payments), KIT-A3-F12-P1 (Regular payments) |  |
| RENT-A15-F9-P1 | **Worker status**: Whether co-op managers, inspectors and viewing hosts are employees or contractors, settled with the kit's worker status test and declared in Spare Rent's settings. | data | small | KIT-A23-F1-P1 (Worker status test), KIT-A23-F1-P2 (Worker status per app) |  |
| RENT-A15-F9-P2 | **Safety when working alone**: Safety for workers alone in other people's homes: check-ins, an alert, hazard notes and incident records, under the co-op's health and safety duties. | service | small | RENT-A5-F2-P3 (Safety for whoever hosts a viewing), KIT-A24-F1-P1 (Health and safety duties), KIT-A24-F2-P1 (Incident register), KIT-A24-F5-P1 (Hazard notes) |  |
| RENT-A15-F9-P3 | **Right to work and safety check**: Right to work and a consented safety check before anyone enters tenanted homes for the co-op. | service | small | KIT-A2-F11-P2 (Visa entitlement check), KIT-A2-F9-P2 (Background check link), KIT-A2-F2-P2 (Check rules per app) |  |
| RENT-A15-F9-P4 | **Professional indemnity cover**: Managers and inspectors hold professional indemnity cover, recorded with its expiry and re-checked. | data | small | KIT-A2-F3-P1 (Expiry and re-check scheduler) |  |
| RENT-A15-F9-P5 | **Tax invoices and income reporting**: GST tax invoices for their fees, and their income reported as personal services where the platform rules apply. | service | small | KIT-A4-F2-P1 (Tax invoices), KIT-A4-F3-P3 (Yearly IRD report), RENT-A14-F10-P3 (Yearly report and seller copies) |  |

## RENT-A16 Messages, notifications and documents

One message thread per tenancy kept as a record, formal notices kept apart from chat, each person choosing how they are reached, and every document kept for exactly as long as the law requires.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A16-F1 Tenancy messages | Message the other side of a tenancy in one thread that is kept as a record. | core | Tenancy thread; Message export |
| RENT-A16-F2 Formal and everyday messages apart | Tell at a glance which messages are formal notices and which are chat. | core | Notice markers in the thread |
| RENT-A16-F3 Choose how you're reached | Choose push, email or text for each kind of message, and set quiet hours. | core | Notification preferences |
| RENT-A16-F4 Email for formal notices | Agree an email address for formal notices, recorded in the agreement. | core | Address for service record |
| RENT-A16-F5 Documents kept for the right time | Keep every tenancy document for the time the law requires, then have it removed. | core | Retention rules; Tenancy documents list; Record-keeping duties |
| RENT-A16-F6 Messages in your language | Read and write messages in your own language, with the original kept as the record. | next | Message translation; Tenancy wording in translation |
| RENT-A16-F7 Building announcements | Message all tenants in a building at once about shared matters. | next | Building broadcast |
| RENT-A16-F8 Take part without an account | As a tenant, co-tenant or guarantor who hasn't joined, read and sign the agreement, get notices, see reports and report repairs from a private link. | core | Guest links for tenancies; Repair QR code; Guest link safety |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A16-F1-P1 | **Tenancy thread**: One thread per tenancy on the shared messaging service, with photos and files. | service | small | svc-messaging, KIT-A8-F1-P2 (Chat service) |  |
| RENT-A16-F1-P2 | **Message export**: Exports a thread with dates for an evidence pack. | service | small | RENT-A16-F1-P1 (Tenancy thread) |  |
| RENT-A16-F2-P1 | **Notice markers in the thread**: A formal notice shows in the tenancy thread as a marker that links to the notice inbox (RENT-A9-F3-P2) and says how it was legally served; delivery in the app alone is not service. | service | small | RENT-A9-F3-P2 (Notice inbox), RENT-A9-F2-P5 (Service, not just delivery) |  |
| RENT-A16-F3-P1 | **Notification preferences**: Spare Rent's message kinds in the kit's notification settings, with formal notices always delivered. | service | small | KIT-A8-F5-P1 (Notification settings) |  |
| RENT-A16-F4-P1 | **Address for service record**: The one record of each party's agreed email and postal addresses for service, with the date each was given, used by the landlord's details, agreements and notices; proposed for the kit's formal-notices service with Spare Home. | data | small | kit-ops, RENT-A1-F4-P1 (Landlord entity model) |  |
| RENT-A16-F5-P1 | **Retention rules**: Tenancy document types added to the kit's retention schedules, with how long each must be kept taken from the rule pack. | service | small | KIT-A18-F6-P1 (Retention schedules), RENT-A21-F1-P1 (Rule pack format), RENT-A16-F5-P3 (Record-keeping duties) |  |
| RENT-A16-F5-P2 | **Tenancy documents list**: All documents for a tenancy, with who can see each one. | interface | small | RENT-A2-F6-P1 (Shared document vault) |  |
| RENT-A16-F5-P3 | **Record-keeping duties**: Research how long landlords must keep each tenancy and tax record. | research | small |  | [RT01](STATUS.md) |
| RENT-A16-F6-P1 | **Message translation**: Uses the kit's message translation; the original message stays the record. | service | small | KIT-A8-F7-P1 (Message translation) |  |
| RENT-A16-F6-P2 | **Tenancy wording in translation**: Checks the kit's translation at cost on tenancy wording, with te reo Māori checked by fluent speakers. | research | small | KIT-A19-F10-P1 (Translation model choice), KIT-A19-F10-P3 (Te reo quality checks) |  |
| RENT-A16-F7-P1 | **Building broadcast**: One message to all tenants of a building, kept with each tenancy. | service | small | svc-messaging, RENT-A2-F1-P2 (Rental fields on the shared property record) |  |
| RENT-A16-F8-P1 | **Guest links for tenancies**: A private link lets a tenant, co-tenant or guarantor who hasn't joined read and sign the agreement, receive notices, see inspection reports and report repairs. | service | medium | KIT-A15-F8-P1 (Sharing and access), KIT-A20-F4-P1 (Deep links), RENT-A6-F2-P1 (E-signing), RENT-A9-F3-P2 (Notice inbox), RENT-A11-F1-P2 (Report screen) |  |
| RENT-A16-F8-P2 | **Repair QR code**: A QR code for the property that opens a repair report with no app or account. | interface | small | RENT-A16-F8-P1 (Guest links for tenancies), RENT-A11-F1-P2 (Report screen) |  |
| RENT-A16-F8-P3 | **Guest link safety**: Links are personal, expire and can be revoked, every use is logged, and signing confirms a code sent to the guest's email or phone. | library | small | RENT-A16-F8-P1 (Guest links for tenancies), KIT-A1-F1-P3 (Email and phone checks) |  |

## RENT-A17 Reviews, trust and safety

Fair, checked reviews tied to real tenancies, protection from rental scams, reports anyone can make and follow, help with discrimination and harassment, and moderation with a public log and appeals.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A17-F1 Reviews of landlords and tenants | Leave a fair, checked review of a landlord, manager or tenant after a real tenancy. | next | What can be published; Reviews tied to tenancies; Right of reply and correction; No blacklists |
| RENT-A17-F2 Protection from rental scams | Be protected from fake listings and requests for money before a viewing. | core | Scam warnings in messages; Scam guide |
| RENT-A17-F3 Report a problem | Report a listing, message or person, and see what happened to your report. | core | Report flow; Report status |
| RENT-A17-F4 Discrimination | Report a listing or decision you think is unlawful discrimination, and know where else to go. | core | Discrimination reports |
| RENT-A17-F5 Harassment and safety | Block someone, and get help if a tenancy becomes unsafe. | core | Block and mute; Safety resources |
| RENT-A17-F6 Moderation with a public log | See every removal or ban and the rule behind it, and appeal a decision. | core | Moderation queue; Public log and appeals |
| RENT-A17-F7 Trust badges | See verification and track-record badges whose rules are published. | core | Published badge rules |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A17-F1-P1 | **What can be published**: Research what reviews of landlords and tenants can be published fairly and legally. | research | small |  | [RT04](STATUS.md) |
| RENT-A17-F1-P2 | **Reviews tied to tenancies**: Reviews only from people with a verified tenancy, on the kit's ratings and reviews: revealed together, one reply each, and written only by people. | service | medium | KIT-A9-F1-P1 (Ratings service), KIT-A9-F2-P1 (Reviews), KIT-A17-F9-P2 (People-only actions), RENT-A17-F1-P1 (What can be published) |  |
| RENT-A17-F1-P3 | **Right of reply and correction**: The person reviewed can reply and ask for factual errors to be corrected. | interface | small | KIT-A9-F2-P1 (Reviews), KIT-A9-F5-P1 (Rating challenge) |  |
| RENT-A17-F1-P4 | **No blacklists**: Spare Rent never keeps a shared list of bad tenants or bad landlords outside the reviewed person's knowledge and control; every review is visible to the person it is about. | data | small | RENT-A17-F1-P2 (Reviews tied to tenancies) |  |
| RENT-A17-F2-P1 | **Scam warnings in messages**: The kit's scam warnings, with rental patterns added: money asked before a viewing, or a move off the app. | service | small | KIT-A8-F9-P1 (Scam warnings), RENT-A3-F8-P2 (Scam checks on new listings) |  |
| RENT-A17-F2-P2 | **Scam guide**: Plain advice on the common rental scams, how to check official Tenancy Services messages, and what to do. | data | small | svc-trust |  |
| RENT-A17-F3-P1 | **Report flow**: Reports through the kit's reports service, with Spare Rent's report types and evidence attached. | service | small | KIT-A10-F3-P1 (Reports service) |  |
| RENT-A17-F3-P2 | **Report status**: The kit's report forms show the reporter each step and the outcome. | interface | small | KIT-A10-F3-P2 (Report forms) |  |
| RENT-A17-F4-P1 | **Discrimination reports**: A report type that explains the grounds and points to the Human Rights Commission. | service | small | svc-trust, KIT-A10-F19-P1 (Non-discrimination commitment) |  |
| RENT-A17-F5-P1 | **Block and mute**: The kit's blocking stops chat from a person while formal notices still arrive. | service | small | KIT-A10-F4-P1 (Blocking), RENT-A9-F3-P2 (Notice inbox) |  |
| RENT-A17-F5-P2 | **Safety resources**: The kit's emergency numbers and helplines, plus tenancy advice services, in one place. | data | small | KIT-A10-F1-P3 (Helplines and 111 TXT) |  |
| RENT-A17-F6-P1 | **Moderation queue**: Spare Rent's reports and automatic flags in the kit's one review queue. | interface | small | KIT-A10-F6-P2 (Review queue) |  |
| RENT-A17-F6-P2 | **Public log and appeals**: Each removal or ban published with its rule, with appeal to a member panel. | service | small | KIT-A10-F7-P1 (Public removal log), KIT-A11-F5-P1 (Appeals), svc-voting | [M04](../spare-social/STATUS.md) |
| RENT-A17-F7-P1 | **Published badge rules**: Each badge's meaning and how it is earned, in plain words. | data | small | svc-trust, RENT-A1-F2-P1 (Identity check hookup), KIT-A2-F5-P1 (Verified badge) |  |

## RENT-A18 Flatmates, rooms and other ways to rent

Finding and living with flatmates, splitting rent and bills, and later the other ways people rent: boarding houses, community housing providers and commercial leases.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A18-F1 Find a flatmate | List a room or look for one, with profiles that say how the flat lives. | next | Room listing type; Flatmate profiles and matching |
| RENT-A18-F2 Flat agreements | Agree flat rules, rent shares and bills in a simple flat agreement. | next | Flat agreement template; Flatmates and head tenants |
| RENT-A18-F3 Split rent and bills | Split rent and bills among flatmates and see who owes what. | next | Shared bills ledger; Settle-up reminders |
| RENT-A18-F4 Flatmate changes | Handle a flatmate moving out or in, with the bond and agreement updated. | next | Flatmate change flow |
| RENT-A18-F5 Boarding houses | Run a boarding house under its own rules. | later | Boarding house rules |
| RENT-A18-F6 Community housing providers | Let community housing providers run their tenancies on Spare Rent. | later | Community housing needs |
| RENT-A18-F7 Commercial leases | Let shops, offices and workshops on a commercial lease. | later | Commercial lease needs |
| RENT-A18-F8 Is this a tenancy? | Check which law applies before a let starts, so every clock and notice is right. | core | Which law applies; Long stays from Spare Stay |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A18-F1-P1 | **Room listing type**: A room in a shared home, with the flat's rules and costs. | data | small | RENT-A3-F1-P1 (Listing model) |  |
| RENT-A18-F1-P2 | **Flatmate profiles and matching**: Profiles on routines and expectations, matched with the shared matching service. | service | medium | svc-matching, RENT-A5-F5-P2 (Renter profile store) |  |
| RENT-A18-F2-P1 | **Flat agreement template**: A plain agreement between flatmates, signed online. | data | small | RENT-A6-F2-P1 (E-signing) |  |
| RENT-A18-F2-P2 | **Flatmates and head tenants**: Research how flatmates sit outside or inside tenancy law, and head tenants' duties. | research | small |  | [RT01](STATUS.md) |
| RENT-A18-F3-P1 | **Shared bills ledger**: Bills and shares per flatmate, as operations everyone in the flat can see. | service | medium | kit-ops |  |
| RENT-A18-F3-P2 | **Settle-up reminders**: Gentle reminders and a running balance between flatmates. | service | small | svc-messaging |  |
| RENT-A18-F4-P1 | **Flatmate change flow**: Updates the flat agreement and, for named tenants, the bond. | service | small | RENT-A7-F1-P2 (Tenancy Bond API connection), RENT-A6-F1-P1 (Agreement builder) |  |
| RENT-A18-F5-P1 | **Boarding house rules**: Research and build the boarding house rules as their own rule pack. | data | medium | RENT-A21-F1-P1 (Rule pack format) | [RT01](STATUS.md) |
| RENT-A18-F6-P1 | **Community housing needs**: Research what community housing providers need beyond private renting. | research | medium |  |  |
| RENT-A18-F7-P1 | **Commercial lease needs**: Research what commercial leasing needs: rent reviews, outgoings and lease terms. | research | medium |  |  |
| RENT-A18-F8-P1 | **Which law applies**: Questions before a let starts that sort out service tenancies, boarders in the owner's own home, student halls, boarding houses and flatmates, then set the right rule pack or say tenancy law doesn't apply. | service | small | RENT-A21-F1-P3 (New Zealand rule pack), RENT-A18-F2-P2 (Flatmates and head tenants) |  |
| RENT-A18-F8-P2 | **Long stays from Spare Stay**: Accepts a hand-off through Spare Stay's public command API when a long stay becomes a tenancy (STAY-A14-F8), carrying the people and dates over. | integration | small | RENT-A18-F8-P1 (Which law applies), KIT-A16-F1-P1 (Command API), RENT-A19-F3-P1 (Spare Rent command set) |  |

## RENT-A19 Data, AI and plugins

Everything a person can do is a command that scripts, AI assistants and plugins can use too; members can export all their data in open formats and bring records in from elsewhere.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A19-F1 Export everything | Download all your Spare Rent data in open formats at any time. | core | Full export; Published file formats |
| RENT-A19-F2 Import from elsewhere | Bring properties, tenancies and records in from a spreadsheet or another tool's export. | next | Spreadsheet import; Other tools' export formats |
| RENT-A19-F3 Commands for everything | Do anything in Spare Rent through the same commands from a script. | core | Spare Rent command set; Command reference |
| RENT-A19-F4 Your own AI assistant | Let your own AI assistant draft listings, triage repairs and check deadlines, while a person approves anything with legal effect. | next | MCP tools; Actions only a person approves; Agent labels and guards; AI uses listed |
| RENT-A19-F5 Automations | Set your own rules, such as a reminder or a task when something happens. | next | Event rules and webhooks |
| RENT-A19-F6 Plugins | Add sandboxed add-ins, such as a council data feed or a custom report. | later | Plugin hooks |
| RENT-A19-F7 Open rental statistics | Share anonymised rent and compliance statistics as open data, if members vote for it. | later | Anonymised statistics; Privacy-safe grouping |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A19-F1-P1 | **Full export**: Spare Rent's records in the kit's data export, including each side's copy of shared tenancy records, as JSON, CSV and PDFs. | service | small | KIT-A18-F1-P1 (Data export), kit-store |  |
| RENT-A19-F1-P2 | **Published file formats**: Documented formats so exports can be read without Spare Rent. | data | small | RENT-A19-F1-P1 (Full export) |  |
| RENT-A19-F2-P1 | **Spreadsheet import**: Maps columns from a spreadsheet to properties, tenancies and rent. | service | medium | RENT-A2-F1-P2 (Rental fields on the shared property record), RENT-A8-F1-P1 (Rent ledger) |  |
| RENT-A19-F2-P2 | **Other tools' export formats**: Research the export formats other landlord tools publish in their own documentation. | research | small |  |  |
| RENT-A19-F3-P1 | **Spare Rent command set**: Every action as a command on the operation log, used by the interface, AI and plugins alike. | library | large | kit-ops, KIT-A15-F1-P3 (Command schemas) |  |
| RENT-A19-F3-P2 | **Command reference**: Spare Rent's commands and queries in the kit's generated developer docs. | data | small | KIT-A16-F3-P2 (Developer docs), RENT-A19-F3-P1 (Spare Rent command set) |  |
| RENT-A19-F4-P1 | **MCP tools**: Spare Rent commands on the shared MCP server, with the same permissions as the person. | service | medium | kit-mcp, KIT-A17-F1-P1 (MCP server), KIT-A17-F4-P1 (Agent limits), RENT-A19-F3-P1 (Spare Rent command set) |  |
| RENT-A19-F4-P2 | **Actions only a person approves**: A full list of commands an agent may prepare but only a person may approve: serving any notice, accepting or declining an applicant, sharing an application, signing, proposing bond deductions, agreeing a payment plan, approving repair spending and ending a tenancy. | service | small | RENT-A19-F3-P1 (Spare Rent command set), KIT-A17-F2-P1 (Approval steps), KIT-A17-F9-P2 (People-only actions) |  |
| RENT-A19-F4-P3 | **Agent labels and guards**: Messages an agent sends are labelled, applicants' and tenants' messages are guarded against hidden instructions, and reviews stay people-only. | library | small | KIT-A17-F9-P1 (Agent labels), KIT-A17-F5-P1 (Hidden instruction guards), KIT-A17-F9-P2 (People-only actions), RENT-A19-F4-P1 (MCP tools) |  |
| RENT-A19-F4-P4 | **AI uses listed**: Every place Spare Rent uses AI is in the kit's AI use register with what it may decide, and no AI declines an applicant. | data | small | KIT-A17-F10-P1 (AI use register), RENT-A5-F10-P5 (People decide, not software) |  |
| RENT-A19-F5-P1 | **Event rules and webhooks**: When-this-then-that rules on tenancy events, with webhooks through the kit. | service | medium | KIT-A16-F2-P1 (Webhooks), RENT-A19-F3-P1 (Spare Rent command set) |  |
| RENT-A19-F6-P1 | **Plugin hooks**: Points where sandboxed plugins can read and act with declared permissions. | service | medium | kit-plugin, RENT-A19-F3-P1 (Spare Rent command set) |  |
| RENT-A19-F7-P1 | **Anonymised statistics**: Rent and compliance figures by area, grouped so no home or person can be picked out, published through the kit's public reporting pipeline. | service | medium | KIT-A13-F9-P1 (Public reporting pipeline), svc-voting, RENT-A8-F1-P1 (Rent ledger), RENT-A12-F1-P2 (Standards model), RENT-A19-F7-P2 (Privacy-safe grouping) |  |
| RENT-A19-F7-P2 | **Privacy-safe grouping**: Research how to group statistics so they can't identify anyone. | research | small |  |  |

## RENT-A20 Access for everyone

Spare Rent works for everyone: with a screen reader or keyboard, in plain words, easy read and NZ Sign Language, in te reo Māori and other languages, offline, on any phone and on slow connections, by paper and phone for people who are offline, and with a trusted helper for anyone who needs one.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A20-F1 Accessible by default | Use every screen with a keyboard, screen reader, large text or high contrast. | core | Accessibility checks; Testing with disabled people |
| RENT-A20-F2 Plain words | Read everything in short, plain sentences, with legal terms explained. | core | Tenancy glossary; Easy read and NZ Sign Language |
| RENT-A20-F3 Many languages | Use the app in English or te reo Māori first, then other languages. | core | Interface translation system; Te reo Māori; Pacific and Asian languages |
| RENT-A20-F4 Work offline | Keep working with no signal; changes sync when you're back online. | core | Offline data for active tenancies |
| RENT-A20-F5 Phone apps | Use Spare Rent on any phone, with notifications and the camera. | core | Phone app approach; Phone app shell |
| RENT-A20-F6 Slow connections and older phones | Use the app on slow connections and older phones. | next | Low-data mode |
| RENT-A20-F7 A trusted helper | Let a family member or support worker help you use Spare Rent, with access you control. | next | Support person role |
| RENT-A20-F8 Rent without a phone or internet | Take every legal step by paper or phone if you are offline, and get notices by post. | core | Paper and phone route; Posted copies |
| RENT-A20-F9 Accessible documents | Read and sign agreements, notices and statements with a screen reader. | core | Accessible tenancy documents; Signing with a screen reader |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A20-F1-P1 | **Accessibility checks**: Spare Rent's screens checked with the kit's accessible components and accessibility testing against WCAG 2.2 AA, plus manual checks of tenancy journeys. | service | small | KIT-A19-F2-P1 (Accessible components), KIT-A19-F2-P2 (Accessibility testing) |  |
| RENT-A20-F1-P2 | **Testing with disabled people**: Research sessions with disabled renters and landlords on the main journeys. | research | small |  |  |
| RENT-A20-F2-P1 | **Tenancy glossary**: A plain-language glossary linked wherever a legal term appears. | data | small |  |  |
| RENT-A20-F2-P2 | **Easy read and NZ Sign Language**: Easy-read and NZ Sign Language versions of tenants' and landlords' main rights, on the kit's easy-read support. | data | medium | KIT-A19-F8-P1 (Easy read and NZ Sign Language), RENT-A20-F2-P1 (Tenancy glossary) |  |
| RENT-A20-F3-P1 | **Interface translation system**: Uses the kit's translation framework; Spare Rent adds only its own strings. | library | small | KIT-A19-F1-P1 (Translation framework) |  |
| RENT-A20-F3-P2 | **Te reo Māori**: A full te reo Māori interface, made with fluent speakers. | data | medium | RENT-A20-F3-P1 (Interface translation system) |  |
| RENT-A20-F3-P3 | **Pacific and Asian languages**: Samoan, Tongan, Chinese, Hindi and other languages renters use most. | data | medium | RENT-A20-F3-P1 (Interface translation system) |  |
| RENT-A20-F4-P1 | **Offline data for active tenancies**: Each person's active tenancies, documents and drafts kept on the device. | service | medium | kit-sync |  |
| RENT-A20-F5-P1 | **Phone app approach**: Spare Rent's needs, such as offline inspections and the camera, fed into the kit's phone app approach. | research | small | KIT-A20-F2-P1 (Phone app approach) | [K04](../kit/STATUS.md) |
| RENT-A20-F5-P2 | **Phone app shell**: Spare Rent in the kit's installable phone apps, with push notifications, camera and offline storage. | interface | medium | KIT-A20-F2-P2 (Installable apps), RENT-A20-F5-P1 (Phone app approach), kit-sync |  |
| RENT-A20-F6-P1 | **Low-data mode**: Uses the kit's light mode for slow connections, and uploads inspection photos on Wi-Fi if the person chooses. | service | small | KIT-A19-F5-P1 (Light mode for slow connections) |  |
| RENT-A20-F7-P1 | **Support person role**: The kit's delegated access, with a support person role limited to what the person chooses and ended at any time. | service | small | KIT-A1-F8-P1 (Delegated access), RENT-A1-F5-P1 (Role and permission model) |  |
| RENT-A20-F8-P1 | **Paper and phone route**: A paper and phone path for every legal step, with co-op support entering records for the person. | service | medium | KIT-A21-F4-P1 (Other support channels), RENT-A22-F5-P2 (Support desk) |  |
| RENT-A20-F8-P2 | **Posted copies**: Notices and agreements printed and posted where a party has no email for service, with the posting date recorded as proof of service. | service | small | RENT-A9-F2-P5 (Service, not just delivery), engine-pdf |  |
| RENT-A20-F9-P1 | **Accessible tenancy documents**: Agreements, notices, statements and reports made as accessible PDFs. | service | small | KIT-A19-F11-P1 (Accessible PDFs), RENT-A6-F1-P1 (Agreement builder), RENT-A9-F2-P2 (Notice builder) |  |
| RENT-A20-F9-P2 | **Signing with a screen reader**: E-signing tested with screen readers and keyboards. | interface | small | KIT-A25-F1-P2 (Signatures with audit trail), KIT-A19-F2-P2 (Accessibility testing), RENT-A6-F2-P1 (E-signing) |  |

## RENT-A21 Countries and rule packs

Tenancy law lives in rule packs, not in code, layered on the kit's country packs: New Zealand first, then other countries, states, provinces and cities, each with its own notices, clocks, caps, statements, bond scheme and duties, lawyer-checked and tested in the open.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A21-F1 Rule packs | Run each tenancy under the rules of the country and region the home is in. | core | Rule pack format; Tenancy law as product rules; New Zealand rule pack; Rule pack tests; Lawyer sign-off on releases |
| RENT-A21-F2 Local money, dates and addresses | See money, dates and addresses in the local format. | core | Locale formats |
| RENT-A21-F3 Bond and deposit schemes abroad | Lodge bonds and deposits with the right scheme in each country. | later | Bond scheme adapters; Schemes in Australia and the UK |
| RENT-A21-F4 Australia | Let homes in each Australian state and territory under its own law. | later | Australian tenancy laws; Australian rule packs |
| RENT-A21-F5 United Kingdom | Let homes in each UK nation under its own law. | later | UK tenancy laws; UK rule packs |
| RENT-A21-F6 Community-made rule packs | Let members in other countries write and maintain rule packs, reviewed by owners. | later | Contribution and review process |
| RENT-A21-F7 Data kept in region | Keep members' data in their own region where the law asks. | next | Region-pinned storage |
| RENT-A21-F8 United States and Canada | Let homes in US states and Canadian provinces under their own law, down to city rules. | later | US and Canadian tenancy laws |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A21-F1-P1 | **Rule pack format**: Spare Rent's tenancy layer on the kit's country packs: notices, clocks, caps, required statements, allowed charges and tenant fee bans, plus fields other countries need, such as rent-control formulas, landlord registration and licensing, right-to-rent checks, deposit-scheme information duties, repeating safety certificates with expiry dates (gas, electrical, energy), eviction grounds, and state, province or city layers. | library | large | KIT-A19-F6-P1 (Country packs), KIT-A19-F9-P1 (NZ country pack), kit-ops |  |
| RENT-A21-F1-P2 | **Tenancy law as product rules**: Research every duty, notice and timeframe in New Zealand tenancy law, with sources. | research | large |  | [RT01](STATUS.md) |
| RENT-A21-F1-P3 | **New Zealand rule pack**: The full New Zealand rules, with the date each started; the tenancy rules Spare Home also uses (HOME-A7-F5-P1) are kept here once and proposed for the kit. | data | large | RENT-A21-F1-P1 (Rule pack format), RENT-A21-F1-P2 (Tenancy law as product rules) |  |
| RENT-A21-F1-P4 | **Rule pack tests**: Worked examples from official guidance run as tests on every change. | library | medium | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A21-F1-P5 | **Lawyer sign-off on releases**: A lawyer signs off each rule-pack release before it goes live. | legal | small | RENT-A21-F1-P3 (New Zealand rule pack), RENT-A21-F1-P4 (Rule pack tests) |  |
| RENT-A21-F2-P1 | **Locale formats**: Uses the kit's local formats; Spare Rent adds only rent shown per week or per month as each country expects. | library | small | KIT-A19-F1-P2 (Local formats) |  |
| RENT-A21-F3-P1 | **Bond scheme adapters**: One interface with an adapter per government or approved deposit scheme. | library | medium | RENT-A7-F1-P2 (Tenancy Bond API connection) |  |
| RENT-A21-F3-P2 | **Schemes in Australia and the UK**: Research the bond and deposit schemes in Australian states and the UK, and their APIs. | research | medium |  |  |
| RENT-A21-F4-P1 | **Australian tenancy laws**: Research each state's and territory's tenancy law as product rules. | research | large |  |  |
| RENT-A21-F4-P2 | **Australian rule packs**: A rule pack per state and territory. | data | large | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A21-F5-P1 | **UK tenancy laws**: Research each UK nation's tenancy law as product rules, including England's end of no-fault eviction from 1 May 2026 and its planned landlord database and ombudsman. | research | large |  |  |
| RENT-A21-F5-P2 | **UK rule packs**: A rule pack per UK nation. | data | large | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A21-F6-P1 | **Contribution and review process**: How a rule pack is proposed, sourced, tested and accepted. | research | small | RENT-A21-F1-P1 (Rule pack format) |  |
| RENT-A21-F7-P1 | **Region-pinned storage**: Uses the kit's data location so each tenancy's records stay in the region its law requires. | service | small | KIT-A18-F9-P1 (Data location) |  |
| RENT-A21-F8-P1 | **US and Canadian tenancy laws**: Research tenancy law by state, province and city, such as rent control and deposit rules, as later rule pack targets. | research | large |  |  |

## RENT-A22 Running the co-op: membership, fees, support and legal duties

Landlords, renters and managers join as members and vote; the at-cost fee and the monthly ledger are public; people get help when they are stuck; and the co-op meets its own legal duties for privacy, security, terms and legal requests.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| RENT-A22-F1 Join as a member | Join the co-op as a landlord, renter or manager and get one vote. | core | Membership sign-up |
| RENT-A22-F2 The at-cost fee | See what Spare Rent charges landlords, why, and how it was worked out; renters never pay. | core | Fee model; Fee page; Fee charging and tax invoices; Renters never pay |
| RENT-A22-F3 Open cost ledger | Read Spare Rent's monthly running costs and what the fee paid for. | core | Spare Rent ledger page |
| RENT-A22-F4 Vote on fees and rules | Propose and vote on Spare Rent's fees and rules. | core | Proposals and votes; Who votes on Spare Rent; Settings the law fixes; No votes on rent levels |
| RENT-A22-F5 Help and support | Get help from guides and from people when you are stuck. | core | Help centre; Support desk |
| RENT-A22-F6 Operator console | As co-op staff, handle support, moderation and incidents, with every action logged. | core | Admin console; Admin action log |
| RENT-A22-F7 Privacy rights | Ask to see, correct or delete your information, and be told of any serious breach. | core | Privacy requests; Breach response; The platform's privacy duties |
| RENT-A22-F8 Terms and policies | Read short, plain terms of use, privacy policy and community rules. | core | Terms, privacy policy and community rules; Is the co-op an agent? |
| RENT-A22-F9 Security | Trust that tenancy records are protected and checked by outside reviewers. | core | Encryption of sensitive records; Security review before launch |
| RENT-A22-F10 Legal requests | See how the co-op handles requests from police and courts, in a public report. | next | Legal request process and report |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RENT-A22-F1-P1 | **Membership sign-up**: Joining through the kit's join flow, with the member's Spare Rent role recorded. | service | small | KIT-A14-F1-P3 (Join flow), svc-voting | [K01](../kit/STATUS.md) |
| RENT-A22-F2-P1 | **Fee model**: The fee per active tenancy and per listing, worked out from last period's running costs and paid by the landlord, never the renter. | service | medium | svc-ledger, kit-market, KIT-A13-F3-P1 (Fee setting), RENT-A22-F2-P4 (Renters never pay) |  |
| RENT-A22-F2-P2 | **Fee page**: The current fee, how it was worked out and the vote that set it. | interface | small | RENT-A22-F2-P1 (Fee model) |  |
| RENT-A22-F2-P3 | **Fee charging and tax invoices**: Charges the landlord the fee with GST and a tax invoice through the kit's tax service. | integration | small | KIT-A4-F9-P3 (GST on the fee), KIT-A4-F2-P1 (Tax invoices), RENT-A22-F2-P1 (Fee model) |  |
| RENT-A22-F2-P4 | **Renters never pay**: Blocks any charge to a renter connected to getting, keeping or renewing a tenancy, including consented credit checks, as the letting fee and key money bans require; the landlord pays every fee. | library | small | RENT-A21-F1-P3 (New Zealand rule pack) |  |
| RENT-A22-F3-P1 | **Spare Rent ledger page**: Spare Rent's share of the open cost ledger each month. | interface | small | svc-ledger, KIT-A13-F1-P3 (Ledger pages) | [K06](../kit/STATUS.md) |
| RENT-A22-F4-P1 | **Proposals and votes**: Spare Rent proposals on the shared voting service. | service | small | svc-voting, KIT-A12-F1-P1 (Proposals service), KIT-A12-F3-P1 (Ballots) |  |
| RENT-A22-F4-P2 | **Who votes on Spare Rent**: Who votes on Spare Rent's fees and rules: landlords, renters, co-op managers or all three, balanced so neither landlords nor renters outvote the other on rules that affect both, and whether renters who pay no fee count as transacting shareholders under the co-operative test. | research | small | svc-voting, KIT-A12-F5-P2 (Electorates), KIT-A14-F2-P3 (Co-operative test) | [K01](../kit/STATUS.md) |
| RENT-A22-F4-P3 | **Settings the law fixes**: Lists which Spare Rent settings members may vote on and which the law fixes, such as notice periods, bond caps and the letting-fee ban. | data | small | RENT-A21-F1-P3 (New Zealand rule pack), KIT-A12-F4-P1 (Results to settings) |  |
| RENT-A22-F4-P4 | **No votes on rent levels**: Members never vote or coordinate on rent levels, and the rent guide uses only public bond data, never members' pooled private rents, to keep clear of competition law. | library | small | RENT-A3-F4-P1 (Market rent data), KIT-A12-F4-P3 (Competition law check) |  |
| RENT-A22-F5-P1 | **Help centre**: Spare Rent's guides in the kit's help centre, in our own words and linking to official sources. | interface | small | KIT-A21-F1-P1 (Help centre) |  |
| RENT-A22-F5-P2 | **Support desk**: Spare Rent questions handled as the kit's support cases, with replies tracked. | service | small | KIT-A21-F2-P1 (Support cases) |  |
| RENT-A22-F6-P1 | **Admin console**: Spare Rent's accounts, listings and tenancies in the kit's admin console, within strict permissions. | interface | medium | KIT-A22-F6-P1 (Admin console), RENT-A1-F5-P1 (Role and permission model) |  |
| RENT-A22-F6-P2 | **Admin action log**: Uses the kit's logged admin actions, which members can audit; nothing extra. | service | small | KIT-A22-F6-P1 (Admin console) |  |
| RENT-A22-F7-P1 | **Privacy requests**: The kit's access, correction and deletion requests, covering Spare Rent's records and each side's copy of shared records. | service | small | KIT-A18-F3-P1 (Privacy page), RENT-A19-F1-P1 (Full export), RENT-A16-F5-P1 (Retention rules) |  |
| RENT-A22-F7-P2 | **Breach response**: The kit's breach response, with Spare Rent's most sensitive records, such as applications, flagged first. | service | small | KIT-A18-F7-P1 (Breach response) |  |
| RENT-A22-F7-P3 | **The platform's privacy duties**: Research Spare Rent's own Privacy Act duties beyond the kit's collection notices, such as the Privacy Commissioner's rental guidance. | research | small | KIT-A18-F5-P1 (Collection notices) |  |
| RENT-A22-F8-P1 | **Terms, privacy policy and community rules**: Spare Rent's terms and community rules on the kit's lawyer-checked terms and privacy policy. | legal | medium | KIT-A18-F4-P2 (Terms and privacy policy) |  |
| RENT-A22-F8-P2 | **Is the co-op an agent?**: A lawyer's view on whether the co-op or its managers count as landlords' agents under tenancy law. | legal | medium |  |  |
| RENT-A22-F9-P1 | **Encryption of sensitive records**: Applications, identity check results and documents encrypted at rest and in transit. | service | medium | KIT-A22-F5-P5 (Encryption and keys), kit-store |  |
| RENT-A22-F9-P2 | **Security review before launch**: Spare Rent in the kit's outside security reviews before it goes live. | research | medium | KIT-A22-F5-P3 (Security reviews) |  |
| RENT-A22-F10-P1 | **Legal request process and report**: Spare Rent's legal requests handled under the kit's process and counted in its transparency reports. | legal | small | KIT-A10-F9-P3 (Law enforcement requests), KIT-A10-F15-P1 (Transparency reports) |  |

## Questions for assessing this map and its demo

- Can a landlord go from adding a property to a signed agreement and a lodged bond without a property manager, and does each step say what the law needs?
- Is every legal clock right for a tenancy signed on a given date: bond lodged within 23 working days, entry notice, rent rise once in 12 months with 60 days' notice, and the 90, 42 and 21 day ending notices?
- Does the platform ever hold a bond or rent? It must never hold a bond, and rent should only pass through a regulated payment provider if both sides choose.
- Has the co-op signed MBIE's API Access Agreement, does each landlord give RealMe consent, and which bond operations go through the API and which through Bond Hub? What happens to lodgement if that access is late?
- Can a renter apply once and see exactly what each landlord sees, when, and for how long it is kept?
- Does the application flow ask only what the Privacy Commissioner's guidance allows at each stage, and does it tell renters when information comes from others (IPP 3A)?
- Are the Healthy Homes compliance statement, insulation statement and insurance statement produced correctly for a new agreement?
- What would the at-cost fee be per tenancy per month, compared with 7-10% of rent plus a letting fee?
- How will renters find listings if the big listing sites won't take a feed, and is the reach research (HM02) enough?
- Are reviews of landlords and tenants fair and legal, and who decides when one is challenged?
- Does an inspection work start to finish with no signal, and do the photos keep their dates?
- Is it usable in te reo Māori and with a screen reader from the first release?
- Which questions need a lawyer before launch: whether the co-op or its managers count as agents under tenancy law, client money, privacy duties and reviews?
- Which core parts make the smallest first release a landlord and renter could actually use?
- How hard is it to add another country's rule pack, and who keeps the New Zealand pack current when the law changes?
- Does anything in the demo look or read like an incumbent's product? It must be our own design and our own words.
- Is a renter ever charged anything connected to getting, keeping or renewing a tenancy, including a credit check? They must not be.
- Does any software or agent score, rank or decline applicants? A person must decide, against criteria set beforehand.
- Do tenants know that Tenancy Services will email or text them directly about bond refunds, and can they tell those messages from scams?
- Is Spare Rent a reporting platform under IRD's digital platform rules, and if so is the report filed by 7 February?
- Can a tenant, co-tenant or guarantor who never joins still sign, get notices, see reports and report repairs?
- When a rule or clock turns out wrong, who finds the affected tenancies, who tells both sides, and what cover pays for losses?
- Has the kit taken on the parts Spare Rent shares with other programs: the property record, document vault, tenancy rules, formal notices and service, bank feeds and text recognition?
- Are there work-board pieces for the Tenancy Bond API research and for the parts of RT01 that need their own piece or a lawyer?

## Sources

Researched on 2026-10-18 and 2026-10-19. Not legal advice; the research parts above check each point before it is built as a rule.

- [Tenancy Services: new pet laws and Bond Hub launch](https://www.tenancy.govt.nz/about-tenancy-services/news/new-pet-laws-and-bond-hub-launch/)
- [Tenancy Services: faster, simpler bond management is now live (29 June 2026)](https://www.tenancy.govt.nz/about-tenancy-services/news/faster-simpler-bond-management-is-now-live/)
- [Tenancy Services: using property management software for bond transactions](https://www.tenancy.govt.nz/rent-bond-and-bills/bond/using-property-management-software-to-complete-bond-transactions/)
- [MBIE API portal: Tenancy Bond API (subscription, three-legged OAuth, RealMe consent)](https://portal.api.business.govt.nz/api/tenancy-bond)
- [MBIE Tenancy Bond API guidelines (API Access Agreement)](https://portal.api.business.govt.nz/content/Tenancy%20Bond%20API%20Guidelines%20v2%20October%202025.pdf)
- [Tenancy Services: changes to tenancy terminations from 30 January 2025](https://www.tenancy.govt.nz/about-tenancy-services/news/changes-to-tenancy-terminations-now-in-force/)
- [Tenancy Services: Residential Tenancies Amendment Act 2024 changes (retaliatory notices, pets)](https://tenancy.govt.nz/law-changes)
- [Tenancy Services: letting fees and key money](https://tenancy.govt.nz/rent-bond-and-bills/letting-fees-and-key-money)
- [Tenancy Services: dealing with anti-social behaviour](https://www.tenancy.govt.nz/disputes/breaches-of-the-residential-tenancies-act/dealing-with-anti-social-behaviour/)
- [Tenancy Services: FastTrack Resolution](https://www.tenancy.govt.nz/disputes/fasttrack-resolution/)
- [Tenancy Services: repairs after a natural disaster](https://www.tenancy.govt.nz/maintenance-and-inspections/repairs-and-damages/repairs-after-a-natural-disaster/)
- [Tenancy Services: change of landlord (landlords away more than 21 days)](https://www.tenancy.govt.nz/ending-a-tenancy/change-of-landlord-or-tenant/change-of-landlord/)
- [Tenancy Services: Healthy Homes compliance statement](https://tenancy.govt.nz/healthy-homes/compliance-statement)
- [HUD: updates to the Healthy Homes standards (new heating formula for post-2008 homes and some apartments)](https://www.hud.govt.nz/news/updates-to-the-healthy-homes-standards-takes-effect-today)
- [Tenancy Services: pre-tenancy applications](https://www.tenancy.govt.nz/starting-a-tenancy/new-to-tenancy/pre-tenancy-applications/)
- [Privacy Commissioner: Privacy Act guidance for landlords (February 2025)](https://privacy.org.nz/assets/New-order/Resources-/Publications/Guidance-resources/Rental-guidance/2025_02_14-Privacy-Act-guidance-for-landlords-A1050761.pdf)
- [Bell Gully: IPP 3A in force from 1 May 2026](https://www.bellgully.com/insights/preparing-for-ipp-3a-new-requirements-effective-1-may-2026/)
- [Tenancy Services: Residential Tenancies Amendment Act 2020 changes (rent rises, bidding)](https://tenancy.govt.nz/about-tenancy-services/news/residential-tenancies-amendment-act-2020-further-changes-now-in-force)
- [IRD: digital platform information](https://ird.govt.nz/international-tax/exchange-of-information/digital-platform-information)
- [DIA: anti-money-laundering information for online marketplaces](https://www.dia.govt.nz/AML-CFT-Information-for-Online-Marketplaces)
- [Property Brokers: property management costs in New Zealand](https://www.propertybrokers.co.nz/news/how-much-does-property-management-cost-in-new-zealand)
