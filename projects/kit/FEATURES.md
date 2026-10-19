# Shared kit and co-op portal: feature map

*Stage: Mapping. Everything here is open for discussion until an owner accepts a decision note. The machine-readable copy is [features.json](features.json); the architecture is [ARCHITECTURE.md](ARCHITECTURE.md) and the work board is [STATUS.md](STATUS.md).*

The shared kit is built once and used by every Spareware app, so a new app is a thin layer of its own rules on top. This map lists everything the kit and the co-op's member portal need, top-down: areas, then what a person can do in each, then the buildable parts. It was drawn from every journey a member takes across the apps (first sign-up, daily use, being paid, a dispute, a vote, leaving) and from what the incumbent services offer, with the law in New Zealand first and other countries planned for. It also holds every part that two or more programs share, such as the property record Spare Home, Rent and Stay use, so no program depends on another, and it maps the shared engines (KIT-A27), which are built in projects/engines, so every program has one place to depend on. Money is always held by a regulated payment provider, never by the co-op. Nothing here is decided: each part becomes a design note before any code.

**Replaces:** The services every platform rebuilds or rents: sign-in and identity services (such as Auth0 and Okta), map and routing services (such as Google Maps Platform and Mapbox), chat, call and notification services (such as Twilio, SendGrid, Firebase and Zoom), e-signature services (such as DocuSign), trust and safety tools, voting tools (such as Loomio and Decidim), open budget tools (such as Open Collective) and back-office systems, plus the member register and portal a co-op would otherwise run by hand.

**In numbers:** 28 areas, 369 features (287 core), 765 buildable parts. Priority: **core** is needed for a first usable release of the kit or of any program that builds on it, **next** comes soon after, **later** after that. Parts name the kit crate, service, engine or other part they depend on, and the work-board piece that covers them where one exists. Service and engine names other than the kit's eight existing services and five existing engines are proposals (KIT-A16-F10-P1).

## Areas

| ID | Area | What it covers | Features | Core |
| --- | --- | --- | --- | --- |
| KIT-A1 | [Accounts and sign-in](#kit-a1-accounts-and-sign-in) | One Spareware login for every app: signing up, signing in safely, getting back into a lost account, devices, roles, teams, and the sign-in service other apps and AI agents can use. Built as svc-accounts. | 17 | 12 |
| KIT-A2 | [Identity, licence and business checks](#kit-a2-identity-licence-and-business-checks) | Proves people are who they say and hold the licences their work needs, through accredited providers and public registers. The co-op keeps the result; the provider keeps the documents under contract, and the co-op can get them back when the law asks. Built in svc-accounts; svc-identity is a proposed name (KIT-A16-F10-P1). | 14 | 13 |
| KIT-A3 | [Payments and payouts](#kit-a3-payments-and-payouts) | Customers pay, a regulated provider holds the money within its time limits, workers are paid everything except the at-cost fee, and every dollar is shown, including who carries a loss, payments taken in person and payments made outside the app. The co-op never holds money or card numbers itself. Built as svc-payments. | 23 | 17 |
| KIT-A4 | [Tax, invoices and money law](#kit-a4-tax-invoices-and-money-law) | The tax and money law every marketplace must meet, built once: GST on listed services, the co-op's own GST returns, tax invoices, reporting sellers' income to IRD, anti-money-laundering duties and record keeping. Built in svc-payments; svc-tax is a proposed name (KIT-A16-F10-P1). | 13 | 10 |
| KIT-A5 | [Marketplace engine](#kit-a5-marketplace-engine) | The one loop every marketplace runs (list or ask, match, agree, pay and hold, do the job, confirm, pay out, rate, resolve), plus changes, extra charges and promotions, built once in kit-market, with each app adding only its own rules. | 32 | 23 |
| KIT-A6 | [Search, matching and dispatch](#kit-a6-search-matching-and-dispatch) | Pairs a request with the right worker or listing (search with filters, nearest driver, ready-timed courier, open dates, best quote) using rules anyone can read, fair to workers and customers alike. Built in svc-matching; svc-search is a proposed name (KIT-A16-F10-P1). | 10 | 7 |
| KIT-A7 | [Maps, places and live location](#kit-a7-maps-places-and-live-location) | Maps, address search, routing, live location and service areas on open data, so no app depends on a paid map provider, with exact places kept private until a booking needs them. Built as svc-maps. | 13 | 9 |
| KIT-A8 | [Messaging and notifications](#kit-a8-messaging-and-notifications) | Chat between the two sides of a job, calls that hide phone numbers, video calls, push, email, text and letters by post, with each member choosing what reaches them. Built in svc-messaging; svc-calls is a proposed name (KIT-A16-F10-P1). | 18 | 15 |
| KIT-A9 | [Ratings and reputation](#kit-a9-ratings-and-reputation) | Two-way ratings and reviews after real jobs, made by people and never agents, fair rules on what counts, and a reputation members can see, challenge and take with them. Built as svc-trust. | 9 | 9 |
| KIT-A10 | [Safety, reports and moderation](#kit-a10-safety-reports-and-moderation) | Keeps people safe during a job and online: emergency help that works anywhere, official alerts and a disaster mode, helplines, a safe path for sexual harm reports, reports, blocking, published rules, fair treatment whoever you are, member moderators, legal notices, known-illegal-content checks and a public log of every removal. Built as svc-trust. | 23 | 21 |
| KIT-A11 | [Disputes and appeals](#kit-a11-disputes-and-appeals) | When a job goes wrong the money stays held, both sides are heard, a panel of trained members decides, and either side can appeal. Built as svc-trust. | 10 | 9 |
| KIT-A12 | [Member voting and meetings](#kit-a12-member-voting-and-meetings) | Members propose, discuss and vote on fees and rules for each app and for the whole co-op, elect directors, panels and moderators, follow decisions through, and hold meetings online, with a way to vote without a device. One member, one vote. Built as svc-voting. | 11 | 7 |
| KIT-A13 | [Open cost ledger](#kit-a13-open-cost-ledger) | Every month each app publishes what it cost to run, what the fee paid for and how the next fee is set, checked and signed off by members. It also keeps the co-op's own books and runs one pipeline for every public report. Built as svc-ledger. | 12 | 8 |
| KIT-A14 | [Co-op membership and member portal](#kit-a14-co-op-membership-and-member-portal) | The co-op's own home: joining once per person, the share register and the co-operative test, member roles, rebates of any surplus, reports, the constitution, discipline, board work, insurance, contributors' rights, the steward society and the federation of local co-ops. Proposed as svc-members (KIT-A16-F10-P1); the member register feeds svc-voting. | 19 | 11 |
| KIT-A15 | [Sync, history and storage](#kit-a15-sync-history-and-storage) | The base every app is built on (kit-core, kit-ops, kit-sync, kit-store): permanent IDs, every change as an operation with versioned schemas, personal data that can still be erased, undo, history, offline work, live co-editing, timers that survive restarts, files, previews, shared libraries, a document vault and sharing. Files are served by a file service proposed as svc-files (KIT-A16-F10-P1). | 18 | 15 |
| KIT-A16 | [Plugins, open API and integrations](#kit-a16-plugins-open-api-and-integrations) | Everything a person can do, a script or add-in can do: one command API per app, sandboxed plugins (kit-plugin), webhooks, SDKs, a plugin directory and links to outside tools, with limits that stop bulk copying, and named kit parts every program builds on. | 13 | 8 |
| KIT-A17 | [AI access](#kit-a17-ai-access) | Lets members use AI agents with every app through the same commands as people (kit-mcp), with permissions they choose, approval for money, guards against hidden instructions, agent actions labelled to the other side, people-only votes and reviews, and a record of everything an agent did. | 13 | 9 |
| KIT-A18 | [Privacy, data rights and legal duties](#kit-a18-privacy-data-rights-and-legal-duties) | Members see, take and delete their data, consent is recorded, data is kept only as long as needed, a named privacy officer answers, overseas handling is checked, Māori data is cared for, and the co-op meets privacy, consumer and platform law in NZ first and each new country after. Built in svc-accounts; svc-privacy is a proposed name (KIT-A16-F10-P1). | 18 | 17 |
| KIT-A19 | [Languages, accessibility and countries](#kit-a19-languages-accessibility-and-countries) | Every app works in te reo Māori and English from the start, meets accessibility standards, gets NZ's holidays, time zones and emergency numbers right, and takes on new languages and countries without rewriting. Proposed as kit-i18n and svc-translate (KIT-A16-F10-P1). | 12 | 11 |
| KIT-A20 | [App shell and phone apps](#kit-a20-app-shell-and-phone-apps) | The shared frame every app runs in: components from the design guide, installable phone and desktop apps, push, deep links, settings and moving between Spareware apps. Proposed as kit-ui (KIT-A16-F10-P1). | 11 | 11 |
| KIT-A21 | [Help and member support](#kit-a21-help-and-member-support) | Help pages in every language, support tied to the job, urgent help at any hour, members helping members, and a way to complain about the co-op itself, with response times published. Proposed as svc-support (KIT-A16-F10-P1). | 10 | 7 |
| KIT-A22 | [Running it: hosting, security and admin](#kit-a22-running-it-hosting-security-and-admin) | Servers run at cost by the co-op or by member groups, with monitoring, backups, encryption, signed releases, security reviews and an admin console where every action is logged. Proposed as svc-admin (KIT-A16-F10-P1). | 10 | 8 |
| KIT-A23 | [Work law and worker status](#kit-a23-work-law-and-worker-status) | One worker-status framework for every app: employee or contractor under the law's tests, agreements that meet them, personal grievances, minimum pay, holidays and leave, payroll, KiwiSaver and ACC guidance, switched on by each app's settings. Proposed as svc-work (KIT-A16-F10-P1). | 3 | 2 |
| KIT-A24 | [Health and safety at work](#kit-a24-health-and-safety-at-work) | The co-op's health and safety duties, built once: an incident register, notifiable event reports, work-time limits, help after violence or a crash, and hazard notes. Proposed as svc-work (KIT-A16-F10-P1). | 5 | 5 |
| KIT-A25 | [Signatures, forms, notices and training](#kit-a25-signatures-forms-notices-and-training) | Tools many apps need that no single app should own: electronic signatures with an audit trail, one form model, formal notices served the way the law asks, and one module for courses, quizzes and certificates. Proposed as svc-sign and svc-training (KIT-A16-F10-P1). | 4 | 4 |
| KIT-A26 | [Property records, ownership and tenancy rules](#kit-a26-property-records-ownership-and-tenancy-rules) | One shared record for each property with its chattels and repairs, proof of who owns or acts for it, decisions between co-owners, hand-over to a new owner, viewings, simple floor plans, owner statements and one pack of tenancy law, used by Spare Home, Spare Rent and Spare Stay so none of them depends on another. Built in kit-market as shared data; the document vault is KIT-A15-F13. | 10 | 9 |
| KIT-A27 | [Shared engines](#kit-a27-shared-engines) | The heavy engines several programs share: GPU rendering, 2D drawing, text, PDF, and video and audio, plus two proposed engines for office files and AI models run on the device. They are built in projects/engines, not in the kit, and depend only on the kit; they are mapped here so every program has one place to depend on and none depends on another. Names other than the five existing engines are proposals (KIT-A16-F10-P1). | 12 | 8 |
| KIT-A28 | [Own sites, public pages and feeds](#kit-a28-own-sites-public-pages-and-feeds) | Helps members reach people outside the apps: their own web address, fast public pages that web search can find, listing feeds to other sites, simple shop and booking sites, and a customer list with newsletters, all on the same checkout and seller of record rule. Built on kit-market and the file service; the service name is open (KIT-A16-F10-P1). | 6 | 2 |

## KIT-A1 Accounts and sign-in

One Spareware login for every app: signing up, signing in safely, getting back into a lost account, devices, roles, teams, and the sign-in service other apps and AI agents can use. Built as svc-accounts.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A1-F1** Sign up once for every app | Create one Spareware account with an email or phone number and use it in every app. | core | P1, P2, P3, P4, P5 |
| **KIT-A1-F2** Sign in with a passkey | Sign in with a passkey on a phone or computer, with no password to remember. | core | P1, P2, P3, P4, P5 |
| **KIT-A1-F3** Keep my account secure | Turn on a second sign-in step, see every signed-in device and sign any of them out. | core | P1, P2, P3, P4 |
| **KIT-A1-F4** Get back into a lost account | Recover an account after losing a phone or passkey, mostly without needing staff. | core | P1, P2 |
| **KIT-A1-F5** Have one profile and a role in each app | Keep one name and photo while being a rider in one app and a driver in another. | core | P1, P2, P3, P4, P5 |
| **KIT-A1-F6** Sign in to other apps with Spareware | Use a Spareware login in add-ins, member-run apps and AI agents, choosing what each may see. | core | P1, P2 |
| **KIT-A1-F7** Run a business or team account | Set up a restaurant, trade firm or office team with staff who act for it. | core | P1, P2, P3, P4, P5, P6, P7, P8 |
| **KIT-A1-F8** Let someone act for me | Let a carer, parent or assistant act for me within limits we can both see. | core | P1, P2 |
| **KIT-A1-F9** Carry one identity across open networks | Use my Spareware identity as a handle on open social networks and move it to another host. | core | P1 |
| **KIT-A1-F10** Sign in with another provider | Link a sign-in from another provider, where members have voted to allow it. | later | P1 |
| **KIT-A1-F11** Change my email or phone | Change the email or phone on my account, with the new one checked and the old one told. | core | P1 |
| **KIT-A1-F12** Sign in on a TV or command line | Sign in on a device with no keyboard or browser by entering a short code on my phone. | next | P1 |
| **KIT-A1-F13** Merge two accounts | Join two accounts I made by mistake, such as one by email and one by phone, into one. | next | P1 |
| **KIT-A1-F14** Switch between my accounts | Keep a personal and a work account signed in side by side, or several people's logins on a shared device, and switch in one step. | core | P1 |
| **KIT-A1-F15** Try an app before signing up | Use a design app on my own computer with no account, and keep my settings and recent work if I sign up later. | core | P1 |
| **KIT-A1-F16** Link family accounts | Link a parent's and a young person's accounts, with one payer and limits we can both see. | next | P1, P2 |
| **KIT-A1-F17** Hand an organisation or project to a new owner | Hand an organisation, or a project with its history, to another member, who accepts before it changes. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A1-F1-P1 | **Account service**: Stores accounts, verified emails and phones, and the apps each person uses, with every change an operation. | service | medium | kit-core, kit-ops |  |
| KIT-A1-F1-P2 | **Sign-up flow**: A short sign-up in every app that asks only what that app needs and records which terms were accepted. | interface | small | KIT-A1-F1-P1, KIT-A20-F1-P1, KIT-A18-F4-P1, KIT-A1-F1-P5 |  |
| KIT-A1-F1-P3 | **Email and phone checks**: Confirms an email or phone number with a one-time code or link. | service | small | KIT-A1-F1-P1, KIT-A8-F4-P1, KIT-A8-F4-P2 |  |
| KIT-A1-F1-P4 | **Sign-up abuse protection**: Rules for slowing mass sign-ups, set on the shared limiter, with privacy-friendly checks and never an outside tracker. | service | small | KIT-A1-F1-P1, KIT-A22-F5-P4 |  |
| KIT-A1-F1-P5 | **Minimum age at sign-up**: The youngest age each app accepts, and how sign-up checks it without collecting more than it needs. | legal | small |  |  |
| KIT-A1-F2-P1 | **Passkey sign-in**: Passkey registration and sign-in on the WebAuthn standard, using an open library whose licence suits the core. | library | medium | KIT-A1-F1-P1 |  |
| KIT-A1-F2-P2 | **Password fallback**: An optional password, stored with a slow memory-hard hash and checked against known breached passwords. | service | small | KIT-A1-F1-P1 |  |
| KIT-A1-F2-P3 | **One-time code sign-in**: Signs in with a code sent by email or text where no passkey is set up. | service | small | KIT-A1-F1-P1, KIT-A1-F1-P3 |  |
| KIT-A1-F2-P4 | **Sign-in screens**: Shared sign-in, account chooser and consent screens used by every app. | interface | small | KIT-A20-F1-P1, KIT-A1-F2-P1 |  |
| KIT-A1-F2-P5 | **Sign-in attack protection**: Slows repeated failed sign-ins, checks for credential stuffing across accounts, and locks out attackers without locking out the owner. | service | small | KIT-A1-F1-P1, KIT-A22-F5-P4, KIT-A8-F4-P4 |  |
| KIT-A1-F3-P1 | **Second sign-in step**: Authenticator codes or a second passkey, required for workers, moderators and admins. | service | small | KIT-A1-F1-P1 |  |
| KIT-A1-F3-P2 | **Sessions and devices**: Session tokens with expiry and refresh, a device list and remote sign-out. | service | medium | KIT-A1-F1-P1 |  |
| KIT-A1-F3-P3 | **New sign-in alerts**: Tells the person when a new device or place signs in to their account. | service | small | KIT-A1-F3-P2, KIT-A8-F3-P2 |  |
| KIT-A1-F3-P4 | **Fresh sign-in for risky commands**: Asks for a fresh sign-in with the second step before risky commands, such as changing payout or contact details, declared once in each command's schema and applied to people and agents alike. | service | small | KIT-A1-F3-P1, KIT-A1-F3-P2, KIT-A15-F1-P3 |  |
| KIT-A1-F4-P1 | **Recovery codes and backup methods**: Recovery codes, a second email or phone, and a second passkey. | service | small | KIT-A1-F1-P1 |  |
| KIT-A1-F4-P2 | **Assisted recovery**: A support-run recovery path with an identity re-check and a waiting period, every step logged. | service | medium | KIT-A1-F1-P1, KIT-A2-F1-P2, KIT-A21-F2-P2 |  |
| KIT-A1-F5-P1 | **Profile model**: Display name, photo, languages, pronouns and what each app may show. | data | small | kit-core |  |
| KIT-A1-F5-P2 | **Roles per app**: Grants roles such as customer, worker, host or moderator per app, each with its own checks. | service | small | KIT-A1-F1-P1, kit-ops |  |
| KIT-A1-F5-P3 | **Profile and roles page**: Edit the profile and see which roles are active in which app. | interface | small | KIT-A20-F1-P1, KIT-A1-F5-P1, KIT-A1-F5-P2 |  |
| KIT-A1-F5-P4 | **Access needs**: Access needs saved on the profile with consent, such as a wheelchair, an assistance dog or 'message, don't call', and carried on every request and order. | data | small | KIT-A1-F5-P1, KIT-A18-F17-P1 |  |
| KIT-A1-F5-P5 | **Profile photo checks**: Checks that a profile photo shows a face and nothing offensive before others see it, with any face check run under the Biometric Processing Privacy Code and never used to identify anyone. Spare Ride and Spare Stay need the same check. | service | small | KIT-A1-F5-P1, KIT-A2-F1-P4, engine-media |  |
| KIT-A1-F6-P1 | **Sign-in provider**: An OpenID Connect and OAuth 2 provider with consent screens, scopes and token revocation. | service | large | KIT-A1-F1-P1, KIT-A1-F3-P2 |  |
| KIT-A1-F6-P2 | **Connected apps page**: Lists every app and agent with access and removes any of them in one step. | interface | small | KIT-A20-F1-P1, KIT-A1-F6-P1 |  |
| KIT-A1-F7-P1 | **Organisation model**: Organisations with members, roles, invites and owners, shared by every app. | data | medium | kit-core, KIT-A1-F1-P1 |  |
| KIT-A1-F7-P2 | **Team management**: Invite staff, set what each can do and remove them. | interface | medium | KIT-A20-F1-P1, KIT-A1-F7-P1 |  |
| KIT-A1-F7-P3 | **Directory sync for offices and schools**: Brings people in from an organisation's own directory using open standards such as SCIM and LDAP. | integration | medium | KIT-A1-F7-P1 | [OS06](../spare-os/STATUS.md) |
| KIT-A1-F7-P4 | **Organisation sign-in**: Lets staff sign in through their organisation's own identity provider using SAML or OpenID Connect, beside directory sync. | integration | medium | KIT-A1-F7-P1, KIT-A1-F6-P1 |  |
| KIT-A1-F7-P5 | **Organisation policies**: Settings an organisation's admin can lock, such as AI tools off, no sharing outside the organisation and the region its data is kept in. | library | small | KIT-A1-F7-P1, KIT-A15-F8-P1 |  |
| KIT-A1-F7-P6 | **Shared devices and staff PINs**: Quick staff switching with a PIN on a shared tablet or till, with each person's actions recorded under their own name and limited by their role. | service | small | KIT-A1-F7-P1, KIT-A1-F3-P2 |  |
| KIT-A1-F7-P7 | **Organisation audit log**: Lets an organisation's admins see who did what across every app the organisation uses, built from the operation log, searchable and exportable, with its own retention. Spare Office, Spare Studio and Spare Construct need the same log. | service | medium | KIT-A1-F7-P1, KIT-A15-F1-P2, KIT-A18-F6-P1 |  |
| KIT-A1-F7-P8 | **Organisation branding**: An organisation's logo, colours and fonts, set once and shown on its sign-in page, mail, forms and shared work, and locked as a brand kit where the admin chooses. Spare Office and Spare Studio need the same settings. | data | small | KIT-A1-F7-P1, KIT-A1-F7-P5 |  |
| KIT-A1-F8-P1 | **Delegated access**: Time-limited, scoped permission for one account to act for another, shown in both accounts and logged. | service | medium | KIT-A1-F1-P1, KIT-A1-F5-P2, kit-ops, KIT-A1-F8-P2 |  |
| KIT-A1-F8-P2 | **Rules for parents and carers**: What a parent or carer may see and do for a young person or someone they support, building on each app's minimum age. | legal | small | KIT-A1-F1-P5 |  |
| KIT-A1-F9-P1 | **Open network identity**: Links a Spareware account to a portable handle on the open protocols Spare Social chooses. | integration | large | KIT-A1-F6-P1 | [M01](../spare-social/STATUS.md) |
| KIT-A1-F10-P1 | **Outside sign-in links**: Optional sign-in through other providers, off by default and subject to a member vote. | integration | small | KIT-A1-F6-P1, KIT-A12-F4-P1 |  |
| KIT-A1-F11-P1 | **Contact change**: Asks for a fresh sign-in, checks the new email or phone, tells the old one, and gives a short window to undo a change the owner did not make. | service | small | KIT-A1-F1-P3, KIT-A1-F3-P2, KIT-A8-F3-P2, KIT-A1-F3-P4 |  |
| KIT-A1-F12-P1 | **Device sign-in**: Signs in TVs, command-line tools and other devices without a browser using a short code entered on another device, under OAuth device authorisation (RFC 8628). | service | small | KIT-A1-F6-P1 |  |
| KIT-A1-F13-P1 | **Account merge**: Merges two accounts that belong to one person after a fresh sign-in to both, keeping orders, ratings, roles and checks, and leaving one membership and one vote. | service | medium | KIT-A1-F1-P1, KIT-A1-F3-P4, KIT-A14-F1-P4 |  |
| KIT-A1-F14-P1 | **Account switcher**: Keeps several accounts signed in on one device, each with its own sessions, and on a shared device locks each one behind its owner's own sign-in. Spare Office and Spare Social need the same switcher. | service | small | KIT-A1-F3-P2, KIT-A1-F2-P4 |  |
| KIT-A1-F15-P1 | **Local profile without an account**: Keeps preferences, recent files and libraries on the device with no account at all, and moves them into the account if the person later signs up. Spare Construct and Spare Studio both work this way. | library | small | kit-core, KIT-A20-F5-P2 |  |
| KIT-A1-F16-P1 | **Family accounts**: Links family accounts only when both agree, with a payer, settings for each age set by the rules, and a link the young person can always see. Spare Social, Spare Ride and Spare Food map the same need. | service | medium | KIT-A1-F8-P1, KIT-A1-F8-P2, KIT-A1-F1-P5 |  |
| KIT-A1-F16-P2 | **Purchase approvals for children**: Holds a child's purchase, tip or booking until a parent approves it, with spending limits for each child, as app store laws ask. Spare Social and Spare OS need the same approvals. | service | small | KIT-A1-F16-P1, KIT-A3-F1-P2 |  |
| KIT-A1-F17-P1 | **Ownership transfer**: Hands an organisation, or a project with its history, to another member, who accepts before anything changes, with both told and every step logged. Spare Office and Spare Construct need the same hand-over. | service | small | KIT-A1-F7-P1, KIT-A15-F1-P2, KIT-A8-F3-P2 |  |

## KIT-A2 Identity, licence and business checks

Proves people are who they say and hold the licences their work needs, through accredited providers and public registers. The co-op keeps the result; the provider keeps the documents under contract, and the co-op can get them back when the law asks. Built in svc-accounts; svc-identity is a proposed name (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A2-F1** Prove who I am | Confirm my identity once with a photo ID and a selfie, and reuse that result in every app that needs it. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A2-F2** Show my work licence | Link a driver licence, trade registration or other licence so an app lets me take that work. | core | P1, P2, P3 |
| **KIT-A2-F3** Keep my checks current | Get reminded before a licence or check expires and renew it without starting over. | core | P1 |
| **KIT-A2-F4** Ask for a failed check to be reviewed | Ask a person to review a check that failed or looks wrong. | core | P1 |
| **KIT-A2-F5** See what was verified about someone | See what was checked about a worker or host, and when, before working with them. | core | P1 |
| **KIT-A2-F6** Add a vehicle | Add a vehicle and have its registration and safety certificate checked. | core | P1, P2 |
| **KIT-A2-F7** Prove my business is real | Verify a business so it can sell, hire or host in any app. | core | P1, P2, P3 |
| **KIT-A2-F8** Prove my age without sharing more | Show an app I am old enough without handing over my birth date or a document. | core | P1 |
| **KIT-A2-F9** Pass a safety check where the law needs one | Consent to a background check where an app's law requires it, and see the outcome. | core | P1, P2 |
| **KIT-A2-F10** Use my checks in another country | Have identity and licence checks work with each country's own providers and registers. | later | P1 |
| **KIT-A2-F11** Show I may work here | Show that my visa lets me do this kind of work, as an employee or a contractor, before I take jobs. | core | P1, P2 |
| **KIT-A2-F12** Prove it is still me | Pass a quick face re-check now and then, so no one else can work under my account. | core | P1, P2 |
| **KIT-A2-F13** Have a document checked by a person | Upload a paper that has no public register, such as an insurance certificate or a warrant of fitness, and have a trained person check it. | core | P1 |
| **KIT-A2-F14** Prove my address | Prove I live at or control an address, sharing only the result. | core | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A2-F1-P1 | **Identity provider choice**: Compares providers accredited under NZ's Digital Identity Services Trust Framework on cost, method and what data they keep. | research | small |  |  |
| KIT-A2-F1-P2 | **Identity check link**: Sends a person to an accredited identity provider and receives only the result. | integration | medium | KIT-A1-F1-P1, KIT-A2-F1-P1, KIT-A2-F1-P4 |  |
| KIT-A2-F1-P3 | **Check results store**: Records what was checked, by whom, when and until when; the documents stay with the provider, and the co-op can retrieve them when the law asks. | data | small | kit-core, kit-ops, KIT-A2-F1-P1 |  |
| KIT-A2-F1-P4 | **Biometric code compliance**: Meets the Biometric Processing Privacy Code 2025 for selfie and face-match checks: a proportionality assessment, clear notices, a non-biometric option and limits on keeping face data. | legal | medium |  |  |
| KIT-A2-F1-P5 | **Verification records on demand**: A contract under which the identity provider keeps verification records for the anti-money-laundering period, a way to retrieve them on request, and a legal hold that account deletion skips. | integration | medium | KIT-A2-F1-P2, KIT-A4-F5-P1, KIT-A18-F6-P1 |  |
| KIT-A2-F1-P6 | **Identity checks for people overseas**: Lets people who live outside NZ, such as overseas sellers and visiting guests, prove who they are in the first release, through a provider that covers their country or their NZ lawyer's anti-money-laundering identity check, recorded as the same result as a local check. Spare Home and Spare Stay both need it. | integration | medium | KIT-A2-F1-P1, KIT-A2-F1-P3 |  |
| KIT-A2-F2-P1 | **Public register checks**: Looks up licences in public registers, such as trade boards and the business number register, on their published terms. | integration | medium | KIT-A2-F1-P3 | [WK02](../spare-work/STATUS.md) |
| KIT-A2-F2-P2 | **Check rules per app**: Each app declares which checks a role needs and how often they repeat. | data | medium | KIT-A1-F5-P2, KIT-A2-F1-P3 |  |
| KIT-A2-F2-P3 | **My checks page**: Shows each check, its status and expiry, and what to do next. | interface | small | KIT-A20-F1-P1, KIT-A2-F1-P3 |  |
| KIT-A2-F3-P1 | **Expiry and re-check scheduler**: Schedules re-checks and reminders, and pauses a role when a check lapses. | service | small | KIT-A2-F2-P2, KIT-A8-F3-P2, KIT-A15-F14-P1 |  |
| KIT-A2-F4-P1 | **Check review request**: Lets a person explain and add evidence, and sends the request to the one review queue for a person to decide. | interface | small | KIT-A2-F1-P3, KIT-A21-F2-P2, KIT-A10-F6-P2 |  |
| KIT-A2-F5-P1 | **Verified badge**: Shows which checks passed and their dates, without the details behind them. | interface | small | KIT-A2-F1-P3, KIT-A20-F1-P1 |  |
| KIT-A2-F6-P1 | **Vehicle data access**: Which official vehicle data an app may look up, on what terms and at what cost. | research | small |  |  |
| KIT-A2-F6-P2 | **Vehicle checks**: Checks registration and safety certificate status from sources that allow it. | integration | medium | KIT-A2-F1-P3, KIT-A2-F6-P1 |  |
| KIT-A2-F7-P1 | **Business number lookup**: Confirms a business through the NZ Business Number register and stores the result. | integration | small | KIT-A2-F1-P3, KIT-A1-F7-P1 |  |
| KIT-A2-F7-P2 | **Trader details**: Keeps the trader details marketplace law asks for and shows buyers what they are owed. | data | small | KIT-A1-F7-P1 |  |
| KIT-A2-F7-P3 | **Charities Register check**: Confirms a registered charity through the NZ Charities Register, on its published terms, and stores the result beside the business number check. | integration | small | KIT-A2-F7-P1 |  |
| KIT-A2-F8-P1 | **Age checks**: Age assurance through an accredited provider, returning only whether a person is over the age set. | integration | medium | KIT-A2-F1-P2 | [M07](../spare-social/STATUS.md) |
| KIT-A2-F9-P1 | **Background checks in NZ**: Which checks each app's law requires, who may run them and how to do it lawfully. | research | medium |  |  |
| KIT-A2-F9-P2 | **Background check link**: Requests a consented check from an approved provider and records only the outcome. | integration | medium | KIT-A2-F1-P3, KIT-A2-F9-P1, KIT-A18-F17-P1 |  |
| KIT-A2-F10-P1 | **Check providers per country**: Identity, licence and register sources for each country added, in its country pack. | data | medium | KIT-A2-F2-P2, KIT-A19-F6-P1 |  |
| KIT-A2-F11-P1 | **Right-to-work rules**: Which visas allow employment, which allow contracting, and what each app must check for its workers' status. | research | small |  |  |
| KIT-A2-F11-P2 | **Visa entitlement check**: Checks a worker's entitlement through an official visa check or the identity provider, records the result and its expiry, and re-checks before it lapses. | integration | medium | KIT-A2-F11-P1, KIT-A2-F1-P3, KIT-A2-F3-P1 |  |
| KIT-A2-F12-P1 | **Face check fairness testing**: Tests re-check providers for error rates across skin tones, ages and genders, and publishes the results before any is used. | research | medium | KIT-A2-F1-P4 |  |
| KIT-A2-F12-P2 | **Repeat face checks**: Random re-checks at the start of work to stop account renting, with a non-biometric option and a person reviewing every failure before any pause. | integration | medium | KIT-A2-F12-P1, KIT-A2-F1-P2, KIT-A2-F1-P4, KIT-A10-F6-P2 |  |
| KIT-A2-F13-P1 | **Document checks by a person**: Checks papers with no public register, such as insurance certificates, warrants and certificates of fitness and food registrations, by a trained person on the one review queue, recording each result with an expiry for the re-check scheduler. | service | medium | KIT-A2-F1-P3, KIT-A2-F2-P2, KIT-A2-F3-P1, KIT-A10-F6-P2, KIT-A15-F13-P1 |  |
| KIT-A2-F14-P1 | **Address check by posted code**: Confirms an address by a code sent by post, or by a trusted record such as a rates notice, and stores only the result and its date. Spare Market and Spare Home need it, and hosts and landlords can use the same check. | service | small | KIT-A2-F1-P3, KIT-A7-F2-P3, KIT-A8-F17-P1 |  |

## KIT-A3 Payments and payouts

Customers pay, a regulated provider holds the money within its time limits, workers are paid everything except the at-cost fee, and every dollar is shown, including who carries a loss, payments taken in person and payments made outside the app. The co-op never holds money or card numbers itself. Built as svc-payments.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A3-F1** Pay for a job | Pay by card or phone wallet in any app, seeing the full price before paying. | core | P1, P2, P3, P4, P5, P6, P7 |
| **KIT-A3-F2** Know my money is held until the job is done | Have my payment held by the provider, within its time limits, and released only when the job is confirmed. | core | P1, P2, P3, P4 |
| **KIT-A3-F3** Get paid out | Set up payouts once and receive everything I earned, less only the at-cost fee. | core | P1, P2, P3, P4, P5, P6, P7 |
| **KIT-A3-F4** See the at-cost fee | See exactly what the fee is, what it pays for and how it was worked out. | core | P1, P2, P3 |
| **KIT-A3-F5** Get a refund | Get a full or part refund when a job is cancelled or a dispute says so. | core | P1, P2 |
| **KIT-A3-F6** Tip the worker | Add a tip that goes to the worker in full. | core | P1 |
| **KIT-A3-F7** Keep a receipt | Get a clear receipt showing the fee and GST for every payment. | core | P1 |
| **KIT-A3-F8** Save a card for next time | Save a card with the provider so paying next time is one tap. | core | P1 |
| **KIT-A3-F9** Trust that every dollar adds up | Know that what the provider moved matches what every order says. | core | P1 |
| **KIT-A3-F10** Pay a deposit or damage hold | Pay a refundable deposit or damage hold for a stay or a hire. | next | P1 |
| **KIT-A3-F11** Pay straight from my bank | Pay from a bank account where the provider supports it. | core | P1 |
| **KIT-A3-F12** Pay a regular amount | Pay monthly for shop hosting, storage or supporting a creator. | core | P1, P2 |
| **KIT-A3-F13** Be protected from payment fraud | Have stolen cards and fake accounts stopped before money moves. | core | P1, P2 |
| **KIT-A3-F14** Pay and be paid in my own currency | Pay and be paid in the money of each country Spareware runs in. | later | P1 |
| **KIT-A3-F15** Give a gift card | Give someone a gift card for any Spareware app. | later | P1, P2 |
| **KIT-A3-F16** Know who carries a loss | Know what happens when a refund or a lost card dispute lands after the worker was already paid. | core | P1, P2 |
| **KIT-A3-F17** Pay on account as a business | Let a business pay on account and get one GST invoice a month. | core | P1, P2 |
| **KIT-A3-F18** Record a payment made outside the app | Record a job paid in cash or by bank transfer, so the order, ratings and records still work. | core | P1, P2, P3 |
| **KIT-A3-F19** Take a card in person | Take a card payment at the end of a job on my own phone, a card reader or a shop till. | core | P1, P2 |
| **KIT-A3-F20** Keep taking payments if a provider drops us | Keep paying and being paid if the payment provider closes the co-op's account. | next | P1 |
| **KIT-A3-F21** Split a payment with others | Split one payment between several people, each paying their share by a deadline. | later | P1 |
| **KIT-A3-F22** Know what happens when a payment fails | Be told when a payment fails, fix it simply, and never be charged extra for the failure. | core | P1 |
| **KIT-A3-F23** Raise money for a cause | Run a fundraiser or a fundraising sale for a school, club or charity. | later | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A3-F1-P1 | **Payment provider choice**: Compares providers that pay out to workers in NZ, and lists what Spareware must never handle itself. | research | medium |  | [K02](STATUS.md) |
| KIT-A3-F1-P2 | **Payment provider link**: One adapter to the chosen regulated provider for charges, holds, splits, refunds and payouts. | integration | large | kit-core, kit-ops, KIT-A3-F1-P1, KIT-A3-F1-P5, KIT-A15-F1-P9 | [K02](STATUS.md) |
| KIT-A3-F1-P3 | **Checkout**: A shared checkout using the provider's own secure card fields, so card numbers never reach our servers. | interface | medium | KIT-A3-F1-P2, KIT-A20-F1-P1 |  |
| KIT-A3-F1-P4 | **Money type**: Exact amounts in each currency's ISO 4217 minor units, with currency, rounding rules and GST fields: the one money type every app and kit-core use. | library | small | kit-core, KIT-A15-F1-P1 |  |
| KIT-A3-F1-P5 | **Seller of record per app**: Decides for each app whether the worker or the co-op is the seller, which sets who charges GST, who owes consumer guarantees, whose name is on the receipt and who answers card disputes. | legal | medium | KIT-A3-F1-P1 | [K02](STATUS.md) |
| KIT-A3-F1-P6 | **Yearly card security check**: A yearly PCI DSS self-assessment, SAQ A while card fields stay with the provider, as the card schemes ask of every merchant and platform. | legal | small | KIT-A3-F1-P3 |  |
| KIT-A3-F1-P7 | **Checkout without an account**: Whether people may order with no account, weighing card fraud, anti-money-laundering duties and data rights, decided once. Spare Food and Spare Market both wait on it. | research | small | KIT-A3-F13-P1, KIT-A4-F5-P1, KIT-A18-F5-P1 |  |
| KIT-A3-F2-P1 | **Hold and release rules**: Holds a payment against an order and releases, refunds or splits it when the order says so. | service | medium | KIT-A3-F1-P2, kit-market, KIT-A3-F2-P2, KIT-A3-F2-P4 | [MK01](../spare-market/STATUS.md) |
| KIT-A3-F2-P2 | **Holding money lawfully**: A lawyer confirms how held money must be run so the co-op never holds it itself. | legal | medium | KIT-A3-F1-P1 | [K02](STATUS.md) |
| KIT-A3-F2-P3 | **Stage payments**: Holds and releases money in stages for longer jobs. | service | medium | KIT-A3-F2-P1 | [WK01](../spare-work/STATUS.md) |
| KIT-A3-F2-P4 | **Hold time limits**: Each provider's limits on holding money, with rules for when to charge bookings made far ahead, deposit then balance, and re-authorising card holds that expire. | library | medium | KIT-A3-F1-P1, KIT-A3-F2-P2, KIT-A15-F14-P1 |  |
| KIT-A3-F3-P1 | **Payout set-up**: Sends workers to the provider's own identity and bank account set-up, passing on the kit's identity check result where the provider accepts it, so a worker proves who they are once. | integration | medium | KIT-A3-F1-P2, KIT-A1-F1-P1, KIT-A2-F1-P3 |  |
| KIT-A3-F3-P2 | **Payout runs**: Pays out daily in batches, with faster payouts at the provider's cost where offered. | service | medium | KIT-A3-F1-P2, KIT-A15-F14-P1 |  |
| KIT-A3-F3-P3 | **Earnings page**: Shows each job, the fee, GST and every payout, and exports them. | interface | medium | KIT-A20-F1-P1, KIT-A3-F3-P2, KIT-A3-F4-P1 |  |
| KIT-A3-F3-P4 | **Payout change guard**: When a bank account, email or phone changes, asks for a fresh sign-in, holds payouts for a short time and alerts both the old and new contacts. | service | small | KIT-A3-F3-P1, KIT-A1-F3-P2, KIT-A1-F11-P1, KIT-A8-F3-P2, KIT-A1-F3-P4 |  |
| KIT-A3-F3-P5 | **Failed payouts**: Handles payouts a bank returns, such as for a closed account or wrong details, keeping the money with the provider, telling the worker and paying again once the details are fixed. | service | small | KIT-A3-F3-P2, KIT-A3-F9-P1, KIT-A8-F3-P2 |  |
| KIT-A3-F3-P6 | **Money that cannot be paid out**: What happens to money that can never be paid out, and how the Unclaimed Money Act 1971 applies to it (*unverified* period for platform balances). | legal | small | KIT-A3-F2-P2 |  |
| KIT-A3-F3-P7 | **Bank account name check**: Checks the account name matches when payout details change, through NZ banks' Confirmation of Payee, run since late 2024 (*unverified* whether a business can call it), or the provider's own check. | integration | small | KIT-A3-F3-P4 |  |
| KIT-A3-F4-P1 | **Fee calculator**: Works out each job's fee from the period's published rate and passes provider charges through at cost. | library | medium | KIT-A3-F1-P4, KIT-A13-F3-P1, KIT-A3-F4-P3 |  |
| KIT-A3-F4-P2 | **Fee breakdown**: Shows this job's fee and where every dollar goes on the price, the receipt and the earnings page, linking to how the period's fee was set. | interface | small | KIT-A3-F4-P1, KIT-A20-F1-P1 |  |
| KIT-A3-F4-P3 | **Card fee pass-through check**: Checks that passing card costs through at cost is not a card surcharge where the law bans one, such as in-store payments if NZ's planned ban applies (*unverified*). | legal | small |  |  |
| KIT-A3-F5-P1 | **Refunds**: Full and part refunds through the provider, following the order's cancellation policy. | service | medium | KIT-A3-F2-P1 |  |
| KIT-A3-F5-P2 | **Card disputes**: Answers card disputes raised with banks, sending the order's evidence to the provider. | service | medium | KIT-A3-F1-P2, KIT-A11-F2-P1 |  |
| KIT-A3-F6-P1 | **Tips**: Adds a tip during or after the job and pays all of it to the worker; the card cost on a tip is carried as a running cost in the at-cost fee, never taken from the tip. | service | small | KIT-A3-F3-P2, KIT-A3-F4-P1 |  |
| KIT-A3-F7-P1 | **Receipts**: Issues receipts in the person's language, with a printable copy. | service | small | KIT-A3-F4-P1, KIT-A4-F2-P1, engine-pdf |  |
| KIT-A3-F8-P1 | **Saved payment methods**: Stores only the provider's token, never the card, and lets the person remove it. | integration | small | KIT-A3-F1-P2 |  |
| KIT-A3-F9-P1 | **Reconciliation**: Matches provider reports against orders and payouts daily and flags any gap. | service | medium | KIT-A3-F1-P2, KIT-A3-F3-P2 |  |
| KIT-A3-F10-P1 | **Deposits and damage holds**: Holds an amount and releases it by a set time unless a claim is made. | service | medium | KIT-A3-F2-P1 |  |
| KIT-A3-F11-P1 | **Bank payments**: Adds account-to-account payments through the provider as open banking reaches more banks. | integration | medium | KIT-A3-F1-P2 |  |
| KIT-A3-F12-P1 | **Regular payments**: Monthly charges at cost with clear notice, pausing and one-step cancelling. | service | medium | KIT-A3-F1-P2 |  |
| KIT-A3-F12-P2 | **Monthly caps and billing after use**: Lets a person set a monthly cap, warns before it is reached and charges for the month's use afterwards through the provider, so the co-op never holds a prepaid balance. | service | medium | KIT-A3-F12-P1, KIT-A13-F2-P3, KIT-A13-F3-P6 |  |
| KIT-A3-F13-P1 | **Provider fraud screening**: Uses the provider's own fraud screening and records its decision on the order. | integration | small | KIT-A3-F1-P2 |  |
| KIT-A3-F13-P2 | **Published risk rules**: Published rules for new accounts, large amounts and fast repeat payments, starting from the provider's screening; the shared fraud signals service builds on them. | service | medium | KIT-A3-F2-P1, KIT-A3-F13-P1 | [MK04](../spare-market/STATUS.md) |
| KIT-A3-F14-P1 | **Currencies per country**: Prices, payouts and fees in each country's currency, with exchange costs published. | service | medium | KIT-A3-F1-P4, KIT-A19-F6-P1 |  |
| KIT-A3-F15-P1 | **Gift card rules**: What consumer and anti-money-laundering law say about gift cards and stored credit. | legal | small |  |  |
| KIT-A3-F15-P2 | **Gift cards**: Gift cards held and settled by the provider, never by the co-op. | service | medium | KIT-A3-F1-P2, KIT-A3-F15-P1 |  |
| KIT-A3-F16-P1 | **Loss rules**: Who carries a late refund or a lost card dispute: recovery from future earnings, a protection pool or the co-op, and the platform's liability under the provider's terms. | legal | medium | KIT-A3-F1-P1, KIT-A3-F1-P5 | [K02](STATUS.md) |
| KIT-A3-F16-P2 | **Negative balances**: Tracks what a worker owes after a late refund or lost dispute and recovers it from future payouts within published limits, never by surprise. | service | medium | KIT-A3-F16-P1, KIT-A3-F3-P2, KIT-A3-F5-P1, KIT-A3-F5-P2 |  |
| KIT-A3-F17-P1 | **Business accounts on credit**: Lets an organisation book on account with spending limits for its staff, then charges it once a month with one tax invoice. | service | medium | KIT-A1-F7-P1, KIT-A3-F1-P2, KIT-A4-F2-P1 |  |
| KIT-A3-F17-P2 | **Company travel and spending rules**: Simple company rules, such as a nightly price limit, where and when staff may travel and an amount per trip, checked at booking. Spare Ride and Spare Stay need the same rules. | library | small | KIT-A3-F17-P1, KIT-A1-F7-P5 |  |
| KIT-A3-F18-P1 | **Outside payment records**: Records a cash or bank transfer payment that both sides confirm, so the order, ratings, stock and records work as if it was paid in the app. | service | small | kit-market, KIT-A5-F4-P1 |  |
| KIT-A3-F18-P2 | **Fee on outside payments**: Invoices the worker for the at-cost fee on a job paid outside the provider, under the fee rule each app's members chose. | service | small | KIT-A3-F18-P1, KIT-A3-F4-P1, KIT-A4-F2-P1 |  |
| KIT-A3-F18-P3 | **Cash checks**: Flags cash payments for the anti-money-laundering programme, such as large or repeated cash amounts, and counts them toward the due diligence threshold. | service | small | KIT-A3-F18-P1, KIT-A4-F5-P3, KIT-A4-F5-P5 |  |
| KIT-A3-F19-P1 | **Tap to pay on a phone**: Takes a card on the worker's own phone through the provider's tap to pay, which Stripe lists as available in NZ (*unverified* for Connect platforms), with the in-store surcharge check applied. | integration | medium | KIT-A3-F1-P2, KIT-A3-F4-P3, KIT-A20-F2-P2 |  |
| KIT-A3-F19-P2 | **Card readers and a shared till**: The provider's card readers and a shared till for shops and restaurants, with quick staff switching on the shared device. | integration | medium | KIT-A3-F19-P1, KIT-A1-F7-P6 | [MK07](../spare-market/STATUS.md) |
| KIT-A3-F20-P1 | **Second provider and exit plan**: A second regulated provider ready behind the same adapter, and a tested plan to move to it, including moving saved card tokens through the providers' own migration process. | research | medium | KIT-A3-F1-P1, KIT-A3-F1-P2 |  |
| KIT-A3-F21-P1 | **Split payments**: Splits one payment between several payers, each paying their share through the provider, with a deadline, reminders and a published rule for shares not paid in time. | service | medium | KIT-A3-F1-P2, KIT-A3-F2-P1, KIT-A15-F14-P1 |  |
| KIT-A3-F22-P1 | **Failed payment handling**: Retries a failed charge on a published schedule, tells the person, asks for another method and holds new bookings until any balance is paid, never adding a charge for the failure. Spare Ride, Spare Food, Spare Stay and Spare Rent need the same rules. | service | small | KIT-A3-F1-P2, KIT-A8-F3-P2, KIT-A15-F14-P1 |  |
| KIT-A3-F23-P1 | **Fundraising rules**: NZ rules for charitable fundraising, receipts and donation tax credits, and the rules overseas, for fundraisers in Spare Social and fundraising sales in Spare Market. | legal | small | KIT-A2-F7-P3 |  |

## KIT-A4 Tax, invoices and money law

The tax and money law every marketplace must meet, built once: GST on listed services, the co-op's own GST returns, tax invoices, reporting sellers' income to IRD, anti-money-laundering duties and record keeping. Built in svc-payments; svc-tax is a proposed name (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A4-F1** Have GST charged correctly | Have GST worked out and collected the way the law says for each kind of job. | core | P1, P2, P3 |
| **KIT-A4-F2** Get a proper tax invoice | Get a valid tax invoice, or have the platform issue one for the seller where the law allows. | core | P1, P2, P3 |
| **KIT-A4-F3** Know my income is reported correctly | Have the platform report sellers' income to IRD each year, and see what was sent. | core | P1, P2, P3 |
| **KIT-A4-F4** Get a year-end statement | Download a year's earnings, fees and GST to give to an accountant. | core | P1, P2, P3 |
| **KIT-A4-F5** Know the co-op meets anti-money-laundering law | Trust that the co-op follows anti-money-laundering law for online marketplaces. | core | P1, P2, P3, P4, P5, P6, P7 |
| **KIT-A4-F6** Know records are kept as long as the law says | Trust that every financial record is kept for the required time and can be produced. | core | P1 |
| **KIT-A4-F7** Have tax handled in other countries | Have each country's sales tax and platform reporting handled when an app goes overseas. | later | P1, P2, P3 |
| **KIT-A4-F8** Know payouts never reach sanctioned people | Trust that payouts are screened against sanctions lists. | core | P1 |
| **KIT-A4-F9** Know the co-op pays its own GST | Trust that the co-op files its own GST returns as marketplace operator and charges GST on its fee. | core | P1, P2, P3 |
| **KIT-A4-F10** Opt out of platform GST as a large supplier | As a large accommodation owner or large registered supplier, agree with the platform to account for GST myself where the law allows. | core | P1 |
| **KIT-A4-F11** Get GST figures for each return | As a GST-registered worker, get the platform's figures for each two-monthly or six-monthly return. | next | P1 |
| **KIT-A4-F12** Have tax held back from my payouts | Choose a rate of tax to be held back from each payout and paid to IRD for me. | next | P1, P2 |
| **KIT-A4-F13** Be warned before I must register for GST | Be told before my sales across every Spareware app reach the GST registration threshold. | core | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A4-F1-P1 | **GST rules per app**: Maps which apps fall under GST on listed services and how each kind of job is taxed. | research | medium |  |  |
| KIT-A4-F1-P2 | **GST engine**: Works out GST on every order, including the platform's own duty to collect it on listed services. | library | large | KIT-A3-F1-P4, KIT-A4-F1-P1 |  |
| KIT-A4-F1-P3 | **Flat-rate credit notices**: Tells unregistered sellers their flat-rate credits at least monthly, as the law asks. | service | small | KIT-A4-F1-P2, KIT-A8-F3-P2 |  |
| KIT-A4-F2-P1 | **Tax invoices**: Issues tax invoices and buyer-created tax invoices with every field the law asks for. | service | medium | KIT-A4-F1-P2, engine-pdf | [WK05](../spare-work/STATUS.md) |
| KIT-A4-F2-P2 | **Supply correction information**: Issues supply correction information, which replaced credit and debit notes from 1 April 2023, whenever a refund, an order change or an extra charge alters an amount already invoiced, with invoices numbered in order for each seller. | service | medium | KIT-A4-F2-P1, KIT-A3-F5-P1, KIT-A5-F15-P1, KIT-A5-F15-P2 | [WK05](../spare-work/STATUS.md) |
| KIT-A4-F2-P3 | **Invoice numbers and payment terms**: Numbers each seller's invoices in order, with changes issued as supply correction information, and adds due dates, reminders and part payments for invoices paid later. Spare Work numbers invoices this way, and Spare Market's wholesale invoices need the terms. | library | small | KIT-A4-F2-P1, KIT-A4-F2-P2, KIT-A8-F3-P2 |  |
| KIT-A4-F3-P1 | **Platform reporting rules**: What the digital platform reporting rules ask for, which sellers are covered and when. | research | small |  |  |
| KIT-A4-F3-P2 | **Seller details**: Collects and checks the details reporting needs, including tax numbers. | service | medium | KIT-A2-F1-P3, KIT-A4-F3-P1 |  |
| KIT-A4-F3-P3 | **Yearly IRD report**: Builds and files the yearly report and gives each seller a copy. | service | medium | KIT-A4-F3-P2, KIT-A3-F3-P2 |  |
| KIT-A4-F4-P1 | **Yearly statements**: A yearly statement for each worker, printable and in open formats. | service | small | KIT-A3-F3-P2, engine-pdf |  |
| KIT-A4-F4-P2 | **Accounting exports**: Exports in open formats that common accounting software can import. | integration | small | KIT-A3-F3-P2 |  |
| KIT-A4-F4-P3 | **Work mileage and expense log**: Work kilometres from finished jobs and time online, measured without keeping the route, and expenses with a receipt photo, gathered into a year-end tax pack. | library | medium | KIT-A4-F4-P1, KIT-A7-F3-P4, KIT-A15-F7-P1 |  |
| KIT-A4-F5-P1 | **Anti-money-laundering programme**: A risk assessment, compliance programme and named compliance officer, checked by a lawyer. | legal | medium |  | [K02](STATUS.md) |
| KIT-A4-F5-P2 | **Customer checks**: Runs the checks the programme requires through the identity and payment providers, reusing the kit's identity check result and the provider's own checks wherever the law allows, so a person proves who they are once. | service | medium | KIT-A4-F5-P1, KIT-A2-F1-P2, KIT-A2-F1-P5, KIT-A2-F1-P3 |  |
| KIT-A4-F5-P3 | **Suspicious activity reports**: Flags, reviews and files reports to the authorities, with access kept to named people. | service | medium | KIT-A4-F5-P1, KIT-A3-F1-P2 |  |
| KIT-A4-F5-P4 | **Yearly compliance report**: Gathers the figures for the yearly report and the independent audit. | service | small | KIT-A4-F5-P1 |  |
| KIT-A4-F5-P5 | **Due diligence threshold**: Counts each person's transactions with the co-op over a rolling 12 months and starts customer due diligence once they pass NZ$10,000, as DIA's online marketplace exemption allows (*unverified* exact test), while suspicious activity reporting still covers everyone. | service | medium | KIT-A4-F5-P1, KIT-A4-F5-P2, KIT-A3-F9-P1 |  |
| KIT-A4-F5-P6 | **Customer risk ratings**: Gives each person who passes the threshold a risk rating under the programme, keeps it on record and reviews it when their activity changes. | service | small | KIT-A4-F5-P1, KIT-A4-F5-P5 |  |
| KIT-A4-F5-P7 | **Politically exposed people and enhanced checks**: Checks whether a person over the threshold is politically exposed, and runs enhanced due diligence where the risk rating or the law calls for it (*unverified* detail for marketplaces). | integration | small | KIT-A4-F5-P6, KIT-A2-F1-P2 |  |
| KIT-A4-F6-P1 | **Financial record keeping**: Keeps orders, invoices and payouts for the required period in a tamper-evident log. | service | small | kit-ops, KIT-A18-F6-P1 |  |
| KIT-A4-F7-P1 | **Tax rules per country**: Country packs for VAT and GST rules and platform reporting abroad. | library | large | KIT-A4-F1-P2, KIT-A19-F6-P1 |  |
| KIT-A4-F7-P2 | **Tax on services sold across borders**: Which GST, VAT and digital services taxes apply when a member overseas pays for at-cost storage or AI, or a worker overseas sells remote services, and when the co-op must register. Spare Studio and Spare Work both need the answer. | research | medium | KIT-A4-F7-P1, KIT-A4-F9-P1 |  |
| KIT-A4-F7-P3 | **Levies and tourist taxes**: Each place's levies, tourist taxes and per-trip charges as data in its country pack, such as a state's short-stay levy or a trip levy. Spare Stay and Spare Ride map the same need. | data | small | KIT-A4-F7-P1, KIT-A19-F6-P1 |  |
| KIT-A4-F8-P1 | **Sanctions screening**: Uses the payment provider's screening and records the result. | integration | small | KIT-A3-F1-P2 |  |
| KIT-A4-F9-P1 | **Co-op GST registration**: When the co-op must register for GST, its return periods, and how listed services rules make it the supplier of record for GST. | legal | small | KIT-A4-F1-P1 |  |
| KIT-A4-F9-P2 | **Operator GST returns**: Builds and files the co-op's GST returns as marketplace operator from the GST engine's records, ready for an accountant to check. | service | medium | KIT-A4-F9-P1, KIT-A4-F1-P2, KIT-A4-F6-P1 |  |
| KIT-A4-F9-P3 | **GST on the fee**: Charges GST on the at-cost fee where it applies and issues tax invoices for the fee to GST-registered sellers. | service | small | KIT-A4-F9-P1, KIT-A4-F1-P2, KIT-A4-F2-P1, KIT-A3-F4-P1 |  |
| KIT-A4-F10-P1 | **GST opt-out agreements**: Records opt-out agreements under the listed services rules, checks the size tests, and switches the GST engine to the seller accounting for GST. | service | small | KIT-A4-F1-P1, KIT-A4-F1-P2 |  |
| KIT-A4-F11-P1 | **GST period statements**: Statements for GST-registered workers that match their return periods, showing sales, the GST the platform collected and fees (*unverified* exact duty under the listed services rules). | service | small | KIT-A4-F1-P2, KIT-A4-F4-P1 |  |
| KIT-A4-F12-P1 | **Voluntary withholding rules**: Confirms with IRD how voluntary withholding works when a platform pays a contractor, and what each statement must show. Spare Food and Spare Work both map it. | legal | small | KIT-A4-F3-P1 |  |
| KIT-A4-F12-P2 | **Withholding at payout**: Holds back the rate a worker chooses from each payout, passes it to IRD and shows it on statements and the yearly summary. | service | small | KIT-A4-F12-P1, KIT-A3-F3-P2, KIT-A4-F4-P1 |  |
| KIT-A4-F13-P1 | **GST threshold warning**: Warns a worker or seller whose sales across every Spareware app near the NZ$60,000 GST registration threshold, and explains what registering changes. How platform sales count towards the threshold is for the GST study to confirm (*unverified*). Spare Food and Spare Market map the same warning. | service | small | KIT-A4-F1-P2, KIT-A4-F1-P3, KIT-A3-F3-P3 |  |

## KIT-A5 Marketplace engine

The one loop every marketplace runs (list or ask, match, agree, pay and hold, do the job, confirm, pay out, rate, resolve), plus changes, extra charges and promotions, built once in kit-market, with each app adding only its own rules.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A5-F1** List something | Create a listing for a ride, a meal, a stay, a home, a job or an item. | core | P1, P2, P3, P4, P5 |
| **KIT-A5-F2** Ask for something | Post a request, such as a ride or a job, and let workers respond. | core | P1 |
| **KIT-A5-F3** Agree a price | See the full price before agreeing, set by the app's own pricing rule. | core | P1, P2, P3 |
| **KIT-A5-F4** Follow an order from start to finish | See where an order is up to, from booking to paid out. | core | P1, P2, P3 |
| **KIT-A5-F5** Set when I am available | Set opening hours, free dates or shifts, and sync calendars from other booking sites. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A5-F6** Cancel fairly | Cancel a booking knowing the cost in advance, under the policy in force when I booked. | core | P1 |
| **KIT-A5-F7** Confirm the job is done | Confirm a job is done, or have it confirm itself after a set time if nobody objects. | core | P1, P2 |
| **KIT-A5-F8** See my orders | Find past and current orders, receipts and messages, and book again. | core | P1 |
| **KIT-A5-F9** Book ahead or repeat a booking | Book for a later time, or set a booking to repeat weekly. | core | P1 |
| **KIT-A5-F10** Keep stock right everywhere I sell | Keep stock counts right across an online shop, a market listing and a shop till. | next | P1 |
| **KIT-A5-F11** Run a job with several parties | Run an order that needs more than one party, such as a restaurant and a courier. | core | P1 |
| **KIT-A5-F12** Save favourites | Save favourite workers, listings and places to use again. | core | P1, P2 |
| **KIT-A5-F13** Add a new marketplace without changing the engine | Let a new app add its own rules as plug-ins instead of changing the engine. | later | P1 |
| **KIT-A5-F14** Use a discount or voucher | Use a discount code, voucher or referral credit, and see who pays for it. | next | P1, P2 |
| **KIT-A5-F15** Change an order or add a charge | Change a booking after it is made, or agree an extra charge after the job, with evidence and consent. | core | P1, P2 |
| **KIT-A5-F16** Choose the right kind of service | Pick a service type from one shared list, so every app knows what it includes and which checks the worker needs. | core | P1, P2 |
| **KIT-A5-F17** Bring my listings from another platform | Bring my own listings, menus, product lists and calendars across from another platform without retyping them. | next | P1, P2 |
| **KIT-A5-F18** Send a parcel or goods | Book a courier to carry a parcel or goods from one place to another. | later | P1 |
| **KIT-A5-F19** Put things in a cart | Gather items from one or more sellers in a cart and check out once. | core | P1 |
| **KIT-A5-F20** Bring others into an order | Invite others onto a booking or a shared cart, or book for someone else. | next | P1, P2 |
| **KIT-A5-F21** Pause or go away | Pause taking orders, set my hours, or mark my listings as away until I return. | core | P1 |
| **KIT-A5-F22** Get back something I left behind | Report an item I left in a car or a stay and get it back. | core | P1 |
| **KIT-A5-F23** See how my listings are doing | See views, orders, sales and what needs doing across my listings, in totals that track nobody. | core | P1, P2 |
| **KIT-A5-F24** Know a fair price | See what similar things sold or let for, to set or judge a price. | core | P1, P2 |
| **KIT-A5-F25** Run an auction | Sell by auction, online or in a room, with every bidder seeing the same bids. | later | P1, P2, P3, P4 |
| **KIT-A5-F26** Know what may be sold or offered | Know which goods and services are banned or need a licence, and why. | core | P1, P2, P3 |
| **KIT-A5-F27** Change many listings at once | Change the price, stock or dates of many listings in one step, and undo it in one step. | next | P1 |
| **KIT-A5-F28** See a listing's history | See a listing's price changes, relists and time on the market. | core | P1 |
| **KIT-A5-F29** Answer questions about a listing | See every question about my listing in one place and answer it, privately or for everyone. | core | P1 |
| **KIT-A5-F30** Book a service from another app | Book a pro or add a ride, meal or cleaner from another Spareware app without leaving the one I am in. | core | P1, P2 |
| **KIT-A5-F31** See what I looked at recently | Go back to listings I looked at, kept private to me. | next | P1 |
| **KIT-A5-F32** Order without the app | Book or order by text or phone when I cannot or would rather not use the app. | core | P1, P2 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A5-F1-P1 | **Listing model**: One shared listing, with each app's own fields declared as a schema. | library | large | kit-core, kit-ops |  |
| KIT-A5-F1-P2 | **Listing editor**: A shared editor for text, photos, prices and availability, plus each app's own fields. | interface | medium | KIT-A5-F1-P1, KIT-A20-F1-P1, KIT-A15-F7-P1 |  |
| KIT-A5-F1-P3 | **Listing checks**: Checks a listing against its app's published rules before it goes live. | service | medium | KIT-A5-F1-P1, KIT-A10-F5-P1 |  |
| KIT-A5-F1-P4 | **Accessibility features list**: One shared list of measurable access features, such as step-free entry and door width in centimetres, used on listings and in search filters, so Spare Stay, Spare Rent and Spare Home describe access the same way. | data | small | KIT-A5-F1-P1, KIT-A1-F5-P4 |  |
| KIT-A5-F1-P5 | **Category trees**: Versioned category trees with translations, built from open classifications whose licence allows it and credited, which each app extends. Spare Work and Spare Market both need one. | data | medium | KIT-A5-F1-P1, KIT-A19-F1-P1 |  |
| KIT-A5-F2-P1 | **Request model**: A shared request with place, time, details and how it should be matched. | library | medium | KIT-A5-F1-P1 |  |
| KIT-A5-F3-P1 | **Pricing rules engine**: Metered, fixed, nightly, quoted, offer and auction prices as rules each app chooses. | library | large | KIT-A3-F1-P4, kit-plugin |  |
| KIT-A5-F3-P2 | **Full price first**: Works out every fee, tax and charge before any price is shown. | library | medium | KIT-A5-F3-P1, KIT-A3-F4-P1, KIT-A4-F1-P2 | [ST05](../spare-stay/STATUS.md) |
| KIT-A5-F3-P3 | **Offers and quotes**: Offers, counter-offers and quotes, each with an expiry. | library | medium | KIT-A5-F3-P1 |  |
| KIT-A5-F4-P1 | **Order steps**: The shared order steps as operations, with each app's own rules for moving between them. | library | large | kit-ops, KIT-A3-F2-P1 |  |
| KIT-A5-F4-P2 | **Order timeline**: Shows each step, who acted and what comes next, in every app. | interface | medium | KIT-A5-F4-P1, KIT-A20-F1-P1 |  |
| KIT-A5-F4-P3 | **First marketplace**: Ranks Ride, Food, Work and others to choose which app proves the engine first. | research | small |  | [K05](STATUS.md) |
| KIT-A5-F5-P1 | **Availability calendars**: Time slots, blocked dates and capacity for each listing or worker. | library | medium | KIT-A5-F1-P1, KIT-A5-F5-P3 |  |
| KIT-A5-F5-P2 | **Calendar import and export**: iCalendar sync with other booking sites, catching double bookings. | integration | small | KIT-A5-F5-P1 | [ST02](../spare-stay/STATUS.md) |
| KIT-A5-F5-P3 | **Repeat and time-zone engine**: Repeating events and their exceptions on the iCalendar standard, with current time-zone data including the Chatham Islands, used by every calendar, repeat booking and recurring job. | library | medium | kit-core, KIT-A19-F9-P1 |  |
| KIT-A5-F5-P4 | **Calendar screens**: Shared day, week, month and many-listing calendar views, with drag to move and tap to block, used by Spare Stay, Spare Work and Spare Office's bookings. | interface | medium | KIT-A5-F5-P1, KIT-A20-F1-P1 |  |
| KIT-A5-F5-P5 | **Booking reminders and change links**: Confirmations and reminders before a booking or viewing, links to move or cancel it within the policy, and a message to everyone booked when it changes. Spare Home, Spare Rent, Spare Office, Spare Social and Spare Market need the same. | service | small | KIT-A5-F5-P1, KIT-A8-F3-P2, KIT-A15-F14-P1, KIT-A5-F6-P1 |  |
| KIT-A5-F5-P6 | **Personal calendar feeds**: A private calendar feed of a person's own bookings, trips and viewings that any calendar app can subscribe to. Spare Stay, Spare Rent and Spare Social need it. | service | small | KIT-A5-F5-P3, KIT-A5-F8-P1 |  |
| KIT-A5-F6-P1 | **Cancellation and no-show policies**: Versioned policies stored with each booking and set by each app's members. | library | medium | KIT-A5-F4-P1, KIT-A12-F4-P1 |  |
| KIT-A5-F7-P1 | **Completion and auto-confirm**: Confirmation by both sides, with a timer that releases payment if nobody objects. | library | small | KIT-A5-F4-P1, KIT-A15-F14-P1 |  |
| KIT-A5-F7-P2 | **Proof of handover**: Handover codes, photos and location stamps that show a job happened. | library | medium | KIT-A5-F4-P1, KIT-A15-F7-P1 |  |
| KIT-A5-F8-P1 | **Orders page**: Lists orders with search, receipts and rebooking. | interface | medium | KIT-A5-F4-P1, KIT-A20-F1-P1 |  |
| KIT-A5-F9-P1 | **Scheduled and repeat bookings**: Future and repeating orders that match and charge at the right time. | library | medium | KIT-A5-F4-P1, KIT-A5-F5-P1, KIT-A5-F5-P3, KIT-A15-F14-P1 |  |
| KIT-A5-F10-P1 | **Stock**: Stock levels and reservations shared by every sales channel. | library | medium | KIT-A5-F1-P1 | [MK06](../spare-market/STATUS.md) |
| KIT-A5-F11-P1 | **Multi-party orders**: Orders split between parties, each with its own steps and payout. | library | medium | KIT-A5-F4-P1, KIT-A3-F2-P1 |  |
| KIT-A5-F12-P1 | **Favourites**: Saved listings, workers and places for each person. | library | small | KIT-A5-F1-P1 |  |
| KIT-A5-F12-P2 | **Shortlists with notes and compare**: Adds private notes, sharing with chosen people and a side-by-side compare of facts and full prices to saved listings. Spare Home, Spare Rent and Spare Stay need the same. | library | small | KIT-A5-F12-P1, KIT-A15-F8-P1 |  |
| KIT-A5-F13-P1 | **App rule plug-ins**: Pricing, matching and step rules supplied by each app as sandboxed plugins. | library | medium | kit-plugin, KIT-A5-F3-P1, KIT-A5-F4-P1 |  |
| KIT-A5-F14-P1 | **Promotion rules**: What fair trading law asks of promotions, vouchers and referral credit, and who may fund them: the worker, a business or the co-op's budget. | legal | small |  |  |
| KIT-A5-F14-P2 | **Promotions and vouchers**: Discount codes, vouchers and referral credit as pricing rules, each showing who pays for it, so no worker's pay is cut without their say. | library | medium | KIT-A5-F14-P1, KIT-A5-F3-P1, KIT-A3-F4-P1 |  |
| KIT-A5-F15-P1 | **Order changes**: A new time, an extra stop, more items or a longer stay, with the new price agreed by both sides and the held amount updated. | library | medium | KIT-A5-F4-P1, KIT-A5-F3-P1, KIT-A3-F2-P1 |  |
| KIT-A5-F15-P2 | **Extra charges after the job**: Cleaning, tolls, damage or agreed variations added after the job, each with evidence, the customer's consent or a route to a dispute. | library | medium | KIT-A5-F4-P1, KIT-A3-F1-P2, KIT-A11-F1-P1, KIT-A15-F7-P1 |  |
| KIT-A5-F16-P1 | **Service catalogue**: One shared list of service types, what each includes, when it is usually needed and which checks a worker needs, used by listings and requests in Work, Home, Rent and Stay. | data | medium | KIT-A5-F1-P1, KIT-A2-F2-P2 |  |
| KIT-A5-F16-P2 | **Catalogue entries**: Each professional's or local service's offer listed once in the shared service catalogue, with its checks, prices and reviews, so Spare Home, Spare Rent and Spare Stay book the same pro that Spare Work lists, with one account and one payout. | data | medium | KIT-A5-F16-P1, KIT-A2-F2-P2, KIT-A9-F1-P1, KIT-A3-F3-P2 |  |
| KIT-A5-F17-P1 | **Import terms**: What each source's terms let a member bring across, such as their own reviews, and what must stay behind. | legal | small |  |  |
| KIT-A5-F17-P2 | **Bulk import**: Imports a member's own listings, menus, product lists and calendars from spreadsheets, exports and published feeds, checked against the app's rules before anything goes live. | library | medium | KIT-A5-F1-P1, KIT-A5-F1-P3, KIT-A5-F5-P2, KIT-A5-F17-P1 |  |
| KIT-A5-F18-P1 | **Delivery job type**: A shared A-to-B delivery job type for parcels and goods, with size limits, banned items and proof of delivery, that Food couriers carry and Market sellers can book. | library | medium | KIT-A5-F2-P1, KIT-A5-F7-P2, KIT-A6-F3-P2 |  |
| KIT-A5-F19-P1 | **Cart**: Items from one or many sellers with options and notes, kept on the device and the server, and split into one order per seller at checkout. Spare Food and Spare Market both need it. | library | medium | KIT-A5-F1-P1, KIT-A5-F3-P2, KIT-A3-F1-P3 |  |
| KIT-A5-F20-P1 | **People on an order**: Invites co-travellers, flatmates or friends onto an order, or shares one cart that several people edit, each seeing only what they need. Spare Stay, Spare Food and Spare Ride map the same need. | library | medium | KIT-A5-F4-P1, KIT-A15-F8-P1, KIT-A5-F19-P1 |  |
| KIT-A5-F20-P2 | **Book for someone else**: Books for a named person whose contact details are used only for that order, with a notice telling them what was shared. Spare Ride, Spare Stay and Spare Food all do this. | library | small | KIT-A5-F4-P1, KIT-A18-F5-P1 |  |
| KIT-A5-F21-P1 | **Away and pause**: Pauses orders for a set time, sets regular hours and holiday closures, or marks listings as away with a message and a return date. Spare Food and Spare Market both need it. | library | small | KIT-A5-F5-P1, KIT-A5-F1-P1 |  |
| KIT-A5-F22-P1 | **Lost property**: Reports a lost item, opens a masked contact with the worker or host for a set time, tracks the return and pays any published return fee to the person who returns it. Spare Ride and Spare Stay need the same flow. | service | small | KIT-A8-F2-P1, KIT-A5-F15-P2, KIT-A3-F1-P2 |  |
| KIT-A5-F23-P1 | **Insights for sellers and hosts**: Counts of views, orders, sales, fees and refunds for each listing, worked out from the operation log in totals only, with simple charts and downloads. Spare Food, Spare Stay, Spare Market, Spare Home and Spare Social map the same need. | service | medium | KIT-A15-F1-P2, KIT-A22-F8-P1, KIT-A27-F3-P3 |  |
| KIT-A5-F23-P2 | **Seller to-do page**: One page of what needs doing across listings, orders, offers, questions and disputes. | interface | small | KIT-A5-F8-P1, KIT-A20-F1-P1 |  |
| KIT-A5-F24-P1 | **Sold price records**: Final prices of finished sales and lets, kept without the buyer's name and shown only where the law and the source's terms allow. | data | medium | KIT-A5-F4-P1, KIT-A13-F9-P2 |  |
| KIT-A5-F24-P2 | **Price guides**: A likely price range from similar recent sales or listings, with every step of the working visible, shown while pricing and searching. Spare Home, Spare Rent and Spare Market need the same guide. | service | medium | KIT-A5-F24-P1, KIT-A6-F1-P2 |  |
| KIT-A5-F25-P1 | **Auction law**: What consumer law, auction rules and in-trade disclosure ask of online and in-room auctions, including who may run a home auction and the rules on seller bids and reserves. | legal | medium | KIT-A18-F8-P1 | [MK02](../spare-market/STATUS.md) |
| KIT-A5-F25-P2 | **Live auction engine**: Bids as operations ordered by the server, one server clock, a public bid history, automatic bids and extensions near the end, so every bidder sees the same truth. Spare Home and Spare Market share it. | service | large | KIT-A5-F3-P1, KIT-A15-F5-P1, KIT-A5-F25-P1 |  |
| KIT-A5-F25-P3 | **Bidder registration**: Registers and checks bidders before an auction, with the checks the law or the seller asks for. | service | small | KIT-A5-F25-P2, KIT-A2-F1-P3 |  |
| KIT-A5-F25-P4 | **Fake bid checks**: Flags bids from accounts linked to the seller or to each other, by device, payment or address signals, as rules on the shared fraud signals service. | service | small | KIT-A5-F25-P2, KIT-A10-F12-P2 |  |
| KIT-A5-F26-P1 | **Restricted goods and services law**: Every NZ law that limits what may be sold or offered online, such as alcohol, vapes, medicines, firearms and work reserved to licensed people, with sources. Spare Market, Spare Work and Spare Food all need it. | legal | medium | KIT-A18-F8-P1 |  |
| KIT-A5-F26-P2 | **Banned and restricted rules**: Banned, restricted and licence-needed items and services as versioned rules linked to categories and keywords, changed by member vote within the law and checked when a listing is made. | library | medium | KIT-A5-F26-P1, KIT-A5-F1-P5, KIT-A5-F1-P3, KIT-A10-F5-P1 |  |
| KIT-A5-F26-P3 | **Rules pages**: Plain pages listing what can and cannot be sold or offered, and why. | interface | small | KIT-A5-F26-P2, KIT-A20-F1-P1 |  |
| KIT-A5-F27-P1 | **Bulk edit**: Changes price, stock, dates or settings for many listings in one checked operation that can be undone together. Spare Stay and Spare Market both need it. | library | small | KIT-A5-F1-P1, KIT-A15-F2-P1 |  |
| KIT-A5-F28-P1 | **Public listing history**: Shows price changes, relists, time on the market and changes made after someone engaged, drawn from the operation log with personal details removed. Spare Home and Spare Market both show it. | library | small | KIT-A5-F1-P1, KIT-A15-F3-P1, KIT-A15-F1-P6 |  |
| KIT-A5-F29-P1 | **Enquiry inbox**: One inbox for every question about a listing, linked to the asker's profile, with quick replies, files and answers the lister can publish for everyone. Spare Home and Spare Rent both need it. | interface | medium | KIT-A8-F1-P2, KIT-A5-F1-P1, KIT-A8-F13-P1 |  |
| KIT-A5-F30-P1 | **Bookings across apps**: Books a service from the shared catalogue, or adds another app's service to an order, such as an airport ride, a meal or a cleaner, as a linked kit-market order, so no app calls another app's code. Spare Home, Spare Stay and Spare Rent book Spare Work's pros this way. | service | medium | KIT-A5-F11-P1, KIT-A5-F16-P2, KIT-A16-F1-P1 |  |
| KIT-A5-F30-P2 | **Which ledger carries a cross-app booking**: Records each cross-app booking against the app it was booked from, so its fee and cost show once, in one app's ledger. | library | small | KIT-A13-F2-P2, KIT-A5-F30-P1 |  |
| KIT-A5-F31-P1 | **Recently viewed**: A private list of listings a person looked at, kept on the device unless they choose to sync it. Spare Stay and Spare Market both offer it. | library | small | KIT-A5-F1-P1, KIT-A15-F4-P4 |  |
| KIT-A5-F32-P1 | **Text ordering and booking**: Books or orders by text from a favourite or a past order, with updates by text, a PIN or an approval in the app for a saved card, and the text cost guard. Spare Ride books this way, and Spare Food maps the same channel. | service | medium | KIT-A8-F4-P2, KIT-A8-F4-P4, KIT-A5-F12-P1 |  |
| KIT-A5-F32-P2 | **Phone ordering line**: A phone number with a simple menu and a person who takes the order into the same order path, including calls through NZ Relay. Spare Ride and Spare Food both need it. | service | medium | KIT-A21-F4-P1, KIT-A5-F4-P1 |  |

## KIT-A6 Search, matching and dispatch

Pairs a request with the right worker or listing (search with filters, nearest driver, ready-timed courier, open dates, best quote) using rules anyone can read, fair to workers and customers alike. Built in svc-matching; svc-search is a proposed name (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A6-F1** Search listings | Search by words, place, date and price, with results ranked by published rules. | core | P1, P2, P3, P4, P5 |
| **KIT-A6-F2** Go online for work | Go online or offline, and set where, when and how much work I take on. | core | P1 |
| **KIT-A6-F3** Get matched quickly | Have a request sent to the best worker nearby within seconds. | core | P1, P2, P3, P4 |
| **KIT-A6-F4** Accept or decline work fairly | See the job and pay before accepting, and decline without hidden penalties. | core | P1, P2, P3, P4, P5 |
| **KIT-A6-F5** Know why I was matched | See why a worker or listing was matched or ranked where it was. | core | P1, P2, P3, P4 |
| **KIT-A6-F6** Get quotes from several workers | Send one request to several suitable workers and compare their quotes. | core | P1 |
| **KIT-A6-F7** Take jobs going the same way | Let a worker take two jobs on the same route where the app allows it. | next | P1 |
| **KIT-A6-F8** Save a search | Save a search and be told when something new matches. | core | P1 |
| **KIT-A6-F9** See where work is busy | See where and when demand is high, to choose where to work. | later | P1, P2 |
| **KIT-A6-F10** Get suggestions I can switch off | See suggestions based on my past use, with reasons shown and an off switch. | later | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A6-F1-P1 | **Search engine choice**: Compares open search engines on speed, licence and running cost. | research | small |  | [MK05](../spare-market/STATUS.md) |
| KIT-A6-F1-P2 | **Search service**: The one search service: full-text, filter and map search for listings and any other content an app declares, for every app. | service | large | KIT-A5-F1-P1, KIT-A7-F2-P2, KIT-A6-F1-P1, KIT-A19-F1-P4 | [MK05](../spare-market/STATUS.md) |
| KIT-A6-F1-P3 | **Published ranking rules**: Ranking rules anyone can read, with no paid placement. | library | medium | KIT-A6-F1-P2 |  |
| KIT-A6-F1-P4 | **Audience filters**: Shows each search result only to the people its owner chose, so posts, files and groups can be searched as safely as listings. | library | medium | KIT-A6-F1-P2, KIT-A15-F8-P1 |  |
| KIT-A6-F1-P5 | **Search screens**: Shared search box, suggestions, recent searches, filter chips, sorting and result lists that work on a phone and with a screen reader, which each app fills with its own fields. Every marketplace, Spare Office and Spare Social build on them. | interface | medium | KIT-A6-F1-P2, KIT-A20-F1-P1, KIT-A19-F2-P1 |  |
| KIT-A6-F2-P1 | **Worker availability**: Online status, work areas, capacity and breaks for each worker. | service | medium | KIT-A1-F5-P2, KIT-A5-F5-P1 |  |
| KIT-A6-F3-P1 | **Live location index**: A hexagon-grid index of where workers are, updated in real time. | library | large | KIT-A7-F3-P2, KIT-A7-F4-P1 |  |
| KIT-A6-F3-P2 | **Dispatch engine**: Sends offers to workers in turn, with timeouts and passing on. | service | large | KIT-A6-F3-P1, KIT-A6-F2-P1, KIT-A5-F2-P1 |  |
| KIT-A6-F3-P3 | **Matching rules per app**: Each app's scoring rules, such as distance, ready time or trade, as plugins. | library | medium | KIT-A6-F3-P2, kit-plugin |  |
| KIT-A6-F3-P4 | **Access-aware matching**: Matches a request only with workers and listings that meet the access needs it carries, and sends any refusal of an access need to a person for review. | library | small | KIT-A6-F3-P2, KIT-A1-F5-P4, KIT-A10-F6-P2 |  |
| KIT-A6-F4-P1 | **Job offer screen**: Shows the job, pay, distance and time before accepting. | interface | medium | KIT-A6-F3-P2, KIT-A20-F1-P1 |  |
| KIT-A6-F4-P2 | **Fair offer rules**: Published, member-voted rules on declines, with no hidden throttling. | library | small | KIT-A6-F3-P2, KIT-A12-F4-P1 |  |
| KIT-A6-F4-P3 | **Automated acceptance rules**: Published, member-voted rules for scripts and agents that accept offers or grab bookings, so bots do not beat people to work or scarce bookings. | library | small | KIT-A6-F4-P2, KIT-A17-F4-P1 |  |
| KIT-A6-F4-P4 | **More time to answer offers**: Lets a worker choose a longer time to answer offers when they need it, as WCAG 2.2.1 asks of time limits, with dispatch allowing for it. | library | small | KIT-A6-F3-P2, KIT-A19-F2-P1 |  |
| KIT-A6-F4-P5 | **Fair queues**: Per-person caps and a fair queue for scarce viewings, applications and limited releases, so nobody, person or script, takes every slot. Spare Rent and Spare Market need it. | library | small | KIT-A6-F4-P3, KIT-A22-F5-P4 |  |
| KIT-A6-F5-P1 | **Match explanations**: A plain reason with each match and each ranking. | library | small | KIT-A6-F3-P2, KIT-A6-F1-P2 |  |
| KIT-A6-F5-P2 | **Fairness checks**: Regular checks that offers, rankings and declines are fair to workers and to customers, with results published through the public reporting pipeline. | service | medium | KIT-A6-F3-P2, KIT-A13-F9-P1 |  |
| KIT-A6-F5-P3 | **Decline pattern checks**: Spots workers or hosts declining people in patterns that point to discrimination on grounds the Human Rights Act 1993 protects, for a person to review. | service | medium | KIT-A6-F5-P2, KIT-A10-F19-P1, KIT-A10-F6-P2, KIT-A6-F5-P4 |  |
| KIT-A6-F5-P4 | **Measuring discrimination without collecting ethnicity**: Measures gaps in how groups of people are treated without storing members' ethnicity, porting a published privacy-preserving method such as Airbnb's Project Lighthouse, with a privacy impact assessment first. | research | medium | KIT-A6-F5-P2, KIT-A18-F10-P1 |  |
| KIT-A6-F6-P1 | **Quote requests**: Sends a request to a shortlist and gathers quotes side by side. | library | medium | KIT-A6-F3-P2 |  |
| KIT-A6-F7-P1 | **Batching**: Groups compatible jobs by route and time. | library | large | KIT-A6-F3-P2, KIT-A7-F3-P2 |  |
| KIT-A6-F8-P1 | **Saved searches**: Stores searches and sends alerts through the person's chosen channels. | service | small | KIT-A6-F1-P2, KIT-A8-F3-P2 |  |
| KIT-A6-F9-P1 | **Demand maps**: Demand by area and hour, shown only in groups large enough to protect privacy. | service | medium | KIT-A6-F3-P1, KIT-A7-F1-P2 |  |
| KIT-A6-F9-P2 | **Demand forecasts**: Short-term demand forecasts by area and hour from past orders, events and weather, using proven open methods, shown only in groups large enough to protect privacy. Spare Ride and Spare Food both map them. | service | medium | KIT-A6-F9-P1, KIT-A10-F22-P1 |  |
| KIT-A6-F10-P1 | **Explainable suggestions**: Simple suggestions that explain themselves and run only with consent. | library | medium | KIT-A6-F1-P2 |  |

## KIT-A7 Maps, places and live location

Maps, address search, routing, live location and service areas on open data, so no app depends on a paid map provider, with exact places kept private until a booking needs them. Built as svc-maps.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A7-F1** See a map | See a fast map of where a job or listing is, in every app. | core | P1, P2, P3, P4 |
| **KIT-A7-F2** Find an address | Type an address or place and pick it from suggestions. | core | P1, P2, P3, P4 |
| **KIT-A7-F3** Get a route and arrival time | See the route and a reliable arrival time for a job. | core | P1, P2, P3, P4 |
| **KIT-A7-F4** Share my location only during a job | Let the other side see where I am, only while the job is running. | core | P1, P2, P3 |
| **KIT-A7-F5** Set the area I serve | Draw the area a worker or business serves, or where an app runs. | core | P1, P2, P3 |
| **KIT-A7-F6** Get turn-by-turn directions | Get spoken directions in the app, or hand off to my phone's map app. | core | P1 |
| **KIT-A7-F7** Use maps with no signal | Keep a map and my job addresses working without a connection. | next | P1 |
| **KIT-A7-F8** Fix a map mistake | Report a wrong address or road so it gets fixed for everyone. | next | P1 |
| **KIT-A7-F9** See the right local time and places | See local time zones and points of interest correctly in every country. | later | P1 |
| **KIT-A7-F10** Keep my exact place private until booked | Show an approximate area on listings and profiles, and the exact address only once a booking is confirmed. | core | P1 |
| **KIT-A7-F11** See the facts about a place | See the hazards, zones and local rules that apply to an address, with their sources. | core | P1, P2, P3 |
| **KIT-A7-F12** See boundaries on aerial photos | See a section's boundaries drawn on an aerial photo. | core | P1 |
| **KIT-A7-F13** Count public transport in travel times | See travel times by bus and train as well as by car. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A7-F1-P1 | **Map tiles**: Vector tiles built from OpenStreetMap data and served at cost. | service | large | KIT-A7-F1-P3 |  |
| KIT-A7-F1-P2 | **Map display**: Map display with MapLibre, styled to the design guide. | library | medium | KIT-A7-F1-P1, KIT-A20-F1-P1 |  |
| KIT-A7-F1-P3 | **Map data licence**: Meets OpenStreetMap's licence: attribution shown and share-alike handled for any changed data. | legal | small |  |  |
| KIT-A7-F1-P4 | **Maps in words**: A list view, written directions and areas described in words beside every map, so screen reader users get the same information. | library | small | KIT-A7-F1-P2, KIT-A19-F2-P1 |  |
| KIT-A7-F2-P1 | **Geocoder choice**: Compares open geocoders on NZ address quality and licence, keeping any GPL engine as a separate service. | research | small |  |  |
| KIT-A7-F2-P2 | **Address search**: Address and place search with suggestions as you type. | service | large | KIT-A7-F2-P1, KIT-A19-F1-P4 |  |
| KIT-A7-F2-P3 | **NZ address data**: Loads official NZ address data under its open licence and keeps it current. | data | medium | KIT-A7-F2-P2 |  |
| KIT-A7-F2-P4 | **Address from a pin**: Turns a pin or GPS position into the nearest address. | service | small | KIT-A7-F2-P2 |  |
| KIT-A7-F3-P1 | **Routing engine choice**: Compares open routing engines on NZ travel-time accuracy, speed, licence and running cost. | research | medium |  | [RD03](../spare-ride/STATUS.md) |
| KIT-A7-F3-P2 | **Routing service**: Routes, travel times and distance tables for every app. | service | large | KIT-A7-F1-P1, KIT-A7-F3-P1 | [RD03](../spare-ride/STATUS.md) |
| KIT-A7-F3-P3 | **Arrival time estimates**: Arrival times that learn from finished trips without keeping anyone's trace. | library | medium | KIT-A7-F3-P2 |  |
| KIT-A7-F3-P4 | **Map matching**: Snaps GPS traces to roads to measure distance accurately for metered fares, pay by distance and the mileage log, using the open routing engine's map matching. | library | medium | KIT-A7-F3-P2 | [RD03](../spare-ride/STATUS.md) |
| KIT-A7-F4-P1 | **Live location stream**: Real-time positions that stop when the job ends and are not kept afterwards. | service | large | kit-sync, KIT-A5-F4-P1 |  |
| KIT-A7-F4-P2 | **Trip trace**: Keeps a job's route only where the law, safety or a dispute needs it. | library | small | KIT-A7-F4-P1, KIT-A18-F6-P1 |  |
| KIT-A7-F4-P3 | **Faked location checks**: Spots simulated or spoofed positions from device signals and impossible movement, and flags them for review before fares or payouts rely on them. | library | medium | KIT-A7-F4-P1 |  |
| KIT-A7-F5-P1 | **Service areas**: Area shapes, checks for whether a place is inside, and app launch areas. | library | medium | KIT-A7-F1-P2 |  |
| KIT-A7-F5-P2 | **Area editor**: Draws and changes service areas, zones and fare regions on a map, each change a versioned operation. Spare Ride and Spare Food both need it. | interface | small | KIT-A7-F5-P1, KIT-A7-F1-P2 |  |
| KIT-A7-F5-P3 | **Launch readiness**: Counts workers, customers and sellers signed up in each launch area against published targets before the area opens. Spare Food and Spare Ride plan launches this way. | service | small | KIT-A7-F5-P1, KIT-A22-F8-P1 |  |
| KIT-A7-F6-P1 | **Turn-by-turn guidance**: Spoken guidance from the routing service, or a hand-off to the phone's own map app. | library | large | KIT-A7-F3-P2, KIT-A20-F2-P2 | [K04](STATUS.md) |
| KIT-A7-F7-P1 | **Offline map packs**: Downloadable map areas for workers in places with weak signal. | library | medium | KIT-A7-F1-P1 |  |
| KIT-A7-F8-P1 | **Map fix reports**: Sends fixes to OpenStreetMap, where the open data is kept. | interface | small | KIT-A7-F1-P2 |  |
| KIT-A7-F9-P1 | **Time zones and places**: Time zone lookup by place, and open points of interest. | data | small | KIT-A7-F2-P2 |  |
| KIT-A7-F10-P1 | **Approximate location**: Blurs a place to an area on maps and in search, and reveals the exact address to the other side only when the order reaches the step that needs it. | library | small | KIT-A7-F1-P2, KIT-A5-F4-P1 |  |
| KIT-A7-F11-P1 | **Area data sources**: Which open sources exist for natural hazards, such as flood, liquefaction and coastal hazards, and for zoning, school zones and transport, under what licence and at what cost. Spare Construct, Spare Home and Spare Rent need the same answer. | research | medium | KIT-A26-F1-P2 |  |
| KIT-A7-F11-P2 | **Area layers and lookups**: Hazard, zone and school zone layers on the map, and a lookup for any address, from sources whose licence allows it, each shown with its source and date. | service | medium | KIT-A7-F11-P1, KIT-A7-F1-P2, KIT-A7-F2-P2 |  |
| KIT-A7-F11-P3 | **Council rules as data**: Each council's rules that apps apply, such as short-stay registration and night caps, sign rules and planning rules, kept as dated data with sources. Spare Stay, Spare Home and Spare Construct need them. | data | medium | KIT-A19-F9-P1, KIT-A7-F11-P1 |  |
| KIT-A7-F12-P1 | **Aerial photos and boundary lines**: Adds open aerial imagery and property boundary lines from LINZ to the kit's maps under their licences. Spare Home and Spare Construct both need them. | integration | medium | KIT-A7-F1-P1, KIT-A7-F1-P3, KIT-A26-F1-P2 |  |
| KIT-A7-F13-P1 | **Public transport timetables**: Imports the open timetable feeds (GTFS) that transport agencies publish, under their terms, so travel times can include buses and trains. Spare Ride and Spare Rent both need them. | integration | medium | KIT-A7-F3-P2 |  |

## KIT-A8 Messaging and notifications

Chat between the two sides of a job, calls that hide phone numbers, video calls, push, email, text and letters by post, with each member choosing what reaches them. Built in svc-messaging; svc-calls is a proposed name (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A8-F1** Chat about a job | Message the other side of a job, with photos, until the job is closed. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A8-F2** Call without sharing my number | Phone the other side of a job without either of us seeing the other's number. | core | P1 |
| **KIT-A8-F3** Get push notifications | Get alerts on my phone or computer for the things I care about. | core | P1, P2, P3, P4 |
| **KIT-A8-F4** Get emails and texts | Get receipts, codes and urgent alerts by email or text. | core | P1, P2, P3, P4, P5 |
| **KIT-A8-F5** Choose what reaches me | Turn each kind of alert on or off for each channel, and set quiet hours. | core | P1, P2, P3 |
| **KIT-A8-F6** Block and report from a chat | Block someone or report a message straight from the chat. | core | P1 |
| **KIT-A8-F7** Read messages in my language | Read messages from the other side translated into my language. | core | P1 |
| **KIT-A8-F8** Hear from the co-op | Get co-op news, votes and meeting notices in one place. | core | P1 |
| **KIT-A8-F9** Be warned about scams in chat | See a warning when a message looks like a scam or asks me to pay outside the app. | core | P1 |
| **KIT-A8-F10** Keep private chats unreadable to anyone else | Have private chats that even the co-op cannot read. | core | P1 |
| **KIT-A8-F11** Make a video call | Make a video call with one person or a group, in any app, at cost. | core | P1, P2 |
| **KIT-A8-F12** Discuss in threads | Discuss a proposal, a cost line, a suggestion or a file in a thread, with replies and mentions. | core | P1, P2 |
| **KIT-A8-F13** Reuse my replies | Save replies I send often and answer in one tap. | core | P1 |
| **KIT-A8-F14** Schedule a message | Write a message now and have it sent at a set time or when something happens. | next | P1 |
| **KIT-A8-F15** Run a quick poll | Ask a quick question in a chat, post or meeting and see the answers live. | next | P1 |
| **KIT-A8-F16** Keep encrypted chats safe on every device | Have private group chats stay unreadable to anyone else on every device I use, with a backup only I can open. | core | P1 |
| **KIT-A8-F17** Get a letter by post | Get a code or a formal notice by post when the law or a check needs one. | core | P1 |
| **KIT-A8-F18** See captions in a call | Read live captions in a call or meeting, in my language if I ask. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A8-F1-P1 | **Chat protocol choice**: Compares Matrix with building chat on the kit, including licences: Matrix's main servers, Synapse and Dendrite, are AGPL, so one could only ever run as a separate service, never in the core. | research | medium |  | [OF05](../spare-office/STATUS.md) |
| KIT-A8-F1-P2 | **Chat service**: The one chat server for every app, Spare Office's chat included: job chats and group chats, stored as operations and synced to every device. | service | large | kit-sync, KIT-A1-F1-P1, KIT-A8-F1-P1 | [OF05](../spare-office/STATUS.md) |
| KIT-A8-F1-P3 | **Chat screens**: Shared chat screens with photos, read receipts and quick replies. | interface | medium | KIT-A8-F1-P2, KIT-A20-F1-P1, KIT-A15-F7-P1 |  |
| KIT-A8-F1-P4 | **Link previews without tracking**: Our server fetches a link's title and picture once and shows them to readers, so the linked site never sees who read it, and never inside end-to-end encrypted chats. | service | small | KIT-A8-F1-P2, KIT-A15-F7-P1 |  |
| KIT-A8-F1-P5 | **Interception duty check**: Whether the co-op's chat and call services make it a service provider with a duty to assist under the Telecommunications (Interception Capability and Security) Act 2013 (*unverified* whether app chat counts). | legal | small |  |  |
| KIT-A8-F1-P6 | **Bridges to other chat networks**: Whether links to other chat networks are allowed by their terms, and how to join only those without weakening encryption. Spare Office and Spare Social share this question. | research | small | KIT-A8-F1-P1, KIT-A8-F10-P1 |  |
| KIT-A8-F2-P1 | **Masked calls**: Temporary numbers from a phone provider, paid at cost, that stop working after the job. | integration | medium | KIT-A5-F4-P1 |  |
| KIT-A8-F3-P1 | **Push delivery**: Sends push through Apple's and Google's push services and web push, as phones require. | integration | medium | KIT-A20-F2-P2 | [K04](STATUS.md) |
| KIT-A8-F3-P2 | **Notification router**: Chooses the channel for each alert from the person's settings, with retries. | service | medium | KIT-A8-F3-P1, KIT-A8-F4-P1, KIT-A8-F4-P2 |  |
| KIT-A8-F3-P3 | **Notification inbox**: One place in each app to see every alert. | interface | small | KIT-A8-F3-P2, KIT-A20-F1-P1 |  |
| KIT-A8-F3-P4 | **Private push content**: Push sent through Apple and Google carries only a wake-up signal or encrypted content, never message text in the clear. | library | small | KIT-A8-F3-P1 |  |
| KIT-A8-F4-P1 | **Email sending**: Sends email through a provider paid at cost, handling bounces and complaints. | integration | medium |  |  |
| KIT-A8-F4-P2 | **Text sending**: Sends texts through a provider paid at cost, for codes and urgent alerts. | integration | medium |  |  |
| KIT-A8-F4-P3 | **Message templates**: Templates for each app and language, with plain-text copies. | library | small | KIT-A19-F1-P1 |  |
| KIT-A8-F4-P4 | **Text cost guard**: Limits on texted codes by number, country and hour, with alerts on unusual volume, to stop SMS pumping fraud running up an at-cost bill. | service | small | KIT-A8-F4-P2, KIT-A22-F5-P4 |  |
| KIT-A8-F4-P5 | **Sender authentication**: SPF, DKIM and DMARC on every sending domain, and a page where a member can check that an email really came from Spareware. | service | small | KIT-A8-F4-P1 |  |
| KIT-A8-F5-P1 | **Notification settings**: Choices by type and channel, plus quiet hours, shared by every app. | interface | small | KIT-A8-F3-P2, KIT-A20-F1-P1, KIT-A8-F5-P2 |  |
| KIT-A8-F5-P2 | **Consent for messages**: Meets spam law: consent, a named sender and one-step unsubscribe for anything not essential. | legal | small |  |  |
| KIT-A8-F5-P3 | **Digests**: Daily or weekly summaries by email or in the app instead of single alerts, chosen for each kind of alert. Spare Social, Spare Office, Spare Work, Spare Market and Spare Home all offer them. | service | small | KIT-A8-F3-P2, KIT-A8-F5-P1, KIT-A8-F4-P1 |  |
| KIT-A8-F6-P1 | **Report from chat**: Report and block actions on every message, attaching the message to the report. | interface | small | KIT-A8-F1-P2, KIT-A10-F3-P1 |  |
| KIT-A8-F7-P1 | **Message translation**: Optional translation of chat messages, with the original one tap away. | integration | medium | KIT-A8-F1-P2, KIT-A19-F10-P2 |  |
| KIT-A8-F8-P1 | **Co-op announcements**: Messages from the co-op and each app to members who chose to receive them. | service | small | KIT-A8-F3-P2 |  |
| KIT-A8-F9-P1 | **Scam warnings**: Scam warnings in chat, as rules on the shared fraud signals service, with no message content sent elsewhere. | library | medium | KIT-A8-F1-P2, KIT-A10-F12-P2 | [MK04](../spare-market/STATUS.md) |
| KIT-A8-F10-P1 | **End-to-end encryption choice**: When chats should be end-to-end encrypted, and how that fits with reports and disputes. | research | medium | KIT-A8-F1-P2 |  |
| KIT-A8-F11-P1 | **Call method choice**: Compares open WebRTC servers and relay servers on licence, quality and running cost, together with Spare Office. | research | medium |  | [OF05](../spare-office/STATUS.md) |
| KIT-A8-F11-P2 | **Call service**: One-to-one and group video calls over WebRTC for every app, with relay servers run at cost. | service | large | KIT-A8-F11-P1, engine-media, KIT-A1-F1-P1 |  |
| KIT-A8-F12-P1 | **Threads and comments**: One threaded discussion part, with replies, mentions, reports and moderation, used by proposals, the member forum, suggestions, ledger questions, panel discussion, consultations and comments on files. | service | medium | kit-ops, KIT-A8-F3-P2, KIT-A10-F3-P1, KIT-A20-F1-P1 |  |
| KIT-A8-F12-P2 | **Anchored comments**: Comments pinned to a point, region, page, video frame or text range, with replies and mentions, that map to and from PDF annotations. Spare Studio, Spare Office and Spare Construct's markups need the same model. | library | medium | KIT-A8-F12-P1, KIT-A27-F6-P4 |  |
| KIT-A8-F13-P1 | **Saved replies**: Reusable replies with fields such as a name and dates, and answers a lister can publish for everyone. Spare Stay, Spare Rent and Spare Work need them. | library | small | KIT-A8-F1-P3, KIT-A8-F4-P3 |  |
| KIT-A8-F14-P1 | **Scheduled and automatic messages**: Messages sent at a chosen time or on a trigger, such as the day before arrival, each labelled as automatic. Spare Stay, Spare Social and Spare Office need them. | service | small | KIT-A8-F1-P2, KIT-A15-F14-P1 |  |
| KIT-A8-F15-P1 | **Quick polls**: One-question polls in a chat, a post, a meeting or a live talk, one vote per person and never by an agent, with live results, kept apart from binding member votes. Spare Social and Spare Office both need them. | service | small | KIT-A8-F1-P2, KIT-A17-F9-P2 |  |
| KIT-A8-F16-P1 | **Encryption library**: One library for end-to-end encrypted group chats on the MLS standard, with each device holding its own keys, members joining and leaving safely, and backups the co-op cannot read, using openly licensed code. Spare Social and Spare Office both need it. | library | large | KIT-A8-F10-P1, KIT-A1-F3-P2, KIT-A22-F5-P5 |  |
| KIT-A8-F17-P1 | **Posted letters**: Prints and posts letters, notices and codes through a mail provider paid at cost, recording the posting date as evidence. Spare Home's owner notices, Spare Rent's posted notices and Spare Market's address checks all need it. | integration | medium | KIT-A8-F3-P2, engine-pdf, KIT-A13-F2-P2 |  |
| KIT-A8-F18-P1 | **Live captions in calls**: Live captions in voice and video calls and meetings, with speaker names, made on each person's device in encrypted calls and translated if a viewer asks. Spare Social and Spare Office both need them. | service | medium | KIT-A8-F11-P2, KIT-A27-F12-P4, KIT-A19-F10-P2 |  |

## KIT-A9 Ratings and reputation

Two-way ratings and reviews after real jobs, made by people and never agents, fair rules on what counts, and a reputation members can see, challenge and take with them. Built as svc-trust.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A9-F1** Rate after a job | Rate the other side after a job, and see their rating of me only once we have both rated. | core | P1, P2 |
| **KIT-A9-F2** Write and answer reviews | Write a public review, and reply once to a review about me. | core | P1, P2 |
| **KIT-A9-F3** Not be marked down for what I can't control | Have ratings that blame things outside my control set aside. | core | P1 |
| **KIT-A9-F4** Know the rating rules | See the rating levels that lead to a warning or a pause, set by member vote. | core | P1, P2 |
| **KIT-A9-F5** Challenge a rating or review | Ask for a rating or review to be checked and removed if it breaks the rules. | core | P1 |
| **KIT-A9-F6** Show my reputation | Show jobs done, ratings and checks passed on my profile. | core | P1 |
| **KIT-A9-F7** Take my reputation with me | Export my ratings and reviews in an open, signed format others can check. | core | P1 |
| **KIT-A9-F8** Trust that reviews are real | Know reviews come from real jobs and fake ones are removed. | core | P1 |
| **KIT-A9-F9** Earn badges by published rules | Earn badges for good service under rules members set, and know how a badge is lost. | core | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A9-F1-P1 | **Ratings service**: Two-way ratings tied to finished orders and revealed together. | service | medium | KIT-A5-F4-P1, kit-ops |  |
| KIT-A9-F1-P2 | **Rating screen**: A quick star rating with optional tags and a comment. | interface | small | KIT-A9-F1-P1, KIT-A20-F1-P1 |  |
| KIT-A9-F2-P1 | **Reviews**: Written reviews with one reply each, always linked to a real job. | service | medium | KIT-A9-F1-P1, KIT-A9-F2-P2 |  |
| KIT-A9-F2-P2 | **Fair review rules**: What may be published about people, and how defamation complaints are handled. | legal | small |  | [RT04](../spare-rent/STATUS.md) |
| KIT-A9-F3-P1 | **Rating exclusions**: Rules that set aside ratings caused by things like a slow kitchen or traffic. | library | small | KIT-A9-F1-P1 |  |
| KIT-A9-F4-P1 | **Rating thresholds**: Published, voted thresholds with a warning and time to improve before any pause. | library | small | KIT-A9-F1-P1, KIT-A12-F4-P1 |  |
| KIT-A9-F4-P2 | **Rating bias checks**: Checks ratings for bias against groups of workers, using the privacy-safe measurement method, before any rating can lead to a warning or a pause. | service | small | KIT-A9-F4-P1, KIT-A6-F5-P4 |  |
| KIT-A9-F5-P1 | **Rating challenge**: Sends a challenge with the order's facts to the one review queue, where member moderators decide. | interface | small | KIT-A9-F1-P1, KIT-A10-F6-P2 |  |
| KIT-A9-F6-P1 | **Reputation summary**: A profile summary of jobs done, ratings and checks passed. | interface | small | KIT-A9-F1-P1, KIT-A2-F1-P3 |  |
| KIT-A9-F7-P1 | **Portable reputation**: Signed exports of ratings and reviews that anyone can verify. | library | medium | KIT-A9-F1-P1, KIT-A18-F1-P1 |  |
| KIT-A9-F8-P1 | **Fake review checks**: Flags review rings and paid reviews from order and account patterns, as rules on the shared fraud signals service. | service | medium | KIT-A9-F2-P1, KIT-A10-F12-P2 |  |
| KIT-A9-F9-P1 | **Earned badges**: Badges earned from published criteria, such as reply time or cancellations, with criteria set by member vote and each badge saying how it is lost. Spare Rent, Spare Stay, Spare Market and Spare Social map the same need. | service | small | KIT-A9-F6-P1, KIT-A12-F4-P1, KIT-A2-F5-P1 |  |

## KIT-A10 Safety, reports and moderation

Keeps people safe during a job and online: emergency help that works anywhere, official alerts and a disaster mode, helplines, a safe path for sexual harm reports, reports, blocking, published rules, fair treatment whoever you are, member moderators, legal notices, known-illegal-content checks and a public log of every removal. Built as svc-trust.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A10-F1** Get help in an emergency | Call emergency services from inside a job, with my location and the job details ready to read out. | core | P1, P2, P3 |
| **KIT-A10-F2** Share my trip with someone I trust | Send a friend a live link to my trip or job. | core | P1 |
| **KIT-A10-F3** Report a problem | Report a person, listing, message or post, and follow what happens to my report. | core | P1, P2, P3 |
| **KIT-A10-F4** Block someone | Block a person so they can't contact me or be matched with me again. | core | P1 |
| **KIT-A10-F5** Read the rules | Read each app's rules in plain words, with every past version. | core | P1 |
| **KIT-A10-F6** Have reports handled by trained members | Have reports reviewed by trained, elected members following clear steps. | core | P1, P2, P3, P4 |
| **KIT-A10-F7** See every removal in public | See a public log of every removal or ban and the rule behind it. | core | P1 |
| **KIT-A10-F8** Know why action was taken against me | Get a clear statement of what was done, which rule applied and how to appeal. | core | P1 |
| **KIT-A10-F9** Have legal complaints handled on time | Have legal complaints, such as harmful digital communications notices, handled within the legal time limits. | core | P1, P2, P3 |
| **KIT-A10-F10** Be protected from known illegal content | Have known illegal images and videos blocked and reported as the law requires. | core | P1, P2 |
| **KIT-A10-F11** Be protected from spam | Have spam and message floods slowed down without blocking real people. | core | P1 |
| **KIT-A10-F12** Be protected from scams and fake accounts | Have scams, fake accounts and off-app payment tricks caught early. | core | P1, P2, P3, P4 |
| **KIT-A10-F13** Filter what I see by label | Have content labelled, including AI-made content, and choose what I see. | core | P1, P2 |
| **KIT-A10-F14** Know serious bans follow due process | Have a serious ban apply in every app, only for published reasons and with a right of appeal. | core | P1 |
| **KIT-A10-F15** Read transparency reports | Read regular reports on reports, removals, appeals and legal requests. | core | P1 |
| **KIT-A10-F16** Be checked on when something looks wrong | Get a check-in when a trip stops for a long time or leaves its route. | core | P1 |
| **KIT-A10-F17** Let safety groups flag content | Let recognised safety groups flag content for faster review. | later | P1 |
| **KIT-A10-F18** Report sexual harm safely | Report sexual harm to a trained person, stay in control of what happens next, and be referred to specialist support. | core | P1, P2 |
| **KIT-A10-F19** Be treated fairly whoever I am | Know every member, worker and customer alike, has agreed not to discriminate, and that patterns of discrimination are checked. | core | P1, P2 |
| **KIT-A10-F20** Trust that moderators are consistent and impartial | Know a sample of moderator and panel decisions gets a second review, and that moderators declare conflicts. | core | P1 |
| **KIT-A10-F21** Report copied or fake listings | Report a listing that copies my photos, misuses my trade mark or sells fake goods. | core | P1, P2 |
| **KIT-A10-F22** Know about emergencies where I work | See Civil Defence and weather warnings for my area, and have fees, ratings and dispatch paused there when disaster strikes. | core | P1, P2 |
| **KIT-A10-F23** Know about recalled products | Know when something listed or bought is on a product or food recall list. | next | P1, P2 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A10-F1-P1 | **Emergency button**: Calls the emergency number from the country pack (111 in NZ) and shows the address, vehicle and job details to read out, and still works with no data connection. | interface | small | KIT-A7-F4-P1, KIT-A19-F9-P1 |  |
| KIT-A10-F1-P2 | **Urgent safety lane**: The safety lane of the one urgent route, so urgent safety reports reach a trained person on the shared on-call roster at any hour. | service | medium | KIT-A21-F3-P1, KIT-A21-F2-P2 |  |
| KIT-A10-F1-P3 | **Helplines and 111 TXT**: Shows each country's helplines, such as 1737 in NZ, and guides Deaf and speech-impaired members to register for 111 TXT before they need it. | data | small | KIT-A19-F9-P1 |  |
| KIT-A10-F2-P1 | **Trusted contact link**: A time-limited link showing live progress, which ends with the job. | service | small | KIT-A7-F4-P1 |  |
| KIT-A10-F3-P1 | **Reports service**: Reports with categories, attachments and status, all as operations. | service | medium | kit-ops |  |
| KIT-A10-F3-P2 | **Report forms**: Short report forms in every app, reachable from anything that can be reported. | interface | small | KIT-A10-F3-P1, KIT-A20-F1-P1 |  |
| KIT-A10-F3-P3 | **Reports without an account**: A public form anyone can use without an account, such as neighbours, people shown in a listing and rights holders, with abuse limits on the shared limiter. | interface | small | KIT-A10-F3-P1, KIT-A22-F5-P4 |  |
| KIT-A10-F4-P1 | **Blocking**: Blocks that apply across chat, matching and feeds in every app. | service | small | KIT-A1-F1-P1, KIT-A6-F3-P2 |  |
| KIT-A10-F5-P1 | **Rules library**: Versioned rules for each app and the whole co-op, changed only by member vote. | service | small | KIT-A12-F4-P1 | [M04](../spare-social/STATUS.md) |
| KIT-A10-F6-P1 | **Moderation design**: Published rules, member-elected moderators, appeals, the public removal log and personal filters. | research | medium |  | [M04](../spare-social/STATUS.md) |
| KIT-A10-F6-P2 | **Review queue**: The one queue for reports, check reviews, rating challenges and appeals, with assignment, decisions and reasons, all as operations. | service | large | KIT-A10-F3-P1, kit-ops, KIT-A10-F6-P1 | [M04](../spare-social/STATUS.md) |
| KIT-A10-F6-P3 | **Moderator tools**: Review screens with context, history, blurring for distressing content and decision templates. | interface | medium | KIT-A10-F6-P2, KIT-A20-F1-P1, KIT-A25-F2-P1 |  |
| KIT-A10-F6-P4 | **Moderator wellbeing**: Exposure limits, breaks and support for people reviewing hard content. | service | small | KIT-A10-F6-P2 |  |
| KIT-A10-F7-P1 | **Public removal log**: Publishes each decision with its rule and no personal details, through the public reporting pipeline. | service | medium | KIT-A10-F6-P2, KIT-A13-F9-P1 | [M04](../spare-social/STATUS.md) |
| KIT-A10-F8-P1 | **Statements of reasons**: Sends the person affected the reason and an appeal link with every decision. | service | small | KIT-A10-F6-P2, KIT-A8-F3-P2 |  |
| KIT-A10-F9-P1 | **Legal duties for hosts**: What NZ and other countries require hosts to remove, how fast and by whom. | research | medium |  | [M02](../spare-social/STATUS.md) |
| KIT-A10-F9-P2 | **Legal notice handling**: Tracks legal complaints and counter-notices against their deadlines. | service | medium | KIT-A10-F6-P2, KIT-A10-F9-P1, KIT-A10-F9-P3 |  |
| KIT-A10-F9-P3 | **Law enforcement requests**: A published process for police and court requests, checked by a lawyer. | legal | medium |  |  |
| KIT-A10-F10-P1 | **Safety tooling**: Known-illegal-content matching, age checks and reporting, with licences and legal duties. | research | medium |  | [M07](../spare-social/STATUS.md) |
| KIT-A10-F10-P2 | **Known-content matching**: Checks uploads against approved hash lists and reports matches to the right agency. | integration | large | KIT-A15-F7-P1, KIT-A10-F10-P1 | [M07](../spare-social/STATUS.md) |
| KIT-A10-F11-P1 | **Spam limits**: Limits on new accounts' messages, links and listings, relaxed as trust grows, set as rules on the shared limiter. | library | small | KIT-A22-F5-P4, KIT-A1-F1-P1 |  |
| KIT-A10-F12-P1 | **Scam methods**: Proven methods other marketplaces use to stop scams, and what fits a co-op. | research | medium |  | [MK04](../spare-market/STATUS.md) |
| KIT-A10-F12-P2 | **Fraud signals**: The one fraud signals service for every app, such as new-account patterns, under published rules, which payment risk rules, scam warnings, fake review checks and faked location flags all feed and read. | service | large | KIT-A1-F1-P1, kit-ops, KIT-A10-F12-P1, KIT-A3-F13-P2, KIT-A7-F4-P3 | [MK04](../spare-market/STATUS.md) |
| KIT-A10-F12-P3 | **Reused photo checks**: Checks new listing and portfolio photos against photos already used elsewhere on Spareware, and sends any match to a person to review. Spare Rent, Spare Market and Spare Work need it. | service | medium | KIT-A10-F12-P2, KIT-A15-F7-P1, engine-media |  |
| KIT-A10-F12-P4 | **Scam guides**: Short plain advice on spotting common scams, shown where people need it in each app. Spare Rent and Spare Market both need it. | interface | small | KIT-A10-F12-P1, KIT-A21-F1-P1 |  |
| KIT-A10-F13-P1 | **Content labels**: Labels on posts and listings, set by authors, moderators and checks. | library | medium | KIT-A10-F6-P2 | [M09](../spare-social/STATUS.md) |
| KIT-A10-F13-P2 | **Personal filters**: Hides or blurs labelled content by each person's own choice. | library | small | KIT-A20-F1-P1 |  |
| KIT-A10-F14-P1 | **Restrictions across apps**: Restrictions that follow a person across apps only for serious, published reasons. | service | medium | KIT-A10-F6-P2, KIT-A1-F5-P2 |  |
| KIT-A10-F15-P1 | **Transparency reports**: Builds regular reports on reports, removals, appeals and legal requests from the operation log, through the public reporting pipeline. | service | small | KIT-A10-F7-P1, KIT-A13-F9-P1 |  |
| KIT-A10-F16-P1 | **Trip check-ins**: Spots long stops and route changes and asks whether all is well. | library | medium | KIT-A7-F4-P1, KIT-A7-F3-P2 |  |
| KIT-A10-F17-P1 | **Trusted flaggers**: A priority queue for recognised safety organisations. | service | small | KIT-A10-F6-P2 |  |
| KIT-A10-F18-P1 | **Sexual harm response design**: How specialist services and other platforms handle sexual harm reports: trauma-informed steps, responder training and referral partners in each country. | research | medium |  |  |
| KIT-A10-F18-P2 | **Sexual harm path**: A separate, trauma-informed report path handled only by trained responders, with referral to support and the reporter choosing whether police are told. | service | medium | KIT-A10-F18-P1, KIT-A10-F3-P1, KIT-A10-F1-P2, KIT-A10-F1-P3 |  |
| KIT-A10-F19-P1 | **Non-discrimination commitment**: A commitment every member accepts at sign-up under the Human Rights Act 1993, covering how workers treat customers and how customers treat workers. | legal | small |  |  |
| KIT-A10-F19-P2 | **Assistance animal rule**: One rule for every app that assistance dogs are always accepted and never charged as pets, checked by a lawyer under the Dog Control Act 1996 and the Human Rights Act 1993 (*unverified* per app). | legal | small | KIT-A10-F19-P1 |  |
| KIT-A10-F20-P1 | **Decision sampling and conflict checks**: Sends a random sample of decisions for a second review, publishes how often they agree, and checks moderators for conflicts of interest. | service | medium | KIT-A10-F6-P2, KIT-A11-F4-P1 |  |
| KIT-A10-F21-P1 | **Copyright and counterfeit rules**: What copyright and trade mark law ask of a host that receives notices, including the Copyright Act's rules for online hosts and counterfeit goods. | legal | small | KIT-A10-F9-P1 |  |
| KIT-A10-F21-P2 | **Copyright and trade mark notices**: Notices and counter-notices for copied photos, trade mark misuse and counterfeit listings, tracked on the legal notice deadlines. | service | small | KIT-A10-F21-P1, KIT-A10-F9-P2, KIT-A5-F1-P3 |  |
| KIT-A10-F22-P1 | **Official alerts**: Brings in Civil Defence emergency alerts and MetService warnings for each service area on each source's published terms (MetService's data terms *unverified*), and shows them to people there. | integration | medium | KIT-A7-F5-P1, KIT-A8-F3-P2 |  |
| KIT-A10-F22-P2 | **Disaster mode**: A switch for an affected zone that waives cancellation fees, pauses ratings and dispatch there and tells workers in the zone, with each use logged and reported. | service | medium | KIT-A10-F22-P1, KIT-A5-F6-P1, KIT-A9-F1-P1, KIT-A6-F3-P2, KIT-A22-F2-P2 |  |
| KIT-A10-F23-P1 | **Recall feeds**: Brings in government product and food recall lists, starting with New Zealand's, on each source's terms. Spare Market and Spare Food both need them. | integration | small | KIT-A16-F8-P1 |  |
| KIT-A10-F23-P2 | **Recall matching**: Matches recalls to live listings and past orders by product, brand and model, pulls matching listings and tells buyers. | service | small | KIT-A10-F23-P1, KIT-A6-F1-P2, KIT-A8-F3-P2 |  |

## KIT-A11 Disputes and appeals

When a job goes wrong the money stays held, both sides are heard, a panel of trained members decides, and either side can appeal. Built as svc-trust.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A11-F1** Open a dispute | Raise a problem with an order and have the payout paused until it is settled. | core | P1, P2 |
| **KIT-A11-F2** Add evidence | Add photos, messages and the job record to a dispute. | core | P1 |
| **KIT-A11-F3** Settle it between us first | Offer or accept a fix, such as a part refund, before a panel is needed. | core | P1, P2 |
| **KIT-A11-F4** Have a member panel decide | Have trained members, drawn at random, decide the dispute and give reasons. | core | P1, P2, P3, P4, P5 |
| **KIT-A11-F5** Appeal a decision | Appeal a dispute or moderation decision to a fresh panel. | core | P1 |
| **KIT-A11-F6** Know how long each step takes | See each step's deadline and get reminders. | core | P1 |
| **KIT-A11-F7** Take it further if needed | Learn how to take a dispute to a tribunal or regulator if I am not satisfied. | core | P1, P2 |
| **KIT-A11-F8** See how disputes are decided | See how many disputes each app had and how they were decided. | next | P1 |
| **KIT-A11-F9** Claim on insurance | Start an insurance claim from a dispute where an app carries cover. | core | P1, P2, P3 |
| **KIT-A11-F10** Be covered by a protection pool | Claim from a shared protection pool when a buyer, host or customer loses out and the rules cover it. | core | P1, P2 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A11-F1-P1 | **Disputes service**: Disputes linked to orders, with categories, deadlines and status. | service | medium | KIT-A5-F4-P1, KIT-A3-F2-P1 |  |
| KIT-A11-F1-P2 | **Dispute form**: Explains what can be disputed and asks only what is needed. | interface | small | KIT-A11-F1-P1, KIT-A20-F1-P1 |  |
| KIT-A11-F2-P1 | **Evidence store**: Gathers order records, chat, trip trace and uploads, with access kept to the parties and the panel. | service | medium | KIT-A11-F1-P1, KIT-A15-F7-P1 |  |
| KIT-A11-F3-P1 | **Settlement offers**: Offers and acceptances that move the held money straight away. | library | small | KIT-A11-F1-P1, KIT-A3-F5-P1 |  |
| KIT-A11-F3-P2 | **Mediation**: Offers mediation by a trained member, or an outside mediator at cost, before a panel decides. Spare Work and Spare Rent both map it. | service | small | KIT-A11-F3-P1, KIT-A11-F4-P3 |  |
| KIT-A11-F4-P1 | **Panel selection**: Draws eligible members at random, checks for conflicts of interest and keeps panels balanced. | service | medium | KIT-A11-F1-P1, KIT-A14-F2-P1, KIT-A11-F4-P5 |  |
| KIT-A11-F4-P2 | **Panel workspace**: Lets panel members read the evidence, discuss and record a decision. | interface | medium | KIT-A11-F4-P1, KIT-A11-F2-P1, KIT-A20-F1-P1, KIT-A8-F12-P1 |  |
| KIT-A11-F4-P3 | **Panel pay**: Pay for panel time, paid out at cost, with training and certificates from the shared training module. | service | small | KIT-A11-F4-P1, KIT-A3-F3-P2, KIT-A25-F2-P1, KIT-A11-F4-P4 |  |
| KIT-A11-F4-P4 | **Tax on member role pay**: How pay to panel members, moderators and ledger checkers is taxed and treated, such as honoraria under the schedular payment rules (*unverified*). | legal | small |  |  |
| KIT-A11-F4-P5 | **Legal standing of panel decisions**: What a panel decision binds and how it sits beside the Disputes Tribunal and Consumer Guarantees Act rights, given that a consumer is bound to arbitration only by a separate written agreement made after the dispute arises (Arbitration Act 1996 s 11). | legal | medium | KIT-A18-F8-P1 |  |
| KIT-A11-F5-P1 | **Appeals**: A second review by different members within set times, run through the one review queue. | service | small | KIT-A11-F4-P1, KIT-A10-F6-P2 |  |
| KIT-A11-F6-P1 | **Dispute clocks**: Deadlines for each step, with reminders and a set outcome if time runs out. | service | small | KIT-A11-F1-P1, KIT-A8-F3-P2, KIT-A19-F9-P2, KIT-A15-F14-P1 |  |
| KIT-A11-F7-P1 | **Outside routes**: Plain guidance on tribunals and regulators for each app and country. | data | small | KIT-A19-F6-P1 |  |
| KIT-A11-F7-P2 | **Evidence pack export**: Builds a dated evidence pack from the order, messages, photos and decisions as an accessible PDF for a tribunal, a regulator or an insurer. Spare Rent, Spare Market and Spare Work need it. | service | small | KIT-A11-F2-P1, KIT-A19-F11-P1, KIT-A27-F6-P3 |  |
| KIT-A11-F8-P1 | **Dispute statistics**: Published, anonymous counts and outcomes for each app, through the public reporting pipeline. | service | small | KIT-A11-F1-P1, KIT-A13-F9-P1 |  |
| KIT-A11-F9-P1 | **Insurance options**: Shared cover options across apps, such as a co-op pool or outside insurers, with costs. | research | medium |  | [ST03](../spare-stay/STATUS.md) |
| KIT-A11-F9-P2 | **Insurance claims link**: Passes dispute evidence to an insurer for apps that carry cover. | integration | medium | KIT-A11-F2-P1, KIT-A11-F9-P1 |  |
| KIT-A11-F9-P3 | **Income cover options**: Options for income cover beyond ACC, such as an optional group policy at cost, what each covers and what it costs. Spare Food and Spare Ride both map it. | research | small | KIT-A11-F9-P1, KIT-A23-F2-P4 |  |
| KIT-A11-F10-P1 | **Protection pool rules**: What the pool covers, how it is funded and how claims are decided, and whether a member-funded pool counts as insurance business under the Insurance (Prudential Supervision) Act 2010 (*unverified*). | legal | medium | KIT-A11-F9-P1, KIT-A3-F16-P1 |  |
| KIT-A11-F10-P2 | **Protection pool**: One pool service, its money held by the payment provider, with published rules, claims decided through disputes and its own ledger lines for money in and claims paid. | service | medium | KIT-A11-F10-P1, KIT-A11-F1-P1, KIT-A13-F1-P2, KIT-A3-F1-P2 |  |

## KIT-A12 Member voting and meetings

Members propose, discuss and vote on fees and rules for each app and for the whole co-op, elect directors, panels and moderators, follow decisions through, and hold meetings online, with a way to vote without a device. One member, one vote. Built as svc-voting.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A12-F1** Make a proposal | Propose a change to an app's fees or rules, with my reasons and the exact change. | core | P1, P2 |
| **KIT-A12-F2** Discuss before voting | Discuss a proposal and suggest amendments before the vote opens. | core | P1 |
| **KIT-A12-F3** Vote | Cast one vote on each proposal I am eligible for. | core | P1, P2, P3, P4 |
| **KIT-A12-F4** See results take effect | See the result, and have a passed change applied to the app automatically. | core | P1, P2, P3, P4 |
| **KIT-A12-F5** Know who votes on what | See the quorum, timing and who may vote for each kind of decision. | core | P1, P2 |
| **KIT-A12-F6** Elect people | Stand for or vote in elections for directors, panel members and moderators. | core | P1, P2, P3, P4 |
| **KIT-A12-F7** Attend the annual meeting online | Join the annual meeting online, follow the agenda and vote on motions. | next | P1, P2 |
| **KIT-A12-F8** Vote early or by proxy | Vote before a meeting, or appoint a proxy where the rules allow. | next | P1 |
| **KIT-A12-F9** Call a vote or meeting | Gather enough members to call a vote or a special meeting. | next | P1 |
| **KIT-A12-F10** Share out a budget together | Divide a budget between projects by member vote. | later | P1 |
| **KIT-A12-F11** Follow decisions through and answer surveys | See decisions that need work tracked to done, and answer consultations and surveys that do not bind. | core | P1, P2 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A12-F1-P1 | **Proposals service**: Proposals with text, the exact settings change, supporters and status. | service | large | kit-ops, KIT-A14-F2-P1 | [K01](STATUS.md) |
| KIT-A12-F1-P2 | **Proposal editor**: Write a proposal, preview its effect and gather supporters. | interface | medium | KIT-A12-F1-P1, KIT-A20-F1-P1 |  |
| KIT-A12-F2-P1 | **Discussion and amendments**: Threaded discussion and amendments linked to a proposal. | service | medium | KIT-A12-F1-P1, KIT-A8-F12-P1 |  |
| KIT-A12-F3-P1 | **Ballots**: One member, one vote, with eligibility from the member register and each app's electorate. | service | large | KIT-A12-F1-P1, KIT-A14-F2-P1, KIT-A12-F3-P3 | [K01](STATUS.md) |
| KIT-A12-F3-P2 | **Voting screen**: A plain voting screen with the proposal summary and the effect of each choice. | interface | small | KIT-A12-F3-P1, KIT-A20-F1-P1 |  |
| KIT-A12-F3-P3 | **Secret and checkable votes**: How to keep each vote secret while letting members check the count. | research | medium |  |  |
| KIT-A12-F3-P4 | **Vote without a device**: Postal or phone voting for members without a device, counted with the app votes and checked against the register. | service | medium | KIT-A12-F3-P1 |  |
| KIT-A12-F4-P1 | **Results to settings**: Turns a passed vote into the operation that changes the app's fees or rules. | service | medium | KIT-A12-F3-P1, kit-ops, KIT-A12-F4-P3, KIT-A12-F4-P4 |  |
| KIT-A12-F4-P2 | **Results page**: Turnout, quorum, the result and exactly what changed. | interface | small | KIT-A12-F3-P1, KIT-A20-F1-P1 |  |
| KIT-A12-F4-P3 | **Competition law check**: Whether worker-members who are contractors may vote together on fares, prices or fees without making a cartel provision under the Commerce Act, and whether a collective bargaining route proposed in late 2025 now applies (*unverified*). | legal | medium |  |  |
| KIT-A12-F4-P4 | **Legal duty locks**: Marks settings that carry a legal duty, such as a required check, GST collection or a notice period, so a passed vote cannot switch them off, and sends any proposal that touches them for legal review before the vote opens. | library | small | KIT-A12-F1-P1 |  |
| KIT-A12-F5-P1 | **Voting rules in the constitution**: Quorum, notice periods and electorates written into the constitution and rules. | legal | medium |  | [K01](STATUS.md) |
| KIT-A12-F5-P2 | **Electorates**: Who votes on what: each app's workers, each app's customer-members, every member, or the board, with what customer-members vote on stated for each app. | data | small | KIT-A14-F2-P1, KIT-A12-F5-P1 |  |
| KIT-A12-F6-P1 | **Elections**: Nominations, candidate statements and ranked voting. | service | medium | KIT-A12-F3-P1 |  |
| KIT-A12-F6-P2 | **Independent count**: An independent returning officer or scrutineer for board elections, and a published audit of each count. | service | small | KIT-A12-F6-P1, KIT-A12-F3-P3 |  |
| KIT-A12-F6-P3 | **Candidate checks and consent**: Checks that each board candidate is not disqualified under the Companies Act 1993 and holds their written consent before the election opens. | service | small | KIT-A12-F6-P1, KIT-A14-F1-P1 |  |
| KIT-A12-F6-P4 | **Election challenges**: A route to challenge an election result within a set time, decided by the independent returning officer or as the constitution sets. | service | small | KIT-A12-F6-P2, KIT-A12-F5-P1 |  |
| KIT-A12-F7-P1 | **Meetings**: Notices, agendas, motions, attendance and minutes. | service | medium | KIT-A12-F3-P1, KIT-A8-F3-P2, KIT-A19-F9-P2 |  |
| KIT-A12-F7-P2 | **Meeting video**: Live video for meetings on the kit's own call service. | integration | medium | KIT-A8-F11-P2, KIT-A12-F7-P1 |  |
| KIT-A12-F8-P1 | **Early and proxy votes**: Records early votes and proxies under the constitution's rules. | service | small | KIT-A12-F3-P1 |  |
| KIT-A12-F9-P1 | **Member petitions**: Collects support towards the number that calls a vote or meeting. | service | small | KIT-A12-F1-P1 |  |
| KIT-A12-F10-P1 | **Budget votes**: Spending choices voted on with points or ranked choices. | service | medium | KIT-A12-F3-P1 |  |
| KIT-A12-F11-P1 | **Decision tracker**: Passed decisions that need work rather than a settings change, such as building something or changing a supplier, with an owner, progress and a report back. | service | medium | KIT-A12-F4-P1 |  |
| KIT-A12-F11-P2 | **Consultations and surveys**: Non-binding questions and surveys to members, with results published and kept apart from binding votes. | service | small | KIT-A12-F1-P1, KIT-A14-F2-P1, KIT-A8-F12-P1 |  |

## KIT-A13 Open cost ledger

Every month each app publishes what it cost to run, what the fee paid for and how the next fee is set, checked and signed off by members. It also keeps the co-op's own books and runs one pipeline for every public report. Built as svc-ledger.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A13-F1** See what each app cost | See each app's costs for the month, line by line. | core | P1, P2, P3, P4 |
| **KIT-A13-F2** Have bills brought in without retyping | Have hosting, provider and insurance bills brought into the ledger. | core | P1, P2, P3, P4 |
| **KIT-A13-F3** See how the fee is set | See how last period's costs divided by its jobs became this period's fee. | core | P1, P2, P3, P4, P5, P6, P7 |
| **KIT-A13-F4** Check and sign off a month | As a member chosen for the job, check the month's figures and sign them off. | core | P1 |
| **KIT-A13-F5** Download the ledger | Download the ledger as open data. | core | P1 |
| **KIT-A13-F6** Ask about a cost | Ask a question about any cost line and see the answer. | next | P1 |
| **KIT-A13-F7** See uptime next to the costs | See uptime and outages next to what was spent. | next | P1 |
| **KIT-A13-F8** See next period's budget | See expected costs and the planned fee before a period starts. | later | P1 |
| **KIT-A13-F9** Find every public report in one place | Find the ledger, removals, disputes, support figures and fairness checks published together as open data. | core | P1, P2 |
| **KIT-A13-F10** Claim expenses | As a member or volunteer, claim an expense with a receipt and be paid back once it is approved. | next | P1 |
| **KIT-A13-F11** Keep the co-op's own books | Trust that the co-op keeps proper books, pays its staff and suppliers, and has its yearly accounts audited. | core | P1, P2, P3, P4 |
| **KIT-A13-F12** Fund the start | Give a donation, a grant or a member loan to fund the co-op before fees cover its costs, and see how it was spent. | core | P1, P2 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A13-F1-P1 | **Ledger design**: What the ledger publishes and how it is checked, with a sample month and who signs it off. | research | small |  | [K06](STATUS.md) |
| KIT-A13-F1-P2 | **Ledger service**: Cost entries for each app and period, as operations linked to the bills behind them. | service | large | kit-ops, KIT-A13-F1-P1, KIT-A13-F1-P4 | [K06](STATUS.md) |
| KIT-A13-F1-P3 | **Ledger pages**: Public pages and charts for each app and month. | interface | medium | KIT-A13-F1-P2, KIT-A20-F1-P1 |  |
| KIT-A13-F1-P4 | **Ledger privacy rules**: Groups staff pay so no one's salary shows, and handles supplier terms that forbid publishing prices, while every dollar still adds up. | legal | small |  | [K06](STATUS.md) |
| KIT-A13-F2-P1 | **Bill imports**: Pulls bills from hosting and provider accounts, with manual entry for the rest. | integration | medium | KIT-A13-F1-P2 |  |
| KIT-A13-F2-P2 | **Usage metering**: Measures each app's use of shared servers so shared costs are split fairly. | service | medium | KIT-A13-F1-P2, KIT-A22-F1-P1 |  |
| KIT-A13-F2-P3 | **Per-account metering**: Measures each person's and organisation's use of storage, mail, meeting minutes, AI compute and transfer, beside the per-app metering, for apps that price by use. | service | medium | KIT-A13-F2-P2, kit-ops |  |
| KIT-A13-F2-P4 | **Shared cost split rule**: A published, member-voted rule for splitting costs that cannot be metered, such as staff, legal, board and audit, between apps. | library | small | KIT-A13-F2-P2, KIT-A12-F4-P1 |  |
| KIT-A13-F3-P1 | **Fee setting**: Works out each app's next fee from its costs, its jobs, the reserve line and any surplus carried forward. | library | medium | KIT-A13-F1-P2, KIT-A13-F2-P2, KIT-A13-F3-P5, KIT-A13-F2-P4 |  |
| KIT-A13-F3-P2 | **Fee explainer**: Shows how the period's fee was set, with last period's costs, jobs, reserve and any surplus as real figures, linked from each job's fee breakdown. | interface | small | KIT-A13-F3-P1, KIT-A20-F1-P1 |  |
| KIT-A13-F3-P3 | **Launch cost estimate**: What the kit and the first app will cost to run each month at launch, line by line, as the basis for the first fee. | research | small | KIT-A22-F4-P2 | [K06](STATUS.md) |
| KIT-A13-F3-P4 | **Start-up fee rule**: Sets the fee for the first period, when there is no last period, from the launch estimate, and trues it up once real costs are known. | library | small | KIT-A13-F3-P3, KIT-A13-F3-P1 |  |
| KIT-A13-F3-P5 | **Reserve line**: A published reserve for chargeback losses, legal costs and insurance excess, set by member vote and shown as its own cost line. | library | small | KIT-A13-F1-P2, KIT-A3-F16-P1, KIT-A12-F4-P1 |  |
| KIT-A13-F3-P6 | **Prices by use**: Works out at-cost prices per unit of use, such as a gigabyte stored or a meeting minute, from the ledger's costs, for apps that have no jobs to divide by. | library | medium | KIT-A13-F3-P1, KIT-A13-F2-P3, KIT-A13-F3-P7 |  |
| KIT-A13-F3-P7 | **Free allowances and hardship rule**: Free allowances set by member vote and a published rule for people who cannot pay, with the cost of both shown as a ledger line. | library | small | KIT-A12-F4-P1, KIT-A13-F1-P2 |  |
| KIT-A13-F4-P1 | **Sign-off workflow**: Two or more chosen members review, question and sign each month. | service | medium | KIT-A13-F1-P2, KIT-A14-F2-P1 | [K06](STATUS.md) |
| KIT-A13-F5-P1 | **Open data export**: Publishes each month in CSV and JSON with a stable layout. | service | small | KIT-A13-F1-P2 |  |
| KIT-A13-F6-P1 | **Ledger questions**: Questions and answers attached to ledger lines. | service | small | KIT-A13-F1-P2, KIT-A8-F12-P1 |  |
| KIT-A13-F7-P1 | **Uptime in the ledger**: Adds uptime and incident summaries to each month. | service | small | KIT-A13-F1-P2, KIT-A21-F5-P1 |  |
| KIT-A13-F8-P1 | **Budgets and forecasts**: Expected costs and fees, compared with the actual figures afterwards. | library | medium | KIT-A13-F1-P2 |  |
| KIT-A13-F9-P1 | **Public reporting pipeline**: One pipeline that builds every public report from the operation log, strips personal details and publishes them with the ledger as open data. | service | medium | KIT-A13-F5-P1, KIT-A15-F1-P2, KIT-A13-F9-P2 |  |
| KIT-A13-F9-P2 | **Small-number suppression**: Hides or groups any count in a public report small enough to identify a person, such as one app, place and month, under one published minimum. | library | small |  |  |
| KIT-A13-F10-P1 | **Expense claims**: Claims with receipts, approval by someone other than the claimant, and repayment, each shown in the ledger. | service | medium | KIT-A13-F11-P2, KIT-A15-F7-P1 |  |
| KIT-A13-F11-P1 | **Bookkeeping system choice**: Compares existing accounting systems on NZ GST, bank feeds, payroll links, licence and cost, so the co-op uses one rather than building its own. | research | small |  |  |
| KIT-A13-F11-P2 | **Books link**: Feeds ledger costs, fee income and payouts owed into the co-op's accounts and reconciles them with the bank. | integration | medium | KIT-A13-F11-P1, KIT-A13-F1-P2, KIT-A3-F9-P1 |  |
| KIT-A13-F11-P3 | **Supplier bills and staff pay**: Approves and pays supplier bills, and pays staff through the one payroll provider link, each payment an operation with two people signing off. | integration | medium | KIT-A13-F11-P2, KIT-A22-F5-P1, KIT-A23-F2-P3 |  |
| KIT-A13-F11-P4 | **Yearly accounts and audit**: The financial statements and audit the law and the constitution require, with the auditor chosen by members. | legal | small | KIT-A13-F11-P2 |  |
| KIT-A13-F12-P1 | **Funding rules**: How the co-op may take donations, grants and member loans, including securities law for member loans, tax on donations and anti-money-laundering checks. | legal | medium | KIT-A14-F1-P1, KIT-A4-F5-P1 |  |
| KIT-A13-F12-P2 | **Funding records**: Takes donations, grants and member loans through the payment provider, records each on the ledger with its terms, and shows how it was spent and what is owed back. | service | medium | KIT-A13-F12-P1, KIT-A13-F1-P2, KIT-A3-F1-P2 |  |

## KIT-A14 Co-op membership and member portal

The co-op's own home: joining once per person, the share register and the co-operative test, member roles, rebates of any surplus, reports, the constitution, discipline, board work, insurance, contributors' rights, the steward society and the federation of local co-ops. Proposed as svc-members (KIT-A16-F10-P1); the member register feeds svc-voting.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A14-F1** Join the co-op | Become a member by buying one $1 share and agreeing to the constitution. | core | P1, P2, P3, P4, P5 |
| **KIT-A14-F2** Be on the share register | Know my membership and share are recorded correctly. | core | P1, P2, P3, P4 |
| **KIT-A14-F3** See my membership at a glance | See my open votes, roles, earnings and co-op news in one place. | core | P1 |
| **KIT-A14-F4** Read the constitution and reports | Read the constitution, rules, minutes and annual reports, with past versions. | core | P1, P2 |
| **KIT-A14-F5** Share in any surplus | See my share of any surplus, returned as a rebate or as lower fees. | next | P1, P2, P3 |
| **KIT-A14-F6** Take on a member role | Volunteer as a panel member, moderator or ledger checker, and get trained. | core | P1 |
| **KIT-A14-F7** Invite people | Invite friends or workers to join with a personal link. | core | P1 |
| **KIT-A14-F8** Leave the co-op | Leave and have my share paid back under the constitution. | core | P1 |
| **KIT-A14-F9** Find other members | Find members who chose to be listed, by app or place. | next | P1 |
| **KIT-A14-F10** Read what other co-ops learned | Read the study of other platform co-ops and see which of its lessons shaped the kit's rules. | core | P1 |
| **KIT-A14-F11** Run the board in the open | As a director, keep board papers, minutes and interests in one place members can check. | next | P1 |
| **KIT-A14-F12** Start a co-op in another country | Start or join a local co-op in another country under the shared name and kit. | later | P1, P2 |
| **KIT-A14-F13** See contributors credited | See contributions to the code credited to the members who made them. | later | P1 |
| **KIT-A14-F14** Have a fair hearing before suspension or expulsion | Know a member is suspended or expelled only under the constitution, after a hearing, and separately from any app ban. | next | P1, P2 |
| **KIT-A14-F15** Run the steward society | As a member of the steward society, see its register, committee, annual meeting, accounts and disputes procedure. | next | P1, P2 |
| **KIT-A14-F16** Contribute with clear rights | Sign off my contributions so the co-op can use them, with my employer's consent where I worked on their time. | core | P1, P2 |
| **KIT-A14-F17** Know the co-op is insured | Know the co-op and its board carry the insurance they need, with the cost in the ledger. | core | P1 |
| **KIT-A14-F18** Know the name is protected | Know Spareware's names, domains and accounts are held by the co-op's steward, not by any one person. | core | P1, P2 |
| **KIT-A14-F19** Know what happens if an app or the co-op closes | Know I will get notice, my data and any money owed if an app or the co-op closes. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A14-F1-P1 | **Legal entity**: The legal form of the operating co-op and of the steward that holds the code and name. | legal | medium |  | [K01](STATUS.md) |
| KIT-A14-F1-P2 | **Member share offer**: Confirms $1 member shares can be offered without a full disclosure document. | legal | small |  | [K01](STATUS.md) |
| KIT-A14-F1-P3 | **Join flow**: Explains membership, takes the $1 share payment and records acceptance of the constitution. | interface | medium | KIT-A1-F1-P1, KIT-A3-F1-P3, KIT-A20-F1-P1, KIT-A14-F1-P1, KIT-A14-F1-P2, KIT-A14-F1-P4 |  |
| KIT-A14-F1-P4 | **One person, one membership**: An identity check and a duplicate check before a share is issued, so no one holds two memberships or two votes. | service | small | KIT-A2-F1-P2, KIT-A2-F1-P3, KIT-A14-F2-P1 |  |
| KIT-A14-F1-P5 | **Business members**: Restaurants, shops and trade firms join as business members under the constitution's member classes, with a check that the business is real before a share is issued. | service | small | KIT-A14-F1-P1, KIT-A14-F2-P1, KIT-A2-F7-P1, KIT-A1-F7-P1 |  |
| KIT-A14-F2-P1 | **Member register**: The legal share register and member classes for each app, every change an operation. | service | large | kit-ops, KIT-A1-F1-P1 | [K01](STATUS.md) |
| KIT-A14-F2-P2 | **Membership certificate**: A member number and a downloadable certificate. | service | small | KIT-A14-F2-P1, engine-pdf |  |
| KIT-A14-F2-P3 | **Co-operative test**: Tracks that transacting shareholders hold at least 60% of voting rights, as the Co-operative Companies Act 1996 requires, and flags members who stop trading. | service | small | KIT-A14-F2-P1, KIT-A5-F4-P1, KIT-A3-F3-P2 |  |
| KIT-A14-F2-P4 | **Register inspection**: Handles requests to inspect the share register under company law (*unverified* scope for co-operative companies) without showing members' home addresses beyond what the law requires. | legal | small | KIT-A14-F2-P1 |  |
| KIT-A14-F3-P1 | **Member portal home**: One page with open votes, my roles, my apps, ledger news and meetings, in Spareware red. | interface | large | KIT-A14-F2-P1, KIT-A12-F3-P1, KIT-A13-F1-P2, KIT-A20-F1-P1 |  |
| KIT-A14-F4-P1 | **Co-op documents library**: The versioned constitution, rules, minutes and reports. | service | small | engine-pdf |  |
| KIT-A14-F4-P2 | **Yearly filings**: Annual returns and statements to the Companies Office and regulators, using the audited yearly accounts. | legal | small | KIT-A14-F2-P1, KIT-A13-F11-P4 |  |
| KIT-A14-F5-P1 | **Trade records**: Records each member's trade with the co-op, as the basis for any rebate. | service | medium | KIT-A14-F2-P1, KIT-A3-F3-P2, KIT-A14-F5-P3 |  |
| KIT-A14-F5-P2 | **Rebates**: Works out and pays rebates once members have voted on them. | service | medium | KIT-A14-F5-P1, KIT-A3-F3-P2, KIT-A12-F3-P1 |  |
| KIT-A14-F5-P3 | **Rebate tax**: How rebates are taxed and reported for each member. | legal | small |  | [K01](STATUS.md) |
| KIT-A14-F6-P1 | **Member roles**: Role sign-up, terms and conflict of interest declarations, with training and certificates from the shared training module. | service | medium | KIT-A14-F2-P1, KIT-A1-F5-P2, KIT-A25-F2-P1, KIT-A11-F4-P4 |  |
| KIT-A14-F7-P1 | **Invites**: Personal invite links that count joins without tracking anyone elsewhere. | service | small | KIT-A14-F2-P1 |  |
| KIT-A14-F8-P1 | **Leaving and share redemption**: Ends a membership and redeems the share, keeping the records the law requires. | service | small | KIT-A14-F2-P1, KIT-A3-F5-P1 |  |
| KIT-A14-F9-P1 | **Member directory**: An opt-in directory showing only what each member chooses. | interface | small | KIT-A14-F2-P1, KIT-A1-F5-P1 |  |
| KIT-A14-F10-P1 | **Platform co-op study**: What worked and failed for ride, courier and holiday rental co-ops, with current status. | research | medium |  | [K03](STATUS.md) |
| KIT-A14-F11-P1 | **Board area**: Board papers, minutes, an interests register and resolutions, with board meetings run on the shared meetings part. | service | medium | KIT-A14-F2-P1, KIT-A12-F7-P1 |  |
| KIT-A14-F12-P1 | **Name licence for local co-ops**: The steward's licence of the Spareware name to co-ops that keep the rules. | legal | medium |  | [K01](STATUS.md) |
| KIT-A14-F12-P2 | **Federation register**: Local co-ops, their regions, members and apps. | service | medium | KIT-A14-F2-P1, KIT-A14-F12-P1 |  |
| KIT-A14-F13-P1 | **Contributor records**: Links reviewed contributions to members who choose to show them. | integration | small | KIT-A14-F2-P1 |  |
| KIT-A14-F14-P1 | **Member discipline rules**: The constitution's grounds, notice, hearing and appeal for suspending or expelling a member. | legal | small | KIT-A14-F1-P1 | [K01](STATUS.md) |
| KIT-A14-F14-P2 | **Suspension and expulsion**: Notice, hearing, decision and appeal recorded against the register, kept apart from app bans and restrictions. | service | medium | KIT-A14-F14-P1, KIT-A14-F2-P1 |  |
| KIT-A14-F15-P1 | **Steward society rules**: The constitution, committee, disputes procedure and yearly duties the Incorporated Societies Act 2022 requires of the steward. | legal | small | KIT-A14-F1-P1 | [K01](STATUS.md) |
| KIT-A14-F15-P2 | **Steward register and meetings**: The society's own member register, committee records, annual meeting and financial statements, reusing the co-op's register, meetings and books. | service | medium | KIT-A14-F15-P1, KIT-A14-F2-P1, KIT-A12-F7-P1, KIT-A13-F11-P2 |  |
| KIT-A14-F16-P1 | **Contributor sign-off**: A developer certificate of origin or a contributor licence agreement, and employer consent for work done on an employer's time. | legal | small | KIT-A16-F4-P1 | [A14](../spare-construct/STATUS.md) |
| KIT-A14-F16-P2 | **Sign-off check**: A repository check that every contribution carries the sign-off before it is merged. | service | small | KIT-A14-F16-P1, KIT-A22-F2-P1 |  |
| KIT-A14-F17-P1 | **Co-op insurance**: Directors and officers, cyber and public liability cover for the co-op across all apps, compared on cost and what each covers. | research | medium |  |  |
| KIT-A14-F18-P1 | **Trade marks**: Registers Spareware and the product names as trade marks, in NZ first, held by the steward and licensed to local co-ops. | legal | small | KIT-A14-F1-P1 |  |
| KIT-A14-F18-P2 | **Domains held by the steward**: The steward holds every Spareware domain under two-person control, so no single person can lose or take one. | legal | small | KIT-A14-F1-P1, KIT-A22-F5-P1 |  |
| KIT-A14-F19-P1 | **Wind-down plan**: Notice periods, a data export window, payouts and held money settled, and code and data passed to the steward or members when an app or the co-op closes. | legal | medium | KIT-A14-F1-P1, KIT-A18-F1-P1, KIT-A3-F2-P2 |  |

## KIT-A15 Sync, history and storage

The base every app is built on (kit-core, kit-ops, kit-sync, kit-store): permanent IDs, every change as an operation with versioned schemas, personal data that can still be erased, undo, history, offline work, live co-editing, timers that survive restarts, files, previews, shared libraries, a document vault and sharing. Files are served by a file service proposed as svc-files (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A15-F1** Have every change go through one checked path | Have every change, by a person, an AI agent or an add-in, checked and recorded the same way. | core | P1, P2, P3, P4, P5, P6, P7, P8, P9 |
| **KIT-A15-F2** Undo and redo | Undo and redo my own changes, even in shared work. | core | P1 |
| **KIT-A15-F3** See history | See who changed what and when, and go back to any earlier version. | core | P1 |
| **KIT-A15-F4** Work offline | Keep working without a connection and have it sync when I am back. | core | P1, P2, P3, P4, P5 |
| **KIT-A15-F5** See changes live | See other people's changes appear as they make them. | core | P1, P2 |
| **KIT-A15-F6** Open big work quickly | Open large design files fast, loading only what I am working on. | core | P1, P2 |
| **KIT-A15-F7** Add photos and files | Add photos and files to listings, chats and documents. | core | P1, P2, P3, P4, P5 |
| **KIT-A15-F8** Share with people | Share a file or item with chosen people or a link, and change access at any time. | core | P1, P2 |
| **KIT-A15-F9** Get deleted things back | Restore deleted items for a set time before they are gone for good. | core | P1 |
| **KIT-A15-F10** Co-edit text | Type in the same document as others with no lost work. | core | P1 |
| **KIT-A15-F11** Branch and merge | Try changes on a branch and merge them back. | next | P1 |
| **KIT-A15-F12** Keep my files private | Have files encrypted so only the people I share with can read them. | next | P1 |
| **KIT-A15-F13** Keep my documents in one place | Keep the documents for an order, a property or an organisation in one place, with versions, dates and expiry, and share them with the people who need them. | core | P1 |
| **KIT-A15-F14** Have things happen on time | Have auto-confirms, deadlines, reminders and scheduled bookings happen on time, even after a server restarts. | core | P1 |
| **KIT-A15-F15** Sync my folders with my computer | Keep chosen folders on my computer in sync with my Spareware storage. | core | P1 |
| **KIT-A15-F16** See a preview of any file | See a thumbnail or quick preview of any photo, video, document or model without opening its app. | core | P1 |
| **KIT-A15-F17** Show work to someone with no app | Send someone a link to view my work in a browser, with no account or install. | next | P1 |
| **KIT-A15-F18** Use shared libraries and templates | Use libraries of reusable content and templates that my team or the co-op shares. | core | P1, P2, P3 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A15-F1-P1 | **Permanent IDs and core types**: 128-bit time-ordered IDs, units, errors and time types, so offline work never collides; money uses the one shared money type. | library | medium |  |  |
| KIT-A15-F1-P2 | **Commands and operations**: Commands, checks, transactions and the operation log. | library | large | KIT-A15-F1-P1, KIT-A15-F1-P5, KIT-A15-F1-P9 |  |
| KIT-A15-F1-P3 | **Command schemas**: Each app's commands described once, so the interface, API, plugins and AI share them. | library | medium | KIT-A15-F1-P2 |  |
| KIT-A15-F1-P4 | **Permission checks**: Checks every operation against who is acting and their roles. | library | medium | KIT-A15-F1-P2, KIT-A1-F5-P2 |  |
| KIT-A15-F1-P5 | **Personal data in a permanent log**: How to erase personal data from an append-only, tamper-evident operation log, such as keeping it outside operations or encrypting it per person and destroying the key. | research | medium |  |  |
| KIT-A15-F1-P6 | **Erasable personal data**: Builds the chosen method, so deletion, correction and retention work while replay and the financial log stay intact. | library | large | KIT-A15-F1-P5, KIT-A15-F1-P2, KIT-A22-F5-P5 |  |
| KIT-A15-F1-P7 | **Operation versions and upgrades**: A version on every operation schema, upgraders that read old operations as new ones, and migration tests run against recorded history. | library | medium | KIT-A15-F1-P3 |  |
| KIT-A15-F1-P8 | **Snapshots**: Checked snapshots of state that cap how much history must be replayed to open an app or rebuild a service. | library | medium | KIT-A15-F1-P2, KIT-A15-F1-P7 |  |
| KIT-A15-F1-P9 | **Idempotency keys**: Every command carries a unique key, so a command retried or replayed from an offline queue, an agent, a script or a webhook never books or charges twice, the pattern Stripe's API uses. | library | small | KIT-A15-F1-P1 |  |
| KIT-A15-F2-P1 | **Undo and redo**: Undo by inverse operations, kept for each person in shared work. | library | medium | KIT-A15-F1-P2 |  |
| KIT-A15-F3-P1 | **Version history**: Browse, compare and restore versions from the operation log. | library | medium | KIT-A15-F1-P2, KIT-A15-F1-P8 |  |
| KIT-A15-F4-P1 | **Sync method**: Server-ordered operation log or CRDT, covering offline work, conflicts and deterministic replay. | research | medium |  | [A05](../spare-construct/STATUS.md) |
| KIT-A15-F4-P2 | **Sync engine**: A server-ordered operation log with offline queues and deterministic replay. | library | large | KIT-A15-F1-P2, KIT-A15-F4-P1 | [A05](../spare-construct/STATUS.md) |
| KIT-A15-F4-P3 | **Conflict handling**: Clear rules, and a plain prompt, when offline changes clash. | library | medium | KIT-A15-F4-P2 |  |
| KIT-A15-F4-P4 | **Protected offline data**: Keeps offline data on phones and computers encrypted, and wipes it when the device is signed out remotely. | library | small | KIT-A15-F4-P2, KIT-A1-F3-P2, KIT-A22-F5-P5 |  |
| KIT-A15-F4-P5 | **Autosave and crash recovery**: Saves work to a local journal as it happens and replays it after a crash or power cut, so a file opens exactly where it was. Spare Studio and Spare Office both need it. | library | medium | KIT-A15-F4-P2, KIT-A15-F1-P8 |  |
| KIT-A15-F5-P1 | **Live updates**: Pushes new operations to every open device. | service | medium | KIT-A15-F4-P2 |  |
| KIT-A15-F5-P2 | **Presence**: Shows who else is here and what they are working on. | library | medium | KIT-A15-F4-P2 |  |
| KIT-A15-F6-P1 | **Storage choices**: Entity store and project file format, covering chunked storage and permanent IDs. | research | medium |  | [A02](../spare-construct/STATUS.md) |
| KIT-A15-F6-P2 | **Chunked storage**: Chunks, paging from disk and a spatial index, in a crash-safe container file. | library | large | KIT-A15-F1-P1, KIT-A15-F6-P1 | [A06](../spare-construct/STATUS.md) |
| KIT-A15-F7-P1 | **File uploads**: Resumable uploads, safety scanning, image sizes and content addressing, kept in object storage by the one file service. | service | large | KIT-A15-F1-P1, KIT-A22-F4-P1 |  |
| KIT-A15-F7-P2 | **Storage at cost**: Object storage with quotas, and at-cost prices for heavy use. | service | medium | KIT-A15-F7-P1, KIT-A13-F2-P2 |  |
| KIT-A15-F7-P3 | **Photo metadata stripping**: Removes location and other hidden details from uploaded photos, keeping only what the person chooses to share. | library | small | KIT-A15-F7-P1 |  |
| KIT-A15-F7-P4 | **Photo privacy check**: Flags faces, number plates, mail, valuables, alarm panels and keys in photos, without identifying anyone, so the person can blur them before the photos go live. | service | medium | KIT-A15-F7-P1, engine-media |  |
| KIT-A15-F7-P5 | **When storage lapses**: Over quota or after non-payment, files turn read-only after long notice, and nothing is deleted without repeated warnings and a window to export. | library | small | KIT-A15-F7-P2, KIT-A8-F3-P2, KIT-A18-F1-P1 |  |
| KIT-A15-F8-P1 | **Sharing and access**: Sharing with people or by link, at view, comment or edit level. | library | medium | KIT-A15-F1-P4 |  |
| KIT-A15-F8-P2 | **Guest access by private link**: Lets someone with no account, such as a tenant, guarantor or outside guest, open just what was shared with them through a private link and a one-time code, to read, sign or answer. Spare Office, Spare Home and Spare Rent need it. | service | small | KIT-A15-F8-P1, KIT-A1-F2-P3 |  |
| KIT-A15-F9-P1 | **Recycle bin**: Keeps deleted items for a set time and restores them on request. | library | small | KIT-A15-F1-P2 |  |
| KIT-A15-F10-P1 | **Text co-editing**: A text CRDT on the operation log for documents and long text. | library | large | KIT-A15-F4-P2 | [OF03](../spare-office/STATUS.md) |
| KIT-A15-F11-P1 | **Branches**: Branches and merges of operation logs, used most by the design tools. | library | large | KIT-A15-F4-P2 |  |
| KIT-A15-F12-P1 | **Encryption choice**: Where to encrypt at rest and where end to end, and what that costs search and recovery. | research | medium |  |  |
| KIT-A15-F13-P1 | **Document vault**: Typed documents with versions, dates, expiry reminders and sharing, attached to an order, a property or an organisation, on the one file service. | service | medium | KIT-A15-F7-P1, KIT-A15-F8-P1, KIT-A15-F3-P1, KIT-A15-F14-P1 |  |
| KIT-A15-F14-P1 | **Durable timers**: Timers and scheduled jobs stored as operations, so auto-confirm, dispute clocks, re-checks, scheduled bookings, retention deletion, card hold renewals and payout runs survive restarts and fire once. | library | medium | KIT-A15-F1-P2, KIT-A15-F1-P9 |  |
| KIT-A15-F15-P1 | **Desktop sync client**: One desktop client for every program that syncs chosen folders with a person's Spareware storage, handling conflicts and offline changes. | library | large | kit-sync, kit-store, KIT-A15-F7-P2, KIT-A1-F6-P1 |  |
| KIT-A15-F16-P1 | **Preview service**: Makes thumbnails and previews for every supported format, such as photos, video, PDF, office files and 3D, in a sandbox, using the shared engines. Spare Office, Spare Studio, Spare Social, Spare OS and Spare Construct need the same service. | service | medium | KIT-A15-F7-P1, engine-media, engine-pdf, engine-render |  |
| KIT-A15-F17-P1 | **Web viewer**: A read-only browser viewer for PDFs, images, video, audio, layouts, sheets and 3D, with no account or install. Spare Construct and Spare Studio both need it. | interface | medium | KIT-A15-F16-P1, KIT-A15-F8-P1, engine-render |  |
| KIT-A15-F18-P1 | **Content library service**: One versioned library of reusable content, such as families, materials, fonts, brand kits and templates, loaded by reference, with organisation and public collections, stored on the file service and metered to the ledger. Spare Construct and Spare Studio both need it. | service | large | KIT-A15-F7-P1, KIT-A15-F3-P1, KIT-A13-F2-P2, KIT-A15-F8-P1 |  |
| KIT-A15-F18-P2 | **Template gallery**: Co-op-made templates kept as normal files with fill-in fields and listed in a gallery, for documents, slides, posters and social posts. Spare Studio and Spare Office both need it. | interface | small | KIT-A15-F18-P1, KIT-A20-F1-P1 |  |
| KIT-A15-F18-P3 | **Open collections with licences**: Collections of materials, textures, sounds and music under open licences, each with its licence and credit recorded and shown. Spare Construct, Spare Studio and Spare Social need them. | data | medium | KIT-A15-F18-P1, KIT-A20-F1-P3 |  |

## KIT-A16 Plugins, open API and integrations

Everything a person can do, a script or add-in can do: one command API per app, sandboxed plugins (kit-plugin), webhooks, SDKs, a plugin directory and links to outside tools, with limits that stop bulk copying, and named kit parts every program builds on.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A16-F1** Use every app from a script | Do anything I can do in an app through its API. | core | P1, P2, P3, P4, P5 |
| **KIT-A16-F2** Be told when something changes | Have my own systems called when an order, payment or message changes. | core | P1 |
| **KIT-A16-F3** Build with a ready-made library | Use ready-made libraries for common languages. | core | P1, P2, P3, P4, P5 |
| **KIT-A16-F4** Know the licence | Know which licence covers the code and the SDK. | core | P1 |
| **KIT-A16-F5** Install an add-in | Add a plugin to an app and see what it will be allowed to do first. | core | P1, P2, P3, P4 |
| **KIT-A16-F6** Find add-ins | Browse, rate and install plugins from one directory. | next | P1, P2 |
| **KIT-A16-F7** Publish an add-in | Publish a plugin, and charge a fair price for it if I choose. | next | P1, P2 |
| **KIT-A16-F8** Connect outside tools | Link Spareware to accounting, calendar and other common tools. | next | P1 |
| **KIT-A16-F9** Rely on the API staying stable | Rely on the API not breaking without notice. | core | P1 |
| **KIT-A16-F10** Build an app on named kit parts | As a contributor, depend on a named kit service or crate instead of rebuilding tax, search, support or any other shared part. | core | P1, P2 |
| **KIT-A16-F11** Record and replay my actions | Record what I do once and replay it, on one file or many. | next | P1 |
| **KIT-A16-F12** Run scripts inside an app | Write, run and fix a script next to the work it changes. | next | P1 |
| **KIT-A16-F13** Write formulas safely | Write a formula or expression that can never harm my file or my computer. | core | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A16-F1-P1 | **Command API**: A command and query API for each app, generated from the command schemas. | library | large | KIT-A15-F1-P3, KIT-A1-F6-P1 |  |
| KIT-A16-F1-P2 | **API keys and tokens**: Personal and app tokens with scopes, expiry and revocation. | service | small | KIT-A1-F6-P1 |  |
| KIT-A16-F1-P3 | **API rate limits**: Fair, published limits for each token, set as rules on the shared limiter. | service | small | KIT-A16-F1-P1, KIT-A22-F5-P4 |  |
| KIT-A16-F1-P4 | **Bulk copy limits**: Stops the API and MCP server being used to harvest profiles, addresses and reviews: no bulk lists of personal data, caps per person reached, and alerts on harvesting patterns. | service | medium | KIT-A16-F1-P1, KIT-A22-F5-P4 |  |
| KIT-A16-F1-P5 | **Headless parity check**: A release check that every action on every screen maps to a command in the schema, so anything a person can do, an agent or a script can do too. | library | small | KIT-A15-F1-P3, KIT-A22-F2-P1 |  |
| KIT-A16-F2-P1 | **Webhooks**: Signed webhooks with retries and a delivery log. | service | medium | KIT-A16-F1-P1 |  |
| KIT-A16-F3-P1 | **SDKs**: One TypeScript library generated from the command schemas at first, with Rust and Python libraries added next. | library | small | KIT-A15-F1-P3 |  |
| KIT-A16-F3-P2 | **Developer docs**: Docs, examples and a changelog, generated wherever possible. | interface | medium | KIT-A16-F1-P1 |  |
| KIT-A16-F3-P3 | **Test mode**: A sandbox with test accounts and test payments. | service | medium | KIT-A16-F1-P1, KIT-A3-F1-P2 |  |
| KIT-A16-F3-P4 | **Marketplace simulator**: Runs simulated customers and workers on real map data through the same commands as people, so matching, dispatch, fee setting and fraud rules are measured before launch, with each app adding its own behaviours. | service | large | KIT-A16-F3-P3, KIT-A7-F3-P2, KIT-A6-F3-P2, KIT-A15-F1-P3 |  |
| KIT-A16-F3-P5 | **Command-line tool**: One command-line tool generated from each app's command schemas, for scripts, batch conversion and checks from a terminal. Spare Construct, Spare Studio and Spare OS need it. | library | medium | KIT-A15-F1-P3, KIT-A16-F1-P2 |  |
| KIT-A16-F4-P1 | **Licence decision**: The core and SDK licences, chosen for contributors and add-in authors. | legal | small |  | [A14](../spare-construct/STATUS.md) |
| KIT-A16-F5-P1 | **Plugin method**: WebAssembly components on wasmtime: what authors in each language can use today. | research | medium |  | [A08](../spare-construct/STATUS.md) |
| KIT-A16-F5-P2 | **Plugin host**: Runs plugins as sandboxed WebAssembly components with declared permissions. | library | large | KIT-A15-F1-P2, KIT-A16-F5-P1 | [A08](../spare-construct/STATUS.md) |
| KIT-A16-F5-P3 | **Permission prompt**: Shows a plugin's permissions in plain words before it is installed. | interface | small | KIT-A16-F5-P2, KIT-A20-F1-P1 |  |
| KIT-A16-F5-P4 | **Plugin panels**: Lets plugins add panels and buttons built with HTML and TypeScript, inside the sandbox and with declared permissions. Spare Construct, Spare Studio and Spare Office need them. | library | medium | KIT-A16-F5-P2, KIT-A20-F1-P1 |  |
| KIT-A16-F6-P1 | **Plugin directory**: Listings, versions, reviews and install counts. | service | medium | KIT-A16-F5-P2 |  |
| KIT-A16-F6-P2 | **Plugin review and signing**: Checks and signs plugins before they are listed, with a way to pull a bad one. | service | medium | KIT-A16-F6-P1 |  |
| KIT-A16-F7-P1 | **Publisher tools**: Upload, versions and usage figures for plugin authors. | interface | medium | KIT-A16-F6-P1 |  |
| KIT-A16-F7-P2 | **Paid add-ins**: Pays plugin authors through the payments service, charging GST under the remote services and marketplace operator rules and issuing tax invoices. | service | small | KIT-A16-F6-P1, KIT-A3-F1-P2, KIT-A4-F1-P2, KIT-A4-F2-P1 |  |
| KIT-A16-F8-P1 | **Common connectors**: Connectors to widely used tools through their published APIs, on terms that allow it. | integration | medium | KIT-A16-F1-P1 |  |
| KIT-A16-F9-P1 | **Versions and deprecation**: Versioned APIs with published notice periods before anything is removed. | library | small | KIT-A16-F1-P1 |  |
| KIT-A16-F10-P1 | **Kit service and crate catalogue**: Published names for the kit's eight services and seven crates, the five shared engines mapped in KIT-A27, the new names this map proposes (such as svc-identity, svc-tax, svc-search, svc-support, svc-members, svc-files, svc-translate, kit-ui, kit-i18n, engine-formats and engine-models) until ARCHITECTURE.md adopts them, the part IDs each covers, and the layering rule that kit services may use engines while kit-core never may. | data | small |  |  |
| KIT-A16-F10-P2 | **Layering check**: A repository check that flags a program depending on another program, a program mapping its own copy of a part the kit provides, or kit-core depending on an engine. | library | small | KIT-A16-F10-P1, KIT-A22-F2-P1 |  |
| KIT-A16-F11-P1 | **Action recorder**: Records a person's operations as a script they can edit, replay and run on many files. Spare Construct, Spare Studio and Spare Office need the same recorder. | library | medium | KIT-A15-F1-P2, KIT-A15-F1-P3 |  |
| KIT-A16-F12-P1 | **Script console**: A console and editor inside each app to write, run and debug scripts against its command API, with time and memory limits. Spare Construct, Spare Studio and Spare Office need it. | interface | medium | KIT-A16-F1-P1, KIT-A16-F3-P1 |  |
| KIT-A16-F13-P1 | **Safe expression language**: A small, sandboxed expression language with units checked and a library of common functions, for parameter formulas and animation expressions. Spare Construct and Spare Studio both need it. | library | medium | kit-core |  |

## KIT-A17 AI access

Lets members use AI agents with every app through the same commands as people (kit-mcp), with permissions they choose, approval for money, guards against hidden instructions, agent actions labelled to the other side, people-only votes and reviews, and a record of everything an agent did.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A17-F1** Connect my AI agent | Connect an AI agent to any Spareware app and choose what it may do. | core | P1, P2, P3 |
| **KIT-A17-F2** Approve important actions | Approve any payment, booking or step that can't be undone before an agent takes it. | core | P1 |
| **KIT-A17-F3** See what an agent did | See every action an agent took, and undo it. | core | P1 |
| **KIT-A17-F4** Keep agents within limits | Set spending, rate and scope limits for each agent. | core | P1 |
| **KIT-A17-F5** Protect my agent from hidden instructions | Have agents protected from instructions hidden in listings, messages or documents. | core | P1 |
| **KIT-A17-F6** Choose the AI model | Bring my own AI, or use one the co-op runs at cost. | core | P1, P2 |
| **KIT-A17-F7** Ask an assistant for help | Ask an assistant how to do something in any app. | next | P1 |
| **KIT-A17-F8** Use an app by voice | Run an app hands-free by voice, such as while driving. | later | P1 |
| **KIT-A17-F9** Know when I am dealing with an agent | See when a message, offer or booking came from someone's AI agent, and know votes and reviews come only from people. | core | P1, P2 |
| **KIT-A17-F10** See where the co-op uses AI | See every place the co-op uses AI to shape a decision about me, and ask for a person to review it. | core | P1, P2 |
| **KIT-A17-F11** Choose whether my data trains AI | Choose whether my data or work is ever used to train an AI model, with the answer no until I say yes. | core | P1 |
| **KIT-A17-F12** Let my agent pay under a mandate I signed | Let my agent pay within a mandate I signed, which the payment provider and the other side can check. | later | P1 |
| **KIT-A17-F13** Run an agent for my organisation | Run an AI agent for my restaurant, shop or office, with permissions set for the organisation and a named owner. | later | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A17-F1-P1 | **MCP server**: An MCP server for each app, built from its command schemas. | library | large | KIT-A15-F1-P3, KIT-A16-F1-P1 |  |
| KIT-A17-F1-P2 | **Agent sign-in**: Agents sign in through the sign-in service with the scopes the person picks. | service | medium | KIT-A1-F6-P1, KIT-A17-F1-P1, KIT-A17-F1-P3 |  |
| KIT-A17-F1-P3 | **MCP authorisation**: Meets the MCP authorisation spec: OAuth 2.1 with PKCE S256, protected resource metadata (RFC 9728), audience-bound tokens (RFC 8707), refresh token rotation and client ID metadata documents. | integration | medium | KIT-A1-F6-P1, KIT-A17-F1-P1 |  |
| KIT-A17-F2-P1 | **Approval steps**: Marks the commands that need a person's approval and asks for it. | library | medium | KIT-A17-F1-P1, KIT-A15-F1-P3 |  |
| KIT-A17-F3-P1 | **Agent activity**: Lists agent actions from the operation log, with undo. | interface | small | KIT-A17-F1-P1, KIT-A15-F1-P2 |  |
| KIT-A17-F4-P1 | **Agent limits**: Spending, rate and scope limits for each agent. | library | small | KIT-A17-F1-P1 |  |
| KIT-A17-F5-P1 | **Hidden instruction guards**: Marks outside content as data and limits what an agent can do with it. | library | medium | KIT-A17-F1-P1 |  |
| KIT-A17-F6-P1 | **AI tools policy**: Options for models, cost and consent, shared with Spare Studio. | research | small |  | [S07](../spare-studio/STATUS.md) |
| KIT-A17-F6-P2 | **Co-op AI at cost**: Open models run by the co-op and charged at cost. | service | large | KIT-A13-F2-P2, KIT-A17-F6-P1 |  |
| KIT-A17-F7-P1 | **Help assistant**: Answers from the help centre and acts only with approval. | service | medium | KIT-A17-F6-P2, KIT-A21-F1-P1 |  |
| KIT-A17-F8-P1 | **Voice control**: Voice commands mapped to the same commands as the interface. | library | medium | KIT-A17-F1-P1, KIT-A17-F6-P2 |  |
| KIT-A17-F9-P1 | **Agent labels**: Marks every message, offer and booking an agent makes, so the other side sees it came from an agent acting for a named person. | library | small | KIT-A17-F1-P2, KIT-A15-F1-P2 |  |
| KIT-A17-F9-P2 | **People-only actions**: Ratings, reviews, votes, elections, proxies, dispute statements, signing agreements, accepting terms, joining or leaving the co-op, and consenting to biometric or background checks can be done only by the person, never by an agent. | library | small | KIT-A17-F1-P2, KIT-A15-F1-P4 |  |
| KIT-A17-F10-P1 | **AI use register**: A public register of where AI is used, such as moderation hints, matching, support and translation, with what each model may decide. | service | small | KIT-A17-F6-P1 |  |
| KIT-A17-F10-P2 | **Human review of AI decisions**: Lets a member ask for a person to review any decision an automated tool shaped, through the one review queue. | service | small | KIT-A17-F10-P1, KIT-A10-F6-P2 |  |
| KIT-A17-F11-P1 | **AI training choice**: A choice, off by default, over whether a member's data or work is used to train any AI model, kept in the consent records and checked before every training run. | library | small | KIT-A18-F17-P1 |  |
| KIT-A17-F12-P1 | **Signed payment mandates**: Agent payments under a signed mandate the person approved, which the provider and the other side can check, on an open protocol such as AP2 where the provider supports it. | integration | medium | KIT-A17-F4-P1, KIT-A17-F2-P1, KIT-A3-F1-P2 |  |
| KIT-A17-F13-P1 | **Organisation agents**: Agents that act for an organisation, with organisation-wide permissions and limits, and a named person who owns each one. | service | small | KIT-A17-F1-P2, KIT-A17-F4-P1, KIT-A1-F7-P1 |  |

## KIT-A18 Privacy, data rights and legal duties

Members see, take and delete their data, consent is recorded, data is kept only as long as needed, a named privacy officer answers, overseas handling is checked, Māori data is cared for, and the co-op meets privacy, consumer and platform law in NZ first and each new country after. Built in svc-accounts; svc-privacy is a proposed name (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A18-F1** Download all my data | Download everything held about me, from every app, in open formats. | core | P1 |
| **KIT-A18-F2** Delete my account | Delete my account and data, apart from what the law says must be kept. | core | P1, P2 |
| **KIT-A18-F3** See and correct my data | See what is held about me and who accessed it, and ask to correct it. | core | P1, P2 |
| **KIT-A18-F4** Agree to terms I can read | Read short terms, see what changed, and know what I agreed to. | core | P1, P2 |
| **KIT-A18-F5** Be told what is collected about me | Be told what is collected and why, including data that comes from someone else. | core | P1 |
| **KIT-A18-F6** Know data is kept only as long as needed | Know data is deleted on a published schedule. | core | P1 |
| **KIT-A18-F7** Be told about a breach | Be told quickly if my data is exposed. | core | P1 |
| **KIT-A18-F8** Get the protection consumer law gives | Get my consumer law rights in every app. | core | P1 |
| **KIT-A18-F9** Keep my data in my region | Have my data stored in my region where the law asks. | next | P1 |
| **KIT-A18-F10** Trust that new features are checked for privacy | Know every new feature is checked for privacy before it ships. | core | P1 |
| **KIT-A18-F11** Have my account handled after I die | Let a named person close or keep my account after my death. | core | P1 |
| **KIT-A18-F12** Have overseas privacy rights honoured | Have the rights other countries' privacy laws give me honoured. | core | P1 |
| **KIT-A18-F13** Have overseas platform law met | Have overseas platform duties, such as notice and appeal rules, met. | core | P1 |
| **KIT-A18-F14** Reach the privacy officer | Contact the named privacy officer for the co-op or any app about my data. | core | P1 |
| **KIT-A18-F15** Know who else handles my data | See which outside services handle my data, what they do with it and which countries it goes to. | core | P1, P2 |
| **KIT-A18-F16** Have Māori data cared for | Know te reo Māori data, voice recordings and iwi-related data are cared for under Māori data sovereignty principles. | core | P1 |
| **KIT-A18-F17** See and change every consent I gave | See every consent I gave, such as for marketing, location or AI use, and withdraw any of them in one step. | core | P1, P2 |
| **KIT-A18-F18** Have sensitive details handled with care | Have my health, religious and disability details asked for only when needed and kept with extra care. | core | P1, P2 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A18-F1-P1 | **Data export**: Exports everything held about a person in documented open formats: profile, orders, messages, ratings, files, support cases, decisions about them, check results, consents, agent activity and, where the law allows, risk flags. | service | medium | KIT-A15-F1-P2, KIT-A15-F7-P1, KIT-A18-F17-P1 |  |
| KIT-A18-F2-P1 | **Account deletion**: Deletes or anonymises data across services, keeping legally required records apart. | service | medium | KIT-A1-F1-P1, KIT-A18-F6-P1, KIT-A15-F1-P6, KIT-A18-F2-P2 |  |
| KIT-A18-F2-P2 | **Settle up before closing**: Before an account closes, ends or cancels open orders, settles money owed either way, closes open disputes and checks legal holds, in that order. | service | small | KIT-A5-F4-P1, KIT-A3-F16-P2, KIT-A11-F1-P1, KIT-A18-F6-P1 |  |
| KIT-A18-F3-P1 | **Privacy page**: Shows the data held, who accessed it, and correction requests. | interface | medium | KIT-A20-F1-P1, KIT-A15-F1-P2, KIT-A15-F1-P6 |  |
| KIT-A18-F3-P2 | **Privacy request clock**: Tracks every request to see or correct personal information against the Privacy Act's 20 working day limit, with reminders and any extension recorded. | service | small | KIT-A18-F3-P1, KIT-A19-F9-P2, KIT-A15-F14-P1 |  |
| KIT-A18-F4-P1 | **Terms and policy versions**: Versioned terms and policies, with a record of each person's acceptance. | service | small | kit-ops, KIT-A18-F4-P2 |  |
| KIT-A18-F4-P2 | **Terms and privacy policy**: Plain terms and a privacy policy for each app, checked by a lawyer. | legal | medium |  |  |
| KIT-A18-F5-P1 | **Collection notices**: Notices for direct and indirect collection under the NZ Privacy Act, including principle 3A from 1 May 2026. | legal | small |  |  |
| KIT-A18-F6-P1 | **Retention schedules**: Retention rules for each kind of data, legal holds and automatic deletion. | service | medium | KIT-A15-F1-P2, KIT-A15-F14-P1 |  |
| KIT-A18-F7-P1 | **Breach response**: Breach assessment and notification to the Privacy Commissioner and the people affected. | legal | small |  |  |
| KIT-A18-F8-P1 | **Consumer law for platforms**: Fair trading, consumer guarantees and unfair contract terms duties for each app. | research | medium |  | [MK02](../spare-market/STATUS.md) |
| KIT-A18-F9-P1 | **Data location**: Places each member's data by region and records where it is kept. | service | medium | KIT-A22-F4-P1 |  |
| KIT-A18-F10-P1 | **Privacy impact assessments**: A short template and a public register of assessments. | legal | small |  |  |
| KIT-A18-F11-P1 | **After-death account handling**: A named contact and a process to close, keep or hand over an account. | service | small | KIT-A1-F1-P1, KIT-A21-F2-P2 |  |
| KIT-A18-F12-P1 | **Overseas privacy duties**: Duties under laws such as the EU's GDPR for each country added. | legal | medium | KIT-A19-F6-P1 |  |
| KIT-A18-F13-P1 | **Overseas platform duties**: Duties under laws such as the EU Digital Services Act and the UK Online Safety Act. | legal | medium | KIT-A19-F6-P1 |  |
| KIT-A18-F14-P1 | **Privacy officer**: Appoints a named privacy officer for the co-op and every app, as the Privacy Act 2020 requires, reachable from every privacy page. | legal | small |  |  |
| KIT-A18-F15-P1 | **Overseas disclosure checks**: Checks under information privacy principle 12 before personal data goes to an overseas provider, such as the payment provider, push, email and text services. | legal | small |  |  |
| KIT-A18-F15-P2 | **Processor list**: A published list of every outside processor, what it handles and where, updated whenever one changes. | service | small | KIT-A18-F15-P1 |  |
| KIT-A18-F16-P1 | **Māori data principles**: How Māori data sovereignty principles, such as Te Mana Raraunga's, apply to te reo data, voice and iwi-related data, worked through with Māori members. | research | small |  |  |
| KIT-A18-F17-P1 | **Consent records**: One service that records each consent, such as marketing, biometrics, background checks, location, AI use and crash reports, with its version and time, and that every app checks before use. | service | medium | kit-ops, KIT-A1-F1-P1, KIT-A18-F4-P1 |  |
| KIT-A18-F17-P2 | **Consents page**: Shows every consent a person gave, in every app, and withdraws any of them in one step. | interface | small | KIT-A18-F17-P1, KIT-A20-F1-P1 |  |
| KIT-A18-F18-P1 | **Sensitive details rules**: Rules for health, religious and disability details: consent before saving, the least detail each party needs, no use in suggestions unless switched on, one-step deletion, and special category treatment where the GDPR applies. Spare Food, Spare Ride and Spare Office need them. | legal | small | KIT-A18-F17-P1, KIT-A18-F6-P1 |  |
| KIT-A18-F18-P2 | **Health information rules**: What the Health Information Privacy Code 2020 asks when health details are kept, such as rides booked for patients or health providers using office tools. | legal | small | KIT-A18-F18-P1 |  |

## KIT-A19 Languages, accessibility and countries

Every app works in te reo Māori and English from the start, meets accessibility standards, gets NZ's holidays, time zones and emergency numbers right, and takes on new languages and countries without rewriting. Proposed as kit-i18n and svc-translate (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A19-F1** Use an app in my language | Switch any app between te reo Māori, English and other languages. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A19-F2** Use every screen with a screen reader or keyboard | Use every screen with a screen reader, a keyboard or a switch. | core | P1, P2, P3, P4, P5, P6, P7, P8 |
| **KIT-A19-F3** Make text and colours easy to read | Make text larger, raise contrast and reduce motion. | core | P1 |
| **KIT-A19-F4** Get captions and transcripts | Get captions and transcripts on videos and voice messages. | core | P1 |
| **KIT-A19-F5** Use an app on an old phone or a slow connection | Use apps on older phones and weak connections. | core | P1 |
| **KIT-A19-F6** Use an app in a new country | Use an app in another country with its money, taxes, laws and ID checks. | core | P1 |
| **KIT-A19-F7** Read right to left | Use the apps in right-to-left languages. | core | P1 |
| **KIT-A19-F8** Read an easy version | Read key screens and rules in easy-read and NZ Sign Language versions. | core | P1 |
| **KIT-A19-F9** Have NZ's dates and numbers right from day one | Have deadlines, notice periods and emergency numbers follow NZ's real calendar, holidays and time zones. | core | P1, P2 |
| **KIT-A19-F10** Read translations made at cost | Read messages, listings and help pages machine-translated by one co-op service, with te reo Māori checked by fluent speakers. | core | P1, P2, P3 |
| **KIT-A19-F11** Read receipts and statements with a screen reader | Read every receipt, statement, invoice and certificate with a screen reader. | core | P1 |
| **KIT-A19-F12** Use a simpler layout | Switch to a pared-back layout with large text and only the main tasks. | later | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A19-F1-P1 | **Translation framework**: The one translation framework every app uses: message strings with plural and gender rules, in an open format such as Fluent. | library | medium |  |  |
| KIT-A19-F1-P2 | **Local formats**: Dates, numbers, money and addresses for each locale, using ICU4X. | library | medium | KIT-A15-F1-P1 |  |
| KIT-A19-F1-P3 | **Member translation**: A workflow for member translators, with reviewers for each language. | service | medium | KIT-A19-F1-P1, KIT-A18-F16-P1 |  |
| KIT-A19-F1-P4 | **Macron-aware matching**: Matching that ignores macrons when people type, so Maori finds Māori in search and addresses. | library | small | KIT-A19-F1-P2 |  |
| KIT-A19-F1-P5 | **Community languages**: Chooses and funds the next languages members speak most, starting in NZ with languages such as Samoan, Tongan, Chinese and Hindi, translated with fluent speakers once for every app. Spare OS, Spare Home, Spare Rent, Spare Work and Spare Market all plan them. | research | medium | KIT-A19-F1-P3 |  |
| KIT-A19-F1-P6 | **Translated listing fields**: Lets authors write or correct their own translations of names and descriptions, shown in each reader's language and found by search. Spare Social, Spare Food and Spare Stay need it. | library | small | KIT-A19-F1-P1, KIT-A6-F1-P2 |  |
| KIT-A19-F2-P1 | **Accessible components**: Shared components built and tested to WCAG 2.2 AA. | library | medium | KIT-A20-F1-P1, KIT-A19-F2-P2 |  |
| KIT-A19-F2-P2 | **Accessibility testing**: Automatic checks plus testing with disabled members before each release. | research | small |  |  |
| KIT-A19-F2-P3 | **Image descriptions**: Asks for a description when an image is added, so screen readers can read it. | library | small | KIT-A15-F7-P1, KIT-A19-F2-P2 |  |
| KIT-A19-F2-P4 | **Accessibility statement and barrier reports**: An accessibility statement for each app and a way to report a barrier, answered through support within a published time, as the European Accessibility Act asks of online shops and transport services. | interface | small | KIT-A19-F2-P2, KIT-A21-F2-P1 |  |
| KIT-A19-F2-P5 | **Accessibility conformance reports**: A published conformance report for each app against WCAG 2.2 and EN 301 549, updated each release. Spare Studio, Spare Office and Spare OS need them. | legal | small | KIT-A19-F2-P2 |  |
| KIT-A19-F2-P6 | **Accessibility checker for content**: Checks documents and designs against WCAG 2.2 and PDF/UA, such as contrast, missing alt text and reading order, with a fix for each issue. Spare Office and Spare Studio need the same checker. | library | medium | KIT-A19-F2-P3, KIT-A27-F6-P2 |  |
| KIT-A19-F2-P7 | **Canvas mirror for screen readers**: A hidden structure that tells screen readers what is drawn on a canvas, such as pages, sheets, slides, layers and selections. Spare Office and Spare Studio share one. | library | medium | KIT-A19-F2-P1, KIT-A20-F1-P1 |  |
| KIT-A19-F2-P8 | **Accessibility law research**: Which accessibility laws apply to each app in NZ, the EU and other markets, such as the European Accessibility Act, and what each requires. Spare OS, Spare Studio and Spare Stay all ask. | research | small | KIT-A19-F2-P4 |  |
| KIT-A19-F3-P1 | **Display settings**: Text size, contrast and motion settings that follow the device. | library | small | KIT-A20-F1-P1 |  |
| KIT-A19-F4-P1 | **Captions**: Captions and transcripts for media, shared with Spare Social. | library | medium | engine-media |  |
| KIT-A19-F5-P1 | **Light mode for slow connections**: Smaller pages, fewer images and screens that work offline first. | library | medium | KIT-A20-F2-P2 |  |
| KIT-A19-F6-P1 | **Country packs**: Packs for each new country in the NZ pack's format: currency, tax, ID providers, emergency numbers, helplines, holidays and legal rules. | data | large | KIT-A15-F1-P1, KIT-A19-F9-P1 |  |
| KIT-A19-F7-P1 | **Right-to-left layout**: Mirrored layouts and right-to-left text handling. | library | medium | KIT-A20-F1-P1, engine-text |  |
| KIT-A19-F8-P1 | **Easy read and NZ Sign Language**: Easy-read text and NZ Sign Language videos for the key steps in each app. | data | medium | KIT-A19-F1-P1 |  |
| KIT-A19-F9-P1 | **NZ country pack**: The first country pack: the emergency number and 111 TXT, helplines, GST rate, currency, public holidays including Matariki and regional anniversary days, and both time zones including the Chatham Islands. | data | medium | KIT-A15-F1-P1 |  |
| KIT-A19-F9-P2 | **Working days and notice periods**: Counts working days, public holidays and notice periods for legal clocks, dispute deadlines, meeting notices and tax due dates. | library | small | KIT-A19-F9-P1 |  |
| KIT-A19-F10-P1 | **Translation model choice**: Compares open translation models whose licences allow this use, on quality for te reo Māori and other languages and on running cost. | research | medium | KIT-A17-F6-P1 |  |
| KIT-A19-F10-P2 | **Machine translation at cost**: One translation service for every app, run at cost, that labels machine translations and keeps the original one tap away. | service | large | KIT-A19-F10-P1, KIT-A22-F4-P1 |  |
| KIT-A19-F10-P3 | **Te reo quality checks**: Fluent members review samples of te reo Māori translations, and weak language pairs are switched off until they pass. | service | small | KIT-A19-F10-P2, KIT-A19-F1-P3, KIT-A18-F16-P1 |  |
| KIT-A19-F11-P1 | **Accessible PDFs**: Tagged, accessible PDFs for receipts, statements, invoices and certificates. | library | medium | engine-pdf, KIT-A19-F2-P1 |  |
| KIT-A19-F12-P1 | **Simple mode**: A pared-back layout with the main tasks first and large text, for people new to an app. Spare Ride and Spare Market both plan it. | interface | small | KIT-A20-F1-P1, KIT-A19-F3-P1 |  |

## KIT-A20 App shell and phone apps

The shared frame every app runs in: components from the design guide, installable phone and desktop apps, push, deep links, settings and moving between Spareware apps. Proposed as kit-ui (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A20-F1** Use apps that look and work alike | Find the same buttons and screens in every Spareware app. | core | P1, P2, P3 |
| **KIT-A20-F2** Install an app on my phone | Install an app on a phone and get push and location like any other app. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A20-F3** Move between apps | Move between Spareware apps without signing in again. | core | P1 |
| **KIT-A20-F4** Open a link in the right place | Open a shared link straight to the listing, order or chat it points to. | core | P1 |
| **KIT-A20-F5** Change my settings | Find every setting for my account and each app in one place. | core | P1, P2 |
| **KIT-A20-F6** Get started quickly | Learn what an app does in a short first-run guide. | core | P1, P2 |
| **KIT-A20-F7** Get updates without losing work | Get app updates without losing anything I was doing. | core | P1, P2 |
| **KIT-A20-F8** Find the apps in phone app stores | Find Spareware apps in the phone app stores. | core | P1 |
| **KIT-A20-F9** Share a short link or QR code | Share a short link or QR code to a listing, a form or an invite, and see how many times it was opened. | core | P1 |
| **KIT-A20-F10** Find any command | Type to find and run any command, and use the shortcuts I already know. | core | P1 |
| **KIT-A20-F11** Use a work app safely on the road | Use a worker app in a car or on a bike without typing or tapping while moving. | core | P1, P2 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A20-F1-P1 | **Shared interface components**: HTML and TypeScript components built on the design guide and its tokens. | library | large |  |  |
| KIT-A20-F1-P2 | **App frame**: Header, navigation, account menu and each app's own colour. | library | medium | KIT-A20-F1-P1 |  |
| KIT-A20-F1-P3 | **Licences and credits screen**: A screen in every app listing each open-source library and asset used, with its licence notice and source, generated from the dependency checks. | interface | small | KIT-A20-F1-P1, KIT-A22-F5-P2 |  |
| KIT-A20-F2-P1 | **Phone app approach**: Web apps, Tauri mobile or native, covering live location, push and app store rules. | research | medium |  | [K04](STATUS.md) |
| KIT-A20-F2-P2 | **Installable apps**: The chosen phone and desktop wrappers for every app. | library | large | KIT-A20-F1-P1, KIT-A20-F2-P1 | [K04](STATUS.md) |
| KIT-A20-F2-P3 | **Location and background permissions**: Asks for location and background use only when needed, explained in plain words. | library | medium | KIT-A20-F2-P2 | [K04](STATUS.md) |
| KIT-A20-F2-P4 | **Desktop shell choice**: A decision note on the desktop wrapper, such as Tauri or our own web view host, with native code in the same process for heavy apps. Spare Construct, Spare Office, Spare Studio and Spare Social need the same shell. | research | medium | KIT-A20-F2-P1 |  |
| KIT-A20-F2-P5 | **Desktop installers**: Signed installers for Windows, macOS and Linux that add or remove single apps, with automatic updates and roll back. Spare Construct, Spare Studio and Spare Office need them. | service | medium | KIT-A20-F2-P4, KIT-A22-F2-P3, KIT-A20-F7-P1 |  |
| KIT-A20-F2-P6 | **Battery and data budgets**: Published limits on battery and data use over a full shift for worker apps, tested on common phones, with fewer location updates while waiting. Spare Ride and Spare Food both need them. | library | small | KIT-A20-F2-P3, KIT-A7-F4-P1 |  |
| KIT-A20-F3-P1 | **App switcher**: Lists the apps I use and opens each with my account. | interface | small | KIT-A1-F3-P2, KIT-A20-F1-P1 |  |
| KIT-A20-F4-P1 | **Deep links**: Links that open the right screen in the app or the browser. | library | small | KIT-A20-F2-P2 |  |
| KIT-A20-F5-P1 | **Settings framework**: Shared settings pages that each app adds to. | interface | medium | KIT-A20-F1-P1 |  |
| KIT-A20-F5-P2 | **Preferences that follow me**: Typed preferences per person, organisation and project, with a clear order of which wins, saved as operations so they reach every device and can be exported. Spare Construct, Spare Studio, Spare Office and Spare Ride need them. | library | small | KIT-A20-F5-P1, KIT-A15-F4-P2 |  |
| KIT-A20-F6-P1 | **First-run guide**: Short intro screens and helpful empty pages in each app. | interface | small | KIT-A20-F1-P1, KIT-A19-F1-P1 |  |
| KIT-A20-F6-P2 | **Tips in context**: Short tips on a tool or setting the first time it is used, which can be turned off. Spare Construct and Spare Studio both need them. | interface | small | KIT-A20-F6-P1 |  |
| KIT-A20-F7-P1 | **Update delivery**: Update checks and safe reloads that keep unsaved work. | library | small | KIT-A20-F2-P2, KIT-A15-F4-P2 |  |
| KIT-A20-F7-P2 | **Update channels**: Stable, preview and long-support channels, with pause, skip, pinned versions and roll back. Spare Construct, Spare Studio and Spare OS need them. | service | small | KIT-A20-F7-P1, KIT-A22-F2-P3 |  |
| KIT-A20-F8-P1 | **App store rules**: Meets each store's rules on payments, privacy labels and reviews. | legal | small | KIT-A20-F2-P2 | [K04](STATUS.md) |
| KIT-A20-F9-P1 | **Short links and QR codes**: Short links and QR codes that open the right screen through deep links and count scans in totals only, without tracking anyone. | service | small | KIT-A20-F4-P1 |  |
| KIT-A20-F10-P1 | **Command search and shortcuts**: Type to find and run any command, with shortcut sets that start from the ones people already know and can be changed, exported and shared. Spare Construct, Spare Studio, Spare Office and Spare OS need the same. | interface | small | KIT-A15-F1-P3, KIT-A20-F1-P1 |  |
| KIT-A20-F11-P1 | **Phone rules on the road**: What NZ road rules allow for phones in cradles in cars and on bikes, e-bikes and scooters, with sources, checked against each worker screen. Spare Ride and Spare Food both need it. | legal | small |  |  |
| KIT-A20-F11-P2 | **Driving-safe mode**: Worker screens that block typing while moving, use large targets, and read offers and messages aloud. Spare Food and Spare Ride both need it. | interface | medium | KIT-A20-F11-P1, KIT-A19-F2-P1, KIT-A27-F12-P5 |  |

## KIT-A21 Help and member support

Help pages in every language, support tied to the job, urgent help at any hour, members helping members, and a way to complain about the co-op itself, with response times published. Proposed as svc-support (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A21-F1** Find help myself | Search help pages for any app in my language. | core | P1, P2 |
| **KIT-A21-F2** Contact support about a job | Ask for help about a particular order and follow the case. | core | P1, P2 |
| **KIT-A21-F3** Reach a person when it is urgent | Reach a person quickly about safety or money problems. | core | P1 |
| **KIT-A21-F4** Get help another way | Get support by phone, email or the relay service if apps are hard for me to use. | core | P1 |
| **KIT-A21-F5** Know if something is down | Check whether an app is working and when it will be fixed. | core | P1 |
| **KIT-A21-F6** Help each other | Ask and answer questions with other members in a forum. | core | P1 |
| **KIT-A21-F7** Suggest an improvement | Suggest a change, and turn it into a proposal if others support it. | next | P1 |
| **KIT-A21-F8** See how fast support answers | See published support response times. | next | P1 |
| **KIT-A21-F9** Complain about the co-op itself | Make a complaint about the co-op, not just about a job, and know where to take it if I am not satisfied. | core | P1, P2 |
| **KIT-A21-F10** Get an interpreter | Have an interpreter, including NZ Sign Language, for a dispute, a panel hearing or a support call. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A21-F1-P1 | **Help centre**: Articles for each app and language, searchable and linked from each screen. | service | medium | KIT-A19-F1-P1, KIT-A6-F1-P2 |  |
| KIT-A21-F1-P2 | **Plain-words glossary**: Explains legal and trade terms in plain words, linked wherever they appear. Spare Home and Spare Rent both need it. | data | small | KIT-A21-F1-P1, KIT-A19-F1-P1 |  |
| KIT-A21-F2-P1 | **Support cases**: Cases linked to orders and accounts, with status and replies. | service | medium | KIT-A5-F4-P1, KIT-A8-F1-P2 |  |
| KIT-A21-F2-P2 | **Support console**: A case queue for support staff with history, reply templates and every action logged. | interface | large | KIT-A21-F2-P1, KIT-A15-F1-P4 |  |
| KIT-A21-F3-P1 | **Urgent route**: The one urgent route and on-call roster for safety and money problems, with a safety lane staffed by trained responders. | service | medium | KIT-A21-F2-P1 |  |
| KIT-A21-F4-P1 | **Other support channels**: Email and phone channels, and NZ Relay for Deaf, hard of hearing and speech-impaired members. | integration | small | KIT-A21-F2-P1 |  |
| KIT-A21-F5-P1 | **Status page**: Live status for each app with incident updates, hosted apart from the main servers. | service | small | KIT-A22-F1-P1 |  |
| KIT-A21-F6-P1 | **Member forum**: A forum for each app, looked after by member moderators. | service | medium | KIT-A10-F6-P2, KIT-A8-F12-P1 |  |
| KIT-A21-F7-P1 | **Suggestions**: Suggestions that can move to a member proposal. | service | small | KIT-A12-F1-P1, KIT-A8-F12-P1 |  |
| KIT-A21-F8-P1 | **Support figures**: Published response times and satisfaction for each app, through the public reporting pipeline. | service | small | KIT-A21-F2-P1, KIT-A13-F9-P1 |  |
| KIT-A21-F9-P1 | **Dispute resolution scheme**: Whether the co-op must register on the Financial Service Providers Register and, if so, join an approved dispute resolution scheme. | legal | small | KIT-A3-F2-P2 |  |
| KIT-A21-F9-P2 | **Complaints about the co-op**: A published complaints process about the co-op itself, with response times and the outside route that applies. | service | small | KIT-A21-F9-P1, KIT-A21-F2-P1 |  |
| KIT-A21-F10-P1 | **Interpreters**: Books qualified interpreters, including NZ Sign Language, for disputes, panel hearings and support, paid at cost and shown in the ledger. | integration | small | KIT-A21-F2-P1, KIT-A11-F4-P2 |  |

## KIT-A22 Running it: hosting, security and admin

Servers run at cost by the co-op or by member groups, with monitoring, backups, encryption, signed releases, security reviews and an admin console where every action is logged. Proposed as svc-admin (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A22-F1** Keep the apps running | Have apps stay up, with problems spotted and fixed quickly. | core | P1, P2, P3 |
| **KIT-A22-F2** Get changes without breakage | Have changes reach members without breaking things. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A22-F3** Know my data is backed up | Know data survives a failure and restores are tested. | core | P1, P2 |
| **KIT-A22-F4** Have servers run at cost | Have servers run in NZ first, at cost, with their costs fed to the ledger. | core | P1, P2 |
| **KIT-A22-F5** Know the apps are secure | Know the apps are protected and checked by outside reviewers. | core | P1, P2, P3, P4, P5 |
| **KIT-A22-F6** Have admin work done in the open | Have staff act only through an admin console where every action is logged. | core | P1 |
| **KIT-A22-F7** Run our own server | Let a member group run its own servers and connect them. | next | P1, P2, P3, P4 |
| **KIT-A22-F8** Know how apps are used without being tracked | Know how apps are used from counts that never track a person. | core | P1 |
| **KIT-A22-F9** Have crashes fixed | Have app crashes reported, with my consent, so they get fixed. | core | P1 |
| **KIT-A22-F10** Install apps on many machines | Install, update and set up apps on every machine in an office or school at once. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A22-F1-P1 | **Monitoring and alerts**: Open-source metrics, logs and alerts, with every licence checked. | service | medium |  |  |
| KIT-A22-F1-P2 | **Incident response**: An on-call roster, runbooks and published incident reviews. | service | medium | KIT-A22-F1-P1, KIT-A21-F5-P1 |  |
| KIT-A22-F1-P3 | **Spend alerts and budgets**: Budgets and alerts on cloud and provider spend for each app, so a runaway bill is caught within hours, not at month end. | service | small | KIT-A22-F1-P1, KIT-A13-F2-P1 |  |
| KIT-A22-F2-P1 | **Release pipeline**: Builds, tests, staged rollouts and rollback for every service. | service | medium |  |  |
| KIT-A22-F2-P2 | **Feature switches**: Turns features on for each app, region or group, with every change logged. | library | small | KIT-A15-F1-P2 |  |
| KIT-A22-F2-P3 | **Signed releases**: Signed releases, a software bill of materials, reproducible builds and app-signing keys kept offline under two-person control. | service | medium | KIT-A22-F2-P1, KIT-A22-F5-P1 |  |
| KIT-A22-F2-P4 | **Developer accounts and signing certificates**: Apple, Google and Windows developer accounts and code signing certificates held by the legal entity, including Google's developer verification for sideloaded Android apps from 2026 to 2027 (*unverified* dates for NZ), with costs on the ledger. | legal | small | KIT-A22-F2-P3, KIT-A14-F1-P1 |  |
| KIT-A22-F2-P5 | **Performance budgets**: Published limits on start time, load time, memory and data use for each app, measured on old devices and real connections on every change, so a slower change does not merge. Spare Social and Spare Office both set them. | library | small | KIT-A22-F2-P1 |  |
| KIT-A22-F2-P6 | **Name and interface review**: Checks before each release that names, icons and interface designs are our own, and that other companies' names appear only to say what we replace. Spare Construct and Spare Office both map it. | legal | small | KIT-A14-F18-P1 |  |
| KIT-A22-F3-P1 | **Backups and restore drills**: Continuous off-site copies of the operation log and scheduled restore tests. | service | medium | KIT-A15-F4-P2 |  |
| KIT-A22-F3-P2 | **Disaster recovery**: A second region and a tested plan to move to it. | service | medium | KIT-A22-F4-P1 |  |
| KIT-A22-F4-P1 | **Hosting**: Infrastructure as code for the co-op's servers, NZ first. | service | large | KIT-A22-F2-P1, KIT-A22-F4-P2 |  |
| KIT-A22-F4-P2 | **Hosting provider choice**: Compares hosting providers on NZ regions, data sovereignty, overseas laws that can reach the data and cost, as the basis for the launch cost estimate and for where data lives. | research | medium |  |  |
| KIT-A22-F5-P1 | **Secrets and access**: Least-access roles, secret storage and two-person approval for risky actions. | service | medium | KIT-A15-F1-P4 |  |
| KIT-A22-F5-P2 | **Dependency checks**: Licence and vulnerability checks on every dependency. | service | small | KIT-A22-F2-P1 |  |
| KIT-A22-F5-P3 | **Security reviews**: Outside reviews before each app goes live, and a page for reporting weaknesses. | service | medium |  |  |
| KIT-A22-F5-P4 | **Traffic and abuse limits**: The one rate limiter: limits and protection from traffic floods, on which sign-up, spam, sign-in, API and agent rules are all set. | service | medium | KIT-A22-F4-P1 |  |
| KIT-A22-F5-P5 | **Encryption and keys**: Encryption in transit and at rest for every service and backup, with key management, rotation and access logged. | service | medium | KIT-A22-F5-P1, KIT-A22-F4-P1 |  |
| KIT-A22-F6-P1 | **Admin console**: Search members and orders and act on them, each action an operation with a stated reason. | interface | large | KIT-A15-F1-P2, KIT-A15-F1-P4, KIT-A20-F1-P1 |  |
| KIT-A22-F7-P1 | **Self-host package**: Container images, guides and upgrade paths for member-run servers. | service | large | KIT-A22-F4-P1, KIT-A22-F7-P2 |  |
| KIT-A22-F7-P2 | **Connecting member servers**: How member-run servers join, share data and stay trusted. | research | medium |  |  |
| KIT-A22-F7-P3 | **Move an account between servers**: Moves a member's account and history between the co-op's servers and a member-run server, either way. | service | medium | KIT-A22-F7-P1, KIT-A22-F7-P2, KIT-A18-F1-P1 |  |
| KIT-A22-F7-P4 | **Duties when members self-host**: Who carries GST, anti-money-laundering, privacy and safety duties when a member group runs its own servers, set out in the agreement each group signs. | legal | medium | KIT-A22-F7-P2, KIT-A4-F9-P1, KIT-A4-F5-P1 |  |
| KIT-A22-F8-P1 | **Private usage counts**: Usage counts in totals only, with no tracking cookies or personal profiles. | service | medium | KIT-A22-F1-P1 |  |
| KIT-A22-F9-P1 | **Crash reports**: Opt-in crash reports with personal data stripped out. | service | small | KIT-A22-F1-P1, KIT-A18-F17-P1 |  |
| KIT-A22-F10-P1 | **Many-machine installs**: Installs, updates and sets up apps on many machines at once through an organisation's own device tools and set-up files. Spare Construct and Spare OS both need it. | integration | medium | KIT-A20-F2-P5, KIT-A1-F7-P3 | [OS06](../spare-os/STATUS.md) |

## KIT-A23 Work law and worker status

One worker-status framework for every app: employee or contractor under the law's tests, agreements that meet them, personal grievances, minimum pay, holidays and leave, payroll, KiwiSaver and ACC guidance, switched on by each app's settings. Proposed as svc-work (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A23-F1** Know whether I am an employee or a contractor | Know my status in each app, why, and what it means for my pay, tax, vote and membership. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A23-F2** Get paid what the law says | Know my earnings meet the minimum the law sets for my status, with holiday pay and payroll handled where I am an employee. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A23-F3** Know platform work law in other countries | Know what platform work law asks in each country an app opens in. | later | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A23-F1-P1 | **Worker status test**: Applies NZ employment law's tests, including the new gateway test for contractors (in force date *unverified*), to each app's way of working, checked by a lawyer. | legal | medium |  |  |
| KIT-A23-F1-P2 | **Worker status per app**: Each app's settings declare its workers' status, which switches on the matching pay, tax, agreement and membership rules. | data | small | KIT-A23-F1-P1, KIT-A1-F5-P2 |  |
| KIT-A23-F1-P3 | **Contractor agreements**: Plain contractor agreements that meet the gateway test, versioned, with each worker's acceptance recorded. | legal | medium | KIT-A23-F1-P1, KIT-A18-F4-P1 |  |
| KIT-A23-F1-P4 | **Employment agreements**: Written employment agreements for apps whose workers are employees, as the Employment Relations Act 2000 requires, versioned, with each worker's signed copy kept. | legal | medium | KIT-A23-F1-P1, KIT-A18-F4-P1 |  |
| KIT-A23-F1-P5 | **Personal grievances**: A way for employee workers to raise a personal grievance within the 90-day limit, with mediation and every step recorded. | service | small | KIT-A23-F1-P4, KIT-A21-F2-P1 |  |
| KIT-A23-F1-P6 | **Union access**: How unions reach and meet workers who work only through an app, under the Employment Relations Act's access rules (*unverified* how they apply to an app). | legal | small | KIT-A23-F1-P1 |  |
| KIT-A23-F2-P1 | **Minimum pay top-up**: Checks earnings per hour of engaged time against the minimum wage where it applies, and tops them up in the next payout. | library | medium | KIT-A23-F1-P2, KIT-A3-F3-P2 |  |
| KIT-A23-F2-P2 | **Holidays and leave**: Works out annual holidays, public holiday pay, and sick, bereavement and family violence leave for apps whose workers are employees. | library | medium | KIT-A23-F1-P2, KIT-A19-F9-P2 |  |
| KIT-A23-F2-P3 | **Payroll provider link**: The one payroll provider link, for co-op staff and employee workers alike, which handles PAYE, KiwiSaver and payday filing. | integration | medium | KIT-A23-F1-P2, KIT-A3-F3-P2 |  |
| KIT-A23-F2-P4 | **KiwiSaver, ACC and tax guidance**: Plain guidance for each status on KiwiSaver, ACC levies and income tax, linked from the earnings page. | data | small | KIT-A23-F1-P1 |  |
| KIT-A23-F2-P5 | **Move to the Employment Leave Act**: The move from the Holidays Act 2003 to the Employment Leave Act 2026's hours-based leave, which applies from the first pay period on or after 6 August 2028, with agreements updated in time. | legal | small | KIT-A23-F2-P2 |  |
| KIT-A23-F2-P6 | **Wage and time records**: The wage, time, holiday and leave records employment law requires for employee workers, kept from their shift and online records. Spare Food and Spare Ride both need them. | service | small | KIT-A23-F2-P2, KIT-A24-F3-P2 |  |
| KIT-A23-F3-P1 | **Platform work law per country**: Laws on platform work overseas, such as the EU Platform Work Directive and Australia's minimum standards, and what each changes in matching, ranking and pausing. Spare Ride, Spare Food and Spare Work need them. | legal | medium | KIT-A23-F1-P1, KIT-A18-F13-P1 |  |

## KIT-A24 Health and safety at work

The co-op's health and safety duties, built once: an incident register, notifiable event reports, work-time limits, help after violence or a crash, and hazard notes. Proposed as svc-work (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A24-F1** Know the co-op's safety duties | Know what health and safety duties the co-op owes me while I work through an app, whatever my status. | core | P1 |
| **KIT-A24-F2** Record and report an incident | Record an injury, near miss, assault or crash at work, and know it is reported to the regulator when the law says. | core | P1, P2 |
| **KIT-A24-F3** Not be worked to exhaustion | Have the app stop offering me work when I reach a fatigue or work-time limit. | core | P1, P2 |
| **KIT-A24-F4** Get help after violence or a crash at work | Get safety, support and the right reports started after an assault or crash during a job. | core | P1 |
| **KIT-A24-F5** Know the hazards of a job | See hazards to know before a job, such as a dog, steep stairs or a dangerous site. | core | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A24-F1-P1 | **Health and safety duties**: The co-op's duties as a person conducting a business or undertaking under the Health and Safety at Work Act 2015, for workers whatever their status (platform position *unverified*). | legal | medium | KIT-A23-F1-P1 |  |
| KIT-A24-F2-P1 | **Incident register**: Records injuries, near misses, violence and crashes during work as operations, with follow-up actions. | service | medium | KIT-A24-F1-P1, KIT-A10-F3-P1 |  |
| KIT-A24-F2-P2 | **Notifiable event reports**: Flags events that must be notified to WorkSafe and tracks the notice and the duty to keep the scene as it was. | service | small | KIT-A24-F2-P1 |  |
| KIT-A24-F3-P1 | **Work-time limits**: Hooks for each app's fatigue and work-time limits, such as online hours and breaks, that pause offers when a limit is reached. | library | small | KIT-A24-F1-P1, KIT-A6-F2-P1, KIT-A24-F3-P2 |  |
| KIT-A24-F3-P2 | **Shared work-time record**: One record of each person's online and engaged time across every Spare app, so work-time limits count all their work and nobody types it twice. | data | small | KIT-A6-F2-P1, KIT-A18-F6-P1 |  |
| KIT-A24-F4-P1 | **Violence and crash handling**: Steps after an assault or crash during a job: safety first, support, the incident record, insurance and any notice to the regulator. | service | medium | KIT-A24-F2-P1, KIT-A10-F1-P2 |  |
| KIT-A24-F5-P1 | **Hazard notes**: Hazard notes on a place or listing, added by customers or past workers and shown before a job is accepted. | library | small | KIT-A24-F1-P1, KIT-A5-F1-P1 |  |

## KIT-A25 Signatures, forms, notices and training

Tools many apps need that no single app should own: electronic signatures with an audit trail, one form model, formal notices served the way the law asks, and one module for courses, quizzes and certificates. Proposed as svc-sign and svc-training (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A25-F1** Sign a document | Sign an agreement in any app and get a copy with a record of who signed and when. | core | P1, P2 |
| **KIT-A25-F2** Take a course and keep my certificates | Take a short course, pass a quiz and keep a certificate that apps and member roles can check. | core | P1, P2 |
| **KIT-A25-F3** Fill in a form | Build or fill in a survey, quiz, application or request form that works the same in every app. | core | P1 |
| **KIT-A25-F4** Serve a formal notice | Send a notice the law requires, such as a tenancy or sale notice, and prove how and when it arrived. | core | P1, P2, P3, P4 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A25-F1-P1 | **Electronic signature rules**: What NZ law asks of an electronic signature, and which documents need more than a click. | research | small |  |  |
| KIT-A25-F1-P2 | **Signatures with audit trail**: Signing flows for any app, with a tamper-evident audit trail and a signed PDF copy for every party. | service | medium | KIT-A25-F1-P1, KIT-A15-F1-P2, KIT-A1-F1-P1, engine-pdf |  |
| KIT-A25-F2-P1 | **Training and certificates**: Courses, quizzes, completion records and renewals for any role in any app, such as worker orientation, moderators and panel members. | service | medium | KIT-A1-F5-P2, KIT-A15-F1-P2 |  |
| KIT-A25-F2-P2 | **Course screens**: Shared screens to take a course and show a certificate. | interface | small | KIT-A25-F2-P1, KIT-A20-F1-P1 |  |
| KIT-A25-F3-P1 | **Form model and builder**: One form model and builder for surveys, quizzes, applications and request forms, with question types, logic and a preview on phone and computer. Spare Office and Spare Studio both map form builders; whether Spare Office's moves here is OFF-A9-F1-P6. | library | medium | KIT-A15-F1-P3, KIT-A20-F1-P1, KIT-A18-F5-P1 |  |
| KIT-A25-F4-P1 | **Address for service record**: Each party's agreed email and postal addresses for service, with the date each was given, used by agreements and notices. Spare Rent and Spare Home both need it. | data | small | kit-ops, KIT-A1-F5-P1 |  |
| KIT-A25-F4-P2 | **Notice builder**: Fills a notice from the agreement, works out the dates and renders an accessible PDF. Spare Rent's tenancy notices and Spare Home's sale notices to tenants use it. | library | medium | KIT-A27-F6-P3, KIT-A19-F9-P2 |  |
| KIT-A25-F4-P3 | **Proof of service**: Records how a notice was served, by the agreed email, post or hand, and when it counts as received; delivery in the app alone is not service. | service | small | KIT-A25-F4-P1, KIT-A8-F17-P1, KIT-A8-F4-P1 |  |
| KIT-A25-F4-P4 | **Notice clocks**: Legal clocks for notices on the kit's working days, adding the extra days each way of serving allows. | library | small | KIT-A19-F9-P2, KIT-A15-F14-P1, KIT-A25-F4-P3 | [RT03](../spare-rent/STATUS.md) |

## KIT-A26 Property records, ownership and tenancy rules

One shared record for each property with its chattels and repairs, proof of who owns or acts for it, decisions between co-owners, hand-over to a new owner, viewings, simple floor plans, owner statements and one pack of tenancy law, used by Spare Home, Spare Rent and Spare Stay so none of them depends on another. Built in kit-market as shared data; the document vault is KIT-A15-F13.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A26-F1** Keep one record for a property | Keep one record of a property's land, title, buildings, rooms, access facts, rates, past jobs and past sales that every app working on it shares. | core | P1, P2, P3, P4, P5, P6 |
| **KIT-A26-F2** Record who came to a viewing | Sign visitors in at an open home or viewing, and know who came and when. | core | P1 |
| **KIT-A26-F3** Have tenancy law applied the same way everywhere | Have tenants' rights, entry notices and notice periods applied the same way whether a home is rented out or sold. | core | P1, P2 |
| **KIT-A26-F4** Prove I own or control a property | Prove I own or control a place before I list it, so nobody can list a home that is not theirs. | core | P1, P2, P3, P4 |
| **KIT-A26-F5** Act for a property as a trust, estate or company | Sell, buy or let as joint owners, a trust, an estate or a company, with each person's part clear. | core | P1, P2, P3 |
| **KIT-A26-F6** Decide together as co-owners | Bring every owner in and have the big steps agreed by all of them. | core | P1, P2 |
| **KIT-A26-F7** Hand a property to a new owner | Pass a property's record, documents and any tenancy to its new owner when it sells. | core | P1 |
| **KIT-A26-F8** Book a viewing | Book a viewing or open home, in person or by live video. | core | P1, P2 |
| **KIT-A26-F9** Draw a simple floor plan | Draw or bring in a simple floor plan with room sizes, with no design skills needed. | core | P1, P2, P3 |
| **KIT-A26-F10** Send statements to owners when a manager runs the place | Give owners a monthly statement when a manager runs their place. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A26-F1-P1 | **Property record**: Land, title, buildings, rooms, accessibility facts, rates, zoning, past jobs and past sales for one place, every change an operation, with each app adding its own fields. | data | large | kit-core, kit-ops, kit-store, KIT-A7-F2-P2, KIT-A5-F1-P1 |  |
| KIT-A26-F1-P2 | **Property data sources**: Which official land, title, rates and zoning data the record may load, such as LINZ title and parcel data and council rating information, under what licence and at what cost, done once for every app. | research | medium |  |  |
| KIT-A26-F1-P3 | **Who sees what, and what passes on**: Rules for who may see each part of the record, such as an owner, tenant, guest or worker, and what passes to a new owner on sale, so private details and past jobs never reach the wrong person. | library | medium | KIT-A26-F1-P1, KIT-A15-F1-P4, KIT-A18-F6-P1 |  |
| KIT-A26-F1-P4 | **Chattels and appliances**: One register of the rooms, chattels and appliances in a place, with make, model, manuals, warranties and service dates. Spare Home, Spare Rent and Spare Work's completion packs use it. | data | small | KIT-A26-F1-P1, KIT-A15-F13-P1 |  |
| KIT-A26-F1-P5 | **Repairs and maintenance history**: Problems, repairs, costs and records of work for each place, linked to reports and jobs, and passed to the next owner where the rules allow. Spare Rent, Spare Stay and Spare Work need it. | data | small | KIT-A26-F1-P1, KIT-A26-F1-P3 |  |
| KIT-A26-F1-P6 | **Body corporate details**: The body corporate's contact, levies and rules stored against a unit. Spare Home and Spare Rent both need them. | data | small | KIT-A26-F1-P1 |  |
| KIT-A26-F2-P1 | **Visitor register**: Records who came to an open home or viewing and when, from a check-in code or the host's phone, kept only as long as the retention schedule allows. | service | small | kit-ops, KIT-A26-F1-P1, KIT-A18-F6-P1 |  |
| KIT-A26-F3-P1 | **Tenancy rules pack**: The tenancy rules Spare Rent and Spare Home both need, such as written notice to tenants, consent before entry, rights about photos and viewings, and notice periods after a sale, as one versioned pack in the NZ country pack. | data | medium | KIT-A19-F9-P1, KIT-A19-F9-P2, KIT-A26-F3-P2 | [RT01](../spare-rent/STATUS.md) |
| KIT-A26-F3-P2 | **Tenancy law check**: A lawyer checks the pack against the Residential Tenancies Act 1986, and again whenever the law changes. | legal | small |  |  |
| KIT-A26-F4-P1 | **Owner name match**: Matches a verified name against the owners on the record of title and flags any mismatch, though a name match alone never proves ownership. Spare Home and Spare Rent share one check. | service | small | KIT-A26-F1-P2, KIT-A2-F1-P3 |  |
| KIT-A26-F4-P2 | **Proof of control**: Confirms the person controls the place through the rates assessment number, their lawyer or a code posted to the property, and never by a posted code alone for a home someone else lives in. | service | small | KIT-A26-F4-P1, KIT-A2-F14-P1, KIT-A2-F1-P3 |  |
| KIT-A26-F4-P3 | **Notice to the owner of record**: Posts a letter with a code to the owner's address of record whenever a listing starts, telling them how to stop it, where that address may lawfully be used (*unverified*). | service | small | KIT-A8-F17-P1, KIT-A26-F1-P2 |  |
| KIT-A26-F4-P4 | **Ownership review**: A trained member settles the cases the automatic checks cannot, through the one review queue, with every decision logged. | service | small | KIT-A10-F6-P2, KIT-A26-F4-P1 |  |
| KIT-A26-F5-P1 | **Owning parties**: Joint owners, trusts, estates and companies as one owning or buying party, with each person's identity and role. Spare Home and Spare Rent both need it. | data | medium | KIT-A1-F7-P1, KIT-A2-F1-P3 |  |
| KIT-A26-F5-P2 | **Kinds of authority**: Each kind of person acting for an owner, such as a trustee, executor, attorney or company director, and the evidence each must give, per country. | legal | small |  |  |
| KIT-A26-F5-P3 | **Authority evidence check**: Collects trust deeds, probate, powers of attorney and company records, keeps only the reviewed result and deletes the documents. | service | small | KIT-A26-F5-P2, KIT-A10-F6-P2, KIT-A18-F6-P1 |  |
| KIT-A26-F6-P1 | **Co-owner invitations**: Invites each owner on the title to join and confirm who they are and their share. Spare Home and Spare Rent both need it. | service | small | KIT-A26-F5-P1, KIT-A8-F3-P2 |  |
| KIT-A26-F6-P2 | **Every-owner approvals**: Sets which steps need every owner to agree, such as going live, changing the price or accepting an offer. | library | small | KIT-A26-F6-P1, kit-ops |  |
| KIT-A26-F7-P1 | **Property hand-over**: Passes the record, plans, documents and appliance details to the new owner when the seller agrees, tells tenants who their new landlord is and guides the bond transfer, the old owner keeping only their own accounts. Spare Home and Spare Rent share one flow. | service | medium | KIT-A26-F1-P3, KIT-A15-F13-P1, KIT-A26-F3-P1, KIT-A25-F4-P2 |  |
| KIT-A26-F8-P1 | **Viewing slots**: Open homes and private viewing slots with limits on numbers, the lister's approval if chosen, and the exact address shared only once booked. Spare Home and Spare Rent use them, and Spare Market's item viewings can too. | service | medium | KIT-A5-F5-P1, KIT-A7-F10-P1, KIT-A26-F2-P1 |  |
| KIT-A26-F8-P2 | **Live video viewings**: Books and runs a live video viewing on the kit's call service, with no recording unless both agree. | service | small | KIT-A26-F8-P1, KIT-A8-F11-P2 |  |
| KIT-A26-F9-P1 | **Simple plan drawer**: Lets a person trace rooms with set sizes, doors and windows with no design skills needed, on the shared 2D drawing engine. Spare Home, Spare Rent and Spare Stay share one drawer. | interface | medium | engine-draw2d, KIT-A26-F1-P1, kit-ops |  |
| KIT-A26-F9-P2 | **Plan import**: Imports a plan exported in an open format, such as one from Spare Construct, or a PDF or image with its copyright owner and licence recorded. | library | small | KIT-A26-F9-P1, engine-pdf | [HM04](../spare-home/STATUS.md) |
| KIT-A26-F9-P3 | **Plan output and room areas**: Draws a plan as an image, a PDF and a plain text room list for screen readers, and works out room and floor areas, stating how they were measured. | library | small | KIT-A26-F9-P1, engine-pdf, engine-text |  |
| KIT-A26-F10-P1 | **Owner statements**: Monthly statements for owners whose places a manager runs: income, payouts, costs, the manager's at-cost fee and GST. Spare Stay and Spare Rent both need them. | service | small | KIT-A4-F4-P1, KIT-A1-F8-P1, KIT-A27-F6-P3 |  |

## KIT-A27 Shared engines

The heavy engines several programs share: GPU rendering, 2D drawing, text, PDF, and video and audio, plus two proposed engines for office files and AI models run on the device. They are built in projects/engines, not in the kit, and depend only on the kit; they are mapped here so every program has one place to depend on and none depends on another. Names other than the five existing engines are proposals (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A27-F1** See 3D and 2D views drawn fast | Open and move around large models, drawings and designs smoothly, on a computer or in a browser. | core | P1, P2, P3, P4 |
| **KIT-A27-F2** Edit photos on the device | Edit a photo quickly on my phone or computer, with the same tools in every app. | core | P1, P2, P3 |
| **KIT-A27-F3** Draw lines, shapes and charts | Draw precise lines, shapes and charts that look the same on screen, on paper and in a PDF. | core | P1, P2, P3 |
| **KIT-A27-F4** Think together on a whiteboard | Sketch, write and pin ideas on one endless board with other people. | next | P1 |
| **KIT-A27-F5** Set text in any language | Read and write text in any language and script, spelled and laid out properly. | core | P1, P2, P3, P4, P5 |
| **KIT-A27-F6** Read, write and sign PDFs | Open, make, mark up, sign and print PDFs that work everywhere and with screen readers. | core | P1, P2, P3, P4, P5, P6, P7, P8 |
| **KIT-A27-F7** Remove what should not be shared | Remove hidden data and redact content for good before a file is shared. | next | P1 |
| **KIT-A27-F8** Play, edit and record video and audio | Play, record and edit video and sound on any device, with captions and a record of how it was made. | core | P1, P2, P3, P4, P5, P6, P7 |
| **KIT-A27-F9** Stream live to many people | Stream a live event or video to thousands of viewers with a short delay. | later | P1 |
| **KIT-A27-F10** Open office files anywhere | Open and save common document, spreadsheet, slide and mail files in any app that needs them. | core | P1, P2, P3 |
| **KIT-A27-F11** Make many documents from data | Make one page, letter or design for each row of a sheet or contact list. | next | P1 |
| **KIT-A27-F12** Run AI models on the device | Have text read from pictures, speech turned into text and text read aloud on my own device, with nothing sent away unless I choose. | core | P1, P2, P3, P4, P5 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A27-F1-P1 | **Renderer choice**: Our own renderer on wgpu, or a web renderer first, covering instancing, culling and browser support, decided once for Spare Construct and Spare Studio. | research | medium |  | [A04](../spare-construct/STATUS.md) |
| KIT-A27-F1-P2 | **GPU renderer**: The engine-render renderer for 3D and 2D views: instancing, levels of detail, GPU culling and positions relative to the camera, compiled native and to WebAssembly. | library | large | KIT-A27-F1-P1, kit-core |  |
| KIT-A27-F1-P3 | **3D viewport**: A fast viewport with lighting, materials and camera controls, shared by Spare Construct and Spare Studio, with a plain fallback for slow devices that Spare Home's tours can use. | library | medium | KIT-A27-F1-P2 |  |
| KIT-A27-F1-P4 | **Path-traced rendering**: Realistic path-traced still images with an open denoiser, shared by Spare Construct and Spare Studio. | library | large | KIT-A27-F1-P2 |  |
| KIT-A27-F2-P1 | **GPU image pipeline**: Tiles, filters, blending and adjustments as GPU compute with a CPU fallback, one image pipeline for Spare Studio and the photo tools in other apps. | library | large | KIT-A27-F1-P2 |  |
| KIT-A27-F2-P2 | **Simple photo editor**: Crop, rotate, straighten, exposure, colour and a before-and-after view on phone and web, built on the image pipeline. Spare Social, Spare Market, Spare OS and the listing apps share it. | interface | medium | KIT-A27-F2-P1, KIT-A20-F1-P1 |  |
| KIT-A27-F2-P3 | **Colour management**: ICC profile transforms for photo and print and OpenColorIO for video, keeping colour profiles and HDR correct from camera to screen. Spare Studio, Spare Social and Spare OS need it. | library | medium | KIT-A27-F2-P1 | [S03](../spare-studio/STATUS.md) |
| KIT-A27-F3-P1 | **2D drawing core**: The engine-draw2d core: lines with true weights, fills, patterns, paths and shapes drawn on the GPU, used by Spare Construct drawings, Spare Studio vectors and Spare Office shapes. | library | large | kit-core, KIT-A27-F1-P2 |  |
| KIT-A27-F3-P2 | **Path booleans**: Robust 2D boolean operations on curves, shared by Spare Studio and Spare Construct. | library | medium | KIT-A27-F3-P1 |  |
| KIT-A27-F3-P3 | **Chart engine**: Bar, line, pie, area, scatter and other charts drawn from data, each with a text description and a data table for screen readers. Spare Office, Spare Studio, the ledger pages and seller insights share it. | library | medium | KIT-A27-F3-P1, KIT-A27-F5-P1 |  |
| KIT-A27-F4-P1 | **Whiteboard canvas**: An endless canvas with ink, shapes, sticky notes and images, edited together, shared by Spare Office's whiteboard and Spare Studio's mood boards. | library | medium | KIT-A27-F3-P1, KIT-A15-F5-P1 |  |
| KIT-A27-F5-P1 | **Text shaping and layout**: Font loading, shaping, bidirectional and complex scripts, vertical text, colour and emoji fonts, line breaking and paragraph layout, shared by Spare Studio, Spare Office, Spare Construct and Spare Social. | library | large | kit-core | [S05](../spare-studio/STATUS.md) |
| KIT-A27-F5-P2 | **Spelling and hyphenation dictionaries**: Open dictionaries for each language, te reo Māori and English (New Zealand) first, and Pacific languages built with native speakers, each licence checked, used by every app's spell checker. | data | medium | KIT-A27-F5-P1, KIT-A18-F16-P1 |  |
| KIT-A27-F5-P3 | **Input methods for drawn text**: IME composition, dead keys, compose keys and easy macron typing for text drawn on a canvas, on desktop and in the browser. Spare Studio, Spare Office and Spare Construct need it. | library | medium | KIT-A27-F5-P1 |  |
| KIT-A27-F5-P4 | **Open font catalogue**: One catalogue of fonts under open licences, such as the SIL Open Font License, covering every major script, with what each licence allows for embedding, print and the web. Spare Studio, Spare Office, Spare OS and Spare Construct share it. | data | medium | KIT-A27-F5-P1 |  |
| KIT-A27-F5-P5 | **Long-document layout**: Fields, footnotes, tables of contents, indexes, cross-references, headers and footers, and accessible EPUB export, shared by Spare Studio's layout tools and Spare Office's documents. | library | large | KIT-A27-F5-P1 |  |
| KIT-A27-F6-P1 | **PDF reader and writer**: Reads, views, edits and writes PDF to ISO 32000-2 with embedded fonts, layers and bookmarks, used by every app that makes or shows a PDF. | library | large | kit-core, KIT-A27-F5-P1 |  |
| KIT-A27-F6-P2 | **Tagged, archival and print PDF**: Tagged PDF/UA, archival PDF/A and print-ready PDF/X output, with headings, reading order, table headers and alt text passed in by each app. | library | medium | KIT-A27-F6-P1 |  |
| KIT-A27-F6-P3 | **Documents from templates**: Fills a versioned template from an app's data and renders an accessible PDF, for agreements, notices, statements, receipts, brochures and packing slips. Spare Rent, Spare Work, Spare Home, Spare Ride and Spare Market all make documents this way. | library | medium | KIT-A27-F6-P2, KIT-A19-F1-P2 |  |
| KIT-A27-F6-P4 | **PDF markup**: Clouds, text, arrows, stamps and every standard PDF annotation type, read and written to the specification. Spare Construct, Spare Studio and Spare Office share it. | library | medium | KIT-A27-F6-P1 |  |
| KIT-A27-F6-P5 | **Certificate signatures**: Digital certificate signatures with long-term validation and a trust store, shared by Spare Studio, Spare Office and the kit's signing flow. | library | medium | KIT-A27-F6-P1, KIT-A25-F1-P1 |  |
| KIT-A27-F6-P6 | **Page recovery**: Rebuilds paragraphs, columns, tables and images from PDF pages, for PDF to layout and PDF to office files. Spare Studio and Spare Office share it. | library | large | KIT-A27-F6-P1, KIT-A27-F5-P1 |  |
| KIT-A27-F6-P7 | **Scan to PDF**: Edge finding, perspective fixing and clean-up for photos of paper, with a searchable text layer from text recognition. Spare Studio, Spare Office, Spare OS, Spare Home and Spare Rent need it. | library | medium | KIT-A27-F6-P1, KIT-A27-F12-P3 |  |
| KIT-A27-F6-P8 | **Printing**: Sends what an app shows to the system's print service through engine-pdf, with preview, page choices, paper sizes and scaling. Spare Construct, Spare Studio, Spare Office and Spare OS need it. | library | small | KIT-A27-F6-P1 |  |
| KIT-A27-F7-P1 | **True redaction and hidden data removal**: Removes chosen content for good, including any text under it, and strips hidden data, metadata and scripts before a file is shared. Spare Construct, Spare Studio and Spare Office need it. | library | medium | KIT-A27-F6-P1, KIT-A27-F10-P1 |  |
| KIT-A27-F8-P1 | **Codec and patent study**: One study of which video, audio and image formats to decode and encode and how to avoid patent and licence risk, merging Spare Studio's, Spare Social's and Spare OS's codec research. | research | medium |  | [S04](../spare-studio/STATUS.md) |
| KIT-A27-F8-P2 | **Decoding and encoding**: Decodes and encodes the chosen formats, using hardware where the platform offers it, with image sizes and short videos compressed for phones. Spare Studio, Spare Social, Spare OS and every listing app need it. | library | large | KIT-A27-F8-P1, kit-core |  |
| KIT-A27-F8-P3 | **Timeline and keyframes**: One timeline model for tracks, clips and edits as operations, with keyframes and easing for every property, shared by Spare Studio's apps, Spare Social's short video editor and Spare Office's slide animation. | library | large | KIT-A27-F8-P2, kit-ops |  |
| KIT-A27-F8-P4 | **Audio engine**: Low-latency playback, recording and mixing on every platform, with even loudness, shared by Spare Studio and Spare Social. | library | large | KIT-A27-F8-P2 |  |
| KIT-A27-F8-P5 | **Captions and extra tracks**: Reads and writes open caption and subtitle formats, and carries audio description and extra language tracks. Spare Studio, Spare Social and the kit's captions share it. | library | medium | KIT-A27-F8-P2, KIT-A19-F4-P1 |  |
| KIT-A27-F8-P6 | **Content credentials**: Reads and signs C2PA content credentials with an openly licensed library, so edits and AI use stay visible. Spare Studio, Spare Social and Spare Home need it. | library | medium | KIT-A27-F8-P2 |  |
| KIT-A27-F8-P7 | **Signing certificate for content credentials**: How the co-op gets, stores and rotates a certificate from a provider on the C2PA trust list, done once for Spare Studio and Spare Social. | research | small | KIT-A27-F8-P6 |  |
| KIT-A27-F9-P1 | **Low-delay live streaming**: Streams live video to many viewers with a short delay, shared by Spare Social's live video and Spare Office's large events. | service | large | KIT-A27-F8-P2 |  |
| KIT-A27-F10-P1 | **Office formats engine**: Reads and writes DOCX, XLSX, PPTX, ODT, ODS, ODP and RTF from their published standards, below Spare Office, Spare Studio and Spare Construct's spreadsheet export, so none depends on another. Proposed as engine-formats (KIT-A16-F10-P1). | library | large | KIT-A27-F5-P1, kit-core | [OF02](../spare-office/STATUS.md) |
| KIT-A27-F10-P2 | **Mail and notebook file readers**: Reads Outlook data files, saved messages and notebook files from Microsoft's published specifications, for Spare Office's importers and Spare OS's move from Windows. | library | medium | KIT-A27-F10-P1 |  |
| KIT-A27-F10-P3 | **File encryption**: Reads and writes password encryption for office files and PDFs from their published specifications. Spare Office and Spare Studio both need it. | library | small | KIT-A27-F10-P1, KIT-A27-F6-P1 |  |
| KIT-A27-F11-P1 | **Data merge**: Makes a page, record or message for each row of a sheet, CSV file or contact group, with previews, for Spare Studio's data merge and Spare Office's mail merge. | library | medium | KIT-A27-F10-P1 |  |
| KIT-A27-F12-P1 | **On-device model runtime**: Runs open models on the processor, graphics chip or AI chip, for text recognition, speech, voices and image tasks. Spare Studio, Spare OS, Spare Social and Spare Office need it. Proposed as engine-models (KIT-A16-F10-P1). | library | large | kit-core, KIT-A17-F6-P1 |  |
| KIT-A27-F12-P2 | **Open model choices**: Which open models for text recognition, speech recognition and voices may be used under their licences, how accurate each is, and how te reo Māori models follow Māori data principles, decided once for every program. | research | medium | KIT-A18-F16-P1, KIT-A17-F6-P1 |  |
| KIT-A27-F12-P3 | **Text recognition**: Recognises printed and handwritten text in photos, scans and receipts on the device, for search, copying, scanning and receipt reading. Spare Office, Spare Studio, Spare OS and Spare Rent need it. | library | medium | KIT-A27-F12-P1, KIT-A27-F12-P2 |  |
| KIT-A27-F12-P4 | **Speech recognition**: Speech to text for captions, transcripts, dictation and voice control, on the device or on co-op servers at cost, te reo Māori and English first. Spare Social, Spare Office, Spare Studio and Spare OS need it, and the kit's captions use it. | library | medium | KIT-A27-F12-P1, KIT-A27-F12-P2 |  |
| KIT-A27-F12-P5 | **Text-to-speech voices**: Natural voices in many languages from open models, for read aloud, screen readers and offers read to drivers, with te reo Māori voices used only on terms that respect Māori data sovereignty. Spare Office, Spare OS, Spare Studio and Spare Ride need them. | library | medium | KIT-A27-F12-P1, KIT-A27-F12-P2, KIT-A18-F16-P1 |  |

## KIT-A28 Own sites, public pages and feeds

Helps members reach people outside the apps: their own web address, fast public pages that web search can find, listing feeds to other sites, simple shop and booking sites, and a customer list with newsletters, all on the same checkout and seller of record rule. Built on kit-market and the file service; the service name is open (KIT-A16-F10-P1).

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **KIT-A28-F1** Use my own web address | Use my own domain for my shop, site, email or profile, with security set up for me. | core | P1, P2, P3 |
| **KIT-A28-F2** Be found through web search | Have my public listing found through web search and shared with a clear preview. | core | P1, P2, P3 |
| **KIT-A28-F3** Send my listings to other sites | Show my listing on other property, travel and shopping sites from one place. | next | P1, P2 |
| **KIT-A28-F4** Run my own shop or booking site | Have a simple site in my own name and colours that takes bookings or orders through Spareware. | next | P1, P2 |
| **KIT-A28-F5** Keep a customer list and send newsletters | Keep my own list of customers and send news to those who asked for it. | next | P1, P2 |
| **KIT-A28-F6** Buy a web address at cost | Buy a domain name through Spareware at the registrar's own price. | next | P1 |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| KIT-A28-F1-P1 | **Domain verification**: Proves an account or organisation owns a domain through a DNS record or a file on the site before it is used, such as for an organisation's email and sign-in, a shop's address or a profile handle. Spare Office, Spare Social, Spare Market and Spare Stay need it. | service | small | KIT-A1-F1-P1, KIT-A1-F7-P1 |  |
| KIT-A28-F1-P2 | **Custom domains**: Connects a member's own domain to their shop, booking site or portfolio, checks its records and issues free security certificates automatically. | service | medium | KIT-A28-F1-P1, KIT-A22-F4-P1 | [MK06](../spare-market/STATUS.md) |
| KIT-A28-F1-P3 | **Free web addresses**: A free address for members without a domain, on a separate domain listed on the Public Suffix List so one member's site cannot read another's. | service | small | KIT-A28-F1-P2 |  |
| KIT-A28-F2-P1 | **Public listing pages**: Fast, server-rendered pages for every public listing, place and region that anyone can open without signing in, and that keep working after a listing ends. Spare Home, Spare Rent, Spare Stay and Spare Market need them. | service | medium | KIT-A5-F1-P1, KIT-A7-F10-P1, KIT-A19-F5-P1 |  |
| KIT-A28-F2-P2 | **Web search basics**: Page titles, descriptions, structured listing data in an open vocabulary, site maps and tidy addresses for every public page. | library | small | KIT-A28-F2-P1 |  |
| KIT-A28-F2-P3 | **Share cards**: Preview cards with a photo, the title and the full price when a public link is shared, built without tracking who opens them. | library | small | KIT-A28-F2-P1, KIT-A5-F3-P2 |  |
| KIT-A28-F3-P1 | **Where listings can go**: Which property, travel and shopping sites accept listings by feed, at what cost and on what terms. | research | small |  | [HM02](../spare-home/STATUS.md) |
| KIT-A28-F3-P2 | **Listing feed service**: One service that sends listings, changes and withdrawals to other sites, and publishes an open feed of public listings and full prices, with each site's format as a plugin. Spare Home, Spare Rent, Spare Stay and Spare Market share it. | service | medium | KIT-A28-F3-P1, KIT-A5-F1-P1, KIT-A16-F5-P2 |  |
| KIT-A28-F4-P1 | **Own site builder**: A simple site in the member's own logo and colours, with a set-up guide, that uses the same listings, checkout and seller of record rule as the app. Spare Food, Spare Market and Spare Stay need it. | service | large | KIT-A28-F1-P2, KIT-A28-F2-P1, KIT-A3-F1-P3, KIT-A1-F7-P8 | [MK06](../spare-market/STATUS.md) |
| KIT-A28-F4-P2 | **Booking box and embeds**: A small booking or ordering box and a QR code that members can put on a site they already have. Spare Ride, Spare Stay and Spare Food need it. | library | small | KIT-A28-F4-P1, KIT-A20-F9-P1 |  |
| KIT-A28-F5-P1 | **Customer records**: A seller's own list of customers with their orders and consent, kept to what privacy law allows and never shared with other sellers. Spare Market and Spare Work need it. | service | small | KIT-A18-F17-P1, KIT-A5-F8-P1 |  |
| KIT-A28-F5-P2 | **Newsletters**: Simple newsletters and follower messages sent only to people who opted in, with a named sender and one-step unsubscribe, as anti-spam law requires. Spare Market and Spare Food need them. | service | small | KIT-A28-F5-P1, KIT-A8-F4-P1, KIT-A8-F5-P2 |  |
| KIT-A28-F6-P1 | **Domains bought at cost**: Whether the co-op can sell domain names at the registrar's price, and the rules for .nz names. Spare Office and Spare Market both ask. | research | small | KIT-A14-F18-P2 |  |

## Assessing this map and its demo

- Does the map cover every journey a member takes across the apps: first sign-up, a first job, being paid, a dispute, a vote, a meeting and leaving the co-op?
- Is the core set the smallest that gets the first marketplace and the member portal live, or is something marked core that can wait?
- Does any part here belong in only one program, or does any program hold something every app needs (the layering rule)?
- Is every place money moves run through a regulated payment provider, so the co-op never holds money itself?
- Which legal parts need a lawyer before Design starts, and in what order: legal entity, holding money, anti-money-laundering, GST, privacy, terms?
- Can a member trace any fee on a receipt back to real bills in the open cost ledger?
- Does a passed vote actually change an app's settings, and are quorum and electorates taken from the constitution?
- Are the safety tools enough for the first app's real risks (emergencies, reports, disputes) while removing only what the rules or the law require?
- Is every outside service and open-source project named here usable under its licence and terms, with no GPL or AGPL code in the core?
- Are te reo Māori, accessibility and data export built in from the first release rather than added later?
- Which parts have no work-board piece yet, and which should become the next pieces?
- Does the demo show each area's main screens in the design guide's look, with Spareware red only on co-op pages?
- What will the kit cost to run each month at launch, and does the ledger design show that cost clearly?
- Is member self-hosting needed for the first release, or can it wait?
- Are the proposed kit service and crate names (KIT-A16-F10-P1) the ones every program should depend on, and should the kit's ARCHITECTURE.md adopt them and state that kit services may use engines while kit-core never may?
- Which new legal parts must a lawyer settle before the first app goes live: worker status, health and safety, competition law on voted prices, the biometric code, seller of record and who carries losses?
- Does every hold fit inside the payment provider's time limit, including stays booked months ahead, staged work and buyer protection?
- Paying on account (KIT-A3-F17) and regular payments (KIT-A3-F12) are now core because Spare Office's core billing depends on them; should Spare Office's billing wait instead, so the first release stays small?
- Which core research and legal parts listed as waiting for a work-board piece should become kit pieces first?
- Kit core now includes every kit feature a program's core feature depends on; should some programs lower their own features instead, so the first release stays small?
- Should the programs that still map their own copies of kit parts switch to the kit's: translation frameworks (CON-A22-F4-P1, MKT-A19-F1-P1, OFF-A17-F8-P1, SOC-A21-F5-P1, STAY-A17-F2-P1, STU-A19-F3-P1), status pages (SOC-A22-F4-P3, MKT-A17-F7-P1), Office's chat server (OFF-A13-F1-P2), at-cost pricing (OFF-A20, STU-A1-F5, SOC-A22-F3) and the other copies the review listed?
- Should the kit pieces now tracked on other programs' boards (A05, A08, A14, MK01, MK04, MK05, OF05, M04 and RD03) move to the kit's own board?
- Can a member panel's decision move held money without binding consumers in a way the Arbitration Act forbids, and what do the order terms need to say?
- Is a member-funded protection pool an insurance business that needs a licence, and if so, is outside insurance the better route?
- Does the map now hold everything Spare Home, Rent and Stay share (property record, document vault, visitor register, tenancy rules), so none of them depends on another?
- The shared engines (KIT-A27) are mapped here though they are built in projects/engines; should the engines get a map of their own, and should engine-formats and engine-models be adopted as engine names?
- Programs still map their own copies of parts this pass added to the kit, such as floor plans (HOME-A3-F3-P1, RENT-A3-F2-P4, STAY-A2-F11-P1), formal notices (RENT-A9-F2-P2, RENT-A9-F2-P3, RENT-A16-F4-P1), the auction engine (MKT-A6-F1-P2), carts (FOOD-A4-F1-P1, MKT-A5-F5-P1), text recognition (OFF-A8-F3-P4, RENT-A14-F2-P1) and the office formats engine (STU-A12-F7-P4); should they now depend on the kit's parts instead?
- Some program parts say they belong in the kit but only one program maps them, so this pass left them where they are: in-person identity checks (HOME-A1-F2-P3), second-step set-up (HOME-A1-F8-P4), proof a pro is the registered person (HOME-A12-F2-P3), per-file media licences (HOME-A3-F8-P2), the separate file domain (OFF-A10-F14-P1), link safety checks (OFF-A21-F2-P3), bank feeds (RENT-A8-F2-P1), e-invoicing (WORK-A13-F6-P3), automation rules (MKT-A16-F8-P1), smart lock links (STAY-A9-F4-P1), age rules (SOC-A14-F1-P1), product security law (OS-A21-F3-P1) and encryption export rules (OS-A21-F8-P1). Should any move now?
- Should the pieces on other boards that now cover kit and engine parts (A04, S03, S04, S05, OF02, HM02, HM04, MK02, MK06, OS06 and RT03) move to the kit's own board?

## Work-board pieces used in this map

| Piece | Parts it covers |
| --- | --- |
| [A02](../spare-construct/STATUS.md) | KIT-A15-F6-P1 |
| [A04](../spare-construct/STATUS.md) | KIT-A27-F1-P1 |
| [A05](../spare-construct/STATUS.md) | KIT-A15-F4-P1, KIT-A15-F4-P2 |
| [A06](../spare-construct/STATUS.md) | KIT-A15-F6-P2 |
| [A08](../spare-construct/STATUS.md) | KIT-A16-F5-P1, KIT-A16-F5-P2 |
| [A14](../spare-construct/STATUS.md) | KIT-A14-F16-P1, KIT-A16-F4-P1 |
| [HM02](../spare-home/STATUS.md) | KIT-A28-F3-P1 |
| [HM04](../spare-home/STATUS.md) | KIT-A26-F9-P2 |
| [K01](STATUS.md) | KIT-A12-F1-P1, KIT-A12-F3-P1, KIT-A12-F5-P1, KIT-A14-F1-P1, KIT-A14-F1-P2, KIT-A14-F2-P1, KIT-A14-F5-P3, KIT-A14-F12-P1, KIT-A14-F14-P1, KIT-A14-F15-P1 |
| [K02](STATUS.md) | KIT-A3-F1-P1, KIT-A3-F1-P2, KIT-A3-F1-P5, KIT-A3-F2-P2, KIT-A3-F16-P1, KIT-A4-F5-P1 |
| [K03](STATUS.md) | KIT-A14-F10-P1 |
| [K04](STATUS.md) | KIT-A7-F6-P1, KIT-A8-F3-P1, KIT-A20-F2-P1, KIT-A20-F2-P2, KIT-A20-F2-P3, KIT-A20-F8-P1 |
| [K05](STATUS.md) | KIT-A5-F4-P3 |
| [K06](STATUS.md) | KIT-A13-F1-P1, KIT-A13-F1-P2, KIT-A13-F1-P4, KIT-A13-F3-P3, KIT-A13-F4-P1 |
| [M01](../spare-social/STATUS.md) | KIT-A1-F9-P1 |
| [M02](../spare-social/STATUS.md) | KIT-A10-F9-P1 |
| [M04](../spare-social/STATUS.md) | KIT-A10-F5-P1, KIT-A10-F6-P1, KIT-A10-F6-P2, KIT-A10-F7-P1 |
| [M07](../spare-social/STATUS.md) | KIT-A2-F8-P1, KIT-A10-F10-P1, KIT-A10-F10-P2 |
| [M09](../spare-social/STATUS.md) | KIT-A10-F13-P1 |
| [MK01](../spare-market/STATUS.md) | KIT-A3-F2-P1 |
| [MK02](../spare-market/STATUS.md) | KIT-A5-F25-P1, KIT-A18-F8-P1 |
| [MK04](../spare-market/STATUS.md) | KIT-A3-F13-P2, KIT-A8-F9-P1, KIT-A10-F12-P1, KIT-A10-F12-P2 |
| [MK05](../spare-market/STATUS.md) | KIT-A6-F1-P1, KIT-A6-F1-P2 |
| [MK06](../spare-market/STATUS.md) | KIT-A5-F10-P1, KIT-A28-F1-P2, KIT-A28-F4-P1 |
| [MK07](../spare-market/STATUS.md) | KIT-A3-F19-P2 |
| [OF02](../spare-office/STATUS.md) | KIT-A27-F10-P1 |
| [OF03](../spare-office/STATUS.md) | KIT-A15-F10-P1 |
| [OF05](../spare-office/STATUS.md) | KIT-A8-F1-P1, KIT-A8-F1-P2, KIT-A8-F11-P1 |
| [OS06](../spare-os/STATUS.md) | KIT-A1-F7-P3, KIT-A22-F10-P1 |
| [RD03](../spare-ride/STATUS.md) | KIT-A7-F3-P1, KIT-A7-F3-P2, KIT-A7-F3-P4 |
| [RT01](../spare-rent/STATUS.md) | KIT-A26-F3-P1 |
| [RT03](../spare-rent/STATUS.md) | KIT-A25-F4-P4 |
| [RT04](../spare-rent/STATUS.md) | KIT-A9-F2-P2 |
| [S03](../spare-studio/STATUS.md) | KIT-A27-F2-P3 |
| [S04](../spare-studio/STATUS.md) | KIT-A27-F8-P1 |
| [S05](../spare-studio/STATUS.md) | KIT-A27-F5-P1 |
| [S07](../spare-studio/STATUS.md) | KIT-A17-F6-P1 |
| [ST02](../spare-stay/STATUS.md) | KIT-A5-F5-P2 |
| [ST03](../spare-stay/STATUS.md) | KIT-A11-F9-P1 |
| [ST05](../spare-stay/STATUS.md) | KIT-A5-F3-P2 |
| [WK01](../spare-work/STATUS.md) | KIT-A3-F2-P3 |
| [WK02](../spare-work/STATUS.md) | KIT-A2-F2-P1 |
| [WK05](../spare-work/STATUS.md) | KIT-A4-F2-P1, KIT-A4-F2-P2 |
