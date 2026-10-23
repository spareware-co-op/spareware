# Spare Movies: everything to build

*Stage: Mapping. Mapped top-down on 2026-10-19. Generated from [features.json](features.json); edit that file, not this one. The architecture is in [ARCHITECTURE.md](ARCHITECTURE.md), the clickable demo in [demo/index.html](demo/index.html) and the work board in [STATUS.md](STATUS.md). Every choice is open until an owner accepts a decision note.*

Spare Movies replaces Netflix, Prime Video, Disney+ and film rental stores with one place where filmmakers sell their films and series the way they choose (rent, buy, pay what you like, free, or later a viewer pass) and viewers come to watch them. It is free to use and join, has no adverts, and runs at cost: filmmakers keep everything except running costs. The catalogue has Human and AI sections, only what the law requires is removed, and each country gets its own rules pack for money, tax, age ratings and rights. This map lists everything a full replacement needs, top down: 23 areas, 169 features a person can use, and 336 buildable parts. 120 features are core, meaning a first usable release needs them; the rest are next (soon after) or later. Each part names what it depends on: Spare Kit crates (kit-), kit services (svc-), shared engines (engine-), parts of Spare Kit by their KIT- ID (187 of them; see the [kit map](../kit/FEATURES.md)) or other Spare Movies parts by ID. No core part depends on a part marked next or later. Spare Movies has no work-board pieces yet, so where a part waits on research that another program already has on its board, the piece links to that board; the questions at the end list the research that still needs pieces of its own. Other companies are named only to say what we replace.

## Areas at a glance

| Area | Features | Core | Parts |
| --- | --- | --- | --- |
| [MOV-A1 Joining, accounts and profiles](#mov-a1-joining-accounts-and-profiles) | 8 | 6 | 18 |
| [MOV-A2 Browse, search and discovery](#mov-a2-browse-search-and-discovery) | 7 | 4 | 16 |
| [MOV-A3 Film and series pages](#mov-a3-film-and-series-pages) | 7 | 5 | 14 |
| [MOV-A4 The player](#mov-a4-the-player) | 12 | 7 | 21 |
| [MOV-A5 Human and AI sections](#mov-a5-human-and-ai-sections) | 7 | 6 | 16 |
| [MOV-A6 Paying for films: rent, buy, support or free](#mov-a6-paying-for-films-rent-buy-support-or-free) | 12 | 9 | 26 |
| [MOV-A7 The viewer pass](#mov-a7-the-viewer-pass) | 6 | 0 | 12 |
| [MOV-A8 Young viewers and age limits](#mov-a8-young-viewers-and-age-limits) | 5 | 4 | 9 |
| [MOV-A9 Your lists, history and following](#mov-a9-your-lists-history-and-following) | 6 | 5 | 8 |
| [MOV-A10 Publishing a film](#mov-a10-publishing-a-film) | 10 | 7 | 17 |
| [MOV-A11 Rights, territories and release windows](#mov-a11-rights-territories-and-release-windows) | 8 | 7 | 17 |
| [MOV-A12 Age ratings and content advice](#mov-a12-age-ratings-and-content-advice) | 5 | 4 | 11 |
| [MOV-A13 Captions, audio description and dubbing](#mov-a13-captions-audio-description-and-dubbing) | 7 | 5 | 15 |
| [MOV-A14 Filmmaker earnings and payouts](#mov-a14-filmmaker-earnings-and-payouts) | 7 | 5 | 15 |
| [MOV-A15 Filmmaker studio and stats](#mov-a15-filmmaker-studio-and-stats) | 6 | 3 | 10 |
| [MOV-A16 Stopping copies](#mov-a16-stopping-copies) | 5 | 3 | 13 |
| [MOV-A17 Moderation, legal removals and transparency](#mov-a17-moderation-legal-removals-and-transparency) | 9 | 9 | 18 |
| [MOV-A18 Video pipeline and delivery at cost](#mov-a18-video-pipeline-and-delivery-at-cost) | 9 | 7 | 21 |
| [MOV-A19 Apps and devices](#mov-a19-apps-and-devices) | 6 | 4 | 8 |
| [MOV-A20 Accessible apps and languages](#mov-a20-accessible-apps-and-languages) | 7 | 5 | 10 |
| [MOV-A21 Your data and privacy](#mov-a21-your-data-and-privacy) | 7 | 6 | 13 |
| [MOV-A22 Running Spare Movies: rules, costs, votes and help](#mov-a22-running-spare-movies-rules-costs-votes-and-help) | 7 | 6 | 18 |
| [MOV-A23 Headless API, plugins and AI agents](#mov-a23-headless-api-plugins-and-ai-agents) | 6 | 3 | 10 |

Kinds of part: **library** (code compiled into the app), **service** (runs on servers), **interface** (screens), **data** (records and formats), **integration** (joins a kit service or outside provider), **research** and **legal** (notes and checks before building). Sizes follow the work board: small, medium, large.

## MOV-A1 Joining, accounts and profiles

Signing up with one Spareware login, watching free films before joining, turning on the filmmaker side for a person, company or collective, household profiles, signing in on a TV, getting back in, and leaving cleanly.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A1-F1** Sign up with one Spareware login | Join in under a minute with a passkey and a display name, using the same login as every other Spareware app. | core | P1 Spareware login hook-up; P2 Sign-up flow; P3 Sign-up spam guard |
| **MOV-A1-F2** Watch free films before joining | Browse the catalogue and watch free films and trailers with no account, and join only to pay, follow or keep a list. | core | P1 Local profile without an account |
| **MOV-A1-F3** Become a filmmaker | Turn on the filmmaker side of your account, alone or as a company or collective, with a public page for your work. | core | P1 Filmmaker role; P2 Companies and collectives; P3 Filmmaker page; P4 Checks before the first payout |
| **MOV-A1-F4** Share one account at home | Give each person in a household a profile with their own list, history, section choice and age limit. | core | P1 Profiles on an account; P2 Profile picker; P3 Profile lock |
| **MOV-A1-F5** Get back into your account | Recover a lost login and sign out of phones, computers and TVs you no longer use. | core | P1 Recovery options; P2 Devices list; P3 Sign-in alerts |
| **MOV-A1-F6** Sign in on a TV | Sign in on a TV or games console by typing a short code shown on the TV into your phone. | next | P1 TV code sign-in |
| **MOV-A1-F7** Pause or leave | Pause or delete your account, knowing what happens to films you bought and, for filmmakers, to the films you published. | core | P1 Pause and delete; P2 Films stay for buyers |
| **MOV-A1-F8** Hand your films on | Pass a catalogue to a new rights holder when rights are sold, a company changes hands or a filmmaker dies. | next | P1 Rights holder change |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A1-F1-P1 | **Spareware login hook-up**: Use the kit's account service and passkey sign-in (KIT-A1-F2-P1), passwords as a fallback. | integration | small | KIT-A1-F1-P1, KIT-A1-F2-P1 |  |
| MOV-A1-F1-P2 | **Sign-up flow**: Screens for a display name, the viewer's country and agreeing to the published rules, plus a date of birth checked against that country's age rules; only the age band is kept, never the full date (KIT-A2-F8). | interface | small | MOV-A1-F1-P1, MOV-A8-F1-P1, MOV-A17-F2-P1 |  |
| MOV-A1-F1-P3 | **Sign-up spam guard**: Uses the kit's sign-up abuse protection (KIT-A1-F1-P4), with limits set by Spare Movies' rules. | integration | small | KIT-A1-F1-P4 |  |
| MOV-A1-F2-P1 | **Local profile without an account**: Uses the kit's local profile (KIT-A1-F15-P1) so a visitor can browse, watch free films and keep a list on the device, then carry it into a new account. | integration | small | KIT-A1-F15-P1, MOV-A2-F1-P2 |  |
| MOV-A1-F3-P1 | **Filmmaker role**: A filmmaker role on the account (KIT-A1-F5-P2) for a person, a production company or a collective, with the country it is based in. | data | small | kit-core, KIT-A1-F5-P2 |  |
| MOV-A1-F3-P2 | **Companies and collectives**: Production companies and collectives as organisations on the kit (KIT-A1-F7-P1), with members and roles (KIT-A1-F7-P2). | integration | small | KIT-A1-F7-P1, KIT-A1-F7-P2 |  |
| MOV-A1-F3-P3 | **Filmmaker page**: A public page with the films, a short bio, links, the team and a follow button. | interface | medium | MOV-A1-F3-P1, MOV-A3-F1-P1 |  |
| MOV-A1-F3-P4 | **Checks before the first payout**: Before any money is paid out, the person or company is checked through the kit's identity and business checks (KIT-A2-F1-P2, KIT-A2-F7-P1), including people overseas (KIT-A2-F1-P6), keeping only the result. | integration | small | KIT-A2-F1-P2, KIT-A2-F7-P1, KIT-A2-F1-P6 | [K02](../kit/STATUS.md) |
| MOV-A1-F4-P1 | **Profiles on an account**: Up to a published number of profiles per account, each with its own list, history, Human and AI choice, caption settings and age limit. | data | small | kit-core, KIT-A1-F5-P1 |  |
| MOV-A1-F4-P2 | **Profile picker**: Pick a profile when the app opens, with the age limit shown on each. | interface | small | MOV-A1-F4-P1 |  |
| MOV-A1-F4-P3 | **Profile lock**: A PIN or passkey on a profile, asked for before leaving a young person's profile and before films above a profile's age limit, using the kit's fresh sign-in for risky commands (KIT-A1-F3-P4). | service | small | MOV-A1-F4-P1, KIT-A1-F3-P4 |  |
| MOV-A1-F5-P1 | **Recovery options**: Recovery codes, a backup passkey and assisted recovery from the kit (KIT-A1-F4). | integration | small | KIT-A1-F4-P1, KIT-A1-F4-P2 |  |
| MOV-A1-F5-P2 | **Devices list**: Every signed-in phone, computer and TV, with sign-out for any or all, on the kit's sessions and devices (KIT-A1-F3-P2). | integration | small | KIT-A1-F3-P2 |  |
| MOV-A1-F5-P3 | **Sign-in alerts**: Uses the kit's new sign-in alerts (KIT-A1-F3-P3). | integration | small | KIT-A1-F3-P3 |  |
| MOV-A1-F6-P1 | **TV code sign-in**: Uses the kit's device sign-in (KIT-A1-F12-P1), the open OAuth device flow, so no password is typed with a remote. | integration | small | KIT-A1-F12-P1, MOV-A1-F1-P1 |  |
| MOV-A1-F7-P1 | **Pause and delete**: Uses the kit's account deletion and settling up before closing (KIT-A18-F2), so a filmmaker's last earnings are paid out first. | integration | small | KIT-A18-F2-P1, KIT-A18-F2-P2 |  |
| MOV-A1-F7-P2 | **Films stay for buyers**: When a filmmaker leaves, people who bought their films keep watching for the time the keep-access promise gives (A6-F2-P3), with the earnings still paid out. | legal | small | MOV-A6-F2-P3 |  |
| MOV-A1-F8-P1 | **Rights holder change**: Moves films, their rights records and future earnings to a new owner with both sides' confirmation (KIT-A1-F17-P1), or to an estate (KIT-A18-F11-P1), with past earnings kept where they were. | service | medium | MOV-A11-F1-P1, KIT-A1-F17-P1, KIT-A18-F11-P1 |  |

## MOV-A2 Browse, search and discovery

Finding something to watch: a catalogue in an order whose rules are published, filters for what matters (price, captions, audio description, age rating, language, length and section), search across titles, people and words spoken, release calendars, collections made by people, and suggestions that stay off unless you turn them on.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A2-F1** Browse the catalogue | Browse films and series by newest, genre, length, country and language, with the rule for each list shown above it. | core | P1 Catalogue index; P2 Browse screens; P3 Published order rules; P4 Only what you can watch here |
| **MOV-A2-F2** Filter by what matters to you | Narrow any list by free, pass, rent or buy, captions, audio description, age rating, spoken language, length and section. | core | P1 Filters; P2 Saved filters |
| **MOV-A2-F3** Search everything | Search titles, people, genres and the words spoken in a film, and find it even with a misspelling or a missing accent. | core | P1 Search service; P2 Search inside captions; P3 Search screen; P4 Forgiving search |
| **MOV-A2-F4** See what is new and coming | See new releases this week and a calendar of coming films, and follow one to be told when it opens in your country. | core | P1 Release calendar; P2 Coming soon pages |
| **MOV-A2-F5** Collections made by people | Browse collections put together by filmmakers, festivals, libraries, critics and viewers, and follow the ones you like. | next | P1 Collections; P2 Collection pages |
| **MOV-A2-F6** Find films from your own country | Find films made in your country or in your languages on their own shelf. | next | P1 Local shelves |
| **MOV-A2-F7** Suggestions you can switch off | Turn on suggestions worked out on your own device, each with its reason shown, and switch them off again any time. | later | P1 Suggestions on the device |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A2-F1-P1 | **Catalogue index**: Every film and series on the kit's search service (KIT-A6-F1-P2), updated as films are published, changed or taken down. | service | large | MOV-A3-F1-P1, KIT-A6-F1-P2 |  |
| MOV-A2-F1-P2 | **Browse screens**: Plain lists and grids by genre, newest, length and language, in our own design; no endless autoplaying rows. | interface | medium | MOV-A2-F1-P1 |  |
| MOV-A2-F1-P3 | **Published order rules**: Every list says how it is ordered, on the kit's published ranking rules (KIT-A6-F1-P3); no one can pay to be placed higher. | data | small | KIT-A6-F1-P3 |  |
| MOV-A2-F1-P4 | **Only what you can watch here**: Lists show only films available in the viewer's country, with that country's age rating, so nothing appears that cannot be played. | service | small | MOV-A11-F2-P1, MOV-A12-F1-P1, MOV-A2-F1-P1 |  |
| MOV-A2-F2-P1 | **Filters**: One filter panel for price type, captions, audio description, rating, language, length and Human or AI section. | interface | small | MOV-A2-F1-P1, MOV-A13-F1-P1, MOV-A13-F3-P4, MOV-A12-F1-P1, MOV-A5-F2-P1 |  |
| MOV-A2-F2-P2 | **Saved filters**: A profile can keep its filters, for example always audio description, or always free films first. | data | small | MOV-A2-F2-P1, MOV-A1-F4-P1 |  |
| MOV-A2-F3-P1 | **Search service**: Titles, people, genres, collections and filmmakers on the kit's search service (KIT-A6-F1-P2). | integration | medium | KIT-A6-F1-P1, KIT-A6-F1-P2, MOV-A2-F1-P1 | [MK05](../spare-market/STATUS.md) |
| MOV-A2-F3-P2 | **Search inside captions**: Find a film by a line someone says in it, using its caption text, with a jump to that moment. | service | medium | MOV-A13-F1-P1, MOV-A2-F3-P1 |  |
| MOV-A2-F3-P3 | **Search screen**: One search box with results grouped by films, series, people and filmmakers. | interface | small | MOV-A2-F3-P1 |  |
| MOV-A2-F3-P4 | **Forgiving search**: Typo-tolerant matching and the kit's accent and macron-aware matching (KIT-A19-F1-P4), so names in any language are found. | library | small | KIT-A19-F1-P4, MOV-A2-F3-P1 |  |
| MOV-A2-F4-P1 | **Release calendar**: Coming films by date for the viewer's country, from each film's release windows. | service | small | MOV-A11-F3-P1, MOV-A2-F1-P1 |  |
| MOV-A2-F4-P2 | **Coming soon pages**: A film page before release with the trailer, the date in your country and a button to be told. | interface | small | MOV-A3-F1-P2, MOV-A9-F3-P2 |  |
| MOV-A2-F5-P1 | **Collections**: A named list of films with a short note for each, made by anyone, with the maker shown. | data | small | MOV-A3-F1-P1 |  |
| MOV-A2-F5-P2 | **Collection pages**: A page per collection with a follow button and a share link. | interface | small | MOV-A2-F5-P1, MOV-A9-F3-P1 |  |
| MOV-A2-F6-P1 | **Local shelves**: Shelves for films made in the viewer's country and in its languages, from the country pack (KIT-A19-F6-P1). | service | small | MOV-A2-F1-P1, KIT-A19-F6-P1 |  |
| MOV-A2-F7-P1 | **Suggestions on the device**: Suggestions worked out on the viewer's device from their own history, with the reason shown, on the kit's explainable suggestions (KIT-A6-F10-P1); off unless turned on, and nothing leaves the device. | library | medium | KIT-A6-F10-P1, MOV-A9-F2-P1 |  |

## MOV-A3 Film and series pages

Everything about a film on one page: the trailer, synopsis, cast and crew, the age rating and content advice for your country, captions, audio description and languages, how AI was used, the ways to watch and what each costs, extras, and series with seasons and episodes.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A3-F1** See everything about a film | Open one page with the trailer, synopsis, length, year, genre, languages, the age rating for your country and the ways to watch. | core | P1 Film record; P2 Film page; P3 Public pages found by web search |
| **MOV-A3-F2** Watch the trailer | Play the trailer free, with captions, before deciding. | core | P1 Trailer playback; P2 Trailer rating rules |
| **MOV-A3-F3** See who made it | See the cast and crew with their roles, and open a page for each person. | core | P1 Credits record; P2 People pages; P3 Claim your credit |
| **MOV-A3-F4** Know how to watch it | See every way to watch, what each costs in your money with the fee shown, and what you already have. | core | P1 Ways to watch panel |
| **MOV-A3-F5** Series, seasons and episodes | Open a series, pick a season and an episode, and buy a whole season at once. | core | P1 Series model; P2 Series page |
| **MOV-A3-F6** Extras and other versions | Watch extras such as commentary and behind-the-scenes, and pick another cut such as the director's or a festival version. | next | P1 Extras and versions |
| **MOV-A3-F7** Reviews from people who watched | Read and write reviews, open only to people who watched the film here, and see festival selections and awards. | next | P1 Watched-only reviews; P2 Festivals and awards |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A3-F1-P1 | **Film record**: Title, synopsis, genres, length, year, country of origin, spoken languages, team, artwork, trailer and versions, stored as operations so every change has a history. | data | medium | kit-core, kit-ops |  |
| MOV-A3-F1-P2 | **Film page**: One page with the trailer at the top, then the facts, the rating and advice, the access tracks, the AI-use answer, the ways to watch and the credits. | interface | medium | MOV-A3-F1-P1, MOV-A12-F1-P2, MOV-A3-F4-P1, MOV-A5-F4-P2 |  |
| MOV-A3-F1-P3 | **Public pages found by web search**: Film pages anyone can open without an account, with share cards and web search basics from the kit (KIT-A28-F2). | integration | small | KIT-A28-F2-P1, KIT-A28-F2-P3, MOV-A3-F1-P1 |  |
| MOV-A3-F2-P1 | **Trailer playback**: Trailers play free for everyone in the main player with captions, and never start on their own with sound. | interface | small | MOV-A4-F1-P1, MOV-A13-F1-P1 |  |
| MOV-A3-F2-P2 | **Trailer rating rules**: Some countries rate trailers on their own; a trailer carries its own rating or is shown only to profiles allowed to see the film, as each country's pack says. | legal | small | MOV-A12-F1-P3 |  |
| MOV-A3-F3-P1 | **Credits record**: Cast and crew with their roles in the order the filmmaker sets, from a published list of film roles. | data | small | MOV-A3-F1-P1 |  |
| MOV-A3-F3-P2 | **People pages**: A page for each person listing the films they worked on here. | interface | small | MOV-A3-F3-P1, MOV-A1-F3-P3 |  |
| MOV-A3-F3-P3 | **Claim your credit**: A person can claim a credit and link it to their account once the filmmaker confirms; a disputed credit goes to moderators. | service | small | MOV-A3-F3-P1, MOV-A17-F1-P1 |  |
| MOV-A3-F4-P1 | **Ways to watch panel**: Free, in the pass, rent or buy, each with its price in local money, what it gives you and whether you already have it. | interface | small | MOV-A6-F1-P2, MOV-A11-F2-P1 |  |
| MOV-A3-F5-P1 | **Series model**: Series made of seasons and episodes, each episode with its own length, rating and access tracks. | data | small | MOV-A3-F1-P1 |  |
| MOV-A3-F5-P2 | **Series page**: Seasons as tabs, episodes in order with what you have watched marked. | interface | small | MOV-A3-F5-P1 |  |
| MOV-A3-F6-P1 | **Extras and versions**: Extras and alternate versions linked to the main film, each with its own rating and access tracks. | data | small | MOV-A3-F1-P1 |  |
| MOV-A3-F7-P1 | **Watched-only reviews**: Reviews on the kit's reviews (KIT-A9-F2-P1) and fake review checks (KIT-A9-F8-P1), open only to profiles with watch time on the film. | integration | small | KIT-A9-F2-P1, KIT-A9-F8-P1, MOV-A9-F2-P1 |  |
| MOV-A3-F7-P2 | **Festivals and awards**: Festival selections and awards the filmmaker lists, each with a link to the festival as proof. | data | small | MOV-A3-F1-P1 |  |

## MOV-A4 The player

Watching: a fast start that adjusts to your connection, picking up where you left off, captions and audio description your way, audio languages, next episodes you control, chapters and speed, casting to a TV, downloads for offline, picture-in-picture, watching together, and reporting a playback problem.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A4-F1** Press play and it starts fast | Press play and the film starts quickly and keeps playing as your connection changes. | core | P1 Player engine; P2 Player controls; P3 Start-time budget |
| **MOV-A4-F2** Pick up where you left off | Stop on one device and carry on from the same moment on another. | core | P1 Resume points |
| **MOV-A4-F3** Captions and audio description your way | Turn on captions or audio description in one tap, choose the language and caption style, and have the choice remembered. | core | P1 Caption display; P2 Caption style; P3 Audio description switch; P4 Remembered choices |
| **MOV-A4-F4** Audio languages | Switch the spoken language when the filmmaker has added other language tracks. | core | P1 Audio track switch |
| **MOV-A4-F5** Quality that fits your connection | Let quality follow your connection, or cap it to save data, and see how much data an hour uses. | core | P1 Quality choice; P2 Data saver |
| **MOV-A4-F6** Next episode you control | Have the next episode start only if you turned that on, and never on a young person's profile. | core | P1 Next episode |
| **MOV-A4-F7** Chapters, speed and skipping | Jump between chapters, skip opening titles, and change the playback speed. | next | P1 Chapters and markers; P2 Speed and skip controls |
| **MOV-A4-F8** Watch on your TV | Send a film from your phone to a TV. | next | P1 Casting research; P2 Cast to a TV |
| **MOV-A4-F9** Download to watch offline | Download a film to watch on a plane or with no signal, within limits the filmmaker sets. | next | P1 Offline downloads; P2 Download rules |
| **MOV-A4-F10** Picture-in-picture | Keep a film playing in a small window while you use another app. | later | P1 Picture-in-picture |
| **MOV-A4-F11** Watch together | Watch in step with friends in other homes, each with their own access to the film. | later | P1 Watch party |
| **MOV-A4-F12** Report a playback problem | Tell us when a film stalls, captions are wrong or the sound is out of step, with the details filled in for you. | core | P1 Playback problem report |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A4-F1-P1 | **Player engine**: Adaptive playback built on an openly licensed player core (A18-F3-P2), with our own controls on top. | library | large | MOV-A18-F3-P2, engine-media |  |
| MOV-A4-F1-P2 | **Player controls**: Play, pause, seek with a preview, volume, full screen, captions, audio, quality and help, all in our own design and all reachable by keyboard and remote. | interface | medium | MOV-A4-F1-P1 |  |
| MOV-A4-F1-P3 | **Start-time budget**: Targets for time to first frame and stalls per hour, measured on real connections in each launch country. | research | small |  |  |
| MOV-A4-F2-P1 | **Resume points**: The last position per profile and film, synced across devices and cleared with the history. | service | small | MOV-A9-F2-P1, kit-sync |  |
| MOV-A4-F3-P1 | **Caption display**: Shows the chosen caption track in time with the film, including captions for Deaf and hard of hearing viewers. | library | small | MOV-A13-F1-P1, MOV-A4-F1-P1 |  |
| MOV-A4-F3-P2 | **Caption style**: Size, colour, background and position, from the kit's display settings (KIT-A19-F3-P1). | interface | small | KIT-A19-F3-P1, MOV-A4-F3-P1 |  |
| MOV-A4-F3-P3 | **Audio description switch**: Switches to the audio description track, or reads text descriptions aloud (A13-F3-P2), without restarting the film. | library | small | MOV-A13-F3-P1, MOV-A4-F1-P1 |  |
| MOV-A4-F3-P4 | **Remembered choices**: Each profile's caption, audio description and language choices follow it to every device (KIT-A20-F5-P2). | data | small | KIT-A20-F5-P2, MOV-A1-F4-P1 |  |
| MOV-A4-F4-P1 | **Audio track switch**: Lists and switches audio tracks, marking dubbed tracks and AI-made voices (A13-F4-P2). | library | small | MOV-A4-F1-P1, MOV-A13-F4-P1 |  |
| MOV-A4-F5-P1 | **Quality choice**: Auto, or a fixed size, with the data used per hour shown for each. | interface | small | MOV-A4-F1-P1, MOV-A18-F2-P1 |  |
| MOV-A4-F5-P2 | **Data saver**: A data cap per profile and the kit's light mode for slow connections (KIT-A19-F5-P1). | service | small | MOV-A4-F5-P1, KIT-A19-F5-P1 |  |
| MOV-A4-F6-P1 | **Next episode**: Offers the next episode at the end; starts it on its own only if the profile turned autoplay on; off for young viewers. | service | small | MOV-A3-F5-P1, MOV-A4-F2-P1, MOV-A8-F2-P1 |  |
| MOV-A4-F7-P1 | **Chapters and markers**: Chapters and markers for opening titles and end credits, set by the filmmaker. | data | small | MOV-A10-F1-P1 |  |
| MOV-A4-F7-P2 | **Speed and skip controls**: Chapter list, skip buttons and speeds from half to double. | interface | small | MOV-A4-F7-P1, MOV-A4-F1-P2 |  |
| MOV-A4-F8-P1 | **Casting research**: Compare ways to send video to a TV on licences, terms and reach: the casting kits phone makers offer, the open DLNA and UPnP standards, and our own TV apps (A19-F5). | research | small |  |  |
| MOV-A4-F8-P2 | **Cast to a TV**: Casting by the method the research picks, carrying captions and audio description to the TV. | integration | medium | MOV-A4-F8-P1, MOV-A4-F1-P1 |  |
| MOV-A4-F9-P1 | **Offline downloads**: Downloads kept protected on the device (KIT-A15-F4-P4), with a rental's clock still running and the access checked again when back online. | library | large | MOV-A4-F1-P1, MOV-A16-F1-P3, KIT-A15-F4-P4 |  |
| MOV-A4-F9-P2 | **Download rules**: Which films may be downloaded, in which countries and for how long, set by the filmmaker within the rights they hold. | legal | small | MOV-A11-F1-P1 |  |
| MOV-A4-F10-P1 | **Picture-in-picture**: The player's picture-in-picture mode on phones and computers that offer it. | interface | small | MOV-A4-F1-P1 |  |
| MOV-A4-F11-P1 | **Watch party**: Keeps players in step with the kit's live updates (KIT-A15-F5-P1) and a chat beside the film (KIT-A8-F1-P2); each person needs their own access. | service | medium | MOV-A4-F1-P1, KIT-A15-F5-P1, KIT-A8-F1-P2 |  |
| MOV-A4-F12-P1 | **Playback problem report**: A report that attaches the film, the moment, the device and the stream details, on the kit's support cases (KIT-A21-F2-P1), and refunds a rental that could not play (A6-F7-P1). | integration | small | KIT-A21-F2-P1, MOV-A4-F1-P1 |  |

## MOV-A5 Human and AI sections

The catalogue has a Human section and an AI section. Filmmakers declare AI use for every film, episode, trailer and poster, the section follows that answer whatever tool published it, AI agent accounts are labelled and publish only in the AI section, uploads are checked for content credentials and detection signals as hints only, each viewer chooses Human, AI or both, and wrong placements are fixed in the open.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A5-F1** Declare AI use on every film | Say for every film, episode, trailer and poster whether AI was used and how, in plain categories. | core | P1 AI-use answer; P2 Declaration step; P3 Where the line falls |
| **MOV-A5-F2** Choose Human, AI or both | Switch the catalogue, search and lists to the Human section, the AI section or both, and change it any time. | core | P1 Section filter; P2 Section switch; P3 Choice per profile |
| **MOV-A5-F3** Labelled AI agents | Know at a glance when an account is an AI agent and who is responsible for it. | core | P1 Agent account type; P2 Section follows the answer, not the route |
| **MOV-A5-F4** Checked, not assumed | See content credentials when a film or trailer carries them, while moderators get detection signals as hints, never verdicts. | core | P1 Content credentials reader; P2 Credentials on the film page; P3 Detection signals research; P4 Hints to the review queue |
| **MOV-A5-F5** Fix wrong sections in the open | Flag a film in the wrong section; a moderator moves it with the reason logged in public, and the filmmaker can appeal. | core | P1 Wrong-section report; P2 Section move and appeal |
| **MOV-A5-F6** Fair to both sections | Know that neither section is buried, and that films are paid the same way whichever section they are in. | core | P1 Same rules in both sections |
| **MOV-A5-F7** Credentials from our own tools | Keep the content credentials Spare Studio adds when a film is exported, so the record of how it was made travels with it. | later | P1 Keep Studio credentials |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A5-F1-P1 | **AI-use answer**: None, AI-assisted (with the tasks named, such as clean-up, upscaling, dubbing or visual effects) or AI-made, with the tools named, for the film and separately for its trailer and poster. | data | small | MOV-A3-F1-P1 |  |
| MOV-A5-F1-P2 | **Declaration step**: The publishing steps ask every time, with plain examples of each answer; it cannot be skipped. | interface | small | MOV-A5-F1-P1, MOV-A10-F1-P2 |  |
| MOV-A5-F1-P3 | **Where the line falls**: Published rules for what counts as AI-assisted or AI-made in film, for example AI voices, AI dubbing, de-ageing an actor or generated shots, set and changed by the filmmakers' vote. | legal | small | MOV-A17-F2-P1 |  |
| MOV-A5-F2-P1 | **Section filter**: Every list, search and filmmaker page filters by section on the server. | service | small | MOV-A5-F1-P1, MOV-A2-F1-P1 |  |
| MOV-A5-F2-P2 | **Section switch**: A clear Human, AI, Both switch at the top of browse and search. | interface | small | MOV-A5-F2-P1 |  |
| MOV-A5-F2-P3 | **Choice per profile**: Each household profile keeps its own choice. | data | small | MOV-A1-F4-P1 |  |
| MOV-A5-F3-P1 | **Agent account type**: AI agent accounts carry the kit's agent label (KIT-A17-F9-P1) and the person or organisation responsible, and publish only in the AI section. | data | small | svc-accounts, KIT-A17-F9-P1 |  |
| MOV-A5-F3-P2 | **Section follows the answer, not the route**: A film goes to the section its AI-use answer gives, whether it came from the app, the command API, MCP, a bulk import or Spare Studio; an agent acting for a filmmaker carries a label without changing the film's section. | service | small | MOV-A5-F1-P1, MOV-A5-F3-P1, MOV-A23-F1-P1, kit-mcp |  |
| MOV-A5-F4-P1 | **Content credentials reader**: Reads C2PA content credentials on upload with the kit's openly licensed library (KIT-A27-F8-P6). | integration | small | KIT-A27-F8-P6, MOV-A18-F1-P1 |  |
| MOV-A5-F4-P2 | **Credentials on the film page**: A plain summary of how the film says it was made, from its credentials, beside the AI-use answer. | interface | small | MOV-A5-F4-P1 |  |
| MOV-A5-F4-P3 | **Detection signals research**: Compare open AI-detection methods for long video on accuracy, bias and licence, for use as moderator hints only. | research | medium |  |  |
| MOV-A5-F4-P4 | **Hints to the review queue**: A strong signal against a Human answer puts the film in the review queue for a person; nothing is moved automatically. | service | small | MOV-A5-F4-P3, MOV-A17-F7-P1 |  |
| MOV-A5-F5-P1 | **Wrong-section report**: A report type for films in the wrong section. | data | small | MOV-A17-F1-P1 |  |
| MOV-A5-F5-P2 | **Section move and appeal**: Moderators move a film with a logged reason; the filmmaker can appeal. | service | small | MOV-A17-F5-P1, MOV-A17-F6-P1 |  |
| MOV-A5-F6-P1 | **Same rules in both sections**: Both sections use the same published order rules and the same at-cost fee; using AI is not a penalty. | data | small | MOV-A2-F1-P3, MOV-A6-F1-P3 |  |
| MOV-A5-F7-P1 | **Keep Studio credentials**: Films exported from Spare Studio keep their C2PA credentials, signed with the co-op's certificate (KIT-A27-F8-P7), through encoding. | integration | medium | KIT-A27-F8-P6, KIT-A27-F8-P7, MOV-A5-F4-P1 |  |

## MOV-A6 Paying for films: rent, buy, support or free

Each filmmaker chooses how each film is sold: rent, buy to keep, pay what you like, support, free, or in the viewer pass. Viewers see the price in their own money with tax and the at-cost fee shown, pay through a regulated payment provider, and get receipts, refunds when a film will not play, and limits on young profiles.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A6-F1** See the price and the fee before you pay | See what you pay, the tax in it, the at-cost fee and what the filmmaker gets, before you pay. | core | P1 Price rules per film; P2 Price breakdown; P3 At-cost fee for films |
| **MOV-A6-F2** Buy to keep | Buy a film or a season and watch it any time, on any of your devices, for as long as the promise on the page says. | core | P1 Purchases and access; P2 Buy flow; P3 Keep-access promise |
| **MOV-A6-F3** Rent | Rent a film, start it within a set time, and finish it within a set time after starting. | core | P1 Rental clocks; P2 Rent flow |
| **MOV-A6-F4** Support a filmmaker | Pay what you like for a film, add support on top of a price, or support a filmmaker after a free film. | core | P1 Support payments; P2 Pay what you like |
| **MOV-A6-F5** Watch free films | Watch films the filmmaker made free, with no adverts. | core | P1 Free films |
| **MOV-A6-F6** Pay once, safely | Pay by card or bank through a regulated payment provider, save a card for next time, and get a receipt. | core | P1 Checkout link; P2 Saved payment methods; P3 Receipts; P4 Fraud screening |
| **MOV-A6-F7** Refunds when it will not play | Get your money back automatically when a rental fails to play, and know your rights for everything else. | core | P1 Refund rules; P2 Refunds and card disputes |
| **MOV-A6-F8** Tax done right | Know the right tax is in the price and goes to the right tax office, whoever is the seller. | core | P1 Seller of record for films; P2 Tax on digital sales; P3 Tax in the price |
| **MOV-A6-F9** Sell to viewers in other countries | Sell a film to viewers anywhere the filmmaker holds the rights, in each viewer's money, with that country's tax. | next | P1 Tax across borders; P2 Prices in each currency |
| **MOV-A6-F10** Pay in phone and TV apps | Buy or rent inside phone and TV apps where store rules allow it, at the same price. | next | P1 App store payment rules; P2 Store billing where required |
| **MOV-A6-F11** Limits on young profiles | Know that nothing is bought, rented or paid from a young person's profile without the account holder. | core | P1 Purchase lock on profiles |
| **MOV-A6-F12** Give a film as a gift | Send a film to someone as a gift, or give a gift code. | later | P1 Gift a film |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A6-F1-P1 | **Price rules per film**: The filmmaker's chosen ways and prices for each film and each country, in exact money amounts (KIT-A3-F1-P4). | data | small | MOV-A3-F1-P1, KIT-A3-F1-P4 |  |
| MOV-A6-F1-P2 | **Price breakdown**: Price, tax, the payment provider's charge, Spare Movies' running-cost share and what the filmmaker gets, on the kit's fee breakdown (KIT-A3-F4-P2). | interface | small | MOV-A6-F1-P1, MOV-A6-F1-P3, KIT-A3-F4-P2 |  |
| MOV-A6-F1-P3 | **At-cost fee for films**: The fee on each sale: the payment provider's charge passed through, plus the last period's running cost per sale and per hour delivered (A18-F5-P1), published on the ledger; no margin. | service | medium | KIT-A3-F4-P1, KIT-A13-F3-P6, MOV-A18-F5-P1 | [K06](../kit/STATUS.md) |
| MOV-A6-F2-P1 | **Purchases and access**: Purchases recorded as orders on the kit's marketplace engine (KIT-A5-F4-P1), giving the account the right to watch. | service | medium | kit-market, KIT-A5-F4-P1, MOV-A6-F1-P1 |  |
| MOV-A6-F2-P2 | **Buy flow**: Buy in two steps from the film page, with the breakdown and the keep-access promise shown. | interface | small | MOV-A6-F2-P1, MOV-A6-F6-P1 |  |
| MOV-A6-F2-P3 | **Keep-access promise**: What 'buy' means in plain words: how long access lasts, what happens if the filmmaker withdraws the film, if rights end in your country or if Spare Movies closes, checked against consumer law in each country (KIT-A18-F8-P1). | legal | small | KIT-A18-F8-P1 |  |
| MOV-A6-F3-P1 | **Rental clocks**: Start and finish windows set by the filmmaker within published limits, run on the kit's durable timers (KIT-A15-F14-P1). | service | small | MOV-A6-F2-P1, KIT-A15-F14-P1 |  |
| MOV-A6-F3-P2 | **Rent flow**: Rent in two steps, with the clocks stated plainly before paying and shown on the film afterwards. | interface | small | MOV-A6-F3-P1, MOV-A6-F6-P1 |  |
| MOV-A6-F4-P1 | **Support payments**: Support on the kit's tips (KIT-A3-F6-P1); the filmmaker gets it all except the provider's charge, which the viewer can choose to cover. | integration | small | KIT-A3-F6-P1, MOV-A6-F1-P1 |  |
| MOV-A6-F4-P2 | **Pay what you like**: A film can have a minimum price chosen by the filmmaker, which can be zero, and a suggested amount. | data | small | MOV-A6-F1-P1 |  |
| MOV-A6-F5-P1 | **Free films**: A free way to watch, for everyone or for one country, with an optional support button. | data | small | MOV-A6-F1-P1 |  |
| MOV-A6-F6-P1 | **Checkout link**: The kit's checkout (KIT-A3-F1-P3) through the chosen payment provider (KIT-A3-F1-P2); Spare Movies never handles card numbers. | integration | small | KIT-A3-F1-P2, KIT-A3-F1-P3 | [K02](../kit/STATUS.md) |
| MOV-A6-F6-P2 | **Saved payment methods**: Uses the kit's saved payment methods (KIT-A3-F8-P1). | integration | small | KIT-A3-F8-P1 |  |
| MOV-A6-F6-P3 | **Receipts**: Receipts and tax invoices from the kit (KIT-A3-F7-P1, KIT-A4-F2-P1), naming the seller of record (A6-F8-P1). | integration | small | KIT-A3-F7-P1, KIT-A4-F2-P1 |  |
| MOV-A6-F6-P4 | **Fraud screening**: The provider's fraud screening with the kit's published risk rules (KIT-A3-F13). | integration | small | KIT-A3-F13-P1, KIT-A3-F13-P2 |  |
| MOV-A6-F7-P1 | **Refund rules**: Published refund rules: an automatic refund when a film could not play, and what consumer law gives in each country (KIT-A18-F8-P1). | legal | small | KIT-A18-F8-P1 |  |
| MOV-A6-F7-P2 | **Refunds and card disputes**: Uses the kit's refunds and card disputes (KIT-A3-F5), taking a refunded amount back from the filmmaker's next payout. | integration | small | KIT-A3-F5-P1, KIT-A3-F5-P2, MOV-A6-F7-P1 |  |
| MOV-A6-F8-P1 | **Seller of record for films**: Decide for each kind of sale whether the filmmaker or the co-op is the seller (KIT-A3-F1-P5), which sets who charges tax, whose name is on the receipt and who answers card disputes. | legal | medium | KIT-A3-F1-P5 | [K01](../kit/STATUS.md) |
| MOV-A6-F8-P2 | **Tax on digital sales**: Each launch country's rules for tax on films sold online, including the rules that make a platform collect tax on digital services sold through it, kept in its country pack (KIT-A19-F6-P1). | legal | medium | KIT-A4-F1-P1, KIT-A19-F6-P1 |  |
| MOV-A6-F8-P3 | **Tax in the price**: The kit's tax engine (KIT-A4-F1-P2) adds or includes tax as each country shows prices. | integration | small | KIT-A4-F1-P2, MOV-A6-F8-P2 |  |
| MOV-A6-F9-P1 | **Tax across borders**: Tax on films sold to viewers in other countries, on the kit's tax on services sold across borders (KIT-A4-F7-P2). | integration | medium | KIT-A4-F7-P2, MOV-A6-F8-P2 |  |
| MOV-A6-F9-P2 | **Prices in each currency**: Prices set or converted per country on the kit's currencies (KIT-A3-F14-P1), with the filmmaker paid in their own money. | integration | small | KIT-A3-F14-P1, MOV-A6-F1-P1 |  |
| MOV-A6-F10-P1 | **App store payment rules**: Each app store's rules for selling films inside an app, and where buying on the web and watching in the app is allowed, country by country (KIT-A20-F8-P1). | legal | medium | KIT-A20-F8-P1 | [K04](../kit/STATUS.md) |
| MOV-A6-F10-P2 | **Store billing where required**: Store billing only where a store requires it, with the store's charge shown as a running cost and never hidden. | integration | medium | MOV-A6-F10-P1, MOV-A19-F2-P2 | [K04](../kit/STATUS.md) |
| MOV-A6-F11-P1 | **Purchase lock on profiles**: Buying, renting or support from a profile with an age limit needs the account holder's PIN or passkey (A1-F4-P3). | service | small | MOV-A1-F4-P3, MOV-A8-F2-P1 |  |
| MOV-A6-F12-P1 | **Gift a film**: Gift codes on the kit's gift cards (KIT-A3-F15-P2), turned into a purchase when used. | integration | small | KIT-A3-F15-P2, MOV-A6-F2-P1 |  |

## MOV-A7 The viewer pass

One monthly pass for every film whose filmmaker puts it in: priced at running cost plus what is shared out, each viewer's money shared out by watch time in the open, guarded against fake viewing, with a statement for every viewer and filmmaker, and passes for people who cannot pay and for libraries and schools.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A7-F1** Watch everything in the pass | Pay one monthly price and watch every film its filmmaker put in the pass, and cancel in one step. | next | P1 Pass plans; P2 Join, pause and cancel; P3 Films opt in |
| **MOV-A7-F2** Shared out by watch time | Know that pass money goes to filmmakers by how long people watched their films, worked out in the open each month. | next | P1 Share-out method decision; P2 Watch-time counter; P3 Share-out engine; P4 Share-out statements |
| **MOV-A7-F3** Guarded against fake viewing | Know that bots and played-on-repeat films cannot take pass money from honest filmmakers. | next | P1 Fake viewing checks; P2 Holds and claw-backs |
| **MOV-A7-F4** Passes for people who cannot pay | Get a free pass under a published hardship rule, with filmmakers still paid. | later | P1 Hardship passes |
| **MOV-A7-F5** Passes for libraries and schools | Let a library or school give its members a pass, with filmmakers paid by watch time. | later | P1 Organisation passes |
| **MOV-A7-F6** Give a pass | Give someone a month or a year of the pass. | later | P1 Gift passes |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A7-F1-P1 | **Pass plans**: A monthly price set by the filmmakers' vote: the running cost per viewer plus the amount shared out, charged through the kit's regular payments (KIT-A3-F12-P1). | data | small | KIT-A3-F12-P1, MOV-A6-F1-P1 | [K02](../kit/STATUS.md) |
| MOV-A7-F1-P2 | **Join, pause and cancel**: Join in two steps, pause, and cancel in one step with no questions. | interface | small | MOV-A7-F1-P1, KIT-A3-F12-P1 |  |
| MOV-A7-F1-P3 | **Films opt in**: Each filmmaker chooses which films are in the pass, in which countries and from when, and can keep rent and buy open beside it. | data | small | MOV-A6-F1-P1, MOV-A11-F1-P1 |  |
| MOV-A7-F2-P1 | **Share-out method decision**: Compare a share per viewer (each viewer's fee goes only to what that viewer watched) with one pool shared by total watch time, on fairness to small films, fake viewing and how easy each is to explain, with worked examples, for an owner and a vote to decide. | research | medium |  |  |
| MOV-A7-F2-P2 | **Watch-time counter**: Counts minutes actually watched per film and profile on the server, capped per day, keeping only what the share-out needs and only until it is paid. | service | medium | MOV-A4-F1-P1, kit-ops |  |
| MOV-A7-F2-P3 | **Share-out engine**: The monthly calculation, built so either method can run, with the result paid through the payout runs (A14-F2-P2). | service | medium | MOV-A7-F2-P1, MOV-A7-F2-P2, svc-payments |  |
| MOV-A7-F2-P4 | **Share-out statements**: Each viewer sees where their pass money went this month; each filmmaker sees how their share was worked out. | interface | small | MOV-A7-F2-P3 |  |
| MOV-A7-F3-P1 | **Fake viewing checks**: Published rules: minimum watch time, daily caps per profile, device and pattern checks on the kit's fraud signals (KIT-A10-F12-P2). | service | medium | MOV-A7-F2-P2, KIT-A10-F12-P2 |  |
| MOV-A7-F3-P2 | **Holds and claw-backs**: Rules for holding a share under review and taking back money proven to come from fake viewing, under the kit's loss rules (KIT-A3-F16-P1). | legal | small | MOV-A7-F3-P1, KIT-A3-F16-P1 |  |
| MOV-A7-F4-P1 | **Hardship passes**: Free passes under the kit's hardship rule (KIT-A13-F3-P7), paid for from voluntary support set aside for it and shown on the ledger. | integration | small | KIT-A13-F3-P7, MOV-A7-F1-P1 |  |
| MOV-A7-F5-P1 | **Organisation passes**: An organisation buys passes for its members on the kit organisation model (KIT-A1-F7-P1), with watch time shared out the same way. | integration | medium | KIT-A1-F7-P1, MOV-A7-F1-P1 |  |
| MOV-A7-F6-P1 | **Gift passes**: Pass gift codes on the kit's gift cards (KIT-A3-F15-P2). | integration | small | KIT-A3-F15-P2, MOV-A7-F1-P1 |  |

## MOV-A8 Young viewers and age limits

Age limits that follow each country's ratings: age rules per country, profiles with an age limit, age checks for restricted films where the law asks, a decision on adult films, safe defaults for children, and family links.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A8-F1** Age rules for each country | Know the minimum age to hold an account and what each rating means in your country. | core | P1 Age rules pack; P2 Age band on the account |
| **MOV-A8-F2** Profiles with an age limit | Set an age limit on a profile so films above it are hidden or locked, with simpler screens for children. | core | P1 Profile age limit; P2 Children's profile |
| **MOV-A8-F3** Age checks for restricted films | Know that films the law restricts to adults ask for an age check that proves only that you are old enough. | core | P1 Restricted film gate; P2 Adult films decision |
| **MOV-A8-F4** Children's safety checked by design | Trust that children's privacy and safety were assessed before launch. | core | P1 Child safety review |
| **MOV-A8-F5** Parents and carers | Link family accounts so a parent can set limits and approve purchases for a child with their own account. | next | P1 Family link; P2 Purchase approvals |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A8-F1-P1 | **Age rules pack**: For each country: the minimum age for an account (KIT-A1-F1-P5), the age for each rating, and when an age check is required, kept in the country pack. | legal | medium | KIT-A19-F6-P1, KIT-A1-F1-P5 |  |
| MOV-A8-F1-P2 | **Age band on the account**: Only an age band is kept, never the birth date, matching the kit's age checks (KIT-A2-F8-P1). | integration | small | KIT-A2-F8-P1, MOV-A8-F1-P1 |  |
| MOV-A8-F2-P1 | **Profile age limit**: Films rated above the profile's limit in the viewer's country are hidden, or locked behind the account holder's PIN. | service | small | MOV-A1-F4-P1, MOV-A12-F1-P1 |  |
| MOV-A8-F2-P2 | **Children's profile**: Simpler screens with no paying, no reviews, no autoplay and no outside links. | interface | small | MOV-A8-F2-P1 |  |
| MOV-A8-F3-P1 | **Restricted film gate**: Films restricted by law to adults need an age check through an accredited provider (KIT-A2-F8-P1) that returns only over or under. | service | small | KIT-A2-F8-P1, MOV-A12-F1-P1 |  |
| MOV-A8-F3-P2 | **Adult films decision**: Whether sexually explicit films are allowed at all, and if so with which checks and kept out of store apps, for a vote with legal advice. | legal | small | MOV-A17-F2-P1 |  |
| MOV-A8-F4-P1 | **Child safety review**: A privacy and safety assessment for children (KIT-A18-F10-P1) against codes such as the UK's age-appropriate design code, before launch and after big changes. | legal | medium | KIT-A18-F10-P1 |  |
| MOV-A8-F5-P1 | **Family link**: Uses the kit's family accounts (KIT-A1-F16-P1) and rules for parents and carers (KIT-A1-F8-P2). | integration | small | KIT-A1-F16-P1, KIT-A1-F8-P2 |  |
| MOV-A8-F5-P2 | **Purchase approvals**: Uses the kit's purchase approvals for children (KIT-A1-F16-P2). | integration | small | KIT-A1-F16-P2 |  |

## MOV-A9 Your lists, history and following

Keeping track: a watchlist, continue watching, a history you can pause and clear, following filmmakers and being told when they release, notifications you choose, sharing a film, and lists you make public.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A9-F1** Keep a watchlist | Save films and series to watch later, per profile. | core | P1 Watchlist |
| **MOV-A9-F2** Continue watching and history | See what you are part way through, and pause, clear or remove items from your history. | core | P1 Watch history; P2 History controls |
| **MOV-A9-F3** Follow filmmakers | Follow a filmmaker and be told when they release a film in your country. | core | P1 Follows; P2 Release alerts |
| **MOV-A9-F4** Notifications you choose | Choose which notices you get and how: release alerts, rental reminders, receipts. | core | P1 Notification settings |
| **MOV-A9-F5** Share a film | Send a link to a film that opens in the app or the browser, with nothing tracking who clicked. | core | P1 Share links |
| **MOV-A9-F6** Public lists | Make a list public so others can see and follow it. | later | P1 Public lists |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A9-F1-P1 | **Watchlist**: A watchlist per profile on the kit's favourites (KIT-A5-F12-P1). | integration | small | KIT-A5-F12-P1, MOV-A1-F4-P1 |  |
| MOV-A9-F2-P1 | **Watch history**: Per profile, kept only for resume points, your own lists and the pass share-out, for the published time. | data | small | kit-ops, MOV-A1-F4-P1 |  |
| MOV-A9-F2-P2 | **History controls**: Pause history, clear it all, or remove one film. | interface | small | MOV-A9-F2-P1 |  |
| MOV-A9-F3-P1 | **Follows**: Follows of filmmakers, people and collections. | data | small | MOV-A1-F3-P1 |  |
| MOV-A9-F3-P2 | **Release alerts**: A notice when a followed filmmaker's film opens in your country, through the kit's notification router (KIT-A8-F3-P2). | integration | small | KIT-A8-F3-P2, MOV-A9-F3-P1, MOV-A11-F3-P1 |  |
| MOV-A9-F4-P1 | **Notification settings**: Uses the kit's notification settings (KIT-A8-F5-P1); nothing is sent to sell you more. | integration | small | KIT-A8-F5-P1 |  |
| MOV-A9-F5-P1 | **Share links**: Short links and QR codes (KIT-A20-F9-P1) that open in the right place (KIT-A20-F4-P1). | integration | small | KIT-A20-F9-P1, KIT-A20-F4-P1 |  |
| MOV-A9-F6-P1 | **Public lists**: A watchlist made public becomes a collection (A2-F5-P1). | data | small | MOV-A9-F1-P1, MOV-A2-F5-P1 |  |

## MOV-A10 Publishing a film

Getting a film up: one checklist from master file to release, uploads that resume on bad connections, quality checks in plain words, artwork and trailer, captions and audio description, release dates, series and episodes, fixing a published film, private screeners, bulk delivery from distributors, and publishing from Spare Studio.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A10-F1** Upload a film | Upload the master file once, from any connection, and follow one checklist to publish. | core | P1 Film draft; P2 Publishing checklist; P3 Resumable master upload; P4 Upload from a link |
| **MOV-A10-F2** Quality checks before publishing | Get a plain report on the picture and sound before publishing, with how to fix each problem. | core | P1 Technical checks; P2 Fix-it guide |
| **MOV-A10-F3** Artwork and trailer | Add a poster, stills and a trailer, with descriptions for people who cannot see them. | core | P1 Poster and stills; P2 Trailer upload |
| **MOV-A10-F4** Captions and audio description before publishing | Add captions in the film's main language before publishing, and audio description if you have it. | core | P1 Access tracks step |
| **MOV-A10-F5** Set the release | Pick a release date and time for each country and see it go live on time. | core | P1 Release scheduling |
| **MOV-A10-F6** Series and episodes | Publish a series season by season, releasing episodes all at once or week by week. | core | P1 Episode publishing |
| **MOV-A10-F7** Fix a published film | Replace the file, captions or artwork of a published film without viewers losing access, with every change logged. | core | P1 Replace and version; P2 Change log |
| **MOV-A10-F8** Private screeners | Send festivals, critics or distributors a private link that expires, carrying a hidden mark if you choose. | next | P1 Screener links |
| **MOV-A10-F9** Bulk delivery from distributors | Let a distributor deliver a whole catalogue at once from files and a spreadsheet. | next | P1 Delivery spec; P2 Catalogue import |
| **MOV-A10-F10** Publish from Spare Studio | Send a finished film from the Spare Studio editor straight to a draft here. | later | P1 Studio hand-off |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A10-F1-P1 | **Film draft**: A private draft that holds the file, the details and every answer until it is published. | data | small | MOV-A3-F1-P1, kit-ops |  |
| MOV-A10-F1-P2 | **Publishing checklist**: One checklist: file, details, AI-use answer, captions and audio description, ratings, rights, prices and release, with what is missing shown plainly. | interface | medium | MOV-A10-F1-P1 |  |
| MOV-A10-F1-P3 | **Resumable master upload**: Uploads of large master files that resume after a drop, on the kit's file service (KIT-A15-F7-P1) using the open tus protocol. | integration | medium | KIT-A15-F7-P1, MOV-A18-F1-P1 |  |
| MOV-A10-F1-P4 | **Upload from a link**: Fetch the master from a cloud storage link, through the safe outside fetches (A18-F6-P2). | integration | small | MOV-A10-F1-P3, MOV-A18-F6-P2 |  |
| MOV-A10-F2-P1 | **Technical checks**: Resolution, frame rate, loudness, sound in step with picture, silent or black stretches and damaged frames. | service | medium | MOV-A18-F1-P2 |  |
| MOV-A10-F2-P2 | **Fix-it guide**: Plain advice for each problem found, with links to free tools such as Spare Studio. | data | small | MOV-A10-F2-P1 |  |
| MOV-A10-F3-P1 | **Poster and stills**: Images on the kit's file service, with an image description asked for each (KIT-A19-F2-P3). | integration | small | KIT-A15-F7-P1, KIT-A19-F2-P3 |  |
| MOV-A10-F3-P2 | **Trailer upload**: A trailer with its own captions, AI-use answer and, where needed, its own rating. | service | small | MOV-A10-F1-P3, MOV-A12-F1-P1 |  |
| MOV-A10-F4-P1 | **Access tracks step**: The checklist asks for captions in the film's main spoken language, offers caption drafts (A13-F2) and audio description, and shows viewers what is missing. | interface | small | MOV-A13-F1-P1, MOV-A13-F3-P1, MOV-A10-F1-P2 |  |
| MOV-A10-F5-P1 | **Release scheduling**: Release dates per country and per way to watch, run on the kit's durable timers (KIT-A15-F14-P1). | service | small | MOV-A11-F3-P1, KIT-A15-F14-P1 |  |
| MOV-A10-F6-P1 | **Episode publishing**: Each episode has its own draft, checks and release date inside its season. | service | small | MOV-A3-F5-P1, MOV-A10-F1-P1 |  |
| MOV-A10-F7-P1 | **Replace and version**: A new file is encoded beside the old one and swapped in when ready, keeping resume points close to where they were. | service | medium | MOV-A10-F1-P1, MOV-A18-F2-P1 |  |
| MOV-A10-F7-P2 | **Change log**: Changes to price, rights, rating, section and file on the kit's public listing history (KIT-A5-F28-P1). | data | small | MOV-A10-F7-P1, KIT-A5-F28-P1 |  |
| MOV-A10-F8-P1 | **Screener links**: Private links on the kit's guest access (KIT-A15-F8-P2), with an end date, a view limit and, if chosen, a per-viewer watermark (A16-F4). | service | small | KIT-A15-F8-P2, MOV-A16-F1-P4 |  |
| MOV-A10-F9-P1 | **Delivery spec**: A published delivery spec for masters, captions, artwork and details in open formats. | data | small | MOV-A10-F1-P1 |  |
| MOV-A10-F9-P2 | **Catalogue import**: Bulk import on the kit's listing import (KIT-A5-F17-P2), each film still needing its AI-use answer, rights and rating. | integration | medium | KIT-A5-F17-P2, MOV-A10-F9-P1 |  |
| MOV-A10-F10-P1 | **Studio hand-off**: Spare Studio exports to a Spare Movies draft through the public API (A23-F5-P1), keeping content credentials (A5-F7-P1). | integration | small | MOV-A23-F5-P1, MOV-A10-F1-P1 |  |

## MOV-A11 Rights, territories and release windows

Showing a film only where the filmmaker holds the rights: rights by country and by way of watching, proof kept privately, release dates and windows, the country check, travellers keeping paid access where the law asks, what happens when rights end, disputes over who owns a film, music clearances, and screening licences.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A11-F1** Declare your rights | Say which countries you hold the rights for, for which ways of watching and until when, with proof kept privately. | core | P1 Rights record; P2 Rights editor; P3 Proof of rights; P4 Rights declaration |
| **MOV-A11-F2** Films only where rights allow | See only films you can watch where you are, and a plain reason when one is not available here. | core | P1 Country check; P2 Where you are, kept private; P3 Not available here |
| **MOV-A11-F3** Release dates and windows | Set when each way of watching opens in each country, such as buy first, the pass later, free after a year. | core | P1 Windows; P2 Windows open on time |
| **MOV-A11-F4** Travellers keep paid access | Keep watching what you paid for when you travel, where the law asks for it and the rights allow. | core | P1 Portability rules; P2 Home country check |
| **MOV-A11-F5** When rights end | Be told before a film leaves your country, and keep what you bought where the promise says. | core | P1 Rights end handling |
| **MOV-A11-F6** Rights disputes | When two people claim the same film, have it decided on evidence with the money held until then. | core | P1 Rights dispute |
| **MOV-A11-F7** Music clearances | List the music in your film so writers are paid where the law asks. | core | P1 Music cue sheet; P2 Collecting society licences |
| **MOV-A11-F8** Screening licences | Licence a film for a cinema, festival, school or community screening. | later | P1 Screening orders; P2 Screening files |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A11-F1-P1 | **Rights record**: For each film: countries, ways of watching (rent, buy, pass, free), start and end dates, holdbacks and deals elsewhere, as operations with a full history. | data | medium | MOV-A3-F1-P1, kit-ops |  |
| MOV-A11-F1-P2 | **Rights editor**: Pick countries from a list grouped by region, with quick choices such as 'worldwide' or 'worldwide except', and dates for each. | interface | medium | MOV-A11-F1-P1 |  |
| MOV-A11-F1-P3 | **Proof of rights**: Contracts, licences and releases kept privately in the kit's document vault (KIT-A15-F13-P1), opened only by moderators when a claim is made. | service | small | KIT-A15-F13-P1, MOV-A11-F1-P1 |  |
| MOV-A11-F1-P4 | **Rights declaration**: The filmmaker confirms they hold the rights, including music, archive footage and actors' releases, with published consequences for a false declaration. | legal | small | MOV-A17-F2-P1 |  |
| MOV-A11-F2-P1 | **Country check**: Works out the viewer's country for each play and sale and checks it against the rights record. | service | medium | MOV-A11-F1-P1, KIT-A19-F6-P1 |  |
| MOV-A11-F2-P2 | **Where you are, kept private**: The country is worked out from the network address at the moment of play and dropped, never stored; the rules for travel and private networks are published. | library | small | MOV-A11-F2-P1 |  |
| MOV-A11-F2-P3 | **Not available here**: The page says plainly when a film is not available in your country and why, with a button to be told if it comes. | interface | small | MOV-A11-F2-P1 |  |
| MOV-A11-F3-P1 | **Windows**: Release dates per country and per way of watching. | data | small | MOV-A11-F1-P1 |  |
| MOV-A11-F3-P2 | **Windows open on time**: Each window opens and closes on the kit's durable timers (KIT-A15-F14-P1), with the change logged. | service | small | MOV-A11-F3-P1, KIT-A15-F14-P1 |  |
| MOV-A11-F4-P1 | **Portability rules**: Paid viewers who are only visiting another EU country keep their access, as the EU's portability regulation asks of paid services, with each other country's rules in its pack. | legal | small | MOV-A11-F2-P1 |  |
| MOV-A11-F4-P2 | **Home country check**: The viewer's home country from their payment details, checked as the portability rules allow. | service | small | MOV-A11-F4-P1, svc-payments |  |
| MOV-A11-F5-P1 | **Rights end handling**: Tells viewers ahead, keeps buyers' access where the keep-access promise gives it, and refunds where it cannot. | service | small | MOV-A11-F3-P2, MOV-A6-F2-P3 |  |
| MOV-A11-F6-P1 | **Rights dispute**: A dispute on the kit's disputes service (KIT-A11-F1-P1) with evidence (KIT-A11-F2-P1), earnings held while it runs, and the court's decision followed where there is one. | integration | medium | KIT-A11-F1-P1, KIT-A11-F2-P1, MOV-A11-F1-P3 |  |
| MOV-A11-F7-P1 | **Music cue sheet**: The music used, with writers, publishers and timings, in the form collecting societies ask for. | data | small | MOV-A11-F1-P1 |  |
| MOV-A11-F7-P2 | **Collecting society licences**: Whether Spare Movies needs its own licences from music collecting societies to stream films in each country even when the filmmaker cleared the music, and the cost passed on at cost (unverified). | legal | medium |  |  |
| MOV-A11-F8-P1 | **Screening orders**: A screening licence as an order on the kit's marketplace engine, priced by the filmmaker, with the date and place. | integration | medium | kit-market, MOV-A11-F1-P1 |  |
| MOV-A11-F8-P2 | **Screening files**: A download for the licensed screening that stops working after the date. | service | small | MOV-A11-F8-P1, MOV-A18-F2-P1 |  |

## MOV-A12 Age ratings and content advice

The right age rating in every country: each country's rules in its pack, rating a film with the approved tool where a country allows it and an official rating where it asks for one, content advice in plain words, ratings shown in our own plain style, wrong ratings reported and checked, and warnings for flashing lights.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A12-F1** See the right rating for your country | See the film's rating in your own country, with the body that set it and the reasons. | core | P1 Ratings by country; P2 Rating and advice display; P3 Ratings rules pack |
| **MOV-A12-F2** Rate your film | Get your film rated for each country the simplest lawful way, with any fee shown. | core | P1 Rating questions; P2 Official rating request; P3 Films without a rating |
| **MOV-A12-F3** Content advice in plain words | See what is in a film, such as violence, strong language or suicide themes, before you press play. | core | P1 Content advice list; P2 Filter by advice |
| **MOV-A12-F4** Wrong ratings reported and checked | Report a rating that looks wrong and see it checked; an official rating always wins. | core | P1 Rating reports; P2 Rating audits |
| **MOV-A12-F5** Flashing lights warning | Be warned before a film with flashing that can trigger seizures. | next | P1 Flash detection |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A12-F1-P1 | **Ratings by country**: Each film's rating per country, with who set it (the official body, an approved tool or the filmmaker's own answers), the date and the reasons. | data | medium | MOV-A3-F1-P1, KIT-A19-F6-P1 |  |
| MOV-A12-F1-P2 | **Rating and advice display**: The country's rating as plain text in our own style, never copying official label artwork, with the content advice beside it. | interface | small | MOV-A12-F1-P1 |  |
| MOV-A12-F1-P3 | **Ratings rules pack**: For each country: the law, the body, whether on-demand films must be rated, whether rating with an approved tool is allowed, how ratings must be shown and the fees, kept in its country pack. | legal | large | KIT-A19-F6-P1 |  |
| MOV-A12-F2-P1 | **Rating questions**: Plain questions about what is in the film; where a country has an approved rating tool, the answers go to that tool on its terms; we never invent an official rating. | interface | medium | MOV-A12-F1-P3, MOV-A10-F1-P2 |  |
| MOV-A12-F2-P2 | **Official rating request**: Where a country needs an official rating, the filmmaker applies to the body, or Spare Movies does it at cost, with the fee shown before starting. | integration | medium | MOV-A12-F1-P3 |  |
| MOV-A12-F2-P3 | **Films without a rating**: What happens where a film has no rating yet: hidden from young profiles, or not shown in a country where the law forbids it. | legal | small | MOV-A12-F1-P3 |  |
| MOV-A12-F3-P1 | **Content advice list**: Plain categories with how strong each is, shown on the film page and before play. | data | small | MOV-A12-F1-P1 |  |
| MOV-A12-F3-P2 | **Filter by advice**: Hide films with chosen advice from a profile. | service | small | MOV-A12-F3-P1, MOV-A2-F2-P1 |  |
| MOV-A12-F4-P1 | **Rating reports**: A report type for a wrong rating or missing advice, reviewed by a moderator. | data | small | MOV-A17-F1-P1 |  |
| MOV-A12-F4-P2 | **Rating audits**: Regular sample checks of filmmakers' own ratings, with the results published. | service | small | MOV-A12-F4-P1 |  |
| MOV-A12-F5-P1 | **Flash detection**: Checks films for flashing against published thresholds and adds a warning the filmmaker can confirm. | library | medium | engine-media |  |

## MOV-A13 Captions, audio description and dubbing

Every film watchable by people who are Deaf, hard of hearing, blind or have low vision: captions in the main language before publishing, caption drafts from speech that people check, audio description recorded or read aloud from text, audio languages and labelled AI dubbing, subtitles in other languages, sign language versions, and the access law in each country.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A13-F1** Captions on every film | Watch every film with captions in its main language, checked for timing and reading speed. | core | P1 Caption tracks; P2 Caption checks; P3 Captions for Deaf and hard of hearing viewers |
| **MOV-A13-F2** Caption drafts from speech | Get a caption draft made from the soundtrack, then fix it before publishing. | core | P1 Speech to text drafts; P2 Caption editor |
| **MOV-A13-F3** Audio description | Hear what is happening on screen, from a recorded description track or descriptions read aloud. | core | P1 Audio description tracks; P2 Descriptions read aloud; P3 Description writer; P4 Find films with description |
| **MOV-A13-F4** Audio languages and dubbing | Hear a film in another language when a dubbed track exists, and know when the voices were made with AI. | core | P1 Language tracks; P2 AI dubbing label |
| **MOV-A13-F5** Subtitles in other languages | Watch with subtitles in your language, drafted by machine translation and checked by people. | next | P1 Translated subtitles; P2 Translator credits |
| **MOV-A13-F6** Access law for every country | Know the captions, audio description and player access each country's law asks for are met. | core | P1 Access duties research |
| **MOV-A13-F7** Sign language versions | Watch a version with a sign language interpreter in the picture. | later | P1 Sign language version |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A13-F1-P1 | **Caption tracks**: Upload WebVTT or TTML (IMSC) captions through the kit's captions library (KIT-A27-F8-P5), with timing and styling kept. | integration | small | KIT-A27-F8-P5, KIT-A19-F4-P1 |  |
| MOV-A13-F1-P2 | **Caption checks**: Reading speed, timing against speech, gaps with no captions and overlaps, flagged before publishing. | service | small | MOV-A13-F1-P1 |  |
| MOV-A13-F1-P3 | **Captions for Deaf and hard of hearing viewers**: Speaker names, sounds and music marked, kept apart from plain subtitles so each can be chosen. | data | small | MOV-A13-F1-P1 |  |
| MOV-A13-F2-P1 | **Speech to text drafts**: Drafts from the kit's speech recognition (KIT-A27-F12-P4), an openly licensed model run at cost; a draft nobody checked is marked as automatic. | integration | medium | KIT-A27-F12-P4, KIT-A19-F4-P1 |  |
| MOV-A13-F2-P2 | **Caption editor**: Fix text and timing beside the film, with the waveform and shortcut keys. | interface | medium | MOV-A13-F2-P1 |  |
| MOV-A13-F3-P1 | **Audio description tracks**: A recorded description track, mixed or separate, carried by the kit's extra tracks (KIT-A27-F8-P5). | integration | small | KIT-A27-F8-P5, MOV-A18-F2-P1 |  |
| MOV-A13-F3-P2 | **Descriptions read aloud**: A timed text description script read by a synthetic voice (KIT-A27-F12-P5) or the viewer's own screen reader, for filmmakers who cannot pay for a recording; marked as synthetic. | library | medium | KIT-A27-F12-P5, MOV-A13-F3-P1 |  |
| MOV-A13-F3-P3 | **Description writer**: An editor that shows the gaps in dialogue where descriptions fit. | interface | small | MOV-A13-F3-P2 |  |
| MOV-A13-F3-P4 | **Find films with description**: Films with audio description are marked, so blind viewers can filter for them. | data | small | MOV-A13-F3-P1 |  |
| MOV-A13-F4-P1 | **Language tracks**: Extra audio tracks with their language and whether they are original or dubbed. | data | small | MOV-A3-F1-P1, KIT-A27-F8-P5 |  |
| MOV-A13-F4-P2 | **AI dubbing label**: A dubbed track made with AI voices is labelled, and the actors' consent to it is recorded. | data | small | MOV-A5-F1-P1, MOV-A13-F4-P1 |  |
| MOV-A13-F5-P1 | **Translated subtitles**: Drafts from the kit's machine translation at cost (KIT-A19-F10-P2), checked by the filmmaker or volunteer translators before they show. | integration | medium | KIT-A19-F10-P2, MOV-A13-F1-P1 |  |
| MOV-A13-F5-P2 | **Translator credits**: Translators are credited and can be given a share of earnings by the filmmaker (A14-F3-P1). | service | small | MOV-A13-F5-P1, MOV-A14-F3-P1 |  |
| MOV-A13-F6-P1 | **Access duties research**: What each launch country requires of on-demand services for captions, audio description and accessible players, building on the kit's accessibility law research (KIT-A19-F2-P8). | legal | medium | KIT-A19-F2-P8 |  |
| MOV-A13-F7-P1 | **Sign language version**: A sign language version added as an extra version of the film (A3-F6-P1), marked so it can be found. | data | small | MOV-A3-F6-P1 |  |

## MOV-A14 Filmmaker earnings and payouts

Money to the people who made the film: earnings by film and by way of watching with the at-cost fee on every line, payouts on a schedule, splits among producers, cast and crew, tax statements and reporting, money held only when there is a reason, paying back investors first, and pre-orders.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A14-F1** See your earnings | See every sale, rental, support payment and pass share, with tax, the fee and what you got. | core | P1 Earnings ledger; P2 Earnings page; P3 Accounting export |
| **MOV-A14-F2** Get paid out | Get your money on a set schedule to your bank, with any failed payout fixed quickly. | core | P1 Payout set-up; P2 Payout runs; P3 Payout change guard |
| **MOV-A14-F3** Split earnings with your team | Share a film's earnings with producers, cast and crew by agreed percentages, each paid directly. | core | P1 Split rules; P2 Split payouts |
| **MOV-A14-F4** Tax statements and reporting | Get a yearly statement, and know your income is reported where the law asks. | core | P1 Yearly statements; P2 Platform income reporting; P3 Tax on payouts across borders; P4 Sanctions screening |
| **MOV-A14-F5** Money held only with a reason | Know money is held only for a refund window, a rights dispute or a fake viewing check, with the reason shown. | core | P1 Hold rules |
| **MOV-A14-F6** Pay back investors first | Pay investors and funders back in an agreed order before the splits start. | next | P1 Recoupment order |
| **MOV-A14-F7** Pre-orders | Let viewers buy a film before release to help fund it. | later | P1 Pre-orders |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A14-F1-P1 | **Earnings ledger**: Every money movement per film, with tax, the provider charge, the running-cost share and the net amount. | service | medium | svc-payments, MOV-A6-F2-P1 |  |
| MOV-A14-F1-P2 | **Earnings page**: Earnings by film, country and way of watching, on the kit's earnings page (KIT-A3-F3-P3). | interface | medium | MOV-A14-F1-P1, KIT-A3-F3-P3 |  |
| MOV-A14-F1-P3 | **Accounting export**: Exports for accounting software from the kit (KIT-A4-F4-P2). | integration | small | KIT-A4-F4-P2, MOV-A14-F1-P1 |  |
| MOV-A14-F2-P1 | **Payout set-up**: Bank details on the kit's payout set-up (KIT-A3-F3-P1) after the checks (A1-F3-P4). | integration | small | KIT-A3-F3-P1, MOV-A1-F3-P4 | [K02](../kit/STATUS.md) |
| MOV-A14-F2-P2 | **Payout runs**: Weekly or monthly payouts on the kit's payout runs (KIT-A3-F3-P2), with failed payouts handled (KIT-A3-F3-P5). | integration | small | KIT-A3-F3-P2, KIT-A3-F3-P5 | [K02](../kit/STATUS.md) |
| MOV-A14-F2-P3 | **Payout change guard**: Uses the kit's payout change guard (KIT-A3-F3-P4), so a hijacked account cannot redirect earnings. | integration | small | KIT-A3-F3-P4 |  |
| MOV-A14-F3-P1 | **Split rules**: Percentage splits per film, agreed and signed by every person in them (KIT-A25-F1-P2); a change needs everyone's signature again. | data | small | MOV-A14-F1-P1, KIT-A25-F1-P2 |  |
| MOV-A14-F3-P2 | **Split payouts**: Each person in a split is paid their share directly in the payout run. | service | medium | MOV-A14-F3-P1, MOV-A14-F2-P2 |  |
| MOV-A14-F4-P1 | **Yearly statements**: The kit's yearly statements (KIT-A4-F4-P1). | integration | small | KIT-A4-F4-P1 |  |
| MOV-A14-F4-P2 | **Platform income reporting**: Income reported under the platform reporting rules that apply (KIT-A4-F3). | integration | medium | KIT-A4-F3-P1, KIT-A4-F3-P2 |  |
| MOV-A14-F4-P3 | **Tax on payouts across borders**: Whether payouts to filmmakers in other countries count as royalties that need tax held back, and the forms each country asks for (unverified). | legal | medium |  |  |
| MOV-A14-F4-P4 | **Sanctions screening**: Payouts never reach sanctioned people, through the kit's screening (KIT-A4-F8-P1). | integration | small | KIT-A4-F8-P1 |  |
| MOV-A14-F5-P1 | **Hold rules**: Published reasons and time limits for holding earnings, on the kit's hold and release rules (KIT-A3-F2-P1). | legal | small | KIT-A3-F2-P1 |  |
| MOV-A14-F6-P1 | **Recoupment order**: An agreed order of who is paid back how much before the percentage splits begin, shown to everyone in it. | service | medium | MOV-A14-F3-P1 |  |
| MOV-A14-F7-P1 | **Pre-orders**: A purchase that opens on the release date, refunded in full if the film never comes out. | integration | small | kit-market, MOV-A6-F2-P1 |  |

## MOV-A15 Filmmaker studio and stats

Running a catalogue: one dashboard for films, checks, ratings, rights, protection and earnings, honest numbers on watching that never track a person, a team with roles, promotion without paid ranking, replies to viewers, and online premieres.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A15-F1** One dashboard for your films | See every film's state in one place: drafts, checks, ratings, rights, protection and earnings. | core | P1 Studio home; P2 To-do list |
| **MOV-A15-F2** See how your films are doing | See plays, watch time, how far people watch and which countries, without anyone being tracked. | core | P1 Watch numbers; P2 Numbers page |
| **MOV-A15-F3** Work as a team | Give producers, editors and accountants their own roles on your films. | core | P1 Team roles |
| **MOV-A15-F4** Promote your film | Share a press page, an embeddable trailer and a QR code for posters, and write to your followers. | next | P1 Press page; P2 Trailer embed; P3 Newsletters to followers |
| **MOV-A15-F5** Reply to viewers | Answer questions and reviews from one inbox. | next | P1 Viewer inbox |
| **MOV-A15-F6** Online premieres | Hold an online premiere at a set time with a question time after. | later | P1 Premieres |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A15-F1-P1 | **Studio home**: A list of films with their state and the next thing each needs. | interface | medium | MOV-A10-F1-P1, MOV-A14-F1-P2 |  |
| MOV-A15-F1-P2 | **To-do list**: Missing captions, expiring rights, ratings due and disputes, on the kit's seller to-do page (KIT-A5-F23-P2). | integration | small | KIT-A5-F23-P2 |  |
| MOV-A15-F2-P1 | **Watch numbers**: Counted privately (KIT-A22-F8-P1), never per person, with small numbers hidden (KIT-A13-F9-P2). | service | medium | MOV-A9-F2-P1, KIT-A22-F8-P1, KIT-A13-F9-P2 |  |
| MOV-A15-F2-P2 | **Numbers page**: Plain charts by week, country and way of watching. | interface | small | MOV-A15-F2-P1 |  |
| MOV-A15-F3-P1 | **Team roles**: Owner, editor, finance and viewer roles on the kit's team management (KIT-A1-F7-P2). | integration | small | KIT-A1-F7-P2, MOV-A1-F3-P2 |  |
| MOV-A15-F4-P1 | **Press page**: A press page with stills, synopsis, credits and contacts. | service | small | MOV-A3-F1-P3 |  |
| MOV-A15-F4-P2 | **Trailer embed**: An embeddable trailer player with no tracking. | interface | small | MOV-A4-F1-P1 |  |
| MOV-A15-F4-P3 | **Newsletters to followers**: Write to followers who agreed, on the kit's newsletters (KIT-A28-F5-P2). | integration | small | KIT-A28-F5-P2, MOV-A9-F3-P1 |  |
| MOV-A15-F5-P1 | **Viewer inbox**: Questions and reviews in one place on the kit's enquiry inbox (KIT-A5-F29-P1). | integration | small | KIT-A5-F29-P1, MOV-A3-F7-P1 |  |
| MOV-A15-F6-P1 | **Premieres**: A set start time for everyone with a ticket, played in step, then a chat with the filmmaker. | service | medium | MOV-A10-F5-P1, MOV-A4-F11-P1 |  |

## MOV-A16 Stopping copies

Making films hard to copy and pass around without paying, honestly: the open decision between locked playback, a hidden watermark per viewer and no lock, encrypted delivery and a key service, fingerprints that spot stolen uploads before they earn anything, tracing a leak back to an account, help with takedowns on other sites, and a plain promise to viewers. Nothing stops someone recording the screen as it plays.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A16-F1** Protect playback | Know that a film is delivered so it cannot simply be downloaded and passed around. | core | P1 Protection decision; P2 Encrypted delivery; P3 Key service; P4 Signed, expiring links |
| **MOV-A16-F2** Fingerprints that spot stolen uploads | Know that someone cannot upload your film and collect money for it. | core | P1 Fingerprint method; P2 Fingerprint every film; P3 Check every upload; P4 Match review |
| **MOV-A16-F3** A plain promise to viewers | Know what the protection does, what it never collects and which devices it works on. | core | P1 Protection promise |
| **MOV-A16-F4** Hidden watermark per viewer | Have each stream carry a hidden mark, so a leaked copy leads back to the account it came from. | next | P1 Watermark method; P2 Watermarked delivery; P3 Leak tracing |
| **MOV-A16-F5** Help with copies on other sites | Check a link to a copy on another site and get a takedown notice ready to send. | next | P1 Copy check and notice |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A16-F1-P1 | **Protection decision**: Compare locked playback (Widevine, FairPlay and PlayReady are closed systems and each needs an agreement with its company), a hidden per-viewer watermark and no lock, on cost, reach, privacy, patents and what filmmakers and their investors require. | research | medium |  |  |
| MOV-A16-F1-P2 | **Encrypted delivery**: Films encrypted once with the Common Encryption standard (CENC, cbcs), so the same files work with any locked system if one is chosen. | library | medium | MOV-A18-F3-P1, MOV-A16-F1-P1 |  |
| MOV-A16-F1-P3 | **Key service**: Gives out playback keys per session after checking the access right, the country and the device limit, on the kit's encryption and keys (KIT-A22-F5-P5). | service | large | MOV-A16-F1-P2, KIT-A22-F5-P5 |  |
| MOV-A16-F1-P4 | **Signed, expiring links**: Delivery links tied to the session that expire, on the kit's sharing rules (KIT-A15-F8-P1). | integration | small | KIT-A15-F8-P1 |  |
| MOV-A16-F2-P1 | **Fingerprint method**: Compare open video and audio fingerprinting methods on accuracy, speed and licence, starting with Meta's TMK+PDQF and vPDQ (BSD-2-Clause), with a patent search before any code. | research | medium |  |  |
| MOV-A16-F2-P2 | **Fingerprint every film**: Fingerprints made for every film and trailer on upload and stored privately. | service | medium | MOV-A16-F2-P1, MOV-A18-F1-P1 |  |
| MOV-A16-F2-P3 | **Check every upload**: New uploads are matched against every film; a match pauses publishing and goes to a person, never removed or paid automatically. | service | medium | MOV-A16-F2-P2 |  |
| MOV-A16-F2-P4 | **Match review**: Both films side by side with the matching stretches, for a moderator to decide, logged in public. | interface | small | MOV-A16-F2-P3, MOV-A17-F7-P1 |  |
| MOV-A16-F3-P1 | **Protection promise**: Plain words on what protection is used, that it installs nothing hidden and tracks nothing, and which devices can play. | legal | small | MOV-A16-F1-P1 |  |
| MOV-A16-F4-P1 | **Watermark method**: Compare watermarking methods, such as two marked copies of each piece with every viewer getting a unique pattern of them, with a patent search before any code; the field is heavily patented (unverified). | research | medium |  |  |
| MOV-A16-F4-P2 | **Watermarked delivery**: Each session gets its own pattern of marked pieces, chosen at delivery from two marked copies of each piece. | service | large | MOV-A16-F4-P1, MOV-A18-F3-P1 |  |
| MOV-A16-F4-P3 | **Leak tracing**: Reads the mark from a leaked copy and finds the account; a person reviews before any action, and the action is logged. | service | medium | MOV-A16-F4-P2 |  |
| MOV-A16-F5-P1 | **Copy check and notice**: The filmmaker submits a link; the copy is checked against the fingerprint and a notice is prepared on the kit's notice templates (KIT-A10-F21-P2) for the filmmaker to send. | service | small | MOV-A16-F2-P2, KIT-A10-F21-P2 |  |

## MOV-A17 Moderation, legal removals and transparency

Spare Movies removes only what the law requires, in the open: reports from anyone, published rules, illegal material removed fast and blocked only where it is illegal, copyright notices and counter-notices, a public log of every removal and section move with the law behind it, appeals, moderators that filmmakers elect, government requests handled by law, and fake filmmakers stopped.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A17-F1** Report a film or account | Report a film, trailer, review or account in two taps, with or without an account. | core | P1 Report types; P2 Reports without an account |
| **MOV-A17-F2** Read the rules | Read Spare Movies' rules in plain words, including what it will never remove. | core | P1 Spare Movies rules |
| **MOV-A17-F3** Illegal material removed fast | Know that material the law forbids is removed fast, and blocked only in the countries where it is illegal. | core | P1 Legal duties per country; P2 Known illegal content matching; P3 Blocked only where illegal |
| **MOV-A17-F4** Copyright notices | Send or answer a copyright notice, with deadlines tracked and a counter-notice for mistakes. | core | P1 Notice and counter-notice; P2 Repeat infringer rule; P3 Registered notice agent |
| **MOV-A17-F5** See every removal in public | Read a public log of every removal, block and section move, with the rule and the law behind it. | core | P1 Removal log entries; P2 Log screens; P3 Transparency reports |
| **MOV-A17-F6** Appeal any decision | Appeal any removal, block, section move or rating change, and get a reasoned answer. | core | P1 Appeals |
| **MOV-A17-F7** Moderators filmmakers elect | Know that reports are decided by trained moderators elected by filmmakers, checked for consistency. | core | P1 Review queue; P2 Elected moderators; P3 Moderator wellbeing |
| **MOV-A17-F8** Government and police requests | Know that requests from governments and police are handled only by law and counted in public. | core | P1 Requests handled by law |
| **MOV-A17-F9** Fake filmmakers stopped | Know that accounts pretending to be a filmmaker to sell their films are stopped. | core | P1 Impersonation checks |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A17-F1-P1 | **Report types**: Illegal content, copyright, wrong section, wrong rating, privacy and impersonation, on the kit's reports service (KIT-A10-F3). | integration | small | KIT-A10-F3-P1, KIT-A10-F3-P2 |  |
| MOV-A17-F1-P2 | **Reports without an account**: Rights holders and the public can report without joining (KIT-A10-F3-P3). | integration | small | KIT-A10-F3-P3 |  |
| MOV-A17-F2-P1 | **Spare Movies rules**: The published rules on the kit's rules library (KIT-A10-F5-P1): what the law requires to be removed in each country, and the promise that lawful films stay up. | legal | medium | KIT-A10-F5-P1, KIT-A10-F9-P1 |  |
| MOV-A17-F3-P1 | **Legal duties per country**: What each country requires a film host to remove and how fast, including classification law, court orders and platform laws such as the EU Digital Services Act (KIT-A18-F13-P1). | legal | large | KIT-A10-F9-P1, KIT-A18-F13-P1 |  |
| MOV-A17-F3-P2 | **Known illegal content matching**: Every upload checked against approved hash lists through the kit (KIT-A10-F10-P2), with matches reported to the right agency. | integration | medium | KIT-A10-F10-P2 | [M07](../spare-social/STATUS.md) |
| MOV-A17-F3-P3 | **Blocked only where illegal**: A film banned in one country is blocked there only, with the law named in the public log. | service | small | MOV-A11-F2-P1, MOV-A17-F3-P1 |  |
| MOV-A17-F4-P1 | **Notice and counter-notice**: Copyright notices and counter-notices on the kit's notice handling and deadlines (KIT-A10-F21-P2, KIT-A10-F9-P2), with earnings held while a notice is open. | integration | medium | KIT-A10-F21-P2, KIT-A10-F9-P2 |  |
| MOV-A17-F4-P2 | **Repeat infringer rule**: A published rule for accounts that upload others' films again and again. | legal | small | MOV-A17-F4-P1 |  |
| MOV-A17-F4-P3 | **Registered notice agent**: An agent for copyright notices registered where the law asks, such as with the US Copyright Office, renewed on time, with the contact details published. | legal | small | MOV-A17-F4-P1 |  |
| MOV-A17-F5-P1 | **Removal log entries**: Spare Movies' entries on the kit's public removal log (KIT-A10-F7-P1): the film, the countries, the law or rule, who asked, the date and the appeal state, with no personal details. | integration | small | KIT-A10-F7-P1 |  |
| MOV-A17-F5-P2 | **Log screens**: The log with filters by reason, country and month, and counts at the top. | interface | small | MOV-A17-F5-P1 |  |
| MOV-A17-F5-P3 | **Transparency reports**: Spare Movies' numbers in the kit's transparency reports (KIT-A10-F15-P1). | integration | small | KIT-A10-F15-P1 |  |
| MOV-A17-F6-P1 | **Appeals**: Statements of reasons and appeals on the kit (KIT-A10-F8-P1, KIT-A11-F5-P1). | integration | small | KIT-A11-F5-P1, KIT-A10-F8-P1 |  |
| MOV-A17-F7-P1 | **Review queue**: Reports, fingerprint matches and section hints in the kit's review queue and moderator tools (KIT-A10-F6). | integration | small | KIT-A10-F6-P2, KIT-A10-F6-P3 |  |
| MOV-A17-F7-P2 | **Elected moderators**: Moderators elected on the kit's elections (KIT-A12-F6-P1), with decisions sampled for consistency (KIT-A10-F20-P1). | integration | small | KIT-A12-F6-P1, KIT-A10-F20-P1 |  |
| MOV-A17-F7-P3 | **Moderator wellbeing**: The kit's moderator wellbeing support (KIT-A10-F6-P4), since reviewing films can mean hours of distressing material. | integration | small | KIT-A10-F6-P4 |  |
| MOV-A17-F8-P1 | **Requests handled by law**: The kit's law enforcement requests (KIT-A10-F9-P3), counted in the transparency report. | integration | small | KIT-A10-F9-P3 |  |
| MOV-A17-F9-P1 | **Impersonation checks**: Scam methods and fraud signals from the kit (KIT-A10-F12), with the payout checks (A1-F3-P4) before any money moves. | integration | small | KIT-A10-F12-P1, KIT-A10-F12-P2 |  |

## MOV-A18 Video pipeline and delivery at cost

The machinery under every film: master files taken in safely and kept, encoding into a ladder of sizes with open codecs where devices allow, packaging for streaming, an openly licensed player core, storage in tiers, delivery bought at cost and metered for the ledger, the fee worked out from real costs, safe processing, deletion that really deletes, peer-assisted delivery, and HDR and surround sound.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A18-F1** Take in a master safely | Know your master file is taken in safely, checked and kept for later. | core | P1 Master intake; P2 Technical analysis; P3 Master archive |
| **MOV-A18-F2** Every size from one master | Have every film turned into the sizes phones, computers and TVs need. | core | P1 Encoding ladder; P2 Codec choice; P3 Encoding queue; P4 Sound mixes and loudness |
| **MOV-A18-F3** Streaming on any connection | Have films packaged so they stream on any device and connection. | core | P1 Packaging; P2 Player core choice; P3 Thumbnails and scrub previews |
| **MOV-A18-F4** Delivered at cost | Know films are stored and delivered as cheaply as possible, with the costs published. | core | P1 Storage tiers; P2 Delivery network choice; P3 Delivery metering |
| **MOV-A18-F5** The fee from real costs | Know the fee on each sale comes from what delivery, storage, encoding and payments really cost. | core | P1 Cost per sale and per hour |
| **MOV-A18-F6** Media handled safely | Know a booby-trapped file or link cannot break into the servers. | core | P1 Sandboxed processing; P2 Safe outside fetches |
| **MOV-A18-F7** Deleted means deleted | When a film is taken down for good, every copy, size and cached piece is gone within a published time. | core | P1 Deletion fan-out; P2 Legal hold |
| **MOV-A18-F8** Viewers share pieces of popular films | Opt in to share pieces of films you are watching with nearby viewers, to cut delivery costs. | later | P1 Peer-assisted delivery |
| **MOV-A18-F9** HDR and surround sound | Watch in HDR and hear surround sound where your TV and the film allow. | later | P1 HDR versions; P2 Surround sound |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A18-F1-P1 | **Master intake**: Takes in large master files through the kit's file service, unpacked in a sandbox (A18-F6-P1). | service | medium | KIT-A15-F7-P1, MOV-A18-F6-P1 |  |
| MOV-A18-F1-P2 | **Technical analysis**: Reads every stream, measures loudness and finds damaged frames, for the quality checks (A10-F2-P1). | library | medium | engine-media |  |
| MOV-A18-F1-P3 | **Master archive**: Originals kept in cold storage at cost, so films can be encoded again as codecs improve. | data | medium | kit-store |  |
| MOV-A18-F2-P1 | **Encoding ladder**: Sizes and bitrates chosen per film, on the kit's decoding and encoding (KIT-A27-F8-P2). | service | large | engine-media, KIT-A27-F8-P2 | [M03](../spare-social/STATUS.md) |
| MOV-A18-F2-P2 | **Codec choice**: AV1 where devices play it, H.264 for reach, and when HEVC is worth its patent licences, from the kit's one codec and patent study (KIT-A27-F8-P1). | research | medium | KIT-A27-F8-P1 | [M03](../spare-social/STATUS.md) |
| MOV-A18-F2-P3 | **Encoding queue**: Jobs and workers for encoding, recording machine time per film for the cost ledger. | service | medium | kit-ops, svc-ledger |  |
| MOV-A18-F2-P4 | **Sound mixes and loudness**: Stereo from any mix, levelled to a published loudness target. | library | small | engine-media |  |
| MOV-A18-F3-P1 | **Packaging**: HLS and DASH from one set of CMAF files, with captions and extra audio tracks. | library | medium | engine-media, MOV-A18-F2-P1 |  |
| MOV-A18-F3-P2 | **Player core choice**: Pick the player core from openly licensed ones such as Shaka Player and hls.js (both Apache-2.0) and dash.js (BSD-3-Clause), on device reach, captions, audio description and protection support. | research | small |  |  |
| MOV-A18-F3-P3 | **Thumbnails and scrub previews**: Frames for the seek bar and posters picked from the film. | library | small | engine-media |  |
| MOV-A18-F4-P1 | **Storage tiers**: Popular and new films in fast storage, the rest in cheaper storage, on the kit's storage at cost (KIT-A15-F7-P2). | service | medium | kit-store, KIT-A15-F7-P2 | [M06](../spare-social/STATUS.md) |
| MOV-A18-F4-P2 | **Delivery network choice**: Compare delivery networks bought at cost, co-op caches and member servers on price per gigabyte in each region and reach to TVs and phones. | research | medium |  | [M06](../spare-social/STATUS.md) |
| MOV-A18-F4-P3 | **Delivery metering**: Gigabytes delivered per film and country, on the kit's metering (KIT-A13-F2), feeding the fee and the ledger. | service | medium | svc-ledger, KIT-A13-F2-P2, KIT-A13-F2-P3 | [K06](../kit/STATUS.md) |
| MOV-A18-F5-P1 | **Cost per sale and per hour**: Works out last period's running cost per sale and per hour delivered, on the kit's fee setting (KIT-A13-F3-P1). | service | small | MOV-A18-F4-P3, KIT-A13-F3-P1 | [K06](../kit/STATUS.md) |
| MOV-A18-F6-P1 | **Sandboxed processing**: Encoders and file readers run in a locked-down sandbox with no network access. | service | medium | MOV-A18-F2-P3 |  |
| MOV-A18-F6-P2 | **Safe outside fetches**: Fetching a master from a link refuses internal addresses, follows few redirects and caps size and time. | library | small | kit-core |  |
| MOV-A18-F7-P1 | **Deletion fan-out**: Removes every size, copy and delivery cache, keeping buyers' access only where the keep-access promise says. | service | medium | KIT-A18-F2-P1, MOV-A18-F4-P1 |  |
| MOV-A18-F7-P2 | **Legal hold**: Keeps evidence only where the law requires it, sealed and logged. | legal | small | KIT-A10-F9-P1 |  |
| MOV-A18-F8-P1 | **Peer-assisted delivery**: Players share pieces with each other over WebRTC, opt-in and within data limits; we learn from PeerTube's method and write our own code, never copying its AGPL code. | library | large | MOV-A18-F3-P1 | [M06](../spare-social/STATUS.md) |
| MOV-A18-F9-P1 | **HDR versions**: HDR in open formats such as HDR10 and HLG; closed HDR formats only after their licences are checked. | library | medium | MOV-A18-F2-P1 | [M03](../spare-social/STATUS.md) |
| MOV-A18-F9-P2 | **Surround sound**: 5.1 tracks where the master has them, in codecs whose licences are checked. | library | small | MOV-A18-F2-P4 |  |

## MOV-A19 Apps and devices

Watching on what you have: a web app in any browser that installs on phones, store apps, settings that follow you, slow connections and old devices, TV apps, and a computer app.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A19-F1** Watch in a browser | Watch in any modern browser on a computer, phone or tablet. | core | P1 Web app |
| **MOV-A19-F2** Install on your phone | Install Spare Movies on a phone from the browser or an app store. | core | P1 Installable app; P2 Store apps |
| **MOV-A19-F3** Settings that follow you | Have your profile, captions and choices the same on every device. | core | P1 Preferences that follow you |
| **MOV-A19-F4** Slow connections and old devices | Watch on an old phone or a slow connection. | core | P1 Light mode |
| **MOV-A19-F5** Apps for TVs | Watch on a smart TV or TV box with an app you control by remote. | next | P1 TV app approach; P2 TV app |
| **MOV-A19-F6** Computer app | Install Spare Movies on a computer to watch offline. | later | P1 Desktop app |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A19-F1-P1 | **Web app**: Spare Movies in the kit's app frame (KIT-A20-F1-P2), laid out for phones, computers and big screens. | interface | large | KIT-A20-F1-P2, MOV-A4-F1-P2 |  |
| MOV-A19-F2-P1 | **Installable app**: The kit's installable apps (KIT-A20-F2-P2). | integration | small | KIT-A20-F2-P2 | [K04](../kit/STATUS.md) |
| MOV-A19-F2-P2 | **Store apps**: Store apps that meet each store's rules (KIT-A20-F8-P1), with paying handled as A6-F10 decides. | integration | medium | KIT-A20-F8-P1, MOV-A19-F2-P1 | [K04](../kit/STATUS.md) |
| MOV-A19-F3-P1 | **Preferences that follow you**: Uses the kit's preferences that follow you (KIT-A20-F5-P2). | integration | small | KIT-A20-F5-P2 |  |
| MOV-A19-F4-P1 | **Light mode**: The kit's light mode (KIT-A19-F5-P1): small images, low data, the smallest video sizes. | integration | small | KIT-A19-F5-P1 |  |
| MOV-A19-F5-P1 | **TV app approach**: Compare smart TV platforms and web-based TV apps on reach, store terms, protection needs and the cost of keeping each up to date. | research | medium |  | [K04](../kit/STATUS.md) |
| MOV-A19-F5-P2 | **TV app**: A remote-friendly app with large text, the same captions and audio description, and TV code sign-in (A1-F6-P1). | interface | large | MOV-A19-F5-P1, MOV-A4-F1-P1, MOV-A1-F6-P1 |  |
| MOV-A19-F6-P1 | **Desktop app**: The kit's desktop shell and installers (KIT-A20-F2-P4, KIT-A20-F2-P5), with offline downloads (A4-F9-P1). | integration | medium | KIT-A20-F2-P4, KIT-A20-F2-P5, MOV-A4-F9-P1 |  |

## MOV-A20 Accessible apps and languages

Every screen usable by everyone: screen readers, keyboards and remotes, a player whose every control is announced, larger text and higher contrast, reduced motion, the interface and film details in many languages, local formats, an accessibility statement, easy-read help and right-to-left layouts.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A20-F1** Works with screen readers, keyboards and remotes | Use every screen and every player control with a screen reader, a keyboard or a TV remote. | core | P1 Accessible screens; P2 Accessible player |
| **MOV-A20-F2** Comfortable to use | Make text larger, raise contrast and stop trailers moving on their own. | core | P1 Display settings; P2 Reduced motion |
| **MOV-A20-F3** Your language | Use Spare Movies in your language and read film details in it where they are translated. | core | P1 Interface translations; P2 Film details in many languages |
| **MOV-A20-F4** Local formats | See dates, times, lengths and money the way your country writes them. | core | P1 Local formats |
| **MOV-A20-F5** Accessibility statement | Read what works, what does not yet, and report a barrier. | core | P1 Statement and barrier reports |
| **MOV-A20-F6** Easy-read help | Read the key steps in easy-read words and sign language. | next | P1 Easy read |
| **MOV-A20-F7** Right-to-left languages | Use Spare Movies in Arabic, Hebrew and other right-to-left languages. | next | P1 Right-to-left layout |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A20-F1-P1 | **Accessible screens**: Built from the kit's accessible components (KIT-A19-F2-P1) and tested (KIT-A19-F2-P2). | integration | medium | KIT-A19-F2-P1, KIT-A19-F2-P2 |  |
| MOV-A20-F1-P2 | **Accessible player**: Every player control reachable and announced, with caption and description switches in a fixed, easy place. | library | medium | MOV-A4-F1-P2, KIT-A19-F2-P1 |  |
| MOV-A20-F2-P1 | **Display settings**: The kit's display settings (KIT-A19-F3-P1). | integration | small | KIT-A19-F3-P1 |  |
| MOV-A20-F2-P2 | **Reduced motion**: No moving previews or trailers that start on their own when reduced motion is set. | data | small | KIT-A19-F3-P1 |  |
| MOV-A20-F3-P1 | **Interface translations**: The kit's translation framework and member translation (KIT-A19-F1). | integration | medium | KIT-A19-F1-P1, KIT-A19-F1-P3 |  |
| MOV-A20-F3-P2 | **Film details in many languages**: Titles and synopses in several languages on the kit's translated fields (KIT-A19-F1-P6). | data | small | KIT-A19-F1-P6, MOV-A3-F1-P1 |  |
| MOV-A20-F4-P1 | **Local formats**: The kit's local formats (KIT-A19-F1-P2). | integration | small | KIT-A19-F1-P2 |  |
| MOV-A20-F5-P1 | **Statement and barrier reports**: On the kit's accessibility statement and barrier reports (KIT-A19-F2-P4). | integration | small | KIT-A19-F2-P4 |  |
| MOV-A20-F6-P1 | **Easy read**: The kit's easy read and sign language videos (KIT-A19-F8-P1) for joining, paying and publishing. | integration | small | KIT-A19-F8-P1 |  |
| MOV-A20-F7-P1 | **Right-to-left layout**: The kit's right-to-left layout (KIT-A19-F7-P1), including the player. | integration | small | KIT-A19-F7-P1 |  |

## MOV-A21 Your data and privacy

No adverts and no tracking: what you watch is kept only for what you use it for and protected by video privacy law, a choice over AI training that is off by default, guards against scraping, download all your data, consents you can change, privacy duties met in each country, and data kept in the right place.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A21-F1** No adverts, no tracking | Watch with no adverts, no tracking and no data sold, ever. | core | P1 No adverts promise; P2 Private usage counts |
| **MOV-A21-F2** What you watch stays yours | Know what you watch is never shared, and is kept only as long as it is needed. | core | P1 Retention schedule; P2 Video privacy law |
| **MOV-A21-F3** Download all your data | Download everything Spare Movies holds about you. | core | P1 Data export |
| **MOV-A21-F4** Consents you can change | See and change every consent you gave, and what is collected about you. | core | P1 Consents page; P2 Collection notices |
| **MOV-A21-F5** Your rights under the law | Use your privacy rights in your country, answered on time. | core | P1 Privacy requests; P2 Privacy impact assessment; P3 Overseas privacy duties |
| **MOV-A21-F6** Films kept out of AI training | Know no film is used to train AI unless its filmmaker says yes. | core | P1 AI training choice; P2 Scraping guard |
| **MOV-A21-F7** Data kept in the right place | Have your data kept in your region where the law or your choice asks. | next | P1 Data location |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A21-F1-P1 | **No adverts promise**: No adverts and no sale of data, as a protected promise (A22-F1-P3). | legal | small | MOV-A22-F1-P3 |  |
| MOV-A21-F1-P2 | **Private usage counts**: How the app is used is counted on the kit's private usage counts (KIT-A22-F8-P1), never per person. | integration | small | KIT-A22-F8-P1 |  |
| MOV-A21-F2-P1 | **Retention schedule**: Watch history and watch-time counts on the kit's retention schedules (KIT-A18-F6-P1). | integration | small | KIT-A18-F6-P1, MOV-A9-F2-P1 |  |
| MOV-A21-F2-P2 | **Video privacy law**: Laws that protect which films a person watched, such as the US Video Privacy Protection Act, built into the rules for every country (unverified for countries other than the US). | legal | small | KIT-A18-F12-P1 |  |
| MOV-A21-F3-P1 | **Data export**: The kit's data export (KIT-A18-F1-P1), including purchases, lists, history and, for filmmakers, every film's records. | integration | small | KIT-A18-F1-P1 |  |
| MOV-A21-F4-P1 | **Consents page**: The kit's consents page (KIT-A18-F17-P2). | integration | small | KIT-A18-F17-P2 |  |
| MOV-A21-F4-P2 | **Collection notices**: The kit's collection notices (KIT-A18-F5-P1). | integration | small | KIT-A18-F5-P1 |  |
| MOV-A21-F5-P1 | **Privacy requests**: The kit's privacy page and request clock (KIT-A18-F3). | integration | small | KIT-A18-F3-P1, KIT-A18-F3-P2 |  |
| MOV-A21-F5-P2 | **Privacy impact assessment**: Spare Movies' assessment on the kit's method (KIT-A18-F10-P1), covering watch history, the country check, age checks and protection. | legal | medium | KIT-A18-F10-P1 |  |
| MOV-A21-F5-P3 | **Overseas privacy duties**: The kit's overseas privacy duties (KIT-A18-F12-P1). | integration | small | KIT-A18-F12-P1 |  |
| MOV-A21-F6-P1 | **AI training choice**: The kit's AI training choice (KIT-A17-F11-P1), off by default for every film, trailer and caption. | integration | small | KIT-A17-F11-P1 |  |
| MOV-A21-F6-P2 | **Scraping guard**: Bulk copy limits and traffic limits from the kit (KIT-A16-F1-P4, KIT-A22-F5-P4). | integration | small | KIT-A16-F1-P4, KIT-A22-F5-P4 |  |
| MOV-A21-F7-P1 | **Data location**: The kit's data location (KIT-A18-F9-P1). | integration | small | KIT-A18-F9-P1 |  |

## MOV-A22 Running Spare Movies: rules, costs, votes and help

How Spare Movies is run: filmmakers vote on its fees and rules and major changes go to a vote of members, protected core promises, the open cost ledger for delivery, storage, encoding, ratings and payments, help and support, safe servers, the legal set-up including local content duties, measured performance and insurance.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A22-F1** Filmmakers set the fees and rules | Propose and vote on Spare Movies' fees and rules as a filmmaker; major changes go to a vote of members. | core | P1 Proposals and votes; P2 Who votes on what; P3 Protected core promises |
| **MOV-A22-F2** See what it costs | See each month what delivery, storage, encoding, ratings, payments and moderation cost, and how the fee follows. | core | P1 Movies cost lines; P2 Fee explainer |
| **MOV-A22-F3** Get help | Find answers, ask a person and see if something is down. | core | P1 Help centre; P2 Support requests; P3 Status page |
| **MOV-A22-F4** Run the servers safely | Admins run, watch, back up and secure the servers, with every admin action logged. | core | P1 Admin console; P2 Monitoring and alerts; P3 Backups of masters; P4 Security reviews |
| **MOV-A22-F5** Legal set-up | Read plain terms for viewers and filmmakers and know which legal body runs the service. | core | P1 Terms for viewers and filmmakers; P2 Licence register; P3 Regulator notices; P4 Local content and levy duties |
| **MOV-A22-F6** Measured performance | Know how fast films start and how often they stall, measured and published. | core | P1 Performance budgets |
| **MOV-A22-F7** Insurance | Know the co-op is insured against claims over films it hosts. | next | P1 Media liability insurance |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A22-F1-P1 | **Proposals and votes**: Proposals and results through the kit's ballots (KIT-A12-F3-P1), one vote per person, never per profile, company or agent. | integration | small | svc-voting, KIT-A12-F3-P1, KIT-A12-F5-P2 | [K01](../kit/STATUS.md) |
| MOV-A22-F1-P2 | **Who votes on what**: Filmmakers, and the people in their splits, vote on Spare Movies' fees and rules; viewers can propose and are asked; major changes go to a vote of members (KIT-A12-F5). | legal | medium | KIT-A12-F5-P1, KIT-A12-F5-P2 | [K01](../kit/STATUS.md) |
| MOV-A22-F1-P3 | **Protected core promises**: No adverts, removing only what the law requires, and filmmakers keeping everything except running costs can change only by a vote of members. | legal | small | KIT-A12-F5-P1 | [K01](../kit/STATUS.md) |
| MOV-A22-F2-P1 | **Movies cost lines**: Monthly lines for gigabytes delivered and stored, encoding hours, rating fees, payment charges and moderation hours. | data | medium | svc-ledger, MOV-A18-F4-P3 | [K06](../kit/STATUS.md) |
| MOV-A22-F2-P2 | **Fee explainer**: The kit's fee explainer (KIT-A13-F3-P2) with a worked example for a rental, a purchase and a pass month. | integration | small | KIT-A13-F3-P2 | [K06](../kit/STATUS.md) |
| MOV-A22-F3-P1 | **Help centre**: Spare Movies' pages in the kit's help centre (KIT-A21-F1-P1). | integration | small | KIT-A21-F1-P1 |  |
| MOV-A22-F3-P2 | **Support requests**: The kit's support cases (KIT-A21-F2-P1). | integration | small | KIT-A21-F2-P1 |  |
| MOV-A22-F3-P3 | **Status page**: The kit's status page (KIT-A21-F5-P1). | integration | small | KIT-A21-F5-P1 |  |
| MOV-A22-F4-P1 | **Admin console**: Spare Movies' admin actions on the kit's admin console (KIT-A22-F6-P1), each with a stated reason. | integration | medium | KIT-A22-F6-P1 |  |
| MOV-A22-F4-P2 | **Monitoring and alerts**: The kit's monitoring (KIT-A22-F1-P1), with start times and stalls watched. | integration | small | KIT-A22-F1-P1 |  |
| MOV-A22-F4-P3 | **Backups of masters**: The kit's backups and restore drills (KIT-A22-F3-P1) extended to the master archive. | integration | medium | KIT-A22-F3-P1, MOV-A18-F1-P3 |  |
| MOV-A22-F4-P4 | **Security reviews**: Outside reviews of the key service and payments through the kit (KIT-A22-F5-P3). | integration | small | KIT-A22-F5-P3 |  |
| MOV-A22-F5-P1 | **Terms for viewers and filmmakers**: Plain terms for viewers and a plain agreement for filmmakers that match the published rules. | legal | medium | KIT-A18-F4-P2, MOV-A17-F2-P1 | [K01](../kit/STATUS.md) |
| MOV-A22-F5-P2 | **Licence register**: Spare Movies' entries in the repository's one licence register: every open-source dependency, its licence and where it came from. | legal | small |  | [A14](../spare-construct/STATUS.md) |
| MOV-A22-F5-P3 | **Regulator notices**: Tell regulators before starting where the law asks, such as Ofcom for an on-demand service based in the UK; which ones apply depends on where the operating body is set up (KIT-A18-F13-P1). | legal | small | KIT-A18-F13-P1 | [K01](../kit/STATUS.md) |
| MOV-A22-F5-P4 | **Local content and levy duties**: Whether rules such as the EU's share of European works on on-demand services, Canada's streaming contributions, Australia's local content rules or national film levies reach Spare Movies, from what size, and what they would cost. | legal | medium |  |  |
| MOV-A22-F6-P1 | **Performance budgets**: Targets for time to first frame, stalls per hour and catalogue load, measured on real connections in each launch country. | research | small |  |  |
| MOV-A22-F7-P1 | **Media liability insurance**: Cover for copyright and defamation claims over hosted films, compared on cost, with the premium on the ledger (KIT-A14-F17-P1). | research | small | KIT-A14-F17-P1 |  |

## MOV-A23 Headless API, plugins and AI agents

Everything by command, so people, scripts and AI agents use the same path: the command set and query API, an MCP server with approval before spending or publishing, a register of where Spare Movies uses AI, plugins, a developer API with a catalogue feed, and webhooks.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **MOV-A23-F1** Everything by command | Do anything a person can do, from browsing to publishing, through the same commands. | core | P1 Spare Movies command set; P2 Query API |
| **MOV-A23-F2** Use your own AI | Connect your AI agent to search, keep lists or run your catalogue, with approval before it spends or publishes. | core | P1 MCP server; P2 Approval before spending or publishing; P3 Guards against hidden instructions |
| **MOV-A23-F3** See where Spare Movies uses AI | See every place Spare Movies itself uses AI, such as caption drafts, read-aloud descriptions and detection hints. | core | P1 AI use register |
| **MOV-A23-F4** Plugins | Add plugins such as other caption formats or festival submission tools. | next | P1 Plugin points |
| **MOV-A23-F5** Developer API | Build tools on Spare Movies with a documented public API, and feed the catalogue to other sites. | next | P1 Public API; P2 Catalogue feed |
| **MOV-A23-F6** Webhooks | Be told by webhook when a film is published, sold or reported. | later | P1 Webhooks |

**Parts**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| MOV-A23-F1-P1 | **Spare Movies command set**: Every action (publish, price, rate, rent, buy, report, moderate) as a command on the kit's operation layer. | library | large | kit-ops, MOV-A3-F1-P1 |  |
| MOV-A23-F1-P2 | **Query API**: Read the catalogue, film pages, rights and earnings through one query API that respects every permission. | library | medium | MOV-A2-F1-P1 |  |
| MOV-A23-F2-P1 | **MCP server**: Spare Movies' commands on the kit's MCP server (KIT-A17-F1-P1) with MCP authorisation (KIT-A17-F1-P3). | integration | medium | kit-mcp, KIT-A17-F1-P1, KIT-A17-F1-P3, MOV-A23-F1-P1 |  |
| MOV-A23-F2-P2 | **Approval before spending or publishing**: A person approves before an agent rents, buys, publishes, changes prices or rights (KIT-A17-F2-P1), within the agent's limits (KIT-A17-F4-P1). | integration | small | KIT-A17-F2-P1, KIT-A17-F4-P1 |  |
| MOV-A23-F2-P3 | **Guards against hidden instructions**: Synopses, captions and reviews reach an agent marked as data, never as instructions (KIT-A17-F5-P1). | integration | small | KIT-A17-F5-P1 |  |
| MOV-A23-F3-P1 | **AI use register**: Spare Movies' uses of AI in the kit's public register (KIT-A17-F10-P1), with review by a person on request (KIT-A17-F10-P2). | integration | small | KIT-A17-F10-P1, KIT-A17-F10-P2 |  |
| MOV-A23-F4-P1 | **Plugin points**: Plugin hooks for formats, publishing steps and filmmaker tools on the kit's plugin host (KIT-A16-F5-P2). | library | medium | kit-plugin, KIT-A16-F5-P2 |  |
| MOV-A23-F5-P1 | **Public API**: Scoped tokens and published rate limits from the kit (KIT-A16-F1-P2, KIT-A16-F1-P3). | integration | medium | KIT-A16-F1-P2, KIT-A16-F1-P3, MOV-A23-F1-P1 |  |
| MOV-A23-F5-P2 | **Catalogue feed**: Films and where to watch them as an open feed, on the kit's listing feed service (KIT-A28-F3-P2). | integration | small | KIT-A28-F3-P2, MOV-A2-F1-P1 |  |
| MOV-A23-F6-P1 | **Webhooks**: Spare Movies events on the kit's signed webhooks (KIT-A16-F2-P1). | integration | small | KIT-A16-F2-P1 |  |

## Questions for assessing this map

- Would the core features alone make a credible switch for someone leaving Netflix, Prime Video, Disney+ or a film rental store, when the first catalogue will be independent films rather than studio titles?
- Is paying per film (rent, buy, support or free) enough for a first release, or must the viewer pass be core, and should a pass be shared out by each viewer's own watch time or by one pool of all watch time (MOV-A7-F2-P1)?
- Can the at-cost fee stay small and easy to understand when delivery cost depends on a film's length, its quality and the viewer's country, and should it be a fixed amount per sale, an amount per hour watched, or both (MOV-A6-F1-P3, MOV-A18-F5-P1)?
- Which way of stopping copies should come first: locked playback with the agreements it needs, a hidden watermark per viewer, or no lock, and what do filmmakers and their investors actually require (MOV-A16-F1-P1)?
- Can fingerprints catch stolen uploads well enough with a person deciding every match, and fast enough that nobody collects money for a film they did not make (MOV-A16-F2)?
- How does a filmmaker prove they hold the rights in each country, and what happens to buyers when rights end or two people claim the same film (MOV-A11)?
- Which countries require official age ratings for on-demand films, which allow rating with an approved tool, and what would rating one film in every launch country cost a small filmmaker (MOV-A12-F1-P3)?
- Do local content rules and levies, such as the EU's share of European works, Canada's streaming contributions or Australia's local content rules, reach a small at-cost service, and from what size (MOV-A22-F5-P4)?
- Is asking for captions before publishing fair to filmmakers with no budget, and are checked caption drafts and descriptions read by a synthetic voice good enough for Deaf and blind viewers (MOV-A13)?
- Who is the seller of record for each sale, the filmmaker or the co-op, and can tax on films sold online be handled in every launch country without a tax team (MOV-A6-F8)?
- Does Spare Movies need its own licences from music collecting societies to stream films whose makers already cleared the music, and what would that cost (MOV-A11-F7-P2)?
- Can filmmakers really keep everything except running costs in phone and TV apps, given the app stores' rules for selling films (MOV-A6-F10)?
- Where is the line between AI-assisted and AI-made in film, for example AI dubbing, de-ageing or generated shots, and is a filmmakers' vote the right way to set it (MOV-A5-F1-P3)?
- Should sexually explicit films be allowed with full age checks and kept out of store apps, or ruled out (MOV-A8-F3-P2)?
- Selling to viewers in other countries rests on kit parts marked later (KIT-A3-F14, KIT-A4-F7); should the kit move them up for an international film service?
- Which parts should move down to the kit because Spare Social and Spare Music need them too: the Human and AI sections, fingerprinting, the pass share-out engine, rights by country, and encrypted delivery with a key service?
- Spare Movies has no work-board pieces yet. Which research and legal parts gate core features and should become pieces first: protection, fingerprinting, the ratings packs, rights proof, seller of record and tax, local content duties, collecting societies and access law?
- Is every open project used in the core under a licence that allows it, and are PeerTube (AGPL-3.0), Jellyfin and Kodi (GPL) kept to learning from how they work, with none of their code copied?
- What numbers would prove the demo works, such as time to first frame, stalls per hour, delivery cost per hour watched in each region, and the share of films with captions and audio description?
