# Spare Ride: what needs to be built

*Stage: Mapping. Machine-readable version: [features.json](features.json). Map: [ARCHITECTURE.md](ARCHITECTURE.md). Work board: [STATUS.md](STATUS.md).*

Spare Ride replaces Uber, and the taxi apps and phone booking lines people use today. This map lists everything a full replacement needs, from the top down: 23 areas, 249 features a person can use, and 480 parts someone can build. It follows every journey a rider, a driver, a business, the co-op's staff and its members take: first sign-up, daily use, payments, safety, accessibility, poor signal, other languages and countries, data export, disputes and the law. Features come from what people need to do, compared with what the incumbents offer, never copied from their interfaces. Core features (150) make the first usable release in one city (in the first example country, New Zealand), next (62) follow soon after, and later (37) wait until the service is running. Spare Ride keeps only its own rules: where the [shared kit](../kit/FEATURES.md) already maps a part, the Ride part depends on that kit part by its ID and adds only what rides need. Each part links to a work-board piece where one already covers it. IDs are permanent: a gap in the numbering is a part that moved to the kit or merged into another part, never something missing.

## Areas

| ID | Area | Features | Core | Parts |
| --- | --- | --- | --- | --- |
| RIDE-A1 | [Rider accounts and getting started](#ride-a1-rider-accounts-and-getting-started) | 9 | 6 | 20 |
| RIDE-A2 | [Driver sign-up and checks](#ride-a2-driver-sign-up-and-checks) | 16 | 12 | 41 |
| RIDE-A3 | [Booking a ride](#ride-a3-booking-a-ride) | 14 | 7 | 26 |
| RIDE-A4 | [More ways to ride](#ride-a4-more-ways-to-ride) | 8 | 0 | 19 |
| RIDE-A5 | [The live trip](#ride-a5-the-live-trip) | 9 | 7 | 20 |
| RIDE-A6 | [The driver's working day](#ride-a6-the-drivers-working-day) | 13 | 8 | 28 |
| RIDE-A7 | [Dispatch and matching](#ride-a7-dispatch-and-matching) | 11 | 7 | 21 |
| RIDE-A8 | [Fares and pricing](#ride-a8-fares-and-pricing) | 9 | 7 | 20 |
| RIDE-A9 | [Payments, tips and payouts](#ride-a9-payments-tips-and-payouts) | 13 | 8 | 19 |
| RIDE-A10 | [Tax and earnings records](#ride-a10-tax-and-earnings-records) | 7 | 4 | 15 |
| RIDE-A11 | [Maps, routing and navigation](#ride-a11-maps-routing-and-navigation) | 10 | 6 | 14 |
| RIDE-A12 | [Messages, calls and notifications](#ride-a12-messages-calls-and-notifications) | 9 | 6 | 11 |
| RIDE-A13 | [Safety](#ride-a13-safety) | 16 | 8 | 30 |
| RIDE-A14 | [Accessibility](#ride-a14-accessibility) | 15 | 7 | 27 |
| RIDE-A15 | [Ratings, reports, disputes and moderation](#ride-a15-ratings-reports-disputes-and-moderation) | 9 | 5 | 20 |
| RIDE-A16 | [Help, support and lost items](#ride-a16-help-support-and-lost-items) | 9 | 7 | 15 |
| RIDE-A17 | [Businesses, families, taxis and fleets](#ride-a17-businesses-families-taxis-and-fleets) | 7 | 0 | 20 |
| RIDE-A18 | [Co-op membership, voting and open costs](#ride-a18-co-op-membership-voting-and-open-costs) | 9 | 5 | 15 |
| RIDE-A19 | [Operations and admin](#ride-a19-operations-and-admin) | 15 | 11 | 24 |
| RIDE-A20 | [Legal duties and compliance](#ride-a20-legal-duties-and-compliance) | 12 | 11 | 23 |
| RIDE-A21 | [Privacy and data export](#ride-a21-privacy-and-data-export) | 12 | 10 | 22 |
| RIDE-A22 | [Languages, countries and poor signal](#ride-a22-languages-countries-and-poor-signal) | 8 | 4 | 12 |
| RIDE-A23 | [Apps, platform and open access](#ride-a23-apps-platform-and-open-access) | 9 | 4 | 18 |

**Reading the tables.** Priority: core is needed for a first usable release, next comes soon after, later waits. Kind: library (shared code), service (runs on a server), interface (screens), data (records and settings), integration (a link to an outside system), research (a note to write first), legal (a duty the law sets). Depends on names kit crates (`kit-`), kit services (`svc-`), engines (`engine-`), parts of the [shared kit map](../kit/FEATURES.md) by ID (`KIT-`), or other Spare Ride parts by ID.

## RIDE-A1 Rider accounts and getting started

How a rider joins, signs in, sets up their profile and preferences, and closes their account, using the one Spareware login.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Sign up in a minute | A rider creates an account with a passkey, a phone code or an email and can book straight away. | core | Rider sign-up screens; Phone number check; Rider record; Registered passenger details; Minimum rider age |
| **F2** Sign in on any device | A rider signs in on a new phone or in a browser and finds their places, settings and history waiting. | core | Sign-in and recovery screens; Settings sync |
| **F3** Profile | A rider sets the name drivers see, an optional photo and their language. | core | Profile screen; Photo check |
| **F4** Saved places | A rider saves home, work and favourite places and picks them in one tap. | core | Saved places store; Saved places screen |
| **F5** Ride preferences | A rider sets standing preferences such as a quiet ride, help with bags, temperature and accessibility needs. | core | Rider preferences; Rider preferences screen |
| **F6** First ride guide | A new rider sees a short guide to the upfront fare, the at-cost fee and the safety tools before their first trip. | next | First ride guide |
| **F7** Change or close an account | A rider changes their details, pauses their account or closes it for good. | core | Account settings; Close account screen |
| **F8** Invite a friend | A rider shares an invite link so friends can join; paid sign-up bonuses exist only if members vote for them. | later | Ride invites |
| **F9** Visiting from another country | A visitor signs up with an overseas phone number or email, pays with a card from home, sees the higher overseas card fee before booking and uses the app in their own language. | next | Overseas sign-up; Overseas card fee; Visitor's language |

### Parts: Rider accounts and getting started

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A1-F1-P1 | **Rider sign-up screens**: Screens to sign up with the shared Spareware login, passkey first, phone code or email as fallback. | interface | medium | [KIT-A1-F1-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RIDE-A1-F1-P2 | **Phone number check**: Confirms the rider's phone with a one-time code so drivers and support can reach them. | integration | small | [KIT-A1-F1-P3](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RIDE-A1-F1-P3 | **Rider record**: The rider's profile, settings and status, stored as operations on their Spareware account. | data | small | `kit-core`, `kit-ops`, [KIT-A1-F5-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RIDE-A1-F1-P4 | **Registered passenger details**: Keeps the details the passenger service rules ask for, so every app rider counts as a registered passenger. | legal | small | RIDE-A1-F1-P3 | [RD01](STATUS.md) |
| RIDE-A1-F1-P5 | **Minimum rider age**: The youngest age a rider may hold an account, checked at sign-up on the kit's minimum age rule without collecting more than it needs. | legal | small | [KIT-A1-F1-P5](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), RIDE-A1-F1-P1 |  |
| RIDE-A1-F2-P1 | **Sign-in and recovery screens**: Passkey sign-in with a code fallback and a safe account recovery path. | interface | small | [KIT-A1-F2-P4](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A1-F4-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RIDE-A1-F2-P2 | **Settings sync**: Saved places, preferences and trip history follow the rider to every device. | library | small | `kit-sync`, RIDE-A1-F1-P3 |  |
| RIDE-A1-F3-P1 | **Profile screen**: Edit name, photo and language, with a preview of what a driver sees once they have accepted the trip, and never before. | interface | small | RIDE-A1-F1-P3 |  |
| RIDE-A1-F3-P2 | **Photo check**: Checks an uploaded photo shows a face and is not offensive before drivers see it, within the limits the biometrics assessment sets. | service | small | [KIT-A15-F7-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage), `engine-media`, RIDE-A21-F9-P1 |  |
| RIDE-A1-F4-P1 | **Saved places store**: A rider's named places with their exact pickup point, kept private to the rider. | data | small | RIDE-A1-F1-P3, [KIT-A7-F2-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A1-F4-P2 | **Saved places screen**: Add, rename, reorder and delete saved places. | interface | small | RIDE-A1-F4-P1 |  |
| RIDE-A1-F5-P1 | **Rider preferences**: The rider's standing preferences and accessibility needs, kept on their profile. | data | small | RIDE-A1-F1-P3 |  |
| RIDE-A1-F5-P2 | **Rider preferences screen**: Simple toggles for each preference, with plain notes on what drivers see. | interface | small | RIDE-A1-F5-P1 |  |
| RIDE-A1-F6-P1 | **First ride guide**: Three or four short screens on the kit's first-run guide, which can be skipped and found again in help. | interface | small | RIDE-A1-F1-P3, [KIT-A20-F6-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RIDE-A1-F7-P1 | **Account settings**: Change phone, email, passkeys and language, with a list of signed-in devices. | interface | small | [KIT-A1-F3-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A20-F5-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RIDE-A1-F7-P2 | **Close account screen**: Pause or close the account from settings, showing any balance, ride credit or open dispute first, then hand over to Ride's deletion rules. | interface | small | RIDE-A21-F2-P1, [KIT-A20-F5-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RIDE-A1-F8-P1 | **Ride invites**: Invite links that open Spare Ride's sign-up, on the kit's invites, with any sign-up bonus set only by a members' vote. | service | small | [KIT-A14-F7-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RIDE-A1-F9-P1 | **Overseas sign-up**: Sign-up with an overseas phone number or an email, offering email or a passkey first and limiting texted codes to overseas numbers with the kit's text cost guard against SMS pumping. | service | small | [KIT-A1-F1-P3](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A8-F4-P4](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A1-F1-P1 |  |
| RIDE-A1-F9-P2 | **Overseas card fee**: Shows the higher payment fee on overseas cards, passed through at cost, in the fare breakdown before the rider books. | library | small | RIDE-A8-F3-P1, RIDE-A8-F3-P5, [KIT-A3-F4-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A1-F9-P3 | **Visitor's language**: Opens the app in the phone's language where a translation exists, with the languages most visitors to each country speak translated first. | interface | small | [KIT-A19-F1-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), RIDE-A1-F3-P1 |  |

## RIDE-A2 Driver sign-up and checks

How a person becomes a driver: applying, proving their licence, identity, vehicle and insurance, setting up payouts, joining the co-op and staying checked.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Apply to drive | A person applies to drive from a phone or a browser, sees every requirement up front and tracks their progress. | core | Application screens; Application record; Progress tracker; Driver requirements list |
| **F2** Licence and passenger endorsement check | An applicant proves they hold a full driver licence and any passenger endorsement their country asks for (example: New Zealand's P endorsement), checked against the official record. | core | Licence check; Endorsement rules; Drivers with their own operator licence |
| **F3** Identity check | An applicant proves who they are with photo ID and a live selfie, checked by a regulated verification provider. | core | Driver identity step; Verification provider choice |
| **F4** Vehicle check | An applicant adds their vehicle and its registration, Certificate of Fitness, age and seats are checked. | core | Vehicle record; Vehicle photos; Vehicle rules; Licence label per vehicle |
| **F5** Insurance | An applicant shows their vehicle is insured for carrying passengers, or joins cover the co-op arranges. | core | Insurance options; Insurance proof |
| **F6** Payout and tax details | An applicant adds a bank account for payouts, their tax number (example: New Zealand's IRD number) and whether they are registered for sales tax. | core | Payout account set-up; Tax details; Driver account protection |
| **F7** Join the co-op | An applicant becomes a member of the co-op for free and agrees to the driver agreement written in plain words. | core | Driver member step; Driver agreement; Agreement in drivers' languages; Non-discrimination commitment; Signing the agreement |
| **F8** Orientation | A new driver completes a short orientation on safety, accessibility, assistance animals, work time limits and the app before their first trip. | core | Orientation content; Orientation course; Children riding alone |
| **F9** Reviewer approval | A co-op reviewer checks anything the automatic checks cannot and approves or explains a refusal. | core | Review hand-off; Refusal reasons |
| **F10** Expiry reminders and re-checks | A driver is reminded before any document expires and is paused, not removed, if a check lapses. | core | Expiry reminders; Ongoing licence checks |
| **F11** More than one vehicle | A driver adds several vehicles and picks which one they drive today. | next | Vehicle switcher |
| **F12** Stop driving and leave the co-op | A driver who stops driving gets their last payout, settles any balance either way, has their share repaid and takes their record with them. | core | Driver exit; Member role on leaving; Take my driving record; Unclaimed payouts |
| **F13** Help getting a passenger endorsement | A person who wants to drive but has no passenger endorsement follows a step-by-step guide and tracks each step. | next | Passenger endorsement guide |
| **F14** Right to work checked | An applicant on a visa shows they may drive in the country they apply in, and is checked again before their visa lapses. | core | Driver visa check; Visa conditions for driving |
| **F15** Bring in drivers and help each other | A driver invites people to drive and talks with other drivers in a forum. | next | Driver referral; Driver forum |
| **F16** Cover for lost income | A driver joins an optional group policy, arranged by the co-op at cost, that pays part of their income when sickness or injury stops them driving. | later | Income protection option |

### Parts: Driver sign-up and checks

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A2-F1-P1 | **Application screens**: A step-by-step application that saves as it goes and can be finished later. | interface | medium | [KIT-A1-F5-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RIDE-A2-F1-P2 | **Application record**: Each application and every check result, stored as operations so the history is clear. | data | small | `kit-core`, `kit-ops`, [KIT-A2-F1-P3](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A2-F1-P3 | **Progress tracker**: Shows which checks are done, waiting or failed, and what to do next. | interface | small | RIDE-A2-F1-P2 |  |
| RIDE-A2-F1-P4 | **Driver requirements list**: Every licence, check and document a driver needs in each country, starting with New Zealand, with the source for each. | research | medium |  | [RD01](STATUS.md) |
| RIDE-A2-F2-P1 | **Licence check**: Checks licence class, status and endorsements against the official register through an approved verification route. | integration | medium | [KIT-A2-F2-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), RIDE-A2-F1-P2 | [RD01](STATUS.md) |
| RIDE-A2-F2-P2 | **Endorsement rules**: Encodes what a valid passenger endorsement is in each country pack (example: New Zealand's P endorsement) and when it lapses, so the app never lets an unendorsed driver online. | legal | small | RIDE-A2-F1-P4 | [RD01](STATUS.md) |
| RIDE-A2-F2-P3 | **Drivers with their own operator licence**: Lets drivers who hold their own operator licence (example: New Zealand's small passenger service licence) record it, which changes what records they keep. | data | small | RIDE-A2-F1-P2 | [RD01](STATUS.md) |
| RIDE-A2-F3-P1 | **Driver identity step**: Places the kit's identity check in the driver application and keeps the verified photo for later right-driver checks, as the biometrics assessment allows. | integration | small | [KIT-A2-F1-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), RIDE-A21-F9-P1 |  |
| RIDE-A2-F3-P2 | **Verification provider choice**: Which of the kit's identity providers can also run the live selfie match drivers need, and on what terms. | research | small | [KIT-A2-F1-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A2-F4-P1 | **Vehicle record**: Each vehicle's plate, make, model, colour, seats, fuel type, equipment and document expiries. | data | small | `kit-core`, `kit-ops`, [KIT-A2-F6-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A2-F4-P3 | **Vehicle photos**: The applicant uploads photos of the vehicle and plate for a reviewer to check. | interface | small | `kit-store`, `engine-media` |  |
| RIDE-A2-F4-P4 | **Vehicle rules**: Decides which ride options a vehicle can offer from its seats, age, fuel type and equipment, set per city. | library | small | RIDE-A2-F4-P1 |  |
| RIDE-A2-F4-P5 | **Licence label per vehicle**: Where the country asks for one, issues each approved vehicle a licence label under the co-op's licence (example: New Zealand's small passenger service licence label), records which label is in which vehicle, checks a photo that it is in the front windscreen above the registration label, and takes it back when the vehicle leaves. | data | small | RIDE-A2-F4-P1, RIDE-A2-F4-P3, RIDE-A20-F1-P2 | [RD01](STATUS.md) |
| RIDE-A2-F5-P1 | **Insurance options**: What cover passengers, drivers and the co-op need and who offers it in each country, starting with New Zealand. | research | medium |  | [RD02](STATUS.md) |
| RIDE-A2-F5-P2 | **Insurance proof**: Upload of the policy with its expiry date, checked by a reviewer. | interface | small | RIDE-A2-F1-P2 | [RD02](STATUS.md) |
| RIDE-A2-F6-P1 | **Payout account set-up**: Puts the kit's payout set-up into the driver application, so bank details go to the regulated payment provider and never to Spareware. | integration | small | [KIT-A3-F3-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) | [K02](../kit/STATUS.md) |
| RIDE-A2-F6-P2 | **Tax details**: Tax number and sales tax status, stored so the sales tax engine and income reports are correct. | data | small | RIDE-A2-F1-P2, [KIT-A4-F3-P2](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), [KIT-A4-F1-P2](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) |  |
| RIDE-A2-F6-P3 | **Driver account protection**: Every driver uses a second sign-in step, and a change to payout bank details waits behind the kit's payout change guard. | service | small | [KIT-A1-F3-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A3-F3-P4](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A2-F6-P1 |  |
| RIDE-A2-F7-P1 | **Driver member step**: Folds the co-op's member sign-up into the application. | interface | small | [KIT-A14-F1-P3](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), RIDE-A18-F1-P1 | [K01](../kit/STATUS.md) |
| RIDE-A2-F7-P2 | **Driver agreement**: The agreement between driver and co-op, on the kit's contractor agreements if drivers are contractors, written once the status decision is made. | legal | medium | RIDE-A20-F5-P1, [KIT-A23-F1-P3](../kit/FEATURES.md#kit-a23-work-law-and-worker-status) | [RD01](STATUS.md) |
| RIDE-A2-F7-P3 | **Agreement in drivers' languages**: Plain translations of the driver agreement in the languages many drivers speak, so every driver understands what they sign, which the unfair contract terms rules for small trade contracts weigh. | legal | small | RIDE-A2-F7-P2, [KIT-A19-F1-P3](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A18-F8-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RIDE-A2-F7-P4 | **Non-discrimination commitment**: Each driver accepts the kit's non-discrimination commitment at sign-up, written for rides: never refusing, cancelling on or rating down a rider on any ground human rights law protects (example: New Zealand's Human Rights Act 1993). | legal | small | [KIT-A10-F19-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A2-F7-P2 |  |
| RIDE-A2-F7-P5 | **Signing the agreement**: The driver signs the agreement with the kit's signatures and audit trail and keeps a signed copy, so there is a written agreement as the gateway test asks. | service | small | [KIT-A25-F1-P2](../kit/FEATURES.md#kit-a25-signatures-and-training), RIDE-A2-F7-P2 |  |
| RIDE-A2-F8-P1 | **Orientation content**: Short lessons and a quiz on safety, accessibility, assistance animals, work time limits, caring for intoxicated or vulnerable riders, spotting human trafficking and the app, in the example country's languages first (for New Zealand: English and te reo Māori), then in the languages many drivers speak, such as Hindi, Punjabi, Mandarin, Samoan and Tongan. | data | medium | [KIT-A19-F1-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-and-training) |  |
| RIDE-A2-F8-P2 | **Orientation course**: Runs the orientation as a course on the kit's training and certificates, and unlocks going online once it is passed. | interface | small | [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-and-training), [KIT-A25-F2-P2](../kit/FEATURES.md#kit-a25-signatures-and-training), RIDE-A2-F8-P1, RIDE-A2-F1-P2 |  |
| RIDE-A2-F8-P3 | **Children riding alone**: What a driver does when a child turns up to ride alone before teen accounts exist: decline with no penalty, tell the account holder and the safety team through the app, and never drive off leaving the child somewhere unsafe. | data | small | RIDE-A2-F8-P1, RIDE-A7-F11-P2, RIDE-A1-F1-P5 |  |
| RIDE-A2-F9-P1 | **Review hand-off**: Sends applications that need a person to the document review queue with every check result attached. | service | small | RIDE-A2-F1-P2, RIDE-A19-F3-P1 |  |
| RIDE-A2-F9-P2 | **Refusal reasons**: Every refusal names the rule it rests on and how to fix it or appeal. | data | small | [KIT-A10-F8-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A2-F4-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A2-F10-P2 | **Expiry reminders**: Push, email and text reminders at set times before expiry. | service | small | [KIT-A2-F3-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RIDE-A2-F10-P3 | **Ongoing licence checks**: Re-checks licence and endorsement status on a schedule the law allows. | integration | small | RIDE-A2-F2-P1 | [RD01](STATUS.md) |
| RIDE-A2-F11-P1 | **Vehicle switcher**: Choose today's vehicle before going online; only checked vehicles appear. | interface | small | RIDE-A2-F4-P1 |  |
| RIDE-A2-F12-P1 | **Driver exit**: Ends the driver role, makes the last payout, settles any money owed either way and keeps tax statements available after they leave. | service | small | RIDE-A9-F10-P1, RIDE-A9-F13-P2, [KIT-A3-F3-P2](../kit/FEATURES.md#kit-a3-payments-and-payouts), [KIT-A4-F4-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) |  |
| RIDE-A2-F12-P2 | **Member role on leaving**: Ends the driver's member role in Spare Ride when they leave, unless they choose to stay a member. | service | small | [KIT-A14-F8-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), RIDE-A18-F1-P1 |  |
| RIDE-A2-F12-P3 | **Take my driving record**: Exports the driver's ratings, trip count and checks passed as a signed file another platform or a lender can verify. | library | small | [KIT-A9-F7-P1](../kit/FEATURES.md#kit-a9-ratings-and-reputation), RIDE-A21-F1-P1 |  |
| RIDE-A2-F12-P4 | **Unclaimed payouts**: Payouts that cannot be delivered after a driver leaves, and dormant ride credit, are recorded and, if the co-op rather than the payment provider holds them, paid to the tax office under unclaimed money law (example: New Zealand's Inland Revenue and Unclaimed Money Act 1971), which is unverified. | legal | small | RIDE-A2-F12-P1, [KIT-A3-F16-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) | [K02](../kit/STATUS.md) |
| RIDE-A2-F13-P1 | **Passenger endorsement guide**: Steps for the medical, the fit and proper check and the application, with links to the official forms and a progress tracker. | interface | small | RIDE-A2-F1-P4, RIDE-A2-F1-P3 | [RD01](STATUS.md) |
| RIDE-A2-F14-P1 | **Driver visa check**: Runs the kit's visa entitlement check in the application, records the entitlement and its expiry, and checks again before it lapses, pausing offers rather than removing the driver. | integration | small | [KIT-A2-F11-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), RIDE-A2-F1-P2 |  |
| RIDE-A2-F14-P2 | **Visa conditions for driving**: Which visa conditions allow driving for Spare Ride as a contractor or as a co-op employee, so a visa tied to another employer or one that bars self-employment is caught. | legal | small | [KIT-A2-F11-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), RIDE-A20-F5-P1 | [RD01](STATUS.md) |
| RIDE-A2-F15-P1 | **Driver referral**: Invite links to apply to drive, with any referral reward set only by a members' vote. | service | small | [KIT-A14-F7-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), RIDE-A2-F1-P1 |  |
| RIDE-A2-F15-P2 | **Driver forum**: A Spare Ride space on the kit's member forum where drivers help each other and raise ideas for votes. | service | small | [KIT-A21-F6-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RIDE-A2-F16-P1 | **Income protection option**: Whether insurers offer drivers an optional group income protection policy at cost, what it covers and what it costs. | research | small | RIDE-A2-F5-P1 | [RD02](STATUS.md) |

## RIDE-A3 Booking a ride

Everything a rider does before the car is on its way: where to, where from, which kind of car, the upfront fare, booking now or later, and cancelling.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Where to | A rider types or picks a destination and sees matching addresses and places as they type. | core | Destination search box; Recent places |
| **F2** Pickup point | A rider confirms where to be picked up from their location or a pin, snapped to a safe, legal stopping spot. | core | Pickup pin; Location permission handling; Walk to the pickup |
| **F3** Choose a ride option | A rider compares ride options such as standard, larger, premium, electric, pet-friendly, wheelchair accessible and taxi, each with fare and wait time. | core | Ride option catalogue; Ride option list; Passengers and bags |
| **F4** Upfront fare and where it goes | A rider sees the full fare before booking and how it splits between driver, running cost, payment fee and sales tax. | core | Fare card; Quote hold |
| **F5** Book now | A rider confirms and the request goes out to drivers straight away. | core | Ride request command; Ride state machine; Booking screen |
| **F6** Note for the driver | A rider adds a pickup note such as a gate code or which door. | core | Pickup note |
| **F7** Schedule ahead | A rider books a ride for a set time up to a set number of days ahead. | next | Scheduled booking record; Schedule picker; Hold timing for scheduled rides |
| **F8** Several stops | A rider adds one or more stops on the way, each shown to the driver in order. | next | Multi-stop routing; Stops editor |
| **F9** Book for someone else | A rider books for a friend or family member, who gets the driver and car details by text. | next | Guest rider details; Guest trip text |
| **F10** Cancel a booking | A rider cancels free inside a published window, and sees any fee before confirming a later cancellation. | core | Cancel flow |
| **F11** Ask for the same driver again | A rider asks a driver they liked for a future booking, and the driver chooses whether to take it. | later | Favourite drivers |
| **F12** Move the pickup point | A rider moves their pickup point a short way before the car arrives, and the driver is told at once. | next | Pickup change |
| **F13** Check a fare without an account | Anyone enters two places and sees the upfront fare and where every dollar goes, without signing up. | next | Public fare estimator |
| **F14** Wait longer, pay less | A rider who can wait longer for a driver picks a cheaper option. | later | Wait and save option |

### Parts: Booking a ride

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A3-F1-P1 | **Destination search box**: Search-as-you-type over addresses, places and saved places, with recent trips first. | interface | medium | [KIT-A7-F2-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A11-F2-P3, RIDE-A1-F4-P1 |  |
| RIDE-A3-F1-P2 | **Recent places**: The rider's last destinations, kept on the account and cleared when they ask. | data | small | RIDE-A1-F1-P3 |  |
| RIDE-A3-F2-P1 | **Pickup pin**: A draggable pin on the map that shows the suggested stopping spot and a short description. | interface | medium | RIDE-A11-F1-P1, RIDE-A11-F5-P2 |  |
| RIDE-A3-F2-P2 | **Location permission handling**: Asks for location only while booking, explains why, and works by typing if refused. | library | small | [KIT-A20-F2-P3](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) | [K04](../kit/STATUS.md) |
| RIDE-A3-F2-P3 | **Walk to the pickup**: Shows the rider a walking route, distance and time from where they are to the pickup spot the app moved them to. | interface | small | RIDE-A3-F2-P1, [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A3-F3-P1 | **Ride option catalogue**: The list of ride options per city, each with its vehicle rules and fare formula. | data | small | RIDE-A2-F4-P4, RIDE-A8-F1-P2 |  |
| RIDE-A3-F3-P2 | **Ride option list**: Shows each option with its upfront fare, wait time and seats, and hides options with no driver nearby. | interface | small | RIDE-A3-F3-P1, RIDE-A7-F2-P1, RIDE-A8-F2-P1 |  |
| RIDE-A3-F3-P3 | **Passengers and bags**: The rider says how many people and bags are coming; options without enough seats or space are hidden and the driver sees the numbers on the offer. | interface | small | RIDE-A3-F3-P1, RIDE-A2-F4-P4 |  |
| RIDE-A3-F4-P1 | **Fare card**: Shows the upfront fare and taps open to show the breakdown and the formula version used. | interface | small | RIDE-A8-F2-P1, RIDE-A8-F3-P1 |  |
| RIDE-A3-F4-P2 | **Quote hold**: Keeps a quoted fare valid for a few minutes so it does not change while the rider decides. | library | small | RIDE-A8-F2-P1 |  |
| RIDE-A3-F5-P1 | **Ride request command**: The request as one command on the marketplace loop, so people, AI and scripts book the same way. | library | medium | `kit-market`, `kit-ops`, [KIT-A5-F2-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A23-F4-P1 |  |
| RIDE-A3-F5-P2 | **Ride state machine**: The states a ride moves through, from requested to matched, arriving, on trip, completed or cancelled. | library | medium | `kit-market`, `kit-ops`, [KIT-A5-F4-P1](../kit/FEATURES.md#kit-a5-marketplace-engine) |  |
| RIDE-A3-F5-P3 | **Booking screen**: Confirm, then a clear searching state with the option to cancel. | interface | small | RIDE-A3-F5-P1 |  |
| RIDE-A3-F6-P1 | **Pickup note**: A short note sent with the request and shown on the driver's offer and pickup screen. | interface | small | RIDE-A3-F5-P1 |  |
| RIDE-A3-F7-P1 | **Scheduled booking record**: The scheduled ride with its pickup time, fare and reminders. | data | small | RIDE-A3-F5-P2 |  |
| RIDE-A3-F7-P2 | **Schedule picker**: Pick a date and time, see the fare and the latest time to cancel free. | interface | small | RIDE-A3-F7-P1 |  |
| RIDE-A3-F7-P3 | **Hold timing for scheduled rides**: Checks the card when a ride is booked ahead and places the hold close to pickup time, so it never runs past the card's hold limit. | library | small | RIDE-A3-F7-P1, RIDE-A9-F2-P1, [KIT-A3-F2-P4](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A3-F8-P1 | **Multi-stop routing**: Routes and prices a trip through several stops in order. | library | medium | [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A8-F2-P1 |  |
| RIDE-A3-F8-P2 | **Stops editor**: Add, reorder and remove stops before or during booking. | interface | small | RIDE-A3-F8-P1 |  |
| RIDE-A3-F9-P1 | **Guest rider details**: Name and phone of the person riding, used only for this trip. | data | small | RIDE-A3-F5-P2, RIDE-A20-F2-P4, RIDE-A21-F10-P1 |  |
| RIDE-A3-F9-P2 | **Guest trip text**: Sends the guest a text with the driver, car and a live trip link, and a short notice of the details shared about them. | service | small | RIDE-A12-F4-P1, RIDE-A13-F2-P1, RIDE-A21-F10-P2 |  |
| RIDE-A3-F10-P1 | **Cancel flow**: Shows whether cancelling is free or what it costs, then cancels in one tap. | interface | small | RIDE-A8-F7-P1 |  |
| RIDE-A3-F11-P1 | **Favourite drivers**: A rider's list of drivers to offer scheduled trips to first, on the kit's favourites, which drivers can switch off. | library | small | [KIT-A5-F12-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A7-F6-P1, RIDE-A7-F5-P1 |  |
| RIDE-A3-F12-P1 | **Pickup change**: Moves the pickup within a set distance, tells and re-routes the driver, and re-quotes the fare only if the change is large. | library | small | RIDE-A3-F2-P1, RIDE-A8-F6-P1, RIDE-A12-F3-P2 |  |
| RIDE-A3-F13-P1 | **Public fare estimator**: A public page that quotes a trip and shows its breakdown, with limits that stop it being used to track drivers. | interface | small | RIDE-A8-F2-P1, RIDE-A8-F3-P1, RIDE-A21-F11-P2 |  |
| RIDE-A3-F14-P1 | **Wait and save option**: A ride option with a longer wait and a lower fare, with the discount and the wait set by the drivers' vote. | library | small | RIDE-A3-F3-P1, RIDE-A8-F1-P2, RIDE-A18-F2-P2 | [RD06](STATUS.md) |

## RIDE-A4 More ways to ride

Trip types beyond one rider going from A to B: shared rides, hire by the hour, parcels, airports and events, long trips between towns, links to public transport and child seats.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Airport and event pickups | A rider at an airport or a big event goes to a clear pickup zone, and drivers wait in a fair first-in, first-out queue. | next | Pickup queue; Airport access and fees; Event pickup zones |
| **F2** Shared rides | Riders going the same way share a car and each pays less. | later | Pooling methods; Shared ride rules; Shared ride fare; Shared trip screens |
| **F3** Hire by the hour | A rider books a car and driver for a number of hours with several stops. | later | Hourly fare rule; Hourly booking screen |
| **F4** Send a parcel | A person sends a small parcel with a driver, with no passenger, and the recipient confirms with a code. | later | Parcel rules; Parcel booking; Handover code |
| **F5** Long trips between towns | A rider books a trip to another town with a fare that fairly covers the driver's return. | later | Long trip fare; Long trip work time check |
| **F6** Ride to public transport | A rider plans a trip that combines a short ride with a bus, train or ferry. | later | Public transport timetables; Combined trip suggestions |
| **F7** Child car seats | A rider travelling with a young child asks for a car with the right child seat. | later | Child seat rules; Child seat equipment |
| **F8** Same fare for my regular trip | A rider who makes the same trip often fixes its fare for a period, with no subscription charge. | later | Regular trip fare |

### Parts: More ways to ride

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A4-F1-P1 | **Pickup queue**: A first-in, first-out driver queue for a zone, with rules drivers can read. | library | medium | [KIT-A6-F3-P3](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), RIDE-A11-F6-P1 |  |
| RIDE-A4-F1-P2 | **Airport access and fees**: What each airport charges and requires of ride-hailing drivers, and the agreements needed. | research | small |  |  |
| RIDE-A4-F1-P3 | **Event pickup zones**: Temporary pickup zones and queues set up by the operations team for big events. | data | small | RIDE-A11-F6-P1, RIDE-A4-F1-P1 |  |
| RIDE-A4-F2-P1 | **Pooling methods**: How proven open projects and papers match shared rides, and whether riders in the first cities would use them. | research | medium |  |  |
| RIDE-A4-F2-P2 | **Shared ride rules**: Ride's detour limits, seat counts and pickup order for grouping requests with the kit's batching. | library | medium | [KIT-A6-F7-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A4-F2-P1 |  |
| RIDE-A4-F2-P3 | **Shared ride fare**: Splits the fare between riders fairly, set by the drivers' vote. | library | small | RIDE-A8-F1-P2 |  |
| RIDE-A4-F2-P4 | **Shared trip screens**: Show riders and the driver who is picked up and dropped off in what order. | interface | medium | RIDE-A4-F2-P2 |  |
| RIDE-A4-F3-P1 | **Hourly fare rule**: A fare per hour with included kilometres, set by vote. | library | small | RIDE-A8-F1-P2 |  |
| RIDE-A4-F3-P2 | **Hourly booking screen**: Pick start time, hours and rough plan; extend while the hire is running. | interface | small | RIDE-A4-F3-P1, RIDE-A3-F7-P1 |  |
| RIDE-A4-F4-P1 | **Parcel rules**: What items drivers may carry, insurance, and whether the passenger service licence covers parcels. | legal | small |  | [RD02](STATUS.md) |
| RIDE-A4-F4-P2 | **Parcel booking**: Book a parcel trip with sender and recipient details and a size. | interface | small | RIDE-A3-F5-P1 |  |
| RIDE-A4-F4-P3 | **Handover code**: The recipient gives the driver a code, using the kit's handover codes, so delivery is confirmed. | library | small | [KIT-A5-F7-P2](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A5-F3-P2 |  |
| RIDE-A4-F5-P1 | **Long trip fare**: A fare rule for long trips that can include part of the driver's return, set by vote. | library | small | RIDE-A8-F1-P2 |  |
| RIDE-A4-F5-P2 | **Long trip work time check**: Applies the stricter work time limits to fares over 100 km before the offer is made. | library | small | RIDE-A6-F6-P1 | [RD01](STATUS.md) |
| RIDE-A4-F6-P1 | **Public transport timetables**: Imports the open timetable feeds (GTFS) that transport agencies publish in each country, starting with New Zealand's, under their licence terms. | integration | medium |  |  |
| RIDE-A4-F6-P2 | **Combined trip suggestions**: Suggests a ride to the right stop to meet a service, with total time and cost. | interface | medium | RIDE-A4-F6-P1, RIDE-A7-F2-P1 |  |
| RIDE-A4-F7-P1 | **Child seat rules**: What the law requires for children in passenger service vehicles and what drivers may supply. | research | small |  | [RD01](STATUS.md) |
| RIDE-A4-F7-P2 | **Child seat equipment**: Records child seats fitted to a vehicle and matches them to requests. | data | small | RIDE-A2-F4-P1, RIDE-A7-F5-P1 |  |
| RIDE-A4-F8-P1 | **Regular trip fare**: A fixed fare for a regular route and time window, set by the drivers' vote and charged per trip with no monthly fee. | library | small | RIDE-A8-F1-P2, [KIT-A5-F9-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A18-F2-P2 | [RD06](STATUS.md) |

## RIDE-A5 The live trip

From the moment a driver accepts to the moment the rider gets out: who is coming, where they are, waiting, starting and ending the trip, changes on the way, and keeping going through lost signal.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Driver on the way | A rider sees the driver's name, photo, car, plate and rating, and watches the car arrive with a live arrival time. | core | Trip position sharing; Trip map; Driver card; Live arrival time |
| **F2** Arrival and waiting | The rider is told when the car arrives; a short free wait applies, then a published waiting charge. | core | Arrival detection; Wait timer; Arrival notice |
| **F3** Start and end the trip | The driver starts the trip after checking the rider and ends it at the destination; both see the same route and fare. | core | Trip commands; Trip record; Trip summary |
| **F4** Change the destination or add a stop | A rider changes where they are going during the trip and sees the new fare before it applies. | next | In-trip change command; In-trip change screen |
| **F5** Keeps going through lost signal | If either phone loses signal, the trip carries on and catches up when the signal is back. | core | Offline trip state; Route trace buffer; Trace reconciliation |
| **F6** No-show | A driver who waited the full time marks a no-show and is paid the published fee. | core | No-show rule |
| **F7** After the trip | A rider and driver each rate the trip, the rider can tip, and both get their receipt or earnings. | core | After-trip screen |
| **F8** Past trips and ride again | A rider or driver lists their past trips, opens any one for its map, fare, receipt and help, and books the same trip again in one tap. | core | Trip history; Ride again |
| **F9** A driver who speaks my language | A rider sees the languages their driver speaks and can prefer a driver who shares their language. | later | Driver languages |

### Parts: The live trip

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A5-F1-P1 | **Trip position sharing**: Decides which driver positions the rider sees, from acceptance to drop-off only, sent over the kit's live location stream. | library | small | [KIT-A7-F4-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A5-F1-P2 | **Trip map**: Shows the car, the route and the pickup point, updating smoothly. | interface | medium | RIDE-A11-F1-P1, RIDE-A5-F1-P1 |  |
| RIDE-A5-F1-P3 | **Driver card**: Name, photo, car, colour, plate and rating, with buttons to message, call and share the trip. | interface | small | RIDE-A2-F4-P1 |  |
| RIDE-A5-F1-P4 | **Live arrival time**: Recalculates the arrival time as the car moves. | library | small | RIDE-A7-F2-P1, RIDE-A5-F1-P1 |  |
| RIDE-A5-F2-P1 | **Arrival detection**: Marks the driver as arrived when they reach the pickup point or tap arrived. | library | small | [KIT-A7-F4-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A3-F5-P2 |  |
| RIDE-A5-F2-P2 | **Wait timer**: Counts free wait time, then waiting charges, shown to both sides, with a longer free wait and no charges for riders who need more time to board. | library | small | RIDE-A8-F1-P2, RIDE-A14-F12-P1 |  |
| RIDE-A5-F2-P3 | **Arrival notice**: Push and text when the car arrives and again before charges start. | service | small | [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A12-F4-P1 |  |
| RIDE-A5-F3-P1 | **Trip commands**: Start, end and no-show as commands on the marketplace loop. | library | medium | `kit-market`, `kit-ops`, RIDE-A3-F5-P2 |  |
| RIDE-A5-F3-P2 | **Trip record**: The full record of a trip: times, route trace, fare version, people and vehicle, kept as the rules require. | data | medium | `kit-core`, `kit-ops`, `kit-store`, [KIT-A7-F4-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) | [RD01](STATUS.md) |
| RIDE-A5-F3-P3 | **Trip summary**: Shows both sides the route, time, distance and final fare when the trip ends. | interface | small | RIDE-A5-F3-P2 |  |
| RIDE-A5-F4-P1 | **In-trip change command**: Ride's destination changes and added stops on the kit's order changes, with the driver told, the fare re-quoted and the hold raised. | library | small | [KIT-A5-F15-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A8-F6-P1, RIDE-A3-F8-P1 |  |
| RIDE-A5-F4-P2 | **In-trip change screen**: Edit destination or stops and confirm the new fare. | interface | small | RIDE-A5-F4-P1 |  |
| RIDE-A5-F5-P1 | **Offline trip state**: Keeps the trip state on the phone and replays it to the server when back online. | library | medium | `kit-sync`, RIDE-A5-F3-P1 |  |
| RIDE-A5-F5-P2 | **Route trace buffer**: Stores GPS points on the phone while offline so the distance and fare are still right. | library | small | RIDE-A5-F5-P1 |  |
| RIDE-A5-F5-P3 | **Trace reconciliation**: Rebuilds the distance from a patchy trace by matching it to roads. | library | medium | [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A5-F5-P2 |  |
| RIDE-A5-F6-P1 | **No-show rule**: When a no-show can be marked and what it costs, checked against the driver's location, and only once any longer wait a rider needs has passed. | library | small | RIDE-A8-F7-P1, RIDE-A5-F2-P1, RIDE-A14-F12-P1 |  |
| RIDE-A5-F7-P1 | **After-trip screen**: Rating, tip and a link to the receipt or earnings, all optional. | interface | small | [KIT-A9-F1-P1](../kit/FEATURES.md#kit-a9-ratings-and-reputation), RIDE-A9-F3-P1, RIDE-A9-F9-P1 |  |
| RIDE-A5-F8-P1 | **Trip history**: Past trips for riders and drivers with map, fare and receipt, the starting point for help, lost items, fare reviews and disputes. | interface | small | [KIT-A5-F8-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A5-F3-P2, RIDE-A21-F4-P1 |  |
| RIDE-A5-F8-P2 | **Ride again**: Books the same trip, or the trip back, in one tap at today's quoted fare. | library | small | RIDE-A5-F8-P1, RIDE-A3-F5-P1 |  |
| RIDE-A5-F9-P1 | **Driver languages**: Shows the languages a driver speaks on the driver card, and lets a rider prefer, but not require, a driver who shares their language. | library | small | RIDE-A5-F1-P3, RIDE-A7-F5-P1, RIDE-A1-F5-P1 |  |

## RIDE-A6 The driver's working day

What a driver does from going online to going home: choosing what they drive, seeing full offers, accepting or declining freely, breaks and work time limits, earnings and where riders are waiting.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Go online and offline | A driver goes online with one tap and offline whenever they like. | core | Online state; Location while online; Driver home screen |
| **F2** See the whole offer | A driver sees the pickup distance, destination, fare, the at-cost fee and their earnings before deciding. | core | Offer card; Offer alerts |
| **F3** Accept or decline with no penalty | A driver accepts or declines any offer and declining never lowers their standing or future offers. | core | Accept and decline commands; No-penalty rule |
| **F4** Choose where and what to drive | A driver sets areas, ride options, the longest pickup they will take, pets, shared rides and trip lengths. | core | Driver preferences; Driver preferences screen |
| **F5** Heading home | A driver asks for trips that head toward a chosen place, as often as they like. | next | Destination filter |
| **F6** Breaks and work time limits | A driver sees work time left, is warned before a limit, is stopped from taking offers when a rest is due, and their logbook keeps itself. | core | Passenger work time rules; Electronic logbook; Break reminders; Logbook export; Other work entries; Work time from other Spareware apps; Logbook approval |
| **F7** Where riders are waiting | A driver sees a map of where riders are waiting now, with no bonuses that push them around. | next | Demand map service; Demand map layer |
| **F8** Today's earnings | A driver sees what they earned today, this week and per trip, and what the fee paid for. | core | Earnings screen |
| **F9** What a trip really earns | A driver enters their vehicle costs and sees what each offer earns after fuel and wear. | next | Vehicle cost model; Earnings after costs |
| **F10** Next trip lined up | A driver near the end of a trip can be offered the next one, or switch this off. | next | Chained offers |
| **F11** Driver ID on screen | A driver shows their driver identification on screen when a rider, officer or venue asks. | core | Driver ID screen |
| **F12** Driving mode | While the car moves, the driver app shows only what can be read at a glance, reads offers and messages aloud and locks typing, so drivers keep within the mobile phone rule. | core | Phone rule for drivers; Moving lock; Read aloud |
| **F13** Drive by voice | A driver accepts or declines an offer, marks arrival and sends a set reply by voice, without touching the phone. | next | Voice commands for drivers |

### Parts: The driver's working day

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A6-F1-P1 | **Online state**: Ride's online, on trip and on break states, as operations on the kit's worker availability. | service | small | `kit-ops`, [KIT-A6-F2-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A6-F1-P2 | **Location while online**: Sends the driver's location every few seconds only while online, and never when offline. | library | medium | RIDE-A23-F3-P1, RIDE-A5-F1-P1 |  |
| RIDE-A6-F1-P3 | **Driver home screen**: Online switch, today's earnings, work time left and the map. | interface | medium | RIDE-A6-F1-P1, RIDE-A11-F1-P1 |  |
| RIDE-A6-F2-P1 | **Offer card**: Shows pickup time, trip length, destination area, passengers and bags, rider rating, needs, fare, fee and earnings, readable at a glance and accepted with one tap, and never the rider's name or photo, which appear only once the driver accepts. | interface | medium | [KIT-A6-F4-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), RIDE-A8-F2-P1, RIDE-A8-F3-P1, RIDE-A3-F3-P3, RIDE-A6-F12-P2 |  |
| RIDE-A6-F2-P2 | **Offer alerts**: Sound, vibration and a flash for each offer, each one adjustable. | interface | small | [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RIDE-A6-F3-P1 | **Accept and decline commands**: Accept, decline and let-expire as commands, taken only from the signed-in driver's own app, with no score kept against declines. | library | small | `kit-market`, [KIT-A6-F4-P2](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), RIDE-A7-F3-P1 |  |
| RIDE-A6-F3-P2 | **No-penalty rule**: Writes into the published dispatch rules that declines and acceptance rates never affect offers. | data | small | RIDE-A7-F4-P1, [KIT-A6-F4-P2](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A6-F4-P1 | **Driver preferences**: The driver's standing choices, used as filters by dispatch. | data | small | `kit-ops` |  |
| RIDE-A6-F4-P2 | **Driver preferences screen**: Simple settings with a map to draw or pick areas. | interface | small | RIDE-A6-F4-P1, RIDE-A11-F1-P1 |  |
| RIDE-A6-F5-P1 | **Destination filter**: Offers only trips that bring the driver closer to their chosen place by road time. | library | medium | [KIT-A6-F3-P3](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A7-F5-P1 |  |
| RIDE-A6-F6-P1 | **Passenger work time rules**: Ride's work time rules for passenger drivers in each country pack (example: New Zealand's) on the kit's work-time limits, counting all paid work and not only Spare Ride time: continuous work, daily and cumulative limits and rest breaks. | library | medium | [KIT-A24-F3-P1](../kit/FEATURES.md#kit-a24-health-and-safety-at-work), `kit-ops`, RIDE-A6-F1-P1 | [RD01](STATUS.md) |
| RIDE-A6-F6-P2 | **Electronic logbook**: Records work time as an electronic logbook built to the transport regulator's approval rules (example: New Zealand's NZTA), so drivers need no paper logbook. | service | large | RIDE-A6-F6-P1, RIDE-A6-F6-P5, RIDE-A5-F3-P2 | [RD01](STATUS.md) |
| RIDE-A6-F6-P3 | **Break reminders**: Warnings before a rest is due and a clear rest-break screen. | interface | small | RIDE-A6-F6-P1 |  |
| RIDE-A6-F6-P4 | **Logbook export**: Shows and exports the logbook for an enforcement officer or the driver. | interface | small | RIDE-A6-F6-P2, `engine-pdf` | [RD01](STATUS.md) |
| RIDE-A6-F6-P5 | **Other work entries**: The driver records time on other paid work, such as another ride app or a day job, so the limits and the logbook count it. | interface | small | RIDE-A6-F6-P1 | [RD01](STATUS.md) |
| RIDE-A6-F6-P6 | **Work time from other Spareware apps**: Counts time the same person worked in other Spareware apps, such as Spare Food deliveries, through the kit's shared worker record, so nobody types it twice. | library | small | RIDE-A6-F6-P1, [KIT-A6-F2-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A6-F6-P7 | **Logbook approval**: Gets the transport regulator's approval (example: New Zealand's NZTA) for the electronic logbook before drivers rely on it, with what the approval asks for. | legal | medium | RIDE-A6-F6-P2 | [RD01](STATUS.md) |
| RIDE-A6-F7-P1 | **Demand map service**: Counts open requests and recent trips by map cell, delayed and blurred so no rider can be found. | service | medium | [KIT-A6-F9-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A6-F7-P2 | **Demand map layer**: Shows busy areas on the driver's map. | interface | small | RIDE-A6-F7-P1, RIDE-A11-F1-P1 |  |
| RIDE-A6-F8-P1 | **Earnings screen**: Totals by day and week, each trip's fare, fee, tip and payout status. | interface | medium | RIDE-A9-F10-P1, [KIT-A3-F3-P3](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A6-F9-P1 | **Vehicle cost model**: Works out a cost per kilometre from the driver's own figures, including road user charges for electric and diesel vehicles. | library | small | RIDE-A2-F4-P1 |  |
| RIDE-A6-F9-P2 | **Earnings after costs**: Adds an after-costs figure to each offer, if the driver switches it on. | interface | small | RIDE-A6-F9-P1, RIDE-A6-F2-P1 |  |
| RIDE-A6-F10-P1 | **Chained offers**: Offers a trip that starts near the current drop-off before the trip ends. | library | small | RIDE-A7-F3-P1 |  |
| RIDE-A6-F11-P1 | **Driver ID screen**: Shows the driver's photo, name, licence details and operator, as the rules allow it to be shown. | interface | small | RIDE-A2-F1-P2, RIDE-A20-F3-P1 | [RD01](STATUS.md) |
| RIDE-A6-F12-P1 | **Phone rule for drivers**: What the Road User Rule lets a driver do with a mounted phone while driving, with sources, checked against each driver screen. | legal | small |  | [RD01](STATUS.md) |
| RIDE-A6-F12-P2 | **Moving lock**: Detects when the car is moving and locks typing, chat and anything beyond one-tap controls until it stops. | library | small | RIDE-A6-F1-P2, RIDE-A6-F12-P1 |  |
| RIDE-A6-F12-P3 | **Read aloud**: Reads new offers, rider messages and area notices aloud with the phone's own voice. | interface | small | RIDE-A6-F12-P2, RIDE-A23-F1-P3 |  |
| RIDE-A6-F13-P1 | **Voice commands for drivers**: Offer, arrival and reply commands spoken through the kit's voice control, as the same commands the screen uses. | library | medium | RIDE-A6-F12-P3, [KIT-A17-F8-P1](../kit/FEATURES.md#kit-a17-ai-access) |  |

## RIDE-A7 Dispatch and matching

How a request finds a driver: nearest by road time, offers that pass on, matching needs to vehicles, published rules with no hidden ranking, scheduled rides and fairness checks.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Nearest driver by road time | A rider's request goes first to the driver who can reach them soonest by road, not by straight-line distance. | core | Candidate search |
| **F2** Arrival estimates before booking | A rider sees how long a car will take before they book. | core | Arrival estimate |
| **F3** Offers that pass on | An offer waits a set time and then goes to the next driver, until someone accepts or the rider is told no one is free. | core | Offer cascade; No driver found |
| **F4** Published dispatch rules | Anyone can read exactly how trips are offered, and every trip records which version of the rules matched it. | core | Dispatch rules document; Rule versions on trips; Why did I get this trip |
| **F5** Match the right car to each need | Requests for wheelchair access, pets, larger cars, child seats or other needs go only to drivers and vehicles that can meet them. | core | Need and preference filters; Battery range check |
| **F6** Scheduled rides find a driver | A scheduled ride is offered to drivers ahead of time, and the rider is told early if no one has taken it. | next | Scheduled dispatch; Scheduled ride reminders; Scheduled trip board |
| **F7** Fair shares of work | The co-op checks that trips are shared fairly across drivers, areas and groups, and publishes the results. | next | Fairness measures; Fairness report; Decline pattern checks |
| **F8** Busy-time forecasts | Drivers see when and where demand is likely to rise, so they can choose when to work. | later | Forecast methods; Forecast service |
| **F9** City rules as plugins | A city's drivers vote to change how offers are scored, and the change runs as a checked plugin. | later | Scoring plugins |
| **F10** Replay past days before a change | The co-op replays past days of real requests through a proposed dispatch rule and compares wait times and earnings before members vote. | core | Dispatch replay |
| **F11** When a driver cancels | If a driver cancels after accepting, the rider is told why in plain words and goes to the front of the queue for the next driver at the same fare. | core | Re-match first; Driver cancel reasons |

### Parts: Dispatch and matching

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A7-F1-P2 | **Candidate search**: Ride's matching rule for the kit's dispatch engine: finds drivers in nearby cells, then ranks them by road time. | library | medium | [KIT-A6-F3-P3](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [KIT-A6-F3-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A7-F2-P1 | **Arrival estimate**: Shows pickup time before booking from the kit's arrival estimates and nearby free drivers, with driver positions blurred. | library | small | [KIT-A7-F3-P3](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), [KIT-A6-F3-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), RIDE-A21-F11-P1 |  |
| RIDE-A7-F3-P1 | **Offer cascade**: Ride's offer timeout and stop rule on the kit's dispatch engine: one offer at a time, passed on until someone accepts or the rider is told. | library | small | [KIT-A6-F3-P2](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), RIDE-A7-F1-P2 |  |
| RIDE-A7-F3-P2 | **No driver found**: Tells the rider plainly and offers to try again, widen the search or schedule. | interface | small | RIDE-A7-F3-P1 |  |
| RIDE-A7-F4-P1 | **Dispatch rules document**: The rules in plain words and as settings, changed only by a drivers' vote. | data | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A7-F4-P2 | **Rule versions on trips**: Stores the dispatch rule version with each match so any match can be explained later. | library | small | RIDE-A7-F4-P1, RIDE-A5-F3-P2 |  |
| RIDE-A7-F4-P3 | **Why did I get this trip**: Shows a driver why an offer came to them, and why others did not, from the stored rules. | interface | small | RIDE-A7-F4-P2 |  |
| RIDE-A7-F5-P1 | **Need and preference filters**: Filters candidates by vehicle rules, driver preferences, blocks and rider needs. | library | small | RIDE-A2-F4-P4, RIDE-A6-F4-P1, [KIT-A10-F4-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A7-F5-P2 | **Battery range check**: For electric cars, compares the range the driver sets when going online with the pickup and the trip plus a margin before a long offer, so nobody is stranded. | library | small | RIDE-A7-F5-P1, RIDE-A2-F4-P1 |  |
| RIDE-A7-F6-P1 | **Scheduled dispatch**: Offers scheduled rides ahead, reminds the driver, and falls back to live dispatch near pickup time. | service | medium | RIDE-A7-F3-P1, RIDE-A3-F7-P1, [KIT-A5-F9-P1](../kit/FEATURES.md#kit-a5-marketplace-engine) |  |
| RIDE-A7-F6-P2 | **Scheduled ride reminders**: Reminders to rider and driver, and an early warning if no driver is confirmed. | service | small | RIDE-A7-F6-P1, [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RIDE-A7-F6-P3 | **Scheduled trip board**: An open list where drivers pick scheduled and long trips themselves, alongside offers sent one at a time, under the same published rules. | interface | small | RIDE-A7-F6-P1, RIDE-A7-F4-P1 |  |
| RIDE-A7-F7-P1 | **Fairness measures**: Measures offers and earnings across drivers and areas without exposing anyone. | data | medium | RIDE-A5-F3-P2, [KIT-A6-F5-P2](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A7-F7-P2 | **Fairness report**: A regular public report on the measures, published with the cost ledger. | interface | small | RIDE-A7-F7-P1, [KIT-A13-F1-P3](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |
| RIDE-A7-F7-P3 | **Decline pattern checks**: Looks for declines and cancellations clustering by suburb, destination, rider name or disability need, on the kit's decline pattern checks, and sends patterns to a person, never acting on a single decline. | service | small | [KIT-A6-F5-P3](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), RIDE-A7-F7-P1 |  |
| RIDE-A7-F8-P1 | **Forecast methods**: Proven open methods for short-term demand forecasts from trip history. | research | small |  |  |
| RIDE-A7-F8-P2 | **Forecast service**: Produces forecasts by area and hour for the driver app. | service | medium | RIDE-A7-F8-P1, RIDE-A6-F7-P1 |  |
| RIDE-A7-F9-P1 | **Scoring plugins**: Dispatch scoring as sandboxed plugins with declared inputs, swapped only after a vote. | library | medium | `kit-plugin`, RIDE-A7-F4-P1 |  |
| RIDE-A7-F10-P1 | **Dispatch replay**: Replays past days of requests through a new rule version and compares wait times and earnings. | library | medium | [KIT-A6-F3-P2](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), RIDE-A19-F9-P1 |  |
| RIDE-A7-F11-P1 | **Re-match first**: Puts the rider back at the front of dispatch with the same quote and tells them what happened. | library | small | RIDE-A7-F3-P1, RIDE-A3-F4-P2 |  |
| RIDE-A7-F11-P2 | **Driver cancel reasons**: The reasons a driver can give that never count against them, such as too many passengers, no child seat, a child riding alone or a safety worry, and what the rider sees. | data | small | RIDE-A8-F7-P1, [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |

## RIDE-A8 Fares and pricing

How a fare is set, quoted, broken down and explained: a published formula drivers vote on, the at-cost fee from the ledger, extras, changes during a trip and any busy-time pricing.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Fare formula set by drivers | Drivers vote on a published fare formula of a base fare, a rate per kilometre, a rate per minute and a minimum fare. | core | Fare formula design; Fare formula; Formula versions |
| **F2** Upfront quote | A rider gets a firm fare before booking from the expected route and time. | core | Quote service |
| **F3** Breakdown on every fare | Every fare shows the driver's share, the at-cost fee, the payment fee and sales tax. | core | Breakdown calculator; At-cost fee; Who pays the fee; Who sells the ride; Payment fee and surcharge rules |
| **F4** Busy-time pricing | Riders see any busy-time price before booking, and it only exists if drivers voted for it, within a voted cap. | next | Busy-time rule; Busy-time notice |
| **F5** Extras | Tolls, airport fees, waiting time and cleaning charges are added by published rules and shown on the receipt. | core | Toll and airport fee data; Extras rules; Cleaning claim |
| **F6** Fare changes during a trip | If the route, stops or destination change, the fare changes by a published rule the rider can check. | core | Fare adjustment rules; Route check |
| **F7** Cancellation and no-show fees | Cancellation and no-show fees follow published rules, and go to the driver who lost the time. | core | Cancellation rules |
| **F8** Explain any past fare | A rider or driver taps any past trip and sees exactly how its fare was worked out. | core | Fare explainer; Fare explanation screen |
| **F9** Fares per city | Drivers in each city set their own fare formula. | next | Regional fares |

### Parts: Fares and pricing

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A8-F1-P1 | **Fare formula design**: How the formula is set, shown and changed, with worked examples, within what competition law allows. | research | small | RIDE-A20-F12-P1 | [RD06](STATUS.md) |
| RIDE-A8-F1-P2 | **Fare formula**: Calculates a fare from distance, time and the voted rates, the same on server and phone. | library | medium | `kit-market`, `kit-core`, [KIT-A5-F3-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A8-F1-P1 | [RD06](STATUS.md) |
| RIDE-A8-F1-P3 | **Formula versions**: Every formula version stored with its vote and start date, and linked from each trip. | data | small | `kit-ops`, RIDE-A18-F2-P2 | [RD06](STATUS.md) |
| RIDE-A8-F2-P1 | **Quote service**: Prices the expected route for each ride option and signs the quote so it cannot be changed. | service | medium | RIDE-A8-F1-P2, [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), [KIT-A5-F3-P2](../kit/FEATURES.md#kit-a5-marketplace-engine) |  |
| RIDE-A8-F3-P1 | **Breakdown calculator**: Splits a fare into driver share, fee, payment fee and sales tax, to the cent. | library | small | RIDE-A8-F1-P2, RIDE-A8-F3-P2, RIDE-A8-F3-P3, RIDE-A8-F3-P5, [KIT-A4-F1-P2](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) |  |
| RIDE-A8-F3-P2 | **At-cost fee**: Applies Spare Ride's fee for the period, set by the kit's fee setting from the last period's running cost divided by trips. | library | small | [KIT-A13-F3-P1](../kit/FEATURES.md#kit-a13-open-cost-ledger), [KIT-A3-F4-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) | [K06](../kit/STATUS.md) |
| RIDE-A8-F3-P3 | **Who pays the fee**: Whether the rider or the driver pays the at-cost fee, and what that means for sales tax on the fee and tax invoices for drivers registered for sales tax. | research | small | [KIT-A4-F1-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) | [RD06](STATUS.md) |
| RIDE-A8-F3-P4 | **Who sells the ride**: Settles on the kit's seller of record rule whether the co-op or the driver sells each ride, which sets whose name is on the receipt, who owes consumer guarantees and how tax invoices read. | legal | small | [KIT-A3-F1-P5](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A20-F5-P1 | [K02](../kit/STATUS.md) |
| RIDE-A8-F3-P5 | **Payment fee and surcharge rules**: Checks that the payment fee line on every fare is not a card surcharge that a country's ban forbids for in-app payments (example: New Zealand's planned ban), on the kit's card fee pass-through check. | legal | small | [KIT-A3-F4-P3](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A8-F4-P1 | **Busy-time rule**: A capped multiplier that switches on from published demand measures, or not at all. | library | medium | RIDE-A8-F1-P2, RIDE-A6-F7-P1, RIDE-A18-F2-P2 | [RD06](STATUS.md) |
| RIDE-A8-F4-P2 | **Busy-time notice**: Shows the multiplier, why it is on and the cap, before the rider books. | interface | small | RIDE-A8-F4-P1 |  |
| RIDE-A8-F5-P1 | **Toll and airport fee data**: Toll roads and airport charges with their prices, kept up to date from the road agency (example: New Zealand's NZTA) and each airport and matched to the route. | data | small |  |  |
| RIDE-A8-F5-P2 | **Extras rules**: Adds tolls, airport fees and waiting charges to a fare, and passes them straight through. | library | small | RIDE-A8-F5-P1, RIDE-A8-F1-P2 |  |
| RIDE-A8-F5-P3 | **Cleaning claim**: A driver claims a cleaning charge with photos on the kit's extra charges after the job; the rider can dispute it. | interface | small | [KIT-A5-F15-P2](../kit/FEATURES.md#kit-a5-marketplace-engine), RIDE-A15-F4-P1, `engine-media` |  |
| RIDE-A8-F6-P1 | **Fare adjustment rules**: When a fare stays fixed, when it is re-quoted and by how much it may move. | library | small | RIDE-A8-F2-P1 | [RD06](STATUS.md) |
| RIDE-A8-F6-P2 | **Route check**: Compares the trace with the quoted route to spot long detours and fix the fare. | library | small | RIDE-A5-F5-P3, RIDE-A8-F6-P1 |  |
| RIDE-A8-F7-P1 | **Cancellation rules**: Free window, fee and who gets it, for riders and drivers alike. | library | small | RIDE-A8-F1-P2, [KIT-A5-F6-P1](../kit/FEATURES.md#kit-a5-marketplace-engine) | [RD06](STATUS.md) |
| RIDE-A8-F8-P1 | **Fare explainer**: Replays a trip's stored formula version and inputs to show every step. | library | small | RIDE-A8-F1-P3, RIDE-A5-F3-P2 |  |
| RIDE-A8-F8-P2 | **Fare explanation screen**: The steps of the fare in plain words. | interface | small | RIDE-A8-F8-P1 |  |
| RIDE-A8-F9-P1 | **Regional fares**: Formula settings per service area, each with its own vote. | data | small | RIDE-A8-F1-P3, RIDE-A11-F6-P1 |  |

## RIDE-A9 Payments, tips and payouts

How money moves: riders pay through a regulated payment provider, the fare is held then taken, tips go untouched to the driver, refunds and failed payments are handled, and drivers are paid out daily.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Add a way to pay | A rider adds a card, Apple Pay or Google Pay, entered on the payment provider's own form. | core | Payment method screens |
| **F2** Pay and hold | The fare is held when a driver accepts and taken when the trip ends, so riders are never charged for a trip that did not happen. | core | Hold and capture; Hold top-up |
| **F3** Tips | A rider adds a tip during or after the trip and all of it goes to the driver. | core | Tip flow; Card fee on tips |
| **F4** Split a fare | Riders sharing a car split the fare in the app, each paying their share. | next | Fare split |
| **F5** Pay by bank | A rider pays straight from their bank account. | later | Bank payments research |
| **F6** Cash | A rider pays the driver in cash where members decide to allow it. | later | Cash rules |
| **F7** Ride credit and gift vouchers | A person buys ride credit or a gift voucher for someone else. | later | Ride vouchers; Voucher terms |
| **F8** Refunds and corrections | A rider gets a refund or a corrected fare when a review finds they were overcharged. | core | Refund command |
| **F9** Receipts and tax invoices | A rider gets a receipt for every trip that is also a valid tax invoice where sales tax applies. | core | Receipt maker; Receipt delivery |
| **F10** Daily payouts | A driver is paid out every day with no fee beyond what the provider charges. | core | Driver earnings ledger |
| **F11** Instant payout | A driver cashes out now, paying only the provider's cost. | next | Instant payout |
| **F12** Failed payments and chargebacks | The app handles declined cards and chargebacks fairly, using the trip record as evidence. | core | Failed payment handling; Chargeback evidence for trips |
| **F13** Money owed after a payout | When a refund, chargeback or fare correction comes after a driver was paid, the driver sees what is owed and repays it fairly from later earnings. | core | Ride clawback rules; Ride balance recovery |

### Parts: Payments, tips and payouts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A9-F1-P2 | **Payment method screens**: Add, remove and pick a default card or wallet in Spare Ride, using the kit's checkout and saved payment methods. | interface | small | [KIT-A3-F8-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), [KIT-A3-F1-P3](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A9-F2-P1 | **Hold and capture**: Places a hold for the quoted fare when a driver accepts and captures the final fare at the end, using the kit's hold and release rules. | library | medium | [KIT-A3-F2-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), `kit-market`, RIDE-A3-F5-P2 | [K02](../kit/STATUS.md) |
| RIDE-A9-F2-P2 | **Hold top-up**: Raises the hold when the fare rises from new stops, a new destination, waiting or tolls, and asks the rider for another card if the larger hold fails. | library | small | RIDE-A9-F2-P1, [KIT-A5-F15-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), [KIT-A3-F2-P4](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A9-F3-P1 | **Tip flow**: Tip amounts, custom tips and a time limit, passed to the driver with no fee taken. | library | small | [KIT-A3-F6-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A9-F2-P1 |  |
| RIDE-A9-F3-P2 | **Card fee on tips**: A published rule, set by vote, on whether the rider adds the card fee on a tip or the running costs cover it; it is never taken from the driver's tip. | data | small | RIDE-A9-F3-P1, [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RIDE-A9-F4-P1 | **Fare split**: Sends a split request to friends and charges each their share once they accept. | library | medium | RIDE-A9-F2-P1 |  |
| RIDE-A9-F5-P1 | **Bank payments research**: Whether open banking payments suit rides in each country, starting with New Zealand, and which providers support them. | research | small | [KIT-A3-F11-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) | [K02](../kit/STATUS.md) |
| RIDE-A9-F6-P1 | **Cash rules**: Whether cash fits the registered passenger rule, sales tax collection and driver safety. | research | small |  | [RD01](STATUS.md) |
| RIDE-A9-F7-P2 | **Ride vouchers**: Ride credit and vouchers for a set amount or a named rider, redeemed at payment, on the kit's gift cards. | library | small | [KIT-A3-F15-P2](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A9-F2-P1, RIDE-A9-F7-P3 |  |
| RIDE-A9-F7-P3 | **Voucher terms**: Vouchers stay valid for as long as each country's law asks, such as at least three years under New Zealand's Fair Trading Act since 16 March 2026, and ride credit is paid back or kept when an account closes. | legal | small | [KIT-A3-F15-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A9-F8-P1 | **Refund command**: Ride refunds and fare corrections on the kit's refunds, with the reason recorded and the driver's share handled by the clawback rules. | library | small | [KIT-A3-F5-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), `kit-ops`, RIDE-A9-F13-P1 |  |
| RIDE-A9-F9-P1 | **Receipt maker**: Ride receipts on the kit's receipts and tax invoices, showing who sold the ride, the fare breakdown, the route, the driver's identification details and the vehicle's registration number. | library | small | [KIT-A3-F7-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), [KIT-A4-F2-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), `engine-pdf`, RIDE-A8-F3-P1, RIDE-A8-F3-P4, RIDE-A2-F4-P1, RIDE-A6-F11-P1 |  |
| RIDE-A9-F9-P2 | **Receipt delivery**: Sends receipts by email and keeps them in the trip history. | service | small | [KIT-A8-F4-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A9-F9-P1 |  |
| RIDE-A9-F10-P1 | **Driver earnings ledger**: Every fare, tip, fee and payout for a driver, as operations that add up exactly. | data | medium | `kit-ops`, [KIT-A3-F9-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A9-F11-P1 | **Instant payout**: Sends a payout on demand, passing the provider's charge through at cost. | integration | small | [KIT-A3-F3-P2](../kit/FEATURES.md#kit-a3-payments-and-payouts) | [K02](../kit/STATUS.md) |
| RIDE-A9-F12-P1 | **Failed payment handling**: Retries, tells the rider, and holds new bookings until the balance is paid. | library | small | RIDE-A9-F2-P1 |  |
| RIDE-A9-F12-P2 | **Chargeback evidence for trips**: Sends the trip record, route and fare steps to the kit's card dispute handling, and keeps the driver paid when the trip was real. | library | small | [KIT-A3-F5-P2](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A5-F3-P2, RIDE-A9-F13-P1 | [K02](../kit/STATUS.md) |
| RIDE-A9-F13-P1 | **Ride clawback rules**: Ride's voted limits on the kit's loss rules: when a driver's share can be taken back, the most taken from any one payout, and which errors the co-op pays for itself. | data | small | [KIT-A3-F16-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A9-F10-P1 |  |
| RIDE-A9-F13-P2 | **Ride balance recovery**: Recovers money a driver owes through the kit's negative balances, in small steps from later earnings and never from tips, shown on the earnings ledger. | library | small | [KIT-A3-F16-P2](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A9-F13-P1 |  |

## RIDE-A10 Tax and earnings records

The tax and pay records the platform must keep and give drivers in each country: sales tax on rides (example: New Zealand GST on rides as a listed service, with flat-rate credits), income reports to the tax office (example: New Zealand's IRD), earnings statements, mileage logs, and payroll if drivers are employees.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Sales tax on rides (example: New Zealand GST) | The platform charges and pays sales tax on every ride where the law requires it, such as 15% GST on ride-sharing in New Zealand. | core | Ride figures for sales tax returns; Ride fee invoices; Tax on extras |
| **F2** Flat-rate credit statements | A driver who is not registered for sales tax gets a monthly statement of any flat-rate credits their country gives (example: New Zealand's GST flat-rate credits). | core | Flat-rate credits for drivers |
| **F3** Income reports to the tax office (example: New Zealand's IRD) | The platform reports each driver's income to the tax office as the law requires and shows the driver what was sent. | core | What we told the tax office |
| **F4** Earnings statements | A driver downloads weekly, monthly and yearly earnings statements for their accountant or a lender. | core | Weekly and monthly statements |
| **F5** Mileage and expenses log | A driver keeps a log of work kilometres and expenses, filled in from their trips, for their tax return. | next | Work kilometre tracker; Expense entry; Tax pack export |
| **F6** Pay records if drivers are employees | If drivers are employed by the co-op, they get payslips, tax and retirement savings deductions (example: New Zealand's PAYE and KiwiSaver), leave, holiday pay and at least the minimum wage for time worked. | next | Ride pay inputs; Ride pay to payroll; Leave requests; Time and wage records; Availability terms |
| **F7** Accident cover and levies explained | A self-employed driver sees what accident cover levies (example: New Zealand's ACC) and other costs to expect and where to sort them. | later | Ride's accident cover page |

### Parts: Tax and earnings records

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A10-F1-P3 | **Ride figures for sales tax returns**: Feeds Ride's sales tax on fares, fees and extras into the kit's operator sales tax returns. | service | small | [KIT-A4-F9-P2](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), [KIT-A4-F1-P2](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) |  |
| RIDE-A10-F1-P4 | **Ride fee invoices**: Ride's fee lines on the kit's sales tax on the fee, so drivers registered for sales tax get a tax invoice for any fee they pay once it is settled who pays it. | service | small | [KIT-A4-F9-P3](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), RIDE-A8-F3-P3, RIDE-A8-F3-P4 |  |
| RIDE-A10-F1-P5 | **Tax on extras**: How tips, cancellation and no-show fees, waiting charges, tolls, airport fees and cleaning charges are treated for sales tax and in income reports to the tax office, starting with New Zealand's GST listed services rules and IRD, which is unverified. | research | small | [KIT-A4-F1-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), [KIT-A4-F3-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), RIDE-A8-F5-P2 | [RD06](STATUS.md) |
| RIDE-A10-F2-P1 | **Flat-rate credits for drivers**: Shows each month's flat-rate credits in the driver's earnings, from the kit's flat-rate credit notices, with a PDF copy. | interface | small | [KIT-A4-F1-P3](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), RIDE-A6-F8-P1 |  |
| RIDE-A10-F3-P2 | **What we told the tax office**: Shows each driver the figures reported about them. | interface | small | [KIT-A4-F3-P3](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) |  |
| RIDE-A10-F4-P1 | **Weekly and monthly statements**: Weekly and monthly earnings statements as PDF and CSV for an accountant or a lender, alongside the kit's yearly statement. | service | small | RIDE-A9-F10-P1, [KIT-A4-F4-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), [KIT-A4-F4-P2](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) |  |
| RIDE-A10-F5-P1 | **Work kilometre tracker**: Counts kilometres while online and on trips from the driver's own traces. | library | small | RIDE-A6-F1-P2, RIDE-A5-F3-P2 |  |
| RIDE-A10-F5-P2 | **Expense entry**: Add fuel, servicing and other costs with a photo of the receipt. | interface | small | `kit-store`, `engine-media` |  |
| RIDE-A10-F5-P3 | **Tax pack export**: One download of earnings, kilometres and expenses for the year. | service | small | RIDE-A10-F5-P1, RIDE-A10-F5-P2, RIDE-A10-F4-P1 |  |
| RIDE-A10-F6-P1 | **Ride pay inputs**: If drivers are employees, which Ride time counts as work, such as online, waiting and on trip, and how fares and tips feed pay under the kit's pay rules. | research | small | [KIT-A23-F2-P1](../kit/FEATURES.md#kit-a23-work-law-and-worker-status), RIDE-A20-F5-P1 | [RD01](STATUS.md) |
| RIDE-A10-F6-P2 | **Ride pay to payroll**: Sends Ride's worked time and earnings to the kit's payroll provider link and minimum pay top-up. | integration | small | [KIT-A23-F2-P3](../kit/FEATURES.md#kit-a23-work-law-and-worker-status), [KIT-A23-F2-P1](../kit/FEATURES.md#kit-a23-work-law-and-worker-status), RIDE-A9-F10-P1, RIDE-A6-F6-P1, RIDE-A10-F6-P1 |  |
| RIDE-A10-F6-P4 | **Leave requests**: If drivers are employees, they ask for annual leave, sick leave and public holidays in the app, paid under the kit's holiday pay. | interface | small | [KIT-A23-F2-P2](../kit/FEATURES.md#kit-a23-work-law-and-worker-status), RIDE-A20-F5-P2 |  |
| RIDE-A10-F6-P5 | **Time and wage records**: If drivers are employees, keeps the time, wage, holiday and leave records employment law requires, from online and trip time. | data | small | RIDE-A6-F6-P1, RIDE-A9-F10-P1, [KIT-A23-F2-P3](../kit/FEATURES.md#kit-a23-work-law-and-worker-status) |  |
| RIDE-A10-F6-P6 | **Availability terms**: If drivers are employees, how rosters or availability terms work, since going online at any time may not fit employment. | legal | small | RIDE-A20-F5-P1 | [RD01](STATUS.md) |
| RIDE-A10-F7-P1 | **Ride's accident cover page**: Shows self-employed drivers the kit's savings, accident cover and tax guidance in the driver help hub, with examples from ride work. | data | small | [KIT-A23-F2-P4](../kit/FEATURES.md#kit-a23-work-law-and-worker-status), RIDE-A16-F5-P1 |  |

## RIDE-A11 Maps, routing and navigation

The maps underneath every screen, all on open data: map display, address search, routes and travel times, turn-by-turn directions, pickup points, service areas, map fixes and offline maps.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** A map on every screen | A rider or driver sees a clear, fast map that works on cheap phones, with open map credits shown. | core | Ride map layers |
| **F2** Address and place search | A person finds any address or place by name, in each place's languages (for New Zealand: English or te reo Māori). | core | Place names in two languages |
| **F3** Routes and travel times | The app works out the best road route and a realistic travel time for every quote, offer and trip. | core | Ride trips for travel time learning |
| **F4** Turn-by-turn directions | A driver gets spoken turn-by-turn directions in the app, or opens the trip in another navigation app. | core | Driver navigation screen; Open in another app |
| **F5** Pickup and drop-off points | Riders and drivers meet at safe, legal spots instead of the middle of a road. | core | Pickup point data; Pickup point finder |
| **F6** Service areas and zones | The co-op sets where Spare Ride works, and zones for airports, events and fares. | core | Ride zones; Zone editor |
| **F7** Fix the map | A driver reports a wrong road, a closed street or a bad pickup spot, and fixes go back to OpenStreetMap. | next | Pickup spot reports |
| **F8** Offline maps | A driver downloads their city's map so directions still work in dead spots. | next | Driver's city pack |
| **F9** Road closures and events | Drivers and quotes take road closures and works into account. | next | Road event feeds; Closures on the map |
| **F10** Find a charger | A driver with an electric car sees public chargers on the map. | next | Charger map layer |

### Parts: Maps, routing and navigation

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A11-F1-P1 | **Ride map layers**: Cars, pickup pins, routes and zones drawn on the kit's map display, light and dark. | interface | small | [KIT-A7-F1-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), [KIT-A7-F1-P3](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A11-F2-P3 | **Place names in two languages**: Matches places by their names in each language, such as English and te reo Māori names in New Zealand. | data | small | [KIT-A7-F2-P3](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A11-F3-P3 | **Ride trips for travel time learning**: Feeds finished Spare Ride trips into the kit's arrival time learning, anonymised first. | library | small | [KIT-A7-F3-P3](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A5-F3-P2 |  |
| RIDE-A11-F4-P1 | **Driver navigation screen**: Guidance to the pickup and then each stop in order, laid out for a phone in a cradle, on the kit's turn-by-turn guidance. | interface | medium | [KIT-A7-F6-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A11-F1-P1, RIDE-A6-F12-P2 |  |
| RIDE-A11-F4-P2 | **Open in another app**: Hands the next stop to the navigation app the driver prefers. | integration | small |  | [K04](../kit/STATUS.md) |
| RIDE-A11-F5-P1 | **Pickup point data**: Good stopping spots, no-stopping zones and entrances, from open data and driver reports. | data | medium | [KIT-A7-F1-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), [KIT-A7-F1-P3](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A11-F5-P2 | **Pickup point finder**: Snaps a location to the nearest good spot on the right side of the road. | library | medium | RIDE-A11-F5-P1, [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A11-F6-P1 | **Ride zones**: Airport zones, event zones and fare regions as versioned map shapes on top of the kit's service areas. | data | small | [KIT-A7-F5-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), `kit-ops` |  |
| RIDE-A11-F6-P2 | **Zone editor**: Draw and change areas and zones on a map, each change an operation. | interface | medium | RIDE-A11-F6-P1, RIDE-A11-F1-P1 |  |
| RIDE-A11-F7-P1 | **Pickup spot reports**: A driver reports a bad pickup spot, which updates Ride's pickup data; road mistakes go to the kit's map fix reports. | interface | small | [KIT-A7-F8-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A11-F5-P1 |  |
| RIDE-A11-F8-P1 | **Driver's city pack**: Keeps the map and routing pack for the driver's service area current on the phone, using the kit's offline map packs. | library | small | [KIT-A7-F7-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A11-F6-P1 |  |
| RIDE-A11-F9-P1 | **Road event feeds**: Imports official open road closure and works feeds, under their terms, into the kit's routing service. | integration | medium | [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A11-F9-P2 | **Closures on the map**: Shows closures and routes around them. | interface | small | RIDE-A11-F9-P1, RIDE-A11-F1-P1 |  |
| RIDE-A11-F10-P1 | **Charger map layer**: Shows public electric vehicle chargers on the driver's map from open charger data, under its licence terms. | integration | small | RIDE-A11-F1-P1 |  |

## RIDE-A12 Messages, calls and notifications

How riders, drivers and the co-op reach each other: in-trip chat, calls that hide phone numbers, push, text and email, translation, and each person's choice of what reaches them.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Chat during a trip | A rider and driver message each other from booking until shortly after the trip ends. | core | Trip chat; Quick replies |
| **F2** Calls that hide numbers | A rider and driver call each other without seeing each other's phone number. | core | Trip calls; Ride call needs |
| **F3** Push notifications | A rider or driver is told at once when something changes, even with the app closed. | core | Notice catalogue |
| **F4** Text message updates | A rider without data or the app gets booking updates by text. | core | Text fallback |
| **F5** Email | A person gets receipts, statements and account notices by email. | core | Email notices |
| **F6** Choose what reaches me | A person picks which notices they get and how, and the app never sends marketing they did not ask for. | core | Ride notice choices |
| **F7** Messages in my language | A rider and driver who speak different languages see each other's messages translated. | next | Trip chat translation |
| **F8** Co-op announcements | Members get notices about votes, rule changes and the monthly cost ledger. | next | Ride member notices |
| **F9** Warned about trips off the app | A rider or driver is warned when chat drifts toward setting up a ride outside the app. | next | Off-app trip warnings |

### Parts: Messages, calls and notifications

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A12-F1-P1 | **Trip chat**: A trip chat on the kit's chat service that opens at booking and closes on its own after the trip. | service | medium | [KIT-A8-F1-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A3-F5-P2 |  |
| RIDE-A12-F1-P2 | **Quick replies**: One-tap messages such as "I'm here" and "Coming now", which a driver can send only when the car is stopped, or by voice. | interface | small | RIDE-A12-F1-P1, RIDE-A6-F12-P2 |  |
| RIDE-A12-F2-P1 | **Trip calls**: Masked calls between rider and driver from acceptance until shortly after the trip, and for lost items, on the kit's masked calls. | integration | small | [KIT-A8-F2-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RIDE-A12-F2-P2 | **Ride call needs**: What Ride needs from the kit's telephony provider, such as masked calls on each trip, the phone booking line and relay service calls (example: New Zealand Relay), with expected call volumes. | research | small | [KIT-A8-F2-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A21-F4-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RIDE-A12-F3-P2 | **Notice catalogue**: Every notice the app sends, its wording in each language and who can switch it off. | data | small | [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A8-F4-P3](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A19-F1-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RIDE-A12-F4-P1 | **Text fallback**: Sends key trip updates by SMS when push fails or the rider has no app. | service | small | [KIT-A8-F4-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RIDE-A12-F5-P1 | **Email notices**: Plain, accessible emails for receipts, statements and account changes. | service | small | [KIT-A8-F4-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RIDE-A12-F6-P1 | **Ride notice choices**: Spare Ride's notice types added to the kit's notification settings, with safety and trip notices always on. | interface | small | [KIT-A8-F5-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A12-F3-P2 |  |
| RIDE-A12-F7-P1 | **Trip chat translation**: Turns on the kit's message translation in trip chat when the rider and driver use different languages, with the original one tap away. | integration | small | [KIT-A8-F7-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A12-F1-P1 |  |
| RIDE-A12-F8-P1 | **Ride member notices**: Spare Ride's vote, fare change and ledger notices, sent to its members through the kit's co-op announcements. | service | small | [KIT-A8-F8-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A12-F4-P2](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RIDE-A12-F9-P1 | **Off-app trip warnings**: Warns both people when trip chat suggests a ride outside the app, such as a direct cash trip, using the kit's scam warnings, and explains what the rider loses. | service | small | [KIT-A8-F9-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A12-F1-P1, RIDE-A20-F2-P5 |  |

## RIDE-A13 Safety

Tools that keep riders and drivers safe before, during and after a trip: emergency help, trip sharing, checking the right car and driver, check-ins when something looks wrong, recording, preferences and a team that responds.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Emergency button | A rider or driver calls emergency services from the app and sees their location, car and trip details to read out. | core | Ride emergency details; Safety team alert; Help by text |
| **F2** Share my trip | A rider or driver shares a live link of the trip with people they trust, by hand or automatically. | core | Ride trip link page; Trusted contacts |
| **F3** Check it is the right car | A rider confirms the car with a four-digit PIN given to the driver, as well as the plate and photo. | core | Trip PIN; Right car prompt |
| **F4** Check it is the right driver | A driver takes a quick live selfie now and then so riders and the co-op know the account holder is the one driving. | core | Right-driver checks |
| **F5** Check-ins when something looks wrong | The app notices a long stop, a big detour or an early end and asks both people if they are OK. | core | Ride check-in rules; Check-in prompt |
| **F6** Crash detection | The app notices a possible crash and offers help. | later | Crash detection methods; Crash detection |
| **F7** Record the trip audio | A rider or driver records audio during a trip; it stays encrypted on their phone and is shared only if they report an incident. | next | Recording rules; Encrypted recorder |
| **F8** Safety preferences | A person turns safety tools on automatically, such as sharing every trip after dark or always using a PIN. | next | Safety preferences |
| **F9** Women matched with women | Women riders and drivers choose to be matched with women. | later | Women preferences and the law; Women preferences filter |
| **F10** Verified riders | A driver sees that a rider's identity is verified and can choose to take only verified riders at night. | next | Rider verification |
| **F11** Report a safety incident | A rider or driver reports a safety incident during or after a trip and a trained person responds. | core | Incident report; Safety team plan; Sexual harm reports |
| **F12** After a crash | A driver involved in a crash reports it, gets help with insurance, and the rider is looked after. | core | Ride crash report |
| **F13** Safe driving help | A driver sees speed limit alerts and gets gentle feedback on hard braking and phone use. | later | Speed limit alerts; Driving feedback |
| **F14** Safety tips and education | Riders and drivers find short guides on staying safe. | next | Safety guides |
| **F15** No driving impaired | Riders know no driver may drive under the influence of alcohol or drugs, and report a driver who seems impaired in one tap. | core | Drug and alcohol rule; Impaired driver report |
| **F16** Dashcam footage | A driver who runs their own dashcam tells riders, keeps footage only as long as the rules allow and shares it as evidence when something goes wrong. | next | Dashcam rules; Camera notice; Footage upload |

### Parts: Safety

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A13-F1-P1 | **Ride emergency details**: Shows the plate, car, driver and trip large on the kit's emergency button, with the right number for the country. | interface | small | [KIT-A10-F1-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A22-F5-P1, RIDE-A5-F3-P2 |  |
| RIDE-A13-F1-P2 | **Safety team alert**: Tells the co-op's safety team at once when the emergency button is used. | service | small | RIDE-A19-F5-P1, [KIT-A10-F1-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A13-F1-P3 | **Help by text**: For people who cannot make a voice call, opens the country's emergency text service (example: 111 TXT in New Zealand, for those registered with Police), with the location and car filled in, and a text chat with the safety team. | interface | small | RIDE-A13-F1-P1, [KIT-A10-F1-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A13-F2-P1 | **Ride trip link page**: Shows the car, plate, route and arrival time on the kit's trusted contact link, ending when the trip ends. | service | small | [KIT-A10-F2-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A11-F1-P1 |  |
| RIDE-A13-F2-P2 | **Trusted contacts**: Up to five people to share trips with, by hand or by rule. | data | small | RIDE-A1-F1-P3, RIDE-A21-F10-P2 |  |
| RIDE-A13-F3-P1 | **Trip PIN**: Gives the rider a PIN that the driver must enter to start the trip. | library | small | RIDE-A5-F3-P1 |  |
| RIDE-A13-F3-P2 | **Right car prompt**: Shows the plate, colour and driver photo large as the car arrives. | interface | small | RIDE-A5-F1-P3 |  |
| RIDE-A13-F4-P1 | **Right-driver checks**: When and how often Ride asks a driver for a live selfie on the kit's repeat face checks, within the limits the biometrics assessment sets, with a person reviewing any failure. | library | small | [KIT-A2-F12-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), [KIT-A2-F12-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), RIDE-A2-F3-P1, RIDE-A21-F9-P1 |  |
| RIDE-A13-F5-P1 | **Ride check-in rules**: Thresholds for long stops, big detours and early ends on ride trips, run by the kit's trip check-ins. | library | small | [KIT-A10-F16-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A5-F1-P1 |  |
| RIDE-A13-F5-P2 | **Check-in prompt**: Asks if everything is OK, with quick routes to help or the emergency button. | interface | small | RIDE-A13-F5-P1, RIDE-A13-F1-P1 |  |
| RIDE-A13-F6-P1 | **Crash detection methods**: Proven ways phones detect crashes, and the false alarm rate. | research | small |  |  |
| RIDE-A13-F6-P2 | **Crash detection**: Watches phone sensors during trips and starts a check-in after a likely crash. | library | medium | RIDE-A13-F6-P1, RIDE-A13-F5-P2 |  |
| RIDE-A13-F7-P1 | **Recording rules**: What each country's privacy law requires for recording in a vehicle, starting with New Zealand's, including notice and consent. | legal | small |  | [RD01](STATUS.md) |
| RIDE-A13-F7-P2 | **Encrypted recorder**: Records audio to encrypted storage on the phone that only the co-op's safety team can open after a report. | library | medium | `engine-media`, RIDE-A13-F7-P1 |  |
| RIDE-A13-F8-P1 | **Safety preferences**: Rules that switch safety tools on by time or every trip. | data | small | RIDE-A1-F5-P1, RIDE-A13-F3-P1, RIDE-A13-F2-P1 |  |
| RIDE-A13-F9-P1 | **Women preferences and the law**: Whether each country's human rights law allows this, starting with New Zealand's, and on what terms. | legal | small |  | [RD01](STATUS.md) |
| RIDE-A13-F9-P2 | **Women preferences filter**: Matches women who opt in, using the gender each person states or their ID shows, never one guessed from a face. | library | small | RIDE-A7-F5-P1, RIDE-A13-F9-P1, RIDE-A21-F9-P3 |  |
| RIDE-A13-F10-P1 | **Rider verification**: Optional ID check for riders, storing only the result. | integration | small | [KIT-A2-F1-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), RIDE-A1-F1-P3, RIDE-A21-F9-P1 |  |
| RIDE-A13-F11-P1 | **Incident report**: A short report form tied to the trip, with optional audio and photos. | interface | small | [KIT-A10-F3-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A5-F3-P2 |  |
| RIDE-A13-F11-P2 | **Safety team plan**: How the co-op staffs a response team at all hours on the kit's urgent safety line, what it can do, when it calls Police and how responders are supported, planned so other marketplaces can share it. | research | medium | [KIT-A10-F1-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A21-F3-P1](../kit/FEATURES.md#kit-a21-help-and-member-support), [KIT-A10-F6-P4](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A13-F11-P3 | **Sexual harm reports**: A separate, trauma-informed path on the kit's sexual harm path: only trained responders see the report, the reporter is referred to specialist support and decides whether Police are told, and the trip record is kept. | service | small | [KIT-A10-F18-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F18-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A13-F11-P1 |  |
| RIDE-A13-F12-P1 | **Ride crash report**: Records a crash with the trip, vehicle, people on board and photos on the kit's violence and crash handling, looks after the rider and sends the details to the driver's insurer. | interface | small | [KIT-A24-F4-P1](../kit/FEATURES.md#kit-a24-health-and-safety-at-work), RIDE-A5-F3-P2, RIDE-A2-F5-P1 | [RD02](STATUS.md) |
| RIDE-A13-F13-P1 | **Speed limit alerts**: Warns when over the posted limit, from open speed limit data matched to the driver's position. | library | small | [KIT-A7-F4-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) |  |
| RIDE-A13-F13-P2 | **Driving feedback**: Private weekly feedback from phone sensors that only the driver sees. | interface | small | RIDE-A13-F13-P1 |  |
| RIDE-A13-F14-P1 | **Safety guides**: Short safety guides in help, in the example country's languages first (for New Zealand: English and te reo Māori). | data | small | RIDE-A16-F1-P1 |  |
| RIDE-A13-F15-P1 | **Drug and alcohol rule**: A zero-tolerance rule that no one drives for Spare Ride under the influence of alcohol or drugs, in the published rules and the driver agreement. | legal | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A2-F7-P2 |  |
| RIDE-A13-F15-P2 | **Impaired driver report**: A one-tap report during or just after a trip that the driver may be impaired, which points the rider to the emergency number if they are in danger and pauses offers to that driver until a person reviews it, under the safety pause rules. | service | small | RIDE-A15-F3-P1, RIDE-A13-F11-P1, RIDE-A15-F5-P4, RIDE-A6-F1-P1, RIDE-A13-F15-P1 |  |
| RIDE-A13-F16-P1 | **Dashcam rules**: Rules for drivers' own dashcams: notice to riders, what privacy law asks (example: New Zealand's Privacy Act) and how long footage may be kept. | legal | small | RIDE-A13-F7-P1, [KIT-A18-F5-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) | [RD01](STATUS.md) |
| RIDE-A13-F16-P2 | **Camera notice**: Tells riders on the driver card, before pickup, that the car records video or sound. | interface | small | RIDE-A5-F1-P3, RIDE-A2-F4-P1, RIDE-A13-F16-P1 |  |
| RIDE-A13-F16-P3 | **Footage upload**: Lets a driver or rider upload footage as evidence for an incident or a dispute, added to the evidence bundle and deleted when the case closes. | service | small | [KIT-A11-F2-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A13-F11-P1, RIDE-A15-F4-P2, RIDE-A13-F16-P1 |  |

## RIDE-A14 Accessibility

Rides for everyone: wheelchair accessible vehicles, assistance animals always carried, help to the car, apps that work with screen readers, deaf and hard of hearing drivers, booking by phone or by voice, disability fare subsidies (example: New Zealand's Total Mobility) and a simpler mode.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Wheelchair accessible rides | A wheelchair user books a vehicle with a ramp or hoist that is checked and fit for the job. | core | Accessible vehicle check; Accessible ride option; Accessible vehicle supply; Wheelchair trip training |
| **F2** Assistance animals always carried | A rider with an assistance animal is never refused, and drivers know it. | core | Assistance animal flag; Assistance animal rules; Refusal reports |
| **F3** Help to the car | A rider who needs a hand asks for a driver trained to help them to and from the car. | next | Assisted ride option; Assistance course |
| **F4** Works with screen readers and large text | A blind or low-vision person uses every screen with a screen reader, large text or high contrast. | core | Accessible ride screens; Testing with disabled people |
| **F5** Deaf and hard of hearing drivers | A deaf driver gets flashing offer alerts and text-only contact, and riders are told to message instead of calling. | core | Deaf driver mode |
| **F6** Book by phone call | A person without a smartphone books a ride by calling a phone line and gets updates by text. | core | Phone booking line; Phone booking screen; Text and relay booking |
| **F7** Disability fare subsidies | A rider with a disability fare subsidy card, such as New Zealand's Total Mobility card, gets their subsidised fare on Spare Ride. | next | Fare subsidy rules; Fare subsidy card link; Fare subsidy driver checks |
| **F8** Simple mode | An older or less confident rider uses a simpler app with bigger buttons and fewer steps. | later | Simple mode |
| **F9** Accessibility needs on every request | A rider's access needs travel with every booking so the driver can prepare. | core | Needs on request |
| **F10** Find the car without sight | A blind or low-vision rider gets sound and vibration cues as the car arrives and can ask the driver to come to them. | next | Arrival cues |
| **F11** Room for a mobility aid | A rider with a folding wheelchair, walker or mobility scooter is matched with a car that has room for it. | next | Mobility aid space |
| **F12** More time to get in | A rider who needs more time to board because of a disability sets it once in their needs, gets a longer free wait and never pays waiting charges. | core | Extended wait setting; Waiting charge refund |
| **F13** Read receipts with a screen reader | A blind or low-vision rider or driver reads their receipts, statements and logbook with a screen reader. | next | Accessible ride documents |
| **F14** Easy read and sign language | A person reads orientation, safety and key help pages in easy read or watches them in sign language (for New Zealand: New Zealand Sign Language). | later | Easy read and sign language versions |
| **F15** Drivers with an animal allergy | A driver with a documented allergy to animals is not sent assistance animal trips first, and the rider is not kept waiting or refused. | later | Allergy re-offer rule |
| **F16** Book by voice | A blind or low-vision rider books, checks and cancels a ride by talking, to the app or to their phone's own voice assistant, without needing to see the screen. | next | Voice booking; Phone assistant shortcuts |

### Parts: Accessibility

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A14-F1-P1 | **Accessible vehicle check**: Records and checks ramps, hoists and restraints, with photos, before a vehicle offers accessible rides. | data | small | RIDE-A2-F4-P1, RIDE-A2-F4-P4 |  |
| RIDE-A14-F1-P2 | **Accessible ride option**: A ride option matched only to checked accessible vehicles, priced the same as standard. | library | small | RIDE-A3-F3-P1, RIDE-A7-F5-P1, RIDE-A14-F1-P4 | [RD06](STATUS.md) |
| RIDE-A14-F1-P3 | **Accessible vehicle supply**: How many accessible vehicles each city needs and how the co-op helps drivers fund them. | research | medium |  |  |
| RIDE-A14-F1-P4 | **Wheelchair trip training**: Drivers who offer wheelchair trips complete ramp, hoist and wheelchair restraint training, tracked as a check with an expiry date before any wheelchair offer. | data | small | [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-and-training), [KIT-A2-F3-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), RIDE-A14-F1-P1 |  |
| RIDE-A14-F2-P1 | **Assistance animal flag**: Riders note an assistance animal so the driver knows before arriving. | data | small | RIDE-A1-F5-P1 |  |
| RIDE-A14-F2-P2 | **Assistance animal rules**: The assistance animal part of the access duties: which animals must be carried and what a driver may ask, with sources. | legal | small | RIDE-A20-F10-P1 | [RD01](STATUS.md) |
| RIDE-A14-F2-P3 | **Refusal reports**: A fast report path for refused riders, with action under the published rules. | service | small | RIDE-A15-F3-P1, RIDE-A14-F2-P2 |  |
| RIDE-A14-F3-P1 | **Assisted ride option**: A ride option for drivers who completed the assistance training. | library | small | RIDE-A3-F3-P1, RIDE-A7-F5-P1, RIDE-A14-F3-P2 |  |
| RIDE-A14-F3-P2 | **Assistance course**: A course on helping people with disabilities and older riders, made with disabled people's groups and run on the kit's training and certificates. | data | medium | [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-and-training) |  |
| RIDE-A14-F4-P1 | **Accessible ride screens**: Live maps, the offer countdown and trip updates made usable with screen readers, large text and high contrast, on the kit's accessible components. | library | medium | [KIT-A19-F2-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A19-F3-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) | [K04](../kit/STATUS.md) |
| RIDE-A14-F4-P2 | **Testing with disabled people**: Paid testing sessions with disabled riders and drivers before each release. | research | small | RIDE-A14-F4-P1, [KIT-A19-F2-P2](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RIDE-A14-F5-P1 | **Deaf driver mode**: Flash and vibration alerts, calls turned into a prompt to message, and a note to riders. | interface | small | RIDE-A6-F2-P2, RIDE-A12-F1-P1 |  |
| RIDE-A14-F6-P1 | **Phone booking line**: A phone number with a simple menu and a person to talk to, using a telephony provider at cost. | integration | medium | RIDE-A12-F2-P1, [KIT-A21-F4-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RIDE-A14-F6-P2 | **Phone booking screen**: A screen for co-op staff to book, change and track rides for callers. | interface | medium | RIDE-A3-F5-P1, RIDE-A12-F4-P1, RIDE-A20-F2-P4, RIDE-A21-F10-P1 |  |
| RIDE-A14-F6-P3 | **Text and relay booking**: People who cannot use a voice line book by text, or call through a relay service (example: New Zealand Relay), and staff are trained to take relay calls. | integration | small | RIDE-A14-F6-P1, [KIT-A21-F4-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RIDE-A14-F7-P1 | **Fare subsidy rules**: How each scheme approves operators, the subsidy, the card and reporting, region by region, starting with the councils that run New Zealand's Total Mobility. | research | medium |  |  |
| RIDE-A14-F7-P2 | **Fare subsidy card link**: Reads the rider's card, applies the subsidy and reports trips to the scheme. | integration | large | RIDE-A14-F7-P1, RIDE-A9-F2-P1, RIDE-A20-F2-P4 |  |
| RIDE-A14-F7-P3 | **Fare subsidy driver checks**: Tracks each driver's qualifications for each scheme, such as first aid and the NZQA unit standards Auckland Transport asks for under New Zealand's Total Mobility, as checks with expiry dates. | data | small | [KIT-A2-F2-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), [KIT-A2-F3-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-and-training), RIDE-A14-F7-P1 |  |
| RIDE-A14-F8-P1 | **Simple mode**: A pared-back booking flow with saved places first and large text. | interface | medium | RIDE-A14-F4-P1, RIDE-A1-F4-P1 |  |
| RIDE-A14-F9-P1 | **Needs on request**: Sends the rider's chosen needs with the request and shows them on the offer. | library | small | RIDE-A1-F5-P1, RIDE-A6-F2-P1 |  |
| RIDE-A14-F10-P1 | **Arrival cues**: Sound and vibration as the car nears and stops, and a standing request that the driver come to the door. | interface | small | RIDE-A5-F2-P1, RIDE-A1-F5-P1 |  |
| RIDE-A14-F11-P1 | **Mobility aid space**: Records boot space on vehicles and mobility aids on rider needs, and matches the two. | data | small | RIDE-A2-F4-P4, RIDE-A7-F5-P1, RIDE-A1-F5-P1 |  |
| RIDE-A14-F12-P1 | **Extended wait setting**: A need set once on the rider's profile, on the rider's word, that lengthens the free wait and holds off waiting charges and the no-show clock. | data | small | RIDE-A1-F5-P1, RIDE-A20-F10-P1 |  |
| RIDE-A14-F12-P2 | **Waiting charge refund**: A rider with the longer wait set who is charged for waiting gets it back in one tap, with no review needed. | library | small | RIDE-A14-F12-P1, RIDE-A9-F8-P1 |  |
| RIDE-A14-F13-P1 | **Accessible ride documents**: Receipts, earnings statements, flat-rate credit notices and logbook exports made as tagged, accessible PDFs on the kit's accessible PDFs. | library | small | [KIT-A19-F11-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), RIDE-A9-F9-P1, RIDE-A10-F4-P1, RIDE-A10-F2-P1, RIDE-A6-F6-P4 |  |
| RIDE-A14-F14-P1 | **Easy read and sign language versions**: Easy read and sign language versions, starting with New Zealand Sign Language, of driver orientation, safety guides and key help pages. | data | small | [KIT-A19-F8-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), RIDE-A2-F8-P1, RIDE-A13-F14-P1, RIDE-A16-F1-P1 |  |
| RIDE-A14-F15-P1 | **Allergy re-offer rule**: Offers trips with an assistance animal to another driver first when a driver has a documented allergy, so the rider is not kept waiting or refused, with a lawyer checking the rule. | legal | small | RIDE-A14-F2-P2, RIDE-A7-F5-P1 | [RD01](STATUS.md) |
| RIDE-A14-F16-P1 | **Voice booking**: Book, check and cancel a ride by speaking to the app: say where to, hear the fare and the driver's details read back, and confirm by voice. Asked for by a rider's friend who has lost her sight (r/cooperatives, 2026-10-19). | interface | medium | RIDE-A3-F1-P1, RIDE-A3-F5-P1, RIDE-A14-F4-P1, RIDE-A14-F4-P2 |  |
| RIDE-A14-F16-P2 | **Phone assistant shortcuts**: Book, check and cancel through the phone's own voice assistant (Siri on iPhone, Google Assistant on Android) using each platform's published shortcut and app action systems. How far this goes depends on the phone app approach. | integration | medium | RIDE-A14-F16-P1, [KIT-A20-F2-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), [KIT-A20-F4-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |

## RIDE-A15 Ratings, reports, disputes and moderation

How trust is kept fairly: two-way ratings with published rules, reports, disputes decided by member panels while money is held, deactivation only with reasons and appeal, blocks, and moderation of profiles and messages.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Two-way ratings | A rider and a driver rate each other after each trip, and ratings follow published rules. | core | Ride rating rules; Rating screen |
| **F2** Feedback tags and comments | A person adds tags such as "great conversation" or "unsafe driving" and a short comment. | next | Feedback tags; Comment checks |
| **F3** Report a problem | A rider or driver reports a problem with a trip or a person, and sees what happened next. | core | Report flow |
| **F4** Disputes decided by members | When a rider and driver disagree about money or conduct, the money stays held and a panel of members decides. | core | Dispute flow; Evidence bundle; Balanced panels; Settle before a panel; Dispute deadlines |
| **F5** Deactivation only with reasons and appeal | A driver or rider is removed only under a published rule, is told which rule, and can appeal to a member panel. | core | Deactivation rules; Appeal flow; Safety pause rules |
| **F6** Never matched again | A rider or driver chooses never to be matched with a particular person again. | next | Block after a trip |
| **F7** Profiles and messages kept clean | Profile photos, names and messages that break the rules are caught and handled by people. | core | Profile checks; Chat reports |
| **F8** Claim for damage or harm | A driver whose car was damaged by a rider, or who was hurt or threatened, makes a claim with evidence while any money stays held until a panel decides. | next | Damage claim; Claim rules and limits; Support after harm |
| **F9** Take a dispute further | A rider or driver who is not satisfied learns how to take a dispute outside the co-op. | next | Outside routes for rides |

### Parts: Ratings, reports, disputes and moderation

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A15-F1-P2 | **Ride rating rules**: Which ride ratings are set aside, such as those caused by traffic, the app's route or the fare, and what ratings may be used for, voted by members on the kit's rating thresholds. | data | small | [KIT-A9-F4-P1](../kit/FEATURES.md#kit-a9-ratings-and-reputation), [KIT-A9-F3-P1](../kit/FEATURES.md#kit-a9-ratings-and-reputation) |  |
| RIDE-A15-F1-P3 | **Rating screen**: Stars and optional tags in one step, skippable. | interface | small | [KIT-A9-F1-P2](../kit/FEATURES.md#kit-a9-ratings-and-reputation) |  |
| RIDE-A15-F2-P1 | **Feedback tags**: A short list of tags per side, each feeding a clear rule or simply thanks. | data | small | [KIT-A9-F1-P1](../kit/FEATURES.md#kit-a9-ratings-and-reputation) |  |
| RIDE-A15-F2-P2 | **Comment checks**: Flags abusive comments for a person to check before the other side sees them. | service | small | [KIT-A10-F6-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A15-F3-P1 | **Report flow**: Report categories, evidence and status updates, tied to the trip. | service | medium | [KIT-A10-F3-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A5-F3-P2 |  |
| RIDE-A15-F4-P1 | **Dispute flow**: Opens a dispute, keeps the money held and collects both sides' statements. | library | medium | `kit-market`, [KIT-A11-F1-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A9-F2-P1 |  |
| RIDE-A15-F4-P2 | **Evidence bundle**: Gathers the trip record, route, fare steps, chat, any shared recording and any uploaded footage for the panel. | library | small | [KIT-A11-F2-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A5-F3-P2, RIDE-A8-F8-P1, RIDE-A12-F1-P1 |  |
| RIDE-A15-F4-P4 | **Balanced panels**: Panels for a rider's complaint against a driver include riders or independent members, and anyone with a conflict of interest steps aside. | data | small | [KIT-A11-F4-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals) |  |
| RIDE-A15-F4-P5 | **Settle before a panel**: Both sides can offer and accept a settlement before a panel sits, on the kit's settlement offers. | library | small | [KIT-A11-F3-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A15-F4-P1 |  |
| RIDE-A15-F4-P6 | **Dispute deadlines**: Deadlines and reminders for each step of a ride dispute, on the kit's dispute clocks. | service | small | [KIT-A11-F6-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A15-F4-P1 |  |
| RIDE-A15-F5-P1 | **Deactivation rules**: The published list of what leads to a pause or removal, voted on by members. | data | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A9-F4-P1](../kit/FEATURES.md#kit-a9-ratings-and-reputation) |  |
| RIDE-A15-F5-P2 | **Appeal flow**: Lodge an appeal, see its status and get a reasoned decision. | interface | small | [KIT-A11-F5-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A15-F5-P1, RIDE-A15-F4-P4 |  |
| RIDE-A15-F5-P4 | **Safety pause rules**: A pause during an investigation has a set maximum length, the person is told it has started and why, a panel reviews it within set days, and a driver who is cleared gets back pay for the time lost under a voted rule. | data | small | RIDE-A15-F5-P1, [KIT-A11-F4-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), [KIT-A10-F8-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A15-F6-P1 | **Block after a trip**: Offers never to be matched again on the rating and report screens, using the kit's blocking in dispatch. | interface | small | [KIT-A10-F4-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A15-F1-P3 |  |
| RIDE-A15-F7-P1 | **Profile checks**: Checks names and photos against the rules, with a person deciding anything unclear. | service | small | [KIT-A10-F6-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A1-F3-P2 |  |
| RIDE-A15-F7-P2 | **Chat reports**: Lets a person report a message, which sends the chat to a moderator. | interface | small | [KIT-A8-F6-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A12-F1-P1 |  |
| RIDE-A15-F8-P1 | **Damage claim**: The driver claims for damage with photos and a repair quote, and the rider answers before a panel decides. | interface | small | [KIT-A11-F1-P2](../kit/FEATURES.md#kit-a11-disputes-and-appeals), [KIT-A11-F2-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A5-F3-P2 |  |
| RIDE-A15-F8-P2 | **Claim rules and limits**: What can be claimed, the evidence needed, how much can be held from the rider and the most charged without a panel, set by vote. | data | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A3-F10-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A15-F4-P1 |  |
| RIDE-A15-F8-P3 | **Support after harm**: How a driver hurt or threatened by a rider is supported: Police, accident cover such as New Zealand's ACC, the co-op's insurance and any claim against the rider. | research | small | RIDE-A13-F11-P1 | [RD02](STATUS.md) |
| RIDE-A15-F9-P1 | **Outside routes for rides**: Guidance on taking a ride dispute to each country's small claims body or transport regulator (example: New Zealand's Disputes Tribunal and NZTA), on the kit's outside routes. | data | small | [KIT-A11-F7-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), RIDE-A15-F4-P1 |  |

## RIDE-A16 Help, support and lost items

How people get help: a searchable help centre, help tied to a trip, lost items, fare reviews, driver support, the complaints register the law requires, and the staff tools behind it.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Help centre | A person searches plain help articles in their language. | core | Ride help articles; Help search |
| **F2** Help with a trip | A person picks a trip, says what went wrong, and gets an answer from a person or an automatic fix. | core | Trip help screens; Something went wrong |
| **F3** Lost items | A rider who left something behind contacts the driver through the app and arranges its return, with a return fee to the driver. | core | Lost item flow; Return fee; Unclaimed items |
| **F4** Fare review | A rider who thinks they were overcharged asks for a review and gets an automatic check first. | core | Fare review |
| **F5** Driver support | A driver gets help with payouts, documents, offers and the app from people who know the work. | core | Driver help hub |
| **F6** Complaints register | Every complaint is recorded and kept for the time the law requires, ready for inspection. | core | Complaints register; Register export |
| **F7** Support staff tools | A support worker sees the trip, the people and the history in one place and fixes things through the same commands as the app. | core | Ride view in the support console; Support roles |
| **F8** Support staffing at cost | The co-op runs support with enough people at a published cost per trip. | next | Support staffing plan |
| **F9** Help from an AI assistant | A person asks an AI assistant that can look up their trips and act through the same commands, with a person always one tap away. | later | Help assistant |

### Parts: Help, support and lost items

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A16-F1-P1 | **Ride help articles**: Short articles for riders and drivers, in the example country's languages first (for New Zealand: English and te reo Māori) and then in languages such as Samoan and Tongan, kept as open text files in the kit's help centre. | data | medium | [KIT-A21-F1-P1](../kit/FEATURES.md#kit-a21-help-and-member-support), [KIT-A19-F1-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RIDE-A16-F1-P2 | **Help search**: Search over the articles, also usable offline in the app. | interface | small | RIDE-A16-F1-P1, [KIT-A21-F1-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RIDE-A16-F2-P2 | **Trip help screens**: Choose a trip, choose the problem, add details and see the reply. | interface | small | [KIT-A21-F2-P1](../kit/FEATURES.md#kit-a21-help-and-member-support), RIDE-A5-F8-P1 |  |
| RIDE-A16-F2-P3 | **Something went wrong**: One entry on every trip that asks what happened and sends it to the right place: a safety incident, a sexual harm report, a report about a person, a fare review, a lost item or a dispute. | library | small | RIDE-A16-F2-P2, RIDE-A13-F11-P1, RIDE-A13-F11-P3, RIDE-A15-F3-P1, RIDE-A16-F4-P1, RIDE-A16-F3-P1, RIDE-A15-F4-P1 |  |
| RIDE-A16-F3-P1 | **Lost item flow**: Opens a masked contact with the driver for a set time after the trip and tracks the return. | service | small | RIDE-A12-F2-P1, RIDE-A12-F1-P1, [KIT-A21-F2-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RIDE-A16-F3-P2 | **Return fee**: A published fee paid to the driver for returning an item. | library | small | RIDE-A9-F2-P1 |  |
| RIDE-A16-F3-P3 | **Unclaimed items**: How long a driver holds an item nobody collects, when it goes to Police, and how it is disposed of. | legal | small | RIDE-A16-F3-P1 |  |
| RIDE-A16-F4-P1 | **Fare review**: Runs the route check and fare explainer, then refunds or sends to a person. | library | small | RIDE-A8-F6-P2, RIDE-A8-F8-P1, RIDE-A9-F8-P1 |  |
| RIDE-A16-F5-P1 | **Driver help hub**: A driver section of help with tickets for payouts, documents and safety. | interface | small | [KIT-A21-F2-P1](../kit/FEATURES.md#kit-a21-help-and-member-support), RIDE-A16-F1-P1 |  |
| RIDE-A16-F6-P1 | **Complaints register**: Records complaints and their outcomes and keeps them for the required period. | legal | small | [KIT-A21-F2-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) | [RD01](STATUS.md) |
| RIDE-A16-F6-P2 | **Register export**: Exports the register for an enforcement officer. | interface | small | RIDE-A16-F6-P1, `engine-pdf` | [RD01](STATUS.md) |
| RIDE-A16-F7-P1 | **Ride view in the support console**: Adds the trip, map, fare steps, messages and ride actions to the kit's support console, each action logged. | interface | medium | [KIT-A21-F2-P2](../kit/FEATURES.md#kit-a21-help-and-member-support), RIDE-A23-F4-P1, RIDE-A8-F8-P1 |  |
| RIDE-A16-F7-P2 | **Support roles**: What each support role may see and do, with least access by default. | data | small | [KIT-A1-F5-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A22-F5-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RIDE-A16-F8-P1 | **Support staffing plan**: Hours, staffing levels and cost per trip, published on the ledger. | research | small | [KIT-A13-F1-P2](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |
| RIDE-A16-F9-P1 | **Help assistant**: Lets the kit's help assistant look up Spare Ride trips and act through ride commands, with the person's own permissions and a person always one tap away. | service | medium | [KIT-A17-F7-P1](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F1-P1](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A21-F2-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |

## RIDE-A17 Businesses, families, taxis and fleets

Riding and driving as part of a group: family profiles, teen accounts, business accounts with invoices and policies, booking for guests, health and community transport, taxi companies joining, and fleet owners.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Family profile | A family shares one payment method, and the payer sees and can track family members' trips. | next | Ride family group; Family screens; Tracking safeguards |
| **F2** Teen accounts | A parent sets up an account for a teenager, with live tracking and extra safety checks on every trip. | later | Teen riding rules; Teen account |
| **F3** Business accounts | A business pays for staff rides on one monthly invoice, with spending rules and expense exports. | next | Ride business settings; Travel policy rules; Ride monthly invoice; Expense export; Business dashboard |
| **F4** Book for guests | A hotel, venue or clinic books rides for guests who have no account, paid by the organisation. | later | Guest booking screen |
| **F5** Health and community transport | A health provider or community group books and pays for rides for patients or members. | later | Health information rules; Partner billing |
| **F6** Taxi companies join | A taxi company brings its drivers and vehicles to Spare Ride while keeping its own brand on the car. | next | Taxi company onboarding; Taxi ride option; Taxi dispatch links |
| **F7** Fleet owners | A person who owns several vehicles assigns them to drivers and sees what each earns. | later | Fleet portal; Fleet earnings split; Fleet fairness rules; Traffic notices to the driver |

### Parts: Businesses, families, taxis and fleets

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A17-F1-P1 | **Ride family group**: A payer and family riders sharing a payment method, with the payer able to see and track family trips, built on the kit's delegated access. | data | small | [KIT-A1-F8-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A3-F8-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A17-F1-P2 | **Family screens**: Invite members, set the payer and see family trips. | interface | small | RIDE-A17-F1-P1 |  |
| RIDE-A17-F1-P3 | **Tracking safeguards**: An adult in a family group is told their trips can be seen, sees who can see them, and can leave the group alone at any time; teen accounts are the only exception. | library | small | RIDE-A17-F1-P1 |  |
| RIDE-A17-F2-P1 | **Teen riding rules**: Whether and how under-18s may ride alone in each country, starting with New Zealand, and what drivers must know. | legal | small | [KIT-A1-F8-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) | [RD01](STATUS.md) |
| RIDE-A17-F2-P2 | **Teen account**: A linked account that always shares trips with the parent and always uses a PIN. | library | small | RIDE-A17-F1-P1, RIDE-A13-F3-P1, RIDE-A13-F2-P1, RIDE-A17-F2-P1 |  |
| RIDE-A17-F3-P1 | **Ride business settings**: Billing and ride settings for an organisation, on the kit's organisation accounts. | data | small | [KIT-A1-F7-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A3-F1-P2](../kit/FEATURES.md#kit-a3-payments-and-payouts) |  |
| RIDE-A17-F3-P2 | **Travel policy rules**: Limits by time, place, ride option and amount, checked at booking. | library | small | RIDE-A17-F3-P1 |  |
| RIDE-A17-F3-P3 | **Ride monthly invoice**: Ride's trip lines on the kit's business accounts on credit, which charges once a month with one tax invoice. | service | small | [KIT-A3-F17-P1](../kit/FEATURES.md#kit-a3-payments-and-payouts), RIDE-A17-F3-P1 |  |
| RIDE-A17-F3-P4 | **Expense export**: Trip and invoice data in open formats that expense tools can import. | service | small | RIDE-A17-F3-P1 |  |
| RIDE-A17-F3-P5 | **Business dashboard**: Manage staff, policies, invoices and trips in a browser. | interface | medium | RIDE-A17-F3-P1, RIDE-A17-F3-P2, [KIT-A1-F7-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RIDE-A17-F4-P1 | **Guest booking screen**: Book and track rides for named guests from the business dashboard. | interface | small | RIDE-A17-F3-P5, RIDE-A3-F9-P1, RIDE-A20-F2-P4, RIDE-A21-F10-P1 |  |
| RIDE-A17-F5-P1 | **Health information rules**: What the health information privacy rules require when rides are booked for patients. | legal | small |  |  |
| RIDE-A17-F5-P2 | **Partner billing**: Billing to a funder, with only the details they are allowed to see. | library | small | RIDE-A17-F3-P1, RIDE-A17-F5-P1, RIDE-A20-F2-P4, RIDE-A21-F10-P1 |  |
| RIDE-A17-F6-P1 | **Taxi company onboarding**: Adds a company with its own operator licence, its drivers and vehicles; rank and street-hail work stays under the company's own licence and camera rules. | interface | medium | RIDE-A2-F1-P2, RIDE-A2-F2-P3, RIDE-A20-F2-P3 | [RD01](STATUS.md) |
| RIDE-A17-F6-P2 | **Taxi ride option**: A taxi ride option with an upfront fare or a meter, as the rules allow. | library | small | RIDE-A3-F3-P1, RIDE-A8-F1-P2 | [RD06](STATUS.md) |
| RIDE-A17-F6-P3 | **Taxi dispatch links**: Which taxi dispatch systems have open, licensable interfaces to connect to. | research | medium |  |  |
| RIDE-A17-F7-P1 | **Fleet portal**: Vehicles, assigned drivers, documents and earnings in a browser. | interface | medium | RIDE-A2-F4-P1, RIDE-A9-F10-P1 |  |
| RIDE-A17-F7-P2 | **Fleet earnings split**: Splits earnings between fleet owner and driver on terms both can see. | library | small | RIDE-A9-F10-P1 |  |
| RIDE-A17-F7-P3 | **Fleet fairness rules**: Rules that protect drivers renting vehicles, set by member vote. | data | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A17-F7-P4 | **Traffic notices to the driver**: Matches speed camera, red light and toll notices sent to a fleet vehicle's owner to whoever was driving it at the time, from trip and shift records, so the owner can name them. | library | small | RIDE-A17-F7-P1, RIDE-A5-F3-P2 |  |

## RIDE-A18 Co-op membership, voting and open costs

What makes Spare Ride a co-op: drivers join as members for free, vote on fares and rules, see the monthly cost ledger and where each trip's fee went, propose changes and elect panels.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Join as a member | A driver joins for free and gets one vote on Spare Ride's fares and rules; riders who take part are members too and vote on major changes. | core | Ride member classes |
| **F2** Vote on fares and rules | A driver votes on the fare formula, busy-time pricing, the fee and the rules, and the result changes the app. | core | Vote screens; Vote results to settings; Fare impact preview; Lessons from driver co-ops |
| **F3** Open cost ledger | Anyone sees what Spare Ride cost to run each month and what the fee paid for. | core | Ledger page; Ride cost lines |
| **F4** Where my fee went | A rider or driver taps the fee on any trip and sees which costs it covered. | core | Fee trace |
| **F5** Propose a change | A member writes a proposal, gathers support and puts it to a vote. | next | Proposal form |
| **F6** Panels and the board | Members see how dispute panels are drawn at random from trained members, join the trained pool, and vote in board elections. | next | Panel pool and board votes |
| **F7** Surplus lowers the next fee | Any surplus at the end of a period lowers the next period's fee, shown on the ledger. | next | Surplus rule |
| **F10** Clear rules on who decides | A member sees who set the first rules, who may vote on each Spare Ride matter, and how an urgent change is made and then confirmed by members. | core | Founding settings; Ride electorate; Urgent changes in Ride |
| **F11** Riders have a say | A rider answers surveys on fares, safety and service and sees the results, without a vote. | next | Rider surveys |

### Parts: Co-op membership, voting and open costs

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A18-F1-P1 | **Ride member classes**: Driver members and rider members as Spare Ride's classes on the kit's member register, with drivers voting on fares and rules, and the kit's active member check so members who stop riding or driving do not tip the balance. | data | small | [KIT-A14-F2-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), [KIT-A14-F1-P3](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), [KIT-A14-F2-P3](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal) | [K01](../kit/STATUS.md) |
| RIDE-A18-F2-P1 | **Vote screens**: Read a proposal, see its effect on example fares and vote. | interface | medium | [KIT-A12-F3-P2](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), RIDE-A18-F2-P3 |  |
| RIDE-A18-F2-P2 | **Vote results to settings**: Turns a passed Spare Ride vote into a settings change with a start date, using the kit's results to settings and only within the clearance held. | library | small | [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), `kit-ops`, RIDE-A20-F12-P3 | [RD06](STATUS.md) |
| RIDE-A18-F2-P3 | **Fare impact preview**: Shows what a proposed formula would do to typical trips and to driver earnings. | library | small | RIDE-A8-F1-P2, RIDE-A7-F10-P1 | [RD06](STATUS.md) |
| RIDE-A18-F2-P4 | **Lessons from driver co-ops**: What worked and failed at existing driver co-ops, and why, so fares and votes are designed with their lessons. | research | medium |  | [RD04](STATUS.md) |
| RIDE-A18-F3-P1 | **Ledger page**: The monthly ledger for Spare Ride as a public page and open data. | interface | small | [KIT-A13-F1-P3](../kit/FEATURES.md#kit-a13-open-cost-ledger) | [K06](../kit/STATUS.md) |
| RIDE-A18-F3-P2 | **Ride cost lines**: The running costs that belong to rides, such as maps, routing, support and insurance. | data | small | [KIT-A13-F1-P2](../kit/FEATURES.md#kit-a13-open-cost-ledger), [KIT-A13-F2-P2](../kit/FEATURES.md#kit-a13-open-cost-ledger) | [K06](../kit/STATUS.md) |
| RIDE-A18-F4-P1 | **Fee trace**: Links each trip's fee to that period's ledger lines. | library | small | RIDE-A8-F3-P2, RIDE-A18-F3-P1, [KIT-A13-F3-P2](../kit/FEATURES.md#kit-a13-open-cost-ledger) | [K06](../kit/STATUS.md) |
| RIDE-A18-F5-P1 | **Proposal form**: Write a proposal, see the support needed and track it. | interface | small | [KIT-A12-F1-P2](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RIDE-A18-F6-P1 | **Panel pool and board votes**: Members join the trained pool that the kit's panel selection draws from, and vote in the co-op-wide board elections on the kit's elections. | service | small | [KIT-A11-F4-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), [KIT-A12-F6-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-and-training) | [K01](../kit/STATUS.md) |
| RIDE-A18-F7-P1 | **Surplus rule**: Shows Spare Ride's surplus carried into the next fee, as worked out by the kit's fee setting. | library | small | RIDE-A8-F3-P2, [KIT-A13-F3-P1](../kit/FEATURES.md#kit-a13-open-cost-ledger) | [K06](../kit/STATUS.md) |
| RIDE-A18-F10-P1 | **Founding settings**: Who sets the first fare formula, fee and rules before any driver has voted, and the date by which members must confirm or change them. | legal | small | RIDE-A8-F1-P1, [KIT-A14-F1-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal) | [RD06](STATUS.md) |
| RIDE-A18-F10-P2 | **Ride electorate**: Who votes on what in Spare Ride: active drivers, fleet owners, taxi company drivers, each city's drivers on its own fares, and any votes riders join. | data | small | [KIT-A12-F5-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), [KIT-A12-F5-P2](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), RIDE-A18-F1-P1 | [K01](../kit/STATUS.md) |
| RIDE-A18-F10-P3 | **Urgent changes in Ride**: Which Ride rules the co-op-wide urgent change power in the constitution may touch, such as safety rules and new legal duties, each published at once and put to members to confirm within a set time. | legal | small | [KIT-A12-F5-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) | [K01](../kit/STATUS.md) |
| RIDE-A18-F11-P1 | **Rider surveys**: Non-binding surveys of riders on fares, safety and service, on the kit's consultations and surveys, with the results published. | service | small | [KIT-A12-F11-P2](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |

## RIDE-A19 Operations and admin

The tools the co-op's own staff and elected members use to run the service: launching a city, watching it live, reviewing documents, catching fraud, handling incidents and official requests, logging every admin action and testing with simulations.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Launch a city | The co-op picks a city, sets its areas, ride options and fares, and switches it on when enough drivers are ready. | core | First city decision; City settings; Launch checklist |
| **F2** Watch the service live | The operations team sees live requests, trips, wait times and drivers online on a map. | core | Operations map; Live measures |
| **F3** Review documents | A reviewer checks documents and photos the automatic checks could not, and approves or refuses with a reason. | core | Document review queue |
| **F4** Catch fraud | The co-op spots fake locations, duplicate accounts, stolen cards and trips faked to earn fares. | core | Location fraud rules; Duplicate account rules; Fake trip rules |
| **F5** Handle incidents | The safety team sees open incidents, contacts people, pauses accounts and works with Police when needed. | core | Incident console |
| **F6** Official requests for data | The co-op answers Police and regulator requests only with proper legal authority, and publishes how many it gets. | core | Ride records in official requests |
| **F7** Every admin action logged | Every action a staff member takes is an operation in the log that elected members can audit. | core | Ride staff roles |
| **F8** Uptime and monitoring | The co-op watches the service and publishes its uptime with the ledger. | core | Ride health checks |
| **F9** Test with simulated riders and drivers | The co-op runs a city full of simulated riders and drivers to test changes, train staff and give demos. | core | Simulated riders and drivers; Load tests |
| **F10** Supply and demand reports | Members see where and when riders went unserved and where drivers waited, to plan together. | next | Supply and demand report |
| **F11** Disaster mode | In a declared emergency, such as a Civil Defence emergency in New Zealand, operations staff switch an area into disaster mode: busy-time pricing off, areas paused or limited, free trips where members agreed, and official alerts shown. | next | Disaster mode switch; Official alerts; Drivers safe in emergencies |
| **F12** Tell drivers in an area | Operations staff send a notice to drivers online in a zone about a closure, an event or a safety warning. | core | Area notices |
| **F13** Give a trip to a chosen driver | Staff offer a phone booking, subsidised fare trip or wheelchair trip to a named driver, who can still decline. | next | Manual dispatch |
| **F14** Trips stay safe in an outage | When dispatch or payments are down for a city, trips under way finish, fares settle later, and riders and drivers are told plainly what is happening. | core | Outage rules for trips; Outage notices |
| **F15** Service in quiet areas | People in rural towns and other quiet areas can book rides, even where few drivers work. | later | Quiet area plan |

### Parts: Operations and admin

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A19-F1-P1 | **First city decision**: Ranks cities on riders, drivers, competition and rules, starting with New Zealand's. | research | small |  | [RD05](STATUS.md) |
| RIDE-A19-F1-P2 | **City settings**: Each city's areas, ride options, fares, fees and launch state as versioned settings. | data | small | RIDE-A11-F6-P1, RIDE-A8-F1-P3, `kit-ops` |  |
| RIDE-A19-F1-P3 | **Launch checklist**: The steps to launch a city, from driver numbers and support hours to the outside security review and the privacy impact assessment, tracked in one place. | interface | small | RIDE-A19-F1-P2, [KIT-A22-F5-P3](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin), RIDE-A21-F12-P1 |  |
| RIDE-A19-F2-P1 | **Operations map**: A live map with requests, drivers and trips, without showing riders' names. | interface | medium | RIDE-A11-F1-P1, [KIT-A6-F3-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A19-F2-P2 | **Live measures**: Wait times, unmatched requests, cancellations and drivers online, per area. | service | medium | RIDE-A5-F3-P2, [KIT-A6-F3-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A19-F3-P1 | **Document review queue**: A queue of checks for people, with the document, the rule and approve or refuse buttons. | interface | medium | RIDE-A2-F1-P2, RIDE-A2-F9-P2, [KIT-A2-F4-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A19-F4-P1 | **Location fraud rules**: Ride's thresholds for impossible jumps and simulated locations in driver traces, on the kit's faked location checks. | library | small | [KIT-A7-F4-P3](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), RIDE-A5-F1-P1 |  |
| RIDE-A19-F4-P2 | **Duplicate account rules**: Ride's rules on the kit's fraud signals for one person running several accounts, or a banned person returning. | library | small | [KIT-A10-F12-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A1-F1-P4](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A2-F1-P3](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A19-F4-P3 | **Fake trip rules**: Ride's rules on the kit's fraud signals for riders and drivers who collude to create fares. | library | small | [KIT-A10-F12-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A5-F3-P2 |  |
| RIDE-A19-F5-P1 | **Incident console**: Open incidents by urgency with the trip, the people and the actions taken. | interface | medium | [KIT-A10-F1-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A5-F3-P2, RIDE-A16-F7-P1, RIDE-A15-F5-P4 |  |
| RIDE-A19-F6-P1 | **Ride records in official requests**: Which Ride records, such as trip records, traces and recordings, may be shared under the kit's law enforcement process, and what the passenger rules let officers see. | legal | small | [KIT-A10-F9-P3](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) | [RD01](STATUS.md) |
| RIDE-A19-F7-P1 | **Ride staff roles**: What reviewers, the safety team and operations staff may see and do in Spare Ride, given and removed on the kit's admin console. | data | small | [KIT-A22-F6-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin), [KIT-A22-F5-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RIDE-A19-F8-P1 | **Ride health checks**: Checks and alerts for Spare Ride's own services, such as dispatch, quotes and live trips, on the kit's monitoring. | service | small | [KIT-A22-F1-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RIDE-A19-F9-P1 | **Simulated riders and drivers**: Rider and driver behaviours for simulated city runs, using the same commands as people on real map data, ready to move into a shared kit simulator. | library | medium | RIDE-A23-F4-P1, [KIT-A7-F3-P2](../kit/FEATURES.md#kit-a7-maps-places-and-live-location), [KIT-A16-F3-P3](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations) |  |
| RIDE-A19-F9-P2 | **Load tests**: Tests that a city's busiest hour runs smoothly before launch. | service | small | RIDE-A19-F9-P1 |  |
| RIDE-A19-F10-P1 | **Supply and demand report**: Monthly report of unmet requests and idle time by area and hour. | interface | small | RIDE-A19-F2-P2 |  |
| RIDE-A19-F11-P1 | **Disaster mode switch**: One logged switch per area that turns off busy-time pricing, pauses or limits areas and allows free or subsidised trips under a rule members set in advance. | library | small | RIDE-A19-F1-P2, RIDE-A8-F4-P1, [KIT-A22-F2-P2](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RIDE-A19-F11-P2 | **Official alerts**: Brings in Civil Defence and other official alerts for the areas served and shows them to riders and drivers there. | integration | small | [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A11-F6-P1 |  |
| RIDE-A19-F11-P3 | **Drivers safe in emergencies**: Rules that drivers may go offline or refuse trips into danger with no effect on them, and a check-in for drivers in an affected area. | data | small | RIDE-A6-F3-P2, RIDE-A13-F5-P2 |  |
| RIDE-A19-F12-P1 | **Area notices**: Sends a notice to drivers online in a chosen zone, read aloud if they are driving, with every notice logged. | service | small | RIDE-A11-F6-P1, [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), RIDE-A6-F12-P3 |  |
| RIDE-A19-F13-P1 | **Manual dispatch**: Offers a trip to a chosen driver with the reason logged; a decline carries no penalty and the trip goes back to normal dispatch. | library | small | RIDE-A7-F3-P1, RIDE-A19-F7-P1 |  |
| RIDE-A19-F14-P1 | **Outage rules for trips**: Trips under way finish on the phones, fares settle from the stored quote when service returns, and new bookings pause. | library | small | RIDE-A5-F5-P1, [KIT-A22-F3-P2](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RIDE-A19-F14-P2 | **Outage notices**: Tells riders and drivers in a city what is down and what happens to their trip and pay, by push, text and the status page. | service | small | [KIT-A21-F5-P1](../kit/FEATURES.md#kit-a21-help-and-member-support), RIDE-A12-F4-P1 |  |
| RIDE-A19-F15-P1 | **Quiet area plan**: How to serve rural towns and other quiet areas, such as scheduled-only service, wider pickup ranges and taxi partners. | research | small | RIDE-A19-F1-P1 |  |

## RIDE-A20 Legal duties and compliance

What the law asks of a ride platform in each country and how the app meets it, starting with New Zealand, the first country pack: the operator licence, registered passengers instead of cameras, trip records, driver identification, agreed fares, drivers' status, health and safety, insurance, plain terms, consumer law and anti-money-laundering checks.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Hold the operator licence | The co-op holds the operator licence each country asks of a ride app (example: New Zealand's small passenger service licence), with fit and proper people in control. | core | Operator licence application; Licence duties list |
| **F2** Registered passengers instead of cameras | Every rider is a registered passenger, so vehicles need no in-car camera, and the records the rules ask for are kept. | core | Registered passenger rule; Trip record retention; Registered passengers on every booking path; Booking path guard; No trips off the app |
| **F3** Driver identification rules met | The co-op confirms whether the in-app driver ID meets the identification rule or a physical card is also needed. | core | Driver identification rules |
| **F4** Fare agreed before the trip | The upfront fare meets the rule that the fare or its basis is agreed before the trip starts. | core | Fare agreement check |
| **F5** Drivers' status decided | The co-op decides whether drivers are employed worker-members or independent contractors, and the app follows that choice. | core | Drivers' status decision; Ride status settings; Gateway test check |
| **F6** Health and safety duties | The co-op meets its health and safety duties to drivers and riders. | core | Ride health and safety risks; Trip incidents in the register |
| **F7** Insurance in place | Passengers, drivers and the co-op are insured as the law and good sense require. | core | Ride insurance needs |
| **F8** Plain terms and policies | Riders and drivers get short, plain terms and a privacy policy that meet consumer law. | core | Rider terms; Privacy policy |
| **F9** Anti-money-laundering check | The co-op meets the anti-money-laundering duties online marketplaces have in each country (example: New Zealand's, since 1 June 2025), covering the risks rides bring. | core | Ride money-laundering risks |
| **F10** Duties to disabled riders | The service meets human rights law on access for disabled people and assistance animals. | core | Access duties |
| **F11** Rules tracked over time | The co-op notices when a law or rule changes and updates the app to match. | next | Rules watch |
| **F12** Fare vote cleared under competition law | Drivers vote on fares only in a way competition law allows, with any clearance from the competition regulator (example: New Zealand's Commerce Commission) in place before the first vote. | core | Ride fare vote and competition law; Competition regulator clearance; Lawful vote limits |

### Parts: Legal duties and compliance

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A20-F1-P1 | **Operator licence application**: What the transport regulator needs for the licence, starting with New Zealand's NZTA: who must pass the fit and proper check, and the New Zealand-resident representative. | legal | medium |  | [RD01](STATUS.md) |
| RIDE-A20-F1-P2 | **Licence duties list**: Every ongoing duty the licence brings, such as licence labels in vehicles, each linked to the part of the app that meets it. | legal | small | RIDE-A20-F1-P1 | [RD01](STATUS.md) |
| RIDE-A20-F2-P1 | **Registered passenger rule**: Makes sure no trip starts without identifying details for driver and rider, as the rule requires. | legal | small | RIDE-A1-F1-P4, RIDE-A5-F3-P2 | [RD01](STATUS.md) |
| RIDE-A20-F2-P2 | **Trip record retention**: Keeps the identifying details and trip records for the period the rules set, then deletes them unless another law needs them. | library | small | RIDE-A5-F3-P2, RIDE-A21-F3-P1 | [RD01](STATUS.md) |
| RIDE-A20-F2-P3 | **Registered passengers on every booking path**: A ruling for each way a trip can be booked (for a guest, by phone, by a hotel or clinic, a fare subsidy scheme, taxi rank and street hail) on whether every passenger is a registered passenger, and what the vehicle needs if not. | legal | medium | RIDE-A20-F2-P1 | [RD01](STATUS.md) |
| RIDE-A20-F2-P4 | **Booking path guard**: Stops any booking that would carry an unregistered passenger in a vehicle without an approved camera, and says why. | library | small | RIDE-A20-F2-P3, RIDE-A3-F5-P1 |  |
| RIDE-A20-F2-P5 | **No trips off the app**: A published rule that drivers and riders never use Spare Ride to set up trips outside it, since such a trip breaks the registered passenger rule and skips the sales tax the platform must collect. | legal | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A20-F2-P1 |  |
| RIDE-A20-F3-P1 | **Driver identification rules**: Whether a digital ID in the app meets the rule, or a physical card is also needed. | research | small |  | [RD01](STATUS.md) |
| RIDE-A20-F4-P1 | **Fare agreement check**: Confirms the upfront quote, extras and fare changes meet the fare rules. | legal | small | RIDE-A8-F2-P1, RIDE-A8-F6-P1 | [RD01](STATUS.md) |
| RIDE-A20-F5-P1 | **Drivers' status decision**: Applies the kit's worker status test to Spare Ride's drivers after the Supreme Court ruling and the 2026 amendment, with a lawyer, and records the choice. | legal | large | [KIT-A23-F1-P1](../kit/FEATURES.md#kit-a23-work-law-and-worker-status) | [RD01](STATUS.md) |
| RIDE-A20-F5-P2 | **Ride status settings**: Switches Ride's pay, work time, leave and agreement features on or off from the status set in the kit's worker status per app. | library | small | RIDE-A20-F5-P1, [KIT-A23-F1-P2](../kit/FEATURES.md#kit-a23-work-law-and-worker-status) |  |
| RIDE-A20-F5-P3 | **Gateway test check**: If drivers are contractors, checks every driver-facing rule, such as required orientation, selfie checks, rating thresholds, area rules and deactivation, against the 2026 gateway test: a written agreement, freedom to work for others, no required availability or a right to subcontract, no ending for declining work, and a chance to get advice. | legal | medium | RIDE-A20-F5-P1, [KIT-A23-F1-P1](../kit/FEATURES.md#kit-a23-work-law-and-worker-status) | [RD01](STATUS.md) |
| RIDE-A20-F6-P1 | **Ride health and safety risks**: Ride's risks under the kit's health and safety duties, such as fatigue, violence from riders, crashes and phone use while driving, and how each is managed. | legal | small | [KIT-A24-F1-P1](../kit/FEATURES.md#kit-a24-health-and-safety-at-work), RIDE-A20-F5-P1, RIDE-A6-F6-P1, RIDE-A6-F12-P1 | [RD01](STATUS.md) |
| RIDE-A20-F6-P2 | **Trip incidents in the register**: Crashes, assaults and serious injuries during trips go into the kit's incident register, each checked for whether it must be notified to the safety regulator (example: New Zealand's WorkSafe), which for road crashes is unverified. | service | small | [KIT-A24-F2-P1](../kit/FEATURES.md#kit-a24-health-and-safety-at-work), [KIT-A24-F2-P2](../kit/FEATURES.md#kit-a24-health-and-safety-at-work), RIDE-A13-F11-P1, RIDE-A13-F12-P1 |  |
| RIDE-A20-F7-P1 | **Ride insurance needs**: The cover rides add to the kit's co-op insurance, such as passenger liability and the platform's part in crashes, bought at cost. | legal | small | [KIT-A14-F17-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), RIDE-A2-F5-P1 | [RD02](STATUS.md) |
| RIDE-A20-F8-P1 | **Rider terms**: Rider terms in plain words, including the non-discrimination commitment and who sells each ride, checked against fair trading and consumer guarantees law. | legal | medium | [KIT-A18-F4-P2](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [KIT-A18-F8-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [KIT-A10-F19-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), RIDE-A8-F3-P4 |  |
| RIDE-A20-F8-P2 | **Privacy policy**: What is collected, why, for how long and who sees it, in plain words. | legal | small | RIDE-A21-F3-P1, [KIT-A18-F4-P2](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RIDE-A20-F9-P1 | **Ride money-laundering risks**: The ride risks the kit's anti-money-laundering programme must cover, such as stored ride credit, cash and fake trips used to move money. | legal | small | [KIT-A4-F5-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law) | [K02](../kit/STATUS.md) |
| RIDE-A20-F10-P1 | **Access duties**: What human rights law requires of a ride service for disabled people, including refusals and charges such as waiting fees, with sources. | legal | small |  | [RD01](STATUS.md) |
| RIDE-A20-F11-P1 | **Rules watch**: A list of the laws and rules Spare Ride depends on, each with an owner and a review date. | data | small | RIDE-A20-F1-P2 |  |
| RIDE-A20-F12-P1 | **Ride fare vote and competition law**: Applies the kit's competition law check to drivers voting on a shared fare formula, which may be cartel conduct under competition law if they are contractors (example: New Zealand's Commerce Act 1986), and picks the route that makes it lawful. | legal | medium | [KIT-A12-F4-P3](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), RIDE-A20-F5-P1 | [RD01](STATUS.md) |
| RIDE-A20-F12-P2 | **Competition regulator clearance**: If needed, an authorisation (example: under section 58 of New Zealand's Commerce Act), or a notice under any new collective bargaining regime for small businesses, before the first fare vote. | legal | medium | RIDE-A20-F12-P1 |  |
| RIDE-A20-F12-P3 | **Lawful vote limits**: Records which fare and fee questions members may vote on under the clearance held, so a vote outside it cannot be put. | data | small | RIDE-A20-F12-P1, [KIT-A12-F5-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) | [RD06](STATUS.md) |

## RIDE-A21 Privacy and data export

People own their data: download everything in open formats, delete what the law allows, keep data only as long as needed, protect locations, answer privacy requests, show drivers the data used about them and handle breaches.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Download my data | A rider or driver downloads all their data in open formats, including trips, routes, fares, ratings and messages. | core | Ride export contents; Export screen |
| **F2** Delete my data | A person deletes their account and data, and is told what must be kept by law and until when. | core | Ride deletion rules |
| **F3** Keep data only as long as needed | The co-op keeps each kind of data only as long as the law or the service needs, then deletes it automatically. | core | Retention schedule |
| **F4** Location privacy | A rider's exact addresses are hidden from the driver after the trip, and location history can be cleared. | core | Address masking; Location history controls |
| **F5** Privacy requests | A person asks to see or correct their personal information and gets an answer within the legal time. | core | Privacy request tracker |
| **F6** See the data used about me | A rider or driver sees every piece of data used in decisions about them, such as offers, ratings, fraud flags and pauses. | next | Driver decision data view; Rider decision data view; Ride decisions in the AI register |
| **F7** Breach response | If personal data leaks, the co-op tells the people affected and the privacy regulator (example: New Zealand's Privacy Commissioner) as the law requires. | core | Ride breach risks |
| **F8** Open travel data | The co-op publishes anonymised travel time and demand data for cities and researchers. | later | Anonymising methods; Open travel data; Indigenous data check |
| **F9** Face checks within the biometrics code | A driver or rider asked for a face check is told why, what happens to their face data and what other way they have, as the Biometric Processing Privacy Code requires. | core | Ride biometrics assessment; Face check notices; Limits on face data |
| **F10** Told when someone else gives us your details | A guest rider, trusted contact, staff member or patient whose details came from someone else is told who has them and why, as privacy law requires (example: New Zealand's Privacy Act since 1 May 2026). | core | Indirect collection map; Notice to the person |
| **F11** Driver locations kept private | Nobody can use the app, the API or published data to track a driver live or to find where they live. | core | Driver position blurring; Tracking and scraping limits |
| **F12** Privacy checked for the whole app | A rider or driver can read how the co-op checked the privacy risks of everything Spare Ride collects before launch. | core | Ride privacy impact assessment |

### Parts: Privacy and data export

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A21-F1-P1 | **Ride export contents**: Adds trips, routes as GPX, fares, earnings, ratings and messages to the kit's data export, with a readme for each file. | service | small | [KIT-A18-F1-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), RIDE-A5-F3-P2, RIDE-A9-F10-P1 |  |
| RIDE-A21-F1-P2 | **Export screen**: Ask for an export and download it when ready. | interface | small | RIDE-A21-F1-P1 |  |
| RIDE-A21-F2-P1 | **Ride deletion rules**: What deleting an account does to trips under way, held money, ride credit and open disputes, and which ride records the law keeps, on the kit's account deletion. | service | small | [KIT-A18-F2-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), RIDE-A21-F3-P1 |  |
| RIDE-A21-F3-P1 | **Retention schedule**: How long each kind of data is kept and why, from trip records to tax records. | data | small | [KIT-A18-F6-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) | [RD01](STATUS.md) |
| RIDE-A21-F4-P1 | **Address masking**: Shows drivers only the area of past pickups and drop-offs. | library | small | RIDE-A5-F3-P2 |  |
| RIDE-A21-F4-P2 | **Location history controls**: See and clear saved locations and recent places. | interface | small | RIDE-A3-F1-P2, RIDE-A1-F4-P1 |  |
| RIDE-A21-F5-P1 | **Privacy request tracker**: Logs access and correction requests about Ride data and tracks each legal deadline in working days with the kit's working day counter. | service | small | [KIT-A18-F3-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [KIT-A19-F9-P2](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A21-F2-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RIDE-A21-F6-P1 | **Driver decision data view**: Shows a driver their offer history, ratings used, rule versions and any automated decision, with reasons. | interface | medium | RIDE-A7-F4-P2, RIDE-A15-F1-P2, RIDE-A7-F4-P3 |  |
| RIDE-A21-F6-P2 | **Rider decision data view**: Shows a rider the ratings used, any fraud flag or pause, the rule behind it and the data it rested on, except where the law allows it to be withheld. | interface | small | RIDE-A15-F1-P2, RIDE-A19-F4-P2, [KIT-A10-F8-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RIDE-A21-F6-P3 | **Ride decisions in the AI register**: Adds each automated decision in Ride, such as fraud flags, face matches, check-ins, automatic fare reviews and translation, to the kit's AI use register, with a way to ask a person to review it. | service | small | [KIT-A17-F10-P1](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F10-P2](../kit/FEATURES.md#kit-a17-ai-access) |  |
| RIDE-A21-F7-P1 | **Ride breach risks**: Which Ride data would cause serious harm if leaked, such as live locations, home addresses and recordings, so the kit's breach response acts fast. | legal | small | [KIT-A18-F7-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RIDE-A21-F8-P1 | **Anonymising methods**: Proven methods that stop people being found in published travel data. | research | medium |  |  |
| RIDE-A21-F8-P2 | **Open travel data**: Regular releases of travel times and demand by area and hour as open data. | service | medium | RIDE-A21-F8-P1, RIDE-A21-F8-P3, RIDE-A11-F3-P3, RIDE-A21-F11-P1 |  |
| RIDE-A21-F8-P3 | **Indigenous data check**: Checks each release of travel data against indigenous data sovereignty principles, such as Māori data sovereignty in New Zealand, with advice from those communities, as well as against the anonymising methods. | research | small | RIDE-A21-F8-P1 |  |
| RIDE-A21-F9-P1 | **Ride biometrics assessment**: Ride's proportionality assessment for each of its face checks (driver sign-up, the right-driver selfie, rider checks and photo checks), using the kit's biometric code compliance. | legal | medium | [KIT-A2-F1-P4](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks), [KIT-A2-F1-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A21-F9-P2 | **Face check notices**: A plain notice before each face check, with another way to prove identity where the assessment calls for one. | interface | small | RIDE-A21-F9-P1, [KIT-A18-F5-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RIDE-A21-F9-P3 | **Limits on face data**: Keeps face results to the purpose assessed, never infers gender, mood or health from a face, and deletes face data on schedule. | library | small | RIDE-A21-F9-P1, [KIT-A2-F1-P3](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) |  |
| RIDE-A21-F10-P1 | **Indirect collection map**: Every Ride path that collects details from someone else, such as guest riders, trusted contacts, business staff, patients and phone bookings, with the notice each needs and any exception. | legal | small | [KIT-A18-F5-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RIDE-A21-F10-P2 | **Notice to the person**: Sends a short notice by text or email with a link to the privacy policy, unless an exception applies. | service | small | RIDE-A21-F10-P1, [KIT-A8-F4-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RIDE-A21-F11-P1 | **Driver position blurring**: Nearby cars shown to riders and the API are snapped to cells, delayed and capped in number, and hidden near where a shift starts or ends. | library | small | [KIT-A6-F3-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RIDE-A21-F11-P2 | **Tracking and scraping limits**: Rate limits and pattern checks on arrival time and nearby car queries, so no one can follow a driver. | service | small | [KIT-A16-F1-P3](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), [KIT-A22-F5-P4](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RIDE-A21-F12-P1 | **Ride privacy impact assessment**: A privacy impact assessment for all of Spare Ride on the kit's template, covering live location, trip traces, audio recordings, trusted contacts and demand maps. | legal | medium | [KIT-A18-F10-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), RIDE-A21-F9-P1 |  |

## RIDE-A22 Languages, countries and poor signal

International by design, with New Zealand as the first example country: many languages, starting with the example country's (for New Zealand: English and te reo Māori), local formats, country rule packs, payments and tax per country, local emergency numbers, local co-ops, data kept in region, and apps that cope with weak signal.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Many languages | A person uses the app in their language, starting with the example country's (for New Zealand: English and te reo Māori), with others added by volunteers. | core | Fluent speaker review |
| **F2** Local formats | A person sees money, distances, dates and addresses in the way their country writes them. | core | Ride units |
| **F3** Country rule packs | A new country's rules for licences, records, fares and work time are added as a pack without changing the core. | later | Ride country pack; Country research template; Trip levies and local permits; Platform work laws overseas |
| **F4** Payments and tax in each country | A rider pays in their own currency and the right tax is collected wherever the trip happens. | later | Tax per country |
| **F5** Local emergency numbers | The emergency button always calls the right number for the country the trip is in. | core | Emergency numbers check |
| **F6** Local co-ops | Drivers in another country run their own co-op on the same software, joined to the federation. | later | Ride for local co-ops |
| **F7** Data kept in region | A country's riders' and drivers' data stays in that country where the law asks. | later | Ride data placement |
| **F8** Works on weak signal | A rider or driver on a weak or patchy connection can still book, drive and finish trips. | core | Low-data mode; Weak signal tests |

### Parts: Languages, countries and poor signal

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A22-F1-P3 | **Fluent speaker review**: Paid review of Ride's text in each language by fluent speakers before each release, starting with te reo Māori in New Zealand, through the kit's member translation. | service | small | [KIT-A19-F1-P3](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RIDE-A22-F2-P1 | **Ride units**: Distances in kilometres or miles and fare rounding for each country, on the kit's local formats. | library | small | [KIT-A19-F1-P2](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RIDE-A22-F3-P1 | **Ride country pack**: The ride rules a country pack adds to the kit's country packs: licences, required records, fares and work time. | library | medium | [KIT-A19-F6-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), `kit-plugin`, RIDE-A6-F6-P1, RIDE-A21-F3-P1 |  |
| RIDE-A22-F3-P2 | **Country research template**: The questions to answer about each new country's ride rules, with sources. | research | small | RIDE-A22-F3-P1 |  |
| RIDE-A22-F3-P3 | **Trip levies and local permits**: Per-trip government levies and booking service or operator permits by state or city, such as those in Australian states, as part of each country pack. | data | small | RIDE-A22-F3-P1 |  |
| RIDE-A22-F3-P4 | **Platform work laws overseas**: Laws on platform work in other countries, starting with the EU Platform Work Directive 2024/2831, due in national law by 2 December 2026: a presumption of employment, open rules for automated management, human review and no processing of private conversations or off-shift data, and what Ride's audio recording and selfie checks would need there. | research | medium | RIDE-A22-F3-P1, [KIT-A18-F13-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RIDE-A22-F4-P1 | **Tax per country**: Ride's tax settings for each country on the kit's tax rules per country. | library | small | [KIT-A4-F7-P1](../kit/FEATURES.md#kit-a4-tax-invoices-and-money-law), RIDE-A22-F3-P1 |  |
| RIDE-A22-F5-P1 | **Emergency numbers check**: Takes emergency numbers from the kit's country packs, starting with New Zealand's, and checks them before each city launch. | data | small | [KIT-A19-F9-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RIDE-A22-F6-P1 | **Ride for local co-ops**: What a country's ride co-op takes up when it joins, on the kit's name licence and federation register. | legal | small | [KIT-A14-F12-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), [KIT-A14-F12-P2](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal) | [K01](../kit/STATUS.md) |
| RIDE-A22-F7-P1 | **Ride data placement**: Which Spare Ride data must stay in each country, placed by the kit's data location service. | service | small | [KIT-A18-F9-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RIDE-A22-F8-P1 | **Low-data mode**: Fewer location updates, smaller trip messages and text fallbacks when the connection is poor, on the kit's light mode for slow connections. | library | medium | [KIT-A19-F5-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), RIDE-A5-F5-P1, RIDE-A12-F4-P1 | [K04](../kit/STATUS.md) |
| RIDE-A22-F8-P2 | **Weak signal tests**: Tests every key flow on slow and dropping connections and cheap phones. | service | small | RIDE-A5-F5-P1 |  |

## RIDE-A23 Apps, platform and open access

The apps and the open ways in: rider and driver apps on phones, a web booking page, background location, one command API for people, AI and scripts, an MCP server, plugins, booking links for other sites, the Spareware look and self-hosting.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **F1** Rider and driver apps on phones | A rider or driver installs Spare Ride on an Android phone or an iPhone. | core | Rider app shell; Driver app shell; Ride app store listings |
| **F2** Book in a browser | A rider books a ride from any browser without installing anything. | next | Web booking page |
| **F3** Location in the background | A driver's location keeps updating while the app is in the background, without draining the battery. | core | Shift-long location; Battery tests |
| **F4** One command API | Everything a person can do in Spare Ride, a script or AI agent can do through the same commands. | core | Ride command set; Query API; API documentation; Ride events for webhooks |
| **F5** AI agents | A person lets an AI agent book rides or check earnings for them, with approval before it spends, and staff use agents for routine work, all through the same commands. | next | Ride agent rules; No automated offer taking; Agent labels and limits |
| **F6** Plugins | Members add checked plugins, such as a city's dispatch rules or a new report. | later | Plugin points |
| **F7** Booking links for other sites | A venue, event or website adds a Spare Ride booking button or link with the destination filled in. | next | Booking links; Booking widget |
| **F8** The Spareware look | Spare Ride looks and works like every Spareware app, with its own light colour. | core | Spare Ride design tokens |
| **F9** Run your own server | A member group runs its own Spare Ride server and connects it to the co-op. | later | Ride in the self-host package |

### Parts: Apps, platform and open access

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RIDE-A23-F1-P2 | **Rider app shell**: The rider app's navigation, screens and offline storage on top of the shared core. | interface | large | [KIT-A20-F2-P2](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), [KIT-A20-F1-P2](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), `kit-sync` | [K04](../kit/STATUS.md) |
| RIDE-A23-F1-P3 | **Driver app shell**: The driver app's navigation, screens and offline storage, built for use in a cradle. | interface | large | [KIT-A20-F2-P2](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), [KIT-A20-F1-P2](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), `kit-sync` | [K04](../kit/STATUS.md) |
| RIDE-A23-F1-P4 | **Ride app store listings**: Ride's own store needs, such as explaining background location and the ride-hailing rules, on top of the kit's app store rules. | legal | small | [KIT-A20-F8-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), RIDE-A23-F1-P2, RIDE-A23-F1-P3 | [K04](../kit/STATUS.md) |
| RIDE-A23-F2-P1 | **Web booking page**: The booking flow as a fast web page that works on any device. | interface | medium | RIDE-A3-F5-P1, RIDE-A11-F1-P1 |  |
| RIDE-A23-F3-P1 | **Shift-long location**: The update rate, accuracy and battery settings a driver's shift needs, kept running in the background with the kit's location and background permissions. | library | medium | [KIT-A20-F2-P3](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) | [K04](../kit/STATUS.md) |
| RIDE-A23-F3-P2 | **Battery tests**: Measures battery use over a full shift on common phones. | service | small | RIDE-A23-F3-P1 |  |
| RIDE-A23-F4-P1 | **Ride command set**: Every ride action as a typed command on the kit's operation layer, with permissions. | library | large | `kit-ops`, `kit-market`, [KIT-A15-F1-P3](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RIDE-A23-F4-P2 | **Query API**: Read access to trips, fares, earnings and settings with the caller's own permissions, with driver positions only ever blurred. | service | medium | RIDE-A23-F4-P1, [KIT-A16-F1-P1](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), RIDE-A21-F11-P1 |  |
| RIDE-A23-F4-P3 | **API documentation**: Open documentation of every command and query with examples. | data | small | RIDE-A23-F4-P1, RIDE-A23-F4-P2, [KIT-A16-F3-P2](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations) |  |
| RIDE-A23-F4-P4 | **Ride events for webhooks**: A published list of ride events sent through the kit's webhooks: booked, driver assigned, arrived, started, completed, cancelled and receipt ready. | data | small | [KIT-A16-F2-P1](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), RIDE-A3-F5-P2 |  |
| RIDE-A23-F5-P2 | **Ride agent rules**: Which ride commands need the person's approval, such as booking, tipping or cancelling with a fee, and default spending limits, using the kit's approval steps and agent limits. | library | small | [KIT-A17-F2-P1](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F4-P1](../kit/FEATURES.md#kit-a17-ai-access) |  |
| RIDE-A23-F5-P3 | **No automated offer taking**: A published rule that only the driver, in the driver app, can accept offers; scripts and agents may read offers and earnings but never take trips, to keep shares fair and drivers within the phone rule. | data | small | RIDE-A6-F3-P1, [KIT-A17-F4-P1](../kit/FEATURES.md#kit-a17-ai-access) |  |
| RIDE-A23-F5-P4 | **Agent labels and limits**: Labels bookings and messages an agent makes, keeps ratings, dispute statements and votes for people only, and treats pickup notes and chat as data an agent never obeys. | library | small | [KIT-A17-F9-P1](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F9-P2](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F5-P1](../kit/FEATURES.md#kit-a17-ai-access) |  |
| RIDE-A23-F6-P1 | **Plugin points**: The places in Spare Ride where a sandboxed plugin can add rules, screens or reports. | library | medium | `kit-plugin`, [KIT-A16-F5-P2](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), RIDE-A23-F4-P1 |  |
| RIDE-A23-F7-P1 | **Booking links**: Links that open a booking with pickup and destination filled in. | library | small | RIDE-A3-F5-P1, [KIT-A20-F4-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RIDE-A23-F7-P2 | **Booking widget**: A small booking box other sites can embed. | interface | small | RIDE-A23-F2-P1 |  |
| RIDE-A23-F8-P1 | **Spare Ride design tokens**: Colours, type and parts from the Spareware design guide, with Spare Ride's own colour. | library | small | [KIT-A20-F1-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RIDE-A23-F9-P1 | **Ride in the self-host package**: Adds Spare Ride's services to the kit's self-host package, with a guide for running a city. | service | small | [KIT-A22-F7-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |

## What Spare Ride needs from the kit

Spare Ride depends on these kit parts earlier than the [kit map](../kit/FEATURES.md) plans them. The kit's priorities are the kit owner's call; until they change, these Ride parts wait.

| Ride part | Ride priority | Kit part | Kit priority |
| --- | --- | --- | --- |
| RIDE-A2-F4-P1 Vehicle record | core | [KIT-A2-F6-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) Vehicle checks | next |
| RIDE-A2-F7-P5 Signing the agreement | core | [KIT-A25-F1-P2](../kit/FEATURES.md#kit-a25-signatures-and-training) Signatures with audit trail | next |
| RIDE-A2-F12-P3 Take my driving record | core | [KIT-A9-F7-P1](../kit/FEATURES.md#kit-a9-ratings-and-reputation) Portable reputation | next |
| RIDE-A6-F7-P1 Demand map service | next | [KIT-A6-F9-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) Demand maps | later |
| RIDE-A6-F13-P1 Voice commands for drivers | next | [KIT-A17-F8-P1](../kit/FEATURES.md#kit-a17-ai-access) Voice control | later |
| RIDE-A8-F5-P3 Cleaning claim | core | [KIT-A5-F15-P2](../kit/FEATURES.md#kit-a5-marketplace-engine) Extra charges after the job | next |
| RIDE-A9-F2-P2 Hold top-up | core | [KIT-A5-F15-P1](../kit/FEATURES.md#kit-a5-marketplace-engine) Order changes | next |
| RIDE-A11-F4-P1 Driver navigation screen | core | [KIT-A7-F6-P1](../kit/FEATURES.md#kit-a7-maps-places-and-live-location) Turn-by-turn guidance | next |
| RIDE-A13-F4-P1 Right-driver checks | core | [KIT-A2-F12-P2](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) Repeat face checks | next |
| RIDE-A13-F4-P1 Right-driver checks | core | [KIT-A2-F12-P1](../kit/FEATURES.md#kit-a2-identity-licence-and-business-checks) Face check fairness testing | next |
| RIDE-A13-F5-P1 Ride check-in rules | core | [KIT-A10-F16-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) Trip check-ins | later |
| RIDE-A19-F4-P2 Duplicate account rules | core | [KIT-A10-F12-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) Fraud signals | next |
| RIDE-A19-F4-P3 Fake trip rules | core | [KIT-A10-F12-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) Fraud signals | next |
| RIDE-A21-F12-P1 Ride privacy impact assessment | core | [KIT-A18-F10-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) Privacy impact assessments | next |
| RIDE-A22-F8-P1 Low-data mode | core | [KIT-A19-F5-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) Light mode for slow connections | next |

The ones that hold back Spare Ride's first release most are adding a vehicle (KIT-A2-F6), turn-by-turn directions (KIT-A7-F6), light mode for slow connections (KIT-A19-F5) and trip check-ins (KIT-A10-F16); demand maps (KIT-A6-F9) and voice control (KIT-A17-F8) hold back parts marked next.

These were found while mapping Spare Ride but belong in the kit, because other apps need them too. Until the kit maps them, Spare Ride keeps its own part, named in brackets:

- A shared city simulator, with simulated customers and workers driving the real commands on real map data, so Ride and Spare Food stop mapping one each (RIDE-A19-F9-P1).
- One online state and one work-time total across Spareware apps, including Spare Food deliveries offered between rides, on the kit's worker availability and work-time limits (RIDE-A6-F6-P6).
- Matching a patchy GPS trace to roads to rebuild the distance travelled, which Spare Food couriers paid by distance also need (RIDE-A5-F5-P3).
- Map data for public transport timetables, road closures and works, speed limits, and toll and airport fees (RIDE-A4-F6-P1, RIDE-A11-F9-P1, RIDE-A13-F13-P1, RIDE-A8-F5-P1).
- The choice of telephony provider for masked calls, phone support and phone booking, made once for every app (RIDE-A12-F2-P2 now lists only what Ride needs).
- A safety response team on call at all hours, shared by the marketplaces, with its staffing, powers and cost on the ledger, and wellbeing support for safety responders and incident reviewers as well as moderators (RIDE-A13-F11-P2).
- The urgent rule change power, written into the constitution's voting rules (RIDE-A18-F10-P3 now says only which Ride rules it may touch).
- A watch list of the laws each app depends on, and a tracker for privacy requests against their legal deadlines (RIDE-A20-F11-P1, RIDE-A21-F5-P1).
- Unclaimed money: payouts that cannot be delivered and dormant credit, and who must pay them to the tax office (RIDE-A2-F12-P4).
- Leave requests, rosters and time and wage records for apps whose workers are employees (RIDE-A10-F6-P4 to P6), and platform work laws overseas such as the EU Platform Work Directive (RIDE-A22-F3-P4).
- Pickup point data and the pickup finder, demand forecasts, the methods for publishing anonymous travel data, and crash detection, which Spare Food couriers and Spare Market pickups also need (RIDE-A11-F5, RIDE-A7-F8, RIDE-A21-F8-P1, RIDE-A13-F6).
- Disaster mode and official alerts, which Spare Stay also brings in (RIDE-A19-F11).
- The routing engine choice, piece RD03, which the kit's routing part already points at and which would sit better on the kit's work board.

## Core research and legal parts with no piece yet

These parts block the first release or the first fare vote, but no piece on the [work board](STATUS.md) covers them yet, so no one can pick them up. Adding pieces for them is the owner's call.

| ID | Part | Kind |
| --- | --- | --- |
| RIDE-A1-F1-P5 | Minimum rider age | legal |
| RIDE-A2-F3-P2 | Verification provider choice | research |
| RIDE-A2-F7-P3 | Agreement in drivers' languages | legal |
| RIDE-A2-F7-P4 | Non-discrimination commitment | legal |
| RIDE-A8-F3-P5 | Payment fee and surcharge rules | legal |
| RIDE-A12-F2-P2 | Ride call needs | research |
| RIDE-A13-F11-P2 | Safety team plan | research |
| RIDE-A13-F15-P1 | Drug and alcohol rule | legal |
| RIDE-A14-F1-P3 | Accessible vehicle supply | research |
| RIDE-A14-F4-P2 | Testing with disabled people | research |
| RIDE-A16-F3-P3 | Unclaimed items | legal |
| RIDE-A20-F2-P5 | No trips off the app | legal |
| RIDE-A20-F8-P1 | Rider terms | legal |
| RIDE-A20-F8-P2 | Privacy policy | legal |
| RIDE-A20-F12-P2 | Competition regulator clearance | legal |
| RIDE-A21-F7-P1 | Ride breach risks | legal |
| RIDE-A21-F9-P1 | Ride biometrics assessment | legal |
| RIDE-A21-F10-P1 | Indirect collection map | legal |
| RIDE-A21-F12-P1 | Ride privacy impact assessment | legal |

## Rules found while mapping

Researched on 2026-10-18 for New Zealand, the first country pack. Each country gets its own pack. Not legal advice; piece [RD01](STATUS.md) confirms each one.

- **Work time.** Passenger drivers doing short fares may work 7 hours before a 30-minute break (5.5 hours on fares over 100 km), at most 13 hours in a work day followed by a 10-hour break, and 70 hours before a 24-hour break. Work time goes in an approved logbook, paper or electronic. ([NZTA](https://nzta.govt.nz/commercial-driving/commercial-safety/work-time-and-logbook-requirements/), [Uber NZ driver guidance](https://www.uber.com/nz/en/drive/requirements/regulatory))
- **Cameras or registered passengers.** In listed urban areas a passenger service vehicle needs an approved camera unless it carries only registered passengers, such as those registered through an app. Identifying details for driver and passenger, and a trip record, are kept for 168 hours. ([NZTA](https://www.nzta.govt.nz/commercial-driving/taxis-shuttles-buses-and-other-passenger-services/new-passenger-service-rules-2017/in-vehicle-cameras-and-registered-passengers/))
- **Complaints and fares.** Operators keep a complaints record; the penalty rules point to keeping it for 2 years (*unverified*). The fare or its basis is agreed before the trip. ([Offences and Penalties Regulations](https://www.legislation.govt.nz/secondary-legislation/pco-drafted/2017/252/en/latest/), [Consumer Protection](https://consumerprotection.govt.nz/help-product-service/cars/taxis-and-other-ride-share-services))
- **Total Mobility.** Councils approve Total Mobility operators region by region. One NZ taxi app already takes Total Mobility cards in Auckland, so card payment in an app is possible (*unverified* for other regions). ([Times](https://www.times.co.nz/business/getting-around-just-got-easier-for-aucklanders-with-disabilities/), [Environment Canterbury](https://www.ecan.govt.nz/your-region/living-here/transport/public-transport-services/total-mobility/total-mobility-transport-providers-and-fares-in-canterbury))
- **Safety tools to match.** The incumbent offers a trip PIN, check-ins after unusual stops or detours, encrypted audio recording kept on the phone, trip sharing, and preferences that switch these on automatically. ([Uber newsroom](https://www.uber.com/newsroom/set-it-and-forget-it-how-to-maximize-your-safety-features))
- **Drivers voting on fares.** If drivers are independent contractors they compete with each other, so a vote that fixes their fare formula could be cartel conduct under the Commerce Act 1986, a crime since April 2021. The Commerce Commission can authorise collective bargaining under section 58, as it did for Tegel's poultry growers. A bill introduced on 8 December 2025 would add a notification route for collective bargaining by small businesses; whether it has passed is *unverified*. (Digital Policy Alert, event 37367; Lane Neave on collective bargaining authorisation)
- **Face checks.** Comparing faces is biometric processing under the Biometric Processing Privacy Code 2025. It applies to new processing from 3 November 2025 and to all processing from 3 August 2026, and asks for a proportionality assessment, notice, safeguards and limits on use. (Privacy Commissioner, Biometric Processing Privacy Code 2025; Simpson Grierson, 8 August 2025)
- **Phones while driving.** Under the Land Transport (Road User) Rule a driver may touch a mounted phone only briefly and infrequently, and must not write, send or read text messages while driving. (NZTA, Road User Amendment 2009 questions and answers)
- **Work time counts all paid work.** For small passenger service drivers, work time includes other paid work, not only time on one app. ([Uber NZ driver guidance](https://www.uber.com/nz/en/drive/requirements/regulatory))
- **Details from someone else.** Since 1 May 2026, information privacy principle 3A requires telling people when their information is collected from someone else, such as a guest rider or a trusted contact. (Bell Gully, Preparing for IPP 3A)
- **Texting 111.** NZ Police run 111 TXT for deaf and speech-impaired people who register first, so a voice-only emergency button leaves them out. (NZ Police, 111 TXT)
- **Receipts, labels and impaired drivers.** A receipt should show the driver's unique identification details and the vehicle's registration number. The small passenger service licence label goes in the front windscreen, above the registration label. A driver under the influence of alcohol or drugs is a matter for Police. ([Consumer Protection](https://consumerprotection.govt.nz/help-product-service/cars/taxis-and-other-ride-share-services))
- **Waiting fees for disabled riders.** The US Justice Department sued Uber in November 2021 over wait time fees charged to riders who need more time to board because of a disability. In July 2022 Uber agreed to waive them for riders who certify that need, and to refund past fees. In NZ the Human Rights Act 1993 bars offering a service on less favourable terms because of disability. ([US Justice Department](https://www.justice.gov/crt/case/united-states-v-uber-technologies-inc))
- **Total Mobility drivers.** Auckland Transport asks Total Mobility drivers for a P endorsement, first aid unit standards 6401 and 6402, and unit standards 1734, 1748, 1750 and 17593. Other councils' lists are *unverified*. ([Auckland Transport](https://at.govt.nz/bus-train-ferry/accessible-travel/total-mobility-scheme/become-a-total-mobility-provider))
- **Booking by phone.** Uber NZ has run a 0800 phone booking line, open at all hours, since 28 May 2024, so phone booking is day-one parity. ([Uber NZ](https://www.uber.com/nz/en/newsroom/0800gouber))
- **Vouchers.** Since 16 March 2026, gift cards sold to consumers must stay valid for at least three years. Whether ride credit counts as a gift card is *unverified*. ([Fair Trading (Gift Card Expiry) Amendment Act 2024](https://www.legislation.govt.nz/act/public/2024/35/en/latest/))
- **The gateway test.** The Employment Relations Amendment Act 2026 is in force from 21 February 2026. A contractor is outside employee rights only if all five gateway criteria are met. When the test applies to existing contracts is *unverified*. ([MinterEllisonRuddWatts](https://minterellison.co.nz/insights/employment-relations-amendment-act-is-a-new-era-of))
- **Platform work overseas.** EU member states must bring the Platform Work Directive 2024/2831 into their law by 2 December 2026. It presumes employment where a platform directs and controls the work, and sets rules for automated management. ([EUR-Lex](https://eur-lex.europa.eu/eli/dir/2024/2831/oj))
- **Overseas cards.** Stripe in NZ charges 2.7% + 30c on domestic cards and 3.7% + 30c on international ones, so visitors' fares carry a higher payment fee. ([Stripe](https://stripe.com/en-nz/connect/pricing))

## Questions for assessing this map and its demo

- Is there anything you do in Uber or a taxi app today, as a rider or a driver, that has no feature in this map?
- Could the first city launch with only the core features, or is something marked next that riders would miss on day one?
- Is the fare card clear in five seconds: the fare, the driver's share, the at-cost fee, the payment fee and sales tax?
- Would a driver trust the offer card: does it show everything needed to decide, and is declining clearly free?
- Can the dispatch rules be published in one page and still work fairly at airports, events and quiet times?
- Is the drivers' status decision (RD01) far enough along to build pay, work time and the driver agreement, or should those parts wait?
- Can the co-op get access to the official licence and vehicle registers, or does sign-up start with people reviewing documents?
- Should Spare Ride build its own approved electronic logbook, or have drivers use an existing approved one at first?
- Are the core safety tools at least as good as the incumbent's on day one: emergency button, trip sharing, PIN, check-ins and a response team?
- Does the first city have enough wheelchair accessible vehicles, and should phone booking and disability fare subsidies be in the first release?
- Once support staff, the safety team, insurance, telephony and maps are on the ledger, what does the at-cost fee per trip come to?
- Which kit services (payments, matching, maps, trust) must exist before Spare Ride can start, and who is building them?
- Are the part sizes believable, and which large parts (routing, address search, navigation, background location, payments, sales tax engine, the apps) carry the most risk?
- Does every screen in the demo come from what people need to do, with nothing copied from an incumbent's interface?
- Does anything here need a lawyer before it is built: cameras and registered passengers, recording audio, women preferences, teen accounts, cash, stored credit?
- Can drivers lawfully vote on a shared fare under competition law (example: New Zealand's Commerce Act), and does the co-op need the regulator's authorisation (example: the Commerce Commission) before the first vote (RIDE-A20-F12)?
- Do the face checks at sign-up and during shifts pass the Biometric Processing Privacy Code's proportionality test, and what other way is there for people who will not use their face (RIDE-A21-F9)?
- Who sells each ride, the co-op or the driver, and do the receipt, the tax invoices and the rider terms all match that choice?
- If drivers are contractors, does every driver-facing rule pass the 2026 gateway test check, and if RD01 decides they are employees, are pay records, leave and rosters (RIDE-A10-F6) moved to core before launch?
- Has the kit raised the parts Spare Ride's core depends on, such as adding a vehicle, turn-by-turn directions, light mode and trip check-ins, or does launch wait for them?
- Do the safety tools cover the worst harms from day one: the separate sexual harm path, impaired driver reports, and limits on safety pauses?
- Do disabled riders get the longer free wait with no waiting charges, and do receipts show the driver's identification and the vehicle's registration?
