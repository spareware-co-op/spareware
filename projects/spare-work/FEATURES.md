# Spare Work: everything to build

*Stage: Mapping. Made from [features.json](features.json), which is the source; edit that file and rebuild this page.*

Spare Work replaces Builderscrack, hipages, Upwork and Fiverr. A trades and freelance marketplace run for the people who do the work, who vote on its fees and rules. Customers post a job once and get free quotes, each stage's payment is held until it is signed off, licences are checked against public registers, and workers keep everything except the at-cost fee. This page maps everything a full replacement needs, top down: 22 areas, 153 features (things a person can do) and 397 buildable parts. 94 features are core, needed for a first usable release; the rest come next or later. Parts build on the shared kit and engines named in the root [ARCHITECTURE.md](../../ARCHITECTURE.md) and on each other. Parts tied to a work-board piece link to [STATUS.md](STATUS.md). The program's map is [ARCHITECTURE.md](ARCHITECTURE.md).

Spare Work keeps only its own rules. Where the shared kit already builds something, the Spare Work part says what Spare Work adds and depends on the kit part by its ID (KIT-...), listed in the [kit's feature map](../kit/FEATURES.md). Parts that only repeated a kit part were removed, so some IDs are missing; IDs are never reused or renumbered.

Priorities: **core** is needed for a first usable release, **next** comes soon after, **later** after that. Kinds: library, service, interface, data, integration, research, legal. Sizes: small, medium, large.

## Areas at a glance

| ID | Area | Features | Core | Parts |
| --- | --- | --- | --- | --- |
| WORK-A1 | [Sign-up and accounts](#work-a1-sign-up-and-accounts) | 6 | 5 | 22 |
| WORK-A2 | [Profiles, portfolios and credentials](#work-a2-profiles-portfolios-and-credentials) | 8 | 3 | 30 |
| WORK-A3 | [Posting a job](#work-a3-posting-a-job) | 10 | 6 | 35 |
| WORK-A4 | [Finding work and matching](#work-a4-finding-work-and-matching) | 7 | 5 | 20 |
| WORK-A5 | [Finding a worker](#work-a5-finding-a-worker) | 5 | 3 | 10 |
| WORK-A6 | [Fixed-price services](#work-a6-fixed-price-services) | 7 | 3 | 13 |
| WORK-A7 | [Quotes and proposals](#work-a7-quotes-and-proposals) | 8 | 5 | 21 |
| WORK-A8 | [Agreements, stages and changes](#work-a8-agreements-stages-and-changes) | 7 | 5 | 25 |
| WORK-A9 | [Held payments and payouts](#work-a9-held-payments-and-payouts) | 10 | 7 | 29 |
| WORK-A10 | [Doing the job](#work-a10-doing-the-job) | 8 | 6 | 22 |
| WORK-A11 | [Hourly and remote work](#work-a11-hourly-and-remote-work) | 6 | 0 | 14 |
| WORK-A12 | [Messages and notifications](#work-a12-messages-and-notifications) | 6 | 4 | 11 |
| WORK-A13 | [Invoices, tax and records](#work-a13-invoices-tax-and-records) | 7 | 5 | 17 |
| WORK-A14 | [Reviews and reputation](#work-a14-reviews-and-reputation) | 6 | 5 | 12 |
| WORK-A15 | [Disputes, defects and warranties](#work-a15-disputes-defects-and-warranties) | 7 | 4 | 16 |
| WORK-A16 | [Trust, safety and moderation](#work-a16-trust-safety-and-moderation) | 6 | 6 | 17 |
| WORK-A17 | [Running a trade or freelance business](#work-a17-running-a-trade-or-freelance-business) | 9 | 1 | 18 |
| WORK-A18 | [Membership, fees and voting](#work-a18-membership-fees-and-voting) | 5 | 3 | 8 |
| WORK-A19 | [Links to other programs, AI and plugins](#work-a19-links-to-other-programs-ai-and-plugins) | 7 | 4 | 15 |
| WORK-A20 | [Access for everyone: phones, offline, languages and countries](#work-a20-access-for-everyone-phones-offline-languages-and-countries) | 6 | 3 | 18 |
| WORK-A21 | [Privacy, data and legal duties](#work-a21-privacy-data-and-legal-duties) | 6 | 6 | 15 |
| WORK-A22 | [Support and administration](#work-a22-support-and-administration) | 6 | 5 | 9 |

## WORK-A1 Sign-up and accounts

One Spareware login with a customer side and a worker side, plus business accounts for companies, landlords and property managers who hire often.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A1-F1 Sign up as a customer | A person can sign up with a passkey or email and post their first job within minutes. | core | P1 Customer sign-up flow; P2 Post first, confirm after; P3 Confirmed contact before a job goes live; P4 Customer age rules |
| WORK-A1-F2 Sign up as a worker | A tradesperson or freelancer can join, choose their trades or skills and areas, and start quoting. | core | P1 Worker onboarding steps; P2 Trade and skill picker; P3 Identity check; P4 Own-business questions; P5 Payout account set-up; P6 Business check; P7 Worker age rules; P8 Right to work |
| WORK-A1-F3 Hire and work from one account | One person can hire and work from the same account and switch views. | core | P1 Roles on one account; P2 Role switcher |
| WORK-A1-F4 Business customer accounts | A company, landlord, school or property manager can hire with several staff, spending limits and approvals. | next | P1 Business customer roles; P2 Spending limits and approvals; P3 Purchase order numbers and cost codes; P4 Monthly statement for businesses |
| WORK-A1-F5 Sign in safely | A member can sign in on any device with the kit's sign-in, and payouts stay safe after a lost account is recovered. | core | P1 Kit sign-in in Spare Work |
| WORK-A1-F6 Pause or close an account | A member can pause taking work, or close their account and take their data with them. | core | P1 Pause availability; P2 Close with open-job checks; P3 When a sole trader dies |

### Parts for WORK-A1

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A1-F1-P1 | **Customer sign-up flow**: Spare Work's short sign-up on the kit's sign-up flow, asking only what a first job needs. | interface | small | KIT-A1-F1-P2 (Sign-up flow) |  |
| WORK-A1-F1-P2 | **Post first, confirm after**: A customer can write a job first and confirm their account at the end, without losing the draft. | interface | small | WORK-A1-F1-P1 (Customer sign-up flow), WORK-A3-F1-P2 (Job posting form) |  |
| WORK-A1-F1-P3 | **Confirmed contact before a job goes live**: Requires a phone number or email confirmed with the kit's checks before a customer's first job goes live, so workers know it is real. | library | small | KIT-A1-F1-P3 (Email and phone checks) |  |
| WORK-A1-F1-P4 | **Customer age rules**: Customers under 18 can post only small jobs, and those are paid through held stages like any other job, never the less protected path; what a person under 18 may agree to is checked against the contract law for minors first, with a parent or carer linked where needed. | legal | small | KIT-A1-F1-P5 (Minimum age at sign-up), KIT-A1-F8-P2 (Rules for parents and carers), WORK-A9-F1-P2 (Held funds per stage) |  |
| WORK-A1-F2-P1 | **Worker onboarding steps**: Guided steps for business details, trades, areas, licences and payout account, saved as the worker goes. | interface | medium | KIT-A1-F1-P2 (Sign-up flow), WORK-A2-F1-P1 (Profile data model) |  |
| WORK-A1-F2-P2 | **Trade and skill picker**: Picks trades and skills from the shared category tree, with search. | interface | small | WORK-A3-F2-P1 (Category tree) |  |
| WORK-A1-F2-P3 | **Identity check**: Sends the worker to the kit's identity check and keeps only the verified name and date of birth, which licence matching uses. | integration | small | KIT-A2-F1-P2 (Identity check link), KIT-A2-F1-P3 (Check results store) |  |
| WORK-A1-F2-P4 | **Own-business questions**: Asks and records that the worker runs their own business and sets their own prices and hours. | legal | small | WORK-A21-F4-P1 (Contractor law and the platform) |  |
| WORK-A1-F2-P5 | **Payout account set-up**: Puts the kit's payout set-up into the onboarding steps; a worker can quote before it is done but cannot be paid until it is. | integration | small | KIT-A3-F3-P1 (Payout set-up) |  |
| WORK-A1-F2-P6 | **Business check**: Confirms a worker's business through the kit's business number lookup (example: the NZ Business Number) and keeps its trader details, so customers see the legal entity behind each quote. | integration | small | KIT-A2-F7-P1 (Business number lookup), KIT-A2-F7-P2 (Trader details) |  |
| WORK-A1-F2-P7 | **Worker age rules**: A minimum age for each category: a teenager can offer simple jobs such as lawn mowing with a parent or carer linked, while licensed work, childcare and work with held money are for adults only; young workers also get the hazardous-work limits, such as, in New Zealand, no ride-on mowers or other heavy machinery under 15 (confirm the rule in the health and safety regulations). | legal | small | KIT-A1-F1-P5 (Minimum age at sign-up), KIT-A1-F8-P2 (Rules for parents and carers), WORK-A3-F2-P1 (Category tree), KIT-A24-F1-P1 (Health and safety duties) |  |
| WORK-A1-F2-P8 | **Right to work**: Uses the kit's visa entitlement check at sign-up and re-checks before a visa lapses, since many work visas tie a person to one employer and do not allow contracting on the side. | integration | small | KIT-A2-F11-P1 (Right-to-work rules), KIT-A2-F11-P2 (Visa entitlement check), WORK-A1-F2-P3 (Identity check) |  |
| WORK-A1-F3-P1 | **Roles on one account**: Declares Spare Work's customer, worker and business roles on the kit's roles per app. | data | small | KIT-A1-F5-P2 (Roles per app) |  |
| WORK-A1-F3-P2 | **Role switcher**: Switches between the customer view and the worker view in one tap. | interface | small | WORK-A1-F3-P1 (Roles on one account), KIT-A1-F5-P3 (Profile and roles page) |  |
| WORK-A1-F4-P1 | **Business customer roles**: Spare Work's roles on the kit's organisation model: requester, approver, site contact and accounts, and who may approve spending. | data | small | KIT-A1-F7-P1 (Organisation model), WORK-A1-F3-P1 (Roles on one account) |  |
| WORK-A1-F4-P2 | **Spending limits and approvals**: Jobs over a set amount wait for a named approver before a quote is accepted. | library | medium | WORK-A1-F4-P1 (Business customer roles), kit-ops |  |
| WORK-A1-F4-P3 | **Purchase order numbers and cost codes**: Adds purchase order numbers, sites and cost codes to jobs and invoices. | data | small | WORK-A1-F4-P1 (Business customer roles) |  |
| WORK-A1-F4-P4 | **Monthly statement for businesses**: One monthly statement of all jobs, stages and invoices for an organisation. | service | medium | WORK-A1-F4-P1 (Business customer roles), KIT-A3-F17-P1 (Business accounts on credit), WORK-A13-F1-P2 (Invoice generator) |  |
| WORK-A1-F5-P1 | **Kit sign-in in Spare Work**: Uses the kit's passkeys, second sign-in step, sessions and account recovery; Spare Work requires the second step for workers and pauses payouts for a safe period after a recovery. | integration | small | KIT-A1-F2-P1 (Passkey sign-in), KIT-A1-F3-P1 (Second sign-in step), KIT-A1-F3-P2 (Sessions and devices), KIT-A1-F4-P2 (Assisted recovery) |  |
| WORK-A1-F6-P1 | **Pause availability**: Hides a worker from matching and search while they are away, without losing anything. | library | small | KIT-A6-F2-P1 (Worker availability) |  |
| WORK-A1-F6-P2 | **Close with open-job checks**: Closing waits until held money, open stages and disputes are settled, offers a data export first, and ends the person's Spare Work membership, unless they are still a member through another app. | library | small | kit-ops, WORK-A21-F1-P1 (Full data export), KIT-A14-F8-P1 (Leaving) |  |
| WORK-A1-F6-P3 | **When a sole trader dies**: Uses the kit's after-death handling, so an estate's named contact can close the account, settle held money for work done and hand open jobs to a replacement worker. | library | small | KIT-A18-F11-P1 (After-death account handling), WORK-A8-F7-P1 (Settle the part done) |  |

## WORK-A2 Profiles, portfolios and credentials

Worker profiles that show checked facts: licences matched against public registers, insurance, past jobs done on the platform, and a portfolio.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A2-F1 Build a worker profile | A worker can show their business name, trades, areas served, about text, hours and how to reach them. | core | P1 Profile data model; P2 Profile editor; P3 Public profile page; P4 Areas served |
| WORK-A2-F2 Show a portfolio | A worker can add photos, videos, documents and past projects, including jobs done on Spare Work. | core | P1 Portfolio items; P2 Job and portfolio media; P3 Add a finished job; P4 Copied portfolio checks |
| WORK-A2-F3 Prove licences and registrations | A worker can link a licence or registration, and customers see it checked against the public register with its expiry date. | core | P1 Which trade and professional registers can be checked; P2 Registers each trade needs; P3 Re-checks and expiry alerts; P4 Lapsed licence on open jobs; P5 Licence rules per trade; P6 Credential badges; P7 Professional and other registers; P8 Match the licence to the person; P9 Named licence holder on each stage; P10 Licence holder is the one signed in |
| WORK-A2-F4 Show insurance and memberships | A worker can add public liability insurance and trade association memberships, checked by a person where no lookup exists. | next | P1 Documents with expiry; P2 Manual check queue; P3 Association membership lookups |
| WORK-A2-F5 Background checks | A worker can choose to show that they passed a criminal record check, with their consent; for care work and work with children it is required. | later | P1 Background check options; P2 Background check badge |
| WORK-A2-F6 Freelancer skills and qualifications | A remote worker can show skills, languages, qualifications and optional skill tests. | next | P1 Skills and languages; P2 Qualification check; P3 Open skill tests |
| WORK-A2-F7 Plain performance facts | A customer can see plain facts: jobs done, on-time stages, response time and repeat customers. | next | P1 Spare Work facts for the reputation summary; P2 Facts panel |
| WORK-A2-F8 Show a verified ownership tag | A business owned by a group that buyers' procurement targets favour (example: Māori- or Pasifika-owned businesses in New Zealand) can choose to show a verified tag, for business customers with progressive procurement targets. | later | P1 Ownership tag check; P2 Filter for business buyers |

### Parts for WORK-A2

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A2-F1-P1 | **Profile data model**: Business details, trades, skills, areas, hours and credentials for each worker or business. | data | medium | kit-core, KIT-A1-F5-P1 (Profile model) |  |
| WORK-A2-F1-P2 | **Profile editor**: Edits every part of the profile, with a preview of what customers see. | interface | medium | WORK-A2-F1-P1 (Profile data model) |  |
| WORK-A2-F1-P3 | **Public profile page**: The page customers see, with credentials, reviews, facts and portfolio. | interface | medium | WORK-A2-F1-P1 (Profile data model), WORK-A14-F3-P1 (Review display) |  |
| WORK-A2-F1-P4 | **Areas served**: Draws areas on a map or picks suburbs and towns, with a travel distance. | interface | medium | KIT-A7-F5-P1 (Service areas), WORK-A2-F1-P1 (Profile data model) |  |
| WORK-A2-F2-P1 | **Portfolio items**: Portfolio entries with media, a description, trade and rough cost. | data | small | kit-store, engine-media |  |
| WORK-A2-F2-P2 | **Job and portfolio media**: Spare Work's photo and video settings on the kit's file uploads: phone-sized copies, location data stripped from every copy others see, and a prompt to describe each image; for job evidence, the capture time and place are kept privately in the evidence store for disputes. | integration | small | KIT-A15-F7-P1 (File uploads), KIT-A19-F2-P3 (Image descriptions), engine-media, KIT-A15-F7-P3 (Photo metadata stripping), KIT-A11-F2-P1 (Evidence store) |  |
| WORK-A2-F2-P3 | **Add a finished job**: Turns a finished Spare Work job into a portfolio entry once the customer agrees. | library | small | WORK-A10-F6-P1 (Sign-off and fix requests), WORK-A2-F2-P1 (Portfolio items) |  |
| WORK-A2-F2-P4 | **Copied portfolio checks**: Checks new portfolio photos against photos already on Spare Work, and sends any match, or a firm's report that its work is shown by someone else, to a person to review. | library | medium | WORK-A2-F2-P1 (Portfolio items), WORK-A16-F2-P4 (Rights-owner reports) |  |
| WORK-A2-F3-P1 | **Which trade and professional registers can be checked**: Which trade and professional registers have public lookups, their terms and how often to re-check, as Spare Work's input to the kit's register checks. | research | small |  | [WK02](STATUS.md) |
| WORK-A2-F3-P2 | **Registers each trade needs**: Declares to the kit's register checks which registers each trade needs in each country, starting with New Zealand's registers for licensed building practitioners, electrical workers, and plumbers, gasfitters and drainlayers. | data | small | KIT-A2-F2-P1 (Public register checks), KIT-A2-F2-P2 (Check rules per app), WORK-A2-F3-P1 (Which trade and professional registers can be checked), WORK-A2-F3-P5 (Licence rules per trade) |  |
| WORK-A2-F3-P3 | **Re-checks and expiry alerts**: Uses the kit's re-check scheduler and adds a fresh check before each licensed stage starts. | library | small | KIT-A2-F3-P1 (Expiry and re-check scheduler), WORK-A2-F3-P2 (Registers each trade needs) |  |
| WORK-A2-F3-P4 | **Lapsed licence on open jobs**: Tells the customer and pauses licensed stages if the named licence holder's licence lapses during a job, with the option to replace the worker. | library | small | WORK-A2-F3-P3 (Re-checks and expiry alerts), WORK-A2-F3-P9 (Named licence holder on each stage), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A2-F3-P5 | **Licence rules per trade**: Which work needs which licence, registration or licence class, as an open rules table per country, covering, in the New Zealand pack, restricted building work, the other trades and the professions in WORK-A2-F3-P7, with exemptions such as the owner-builder exemption for restricted building work and the homeowner exemption for some electrical work. | data | medium | WORK-A2-F3-P1 (Which trade and professional registers can be checked), WORK-A3-F8-P1 (Reserved and prohibited services law) |  |
| WORK-A2-F3-P6 | **Credential badges**: Shows each credential on profiles and quotes with the kit's verified badge, the date checked, the licence class and a link to the register. | interface | small | KIT-A2-F5-P1 (Verified badge), WORK-A2-F3-P8 (Match the licence to the person) |  |
| WORK-A2-F3-P7 | **Professional and other registers**: Adds the registers for lawyers and conveyancers, registered valuers, real estate licensees, engineers, architects, cadastral surveyors, immigration advisers, financial advice providers, asbestos removal licence holders and security staff, within each register's terms. | data | medium | KIT-A2-F2-P1 (Public register checks), WORK-A2-F3-P1 (Which trade and professional registers can be checked), WORK-A2-F3-P5 (Licence rules per trade) |  |
| WORK-A2-F3-P8 | **Match the licence to the person**: Matches the name and details on each register entry to the worker's identity check, so nobody can quote someone else's licence number; a mismatch goes to a person to review. | library | medium | WORK-A1-F2-P3 (Identity check), KIT-A2-F2-P1 (Public register checks), KIT-A2-F4-P1 (Check review request) |  |
| WORK-A2-F3-P9 | **Named licence holder on each stage**: Each licensed stage names the licensed person doing or supervising it, with their licence class, so unlicensed crew can work under supervision and the record of work comes from that person. | data | small | WORK-A2-F3-P8 (Match the licence to the person), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A2-F3-P10 | **Licence holder is the one signed in**: Uses the kit's repeat identity checks before a licensed stage starts and when the named licence holder signs off their work, so a licensed account cannot be lent to unlicensed crew. | integration | small | KIT-A2-F12-P2 (Repeat face checks), WORK-A2-F3-P8 (Match the licence to the person), WORK-A2-F3-P9 (Named licence holder on each stage) |  |
| WORK-A2-F4-P1 | **Documents with expiry**: Uploads a certificate with its expiry date and reminds the worker to renew it. | library | small | WORK-A2-F2-P2 (Job and portfolio media), KIT-A2-F3-P1 (Expiry and re-check scheduler) |  |
| WORK-A2-F4-P2 | **Manual check queue**: Spare Work's rules for checking documents that have no public lookup, such as insurance certificates, on the kit's review queue; a shared document check belongs in the kit, since hosts and vehicles need it too. | library | small | KIT-A2-F4-P1 (Check review request), KIT-A10-F6-P2 (Review queue) |  |
| WORK-A2-F4-P3 | **Association membership lookups**: Checks trade association membership where the association offers an open lookup. | integration | medium | WORK-A2-F3-P1 (Which trade and professional registers can be checked) |  |
| WORK-A2-F5-P1 | **Background check options**: Which Spare Work categories need the kit's safety check: required for care in the home, childcare, tutoring and any other work with children before those categories open, optional for others such as cleaning inside homes, and how to show the result fairly. | research | small | KIT-A2-F9-P1 (Background checks in each country) |  |
| WORK-A2-F5-P2 | **Background check badge**: Shows the check type and date with the kit's verified badge, never the details. | interface | small | KIT-A2-F9-P2 (Background check link), KIT-A2-F5-P1 (Verified badge), WORK-A2-F1-P1 (Profile data model) |  |
| WORK-A2-F6-P1 | **Skills and languages**: Skills and spoken languages on the profile, from the shared category tree. | data | small | WORK-A2-F1-P1 (Profile data model), WORK-A3-F2-P1 (Category tree) |  |
| WORK-A2-F6-P2 | **Qualification check**: Uploads a qualification for a person to check. | library | small | WORK-A2-F4-P2 (Manual check queue) |  |
| WORK-A2-F6-P3 | **Open skill tests**: Optional skill tests written and reviewed by member workers, published openly. | research | medium |  |  |
| WORK-A2-F7-P1 | **Spare Work facts for the reputation summary**: Spare Work's facts for the kit's reputation summary: jobs done, on-time stages, response time and repeat customers, each worked out from real job records, never from claims. | library | medium | KIT-A9-F6-P1 (Reputation summary), kit-ops, kit-market |  |
| WORK-A2-F7-P2 | **Facts panel**: Shows the kit's reputation summary on the profile with Spare Work's facts, and how each one is worked out. | interface | small | WORK-A2-F7-P1 (Spare Work facts for the reputation summary), KIT-A9-F6-P1 (Reputation summary) |  |
| WORK-A2-F8-P1 | **Ownership tag check**: Checks the tag against an existing supplier-diversity register, with the business's consent, and shows it only if the business chooses. | integration | small | WORK-A1-F2-P6 (Business check), WORK-A2-F1-P1 (Profile data model) |  |
| WORK-A2-F8-P2 | **Filter for business buyers**: Lets business customers filter search results by the tag. | interface | small | WORK-A2-F8-P1 (Ownership tag check), WORK-A5-F1-P2 (Search page with filters) |  |

## WORK-A3 Posting a job

A customer describes what they need once, with photos, drawings, address, timing and budget, and the job reaches the right workers.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A3-F1 Describe a job | A customer can post a job with a title, description, trade, photos, location, timing and rough budget. | core | P1 Job data model; P2 Job posting form; P3 Photos and video on a job; P4 Address privacy; P5 Roles on a job; P6 Disclosed commissions and markups |
| WORK-A3-F2 Pick the right trade | A customer can pick the right trade or service from a clear list, or have one suggested from their description. | core | P1 Category tree; P2 Suggest a category; P3 Category changes by vote |
| WORK-A3-F3 Attach drawings and documents | A customer can attach plans, Spare Construct drawings, building consent documents and specifications. | next | P1 Document attachments and viewer; P2 Spare Construct drawing links; P3 Building consent details; P4 Does it need consent? |
| WORK-A3-F4 Help with scoping | A customer can see a rough price range and a checklist of what to ask before posting. | next | P1 Price ranges from past jobs; P2 Scoping checklists; P3 Licensed work notice |
| WORK-A3-F5 Choose who sees the job | A customer can send a job to all matching workers, to a shortlist, or straight to one worker they know. | core | P1 Open, invited or direct; P2 Invite by link |
| WORK-A3-F6 Manage a posted job | A customer can edit, pause, close or repost a job and see who has responded. | core | P1 Job states and history; P2 Customer job page; P3 Close stale jobs |
| WORK-A3-F7 Recurring and urgent jobs | A customer can mark a job as urgent so nearby available workers see it first, or post a repeating job such as monthly lawn mowing. | core | P1 Recurring jobs; P2 Urgent call-outs; P3 Danger first |
| WORK-A3-F8 Know what work can be offered | A customer and a worker can see which services may never be offered, and which need a licence, registration or age check, before posting. | core | P1 Reserved and prohibited services law; P2 Service rules as data; P3 Checks on jobs and services; P4 Rules pages; P5 Remote services stay within one country at first; P6 Boundaries with other programs; P7 Care work waits for safety checks |
| WORK-A3-F9 Describe a job by voice | A customer who cannot type easily can describe a job by speaking and check the text before posting. | later | P1 Voice to text for jobs |
| WORK-A3-F10 Rebuild after a disaster | After a flood or storm, a region can switch to disaster mode, with urgent safety work first, insurer-funded jobs and fair prices. | later | P1 Disaster mode; P2 Workers from other regions; P3 Price guidance against gouging |

### Parts for WORK-A3

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A3-F1-P1 | **Job data model**: A job on the kit's request model, with trade, description, media, place, timing, budget and visibility. | data | medium | KIT-A5-F2-P1 (Request model), kit-core |  |
| WORK-A3-F1-P2 | **Job posting form**: Guided questions per trade, so workers get what they need to quote. | interface | medium | WORK-A3-F1-P1 (Job data model), WORK-A3-F2-P1 (Category tree) |  |
| WORK-A3-F1-P3 | **Photos and video on a job**: Adds photos and short videos to a job from the phone camera, with a prompt to describe each image. | interface | small | WORK-A2-F2-P2 (Job and portfolio media) |  |
| WORK-A3-F1-P4 | **Address privacy**: Uses the kit's approximate location: only the suburb shows to everyone, and the exact address only to workers the customer chooses. | library | medium | KIT-A7-F10-P1 (Approximate location), WORK-A3-F1-P1 (Job data model) |  |
| WORK-A3-F1-P5 | **Roles on a job**: Names who pays, who approves, the owner for consent and the record of work, and the site contact who lets the worker in, so an adult child, a body corporate, a landlord or an insurer can each play their part. | data | medium | WORK-A3-F1-P1 (Job data model) |  |
| WORK-A3-F1-P6 | **Disclosed commissions and markups**: When the person who approves a job is not the one who pays, such as a property manager, body corporate manager or insurer's agent, they must disclose any commission, referral fee or markup they get from the job, and the payer sees it on the quote and invoice, as the law on secret commissions expects (example: New Zealand's Secret Commissions Act 1910). | legal | small | WORK-A3-F1-P5 (Roles on a job), KIT-A18-F8-P1 (Consumer law for platforms) |  |
| WORK-A3-F2-P1 | **Category tree**: Trades and services as open data, in many languages, with plain names. | data | medium | kit-core |  |
| WORK-A3-F2-P2 | **Suggest a category**: Suggests a category from the description for the customer to confirm, using a model run at cost. | library | medium | WORK-A3-F2-P1 (Category tree), KIT-A17-F6-P2 (Co-op AI at cost) |  |
| WORK-A3-F2-P3 | **Category changes by vote**: New or changed categories are proposed and voted on by members. | service | small | KIT-A12-F4-P1 (Results to settings), WORK-A3-F2-P1 (Category tree) |  |
| WORK-A3-F3-P1 | **Document attachments and viewer**: Attaches PDFs and images and views them on any device. | interface | medium | engine-pdf, KIT-A15-F7-P1 (File uploads) |  |
| WORK-A3-F3-P2 | **Spare Construct drawing links**: Links a Spare Construct drawing set through Spare Construct's public command API, never a code dependency or the AI channel. | integration | medium | KIT-A16-F1-P1 (Command API), WORK-A19-F1-P1 (Work command schemas) |  |
| WORK-A3-F3-P3 | **Building consent details**: Records the consent number, the council and the consent documents on the job. | data | small | WORK-A3-F1-P1 (Job data model) |  |
| WORK-A3-F3-P4 | **Does it need consent?**: Plain guidance on whether work needs a building consent or is exempt under the country's building law (example: Schedule 1 of New Zealand's Building Act, including the small stand-alone dwelling exemption), with a link to the council. | legal | medium | WORK-A3-F2-P1 (Category tree) |  |
| WORK-A3-F4-P1 | **Price ranges from past jobs**: Price ranges by trade and region from finished jobs, published as open data only for groups large enough that no customer or worker can be identified. | library | medium | WORK-A3-F1-P1 (Job data model), KIT-A5-F4-P1 (Order steps) |  |
| WORK-A3-F4-P2 | **Scoping checklists**: Short checklists per trade written by member workers. | data | medium | WORK-A3-F2-P1 (Category tree) |  |
| WORK-A3-F4-P3 | **Licensed work notice**: Tells the customer in plain words when the job needs a licensed or registered person, and matches only workers who hold it. | library | small | WORK-A2-F3-P5 (Licence rules per trade) |  |
| WORK-A3-F5-P1 | **Open, invited or direct**: Visibility settings for a job: open to matching workers, invited only, or direct to one worker. | library | small | WORK-A3-F1-P1 (Job data model), KIT-A5-F2-P1 (Request model) |  |
| WORK-A3-F5-P2 | **Invite by link**: Invites a named worker, already on Spare Work or not, with a link; an invite to someone who is not a member carries a named sender and one-step unsubscribe, as spam law asks. | library | small | KIT-A8-F4-P1 (Email sending), WORK-A3-F5-P1 (Open, invited or direct), KIT-A8-F5-P2 (Consent for messages) |  |
| WORK-A3-F6-P1 | **Job states and history**: Draft, open, quoting, agreed, in progress, done, paid outside the app and closed, each change an operation on the kit's order steps. | library | small | KIT-A5-F4-P1 (Order steps), WORK-A3-F1-P1 (Job data model) |  |
| WORK-A3-F6-P2 | **Customer job page**: Shows responses, questions, quotes and next steps for one job. | interface | medium | WORK-A3-F6-P1 (Job states and history) |  |
| WORK-A3-F6-P3 | **Close stale jobs**: Reminds the customer and then closes a job nobody has acted on, telling workers. | service | small | KIT-A8-F3-P2 (Notification router), WORK-A3-F6-P1 (Job states and history) |  |
| WORK-A3-F7-P1 | **Recurring jobs**: Repeating jobs on the kit's scheduled and repeat bookings, with the same worker until either side stops. | library | medium | WORK-A3-F1-P1 (Job data model), WORK-A17-F1-P1 (Calendar model), KIT-A5-F9-P1 (Scheduled and repeat bookings) |  |
| WORK-A3-F7-P2 | **Urgent call-outs**: Urgent jobs go to available workers nearby first, with any call-out charge shown upfront. | library | small | WORK-A4-F2-P1 (Work matching rules), KIT-A6-F3-P2 (Dispatch engine) |  |
| WORK-A3-F7-P3 | **Danger first**: For a gas smell, live wires or flooding, shows the emergency number (111 in New Zealand) and the gas or lines company's emergency number before any matching starts. | library | small | KIT-A10-F1-P3 (Helplines and emergency text), KIT-A19-F9-P1 (First country pack (example: New Zealand)), WORK-A3-F7-P2 (Urgent call-outs) |  |
| WORK-A3-F8-P1 | **Reserved and prohibited services law**: Every law in each country that reserves work to licensed people or bans a service, with sources, starting with New Zealand's: lawyers' reserved areas, immigration advice, real estate agency, financial advice, security work, asbestos removal, health practitioners' restricted activities and protected titles, veterinary work, auctioneers (registration under the Auctioneers Act 2013), debt collection and repossession, food businesses under the Food Act 2014 such as private chefs and caterers, passenger services (a small passenger service licence), goods moved for hire in heavier vehicles (a goods service licence), and contract cheating (an offence under the Education and Training Act 2020, reported as s393; confirm the section). | legal | medium |  |  |
| WORK-A3-F8-P2 | **Service rules as data**: Banned, restricted and licence-needed services as versioned rules linked to categories and keywords, changed by member vote. | data | medium | WORK-A3-F8-P1 (Reserved and prohibited services law), WORK-A3-F2-P1 (Category tree), KIT-A10-F5-P1 (Rules library) |  |
| WORK-A3-F8-P3 | **Checks on jobs and services**: Checks each job, service and quote against the rules: blocks banned work such as fake reviews, contract cheating and payment agent jobs that move money for others, and asks for the licence where the work needs one. | library | medium | WORK-A3-F8-P2 (Service rules as data), WORK-A2-F3-P5 (Licence rules per trade), KIT-A5-F1-P3 (Listing checks) |  |
| WORK-A3-F8-P4 | **Rules pages**: Plain pages listing what can and cannot be offered and why. | interface | small | WORK-A3-F8-P2 (Service rules as data) |  |
| WORK-A3-F8-P5 | **Remote services stay within one country at first**: Until the sales tax rule for remote services from overseas workers is live (WORK-A11-F6-P3), remote services such as a logo or a report are bought and sold only between people resident in the same country, so no overseas worker sells to a consumer in another country through the co-op. | legal | small | WORK-A3-F8-P3 (Checks on jobs and services), WORK-A1-F2-P3 (Identity check), WORK-A1-F2-P5 (Payout account set-up) |  |
| WORK-A3-F8-P6 | **Boundaries with other programs**: Which program owns overlapping work, agreed with Spare Ride, Food, Stay and Rent: rides and courier deliveries belong to Spare Ride or Spare Food and property management to Spare Rent, and Spare Work blocks or redirects those jobs, so 'drive me to the airport' never becomes an unlicensed passenger service. | data | small | WORK-A3-F8-P2 (Service rules as data), WORK-A3-F2-P1 (Category tree) |  |
| WORK-A3-F8-P7 | **Care work waits for safety checks**: Childcare, care in the home, tutoring and any other work with children stay out of the first release, and open only with the kit's safety check required for every worker in them. | legal | small | WORK-A3-F8-P2 (Service rules as data), WORK-A2-F5-P1 (Background check options) |  |
| WORK-A3-F9-P1 | **Voice to text for jobs**: Turns a spoken description into text with a model run at cost, for the customer to check and edit. | library | medium | WORK-A3-F1-P2 (Job posting form), KIT-A17-F6-P2 (Co-op AI at cost) |  |
| WORK-A3-F10-P1 | **Disaster mode**: A region switch on the kit's feature switches, set by a logged emergency action, that puts urgent safety jobs first and opens insurer-funded jobs; a shared emergency mode belongs in the kit, since Spare Ride, Stay and Market need the same switch. | library | medium | WORK-A22-F3-P1 (Voted settings), WORK-A3-F7-P2 (Urgent call-outs), WORK-A9-F9-P1 (Insurer as payer), KIT-A22-F2-P2 (Feature switches) |  |
| WORK-A3-F10-P2 | **Workers from other regions**: Lets workers from other regions serve the region, with licences and identity checked as usual before any quote. | library | small | WORK-A2-F3-P2 (Registers each trade needs), WORK-A2-F1-P4 (Areas served) |  |
| WORK-A3-F10-P3 | **Price guidance against gouging**: Shows the customer the region's normal price range next to each quote, as guidance only. | library | small | WORK-A3-F4-P1 (Price ranges from past jobs) |  |

## WORK-A4 Finding work and matching

Workers see jobs that fit their trades, areas and licences, for free. No paid leads, no tokens to quote, no paid placement.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A4-F1 Job feed for workers | A worker can browse open jobs that fit their trades, areas and licences, at no cost. | core | P1 Job feed with filters; P2 Map of jobs; P3 Saved searches |
| WORK-A4-F2 Fair matching | The platform sends each job to workers who fit, fairly, and nobody can pay to be seen first. | core | P1 Work matching rules; P2 Fair rotation; P3 Cap on quotes per job; P4 Published matching rules; P5 Explain and review automated decisions; P6 Automated decisions in the AI register |
| WORK-A4-F3 Job alerts | A worker can get push, email or text alerts for new jobs that fit them. | core | P1 Alert rules and quiet hours; P2 Daily digest |
| WORK-A4-F4 Show interest and ask questions | A worker can say they are interested, ask the customer questions or ask for a site visit before quoting. | core | P1 Interest, then choose; P2 Shared questions; P3 Decline with a reason; P4 Intellectual property check |
| WORK-A4-F5 Invitations to quote | A worker can receive a direct invitation to quote and accept or decline it. | core | P1 Invitation inbox |
| WORK-A4-F6 Capacity and availability | A worker can set how much work they can take and when, so they only see jobs they can do. | next | P1 Capacity settings; P2 Matching honours capacity |
| WORK-A4-F7 Subcontract and team up | A worker can find other trades to subcontract to or team up with on a bigger job. | later | P1 Worker-to-worker jobs; P2 Joint quotes |

### Parts for WORK-A4

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A4-F1-P1 | **Job feed with filters**: Lists matching jobs with filters for trade, distance, budget and timing. | interface | medium | WORK-A4-F2-P1 (Work matching rules) |  |
| WORK-A4-F1-P2 | **Map of jobs**: Shows jobs on a map at suburb level, never the exact address. | interface | medium | KIT-A7-F1-P2 (Map display), WORK-A3-F1-P4 (Address privacy) |  |
| WORK-A4-F1-P3 | **Saved searches**: Spare Work's job filters saved with the kit's saved searches, alerting on new jobs that fit. | library | small | KIT-A6-F8-P1 (Saved searches), WORK-A3-F1-P1 (Job data model) |  |
| WORK-A4-F2-P1 | **Work matching rules**: Spare Work's scoring rules as a plugin to the kit's matching: trade, area, licence and capacity only; declining a job never lowers a worker's place. | library | large | KIT-A6-F3-P3 (Matching rules per app), kit-plugin, WORK-A2-F1-P4 (Areas served), WORK-A2-F3-P5 (Licence rules per trade) |  |
| WORK-A4-F2-P2 | **Fair rotation**: Rotates who sees each job first, so new and small workers are seen and not only the busiest, checked by the kit's fairness checks. | library | medium | WORK-A4-F2-P1 (Work matching rules), KIT-A6-F5-P2 (Fairness checks) |  |
| WORK-A4-F2-P3 | **Cap on quotes per job**: Limits how many workers can quote one job, never below the customer protection floor, so customers are not flooded and workers do not waste time. | library | small | WORK-A4-F2-P1 (Work matching rules), WORK-A22-F3-P2 (Customer protection floors) |  |
| WORK-A4-F2-P4 | **Published matching rules**: Publishes Spare Work's matching rules and their voted values through the kit's published ranking and offer rules; no paid placement anywhere. | data | small | KIT-A6-F1-P3 (Published ranking rules), KIT-A6-F4-P2 (Fair offer rules), WORK-A4-F2-P1 (Work matching rules) |  |
| WORK-A4-F2-P5 | **Explain and review automated decisions**: Every automated decision about a worker's income, such as rotation, quote caps, auto-release, fraud holds and pauses, comes with a plain reason and a human review on request, and never uses grounds the law forbids. | library | medium | KIT-A6-F5-P1 (Match explanations), KIT-A10-F8-P1 (Statements of reasons), WORK-A4-F2-P1 (Work matching rules), KIT-A17-F10-P2 (Human review of AI decisions) |  |
| WORK-A4-F2-P6 | **Automated decisions in the AI register**: Lists each automated or AI decision Spare Work makes, such as category suggestions, rotation, quote caps, auto-release, fraud holds, fake review flags and copied photo checks, in the co-op's AI use register with what each may decide. | data | small | KIT-A17-F10-P1 (AI use register), KIT-A17-F10-P2 (Human review of AI decisions), WORK-A4-F2-P5 (Explain and review automated decisions) |  |
| WORK-A4-F3-P1 | **Alert rules and quiet hours**: Chooses which jobs trigger an alert and when alerts stay silent. | library | small | KIT-A8-F5-P1 (Notification settings), WORK-A4-F2-P1 (Work matching rules) |  |
| WORK-A4-F3-P2 | **Daily digest**: One summary a day instead of single alerts. | service | small | KIT-A8-F3-P2 (Notification router) |  |
| WORK-A4-F4-P1 | **Interest, then choose**: The worker shows interest and the customer chooses who to talk to, before details are shared; the flow and its name are our own, checked under WORK-A4-F4-P4. | library | medium | KIT-A5-F2-P1 (Request model), WORK-A3-F1-P1 (Job data model) |  |
| WORK-A4-F4-P2 | **Shared questions**: Questions and answers on a job, shown to all quoting workers when the customer allows. | library | small | WORK-A12-F1-P1 (Message threads) |  |
| WORK-A4-F4-P3 | **Decline with a reason**: A worker can pass on a job with a quick reason that helps the customer re-scope it. | library | small | WORK-A4-F4-P1 (Interest, then choose) |  |
| WORK-A4-F4-P4 | **Intellectual property check**: Searches the IPONZ patent register and others before design, since a trades site we replace markets a similar two-step connection flow. | legal | small |  |  |
| WORK-A4-F5-P1 | **Invitation inbox**: Lists invitations with accept and decline, and tells the customer the answer. | interface | small | WORK-A4-F4-P1 (Interest, then choose) |  |
| WORK-A4-F6-P1 | **Capacity settings**: Sets how many jobs at once, lead time and a booked-out mode. | library | small | WORK-A17-F1-P1 (Calendar model), KIT-A6-F2-P1 (Worker availability) |  |
| WORK-A4-F6-P2 | **Matching honours capacity**: Matching skips workers who are booked out until their chosen date. | library | small | WORK-A4-F2-P1 (Work matching rules) |  |
| WORK-A4-F7-P1 | **Worker-to-worker jobs**: A worker can post a job for other workers, such as a main contractor finding a tiler. | library | medium | WORK-A3-F1-P1 (Job data model) |  |
| WORK-A4-F7-P2 | **Joint quotes**: Several workers quote one job together on the kit's multi-party orders, each with their own stages and payouts. | library | large | WORK-A7-F1-P2 (Quote data model), WORK-A8-F2-P1 (Stage model), KIT-A5-F11-P1 (Multi-party orders) |  |

## WORK-A5 Finding a worker

Customers can search and browse workers directly by trade, place, checked licences, reviews and price, and compare them side by side.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A5-F1 Search for workers | A customer can search for workers by trade or skill, place, licence and rating. | core | P1 Profile search index; P2 Search page with filters; P3 Search in many languages; P4 Harvest limits |
| WORK-A5-F2 Trades directory | A customer can browse a directory of trades by region, such as plumbers in a given town. | next | P1 Directory pages; P2 Regions and suburbs |
| WORK-A5-F3 Compare workers | A customer can compare the workers who responded side by side: price, reviews, credentials and availability. | core | P1 Comparison view |
| WORK-A5-F4 Save and rehire | A customer can save favourite workers and send a new job straight to them. | core | P1 Favourites; P2 Rehire |
| WORK-A5-F5 Related trades | A customer can see which other trades a job usually needs, such as a plasterer after an electrician. | later | P1 Related-trade rules |

### Parts for WORK-A5

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A5-F1-P1 | **Profile search index**: Spare Work's profile and service fields indexed in the kit's search service, kept current from the operation log. | data | small | KIT-A6-F1-P2 (Search service), WORK-A2-F1-P1 (Profile data model) |  |
| WORK-A5-F1-P2 | **Search page with filters**: Search with filters for trade, distance, credentials, rating and availability. | interface | medium | WORK-A5-F1-P1 (Profile search index) |  |
| WORK-A5-F1-P3 | **Search in many languages**: Finds the right trade whatever language the customer searches in. | library | medium | WORK-A5-F1-P1 (Profile search index), WORK-A20-F4-P1 (Translation system) |  |
| WORK-A5-F1-P4 | **Harvest limits**: Spare Work's limits on the kit's bulk copy and traffic limits for search, directory pages and the API, so nobody can harvest sole traders' phone numbers, addresses or reviews in bulk. | library | small | KIT-A16-F1-P4 (Bulk copy limits), KIT-A22-F5-P4 (Traffic and abuse limits), WORK-A5-F1-P1 (Profile search index) |  |
| WORK-A5-F2-P1 | **Directory pages**: Directory pages built from profiles, fast to load and readable by search engines, within the harvest limits. | interface | medium | WORK-A5-F1-P1 (Profile search index), WORK-A5-F1-P4 (Harvest limits) |  |
| WORK-A5-F2-P2 | **Regions and suburbs**: Region, town and suburb names from open address data. | data | small | KIT-A7-F2-P3 (Official address data) |  |
| WORK-A5-F3-P1 | **Comparison view**: Side-by-side view of responding workers and their quotes. | interface | medium | WORK-A7-F1-P2 (Quote data model), WORK-A14-F3-P1 (Review display) |  |
| WORK-A5-F4-P1 | **Favourites**: Saved workers on the kit's favourites, private to the customer. | library | small | KIT-A5-F12-P1 (Favourites) |  |
| WORK-A5-F4-P2 | **Rehire**: Starts a direct job to a past worker with details filled in from the last job. | interface | small | WORK-A3-F1-P2 (Job posting form) |  |
| WORK-A5-F5-P1 | **Related-trade rules**: Links between trades from the category tree, shown as suggestions, never paid placement. | library | small | WORK-A3-F2-P1 (Category tree) |  |

## WORK-A6 Fixed-price services

Workers can sell set services at a set price, such as a building inspection, a gas safety check or a logo, bought in a few taps.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A6-F1 List a service | A worker can list a service with a description, price, delivery time and what is included. | core | P1 Service listing model; P2 Listing editor; P3 Entries in the shared service catalogue |
| WORK-A6-F2 Packages and extras | A worker can offer up to three packages and paid add-ons such as faster delivery or more revisions. | next | P1 Package and extras pricing; P2 Package comparison |
| WORK-A6-F3 Buy a service | A customer can buy a listed service, answer the worker's set questions and pay into a held account. | core | P1 Service checkout; P2 Set questions; P3 Book a time for on-site services |
| WORK-A6-F4 Custom offers | A worker can send one customer a one-off offer with its own price, time and stages. | next | P1 Custom offer |
| WORK-A6-F5 Browse services | A customer can browse and search services by category, price and delivery time. | core | P1 Service search and pages |
| WORK-A6-F6 Revisions and delivery | A customer can use the revisions included, accept the final delivery, or cancel if it comes late. | next | P1 Revision counter and acceptance; P2 Cancel when late |
| WORK-A6-F7 Paid consultations | A customer can book a short paid call or site visit for advice before deciding on a job. | later | P1 Consultation booking |

### Parts for WORK-A6

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A6-F1-P1 | **Service listing model**: A listed service with scope, price, time, area and category, on the shared marketplace engine. | data | medium | KIT-A5-F1-P1 (Listing model), WORK-A3-F2-P1 (Category tree), WORK-A3-F8-P2 (Service rules as data) |  |
| WORK-A6-F1-P2 | **Listing editor**: Writes and previews a service listing with photos and samples. | interface | medium | WORK-A6-F1-P1 (Service listing model), KIT-A5-F1-P2 (Listing editor), WORK-A2-F2-P2 (Job and portfolio media) |  |
| WORK-A6-F1-P3 | **Entries in the shared service catalogue**: Publishes Spare Work's services as entries in one shared service catalogue on the kit's marketplace engine, so Spare Home and Spare Stay book the same pro with one account, one set of checks and one reputation; the catalogue belongs in the kit and is not yet on the kit's map. | integration | medium | kit-market, KIT-A5-F1-P1 (Listing model), svc-matching, WORK-A6-F1-P1 (Service listing model) |  |
| WORK-A6-F2-P1 | **Package and extras pricing**: Package tiers and add-ons with their own prices and times. | library | medium | WORK-A6-F1-P1 (Service listing model), KIT-A5-F3-P1 (Pricing rules engine) |  |
| WORK-A6-F2-P2 | **Package comparison**: Shows packages side by side on the listing. | interface | small | WORK-A6-F1-P1 (Service listing model) |  |
| WORK-A6-F3-P1 | **Service checkout**: Checkout for listed services using held payments. | interface | medium | WORK-A9-F1-P2 (Held funds per stage), WORK-A6-F1-P1 (Service listing model) |  |
| WORK-A6-F3-P2 | **Set questions**: Questions the worker needs answered before starting, asked at checkout. | library | small | WORK-A6-F1-P1 (Service listing model) |  |
| WORK-A6-F3-P3 | **Book a time for on-site services**: Picks a time slot from the worker's calendar for services done on site. | library | medium | WORK-A17-F1-P1 (Calendar model) |  |
| WORK-A6-F4-P1 | **Custom offer**: A one-off offer built from the quote model and sent in a message; an offer the customer did not ask for carries the uninvited direct sale notice. | library | small | WORK-A7-F1-P2 (Quote data model), WORK-A12-F1-P1 (Message threads), WORK-A8-F1-P9 (Uninvited direct sales) |  |
| WORK-A6-F5-P1 | **Service search and pages**: Search and category pages for listed services. | interface | medium | WORK-A5-F1-P1 (Profile search index), WORK-A6-F1-P1 (Service listing model) |  |
| WORK-A6-F6-P1 | **Revision counter and acceptance**: Tracks revisions used and releases payment when the delivery is accepted. | library | small | WORK-A10-F5-P1 (Deliveries), WORK-A9-F2-P1 (Release with sign-off) |  |
| WORK-A6-F6-P2 | **Cancel when late**: A buyer can cancel and get the held money back when a fixed-price service is late, unless the buyer caused the delay. | library | small | WORK-A6-F6-P1 (Revision counter and acceptance), WORK-A9-F5-P1 (Refunds and part refunds) |  |
| WORK-A6-F7-P1 | **Consultation booking**: Books a consultation slot with the fee held until it happens. | library | small | WORK-A17-F1-P1 (Calendar model), WORK-A9-F1-P2 (Held funds per stage) |  |

## WORK-A7 Quotes and proposals

Workers quote for free; customers compare quotes, ask questions and accept one. An accepted quote becomes stages with amounts and sign-off conditions.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A7-F1 Write a quote | A worker can write a quote with line items, labour, materials, sales tax, exclusions, start date and expiry. | core | P1 Quote, stage and sign-off model; P2 Quote data model; P3 Quote builder; P4 Price book; P5 Tax and totals |
| WORK-A7-F2 Estimate, quote or fixed price | A worker can mark a price as an estimate, a quote or a fixed price, and the customer sees what each means. | core | P1 Price types; P2 Estimate to quote after a visit; P3 Paid measure-ups for large jobs |
| WORK-A7-F3 Proposals for remote work | A freelancer can send a proposal with an approach, timeline, milestones and samples. | next | P1 Proposal form; P2 Attach portfolio samples |
| WORK-A7-F4 Compare and accept quotes | A customer can compare quotes line by line, ask questions, and accept one. | core | P1 Quote comparison; P2 Accept or decline; P3 Quote to agreement |
| WORK-A7-F5 Revise a quote | A worker can revise a quote before it is accepted, and the customer sees exactly what changed. | core | P1 Quote versions |
| WORK-A7-F6 Quote templates | A worker can save quote templates for jobs they do often. | next | P1 Templates |
| WORK-A7-F7 Price from drawings | A worker can price straight from quantities in a Spare Construct drawing set attached to the job. | later | P1 Read quantities; P2 Quantities to price book |
| WORK-A7-F8 Charge-up jobs | A trade can work on time and materials, with a rate card, time on site, materials receipts with the markup shown, and an optional cap. | core | P1 Rate card; P2 Time on site; P3 Materials with receipts; P4 Not-to-exceed cap |

### Parts for WORK-A7

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A7-F1-P1 | **Quote, stage and sign-off model**: Design note with worked examples for a small repair, a renovation and a remote job. | research | medium |  | [WK04](STATUS.md) |
| WORK-A7-F1-P2 | **Quote data model**: Spare Work's quote fields, such as line items, labour, materials, exclusions, expiry and proposed stages, as an extension of the kit's offers and quotes and its quote requests. | data | medium | KIT-A5-F3-P3 (Offers and quotes), KIT-A6-F6-P1 (Quote requests), WORK-A7-F1-P1 (Quote, stage and sign-off model) |  |
| WORK-A7-F1-P3 | **Quote builder**: Builds a quote on a phone or desktop, from scratch, a template or the price book. | interface | large | WORK-A7-F1-P2 (Quote data model) |  |
| WORK-A7-F1-P4 | **Price book**: The worker's saved items, rates and markups, used across quotes. | library | medium | WORK-A7-F1-P2 (Quote data model) |  |
| WORK-A7-F1-P5 | **Tax and totals**: Works out sales tax and totals from the worker's tax status and the country's rules, using the kit's full price first, so consumers always see tax-inclusive totals. | library | small | WORK-A13-F3-P1 (Tax rules engine), WORK-A7-F1-P2 (Quote data model), KIT-A5-F3-P2 (Full price first) |  |
| WORK-A7-F2-P1 | **Price types**: Price type rules with a plain explanation shown to the customer. | library | small | WORK-A7-F1-P2 (Quote data model) |  |
| WORK-A7-F2-P2 | **Estimate to quote after a visit**: Turns an estimate into a firm quote after a site visit, keeping the history. | library | small | WORK-A10-F2-P1 (Site visit booking), WORK-A7-F1-P2 (Quote data model) |  |
| WORK-A7-F2-P3 | **Paid measure-ups for large jobs**: For a large job, a worker can ask for an agreed fee for a measure-up or design before quoting, shown upfront and credited if the quote is accepted; quoting itself stays free. | library | small | WORK-A7-F1-P2 (Quote data model), WORK-A9-F1-P2 (Held funds per stage) |  |
| WORK-A7-F3-P1 | **Proposal form**: A proposal view of the quote model for remote work. | interface | medium | WORK-A7-F1-P2 (Quote data model) |  |
| WORK-A7-F3-P2 | **Attach portfolio samples**: Picks relevant portfolio items to send with a proposal. | interface | small | WORK-A2-F2-P1 (Portfolio items) |  |
| WORK-A7-F4-P1 | **Quote comparison**: Line-by-line comparison that lines up similar items and shows what each quote leaves out. | interface | medium | WORK-A7-F1-P2 (Quote data model) |  |
| WORK-A7-F4-P2 | **Accept or decline**: Accepts one quote, declines others with an optional reason, and tells every worker. | library | small | WORK-A7-F1-P2 (Quote data model), KIT-A8-F3-P2 (Notification router) |  |
| WORK-A7-F4-P3 | **Quote to agreement**: Turns the accepted quote into a written agreement with stages. | library | medium | WORK-A7-F1-P2 (Quote data model), WORK-A8-F1-P1 (Agreement document), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A7-F5-P1 | **Quote versions**: Keeps every version and shows changes between them. | interface | small | WORK-A7-F1-P2 (Quote data model), kit-sync |  |
| WORK-A7-F6-P1 | **Templates**: Saves a quote as a template and starts new quotes from it. | library | small | WORK-A7-F1-P2 (Quote data model), WORK-A7-F1-P4 (Price book) |  |
| WORK-A7-F7-P1 | **Read quantities**: Reads quantities from attached drawings through Spare Construct's public command API. | integration | large | KIT-A16-F1-P1 (Command API), WORK-A19-F1-P1 (Work command schemas) |  |
| WORK-A7-F7-P2 | **Quantities to price book**: Matches quantities to price book items to start a quote. | library | medium | WORK-A7-F1-P4 (Price book) |  |
| WORK-A7-F8-P1 | **Rate card**: Hourly rates, call-out charge and materials markup, shown to the customer before they agree. | data | small | WORK-A7-F1-P4 (Price book) |  |
| WORK-A7-F8-P2 | **Time on site**: Records time on site with start and stop, or entered after, with notes, as operations the customer can see; hourly remote work uses the same records. | library | medium | kit-ops, WORK-A10-F2-P1 (Site visit booking) |  |
| WORK-A7-F8-P3 | **Materials with receipts**: Materials lines with a photo of the receipt and the markup shown. | library | small | WORK-A7-F8-P1 (Rate card), WORK-A2-F2-P2 (Job and portfolio media) |  |
| WORK-A7-F8-P4 | **Not-to-exceed cap**: An optional cap; work pauses for the customer's approval before going over it. | library | small | WORK-A7-F8-P2 (Time on site), WORK-A8-F3-P1 (Variation requests) |  |

## WORK-A8 Agreements, stages and changes

Every accepted job becomes a written agreement in plain words, with stages, sign-off conditions and a clean way to agree changes.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A8-F1 Written agreement | Both sides get a written agreement from the accepted quote, with the terms the law requires. | core | P1 Agreement document; P2 Building law pack (example: New Zealand's Building Act); P3 Plain standard terms; P4 Confidentiality and ownership terms; P5 Signing; P6 Residential building duties on every job; P7 Worker's own terms; P8 One binding language; P9 Uninvited direct sales |
| WORK-A8-F2 Stages and sign-off conditions | A customer and worker can split a job into stages, each with an amount, a due date and what counts as done. | core | P1 Stage model; P2 Stage planner; P3 Deposit rules |
| WORK-A8-F3 Agree changes | Either side can propose a change to scope, price or time, and it applies only once both agree. | core | P1 Variation requests; P2 Variation approval; P3 Agreement history |
| WORK-A8-F4 Payment claims | A worker can make a payment claim for a stage, and the customer can answer with a payment schedule, as construction law expects. | core | P1 Construction payment rules; P2 Claim and schedule forms; P3 Deadline reminders; P4 Suspending work after a missed payment |
| WORK-A8-F5 Cancel an agreement | Either side can cancel by the agreed rules, with held money returned or released fairly. | core | P1 Cancellation and settlement |
| WORK-A8-F6 Retentions | A customer or main contractor can hold back a retention that is kept safely and released on time. | later | P1 Retention rules; P2 Retention tracking |
| WORK-A8-F7 Replace a worker mid-job | A customer can replace a worker who abandons the job, becomes insolvent, dies or loses their licence, without losing the job's history. | next | P1 Settle the part done; P2 Record of work for the part done; P3 Re-tender the rest |

### Parts for WORK-A8

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A8-F1-P1 | **Agreement document**: Builds the agreement from the quote, both sides' details and the standard terms, as a PDF and a web page. | library | medium | WORK-A7-F1-P2 (Quote data model), engine-pdf |  |
| WORK-A8-F1-P2 | **Building law pack (example: New Zealand's Building Act)**: Each country's rules for building contracts; for New Zealand residential building work: the disclosure statement and checklist before signing when the price is $30,000 or more including GST, or whenever the client asks, and a written contract at $30,000 or more. | legal | medium | WORK-A8-F1-P1 (Agreement document) |  |
| WORK-A8-F1-P3 | **Plain standard terms**: Standard terms in plain words, written with a lawyer, with no unfair terms. | legal | medium | WORK-A21-F4-P1 (Contractor law and the platform), KIT-A18-F8-P1 (Consumer law for platforms) |  |
| WORK-A8-F1-P4 | **Confidentiality and ownership terms**: Optional confidentiality terms and who owns the work once paid, mainly for remote work. | legal | small | WORK-A8-F1-P1 (Agreement document) |  |
| WORK-A8-F1-P5 | **Signing**: The paying party, any named approver and the worker sign in the app through the kit's signatures, with an audit trail and a signed PDF copy for each party. | library | small | kit-ops, KIT-A25-F1-P1 (Electronic signature rules), KIT-A25-F1-P2 (Signatures with audit trail), WORK-A3-F1-P5 (Roles on a job) |  |
| WORK-A8-F1-P6 | **Residential building duties on every job**: For all residential building work in the New Zealand pack, whatever the price: the implied warranties, the 12-month defect repair period and the information due on completion, shown in the agreement. | legal | medium | WORK-A8-F1-P1 (Agreement document), WORK-A3-F2-P1 (Category tree) |  |
| WORK-A8-F1-P7 | **Worker's own terms**: A worker can attach their own terms of trade next to the standard terms, with a plain rule for which wins; terms that cut back consumer guarantees or are unfair are blocked. | legal | medium | WORK-A8-F1-P3 (Plain standard terms) |  |
| WORK-A8-F1-P8 | **One binding language**: Each agreement and change names one binding language; translations are marked as translations, so both sides see which text counts. | library | small | WORK-A8-F1-P1 (Agreement document), WORK-A20-F4-P1 (Translation system) |  |
| WORK-A8-F1-P9 | **Uninvited direct sales**: When the worker starts the deal uninvited, such as a custom offer, a yearly service reminder with a price, a quote nobody asked for or a door-knock after a storm, the agreement carries the cancellation notice and cooling-off period consumer law requires (example: the Fair Trading Act notice and 5 working days in New Zealand); which flows count is confirmed with a lawyer. | legal | medium | WORK-A8-F1-P1 (Agreement document), KIT-A18-F8-P1 (Consumer law for platforms), KIT-A19-F9-P2 (Working days and notice periods) |  |
| WORK-A8-F2-P1 | **Stage model**: Stages with amounts, dates, sign-off conditions and required evidence. | data | medium | WORK-A7-F1-P1 (Quote, stage and sign-off model), kit-market |  |
| WORK-A8-F2-P2 | **Stage planner**: Plans and edits stages before signing, with totals that must match the quote. | interface | medium | WORK-A8-F2-P1 (Stage model) |  |
| WORK-A8-F2-P3 | **Deposit rules**: Shows the deposit size and when it is due, including a materials deposit, within a voted cap that never goes above the customer protection floor, once the kit's competition law check allows members to vote on it. | library | small | WORK-A8-F2-P1 (Stage model), KIT-A12-F4-P1 (Results to settings), KIT-A12-F4-P3 (Competition law check), WORK-A22-F3-P2 (Customer protection floors) |  |
| WORK-A8-F3-P1 | **Variation requests**: A change request on the kit's order changes, linked to the agreement, with price and time effects; extras found after the work use the kit's extra charges after the job. | library | medium | kit-ops, WORK-A8-F1-P1 (Agreement document), KIT-A5-F15-P1 (Order changes), KIT-A5-F15-P2 (Extra charges after the job) |  |
| WORK-A8-F3-P2 | **Variation approval**: Shows the change side by side with the current agreement and asks for approval. | interface | small | WORK-A8-F3-P1 (Variation requests) |  |
| WORK-A8-F3-P3 | **Agreement history**: Keeps every agreed version with who agreed and when. | library | small | kit-sync, WORK-A8-F3-P1 (Variation requests) |  |
| WORK-A8-F4-P1 | **Construction payment rules**: Whether the app's stage invoices count as payment claims under construction contracts law (example: New Zealand's Construction Contracts Act), including for residential occupiers, and the timings and content that follow; settled before stage payments ship. | research | medium |  |  |
| WORK-A8-F4-P2 | **Claim and schedule forms**: Payment claim and payment schedule forms with the legal wording and deadlines. | library | medium | WORK-A8-F2-P1 (Stage model), engine-pdf |  |
| WORK-A8-F4-P3 | **Deadline reminders**: Reminds both sides before a claim or schedule deadline passes, counted in working days on the kit's working-day clock. | service | small | KIT-A8-F3-P2 (Notification router), WORK-A8-F4-P2 (Claim and schedule forms), KIT-A19-F9-P2 (Working days and notice periods) |  |
| WORK-A8-F4-P4 | **Suspending work after a missed payment**: Shows a worker their right under construction contracts law (example: New Zealand's Construction Contracts Act) to suspend work after a missed scheduled payment, with the notice it needs, and tells the customer before work stops. | legal | small | WORK-A8-F4-P1 (Construction payment rules), WORK-A8-F4-P2 (Claim and schedule forms), KIT-A19-F9-P2 (Working days and notice periods) |  |
| WORK-A8-F5-P1 | **Cancellation and settlement**: Spare Work's settlement rule on the kit's cancellation policies: works out what is owed for work done and returns or releases held money. | library | medium | KIT-A5-F6-P1 (Cancellation and no-show policies), WORK-A9-F1-P2 (Held funds per stage), WORK-A8-F1-P1 (Agreement document) |  |
| WORK-A8-F6-P1 | **Retention rules**: When retentions are allowed and how they must be held in each country, starting with New Zealand commercial contracts and the Construction Contracts Act's trust rules for retention money. | research | small |  |  |
| WORK-A8-F6-P2 | **Retention tracking**: Tracks each retention and releases it on the agreed date; retention money is never left with the payment provider, whose holds end long before a 12-month retention does, and is held as the trust rules require. | library | medium | WORK-A8-F6-P1 (Retention rules), KIT-A3-F2-P1 (Hold and release rules), KIT-A3-F2-P4 (Hold time limits), WORK-A9-F1-P5 (Fund each stage just in time) |  |
| WORK-A8-F7-P1 | **Settle the part done**: Settles held money for work done to date, with a member panel deciding if the sides cannot agree. | library | medium | WORK-A8-F5-P1 (Cancellation and settlement), WORK-A15-F3-P2 (Decision and reasons) |  |
| WORK-A8-F7-P2 | **Record of work for the part done**: Asks the named licence holder, or their firm or estate, for a record of work for the part done. | library | small | WORK-A10-F5-P3 (Record of work) |  |
| WORK-A8-F7-P3 | **Re-tender the rest**: Re-posts the remaining stages with the job history, drawings and inspections attached. | library | medium | WORK-A3-F1-P1 (Job data model), WORK-A10-F1-P2 (Job timeline) |  |

## WORK-A9 Held payments and payouts

Money for each stage is held by a regulated payment provider, released when the stage is signed off, and the worker keeps everything except the at-cost fee.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A9-F1 Fund a stage | A customer can pay a stage into a held account by card or bank payment before the work starts. | core | P1 Holding stage payments legally; P2 Held funds per stage; P3 Payment screen; P4 Held money on the job; P5 Fund each stage just in time; P6 Bank payments for stages |
| WORK-A9-F2 Release a stage | A customer can release a stage when it is done, and the money reaches the worker quickly. | core | P1 Release with sign-off; P2 Release if no reply; P4 Release for materials |
| WORK-A9-F3 Pay without holding | A customer and worker can skip holding, pay an invoice in the app or on the worker's phone, or record a payment made outside the app. | core | P1 Pay an invoice; P2 Holding threshold; P3 Paid outside the app; P4 Take a card on site |
| WORK-A9-F4 See the fee | Both sides can see the at-cost fee on every job and what it paid for. | core | P1 Fee calculation; P2 Fee shown everywhere; P4 Fee basis when money stays outside; P5 Gap shown on the ledger; P6 Interest on held money |
| WORK-A9-F5 Refunds | A customer can get held money back when a stage is cancelled or a dispute goes their way, and a worker knows where they stand if a card is charged back. | core | P1 Refunds and part refunds; P2 Chargeback rules; P3 Chargeback evidence from the workroom |
| WORK-A9-F6 Tips and bonuses | A customer can add a tip or bonus, and the worker gets all of it except the payment provider's own charge on it, shown before paying. | core | P1 Tips |
| WORK-A9-F7 Payments across countries | A customer and worker in different countries can work together with clear currency conversion. | later | P1 Many currencies; P2 Exchange rate shown first |
| WORK-A9-F8 Payout history | A worker can see every payout, fee and refund, and download statements. | core | P1 Payouts and statements |
| WORK-A9-F9 Jobs paid by an insurer | An insurer, or the Natural Hazards Commission, can pay the worker directly while the owner pays any excess and approves the work. | next | P1 Insurer as payer; P2 Insurance repair jobs |
| WORK-A9-F10 Pay for a large job in instalments | A customer can pay a large job over time through a regulated lender, with the full cost of credit shown first. | later | P1 Credit law check; P2 Lender link |

### Parts for WORK-A9

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A9-F1-P1 | **Holding stage payments legally**: How held stage payments must be run, building on shared kit piece K02. | research | medium |  | [WK01](STATUS.md) |
| WORK-A9-F1-P2 | **Held funds per stage**: Maps each Spare Work stage onto the kit's stage payments, linked to the stage as an operation. | integration | medium | KIT-A3-F2-P3 (Stage payments), WORK-A9-F1-P1 (Holding stage payments legally), WORK-A8-F2-P1 (Stage model), KIT-A3-F2-P1 (Hold and release rules), KIT-A3-F2-P4 (Hold time limits) |  |
| WORK-A9-F1-P3 | **Payment screen**: Spare Work's view of the kit's checkout: the stage, the amount, the fee and where the money is held. | interface | small | KIT-A3-F1-P3 (Checkout), WORK-A9-F4-P2 (Fee shown everywhere) |  |
| WORK-A9-F1-P4 | **Held money on the job**: Shows each stage's money as held, released or refunded, with dates. | interface | small | WORK-A9-F1-P2 (Held funds per stage) |  |
| WORK-A9-F1-P5 | **Fund each stage just in time**: Asks the customer to pay a stage in shortly before it starts, releases or refunds it within the provider's hold limit (90 days for Stripe outside the US), and captures or renews card holds before they expire, so a long job never outlasts a hold. | library | medium | KIT-A3-F2-P1 (Hold and release rules), KIT-A3-F2-P4 (Hold time limits), WORK-A9-F1-P2 (Held funds per stage), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A9-F1-P6 | **Bank payments for stages**: Lets a customer fund a stage by bank payment through the kit, the default for large stages where the provider supports it, since card fees of 2.7% + 30c would add about $1,080 to a $40,000 stage. | integration | medium | KIT-A3-F11-P1 (Bank payments), WORK-A9-F1-P2 (Held funds per stage), WORK-A9-F1-P3 (Payment screen) |  |
| WORK-A9-F2-P1 | **Release with sign-off**: Releases the stage's money when the customer signs it off. | library | small | WORK-A9-F1-P2 (Held funds per stage), kit-ops |  |
| WORK-A9-F2-P2 | **Release if no reply**: Spare Work's settings for the kit's auto-confirm: how many working days after a stage is marked done, with reminders, set by member vote but never below the customer protection floor. | library | small | KIT-A5-F7-P1 (Completion and auto-confirm), WORK-A9-F2-P1 (Release with sign-off), KIT-A19-F9-P2 (Working days and notice periods), WORK-A22-F3-P2 (Customer protection floors) |  |
| WORK-A9-F2-P4 | **Release for materials**: Releases part of a held stage early when the worker shows proof of buying materials for it, up to the amount agreed. | library | small | WORK-A9-F2-P1 (Release with sign-off), WORK-A7-F8-P3 (Materials with receipts) |  |
| WORK-A9-F3-P1 | **Pay an invoice**: Pays a worker's invoice straight through the payment provider. | library | small | KIT-A3-F1-P3 (Checkout), WORK-A13-F1-P2 (Invoice generator) |  |
| WORK-A9-F3-P2 | **Holding threshold**: Holding is on by default above a size set by member vote, never above the customer protection floor's maximum. | data | small | KIT-A12-F4-P1 (Results to settings), WORK-A22-F3-P2 (Customer protection floors) |  |
| WORK-A9-F3-P3 | **Paid outside the app**: Records that the customer paid by bank transfer or cash, confirmed by the worker, so reviews, the completion pack, defect tracking and records still work. | library | small | WORK-A3-F6-P1 (Job states and history) |  |
| WORK-A9-F3-P4 | **Take a card on site**: Takes a card on the worker's phone at the end of a job through the payment provider's tap to pay, with the kit's card fee check applied, since a surcharge ban may cover in-person payments; needs card-present payments added to the kit. | integration | medium | KIT-A3-F1-P2 (Payment provider link), WORK-A13-F1-P2 (Invoice generator), KIT-A3-F4-P3 (Card fee pass-through check) |  |
| WORK-A9-F4-P1 | **Fee calculation**: Spare Work's fee on the kit's fee calculator and fee setting, counting the jobs and costs that the fee basis sets. | library | small | KIT-A3-F4-P1 (Fee calculator), KIT-A13-F3-P1 (Fee setting), WORK-A9-F4-P4 (Fee basis when money stays outside) |  |
| WORK-A9-F4-P2 | **Fee shown everywhere**: Shows the kit's fee breakdown on Spare Work quotes, stage payments, invoices and payouts. | interface | small | KIT-A3-F4-P2 (Fee breakdown), WORK-A9-F4-P1 (Fee calculation) |  |
| WORK-A9-F4-P4 | **Fee basis when money stays outside**: Decides how matching, licence re-checks, messages, panels and support are paid for on jobs paid outside the app: a small fee on accepted agreements, or a fee only on paid jobs with the gap shown; settled before the first fee is set. | research | medium | KIT-A13-F1-P1 (Ledger design) |  |
| WORK-A9-F4-P5 | **Gap shown on the ledger**: Shows on Spare Work's ledger page what jobs paid outside the app cost to support and who covered it. | data | small | WORK-A9-F4-P4 (Fee basis when money stays outside), WORK-A18-F2-P1 (Spare Work ledger page) |  |
| WORK-A9-F4-P6 | **Interest on held money**: Whether held money earns interest or another benefit for the provider or the co-op, who gets it, and how it is shown on the ledger at cost. | research | small | WORK-A9-F1-P1 (Holding stage payments legally), WORK-A18-F2-P1 (Spare Work ledger page) |  |
| WORK-A9-F5-P1 | **Refunds and part refunds**: Part refunds by stage on the kit's refunds, recorded against the stage. | library | small | KIT-A3-F5-P1 (Refunds), WORK-A9-F1-P2 (Held funds per stage), WORK-A15-F2-P1 (Dispute flow) |  |
| WORK-A9-F5-P2 | **Chargeback rules**: Spare Work's choices under the kit's loss rules when a card is charged back after a stage is released: who bears the loss, recovery from later payouts within limits, and protection for workers whose customer signed off first. | legal | medium | KIT-A3-F16-P1 (Loss rules), KIT-A3-F16-P2 (Negative balances), KIT-A3-F5-P2 (Card disputes), WORK-A9-F1-P1 (Holding stage payments legally) |  |
| WORK-A9-F5-P3 | **Chargeback evidence from the workroom**: Sends the agreement, sign-off, photos and messages from the workroom with the kit's answer to a card dispute. | library | small | KIT-A3-F5-P2 (Card disputes), WORK-A10-F1-P2 (Job timeline), WORK-A9-F5-P2 (Chargeback rules) |  |
| WORK-A9-F6-P1 | **Tips**: Turns on the kit's tips for Spare Work jobs: no co-op fee is taken, and only the provider's own charge on the tip comes off, shown before the customer pays. | library | small | KIT-A3-F6-P1 (Tips) |  |
| WORK-A9-F7-P1 | **Many currencies**: Spare Work's held stages and payouts in the kit's currencies for each country. | integration | small | KIT-A3-F14-P1 (Currencies per country), WORK-A9-F1-P2 (Held funds per stage), WORK-A20-F5-P1 (Country rule packs) |  |
| WORK-A9-F7-P2 | **Exchange rate shown first**: Shows the exchange rate and any conversion cost before paying. | interface | small | KIT-A3-F14-P1 (Currencies per country) |  |
| WORK-A9-F8-P1 | **Payouts and statements**: Spare Work's view of the kit's earnings page and yearly statement, linking each payout to its job and stage, with a tax invoice for the fee. | interface | small | KIT-A3-F3-P3 (Earnings page), KIT-A4-F4-P1 (Yearly statements), WORK-A13-F1-P4 (Tax invoice for the fee) |  |
| WORK-A9-F9-P1 | **Insurer as payer**: Lets a job name an insurer as payer for agreed stages, with the owner paying the excess and approving the work. | library | medium | WORK-A3-F1-P5 (Roles on a job), WORK-A9-F1-P2 (Held funds per stage) |  |
| WORK-A9-F9-P2 | **Insurance repair jobs**: How insurers and the Natural Hazards Commission authorise and pay for repairs, and what they need from the worker. | research | small |  |  |
| WORK-A9-F10-P1 | **Credit law check**: What consumer credit law and the buy now, pay later rules ask of a platform that offers a lender's finance (example: New Zealand's Credit Contracts and Consumer Finance Act), settled before any is offered. | research | small |  |  |
| WORK-A9-F10-P2 | **Lender link**: Offers finance from a regulated lender at checkout for large stages; the lender pays into the held stage and the co-op takes no commission. | integration | medium | WORK-A9-F10-P1 (Credit law check), WORK-A9-F1-P2 (Held funds per stage) |  |

## WORK-A10 Doing the job

Each job has one place for its schedule, site visits, messages, files, progress photos, deliveries and sign-offs.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A10-F1 Job workroom | Both sides can see the agreement, stages, money, messages, files and next steps for a job in one place. | core | P1 Workroom page; P2 Job timeline; P3 Next step prompts |
| WORK-A10-F2 Book a site visit | A worker can offer times for a site visit and the customer picks one. | core | P1 Site visit booking; P2 Address and access notes; P3 Visit reminders and running late; P4 No-shows and call-out fees; P5 On my way |
| WORK-A10-F3 Schedule the work | A worker can set start and stage dates, and the customer sees any change. | core | P1 Job schedule |
| WORK-A10-F4 Share progress | A worker can post progress photos and notes for each stage. | core | P1 Progress updates; P2 Evidence photo checks |
| WORK-A10-F5 Deliver work | A remote worker can deliver files, and a trade can mark a stage as done with evidence. | core | P1 Deliveries; P2 Completion evidence; P3 Record of work; P4 Preview deliveries; P5 How it was made |
| WORK-A10-F6 Sign off a stage | A customer can sign off a stage, ask for fixes, or raise a concern. | core | P1 Sign-off and fix requests; P2 Completion pack; P3 Sign-off by more than one party |
| WORK-A10-F7 Meet online | A customer and freelancer can hold a video call from the workroom. | later | P1 Video calls |
| WORK-A10-F8 Council inspections and sign-off | A customer and worker can see council inspections in the schedule and track the code compliance certificate at the end. | next | P1 Inspection stages; P2 Code compliance certificate |

### Parts for WORK-A10

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A10-F1-P1 | **Workroom page**: One page per job with the agreement, stages, held money, messages and files. | interface | large | WORK-A8-F1-P1 (Agreement document), WORK-A12-F1-P1 (Message threads), WORK-A9-F1-P2 (Held funds per stage) |  |
| WORK-A10-F1-P2 | **Job timeline**: Spare Work's events on the kit's order timeline. | library | small | KIT-A5-F4-P2 (Order timeline), kit-ops |  |
| WORK-A10-F1-P3 | **Next step prompts**: Tells each side what they need to do next and by when. | library | small | WORK-A8-F2-P1 (Stage model) |  |
| WORK-A10-F2-P1 | **Site visit booking**: Offers time slots from the worker's calendar and books one. | library | medium | WORK-A17-F1-P1 (Calendar model) |  |
| WORK-A10-F2-P2 | **Address and access notes**: Shares the exact address, parking and access notes only once a visit is booked. | library | small | WORK-A3-F1-P4 (Address privacy) |  |
| WORK-A10-F2-P3 | **Visit reminders and running late**: Reminds both sides and lets the worker say they are running late in one tap. | service | small | KIT-A8-F3-P2 (Notification router), WORK-A10-F2-P1 (Site visit booking) |  |
| WORK-A10-F2-P4 | **No-shows and call-out fees**: Uses the kit's no-show policies for site visits on either side, with any call-out fee agreed before the visit. | library | small | KIT-A5-F6-P1 (Cancellation and no-show policies), WORK-A10-F2-P1 (Site visit booking) |  |
| WORK-A10-F2-P5 | **On my way**: The worker sends an 'on my way' message with an arrival time, and their live location is shared only until they arrive. | library | small | KIT-A7-F4-P1 (Live location stream), KIT-A7-F3-P3 (Arrival time estimates), WORK-A10-F2-P1 (Site visit booking) |  |
| WORK-A10-F3-P1 | **Job schedule**: Stage dates on the shared calendar, with changes sent to the customer and shared through the kit's calendar export. | library | medium | WORK-A17-F1-P1 (Calendar model), WORK-A8-F2-P1 (Stage model), KIT-A5-F5-P2 (Calendar import and export) |  |
| WORK-A10-F4-P1 | **Progress updates**: Photos and notes linked to a stage. | library | small | WORK-A2-F2-P2 (Job and portfolio media), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A10-F4-P2 | **Evidence photo checks**: Prefers progress photos taken in the app, with their time and place kept as evidence, and checks stage photos for reuse from other jobs or the web and for signs of editing, sending any match to a person to review. | library | medium | WORK-A10-F4-P1 (Progress updates), WORK-A2-F2-P4 (Copied portfolio checks), KIT-A5-F7-P2 (Proof of handover), KIT-A11-F2-P1 (Evidence store) |  |
| WORK-A10-F5-P1 | **Deliveries**: Delivers files and notes against a stage. | library | medium | kit-store, WORK-A8-F2-P1 (Stage model) |  |
| WORK-A10-F5-P2 | **Completion evidence**: Photos, certificates of compliance, electrical safety and test certificates, and producer statements attached to a stage as the kit's proof of handover. | data | medium | WORK-A10-F5-P1 (Deliveries), KIT-A5-F7-P2 (Proof of handover) |  |
| WORK-A10-F5-P3 | **Record of work**: In the New Zealand pack, the named licensed building practitioner gives the owner and the council a record of work when restricted building work is done, or when they stop work, as Building Act s88 requires; whether this now covers small stand-alone dwellings built without consent is unverified. | legal | medium | WORK-A2-F3-P9 (Named licence holder on each stage), WORK-A3-F1-P5 (Roles on a job), engine-pdf |  |
| WORK-A10-F5-P4 | **Preview deliveries**: A remote worker can deliver a preview or watermarked copy; the full files and ownership pass when the stage is released. | library | medium | WORK-A10-F5-P1 (Deliveries), WORK-A9-F2-P1 (Release with sign-off) |  |
| WORK-A10-F5-P5 | **How it was made**: A worker says whether a delivery was made with AI tools, shown on the delivery and in the portfolio. | data | small | WORK-A10-F5-P1 (Deliveries) |  |
| WORK-A10-F6-P1 | **Sign-off and fix requests**: Signs off a stage, asks for listed fixes with a date, or raises a concern. | library | medium | WORK-A9-F2-P1 (Release with sign-off), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A10-F6-P2 | **Completion pack**: Collects certificates, the record of work, the code compliance certificate once issued, warranties, manuals and the final invoice for the owner to keep. | library | medium | engine-pdf, WORK-A10-F5-P1 (Deliveries), WORK-A10-F5-P3 (Record of work), WORK-A13-F1-P2 (Invoice generator) |  |
| WORK-A10-F6-P3 | **Sign-off by more than one party**: When a job names several parties, such as a tenant and a landlord, each confirms completion, and the job says whose sign-off releases the money. | library | small | WORK-A10-F6-P1 (Sign-off and fix requests), WORK-A3-F1-P5 (Roles on a job) |  |
| WORK-A10-F7-P1 | **Video calls**: Calls through the kit's call service, never recorded unless both agree. | integration | medium | KIT-A8-F11-P2 (Call service) |  |
| WORK-A10-F8-P1 | **Inspection stages**: Adds the council's inspections from the consent to the stage schedule, so stage dates follow them. | library | small | WORK-A3-F3-P3 (Building consent details), WORK-A10-F3-P1 (Job schedule) |  |
| WORK-A10-F8-P2 | **Code compliance certificate**: Tracks the owner's application for the code compliance certificate and adds it to the completion pack once issued. | library | small | WORK-A10-F8-P1 (Inspection stages), WORK-A10-F6-P2 (Completion pack), WORK-A10-F5-P3 (Record of work) |  |

## WORK-A11 Hourly and remote work

Remote and hourly work such as design, writing and software, with time records the client can check, weekly billing and work across borders.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A11-F1 Hourly agreements | A client can hire at an hourly rate with a weekly hour limit. | next | P1 Hourly terms |
| WORK-A11-F2 Log time | A freelancer can log time by hand or with a timer, with notes for each entry. | next | P1 Time entries; P2 Timer and manual entry; P3 Optional desktop tracker |
| WORK-A11-F3 Review and approve time | A client can review weekly time and approve or query it before payment. | next | P1 Weekly timesheet; P2 Weekly billing; P3 Contested hours; P4 Weekly cap paid in advance |
| WORK-A11-F4 Milestone projects | A client and freelancer can run a remote project with milestones on the same stage flow as trades. | next | P1 Milestones are stages |
| WORK-A11-F5 Ongoing work | A client can keep working with a freelancer under one ongoing agreement with weekly or monthly payments. | later | P1 Retainers |
| WORK-A11-F6 Work across borders | A client in one country can hire a freelancer in another, with the right tax forms and rules for both. | later | P1 Cross-border tax details; P2 Sanctions checks; P3 Sales tax on remote services from overseas (example: New Zealand GST); P4 Each side's local time |

### Parts for WORK-A11

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A11-F1-P1 | **Hourly terms**: Hourly agreements with a rate, weekly cap and billing day. | library | medium | WORK-A8-F1-P1 (Agreement document) |  |
| WORK-A11-F2-P1 | **Time entries**: Time entries for remote work on the same time records as charge-up jobs, with notes and the stage or task they belong to. | data | small | WORK-A7-F8-P2 (Time on site), WORK-A11-F1-P1 (Hourly terms) |  |
| WORK-A11-F2-P2 | **Timer and manual entry**: Starts and stops a timer or adds time by hand, on phone or desktop. | interface | medium | WORK-A11-F2-P1 (Time entries) |  |
| WORK-A11-F2-P3 | **Optional desktop tracker**: A desktop tracker that shares only what the worker chooses, never secret screenshots. | interface | large | WORK-A11-F2-P1 (Time entries) |  |
| WORK-A11-F3-P1 | **Weekly timesheet**: Shows the week's entries for the client to approve or query. | interface | medium | WORK-A11-F2-P1 (Time entries) |  |
| WORK-A11-F3-P2 | **Weekly billing**: Charges approved time each week through held payments. | service | medium | WORK-A9-F1-P2 (Held funds per stage), WORK-A11-F2-P1 (Time entries) |  |
| WORK-A11-F3-P3 | **Contested hours**: A client can query entries within a set time; what counts as evidence, such as delivered work and notes, is published, and a contested week goes to the dispute flow. | library | medium | WORK-A11-F3-P1 (Weekly timesheet), WORK-A15-F2-P1 (Dispute flow) |  |
| WORK-A11-F3-P4 | **Weekly cap paid in advance**: The client funds each week's cap into held money shortly before the week starts, within the provider's hold limits, so approved time is paid even if a card fails later. | library | small | WORK-A11-F3-P2 (Weekly billing), WORK-A9-F1-P2 (Held funds per stage), KIT-A3-F2-P1 (Hold and release rules), KIT-A3-F2-P4 (Hold time limits), WORK-A9-F1-P5 (Fund each stage just in time) |  |
| WORK-A11-F4-P1 | **Milestones are stages**: Remote milestones reuse the stage model, deliveries and sign-off. | library | small | WORK-A8-F2-P1 (Stage model), WORK-A10-F5-P1 (Deliveries) |  |
| WORK-A11-F5-P1 | **Retainers**: Ongoing agreements on the kit's regular payments, with notice to stop. | library | small | KIT-A3-F12-P1 (Regular payments), WORK-A11-F1-P1 (Hourly terms) |  |
| WORK-A11-F6-P1 | **Cross-border tax details**: Collects the tax details each country needs from both sides. | legal | large | WORK-A20-F5-P1 (Country rule packs), WORK-A13-F3-P1 (Tax rules engine), KIT-A4-F7-P1 (Tax rules per country) |  |
| WORK-A11-F6-P2 | **Sanctions checks**: Uses the kit's sanctions screening for every cross-border job and payout. | integration | small | KIT-A4-F8-P1 (Sanctions screening) |  |
| WORK-A11-F6-P3 | **Sales tax on remote services from overseas (example: New Zealand GST)**: When an overseas worker sells remote services to a consumer, the marketplace is often treated as the supplier and must register, charge and return sales tax, as in New Zealand (GST), the EU, the UK and Australia; live before any overseas worker sells to consumers in another country. Confirm the current law in each country. | legal | medium | KIT-A4-F7-P1 (Tax rules per country), KIT-A4-F1-P2 (Sales tax engine), WORK-A13-F3-P1 (Tax rules engine) |  |
| WORK-A11-F6-P4 | **Each side's local time**: Shows deadlines, weekly billing days and calls in each side's local time. | library | small | KIT-A7-F9-P1 (Time zones and places), WORK-A11-F1-P1 (Hourly terms), WORK-A17-F1-P1 (Calendar model) |  |

## WORK-A12 Messages and notifications

Messages between customers and workers tied to each job, plus alerts each member controls.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A12-F1 Message about a job | A customer and worker can message about a job, with photos and files. | core | P1 Message threads; P2 Inbox; P3 Attachments |
| WORK-A12-F2 Share contact details by choice | A member can share their phone number or email when they choose, and block anyone. | core | P1 Contact sharing; P2 Off-platform payment warnings; P3 Block a member |
| WORK-A12-F3 Choose notifications | A member can choose which alerts reach them by push, email or text, and when. | core | P1 Work event list |
| WORK-A12-F4 Saved replies | A worker can save replies they send often. | next | P1 Saved replies |
| WORK-A12-F5 Translate messages | A customer and worker who speak different languages can see messages translated. | later | P1 Message translation |
| WORK-A12-F6 Call without sharing a number | A customer can call a worker through the app without showing their own number, if they choose. | core | P1 Masked calls; P2 Text or relay service instead of a call |

### Parts for WORK-A12

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A12-F1-P1 | **Message threads**: Spare Work's threads per job and per quote on the kit's chat service. | library | small | KIT-A8-F1-P2 (Chat service) |  |
| WORK-A12-F1-P2 | **Inbox**: Groups the kit's chat screens by job and quote, with unread counts and search. | interface | small | KIT-A8-F1-P3 (Chat screens), WORK-A12-F1-P1 (Message threads) |  |
| WORK-A12-F1-P3 | **Attachments**: Photos, files and voice notes in messages. | library | small | WORK-A2-F2-P2 (Job and portfolio media) |  |
| WORK-A12-F2-P1 | **Contact sharing**: Shares contact details only when the member chooses; nothing is hidden to trap people. | library | small | WORK-A12-F1-P1 (Message threads) |  |
| WORK-A12-F2-P2 | **Off-platform payment warnings**: Spare Work's wording for the kit's scam warnings when someone asks to pay outside the app: explains the protection lost and never blocks. | library | small | KIT-A8-F9-P1 (Scam warnings), WORK-A12-F1-P1 (Message threads) |  |
| WORK-A12-F2-P3 | **Block a member**: Uses the kit's blocking, which also stops the blocked member quoting or inviting. | library | small | KIT-A10-F4-P1 (Blocking) |  |
| WORK-A12-F3-P1 | **Work event list**: Every work event that can send an alert, such as a new quote or a stage due. | data | small | KIT-A8-F3-P2 (Notification router), KIT-A8-F5-P1 (Notification settings), kit-ops |  |
| WORK-A12-F4-P1 | **Saved replies**: Saves and inserts reply templates. | library | small | WORK-A12-F1-P1 (Message threads) |  |
| WORK-A12-F5-P1 | **Message translation**: Turns on the kit's message translation in job threads, marked as machine translated; agreements follow the binding language rule. | integration | small | KIT-A8-F7-P1 (Message translation), WORK-A12-F1-P1 (Message threads), WORK-A8-F1-P8 (One binding language) |  |
| WORK-A12-F6-P1 | **Masked calls**: Turns on the kit's masked calls for Spare Work jobs, ending when the job ends. | integration | small | KIT-A8-F2-P1 (Masked calls) |  |
| WORK-A12-F6-P2 | **Text or relay service instead of a call**: A member who is Deaf, hard of hearing or speech-impaired can choose job chat or a relay service call (example: NZ Relay) instead of a masked voice call, and the other side sees how best to reach them; whether masked numbers work through relay services is unverified. | integration | small | KIT-A8-F2-P1 (Masked calls), KIT-A21-F4-P1 (Other support channels), WORK-A12-F1-P1 (Message threads) |  |

## WORK-A13 Invoices, tax and records

Tax-correct invoices and receipts made from each job, records both sides can keep, and the platform's own reports to the tax office (example: New Zealand's Inland Revenue).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A13-F1 Send an invoice | A worker can send a tax-correct invoice made from the agreement and stage, in one tap. | core | P1 Tax invoices and records for trade and freelance work; P2 Invoice generator; P3 Invoice numbering; P4 Tax invoice for the fee |
| WORK-A13-F2 Keep receipts and records | A customer can download receipts and a full record of each job for insurance, tax or a house sale. | core | P1 Receipts and job record; P2 Records kept for the legal period; P3 Records that pass with the property |
| WORK-A13-F3 Set tax status | A worker can set their sales tax status, and quotes and invoices follow it. | core | P1 Tax rules engine; P2 Sales tax number check |
| WORK-A13-F4 Platform income reporting | The co-op reports workers' income to the tax office (example: New Zealand's Inland Revenue) as the digital platform rules for personal services require, and sends each worker a copy. | core | P1 Personal services in the kit's report |
| WORK-A13-F5 Withholding tax | A worker can choose a tax rate to be held back from payouts, where the law allows or requires it. | next | P1 Schedular payments and platforms; P2 Withholding at payout |
| WORK-A13-F6 Send to accounting software | A worker can send invoices, payments and fees to their accounting software. | next | P2 Accounting connectors; P3 E-invoicing |
| WORK-A13-F7 Chase an unpaid invoice | A worker can set due dates, send reminders and statements, and see the next legal steps when an invoice is overdue. | core | P1 Due dates and reminders; P2 Statements of account; P3 Next steps for unpaid work |

### Parts for WORK-A13

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A13-F1-P1 | **Tax invoices and records for trade and freelance work**: What invoices and records for trade and freelance jobs must show in each country, with sources, starting with New Zealand's, as Spare Work's input to the kit's tax invoices. | research | small |  | [WK05](STATUS.md) |
| WORK-A13-F1-P2 | **Invoice generator**: Maps a stage, a charge-up job or a service onto the kit's tax invoices, as a PDF and open data. | library | small | KIT-A4-F2-P1 (Tax invoices), WORK-A13-F1-P1 (Tax invoices and records for trade and freelance work), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A13-F1-P3 | **Invoice numbering**: Numbers invoices per worker in order, with credit notes for changes; this belongs in the kit's tax invoices, since every app that issues tax invoices needs it. | library | small | WORK-A13-F1-P2 (Invoice generator), KIT-A4-F2-P1 (Tax invoices) |  |
| WORK-A13-F1-P4 | **Tax invoice for the fee**: Shows the kit's tax invoice for the co-op's fee on each Spare Work payout statement, so workers registered for sales tax can claim it back. | library | small | KIT-A4-F9-P3 (Sales tax on the fee), WORK-A9-F4-P1 (Fee calculation) |  |
| WORK-A13-F2-P1 | **Receipts and job record**: Issues receipts through the kit's receipts, and exports a full job record as PDF and open data. | library | small | KIT-A3-F7-P1 (Receipts), engine-pdf, WORK-A13-F1-P2 (Invoice generator) |  |
| WORK-A13-F2-P2 | **Records kept for the legal period**: Spare Work's records on the kit's record keeping and retention schedules, including completion packs owners need after a worker leaves. | data | small | KIT-A4-F6-P1 (Financial record keeping), KIT-A18-F6-P1 (Retention schedules) |  |
| WORK-A13-F2-P3 | **Records that pass with the property**: Links each job to its address on a shared property record, so completion packs, records of work, certificates and warranties pass to the next owner when the property is sold, as building law's implied warranties do (example: New Zealand's Building Act); the shared record that Spare Home and Spare Rent map belongs in the kit. | library | medium | WORK-A10-F6-P2 (Completion pack), WORK-A13-F2-P2 (Records kept for the legal period), KIT-A7-F2-P3 (Official address data), kit-store |  |
| WORK-A13-F3-P1 | **Tax rules engine**: Spare Work's tax cases on the kit's sales tax engine: in New Zealand, trades and freelance work are not listed services, so each worker charges GST by their own status. | library | small | KIT-A4-F1-P2 (Sales tax engine), KIT-A3-F1-P5 (Seller of record per app), WORK-A13-F1-P1 (Tax invoices and records for trade and freelance work) |  |
| WORK-A13-F3-P2 | **Sales tax number check**: Checks the format of a sales tax number (example: a New Zealand GST number) and records the worker's status. | integration | small | KIT-A4-F3-P2 (Seller details) |  |
| WORK-A13-F4-P1 | **Personal services in the kit's report**: Feeds Spare Work's personal services income into the kit's yearly report to the tax office and each worker's copy. | integration | small | KIT-A4-F3-P3 (Yearly tax office report), KIT-A4-F3-P2 (Seller details), KIT-A3-F3-P2 (Payout runs) |  |
| WORK-A13-F5-P1 | **Schedular payments and platforms**: Whether withholding tax rules apply to platform work, and how to offer it as an option. | research | small | WORK-A21-F4-P1 (Contractor law and the platform) |  |
| WORK-A13-F5-P2 | **Withholding at payout**: Holds back the chosen rate at payout and gives a yearly summary. | library | medium | KIT-A3-F3-P2 (Payout runs), WORK-A13-F3-P1 (Tax rules engine) |  |
| WORK-A13-F6-P2 | **Accounting connectors**: Maps Spare Work invoices, fees and payouts onto the kit's connectors to accounting software. | integration | small | KIT-A16-F8-P1 (Common connectors), WORK-A13-F1-P2 (Invoice generator) |  |
| WORK-A13-F6-P3 | **E-invoicing**: Sends invoices over the open Peppol e-invoicing network; this belongs in the kit beside its tax invoices, for every app that invoices businesses. | integration | medium | WORK-A13-F1-P2 (Invoice generator), KIT-A4-F2-P1 (Tax invoices), KIT-A16-F8-P1 (Common connectors) |  |
| WORK-A13-F7-P1 | **Due dates and reminders**: Due dates on every invoice, counted in working days where the law asks, polite reminders on a schedule the worker sets, and an overdue status. | library | small | WORK-A13-F1-P2 (Invoice generator), KIT-A8-F3-P2 (Notification router), KIT-A19-F9-P2 (Working days and notice periods) |  |
| WORK-A13-F7-P2 | **Statements of account**: A statement of all open and paid invoices for one customer. | library | small | WORK-A13-F1-P2 (Invoice generator), engine-pdf |  |
| WORK-A13-F7-P3 | **Next steps for unpaid work**: A plain guide to a payment claim under construction contracts law or a small claims tribunal claim (examples: New Zealand's Construction Contracts Act and Disputes Tribunal), with the job record exported. | legal | small | WORK-A8-F4-P2 (Claim and schedule forms), WORK-A13-F2-P1 (Receipts and job record) |  |

## WORK-A14 Reviews and reputation

Two-way reviews that only come from real jobs, shown honestly, good and bad, with a fair right of reply.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A14-F1 Review a worker | A customer can rate and review a worker after a job, with photos. | core | P1 Review model; P2 Review form; P3 Delays the worker cannot control |
| WORK-A14-F2 Review a customer | A worker can review a customer, so other workers know who communicates well and pays on time, and the customer can reply. | core | P1 Customer reviews; P2 Customer's reply, challenge and access |
| WORK-A14-F3 See honest reviews | A customer can see every review, good and bad, and how ratings are worked out. | core | P1 Review display; P3 Job size next to each review |
| WORK-A14-F4 Reply and challenge | A worker or a customer can reply in public to a review about them and ask for a review that breaks the rules to be checked. | core | P1 Public reply; P2 Review challenge |
| WORK-A14-F5 Bring references | A new worker can show references from past customers, clearly marked as from outside the platform. | later | P1 Reference requests |
| WORK-A14-F6 Stop fake reviews | The platform stops fake, paid and self reviews, and nobody can pay to remove one. | core | P1 Fake review checks; P2 Review rules |

### Parts for WORK-A14

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A14-F1-P1 | **Review model**: Spare Work's review categories on the kit's ratings, tied to a finished or cancelled agreement or a job paid outside the app, so only real jobs can be reviewed. | data | small | KIT-A9-F1-P1 (Ratings service), WORK-A9-F3-P3 (Paid outside the app) |  |
| WORK-A14-F1-P2 | **Review form**: Ratings for quality, timing, communication and value, with text and photos. | interface | small | WORK-A14-F1-P1 (Review model) |  |
| WORK-A14-F1-P3 | **Delays the worker cannot control**: Adds Spare Work's causes to the kit's rating exclusions, such as weather, council inspections and supplier shortages, so a worker is not marked down for them. | library | small | KIT-A9-F3-P1 (Rating exclusions), WORK-A14-F1-P1 (Review model) |  |
| WORK-A14-F2-P1 | **Customer reviews**: Reviews of customers shown to workers deciding whether to quote. | library | small | WORK-A14-F1-P1 (Review model) |  |
| WORK-A14-F2-P2 | **Customer's reply, challenge and access**: A customer can reply to, challenge and see every review about them, and ask for a correction under privacy law. | library | small | WORK-A14-F2-P1 (Customer reviews), WORK-A14-F4-P1 (Public reply), WORK-A14-F4-P2 (Review challenge), KIT-A18-F3-P1 (Privacy page) |  |
| WORK-A14-F3-P1 | **Review display**: Shows all reviews with the rating maths published. | interface | medium | WORK-A14-F1-P1 (Review model) |  |
| WORK-A14-F3-P3 | **Job size next to each review**: Shows the job's price band next to each review, such as under $500 or over $50,000, from the agreement, never the exact amount. | interface | small | WORK-A14-F3-P1 (Review display), WORK-A8-F1-P1 (Agreement document) |  |
| WORK-A14-F4-P1 | **Public reply**: Uses the kit's one reply per review, for workers and customers alike. | library | small | KIT-A9-F2-P1 (Reviews) |  |
| WORK-A14-F4-P2 | **Review challenge**: Uses the kit's rating challenge, open to both sides and judged by moderators with no competing business. | library | small | KIT-A9-F5-P1 (Rating challenge), WORK-A16-F2-P5 (Competitor conflict rules) |  |
| WORK-A14-F5-P1 | **Reference requests**: Asks past customers for a reference, shown apart from reviews; each request names the sender and offers one-step unsubscribe, as spam law asks. | library | medium | KIT-A8-F4-P1 (Email sending), WORK-A2-F1-P1 (Profile data model), KIT-A8-F5-P2 (Consent for messages) |  |
| WORK-A14-F6-P1 | **Fake review checks**: Adds Spare Work signals to the kit's fake review checks, such as a reviewer and a worker sharing a card, address or device. | library | small | KIT-A9-F8-P1 (Fake review checks) |  |
| WORK-A14-F6-P2 | **Review rules**: Spare Work's review rules on the kit's fair review rules: no paid removal, no hidden reviews, and removals only under a published rule, logged. | legal | small | KIT-A9-F2-P2 (Fair review rules), KIT-A10-F5-P1 (Rules library) |  |

## WORK-A15 Disputes, defects and warranties

When things go wrong, the money stays held, both sides give their side, and a member panel decides under published rules. Defects after the job are tracked too.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A15-F1 Raise a concern | A customer or worker can raise a concern on a stage and try to sort it out with guided steps first. | core | P1 Concern steps |
| WORK-A15-F2 Open a dispute | Either side can open a dispute, which keeps the money held until it is decided. | core | P1 Dispute flow; P2 Evidence and timeline |
| WORK-A15-F3 Member panel decision | A dispute can go to mediation and, for workmanship, an independent inspection, before a panel of trained members decides and splits the held money. | core | P1 Panel selection; P2 Decision and reasons; P4 Mediation first; P5 Independent inspection; P6 Appeal a panel decision; P7 Dispute clocks |
| WORK-A15-F4 Outside options | A member can see their legal options, such as a small claims tribunal (example: New Zealand's Disputes Tribunal) or adjudication, and export the file for them. | next | P1 Dispute file export; P2 Guide to outside options |
| WORK-A15-F5 Report a defect | A customer can report a defect after completion, and the worker is reminded of their duty to fix it. | core | P1 Defect reports; P2 Defect repair period |
| WORK-A15-F6 Warranties and guarantees | A customer can see the warranties and guarantees for the job and claim on them. | later | P1 Warranty records; P2 Guarantees and job cover |
| WORK-A15-F7 Protection when a worker cannot pay back | A customer can still be refunded when a decision goes against a worker who was already paid and cannot pay back, from a shared pool shown on the ledger. | next | P1 Claims on a shared pool |

### Parts for WORK-A15

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A15-F1-P1 | **Concern steps**: Guided steps with suggested fixes and deadlines before a formal dispute. | library | medium | WORK-A10-F6-P1 (Sign-off and fix requests), KIT-A11-F3-P1 (Settlement offers) |  |
| WORK-A15-F2-P1 | **Dispute flow**: Spare Work's dispute types on the kit's disputes service, freezing the stage's held money. | library | small | KIT-A11-F1-P1 (Disputes service), WORK-A9-F1-P2 (Held funds per stage) |  |
| WORK-A15-F2-P2 | **Evidence and timeline**: Attaches the workroom timeline, sign-offs and photos to the kit's evidence store. | library | small | KIT-A11-F2-P1 (Evidence store), WORK-A10-F1-P2 (Job timeline) |  |
| WORK-A15-F3-P1 | **Panel selection**: Adds Spare Work's rules to the kit's panel selection: trade knowledge, and no panel member in the same trade and region as either side. | library | small | KIT-A11-F4-P1 (Panel selection), WORK-A16-F2-P5 (Competitor conflict rules) |  |
| WORK-A15-F3-P2 | **Decision and reasons**: Records the decision in the kit's panel workspace and splits the held money by stage. | library | small | KIT-A11-F4-P2 (Panel workspace), WORK-A15-F2-P1 (Dispute flow) |  |
| WORK-A15-F3-P4 | **Mediation first**: Offers mediation, by a trained member or an outside mediator at cost, before the panel decides. | library | small | WORK-A15-F2-P1 (Dispute flow), KIT-A11-F3-P1 (Settlement offers) |  |
| WORK-A15-F3-P5 | **Independent inspection**: For a workmanship dispute, books an independent building inspection at cost, with the cost shared according to the outcome. | library | medium | WORK-A15-F2-P1 (Dispute flow), WORK-A6-F3-P1 (Service checkout) |  |
| WORK-A15-F3-P6 | **Appeal a panel decision**: Uses the kit's appeals for a second review by different members with trade knowledge, within set times, while the held money stays held. | library | small | KIT-A11-F5-P1 (Appeals), WORK-A15-F3-P1 (Panel selection), WORK-A15-F3-P2 (Decision and reasons) |  |
| WORK-A15-F3-P7 | **Dispute clocks**: Sets a deadline in working days for each dispute step, with reminders and a set outcome when time runs out, so every dispute and appeal ends well inside the provider's hold limit. | library | small | KIT-A11-F6-P1 (Dispute clocks), KIT-A19-F9-P2 (Working days and notice periods), KIT-A3-F2-P4 (Hold time limits), WORK-A15-F2-P1 (Dispute flow) |  |
| WORK-A15-F4-P1 | **Dispute file export**: Exports the full dispute file in a form an outside body can read. | library | small | engine-pdf, WORK-A15-F2-P1 (Dispute flow) |  |
| WORK-A15-F4-P2 | **Guide to outside options**: Spare Work's outside routes on the kit's guidance: a small claims tribunal, adjudication under construction contracts law and complaints to licensing boards (examples: New Zealand's Disputes Tribunal and Construction Contracts Act). | legal | small | KIT-A11-F7-P1 (Outside routes) |  |
| WORK-A15-F5-P1 | **Defect reports**: Reports a defect against the finished job with photos. | library | small | WORK-A10-F6-P1 (Sign-off and fix requests) |  |
| WORK-A15-F5-P2 | **Defect repair period**: Tracks the defect repair period each country sets, such as 12 months for all New Zealand residential building work whatever the price, and reminds both sides. | legal | small | WORK-A8-F1-P6 (Residential building duties on every job), WORK-A15-F5-P1 (Defect reports) |  |
| WORK-A15-F6-P1 | **Warranty records**: Keeps product and workmanship warranties in the completion pack with end dates. | data | small | WORK-A10-F6-P1 (Sign-off and fix requests) |  |
| WORK-A15-F6-P2 | **Guarantees and job cover**: Research on building guarantee schemes and whether job cover is worth offering at cost. | research | small |  |  |
| WORK-A15-F7-P1 | **Claims on a shared pool**: Spare Work's claims on a protection pool that belongs in the kit beside its insurance options, with income and claims shown on the ledger. | library | small | KIT-A11-F9-P1 (Insurance options), KIT-A3-F16-P1 (Loss rules), WORK-A15-F3-P2 (Decision and reasons) |  |

## WORK-A16 Trust, safety and moderation

Keeping members safe on site and online: reports, scam protection, safety on visits, and moderation under published rules with a public log.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A16-F1 Report a problem | A member can report a job, profile, quote, message or review, and see what happened to their report. | core | P1 Report button |
| WORK-A16-F2 Moderation | Moderators can review reports and act under published rules, with every action in a public log. | core | P1 Moderation queue; P2 Work content rules; P4 Rights-owner reports; P5 Competitor conflict rules |
| WORK-A16-F3 Scam and fraud protection | The platform spots fake jobs, fake workers, stolen cards and payment scams. | core | P1 Fraud signals; P2 Scam message checks; P3 Bank detail change protection |
| WORK-A16-F4 Safety on site | A worker or customer can share visit details with a trusted contact and get help fast. | core | P1 Share a visit; P2 Emergency help; P3 Report sexual harm |
| WORK-A16-F5 Site hazards | A customer can share known hazards, such as asbestos, and a worker can record site hazards. | core | P1 Hazard notes on a job; P2 Safety duties on Spare Work jobs; P3 Asbestos check before refurbishment; P4 Injuries and notifiable events |
| WORK-A16-F6 Suspensions and appeals | A member who is suspended can see why and appeal. | core | P1 Suspension and appeal; P2 Restrictions from other apps |

### Parts for WORK-A16

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A16-F1-P1 | **Report button**: Spare Work's report categories on the kit's reports, reachable from every job, profile, quote, message and review. | library | small | KIT-A10-F3-P1 (Reports service), KIT-A10-F3-P2 (Report forms) |  |
| WORK-A16-F2-P1 | **Moderation queue**: Spare Work's context in the kit's moderation queue, such as the job, quote and credential behind each report, with every decision in the kit's public removal log and a sample sent for a second review. | library | small | KIT-A10-F6-P2 (Review queue), KIT-A10-F6-P3 (Moderator tools), KIT-A10-F7-P1 (Public removal log), KIT-A10-F20-P1 (Decision sampling and conflict checks) |  |
| WORK-A16-F2-P2 | **Work content rules**: What may and may not be posted on jobs, profiles, services and reviews, including the banned services in WORK-A3-F8. | legal | small | WORK-A3-F8-P2 (Service rules as data), KIT-A10-F5-P1 (Rules library) |  |
| WORK-A16-F2-P4 | **Rights-owner reports**: A way for a photographer, firm or author to report copied portfolio work or deliverables on the kit's copyright and trade mark notices, with a reply for the member reported. | library | small | KIT-A10-F3-P1 (Reports service), KIT-A10-F21-P2 (Copyright and trade mark notices) |  |
| WORK-A16-F2-P5 | **Competitor conflict rules**: Spare Work's rule for the kit's conflict checks: nobody judges a dispute, or moderates a review or profile, of a worker in the same trade and region as their own business. | data | small | KIT-A10-F20-P1 (Decision sampling and conflict checks), KIT-A11-F4-P1 (Panel selection), KIT-A14-F6-P1 (Member roles) |  |
| WORK-A16-F3-P1 | **Fraud signals**: Adds Spare Work signals, such as fake jobs and bulk quoting, to the kit's fraud screening and shared signals. | library | small | KIT-A3-F13-P1 (Provider fraud screening), KIT-A10-F12-P2 (Fraud signals) |  |
| WORK-A16-F3-P2 | **Scam message checks**: Adds Spare Work patterns to the kit's scam warnings, such as fake deposit requests and messages asking to change bank details. | library | small | KIT-A8-F9-P1 (Scam warnings), WORK-A12-F1-P1 (Message threads) |  |
| WORK-A16-F3-P3 | **Bank detail change protection**: Uses the kit's payout change guard when a worker's bank details change, and warns a customer who is asked by message to pay into new details. | library | small | KIT-A3-F3-P4 (Payout change guard), WORK-A16-F3-P2 (Scam message checks) |  |
| WORK-A16-F4-P1 | **Share a visit**: Turns on the kit's trusted contact link for site visits. | library | small | KIT-A10-F2-P1 (Trusted contact link) |  |
| WORK-A16-F4-P2 | **Emergency help**: Turns on the kit's emergency button during visits and records a safety report. | library | small | KIT-A10-F1-P1 (Emergency button) |  |
| WORK-A16-F4-P3 | **Report sexual harm**: Opens the kit's trauma-informed sexual harm report path from site visits, messages and the job workroom. | library | small | KIT-A10-F18-P2 (Sexual harm path), WORK-A16-F1-P1 (Report button), WORK-A10-F1-P1 (Workroom page) |  |
| WORK-A16-F5-P1 | **Hazard notes on a job**: Spare Work's hazard types, such as asbestos, dogs, roof access and live services, on the kit's hazard notes, shown to every worker who visits. | data | small | KIT-A24-F5-P1 (Hazard notes), WORK-A3-F1-P1 (Job data model) |  |
| WORK-A16-F5-P2 | **Safety duties on Spare Work jobs**: Spare Work's input to the kit's safety duties: what falls on customers, workers and the co-op when trade work happens in homes and businesses. | legal | small | KIT-A24-F1-P1 (Health and safety duties), WORK-A21-F4-P1 (Contractor law and the platform) |  |
| WORK-A16-F5-P3 | **Asbestos check before refurbishment**: For buildings from before 2000, asks whether asbestos has been checked before refurbishment or demolition is agreed, and requires the safety regulator's licence for removal work (example: a WorkSafe licence in New Zealand). | legal | small | WORK-A16-F5-P1 (Hazard notes on a job), WORK-A2-F3-P7 (Professional and other registers) |  |
| WORK-A16-F5-P4 | **Injuries and notifiable events**: Records injuries and near misses on a job in the kit's incident register, and flags events that must be notified to the workplace safety regulator (example: New Zealand's WorkSafe), such as a serious injury. | library | small | KIT-A24-F2-P1 (Incident register), KIT-A24-F2-P2 (Notifiable event reports), WORK-A10-F1-P1 (Workroom page) |  |
| WORK-A16-F6-P1 | **Suspension and appeal**: Uses the kit's statements of reasons and appeals, and keeps held money safe while a member is suspended. | library | small | KIT-A10-F8-P1 (Statements of reasons), WORK-A9-F1-P2 (Held funds per stage) |  |
| WORK-A16-F6-P2 | **Restrictions from other apps**: Applies the kit's serious restrictions across apps, such as a removal for violence in Spare Ride or Spare Stay, before anyone can quote for or visit a home. | library | small | KIT-A10-F14-P1 (Restrictions across apps), WORK-A16-F6-P1 (Suspension and appeal) |  |

## WORK-A17 Running a trade or freelance business

Free tools a small business needs to run its work: calendar, customer list, crew, outside jobs, materials and reports, so there is no need to pay for separate job software.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A17-F1 Calendar | A worker can see site visits, jobs and stages on one calendar and sync it with their phone. | core | P1 Calendar model; P2 Calendar views |
| WORK-A17-F2 Customer list | A worker can see past and current customers, their jobs and notes in one list. | next | P1 Customer records; P2 Notes and follow-ups |
| WORK-A17-F3 Crew and staff | A business can add staff, give them jobs, and control what they see. | next | P1 Business team roles; P2 Assign staff; P3 Crew phone view; P4 When staff leave; P5 Office helper |
| WORK-A17-F4 Jobs from elsewhere | A worker can add jobs that came from outside Spare Work and use the same quotes, stages and invoices. | next | P1 Outside jobs |
| WORK-A17-F5 Materials and suppliers | A worker can list materials on a quote and order them from suppliers. | later | P1 Materials lists; P2 Supplier price lists |
| WORK-A17-F6 Agencies | A freelance agency can quote as a team and split work and pay among its members. | later | P1 Agency profiles and pay splits |
| WORK-A17-F7 Business reports | A worker can see their earnings, win rate, busy months and fees paid. | next | P1 Earnings and job reports; P2 Tax threshold alerts |
| WORK-A17-F8 Move in from other job software | A worker can bring their customers, price book, open quotes and portfolio from their old job software using their own export files. | next | P1 Import from export files; P2 Moving terms check |
| WORK-A17-F9 Quote form on my own website | A worker can put a quote request form or booking button on their own website that sends jobs straight to them. | next | P1 Quote form for a worker's own site |

### Parts for WORK-A17

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A17-F1-P1 | **Calendar model**: Spare Work's visits, stages, services and time off on the kit's availability calendars. | data | small | KIT-A5-F5-P1 (Availability calendars) |  |
| WORK-A17-F1-P2 | **Calendar views**: Day, week and crew views, with drag to move. | interface | medium | WORK-A17-F1-P1 (Calendar model) |  |
| WORK-A17-F2-P1 | **Customer records**: Customer records built from jobs, private to the worker. | library | small | WORK-A3-F1-P1 (Job data model) |  |
| WORK-A17-F2-P2 | **Notes and follow-ups**: Private notes and follow-up reminders, such as a yearly service; a reminder that offers a price counts as an uninvited sale and carries its notice. | library | small | KIT-A8-F3-P2 (Notification router), WORK-A17-F2-P1 (Customer records), WORK-A8-F1-P9 (Uninvited direct sales) |  |
| WORK-A17-F3-P1 | **Business team roles**: Owner, office and crew roles for a worker business on the kit's organisation model. | library | small | KIT-A1-F7-P1 (Organisation model) |  |
| WORK-A17-F3-P2 | **Assign staff**: Assigns staff to visits and stages. | library | small | WORK-A17-F1-P1 (Calendar model) |  |
| WORK-A17-F3-P3 | **Crew phone view**: A simple phone view of today's jobs, addresses and tasks, working offline. | interface | medium | WORK-A20-F3-P1 (Offline jobs) |  |
| WORK-A17-F3-P4 | **When staff leave**: Sets what a person takes when they leave a business or agency, such as their own licence record and the portfolio items they made, and what stays, such as the business's reviews and job history. | legal | small | WORK-A17-F3-P1 (Business team roles), WORK-A14-F1-P1 (Review model) |  |
| WORK-A17-F3-P5 | **Office helper**: A partner or helper can do a sole trader's admin through the kit's delegated access, within limits that never include payouts. | library | small | KIT-A1-F8-P1 (Delegated access) |  |
| WORK-A17-F4-P1 | **Outside jobs**: Manual jobs with the same tools; the fee applies only to money moving through the platform. | library | medium | WORK-A3-F1-P1 (Job data model), WORK-A9-F4-P1 (Fee calculation) |  |
| WORK-A17-F5-P1 | **Materials lists**: Materials lines on quotes and stages, with quantities. | library | small | WORK-A7-F1-P4 (Price book) |  |
| WORK-A17-F5-P2 | **Supplier price lists**: Supplier price lists and orders through plugins. | integration | medium | kit-plugin |  |
| WORK-A17-F6-P1 | **Agency profiles and pay splits**: Agency profiles with members and agreed pay splits. | library | medium | WORK-A17-F3-P1 (Business team roles), KIT-A5-F11-P1 (Multi-party orders) |  |
| WORK-A17-F7-P1 | **Earnings and job reports**: Reports on earnings, quotes won, and fees, exportable. | interface | medium | KIT-A3-F3-P3 (Earnings page), WORK-A7-F1-P2 (Quote data model) |  |
| WORK-A17-F7-P2 | **Tax threshold alerts**: Warns a worker whose earnings near the sales tax registration threshold (example: $60,000 for New Zealand GST), and links the kit's guidance on levies and provisional tax (example: New Zealand's ACC levies). | library | small | KIT-A23-F2-P4 (Savings, accident cover and tax guidance), WORK-A17-F7-P1 (Earnings and job reports), WORK-A13-F3-P1 (Tax rules engine) |  |
| WORK-A17-F8-P1 | **Import from export files**: Reads the worker's own CSV or open export files into draft customers, price book items, quotes and portfolio entries; reviews are never scraped from other sites. | service | medium | WORK-A17-F2-P1 (Customer records), WORK-A7-F1-P4 (Price book), WORK-A2-F2-P1 (Portfolio items) |  |
| WORK-A17-F8-P2 | **Moving terms check**: Which job software lets members export their own data, and whether past reviews may be shown as outside references. | research | small |  |  |
| WORK-A17-F9-P1 | **Quote form for a worker's own site**: A small form or button for a worker's own website that creates a direct job to them, with no tracking and the kit's spam limits. | interface | small | WORK-A3-F5-P1 (Open, invited or direct), WORK-A3-F1-P2 (Job posting form), KIT-A10-F11-P1 (Spam limits), KIT-A20-F4-P1 (Deep links) |  |

## WORK-A18 Membership, fees and voting

Workers keep all the profit except running costs. They see the costs, vote on the fee and the rules, and take part in panels.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A18-F1 Become a member | A worker can become a member, free, with one vote. | core | P1 Member sign-up; P2 Spare Work member rules |
| WORK-A18-F2 See the costs | Anyone can see what Spare Work cost to run each month and how the fee was set. | core | P1 Spare Work ledger page |
| WORK-A18-F3 Vote on fees and rules | A worker member can propose and vote on the fee, the holding threshold, matching rules and categories. | core | P1 Proposals and votes; P3 What members may vote on |
| WORK-A18-F4 Serve on a panel | A member can volunteer for dispute and moderation panels and be trained. | next | P1 Panel volunteers |
| WORK-A18-F5 Customer voice | Customers can give feedback and take part in consultations. | later | P1 Consultations; P2 Customer membership |

### Parts for WORK-A18

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A18-F1-P1 | **Member sign-up**: Joins the co-op from the worker profile through the kit's join flow. | interface | small | KIT-A14-F1-P3 (Join flow) |  |
| WORK-A18-F1-P2 | **Spare Work member rules**: Who counts as a Spare Work worker member and what they vote on, drafted with a lawyer. | legal | medium |  |  |
| WORK-A18-F2-P1 | **Spare Work ledger page**: Spare Work's monthly costs, jobs and fee on the open cost ledger. | interface | small | KIT-A13-F1-P3 (Ledger pages) |  |
| WORK-A18-F3-P1 | **Proposals and votes**: Spare Work's proposal types, such as the fee, the holding threshold, matching rules and categories, on the kit's proposals and ballots. | interface | small | KIT-A12-F1-P1 (Proposals service), KIT-A12-F3-P1 (Ballots) |  |
| WORK-A18-F3-P3 | **What members may vote on**: Spare Work's questions for the kit's competition law check: the platform fee is one thing, but competitors jointly setting deposit caps, call-out charges or other customer prices may be a cartel provision. | legal | small | KIT-A12-F4-P3 (Competition law check), WORK-A18-F1-P2 (Spare Work member rules) |  |
| WORK-A18-F4-P1 | **Panel volunteers**: Spare Work's needs for the kit's member roles, panel training and training certificates: trade knowledge and enough panel members in each region. | library | small | KIT-A11-F4-P3 (Panel pay), KIT-A14-F6-P1 (Member roles), KIT-A25-F2-P1 (Training and certificates) |  |
| WORK-A18-F5-P1 | **Consultations**: Open consultations on proposed changes that customers can answer. | interface | small | KIT-A12-F2-P1 (Discussion and amendments) |  |
| WORK-A18-F5-P2 | **Customer membership**: What customers, who are free members too, vote on beyond the major changes every member votes on. | research | small |  |  |

## WORK-A19 Links to other programs, AI and plugins

Spare Work is the services layer for the other programs, and everything a person can do, an AI agent or script can do through the same commands.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A19-F1 Command API | A script or another program can do anything a person can, through Spare Work's command and query API, and be told when something changes. | core | P1 Work command schemas; P2 API docs, limits and test mode; P3 Work events |
| WORK-A19-F2 AI access | A member can let an AI agent post jobs, write quotes or track stages, with their permission. | core | P1 Spare Work MCP server; P2 Agent permissions; P3 Agent quote limits; P4 Stranger content is data; P5 Agent activity with undo |
| WORK-A19-F3 Maintenance from Spare Rent | A landlord or tenant can raise a maintenance job in Spare Rent that becomes a Spare Work job. | core | P1 Job intake from other programs; P2 Who approves and pays |
| WORK-A19-F4 Services for Spare Home | A seller or buyer can book photographers, inspectors, valuers, lawyers and licensed agents from Spare Home, checked and rated once in one shared catalogue. | core | P1 Home's services in the shared catalogue; P2 One check and one reputation |
| WORK-A19-F5 Tender a Spare Construct design | A designer can send a Spare Construct project out to trades for quotes. | later | P1 Tender package |
| WORK-A19-F6 Plugins | A worker or business can add plugins for their trade, such as a roofing calculator. | later | P1 Plugin slots |
| WORK-A19-F7 Local services for other programs | A host in Spare Stay or a mover in Spare Home can find cleaners, co-hosts and movers in the same shared catalogue, so local services have one home. | next | P1 Local services in the shared catalogue |

### Parts for WORK-A19

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A19-F1-P1 | **Work command schemas**: Every Spare Work action described once in the kit's command schemas, from which the kit generates the API. | data | medium | KIT-A15-F1-P3 (Command schemas), KIT-A16-F1-P1 (Command API), kit-market |  |
| WORK-A19-F1-P2 | **API docs, limits and test mode**: Spare Work examples and limits on the kit's developer docs and rate limits, with test jobs and test stage payments in the kit's test mode, scoped tokens and versioned APIs. | data | small | KIT-A16-F3-P2 (Developer docs), KIT-A16-F1-P3 (API rate limits), WORK-A19-F1-P1 (Work command schemas), KIT-A16-F3-P3 (Test mode), KIT-A16-F1-P2 (API keys and tokens), KIT-A16-F9-P1 (Versions and deprecation), KIT-A16-F1-P4 (Bulk copy limits) |  |
| WORK-A19-F1-P3 | **Work events**: Declares the events Spare Work sends through the kit's signed webhooks, such as quote received, agreement signed, stage funded, stage done, visit booked, dispute opened and invoice issued. | data | small | KIT-A16-F2-P1 (Webhooks), WORK-A19-F1-P1 (Work command schemas) |  |
| WORK-A19-F2-P1 | **Spare Work MCP server**: Spare Work's commands offered on the kit's MCP server. | integration | small | KIT-A17-F1-P1 (MCP server), WORK-A19-F1-P1 (Work command schemas) |  |
| WORK-A19-F2-P2 | **Agent permissions**: Marks the commands that always need the person's own approval, on the kit's approval steps and people-only actions: accept a quote, sign an agreement, approve a change, fund or pay a stage, sign off or release a stage, approve time, cancel an agreement, send a payment claim or payment schedule, share the exact address or access notes, open a dispute, change payout details and close the account. | data | small | KIT-A17-F2-P1 (Approval steps), KIT-A17-F4-P1 (Agent limits), WORK-A19-F1-P1 (Work command schemas), KIT-A17-F9-P2 (People-only actions) |  |
| WORK-A19-F2-P3 | **Agent quote limits**: Limits how many quotes an agent can send, on top of the kit's agent labels on every quote and message an agent writes. | library | small | WORK-A19-F2-P1 (Spare Work MCP server), WORK-A4-F2-P3 (Cap on quotes per job), KIT-A6-F4-P3 (Automated acceptance rules), KIT-A17-F9-P1 (Agent labels) |  |
| WORK-A19-F2-P4 | **Stranger content is data**: Uses the kit's hidden instruction guards, so job descriptions, quotes, messages, reviews and attached PDFs written by others are treated as data when an agent reads them, never as instructions. | library | small | KIT-A17-F5-P1 (Hidden instruction guards), WORK-A19-F2-P1 (Spare Work MCP server) |  |
| WORK-A19-F2-P5 | **Agent activity with undo**: Shows what a member's agent did on each job in the workroom, from the kit's agent activity, with undo where the action allows it. | interface | small | KIT-A17-F3-P1 (Agent activity), WORK-A10-F1-P1 (Workroom page) |  |
| WORK-A19-F3-P1 | **Job intake from other programs**: Takes jobs from other programs through the public command API, never a code dependency. | integration | medium | WORK-A19-F1-P1 (Work command schemas) |  |
| WORK-A19-F3-P2 | **Who approves and pays**: Maps a Spare Rent repair onto job roles: the landlord pays, the property manager approves and discloses any commission or markup, the tenant is the site contact, and both confirm completion. | library | medium | WORK-A3-F1-P5 (Roles on a job), WORK-A10-F6-P3 (Sign-off by more than one party), WORK-A3-F1-P6 (Disclosed commissions and markups) |  |
| WORK-A19-F4-P1 | **Home's services in the shared catalogue**: Spare Work's photographers, lawyers, valuers, inspectors and licensed agents are entries in the shared service catalogue, which Spare Home books through the kit's marketplace engine without calling Spare Work. | integration | small | WORK-A6-F1-P3 (Entries in the shared service catalogue), kit-market |  |
| WORK-A19-F4-P2 | **One check and one reputation**: Each pro's licence checks and reviews live in the kit's register checks and ratings, so Spare Home shows the same results without checking or rating them again. | integration | small | KIT-A2-F2-P1 (Public register checks), KIT-A9-F1-P1 (Ratings service), WORK-A2-F3-P2 (Registers each trade needs), WORK-A14-F1-P1 (Review model) |  |
| WORK-A19-F5-P1 | **Tender package**: Builds a job with drawings, schedules and quantities from a Spare Construct project through Spare Construct's public command API. | integration | large | KIT-A16-F1-P1 (Command API), WORK-A19-F1-P1 (Work command schemas) |  |
| WORK-A19-F6-P1 | **Plugin slots**: Places in quotes, jobs and profiles where sandboxed plugins can add tools. | library | medium | kit-plugin |  |
| WORK-A19-F7-P1 | **Local services in the shared catalogue**: Offers cleaning, turnovers, co-hosting and moving as entries in the shared service catalogue, which Spare Stay and Spare Home book through the kit's marketplace engine, instead of separate service listings in those apps. | integration | small | WORK-A6-F1-P3 (Entries in the shared service catalogue), WORK-A3-F7-P1 (Recurring jobs), kit-market |  |

## WORK-A20 Access for everyone: phones, offline, languages and countries

Usable by everyone, on any phone, on site with poor signal, in many languages, and ready for any country's licences, tax and law.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A20-F1 Accessible to all | A member using a screen reader, keyboard, large text or high contrast can do everything. | core | P1 Accessibility standard; P3 Accessible legal documents; P4 Captions on job videos; P5 Display settings |
| WORK-A20-F2 Phone app | A worker can run their whole business from a phone, and a customer can do everything from theirs. | core | P1 Phone app; P3 Camera and files |
| WORK-A20-F3 Work offline on site | A worker can see job details, write quotes, take photos and notes, and take an in-person sign-off with no signal, and sync later. | next | P1 Offline jobs; P2 Offline photos and notes; P3 Quote offline on site; P4 Sign off in person |
| WORK-A20-F4 Many languages | A member can use Spare Work in their own language, starting with English and each launch country's own languages (example: te reo Māori). | core | P1 Translation system; P2 Trade words in launch country languages (example: te reo Māori); P3 Long words; P4 Languages by need |
| WORK-A20-F5 Launch in any country | Spare Work can run in any country with that country's licences, tax and law, set in its own country pack. | later | P1 Country rule packs; P2 Country launch checklist; P3 Platform-work law per country |
| WORK-A20-F6 Slow connections and old phones | A member on a slow connection or an old phone can still use the app. | next | P1 Performance budget |

### Parts for WORK-A20

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A20-F1-P1 | **Accessibility standard**: Spare Work screens built from the kit's accessible components, in plain words, and tested with disabled members on Spare Work's own flows. | library | medium | KIT-A19-F2-P1 (Accessible components), KIT-A19-F2-P2 (Accessibility testing) |  |
| WORK-A20-F1-P3 | **Accessible legal documents**: Makes agreements, invoices, payment claims, payment schedules and completion packs as the kit's accessible PDFs. | library | small | KIT-A19-F11-P1 (Accessible PDFs), WORK-A8-F1-P1 (Agreement document), WORK-A13-F1-P2 (Invoice generator), WORK-A8-F4-P2 (Claim and schedule forms), WORK-A10-F6-P2 (Completion pack) |  |
| WORK-A20-F1-P4 | **Captions on job videos**: Adds the kit's captions and transcripts to portfolio and job videos. | library | small | KIT-A19-F4-P1 (Captions), WORK-A2-F2-P2 (Job and portfolio media) |  |
| WORK-A20-F1-P5 | **Display settings**: Uses the kit's text size, contrast and motion settings on every Spare Work screen. | library | small | KIT-A19-F3-P1 (Display settings), WORK-A20-F1-P1 (Accessibility standard) |  |
| WORK-A20-F2-P1 | **Phone app**: Spare Work's screens in the kit's installable phone app. | interface | medium | KIT-A20-F2-P2 (Installable apps), kit-sync |  |
| WORK-A20-F2-P3 | **Camera and files**: Camera, photo library and file access for jobs, quotes and evidence. | interface | small | WORK-A2-F2-P2 (Job and portfolio media) |  |
| WORK-A20-F3-P1 | **Offline jobs**: What Spare Work keeps on the device for the kit's offline sync: the day's jobs, quotes in progress and the price book. | library | small | KIT-A15-F4-P2 (Sync engine), kit-store |  |
| WORK-A20-F3-P2 | **Offline photos and notes**: Takes progress photos and stage notes with no signal and syncs them later. | library | medium | WORK-A20-F3-P1 (Offline jobs), WORK-A2-F2-P2 (Job and portfolio media), WORK-A8-F2-P1 (Stage model) |  |
| WORK-A20-F3-P3 | **Quote offline on site**: Writes and saves a quote with no signal and sends it once back online. | library | medium | WORK-A20-F3-P1 (Offline jobs), WORK-A7-F1-P3 (Quote builder) |  |
| WORK-A20-F3-P4 | **Sign off in person**: A customer with no phone can sign off a stage on the worker's device with a code given at signing; money actions wait and are confirmed once back online. | library | medium | WORK-A20-F3-P1 (Offline jobs), WORK-A10-F6-P1 (Sign-off and fix requests) |  |
| WORK-A20-F4-P1 | **Translation system**: Spare Work's trade words and templates on the kit's translation framework and local formats. | library | small | KIT-A19-F1-P1 (Translation framework), KIT-A19-F1-P2 (Local formats) |  |
| WORK-A20-F4-P2 | **Trade words in launch country languages (example: te reo Māori)**: A glossary of trade words in each launch country's own languages, starting with te reo Māori, reviewed through the kit's member translation and language quality checks. | data | small | KIT-A19-F1-P3 (Member translation), KIT-A19-F10-P3 (Translation quality checks), WORK-A20-F4-P1 (Translation system) |  |
| WORK-A20-F4-P3 | **Long words**: Checks Spare Work's screens with long words, such as te reo Māori trade terms; right-to-left scripts come later with the kit's layout. | interface | small | WORK-A20-F4-P1 (Translation system) |  |
| WORK-A20-F4-P4 | **Languages by need**: Which languages Spare Work's workers and customers need in each country, such as, in New Zealand, Samoan, Tongan, Hindi, Mandarin and Tagalog after English and te reo Māori, planned by need. | research | small | WORK-A20-F4-P1 (Translation system), KIT-A19-F1-P3 (Member translation) |  |
| WORK-A20-F5-P1 | **Country rule packs**: Spare Work's layer on the kit's country packs: licences, registers, building law, consumer law and platform-work law for each country. | data | medium | KIT-A19-F6-P1 (Country packs), WORK-A2-F3-P5 (Licence rules per trade), WORK-A13-F3-P1 (Tax rules engine) |  |
| WORK-A20-F5-P2 | **Country launch checklist**: What must be researched and set up before Spare Work launches in a new country. | research | medium | WORK-A20-F5-P1 (Country rule packs) |  |
| WORK-A20-F5-P3 | **Platform-work law per country**: Laws that change matching, ranking and pausing, such as the EU Platform Work Directive, due 2 December 2026, EU trader traceability, and Australia's minimum standards for employee-like platform workers. | legal | large | WORK-A20-F5-P1 (Country rule packs), WORK-A21-F4-P1 (Contractor law and the platform), KIT-A18-F13-P1 (Overseas platform duties) |  |
| WORK-A20-F6-P1 | **Performance budget**: Spare Work's size and speed limits on the kit's light mode, checked each release. | library | small | KIT-A19-F5-P1 (Light mode for slow connections) |  |

## WORK-A21 Privacy, data and legal duties

Members control their data and can take it with them. The platform meets every legal duty that applies to a work marketplace.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A21-F1 Export my data | A member can download all their jobs, quotes, messages, reviews and payments in open formats. | core | P1 Full data export; P2 Portable history; P3 Business tools export |
| WORK-A21-F2 Delete my data | A member can delete their account and data, except what the law says must be kept. | core | P1 Deletion with legal holds |
| WORK-A21-F3 Control my privacy | A member can see and control who sees their address, phone, photos and history. | core | P1 Privacy settings and statement; P2 Spare Work collection notices; P3 Photos of homes; P4 Privacy impact checks |
| WORK-A21-F4 Keep workers independent | The platform keeps workers in control of their own business and on the right side of employment law. | core | P1 Contractor law and the platform; P2 Product rules for independence; P3 Declare worker status; P4 Worker terms |
| WORK-A21-F5 Consumer and building law | Customers get the protections the law gives them, shown in plain words. | core | P1 Consumer law duties; P3 Terms of service |
| WORK-A21-F6 Anti-money-laundering | The platform meets the anti-money-laundering duties of an online marketplace, mostly through the payment provider. | core | P1 Anti-money-laundering duties |

### Parts for WORK-A21

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A21-F1-P1 | **Full data export**: Adds Spare Work's jobs, quotes, agreements, completion packs and invoices to the kit's data export. | library | small | KIT-A18-F1-P1 (Data export) |  |
| WORK-A21-F1-P2 | **Portable history**: Adds job history and completed stages to the kit's portable reputation. | library | small | KIT-A9-F7-P1 (Portable reputation), WORK-A21-F1-P1 (Full data export) |  |
| WORK-A21-F1-P3 | **Business tools export**: Adds the worker's business tools to the export: customer list, price book, quote templates, calendar as ICS, time records and saved replies, in CSV or open formats that other job software can read. | library | small | WORK-A21-F1-P1 (Full data export), WORK-A7-F1-P4 (Price book), WORK-A17-F1-P1 (Calendar model), WORK-A7-F8-P2 (Time on site), KIT-A5-F5-P2 (Calendar import and export) |  |
| WORK-A21-F2-P1 | **Deletion with legal holds**: Spare Work's legal holds on the kit's account deletion: tax, payment, dispute and building records are kept for their period, then removed. | library | small | KIT-A18-F2-P1 (Account deletion), KIT-A18-F6-P1 (Retention schedules) |  |
| WORK-A21-F3-P1 | **Privacy settings and statement**: Spare Work's settings for address, phone and photos on the kit's privacy page. | interface | small | KIT-A18-F3-P1 (Privacy page) |  |
| WORK-A21-F3-P2 | **Spare Work collection notices**: Notices for what Spare Work collects about people from others, such as register records and reviews of customers, on the kit's collection notices, including New Zealand's principle 3A from 1 May 2026. | legal | small | KIT-A18-F5-P1 (Collection notices) |  |
| WORK-A21-F3-P3 | **Photos of homes**: Warns before sharing photos that show people or valuables; location data is stripped from every copy others see. | library | small | WORK-A2-F2-P2 (Job and portfolio media) |  |
| WORK-A21-F3-P4 | **Privacy impact checks**: Runs the kit's privacy impact assessments on reviews of customers, register lookups and background checks before each goes live. | legal | small | KIT-A18-F10-P1 (Privacy impact assessments), WORK-A14-F2-P1 (Customer reviews), WORK-A2-F3-P2 (Registers each trade needs), WORK-A2-F5-P1 (Background check options) |  |
| WORK-A21-F4-P1 | **Contractor law and the platform**: Spare Work's input to the kit's worker status test: how trades and freelancers work on Spare Work, tested against the gateway test. | research | medium | KIT-A23-F1-P1 (Worker status test) | [WK03](STATUS.md) |
| WORK-A21-F4-P2 | **Product rules for independence**: Workers set their own prices and hours, are free to refuse any job and are never pushed down for declining, built into the product. | legal | medium | WORK-A21-F4-P1 (Contractor law and the platform) |  |
| WORK-A21-F4-P3 | **Declare worker status**: Declares Spare Work workers' status in the kit's worker status per app, as independent contractors unless WK03 finds otherwise, which switches on the matching pay, tax and agreement rules. | data | small | KIT-A23-F1-P2 (Worker status per app), WORK-A21-F4-P1 (Contractor law and the platform) |  |
| WORK-A21-F4-P4 | **Worker terms**: Spare Work's terms with workers on the kit's contractor agreements, stating that each worker runs their own business and contracts with each customer. | legal | small | KIT-A23-F1-P3 (Contractor agreements), WORK-A21-F4-P1 (Contractor law and the platform) |  |
| WORK-A21-F5-P1 | **Consumer law duties**: Spare Work's duties on the kit's consumer law research: guarantees for services and building law's implied warranties (example: New Zealand's Building Act), for the platform and for workers. | legal | small | KIT-A18-F8-P1 (Consumer law for platforms), WORK-A8-F1-P6 (Residential building duties on every job) |  |
| WORK-A21-F5-P3 | **Terms of service**: Spare Work's terms on the kit's plain terms, with no unfair contract terms. | legal | small | KIT-A18-F4-P2 (Terms and privacy policy) |  |
| WORK-A21-F6-P1 | **Anti-money-laundering duties**: Spare Work's risks for the kit's anti-money-laundering programme, such as cross-border payments and payment agent jobs. | legal | small | KIT-A4-F5-P1 (Anti-money-laundering programme), WORK-A3-F8-P2 (Service rules as data) |  |

## WORK-A22 Support and administration

Help for members when they need it, and the tools a small co-op team needs to run Spare Work safely, fairly and at cost.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| WORK-A22-F1 Get help | A member can find answers, contact support, and track their request. | core | P1 Help centre; P3 Support cases and channels |
| WORK-A22-F2 Admin console | Staff can look up accounts, jobs and payments, with every action logged. | core | P1 Admin console |
| WORK-A22-F3 Settings by vote | Staff can change categories, fees and settings only as member votes decide. | core | P1 Voted settings; P2 Customer protection floors |
| WORK-A22-F4 Work queues | Staff and panels can work the queues for credential checks, reports and stuck payments. | core | P1 One work queue |
| WORK-A22-F5 Health and monitoring | Staff can see uptime, errors and costs, and publish them with the ledger. | core | P1 Monitoring |
| WORK-A22-F6 Grow fairly | The co-op can bring in workers and customers region by region without selling leads. | next | P1 Launch plan; P2 Bring your customers |

### Parts for WORK-A22

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| WORK-A22-F1-P1 | **Help centre**: Spare Work's articles in the kit's help centre, for customers and workers in each language. | data | small | KIT-A21-F1-P1 (Help centre), WORK-A20-F4-P1 (Translation system) |  |
| WORK-A22-F1-P3 | **Support cases and channels**: Spare Work's case types, such as a stuck stage payment or a licence check, on the kit's support cases, with urgent routing for safety on site, phone and relay service channels (example: NZ Relay), and the co-op's complaints process. | library | small | KIT-A21-F2-P1 (Support cases), KIT-A21-F3-P1 (Urgent support routing), KIT-A21-F4-P1 (Other support channels), KIT-A21-F9-P2 (Complaints about the co-op) |  |
| WORK-A22-F2-P1 | **Admin console**: Spare Work's views in the kit's admin console, such as stuck stages and credential reviews. | interface | small | KIT-A22-F6-P1 (Admin console) |  |
| WORK-A22-F3-P1 | **Voted settings**: Spare Work's settings, such as the fee, holding threshold, release days, quote caps and categories, which change only through the kit's results to settings or a logged emergency action. | library | small | KIT-A12-F4-P1 (Results to settings) |  |
| WORK-A22-F3-P2 | **Customer protection floors**: Limits set in the co-op's rules that worker votes cannot cross: a minimum number of days before automatic release, a maximum holding threshold, a maximum deposit and a minimum number of quotes a job may get, with a customer consultation before any change that weakens customer protection. | legal | small | WORK-A22-F3-P1 (Voted settings), KIT-A12-F11-P2 (Consultations and surveys), KIT-A18-F8-P1 (Consumer law for platforms) |  |
| WORK-A22-F4-P1 | **One work queue**: Spare Work's queue types, such as credential reviews, disputes and stuck payments, in the kit's support console and moderation queue. | interface | small | KIT-A21-F2-P2 (Support console), KIT-A10-F6-P2 (Review queue), WORK-A16-F2-P1 (Moderation queue), WORK-A15-F2-P1 (Dispute flow) |  |
| WORK-A22-F5-P1 | **Monitoring**: Spare Work's health checks, such as payment holds and register lookups, on the kit's monitoring and status page. | library | small | KIT-A22-F1-P1 (Monitoring and alerts), KIT-A21-F5-P1 (Status page) |  |
| WORK-A22-F6-P1 | **Launch plan**: Which region and trades to start with, and how many workers each needs. | research | medium |  |  |
| WORK-A22-F6-P2 | **Bring your customers**: A worker can invite their own customers to pay through held stages, with consent, a named sender and one-step unsubscribe; an invite that offers a price carries the uninvited sale notice. | library | small | KIT-A8-F4-P1 (Email sending), WORK-A3-F5-P2 (Invite by link), KIT-A8-F5-P2 (Consent for messages), WORK-A8-F1-P9 (Uninvited direct sales) |  |

## Questions for assessing this map and its demo

- Does every step a customer and a worker take on the incumbents have a home on this map, from first sign-up to the defect repair period?
- Is free to quote, always, workable? Which fee basis in WORK-A9-F4-P4 covers the running cost of jobs paid outside the app?
- What keeps customers and workers using Spare Work after the first job, without trapping them or hiding contact details?
- Which core parts are truly needed for one trade in one region to win and finish real paid work?
- Are the licence checks real? Which public registers allow automated lookups under their terms (piece WK02), does each licence match the verified person, and is the licence holder the one signed in before each licensed stage?
- Can the payment provider hold stage money long enough? Stripe holds manual payouts for at most 90 days outside the US and offers no escrow. Does funding each stage just in time keep every stage, dispute and appeal inside that limit (piece WK01, WORK-A9-F1-P5), and who bears a chargeback after a stage is released?
- Does the contractor design keep workers independent under the 2026 gateway test, and does piece WK03 feed the kit's worker status test (KIT-A23-F1-P1) instead of repeating it?
- Is the building law pack for New Zealand correct and current: the disclosure statement and checklist at $30,000 or on request, and the warranties, defect repair period and record of work on every residential job?
- Does the quote, stage and sign-off model work for a small repair, a charge-up job, a renovation and a remote design job alike (piece WK04)?
- Do Spare Work's stage invoices count as payment claims under construction contracts law (example: New Zealand's Construction Contracts Act)?
- Remote work in the first release is limited to people resident in the same country (WORK-A3-F8-P5), while ARCHITECTURE.md leans 'Remote work: Later'. Is that the right line until the sales tax rule for remote services from overseas workers is live?
- Is the member dispute panel fair and fast enough, with mediation and inspections first, and who pays for panel members' time?
- Are the customer protection floors (WORK-A22-F3-P2) set at the right levels, and who sets them while only workers vote?
- Can an agent ever move money, end an agreement, serve a legal notice or share an address without the person (WORK-A19-F2-P2), and can hidden text in a job, quote, review or PDF ever reach an agent as an instruction (WORK-A19-F2-P4)?
- Is the at-cost fee shown clearly everywhere money moves, with a tax invoice for it?
- What may worker members vote on without breaking competition law (example: New Zealand's Commerce Act)? Does the kit's competition law check (KIT-A12-F4-P3) answer Spare Work's questions on deposit caps and call-out charges (WORK-A18-F3-P3)?
- Does anything on the map copy an incumbent's interface, wording or patented process rather than solve the job our own way?
- What must the shared kit deliver first? Spare Work's core relies on kit parts the kit marks next: business checks, verified badges, quote requests, saved searches, match explanations and fairness checks, favourites, scam warnings and scam signals, fake review checks, portable reputation, member roles, rules for parents and carers, signatures, bank payments, order changes and extra charges, scheduled and repeat bookings, accessible PDFs, captions, repeat face checks, hazard notes, decision sampling, restrictions across apps, copyright notices, the AI use register and human review of AI decisions, co-op AI at cost, privacy impact assessments, after-death handling, language quality checks, consultations, complaints about the co-op and API versions. Insurance options are kit later under Spare Work's protection pool (next). Will the kit move these up?
- Will the kit add one shared service catalogue on its marketplace engine, with Spare Work as the main place pros join, are checked and are rated, so Spare Home and Spare Stay book the same people with one account and one reputation (WORK-A6-F1-P3)?
- Will the kit take the parts other apps also need, which Spare Work maps for now: a regional emergency mode, a manual document check, invoice numbering with credit notes, Peppol e-invoicing, card-present payments and the shared property record that Spare Home and Spare Rent also map?
- Will the kit open its own pieces for public register checks (KIT-A2-F2-P1) and tax invoices (KIT-A4-F2-P1), so WK02 and WK05 become Spare Work's input and the kit never waits on a program?
- Are the core priorities realistic for a small volunteer team, given what Spare Rent and Spare Home need from Spare Work at launch?
- Should the root ARCHITECTURE.md list Work as a user of Maps and location, since Spare Work uses service areas and job maps?

The work board is [STATUS.md](STATUS.md).
